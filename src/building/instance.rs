//! One building: what it is, where it stands, what it has, who works there.
//!
//! Java reference: `core/colony/buildings/AbstractSchematicProvider.java` (the
//! level, the parent, the deconstructed flag) and
//! `core/colony/buildings/AbstractBuilding.java` (the built flag, the modules,
//! the compound). Both are one class in Java and one record here: the split is
//! about what this port can answer without a world, not about the game.
//!
//! **A building is its position.** It has no id of its own: `getID()` returns
//! `location` (`AbstractSchematicProvider.java:290-294`), and the colony's
//! manager keys its map on that position
//! (`core/colony/managers/RegisteredStructureManager.java:566`).
//!
//! **Two flags, one level, three stages.** Java keeps `buildingLevel`,
//! `isBuilt` and `isDeconstructed` apart: a hut placed but not built is level 0
//! (`AbstractSchematicProvider.java:64`), the builder sets `isBuilt` when it
//! finishes (`AbstractBuilding.java:951`), and a pickup sets `isDeconstructed`
//! (`AbstractSchematicProvider.java:142-145`). [`Stage`] is those last two
//! flags, named; the level stays a number, because 0 is a real level here and
//! every rule about the range lives in [`Building::set_level`].
//!
//! **The four rules this file enforces**, none of them invented:
//!
//! - the level stops at [`MAX_BUILDING_LEVEL`], and Java refuses it *silently*
//!   (`AbstractSchematicProvider.java:480-485`); the port names the refusal.
//! - a module is registered once, keyed by its serialisation key
//!   (`AbstractBuilding.java:210-219`), and neither is registered twice nor
//!   outranks its building (`Module::level`, a rule of this port: Java's modules
//!   have no level of their own, they read the building's).
//! - a worker arrives only once the hut is built and at level 1 or more
//!   (`api/colony/buildings/IBuilding.java:464-467`).
//! - one hut, one worker: Java puts the count on the module
//!   (`api/colony/buildings/modules/IAssignsCitizen.java:37-46`,
//!   `getModuleMax`/`isFull`), so a warehouse holds several couriers. This
//!   record holds one, and `RAPPORT_PORT_BUILDING.md` says what the rest costs.

use mod_api::{Compound, Tag};

use super::kind::Kind;
use super::nbt;
use super::{MAX_BUILDING_LEVEL, Refusal};

/// The building's own keys, with Java's names. `type` and `location` are what
/// the colony's building list carries (`api/util/constant/NbtTagConstants.java:140`,
/// `:145`); `level` is `TAG_SCHEMATIC_LEVEL` (`:150`, written at
/// `AbstractSchematicProvider.java:171`); `isBuilt` is written at
/// `AbstractBuilding.java:381`, `deconstructed` at
/// `AbstractSchematicProvider.java:182`, `parent` (`TAG_PARENT_SCHEM`) at `:184`,
/// and `building_modules` at `AbstractBuilding.java:393`.
pub const KEY_TYPE: &str = "type";
pub const KEY_AT: &str = "location";
pub const KEY_LEVEL: &str = "level";
pub const KEY_BUILT: &str = "isBuilt";
pub const KEY_DECONSTRUCTED: &str = "deconstructed";
pub const KEY_PARENT: &str = "parent";
pub const KEY_MODULES: &str = "building_modules";
/// The module's own level, inside the compound its key holds. Java's modules
/// save their own data there and none of them has a level; this one does.
pub const KEY_MODULE_LEVEL: &str = "level";
/// The citizen working here. **A port key**: Java has no such tag on the
/// building, the assignment lives on the citizen's job
/// (`api/colony/ICitizenData.java`, `getJob`).
pub const KEY_WORKER: &str = "worker";

/// Java's `BlockPos.ZERO` means "no parent": `hasParent()` is
/// `!parentSchematic.equals(BlockPos.ZERO)`
/// (`AbstractSchematicProvider.java:303-306`), and `BlockPosUtil.write` writes
/// three zeros for it.
pub const NO_PARENT: [i32; 3] = [0, 0, 0];

/// A module a building carries: an assignment, a work order view, a list of
/// settings — Java declares **157** of them in one table
/// (`core/colony/buildings/modules/BuildingModules.java`). The **key** is
/// Java's serialisation key (`api/colony/buildings/registry/BuildingEntry.java:204-220`),
/// never its runtime id, which is a counter that changes between builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    key: String,
    level: i32,
}

impl Module {
    /// `None` for an empty key: a module with no key could not be saved, and
    /// `from_compound` would drop it, so it is refused where it is built.
    pub fn new(key: &str, level: i32) -> Option<Module> {
        if key.is_empty() {
            return None;
        }
        Some(Module {
            key: key.to_owned(),
            level,
        })
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    fn to_compound(&self) -> Compound {
        Compound::from([(KEY_MODULE_LEVEL.into(), Tag::Int(self.level))])
    }
}

/// How far the building has got. Java's two booleans, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the port's own set, closed on purpose
pub enum Stage {
    /// Placed and nothing built: level 0, Java's "wood hut".
    Unbuilt,
    /// The builder finished the current level (`AbstractBuilding.java:951`).
    Built,
    /// The builder took the structure down, a pickup is pending
    /// (`AbstractSchematicProvider.java:142-145`). Wins over `Built`: both flags
    /// can be true at once, and a hole in the ground is the fact that matters.
    Deconstructed,
}

/// One building.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Building {
    kind: Kind,
    at: [i32; 3],
    level: i32,
    built: bool,
    deconstructed: bool,
    parent: Option<[i32; 3]>,
    /// Sorted by key, which is also the order the compound saves them in, so a
    /// round trip is exact. Java keeps the registration order instead
    /// (`AbstractBuilding.java:218`); nothing reads the order, and one canonical
    /// order is what makes `PartialEq` usable.
    modules: Vec<Module>,
    worker: Option<i32>,
}

impl Building {
    /// A hut just placed: level 0, not built, no module, no worker
    /// (`AbstractSchematicProvider.java:64`).
    pub fn new(kind: Kind, at: [i32; 3]) -> Building {
        Building {
            kind,
            at,
            level: 0,
            built: false,
            deconstructed: false,
            parent: None,
            modules: Vec::new(),
            worker: None,
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn at(&self) -> [i32; 3] {
        self.at
    }

    /// The identity every other part of the colony keys on: the position.
    pub fn id(&self) -> [i32; 3] {
        self.at
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn parent(&self) -> Option<[i32; 3]> {
        self.parent
    }

    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    pub fn module(&self, key: &str) -> Option<&Module> {
        let index = self.modules.binary_search_by(|module| module.key.as_str().cmp(key));
        index.ok().map(|index| &self.modules[index])
    }

    pub fn worker(&self) -> Option<i32> {
        self.worker
    }

    pub fn stage(&self) -> Stage {
        if self.deconstructed {
            Stage::Deconstructed
        } else if self.built {
            Stage::Built
        } else {
            Stage::Unbuilt
        }
    }

    /// May a citizen be given to this hut? `getBuildingLevel() > 0 && isBuilt()`
    /// (`api/colony/buildings/IBuilding.java:464-467`), quoted, not clamped.
    pub fn can_assign_citizens(&self) -> bool {
        self.level > 0 && self.built
    }

    /// Sets the level, in `0..=`[`MAX_BUILDING_LEVEL`], and clears the pickup
    /// flag — `setBuildingLevel` does exactly that after its guard
    /// (`AbstractSchematicProvider.java:480-490`).
    ///
    /// **One rule of this port's own**: lowering a building below a module it
    /// carries is refused, because the same file promises a module never
    /// outranks its building ([`Self::attach_module`]). Java has no such rule to
    /// break: its modules have no level.
    pub fn set_level(&mut self, level: i32) -> Result<(), Refusal> {
        if level > MAX_BUILDING_LEVEL {
            return Err(Refusal::LevelAboveMax { level });
        }
        if level < 0 {
            return Err(Refusal::LevelOutOfRange { level });
        }
        if let Some(module) = self.modules.iter().find(|module| module.level > level) {
            return Err(Refusal::LevelBelowModule {
                key: module.key.clone(),
                level: module.level,
                level_asked: level,
            });
        }
        self.level = level;
        self.deconstructed = false;
        Ok(())
    }

    /// The builder finished a level: the level is stored through the same
    /// guard, and `isBuilt` becomes true (`AbstractBuilding.java:940-951`).
    /// A finished build is level 1 or more, so 0 is refused.
    pub fn complete(&mut self, level: i32) -> Result<(), Refusal> {
        if level < super::LEVEL_MIN {
            return Err(Refusal::LevelOutOfRange { level });
        }
        self.set_level(level)?;
        self.built = true;
        Ok(())
    }

    /// The builder took the structure down. The level is kept: Java only sets
    /// the flag (`AbstractSchematicProvider.java:142-145`).
    pub fn deconstruct(&mut self) {
        self.deconstructed = true;
    }

    /// Nests this building in another one, or clears the parent with `None`.
    /// `BlockPos.ZERO` is not a position but the absence of one
    /// (`AbstractSchematicProvider.java:297-306`), so it clears too.
    ///
    /// Only the one check this record can make is made here: a building is not
    /// its own parent (`:317-321`). Whether the parent exists and has no parent
    /// of its own needs the set of standing buildings — that is
    /// [`super::Standing::check_placement`].
    pub fn set_parent(&mut self, at: Option<[i32; 3]>) -> Result<(), Refusal> {
        let at = at.filter(|at| *at != NO_PARENT);
        if at == Some(self.at) {
            return Err(Refusal::ParentSelf { at: self.at });
        }
        self.parent = at;
        Ok(())
    }

    /// Gives the building a module. Refused when the key is already taken
    /// (`AbstractBuilding.java:210-216`, which logs and returns) or when the
    /// module would outrank the building.
    pub fn attach_module(&mut self, module: Module) -> Result<(), Refusal> {
        if module.level < 0 {
            return Err(Refusal::LevelOutOfRange { level: module.level });
        }
        let index = self
            .modules
            .binary_search_by(|held| held.key.as_str().cmp(module.key.as_str()));
        if index.is_ok() {
            return Err(Refusal::ModuleDuplicate {
                key: module.key.clone(),
            });
        }
        if module.level > self.level {
            return Err(Refusal::ModuleAboveBuilding {
                key: module.key.clone(),
                level: module.level,
                building: self.level,
            });
        }
        self.modules.insert(index.unwrap_err(), module);
        Ok(())
    }

    /// Takes a module off and gives it back.
    pub fn detach_module(&mut self, key: &str) -> Result<Module, Refusal> {
        let index = self
            .modules
            .binary_search_by(|module| module.key.as_str().cmp(key))
            .map_err(|_| Refusal::ModuleUnknown {
                key: key.to_owned(),
            })?;
        Ok(self.modules.remove(index))
    }

    /// Moves one module's level. Above the building is refused, below zero is
    /// refused; going down is allowed, a module that was worn out is not a
    /// mistake.
    pub fn set_module_level(&mut self, key: &str, level: i32) -> Result<(), Refusal> {
        if level > self.level {
            return Err(Refusal::ModuleAboveBuilding {
                key: key.to_owned(),
                level,
                building: self.level,
            });
        }
        if level < 0 {
            return Err(Refusal::LevelOutOfRange { level });
        }
        let index = self
            .modules
            .binary_search_by(|module| module.key.as_str().cmp(key))
            .map_err(|_| Refusal::ModuleUnknown {
                key: key.to_owned(),
            })?;
        self.modules[index].level = level;
        Ok(())
    }

    /// Gives the hut its worker.
    pub fn hire(&mut self, citizen: i32) -> Result<(), Refusal> {
        if !self.can_assign_citizens() {
            return Err(Refusal::NotBuilt);
        }
        if let Some(worker) = self.worker {
            return Err(Refusal::Occupied { worker });
        }
        self.worker = Some(citizen);
        Ok(())
    }

    /// Takes the worker away and gives back its id.
    pub fn fire(&mut self) -> Result<i32, Refusal> {
        self.worker.take().ok_or(Refusal::Vacant)
    }

    /// The building as a compound, self-contained: its kind and its position
    /// are written, so a record read from it does not need the colony's list
    /// beside it. Absent fields are **not** written, which is what makes
    /// "absent = default" on the way back meaningful.
    pub fn to_compound(&self) -> Compound {
        let path = self.kind.path();
        let mut modules = Compound::new();
        for module in &self.modules {
            modules.insert(module.key.as_str().into(), Tag::Compound(module.to_compound()));
        }
        let mut data = Compound::from([
            (KEY_TYPE.into(), Tag::String(path.as_str().into())),
            (KEY_AT.into(), nbt::position(self.at)),
            (KEY_LEVEL.into(), Tag::Int(self.level)),
            (KEY_BUILT.into(), nbt::boolean_tag(self.built)),
            (KEY_DECONSTRUCTED.into(), nbt::boolean_tag(self.deconstructed)),
            (KEY_MODULES.into(), Tag::Compound(modules)),
        ]);
        if let Some(at) = self.parent {
            data.insert(KEY_PARENT.into(), nbt::position(at));
        }
        if let Some(worker) = self.worker {
            data.insert(KEY_WORKER.into(), Tag::Int(worker));
        }
        data
    }

    /// Reads a building. **`None` when the type is missing or unknown**: Java
    /// returns null and the caller drops the entry
    /// (`core/colony/buildings/registry/BuildingDataManager.java:35-49`) —
    /// there is nothing to build a building out of, and inventing a kind would
    /// hand the rest of the port a building that is not in the registry.
    ///
    /// Everything else is tolerant: a level out of range is clamped, a module
    /// that outranks its building is dropped (the building survives, which Java
    /// does not do — it drops the whole building on any exception), and a
    /// missing `isBuilt` reads as built, as Java reads it for a legacy save
    /// (`AbstractBuilding.java:332-339`).
    pub fn from_compound(data: &Compound) -> Option<Building> {
        let kind = Kind::from_path(&nbt::text(data, KEY_TYPE)?)?;
        let mut building = Building {
            kind,
            at: nbt::position_in(data, KEY_AT).unwrap_or([0, 0, 0]),
            level: 0,
            built: true,
            deconstructed: false,
            parent: None,
            modules: Vec::new(),
            worker: nbt::int(data, KEY_WORKER),
        };
        // The level goes through the guard, so the rest of the port may assume
        // the range; then the two flags, which `set_level` has just cleared.
        let _ = building.set_level(clamp_level(nbt::int(data, KEY_LEVEL).unwrap_or(0)));
        building.built = nbt::boolean(data, KEY_BUILT).unwrap_or(true);
        building.deconstructed = nbt::boolean(data, KEY_DECONSTRUCTED).unwrap_or(false);
        // Java saves the modules as **one compound keyed by module key**
        // (`AbstractBuilding.java:385-393`), not as a list: the key is the
        // module's identity, and a duplicate cannot exist in a map.
        if let Some(Tag::Compound(modules)) = data.get(&KEY_MODULES.into()) {
            for (key, tag) in modules {
                let Some(level) = tag.get(KEY_MODULE_LEVEL).and_then(nbt::int_of) else {
                    continue;
                };
                let Some(module) = Module::new(&key.to_string_lossy(), level) else {
                    continue;
                };
                // A module that outranks the building is corrupt, and is all
                // this drops; the building itself is still readable.
                let _ = building.attach_module(module);
            }
        }
        if let Some(at) = nbt::position_in(data, KEY_PARENT) {
            let _ = building.set_parent(Some(at));
        }
        Some(building)
    }
}

/// A level read from anywhere, kept in the range the rest of the port assumes:
/// `0` (placed, not built) to [`MAX_BUILDING_LEVEL`]. Java's `deserializeNBT`
/// copies the number it finds without a guard
/// (`AbstractSchematicProvider.java:191`); this port clamps on read, as the
/// colony record does for its own level (`src/colony/colonie.rs:186-190`).
pub fn clamp_level(level: i32) -> i32 {
    level.clamp(0, MAX_BUILDING_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: [i32; 3] = [10, 64, -3];

    fn hut() -> Building {
        let mut building = Building::new(Kind::Builder, AT);
        building.set_level(3).expect("three is a level");
        building
    }

    fn module(key: &str, level: i32) -> Module {
        Module::new(key, level).expect("a named module")
    }

    #[test]
    fn a_hut_just_placed_is_level_zero_and_takes_no_worker() {
        let building = Building::new(Kind::Builder, AT);
        assert_eq!(building.level(), 0);
        assert_eq!(building.stage(), Stage::Unbuilt);
        assert_eq!(building.id(), AT, "a building is its position");
        assert!(!building.can_assign_citizens());
        let mut building = building;
        assert_eq!(building.hire(7), Err(Refusal::NotBuilt));
        assert_eq!(building.worker(), None);
        assert_eq!(module("work", 0).key(), "work");
        assert!(Module::new("", 0).is_none(), "a module with no key cannot be saved");
    }

    #[test]
    fn the_level_stops_at_five_and_takes_no_negative() {
        let mut building = Building::new(Kind::Farmer, AT);
        assert_eq!(building.set_level(6), Err(Refusal::LevelAboveMax { level: 6 }));
        assert_eq!(building.set_level(-1), Err(Refusal::LevelOutOfRange { level: -1 }));
        assert_eq!(building.level(), 0, "a refused level changes nothing");
        assert_eq!(building.set_level(5), Ok(()));
        assert_eq!(building.level(), 5);
        assert_eq!(clamp_level(9), 5);
        assert_eq!(clamp_level(-4), 0);
    }

    #[test]
    fn finishing_a_level_builds_the_hut_and_a_pickup_wins_over_it() {
        let mut building = Building::new(Kind::Builder, AT);
        assert_eq!(building.complete(0), Err(Refusal::LevelOutOfRange { level: 0 }));
        assert_eq!(building.complete(2), Ok(()));
        assert_eq!((building.level(), building.stage()), (2, Stage::Built));
        assert!(building.can_assign_citizens());
        building.deconstruct();
        assert_eq!(building.stage(), Stage::Deconstructed);
        // Setting a level clears the pickup flag (AbstractSchematicProvider.java:487).
        assert_eq!(building.set_level(2), Ok(()));
        assert_eq!(building.stage(), Stage::Built);
    }

    #[test]
    fn a_hut_takes_one_worker_and_gives_it_back_with_its_id() {
        let mut building = hut();
        building.complete(3).expect("level three is built");
        assert_eq!(building.hire(12), Ok(()));
        assert_eq!(building.worker(), Some(12));
        assert_eq!(building.hire(13), Err(Refusal::Occupied { worker: 12 }));
        assert_eq!(building.fire(), Ok(12));
        assert_eq!(building.fire(), Err(Refusal::Vacant));
    }

    #[test]
    fn a_module_is_attached_once_and_never_outranks_its_building() {
        let mut building = hut();
        assert_eq!(building.attach_module(module("work", 4)), Err(Refusal::ModuleAboveBuilding {
            key: "work".to_owned(),
            level: 4,
            building: 3,
        }));
        assert_eq!(building.attach_module(module("work", 3)), Ok(()));
        assert_eq!(
            building.attach_module(module("work", 1)),
            Err(Refusal::ModuleDuplicate {
                key: "work".to_owned()
            })
        );
        // Two different keys, kept sorted, so a round trip is exact.
        assert_eq!(building.attach_module(module("bed", 1)), Ok(()));
        let keys: Vec<&str> = building.modules().iter().map(Module::key).collect();
        assert_eq!(keys, vec!["bed", "work"]);
        assert_eq!(building.module("bed").map(Module::level), Some(1));
        assert_eq!(building.module("absent"), None);
    }

    #[test]
    fn a_module_moves_within_its_building_and_lowering_the_building_is_refused() {
        let mut building = hut();
        building.attach_module(module("work", 3)).expect("three of three");
        assert_eq!(
            building.set_module_level("work", 4),
            Err(Refusal::ModuleAboveBuilding {
                key: "work".to_owned(),
                level: 4,
                building: 3
            })
        );
        assert_eq!(building.set_module_level("work", 2), Ok(()));
        assert_eq!(
            building.set_module_level("absent", 1),
            Err(Refusal::ModuleUnknown {
                key: "absent".to_owned()
            })
        );
        // The invariant holds from both ends: a building may not sink below a
        // module it carries.
        assert_eq!(
            building.set_level(1),
            Err(Refusal::LevelBelowModule {
                key: "work".to_owned(),
                level: 2,
                level_asked: 1
            })
        );
        assert_eq!(building.level(), 3);
        assert_eq!(building.detach_module("work").map(|m| m.level()), Ok(2));
        assert_eq!(building.set_level(1), Ok(()));
        assert_eq!(building.detach_module("work"), Err(Refusal::ModuleUnknown {
            key: "work".to_owned()
        }));
    }

    #[test]
    fn a_building_round_trips_with_its_modules_its_parent_and_its_worker() {
        let mut building = hut();
        building.complete(4).expect("four");
        building.set_parent(Some([12, 64, -3])).expect("a parent elsewhere");
        building.attach_module(module("work", 4)).expect("four of four");
        building.attach_module(module("settings", 0)).expect("no level at all");
        building.hire(31).expect("a built hut hires");

        let data = building.to_compound();
        assert_eq!(nbt::text(&data, KEY_TYPE).as_deref(), Some("minecolonies:builder"));
        assert_eq!(nbt::position_in(&data, KEY_AT), Some(AT));
        assert_eq!(nbt::int(&data, KEY_LEVEL), Some(4));
        assert_eq!(nbt::boolean(&data, KEY_BUILT), Some(true));
        assert_eq!(nbt::int(&data, KEY_WORKER), Some(31));
        assert_eq!(nbt::position_in(&data, KEY_PARENT), Some([12, 64, -3]));
        assert_eq!(nbt::compounds(&data, KEY_MODULES).len(), 0, "modules are a compound");
        assert_eq!(
            data.get(&KEY_MODULES.into()),
            Some(&Tag::Compound(Compound::from([
                ("settings".into(), Tag::compound([(KEY_MODULE_LEVEL, Tag::Int(0))])),
                ("work".into(), Tag::compound([(KEY_MODULE_LEVEL, Tag::Int(4))])),
            ]))),
            "one compound keyed by module key"
        );

        assert_eq!(Building::from_compound(&data), Some(building));
    }

    #[test]
    fn an_unbuilt_hut_without_a_parent_round_trips_too() {
        let building = Building::new(Kind::TownHall, [0, 70, 0]);
        let data = building.to_compound();
        assert_eq!(
            nbt::position_in(&data, KEY_PARENT),
            None,
            "no parent is written as no key, not as three zeros"
        );
        assert_eq!(Building::from_compound(&data), Some(building));
    }

    #[test]
    fn an_unknown_or_missing_type_reads_as_no_building() {
        let mut data = hut().to_compound();
        data.insert(KEY_TYPE.into(), Tag::String("minecolonies:castle".into()));
        assert_eq!(Building::from_compound(&data), None, "a kind the registry does not know");
        let data = nbt::without(&hut().to_compound(), KEY_TYPE);
        assert_eq!(Building::from_compound(&data), None);
        assert_eq!(Building::from_compound(&Compound::new()), None);
        // A bare name in a hand-written merge still names a kind.
        let mut data = hut().to_compound();
        data.insert(KEY_TYPE.into(), Tag::String("builder".into()));
        assert_eq!(Building::from_compound(&data).map(|b| b.kind()), Some(Kind::Builder));
    }

    #[test]
    fn a_level_written_by_hand_is_clamped_and_a_flag_defaults_to_built() {
        let mut data = nbt::without(&hut().to_compound(), KEY_BUILT);
        data.insert(KEY_LEVEL.into(), Tag::Int(9));
        let read = Building::from_compound(&data).expect("a builder");
        assert_eq!(read.level(), 5, "clamped, as the colony record clamps its own level");
        assert_eq!(read.stage(), Stage::Built, "a legacy save without isBuilt reads as built");
        data.insert(KEY_LEVEL.into(), Tag::Int(-4));
        assert_eq!(Building::from_compound(&data).map(|b| b.level()), Some(0));
    }

    #[test]
    fn a_module_above_its_building_is_dropped_on_read_not_the_building() {
        let mut data = hut().to_compound();
        data.insert(KEY_LEVEL.into(), Tag::Int(1));
        let mut modules = Compound::new();
        modules.insert("work".into(), Tag::compound([(KEY_MODULE_LEVEL, Tag::Int(4))]));
        modules.insert("bed".into(), Tag::compound([(KEY_MODULE_LEVEL, Tag::Int(1))]));
        data.insert(KEY_MODULES.into(), Tag::Compound(modules));
        let read = Building::from_compound(&data).expect("a builder");
        assert_eq!(read.module("work"), None, "level 4 module on a level 1 building");
        assert_eq!(read.module("bed").map(Module::level), Some(1));
    }

    #[test]
    fn a_parent_is_never_the_building_itself() {
        let mut building = hut();
        assert_eq!(building.set_parent(Some(AT)), Err(Refusal::ParentSelf { at: AT }));
        assert_eq!(building.parent(), None);
        building.set_parent(Some([1, 2, 3])).expect("another building");
        assert_eq!(building.parent(), Some([1, 2, 3]));
        building.set_parent(Some(NO_PARENT)).expect("three zeros clear it");
        assert_eq!(building.parent(), None);
        building.set_parent(Some([1, 2, 3])).expect("again");
        building.set_parent(None).expect("and explicitly");
        assert_eq!(building.parent(), None);
    }
}
