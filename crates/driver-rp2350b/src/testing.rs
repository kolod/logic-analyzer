//! A scripted serial port and the recorded exchanges, so the driver is tested without a board.
//!
//! A recording is a text file, one step per line:
//!
//! ```text
//! # a comment
//! > *                    the host writes something (anything is accepted; the bytes are kept in the log)
//! > 55 aa 00 aa 55       the host writes exactly these bytes (hex)
//! < 01 02 ff             the board sends these bytes (hex)
//! <t FREQ:100000000      the board sends this text and a line feed
//! <x                     the board is unplugged: the next read fails with `device.disconnected`
//! ```
//!
//! The board sends only after the host has written: a read finds nothing while the next step is a write.

#![allow(clippy::arithmetic_side_effects)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::Result;
use la_diagnostics::lock_recover;

use crate::error;
use crate::link::{PortBackend, PortInfo, SerialLink};

/// One step of a scripted exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The host writes; `None` accepts any bytes.
    Write(Option<Vec<u8>>),
    /// The board sends these bytes.
    Send(Vec<u8>),
    /// The board is unplugged.
    Disconnect,
}

/// Reads a recording.
///
/// # Errors
///
/// A description of the first line that is not a step.
pub fn parse_recording(text: &str) -> std::result::Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |what: &str| format!("line {}: {what}", number + 1);
        if let Some(rest) = line.strip_prefix("<t") {
            let mut bytes = rest.trim_start().as_bytes().to_vec();
            bytes.push(b'\n');
            steps.push(Step::Send(bytes));
        } else if line == "<x" {
            steps.push(Step::Disconnect);
        } else if let Some(rest) = line.strip_prefix('<') {
            steps.push(Step::Send(hex(rest).ok_or_else(|| bad("not hex"))?));
        } else if let Some(rest) = line.strip_prefix('>') {
            let rest = rest.trim();
            steps.push(Step::Write(if rest == "*" {
                None
            } else {
                Some(hex(rest).ok_or_else(|| bad("not hex"))?)
            }));
        } else {
            return Err(bad("a step starts with <, <t, <x or >"));
        }
    }
    Ok(steps)
}

fn hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if digits.len() % 2 != 0 {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| {
            let high = char::from(*pair.first()?).to_digit(16)?;
            let low = char::from(*pair.get(1)?).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()
        })
        .collect()
}

/// The bytes the host wrote to a port, one entry per write, shared with the test.
type WriteLog = Arc<Mutex<Vec<Vec<u8>>>>;

/// A serial link that plays a script.
pub struct ScriptedLink {
    steps: VecDeque<Step>,
    port: String,
    log: WriteLog,
    /// The most bytes one read returns, to exercise reads in pieces.
    max_read: usize,
}

impl ScriptedLink {
    /// A link that plays `steps` and logs what the host writes.
    #[must_use]
    pub fn new(port: &str, steps: Vec<Step>, log: WriteLog) -> Self {
        Self {
            steps: steps.into(),
            port: port.to_owned(),
            log,
            max_read: usize::MAX,
        }
    }
}

impl SerialLink for ScriptedLink {
    fn read(&mut self, buf: &mut [u8], _timeout: Duration) -> Result<usize> {
        match self.steps.front_mut() {
            Some(Step::Send(bytes)) => {
                let n = bytes.len().min(buf.len()).min(self.max_read);
                if let (Some(target), Some(source)) = (buf.get_mut(..n), bytes.get(..n)) {
                    target.copy_from_slice(source);
                }
                bytes.drain(..n);
                if bytes.is_empty() {
                    self.steps.pop_front();
                }
                Ok(n)
            }
            Some(Step::Disconnect) => Err(error::disconnected(&self.port)),
            // Waiting for the host to write, or the script is over: the board is silent.
            Some(Step::Write(_)) | None => Ok(0),
        }
    }

    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        lock_recover(&self.log).push(data.to_vec());
        match self.steps.pop_front() {
            Some(Step::Write(None)) => Ok(()),
            Some(Step::Write(Some(expected))) if expected == data => Ok(()),
            other => Err(error::protocol(&format!(
                "the script did not expect this write ({} bytes); the next step was {other:?}",
                data.len()
            ))),
        }
    }
}

/// Scripted serial ports: each port has a list of scripts, and each open takes the next one.
#[derive(Default)]
pub struct ScriptedPorts {
    ports: Mutex<Vec<PortInfo>>,
    scripts: Mutex<BTreeMap<String, VecDeque<Vec<Step>>>>,
    logs: Mutex<BTreeMap<String, WriteLog>>,
    opens: Mutex<BTreeMap<String, usize>>,
    list_error: Mutex<bool>,
}

impl ScriptedPorts {
    /// No ports.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a port whose successive opens play `scripts` in order (an open with none left finds the port busy).
    pub fn add_port(&self, port: PortInfo, scripts: Vec<Vec<Step>>) {
        lock_recover(&self.scripts).insert(port.name.clone(), scripts.into());
        lock_recover(&self.logs)
            .entry(port.name.clone())
            .or_default();
        lock_recover(&self.ports).push(port);
    }

    /// Removes a port (the board was unplugged).
    pub fn remove_port(&self, name: &str) {
        lock_recover(&self.ports).retain(|p| p.name != name);
    }

    /// Makes listing fail.
    pub fn fail_listing(&self, fail: bool) {
        *lock_recover(&self.list_error) = fail;
    }

    /// Everything the host wrote to `port`, one entry per write.
    #[must_use]
    pub fn writes(&self, port: &str) -> Vec<Vec<u8>> {
        lock_recover(&self.logs)
            .get(port)
            .map(|log| lock_recover(log).clone())
            .unwrap_or_default()
    }

    /// How many times `port` was opened.
    #[must_use]
    pub fn opens(&self, port: &str) -> usize {
        lock_recover(&self.opens).get(port).copied().unwrap_or(0)
    }
}

impl PortBackend for ScriptedPorts {
    fn list(&self) -> Result<Vec<PortInfo>> {
        if *lock_recover(&self.list_error) {
            return Err(la_core::Error::new(
                la_core::ErrorKind::Device,
                la_devices::error::TRANSPORT,
                "the serial ports could not be listed".to_owned(),
            ));
        }
        Ok(lock_recover(&self.ports).clone())
    }

    fn open(&self, port: &str) -> Result<Box<dyn SerialLink>> {
        if !lock_recover(&self.ports).iter().any(|p| p.name == port) {
            return Err(error::not_found(port));
        }
        *lock_recover(&self.opens)
            .entry(port.to_owned())
            .or_insert(0) += 1;
        let steps = lock_recover(&self.scripts)
            .get_mut(port)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| error::port_busy(port, "no script is left for this open"))?;
        let log = lock_recover(&self.logs)
            .entry(port.to_owned())
            .or_default()
            .clone();
        Ok(Box::new(ScriptedLink::new(port, steps, log)))
    }
}
