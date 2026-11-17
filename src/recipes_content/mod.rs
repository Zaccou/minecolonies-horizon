//! MineColonies' own recipe content, and where the pack's ids meet 26.3.
//!
//! # Three questions, three files
//!
//! | Question | Answered by |
//! | --- | --- |
//! | which id becomes what, and on whose authority? | [`renames`] — a view over [`crate::compat`], never a second table |
//! | what does one palette still need, and how many cells does it cost? | [`plan`] — the verifier |
//! | what do the four recipe files the load refuses carry, and what do they wait for? | [`excluded`] |
//!
//! The renaming rules are **not written here**. [`crate::compat::table`] holds the
//! two rows — `minecraft:grass` → `minecraft:short_grass`,
//! `minecraft:chain` → `minecraft:iron_chain` — with the version and the page
//! that prove each one, measured over the palettes of all 9 515 blueprints. What
//! this module adds is what a *content* reader needs on top of that table: the
//! registry an id is read from (a recipe names an **item**, a palette names a
//! **block**), the shipped files that depend on each row, and a verifier that
//! answers per blueprint instead of per state.
//!
//! # The two measured facts of the brief, restated with their weight
//!
//! **Two blocks are missing from 26.3, and neither is lost.** Both are renamed,
//! nothing disappeared, and no block state went with them: `iron_chain` declares
//! the same `axis` and `waterlogged` values 1.20.1's `chain` did, and the plant
//! has no state at all (`crate::compat::states`). The same two names are the whole
//! item-side story: of the 233 distinct ids the 154 shipped recipes write, **two**
//! are 26.3-absent, in **six references over five files** ([`renames::RECIPE_USES`]).
//!
//! **The four excluded files wait for a registration, but not for the reason
//! written down.** They declare `minecolonies:composting` and
//! `minecolonies:zero_waste`, and the stated cause — *"an unknown type costs the
//! whole recipe table"* — is false in the pinned engine: a type outside
//! `minecraft:` is kept as a snapshot, and one file costs the other 154 nothing.
//! What the load really refuses is a file naming an **ingredient kind** nobody
//! answers, and three of these four name `minecolonies:food` /
//! `minecolonies:plant`. The proof, the line numbers and the readiness check are
//! in [`excluded`]'s module doc; the gap itself is `MANQUANT.md` §1.
//!
//! # What this module does not do
//!
//! It does not rewrite `data/`, register a recipe type, or answer an ingredient
//! hook — those are `data/`, `src/crafting/` and `src/lib.rs`, three files this
//! module does not own. [`plan::Plan::palette_edits`] **returns** the rewrite; who
//! assigns it into a palette decides when.
//!
//! | File | Question |
//! | --- | --- |
//! | `renames` | which row, in which registry, over how much content |
//! | `plan` | one palette → the ids to replace, the ids still unknown, weighted by cells |
//! | `excluded` | the four files: bytes, Java type, and what the load will do with them |
//! | `MANQUANT.md` | what this module needs and does not have |

pub mod excluded;
pub mod plan;
pub mod renames;

pub use excluded::{
    custom_ingredient_kinds, Excluded, JavaType, Loads, Readiness, EXCLUDED, INGREDIENT_KEYS,
    VANILLA_CRAFTING,
};
pub use plan::{
    plan_blueprint, plan_ids, plan_palette, Blocked, Entry, Plan, Registry, Verdict,
};
pub use renames::{
    apply, is_foreign, renamed, row, uses_of, Application, MeasuredRecipes, RecipeUse, Target,
    ITEM_DIGEST_26_3, ITEM_KEYS_26_3, MEASURED_RECIPES, RECIPE_USES, REGISTRY_ITEM_26_3,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{Report, SPECS_26_3};
    use std::path::{Path, PathBuf};

    const PACK: &str = "data/minecolonies/recipe";

    #[test]
    fn the_rename_a_recipe_needs_is_the_rename_a_palette_gets() {
        // One row, two surfaces: the item a recipe reads and the block a palette
        // names are renamed by the same compat row, and the verifier agrees.
        let mut report = Report::from_specs(SPECS_26_3);
        report.declare_block("minecraft:stone");
        for used in RECIPE_USES {
            let applied = apply(Target::Item, used.id);
            let target = applied
                .target_id()
                .unwrap_or_else(|| panic!("{} is not renamed", used.id));
            let proof = applied
                .row()
                .unwrap_or_else(|| panic!("{} has no compat row", used.id));
            let checked = plan_ids([used.id], &report);
            let entry = checked
                .entries
                .first()
                .unwrap_or_else(|| panic!("{} is not in the plan", used.id));
            assert_eq!(
                entry.verdict,
                Verdict::Replace {
                    to: target,
                    since: proof.since,
                    source: proof.source,
                },
                "{}: the two surfaces must not disagree",
                used.id
            );
        }
        assert_eq!(
            plan_ids(["minecraft:stone"], &report).replacements().count(),
            0
        );
    }

    #[test]
    fn no_file_of_the_154_carries_one_of_the_two_excluded_types() {
        // The split is the whole reason `recettes-custom/` exists: if one of these
        // types ever lands in `data/`, this test says so instead of the load.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(PACK);
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("recipe pack {}: {error}", dir.display()));
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.to_string_lossy().ends_with(".json"))
            .collect();
        paths.sort();
        assert_eq!(
            paths.len(),
            MEASURED_RECIPES.files,
            "the shipped table holds {} files",
            MEASURED_RECIPES.files
        );
        for path in &paths {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            for excluded in EXCLUDED {
                assert!(
                    !text.contains(excluded.kind.type_name()),
                    "{} carries {}, which lives in recettes-custom/",
                    path.display(),
                    excluded.kind.type_name()
                );
            }
        }
        let declared: Vec<&str> = EXCLUDED.iter().map(|entry| entry.kind.type_name()).collect();
        assert_eq!(
            declared,
            [
                "minecolonies:composting",
                "minecolonies:composting",
                "minecolonies:composting",
                "minecolonies:zero_waste",
            ]
        );
        assert!(
            INGREDIENT_KEYS.contains(&"input") && INGREDIENT_KEYS.contains(&"ingredients"),
            "the two keys the four files use"
        );
    }
}
