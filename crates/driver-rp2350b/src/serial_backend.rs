//! The real serial ports, through the `serialport` crate.

use std::io::{ErrorKind as IoKind, Read, Write};
use std::time::Duration;

use la_core::{Error, ErrorKind, Result};
use serialport::{ErrorKind as PortKind, SerialPort, SerialPortType};

use crate::error;
use crate::link::{PortBackend, PortInfo, SerialLink};

/// The system's serial ports.
pub struct SystemPorts;

impl PortBackend for SystemPorts {
    fn list(&self) -> Result<Vec<PortInfo>> {
        let ports = serialport::available_ports().map_err(|e| {
            Error::new(
                ErrorKind::Device,
                la_devices::error::TRANSPORT,
                format!("the serial ports could not be listed: {e}"),
            )
        })?;
        Ok(ports
            .into_iter()
            .filter_map(|port| match port.port_type {
                SerialPortType::UsbPort(usb) => Some(PortInfo {
                    name: port.port_name,
                    vid: usb.vid,
                    pid: usb.pid,
                    serial: usb.serial_number,
                }),
                _ => None,
            })
            .collect())
    }

    fn open(&self, port: &str) -> Result<Box<dyn SerialLink>> {
        // The baud rate means nothing on a USB serial port.
        let opened = serialport::new(port, 115_200)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| match e.kind() {
                PortKind::NoDevice => error::not_found(port),
                PortKind::Io(IoKind::PermissionDenied) => {
                    // Windows says "access denied" when another program holds the port.
                    if cfg!(windows) {
                        error::port_busy(port, &e.description)
                    } else {
                        error::access_denied(port, &e.description)
                    }
                }
                PortKind::Io(IoKind::NotFound) => error::not_found(port),
                _ => error::port_busy(port, &e.description),
            })?;
        Ok(Box::new(Link {
            port: opened,
            name: port.to_owned(),
        }))
    }
}

struct Link {
    port: Box<dyn SerialPort>,
    name: String,
}

impl Link {
    fn map(&self, e: &std::io::Error) -> Error {
        match e.kind() {
            IoKind::BrokenPipe | IoKind::NotConnected | IoKind::UnexpectedEof => {
                error::disconnected(&self.name)
            }
            _ => Error::new(
                ErrorKind::Device,
                la_devices::error::TRANSPORT,
                format!("the serial port {} failed: {e}", self.name),
            ),
        }
    }
}

impl SerialLink for Link {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        if self.port.set_timeout(timeout).is_err() {
            return Err(error::disconnected(&self.name));
        }
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == IoKind::TimedOut || e.kind() == IoKind::WouldBlock => Ok(0),
            Err(e) => Err(self.map(&e)),
        }
    }

    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.port
            .write_all(data)
            .and_then(|()| self.port.flush())
            .map_err(|e| self.map(&e))
    }
}
