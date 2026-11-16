//! The nine blocks that dress a colony: two gates, two banners, two graves, a
//! sign, a rack and a barrel.
//!
//! Java reference: `api/blocks/AbstractBlockMinecoloniesGrave.java`,
//! `AbstractBlockMinecoloniesNamedGrave.java`, `AbstractBlockMinecoloniesRack.java`,
//! `api/blocks/AbstractBlockBarrel.java`, `api/blocks/decorative/AbstractBlockGate.java`
//! and `AbstractColonyFlagBanner.java`, over their `core/blocks/` subclasses.
//!
//! **Three of the nine are colony blocks by behaviour without being one by
//! class.** The grave, the rack and the barrel all open a screen or take an item,
//! and two of them ask the colony for `ACCESS_HUTS` — but only outside a colony
//! they open for anyone (`BlockMinecoloniesRack.java:230-232`,
//! `BlockMinecoloniesGrave.java:155-157`). That "outside a colony" half is the
//! reason their click is not the huts' rule, and it is tested.
//!
//! **The gates are the one place in the mod where a block is many.** A gate is a
//! wall of door leaves: `setPlacedBy` fills the rectangle out to
//! `maxWidth`/`maxHeight` (`AbstractBlockGate.java:223-288`), `removeGate` takes
//! the whole wall down when one leaf is broken (`:100-148`), and `use` toggles
//! every leaf (`:92-97`). The two concrete blocks differ in three numbers and
//! nothing else — iron 10/6/8, wood 7/6/5 (`ModBlocksInitializer.java:125-126`).
//!
//! **Two state properties are integers and no content file can hold them.**
//! `ROTATION` (a standing banner's 16 positions, `BlockColonyFlagBanner.java:30`)
//! and the rack's `VARIANT` (which *is* an enum, so that one is fine) — the banner
//! is the case that matters, and its template carries the property
//! (`minecraft:white_banner`), which is why it is [`StateIn::Template`] here and
//! not a gap.

use super::{Block, Click, Family, Form, Props, State, StateIn, Values};

/// A horizontal facing, four values, default `north`: the grave's, the rack's, the
/// barrel's and the sign's (`AbstractBlockMinecoloniesGrave.java:16`,
/// `AbstractBlockMinecoloniesRack.java:18`, `AbstractBlockBarrel.java:19`,
/// `BlockColonySign.java:141`).
const FACING: State = State {
    name: "facing",
    values: Values::Enum(super::FACING_VALUES),
    default: "north",
    source: "AbstractBlockMinecoloniesGrave.java:16",
    content: StateIn::Declared,
};

/// The grave's two variants, `GraveType.DEFAULT` then `DECAYED`
/// (`api/blocks/types/GraveType.java:9-12`, declared at
/// `AbstractBlockMinecoloniesGrave.java:11`).
const GRAVE_VARIANT: State = State {
    name: "variant",
    values: Values::Enum(&["default", "decayed"]),
    default: "default",
    source: "api/blocks/AbstractBlockMinecoloniesGrave.java:11",
    content: StateIn::Declared,
};

/// The rack's five variants, in `RackType`'s order: `EMPTY`, `FULL`,
/// `EMPTY_DOUBLE`, `FULL_DOUBLE`, `NO_RENDER` (`api/blocks/types/RackType.java:13-17`,
/// declared at `AbstractBlockMinecoloniesRack.java:13`).
const RACK_VARIANT: State = State {
    name: "variant",
    values: Values::Enum(&[
        "emptysingle",
        "fullsingle",
        "empty",
        "full",
        "dontrender",
    ]),
    default: "emptysingle",
    source: "AbstractBlockMinecoloniesRack.java:13",
    content: StateIn::Declared,
};

/// The barrel's eight variants, in `BarrelType`'s order
/// (`api/blocks/types/BarrelType.java:9-16`).
const BARREL_VARIANT: State = State {
    name: "variant",
    values: Values::Enum(&[
        "0perc", "20perc", "40perc", "60perc", "80perc", "100perc", "working", "done",
    ]),
    default: "0perc",
    source: "api/blocks/AbstractBlockBarrel.java:14",
    content: StateIn::Declared,
};

/// A standing banner's `ROTATION_16`, `BlockStateProperties.ROTATION_16`
/// (`BlockColonyFlagBanner.java:30`), default 0 (`:36`). **An integer**: the
/// template brings it and no content file can declare one.
const BANNER_ROTATION: State = State {
    name: "rotation",
    values: Values::Integer { min: 0, max: 15 },
    default: "0",
    source: "core/blocks/decorative/BlockColonyFlagBanner.java:30",
    content: StateIn::Template("minecraft:white_banner"),
};

/// A wall banner's `HORIZONTAL_FACING`
/// (`BlockColonyFlagWallBanner.java:44`), which its template carries.
const WALL_FACING: State = State {
    name: "facing",
    values: Values::Enum(super::FACING_VALUES),
    default: "north",
    source: "core/blocks/decorative/BlockColonyFlagWallBanner.java:44",
    content: StateIn::Template("minecraft:white_wall_banner"),
};

/// The sign's `CONNECTED`, `BooleanProperty` declared in the class itself
/// (`BlockColonySign.java:46`), registered with the facing at `:141`.
const CONNECTED: State = State {
    name: "connected",
    values: Values::Boolean,
    default: "false",
    source: "BlockColonySign.java:46",
    content: StateIn::Declared,
};

/// The five states a `DoorBlock` declares — `FACING`, `OPEN`, `HALF`, `HINGE`,
/// `POWERED` — which a gate inherits with its superclass
/// (`AbstractBlockGate extends DoorBlock`, `:40`). The template carries all five
/// and only the sounds are the mod's (`:73`, `BlockSetType.SPRUCE`).
const DOOR_STATES: &[State] = &[
    State {
        name: "facing",
        values: Values::Enum(super::FACING_VALUES),
        default: "north",
        source: "AbstractBlockGate.java:40",
        content: StateIn::Template("minecraft:iron_door"),
    },
    State {
        name: "open",
        values: Values::Boolean,
        default: "false",
        source: "AbstractBlockGate.java:40",
        content: StateIn::Template("minecraft:iron_door"),
    },
    State {
        name: "half",
        values: Values::Enum(&["upper", "lower"]),
        default: "lower",
        source: "AbstractBlockGate.java:40",
        content: StateIn::Template("minecraft:iron_door"),
    },
    State {
        name: "hinge",
        values: Values::Enum(&["left", "right"]),
        default: "left",
        source: "AbstractBlockGate.java:40",
        content: StateIn::Template("minecraft:iron_door"),
    },
    State {
        name: "powered",
        values: Values::Boolean,
        default: "false",
        source: "AbstractBlockGate.java:40",
        content: StateIn::Template("minecraft:iron_door"),
    },
];

/// The nine blocks.
pub const BLOCKS: &[Block] = &[
    // `ModBlocksInitializer.java:125`: `new BlockGate(IRON_GATE, 10f, 6, 8)`.
    // Wood map colour and sound, `strength(10, 50)` (`AbstractBlockGate.java:73`).
    Block::plain(
        "gate_iron",
        "core/blocks/decorative/BlockGate.java:8",
        Family::Decoration,
        Form::Gate,
        Click::Gate,
        Props::wood(10.0, 50.0),
        DOOR_STATES,
    ),
    // `ModBlocksInitializer.java:126`: `new BlockGate(WOODEN_GATE, 7f, 6, 5)`.
    Block::plain(
        "gate_wood",
        "core/blocks/decorative/BlockGate.java:8",
        Family::Decoration,
        Form::Gate,
        Click::Gate,
        Props::wood(7.0, 35.0),
        DOOR_STATES,
    ),
    // Map colour WOOD, sound WOOD, `noCollission`, `strength(1F)`
    // (`AbstractColonyFlagBanner.java:39-44`). The name is
    // `REGISTRY_NAME = "colony_banner"` (`:32`), not the class's.
    Block::plain(
        "colony_banner",
        "core/blocks/decorative/BlockColonyFlagBanner.java:28",
        Family::Decoration,
        Form::Banner,
        Click::None,
        Props::wood(1.0, 1.0).thin(),
        &[BANNER_ROTATION],
    ),
    // `REGISTRY_NAME_WALL = "colony_wall_banner"` (`AbstractColonyFlagBanner.java:33`).
    Block::plain(
        "colony_wall_banner",
        "core/blocks/decorative/BlockColonyFlagWallBanner.java:32",
        Family::Decoration,
        Form::Banner,
        Click::None,
        Props::wood(1.0, 1.0).thin(),
        &[WALL_FACING],
    ),
    // Stone, 1.5/5, and it **drops its inventory** when it goes
    // (`BlockMinecoloniesGrave.java:68`, `:204-220`).
    Block::plain(
        "blockminecoloniesgrave",
        "core/blocks/BlockMinecoloniesGrave.java:44",
        Family::Decoration,
        Form::SmallBox,
        Click::Grave,
        Props::stone(1.5, 5.0),
        &[FACING, GRAVE_VARIANT],
    )
    .with_contents(),
    // Stone, 5/1, `noLootTable()` (`:51`): the name on it is the loot.
    Block::plain(
        "blockminecoloniesnamedgrave",
        "core/blocks/BlockMinecoloniesNamedGrave.java:32",
        Family::Decoration,
        Form::Full,
        Click::None,
        Props::stone(5.0, 1.0),
        &[FACING],
    ),
    // Wood, 5/1, `noCollission` (`BlockColonySign.java:73`). Breaking it takes a
    // connection node out of the colony (`:112-131`).
    Block::plain(
        "colonysign",
        "core/blocks/BlockColonySign.java:41",
        Family::Decoration,
        Form::SmallBox,
        Click::None,
        Props::wood(5.0, 1.0).thin(),
        &[FACING, CONNECTED],
    ),
    // Wood, 10/∞ (`BlockMinecoloniesRack.java:85`), and it drops its inventory
    // (`:269-288`).
    Block::plain(
        "blockminecoloniesrack",
        "core/blocks/BlockMinecoloniesRack.java:50",
        Family::Decoration,
        Form::Rack,
        Click::Rack,
        Props::wood(10.0, f32::INFINITY),
        &[FACING, RACK_VARIANT],
    )
    .with_contents(),
    // Wood, 5/1 (`BlockBarrel.java:50`), a 1.5-high box (`:96-99`).
    Block::plain(
        "barrel_block",
        "core/blocks/BlockBarrel.java:33",
        Family::Decoration,
        Form::Full,
        Click::Barrel,
        Props::wood(5.0, 1.0),
        &[FACING, BARREL_VARIANT],
    ),
];

/// Whether breaking this block takes a connection node out of the colony: the
/// colony sign alone (`BlockColonySign.java:112-131`, `removeConnectionNode`).
///
/// The node is what the colony's connection manager walks; the port has the
/// module key (`crate::building::behaviour::modules`) and not the manager, so the
/// rule is stated here for the wiring to call (`MANQUANT.md` §3).
pub fn removes_connection_node(block: &Block) -> bool {
    block.name == "colonysign"
}

/// The gate's biggest wall, in blocks: `maxWidth` and `maxHeight` are the
/// constructor's two numbers, and the two gates differ only in them and in the
/// hardness (`ModBlocksInitializer.java:125-126`).
pub fn gate_size(block: &Block) -> Option<(u8, u8)> {
    if block.click != Click::Gate {
        return None;
    }
    let iron = block.name == "gate_iron";
    Some(if iron { (6, 8) } else { (6, 5) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{Answer, Facts, Refusal, Window, all, block, click, kind_of};
    use crate::colony::permissions::Rank;

    fn b(name: &str) -> &'static Block {
        block(name).unwrap_or_else(|| panic!("{name} is a decoration block"))
    }

    #[test]
    fn the_nine_blocks_are_all_decorative_and_none_is_a_colony_block() {
        assert_eq!(BLOCKS.len(), 9);
        for block in BLOCKS {
            assert_eq!(block.family, Family::Decoration);
            assert!(block.building.is_none(), "{}", block.name);
            assert_eq!(
                kind_of(&block.path()),
                Err(Refusal::NotAColonyBlock { block: block.name })
            );
        }
        assert_eq!(all().filter(|b| b.family == Family::Decoration).count(), 9);
    }

    #[test]
    fn the_two_gates_differ_in_hardness_and_size_only() {
        let iron = b("minecolonies:gate_iron");
        let wood = b("minecolonies:gate_wood");
        assert_eq!(iron.states, wood.states, "the same door states");
        assert_eq!(iron.form, wood.form);
        assert_eq!(iron.props.hardness, 10.0);
        assert_eq!(iron.props.resistance, 50.0, "hardness * 5");
        assert_eq!(wood.props.hardness, 7.0);
        assert_eq!(wood.props.resistance, 35.0);
        assert_eq!(gate_size(iron), Some((6, 8)));
        assert_eq!(gate_size(wood), Some((6, 5)));
        assert_eq!(gate_size(b("minecolonies:barrel_block")), None);
    }

    #[test]
    fn a_gate_opens_and_closes_with_no_permission_in_the_block() {
        let facts = Facts::member();
        assert_eq!(
            click(b("minecolonies:gate_iron"), &facts),
            Answer::Toggle { property: "open" }
        );
        // No off hand, no colony, no rank: the block does not look at any of it
        // (`AbstractBlockGate.java:92-97`).
        let stranger_outside = Facts {
            off_hand: true,
            building: false,
            building_has_colony: false,
            colony_covers: false,
            rank: Rank::Hostile,
            ..facts
        };
        assert_eq!(
            click(b("minecolonies:gate_wood"), &stranger_outside),
            Answer::Toggle { property: "open" }
        );
    }

    #[test]
    fn a_rack_and_a_grave_open_outside_a_colony_and_ask_inside_one() {
        let member = Facts::member();
        assert_eq!(
            click(b("minecolonies:blockminecoloniesrack"), &member),
            Answer::Open {
                window: Window::Rack,
                shift: false
            }
        );
        assert_eq!(
            click(b("minecolonies:blockminecoloniesgrave"), &member),
            Answer::Open {
                window: Window::Grave,
                shift: false
            }
        );
        // Outside a colony: anyone.
        let outside = Facts {
            colony_covers: false,
            ..member.stranger()
        };
        assert!(matches!(
            click(b("minecolonies:blockminecoloniesrack"), &outside),
            Answer::Open { window: Window::Rack, .. }
        ));
        assert!(matches!(
            click(b("minecolonies:blockminecoloniesgrave"), &outside),
            Answer::Open {
                window: Window::Grave,
                ..
            }
        ));
        // Inside one, a stranger is refused, and with the same refusal a hut gives.
        let refused = Facts {
            colony_covers: true,
            ..member.stranger()
        };
        assert_eq!(
            click(b("minecolonies:blockminecoloniesrack"), &refused),
            Answer::Refused(Refusal::LacksAccess {
                rank: Rank::Neutral
            })
        );
        // The named grave has no `use` at all: it is a name, not a container.
        assert_eq!(click(b("minecolonies:blockminecoloniesnamedgrave"), &member), Answer::Pass);
    }

    #[test]
    fn a_barrel_takes_the_held_stack_and_answers_nothing_without_its_entity() {
        let member = Facts::member();
        assert_eq!(click(b("minecolonies:barrel_block"), &member), Answer::Insert);
        let empty = Facts {
            block_entity: false,
            ..member
        };
        assert_eq!(click(b("minecolonies:barrel_block"), &empty), Answer::Pass);
    }

    #[test]
    fn the_banner_s_rotation_is_an_integer_its_template_brings() {
        let banner = b("minecolonies:colony_banner");
        let rotation = banner.states[0];
        assert_eq!(rotation.name, "rotation");
        assert_eq!(rotation.values, Values::Integer { min: 0, max: 15 });
        assert!(rotation.values.names().is_empty(), "an integer has no names");
        assert!(rotation.values.allows("0") && rotation.values.allows("15"));
        assert!(!rotation.values.allows("16") && !rotation.values.allows("north"));
        assert_eq!(
            rotation.content,
            StateIn::Template("minecraft:white_banner"),
            "content cannot declare an integer: the template owns it"
        );
        // The wall banner's facing is its template's too.
        assert_eq!(
            b("minecolonies:colony_wall_banner").states[0].content,
            StateIn::Template("minecraft:white_wall_banner")
        );
    }

    #[test]
    fn the_two_variants_hold_java_s_own_names_in_java_s_own_order() {
        assert_eq!(
            b("minecolonies:blockminecoloniesgrave").states[1].values.names(),
            &["default", "decayed"]
        );
        assert_eq!(
            b("minecolonies:blockminecoloniesrack").states[1].values.names(),
            &["emptysingle", "fullsingle", "empty", "full", "dontrender"]
        );
        assert_eq!(
            b("minecolonies:barrel_block").states[1].values.names().len(),
            8,
            "BarrelType has eight values, `BarrelType.java:9-16`"
        );
        for block in BLOCKS {
            for state in block.states {
                assert!(
                    state.values.allows(state.default),
                    "{} {}: default {}",
                    block.name,
                    state.name,
                    state.default
                );
            }
        }
    }

    #[test]
    fn two_of_the_nine_hold_an_inventory_and_the_sign_holds_a_node() {
        assert!(b("minecolonies:blockminecoloniesrack").contents);
        assert!(b("minecolonies:blockminecoloniesgrave").contents);
        assert!(!b("minecolonies:blockminecoloniesnamedgrave").contents);
        assert!(!b("minecolonies:colony_banner").contents);
        // The sign's break removes a connection node; nothing else here does.
        assert!(removes_connection_node(b("minecolonies:colonysign")));
        assert_eq!(
            BLOCKS.iter().filter(|block| removes_connection_node(block)).count(),
            1,
            "the colony sign alone, `BlockColonySign.java:112-131`"
        );
    }
}
