//! What a research costs, and what a colony must already stand to begin it.
//!
//! Java reference: `api/research/IResearchCost.java`,
//! `api/research/costs/{SimpleItemCost,ListItemCost,TagItemCost}.java` (the
//! three cost types), `api/research/requirements/Building*ResearchRequirement.java`
//! and `ResearchResearchRequirement.java`, and the registry names in
//! `ModResearchCosts.java:17-19` / `ModResearchRequirements.java:17-20`.
//!
//! **The three costs, and which ones the data uses.** `costs` is an array whose
//! entries carry a `type`: `minecolonies:item_simple` (item + quantity),
//! `minecolonies:item_list` (items + quantity) or `minecolonies:item_tag` (tag +
//! quantity). The 217 files cost **247** entries: **240** `item_simple`, **7**
//! `item_tag`, **0** `item_list`. [`Cost::Any`] still exists because
//! `ListItemCost` does and a data pack may use it, and a test says it is the
//! one type no shipped row exercises.
//!
//! **A quantity is at least 1, by construction.** All three readers take
//! `Math.max(quantity, 1)` (`SimpleItemCost.java:63`, `:84`,
//! `ListItemCost.java:74`), so a `quantity: 0` in a data pack costs one item, not
//! zero. [`Cost`] stores the number it was given and the loader refuses a zero
//! rather than reproducing that clamp silently: a row that says 0 is a data bug
//! worth seeing.
//!
//! **A `building` requirement sums the levels of every building of that kind.**
//! This is the one rule that is easy to get wrong and it changes what the data
//! means: `hasBuilding(id, level, singleBuilding)` adds up
//! `getBuildingLevel()` over all buildings of the id and compares the **sum**
//! (`api/colony/managers/interfaces/ICommonRegisteredStructureManager.java:207-231`).
//! So `combat/cleave`'s `guardtower` at level 10 is *not* impossible: a guard
//! tower's own maximum is 5 (`core/colony/buildings/workerbuildings/BuildingGuardTower.java:24`),
//! and two of them at 5 satisfy 10. `minecolonies:single-building` is the other
//! reading — one building at or above the level — and
//! `minecolonies:alternate-building` is the same sum, tried on each of the
//! listed kinds (`BuildingAlternatesResearchRequirement.java:150-160`).

use std::collections::BTreeMap;

use super::gate::Refusal;
use crate::inventory::{Inventory, ItemName};

/// One entry of a research's `costs` array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three registry types, `ModResearchCosts.java:17-19`
pub enum Cost {
    /// `minecolonies:item_simple`: `quantity` of one item (`SimpleItemCost`).
    Item {
        item: &'static str,
        count: u32,
    },
    /// `minecolonies:item_tag`: `quantity` of anything in a tag
    /// (`TagItemCost`), resolved through [`TAGS`].
    Tag { tag: &'static str, count: u32 },
    /// `minecolonies:item_list`: `quantity` of anything in the list
    /// (`ListItemCost`), the items joined by `+`. No shipped row uses it.
    Any { items: &'static str, count: u32 },
}

/// One entry of a research's `requirements` array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four registry types, `ModResearchRequirements.java:17-20`
pub enum Requirement {
    /// `minecolonies:building`: the **sum** of the levels of every building of
    /// that kind is at least `level`.
    Building {
        building: &'static str,
        level: i32,
    },
    /// `minecolonies:single-building`: one building of that kind is at least
    /// `level`.
    SingleBuilding {
        building: &'static str,
        level: i32,
    },
    /// `minecolonies:alternate-building`: one of the listed kinds satisfies the
    /// `building` reading. The kinds are joined by `+`, as the table writes them.
    AlternateBuilding {
        buildings: &'static str,
        level: i32,
    },
    /// `minecolonies:research`: another research is finished
    /// (`ResearchResearchRequirement`). No shipped row uses it.
    Research { research: &'static str },
}

/// One item tag, and the items it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagList {
    /// The tag's full id, as the cost names it: `minecraft:iron_ores`.
    pub id: &'static str,
    /// Its members, in the order the data lists them.
    pub items: &'static [&'static str],
}

/// The five tags the 7 tag costs name. Read from the game's own data:
/// `minecraft:iron_ores`, `:gold_ores`, `:diamond_ores` and `:planks` from
/// `data/minecraft/tags/item/` of the extracted client jar, and
/// `minecolonies:concrete` from
/// `src/datagen/generated/minecolonies/data/minecolonies/tags/items/concrete.json`.
pub const TAGS: &[TagList] = &[
    TagList {
        id: "minecraft:iron_ores",
        items: &["minecraft:iron_ore", "minecraft:deepslate_iron_ore"],
    },
    TagList {
        id: "minecraft:gold_ores",
        items: &[
            "minecraft:gold_ore",
            "minecraft:nether_gold_ore",
            "minecraft:deepslate_gold_ore",
        ],
    },
    TagList {
        id: "minecraft:diamond_ores",
        items: &[
            "minecraft:diamond_ore",
            "minecraft:deepslate_diamond_ore",
        ],
    },
    TagList {
        id: "minecraft:planks",
        items: &[
            "minecraft:oak_planks",
            "minecraft:spruce_planks",
            "minecraft:birch_planks",
            "minecraft:jungle_planks",
            "minecraft:acacia_planks",
            "minecraft:dark_oak_planks",
            "minecraft:pale_oak_planks",
            "minecraft:crimson_planks",
            "minecraft:warped_planks",
            "minecraft:mangrove_planks",
            "minecraft:bamboo_planks",
            "minecraft:cherry_planks",
            "minecraft:poplar_planks",
        ],
    },
    TagList {
        id: "minecolonies:concrete",
        items: &[
            "minecraft:white_concrete",
            "minecraft:orange_concrete",
            "minecraft:magenta_concrete",
            "minecraft:light_blue_concrete",
            "minecraft:yellow_concrete",
            "minecraft:lime_concrete",
            "minecraft:pink_concrete",
            "minecraft:gray_concrete",
            "minecraft:light_gray_concrete",
            "minecraft:cyan_concrete",
            "minecraft:purple_concrete",
            "minecraft:blue_concrete",
            "minecraft:brown_concrete",
            "minecraft:green_concrete",
            "minecraft:red_concrete",
            "minecraft:black_concrete",
        ],
    },
];

/// The members of a tag, or [`Refusal::TagUnknown`] when the data names one this
/// port does not carry.
pub fn tag(id: &str) -> Result<&'static [&'static str], Refusal> {
    TAGS
        .iter()
        .find(|list| list.id == id)
        .map(|list| list.items)
        .ok_or_else(|| Refusal::TagUnknown {
            tag: id.to_string(),
        })
}

/// A `+`-joined list, split. The table joins an item list and an alternate
/// building group this way, so that the data stays a slice of the row itself
/// rather than an allocation beside it.
fn split(joined: &'static str) -> Vec<&'static str> {
    joined.split('+').collect()
}

impl Cost {
    /// How many items it takes (`getCount`).
    pub fn count(&self) -> u32 {
        match self {
            Cost::Item { count, .. } | Cost::Tag { count, .. } | Cost::Any { count, .. } => *count,
        }
    }

    /// The item ids that satisfy this cost, in order: one for
    /// [`Cost::Item`], the list for [`Cost::Any`], the tag's members for
    /// [`Cost::Tag`].
    pub fn members(&self) -> Result<Vec<&'static str>, Refusal> {
        match self {
            Cost::Item { item, .. } => Ok(vec![*item]),
            Cost::Any { items, .. } => Ok(split(*items)),
            Cost::Tag { tag: id, .. } => Ok(tag(id)?.to_vec()),
        }
    }

    /// How many items satisfying this cost the given inventories hold together:
    /// the university's first, then the player's
    /// (`GlobalResearch.java:183-230`). Java short-circuits when the building
    /// alone has enough; adding both is the same total, because every item is
    /// counted once against the same need.
    pub fn available(
        &self,
        university: &Inventory<'_>,
        player: &Inventory<'_>,
    ) -> Result<u32, Refusal> {
        let mut total = 0u32;
        for member in self.members()? {
            let name = ItemName::new(member).ok_or_else(|| Refusal::ItemUnparsable {
                item: member.to_string(),
            })?;
            total = total
                .saturating_add(university.count(&name))
                .saturating_add(player.count(&name));
        }
        Ok(total)
    }

    /// What is still missing, or `0` when the two inventories hold enough.
    pub fn missing(
        &self,
        university: &Inventory<'_>,
        player: &Inventory<'_>,
    ) -> Result<u32, Refusal> {
        Ok(self
            .count()
            .saturating_sub(self.available(university, player)?))
    }

    /// The same question as a `Result`: `Ok(())` or the first shortfall, named.
    pub fn met(&self, university: &Inventory<'_>, player: &Inventory<'_>) -> Result<(), Refusal> {
        let missing = self.missing(university, player)?;
        if missing == 0 {
            return Ok(());
        }
        Err(Refusal::CostMissing {
            need: self.count(),
            missing,
            what: self.what(),
        })
    }

    /// How the cost reads in a refusal: the item id, the tag id with its
    /// members, or the list.
    pub fn what(&self) -> String {
        match self {
            Cost::Item { item, .. } => (*item).to_string(),
            Cost::Tag { tag: id, .. } => match tag(id) {
                Ok(items) => format!("{id} ({})", items.join(", ")),
                Err(_) => (*id).to_string(),
            },
            Cost::Any { items, .. } => split(*items).join(" or "),
        }
    }
}

impl Requirement {
    /// Is it met by these buildings? The three building readings differ only in
    /// how many of the kind are asked (`ICommonRegisteredStructureManager.java:207-231`).
    pub fn met(&self, standing: &Buildings) -> bool {
        match self {
            Requirement::Building { building, level } => {
                standing.level_sum(building) >= *level
            }
            Requirement::SingleBuilding { building, level } => {
                standing.highest(building).is_some_and(|held| held >= *level)
            }
            Requirement::AlternateBuilding { buildings, level } => buildings
                .split('+')
                .any(|building| standing.level_sum(building) >= *level),
            // The fourth is answered by the caller's progress, not by buildings:
            // `Requirement::met` cannot see it, and `Research` says so by
            // answering `false`.
            Requirement::Research { .. } => false,
        }
    }

    /// The same question for a [`Requirement::Research`], which needs the
    /// colony's progress rather than its buildings.
    pub fn met_with(&self, standing: &Buildings, finished: &dyn Fn(&str) -> bool) -> bool {
        match self {
            Requirement::Research { research } => finished(research),
            other => other.met(standing),
        }
    }

    /// How the requirement reads in a refusal.
    pub fn what(&self) -> String {
        match self {
            Requirement::Building { building, level } => {
                format!("minecolonies:{building} levels summing to {level}")
            }
            Requirement::SingleBuilding { building, level } => {
                format!("minecolonies:{building} at level {level}")
            }
            Requirement::AlternateBuilding { buildings, level } => format!(
                "one of {} summing to {level}",
                buildings
                    .split('+')
                    .map(|building| format!("minecolonies:{building}"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Requirement::Research { research } => format!("research minecolonies:{research}"),
        }
    }
}

/// The buildings standing in a colony, as the requirement readers need them:
/// one entry per building, with its kind and its level.
///
/// A flat list, because [`Requirement::Building`] sums and
/// [`Requirement::SingleBuilding`] takes the maximum — a map from kind to one
/// level could not answer both. `crate::colony::Colony` keeps its buildings in a
/// map keyed by position and exposes only `building_at`
/// (`src/colony/colonie.rs:216-219`), so the caller builds this view; the report
/// says so instead of adding an accessor to a module this agent does not own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Buildings(BTreeMap<String, Vec<i32>>);

impl Buildings {
    pub fn new() -> Buildings {
        Buildings::default()
    }

    /// One standing building. A level below 0 is clamped to 0: a hut placed and
    /// not built is level 0 (`core/colony/buildings/AbstractSchematicProvider.java:64`).
    pub fn add(&mut self, building: &str, level: i32) {
        self.0
            .entry(building.to_string())
            .or_default()
            .push(level.max(0));
    }

    /// The sum of the levels of every building of that kind, `0` when none
    /// stands.
    pub fn level_sum(&self, building: &str) -> i32 {
        self.0
            .get(building)
            .map(|levels| levels.iter().sum())
            .unwrap_or(0)
    }

    /// The highest level of that kind, `None` when none stands. The difference
    /// matters: "no smeltery" and "a smeltery at level 0" are two answers, and
    /// only the second can grow.
    pub fn highest(&self, building: &str) -> Option<i32> {
        self.0.get(building).and_then(|levels| levels.iter().copied().max())
    }

    /// How many of that kind stand.
    pub fn count_of(&self, building: &str) -> usize {
        self.0.get(building).map(Vec::len).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{SingleLimit, Stack};

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("a name the engine can parse")
    }

    fn stack(name: &str, count: u32) -> Stack {
        Stack::new(item(name), count).expect("a positive count")
    }

    fn inventory(contents: &[(&str, u32)]) -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        let slots = contents
            .iter()
            .map(|(name, count)| Some(stack(name, *count)))
            .collect();
        Inventory::with_slots(slots, &LIMITS)
    }

    fn empty() -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        Inventory::new(4, &LIMITS)
    }

    #[test]
    fn a_simple_cost_is_counted_in_both_inventories() {
        let cost = Cost::Item {
            item: "minecraft:book",
            count: 8,
        };
        assert_eq!(cost.count(), 8);
        assert_eq!(cost.available(&empty(), &inventory(&[("minecraft:book", 5)])), Ok(5));
        assert_eq!(cost.missing(&empty(), &inventory(&[("minecraft:book", 5)])), Ok(3));
        assert_eq!(
            cost.missing(&inventory(&[("minecraft:book", 3)]), &inventory(&[("minecraft:book", 5)])),
            Ok(0),
            "the university's books count too"
        );
    }

    #[test]
    fn a_shortfall_names_the_item_and_both_numbers() {
        let cost = Cost::Item {
            item: "minecraft:book",
            count: 8,
        };
        let refusal = cost
            .met(&empty(), &inventory(&[("minecraft:book", 2)]))
            .expect_err("six books short");
        assert_eq!(
            refusal,
            Refusal::CostMissing {
                need: 8,
                missing: 6,
                what: "minecraft:book".to_string()
            }
        );
    }

    #[test]
    fn a_tag_cost_counts_every_member() {
        let cost = Cost::Tag {
            tag: "minecraft:iron_ores",
            count: 64,
        };
        let held = inventory(&[
            ("minecraft:iron_ore", 40),
            ("minecraft:deepslate_iron_ore", 24),
        ]);
        assert_eq!(cost.available(&empty(), &held), Ok(64));
        assert_eq!(cost.met(&empty(), &held), Ok(()));
        assert_eq!(cost.what(), "minecraft:iron_ores (minecraft:iron_ore, minecraft:deepslate_iron_ore)");
    }

    #[test]
    fn an_unknown_tag_is_a_named_refusal_not_a_zero() {
        let cost = Cost::Tag {
            tag: "minecolonies:no_such_tag",
            count: 4,
        };
        assert_eq!(
            cost.missing(&empty(), &empty()),
            Err(Refusal::TagUnknown {
                tag: "minecolonies:no_such_tag".to_string()
            })
        );
    }

    #[test]
    fn a_list_cost_takes_any_of_its_items_and_no_row_uses_it() {
        let cost = Cost::Any {
            items: "minecraft:stone+minecraft:cobblestone",
            count: 10,
        };
        let held = inventory(&[("minecraft:stone", 4), ("minecraft:cobblestone", 6)]);
        assert_eq!(cost.available(&empty(), &held), Ok(10));
        assert_eq!(cost.what(), "minecraft:stone or minecraft:cobblestone");
    }

    #[test]
    fn the_five_tags_are_the_ones_the_costs_name() {
        assert_eq!(TAGS.len(), 5);
        assert_eq!(tag("minecraft:planks").map(|items| items.len()), Ok(13));
        assert_eq!(tag("minecolonies:concrete").map(|items| items.len()), Ok(16));
        assert_eq!(tag("minecraft:iron_ores").map(|items| items.len()), Ok(2));
        assert_eq!(tag("minecraft:gold_ores").map(|items| items.len()), Ok(3));
        assert_eq!(
            tag("minecraft:diamond_ores").map(|items| items.len()),
            Ok(2)
        );
    }

    #[test]
    fn a_building_requirement_sums_the_levels() {
        let mut standing = Buildings::new();
        standing.add("guardtower", 5);
        standing.add("guardtower", 5);
        let requirement = Requirement::Building {
            building: "guardtower",
            level: 10,
        };
        assert!(requirement.met(&standing), "two towers at 5 sum to 10");
        assert_eq!(standing.level_sum("guardtower"), 10);
        assert_eq!(standing.highest("guardtower"), Some(5));
        assert_eq!(standing.count_of("guardtower"), 2);
    }

    #[test]
    fn a_single_building_requirement_does_not_sum() {
        let mut standing = Buildings::new();
        standing.add("sifter", 5);
        standing.add("sifter", 5);
        assert!(!Requirement::SingleBuilding {
            building: "sifter",
            level: 6
        }
        .met(&standing));
        assert!(Requirement::SingleBuilding {
            building: "sifter",
            level: 5
        }
        .met(&standing));
        assert!(!Requirement::SingleBuilding {
            building: "sifter",
            level: 4
        }
        .met(&Buildings::new()));
    }

    #[test]
    fn an_alternate_requirement_takes_the_first_kind_that_fits() {
        let mut standing = Buildings::new();
        standing.add("sawmill", 2);
        let requirement = Requirement::AlternateBuilding {
            buildings: "sawmill+stonemason",
            level: 3,
        };
        assert!(!requirement.met(&standing), "2 is below 3");
        standing.add("stonemason", 3);
        assert!(requirement.met(&standing));
        assert_eq!(
            requirement.what(),
            "one of minecolonies:sawmill or minecolonies:stonemason summing to 3"
        );
    }

    #[test]
    fn a_research_requirement_needs_the_progress_not_the_buildings() {
        let requirement = Requirement::Research {
            research: "civilian/keen",
        };
        assert!(
            !requirement.met(&Buildings::new()),
            "buildings cannot answer this one"
        );
        assert!(requirement.met_with(&Buildings::new(), &|id| id == "civilian/keen"));
        assert!(!requirement.met_with(&Buildings::new(), &|_| false));
        assert_eq!(requirement.what(), "research minecolonies:civilian/keen");
    }

    #[test]
    fn a_level_below_zero_is_a_hut_placed_not_built() {
        let mut standing = Buildings::new();
        standing.add("builder", -3);
        assert_eq!(standing.highest("builder"), Some(0));
        assert_eq!(standing.level_sum("builder"), 0);
    }

    #[test]
    fn a_kind_that_stands_nowhere_is_zero_not_a_level() {
        let standing = Buildings::new();
        assert_eq!(standing.level_sum("smeltery"), 0);
        assert_eq!(standing.highest("smeltery"), None);
        assert_eq!(standing.count_of("smeltery"), 0);
        assert!(!Requirement::SingleBuilding {
            building: "smeltery",
            level: 1
        }
        .met(&standing));
    }
}
