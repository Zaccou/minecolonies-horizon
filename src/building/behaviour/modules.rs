//! The 157 module keys, which of the 54 kinds carries which, and which module
//! takes a worker.
//!
//! Java reference: `core/colony/buildings/modules/BuildingModules.java` — **157**
//! `ModuleProducer`s, each with a serialisation key that is the module's
//! identity (`api/colony/buildings/registry/BuildingEntry.java:204-220`) — and
//! `apiimp/initializer/ModBuildingsInitializer.java`, whose 54 entries attach
//! **279** modules in total.
//!
//! **Why a table and not `Building::attach_module`.** `src/building/instance.rs`
//! takes whatever module is handed to it: it knows a module has a key and a
//! level. It cannot know that a builder carries seven modules and a mystical
//! site none, because that is Java's initialiser, not a rule of the model. So the
//! table lives here, as data, and the counts are asserted rather than assumed.
//!
//! **Four kinds of module, and the difference is a permission.**
//!
//! | [`Role`] | Count | What it means |
//! | --- | ---: | --- |
//! | [`Role::Worker`] | 59 | takes a citizen **with a job** (`IAssignsCitizen` through a `WorkerBuildingModule` or a subclass) |
//! | [`Role::Resident`] | 2 | takes a citizen **without a job**: the residence and the tavern (`LivingBuildingModule.java:20`, `TavernLivingBuildingModule.java:12`) |
//! | [`Role::Data`] | 88 | settings, item lists, stats, a field list — holds no citizen |
//! | [`Role::View`] | 8 | a `ModuleProducer` with **no server module at all** (the factory is `null`): a window on the client |
//!
//! **The job side is not repeated here.** `crate::jobs::employ(hut, job)` already
//! carries the 58 rows a hut hires — the two skills, the rain rule and the head
//! count. This file adds the columns that table has no room for (the module key,
//! whether the worker sleeps at work) and **a test holds the two together**: for
//! every attachment whose role is a worker, `employ` must answer that hut with
//! that job, and name the same module.
//!
//! **Two facts the table shows that the job table cannot.**
//!
//! - `druid_tower_work` is declared and **attached to nobody**
//!   (`BuildingModules.java:573`, and no `.addBuildingModuleProducer(DRUID_TOWER_WORK)`
//!   in the initialiser). It is a guard tower that could hire a druid and does
//!   not; `crate::jobs::employment` already says the tower has four guard types
//!   and not five.
//! - two kinds carry **no module at all**: the mystical site and the stash. A
//!   town hall carries exactly one.
//!
//! **A capacity is not a module's own number.** `Role::Worker` defers to
//! [`crate::jobs::employ`] (`Capacity::at`), which is
//! `WorkerBuildingModule.getModuleMax` reading `sizeLimit`; [`Role::Resident`]
//! has no job and no `sizeLimit`, so it answers the building's level
//! (`LivingBuildingModule.java:100-105`, `getModuleMax` → `getBuildingLevel()`).

use crate::building::Kind;
use crate::jobs::{self, Job};

/// What a module does with citizens, which is the only thing about it another
/// module ever asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the port's own set, closed on purpose
pub enum Role {
    /// Takes a citizen that holds a job. `at_home` is whether the module
    /// descends from `WorkAtHomeBuildingModule` (`:23`), which is what makes a
    /// citizen *sleep* at this hut and is the first branch of Java's citizen
    /// census (`managers/CitizenManager.java:133-140`).
    Worker { job: Job, at_home: bool },
    /// Takes a citizen that holds no job: the residence and the tavern's rooms.
    Resident,
    /// Holds data, not people.
    Data,
    /// A producer with no server module: the factory in the producer is `null`.
    View,
}

/// One module producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Module {
    /// Java's serialisation key, which is also what a saved building stores
    /// (`AbstractBuilding.java:385-393`).
    pub key: &'static str,
    /// The Java class that produces it, short name only. It is documentation —
    /// nothing branches on it — and it is what a reader checks the port against.
    pub class: &'static str,
    pub role: Role,
}

impl Module {
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The job this module takes, or `None` for a module that takes none.
    pub fn job(&self) -> Option<Job> {
        match self.role {
            Role::Worker { job, .. } => Some(job),
            _ => None,
        }
    }

    /// Whether this module puts its citizen to bed at the hut
    /// (`WorkAtHomeBuildingModule`, `BuildingModules.java:550-585`).
    pub fn at_home(&self) -> bool {
        matches!(
            self.role,
            Role::Worker {
                at_home: true,
                ..
            }
        )
    }

    /// Whether this module holds a citizen at all — the question
    /// `AbstractAssignedCitizenModule.isFull` (`:106-109`) asks before a hire.
    pub fn holds_citizens(&self) -> bool {
        matches!(self.role, Role::Worker { .. } | Role::Resident)
    }
}

/// A module that takes a worker with this job, and whether the worker sleeps
/// here. A function rather than a macro so the table reads as data at the call
/// site: `worker(Job::Composter, false)`.
const fn worker(job: Job, at_home: bool) -> Role {
    Role::Worker { job, at_home }
}

/// A module that takes residents and no job.
const fn resident() -> Role {
    Role::Resident
}

/// A module with data in it and nobody.
const fn data() -> Role {
    Role::Data
}

/// A producer with no server module at all.
const fn view() -> Role {
    Role::View
}

/// The producer table, one row per `ModuleProducer`. One table expanded once, so
/// the catalogue cannot disagree with itself; a hand-written `match` could.
macro_rules! catalogue {
    ($($key:literal | $class:literal | $role:expr),* $(,)?) => {
        /// Every module producer Java declares, in `BuildingModules.java`'s order.
        pub const CATALOGUE: &[Module] = &[
            // The trailing comma after `$role` is not decoration: an `expr`
            // fragment may only be followed by `,`, `;` or `=>`, so `role: $role`
            // closed by a brace would not parse.
            $(Module { key: $key, class: $class, role: $role, }),*
        ];
    };
}

catalogue! {
    "min_stock" | "MinimumStockModule" | data(),
    "min_stock_postbox" | "MinimumStockModule" | data(),
    "bed" | "BedHandlingModule" | data(),
    "furnace" | "FurnaceUserModule" | data(),
    "craft_task_view" | "" | view(),
    "courier_task_view" | "" | view(),
    "craft_settings" | "SettingsModule" | data(),
    "guard_entity_list" | "EntityListModule" | data(),
    "stats_module" | "BuildingStatisticsModule" | data(),
    "barracks_stats_module" | "BarracksStatisticsModule" | data(),
    "itemlist_fuel" | "ItemListModule" | data(),
    "itemlist_compostable" | "ItemListModule" | data(),
    "restaurant_menu" | "RestaurantMenuModule" | data(),
    "netherminer_menu" | "RestaurantMenuModule" | data(),
    "itemlist_sapling" | "ItemListModule" | data(),
    "itemlist_ore" | "ItemListModule" | data(),
    "itemlist_flower" | "ItemListModule" | data(),
    "composter_work" | "WorkerBuildingModule" | worker(Job::Composter, false),
    "composter_settings" | "SettingsModule" | data(),
    "farmer_craft" | "CraftingWorkerBuildingModule" | worker(Job::Farmer, false),
    "farmer_work" | "BuildingFarmer.CraftingModule" | data(),
    "farmer_fields" | "BuildingFarmer.FarmerFieldsModule" | data(),
    "farmer_settings" | "SettingsModule" | data(),
    "plantation_work" | "CraftingWorkerBuildingModule" | worker(Job::Planter, false),
    "plantation_craft" | "BuildingPlantation.CraftingModule" | data(),
    "plantation_fields" | "BuildingPlantation.PlantationFieldsModule" | data(),
    "plantation_settings" | "SettingsModule" | data(),
    "florist_work" | "WorkerBuildingModule" | worker(Job::Florist, false),
    "florist_items" | "ItemListModule" | data(),
    "beekeeper_work" | "WorkerBuildingModule" | worker(Job::Beekeeper, false),
    "beekeeper_settings" | "SettingsModule" | data(),
    "beekeeper_tool" | "" | view(),
    "beekeeper_herding" | "BuildingBeekeeper.HerdingModule" | data(),
    "chickenherder_work" | "WorkerBuildingModule" | worker(Job::ChickenHerder, false),
    "chickenherder_herding" | "BuildingChickenHerder.HerdingModule" | data(),
    "chickenherder_breeding_settings" | "SettingsModule" | data(),
    "cowherder_work" | "WorkerBuildingModule" | worker(Job::Cowboy, false),
    "cowherder_settings" | "SettingsModule" | data(),
    "cowherder_herding" | "BuildingCowboy.HerdingModule" | data(),
    "stablemaster_work" | "WorkerBuildingModule" | worker(Job::Stablemaster, false),
    "stablemaster_herding" | "BuildingStable.HerdingModule" | data(),
    "stable_settings" | "SettingsModule" | data(),
    "fisher_work" | "WorkerBuildingModule" | worker(Job::Fisherman, false),
    "rabbitherder_work" | "WorkerBuildingModule" | worker(Job::RabbitHerder, false),
    "rabbitherder_settings" | "SettingsModule" | data(),
    "rabbitherder_herding" | "AnimalHerdingModule" | data(),
    "sheperd_work" | "WorkerBuildingModule" | worker(Job::Shepherd, false),
    "sheperd_settings" | "SettingsModule" | data(),
    "sheperd_herding" | "BuildingShepherd.HerdingModule" | data(),
    "swineherder_work" | "WorkerBuildingModule" | worker(Job::SwineHerder, false),
    "swineherder_settings" | "SettingsModule" | data(),
    "swineherder_herding" | "AnimalHerdingModule" | data(),
    "alchemist_work" | "CraftingWorkerBuildingModule" | worker(Job::Alchemist, false),
    "alchemist_craft" | "BuildingAlchemist.CraftingModule" | data(),
    "alchemist_brew" | "BuildingAlchemist.BrewingModule" | data(),
    "baker_craft" | "BuildingBaker.CraftingModule" | data(),
    "baker_smelt" | "BuildingBaker.SmeltingModule" | data(),
    "baker_work" | "CraftingWorkerBuildingModule" | worker(Job::Baker, false),
    "blacksmith_work" | "CraftingWorkerBuildingModule" | worker(Job::Blacksmith, false),
    "blacksmith_craft" | "BuildingBlacksmith.CraftingModule" | data(),
    "concretemixer_work" | "CraftingWorkerBuildingModule" | worker(Job::ConcreteMixer, false),
    "concretemixer_craft" | "BuildingConcreteMixer.CraftingModule" | data(),
    "crusher_work" | "CraftingWorkerBuildingModule" | worker(Job::Crusher, false),
    "crusher_craft" | "BuildingCrusher.CraftingModule" | data(),
    "crusher_settings" | "SettingsModule" | data(),
    "dyer_work" | "CraftingWorkerBuildingModule" | worker(Job::Dyer, false),
    "dyer_craft" | "BuildingDyer.CraftingModule" | data(),
    "dyer_smelt" | "BuildingDyer.SmeltingModule" | data(),
    "fletcher_work" | "CraftingWorkerBuildingModule" | worker(Job::Fletcher, false),
    "fletcher_craft" | "BuildingFletcher.CraftingModule" | data(),
    "fletcher_do_craft" | "BuildingFletcher.DOCraftingModule" | data(),
    "glassblower_work" | "CraftingWorkerBuildingModule" | worker(Job::Glassblower, false),
    "glassblower_craft" | "BuildingGlassblower.CraftingModule" | data(),
    "glassblower_smelting" | "BuildingGlassblower.SmeltingModule" | data(),
    "glassblower_do_craft" | "BuildingGlassblower.DOCraftingModule" | data(),
    "mechanic_work" | "CraftingWorkerBuildingModule" | worker(Job::Mechanic, false),
    "mechanic_craft" | "BuildingMechanic.CraftingModule" | data(),
    "mechanic_do_craft" | "BuildingMechanic.DOCraftingModule" | data(),
    "sawmill_work" | "CraftingWorkerBuildingModule" | worker(Job::Sawmill, false),
    "sawmill_craft" | "BuildingSawmill.CraftingModule" | data(),
    "sawmill_do_craft" | "BuildingSawmill.DOCraftingModule" | data(),
    "sifter_work" | "CraftingWorkerBuildingModule" | worker(Job::Sifter, false),
    "sifter_craft" | "BuildingSifter.CraftingModule" | data(),
    "smelter_work" | "CraftingWorkerBuildingModule" | worker(Job::Smelter, false),
    "smelter_smelting" | "BuildingSmeltery.SmeltingModule" | data(),
    "smelter_orebreak" | "BuildingSmeltery.OreBreakingModule" | data(),
    "smelter_settings" | "SettingsModule" | data(),
    "stonemason_work" | "CraftingWorkerBuildingModule" | worker(Job::StoneMason, false),
    "stonemason_craft" | "BuildingStonemason.CraftingModule" | data(),
    "stonemason_do_craft" | "BuildingStonemason.DOCraftingModule" | data(),
    "stonesmelter_work" | "CraftingWorkerBuildingModule" | worker(Job::StoneSmeltery, false),
    "stonesmelter_smelting" | "BuildingStoneSmeltery.SmeltingModule" | data(),
    "courier_work" | "DeliverymanAssignmentModule" | worker(Job::Deliveryman, false),
    "warehouse_couriers" | "CourierAssignmentModule" | worker(Job::Deliveryman, false),
    "warehouse_options" | "WarehouseModule" | data(),
    "warehouse_request_queue" | "WarehouseRequestQueueModule" | data(),
    "pupil_work" | "ChildrenBuildingModule" | worker(Job::Pupil, false),
    "student_work" | "WorkerBuildingModule" | worker(Job::Student, false),
    "teacher_work" | "WorkerBuildingModule" | worker(Job::Teacher, false),
    "university_work" | "WorkerBuildingModule" | worker(Job::Researcher, false),
    "university_research" | "" | view(),
    "builder_work" | "WorkerBuildingModule" | worker(Job::Builder, false),
    "builder_settings" | "SettingsModule" | data(),
    "builder_craft" | "SimpleCraftingModule" | data(),
    "workorderlist" | "" | view(),
    "buildingresources" | "BuildingResourcesModule" | data(),
    "cook_craft" | "NoPrivateCrafterWorkerModule" | worker(Job::Cook, false),
    "chef_work" | "CraftingWorkerBuildingModule" | worker(Job::Chef, false),
    "chef_craft" | "BuildingKitchen.CraftingModule" | data(),
    "chef_smelt" | "BuildingKitchen.SmeltingModule" | data(),
    "forester_craft" | "BuildingLumberjack.CraftingModule" | data(),
    "forester_work" | "LumberjackAssignmentModule" | worker(Job::Lumberjack, false),
    "forester_settings" | "SettingsModule" | data(),
    "forester_zone" | "" | view(),
    "healer_work" | "HospitalAssignmentModule" | worker(Job::Healer, false),
    "home" | "HomeBuildingModule" | data(),
    "living" | "LivingBuildingModule" | resident(),
    "miner_work" | "MinerBuildingModule" | worker(Job::Miner, false),
    "miner_craft" | "SimpleCraftingModule" | data(),
    "miner_levels" | "MinerLevelManagementModule" | data(),
    "miner_settings" | "SettingsModule" | data(),
    "miner_guard_assign" | "" | view(),
    "quarrier_work" | "MinerBuildingModule" | worker(Job::Quarrier, false),
    "simple_quarry" | "QuarryModule" | worker(Job::Quarrier, false),
    "medium_quarry" | "QuarryModule" | worker(Job::Quarrier, false),
    "tavern_living" | "TavernLivingBuildingModule" | resident(),
    "tavern_visitor" | "TavernBuildingModule" | data(),
    "tavern_settings" | "SettingsModule" | data(),
    "townhall_settings" | "SettingsModule" | data(),
    "archery_work" | "WorkAtHomeBuildingModule" | worker(Job::ArcherTraining, true),
    "knight_training" | "WorkAtHomeBuildingModule" | worker(Job::CombatTraining, true),
    "knight_barracks_work" | "GuardBuildingModule" | worker(Job::Knight, true),
    "ranger_barracks_work" | "GuardBuildingModule" | worker(Job::Ranger, true),
    "druid_barracks_work" | "GuardBuildingModule" | worker(Job::Druid, true),
    "marksman_barracks_work" | "GuardBuildingModule" | worker(Job::Marksman, true),
    "huscarl_barracks_work" | "GuardBuildingModule" | worker(Job::Huscarl, true),
    "cavalry_stable_work" | "StableCavalryBuildingModule" | worker(Job::Cavalry, true),
    "knight_tower_work" | "GuardBuildingModule" | worker(Job::Knight, true),
    "ranger_tower_work" | "GuardBuildingModule" | worker(Job::Ranger, true),
    "druid_tower_work" | "GuardBuildingModule" | worker(Job::Druid, true),
    "marksman_tower_work" | "GuardBuildingModule" | worker(Job::Marksman, true),
    "huscarl_tower_work" | "GuardBuildingModule" | worker(Job::Huscarl, true),
    "knight_gate_work" | "GuardBuildingModule" | worker(Job::Knight, true),
    "ranger_gate_work" | "GuardBuildingModule" | worker(Job::Ranger, true),
    "tool_scepterguard_view" | "" | view(),
    "guard_settings" | "SettingsModule" | data(),
    "gate_guard_settings" | "SettingsModule" | data(),
    "enchanter_work" | "CraftingWorkerBuildingModule" | worker(Job::Enchanter, false),
    "enchanter_craft" | "BuildingEnchanter.CraftingModule" | data(),
    "enchanter_stations" | "EnchanterStationsModule" | data(),
    "graveyard" | "GraveyardManagementModule" | data(),
    "graveyard_work" | "WorkerBuildingModule" | worker(Job::Undertaker, false),
    "netherworker_work" | "CraftingWorkerBuildingModule" | worker(Job::NetherWorker, false),
    "netherworker_craft" | "BuildingNetherWorker.CraftingModule" | data(),
    "netherworker_settings" | "SettingsModule" | data(),
    "netherworker_expedition" | "ExpeditionLogModule" | data(),
    "colony_connections" | "ColonyConnectionModule" | data(),
}

/// Which modules each kind carries, in the order the initialiser attaches them —
/// which is the order Java's `getFirstModuleOccurance` walks
/// (`AbstractBuilding.java:262-272`), and therefore the order that decides which
/// guard module's head count the citizen census reads. The rows are in `Kind::ALL`
/// order, which is also alphabetical by registry path.
macro_rules! attached {
    ($($kind:ident: &[$($key:literal),*],)*) => {
        /// One row per registered kind, in `Kind::ALL` order.
        pub const ATTACHED: &[(Kind, &[&str])] = &[
            $((Kind::$kind, &[$($key),*])),*
        ];
    };
}

attached! {
    Alchemist: &["alchemist_work", "alchemist_craft", "alchemist_brew", "craft_task_view", "stats_module"],
    Archery: &["archery_work", "bed", "min_stock", "stats_module"],
    Baker: &["baker_work", "baker_craft", "craft_task_view", "baker_smelt", "craft_settings", "furnace", "min_stock", "itemlist_fuel", "stats_module"],
    Barracks: &["min_stock", "barracks_stats_module"],
    BarracksTower: &["knight_barracks_work", "ranger_barracks_work", "druid_barracks_work", "huscarl_barracks_work", "marksman_barracks_work", "tool_scepterguard_view", "guard_entity_list", "guard_settings", "min_stock", "bed", "stats_module"],
    Beekeeper: &["beekeeper_work", "beekeeper_tool", "beekeeper_herding", "beekeeper_settings", "itemlist_flower", "min_stock", "stats_module"],
    Blacksmith: &["blacksmith_work", "blacksmith_craft", "craft_settings", "craft_task_view", "stats_module"],
    Builder: &["builder_work", "builder_craft", "buildingresources", "builder_settings", "workorderlist", "min_stock", "stats_module"],
    ChickenHerder: &["chickenherder_work", "chickenherder_herding", "chickenherder_breeding_settings", "min_stock", "stats_module"],
    CombatAcademy: &["knight_training", "min_stock", "bed", "stats_module"],
    Composter: &["composter_work", "composter_settings", "itemlist_compostable", "stats_module"],
    ConcreteMixer: &["concretemixer_work", "concretemixer_craft", "craft_settings", "craft_task_view", "stats_module"],
    Cook: &["cook_craft", "furnace", "itemlist_fuel", "restaurant_menu", "stats_module"],
    Cowboy: &["cowherder_work", "cowherder_herding", "cowherder_settings", "min_stock", "stats_module"],
    Crusher: &["crusher_work", "crusher_craft", "crusher_settings", "craft_task_view", "stats_module"],
    Deliveryman: &["courier_work", "courier_task_view", "stats_module"],
    Dyer: &["dyer_work", "dyer_craft", "dyer_smelt", "craft_settings", "furnace", "itemlist_fuel", "craft_task_view", "stats_module"],
    Enchanter: &["enchanter_work", "enchanter_craft", "enchanter_stations", "min_stock", "stats_module"],
    Farmer: &["farmer_work", "farmer_craft", "farmer_fields", "farmer_settings", "craft_task_view", "min_stock", "stats_module"],
    Fisherman: &["fisher_work", "min_stock", "stats_module"],
    Fletcher: &["fletcher_work", "fletcher_craft", "fletcher_do_craft", "craft_settings", "craft_task_view", "stats_module"],
    Florist: &["florist_work", "florist_items", "min_stock", "stats_module"],
    GateHouse: &["knight_gate_work", "ranger_gate_work", "guard_entity_list", "gate_guard_settings", "min_stock", "bed", "stats_module", "colony_connections"],
    Glassblower: &["glassblower_work", "glassblower_craft", "glassblower_do_craft", "glassblower_smelting", "craft_settings", "furnace", "itemlist_fuel", "craft_task_view", "stats_module"],
    Graveyard: &["graveyard_work", "graveyard", "min_stock", "stats_module"],
    GuardTower: &["knight_tower_work", "ranger_tower_work", "marksman_tower_work", "huscarl_tower_work", "tool_scepterguard_view", "guard_entity_list", "guard_settings", "min_stock", "bed", "stats_module"],
    Hospital: &["healer_work", "min_stock", "stats_module"],
    Kitchen: &["min_stock", "craft_task_view", "chef_work", "chef_craft", "chef_smelt", "furnace", "itemlist_fuel", "stats_module"],
    Library: &["student_work", "min_stock", "stats_module"],
    Lumberjack: &["forester_work", "forester_craft", "forester_settings", "forester_zone", "itemlist_sapling", "craft_task_view", "min_stock", "stats_module"],
    Mechanic: &["mechanic_work", "mechanic_craft", "mechanic_do_craft", "craft_settings", "craft_task_view", "stats_module"],
    MediumQuarry: &["medium_quarry"],
    Miner: &["miner_work", "quarrier_work", "miner_craft", "miner_levels", "miner_settings", "miner_guard_assign", "buildingresources", "min_stock", "stats_module"],
    MysticalSite: &[],
    NetherWorker: &["netherworker_work", "netherworker_craft", "netherworker_expedition", "netherworker_settings", "netherminer_menu", "craft_task_view", "min_stock", "stats_module"],
    Plantation: &["plantation_work", "plantation_craft", "plantation_fields", "plantation_settings", "craft_task_view", "stats_module"],
    Postbox: &["min_stock_postbox"],
    RabbitHutch: &["rabbitherder_work", "rabbitherder_herding", "rabbitherder_settings", "min_stock", "stats_module"],
    Residence: &["home", "living", "bed"],
    Sawmill: &["sawmill_work", "sawmill_craft", "sawmill_do_craft", "craft_settings", "craft_task_view", "stats_module"],
    School: &["teacher_work", "pupil_work", "min_stock", "stats_module"],
    Shepherd: &["sheperd_work", "sheperd_herding", "sheperd_settings", "min_stock", "stats_module"],
    Sifter: &["sifter_work", "sifter_craft", "min_stock", "stats_module"],
    SimpleQuarry: &["simple_quarry"],
    Smeltery: &["smelter_work", "smelter_smelting", "smelter_orebreak", "furnace", "itemlist_fuel", "itemlist_ore", "min_stock", "smelter_settings", "stats_module"],
    Stable: &["cavalry_stable_work", "bed", "stablemaster_work", "stablemaster_herding", "guard_entity_list", "stable_settings", "min_stock", "stats_module"],
    Stash: &[],
    StoneMason: &["stonemason_work", "stonemason_craft", "stonemason_do_craft", "craft_settings", "craft_task_view", "stats_module"],
    StoneSmeltery: &["stonesmelter_work", "stonesmelter_smelting", "craft_settings", "furnace", "itemlist_fuel", "craft_task_view", "stats_module"],
    SwineHerder: &["swineherder_work", "swineherder_herding", "swineherder_settings", "min_stock", "stats_module"],
    Tavern: &["tavern_living", "tavern_visitor", "bed", "stats_module", "tavern_settings"],
    TownHall: &["townhall_settings"],
    University: &["university_work", "university_research", "stats_module"],
    Warehouse: &["warehouse_couriers", "warehouse_options", "min_stock", "warehouse_request_queue"],
}

/// The catalogue, read as a table rather than as a slice.
#[derive(Clone, Copy, Debug, Default)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Catalogue;

impl Catalogue {
    /// Every module key, in Java's declaration order.
    pub fn keys() -> impl Iterator<Item = &'static str> {
        CATALOGUE.iter().map(|module| module.key)
    }

    /// How many modules a kind carries.
    pub fn count(kind: Kind) -> usize {
        modules_of(kind).len()
    }

    /// Every module of a kind that holds a citizen, in attachment order.
    pub fn holders(kind: Kind) -> impl Iterator<Item = &'static Module> {
        modules_of(kind)
            .iter()
            .filter_map(|key| module(key))
            .filter(|module| module.holds_citizens())
    }

    /// One module of a kind, by key.
    pub fn of_kind(kind: Kind, key: &str) -> Option<&'static Module> {
        if !has(kind, key) {
            return None;
        }
        module(key)
    }
}

/// The module a key names, or `None` for a key no producer declares.
pub fn module(key: &str) -> Option<&'static Module> {
    CATALOGUE.iter().find(|module| module.key == key)
}

/// The keys one kind carries, in the initialiser's order.
pub fn modules_of(kind: Kind) -> &'static [&'static str] {
    ATTACHED
        .iter()
        .find(|(known, _)| *known == kind)
        .map(|(_, keys)| *keys)
        .unwrap_or(&[])
}

/// Whether a kind carries this module. Both halves must exist: a key that no
/// producer declares is never carried, even if a row names it.
pub fn has(kind: Kind, key: &str) -> bool {
    module(key).is_some() && modules_of(kind).contains(&key)
}

/// Which kinds carry a module, in `Kind`'s order. The reverse of
/// [`modules_of`], and the question `BlockHutTavern.canPlaceAt` asks when it walks
/// the colony looking for a `tavern_visitor` module
/// (`BlockHutTavern.java:52-56`).
pub fn attached_to(key: &str) -> Vec<Kind> {
    ATTACHED
        .iter()
        .filter(|(_, keys)| keys.contains(&key))
        .map(|(kind, _)| *kind)
        .collect()
}

/// How many citizens one module of one kind holds at a level, or `None` when the
/// kind does not carry that module.
///
/// Three answers, and each comes from somewhere else in Java:
///
/// - a **worker** module asks `crate::jobs::employ`, which is
///   `WorkerBuildingModule.getModuleMax` reading the `sizeLimit` lambda it was
///   built with (`WorkerBuildingModule.java:60`, `:236-239`);
/// - a **resident** module answers the building's level
///   (`LivingBuildingModule.java:100-105`) — the same number for the residence
///   and for the tavern's rooms, because the tavern's module is a subclass;
/// - a module that holds nobody answers `0`, which is not "unknown": a settings
///   module has no capacity, and a caller that confused that with "one seat"
///   would hire into a furnace.
pub fn capacity_at(kind: Kind, key: &str, level: i32) -> Option<i32> {
    let module = Catalogue::of_kind(kind, key)?;
    // A hut nobody built holds nobody, whatever its lambda would answer: `isFull`
    // is only ever asked after `canAssignCitizens`
    // (`api/colony/buildings/IBuilding.java:464-467`), and a caller that asked
    // earlier would otherwise be told a level-0 builder has a seat.
    if level <= 0 {
        return Some(0);
    }
    Some(match module.role {
        Role::Worker { job, .. } => jobs::employ(kind, job).ok()?.capacity_at(level),
        Role::Resident => level,
        Role::Data | Role::View => 0,
    })
}

/// The seats a kind offers at a level, summed over the modules that hold
/// citizens. A school has two holding modules and therefore two seats at level 1;
/// a residence has one and holds one.
pub fn seats(kind: Kind, level: i32) -> i32 {
    Catalogue::holders(kind)
        .filter_map(|module| capacity_at(kind, module.key, level))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::EMPLOYMENTS;

    #[test]
    fn the_catalogue_holds_the_hundred_and_fifty_seven_producers() {
        assert_eq!(CATALOGUE.len(), 157, "BuildingModules.java declares 157");
        let mut keys: Vec<&str> = CATALOGUE.iter().map(|module| module.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 157, "two producers share a key, so a save cannot tell them apart");
        for module in CATALOGUE {
            assert!(!module.key.is_empty());
            assert!(
                module.key.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{}: the key is a serialisation name, lower case only",
                module.key
            );
        }
    }

    #[test]
    fn the_roles_add_up_and_no_module_is_two_things() {
        let count = |role: fn(&Role) -> bool| CATALOGUE.iter().filter(|m| role(&m.role)).count();
        assert_eq!(
            count(|role| matches!(role, Role::Worker { .. })),
            59,
            "58 attached worker modules plus the druid tower's, which no building lists"
        );
        assert_eq!(count(|role| matches!(role, Role::Resident)), 2, "living, tavern_living");
        assert_eq!(count(|role| matches!(role, Role::View)), 8, "producers whose factory is null");
        assert_eq!(count(|role| matches!(role, Role::Data)), 88);
        assert_eq!(59 + 2 + 8 + 88, 157);
        // The eight view producers have no class; every other one names its own.
        for module in CATALOGUE {
            assert_eq!(
                module.class.is_empty(),
                matches!(module.role, Role::View),
                "{}: a view producer has no server module and every other one has a class",
                module.key
            );
        }
    }

    #[test]
    fn the_attachment_table_has_one_row_per_kind_and_two_hundred_and_seventy_nine_modules() {
        assert_eq!(ATTACHED.len(), 54, "54 registered kinds");
        let mut kinds: Vec<Kind> = ATTACHED.iter().map(|(kind, _)| *kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), 54, "a kind has two rows, so one of them is dead");
        assert_eq!(
            ATTACHED.iter().map(|(_, keys)| keys.len()).sum::<usize>(),
            279,
            "ModBuildingsInitializer.java attaches 279 modules in 54 entries"
        );
        // Every kind of the registry is in the table, and every named key exists.
        for kind in Kind::ALL {
            assert!(
                ATTACHED.iter().any(|(known, _)| known == kind),
                "{kind} has no row"
            );
        }
        for (kind, keys) in ATTACHED {
            for key in *keys {
                assert!(module(key).is_some(), "{kind} carries {key}, which no producer declares");
            }
        }
    }

    #[test]
    fn exactly_one_module_is_declared_and_attached_to_nobody() {
        let orphans: Vec<&str> = Catalogue::keys()
            .filter(|key| attached_to(key).is_empty())
            .collect();
        assert_eq!(
            orphans,
            vec!["druid_tower_work"],
            "a guard tower could hire a druid and its entry never lists the module"
        );
        // And the reverse: no row names a key twice.
        for (kind, keys) in ATTACHED {
            let mut sorted: Vec<&str> = keys.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), keys.len(), "{kind} carries a module twice");
        }
    }

    #[test]
    fn two_kinds_carry_nothing_and_the_town_hall_carries_one() {
        let empty: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| modules_of(*kind).is_empty())
            .collect();
        assert_eq!(empty, vec![Kind::MysticalSite, Kind::Stash]);
        assert_eq!(modules_of(Kind::TownHall), &["townhall_settings"]);
        assert_eq!(Catalogue::count(Kind::BarracksTower), 11, "five guard types and six others");
        assert_eq!(Catalogue::count(Kind::Barracks), 2, "a dormitory with no worker module");
    }

    #[test]
    fn every_attached_worker_module_agrees_with_the_job_table() {
        // The two tables were written from the same Java file; this is the check
        // that they cannot drift. `crate::jobs::employ` must answer the hut with
        // that job, and the row it finds must name this module.
        let mut checked = 0;
        for (kind, keys) in ATTACHED {
            for key in *keys {
                let module = module(key).expect("every attached key exists");
                let Some(job) = module.job() else {
                    continue;
                };
                let row = jobs::employ(*kind, job).unwrap_or_else(|_| {
                    panic!("{kind} carries {key} for {job:?} and the job table says it hires nobody")
                });
                assert_eq!(
                    row.module, *key,
                    "{kind} hires {job:?} through {} and not through {key}",
                    row.module
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 58, "one attachment per employment row");
    }

    #[test]
    fn the_job_table_has_no_row_a_kind_does_not_carry() {
        for row in EMPLOYMENTS {
            assert!(
                has(row.hut, row.module),
                "{} hires {:?} through {} and does not carry it",
                row.hut,
                row.job,
                row.module
            );
        }
    }

    #[test]
    fn forty_seven_kinds_hire_somebody_and_seven_hire_nobody() {
        let hiring: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| Catalogue::holders(*kind).any(|module| module.job().is_some()))
            .collect();
        assert_eq!(hiring.len(), 47);
        let idle: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| !hiring.contains(kind))
            .collect();
        assert_eq!(
            idle,
            vec![
                Kind::Barracks,
                Kind::MysticalSite,
                Kind::Postbox,
                Kind::Residence,
                Kind::Stash,
                Kind::Tavern,
                Kind::TownHall,
            ],
            "a dormitory, a shrine, a postbox, a house, a stash, a tavern and a hall"
        );
    }

    #[test]
    fn the_residence_and_the_tavern_hold_residents_and_nobody_else_does() {
        let residents: Vec<(Kind, &str)> = ATTACHED
            .iter()
            .flat_map(|(kind, keys)| keys.iter().map(move |key| (*kind, *key)))
            .filter(|(_, key)| matches!(module(key).map(|m| m.role), Some(Role::Resident)))
            .collect();
        assert_eq!(
            residents,
            vec![(Kind::Residence, "living"), (Kind::Tavern, "tavern_living")]
        );
        assert_eq!(capacity_at(Kind::Residence, "living", 4), Some(4));
        assert_eq!(capacity_at(Kind::Residence, "home", 4), Some(0), "home is a stat, not a seat");
        assert_eq!(capacity_at(Kind::Builder, "living", 4), None, "no living module");
    }

    #[test]
    fn a_capacity_comes_from_the_job_table_and_a_module_with_nobody_in_it_holds_none() {
        // One head at any level.
        assert_eq!(capacity_at(Kind::Builder, "builder_work", 0), Some(0));
        assert_eq!(capacity_at(Kind::Builder, "builder_work", 1), Some(1));
        assert_eq!(capacity_at(Kind::Builder, "builder_work", 5), Some(1));
        // Two heads at any level: the gatehouse's two guard modules.
        assert_eq!(capacity_at(Kind::GateHouse, "knight_gate_work", 3), Some(2));
        // One per level: a guard tower's knight.
        assert_eq!(capacity_at(Kind::GuardTower, "knight_tower_work", 3), Some(3));
        // Two per level: the school's pupils, and a warehouse's couriers.
        assert_eq!(capacity_at(Kind::School, "pupil_work", 3), Some(6));
        assert_eq!(capacity_at(Kind::Warehouse, "warehouse_couriers", 2), Some(4));
        // The university hires one researcher per level (`WorkerBuildingModule`
        // with `ICommonBuilding::getBuildingLevel`).
        assert_eq!(capacity_at(Kind::University, "university_work", 4), Some(4));
        // A module that holds nobody holds zero — if the kind carries it at all.
        assert_eq!(capacity_at(Kind::Smeltery, "furnace", 5), Some(0));
        assert_eq!(
            capacity_at(Kind::Smeltery, "craft_task_view", 5),
            None,
            "the smeltery registers nine modules and no CRAFT_TASK_VIEW \
             (ModBuildingsInitializer.java:332-346): its crafting windows are \
             smelter_smelting and smelter_orebreak"
        );
        // A kind that does register the task view answers zero, not `None`
        // (ModBuildingsInitializer.java:182-192, the crusher).
        assert_eq!(capacity_at(Kind::Crusher, "craft_task_view", 5), Some(0));
        assert_eq!(capacity_at(Kind::Builder, "furnace", 5), None);
        assert_eq!(capacity_at(Kind::Builder, "no_such_module", 5), None);
        // Level 0 is a hut nobody built: every seat is closed.
        assert_eq!(capacity_at(Kind::Residence, "living", 0), Some(0));
        assert_eq!(capacity_at(Kind::Builder, "builder_work", 0), Some(0));
    }

    #[test]
    fn a_school_offers_three_seats_at_level_one_and_a_residence_one() {
        assert_eq!(seats(Kind::School, 1), 3, "one teacher and two pupils");
        assert_eq!(seats(Kind::School, 3), 7, "1 teacher + 2 per level of pupils");
        assert_eq!(seats(Kind::Residence, 3), 3);
        assert_eq!(seats(Kind::Residence, 0), 0, "a hut nobody built holds nobody");
        assert_eq!(seats(Kind::MysticalSite, 5), 0, "no module at all");
        assert_eq!(seats(Kind::Barracks, 5), 0, "a dormitory with no worker module");
        assert_eq!(seats(Kind::Miner, 2), 2, "a miner and a quarrier, one head each");
        assert_eq!(seats(Kind::BarracksTower, 3), 15, "five guard types, three heads each");
    }

    #[test]
    fn the_at_home_modules_are_the_ones_a_hut_with_a_bed_carries() {
        // Java's census takes the `bed` + WorkAtHome branch first
        // (CitizenManager.java:133-140); this is the set it can fire on.
        let at_home: Vec<Kind> = ATTACHED
            .iter()
            .filter(|(_, keys)| keys.contains(&"bed"))
            .filter(|(kind, _)| Catalogue::holders(*kind).any(|module| module.at_home()))
            .map(|(kind, _)| *kind)
            .collect();
        assert_eq!(
            at_home,
            vec![
                Kind::Archery,
                Kind::BarracksTower,
                Kind::CombatAcademy,
                Kind::GateHouse,
                Kind::GuardTower,
                Kind::Stable,
            ],
            "the six huts a guard or a squire sleeps in"
        );
        // The residence and the tavern have a bed and no worker at home: the
        // census takes their `living` branch instead.
        assert!(has(Kind::Residence, "bed"));
        assert!(!Catalogue::holders(Kind::Residence).any(|module| module.at_home()));
        assert_eq!(
            Catalogue::holders(Kind::Stable)
                .next()
                .map(|module| module.key),
            Some("cavalry_stable_work"),
            "the first holder is the cavalry, which is the module Java's census reads"
        );
    }

    #[test]
    fn a_key_no_producer_declares_is_never_carried() {
        assert!(has(Kind::Builder, "builder_work"));
        assert!(!has(Kind::Builder, "maker_work"));
        assert!(!has(Kind::Builder, ""), "an empty key cannot be a module");
        assert!(has(Kind::TownHall, "townhall_settings"));
        assert!(!has(Kind::TownHall, "min_stock"), "a town hall carries no stock list");
        assert!(has(Kind::BarracksTower, "knight_barracks_work"));
        assert!(!has(Kind::GuardTower, "druid_tower_work"), "declared, never attached");
        assert_eq!(module("no_such_module"), None);
        assert_eq!(module("builder_work").map(|module| module.job()), Some(Some(Job::Builder)));
        assert_eq!(module("builder_craft").map(|module| module.job()), Some(None));
    }

    #[test]
    fn the_reverse_lookup_names_every_kind_carrying_a_module() {
        assert_eq!(
            attached_to("bed"),
            vec![
                Kind::Archery,
                Kind::BarracksTower,
                Kind::CombatAcademy,
                Kind::GateHouse,
                Kind::GuardTower,
                Kind::Residence,
                Kind::Stable,
                Kind::Tavern,
            ]
        );
        assert_eq!(attached_to("tavern_visitor"), vec![Kind::Tavern], "one tavern per colony");
        assert_eq!(attached_to("townhall_settings"), vec![Kind::TownHall]);
        assert_eq!(
            attached_to("knight_tower_work"),
            vec![Kind::GuardTower],
            "the barracks tower has its own knight module, not this one"
        );
        assert!(attached_to("no_such_module").is_empty());
    }

    #[test]
    fn a_module_who_takes_a_job_says_which_one_and_where_it_sleeps() {
        assert_eq!(
            module("knight_gate_work").map(|module| module.job()),
            Some(Some(Job::Knight))
        );
        assert!(module("knight_gate_work").expect("a producer").at_home());
        assert!(!module("builder_work").expect("a producer").at_home(), "a builder goes home");
        assert!(module("living").expect("a producer").holds_citizens());
        assert!(module("tavern_living").expect("a producer").holds_citizens());
        assert!(!module("stats_module").expect("a producer").holds_citizens());
        assert!(!module("craft_task_view").expect("a producer").holds_citizens());
        assert_eq!(
            module("cook_craft").map(|module| module.class),
            Some("NoPrivateCrafterWorkerModule"),
            "a cook is hired but has no private crafting list"
        );
    }
}
