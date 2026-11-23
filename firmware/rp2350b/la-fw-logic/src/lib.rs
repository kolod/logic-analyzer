//! The logic of the RP2350B analyzer firmware, without hardware.
//!
//! Everything here is `no_std`, allocates nothing and runs on the computer in tests: the tables the board
//! reports, the handling of the host's commands, the streaming compact coding of samples and the accounting of
//! the samples lost on an overrun. The board crate (`la-fw`) wires these to the USB serial port, the PIO state
//! machines and the DMA channels. The protocol itself is `la-proto`, shared with the driver.
//!
//! Nothing here panics on what the host sends.

#![no_std]
#![deny(missing_docs)]

mod compact;
mod dispatch;
mod loss;
mod tables;

pub use compact::{CompactEncoder, Pushed};
pub use dispatch::{Dispatcher, Response};
pub use loss::LossTracker;
pub use tables::{Tables, device_info, mode_for_mask};
