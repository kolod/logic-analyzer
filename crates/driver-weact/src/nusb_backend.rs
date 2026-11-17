//! The only module that uses the `nusb` crate: the real implementation of the USB seam.
//!
//! It maps what the system says onto the typed errors of the driver (`map_failure`, tested as a
//! table) and keeps the bulk stream running by resubmitting each buffer that completes. It has no
//! `unsafe`: `nusb` is the safe wrapper. What needs hardware (a real board) is checked by the
//! probe example and by the rig tests, not here.

use std::time::{Duration, Instant};

use la_core::Error;
use la_devices::error as device_error;
use nusb::transfer::{
    Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Recipient, TransferError,
};
use nusb::{DeviceInfo, Endpoint, ErrorKind, Interface, MaybeFuture};

use crate::error;
use crate::protocol::{BufferPlan, ENDPOINT_IN, INTERFACE};
use crate::usb::{BoardKey, BulkStream, StreamEvent, UsbBackend, UsbInfo, UsbLink};

/// How the system said that something failed, in the terms the driver cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The device is gone.
    Disconnected,
    /// The user may not do this.
    PermissionDenied,
    /// Another program or driver holds it.
    Busy,
    /// The platform or the bound driver does not support the operation.
    Unsupported,
    /// The thing asked for was not found.
    NotFound,
    /// Anything else.
    Other,
}

impl From<ErrorKind> for Failure {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::Disconnected => Failure::Disconnected,
            ErrorKind::PermissionDenied => Failure::PermissionDenied,
            ErrorKind::Busy => Failure::Busy,
            ErrorKind::Unsupported => Failure::Unsupported,
            ErrorKind::NotFound => Failure::NotFound,
            _ => Failure::Other,
        }
    }
}

/// Whether `driver` is the driver the application needs (WinUSB, compared without case).
fn is_winusb(driver: &str) -> bool {
    driver.eq_ignore_ascii_case("winusb")
}

/// The typed error for a failure while opening or claiming. `driver` is the bound driver name
/// where the platform gives it: a refusal of the system on a board that is bound to something
/// other than WinUSB is a missing driver, not a missing permission.
pub(crate) fn map_failure(failure: Failure, detail: &str, driver: Option<&str>) -> Error {
    let foreign_driver = driver.filter(|name| !is_winusb(name));
    match failure {
        Failure::Disconnected => device_error::disconnected(detail),
        Failure::Busy => error::in_use(detail),
        Failure::PermissionDenied => match foreign_driver {
            Some(name) if cfg!(windows) => error::driver_missing(Some(name)),
            _ => error::access_denied(detail),
        },
        Failure::Unsupported | Failure::NotFound => match foreign_driver {
            Some(name) => error::driver_missing(Some(name)),
            None if cfg!(windows) => error::driver_missing(driver),
            None => device_error::transport(detail),
        },
        Failure::Other => device_error::transport(detail),
    }
}

fn transfer_error(error: TransferError, what: &str) -> Error {
    match error {
        TransferError::Disconnected => {
            device_error::disconnected(format!("{what}: the board was unplugged"))
        }
        other => device_error::transport(format!("{what} failed: {other:?}")),
    }
}

/// The real USB bus.
#[derive(Debug, Default, Clone, Copy)]
pub struct NusbBackend;

fn info_of(device: &DeviceInfo) -> UsbInfo {
    UsbInfo {
        key: BoardKey::new(device.bus_id(), device.port_chain().to_vec()),
        vendor_id: device.vendor_id(),
        product_id: device.product_id(),
        manufacturer: device.manufacturer_string().map(str::to_owned),
        product: device.product_string().map(str::to_owned),
        address: device.device_address(),
        driver: bound_driver(device),
    }
}

impl UsbBackend for NusbBackend {
    fn list(&self) -> Result<Vec<UsbInfo>, Error> {
        let devices = nusb::list_devices().wait().map_err(|e| {
            device_error::transport(format!("the USB bus could not be listed: {e}"))
        })?;
        Ok(devices.map(|device| info_of(&device)).collect())
    }

    fn open(&self, key: &BoardKey) -> Result<Box<dyn UsbLink>, Error> {
        let devices = nusb::list_devices().wait().map_err(|e| {
            device_error::transport(format!("the USB bus could not be listed: {e}"))
        })?;
        let Some(found) = devices
            .into_iter()
            .find(|device| device.bus_id() == key.bus() && device.port_chain() == key.ports())
        else {
            return Err(device_error::disconnected(format!(
                "the board at {key} is not on the bus"
            )));
        };
        let driver = bound_driver(&found);
        // A board that is not on WinUSB cannot be opened on Windows; say so before trying.
        if let Some(name) = driver.as_deref().filter(|name| !is_winusb(name)) {
            if cfg!(windows) {
                return Err(error::driver_missing(Some(name)));
            }
        }
        let device = found.open().wait().map_err(|e| {
            map_failure(
                e.kind().into(),
                &format!("opening the board at {key}: {e}"),
                driver.as_deref(),
            )
        })?;
        // Control transfers go through the interface on every platform (on Windows the device
        // handle has none), so the interface is claimed here, as libsigrok does when it opens.
        let interface = device.claim_interface(INTERFACE).wait().map_err(|e| {
            map_failure(
                e.kind().into(),
                &format!("claiming the interface of the board at {key}: {e}"),
                driver.as_deref(),
            )
        })?;
        Ok(Box::new(NusbLink { interface, driver }))
    }
}

struct NusbLink {
    interface: Interface,
    driver: Option<String>,
}

impl UsbLink for NusbLink {
    fn control_in(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        timeout: Duration,
    ) -> Result<Vec<u8>, Error> {
        self.interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request,
                    value,
                    index,
                    length,
                },
                timeout,
            )
            .wait()
            .map_err(|e| transfer_error(e, "a control read"))
    }

    fn control_out(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), Error> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request,
                    value,
                    index,
                    data,
                },
                timeout,
            )
            .wait()
            .map_err(|e| transfer_error(e, "a control write"))
    }

    fn open_stream(&mut self, plan: BufferPlan) -> Result<Box<dyn BulkStream>, Error> {
        let driver = self.driver.clone();
        let endpoint = self
            .interface
            .endpoint::<Bulk, In>(ENDPOINT_IN)
            .map_err(|e| {
                map_failure(
                    e.kind().into(),
                    &format!("opening the bulk endpoint: {e}"),
                    driver.as_deref(),
                )
            })?;
        let buffers = (0..plan.count)
            .map(|_| endpoint.allocate(plan.size))
            .collect();
        Ok(Box::new(NusbStream {
            endpoint: Some(endpoint),
            idle: buffers,
        }))
    }
}

struct NusbStream {
    endpoint: Option<Endpoint<Bulk, In>>,
    /// Buffers not yet submitted (before `submit_all`).
    idle: Vec<Buffer>,
}

impl BulkStream for NusbStream {
    fn submit_all(&mut self) -> Result<(), Error> {
        let Some(endpoint) = self.endpoint.as_mut() else {
            return Err(device_error::invalid_state(
                "the stream is closed".to_owned(),
            ));
        };
        for buffer in self.idle.drain(..) {
            endpoint.submit(buffer);
        }
        Ok(())
    }

    fn wait(&mut self, timeout: Duration) -> StreamEvent {
        let Some(endpoint) = self.endpoint.as_mut() else {
            return StreamEvent::Error(device_error::invalid_state(
                "the stream is closed".to_owned(),
            ));
        };
        if endpoint.pending() == 0 {
            return StreamEvent::Error(device_error::transport(
                "no transfer is pending on the bulk endpoint",
            ));
        }
        let Some(completion) = endpoint.wait_next_complete(timeout) else {
            return StreamEvent::Timeout;
        };
        let length = completion.actual_len;
        let status = completion.status;
        let buffer = completion.buffer;
        let event = match status {
            Ok(()) if length == 0 => StreamEvent::Empty,
            Ok(()) => match buffer.get(..length) {
                Some(bytes) => StreamEvent::Data(bytes.to_vec()),
                None => StreamEvent::Error(device_error::transport(format!(
                    "a transfer reported {length} bytes but holds {}",
                    buffer.len()
                ))),
            },
            // The buffers are cancelled on close; nothing arrived.
            Err(TransferError::Cancelled) => StreamEvent::Empty,
            Err(other) => {
                return StreamEvent::Error(transfer_error(other, "a bulk transfer"));
            }
        };
        endpoint.submit(buffer);
        event
    }

    fn close(&mut self, within: Duration) {
        if let Some(mut endpoint) = self.endpoint.take() {
            endpoint.cancel_all();
            let deadline = Instant::now()
                .checked_add(within)
                .unwrap_or_else(Instant::now);
            while endpoint.pending() > 0 {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                let _ = endpoint.wait_next_complete(left);
            }
        }
        self.idle.clear();
    }
}

/// The name of the driver the system bound to the device, where the platform tells (Windows).
#[cfg(windows)]
fn bound_driver(info: &nusb::DeviceInfo) -> Option<String> {
    info.driver().map(str::to_owned)
}

/// The name of the driver the system bound to the device, where the platform tells (Windows).
#[cfg(not(windows))]
fn bound_driver(_info: &nusb::DeviceInfo) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_without_a_foreign_driver_is_a_permission_problem() {
        let error = map_failure(Failure::PermissionDenied, "x", Some("WinUSB"));
        assert_eq!(error.code(), "weact.access_denied");
        let error = map_failure(Failure::PermissionDenied, "x", None);
        assert_eq!(error.code(), "weact.access_denied");
    }

    #[test]
    fn a_refusal_on_a_board_bound_to_another_driver_names_the_driver_on_windows() {
        let error = map_failure(Failure::PermissionDenied, "x", Some("libusb0"));
        if cfg!(windows) {
            assert_eq!(error.code(), "weact.driver_missing");
            assert!(error.detail().contains("libusb0"));
        } else {
            assert_eq!(error.code(), "weact.access_denied");
        }
        let error = map_failure(Failure::Unsupported, "x", Some("libusb0"));
        assert_eq!(error.code(), "weact.driver_missing");
        assert!(error.detail().contains("libusb0"));
        let error = map_failure(Failure::NotFound, "x", Some("usbccgp"));
        assert_eq!(error.code(), "weact.driver_missing");
    }

    #[test]
    fn unsupported_without_a_driver_name_is_a_missing_driver_only_on_windows() {
        let error = map_failure(Failure::Unsupported, "x", None);
        let expected = if cfg!(windows) {
            "weact.driver_missing"
        } else {
            "device.transport"
        };
        assert_eq!(error.code(), expected);
    }

    #[test]
    fn the_other_failures_map_to_their_codes() {
        assert_eq!(
            map_failure(Failure::Disconnected, "x", None).code(),
            "device.disconnected"
        );
        assert_eq!(
            map_failure(Failure::Busy, "x", Some("WinUSB")).code(),
            "weact.in_use"
        );
        assert_eq!(
            map_failure(Failure::Other, "x", None).code(),
            "device.transport"
        );
    }

    #[test]
    fn winusb_is_recognised_without_regard_to_case() {
        assert!(is_winusb("WinUSB"));
        assert!(is_winusb("winusb"));
        assert!(!is_winusb("libusb0"));
        assert!(!is_winusb("libusbK"));
        assert!(!is_winusb(""));
    }

    #[test]
    fn transfer_errors_are_typed() {
        assert_eq!(
            transfer_error(TransferError::Disconnected, "x").code(),
            "device.disconnected"
        );
        for other in [
            TransferError::Stall,
            TransferError::Fault,
            TransferError::InvalidArgument,
            TransferError::Unknown(7),
        ] {
            assert_eq!(transfer_error(other, "x").code(), "device.transport");
        }
    }

    #[test]
    fn the_error_kinds_of_the_crate_map_onto_the_failures() {
        assert_eq!(
            Failure::from(ErrorKind::Disconnected),
            Failure::Disconnected
        );
        assert_eq!(Failure::from(ErrorKind::Busy), Failure::Busy);
        assert_eq!(
            Failure::from(ErrorKind::PermissionDenied),
            Failure::PermissionDenied
        );
        assert_eq!(Failure::from(ErrorKind::Other), Failure::Other);
    }
}
