//! An immutable run: entries sorted by start with the maximum-end tree and the error prefix counts.

use super::Entry;
use super::tree;

/// Entries sorted by `(start, id)`, never changed after building.
#[derive(Debug)]
pub struct Run {
    entries: Vec<Entry>,
    /// The tree over the effective ends (see [`Entry::reach`]).
    tree: Vec<u64>,
    /// `errors[i]` is the number of error annotations among the first `i` entries.
    errors: Vec<u32>,
}

impl Run {
    /// A run of `entries` (any order).
    #[must_use]
    pub fn build(mut entries: Vec<Entry>) -> Self {
        // The sort is stable and adaptive: merging runs that are already sorted is linear.
        entries.sort_by_key(|e| (e.start(), e.id));
        let reaches: Vec<u64> = entries.iter().map(Entry::reach).collect();
        let tree = tree::build(&reaches);
        let mut errors = Vec::with_capacity(entries.len().saturating_add(1));
        let mut count = 0_u32;
        errors.push(0);
        for entry in &entries {
            if entry.annotation.error {
                count = count.saturating_add(1);
            }
            errors.push(count);
        }
        Self {
            entries,
            tree,
            errors,
        }
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The entries in order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Consumes the run into its entries.
    #[must_use]
    pub fn into_entries(self) -> Vec<Entry> {
        self.entries
    }

    /// How many entries start before `position`, and how many of those are errors.
    #[must_use]
    pub fn rank(&self, position: u64, probes: &mut u64) -> (usize, u32) {
        let count = self.partition(position, probes);
        let errors = self.errors.get(count).copied().unwrap_or(0);
        (count, errors)
    }

    /// The index of the first entry with `start >= position` (binary search).
    fn partition(&self, position: u64, probes: &mut u64) -> usize {
        let (mut lo, mut hi) = (0_usize, self.entries.len());
        while lo < hi {
            *probes = probes.saturating_add(1);
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            if self.entries.get(mid).is_some_and(|e| e.start() < position) {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Calls `visit` for the entries that intersect `[from, to)`, in order, until it returns
    /// `false`. Returns `false` if it was stopped.
    pub fn window<'a>(
        &'a self,
        from: u64,
        to: u64,
        probes: &mut u64,
        visit: &mut dyn FnMut(&'a Entry) -> bool,
    ) -> bool {
        let limit = self.partition(to, probes);
        let reach = |i: usize| self.entries.get(i).map_or(0, Entry::reach);
        tree::walk(&self.tree, &reach, from, limit, probes, &mut |i| {
            self.entries.get(i).is_none_or(&mut *visit)
        })
    }

    /// The first entry with `start > position`.
    #[must_use]
    pub fn after(&self, position: u64, probes: &mut u64) -> Option<&Entry> {
        let first = self.partition(position.saturating_add(1), probes);
        self.entries.get(first)
    }

    /// The last entry with `start < position`.
    #[must_use]
    pub fn before(&self, position: u64, probes: &mut u64) -> Option<&Entry> {
        let first = self.partition(position, probes);
        first.checked_sub(1).and_then(|i| self.entries.get(i))
    }
}
