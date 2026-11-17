//! The loot tables: what a broken block leaves, what a raider drops, what a
//! supply crate holds, what a fishing trip or a sifter mesh turns up.
//!
//! Java reference: `defaults/DefaultBlockLootTableProvider.java` (169),
//! `DefaultEntityLootProvider.java` (179), `DefaultSupplyLootProvider.java` (69),
//! `DefaultLuckyOreLootProvider.java` (68), `workers/DefaultFishermanLootProvider.java`
//! (157), `workers/DefaultRecipeLootProvider.java` (57),
//! `workers/DefaultSifterCraftingProvider.java:52-238`,
//! `workers/DefaultNetherWorkerLootProvider.java:63-216`, plus the 30 lines of
//! `SimpleLootTableProvider` that turn a builder into a file (102).
//!
//! # The four things a table says, and where each is decided
//!
//! | Field | Decided by | Example |
//! | --- | --- | --- |
//! | `type` | the context the table was registered with | 83 `minecraft:block`, 18 with none (`ALL_PARAMS`) |
//! | `random_sequence` | the table's own id | `minecolonies:blocks/blockhutbaker` |
//! | a pool's `rolls` / `bonus_rolls` | the pool builder, `1.0` / `0.0` by default | the nether worker rolls `3..10` and one bonus roll of `0.3` |
//! | an entry's `weight` / `quality` | the entry builder, **omitted when 1 / 0** | `minecraft:sponge` has no `weight` at all |
//!
//! That last row is not cosmetic: `LootPoolSingletonContainer` writes `weight`
//! only when it is not 1 and `quality` only when it is not 0, so a generator that
//! wrote them unconditionally would deliver 146 tables that differ from these in
//! a way no reader would notice.
//!
//! # What a block drops, in the order Java decides it
//!
//! 1. **no table at all** — the decoration controller and the two graves: no
//!    provider registers one, so the break drops nothing and this module refuses
//!    them by name rather than inventing a rule;
//! 2. **composted dirt** — silk touch keeps the block, otherwise dirt
//!    (`DefaultBlockLootTableProvider.java:67-72`);
//! 3. **farmland and flooded farmland** — dirt, unconditionally (`:74-75`);
//! 4. **the 15 crops** — the block at `age == 6`, with the binomial fortune
//!    bonus `(0.5714286, 3)` (`:78-82`);
//! 5. **the 53 hut-like blocks** — the item, plus `copy_name` from the block
//!    entity (`:130-139`);
//! 6. **everything else** — the item, and `survives_explosion` on the pool.

use super::constants::{loot_path, Context};
use super::json::{arr, f, i, obj, own, s, vanilla, Json};
use super::{File, Refusal};
use crate::blocks::{Block, Family};

/// How many items an entry gives, and which functions say so.
///
/// `SetItemCountFunction` writes `{"add": false, "count": n}`; `ConstantValue`
/// serialises as a bare number and `UniformGenerator` as a `minecraft:uniform`
/// object, which is why the two spellings below are not interchangeable
/// (`data/minecolonies/loot_table/chests/supplycamp.json` has `"count": 4.0` and
/// `recipes/netherworker/trip1.json` has a uniform object).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// One: no function at all.
    One,
    /// `set_count` with a constant.
    Exactly(u32),
    /// `set_count` with `uniform(min, max)`, and nothing else.
    Uniform(u32, u32),
    /// `set_count` with `uniform(min, max)`, then `looting_enchant` with
    /// `uniform(1, 32)` — the pharao's two arrow entries are the only ones
    /// (`DefaultEntityLootProvider.java:133-138`).
    UniformLooting(u32, u32),
}

/// One entry of a pool, as a rule: the item, its weight, its quality and its
/// count. A weight of 1 and a quality of 0 are the defaults and are not written.
pub type Grant = (&'static str, u32, i32, Count);

/// A numbered channel of a table. `rolls` and `bonus_rolls` are number
/// *providers*: a constant writes a bare float (`1.0`, `0.0`, `0.025`), a range
/// writes a `minecraft:uniform` object — and a range whose two ends are equal is
/// still an object.
pub struct Pool {
    pub rolls: Json,
    pub bonus: Json,
    pub conditions: Vec<Json>,
    pub functions: Vec<Json>,
    pub entries: Vec<Json>,
}

impl Pool {
    pub fn new() -> Pool {
        Pool {
            rolls: f(1.0),
            bonus: f(0.0),
            conditions: Vec::new(),
            functions: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// `setRolls(UniformGenerator.between(min, max))`.
    pub fn uniform_rolls(mut self, min: f64, max: f64) -> Pool {
        self.rolls = uniform(min, max);
        self
    }

    /// `setBonusRolls(ConstantValue.exactly(n))`, which writes a bare number.
    pub fn bonus(mut self, bonus: f64) -> Pool {
        self.bonus = f(bonus);
        self
    }

    /// `setBonusRolls(UniformGenerator.between(a, b))`, which writes a **provider
    /// object** even when `a == b` — the nether worker's `0.3 .. 0.3`
    /// (`DefaultNetherWorkerLootProvider.java:75`) is not the same JSON as `0.3`.
    pub fn bonus_uniform(mut self, min: f64, max: f64) -> Pool {
        self.bonus = uniform(min, max);
        self
    }

    /// `when(condition)` on the **pool**: the whole channel is skipped, which is
    /// what `survives_explosion` does on the 83 block tables.
    pub fn when(mut self, condition: Json) -> Pool {
        self.conditions.push(condition);
        self
    }

    /// `apply(function)` on the **pool**: the banner tables copy their pattern
    /// this way (`DefaultBlockLootTableProvider.java:164`).
    pub fn apply(mut self, function: Json) -> Pool {
        self.functions.push(function);
        self
    }

    pub fn add(mut self, entry: Json) -> Pool {
        self.entries.push(entry);
        self
    }

    pub fn json(&self) -> Json {
        let mut pairs = vec![
            ("bonus_rolls", self.bonus.clone()),
            ("entries", arr(self.entries.clone())),
            ("rolls", self.rolls.clone()),
        ];
        if !self.conditions.is_empty() {
            pairs.push(("conditions", arr(self.conditions.clone())));
        }
        if !self.functions.is_empty() {
            pairs.push(("functions", arr(self.functions.clone())));
        }
        obj(pairs)
    }
}

impl Default for Pool {
    fn default() -> Self {
        Pool::new()
    }
}

/// A `minecraft:uniform` number provider.
pub fn uniform(min: f64, max: f64) -> Json {
    obj(vec![
        ("type", s("minecraft:uniform")),
        ("max", f(max)),
        ("min", f(min)),
    ])
}

/// A `minecraft:item` entry.
pub fn item(name: &str) -> Json {
    obj(vec![
        ("type", s("minecraft:item")),
        ("name", s(name)),
    ])
}

/// A `minecraft:empty` entry — `EmptyLootItem`, the "nothing" a weighted pool
/// needs to be able to give nothing.
pub fn empty() -> Json {
    obj(vec![("type", s("minecraft:empty"))])
}

/// The entry a rule's item name stands for: an item, or the empty slot when the
/// rule names it.
///
/// A weighted pool that can give nothing writes an `EmptyLootItem`
/// (`DefaultSifterCraftingProvider.java:55`, `DefaultEntityLootProvider.java:45`),
/// which serialises as `{"type": "minecraft:empty"}` — **no `name` at all**. A
/// rule that keeps its "nothing" as the string `minecraft:empty` and wraps it in
/// [`item`] would deliver `{"name": "minecraft:empty", "type": "minecraft:item"}`,
/// which is a *different* entry: Java never builds one, and the pack never
/// carries one (`data/minecolonies/loot_table/recipes/string/dirt.json` says
/// `{"type": "minecraft:empty", "weight": 71}`).
pub fn item_or_empty(name: &str) -> Json {
    if name == "minecraft:empty" {
        empty()
    } else {
        item(name)
    }
}

/// A `minecraft:loot_table` entry, which rolls another table.
pub fn table_ref(name: &str) -> Json {
    obj(vec![
        ("type", s("minecraft:loot_table")),
        ("name", s(name)),
    ])
}

/// `setWeight`, written only when it is not 1.
pub fn weight(mut entry: Json, value: u32) -> Json {
    if value != 1 {
        super::json::set(&mut entry, "weight", i(value as i64));
    }
    entry
}

/// `setQuality`, written only when it is not 0.
pub fn quality(mut entry: Json, value: i32) -> Json {
    if value != 0 {
        super::json::set(&mut entry, "quality", i(value as i64));
    }
    entry
}

/// `when(condition)` on an **entry**: this one entry is skipped, the pool is not.
pub fn when(mut entry: Json, condition: Json) -> Json {
    super::json::push(&mut entry, "conditions", condition);
    entry
}

/// `apply(function)` on an **entry**.
pub fn apply(mut entry: Json, function: Json) -> Json {
    super::json::push(&mut entry, "functions", function);
    entry
}

/// The functions a [`Count`] adds.
pub fn counted(mut entry: Json, count: Count) -> Json {
    let set_count = |provider: Json| {
        obj(vec![
            ("add", Json::Bool(false)),
            ("count", provider),
            ("function", s("minecraft:set_count")),
        ])
    };
    match count {
        Count::One => entry,
        Count::Exactly(n) => {
            entry = apply(entry, set_count(f(n as f64)));
            entry
        }
        Count::Uniform(min, max) => apply(entry, set_count(uniform(min as f64, max as f64))),
        Count::UniformLooting(min, max) => {
            entry = apply(entry, set_count(uniform(min as f64, max as f64)));
            apply(
                entry,
                obj(vec![
                    ("count", uniform(1.0, 32.0)),
                    ("function", s("minecraft:looting_enchant")),
                ]),
            )
        }
    }
}

/// A whole entry from a [`Grant`].
pub fn grant(row: &Grant) -> Json {
    counted(quality(weight(item_or_empty(row.0), row.1), row.2), row.3)
}

// ---------------------------------------------------------------------------
// Conditions and functions, each one a call the provider makes.
// ---------------------------------------------------------------------------

/// `ExplosionCondition.survivesExplosion()`.
pub fn survives_explosion() -> Json {
    obj(vec![("condition", s("minecraft:survives_explosion"))])
}

/// `ModLootConditions.HAS_SILK_TOUCH`.
pub fn silk_touch() -> Json {
    obj(vec![
        ("condition", s("minecraft:match_tool")),
        (
            "predicate",
            obj(vec![(
                "enchantments",
                arr(vec![obj(vec![
                    ("enchantment", s("minecraft:silk_touch")),
                    ("levels", obj(vec![("min", i(1))])),
                ])]),
            )]),
        ),
    ])
}

/// `LootItemBlockStatePropertyCondition…hasProperty(CropBlock.AGE, 6)`.
pub fn age_six(crop: &str) -> Json {
    obj(vec![
        ("block", s(&own(crop))),
        ("condition", s("minecraft:block_state_property")),
        ("properties", obj(vec![("age", s("6"))])),
    ])
}

/// `ApplyBonusCount.addBonusBinomialDistributionCount(BLOCK_FORTUNE, p, extra)`.
pub fn fortune_binomial(probability: f64, extra: u32) -> Json {
    obj(vec![
        ("enchantment", s("minecraft:fortune")),
        ("formula", s("minecraft:binomial_with_bonus_count")),
        (
            "function",
            s("minecraft:apply_bonus"),
        ),
        (
            "parameters",
            obj(vec![("extra", i(extra as i64)), ("probability", f(probability))]),
        ),
    ])
}

/// `LootItemRandomChanceCondition.randomChance(p)`.
pub fn random_chance(chance: f64) -> Json {
    obj(vec![
        ("chance", f(chance)),
        ("condition", s("minecraft:random_chance")),
    ])
}

/// `CopyNameFunction.copyName(BLOCK_ENTITY)`.
pub fn copy_name() -> Json {
    obj(vec![
        ("function", s("minecraft:copy_name")),
        ("source", s("block_entity")),
    ])
}

/// `SetNbtFunction.setTag(tag)`.
pub fn set_nbt(tag: &str) -> Json {
    obj(vec![
        ("function", s("minecraft:set_nbt")),
        ("tag", s(tag)),
    ])
}

/// `SetNameFunction.setName(Component.translatable(key, with))`.
pub fn set_name(key: &str, with: &str) -> Json {
    obj(vec![
        ("function", s("minecraft:set_name")),
        (
            "name",
            obj(vec![
                ("translate", s(key)),
                ("with", arr(vec![obj(vec![("translate", s(with))])])),
            ]),
        ),
    ])
}

/// `AlternativesEntry.alternatives()…otherwise(…)`.
pub fn alternatives(children: Vec<Json>) -> Json {
    obj(vec![
        ("type", s("minecraft:alternatives")),
        ("children", arr(children)),
    ])
}

// ---------------------------------------------------------------------------
// The tables.
// ---------------------------------------------------------------------------

/// A table, from its id, the context it is registered with, and its pools.
pub fn table(id: &str, context: Context, pools: Vec<Json>) -> File {
    let mut pairs = vec![("random_sequence", s(id))];
    if let Some(kind) = context.type_field() {
        pairs.push(("type", s(kind)));
    }
    if !pools.is_empty() {
        pairs.push(("pools", arr(pools)));
    }
    File {
        path: loot_path(id),
        json: obj(pairs),
    }
}

/// The blocks Java registers no table for, and why.
///
/// Two are graves and one is the decoration controller. Nothing in
/// `DefaultBlockLootTableProvider.registerTables` (`:50-120`) names them; a
/// grave holds a player's inventory and a controller is a marker, so dropping
/// themselves would be the wrong answer to a break, and **no answer** is the
/// answer the pack carries.
const NO_TABLE: [(&str, &str); 3] = [
    (
        "blockminecoloniesgrave",
        "a grave holds the inventory a citizen carried, and the provider registers no table for it",
    ),
    (
        "blockminecoloniesnamedgrave",
        "the named grave is the same block with a name, and no table either",
    ),
    (
        "decorationcontroller",
        "the controller is a placement marker, never harvested",
    ),
];

/// The one block whose **item** is another block's, because it has none of its
/// own: a wall banner drops the standing banner, exactly as vanilla's
/// `WallBannerBlock` does (`core/blocks/decorative/BlockColonyFlagWallBanner.java:32`).
/// Measured: `data/minecolonies/loot_table/blocks/colony_wall_banner.json` names
/// `minecolonies:colony_banner`.
const ITEM_OF: [(&str, &str); 1] = [("colony_wall_banner", "colony_banner")];

/// The item id a block leaves behind: its own name, unless [`ITEM_OF`] says
/// otherwise.
pub fn block_item(name: &str) -> String {
    for (block, item) in ITEM_OF {
        if block == name {
            return own(item);
        }
    }
    own(name)
}

/// One block's table, or the refusal that names why it has none.
pub fn block_table(block: &Block) -> Result<File, Refusal> {
    let name = super::json::path_of(block.name);
    let id = own(&format!("blocks/{name}"));
    let piece = item(&block_item(name));

    let mut refused: Option<&'static str> = None;
    for (block_name, why) in NO_TABLE {
        if block_name == name {
            refused = Some(why);
        }
    }
    if let Some(why) = refused {
        return Err(Refusal::NoLootTable {
            block: block.name,
            why,
            java: "defaults/DefaultBlockLootTableProvider.java:50-120",
        });
    }

    // `ModBlocks.blockCompostedDirt` — silk touch or dirt (`:67-72`).
    if name == "composted_dirt" {
        let pool = Pool::new().add(alternatives(vec![
            when(piece, silk_touch()),
            when(item(&vanilla("dirt")), survives_explosion()),
        ]));
        return Ok(table(&id, Context::Block, vec![pool.json()]));
    }

    // `ModBlocks.farmland` and `ModBlocks.floodedFarmland` — dirt (`:74-75`).
    if name == "farmland" || name == "floodedfarmland" {
        let pool = Pool::new().add(alternatives(vec![item(&vanilla("dirt"))]));
        return Ok(table(&id, Context::Block, vec![pool.json()]));
    }

    // The 15 crops (`:78-82`): the seed when ripe, with the binomial fortune
    // bonus, and the seed anyway when not.
    if block.sown_on.is_some() {
        let pool = Pool::new().add(alternatives(vec![
            apply(when(piece.clone(), age_six(name)), fortune_binomial(0.5714286, 3)),
            piece,
        ]));
        return Ok(table(&id, Context::Block, vec![pool.json()]));
    }

    // `saveBlock` (`:130-139`): a hut or the rack copies its custom name, and
    // every plain block survives an explosion instead.
    let pool = Pool::new()
        .add(if copies_name(block) {
            apply(piece, copy_name())
        } else {
            piece
        })
        .when(survives_explosion());
    Ok(table(&id, Context::Block, vec![pool.json()]))
}

/// Whether `saveBlock` puts `CopyNameFunction` on this block
/// (`DefaultBlockLootTableProvider.java:133`): Java tests
/// `block instanceof AbstractBlockHut || block instanceof BlockMinecoloniesRack`.
///
/// The two `Family::Huts` rows that are **not** `AbstractBlockHut` are the post
/// box and the stash (`BlockPostBox`, `BlockStash` — both are
/// `AbstractColonyBlock`s), and `crate::blocks` groups them with the huts on
/// purpose; this is the one rule that has to tell them apart.
pub fn copies_name(block: &Block) -> bool {
    if block.name == "blockminecoloniesrack" {
        return true;
    }
    block.family == Family::Huts && block.name != "blockpostbox" && block.name != "blockstash"
}

/// The two supply crates (`DefaultSupplyLootProvider.java:40-68`).
///
/// Both pools say the same thing: a 1 % deployer that places itself instantly and
/// carries the "free" name, and four buff scrolls at 10 %.
pub fn chest_tables() -> Vec<File> {
    let instant = "{Placement:\"instant\"}";
    [(
        "chests/supplycamp",
        "supplycampdeployer",
        "item.minecolonies.supplycampdeployer",
    ), (
        "chests/supplyship",
        "supplychestdeployer",
        "item.minecolonies.supplychestdeployer",
    )]
    .into_iter()
    .map(|(path, deployer, description)| {
        let pool = Pool::new()
            .add(
                apply(
                    apply(
                        when(item(&own(deployer)), random_chance(0.01)),
                        set_nbt(instant),
                    ),
                    set_name("item.minecolonies.supply.free", description),
                ),
            )
            .add(counted(
                when(item(&own("scroll_buff")), random_chance(0.1)),
                Count::Exactly(4),
            ));
        table(&own(path), Context::Chest, vec![pool.json()])
    })
    .collect()
}

/// The miner's five lucky-ore tables (`DefaultLuckyOreLootProvider.java:33-67`).
///
/// Level `n` adds rarer ore to level `n - 1`, and **level 5 is level 4**
/// (`:66` registers the same builder twice) — not a bigger table, the very same
/// one.
pub fn lucky_ore_tables() -> Vec<File> {
    const ORE: [(&str, u32); 8] = [
        ("minecraft:coal_ore", 64),
        ("minecraft:copper_ore", 48),
        ("minecraft:iron_ore", 32),
        ("minecraft:gold_ore", 16),
        ("minecraft:redstone_ore", 8),
        ("minecraft:lapis_ore", 4),
        ("minecraft:diamond_ore", 2),
        ("minecraft:emerald_ore", 1),
    ];
    const SIZES: [usize; 5] = [2, 4, 6, 8, 8];
    SIZES
        .into_iter()
        .enumerate()
        .map(|(index, size)| {
            let mut pool = Pool::new();
            for (name, chance) in ORE.iter().take(size) {
                pool = pool.add(weight(item(name), *chance));
            }
            table(
                &own(&format!("miner/lucky_ore{}", index + 1)),
                Context::All,
                vec![pool.json()],
            )
        })
        .collect()
}

/// The nine fishing tables (`DefaultFishermanLootProvider.java:73-135`).
pub fn fisherman_tables() -> Vec<File> {
    let any_of = obj(vec![
        ("condition", s("minecraft:any_of")),
        (
            "terms",
            arr(vec![
                obj(vec![
                    ("condition", s("minecolonies:entity_in_biome_tag")),
                    ("tag", s("minecraft:is_ocean")),
                ]),
                obj(vec![
                    ("condition", s("minecolonies:research_unlocked")),
                    ("id", s("minecolonies:effects/fishingtreasure")),
                ]),
            ]),
        ),
    ]);

    let mut files = vec![table(
        &own("fisherman"),
        Context::Fishing,
        vec![Pool::new()
            .add(quality(weight(
                table_ref("minecolonies:fisherman/junk"),
                10,
            ), -2))
            .add(when(
                quality(weight(table_ref("minecolonies:fisherman/treasure"), 5), 2),
                any_of,
            ))
            .add(quality(
                weight(table_ref("minecolonies:fisherman/fish"), 85),
                -1,
            ))
            .json()],
    )];

    // The three channels are one reference each: the mod does not restate
    // vanilla's fishing loot, it rolls it (`:86-99`).
    for (name, vanilla_table) in [
        ("junk", "minecraft:gameplay/fishing/junk"),
        ("treasure", "minecraft:gameplay/fishing/treasure"),
        ("fish", "minecraft:gameplay/fishing/fish"),
    ] {
        files.push(table(
            &own(&format!("fisherman/{name}")),
            Context::Fishing,
            vec![Pool::new().add(table_ref(vanilla_table)).json()],
        ));
    }

    // The bonus table: `makeLoot(1000, …)` fills the rest of the thousand with
    // an empty entry, so the total weight is always 1000 and the odds in the
    // comment above it (`:102-135`) hold.
    let bonus = |level: u32, drops: &[(&str, u32)]| {
        let spent: u32 = drops.iter().map(|(_, chance)| *chance).sum();
        let mut pool = Pool::new();
        for (name, chance) in drops {
            pool = pool.add(quality(weight(item(name), *chance), 1));
        }
        pool = pool.add(weight(empty(), 1000 - spent));
        let context = if level <= 2 {
            Context::Empty
        } else {
            Context::All
        };
        let table_value = if level <= 2 {
            Vec::new()
        } else {
            vec![pool.json()]
        };
        table(&own(&format!("fisherman/bonus{level}")), context, table_value)
    };
    // Levels 1 and 2 drop nothing at all: `noBonus` is a table with no pool.
    files.push(bonus(1, &[]));
    files.push(bonus(2, &[]));
    files.push(bonus(
        3,
        &[
            ("minecraft:prismarine_shard", 25),
            ("minecraft:prismarine_crystals", 25),
        ],
    ));
    files.push(bonus(
        4,
        &[
            ("minecraft:sponge", 1),
            ("minecraft:prismarine_shard", 25),
            ("minecraft:prismarine_crystals", 25),
        ],
    ));
    files.push(bonus(
        5,
        &[
            ("minecraft:sponge", 1),
            ("minecraft:prismarine_shard", 25),
            ("minecraft:prismarine_crystals", 25),
        ],
    ));
    files
}

/// One raider, as the table of `DefaultEntityLootProvider.java:44-162` states it:
/// the loot-table name, the number of rolls, and the entries in source order.
type Mob = (&'static str, u32, &'static [Grant]);

/// The 18 raiders. Weights are Java's, and a weight of 1 really is an unweighted
/// entry (`diamond_axe` at `:61`).
const MOBS: [Mob; 18] = [
    ("amazon", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 15, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("amazonspearman", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecolonies:spear", 15, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("amazonchief", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 80, 0, Count::One),
        ("minecolonies:ancienttome", 30, 0, Count::One),
    ]),
    ("barbarian", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:diamond_axe", 1, 0, Count::One),
        ("minecraft:golden_axe", 2, 0, Count::One),
        ("minecraft:iron_axe", 5, 0, Count::One),
        ("minecraft:stone_axe", 6, 0, Count::One),
        ("minecolonies:ancienttome", 3, 0, Count::One),
    ]),
    ("archerbarbarian", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("chiefbarbarian", 1, &[
        ("minecraft:empty", 50, 0, Count::One),
        ("minecolonies:chiefsword", 1, 1, Count::One),
        ("minecraft:diamond_sword", 5, 0, Count::One),
        ("minecraft:golden_sword", 5, 0, Count::One),
        ("minecraft:iron_sword", 10, 0, Count::One),
        ("minecraft:stone_sword", 20, 0, Count::One),
        ("minecolonies:ancienttome", 30, 0, Count::One),
    ]),
    ("shieldmaiden", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:shield", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("norsemenarcher", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    // The only two-roll table in the family (`:92`).
    ("norsemenchief", 2, &[
        ("minecraft:empty", 50, 0, Count::One),
        ("minecraft:leather", 15, 5, Count::One),
        ("minecraft:diamond_axe", 10, 1, Count::One),
        ("minecolonies:ancienttome", 50, 0, Count::One),
    ]),
    ("pirate", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecolonies:iron_scimitar", 6, 0, Count::One),
        ("minecolonies:ancienttome", 4, 0, Count::One),
    ]),
    ("archerpirate", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("chiefpirate", 1, &[
        ("minecraft:empty", 50, 0, Count::One),
        ("minecolonies:pirate_hat", 5, 1, Count::One),
        ("minecolonies:pirate_leggins", 5, 1, Count::One),
        ("minecolonies:pirate_boots", 5, 1, Count::One),
        ("minecolonies:pirate_top", 5, 1, Count::One),
        ("minecolonies:pirate_cap", 5, 1, Count::One),
        ("minecolonies:pirate_legs", 5, 1, Count::One),
        ("minecolonies:pirate_shoes", 5, 1, Count::One),
        ("minecolonies:pirate_chest", 5, 1, Count::One),
        ("minecolonies:iron_scimitar", 25, 1, Count::One),
        ("minecolonies:ancienttome", 30, 0, Count::One),
    ]),
    ("mummy", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecolonies:ancienttome", 15, 0, Count::One),
    ]),
    ("archermummy", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("pharao", 1, &[
        ("minecraft:empty", 50, 0, Count::One),
        ("minecolonies:pharaoscepter", 3, 1, Count::One),
        ("minecraft:arrow", 20, 0, Count::UniformLooting(1, 16)),
        ("minecolonies:firearrow", 10, 0, Count::UniformLooting(1, 16)),
        ("minecolonies:ancienttome", 30, 0, Count::One),
    ]),
    ("drownedpirate", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecolonies:iron_scimitar", 6, 0, Count::One),
        ("minecolonies:ancienttome", 4, 0, Count::One),
    ]),
    ("drownedarcherpirate", 1, &[
        ("minecraft:empty", 80, 0, Count::One),
        ("minecraft:bow", 10, 0, Count::One),
        ("minecolonies:ancienttome", 5, 0, Count::One),
    ]),
    ("drownedchiefpirate", 1, &[
        ("minecraft:empty", 50, 0, Count::One),
        ("minecolonies:pirate_hat", 5, 1, Count::One),
        ("minecolonies:pirate_leggins", 5, 1, Count::One),
        ("minecolonies:pirate_boots", 5, 1, Count::One),
        ("minecolonies:pirate_top", 5, 1, Count::One),
        ("minecolonies:pirate_cap", 5, 1, Count::One),
        ("minecolonies:pirate_legs", 5, 1, Count::One),
        ("minecolonies:pirate_shoes", 5, 1, Count::One),
        ("minecolonies:pirate_chest", 5, 1, Count::One),
        ("minecolonies:iron_scimitar", 25, 1, Count::One),
        ("minecolonies:ancienttome", 30, 0, Count::One),
    ]),
];

/// The 18 raider tables. `registerLoot` (`:165-178`) puts every one of them in
/// `ALL_PARAMS`, so none carries a `type`.
pub fn entity_tables() -> Vec<File> {
    MOBS
        .iter()
        .map(|(name, rolls, grants)| {
            let mut pool = Pool::new();
            for row in *grants {
                pool = pool.add(grant(row));
            }
            if *rolls != 1 {
                pool.rolls = f(*rolls as f64);
            }
            table(&own(&format!("entities/{name}")), Context::All, vec![pool.json()])
        })
        .collect()
}

/// The three generic recipe loot tables (`DefaultRecipeLootProvider.java:23-56`).
///
/// The glass bottle and the large bottle are a coin flip between nothing and the
/// bottle, expressed as a quality of −1 / +1 so that a luck modifier can weigh in;
/// gravel is a flat 90/10 for flint.
pub fn recipe_loot_tables() -> Vec<File> {
    let bottle = |name: &str, item_name: &str| {
        table(
            &own(&format!("recipes/{name}")),
            Context::All,
            vec![Pool::new()
                .add(quality(weight(empty(), 100), -1))
                .add(quality(weight(item(item_name), 100), 1))
                .json()],
        )
    };
    vec![
        bottle("glass_bottle", "minecraft:glass_bottle"),
        bottle("large_bottle", "minecolonies:large_empty_bottle"),
        table(
            &own("recipes/gravel"),
            Context::All,
            vec![Pool::new()
                .add(weight(empty(), 90))
                .add(weight(item("minecraft:flint"), 10))
                .json()],
        ),
    ]
}

/// A sifter table: the mesh, the block sifted, the minimum building level, the
/// bonus rolls, and the entries.
pub type Sift = (&'static str, &'static str, u32, f64, &'static [(&'static str, u32)]);

/// The 16 sifter tables (`DefaultSifterCraftingProvider.java:52-238`).
pub const SIFTER: [Sift; 16] = [
    ("string", "dirt", 1, 0.0, &[
        ("minecraft:empty", 71),
        ("minecraft:wheat_seeds", 25),
        ("minecraft:oak_sapling", 1),
        ("minecraft:birch_sapling", 1),
        ("minecraft:spruce_sapling", 1),
        ("minecraft:jungle_sapling", 1),
    ]),
    ("flint", "dirt", 3, 0.0, &[
        ("minecraft:empty", 28),
        ("minecraft:wheat_seeds", 50),
        ("minecraft:oak_sapling", 5),
        ("minecraft:birch_sapling", 5),
        ("minecraft:spruce_sapling", 5),
        ("minecraft:jungle_sapling", 5),
        ("minecraft:carrot", 1),
        ("minecraft:potato", 1),
    ]),
    ("iron", "dirt", 4, 0.025, &[
        ("minecraft:empty", 3),
        ("minecraft:wheat_seeds", 50),
        ("minecraft:oak_sapling", 10),
        ("minecraft:birch_sapling", 10),
        ("minecraft:spruce_sapling", 10),
        ("minecraft:jungle_sapling", 10),
        ("minecraft:carrot", 1),
        ("minecraft:potato", 1),
        ("minecraft:pumpkin_seeds", 1),
        ("minecraft:melon_seeds", 1),
        ("minecraft:beetroot_seeds", 1),
        ("minecraft:dark_oak_sapling", 1),
        ("minecraft:acacia_sapling", 1),
    ]),
    ("diamond", "dirt", 5, 0.035, &[
        ("minecraft:empty", 5),
        ("minecraft:wheat_seeds", 25),
        ("minecraft:oak_sapling", 10),
        ("minecraft:birch_sapling", 10),
        ("minecraft:spruce_sapling", 10),
        ("minecraft:jungle_sapling", 10),
        ("minecraft:carrot", 5),
        ("minecraft:potato", 5),
        ("minecraft:pumpkin_seeds", 5),
        ("minecraft:melon_seeds", 5),
        ("minecraft:beetroot_seeds", 5),
        ("minecraft:dark_oak_sapling", 5),
        ("minecraft:acacia_sapling", 5),
    ]),
    ("string", "gravel", 1, 0.0, &[
        ("minecraft:empty", 85),
        ("minecraft:iron_nugget", 5),
        ("minecraft:flint", 5),
        ("minecraft:coal", 5),
    ]),
    ("flint", "gravel", 3, 0.0, &[
        ("minecraft:empty", 60),
        ("minecraft:iron_nugget", 10),
        ("minecraft:flint", 10),
        ("minecraft:coal", 10),
        ("minecraft:redstone", 10),
    ]),
    ("iron", "gravel", 4, 0.025, &[
        ("minecraft:empty", 46),
        ("minecraft:redstone", 15),
        ("minecraft:iron_nugget", 15),
        ("minecraft:coal", 15),
        ("minecraft:lapis_lazuli", 5),
        ("minecraft:iron_ingot", 1),
        ("minecraft:gold_ingot", 1),
        ("minecraft:emerald", 1),
        ("minecraft:diamond", 1),
    ]),
    ("diamond", "gravel", 5, 0.035, &[
        ("minecraft:empty", 40),
        ("minecraft:redstone", 20),
        ("minecraft:iron_nugget", 20),
        ("minecraft:coal", 20),
        ("minecraft:lapis_lazuli", 10),
        ("minecraft:iron_ingot", 2),
        ("minecraft:gold_ingot", 2),
        ("minecraft:emerald", 2),
        ("minecraft:diamond", 2),
    ]),
    ("string", "sand", 1, 0.0, &[
        ("minecraft:empty", 95),
        ("minecraft:cactus", 2),
        ("minecraft:sugar_cane", 2),
    ]),
    ("flint", "sand", 3, 0.0, &[
        ("minecraft:empty", 85),
        ("minecraft:cactus", 5),
        ("minecraft:sugar_cane", 5),
        ("minecraft:gold_nugget", 5),
    ]),
    ("iron", "sand", 4, 0.025, &[
        ("minecraft:empty", 60),
        ("minecraft:cactus", 10),
        ("minecraft:sugar_cane", 10),
        ("minecraft:gold_nugget", 10),
        ("minecraft:cocoa_beans", 10),
    ]),
    ("diamond", "sand", 5, 0.035, &[
        ("minecraft:empty", 40),
        ("minecraft:cactus", 15),
        ("minecraft:sugar_cane", 15),
        ("minecraft:gold_nugget", 15),
        ("minecraft:cocoa_beans", 15),
    ]),
    ("string", "soul_sand", 1, 0.0, &[
        ("minecraft:empty", 90),
        ("minecraft:nether_wart", 5),
        ("minecraft:quartz", 5),
    ]),
    ("flint", "soul_sand", 3, 0.0, &[
        ("minecraft:empty", 70),
        ("minecraft:nether_wart", 10),
        ("minecraft:quartz", 10),
        ("minecraft:glowstone_dust", 10),
    ]),
    ("iron", "soul_sand", 4, 0.025, &[
        ("minecraft:empty", 50),
        ("minecraft:nether_wart", 10),
        ("minecraft:quartz", 10),
        ("minecraft:glowstone_dust", 10),
        ("minecraft:blaze_powder", 1),
        ("minecraft:magma_cream", 1),
    ]),
    ("diamond", "soul_sand", 5, 0.035, &[
        ("minecraft:empty", 40),
        ("minecraft:nether_wart", 15),
        ("minecraft:quartz", 15),
        ("minecraft:glowstone_dust", 15),
        ("minecraft:blaze_powder", 5),
        ("minecraft:magma_cream", 5),
        ("minecraft:player_head", 5),
    ]),
];

/// The 16 sifter tables, under `recipes/<mesh>/<block>` (`:326-337`).
pub fn sifter_tables() -> Vec<File> {
    SIFTER
        .iter()
        .map(|(mesh, input, _, bonus, drops)| {
            let mut pool = Pool::new();
            if *bonus != 0.0 {
                pool = pool.bonus(*bonus);
            }
            for (name, weight_of) in *drops {
                pool = pool.add(weight(item_or_empty(name), *weight_of));
            }
            table(
                &own(&format!("recipes/{mesh}/{input}")),
                Context::All,
                vec![pool.json()],
            )
        })
        .collect()
}

/// One adventure token: the mob it stands for, and the damage and experience it
/// pays.
type Token = (&'static str, u32, i32, u32, u32);

/// The mobs a nether trip can meet (`DefaultNetherWorkerLootProvider.java:184-200`).
const TOKENS: [Token; 6] = [
    ("minecraft:zombified_piglin", 5500, -10, 5, 5),
    ("minecraft:magma_cube", 300, 10, 3, 4),
    ("minecraft:hoglin", 500, -1, 3, 5),
    ("minecraft:ghast", 300, -3, 12, 5),
    ("minecraft:enderman", 300, -3, 7, 5),
    ("minecraft:blaze", 100, 1, 5, 10),
];

/// The blocks a nether trip can bring back, with the level that unlocks each and
/// the range of the count (`:71-175`), **in source order**: the level-4 pair sits
/// before `ancient_debris` because that is where Java adds them, and a pool's
/// entry order is part of what it rolls.
pub const NETHER_BLOCKS: [(&str, u32, u32, Option<(u32, u32)>); 22] = [
    ("minecraft:netherrack", 20, 1, Some((5, 25))),
    ("minecraft:soul_sand", 10, 1, Some((1, 7))),
    ("minecraft:soul_soil", 8, 1, Some((1, 5))),
    ("minecraft:gravel", 10, 1, Some((1, 10))),
    ("minecraft:nether_quartz_ore", 15, 1, Some((1, 4))),
    ("minecraft:glowstone", 5, 2, Some((2, 4))),
    ("minecraft:nether_wart", 3, 2, Some((1, 2))),
    ("minecraft:brown_mushroom", 10, 2, Some((1, 2))),
    ("minecraft:red_mushroom", 10, 2, Some((1, 2))),
    ("minecraft:crimson_nylium", 5, 2, Some((1, 1))),
    ("minecraft:crimson_fungus", 10, 2, Some((0, 1))),
    ("minecraft:crimson_stem", 5, 2, Some((1, 3))),
    ("minecraft:basalt", 5, 3, Some((1, 3))),
    ("minecraft:warped_nylium", 5, 3, Some((1, 1))),
    ("minecraft:warped_fungus", 10, 3, Some((0, 1))),
    ("minecraft:warped_stem", 5, 3, Some((1, 3))),
    ("minecraft:ochre_froglight", 2, 3, None),
    ("minecraft:pearlescent_froglight", 1, 3, None),
    ("minecraft:verdant_froglight", 1, 3, None),
    ("minecraft:nether_gold_ore", 5, 4, Some((1, 2))),
    ("minecraft:blackstone", 5, 4, Some((1, 3))),
    ("minecraft:ancient_debris", 1, 5, Some((1, 2))),
];

/// The five nether trip tables (`:63-68`, `:260-270`).
///
/// Two pools, and the levels are cumulative: level 5 rolls everything levels 1
/// to 4 also roll. The block list is filtered by the level each row needs, in
/// source order — so a level-3 trip is the first twelve rows, and level 5 is all
/// twenty-two.
pub fn nether_worker_tables() -> Vec<File> {
    (1..=5)
        .map(|level| {
            let mut blocks = Pool::new()
                .uniform_rolls(3.0, 10.0)
                .bonus_uniform(0.3, 0.3);
            for (name, weight_of, min_level, count) in NETHER_BLOCKS {
                if min_level > level {
                    continue;
                }
                let entry = counted(
                    weight(item(name), weight_of),
                    match count {
                        Some((min, max)) => Count::Uniform(min, max),
                        None => Count::One,
                    },
                );
                blocks = blocks.add(entry);
            }

            let mut mobs = Pool::new().uniform_rolls(2.0, 6.0).bonus_uniform(0.1, 0.1);
            for (mob, weight_of, quality_of, damage, xp) in TOKENS {
                let tag = format!(
                    "{{damage-done:{damage},entity-type:\"{mob}\",xp-dropped:{xp}}}"
                );
                mobs = mobs.add(quality(
                    weight(apply(item("minecolonies:adventure_token"), set_nbt(&tag)), weight_of),
                    quality_of,
                ));
            }

            table(
                &own(&format!("recipes/netherworker/trip{level}")),
                Context::All,
                vec![blocks.json(), mobs.json()],
            )
        })
        .collect()
}

/// The five enchanter tables, refused rather than guessed.
pub fn enchanter_refusal() -> Refusal {
    Refusal::HandWritten {
        family: "enchanter loot",
        files: 5,
        java: "workers/DefaultEnchanterCraftingProvider.java:51-435",
    }
}

/// The 32 banner tables (`DefaultBlockLootTableProvider.java:87-119`).
///
/// A banner keeps its name **and its pattern**: two functions on the pool, and
/// the `copy_nbt` op list is `Patterns` and `id`, both into `BlockEntityTag`.
/// They are written into `data/minecraft/` because the block is vanilla's, and
/// they are the only files this generator produces outside its own namespace.
///
/// **A wall banner's table names the standing banner.** Vanilla gives a wall
/// banner no item of its own, so `LootItem.lootTableItem(block)` (`:162`) writes
/// `block.asItem()`, which is the standing banner: measured on the delivered
/// datagen, `minecraft/loot_tables/blocks/black_wall_banner.json` names
/// `minecraft:black_banner`, and the 16 wall files all do. The table *id* keeps
/// the wall suffix — only the item is the standing one.
pub fn vanilla_banner_tables() -> Vec<File> {
    const COLORS: [&str; 16] = [
        "black", "blue", "brown", "white", "cyan", "gray", "green", "light_blue",
        "light_gray", "lime", "magenta", "orange", "pink", "purple", "red", "yellow",
    ];
    let mut files = Vec::new();
    for colour in COLORS {
        for shape in ["banner", "wall_banner"] {
            let block = format!("{colour}_{shape}");
            let pool = Pool::new()
                // `block.asItem()`: the standing banner, for both shapes.
                .add(item(&vanilla(&format!("{colour}_banner"))))
                .apply(copy_name())
                .apply(obj(vec![
                    ("function", s("minecraft:copy_nbt")),
                    (
                        "ops",
                        arr(["Patterns", "id"]
                            .into_iter()
                            .map(|key| {
                                obj(vec![
                                    ("op", s("replace")),
                                    ("source", s(key)),
                                    ("target", s(&format!("BlockEntityTag.{key}"))),
                                ])
                            })
                            .collect()),
                    ),
                    ("source", s("block_entity")),
                ]))
                .when(survives_explosion());
            files.push(table(
                &format!("minecraft:blocks/{block}"),
                Context::Block,
                vec![pool.json()],
            ));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::json::{f, i, s};
    use crate::generation::{delivered, matches_delivered, Pack};

    fn pack() -> Pack {
        Pack::build()
    }

    #[test]
    fn a_hut_table_keeps_its_name_and_survives_an_explosion() {
        // `saveBlock`: `copy_name` on the entry, `survives_explosion` on the
        // pool. The pack agrees on all 53 (see the next test), this one names one.
        matches_delivered(
            &pack(),
            "minecolonies/loot_table/blocks/blockhutbaker.json",
        );
    }

    #[test]
    fn all_83_block_tables_are_the_83_the_pack_delivers() {
        let pack = pack();
        let mut checked = 0;
        for block in crate::blocks::all() {
            let name = super::super::json::path_of(block.name).to_owned();
            let path = format!("minecolonies/loot_table/blocks/{name}.json");
            if pack.get(&path).is_some() {
                matches_delivered(&pack, &path);
                checked += 1;
            }
        }
        assert_eq!(checked, 83);
    }

    #[test]
    fn exactly_53_blocks_copy_their_name() {
        let count = crate::blocks::all().filter(|block| copies_name(block)).count();
        assert_eq!(count, 53, "50 huts, 2 quarries and the rack");
        let named = |name: &str| copies_name(crate::blocks::block(name).expect(name));
        assert!(!named("blockpostbox"));
        assert!(!named("blockstash"));
        assert!(named("blockminecoloniesrack"));
        assert!(named("simplequarry"));
    }

    #[test]
    fn a_crop_drops_at_age_six_with_the_binomial_fortune_bonus() {
        // The one formula with a magic number in it (`:81`): p = 0.5714286, 3
        // extra drops. A crop table is the only place it appears.
        matches_delivered(
            &pack(),
            "minecolonies/loot_table/blocks/bell_pepper.json",
        );
        let table = delivered("minecolonies/loot_table/blocks/bell_pepper.json");
        let bonus = &table["pools"][0]["entries"][0]["children"][0]["functions"][0];
        assert_eq!(bonus["parameters"]["probability"], f(0.5714286));
        assert_eq!(bonus["parameters"]["extra"], i(3));
    }

    #[test]
    fn the_three_odd_blocks_are_the_three_odd_tables() {
        let pack = pack();
        for path in [
            "minecolonies/loot_table/blocks/composted_dirt.json",
            "minecolonies/loot_table/blocks/farmland.json",
            "minecolonies/loot_table/blocks/floodedfarmland.json",
        ] {
            matches_delivered(&pack, path);
        }
    }

    #[test]
    fn a_grave_and_the_controller_are_refused_by_name() {
        for name in ["blockminecoloniesgrave", "blockminecoloniesnamedgrave", "decorationcontroller"] {
            let block = crate::blocks::block(name)
                .unwrap_or_else(|| panic!("{name} is not a block of this port"));
            let refusal = block_table(block).expect_err("no loot table");
            assert!(
                refusal.sentence().contains(name),
                "the refusal names the block: {}",
                refusal.sentence()
            );
        }
    }

    #[test]
    fn the_two_supply_crates_are_identical_but_for_their_deployer() {
        let pack = pack();
        matches_delivered(&pack, "minecolonies/loot_table/chests/supplycamp.json");
        matches_delivered(&pack, "minecolonies/loot_table/chests/supplyship.json");
        let camp = pack.get("minecolonies/loot_table/chests/supplycamp.json").unwrap();
        assert_eq!(
            camp["pools"][0]["entries"][1]["functions"][0]["count"],
            f(4.0),
            "four scrolls, and the count is a float"
        );
        assert_eq!(camp["type"], s("minecraft:chest"));
    }

    #[test]
    fn level_five_of_the_lucky_ore_is_level_four() {
        // `:66` registers `luckyOres4` a second time; a generator that "fixed"
        // this would change the miner's odds at level 5.
        let pack = pack();
        for level in 1..=5 {
            matches_delivered(
                &pack,
                &format!("minecolonies/loot_table/miner/lucky_ore{level}.json"),
            );
        }
        let four = pack.get("minecolonies/loot_table/miner/lucky_ore4.json").unwrap();
        let five = pack.get("minecolonies/loot_table/miner/lucky_ore5.json").unwrap();
        let entries = |table: &Json| table["pools"][0]["entries"].clone();
        assert_eq!(entries(four), entries(five));
    }

    #[test]
    fn a_fisher_level_that_drops_nothing_has_no_pool_at_all() {
        let pack = pack();
        matches_delivered(&pack, "minecolonies/loot_table/fisherman/bonus1.json");
        let empty_table = pack.get("minecolonies/loot_table/fisherman/bonus1.json").unwrap();
        assert!(empty_table.get("pools").is_none(), "no pools key");
        assert_eq!(empty_table["type"], s("minecraft:empty"));
    }

    #[test]
    fn the_bonus_pool_always_weighs_a_thousand() {
        // `makeLoot(1000, …)` subtracts what it added; the empty entry makes up
        // the rest, which is what the odds in the comment are computed from.
        let pack = pack();
        for level in 3..=5 {
            let path = format!("minecolonies/loot_table/fisherman/bonus{level}.json");
            matches_delivered(&pack, &path);
            let table = pack.get(&path).unwrap();
            let total: u64 = table["pools"][0]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["weight"].as_u64().unwrap_or(1))
                .sum();
            assert_eq!(total, 1000, "{path}");
        }
    }

    #[test]
    fn the_fishing_root_references_the_three_channels_with_the_quality_java_gives() {
        let pack = pack();
        matches_delivered(&pack, "minecolonies/loot_table/fisherman.json");
        let root = pack.get("minecolonies/loot_table/fisherman.json").unwrap();
        let entries = root["pools"][0]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["quality"], i(-2));
        assert_eq!(entries[1]["quality"], i(2), "treasure needs an ocean or a research");
        assert_eq!(entries[2]["quality"], i(-1));
    }

    #[test]
    fn the_18_raider_tables_are_the_18_the_pack_delivers() {
        let pack = pack();
        let mut checked = 0;
        for (mob, _, _) in MOBS {
            matches_delivered(&pack, &format!("minecolonies/loot_table/entities/{mob}.json"));
            checked += 1;
        }
        assert_eq!(checked, 18);
        assert_eq!(pack.under("minecolonies/loot_table/entities/").len(), 18);
    }

    #[test]
    fn an_unweighted_entry_writes_no_weight_at_all() {
        // `setWeight(1)` is the default and is not serialised; the barbarian's
        // diamond axe is the proof.
        let pack = pack();
        let barbarian = pack
            .get("minecolonies/loot_table/entities/barbarian.json")
            .unwrap();
        let axe = &barbarian["pools"][0]["entries"][1];
        assert_eq!(axe["name"], s("minecraft:diamond_axe"));
        assert!(axe.get("weight").is_none(), "{axe}");
    }

    #[test]
    fn the_pharao_arrows_are_the_only_two_looting_entries() {
        let pack = pack();
        matches_delivered(&pack, "minecolonies/loot_table/entities/pharao.json");
        let mut looting = 0;
        for (mob, _, _) in MOBS {
            let table = pack
                .get(&format!("minecolonies/loot_table/entities/{mob}.json"))
                .unwrap();
            let text = table.to_string();
            looting += text.matches("minecraft:looting_enchant").count();
        }
        assert_eq!(looting, 2);
    }

    #[test]
    fn the_two_bottle_tables_are_a_luck_flip_and_gravel_is_not() {
        let pack = pack();
        for path in [
            "minecolonies/loot_table/recipes/glass_bottle.json",
            "minecolonies/loot_table/recipes/large_bottle.json",
            "minecolonies/loot_table/recipes/gravel.json",
        ] {
            matches_delivered(&pack, path);
        }
        let gravel = pack.get("minecolonies/loot_table/recipes/gravel.json").unwrap();
        assert!(gravel["pools"][0]["entries"][0].get("quality").is_none());
    }

    #[test]
    fn the_16_sifter_tables_pair_every_mesh_with_every_block() {
        let pack = pack();
        let mut checked = 0;
        for (mesh, input, _, _, _) in SIFTER {
            matches_delivered(
                &pack,
                &format!("minecolonies/loot_table/recipes/{mesh}/{input}.json"),
            );
            checked += 1;
        }
        assert_eq!(checked, 16);
        assert_eq!(pack.under("minecolonies/loot_table/recipes/string/").len(), 4);
    }

    #[test]
    fn only_the_iron_and_diamond_meshes_roll_a_bonus() {
        // `setBonusRolls` on two of the four meshes only (`:77`, `:95`).
        let pack = pack();
        for (mesh, input, _, bonus, _) in SIFTER {
            let path = format!("minecolonies/loot_table/recipes/{mesh}/{input}.json");
            let table = pack.get(&path).unwrap();
            assert_eq!(
                table["pools"][0]["bonus_rolls"],
                f(bonus),
                "{path}: the bonus roll of this mesh"
            );
        }
    }

    #[test]
    fn the_five_nether_trips_cumulate_and_pay_an_adventure_token() {
        let pack = pack();
        for level in 1..=5 {
            matches_delivered(
                &pack,
                &format!("minecolonies/loot_table/recipes/netherworker/trip{level}.json"),
            );
        }
        let one = pack
            .get("minecolonies/loot_table/recipes/netherworker/trip1.json")
            .unwrap();
        let five = pack
            .get("minecolonies/loot_table/recipes/netherworker/trip5.json")
            .unwrap();
        let blocks = |table: &Json| table["pools"][0]["entries"].as_array().unwrap().len();
        assert_eq!(blocks(one), 5);
        assert!(blocks(five) > blocks(one), "level 5 rolls more");
        assert_eq!(one["pools"][1]["entries"][0]["name"], s("minecolonies:adventure_token"));
        assert_eq!(
            one["pools"][1]["entries"][0]["functions"][0]["tag"],
            s("{damage-done:5,entity-type:\"minecraft:zombified_piglin\",xp-dropped:5}")
        );
    }

    #[test]
    fn a_nether_level_three_trip_has_the_three_froglight() {
        let pack = pack();
        let three = pack
            .get("minecolonies/loot_table/recipes/netherworker/trip3.json")
            .unwrap();
        let text = three.to_string();
        for light in ["ochre_froglight", "pearlescent_froglight", "verdant_froglight"] {
            assert!(text.contains(light), "{light}");
        }
        let two = pack
            .get("minecolonies/loot_table/recipes/netherworker/trip2.json")
            .unwrap();
        assert!(!two.to_string().contains("froglight"), "level 2 has none");
    }

    #[test]
    fn the_32_banner_tables_are_written_into_the_vanilla_namespace() {
        let files = vanilla_banner_tables();
        assert_eq!(files.len(), 32);
        assert!(files
            .iter()
            .all(|file| file.path.starts_with("minecraft/loot_table/blocks/")));
        let black = &files[0];
        assert_eq!(black.path, "minecraft/loot_table/blocks/black_banner.json");
        let pool = &black.json["pools"][0];
        assert_eq!(pool["conditions"][0]["condition"], s("minecraft:survives_explosion"));
        assert_eq!(pool["functions"][1]["function"], s("minecraft:copy_nbt"));
        assert_eq!(
            pool["functions"][1]["ops"][0]["target"],
            s("BlockEntityTag.Patterns")
        );
        // A wall banner has no item of its own: its table keeps the wall id but
        // names the **standing** banner (`block.asItem()`,
        // `DefaultBlockLootTableProvider.java:162`, and the delivered
        // `minecraft/loot_tables/blocks/black_wall_banner.json`).
        let wall = &files[1];
        assert_eq!(wall.path, "minecraft/loot_table/blocks/black_wall_banner.json");
        assert_eq!(
            wall.json["pools"][0]["entries"][0]["name"],
            s("minecraft:black_banner")
        );
    }

    #[test]
    fn the_five_enchanter_tables_are_refused_rather_than_guessed() {
        let pack = pack();
        assert!(pack
            .refused
            .iter()
            .any(|refusal| matches!(refusal, Refusal::HandWritten { files: 5, .. })));
        assert!(pack
            .under("minecolonies/loot_table/recipes/")
            .iter()
            .all(|file| !file.path.contains("enchanter")));
    }

    #[test]
    fn every_item_a_table_names_is_an_item_this_port_registers() {
        // The cross-check rule 6 of the brief asks for: a table that named an id
        // the port knows in neither registry would be a typo the JSON comparison
        // could only catch for the families that have a delivered file. The
        // delivered JSON comparison cannot see it at all — the pack names the
        // 62 block items this port declares as blocks.
        let pack = pack();
        let mut unknown = Vec::new();
        for file in &pack.files {
            if !file.path.contains("/loot_table/") {
                continue;
            }
            collect_unknown(&file.json, &mut unknown);
        }
        unknown.sort_unstable();
        unknown.dedup();
        assert!(unknown.is_empty(), "unknown items: {unknown:?}");
    }

    fn collect_unknown(value: &Json, unknown: &mut Vec<String>) {
        match value {
            Json::Object(map) => {
                // A nested table's `name` is a table id (`NestedLootTable`), not an
                // item: `fisherman/junk` is one of the three the fishing root rolls.
                let is_table_reference =
                    map.get("type").and_then(|kind| kind.as_str()) == Some("minecraft:loot_table");
                if !is_table_reference {
                    if let Some(Json::String(name)) = map.get("name") {
                        let path = super::super::json::path_of(name).to_owned();
                        let namespace = super::super::json::namespace_of(name);
                        // Two registries can own an id of this namespace: `crate::items`
                        // for the 163 items of `ModItems`, and `crate::blocks` for the
                        // block items, which the *block* initialiser registers
                        // (`ModBlocksInitializer.registerBlockItem`, one call per block).
                        // A block table names the block's item, so asking only
                        // `crate::items` would call every hut a typo.
                        if namespace == "minecolonies"
                            && crate::items::get(&path).is_none()
                            && crate::blocks::block(&path).is_none()
                        {
                            unknown.push(path);
                        }
                    }
                }
                for child in map.values() {
                    collect_unknown(child, unknown);
                }
            }
            Json::Array(items) => {
                for item in items {
                    collect_unknown(item, unknown);
                }
            }
            _ => {}
        }
    }
}
