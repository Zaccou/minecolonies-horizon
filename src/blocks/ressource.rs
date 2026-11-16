//! The twenty blocks the colony farms: the two farmlands, the fifteen crops the
//! farmer sows, the compost the composter fills, the scarecrow the field hangs
//! from, and the plantation's plot marker.
//!
//! Java reference: `core/blocks/MinecoloniesFarmland.java` (236 lines),
//! `core/blocks/MinecoloniesCropBlock.java` (258 lines, **one class for fifteen
//! blocks** — the fifteen are constructor arguments, not subclasses), the two
//! furniture blocks `BlockScarecrow.java` (304 lines) and
//! `BlockPlantationField.java` (257 lines), and `BlockCompostedDirt.java`.
//!
//! **Fifteen of the twenty share one class.** Every crop is a
//! `MinecoloniesCropBlock` built with its own name, its preferred farmland, the
//! blocks it can be harvested out of and a biome tag
//! (`ModBlocksInitializer.java:132-147`). That is why a crop row's `source` names
//! the initialiser line and not a class: the class cannot tell the fifteen apart,
//! and the arguments do.
//!
//! **Rice is the one crop sown on flooded farmland**
//! (`ModBlocksInitializer.java:141`), which is the only reason
//! [`crate::blocks::Block::sown_on`] is worth a field: it is the one fact about a
//! crop that changes what the farmer does with it.
//!
//! **The crop `age` cannot be declared in content.** Java's is
//! `IntegerProperty.create("age", 0, 6)` (`MinecoloniesCropBlock.java:71`), the
//! content format declares only booleans and enums
//! (`crates/horizon-mod-package/src/content.rs:356-363`), and the obvious template
//! — `minecraft:wheat` — has `age` 0..**7**, one value too many. That is a
//! [`StateIn::Gap`], and `MANQUANT.md` §4 says what it would take.
//!
//! **The farmland's `moisture` is the opposite case**: the template
//! (`minecraft:farmland`) already carries it, so only `waterlogged` is the port's
//! own — and the content file shipped today declares neither
//! (`data/minecolonies/horizon/block/farmland.json` sets `properties` only).

use super::{Block, Click, Family, Form, Props, State, StateIn, Values};

/// `BlockStateProperties.MOISTURE` (`MinecoloniesFarmland.java:54`), 0 to 7, from
/// the farmland template (`:67`).
const MOISTURE: State = State {
    name: "moisture",
    values: Values::Integer { min: 0, max: 7 },
    default: "0",
    source: "core/blocks/MinecoloniesFarmland.java:54",
    content: StateIn::Template("minecraft:farmland"),
};

/// `SimpleWaterloggedBlock`'s `WATERLOGGED` (`MinecoloniesFarmland.java:49`),
/// which the vanilla farmland does **not** have: the port declares it.
const FARMLAND_WATERLOGGED: State = State {
    name: "waterlogged",
    values: Values::Boolean,
    default: "false",
    source: "core/blocks/MinecoloniesFarmland.java:49",
    content: StateIn::Declared,
};

/// The flooded farmland's copy of it, whose default is `true`
/// (`ModBlocksInitializer.java:128` passes `true`, used at
/// `MinecoloniesFarmland.java:69`).
const FLOODED_WATERLOGGED: State = State {
    name: "waterlogged",
    values: Values::Boolean,
    default: "true",
    source: "core/blocks/MinecoloniesFarmland.java:49",
    content: StateIn::Declared,
};

/// The crop `age`, 0 to 6 (`MinecoloniesCropBlock.java:71`). An integer, and the
/// one gap of this family.
const AGE: State = State {
    name: "age",
    values: Values::Integer { min: 0, max: 6 },
    default: "0",
    source: "core/blocks/MinecoloniesCropBlock.java:71",
    content: StateIn::Gap(
        "an integer state property: content has `boolean` and `enum` only, and \
         `minecraft:wheat`'s `age` runs 0..7 rather than 0..6",
    ),
};

/// `DOUBLE_BLOCK_HALF`, lower first (`BlockScarecrow.java:54`, default at `:64`).
const HALF: State = State {
    name: "half",
    values: Values::Enum(&["lower", "upper"]),
    default: "lower",
    source: "core/blocks/BlockScarecrow.java:54",
    content: StateIn::Declared,
};

/// The scarecrow's own `lantern` flag (`BlockScarecrow.java:56`), set by a click
/// that holds a lantern.
const LANTERN: State = State {
    name: "lantern",
    values: Values::Boolean,
    default: "false",
    source: "core/blocks/BlockScarecrow.java:56",
    content: StateIn::Declared,
};

/// The plot marker's mirror flag (`BlockPlantationField.java:51`).
const PLOT_MIRROR: State = State {
    name: "mirror",
    values: Values::Boolean,
    default: "false",
    source: "core/blocks/BlockPlantationField.java:51",
    content: StateIn::Declared,
};

/// A horizontal facing, four values, default `north`
/// (`BlockScarecrow.java:64` and `BlockPlantationField.java:79` register the same
/// one).
const FACING: State = State {
    name: "facing",
    values: Values::Enum(super::FACING_VALUES),
    default: "north",
    source: "BlockPlantationField.java:79",
    content: StateIn::Declared,
};

/// One crop: the shape, the material and the `age` are the fifteen's; the name,
/// the line that builds it and the soil are the row's.
///
/// A **const** constructor so the fifteen can be written in the table itself:
/// their rows differ in three strings, and a table of fifteen `Block { .. }`
/// literals would bury that.
const fn crop(name: &'static str, source: &'static str, soil: &'static str) -> Block {
    Block::plain(
        name,
        source,
        Family::Ressource,
        Form::Crop,
        Click::None,
        Props::crop(),
        &[AGE],
    )
    .sown(soil)
}

/// The twenty blocks: the two farmlands, the compost, the two pieces of furniture
/// and the fifteen crops in the initialiser's order.
pub const BLOCKS: &[Block] = &[
    // `Properties.of().mapColor(DIRT).randomTicks().strength(0.6F).sound(GRAVEL)`
    // (`MinecoloniesFarmland.java:66`), waterlogged default false (`:69`, the
    // `false` argument at `ModBlocksInitializer.java:127`).
    Block::plain(
        "farmland",
        "core/blocks/MinecoloniesFarmland.java:47",
        Family::Ressource,
        Form::Full,
        Click::None,
        Props::dirt(0.6, 0.6),
        &[MOISTURE, FARMLAND_WATERLOGGED],
    ),
    // The same block under its own name (`:52`), waterlogged by default
    // (`ModBlocksInitializer.java:128`).
    Block::plain(
        "floodedfarmland",
        "core/blocks/MinecoloniesFarmland.java:47",
        Family::Ressource,
        Form::Full,
        Click::None,
        Props::dirt(0.6, 0.6),
        &[MOISTURE, FLOODED_WATERLOGGED],
    ),
    // DIRT map colour, `strength(5, 1)`, and the **gravel** sound: the second
    // `.sound(...)` of `BlockCompostedDirt.java:38` wins over the first.
    Block::plain(
        "composted_dirt",
        "core/blocks/BlockCompostedDirt.java:26",
        Family::Ressource,
        Form::Full,
        Click::None,
        Props::dirt(5.0, 1.0),
        &[],
    ),
    // The scarecrow: wood, 10/10
    // (`api/blocks/huts/AbstractBlockMinecoloniesDefault.java:20-24`), two blocks
    // tall, lit by its lantern (`BlockScarecrow.java:74-78`), and its id is the
    // inherited `REGISTRY_NAME = "blockhutfield"`, not its class name.
    Block::plain(
        "blockhutfield",
        "core/blocks/BlockScarecrow.java:52",
        Family::Ressource,
        Form::TwoTall,
        Click::Scarecrow,
        Props::wood(10.0, 10.0),
        &[HALF, FACING, LANTERN],
    ),
    // The plantation's plot marker: wood, 5/1 (`BlockPlantationField.java:78`),
    // `IBuilderUndestroyable` (`:46`).
    Block::plain(
        "blockhutplantationfield",
        "core/blocks/BlockPlantationField.java:46",
        Family::Ressource,
        Form::Field,
        Click::PlantationField,
        Props::wood(5.0, 1.0),
        &[FACING, PLOT_MIRROR],
    )
    .undestroyable_block(),
    // The fifteen crops. `rice` is the one sown on the flooded farmland
    // (`ModBlocksInitializer.java:141`); the fourteen others take
    // `ModBlocks.farmland`.
    crop(
        "bell_pepper",
        "apiimp/initializer/ModBlocksInitializer.java:132",
        "farmland",
    ),
    crop(
        "cabbage",
        "apiimp/initializer/ModBlocksInitializer.java:133",
        "farmland",
    ),
    crop(
        "chickpea",
        "apiimp/initializer/ModBlocksInitializer.java:134",
        "farmland",
    ),
    crop(
        "durum",
        "apiimp/initializer/ModBlocksInitializer.java:135",
        "farmland",
    ),
    crop(
        "eggplant",
        "apiimp/initializer/ModBlocksInitializer.java:136",
        "farmland",
    ),
    crop(
        "garlic",
        "apiimp/initializer/ModBlocksInitializer.java:137",
        "farmland",
    ),
    crop(
        "onion",
        "apiimp/initializer/ModBlocksInitializer.java:138",
        "farmland",
    ),
    crop(
        "soybean",
        "apiimp/initializer/ModBlocksInitializer.java:139",
        "farmland",
    ),
    crop(
        "tomato",
        "apiimp/initializer/ModBlocksInitializer.java:140",
        "farmland",
    ),
    crop(
        "rice",
        "apiimp/initializer/ModBlocksInitializer.java:141",
        "floodedfarmland",
    ),
    crop(
        "butternut_squash",
        "apiimp/initializer/ModBlocksInitializer.java:143",
        "farmland",
    ),
    crop(
        "corn",
        "apiimp/initializer/ModBlocksInitializer.java:144",
        "farmland",
    ),
    crop(
        "mint",
        "apiimp/initializer/ModBlocksInitializer.java:145",
        "farmland",
    ),
    crop(
        "nether_pepper",
        "apiimp/initializer/ModBlocksInitializer.java:146",
        "farmland",
    ),
    crop(
        "peas",
        "apiimp/initializer/ModBlocksInitializer.java:147",
        "farmland",
    ),
];

/// The fifteen crops, in the initialiser's order.
pub fn crops() -> impl Iterator<Item = &'static Block> {
    BLOCKS.iter().filter(|block| block.form == Form::Crop)
}

/// What a broken scarecrow leaves behind on top of its own drop: a lantern, and
/// only when the **lower** half has one (`BlockScarecrow.java:183-192`).
pub fn scarecrow_extra_drops(
    block: &Block,
    lantern: bool,
    lower_half: bool,
) -> &'static [&'static str] {
    if block.click == Click::Scarecrow && lantern && lower_half {
        return &["minecraft:lantern"];
    }
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{Answer, Facts, Held, Refusal, Window, block, click, kind_of};

    fn named(name: &str) -> &'static Block {
        block(&format!("minecolonies:{name}"))
            .unwrap_or_else(|| panic!("{name} is a ressource block"))
    }

    #[test]
    fn the_family_holds_twenty_blocks_fifteen_of_which_are_crops() {
        assert_eq!(BLOCKS.len(), 20);
        assert_eq!(crops().count(), 15);
        assert_eq!(
            BLOCKS.iter().filter(|block| block.form != Form::Crop).count(),
            5,
            "two farmlands, the compost, the scarecrow and the plot marker"
        );
        // Every crop is a crop: no hardness, no collision, no building.
        for crop in crops() {
            assert_eq!(crop.props.hardness, 0.0, "{}", crop.name);
            assert!(!crop.props.collision, "{}: instabreak and noCollission", crop.name);
            assert!(crop.building.is_none(), "{}", crop.name);
            assert_eq!(
                kind_of(&crop.path()),
                Err(Refusal::NotAColonyBlock { block: crop.name })
            );
        }
    }

    #[test]
    fn rice_is_the_only_crop_sown_on_flooded_farmland() {
        assert_eq!(named("rice").sown_on, Some("floodedfarmland"));
        let flooded: Vec<&str> = crops()
            .filter(|crop| crop.sown_on == Some("floodedfarmland"))
            .map(|crop| crop.name)
            .collect();
        assert_eq!(flooded, vec!["rice"], "`ModBlocksInitializer.java:141`");
        // The two soils are the two farmlands of this family, by name.
        for soil in ["farmland", "floodedfarmland"] {
            assert_eq!(named(soil).name, soil);
        }
        // A block that is not a crop wants no soil at all.
        assert_eq!(named("composted_dirt").sown_on, None);
    }

    #[test]
    fn a_crop_s_age_is_a_gap_and_the_farmland_s_moisture_is_not() {
        let age = named("cabbage").states[0];
        assert_eq!(age.name, "age");
        assert_eq!(age.values, Values::Integer { min: 0, max: 6 });
        match age.content {
            StateIn::Gap(need) => assert!(need.contains("integer"), "{need}"),
            other => panic!("the crop age must be a gap, not {other:?}"),
        }
        // The farmland's moisture comes from the template, so it costs nothing.
        let moisture = named("farmland")
            .states
            .iter()
            .find(|state| state.name == "moisture")
            .expect("farmland declares moisture");
        assert_eq!(moisture.content, StateIn::Template("minecraft:farmland"));
        assert_eq!(moisture.values, Values::Integer { min: 0, max: 7 });
        // Its waterlogged state, which the template does not have, is declared.
        let waterlogged = named("farmland")
            .states
            .iter()
            .find(|state| state.name == "waterlogged")
            .expect("farmland declares waterlogged");
        assert_eq!(waterlogged.content, StateIn::Declared);
    }

    #[test]
    fn the_flooded_farmland_differs_from_the_dry_one_by_one_default() {
        let dry = named("farmland");
        let flooded = named("floodedfarmland");
        assert_eq!(dry.props, flooded.props, "same material, same hardness");
        assert_eq!(dry.states[0], flooded.states[0], "moisture");
        assert_eq!(dry.states[1].name, "waterlogged");
        assert_eq!(dry.states[1].default, "false");
        assert_eq!(
            flooded.states[1].default, "true",
            "`ModBlocksInitializer.java:128` passes `true`"
        );
    }

    #[test]
    fn the_scarecrow_is_two_blocks_tall_and_its_lantern_lights_it() {
        let scarecrow = named("blockhutfield");
        assert_eq!(scarecrow.form, Form::TwoTall);
        assert_eq!(scarecrow.states.len(), 3);
        assert_eq!(scarecrow.states[0].values.names(), &["lower", "upper"]);
        // A lantern in hand fills it, and the click consumes the item.
        let with_lantern = Facts {
            held: Held::Lantern,
            ..Facts::member()
        };
        assert_eq!(
            click(scarecrow, &with_lantern),
            Answer::Consume {
                sets: Some(("lantern", "true"))
            }
        );
        // A second lantern changes nothing: the screen opens instead.
        let already = Facts {
            lantern: true,
            ..with_lantern
        };
        assert_eq!(
            click(scarecrow, &already),
            Answer::Open {
                window: Window::Field,
                shift: false
            }
        );
        // And the lantern comes back when the lower half is broken.
        assert_eq!(
            scarecrow_extra_drops(scarecrow, true, true),
            &["minecraft:lantern"]
        );
        assert!(scarecrow_extra_drops(scarecrow, true, false).is_empty());
        assert!(scarecrow_extra_drops(scarecrow, false, true).is_empty());
        assert!(scarecrow_extra_drops(named("composted_dirt"), true, true).is_empty());
    }

    #[test]
    fn the_plantations_plot_marker_is_one_more_block_the_builder_must_not_touch() {
        let plot = named("blockhutplantationfield");
        assert!(!plot.builder_may_break(), "IBuilderUndestroyable, `:46`");
        assert_eq!(plot.states.len(), 2);
        assert_eq!(plot.states[1].name, "mirror");
        assert_eq!(
            click(plot, &Facts::member()),
            Answer::Open {
                window: Window::PlantationField,
                shift: false
            }
        );
        // The off hand is refused here, unlike on the scarecrow
        // (`BlockPlantationField.java:118-121`).
        let off_hand = Facts {
            off_hand: true,
            ..Facts::member()
        };
        assert_eq!(click(plot, &off_hand), Answer::Refused(Refusal::OffHand));
        // The three farm blocks answer no click of ours.
        for name in ["composted_dirt", "farmland", "floodedfarmland"] {
            assert_eq!(click(named(name), &Facts::member()), Answer::Pass, "{name}");
        }
        for crop in crops() {
            assert_eq!(click(crop, &Facts::member()), Answer::Pass, "{}", crop.name);
        }
    }

    #[test]
    fn the_compost_sound_is_the_second_one_java_asks_for() {
        let compost = named("composted_dirt");
        assert_eq!(
            compost.props.sounds, "minecraft:gravel",
            "`BlockCompostedDirt.java:38` sets ROOTED_DIRT then GRAVEL, and the last wins"
        );
        assert_eq!(compost.props.map_color, crate::blocks::MAP_COLOR_DIRT);
        assert_eq!(compost.states.len(), 0, "no state of its own");
        // The farmland borrows gravel too, and the wood family does not.
        assert_eq!(named("farmland").props.sounds, "minecraft:gravel");
        assert_eq!(
            named("blockhutbuilder").props.sounds,
            "minecraft:oak_planks"
        );
    }

    #[test]
    fn the_fifteen_crops_are_the_fifteen_java_registers() {
        let mut names: Vec<&str> = crops().map(|crop| crop.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "bell_pepper",
                "butternut_squash",
                "cabbage",
                "chickpea",
                "corn",
                "durum",
                "eggplant",
                "garlic",
                "mint",
                "nether_pepper",
                "onion",
                "peas",
                "rice",
                "soybean",
                "tomato",
            ],
            "`ModBlocksInitializer.java:132-147`, in the initialiser's order"
        );
        // Every crop has its own line, so a row cannot borrow another's.
        let mut sources: Vec<&str> = crops().map(|crop| crop.source).collect();
        let count = sources.len();
        sources.sort_unstable();
        sources.dedup();
        assert_eq!(sources.len(), count);
    }
}
