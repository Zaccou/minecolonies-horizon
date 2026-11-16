//! Growing a hut one level: what the next level changes, what it keeps, and what
//! it recomputes.
//!
//! Java reference: `core/colony/buildings/AbstractBuilding.java:784-830`
//! (`requestUpgrade`, which decides *what kind of work order* to raise) and
//! `:940-990` (`onUpgradeComplete`, which is what actually happens when the
//! builder has finished), `managers/RegisteredStructureManager.java:734-745`
//! (`onBuildingUpgradeComplete`), `managers/CitizenManager.java:120-155`
//! (`calculateMaxCitizens`) and `AbstractSchematicProvider.java:480-490` (the
//! level write itself).
//!
//! **Three questions, three answers, and the port keeps them apart.**
//!
//! - **What kind of work order** — a hut nobody built takes a `BUILD`, a built hut
//!   takes an `UPGRADE`, and a hut at its ceiling takes neither
//!   (`AbstractBuilding.java:803-814`). `src/building/mod.rs` lists "no work
//!   order" as a gap and points at `crate::colony::WorkOrder`; this is the rule
//!   that fills it, and [`Step::order`] is the answer.
//! - **What changes** — the level, the chunks it holds, the seats each module
//!   offers, and the colony's citizen ceiling. [`Step`] carries all four, before
//!   and after, so a caller never has to recompute a delta.
//! - **What is recomputed and what is kept** — [`RECALCULATED`] is Java's eleven
//!   steps of `onUpgradeComplete` in its own order, [`CONSERVED`] is the six
//!   things the level change does not touch.
//!
//! **The ceiling is not always five.** `AbstractBuilding.java:807` compares the
//! level to `getMaxBuildingLevel()`, which eleven subclasses override — a
//! gatehouse stops at 3, a postbox at 1, a stash at 0 (see `behaviour::max_level`).
//! `building::Refusal::AlreadyMaxLevel` can only ever mean "five", so the port
//! names the per-kind ceiling separately.
//!
//! **The research gate, and what it is missing.** Java asks the colony's research
//! tree twice before it raises an order: is the hut unlocked at all
//! (`:788-792`), and is the hut's own level allowed by the effect's strength
//! (`:794-798`). The port has no research tree, so [`Research`] is what the caller
//! read out of one: whether the hut's effect exists, and its strength. A caller
//! with no research system passes [`Research::none`], which is Java's behaviour
//! for a hut the tree does not mention.
//!
//! **What the citizen census cannot count yet.** Java's `calculateMaxCitizens`
//! sums `getAssignedCitizen().size()` per module and reads each module's
//! `getModuleMax()`; the port's `building::Building` holds **one worker per
//! building** (`instance.rs` says why), so a school with a teacher and two pupils
//! contributes one head here and four there. And a residence in `HiringMode.LOCKED`
//! counts its *assigned* citizens instead of its capacity (`CitizenManager.java:145-148`);
//! the port has no hiring mode on a building, so a residence always counts its
//! capacity. Both gaps are `MANQUANT.md`'s, not this file's inventions.

use crate::building::{self, Building, Refusal as BuildingRefusal, Standing};
use crate::colony::WorkType;

use super::{Refusal, capacity_at, max_level, modules, modules::Catalogue};

/// What the colony's research says about one hut.
///
/// Two facts because Java asks two questions: `hasResearchEffect` (does the tree
/// know this hut at all) and `getEffectStrength` (how far the effect goes). A hut
/// the tree does not mention is unconstrained, which is why [`Self::none`] is not
/// "locked".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Research {
    /// `getGlobalResearchTree().hasResearchEffect(hutResearch)` (`:788`).
    pub has_effect: bool,
    /// `getEffectStrength(hutResearch)` (`:791`, `:796`).
    pub strength: i32,
}

impl Research {
    /// No research tree, or one that says nothing about this hut: no gate at all.
    pub fn none() -> Research {
        Research {
            has_effect: false,
            strength: 0,
        }
    }

    /// An effect of the given strength, which is the level the hut may reach.
    pub fn at(strength: i32) -> Research {
        Research {
            has_effect: true,
            strength,
        }
    }
}

/// What `onUpgradeComplete` recomputes, **in Java's order** — the order matters
/// because the claim (2) has to know the new level and the tape (4) has to be
/// removed before the corners (5) are measured again.
///
/// `AbstractBuilding.java:941-989`, one variant per statement:
///
/// 1. `cachedRotation = -1` — the rotation is re-read from the blueprint;
/// 2. `claimBuildingChunks(…, newLevel, …)` — only for a hut with no parent;
/// 3. `recheckGuardBuildingNear = true` — the guard coverage of this hut and its
///    neighbours is stale (`RegisteredStructureManager.java:713-725`);
/// 4. `removeConstructionTape` — the tape of the level just finished;
/// 5. `calculateCorners` — the new schematic is bigger or smaller;
/// 6. `isBuilt = true`;
/// 7. the stuck **tool requests** are cancelled (`:952-966`);
/// 8. every module's `onUpgradeComplete(newLevel)` (`IBuildingEventsModule`);
/// 9. `checkAutoStartResearch` — the university may now start something;
/// 10. `onBuildingUpgradeComplete` → `calculateMaxCitizens`
///     (`RegisteredStructureManager.java:736-741`);
/// 11. `calculatePrestige(blueprint)` — only when a blueprint was pasted, and the
///     port has no `SchemAnalyzerUtil`, so `MANQUANT.md` §4 carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: eleven statements, closed on purpose
pub enum Recalculated {
    Rotation,
    ClaimChunks,
    GuardCoverage,
    ConstructionTape,
    Corners,
    BuiltFlag,
    StuckToolRequests,
    Modules,
    Research,
    Citizens,
    Prestige,
}

/// The eleven steps of an upgrade, in Java's order.
pub const RECALCULATED: [Recalculated; 11] = [
    Recalculated::Rotation,
    Recalculated::ClaimChunks,
    Recalculated::GuardCoverage,
    Recalculated::ConstructionTape,
    Recalculated::Corners,
    Recalculated::BuiltFlag,
    Recalculated::StuckToolRequests,
    Recalculated::Modules,
    Recalculated::Research,
    Recalculated::Citizens,
    Recalculated::Prestige,
];

/// What an upgrade **does not** touch: the answer to "ce qui est conservé".
///
/// Every one of these is a fact stored on, or reached through, the building that
/// `setBuildingLevel` (`AbstractSchematicProvider.java:480-490`) writes nothing
/// to — the level is one field, and everything else survives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the six things a level change leaves alone
pub enum Kept {
    /// The modules and their own saved data (`AbstractBuilding.java:385-393`).
    Modules,
    /// The citizens assigned to those modules: a level change fires nobody
    /// (`AbstractAssignedCitizenModule.onUpgradeComplete` only re-models them).
    Workers,
    /// The parent link and, with it, the children (`AbstractSchematicProvider.java:309-321`).
    Parent,
    /// The player's custom name (`AbstractBuilding.java:349-355`).
    CustomName,
    /// The open requests: an upgrade cancels **tool** requests only, because the
    /// new level may accept a better tool (`:952-966`).
    Requests,
    /// The hut's own contents — what the request system delivers never leaves.
    Contents,
}

/// The six things an upgrade keeps.
pub const CONSERVED: [Kept; 6] = [
    Kept::Modules,
    Kept::Workers,
    Kept::Parent,
    Kept::CustomName,
    Kept::Requests,
    Kept::Contents,
];

/// The colony's ceiling on citizens, and the ceiling it could reach.
///
/// `calculateMaxCitizens` (`CitizenManager.java:120-155`) walks every built
/// building and takes one of two branches:
///
/// | Condition | Contribution | Where |
/// | --- | --- | --- |
/// | has a `bed` **and** a `WorkAtHomeBuildingModule` | the workers actually assigned, plus room to spare | `:133-140` |
/// | has a `LivingBuildingModule` | the module's capacity | `:141-152` |
/// | neither | nothing | — |
///
/// The two lists are deliberately different: a guard tower houses the guards it
/// hired, a residence houses citizens whether or not they work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Census {
    /// `getMaxCitizens()`: how many citizens the colony may hold now.
    pub max: i32,
    /// `getPotentialMaxCitizens()`: `max` plus the free beds.
    pub potential: i32,
}

/// The colony's citizen ceiling, from the buildings that stand.
///
/// A building at level 0 contributes nothing (`if (b.getBuildingLevel() > 0)`,
/// `:128`): a hut nobody built houses nobody.
pub fn census(standing: &Standing) -> Census {
    let mut max = 0;
    let mut spare = 0;
    for building in standing.iter() {
        let level = building.level();
        if level <= 0 {
            continue;
        }
        let kind = building.kind();
        // The two branches are `if (bed && WorkAtHome) … else if (living) …`, and
        // the `else` matters: a residence has a bed and no worker at home, so it
        // falls through to its living module rather than counting nothing.
        let at_home = Catalogue::holders(kind).find(|module| module.at_home());
        let resident =
            Catalogue::holders(kind).find(|module| matches!(module.role, modules::Role::Resident));
        if modules::has(kind, "bed") && at_home.is_some() {
            // The port holds one worker per building, so the assigned count is
            // 0 or 1; Java counts the module's list.
            let heads = i32::from(building.worker().is_some());
            max += heads;
            if let Some(module) = at_home {
                let capacity = capacity_at(kind, module.key, level).unwrap_or(0);
                spare += (capacity - heads).max(0);
            }
        } else if let Some(module) = resident {
            max += capacity_at(kind, module.key, level).unwrap_or(0);
        }
    }
    Census {
        max,
        potential: max + spare,
    }
}

/// One level: what the next one is called, and everything it changes.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Step {
    pub kind: crate::building::Kind,
    /// The level the hut stands at now, `0` included.
    pub from: i32,
    /// The level it would reach: `from + 1`.
    pub to: i32,
    /// `Build` when the hut nobody built is being raised the first time,
    /// `Upgrade` afterwards — Java's two branches of `requestUpgrade`.
    pub order: WorkType,
    /// The chunks it holds before and after: `[before, after]`, from
    /// `behaviour::claim_radius`.
    pub claim: [i32; 2],
    /// Every module that holds a citizen, with its seats before and after:
    /// `(key, before, after)`.
    pub seats: Vec<(&'static str, i32, i32)>,
    /// The colony's ceiling before and after: `[before, after]`.
    pub citizens: [Census; 2],
}

impl Step {
    /// The modules whose seat count goes up, which is the only reason a level
    /// ever matters to a player.
    pub fn gained(&self) -> Vec<(&'static str, i32)> {
        self.seats
            .iter()
            .filter(|(_, before, after)| after > before)
            .map(|(key, before, after)| (*key, after - before))
            .collect()
    }
}

/// Can this hut grow, and by how much? The research gate, the kind's ceiling, the
/// order to raise, then Java's parent rules through
/// `building::Standing::check_upgrade`.
///
/// The order is Java's, and it is why a gatehouse at level 3 answers
/// [`Refusal::AtKindMaxLevel`] rather than "no upgrade": Java's test is
/// `getBuildingLevel() < getMaxBuildingLevel()` (`:807`), and a gatehouse's
/// `getMaxBuildingLevel` is 3.
pub fn check(
    standing: &Standing,
    hut: &Building,
    requirements: &building::Rules,
    research: Research,
) -> Result<Step, Refusal> {
    let kind = hut.kind();
    let from = hut.level();

    // 1. The research gate (`AbstractBuilding.java:788-798`): the hut must be
    //    unlocked, and the effect's strength must allow the *current* level to
    //    grow.
    if research.has_effect {
        if research.strength < 1 {
            return Err(Refusal::ResearchLocked);
        }
        if research.strength <= from {
            return Err(Refusal::ResearchBelow {
                strength: research.strength,
                level: from,
            });
        }
    }

    // 2. The kind's own ceiling (`:807`, `getMaxBuildingLevel` overridden).
    let ceiling = max_level(kind);
    if from >= ceiling {
        return Err(Refusal::AtKindMaxLevel {
            level: from,
            max: ceiling,
        });
    }

    // 3. Which order. A hut nobody built takes a `BUILD`, and only when the
    //    parent it nests in is built (`:803-805`): a tower cannot rise inside a
    //    barracks that is still a hole in the ground.
    let order = if from < building::LEVEL_MIN {
        if let Some(at) = hut.parent() {
            if let Some(parent) = standing.at(at) {
                if parent.level() <= 0 {
                    return Err(Refusal::ParentNotBuilt { at });
                }
            }
        }
        WorkType::Build
    } else {
        // 4. The parent rules, quoted from the port's own implementation
        //    (`:801`, `:807`): the parent must exist, the hut may not pass it
        //    unless the parent is at its own top.
        standing.check_upgrade(hut, requirements).map_err(Refusal::Rules)?;
        WorkType::Upgrade
    };

    let to = from + 1;

    // 5. The seats, module by module.
    let seats = Catalogue::holders(kind)
        .filter_map(|module| {
            let before = capacity_at(kind, module.key, from)?;
            let after = capacity_at(kind, module.key, to)?;
            Some((module.key, before, after))
        })
        .collect();

    // 6. The colony's ceiling, with the hut at its new level. The census is a
    //    fact about the colony, so the copy that grows is the one the colony
    //    holds — not the caller's, which may be a stale view of the same hut.
    let before = census(standing);
    let mut grown = standing.clone();
    let mut upgraded = standing.at(hut.at()).cloned().unwrap_or_else(|| hut.clone());
    // `complete` on a level above zero cannot fail for a hut whose level is in
    // range, and the range was checked at step 2.
    let _ = upgraded.complete(to);
    let _ = grown.insert(upgraded);
    let after = census(&grown);

    Ok(Step {
        kind,
        from,
        to,
        order,
        claim: [super::claim_radius(kind, from), super::claim_radius(kind, to)],
        seats,
        citizens: [before, after],
    })
}

/// Apply a step: the level write of `setBuildingLevel`
/// (`AbstractSchematicProvider.java:480-490`) and the built flag of
/// `onUpgradeComplete` (`AbstractBuilding.java:951`) in one call.
///
/// Everything else the step describes is world state or another module's, which
/// is why [`RECALCULATED`] exists: a caller that applied the level and stopped
/// would have a hut at the right level, the wrong claim, and a stale citizen
/// ceiling.
pub fn apply(hut: &mut Building, step: &Step) -> Result<(), BuildingRefusal> {
    hut.complete(step.to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::{Kind, Requirements};

    const HALL: [i32; 3] = [0, 70, 0];
    const HUT: [i32; 3] = [10, 70, 0];
    const BARRACKS: [i32; 3] = [40, 70, 0];
    const TOWER: [i32; 3] = [44, 70, 0];

    fn requirements() -> building::Rules {
        let mut rules = building::Rules::new();
        for kind in [Kind::BarracksTower] {
            rules.set(
                kind,
                Requirements {
                    parent: building::ParentRule::Inside(Kind::Barracks),
                    parent_level: 1,
                    footprint: [5, 5, 5],
                },
            );
        }
        rules
    }

    fn standing_with(huts: &[(Kind, [i32; 3], i32)]) -> Standing {
        let mut standing = Standing::new();
        let mut town_hall = Building::new(Kind::TownHall, HALL);
        town_hall.complete(5).expect("a town hall");
        standing.insert(town_hall);
        for (kind, at, level) in huts {
            let mut hut = Building::new(*kind, *at);
            if *level > 0 {
                hut.complete(*level).expect("a built hut");
            }
            standing.insert(hut);
        }
        standing
    }

    fn hut(kind: Kind, at: [i32; 3], level: i32) -> Building {
        let mut hut = Building::new(kind, at);
        if level > 0 {
            hut.complete(level).expect("a built hut");
        }
        hut
    }

    #[test]
    fn a_built_hut_takes_an_upgrade_and_an_unbuilt_one_takes_a_build() {
        let standing = standing_with(&[(Kind::Builder, HUT, 2)]);
        let step = check(
            &standing,
            &hut(Kind::Builder, HUT, 2),
            &requirements(),
            Research::none(),
        )
        .expect("a builder at two may grow");
        assert_eq!(step.order, WorkType::Upgrade);
        assert_eq!((step.from, step.to), (2, 3));

        let fresh = hut(Kind::Builder, HUT, 0);
        let standing = standing_with(&[]);
        let step = check(&standing, &fresh, &requirements(), Research::none())
            .expect("a hut nobody built is a build, not an upgrade");
        assert_eq!(step.order, WorkType::Build, "AbstractBuilding.java:803-805");
        assert_eq!((step.from, step.to), (0, 1));
    }

    #[test]
    fn the_kinds_ceiling_is_not_always_five() {
        let standing = standing_with(&[(Kind::GateHouse, HUT, 3)]);
        assert_eq!(
            check(
                &standing,
                &hut(Kind::GateHouse, HUT, 3),
                &requirements(),
                Research::none()
            ),
            Err(Refusal::AtKindMaxLevel { level: 3, max: 3 }),
            "BuildingGateHouse.java:34: a gatehouse stops at three"
        );
        // And five is the end of the road for everything else.
        let standing = standing_with(&[(Kind::Builder, HUT, 5)]);
        assert_eq!(
            check(
                &standing,
                &hut(Kind::Builder, HUT, 5),
                &requirements(),
                Research::none()
            ),
            Err(Refusal::AtKindMaxLevel { level: 5, max: 5 })
        );
        // A postbox is never upgraded and a stash never has a level.
        for (kind, level) in [(Kind::Postbox, 1), (Kind::Stash, 0)] {
            let standing = standing_with(&[]);
            assert_eq!(
                check(&standing, &hut(kind, HUT, level), &requirements(), Research::none()),
                Err(Refusal::AtKindMaxLevel {
                    level,
                    max: max_level(kind)
                }),
                "{kind}"
            );
        }
    }

    #[test]
    fn the_research_gate_asks_two_questions_in_javas_order() {
        let builder = hut(Kind::Builder, HUT, 1);
        let standing = standing_with(&[(Kind::Builder, HUT, 1)]);
        // A tree that knows the hut and says its effect is nothing: locked.
        assert_eq!(
            check(&standing, &builder, &requirements(), Research::at(0)),
            Err(Refusal::ResearchLocked),
            "AbstractBuilding.java:788-792"
        );
        // A strength of one allows *one* level, so a hut already at one may not
        // grow: `strength <= getBuildingLevel()` (`:794-798`).
        assert_eq!(
            check(&standing, &builder, &requirements(), Research::at(1)),
            Err(Refusal::ResearchBelow {
                strength: 1,
                level: 1
            })
        );
        // A strength of two lets it reach two.
        assert_eq!(
            check(&standing, &builder, &requirements(), Research::at(2)).map(|step| step.to),
            Ok(2)
        );
        // And a hut at two needs a strength of three, not two.
        let two = hut(Kind::Builder, HUT, 2);
        let standing = standing_with(&[(Kind::Builder, HUT, 2)]);
        assert_eq!(
            check(&standing, &two, &requirements(), Research::at(2)),
            Err(Refusal::ResearchBelow {
                strength: 2,
                level: 2
            })
        );
        assert_eq!(
            check(&standing, &two, &requirements(), Research::at(3)).map(|step| step.to),
            Ok(3)
        );
        // A tree that does not mention the hut imposes nothing.
        assert!(check(&standing, &two, &requirements(), Research::none()).is_ok());
    }

    #[test]
    fn a_nested_hut_waits_for_its_parent_and_for_its_own_build() {
        let rules = requirements();
        let mut tower = hut(Kind::BarracksTower, TOWER, 0);
        tower.set_parent(Some(BARRACKS)).expect("inside the barracks");

        // A barracks still in the ground: the tower cannot even be built.
        let unbuilt = {
            let mut standing = standing_with(&[(Kind::Barracks, BARRACKS, 0)]);
            let mut barracks = Building::new(Kind::Barracks, BARRACKS);
            barracks.set_level(0).expect("zero");
            standing.insert(barracks);
            standing
        };
        assert_eq!(
            check(&unbuilt, &tower, &rules, Research::none()),
            Err(Refusal::ParentNotBuilt { at: BARRACKS }),
            "AbstractBuilding.java:803-805"
        );

        // A built barracks: the build may go ahead.
        let standing = standing_with(&[(Kind::Barracks, BARRACKS, 2)]);
        assert_eq!(
            check(&standing, &tower, &rules, Research::none())
                .map(|step| step.order),
            Ok(WorkType::Build)
        );

        // And a built tower may not pass its parent.
        let mut grown = tower.clone();
        grown.complete(2).expect("two");
        let standing = standing_with(&[(Kind::Barracks, BARRACKS, 2)]);
        assert_eq!(
            check(&standing, &grown, &rules, Research::none()),
            Err(Refusal::Rules(BuildingRefusal::ParentLevelAhead {
                level: 2,
                parent: 2
            })),
            "the port's own check_upgrade, reached through the wrapping"
        );
    }

    #[test]
    fn a_level_change_moves_the_claim_the_seats_and_the_colony_ceiling() {
        let standing = standing_with(&[(Kind::Builder, HUT, 3)]);
        let step = check(
            &standing,
            &hut(Kind::Builder, HUT, 3),
            &requirements(),
            Research::none(),
        )
        .expect("three to four");
        assert_eq!(step.claim, [1, 2], "AbstractBuilding.java:658-665: 1..3 → 4..5");
        assert_eq!(step.seats, vec![("builder_work", 1, 1)], "a builder gains no seat");
        assert_eq!(step.gained(), Vec::new());
        assert_eq!(step.citizens, [Census { max: 0, potential: 0 }; 2], "a builder houses nobody");

        // The town hall's claim jumps from three to five at level five, not four.
        let standing = standing_with(&[]);
        let step = check(
            &standing,
            &hut(Kind::TownHall, HALL, 4),
            &requirements(),
            Research::none(),
        )
        .expect("four to five");
        assert_eq!(step.claim, [3, 5]);
    }

    #[test]
    fn a_residence_grows_the_colony_by_one_citizen_per_level() {
        let standing = standing_with(&[(Kind::Residence, HUT, 3)]);
        let step = check(
            &standing,
            &hut(Kind::Residence, HUT, 3),
            &requirements(),
            Research::none(),
        )
        .expect("three to four");
        assert_eq!(step.gained(), vec![("living", 1)], "one more bed");
        assert_eq!(step.citizens[0], Census { max: 3, potential: 3 });
        assert_eq!(step.citizens[1], Census { max: 4, potential: 4 });
    }

    #[test]
    fn a_school_grows_two_seats_a_level_and_the_citizen_ceiling_not_at_all() {
        // A school's two modules hold citizens, and neither houses anybody: the
        // census takes neither branch (CitizenManager.java:133-152).
        let standing = standing_with(&[(Kind::School, HUT, 2)]);
        let step = check(
            &standing,
            &hut(Kind::School, HUT, 2),
            &requirements(),
            Research::none(),
        )
        .expect("two to three");
        assert_eq!(
            step.gained(),
            vec![("pupil_work", 2)],
            "a teacher is one head at any level, a pupil is two per level"
        );
        assert_eq!(step.citizens, [Census { max: 0, potential: 0 }; 2]);
    }

    #[test]
    fn a_guard_tower_houses_the_guard_it_hired_and_keeps_room_for_the_rest() {
        // The WorkAtHome branch: the assigned heads count, and the free beds do
        // not (CitizenManager.java:133-140).
        let standing = standing_with(&[(Kind::GuardTower, HUT, 3)]);
        // One knight module holds three heads at level three; nobody is hired.
        assert_eq!(census(&standing), Census { max: 0, potential: 3 });
        // With the worker in place, the same tower counts one head and keeps two
        // beds spare.
        let mut staffed = Standing::new();
        let mut tower = hut(Kind::GuardTower, HUT, 3);
        tower.hire(7).expect("a built tower hires");
        staffed.insert(tower.clone());
        assert_eq!(census(&staffed), Census { max: 1, potential: 3 });
        // Level 3 → 4 adds a bed, not a worker.
        let step = check(&standing, &tower, &requirements(), Research::none()).expect("grow");
        assert_eq!(step.citizens[0], Census { max: 0, potential: 3 });
        assert_eq!(step.citizens[1], Census { max: 0, potential: 4 }, "four heads at level four");
    }

    #[test]
    fn a_hut_nobody_built_houses_nobody() {
        let standing = standing_with(&[]);
        assert_eq!(census(&standing), Census { max: 0, potential: 0 });
        let mut empty = Standing::new();
        empty.insert(hut(Kind::Residence, HUT, 0));
        assert_eq!(census(&empty), Census { max: 0, potential: 0 }, "level 0 houses nobody");
        // And two residences add up.
        let mut two = Standing::new();
        two.insert(hut(Kind::Residence, HUT, 2));
        two.insert(hut(Kind::Residence, [20, 70, 0], 3));
        assert_eq!(census(&two), Census { max: 5, potential: 5 });
        // A tavern's rooms count the same way, through the same module.
        let mut tavern = Standing::new();
        tavern.insert(hut(Kind::Tavern, HUT, 2));
        assert_eq!(census(&tavern), Census { max: 2, potential: 2 });
    }

    #[test]
    fn the_eleven_steps_of_an_upgrade_are_in_java_s_order_and_six_things_are_kept() {
        assert_eq!(RECALCULATED.len(), 11, "AbstractBuilding.java:941-989");
        for pair in RECALCULATED.windows(2) {
            assert!(pair[0] < pair[1], "the list is in Java's order, not sorted by name");
        }
        assert_eq!(RECALCULATED[0], Recalculated::Rotation, "cachedRotation = -1 comes first");
        assert_eq!(RECALCULATED[1], Recalculated::ClaimChunks, "then the claim, at the new level");
        assert_eq!(RECALCULATED[3], Recalculated::ConstructionTape, "the tape goes before the corners");
        assert_eq!(RECALCULATED[4], Recalculated::Corners);
        assert_eq!(RECALCULATED[5], Recalculated::BuiltFlag);
        assert_eq!(RECALCULATED[9], Recalculated::Citizens, "the census is second to last");
        assert_eq!(RECALCULATED[10], Recalculated::Prestige);
        assert_eq!(CONSERVED.len(), 6);
        assert!(CONSERVED.contains(&Kept::Modules));
        assert!(CONSERVED.contains(&Kept::Workers));
        assert!(CONSERVED.contains(&Kept::Requests));
    }

    #[test]
    fn applying_a_step_writes_the_level_and_the_built_flag() {
        let standing = standing_with(&[(Kind::Residence, HUT, 2)]);
        let step = check(
            &standing,
            &hut(Kind::Residence, HUT, 2),
            &requirements(),
            Research::none(),
        )
        .expect("two to three");
        let mut building = hut(Kind::Residence, HUT, 2);
        assert_eq!(apply(&mut building, &step), Ok(()));
        assert_eq!(building.level(), 3);
        assert_eq!(building.stage(), crate::building::Stage::Built);
        // The building's own rules still hold after the write: a module above it
        // is still impossible, and an upgrade to six is still refused.
        assert_eq!(
            building.set_level(6),
            Err(BuildingRefusal::LevelAboveMax { level: 6 })
        );
    }
}
