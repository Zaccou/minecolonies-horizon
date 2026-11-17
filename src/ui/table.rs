//! The two joins: which screen a hut kind opens, and which screen a métier reaches.
//!
//! Java reference: `IBuildingView.getWindow()` (`api/colony/buildings/views/IBuildingView.java:157`),
//! its default in `AbstractBuildingView.java:390-399`, the six `getWindow()` overrides
//! under `core/colony/buildings/`, and `IBuildingModuleView.getWindow()`
//! (`api/colony/buildings/modules/IBuildingModuleView.java:47`) for the sidebar tabs.
//!
//! **The join is on the hut, not on the métier, and that is Java's design.** A window is
//! asked for by a building, never by a job: `IBuildingView.getWindow()` has no argument.
//! So this table starts from [`Kind`] and answers a screen; a job's screen is then *the
//! screens of the huts that hire it* ([`screens_of`]). The consequence is worth stating
//! because it contradicts the obvious guess: **the forty-nine métiers reach three main
//! screens between them** — [`super::screen::HUT_WORKER_ID`], plus
//! [`super::screen::HUT_MIN_ID`] for the quarries and the warehouse, plus
//! [`super::screen::HUT_BUILDER_ID`] for the one job whose hut has a window of its own —
//! and only the quarrier and the courier reach two.
//!
//! **Why `hut.min` is not just "the huts that hire nobody".** The warehouse, the simple
//! quarry and the medium quarry *do* hire — a courier and two quarriers — but through
//! modules that are not `WorkerBuildingModule`s
//! (`CourierAssignmentModule.java:25`, `QuarryModule.java:28`), and the default window
//! asks for exactly that class (`AbstractBuildingView.java:392-397`). The employment
//! table already says which rows those are: their `worker` column is `-`. So
//! [`has_worker_module`] reads the column instead of inventing a second list.
//!
//! **Two buildings have a window and no hut screen**, and they say so by name:
//! [`Refusal::NotAHutScreen`] for the town hall (`WindowMainPage`,
//! `BuildingTownHall.java:248`) and the postbox (`WindowPostBoxMain`, `PostBox.java:125`).
//! Both are colony furniture, not a métier's hut, and neither hires anybody.

use crate::building::Kind;
use crate::jobs::{Employment, Job, employ, employees};

use super::action::{Effect, Refusal};
use super::screen::{
    HUT_BARRACKS_ID, HUT_BUILDER_ID, HUT_LIVING_ID, HUT_MIN_ID, HUT_WORKER_ID, SCREENS, Screen,
    TAB_MINER_LEVELS_ID, TAB_RESOURCES_ID, TAB_TASKS_ID,
};

/// The four kinds that carry a `getWindow()` override of their own.
///
/// Six overrides exist in Java and they name five distinct windows; two of them are the
/// windows this table does not carry ([`NOT_A_HUT_WINDOW`]), so four rows are left. The
/// residence and the tavern share one window through two different modules
/// (`HomeBuildingModule.java:44`, `TavernBuildingModule.java:439`).
const OWN_WINDOW: &[(Kind, &'static str)] = &[
    // BuildingBuilder.java:230
    (Kind::Builder, HUT_BUILDER_ID),
    // BuildingBarracks.java:231
    (Kind::Barracks, HUT_BARRACKS_ID),
    // HomeBuildingModule.java:44
    (Kind::Residence, HUT_LIVING_ID),
    // TavernBuildingModule.java:439
    (Kind::Tavern, HUT_LIVING_ID),
];

/// The two windows that exist and are **not** hut screens.
///
/// `BuildingWareHouse.getWindow()` is a third override and it is **not** here: it answers
/// the same `WindowHutMinPlaceholder` the default answers, so the warehouse needs no row
/// ([`has_worker_module`] already sends it to `hut.min`, `BuildingWareHouse.java:219`).
const NOT_A_HUT_WINDOW: &[(Kind, &'static str)] = &[
    // BuildingTownHall.java:248
    (Kind::TownHall, "WindowMainPage"),
    // PostBox.java:125
    (Kind::Postbox, "WindowPostBoxMain"),
];

/// Every module view whose `getWindow()` this table describes, by **module key**.
///
/// Five keys, three screens: the two task-list modules are two views of one window
/// (`RequestTaskModuleView.getWindow` is not abstract and both subclasses inherit it,
/// `RequestTaskModuleView.java:30-33`). A key here is the same string
/// [`crate::building::Module::key`] saves and `BuildingModules` declares
/// (`BuildingModules.java:52`, `:53`, `:398`, `:444`, `:497`), so a saved module can be
/// asked for its screen without a second name for it.
pub const MODULE_SCREENS: &[(&str, &str)] = &[
    ("buildingresources", TAB_RESOURCES_ID),
    ("miner_levels", TAB_MINER_LEVELS_ID),
    ("craft_task_view", TAB_TASKS_ID),
    ("courier_task_view", TAB_TASKS_ID),
    ("warehouse_request_queue", TAB_TASKS_ID),
];

/// The screen with this id, or [`Refusal::UnknownScreen`].
///
/// The refusal is not decoration: it is what a broken row of a table returns, and a test
/// walks every row to prove none is broken.
pub fn screen_by_id(id: &str) -> Result<&'static Screen, Refusal> {
    SCREENS
        .iter()
        .copied()
        .find(|screen| screen.id == id)
        .ok_or(Refusal::UnknownScreen)
}

/// The screen a module view opens, from its module key.
pub fn module_screen(module_key: &str) -> Option<&'static Screen> {
    let id = MODULE_SCREENS
        .iter()
        .find(|(key, _)| *key == module_key)
        .map(|(_, id)| *id)?;
    SCREENS.iter().copied().find(|screen| screen.id == id)
}

/// Whether the hut carries a `WorkerBuildingModule` — the exact question
/// `AbstractBuildingView.getWindow()` asks with
/// `getModuleViews(WorkerBuildingModuleView.class).isEmpty()`
/// (`AbstractBuildingView.java:392-397`).
///
/// Read off the employment table's `worker` column instead of a class list: the three
/// rows whose column is `-` are the three modules that are not `WorkerBuildingModule`s
/// (`crate::jobs::employment`, module note), so the column *is* the answer.
pub fn has_worker_module(hut: Kind) -> bool {
    employees(hut)
        .map(|rows| rows.iter().any(|row| row.worker.is_some()))
        .unwrap_or(false)
}

/// The screen a hut opens, or the reason it opens none of ours.
///
/// This is `IBuildingView.getWindow()` in table form, in Java's own order:
/// the override first, then the worker module, then the minimum window.
pub fn main_screen(hut: Kind) -> Result<&'static Screen, Refusal> {
    if let Some((_, id)) = OWN_WINDOW.iter().find(|(kind, _)| *kind == hut) {
        return screen_by_id(id);
    }
    if NOT_A_HUT_WINDOW.iter().any(|(kind, _)| *kind == hut) {
        return Err(Refusal::NotAHutScreen { hut });
    }
    if has_worker_module(hut) {
        screen_by_id(HUT_WORKER_ID)
    } else {
        screen_by_id(HUT_MIN_ID)
    }
}

/// Every (hut, screen) a job reaches, in table order — one per hut that hires it.
///
/// Empty for [`Job::Placeholder`], which no hut hires. Three huts for a knight, and all
/// three answer the same screen; three for a quarrier, and two of them do not.
pub fn screens_of(job: Job) -> impl Iterator<Item = (Kind, &'static Screen)> {
    job.huts().filter_map(|hut| main_screen(hut).ok().map(|screen| (hut, screen)))
}

/// The screen of the first hut that hires this job, or
/// [`Refusal::NoScreenForJob`].
///
/// **Java has no such lookup** — it never goes from a job to a window — so this is this
/// table's convenience, and it is only meaningful because most jobs have one hut. A
/// caller that wants all of them uses [`screens_of`]; a caller that wants *the* screen
/// of a knight is asking a question with three answers.
pub fn screen_of(job: Job) -> Result<&'static Screen, Refusal> {
    match screens_of(job).next() {
        Some((_, screen)) => Ok(screen),
        None => Err(Refusal::NoScreenForJob { job }),
    }
}

/// The module that hires this job in this hut, or the jobs table's own refusal
/// ([`crate::jobs::employ`]).
///
/// This is what a screen needs before it can draw a worker row: the job's two skills,
/// its rain rule and its head count all hang off the module, not off the job
/// (`crate::jobs::employment`, "three columns are the module's").
pub fn worker_hut(hut: Kind, job: Job) -> Result<&'static Employment, Refusal> {
    employ(hut, job).map_err(Refusal::HutRefuses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Refusal as EmploymentRefusal;

    /// How many kinds answer each of the five screens a hut opens.
    fn census() -> Vec<(&'static str, usize)> {
        let mut counts = Vec::new();
        for id in [
            HUT_WORKER_ID,
            HUT_BUILDER_ID,
            HUT_MIN_ID,
            HUT_LIVING_ID,
            HUT_BARRACKS_ID,
        ] {
            let count = Kind::ALL
                .iter()
                .filter(|hut| main_screen(**hut).map(|screen| screen.id) == Ok(id))
                .count();
            counts.push((id, count));
        }
        counts
    }

    #[test]
    fn fifty_four_kinds_and_fifty_two_of_them_have_a_hut_screen() {
        let mut ok = 0;
        let mut refused = 0;
        for hut in Kind::ALL {
            match main_screen(*hut) {
                Ok(_) => ok += 1,
                Err(Refusal::NotAHutScreen { .. }) => refused += 1,
                Err(other) => panic!("{} was refused by {other}", hut.name()),
            }
        }
        assert_eq!(Kind::ALL.len(), 54);
        assert_eq!(ok, 52);
        assert_eq!(refused, 2);
        // The five screens those fifty-two kinds share, and how many each one takes.
        let mut census = census();
        census.sort_unstable();
        assert_eq!(
            census,
            [
                (HUT_BARRACKS_ID, 1),
                (HUT_BUILDER_ID, 1),
                (HUT_LIVING_ID, 2),
                (HUT_MIN_ID, 5),
                (HUT_WORKER_ID, 43),
            ]
        );
        assert_eq!(43 + 1 + 1 + 2 + 5 + 2, 54);
    }

    #[test]
    fn the_two_buildings_whose_window_is_not_a_hut_screen_name_the_refusal() {
        for (hut, window) in NOT_A_HUT_WINDOW {
            assert_eq!(main_screen(*hut), Err(Refusal::NotAHutScreen { hut: *hut }));
            assert_eq!(
                Refusal::NotAHutScreen { hut: *hut }.name(),
                "ui.refusal.not_a_hut_screen"
            );
            assert_eq!(Refusal::NotAHutScreen { hut: *hut }.key(), None);
            // The window exists; it is simply not one of the nine WindowHut* files.
            assert!(!window.starts_with("WindowHut"), "{window} ");
            assert!(!SCREENS.iter().any(|screen| screen.window == *window));
        }
        assert_eq!(NOT_A_HUT_WINDOW.len(), 2);
    }

    #[test]
    fn no_row_of_this_table_names_a_screen_that_does_not_exist() {
        for (hut, id) in OWN_WINDOW {
            assert!(
                screen_by_id(id).is_ok(),
                "{} names {id}, which no screen has",
                hut.name()
            );
            assert_eq!(screen_by_id(id).map(|screen| screen.id), Ok(*id));
        }
        for (key, id) in MODULE_SCREENS {
            assert!(module_screen(key).is_some(), "{key} answers nothing");
            assert_eq!(module_screen(key).map(|screen| screen.id), Some(*id));
        }
        // Every `Opens` target of every screen, which is the same proof from the other
        // side: an effect that names a screen has to name a real one.
        for screen in SCREENS {
            for action in screen.actions {
                if let Effect::Opens(target) = action.effect {
                    assert!(
                        screen_by_id(target).is_ok(),
                        "{}.{} opens {target}, which no screen has",
                        screen.id,
                        action.id
                    );
                }
            }
        }
    }

    #[test]
    fn forty_nine_jobs_reach_a_screen_and_the_placeholder_is_refused() {
        let mut reached = 0;
        for job in Job::ALL {
            match screen_of(*job) {
                Ok(screen) => {
                    reached += 1;
                    assert!(
                        [HUT_WORKER_ID, HUT_MIN_ID, HUT_BUILDER_ID].contains(&screen.id),
                        "{} reached {}",
                        job.path(),
                        screen.id
                    );
                    // The builder is the one job whose hut has a window of its own.
                    if screen.id == HUT_BUILDER_ID {
                        assert_eq!(*job, Job::Builder);
                    }
                }
                Err(refusal) => {
                    assert_eq!(*job, Job::Placeholder);
                    assert_eq!(refusal, Refusal::NoScreenForJob { job: *job });
                    assert_eq!(refusal.name(), "ui.refusal.no_screen");
                }
            }
        }
        assert_eq!(reached, 49);
        assert_eq!(
            screen_of(Job::Placeholder),
            Err(Refusal::NoScreenForJob { job: Job::Placeholder })
        );
    }

    #[test]
    fn the_knight_reaches_three_huts_and_one_screen_and_the_quarrier_three_huts_and_two() {
        let knight: Vec<(Kind, &str)> = screens_of(Job::Knight)
            .map(|(hut, screen)| (hut, screen.id))
            .collect();
        assert_eq!(knight.len(), 3, "barrackstower, gatehouse, guardtower");
        assert!(knight.iter().all(|(_, id)| *id == HUT_WORKER_ID));
        // One screen, three huts: the window is the hut's, the métier is the module's.
        let mut knight_screens: Vec<&str> = knight.iter().map(|(_, id)| *id).collect();
        knight_screens.sort_unstable();
        knight_screens.dedup();
        assert_eq!(knight_screens, [HUT_WORKER_ID]);

        let quarrier: Vec<(Kind, &str)> = screens_of(Job::Quarrier)
            .map(|(hut, screen)| (hut, screen.id))
            .collect();
        assert_eq!(quarrier.len(), 3, "miner, simplequarry, mediumquarry");
        // The employment table is grouped by hut in `Kind::ALL` order, which is
        // alphabetical by registry path: mediumquarry before miner.
        let huts: Vec<&str> = quarrier.iter().map(|(hut, _)| hut.name()).collect();
        assert_eq!(huts, ["mediumquarry", "miner", "simplequarry"]);
        let mut ids: Vec<&str> = quarrier.iter().map(|(_, id)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        // Two of the three huts hire through a QuarryModule, so they answer `hut.min`.
        assert_eq!(ids, [HUT_MIN_ID, HUT_WORKER_ID]);
    }

    #[test]
    fn exactly_two_jobs_reach_two_screens_and_the_other_forty_seven_reach_one() {
        let mut one = 0;
        let mut two = Vec::new();
        for job in Job::ALL {
            // The placeholder is not hired anywhere: it is the refusal, below.
            if *job == Job::Placeholder {
                continue;
            }
            let mut ids: Vec<&str> = screens_of(*job).map(|(_, screen)| screen.id).collect();
            ids.sort_unstable();
            ids.dedup();
            match ids.len() {
                1 => one += 1,
                2 => two.push(job.path()),
                _ => panic!("{} reaches {} screens", job.path(), ids.len()),
            }
        }
        // The two jobs that work both in a worker hut and in a module-only one.
        assert_eq!(two, ["deliveryman", "quarrier"]);
        assert_eq!(one, 47);
        assert_eq!(one + two.len(), 49);
        // The builder is one of the forty-seven: its hut has a screen of its own.
        assert_eq!(screen_of(Job::Builder).map(|screen| screen.id), Ok(HUT_BUILDER_ID));
    }

    #[test]
    fn the_five_module_keys_name_three_screens() {
        assert_eq!(MODULE_SCREENS.len(), 5);
        let mut ids: Vec<&str> = MODULE_SCREENS.iter().map(|(_, id)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids, [TAB_MINER_LEVELS_ID, TAB_RESOURCES_ID, TAB_TASKS_ID]);
        // The two task-list views share one window class, so one screen.
        assert_eq!(module_screen("craft_task_view"), module_screen("courier_task_view"));
        assert_eq!(module_screen("warehouse_request_queue"), module_screen("craft_task_view"));
        assert_ne!(module_screen("miner_levels"), module_screen("buildingresources"));
        // A key nobody declares answers nothing, by name rather than by returning a
        // worker screen.
        assert_eq!(module_screen("worker"), None);
    }

    #[test]
    fn only_a_hut_with_a_worker_module_gets_the_worker_screen() {
        for hut in Kind::ALL {
            match main_screen(*hut) {
                Ok(screen) if screen.id == HUT_WORKER_ID => {
                    assert!(has_worker_module(*hut), "{} ", hut.name());
                    assert!(employees(*hut).is_ok());
                }
                Ok(screen) if screen.id == HUT_MIN_ID => {
                    assert!(!has_worker_module(*hut), "{} ", hut.name());
                }
                _ => {}
            }
        }
        // The three huts that hire through a module that is not a worker module: they
        // answer `hut.min` even though the employment table has a row for them.
        for hut in [Kind::Warehouse, Kind::SimpleQuarry, Kind::MediumQuarry] {
            assert!(employees(hut).is_ok(), "{} hires somebody", hut.name());
            assert!(!has_worker_module(hut));
            assert_eq!(main_screen(hut).map(|screen| screen.id), Ok(HUT_MIN_ID));
        }
        // Two huts hire nobody and still answer the minimum window.
        for hut in [Kind::MysticalSite, Kind::Stash] {
            assert!(employees(hut).is_err());
            assert_eq!(main_screen(hut).map(|screen| screen.id), Ok(HUT_MIN_ID));
        }
    }

    #[test]
    fn a_job_is_only_drawn_by_a_hut_that_actually_hires_it() {
        assert_eq!(
            worker_hut(Kind::Farmer, Job::Farmer).map(|employment| employment.module),
            Ok("farmer_craft")
        );
        assert_eq!(
            worker_hut(Kind::Farmer, Job::Knight),
            Err(Refusal::HutRefuses(EmploymentRefusal::JobNotEmployedHere {
                hut: Kind::Farmer,
                job: Job::Knight,
            }))
        );
        assert_eq!(
            worker_hut(Kind::Tavern, Job::Cook),
            Err(Refusal::HutRefuses(EmploymentRefusal::BuildingEmploysNobody {
                hut: Kind::Tavern,
            }))
        );
        // The refusal keeps the jobs table's name, so a caller can tell the two apart.
        let full = worker_hut(Kind::Tavern, Job::Cook).err().unwrap_or(Refusal::NoRoom);
        let other = worker_hut(Kind::Farmer, Job::Knight).err().unwrap_or(Refusal::NoRoom);
        assert_ne!(full.name(), other.name());
        // The module is what a worker screen needs: two skills and a rain rule.
        let employment = worker_hut(Kind::Miner, Job::Miner).ok();
        assert!(employment.is_some());
        assert_eq!(
            employment.and_then(|employment| employment.skills()),
            Some((crate::jobs::Skill::Strength, crate::jobs::Skill::Stamina))
        );
    }
}
