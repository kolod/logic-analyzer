//! The seam between the driver and USB access, scriptable by the tests.
//!
//! The driver never touches the USB library directly: it talks to a [`UsbBackend`] (what is on
//! the bus, and how to open a board) and a [`UsbLink`] (control transfers and the bulk stream of
//! an open board). The real implementation is `nusb_backend`; the tests use
//! [`crate::testing::RecordedUsb`]. Time goes through a [`Timer`] for the same reason.
//!
//! All methods return typed [`la_core::Error`]s and none of them may panic.

use std::fmt;
use std::time::{Duration, Instant};

use la_core::Error;
use la_devices::DeviceId;

use crate::protocol::BufferPlan;

/// Where a board is on the USB bus: the bus identifier as text and the chain of port numbers from
/// the root hub. It is the identity of a board: it does not change when the board renumerates
/// (the device address does) and it differs for two boards plugged in at once.
///
/// The bus identifier is text because the platforms differ (on Windows the path of the host
/// controller, on Linux the bus number). The key is built and compared as a whole; nothing parses
/// it back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoardKey {
    bus: String,
    ports: Vec<u8>,
}

impl BoardKey {
    /// A key from the bus identifier and the port chain.
    #[must_use]
    pub fn new(bus: impl Into<String>, ports: impl Into<Vec<u8>>) -> Self {
        Self {
            bus: bus.into(),
            ports: ports.into(),
        }
    }

    /// The bus identifier as the platform gives it.
    #[must_use]
    pub fn bus(&self) -> &str {
        &self.bus
    }

    /// The port numbers from the root hub to the board.
    #[must_use]
    pub fn ports(&self) -> &[u8] {
        &self.ports
    }

    /// The identity of the board in the device list: `weact:<bus>-<p1>.<p2>…`.
    #[must_use]
    pub fn device_id(&self) -> DeviceId {
        DeviceId::new(crate::PROVIDER_NAME, &self.to_string())
    }
}

impl fmt::Display for BoardKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-", self.bus)?;
        for (index, port) in self.ports.iter().enumerate() {
            if index > 0 {
                f.write_str(".")?;
            }
            write!(f, "{port}")?;
        }
        Ok(())
    }
}

/// What a scan reads about a device without opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbInfo {
    /// Where it is on the bus.
    pub key: BoardKey,
    /// USB vendor ID.
    pub vendor_id: u16,
    /// USB product ID.
    pub product_id: u16,
    /// The manufacturer string, if the device has one and the system knows it.
    pub manufacturer: Option<String>,
    /// The product string, if the device has one and the system knows it.
    pub product: Option<String>,
    /// The address on the bus; it changes when the board renumerates.
    pub address: u8,
    /// The name of the driver bound to it, where the platform gives it (Windows: `WinUSB`,
    /// `libusb0`, ...).
    pub driver: Option<String>,
}

impl UsbInfo {
    /// Whether this is a board of this driver (the vendor and product IDs).
    #[must_use]
    pub fn is_weact(&self) -> bool {
        self.vendor_id == crate::protocol::VENDOR_ID
            && self.product_id == crate::protocol::PRODUCT_ID
    }

    /// Whether the board runs the firmware: its strings are `sigrok` and `fx2lafw`. A board
    /// without firmware has other strings or none.
    #[must_use]
    pub fn runs_firmware(&self) -> bool {
        self.manufacturer.as_deref() == Some(crate::protocol::MANUFACTURER)
            && self.product.as_deref() == Some(crate::protocol::PRODUCT)
    }
}

/// What the bulk stream delivers.
#[derive(Debug)]
pub enum StreamEvent {
    /// A completed transfer with its bytes (at least one).
    Data(Vec<u8>),
    /// A completed transfer with no bytes: the board had nothing to send.
    Empty,
    /// Nothing completed within the wait.
    Timeout,
    /// A transfer failed; the error says how (`device.disconnected`, `device.transport`, ...).
    Error(Error),
}

/// The finding of the bus and the opening of boards.
pub trait UsbBackend: Send + Sync {
    /// Lists the devices on the bus. Must return promptly.
    ///
    /// # Errors
    ///
    /// `device.transport` when the bus cannot be read.
    fn list(&self) -> Result<Vec<UsbInfo>, Error>;

    /// Opens the board at `key`.
    ///
    /// # Errors
    ///
    /// `weact.access_denied`, `weact.driver_missing`, `weact.in_use`, `device.disconnected` or
    /// `device.transport`, as the system's refusal says.
    fn open(&self, key: &BoardKey) -> Result<Box<dyn UsbLink>, Error>;
}

/// An open board: control transfers and the bulk stream.
pub trait UsbLink: Send {
    /// A vendor control transfer to the device that reads up to `length` bytes.
    ///
    /// # Errors
    ///
    /// A typed device error.
    fn control_in(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        timeout: Duration,
    ) -> Result<Vec<u8>, Error>;

    /// A vendor control transfer to the device that writes `data`.
    ///
    /// # Errors
    ///
    /// A typed device error.
    fn control_out(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Claims the interface and opens the bulk IN endpoint with the buffers of `plan`. No
    /// transfer is submitted yet.
    ///
    /// # Errors
    ///
    /// `weact.in_use` when another program holds the interface, or a typed device error.
    fn open_stream(&mut self, plan: BufferPlan) -> Result<Box<dyn BulkStream>, Error>;
}

/// The bulk IN stream of an open board.
pub trait BulkStream: Send {
    /// Submits every buffer. Called before the start command is sent.
    ///
    /// # Errors
    ///
    /// A typed device error.
    fn submit_all(&mut self) -> Result<(), Error>;

    /// Waits up to `timeout` for a transfer to complete; a buffer that delivered data is
    /// submitted again.
    fn wait(&mut self, timeout: Duration) -> StreamEvent;

    /// Cancels every transfer, waits up to `within` for them to return and releases the
    /// interface. Safe to call more than once.
    fn close(&mut self, within: Duration);
}

/// Time as the driver uses it: a clock and a sleep, so that tests control both.
pub trait Timer: Send + Sync {
    /// Time since an arbitrary fixed start; never goes backwards.
    fn now(&self) -> Duration;
    /// Waits `duration`.
    fn sleep(&self, duration: Duration);
}

/// The real time.
#[derive(Debug)]
pub struct SystemTimer {
    start: Instant,
}

impl SystemTimer {
    /// A timer that counts from now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer for SystemTimer {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_written_as_the_bus_and_the_port_chain() {
        let key = BoardKey::new("PCIROOT(0)#PCI(1400)#USBROOT(0)", [13, 2]);
        assert_eq!(key.to_string(), "PCIROOT(0)#PCI(1400)#USBROOT(0)-13.2");
        assert_eq!(BoardKey::new("001", [4]).to_string(), "001-4");
        assert_eq!(BoardKey::new("001", []).to_string(), "001-");
    }

    #[test]
    fn the_identity_is_the_provider_name_and_the_key() {
        let id = BoardKey::new("001", [1, 2, 3]).device_id();
        assert_eq!(id.as_str(), "weact:001-1.2.3");
    }

    #[test]
    fn keys_of_two_ports_differ_and_a_key_equals_itself() {
        let a = BoardKey::new("001", [1]);
        let b = BoardKey::new("001", [2]);
        let c = BoardKey::new("002", [1]);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, BoardKey::new("001", vec![1]));
        assert_ne!(a.device_id(), b.device_id());
    }

    #[test]
    fn hostile_text_in_a_key_never_breaks_anything() {
        let long = "x".repeat(10_000);
        for bus in ["", "-", ":", "a:b-c.d", "é\u{0}\u{202e}", long.as_str()] {
            let key = BoardKey::new(bus, [255, 0, 1]);
            let id = key.device_id();
            assert!(id.as_str().starts_with("weact:"));
            assert_eq!(id, key.device_id());
        }
        let deep = BoardKey::new("001", vec![7_u8; 255]);
        assert!(deep.to_string().len() > 255);
    }

    #[test]
    fn a_board_runs_the_firmware_only_with_both_strings() {
        let mut info = UsbInfo {
            key: BoardKey::new("001", [1]),
            vendor_id: 0x1d50,
            product_id: 0x608c,
            manufacturer: None,
            product: None,
            address: 5,
            driver: None,
        };
        assert!(info.is_weact());
        assert!(!info.runs_firmware(), "no strings: blank");
        info.manufacturer = Some("sigrok".to_owned());
        assert!(!info.runs_firmware(), "one string is not enough");
        info.product = Some("fx2lafw".to_owned());
        assert!(info.runs_firmware());
        info.product = Some("FX2LAFW".to_owned());
        assert!(!info.runs_firmware(), "the strings are compared exactly");
        info.vendor_id = 0x04b4;
        assert!(!info.is_weact());
    }

    #[test]
    fn the_system_timer_does_not_go_backwards() {
        let timer = SystemTimer::new();
        let first = timer.now();
        timer.sleep(Duration::from_millis(2));
        assert!(timer.now() >= first + Duration::from_millis(1));
    }
}
