//! The 1.20.1 → 26.3 block table: every palette id the pack uses that 26.3 does
//! not know, its 26.3 name — or the mark that the block is gone — and the proof.
//!
//! # The measurement this table answers
//!
//! Only the **palette** is looked at, the `palette` list of the NBT document that
//! a cell indexes ([`crate::blueprint::Blueprint::palette`], read the way
//! `format.rs` / `BlueprintUtil.java:217-223` read it). Reading every string of
//! the file instead is what this module exists to undo, and the two known cases
//! show the mechanism, counted on the decompressed NBT of all 9 515 files:
//!
//! | id | raw string hits | palette entries | the rest is |
//! | --- | --- | --- | --- |
//! | `minecraft:grass` | 4 108 | 1 715 | 2 393 × `minecraft:grass_block` |
//! | `minecraft:chain` | 1 729 | 1 313 | 416 × `minecraft:chainmail_*` **items**, in an entity's `ArmorItems` |
//!
//! That is how a raw scan reaches the brief's 238 "missing `minecraft:` ids" —
//! `bread`, `arrow`, `bed` and the chain mail are items — while the palette holds
//! **two**. The whole measurement, in [`Measured`]:
//!
//! | Measured | Value |
//! | --- | --- |
//! | blueprint files | 9 515 |
//! | palette entries | 612 400 |
//! | distinct block ids | 1 017 (860 `minecraft`, 157 in a mod namespace) |
//! | blocks the 26.3 registry knows | 1 286 |
//! | **`minecraft` blocks 26.3 does not know** | **2** |
//!
//! The 1 286 come from the pinned client's export,
//! `["minecraft:block"]["entries"]` of `.local/horizon-client/26.3/registries.json`;
//! a test re-derives the palette side from the pack and compares both whenever the
//! file is on the machine.
//!
//! # Why only two rows
//!
//! **An entry without proof is an invention.** Each row carries the version that
//! introduced the change and the page that says so, and its 26.3 side was checked
//! in the registry. Nothing else is tabulated because nothing else was measured:
//! when a future blueprint needs an id the registry lacks, the measurement test
//! fails before a wrong row can be added to hide it.
//!
//! # The 157 mod ids are not rows
//!
//! `minecolonies`, `domum_ornamentum`, `structurize` and `multipiston` are the
//! mod's own content. Their ids cannot be in a vanilla registry and they are not
//! missing: [`crate::compat::resolve`] answers
//! [`crate::compat::Resolution::Foreign`] for them, so the audit says "a mod must
//! bring this", never "26.3 lost this".

/// Which way one id moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// The block still exists in 26.3, under another name.
    Renamed,
    /// The block no longer exists in 26.3 at all. **No row carries it**: all 860
    /// `minecraft` ids of the palette were found once the two renames below were
    /// applied, so nothing disappeared. The variant exists so a future
    /// disappearance has a place to be written down instead of being forced into
    /// a rename.
    Disappeared,
    /// No proof either way. Never guessed into a rename; reported as unproven by
    /// [`crate::compat::states`].
    Uncertain,
}

/// Where the 26.3 side of every row is written down: the pinned client's exported
/// registry, one key per block under `["minecraft:block"]["entries"]` (1 286 of
/// them). A rename is only a row if its target is one of those keys and its 1.20.1
/// source is not.
pub const REGISTRY_26_3: &str =
    ".local/horizon-client/26.3/registries.json, [\"minecraft:block\"][\"entries\"] (1 286 keys)";

/// One id of the 1.20.1 palette and what 26.3 does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rename {
    /// The name the blueprint writes, in the 1.20.1 palette.
    pub from_1_20_1: &'static str,
    pub disposition: Disposition,
    /// The 26.3 name, as found in [`REGISTRY_26_3`] — `Some` exactly when
    /// `disposition` is [`Disposition::Renamed`].
    pub to_26_3: Option<&'static str>,
    /// The version that introduced it, as the wiki dates it.
    pub since: &'static str,
    /// Where that is written down. A row without this is an invention.
    pub source: &'static str,
    /// What a reader would otherwise get wrong.
    pub note: &'static str,
}

/// The whole table. Two rows, both proven.
pub const RENAMES: &[Rename] = &[
    Rename {
        from_1_20_1: "minecraft:grass",
        disposition: Disposition::Renamed,
        to_26_3: Some("minecraft:short_grass"),
        since: "1.20.3 (1.20.3 Pre-Release 1)",
        source: "minecraft.wiki/w/Short_Grass § Data history: \u{201c}The ID of \u{2018}Grass\u{2019}, \
                 now \u{2018}Short Grass\u{2019}, has been changed from grass to short_grass\u{201d}; \
                 § History: 1.20.3, pre1, \u{201c}Grass has been renamed to Short Grass\u{201d}",
        note: "The plant, not the block: 1.20.1's grass is the short plant shears collects, and \
               grass_block kept its name (so it is not a row here). The plant has no block state \
               and neither does short_grass, so the 1 715 palette entries survive untouched.",
    },
    Rename {
        from_1_20_1: "minecraft:chain",
        disposition: Disposition::Renamed,
        to_26_3: Some("minecraft:iron_chain"),
        since: "1.21.9 (25w35a)",
        source: "minecraft.wiki/w/Iron_Chain § Data history: \u{201c}Renamed chain to iron_chain\u{201d} \
                 (1.21.9, 25w35a); § History: 25w32a renames the display name to Iron Chain, when \
                 iron and copper chains were split apart",
        note: "Same shape, same state: iron_chain declares axis (x, y, z) and waterlogged \
               (true, false) — exactly the four values the pack's 1 313 chain entries write \
               (axis x/y/z, waterlogged false). The block did not lose a property.",
    },
];

/// One measured absence: the id, and how much of the pack depends on it.
///
/// `palette_entries` counts palette entries (a file listing one state twice counts
/// twice), `files` the blueprints naming the id at all, `cells` the structure
/// cells it fills, read from the `blocks` index array.
#[derive(Clone, Copy, Debug)]
pub struct Absent {
    pub id: &'static str,
    pub palette_entries: usize,
    pub files: usize,
    pub cells: u64,
}

/// The measurement itself — what the table must cover, and nothing else.
pub const MEASURED_ABSENT: &[Absent] = &[
    Absent {
        id: "minecraft:grass",
        palette_entries: 1_715,
        files: 1_715,
        cells: 6_192,
    },
    Absent {
        id: "minecraft:chain",
        palette_entries: 1_313,
        files: 1_196,
        cells: 3_123,
    },
];

/// The measurement behind [`MEASURED_ABSENT`], as numbers a test can re-derive.
///
/// The digests are FNV-1a 64 ([`crate::blueprint::fnv1a64`]) over the ids sorted
/// ascending and joined by a single `\n`, no trailing newline: the palette digest
/// over all 1 017 distinct ids, the `minecraft` one over the 860 of them, the
/// registry one over the 1 286 keys of `["minecraft:block"]["entries"]`. A
/// changed pack or a changed build shows up as a number, not as a surprise.
#[derive(Clone, Copy, Debug)]
pub struct Measured {
    pub files: usize,
    pub palette_entries: usize,
    pub distinct_ids: usize,
    pub minecraft_ids: usize,
    pub foreign_ids: usize,
    pub registry_blocks: usize,
    pub palette_digest: u64,
    pub minecraft_digest: u64,
    pub registry_digest: u64,
}

pub const MEASURED: Measured = Measured {
    files: 9_515,
    palette_entries: 612_400,
    distinct_ids: 1_017,
    minecraft_ids: 860,
    foreign_ids: 157,
    registry_blocks: 1_286,
    palette_digest: 0x972a_761e_ae85_1a4e,
    minecraft_digest: 0xbb68_9a70_e206_b72d,
    registry_digest: 0x7678_fe47_28f7_8011,
};

/// The mod namespaces the palette uses, and how many distinct ids each holds.
pub const FOREIGN_NAMESPACES: &[(&str, usize)] = &[
    ("domum_ornamentum", 83),
    ("minecolonies", 69),
    ("structurize", 4),
    ("multipiston", 1),
];

/// The row for `id`, if the table has one. Exact match: `minecraft:chain` must
/// not answer for `minecraft:chain_command_block`.
pub fn find(id: &str) -> Option<&'static Rename> {
    RENAMES.iter().find(|row| row.from_1_20_1 == id)
}

/// Every row with a given disposition, for a caller that wants to report them.
pub fn rows_with(disposition: Disposition) -> impl Iterator<Item = &'static Rename> {
    RENAMES.iter().filter(move |row| row.disposition == disposition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{fnv1a64, Blueprint};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// The pack inside the crate, the tree `src/blueprint/corpus.rs` reads too.
    const PACK: &str = "data/minecolonies/blueprints/minecolonies";

    /// The block registry of the pinned client, when the build tree is on this
    /// machine. It is an export of the game, not content this mod ships, so a tree
    /// without it skips — never fails.
    fn registry_26_3() -> Option<BTreeSet<String>> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(path) = std::env::var("HORIZON_REGISTRY_26_3") {
            candidates.push(PathBuf::from(path));
        }
        for path in [
            "/home/zaccou/horizon-modapi/.local/horizon-client/26.3/registries.json",
            "/workspace/horizon/.local/horizon-client/26.3/registries.json",
        ] {
            candidates.push(PathBuf::from(path));
        }
        let path = candidates.into_iter().find(|candidate| candidate.is_file())?;
        let text = std::fs::read_to_string(path).ok()?;
        block_keys(&text).map(|keys| keys.into_iter().collect())
    }

    /// The keys of the `"minecraft:block"` section's `"entries"` object.
    ///
    /// A depth walk instead of a parser: the file is one generated document and a
    /// registry id carries no escape. A malformed excerpt answers `None` so the
    /// caller skips rather than lies.
    fn block_keys(json: &str) -> Option<Vec<String>> {
        let section = json.find("\"minecraft:block\":")?;
        let entries = section + json[section..].find("\"entries\":")?;
        let open = entries + json[entries..].find('{')?;
        let bytes = json.as_bytes();
        let mut keys = Vec::new();
        let mut depth: usize = 0;
        let mut index = open;
        while index < bytes.len() {
            match bytes[index] {
                b'"' => {
                    let mut end = index + 1;
                    while end < bytes.len() && bytes[end] != b'"' {
                        if bytes[end] == b'\\' {
                            end += 1;
                        }
                        end += 1;
                    }
                    let text = json.get(index + 1..end)?.to_owned();
                    let mut after = end + 1;
                    while after < bytes.len()
                        && matches!(bytes[after], b' ' | b'\t' | b'\n' | b'\r')
                    {
                        after += 1;
                    }
                    // An entry key sits at depth 1 and is followed by a colon; the
                    // `protocol_id` of a value sits at depth 2.
                    if depth == 1 && after < bytes.len() && bytes[after] == b':' {
                        keys.push(text);
                    }
                    index = end + 1;
                }
                b'{' | b'[' => {
                    depth += 1;
                    index += 1;
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                    index += 1;
                }
                _ => index += 1,
            }
        }
        if keys.is_empty() { None } else { Some(keys) }
    }

    fn pack_paths() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let entries = std::fs::read_dir(dir)
                .unwrap_or_else(|error| panic!("blueprint pack {}: {error}", dir.display()));
            for entry in entries {
                let path = entry.expect("blueprint pack entry").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.to_string_lossy().ends_with(".blueprint") {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join(PACK), &mut out);
        out.sort();
        out
    }

    fn parse(path: &Path) -> Blueprint {
        let bytes =
            std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        Blueprint::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error:?}", path.display()))
    }

    fn digest(ids: &BTreeSet<String>) -> u64 {
        let joined: Vec<&str> = ids.iter().map(String::as_str).collect();
        fnv1a64(joined.join("\n").as_bytes())
    }

    #[test]
    fn the_measurement_is_internally_consistent() {
        assert_eq!(
            MEASURED.minecraft_ids + MEASURED.foreign_ids,
            MEASURED.distinct_ids,
            "every distinct id is either vanilla or a mod's"
        );
        assert_eq!(
            FOREIGN_NAMESPACES.iter().map(|(_, count)| count).sum::<usize>(),
            MEASURED.foreign_ids,
            "the four namespaces account for all 157 foreign ids"
        );
        let absent: usize = MEASURED_ABSENT.iter().map(|row| row.palette_entries).sum();
        assert!(absent < MEASURED.palette_entries, "two ids is a small hole");
        assert!(MEASURED_ABSENT.iter().all(|row| row.files <= MEASURED.files));
        for digest in [
            MEASURED.palette_digest,
            MEASURED.minecraft_digest,
            MEASURED.registry_digest,
        ] {
            assert_ne!(digest, 0);
        }
        assert_ne!(MEASURED.palette_digest, MEASURED.registry_digest);
    }

    #[test]
    fn the_table_covers_the_measured_set_and_adds_nothing() {
        let mut measured: Vec<&str> = MEASURED_ABSENT.iter().map(|row| row.id).collect();
        measured.sort_unstable();
        let mut tabulated: Vec<&str> = RENAMES.iter().map(|row| row.from_1_20_1).collect();
        tabulated.sort_unstable();
        assert_eq!(
            measured, tabulated,
            "a row without a measurement is an invention, a measured id without a row is a \
             blueprint that cannot be placed"
        );
        assert_eq!(
            measured[..],
            ["minecraft:chain", "minecraft:grass"],
            "the two known cases of the brief are the whole set"
        );
    }

    #[test]
    fn neither_known_case_is_forgotten_and_a_lookup_stays_exact() {
        assert_eq!(
            find("minecraft:grass").and_then(|row| row.to_26_3),
            Some("minecraft:short_grass")
        );
        assert_eq!(
            find("minecraft:chain").and_then(|row| row.to_26_3),
            Some("minecraft:iron_chain")
        );
        for neighbour in [
            "minecraft:chain_command_block",
            "minecraft:grass_block",
            "minecraft:chainmail_chestplate",
            "structurize:blocksubstitution",
            "",
        ] {
            assert!(find(neighbour).is_none(), "{neighbour} is not a row");
        }
    }

    #[test]
    fn every_row_is_proven_and_nothing_is_a_guess() {
        assert!(REGISTRY_26_3.contains("registries.json"), "the 26.3 side cites no file");
        for row in RENAMES {
            assert!(row.since.chars().any(|character| character.is_ascii_digit()));
            assert!(row.source.contains("minecraft.wiki/"), "{} has no page", row.from_1_20_1);
            assert!(row.note.len() > 40, "{} says nothing worth reading", row.from_1_20_1);
        }
        assert!(
            rows_with(Disposition::Uncertain).next().is_none(),
            "both renames are dated and sourced"
        );
        assert!(
            rows_with(Disposition::Disappeared).next().is_none(),
            "all 860 vanilla ids of the palette exist in 26.3 once renamed, so nothing is gone"
        );
    }

    #[test]
    fn a_rename_is_never_read_backwards() {
        let mut froms = BTreeSet::new();
        for row in RENAMES {
            let target = row.to_26_3.expect("a renamed row has a target");
            assert!(find(target).is_none(), "{target} is a 26.3 name, never a source");
            assert_ne!(target, row.from_1_20_1);
            assert!(target.starts_with("minecraft:"));
            assert!(froms.insert(row.from_1_20_1), "no id is tabulated twice");
        }
    }

    #[test]
    fn the_real_pack_only_lacks_ids_the_table_covers() {
        let registry = registry_26_3();
        let paths = pack_paths();
        assert_eq!(paths.len(), MEASURED.files, "the pack ships 9 515 blueprints");

        // A stride, like `corpus::sample`: 25 styles × the building kinds, so every
        // `len / 120`-th file walks all of them and the sample is the same each run.
        let stride = (paths.len() / 120).max(1);
        let (mut parsed, mut saw_grass, mut saw_chain) = (0usize, false, false);
        let mut absent = BTreeSet::new();
        for path in paths.iter().step_by(stride).take(120) {
            let blueprint = parse(path);
            parsed += 1;
            for state in &blueprint.palette {
                saw_grass |= state.name == "minecraft:grass";
                saw_chain |= state.name == "minecraft:chain";
                if let Some(registry) = registry.as_ref() {
                    if state.name.starts_with("minecraft:") && !registry.contains(&state.name) {
                        absent.insert(state.name.clone());
                    }
                }
            }
        }
        assert_eq!(parsed, 120);
        assert!(saw_grass && saw_chain, "the sample must hold both known cases");

        if registry.is_none() {
            return; // no exported registry here: "absent from 26.3" cannot be asked
        }
        assert!(!absent.is_empty(), "a sample lacking nothing would make this vacuous");
        for id in &absent {
            assert!(find(id).is_some(), "{id} is missing from 26.3 and has no row");
        }
    }

    #[test]
    fn the_registry_of_the_pinned_client_confirms_both_targets() {
        let Some(registry) = registry_26_3() else {
            return;
        };
        assert_eq!(registry.len(), MEASURED.registry_blocks);
        assert_eq!(digest(&registry), MEASURED.registry_digest);
        for row in RENAMES {
            let target = row.to_26_3.expect("a renamed row has a target");
            assert!(registry.contains(target), "{target} is not a block of 26.3");
            assert!(
                !registry.contains(row.from_1_20_1),
                "{} is still a block of 26.3, so the row is wrong",
                row.from_1_20_1
            );
        }
        assert!(
            registry.contains("minecraft:grass_block"),
            "the block that kept its name is still there: the two renames are not a prefix match"
        );
    }

    #[test]
    #[ignore = "reads all 9 515 blueprints; run with --ignored to re-derive the measurement"]
    fn the_whole_pack_gives_the_recorded_numbers() {
        let mut ids = BTreeSet::new();
        let mut entries = 0usize;
        let paths = pack_paths();
        for path in &paths {
            let blueprint = parse(path);
            entries += blueprint.palette.len();
            for state in &blueprint.palette {
                ids.insert(state.name.clone());
            }
        }
        assert_eq!(paths.len(), MEASURED.files);
        assert_eq!(entries, MEASURED.palette_entries);
        assert_eq!(ids.len(), MEASURED.distinct_ids);
        assert_eq!(digest(&ids), MEASURED.palette_digest);
        let minecraft: BTreeSet<String> = ids
            .iter()
            .filter(|id| id.starts_with("minecraft:"))
            .cloned()
            .collect();
        assert_eq!(minecraft.len(), MEASURED.minecraft_ids);
        assert_eq!(digest(&minecraft), MEASURED.minecraft_digest);

        if let Some(registry) = registry_26_3() {
            let absent: Vec<&String> =
                minecraft.iter().filter(|id| !registry.contains(*id)).collect();
            assert_eq!(
                absent.len(),
                MEASURED_ABSENT.len(),
                "the whole pack lacks exactly the measured ids: {absent:?}"
            );
            for id in absent {
                assert!(find(id).is_some(), "{id} has no row");
            }
        }
    }
}
