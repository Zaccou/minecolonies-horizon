//! The three blocks of a build site: the tape that fences it, the anchor the
//! builder hangs a decoration on, and the marker a schematic is read from.
//!
//! None of them is a colony block, and that is the point of the family: they are
//! put down by the builder's own handler, not by a player with a hut item. The
//! tape is placed and removed by `ConstructionTapeHelper`
//! (`core/entity/ai/workers/util/ConstructionTapeHelper.java:106-142`), the
//! decoration controller is the `IAnchorBlock` a decoration's blocks are hung
//! from (`BlockDecorationController.java:51-52`), and the waypoint is the marker
//! `core/blocks/schematic/BlockWaypoint.java` puts in a blueprint.
//!
//! **Two of the three are `IBuilderUndestroyable`** — the controller
//! (`BlockDecorationController.java:51`) — while the tape is not: the builder
//! removes the tape when the hut is finished, which is what it is for. The
//! waypoint is not on that list either, and its hardness is 0, so anything that
//! touches it removes it (`BlockWaypoint.java:17`).
//!
//! **The decoration controller's `FACING` is not the huts'.** It comes from
//! `DirectionalBlock` (`AbstractBlockMinecoloniesDirectional.java:10`), so it has
//! **six** values and takes `getClickedFace().getOpposite()`
//! (`BlockDecorationController.java:221`), where a hut's `FACING` has four and
//! follows the placer's yaw (`AbstractColonyBlock.java:237`). A content file that
//! reused the hut's four-value `facing` here would be wrong for a controller on a
//! ceiling.

use super::{Block, Click, Family, Form, Props, State, StateIn, Values};

/// The tape's four connection sides, `PipeBlock.NORTH` and friends
/// (`AbstractBlockMinecoloniesConstructionTape.java:24-27`).
const NORTH: State = State {
    name: "north",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:24",
    content: StateIn::Declared,
};
const EAST: State = State {
    name: "east",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:25",
    content: StateIn::Declared,
};
const SOUTH: State = State {
    name: "south",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:26",
    content: StateIn::Declared,
};
const WEST: State = State {
    name: "west",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:27",
    content: StateIn::Declared,
};
/// `SimpleWaterloggedBlock`'s, `BlockStateProperties.WATERLOGGED`
/// (`AbstractBlockMinecoloniesConstructionTape.java:28`).
const WATERLOGGED: State = State {
    name: "waterlogged",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:28",
    content: StateIn::Declared,
};
/// The tape's own facing, the default for a tape with no connections
/// (`AbstractBlockMinecoloniesConstructionTape.java:35`).
const TAPE_FACING: State = State {
    name: "facing",
    values: Values::Enum(super::FACING_VALUES),
    default: "north",
    source: "AbstractBlockMinecoloniesConstructionTape.java:35",
    content: StateIn::Declared,
};
/// `BooleanProperty.create("corner")`, which only the builder sets
/// (`AbstractBlockMinecoloniesConstructionTape.java:40`, set at
/// `ConstructionTapeHelper.java:106`).
const CORNER: State = State {
    name: "corner",
    values: Values::Boolean,
    default: "false",
    source: "AbstractBlockMinecoloniesConstructionTape.java:40",
    content: StateIn::Declared,
};

/// The six values `DirectionalBlock.FACING` has, in vanilla's `Direction` order.
const DIRECTIONS: &[&str] = &["down", "up", "north", "south", "west", "east"];

/// `DirectionalBlock.FACING` (`AbstractBlockMinecoloniesDirectional.java:10`,
/// `DirectionalBlock`), six values, default `NORTH`
/// (`BlockDecorationController.java:94`).
const SIX_WAY: State = State {
    name: "facing",
    values: Values::Enum(DIRECTIONS),
    default: "north",
    source: "BlockDecorationController.java:94",
    content: StateIn::Declared,
};

/// The build tool's mirror flag (`BlockDecorationController.java:94`).
const MIRROR: State = State {
    name: "mirror",
    values: Values::Boolean,
    default: "false",
    source: "BlockDecorationController.java:94",
    content: StateIn::Declared,
};

/// The three blocks, in the order a build meets them.
pub const BLOCKS: &[Block] = &[
    // `Properties.of().mapColor(PLANT).sound(WOOD).replaceable()
    // .pushReaction(DESTROY).strength(0.0f).noCollission().noLootTable()`
    // (`BlockConstructionTape.java:47-53`): nothing drops, because the builder
    // owns it.
    Block::plain(
        "blockconstructiontape",
        "core/blocks/decorative/BlockConstructionTape.java:35",
        Family::Chantier,
        Form::Tape,
        Click::None,
        Props::plant(0.0, 0.0).thin(),
        &[NORTH, EAST, SOUTH, WEST, WATERLOGGED, TAPE_FACING, CORNER],
    ),
    // `IBuilderUndestroyable`, `IAnchorBlock`, `ILeveledBlueprintAnchorBlock`,
    // `SimpleWaterloggedBlock` (`:51-52`); wood, 5/1, no collision (`:93`).
    Block::plain(
        "decorationcontroller",
        "core/blocks/BlockDecorationController.java:51",
        Family::Chantier,
        Form::Plate,
        Click::Decoration,
        Props::wood(5.0, 1.0).thin(),
        &[SIX_WAY, MIRROR, WATERLOGGED],
    )
    .undestroyable_block(),
    // Wood, 0/1, no collision (`BlockWaypoint.java:34`), and **no state at all**:
    // the block's only job is to be found by a blueprint reader. The content file
    // that already exists says the same in three keys
    // (`data/minecolonies/horizon/block/blockwaypoint.json`).
    Block::plain(
        "blockwaypoint",
        "core/blocks/schematic/BlockWaypoint.java:12",
        Family::Chantier,
        Form::Empty,
        Click::None,
        Props::wood(0.0, 1.0).thin(),
        &[],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{Answer, Facts, Refusal, Window, all, block, click};

    fn b(name: &str) -> &'static Block {
        block(name).unwrap_or_else(|| panic!("{name} is a chantier block"))
    }

    #[test]
    fn the_three_blocks_are_the_builder_s_own() {
        assert_eq!(BLOCKS.len(), 3);
        for block in BLOCKS {
            assert_eq!(block.family, Family::Chantier);
            assert!(block.building.is_none(), "{} is no colony block", block.name);
            // Nothing in this family holds an inventory.
            assert!(!block.contents, "{}", block.name);
        }
        assert_eq!(all().filter(|b| b.family == Family::Chantier).count(), 3);
    }

    #[test]
    fn only_the_controller_refuses_to_be_removed() {
        assert!(
            !b("minecolonies:decorationcontroller").builder_may_break(),
            "IBuilderUndestroyable, `BlockDecorationController.java:51`"
        );
        // The tape is the builder's to take down: it is replaceable and drops
        // nothing (`BlockConstructionTape.java:47-53`), and it is not on the
        // undestroyable list.
        assert!(b("minecolonies:blockconstructiontape").builder_may_break());
        // So is the waypoint, at hardness 0.
        assert!(b("minecolonies:blockwaypoint").builder_may_break());
        assert_eq!(b("minecolonies:blockwaypoint").props.hardness, 0.0);
    }

    #[test]
    fn the_tape_has_seven_states_and_the_waypoint_none() {
        assert_eq!(b("minecolonies:blockconstructiontape").states.len(), 7);
        assert!(b("minecolonies:blockwaypoint").states.is_empty());
        // The four connections default to false, so a lone tape is a post.
        for state in b("minecolonies:blockconstructiontape").states {
            if matches!(state.name, "north" | "east" | "south" | "west" | "corner" | "waterlogged")
            {
                assert_eq!(state.default, "false", "{}", state.name);
            }
        }
    }

    #[test]
    fn the_controller_faces_six_ways_and_the_hut_four() {
        let controller = b("minecolonies:decorationcontroller");
        let facing = controller
            .states
            .iter()
            .find(|state| state.name == "facing")
            .expect("the controller has a facing");
        assert_eq!(facing.values, Values::Enum(DIRECTIONS));
        assert_eq!(facing.values.names().len(), 6);
        // The hut's own is four, and the two must not be confused.
        let builder = block("minecolonies:blockhutbuilder").unwrap();
        assert_eq!(
            builder.states[0].values.names(),
            &["north", "south", "east", "west"]
        );
        assert_ne!(facing.values.names(), builder.states[0].values.names());
    }

    #[test]
    fn the_controller_opens_its_settings_and_the_other_two_answer_nothing() {
        let member = Facts::member();
        assert_eq!(
            click(b("minecolonies:decorationcontroller"), &member),
            Answer::Open {
                window: Window::Decoration,
                shift: false
            }
        );
        // With no block entity there is nothing to configure.
        let empty = Facts {
            block_entity: false,
            ..member
        };
        assert_eq!(
            click(b("minecolonies:decorationcontroller"), &empty),
            Answer::Refused(Refusal::NoBlockEntity {
                block: "decorationcontroller"
            })
        );
        // The tape and the waypoint leave the click to the engine.
        assert_eq!(click(b("minecolonies:blockconstructiontape"), &member), Answer::Pass);
        assert_eq!(click(b("minecolonies:blockwaypoint"), &member), Answer::Pass);
    }

    #[test]
    fn none_of_them_is_a_colony_block() {
        for name in [
            "minecolonies:blockconstructiontape",
            "minecolonies:decorationcontroller",
            "minecolonies:blockwaypoint",
        ] {
            assert_eq!(
                crate::blocks::kind_of(name),
                Err(Refusal::NotAColonyBlock {
                    block: b(name).name
                }),
                "{name}"
            );
        }
    }
}
