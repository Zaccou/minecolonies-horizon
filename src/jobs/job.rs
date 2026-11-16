//! The jobs, their registry paths, their display names and the tools they state.
//!
//! Java reference: `api/colony/jobs/ModJobs.java` (**51** `ResourceLocation`
//! constants, one of them `@Deprecated`), the initialiser that registers them
//! (`apiimp/initializer/ModJobsInitializer.java`, 51 entries) and the 50 job
//! classes under `core/colony/jobs/` and `core/colony/jobs/guard/`.
//!
//! **The count, since three different numbers look right and only one is.**
//!
//! | What | How many | Where |
//! | --- | --- | --- |
//! | `ModJobs` ids | 51 | `ModJobs.java:13-65` |
//! | entries registered | 51 | `ModJobsInitializer.java:28-332` — `register` is called once per entry (`:342-346`) |
//! | **distinct job classes** | **50** | the initialiser names 51 classes, but `cookassistant` reuses `JobChef` (`:298-302`, `:328-332`) |
//! | **jobs this module carries** | **50** | [`Job::ALL`] |
//! | jobs that work in a hut | 49 | everything but [`Job::Placeholder`] |
//!
//! `cookassistant` is **not** a [`Job`]: it is `@Deprecated` in Java
//! (`ModJobs.java:64-65`), reuses the chef's class, and has no translation key
//! (`assets/minecolonies/lang/manual_en_us.json` has 49 `com.minecolonies.job.*`
//! lines between `:2272` and `:2792`, none for it), so no citizen can be shown as
//! one. A saved colony that
//! still holds the id loads as a chef; [`Job::from_path`] answering `None` for
//! `cookassistant` is what makes that rename explicit instead of silent.
//!
//! **Where the display name comes from.** `JobEntry` builds the key as
//! `"com." + namespace + ".job." + path` (`api/colony/jobs/registry/JobEntry.java:129`),
//! and the English text is a resource line, not code — this port carries both:
//! [`Job::translation_key`] rebuilds Java's key and [`Job::display`] holds the
//! string `manual_en_us.json:2271-2320` gives it, so a log line and a test can
//! both read a job without a lang file. `Placeholder` is the one job Java never
//! names (there is no `com.minecolonies.job.placeholder` line): "Used if a
//! certain building doesn't have a job yet"
//! (`core/colony/jobs/JobPlaceholder.java:9`), and its AI is `null` (`:31-34`).

use super::tool::ToolKind;

macro_rules! jobs {
    ($($variant:ident => $path:literal, $display:literal, [$($tool:ident),*],)*) => {
        /// One entry of the `minecolonies:jobs` registry.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        // exhaustive: the registry is a closed list in Java, and a new job is a
        // change to this enum
        pub enum Job {
            $($variant,)*
        }

        impl Job {
            /// Every job class Java registers, `Placeholder` first — the order
            /// `ModJobsInitializer` registers them in.
            pub const ALL: &'static [Job] = &[$(Job::$variant,)*];

            /// The registry path, without its namespace: `builder`, not
            /// `minecolonies:builder` (`ModJobs.java:14`).
            pub fn path(self) -> &'static str {
                match self {
                    $(Job::$variant => $path,)*
                }
            }

            /// The English name `assets/minecolonies/lang/manual_en_us.json`
            /// gives the job, already resolved.
            pub fn display(self) -> &'static str {
                match self {
                    $(Job::$variant => $display,)*
                }
            }

            /// The tools this job **states** before it works, in the order its
            /// AI adds them. Empty is "states none", not "needs none": see
            /// [`Self::declares_tools`] and the module note.
            pub fn tools(self) -> &'static [ToolKind] {
                match self {
                    $(Job::$variant => &[$(ToolKind::$tool),*],)*
                }
            }
        }
    };
}

jobs! {
    Placeholder => "placeholder", "Placeholder", [],
    Alchemist => "alchemist", "Alchemist", [Shears],
    ArcherTraining => "archertraining", "Archer Squire", [Bow],
    Baker => "baker", "Baker", [],
    Beekeeper => "beekeeper", "Beekeeper", [Shears],
    Blacksmith => "blacksmith", "Blacksmith", [],
    Builder => "builder", "Builder", [],
    Cavalry => "cavalry", "Cavalry", [Spear],
    Chef => "chef", "Chef", [],
    ChickenHerder => "chickenherder", "Chickenherder", [Axe],
    CombatTraining => "combattraining", "Knight Squire", [Sword, Shield],
    Composter => "composter", "Composter", [],
    ConcreteMixer => "concretemixer", "Concrete Mixer", [],
    Cook => "cook", "Waiter", [],
    Cowboy => "cowboy", "Cowhand", [Axe],
    Crusher => "crusher", "Crusher", [],
    Deliveryman => "deliveryman", "Courier", [],
    Druid => "druid", "Druid", [],
    Dyer => "dyer", "Dyer", [],
    Enchanter => "enchanter", "Enchanter", [],
    Farmer => "farmer", "Farmer", [Hoe],
    Fisherman => "fisherman", "Fisher", [Rod],
    Fletcher => "fletcher", "Fletcher", [],
    Florist => "florist", "Florist", [Shears],
    Glassblower => "glassblower", "Glassblower", [],
    Healer => "healer", "Doctor", [],
    Huscarl => "huscarl", "Huscarl", [Axe],
    Knight => "knight", "Knight", [Sword],
    Lumberjack => "lumberjack", "Forester", [Axe, Hoe],
    Marksman => "marksman", "Marksman", [Crossbow],
    Mechanic => "mechanic", "Mechanic", [],
    Miner => "miner", "Miner", [],
    NetherWorker => "netherworker", "Nether Miner", [Axe, Pickaxe, Shovel, Sword, FlintAndSteel],
    Planter => "planter", "Planter", [],
    Pupil => "pupil", "Pupil", [],
    Quarrier => "quarrier", "Quarrier", [],
    RabbitHerder => "rabbitherder", "Rabbitherder", [Axe],
    Ranger => "ranger", "Archer", [Bow],
    Researcher => "researcher", "Researcher", [],
    Sawmill => "sawmill", "Carpenter", [],
    Shepherd => "shepherd", "Shepherd", [Axe, Shears],
    Sifter => "sifter", "Sifter", [],
    Smelter => "smelter", "Smelter", [],
    Stablemaster => "stablemaster", "Stablemaster", [Lead],
    StoneMason => "stonemason", "Stonemason", [],
    StoneSmeltery => "stonesmeltery", "Brickmaker", [],
    Student => "student", "Student", [],
    SwineHerder => "swineherder", "Swineherder", [Axe],
    Teacher => "teacher", "Teacher", [],
    Undertaker => "undertaker", "Undertaker", [Shovel],
}

impl Job {
    /// The job a bare registry path names, or `None`.
    ///
    /// `None` for `cookassistant`: see the module table.
    pub fn from_path(path: &str) -> Option<Job> {
        Job::ALL.into_iter().find(|job| job.path() == path).copied()
    }

    /// Java's translation key for the job, `com.minecolonies.job.builder`
    /// (`api/colony/jobs/registry/JobEntry.java:129`).
    ///
    /// Built rather than stored: a stored key is a second copy of the path, and
    /// the two could disagree.
    pub fn translation_key(self) -> String {
        format!("com.minecolonies.job.{}", self.path())
    }

    /// Whether the job states at least one tool.
    ///
    /// **`false` is not "needs no tool".** Java has two other routes to a tool
    /// request, neither of which is a per-job fact:
    ///
    /// - a **crafter** asks for the tool the *recipe* names
    ///   (`core/entity/ai/workers/crafting/AbstractEntityAICrafting.java:355-364`),
    ///   so a carpenter needs an axe for one recipe and nothing for the next;
    /// - any worker that meets a block it must break asks for the best tool for
    ///   that **block**, with the block's own harvest level as the floor
    ///   (`AbstractEntityAIBasic.java:1338-1377`), which is why a miner carries a
    ///   pickaxe without the miner job stating one.
    pub fn declares_tools(self) -> bool {
        !self.tools().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_jobs_and_only_the_placeholder_does_no_work() {
        assert_eq!(Job::ALL.len(), 50);
        for (index, job) in Job::ALL.iter().enumerate() {
            for other in &Job::ALL[index + 1..] {
                assert_ne!(job.path(), other.path(), "two jobs share a registry path");
            }
            assert_eq!(Job::from_path(job.path()), Some(*job));
        }
        // The id Java keeps for a save from before the split: same class, no
        // translation key, deprecated. It is not a job a citizen can hold.
        assert_eq!(Job::from_path("cookassistant"), None);
        assert_ne!(Job::from_path("chef"), None);
    }

    #[test]
    fn the_path_and_the_display_name_are_two_different_strings() {
        // The path is the registry's, the display the lang file's, and they are
        // not the same string for twelve jobs: `cook` is a Waiter, `ranger` an
        // Archer, `sawmill` a Carpenter, `deliveryman` a Courier.
        assert_eq!(Job::Cook.path(), "cook");
        assert_eq!(Job::Cook.display(), "Waiter");
        assert_eq!(Job::Ranger.display(), "Archer");
        assert_eq!(Job::Sawmill.display(), "Carpenter");
        assert_eq!(Job::Lumberjack.display(), "Forester");
        assert_eq!(Job::StoneSmeltery.display(), "Brickmaker");
        assert_eq!(Job::Healer.display(), "Doctor");
        assert_eq!(Job::Cowboy.display(), "Cowhand");
        assert_eq!(Job::ArcherTraining.display(), "Archer Squire");
        assert_eq!(Job::CombatTraining.display(), "Knight Squire");
        assert_eq!(Job::NetherWorker.display(), "Nether Miner");
        assert_eq!(Job::ConcreteMixer.display(), "Concrete Mixer");
        assert_eq!(
            Job::Builder.translation_key(),
            "com.minecolonies.job.builder",
            "JobEntry.java:129, and the lang line is manual_en_us.json:2272"
        );
        for job in Job::ALL {
            assert!(!job.display().is_empty(), "{} has no name", job.path());
        }
    }

    #[test]
    fn twenty_one_jobs_state_a_tool_and_the_other_twenty_nine_do_not() {
        let stating: Vec<&str> = Job::ALL
            .iter()
            .filter(|job| job.declares_tools())
            .map(|job| job.path())
            .collect();
        assert_eq!(
            stating.len(),
            21,
            "the jobs whose AI calls checkForToolOrWeapon with a fixed type"
        );
        assert!(
            !stating.contains(&"miner"),
            "the miner's pickaxe is the block's"
        );
        assert!(!stating.contains(&"builder"));
        assert!(!stating.contains(&"cook"));
    }

    #[test]
    fn every_guard_states_the_weapon_its_job_class_gives_it() {
        // `EntityAIMelee` adds a sword, an axe for the huscarl (`:33-40`);
        // `EntityAIRange` adds `job.getEquipmentType()` (`:30`), which is a bow
        // for the ranger (`JobRanger.java:44-47`) and a crossbow for the marksman
        // (`JobMarksman.java:25-27`); the cavalry's weapon is a spear
        // (`JobCavalry.java:186-188`) and the druid states none at all — it
        // throws potions.
        assert_eq!(Job::Knight.tools(), &[ToolKind::Sword]);
        assert_eq!(Job::Huscarl.tools(), &[ToolKind::Axe]);
        assert_eq!(Job::Ranger.tools(), &[ToolKind::Bow]);
        assert_eq!(Job::Marksman.tools(), &[ToolKind::Crossbow]);
        assert_eq!(Job::Cavalry.tools(), &[ToolKind::Spear]);
        assert!(Job::Druid.tools().is_empty());
        // The two trainees, and the one guard that needs a shield to train.
        assert_eq!(Job::ArcherTraining.tools(), &[ToolKind::Bow]);
        assert_eq!(
            Job::CombatTraining.tools(),
            &[ToolKind::Sword, ToolKind::Shield]
        );
    }

    #[test]
    fn the_tool_lists_are_as_long_as_the_ai_makes_them() {
        assert_eq!(
            Job::Farmer.tools(),
            &[ToolKind::Hoe],
            "EntityAIWorkFarmer.java:256"
        );
        assert_eq!(Job::Fisherman.tools(), &[ToolKind::Rod], ":207");
        assert_eq!(Job::Undertaker.tools(), &[ToolKind::Shovel], ":180");
        assert_eq!(Job::Florist.tools(), &[ToolKind::Shears], ":309");
        assert_eq!(Job::Beekeeper.tools(), &[ToolKind::Shears], ":166");
        assert_eq!(Job::Alchemist.tools(), &[ToolKind::Shears], ":236");
        // The forester is the one job whose second tool depends on a setting:
        // `axe` and `shears` when the hut has USE_SHEARS, `axe` and `hoe`
        // otherwise (`EntityAIWorkLumberjack.java:295`). The port keeps the
        // default branch and says so.
        assert_eq!(
            Job::Lumberjack.tools(),
            &[ToolKind::Axe, ToolKind::Hoe],
            "the shears branch is a setting, not a second job"
        );
        // The shepherd's shears are conditional the same way, on SHEARING
        // (`EntityAIWorkShepherd.java:56-64`), and the stablemaster's lead is
        // *instead of* the axe every other herder needs (`:143-149`).
        assert_eq!(Job::Shepherd.tools(), &[ToolKind::Axe, ToolKind::Shears]);
        assert_eq!(Job::Stablemaster.tools(), &[ToolKind::Lead]);
        for herder in [
            Job::Cowboy,
            Job::SwineHerder,
            Job::RabbitHerder,
            Job::ChickenHerder,
        ] {
            assert_eq!(
                herder.tools(),
                &[ToolKind::Axe],
                "AbstractEntityAIHerder.java:176-181"
            );
        }
        // The one job that states five: the nether miner reaches the nether.
        assert_eq!(
            Job::NetherWorker.tools(),
            &[
                ToolKind::Axe,
                ToolKind::Pickaxe,
                ToolKind::Shovel,
                ToolKind::Sword,
                ToolKind::FlintAndSteel
            ],
            "EntityAIWorkNether.java:248-252"
        );
    }
}
