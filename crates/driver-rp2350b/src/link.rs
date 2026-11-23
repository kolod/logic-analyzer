//! The seam between the driver and the serial ports: the driver is written against these two traits, so the
//! protocol code is tested with recorded exchanges and without a board.

use std::time::Duration;

use la_core::Result;

/// A serial port the system lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// The system's name of the port (`COM5`, `/dev/ttyACM0`).
    pub name: String,
    /// The USB vendor id.
    pub vid: u16,
    /// The USB product id.
    pub pid: u16,
    /// The USB serial number string, when the port has one (the chip's serial number on this board).
    pub serial: Option<String>,
}

/// One open serial port.
pub trait SerialLink: Send {
    /// Reads up to `buf.len()` bytes, waiting at most `timeout`. Zero means nothing arrived in time.
    ///
    /// # Errors
    ///
    /// `device.disconnected` when the board went away, or another typed `device.*` error.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize>;

    /// Writes all of `data`.
    ///
    /// # Errors
    ///
    /// `device.disconnected` when the board went away, or another typed `device.*` error.
    fn write_all(&mut self, data: &[u8]) -> Result<()>;
}

/// The system's serial ports.
pub trait PortBackend: Send + Sync {
    /// The ports the system lists now (all of them; the driver picks its own by the USB ids).
    ///
    /// # Errors
    ///
    /// A typed `device.*` error when the ports cannot be listed.
    fn list(&self) -> Result<Vec<PortInfo>>;

    /// Opens `port`.
    ///
    /// # Errors
    ///
    /// `rp2350b.access_denied`, `rp2350b.port_busy` or `rp2350b.not_found`.
    fn open(&self, port: &str) -> Result<Box<dyn SerialLink>>;
}
