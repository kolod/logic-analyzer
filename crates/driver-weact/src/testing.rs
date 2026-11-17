//! A scripted USB double and recordings for tests (feature `test-support`).
//!
//! [`RecordedUsb`] stands for the whole USB bus with some boards on it. It implements
//! [`UsbBackend`], so the driver cannot tell it from the real thing, and it logs every request the
//! driver makes ([`Request`]) so that tests can assert exactly what was sent and in which order.
//!
//! What a test can script:
//!
//! - boards that are plugged and unplugged, with or without the firmware, with a bound driver
//!   name, that refuse to open in a given way;
//! - the answers to control requests, and what the firmware download does: after the CPUCS release
//!   the board leaves the bus and returns (running the firmware, at another address) after a
//!   delay of the virtual clock, or never;
//! - for every capture, the data that is in the board's buffer from an earlier capture (stale,
//!   served before the start command) and the stream that follows the start command, step by step
//!   (data, an empty transfer, a timeout, an error);
//! - faults at chosen points: a panic, a hang until [`RecordedUsb::release`], or an error.
//!
//! Time is virtual ([`ManualTimer`]): sleeping and waiting advance it, so the tests of the
//! renumeration limit and of the silence limits take no real time.

#![allow(
    clippy::panic,
    reason = "the double injects panics on purpose to test containment"
)]

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;
use la_diagnostics::lock_recover;

use crate::error;
use crate::protocol::{
    self, BufferPlan, CPUCS, MANUFACTURER, PRODUCT, PRODUCT_ID, REQUEST_GET_FW_VERSION,
    REQUEST_GET_REVID, REQUEST_LOADER, REQUEST_START, VENDOR_ID,
};
use crate::usb::{BoardKey, BulkStream, StreamEvent, Timer, UsbBackend, UsbInfo, UsbLink};

/// The virtual clock: it moves only when the driver sleeps or waits, or when a test advances it.
#[derive(Debug, Default)]
pub struct ManualTimer {
    now: Mutex<Duration>,
}

impl ManualTimer {
    /// A clock at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves the clock forward.
    pub fn advance(&self, by: Duration) {
        let mut now = lock_recover(&self.now);
        *now = now.saturating_add(by);
    }
}

impl Timer for ManualTimer {
    fn now(&self) -> Duration {
        *lock_recover(&self.now)
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
    }
}

/// How a request or a transfer fails, in the terms of the driver's errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fail {
    /// The board was unplugged (`device.disconnected`).
    Disconnected,
    /// A transfer failed (`device.transport`).
    Transport,
    /// The system refuses access (`weact.access_denied`).
    AccessDenied,
    /// The board is not on WinUSB (`weact.driver_missing`, naming the driver if given).
    DriverMissing(Option<String>),
    /// Another program holds the board (`weact.in_use`).
    InUse,
}

impl Fail {
    /// The typed error this stands for.
    #[must_use]
    pub fn build(&self) -> Error {
        match self {
            Fail::Disconnected => device_error::disconnected("scripted: the board is gone"),
            Fail::Transport => device_error::transport("scripted: a transfer failed"),
            Fail::AccessDenied => error::access_denied("scripted: access denied"),
            Fail::DriverMissing(driver) => error::driver_missing(driver.as_deref()),
            Fail::InUse => error::in_use("scripted: the board is in use"),
        }
    }
}

/// One step of the bulk stream of a capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStep {
    /// A completed transfer with these bytes.
    Data(Vec<u8>),
    /// A completed transfer with no bytes.
    Empty,
    /// Nothing completes: the wait times out (the virtual clock moves by the wait).
    Timeout,
    /// A transfer fails.
    Fail(Fail),
}

/// Splits a signal (one byte per sample) into `Data` steps of at most `chunk` bytes.
#[must_use]
pub fn steps_from_signal(signal: &[u8], chunk: usize) -> Vec<StreamStep> {
    signal
        .chunks(chunk.max(1))
        .map(|part| StreamStep::Data(part.to_vec()))
        .collect()
}

/// What the board holds and delivers in one capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capture {
    /// Bytes from an earlier capture still in the buffer: they come out first, whether or
    /// not the start command has been sent, until the driver has read them all.
    pub stale: Vec<StreamStep>,
    /// The stream after the start command. When it runs out the board is silent (timeouts),
    /// unless `endless` is set.
    pub stream: Vec<StreamStep>,
    /// After the stream has run out, deliver this chunk again and again (a board that never
    /// stops, for open-ended captures and the conformance suite).
    pub endless: Option<Vec<u8>>,
}

impl Capture {
    /// A capture with only a stream.
    #[must_use]
    pub fn stream(stream: Vec<StreamStep>) -> Self {
        Self {
            stale: Vec::new(),
            stream,
            endless: None,
        }
    }

    /// A capture that delivers `chunk` forever.
    #[must_use]
    pub fn endless(chunk: Vec<u8>) -> Self {
        Self {
            endless: Some(chunk),
            ..Self::default()
        }
    }
}

/// How a board is plugged in.
#[derive(Debug, Clone)]
pub struct BoardSpec {
    /// Where it is.
    pub key: BoardKey,
    /// Whether it runs the firmware (`sigrok` / `fx2lafw` strings) or is blank (no strings).
    pub running: bool,
    /// The bound driver name, as `nusb` would report it.
    pub driver: Option<String>,
    /// The vendor ID (the board's by default; another value makes a foreign device).
    pub vendor_id: u16,
    /// The product ID.
    pub product_id: u16,
}

impl BoardSpec {
    /// A board that runs the firmware, on WinUSB.
    #[must_use]
    pub fn running(bus: &str, ports: &[u8]) -> Self {
        Self {
            key: BoardKey::new(bus, ports),
            running: true,
            driver: Some("WinUSB".to_owned()),
            vendor_id: VENDOR_ID,
            product_id: PRODUCT_ID,
        }
    }

    /// A blank board (no firmware, no strings), on WinUSB.
    #[must_use]
    pub fn blank(bus: &str, ports: &[u8]) -> Self {
        Self {
            running: false,
            ..Self::running(bus, ports)
        }
    }
}

/// The places where a fault can be injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// `UsbBackend::list`.
    List,
    /// `UsbBackend::open`.
    Open,
    /// `UsbLink::control_in`.
    ControlIn,
    /// `UsbLink::control_out`.
    ControlOut,
    /// `UsbLink::open_stream`.
    OpenStream,
    /// `BulkStream::submit_all`.
    Submit,
    /// `BulkStream::wait`.
    Wait,
    /// `BulkStream::close`.
    Close,
}

/// What an injected fault does, once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Panics.
    Panic,
    /// Blocks until [`RecordedUsb::release`].
    Hang,
    /// Fails with this error (not for `wait`, whose failures are stream steps).
    Fail(Fail),
}

/// What the driver asked of the bus, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// The bus was listed.
    List,
    /// A board was opened.
    Open(BoardKey),
    /// A control read: request, value, index, length.
    ControlIn(u8, u16, u16, u16),
    /// A control write: request, value, index, data.
    ControlOut(u8, u16, u16, Vec<u8>),
    /// The stream was opened with this plan.
    OpenStream(BufferPlan),
    /// All buffers were submitted.
    SubmitAll,
    /// The stream was closed.
    Close,
}

#[derive(Debug)]
struct Board {
    key: BoardKey,
    address: u8,
    running: bool,
    present: bool,
    driver: Option<String>,
    vendor_id: u16,
    product_id: u16,
    open_fail: Option<Fail>,
    version: Vec<u8>,
    renumerate_after: Option<Duration>,
    hide_manufacturer: bool,
    returns_at: Option<Duration>,
    image: Vec<u8>,
    captures: VecDeque<Capture>,
    default_capture: Option<Capture>,
}

#[derive(Debug, Default)]
struct World {
    boards: Vec<Board>,
    log: Vec<Request>,
    faults: Vec<(Point, Fault)>,
    list_fail: Option<Fail>,
    next_address: u8,
}

#[derive(Debug)]
struct Shared {
    world: Mutex<World>,
    released: Mutex<bool>,
    gate: Condvar,
    timer: Arc<ManualTimer>,
}

/// The scripted USB bus. Cloning shares the same bus.
#[derive(Debug, Clone)]
pub struct RecordedUsb {
    shared: Arc<Shared>,
}

impl Default for RecordedUsb {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordedUsb {
    /// An empty bus with a virtual clock at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                world: Mutex::new(World {
                    next_address: 2,
                    ..World::default()
                }),
                released: Mutex::new(false),
                gate: Condvar::new(),
                timer: Arc::new(ManualTimer::new()),
            }),
        }
    }

    /// The virtual clock; give it to the code under test as its `Timer`.
    #[must_use]
    pub fn timer(&self) -> Arc<ManualTimer> {
        Arc::clone(&self.shared.timer)
    }

    fn with_world<T>(&self, f: impl FnOnce(&mut World) -> T) -> T {
        f(&mut lock_recover(&self.shared.world))
    }

    fn with_board<T>(&self, key: &BoardKey, f: impl FnOnce(&mut Board) -> T) -> Option<T> {
        self.with_world(|world| world.boards.iter_mut().find(|b| b.key == *key).map(f))
    }

    /// Plugs a board in (or plugs it in again after [`unplug`](Self::unplug)).
    pub fn plug(&self, spec: BoardSpec) {
        self.with_world(|world| {
            let address = world.next_address;
            world.next_address = world.next_address.wrapping_add(1).max(2);
            if let Some(board) = world.boards.iter_mut().find(|b| b.key == spec.key) {
                board.present = true;
                board.running = spec.running;
                board.driver = spec.driver;
                board.address = address;
                board.returns_at = None;
                return;
            }
            world.boards.push(Board {
                key: spec.key,
                address,
                running: spec.running,
                present: true,
                driver: spec.driver,
                vendor_id: spec.vendor_id,
                product_id: spec.product_id,
                open_fail: None,
                version: vec![1, 4],
                renumerate_after: Some(Duration::from_millis(900)),
                hide_manufacturer: false,
                returns_at: None,
                image: Vec::new(),
                captures: VecDeque::new(),
                default_capture: None,
            });
        });
    }

    /// Unplugs a board.
    pub fn unplug(&self, key: &BoardKey) {
        self.with_board(key, |board| {
            board.present = false;
            board.returns_at = None;
        });
    }

    /// Changes the bound driver name.
    pub fn set_driver(&self, key: &BoardKey, driver: Option<&str>) {
        self.with_board(key, |board| board.driver = driver.map(str::to_owned));
    }

    /// Makes opening the board fail (or work again with `None`).
    pub fn fail_open(&self, key: &BoardKey, fail: Option<Fail>) {
        self.with_board(key, |board| board.open_fail = fail);
    }

    /// Makes the bus stop reporting the manufacturer string of the board, as Windows does.
    pub fn hide_manufacturer(&self, key: &BoardKey, hide: bool) {
        self.with_board(key, |board| board.hide_manufacturer = hide);
    }

    /// Makes listing the bus fail (or work again with `None`).
    pub fn fail_list(&self, fail: Option<Fail>) {
        self.with_world(|world| world.list_fail = fail);
    }

    /// Sets the bytes the board answers to the firmware version request.
    pub fn set_version(&self, key: &BoardKey, answer: &[u8]) {
        self.with_board(key, |board| board.version = answer.to_vec());
    }

    /// Sets how long after the firmware download the board returns; `None`: it never does.
    pub fn renumerate_after(&self, key: &BoardKey, delay: Option<Duration>) {
        self.with_board(key, |board| board.renumerate_after = delay);
    }

    /// Queues the data of the next capture the board will deliver.
    pub fn script_capture(&self, key: &BoardKey, capture: Capture) {
        self.with_board(key, |board| board.captures.push_back(capture));
    }

    /// The capture the board delivers when none is queued (for tests that open it many times).
    pub fn set_default_capture(&self, key: &BoardKey, capture: Capture) {
        self.with_board(key, |board| board.default_capture = Some(capture));
    }

    /// Injects a fault at a point; it fires once, at the next request of that kind.
    pub fn inject(&self, point: Point, fault: Fault) {
        self.with_world(|world| world.faults.push((point, fault)));
    }

    /// Releases every hang.
    pub fn release(&self) {
        *lock_recover(&self.shared.released) = true;
        self.shared.gate.notify_all();
    }

    /// Everything the driver asked of the bus so far.
    #[must_use]
    pub fn log(&self) -> Vec<Request> {
        self.with_world(|world| world.log.clone())
    }

    /// Forgets the log.
    pub fn clear_log(&self) {
        self.with_world(|world| world.log.clear());
    }

    /// The firmware image the board received: the chunks of the download in the order written.
    #[must_use]
    pub fn downloaded_image(&self, key: &BoardKey) -> Vec<u8> {
        self.with_board(key, |board| board.image.clone())
            .unwrap_or_default()
    }

    /// Whether the board is on the bus and runs the firmware now.
    #[must_use]
    pub fn runs_firmware(&self, key: &BoardKey) -> bool {
        self.settle();
        self.with_board(key, |board| board.present && board.running)
            .unwrap_or(false)
    }

    /// Brings boards that are due back onto the bus (the virtual time has passed their return).
    fn settle(&self) {
        let now = self.shared.timer.now();
        self.with_world(|world| {
            for board in &mut world.boards {
                if board.returns_at.is_some_and(|at| now >= at) {
                    board.returns_at = None;
                    board.present = true;
                    board.running = true;
                }
            }
        });
    }

    fn record(&self, request: Request) {
        self.with_world(|world| world.log.push(request));
    }

    /// Applies an injected fault for `point`, if there is one. A hang waits for the release.
    fn fault(&self, point: Point) -> Result<(), Error> {
        let fault = self.with_world(|world| {
            let at = world.faults.iter().position(|(p, _)| *p == point)?;
            Some(world.faults.remove(at).1)
        });
        match fault {
            None => Ok(()),
            Some(Fault::Panic) => panic!("scripted panic at {point:?}"),
            Some(Fault::Fail(fail)) => Err(fail.build()),
            Some(Fault::Hang) => {
                let mut released = lock_recover(&self.shared.released);
                while !*released {
                    released = match self.shared.gate.wait(released) {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                }
                Ok(())
            }
        }
    }
}

impl UsbBackend for RecordedUsb {
    fn list(&self) -> Result<Vec<UsbInfo>, Error> {
        self.record(Request::List);
        self.fault(Point::List)?;
        self.settle();
        self.with_world(|world| {
            if let Some(fail) = &world.list_fail {
                return Err(fail.build());
            }
            Ok(world
                .boards
                .iter()
                .filter(|board| board.present)
                .map(|board| UsbInfo {
                    key: board.key.clone(),
                    vendor_id: board.vendor_id,
                    product_id: board.product_id,
                    manufacturer: (board.running && !board.hide_manufacturer)
                        .then(|| MANUFACTURER.to_owned()),
                    product: board.running.then(|| PRODUCT.to_owned()),
                    address: board.address,
                    driver: board.driver.clone(),
                })
                .collect())
        })
    }

    fn open(&self, key: &BoardKey) -> Result<Box<dyn UsbLink>, Error> {
        self.record(Request::Open(key.clone()));
        self.fault(Point::Open)?;
        self.settle();
        let outcome = self.with_board(key, |board| {
            if !board.present {
                return Err(device_error::disconnected(
                    "scripted: the board is not on the bus",
                ));
            }
            match &board.open_fail {
                Some(fail) => Err(fail.build()),
                None => Ok(()),
            }
        });
        match outcome {
            None => Err(device_error::disconnected("scripted: no such board")),
            Some(Err(error)) => Err(error),
            Some(Ok(())) => Ok(Box::new(RecordedLink {
                usb: self.clone(),
                key: key.clone(),
            })),
        }
    }
}

struct RecordedLink {
    usb: RecordedUsb,
    key: BoardKey,
}

impl RecordedLink {
    fn gone(&self) -> bool {
        self.usb.settle();
        !self
            .usb
            .with_board(&self.key, |board| board.present)
            .unwrap_or(false)
    }
}

impl UsbLink for RecordedLink {
    fn control_in(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        _timeout: Duration,
    ) -> Result<Vec<u8>, Error> {
        self.usb
            .record(Request::ControlIn(request, value, index, length));
        self.usb.fault(Point::ControlIn)?;
        if self.gone() {
            return Err(Fail::Disconnected.build());
        }
        let answer = self
            .usb
            .with_board(&self.key, |board| match request {
                REQUEST_GET_FW_VERSION => board.version.clone(),
                REQUEST_GET_REVID => vec![0x04],
                _ => Vec::new(),
            })
            .unwrap_or_default();
        Ok(answer)
    }

    fn control_out(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.usb
            .record(Request::ControlOut(request, value, index, data.to_vec()));
        self.usb.fault(Point::ControlOut)?;
        if self.gone() {
            return Err(Fail::Disconnected.build());
        }
        let now = self.usb.shared.timer.now();
        self.usb.with_board(&self.key, |board| {
            if request == REQUEST_LOADER && value != CPUCS {
                // A chunk of the firmware: written at its address (the image starts at 0).
                let at = usize::from(value);
                if board.image.len() < at {
                    board.image.resize(at, 0);
                }
                board.image.truncate(at);
                board.image.extend_from_slice(data);
            } else if request == REQUEST_LOADER && value == CPUCS && data == [0x00] {
                // The reset is released: the board leaves the bus and comes back with the firmware.
                board.present = false;
                board.returns_at = board
                    .renumerate_after
                    .map(|delay| now.saturating_add(delay));
            } else if request == REQUEST_START {
                board.running = true;
            }
        });
        Ok(())
    }

    fn open_stream(&mut self, plan: BufferPlan) -> Result<Box<dyn BulkStream>, Error> {
        self.usb.record(Request::OpenStream(plan));
        self.usb.fault(Point::OpenStream)?;
        if self.gone() {
            return Err(Fail::Disconnected.build());
        }
        let capture = self
            .usb
            .with_board(&self.key, |board| {
                board
                    .captures
                    .pop_front()
                    .or_else(|| board.default_capture.clone())
            })
            .flatten()
            .unwrap_or_default();
        Ok(Box::new(RecordedStream {
            usb: self.usb.clone(),
            key: self.key.clone(),
            stale: capture.stale.into(),
            stream: capture.stream.into(),
            endless: capture.endless,
            started: false,
            closed: false,
        }))
    }
}

struct RecordedStream {
    usb: RecordedUsb,
    key: BoardKey,
    stale: VecDeque<StreamStep>,
    stream: VecDeque<StreamStep>,
    endless: Option<Vec<u8>>,
    started: bool,
    closed: bool,
}

impl RecordedStream {
    /// Whether the start command has been sent since this stream was opened: the log has a
    /// `CMD_START` after the last `OpenStream`.
    fn start_seen(&self) -> bool {
        self.usb.with_world(|world| {
            let opened = world
                .log
                .iter()
                .rposition(|r| matches!(r, Request::OpenStream(_)));
            opened.is_some_and(|at| {
                world.log.iter().skip(at).any(
                    |r| matches!(r, Request::ControlOut(request, ..) if *request == REQUEST_START),
                )
            })
        })
    }
}

impl BulkStream for RecordedStream {
    fn submit_all(&mut self) -> Result<(), Error> {
        self.usb.record(Request::SubmitAll);
        self.usb.fault(Point::Submit)?;
        Ok(())
    }

    fn wait(&mut self, timeout: Duration) -> StreamEvent {
        if let Err(error) = self.usb.fault(Point::Wait) {
            return StreamEvent::Error(error);
        }
        self.started = self.started || self.start_seen();
        // What was left in the buffer by an earlier capture comes out first, before or after the
        // start command, until the driver has read it all; only then does the new stream follow.
        let step = self.stale.pop_front().or_else(|| {
            if self.started {
                self.stream
                    .pop_front()
                    .or_else(|| self.endless.clone().map(StreamStep::Data))
            } else {
                None
            }
        });
        match step {
            Some(StreamStep::Data(bytes)) => StreamEvent::Data(bytes),
            Some(StreamStep::Empty) => StreamEvent::Empty,
            Some(StreamStep::Fail(fail)) => StreamEvent::Error(fail.build()),
            Some(StreamStep::Timeout) | None => {
                self.usb.shared.timer.advance(timeout);
                StreamEvent::Timeout
            }
        }
    }

    fn close(&mut self, _within: Duration) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.usb.record(Request::Close);
        // A fault at `Close` cannot fail the call: a panic or a hang is what is tested.
        let _ = self.usb.fault(Point::Close);
        let _ = &self.key;
    }
}

/// The CPUCS value that holds the 8051 in reset, for scripts and assertions.
pub const CPUCS_HOLD: [u8; 1] = [0x01];
/// The CPUCS value that releases the reset.
pub const CPUCS_RELEASE: [u8; 1] = [0x00];

/// A convenience for tests: the payload of `CMD_START` for a rate (see [`protocol::start_payload`]).
///
/// # Errors
///
/// The error of [`protocol::start_payload`].
pub fn start_payload(rate: u64) -> Result<[u8; 3], Error> {
    protocol::start_payload(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_millis(100);

    fn bus_with(spec: BoardSpec) -> (RecordedUsb, BoardKey) {
        let usb = RecordedUsb::new();
        let key = spec.key.clone();
        usb.plug(spec);
        (usb, key)
    }

    #[test]
    fn a_listed_board_shows_its_strings_only_when_it_runs_the_firmware() {
        let (usb, _) = bus_with(BoardSpec::running("001", &[1]));
        usb.plug(BoardSpec::blank("001", &[2]));
        let list = usb.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].runs_firmware());
        assert!(!list[1].runs_firmware());
        assert_eq!(list[1].manufacturer, None);
        assert_eq!(usb.log(), vec![Request::List]);
    }

    #[test]
    fn the_download_makes_the_board_leave_and_return_after_the_delay_of_the_virtual_clock() {
        let (usb, key) = bus_with(BoardSpec::blank("001", &[1]));
        let mut link = usb.open(&key).unwrap();
        link.control_out(REQUEST_LOADER, CPUCS, 0, &CPUCS_HOLD, T)
            .unwrap();
        link.control_out(REQUEST_LOADER, 0, 0, &[1, 2, 3], T)
            .unwrap();
        link.control_out(REQUEST_LOADER, 3, 0, &[4, 5], T).unwrap();
        link.control_out(REQUEST_LOADER, CPUCS, 0, &CPUCS_RELEASE, T)
            .unwrap();
        assert_eq!(usb.downloaded_image(&key), vec![1, 2, 3, 4, 5]);
        assert!(usb.list().unwrap().is_empty(), "the board left the bus");
        usb.timer().advance(Duration::from_millis(899));
        assert!(usb.list().unwrap().is_empty());
        usb.timer().advance(Duration::from_millis(1));
        let back = usb.list().unwrap();
        assert_eq!(back.len(), 1);
        assert!(back[0].runs_firmware());
        assert_eq!(back[0].key, key, "the same place");
    }

    #[test]
    fn a_board_can_be_scripted_never_to_return() {
        let (usb, key) = bus_with(BoardSpec::blank("001", &[1]));
        usb.renumerate_after(&key, None);
        let mut link = usb.open(&key).unwrap();
        link.control_out(REQUEST_LOADER, CPUCS, 0, &CPUCS_RELEASE, T)
            .unwrap();
        usb.timer().advance(Duration::from_secs(3_600));
        assert!(usb.list().unwrap().is_empty());
    }

    #[test]
    fn opening_can_be_refused_in_every_way_the_driver_distinguishes() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        for (fail, code) in [
            (Fail::AccessDenied, "weact.access_denied"),
            (
                Fail::DriverMissing(Some("libusb0".to_owned())),
                "weact.driver_missing",
            ),
            (Fail::InUse, "weact.in_use"),
            (Fail::Disconnected, "device.disconnected"),
            (Fail::Transport, "device.transport"),
        ] {
            usb.fail_open(&key, Some(fail));
            assert_eq!(usb.open(&key).err().unwrap().code(), code);
        }
        usb.fail_open(&key, None);
        assert!(usb.open(&key).is_ok());
        let other = BoardKey::new("001", [9]);
        assert_eq!(
            usb.open(&other).err().unwrap().code(),
            "device.disconnected"
        );
    }

    #[test]
    fn control_answers_follow_the_script() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        usb.set_version(&key, &[2, 0]);
        let mut link = usb.open(&key).unwrap();
        assert_eq!(
            link.control_in(REQUEST_GET_FW_VERSION, 0, 0, 2, T).unwrap(),
            vec![2, 0]
        );
        assert_eq!(
            link.control_in(REQUEST_GET_REVID, 0, 0, 1, T).unwrap(),
            vec![4]
        );
        assert!(link.control_in(0x77, 0, 0, 8, T).unwrap().is_empty());
    }

    #[test]
    fn the_stream_serves_stale_data_before_the_start_and_the_capture_after_it() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        usb.script_capture(
            &key,
            Capture {
                stale: vec![StreamStep::Data(vec![9, 9])],
                stream: vec![
                    StreamStep::Data(vec![1, 2]),
                    StreamStep::Empty,
                    StreamStep::Timeout,
                ],
                endless: None,
            },
        );
        let mut link = usb.open(&key).unwrap();
        let mut stream = link
            .open_stream(BufferPlan {
                size: 512,
                count: 4,
            })
            .unwrap();
        stream.submit_all().unwrap();
        assert!(matches!(stream.wait(T), StreamEvent::Data(d) if d == [9, 9]));
        assert!(
            matches!(stream.wait(T), StreamEvent::Timeout),
            "the buffer is quiet"
        );
        link.control_out(REQUEST_START, 0, 0, &[0x40, 0, 0x2F], T)
            .unwrap();
        assert!(matches!(stream.wait(T), StreamEvent::Data(d) if d == [1, 2]));
        assert!(matches!(stream.wait(T), StreamEvent::Empty));
        assert!(matches!(stream.wait(T), StreamEvent::Timeout));
        assert!(
            matches!(stream.wait(T), StreamEvent::Timeout),
            "silent when the script ends"
        );
        stream.close(T);
        stream.close(T);
        let log = usb.log();
        assert_eq!(log.iter().filter(|r| **r == Request::Close).count(), 1);
        let order: Vec<&Request> = log
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Request::SubmitAll | Request::ControlOut(REQUEST_START, ..)
                )
            })
            .collect();
        assert!(matches!(order[0], Request::SubmitAll));
        assert!(matches!(order[1], Request::ControlOut(REQUEST_START, ..)));
    }

    #[test]
    fn waiting_without_data_moves_the_virtual_clock_by_the_wait() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        let mut link = usb.open(&key).unwrap();
        let mut stream = link
            .open_stream(BufferPlan {
                size: 512,
                count: 4,
            })
            .unwrap();
        let before = usb.timer().now();
        let _ = stream.wait(Duration::from_millis(40));
        assert_eq!(usb.timer().now() - before, Duration::from_millis(40));
    }

    #[test]
    fn a_failing_step_and_an_unplug_are_delivered_as_typed_errors() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        usb.script_capture(
            &key,
            Capture::stream(vec![StreamStep::Fail(Fail::Transport)]),
        );
        let mut link = usb.open(&key).unwrap();
        let mut stream = link
            .open_stream(BufferPlan {
                size: 512,
                count: 4,
            })
            .unwrap();
        link.control_out(REQUEST_START, 0, 0, &[0, 0, 1], T)
            .unwrap();
        assert!(matches!(stream.wait(T), StreamEvent::Error(e) if e.code() == "device.transport"));
        usb.unplug(&key);
        let error = link
            .control_in(REQUEST_GET_FW_VERSION, 0, 0, 2, T)
            .unwrap_err();
        assert_eq!(error.code(), "device.disconnected");
    }

    #[test]
    fn faults_fire_once_at_the_chosen_point() {
        let (usb, key) = bus_with(BoardSpec::running("001", &[1]));
        usb.inject(Point::List, Fault::Fail(Fail::Transport));
        assert_eq!(usb.list().err().unwrap().code(), "device.transport");
        assert!(usb.list().is_ok(), "once");
        usb.inject(Point::Open, Fault::Panic);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| usb.open(&key)));
        assert!(caught.is_err());
        assert!(usb.open(&key).is_ok());
    }

    #[test]
    fn a_hang_lasts_until_it_is_released() {
        let (usb, _) = bus_with(BoardSpec::running("001", &[1]));
        usb.inject(Point::List, Fault::Hang);
        let handle = {
            let usb = usb.clone();
            std::thread::spawn(move || usb.list().map(|l| l.len()))
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!handle.is_finished(), "still hanging");
        usb.release();
        assert_eq!(handle.join().unwrap().unwrap(), 1);
    }

    #[test]
    fn a_bus_that_cannot_be_read_gives_the_scripted_error() {
        let (usb, _) = bus_with(BoardSpec::running("001", &[1]));
        usb.fail_list(Some(Fail::Transport));
        assert_eq!(usb.list().err().unwrap().code(), "device.transport");
        usb.fail_list(None);
        assert!(usb.list().is_ok());
    }

    #[test]
    fn a_signal_is_cut_into_chunks_and_start_payload_is_the_one_of_the_protocol() {
        let steps = steps_from_signal(&[1, 2, 3, 4, 5], 2);
        assert_eq!(
            steps,
            vec![
                StreamStep::Data(vec![1, 2]),
                StreamStep::Data(vec![3, 4]),
                StreamStep::Data(vec![5]),
            ]
        );
        assert!(steps_from_signal(&[], 4).is_empty());
        assert_eq!(start_payload(1_000_000).unwrap(), [0x40, 0x00, 0x2F]);
    }
}
