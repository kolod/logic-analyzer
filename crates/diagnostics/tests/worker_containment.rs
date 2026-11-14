//! Worker panic containment (spec US4, FR-012..FR-014, SC-005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::{Arc, Mutex, OnceLock};

use la_diagnostics::{
    LogConfig, LogGuard, init_logging, install_panic_hook, lock_recover, spawn_worker,
};

/// Installs the hook and a global log into a temporary directory once per process, so
/// the many deliberate panics below are logged to a file instead of noisy stderr.
fn setup() {
    static GUARD: OnceLock<LogGuard> = OnceLock::new();
    GUARD.get_or_init(|| {
        let guard = init_logging(LogConfig {
            directory: common::temp_dir("worker-containment"),
            ..LogConfig::default()
        });
        install_panic_hook();
        guard
    });
}

#[test]
fn panicking_worker_is_reported_with_name_message_and_location() {
    setup();
    let failure = spawn_worker("decoder-i2c", || -> u32 { panic!("boom {}", 7) })
        .join()
        .unwrap_err();
    assert_eq!(failure.worker, "decoder-i2c");
    assert!(failure.message.contains("boom 7"), "{failure:?}");
    let location = failure.location.as_deref().expect("location captured");
    assert!(location.contains("worker_containment.rs"), "{location}");
}

#[test]
fn non_string_payload_is_rendered_as_text() {
    setup();
    let failure = spawn_worker("w", || std::panic::panic_any(42_u32))
        .join()
        .unwrap_err();
    assert_eq!(failure.message, "<non-string panic payload>");
}

#[test]
fn string_payload_is_preserved() {
    setup();
    let failure = spawn_worker("w", || std::panic::panic_any(String::from("owned message")))
        .join()
        .unwrap_err();
    assert_eq!(failure.message, "owned message");
}

#[test]
fn successful_worker_returns_its_value() {
    setup();
    assert_eq!(spawn_worker("ok", || 21 * 2).join().unwrap(), 42);
}

#[test]
fn siblings_keep_running_when_one_worker_panics() {
    setup();
    let handles: Vec<_> = (0..6_u32)
        .map(|i| {
            spawn_worker(&format!("sibling-{i}"), move || {
                if i == 3 {
                    panic!("only worker 3 fails");
                }
                i * 10
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join()).collect();
    for (i, result) in results.into_iter().enumerate() {
        if i == 3 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), u32::try_from(i).unwrap() * 10);
        }
    }
}

#[test]
fn shared_state_stays_usable_after_a_worker_panics_while_holding_the_lock() {
    setup();
    let state = Arc::new(Mutex::new(vec![1_u32, 2, 3]));
    let worker_state = Arc::clone(&state);
    let failure = spawn_worker("mutator", move || {
        let mut guard = worker_state.lock().unwrap();
        // Transactional update: build the new value, then swap it in, then fail.
        let mut next = guard.clone();
        next.push(4);
        *guard = next;
        panic!("fails after committing");
    })
    .join();
    assert!(failure.is_err());
    assert!(
        state.is_poisoned(),
        "the std mutex is poisoned by the panic"
    );

    let guard = lock_recover(&state);
    assert_eq!(*guard, vec![1, 2, 3, 4], "last committed state is intact");
    drop(guard);
    assert!(!state.is_poisoned(), "poison is cleared after recovery");
}

#[test]
fn uncommitted_update_leaves_previous_state_in_place() {
    setup();
    let state = Arc::new(Mutex::new(vec![1_u32, 2, 3]));
    let worker_state = Arc::clone(&state);
    let _ = spawn_worker("mutator", move || {
        let guard = worker_state.lock().unwrap();
        let mut next = guard.clone();
        next.push(4);
        // Fails before swapping the new value in: nothing was changed.
        panic!("fails before committing");
    })
    .join();
    assert_eq!(*lock_recover(&state), vec![1, 2, 3]);
}

#[test]
fn a_thousand_panics_never_terminate_the_process() {
    setup();
    for i in 0..1_000_u32 {
        let result = spawn_worker("repeat", move || {
            if i % 2 == 0 {
                panic!("iteration {i}");
            }
            i
        })
        .join();
        assert_eq!(result.is_err(), i % 2 == 0);
    }
}
