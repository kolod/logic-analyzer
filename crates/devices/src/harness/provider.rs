//! A provider with scripted devices and scripted hot-plug sequences.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::Result;
use la_diagnostics::lock_recover;

use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::driver::{Device, DeviceEvent, DeviceEventSink, DeviceProvider};
use crate::error;
use crate::harness::scripted::{ScriptedDevice, Step};

/// Creates a fresh driver instance each time a device is opened.
pub type DeviceFactory = Arc<dyn Fn() -> Box<dyn Device> + Send + Sync>;

struct Entry {
    descriptor: DeviceDescriptor,
    factory: DeviceFactory,
}

struct State {
    entries: Vec<Entry>,
    sink: Option<DeviceEventSink>,
    push: bool,
}

struct Inner {
    name: String,
    state: Mutex<State>,
    scan_hang: AtomicBool,
    scan_error: AtomicBool,
    release: AtomicBool,
    scan_calls: AtomicUsize,
    open_calls: AtomicUsize,
}

/// A test provider. Cheap to clone: all clones share the same state, so keep one to script
/// the devices and give another (in an `Arc`) to the registry.
#[derive(Clone)]
pub struct TestProvider {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for TestProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestProvider")
            .field("name", &self.inner.name)
            .finish_non_exhaustive()
    }
}

impl TestProvider {
    /// Creates an empty provider with the given name (the id prefix of its devices).
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            inner: Arc::new(Inner {
                name: name.to_owned(),
                state: Mutex::new(State {
                    entries: Vec::new(),
                    sink: None,
                    push: true,
                }),
                scan_hang: AtomicBool::new(false),
                scan_error: AtomicBool::new(false),
                release: AtomicBool::new(false),
                scan_calls: AtomicUsize::new(0),
                open_calls: AtomicUsize::new(0),
            }),
        }
    }

    /// Connects a device whose driver instances replay an immediate `Finished`.
    pub fn plug(&self, descriptor: DeviceDescriptor) {
        let for_factory = descriptor.clone();
        self.plug_with(
            descriptor,
            Arc::new(move || {
                Box::new(ScriptedDevice::new(for_factory.clone(), vec![Step::Finish]))
            }),
        );
    }

    /// Connects a device whose driver instances come from `factory`. Pushes an `Appeared`
    /// event if the registry is watching and pushing is enabled.
    pub fn plug_with(&self, descriptor: DeviceDescriptor, factory: DeviceFactory) {
        let mut state = lock_recover(&self.inner.state);
        state.entries.retain(|e| e.descriptor.id != descriptor.id);
        state.entries.push(Entry {
            descriptor: descriptor.clone(),
            factory,
        });
        if state.push {
            if let Some(sink) = &state.sink {
                sink.send(DeviceEvent::Appeared(descriptor));
            }
        }
    }

    /// Disconnects a device. Pushes a `Disappeared` event if the registry is watching and
    /// pushing is enabled.
    pub fn unplug(&self, id: &DeviceId) {
        let mut state = lock_recover(&self.inner.state);
        let before = state.entries.len();
        state.entries.retain(|e| e.descriptor.id != *id);
        if state.entries.len() != before && state.push {
            if let Some(sink) = &state.sink {
                sink.send(DeviceEvent::Disappeared(id.clone()));
            }
        }
    }

    /// Enables or disables pushing of hot-plug events (disabled: only scans reveal changes).
    pub fn set_push(&self, push: bool) {
        lock_recover(&self.inner.state).push = push;
    }

    /// Makes `scan` hang until [`release`](Self::release) is called.
    pub fn set_scan_hang(&self, hang: bool) {
        self.inner.scan_hang.store(hang, Ordering::Release);
    }

    /// Makes `scan` return an error.
    pub fn set_scan_error(&self, fail: bool) {
        self.inner.scan_error.store(fail, Ordering::Release);
    }

    /// Ends every hanging `scan` so abandoned threads can finish.
    pub fn release(&self) {
        self.inner.release.store(true, Ordering::Release);
    }

    /// Number of `scan` calls so far.
    #[must_use]
    pub fn scan_calls(&self) -> usize {
        self.inner.scan_calls.load(Ordering::Acquire)
    }

    /// Number of `open` calls so far.
    #[must_use]
    pub fn open_calls(&self) -> usize {
        self.inner.open_calls.load(Ordering::Acquire)
    }
}

impl DeviceProvider for TestProvider {
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>> {
        self.inner.scan_calls.fetch_add(1, Ordering::AcqRel);
        while self.inner.scan_hang.load(Ordering::Acquire)
            && !self.inner.release.load(Ordering::Acquire)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.inner.scan_error.load(Ordering::Acquire) {
            return Err(error::driver_error("scripted scan failure"));
        }
        let state = lock_recover(&self.inner.state);
        Ok(state.entries.iter().map(|e| e.descriptor.clone()).collect())
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        self.inner.open_calls.fetch_add(1, Ordering::AcqRel);
        let state = lock_recover(&self.inner.state);
        match state.entries.iter().find(|e| e.descriptor.id == *id) {
            Some(entry) => Ok((entry.factory)()),
            None => Err(error::not_found(id.as_str())),
        }
    }

    fn watch(&self, sink: DeviceEventSink) {
        lock_recover(&self.inner.state).sink = Some(sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::test_descriptor;

    #[test]
    fn plug_and_unplug_change_the_scan_and_push_events() {
        let p = TestProvider::new("t");
        let (sink, events) = DeviceEventSink::channel();
        p.watch(sink);
        let d = test_descriptor("t", "1", 4);
        p.plug(d.clone());
        assert_eq!(p.scan().unwrap(), vec![d.clone()]);
        assert_eq!(events.try_recv().unwrap(), DeviceEvent::Appeared(d.clone()));
        p.unplug(&d.id);
        assert!(p.scan().unwrap().is_empty());
        assert_eq!(
            events.try_recv().unwrap(),
            DeviceEvent::Disappeared(d.id.clone())
        );
        assert!(events.try_recv().is_err());
        assert_eq!(p.scan_calls(), 2);
    }

    #[test]
    fn pushing_can_be_disabled_and_open_is_counted() {
        let p = TestProvider::new("t");
        let (sink, events) = DeviceEventSink::channel();
        p.watch(sink);
        p.set_push(false);
        let d = test_descriptor("t", "1", 4);
        p.plug(d.clone());
        assert!(events.try_recv().is_err());
        assert!(p.open(&d.id).is_ok());
        assert_eq!(
            p.open(&DeviceId::new("t", "nope")).err().unwrap().code(),
            error::NOT_FOUND
        );
        assert_eq!(p.open_calls(), 2);
    }

    #[test]
    fn scan_can_fail_or_hang_until_released() {
        let p = TestProvider::new("t");
        p.set_scan_error(true);
        assert_eq!(p.scan().unwrap_err().code(), error::DRIVER_ERROR);
        p.set_scan_error(false);
        p.set_scan_hang(true);
        let clone = p.clone();
        let handle = std::thread::spawn(move || clone.scan().map(|v| v.len()));
        std::thread::sleep(Duration::from_millis(30));
        p.release();
        assert_eq!(handle.join().unwrap().unwrap(), 0);
    }
}
