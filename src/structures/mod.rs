//! The structures: which blueprint a hut asks for, which file answers it, and
//! whether the ground will take it.
//!
//! Java reference: `api/compatibility/newstruct/BlueprintMapping.java` (the
//! style and folder tables), `core/colony/buildings/AbstractSchematicProvider.java`
//! (the path a building computes from its style and its level),
//! `core/colony/workorders/WorkOrderBuilding.java` (the same path, retargeted to
//! another level) and, for the pose itself, `core/placementhandlers/main/
//! SurvivalHandler.java` with `com.ldtteam.structurize.util.RotationMirror`
//! (the eight states a structure can be pasted in).
//!
//! **The question, in one sentence.** Given a style, a building kind and a
//! level, [`Index`] answers the one `.nbt` file that the engine will paste, or a
//! named [`Refusal`]; [`Placing`] then says which blocks that paste will touch;
//! [`survey`] says whether they are free and whether the ground under
//! them bears.
//!
//! | File | Question |
//! | --- | --- |
//! | `naming` | what a blueprint is called: style, folder, base name, level |
//! | [`Index`] | which of the 9 515 files answer a name, and which one if the variant is missing |
//! | [`Placing`] | what has to be supplied to paste it, and the box it will cover |
//! | [`survey`] | is the volume free, does the ground bear, and what does the builder have to do first |
//!
//! **The corpus is the second source of truth.** Every rule below was checked
//! against `data/minecolonies/structure/minecolonies/` — **9 515 `.nbt` files in
//! 23 style folders**, 54 building kinds, 1 691 files that are decorations, roads
//! or fields rather than levelled buildings. The tests name the file they read a
//! size or a level from, because a naming rule that only agrees with its own
//! table is a rule nobody has run.
//!
//! **What Java says and what the corpus says, and where they meet.** Java's
//! folder table names 49 of the 54 kinds; `gatehouse`, `kitchen`, `postbox`,
//! `stable` and `stash` have no entry (`BlueprintMapping.java:57-72`), and the
//! corpus has files for the first, second and fourth — so [`Index`] falls back to
//! a scan by base name instead of inventing a folder. Java's `getPathMapping`
//! returns `null` for those (`:140-143`) and the game survives only because the
//! build tool carries a path of its own.
//!
//! The **level ceilings** agree, and that agreement is the whole justification of
//! the cap: Java caps a building through `getMaxBuildingLevel()` or through the
//! fourth argument of `DefaultBuildingInstance` — `residence` 5, `tavern` 3
//! (`ModBuildingsInitializer.java:247`, `:521`), both quarries 1 (`:631`, `:638`),
//! `gatehouse` 3 (`BuildingGateHouse.java:34`), `postbox` 1
//! (`workerbuildings/PostBox.java:63-67`), `stash` 0 (`workerbuildings/Stash.java:54-57`).
//! The corpus has exactly those files, measured, and no more. Where they ever
//! disagree the port follows the **corpus**, because a file that does not exist
//! cannot be pasted: a level the pack lacks is capped and reported
//! ([`Resolved::capped`]), never silently claimed.
//!
//! **What is deliberately absent**, each with its cost, all of them also listed
//! in `src/structures/MANQUANT.md`:
//!
//! - **the paste itself.** No host call is made here at all: a wasm mod reaches
//!   the world through hooks, and `crate::ai::builder` is where a work order
//!   becomes steps. This module answers *what* would be pasted, *where*, and
//!   *whether the ground allows it* — three questions that need no world.
//! - **the anchor.** The conversion dropped Structurize's `primary_offset`
//!   (`RAPPORT_BP2NBT.md` §2), so `Placing::anchor` is a fact the caller
//!   supplies; the corpus cannot answer it.
//! - **the weak-solid distinction.** `BlockUtils.isWeakSolidBlock`
//!   (`strukturize/util/BlockUtils.java:808-822`) tells dirt from stone, and
//!   [`crate::ai::Block`] has no such answer: `Block::Solid` is "bears".
//! - **the scan.** [`Index`] is built from a list the caller read; a wasm mod has
//!   no file system, so nothing here enumerates `data/`.

mod index;
mod naming;
mod place;
mod terrain;

pub use index::{Entry, Index, Resolved};
pub use naming::{Named, Style, candidates, folder_of, level_of, named, strip_level, with_level};
pub use place::{Limits, Mirror, Orientation, Placing, Turn, turn_from_count};
pub use terrain::{SiteReport, Verdict, site, site_within, survey, survey_within};

/// The namespace the corpus lives under: `data/minecolonies/structure/…`, which
/// is what the engine's template reader resolves (`NOTES_TEMPLATE_NBT.md` §1.1,
/// `<resources>/data/<namespace>/structure/<path>.nbt`).
pub const NAMESPACE: &str = "minecolonies";

/// The extension the corpus uses. Java writes `.blueprint` and reads the file
/// name with `Utils.getBlueprintLevel`, which strips `.blueprint` before looking
/// for the trailing digit (`api/util/Utils.java:199-219`); the conversion renamed
/// the containers, not the names.
pub const EXTENSION: &str = ".nbt";

/// The lowest level a blueprint file is named after.
///
/// Level 0 is a hut that stands and has never been built
/// (`AbstractSchematicProvider.java:64`), and no file is named `…0`: a `REMOVE`
/// order reuses the *current* level (`WorkOrderBuilding.java:57-60`), never 0.
pub const MIN_LEVEL: u8 = 1;

/// Why a structure operation was refused, named.
///
/// Every variant is a rule that exists somewhere, and the doc says where. A
/// variant is **not** a `lang` key — those live in `assets/minecolonies/lang/`,
/// which this module does not own; the name is for logs and tests.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The style is not one of the 23 the corpus carries, and not one of the
    /// twenty legacy names `BlueprintMapping` maps onto them (`:25-55`).
    UnknownStyle { style: String },
    /// No file, no candidate and no scan answered the name at this level: the
    /// building cannot be pasted in this style.
    UnknownStructure {
        style: String,
        name: String,
        level: u8,
    },
    /// The style holds the building, but not at a level at or below the one
    /// asked for: level 0, or a file set that starts above the request.
    NoSuchLevel {
        base: String,
        asked: u8,
        lowest: u8,
        highest: u8,
    },
    /// Two folders of one style hold the same base name, so the scan cannot say
    /// which file Java would have meant. `cavern` is the measured case: its
    /// military buildings sit in `military/default/` and `military/megahall/`.
    Ambiguous {
        style: String,
        base: String,
        first: String,
        second: String,
    },
    /// The path handed to [`with_level`] does not end in a digit, so there is no
    /// level in it to replace. Java writes the replacement over the last
    /// character regardless (`WorkOrderBuilding.java:63-65`) and corrupts the
    /// name; the port refuses.
    NotALeveledPath { path: String },
    /// The box the paste covers is larger than the box the terrain offers
    /// (`Footprint::covers`, `AbstractSchematicProvider.java:505-521`).
    TooBigForSite {
        needs: [u32; 3],
        site: [u32; 3],
    },
    /// The box crosses the build height: below `min_y` or at or above `max_y`,
    /// the engine's own bounds (`crates/horizon-world/src/level/mod.rs:913-919`:
    /// inclusive minimum, exclusive maximum).
    OutOfWorld {
        low: i32,
        high: i32,
        min_y: i32,
        max_y: i32,
    },
    /// A position the survey had to read is in a chunk the caller marked
    /// unloaded. Absent is not false: an unloaded chunk is not air
    /// (`crate::ai::facts`, `Blocks::is_loaded`), so the survey refuses instead
    /// of clearing a house it never saw.
    NotLoaded { at: crate::ai::Pos },
}

impl Refusal {
    /// The variant's own name, namespaced so it cannot be mistaken for a
    /// sentence. Mirrors `crate::building::Refusal::name`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::UnknownStyle { .. } => "structures.refusal.unknown_style",
            Self::UnknownStructure { .. } => "structures.refusal.unknown_structure",
            Self::NoSuchLevel { .. } => "structures.refusal.no_such_level",
            Self::Ambiguous { .. } => "structures.refusal.ambiguous",
            Self::NotALeveledPath { .. } => "structures.refusal.not_a_leveled_path",
            Self::TooBigForSite { .. } => "structures.refusal.too_big_for_site",
            Self::OutOfWorld { .. } => "structures.refusal.out_of_world",
            Self::NotLoaded { .. } => "structures.refusal.not_loaded",
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
    fn every_refusal_has_its_own_name() {
        let style = String::from("original");
        let base = String::from("builder");
        let at = crate::ai::Pos::new(1, 2, 3);
        let all = [
            Refusal::UnknownStyle {
                style: style.clone(),
            },
            Refusal::UnknownStructure {
                style: style.clone(),
                name: String::from("citizen"),
                level: 1,
            },
            Refusal::NoSuchLevel {
                base: base.clone(),
                asked: 0,
                lowest: 1,
                highest: 5,
            },
            Refusal::Ambiguous {
                style: style.clone(),
                base: base.clone(),
                first: String::from("military/default"),
                second: String::from("military/megahall"),
            },
            Refusal::NotALeveledPath {
                path: String::from("decorations/apiary"),
            },
            Refusal::TooBigForSite {
                needs: [15, 8, 13],
                site: [3, 3, 3],
            },
            Refusal::OutOfWorld {
                low: 300,
                high: 321,
                min_y: -64,
                max_y: 320,
            },
            Refusal::NotLoaded { at },
        ];
        let mut names: Vec<&str> = all.iter().map(Refusal::name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len(), "two refusals share a name");
        for refusal in &all {
            assert_eq!(refusal.to_string(), refusal.name(), "Display is the name");
            assert!(
                refusal.name().starts_with("structures.refusal."),
                "a name has to be namespaced: {}",
                refusal.name()
            );
        }
    }

    #[test]
    fn a_refusal_is_an_error_so_a_caller_can_use_the_question_mark() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&Refusal::UnknownStyle {
            style: String::from("nope"),
        });
    }

    #[test]
    fn the_namespace_and_the_extension_are_the_corpus() {
        // The id of a template is `minecolonies:<style>/<folder>/<base><level>`,
        // which is the on-disk path under `data/minecolonies/structure/`.
        assert_eq!(NAMESPACE, "minecolonies");
        assert_eq!(EXTENSION, ".nbt");
        // No file is named after level 0 (`AbstractSchematicProvider.java:64`,
        // `WorkOrderBuilding.java:57-60`).
        assert_eq!(MIN_LEVEL, 1);
    }
}
