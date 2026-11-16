//! The annotation store of one decoder instance (`data-model.md`, "Annotation store"; research R4).
//!
//! Annotations arrive in the order a decoder emits them, which is roughly but not exactly by
//! start. They are kept in an **index** built by the logarithmic method: a small unsorted *tail*
//! (at most [`TAIL_MAX`] entries) that is turned into an immutable sorted *run* with a
//! maximum-end tree when it is full, and runs that are merged geometrically so that there are
//! always at most [`RUNS_MAX`] of them. A window query costs `O(R · log n + k)`; inserting is
//! amortised `O(log n)`; a snapshot for the interface is a handful of `Arc` clones plus a copy of
//! the tail.
//!
//! The store counts annotations and bytes and refuses an insert past its limits (the shared
//! [`Budget`] is the memory limit of all instances together).

mod runs;
mod tree;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::model::Annotation;
use runs::Run;

/// The most entries of the unsorted tail.
pub const TAIL_MAX: usize = 4_096;
/// The most runs of the index.
pub const RUNS_MAX: usize = 16;
/// The default number of annotations one instance may store.
pub const DEFAULT_MAX_ANNOTATIONS: usize = 5_000_000;
/// The default decoder results memory limit, in bytes (256 MiB).
pub const DEFAULT_MEMORY_LIMIT: u64 = 256 * 1024 * 1024;
/// Bytes counted per entry for the index (tree and prefix counts) on top of the entry itself.
const INDEX_OVERHEAD: u64 = 16;

/// One stored annotation with its identifier (the order of insertion, from 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The identifier, unique in the store and never reused; selection and navigation use it.
    pub id: u64,
    /// The annotation.
    pub annotation: Annotation,
}

impl Entry {
    /// The first position of the annotation.
    #[must_use]
    pub fn start(&self) -> u64 {
        self.annotation.span().0
    }

    /// The end the index compares with: the real end, or one past the start for a point, an
    /// edge and a zero-length range (they occupy their sample).
    #[must_use]
    pub fn reach(&self) -> u64 {
        let (start, end) = self.annotation.span();
        end.max(start.saturating_add(1))
    }

    /// The bytes this entry is counted as.
    #[must_use]
    pub fn cost(&self) -> u64 {
        use crate::model::Text;
        let text = match &self.annotation.text {
            Text::Literal(text) => text.len(),
            Text::Message { args, .. } => args.iter().fold(0_usize, |sum, a| {
                sum.saturating_add(a.len()).saturating_add(24)
            }),
        };
        let entry = u64::try_from(std::mem::size_of::<Entry>()).unwrap_or(u64::MAX);
        entry
            .saturating_add(u64::try_from(text).unwrap_or(u64::MAX))
            .saturating_add(INDEX_OVERHEAD)
    }

    fn intersects(&self, from: u64, to: u64) -> bool {
        self.start() < to && self.reach() > from
    }
}

/// Which limit refused an insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Full {
    /// The number of annotations of the instance.
    Count,
    /// The memory budget.
    Memory,
}

/// The memory budget shared by every store (the decoder results memory limit).
#[derive(Debug)]
pub struct Budget {
    limit: AtomicU64,
    used: AtomicU64,
}

impl Budget {
    /// A budget of `limit` bytes.
    #[must_use]
    pub fn new(limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit: AtomicU64::new(limit),
            used: AtomicU64::new(0),
        })
    }

    /// Changes the limit (stores that already hold more keep what they have).
    pub fn set_limit(&self, limit: u64) {
        self.limit.store(limit, Ordering::Relaxed);
    }

    /// The limit.
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.limit.load(Ordering::Relaxed)
    }

    /// The bytes in use.
    #[must_use]
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// Applies `change` to the bytes in use with a compare-and-swap loop (`fetch_update` is
    /// renamed in newer toolchains, so the loop is written out to build on both).
    fn update(&self, change: impl Fn(u64) -> Option<u64>) -> bool {
        let mut current = self.used.load(Ordering::Acquire);
        loop {
            let Some(next) = change(current) else {
                return false;
            };
            match self.used.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    /// Takes `bytes` from the budget; `false` (and nothing taken) if it would pass the limit.
    pub fn reserve(&self, bytes: u64) -> bool {
        let limit = self.limit();
        self.update(|used| {
            let next = used.saturating_add(bytes);
            (next <= limit).then_some(next)
        })
    }

    /// Gives `bytes` back.
    pub fn release(&self, bytes: u64) {
        self.update(|used| Some(used.saturating_sub(bytes)));
    }
}

/// The limits of one store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most annotations.
    pub max_annotations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_annotations: DEFAULT_MAX_ANNOTATIONS,
        }
    }
}

/// The result of a rank query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rank {
    /// How many annotations start before the position.
    pub count: u64,
    /// How many of them are errors.
    pub errors: u64,
}

/// What a snapshot knows about how much of the capture the results cover.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Everything before this position has been processed.
    pub covered: u64,
    /// The pass was cut or failed: the results are not whole.
    pub incomplete: bool,
    /// The number of records the framework refused (never stored).
    pub refused: u64,
}

/// The annotation store of one instance.
#[derive(Debug)]
pub struct Store {
    limits: Limits,
    budget: Arc<Budget>,
    runs: Vec<Arc<Run>>,
    tail: Vec<Entry>,
    next_id: u64,
    bytes: u64,
    errors: u64,
    coverage: Coverage,
    cache: Option<Arc<Snapshot>>,
}

impl Store {
    /// An empty store drawing its memory from `budget`.
    #[must_use]
    pub fn new(limits: Limits, budget: Arc<Budget>) -> Self {
        Self {
            limits,
            budget,
            runs: Vec::new(),
            tail: Vec::new(),
            next_id: 0,
            bytes: 0,
            errors: 0,
            coverage: Coverage::default(),
            cache: None,
        }
    }

    /// The number of annotations.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::try_from(self.next_id).unwrap_or(usize::MAX)
    }

    /// Whether the store holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.next_id == 0
    }

    /// The bytes the store is counted as.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The number of runs of the index.
    #[must_use]
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// The number of entries of the unsorted tail.
    #[must_use]
    pub fn tail_len(&self) -> usize {
        self.tail.len()
    }

    /// Stores an annotation (already validated) and gives its id.
    ///
    /// # Errors
    ///
    /// [`Full`] when the count or memory limit would be exceeded; nothing is stored then.
    pub fn insert(&mut self, annotation: Annotation) -> Result<u64, Full> {
        if self.len() >= self.limits.max_annotations {
            return Err(Full::Count);
        }
        let entry = Entry {
            id: self.next_id,
            annotation,
        };
        let cost = entry.cost();
        if !self.budget.reserve(cost) {
            return Err(Full::Memory);
        }
        self.bytes = self.bytes.saturating_add(cost);
        if entry.annotation.error {
            self.errors = self.errors.saturating_add(1);
        }
        let id = entry.id;
        self.next_id = self.next_id.saturating_add(1);
        // The tail is kept sorted (ids grow, so an entry goes after those with the same start):
        // the common, nearly in-order case appends, and rank and neighbour queries stay logarithmic.
        let at = self.tail.partition_point(|e| e.start() <= entry.start());
        self.tail.insert(at, entry);
        self.cache = None;
        if self.tail.len() >= TAIL_MAX {
            self.flush_tail();
        }
        Ok(id)
    }

    /// Records how far the pass has got and whether the results are whole.
    pub fn set_coverage(&mut self, coverage: Coverage) {
        if self.coverage != coverage {
            self.coverage = coverage;
            self.cache = None;
        }
    }

    /// The coverage as last recorded.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        self.coverage
    }

    /// Turns the tail into a run and merges runs that are of similar size.
    fn flush_tail(&mut self) {
        if self.tail.is_empty() {
            return;
        }
        let entries = std::mem::take(&mut self.tail);
        self.runs.push(Arc::new(Run::build(entries)));
        while self.runs.len() >= 2 {
            let sizes = (
                self.runs
                    .get(self.runs.len().saturating_sub(2))
                    .map(|r| r.len()),
                self.runs.last().map(|r| r.len()),
            );
            let similar =
                matches!(sizes, (Some(older), Some(newer)) if newer.saturating_mul(2) >= older);
            if !similar && self.runs.len() <= RUNS_MAX {
                break;
            }
            self.merge_last_two();
        }
    }

    fn merge_last_two(&mut self) {
        let (Some(newer), Some(older)) = (self.runs.pop(), self.runs.pop()) else {
            return;
        };
        let take = |run: Arc<Run>| match Arc::try_unwrap(run) {
            Ok(run) => run.into_entries(),
            Err(shared) => shared.entries().to_vec(),
        };
        let mut entries = take(older);
        entries.extend(take(newer));
        self.runs.push(Arc::new(Run::build(entries)));
    }

    /// Merges everything into one run (a finished store is compact and immutable).
    pub fn compact(&mut self) {
        self.flush_tail();
        while self.runs.len() > 1 {
            self.merge_last_two();
        }
        self.cache = None;
    }

    /// An immutable view of the store at this moment. Cheap when nothing changed since the last
    /// one.
    #[must_use]
    pub fn snapshot(&mut self) -> Arc<Snapshot> {
        if let Some(cached) = &self.cache {
            return Arc::clone(cached);
        }
        let snapshot = Arc::new(Snapshot {
            runs: self.runs.clone(),
            tail_errors: prefix_errors(&self.tail),
            tail: self.tail.clone(),
            len: self.len(),
            bytes: self.bytes,
            errors: self.errors,
            coverage: self.coverage,
        });
        self.cache = Some(Arc::clone(&snapshot));
        snapshot
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}

/// An immutable view of a store: what the interface and the lane plan read.
#[derive(Debug)]
pub struct Snapshot {
    runs: Vec<Arc<Run>>,
    tail: Vec<Entry>,
    /// `tail_errors[i]` is the number of error annotations among the first `i` tail entries.
    tail_errors: Vec<u32>,
    len: usize,
    bytes: u64,
    errors: u64,
    coverage: Coverage,
}

impl Snapshot {
    /// The number of annotations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether it holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes counted.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The number of error annotations.
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// The coverage.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        self.coverage
    }

    /// The annotations that intersect `[from, to)`, ordered by start (then id). A point, an edge
    /// and a zero-length range occupy their own sample.
    #[must_use]
    pub fn window(&self, from: u64, to: u64) -> Vec<&Entry> {
        self.window_limited(from, to, usize::MAX, &mut 0).0
    }

    /// Like [`Snapshot::window`] but gives up after `limit` annotations (the flag says so: the
    /// answer is then an arbitrary subset) and counts the index probes it made in `probes`.
    #[must_use]
    pub fn window_limited(
        &self,
        from: u64,
        to: u64,
        limit: usize,
        probes: &mut u64,
    ) -> (Vec<&Entry>, bool) {
        if from >= to {
            return (Vec::new(), false);
        }
        self.collect(from, to, limit, probes)
    }

    /// The annotations that start before `position` and reach beyond it (ranges that cross it),
    /// at most `limit` of them (the flag says there are more), ordered by start.
    #[must_use]
    pub fn crossing(&self, position: u64, limit: usize, probes: &mut u64) -> (Vec<&Entry>, bool) {
        self.collect(position, position, limit, probes)
    }

    /// The entries with `start < to` and `reach > from` (no check that the window is not empty).
    fn collect(&self, from: u64, to: u64, limit: usize, probes: &mut u64) -> (Vec<&Entry>, bool) {
        let mut found: Vec<&Entry> = Vec::new();
        let mut truncated = false;
        for run in &self.runs {
            let completed = run.window(from, to, probes, &mut |entry| {
                if found.len() >= limit {
                    truncated = true;
                    return false;
                }
                found.push(entry);
                true
            });
            if !completed {
                break;
            }
        }
        if !truncated {
            for entry in &self.tail {
                *probes = probes.saturating_add(1);
                if entry.intersects(from, to) {
                    if found.len() >= limit {
                        truncated = true;
                        break;
                    }
                    found.push(entry);
                }
            }
        }
        found.sort_by_key(|e| (e.start(), e.id));
        (found, truncated)
    }

    /// How many annotations start before `position`, and how many of those are errors.
    #[must_use]
    pub fn rank(&self, position: u64, probes: &mut u64) -> Rank {
        let mut count = 0_u64;
        let mut errors = 0_u64;
        for run in &self.runs {
            let (c, e) = run.rank(position, probes);
            count = count.saturating_add(u64::try_from(c).unwrap_or(u64::MAX));
            errors = errors.saturating_add(u64::from(e));
        }
        let in_tail = self.tail.partition_point(|e| e.start() < position);
        *probes = probes.saturating_add(u64::from(
            usize::BITS.saturating_sub(in_tail.leading_zeros()),
        ));
        count = count.saturating_add(u64::try_from(in_tail).unwrap_or(u64::MAX));
        errors = errors.saturating_add(u64::from(
            self.tail_errors.get(in_tail).copied().unwrap_or(0),
        ));
        Rank { count, errors }
    }

    /// The annotation with this id that starts at `start`.
    #[must_use]
    pub fn find(&self, start: u64, id: u64) -> Option<&Entry> {
        let mut probes = 0;
        let in_tail = self
            .tail
            .binary_search_by_key(&(start, id), |e| (e.start(), e.id))
            .ok()
            .and_then(|i| self.tail.get(i));
        self.runs
            .iter()
            .find_map(|r| r.find(start, id, &mut probes))
            .or(in_tail)
    }

    /// The annotation with the smallest start at or after `position` (ties by id).
    #[must_use]
    pub fn first_from(&self, position: u64, probes: &mut u64) -> Option<&Entry> {
        let from_runs = self
            .runs
            .iter()
            .filter_map(|r| r.first_from(position, probes));
        let first = self.tail.partition_point(|e| e.start() < position);
        from_runs
            .chain(self.tail.get(first))
            .min_by_key(|e| (e.start(), e.id))
    }

    /// The annotation with the smallest start above `position` (ties by id).
    #[must_use]
    pub fn next_after(&self, position: u64, probes: &mut u64) -> Option<&Entry> {
        let from_runs = self.runs.iter().filter_map(|r| r.after(position, probes));
        let first = self.tail.partition_point(|e| e.start() <= position);
        from_runs
            .chain(self.tail.get(first))
            .min_by_key(|e| (e.start(), e.id))
    }

    /// The annotation with the largest start below `position` (ties: the last by id).
    #[must_use]
    pub fn prev_before(&self, position: u64, probes: &mut u64) -> Option<&Entry> {
        let from_runs = self.runs.iter().filter_map(|r| r.before(position, probes));
        let first = self.tail.partition_point(|e| e.start() < position);
        let last = first.checked_sub(1).and_then(|i| self.tail.get(i));
        from_runs.chain(last).max_by_key(|e| (e.start(), e.id))
    }
}

fn prefix_errors(entries: &[Entry]) -> Vec<u32> {
    let mut prefix = Vec::with_capacity(entries.len().saturating_add(1));
    let mut count = 0_u32;
    prefix.push(0);
    for entry in entries {
        if entry.annotation.error {
            count = count.saturating_add(1);
        }
        prefix.push(count);
    }
    prefix
}
