//! What a button does, and why it can refuse — the actions a hut screen renders.
//!
//! Java reference: `AbstractWindowSkeleton.registerButton` binds a **pane id** to a
//! handler (`core/client/gui/AbstractWindowSkeleton.java:130`); every window in
//! `core/client/gui/` registers its buttons that way (`AbstractBuildingMainWindow.java:47-51`,
//! `AbstractWindowWorkerModuleBuilding.java:90-94`, `WindowHutLiving.java:62-63`), and
//! the handler either opens another window or sends one of the server messages under
//! `core/network/messages/server/`. An [`Action`] here is that `(id, label, effect,
//! refusals)` quadruple: what the click asks for, and every way the game says no.
//!
//! **This port has no client and no network.** There is no `Network.getNetwork()` to
//! send through, so an action whose effect is a message carries the **class name** of
//! that message and nothing else: the name is the whole of the effect until a client
//! slice exists, and [`Effect::Sends`] says so rather than pretending a call happened.
//!
//! **A refusal is named, and two of them are not Java's.** Java's player-facing
//! refusals are `lang` keys raised by an interaction (`MessageUtils.format(...)`);
//! Java's *other* refusals are silent — a button that is `disable()`d, or a whole
//! button that is never added (`WindowHireWorker.java:515-518`). Both are real
//! refusals and both get a name here: [`Refusal::key`] answers with Java's key when
//! there is one, and `None` when the refusal is only ever a disabled button, so a
//! caller can never print our name where the player expects a sentence.

use crate::jobs::Refusal as EmploymentRefusal;

/// What a click does once nothing refused it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set, and a new kind is a change here
pub enum Effect {
    /// Opens another screen **of this table**, by [`crate::ui::Screen::id`].
    Opens(&'static str),
    /// Opens a Java window this table does not describe, by class name — the build
    /// tool (`AbstractBuildingMainWindow.java:85`), the info page (`:94`), the spy
    /// list (`WindowBarracksBuilding.java:106`). Naming the class is the honest
    /// answer: the window is not a hut screen and this slice does not carry it.
    OpensJava(&'static str),
    /// Reopens the screen that opened this one, `building.openGui(false)`
    /// (`WindowAssignCitizen.java:174-178`, `WindowHutNameEntry.java:72-75`).
    Reopens,
    /// Sends one of Java's server messages, by class name. Nothing is sent here.
    Sends(&'static str),
    /// Opens the game's own container screen for the hut block. Java's route is
    /// `OpenInventoryMessage` (`AbstractBuildingMainWindow.java:100-103`); this port
    /// already declares the container instead (`src/hut.rs`, `hut::container`), which
    /// is what `src/menu.rs` explains at length.
    Container,
    /// Edits the building's custom name locally, then sends `HutRenameMessage`
    /// (`AbstractBuildingView.java:639-644`). See [`rename`] for the truncation.
    Renames,
    /// Flips a boolean in the window and nothing else: the "show employed" toggle
    /// (`WindowHireWorker.java:313-319`).
    Toggles,
    /// Steps a local counter through a fixed number of states: the all-inventory sort
    /// (`WindowHutAllInventory.java:163-200`), see [`next_sort`].
    Cycles,
    /// Draws a timed highlight box in the world (`WindowHutAllInventory.java:117-151`).
    /// No window, no server call.
    Highlights,
}

/// A button of a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    /// The XML button id, which is what `registerButton` binds
    /// (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// The label's translation key; `""` for a button whose label the window writes
    /// later or leaves empty (`pause` starts empty, `WindowHireWorker.java:437`).
    pub label: &'static str,
    /// What the click does.
    pub effect: Effect,
    /// Every way the click can be refused, in the order Java checks them. Each
    /// variant's doc says **where** Java applies it — a disabled button, a hidden
    /// button, or a message after the click.
    pub refusals: &'static [Refusal],
}

impl Action {
    /// A button with no refusal of its own; the server may still refuse later, and
    /// then the refusal is the message's.
    pub const fn plain(id: &'static str, label: &'static str, effect: Effect) -> Action {
        Action { id, label, effect, refusals: &[] }
    }

    /// A button Java can refuse.
    pub const fn guarded(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        refusals: &'static [Refusal],
    ) -> Action {
        Action { id, label, effect, refusals }
    }

    /// Whether Java ever shows this button in a state that does something: every action
    /// of this table does, which is why [`Action::refusals`] is the whole story.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }
}

/// Why a click did nothing, and whether the player is ever told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set
pub enum Refusal {
    /// The hut is placed and not built: `level == 0` refuses every hire and every
    /// assignment. Java's message is `COM_MINECOLONIES_COREMOD_GUI_WORKERHUTS_LEVEL_0`
    /// (`TranslationConstants.java:75`), raised at
    /// `AbstractWindowWorkerModuleBuilding.java:145-149` and `WindowHutLiving.java:117-121`.
    NotBuilt,
    /// Every place is taken: `WorkerBuildingModuleView.isFull()`
    /// (`WorkerBuildingModuleView.java:197-201`), or `residents >= max` for a home
    /// (`WindowAssignCitizen.java:105`, `:303`). **Java says nothing**: the button is
    /// disabled, so this refusal has no key.
    NoRoom,
    /// The citizen cannot take this job: a child, or employed at a hut whose module
    /// refuses to be hired away from
    /// (`WorkerBuildingModuleView.canAssign`, `WorkerBuildingModuleView.java:155-162`).
    /// Silent too — the row's `done` button is turned off
    /// (`WindowHireWorker.java:407-415`).
    CannotAssign,
    /// The town hall hands out homes itself, or the player asked for it: Java disables
    /// the assign and unassign buttons with the `hire.warning` tooltip
    /// (`WindowAssignCitizen.java:301-317`, `:379-397`). Key:
    /// `com.minecolonies.coremod.gui.home.hire.warning`.
    HousingHandledElsewhere,
    /// The citizen is on its way somewhere: unassigning is refused, with the
    /// `home.travelling` tooltip (`WindowAssignCitizen.java:381-385`).
    Travelling,
    /// A recall could not move one citizen (`WARNING_CITIZEN_RECALL_FAILED`,
    /// `TranslationConstants.java:612`, raised at `RecallCitizenMessage.java:97-99`).
    RecallFailed,
    /// A forced pickup asked for something the hut cannot give
    /// (`COM_MINECOLONIES_COREMOD_ENTITY_DELIVERYMAN_FORCEPICKUP_FAILED`,
    /// `TranslationConstants.java:45`, raised at `ForcePickupMessage.java:76-81`).
    PickupFailed,
    /// The job's research is not done, so its button is **never added** to the hire
    /// window (`WindowHireWorker.java:515-518`). Silent.
    ResearchMissing,
    /// The barracks hides its spies button below level 3
    /// (`WindowBarracksBuilding.java:92-96`, `BUILDING_LEVEL_FOR_LIST`). Silent.
    SpiesBelowLevel,
    /// The builder's reserve row is `DONT_HAVE`: the ↥ button is disabled
    /// (`WindowBuilderResModule.java:166-171`).
    ResourceMissing,
    /// The row is `NOT_NEEDED` — the builder already holds enough — so ↥ is disabled
    /// (`WindowBuilderResModule.java:184-190`).
    ResourceNotNeeded,
    /// A mine level already has a work order queued, so `repair` is disabled
    /// (`WindowHutMinerModule.java:99`).
    WorkOrderExists,
    /// The mine level shown is the active one, so `mine` is disabled
    /// (`WindowHutMinerModule.java:100`).
    CurrentLevel,
    /// The hut has no assignable module at all, so the hire window closes itself
    /// (`WindowHireWorker.java:112-115` and `:385-389`).
    NoAssignableModule,
    /// No job works here: the hire window is asked for a hut whose module list is
    /// empty. This port's own, for the table in [`crate::ui::table`].
    NoScreenForJob { job: crate::jobs::Job },
    /// The building has a window, but not a **hut** screen: the town hall's
    /// `WindowMainPage` and the postbox's `WindowPostBoxMain` live outside the
    /// `WindowHut*` set this slice describes.
    NotAHutScreen { hut: crate::building::Kind },
    /// A table row names a screen id that [`crate::ui::SCREENS`] does not hold. It is
    /// only reachable through a broken constant, which is why a test walks every row.
    UnknownScreen,
    /// The hut refuses this job, with the reason the jobs table already gives
    /// ([`crate::jobs::Refusal`]). Forwarded so the two tables cannot disagree.
    HutRefuses(EmploymentRefusal),
}

impl Refusal {
    /// A stable name, namespaced so it can never be mistaken for a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NotBuilt => "ui.refusal.not_built",
            Refusal::NoRoom => "ui.refusal.full",
            Refusal::CannotAssign => "ui.refusal.cannot_assign",
            Refusal::HousingHandledElsewhere => "ui.refusal.housing_not_manual",
            Refusal::Travelling => "ui.refusal.travelling",
            Refusal::RecallFailed => "ui.refusal.recall_failed",
            Refusal::PickupFailed => "ui.refusal.pickup_failed",
            Refusal::ResearchMissing => "ui.refusal.research",
            Refusal::SpiesBelowLevel => "ui.refusal.spies_level",
            Refusal::ResourceMissing => "ui.refusal.resource_missing",
            Refusal::ResourceNotNeeded => "ui.refusal.resource_not_needed",
            Refusal::WorkOrderExists => "ui.refusal.work_order",
            Refusal::CurrentLevel => "ui.refusal.current_level",
            Refusal::NoAssignableModule => "ui.refusal.no_module",
            Refusal::NoScreenForJob { .. } => "ui.refusal.no_screen",
            Refusal::NotAHutScreen { .. } => "ui.refusal.not_a_hut_screen",
            Refusal::UnknownScreen => "ui.refusal.unknown_screen",
            Refusal::HutRefuses(inner) => inner.name(),
        }
    }

    /// Java's translation key for this refusal, when Java tells the player anything.
    ///
    /// `None` is not "no reason": it is "Java only ever disables the button", which is
    /// why a caller must not fall back to [`Self::name`] as a sentence.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::NotBuilt => Some("com.minecolonies.coremod.gui.workerhuts.level0"),
            Refusal::HousingHandledElsewhere => {
                Some("com.minecolonies.coremod.gui.home.hire.warning")
            }
            Refusal::Travelling => Some("com.minecolonies.coremod.gui.home.travelling"),
            Refusal::RecallFailed => Some("com.minecolonies.coremod.workerhuts.recallfail"),
            Refusal::PickupFailed => Some("entity.deliveryman.forcepickupfailed"),
            Refusal::NoRoom
            | Refusal::CannotAssign
            | Refusal::ResearchMissing
            | Refusal::SpiesBelowLevel
            | Refusal::ResourceMissing
            | Refusal::ResourceNotNeeded
            | Refusal::WorkOrderExists
            | Refusal::CurrentLevel
            | Refusal::NoAssignableModule
            | Refusal::NoScreenForJob { .. }
            | Refusal::NotAHutScreen { .. }
            | Refusal::UnknownScreen
            | Refusal::HutRefuses(_) => None,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// How a building takes its workers, in Java's order: `HiringMode.java:8-13`.
///
/// **The order matters**: `switchHiringMode` walks the enum's ordinals, so the cycle is
/// a fact about the declaration, not about the window
/// (`WindowHireWorker.java:166-181`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: four modes in Java, declared in this order
pub enum HiringMode {
    /// Whatever the colony's settings say.
    Default,
    /// The hut hires by itself.
    Auto,
    /// The player assigns, one citizen at a time.
    Manual,
    /// Nobody may be assigned — **homes only** (`WindowHireWorker.java:169-172`).
    Locked,
}

impl HiringMode {
    /// Every mode, in Java's declaration order.
    pub const ALL: [HiringMode; 4] =
        [HiringMode::Default, HiringMode::Auto, HiringMode::Manual, HiringMode::Locked];

    /// The label's key, one per mode (`TranslationConstants.java:484-490`).
    pub fn key(self) -> &'static str {
        match self {
            HiringMode::Default => "com.minecolonies.coremod.gui.hiringmode.default",
            HiringMode::Auto => "com.minecolonies.coremod.gui.hiringmode.auto",
            HiringMode::Manual => "com.minecolonies.coremod.gui.hiringmode.manual",
            HiringMode::Locked => "com.minecolonies.coremod.gui.hiringmode.locked",
        }
    }

    /// Java's `name().toLowerCase(Locale.US)`, used to build the assignment window's
    /// key (`WindowAssignCitizen.java:187`).
    pub fn slug(self) -> &'static str {
        match self {
            HiringMode::Default => "default",
            HiringMode::Auto => "auto",
            HiringMode::Manual => "manual",
            HiringMode::Locked => "locked",
        }
    }
}

/// The mode the `mode` button moves to.
///
/// **Two different cycles, and the difference is real**: a workplace walks
/// Default → Auto → Manual → Default, because Java skips `LOCKED` with the comment
/// "only homes can be locked, not workplaces" (`WindowHireWorker.java:166-181`); a home
/// walks all four (`WindowAssignCitizen.java:154-165`).
pub fn next_hiring_mode(mode: HiringMode, home: bool) -> HiringMode {
    let next = match mode {
        HiringMode::Default => HiringMode::Auto,
        HiringMode::Auto => HiringMode::Manual,
        HiringMode::Manual => {
            if home {
                HiringMode::Locked
            } else {
                HiringMode::Default
            }
        }
        HiringMode::Locked => HiringMode::Default,
    };
    next
}

/// Whether the player assigns citizens to this home by hand.
///
/// Java's condition, verbatim: `(!autoHousing && mode == DEFAULT) || mode == MANUAL`
/// (`WindowAssignCitizen.java:301`, `:379`). `LOCKED` falls in the other branch, which
/// is Java's own behaviour for a locked home — the button is disabled with the same
/// `hire.warning` tooltip, not a different one.
pub fn hands_on_housing(auto_housing: bool, mode: HiringMode) -> bool {
    (!auto_housing && mode == HiringMode::Default) || mode == HiringMode::Manual
}

/// Whether the `assign` (hire) button of a home may be used.
pub fn may_assign(
    auto_housing: bool,
    mode: HiringMode,
    residents: i32,
    places: i32,
) -> Result<(), Refusal> {
    if !hands_on_housing(auto_housing, mode) {
        return Err(Refusal::HousingHandledElsewhere);
    }
    if residents >= places {
        return Err(Refusal::NoRoom);
    }
    Ok(())
}

/// Whether the `unassign` (fire) button of a home may be used.
///
/// The same housing rule, plus the travelling check Java adds on the assigned list
/// (`WindowAssignCitizen.java:381-389`).
pub fn may_unassign(
    auto_housing: bool,
    mode: HiringMode,
    travelling: bool,
) -> Result<(), Refusal> {
    if !hands_on_housing(auto_housing, mode) {
        return Err(Refusal::HousingHandledElsewhere);
    }
    if travelling {
        return Err(Refusal::Travelling);
    }
    Ok(())
}

/// The longest name a hut keeps: fifteen characters
/// (`WindowHutNameEntry.java:28`, `MAX_NAME_LENGTH`).
///
/// **The XML asks for twenty-five** (`windowhutnameentry.xml:6`, `maxlength="25"`), so a
/// player can type a name the class then cuts. That is Java's behaviour, and this port
/// keeps it rather than silently tightening the field: the field's limit and the
/// building's limit are two different numbers, and only the second one is a rule.
pub const MAX_NAME_LENGTH: usize = 15;

/// The field's own limit, which the XML imposes and no class checks
/// (`windowhutnameentry.xml:6`).
pub const NAME_FIELD_MAX_LENGTH: usize = 25;

/// What a rename does: the name that is kept, and whether the player was warned
/// (`WARNING_NAME_TOO_LONG`, `TranslationConstants.java:480`, raised at
/// `WindowHutNameEntry.java:59-63`).
///
/// The rename **still happens** — Java truncates and sends — so this is a warning, not
/// a refusal, and it is a return value rather than a [`Refusal`] variant.
pub fn rename(name: &str) -> (String, bool) {
    if name.chars().count() > MAX_NAME_LENGTH {
        (name.chars().take(MAX_NAME_LENGTH).collect(), true)
    } else {
        (name.to_owned(), false)
    }
}

/// The number of states the all-inventory sort walks
/// (`WindowHutAllInventory.java:163-168`: `NO_SORT`, `ASC_SORT`, `DESC_SORT`,
/// `COUNT_ASC_SORT`, `COUNT_DESC_SORT`).
pub const SORT_STAGES: u8 = 5;

/// The next sort state, wrapping at [`SORT_STAGES`]
/// (`WindowHutAllInventory.java:163-168`).
pub fn next_sort(stage: u8) -> u8 {
    if stage + 1 >= SORT_STAGES {
        0
    } else {
        stage + 1
    }
}

/// The label the `build` button shows, one per state
/// (`AbstractBuildingMainWindow.updateButtonBuild`, `:133-163`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five labels Java's button can carry
pub enum BuildLabel {
    /// Idle: "build or repair".
    BuildOrRepair,
    /// Building, level 0: cancel the build.
    CancelBuild,
    /// Building, level 1+: cancel the upgrade.
    CancelUpgrade,
    /// Repairing: cancel the repair.
    CancelRepair,
    /// Deconstructing: cancel the deconstruction.
    CancelDeconstruction,
}

impl BuildLabel {
    /// The label's key (`TranslationConstants.java:462-470`).
    pub fn key(self) -> &'static str {
        match self {
            BuildLabel::BuildOrRepair => "com.minecolonies.coremod.gui.workerhuts.buildrepair",
            BuildLabel::CancelBuild => "com.minecolonies.coremod.gui.workerhuts.cancelbuild",
            BuildLabel::CancelUpgrade => "com.minecolonies.coremod.gui.workerhuts.cancelupgrade",
            BuildLabel::CancelRepair => "com.minecolonies.coremod.gui.workerhuts.cancelrepair",
            BuildLabel::CancelDeconstruction => {
                "com.minecolonies.coremod.gui.workerhuts.canceldeconstruction"
            }
        }
    }

    /// The mode `BuildRequestMessage` carries in this state, `None` when the button
    /// opens the build tool instead (`AbstractBuildingMainWindow.java:71-86`).
    ///
    /// Java decides by **reading its own button's label** — the click compares the
    /// `TranslatableContents` key against the five constants (`:67-71`) — which is why
    /// this function takes a label rather than a building: the label *is* the state.
    pub fn build_request_mode(self) -> Option<&'static str> {
        match self {
            BuildLabel::BuildOrRepair => None,
            BuildLabel::CancelBuild | BuildLabel::CancelUpgrade => Some("BUILD"),
            BuildLabel::CancelRepair => Some("REPAIR"),
            BuildLabel::CancelDeconstruction => Some("REMOVE"),
        }
    }

    /// What one click on the button does in this state.
    pub fn effect(self) -> Effect {
        match self.build_request_mode() {
            Some(_) => Effect::Sends("BuildRequestMessage"),
            None => Effect::OpensJava("WindowBuildBuilding"),
        }
    }
}

/// Which label the build button shows, in Java's order of tests
/// (`AbstractBuildingMainWindow.java:140-162`).
///
/// A deconstruction is only read when nothing is being built and nothing repaired,
/// which is why the order is part of the rule: a hut that is building *and* flagged for
/// repair shows the build label.
pub fn build_label(
    building: bool,
    level: i32,
    repairing: bool,
    deconstructing: bool,
) -> BuildLabel {
    if building {
        if level == 0 {
            BuildLabel::CancelBuild
        } else {
            BuildLabel::CancelUpgrade
        }
    } else if repairing {
        BuildLabel::CancelRepair
    } else if deconstructing {
        BuildLabel::CancelDeconstruction
    } else {
        BuildLabel::BuildOrRepair
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        let all = [
            Refusal::NotBuilt,
            Refusal::NoRoom,
            Refusal::CannotAssign,
            Refusal::HousingHandledElsewhere,
            Refusal::Travelling,
            Refusal::RecallFailed,
            Refusal::PickupFailed,
            Refusal::ResearchMissing,
            Refusal::SpiesBelowLevel,
            Refusal::ResourceMissing,
            Refusal::ResourceNotNeeded,
            Refusal::WorkOrderExists,
            Refusal::CurrentLevel,
            Refusal::NoAssignableModule,
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("ui.refusal."), "{refusal} ");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(all.len(), 14, "the fourteen refusals no job is needed to name");
    }

    #[test]
    fn only_five_refusals_are_told_to_the_player() {
        let keyed: Vec<&str> = [
            Refusal::NotBuilt,
            Refusal::NoRoom,
            Refusal::HousingHandledElsewhere,
            Refusal::Travelling,
            Refusal::RecallFailed,
            Refusal::PickupFailed,
        ]
        .iter()
        .filter(|refusal| refusal.key().is_some())
        .map(|refusal| refusal.name())
        .collect();
        // Java disables a button and says nothing for the rest.
        assert_eq!(keyed.len(), 5);
        assert_eq!(Refusal::NotBuilt.key(), Some("com.minecolonies.coremod.gui.workerhuts.level0"));
        assert_eq!(Refusal::Travelling.key(), Some("com.minecolonies.coremod.gui.home.travelling"));
        assert_eq!(
            Refusal::RecallFailed.key(),
            Some("com.minecolonies.coremod.workerhuts.recallfail")
        );
        assert_eq!(
            Refusal::PickupFailed.key(),
            Some("entity.deliveryman.forcepickupfailed")
        );
        assert_eq!(Refusal::NoRoom.key(), None);
        assert_eq!(Refusal::SpiesBelowLevel.key(), None);
    }

    #[test]
    fn a_forwarded_refusal_keeps_the_jobs_table_name() {
        let inner = EmploymentRefusal::BuildingEmploysNobody { hut: crate::building::Kind::Tavern };
        assert_eq!(Refusal::HutRefuses(inner).name(), inner.name());
        assert_eq!(Refusal::HutRefuses(inner).key(), None);
        let other = EmploymentRefusal::JobNotEmployedHere {
            hut: crate::building::Kind::Farmer,
            job: crate::jobs::Job::Knight,
        };
        // Two different inner refusals stay two different names.
        assert_ne!(Refusal::HutRefuses(inner).name(), Refusal::HutRefuses(other).name());
    }

    #[test]
    fn a_workplace_never_reaches_locked_and_a_home_does() {
        assert_eq!(
            next_hiring_mode(HiringMode::Manual, false),
            HiringMode::Default,
            "WindowHireWorker.java:169-172 skips LOCKED for workplaces"
        );
        assert_eq!(next_hiring_mode(HiringMode::Manual, true), HiringMode::Locked);
        assert_eq!(next_hiring_mode(HiringMode::Locked, true), HiringMode::Default);
        assert_eq!(next_hiring_mode(HiringMode::Default, true), HiringMode::Auto);
        assert_eq!(next_hiring_mode(HiringMode::Auto, true), HiringMode::Manual);
    }

    #[test]
    fn the_workplace_cycle_is_three_steps_and_the_home_cycle_is_four() {
        let mut mode = HiringMode::Default;
        for _ in 0..3 {
            mode = next_hiring_mode(mode, false);
        }
        assert_eq!(mode, HiringMode::Default);
        let mut home = HiringMode::Default;
        for _ in 0..4 {
            home = next_hiring_mode(home, true);
        }
        assert_eq!(home, HiringMode::Default);
        // Every mode a workplace's cycle can reach is one of three.
        let mut seen = Vec::new();
        let mut walk = HiringMode::Default;
        for _ in 0..3 {
            seen.push(walk);
            walk = next_hiring_mode(walk, false);
        }
        assert!(!seen.contains(&HiringMode::Locked));
    }

    #[test]
    fn the_four_hiring_mode_labels_are_four_keys() {
        let keys: Vec<&str> = HiringMode::ALL.iter().map(|mode| mode.key()).collect();
        assert_eq!(keys.len(), 4);
        for (index, key) in keys.iter().enumerate() {
            assert!(key.starts_with("com.minecolonies.coremod.gui.hiringmode."));
            for other in &keys[index + 1..] {
                assert_ne!(key, other);
            }
        }
        assert_eq!(HiringMode::Locked.slug(), "locked");
        assert_eq!(HiringMode::Default.slug(), "default");
    }

    #[test]
    fn auto_housing_takes_the_assign_button_away() {
        // The town hall hands out homes: Java disables the button, mode or no mode.
        assert_eq!(
            may_assign(true, HiringMode::Default, 0, 5),
            Err(Refusal::HousingHandledElsewhere)
        );
        assert_eq!(
            may_assign(false, HiringMode::Default, 0, 5),
            Ok(()),
            "no auto housing and a default mode is the hands-on branch"
        );
        assert_eq!(may_assign(true, HiringMode::Manual, 0, 5), Ok(()));
        assert_eq!(
            may_assign(false, HiringMode::Locked, 0, 5),
            Err(Refusal::HousingHandledElsewhere),
            "a locked home is disabled with the same tooltip, WindowAssignCitizen.java:379-397"
        );
    }

    #[test]
    fn a_full_home_refuses_before_it_reads_the_citizen() {
        assert_eq!(may_assign(false, HiringMode::Manual, 5, 5), Err(Refusal::NoRoom));
        assert_eq!(may_assign(false, HiringMode::Manual, 4, 5), Ok(()));
        // A level-0 home has no places, so the same call refuses.
        assert_eq!(may_assign(false, HiringMode::Manual, 0, 0), Err(Refusal::NoRoom));
    }

    #[test]
    fn an_unassign_is_refused_for_a_travelling_citizen() {
        assert_eq!(may_unassign(false, HiringMode::Manual, true), Err(Refusal::Travelling));
        assert_eq!(may_unassign(false, HiringMode::Manual, false), Ok(()));
        assert_eq!(
            may_unassign(true, HiringMode::Default, false),
            Err(Refusal::HousingHandledElsewhere)
        );
    }

    #[test]
    fn a_name_is_cut_at_fifteen_and_the_player_is_told() {
        let (kept, warned) = rename("Builder Hut Number One");
        assert_eq!(kept, "Builder Hut Num");
        assert!(warned);
        assert_eq!(kept.chars().count(), MAX_NAME_LENGTH);
        // The XML lets twenty-five characters through, so the warning is reachable.
        assert_eq!(NAME_FIELD_MAX_LENGTH, 25);
        assert!(NAME_FIELD_MAX_LENGTH > MAX_NAME_LENGTH);
    }

    #[test]
    fn a_short_name_is_kept_whole_and_nothing_is_warned() {
        let (kept, warned) = rename("Farm");
        assert_eq!(kept, "Farm");
        assert!(!warned);
        let (empty, warned) = rename("");
        assert_eq!(empty, "");
        assert!(!warned, "an empty name is cleared, not truncated");
        // Exactly fifteen is not too long.
        let (edge, warned) = rename("0123456789abcde");
        assert_eq!(edge, "0123456789abcde");
        assert!(!warned);
    }

    #[test]
    fn the_all_inventory_sort_walks_five_states_and_wraps() {
        let mut stage = 0u8;
        let mut seen = Vec::new();
        for _ in 0..SORT_STAGES {
            seen.push(stage);
            stage = next_sort(stage);
        }
        assert_eq!(seen, [0, 1, 2, 3, 4]);
        assert_eq!(stage, 0, "WindowHutAllInventory.java:164-168 wraps to NO_SORT");
        assert_eq!(next_sort(4), 0);
        assert_eq!(next_sort(0), 1);
    }

    #[test]
    fn the_build_button_reads_building_first_then_repair_then_deconstruction() {
        assert_eq!(build_label(true, 0, false, false), BuildLabel::CancelBuild);
        assert_eq!(build_label(true, 3, false, false), BuildLabel::CancelUpgrade);
        // Building wins over repairing, which is Java's order of tests.
        assert_eq!(build_label(true, 2, true, false), BuildLabel::CancelUpgrade);
        assert_eq!(build_label(false, 2, true, true), BuildLabel::CancelRepair);
        assert_eq!(build_label(false, 2, false, true), BuildLabel::CancelDeconstruction);
        assert_eq!(build_label(false, 2, false, false), BuildLabel::BuildOrRepair);
    }

    #[test]
    fn the_five_build_labels_are_five_keys() {
        let labels = [
            BuildLabel::BuildOrRepair,
            BuildLabel::CancelBuild,
            BuildLabel::CancelUpgrade,
            BuildLabel::CancelRepair,
            BuildLabel::CancelDeconstruction,
        ];
        for (index, label) in labels.iter().enumerate() {
            assert!(label.key().starts_with("com.minecolonies.coremod.gui.workerhuts."));
            for other in &labels[index + 1..] {
                assert_ne!(label.key(), other.key());
            }
        }
    }

    #[test]
    fn the_build_button_cancels_from_three_states_and_opens_the_tool_from_the_other_two() {
        assert_eq!(BuildLabel::BuildOrRepair.build_request_mode(), None);
        assert_eq!(
            BuildLabel::BuildOrRepair.effect(),
            Effect::OpensJava("WindowBuildBuilding")
        );
        assert_eq!(BuildLabel::CancelBuild.build_request_mode(), Some("BUILD"));
        assert_eq!(BuildLabel::CancelUpgrade.build_request_mode(), Some("BUILD"));
        assert_eq!(BuildLabel::CancelRepair.build_request_mode(), Some("REPAIR"));
        assert_eq!(BuildLabel::CancelDeconstruction.build_request_mode(), Some("REMOVE"));
        // The three cancel states cancel three different things; a fourth mode would
        // have to be a new variant of BuildLabel.
        for label in [
            BuildLabel::CancelBuild,
            BuildLabel::CancelUpgrade,
            BuildLabel::CancelRepair,
            BuildLabel::CancelDeconstruction,
        ] {
            assert_eq!(label.effect(), Effect::Sends("BuildRequestMessage"));
        }
    }

    #[test]
    fn an_action_keeps_its_id_its_label_and_its_refusals() {
        let hire = Action::guarded(
            "hire",
            "com.minecolonies.coremod.gui.workerhuts.manage",
            Effect::Opens("hire"),
            &[Refusal::NotBuilt],
        );
        assert_eq!(hire.id, "hire");
        assert_eq!(hire.effect, Effect::Opens("hire"));
        assert!(hire.is_guarded());
        let prio = Action::plain("-", "-", Effect::Sends("ChangeDeliveryPriorityMessage"));
        assert!(!prio.is_guarded());
        assert_eq!(prio.refusals.len(), 0);
        assert_eq!(Action::plain("x", "", Effect::Reopens).label, "");
    }
}
