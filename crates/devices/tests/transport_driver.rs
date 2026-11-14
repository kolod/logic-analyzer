//! A driver written against `Transport` and tested without hardware (spec US8, SC-011).
//!
//! `TransportDevice` is a tiny example driver for an invented protocol. Its tests replay
//! recorded traffic (bytes, short reads, timeouts, disconnects) and need no hardware.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{descriptor, registry_fast, std_config, wait_outcome, wait_until};
use la_core::Result;
use la_devices::error::{DISCONNECTED, TRANSPORT};
use la_devices::harness::conformance::{Expectations, run_all};
use la_devices::harness::{RecordedTransport, TestProvider, TransportStep};
use la_devices::{
    AcquisitionOptions, AppliedConfig, Device, DeviceDescriptor, DeviceId, DeviceProvider,
    EndReason, Flow, READ_TIMEOUT, ReadOutcome, ResolvedConfig, Transport,
};

const FRAME_SAMPLES: u8 = 0x01;
const FRAME_LOST: u8 = 0x02;
const FRAME_END: u8 = 0x03;

/// The example driver: writes `START`, reads frames, writes `STOP`.
///
/// Frames: `0x01 len(u16 LE) bytes...` samples (one byte per sample), `0x02 count(u32 LE)` lost
/// samples, `0x03` end of the capture.
struct TransportDevice<T: Transport> {
    descriptor: DeviceDescriptor,
    transport: T,
    buffer: Vec<u8>,
}

enum Frame {
    Samples(Vec<u8>),
    Lost(u32),
    End,
}

/// Parses one frame from the front of `buffer`; `None` if it is not complete yet.
fn parse(buffer: &[u8]) -> Option<(Frame, usize)> {
    match *buffer.first()? {
        FRAME_SAMPLES => {
            let len = usize::from(u16::from_le_bytes([*buffer.get(1)?, *buffer.get(2)?]));
            let body = buffer.get(3..3 + len)?;
            Some((Frame::Samples(body.to_vec()), 3 + len))
        }
        FRAME_LOST => {
            let bytes: [u8; 4] = buffer.get(1..5)?.try_into().ok()?;
            Some((Frame::Lost(u32::from_le_bytes(bytes)), 5))
        }
        FRAME_END => Some((Frame::End, 1)),
        _ => None,
    }
}

impl<T: Transport> Device for TransportDevice<T> {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn flow(&self) -> Flow {
        Flow::Throttleable
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        self.transport.write(b"START", READ_TIMEOUT)?;
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }

    fn read(&mut self, _max: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        loop {
            if let Some((frame, used)) = parse(&self.buffer) {
                self.buffer.drain(..used);
                return Ok(match frame {
                    Frame::Samples(bytes) => {
                        out.extend(bytes.iter().map(|b| u32::from(*b)));
                        ReadOutcome::Samples
                    }
                    Frame::Lost(n) => ReadOutcome::Lost(u64::from(n)),
                    Frame::End => ReadOutcome::Finished,
                });
            }
            let mut chunk = [0_u8; 64];
            let n = self.transport.read(&mut chunk, READ_TIMEOUT)?;
            if n == 0 {
                return Ok(ReadOutcome::Idle);
            }
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }

    fn stop(&mut self) -> Result<()> {
        self.transport.write(b"STOP", READ_TIMEOUT)?;
        Ok(())
    }
}

fn samples_frame(bytes: &[u8]) -> Vec<u8> {
    let mut frame = vec![FRAME_SAMPLES];
    frame.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    frame.extend_from_slice(bytes);
    frame
}

fn lost_frame(n: u32) -> Vec<u8> {
    let mut frame = vec![FRAME_LOST];
    frame.extend_from_slice(&n.to_le_bytes());
    frame
}

/// Records which transports were created, so tests can inspect what the driver wrote.
type Logs = Arc<Mutex<Vec<la_devices::harness::TransportLog>>>;

/// A provider whose device speaks the protocol over a replayed recording.
fn provider_replaying(steps: Vec<TransportStep>) -> (Arc<dyn DeviceProvider>, DeviceId, Logs) {
    let provider = TestProvider::new("rec");
    let d = descriptor("rec", "device", 8);
    let id = d.id.clone();
    let logs: Logs = Arc::new(Mutex::new(Vec::new()));
    let (for_factory, logs_for_factory) = (d.clone(), Arc::clone(&logs));
    provider.plug_with(
        d,
        Arc::new(move || {
            let transport = RecordedTransport::new(steps.clone());
            logs_for_factory.lock().unwrap().push(transport.log());
            Box::new(TransportDevice {
                descriptor: for_factory.clone(),
                transport,
                buffer: Vec::new(),
            })
        }),
    );
    (Arc::new(provider), id, logs)
}

/// The recording of a healthy session: frames split across short reads, a timeout, a loss.
fn healthy_recording() -> (Vec<TransportStep>, Vec<u32>, u64) {
    let (a, b) = (
        (0..100_u8).map(|i| i & 0xF).collect::<Vec<_>>(),
        (0..60_u8).map(|i| i & 0x7).collect::<Vec<_>>(),
    );
    let mut first = samples_frame(&a);
    first.extend(samples_frame(&[1, 2, 3]));
    let steps = vec![
        TransportStep::ShortRead(2),
        TransportStep::Bytes(first),
        TransportStep::Timeout,
        TransportStep::Bytes(lost_frame(25)),
        TransportStep::ShortRead(5),
        TransportStep::Bytes(samples_frame(&b)),
        TransportStep::Bytes(vec![FRAME_END]),
    ];
    let words: Vec<u32> = a
        .iter()
        .chain([1_u8, 2, 3].iter())
        .chain(b.iter())
        .map(|w| u32::from(*w))
        .collect();
    (steps, words, 25)
}

fn run(
    provider: Arc<dyn DeviceProvider>,
    id: &DeviceId,
) -> (la_devices::Acquisition, la_devices::AcquisitionOutcome) {
    let registry = registry_fast();
    registry.add_provider(provider);
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(id)
        .is_some()));
    let acq = registry
        .start(
            id,
            std_config(0b1111, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    (acq, outcome)
}

#[test]
fn a_replayed_recording_reproduces_the_same_samples_gaps_and_errors_every_time() {
    let (steps, words, lost) = healthy_recording();
    let mut fingerprints = Vec::new();
    for _ in 0..20 {
        let (provider, id, _) = provider_replaying(steps.clone());
        let (acq, outcome) = run(provider, &id);
        assert_eq!(outcome.end, EndReason::Completed);
        assert_eq!(outcome.delivered_samples, words.len() as u64);
        assert_eq!(outcome.lost_positions, lost);
        assert_eq!(outcome.positions, words.len() as u64 + lost);
        let snap = acq.capture().snapshot();
        assert_eq!(snap.gaps().len(), 1);
        assert_eq!(snap.gaps()[0].length, lost);
        // The samples around the gap are exactly the recorded ones.
        let gap_start = snap.gaps()[0].start;
        let mut seen = Vec::new();
        for p in 0..snap.length() {
            if p >= gap_start && p < gap_start + lost {
                continue;
            }
            let word: u32 = (0..4_u8)
                .map(|ch| {
                    u32::from(snap.level_at(ch, p).unwrap() == la_storage::LevelState::High) << ch
                })
                .sum();
            seen.push(word & 0xF);
        }
        let expected: Vec<u32> = words.iter().map(|w| w & 0xF).collect();
        assert_eq!(seen, expected);
        fingerprints.push((outcome.delivered_samples, outcome.lost_positions, seen));
    }
    assert!(
        fingerprints.windows(2).all(|w| w[0] == w[1]),
        "identical on every run"
    );
}

#[test]
fn the_driver_writes_the_protocol_commands_in_order() {
    let (steps, _, _) = healthy_recording();
    let (provider, id, logs) = provider_replaying(steps);
    let (_, outcome) = run(provider, &id);
    assert_eq!(outcome.end, EndReason::Completed);
    let log = logs.lock().unwrap()[0].clone();
    assert_eq!(log.written(), vec![b"START".to_vec(), b"STOP".to_vec()]);
}

#[test]
fn a_disconnect_in_the_recording_is_a_disconnect_with_the_data_before_it_intact() {
    let steps = vec![
        TransportStep::Bytes(samples_frame(&[1, 2, 3, 4])),
        TransportStep::Bytes(vec![FRAME_SAMPLES, 10, 0, 1, 2]), // a frame cut short by the disconnect
        TransportStep::Disconnect,
    ];
    let (provider, id, _) = provider_replaying(steps);
    let (acq, outcome) = run(provider, &id);
    assert_eq!(outcome.end, EndReason::Failed);
    assert_eq!(outcome.error.as_ref().unwrap().code(), DISCONNECTED);
    assert_eq!(
        outcome.delivered_samples, 4,
        "the incomplete frame is not delivered"
    );
    assert!(outcome.capture_status.interrupted);
    assert_eq!(acq.capture().snapshot().length(), 4);
}

#[test]
fn a_transport_error_and_an_exhausted_recording_are_transport_errors() {
    let (provider, id, _) = provider_replaying(vec![
        TransportStep::Bytes(samples_frame(&[1, 2])),
        TransportStep::Error("USB transfer failed".to_owned()),
    ]);
    let (_, outcome) = run(provider, &id);
    assert_eq!(outcome.error.as_ref().unwrap().code(), TRANSPORT);
    assert!(
        outcome
            .error
            .as_ref()
            .unwrap()
            .detail()
            .contains("USB transfer failed")
    );

    // A recording that ends before the capture does is a test bug and is reported as such.
    let (provider, id, _) = provider_replaying(vec![TransportStep::Bytes(samples_frame(&[7]))]);
    let (_, outcome) = run(provider, &id);
    assert_eq!(outcome.end, EndReason::Failed);
    assert_eq!(outcome.error.as_ref().unwrap().code(), TRANSPORT);
    assert!(
        outcome
            .error
            .as_ref()
            .unwrap()
            .detail()
            .contains("recording ended")
    );
}

#[test]
fn the_transport_driver_passes_the_shared_conformance_suite() {
    // A long recording, so the driver is still running when the suite stops or injects faults.
    let mut steps = vec![TransportStep::ShortRead(3)];
    for i in 0..400_u32 {
        steps.push(TransportStep::Bytes(samples_frame(&[(i & 0xF) as u8; 50])));
    }
    steps.push(TransportStep::Bytes(vec![FRAME_END]));
    let (provider, id, _) = provider_replaying(steps);
    let expectations = Expectations {
        finite_capture_samples: None,
        ..Expectations::default()
    };
    run_all(provider, &id, expectations);
}
