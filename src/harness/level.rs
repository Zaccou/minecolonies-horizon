//! The world a test runs in: blocks, block entity data and mobs, in memory.

use std::collections::BTreeMap;

use mod_api::__private::parts_installation;
use mod_api::{BlockKey, Compound, DimensionKey, LevelRef, MobGoalEntry, Text, WorldEpoch};

/// The world epoch every fake handle names. The engine changes it when a world
/// closes and another opens (`crates/horizon-mod-vocab/src/world/types.rs:227-240`);
/// a test has one world, so one epoch, and a handle here cannot name another
/// world's block entity.
pub const EPOCH: u64 = 1;

/// The opaque key a fake world answers for the block `name`.
///
/// **A mod cannot build a `BlockKey`**: its field is `pub(crate)` and no public
/// constructor exists (`crates/horizon-mod-vocab/src/block/types.rs:6-8`), yet the
/// port's whole "is this a hut" test is `level.block(pos) == host.block_key(ID)`.
/// The game's own authoring constructor is `installation_tokens::block_key(slot)`
/// (`crates/horizon-mod-vocab/src/parts/installation_tokens/block_key.rs:3-5`),
/// re-exported as `__private::parts_installation`
/// (`crates/horizon-mod-vocab/generated/mod_api_exports_private.rs`). It wraps a slot
/// and validates nothing, so the slot is a hash of the name: one name, one key, for
/// the process — which is all the port asks, since both sides of its comparison come
/// from here.
pub fn block_key(name: &str) -> BlockKey {
    parts_installation::block_key(slot(name))
}

/// A stable name-to-slot map with no registry behind it (FNV-1a).
fn slot(name: &str) -> u32 {
    let mut slot: u32 = 0x811c_9dc5;
    for byte in name.as_bytes() {
        slot ^= u32::from(*byte);
        slot = slot.wrapping_mul(0x0100_0193);
    }
    slot
}

/// The key of one of vanilla's three dimensions.
///
/// **A mod cannot name a dimension the way the game does.** `dimension_key(id)` and
/// `dimension_id(key)` are not public: they sit in the private `mod __private` of
/// `crates/horizon-mod-vocab/src/exports/registry_keys.rs:3-22`, and `DimensionKey`'s
/// field is `pub(crate)` with no `Default` (`crates/horizon-mod-vocab/src/world/types.rs:15`),
/// `from_slot` being `pub(crate)` too (`content/registry_keys.rs:369-374`). What *is*
/// public is the three constants below (`content/registry_keys.rs:361-367`), and
/// `LevelRef::new` is `pub` though `#[doc(hidden)]`
/// (`crates/horizon-mod-vocab/src/world/types.rs:307-317`). So a test world can only
/// be vanilla's three — a mod's own dimension id has no key a mod can write.
pub fn dimension_key(id: &str) -> DimensionKey {
    match id {
        "minecraft:the_nether" => DimensionKey::NETHER,
        "minecraft:the_end" => DimensionKey::END,
        _ => DimensionKey::OVERWORLD,
    }
}

/// One block: its key, and its `facing` state property when it has one.
#[derive(Clone, Debug)]
pub struct FakeBlock {
    pub key: BlockKey,
    pub facing: Option<String>,
}

/// A mob: what `citizen` and `hire` read and write on it.
#[derive(Clone, Debug)]
pub struct FakeMob {
    pub kind: String,
    pub goals: Vec<MobGoalEntry>,
    pub persistent: bool,
    /// Where `move_to` sent it, and how fast.
    pub path: Option<([f64; 3], f64)>,
    pub name: Option<Text>,
}

/// The overworld, empty.
pub struct FakeLevel {
    dimension: &'static str,
    level: LevelRef,
    blocks: BTreeMap<[i32; 3], FakeBlock>,
    data: BTreeMap<[i32; 3], Compound>,
    mobs: BTreeMap<u32, FakeMob>,
}

impl Default for FakeLevel {
    fn default() -> Self {
        Self::new("minecraft:overworld")
    }
}

impl FakeLevel {
    /// An empty level of `dimension` (`minecraft:the_nether`).
    pub fn new(dimension: &'static str) -> Self {
        let key = dimension_key(dimension);
        Self {
            dimension,
            level: LevelRef::new(WorldEpoch::new(EPOCH), key),
            blocks: BTreeMap::new(),
            data: BTreeMap::new(),
            mobs: BTreeMap::new(),
        }
    }

    /// The handle the host's level reads take.
    pub fn level(&self) -> LevelRef {
        self.level
    }

    /// The dimension's id, as `entity_join_level` is told it.
    pub fn dimension(&self) -> &'static str {
        self.dimension
    }

    /// Puts block `name` at `pos`, with its `facing` property when it has one.
    pub fn put_block(&mut self, pos: [i32; 3], name: &str, facing: Option<&str>) {
        let key = block_key(name);
        let facing = facing.map(str::to_owned);
        self.blocks.insert(pos, FakeBlock { key, facing });
    }

    /// The block at `pos`: what `LevelRef::block` answers.
    pub fn block_at(&self, pos: [i32; 3]) -> Option<BlockKey> {
        self.blocks.get(&pos).map(|block| block.key)
    }

    /// Property `key` of the block at `pos`. One exists here, `facing`; anything else
    /// is absent, as on a block that does not declare it.
    pub fn property_at(&self, pos: [i32; 3], key: &str) -> Option<String> {
        let block = self.blocks.get(&pos)?;
        match key {
            "facing" => block.facing.clone(),
            _ => None,
        }
    }

    /// Gives `pos` the block entity data `data`, as placing a declared block does.
    pub fn put_data(&mut self, pos: [i32; 3], data: Compound) {
        self.data.insert(pos, data);
    }

    /// Whether a block entity with saved data stands at `pos`.
    pub fn has_data(&self, pos: [i32; 3]) -> bool {
        self.data.contains_key(&pos)
    }

    /// The data at `pos`, as `block_entity.data` reads it.
    pub fn data_at(&self, pos: [i32; 3]) -> Option<&Compound> {
        self.data.get(&pos)
    }

    /// Merges `payload` into the data at `pos` key by key, as
    /// `block_entity_merge_data` and `/data merge block` do; false when nothing stands
    /// there. `extend` replaces a key it holds and keeps the others — the whole
    /// difference between merging and replacing.
    pub fn merge_at(&mut self, pos: [i32; 3], payload: Compound) -> bool {
        let Some(data) = self.data.get_mut(&pos) else {
            return false;
        };
        data.extend(payload);
        true
    }

    /// Adds mob `id` of type `kind` with `goals`, and gives it back.
    pub fn spawn(&mut self, id: u32, kind: &str, goals: Vec<MobGoalEntry>) -> &mut FakeMob {
        self.mobs.entry(id).or_insert_with(|| FakeMob {
            kind: kind.to_owned(),
            goals,
            persistent: false,
            path: None,
            name: None,
        })
    }

    /// Mob `id`, if one stands in this level.
    pub fn mob(&self, id: u32) -> Option<&FakeMob> {
        self.mobs.get(&id)
    }

    /// Mob `id` to change, if one stands in this level.
    pub fn mob_mut(&mut self, id: u32) -> Option<&mut FakeMob> {
        self.mobs.get_mut(&id)
    }
}
