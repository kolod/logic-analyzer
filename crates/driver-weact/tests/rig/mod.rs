//! Support of the opt-in rig tests: the serial link to the Arduino Nano and the skip rule.
//!
//! Nothing here is read by a test that runs without `--ignored` except the unit tests of
//! `patterns` and of the skip rule, which need no hardware.
#![allow(dead_code)]

pub mod patterns;

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use patterns::{Answer, parse_answer};

/// The environment variable that names the serial port of the Nano.
pub const PORT_VARIABLE: &str = "LA_RIG_PORT";
/// The environment variable that names the directory for recordings.
pub const RECORD_VARIABLE: &str = "LA_RIG_RECORD";
/// How long the Nano may take to greet after the port was opened (opening resets it).
pub const GREETING_WAIT: Duration = Duration::from_secs(5);

/// Prints the skip line and returns, so a test can `return`.
pub fn skip(reason: &str) {
    println!("SKIPPED: {reason}");
}

/// The port name from the environment value, or the reason to skip.
pub fn port_from(value: Option<String>) -> Result<String, String> {
    match value {
        Some(port) if !port.trim().is_empty() => Ok(port.trim().to_owned()),
        _ => Err(format!("{PORT_VARIABLE} is not set")),
    }
}

/// The Nano on its serial port.
pub struct Rig {
    port: Box<dyn serialport::SerialPort>,
}

impl Rig {
    /// Opens the port named by `LA_RIG_PORT` and waits for the greeting.
    pub fn connect() -> Result<Self, String> {
        let name = port_from(std::env::var(PORT_VARIABLE).ok())?;
        let port = serialport::new(&name, 115_200)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|error| format!("the port {name} cannot be opened: {error}"))?;
        let mut rig = Self { port };
        rig.wait_for_greeting()?;
        Ok(rig)
    }

    fn wait_for_greeting(&mut self) -> Result<(), String> {
        let started = Instant::now();
        // Opening the port resets the Nano, which then greets once; ask as well, in case it did not.
        let mut asked = false;
        while started.elapsed() < GREETING_WAIT {
            if let Some(line) = self.read_line(Duration::from_millis(300))? {
                if matches!(parse_answer(&line), Ok(Answer::Greeting(1))) {
                    return Ok(());
                }
            } else if !asked && started.elapsed() > Duration::from_secs(2) {
                asked = true;
                let _ = self.port.write_all(b"HELLO\n");
            }
        }
        Err("the rig did not greet with `LA-RIG 1` within 5 s".to_owned())
    }

    /// Reads one line (without its end), or `None` when nothing complete came within `wait`.
    pub fn read_line(&mut self, wait: Duration) -> Result<Option<String>, String> {
        let started = Instant::now();
        let mut line = Vec::new();
        let mut byte = [0_u8; 1];
        while started.elapsed() < wait {
            match self.port.read(&mut byte) {
                Ok(1) if byte[0] == b'\n' => {
                    return Ok(Some(String::from_utf8_lossy(&line).trim().to_owned()));
                }
                Ok(1) => line.push(byte[0]),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
                Err(error) => return Err(format!("the serial port failed: {error}")),
            }
        }
        Ok(None)
    }

    /// Sends a command and returns its answer.
    pub fn command(&mut self, text: &str) -> Result<Answer, String> {
        self.port
            .write_all(format!("{text}\n").as_bytes())
            .map_err(|error| format!("the serial port failed: {error}"))?;
        for _ in 0..20 {
            match self.read_line(Duration::from_millis(500))? {
                // A late greeting from the reset is not the answer.
                Some(line) => match parse_answer(&line)? {
                    Answer::Greeting(_) if text != "HELLO" => {}
                    answer => return Ok(answer),
                },
                None => return Err(format!("no answer to `{text}`")),
            }
        }
        Err(format!("no answer to `{text}`"))
    }

    /// Sends a command that must be answered `OK`.
    pub fn expect_ok(&mut self, text: &str) -> Result<(), String> {
        match self.command(text)? {
            Answer::Ok => Ok(()),
            other => Err(format!("`{text}` was answered {other:?}")),
        }
    }
}

/// Stops the pattern when dropped, also after a failing test.
pub struct Guard {
    pub rig: Rig,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.rig.command("STOP");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skip_rule_needs_a_port() {
        assert!(port_from(None).unwrap_err().contains(PORT_VARIABLE));
        assert!(port_from(Some(String::new())).is_err());
        assert!(port_from(Some("  ".to_owned())).is_err());
        assert_eq!(port_from(Some(" COM7 ".to_owned())).unwrap(), "COM7");
        assert_eq!(
            port_from(Some("/dev/ttyUSB0".to_owned())).unwrap(),
            "/dev/ttyUSB0"
        );
    }
}
