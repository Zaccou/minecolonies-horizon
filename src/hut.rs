//! The hut block's block entity: the initial saved compound, and the reads a
//! command or a click does on the way back.
//!
//! The state lives in the **saved compound** of the block entity, never in a
//! `BlockEntityPartState`: a wasm mod cannot declare a state without a codec
//! (`docs/modding/src/concepts/parts/block-entity-types.md:95-97`), and the
//! compound already survives with the chunk, with no Wire depth limit.
//!
//! **Two of the seven keys are optional, and that is what keeps an old save
//! readable.** A hut is placed with the five keys of [`initial_data`]; the two
//! that link it to a colony ([`KEY_COLONY`], [`KEY_MAIN`]) are written later, by
//! whoever knows the colony — nothing in this slice does (`src/wiring.rs` says
//! where the record lives). A hut without them reads as a hut that belongs to no
//! colony, which is exactly what a hut placed before the link existed is.

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, LevelSide, ModContainer, Tag,
};

/// The block, by full registry name. **Never bare**: a bare name in
/// `.blocks(&[...])` is qualified with the **mod id** (`ported_minecolonies`),
/// not with a declared namespace (`crates/horizon-mod-vocab/src/parts/owned_namespaces.rs:14-19`),
/// and would name a block that does not exist. The same name is what
/// `Host::block_key` takes.
pub const BLOCK_ID: &str = "minecolonies:blockhutbuilder";
/// The block entity type, by full registry name: the namespace this package
/// declares in `horizon-mod.toml`.
pub const TYPE_ID: &str = "minecolonies:hut";

/// The menu a hut's container shows. A **vanilla** menu on purpose: the grid
/// table `MOD_MENUS` gives it a stock screen on every Horizon client
/// (`crates/horizon-client-render/src/chunk/block_entity.rs:119-131`), while a
/// content menu type with no `native_screen` is closed again by the client
/// (`crates/horizon-client/src/game/network/remote/mod.rs:9833-9846`).
pub const MENU_ID: &str = "minecraft:generic_3x3";
/// The slot count `MENU_ID` shows. `native_containers` drops the menu when a
/// container's slot count and the menu's differ
/// (`crates/horizon-client/src/game/native_hooks/shared.rs:2832-2834`).
pub const SLOTS: usize = 9;

pub const KEY_STYLE: &str = "style";
pub const KEY_MIRROR: &str = "mirror";
pub const KEY_BLUEPRINT: &str = "blueprint";
pub const KEY_LEVEL: &str = "level";
pub const KEY_HIRED: &str = "hired";
/// The colony this hut belongs to: the colony's `id`, the same key
/// `Colony::to_compound` writes (`crate::colony::colonie::KEY_ID`). Absent on a
/// hut that belongs to none, and `0` is read as none: the id counter starts at
/// `1`, so a zero is a link that was cleared, not a colony.
pub const KEY_COLONY: &str = "colony";
/// Where the main building (the town hall) stands: the position of the block
/// entity that holds the colony record, laid out as [`position`] lays one out.
/// The link is a **position**, not a handle: a `BlockEntityRef` dies with its
/// chunk, a position survives the unload.
pub const KEY_MAIN: &str = "main";

pub const STYLE_DEFAULT: &str = "minecolonies:medieval_oak";
pub const MIRROR_DEFAULT: &str = "NONE";
pub const BLUEPRINT_DEFAULT: &str = "builder";
/// A hut is level 1 when it is placed and reaches level 5, as in MineColonies.
pub const LEVEL_MIN: i32 = 1;
pub const LEVEL_MAX: i32 = 5;

/// How many blocks in front of the hut the citizen stands, and how far it then
/// walks: two cells, one step at a time, on flat ground.
pub const SPAWN_DISTANCE: i32 = 1;
pub const WALK_DISTANCE: i32 = 4;

/// The block entity part. `block_entity` is a `BlockEntityPart` method, not a
/// global `Hooks` one (`docs/modding/src/concepts/parts/block-entity-types.md:19-21`).
pub struct Hut;

/// `BlockEntityPart` is declared `pub trait BlockEntityPart: BlockPart`
/// (`.horizon/sdk/api/crates/horizon-mod-vocab/generated/mod_api_generated.rs:8879`),
/// so every block entity Part is also a `BlockPart`. This hut answers no block
/// hook; `#[horizon_part]` gives it the empty subscription list.
#[mod_api::horizon_part]
impl BlockPart for Hut {}

#[mod_api::horizon_part]
impl BlockEntityPart for Hut {
    /// The compound a hut starts with. Keyed on the **block**, so turning it
    /// (`facing: north` to `facing: east`) keeps the entity and its data
    /// (`docs/modding/src/how-to/world/machine-menu-data.md:18-20`).
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == BLOCK_ID).then(initial_data)
    }
}

/// What a hut is when it is placed, before anything wrote to it.
pub fn initial_data() -> Compound {
    Compound::from([
        (KEY_STYLE.into(), Tag::String(STYLE_DEFAULT.into())),
        (KEY_MIRROR.into(), Tag::String(MIRROR_DEFAULT.into())),
        (KEY_BLUEPRINT.into(), Tag::String(BLUEPRINT_DEFAULT.into())),
        (KEY_LEVEL.into(), Tag::Int(LEVEL_MIN)),
        (KEY_HIRED.into(), Tag::Int(0)),
    ])
}

/// The container `Registrar::block_container` declares: nine slots, every face
/// reaches them, nothing is automation-only, and the menu names the stock 3x3.
pub fn container() -> ModContainer {
    ModContainer {
        slots: SLOTS,
        menu: Some(MENU_ID.to_owned()),
        ..Default::default()
    }
}

/// A hut's data, read back with the defaults a missing key takes.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct State {
    pub style: String,
    pub mirror: String,
    pub blueprint: String,
    pub level: i32,
    pub hired: i32,
    /// The colony this hut belongs to, when it knows one.
    pub colony: Option<i32>,
    /// Where that colony's record lives, when the hut knows one.
    pub main: Option<[i32; 3]>,
}

/// The data of a hut, as the compound holds it.
pub fn state_of(data: &Compound) -> State {
    State {
        style: text(data, KEY_STYLE, STYLE_DEFAULT),
        mirror: text(data, KEY_MIRROR, MIRROR_DEFAULT),
        blueprint: text(data, KEY_BLUEPRINT, BLUEPRINT_DEFAULT),
        level: level_of(data),
        hired: int(data, KEY_HIRED).unwrap_or(0).max(0),
        colony: colony_of(data),
        main: position_in(data, KEY_MAIN),
    }
}

/// The colony id a hut reads back, `None` for a hut that belongs to none: a
/// missing key, a value of another kind, and a `0` all mean the same thing, so a
/// link cleared by hand cannot name colony zero.
pub fn colony_of(data: &Compound) -> Option<i32> {
    int(data, KEY_COLONY).filter(|id| *id > 0)
}

/// The hut's level, clamped to 1..=5: a hand-written `/data merge` cannot put
/// a hut out of the range the rest of the port assumes.
pub fn level_of(data: &Compound) -> i32 {
    clamp_level(int(data, KEY_LEVEL).unwrap_or(LEVEL_MIN))
}

pub fn clamp_level(level: i32) -> i32 {
    level.clamp(LEVEL_MIN, LEVEL_MAX)
}

/// How many citizens a hut of `level` houses: one per level, so a level-3 hut
/// houses three. This port's rule, stated here and nowhere else.
pub fn capacity(level: i32) -> i32 {
    clamp_level(level)
}

/// How many citizens the hut may still hire: zero or less means full.
pub fn room(state: &State) -> i32 {
    capacity(state.level) - state.hired
}

/// The merge payload that records `hired` citizens, without rewriting the other
/// keys: `merge_data` merges, it does not replace
/// (`crates/horizon-mod-vocab/src/host/block_entities.rs:107-120`).
pub fn with_hired(hired: i32) -> Compound {
    Compound::from([(KEY_HIRED.into(), Tag::Int(hired.max(0)))])
}

/// The merge payload that links a hut to a colony: its id and where the
/// record stands. Merging keeps the five keys a placed hut carries.
pub fn with_colony(colony: i32, main: [i32; 3]) -> Compound {
    Compound::from([
        (KEY_COLONY.into(), Tag::Int(colony)),
        (KEY_MAIN.into(), position(main)),
    ])
}

/// A position, laid out as `BlockPosUtil` lays one out: three ints under `x`,
/// `y`, `z` (`api/util/BlockPosUtil.java:190-197`, written at `:219-226`).
///
/// **This is a copy of `crate::colony::nbt::position`,** which is `mod nbt;`
/// inside `colony` and therefore not reachable from here
/// (`src/colony/mod.rs:28`); `src/building/nbt.rs:8-11` keeps the same copy for
/// the same reason, and `src/requests/nbt.rs` a third. Making the colony's
/// `nbt` public is a one-line change to a module this agent may not touch.
pub fn position(at: [i32; 3]) -> Tag {
    Tag::compound([
        ("x", Tag::Int(at[0])),
        ("y", Tag::Int(at[1])),
        ("z", Tag::Int(at[2])),
    ])
}

/// The position stored under `key`, or `None` when the compound holds no
/// readable one: a link without its position is a link that cannot be followed.
pub fn position_in(data: &Compound, key: &str) -> Option<[i32; 3]> {
    let tag = data.get(&key.into())?;
    Some([
        number(tag.get("x")?)?,
        number(tag.get("y")?)?,
        number(tag.get("z")?)?,
    ])
}

/// The unit offset `facing` points at, on the XZ plane:
/// `north` is -Z, `south` +Z, `east` +X, `west` -X. An unknown facing reads as
/// `north`, which is what `default_state` places.
pub fn facing_offset(facing: &str) -> [i32; 2] {
    match facing {
        "south" => [0, 1],
        "east" => [1, 0],
        "west" => [-1, 0],
        _ => [0, -1],
    }
}

/// Where the citizen appears: the centre of the block `SPAWN_DISTANCE` in front
/// of the hut, at the hut's own Y, feet on the floor.
pub fn spawn_at(hut: [i32; 3], facing: &str) -> [f64; 3] {
    let [dx, dz] = facing_offset(facing);
    let x = hut[0] + dx * SPAWN_DISTANCE;
    let z = hut[2] + dz * SPAWN_DISTANCE;
    [f64::from(x) + 0.5, f64::from(hut[1]), f64::from(z) + 0.5]
}

/// Where the citizen walks to: `WALK_DISTANCE` blocks in front of the hut, so
/// a hire is a visible walk and not a pop-in.
pub fn walk_to(hut: [i32; 3], facing: &str) -> [f64; 3] {
    let [dx, dz] = facing_offset(facing);
    let x = hut[0] + dx * WALK_DISTANCE;
    let z = hut[2] + dz * WALK_DISTANCE;
    [f64::from(x) + 0.5, f64::from(hut[1]), f64::from(z) + 0.5]
}

fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

/// An integer tag, in whichever width NBT saved it: a `/data merge block` writes
/// a `byte` where the port writes an `int`.
fn number(tag: &Tag) -> Option<i32> {
    match tag {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

fn text(data: &Compound, key: &str, default: &str) -> String {
    match data.get(&key.into()) {
        Some(Tag::String(value)) => value.to_string_lossy(),
        _ => default.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: [i32; 3] = [10, 64, -10];

    #[test]
    fn a_placed_hut_belongs_to_no_colony() {
        let data = initial_data();
        assert_eq!(data.len(), 5, "the link is not written at placement");
        let state = state_of(&data);
        assert_eq!(state.colony, None);
        assert_eq!(state.main, None);
        assert_eq!(
            state_of(&Compound::new()).colony,
            None,
            "a compound written by an older build reads the same way"
        );
    }

    #[test]
    fn a_link_reads_back_and_leaves_the_five_keys_alone() {
        let mut data = initial_data();
        data.extend(with_colony(7, MAIN));
        let state = state_of(&data);
        assert_eq!(state.colony, Some(7));
        assert_eq!(state.main, Some(MAIN));
        assert_eq!(state.style, STYLE_DEFAULT, "the merge named two keys only");
        assert_eq!(state.level, LEVEL_MIN);
        assert_eq!(data.len(), 7);
    }

    #[test]
    fn a_colony_of_zero_and_a_half_position_are_no_link() {
        let mut data = initial_data();
        data.insert(KEY_COLONY.into(), Tag::Int(0));
        assert_eq!(colony_of(&data), None, "0 is a cleared link, not colony zero");
        data.insert(KEY_COLONY.into(), Tag::Byte(7));
        assert_eq!(colony_of(&data), Some(7), "a width written by hand still reads");

        // A position without its `y` cannot be followed: a link half-written by
        // hand is no link.
        data.insert(
            KEY_MAIN.into(),
            Tag::compound([("x", Tag::Int(4)), ("z", Tag::Int(4))]),
        );
        assert_eq!(position_in(&data, KEY_MAIN), None);
        data.insert(KEY_MAIN.into(), Tag::String("here".into()));
        assert_eq!(position_in(&data, KEY_MAIN), None);
    }

    #[test]
    fn a_position_is_three_ints_and_reads_back_as_written() {
        let data = Compound::from([(KEY_MAIN.into(), position([-3, 71, 1208]))]);
        assert_eq!(position_in(&data, KEY_MAIN), Some([-3, 71, 1208]));
        assert_eq!(position_in(&Compound::new(), KEY_MAIN), None);
    }
}

/// Le nombre de ticks que la boucle native du jeu a donnes a un block entity de mod.
pub static MOD_TICKS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Appelee par le slot de la boucle `block_entities.tick` (route native) a chaque tick d'un block
/// entity de mod. C'est le point de jonction entre le jeu et ce crate : si ce compteur monte, la
/// chaine jeu -> slot -> port fonctionne.
pub fn note_mod_tick(seen: u64) {
    MOD_TICKS.store(seen, core::sync::atomic::Ordering::Relaxed);
}

/// Combien de ticks le port a recus par la route native.
pub fn mod_ticks() -> u64 {
    MOD_TICKS.load(core::sync::atomic::Ordering::Relaxed)
}
