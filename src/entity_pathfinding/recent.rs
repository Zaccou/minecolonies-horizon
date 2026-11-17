//! The recent-target cache: how a crowd spreads over places it has just used.
//!
//! Java keeps a static map of block positions that recently served as a path
//! target, and adds a penalty to any *new* path ending there
//! (`RecentTargetCache.java`, 93 lines). It is what stops twenty citizens from
//! queuing at the same door: the first one to aim at a block makes the block
//! cost more for the next 200 seconds, so the search prefers a neighbouring
//! block.
//!
//! **Two constants, one rule.** A target lives `EXPIRE_TIME = 1000 * 200`
//! milliseconds (`:31`) and the cleanup removes at most `REMOVALS = 64` entries
//! per call (`:27`), in insertion order, stopping at the first live one
//! (`:80-92`). There is no timer: the sweep happens when somebody asks
//! (`getExtraCost`, `:74`).
//!
//! **Time is an argument.** Java reads `System.currentTimeMillis()`; a port that
//! did the same could not be tested, so every call takes `now` in milliseconds
//! and the caller's clock is the only clock.

use std::collections::{BTreeMap, VecDeque};

use crate::pathfinding::Pos;

/// The lifetime of a target: `EXPIRE_TIME = 1000 * 200` milliseconds
/// (`RecentTargetCache.java:31`).
pub const TARGET_TTL: i64 = 200_000;

/// How many expired entries one sweep removes: `REMOVALS = 64`
/// (`RecentTargetCache.java:27`).
pub const REMOVALS_PER_CLEAN: usize = 64;

/// The penalty Java puts on the block a path just ended at:
/// `RecentTargetCache.add(..., 50)` (`AbstractPathJob.java:490`).
pub const JUST_USED_PENALTY: f64 = 50.0;

/// One cached target: when it expires and what it costs to end there.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    expires_at: i64,
    extra_cost: f64,
}

/// The cache itself: a map for the lookups and a queue for the sweep.
///
/// **Why two structures.** The map alone cannot answer "which is oldest" without
/// a scan; the queue alone cannot answer "what does *this* block cost" without
/// a scan. Java keeps both and reconciles them by identity (`cache.remove(pos,
/// info)`, `:88`), which a value-equal `Entry` reproduces: a stale queue entry
/// for a re-added position does not remove the new one.
#[derive(Clone, Debug, Default)]
pub struct RecentTargets {
    entries: BTreeMap<Pos, Entry>,
    queue: VecDeque<(Pos, Entry)>,
}

impl RecentTargets {
    /// An empty cache.
    pub fn new() -> RecentTargets {
        RecentTargets::default()
    }

    /// How many targets it holds — the map's size, not the queue's, which may
    /// carry superseded entries until the sweep reaches them.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record a target used at `now`, with the penalty a later path pays to end
    /// there — `add(pos, extraCost)` (`:50-60`).
    ///
    /// A second add for the same position replaces the first and queues a fresh
    /// entry: Java's `cache.put` overwrites, and its queue keeps both, which is
    /// why the sweep compares values.
    pub fn add(&mut self, pos: Pos, extra_cost: f64, now: i64) {
        let entry = Entry {
            expires_at: now.saturating_add(TARGET_TTL),
            extra_cost,
        };
        self.entries.insert(pos, entry);
        self.queue.push_back((pos, entry));
    }

    /// The penalty for ending a path at `pos` — `getExtraCost` (`:66-77`), which
    /// sweeps first.
    pub fn extra_cost(&mut self, pos: Pos, now: i64) -> f64 {
        self.clean(now);
        self.entries.get(&pos).map_or(0.0, |e| e.extra_cost)
    }

    /// Remove up to [`REMOVALS_PER_CLEAN`] expired targets, oldest first, and
    /// stop at the first one that is still alive — `cleanup` (`:80-92`).
    ///
    /// **Stopping is the rule, not an optimisation.** The queue is in insertion
    /// order, so the first live entry means every entry behind it is live too:
    /// sweeping the whole queue would be the same answer for more work, and
    /// Java's `break` is what keeps the cost of a lookup bounded at 64.
    pub fn clean(&mut self, now: i64) {
        for _ in 0..REMOVALS_PER_CLEAN {
            let Some((pos, entry)) = self.queue.front().copied() else {
                break;
            };
            if entry.expires_at > now {
                break;
            }
            self.queue.pop_front();
            // Only drop the map entry if it is still this one: a re-add since
            // then owns the position (`cache.remove(pos, info)`, `:88`).
            if self.entries.get(&pos) == Some(&entry) {
                self.entries.remove(&pos);
            }
        }
    }

    /// The penalty the search actually applies, which is Java's
    /// `getEndNodeScoreWithExtraCost` (`AbstractPathJob.java:985-990`): the
    /// job's own score plus whatever this cache says about the block.
    pub fn score(&mut self, job_score: f64, pos: Pos, now: i64) -> f64 {
        job_score + self.extra_cost(pos, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::at;

    #[test]
    fn an_unknown_target_costs_nothing() {
        let mut cache = RecentTargets::new();
        assert_eq!(cache.extra_cost(at(1, 2, 3), 0), 0.0);
        assert!(cache.is_empty());
    }

    #[test]
    fn a_target_costs_what_it_was_added_with_until_it_expires() {
        let mut cache = RecentTargets::new();
        cache.add(at(1, 0, 1), JUST_USED_PENALTY, 1_000);
        assert_eq!(cache.extra_cost(at(1, 0, 1), 1_000), 50.0);
        assert_eq!(
            cache.extra_cost(at(1, 0, 1), 1_000 + TARGET_TTL - 1),
            50.0,
            "one millisecond before the deadline"
        );
        assert_eq!(
            cache.extra_cost(at(1, 0, 1), 1_000 + TARGET_TTL),
            0.0,
            "at the deadline it is gone (:83)"
        );
        assert!(cache.is_empty(), "and swept");
    }

    #[test]
    fn a_neighbouring_target_is_a_different_target() {
        let mut cache = RecentTargets::new();
        cache.add(at(1, 0, 1), 50.0, 0);
        assert_eq!(cache.extra_cost(at(1, 0, 2), 0), 0.0);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn re_adding_a_target_refreshes_it_without_leaving_a_ghost() {
        let mut cache = RecentTargets::new();
        cache.add(at(0, 0, 0), 50.0, 0);
        cache.add(at(0, 0, 0), 20.0, TARGET_TTL / 2);
        assert_eq!(cache.len(), 1, "one position, one entry");
        assert_eq!(
            cache.extra_cost(at(0, 0, 0), TARGET_TTL + 1),
            20.0,
            "the first deadline has passed, the second has not"
        );
        // And the sweep that removed the first entry did not remove the second.
        assert_eq!(cache.extra_cost(at(0, 0, 0), TARGET_TTL * 2), 0.0);
        assert!(cache.is_empty());
    }

    #[test]
    fn the_sweep_stops_at_the_first_live_entry() {
        let mut cache = RecentTargets::new();
        cache.add(at(0, 0, 0), 50.0, 0); // expires at 200_000
        cache.add(at(1, 0, 0), 50.0, 100); // expires at 200_100
        cache.clean(200_050);
        assert_eq!(
            cache.len(),
            1,
            "the second is still alive, so the sweep stops"
        );
        assert_eq!(cache.extra_cost(at(1, 0, 0), 200_050), 50.0);
    }

    #[test]
    fn a_sweep_removes_at_most_its_batch() {
        let mut cache = RecentTargets::new();
        for x in 0..100 {
            cache.add(at(x, 0, 0), 50.0, 0);
        }
        cache.clean(TARGET_TTL);
        assert_eq!(cache.len(), 100 - REMOVALS_PER_CLEAN, ":27 caps one call");
        cache.clean(TARGET_TTL);
        assert_eq!(cache.len(), 0, "the second batch takes the remaining 36");
    }

    #[test]
    fn the_penalty_is_added_to_the_jobs_own_score() {
        let mut cache = RecentTargets::new();
        cache.add(at(4, 0, 4), JUST_USED_PENALTY, 0);
        assert_eq!(cache.score(12.0, at(4, 0, 4), 0), 62.0);
        assert_eq!(cache.score(12.0, at(4, 0, 5), 0), 12.0);
    }
}
