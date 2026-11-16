//! The persisted forms of decoder setups and results: the `DECO` and `DANN` sections of the session
//! file (`contracts/deco-section.md`).
//!
//! Both are plain, bounded, canonical payloads: the same state gives the same bytes, a reader
//! accepts only the shortest form of every number, and any payload it accepts is written back
//! unchanged. This module knows nothing about files; the host puts the payloads into the sections.

pub mod dann;
pub mod deco;
pub mod varint;

use std::collections::BTreeSet;
use std::sync::Mutex;

use la_diagnostics::lock_recover;

pub use varint::Invalid;

/// Why a section was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// A rule of the contract is broken.
    Invalid(Invalid),
    /// The payload is of a newer version: it is kept as it is and not interpreted.
    Newer,
}

/// The most distinct strings kept for the life of the process (see [`intern`]).
const MAX_INTERNED: usize = 65_536;

/// A string that lives for the whole process.
///
/// Message keys and item kinds of annotations are `&'static str` (a decoder writes them as string
/// literals). Keys read from a file are made static here, deduplicated and bounded, so that opening
/// many files cannot grow memory without limit; past the bound `None` is returned and the section
/// is rejected.
#[must_use]
pub fn intern(text: &str) -> Option<&'static str> {
    static TABLE: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut table = lock_recover(&TABLE);
    if let Some(found) = table.get(text) {
        return Some(found);
    }
    if table.len() >= MAX_INTERNED {
        return None;
    }
    let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
    table.insert(leaked);
    Some(leaked)
}

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::model::{Annotation, Item, Target};
use crate::run::Results;
use crate::stack::ItemStream;
use crate::store::{Budget, Coverage, Full, Limits, Store};

/// The saved block of an instance: its results (lane and overlays merged and sorted by start, the
/// order inside equal starts kept) and its items.
#[must_use]
pub fn block_of(
    results: &Results,
    items: Vec<Item>,
    instance: u32,
    setup_version: u64,
    incomplete: bool,
) -> dann::Block {
    let mut annotations: Vec<Annotation> = results
        .lane
        .window(0, u64::MAX)
        .iter()
        .map(|e| e.annotation.clone())
        .collect();
    for snapshot in results.overlays.values() {
        annotations.extend(
            snapshot
                .window(0, u64::MAX)
                .iter()
                .map(|e| e.annotation.clone()),
        );
    }
    annotations.sort_by_key(|a| a.span().0);
    dann::Block {
        instance,
        setup_version,
        covered: results.coverage.covered,
        incomplete: incomplete || results.coverage.incomplete,
        annotations,
        items,
    }
}

/// Results rebuilt from a saved block, with the stream of its items (ended: the pass that made
/// them is over), drawing memory from `budget`.
///
/// # Errors
///
/// [`Full`] if the block does not fit the budget.
pub fn results_of(
    block: &dann::Block,
    budget: &Arc<Budget>,
) -> Result<(Results, ItemStream), Full> {
    let unlimited = Limits {
        max_annotations: usize::MAX,
    };
    let mut lane = Store::new(unlimited, Arc::clone(budget));
    let mut overlays: BTreeMap<u8, Store> = BTreeMap::new();
    for annotation in &block.annotations {
        let store = match annotation.target {
            Target::Lane => &mut lane,
            Target::Channel(c) => overlays
                .entry(c)
                .or_insert_with(|| Store::new(unlimited, Arc::clone(budget))),
        };
        store.insert(annotation.clone())?;
    }
    let coverage = Coverage {
        covered: block.covered,
        incomplete: block.incomplete,
        refused: 0,
    };
    lane.set_coverage(coverage);
    lane.compact();
    let mut snapshots = BTreeMap::new();
    for (channel, store) in &mut overlays {
        store.set_coverage(coverage);
        store.compact();
        snapshots.insert(*channel, store.snapshot());
    }
    let results = Results {
        lane: lane.snapshot(),
        overlays: snapshots,
        coverage,
    };
    let stream = ItemStream::new();
    for item in &block.items {
        stream.push(item.clone());
    }
    stream.set_covered(block.covered);
    stream.end(!block.incomplete);
    Ok((results, stream))
}
