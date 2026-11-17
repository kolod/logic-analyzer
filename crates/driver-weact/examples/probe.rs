//! Lists every USB device the OS shows, with what `nusb` can read without opening it.
//!
//! A diagnostic for the person setting up a board: it shows whether the WeAct board
//! (`1d50:608c`) is there, what its strings are (a board without firmware has other strings than
//! `sigrok` / `fx2lafw`) and, where the platform gives it, which driver is bound (WinUSB is
//! needed on Windows). Run with `cargo run -p la-driver-weact --example probe`.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command line diagnostic"
)]

use nusb::MaybeFuture;

fn main() {
    let devices = match nusb::list_devices().wait() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("the USB bus could not be listed: {error}");
            std::process::exit(1);
        }
    };
    for device in devices {
        println!(
            "{:04x}:{:04x} rev {:04x} class {:02x} | manufacturer {:?} | product {:?} | serial {:?} | bus {:?} address {} | ports {:?} | driver {:?}",
            device.vendor_id(),
            device.product_id(),
            device.device_version(),
            device.class(),
            device.manufacturer_string(),
            device.product_string(),
            device.serial_number(),
            device.bus_id(),
            device.device_address(),
            device.port_chain(),
            device.driver(),
        );
    }
}
