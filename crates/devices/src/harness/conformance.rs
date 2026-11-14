//! The shared conformance suite: the rules every device must obey, as executable checks.
//!
//! A driver author adds one test that calls [`run_all`] with the driver's provider (backed by a
//! recorded transport or a simulated device) and gets the whole contract checked, with a
//! failure message that names the broken rule:
//!
//! ```
//! use std::sync::Arc;
//! use la_devices::harness::conformance::{Expectations, run_all};
//! use la_devices::{DeviceId, VirtualProvider};
//!
//! // For a real driver pass `Arc::new(MyProvider::new(recorded_transport))` and its id.
//! run_all(
//!     Arc::new(VirtualProvider),
//!     &DeviceId::new("virtual", "0"),
//!     Expectations::default(),
//! );
//! ```
//!
//! Each check returns `Err("[rule] explanation")` instead of panicking, so the suite itself can
//! be tested against deliberately broken devices.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::acquisition::{Acquisition, AcquisitionOptions, AcquisitionOutcome, EndReason};
use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::config::{DeviceConfig, DeviceOptions};
use crate::driver::DeviceProvider;
use crate::error;
use crate::registry::{DeviceRegistry, RegistryConfig};
use crate::virtual_device::PROVIDER_NAME as VIRTUAL_PROVIDER_NAME;

/// What the suite may assume about the device under test.
#[derive(Debug, Clone)]
pub struct Expectations {
    /// Length of the finite capture check; `None` skips it (for devices that cannot produce
    /// an arbitrary number of samples, such as a replayed recording).
    pub finite_capture_samples: Option<u64>,
    /// How long the device may take to deliver its first samples or finish a finite capture.
    pub timeout: Duration,
    /// How long it may take to end an acquisition after a stop request.
    pub stop_timeout: Duration,
}

impl Default for Expectations {
    fn default() -> Self {
        Self {
            finite_capture_samples: Some(10_000),
            timeout: Duration::from_secs(10),
            stop_timeout: Duration::from_secs(5),
        }
    }
}

type Provider = Arc<dyn DeviceProvider>;
type Check = Result<(), String>;

fn fail(rule: &str, message: impl std::fmt::Display) -> String {
    format!("[{rule}] {message}")
}

/// A registry that knows `provider` and lists `id`.
fn registry_with(
    rule: &str,
    provider: &Provider,
    id: &DeviceId,
) -> Result<(DeviceRegistry, DeviceDescriptor), String> {
    let registry = DeviceRegistry::new(RegistryConfig {
        scan_interval: Duration::from_millis(10),
        scan_timeout: Duration::from_millis(500),
    });
    // The registry always has the virtual device built in.
    if id.provider() != VIRTUAL_PROVIDER_NAME {
        registry.add_provider(Arc::clone(provider));
    }
    let started = Instant::now();
    loop {
        if let Some(descriptor) = registry.describe(id) {
            return Ok((registry, descriptor));
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err(fail(
                rule,
                format!("`{id}` is not listed by the registry within 5 seconds"),
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A valid configuration: every channel, a rate near 1 MHz, the given length (clamped to the
/// buffer of a device that has one; devices with a buffer cannot capture without a length).
fn config_for(
    rule: &str,
    descriptor: &DeviceDescriptor,
    length: Option<u64>,
) -> Result<DeviceConfig, String> {
    let caps = &descriptor.capabilities;
    let sample_rate_hz = caps
        .sample_rates
        .nearest(1_000_000)
        .ok_or_else(|| fail(rule, "the device declares no sample rate"))?;
    let capture_length = match (length, caps.buffer_depth) {
        (Some(n), Some(depth)) => Some(n.min(depth)),
        (None, Some(depth)) => Some(depth.min(1_000_000)),
        (n, None) => n,
    };
    Ok(DeviceConfig {
        enabled_channels: 1_u32
            .checked_shl(u32::from(caps.channels))
            .map_or(u32::MAX, |bit| bit.wrapping_sub(1)),
        sample_rate_hz,
        trigger: None,
        capture_length,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    })
}

/// Describes how an acquisition failed.
fn failure_text(outcome: &AcquisitionOutcome) -> String {
    outcome.error.as_ref().map_or_else(
        || "failed without an error".to_owned(),
        |e| format!("{}: {}", e.code(), e.detail()),
    )
}

/// The accounting rule: every position is either a delivered sample or a recorded loss.
fn check_accounting(rule: &str, outcome: &AcquisitionOutcome) -> Check {
    if outcome
        .delivered_samples
        .saturating_add(outcome.lost_positions)
        != outcome.positions
    {
        return Err(fail(
            rule,
            format!(
                "{} delivered + {} lost does not add up to {} positions",
                outcome.delivered_samples, outcome.lost_positions, outcome.positions
            ),
        ));
    }
    if !outcome.capture_status.closed {
        return Err(fail(
            rule,
            "the capture was not closed when the acquisition ended",
        ));
    }
    Ok(())
}

/// Starts an acquisition with `config` and waits for its first samples (or its end).
fn start_and_wait_for_samples(
    rule: &str,
    registry: &DeviceRegistry,
    id: &DeviceId,
    config: DeviceConfig,
    expectations: &Expectations,
) -> Result<Acquisition, String> {
    let acquisition = registry
        .start(id, config, AcquisitionOptions::default())
        .map_err(|e| fail(rule, format!("start failed: {}: {}", e.code(), e.detail())))?;
    let started = Instant::now();
    while acquisition.progress().samples_captured == 0 && acquisition.outcome().is_none() {
        if started.elapsed() > expectations.timeout {
            return Err(fail(
                rule,
                format!(
                    "no samples arrived within {:?} of starting",
                    expectations.timeout
                ),
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(acquisition)
}

/// Stops `acquisition` and returns its outcome, which must not be a failure.
fn stop_cleanly(
    rule: &str,
    acquisition: &Acquisition,
    expectations: &Expectations,
) -> Result<AcquisitionOutcome, String> {
    acquisition.stop();
    let outcome = acquisition
        .wait_timeout(expectations.stop_timeout)
        .ok_or_else(|| {
            fail(
                rule,
                format!(
                    "the acquisition did not end within {:?} of the stop request",
                    expectations.stop_timeout
                ),
            )
        })?;
    if outcome.end == EndReason::Failed {
        return Err(fail(
            rule,
            format!("the acquisition failed: {}", failure_text(&outcome)),
        ));
    }
    check_accounting(rule, &outcome)?;
    Ok(outcome)
}

/// Rule `descriptor`: the provider lists the device and its description is consistent
/// (a name, 1 to 32 channels, valid sample rates).
///
/// # Errors
///
/// `[descriptor] ...` naming the problem.
pub fn check_descriptor(provider: &Provider, id: &DeviceId) -> Check {
    let listed = provider.scan().map_err(|e| {
        fail(
            "descriptor",
            format!("scan failed: {}: {}", e.code(), e.detail()),
        )
    })?;
    let descriptor = listed.iter().find(|d| d.id == *id).ok_or_else(|| {
        fail(
            "descriptor",
            format!("`{id}` is not among the scanned devices"),
        )
    })?;
    descriptor
        .validate()
        .map_err(|e| fail("descriptor", e.detail()))
}

/// Rule `capability-enforcement`: a configuration outside the declared capabilities is
/// rejected with a typed error before it reaches the device.
///
/// # Errors
///
/// `[capability-enforcement] ...` naming the configuration that was wrongly accepted.
pub fn check_capability_enforcement(provider: &Provider, id: &DeviceId) -> Check {
    let rule = "capability-enforcement";
    let (registry, descriptor) = registry_with(rule, provider, id)?;
    let base = config_for(rule, &descriptor, Some(1_000))?;
    let caps = &descriptor.capabilities;
    let mut cases: Vec<(String, DeviceConfig, &'static str)> = vec![(
        "no channel enabled".to_owned(),
        DeviceConfig {
            enabled_channels: 0,
            ..base.clone()
        },
        error::INVALID_CHANNEL,
    )];
    if let Some(bit) = 1_u32.checked_shl(u32::from(caps.channels)) {
        cases.push((
            format!(
                "channel {} on a {}-channel device",
                caps.channels, caps.channels
            ),
            DeviceConfig {
                enabled_channels: bit,
                ..base.clone()
            },
            error::INVALID_CHANNEL,
        ));
    }
    if let Some(rate) = [3_u64, 7, 13, 999_999_937]
        .into_iter()
        .find(|r| !caps.sample_rates.contains(*r))
    {
        cases.push((
            format!("unsupported sample rate {rate} Hz"),
            DeviceConfig {
                sample_rate_hz: rate,
                ..base.clone()
            },
            error::UNSUPPORTED_SAMPLE_RATE,
        ));
    }
    for (what, config, code) in cases {
        match registry.start(id, config, AcquisitionOptions::default()) {
            Ok(acquisition) => {
                acquisition.stop();
                return Err(fail(rule, format!("{what} was accepted")));
            }
            Err(e) if e.code() == code => {}
            Err(e) => {
                return Err(fail(
                    rule,
                    format!("{what} was rejected with {} instead of {code}", e.code()),
                ));
            }
        }
    }
    Ok(())
}

/// Rule `start-stop`: an acquisition starts, delivers samples, ends promptly when stopped, and
/// its books balance.
///
/// # Errors
///
/// `[start-stop] ...` naming the problem.
pub fn check_start_stop(provider: &Provider, id: &DeviceId, expectations: &Expectations) -> Check {
    let rule = "start-stop";
    let (registry, descriptor) = registry_with(rule, provider, id)?;
    let config = config_for(rule, &descriptor, None)?;
    let acquisition = start_and_wait_for_samples(rule, &registry, id, config, expectations)?;
    if let Some(outcome) = acquisition.outcome() {
        // The device ended by itself (a finite recording): that must not be a failure.
        if outcome.end == EndReason::Failed {
            return Err(fail(
                rule,
                format!("the acquisition failed: {}", failure_text(&outcome)),
            ));
        }
        return check_accounting(rule, &outcome);
    }
    stop_cleanly(rule, &acquisition, expectations).map(|_| ())
}

/// Rule `finite-capture`: a capture of `samples` samples ends by itself with exactly that many
/// positions, and its books balance.
///
/// # Errors
///
/// `[finite-capture] ...` naming the problem.
pub fn check_finite_capture(
    provider: &Provider,
    id: &DeviceId,
    samples: u64,
    expectations: &Expectations,
) -> Check {
    let rule = "finite-capture";
    let (registry, descriptor) = registry_with(rule, provider, id)?;
    let config = config_for(rule, &descriptor, Some(samples))?;
    let wanted = config.capture_length.unwrap_or(samples);
    let acquisition = registry
        .start(id, config, AcquisitionOptions::default())
        .map_err(|e| fail(rule, format!("start failed: {}: {}", e.code(), e.detail())))?;
    let outcome = acquisition
        .wait_timeout(expectations.timeout)
        .ok_or_else(|| {
            fail(
                rule,
                format!("the capture did not end within {:?}", expectations.timeout),
            )
        })?;
    if outcome.end != EndReason::Completed {
        return Err(fail(
            rule,
            format!(
                "the capture ended as {:?} instead of Completed ({})",
                outcome.end,
                outcome.error.as_ref().map_or_else(
                    || "no error".to_owned(),
                    |e| format!("{}: {}", e.code(), e.detail())
                )
            ),
        ));
    }
    if outcome.positions != wanted {
        return Err(fail(
            rule,
            format!(
                "asked for {wanted} samples but the capture holds {} positions",
                outcome.positions
            ),
        ));
    }
    check_accounting(rule, &outcome)
}

/// Rule `failure-containment`: after an acquisition ended (stopped), the device is released and
/// listed, the capture that was made is intact, and a new acquisition starts on the same
/// device.
///
/// # Errors
///
/// `[failure-containment] ...` naming the problem.
pub fn check_failure_containment(
    provider: &Provider,
    id: &DeviceId,
    expectations: &Expectations,
) -> Check {
    let rule = "failure-containment";
    let (registry, descriptor) = registry_with(rule, provider, id)?;
    for round in 1..=2_u32 {
        let config = config_for(rule, &descriptor, None)?;
        let acquisition = start_and_wait_for_samples(rule, &registry, id, config, expectations)
            .map_err(|m| format!("{m} (acquisition {round} of 2)"))?;
        let outcome = match acquisition.outcome() {
            Some(outcome) if outcome.end == EndReason::Failed => {
                return Err(fail(
                    rule,
                    format!(
                        "acquisition {round} of 2 failed: {}",
                        failure_text(&outcome)
                    ),
                ));
            }
            Some(outcome) => outcome,
            None => stop_cleanly(rule, &acquisition, expectations)?,
        };
        check_accounting(rule, &outcome)?;
        let snapshot = acquisition.capture().snapshot();
        if snapshot.length() != outcome.positions {
            return Err(fail(
                rule,
                format!(
                    "the capture holds {} positions but the outcome reports {}",
                    snapshot.length(),
                    outcome.positions
                ),
            ));
        }
        if registry.describe(id).is_none() {
            return Err(fail(
                rule,
                "the device vanished from the list after an acquisition",
            ));
        }
    }
    Ok(())
}

/// Runs every check against the device `id` of `provider` and panics, listing every broken
/// rule, if any fails.
///
/// `expectations.finite_capture_samples` of `None` skips the finite capture check.
///
/// # Panics
///
/// If any check fails; this is the function a driver's test calls.
#[allow(
    clippy::panic,
    reason = "a failing conformance suite must fail the calling test"
)]
pub fn run_all(provider: Arc<dyn DeviceProvider>, id: &DeviceId, expectations: Expectations) {
    let mut failures = Vec::new();
    let mut record = |result: Check| {
        if let Err(message) = result {
            failures.push(message);
        }
    };
    record(check_descriptor(&provider, id));
    record(check_capability_enforcement(&provider, id));
    record(check_start_stop(&provider, id, &expectations));
    if let Some(samples) = expectations.finite_capture_samples {
        record(check_finite_capture(&provider, id, samples, &expectations));
    }
    record(check_failure_containment(&provider, id, &expectations));
    assert!(
        failures.is_empty(),
        "the device `{id}` breaks the device contract:\n  {}",
        failures.join("\n  ")
    );
}
