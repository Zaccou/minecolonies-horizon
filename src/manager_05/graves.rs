//! The graves of a colony: where they are, which one is taken, what a death puts
//! in one, and when one rots.
//!
//! Java reference: `core/colony/managers/GraveManager.java` (332 lines), the
//! decay of `core/tileentities/TileEntityGrave.java:228-251` and
//! `api/tileentities/AbstractTileEntityGrave.java:22`, `:51-54`, the ring search
//! of `api/util/BlockPosUtil.java:1035-1127`, `TranslationConstants.java:600-602`
//! and `NbtTagConstants.java:29`, `:51`, `:95`.
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | what the colony holds | `BlockPos → reserved`, nothing else | `:47` |
//! | when a grave is dropped from the map | the block there is loaded and is not a grave | `:119-125`, `:228-233` |
//! | when it is dropped silently | a reserved grave looking for a bed, and nothing was written | `:228-233` |
//! | how long a grave lasts | 12 000 ticks, then it is `decayed`, 12 000 more, then it is gone | `:22`, `:228-251` |
//! | what a death's inventory does | the grave first, the ground as the fallback, then each armour piece | `:294-308` |
//! | what decides a grave may be made at all | lava refuses; water demands air above; anything else searches | `:256-289` |
//!
//! **Four refusals, and every one of them happens before a block is placed.**
//! Java checks lava first (`:257-261`), then the water probe (`:266-282`), then
//! the search (`:285-289`), and only a position that survived all three is built
//! on (`:294`). The port returns a [`Site`] and a [`SiteRefusal`], and the caller
//! builds nothing on a refusal — which is what lets a test prove the ordering.
//!
//! **Five things a tidy port would get wrong, and this one keeps.**
//!
//! 1. **The two radii are not the two ranges you would guess.** The water branch
//!    calls `findAround(world, pos, 1, 16, …)` (`:270`): **one** block vertically,
//!    **sixteen** horizontally. The dry branch calls `findAround(world, pos, 10,
//!    10, …)` (`:285`). Read the signature before believing either.
//! 2. **The water probe decides whether to search at all, not where.** `:266-276`
//!    walks `pos.above(i)` for `i` in `1..=10` and, on the first air block,
//!    searches **around `pos`** — the air it found is not the search's start. If
//!    none of the ten is air, no search happens and the water warning is sent.
//! 3. **A dry failure says nothing.** Only lava (`:259`) and water (`:280`) carry
//!    a message; a dry search that finds nothing drops the inventory in silence
//!    (`:328`).
//! 4. **The whole inventory is dropped, not the part that did not fit.** `:297-300`
//!    drops everything at the death position when the grave refuses the transfer,
//!    even though the transfer may have stored some of it first.
//! 5. **`reserveNextFreeGrave` removes a broken grave without saving.**
//!    `:231` removes it from the map and never calls `markDirty`, unlike
//!    `onColonyTick` (`:123`) which does.
//!
//! **`createCitizenGrave` also carries the whole inventory through.** That is not
//! a rule but four writes to a world the port does not have: the block, the
//! transfer, the armour loop and the decay delay. They are modelled as an ordered
//! [`Deposit`] log, because the order **is** the rule (`:297`, `:301`, `:311`,
//! `:323`) and a test can read it.
//!
//! **Pure data, pure rules**: the world is two closures — [`Slot`] for a grave
//! position and [`Decay`] for its timer — and nothing here touches a host.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;

/// The list, inside the `graveManager` compound (`NbtTagConstants.java:29`).
pub const KEY_GRAVE: &str = "graves";
/// The `graveManager` compound, at the colony level (`Colony.java:945`,
/// `NbtTagConstants.java:60`).
pub const KEY_GRAVE_MANAGER: &str = "graveManager";
/// One grave's position (`:79`, `NbtTagConstants.java:51`).
pub const KEY_POS: &str = "pos";
/// One grave's reservation (`:79`, `NbtTagConstants.java:95`).
pub const KEY_RESERVED: &str = "reserved";

/// The decay timer of a new grave: `TICKS_SECOND * 60 * 10`, twenty ticks a
/// second for ten minutes — 12 000 ticks
/// (`AbstractTileEntityGrave.java:22`, `Constants.java:21`).
pub const DEFAULT_DECAY_TIMER: i32 = 20 * 60 * 10;
/// The delay the colony tick hands the grave, `MAX_TICKRATE`
/// (`TileEntityGrave.java:228`, `TickRateConstants.java:11`). One call is 25
/// seconds of game time.
pub const COLONY_TICK_DELAY: i32 = 500;
/// How many blocks above a drowned death Java will look for air
/// (`GraveManager.java:266`).
pub const WATER_PROBE_HEIGHT: i32 = 10;
/// The horizontal range of the water search (`:270`) — **sixteen**, not one.
pub const WATER_SEARCH_HORIZONTAL: i32 = 16;
/// The vertical range of the water search (`:270`).
pub const WATER_SEARCH_VERTICAL: i32 = 1;
/// The two ranges of the dry search (`:285`).
pub const DRY_SEARCH_RANGE: i32 = 10;
/// Every slot of a citizen's equipment, which Java walks whole (`:301`):
/// `EquipmentSlot.values().length` in 1.20.1, main hand, off hand, feet, legs,
/// chest, head, body.
pub const EQUIPMENT_SLOTS: u8 = 7;

/// `WARNING_GRAVE_LAVA` (`TranslationConstants.java:600`), sent when a death
/// happened in lava (`:259`).
pub const WARNING_LAVA: &str = "com.minecolonies.coremod.grave.lava";
/// `WARNING_GRAVE_WATER` (`:602`), sent when the water search found nothing
/// (`:280`).
pub const WARNING_WATER: &str = "com.minecolonies.coremod.grave.water";

/// The medium a death happened in — `here.getBlock() == Blocks.LAVA` and its
/// water test (`:257`, `:264`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three blocks that change the rule
pub enum Medium {
    /// Lava: no grave at all, and the managers are told (`:257-261`).
    Lava,
    /// Water: air must be found above before the search even starts (`:264-282`).
    Water,
    /// Anything else, including air: the ten-by-ten search (`:284-289`).
    Dry,
}

/// Why no grave can be made. Every one of them is answered **before** the block
/// is placed, which is the ordering the tests hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three ways to have nowhere to put a grave
pub enum SiteRefusal {
    /// `:257-261`: lava. The message is [`WARNING_LAVA`]; the inventory is left
    /// where it is, because Java never reaches the drop.
    Lava,
    /// `:266-282`: water with no air above, or a search that found nothing. The
    /// message is [`WARNING_WATER`], and the inventory is dropped at the death
    /// position (`:328`).
    Water,
    /// `:285-289`, `:328`: dry ground and no free cell. **No message**: Java's
    /// `else` branch has no warning.
    NoGround,
}

impl SiteRefusal {
    pub fn name(&self) -> &'static str {
        match self {
            SiteRefusal::Lava => "lava",
            SiteRefusal::Water => "water",
            SiteRefusal::NoGround => "no_ground",
        }
    }

    /// The translation key the colony hears, if any (`:259`, `:280`).
    pub fn warning(&self) -> Option<&'static str> {
        match self {
            SiteRefusal::Lava => Some(WARNING_LAVA),
            SiteRefusal::Water => Some(WARNING_WATER),
            SiteRefusal::NoGround => None,
        }
    }
}

/// Java's `BlockPosUtil.findAround` (`:1035-1127`), the ring search both grave
/// branches use.
///
/// The order is the rule and a test pins it: the start, then the four horizontal
/// neighbours in `Direction.Plane.HORIZONTAL` order (north, east, south, west),
/// then a ring per radius `steps` — a square walked east, south, west, north,
/// each edge `steps + 1` cells long, starting from the north-west corner **after
/// one step**, so each ring ends on its corner. Then the height ladder: `0`,
/// `+1`, `-1`, `+2`, `-2`, … `vertical + 2` levels of it, and the search stops the
/// moment one of those heights leaves the world (`:1121-1124`).
///
/// **The square is not centred.** `:1069` starts at `start.offset(-steps, y,
/// -steps)` and then moves *before* the first test, so the visited square spans
/// `-steps ..= +1` on both axes, not `-steps ..= +steps`. That is structurize's
/// arithmetic, not a transcription slip: [`visited`] shows it.
pub fn find_around(
    start: [i32; 3],
    vertical: i32,
    horizontal: i32,
    mut free: impl FnMut([i32; 3]) -> bool,
    in_world: impl Fn(i32) -> bool,
) -> Option<[i32; 3]> {
    if horizontal < 1 && vertical < 1 {
        return None;
    }
    if free(start) {
        return Some(start);
    }
    for direction in [
        [0, 0, -1],
        [1, 0, 0],
        [0, 0, 1],
        [-1, 0, 0],
    ] {
        let candidate = [start[0] + direction[0], start[1], start[2] + direction[2]];
        if free(candidate) {
            return Some(candidate);
        }
    }

    let mut y = 0;
    let mut offset = 1;
    for _ in 0..vertical + 2 {
        for steps in 1..=horizontal {
            let mut x = -steps;
            let mut z = -steps;
            for edge in 0..4 {
                for _ in 0..=steps {
                    match edge {
                        0 => x += 1,
                        1 => z += 1,
                        2 => x -= 1,
                        _ => z -= 1,
                    }
                    let candidate = [start[0] + x, start[1] + y, start[2] + z];
                    if free(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }

        y += offset;
        offset = if offset > 0 { offset + 1 } else { offset - 1 };
        offset *= -1;
        if !in_world(start[1] + y) {
            return None;
        }
    }
    None
}

/// The cells [`find_around`] visits, in order — the same walk, written out, so a
/// test can hold the order. Nothing else calls it.
pub fn visited(
    start: [i32; 3],
    vertical: i32,
    horizontal: i32,
    mut seen: impl FnMut([i32; 3]),
) {
    let _ = find_around(
        start,
        vertical,
        horizontal,
        |cell| {
            seen(cell);
            false
        },
        |_| true,
    );
}

/// Where a grave may be built, or why not (`:254-331`).
pub fn find_site(
    at: [i32; 3],
    medium: Medium,
    free: impl FnMut([i32; 3]) -> bool,
    air: impl Fn([i32; 3]) -> bool,
    in_world: impl Fn(i32) -> bool,
) -> Result<[i32; 3], SiteRefusal> {
    if medium == Medium::Lava {
        return Err(SiteRefusal::Lava);
    }
    if medium == Medium::Water {
        // `:266-276`: the probe decides *whether* to search, and the search is
        // around `pos`, not around the air it found.
        let mut searched = false;
        for i in 1..=WATER_PROBE_HEIGHT {
            if air([at[0], at[1] + i, at[2]]) {
                searched = true;
                break;
            }
        }
        if !searched {
            return Err(SiteRefusal::Water);
        }
        return find_around(
            at,
            WATER_SEARCH_VERTICAL,
            WATER_SEARCH_HORIZONTAL,
            free,
            in_world,
        )
        .ok_or(SiteRefusal::Water);
    }
    find_around(at, DRY_SEARCH_RANGE, DRY_SEARCH_RANGE, free, in_world)
        .ok_or(SiteRefusal::NoGround)
}

/// What the world says about a position the colony holds as a grave
/// (`:114-125`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java's three branches, in its order
pub enum Slot {
    /// `!WorldUtil.isBlockLoaded` (`:114`): skipped, and the entry is kept.
    Unloaded,
    /// Loaded, and no `TileEntityGrave` there (`:120`): the entry is dropped.
    NotAGrave,
    /// A grave that is still there.
    Grave,
}

/// One step of a grave's rot (`TileEntityGrave.java:228-251`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three ways `onColonyTick` returns
pub enum DecayStep {
    /// Still whole: `onColonyTick` answered true with nothing to change.
    Waiting,
    /// The timer ran out the first time: the block turns into the decayed
    /// variant and a fresh 12 000 ticks start (`:236-240`).
    Decayed,
    /// The timer ran out again, the inventory drops and the block becomes air
    /// (`:241-247`).
    Removed,
}

/// A grave's rot, as the tile entity keeps it.
///
/// `decay_timer` is a field with a sentinel: **`-1` disables decay entirely**
/// (`TileEntityGrave.java:230`), which nothing in this tree sets but a save can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Decay {
    pub timer: i32,
    pub decayed: bool,
}

impl Default for Decay {
    fn default() -> Decay {
        Decay::new()
    }
}

impl Decay {
    /// A grave just built (`AbstractTileEntityGrave.java:43-44`).
    pub fn new() -> Decay {
        Decay {
            timer: DEFAULT_DECAY_TIMER,
            decayed: false,
        }
    }

    /// `delayDecayTimer` (`:51-54`): minutes of research bonus, added. Java's
    /// `itemHandler` of this is `minutes * TICKS_SECOND * 60`.
    pub fn delay(&mut self, minutes: f64) {
        self.timer += (minutes * 20.0 * 60.0) as i32;
    }

    /// `onColonyTick` (`TileEntityGrave.java:228-251`), one call every
    /// [`COLONY_TICK_DELAY`] ticks.
    pub fn tick(&mut self, delay: i32) -> DecayStep {
        if self.timer == -1 {
            return DecayStep::Waiting;
        }
        self.timer -= delay;
        if self.timer > 0 {
            return DecayStep::Waiting;
        }
        if !self.decayed {
            self.decayed = true;
            self.timer = DEFAULT_DECAY_TIMER;
            DecayStep::Decayed
        } else {
            DecayStep::Removed
        }
    }
}

/// What happened to a position during a colony tick (`:111-132`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: kept or removed, and why
pub enum Ticked {
    /// Loaded, a grave, and its rot did not finish: it stays.
    Kept { at: [i32; 3] },
    /// The block is no longer a grave, or the rot finished.
    Removed { at: [i32; 3], reason: TickedRemoval },
}

/// Why a grave left the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two `iterator.remove()` sites of `onColonyTick`
pub enum TickedRemoval {
    /// `:120-125`: the block there is loaded and is not a `TileEntityGrave`.
    NotAGrave,
    /// `:127-131`: `onColonyTick` answered false, the grave has rotted away.
    Rotted,
}

/// Why a reservation was refused (`:191-201`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two refusals of `reserveGrave`
pub enum Refusal {
    /// `!graves.containsKey(pos)`.
    Unknown { at: [i32; 3] },
    /// `graves.get(pos)` is already true.
    AlreadyReserved { at: [i32; 3] },
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::Unknown { .. } => "unknown_grave",
            Refusal::AlreadyReserved { .. } => "already_reserved",
        }
    }
}

/// What `addNewGrave` answered (`:154-170`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of `addNewGrave`
pub enum Added {
    /// `:162-168`: put at `false`, and the colony is saved.
    Yes,
    /// `:162-165`: already there, and Java still answers **true** without a
    /// write.
    AlreadyThere,
    /// `:156-160`: no grave block entity at that position, so nothing is
    /// recorded.
    NoEntity,
}

/// One line of what building a grave did, in Java's order (`:294-323`).
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: the five writes `createCitizenGrave` performs, in order
pub enum Deposit {
    /// `:294`: the grave block is placed at the found position.
    BlockPlaced { at: [i32; 3] },
    /// `:297`: the whole inventory fitted in the grave.
    InventoryStored,
    /// `:299`: it did not, and **everything** is dropped at the death position.
    InventoryDropped { at: [i32; 3] },
    /// `:304`: this armour slot fitted.
    ArmourStored { slot: u8 },
    /// `:306`: this one did not, and is spawned at the death position.
    ArmourSpawned { slot: u8 },
    /// `:311`: the research bonus delayed the rot.
    DecayDelayed { minutes: f64 },
    /// `:321`: the grave knows whose it is.
    DataSet,
    /// `:323`: and only now is it in the colony's map.
    Registered { at: [i32; 3] },
}

/// A citizen's load, as the grave sees it: how many stacks the inventory holds,
/// and which armour pieces are worn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Load {
    /// `citizenData.getInventory()`'s stacks.
    pub stacks: u32,
    /// The armour pieces, one entry per worn slot (`:301-308`).
    pub armour: Vec<Armour>,
}

/// One worn piece (`:303`, `:304`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a slot and whether it fitted
pub struct Armour {
    /// `EquipmentSlot.ordinal()`, `0..EQUIPMENT_SLOTS`.
    pub slot: u8,
    /// Whether the grave's handler took this piece.
    pub fits: bool,
}

/// The colony's graves: `BlockPos → reserved` (`:47`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Graves {
    taken: BTreeMap<[i32; 3], bool>,
    writes: u32,
}

impl Graves {
    pub fn new() -> Graves {
        Graves::default()
    }

    /// How many times Java would have marked the colony dirty.
    pub fn writes(&self) -> u32 {
        self.writes
    }

    pub fn len(&self) -> usize {
        self.taken.len()
    }

    pub fn is_empty(&self) -> bool {
        self.taken.is_empty()
    }

    /// The positions, ascending — Java walks a `HashMap` (`:111`), and the port
    /// orders so that two runs of the same save rot the graves in the same order.
    pub fn positions(&self) -> Vec<[i32; 3]> {
        self.taken.keys().copied().collect()
    }

    /// `getGraves` (`:142-145`), read-only.
    pub fn is_reserved(&self, at: [i32; 3]) -> Option<bool> {
        self.taken.get(&at).copied()
    }

    /// `read` (`:70-82`): an entry **must carry both keys** (`:77`), or it is
    /// dropped without a word — a grave with no `reserved` flag is not a grave
    /// with a default, it is not there.
    ///
    /// The map is cleared first (`:72`), so a reload forgets the previous state.
    /// The compound is the inner `graveManager` one (`Colony.java:778`), which is
    /// the same level [`Self::write`] fills.
    pub fn read(&mut self, data: &Compound) {
        self.taken.clear();
        for record in nbt::compounds(data, KEY_GRAVE) {
            if !nbt::holds(record, KEY_POS) || !nbt::holds(record, KEY_RESERVED) {
                continue;
            }
            let Some(at) = nbt::position_in(record, KEY_POS) else {
                continue;
            };
            self.taken.insert(at, nbt::boolean(record, KEY_RESERVED));
        }
    }

    /// `write` (`:90-101`): one record per grave, `pos` then `reserved`.
    ///
    /// The compound it fills is the one **the caller nests under
    /// `graveManager`** (`Colony.java:944-945`), which is why this writes the list
    /// and not the wrapper: a port that wrote its own wrapper would nest it twice,
    /// the mistake `src/manager_03/animal.rs` documents for
    /// `AnimalManager.read`.
    pub fn write(&self, data: &mut Compound) {
        let records: Vec<Compound> = self
            .taken
            .iter()
            .map(|(at, reserved)| {
                Compound::from([
                    (KEY_POS.into(), nbt::position(*at)),
                    (KEY_RESERVED.into(), Tag::Byte(if *reserved { 1 } else { 0 })),
                ])
            })
            .collect();
        data.insert(KEY_GRAVE.into(), nbt::compound_list(records));
    }

    /// `addNewGrave` (`:154-170`): no block entity, nothing recorded; already
    /// there, `true` and no write; otherwise a free grave and a save.
    pub fn add(&mut self, at: [i32; 3], entity_present: bool) -> Added {
        if !entity_present {
            return Added::NoEntity;
        }
        if self.taken.contains_key(&at) {
            return Added::AlreadyThere;
        }
        self.taken.insert(at, false);
        self.writes += 1;
        Added::Yes
    }

    /// `removeGrave` (`:177-182`): **unconditional**, and it always saves, even
    /// when there was nothing to remove.
    pub fn remove(&mut self, at: [i32; 3]) -> bool {
        let present = self.taken.remove(&at).is_some();
        self.writes += 1;
        present
    }

    /// `reserveGrave` (`:191-201`): both refusals come before the write.
    pub fn reserve(&mut self, at: [i32; 3]) -> Result<(), Refusal> {
        match self.taken.get(&at) {
            None => Err(Refusal::Unknown { at }),
            Some(true) => Err(Refusal::AlreadyReserved { at }),
            Some(false) => {
                self.taken.insert(at, true);
                self.writes += 1;
                Ok(())
            }
        }
    }

    /// `unReserveGrave` (`:204-211`): a silent no-op on an unknown or a free
    /// grave, and no save when it does nothing.
    pub fn unreserve(&mut self, at: [i32; 3]) -> bool {
        if self.taken.get(&at).copied() == Some(true) {
            self.taken.insert(at, false);
            self.writes += 1;
            true
        } else {
            false
        }
    }

    /// `reserveNextFreeGrave` (`:219-242`): the first grave that is there, loaded
    /// and free. A grave whose block is gone is dropped from the map **without a
    /// save** (`:231`) — asymmetry 5 of the module note.
    pub fn reserve_next_free(&mut self, probe: impl Fn([i32; 3]) -> Slot) -> Option<[i32; 3]> {
        let candidates = self.positions();
        for at in candidates {
            match probe(at) {
                Slot::Unloaded => continue,
                Slot::NotAGrave => {
                    self.taken.remove(&at);
                    continue;
                }
                Slot::Grave => {
                    if self.reserve(at).is_ok() {
                        return Some(at);
                    }
                }
            }
        }
        None
    }

    /// `onColonyTick` (`:109-133`), one call per colony tick.
    ///
    /// An unloaded position is skipped and **kept**; a loaded position whose block
    /// is not a grave is dropped and saved; otherwise the block entity is asked to
    /// rot, and a finished grave is dropped and saved.
    pub fn tick(
        &mut self,
        probe: impl Fn([i32; 3]) -> Slot,
        rot: impl FnMut([i32; 3]) -> DecayStep,
    ) -> Vec<Ticked> {
        let mut rot = rot;
        let mut steps = Vec::new();
        for at in self.positions() {
            match probe(at) {
                Slot::Unloaded => continue,
                Slot::NotAGrave => {
                    self.taken.remove(&at);
                    self.writes += 1;
                    steps.push(Ticked::Removed {
                        at,
                        reason: TickedRemoval::NotAGrave,
                    });
                }
                Slot::Grave => {
                    if rot(at) == DecayStep::Removed {
                        self.taken.remove(&at);
                        self.writes += 1;
                        steps.push(Ticked::Removed {
                            at,
                            reason: TickedRemoval::Rotted,
                        });
                    } else {
                        steps.push(Ticked::Kept { at });
                    }
                }
            }
        }
        steps
    }

    /// `createCitizenGrave` (`:254-331`) from the placement on: the block, the
    /// inventory, the armour, the delay, the data, and the registration — in that
    /// order, which is the order of the [`Deposit`] log.
    ///
    /// `takes_inventory` is the grave's handler answering
    /// `transferAllItemHandler` (`:297`); `minutes` is the research bonus
    /// (`:311`).
    pub fn fill(
        &mut self,
        at: [i32; 3],
        site: [i32; 3],
        load: &Load,
        takes_inventory: bool,
        minutes: f64,
        entity_present: bool,
    ) -> (Vec<Deposit>, Added) {
        let mut log = vec![Deposit::BlockPlaced { at: site }];
        if takes_inventory {
            log.push(Deposit::InventoryStored);
        } else {
            log.push(Deposit::InventoryDropped { at });
        }
        for piece in &load.armour {
            if piece.fits {
                log.push(Deposit::ArmourStored { slot: piece.slot });
            } else {
                log.push(Deposit::ArmourSpawned { slot: piece.slot });
            }
        }
        log.push(Deposit::DecayDelayed { minutes });
        log.push(Deposit::DataSet);
        let added = self.add(site, entity_present);
        if added == Added::Yes {
            log.push(Deposit::Registered { at: site });
        }
        (log, added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dry(_: [i32; 3]) -> bool {
        true
    }

    fn wall(_: [i32; 3]) -> bool {
        false
    }

    fn anywhere(_: i32) -> bool {
        true
    }

    #[test]
    fn a_grave_rots_after_twelve_thousand_ticks_and_then_leaves() {
        let mut decay = Decay::new();
        assert_eq!(decay.timer, 12_000, "AbstractTileEntityGrave.java:22");
        assert_eq!(decay.tick(500), DecayStep::Waiting);
        decay.timer = 400;
        assert_eq!(decay.tick(500), DecayStep::Decayed, ":236-240");
        assert!(decay.decayed);
        assert_eq!(decay.timer, 12_000, "the first rot restarts the timer");
        decay.timer = 100;
        assert_eq!(decay.tick(500), DecayStep::Removed, ":241-247");
    }

    #[test]
    fn a_timer_of_minus_one_never_rots_and_a_research_bonus_only_adds() {
        let mut decay = Decay {
            timer: -1,
            decayed: false,
        };
        assert_eq!(decay.tick(500), DecayStep::Waiting, ":230 disables the whole branch");

        let mut delayed = Decay::new();
        delayed.delay(2.5);
        assert_eq!(delayed.timer, 12_000 + 3_000, ":51-54 adds minutes * 20 * 60");
    }

    #[test]
    fn lava_refuses_first_and_water_demands_air_above_before_it_searches() {
        assert_eq!(
            find_site([0, 60, 0], Medium::Lava, dry, dry, anywhere),
            Err(SiteRefusal::Lava),
            ":257-261 returns before the search"
        );
        assert_eq!(
            find_site([0, 60, 0], Medium::Water, dry, wall, anywhere),
            Err(SiteRefusal::Water),
            ":266-282 finds no air in the ten blocks above and never searches"
        );
        assert_eq!(
            find_site([0, 60, 0], Medium::Dry, wall, wall, anywhere),
            Err(SiteRefusal::NoGround),
            ":285-289 with nothing free"
        );
        assert_eq!(SiteRefusal::Lava.warning(), Some(WARNING_LAVA));
        assert_eq!(SiteRefusal::Water.warning(), Some(WARNING_WATER));
        assert_eq!(SiteRefusal::NoGround.warning(), None, ":328 has no message");
    }

    #[test]
    fn the_search_starts_at_the_death_position_then_its_four_neighbours() {
        assert_eq!(
            find_site([4, 60, 4], Medium::Dry, dry, dry, anywhere),
            Ok([4, 60, 4]),
            ":1047 the start is tested first"
        );
        let mut seen = Vec::new();
        let found = find_around([0, 0, 0], 1, 1, |cell| cell == [1, 0, 0], anywhere);
        assert_eq!(found, Some([1, 0, 0]), ":1052-1058 north first, then east");
        visited([0, 0, 0], 0, 1, |cell| seen.push(cell));
        assert_eq!(
            &seen[..13],
            &[
                [0, 0, 0],
                [0, 0, -1],
                [1, 0, 0],
                [0, 0, 1],
                [-1, 0, 0],
                [0, 0, -1],
                [1, 0, -1],
                [1, 0, 0],
                [1, 0, 1],
                [0, 0, 1],
                [-1, 0, 1],
                [-1, 0, 0],
                [-1, 0, -1],
            ],
            ":1052-1118, Direction.Plane.HORIZONTAL order then a ring"
        );
    }

    #[test]
    fn the_ring_is_the_structurize_square_and_the_height_ladder_climbs_then_descends() {
        let mut seen = Vec::new();
        visited([0, 0, 0], 2, 2, |cell| seen.push(cell));
        // After the start (1), the neighbours (4) and the ring of radius 1 (8),
        // the ring of radius 2 begins: `:1069-1118`.
        let ring_two: Vec<[i32; 3]> = seen
            .iter()
            .copied()
            .filter(|cell| cell[1] == 0)
            .skip(13)
            .take(12)
            .collect();
        assert_eq!(ring_two.len(), 12, "4 * (steps + 1) cells per ring");
        assert_eq!(ring_two[0], [-1, 0, -2], "one step east of the corner it starts on");
        assert_eq!(ring_two[11], [-2, 0, -2], "and it ends on that corner");
        assert!(
            !ring_two.contains(&[2, 0, 0]),
            "the square spans -steps..=+1, so x = 2 is never visited"
        );

        let levels: Vec<i32> = {
            let mut unique = Vec::new();
            for cell in &seen {
                if !unique.contains(&cell[1]) {
                    unique.push(cell[1]);
                }
            }
            unique
        };
        assert_eq!(
            levels,
            vec![0, 1, -1, 2],
            "vertical + 2 levels of 0, +1, -1, +2 at :1064-1119"
        );
    }

    #[test]
    fn a_search_that_leaves_the_world_stops_instead_of_skipping_the_height() {
        let mut seen = Vec::new();
        let found = find_around(
            [0, 300, 0],
            8,
            1,
            |cell| {
                seen.push(cell);
                false
            },
            |y| y <= 301,
        );
        assert_eq!(found, None);
        let levels: Vec<i32> = {
            let mut unique = Vec::new();
            for cell in &seen {
                if !unique.contains(&cell[1]) {
                    unique.push(cell[1]);
                }
            }
            unique
        };
        assert_eq!(
            levels,
            vec![300, 301, 299],
            ":1121-1124 returns the moment the next height is out of the world"
        );
    }

    #[test]
    fn an_entry_without_pos_or_reserved_is_not_a_grave() {
        let mut graves = Graves::new();
        let data = Compound::from([(
            KEY_GRAVE.into(),
            nbt::compound_list(vec![
                Compound::from([(KEY_POS.into(), nbt::position([1, 2, 3]))]),
                Compound::from([(KEY_RESERVED.into(), Tag::Byte(1))]),
                Compound::from([
                    (KEY_POS.into(), nbt::position([4, 5, 6])),
                    (KEY_RESERVED.into(), Tag::Byte(1)),
                ]),
                Compound::from([
                    (KEY_POS.into(), nbt::position([7, 8, 9])),
                    (KEY_RESERVED.into(), Tag::Byte(0)),
                ]),
            ]),
        )]);
        graves.read(&data);
        assert_eq!(graves.positions(), vec![[4, 5, 6], [7, 8, 9]], ":77 needs both keys");
        assert_eq!(graves.is_reserved([4, 5, 6]), Some(true));
        assert_eq!(graves.is_reserved([7, 8, 9]), Some(false));
    }

    #[test]
    fn a_save_writes_pos_then_reserved_and_the_caller_nests_it() {
        let mut graves = Graves::new();
        assert_eq!(graves.add([1, 64, 2], true), Added::Yes);
        assert_eq!(graves.reserve([1, 64, 2]), Ok(()));

        let mut inner = Compound::new();
        graves.write(&mut inner);
        let mut colony = Compound::new();
        colony.insert(
            KEY_GRAVE_MANAGER.into(),
            Tag::Compound(inner.clone()),
        );

        let Some(Tag::Compound(manager)) = colony.get(&KEY_GRAVE_MANAGER.into()) else {
            panic!("Colony.java:945 nests the list under graveManager");
        };
        let records = nbt::compounds(manager, KEY_GRAVE);
        assert_eq!(records.len(), 1);
        assert!(nbt::holds(records[0], KEY_POS), ":96 writes the position first");
        assert!(nbt::boolean(records[0], KEY_RESERVED));
    }

    #[test]
    fn adding_a_grave_needs_a_block_entity_and_repeats_are_free() {
        let mut graves = Graves::new();
        assert_eq!(graves.add([0, 70, 0], false), Added::NoEntity, ":156-160");
        assert_eq!(graves.writes(), 0, "and nothing is saved");
        assert_eq!(graves.add([0, 70, 0], true), Added::Yes);
        assert_eq!(graves.writes(), 1);
        assert_eq!(graves.add([0, 70, 0], true), Added::AlreadyThere, ":162-165");
        assert_eq!(graves.writes(), 1, "a repeat writes nothing");
        assert_eq!(graves.is_reserved([0, 70, 0]), Some(false));
    }

    #[test]
    fn both_reservations_refuse_before_they_write() {
        let mut graves = Graves::new();
        assert_eq!(
            graves.reserve([0, 70, 0]),
            Err(Refusal::Unknown { at: [0, 70, 0] }),
            ":193-196"
        );
        assert_eq!(graves.writes(), 0, "no save on a refusal");
        let _ = graves.add([0, 70, 0], true);
        let after_add = graves.writes();
        assert_eq!(graves.reserve([0, 70, 0]), Ok(()));
        assert_eq!(graves.writes(), after_add + 1);
        assert_eq!(
            graves.reserve([0, 70, 0]),
            Err(Refusal::AlreadyReserved { at: [0, 70, 0] })
        );
        assert_eq!(graves.writes(), after_add + 1, "and no save on the second refusal");
        assert!(graves.unreserve([0, 70, 0]));
        assert_eq!(graves.is_reserved([0, 70, 0]), Some(false));
        assert!(!graves.unreserve([0, 70, 0]), ":206-210 is a silent no-op");
    }

    #[test]
    fn the_next_free_grave_skips_the_unloaded_and_forgets_the_broken() {
        let mut graves = Graves::new();
        for at in [[0, 70, 0], [16, 70, 0], [32, 70, 0]] {
            let _ = graves.add(at, true);
        }
        let found = graves.reserve_next_free(|at| match at {
            [0, 70, 0] => Slot::Unloaded,
            [16, 70, 0] => Slot::NotAGrave,
            _ => Slot::Grave,
        });
        assert_eq!(found, Some([32, 70, 0]), ":223-241 walks the copy in order");
        assert_eq!(graves.is_reserved([0, 70, 0]), Some(false), "the unloaded one is kept");
        assert_eq!(
            graves.is_reserved([16, 70, 0]),
            None,
            ":231 removes the broken grave, and only one write is counted"
        );
        assert_eq!(graves.writes(), 4, "three adds and one reservation: the removal is not saved");
    }

    #[test]
    fn a_colony_tick_keeps_the_unloaded_drops_the_broken_and_rots_the_rest() {
        let mut graves = Graves::new();
        for at in [[0, 70, 0], [16, 70, 0], [32, 70, 0]] {
            let _ = graves.add(at, true);
        }
        let mut rotted = Vec::new();
        let steps = graves.tick(
            |at| match at {
                [0, 70, 0] => Slot::Unloaded,
                [16, 70, 0] => Slot::NotAGrave,
                _ => Slot::Grave,
            },
            |at| {
                rotted.push(at);
                DecayStep::Removed
            },
        );
        assert_eq!(
            steps,
            vec![
                Ticked::Removed { at: [16, 70, 0], reason: TickedRemoval::NotAGrave },
                Ticked::Removed { at: [32, 70, 0], reason: TickedRemoval::Rotted },
            ],
            ":114-131"
        );
        assert_eq!(rotted, vec![[32, 70, 0]], "an unloaded grave is never asked");
        assert_eq!(graves.positions(), vec![[0, 70, 0]]);
        assert_eq!(graves.writes(), 5, "three adds and two removals, both saved");
    }

    #[test]
    fn a_death_fills_the_grave_in_java_s_order_and_registers_it_last() {
        let mut graves = Graves::new();
        let load = Load {
            stacks: 12,
            armour: vec![
                Armour { slot: 0, fits: false },
                Armour { slot: 2, fits: true },
            ],
        };
        let (log, added) = graves.fill([0, 60, 0], [0, 59, 0], &load, false, 1.5, true);
        assert_eq!(
            log,
            vec![
                Deposit::BlockPlaced { at: [0, 59, 0] },
                Deposit::InventoryDropped { at: [0, 60, 0] },
                Deposit::ArmourSpawned { slot: 0 },
                Deposit::ArmourStored { slot: 2 },
                Deposit::DecayDelayed { minutes: 1.5 },
                Deposit::DataSet,
                Deposit::Registered { at: [0, 59, 0] },
            ],
            ":294, :297-300, :301-308, :311, :321, :323"
        );
        assert_eq!(added, Added::Yes);
        assert_eq!(graves.is_reserved([0, 59, 0]), Some(false));
    }

    #[test]
    fn a_grave_that_cannot_be_registered_is_not_reported_as_registered() {
        let mut graves = Graves::new();
        let (log, added) = graves.fill([0, 60, 0], [0, 59, 0], &Load::default(), true, 0.0, false);
        assert_eq!(added, Added::NoEntity);
        assert!(
            !log.contains(&Deposit::Registered { at: [0, 59, 0] }),
            ":323 is the last line, and it did not happen"
        );
        assert!(log.contains(&Deposit::InventoryStored), "the items went in all the same");
        assert_eq!(EQUIPMENT_SLOTS, 7, "EquipmentSlot.values().length in 1.20.1");
    }
}
