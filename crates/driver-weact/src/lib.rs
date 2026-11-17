//! Driver for the WeAct LogicAnalyzer V1.
//!
//! The board is an 8-channel, 24 MHz, streaming logic analyzer built on a Cypress FX2 USB
//! controller that holds no firmware of its own. This crate implements the device contract of
//! `la-devices` for it: it lists boards by their position on the USB bus, prepares a blank board
//! by loading an open-source firmware image on a background worker, and streams samples from one
//! bulk endpoint into the acquisition engine. All USB access goes through a small seam so that
//! the driver is tested without hardware.
//!
//! The contracts are in `specs/013-driver-weact/contracts/`: the wire protocol, the behaviour
//! against the device contract, and the test rig.

#![deny(missing_docs)]

mod boards;
mod device;
pub mod error;
pub mod firmware;
mod nusb_backend;
mod prepare;
pub mod protocol;
#[cfg(feature = "test-support")]
pub mod recording;
#[cfg(feature = "test-support")]
pub mod testing;
pub mod usb;

use std::sync::{Arc, Mutex};

use la_core::Error;
use la_devices::{Device, DeviceDescriptor, DeviceId, DeviceProvider};
use la_diagnostics::lock_recover;

use crate::boards::Table;
use crate::device::{WeActDevice, check_firmware};
use crate::firmware::Image;
use crate::nusb_backend::NusbBackend;
use crate::usb::{BoardKey, SystemTimer, Timer, UsbBackend};

/// The name of the provider and the prefix of every device identity of this driver.
pub const PROVIDER_NAME: &str = "weact";

/// What the provider and its preparation workers share.
pub(crate) struct Shared {
    pub(crate) backend: Arc<dyn UsbBackend>,
    pub(crate) timer: Arc<dyn Timer>,
    pub(crate) table: Mutex<Table>,
    pub(crate) image: Image,
}

/// The provider of the WeAct LogicAnalyzer V1 boards on the USB bus.
pub struct WeActProvider {
    shared: Arc<Shared>,
}

impl WeActProvider {
    /// A provider over the given USB access and clock (tests pass the scripted bus), with the
    /// firmware image that ships with the application.
    #[must_use]
    pub fn with_backend(backend: Arc<dyn UsbBackend>, timer: Arc<dyn Timer>) -> Self {
        Self::with_image(backend, timer, Image::embedded())
    }

    /// A provider that loads `image` into blank boards (tests pass images of other sizes).
    #[must_use]
    pub fn with_image(backend: Arc<dyn UsbBackend>, timer: Arc<dyn Timer>, image: Image) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                timer,
                table: Mutex::new(Table::default()),
                image,
            }),
        }
    }

    /// Starts the worker that prepares the blank board at `key`.
    fn start_preparation(&self, key: BoardKey) {
        let shared = Arc::clone(&self.shared);
        let name = format!("weact-prepare-{key}");
        // The worker reports to the table itself; its handle is not needed.
        let _worker = la_diagnostics::spawn_worker(&name, move || prepare::run(shared, key));
    }
}

/// The access to the real USB bus (for tests that wrap it, for example to record the exchange).
#[must_use]
pub fn system_backend() -> Arc<dyn UsbBackend> {
    Arc::new(NusbBackend)
}

/// The provider of the real USB bus, for the application to register.
#[must_use]
pub fn provider() -> Arc<WeActProvider> {
    Arc::new(WeActProvider::with_backend(
        Arc::new(NusbBackend),
        Arc::new(SystemTimer::new()),
    ))
}

impl DeviceProvider for WeActProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>, Error> {
        let seen = match self.shared.backend.list() {
            Ok(seen) => seen,
            Err(error) => {
                // The boards already known stay listed; with none known the error is the answer.
                let table = lock_recover(&self.shared.table);
                if table.is_empty() {
                    return Err(error);
                }
                tracing::warn!(
                    target: "la_driver_weact",
                    code = error.code(),
                    detail = error.detail(),
                    "the USB bus could not be read; keeping the boards already known"
                );
                return Ok(table.descriptors());
            }
        };
        let now = self.shared.timer.now();
        let (descriptors, to_prepare) = {
            let mut table = lock_recover(&self.shared.table);
            let to_prepare = table.reconcile(&seen, now);
            (table.descriptors(), to_prepare)
        };
        for key in to_prepare {
            self.start_preparation(key);
        }
        Ok(descriptors)
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>, Error> {
        let (key, descriptor) = lock_recover(&self.shared.table).take_open(id)?;
        let mut link = self.shared.backend.open(&key)?;
        check_firmware(&mut *link)?;
        Ok(Box::new(WeActDevice::new(
            descriptor,
            link,
            Arc::clone(&self.shared.timer),
        )))
    }
}
