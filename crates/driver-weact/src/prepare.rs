//! The preparation of one blank board, on a worker of its own.
//!
//! The worker loads the firmware (hold, image, release), waits for the board to come back with
//! the firmware, and checks the version. It never holds the table lock while it talks to the
//! board, so a scan never waits for it and a board that hangs cannot stop another one. Whatever
//! happens, the board leaves the `Preparing` state: a panic is caught by the supervised worker and
//! the guard below reports it.

use std::sync::Arc;

use la_core::Error;
use la_devices::error as device_error;
use la_diagnostics::lock_recover;

use crate::Shared;
use crate::device::check_firmware;
use crate::error;
use crate::firmware;
use crate::protocol::{RENUMERATION_HEAD_START, RENUMERATION_LIMIT, RENUMERATION_POLL};
use crate::usb::BoardKey;

/// Reports the end of the worker to the table, also when the worker panics.
struct Finish {
    shared: Arc<Shared>,
    key: BoardKey,
    reported: bool,
}

impl Finish {
    fn report(&mut self, result: Result<(), Error>) {
        self.reported = true;
        let now = self.shared.timer.now();
        lock_recover(&self.shared.table).finish(&self.key, result, now);
    }
}

impl Drop for Finish {
    fn drop(&mut self) {
        if !self.reported {
            self.report(Err(device_error::transport(
                "the preparation of the board stopped unexpectedly",
            )));
        }
    }
}

/// Prepares the board at `key`.
pub(crate) fn run(shared: Arc<Shared>, key: BoardKey) {
    let mut finish = Finish {
        shared: Arc::clone(&shared),
        key: key.clone(),
        reported: false,
    };
    let started = shared.timer.now();
    let result = prepare(&shared, &key);
    tracing::info!(
        target: "la_driver_weact",
        board = %key,
        ok = result.is_ok(),
        elapsed_ms = u64::try_from(shared.timer.now().saturating_sub(started).as_millis())
            .unwrap_or(u64::MAX),
        "the preparation of the board ended"
    );
    finish.report(result);
}

fn prepare(shared: &Shared, key: &BoardKey) -> Result<(), Error> {
    shared.image.checked()?;
    let mut link = shared.backend.open(key)?;
    firmware::upload(&mut *link, &shared.image)?;
    drop(link);
    let released = shared.timer.now();
    shared.timer.sleep(RENUMERATION_HEAD_START);
    loop {
        // A listing that fails is a moment of trouble on the bus, not the end of the wait.
        let back = shared.backend.list().is_ok_and(|seen| {
            seen.iter()
                .any(|info| info.key == *key && info.is_weact() && info.runs_firmware())
        });
        if back {
            break;
        }
        let waited = shared.timer.now().saturating_sub(released);
        if waited >= RENUMERATION_LIMIT {
            return Err(error::not_renumerated(
                u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
            ));
        }
        shared.timer.sleep(RENUMERATION_POLL);
    }
    let mut link = shared.backend.open(key)?;
    check_firmware(&mut *link)?;
    Ok(())
}
