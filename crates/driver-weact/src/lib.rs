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
mod firmware;
mod nusb_backend;
pub mod protocol;
#[cfg(feature = "test-support")]
pub mod testing;
pub mod usb;

use std::sync::{Arc, Mutex};

use la_core::Error;
use la_devices::error as device_error;
use la_devices::{Device, DeviceDescriptor, DeviceId, DeviceProvider};
use la_diagnostics::lock_recover;

use crate::boards::Table;
use crate::device::{WeActDevice, check_firmware};
use crate::nusb_backend::NusbBackend;
use crate::usb::{SystemTimer, Timer, UsbBackend};

/// The name of the provider and the prefix of every device identity of this driver.
pub const PROVIDER_NAME: &str = "weact";

/// The provider of the WeAct LogicAnalyzer V1 boards on the USB bus.
pub struct WeActProvider {
    backend: Arc<dyn UsbBackend>,
    timer: Arc<dyn Timer>,
    table: Mutex<Table>,
}

impl WeActProvider {
    /// A provider over the given USB access and clock (tests pass the scripted bus).
    #[must_use]
    pub fn with_backend(backend: Arc<dyn UsbBackend>, timer: Arc<dyn Timer>) -> Self {
        Self {
            backend,
            timer,
            table: Mutex::new(Table::default()),
        }
    }
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
        let seen = match self.backend.list() {
            Ok(seen) => seen,
            Err(error) => {
                // The boards already known stay listed; with none known the error is the answer.
                let table = lock_recover(&self.table);
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
        let now = self.timer.now();
        let mut table = lock_recover(&self.table);
        table.reconcile(&seen, now);
        Ok(table.descriptors())
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>, Error> {
        let (key, descriptor) = {
            let table = lock_recover(&self.table);
            let Some(key) = table.key_of(id) else {
                return Err(device_error::not_found(id.as_str()));
            };
            let Some(descriptor) = table.descriptors().into_iter().find(|d| d.id == *id) else {
                return Err(device_error::not_found(id.as_str()));
            };
            (key, descriptor)
        };
        let mut link = self.backend.open(&key)?;
        check_firmware(&mut *link)?;
        Ok(Box::new(WeActDevice::new(
            descriptor,
            link,
            Arc::clone(&self.timer),
        )))
    }
}
