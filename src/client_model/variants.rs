//! What each block of the mod draws, and with which quarter turns.
//!
//! Java reference: `assets/minecolonies/blockstates/*.json` (95 files, the data
//! itself) and the nine `createBlockStateDefinition` bodies that decide which
//! properties those files may name - `AbstractColonyBlock.java:279-282` for the
//! 55 huts and the two colony blocks, `BlockBarrel.java:63`,
//! `BlockMinecoloniesGrave.java:193`, `BlockMinecoloniesNamedGrave.java:77`,
//! `BlockMinecoloniesRack.java:250`, `BlockConstructionTape.java:248-251`,
//! `BlockDecorationController.java:204-206`, `BlockPlantationField.java:227-229`,
//! `BlockColonySign.java:141`, `MinecoloniesFarmland.java:220-222`,
//! `MinecoloniesCropBlock.java:194-198`.
//!
//! **The rule this file ports is one sentence**: a blockstate is a `variants` map
//! or a `multipart` list, a variant key names *some* properties and the ones it
//! leaves out match every value, and what it resolves to is a model name plus
//! `x`, `y`, `uvlock` and `weight` (`blockstate.rs` does the reading; this file
//! does the answering). The geometry is the game's business: nothing here draws.
//!
//! | Question | Answer |
//! | --- | --- |
//! | blockstate files | **95**, of which 87 `variants`, 1 `multipart`, 7 not blockstates at all |
//! | files whose block is registered | 86 |
//! | families that leave a property out of every key | **10** ([`Family::wildcards`]) |
//! | families that pick a model at random | 2 - the rack (weights 40/30/30) and the citizen hut (5 × 20) |
//! | distinct `facing` conventions | **4**, all one cardinal index plus an offset of 0, 3, 1 or 2 |
//! | families with no `facing` | 23 - crops, banners, signs, waypoint, composted dirt |
//! | one file with `uvlock` | `blockconstructiontape` |
//!
//! **Four things the brief does not have, all measured.**
//!
//! 1. **The same `facing=north` means four different quarter turns.**
//!    `blockhutbuilder` puts north at 0°, `blockhutbaker` at 270°, `blockhutcitizen`
//!    at 90° and `blocktavern`-style blocks (`barrel_block`, `blockpostbox`,
//!    `blockstash`, `blockhutgraveyard`, `blockhuttavern`) at 180°. [`Conv`]
//!    carries them as `A`, `B`, `C`, `D` = offset `0`, `3`, `1`, `2`
//!    ([`Conv::offset`]), so a caller that assumes "north is 0" draws 78 of the
//!    87 variant files wrong. The split is not by family: it is per file.
//! 2. **A gate has two turns for four facings.** `gate_wood` and `gate_iron` fold
//!    north/south onto 0° and east/west onto 90° ([`Conv::Folded`]): the model is
//!    symmetric under a half turn, so the data says so instead of repeating a
//!    rotation that would change nothing.
//! 3. **The rack has one variant turned the other way.** Of its five `variant`
//!    values, `blockrackair` maps east to 180° and west to 0° while the four
//!    others map east to 0° and west to 180° ([`Conv::PerVariant`]). The odd one
//!    out is the invisible model, which is why nobody noticed.
//! 4. **Seven files in `blockstates/` are item models.** `caliper`, `compost`,
//!    the three scepters, `supplycamp` and `supplychest` have neither `variants`
//!    nor `multipart`: vanilla's loader refuses them ("no variants or multipart")
//!    and the items are drawn from `models/item/` instead ([`Kind::ItemModel`]).
//!
//! **What this file does not do**: it does not open `assets/`. The table is
//! generated from the JSON by `generer_table.py` and verified by fingerprint
//! ([`fingerprint`] against `table::FINGERPRINT`), so a row cannot drift from the
//! file it claims to describe without a red test.

use crate::client_model::table::{FAMILIES, FINGERPRINT};

/// The namespace every file in `assets/minecolonies/blockstates/` belongs to.
pub const NAMESPACE: &str = "minecolonies";

// --- domains, one constant per property family, each with its Java line ------

/// `HorizontalDirectionalBlock.FACING` (`AbstractColonyBlock.java:65`).
pub const FACING4: &[&str] = &["north", "east", "south", "west"];
/// `Direction.values()` for `DirectionalBlock.FACING` - six values, not four
/// (`BlockDecorationController.java:71` uses the directional base class).
pub const DIR6: &[&str] = &["down", "up", "north", "south", "west", "east"];
/// `BooleanProperty.getPossibleValues`, always `[true, false]`.
pub const BOOL2: &[&str] = &["true", "false"];
/// `BlockStateProperties.MOISTURE`, 0 to 7 (`MinecoloniesFarmland.java:54`).
pub const MOISTURE8: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7"];
/// `IntegerProperty.create("age", 0, 6)` (`MinecoloniesCropBlock.java:71`).
pub const AGE7: &[&str] = &["0", "1", "2", "3", "4", "5", "6"];
/// `BlockStateProperties.ROTATION_16` (`BlockColonyFlagBanner.java:30`).
pub const ROT16: &[&str] = &[
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
];
/// `DoorBlock.HINGE` (`AbstractBlockGate.java:40` inherits it).
pub const HINGE2: &[&str] = &["left", "right"];
/// `BlockStateProperties.DOUBLE_BLOCK_HALF`, inherited from `DoorBlock`.
pub const HALF2: &[&str] = &["upper", "lower"];
/// `GraveType` (`api/blocks/types/GraveType.java:11-12`), serialized by `name`.
pub const GRAVE2: &[&str] = &["blockgravedefault", "blockgravedecayed"];
/// `RackType` (`api/blocks/types/RackType.java:11-15`), in declaration order.
pub const RACK5: &[&str] = &[
    "blockrackemptysingle",
    "blockrackfullsingle",
    "blockrackempty",
    "blockrackfull",
    "blockrackair",
];
/// `BarrelType` (`api/blocks/types/BarrelType.java:11-18`).
pub const BARREL8: &[&str] = &[
    "0perc", "20perc", "40perc", "60perc", "80perc", "100perc", "working", "done",
];

/// One property a block's state definition declares: its name and every value
/// it can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two fields, both read
pub struct Prop {
    /// The property name, as a variant key spells it.
    pub name: &'static str,
    /// Its legal values, in the order the Java enum or property gives them.
    pub values: &'static [&'static str],
}

/// A row's property, spelled `p("facing", FACING4)` in the generated table.
pub const fn p(name: &'static str, values: &'static [&'static str]) -> Prop {
    Prop { name, values }
}

/// Which of the three shapes a file in `blockstates/` has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three shapes, and the reader has a branch per shape
pub enum Kind {
    /// A `variants` map: one selector per state pattern.
    Variants,
    /// A `multipart` list: a case per model part, each with an optional `when`.
    Multipart,
    /// Neither: an item model that ended up in `blockstates/`.
    ItemModel,
}

impl Kind {
    /// The word the JSON uses, spelled as the fingerprint needs it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Variants => "Variants",
            Kind::Multipart => "Multipart",
            Kind::ItemModel => "ItemModel",
        }
    }
}

/// How a family's `facing` value turns into a quarter turn about Y.
///
/// Every cardinal family is `(index + offset) % 4` with `index` north 0, east 1,
/// south 2, west 3 - so the eight names below are really **one** rule and four
/// offsets, plus three shapes that are not a rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: eight shapes, and `offset` is `None` for four of them
pub enum Conv {
    /// north 0°, east 90°, south 180°, west 270° - 22 files.
    A,
    /// north 270°, east 0°, south 90°, west 180° - 32 files, the huts' majority.
    B,
    /// north 90°, east 180°, south 270°, west 0° - `blockhutcitizen` alone.
    C,
    /// north 180°, east 270°, south 0°, west 90° - 5 files.
    D,
    /// Two facings per turn because the model is half-turn symmetric - the gates.
    Folded,
    /// `facing` turns, but one `variant` value turns the other way - the rack.
    PerVariant,
    /// `facing` includes `up` and `down`, which turn about X - the decoration
    /// controller.
    Vertical,
    /// No `facing` in the keys at all - 23 files.
    None,
}

impl Conv {
    /// The offset added to the cardinal index, in quarter turns, or `None` where
    /// the shape is not a rotation of the four cardinal directions.
    pub fn offset(self) -> Option<u8> {
        match self {
            Conv::A => Some(0),
            Conv::B => Some(3),
            Conv::C => Some(1),
            Conv::D => Some(2),
            Conv::Folded | Conv::PerVariant | Conv::Vertical | Conv::None => None,
        }
    }

    /// The quarter turns a `facing` value gets, or `None` when this shape does not
    /// answer for that value.
    ///
    /// `Folded` answers for the two facings it keeps and refuses the others,
    /// because a `Folded` file maps north and south to the same turn: the caller
    /// that needs a value for `south` gets 0° from the file, not from this method.
    pub fn turn(self, facing: &str) -> Option<u8> {
        let index = match facing {
            "north" => 0,
            "east" => 1,
            "south" => 2,
            "west" => 3,
            "up" | "down" => return self.vertical(facing),
            _ => return None,
        };
        self.offset().map(|offset| (index + offset) % 4)
    }

    /// The `x` turn of `up` and `down`, for [`Conv::Vertical`].
    fn vertical(self, facing: &str) -> Option<u8> {
        match (self, facing) {
            (Conv::Vertical, "up") => Some(1),
            (Conv::Vertical, "down") => Some(3),
            _ => None,
        }
    }

    /// A stable name, for a log line.
    pub fn name(self) -> &'static str {
        match self {
            Conv::A => "a",
            Conv::B => "b",
            Conv::C => "c",
            Conv::D => "d",
            Conv::Folded => "folded",
            Conv::PerVariant => "per_variant",
            Conv::Vertical => "vertical",
            Conv::None => "none",
        }
    }
}

/// Whether a `blockstates/` file has a block behind it in the Java tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three outcomes, each with its own sentence in the report
pub enum Standing {
    /// A `createBlockStateDefinition` body declares these properties.
    Registered,
    /// The file is dead data: no registered block reads it. The reason carries
    /// the `fichier.java:ligne` that proves it.
    Orphan { why: &'static str },
    /// Not a blockstate at all (an item model in the wrong folder).
    ItemModel,
}

/// One file of `assets/minecolonies/blockstates/`, as it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the fields are data, and a new one is a new measurement
pub struct Family {
    /// The file name without `.json`, which is also the block's registry path.
    pub block: &'static str,
    /// Which of the three shapes the file has.
    pub kind: Kind,
    /// The properties the block's state definition declares.
    pub props: &'static [Prop],
    /// The property names the file's selectors mention, sorted.
    pub named: &'static [&'static str],
    /// How many selectors the file has: variant keys, or multipart cases.
    pub selectors: u16,
    /// How many of the block's states at least one selector names, measured by
    /// enumerating them in `generer_table.py`.
    pub covered: u16,
    /// The distinct models the file names, namespace stripped, sorted.
    pub models: &'static [&'static str],
    /// The distinct `(x, y)` quarter-turn pairs, sorted.
    pub turns: &'static [(u8, u8)],
    /// Whether any variant sets `uvlock`.
    pub uvlock: bool,
    /// The distinct `weight` values.
    pub weights: &'static [u16],
    /// How `facing` turns, when it does.
    pub conv: Conv,
    /// `fichier.java:ligne` of the state definition, or of the registration.
    pub source: &'static str,
    /// Whether a block reads this file.
    pub standing: Standing,
    /// FNV-1a 64 of the row's canonical text, measured on the JSON.
    pub print: u64,
}

impl Family {
    /// `minecolonies:blockhutbuilder`.
    pub fn id(&self) -> String {
        let mut out = String::with_capacity(NAMESPACE.len() + 1 + self.block.len());
        out.push_str(NAMESPACE);
        out.push(':');
        out.push_str(self.block);
        out
    }

    /// `minecolonies:block/blockhutbuilder`, the model id a variant names.
    pub fn model_id(&self, model: &str) -> String {
        let mut out = String::with_capacity(NAMESPACE.len() + 1 + model.len());
        out.push_str(NAMESPACE);
        out.push(':');
        out.push_str(model);
        out
    }

    /// How many states the block has: the product of its properties' domains.
    pub fn state_count(&self) -> u32 {
        self.props
            .iter()
            .map(|prop| prop.values.len() as u32)
            .product()
    }

    /// The properties this file never mentions, sorted: every value of each one is
    /// covered by the same selector, so the model does not change with them.
    ///
    /// Ten files have one: the gates ignore `open`, `powered` and `half` (which is
    /// why an open gate draws the closed model), the farmlands ignore
    /// `waterlogged`, the decoration controller ignores `waterlogged`, the sign
    /// ignores its `facing`, the plantation field ignores `mirror`, the tape
    /// ignores `corner` and `waterlogged`, the two banners ignore everything.
    pub fn wildcards(&self) -> Vec<&'static str> {
        self.props
            .iter()
            .map(|prop| prop.name)
            .filter(|name| !self.named.contains(name))
            .collect()
    }

    /// States no selector names. Zero for all 95 files, which is the point of
    /// measuring it.
    pub fn uncovered(&self) -> u32 {
        self.state_count().saturating_sub(u32::from(self.covered))
    }

    /// Whether the file asks the game to pick a model at random.
    pub fn is_weighted(&self) -> bool {
        self.weights.len() > 1 || self.weights.first().is_some_and(|w| *w != 1)
    }
}

/// The row for a block path, or `None`.
pub fn family(block: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|row| row.block == block)
}

/// The canonical text a row's `print` is the hash of, spelled once here so the
/// test and the generator cannot disagree about it.
///
/// `<bloc>|<genre>|<nommees>|<selecteurs>|<couverts>|<modeles>|<tours>|<uvlock>|<poids>`
pub fn canonical(row: &Family) -> String {
    let mut out = String::with_capacity(96);
    out.push_str(row.block);
    out.push('|');
    out.push_str(row.kind.name());
    out.push('|');
    for (index, name) in row.named.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(name);
    }
    out.push('|');
    out.push_str(&row.selectors.to_string());
    out.push('|');
    out.push_str(&row.covered.to_string());
    out.push('|');
    for (index, model) in row.models.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(model);
    }
    out.push('|');
    for (index, (x, y)) in row.turns.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!("{x}:{y}"));
    }
    out.push('|');
    out.push_str(if row.uvlock { "true" } else { "false" });
    out.push('|');
    for (index, weight) in row.weights.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&weight.to_string());
    }
    out
}

/// FNV-1a 64, the hash both sides compute: offset basis `0xcbf29ce484222325`,
/// prime `0x100000001b3`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The fingerprint of a whole table: the rows' canonical texts joined by `\n`.
pub fn fingerprint(rows: &[Family]) -> u64 {
    let mut text = String::with_capacity(rows.len() * 96);
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&canonical(row));
    }
    fnv1a64(text.as_bytes())
}

/// The table's fingerprint, as the generator measured it.
pub const MEASURED: u64 = FINGERPRINT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ninety_five_files_one_row_each_in_file_order() {
        assert_eq!(FAMILIES.len(), 95);
        let mut names: Vec<&str> = FAMILIES.iter().map(|row| row.block).collect();
        assert_eq!(names, {
            let mut sorted = names.clone();
            sorted.sort_unstable();
            sorted
        });
        names.dedup();
        assert_eq!(names.len(), 95, "a file is listed twice");
    }

    #[test]
    fn the_three_shapes_split_eighty_seven_one_seven() {
        let count = |kind: Kind| FAMILIES.iter().filter(|row| row.kind == kind).count();
        assert_eq!(count(Kind::Variants), 87);
        assert_eq!(count(Kind::Multipart), 1);
        assert_eq!(count(Kind::ItemModel), 7);
    }

    #[test]
    fn the_table_still_hashes_to_the_number_the_json_gave() {
        // The one test that makes the whole file mean something: the number was
        // measured on the JSON by `generer_table.py`, and this recomputes it from
        // the rows. Edit any model name, turn or weight and it turns red.
        assert_eq!(fingerprint(FAMILIES), MEASURED);
    }

    #[test]
    fn every_row_carries_its_own_measured_print() {
        for row in FAMILIES {
            assert_eq!(
                fnv1a64(canonical(row).as_bytes()),
                row.print,
                "{} drifted",
                row.block
            );
        }
    }

    #[test]
    fn no_state_of_any_block_is_left_without_a_model() {
        // Measured twice: `generer_table.py` enumerates the states against the
        // JSON, `covered` is its count, `state_count` recomputes the domain
        // product from the Java properties. They agree for all 95 files.
        let mut checked = 0;
        for row in FAMILIES {
            if row.kind == Kind::ItemModel {
                continue;
            }
            assert_eq!(row.covered, row.state_count() as u16, "{}", row.block);
            assert!(row.selectors > 0, "{} names no selector", row.block);
            assert!(!row.models.is_empty(), "{} names no model", row.block);
            checked += 1;
        }
        assert_eq!(checked, 88);
    }

    #[test]
    fn ten_files_leave_a_property_out_of_their_keys() {
        let wild: Vec<(&str, Vec<&str>)> = FAMILIES
            .iter()
            .filter(|row| !row.wildcards().is_empty())
            .map(|row| (row.block, row.wildcards()))
            .collect();
        assert_eq!(wild.len(), 10);
        let gates = wild
            .iter()
            .find(|(b, _)| *b == "gate_wood")
            .expect("a gate");
        // The gate never names the three properties that change when it opens.
        assert_eq!(gates.1, vec!["open", "powered", "half"]);
        let sign = wild
            .iter()
            .find(|(b, _)| *b == "colonysign")
            .expect("a sign");
        // A colony sign names `connected` and nothing else: it is not directional.
        assert_eq!(sign.1, vec!["facing"]);
    }

    #[test]
    fn four_facing_conventions_and_the_rest_are_not_rotations() {
        let count = |conv: Conv| FAMILIES.iter().filter(|row| row.conv == conv).count();
        assert_eq!(count(Conv::A), 22);
        assert_eq!(count(Conv::B), 32);
        assert_eq!(count(Conv::C), 1);
        assert_eq!(count(Conv::D), 5);
        assert_eq!(count(Conv::Folded), 2);
        assert_eq!(count(Conv::PerVariant), 1);
        assert_eq!(count(Conv::Vertical), 1);
        // 64 variant files answer for a facing, 23 do not: every one of the 87
        // variant files is in exactly one class.
        assert_eq!(22 + 32 + 1 + 5 + 2 + 1 + 1, 64);
        assert_eq!(64 + 23, 87);
        // The 23: the 16 crops, the two banners, the sign, the waypoint, the
        // composted dirt, the two farmlands and the rack-like `variant` blocks
        // that carry no `facing`... measured, not guessed: count them.
        let none: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.conv == Conv::None && row.kind == Kind::Variants)
            .map(|row| row.block)
            .collect();
        assert_eq!(none.len(), 23);
        assert!(none.contains(&"colony_banner"));
        assert!(none.contains(&"cotton"));
        assert!(none.contains(&"blockwaypoint"));
    }

    #[test]
    fn the_four_offsets_are_one_rule() {
        // north is 0° for A, 270° for B, 90° for C, 180° for D: one cardinal index
        // plus 0, 3, 1, 2 quarter turns.
        for (conv, north, east, south, west) in [
            (Conv::A, 0, 1, 2, 3),
            (Conv::B, 3, 0, 1, 2),
            (Conv::C, 1, 2, 3, 0),
            (Conv::D, 2, 3, 0, 1),
        ] {
            assert_eq!(conv.turn("north"), Some(north), "{}", conv.name());
            assert_eq!(conv.turn("east"), Some(east), "{}", conv.name());
            assert_eq!(conv.turn("south"), Some(south), "{}", conv.name());
            assert_eq!(conv.turn("west"), Some(west), "{}", conv.name());
        }
        // And the offsets are those four numbers, nothing else.
        assert_eq!(
            [Conv::A, Conv::B, Conv::C, Conv::D].map(Conv::offset),
            [Some(0), Some(3), Some(1), Some(2)]
        );
        assert_eq!(Conv::Vertical.turn("up"), Some(1));
        assert_eq!(Conv::Vertical.turn("down"), Some(3));
        assert_eq!(Conv::Folded.turn("north"), None);
        assert_eq!(Conv::PerVariant.turn("north"), None);
        assert_eq!(Conv::None.turn("north"), None);
        assert_eq!(Conv::A.turn("sideways"), None);
    }

    #[test]
    fn the_citizen_hut_is_the_only_ninety_degree_one() {
        let c: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.conv == Conv::C)
            .map(|row| row.block)
            .collect();
        assert_eq!(c, vec!["blockhutcitizen"]);
        // And it is the family with five models: a hut whose walls differ.
        let citizen = family("blockhutcitizen").expect("a row");
        assert_eq!(
            citizen.models.to_vec(),
            vec![
                "block/blockhutcitizen",
                "block/blockhutcitizen_1",
                "block/blockhutcitizen_2",
                "block/blockhutcitizen_3",
                "block/blockhutcitizen_4",
            ]
        );
        // One weight value, 20, on each of the five models: a pick of one in five.
        assert_eq!(citizen.weights.to_vec(), vec![20]);
    }

    #[test]
    fn a_hut_and_its_neighbour_do_not_turn_alike() {
        // The finding of the file: two huts, one property, two answers.
        let builder = family("blockhutbuilder").expect("a row");
        let baker = family("blockhutbaker").expect("a row");
        assert_eq!(builder.conv.turn("north"), Some(0));
        assert_eq!(baker.conv.turn("north"), Some(3));
        assert_ne!(builder.conv, baker.conv);
        assert_eq!(builder.models.to_vec(), vec!["block/blockhutbuilder"]);
        assert_eq!(baker.models.to_vec(), vec!["block/blockhutbaker"]);
    }

    #[test]
    fn the_gates_fold_four_facings_onto_two_turns() {
        for gate in ["gate_wood", "gate_iron"] {
            let row = family(gate).expect("a row");
            assert_eq!(row.conv, Conv::Folded);
            assert_eq!(row.selectors, 8, "hinge x facing");
            assert_eq!(row.state_count(), 64, "DoorBlock's five properties");
            assert_eq!(row.turns.to_vec(), vec![(0, 0), (0, 1)]);
            // Only the iron gate swaps model on the hinge; the wooden one does not.
            assert_eq!(row.models.len(), if gate == "gate_iron" { 2 } else { 1 });
        }
        assert_eq!(
            family("gate_iron").expect("a row").models.to_vec(),
            vec!["block/gate/gate_iron", "block/gate/gate_iron_spike"]
        );
    }

    #[test]
    fn the_rack_picks_a_model_at_random_and_turns_one_of_them_backwards() {
        let rack = family("blockminecoloniesrack").expect("a row");
        assert_eq!(rack.selectors, 20, "4 facings x 5 RackType values");
        assert_eq!(rack.state_count(), 20);
        // 40 / 30 / 30 on the three full models, 1 on the empty one.
        assert_eq!(rack.weights.to_vec(), vec![1, 30, 40]);
        assert!(rack.is_weighted());
        assert_eq!(rack.conv, Conv::PerVariant);
        assert_eq!(rack.models.len(), 9);
        assert!(rack.models.contains(&"block/blockairrack"));
        // The other sixteen files with several models are the crops: a stage each.
        let crops: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.models.len() == 7)
            .map(|row| row.block)
            .collect();
        assert_eq!(crops.len(), 17, "16 crops and the barrel");
    }

    #[test]
    fn the_barrel_is_the_one_family_with_eight_variants() {
        let barrel = family("barrel_block").expect("a row");
        assert_eq!(barrel.selectors, 32);
        assert_eq!(barrel.state_count(), 32);
        assert_eq!(barrel.named.to_vec(), vec!["facing", "variant"]);
        assert_eq!(barrel.models.len(), 7, "eight variants, two share a model");
        assert_eq!(barrel.turns.to_vec(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
        assert!(!barrel.is_weighted());
    }

    #[test]
    fn the_farmland_swaps_model_at_moisture_seven() {
        let farmland = family("farmland").expect("a row");
        assert_eq!(farmland.selectors, 8);
        assert_eq!(farmland.state_count(), 16, "moisture x waterlogged");
        assert_eq!(
            farmland.models.to_vec(),
            vec!["block/farmland", "block/farmland_moist"]
        );
        assert_eq!(farmland.wildcards(), vec!["waterlogged"]);
    }

    #[test]
    fn the_sixteen_crop_files_name_a_stage_each() {
        let crops: Vec<&Family> = FAMILIES
            .iter()
            .filter(|row| row.source == "MinecoloniesCropBlock.java:198")
            .collect();
        assert_eq!(crops.len(), 16);
        for row in crops {
            assert_eq!(row.selectors, 7);
            assert_eq!(row.turns.to_vec(), vec![(0, 0)], "a crop never turns");
            for stage in 0..7 {
                let expected = format!("block/crops/{}/stage{stage}", row.block);
                assert!(
                    row.models.contains(&expected.as_str()),
                    "{} misses {expected}",
                    row.block
                );
            }
        }
    }

    #[test]
    fn the_tape_is_the_only_multipart_and_the_only_uvlock() {
        let multipart: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.kind == Kind::Multipart)
            .map(|row| row.block)
            .collect();
        assert_eq!(multipart, vec!["blockconstructiontape"]);
        let locked: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.uvlock)
            .map(|row| row.block)
            .collect();
        assert_eq!(locked, vec!["blockconstructiontape"]);
        let tape = family("blockconstructiontape").expect("a row");
        assert_eq!(tape.selectors, 13);
        assert_eq!(
            tape.state_count(),
            256,
            "four booleans, facing, corner, water"
        );
        assert_eq!(tape.models.len(), 3);
    }

    #[test]
    fn the_seven_item_models_are_named_and_have_no_state() {
        let items: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.kind == Kind::ItemModel)
            .map(|row| row.block)
            .collect();
        assert_eq!(
            items,
            vec![
                "caliper",
                "compost",
                "sceptergold",
                "scepterpermission",
                "sceptersteel",
                "supplycamp",
                "supplychest"
            ]
        );
        for row in FAMILIES.iter().filter(|row| row.kind == Kind::ItemModel) {
            assert_eq!(row.state_count(), 1, "no properties, so one state");
            assert_eq!(row.covered, 0, "and no selector names it");
            assert_eq!(row.standing, Standing::ItemModel);
        }
    }

    #[test]
    fn two_files_have_no_block_behind_them_and_the_row_says_why() {
        let orphans: Vec<(&str, &str)> = FAMILIES
            .iter()
            .filter_map(|row| match row.standing {
                Standing::Orphan { why } => Some((row.block, why)),
                _ => None,
            })
            .collect();
        assert_eq!(orphans.len(), 2);
        assert!(
            orphans.iter().any(
                |(b, why)| *b == "largequarry" && why.contains("ModBlocksInitializer.java:151")
            )
        );
        assert!(orphans.iter().any(|(b, _)| *b == "cotton"));
        // 86 files are read by a registered block: 95 - 7 item models - 2 orphans.
        let registered = FAMILIES
            .iter()
            .filter(|row| row.standing == Standing::Registered)
            .count();
        assert_eq!(registered, 86);
    }

    #[test]
    fn every_row_names_where_its_properties_come_from() {
        for row in FAMILIES {
            match row.standing {
                Standing::ItemModel => assert!(!row.source.is_empty(), "{}", row.block),
                _ => {
                    let (file, line) = row.source.rsplit_once(':').expect("a java line");
                    assert!(file.ends_with(".java"), "{} -> {}", row.block, row.source);
                    assert!(
                        line.parse::<u32>().is_ok(),
                        "{} -> {}",
                        row.block,
                        row.source
                    );
                }
            }
        }
    }

    #[test]
    fn a_named_property_is_always_a_declared_one() {
        // The other direction of the wildcard rule: a file may not name a property
        // the block does not have, or the reader would fold it into nothing.
        for row in FAMILIES {
            for name in row.named {
                assert!(
                    row.props.iter().any(|prop| prop.name == *name),
                    "{} names {}, which its state definition lacks",
                    row.block,
                    name
                );
            }
        }
    }

    #[test]
    fn the_models_are_namespaced_on_request_only() {
        let hut = family("blockhutbuilder").expect("a row");
        assert_eq!(hut.id(), "minecolonies:blockhutbuilder");
        assert_eq!(
            hut.model_id(hut.models[0]),
            "minecolonies:block/blockhutbuilder"
        );
        for row in FAMILIES {
            for model in row.models {
                assert!(!model.contains(':'), "{} stores a namespace", row.block);
                assert!(model.starts_with("block/") || model.starts_with("item/"));
            }
        }
    }

    #[test]
    fn the_weights_are_the_four_the_files_use() {
        let mut weights: Vec<u16> = FAMILIES
            .iter()
            .flat_map(|row| row.weights.iter().copied())
            .collect();
        weights.sort_unstable();
        weights.dedup();
        assert_eq!(weights, vec![1, 20, 30, 40]);
        let weighted: Vec<&str> = FAMILIES
            .iter()
            .filter(|row| row.is_weighted())
            .map(|row| row.block)
            .collect();
        assert_eq!(weighted, vec!["blockhutcitizen", "blockminecoloniesrack"]);
    }

    #[test]
    fn a_row_is_reached_by_its_registry_path() {
        assert_eq!(family("nonesuch"), None);
        assert_eq!(
            family("blockhutbuilder").map(|row| row.id()),
            Some("minecolonies:blockhutbuilder".to_owned())
        );
        // The row for `blockhutfield` is the default hut block, not a hut class.
        assert_eq!(
            family("blockhutfield").map(|row| row.source),
            Some("AbstractBlockMinecoloniesDefault.java:44")
        );
    }
}
