//! What the board does with a frame from the host.

use la_proto::memory::{Chosen, MemoryLimits, choose};
use la_proto::{CaptureRequest2, DeviceInfo, Event, command};

/// What the board has to do after a frame. Lines are sent as text followed by a line feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Nothing.
    None,
    /// Send the device information.
    Info,
    /// Send this line.
    Line(&'static str),
    /// Send `CAPTURE_ERROR`, then the name of the field.
    CaptureError(&'static str),
    /// Send `CAPTURE_STARTED` and start this capture, into this memory.
    StartCapture {
        /// The request, already checked against the limits of the board.
        request: CaptureRequest2,
        /// The memory the capture goes to.
        memory: Chosen,
    },
    /// Send `RESTARTING_BOOTLOADER`, flush, then restart into the bootloader.
    Bootloader,
    /// Send `RESTARTING`, flush, then restart.
    Restart,
    /// Switch the indicator.
    Indicator(bool),
}

/// The state of the command handling: whether a capture runs.
#[derive(Debug, Default)]
pub struct Dispatcher {
    busy: bool,
}

impl Dispatcher {
    /// A board with no capture running.
    #[must_use]
    pub const fn new() -> Self {
        Self { busy: false }
    }

    /// Whether a capture is running (or its data is still being sent).
    #[must_use]
    pub const fn busy(&self) -> bool {
        self.busy
    }

    /// The capture ended and its data was sent: the board takes requests again.
    pub fn capture_done(&mut self) {
        self.busy = false;
    }

    /// Handles what the frame parser found. Every input has an answer; none of them stops the board.
    pub fn handle(&mut self, event: Event<'_>, info: &DeviceInfo) -> Response {
        match event {
            Event::Overflow => Response::Line("ERR_MSG_OVERFLOW"),
            Event::Damaged => Response::Line("ERR_BAD_REQUEST"),
            Event::Frame(body) => {
                let Some((&cmd, payload)) = body.split_first() else {
                    return Response::Line("ERR_UNKNOWN_MSG");
                };
                self.command(cmd, payload, info)
            }
        }
    }

    fn command(&mut self, cmd: u8, payload: &[u8], info: &DeviceInfo) -> Response {
        // The restarts are accepted at any time, also while a capture runs.
        match cmd {
            command::BOOTLOADER if payload.is_empty() => return Response::Bootloader,
            command::RESTART if payload.is_empty() => return Response::Restart,
            command::BOOTLOADER | command::RESTART => return Response::Line("ERR_BAD_REQUEST"),
            _ => {}
        }
        match cmd {
            command::INFO | command::CAPTURE | command::CAPTURE2 if self.busy => {
                Response::Line("ERR_BUSY")
            }
            command::INFO if payload.is_empty() => Response::Info,
            command::INFO => Response::Line("ERR_BAD_REQUEST"),
            command::NETWORK | command::POWER | command::CAPTURE => {
                Response::Line("ERR_UNSUPPORTED")
            }
            command::INDICATOR_ON | command::INDICATOR_OFF => {
                if payload.is_empty() {
                    Response::Indicator(cmd == command::INDICATOR_ON)
                } else {
                    Response::Line("ERR_BAD_REQUEST")
                }
            }
            command::CAPTURE2 => match CaptureRequest2::from_bytes(payload) {
                Err(_) => Response::Line("ERR_BAD_REQUEST"),
                Ok(request) => match info.check_request(&request) {
                    Err(field) => Response::CaptureError(field.name()),
                    Ok(()) => match choose(&request, &MemoryLimits::from_info(info)) {
                        Err(_) => Response::Line("ERR_NO_MEMORY"),
                        Ok(memory) => {
                            self.busy = true;
                            Response::StartCapture { request, memory }
                        }
                    },
                },
            },
            _ => Response::Line("ERR_UNKNOWN_MSG"),
        }
    }
}
