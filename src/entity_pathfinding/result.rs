//! The life of a path request: computing, followed, reached, failed, cancelled.
//!
//! Java's `PathResult` (`pathresults/PathResult.java`, 377 lines) is mostly
//! plumbing — a `Future`, a debug-watcher list, a network message — around five
//! rules that matter to a caller:
//!
//! | Rule | Java |
//! | --- | --- |
//! | a request is in progress while computing or following | `isInProgress`, `:98-101` |
//! | it has failed when it is done and does not reach | `failedToReachDestination`, `:107-110` |
//! | it is usable when it is done, has a path, and reaches | `isPathReachingDestination`, `:115-118` |
//! | cancelling settles it without a path | `cancel`, `:296-305` |
//! | the node count is kept for the debug view | `searchedNodes`, `:68` |
//!
//! **The refusal is the interesting part.** The first version's `plan` answers
//! `Result<Path, Refusal>`, and Java answers a path with a `canReach` boolean
//! (`Path.canReach`, queried by `PathResult` at `:213-215`). A port that kept
//! only the boolean would lose *why* the path stopped; this keeps both: the
//! status machine and the named refusal.

use crate::pathfinding::{Path, Refusal};

/// The five states of a path request (`PathFindingStatus.java:3-11`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// `IN_PROGRESS_COMPUTING`.
    Computing,
    /// `IN_PROGRESS_FOLLOWING`: a finished path handed to an entity.
    Following,
    /// `CALCULATION_COMPLETE`: a path, or a named refusal, but nobody has walked
    /// it yet.
    Calculated,
    /// `COMPLETE`: an entity walked it to its end.
    Complete,
    /// `CANCELLED`.
    Cancelled,
}

/// Whether the job reaches what it aimed at, and if not, why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
    /// The goal test succeeded: `pathReachesDestination = true`
    /// (`PathResult.java:76`, set at `AbstractPathJob.java:381`).
    Yes,
    /// The frontier emptied (`PathResult.java:107-110` names it
    /// `failedToReachDestination`).
    No(Refusal),
}

/// One path request, as a value a caller can inspect and a test can drive.
///
/// The path is owned, not borrowed: `plan` returns it by value, and Java keeps
/// it in a field it hands out (`getPath`, `:152-156`).
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    status: Status,
    reached: Option<Reached>,
    path: Option<Path>,
    nodes: u32,
}

impl Progress {
    /// A request that has just been submitted: computing, nothing else known
    /// (`PathResult.java:40`, `:60`).
    pub fn started() -> Progress {
        Progress {
            status: Status::Computing,
            reached: None,
            path: None,
            nodes: 0,
        }
    }

    /// The result of a finished search, `processCalculationResults` (`:250-291`)
    /// minus the debug message: `CALCULATION_COMPLETE`, the path when there is
    /// one, and the named reason when there is not.
    ///
    /// `nodes` is `searchedNodes` (`:68`): a refusal that hit the ceiling is
    /// worth logging with the count that stopped it.
    pub fn finished(result: Result<Path, Refusal>, nodes: u32) -> Progress {
        match result {
            Ok(path) => Progress {
                status: Status::Calculated,
                reached: Some(Reached::Yes),
                path: Some(path),
                nodes,
            },
            Err(refusal) => Progress {
                status: Status::Calculated,
                reached: Some(Reached::No(refusal)),
                path: None,
                nodes,
            },
        }
    }

    /// The status.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Whether the request is computing or being followed — `isInProgress`
    /// (`:98-101`), which is `Computing || Following`.
    pub fn in_progress(&self) -> bool {
        matches!(self.status, Status::Computing | Status::Following)
    }

    /// Whether the calculation is still running — `isComputing` (`:103-106`).
    pub fn computing(&self) -> bool {
        self.status == Status::Computing
    }

    /// Whether the calculation finished and was processed — `isDone`
    /// (`:283-294`). Java also *processes* on this call; here processing has
    /// already happened in [`Self::finished`], and this only reports.
    pub fn done(&self) -> bool {
        matches!(
            self.status,
            Status::Calculated | Status::Complete | Status::Cancelled
        )
    }

    /// Whether the request finished and did not reach — `failedToReachDestination`
    /// (`:107-110`): **done and not reaching**, so a cancelled request is not a
    /// failure (Java asks `isDone() && !pathReachesDestination`, and `cancel`
    /// leaves `pathReachesDestination` false — the distinction is why the
    /// status is checked first here).
    pub fn failed(&self) -> bool {
        self.status == Status::Calculated && matches!(self.reached, Some(Reached::No(_)))
    }

    /// Whether a caller may hand the path to an engine —
    /// `isPathReachingDestination` (`:115-118`): done, a path, and it reaches.
    pub fn usable(&self) -> bool {
        self.done() && self.path.is_some() && self.reached == Some(Reached::Yes)
    }

    /// Why it failed, when it did.
    pub fn refusal(&self) -> Option<Refusal> {
        match self.reached {
            Some(Reached::No(refusal)) => Some(refusal),
            _ => None,
        }
    }

    /// The path, when there is one.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_ref()
    }

    /// How many nodes the search pulled — `searchedNodes` (`:68`).
    pub fn nodes(&self) -> u32 {
        self.nodes
    }

    /// Hand the path to an entity: `CALCULATION_COMPLETE → IN_PROGRESS_FOLLOWING`
    /// (the status Java sets from the navigator, `PathFindingStatus.java:5`).
    ///
    /// Refused, and nothing changes, when there is no usable path: a caller that
    /// tries to follow a failure has a bug, and a `false` says so without
    /// pretending a state change happened.
    pub fn follow(&mut self) -> bool {
        if self.status == Status::Calculated && self.usable() {
            self.status = Status::Following;
            true
        } else {
            false
        }
    }

    /// The entity arrived: `FOLLOWING → COMPLETE` (`PathFindingStatus.java:7`).
    pub fn arrive(&mut self) -> bool {
        if self.status == Status::Following {
            self.status = Status::Complete;
            true
        } else {
            false
        }
    }

    /// `cancel` (`:296-305`): the request is settled, with no path to give.
    ///
    /// Java only drops the `Future` and sets `pathingDoneAndProcessed`; the
    /// status is set by the caller (`setStatus(CANCELLED)`). Here cancelling
    /// *is* the state change, and it drops the path with it.
    pub fn cancel(&mut self) {
        self.status = Status::Cancelled;
        self.path = None;
        self.reached = Some(Reached::No(Refusal::Unreachable));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::{Node, NodeKind, Path, at};

    fn a_path() -> Path {
        Path::new(vec![
            Node::start(at(0, 0, 0), NodeKind::Walk),
            Node::new(at(1, 0, 0), NodeKind::Walk, 1.0),
        ])
    }

    #[test]
    fn a_fresh_request_is_computing_and_not_a_failure() {
        let p = Progress::started();
        assert_eq!(p.status(), Status::Computing);
        assert!(p.in_progress() && p.computing());
        assert!(!p.done() && !p.failed() && !p.usable());
    }

    #[test]
    fn a_finished_path_is_calculated_reaching_and_usable() {
        let p = Progress::finished(Ok(a_path()), 42);
        assert_eq!(p.status(), Status::Calculated);
        assert_eq!(p.reached, Some(Reached::Yes));
        assert!(p.done() && p.usable());
        assert!(!p.failed() && !p.in_progress(), "displaying is neither");
        assert_eq!(p.nodes(), 42, "searchedNodes");
        assert_eq!(p.refusal(), None);
    }

    #[test]
    fn a_refusal_is_calculated_failed_and_named() {
        let p = Progress::finished(Err(Refusal::Unreachable), 900);
        assert!(p.done() && p.failed());
        assert!(!p.usable() && p.path().is_none());
        assert_eq!(p.refusal(), Some(Refusal::Unreachable));
        assert_eq!(p.nodes(), 900);
    }

    #[test]
    fn a_ceiling_refusal_keeps_the_numbers_that_stopped_it() {
        let p = Progress::finished(
            Err(Refusal::NodeLimit {
                visited: 8_001,
                limit: 8_000,
            }),
            8_001,
        );
        assert_eq!(
            p.refusal(),
            Some(Refusal::NodeLimit {
                visited: 8_001,
                limit: 8_000
            })
        );
    }

    #[test]
    fn following_advances_one_state_at_a_time() {
        let mut p = Progress::finished(Ok(a_path()), 7);
        assert!(p.follow(), "calculated → following");
        assert_eq!(p.status(), Status::Following);
        assert!(p.in_progress(), "a followed path is in progress (:100)");
        assert!(p.arrive(), "following → complete");
        assert_eq!(p.status(), Status::Complete);
        assert!(p.usable(), "a finished path is still a path");
        assert!(!p.arrive(), "and arriving twice does nothing");
    }

    #[test]
    fn a_failed_request_cannot_be_followed() {
        let mut p = Progress::finished(Err(Refusal::Unreachable), 10);
        assert!(!p.follow(), "no path, no following");
        assert_eq!(
            p.status(),
            Status::Calculated,
            "and the status is untouched"
        );
    }

    #[test]
    fn cancelling_settles_the_request_without_a_path() {
        let mut p = Progress::finished(Ok(a_path()), 5);
        p.cancel();
        assert_eq!(p.status(), Status::Cancelled);
        assert!(p.done() && !p.in_progress());
        assert!(p.path().is_none(), "cancel drops the path (:296-305)");
        assert!(!p.usable(), "a cancelled path is not handable");
        assert!(
            !p.failed(),
            "cancelled is not failed: Java asks isDone && !reaches, and a caller \
             must tell 'I stopped it' from 'it could not'"
        );
        assert!(!p.follow() && !p.arrive(), "and nothing may restart it");
    }

    #[test]
    fn a_start_blocked_refusal_says_where_the_body_was() {
        let p = Progress::finished(Err(Refusal::StartBlocked { at: at(3, 4, 5) }), 0);
        assert_eq!(p.nodes(), 0, "no node was ever pulled");
        assert_eq!(p.refusal(), Some(Refusal::StartBlocked { at: at(3, 4, 5) }));
    }
}
