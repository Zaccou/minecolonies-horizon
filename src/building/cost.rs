//! What a kind costs, level by level — the **shape**, never the numbers.
//!
//! In Java there is no cost table: the builder walks the blueprint of the level
//! it is building and asks each step for the blocks it would place
//! (`core/entity/ai/workers/AbstractEntityAIStructureWithWorkOrder.java:254-322`),
//! merging them by item as it goes
//! (`core/colony/buildings/modules/BuildingResourcesModule.java:199-225`). The
//! list of materials is therefore **data**: it lives in the structure pack's
//! blueprints, not in the mod's source. This port keeps the same shape — a
//! `(kind, level) -> materials` table the data fills — and hard-codes no count,
//! because a port that shipped one would be wrong for every style pack and every
//! MineColonies update.
//!
//! The three things the table answers:
//!
//! - **what a level needs**, or a named refusal when the level does not exist;
//! - **what is still missing** given an [`Inventory`], which is what
//!   `BuildingBuilderResource.getMissingFromPlayer` answers in Java
//!   (`core/colony/buildings/utils/BuildingBuilderResource.java:70-76`), and is
//!   why the list is made of `Stack`s: it is inventory-compatible by
//!   construction;
//! - **how high the data goes**, which is the builder's real ceiling: Java's
//!   `getMaxBuildingLevel` is a constant 5
//!   (`api/util/constant/BuildingConstants.java:11`), a pack may ship fewer
//!   levels, and both bounds hold.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::kind::Kind;
use super::nbt;
use super::{LEVEL_MIN, MAX_BUILDING_LEVEL, Refusal};
use crate::inventory::{Inventory, ItemName, Stack};

/// The table's keys in a compound: a list of `(type, level, items)` records, one
/// per level. `item` and `count` are a `Stack`'s two halves and nothing else —
/// no components, no quality: a blueprint lists blocks, not enchantments.
pub const KEY_COSTS: &str = "costs";
pub const KEY_TYPE: &str = "type";
pub const KEY_LEVEL: &str = "level";
pub const KEY_ITEMS: &str = "items";
pub const KEY_ITEM: &str = "item";
pub const KEY_COUNT: &str = "count";

/// The materials of one building type, level by level.
///
/// Keys are `(kind, level)`; a level that no entry names **does not exist** for
/// that kind, which is what [`Self::needs`] refuses rather than returning an
/// empty list: "this level is free" and "this level is not in the data" are two
/// different facts, and a builder that confused them would build for nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CostTable {
    entries: BTreeMap<(Kind, i32), Vec<Stack>>,
}

impl CostTable {
    pub fn new() -> CostTable {
        CostTable::default()
    }

    /// Sets the materials of one level.
    ///
    /// Two normalisations, both so that the same data always reads back equal:
    /// **duplicate items are summed**, as the builder's own resource module sums
    /// them (`BuildingResourcesModule.java:199-225`), and the list is **sorted by
    /// item name**. A blueprint that lists 40 spruce planks in one step and 12 in
    /// another costs 52, not two entries a caller has to add up.
    ///
    /// An empty list is legal and means a level that needs nothing.
    pub fn insert(&mut self, kind: Kind, level: i32, materials: Vec<Stack>) -> Result<(), Refusal> {
        if level > MAX_BUILDING_LEVEL {
            return Err(Refusal::LevelAboveMax { level });
        }
        if level < LEVEL_MIN {
            return Err(Refusal::LevelOutOfRange { level });
        }
        let mut merged: Vec<Stack> = Vec::new();
        for stack in materials {
            match merged.iter_mut().find(|held| held.item() == stack.item()) {
                Some(held) => {
                    let item = held.item().clone();
                    let total = held.count().saturating_add(stack.count());
                    // Both counts are above zero, so the sum is too: a `Stack`
                    // cannot be empty (`src/inventory/stack.rs:87-95`).
                    *held = Stack::new(item, total).expect("two positive counts add up");
                }
                None => merged.push(stack),
            }
        }
        merged.sort_by(|a, b| a.item().as_str().cmp(b.item().as_str()));
        self.entries.insert((kind, level), merged);
        Ok(())
    }

    /// What one level of one kind needs, or [`Refusal::NoSuchLevel`] naming both.
    pub fn needs(&self, kind: Kind, level: i32) -> Result<&[Stack], Refusal> {
        match self.entries.get(&(kind, level)) {
            Some(materials) => Ok(materials),
            None => Err(Refusal::NoSuchLevel { kind, level }),
        }
    }

    /// Every level the data gives this kind, in order.
    pub fn levels(&self, kind: Kind) -> Vec<i32> {
        self.entries
            .keys()
            .filter(|(known, _)| *known == kind)
            .map(|(_, level)| *level)
            .collect()
    }

    /// The highest level the data gives this kind, which is the ceiling a
    /// builder may reach — bounded above by [`MAX_BUILDING_LEVEL`] by
    /// construction, since [`Self::insert`] refuses anything higher.
    pub fn max_level(&self, kind: Kind) -> Option<i32> {
        self.levels(kind).into_iter().max()
    }

    /// What is still missing to build this level, given what already stands in
    /// an inventory. Items it holds in full are absent from the answer, so an
    /// empty answer is "nothing more is needed" — the same reading as
    /// `getMissingFromPlayer` returning a negative or zero number.
    pub fn missing(
        &self,
        kind: Kind,
        level: i32,
        inventory: &Inventory<'_>,
    ) -> Result<Vec<Stack>, Refusal> {
        let mut missing = Vec::new();
        for need in self.needs(kind, level)? {
            let short = need.count().saturating_sub(inventory.count(need.item()));
            if short == 0 {
                continue;
            }
            let stack = Stack::new(need.item().clone(), short).map_err(|_| {
                // Unreachable: `short` is above zero, or the branch above ran.
                Refusal::NoSuchLevel { kind, level }
            })?;
            missing.push(stack);
        }
        Ok(missing)
    }

    /// The whole table as a compound, one record per level.
    pub fn to_compound(&self) -> Compound {
        let mut records = Vec::new();
        for ((kind, level), materials) in &self.entries {
            let path = kind.path();
            let items = materials
                .iter()
                .map(|stack| {
                    Compound::from([
                        (KEY_ITEM.into(), Tag::String(stack.item().as_str().into())),
                        (KEY_COUNT.into(), Tag::Int(stack.count() as i32)),
                    ])
                })
                .collect();
            records.push(Compound::from([
                (KEY_TYPE.into(), Tag::String(path.as_str().into())),
                (KEY_LEVEL.into(), Tag::Int(*level)),
                (KEY_ITEMS.into(), nbt::compound_list(items)),
            ]));
        }
        Compound::from([(KEY_COSTS.into(), nbt::compound_list(records))])
    }

    /// Reads a table. A record with an unknown kind, a level outside the range,
    /// or an item name the engine cannot parse is **skipped**: the data pack is
    /// wrong, not the world, and one bad level must not cost a colony its builder.
    pub fn from_compound(data: &Compound) -> CostTable {
        let mut table = CostTable::new();
        for record in nbt::compounds(data, KEY_COSTS) {
            let Some(kind) = nbt::text(record, KEY_TYPE)
                .as_deref()
                .and_then(Kind::from_path)
            else {
                continue;
            };
            let Some(level) = nbt::int(record, KEY_LEVEL) else {
                continue;
            };
            let mut materials = Vec::new();
            for item in nbt::compounds(record, KEY_ITEMS) {
                let name = nbt::text(item, KEY_ITEM).and_then(|name| ItemName::new(&name));
                let count = nbt::int(item, KEY_COUNT);
                if let (Some(item), Some(count)) = (name, count.map(|count| count.max(0) as u32)) {
                    if let Ok(stack) = Stack::new(item, count) {
                        materials.push(stack);
                    }
                }
            }
            let _ = table.insert(kind, level, materials);
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::SingleLimit;

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("a name the engine can parse")
    }

    fn stack(name: &str, count: u32) -> Stack {
        Stack::new(item(name), count).expect("a positive count")
    }

    fn filled() -> CostTable {
        let mut table = CostTable::new();
        table
            .insert(
                Kind::Builder,
                1,
                vec![stack("minecraft:oak_planks", 52), stack("minecraft:cobblestone", 16)],
            )
            .expect("level one");
        table
            .insert(Kind::Builder, 2, vec![stack("minecraft:stone_bricks", 64)])
            .expect("level two");
        table
            .insert(Kind::Farmer, 1, vec![])
            .expect("a level that needs nothing");
        table
    }

    #[test]
    fn a_level_reads_the_materials_it_costs_and_an_absent_one_is_named() {
        let table = filled();
        assert_eq!(
            table.needs(Kind::Builder, 1).map(|needs| needs.len()),
            Ok(2),
            "two kinds of block for level one"
        );
        assert_eq!(
            table.needs(Kind::Builder, 3),
            Err(Refusal::NoSuchLevel {
                kind: Kind::Builder,
                level: 3
            })
        );
        assert_eq!(
            table.needs(Kind::TownHall, 1),
            Err(Refusal::NoSuchLevel {
                kind: Kind::TownHall,
                level: 1
            }),
            "a kind the data never mentions has no level at all"
        );
        assert_eq!(table.max_level(Kind::Builder), Some(2));
        assert_eq!(table.levels(Kind::Builder), vec![1, 2]);
        assert_eq!(table.max_level(Kind::Warehouse), None);
    }

    #[test]
    fn duplicate_items_are_summed_and_the_list_is_canonical() {
        let mut table = CostTable::new();
        table
            .insert(
                Kind::Builder,
                1,
                vec![
                    stack("minecraft:oak_planks", 40),
                    stack("minecraft:cobblestone", 8),
                    stack("minecraft:oak_planks", 12),
                ],
            )
            .expect("level one");
        let needs = table.needs(Kind::Builder, 1).expect("level one");
        assert_eq!(needs.len(), 2, "one entry per item");
        assert_eq!(needs[0].item().as_str(), "minecraft:cobblestone", "sorted by item");
        assert_eq!(needs[1].count(), 52, "40 and 12 are the same cost as 52");
        // Inserting in another order gives the same table, so a save round trips
        // whatever order the data pack listed its blocks in.
        let mut other = CostTable::new();
        other
            .insert(
                Kind::Builder,
                1,
                vec![
                    stack("minecraft:oak_planks", 12),
                    stack("minecraft:oak_planks", 40),
                    stack("minecraft:cobblestone", 8),
                ],
            )
            .expect("level one");
        assert_eq!(other, table);
    }

    #[test]
    fn a_level_outside_the_range_is_refused_where_it_is_inserted() {
        let mut table = CostTable::new();
        assert_eq!(
            table.insert(Kind::Builder, 0, vec![]),
            Err(Refusal::LevelOutOfRange { level: 0 })
        );
        assert_eq!(
            table.insert(Kind::Builder, 6, vec![]),
            Err(Refusal::LevelAboveMax { level: 6 })
        );
        assert_eq!(table.insert(Kind::Builder, 5, vec![]), Ok(()), "five is the ceiling");
        assert_eq!(table.needs(Kind::Builder, 5).map(|needs| needs.len()), Ok(0), "a level may need nothing");
    }

    #[test]
    fn what_is_missing_is_what_the_inventory_lacks() {
        let table = filled();
        let limits = SingleLimit(64);
        let inventory = Inventory::with_slots(
            vec![
                Some(stack("minecraft:oak_planks", 50)),
                Some(stack("minecraft:cobblestone", 16)),
                None,
            ],
            &limits,
        );
        let missing = table.missing(Kind::Builder, 1, &inventory).expect("level one");
        assert_eq!(missing.len(), 1, "the cobblestone is covered in full");
        assert_eq!(missing[0].item().as_str(), "minecraft:oak_planks");
        assert_eq!(missing[0].count(), 2, "52 needed, 50 held");

        let stocked = Inventory::with_slots(
            vec![
                Some(stack("minecraft:oak_planks", 52)),
                Some(stack("minecraft:cobblestone", 16)),
            ],
            &limits,
        );
        assert!(
            table
                .missing(Kind::Builder, 1, &stocked)
                .expect("level one")
                .is_empty(),
            "nothing more is needed"
        );
    }

    #[test]
    fn the_cost_table_round_trips_level_by_level() {
        let table = filled();
        let read = CostTable::from_compound(&table.to_compound());
        assert_eq!(read, table, "same kinds, same levels, same counts");
        assert_eq!(
            read.needs(Kind::Builder, 1).expect("level one")[1].count(),
            52
        );
    }

    #[test]
    fn a_bad_record_is_skipped_and_the_rest_of_the_table_survives() {
        let mut table = CostTable::new();
        table
            .insert(Kind::Builder, 1, vec![stack("minecraft:oak_planks", 4)])
            .expect("level one");
        let mut data = table.to_compound();
        let mut records = nbt::compounds(&data, KEY_COSTS)
            .into_iter()
            .cloned()
            .collect::<Vec<Compound>>();
        records.push(Compound::from([
            (KEY_TYPE.into(), Tag::String("minecolonies:castle".into())),
            (KEY_LEVEL.into(), Tag::Int(1)),
        ]));
        records.push(Compound::from([
            (KEY_TYPE.into(), Tag::String("minecolonies:builder".into())),
            (KEY_LEVEL.into(), Tag::Int(9)),
        ]));
        data.insert(KEY_COSTS.into(), nbt::compound_list(records));
        let read = CostTable::from_compound(&data);
        assert_eq!(read, table, "an unknown kind and an out-of-range level are dropped");
    }
}
