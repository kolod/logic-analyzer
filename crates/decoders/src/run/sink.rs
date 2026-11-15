//! What a decoder writes to: validation, counting and the limits.
//!
//! Everything a decoder emits passes through here: text is sanitized, the record is checked
//! against the capture (`Annotation::validate`, `Item::validate`) and a refused record is counted
//! and never stored. Past the annotation count or the memory budget, `annotate` and `emit` return
//! [`Limit`] and the decoder is expected to return at once.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::model::{Annotation, Bounds, Item, Refusal, Target};
use crate::store::{Budget, Coverage, Full, Limits, Snapshot, Store};

/// A limit was reached; nothing more can be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit(pub Full);

/// How many records were refused, by reason.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Refused {
    by_reason: BTreeMap<&'static str, u64>,
}

impl Refused {
    /// All refused records.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.by_reason.values().sum()
    }

    /// The refused records with this reason (`Refusal::name`).
    #[must_use]
    pub fn by(&self, reason: &str) -> u64 {
        self.by_reason.get(reason).copied().unwrap_or(0)
    }

    fn note(&mut self, refusal: Refusal) {
        let slot = self.by_reason.entry(refusal.name()).or_insert(0);
        *slot = slot.saturating_add(1);
    }
}

/// The results of an instance at one moment, for the interface.
#[derive(Debug, Clone)]
pub struct Results {
    /// The annotations of the lane.
    pub lane: Arc<Snapshot>,
    /// The overlay annotations of each channel that has any.
    pub overlays: BTreeMap<u8, Arc<Snapshot>>,
    /// How far the pass has got and whether the results are whole.
    pub coverage: Coverage,
}

/// The output of one pass.
#[derive(Debug)]
pub struct Sink {
    bounds: Bounds,
    limits: Limits,
    budget: Arc<Budget>,
    lane: Store,
    overlays: BTreeMap<u8, Store>,
    items: Vec<Item>,
    item_bytes: u64,
    last_item_end: u64,
    refused: Refused,
    covered: u64,
    incomplete: bool,
    full: Option<Full>,
}

impl Sink {
    /// An empty sink with the instance limits, drawing memory from the shared `budget`.
    #[must_use]
    pub fn new(limits: Limits, budget: Arc<Budget>) -> Self {
        // The store limits are not used for the count: the instance total is checked here.
        let store_limits = Limits {
            max_annotations: usize::MAX,
        };
        Self {
            bounds: Bounds {
                length: 0,
                channels: 0,
            },
            limits,
            lane: Store::new(store_limits, Arc::clone(&budget)),
            overlays: BTreeMap::new(),
            budget,
            items: Vec::new(),
            item_bytes: 0,
            last_item_end: 0,
            refused: Refused::default(),
            covered: 0,
            incomplete: false,
            full: None,
        }
    }

    /// Sets what records are checked against (the snapshot the chunk was cut from).
    pub fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
    }

    /// The annotations stored (lane and overlays).
    #[must_use]
    pub fn annotation_count(&self) -> usize {
        self.overlays
            .values()
            .fold(self.lane.len(), |sum, s| sum.saturating_add(s.len()))
    }

    /// The items stored.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The records refused so far.
    #[must_use]
    pub fn refused(&self) -> &Refused {
        &self.refused
    }

    /// The limit that stopped the pass, if one did.
    #[must_use]
    pub fn full(&self) -> Option<Full> {
        self.full
    }

    /// Draws something: sanitized, validated and stored; an invalid annotation is counted and
    /// dropped (and is not an error for the decoder).
    ///
    /// # Errors
    ///
    /// [`Limit`] when the count or memory limit is reached; nothing is stored then and the
    /// decoder must return.
    pub fn annotate(&mut self, annotation: Annotation) -> Result<(), Limit> {
        let annotation = annotation.sanitized();
        if let Err(refusal) = annotation.validate(&self.bounds) {
            self.refused.note(refusal);
            return Ok(());
        }
        if self.annotation_count() >= self.limits.max_annotations {
            return Err(self.stop(Full::Count));
        }
        let result = match annotation.target {
            Target::Lane => self.lane.insert(annotation),
            Target::Channel(channel) => {
                let budget = Arc::clone(&self.budget);
                self.overlays
                    .entry(channel)
                    .or_insert_with(|| {
                        Store::new(
                            Limits {
                                max_annotations: usize::MAX,
                            },
                            budget,
                        )
                    })
                    .insert(annotation)
            }
        };
        result.map(|_| ()).map_err(|full| self.stop(full))
    }

    /// Hands data to the decoder above in a stack; refused when invalid, like an annotation.
    ///
    /// # Errors
    ///
    /// [`Limit`] as for [`Sink::annotate`].
    pub fn emit(&mut self, item: Item) -> Result<(), Limit> {
        if let Err(refusal) = item.validate(&self.bounds, self.last_item_end) {
            self.refused.note(refusal);
            return Ok(());
        }
        let cost = u64::try_from(std::mem::size_of::<Item>())
            .unwrap_or(u64::MAX)
            .saturating_add(match &item.payload {
                crate::model::Payload::Integer(_) => 0,
                crate::model::Payload::Bytes(b) => u64::try_from(b.len()).unwrap_or(0),
                crate::model::Payload::Text(t) => u64::try_from(t.len()).unwrap_or(0),
            });
        if !self.budget.reserve(cost) {
            return Err(self.stop(Full::Memory));
        }
        self.item_bytes = self.item_bytes.saturating_add(cost);
        self.last_item_end = item.end;
        self.items.push(item);
        Ok(())
    }

    fn stop(&mut self, full: Full) -> Limit {
        self.full = Some(full);
        Limit(full)
    }

    /// Records how far the pass has got and whether the results are whole.
    pub fn set_coverage(&mut self, covered: u64, incomplete: bool) {
        self.covered = covered;
        self.incomplete = incomplete;
    }

    /// What the interface may read now.
    #[must_use]
    pub fn results(&mut self) -> Results {
        let coverage = Coverage {
            covered: self.covered,
            incomplete: self.incomplete,
            refused: self.refused.total(),
        };
        self.lane.set_coverage(coverage);
        let mut overlays = BTreeMap::new();
        for (channel, store) in &mut self.overlays {
            store.set_coverage(coverage);
            overlays.insert(*channel, store.snapshot());
        }
        Results {
            lane: self.lane.snapshot(),
            overlays,
            coverage,
        }
    }

    /// Merges the stores of a finished pass into compact runs.
    pub fn compact(&mut self) {
        self.lane.compact();
        for store in self.overlays.values_mut() {
            store.compact();
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        self.budget.release(self.item_bytes);
    }
}
