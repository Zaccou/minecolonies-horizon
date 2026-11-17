//! The colony's buildings, and the five indexes it keeps over them.
//!
//! Java reference: `core/colony/managers/RegisteredStructureManager.java` (919
//! lines), `api/colony/managers/interfaces/IRegisteredStructureManager.java`, the
//! chunk-aligned box of `api/util/BlockPosUtil.java:591-600` and the keys of
//! `NbtTagConstants.java:13-14`, `:44`, `:46`, `:51`.
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | may this building be added | not at a position already taken, and **one** town hall | `:556-559`, `:790-793` |
//! | what a removal takes down | the map entry **and** the town-hall slot, the warehouse list, the mystical-site list | `:639-676` |
//! | how wide the colony is | the chunk bounds, grown by one chunk a side of every building, **never shrunk** | `:128-151`, `:225-247` |
//! | which warehouse serves a position | the nearest with a level above zero and a block entity, ties to the first seen | `:471-492` |
//! | how much prestige | the sum of the buildings' | `:290-300` |
//! | which house can take a citizen | the first with a living module, unlocked, and a bed free | `:495-517` |
//! | what a guard tower covers | the chunk box of its claim radius, tested on the target's **exact** position | `:684-712` |
//! | which leisure sites there are | a list with no duplicates | `:428-452` |
//!
//! **What was already covered, and is therefore not duplicated here.**
//! [`crate::building::Building`] is one building — its level, its stage, its
//! modules, its compound. [`crate::manager_02::from_record`] is the registry
//! lookup a saved `type` needs. [`crate::building::behaviour::claim_radius`] and
//! [`crate::building::behaviour::claim_box`] are the claim arithmetic, with the
//! six overrides. [`crate::manager_02::Extension`] is one building extension with
//! its payload. [`crate::colony::Colonie`] carries a *flat* `buildings: Vec` for
//! the colony's own record. What none of them has is **the collection**: the
//! uniqueness rules, the four derived lists, the chunk bounds, the prestige queue
//! and the extension index. That is this file, and it calls the five above rather
//! than re-reading a compound.
//!
//! **Three rules a tidy port would lose.**
//!
//! 1. **The chunk bounds are never shrunk.** `setMaxChunk` runs on add and on
//!    read only (`:174`, `:662`), and it *expands by one chunk past the building*
//!    (`:230-247`): a hut in the centre chunk alone moves both bounds to
//!    `centre ± 1`. Remove every building and the colony still reports that box.
//! 2. **`removeBuilding` runs its teardown whether or not the map held it.** `:601`
//!    guards only the map edit and the packets; the town-hall slot, the two lists,
//!    the citizens, the requests and the event bus run regardless (`:620-676`).
//!    The port returns a [`Removal`] that says whether the map held it, because a
//!    caller that skipped the rest would keep a town hall standing in a hole.
//! 3. **The town hall is the *first* one added, and a second is still a
//!    building.** `:790-793` only assigns when the slot is empty, so a colony with
//!    two town halls answers the first for `getTownHall()` and lists both in
//!    `getBuildings()`.
//!
//! **Two things deliberately not ported**, each with its cost in `MANQUANT.md`:
//! the random leisure ladder of `getRandomLeisureSite` (`:344-408`, four coin
//! flips and a rain test whose every answer is "some position"), and the packet
//! half (`:748-786`), which a mod with no client has nothing to send to.
//!
//! **Pure data, pure rules**: a block-entity question, a prestige value and an
//! occupancy are closures the caller answers, and nothing here touches a host.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use crate::building::behaviour::{claim_box, claim_radius};
use crate::building::Kind;
use crate::building::{Building, Footprint};
use crate::manager_02::building_extension::Extension;
use crate::manager_02::{from_record, BuildingRefusal, ExtensionRefusal};

use super::nbt;

/// The compound the manager's own state is written into, at the colony level
/// (`Colony.java:931-932`, `NbtTagConstants.java:56`).
pub const KEY_BUILDING_MANAGER: &str = "buildingManager";
/// The buildings (`RegisteredStructureManager.java:165`, `:249`,
/// `NbtTagConstants.java:13`).
pub const KEY_BUILDINGS: &str = "buildings";
/// The building extensions, and the key they were saved under before the rename
/// (`:145`, `:257`, `NbtTagConstants.java:14`, `:44`) — **`fields` wins when both
/// are there** (`:139-146`).
pub const KEY_EXTENSIONS: &str = "building_extensions";
/// The pre-rename key, read first (`:139-146`).
pub const KEY_FIELDS: &str = "fields";
/// The leisure sites (`:179`, `:265`, `NbtTagConstants.java:46`).
pub const KEY_LEISURE: &str = "leisureSites";
/// One leisure site's position (`:183`, `:267`).
pub const KEY_POS: &str = "pos";

/// The module key a building that houses citizens carries. Java asks the
/// **class** (`hasModule(LivingBuildingModule.class)`, `:499`); the port asks the
/// key the module is saved under, which is the same fact once the compound is
/// read (`crate::building::Building::module`).
pub const LIVING_MODULE: &str = "living";

/// Why a building was not added (`:556-575`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two refusals of `addNewBuilding`
pub enum AddRefusal {
    /// `buildings.containsKey(tileEntity.getPosition())` (`:559`): the position
    /// already holds a building. Java answers `null`; the port names it.
    Occupied { at: [i32; 3] },
}

impl AddRefusal {
    pub fn name(&self) -> &'static str {
        match self {
            AddRefusal::Occupied { .. } => "position_taken",
        }
    }
}

/// What a removal found (`:601-676`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: whether the map held it, and what it was
pub struct Removal {
    /// Whether the map held a building there. Java's `:601` guards only the map
    /// edit and the packets with it.
    pub was_registered: bool,
    /// The kind, when the map held one — Java's `instanceof` ladder against the
    /// object it was handed.
    pub kind: Option<Kind>,
    /// The town-hall slot was cleared by this removal.
    pub cleared_town_hall: bool,
}

/// The chunk box the colony's buildings span (`:113-116`).
///
/// Java keeps four ints and expands them one chunk past every building
/// (`:225-247`). There is no shrink and no `contains`: `Colony.isCoordInColony`
/// asks the claims, not this (`Colony.java:1148-1170`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_z: i32,
    pub max_z: i32,
}

impl ChunkBounds {
    /// The bounds of a colony that has no building: the chunk of its town hall
    /// (`:132-135`).
    pub fn around(center: [i32; 3]) -> ChunkBounds {
        let chunk_x = center[0] >> 4;
        let chunk_z = center[2] >> 4;
        ChunkBounds {
            min_x: chunk_x,
            max_x: chunk_x,
            min_z: chunk_z,
            max_z: chunk_z,
        }
    }

    /// `setMaxChunk` (`:225-247`), kept **exactly**: a building in a chunk at or
    /// past the current bound pushes that bound **one chunk further**, which is
    /// why a single hut widens the box by a chunk a side.
    pub fn grow(&mut self, at: [i32; 3]) {
        let chunk_x = at[0] >> 4;
        let chunk_z = at[2] >> 4;
        if chunk_x >= self.max_x {
            self.max_x = chunk_x + 1;
        }
        if chunk_x <= self.min_x {
            self.min_x = chunk_x - 1;
        }
        if chunk_z >= self.max_z {
            self.max_z = chunk_z + 1;
        }
        if chunk_z <= self.min_z {
            self.min_z = chunk_z - 1;
        }
    }

    /// Whether a chunk is inside the box, **with no slack**: Java's
    /// `BoundingBox.isInside` is inclusive and exact.
    pub fn holds(&self, chunk_x: i32, chunk_z: i32) -> bool {
        chunk_x >= self.min_x
            && chunk_x <= self.max_x
            && chunk_z >= self.min_z
            && chunk_z <= self.max_z
    }
}

/// A building's bed situation, which Java reads off the living module
/// (`:505-513`) and the port cannot: the module has no occupancy in its compound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three facts `getHouseWithSpareBed` asks
pub struct Occupancy {
    /// `HiringMode.LOCKED.equals(module.getHiringMode())` (`:505-508`).
    pub locked: bool,
    /// `module.getAssignedCitizen().size()`.
    pub assigned: u32,
    /// `module.getModuleMax()`.
    pub capacity: u32,
}

impl Default for Occupancy {
    fn default() -> Occupancy {
        Occupancy {
            locked: false,
            assigned: 0,
            capacity: 0,
        }
    }
}

impl Occupancy {
    /// A bed is free when the module exists, is not locked and has room
    /// (`:509-513`).
    pub fn has_room(&self) -> bool {
        !self.locked && self.assigned < self.capacity
    }
}

/// What a read kept and what it dropped (`:128-212`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the kept registry and the two kinds of loss
pub struct Read {
    pub registry: Registry,
    /// The buildings whose `type` did not resolve (`:169-175`,
    /// `BuildingDataManager.java:35-39`). Java logs and keeps going.
    pub refused_buildings: Vec<BuildingRefusal>,
    /// The extensions that did not read (`:147-162`). Java catches the throw and
    /// logs it.
    pub refused_extensions: Vec<ExtensionRefusal>,
    /// The extensions whose owner is no longer a building, and whose owner was
    /// cleared (`:192-203`).
    pub orphaned_extensions: usize,
}

/// The colony's buildings and the indexes over them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Registry {
    buildings: BTreeMap<[i32; 3], Building>,
    town_hall: Option<[i32; 3]>,
    warehouses: Vec<[i32; 3]>,
    mystical_sites: Vec<[i32; 3]>,
    leisure_sites: Vec<[i32; 3]>,
    extensions: BTreeMap<([i32; 3], &'static str), Extension>,
    bounds: Option<ChunkBounds>,
    prestige_queue: Vec<[i32; 3]>,
    writes: u32,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// How many times Java would have marked the colony dirty.
    pub fn writes(&self) -> u32 {
        self.writes
    }

    pub fn len(&self) -> usize {
        self.buildings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buildings.is_empty()
    }

    /// `getBuildings` (`:519-523`), read-only, ascending by position.
    pub fn get(&self, at: [i32; 3]) -> Option<&Building> {
        self.buildings.get(&at)
    }

    pub fn positions(&self) -> Vec<[i32; 3]> {
        self.buildings.keys().copied().collect()
    }

    /// `getTownHall` (`:521-526`), which is the **first** town hall added.
    pub fn town_hall(&self) -> Option<[i32; 3]> {
        self.town_hall
    }

    /// `getWareHouses` (`:733-736`) and `getMysticalSites` (`:744-748`), in the
    /// order they were added — Java's `ArrayList`, not a set, so a position can
    /// be listed twice if it was added twice. The map refuses the second add
    /// before that can happen (`:556-559`).
    pub fn warehouses(&self) -> &[[i32; 3]] {
        &self.warehouses
    }

    pub fn mystical_sites(&self) -> &[[i32; 3]] {
        &self.mystical_sites
    }

    pub fn leisure_sites(&self) -> &[[i32; 3]] {
        &self.leisure_sites
    }

    /// The chunk bounds, `None` before the first read or the first building —
    /// Java initialises them from the colony's centre in `read` (`:132-135`).
    pub fn bounds(&self) -> Option<ChunkBounds> {
        self.bounds
    }

    /// The colony's centre, which is where the bounds start (`:132-135`).
    pub fn start_bounds(&mut self, center: [i32; 3]) {
        self.bounds = Some(ChunkBounds::around(center));
    }

    /// `addNewBuilding`'s decision (`:556-575`) and `addBuilding`'s bookkeeping
    /// (`:786-803`).
    ///
    /// The refusal comes before anything: an occupied position leaves the map,
    /// the three lists, the bounds and the save untouched.
    ///
    /// **One port choice**: Java always has bounds, because `read` fills them from
    /// the colony's centre (`:132-135`). A registry built without
    /// [`Self::start_bounds`] takes the first building's own chunk as the seed
    /// instead of answering "no bounds", so the two paths agree after one add.
    pub fn add(&mut self, building: Building) -> Result<(), AddRefusal> {
        let at = building.at();
        if self.buildings.contains_key(&at) {
            return Err(AddRefusal::Occupied { at });
        }
        let kind = building.kind();
        self.buildings.insert(at, building);
        if kind.is_town_hall() && self.town_hall.is_none() {
            self.town_hall = Some(at);
        }
        if kind == Kind::Warehouse {
            self.warehouses.push(at);
        } else if kind == Kind::MysticalSite {
            self.mystical_sites.push(at);
        }
        match self.bounds {
            Some(mut bounds) => {
                bounds.grow(at);
                self.bounds = Some(bounds);
            }
            None => {
                let mut bounds = ChunkBounds::around(at);
                bounds.grow(at);
                self.bounds = Some(bounds);
            }
        }
        self.writes += 1;
        Ok(())
    }

    /// `removeBuilding` (`:601-676`), the part a manager owns.
    ///
    /// The map edit is conditional; **everything else is not** (`:620-632`): a
    /// town-hall removal clears the slot even if the map never held it, which a
    /// caller that only looked at `was_registered` would get wrong.
    pub fn remove(&mut self, at: [i32; 3]) -> Removal {
        let kind = self.buildings.remove(&at).map(|building| building.kind());
        let was_registered = kind.is_some();
        let mut cleared_town_hall = false;
        match kind {
            Some(found) if found.is_town_hall() => {
                self.town_hall = None;
                cleared_town_hall = true;
            }
            Some(Kind::Warehouse) => self.warehouses.retain(|held| *held != at),
            Some(Kind::MysticalSite) => self.mystical_sites.retain(|held| *held != at),
            _ => {}
        }
        if was_registered {
            self.writes += 1;
        }
        Removal {
            was_registered,
            kind,
            cleared_town_hall,
        }
    }

    /// `getClosestWarehouseInColony` (`:471-492`).
    ///
    /// Two conditions and one tie: the level must be above zero, the block entity
    /// must be there, and the **strict** `<` means the first of two equidistant
    /// warehouses wins. Distance is `distSqr`, not a square root.
    pub fn closest_warehouse(
        &self,
        at: [i32; 3],
        has_entity: impl Fn([i32; 3]) -> bool,
    ) -> Option<[i32; 3]> {
        let mut chosen: Option<[i32; 3]> = None;
        let mut distance = 0.0;
        for held in &self.warehouses {
            let Some(building) = self.buildings.get(held) else {
                continue;
            };
            if building.level() > 0 && has_entity(*held) {
                let candidate = squared_distance(*held, at);
                if chosen.is_none() || candidate < distance {
                    distance = candidate;
                    chosen = Some(*held);
                }
            }
        }
        chosen
    }

    /// `getColonyPrestige` (`:290-300`): the sum, over every building.
    pub fn prestige(&self, prestige_of: impl Fn(&Building) -> i32) -> i32 {
        self.buildings.values().map(prestige_of).sum()
    }

    /// `onColonyTick`'s prestige half (`:271-283`).
    ///
    /// **The refill tick recalculates nothing**: the `if` fills the queue and the
    /// `else` does the work, so a colony with buildings recalculates one per tick
    /// only from the second tick on. Java shuffles the refill
    /// (`Collections.shuffle`, `:277`); the port keeps the position order and says
    /// so — the shuffle is a `MathUtils.RANDOM` draw with no rule behind it, and an
    /// order a test can read is worth more than a shuffle nothing observes.
    ///
    /// The position returned is the **tail** (`:281`), which is also the one
    /// [`Self::prestige_done`] clears: the queue empties from the end.
    pub fn prestige_tick(&mut self) -> Option<[i32; 3]> {
        if self.prestige_queue.is_empty() {
            self.prestige_queue = self.positions();
            None
        } else {
            self.prestige_queue.last().copied()
        }
    }

    /// `clearPendingPrestigeCalc` (`:285-288`).
    pub fn prestige_done(&mut self, at: [i32; 3]) -> bool {
        let before = self.prestige_queue.len();
        self.prestige_queue.retain(|held| *held != at);
        self.prestige_queue.len() != before
    }

    /// The position waiting for its prestige recalculation, if any.
    pub fn prestige_pending(&self) -> &[[i32; 3]] {
        &self.prestige_queue
    }

    /// `getHouseWithSpareBed` (`:495-517`): the first building in position order
    /// with a living module, an unlocked hiring mode and a free bed.
    pub fn house_with_spare_bed(
        &self,
        occupancy: impl Fn([i32; 3]) -> Occupancy,
    ) -> Option<[i32; 3]> {
        for (at, building) in &self.buildings {
            if building.module(LIVING_MODULE).is_none() {
                continue;
            }
            let beds = occupancy(*at);
            if beds.locked {
                continue;
            }
            if beds.assigned < beds.capacity {
                return Some(*at);
            }
        }
        None
    }

    /// `hasGuardBuildingNear` (`:684-712`), as the question it really is: which
    /// guard buildings cover `at`.
    ///
    /// The `guardedRegion` is `BlockPosUtil.getChunkAlignedBB(pos,
    /// getClaimRadius(level))` (`:701`, `BlockPosUtil.java:591-600`) and the test
    /// is `isInside` (`:701`), which is **exact** — unlike
    /// [`Footprint::contains`], whose one-block slack is for a hut sitting against
    /// its own wall (`AbstractSchematicProvider.java:518-520`). The port therefore
    /// compares the corners itself.
    ///
    /// Only a building with a level above zero guards (`:695`), and only a guard
    /// tower or a barracks — a barracks **tower** never claims anything, which is
    /// [`claim_radius`]'s own rule (`BuildingBarracksTower.java:94-97`).
    pub fn guard_cover(&self, at: [i32; 3]) -> Vec<[i32; 3]> {
        let mut covering = Vec::new();
        for (held, building) in &self.buildings {
            if building.level() <= 0 {
                continue;
            }
            let kind = building.kind();
            if kind != Kind::GuardTower && kind != Kind::Barracks && kind != Kind::GateHouse {
                continue;
            }
            let box_ = claim_box(*held, claim_radius(kind, building.level()));
            if inside(&box_, at) {
                covering.push(*held);
            }
        }
        covering
    }

    /// `guardBuildingChangedAt` (`:714-729`): every building the new claim radius
    /// covers, to be told its guard situation changed.
    ///
    /// Java takes `max(getBuildingLevel(), newLevel)` (`:716`), so an upgrade
    /// widens the box before the level is stored.
    pub fn covered_by_guard(&self, guard: [i32; 3], new_level: i32) -> Vec<[i32; 3]> {
        let Some(building) = self.buildings.get(&guard) else {
            return Vec::new();
        };
        let level = building.level().max(new_level);
        let box_ = claim_box(guard, claim_radius(building.kind(), level));
        self.buildings
            .keys()
            .copied()
            .filter(|at| inside(&box_, *at))
            .collect()
    }

    /// `addLeisureSite` (`:428-438`): no duplicates, and a save only when it
    /// changes something.
    pub fn add_leisure_site(&mut self, at: [i32; 3]) -> bool {
        if self.leisure_sites.contains(&at) {
            return false;
        }
        self.leisure_sites.push(at);
        self.writes += 1;
        true
    }

    /// `removeLeisureSite` (`:440-452`): a silent no-op when it was not there.
    pub fn remove_leisure_site(&mut self, at: [i32; 3]) -> bool {
        let before = self.leisure_sites.len();
        self.leisure_sites.retain(|held| *held != at);
        if self.leisure_sites.len() == before {
            return false;
        }
        self.writes += 1;
        true
    }

    /// `addBuildingExtension` (`:838-847`): `putIfAbsent` on the id, which is the
    /// position **and** the registry entry (`IBuildingExtension.ExtensionId`), so
    /// two fields on one position coexist and two of one kind do not.
    pub fn add_extension(&mut self, extension: Extension) -> bool {
        let key = (extension.at, extension.kind.path);
        if self.extensions.contains_key(&key) {
            return false;
        }
        self.extensions.insert(key, extension);
        self.writes += 1;
        true
    }

    /// `removeBuildingExtension` (`:849-857`): every extension matching the
    /// predicate, and a save even when none did — Java marks the extensions dirty
    /// outside its `removeIf`.
    pub fn remove_extensions(&mut self, matching: impl Fn(&Extension) -> bool) {
        self.extensions.retain(|_, extension| !matching(extension));
        self.writes += 1;
    }

    /// `getMatchingBuildingExtension` (`:831-836`), the id lookup: a position
    /// **and** a registry path, which is `IBuildingExtension.ExtensionId`.
    pub fn extension(&self, at: [i32; 3], path: &str) -> Option<&Extension> {
        for ((held, kind), extension) in &self.extensions {
            if *held == at && *kind == path {
                return Some(extension);
            }
        }
        None
    }

    /// The extensions, ascending by position then kind — Java's `HashMap` order
    /// (`:72`) replaced by one a save can reproduce.
    pub fn extensions(&self) -> impl Iterator<Item = &Extension> {
        self.extensions.values()
    }

    /// `write` (`:249-275`): `buildings`, `building_extensions`, `leisureSites`.
    ///
    /// **It writes `fields` never** — the rename went one way: the read accepts
    /// both keys and prefers `fields` (`:139-146`), the write only produces
    /// `building_extensions` (`:257`). A save round-trips because the read accepts
    /// the new key, not because the write is symmetric.
    pub fn write(&self) -> Compound {
        let buildings: Vec<Compound> = self
            .buildings
            .values()
            .map(|building| building.to_compound())
            .collect();
        let extensions: Vec<Compound> = self
            .extensions
            .values()
            .map(Extension::to_compound)
            .collect();
        let leisure: Vec<Compound> = self
            .leisure_sites
            .iter()
            .map(|at| Compound::from([(KEY_POS.into(), nbt::position(*at))]))
            .collect();
        Compound::from([
            (KEY_BUILDINGS.into(), nbt::compound_list(buildings)),
            (KEY_EXTENSIONS.into(), nbt::compound_list(extensions)),
            (KEY_LEISURE.into(), nbt::compound_list(leisure)),
        ])
    }

    /// `read` (`:128-212`).
    ///
    /// **The compound is the inner one**: Java calls
    /// `buildingManager.read(compound.getCompound(TAG_BUILDING_MANAGER))`
    /// (`Colony.java:931-932`), so `buildings`, `building_extensions` and
    /// `leisureSites` sit one level down — unlike the event manager, which reads
    /// the colony compound itself.
    ///
    /// The order is Java's: the bounds first, from the centre (`:132-135`), then
    /// extensions (`:137-162`), then buildings (`:164-175`) — each building grown
    /// into the bounds as it arrives (`:174`) — then leisure sites (`:177-190`),
    /// then the owner check over the extensions (`:192-211`), which needs the
    /// buildings to be loaded.
    ///
    /// A building whose `type` does not resolve costs **its own entry** and not
    /// the colony (`:169-175`); likewise a field that fails to parse (`:147-162`).
    pub fn read(data: &Compound, center: [i32; 3]) -> Read {
        let mut registry = Registry::new();
        registry.start_bounds(center);
        let mut refused_extensions = Vec::new();
        let mut refused_buildings = Vec::new();

        let extension_key = if nbt::holds(data, KEY_FIELDS) {
            KEY_FIELDS
        } else {
            KEY_EXTENSIONS
        };
        for record in nbt::compounds(data, extension_key) {
            match Extension::from_compound(record) {
                Ok(extension) => {
                    registry.add_extension(extension);
                }
                Err(refusal) => refused_extensions.push(refusal),
            }
        }

        for record in nbt::compounds(data, KEY_BUILDINGS) {
            match from_record(record) {
                Ok(building) => {
                    let _ = registry.add(building);
                }
                Err(refusal) => refused_buildings.push(refusal),
            }
        }

        let mut seen: Vec<[i32; 3]> = Vec::new();
        for record in nbt::compounds(data, KEY_LEISURE) {
            if let Some(at) = nbt::position_in(record, KEY_POS) {
                if !seen.contains(&at) {
                    seen.push(at);
                }
            }
        }
        registry.leisure_sites = seen;

        let mut orphaned = 0;
        let positions: Vec<[i32; 3]> = registry.buildings.keys().copied().collect();
        for extension in registry.extensions.values_mut() {
            let Some(owner) = extension.owner else {
                continue;
            };
            if !positions.contains(&owner) {
                extension.owner = None;
                orphaned += 1;
            }
        }

        Read {
            registry,
            refused_buildings,
            refused_extensions,
            orphaned_extensions: orphaned,
        }
    }
}

/// Whether a position is inside a box, corners included, with **no slack** —
/// `BoundingBox.isInside` (`RegisteredStructureManager.java:701`).
fn inside(box_: &Footprint, at: [i32; 3]) -> bool {
    let low = box_.low();
    let high = box_.high();
    (0..3).all(|axis| at[axis] >= low[axis] && at[axis] <= high[axis])
}

/// `BlockPos.distSqr` (`:479`).
fn squared_distance(a: [i32; 3], b: [i32; 3]) -> f64 {
    let mut total = 0.0;
    for axis in 0..3 {
        let delta = f64::from(a[axis] - b[axis]);
        total += delta * delta;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Module;

    fn hut(kind: Kind, at: [i32; 3], level: i32) -> Building {
        let mut building = Building::new(kind, at);
        let _ = building.set_level(level);
        let _ = building.complete(level);
        building
    }

    fn housing(kind: Kind, at: [i32; 3], level: i32) -> Building {
        let mut building = hut(kind, at, level);
        // The module level is not the bed count — the beds arrive through
        // `Occupancy` — but a module never outranks its building
        // (`src/building/instance.rs:280-286`).
        let module = Module::new(LIVING_MODULE, 1).expect("the living key and a level");
        let _ = building.attach_module(module);
        building
    }

    #[test]
    fn a_position_taken_is_refused_before_any_list_or_bound_moves() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::TownHall, [0, 64, 0], 1)), Ok(()));
        let bounds = registry.bounds();
        let writes = registry.writes();
        assert_eq!(
            registry.add(hut(Kind::Warehouse, [0, 64, 0], 1)),
            Err(AddRefusal::Occupied { at: [0, 64, 0] }),
            ":556-559"
        );
        assert_eq!(registry.len(), 1, "the map is untouched");
        assert!(registry.warehouses().is_empty(), "and so is the warehouse list");
        assert_eq!(registry.bounds(), bounds, "and the bounds");
        assert_eq!(registry.writes(), writes, "and nothing was saved");
        assert_eq!(AddRefusal::Occupied { at: [0, 0, 0] }.name(), "position_taken");
    }

    #[test]
    fn the_first_town_hall_answers_and_a_second_is_still_a_building() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::TownHall, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::TownHall, [64, 64, 0], 1)), Ok(()));
        assert_eq!(registry.town_hall(), Some([0, 64, 0]), ":790-793 keeps the first");
        assert_eq!(registry.len(), 2, "and the second is listed all the same");
    }

    #[test]
    fn the_warehouse_and_mystical_lists_follow_their_kinds() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::Warehouse, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::MysticalSite, [64, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Residence, [128, 64, 0], 1)), Ok(()));
        assert_eq!(registry.warehouses(), &[[0, 64, 0]]);
        assert_eq!(registry.mystical_sites(), &[[64, 64, 0]]);
    }

    #[test]
    fn removing_a_town_hall_clears_the_slot_even_when_the_map_never_held_it() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::TownHall, [0, 64, 0], 1)), Ok(()));

        let removal = registry.remove([0, 64, 0]);
        assert!(removal.was_registered);
        assert_eq!(removal.kind, Some(Kind::TownHall));
        assert!(removal.cleared_town_hall);
        assert_eq!(registry.town_hall(), None, ":620-624");

        let again = registry.remove([0, 64, 0]);
        assert!(!again.was_registered, "the map is empty there");
        assert_eq!(again.kind, None, "and there is no object to test");
        assert!(!again.cleared_town_hall);
    }

    #[test]
    fn removing_a_warehouse_unlists_it_and_leaves_the_next_level_alone() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::Warehouse, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Warehouse, [64, 64, 0], 1)), Ok(()));
        let removal = registry.remove([0, 64, 0]);
        assert_eq!(removal.kind, Some(Kind::Warehouse));
        assert_eq!(registry.warehouses(), &[[64, 64, 0]], ":625-628");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn the_chunk_bounds_grow_past_every_building_and_never_shrink() {
        let mut registry = Registry::new();
        registry.start_bounds([0, 64, 0]);
        assert_eq!(
            registry.bounds(),
            Some(ChunkBounds { min_x: 0, max_x: 0, min_z: 0, max_z: 0 }),
            ":132-135 starts on the colony's own chunk"
        );
        assert_eq!(registry.add(hut(Kind::Builder, [0, 64, 0], 1)), Ok(()));
        assert_eq!(
            registry.bounds(),
            Some(ChunkBounds { min_x: -1, max_x: 1, min_z: -1, max_z: 1 }),
            ":230-247 pushes one chunk past the building on every side it touches"
        );
        assert_eq!(registry.add(hut(Kind::Builder, [200, 64, 0], 1)), Ok(()));
        assert_eq!(
            registry.bounds(),
            Some(ChunkBounds { min_x: -1, max_x: 13, min_z: -1, max_z: 1 }),
            "x = 200 is chunk 12, and the bound goes to 13"
        );
        let _ = registry.remove([0, 64, 0]);
        let _ = registry.remove([200, 64, 0]);
        assert_eq!(
            registry.bounds(),
            Some(ChunkBounds { min_x: -1, max_x: 13, min_z: -1, max_z: 1 }),
            "removeBuilding never calls setMaxChunk: the box stays"
        );
        let bounds = registry.bounds().expect("bounds are set");
        assert!(bounds.holds(13, 1));
        assert!(!bounds.holds(14, 1));
    }

    #[test]
    fn the_closest_warehouse_needs_a_level_and_a_block_entity() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::Warehouse, [10, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Warehouse, [100, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Warehouse, [5, 64, 0], 0)), Ok(()));
        assert_eq!(
            registry.closest_warehouse([0, 64, 0], |_| true),
            Some([10, 64, 0]),
            ":471-492 picks the smallest squared distance"
        );
        assert_eq!(
            registry.closest_warehouse([0, 64, 0], |at| at != [10, 64, 0]),
            Some([100, 64, 0]),
            "a warehouse without a block entity is skipped"
        );
        assert_eq!(
            registry.closest_warehouse([0, 64, 0], |_| false),
            None,
            "the level-0 warehouse never counts"
        );
    }

    #[test]
    fn prestige_sums_and_its_queue_refills_on_one_tick_and_drains_from_the_tail() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::Builder, [0, 64, 0], 2)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Builder, [64, 64, 0], 3)), Ok(()));
        assert_eq!(registry.prestige(|building| building.level()), 5, ":290-300");

        assert_eq!(registry.prestige_tick(), None, ":273-278 the refill tick calculates nothing");
        assert_eq!(registry.prestige_pending().len(), 2);
        assert_eq!(
            registry.prestige_tick(),
            Some([64, 64, 0]),
            ":281 recalculates the tail"
        );
        assert!(registry.prestige_done([64, 64, 0]), ":285-288");
        assert_eq!(registry.prestige_tick(), Some([0, 64, 0]));
        assert!(registry.prestige_done([0, 64, 0]));
        assert_eq!(registry.prestige_tick(), None, "an empty queue refills again");
        assert_eq!(registry.prestige_pending().len(), 2);
    }

    #[test]
    fn a_house_with_a_living_module_and_free_beds_is_the_one_found() {
        let mut registry = Registry::new();
        assert_eq!(registry.add(hut(Kind::Residence, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(housing(Kind::Residence, [64, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(housing(Kind::Residence, [128, 64, 0], 1)), Ok(()));

        let found = registry.house_with_spare_bed(|at| match at {
            [64, 64, 0] => Occupancy { locked: false, assigned: 4, capacity: 4 },
            [128, 64, 0] => Occupancy { locked: true, assigned: 0, capacity: 2 },
            _ => Occupancy::default(),
        });
        assert_eq!(
            found, None,
            ":495-517 skips a hut with no module, a locked one and a full one"
        );

        let free = registry.house_with_spare_bed(|at| match at {
            [64, 64, 0] => Occupancy { locked: false, assigned: 3, capacity: 4 },
            [128, 64, 0] => Occupancy { locked: true, assigned: 0, capacity: 2 },
            _ => Occupancy::default(),
        });
        assert_eq!(free, Some([64, 64, 0]));
    }

    #[test]
    fn a_guard_covers_the_exact_chunk_box_of_its_claim_radius() {
        let mut registry = Registry::new();
        // A level-1 guard tower claims one chunk of radius
        // (`BuildingGuardTower.java:52-66`), and its box is chunk-aligned.
        assert_eq!(registry.add(hut(Kind::GuardTower, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Residence, [30, 64, 0], 1)), Ok(()));
        assert_eq!(registry.guard_cover([30, 64, 0]), vec![[0, 64, 0]], ":701");
        assert!(
            registry.guard_cover([48, 70, 0]).is_empty(),
            "x = 48 is one block past the box's far corner (0 + 2 * 16 + 15)"
        );

        let covered = registry.covered_by_guard([0, 64, 0], 1);
        assert_eq!(covered, vec![[0, 64, 0], [30, 64, 0]], ":714-729");
        let wider = registry.covered_by_guard([0, 64, 0], 5);
        assert!(wider.len() >= covered.len(), "a bigger level never covers less");
        assert!(registry.covered_by_guard([999, 64, 0], 5).is_empty(), "an unknown guard covers nothing");
    }

    #[test]
    fn leisure_sites_refuse_duplicates_and_the_save_follows_the_change() {
        let mut registry = Registry::new();
        assert!(registry.add_leisure_site([0, 64, 0]));
        let writes = registry.writes();
        assert!(!registry.add_leisure_site([0, 64, 0]), ":428-438");
        assert_eq!(registry.writes(), writes, "a duplicate saves nothing");
        assert!(!registry.remove_leisure_site([64, 64, 0]));
        assert_eq!(registry.writes(), writes, ":440-452");
        assert!(registry.remove_leisure_site([0, 64, 0]));
        assert!(registry.leisure_sites().is_empty());
    }

    #[test]
    fn a_saved_registry_round_trips_and_names_its_seven_keys() {
        let mut registry = Registry::new();
        registry.start_bounds([0, 64, 0]);
        assert_eq!(registry.add(hut(Kind::TownHall, [0, 64, 0], 1)), Ok(()));
        assert_eq!(registry.add(hut(Kind::Warehouse, [64, 64, 0], 1)), Ok(()));
        assert!(registry.add_leisure_site([8, 64, 8]));
        let saved = registry.write();
        assert!(nbt::holds(&saved, KEY_BUILDINGS), ":249");
        assert!(nbt::holds(&saved, KEY_EXTENSIONS), ":257 writes the new key");
        assert!(
            !nbt::holds(&saved, KEY_FIELDS),
            "the rename went one way: the write never produces the old key"
        );
        assert!(nbt::holds(&saved, KEY_LEISURE), ":265");

        let read = Registry::read(&saved, [0, 64, 0]);
        assert_eq!(read.registry.positions(), registry.positions());
        assert_eq!(read.registry.town_hall(), Some([0, 64, 0]));
        assert_eq!(read.registry.warehouses(), &[[64, 64, 0]]);
        assert_eq!(read.registry.leisure_sites(), &[[8, 64, 8]]);
        assert_eq!(read.refused_buildings, Vec::new());
    }

    #[test]
    fn a_building_of_an_unknown_kind_costs_its_own_entry_and_not_the_colony() {
        let mut good = hut(Kind::Builder, [0, 64, 0], 1);
        good.set_level(2).expect("a level inside the bounds");
        let mut bad = Compound::from([("location".into(), nbt::position([9, 64, 9]))]);
        bad.insert("type".into(), Tag::String("void:castle".into()));
        let saved = Compound::from([(
            KEY_BUILDINGS.into(),
            nbt::compound_list(vec![good.to_compound(), bad]),
        )]);

        let read = Registry::read(&saved, [0, 64, 0]);
        assert_eq!(read.registry.positions(), vec![[0, 64, 0]], ":169-175");
        assert_eq!(read.refused_buildings.len(), 1, "and the bad one is named");
    }

    #[test]
    fn a_leisure_list_with_a_repeated_position_keeps_one_entry() {
        let saved = Compound::from([(
            KEY_LEISURE.into(),
            nbt::compound_list(vec![
                Compound::from([(KEY_POS.into(), nbt::position([1, 64, 1]))]),
                Compound::from([(KEY_POS.into(), nbt::position([1, 64, 1]))]),
                Compound::from([(KEY_POS.into(), nbt::position([2, 64, 2]))]),
            ]),
        )]);
        let read = Registry::read(&saved, [0, 64, 0]);
        assert_eq!(read.registry.leisure_sites(), &[[1, 64, 1], [2, 64, 2]], ":177-190");
    }
}
