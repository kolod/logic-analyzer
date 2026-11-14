//! The device registry: discovery of devices from all providers.
//!
//! A monitor thread scans every provider periodically. Each scan runs on its own contained
//! worker and is waited for with a timeout, so a provider that hangs, fails or panics never
//! blocks anyone: it is simply omitted from the list until it works again. The list is
//! cached, so [`DeviceRegistry::list`] is instant and never waits for a device.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::time::Duration;

use la_diagnostics::{lock_recover, spawn_worker};

use crate::acquisition::{Acquisition, AcquisitionOptions};
use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::config::{DeviceConfig, DeviceOptions, validate};
use crate::driver::DeviceProvider;
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
    responsive: bool,
    devices: Vec<DeviceDescriptor>,
}

struct Shared {
    config: RegistryConfig,
    slots: Mutex<Vec<Slot>>,
    known: Mutex<Vec<DeviceDescriptor>>,
    woken: Mutex<bool>,
    wake: Condvar,
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
                responsive: true,
                devices: vec![virtual_descriptor()],
            }]),
            // The virtual device is listed from the first moment, before any scan.
            known: Mutex::new(vec![virtual_descriptor()]),
            woken: Mutex::new(false),
            wake: Condvar::new(),
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
    /// the caller).
    pub fn add_provider(&self, provider: Arc<dyn DeviceProvider>) {
        lock_recover(&self.shared.slots).push(Slot {
            provider,
            in_flight: Arc::new(AtomicBool::new(false)),
            responsive: true,
            devices: Vec::new(),
        });
        self.shared.wake_monitor();
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

    /// Sleeps until the next scan is due, or until woken or stopped.
    fn wait_for_next_scan(&self) {
        let guard = lock_recover(&self.woken);
        let (mut guard, _) = self
            .wake
            .wait_timeout_while(guard, self.config.scan_interval, |woken| {
                !*woken && !self.stop.load(Ordering::Acquire)
            })
            .unwrap_or_else(PoisonError::into_inner);
        *guard = false;
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

/// Scans every provider once and replaces the cached list.
fn scan_all(shared: &Shared) {
    let work: Vec<(Arc<dyn DeviceProvider>, Arc<AtomicBool>)> = lock_recover(&shared.slots)
        .iter()
        .map(|s| (Arc::clone(&s.provider), Arc::clone(&s.in_flight)))
        .collect();
    let results: Vec<ScanResult> = work
        .iter()
        .map(|(provider, in_flight)| {
            if provider.name() == PROVIDER_NAME {
                // The virtual provider cannot hang or fail; avoid a thread per scan.
                return provider
                    .scan()
                    .map_or(ScanResult::Failed, ScanResult::Devices);
            }
            scan_provider(provider, in_flight, shared.config.scan_timeout)
        })
        .collect();

    let mut slots = lock_recover(&shared.slots);
    for (slot, result) in slots.iter_mut().zip(results) {
        let (responsive, devices) = match result {
            ScanResult::Devices(devices) => (true, devices),
            ScanResult::Failed => (false, Vec::new()),
            ScanResult::Unresponsive => (false, Vec::new()),
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

    let mut known: Vec<DeviceDescriptor> = Vec::new();
    for slot in slots.iter() {
        for descriptor in &slot.devices {
            if let Err(error) = descriptor.validate() {
                if lock_recover(&shared.reported_invalid).insert(descriptor.id.clone()) {
                    tracing::warn!(
                        target: "la_devices",
                        device = descriptor.id.as_str(),
                        code = error.code(),
                        "an invalid device description was omitted"
                    );
                }
                continue;
            }
            if known.iter().all(|d| d.id != descriptor.id) {
                known.push(descriptor.clone());
            }
        }
    }
    known.sort_by(|a, b| {
        let key = |d: &DeviceDescriptor| (d.id.provider() != PROVIDER_NAME, d.id.clone());
        key(a).cmp(&key(b))
    });
    *lock_recover(&shared.known) = known;
}

/// The monitor loop; it ends when the registry is dropped.
fn monitor(weak: &Weak<Shared>) {
    loop {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        scan_all(&shared);
        shared.wait_for_next_scan();
    }
}
