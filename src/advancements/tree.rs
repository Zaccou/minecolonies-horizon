//! The tree: 62 advancements, their parent, their frame, their condition.
//!
//! Java reference: the generated data under
//! `src/datagen/generated/minecolonies/data/minecolonies/advancements/`, one
//! JSON per advancement. What is kept is `parent`, `display.frame` and the
//! criteria' `trigger` + `conditions`; what is dropped is everything the game
//! needs to *draw* it (`display.icon`, `title`/`description` translation keys,
//! `background`, `show_toast`, `announce_to_chat`, `sends_telemetry_event`),
//! which is content, not logic.
//!
//! **The shape of the tree.** Three roots of its own — `minecolonies/root`,
//! `production/root`, `military/root` — and one advancement whose parent lives
//! outside the mod (`minecraft/craft_supply` hangs under
//! `minecraft:story/root`). A parent inside the tree is a short id
//! (`minecolonies/build_builder`), the folder-qualified path the JSON uses
//! without its `minecolonies:` namespace; a full registry id is
//! [`Advancement::full_id`].
//!
//! **`requirements`, and why a condition is a conjunction.** Minecraft reads
//! `requirements` as "one criterion per group, all groups" — `[[a],[b]]` is
//! `a AND b`, `[[a,b]]` is `a OR b`. 58 rows have a single criterion; four have
//! more (`production/build_all_herders` 5, `minecolonies/building_add_recipe_door`
//! 6, `production/post_and_stash` 2, `minecolonies/fulfill_request` 2 in *one*
//! group, so an OR). [`Condition`] carries both shapes and no row needs a
//! general expression tree.
//!
//! **`hut_name` is a building *type*, not a block.** The completion trigger is
//! fired with `getBuildingType().getRegistryName().getPath()`
//! (`core/colony/workorders/WorkOrderBuilding.java:256`), whose paths are
//! `ModBuildings.java:10-55` (`baker`, `townhall`, `barrackstower`,
//! `residence`, `mysticalsite`, `stonesmeltery` …) — bare, no namespace, and
//! *not* the hut block id (`minecolonies:blockhutbaker`). Every `hut` field in
//! this file is that path.
//!
//! **The table is data, not code.** Its rows are in the order of the files, so a
//! reviewer can diff one against the other; the helpers (`built`, `pop`,
//! `guards`, `saw`, `clicked`) only spare the punctuation, and a row that does
//! not fit wraps rather than hiding an argument.

use super::condition::Condition;
use super::state::Event;
use Condition::*;
use Event::*;
use Frame::{Challenge, Goal, Task};

/// What an advancement looks like in the book. `frame` is `display.frame`, and
/// it is the only display field with a meaning outside the client: 51 `task`,
/// 7 `goal`, 4 `challenge`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Frame {
    Task,
    Goal,
    Challenge,
}

/// Where an advancement hangs in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Parent {
    /// No `parent` key: this is a root of the mod's own trees.
    Root,
    /// An advancement of this mod, by short id.
    Inside(&'static str),
    /// A parent in another mod's or vanilla's tree — `minecraft:story/root`
    /// for `minecraft/craft_supply`. The port cannot read that tree, so
    /// [`super::unlock::Progress`] does not gate on it.
    Outside(&'static str),
}

/// One advancement: its id, where it hangs, how it is framed, and what it asks
/// for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Advancement {
    /// The folder-qualified path, as the JSON file path spells it:
    /// `minecolonies/build_town_hall`.
    pub id: &'static str,
    pub parent: Parent,
    pub frame: Frame,
    pub condition: Condition,
}

impl Advancement {
    /// The registry id the game would know it by:
    /// `minecolonies:minecolonies/build_town_hall`.
    pub fn full_id(&self) -> String {
        format!("minecolonies:{}", self.id)
    }
}

const fn root(id: &'static str, frame: Frame, condition: Condition) -> Advancement {
    Advancement {
        id,
        parent: Parent::Root,
        frame,
        condition,
    }
}

const fn child(
    id: &'static str,
    parent: &'static str,
    frame: Frame,
    condition: Condition,
) -> Advancement {
    Advancement {
        id,
        parent: Parent::Inside(parent),
        frame,
        condition,
    }
}

const fn outside(
    id: &'static str,
    parent: &'static str,
    frame: Frame,
    condition: Condition,
) -> Advancement {
    Advancement {
        id,
        parent: Parent::Outside(parent),
        frame,
        condition,
    }
}

/// `complete_build_request` with `hut_name` and `level`.
const fn built(hut: &'static str, level: i32) -> Condition {
    Condition::Built { hut, level }
}

/// `colony_population` with `population_count`.
const fn pop(at_least: u32) -> Condition {
    Condition::Population { at_least }
}

/// `army_population` with `population_count`.
const fn guards(at_least: u32) -> Condition {
    Condition::Army { at_least }
}

/// A condition that is one recorded event.
const fn saw(event: Event) -> Condition {
    Condition::Happened(event)
}

/// `click_gui_button` with `button_id` and `window_resource_location`.
const fn clicked(button: &'static str, window: &'static str) -> Condition {
    saw(GuiClicked { button, window })
}

/// The five herders `production/build_all_herders` wants, at level 1 each
/// (`requirements: [["sheep"],["cows"],["chickens"],["pigs"],["rabbits"]]`, so
/// five criteria, all required).
const HERDERS: &[(&str, i32)] = &[
    ("chickenherder", 1),
    ("cowboy", 1),
    ("swineherder", 1),
    ("rabbithutch", 1),
    ("shepherd", 1),
];

/// `minecolonies/building_add_recipe_door` asks for six recipes, each on a 3x3
/// grid (six criteria, all required).
const DOORS: &[Event] = &[
    RecipeAdded { output: "minecraft:acacia_door", grid: 3 },
    RecipeAdded { output: "minecraft:birch_door", grid: 3 },
    RecipeAdded { output: "minecraft:dark_oak_door", grid: 3 },
    RecipeAdded { output: "minecraft:jungle_door", grid: 3 },
    RecipeAdded { output: "minecraft:oak_door", grid: 3 },
    RecipeAdded { output: "minecraft:spruce_door", grid: 3 },
];

/// `minecolonies/fulfill_request`: the same button in either window — one
/// `requirements` group, so an OR, not an AND.
const FULFILL: &[Event] = &[
    GuiClicked { button: "fulfill", window: "minecolonies:gui/citizen/requests.xml" },
    GuiClicked { button: "fulfill", window: "minecolonies:gui/windowrequestdetail.xml" },
];

/// `production/post_and_stash`: a postbox and a stash, both placed.
const STASH: &[Event] = &[
    BlockPlaced { block: "minecolonies:blockpostbox" },
    BlockPlaced { block: "minecolonies:blockstash" },
];

/// The tree, in the order its files are written.
///
/// **The order is not the tree**: `parent` is what links, and a row may be read
/// before its parent. [`super::unlock::Progress::sweep`] repeats its pass until
/// nothing new opens, so a table in any order gives the same set.
pub const TREE: &[Advancement] = &[
    root("minecolonies/root", Task, saw(SupplyPlaced)),
    root("production/root", Task, built("builder", 1)),
    root("military/root", Task, built("guardtower", 1)),
    child(
        "minecolonies/place_townhall",
        "minecolonies/root",
        Task,
        saw(StructurePlaced { hut: "townhall" }),
    ),
    child(
        "minecolonies/start_builder",
        "minecolonies/place_townhall",
        Task,
        saw(BuildRequested { hut: "builder", level: 1 }),
    ),
    child(
        "minecolonies/fulfill_request",
        "minecolonies/start_builder",
        Task,
        HappenedAny(FULFILL),
    ),
    child("minecolonies/build_builder", "minecolonies/fulfill_request", Goal, built("builder", 1)),
    child("minecolonies/build_builder_2", "minecolonies/build_builder", Goal, built("builder", 2)),
    child(
        "minecolonies/build_town_hall",
        "minecolonies/build_builder",
        Task,
        built("townhall", 1),
    ),
    child(
        "minecolonies/build_town_hall_5",
        "minecolonies/build_town_hall",
        Challenge,
        built("townhall", 5),
    ),
    child("minecolonies/build_tavern", "minecolonies/build_builder", Task, built("tavern", 1)),
    child("minecolonies/build_citizen", "minecolonies/build_tavern", Task, built("residence", 1)),
    child(
        "minecolonies/build_guard_tower",
        "minecolonies/build_builder",
        Task,
        built("guardtower", 1),
    ),
    child(
        "minecolonies/build_mysticalsite",
        "minecolonies/build_builder",
        Task,
        built("mysticalsite", 1),
    ),
    child(
        "minecolonies/build_mysticalsite_5",
        "minecolonies/build_mysticalsite",
        Challenge,
        built("mysticalsite", 5),
    ),
    child(
        "minecolonies/check_out_guide",
        "minecolonies/place_townhall",
        Task,
        clicked("confirm", "minecolonies:gui/windowhutguide.xml"),
    ),
    child(
        "minecolonies/citizen_eat_food_first",
        "minecolonies/place_townhall",
        Task,
        saw(FoodEaten { item: None }),
    ),
    child(
        "minecolonies/citizen_eat_food_rotten_flesh",
        "minecolonies/citizen_eat_food_first",
        Task,
        saw(FoodEaten { item: Some("minecraft:rotten_flesh") }),
    ),
    child("minecolonies/citizen_bury", "minecolonies/build_tavern", Task, saw(CitizenBuried)),
    child(
        "minecolonies/citizen_resurrect",
        "minecolonies/citizen_bury",
        Task,
        saw(CitizenResurrected),
    ),
    child(
        "minecolonies/undertaker_totem",
        "minecolonies/citizen_resurrect",
        Task,
        saw(UndertakerTotem),
    ),
    child(
        "minecolonies/building_add_recipe_torch",
        "minecolonies/start_builder",
        Task,
        saw(RecipeAdded { output: "minecraft:torch", grid: 2 }),
    ),
    child(
        "minecolonies/building_add_recipe_door",
        "production/build_sawmill",
        Task,
        HappenedAll(DOORS),
    ),
    child("minecolonies/colony_population_5", "minecolonies/build_citizen", Goal, pop(5)),
    child("minecolonies/colony_population_10", "minecolonies/colony_population_5", Task, pop(10)),
    child("minecolonies/colony_population_25", "minecolonies/colony_population_10", Goal, pop(25)),
    child("minecolonies/colony_population_50", "minecolonies/colony_population_25", Goal, pop(50)),
    child(
        "minecolonies/colony_population_75",
        "minecolonies/colony_population_50",
        Challenge,
        pop(75),
    ),
    child("military/build_archery", "military/root", Task, built("archery", 1)),
    child("military/build_barracks", "military/root", Task, built("barracks", 1)),
    child(
        "military/build_barracks_tower",
        "military/build_barracks",
        Task,
        built("barrackstower", 1),
    ),
    child("military/build_combat_academy", "military/root", Task, built("combatacademy", 1)),
    child(
        "military/build_all_barracks_towers",
        "military/build_barracks_tower",
        Goal,
        saw(AllBarracksTowers),
    ),
    child("military/army_8", "military/root", Task, guards(8)),
    child("military/army_30", "military/army_8", Task, guards(30)),
    child("military/army_80", "military/army_30", Challenge, guards(80)),
    child("production/build_cook", "production/root", Task, built("cook", 1)),
    child("production/build_farmer", "production/root", Task, built("farmer", 1)),
    child("production/build_fisherman", "production/root", Task, built("fisherman", 1)),
    child("production/build_library", "production/root", Task, built("library", 1)),
    child("production/build_lumberjack", "production/root", Task, built("lumberjack", 1)),
    child("production/build_miner", "production/root", Task, built("miner", 1)),
    child("production/build_warehouse", "production/root", Task, built("warehouse", 1)),
    child("production/build_baker", "production/build_cook", Task, built("baker", 1)),
    child("production/build_composter", "production/build_farmer", Task, built("composter", 1)),
    child("production/build_florist", "production/build_composter", Task, built("florist", 1)),
    child("production/build_all_herders", "production/build_farmer", Goal, BuiltAll(HERDERS)),
    child(
        "production/build_delivery_person",
        "production/build_warehouse",
        Task,
        built("deliveryman", 1),
    ),
    child(
        "production/build_sawmill",
        "production/build_delivery_person",
        Task,
        built("sawmill", 1),
    ),
    child("production/build_blacksmith", "production/build_sawmill", Task, built("blacksmith", 1)),
    child("production/build_crusher", "production/build_sawmill", Task, built("crusher", 1)),
    child("production/build_sifter", "production/build_sawmill", Task, built("sifter", 1)),
    child("production/build_smeltery", "production/build_sawmill", Task, built("smeltery", 1)),
    child("production/build_smeltery_3", "production/build_smeltery", Task, built("smeltery", 3)),
    child(
        "production/build_smeltery_5",
        "production/build_smeltery_3",
        Task,
        built("smeltery", 5),
    ),
    child(
        "production/build_stone_smeltery",
        "production/build_sawmill",
        Task,
        built("stonesmeltery", 1),
    ),
    child("production/build_stonemason", "production/build_sawmill", Task, built("stonemason", 1)),
    child("production/build_enchanter", "production/build_library", Task, built("enchanter", 1)),
    child("production/deep_mine", "production/build_miner", Task, saw(DeepMine)),
    child("production/max_fields", "production/build_farmer", Task, saw(MaxFields)),
    child(
        "production/post_and_stash",
        "production/build_delivery_person",
        Task,
        HappenedAll(STASH),
    ),
    outside("minecraft/craft_supply", "minecraft:story/root", Task, saw(SupplyPlaced)),
];

/// An advancement by short id.
///
/// `TREE` is a `&'static [Advancement]`, so `iter()` already yields
/// `&'static Advancement` and the closure sees `&&Advancement`: no `copied()`
/// here, which would yield an owned `Advancement` and break every signature.
pub fn find(id: &str) -> Option<&'static Advancement> {
    TREE.iter().find(|advancement| advancement.id == id)
}

/// The three advancements with no parent inside the mod.
pub fn roots() -> impl Iterator<Item = &'static Advancement> {
    TREE.iter()
        .filter(|advancement| advancement.parent == Parent::Root)
}

/// The advancements that hang directly under `id`.
///
/// The comparison goes *through* the match rather than through
/// `Parent::Inside(id)`: `Parent` holds `&'static str`, and `id` is the caller's
/// borrowed id, which is not `'static`.
pub fn children(id: &str) -> impl Iterator<Item = &'static Advancement> + '_ {
    TREE.iter().filter(move |advancement| {
        match advancement.parent {
            Parent::Inside(parent) => parent == id,
            Parent::Root | Parent::Outside(_) => false,
        }
    })
}

/// The parent's short id, when it is one of this mod's advancements.
pub fn parent_of(id: &str) -> Option<&'static str> {
    match find(id)?.parent {
        Parent::Inside(parent) => Some(parent),
        Parent::Root | Parent::Outside(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_row_has_a_unique_id_and_a_parent_that_exists_or_is_named_as_foreign() {
        let mut seen = BTreeSet::new();
        for advancement in TREE {
            assert!(
                seen.insert(advancement.id),
                "{} is declared twice, so `find` would answer with the first only",
                advancement.id
            );
            if let Parent::Inside(parent) = advancement.parent {
                assert!(
                    find(parent).is_some(),
                    "{} hangs under `{parent}`, which no row declares",
                    advancement.id
                );
            }
        }
    }

    #[test]
    fn the_three_roots_are_the_mods_own_and_craft_supply_hangs_outside() {
        let roots: Vec<&str> = roots().map(|advancement| advancement.id).collect();
        assert_eq!(
            roots,
            ["minecolonies/root", "production/root", "military/root"]
        );
        assert_eq!(
            find("minecraft/craft_supply").map(|advancement| advancement.parent),
            Some(Parent::Outside("minecraft:story/root"))
        );
        assert_eq!(parent_of("minecolonies/root"), None);
        assert_eq!(
            parent_of("minecolonies/build_town_hall"),
            Some("minecolonies/build_builder")
        );
        assert_eq!(
            find("minecolonies/build_town_hall").map(Advancement::full_id),
            Some("minecolonies:minecolonies/build_town_hall".to_owned())
        );
    }

    #[test]
    fn the_frames_split_fifty_one_seven_four_as_the_json_does() {
        let count = |frame: Frame| TREE.iter().filter(|a| a.frame == frame).count();
        assert_eq!(count(Task), 51);
        assert_eq!(count(Goal), 7);
        assert_eq!(count(Challenge), 4);
    }

    #[test]
    fn the_children_of_the_town_hall_node_are_the_three_the_json_names() {
        let children: Vec<&str> = children("minecolonies/place_townhall")
            .map(|advancement| advancement.id)
            .collect();
        assert_eq!(
            children,
            [
                "minecolonies/start_builder",
                "minecolonies/check_out_guide",
                "minecolonies/citizen_eat_food_first",
            ]
        );
    }
}
