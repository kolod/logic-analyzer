//! The device registry: discovery of devices and hot-plug events.
//!
//! A monitor thread keeps the device list current. It scans every provider periodically (each
//! scan runs on its own contained worker and is waited for with a timeout, so a provider that
//! hangs, fails or panics never blocks anyone) and applies the hot-plug events that providers
//! push. Every change of the list, whatever its source, goes through one place that compares
//! the old and the new list, so each change produces exactly one event, a repeated or
//! irrelevant event produces none, and events keep their order. The list is cached, so
//! [`DeviceRegistry::list`] is instant and never waits for a device.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use la_diagnostics::{lock_recover, spawn_worker};

use crate::acquisition::{Acquisition, AcquisitionOptions};
use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::config::{DeviceConfig, DeviceOptions, validate};
use crate::driver::{DeviceEvent, DeviceEventSink, DeviceProvider};
use crate::error;
use crate::supervisor::{Launch, launch};
use crate::virtual_device::{PROVIDER_NAME, VirtualProvider, virtual_descriptor};

/// How the registry scans its providers.
#[derive(Debug, Clone, Copy)]
pub struct RegistryConfig {
    /// Time between two scans of a provider.
    pub scan_interval: Duration,
    /// How long a scan may take before the provider is considered unresponsive.
    pub scan_timeout: Duration,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_millis(500),
            scan_timeout: Duration::from_secs(1),
        }
    }
}

struct Slot {
    provider: Arc<dyn DeviceProvider>,
    /// `true` while a scan of this provider is still running (possibly hung).
    in_flight: Arc<AtomicBool>,
    /// Counts the events this provider has pushed; used to discard stale scan results.
    pushes: Arc<AtomicU64>,
    responsive: bool,
    devices: Vec<DeviceDescriptor>,
}

struct Shared {
    config: RegistryConfig,
    slots: Mutex<Vec<Slot>>,
    known: Mutex<Vec<DeviceDescriptor>>,
    /// Events pushed by providers, with the name of the provider, in arrival order.
    incoming: Mutex<VecDeque<(String, DeviceEvent)>>,
    subscribers: Mutex<Vec<Sender<DeviceEvent>>>,
    woken: Mutex<bool>,
    wake: Condvar,
    scan_requested: AtomicBool,
    stop: AtomicBool,
    reported_invalid: Mutex<HashSet<DeviceId>>,
    /// Devices that currently run an acquisition.
    busy: Mutex<HashSet<DeviceId>>,
}

/// The device registry. Owns the monitor thread; dropping the registry stops it.
pub struct DeviceRegistry {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for DeviceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceRegistry").finish_non_exhaustive()
    }
}

impl DeviceRegistry {
    /// Creates a registry with the built-in virtual provider and starts its monitor.
    #[must_use]
    pub fn new(config: RegistryConfig) -> Self {
        let virtual_provider: Arc<dyn DeviceProvider> = Arc::new(VirtualProvider);
        let shared = Arc::new(Shared {
            config,
            slots: Mutex::new(vec![Slot {
                provider: virtual_provider,
                in_flight: Arc::new(AtomicBool::new(false)),
                pushes: Arc::new(AtomicU64::new(0)),
                responsive: true,
                devices: vec![virtual_descriptor()],
            }]),
            // The virtual device is listed from the first moment, before any scan.
            known: Mutex::new(vec![virtual_descriptor()]),
            incoming: Mutex::new(VecDeque::new()),
            subscribers: Mutex::new(Vec::new()),
            woken: Mutex::new(false),
            wake: Condvar::new(),
            scan_requested: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            reported_invalid: Mutex::new(HashSet::new()),
            busy: Mutex::new(HashSet::new()),
        });
        let weak = Arc::downgrade(&shared);
        // Dropping the handle detaches the monitor; it ends when the registry is dropped.
        drop(spawn_worker("device-monitor", move || monitor(&weak)));
        Self { shared }
    }

    /// Adds a provider; its devices appear after its first scan (promptly, without blocking
    /// the caller). The provider is asked to push hot-plug events.
    pub fn add_provider(&self, provider: Arc<dyn DeviceProvider>) {
        let pushes = Arc::new(AtomicU64::new(0));
        lock_recover(&self.shared.slots).push(Slot {
            provider: Arc::clone(&provider),
            in_flight: Arc::new(AtomicBool::new(false)),
            pushes: Arc::clone(&pushes),
            responsive: true,
            devices: Vec::new(),
        });
        let weak = Arc::downgrade(&self.shared);
        let name = provider.name().to_owned();
        provider.watch(DeviceEventSink::new(move |event| {
            let Some(shared) = weak.upgrade() else {
                return false;
            };
            lock_recover(&shared.incoming).push_back((name.clone(), event));
            pushes.fetch_add(1, Ordering::AcqRel);
            shared.wake_monitor();
            true
        }));
        self.shared.scan_requested.store(true, Ordering::Release);
        self.shared.wake_monitor();
    }

    /// Subscribes to hot-plug events: a `Receiver` that gets every change of the device list
    /// from now on, in order, each exactly once. The virtual device never disappears. Every
    /// subscriber receives every event.
    #[must_use]
    pub fn events(&self) -> Receiver<DeviceEvent> {
        let (sender, receiver) = channel();
        lock_recover(&self.shared.subscribers).push(sender);
        receiver
    }

    /// The cached device list: the virtual device first, then the others by id. Instant;
    /// never waits for a device.
    #[must_use]
    pub fn list(&self) -> Vec<DeviceDescriptor> {
        lock_recover(&self.shared.known).clone()
    }

    /// Validates `config`, claims the device and starts an acquisition in the background.
    ///
    /// Returns at once: opening and starting the driver happen on the acquisition's own
    /// thread, so a slow or hung driver cannot block the caller. The configuration is checked
    /// against the device's capabilities first, so an invalid one never reaches a driver
    /// (not even `DeviceProvider::open`).
    ///
    /// # Errors
    ///
    /// `device.not_found`; the validation errors of [`validate`]; `device.busy` if the device
    /// already runs an acquisition; `device.invalid_state` for virtual device options on a
    /// device that is not virtual; storage errors for invalid limits.
    pub fn start(
        &self,
        id: &DeviceId,
        config: DeviceConfig,
        options: AcquisitionOptions,
    ) -> la_core::Result<Acquisition> {
        let descriptor = self
            .describe(id)
            .ok_or_else(|| error::not_found(id.as_str()))?;
        let resolved = validate(&descriptor.capabilities, &config, &options.limits)?;
        if matches!(config.options, DeviceOptions::Virtual(_))
            && descriptor.id.provider() != PROVIDER_NAME
        {
            return Err(error::invalid_state(format!(
                "virtual device options were given for `{id}`, which is not the virtual device"
            )));
        }
        let provider = lock_recover(&self.shared.slots)
            .iter()
            .find(|slot| slot.provider.name() == id.provider())
            .map(|slot| Arc::clone(&slot.provider))
            .ok_or_else(|| error::not_found(id.as_str()))?;

        // Claim the device: exactly one of several simultaneous starts succeeds.
        if !lock_recover(&self.shared.busy).insert(id.clone()) {
            return Err(error::busy(id.as_str()));
        }
        let shared = Arc::clone(&self.shared);
        let claimed = id.clone();
        let release: Box<dyn FnOnce() + Send> = Box::new(move || {
            lock_recover(&shared.busy).remove(&claimed);
        });
        let shared = Arc::clone(&self.shared);
        let claimed = id.clone();
        let release_on_error = move || {
            lock_recover(&shared.busy).remove(&claimed);
        };
        launch(Launch {
            descriptor,
            provider,
            config: resolved,
            options,
            release,
        })
        .inspect_err(|_| release_on_error())
    }

    /// The descriptor of a listed device.
    #[must_use]
    pub fn describe(&self, id: &DeviceId) -> Option<DeviceDescriptor> {
        lock_recover(&self.shared.known)
            .iter()
            .find(|d| d.id == *id)
            .cloned()
    }
}

impl Drop for DeviceRegistry {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.wake_monitor();
    }
}

impl Shared {
    fn wake_monitor(&self) {
        *lock_recover(&self.woken) = true;
        self.wake.notify_all();
    }

    /// Sleeps until `deadline`, or until woken (a pushed event, a new provider, a stop).
    fn wait_until(&self, deadline: Instant) {
        let timeout = deadline.saturating_duration_since(Instant::now());
        let guard = lock_recover(&self.woken);
        let (mut guard, _) = self
            .wake
            .wait_timeout_while(guard, timeout, |woken| {
                !*woken && !self.stop.load(Ordering::Acquire)
            })
            .unwrap_or_else(PoisonError::into_inner);
        *guard = false;
    }

    /// Sends an event to every subscriber; subscribers that went away are dropped.
    fn broadcast(&self, event: &DeviceEvent) {
        lock_recover(&self.subscribers).retain(|s| s.send(event.clone()).is_ok());
    }

    /// Rebuilds the cached list from the providers' devices, and announces what changed:
    /// first the devices that disappeared, then the ones that appeared, each exactly once.
    fn publish(&self, slots: &[Slot]) {
        let mut list: Vec<DeviceDescriptor> = Vec::new();
        for slot in slots {
            for descriptor in &slot.devices {
                if let Err(error) = descriptor.validate() {
                    if lock_recover(&self.reported_invalid).insert(descriptor.id.clone()) {
                        tracing::warn!(
                            target: "la_devices",
                            device = descriptor.id.as_str(),
                            code = error.code(),
                            "an invalid device description was omitted"
                        );
                    }
                    continue;
                }
                if list.iter().all(|d| d.id != descriptor.id) {
                    list.push(descriptor.clone());
                }
            }
        }
        list.sort_by(|a, b| {
            let key = |d: &DeviceDescriptor| (d.id.provider() != PROVIDER_NAME, d.id.clone());
            key(a).cmp(&key(b))
        });
        let old = std::mem::replace(&mut *lock_recover(&self.known), list.clone());
        for gone in old.iter().filter(|d| list.iter().all(|n| n.id != d.id)) {
            self.broadcast(&DeviceEvent::Disappeared(gone.id.clone()));
        }
        for new in list.iter().filter(|n| old.iter().all(|d| d.id != n.id)) {
            self.broadcast(&DeviceEvent::Appeared(new.clone()));
        }
    }

    /// Applies the events providers have pushed, one by one, in arrival order.
    fn process_pushed(&self) {
        loop {
            let Some((provider, event)) = lock_recover(&self.incoming).pop_front() else {
                return;
            };
            let mut slots = lock_recover(&self.slots);
            let Some(slot) = slots.iter_mut().find(|s| s.provider.name() == provider) else {
                continue;
            };
            match event {
                DeviceEvent::Appeared(descriptor) => {
                    if slot.devices.iter().all(|d| d.id != descriptor.id) {
                        slot.devices.push(descriptor);
                    }
                }
                DeviceEvent::Disappeared(id) => {
                    // The virtual device never disappears.
                    if provider != PROVIDER_NAME {
                        slot.devices.retain(|d| d.id != id);
                    }
                }
            }
            self.publish(&slots);
        }
    }
}

/// Clears the in-flight flag when a scan worker ends, even if it panics.
struct ResetFlag(Arc<AtomicBool>);

impl Drop for ResetFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

enum ScanResult {
    Devices(Vec<DeviceDescriptor>),
    Failed,
    Unresponsive,
}

/// Runs one provider scan on a contained worker and waits at most `timeout` for it.
fn scan_provider(
    provider: &Arc<dyn DeviceProvider>,
    in_flight: &Arc<AtomicBool>,
    timeout: Duration,
) -> ScanResult {
    // A previous scan that is still running (hung) is not joined by a new one: no piling up.
    if in_flight.swap(true, Ordering::AcqRel) {
        return ScanResult::Unresponsive;
    }
    let (sender, receiver) = channel();
    let reset = ResetFlag(Arc::clone(in_flight));
    let worker_provider = Arc::clone(provider);
    drop(spawn_worker(
        &format!("scan-{}", provider.name()),
        move || {
            let _reset = reset;
            let result = worker_provider.scan();
            // The receiver is gone if the scan was too late; nobody needs the result then.
            let _ = sender.send(result);
        },
    ));
    match receiver.recv_timeout(timeout) {
        Ok(Ok(devices)) => ScanResult::Devices(devices),
        Ok(Err(error)) => {
            tracing::warn!(
                target: "la_devices",
                provider = provider.name(),
                code = error.code(),
                "a device provider failed to scan"
            );
            ScanResult::Failed
        }
        // Timeout, or the worker panicked before sending (contained by `spawn_worker`).
        Err(_) => ScanResult::Unresponsive,
    }
}

/// Scans every provider once and applies the results.
///
/// A scan result is stale if the provider pushed events while the scan ran (the pushed
/// events are newer than the scan); such a result is discarded and the next round
/// reconciles it, so scans and pushes never disagree about the order of changes.
fn scan_all(shared: &Shared) {
    // (provider, in-flight flag, push counter, pushes seen before the scan)
    type Work = (
        Arc<dyn DeviceProvider>,
        Arc<AtomicBool>,
        Arc<AtomicU64>,
        u64,
    );
    let work: Vec<Work> = lock_recover(&shared.slots)
        .iter()
        .map(|s| {
            (
                Arc::clone(&s.provider),
                Arc::clone(&s.in_flight),
                Arc::clone(&s.pushes),
                s.pushes.load(Ordering::Acquire),
            )
        })
        .collect();
    let results: Vec<ScanResult> = work
        .iter()
        .map(|(provider, in_flight, _, _)| {
            if provider.name() == PROVIDER_NAME {
                // The virtual provider cannot hang or fail; avoid a thread per scan.
                return provider
                    .scan()
                    .map_or(ScanResult::Failed, ScanResult::Devices);
            }
            scan_provider(provider, in_flight, shared.config.scan_timeout)
        })
        .collect();

    // Apply what was pushed first, then the scan results that are still current.
    shared.process_pushed();
    let mut slots = lock_recover(&shared.slots);
    for ((slot, result), (_, _, pushes, before)) in slots.iter_mut().zip(results).zip(&work) {
        if pushes.load(Ordering::Acquire) != *before {
            continue;
        }
        let (responsive, devices) = match result {
            ScanResult::Devices(devices) => (true, devices),
            ScanResult::Failed | ScanResult::Unresponsive => (false, Vec::new()),
        };
        if responsive != slot.responsive {
            tracing::warn!(
                target: "la_devices",
                provider = slot.provider.name(),
                responsive,
                "a device provider changed its responsiveness"
            );
        }
        slot.responsive = responsive;
        slot.devices = devices;
    }
    shared.publish(&slots);
}

/// The monitor loop; it ends when the registry is dropped.
fn monitor(weak: &Weak<Shared>) {
    let mut next_scan = Instant::now();
    loop {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        shared.process_pushed();
        if shared.scan_requested.swap(false, Ordering::AcqRel) || Instant::now() >= next_scan {
            scan_all(&shared);
            next_scan = Instant::now()
                .checked_add(shared.config.scan_interval)
                .unwrap_or_else(Instant::now);
        }
        shared.wait_until(next_scan);
    }
}
