//! The thirteen screens a hut opens, as data: title, fields, buttons, gate.
//!
//! Java reference: `core/client/gui/` — every screen here names the class that builds it
//! and the BlockUI XML that lays it out (`src/main/resources/assets/minecolonies/gui/`).
//! **The XML is not reproduced**: its `pos`/`size`/`source`/`color` attributes are
//! drawing, and `TRANCHE.md` §3 M2 already says this slice has no slot layout either.
//! What is reproduced is what a class **asks for** (`onOpened`, `onUpdate`) and what its
//! buttons **do** (`registerButton`, `AbstractWindowSkeleton.java:130`).
//!
//! **Nine files, not one per métier.** `core/client/gui/` holds exactly **nine**
//! `WindowHut*.java` files, and they are generic: the worker hut is one class
//! (`WindowHutWorkerModulePlaceholder`), used by every hut that carries a worker module
//! (`AbstractBuildingView.java:392-399`), so **forty-three hut kinds share one screen**.
//! What changes from métier to métier is the module's data — its job, its two skills, its
//! head count — which [`crate::jobs::Employment`] already carries, not the window. The
//! four remaining screens this table adds are the windows Java puts outside that set
//! (`WindowBarracksBuilding`, `WindowBuilderResModule`, `WindowHireWorker`,
//! `WindowAssignCitizen`), and [`crate::ui::table`] says which window a kind gets.
//!
//! **A screen has one gate, and only one screen has one.** The builder hut's first open
//! is the guide (`WindowHutBuilderModule.java:52-66`): if the player has not done the
//! advancement, the hut window closes itself and the guide opens instead.
//!
//! **One declaration per pane, one per button.** Every `Field` and every `Action` is a
//! named constant below and the screens are slices **composed** out of them, so the five
//! buttons a building window inherits are written once and listed in four screens.

use super::action::{Action, Effect, Refusal};
use super::field::{Field, OCCUPANCY_KEY, Scope, Source, Value};

/// The label of the worker list, drawn in a sibling pane with no id
/// (`layouthuts/layouthutpageactions.xml:6-7`).
pub const WORKER_ASSIGNED_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.workerassigned";
/// The label of the hire button — "manage", not "hire": it also fires
/// (`layouthuts/layouthutpageactions.xml:17-19`).
pub const MANAGE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.manage";
/// The recall label (`layouthuts/layouthutpageactions.xml:20-22`).
pub const RECALL_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.recall";
/// The forced-pickup label (`layouthuts/layouthutpageactions.xml:31-33`).
pub const FORCE_PICKUP_KEY: &str = "entity.deliveryman.forcepickup";
/// The home's assign label (`gui/windowhuthome.xml:14-16`).
pub const HOME_MANAGE_KEY: &str = "com.minecolonies.coremod.gui.home.manage";
/// The home's recall label — the town hall's key, reused (`gui/windowhuthome.xml:17-19`).
pub const HOME_RECALL_KEY: &str = "com.minecolonies.coremod.gui.townhall.recall";
/// The barracks' spies label (`gui/windowhutbarracks.xml:16-18`).
pub const SPIES_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.spies";
/// The container button (`layouthuts/layouthutpageactionsmin.xml:5-7`).
pub const INVENTORY_KEY: &str = "container.inventory";
/// The tooltip of the chest, the all-inventory window
/// (`layouthuts/layouthutpageactionsmin.xml:8-10`).
pub const ALL_INVENTORY_TIP_KEY: &str = "com.minecolonies.core.building.inv_sum";
/// The tooltip of the wax seal (`layouthuts/layouthutpageactionsminwoinv.xml:16-17`).
pub const INFO_TIP_KEY: &str = "com.minecolonies.core.building.help";
/// The tooltip of the pencil (`layouthuts/layouthutpageactionsminwoinv.xml:10-11`).
pub const EDIT_NAME_TIP_KEY: &str = "com.minecolonies.gui.edit";
/// The build button's idle label (`TranslationConstants.java:470`).
pub const BUILD_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.buildrepair";

/// The screen ids, as constants so the tables and every `Effect::Opens` agree.
pub const HUT_WORKER_ID: &str = "hut.worker";
pub const HUT_BUILDER_ID: &str = "hut.builder";
pub const HUT_MIN_ID: &str = "hut.min";
pub const HUT_LIVING_ID: &str = "hut.living";
pub const HUT_BARRACKS_ID: &str = "hut.barracks";
pub const HIRE_ID: &str = "hire";
pub const ASSIGN_ID: &str = "assign";
pub const HUT_NAME_ID: &str = "hut.name";
pub const HUT_INVENTORY_ID: &str = "hut.inventory";
pub const HUT_GUIDE_ID: &str = "hut.guide";
pub const TAB_RESOURCES_ID: &str = "tab.resources";
pub const TAB_MINER_LEVELS_ID: &str = "tab.miner_levels";
pub const TAB_TASKS_ID: &str = "tab.tasks";

/// What a screen's title reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two shapes, and a third would be a new variant
pub enum Title {
    /// The building's own name, then its level, into **one** pane
    /// (`AbstractBuildingMainWindow.java:171-176`).
    BuildingName,
    /// A fixed label, for a window that is not a building's.
    Fixed(&'static str),
}

/// A condition the screen checks **before** it can be used at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one gate in the whole table, and it is an advancement
pub enum Gate {
    /// An advancement's progress, `minecolonies:minecolonies/check_out_guide`
    /// (`WindowHutBuilderModule.java:23`, `:55-63`).
    Advancement(&'static str),
}

/// One screen: what Java builds, from which XML, with which fields and buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    /// This table's name for the screen. It is **not** Java's: Java has no such id and
    /// looks a window up from a building or a module view.
    pub id: &'static str,
    /// The Java class that builds it.
    pub window: &'static str,
    /// The BlockUI window or layout, under `assets/minecolonies/`.
    pub resource: &'static str,
    /// What the title reads.
    pub title: Title,
    /// The panes the class fills.
    pub fields: &'static [Field],
    /// The buttons the class registers.
    pub actions: &'static [Action],
    /// A condition on the whole screen, when there is one.
    pub gate: Option<Gate>,
}

impl Screen {
    /// The field with this pane id, copied out; the **first** one, since the title is
    /// two writes on one pane.
    pub fn field(&self, id: &str) -> Option<Field> {
        self.fields.iter().find(|field| field.id == id).copied()
    }

    /// The action with this button id, copied out.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions.iter().find(|action| action.id == id).copied()
    }

    /// Whether the screen shows at least one field of this kind.
    pub fn shows(&self, value: Value) -> bool {
        self.fields.iter().any(|field| field.value == value)
    }

    /// Every value kind this screen shows, in the order the panes declare them, without
    /// repeating one.
    pub fn values(&self) -> Vec<Value> {
        let mut seen = Vec::new();
        for field in self.fields {
            if !seen.contains(&field.value) {
                seen.push(field.value);
            }
        }
        seen
    }
}

// ---------------------------------------------------------------------------
// The fields, declared once.
// ---------------------------------------------------------------------------

/// The title: the building's name (`AbstractBuildingMainWindow.java:173`).
const F_NAME: Field = Field::new("name", Value::Name, Source::Building);
/// The title again, appended into the same pane (`AbstractBuildingMainWindow.java:174-175`).
const F_LEVEL: Field = Field::new("name", Value::Level, Source::Building);
/// The worker list (`layouthuts/layouthutpageactions.xml:9`).
const F_WORKERS: Field = Field::labelled("workers", WORKER_ASSIGNED_KEY, Value::Row, Source::Module("worker"));
/// The name inside one worker row (`layouthuts/layouthutpageactions.xml:12`).
const F_WORKER_NAME: Field = Field::in_row("workers", "workerName", Value::Name, Source::Citizen);
/// The delivery priority, prefix and value in one pane
/// (`AbstractWindowWorkerModuleBuilding.java:97-111`).
const F_PRIORITY: Field = Field::new("prioValue", Value::Priority, Source::Building);
/// The residents line of a home (`gui/windowhuthome.xml:4`).
const F_OCCUPANCY: Field = Field::labelled("assignedlabel", OCCUPANCY_KEY, Value::Occupancy, Source::Module("living"));
/// The resident list of a home (`gui/windowhuthome.xml:6-12`).
const F_RESIDENTS: Field = Field::new("assignedCitizen", Value::Row, Source::Module("living"));
/// The name inside one resident row (`gui/windowhuthome.xml:9`) — the same id as the
/// title, in the row's own scope.
const F_RESIDENT_NAME: Field = Field::in_row("assignedCitizen", "name", Value::Name, Source::Citizen);
/// The raid's current direction (`gui/windowhutbarracks.xml:7`).
const F_RAID_CURRENT: Field = Field::new("current", Value::Text, Source::Colony);
/// The barbarian spawn points (`gui/windowhutbarracks.xml:10`).
const F_SPAWN_POINTS: Field = Field::new("positions", Value::Row, Source::Colony);
/// One spawn point, as "distance and direction"
/// (`WindowBarracksBuilding.java:152-172`).
const F_SPAWN_POINT: Field = Field::in_row("positions", "pos", Value::Text, Source::Colony);

/// The title pair, in Java's write order.
const TITLE_FIELDS: &[Field] = &[F_NAME, F_LEVEL];

/// A worker hut's panes: the title, the list, the priority
/// (`AbstractWindowWorkerModuleBuilding.java:162-203`).
const WORKER_FIELDS: &[Field] = &[F_NAME, F_LEVEL, F_WORKERS, F_WORKER_NAME, F_PRIORITY];

// ---------------------------------------------------------------------------
// The buttons, declared once.
// ---------------------------------------------------------------------------

/// The rename button (`layouthuts/layouthutpageactionsminwoinv.xml:10-11`).
const A_EDIT_NAME: Action = Action::plain("editName", EDIT_NAME_TIP_KEY, Effect::Opens(HUT_NAME_ID));
/// The build button. Its label and its effect both depend on the building's state:
/// [`super::action::build_label`] and [`super::action::BuildLabel::effect`].
const A_BUILD: Action = Action::plain("build", BUILD_KEY, Effect::OpensJava("WindowBuildBuilding"));
/// The hint page, visible only when the building has hint text
/// (`AbstractBuildingMainWindow.java:55-59`).
const A_INFO: Action = Action::plain("info", INFO_TIP_KEY, Effect::OpensJava("WindowInfo"));
/// The hut's own container.
const A_INVENTORY: Action = Action::plain("inventory", INVENTORY_KEY, Effect::Container);
/// Every stack of every container of the hut.
const A_ALL_INVENTORY: Action = Action::plain("allinventory", ALL_INVENTORY_TIP_KEY, Effect::Opens(HUT_INVENTORY_ID));
/// Hire or fire, level permitting (`AbstractWindowWorkerModuleBuilding.java:143-152`).
const A_HIRE: Action = Action::guarded("hire", MANAGE_KEY, Effect::Opens(HIRE_ID), &[Refusal::NotBuilt]);
/// Send the worker home (`AbstractWindowWorkerModuleBuilding.java:157-160`).
const A_RECALL: Action = Action::guarded("recall", RECALL_KEY, Effect::Sends("RecallCitizenMessage"), &[Refusal::RecallFailed]);
/// One step down the delivery priority (`AbstractWindowWorkerModuleBuilding.java:123-131`).
const A_PRIO_DOWN: Action = Action::plain("deliveryPrioDown", "-", Effect::Sends("ChangeDeliveryPriorityMessage"));
/// One step up (`AbstractWindowWorkerModuleBuilding.java:113-121`).
const A_PRIO_UP: Action = Action::plain("deliveryPrioUp", "+", Effect::Sends("ChangeDeliveryPriorityMessage"));
/// Ask for a pickup now (`AbstractWindowWorkerModuleBuilding.java:133-136`).
const A_FORCE_PICKUP: Action = Action::guarded("forcePickup", FORCE_PICKUP_KEY, Effect::Sends("ForcePickupMessage"), &[Refusal::PickupFailed]);
/// Assign residents to a home (`WindowHutLiving.java:115-124`).
const A_ASSIGN: Action = Action::guarded("assign", HOME_MANAGE_KEY, Effect::Opens(ASSIGN_ID), &[Refusal::NotBuilt, Refusal::HousingHandledElsewhere]);
/// Send every resident home (`WindowHutLiving.java:71-74`).
const A_RECALL_HOME: Action = Action::guarded("recall", HOME_RECALL_KEY, Effect::Sends("RecallCitizenHutMessage"), &[Refusal::RecallFailed]);
/// Hire spies, level permitting (`WindowBarracksBuilding.java:92-107`).
const A_SPIES: Action = Action::guarded("hireSpies", SPIES_KEY, Effect::OpensJava("WindowsBarracksSpies"), &[Refusal::SpiesBelowLevel]);

/// The five buttons every building window inherits from
/// `AbstractBuildingMainWindow.java:47-51`.
const MAIN_ACTIONS: &[Action] = &[A_EDIT_NAME, A_BUILD, A_INFO, A_INVENTORY, A_ALL_INVENTORY];

/// A worker hut's ten buttons: the five inherited, then the five of
/// `AbstractWindowWorkerModuleBuilding.java:90-94`.
const HUT_ACTIONS: &[Action] = &[
    A_EDIT_NAME, A_BUILD, A_INFO, A_INVENTORY, A_ALL_INVENTORY, A_HIRE, A_RECALL, A_PRIO_DOWN, A_PRIO_UP,
    A_FORCE_PICKUP,
];

/// A home's seven buttons: the five inherited, then assign and recall.
const LIVING_ACTIONS: &[Action] = &[
    A_EDIT_NAME, A_BUILD, A_INFO, A_INVENTORY, A_ALL_INVENTORY, A_ASSIGN, A_RECALL_HOME,
];

/// The barracks' six buttons: the five inherited, then the spies.
const BARRACKS_ACTIONS: &[Action] = &[
    A_EDIT_NAME, A_BUILD, A_INFO, A_INVENTORY, A_ALL_INVENTORY, A_SPIES,
];

// ---------------------------------------------------------------------------
// The thirteen screens.
// ---------------------------------------------------------------------------

/// A hut with a worker module: **one screen for forty-three kinds**.
static HUT_WORKER: Screen = Screen {
    id: HUT_WORKER_ID,
    window: "WindowHutWorkerModulePlaceholder",
    resource: "minecolonies:gui/windowhutworkerplaceholder.xml",
    title: Title::BuildingName,
    fields: WORKER_FIELDS,
    actions: HUT_ACTIONS,
    gate: None,
};

/// The builder hut: the worker screen, plus the guide on its first open
/// (`WindowHutBuilderModule.java:46-66`).
///
/// Its fields and buttons are **the same slices**, not copies: the class extends
/// `AbstractWindowWorkerModuleBuilding` and adds nothing but the gate.
static HUT_BUILDER: Screen = Screen {
    id: HUT_BUILDER_ID,
    window: "WindowHutBuilderModule",
    resource: "minecolonies:gui/windowhutworkerplaceholder.xml",
    title: Title::BuildingName,
    fields: WORKER_FIELDS,
    actions: HUT_ACTIONS,
    gate: Some(Gate::Advancement("minecolonies:minecolonies/check_out_guide")),
};

/// A home or the tavern: residents, not workers (`WindowHutLiving.java:76-124`).
static HUT_LIVING: Screen = Screen {
    id: HUT_LIVING_ID,
    window: "WindowHutLiving",
    resource: "minecolonies:gui/windowhuthome.xml",
    title: Title::BuildingName,
    fields: &[F_NAME, F_LEVEL, F_OCCUPANCY, F_RESIDENTS, F_RESIDENT_NAME],
    actions: LIVING_ACTIONS,
    gate: None,
};

/// The barracks: the barbarian spawn points and the spies
/// (`WindowBarracksBuilding.java:84-145`).
static HUT_BARRACKS: Screen = Screen {
    id: HUT_BARRACKS_ID,
    window: "WindowBarracksBuilding",
    resource: "minecolonies:gui/windowhutbarracks.xml",
    title: Title::BuildingName,
    fields: &[F_NAME, F_LEVEL, F_RAID_CURRENT, F_SPAWN_POINTS, F_SPAWN_POINT],
    actions: BARRACKS_ACTIONS,
    gate: None,
};

/// The minimum window: what every building shows and nothing else
/// (`WindowHutMinPlaceholder.java:19-22`).
///
/// Used by the **five** kinds that carry no worker module view: the warehouse (its
/// courier module is not one, `BuildingWareHouse.java:219`), the two quarries
/// (`QuarryModule.java:28`), the stash and the mystical site — Java's default branch
/// (`AbstractBuildingView.java:395-397`).
static HUT_MIN: Screen = Screen {
    id: HUT_MIN_ID,
    window: "WindowHutMinPlaceholder",
    resource: "minecolonies:gui/layouthuts/layouthutpageactionsmin.xml",
    title: Title::BuildingName,
    fields: TITLE_FIELDS,
    actions: MAIN_ACTIONS,
    gate: None,
};

/// The hire window: one row per assignable citizen (`WindowHireWorker.java:382-508`).
static HIRE: Screen = Screen {
    id: HIRE_ID,
    window: "WindowHireWorker",
    resource: "minecolonies:gui/windowhireworker.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.hiring.description"),
    fields: &[
        Field::new("jobLabel", Value::Text, Source::Module("worker")),
        Field::new("unemployed", Value::Row, Source::Colony),
        Field::in_row("unemployed", "citizen", Value::Name, Source::Citizen),
        Field::in_row("unemployed", "distance", Value::Text, Source::Citizen),
        Field::in_row("unemployed", "attributes", Value::Text, Source::Citizen),
        Field::new("buildingHireMode", Value::Text, Source::Module("worker")),
        Field::new("showEmployedLabel", Value::Text, Source::Module("worker")),
    ],
    actions: &[
        Action::guarded("done", "com.minecolonies.coremod.gui.hiring.buttonhire", Effect::Sends("HireFireMessage"), &[Refusal::NoRoom, Refusal::CannotAssign]),
        Action::guarded("fire", "com.minecolonies.coremod.gui.hiring.buttonfire", Effect::Sends("HireFireMessage"), &[Refusal::Travelling]),
        // `pause` starts with no label: WindowHireWorker.java:437 writes it per row.
        Action::plain("pause", "", Effect::Sends("PauseCitizenMessage")),
        Action::plain("restart", "com.minecolonies.coremod.gui.hiring.restart", Effect::Sends("RestartCitizenMessage")),
        Action::plain("mode", MANAGE_KEY, Effect::Sends("BuildingHiringModeMessage")),
        Action::plain("showEmployed", "gui.no", Effect::Toggles),
        Action::guarded("cancel", "", Effect::Reopens, &[Refusal::NoAssignableModule]),
    ],
    gate: None,
};

/// The assignment window of a home: two lists, one button each
/// (`WindowAssignCitizen.java:219-400`).
static ASSIGN: Screen = Screen {
    id: ASSIGN_ID,
    window: "WindowAssignCitizen",
    resource: "minecolonies:gui/windowassigncitizen.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.assigning.description"),
    fields: &[
        Field::new("assigned", Value::Row, Source::Module("living")),
        Field::new("unassigned", Value::Row, Source::Colony),
        Field::in_row("assigned", "citizen", Value::Name, Source::Citizen),
        Field::in_row("assigned", "job", Value::Text, Source::Citizen),
        Field::in_row("unassigned", "citizen", Value::Name, Source::Citizen),
        Field::in_row("unassigned", "job", Value::Text, Source::Citizen),
        Field::new("buildingassignmentmode", Value::Text, Source::Module("living")),
    ],
    actions: &[
        Action::guarded("hire", "com.minecolonies.coremod.gui.hiring.buttonassign", Effect::Sends("AssignUnassignMessage"), &[Refusal::NoRoom, Refusal::HousingHandledElsewhere]),
        Action::guarded("fire", "com.minecolonies.coremod.gui.hiring.buttonunassign", Effect::Sends("AssignUnassignMessage"), &[Refusal::Travelling, Refusal::HousingHandledElsewhere]),
        Action::plain("mode", "", Effect::Sends("BuildingHiringModeMessage")),
        Action::plain("cancel", "", Effect::Reopens),
    ],
    gate: None,
};

/// The rename window (`WindowHutNameEntry.java:46-76`).
///
/// The field takes **25** characters and the rule keeps **15**
/// ([`super::action::MAX_NAME_LENGTH`], [`super::action::NAME_FIELD_MAX_LENGTH`]); the
/// input's limit is the XML's, not the class's, which is why this screen is the one place
/// the two numbers disagree.
static HUT_NAME: Screen = Screen {
    id: HUT_NAME_ID,
    window: "WindowHutNameEntry",
    resource: "minecolonies:gui/windowhutnameentry.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.building.rename.title"),
    fields: &[Field::labelled(
        "name",
        "com.minecolonies.coremod.gui.building.rename.title",
        Value::Input,
        Source::Building,
    )],
    actions: &[
        Action::plain("done", "gui.done", Effect::Renames),
        Action::plain("cancel", "gui.cancel", Effect::Reopens),
    ],
    gate: None,
};

/// Every stack a hut holds, with a filter, five sorts and a locator
/// (`WindowHutAllInventory.java:88-151`).
static HUT_INVENTORY: Screen = Screen {
    id: HUT_INVENTORY_ID,
    window: "WindowHutAllInventory",
    resource: "minecolonies:gui/windowhutallinventory.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.allinventory"),
    fields: &[
        Field::labelled("desc", "com.minecolonies.coremod.gui.allinventory", Value::Text, Source::Hut),
        Field::new("names", Value::Input, Source::Hut),
        Field::new("allinventorylist", Value::Row, Source::Hut),
        Field::in_row("allinventorylist", "resourceIcon", Value::Icon, Source::Hut),
        // Java's spelling, `ressourceStackName`, is the XML's: WindowConstants has no
        // constant for it and `windowhutallinventory.xml:14` spells it with two s.
        Field::in_row("allinventorylist", "ressourceStackName", Value::Name, Source::Hut),
        Field::in_row("allinventorylist", "quantities", Value::Count, Source::Hut),
        Field::in_row("allinventorylist", "locate", Value::Icon, Source::Hut),
    ],
    actions: &[
        Action::plain("sortStorageFilter", "", Effect::Cycles),
        Action::plain("locate", "?", Effect::Highlights),
        Action::plain("back", "com.minecolonies.coremod.gui.back", Effect::Reopens),
    ],
    gate: None,
};

/// The guide a builder hut opens before its own window
/// (`WindowHutGuide.java:28-41`).
static HUT_GUIDE: Screen = Screen {
    id: HUT_GUIDE_ID,
    window: "WindowHutGuide",
    resource: "minecolonies:gui/windowhutguide.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.workerhuts.guide.questionmark"),
    fields: &[
        Field::new("questionmark", Value::Text, Source::Advancement),
        Field::new("arrow", Value::Text, Source::Advancement),
        Field::new("chest", Value::Text, Source::Advancement),
        Field::new("guide1", Value::Icon, Source::Advancement),
    ],
    actions: &[
        Action::plain("confirm", "com.minecolonies.coremod.gui.workerhuts.guide.confirm", Effect::Opens(HUT_BUILDER_ID)),
        Action::plain("close", "", Effect::Opens(HUT_BUILDER_ID)),
    ],
    gate: None,
};

/// The builder's reserves tab: what the build still needs, and how much of it is here
/// (`WindowBuilderResModule.java:111-148`).
static TAB_RESOURCES: Screen = Screen {
    id: TAB_RESOURCES_ID,
    window: "WindowBuilderResModule",
    resource: "minecolonies:gui/layouthuts/layoutbuilderres.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.workerhuts.resourcelist"),
    fields: &[
        Field::new("desc", Value::Text, Source::Module("buildingresources")),
        Field::new("constructionName", Value::Text, Source::WorkOrder),
        Field::new("stepprogress", Value::Step, Source::Module("buildingresources")),
        Field::new("progress", Value::Progress, Source::Module("buildingresources")),
        Field::new("resources", Value::Row, Source::Module("buildingresources")),
        Field::in_row("resources", "resourceIcon", Value::Icon, Source::Module("buildingresources")),
        Field::in_row("resources", "resourceName", Value::Name, Source::Module("buildingresources")),
        Field::in_row("resources", "resourceMissing", Value::Count, Source::PlayerInventory),
        Field::in_row("resources", "resourceAvailableNeeded", Value::Stock, Source::Module("buildingresources")),
        Field::in_row("resources", "resourceId", Value::Count, Source::Module("buildingresources")),
        Field::in_row("resources", "resourceQuantity", Value::Count, Source::Module("buildingresources")),
    ],
    actions: &[Action::guarded(
        "resourceAdd",
        "↥",
        Effect::Sends("TransferItemsRequestMessage"),
        &[Refusal::ResourceMissing, Refusal::ResourceNotNeeded],
    )],
    gate: None,
};

/// The miner's levels tab: depth, node count, and two buttons per level
/// (`WindowHutMinerModule.java:80-111`).
static TAB_MINER_LEVELS: Screen = Screen {
    id: TAB_MINER_LEVELS_ID,
    window: "WindowHutMinerModule",
    resource: "minecolonies:gui/layouthuts/layoutminermodule.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.workerhuts.minernodelist"),
    fields: &[
        Field::new("levels", Value::Row, Source::Module("miner_levels")),
        Field::in_row("levels", "level", Value::Count, Source::Module("miner_levels")),
        Field::in_row("levels", "depth", Value::Count, Source::Module("miner_levels")),
        Field::in_row("levels", "nodes", Value::Count, Source::Module("miner_levels")),
    ],
    actions: &[
        Action::guarded(
            "repair",
            "com.minecolonies.coremod.gui.workerhuts.minerrepairnode",
            Effect::Sends("MinerRepairLevelMessage"),
            &[Refusal::WorkOrderExists],
        ),
        Action::guarded("mine", "", Effect::Sends("MinerSetLevelMessage"), &[Refusal::CurrentLevel]),
    ],
    gate: None,
};

/// A crafter's task list: read-only (`WindowHutRequestTaskModule.java:46-119`).
///
/// **No action at all**, and that is the screen: the tasks are the worker's, not the
/// player's, so the window registers no button of its own.
static TAB_TASKS: Screen = Screen {
    id: TAB_TASKS_ID,
    window: "WindowHutRequestTaskModule",
    resource: "minecolonies:gui/layouthuts/layouttasklist.xml",
    title: Title::Fixed("com.minecolonies.coremod.gui.workerhuts.crafter.tasks"),
    fields: &[
        Field::new("tasks", Value::Row, Source::Request),
        Field::in_row("tasks", "deliveryImage", Value::Icon, Source::Request),
        Field::in_row("tasks", "shortDetail", Value::Text, Source::Request),
        Field::in_row("tasks", "detailIcon", Value::Icon, Source::Request),
        Field::in_row("tasks", "priority", Value::Priority, Source::Request),
        Field::in_row("tasks", "requester", Value::Name, Source::Request),
    ],
    actions: &[],
    gate: None,
};

/// Every screen this slice describes: **thirteen**.
pub const SCREENS: &[&Screen] = &[
    &HUT_WORKER,
    &HUT_BUILDER,
    &HUT_MIN,
    &HUT_LIVING,
    &HUT_BARRACKS,
    &HIRE,
    &ASSIGN,
    &HUT_NAME,
    &HUT_INVENTORY,
    &HUT_GUIDE,
    &TAB_RESOURCES,
    &TAB_MINER_LEVELS,
    &TAB_TASKS,
];

/// The **nine** `WindowHut*.java` files under `core/client/gui/`, each with the screen
/// this table gives it.
///
/// Counted, not assumed: `find core/client/gui -name 'WindowHut*.java'` answers nine —
/// seven in `gui/` and `gui/huts/`, two in `gui/modules/building/`. **There is no
/// `AbstractWindowBuilding.java`** either: the base class is `AbstractBuildingWindow`
/// (`AbstractBuildingWindow.java:25`), and the main-window one is
/// `AbstractBuildingMainWindow`.
pub const HUT_WINDOWS: &[(&str, &str)] = &[
    ("WindowHutWorkerModulePlaceholder", HUT_WORKER_ID),
    ("WindowHutBuilderModule", HUT_BUILDER_ID),
    ("WindowHutMinPlaceholder", HUT_MIN_ID),
    ("WindowHutLiving", HUT_LIVING_ID),
    ("WindowHutNameEntry", HUT_NAME_ID),
    ("WindowHutAllInventory", HUT_INVENTORY_ID),
    ("WindowHutGuide", HUT_GUIDE_ID),
    ("WindowHutMinerModule", TAB_MINER_LEVELS_ID),
    ("WindowHutRequestTaskModule", TAB_TASKS_ID),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: &str) -> &'static Screen {
        SCREENS
            .iter()
            .copied()
            .find(|screen| screen.id == id)
            .unwrap_or_else(|| panic!("no screen {id}"))
    }

    #[test]
    fn the_title_is_two_writes_on_one_pane() {
        let worker = screen(HUT_WORKER_ID);
        assert_eq!(worker.fields[0].id, "name");
        assert_eq!(worker.fields[0].value, Value::Name);
        assert_eq!(worker.fields[1].id, "name");
        assert_eq!(worker.fields[1].value, Value::Level);
        assert_eq!(
            worker.field("name").map(|field| field.value),
            Some(Value::Name),
            "a lookup answers the first write, and it is the name"
        );
        assert_eq!(worker.title, Title::BuildingName);
        // The level is not a separate pane: AbstractBuildingMainWindow.java:174-175
        // appends it to the same component.
        assert_eq!(worker.fields.iter().filter(|field| field.id == "name").count(), 2);
    }

    #[test]
    fn a_pane_id_is_unique_inside_its_scope_and_only_the_title_repeats() {
        for screen in SCREENS {
            for (index, field) in screen.fields.iter().enumerate() {
                let repeated = screen.fields[..index]
                    .iter()
                    .any(|other| other.id == field.id && other.scope == field.scope);
                if repeated {
                    assert_eq!(field.scope, Scope::Window, "{} ", screen.id);
                    assert_eq!(field.id, "name");
                    assert_eq!(
                        field.value,
                        Value::Level,
                        "{} repeats a pane for something other than the level",
                        screen.id
                    );
                }
                // A row's pane names a list of the same screen.
                if let Scope::Row(list) = field.scope {
                    assert!(
                        screen
                            .fields
                            .iter()
                            .any(|other| other.id == list && other.value == Value::Row),
                        "{}: {} is a row of {list}, which is not a list of this screen",
                        screen.id,
                        field.id
                    );
                }
            }
        }
        // The one id that two lists of the same window share, and the reason it can:
        // the scope separates them (`gui/windowassigncitizen.xml:11`, `:23`).
        let assign = screen(ASSIGN_ID);
        let citizens: Vec<Scope> = assign
            .fields
            .iter()
            .filter(|field| field.id == "citizen")
            .map(|field| field.scope)
            .collect();
        assert_eq!(citizens, [Scope::Row("assigned"), Scope::Row("unassigned")]);
    }

    #[test]
    fn the_worker_screen_shows_a_name_a_level_a_worker_and_a_priority_and_no_progress() {
        let worker = screen(HUT_WORKER_ID);
        assert!(worker.shows(Value::Name));
        assert!(worker.shows(Value::Level));
        assert!(worker.shows(Value::Priority));
        // The finding: no progress bar and no stock count on a worker hut.
        assert!(
            !worker.shows(Value::Progress),
            "WindowHutWorkerModulePlaceholder has no progress pane"
        );
        assert!(!worker.shows(Value::Stock));
        assert!(!worker.shows(Value::Occupancy));
        assert_eq!(
            worker.field("workerName").map(|field| field.source),
            Some(Source::Citizen)
        );
        assert_eq!(
            worker.field("prioValue").map(|field| field.source),
            Some(Source::Building)
        );
    }

    #[test]
    fn the_reserves_tab_is_the_only_screen_that_shows_progress_and_stock() {
        let with_progress: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| screen.shows(Value::Progress))
            .map(|screen| screen.id)
            .collect();
        let with_stock: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| screen.shows(Value::Stock))
            .map(|screen| screen.id)
            .collect();
        assert_eq!(with_progress, [TAB_RESOURCES_ID]);
        assert_eq!(with_stock, [TAB_RESOURCES_ID]);
        // Both are computed out of two numbers, so a caller must supply both.
        assert!(Value::Progress.is_computed());
        assert!(Value::Stock.is_computed());
        // The step count is on the same tab, from the module's two stage counters.
        assert!(screen(TAB_RESOURCES_ID).shows(Value::Step));
    }

    #[test]
    fn the_living_screen_counts_residents_and_the_worker_screen_does_not() {
        let living = screen(HUT_LIVING_ID);
        let occupancy = living
            .field("assignedlabel")
            .unwrap_or_else(|| panic!("no occupancy pane"));
        assert_eq!(occupancy.value, Value::Occupancy);
        assert_eq!(occupancy.label, Some(OCCUPANCY_KEY));
        assert_eq!(
            living.action("assign").map(|action| action.effect),
            Some(Effect::Opens(ASSIGN_ID))
        );
        assert!(living.action("assign").is_some_and(|action| action.is_guarded()));
        assert!(!screen(HUT_WORKER_ID).shows(Value::Occupancy));
        // A home has no worker list, and a worker hut has no resident list.
        assert!(!living.shows(Value::Priority));
        assert_eq!(living.action("hire"), None);
    }

    #[test]
    fn every_screen_names_its_java_class_and_its_resource() {
        assert_eq!(SCREENS.len(), 13);
        for (index, screen) in SCREENS.iter().enumerate() {
            assert!(!screen.window.is_empty(), "{} has no class", screen.id);
            assert!(
                screen.resource.starts_with("minecolonies:gui/"),
                "{} reads {}",
                screen.id,
                screen.resource
            );
            assert!(screen.resource.ends_with(".xml"));
            assert!(!screen.fields.is_empty(), "{} shows nothing", screen.id);
            for other in &SCREENS[index + 1..] {
                assert_ne!(screen.id, other.id, "two screens share an id");
                assert_ne!(
                    screen.window, other.window,
                    "{} and {} name the same class",
                    screen.id, other.id
                );
            }
        }
        // The builder and the worker hut share **one** XML and are still two screens,
        // because they are two classes: one gates on an advancement, the other does not.
        assert_eq!(screen(HUT_BUILDER_ID).resource, screen(HUT_WORKER_ID).resource);
        assert_ne!(screen(HUT_BUILDER_ID).window, screen(HUT_WORKER_ID).window);
    }

    #[test]
    fn the_builder_hut_is_gated_by_an_advancement_and_nothing_else_is() {
        let gated: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| screen.gate.is_some())
            .map(|screen| screen.id)
            .collect();
        assert_eq!(gated, [HUT_BUILDER_ID]);
        assert_eq!(
            screen(HUT_BUILDER_ID).gate,
            Some(Gate::Advancement("minecolonies:minecolonies/check_out_guide"))
        );
        // The guide's two buttons both land on the builder hut, which is the gate's
        // other half: WindowHutGuide.java:37-41 reopens it with the guide disabled.
        let guide = screen(HUT_GUIDE_ID);
        assert_eq!(
            guide.action("confirm").map(|action| action.effect),
            Some(Effect::Opens(HUT_BUILDER_ID))
        );
        assert_eq!(
            guide.action("close").map(|action| action.effect),
            Some(Effect::Opens(HUT_BUILDER_ID))
        );
        assert_eq!(screen(HUT_WORKER_ID).gate, None);
    }

    #[test]
    fn nine_window_hut_files_and_every_one_of_them_is_a_screen_of_this_table() {
        assert_eq!(HUT_WINDOWS.len(), 9, "find -name 'WindowHut*.java' answers nine");
        for (window, id) in HUT_WINDOWS {
            assert!(window.starts_with("WindowHut"), "{window} is not a WindowHut*");
            assert_eq!(screen(id).window, *window, "{id} names the wrong class");
        }        // The four screens that are not a `WindowHut*` file, named one by one.
        let others: Vec<&str> = SCREENS
            .iter()
            .filter(|screen| !screen.window.starts_with("WindowHut"))
            .map(|screen| screen.window)
            .collect();
        assert_eq!(
            others,
            [
                "WindowBarracksBuilding",
                "WindowHireWorker",
                "WindowAssignCitizen",
                "WindowBuilderResModule"
            ]
        );
        // The one screen with no button at all is the task list, and that is the point.
        assert_eq!(screen(TAB_TASKS_ID).actions.len(), 0);
    }
}
