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
pub mod chain;
mod device;
pub mod error;
pub mod link;
mod serial_backend;
#[cfg(feature = "test-support")]
pub mod testing;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::Result;
use la_devices::{Device, DeviceAction, DeviceDescriptor, DeviceId, DeviceProvider};
use la_diagnostics::lock_recover;
use la_proto::{DeviceInfo, command, encode_frame};

pub use capabilities::{
    CHANNELS, MIN_RATE_HZ, build_request, capabilities, capabilities_unsupported, check_supported,
    unsupported_reason,
};
pub use chain::{ChainDefinition, ChainDevice, MAX_BOARDS};
pub use device::Rp2350bDevice;
use link::{PortBackend, PortInfo, SerialLink};
pub use serial_backend::SystemPorts;

/// The name of the provider and the prefix of every device identity of this driver.
pub const PROVIDER_NAME: &str = "rp2350b";

/// The USB vendor id the firmware presents. For development this is the default of the Pico SDK (Raspberry Pi);
/// a pair of our own is registered before the first release (`research.md`, R13).
pub const USB_VID: u16 = 0x2E8A;
/// The USB product id the firmware presents: the Pico SDK's default for a USB serial device. Any other
/// firmware built on the SDK with the same default looks the same on the bus, so the driver asks each such
/// port for the device information and lists only the ones that answer as this analyzer (R13).
pub const USB_PID: u16 = 0x000A;

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
    /// What the board reported.
    info: DeviceInfo,
    /// What the board reported, when its firmware cannot be used: listed so that it can be restarted into its
    /// bootloader and updated, refused when a capture is started.
    unsupported: Option<(String, &'static str)>,
}

/// The provider of the RP2350B boards on the serial ports.
pub struct Rp2350bProvider {
    backend: Arc<dyn PortBackend>,
    boards: Mutex<Vec<Board>>,
    /// The chains the user defined.
    definitions: Mutex<Vec<ChainDefinition>>,
    /// The chains whose boards are all present, from the last listing.
    chains: Mutex<Vec<ChainEntry>>,
}

/// A chain that is listed.
struct ChainEntry {
    definition: ChainDefinition,
    descriptor: DeviceDescriptor,
    boards: Vec<DeviceId>,
    /// The boards run different firmware: listed, refused when a capture is started.
    mismatch: Option<String>,
}

impl Rp2350bProvider {
    /// A provider over the given ports (tests pass the scripted ones).
    #[must_use]
    pub fn with_backend(backend: Arc<dyn PortBackend>) -> Self {
        Self {
            backend,
            boards: Mutex::new(Vec::new()),
            definitions: Mutex::new(Vec::new()),
            chains: Mutex::new(Vec::new()),
        }
    }

    /// Sets the chains the user defined. A chain is listed while all its boards are present and supported; one that
    /// cannot be used (fewer than 2 or more than 5 boards, a board twice, no name, a name used before, a board that
    /// is already in an earlier chain) is left out.
    pub fn set_definitions(&self, chains: Vec<ChainDefinition>) {
        let mut kept: Vec<ChainDefinition> = Vec::new();
        for chain in chains {
            let clash = kept
                .iter()
                .any(|k| k.name == chain.name || k.boards.iter().any(|b| chain.boards.contains(b)));
            if chain.is_valid() && !clash {
                kept.push(chain);
            }
        }
        *lock_recover(&self.definitions) = kept;
    }

    /// The chains of the last listing whose boards are all present, from the definitions.
    fn list_chains(&self, boards: &[Board]) -> Vec<ChainEntry> {
        let mut entries = Vec::new();
        for definition in lock_recover(&self.definitions).iter() {
            let members: Vec<&Board> = definition
                .boards
                .iter()
                .filter_map(|text| boards.iter().find(|b| b.id.as_str() == text))
                .collect();
            if members.len() != definition.boards.len() {
                tracing::debug!(
                    target: "la_driver_rp2350b",
                    chain = %definition.name,
                    "a chain is not listed: a board is missing"
                );
                continue;
            }
            if members.iter().any(|b| b.unsupported.is_some()) {
                tracing::warn!(
                    target: "la_driver_rp2350b",
                    chain = %definition.name,
                    "a chain is not listed: a board runs a firmware this program does not support"
                );
                continue;
            }
            let versions: Vec<String> = members
                .iter()
                .map(|b| {
                    let (a, c, d) = b.info.firmware.unwrap_or((0, 0, 0));
                    format!("{}: {a}.{c}.{d}", b.id)
                })
                .collect();
            let same = members.windows(2).all(|pair| {
                pair.first().map(|b| b.info.firmware) == pair.get(1).map(|b| b.info.firmware)
            });
            let infos: Vec<&DeviceInfo> = members.iter().map(|b| &b.info).collect();
            let descriptor = DeviceDescriptor {
                id: definition.device_id(),
                name: format!(
                    "RP2350B chain {} ({} boards)",
                    definition.name,
                    members.len()
                ),
                simulated: false,
                capabilities: chain::chain_capabilities(&infos),
            };
            entries.push(ChainEntry {
                definition: definition.clone(),
                descriptor,
                boards: members.iter().map(|b| b.id.clone()).collect(),
                mismatch: (!same).then(|| versions.join(", ")),
            });
        }
        entries
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
            capabilities: if unsupported_reason(info).is_none() {
                capabilities(info)
            } else {
                capabilities_unsupported(info)
            },
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
                .map(|b| {
                    (
                        b.id.clone(),
                        b.port.clone(),
                        b.descriptor.clone(),
                        b.info.clone(),
                        b.unsupported.clone(),
                    )
                });
            if let Some((id, port, descriptor, info, unsupported)) = known {
                found.push(Board {
                    id,
                    port,
                    descriptor,
                    info,
                    unsupported,
                });
                continue;
            }
            match self.probe(port) {
                Ok(info) => {
                    let descriptor = Self::describe(port, &info);
                    let unsupported = unsupported_reason(&info);
                    if let Some((found, _)) = &unsupported {
                        tracing::warn!(
                            target: "la_driver_rp2350b",
                            port = %port.name,
                            found = %found,
                            "a board is listed with a firmware this program does not support"
                        );
                    }
                    found.push(Board {
                        id: descriptor.id.clone(),
                        port: port.name.clone(),
                        descriptor,
                        info,
                        unsupported,
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
        let chains = self.list_chains(&found);
        let mut descriptors: Vec<DeviceDescriptor> =
            found.iter().map(|b| b.descriptor.clone()).collect();
        descriptors.extend(chains.iter().map(|c| c.descriptor.clone()));
        *lock_recover(&self.boards) = found;
        *lock_recover(&self.chains) = chains;
        Ok(descriptors)
    }

    fn set_chains(&self, chains: &[la_devices::DeviceChain]) {
        // Only chains of boards of this driver are ours.
        let ours = chains
            .iter()
            .filter(|c| c.boards.iter().all(|b| b.provider() == PROVIDER_NAME))
            .map(|c| ChainDefinition {
                name: c.name.clone(),
                boards: c.boards.iter().map(|b| b.as_str().to_owned()).collect(),
                shifts: c.shifts.clone(),
            })
            .collect();
        self.set_definitions(ours);
    }

    fn perform(&self, id: &DeviceId, action: DeviceAction) -> Result<()> {
        let port = lock_recover(&self.boards)
            .iter()
            .find(|b| b.id == *id)
            .map(|b| b.port.clone())
            .ok_or_else(|| la_devices::error::not_found(id.as_str()))?;
        let mut link = self.backend.open(&port)?;
        let (cmd, expected) = match action {
            DeviceAction::Restart => (command::RESTART, "RESTARTING"),
            DeviceAction::RestartIntoBootloader => (command::BOOTLOADER, "RESTARTING_BOOTLOADER"),
        };
        let mut frame = [0_u8; 8];
        let n = encode_frame(&[cmd], &mut frame)
            .map_err(|e| error::protocol(&format!("the command could not be framed: {e}")))?;
        link.write_all(frame.get(..n).unwrap_or(&[]))?;
        let answer = device::read_line(&mut *link, "waiting for the answer to a restart")?;
        if answer != expected {
            return Err(if answer.starts_with("ERR_") {
                error::request_refused(&answer)
            } else {
                error::protocol("the answer to a restart is not the one the contract names")
            });
        }
        // The board is going away: forget it, so that it is asked again when it comes back.
        lock_recover(&self.boards).retain(|b| b.id != *id);
        Ok(())
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        if let Some(chain) = lock_recover(&self.chains)
            .iter()
            .find(|c| c.descriptor.id == *id)
            .map(|c| {
                (
                    c.definition.clone(),
                    c.descriptor.clone(),
                    c.boards.clone(),
                    c.mismatch.clone(),
                )
            })
        {
            return self.open_chain(chain);
        }
        let (port, descriptor, unsupported) = lock_recover(&self.boards)
            .iter()
            .find(|b| b.id == *id)
            .map(|b| (b.port.clone(), b.descriptor.clone(), b.unsupported.clone()))
            .ok_or_else(|| la_devices::error::not_found(id.as_str()))?;
        if let Some((found, supported)) = unsupported {
            return Err(error::firmware_unsupported(&found, supported));
        }
        let link = self.backend.open(&port)?;
        Ok(Box::new(Rp2350bDevice::new(descriptor, port, link)))
    }
}

/// What `open_chain` needs of a listed chain.
type OpenChain = (
    ChainDefinition,
    DeviceDescriptor,
    Vec<DeviceId>,
    Option<String>,
);

impl Rp2350bProvider {
    /// Opens every board of a chain. A board that went away or cannot be opened makes the chain incomplete,
    /// naming it; boards of different firmware are refused.
    fn open_chain(&self, chain: OpenChain) -> Result<Box<dyn Device>> {
        let (definition, descriptor, ids, mismatch) = chain;
        if let Some(found) = mismatch {
            return Err(error::chain_mismatch(&definition.name, &found));
        }
        let mut boards = Vec::with_capacity(ids.len());
        for id in &ids {
            let known = lock_recover(&self.boards)
                .iter()
                .find(|b| b.id == *id)
                .map(|b| (b.port.clone(), b.descriptor.clone()));
            let Some((port, board_descriptor)) = known else {
                return Err(error::chain_incomplete(
                    &definition.name,
                    id.as_str(),
                    "is not there any more",
                ));
            };
            let link = self.backend.open(&port).map_err(|e| {
                error::chain_incomplete(
                    &definition.name,
                    id.as_str(),
                    &format!("could not be opened: {}", e.detail()),
                )
            })?;
            boards.push((
                id.as_str().to_owned(),
                Rp2350bDevice::new(board_descriptor, port, link),
            ));
        }
        Ok(Box::new(ChainDevice::new(
            descriptor,
            definition.name,
            boards,
            definition.shifts,
        )))
    }
}
