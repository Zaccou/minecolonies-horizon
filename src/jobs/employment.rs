//! Which hut employs which job, with the two skills it levels, its rain rule and
//! its head count.
//!
//! Java reference: `core/colony/buildings/modules/BuildingModules.java` — 56
//! `ModuleProducer`s under a `WorkerBuildingModule`-shaped constructor, one of
//! them orphaned, plus the three assignment modules that hire without declaring
//! skills: **58** rows — and the initialiser that gives each building its modules
//! (`apiimp/initializer/ModBuildingsInitializer.java`, 54 entries).
//!
//! **A job is not married to one hut, and the table says so.** The brief assumed
//! one hut per job; Java has jobs that work in several:
//!
//! | Job | Huts | Where |
//! | --- | --- | --- |
//! | `knight`, `ranger` | 3 — barracks tower, gatehouse, guard tower | `ModBuildingsInitializer.java:70-74`, `:233-236`, `:681-682` |
//! | `huscarl`, `marksman` | 2 — barracks tower, guard tower | `:74`, `:236` |
//! | `quarrier` | 3 — mine, simple quarry, medium quarry | `BuildingModules.java:508-514` |
//! | `deliveryman` | 2 — courier's hut, warehouse | `:390-392`, `CourierAssignmentModule.java:104` |
//!
//! The **other** direction is total: a hut always employs the same jobs, because
//! a module names one `JobEntry` and nothing else. **47 of the 54 building kinds
//! employ somebody**; the seven that do not ([`Refusal::BuildingEmploysNobody`])
//! are the barracks, the residence, the town hall, the postbox, the stash, the
//! tavern and the mystical site — a dormitory, a hall, a postbox, a stash, a
//! tavern, a shrine and a house, none with a worker module in its entry.
//!
//! **Three columns are the module's, not the job's.** `primary`/`secondary` are
//! the two skills whose XP the worker earns
//! (`core/colony/buildings/modules/WorkerBuildingModule.java:44-45`, consumed at
//! `core/entity/citizen/citizenhandlers/CitizenExperienceHandler.java:99-124`),
//! `rain` is `canWorkingDuringRain` (`WorkerBuildingModule.java:55`), and the
//! head count is `sizeLimit` (`:60`). That is why they hang off [`Employment`] and
//! not off [`Job`]: a knight at a gatehouse is two heads, the same knight in a
//! tower is one per level.
//!
//! **Three modules are not `WorkerBuildingModule`s**, so they state no skills and
//! no rain rule: the two quarry modules
//! (`core/colony/buildings/modules/QuarryModule.java:28`) and the warehouse's
//! courier module (`CourierAssignmentModule.java:25`). The table writes `-` in
//! their worker column.

use crate::building::Kind;

use super::job::Job;

/// The eleven citizen skills, and the two each skill drags with it.
///
/// Java reference: `api/entity/citizen/Skill.java:8-163`. The relation is mutual,
/// not transitive: `Athletics` raises `Strength` and `Strength` raises `Athletics`,
/// while `Athletics` lowers `Dexterity` and `Dexterity` lowers `Athletics`. Kept as
/// two functions because a caller applies them differently — one gains a tenth of
/// the XP, the other loses it (`CitizenExperienceHandler.java:103-124`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: eleven skills in Java, and a twelfth would need its two relations
pub enum Skill {
    Athletics,
    Dexterity,
    Strength,
    Agility,
    Stamina,
    Mana,
    Adaptability,
    Focus,
    Creativity,
    Knowledge,
    Intelligence,
}

impl Skill {
    /// Every skill, in Java's declaration order.
    pub const ALL: [Skill; 11] = [
        Skill::Athletics,
        Skill::Dexterity,
        Skill::Strength,
        Skill::Agility,
        Skill::Stamina,
        Skill::Mana,
        Skill::Adaptability,
        Skill::Focus,
        Skill::Creativity,
        Skill::Knowledge,
        Skill::Intelligence,
    ];

    /// The skill that gains `xp / (100 / 10)` whenever this one does, or `None`.
    ///
    /// `PRIMARY_DEPENDENCY_SHARE = 10` and `SECONDARY_DEPENDENCY_SHARE = 5`
    /// (`CitizenExperienceHandler.java:31`, `:36`): a tenth of the XP goes to the
    /// primary's complimentary skill, a twentieth to the secondary's.
    pub fn complimentary(self) -> Option<Skill> {
        match self {
            Skill::Athletics => Some(Skill::Strength),
            Skill::Dexterity => Some(Skill::Agility),
            Skill::Strength => Some(Skill::Athletics),
            Skill::Agility => Some(Skill::Dexterity),
            Skill::Stamina => Some(Skill::Knowledge),
            Skill::Mana => Some(Skill::Focus),
            Skill::Adaptability => Some(Skill::Creativity),
            Skill::Focus => Some(Skill::Mana),
            Skill::Creativity => Some(Skill::Adaptability),
            Skill::Knowledge => Some(Skill::Stamina),
            Skill::Intelligence => None,
        }
    }

    /// The skill that **loses** the same XP, or `None` (`Skill.java:18-22` and its
    /// ten siblings).
    pub fn adverse(self) -> Option<Skill> {
        match self {
            Skill::Athletics => Some(Skill::Dexterity),
            Skill::Dexterity => Some(Skill::Athletics),
            Skill::Strength => Some(Skill::Agility),
            Skill::Agility => Some(Skill::Strength),
            Skill::Stamina => Some(Skill::Mana),
            Skill::Mana => Some(Skill::Stamina),
            Skill::Adaptability => Some(Skill::Focus),
            Skill::Focus => Some(Skill::Adaptability),
            Skill::Creativity => Some(Skill::Knowledge),
            Skill::Knowledge => Some(Skill::Creativity),
            Skill::Intelligence => None,
        }
    }

    /// The Java constant's name, for a log line or a test: Java spells them in
    /// `PascalCase` (`Skill.java:10`, `:24`, …) and so does this port.
    pub fn name(self) -> &'static str {
        match self {
            Skill::Athletics => "Athletics",
            Skill::Dexterity => "Dexterity",
            Skill::Strength => "Strength",
            Skill::Agility => "Agility",
            Skill::Stamina => "Stamina",
            Skill::Mana => "Mana",
            Skill::Adaptability => "Adaptability",
            Skill::Focus => "Focus",
            Skill::Creativity => "Creativity",
            Skill::Knowledge => "Knowledge",
            Skill::Intelligence => "Intelligence",
        }
    }
}

/// The two skills a worker module levels, and whether it may work in the rain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Worker {
    pub primary: Skill,
    pub secondary: Skill,
    pub rain: bool,
}

/// How many citizens one module holds.
///
/// Java writes it as a lambda over the building (`WorkerBuildingModule.java:60`,
/// `:67-79`), and the data uses exactly four shapes: `(b) -> 1`, `(b) -> 2`,
/// `ICommonBuilding::getBuildingLevel`, and `(b) -> 2 * b.getBuildingLevel()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four shapes Java's data uses
pub enum Capacity {
    /// One head, at any level.
    One,
    /// Two heads, at any level — a gatehouse (`BuildingModules.java:580-585`).
    Two,
    /// One head per building level: guards, the university, the two training
    /// grounds (`:421`, `:491-494`, `:538-547`).
    PerLevel,
    /// Two heads per building level: the school (`:405-412`) and the warehouse's
    /// couriers (`CourierAssignmentModule.java:98-100`).
    TwiceLevel,
}

impl Capacity {
    /// How many citizens the module holds in a hut of `level`.
    ///
    /// Level `0` is a hut placed and not built, and Java's lambdas answer `0`
    /// there — `getBuildingLevel()` is `0`, not `1`
    /// (`core/colony/buildings/AbstractSchematicProvider.java:64`). The port keeps
    /// that and does not clamp: a caller that may hire has already checked
    /// `canAssignCitizens` (`api/colony/buildings/IBuilding.java:464-467`).
    pub fn at(self, level: i32) -> i32 {
        match self {
            Capacity::One => 1,
            Capacity::Two => 2,
            Capacity::PerLevel => level,
            Capacity::TwiceLevel => 2 * level,
        }
    }
}

/// One worker module: a hut, the job it hires, and what it declares about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Employment {
    /// The hut that carries the module.
    pub hut: Kind,
    /// Java's `ModuleProducer` key, `farmer_craft` style — the join between a hut
    /// and its job (`api/colony/buildings/registry/BuildingEntry.java:204-220`).
    /// One row disagrees: the warehouse's module is produced as
    /// `warehouse_couriers` but saves itself as `warehouse`
    /// (`CourierAssignmentModule.java:109-112`), and the port keeps the key a
    /// building entry lists, not the one NBT holds.
    pub module: &'static str,
    /// The job hired here.
    pub job: Job,
    /// The skills and the rain rule; `None` for the three modules that are not
    /// `WorkerBuildingModule`s.
    pub worker: Option<Worker>,
    /// The head count.
    pub capacity: Capacity,
}

impl Employment {
    /// The two skills levelled here, primary first.
    pub fn skills(&self) -> Option<(Skill, Skill)> {
        self.worker.map(|w| (w.primary, w.secondary))
    }

    /// Whether the module may work in the rain, `None` when it declares nothing.
    pub fn works_in_rain(&self) -> Option<bool> {
        self.worker.map(|w| w.rain)
    }

    /// How many citizens the module holds at `level`.
    pub fn capacity_at(&self, level: i32) -> i32 {
        self.capacity.at(level)
    }
}

/// The table: **58** modules, one line each, grouped by hut in `Kind::ALL` order
/// so that [`employees`] can answer with one contiguous slice.
///
/// A line reads `hut: "module key" => job, (primary, secondary, rain), heads`, and
/// `-` in the worker column is Java stating no skills. Every line but the last
/// column of the three `-` rows is one `new WorkerBuildingModule(...)` call in
/// `BuildingModules.java`, whose line is the comment above it.
macro_rules! employments {
    ($($hut:ident: $module:literal => $job:ident, $worker:tt, $capacity:ident;)*) => {
        /// Every worker module Java registers, grouped by hut.
        pub const EMPLOYMENTS: &[Employment] = &[$(Employment {
            hut: Kind::$hut,
            module: $module,
            job: Job::$job,
            worker: worker_of!($worker),
            capacity: Capacity::$capacity,
        }),*];
    };
}

/// The worker column of one row: two skills and a rain flag, or `-`.
///
/// A separate macro so the table stays one line per module and a missing worker
/// is written, not inferred.
macro_rules! worker_of {
    (-) => {
        None
    };
    (($primary:ident, $secondary:ident, $rain:literal)) => {
        Some(Worker {
            primary: Skill::$primary,
            secondary: Skill::$secondary,
            rain: $rain,
        })
    };
}

employments! {
    // BuildingModules.java:251-253
    Alchemist: "alchemist_work" => Alchemist, (Dexterity, Mana, false), One;
    // :538-540
    Archery: "archery_work" => ArcherTraining, (Agility, Adaptability, false), PerLevel;
    // :263-265
    Baker: "baker_work" => Baker, (Knowledge, Dexterity, false), One;
    // :549-563, the five guard types of one barracks tower
    BarracksTower: "knight_barracks_work" => Knight, (Adaptability, Stamina, true), PerLevel;
    BarracksTower: "ranger_barracks_work" => Ranger, (Agility, Adaptability, true), PerLevel;
    BarracksTower: "druid_barracks_work" => Druid, (Mana, Focus, true), PerLevel;
    BarracksTower: "huscarl_barracks_work" => Huscarl, (Adaptability, Stamina, true), PerLevel;
    BarracksTower: "marksman_barracks_work" => Marksman, (Agility, Adaptability, true), PerLevel;
    // :156-158
    Beekeeper: "beekeeper_work" => Beekeeper, (Dexterity, Adaptability, false), One;
    // :267-269
    Blacksmith: "blacksmith_work" => Blacksmith, (Strength, Focus, false), One;
    // :430-432
    Builder: "builder_work" => Builder, (Adaptability, Athletics, false), One;
    // :169-171
    ChickenHerder: "chickenherder_work" => ChickenHerder, (Adaptability, Agility, false), One;
    // :542-547
    CombatAcademy: "knight_training" => CombatTraining, (Adaptability, Stamina, false), PerLevel;
    // :113-116
    Composter: "composter_work" => Composter, (Stamina, Athletics, false), One;
    // :273-275
    ConcreteMixer: "concretemixer_work" => ConcreteMixer, (Stamina, Dexterity, false), One;
    // :447-449 — the one job that serves food and works in the rain
    Cook: "cook_craft" => Cook, (Adaptability, Knowledge, true), One;
    // :179-182
    Cowboy: "cowherder_work" => Cowboy, (Athletics, Stamina, false), One;
    // :279-281
    Crusher: "crusher_work" => Crusher, (Stamina, Strength, false), One;
    // :390-392
    Deliveryman: "courier_work" => Deliveryman, (Agility, Adaptability, false), One;
    // :289-296
    Dyer: "dyer_work" => Dyer, (Creativity, Dexterity, false), One;
    // :606-608
    Enchanter: "enchanter_work" => Enchanter, (Mana, Knowledge, false), One;
    // :122-124 — the farmer's worker module is `farmer_craft`; `farmer_work` is
    // its crafting module and hires nobody
    Farmer: "farmer_craft" => Farmer, (Stamina, Athletics, false), One;
    // :211-214
    Fisherman: "fisher_work" => Fisherman, (Focus, Agility, false), One;
    // :302-309
    Fletcher: "fletcher_work" => Fletcher, (Dexterity, Creativity, true), One;
    // :145-148
    Florist: "florist_work" => Florist, (Dexterity, Agility, false), One;
    // :580-585 — the one hut whose guard modules are `(b) -> 2`
    GateHouse: "knight_gate_work" => Knight, (Adaptability, Stamina, true), Two;
    GateHouse: "ranger_gate_work" => Ranger, (Agility, Adaptability, true), Two;
    // :316-323
    Glassblower: "glassblower_work" => Glassblower, (Creativity, Focus, false), One;
    // :616-619
    Graveyard: "graveyard_work" => Undertaker, (Strength, Mana, false), One;
    // :569-578 — four of the five guard types; `druid_tower_work` exists at :573
    // but no building lists it, so a guard tower never hires a druid
    GuardTower: "knight_tower_work" => Knight, (Adaptability, Stamina, true), PerLevel;
    GuardTower: "ranger_tower_work" => Ranger, (Agility, Adaptability, true), PerLevel;
    GuardTower: "marksman_tower_work" => Marksman, (Agility, Adaptability, true), PerLevel;
    GuardTower: "huscarl_tower_work" => Huscarl, (Adaptability, Stamina, true), PerLevel;
    // :481-484
    Hospital: "healer_work" => Healer, (Mana, Knowledge, true), One;
    // :450-457
    Kitchen: "chef_work" => Chef, (Creativity, Knowledge, false), One;
    // :409-412 — the one job whose two skills are the same one
    Library: "student_work" => Student, (Intelligence, Intelligence, true), TwiceLevel;
    // :466-469
    Lumberjack: "forester_work" => Lumberjack, (Strength, Focus, false), One;
    // :332-334
    Mechanic: "mechanic_work" => Mechanic, (Knowledge, Agility, false), One;
    // :513-514 — `QuarryModule`, not a worker module (QuarryModule.java:28)
    MediumQuarry: "medium_quarry" => Quarrier, -, One;
    // :491-494
    Miner: "miner_work" => Miner, (Strength, Stamina, false), One;
    // :508-510 — the mine hires both of its jobs
    Miner: "quarrier_work" => Quarrier, (Strength, Stamina, false), One;
    // :621-623
    NetherWorker: "netherworker_work" => NetherWorker, (Adaptability, Strength, false), One;
    // :134-136
    Plantation: "plantation_work" => Planter, (Agility, Dexterity, false), One;
    // :216-218
    RabbitHutch: "rabbitherder_work" => RabbitHerder, (Agility, Athletics, false), One;
    // :341-343
    Sawmill: "sawmill_work" => Sawmill, (Knowledge, Dexterity, false), One;
    // :414-417
    School: "teacher_work" => Teacher, (Knowledge, Mana, true), One;
    // :405-407
    School: "pupil_work" => Pupil, (Knowledge, Mana, true), TwiceLevel;
    // :226-229 — note Java's own spelling, `sheperd_work`
    Shepherd: "sheperd_work" => Shepherd, (Focus, Strength, false), One;
    // :350-352
    Sifter: "sifter_work" => Sifter, (Focus, Strength, false), One;
    // :511-512 — `QuarryModule(32)`, the same three columns as the medium quarry
    SimpleQuarry: "simple_quarry" => Quarrier, -, One;
    // :356-358
    Smeltery: "smelter_work" => Smelter, (Athletics, Strength, false), One;
    // :565-567
    Stable: "cavalry_stable_work" => Cavalry, (Adaptability, Stamina, true), PerLevel;
    // :193-196
    Stable: "stablemaster_work" => Stablemaster, (Athletics, Dexterity, false), One;
    // :366-368
    StoneMason: "stonemason_work" => StoneMason, (Creativity, Dexterity, false), One;
    // :375-382
    StoneSmeltery: "stonesmelter_work" => StoneSmeltery, (Athletics, Dexterity, false), One;
    // :237-239
    SwineHerder: "swineherder_work" => SwineHerder, (Strength, Athletics, true), One;
    // :419-422
    University: "university_work" => Researcher, (Knowledge, Mana, true), PerLevel;
    // :394-395 — `CourierAssignmentModule`, not a worker module either
    Warehouse: "warehouse_couriers" => Deliveryman, -, TwiceLevel;
}

impl Job {
    /// Every module that hires this job, in table order.
    pub fn employments(self) -> impl Iterator<Item = &'static Employment> {
        EMPLOYMENTS.iter().filter(move |row| row.job == self)
    }

    /// Every hut that hires this job, in table order — one for most jobs, three
    /// for a knight. Empty for [`Job::Placeholder`].
    pub fn huts(self) -> impl Iterator<Item = Kind> {
        self.employments().map(|row| row.hut)
    }
}

/// Every module of one hut, or [`Refusal::BuildingEmploysNobody`].
///
/// A slice, not a `Vec`: the table is grouped, so the answer is a window into it.
pub fn employees(hut: Kind) -> Result<&'static [Employment], Refusal> {
    let start = match EMPLOYMENTS.iter().position(|row| row.hut == hut) {
        Some(start) => start,
        None => return Err(Refusal::BuildingEmploysNobody { hut }),
    };
    let run = EMPLOYMENTS[start..]
        .iter()
        .take_while(|row| row.hut == hut)
        .count();
    Ok(&EMPLOYMENTS[start..start + run])
}

/// The module a hut carries for a job — how Java answers "may this building hire
/// this citizen": `WorkerBuildingModule.assignCitizen` refuses when no module
/// names the job (`WorkerBuildingModule.java:98-118`).
pub fn employ(hut: Kind, job: Job) -> Result<&'static Employment, Refusal> {
    employees(hut)?
        .iter()
        .find(|row| row.job == job)
        .ok_or(Refusal::JobNotEmployedHere { hut, job })
}

/// Why a hut will not take a worker, named.
///
/// Both variants are Java facts rather than this port's inventions: a building
/// with no worker module cannot assign anyone — there is nothing to assign to —
/// and a module names one `JobEntry`, so a knight cannot be hired at a farm.
///
/// A variant is **not** a `lang` key, and it is not the player-facing refusal
/// either: Java's is the `WARNING_*` interaction a failed hiring attempt raises.
/// These names are for logs and tests, namespaced like a registry entry so they
/// can never be mistaken for a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The hut has no worker module at all: a residence, a town hall, a postbox.
    BuildingEmploysNobody { hut: Kind },
    /// The hut hires, but not this job.
    JobNotEmployedHere { hut: Kind, job: Job },
}

impl Refusal {
    /// A stable name, namespaced.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::BuildingEmploysNobody { .. } => "jobs.refusal.nobody",
            Refusal::JobNotEmployedHere { .. } => "jobs.refusal.not_employed",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_eight_modules_over_forty_seven_huts_and_no_row_is_lost() {
        assert_eq!(EMPLOYMENTS.len(), 58);
        let mut huts: Vec<Kind> = EMPLOYMENTS.iter().map(|row| row.hut).collect();
        huts.sort();
        huts.dedup();
        assert_eq!(huts.len(), 47, "47 of the 54 kinds hire somebody");
        // The table is grouped, which is what lets `employees` return a slice:
        // summing every hut's window must give the table back exactly once.
        let summed: usize = Kind::ALL
            .iter()
            .filter_map(|hut| employees(*hut).ok())
            .map(|rows| rows.len())
            .sum();
        assert_eq!(summed, EMPLOYMENTS.len());
    }

    #[test]
    fn the_seven_huts_that_hire_nobody_name_the_refusal() {
        let idle = [
            Kind::Barracks,
            Kind::MysticalSite,
            Kind::Postbox,
            Kind::Residence,
            Kind::Stash,
            Kind::Tavern,
            Kind::TownHall,
        ];
        assert_eq!(Kind::ALL.len() - idle.len(), 47);
        for hut in idle {
            assert_eq!(employees(hut), Err(Refusal::BuildingEmploysNobody { hut }));
        }
    }

    #[test]
    fn forty_nine_jobs_are_hired_somewhere_and_the_placeholder_nowhere() {
        let hired: Vec<Job> = Job::ALL
            .iter()
            .copied()
            .filter(|job| job.employments().next().is_some())
            .collect();
        assert_eq!(hired.len(), 49);
        assert!(!hired.contains(&Job::Placeholder));
        assert_eq!(Job::Placeholder.huts().count(), 0);
    }

    #[test]
    fn the_jobs_that_work_in_several_huts_are_the_guards_the_quarrier_and_the_courier() {
        let huts_of = |job: Job| {
            let mut huts: Vec<&str> = job.huts().map(|hut| hut.name()).collect();
            huts.sort_unstable();
            huts
        };
        assert_eq!(
            huts_of(Job::Knight),
            ["barrackstower", "gatehouse", "guardtower"]
        );
        assert_eq!(
            huts_of(Job::Ranger),
            ["barrackstower", "gatehouse", "guardtower"]
        );
        assert_eq!(huts_of(Job::Marksman), ["barrackstower", "guardtower"]);
        assert_eq!(huts_of(Job::Huscarl), ["barrackstower", "guardtower"]);
        assert_eq!(
            huts_of(Job::Quarrier),
            ["mediumquarry", "miner", "simplequarry"]
        );
        assert_eq!(huts_of(Job::Deliveryman), ["deliveryman", "warehouse"]);
        // The druid only works in a barracks tower: `druid_tower_work` is defined
        // (`BuildingModules.java:573-574`) and no building lists it.
        assert_eq!(huts_of(Job::Druid), ["barrackstower"]);
        assert_eq!(huts_of(Job::Farmer), ["farmer"]);
        assert_eq!(huts_of(Job::Student), ["library"]);
        assert_eq!(huts_of(Job::Pupil), ["school"]);
    }

    #[test]
    fn a_hut_that_hires_refuses_another_jobs_worker_by_name() {
        assert_eq!(
            employ(Kind::Farmer, Job::Farmer).unwrap().module,
            "farmer_craft"
        );
        assert_eq!(
            employ(Kind::Farmer, Job::Knight),
            Err(Refusal::JobNotEmployedHere {
                hut: Kind::Farmer,
                job: Job::Knight
            })
        );
        assert_eq!(
            employ(Kind::Tavern, Job::Cook),
            Err(Refusal::BuildingEmploysNobody { hut: Kind::Tavern }),
            "a tavern has visitors, not a worker module"
        );
        assert_ne!(
            Refusal::BuildingEmploysNobody { hut: Kind::Tavern }.name(),
            Refusal::JobNotEmployedHere {
                hut: Kind::Tavern,
                job: Job::Cook
            }
            .name()
        );
    }

    #[test]
    fn the_head_count_has_four_shapes_and_zero_for_an_unbuilt_hut() {
        assert_eq!(Capacity::One.at(5), 1);
        assert_eq!(Capacity::Two.at(1), 2, "a gatehouse holds two at level 1");
        assert_eq!(
            Capacity::PerLevel.at(3),
            3,
            "a tower holds one guard per level"
        );
        assert_eq!(
            Capacity::TwiceLevel.at(2),
            4,
            "a school holds four pupils at level 2"
        );
        for shape in [
            Capacity::One,
            Capacity::Two,
            Capacity::PerLevel,
            Capacity::TwiceLevel,
        ] {
            assert!(shape.at(0) <= 2, "nothing holds more than two at level 0");
        }
        // The hut's own row decides, not the job's: the same knight is two heads
        // at a gatehouse and one per level in a tower.
        assert_eq!(
            employ(Kind::GateHouse, Job::Knight).unwrap().capacity_at(4),
            2
        );
        assert_eq!(
            employ(Kind::GuardTower, Job::Knight)
                .unwrap()
                .capacity_at(4),
            4
        );
    }

    #[test]
    fn rain_is_a_property_of_the_module_not_of_the_job() {
        assert_eq!(
            employ(Kind::SwineHerder, Job::SwineHerder)
                .unwrap()
                .works_in_rain(),
            Some(true)
        );
        assert_eq!(
            employ(Kind::Cook, Job::Cook).unwrap().works_in_rain(),
            Some(true)
        );
        assert_eq!(
            employ(Kind::Farmer, Job::Farmer).unwrap().works_in_rain(),
            Some(false)
        );
        // Three huts, one answer for every guard they hire — but a stable is not
        // one of them: its guard works in the rain, its stablemaster does not.
        for hut in [Kind::BarracksTower, Kind::GateHouse, Kind::GuardTower] {
            for row in employees(hut).unwrap() {
                assert_eq!(row.works_in_rain(), Some(true), "{:?}", row.module);
            }
        }
        assert_eq!(
            employ(Kind::Stable, Job::Cavalry).unwrap().works_in_rain(),
            Some(true)
        );
        assert_eq!(
            employ(Kind::Stable, Job::Stablemaster)
                .unwrap()
                .works_in_rain(),
            Some(false),
            "the same hut, two modules, two answers"
        );
        // The three modules that declare nothing say so rather than "no".
        assert_eq!(
            employ(Kind::Warehouse, Job::Deliveryman)
                .unwrap()
                .works_in_rain(),
            None
        );
    }

    #[test]
    fn the_three_modules_that_are_not_worker_modules_state_no_skills() {
        let skilless: Vec<&str> = EMPLOYMENTS
            .iter()
            .filter(|row| row.skills().is_none())
            .map(|row| row.module)
            .collect();
        assert_eq!(
            skilless,
            ["medium_quarry", "simple_quarry", "warehouse_couriers"],
            "QuarryModule and CourierAssignmentModule are not WorkerBuildingModules"
        );
        // Every other module states two, and the student states one skill twice.
        assert_eq!(
            employ(Kind::Library, Job::Student).unwrap().skills(),
            Some((Skill::Intelligence, Skill::Intelligence))
        );
        assert_eq!(
            employ(Kind::GuardTower, Job::Knight).unwrap().skills(),
            Some((Skill::Adaptability, Skill::Stamina))
        );
        for row in EMPLOYMENTS {
            if let Some((primary, secondary)) = row.skills() {
                assert!(
                    Skill::ALL.contains(&primary),
                    "{} names no primary",
                    row.module
                );
                assert!(
                    Skill::ALL.contains(&secondary),
                    "{} names no secondary",
                    row.module
                );
            }
        }
    }

    #[test]
    fn every_module_key_is_unique_even_across_huts() {
        for (index, row) in EMPLOYMENTS.iter().enumerate() {
            for other in &EMPLOYMENTS[index + 1..] {
                assert_ne!(
                    row.module, other.module,
                    "two modules share the key {:?}, so one would shadow the other",
                    row.module
                );
            }
        }
    }

    #[test]
    fn a_skill_drags_a_complimentary_one_up_and_an_adverse_one_down() {
        assert_eq!(Skill::ALL.len(), 11);
        for (index, skill) in Skill::ALL.iter().enumerate() {
            for other in &Skill::ALL[index + 1..] {
                assert_ne!(skill.name(), other.name(), "two skills share a name");
            }
            // Both relations are mutual in Java: `Athletics` raises `Strength`
            // and `Strength` raises `Athletics` (`Skill.java:8-163`).
            if let Some(up) = skill.complimentary() {
                assert_eq!(
                    up.complimentary(),
                    Some(*skill),
                    "{} is not mutual",
                    skill.name()
                );
                assert_ne!(Some(*skill), skill.adverse());
            }
            if let Some(down) = skill.adverse() {
                assert_eq!(
                    down.adverse(),
                    Some(*skill),
                    "{} is not mutual",
                    skill.name()
                );
            }
        }
        assert_eq!(Skill::Athletics.complimentary(), Some(Skill::Strength));
        assert_eq!(Skill::Athletics.adverse(), Some(Skill::Dexterity));
        // Intelligence is the odd one: no pair at all, so nothing moves with it.
        assert_eq!(Skill::Intelligence.complimentary(), None);
        assert_eq!(Skill::Intelligence.adverse(), None);
    }
}
