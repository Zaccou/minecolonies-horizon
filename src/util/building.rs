//! `BuildingUtils`: which jobs a hut allows, and how far it is turned.
//!
//! Java reference: `core/util/BuildingUtils.java` (211 lines, seven methods).
//! Five of them reach a `Level`, a `Blueprint` or the colony manager; two decide
//! something a caller can be handed. Both of those had already been *named* by
//! modules that needed them without a port to call:
//!
//! - `BuildingUtils.getAllowedJobs(...).test(jobEntry)` at
//!   `ui_window_03/hire.rs:148` — a boolean the window computes by hand;
//! - `BuildingUtils.getRotationFromBlueprint` at `tile_entities/decoration.rs:167`
//!   — a callback that module passes in.
//!
//! # The rotation rule, one level down
//!
//! Java answers "how far is this building turned" by subtracting two
//! *horizontal indices*: the blueprint's, and the world's
//! (`BuildingUtils.java:149-159`). Everything else in `getRotationFromBlueprint`
//! is getting hold of a `Blueprint` — a resource load, a `0.blueprint → 1.blueprint`
//! fallback (`:133-136`), and three error logs. The subtraction is [`rotation_from_blueprint`];
//! the fallback is [`blueprint_path_fallback`]; the lookups belong to
//! [`crate::blueprint`].
//!
//! **The horizontal index is vanilla's, not MineColonies'.** `get2DDataValue()`
//! on a `Direction` reads `Direction.BY_2D_DATA`, whose order is
//! `south, west, north, east` — `crates/horizon-client/src/game/modding/native_entity_moves.rs:25`
//! spells the same array out. That is what makes the answer `0..=3` and what
//! makes `(4 + world - structure) % 4` correct rather than a guess.
//!
//! # What is not here
//!
//! `getItemStackForHutFromInventory` (`:54-71`) is a slot scan whose only rule is
//! `path.endsWith(hut)` — [`hut_path_matches`] — over a live `Inventory`.
//! `commonBuildingFromPosition` (`:201-211`) goes through
//! `IColonyManager.getInstance()` and the building manager; see `MANQUANT.md` §3.

use crate::ui::HiringMode;
use std::collections::BTreeSet;

/// The tag prefix a hut's blueprint uses to restrict its jobs:
/// `t.startsWith("job=")` (`BuildingUtils.java:82`), whose suffix is the tag's
/// payload (`:84`).
pub const JOB_TAG_PREFIX: &str = "job=";

/// Which jobs a hut block allows: `getAllowedJobs` (`BuildingUtils.java:79-95`).
///
/// Java returns a `Predicate<JobEntry>` and spells "no restriction" as
/// `UNRESTRICTED` (`:99`), a lambda that is always `true`. An enum keeps the
/// difference visible: an unrestricted hut has no set to test, and a caller that
/// wants to *display* the restriction can see that it is unrestricted instead of
/// probing the predicate with made-up jobs.
// exhaustive: the two answers Java's predicate has
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum JobFilter {
    /// No `job=` tag at all: every job is permitted.
    #[default]
    Unrestricted,
    /// At least one `job=` tag: exactly those jobs, and **nothing else**.
    Only(BTreeSet<String>),
}

impl JobFilter {
    /// Is this job allowed? `jobTags.contains(job.getKey().getPath()) ||
    /// jobTags.contains(job.getKey().toString())` (`BuildingUtils.java:90`).
    ///
    /// Java accepts **either spelling** — the bare path (`farmer`) or the full
    /// registry id (`minecolonies:farmer`) — because a data pack's author writes
    /// whatever is shorter. Both are tried, path first.
    pub fn permits(&self, path: &str, id: &str) -> bool {
        match self {
            JobFilter::Unrestricted => true,
            JobFilter::Only(tags) => tags.contains(path) || tags.contains(id),
        }
    }

    /// `true` when Java would have answered `UNRESTRICTED`.
    pub fn is_unrestricted(&self) -> bool {
        matches!(self, JobFilter::Unrestricted)
    }

    /// The tags the filter holds, sorted, for a UI that has to show them.
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        let set = match self {
            JobFilter::Unrestricted => None,
            JobFilter::Only(tags) => Some(tags),
        };
        set.into_iter().flat_map(|tags| tags.iter().map(String::as_str))
    }
}

/// Build the filter from a hut's anchor tags: `getAllowedJobs`
/// (`BuildingUtils.java:79-95`).
///
/// ```java
/// final Set<String> jobTags = provider.getPositionedTags().getOrDefault(BlockPos.ZERO, new ArrayList<>()).stream()
///         .filter(t -> t.startsWith("job="))
///         .map(t -> t.substring(4))
///         .collect(Collectors.toSet());
/// if (!jobTags.isEmpty()) { return job -> jobTags.contains(...) || jobTags.contains(...); }
/// return UNRESTRICTED;
/// ```
///
/// The caller passes the tags **of `BlockPos.ZERO`** — the anchor's list, not the
/// whole map (`:81`). Order does not matter, duplicates collapse, and a tag
/// without the `job=` prefix is ignored.
///
/// **The one edge that bites.** A bare `job=` tag has an empty suffix, and Java
/// keeps it: `jobTags` then holds `""`, is non-empty, and the hut allows **no
/// job at all** — `permits` can never match because a registry path is never
/// empty. That is a trap in a hand-written blueprint, it is what Java does, and
/// the port keeps it rather than quietly treating `job=` as "no restriction".
pub fn allowed_jobs<'a>(tags: impl IntoIterator<Item = &'a str>) -> JobFilter {
    let only: BTreeSet<String> = tags
        .into_iter()
        .filter_map(|tag| tag.strip_prefix(JOB_TAG_PREFIX))
        .map(str::to_owned)
        .collect();
    if only.is_empty() {
        JobFilter::Unrestricted
    } else {
        JobFilter::Only(only)
    }
}

/// Does a block's registry path name this hut: the test inside
/// `getItemStackForHutFromInventory` (`BuildingUtils.java:55-60`).
///
/// ```java
/// ForgeRegistries.BLOCKS.getKey(block).getPath().endsWith(hut)
/// ```
///
/// **A suffix test, not an equality test** — that is the whole point: the caller
/// asks for `"builder"` and `blockhutbuilder` answers. It is also why the
/// `AbstractBlockHut` half of the Java condition cannot be dropped: without it,
/// `minecraft:builder` and `structurize:builder` would match too. The port keeps
/// the two halves: the caller passes the **path** of a block it already knows is
/// a hut ([`is_hut_path`]), and this function only compares endings.
pub fn hut_path_matches(path: &str, hut: &str) -> bool {
    path.ends_with(hut)
}

/// Is this the path of a hut block?
///
/// Java's test is `instanceof AbstractBlockHut` (`:57`) and `:109`; the port has
/// no class hierarchy to ask, so the name is the answer: every hut block carries
/// the `blockhut` prefix. That is exactly the split another module already
/// measured — `blocks/huts.rs:39-44` records that the colony's 54 blocks are the
/// **50 `blockhut*` names** plus four that are not huts at all (the two quarries,
/// the postbox and the stash). So the prefix *is* the `instanceof`, not an
/// approximation of it.
///
/// The check is deliberately on the **path** and not the whole id: `minecolonies:blockhutbuilder`
/// is a hut, `othermod:blockhutbuilder` is not, which is the other half of Java's
/// condition (`:57` asks the registry for the block's key, and the key carries the
/// namespace).
pub fn is_hut_path(path: &str) -> bool {
    path.starts_with("blockhut")
}

/// The horizontal index of a facing, `Direction.get2DDataValue()`.
///
/// `south` → `0`, `west` → `1`, `north` → `2`, `east` → `3`, which is vanilla's
/// `Direction.BY_2D_DATA` order
/// (`crates/horizon-client/src/game/modding/native_entity_moves.rs:25`).
///
/// `None` for a vertical facing or for a name that is not a direction: vanilla's
/// `get2DDataValue` answers `-1` there, and `-1` is what
/// `getRotationFromBlock` returns for a block that has no `FACING` property at
/// all (`BuildingUtils.java:181-192`).
pub fn facing_2d_value(facing: &str) -> Option<u8> {
    match facing {
        "south" => Some(0),
        "west" => Some(1),
        "north" => Some(2),
        "east" => Some(3),
        _ => None,
    }
}

/// The rotations of one block: `getRotationFromBlock` (`BuildingUtils.java:181-192`).
///
/// ```java
/// if (blockState.getBlock() instanceof AbstractBlockHut<?>) return blockState.getValue(AbstractBlockHut.FACING).get2DDataValue();
/// else if (blockState.getBlock() instanceof DirectionalBlock) return blockState.getValue(DirectionalBlock.FACING).get2DDataValue();
/// return -1;
/// ```
///
/// The two branches are **the same read**: `AbstractBlockHut` is a
/// `HorizontalDirectionalBlock`, so both ask the `FACING` property for its
/// horizontal index. So the input is that property, and the two `-1` cases — "no
/// `FACING` property" and "a vertical `FACING`" — collapse into `None`, which is
/// Java's `-1`.
pub fn rotation_from_block(facing: Option<&str>) -> Option<u8> {
    facing.and_then(facing_2d_value)
}

/// Why a blueprint rotation could not be measured.
// exhaustive: the two sides Java reports with one log line (`BuildingUtils.java:155-158`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationRefusal {
    /// The blueprint's primary block is not a directional block (`:153`).
    StructureNotDirectional,
    /// The world's block at the anchor is not a directional block (`:153`).
    WorldNotDirectional,
}

impl RotationRefusal {
    /// What Java logs, for a caller that has a log to write to.
    pub fn reason(self) -> &'static str {
        match self {
            RotationRefusal::StructureNotDirectional => {
                "the blueprint's primary block has no horizontal facing"
            }
            RotationRefusal::WorldNotDirectional => {
                "the block at the anchor has no horizontal facing"
            }
        }
    }
}

/// How far the world's block is turned from the blueprint's:
/// `getRotationFromBlueprint` (`BuildingUtils.java:149-159`).
///
/// ```java
/// final int structureRotation = getRotationFromBlock(structureState);
/// final int worldRotation = getRotationFromBlock(worldState);
/// if (structureRotation == -1 || worldRotation == -1) { log; return -1; }
/// return (4 + worldRotation - structureRotation) % 4;
/// ```
///
/// The `4 +` is there so the subtraction cannot go negative before the modulo —
/// a detail that matters, because `(-1) % 4` is `-1` in Java and `-1` in Rust,
/// and the answer would then not be a rotation at all. The result is `0..=3` and
/// **clockwise**, which is what the placer expects.
///
/// Java reports the two refusals with one message and one `-1`; the port names
/// which side failed, because "the blueprint is fine, the world block is wrong"
/// is the difference between a broken data pack and a broken placement.
pub fn rotation_from_blueprint(
    structure_rotation: Option<u8>,
    world_rotation: Option<u8>,
) -> Result<u8, RotationRefusal> {
    let structure = structure_rotation.ok_or(RotationRefusal::StructureNotDirectional)?;
    let world = world_rotation.ok_or(RotationRefusal::WorldNotDirectional)?;
    Ok((4 + world - structure) % 4)
}

/// The `0.blueprint → 1.blueprint` fallback of `getRotationFromBlueprint`
/// (`BuildingUtils.java:133-136`).
///
/// ```java
/// if (blueprint == null && path.endsWith("0.blueprint")) {
///     blueprint = StructurePacks.getBlueprint(pack, path.replace("0.blueprint", "1.blueprint"), true);
/// }
/// ```
///
/// It is a **`replace`, not a suffix rewrite**: every occurrence in the path
/// changes, so `hut/a0.blueprint/b0.blueprint` becomes `hut/a1.blueprint/b1.blueprint`.
/// Kept that way — a path with `0.blueprint` in the middle is nonsense, and a port
/// that only rewrote the tail would behave differently on nonsense.
///
/// `None` when the path does not end in `0.blueprint`: there is no fallback to
/// try, and the caller's resource load already failed.
pub fn blueprint_path_fallback(path: &str) -> Option<String> {
    if path.ends_with("0.blueprint") {
        Some(path.replace("0.blueprint", "1.blueprint"))
    } else {
        None
    }
}

/// May the hut hire on its own? `canAutoHire` (`BuildingUtils.java:109-117`).
///
/// ```java
/// return building.canAssignCitizens()
///         && (hiringMode == HiringMode.DEFAULT && building.getColony().getSettings().getSetting(BuildingTownHall.AUTO_HIRING_MODE).getValue()
///             || hiringMode == HiringMode.AUTO)
///         && (job == null || getAllowedJobs(...).test(job));
/// ```
///
/// Read as four inputs, in Java's order — and the order matters for nothing but
/// evaluation, since all four are pure. `&&` binds tighter than `||`, so the
/// middle group is `(Default && colony setting) || Auto`: an **explicit** `Auto`
/// hires even when the colony's automatic-hiring setting is off, and `Default`
/// follows the setting. `Manual` and `Locked` never hire by themselves, which is
/// the whole reason the enum has four members.
///
/// `job_permitted` is `true` for the "no specific job" call Java spells as a
/// `null` `JobEntry` (`:107`).
///
/// **`Locked` is not `Manual`.** Java's `Locked` exists for homes
/// (`ui/action.rs:250`); this function treats both as "no automatic hire", which
/// is correct here and is not a statement about assignment.
pub fn can_auto_hire(
    can_assign_citizens: bool,
    hiring_mode: HiringMode,
    colony_auto_hiring: bool,
    job_permitted: bool,
) -> bool {
    let mode_allows = (hiring_mode == HiringMode::Default && colony_auto_hiring)
        || hiring_mode == HiringMode::Auto;
    can_assign_citizens && mode_allows && job_permitted
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The full id and the bare path are both accepted (`:90`), and nothing else
    /// is.
    #[test]
    fn a_job_tag_permits_both_spellings_and_nothing_else() {
        let filter = allowed_jobs(["job=farm", "job=minecolonies:fisherman"]);
        assert!(filter.permits("farm", "minecolonies:farm"));
        assert!(filter.permits("fisherman", "minecolonies:fisherman"));
        assert!(!filter.permits("builder", "minecolonies:builder"));
    }

    /// No `job=` tag at all is `UNRESTRICTED` (`:93`), not an empty filter.
    #[test]
    fn no_job_tag_leaves_the_hut_unrestricted() {
        let filter = allowed_jobs(["tag=bed", "working=garden"]);
        assert_eq!(filter, JobFilter::Unrestricted);
        assert!(filter.is_unrestricted());
        assert!(filter.permits("anything", "any:thing"));
    }

    /// An empty iterator is a hut with no tags, which is the same case.
    #[test]
    fn an_empty_tag_list_is_unrestricted() {
        assert_eq!(allowed_jobs(std::iter::empty()), JobFilter::Unrestricted);
    }

    /// Duplicates collapse and the order is the sorted one, so two huts with the
    /// same tags compare equal.
    #[test]
    fn duplicate_tags_collapse_into_one_set() {
        let a = allowed_jobs(["job=builder", "job=builder", "job=cook"]);
        let b = allowed_jobs(["job=cook", "job=builder"]);
        assert_eq!(a, b);
        assert_eq!(a.tags().collect::<Vec<_>>(), vec!["builder", "cook"]);
    }

    /// The trap: `job=` alone narrows the hut to no job at all.
    #[test]
    fn an_empty_job_tag_forbids_every_job_the_way_java_does() {
        let filter = allowed_jobs(["job=", "tag=bed"]);
        assert_eq!(filter, JobFilter::Only(["".to_owned()].into_iter().collect()));
        assert!(!filter.permits("farmer", "minecolonies:farmer"));
        assert!(!filter.permits("cook", "minecolonies:cook"));
    }

    /// A `job` tag **without** the `=` is not a job tag: Java's filter is
    /// `startsWith("job=")`, not `startsWith("job")`.
    #[test]
    fn a_job_tag_without_an_equals_sign_is_ignored() {
        assert_eq!(allowed_jobs(["job"]), JobFilter::Unrestricted);
        assert_eq!(allowed_jobs(["jobs=farm"]), JobFilter::Unrestricted);
    }

    /// The suffix test is what makes `"builder"` find `blockhutbuilder`.
    #[test]
    fn a_hut_is_found_by_the_ending_of_its_path() {
        assert!(hut_path_matches("blockhutbuilder", "builder"));
        assert!(hut_path_matches("blockhutbuilder", "blockhutbuilder"));
        assert!(!hut_path_matches("blockhutbuilder", "builde"));
        assert!(!hut_path_matches("builder", "blockhutbuilder"));
    }

    /// The hut half of Java's condition, as a name test: `othermod:blockhutbuilder`
    /// carries the path but is not one of this mod's huts — which is why the
    /// caller checks the namespace too.
    #[test]
    fn the_hut_prefix_is_what_separates_a_hut_from_another_block() {
        assert!(is_hut_path("blockhutbuilder"));
        assert!(is_hut_path("blockhuttownhall"));
        assert!(!is_hut_path("builder"));
        assert!(!is_hut_path("blockconstructiontape"));
    }

    /// Vanilla's `BY_2D_DATA` order, which the rotation arithmetic rests on.
    #[test]
    fn the_horizontal_index_is_south_west_north_east() {
        assert_eq!(facing_2d_value("south"), Some(0));
        assert_eq!(facing_2d_value("west"), Some(1));
        assert_eq!(facing_2d_value("north"), Some(2));
        assert_eq!(facing_2d_value("east"), Some(3));
    }

    /// A vertical facing and a block with no `FACING` property are both Java's
    /// `-1`, which the port spells `None`.
    #[test]
    fn up_down_and_no_facing_are_not_rotations() {
        assert_eq!(facing_2d_value("up"), None);
        assert_eq!(facing_2d_value("down"), None);
        assert_eq!(facing_2d_value(""), None);
        assert_eq!(rotation_from_block(None), None);
    }

    /// A hut facing north in a blueprint and east in the world is turned once
    /// clockwise: `(4 + 3 - 2) % 4 == 1`.
    #[test]
    fn the_rotation_is_the_clockwise_difference() {
        let structure = rotation_from_block(Some("north"));
        let world = rotation_from_block(Some("east"));
        assert_eq!(rotation_from_blueprint(structure, world), Ok(1));
    }

    /// The `4 +` keeps the subtraction non-negative, which is what makes an
    /// east-to-south turn `3` and not `-1`.
    #[test]
    fn a_negative_difference_becomes_a_positive_rotation() {
        // world south (0), structure east (3): 4 + 0 - 3 == 1
        assert_eq!(rotation_from_blueprint(Some(3), Some(0)), Ok(1));
        // world south (0), structure west (1): 4 + 0 - 1 == 3
        assert_eq!(rotation_from_blueprint(Some(1), Some(0)), Ok(3));
    }

    /// Identical facings are no rotation at all.
    #[test]
    fn the_same_facing_on_both_sides_is_zero_rotation() {
        for index in 0u8..4 {
            assert_eq!(rotation_from_blueprint(Some(index), Some(index)), Ok(0));
        }
    }

    /// The two refusals are named, and the first side checked is the structure.
    #[test]
    fn a_side_that_is_not_directional_refuses_by_name() {
        assert_eq!(
            rotation_from_blueprint(None, Some(0)),
            Err(RotationRefusal::StructureNotDirectional)
        );
        assert_eq!(
            rotation_from_blueprint(Some(0), None),
            Err(RotationRefusal::WorldNotDirectional)
        );
        assert_eq!(
            rotation_from_blueprint(None, None),
            Err(RotationRefusal::StructureNotDirectional)
        );
    }

    /// The fallback only fires on a path that ends in `0.blueprint`, and it
    /// rewrites every occurrence, like Java's `String.replace`.
    #[test]
    fn the_blueprint_fallback_rewrites_every_zero() {
        assert_eq!(
            blueprint_path_fallback("darkoak/builder0.blueprint"),
            Some("darkoak/builder1.blueprint".to_owned())
        );
        assert_eq!(
            blueprint_path_fallback("a0.blueprint/b0.blueprint"),
            Some("a1.blueprint/b1.blueprint".to_owned())
        );
        assert_eq!(blueprint_path_fallback("darkoak/builder1.blueprint"), None);
        assert_eq!(blueprint_path_fallback("darkoak/builder0"), None);
    }

    /// `Auto` hires even when the colony's setting is off — that is what `||`
    /// rather than `&&` between the two groups means.
    #[test]
    fn an_explicit_auto_mode_overrides_the_colony_setting() {
        assert!(can_auto_hire(true, HiringMode::Auto, false, true));
        assert!(can_auto_hire(true, HiringMode::Auto, true, true));
    }

    /// `Default` follows the colony setting in both directions.
    #[test]
    fn the_default_mode_follows_the_colony_setting() {
        assert!(can_auto_hire(true, HiringMode::Default, true, true));
        assert!(!can_auto_hire(true, HiringMode::Default, false, true));
    }

    /// `Manual` and `Locked` never hire by themselves, whatever the setting.
    #[test]
    fn manual_and_locked_never_hire_automatically() {
        for mode in [HiringMode::Manual, HiringMode::Locked] {
            assert!(!can_auto_hire(true, mode, true, true));
        }
    }

    /// A building that cannot assign citizens is refused before the mode is read,
    /// and a job the hut forbids is refused last — both are `false` all the same.
    #[test]
    fn a_building_that_cannot_assign_or_a_forbidden_job_says_no() {
        assert!(!can_auto_hire(false, HiringMode::Auto, true, true));
        assert!(!can_auto_hire(true, HiringMode::Auto, true, false));
    }
}
