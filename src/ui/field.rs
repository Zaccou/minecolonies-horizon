//! A screen field: one pane Java fills, what it holds, and where its value comes from.
//!
//! Java reference: `core/client/gui/` — a window is a BlockUI XML file
//! (`src/main/resources/assets/minecolonies/gui/`) plus a class that looks its panes up
//! **by id** and writes them in `onOpened` (`AbstractBuildingMainWindow.java:53` and
//! `:171-176` for the title, `AbstractWindowWorkerModuleBuilding.java:162-203` for the
//! worker list, `WindowBuilderResModule.java:112-148` for the reserves list). A
//! [`Field`] here is that `(id, value, source)` triple and nothing else: **the
//! position, the size, the colour and the sprite belong to the XML**, and this slice
//! never repeats them. `WindowBuilderResModule.java:193-196` even computes a button's
//! position from the row's width, which is exactly the kind of thing a data table must
//! not try to carry.
//!
//! **The four values the brief names are not on every screen, and that is the finding.**
//! A worker hut's window shows a name, a level, a worker name and a delivery priority —
//! **no progress and no stock at all**. Progress is the builder's reserves tab
//! (`WindowBuilderResModule.java:105`, `:147`) and stock is its resource rows (`:209`)
//! or the separate all-inventory window (`WindowHutAllInventory.java:254-272`).
//! [`Value::Progress`], [`Value::Stock`] and [`Value::Occupancy`] exist for the panes
//! that really hold them; [`crate::ui::table`] and the module note hold the counts.

/// What a field holds, as a kind rather than a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: this port's own closed set, so a new pane kind is a change here
pub enum Value {
    /// A name: the building's display name, a citizen's, an item's.
    Name,
    /// A building level, `0..=5` (`hut::level_of` clamps it).
    Level,
    /// A percentage, `0..=100`, computed — never read from a record.
    Progress,
    /// "step 3 / 7" of a build (`WindowBuilderResModule.java:147`).
    Step,
    /// "available / needed" for one resource (`WindowBuilderResModule.java:209`).
    Stock,
    /// The hut's delivery priority, `0..=10` (`AbstractBuildingContainer.java:122-125`).
    Priority,
    /// "residents / places" in a home (`WindowHutLiving.java:108`).
    Occupancy,
    /// A plain integer: a node count, a hidden row index, a stack size.
    Count,
    /// A row of a list; the row's own fields are separate entries.
    Row,
    /// A text field the player types in.
    Input,
    /// An item icon.
    Icon,
    /// Text the screen builds itself, out of two or three other values.
    Text,
}

impl Value {
    /// Every value kind, in the order this file declares them.
    pub const ALL: [Value; 12] = [
        Value::Name,
        Value::Level,
        Value::Progress,
        Value::Step,
        Value::Stock,
        Value::Priority,
        Value::Occupancy,
        Value::Count,
        Value::Row,
        Value::Input,
        Value::Icon,
        Value::Text,
    ];

    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            Value::Name => "name",
            Value::Level => "level",
            Value::Progress => "progress",
            Value::Step => "step",
            Value::Stock => "stock",
            Value::Priority => "priority",
            Value::Occupancy => "occupancy",
            Value::Count => "count",
            Value::Row => "row",
            Value::Input => "input",
            Value::Icon => "icon",
            Value::Text => "text",
        }
    }

    /// Whether the screen **computes** this value instead of reading one record.
    ///
    /// The four computed kinds are the ones Java builds out of two numbers in the
    /// window class, not out of one getter: a percentage from the player's contribution
    /// (`WindowBuilderResModule.java:103-106`), a step from two ints (`:147`), a stock
    /// line from two ints (`:209`), an occupancy from two ints
    /// (`WindowHutLiving.java:108`). A caller that reads the model therefore has to
    /// supply both numbers, and can never read the pane alone.
    pub fn is_computed(self) -> bool {
        matches!(
            self,
            Value::Progress | Value::Step | Value::Stock | Value::Occupancy
        )
    }
}

/// Where a field's value comes from: the part of the model the screen reads.
///
/// One variant per source Java reads from, named after the accessor, so a reader can
/// go from a row of the table to the class that answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the sources Java's hut windows read, and no more
pub enum Source {
    /// The building view itself: `IBuildingView.getBuildingDisplayName()`,
    /// `getBuildingLevel()`, `getCustomName()`, `getPickUpPriority()`.
    Building,
    /// The colony behind it: `getColony()`, its citizens and its last spawn points
    /// (`WindowBarracksBuilding.java:115`).
    Colony,
    /// The hut's own saved compound — `crate::hut::State`, which is what this port
    /// has instead of `IBuildingView`.
    Hut,
    /// One building module's view, named by its **module key**
    /// (`BuildingModules.java:52-53`, `:398`, `:444`, `:497`); the same key
    /// [`crate::building::Module::key`] saves.
    Module(&'static str),
    /// A citizen's record (`crate::colony::Citizen`), reached through a module's
    /// assigned ids (`WorkerBuildingModuleView.getAssignedCitizens`).
    Citizen,
    /// The **client** player's own inventory, read only to colour a resource row
    /// (`WindowBuilderResModule.java:76-97`). Nothing here is saved.
    PlayerInventory,
    /// The building's work order, by id (`BuildingResourcesModuleView.getWorkOrderId`).
    WorkOrder,
    /// A request in the colony's request manager
    /// (`WindowHutRequestTaskModule.java:63-117`).
    Request,
    /// An advancement's progress (`WindowHutBuilderModule.java:55-63`).
    Advancement,
}

impl Source {
    /// Whether the value lives in the colony rather than in the hut or the player.
    pub fn is_colony_data(self) -> bool {
        matches!(self, Source::Colony | Source::Citizen | Source::Request)
    }
}

/// Where a field's pane id is looked up.
///
/// **A row's panes are not the window's.** BlockUI resolves a row's `findPaneOfTypeByID`
/// inside the row's pane, so `name` is the title of the window **and** the name inside a
/// resident row (`gui/windowhuthome.xml:9`), and `citizen` is one id in two different
/// lists of the same window (`gui/windowassigncitizen.xml:11`, `:23`). Keeping the scope
/// is what lets a table have both without pretending one overrides the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: two scopes in BlockUI, the window and a row of one of its lists
pub enum Scope {
    /// The window's own panes.
    Window,
    /// A pane inside one row of the list with this pane id.
    Row(&'static str),
}

/// One pane of a screen.
///
/// `id` is the XML's `id` attribute, which is the only handle Java has on the pane:
/// `findPaneOfTypeByID("name", Text.class)` looks up the same string this carries
/// (`AbstractBuildingMainWindow.java:53`). A pane with no `id` cannot be written by a
/// class and therefore is not a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// The XML pane id.
    pub id: &'static str,
    /// The translation key the XML or the class puts next to this value, when there is
    /// one. A label Java draws in a **sibling pane with no id** is attached here to the
    /// value it heads, and the table's comment says so.
    pub label: Option<&'static str>,
    /// What the pane holds.
    pub value: Value,
    /// Where the value comes from.
    pub source: Source,
    /// Whether the pane is the window's or a row's.
    pub scope: Scope,
}

impl Field {
    /// A pane of the window itself, with no label of its own.
    pub const fn new(id: &'static str, value: Value, source: Source) -> Field {
        Field { id, label: None, value, source, scope: Scope::Window }
    }

    /// A pane of the window itself that the XML labels, or that Java labels when it
    /// writes the value.
    pub const fn labelled(
        id: &'static str,
        label: &'static str,
        value: Value,
        source: Source,
    ) -> Field {
        Field { id, label: Some(label), value, source, scope: Scope::Window }
    }

    /// A pane inside one row of the list `list` — the list is a `Value::Row` field of the
    /// same screen, and a row's own panes are looked up in the row.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
    ) -> Field {
        Field { id, label: None, value, source, scope: Scope::Row(list) }
    }

    /// Whether the pane belongs to a row rather than to the window.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }
}

/// The prefix of the delivery-priority label
/// (`TranslationConstants.java:476`, `TEXT_PICKUP_PRIORITY`).
pub const PRIORITY_PREFIX_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.buildprio";

/// The word for "never" (`TranslationConstants.java:478`).
pub const PRIORITY_NEVER_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.deliveryprio.never";

/// The highest priority a player may set — ten, not "up to ten"
/// (`AbstractDeliverymanRequestable.java:15`, `requests::MAX_BUILDING_PRIORITY`).
pub const PRIORITY_MAX: i32 = 10;

/// How the delivery-priority label reads: Java writes **two components**, the prefix
/// and then either the word for "never" or `n/10`
/// (`AbstractWindowWorkerModuleBuilding.java:97-111`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of Java's `if (prio == 0)`
pub enum PriorityText {
    /// Zero: the hut never asks to be picked up.
    Never,
    /// One to ten, out of ten.
    OfTen(i32),
}

/// The label a priority of `prio` renders to.
///
/// Java tests `prio == 0` and trusts the server to have clamped the value
/// (`AbstractBuildingContainer.java:124`); this port answers [`PriorityText::Never`]
/// for anything at or below zero, because a negative priority read back out of a
/// hand-edited compound would otherwise print as `-3/10`.
pub fn priority_text(prio: i32) -> PriorityText {
    if prio <= 0 {
        PriorityText::Never
    } else {
        PriorityText::OfTen(prio)
    }
}

impl PriorityText {
    /// The text of the `n/10` branch, `None` for [`PriorityText::Never`] — which is
    /// not an empty label but a different key ([`PRIORITY_NEVER_KEY`]).
    pub fn ratio(self) -> Option<String> {
        match self {
            PriorityText::Never => None,
            PriorityText::OfTen(prio) => Some(format!("{prio}/{PRIORITY_MAX}")),
        }
    }
}

/// One step of the delivery priority: Java adds `+1` or `-1` and clamps to `0..=10`
/// (`ChangeDeliveryPriorityMessage.java:65-75` →
/// `AbstractBuildingContainer.java:122-125`).
///
/// The clamp is the server's, not the window's: the window also refuses to move past
/// the ends (`AbstractWindowWorkerModuleBuilding.java:115`, `:125`) but still sends
/// the message, and the server is what actually holds the value.
pub fn alter_priority(prio: i32, step: i32) -> i32 {
    (prio + step).clamp(0, PRIORITY_MAX)
}

/// The percentage the builder's reserves tab prints for the player's contribution
/// (`WindowBuilderResModule.java:103-106`): `supplied / total`, floored, in percent.
///
/// Java guards the division with `if (total > 0)` and writes nothing otherwise; this
/// port answers `0` for "nothing to supply", which prints the same `0%` and cannot
/// divide by zero.
pub fn percent(supplied: i32, total: i32) -> i32 {
    if total <= 0 {
        return 0;
    }
    ((supplied as f64 / total as f64) * 100.0) as i32
}

/// A resource row's "available / needed" line (`WindowBuilderResModule.java:209`).
pub fn stock_text(available: i32, needed: i32) -> String {
    format!("{available} / {needed}")
}

/// The occupancy label's key: a translation with two arguments, the residents and the
/// places (`TranslationConstants.java:861`, used at `WindowHutLiving.java:108`).
pub const OCCUPANCY_KEY: &str = "com.minecolonies.coremod.gui.home.assigned";

/// The progress label's key (`WindowBuilderResModule.java:105`), two arguments: the
/// player's share and the building's own progress.
pub const PROGRESS_KEY: &str = "com.minecolonies.coremod.gui.progress.res";

/// The step label's key (`WindowBuilderResModule.java:147`), two arguments: the stage
/// reached and the number of stages.
pub const STEP_KEY: &str = "com.minecolonies.coremod.gui.progress.step";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_values_are_computed_and_eight_are_read() {
        let computed: Vec<&str> = Value::ALL
            .iter()
            .filter(|value| value.is_computed())
            .map(|value| value.name())
            .collect();
        assert_eq!(computed, ["progress", "step", "stock", "occupancy"]);
        assert!(!Value::Name.is_computed());
        assert!(!Value::Count.is_computed());
        assert_eq!(Value::ALL.len(), 12);
    }

    #[test]
    fn a_priority_of_zero_reads_never_and_the_rest_read_out_of_ten() {
        assert_eq!(priority_text(0), PriorityText::Never);
        assert_eq!(priority_text(0).ratio(), None);
        assert_eq!(priority_text(7).ratio().as_deref(), Some("7/10"));
        assert_eq!(priority_text(10).ratio().as_deref(), Some("10/10"));
        // A hand-edited compound can hold a negative; Java would print `-3/10`.
        assert_eq!(priority_text(-3), PriorityText::Never);
        assert_ne!(
            PRIORITY_NEVER_KEY, PRIORITY_PREFIX_KEY,
            "the prefix and the word for never are two keys, not one"
        );
    }

    #[test]
    fn the_priority_steps_stop_at_the_two_ends() {
        assert_eq!(alter_priority(0, -1), 0, "AbstractBuildingContainer.java:124");
        assert_eq!(alter_priority(10, 1), 10);
        assert_eq!(alter_priority(4, 1), 5);
        assert_eq!(alter_priority(4, -1), 3);
        // The window refuses to move past the ends, and the server clamps anyway.
        assert_eq!(priority_text(alter_priority(10, 1)).ratio().as_deref(), Some("10/10"));
    }

    #[test]
    fn the_builder_percentage_floors_and_never_divides_by_zero() {
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 66);
        assert_eq!(percent(3, 3), 100);
        assert_eq!(percent(0, 3), 0);
        assert_eq!(percent(5, 0), 0, "Java only writes the line when total > 0");
        assert_eq!(percent(0, 0), 0);
    }

    #[test]
    fn a_resource_row_reads_available_over_needed() {
        assert_eq!(stock_text(0, 64), "0 / 64");
        assert_eq!(stock_text(64, 64), "64 / 64");
        assert_eq!(
            stock_text(12, 4),
            "12 / 4",
            "a surplus is printed, not hidden: WindowBuilderResModule.java:209"
        );
    }

    #[test]
    fn a_field_defaults_to_no_label_and_the_window_scope() {
        let bare = Field::new("name", Value::Name, Source::Building);
        assert_eq!(bare.label, None);
        assert_eq!(bare.scope, Scope::Window);
        assert!(!bare.is_row_pane());
        let labelled = Field::labelled("workers", OCCUPANCY_KEY, Value::Row, Source::Citizen);
        assert_eq!(labelled.label, Some(OCCUPANCY_KEY));
        assert_eq!(labelled.id, "workers");
        assert_eq!(labelled.scope, Scope::Window);
        // The two are the same type, so a table can mix them in one slice.
        let both = [bare, labelled];
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn a_row_pane_carries_the_list_it_belongs_to() {
        let name = Field::in_row("assignedCitizen", "name", Value::Row, Source::Citizen);
        assert_eq!(name.id, "name");
        assert_eq!(name.scope, Scope::Row("assignedCitizen"));
        assert!(name.is_row_pane());
        assert_ne!(
            name.scope,
            Field::new("name", Value::Name, Source::Building).scope,
            "the title and a row's name are two panes that share an id"
        );
        // The same id in two lists is two scopes, not a collision.
        assert_ne!(
            Scope::Row("assigned"),
            Scope::Row("unassigned"),
            "WindowAssignCitizen.java names both lists' rows `citizen`"
        );
    }

    #[test]
    fn the_three_label_keys_are_three_different_keys() {
        assert_ne!(OCCUPANCY_KEY, PROGRESS_KEY);
        assert_ne!(PROGRESS_KEY, STEP_KEY);
        assert_ne!(OCCUPANCY_KEY, STEP_KEY);
        for key in [OCCUPANCY_KEY, PROGRESS_KEY, STEP_KEY, PRIORITY_PREFIX_KEY] {
            assert!(key.starts_with("com.minecolonies."), "{key} is not a lang key");
        }
    }

    #[test]
    fn a_source_says_whether_it_lives_in_the_colony() {
        assert!(Source::Citizen.is_colony_data());
        assert!(Source::Request.is_colony_data());
        assert!(Source::Colony.is_colony_data());
        assert!(!Source::Hut.is_colony_data());
        assert!(!Source::PlayerInventory.is_colony_data());
        assert!(!Source::Module("worker").is_colony_data());
    }
}
