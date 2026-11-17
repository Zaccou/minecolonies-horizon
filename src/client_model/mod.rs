//! The data that decides what the mod's blocks and items look like.
//!
//! **One sentence.** A block state names a model, the model names textures and
//! turns, and this module answers *which* without a GPU, a game or a file.
//!
//! | File | Answers |
//! | --- | --- |
//! | [`blockstate`] | read a `blockstates/*.json`, resolve a state to its models, measure a file's coverage |
//! | [`variants`] | the 95 files of the mod: their shape, their models, their turns, their `facing` convention, their fingerprint |
//! | [`transforms`] | the quarter turns of a variant, the rotation of an element, the `display` entries of an item |
//! | [`textures`] | a child model's texture replacement, `#name` resolution, the 35 vanilla shades the models borrow |
//!
//! The pipeline is one line long, and every step is testable on its own:
//!
//! ```text
//! "facing=east"  --Key::matches-->  variant  --Part-->  turn_point(p, [0,3,0])
//!                                    ^                    model id for the caller
//!                    Definition::parse (a file), or Family (a row of the table)
//! ```
//!
//! **Measured, not remembered.** 95 blockstate files - 87 `variants`, 1
//! `multipart`, 7 that are item models in the wrong folder - and 86 of them read
//! by a registered block. Every family names at least one model for every state
//! its block declares (`variants::Family::uncovered` is 0 for all 95), 10 leave a
//! property out of their keys, 2 pick a model at random, and the same
//! `facing=north` means four different quarter turns depending on the file.
//!
//! # Two premises of the brief that are false
//!
//! 1. **`core/client/model/` holds no block and no item model.** Its 95 files and
//!    9 168 lines are the 91 entity models - 82 citizens (`CitizenModel`
//!    subclasses), the mercenary, the scarecrow, the spear, and the 9 raiders -
//!    plus `package-info.java`. The block and item appearance of MineColonies is
//!    **pure data**, `assets/minecolonies/blockstates/` (95 files) and
//!    `models/{block,item}/` (216 + 170 files), already ported as JSON by the
//!    content ports (`port-domum`, `data/`). What was missing was the *rule* that
//!    turns a state into a model, and that is what this module is. The skeletal
//!    animation of those 91 models is the renderer's business
//!    (`crate::render::models` holds the model *types* and their textures).
//! 2. **No block has a wool, wood or terracotta variant property.** The colour
//!    variation the brief describes is a **texture replacement**: `blockhutcitizen`
//!    plus four children that each rewrite two keys of its map
//!    (`textures::Textures::overridden` returns `["14", "7"]` for all four). The
//!    only "variant" properties in the mod are `variant` on the barrel, the grave
//!    and the rack, all of which select a *model*, not a colour.
//!
//! In passing: the mod's only `ModelData` is `TileEntityRack.getModelData`
//! (`core/tileentities/TileEntityRack.java:766-774`, a Forge client hook that
//! gives the rack a chest-like texture), and it has no equivalent here -
//! `MANQUANT.md` §1.
//!
//! **What this module never does**: it does not read `assets/`, does not call a
//! host method and does not draw. A test greps these five files for the
//! filesystem, the network and any host call, and fails if one appears - which is
//! what lets the 81 tests of these five files run with no game, no wasm and
//! no prepared resources.

pub mod blockstate;
pub mod table;
pub mod textures;
pub mod transforms;
pub mod variants;

#[cfg(test)]
mod tests {
    /// The five sources of this module, read from the repository at compile time.
    ///
    /// A rule that called the host could not be tested without a game; a rule that
    /// opened a file could not be tested at all on a machine the resources were
    /// not copied to. Neither is allowed here, and this is the test that says so.
    const SOURCES: [(&str, &str); 5] = [
        ("mod.rs", include_str!("mod.rs")),
        ("blockstate.rs", include_str!("blockstate.rs")),
        ("variants.rs", include_str!("variants.rs")),
        ("transforms.rs", include_str!("transforms.rs")),
        ("textures.rs", include_str!("textures.rs")),
    ];

    #[test]
    fn nothing_in_this_module_opens_a_file_or_calls_a_host() {
        // Skip this test's own body: it mentions the strings it forbids.
        let forbidden = [
            ["std::", "fs"].concat(),
            ["std::", "net"].concat(),
            ["std::", "process"].concat(),
            ["mod_api::", "Host"].concat(),
        ];
        for (name, source) in SOURCES {
            let rules = source
                .split("fn nothing_in_this_module_opens_a_file")
                .next()
                .unwrap_or("");
            for needle in &forbidden {
                assert!(
                    !rules.contains(needle.as_str()),
                    "{name} uses {needle}, so its rules need the game to run"
                );
            }
        }
    }

    #[test]
    fn the_table_is_generated_and_says_so() {
        let table = include_str!("table.rs");
        assert!(table.starts_with("//! La table des 95 fichiers"));
        assert!(table.contains("Ne pas editer : relancer le script."));
        // And the generator that makes it ships beside it, so the fingerprint has a
        // second end.
        let generator = include_str!("generer_table.py");
        assert!(generator.contains("def couverture("));
        assert!(generator.contains("0xCBF29CE484222325"));
    }

    #[test]
    fn the_module_counts_are_the_ones_the_tests_count() {
        // A cheap guard against a doc that drifts from the table.
        assert_eq!(super::table::FAMILIES.len(), 95);
        assert_eq!(
            super::variants::fingerprint(super::table::FAMILIES),
            super::variants::MEASURED
        );
    }
}
