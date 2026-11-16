//! What *happens* to a building: it is put down, it grows, it is taken away, and
//! the modules it carries decide who may work there.
//!
//! Java reference: `core/colony/buildings/AbstractBuilding.java` (2 150 lines),
//! `AbstractSchematicProvider.java` (the level, the parent, the two flags) and
//! `core/colony/managers/RegisteredStructureManager.java` (the map a colony keeps,
//! `addNewBuilding` at `:556-619`, `removeBuilding` at `:620-689`), plus the two
//! concrete buildings the brief names — `workerbuildings/BuildingBuilder.java` and
//! `workerbuildings/BuildingFarmer.java` — and the module table
//! `core/colony/buildings/modules/BuildingModules.java` with the initialiser that
//! hands each kind its modules (`apiimp/initializer/ModBuildingsInitializer.java`).
//!
//! | File | Question |
//! | --- | --- |
//! | `pose` | what is checked before a hut stands, and in what order it is written |
//! | `amelioration` | what the next level changes, keeps, and recomputes |
//! | `casse` | what comes back, what is destroyed, what is left to clean |
//! | `modules` | the 157 module keys, and which of the 54 kinds carries which |
//!
//! **`src/building/` already says what a hut *is*** — its kind, its cost, what it
//! demands to be placed, how far it has got. This module is the layer above: the
//! three transitions, each of which is an ordered list of writes, so a test can
//! prove what happened and in which order.
//!
//! **The order is the deliverable, again.** [`pose::pose`] runs every check before
//! it makes one write, and takes the colony as `&mut` only so the one write it
//! makes has somewhere to go — which is why a refusal leaves the record
//! untouched. A test proves it by comparing the colony with a clone taken before
//! the call, not by trusting a comment.
//!
//! **What this layer cannot write.** A hut's chunks, its construction tape, its
//! request resolvers and the colony's chunk extent are world state, not colony
//! state: they come back as a [`pose::Journal`] the caller applies. The one write
//! this crate can make is the colony's own building list, and it is the one it
//! makes. `MANQUANT.md` lists the rest and what each gap costs; every refusal
//! below names a rule, never a `false`.
//!
//! **Two rules here are this port's own**, and both say so where they are
//! written: [`Refusal::ColonyLevelTooLow`] (Java gates a hut's *growth* on
//! research, not on the colony's level) and [`pose::pose`]'s habit of writing
//! nothing at all before a refusal, which Java achieves with a `return` on a
//! message. Everything else quotes a line of Java in the doc comment above it.

pub mod amelioration;
pub mod casse;
pub mod modules;
pub mod pose;

use crate::building::Kind;
use crate::founding;

pub use amelioration::{
    CONSERVED, Census, Kept, RECALCULATED, Recalculated, Research, Step, apply as apply_upgrade,
    census, check as check_upgrade,
};
pub use casse::{
    Cleanup, Pickup, Removal, Tape, be_gathered, can_deconstruct, children_of, demolish, hut_block,
    kind_of_block, pickup as plan_pickup, plan as plan_removal, restore, restored_level,
};
pub use modules::{Catalogue, Module, Role, attached_to, capacity_at, has, module, modules_of, seats};
pub use pose::{Gates, Journal, Plan, Write, placed, pose};

/// The number of blocks on a chunk's side, `Constants.java:55`, re-exported from
/// the founding rules so the claim arithmetic has one source.
pub const BLOCKS_PER_CHUNK: i32 = founding::BLOCKS_PER_CHUNK;

/// The top level a kind may reach, which is **not** always five.
///
/// Java's `getMaxBuildingLevel` is overridden eleven times and the overrides
/// matter:
///
/// | Kind | Max | Where |
/// | --- | ---: | --- |
/// | gatehouse | 3 | `BuildingGateHouse.java:34`, `:55` |
/// | postbox | 1 | `PostBox.java:63-68` (forced, see below) |
/// | stash | 0 | `Stash.java:52-56` |
/// | every other kind | 5 | `AbstractBuilding.java:1154` → `BuildingConstants.java:11` |
///
/// `building::MAX_BUILDING_LEVEL` is that fifth constant, and
/// `building::Refusal::AlreadyMaxLevel` can only ever mean "five". A gatehouse at
/// three is refused here instead, with its own name, which is what Java's
/// `getBuildingLevel() < getMaxBuildingLevel()` test does
/// (`AbstractBuilding.java:807`).
///
/// The postbox is the odd one: it answers **1 for its maximum and 1 for its
/// current level** (`PostBox.java:63-75`), so it can never be built *or* upgraded
/// — its level is a fiction that exists to enable the minimum-stock module.
pub fn max_level(kind: Kind) -> i32 {
    match kind {
        Kind::GateHouse => 3,
        Kind::Postbox => 1,
        Kind::Stash => 0,
        _ => crate::building::MAX_BUILDING_LEVEL,
    }
}

/// The chunks a building holds, as a radius in chunks around its own chunk.
///
/// Java's `getClaimRadius(int newLevel)` (`AbstractBuilding.java:654-670`) and
/// its six overrides. The default is the table every kind shares:
///
/// | Level | Radius | Where |
/// | ---: | ---: | --- |
/// | 0 | 0 | `AbstractBuilding.java:668` (the `default` arm) |
/// | 1..=3 | 1 | `:658-661` |
/// | 4..=5 | 2 | `:662-665` |
///
/// The overrides are not decoration: a level-5 town hall claims **five** chunks
/// of radius (`BuildingTownHall.java:182-202`), a guard tower two at level 1 and
/// five at level 5 (`BuildingGuardTower.java:52-66`), a gatehouse at most two and
/// nothing at level 4 or above (`BuildingGateHouse.java:75-83`), and a barracks
/// tower claims nothing at all because its barracks claims for it
/// (`BuildingBarracksTower.java:94-97`).
///
/// The barracks is the one kind this function cannot answer for: its radius is 3
/// when every tower it holds is level 4 or more and 2 otherwise
/// (`BuildingBarracks.java:162-183`), which is a fact about *other* buildings.
/// [`claim_radius_with_towers`] takes that answer as an argument.
pub fn claim_radius(kind: Kind, level: i32) -> i32 {
    match kind {
        // The conservative arm, and only for a hut that stands: `:164-167`
        // answers 0 at level 0 before it looks at any tower.
        Kind::Barracks => {
            if level <= 0 {
                0
            } else {
                2
            }
        }
        Kind::BarracksTower => 0,
        Kind::TownHall => match level {
            0 => 0,
            1 | 2 => 1,
            3 => 2,
            4 => 3,
            5 => 5,
            _ => 0,
        },
        Kind::GuardTower => match level {
            1 => 2,
            2 | 3 => 3,
            4 => 4,
            5 => 5,
            _ => 0,
        },
        Kind::GateHouse => match level {
            1 | 2 => 1,
            3 => 2,
            _ => 0,
        },
        _ => match level {
            1..=3 => 1,
            4 | 5 => 2,
            _ => 0,
        },
    }
}

/// The barracks' radius, which depends on the towers it holds: 3 when every
/// tower is level 4 or more, 2 otherwise (`BuildingBarracks.java:162-183`).
///
/// `tower_levels` is what the colony says the barracks' towers are; an empty list
/// answers 3, as Java does (the loop finds nothing below 4).
pub fn claim_radius_with_towers(level: i32, tower_levels: &[i32]) -> i32 {
    if level <= 0 {
        return 0;
    }
    if tower_levels.iter().any(|level| *level < 4) {
        2
    } else {
        3
    }
}

/// The chunk-aligned box a building's claim covers:
/// `BlockPosUtil.getChunkAlignedBB` (`api/util/BlockPosUtil.java:591-600`).
///
/// The position is rounded **down** to its chunk corner, then the box grows by
/// `radius * 16` on every side and by 15 to reach the far edge of the last
/// chunk. Note what it does with the height: `y` is chunk-aligned too
/// (`y1 & ~15`), so a claim is a box in three dimensions and a hut at y=70 claims
/// from y=64-…, not from the world floor. The port keeps that, because it is what
/// the guard-coverage test compares against (`RegisteredStructureManager.java:701`).
pub fn claim_box(at: [i32; 3], radius: i32) -> crate::building::Footprint {
    let blocks = radius * BLOCKS_PER_CHUNK;
    let low = [
        (at[0] & !15) - blocks,
        (at[1] & !15) - blocks,
        (at[2] & !15) - blocks,
    ];
    let high = [
        (at[0] & !15) + blocks + 15,
        (at[1] & !15) + blocks + 15,
        (at[2] & !15) + blocks + 15,
    ];
    crate::building::Footprint::between(low, high)
}

/// The chunk a position falls in, floors included: Java's `pos.getX() >> 4`
/// (`ChunkDataHelper.java:236-237`), which agrees with `div_euclid` and not with
/// `/` on negative coordinates.
pub fn chunk_of(at: [i32; 3]) -> [i32; 2] {
    [
        at[0].div_euclid(BLOCKS_PER_CHUNK),
        at[2].div_euclid(BLOCKS_PER_CHUNK),
    ]
}

/// Why a building operation was refused, named.
///
/// The three transitions have their own refusals and share this enum, because a
/// caller that got a refusal should not have to know which layer produced it. Two
/// variants **wrap** the refusals of the layers below ([`crate::building::Refusal`]
/// and [`founding::Refusal`]) rather than re-wording them: melting "the town hall
/// already stands there, at x/y/z" into one string loses the position, which is
/// the only part a player can act on.
///
/// A variant is **not** a `lang` key: those live in `assets/minecolonies/lang/`,
/// which this module does not own.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The colony's own placement rules refused it — one town hall, the hut in
    /// its own box, the box big enough, the parent it asks for
    /// (`api/blocks/AbstractBlockHut.java:305-338`), or the founding checks
    /// around them (`src/founding/placement.rs`).
    Placement(founding::Refusal),
    /// The building model's own rules refused it: the level out of range, the
    /// parent ahead of it, the module above it (`src/building/mod.rs`). Kept
    /// apart from [`Self::Placement`] because the two come from two layers and a
    /// log line that said "placement refused" for a level of six would be wrong.
    Rules(crate::building::Refusal),
    /// A second tavern. `BlockHutTavern.canPlaceAt` walks every building looking
    /// for one with the `tavern_visitor` module and refuses, naming where it is
    /// (`core/blocks/huts/BlockHutTavern.java:50-64`, `WARNING_DUPLICATE_TAVERN`).
    TavernTaken { at: [i32; 3] },
    /// The pack's colony-level gate is higher than the colony's level.
    ///
    /// **A rule of this port.** Java has no such gate on placing a hut: the only
    /// colony-wide test is research, and it is asked when a building is
    /// *upgraded*, not when it is put down (`AbstractBuilding.java:786-799`).
    /// The port needs the gate because a pack that ships a hall-only hut has
    /// nowhere else to say so, and it is `0` unless the data fills it, which is
    /// Java's behaviour exactly.
    ColonyLevelTooLow { level: i32, required: i32 },
    /// The kind's own ceiling, which is not always five: a gatehouse stops at 3,
    /// a postbox at 1, a stash at 0. See [`max_level`].
    AtKindMaxLevel { level: i32, max: i32 },
    /// The research that unlocks this hut has not been done
    /// (`AbstractBuilding.java:788-792`, `WARNING_BUILDING_REQUIRES_RESEARCH_UNLOCK`).
    ResearchLocked,
    /// The research that allows this *level* has not been done: Java compares the
    /// effect's strength to the current level (`:794-798`).
    ResearchBelow { strength: i32, level: i32 },
    /// A hut placed and not built takes a BUILD order only if the parent it nests
    /// in is built: `getBuildingLevel() == 0 && parentBuilding.getBuildingLevel() > 0`
    /// (`AbstractBuilding.java:803-805`).
    ParentNotBuilt { at: [i32; 3] },
    /// A hut nested in another one cannot be picked up
    /// (`AbstractBuilding.java:845-849`, `WARNING_BUILDING_PICKUP_DENIED`).
    NestedPickup { at: [i32; 3] },
    /// This kind refuses to be deconstructed: a barracks tower is taken down with
    /// its barracks (`BuildingBarracksTower.java:88-92`).
    CannotDeconstruct { kind: Kind },
    /// Nowhere to put the hut block the pickup gives back
    /// (`AbstractBuilding.java:855-859`, `WARNING_BUILDING_PICKUP_PLAYER_INVENTORY_FULL`).
    InventoryFull { item: String },
    /// Nothing stands at that position, so there is nothing to take away.
    NoBuilding { at: [i32; 3] },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Placement(refusal) => refusal.name(),
            Self::Rules(refusal) => refusal.name(),
            Self::TavernTaken { .. } => "building.behaviour.refusal.tavern_taken",
            Self::ColonyLevelTooLow { .. } => "building.behaviour.refusal.colony_level_too_low",
            Self::AtKindMaxLevel { .. } => "building.behaviour.refusal.at_kind_max_level",
            Self::ResearchLocked => "building.behaviour.refusal.research_locked",
            Self::ResearchBelow { .. } => "building.behaviour.refusal.research_below",
            Self::ParentNotBuilt { .. } => "building.behaviour.refusal.parent_not_built",
            Self::NestedPickup { .. } => "building.behaviour.refusal.nested_pickup",
            Self::CannotDeconstruct { .. } => "building.behaviour.refusal.cannot_deconstruct",
            Self::InventoryFull { .. } => "building.behaviour.refusal.inventory_full",
            Self::NoBuilding { .. } => "building.behaviour.refusal.no_building",
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
    use crate::building::Kind;

    #[test]
    fn every_refusal_has_its_own_name() {
        let at = [1, 2, 3];
        let all = [
            Refusal::Placement(founding::Refusal::OutsideColony { at }),
            Refusal::Rules(crate::building::Refusal::LevelAboveMax { level: 6 }),
            Refusal::TavernTaken { at },
            Refusal::ColonyLevelTooLow {
                level: 1,
                required: 2,
            },
            Refusal::AtKindMaxLevel { level: 3, max: 3 },
            Refusal::ResearchLocked,
            Refusal::ResearchBelow {
                strength: 2,
                level: 3,
            },
            Refusal::ParentNotBuilt { at },
            Refusal::NestedPickup { at },
            Refusal::CannotDeconstruct {
                kind: Kind::BarracksTower,
            },
            Refusal::InventoryFull {
                item: "minecolonies:blockhutbuilder".into(),
            },
            Refusal::NoBuilding { at },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().contains(".refusal."), "{refusal:?}");
            assert_eq!(refusal.to_string(), refusal.name());
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        // The wrapper keeps the layer below's name, so a log line still says
        // "outside the colony" and not "building refused".
        assert_eq!(
            Refusal::Placement(founding::Refusal::OutsideColony { at }).name(),
            founding::Refusal::OutsideColony { at }.name()
        );
    }

    #[test]
    fn the_level_ceiling_is_five_except_for_three_kinds() {
        // AbstractBuilding.java:654-670 for the default, and the three overrides.
        assert_eq!(max_level(Kind::Builder), 5);
        assert_eq!(max_level(Kind::TownHall), 5);
        assert_eq!(max_level(Kind::GateHouse), 3, "BuildingGateHouse.java:34");
        assert_eq!(max_level(Kind::Postbox), 1, "PostBox.java:63-68");
        assert_eq!(max_level(Kind::Stash), 0, "Stash.java:52-56");
        let odd: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| max_level(*kind) != 5)
            .collect();
        assert_eq!(
            odd,
            vec![Kind::GateHouse, Kind::Postbox, Kind::Stash],
            "three overrides, in Kind's own order"
        );
    }

    #[test]
    fn the_claim_radius_of_the_default_table_and_its_six_overrides() {
        // The default, AbstractBuilding.java:654-670.
        for kind in [Kind::Builder, Kind::Farmer, Kind::Baker, Kind::Residence] {
            assert_eq!(claim_radius(kind, 0), 0, "{kind}: the `default` arm");
            for level in 1..=3 {
                assert_eq!(claim_radius(kind, level), 1, "{kind} at {level}");
            }
            for level in 4..=5 {
                assert_eq!(claim_radius(kind, level), 2, "{kind} at {level}");
            }
        }
        // The town hall: 1, 1, 2, 3, 5 — note the jump to five, not four.
        assert_eq!(
            (0..=5).map(|level| claim_radius(Kind::TownHall, level)).collect::<Vec<_>>(),
            vec![0, 1, 1, 2, 3, 5],
            "BuildingTownHall.java:182-202"
        );
        // The guard tower: 2, 3, 3, 4, 5.
        assert_eq!(
            (0..=5).map(|level| claim_radius(Kind::GuardTower, level)).collect::<Vec<_>>(),
            vec![0, 2, 3, 3, 4, 5],
            "BuildingGuardTower.java:52-66"
        );
        // The gatehouse: 1, 1, 2, then nothing.
        assert_eq!(
            (0..=5).map(|level| claim_radius(Kind::GateHouse, level)).collect::<Vec<_>>(),
            vec![0, 1, 1, 2, 0, 0],
            "BuildingGateHouse.java:75-83"
        );
        // A barracks tower never claims: its barracks claims for it.
        for level in 0..=5 {
            assert_eq!(claim_radius(Kind::BarracksTower, level), 0);
        }
        // The barracks answers its conservative arm, and nothing at level 0.
        assert_eq!(claim_radius(Kind::Barracks, 0), 0);
        for level in 1..=5 {
            assert_eq!(claim_radius(Kind::Barracks, level), 2);
        }
    }

    #[test]
    fn a_barracks_claims_three_only_when_every_tower_is_level_four() {
        // BuildingBarracks.java:162-183.
        assert_eq!(claim_radius_with_towers(0, &[]), 0, "nothing built, nothing claimed");
        assert_eq!(claim_radius_with_towers(2, &[]), 3, "no tower is below four");
        assert_eq!(claim_radius_with_towers(2, &[5, 4]), 3);
        assert_eq!(claim_radius_with_towers(2, &[5, 3]), 2, "one tower at three");
        assert_eq!(claim_radius_with_towers(5, &[1]), 2);
    }

    #[test]
    fn a_claim_is_chunk_aligned_in_all_three_axes() {
        // BlockPosUtil.java:591-600: y is chunk-aligned as well, and the box
        // reaches the far edge of the last chunk.
        let none = claim_box([70, 70, -3], 0);
        assert_eq!(none.low(), [64, 64, -16], "rounded down to the chunk corner");
        assert_eq!(none.high(), [79, 79, -1], "and 15 blocks on");
        let one = claim_box([70, 70, -3], 1);
        assert_eq!(one.low(), [48, 48, -32]);
        assert_eq!(one.high(), [95, 95, 15]);
        assert_eq!(
            one.size(),
            [48, 48, 48],
            "three chunks per axis, which is what (2r+1)*16 means"
        );
        assert_eq!(claim_box([0, 0, 0], 2).size(), [80, 80, 80], "five chunks");
    }

    #[test]
    fn a_chunk_is_a_floor_division_on_both_axes() {
        assert_eq!(chunk_of([0, 0, 0]), [0, 0]);
        assert_eq!(chunk_of([15, 99, 15]), [0, 0]);
        assert_eq!(chunk_of([16, 99, 16]), [1, 1]);
        assert_eq!(chunk_of([-1, 0, -1]), [-1, -1], "-1 is in chunk -1");
        assert_eq!(chunk_of([-16, 0, -16]), [-1, -1]);
        assert_eq!(chunk_of([-17, 0, -17]), [-2, -2]);
        assert_eq!(BLOCKS_PER_CHUNK, 16);
    }
}
