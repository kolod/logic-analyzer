//! Stacking: how an upper decoder reads what a lower one produced (`data-model.md`, "Output item").
//!
//! The lower instance writes its items into an [`ItemStream`] as it decodes, together with a
//! **watermark**: the position up to which it has finished. An item is only handed to the upper
//! decoder when its end is at or below the watermark, so the upper decoder never sees an item before
//! the lower one is sure of it, and a growing capture is followed in step. When the lower pass ends
//! the stream says whether it ended whole; if it did not, the upper results are marked incomplete
//! (a fault below never makes the one above fail).

use std::sync::{Arc, Mutex};

use la_diagnostics::lock_recover;

use crate::model::Item;
use crate::setup::{InstanceId, Setup, Source};

#[derive(Debug, Default)]
struct Inner {
    items: Vec<Item>,
    covered: u64,
    /// `Some(true)` when the pass ended whole, `Some(false)` when it was cut or failed.
    ended: Option<bool>,
}

/// What a reader got from a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRead {
    /// The new items, in order.
    pub items: Vec<Item>,
    /// The watermark: everything before it is final.
    pub covered: u64,
    /// How the lower pass ended, once it has.
    pub ended: Option<bool>,
}

/// The items of one instance, shared with the instances stacked on it.
#[derive(Debug, Clone, Default)]
pub struct ItemStream {
    inner: Arc<Mutex<Inner>>,
}

impl ItemStream {
    /// An empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an item (its end never goes backwards: the sink checks that).
    pub fn push(&self, item: Item) {
        lock_recover(&self.inner).items.push(item);
    }

    /// Moves the watermark forward.
    pub fn set_covered(&self, covered: u64) {
        let mut inner = lock_recover(&self.inner);
        inner.covered = inner.covered.max(covered);
    }

    /// The lower pass ended (`complete`: all of the capture was processed).
    pub fn end(&self, complete: bool) {
        let mut inner = lock_recover(&self.inner);
        if inner.ended.is_none() {
            inner.ended = Some(complete);
        }
    }

    /// The number of items so far.
    #[must_use]
    pub fn len(&self) -> usize {
        lock_recover(&self.inner).items.len()
    }

    /// Whether there are no items yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// All items so far.
    #[must_use]
    pub fn items(&self) -> Vec<Item> {
        lock_recover(&self.inner).items.clone()
    }

    /// Up to `max` items from index `from` whose end is at or below the watermark (all of them
    /// once the lower pass has ended), with the watermark and how the pass ended.
    #[must_use]
    pub fn read(&self, from: usize, max: usize) -> StreamRead {
        let inner = lock_recover(&self.inner);
        let ended = inner.ended;
        let mut covered = inner.covered;
        let items: Vec<Item> = inner
            .items
            .iter()
            .skip(from)
            .take_while(|item| ended.is_some() || item.end <= inner.covered)
            .take(max.max(1))
            .cloned()
            .collect();
        if let Some(last) = items.last() {
            if ended.is_some() {
                covered = covered.max(last.end);
            }
        }
        StreamRead {
            items,
            covered,
            ended,
        }
    }
}

/// The instances in stack order: every instance after the one it reads from.
#[must_use]
pub fn stack_order(setups: &[Setup]) -> Vec<InstanceId> {
    let mut order: Vec<InstanceId> = Vec::new();
    let mut remaining: Vec<&Setup> = setups.iter().collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        remaining.retain(|setup| {
            let ready = match setup.source {
                Source::Capture => true,
                Source::Instance(below) => {
                    order.contains(&below) || !setups.iter().any(|s| s.instance == below)
                }
            };
            if ready {
                order.push(setup.instance);
            }
            !ready
        });
        if remaining.len() == before {
            // A cycle cannot be built through validation; do not loop on one that was.
            order.extend(remaining.iter().map(|s| s.instance));
            break;
        }
    }
    order
}

/// The instances that read, directly or through others, from `id` (not `id` itself), lower first.
#[must_use]
pub fn dependants(setups: &[Setup], id: InstanceId) -> Vec<InstanceId> {
    let mut found: Vec<InstanceId> = Vec::new();
    let mut frontier = vec![id];
    while let Some(current) = frontier.pop() {
        for setup in setups {
            if setup.source == Source::Instance(current) && !found.contains(&setup.instance) {
                found.push(setup.instance);
                frontier.push(setup.instance);
            }
        }
    }
    let order = stack_order(setups);
    found.sort_by_key(|i| order.iter().position(|o| o == i));
    found
}
