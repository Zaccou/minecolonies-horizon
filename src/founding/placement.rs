//! A hut after the founding: is its plot free, and does it stand in the colony?
//!
//! Java reference: four places, because a hut is validated by whoever puts it down —
//! `core/event/EventHandler.java:722-742` (is this inside a colony),
//! `api/blocks/AbstractBlockHut.java:305-338` and
//! `core/blocks/huts/BlockHutTownHall.java:219-234` (the colony's own rules, which
//! the `building` module already carries as `Standing::check_placement`),
//! `core/colony/managers/RegisteredStructureManager.java:563-566` (one building per
//! position) and `core/network/messages/server/DirectPlaceMessage.java:117` (the
//! block's own cell must be air).
//!
//! **This module decides, it does not place.** The hut block and the structure it
//! carries are the engine's work — `place_template`, in a hook, later. A placement
//! here is a [`Placed`]: the building as `building::Building` records it, and the
//! box the schematic covers, which the caller keeps for the paste.
//!
//! The four checks, in order:
//!
//! 1. **in the colony** — a hut outside one belongs to nobody, and Java asks that
//!    first (`EventHandler.java:722-742`);
//! 2. **the colony's rules** — one town hall, the hut inside its box, the box big
//!    enough, the parent it asks for (`building::Standing::check_placement`), which
//!    is the port's implementation of the two Java files above and is not re-done
//!    here;
//! 3. **the position is free** — a second building at one position is what
//!    `addNewBuilding` silently drops (`RegisteredStructureManager.java:566`);
//! 4. **the plot is clear** — [`Refusal::PlotBlocked`], a rule of this port: Java
//!    hands the clearing to the builder's job, and there is no job yet.
//!
//! **A colony record and a building record are not the same shape.** The colony
//! stores a kind as Java's registry path in a `String` (`colony::Building`), the
//! `building` module keys on a [`Kind`]. [`hut_of`] is the one conversion, and it is
//! where a path the registry table does not know is named instead of guessed.

use crate::building::{self, Building, Footprint, Kind, Site, Standing};
use crate::colony::{self, Colony};

use super::rules::Rules;
use super::terrain::{first_obstacle, Ground};
use super::Refusal;

/// A placement the rules accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Placed {
    /// The hut as the `building` module holds it: level 0, not built
    /// (`AbstractSchematicProvider.java:64`) — the builder's job starts here.
    pub building: Building,
    /// The box the schematic covers, which the paste will need and which the checks
    /// were made against (`Site::footprint`).
    pub footprint: Footprint,
}

/// One building of a colony record, as the `building` module holds it.
///
/// Level 0 is a hut placed and not built (`AbstractSchematicProvider.java:64`); 1 to
/// 5 is a finished build, whose `isBuilt` Java infers from the level on load
/// (`AbstractBuilding.java:328-339`). Anything else is refused rather than clamped:
/// a level outside the range means the record is wrong, not that the building is
/// small.
pub fn hut_of(record: &colony::Building) -> Result<Building, Refusal> {
    let kind = Kind::from_path(&record.kind).ok_or_else(|| Refusal::UnknownKind {
        path: record.kind.clone(),
    })?;
    let mut hut = Building::new(kind, record.at);
    if record.level != 0 {
        hut.complete(record.level).map_err(Refusal::Building)?;
    }
    Ok(hut)
}

/// Every building a colony record holds, as the `building` module holds them: what
/// [`building::Standing::check_placement`] needs and nothing else.
pub fn standing_of(colony: &Colony) -> Result<Standing, Refusal> {
    let mut standing = Standing::new();
    for record in &colony.buildings {
        let _ = standing.insert(hut_of(record)?);
    }
    Ok(standing)
}

/// May `kind` be placed at `site` in `colony`? The four checks of the table at the
/// top of this file, or the first rule that said no.
///
/// `requirements` is the pack's table for [`building::Standing::check_placement`]
/// (which kinds nest in which, in how big a box); `rules` is the founding
/// configuration, read for the colony's radius. The one thing not asked is whether
/// the player may place a hut here — that is `Action::PlaceHuts` on
/// `colony::Colony::may`, and it belongs to the hook that has a player.
pub fn plan(
    colony: &Colony,
    kind: Kind,
    site: &Site,
    ground: &dyn Ground,
    requirements: &building::Rules,
    rules: &Rules,
) -> Result<Placed, Refusal> {
    // 1. The hut stands in a colony (`EventHandler.java:722-742`). Java asks the
    //    chunk, not a radius (`Colony.java:1298-1308`); `Rules::inside_colony` is
    //    the same box.
    if !rules.inside_colony(colony.center, site.at) {
        return Err(Refusal::OutsideColony { at: site.at });
    }

    // 2. The colony's own rules, quoted from the `building` module rather than
    //    copied: one town hall (`BlockHutTownHall.java:219-234`), the hut inside its
    //    own box and the box big enough (`AbstractBlockHut.java:305-338`,
    //    `AbstractSchematicProvider.java:505-521`), the parent it asks for (`:303-321`).
    let standing = standing_of(colony)?;
    standing
        .check_placement(kind, site, requirements)
        .map_err(Refusal::Building)?;

    // 3. Nothing stands there already (`RegisteredStructureManager.java:566`:
    //    `if (!buildings.containsKey(tileEntity.getPosition()))`).
    if standing.at(site.at).is_some() {
        return Err(Refusal::PositionTaken { at: site.at });
    }

    // 4. The plot is clear (a rule of this port). The hut's own cell is set aside:
    //    its block is already in the world when its building is registered
    //    (`DirectPlaceMessage.java:137-138`).
    if let Some(at) = first_obstacle(ground, site.footprint, site.at) {
        return Err(Refusal::PlotBlocked { at });
    }

    let mut building = Building::new(kind, site.at);
    building.set_parent(site.parent).map_err(Refusal::Building)?;
    Ok(Placed {
        building,
        footprint: site.footprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Stage;

    use super::super::colonies::Registry;
    use super::super::found::{found, Founder, Founding};
    use super::super::terrain::Air;

    const PLAYER: u128 = 7;
    const OVERWORLD: &str = "minecraft:overworld";
    const HALL: [i32; 3] = [100, 70, 100];

    struct Blocks(Vec<[i32; 3]>);

    impl Ground for Blocks {
        fn is_free(&self, at: [i32; 3]) -> bool {
            !self.0.contains(&at)
        }
    }

    /// The world a colony is founded in: its town hall's block entity is there, and
    /// every other cell is empty.
    struct Hall;

    impl Ground for Hall {
        fn is_free(&self, _at: [i32; 3]) -> bool {
            true
        }

        fn holds_town_hall(&self, _at: [i32; 3]) -> bool {
            true
        }
    }

    const WITH_HALL: Hall = Hall;

    /// A colony founded by the module next door: the town hall at `HALL`, level 1,
    /// nothing else built.
    fn colony() -> Colony {
        let colonies = Registry::new();
        found(&Founding::new(
            Founder::new(PLAYER, "Simon"),
            HALL,
            OVERWORLD,
            [0, 70, 0],
            &WITH_HALL,
            &colonies,
        ))
        .expect("a colony on free ground")
        .colony
    }

    /// The default plan — open air, no requirements table, the default distances —
    /// of a hut `offset` blocks from the town hall on both axes, in a box of `size`.
    fn plan_at(
        colony: &Colony,
        kind: Kind,
        offset: i32,
        size: [u32; 3],
    ) -> Result<Placed, Refusal> {
        plan_site(
            colony,
            kind,
            Site::alone([HALL[0] + offset, HALL[1], HALL[2] + offset], size),
        )
    }

    /// The same plan, at a site the caller placed itself.
    fn plan_site(colony: &Colony, kind: Kind, site: Site) -> Result<Placed, Refusal> {
        plan(colony, kind, &site, &Air, &building::Rules::new(), &Rules::DEFAULT)
    }

    #[test]
    fn a_hut_inside_the_colony_on_clear_ground_is_planned() {
        let colony = colony();
        let site = Site::alone([HALL[0] + 10, HALL[1], HALL[2] + 10], [5, 5, 5]);
        let placed = plan_site(&colony, Kind::Builder, site)
            .expect("ten blocks from the town hall, inside the colony");

        assert_eq!(placed.building.kind(), Kind::Builder);
        assert_eq!(placed.building.at(), site.at);
        assert_eq!(placed.building.level(), 0, "a hut placed is not built");
        assert_eq!(placed.building.stage(), Stage::Unbuilt);
        assert_eq!(placed.footprint, site.footprint, "the box the paste will need");
    }

    #[test]
    fn a_hut_outside_the_colony_is_refused() {
        let colony = colony();
        // Chunk 6 is the town hall's; 18 is twelve chunks further, and a colony of
        // size 4 owns four.
        assert_eq!(
            plan_at(&colony, Kind::Builder, 200, [5, 5, 5]),
            Err(Refusal::OutsideColony {
                at: [HALL[0] + 200, HALL[1], HALL[2] + 200]
            })
        );
        // And the edge is exact: the box starts at block 32 (chunk 2), so 68 blocks
        // below the town hall is inside it and 69 is not.
        let edge = |x: i32| Site::alone([x, HALL[1], HALL[2]], [1, 1, 1]);
        assert!(
            plan_site(&colony, Kind::Builder, edge(HALL[0] - 68)).is_ok(),
            "the last chunk of the box is inside it"
        );
        assert_eq!(
            plan_site(&colony, Kind::Builder, edge(HALL[0] - 69)),
            Err(Refusal::OutsideColony {
                at: [HALL[0] - 69, HALL[1], HALL[2]]
            })
        );
    }

    #[test]
    fn a_tree_in_the_plot_is_refused_at_the_tree_and_the_hut_cell_is_skipped() {
        let colony = colony();
        let site = Site::alone([HALL[0] + 10, HALL[1], HALL[2] + 10], [5, 5, 5]);
        let tree = [site.at[0] + 1, site.at[1] + 2, site.at[2] + 1];
        assert_eq!(
            plan(
                &colony,
                Kind::Builder,
                &site,
                &Blocks(vec![tree]),
                &building::Rules::new(),
                &Rules::DEFAULT
            ),
            Err(Refusal::PlotBlocked { at: tree }),
            "the first blocked cell is named, so a caller can look at it"
        );
        assert!(
            plan(
                &colony,
                Kind::Builder,
                &site,
                &Blocks(vec![site.at]),
                &building::Rules::new(),
                &Rules::DEFAULT
            )
            .is_ok(),
            "the hut's own cell holds the block that was just placed"
        );
    }

    #[test]
    fn a_second_town_hall_is_refused_by_the_colonys_own_rules() {
        let colony = colony();
        assert_eq!(
            plan_at(&colony, Kind::TownHall, 10, [9, 9, 9]),
            Err(Refusal::Building(building::Refusal::TownHallAlreadyThere {
                at: HALL
            }))
        );
    }

    #[test]
    fn a_position_that_already_holds_a_building_is_refused() {
        let colony = colony();
        assert_eq!(
            plan_site(&colony, Kind::Builder, Site::alone(HALL, [1, 1, 1])),
            Err(Refusal::PositionTaken { at: HALL }),
            "the town hall's own position"
        );
    }

    #[test]
    fn a_saved_level_becomes_a_built_hut_and_only_zero_is_unbuilt() {
        let hut = |level: i32| hut_of(&colony::Building::new("minecolonies:builder", HALL, level));
        assert_eq!(hut(0).map(|hut| (hut.level(), hut.stage())), Ok((0, Stage::Unbuilt)));
        assert_eq!(hut(3).map(|hut| (hut.level(), hut.stage())), Ok((3, Stage::Built)));
        assert_eq!(
            hut(9),
            Err(Refusal::Building(building::Refusal::LevelAboveMax { level: 9 })),
            "a level outside the range is a wrong record, not a big building"
        );
        assert_eq!(
            hut(-1),
            Err(Refusal::Building(building::Refusal::LevelOutOfRange { level: -1 }))
        );
    }

    #[test]
    fn a_record_naming_an_unknown_kind_is_refused_before_the_plot() {
        let mut colony = colony();
        colony
            .buildings
            .push(colony::Building::new("minecolonies:castle", [140, 70, 100], 1));
        assert_eq!(
            plan_at(&colony, Kind::Builder, 10, [5, 5, 5]),
            Err(Refusal::UnknownKind {
                path: "minecolonies:castle".to_owned()
            }),
            "the colony's rules cannot be asked about a kind that does not exist"
        );
    }
}
