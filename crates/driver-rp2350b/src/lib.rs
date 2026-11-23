//! Driver for the 32-channel RP2350B logic analyzer.
//!
//! The board runs the firmware of this repository and speaks the protocol of `la-proto` over a USB serial
//! port. This crate implements the device contract of `la-devices` for it: it lists the boards by the USB ids
//! of the firmware, reads each board's information to declare its capabilities, starts a capture with a
//! request, returns the samples in pieces without blocking, and aborts on request. All serial access goes
//! through the seam of [`link`], so the driver is tested with recorded exchanges and no board.
//!
//! The contracts are in `specs/017-driver-rp2350b-32ch/contracts/`.

#![deny(missing_docs)]

mod capabilities;
mod device;
pub mod error;
pub mod link;
mod serial_backend;
#[cfg(feature = "test-support")]
pub mod testing;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::Result;
use la_devices::{Device, DeviceDescriptor, DeviceId, DeviceProvider};
use la_diagnostics::lock_recover;
use la_proto::{DeviceInfo, command, encode_frame};

pub use capabilities::{CHANNELS, MIN_RATE_HZ, build_request, capabilities, check_supported};
pub use device::Rp2350bDevice;
use link::{PortBackend, PortInfo, SerialLink};
pub use serial_backend::SystemPorts;

/// The name of the provider and the prefix of every device identity of this driver.
pub const PROVIDER_NAME: &str = "rp2350b";

/// The USB vendor id the firmware presents (Raspberry Pi).
pub const USB_VID: u16 = 0x2E8A;
/// The USB product id the firmware presents. Provisional: the firmware of this repository will set it, and
/// the value is to be confirmed before the first release.
pub const USB_PID: u16 = 0x4C41;

/// How long a board has to start answering the information request before it is not listed.
const PROBE_WAIT: Duration = Duration::from_millis(500);
/// How long a read waits while probing.
const PROBE_STEP: Duration = Duration::from_millis(50);
/// Two quiet steps after the first byte end the information reply.
const QUIET_STEPS: u32 = 2;
/// The most bytes of an information reply.
const MAX_INFO: usize = 4096;

/// A board the provider has found.
struct Board {
    id: DeviceId,
    port: String,
    descriptor: DeviceDescriptor,
}

/// The provider of the RP2350B boards on the serial ports.
pub struct Rp2350bProvider {
    backend: Arc<dyn PortBackend>,
    boards: Mutex<Vec<Board>>,
}

impl Rp2350bProvider {
    /// A provider over the given ports (tests pass the scripted ones).
    #[must_use]
    pub fn with_backend(backend: Arc<dyn PortBackend>) -> Self {
        Self {
            backend,
            boards: Mutex::new(Vec::new()),
        }
    }

    /// Asks the board at `port` for its information. Never waits longer than [`PROBE_WAIT`] for a port that
    /// stays silent.
    fn probe(&self, port: &PortInfo) -> Result<DeviceInfo> {
        let mut link = self.backend.open(&port.name)?;
        let mut frame = [0_u8; 8];
        let n = encode_frame(&[command::INFO], &mut frame).map_err(|e| {
            error::protocol(&format!("the information request could not be framed: {e}"))
        })?;
        link.write_all(frame.get(..n).unwrap_or(&[]))?;
        let text = read_reply(&mut *link)?;
        let info = DeviceInfo::parse(&text)
            .map_err(|e| error::protocol(&format!("the information reply is not valid: {e}")))?;
        check_supported(&info)?;
        Ok(info)
    }

    fn describe(port: &PortInfo, info: &DeviceInfo) -> DeviceDescriptor {
        let unique = port
            .serial
            .clone()
            .unwrap_or_else(|| format!("{:04x}-{:04x}-{}", port.vid, port.pid, port.name));
        DeviceDescriptor {
            id: DeviceId::new(PROVIDER_NAME, &unique),
            name: port.serial.as_ref().map_or_else(
                || "RP2350B logic analyzer".to_owned(),
                |serial| format!("RP2350B logic analyzer {serial}"),
            ),
            simulated: false,
            capabilities: capabilities(info),
        }
    }
}

/// Reads the information reply: until the board has been quiet for two steps after its first byte.
fn read_reply(link: &mut dyn SerialLink) -> Result<String> {
    let mut data = Vec::new();
    let (mut waited, mut quiet) = (Duration::ZERO, 0_u32);
    let mut buffer = [0_u8; 512];
    loop {
        let n = link.read(&mut buffer, PROBE_STEP)?;
        if n == 0 {
            waited = waited.saturating_add(PROBE_STEP);
            if data.is_empty() {
                if waited >= PROBE_WAIT {
                    return Err(error::timeout(
                        "waiting for the device information",
                        u64::try_from(PROBE_WAIT.as_millis()).unwrap_or(u64::MAX),
                    ));
                }
            } else {
                quiet = quiet.saturating_add(1);
                if quiet >= QUIET_STEPS {
                    break;
                }
            }
            continue;
        }
        quiet = 0;
        data.extend_from_slice(buffer.get(..n).unwrap_or(&[]));
        if data.len() > MAX_INFO {
            return Err(error::protocol("the information reply is too long"));
        }
    }
    String::from_utf8(data).map_err(|_| error::protocol("the information reply is not text"))
}

/// The provider of the real serial ports, for the application to register.
#[must_use]
pub fn provider() -> Arc<Rp2350bProvider> {
    Arc::new(Rp2350bProvider::with_backend(Arc::new(SystemPorts)))
}

impl DeviceProvider for Rp2350bProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>> {
        let listed: Vec<PortInfo> = self
            .backend
            .list()?
            .into_iter()
            .filter(|p| p.vid == USB_VID && p.pid == USB_PID)
            .collect();
        let mut found = Vec::new();
        for port in &listed {
            // A board that is already known stays as it is: asking it again could disturb a capture that
            // holds its port.
            let known = lock_recover(&self.boards)
                .iter()
                .find(|b| b.port == port.name)
                .map(|b| (b.id.clone(), b.port.clone(), b.descriptor.clone()));
            if let Some((id, port, descriptor)) = known {
                found.push(Board {
                    id,
                    port,
                    descriptor,
                });
                continue;
            }
            match self.probe(port) {
                Ok(info) => {
                    let descriptor = Self::describe(port, &info);
                    found.push(Board {
                        id: descriptor.id.clone(),
                        port: port.name.clone(),
                        descriptor,
                    });
                }
                Err(error) => tracing::warn!(
                    target: "la_driver_rp2350b",
                    port = %port.name,
                    code = error.code(),
                    detail = error.detail(),
                    "a serial port with the ids of the analyzer is not listed"
                ),
            }
        }
        let descriptors = found.iter().map(|b| b.descriptor.clone()).collect();
        *lock_recover(&self.boards) = found;
        Ok(descriptors)
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        let (port, descriptor) = lock_recover(&self.boards)
            .iter()
            .find(|b| b.id == *id)
            .map(|b| (b.port.clone(), b.descriptor.clone()))
            .ok_or_else(|| la_devices::error::not_found(id.as_str()))?;
        let link = self.backend.open(&port)?;
        Ok(Box::new(Rp2350bDevice::new(descriptor, port, link)))
    }
}
