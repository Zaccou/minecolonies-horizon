//! `WindowAssignCitizen`: the two lists of a home, the line under each name, and the four buttons.
//!
//! Java reference: `core/client/gui/WindowAssignCitizen.java` (401 lines) and
//! `gui/windowassigncitizen.xml`. The window is **already a `src/ui` screen**
//! ([`crate::ui::screen::ASSIGN_ID`]): that table carries the two lists, the two row panes
//! `citizen`/`job`, the six panes of [`super::constants`] and the four buttons. This module adds the
//! three things a table of fields cannot hold:
//!
//! 1. **what the `job` pane of a row is made of.** Not a value: a translation key, then `": "`, then
//!    a distance, then `" "`, then another distance — or, for a citizen with no job, the "unemployed"
//!    key, a newline and the home line (`:290-299`). [`job_line`] and [`assigned_job_line`] answer it,
//!    and they are **not the same function**: the assigned list measures the 300-block threshold on
//!    another pair of positions (`:366-370` against `:270-276`).
//! 2. **which citizens the left-hand list holds, and in what order.** A filter (`:196-197`) and a
//!    two-key sort (`:198-210`) — [`unassigned`].
//! 3. **what the two clicks change.** [`hire`] and [`fire`] are the state transitions, and they are
//!    not symmetrical: `hireClicked` re-checks the head count (`:105-108`) while `fireClicked`
//!    re-checks nothing (`:123-136`).
//!
//! **`hire`'s refusal is checked twice and `fire`'s is checked nowhere in the handler.** The
//! `hire.warning` and the full-house disable live in the **row provider** (`:301-317`), so a click
//! can only ever reach `hireClicked` with the button enabled; Java still repeats the count check at
//! `:105`, and does not repeat the travelling check at `:123`. [`super::Refusal::NoTownHall`] adds
//! the third silent refusal, the `cancel` button's.
//!
//! **A tooltip Java sets and erases.** In the assigned row, the travelling tooltip is set at `:384`
//! and then unconditionally overwritten by `setText(Component.empty())` at `:391`, so **the player
//! never sees it** — the button is still disabled, which is what actually refuses the click. In the
//! unassigned row the empty tooltip is set inside the *permitted* branch (`:311`), so the
//! `hire.warning` at `:316` survives. [`hire_tooltip`] and [`fire_tooltip`] keep both behaviours
//! rather than smoothing them out.

use crate::ui::action::{Effect, HiringMode, Refusal as SharedRefusal, may_assign, may_unassign, next_hiring_mode};
use crate::ui::screen::Screen;

use super::{Emphasis, Refusal};

/// The distance past which a home is "too far", and the only number of its kind in this window:
/// `FAR_DISTANCE_THRESHOLD = 300` (`WindowAssignCitizen.java:39`), a `double` compared against an
/// `int` in both places that use it (`:273`, `:367`).
pub const FAR_DISTANCE_THRESHOLD: f64 = 300.0;

/// "unemployed", the whole text of the `job` pane for a citizen with no job
/// (`TranslationConstants.java:230`, `WindowAssignCitizen.java:28`, `:297`).
pub const UNEMPLOYED_KEY: &str = "com.minecolonies.coremod.gui.townhall.citizens.unemployed";

/// `gui.home.new <distance>` — the distance from this home to the citizen's workplace
/// (`WindowAssignCitizen.java:261`, `:357`).
pub const NEW_KEY: &str = "com.minecolonies.coremod.gui.home.new";

/// `gui.home.homeless`, when the citizen has no home at all (`:264`).
pub const HOMELESS_KEY: &str = "com.minecolonies.coremod.gui.home.homeless";

/// `gui.home.currently <distance>` — the distance from the citizen's *current* home to its work
/// (`:271`).
pub const CURRENTLY_KEY: &str = "com.minecolonies.coremod.gui.home.currently";

/// The tooltip of a disabled `hire`: the town hall hands housing out (`:316`).
pub const HIRE_WARNING_KEY: &str = "com.minecolonies.coremod.gui.home.hire.warning";

/// The tooltip of a disabled `fire` — set at `:384`, erased at `:391`, so unreachable.
pub const TRAVELLING_KEY: &str = "com.minecolonies.coremod.gui.home.travelling";

/// The `fire` button's label: the **class** writes it over the XML's `buttonfire` (`:347`), so a row
/// of the assigned list reads "unassign", not "fire".
pub const UNASSIGN_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonunassign";

/// The `hire` button's label, which only the XML sets (`windowassigncitizen.xml:24`).
pub const ASSIGN_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonassign";

/// The prefix of the `mode` button's label, completed by the mode's lower-cased name
/// (`WindowAssignCitizen.java:187`): [`crate::ui::action::HiringMode::slug`] is that name.
pub const MODE_PREFIX: &str = "com.minecolonies.coremod.gui.hiringmode.";

/// The work half of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two branches, `work != null` and not
pub enum WorkLine {
    /// `work == null`: Java leaves `workString` as `Component.empty()` (`:256`), so nothing is
    /// appended after the job key and its `": "`.
    Empty,
    /// `work != null`: [`NEW_KEY`] with the distance from this home to the workplace, and the style
    /// [`Emphasis::DarkGreen`] when the citizen would gain by moving here (`:284-287`).
    New { distance: i32, emphasis: Emphasis },
}

impl WorkLine {
    /// The key this line renders, or `None` for [`WorkLine::Empty`] — which is not a key.
    pub fn key(self) -> Option<&'static str> {
        match self {
            WorkLine::Empty => None,
            WorkLine::New { .. } => Some(NEW_KEY),
        }
    }
}

/// The home half of a row, as the **unassigned** list draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of `WindowAssignCitizen.java:264-282`
pub enum HomeLine {
    /// `home == null`: "homeless", and it is the initial value (`:264`), so a citizen with neither a
    /// home nor work still reads it.
    Homeless,
    /// `home != null` and `work == null`: `Component.empty()` (`:280`) — the row then ends with the
    /// `" "` separator and nothing after it.
    Empty,
    /// `home != null` and `work != null`: [`CURRENTLY_KEY`] with the distance from the current home
    /// to the workplace (`:271`), red past the threshold (`:273-276`).
    Currently { distance: i32, emphasis: Emphasis },
}

impl HomeLine {
    /// The key this line renders, or `None` for [`HomeLine::Empty`].
    pub fn key(self) -> Option<&'static str> {
        match self {
            HomeLine::Empty => None,
            HomeLine::Homeless => Some(HOMELESS_KEY),
            HomeLine::Currently { .. } => Some(CURRENTLY_KEY),
        }
    }
}

/// The whole content of an unassigned row's `job` pane (`WindowAssignCitizen.java:290-299`).
///
/// This pane is the only one of the window whose content is **composed**: a translation key, a literal
/// separator, a translatable with a distance, another literal and a second translatable with a distance
/// (`:293`). Each piece keeps its own key rather than a rendered string, because a translatable's
/// arguments are the renderer's business and this port has no renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `if (citizen.getJobView() != null)`
pub enum JobLine {
    /// A job: `translatable(jobKey) + ": " + workString + " " + homeString` (`:293`).
    Employed { job_key: String, work: WorkLine, home: HomeLine },
    /// No job: `unemployed + "\n" + homeString` (`:297`) — a **newline**, not a space, and no work
    /// half at all.
    Unemployed { home: HomeLine },
}

impl JobLine {
    /// The separators Java puts between the pieces, which is the one difference between the two
    /// variants: `": "` then `" "` for a job, `"\n"` for no job.
    pub fn separators(&self) -> &'static [&'static str] {
        match self {
            JobLine::Employed { .. } => &[": ", " "],
            JobLine::Unemployed { .. } => &["\n"],
        }
    }
}

/// The whole content of an **assigned** row's `job` pane (`:360-377`).
///
/// Two things are missing compared with [`JobLine`]: there is no home half, and the 300-block rule is
/// measured on `work ↔ this home` instead of `work ↔ the citizen's current home`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `if (citizen.getJobView() != null)` at `:362`
pub enum AssignedLine {
    /// A job: the job key, `": "`, then the work line — which is empty when the citizen has no
    /// workplace (`:352-358`).
    Employed { job_key: String, work: WorkLine },
    /// No job: [`UNEMPLOYED_KEY`] **alone** (`:376`) — no home line, unlike [`JobLine`].
    Unemployed,
}

/// The distance between two block positions, in blocks: the euclidean norm
/// (`BlockPosUtil.java:453-462`).
///
/// Java computes it in `long` and returns a `double`; so does this.
pub fn distance(from: [i32; 3], to: [i32; 3]) -> f64 {
    let dx = i64::from(from[0]) - i64::from(to[0]);
    let dy = i64::from(from[1]) - i64::from(to[1]);
    let dz = i64::from(from[2]) - i64::from(to[2]);
    ((dx * dx + dy * dy + dz * dz) as f64).sqrt()
}

/// The distance as the rows print it: Java casts the `double` to `int` at every call site
/// (`:260`, `:270`, `:356`, `:366`), which **truncates** — 300.9 blocks reads as 300 and is not
/// past the threshold.
pub fn floor_distance(from: [i32; 3], to: [i32; 3]) -> i32 {
    distance(from, to) as i32
}

/// Whether a distance is past [`FAR_DISTANCE_THRESHOLD`], on the `int` Java actually compares.
fn too_far(distance: i32) -> bool {
    f64::from(distance) > FAR_DISTANCE_THRESHOLD
}

/// The `job` pane of one row of the **unassigned** list
/// (`WindowAssignCitizen.java:256-299`).
///
/// `job_key` is the citizen's job translation key (`getJobView().getEntry().getTranslationKey()`,
/// `:293`); `job_key == None` is Java's `getJobView() == null`.
pub fn job_line(
    job_key: Option<&str>,
    work: Option<[i32; 3]>,
    home: Option<[i32; 3]>,
    here: [i32; 3],
) -> JobLine {
    let new_distance = work.map(|work| floor_distance(work, here));
    let mut work_line = match new_distance {
        None => WorkLine::Empty,
        Some(distance) => WorkLine::New { distance, emphasis: Emphasis::Plain },
    };

    let mut home_line = HomeLine::Homeless;
    let mut better = false;
    if let Some(home) = home {
        match (work, new_distance) {
            (Some(work), Some(new_distance)) => {
                let old = floor_distance(work, home);
                let emphasis =
                    if too_far(old) { Emphasis::Red } else { Emphasis::Plain };
                home_line = HomeLine::Currently { distance: old, emphasis };
                // `better = newDistance < oldDistance` — both ints, so 300 vs 300 is not better.
                better = new_distance < old;
            }
            // `else { homeString = Component.empty(); }` — `:278-281`.
            _ => home_line = HomeLine::Empty,
        }
    }
    if better {
        if let WorkLine::New { distance, .. } = work_line {
            work_line = WorkLine::New { distance, emphasis: Emphasis::DarkGreen };
        }
    }

    match job_key {
        Some(job_key) => JobLine::Employed { job_key: job_key.to_owned(), work: work_line, home: home_line },
        None => JobLine::Unemployed { home: home_line },
    }
}

/// The `job` pane of one row of the **assigned** list (`WindowAssignCitizen.java:352-377`).
///
/// **The threshold is measured on another pair here.** `:366` compares `work ↔ here` — the distance
/// the citizen would *gain* by living here — while the unassigned list compares `work ↔ home`, the
/// distance it has *now* (`:270`). A citizen can therefore be red in one list and plain in the other.
pub fn assigned_job_line(job_key: Option<&str>, work: Option<[i32; 3]>, here: [i32; 3]) -> AssignedLine {
    let Some(job_key) = job_key else {
        return AssignedLine::Unemployed;
    };
    let work_line = match work {
        None => WorkLine::Empty,
        Some(work) => {
            let distance = floor_distance(work, here);
            let emphasis = if too_far(distance) { Emphasis::Red } else { Emphasis::Plain };
            WorkLine::New { distance, emphasis }
        }
    };
    AssignedLine::Employed { job_key: job_key.to_owned(), work: work_line }
}

/// One citizen as the window reads it: an id, where it lives, where it works.
///
/// Java's `ICitizenDataView` has 60 more getters; these three are the ones this window touches
/// (`getHomeBuilding`, `getWorkBuilding`, `getId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// `citizen.getId()`.
    pub citizen: i32,
    /// `citizen.getHomeBuilding()`, `None` for the null `BlockPos`.
    pub home: Option<[i32; 3]>,
    /// `citizen.getWorkBuilding()`.
    pub work: Option<[i32; 3]>,
}

/// Whether a citizen appears in the unassigned list (`WindowAssignCitizen.java:196-197`).
///
/// Java's two conditions, verbatim:
/// `(!Objects.equals(home, work) || home == null) && !here.equals(home)`.
/// A citizen who lives where it works is hidden; so is one already living here; a **homeless**
/// citizen is listed even when it has no work either, because `Objects.equals(null, null)` is true
/// and the `|| home == null` rescues it.
pub fn listable(home: Option<[i32; 3]>, work: Option<[i32; 3]>, here: [i32; 3]) -> bool {
    let lives_at_work = home.is_some() && home == work;
    !lives_at_work && home != Some(here)
}

/// The sort key of one unassigned row (`WindowAssignCitizen.java:198-210`).
///
/// Two keys, in this order: **homeless first** (`home == null ? 0 : 1`), then a `long` that is the
/// distance to this home, or `Integer.MAX_VALUE` for a citizen who has a home but no job (it cannot
/// be compared), or `0` for one who has neither.
pub fn sort_key(home: Option<[i32; 3]>, work: Option<[i32; 3]>, here: [i32; 3]) -> (u8, i64) {
    let homeless_first = u8::from(home.is_some());
    let second = match (work, home) {
        (None, None) => 0,
        (None, Some(_)) => i64::from(i32::MAX),
        (Some(work), _) => i64::from(floor_distance(work, here)),
    };
    (homeless_first, second)
}

/// The unassigned list, filtered and sorted, as **indices into `rows`**
/// (`WindowAssignCitizen.java:196-210`).
///
/// Java's `stream().filter(…).sorted(…).toList()`: a stable sort, so two citizens at the same
/// distance keep the colony's order.
pub fn unassigned(rows: &[Row], here: [i32; 3]) -> Vec<usize> {
    let mut kept: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| listable(row.home, row.work, here))
        .map(|(index, _)| index)
        .collect();
    kept.sort_by_key(|index| sort_key(rows[*index].home, rows[*index].work, here));
    kept
}

/// What one assignment changes, once nothing refused it.
///
/// Java runs three steps after the click: `updateCitizens()`, then
/// `unassignedCitizenList.refreshElementPanes()`, then `assignedCitizenList.refreshElementPanes()`
/// (`:114-116`, `:133-135`). Both clicks run the same three, so the list is a constant rather than a
/// field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignment {
    /// The citizen the row pointed at (`unassignedCitizens.get(row).getId()`, `:103`).
    pub citizen: i32,
    /// The message the click sends, by class name. Nothing is sent by this port.
    pub effect: Effect,
}

/// The three refreshes both clicks run, in Java's order (`:114-116`).
pub const REFRESH: [&str; 3] =
    ["updateCitizens", "unassigned.refreshElementPanes", "assigned.refreshElementPanes"];

/// The `hire` click (`WindowAssignCitizen.java:100-117`).
///
/// Refused by [`SharedRefusal::NoRoom`] when `residents >= getMax()` (`:105-108`) — Java **returns**
/// there, silently, and this is the second check of a condition the row already enforced. The local
/// mutation Java does first (`addResident`, `setHomeBuilding`, `:109-110`) is the caller's: this
/// function answers what the click sends and nothing more.
pub fn hire(residents: usize, places: i32, citizen: i32) -> Result<Assignment, SharedRefusal> {
    if residents as i64 >= i64::from(places) {
        return Err(SharedRefusal::NoRoom);
    }
    Ok(Assignment { citizen, effect: Effect::Sends("AssignUnassignMessage") })
}

/// The `unassign` click (`WindowAssignCitizen.java:123-136`).
///
/// **No refusal, and that is Java**: `fireClicked` re-checks neither the head count nor whether the
/// citizen is travelling — both conditions are only applied by disabling the row's button
/// (`:379-397`). The port keeps the asymmetry instead of guarding a click Java lets through.
pub fn fire(citizen: i32) -> Assignment {
    Assignment { citizen, effect: Effect::Sends("AssignUnassignMessage") }
}

/// Whether the `hire` button of a row is enabled, and with which tooltip
/// (`WindowAssignCitizen.java:301-317`).
///
/// The permitted branch sets an **empty** tooltip (`:311`) and the other one the
/// [`HIRE_WARNING_KEY`] (`:316`), so unlike the `fire` side the warning survives.
pub fn hire_tooltip(
    auto_housing: bool,
    mode: HiringMode,
    residents: usize,
    places: i32,
) -> (Result<(), SharedRefusal>, Option<&'static str>) {
    match may_assign(auto_housing, mode, residents as i32, places) {
        Ok(()) => (Ok(()), None),
        Err(refusal @ SharedRefusal::NoRoom) => (Err(refusal), None),
        Err(refusal) => (Err(refusal), Some(HIRE_WARNING_KEY)),
    }
}

/// Whether the `fire` button of a row is enabled, and with which tooltip
/// (`WindowAssignCitizen.java:379-397`).
///
/// The travelling tooltip is set at `:384` and overwritten at `:391`, so it comes back as `None`
/// here — the button is still disabled, which is what refuses the click.
pub fn fire_tooltip(
    auto_housing: bool,
    mode: HiringMode,
    travelling: bool,
) -> (Result<(), SharedRefusal>, Option<&'static str>) {
    match may_unassign(auto_housing, mode, travelling) {
        Ok(()) => (Ok(()), None),
        Err(SharedRefusal::HousingHandledElsewhere) => {
            (Err(SharedRefusal::HousingHandledElsewhere), Some(HIRE_WARNING_KEY))
        }
        Err(refusal) => (Err(refusal), None),
    }
}

/// What the `mode` click changes (`WindowAssignCitizen.java:144-165`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeClick {
    /// The mode the button shows afterwards: `HiringMode.values()[ordinal + 1]`, wrapping at the end
    /// (`:156-163`) — **four** states for a home, because `LOCKED` is reachable here and is not from
    /// a workplace.
    pub next: HiringMode,
    /// The label the button then carries: [`MODE_PREFIX`] and the lower-cased name (`:187`).
    pub label: String,
    /// The message the **view** sends inside `setHiringMode` (`LivingBuildingView.java:87-91`), not
    /// the window: the window's own handler only sets the mode locally.
    pub effect: Effect,
}

/// The `mode` click, from the mode the button carries.
pub fn mode_click(mode: HiringMode) -> ModeClick {
    let next = next_hiring_mode(mode, true);
    ModeClick {
        next,
        label: format!("{MODE_PREFIX}{}", next.slug()),
        effect: Effect::Sends("BuildingHiringModeMessage"),
    }
}

/// What the `cancel` click does (`WindowAssignCitizen.java:172-178`).
///
/// Java needs **both** the cancel id and a town hall on the client building manager; without a town
/// hall the handler returns and the button does nothing, in silence — [`Refusal::NoTownHall`].
pub fn cancel(town_hall: bool) -> Result<Effect, Refusal> {
    if !town_hall {
        return Err(Refusal::NoTownHall);
    }
    Ok(Effect::Reopens)
}

/// The `src/ui` record for this window, so a caller has one entry point.
///
/// `None` only if `src/ui`'s table lost its assign row, which
/// [`tests::the_row_panes_of_this_module_are_the_ones_src_ui_declares`] would catch.
pub fn screen() -> Option<&'static Screen> {
    crate::ui::screen_by_id(crate::ui::screen::ASSIGN_ID).ok()
}

/// The two panes of a row this module fills, per list (`WindowAssignCitizen.java:253`, `:290`,
/// `:349`, `:360`).
///
/// Returned as `(list id, pane id)` rather than as [`crate::ui::field::Field`] values because
/// `src/ui` already declares those: re-declaring them would be a second description of one pane.
pub const ROW_PANES: [(&str, &str); 4] = [
    ("unassigned", "citizen"),
    ("unassigned", "job"),
    ("assigned", "citizen"),
    ("assigned", "job"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::{Scope, Source, Value};

    const HERE: [i32; 3] = [0, 64, 0];

    fn at(x: i32) -> [i32; 3] {
        [x, 64, 0]
    }

    #[test]
    fn the_distance_is_the_euclidean_norm_truncated_like_the_cast() {
        assert_eq!(distance([0, 0, 0], [3, 4, 0]), 5.0);
        assert_eq!(distance(at(0), at(0)), 0.0);
        // 300 blocks along one axis, and one more block of slack is 301.
        assert_eq!(floor_distance(at(0), at(300)), 300);
        assert_eq!(floor_distance(at(0), at(301)), 301);
        // The cast truncates: 300.9 blocks is 300, so it is **not** past the threshold.
        assert_eq!(floor_distance([0, 0, 0], [300, 1, 0]), 300);
        assert!(distance([0, 0, 0], [300, 1, 0]) > 300.0);
    }

    #[test]
    fn the_three_hundred_block_rule_is_measured_on_another_pair_in_each_list() {
        // A citizen whose home is 299 blocks from work, and whose workplace is 400 from us.
        let home = at(299);
        let work = at(400);
        let unassigned_row = job_line(Some("com.minecolonies.job.miner"), Some(work), Some(home), HERE);
        // Unassigned: `work ↔ home` = 101 blocks, not far.
        match unassigned_row {
            JobLine::Employed { home: HomeLine::Currently { distance, emphasis }, .. } => {
                assert_eq!(distance, 101);
                assert_eq!(emphasis, Emphasis::Plain);
            }
            other => panic!("unexpected line {other:?}"),
        }
        // Assigned: `work ↔ here` = 400 blocks, far — red, on the same citizen.
        match assigned_job_line(Some("com.minecolonies.job.miner"), Some(work), HERE) {
            AssignedLine::Employed { work: WorkLine::New { distance, emphasis }, .. } => {
                assert_eq!(distance, 400);
                assert_eq!(emphasis, Emphasis::Red);
            }
            other => panic!("unexpected line {other:?}"),
        }
        // 300 exactly is not past 300, in either list.
        match job_line(None, Some(at(300)), Some(at(600)), HERE) {
            JobLine::Unemployed { home: HomeLine::Currently { distance, emphasis } } => {
                assert_eq!(distance, 300);
                assert_eq!(emphasis, Emphasis::Plain, "`oldDistance > 300` is strict");
            }
            other => panic!("unexpected line {other:?}"),
        }
        assert_eq!(
            job_line(None, Some(at(300)), Some(at(600)), HERE).separators(),
            ["\n"].as_slice(),
            "an unemployed citizen's line has one separator, not two"
        );
    }

    #[test]
    fn a_closer_home_turns_the_work_line_green_and_equal_does_not() {
        // Home at 100 from work, and work 10 from here: better.
        match job_line(Some("k"), Some(at(10)), Some(at(110)), HERE) {
            JobLine::Employed { work: WorkLine::New { distance, emphasis }, .. } => {
                assert_eq!(distance, 10);
                assert_eq!(emphasis, Emphasis::DarkGreen, "WindowAssignCitizen.java:284-287");
            }
            other => panic!("unexpected line {other:?}"),
        }
        // Same distance on both sides: not better, plain.
        match job_line(Some("k"), Some(at(50)), Some(at(50)), HERE) {
            JobLine::Employed { work: WorkLine::New { emphasis, .. }, .. } => {
                assert_eq!(emphasis, Emphasis::Plain, "`newDistance < oldDistance` is strict");
            }
            other => panic!("unexpected line {other:?}"),
        }
    }

    #[test]
    fn a_citizen_with_a_home_and_no_work_has_an_empty_home_line_and_no_work_line() {
        match job_line(Some("k"), None, Some(at(10)), HERE) {
            JobLine::Employed { work, home, .. } => {
                assert_eq!(work, WorkLine::Empty, "workString stays Component.empty(): :256");
                assert_eq!(home, HomeLine::Empty, "homeString = Component.empty(): :280");
                assert_eq!(work.key(), None);
                assert_eq!(home.key(), None);
            }
            other => panic!("unexpected line {other:?}"),
        }
        // A homeless citizen with no work reads "homeless", not "empty".
        match job_line(None, None, None, HERE) {
            JobLine::Unemployed { home } => {
                assert_eq!(home, HomeLine::Homeless);
                assert_eq!(home.key(), Some(HOMELESS_KEY));
            }
            other => panic!("unexpected line {other:?}"),
        }
        // An unemployed citizen of the assigned list has **no** home half at all.
        assert_eq!(assigned_job_line(None, None, HERE), AssignedLine::Unemployed);
        assert_eq!(UNEMPLOYED_KEY, "com.minecolonies.coremod.gui.townhall.citizens.unemployed");
    }

    #[test]
    fn a_citizen_who_lives_at_work_or_already_here_is_not_listed() {
        // Lives at its workplace: hidden (`!Objects.equals(home, work)`).
        assert!(!listable(Some(at(5)), Some(at(5)), HERE));
        // Already a resident here: hidden.
        assert!(!listable(Some(HERE), None, HERE));
        assert!(!listable(Some(HERE), Some(at(9)), HERE));
        // Homeless and jobless: listed, because `|| home == null` rescues the null/null case.
        assert!(listable(None, None, HERE));
        // A home elsewhere: listed, whatever the job.
        assert!(listable(Some(at(5)), None, HERE));
        // **This expectation was wrong in my first version**, and the source settles it: living at
        // one's workplace hides a citizen *wherever that is*, because the first conjunct of
        // `(!Objects.equals(home, work) || home == null)` is false and `home == null` is false too
        // (`WindowAssignCitizen.java:196-197`). `here` plays no part in that half of the test.
        assert!(!listable(Some(at(5)), Some(at(5)), at(6)));
        // A home elsewhere *and* a workplace elsewhere, both away from here: listed.
        assert!(listable(Some(at(5)), Some(at(6)), at(7)));
    }

    #[test]
    fn the_homeless_lead_then_the_shortest_work_distance_then_the_jobless_with_a_home() {
        let rows = [
            Row { citizen: 1, home: Some(at(500)), work: Some(at(600)) },
            Row { citizen: 2, home: None, work: Some(at(50)) },
            Row { citizen: 3, home: Some(at(20)), work: None },
            Row { citizen: 4, home: None, work: None },
            Row { citizen: 5, home: Some(at(10)), work: Some(at(11)) },
        ];
        let order: Vec<i32> = unassigned(&rows, HERE).iter().map(|index| rows[*index].citizen).collect();
        // Homeless first, in file order (stable sort): 2 (work at 50) then 4 (no work, key 0).
        // A homeless row with no work at all has key 0, so it comes before key 50.
        assert_eq!(order, [4, 2, 5, 1, 3]);
        assert_eq!(sort_key(Some(at(20)), None, HERE), (1, i64::from(i32::MAX)));
        assert_eq!(sort_key(None, None, HERE), (0, 0));
    }

    #[test]
    fn hire_refuses_a_full_home_and_fire_re_checks_nothing() {
        assert_eq!(hire(5, 5, 7), Err(SharedRefusal::NoRoom));
        assert_eq!(hire(6, 5, 7), Err(SharedRefusal::NoRoom), "over-full is still full");
        let hired = hire(4, 5, 7);
        assert!(hired.is_ok());
        if let Ok(hired) = hired {
            assert_eq!(hired.citizen, 7);
            assert_eq!(hired.effect, Effect::Sends("AssignUnassignMessage"));
        }
        // A level-0 home has no places, so the same call refuses before reading the citizen.
        assert_eq!(hire(0, 0, 7), Err(SharedRefusal::NoRoom));
        // `fire` takes no count, no travelling flag and returns no refusal.
        assert_eq!(fire(7).effect, Effect::Sends("AssignUnassignMessage"));
        assert_eq!(REFRESH.len(), 3);
        assert_eq!(REFRESH[0], "updateCitizens");
    }

    #[test]
    fn the_hire_tooltip_warns_and_the_fire_tooltips_travelling_line_is_erased() {
        let (result, tip) = hire_tooltip(true, HiringMode::Default, 0, 5);
        assert_eq!(result, Err(SharedRefusal::HousingHandledElsewhere));
        assert_eq!(tip, Some(HIRE_WARNING_KEY), "set in the else branch, :316");
        let (result, tip) = hire_tooltip(false, HiringMode::Manual, 5, 5);
        assert_eq!(result, Err(SharedRefusal::NoRoom));
        assert_eq!(tip, None, "the permitted branch sets an empty tooltip, :311");
        let (result, tip) = hire_tooltip(false, HiringMode::Manual, 0, 5);
        assert_eq!(result, Ok(()));
        assert_eq!(tip, None);

        let (result, tip) = fire_tooltip(false, HiringMode::Manual, true);
        assert_eq!(result, Err(SharedRefusal::Travelling));
        assert_eq!(tip, None, "WindowAssignCitizen.java:391 erases the :384 tooltip");
        let (result, tip) = fire_tooltip(true, HiringMode::Default, false);
        assert_eq!(result, Err(SharedRefusal::HousingHandledElsewhere));
        assert_eq!(tip, Some(HIRE_WARNING_KEY));
        assert_eq!(TRAVELLING_KEY, "com.minecolonies.coremod.gui.home.travelling");
    }

    #[test]
    fn the_mode_button_walks_four_states_on_a_home_and_sends_one_message() {
        let seen: Vec<HiringMode> = (0..4).scan(HiringMode::Default, |mode, _| {
            let click = mode_click(*mode);
            *mode = click.next;
            Some(click.next)
        }).collect();
        assert_eq!(seen, [HiringMode::Auto, HiringMode::Manual, HiringMode::Locked, HiringMode::Default]);
        let click = mode_click(HiringMode::Manual);
        assert_eq!(click.next, HiringMode::Locked, "a home reaches LOCKED, a workplace does not");
        assert_eq!(click.label, "com.minecolonies.coremod.gui.hiringmode.locked");
        assert_eq!(click.effect, Effect::Sends("BuildingHiringModeMessage"));
        assert_eq!(MODE_PREFIX, "com.minecolonies.coremod.gui.hiringmode.");
    }

    #[test]
    fn cancel_needs_a_town_hall_to_return_to() {
        assert_eq!(cancel(true), Ok(Effect::Reopens));
        assert_eq!(cancel(false), Err(Refusal::NoTownHall));
        assert_eq!(Refusal::NoTownHall.key(), None);
    }

    #[test]
    fn the_row_panes_of_this_module_are_the_ones_src_ui_declares() {
        let screen = screen();
        assert!(screen.is_some(), "src/ui lost the assign screen");
        let Some(screen) = screen else { return };
        assert_eq!(screen.id, crate::ui::screen::ASSIGN_ID);
        for (list, pane) in ROW_PANES {
            // The list is a row field of the same screen …
            assert!(
                screen.fields.iter().any(|field| field.id == list && field.value == Value::Row),
                "{list} is not a list of the assign screen"
            );
            // … and the pane is declared in that list's row scope, from a citizen.
            let declared = screen
                .fields
                .iter()
                .find(|field| field.id == pane && field.scope == Scope::Row(list));
            let Some(field) = declared else {
                panic!("{list}.{pane} is not declared by src/ui");
            };
            assert_eq!(field.source, Source::Citizen);
            // The name pane holds a name and the composed pane holds text: two kinds, one row.
            let expected = if pane == "citizen" { Value::Name } else { Value::Text };
            assert_eq!(field.value, expected, "{list}.{pane}");
        }
        // The two buttons this module describes are the same two `src/ui` registers.
        assert_eq!(screen.actions.iter().filter(|a| a.id == "hire").count(), 1);
        assert_eq!(screen.actions.iter().filter(|a| a.id == "fire").count(), 1);
    }
}
