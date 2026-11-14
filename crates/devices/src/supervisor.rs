//! The two threads of an acquisition.
//!
//! * The **driver thread** is the only code that calls the driver. It opens and starts the
//!   device, reads samples until it is told to stop (or the device finishes or fails) and
//!   hands them to the ingest thread through a bounded channel.
//! * The **ingest thread** never calls the driver. It owns the `la_storage::Capture`, appends
//!   the chunks, closes the capture and publishes the outcome.
//!
//! This separation keeps the user's data on a thread that a misbehaving driver cannot hang.

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::{Error, Result};
use la_diagnostics::spawn_worker;
use la_storage::{Capture, CaptureConfig};

use crate::acquisition::{
    Acquisition, AcquisitionEvent, AcquisitionOptions, AcquisitionOutcome, AcquisitionState,
    Control, EndReason,
};
use crate::capabilities::DeviceDescriptor;
use crate::config::{AppliedConfig, DeviceOptions, ResolvedConfig};
use crate::description::{CaptureDescription, DescriptionParts};
use crate::driver::{DeviceProvider, ReadOutcome};
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

/// What the driver thread tells the ingest thread.
enum Message {
    Started(AppliedConfig),
    Samples(Vec<u32>),
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
    });

    let (event_sender, event_receiver) = sync_channel(256);
    let control = Arc::new(Control::new(description, event_sender));
    control.beat();
    let (sender, receiver) = sync_channel::<Message>(options.channel_capacity.max(1));
    let chunk = options
        .chunk_samples
        .min(usize::try_from(options.limits.max_chunk_samples).unwrap_or(usize::MAX))
        .max(1);

    let driver_control = Arc::clone(&control);
    let device_id = descriptor.id.clone();
    drop(spawn_worker(&format!("driver-{device_id}"), move || {
        driver_main(provider, device_id, config, chunk, driver_control, sender)
    }));

    let ingest_control = Arc::clone(&control);
    let max_chunk = usize::try_from(options.limits.max_chunk_samples).unwrap_or(usize::MAX);
    let device_name = descriptor.id.to_string();
    drop(spawn_worker(&format!("ingest-{device_name}"), move || {
        ingest_main(capture, receiver, max_chunk, ingest_control, release);
    }));

    Ok(Acquisition {
        control,
        events: Mutex::new(event_receiver),
        reader,
    })
}

/// The driver thread: the only place where the driver is called.
fn driver_main(
    provider: Arc<dyn DeviceProvider>,
    id: crate::capabilities::DeviceId,
    config: ResolvedConfig,
    chunk: usize,
    control: Arc<Control>,
    sender: SyncSender<Message>,
) {
    let mut device = match provider.open(&id) {
        Ok(device) => device,
        Err(error) => {
            let _ = sender.send(Message::Failed(error));
            return;
        }
    };
    control.beat();
    let applied = match device.start(&config) {
        Ok(applied) => applied,
        Err(error) => {
            let _ = device.stop();
            let _ = sender.send(Message::Failed(error));
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

    let mut buffer: Vec<u32> = Vec::with_capacity(chunk);
    let end = loop {
        control.beat();
        if control.stop.load(std::sync::atomic::Ordering::Acquire) {
            break DriverEnd::Stopped;
        }
        buffer.clear();
        match device.read(chunk, &mut buffer) {
            Ok(ReadOutcome::Samples) => {
                if !buffer.is_empty() {
                    let full = std::mem::replace(&mut buffer, Vec::with_capacity(chunk));
                    if sender.send(Message::Samples(full)).is_err() {
                        break DriverEnd::Abandoned;
                    }
                }
            }
            // Losses and idle reads are handled in a later phase (failure handling).
            Ok(ReadOutcome::Lost(_) | ReadOutcome::Idle) => {}
            Ok(ReadOutcome::Finished) => break DriverEnd::Finished,
            Err(error) => break DriverEnd::Failed(error),
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

/// Appends `samples` in chunks the store accepts; counts every stored chunk as delivered.
fn append_all(
    capture: &mut Capture,
    samples: &[u32],
    max_chunk: usize,
    control: &Control,
    delivered: &mut u64,
) -> Result<()> {
    for part in samples.chunks(max_chunk.max(1)) {
        capture.append(capture.length(), part)?;
        let count = u64::try_from(part.len()).unwrap_or(u64::MAX);
        *delivered = delivered.saturating_add(count);
        control
            .samples
            .store(*delivered, std::sync::atomic::Ordering::Release);
        control
            .positions
            .store(capture.length(), std::sync::atomic::Ordering::Release);
    }
    Ok(())
}

/// The ingest thread: owns the capture and publishes the outcome.
fn ingest_main(
    mut capture: Capture,
    receiver: Receiver<Message>,
    max_chunk: usize,
    control: Arc<Control>,
    release: Box<dyn FnOnce() + Send>,
) {
    let mut delivered = 0_u64;
    let ending = loop {
        match receiver.recv_timeout(SLICE) {
            Ok(Message::Started(applied)) => {
                control.apply_to_description(&applied);
                let state = if control.stop.load(std::sync::atomic::Ordering::Acquire) {
                    AcquisitionState::Stopping
                } else {
                    AcquisitionState::Running
                };
                control.set_state(state);
                control.emit(AcquisitionEvent::Started { applied });
            }
            Ok(Message::Samples(samples)) => {
                if let Err(error) =
                    append_all(&mut capture, &samples, max_chunk, &control, &mut delivered)
                {
                    break Ending::Storage(error);
                }
            }
            Ok(Message::Finished) => break Ending::Finished,
            Ok(Message::Stopped) => break Ending::Stopped,
            Ok(Message::Failed(error)) => break Ending::Failed(error),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                break Ending::Failed(error::driver_error(
                    "the driver thread ended without reporting a result",
                ));
            }
        }
    };

    let (end, error, status) = match ending {
        Ending::Finished => (EndReason::Completed, None, capture.close()),
        Ending::Stopped => (EndReason::Stopped, None, capture.close()),
        Ending::Failed(error) => (EndReason::Failed, Some(error), capture.close_interrupted()),
        Ending::Storage(error) => {
            // Ask the driver to stop; a storage-limit stop keeps its own flag in the status.
            control
                .stop
                .store(true, std::sync::atomic::Ordering::Release);
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
    let event = match &error {
        Some(error) => AcquisitionEvent::Failed { code: error.code() },
        None => AcquisitionEvent::Finished,
    };
    control.finish(AcquisitionOutcome {
        end,
        error: error.map(Arc::new),
        capture_status,
        positions,
        delivered_samples: delivered,
        lost_positions: capture_status.lost_positions,
    });
    control.emit(event);
    release();
}
