//! `ClientEventHandler.java`: the client half, and the one rule the API cannot carry.
//!
//! Java reference: `ClientEventHandler.java`, 506 lines, 19 branches, all in
//! [`super::table`]. Five of them are functions here — the duplicate-entity sweep, the dyed
//! tooltip, the food tooltip, the recipe lines and the F3 line — because each is a
//! decision over values. Five are `Missing`: the world overlay, its
//! nearby-colony cache, the citizen voice, the schematic tier and the client-side
//! entity join, and `MANQUANT.md` names the primitive each one needs.
//!
//! **The tooltip is the reason this file exists.** `ClientEventHandler` adds seven
//! different lines to one item's tooltip, in a fixed order, and three of them are
//! coloured by a comparison the port already has (`completed research`, `building at
//! level`, `menu contains the food`). Rendering those lines belongs to
//! `src/render/`; **deciding** whether each line is aqua, red, grey or absent is here,
//! and that is the part a test can hold still.
//!
//! **One asymmetry worth naming.** Java's tooltip handler can return early four
//! times (`:175`, `:205`, `:239`, and the `catch` around the recipe lookup), and the
//! four early returns are not the same rule: no player, no colony, an inedible food,
//! and a missing building entry. None of them is collapsed here into "no lines": the
//! caller stops at the same four places, and each function below is only reached once
//! its own stop has passed — `food_tooltip` is asked only with a citizen inventory
//! open, `research_colour` only with the recipe's research list in hand.

/// `ModSoundEvents.CITIZEN_SOUND_EVENT_PREFIX` (`api/sounds/ModSoundEvents.java:24`):
/// every citizen voice's path starts with it.
pub const CITIZEN_SOUND_PREFIX: &str = "citizen.";
/// The namespace those sounds live in (`Constants.MOD_ID`).
pub const MOD_NAMESPACE: &str = "minecolonies";
/// `TranslationConstants.HUT_NEEDS_RESEARCH_TOOLTIP_1` (`EventHandler.java`'s twin
/// lives in `ClientEventHandler.java:434`).
pub const HUT_NEEDS_RESEARCH_1: &str = "com.minecolonies.coremod.item.tooltip.hut.research1";
/// Its second line (`:435`).
pub const HUT_NEEDS_RESEARCH_2: &str = "com.minecolonies.coremod.item.tooltip.hut.research2";
/// `DebugTranslationConstants.DEBUG_NO_CLOSE_COLONY` (`:11`).
pub const DEBUG_NO_CLOSE_COLONY: &str = "com.minecolonies.coremod.gui.debugscreen.noclosecolony";
/// `DebugTranslationConstants.DEBUG_NEXT_COLONY` (`:13`).
pub const DEBUG_NEXT_COLONY: &str = "com.minecolonies.coremod.gui.debugscreen.nextcolony";
/// `DebugTranslationConstants.DEBUG_BLOCKS_FROM_CENTER` (`:15`).
pub const DEBUG_BLOCKS_FROM_CENTER: &str =
    "com.minecolonies.coremod.gui.debugscreen.blocksfromcenter";
/// The sentence Java removes before inserting its own (`:190`).
pub const VANILLA_DYED_LINE: &str = "item.dyed";

// ── the duplicate-entity sweep ───────────────────────────────────────────────

/// `ClientEventHandler.java:108-131`: the ids to discard.
///
/// Java walks every entity of the client level and keeps the ids of the ones that
/// share the joining entity's UUID but are not it. `joining` is the entity that just
/// arrived; `entities` is `entityStorage.getEntityGetter().getAll()` as
/// `(uuid, entity id)`.
pub fn stale_copies(joining_uuid: u128, joining_id: u32, entities: &[(u128, u32)]) -> Vec<u32> {
    entities
        .iter()
        .filter(|(uuid, id)| *uuid == joining_uuid && *id != joining_id)
        .map(|(_, id)| *id)
        .collect()
}

// ── the citizen voice ────────────────────────────────────────────────────────

/// `ClientEventHandler.java:142-158`: whether the sound is muted.
///
/// `sound_location` is `namespace:path` as the sound event names it. Both terms of
/// the Java test are needed: the namespace alone would mute every `minecraft:` sound
/// and the prefix alone would mute another mod's `citizen.` sounds.
pub fn mutes_citizen_voice(config_on: bool, sound_location: &str) -> bool {
    if config_on {
        return false;
    }
    let Some((namespace, path)) = sound_location.split_once(':') else {
        return false;
    };
    namespace == MOD_NAMESPACE && path.starts_with(CITIZEN_SOUND_PREFIX)
}

// ── the dyed tooltip ─────────────────────────────────────────────────────────

/// One line of a tooltip: the translation key it carries, when it has one.
///
/// Java inspects `line.getContents() instanceof TranslatableContents` and compares the
/// key (`:190`); a line with no key is text the mod cannot recognise and must leave
/// alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub key: Option<&'static str>,
    pub text: String,
}

impl Line {
    pub fn keyed(key: &'static str, text: &str) -> Line {
        Line {
            key: Some(key),
            text: text.to_owned(),
        }
    }

    pub fn plain(text: &str) -> Line {
        Line {
            key: None,
            text: text.to_owned(),
        }
    }
}

/// `ClientEventHandler.java:186-196`: remove vanilla's `item.dyed` line and insert the
/// colony's colour **at index 1**. Answers whether it wrote anything.
///
/// **`None` writes nothing at all, and does not even remove.** Java's whole body sits
/// in `getDyeColor(stack).ifPresent(c -> …)` (`:188`), so a dyeable item the
/// compatibility manager knows no colour for keeps vanilla's grey line. An audit
/// against the Java found this: the first version of this function always rewrote the
/// line, because the colour was a parameter instead of an option.
///
/// The index is Java's, not an accident of ordering: the tooltip is
/// `[name, <here>, …]`, so the colour sits right under the name whatever the item's
/// own lines are. With a colour and no `item.dyed` line the insertion is still at 1 —
/// the `removeIf` runs first and unconditionally inside the lambda.
pub fn dyed_tooltip(lines: &mut Vec<Line>, colour_name: Option<&str>) -> bool {
    let Some(colour_name) = colour_name else {
        return false;
    };
    lines.retain(|line| line.key != Some(VANILLA_DYED_LINE));
    let line = Line::keyed(
        VANILLA_DYED_LINE,
        &format!("{VANILLA_DYED_LINE}: color.minecraft.{colour_name}"),
    );
    let at = if lines.is_empty() { 0 } else { 1.min(lines.len()) };
    lines.insert(at, line);
    true
}

// ── the food tooltip ─────────────────────────────────────────────────────────

/// The four sentences `ClientEventHandler.java:234-273` can add, and in which case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoodLine {
    /// `!FoodUtils.EDIBLE.test(stack)` → red `wrongfood`, and Java **returns**
    /// (`:236-240`): no tier, no menu line.
    WrongFood,
    /// `TIER_TOOLTIP + tier`, grey.
    Tier(i32),
    /// `canEatLevel` but the nearest cook's restaurant menu does not hold the stack:
    /// red `nomenu` (`:264-267`).
    NoMenu,
    /// `!canEatLevel(stack, homeLevel)` → red `needbetterfood` (`:269-272`).
    NeedBetterFood,
}

/// The lines a food stack gets in an open citizen inventory.
///
/// `edible` is `FoodUtils.EDIBLE`, `can_eat_at_home` is
/// `FoodUtils.canEatLevel(stack, homeBuildingLevel)`, and `on_menu` is
/// `restaurantMenu.getMenu().contains(new ItemStorage(stack))` — false when there is
/// no cook building at all, which is Java's `cookBuilding != null && …` (`:264`).
pub fn food_tooltip(edible: bool, tier: i32, can_eat_at_home: bool, on_menu: bool) -> Vec<FoodLine> {
    if !edible {
        return vec![FoodLine::WrongFood];
    }
    if can_eat_at_home {
        let mut lines = vec![FoodLine::Tier(tier)];
        if !on_menu {
            lines.push(FoodLine::NoMenu);
        }
        return lines;
    }
    vec![FoodLine::NeedBetterFood]
}

// ── the recipe and building lines ────────────────────────────────────────────

/// The colour a tooltip line is written in (`ChatFormatting`, `:327-332`, `:358-363`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    /// The colony has it.
    Aqua,
    /// It does not.
    Red,
    /// "available", the level-0 line (`:369`).
    Grey,
}

/// `:324-332`: aqua when the colony completed the research **or** holds an effect of
/// that strength, red otherwise. A tooltip shown outside a colony is red, because
/// `colony != null` is part of the test.
pub fn research_colour(in_colony: bool, completed: bool, effect_strength: i32) -> Colour {
    if in_colony && (completed || effect_strength > 0) {
        Colour::Aqua
    } else {
        Colour::Red
    }
}

/// `:343-371`: a recipe's crafting building line — nothing when the minimum level is
/// 0 or less? **No**: a level `<= 0` still produces a line, the italic grey
/// "available" one (`:366-371`). What is skipped is a level above 0 the colony does
/// not have, which is red.
pub fn building_colour(in_colony: bool, has_at_level: bool) -> Colour {
    if in_colony && has_at_level {
        Colour::Aqua
    } else {
        Colour::Red
    }
}

/// Which of the two building lines a recipe produces, from its minimum level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingLine {
    /// `minimumLevel > 0`: `COM_MINECOLONIES_COREMOD_ITEM_BUILDLEVEL_TOOLTIP_GUI`.
    Level(i32),
    /// `minimumLevel <= 0`: `…_AVAILABLE_TOOLTIP_GUI`, italic grey.
    Available,
}

impl BuildingLine {
    /// The colour this line is written in, when it has one.
    pub fn colour(self, in_colony: bool, has_at_level: bool) -> Option<Colour> {
        match self {
            BuildingLine::Level(_) => Some(building_colour(in_colony, has_at_level)),
            BuildingLine::Available => Some(Colour::Grey),
        }
    }
}

/// `:347-371`: the line a minimum building level produces.
pub fn building_line(minimum_level: i32) -> BuildingLine {
    if minimum_level > 0 {
        BuildingLine::Level(minimum_level)
    } else {
        BuildingLine::Available
    }
}

/// A recipe that `handleCrafterRecipeTooltips` keeps: it has to say it wants a
/// tooltip **and** name a crafter of at least two characters (`:295`).
///
/// The length test is not cosmetic: `getCrafter()` is empty for the recipes the
/// manager cannot attribute, and an empty key would match every building if the map
/// were consulted with it.
pub fn recipe_is_shown(show_tooltip: bool, crafter: &str) -> bool {
    show_tooltip && crafter.len() >= 2
}

// ── the F3 lines ─────────────────────────────────────────────────────────────

/// The one line `onDebugOverlay` adds (`ClientEventHandler.java:442-475`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugLine {
    /// No colony here and the position is far enough from all of them
    /// (`:456`).
    NoCloseColony,
    /// No colony here but one is next: its distance and the configured minimum
    /// between town halls (`:466-470`).
    NextColony { distance: i64, minimum: i32 },
    /// A colony here: its name and the distance to its centre (`:473`).
    InColony { name: String, distance: i64 },
}

/// `:451-474`, with the distance Java computes: `(int) Math.sqrt(getDistanceSquared(pos))`
/// — a floor, so a colony three blocks away reads 1.
pub fn debug_line(
    colony_here: Option<&str>,
    far_enough: bool,
    next: Option<(&str, i64, i32)>,
    distance_squared: i64,
) -> Option<DebugLine> {
    if let Some(name) = colony_here {
        return Some(DebugLine::InColony {
            name: name.to_owned(),
            distance: distance(distance_squared),
        });
    }
    if far_enough {
        return Some(DebugLine::NoCloseColony);
    }
    next.map(|(_, squared, minimum)| DebugLine::NextColony {
        distance: distance(squared),
        minimum,
    })
}

/// `(int) Math.sqrt(d)` on a squared distance: truncation, not rounding.
pub fn distance(distance_squared: i64) -> i64 {
    if distance_squared < 0 {
        return 0;
    }
    let mut root = 0i64;
    // Integer square root by Newton's method: the crate has no `f64::sqrt` guarantee
    // on wasm and the value is a block count, so the integer answer is the exact one
    // Java's `(int)` cast produces.
    if distance_squared > 0 {
        root = distance_squared;
        let mut next = (root + 1) / 2;
        while next < root {
            root = next;
            next = (root + distance_squared / root) / 2;
        }
    }
    root
}

// ── the browse click ─────────────────────────────────────────────────────────

/// `ClientEventHandler.java:477-505`: whether the click opens the building browser.
///
/// Three tests in Java's order: the client side, the main hand with a `BlockItem`,
/// and the Forge workaround — a hit on a block also fires `RightClickItem`, so a
/// non-`MISS` `hitResult` is filtered out (`:490`). Only then the block's
/// `shouldBrowseBuildings`.
pub fn browse_click(
    client: bool,
    main_hand: bool,
    block_item: bool,
    hit_is_miss: bool,
    browsable: bool,
) -> bool {
    client && main_hand && block_item && hit_is_miss && browsable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_of_the_joining_entity_is_discarded_and_the_entity_itself_is_not() {
        let entities = [(7u128, 3u32), (7, 9), (8, 4)];
        assert_eq!(stale_copies(7, 3, &entities), vec![9]);
    }

    #[test]
    fn a_join_with_no_copy_discards_nothing() {
        let entities = [(8u128, 4u32)];
        assert!(stale_copies(7, 3, &entities).is_empty());
    }

    #[test]
    fn the_citizen_voice_is_muted_by_namespace_and_prefix() {
        assert!(mutes_citizen_voice(false, "minecolonies:citizen.farmer.male1.no"));
        assert!(!mutes_citizen_voice(true, "minecolonies:citizen.farmer.male1.no"));
        assert!(!mutes_citizen_voice(false, "minecraft:block.note_block.harp"));
        assert!(!mutes_citizen_voice(false, "othermod:citizen.loud"));
        assert!(!mutes_citizen_voice(false, "citizen.nocolon"));
    }

    #[test]
    fn the_vanilla_dyed_line_is_replaced_at_index_one() {
        let mut lines = vec![
            Line::plain("Netherite Helmet"),
            Line::keyed(VANILLA_DYED_LINE, "item.dyed"),
            Line::plain("When on head:"),
        ];
        assert!(dyed_tooltip(&mut lines, Some("red")));
        assert_eq!(lines.len(), 3, "the vanilla line is removed, the new one added");
        assert_eq!(lines[0].text, "Netherite Helmet", "the name stays first");
        assert_eq!(lines[1].key, Some(VANILLA_DYED_LINE));
        assert!(lines[1].text.contains("color.minecraft.red"));
        assert_eq!(lines[2].text, "When on head:");
    }

    #[test]
    fn a_second_colour_line_does_not_accumulate() {
        let mut lines = vec![Line::plain("name")];
        assert!(dyed_tooltip(&mut lines, Some("red")));
        assert!(dyed_tooltip(&mut lines, Some("blue")));
        assert_eq!(lines.len(), 2);
        assert!(lines[1].text.contains("blue"));
    }

    #[test]
    fn an_unknown_colour_leaves_the_vanilla_line_alone() {
        // `getDyeColor(stack).ifPresent(...)` (`:188`): no colour, no `removeIf` either.
        let mut lines = vec![
            Line::plain("Leather Boots"),
            Line::keyed(VANILLA_DYED_LINE, "item.dyed"),
        ];
        assert!(!dyed_tooltip(&mut lines, None));
        assert_eq!(lines.len(), 2, "nothing was touched");
        assert_eq!(lines[1].text, "item.dyed");
    }

    #[test]
    fn an_inedible_food_stops_at_one_red_line() {
        assert_eq!(
            food_tooltip(false, 3, true, true),
            vec![FoodLine::WrongFood],
            "`:236-240` returns before the tier is read"
        );
    }

    #[test]
    fn an_edible_food_at_home_shows_its_tier() {
        assert_eq!(food_tooltip(true, 2, true, true), vec![FoodLine::Tier(2)]);
    }

    #[test]
    fn food_the_cook_does_not_serve_is_flagged_after_the_tier() {
        assert_eq!(
            food_tooltip(true, 2, true, false),
            vec![FoodLine::Tier(2), FoodLine::NoMenu]
        );
    }

    #[test]
    fn food_too_good_for_the_house_gets_the_other_sentence() {
        assert_eq!(
            food_tooltip(true, 4, false, true),
            vec![FoodLine::NeedBetterFood],
            "`canEatLevel` false wins over the menu test"
        );
    }

    #[test]
    fn research_is_aqua_only_inside_a_colony_that_has_it() {
        assert_eq!(research_colour(true, true, 0), Colour::Aqua);
        assert_eq!(research_colour(true, false, 2), Colour::Aqua);
        assert_eq!(research_colour(true, false, 0), Colour::Red);
        assert_eq!(research_colour(false, true, 5), Colour::Red);
    }

    #[test]
    fn a_level_line_is_red_outside_a_colony_that_has_it() {
        assert_eq!(building_line(3), BuildingLine::Level(3));
        assert_eq!(building_line(3).colour(true, true), Some(Colour::Aqua));
        assert_eq!(building_line(3).colour(true, false), Some(Colour::Red));
        assert_eq!(building_line(3).colour(false, true), Some(Colour::Red));
    }

    #[test]
    fn a_zero_level_is_the_grey_available_line() {
        assert_eq!(building_line(0), BuildingLine::Available);
        assert_eq!(building_line(-1), BuildingLine::Available);
        assert_eq!(building_line(0).colour(true, true), Some(Colour::Grey));
    }

    #[test]
    fn a_recipe_needs_a_tooltip_and_a_named_crafter() {
        assert!(recipe_is_shown(true, "sawmill"));
        assert!(!recipe_is_shown(false, "sawmill"));
        assert!(!recipe_is_shown(true, "s"));
        assert!(!recipe_is_shown(true, ""));
    }

    #[test]
    fn a_colony_here_always_wins_the_debug_line() {
        let line = debug_line(Some("Bridgewater"), false, Some(("Other", 100, 8)), 25);
        assert_eq!(
            line,
            Some(DebugLine::InColony {
                name: "Bridgewater".into(),
                distance: 5
            })
        );
    }

    #[test]
    fn no_colony_and_far_away_says_so() {
        assert_eq!(
            debug_line(None, true, None, 100),
            Some(DebugLine::NoCloseColony)
        );
    }

    #[test]
    fn no_colony_but_one_next_names_the_configured_minimum() {
        assert_eq!(
            debug_line(None, false, Some(("Next", 64, 8)), 0),
            Some(DebugLine::NextColony {
                distance: 8,
                minimum: 8
            })
        );
    }

    #[test]
    fn no_colony_and_no_next_adds_no_line() {
        assert_eq!(debug_line(None, false, None, 0), None);
    }

    #[test]
    fn the_distance_is_truncated_like_the_java_cast() {
        assert_eq!(distance(0), 0);
        assert_eq!(distance(3), 1, "(int) sqrt(3) = 1");
        assert_eq!(distance(4), 2);
        assert_eq!(distance(99), 9);
        assert_eq!(distance(100), 10);
        assert_eq!(distance(-4), 0, "a squared distance is never negative");
    }

    #[test]
    fn the_browse_click_needs_an_empty_hit_and_the_main_hand() {
        assert!(browse_click(true, true, true, true, true));
        assert!(!browse_click(false, true, true, true, true), "server side");
        assert!(!browse_click(true, false, true, true, true), "off hand");
        assert!(!browse_click(true, true, true, false, true), "a block was hit");
        assert!(!browse_click(true, true, true, true, false), "not browsable");
    }
}
