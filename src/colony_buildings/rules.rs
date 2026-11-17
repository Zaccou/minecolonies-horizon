//! The same rules, asked of a building that stands: what it holds, what it
//! refuses, and how far it reaches.
//!
//! Java reference: `core/colony/buildings/AbstractBuilding.java` (the level, the
//! upgrade, the workers), `workerbuildings/BuildingWareHouse.java:194-197` (the
//! sorting), `workerbuildings/BuildingBaker.java:161-165` (the crafting types),
//! `workerbuildings/BuildingCrusher.java:87-96` and `BuildingSifter.java:79-88`
//! (the daily limit), `workerbuildings/BuildingGuardTower.java:51-71` and
//! `AbstractBuilding.java:654-667` (the claim radius),
//! `moduleviews/FloristFlowerListModuleView.java:52-66` (the flower filter).
//!
//! **A wrapper, not a second record.** [`Hut`] borrows a
//! [`crate::building::Building`] and adds the questions the record cannot answer
//! on its own, because they need the *kind*'s table ([`super::catalog`]) and the
//! *module*'s formula ([`super::capacity`]). Nothing is copied: `level()`,
//! `kind()` and `module()` read through.
//!
//! **The two rules that make a hut refuse a module**, and this is the pair the
//! brief called "module non supporté par ce type" and "niveau insuffisant":
//!
//! - a module the kind's entry never registers — [`Hut::accepts`] answers it from
//!   the table, and [`attach`] refuses it by name;
//! - a module whose *feature* needs a research or a level — [`Hut::usable`] runs
//!   both gates and names which one failed.
//!
//! **"Batiment en construction" is one refusal and two facts.** A hut placed and
//! not built is level 0 (`AbstractSchematicProvider.java:64`) and `isBuilt` is
//! false (`AbstractBuilding.java:172`); `canAssignCitizens` is the conjunction
//! (`api/colony/buildings/IBuilding.java:464-467`). [`Hut::stands`] is that
//! conjunction, and every rule about *content* starts by asking it, which is why
//! the refusal is [`super::Refusal::HutUnderConstruction`] and not "full".

use crate::building::{Building, Kind, Module, Stage};

use super::capacity::{capacity_on, Capacity};
use super::catalog::accepts;
use super::key::ModuleKey;
use super::refusal::Refusal;
use super::{gate, Gate};

/// A guard tower's claim radius, level by level: 2, then 3, 3, 4, 5
/// (`workerbuildings/BuildingGuardTower.java:51-71`).
///
/// Level 0 answers 0, as Java's `default:` branch does — a hut that is not built
/// claims nothing.
pub fn guard_tower_claim_radius(level: i32) -> i32 {
    match level {
        1 => 2,
        2 | 3 => 3,
        4 => 4,
        5 => 5,
        _ => 0,
    }
}

/// The claim radius of any other hut: 1 up to level 3, 2 at 4 and 5, 0 outside
/// (`core/colony/buildings/AbstractBuilding.java:654-667`).
pub fn default_claim_radius(level: i32) -> i32 {
    match level {
        1..=3 => 1,
        4 | 5 => 2,
        _ => 0,
    }
}

/// The claim radius of a kind at a level: the guard tower is the one override
/// (`BuildingGuardTower.java:46-71`); every other kind uses the default.
///
/// **The barracks is the exception this does not model.** Its radius is 3 for
/// level 1-3 only when *every* barracks tower has reached level 4, 2 otherwise
/// (`workerbuildings/BuildingBarracks.java:168-183`), which needs the towers'
/// levels and so cannot be answered from one building. `MANQUANT.md` says so.
pub fn claim_radius(kind: Kind, level: i32) -> i32 {
    if kind == Kind::GuardTower {
        guard_tower_claim_radius(level)
    } else {
        default_claim_radius(level)
    }
}

/// The level a `minimum stock` module must reach before the colony sorts a
/// warehouse: `DEFAULT_REQUIRED_SORT_LEVEL = 3`
/// (`api/util/constant/BuildingConstants.java:108`, read at
/// `workerbuildings/BuildingWareHouse.java:196`).
pub const SORT_FROM_LEVEL: i32 = 3;

/// The level from which the baker accepts crafting types at all: below it,
/// `getSupportedCraftingTypes` returns the empty set
/// (`workerbuildings/BuildingBaker.java:161-165`, the same override twice — once
/// for `CraftingModule`, once for `SmeltingModule`).
pub const BAKER_CRAFTING_FROM_LEVEL: i32 = 3;

/// A hut that is built or being built, plus the table of what its kind carries.
#[derive(Clone, Copy, Debug)]
pub struct Hut<'a> {
    building: &'a Building,
}

impl<'a> Hut<'a> {
    /// Borrows a building. No copy, no cache: every answer reads through.
    pub fn new(building: &'a Building) -> Hut<'a> {
        Hut { building }
    }

    pub fn building(&self) -> &'a Building {
        self.building
    }

    pub fn kind(&self) -> Kind {
        self.building.kind()
    }

    pub fn level(&self) -> i32 {
        self.building.level()
    }

    /// Whether the hut is built and at level 1 or more: `getBuildingLevel() > 0 &&
    /// isBuilt()` (`api/colony/buildings/IBuilding.java:464-467`).
    ///
    /// **One rule of this port's own, added here.** A deconstructed hut keeps
    /// `isBuilt` true — Java only sets the pickup flag (`AbstractSchematicProvider.java:142-145`)
    /// — so Java's conjunction would still answer true for a hole in the ground.
    /// The port also requires [`Stage::Built`], which is strictly smaller and
    /// says the same thing a player sees. It is the only place in this module
    /// where the answer is narrower than Java's, and it is named as such.
    pub fn stands(&self) -> bool {
        self.building.can_assign_citizens() && self.building.stage() == Stage::Built
    }

    /// Every module the kind's entry registers, in Java's order.
    pub fn spec_modules(&self) -> &'static [ModuleKey] {
        super::catalog::spec_of(self.kind()).modules
    }

    /// Whether the kind registers this module at all — Java's `hasModule`
    /// (`core/colony/buildings/AbstractBuilding.java:190-194`).
    pub fn accepts(&self, key: ModuleKey) -> bool {
        accepts(self.kind(), key)
    }

    /// The modules actually attached to this building, as the record holds them.
    pub fn attached(&self) -> &'a [Module] {
        self.building.modules()
    }

    /// Whether the building carries the module, in the record's own list.
    pub fn has_attached(&self, key: ModuleKey) -> bool {
        self.building.module(key.key()).is_some()
    }

    /// The capacity of a module on this hut: the worker table first, then the
    /// module formula.
    pub fn capacity(&self, key: ModuleKey) -> Capacity {
        capacity_on(self.kind(), key)
    }

    /// The research gate of a module, with the colony's known effects supplied by
    /// the caller.
    ///
    /// `known` answers "is this effect above 0": Java tests
    /// `getEffectStrength(effect) > 0` (`modules/WorkerBuildingModule.java:167`),
    /// and this port has the research tree (`crate::research`) but no colony
    /// state to ask.
    pub fn research_gate(
        &self,
        key: ModuleKey,
        known: impl Fn(&str) -> bool,
    ) -> Result<(), Refusal> {
        if !self.accepts(key) {
            return Err(Refusal::ModuleNotOnThisHut {
                kind: self.kind(),
                key,
            });
        }
        match gate(self.kind(), key).research() {
            Some(effect) if !known(effect) => Err(Refusal::ResearchMissing {
                key,
                research: effect,
            }),
            _ => Ok(()),
        }
    }

    /// The level gate of a module: the feature it carries switches on at that
    /// level, and below it the module is present but does nothing.
    pub fn level_gate(&self, key: ModuleKey) -> Result<(), Refusal> {
        if !self.accepts(key) {
            return Err(Refusal::ModuleNotOnThisHut {
                kind: self.kind(),
                key,
            });
        }
        match gate(self.kind(), key) {
            Gate::Level(needed) if self.level() < needed => Err(Refusal::LevelTooLow {
                kind: self.kind(),
                key,
                level: self.level(),
                needed,
            }),
            _ => Ok(()),
        }
    }

    /// Both gates: the module is usable now, with what the colony knows.
    ///
    /// `Gate::OnceBuilt` is **not** tested here — it is a statement about the
    /// capacity, not about the module, and [`Self::seats`] is where it refuses.
    pub fn usable(
        &self,
        key: ModuleKey,
        known: impl Fn(&str) -> bool,
    ) -> Result<(), Refusal> {
        self.research_gate(key, known)?;
        self.level_gate(key)
    }

    /// How many the module holds, refusing when the hut is not standing.
    ///
    /// Java's lambdas answer a number at level 0 too — `(b) -> 1` answers 1 —
    /// because nothing stops a caller from *asking*; what stops the first worker
    /// is `canAssignCitizens`, which is [`Self::stands`]. The port refuses at the
    /// same place instead of returning a number the caller must not use.
    pub fn seats(&self, key: ModuleKey) -> Result<i32, Refusal> {
        if !self.accepts(key) {
            return Err(Refusal::ModuleNotOnThisHut {
                kind: self.kind(),
                key,
            });
        }
        if !self.stands() {
            return Err(Refusal::HutUnderConstruction {
                kind: self.kind(),
                level: self.level(),
            });
        }
        let capacity = self.capacity(key);
        if !capacity.is_counted() {
            return Err(Refusal::NotCounted { key });
        }
        Ok(capacity.at(self.level()))
    }

    /// How many more the module takes, given how many are already there.
    ///
    /// `held` is the caller's: the building record has one worker slot, while
    /// Java counts per module (`IAssignsCitizen.getModuleMax`,
    /// `api/colony/buildings/modules/IAssignsCitizen.java:36-46`).
    pub fn room_for(&self, key: ModuleKey, held: i32) -> Result<i32, Refusal> {
        let seats = self.seats(key)?;
        let free = seats - held;
        if free <= 0 {
            return Err(Refusal::Full {
                key,
                capacity: seats,
                held,
            });
        }
        Ok(free)
    }

    /// Whether a citizen may be given to this module, `held` of them already
    /// assigned: the module must assign citizens at all
    /// (`IAssignsCitizen.java:14-46`), the hut must stand, and there must be room.
    pub fn hire_room(&self, key: ModuleKey, held: i32) -> Result<i32, Refusal> {
        if !key.hires() {
            return Err(Refusal::NotAnAssignment { key });
        }
        self.room_for(key, held)
    }

    /// Whether this hut sorts its racks, and from which level.
    ///
    /// `IBuilding.canSort()` is false by default (`api/colony/buildings/IBuilding.java:518-525`)
    /// and only the warehouse overrides it, at `getBuildingLevel() >=
    /// DEFAULT_REQUIRED_SORT_LEVEL` (`workerbuildings/BuildingWareHouse.java:194-197`).
    pub fn may_sort(&self) -> Result<(), Refusal> {
        if self.kind() != Kind::Warehouse {
            return Err(Refusal::NotSortable { kind: self.kind() });
        }
        if self.level() < SORT_FROM_LEVEL {
            return Err(Refusal::LevelTooLow {
                kind: Kind::Warehouse,
                key: ModuleKey::WarehouseOptions,
                level: self.level(),
                needed: SORT_FROM_LEVEL,
            });
        }
        Ok(())
    }

    /// The crafting types this hut accepts at its level: none below level 3 for
    /// the baker (`BuildingBaker.java:161-165`), everything its module declares
    /// otherwise.
    ///
    /// The level is read here and the *set* is not: which types a crafter
    /// supports is a crafting-module question (`crate::crafting`), and this rule
    /// only says "none of them yet".
    pub fn may_craft(&self) -> Result<(), Refusal> {
        if self.kind() == Kind::Baker && self.level() < BAKER_CRAFTING_FROM_LEVEL {
            return Err(Refusal::LevelTooLow {
                kind: Kind::Baker,
                key: ModuleKey::BakerCraft,
                level: self.level(),
                needed: BAKER_CRAFTING_FROM_LEVEL,
            });
        }
        Ok(())
    }

    /// What a crusher or a sifter may crush or sift in a day, and `None` for the
    /// other kinds: `level² × 16` for the crusher
    /// (`workerbuildings/BuildingCrusher.java:87-96`) and `level² × 64` for the
    /// sifter (`workerbuildings/BuildingSifter.java:79-88`), with **no limit at
    /// all** at level 5 — Java returns `Integer.MAX_VALUE` there, and the port
    /// keeps that rather than a large number.
    pub fn daily_limit(&self) -> Option<i32> {
        let factor = match self.kind() {
            Kind::Crusher => 16,
            Kind::Sifter => 64,
            _ => return None,
        };
        if self.level() >= crate::building::MAX_BUILDING_LEVEL {
            return Some(i32::MAX);
        }
        Some(self.level() * self.level() * factor)
    }

    /// How many flowers the florist's filter holds, and `None` for the other
    /// kinds: one up to level 3, five at level 4, no bound at 5
    /// (`moduleviews/FloristFlowerListModuleView.java:21`, `:56-66`).
    pub fn flower_filter_limit(&self) -> Option<i32> {
        if self.kind() != Kind::Florist {
            return None;
        }
        Some(match self.level() {
            0..=3 => 1,
            4 => 5,
            _ => i32::MAX,
        })
    }

    /// Whether one more flower may be added to the florist's filter, `held`
    /// already filtered.
    ///
    /// The two refusals are Java's two messages:
    /// `TOO_MANY_FILTERED_BELOW_LVL4_FLORIST` (`:56-58`) and
    /// `TOO_MANY_FILTERED_FLORIST` (`:62-66`).
    pub fn may_filter_flower(&self, held: i32) -> Result<(), Refusal> {
        let limit = self
            .flower_filter_limit()
            .ok_or(Refusal::ModuleNotOnThisHut {
                kind: self.kind(),
                key: ModuleKey::FloristItems,
            })?;
        if held < limit {
            return Ok(());
        }
        if self.level() <= 3 {
            return Err(Refusal::FlowersNotSortedYet {
                level: self.level(),
            });
        }
        Err(Refusal::FlowersNotFilteredAtThisLevel {
            level: self.level(),
            limit,
        })
    }

    /// The blocks of land this hut claims around itself, in chunks, at its level.
    pub fn claim_radius(&self) -> i32 {
        claim_radius(self.kind(), self.level())
    }
}

/// Attaches a module to a building, refusing one the kind never registers.
///
/// The duplicate and the "module above its building" rules are
/// `crate::building::Building::attach_module`'s, and their refusal comes back
/// wrapped (`super::Refusal::Building`) so the eight level rules keep one
/// spelling. This function adds the one check that rule cannot make: whether the
/// *kind* has the module at all — Java's entry-list membership, which no
/// building-side check performs because the list is only walked at construction
/// (`api/colony/buildings/registry/BuildingEntry.java:148-160`).
pub fn attach(building: &mut Building, key: ModuleKey, level: i32) -> Result<(), Refusal> {
    let kind = building.kind();
    if !accepts(kind, key) {
        return Err(Refusal::ModuleNotOnThisHut { kind, key });
    }
    // `ModuleKey::key` is never empty — a test in `key` holds that — so this arm
    // is unreachable; it exists because `Module::new` refuses an empty key and a
    // `Result` has to say something.
    let module = match Module::new(key.key(), level) {
        Some(module) => module,
        None => return Err(Refusal::NotCounted { key }),
    };
    building.attach_module(module).map_err(Refusal::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::MAX_BUILDING_LEVEL;

    const AT: [i32; 3] = [4, 70, -8];

    fn built(kind: Kind, level: i32) -> Building {
        let mut building = Building::new(kind, AT);
        building.complete(level).expect("a level of 1 or more");
        building
    }

    fn none_known(_: &str) -> bool {
        false
    }

    fn all_known(_: &str) -> bool {
        true
    }

    #[test]
    fn a_hut_placed_and_not_built_holds_nothing_and_says_why() {
        let building = Building::new(Kind::Residence, AT);
        let hut = Hut::new(&building);
        assert!(!hut.stands());
        assert_eq!(hut.level(), 0);
        assert_eq!(
            hut.seats(ModuleKey::Living),
            Err(Refusal::HutUnderConstruction {
                kind: Kind::Residence,
                level: 0
            })
        );
        assert_eq!(
            hut.seats(ModuleKey::Bed),
            Err(Refusal::HutUnderConstruction {
                kind: Kind::Residence,
                level: 0
            }),
            "even an uncounted module refuses while the hut is a hole"
        );
    }

    #[test]
    fn a_built_residence_houses_one_citizen_per_level() {
        let building = built(Kind::Residence, 3);
        let hut = Hut::new(&building);
        assert!(hut.stands());
        assert_eq!(hut.seats(ModuleKey::Living), Ok(3));
        assert_eq!(hut.room_for(ModuleKey::Living, 2), Ok(1));
        assert_eq!(
            hut.room_for(ModuleKey::Living, 3),
            Err(Refusal::Full {
                key: ModuleKey::Living,
                capacity: 3,
                held: 3
            })
        );
        assert_eq!(
            hut.hire_room(ModuleKey::Living, 4),
            Err(Refusal::Full {
                key: ModuleKey::Living,
                capacity: 3,
                held: 4
            }),
            "over-full is full, not a negative room"
        );
    }

    #[test]
    fn a_tavern_houses_four_however_high_it_goes_and_holds_no_worker() {
        let building = built(Kind::Tavern, 3);
        let hut = Hut::new(&building);
        assert_eq!(hut.capacity(ModuleKey::TavernLiving), Capacity::FlatWhenBuilt(4));
        assert_eq!(hut.seats(ModuleKey::TavernLiving), Ok(4));
        assert_eq!(hut.seats(ModuleKey::TavernVisitor), Ok(9), "three visitors per level");
        assert_eq!(
            hut.hire_room(ModuleKey::TavernLiving, 0),
            Ok(4),
            "the living module assigns residents and does count"
        );
        assert_eq!(
            hut.hire_room(ModuleKey::Bed, 0),
            Err(Refusal::NotAnAssignment {
                key: ModuleKey::Bed
            }),
            "a bed is a block entity, not an assignment"
        );
    }

    #[test]
    fn a_stash_is_never_built_and_therefore_answers_no_content_question() {
        let building = Building::new(Kind::Stash, AT);
        let hut = Hut::new(&building);
        assert_eq!(hut.kind(), Kind::Stash);
        assert!(!hut.stands());
        assert_eq!(hut.spec_modules().len(), 0);
        assert!(!hut.accepts(ModuleKey::MinStock));
        assert_eq!(
            hut.seats(ModuleKey::MinStock),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::Stash,
                key: ModuleKey::MinStock
            }),
            "the kind check comes before the build check"
        );
    }

    #[test]
    fn a_module_the_kind_never_registers_is_refused_with_the_kind_and_the_key() {
        let building = built(Kind::Blacksmith, 2);
        let hut = Hut::new(&building);
        assert!(hut.accepts(ModuleKey::BlacksmithCraft));
        assert!(!hut.accepts(ModuleKey::Furnace), "the smithy has no furnace module");
        assert_eq!(
            hut.seats(ModuleKey::Furnace),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::Blacksmith,
                key: ModuleKey::Furnace
            })
        );
        assert_eq!(
            hut.level_gate(ModuleKey::Furnace),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::Blacksmith,
                key: ModuleKey::Furnace
            })
        );
    }

    #[test]
    fn attaching_a_module_the_kind_lacks_is_refused_and_one_it_has_is_kept() {
        let mut building = built(Kind::Baker, 2);
        assert_eq!(
            attach(&mut building, ModuleKey::Furnace, 2),
            Ok(()),
            "a bakery has a furnace"
        );
        assert_eq!(building.module("furnace").map(Module::level), Some(2));
        assert_eq!(
            attach(&mut building, ModuleKey::WorkOrderView, 2),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::Baker,
                key: ModuleKey::WorkOrderView
            })
        );
        // The duplicate rule is the record's, and its name comes back unchanged.
        let duplicate = attach(&mut building, ModuleKey::Furnace, 2).expect_err("already there");
        assert_eq!(
            duplicate.name(),
            crate::building::Refusal::ModuleDuplicate {
                key: "furnace".to_owned()
            }
            .name()
        );
    }

    #[test]
    fn a_module_above_its_building_is_refused_through_the_same_door() {
        let mut building = built(Kind::Baker, 1);
        let refused = attach(&mut building, ModuleKey::Furnace, 3).expect_err("three on a one");
        assert_eq!(refused.name(), "building.refusal.module_above_building");
        assert_eq!(
            attach(&mut building, ModuleKey::Furnace, 0),
            Ok(()),
            "level 0 is below every building and allowed"
        );
    }

    #[test]
    fn a_module_that_counts_nothing_is_not_a_seat_count() {
        let building = built(Kind::Baker, 3);
        let hut = Hut::new(&building);
        assert_eq!(
            hut.seats(ModuleKey::StatsModule),
            Err(Refusal::NotCounted {
                key: ModuleKey::StatsModule
            })
        );
        assert_eq!(
            hut.seats(ModuleKey::CraftSettings),
            Err(Refusal::NotCounted {
                key: ModuleKey::CraftSettings
            })
        );
        assert_eq!(hut.seats(ModuleKey::MinStock), Ok(15), "five per level");
    }

    #[test]
    fn the_furnace_keeps_sixty_four_stacks_of_fuel_per_level() {
        let building = built(Kind::Kitchen, 4);
        let hut = Hut::new(&building);
        assert_eq!(hut.capacity(ModuleKey::Furnace), Capacity::KeptStacksPerLevel(64));
        assert_eq!(hut.seats(ModuleKey::Furnace), Ok(256));
    }

    #[test]
    fn a_marksman_needs_his_research_and_a_knight_does_not() {
        let building = built(Kind::GuardTower, 2);
        let hut = Hut::new(&building);
        assert_eq!(
            hut.research_gate(ModuleKey::MarksmanTowerWork, none_known),
            Err(Refusal::ResearchMissing {
                key: ModuleKey::MarksmanTowerWork,
                research: "marksman"
            })
        );
        assert_eq!(hut.research_gate(ModuleKey::MarksmanTowerWork, all_known), Ok(()));
        assert_eq!(hut.research_gate(ModuleKey::KnightTowerWork, none_known), Ok(()));
        // The huscarl is gated on its own effect, not on the marksman's.
        let only_marksman = |effect: &str| effect == "marksman";
        assert!(hut.research_gate(ModuleKey::HuscarlTowerWork, only_marksman).is_err());
        assert!(hut.usable(ModuleKey::HuscarlTowerWork, only_marksman).is_err());
        // A hut that does not carry the module is refused before the research.
        assert_eq!(
            hut.research_gate(ModuleKey::MarksmanBarracksWork, none_known),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::GuardTower,
                key: ModuleKey::MarksmanBarracksWork
            })
        );
    }

    #[test]
    fn the_baker_crafts_from_level_three_and_never_before() {
        assert!(Hut::new(&built(Kind::Baker, 2)).may_craft().is_err());
        assert_eq!(
            Hut::new(&built(Kind::Baker, 2)).may_craft(),
            Err(Refusal::LevelTooLow {
                kind: Kind::Baker,
                key: ModuleKey::BakerCraft,
                level: 2,
                needed: 3
            })
        );
        assert_eq!(Hut::new(&built(Kind::Baker, 3)).may_craft(), Ok(()));
        // The rule is the baker's alone: a sawmill at level 1 crafts.
        assert_eq!(Hut::new(&built(Kind::Sawmill, 1)).may_craft(), Ok(()));
    }

    #[test]
    fn the_bakers_crafting_types_carry_a_level_gate_and_a_smiths_do_not() {
        let baker = built(Kind::Baker, 2);
        let hut = Hut::new(&baker);
        assert_eq!(
            hut.level_gate(ModuleKey::BakerCraft),
            Err(Refusal::LevelTooLow {
                kind: Kind::Baker,
                key: ModuleKey::BakerCraft,
                level: 2,
                needed: 3
            })
        );
        assert_eq!(Hut::new(&built(Kind::Baker, 3)).level_gate(ModuleKey::BakerCraft), Ok(()));
        assert_eq!(
            Hut::new(&built(Kind::Blacksmith, 1)).level_gate(ModuleKey::BlacksmithCraft),
            Ok(())
        );
    }

    #[test]
    fn a_warehouse_sorts_from_level_three_and_no_other_hut_sorts_at_all() {
        assert_eq!(
            Hut::new(&built(Kind::Warehouse, 2)).may_sort(),
            Err(Refusal::LevelTooLow {
                kind: Kind::Warehouse,
                key: ModuleKey::WarehouseOptions,
                level: 2,
                needed: 3
            })
        );
        assert_eq!(Hut::new(&built(Kind::Warehouse, 3)).may_sort(), Ok(()));
        assert_eq!(Hut::new(&built(Kind::Warehouse, 5)).may_sort(), Ok(()));
        assert_eq!(
            Hut::new(&built(Kind::Residence, 5)).may_sort(),
            Err(Refusal::NotSortable {
                kind: Kind::Residence
            })
        );
    }

    #[test]
    fn a_crusher_crumbles_sixteen_times_the_square_of_its_level_and_no_limit_at_five() {
        // BuildingCrusher.java:87-96: `level² × 16`, and `Integer.MAX_VALUE` at 5.
        let mut crusher = Building::new(Kind::Crusher, AT);
        let wanted = [0, 16, 64, 144, 256, i32::MAX];
        for (level, expected) in wanted.iter().enumerate() {
            if level > 0 {
                crusher.complete(level as i32).expect("a level of 1 or more");
            }
            assert_eq!(
                Hut::new(&crusher).daily_limit(),
                Some(*expected),
                "crusher at level {level}"
            );
        }
        // A sifter counts 64 per level squared (BuildingSifter.java:79-88).
        assert_eq!(Hut::new(&built(Kind::Sifter, 2)).daily_limit(), Some(256));
        assert_eq!(Hut::new(&built(Kind::Sifter, 4)).daily_limit(), Some(1024));
        assert_eq!(
            Hut::new(&built(Kind::Sifter, MAX_BUILDING_LEVEL)).daily_limit(),
            Some(i32::MAX)
        );
        // Nobody else has a daily limit.
        assert_eq!(Hut::new(&built(Kind::Baker, 3)).daily_limit(), None);
        assert_eq!(Hut::new(&built(Kind::StoneMason, 3)).daily_limit(), None);
        assert_eq!(Hut::new(&Building::new(Kind::Crusher, AT)).daily_limit(), Some(0));
    }

    #[test]
    fn a_florist_filters_one_flower_then_five_and_then_anything() {
        let florist = |level: i32| -> Building { built(Kind::Florist, level) };
        // Levels 1 to 3: a single flower, and the second is Java's
        // TOO_MANY_FILTERED_BELOW_LVL4_FLORIST.
        for level in 1..=3 {
            let holding = florist(level);
            let hut = Hut::new(&holding);
            assert_eq!(hut.may_filter_flower(0), Ok(()));
            assert_eq!(
                hut.may_filter_flower(1),
                Err(Refusal::FlowersNotSortedYet { level }),
                "level {level}"
            );
        }
        // Level 4: five.
        let four = florist(4);
        let hut = Hut::new(&four);
        assert_eq!(hut.may_filter_flower(4), Ok(()));
        assert_eq!(
            hut.may_filter_flower(5),
            Err(Refusal::FlowersNotFilteredAtThisLevel { level: 4, limit: 5 })
        );
        // Level 5: no bound the port states.
        let five = florist(5);
        let hut = Hut::new(&five);
        assert_eq!(hut.may_filter_flower(40), Ok(()));
        // A hut with no flower filter at all.
        assert_eq!(Hut::new(&built(Kind::Baker, 3)).flower_filter_limit(), None);
        assert!(Hut::new(&built(Kind::Baker, 3)).may_filter_flower(0).is_err());
    }

    #[test]
    fn a_guard_tower_claims_further_at_every_rung_and_nobody_else_does() {
        let towers: Vec<i32> = (0..=5)
            .map(|level| guard_tower_claim_radius(level))
            .collect();
        assert_eq!(towers, vec![0, 2, 3, 3, 4, 5]);
        let others: Vec<i32> = (0..=5).map(default_claim_radius).collect();
        assert_eq!(others, vec![0, 1, 1, 1, 2, 2]);
        assert_eq!(claim_radius(Kind::GuardTower, 3), 3);
        assert_eq!(claim_radius(Kind::Baker, 3), 1);
        assert_eq!(Hut::new(&built(Kind::GuardTower, 4)).claim_radius(), 4);
        assert_eq!(Hut::new(&built(Kind::Residence, 4)).claim_radius(), 2);
        // A hut that is not built claims nothing, as Java's `default:` does.
        assert_eq!(claim_radius(Kind::Baker, 0), 0);
        assert_eq!(claim_radius(Kind::GuardTower, 6), 0);
    }

    #[test]
    fn the_usable_question_runs_both_gates_in_order() {
        let building = built(Kind::GuardTower, 2);
        let hut = Hut::new(&building);
        // Gate first, level second: a marksman at level 2 with no research fails
        // the research even though no level rule applies to him.
        assert!(hut.usable(ModuleKey::MarksmanTowerWork, none_known).is_err());
        assert_eq!(hut.usable(ModuleKey::MarksmanTowerWork, all_known), Ok(()));
        // A baker below three fails the level gate whatever the research says.
        let bakery = built(Kind::Baker, 2);
        assert_eq!(
            Hut::new(&bakery).usable(ModuleKey::BakerCraft, all_known),
            Err(Refusal::LevelTooLow {
                kind: Kind::Baker,
                key: ModuleKey::BakerCraft,
                level: 2,
                needed: 3
            })
        );
        // A module with neither gate is usable as soon as it is registered.
        assert_eq!(hut.usable(ModuleKey::MinStock, none_known), Ok(()));
    }

    #[test]
    fn a_hut_answers_its_own_kind_and_reads_through_to_the_record() {
        let building = built(Kind::Fisherman, 4);
        let hut = Hut::new(&building);
        assert_eq!(hut.kind(), Kind::Fisherman);
        assert_eq!(hut.level(), 4);
        assert_eq!(hut.building().at(), AT);
        assert_eq!(hut.attached().len(), 0, "nothing was attached to this record");
        assert!(hut.accepts(ModuleKey::FisherWork));
        assert!(!hut.has_attached(ModuleKey::FisherWork));
    }

    #[test]
    fn a_module_attached_to_the_record_shows_through_the_hut() {
        let mut building = built(Kind::Baker, 3);
        attach(&mut building, ModuleKey::Furnace, 3).expect("a bakery has a furnace");
        let hut = Hut::new(&building);
        assert!(hut.has_attached(ModuleKey::Furnace));
        assert_eq!(hut.attached().len(), 1);
        assert_eq!(
            building.module("furnace").map(Module::level),
            Some(3),
            "the level attached is the level read back"
        );
        assert_eq!(hut.attached()[0].key(), "furnace");
    }

    #[test]
    fn a_deconstructed_hut_holds_nothing_even_at_the_top_level() {
        // `isDeconstructed` means the structure is gone
        // (`AbstractSchematicProvider.java:142-145`), so `canAssignCitizens` is
        // false even with `isBuilt` still true.
        let mut building = built(Kind::Residence, 5);
        assert_eq!(Hut::new(&building).seats(ModuleKey::Living), Ok(5));
        building.deconstruct();
        let hut = Hut::new(&building);
        assert!(!hut.stands());
        assert_eq!(
            hut.seats(ModuleKey::Living),
            Err(Refusal::HutUnderConstruction {
                kind: Kind::Residence,
                level: 5
            })
        );
    }

    #[test]
    fn a_hut_of_level_one_answers_the_first_rung_of_every_formula() {
        let residence = built(Kind::Residence, 1);
        let hut = Hut::new(&residence);
        assert_eq!(hut.seats(ModuleKey::Living), Ok(1));
        // The residence carries no minimum stock: only the home, the living
        // module and the beds (`ModBuildingsInitializer.java:250-252`).
        assert_eq!(
            hut.seats(ModuleKey::MinStock),
            Err(Refusal::ModuleNotOnThisHut {
                kind: Kind::Residence,
                key: ModuleKey::MinStock
            })
        );

        let baker = built(Kind::Baker, 1);
        let hut = Hut::new(&baker);
        assert_eq!(hut.seats(ModuleKey::MinStock), Ok(5));
        assert_eq!(hut.seats(ModuleKey::Furnace), Ok(64), "one stack per level");
        assert_eq!(hut.capacity(ModuleKey::BakerCraft), Capacity::Recipes);
        assert_eq!(hut.seats(ModuleKey::BakerCraft), Ok(2), "2^1 recipes");
    }

    #[test]
    fn the_five_kinds_that_hire_nobody_answer_no_hiring_question() {
        for kind in [
            Kind::Residence,
            Kind::TownHall,
            Kind::Postbox,
            Kind::Stash,
            Kind::MysticalSite,
        ] {
            let building = built(kind, 1);
            let hut = Hut::new(&building);
            assert_eq!(
                hut.hire_room(ModuleKey::MinStock, 0),
                Err(Refusal::NotAnAssignment {
                    key: ModuleKey::MinStock
                }),
                "{kind}"
            );
            assert_eq!(
                hut.hire_room(ModuleKey::StatsModule, 0),
                Err(Refusal::NotAnAssignment {
                    key: ModuleKey::StatsModule
                }),
                "{kind}"
            );
        }
    }
}
