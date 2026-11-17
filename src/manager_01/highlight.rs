//! The highlights: what a client draws in the world, for how long, and in what order
//! it starts, draws and stops.
//!
//! Java reference: `core/client/render/worldevent/HighlightManager.java:8-132`, and
//! the three methods of `IHighlightRenderData` it calls
//! (`core/client/render/worldevent/highlightmanager/IHighlightRenderData.java`).
//!
//! **Two keys, because a highlight is replaced and a family is dropped.** The map
//! is `group -> subKey -> container` (`:13`): adding with a subkey that is already
//! there **replaces** it (`:77`), which is how a moving box reuses its group, and
//! `clearHighlightsForKey` drops a whole group at once (`:63-66`), which is how a
//! window closes every box it opened.
//!
//! **A duration is measured in whole seconds, from the world's game time.** Java
//! takes a `Duration`, converts it with `getSeconds() * 20` and compares it to
//! `clientLevel.getGameTime()` (`:110-114`); the conversion drops milliseconds, so
//! 1 500 ms is one second — twenty ticks, not thirty. A `null` duration never
//! expires (`:111-115`), and that is the only way a highlight outlives its frame.
//!
//! **Three quirks, all kept, each with its cost.**
//!
//! 1. **`startTime == 0` means "not started"** (`:93`, `:125`). A highlight added
//!    while the world's game time is 0 therefore starts on the *next* frame, and
//!    its `startRender` runs on both frames (`:127-128` runs again because the
//!    field is still 0). Nothing in a normal world is at time 0 for more than one
//!    tick, but a client that joins and adds a highlight in the same tick sees one
//!    extra start. The alternative — an `Option<u64>` — would silently lengthen
//!    every such highlight by one tick's worth of nothing, so the sentinel stays.
//! 2. **Expiry is `start + seconds * 20 < worldTime`** (`:113`), a strict
//!    comparison: a zero-second highlight is drawn on the frame it starts and
//!    stopped on the next one, never on the same frame.
//! 3. **`attemptStart` runs before the expiry test** (`:37`, `:40`), so the frame
//!    that stops a highlight is never the frame that starts it — unless the
//!    duration is zero and the world time is 0, which case 1 sends one frame later.
//!
//! **The order of the three calls is the rule** and this module answers them as
//! [`Step`] values: `Start` before `Draw`, `Stop` *instead of* `Draw`, and a
//! highlight that has stopped is never drawn again. Java removes it from the map
//! in the same pass (`:43`) and drops the group once nothing is left in it
//! (`:51-54`); the port removes after the walk of the group, which is the same
//! visible result with no iterator to keep alive.
//!
//! **What the port changes is the order between highlights.** Java walks two
//! `HashMap`s (`:13`, `:29-35`), so two highlights can start in either order and
//! the draw order changes with the hash of their names. Here both levels are
//! `BTreeMap`s: groups and subkeys come out in name order, every frame, on every
//! client. That is the same divergence `crate::manager::claims` made for a chunk's
//! owner, and it is what lets a test say what a frame does.
//!
//! **Pure data, no host call.** The renderer itself (`IHighlightRenderData`) is not
//! ported: it draws a box, a line or a text, and this crate has no drawing call for
//! a mod's own world overlay (`MANQUANT.md` §2 names what would be needed).

use std::collections::BTreeMap;

/// One second of game time (`HighlightManager.java:113`: `getSeconds() * 20`).
pub const TICKS_PER_SECOND: u64 = 20;

/// What one frame does to one highlight, in Java's order
/// (`HighlightManager.java:37-48`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the three calls `render` can make, and nothing else
pub enum Step {
    /// `attemptStart` set the timer and `startRender` ran (`:123-130`). Once per
    /// highlight, except for the world-time-0 case the module note explains.
    Start { group: String, sub: String },
    /// `render` ran: the highlight is alive and drawn (`:47`).
    Draw { group: String, sub: String },
    /// `stopRender` ran and the highlight was dropped (`:42-43`). Never followed by
    /// `Draw` for the same pair.
    Stop { group: String, sub: String },
}

impl Step {
    /// The pair the step is about, `(group, sub)`.
    pub fn key(&self) -> (&str, &str) {
        match self {
            Step::Start { group, sub } | Step::Draw { group, sub } | Step::Stop { group, sub } => {
                (group, sub)
            }
        }
    }

    /// `"start"`, `"draw"` or `"stop"`, for a log line or a test.
    pub fn kind(&self) -> &'static str {
        match self {
            Step::Start { .. } => "start",
            Step::Draw { .. } => "draw",
            Step::Stop { .. } => "stop",
        }
    }
}

/// One highlight as the manager keeps it: Java's `HighlightRenderDataContainer`
/// without the renderer it wraps (`HighlightManager.java:83-131`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two fields, both of them Java's
pub struct Highlight {
    /// Seconds the highlight lives, exactly Java's `Duration` cut to whole seconds
    /// (`:110-113`); `None` is Java's `null` and never expires.
    duration: Option<u64>,
    /// The world time `startRender` ran at (`:93`); **0 means it has not run**
    /// (`:125`).
    start: u64,
}

impl Highlight {
    pub fn duration_seconds(&self) -> Option<u64> {
        self.duration
    }

    /// The world time the highlight started at, or `None` while its timer is still
    /// the sentinel: Java's `startTime == 0` (`:125`). A highlight that started at
    /// world time 0 reads as unstarted, which is the quirk the module note keeps.
    pub fn started_at(&self) -> Option<u64> {
        (self.start != 0).then_some(self.start)
    }

    /// `isExpired` (`HighlightManager.java:108-116`): no duration, never; else
    /// strictly after `start + seconds * 20`, saturating rather than wrapping on a
    /// duration no game reaches.
    pub fn is_expired(&self, world_time: u64) -> bool {
        match self.duration {
            None => false,
            Some(seconds) => {
                self.start.saturating_add(seconds.saturating_mul(TICKS_PER_SECOND)) < world_time
            }
        }
    }
}

/// The highlights a client holds: Java's static `HIGHLIGHT_ITEMS` (`:13`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Highlights {
    groups: BTreeMap<String, BTreeMap<String, Highlight>>,
}

impl Highlights {
    pub fn new() -> Highlights {
        Highlights::default()
    }

    /// How many highlights are alive, across every group.
    pub fn len(&self) -> usize {
        self.groups.values().map(|subs| subs.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many groups hold at least one highlight.
    pub fn groups(&self) -> usize {
        self.groups.len()
    }

    /// One highlight, for a test or a log line.
    pub fn get(&self, group: &str, sub: &str) -> Option<&Highlight> {
        self.groups.get(group)?.get(sub)
    }

    /// `addHighlight` (`HighlightManager.java:75-78`): creates the group, and
    /// **replaces** a highlight that already carries the same subkey — the new
    /// container starts at the sentinel again, so its timer restarts.
    pub fn add(&mut self, group: impl Into<String>, sub: impl Into<String>, duration: Option<u64>) {
        self.groups.entry(group.into()).or_default().insert(
            sub.into(),
            Highlight {
                duration,
                start: 0,
            },
        );
    }

    /// `clearHighlightsForKey` (`:63-66`): the whole group goes, and answers
    /// whether there was one.
    pub fn clear(&mut self, group: &str) -> bool {
        self.groups.remove(group).is_some()
    }

    /// One frame, `render` (`:20-56`), with the world's game time.
    ///
    /// Returns what the frame did, per highlight and in name order. A group whose
    /// last highlight expired is forgotten in the same frame (`:51-54`), so
    /// [`Highlights::groups`] shrinks without the caller doing anything.
    pub fn frame(&mut self, world_time: u64) -> Vec<Step> {
        if self.groups.is_empty() {
            return Vec::new();
        }

        let mut steps = Vec::new();
        let mut emptied = Vec::new();
        for (group, subs) in self.groups.iter_mut() {
            let mut expired = Vec::new();
            for (sub, highlight) in subs.iter_mut() {
                // `:37` before `:40`: the start is attempted first, every frame.
                if highlight.start == 0 {
                    highlight.start = world_time;
                    steps.push(Step::Start {
                        group: group.clone(),
                        sub: sub.clone(),
                    });
                }
                if highlight.is_expired(world_time) {
                    steps.push(Step::Stop {
                        group: group.clone(),
                        sub: sub.clone(),
                    });
                    expired.push(sub.clone());
                } else {
                    steps.push(Step::Draw {
                        group: group.clone(),
                        sub: sub.clone(),
                    });
                }
            }
            for sub in expired {
                subs.remove(&sub);
            }
            if subs.is_empty() {
                emptied.push(group.clone());
            }
        }
        // `:51-54`: a group with nothing left is dropped after its walk.
        for group in emptied {
            self.groups.remove(&group);
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame as `(kind, group, sub)` triples, which is the whole of what a test
    /// has to read.
    fn read(steps: &[Step]) -> Vec<(&'static str, &str, &str)> {
        steps
            .iter()
            .map(|step| {
                let (group, sub) = step.key();
                (step.kind(), group, sub)
            })
            .collect()
    }

    #[test]
    fn a_client_with_no_highlight_does_nothing_at_all() {
        let mut highlights = Highlights::new();
        assert_eq!(highlights.frame(500), Vec::new());
        assert!(highlights.is_empty());
    }

    #[test]
    fn a_highlight_starts_and_is_drawn_on_the_same_frame() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(3));
        assert_eq!(
            read(&highlights.frame(100)),
            vec![("start", "window", "box"), ("draw", "window", "box")],
            "`:123-130` runs before `:47`, so both happen in one frame"
        );
        assert_eq!(highlights.get("window", "box").and_then(Highlight::started_at), Some(100));
    }

    #[test]
    fn the_timer_starts_at_the_first_frame_not_at_the_insertion() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(2));
        // Ten seconds pass before the first frame: the highlight does not age.
        assert_eq!(highlights.frame(200).len(), 2);
        assert_eq!(highlights.get("window", "box").and_then(Highlight::started_at), Some(200));
    }

    #[test]
    fn no_duration_never_expires() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", None);
        assert_eq!(
            read(&highlights.frame(1)),
            vec![("start", "window", "box"), ("draw", "window", "box")]
        );
        for time in [2, 1_000, 10_000_000] {
            assert_eq!(read(&highlights.frame(time)), vec![("draw", "window", "box")]);
        }
        assert_eq!(highlights.len(), 1);
    }

    #[test]
    fn a_duration_counts_seconds_of_twenty_ticks_and_expires_strictly_after() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(2));
        highlights.frame(1_000);
        let last_alive = 1_000 + 2 * TICKS_PER_SECOND;
        assert_eq!(
            read(&highlights.frame(last_alive)),
            vec![("draw", "window", "box")],
            "`start + 40 < worldTime` is false at exactly start + 40"
        );
        assert_eq!(
            read(&highlights.frame(last_alive + 1)),
            vec![("stop", "window", "box")],
            "and true one tick later, so the highlight is stopped, never drawn"
        );
    }

    #[test]
    fn a_zero_second_highlight_is_drawn_once_and_stopped_on_the_next_frame() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(0));
        assert_eq!(
            read(&highlights.frame(7)),
            vec![("start", "window", "box"), ("draw", "window", "box")]
        );
        assert_eq!(read(&highlights.frame(8)), vec![("stop", "window", "box")]);
        assert!(highlights.is_empty(), "and the group went with it");
        assert_eq!(highlights.groups(), 0);
    }

    #[test]
    fn a_highlight_added_at_world_time_zero_starts_twice() {
        // `startTime == 0` is the "not started" sentinel (`:93`, `:125`), so setting
        // it to 0 leaves it at 0 and the next frame starts it again.
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(5));
        assert_eq!(read(&highlights.frame(0)), vec![("start", "window", "box"), ("draw", "window", "box")]);
        assert_eq!(
            read(&highlights.frame(1)),
            vec![("start", "window", "box"), ("draw", "window", "box")],
            "`startRender` runs a second time, which is Java's own behaviour"
        );
        assert_eq!(read(&highlights.frame(2)), vec![("draw", "window", "box")]);
        assert_eq!(highlights.get("window", "box").and_then(Highlight::started_at), Some(1));
    }

    #[test]
    fn adding_a_subkey_that_exists_replaces_it_and_restarts_its_timer() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(1));
        highlights.frame(100);
        highlights.add("window", "box", Some(1));
        assert_eq!(highlights.len(), 1, "replaced, not appended (`:77`)");
        assert_eq!(
            highlights.get("window", "box"),
            Some(&Highlight { duration: Some(1), start: 0 }),
            "a fresh container, so the timer is the sentinel again"
        );
        assert_eq!(read(&highlights.frame(5_000)).first().map(|s| s.0), Some("start"));
    }

    #[test]
    fn clearing_a_group_leaves_every_other_group_alone() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", None);
        highlights.add("window", "line", None);
        highlights.add("hut", "box", None);
        assert!(highlights.clear("window"));
        assert!(!highlights.clear("window"), "the second clear finds nothing");
        assert_eq!(highlights.len(), 1);
        assert_eq!(
            read(&highlights.frame(10)),
            vec![("start", "hut", "box"), ("draw", "hut", "box")],
            "the surviving group starts on its first frame, as any other does"
        );
    }

    #[test]
    fn a_group_disappears_with_its_last_highlight() {
        let mut highlights = Highlights::new();
        highlights.add("first", "a", Some(0));
        highlights.add("first", "b", Some(0));
        highlights.add("second", "a", None);
        assert_eq!(highlights.groups(), 2);
        highlights.frame(3);
        assert_eq!(highlights.groups(), 2, "both of `first`'s highlights started on that frame");
        let steps = highlights.frame(4);
        assert_eq!(
            read(&steps),
            vec![("stop", "first", "a"), ("stop", "first", "b"), ("draw", "second", "a")],
            "the group's two stops, then the other group"
        );
        assert_eq!(highlights.groups(), 1, "`first` is gone (`:51-54`)");
        assert!(!highlights.clear("first"), "and a second clear finds nothing");
    }

    #[test]
    fn two_groups_and_two_subkeys_come_out_in_name_order_every_frame() {
        let mut highlights = Highlights::new();
        highlights.add("zeta", "b", None);
        highlights.add("alpha", "y", None);
        highlights.add("alpha", "x", None);
        let expected = vec![
            ("start", "alpha", "x"),
            ("draw", "alpha", "x"),
            ("start", "alpha", "y"),
            ("draw", "alpha", "y"),
            ("start", "zeta", "b"),
            ("draw", "zeta", "b"),
        ];
        assert_eq!(read(&highlights.frame(1)), expected.clone());
        assert_eq!(
            read(&highlights.frame(2)),
            vec![
                ("draw", "alpha", "x"),
                ("draw", "alpha", "y"),
                ("draw", "zeta", "b"),
            ]
        );
    }

    #[test]
    fn a_stopped_highlight_never_comes_back() {
        let mut highlights = Highlights::new();
        highlights.add("window", "box", Some(1));
        highlights.frame(10);
        assert_eq!(read(&highlights.frame(31)), vec![("stop", "window", "box")]);
        for time in [32, 100, 10_000] {
            assert!(highlights.frame(time).is_empty(), "nothing left to draw at {time}");
        }
    }
}
