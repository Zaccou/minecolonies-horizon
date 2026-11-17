//! `CreativeRaiderStructureHandler`: which raider a gold block in a ship spawns.
//!
//! Java reference: `core/util/CreativeRaiderStructureHandler.java` (224 lines).
//! The class is a `CreativeStructureHandler` — a placement driver — and almost all
//! of it is placement: a `Blueprint` future, a `StructurePlacer`, a
//! `PlaceStructureOperation` queue, and two constructors that differ only in how the
//! blueprint arrives.
//!
//! Its single decision is in `triggerSuccess` (`:143-167`), which runs once per
//! block the placer puts down:
//!
//! ```java
//! final BlockPos worldPos = getProgressPosInWorld(pos);
//! if (getWorld().getBlockState(worldPos).getBlock() == Blocks.GOLD_BLOCK && map != null) {
//!     final List<String> tags = map.getOrDefault(worldPos, Collections.emptyList());
//!     for (final String tag : tags) {
//!         switch (tag) {
//!             case NORMAL_RAIDER: ShipBasedRaiderUtils.setupSpawner(..., event.getNormalRaiderType(), ...); return;
//!             case ARCHER_RAIDER: ... return;
//!             case BOSS_RAIDER:   ... return;
//!         }
//!     }
//! }
//! ```
//!
//! That is: **a gold block is a spawner iff the blueprint tagged it**, and the tag
//! picks the raider. Three details are behaviour and all three are kept:
//!
//! 1. the gold-block test comes **first**: a tagged block that is not gold is not a
//!    spawner, which is how a blueprint can tag a doorframe it also wants to be a
//!    gold block somewhere else;
//! 2. the tag list is scanned **in order** and the first raider tag wins, so a block
//!    tagged `boss` then `normal` spawns a boss;
//! 3. `map == null` — a blueprint with no `IBlueprintDataProviderBE` at its anchor
//!    — disables *every* spawner, and the constructor logs that once
//!    (`:93-96`, `:137-140`) rather than failing the placement.
//!
//! # What is not here
//!
//! `ShipBasedRaiderUtils.setupSpawner` needs a `Level`, a `BlockPos` and the raid
//! event; `Manager.addToQueue` needs the structure queue. Both are host calls. The
//! gold-block test is `minecraft:gold_block` and nothing else — not the ore, not the
//! raw block — which is why it is a constant here and not a family.

/// The tag that makes a gold block a normal raider's spawner:
/// `SchematicTagConstants.NORMAL_RAIDER = "normal"`
/// (`api/util/constant/SchematicTagConstants.java:37`, matched at
/// `CreativeRaiderStructureHandler.java:155`).
pub const NORMAL_RAIDER: &str = "normal";

/// `SchematicTagConstants.ARCHER_RAIDER = "archer"` (`:38`, matched at `:158`).
///
/// The same string as the knight's `TAG_ARCHER` (`:26`) — two constants, one
/// value, two meanings: in a guard tower it names a job, here a raider type.
pub const ARCHER_RAIDER: &str = "archer";

/// `SchematicTagConstants.BOSS_RAIDER = "boss"` (`:39`, matched at `:161`).
pub const BOSS_RAIDER: &str = "boss";

/// The only block that can carry a raider spawner (`:148`): vanilla's gold block,
/// bare id — `minecraft:gold_ore` and `minecraft:raw_gold_block` are **not** it.
pub const SPAWNER_BLOCK: &str = "minecraft:gold_block";

/// Which raider a spawner block spawns.
// exhaustive: the three raider types the switch names
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiderKind {
    Normal,
    Archer,
    Boss,
}

/// What the placement of one block decided.
// exhaustive: a spawner or nothing, which is what `triggerSuccess` returns
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The block is a spawner of this raider (`:155-163`).
    Spawner(RaiderKind),
    /// Not a spawner: not gold, or gold and untagged, or no blueprint data at all.
    Nothing,
}

/// Is this placement a raider spawner, and of whom:
/// `triggerSuccess` (`CreativeRaiderStructureHandler.java:143-167`).
///
/// `tags` is `map.getOrDefault(worldPos, Collections.emptyList())` (`:150`) — the
/// blueprint's own tags for this cell, in the blueprint's order. `None` is
/// Java's `map == null` (`:148`), the blueprint-without-data case that disables
/// every spawner at once.
///
/// The order is `gold block first, then the first raider tag`: a non-gold block is
/// `Nothing` whatever it is tagged, and a gold block with only `sit`, `work` or no
/// tag at all is `Nothing` too — a gold block in a ship is decoration unless the
/// author said otherwise.
pub fn placement(block: &str, tags: Option<&[&str]>) -> Placement {
    if block != SPAWNER_BLOCK {
        return Placement::Nothing;
    }
    let Some(tags) = tags else {
        return Placement::Nothing;
    };
    for tag in tags {
        let kind = match *tag {
            NORMAL_RAIDER => RaiderKind::Normal,
            ARCHER_RAIDER => RaiderKind::Archer,
            BOSS_RAIDER => RaiderKind::Boss,
            _ => continue,
        };
        return Placement::Spawner(kind);
    }
    Placement::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The happy path: a gold block tagged `normal` spawns a normal raider.
    #[test]
    fn a_gold_block_tagged_normal_is_a_normal_spawner() {
        assert_eq!(
            placement(SPAWNER_BLOCK, Some(&["normal"])),
            Placement::Spawner(RaiderKind::Normal)
        );
    }

    /// All three tags are read.
    #[test]
    fn the_three_raider_tags_are_the_three_kinds() {
        for (tag, kind) in [
            (NORMAL_RAIDER, RaiderKind::Normal),
            (ARCHER_RAIDER, RaiderKind::Archer),
            (BOSS_RAIDER, RaiderKind::Boss),
        ] {
            assert_eq!(
                placement(SPAWNER_BLOCK, Some(&[tag])),
                Placement::Spawner(kind)
            );
        }
    }

    /// The gold-block test comes first: a tagged stone block is not a spawner.
    #[test]
    fn a_tagged_block_that_is_not_gold_is_not_a_spawner() {
        assert_eq!(
            placement("minecraft:stone", Some(&["boss"])),
            Placement::Nothing
        );
        assert_eq!(
            placement("minecraft:gold_ore", Some(&["boss"])),
            Placement::Nothing
        );
        assert_eq!(
            placement("minecraft:raw_gold_block", Some(&["boss"])),
            Placement::Nothing
        );
    }

    /// A gold block with no raider tag — decoration, furniture, or no tag at all —
    /// spawns nothing.
    #[test]
    fn a_gold_block_with_no_raider_tag_spawns_nothing() {
        assert_eq!(placement(SPAWNER_BLOCK, Some(&[])), Placement::Nothing);
        assert_eq!(
            placement(SPAWNER_BLOCK, Some(&["sit", "work"])),
            Placement::Nothing
        );
        assert_eq!(placement(SPAWNER_BLOCK, None), Placement::Nothing);
    }

    /// The list is scanned in order and the **first** raider tag wins.
    #[test]
    fn the_first_raider_tag_in_the_list_wins() {
        assert_eq!(
            placement(SPAWNER_BLOCK, Some(&["boss", "normal"])),
            Placement::Spawner(RaiderKind::Boss)
        );
        assert_eq!(
            placement(SPAWNER_BLOCK, Some(&["sit", "archer", "boss"])),
            Placement::Spawner(RaiderKind::Archer)
        );
    }

    /// A blueprint with no tag data at all disables every spawner, even a correctly
    /// tagged gold block.
    #[test]
    fn a_blueprint_without_tag_data_has_no_spawner_at_all() {
        assert_eq!(placement(SPAWNER_BLOCK, None), Placement::Nothing);
    }
}
