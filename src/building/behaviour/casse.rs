//! Taking a hut away: what comes back, what is destroyed, and what is left to
//! clean up.
//!
//! Java reference: `core/colony/buildings/AbstractBuilding.java:389-441`
//! (`destroy`, which runs the three steps in order), `:418-441` (`onDestroyed`),
//! `:832-871` (`requestRemoval` and `pickUp`), `:884-897` (`deconstruct`, the
//! block-by-block emptying) and `:523-527` (`canDeconstruct`), plus
//! `managers/RegisteredStructureManager.java:620-689` (`removeBuilding`) and
//! `core/blocks/huts/BlockHutBarracksTower.java:88-92` (the one kind that refuses
//! to be deconstructed).
//!
//! **Three answers, and the brief asks for exactly these three.**
//!
//! | Question | Answer |
//! | --- | --- |
//! | what comes back | [`Pickup`] — the hut block, with the colony's id and the level it had |
//! | what is destroyed | the blocks of [`Removal::plot`], the record, and every child hut, recursively |
//! | what is left to clean | [`Cleanup`], in Java's order: the claim, the tape, the modules, the citizens, the requests, the data store |
//!
//! **The order is `destroy`'s own** (`AbstractBuilding.java:389-411`): the
//! building is un-dressed first (`onDestroyed` — unclaim, tape, modules), then it
//! leaves the colony's map (`removeBuilding` — the record, the side list, the
//! citizens, the requests, the citizen ceiling), then its request data store, and
//! **then** each child is destroyed the same way. A caller that dropped the record
//! first would leave the claim behind with nothing to name it.
//!
//! **What a pickup is not.** `pickUp` (`:844-871`) is the *player* taking the hut
//! back after the builder has already emptied it; it is refused for a nested hut
//! (`:845-849`) and gives nothing when the player has no room
//! (`:855-859`). The blocks themselves are emptied by `deconstruct` (`:884-897`),
//! which runs as a work order, not as a click. Both are here, and they are two
//! different things: a nested barracks tower is destroyed and never picked up.
//!
//! **Java's loop and the port's box.** `deconstruct` walks
//! `x < higherCorner.getX()` — exclusive on the far corner — so the wall at the
//! highest x, y and z survives it. That is an off-by-one in Java, three decades
//! of blocks left standing; the port empties the whole [`Footprint`], both corners
//! included, and says so here rather than copying it. Every other rule in
//! `src/building/` treats a box as inclusive (`Footprint::contains`), and a
//! building whose far wall stayed behind would be a bug a test could not tell
//! from a schematic.

use crate::building::{Building, Footprint, Kind, Standing};
use crate::colony::{Building as Record, Colony};

use super::{Refusal, max_level};

/// The hut block's registry path for a kind: `minecolonies:blockhutbuilder`.
///
/// Java takes the block off the world (`AbstractBuilding.java:851`: `new
/// ItemStack(world.getBlockState(getPosition()).getBlock(), 1)`), so the name is
/// the **block**'s, not the building's — and the two disagree five times:
///
/// | Kind | Block | Where |
/// | --- | --- | --- |
/// | residence | `blockhutcitizen` | `BlockHutCitizen.java:21-24` |
/// | postbox | `blockpostbox` | `BlockPostBox` (`:63`) |
/// | stash | `blockstash` | `BlockStash` |
/// | simplequarry | `simplequarry` | `SimpleQuarry.java:21-24` |
/// | mediumquarry | `mediumquarry` | `MediumQuarry.java:21-24` |
/// | every other kind | `blockhut<name>` | `AbstractBlockHut.getHutName` |
///
/// The quarries are the odd ones: they return `ModBuildings.SIMPLE_QUARRY_ID`
/// itself, so their block is `minecolonies:simplequarry` and not
/// `minecolonies:blockhutsimplequarry`. A pack that renamed the block would break
/// this table, and it would break the item a player picks up with it.
pub fn hut_block(kind: Kind) -> String {
    let name = match kind {
        Kind::Residence => "blockhutcitizen",
        Kind::Postbox => "blockpostbox",
        Kind::Stash => "blockstash",
        Kind::SimpleQuarry => "simplequarry",
        Kind::MediumQuarry => "mediumquarry",
        _ => return format!("minecolonies:blockhut{}", kind.name()),
    };
    format!("minecolonies:{name}")
}

/// The two tags `pickUp` writes on the block it gives back
/// (`AbstractBuilding.java:850-853`): the colony the hut belonged to, and the
/// level it had — so a hut placed back into a colony keeps its level instead of
/// starting at one.
pub const KEY_COLONY: &str = "colonyID";
pub const KEY_LEVEL: &str = "otherLevel";

/// The kind a hut **block** names, which is the reverse of [`hut_block`] and not
/// the same function as `Kind::from_path`: a picked-up hut carries its block, not
/// its building entry, so `minecolonies:blockhutbuilder` has to resolve to
/// [`Kind::Builder`].
///
/// `None` for a block of another namespace, for a name that does not start with
/// `blockhut`, and for a `blockhut…` name no kind answers.
pub fn kind_of_block(block: &str) -> Option<Kind> {
    let path = block.strip_prefix("minecolonies:")?;
    // The five blocks that do not start with `blockhut`, in `hut_block`'s order.
    match path {
        "blockhutcitizen" => return Some(Kind::Residence),
        "blockpostbox" => return Some(Kind::Postbox),
        "blockstash" => return Some(Kind::Stash),
        "simplequarry" => return Some(Kind::SimpleQuarry),
        "mediumquarry" => return Some(Kind::MediumQuarry),
        _ => {}
    }
    Kind::from_name(path.strip_prefix("blockhut")?)
}

/// What the player gets back, and the two facts stamped on it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Pickup {
    /// [`hut_block`].
    pub item: String,
    /// `TAG_COLONY_ID` (`:851`).
    pub colony: i32,
    /// `TAG_OTHER_LEVEL` (`:852`).
    pub level: i32,
}

/// The ring of blocks the construction tape occupies, which is what
/// `ConstructionTapeHelper.removeConstructionTape` walks
/// (`core/entity/ai/workers/util/ConstructionTapeHelper.java:66-70` and its two
/// loops).
///
/// The box is the building's corners **grown by one in x and z** and by five
/// below and one above in y (`:66-71`): the tape stands outside the schematic's
/// footprint and reaches down a slope. It is a ring, not a volume — the middle is
/// the building — but a ring is exactly what a removal has to sweep, and its two
/// corners are what a caller needs to name the sweep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Tape {
    pub low: [i32; 3],
    pub high: [i32; 3],
}

impl Tape {
    /// The tape of a plot: one block out in x and z, five blocks down in y, one
    /// block up.
    pub fn around(plot: Footprint) -> Tape {
        let low = plot.low();
        let high = plot.high();
        Tape {
            low: [low[0] - 1, low[1].min(high[1]) - 5, low[2] - 1],
            high: [high[0] + 1, high[1].max(low[1]) + 1, high[2] + 1],
        }
    }

    /// The number of cells the tape's ring passes through: the perimeter of the
    /// box, in blocks. Two sides of every length plus the four corners, which is
    /// what the four loops of the helper walk (with the corners counted once
    /// each, since the loops are on the two axes and the four corners are added
    /// separately).
    pub fn ring(&self) -> i64 {
        let width = i64::from(self.high[0] - self.low[0]);
        let depth = i64::from(self.high[2] - self.low[2]);
        if width <= 0 || depth <= 0 {
            return 0;
        }
        2 * (width + depth) + 4
    }
}

/// One thing a removal has to do after the blocks are gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the steps of destroy() in Java's order, closed on purpose
pub enum Cleanup {
    /// `claimBuildingChunks(colony, false, …)` — the chunks come back
    /// (`AbstractBuilding.java:429`).
    UnclaimChunks { at: [i32; 3], radius: i32 },
    /// The construction tape is swept (`:430`).
    RemoveTape(Tape),
    /// Every module's `onDestroyed` (`:432`): the assigned citizens are released
    /// (`AbstractAssignedCitizenModule.java:80-87`) and a tavern's visitors go
    /// home (`TavernBuildingModule.java:306-312`).
    ReleaseModules,
    /// The record leaves the colony's list (`RegisteredStructureManager.java:625-644`).
    DropRecord { at: [i32; 3] },
    /// The side list it was in is cleared (`:646-656`).
    DropSideList { at: [i32; 3] },
    /// Each citizen is told the hut is gone — its job, its bed and its requests
    /// (`:662-666`).
    FreeCitizens,
    /// Every request the hut had is cancelled (`:666`, `:675-676`).
    CancelRequests,
    /// The colony's citizen ceiling is counted again (`:679`).
    RecountCitizens,
    /// The hut's request data store is dropped (`AbstractBuilding.java:401`).
    DropDataStore { at: [i32; 3] },
    /// A child hut is destroyed the same way, recursively (`:405-411`).
    DestroyChild { at: [i32; 3] },
}

/// A removal, decided and ordered: what the caller has to do, and what comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Removal {
    pub at: [i32; 3],
    pub kind: Kind,
    /// The box the schematic covers, whose blocks are emptied. Java's
    /// `deconstruct` walks it; see the module doc for the off-by-one it keeps.
    pub plot: Footprint,
    /// The hut block a player gets back. Always one: Java's `pickUp` gives the
    /// block unless the hut is nested or the player has no room, and neither is a
    /// fact of the kind — see [`pickup`] for the first and [`Refusal::InventoryFull`]
    /// for the second.
    pub pickup: Pickup,
    /// The huts nested in this one, which go with it (`AbstractBuilding.java:405-411`).
    pub children: Vec<[i32; 3]>,
    /// The steps after the blocks, in `destroy`'s order.
    pub cleanup: Vec<Cleanup>,
}

impl Removal {
    /// The blocks this removal empties, both corners included.
    pub fn blocks(&self) -> u32 {
        let size = self.plot.size();
        size[0] * size[1] * size[2]
    }

    /// The steps as names, which is what a test compares when it cares about the
    /// order and not the arguments.
    pub fn order(&self) -> Vec<&'static str> {
        self.cleanup
            .iter()
            .map(|step| match step {
                Cleanup::UnclaimChunks { .. } => "unclaim_chunks",
                Cleanup::RemoveTape(_) => "remove_tape",
                Cleanup::ReleaseModules => "release_modules",
                Cleanup::DropRecord { .. } => "drop_record",
                Cleanup::DropSideList { .. } => "drop_side_list",
                Cleanup::FreeCitizens => "free_citizens",
                Cleanup::CancelRequests => "cancel_requests",
                Cleanup::RecountCitizens => "recount_citizens",
                Cleanup::DropDataStore { .. } => "drop_data_store",
                Cleanup::DestroyChild { .. } => "destroy_child",
            })
            .collect()
    }
}

/// Whether a hut's contents are gathered by a courier before it goes
/// (`AbstractBuilding.java:767-773`): `canBeGathered` is `true` by default, and
/// **three kinds say no** — the town hall (`BuildingTownHall.java:203-206`), the
/// warehouse (`BuildingWareHouse.java:183-186`) and the postbox
/// (`PostBox.java:77-81`).
///
/// The lumberjack and the farmer override it to `true` with a comment explaining
/// that a crafter is only gatherable while it is producing and these two both
/// gather and craft (`BuildingLumberjack.java:116-127`, `BuildingFarmer.java:113-119`)
/// — the same answer as the default, and worth knowing they wrote it down.
///
/// This is **not** the same question as "does the hut block come back": that is
/// [`Pickup`], and it is answered by the parent link, not by the kind.
pub fn be_gathered(kind: Kind) -> bool {
    !matches!(kind, Kind::TownHall | Kind::Warehouse | Kind::Postbox)
}

/// Whether a kind accepts being deconstructed. `canDeconstruct` is `true` for
/// every building except a barracks tower (`BuildingBarracksTower.java:88-92`),
/// which goes down with its barracks and never on its own.
pub fn can_deconstruct(kind: Kind) -> bool {
    kind != Kind::BarracksTower
}

/// Plan a removal: the kind's two refusals first, then the ordered steps.
///
/// `plot` is the box the schematic covers — `founding::Placed::footprint` for a hut
/// the port placed, or whatever the caller read off the blueprint. The building
/// record does not carry it: the box comes from the *schematic*, and
/// `src/building/instance.rs` says why a record holds a position and not a box.
///
/// The refusals, in Java's order:
///
/// 1. nothing stands there → [`Refusal::NoBuilding`]. Java's `removeBuilding`
///    silently does nothing when the map does not hold the position (`:620-624`);
///    the port names it, because a removal that removed nothing is a bug.
/// 2. the kind refuses to be deconstructed → [`Refusal::CannotDeconstruct`]
///    (`AbstractBuilding.java:523-527`, reached from `requestWorkOrder` at `:455-460`).
pub fn plan(standing: &Standing, at: [i32; 3], plot: Footprint) -> Result<Removal, Refusal> {
    let Some(building) = standing.at(at) else {
        return Err(Refusal::NoBuilding { at });
    };
    let kind = building.kind();
    if !can_deconstruct(kind) {
        return Err(Refusal::CannotDeconstruct { kind });
    }

    // The children, direct and then their own: `destroy` calls each of them, and
    // each of those calls the same method, so the walk is the tree.
    let mut children = Vec::new();
    let mut pending = vec![at];
    while let Some(parent) = pending.pop() {
        for other in standing.iter() {
            if other.parent() == Some(parent) && !children.contains(&other.at()) {
                children.push(other.at());
                pending.push(other.at());
            }
        }
    }

    let level = building.level();
    let mut cleanup = Vec::with_capacity(9 + children.len());
    // 1. The claim goes back (`AbstractBuilding.java:429`).
    cleanup.push(Cleanup::UnclaimChunks {
        at,
        radius: super::claim_radius(kind, level),
    });
    // 2. The tape is swept (`:430`).
    cleanup.push(Cleanup::RemoveTape(Tape::around(plot)));
    // 3. The modules let their citizens go (`:432`).
    cleanup.push(Cleanup::ReleaseModules);
    // 4-5. The record and its side list (`RegisteredStructureManager.java:625-656`).
    cleanup.push(Cleanup::DropRecord { at });
    if super::pose::SideList::of(kind).is_some() {
        cleanup.push(Cleanup::DropSideList { at });
    }
    // 6-8. The citizens, the requests, the ceiling (`:662-679`).
    cleanup.push(Cleanup::FreeCitizens);
    cleanup.push(Cleanup::CancelRequests);
    cleanup.push(Cleanup::RecountCitizens);
    // 9. The data store (`AbstractBuilding.java:401`).
    cleanup.push(Cleanup::DropDataStore { at });
    // 10. Every child, in the order they were found (`:405-411`).
    for child in &children {
        cleanup.push(Cleanup::DestroyChild { at: *child });
    }

    Ok(Removal {
        at,
        kind,
        plot,
        pickup: Pickup {
            item: hut_block(kind),
            // The colony's id is the caller's fact, not the building's: the record
            // has no id of its own and a building cannot see its colony from here.
            colony: 0,
            level: level.max(0),
        },
        children,
        cleanup,
    })
}

/// The pickup a player would get, and the one rule that refuses it: a hut nested
/// in another one cannot be taken back (`AbstractBuilding.java:845-849`,
/// `WARNING_BUILDING_PICKUP_DENIED`).
///
/// The other refusal, "the player has no room", needs an inventory and therefore a
/// caller; it is [`Refusal::InventoryFull`] and this function never raises it —
/// `MANQUANT.md` carries the gap.
pub fn pickup(standing: &Standing, at: [i32; 3], colony: i32) -> Result<Pickup, Refusal> {
    let Some(building) = standing.at(at) else {
        return Err(Refusal::NoBuilding { at });
    };
    if building.parent().is_some() {
        return Err(Refusal::NestedPickup { at });
    }
    Ok(Pickup {
        item: hut_block(building.kind()),
        colony,
        level: building.level().max(0),
    })
}

/// Apply a removal to the colony record: the hut and every child leave the list.
///
/// Returns how many records went. The rest of the journal is world state, which is
/// why [`Removal::cleanup`] exists: a caller that only did this would leave the
/// chunks claimed by a hut no longer in the colony.
pub fn demolish(colony: &mut Colony, removal: &Removal) -> Result<usize, Refusal> {
    if colony.building_at(removal.at).is_none() {
        return Err(Refusal::NoBuilding { at: removal.at });
    }
    let before = colony.buildings.len();
    colony
        .buildings
        .retain(|record| record.at != removal.at && !removal.children.contains(&record.at));
    Ok(before - colony.buildings.len())
}

/// The level a picked-up hut comes back at, which is the level it had — capped by
/// its kind's ceiling, because a `/data` edited record is not a level a hut can be
/// placed at (`AbstractBuilding.java:852` writes what it finds).
pub fn restored_level(pickup: &Pickup, kind: Kind) -> i32 {
    pickup.level.clamp(0, max_level(kind))
}

/// The record a picked-up hut becomes when it is placed again, or `None` when the
/// block names no kind this port knows.
pub fn restore(pickup: &Pickup, at: [i32; 3]) -> Option<Record> {
    let kind = kind_of_block(&pickup.item)?;
    Some(Record::new(&kind.path(), at, restored_level(pickup, kind)))
}

/// Every building the colony holds that the removed one owned, directly. Java's
/// `getChildren` (`AbstractSchematicProvider.java:326-333`) — a filter over the
/// colony, which is why it needs the whole [`Standing`] and not one record.
pub fn children_of(standing: &Standing, at: [i32; 3]) -> Vec<[i32; 3]> {
    standing
        .iter()
        .filter(|building| building.parent() == Some(at))
        .map(Building::at)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Refusal as BuildingRefusal;

    const HALL: [i32; 3] = [0, 70, 0];
    const BARRACKS: [i32; 3] = [40, 70, 0];
    const TOWER_A: [i32; 3] = [44, 70, 0];
    const TOWER_B: [i32; 3] = [48, 70, 0];

    fn standing() -> Standing {
        let mut standing = Standing::new();
        let mut town_hall = Building::new(Kind::TownHall, HALL);
        town_hall.complete(5).expect("a town hall");
        standing.insert(town_hall);
        let mut barracks = Building::new(Kind::Barracks, BARRACKS);
        barracks.complete(3).expect("a barracks");
        standing.insert(barracks);
        for at in [TOWER_A, TOWER_B] {
            let mut tower = Building::new(Kind::BarracksTower, at);
            tower.complete(2).expect("a tower");
            tower.set_parent(Some(BARRACKS)).expect("inside the barracks");
            standing.insert(tower);
        }
        standing
    }

    fn plot(at: [i32; 3]) -> Footprint {
        Footprint::at_size(at, [5, 6, 5])
    }

    #[test]
    fn the_hut_block_is_the_buildings_name_except_five_times() {
        assert_eq!(hut_block(Kind::Builder), "minecolonies:blockhutbuilder");
        assert_eq!(hut_block(Kind::TownHall), "minecolonies:blockhuttownhall");
        assert_eq!(hut_block(Kind::BarracksTower), "minecolonies:blockhutbarrackstower");
        assert_eq!(hut_block(Kind::Warehouse), "minecolonies:blockhutwarehouse");
        // The five that disagree with the pattern.
        assert_eq!(hut_block(Kind::Residence), "minecolonies:blockhutcitizen");
        assert_eq!(hut_block(Kind::Postbox), "minecolonies:blockpostbox");
        assert_eq!(hut_block(Kind::Stash), "minecolonies:blockstash");
        assert_eq!(hut_block(Kind::SimpleQuarry), "minecolonies:simplequarry");
        assert_eq!(hut_block(Kind::MediumQuarry), "minecolonies:mediumquarry");
        // Every kind answers something, and it is a registry path.
        for kind in Kind::ALL {
            let block = hut_block(*kind);
            assert!(block.starts_with("minecolonies:"), "{kind}: {block}");
            assert!(block.len() > "minecolonies:".len());
        }
    }

    #[test]
    fn a_pickup_gives_the_block_its_colony_and_its_level() {
        let standing = standing();
        let pickup = pickup(&standing, BARRACKS, 7).expect("a barracks has no parent");
        assert_eq!(pickup.item, "minecolonies:blockhutbarracks");
        assert_eq!(pickup.colony, 7);
        assert_eq!(pickup.level, 3);
        // The block, back where it came from, is a level-3 barracks record.
        let record = restore(&pickup, BARRACKS).expect("a kind this port knows");
        assert_eq!(record.kind, "minecolonies:barracks");
        assert_eq!(record.level, 3);
        // A level a hand-edited tag could name is capped by the kind's ceiling.
        let edited = Pickup {
            item: "minecolonies:blockhutgatehouse".into(),
            colony: 7,
            level: 9,
        };
        assert_eq!(restored_level(&edited, Kind::GateHouse), 3);
        // And the stash always comes back at zero: it never has a level.
        let stash = Pickup {
            item: "minecolonies:blockstash".into(),
            colony: 7,
            level: 4,
        };
        assert_eq!(restored_level(&stash, Kind::Stash), 0);
        assert_eq!(restore(&stash, HALL).map(|record| record.level), Some(0));
        assert_eq!(
            restore(
                &Pickup {
                    item: "minecolonies:castle".into(),
                    colony: 7,
                    level: 1
                },
                HALL
            ),
            None,
            "a block this port has no kind for is not a hut"
        );
        assert_eq!(
            restore(
                &Pickup {
                    item: "minecolonies:blockhutbuilder".into(),
                    colony: 7,
                    level: 2
                },
                HALL
            )
            .map(|record| (record.kind, record.level)),
            Some(("minecolonies:builder".to_owned(), 2)),
            "the block comes back as the kind its entry is registered under"
        );
    }

    #[test]
    fn every_hut_block_names_its_own_kind_back() {
        for kind in Kind::ALL {
            let block = hut_block(*kind);
            assert_eq!(
                kind_of_block(&block),
                Some(*kind),
                "{kind} → {block} → ?"
            );
        }
        assert_eq!(kind_of_block("minecolonies:blockhutcitizen"), Some(Kind::Residence));
        assert_eq!(kind_of_block("minecolonies:simplequarry"), Some(Kind::SimpleQuarry));
        assert_eq!(kind_of_block("minecraft:blockhutbuilder"), None, "another namespace");
        assert_eq!(kind_of_block("minecolonies:blockhutbuilderx"), None);
        assert_eq!(kind_of_block("minecolonies:builder"), None, "a building entry, not a block");
        assert_eq!(kind_of_block("minecolonies:blockhut"), None);
        assert_eq!(kind_of_block("blockhutbuilder"), None, "the namespace is required");
    }

    #[test]
    fn a_hut_nested_in_another_cannot_be_picked_up() {
        let standing = standing();
        assert_eq!(
            pickup(&standing, TOWER_A, 7),
            Err(Refusal::NestedPickup { at: TOWER_A }),
            "AbstractBuilding.java:845-849"
        );
        assert_eq!(
            pickup(&standing, [1, 2, 3], 7),
            Err(Refusal::NoBuilding { at: [1, 2, 3] })
        );
    }

    #[test]
    fn only_a_barracks_tower_refuses_to_be_deconstructed() {
        let refusing: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| !can_deconstruct(*kind))
            .collect();
        assert_eq!(refusing, vec![Kind::BarracksTower], "BuildingBarracksTower.java:88-92");
        let standing = standing();
        assert_eq!(
            plan(&standing, TOWER_A, plot(TOWER_A)),
            Err(Refusal::CannotDeconstruct {
                kind: Kind::BarracksTower
            })
        );
        assert_eq!(
            plan(&standing, [9, 9, 9], plot([9, 9, 9])),
            Err(Refusal::NoBuilding { at: [9, 9, 9] })
        );
    }

    #[test]
    fn a_removal_un_dresses_the_hut_then_drops_it_then_kills_its_children() {
        let standing = standing();
        let removal = plan(&standing, BARRACKS, plot(BARRACKS)).expect("a barracks may go");
        assert_eq!(
            removal.order(),
            vec![
                "unclaim_chunks",
                "remove_tape",
                "release_modules",
                "drop_record",
                "free_citizens",
                "cancel_requests",
                "recount_citizens",
                "drop_data_store",
                "destroy_child",
                "destroy_child",
            ],
            "AbstractBuilding.java:389-411 then RegisteredStructureManager.java:662-679"
        );
        assert_eq!(
            removal.children,
            vec![TOWER_A, TOWER_B],
            "the barracks' two towers go with it"
        );
        assert!(matches!(
            removal.cleanup[0],
            Cleanup::UnclaimChunks { radius, .. } if radius == 2
        ));
        assert!(matches!(
            removal.cleanup[8],
            Cleanup::DestroyChild { at } if at == TOWER_A
        ));
        // A warehouse declares its side list; a builder does not.
        let mut warehouse = Standing::new();
        let mut hut = Building::new(Kind::Warehouse, HALL);
        hut.complete(2).expect("a warehouse");
        warehouse.insert(hut);
        let removal = plan(&warehouse, HALL, plot(HALL)).expect("a warehouse may go");
        assert!(removal.order().contains(&"drop_side_list"));
        let mut builder = Standing::new();
        let mut hut = Building::new(Kind::Builder, HALL);
        hut.complete(2).expect("a builder");
        builder.insert(hut);
        let removal = plan(&builder, HALL, plot(HALL)).expect("a builder may go");
        assert!(!removal.order().contains(&"drop_side_list"), "a builder is in no list");
    }

    #[test]
    fn the_blocks_a_removal_empties_are_the_whole_plot_both_corners_in() {
        let standing = standing();
        let removal = plan(&standing, BARRACKS, plot(BARRACKS)).expect("a barracks");
        assert_eq!(removal.plot, Footprint::between(BARRACKS, [44, 75, 4]));
        assert_eq!(removal.blocks(), 5 * 6 * 5, "every cell, no off-by-one");
        assert_eq!(removal.pickup.item, "minecolonies:blockhutbarracks");
        assert_eq!(removal.pickup.level, 3);
    }

    #[test]
    fn the_tape_is_a_ring_one_block_out_and_five_blocks_down() {
        // Building 5x6x5 at [40,70,0]: corners [40,70,0] and [44,75,4].
        let tape = Tape::around(Footprint::between(BARRACKS, [44, 75, 4]));
        assert_eq!(tape.low, [39, 65, -1], "one out in x and z, five down in y");
        assert_eq!(tape.high, [45, 76, 5], "one out and one up");
        assert_eq!(tape.ring(), 2 * (6 + 6) + 4, "the perimeter of a 7x7 ring");
        // A plot one block wide has a degenerate ring of nothing: there is no
        // room for a tape around a single block.
        let single = Tape::around(Footprint::of_size([1, 1, 1]));
        assert_eq!(single.low, [-1, -5, -1]);
        assert_eq!(single.high, [1, 1, 1]);
        assert!(single.ring() > 0, "two blocks wide after the one-block margin");
    }

    #[test]
    fn three_kinds_are_not_gathered_before_they_go() {
        let ungathered: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| !be_gathered(*kind))
            .collect();
        assert_eq!(
            ungathered,
            vec![Kind::Postbox, Kind::TownHall, Kind::Warehouse],
            "the hall, the warehouse and the postbox keep their contents"
        );
        assert!(be_gathered(Kind::Builder));
        assert!(be_gathered(Kind::Lumberjack), "a lumberjack is always gatherable");
    }

    #[test]
    fn demolish_takes_the_record_and_the_children_out_of_the_colony() {
        let mut colony = Colony::new(1, "Bridgewater", "minecraft:overworld", HALL, 7);
        colony
            .buildings
            .push(Record::new("minecolonies:townhall", HALL, 5));
        colony
            .buildings
            .push(Record::new("minecolonies:barracks", BARRACKS, 3));
        for at in [TOWER_A, TOWER_B] {
            colony
                .buildings
                .push(Record::new("minecolonies:barrackstower", at, 2));
        }
        colony
            .buildings
            .push(Record::new("minecolonies:builder", [60, 70, 60], 2));

        // The colony record has no parent links, so the removal is planned on the
        // `building` model, which does — and applied to the colony, which keys on
        // the position.
        let removal = plan(&standing(), BARRACKS, plot(BARRACKS)).expect("a barracks");
        assert_eq!(demolish(&mut colony, &removal), Ok(3), "the barracks and its two towers");
        assert!(colony.building_at(BARRACKS).is_none());
        assert!(colony.building_at(TOWER_A).is_none());
        assert!(colony.building_at(TOWER_B).is_none());
        assert!(colony.building_at(HALL).is_some(), "the town hall stays");
        assert_eq!(colony.buildings.len(), 2, "the town hall and the builder");

        // And a second demolition of the same hut found nothing to remove.
        assert_eq!(
            demolish(&mut colony, &removal),
            Err(Refusal::NoBuilding { at: BARRACKS })
        );
        assert_eq!(colony.buildings.len(), 2, "a refusal changes nothing");
    }

    #[test]
    fn a_child_is_found_by_its_parent_link_and_not_by_its_distance() {
        let standing = standing();
        assert_eq!(children_of(&standing, BARRACKS), vec![TOWER_A, TOWER_B]);
        assert!(children_of(&standing, HALL).is_empty());
        assert!(children_of(&standing, TOWER_A).is_empty(), "nesting stops at one");
        // A building at the same coordinates but with no parent link is nobody's
        // child, however close it stands.
        let mut loose = Standing::new();
        let mut barracks = Building::new(Kind::Barracks, BARRACKS);
        barracks.complete(3).expect("a barracks");
        loose.insert(barracks);
        let mut tower = Building::new(Kind::BarracksTower, [41, 70, 0]);
        tower.complete(1).expect("a tower");
        loose.insert(tower);
        assert!(children_of(&loose, BARRACKS).is_empty());
        // A barracks with no tower still plans cleanly: the walk found nothing.
        let removal = plan(&loose, BARRACKS, plot(BARRACKS)).expect("a barracks may go");
        assert!(removal.children.is_empty());
        assert_eq!(removal.order().len(), 8, "the eight steps and no child to destroy");
    }

    #[test]
    fn a_removal_does_not_touch_a_hut_that_is_not_there() {
        let standing = standing();
        assert_eq!(
            plan(&standing, [200, 70, 200], plot([200, 70, 200])),
            Err(Refusal::NoBuilding {
                at: [200, 70, 200]
            })
        );
        // And the building model still refuses a level of six, which is what a
        // removal has to be able to say about the hut it is about to unmake.
        let mut hut = Building::new(Kind::Builder, HALL);
        assert_eq!(
            hut.set_level(6),
            Err(BuildingRefusal::LevelAboveMax { level: 6 })
        );
    }
}
