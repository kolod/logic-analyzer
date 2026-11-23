//! The two threads of an acquisition.
//!
//! * The **driver thread** is the only code that calls the driver. It opens and starts the
//!   device, reads samples until it is told to stop (or the device finishes or fails) and
//!   hands them to the ingest thread through a bounded channel. It never waits for the store:
//!   if the store is slower than the device, the excess positions are sent as losses.
//! * The **ingest thread** never calls the driver. It owns the `la_storage::Capture`, appends
//!   the chunks, records gaps, watches the driver's heartbeat, closes the capture and
//!   publishes the outcome.
//!
//! This separation keeps the user's data on a thread that a misbehaving driver cannot hang:
//! a driver that panics ends only its own thread, a driver that hangs is abandoned after the
//! stall timeout, and in every case the capture is closed with everything delivered so far.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::{Error, Result};
use la_diagnostics::{WorkerHandle, spawn_worker};
use la_storage::{Capture, CaptureConfig};

use crate::acquisition::{
    Acquisition, AcquisitionEvent, AcquisitionOptions, AcquisitionOutcome, AcquisitionState,
    Control, EndReason,
};
use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::config::{AppliedConfig, DeviceOptions, ResolvedConfig};
use crate::description::{CaptureDescription, DescriptionParts};
use crate::driver::{DeviceProvider, Flow, ReadOutcome};
use crate::error;

/// How long the ingest thread waits for the next message before checking on the driver.
const SLICE: Duration = Duration::from_millis(20);

/// Everything needed to start an acquisition.
pub(crate) struct Launch {
    pub(crate) descriptor: DeviceDescriptor,
    pub(crate) provider: Arc<dyn DeviceProvider>,
    pub(crate) config: ResolvedConfig,
    pub(crate) options: AcquisitionOptions,
    /// Called once when the acquisition has ended; releases the device.
    pub(crate) release: Box<dyn FnOnce() + Send>,
}

/// What the driver thread tells the ingest thread. Order is preserved: a `Lost` always comes
/// before the samples that follow the lost positions.
enum Message {
    Started(AppliedConfig),
    Samples(Vec<u32>),
    /// Positions that were lost (reported by the device or dropped because the store lagged).
    Lost(u64),
    /// The device completed a finite capture.
    Finished,
    /// The user stop took effect.
    Stopped,
    /// The device failed.
    Failed(Error),
}

/// Why the driver loop ended.
enum DriverEnd {
    Stopped,
    Finished,
    Failed(Error),
    /// The ingest thread is gone; nobody needs the samples any more.
    Abandoned,
}

/// A driver error as the user sees it: errors that are not `device.*` errors are wrapped as
/// `device.driver_error` (the original is kept as the cause and named in the detail).
fn normalize(error: Error) -> Error {
    if error.code().starts_with("device.") {
        return error;
    }
    let detail = format!("the driver reported `{}`: {}", error.code(), error.detail());
    error::driver_error(detail).with_source(error)
}

/// Starts an acquisition: creates the capture, spawns the two threads and returns the handle
/// at once.
pub(crate) fn launch(launch: Launch) -> Result<Acquisition> {
    let Launch {
        descriptor,
        provider,
        config,
        options,
        release,
    } = launch;
    let capture = Capture::new(CaptureConfig {
        channels: config.channels(),
        sample_rate_hz: config.sample_rate_hz(),
        limits: options.limits,
    })?;
    let reader = capture.reader();

    let seed = match config.options() {
        DeviceOptions::Virtual(v) => Some(v.seed),
        _ => None,
    };
    let description = CaptureDescription::new(DescriptionParts {
        device_id: descriptor.id.clone(),
        device_name: descriptor.name.clone(),
        simulated: descriptor.simulated,
        seed,
        sample_rate_hz: config.sample_rate_hz(),
        channels: config.channels(),
        enabled_mask: config.enabled_channels(),
        trigger: config.trigger(),
        pattern: config.pattern(),
        pre_trigger_samples: config.pre_trigger_samples(),
    });

    let (event_sender, event_receiver) = sync_channel(256);
    let control = Arc::new(Control::new(description, event_sender));
    control.beat();
    let (sender, receiver) = sync_channel::<Message>(options.channel_capacity.max(1));
    let max_chunk = usize::try_from(options.limits.max_chunk_samples).unwrap_or(usize::MAX);
    let chunk = options.chunk_samples.min(max_chunk).max(1);

    let stride = la_storage::words_per_sample(config.channels());
    let driver_control = Arc::clone(&control);
    let device_id = descriptor.id.clone();
    let driver_id = device_id.clone();
    let driver = spawn_worker(&format!("driver-{device_id}"), move || {
        driver_main(
            provider,
            &driver_id,
            &config,
            chunk,
            &driver_control,
            &sender,
        );
    });

    let ingest = Ingest {
        capture,
        receiver,
        max_chunk,
        stride,
        stall_timeout: options.stall_timeout,
        device: device_id.clone(),
        driver,
        control: Arc::clone(&control),
        release,
    };
    drop(spawn_worker(&format!("ingest-{device_id}"), move || {
        ingest_main(ingest);
    }));

    Ok(Acquisition {
        control,
        events: Mutex::new(event_receiver),
        reader,
    })
}

/// Sends the pending losses, waiting if necessary. Used before the final message only.
fn flush_losses_blocking(sender: &SyncSender<Message>, pending_lost: &mut u64) {
    if *pending_lost > 0 {
        let _ = sender.send(Message::Lost(*pending_lost));
        *pending_lost = 0;
    }
}

/// Tries to send the pending losses without waiting. Returns `false` if the ingest side is
/// gone.
fn flush_losses(sender: &SyncSender<Message>, pending_lost: &mut u64) -> bool {
    if *pending_lost == 0 {
        return true;
    }
    match sender.try_send(Message::Lost(*pending_lost)) {
        Ok(()) => {
            *pending_lost = 0;
            true
        }
        Err(TrySendError::Full(_)) => true,
        Err(TrySendError::Disconnected(_)) => false,
    }
}

/// Hands a chunk to the ingest thread without ever waiting for it. If the channel is full the
/// chunk's positions become pending losses (and the buffer is reused), so nothing disappears
/// unaccounted. Returns `false` if the ingest side is gone.
fn deliver_samples(
    sender: &SyncSender<Message>,
    samples: Vec<u32>,
    stride: usize,
    pending_lost: &mut u64,
    spare: &mut Vec<u32>,
) -> bool {
    let count = u64::try_from(samples.len().checked_div(stride).unwrap_or(0)).unwrap_or(u64::MAX);
    if !flush_losses(sender, pending_lost) {
        return false;
    }
    if *pending_lost > 0 {
        // The earlier loss is still waiting: these samples must not overtake it.
        *pending_lost = pending_lost.saturating_add(count);
        *spare = samples;
        return true;
    }
    match sender.try_send(Message::Samples(samples)) {
        Ok(()) => true,
        Err(TrySendError::Full(Message::Samples(returned))) => {
            *pending_lost = pending_lost.saturating_add(count);
            *spare = returned;
            true
        }
        Err(TrySendError::Full(_)) => true,
        Err(TrySendError::Disconnected(_)) => false,
    }
}

/// The driver thread: the only place where the driver is called.
fn driver_main(
    provider: Arc<dyn DeviceProvider>,
    id: &DeviceId,
    config: &ResolvedConfig,
    chunk: usize,
    control: &Control,
    sender: &SyncSender<Message>,
) {
    let stride = la_storage::words_per_sample(config.channels());
    let mut device = match provider.open(id) {
        Ok(device) => device,
        Err(error) => {
            let _ = sender.send(Message::Failed(normalize(error)));
            return;
        }
    };
    control.beat();
    let applied = match device.start(config) {
        Ok(applied) => applied,
        Err(error) => {
            let _ = device.stop();
            let _ = sender.send(Message::Failed(normalize(error)));
            return;
        }
    };
    control.beat();
    if applied.sample_rate_hz != config.sample_rate_hz() {
        let _ = device.stop();
        let _ = sender.send(Message::Failed(error::driver_error(format!(
            "the device applied {} Hz instead of the requested {} Hz",
            applied.sample_rate_hz,
            config.sample_rate_hz()
        ))));
        return;
    }
    if sender.send(Message::Started(applied)).is_err() {
        let _ = device.stop();
        return;
    }

    let flow = device.flow();
    let words = chunk.saturating_mul(stride);
    let mut buffer: Vec<u32> = Vec::with_capacity(words);
    let mut pending_lost = 0_u64;
    let end = loop {
        control.beat();
        if control.stop.load(Ordering::Acquire) {
            break DriverEnd::Stopped;
        }
        buffer.clear();
        match device.read(chunk, &mut buffer) {
            Ok(ReadOutcome::Samples) => {
                if !buffer.is_empty() {
                    let full = std::mem::replace(&mut buffer, Vec::with_capacity(words));
                    let alive = match flow {
                        // A device that can wait is held back by the store, so nothing is lost.
                        Flow::Throttleable => {
                            // A pending loss must not be overtaken by the samples after it.
                            flush_losses_blocking(sender, &mut pending_lost);
                            sender.send(Message::Samples(full)).is_ok()
                        }
                        // A device that cannot wait never waits: excess becomes recorded losses.
                        Flow::Realtime => {
                            deliver_samples(sender, full, stride, &mut pending_lost, &mut buffer)
                        }
                    };
                    if !alive {
                        break DriverEnd::Abandoned;
                    }
                    buffer.clear();
                }
            }
            Ok(ReadOutcome::Lost(lost)) => {
                pending_lost = pending_lost.saturating_add(lost);
                if !flush_losses(sender, &mut pending_lost) {
                    break DriverEnd::Abandoned;
                }
            }
            Ok(ReadOutcome::Idle) => {}
            Ok(ReadOutcome::Finished) => break DriverEnd::Finished,
            Err(error) => break DriverEnd::Failed(normalize(error)),
        }
    };
    if let Err(error) = device.stop() {
        tracing::warn!(
            target: "la_devices",
            device = id.as_str(),
            code = error.code(),
            "stopping the device failed"
        );
    }
    if matches!(end, DriverEnd::Abandoned) {
        return;
    }
    // The last messages must arrive, so they wait for room; the ingest thread is consuming.
    flush_losses_blocking(sender, &mut pending_lost);
    let _ = match end {
        DriverEnd::Stopped => sender.send(Message::Stopped),
        DriverEnd::Finished => sender.send(Message::Finished),
        DriverEnd::Failed(error) => sender.send(Message::Failed(error)),
        DriverEnd::Abandoned => Ok(()),
    };
}

/// How the ingest loop ended.
enum Ending {
    Finished,
    Stopped,
    Failed(Error),
    /// The store rejected data.
    Storage(Error),
}

/// Everything the ingest thread owns.
struct Ingest {
    capture: Capture,
    receiver: Receiver<Message>,
    max_chunk: usize,
    /// Words per sample of the capture.
    stride: usize,
    stall_timeout: Duration,
    device: DeviceId,
    driver: WorkerHandle<()>,
    control: Arc<Control>,
    release: Box<dyn FnOnce() + Send>,
}

/// Appends `samples` in chunks the store accepts; counts every stored chunk as delivered.
fn append_all(
    capture: &mut Capture,
    samples: &[u32],
    max_chunk: usize,
    stride: usize,
    control: &Control,
    delivered: &mut u64,
) -> Result<()> {
    let stride = stride.max(1);
    for part in samples.chunks(max_chunk.max(1).saturating_mul(stride)) {
        capture.append(capture.length(), part)?;
        let count = u64::try_from(part.len().checked_div(stride).unwrap_or(0)).unwrap_or(u64::MAX);
        *delivered = delivered.saturating_add(count);
        control.samples.store(*delivered, Ordering::Release);
        control.positions.store(capture.length(), Ordering::Release);
    }
    Ok(())
}

/// The error for a driver thread that ended without reporting a result: a panic (contained
/// by the worker) or, very unlikely, a silent exit.
fn driver_ended_silently(driver: WorkerHandle<()>) -> Error {
    match driver.join() {
        Err(failure) => error::driver_panic(format!(
            "the driver crashed ({failure}); reconnect the device and start again, and report the problem if it repeats"
        )),
        Ok(()) => error::driver_error("the driver thread ended without reporting a result"),
    }
}

/// The ingest thread: owns the capture and publishes the outcome.
fn ingest_main(ingest: Ingest) {
    let Ingest {
        mut capture,
        receiver,
        max_chunk,
        stride,
        stall_timeout,
        device,
        driver,
        control,
        release,
    } = ingest;
    let mut delivered = 0_u64;
    let ending = loop {
        match receiver.recv_timeout(SLICE) {
            Ok(Message::Started(applied)) => {
                control.apply_to_description(&applied);
                let state = if control.stop.load(Ordering::Acquire) {
                    AcquisitionState::Stopping
                } else {
                    AcquisitionState::Running
                };
                control.set_state(state);
                control.emit(AcquisitionEvent::Started { applied });
            }
            Ok(Message::Samples(samples)) => {
                if let Err(error) = append_all(
                    &mut capture,
                    &samples,
                    max_chunk,
                    stride,
                    &control,
                    &mut delivered,
                ) {
                    break Ending::Storage(error);
                }
            }
            Ok(Message::Lost(0)) => {}
            Ok(Message::Lost(length)) => {
                let position = capture.length();
                if let Err(error) = capture.record_gap(length) {
                    break Ending::Storage(error);
                }
                control.positions.store(capture.length(), Ordering::Release);
                control.emit(AcquisitionEvent::GapRecorded { position, length });
            }
            Ok(Message::Finished) => break Ending::Finished,
            Ok(Message::Stopped) => break Ending::Stopped,
            Ok(Message::Failed(error)) => break Ending::Failed(error),
            Err(RecvTimeoutError::Timeout) => {
                let silent = control.silence();
                if silent > stall_timeout {
                    // The driver thread is abandoned (its handle is dropped, not joined).
                    control.stop.store(true, Ordering::Release);
                    break Ending::Failed(error::unresponsive(format!(
                        "the driver did not respond for {} ms; reconnect the device and start again",
                        silent.as_millis()
                    )));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break Ending::Failed(driver_ended_silently(driver));
            }
        }
    };

    let (end, error, status) = match ending {
        Ending::Finished => (EndReason::Completed, None, capture.close()),
        Ending::Stopped => (EndReason::Stopped, None, capture.close()),
        Ending::Failed(error) => (EndReason::Failed, Some(error), capture.close_interrupted()),
        Ending::Storage(error) => {
            // Ask the driver to stop; a storage-limit stop keeps its own flag in the status.
            control.stop.store(true, Ordering::Release);
            let status = if error.code() == "storage.memory_limit" {
                capture.close()
            } else {
                capture.close_interrupted()
            };
            (EndReason::Failed, Some(error), status)
        }
    };
    let capture_status = status.unwrap_or_default();
    let positions = capture.length();
    if let Some(error) = &error {
        tracing::warn!(
            target: "la_devices",
            device = device.as_str(),
            code = error.code(),
            positions,
            delivered,
            lost = capture_status.lost_positions,
            "an acquisition failed"
        );
    }
    let event = match &error {
        Some(error) => AcquisitionEvent::Failed { code: error.code() },
        None => AcquisitionEvent::Finished,
    };
    // Release the device first: once the outcome is visible the device must already be free,
    // so a caller that starts again right after waiting for the outcome never sees `busy`.
    release();
    control.emit(event);
    control.finish(AcquisitionOutcome {
        end,
        error: error.map(Arc::new),
        capture_status,
        positions,
        delivered_samples: delivered,
        lost_positions: capture_status.lost_positions,
    });
}
