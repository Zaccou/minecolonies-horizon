//! The 54 colony blocks: the 52 huts of `core/blocks/huts/` and the two colony
//! blocks that are not huts — the post box and the stash.
//!
//! Java reference: `core/blocks/huts/*.java` on `api/blocks/AbstractColonyBlock.java`
//! (305 lines, the class 54 of them share) and `api/blocks/AbstractBlockHut.java`
//! (375 lines, the class 52 of those share).
//!
//! **What `AbstractColonyBlock` gives all 54**, and where each rule is:
//!
//! | Moment | Java | Here |
//! | --- | --- | --- |
//! | the material | `:99` — `MapColor.WOOD`, `SoundType.WOOD`, `strength(10, ∞)`, `noOcclusion` | [`crate::blocks::HUT_HARDNESS`] |
//! | the state | `:65`, `:100` — `FACING`, default `north` | [`crate::blocks::FACING`] |
//! | the shape | `:80` — a `0.1..0.9` cube, 1.9 high for the graveyard | [`crate::blocks::Form`] |
//! | the placement | `:235-239` — `Direction.fromYRot`, `NORTH` with no player | [`crate::blocks::facing_from_yaw`] |
//! | the link | `:249-276` — `addNewBuilding` when a colony covers the position | [`crate::blocks::placement`] |
//! | the click | `:174-231` — the building's screen, `ACCESS_HUTS` required | [`crate::blocks::click`] |
//! | the break | `:105-115` — `(pvp ? 1/40 : 1/10) / 30`, plus the child warning | [`crate::blocks::break_progress`] |
//! | the removal | `:148-156` — the inventory drops | [`crate::blocks::Block::contents`] |
//!
//! **Ten of the 54 change one of those, and nothing else changes anything.** The
//! town hall, the gate house, the stash and the post box have their own click or
//! break (the table below); the warehouse and the enchanter create a block entity
//! type of their own (`BlockHutWareHouse.java:35-41`,
//! `BlockHutEnchanter.java:30-35`); the barracks tower is never drawn
//! (`BlockHutBarracksTower.java:25-27`); the graveyard is 1.9 high
//! (`BlockHutGraveyard.java:22`); the tavern refuses a second copy
//! (`BlockHutTavern.java:50-65`); the town hall refuses a second copy too
//! (`BlockHutTownHall.java:219-234`). Everything else is the parent's.
//!
//! **What a hut does *not* do here.** No tick: `AbstractColonyBlock` implements
//! `ITickableBlockMinecolonies` (`:54`), whose tick is the block entity's, and the
//! port has no hook for it in this module (`MANQUANT.md` §2). No blueprint: the
//! paste is `AbstractBlockHut.setup` (`:215-296`), which needs the schematic
//! machinery `src/structures/` and `crate::blueprint` own.

use super::{Block, Click, Form, Kind};

/// The 54 colony blocks. The first **50** are the `blockhut*` names in
/// alphabetical order, which is the order the Java files sort in; the last four
/// close the table because their names carry no `blockhut` prefix — the two
/// quarries (`simplequarry`, `mediumquarry`) and the two colony blocks that are
/// not huts (`blockpostbox`, `blockstash`).
///
/// Each row's `source` is the class's declaration line; the block's name is what
/// that class's `getHutName()` returns, which is the same string unless the row
/// says otherwise.
pub const BLOCKS: &[Block] = &[
    Block::hut(
        "blockhutalchemist",
        "core/blocks/huts/BlockHutAlchemist.java:11",
        Kind::Alchemist,
    ),
    Block::hut(
        "blockhutarchery",
        "core/blocks/huts/BlockHutArchery.java:11",
        Kind::Archery,
    ),
    // `ModBuildings.bakery` is registered under `BAKERY_ID = "baker"`
    // (`ModBuildings.java:10`), so the block and the building agree.
    Block::hut(
        "blockhutbaker",
        "core/blocks/huts/BlockHutBaker.java:11",
        Kind::Baker,
    ),
    Block::hut(
        "blockhutbarracks",
        "core/blocks/huts/BlockHutBarracks.java:11",
        Kind::Barracks,
    ),
    Block::hut(
        "blockhutbarrackstower",
        "core/blocks/huts/BlockHutBarracksTower.java:13",
        Kind::BarracksTower,
    ),
    Block::hut(
        "blockhutbeekeeper",
        "core/blocks/huts/BlockHutBeekeeper.java:10",
        Kind::Beekeeper,
    ),
    Block::hut(
        "blockhutblacksmith",
        "core/blocks/huts/BlockHutBlacksmith.java:11",
        Kind::Blacksmith,
    ),
    Block::hut(
        "blockhutbuilder",
        "core/blocks/huts/BlockHutBuilder.java:11",
        Kind::Builder,
    ),
    Block::hut(
        "blockhutchickenherder",
        "core/blocks/huts/BlockHutChickenHerder.java:11",
        Kind::ChickenHerder,
    ),
    // **The one block whose name is not its building's**: `blockhutcitizen`
    // stands for the residence (`ModBuildings.HOME_ID = "residence"`,
    // `ModBuildings.java:25`, returned by `BlockHutCitizen.getBuildingEntry`,
    // `:29`).
    Block::hut(
        "blockhutcitizen",
        "core/blocks/huts/BlockHutCitizen.java:11",
        Kind::Residence,
    ),
    Block::hut(
        "blockhutcombatacademy",
        "core/blocks/huts/BlockHutCombatAcademy.java:11",
        Kind::CombatAcademy,
    ),
    Block::hut(
        "blockhutcomposter",
        "core/blocks/huts/BlockHutComposter.java:9",
        Kind::Composter,
    ),
    Block::hut(
        "blockhutconcretemixer",
        "core/blocks/huts/BlockHutConcreteMixer.java:12",
        Kind::ConcreteMixer,
    ),
    Block::hut(
        "blockhutcook",
        "core/blocks/huts/BlockHutCook.java:11",
        Kind::Cook,
    ),
    Block::hut(
        "blockhutcowboy",
        "core/blocks/huts/BlockHutCowboy.java:11",
        Kind::Cowboy,
    ),
    Block::hut(
        "blockhutcrusher",
        "core/blocks/huts/BlockHutCrusher.java:11",
        Kind::Crusher,
    ),
    Block::hut(
        "blockhutdeliveryman",
        "core/blocks/huts/BlockHutDeliveryman.java:12",
        Kind::Deliveryman,
    ),
    Block::hut(
        "blockhutdyer",
        "core/blocks/huts/BlockHutDyer.java:11",
        Kind::Dyer,
    ),
    // The enchanter builds its own block entity type instead of the shared
    // `BUILDING` one (`BlockHutEnchanter.java:30-35`): a content/registry
    // difference, not a rule of the block.
    Block::hut(
        "blockhutenchanter",
        "core/blocks/huts/BlockHutEnchanter.java:13",
        Kind::Enchanter,
    ),
    Block::hut(
        "blockhutfarmer",
        "core/blocks/huts/BlockHutFarmer.java:12",
        Kind::Farmer,
    ),
    Block::hut(
        "blockhutfisherman",
        "core/blocks/huts/BlockHutFisherman.java:11",
        Kind::Fisherman,
    ),
    Block::hut(
        "blockhutfletcher",
        "core/blocks/huts/BlockHutFletcher.java:11",
        Kind::Fletcher,
    ),
    Block::hut(
        "blockhutflorist",
        "core/blocks/huts/BlockHutFlorist.java:8",
        Kind::Florist,
    ),
    // The gate house answers a player without `ACCESS_HUTS` with the connection
    // module instead of a refusal (`BlockHutGateHouse.java:56-77`).
    Block::custom(
        "blockhutgatehouse",
        "core/blocks/huts/BlockHutGateHouse.java:24",
        Kind::GateHouse,
        Form::SmallBox,
        Click::GateHouse,
    ),
    Block::hut(
        "blockhutglassblower",
        "core/blocks/huts/BlockHutGlassblower.java:11",
        Kind::Glassblower,
    ),
    // 1.9 high instead of 0.9 (`BlockHutGraveyard.java:22`).
    Block::custom(
        "blockhutgraveyard",
        "core/blocks/huts/BlockHutGraveyard.java:18",
        Kind::Graveyard,
        Form::TallBox,
        Click::Hut,
    ),
    Block::hut(
        "blockhutguardtower",
        "core/blocks/huts/BlockHutGuardTower.java:11",
        Kind::GuardTower,
    ),
    Block::hut(
        "blockhuthospital",
        "core/blocks/huts/BlockHutHospital.java:11",
        Kind::Hospital,
    ),
    Block::hut(
        "blockhutkitchen",
        "core/blocks/huts/BlockHutKitchen.java:11",
        Kind::Kitchen,
    ),
    Block::hut(
        "blockhutlibrary",
        "core/blocks/huts/BlockHutLibrary.java:11",
        Kind::Library,
    ),
    Block::hut(
        "blockhutlumberjack",
        "core/blocks/huts/BlockHutLumberjack.java:11",
        Kind::Lumberjack,
    ),
    Block::hut(
        "blockhutmechanic",
        "core/blocks/huts/BlockHutMechanic.java:11",
        Kind::Mechanic,
    ),
    Block::hut(
        "blockhutminer",
        "core/blocks/huts/BlockHutMiner.java:12",
        Kind::Miner,
    ),
    Block::hut(
        "blockhutmysticalsite",
        "core/blocks/huts/BlockHutMysticalSite.java:10",
        Kind::MysticalSite,
    ),
    Block::hut(
        "blockhutnetherworker",
        "core/blocks/huts/BlockHutNetherWorker.java:7",
        Kind::NetherWorker,
    ),
    Block::hut(
        "blockhutplantation",
        "core/blocks/huts/BlockHutPlantation.java:12",
        Kind::Plantation,
    ),
    Block::hut(
        "blockhutrabbithutch",
        "core/blocks/huts/BlockHutRabbitHutch.java:11",
        Kind::RabbitHutch,
    ),
    Block::hut(
        "blockhutsawmill",
        "core/blocks/huts/BlockHutSawmill.java:11",
        Kind::Sawmill,
    ),
    Block::hut(
        "blockhutschool",
        "core/blocks/huts/BlockHutSchool.java:11",
        Kind::School,
    ),
    Block::hut(
        "blockhutshepherd",
        "core/blocks/huts/BlockHutShepherd.java:11",
        Kind::Shepherd,
    ),
    Block::hut(
        "blockhutsifter",
        "core/blocks/huts/BlockHutSifter.java:11",
        Kind::Sifter,
    ),
    Block::hut(
        "blockhutsmeltery",
        "core/blocks/huts/BlockHutSmeltery.java:11",
        Kind::Smeltery,
    ),
    Block::hut(
        "blockhutstable",
        "core/blocks/huts/BlockHutStable.java:11",
        Kind::Stable,
    ),
    Block::hut(
        "blockhutstonemason",
        "core/blocks/huts/BlockHutStonemason.java:11",
        Kind::StoneMason,
    ),
    // `STONE_SMELTERY_ID = "stonesmeltery"` (`ModBuildings.java:35`), so the
    // block's name and the building's agree even though the Java field is
    // `stoneSmelter` (`BlockHutStoneSmeltery.java:23`).
    Block::hut(
        "blockhutstonesmeltery",
        "core/blocks/huts/BlockHutStoneSmeltery.java:11",
        Kind::StoneSmeltery,
    ),
    Block::hut(
        "blockhutswineherder",
        "core/blocks/huts/BlockHutSwineHerder.java:11",
        Kind::SwineHerder,
    ),
    // One tavern per colony (`BlockHutTavern.java:50-65`, `WARNING_DUPLICATE_TAVERN`).
    Block::hut(
        "blockhuttavern",
        "core/blocks/huts/BlockHutTavern.java:22",
        Kind::Tavern,
    ),
    // The one hut the build tool places outside a colony, and the one every
    // colony has exactly one of (`AbstractBlockHut.java:311-314`,
    // `BlockHutTownHall.java:219-234`). Its click is the colony overview, with no
    // `ACCESS_HUTS` (`:181-215`), and its break is twelve times the hut's in pvp
    // (`:129-130`).
    Block::custom(
        "blockhuttownhall",
        "core/blocks/huts/BlockHutTownHall.java:47",
        Kind::TownHall,
        Form::SmallBox,
        Click::TownHall,
    ),
    Block::hut(
        "blockhutuniversity",
        "core/blocks/huts/BlockHutUniversity.java:11",
        Kind::University,
    ),
    // Its own block entity type (`BlockHutWareHouse.java:35-41`).
    Block::hut(
        "blockhutwarehouse",
        "core/blocks/huts/BlockHutWareHouse.java:18",
        Kind::Warehouse,
    ),
    // **No `blockhut` prefix**: `getHutName()` returns
    // `ModBuildings.SIMPLE_QUARRY_ID` (`SimpleQuarry.java:23`).
    Block::hut(
        "simplequarry",
        "core/blocks/huts/SimpleQuarry.java:11",
        Kind::SimpleQuarry,
    ),
    // The same for the medium quarry (`MediumQuarry.java:23`).
    Block::hut(
        "mediumquarry",
        "core/blocks/huts/MediumQuarry.java:11",
        Kind::MediumQuarry,
    ),
    // **A colony block without being a hut**: no `AbstractBlockHut`, so no
    // blueprint, no level and no `isVisible`; a half-block shape and a `1/30`
    // break (`BlockPostBox.java:20-62`). Its click is the inherited one.
    Block::custom(
        "blockpostbox",
        "core/blocks/huts/BlockPostBox.java:20",
        Kind::Postbox,
        Form::HalfBox,
        Click::Hut,
    ),
    // The same, with the stash's own click: the building inventory and nothing
    // else (`BlockStash.java:87-105`).
    Block::custom(
        "blockstash",
        "core/blocks/huts/BlockStash.java:33",
        Kind::Stash,
        Form::HalfBox,
        Click::Stash,
    ),
];

/// The level a hut's own schematic name gives it, or `None` when the name holds
/// no digit.
///
/// Java reads `TAG_SCHEMATIC_NAME` out of the blueprint data, strips every
/// non-digit and parses what is left (`AbstractBlockHut.java:190-206`), so
/// `"blockhutbuilder3"` is level 3 and `"blockhutbuilder"` logs
/// "Couldn't get level from hut" and answers 0 (`:201-205`). **0 is not a level**
/// — it is a hut placed and not yet built (`crate::building`). This rule answers
/// `None` where Java answers 0 and logs, and the caller decides what to say.
///
/// Java's `replaceAll("[^0-9]", "")` on a name with several numbers glues them
/// together (`"level1copy2"` → 12); the port does the same, because a level read
/// by a different rule than the one written would disagree with Java on a save
/// that already exists.
pub fn schematic_level(schematic: &str) -> Option<i32> {
    let digits: String = schematic
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    digits.parse().ok()
}

/// Whether a hut is drawn for the player who plunders it with the build tool.
///
/// `AbstractBlockHut.isVisible` answers `false` when the blueprint data tags the
/// block `invisible` (`api/blocks/AbstractBlockHut.java:124-128`), and the barracks
/// tower answers `false` always, whatever the data says
/// (`BlockHutBarracksTower.java:25-27`): it is drawn by the barracks it belongs to.
pub fn visible(block: &Block, tagged_invisible: bool) -> bool {
    if block.building == Some(Kind::BarracksTower) {
        return false;
    }
    !tagged_invisible
}

/// The blueprint a hut of this kind pastes, which is its **building**'s registry
/// path and not the block's (`AbstractBlockHut.getBlueprintName`, `:344-347`).
///
/// `blockhutcitizen` pastes `residence`, and the quarries paste `simplequarry`
/// and `mediumquarry` — the one place the two names being different matters.
pub fn blueprint_name(block: &Block) -> Option<String> {
    block.building.map(|kind| kind.name().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{Answer, Facts, Question, Refusal, Window, click, kind_of};

    fn hut(name: &str) -> &'static Block {
        super::BLOCKS
            .iter()
            .find(|block| block.name == name)
            .unwrap_or_else(|| panic!("{name} is a hut"))
    }

    #[test]
    fn the_family_holds_fifty_two_huts_and_two_colony_blocks() {
        assert_eq!(BLOCKS.len(), 54);
        let non_huts: Vec<&str> = BLOCKS
            .iter()
            .filter(|block| block.name == "blockpostbox" || block.name == "blockstash")
            .map(|block| block.name)
            .collect();
        assert_eq!(non_huts.len(), 2, "the two `AbstractColonyBlock`s that are not huts");
        // Every one of them is a colony block: undestroyable, with contents, and
        // knowing its building.
        for block in BLOCKS {
            assert!(block.building.is_some(), "{}", block.name);
            assert!(block.undestroyable, "{}: IBuilderUndestroyable", block.name);
            assert!(block.contents, "{}: its inventory drops", block.name);
            assert_eq!(block.states.len(), 1, "{}: FACING and nothing else", block.name);
            assert_eq!(block.states[0].name, "facing", "{}", block.name);
            assert_eq!(block.props.hardness, crate::blocks::HUT_HARDNESS);
            assert!(block.props.resistance.is_infinite(), "{}", block.name);
        }
    }

    #[test]
    fn the_fifty_huts_come_in_name_order_and_the_four_others_close_the_table() {
        let names: Vec<&str> = BLOCKS[..50].iter().map(|block| block.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "the 50 `blockhut*` names are alphabetical");
        for block in &BLOCKS[..50] {
            assert!(block.name.starts_with("blockhut"), "{}", block.name);
        }
        for block in &BLOCKS[50..] {
            assert!(!block.name.starts_with("blockhut"), "{}", block.name);
        }
        assert_eq!(
            &["simplequarry", "mediumquarry", "blockpostbox", "blockstash"],
            &BLOCKS[50..].iter().map(|block| block.name).collect::<Vec<_>>()[..]
        );
    }

    #[test]
    fn the_ten_that_differ_from_the_parent_differ_where_java_says() {
        // Four have their own click.
        assert_eq!(hut("blockhuttownhall").click, Click::TownHall);
        assert_eq!(hut("blockhutgatehouse").click, Click::GateHouse);
        assert_eq!(hut("blockstash").click, Click::Stash);
        assert_eq!(
            hut("blockpostbox").click,
            Click::Hut,
            "the post box overrides no `use`: it takes the parent's"
        );
        // Two have their own shape.
        assert_eq!(hut("blockhutgraveyard").form, Form::TallBox);
        assert_eq!(hut("blockstash").form, Form::HalfBox);
        assert_eq!(hut("blockpostbox").form, Form::HalfBox);
        // Everything else is the hut shape and the hut click.
        let plain = BLOCKS
            .iter()
            .filter(|block| block.click == Click::Hut && block.form == Form::SmallBox)
            .count();
        assert_eq!(
            plain, 49,
            "51 keep the parent's click; the graveyard and the post box change only the shape"
        );
    }

    #[test]
    fn the_post_box_answers_the_hut_click_and_the_stash_does_not() {
        let member = Facts::member();
        assert_eq!(
            click(hut("blockpostbox"), &member),
            Answer::Open {
                window: Window::Hut,
                shift: false
            }
        );
        assert_eq!(
            click(hut("blockstash"), &member),
            Answer::Open {
                window: Window::BuildingInventory,
                shift: false
            }
        );
        // A stranger gets nothing at all from a stash: no message, no window.
        assert_eq!(click(hut("blockstash"), &member.stranger()), Answer::Pass);
        // A stranger is refused the post box, which is a hut like any other.
        assert_eq!(
            click(hut("blockpostbox"), &member.stranger()),
            Answer::Refused(Refusal::LacksAccess {
                rank: crate::colony::permissions::Rank::Neutral
            })
        );
    }

    #[test]
    fn the_gate_house_opens_for_a_stranger_and_the_other_huts_do_not() {
        let stranger = Facts::member().stranger();
        assert_eq!(
            click(hut("blockhutgatehouse"), &stranger),
            Answer::Open {
                window: Window::ConnectionModule,
                shift: false
            },
            "a gate house is the one hut a non-member may use"
        );
        assert_eq!(
            click(hut("blockhutbuilder"), &stranger),
            Answer::Refused(Refusal::LacksAccess {
                rank: crate::colony::permissions::Rank::Neutral
            })
        );
        // A member gets the ordinary hut screen from the gate house.
        assert_eq!(
            click(hut("blockhutgatehouse"), &Facts::member()),
            Answer::Open {
                window: Window::Hut,
                shift: false
            }
        );
    }

    #[test]
    fn the_town_hall_asks_the_server_instead_of_refusing() {
        let member = Facts::member();
        assert_eq!(
            click(hut("blockhuttownhall"), &member),
            Answer::Open {
                window: Window::ColonyOverview,
                shift: false
            }
        );
        // A stranger is not refused: Java asks the server for the colony's
        // overview, which is how a player joins or founds one.
        assert_eq!(
            click(hut("blockhuttownhall"), &member.stranger()),
            Answer::Ask(Question::ColonyInfo)
        );
        // Nor is a town hall whose colony the client does not know yet.
        let unknown = Facts {
            building: false,
            ..member
        };
        assert_eq!(
            click(hut("blockhuttownhall"), &unknown),
            Answer::Ask(Question::ColonyInfo)
        );
        // Sneaking is passed through, which is what opens the upgrade tab.
        let sneaking = Facts {
            sneaking: true,
            ..member
        };
        assert_eq!(
            click(hut("blockhuttownhall"), &sneaking),
            Answer::Open {
                window: Window::ColonyOverview,
                shift: true
            }
        );
    }

    #[test]
    fn a_hut_that_knows_no_colony_says_so_before_it_says_anything_else() {
        let member = Facts::member();
        let orphan = Facts {
            building_has_colony: false,
            ..member
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &orphan),
            Answer::Refused(Refusal::NoColony),
            "HUT_BLOCK_MISSING_COLONY"
        );
        let unregistered = Facts {
            building: false,
            ..member
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &unregistered),
            Answer::Refused(Refusal::NoBuilding),
            "HUT_BLOCK_MISSING_BUILDING"
        );
        // The off hand comes first of all (`:187-190`).
        let off_hand = Facts {
            off_hand: true,
            building: false,
            ..member
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &off_hand),
            Answer::Refused(Refusal::OffHand)
        );
    }

    #[test]
    fn a_deactivated_hut_opens_the_reactivation_screen_or_names_the_missing_colony() {
        let member = Facts::member();
        let deactivated = Facts {
            deactivated: true,
            building: false,
            ..member
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &deactivated),
            Answer::Open {
                window: Window::Reactivation,
                shift: false
            }
        );
        let no_colony = Facts {
            chunk_owned: false,
            ..deactivated
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &no_colony),
            Answer::Refused(Refusal::MissingColony),
            "MISSING_COLONY"
        );
        // A parent building claiming the position means the hut is not orphaned:
        // Java falls through to the ordinary path.
        let claimed = Facts {
            claiming_building_here: true,
            ..deactivated
        };
        assert_eq!(
            click(hut("blockhutbuilder"), &claimed),
            Answer::Refused(Refusal::NoBuilding)
        );
    }

    #[test]
    fn a_hut_keeps_its_building_name_which_is_not_always_its_block_name() {
        assert_eq!(kind_of("minecolonies:blockhutcitizen"), Ok(Kind::Residence));
        assert_eq!(kind_of("minecolonies:blockhutbakery"), Err(Refusal::UnknownBlock {
            path: "minecolonies:blockhutbakery".to_owned()
        }), "the block is `blockhutbaker`, the building `baker`");
        // The blueprint is named after the building, and for four huts that is
        // not the block's name.
        assert_eq!(
            blueprint_name(hut("blockhutcitizen")).as_deref(),
            Some("residence")
        );
        assert_eq!(
            blueprint_name(hut("blockhutbuilder")).as_deref(),
            Some("builder")
        );
        assert_eq!(
            blueprint_name(hut("simplequarry")).as_deref(),
            Some("simplequarry"),
            "the quarry's block name and building name are the same"
        );
        assert_eq!(blueprint_name(hut("blockhutwarehouse")).as_deref(), Some("warehouse"));
    }

    #[test]
    fn a_hut_s_level_comes_out_of_its_schematic_name() {
        assert_eq!(schematic_level("blockhutbuilder1"), Some(1));
        assert_eq!(schematic_level("blockhutfarmer5"), Some(5));
        assert_eq!(schematic_level("builder3"), Some(3));
        // No digit at all: Java logs and answers 0, the port answers `None`.
        assert_eq!(schematic_level("blockhutbuilder"), None);
        assert_eq!(schematic_level(""), None);
        // Java glues the digits together, so the port does too.
        assert_eq!(schematic_level("level1copy2"), Some(12));
    }

    #[test]
    fn the_barracks_tower_is_never_drawn_and_the_others_follow_the_data() {
        assert!(!visible(hut("blockhutbarrackstower"), false));
        assert!(!visible(hut("blockhutbarrackstower"), true));
        assert!(visible(hut("blockhutbarracks"), false));
        assert!(
            !visible(hut("blockhutbarracks"), true),
            "a blueprint tagged `invisible` is not drawn"
        );
    }

    #[test]
    fn every_row_points_at_one_file_and_the_file_names_the_class_it_claims() {
        let mut sources: Vec<&str> = BLOCKS.iter().map(|block| block.source).collect();
        let count = sources.len();
        sources.sort_unstable();
        sources.dedup();
        assert_eq!(sources.len(), count, "two huts claim the same class");
        for block in BLOCKS {
            let (file, line) = block
                .source
                .split_once(':')
                .unwrap_or_else(|| panic!("{} has no line: {}", block.name, block.source));
            assert!(
                file.starts_with("core/blocks/huts/"),
                "{}: {file} is not in the huts package",
                block.name
            );
            assert!(line.parse::<u32>().is_ok(), "{}: `{line}` is no line", block.name);
            let stem = file.rsplit('/').next().unwrap_or_default();
            let exception = matches!(
                stem,
                "SimpleQuarry.java" | "MediumQuarry.java" | "BlockPostBox.java" | "BlockStash.java"
            );
            if !exception {
                // `BlockHutChickenHerder.java` is `blockhutchickenherder`, and
                // `BlockHutWareHouse.java` is `blockhutwarehouse`: one Java class
                // per hut, and the case is the only difference.
                let class = stem
                    .strip_prefix("BlockHut")
                    .and_then(|rest| rest.strip_suffix(".java"))
                    .unwrap_or_else(|| panic!("{}: {stem} is no BlockHut class", block.name));
                assert_eq!(
                    format!("blockhut{}", class.to_lowercase()),
                    block.name,
                    "the file and the row disagree"
                );
            }
        }
        // The five exceptions, named so a rename cannot pass unnoticed.
        for name in [
            "simplequarry",
            "mediumquarry",
            "blockpostbox",
            "blockstash",
            "blockhutbuilder",
        ] {
            assert!(BLOCKS.iter().any(|block| block.name == name), "{name}");
        }
    }
}
