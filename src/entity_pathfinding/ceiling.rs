//! How many nodes a job may visit, and when it may ask for more.
//!
//! Java caps every search and then relaxes the cap twice: once as a function of
//! the most expensive step seen (`maxCost`, `AbstractPathJob.java:373-379`), and
//! once per job in `stopOnNodeLimit`, which may grant more nodes when the search
//! is making progress (`PathJobMoveToLocation.java:135-151`,
//! `PathJobMoveCloseToXNearY.java:90-101`).
//!
//! **Two constructors, two formulas, and the first version conflated them.** A
//! job built from a *range* gets `min(8000, max(10, range)²)`
//! (`AbstractPathJob.java:190-193`); a job built from a start and an *end* gets
//! an estimate of the corridor between them (`:271-283`). `crate::pathfinding`'s
//! `Limits::for_range` only has the first, without the `max(10, …)` floor, and
//! applies it to a walk-to job that Java builds with the second — see
//! [`parity`](super::parity) and `RAPPORT`.

use crate::pathfinding::{MAX_COST, MAX_NODES, Pos};

/// The smallest range any job searches: `range = Math.max(10, range)`
/// (`AbstractPathJob.java:190`, `:222`).
pub const MIN_RANGE: i32 = 10;

/// `MAX_NODES * 2`, the point past which a walk-to job stops granting
/// (`PathJobMoveToLocation.java:144`).
pub const DOUBLE_CAP: u32 = MAX_NODES * 2;

/// Where a search stands when the cap is reached: what a stop policy reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watch {
    /// Nodes pulled off the frontier, `totalNodesVisited`.
    pub visited: u32,
    /// Nodes since the best end node improved, `nodesSinceEndNode`.
    pub since_end: u32,
    /// Manhattan distance from the start to the best end node so far.
    pub best_from_start: i32,
}

/// What a stop policy decides: stop, or grant this many more nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Give up and keep the best end found (`AbstractPathJob.java:506`).
    Stop,
    /// Raise the cap and keep looking (`PathJobMoveToLocation.java:140`,
    /// `:147`).
    Extend {
        /// Nodes added to the cap.
        nodes: u32,
    },
}

/// The per-job override of `stopOnNodeLimit` (`AbstractPathJob.java:492-501`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopPolicy {
    /// Stop at the first node over the cap: Java's default
    /// (`AbstractPathJob.java:492-501`).
    Immediate,
    /// Keep going while the search is still creeping towards something
    /// (`PathJobMoveToLocation.java:135-151`).
    MoveToLocation,
    /// Keep going until the end node has stood still for 200 nodes
    /// (`PathJobMoveCloseToXNearY.java:90-101`).
    CloseTo,
}

impl StopPolicy {
    /// The decision, with `roll` standing in for `ColonyConstants.rand.nextInt(100)`
    /// — a draw of 0..100 taken by the caller, so the decider is deterministic.
    pub fn decide(&self, watch: Watch, roll: u32) -> Stop {
        match self {
            StopPolicy::Immediate => Stop::Stop,
            // `totalNodesVisited < MAX_NODES && dist < 10 && nextInt(100) <= 20`
            // grants 1000; else `nodesSinceEndNode < 200 && total < MAX_NODES*2`
            // grants 500 (`:138-148`). The first branch is a small chance to
            // search the full cap when the search has found nothing worth
            // moving towards.
            StopPolicy::MoveToLocation => {
                if watch.visited < MAX_NODES && watch.best_from_start < 10 && roll <= 20 {
                    Stop::Extend { nodes: 1_000 }
                } else if watch.since_end < 200 && watch.visited < DOUBLE_CAP {
                    Stop::Extend { nodes: 500 }
                } else {
                    Stop::Stop
                }
            }
            // `nodesSinceEndNode > 200` stops, else grants 200 every time the
            // check is reached (`:92-100`) — a job that waits for progress
            // rather than for a count.
            StopPolicy::CloseTo => {
                if watch.since_end > 200 {
                    Stop::Stop
                } else {
                    Stop::Extend { nodes: 200 }
                }
            }
        }
    }
}

/// A job's node budget: what it started with, what it has been granted, and the
/// relaxation `maxCost` earns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ceiling {
    /// The starting `maxNodes`.
    pub base: u32,
    /// Nodes granted by [`StopPolicy`] since the search began.
    pub granted: u32,
}

impl Ceiling {
    /// The ceiling of a job that searches `range` blocks around its start:
    /// `range = max(10, range)` then `min(8000, range²)`
    /// (`AbstractPathJob.java:190-193`).
    ///
    /// **The floor is not cosmetic.** `of_range(0)` is 100, not 0: Java lifts
    /// every range to ten before squaring it, so there is no job whose cap is
    /// zero.
    pub fn of_range(range: i32) -> Ceiling {
        let lifted = range.max(MIN_RANGE) as u32;
        Ceiling {
            base: lifted.saturating_mul(lifted).min(MAX_NODES),
            granted: 0,
        }
    }

    /// The ceiling of a job that walks from `start` to `end`, from the area it
    /// may cover (`AbstractPathJob.java:271-283`):
    ///
    /// ```text
    /// direct   = max(1,|dx|) + max(1,|dy|) + max(1,|dz|)
    /// radius   = min(20, 3 + direct / 20)
    /// estimate = 300 + direct * 16 + direct * radius² * 2
    /// cap      = min(8000, estimate)
    /// ```
    ///
    /// The three `max(1, …)` are why two equal positions still buy 402 nodes:
    /// a search that does not move still explores its neighbourhood.
    pub fn of_corridor(start: Pos, end: Pos) -> Ceiling {
        let dx = (start[0] - end[0]).abs().max(1) as u32;
        let dy = (start[1] - end[1]).abs().max(1) as u32;
        let dz = (start[2] - end[2]).abs().max(1) as u32;
        let direct = dx + dy + dz;
        let radius = (3 + direct / 20).min(20);
        let estimate = 300 + direct * 16 + direct * radius * radius * 2;
        Ceiling {
            base: estimate.min(MAX_NODES),
            granted: 0,
        }
    }

    /// `maxNodes += range` for a walk-to (`PathJobMoveToLocation.java:43`).
    pub fn plus(mut self, extra: i32) -> Ceiling {
        self.base = self.base.saturating_add(extra.max(0) as u32);
        self
    }

    /// A flat cap that ignores the estimate: `maxNodes = 20000` for the raiders
    /// and for signs (`PathJobRaiderPathing.java:56`,
    /// `PathJobSignConnection.java:18`), `1000`/`2000` for a wander
    /// (`PathJobRandomPos.java:165`). Above [`MAX_NODES`] on purpose — Java
    /// overrides the constant per job.
    pub fn override_to(mut self, base: u32) -> Ceiling {
        self.base = base;
        self
    }

    /// Add what a policy granted; the search calls this when it keeps going.
    pub fn grant(&mut self, nodes: u32) {
        self.granted = self.granted.saturating_add(nodes);
    }

    /// The cap in force once the widest step is `widest`:
    /// `maxNodes + maxCost² × 2`, with `maxCost = min(25, ceil(step))`
    /// (`AbstractPathJob.java:373`, `:889-891`).
    ///
    /// `widest` is Java's `maxCost`, already rounded up and capped when the
    /// step was priced: passing the raw step cost here would add a fraction of
    /// a node, which `u32` would then truncate.
    pub fn limit(&self, widest: u32) -> u32 {
        let widest = widest.min(MAX_COST as u32);
        self.base
            .saturating_add(self.granted)
            .saturating_add(widest.saturating_mul(widest).saturating_mul(2))
    }

    /// Whether the search has spent its cap.
    pub fn spent(&self, visited: u32, widest: u32) -> bool {
        visited > self.limit(widest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_is_lifted_to_ten_before_it_is_squared() {
        assert_eq!(Ceiling::of_range(20).base, 400, "20² (:193)");
        assert_eq!(Ceiling::of_range(0).base, 100, "max(10,0)² = 100, not 0");
        assert_eq!(Ceiling::of_range(-5).base, 100, "and a negative range too");
        assert_eq!(Ceiling::of_range(200).base, MAX_NODES, "capped at 8000");
        assert_eq!(
            Ceiling::of_range(90).base,
            MAX_NODES,
            "8 100 is over the cap too"
        );
        assert_eq!(Ceiling::of_range(85).base, 7_225, "just under it");
    }

    #[test]
    fn a_corridor_is_estimated_from_its_length() {
        // direct = 10 + 1 + 1 = 12, radius = 3, volume = 108
        // estimate = 300 + 192 + 216 = 708
        assert_eq!(
            Ceiling::of_corridor([0, 0, 0], [10, 0, 0]).base,
            708,
            ":271-283"
        );
        // A search that does not move still gets its neighbourhood:
        // direct = 3, radius = 3, estimate = 300 + 48 + 54 = 402
        assert_eq!(Ceiling::of_corridor([5, 5, 5], [5, 5, 5]).base, 402);
        // 400 blocks away: direct = 402, radius = min(20, 3+20) = 20,
        // estimate = 300 + 6432 + 160800 → capped.
        assert_eq!(Ceiling::of_corridor([0, 0, 0], [400, 0, 0]).base, MAX_NODES);
    }

    #[test]
    fn a_walk_to_adds_its_range_after_the_estimate() {
        let c = Ceiling::of_corridor([0, 0, 0], [10, 0, 0]).plus(30);
        assert_eq!(c.base, 738, "708 + 30 (PathJobMoveToLocation.java:43)");
        assert_eq!(
            Ceiling::of_range(10).plus(-4).base,
            100,
            "a negative add is no add"
        );
    }

    #[test]
    fn a_job_may_override_the_cap_above_the_constant() {
        assert_eq!(
            Ceiling::of_range(10).override_to(20_000).base,
            20_000,
            "PathJobRaiderPathing.java:56 is above MAX_NODES on purpose"
        );
    }

    #[test]
    fn a_wide_step_relaxes_the_cap_by_its_square() {
        let plain = Ceiling::of_range(20);
        assert_eq!(plain.limit(0), 400, "no costly step, no relaxation");
        assert_eq!(plain.limit(3), 418, "400 + 3² × 2 (:373)");
        assert_eq!(
            plain.limit(1_000),
            1_650,
            "400 + 25² × 2, capped at MAX_COST"
        );
    }

    #[test]
    fn a_grant_raises_the_cap_and_survives_the_relaxation() {
        let mut c = Ceiling::of_range(20);
        c.grant(500);
        assert_eq!(c.limit(0), 900, "400 + 500");
        c.grant(1_000);
        assert_eq!(c.limit(2), 1_908, "1400 + 500 + 8");
    }

    #[test]
    fn a_default_policy_stops_at_the_first_node_over_the_cap() {
        let watch = Watch {
            visited: 10_000,
            since_end: 0,
            best_from_start: 1,
        };
        assert_eq!(StopPolicy::Immediate.decide(watch, 0), Stop::Stop);
    }

    #[test]
    fn a_walk_to_keeps_going_in_two_cases_and_rolls_a_die_for_the_first() {
        let close = Watch {
            visited: 1_000,
            since_end: 5,
            best_from_start: 4,
        };
        assert_eq!(
            StopPolicy::MoveToLocation.decide(close, 20),
            Stop::Extend { nodes: 1_000 },
            "roll 20 <= 20, under 10 blocks from the start (:138)"
        );
        assert_eq!(
            StopPolicy::MoveToLocation.decide(close, 21),
            Stop::Extend { nodes: 500 },
            "the die failed, but the end node is still fresh (:144)"
        );
        assert_eq!(
            StopPolicy::MoveToLocation.decide(
                Watch {
                    visited: 1_000,
                    since_end: 200,
                    best_from_start: 4
                },
                21
            ),
            Stop::Stop,
            "200 nodes without progress stops it"
        );
        assert_eq!(
            StopPolicy::MoveToLocation.decide(
                Watch {
                    visited: 16_000,
                    since_end: 0,
                    best_from_start: 4
                },
                0
            ),
            Stop::Stop,
            "past MAX_NODES * 2 nothing is granted (:144)"
        );
        assert_eq!(
            StopPolicy::MoveToLocation.decide(
                Watch {
                    visited: 7_000,
                    since_end: 0,
                    best_from_start: 40
                },
                0
            ),
            Stop::Extend { nodes: 500 },
            "the die only feeds the first branch, which needs 10 blocks (:138)"
        );
    }

    #[test]
    fn approaching_stops_on_stagnation_not_on_a_count() {
        assert_eq!(
            StopPolicy::CloseTo.decide(
                Watch {
                    visited: 9_000,
                    since_end: 150,
                    best_from_start: 3
                },
                0
            ),
            Stop::Extend { nodes: 200 },
            "200 granted each time the check is reached (:98)"
        );
        assert_eq!(
            StopPolicy::CloseTo.decide(
                Watch {
                    visited: 9_000,
                    since_end: 201,
                    best_from_start: 3
                },
                0
            ),
            Stop::Stop,
            ":92"
        );
    }

    #[test]
    fn spending_the_cap_is_a_comparison_not_an_equality() {
        let c = Ceiling::of_range(10); // 100
        assert!(!c.spent(100, 0), "the hundredth node is still inside");
        assert!(c.spent(101, 0), ":373 is a strict >");
    }
}
