//! 1.20.1 names against 26.3 names: the table, the measurement, and one rule.
//!
//! Every blueprint of the pack names its blocks the 1.20.1 way. Measured on the
//! palettes of the 9 515 files, **two** ids are missing from 26.3 —
//! `minecraft:grass`, now `short_grass`, and `minecraft:chain`, now `iron_chain`
//! — and **no** block state was lost. [`table`] holds those renames with their
//! proof and the measurement they come from; [`states`] audits a palette against
//! whatever block report the caller can supply.
//!
//! # The target is 26.3, never the other way round
//!
//! Java's palette is the past and the engine is the present, so a rename here is
//! `1.20.1 → 26.3` and only that. No function of this crate returns a 1.20.1
//! name: a blueprint already written the 26.3 way is [`Resolution::Kept`], and
//! [`table::Rename`] names its fields `from_1_20_1` / `to_26_3` so the direction
//! cannot be read backwards. Neither old name is a block of 26.3, so the table
//! cannot be walked in reverse even by mistake — a test asserts it.
//!
//! # Two questions, kept apart
//!
//! | Question | Answered by | How |
//! | --- | --- | --- |
//! | which id changed, and on whose authority | [`table`] | a fixed, cited set the tests compare to the pack |
//! | can 26.3 express this exact state | [`states`] | an audit against a caller-supplied [`StateOracle`] |
//!
//! [`resolve`] answers the first alone: [`Resolution::Kept`] means "the table has
//! no row", not "26.3 certainly has this block" — the table is a list of proven
//! changes, not a copy of a 1 286-key registry. Proving a state placeable is
//! [`states::audit`]'s job, and it asks the game.
//!
//! # The pack's other 157 blocks are not losses
//!
//! 69 `minecolonies`, 83 `domum_ornamentum`, 4 `structurize` and 1 `multipiston`
//! ids belong to mods, so a vanilla registry could never hold them: [`resolve`]
//! answers [`Resolution::Foreign`] and the audit reports them as content a mod
//! must bring, never as a block 26.3 lost.
//!
//! | File | Question |
//! | --- | --- |
//! | `table` | which ids changed, since when, and where that is written down |
//! | `states` | which block states 26.3 cannot express, and what the audit reports |
//! | `RAPPORT.md` | the measurement, the two renames, the gaps left open |

pub mod states;
pub mod table;

use crate::blueprint::BlockState;

pub use states::{
    audit, audit_palette, Audit, BlockSpec, Gap, PropertySpec, Report, Rewrite, StateOracle,
    SPECS_26_3,
};
pub use table::{
    Absent, Disposition, Measured, Rename, MEASURED, MEASURED_ABSENT, REGISTRY_26_3, RENAMES,
};

/// What [`resolve`] says about one palette id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution<'a> {
    /// The table has no row: the name is used as written.
    Kept,
    /// Renamed in 26.3 — [`Resolution::target`] is the name to use.
    Renamed(&'static Rename),
    /// 26.3 has no equivalent: the block has to be substituted by hand.
    Gone(&'static Rename),
    /// The table cannot place it: neither a proven rename nor a proven removal.
    Uncertain(&'static Rename),
    /// A namespace that is not `minecraft`: a mod's own content.
    Foreign(&'a str),
}

impl Resolution<'_> {
    /// The 26.3 name to use, when there is one.
    pub fn target(&self) -> Option<&'static str> {
        self.row().and_then(|row| row.to_26_3)
    }

    /// The table row behind this answer, if it came from one.
    pub fn row(&self) -> Option<&'static Rename> {
        match self {
            Self::Renamed(row) | Self::Gone(row) | Self::Uncertain(row) => Some(*row),
            Self::Kept | Self::Foreign(_) => None,
        }
    }

    /// Needs a human decision before the cell can be placed.
    pub fn is_problem(&self) -> bool {
        matches!(self, Self::Gone(_) | Self::Uncertain(_))
    }
}

impl std::fmt::Display for Resolution<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kept => f.write_str("kept as written"),
            Self::Renamed(row) => write!(
                f,
                "renamed to {} ({}, {})",
                row.to_26_3.unwrap_or("?"),
                row.since,
                row.source
            ),
            Self::Gone(row) => {
                write!(f, "gone since {} — no 26.3 block ({})", row.since, row.source)
            }
            Self::Uncertain(row) => write!(f, "unproven ({}, {})", row.since, row.source),
            Self::Foreign(namespace) => write!(f, "{namespace}: provided by a mod, not by 26.3"),
        }
    }
}

/// The namespace of a block id, `minecraft` when it has none.
pub fn namespace_of(id: &str) -> &str {
    id.split_once(':')
        .map(|(namespace, _)| namespace)
        .unwrap_or("minecraft")
}

/// What the table says about a palette id. Exact match: `minecraft:chain` never
/// answers for `minecraft:chain_command_block`.
pub fn resolve(id: &str) -> Resolution<'_> {
    if namespace_of(id) != "minecraft" {
        return Resolution::Foreign(namespace_of(id));
    }
    match table::find(id) {
        Some(row) => match row.disposition {
            Disposition::Renamed => Resolution::Renamed(row),
            Disposition::Disappeared => Resolution::Gone(row),
            Disposition::Uncertain => Resolution::Uncertain(row),
        },
        None => Resolution::Kept,
    }
}

/// The 26.3 name of a 1.20.1 id, when the table renames it. Never the reverse:
/// `None` for a name already 26.3.
pub fn rename(id: &str) -> Option<&'static str> {
    match resolve(id) {
        Resolution::Renamed(row) => row.to_26_3,
        _ => None,
    }
}

/// The same state under its 26.3 name, **properties untouched**.
///
/// `None` when the table does not rename the block. The properties are copied as
/// they are on purpose: whether 26.3 still accepts them is [`states::audit`]'s
/// answer, and dropping one here would hide what the audit exists to report.
pub fn apply_to(state: &BlockState) -> Option<BlockState> {
    let target = rename(&state.name)?;
    Some(BlockState {
        name: target.to_owned(),
        properties: state.properties.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str, properties: &[(&str, &str)]) -> BlockState {
        BlockState {
            name: name.to_owned(),
            properties: properties
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn grass_is_now_short_grass() {
        let resolution = resolve("minecraft:grass");
        assert_eq!(resolution, Resolution::Renamed(table::find("minecraft:grass").unwrap()));
        assert_eq!(resolution.target(), Some("minecraft:short_grass"));
        assert!(!resolution.is_problem());
        assert!(resolution.to_string().contains("1.20.3"), "{resolution}");
    }

    #[test]
    fn chain_is_now_iron_chain() {
        let resolution = resolve("minecraft:chain");
        assert_eq!(resolution.target(), Some("minecraft:iron_chain"));
        assert!(resolution.to_string().contains("minecraft.wiki/"), "{resolution}");
    }

    #[test]
    fn a_26_3_name_is_kept_and_an_unknown_one_is_not_guessed() {
        for name in [
            "minecraft:short_grass",
            "minecraft:iron_chain",
            "minecraft:grass_block",
            "minecraft:stone",
            // an item: the table has nothing to say, and `states` is what refuses it
            "minecraft:bread",
        ] {
            assert_eq!(resolve(name), Resolution::Kept, "{name}");
            assert!(rename(name).is_none(), "{name} is not a 1.20.1 name");
            assert!(apply_to(&state(name, &[])).is_none(), "{name}");
        }
    }

    #[test]
    fn a_mod_namespace_is_foreign() {
        assert_eq!(
            resolve("structurize:blocksubstitution"),
            Resolution::Foreign("structurize")
        );
        assert_eq!(resolve("multipiston:multipistonblock").target(), None);
        assert!(resolve("structurize:blocksubstitution").to_string().contains("mod"));
    }

    #[test]
    fn apply_renames_the_block_and_keeps_every_property() {
        let chain = state("minecraft:chain", &[("axis", "y"), ("waterlogged", "false")]);
        let renamed = apply_to(&chain).expect("chain is a row");
        assert_eq!(renamed.name, "minecraft:iron_chain");
        assert_eq!(renamed.properties, chain.properties);
        assert_eq!(renamed.to_string(), "minecraft:iron_chain[axis=y,waterlogged=false]");
        assert_eq!(chain.name, "minecraft:chain", "the input is left as it was found");
    }

    #[test]
    fn a_state_with_no_namespace_is_read_as_minecraft() {
        assert_eq!(namespace_of("minecraft:stone"), "minecraft");
        assert_eq!(namespace_of("stone"), "minecraft");
        assert_eq!(namespace_of("domum_ornamentum:shingle"), "domum_ornamentum");
        assert_eq!(resolve("stone"), Resolution::Kept);
    }

    #[test]
    fn the_table_has_no_reverse_entry() {
        assert_eq!(RENAMES.len(), 2);
        for row in RENAMES {
            let target = row.to_26_3.expect("a renamed row has a target");
            assert!(table::find(target).is_none(), "{target} is a 26.3 name");
            assert_eq!(rename(target), None);
        }
    }
}
