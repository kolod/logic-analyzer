//! An implicit maximum-end tree over a sorted array.
//!
//! The entries of a run are sorted by start. The tree is stored in an array of the same length:
//! the node of the index range `[lo, hi)` sits at its middle index `(lo + hi) / 2` and holds the
//! largest `end` in the range. A query for "everything that ends after `a`" among the first
//! `limit` entries prunes every subtree whose maximum is not above `a`, so it costs
//! `O(log n + k)` for `k` answers whatever the number of entries outside the window.

/// Builds the tree for the `ends` of a sorted array (`ends[i]` is the end of the entry `i`).
#[must_use]
pub fn build(ends: &[u64]) -> Vec<u64> {
    let mut tree = vec![0_u64; ends.len()];
    fill(&mut tree, ends, 0, ends.len());
    tree
}

fn fill(tree: &mut [u64], ends: &[u64], lo: usize, hi: usize) -> u64 {
    if lo >= hi {
        return 0;
    }
    let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
    let mut max = ends.get(mid).copied().unwrap_or(0);
    max = max.max(fill(tree, ends, lo, mid));
    max = max.max(fill(tree, ends, mid.saturating_add(1), hi));
    if let Some(slot) = tree.get_mut(mid) {
        *slot = max;
    }
    max
}

/// Calls `visit(index)` in increasing index order for every `index < limit` with
/// `ends[index] > above`, as long as `visit` returns `true`. Returns `false` if `visit` stopped
/// the walk. `probes` is incremented for every tree node examined.
pub fn walk(
    tree: &[u64],
    ends: &dyn Fn(usize) -> u64,
    above: u64,
    limit: usize,
    probes: &mut u64,
    visit: &mut dyn FnMut(usize) -> bool,
) -> bool {
    descend(tree, ends, above, limit, 0, tree.len(), probes, visit)
}

#[allow(clippy::too_many_arguments)]
fn descend(
    tree: &[u64],
    ends: &dyn Fn(usize) -> u64,
    above: u64,
    limit: usize,
    lo: usize,
    hi: usize,
    probes: &mut u64,
    visit: &mut dyn FnMut(usize) -> bool,
) -> bool {
    if lo >= hi || lo >= limit {
        return true;
    }
    *probes = probes.saturating_add(1);
    let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
    if tree.get(mid).is_none_or(|max| *max <= above) {
        return true;
    }
    if !descend(tree, ends, above, limit, lo, mid, probes, visit) {
        return false;
    }
    if mid < limit && ends(mid) > above && !visit(mid) {
        return false;
    }
    descend(
        tree,
        ends,
        above,
        limit,
        mid.saturating_add(1),
        hi,
        probes,
        visit,
    )
}
