//! The language can be changed at run time, safely (spec US4, FR-015, SC-005, SC-006).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use la_i18n::{Args, Language, Localizer, key};

/// The longest a language switch may take (debug builds are given more room).
#[cfg(debug_assertions)]
const SWITCH_LIMIT: Duration = Duration::from_millis(50);
#[cfg(not(debug_assertions))]
const SWITCH_LIMIT: Duration = Duration::from_millis(10);

fn damaged_text(l: &Localizer, language: Language) -> String {
    l.message_in(language, &key!("error-session-damaged"), &Args::new())
}

#[test]
fn the_next_message_after_a_switch_is_in_the_new_language() {
    let l = Localizer::new(Language::En);
    let expected: Vec<(Language, String)> = Language::ALL
        .iter()
        .map(|language| (*language, damaged_text(&l, *language)))
        .collect();
    let distinct: HashSet<&String> = expected.iter().map(|(_, t)| t).collect();
    assert_eq!(
        distinct.len(),
        6,
        "the six languages write six different texts"
    );
    for _ in 0..100 {
        for (language, text) in &expected {
            let started = Instant::now();
            l.set_language(*language);
            assert!(
                started.elapsed() < SWITCH_LIMIT,
                "a switch took {:?}",
                started.elapsed()
            );
            assert_eq!(l.language(), *language);
            assert_eq!(
                &l.message(&key!("error-session-damaged"), &Args::new()),
                text
            );
        }
    }
}

#[test]
fn clones_share_the_language_and_message_in_ignores_it() {
    let l = Localizer::new(Language::En);
    let clone = l.clone();
    clone.set_language(Language::Uk);
    assert_eq!(l.language(), Language::Uk);
    assert_eq!(
        l.message_in(Language::De, &key!("error-session-cancelled"), &Args::new()),
        Localizer::new(Language::De).message(&key!("error-session-cancelled"), &Args::new())
    );
    assert_eq!(l.language(), Language::Uk, "message_in does not switch");
}

#[test]
fn messages_stay_wholly_in_one_language_while_other_threads_switch() {
    let l = Localizer::new(Language::En);
    let key = key!("error-session-damaged");
    let versions: HashSet<String> = Language::ALL
        .iter()
        .map(|language| l.message_in(*language, &key, &Args::new()))
        .collect();
    let versions = Arc::new(versions);
    let stop = Arc::new(AtomicBool::new(false));
    let switchers: Vec<_> = (0..2)
        .map(|offset| {
            let (l, stop) = (l.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut i = offset;
                while !stop.load(Ordering::Acquire) {
                    l.set_language(Language::from_tag(Language::ALL[i % 6].tag()).unwrap());
                    i += 1;
                }
            })
        })
        .collect();
    let started = Instant::now();
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (l, versions, key) = (l.clone(), Arc::clone(&versions), key.clone());
            std::thread::spawn(move || {
                for _ in 0..75_000 {
                    let text = l.message(&key, &Args::new());
                    assert!(versions.contains(&text), "a mixed or unknown text: {text}");
                }
            })
        })
        .collect();
    for reader in readers {
        reader.join().unwrap();
    }
    stop.store(true, Ordering::Release);
    for switcher in switchers {
        switcher.join().unwrap();
    }
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "300,000 messages took {:?}",
        started.elapsed()
    );
    assert!(l.problems().is_empty(), "{:?}", l.problems());
}
