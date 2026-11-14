//! Frame-time statistics, switched on with `LA_FRAME_STATS=1`.
//!
//! The time to build a frame plan and the time to draw the cached layer are recorded and every
//! five seconds the count, median, 95th percentile and maximum are written to the log (never
//! signal data). The statistics are how the budget of spec SC-002 is checked in the running
//! application.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How often the statistics are written.
const REPORT_EVERY: Duration = Duration::from_secs(5);

struct Stats {
    plan: Vec<u64>,
    draw: Vec<u64>,
    last_report: Instant,
}

fn stats() -> Option<&'static Mutex<Stats>> {
    static STATS: OnceLock<Option<Mutex<Stats>>> = OnceLock::new();
    STATS
        .get_or_init(|| {
            std::env::var("LA_FRAME_STATS")
                .is_ok_and(|v| v == "1")
                .then(|| {
                    Mutex::new(Stats {
                        plan: Vec::new(),
                        draw: Vec::new(),
                        last_report: Instant::now(),
                    })
                })
        })
        .as_ref()
}

/// Which stage of a frame was timed.
#[derive(Debug, Clone, Copy)]
pub enum Stage {
    /// Building the frame plan.
    Plan,
    /// Drawing the cached layer of the signal view.
    Draw,
}

fn percentile(sorted: &[u64], numerator: usize) -> u64 {
    let index = sorted.len().saturating_sub(1).saturating_mul(numerator) / 100;
    sorted.get(index).copied().unwrap_or(0)
}

fn summarize(name: &str, values: &mut Vec<u64>) {
    if values.is_empty() {
        return;
    }
    values.sort_unstable();
    tracing::info!(
        target: "la_app",
        stage = name,
        count = values.len(),
        p50_us = percentile(values, 50),
        p95_us = percentile(values, 95),
        max_us = values.last().copied().unwrap_or(0),
        "frame statistics"
    );
    values.clear();
}

/// Records the time of one stage (does nothing unless `LA_FRAME_STATS=1`).
pub fn record(stage: Stage, elapsed: Duration) {
    let Some(stats) = stats() else {
        return;
    };
    let mut stats = la_diagnostics::lock_recover(stats);
    let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
    match stage {
        Stage::Plan => stats.plan.push(micros),
        Stage::Draw => stats.draw.push(micros),
    }
    if stats.last_report.elapsed() >= REPORT_EVERY {
        stats.last_report = Instant::now();
        summarize("plan", &mut stats.plan);
        summarize("draw", &mut stats.draw);
    }
}
