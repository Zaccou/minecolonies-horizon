//! What the goals that are **not** a trade may read.
//!
//! Java reference: the whole of `core/entity/ai/` minus `workers/*Work*` and minus
//! the base classes `src/ai` already ported. The goals here are the ones a citizen
//! runs *as a citizen* — eating, sleeping, being sick, mourning, wandering,
//! growing up, fleeing, looking around, opening a gate — plus the combat layer
//! (`combat/TargetAI`, `combat/AttackMoveAI`) and the guard layer built on it
//! (`workers/guard/`), the training academies, the cavalry mount goals and the
//! visitor.
//!
//! **No `mod_api`.** Like [`crate::ai`], this module is data and rules only: it
//! does not tick, does not path, does not place a block. The rules that would need
//! to *act* are named in `MANQUANT.md` with the primitive that is missing.
//!
//! **Three premises of the brief are wrong, and the facts below are shaped by the
//! correction** (`RAPPORT.md` §1):
//!
//! - there is **no** `EntityAIChaseCitizen.java` in this tree;
//! - there is **no** `EntityAIPlayerAction.java` either;
//! - there is **no** `entity/ai/targeting/` package: targeting is
//!   `combat/TargetAI.java`, and the "guard and raid goals" are
//!   `workers/guard/AbstractEntityAIGuard.java` + `combat/AttackMoveAI.java`.
//!
//! What the brief called "goals of construction" also does not exist as a separate
//! file: construction goals are the builder's, and `src/ai/builder.rs` already
//! ports them. Nothing here duplicates that.
//!
//! **Randomness is a fact, never a draw.** `getRandom()` appears in almost every
//! rule below (`shouldSleep`, `decide`, `stare`, `searchRestaurant`). A decision
//! that rolls its own dice cannot be replayed, so the roll is handed in — see
//! [`CitizenFacts::roll`] and [`GuardFacts::roll`].

use crate::ai::facts::Pos;

/// A number Java declares as a `static final`, with the line it comes from.
///
/// The brief asks every constant to carry its source. A bare `const X: i32` cannot:
/// once the file is read, the provenance is gone. [`Tuned`] keeps both, so a test
/// can assert the value **and** print where it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuned {
    /// The Java value, as written.
    pub value: i64,
    /// `fichier.java:ligne`.
    pub source: &'static str,
}

/// The same, for a constant Java writes as a `double`
/// (`RESTAURANT_LIMIT = 2.5`, `DEFAULT_SPEED = 0.6D`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunedF {
    /// The Java value, as written.
    pub value: f64,
    /// `fichier.java:ligne`.
    pub source: &'static str,
}

impl TunedF {
    /// Write a constant down with its provenance.
    pub const fn new(value: f64, source: &'static str) -> TunedF {
        TunedF { value, source }
    }

    /// The value.
    pub const fn v(self) -> f64 {
        self.value
    }
}

impl Tuned {
    /// Write a constant down with its provenance.
    pub const fn new(value: i64, source: &'static str) -> Tuned {
        Tuned { value, source }
    }

    /// The value as the integer Java uses.
    pub const fn v(self) -> i64 {
        self.value
    }

    /// The value as a tick count or an index.
    pub const fn u32(self) -> u32 {
        self.value as u32
    }

    /// The value as a distance or a chance.
    pub const fn f64(self) -> f64 {
        self.value as f64
    }
}

/// Which trade the citizen holds, as far as a common goal cares.
///
/// Java reads the *class* of the job, never a flag:
/// `citizenData.getJob() instanceof JobCook` (`EntityAIEatTask.java:435`),
/// `instanceof JobCavalry` (`:454`), `instanceof AbstractJobGuard` (`:297`),
/// `instanceof AbstractJobGuard` again in the wander rule
/// (`EntityAICitizenWander.java:296`). The port keeps the same six answers and
/// nothing else: a goal that needed a seventh would say so in `MANQUANT.md`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the six job classes the common goals branch on
pub enum JobKind {
    /// No job at all (`citizenData.getJob() == null`).
    #[default]
    None,
    /// Any job that is not one of the five below (`AbstractJob`).
    Worker,
    /// `JobCook` — may feed itself and never queues for food (`:435-439`).
    Cook,
    /// `AbstractJobGuard<?>` — sleeps standing, eats at night, no leisure
    /// (`EntityAIEatTask.java:297`, `EntityAICitizenWander.java:296`).
    Guard,
    /// `JobCavalry` — dismounts before entering a dining hall
    /// (`EntityAIEatTask.java:454`).
    Cavalry,
    /// `JobDruid` — a guard, but its own combat AI (`EntityAIDruid.java:32`).
    Druid,
}

impl JobKind {
    /// `getJob() instanceof AbstractJobGuard` — true for a knight, a ranger, a
    /// druid and a cavalryman.
    pub fn is_guard(self) -> bool {
        matches!(
            self,
            JobKind::Guard | JobKind::Cavalry | JobKind::Druid
        )
    }

    /// `getJob() instanceof JobCook`.
    pub fn is_cook(self) -> bool {
        self == JobKind::Cook
    }

    /// `getJob() instanceof JobCavalry`.
    pub fn is_cavalry(self) -> bool {
        self == JobKind::Cavalry
    }

    /// Whether a job is held at all: `citizenData.getJob() != null`.
    pub fn has_job(self) -> bool {
        self != JobKind::None
    }
}

/// `core/datalistener/model/Disease.java` — a name and the stacks that cure it.
///
/// Java keeps `ItemStorage` objects (`:250`); the port keeps the item names,
/// because only the count and the presence matter to the rule
/// (`EntityAISickTask.java:457-470`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Disease {
    /// `disease.name()`, shown in the interaction (`:425`).
    pub name: &'static str,
    /// `disease.cureItems()`, one stack each.
    pub cure_items: Vec<&'static str>,
}

impl Disease {
    /// A disease with `cures` cure items.
    pub fn new(name: &'static str, cures: &[&'static str]) -> Disease {
        Disease {
            name,
            cure_items: cures.to_vec(),
        }
    }

    /// `cureItems().isEmpty()` — `EntityAISickTask.applyCure` only puts a stack in
    /// hand when the list is not empty (`:250-254`).
    pub fn has_cure(&self) -> bool {
        !self.cure_items.is_empty()
    }
}

/// The colony's research effects, as strengths.
///
/// Java reads `getResearchManager().getResearchEffects().getEffectStrength(KEY)`
/// everywhere; the keys this layer uses are the ones the common goals and the
/// guards branch on. `api/research/util/ResearchConstants.java` names them; each
/// field below says which rule reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Research {
    /// `SLEEP_LESS` — `shouldSleep` divides by `1 + strength`
    /// (`AbstractEntityAIGuard.java:275`).
    pub sleep_less: f64,
    /// `RETREAT` — a guard may only flee with it (`:357`).
    pub retreat: f64,
    /// `FLEEING_SPEED` — the speed effect while fleeing (`:396`).
    pub fleeing_speed: f64,
    /// `SHIELD_USAGE` — gates the shield and its banner (`:122`, `:157`).
    pub shield_usage: f64,
    /// `KNIGHT_TAUNT` — a knight pulls the mob's aggro (`MeleeCombatAI.java:234`).
    pub knight_taunt: f64,
    /// `KNIGHT_WHIRLWIND` — the area knockback (`:261`).
    pub knight_whirlwind: f64,
    /// `MELEE_DAMAGE` — flat melee bonus (`:337`).
    pub melee_damage: f64,
    /// `GUARD_CRIT` — 1-in-(1+strength) chance to crit (`:345`).
    pub guard_crit: f64,
    /// `DOUBLE_ARROWS` — chance of a second arrow (`RangeCombatAI.java:183`).
    pub double_arrows: f64,
    /// `ARROW_PIERCE` — pierce level 2 (`:195`).
    pub arrow_pierce: f64,
    /// `ARCHER_DAMAGE` — flat ranged bonus (`:279`).
    pub archer_damage: f64,
    /// `ARCHER_USE_ARROWS` — consumes a real arrow, +2 damage (`:281`).
    pub archer_use_arrows: f64,
    /// `DRUID_USE_POTIONS` — the druid may throw magic potions
    /// (`DruidCombatAI.java:145`).
    pub druid_use_potions: f64,
    /// `GROWTH` — the child growth modifier (`EntityAICitizenChild.java:281`).
    pub growth: f64,
}

/// What a guard is doing, i.e. `GuardTaskSetting`
/// (`colony/buildings/modules/settings/GuardTaskSetting.java`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
// exhaustive: four tasks in Java
pub enum GuardTask {
    /// `GUARD` — stay near the hut.
    #[default]
    Guard,
    /// `PATROL` — walk the patrol points.
    Patrol,
    /// `FOLLOW` — follow a player.
    Follow,
    /// `PATROL_MINE` — patrol the mine's completed nodes.
    PatrolMine,
}

impl GuardTask {
    /// Java's identifier.
    pub fn java_name(self) -> &'static str {
        match self {
            GuardTask::Guard => "GUARD",
            GuardTask::Patrol => "PATROL",
            GuardTask::Follow => "FOLLOW",
            GuardTask::PatrolMine => "PATROL_MINE",
        }
    }
}

/// The citizen itself: the numbers every common goal reads.
#[derive(Clone, Debug, PartialEq)]
pub struct CitizenFacts {
    /// Where the citizen is.
    pub at: Pos,
    /// `citizenData.getSaturation()`, 0..[`MAX_SATURATION`].
    pub saturation: f64,
    /// `citizen.getHealth()`.
    pub health: f64,
    /// `citizen.getMaxHealth()`.
    pub max_health: f64,
    /// `citizenData.isChild()`.
    pub child: bool,
    /// `citizen.isBaby()` — `EntityAICitizenWander.canUse` refuses babies
    /// (`:295`), `EntityAICitizenChild.tryGrowUp` needs them (`:266`).
    pub baby: bool,
    /// `getCitizenDiseaseHandler().isSick()`.
    pub sick: bool,
    /// `getCitizenDiseaseHandler().isHurt()` (`EntityAISickTask.java:140`).
    pub hurt: bool,
    /// `getCitizenSleepHandler().isAsleep()`.
    pub asleep: bool,
    /// `citizen.getVehicle() != null`.
    pub riding: bool,
    /// `citizenData.isWorking()` (`EntityAIEatTask.java:495`).
    pub working: bool,
    /// The trade.
    pub job: JobKind,
    /// `citizen.getNavigation().isDone()`.
    pub nav_done: bool,
    /// `WorldUtil.isDayTime(citizen.level)` (`EntityAIEatTask.java:298`).
    pub daytime: bool,
    /// The first `getRandom()` answer the caller made, as `nextInt(denominator)`.
    pub roll: i32,
    /// The second draw, as `nextDouble()` in `[0,1)`.
    pub chance: f64,
    /// `getCitizenData().getCitizenSkillHandler().getLevel(Skill.Adaptability)`.
    pub adaptability: i32,
}

impl Default for CitizenFacts {
    fn default() -> CitizenFacts {
        CitizenFacts {
            at: Pos::new(0, 64, 0),
            // `ICitizenData.MAX_SATURATION` (`:30`) — a well-fed citizen, so a
            // test that does not set it is not accidentally starving.
            saturation: MAX_SATURATION.f64(),
            health: 20.0,
            max_health: 20.0,
            child: false,
            baby: false,
            sick: false,
            hurt: false,
            asleep: false,
            riding: false,
            working: false,
            job: JobKind::Worker,
            nav_done: true,
            daytime: true,
            roll: 0,
            chance: 0.0,
            adaptability: 1,
        }
    }
}

/// `ICitizenData.MAX_SATURATION` (`:30`).
pub const MAX_SATURATION: Tuned = Tuned::new(60, "colony/ICitizenData.java:30");
/// `CitizenConstants.AVERAGE_SATURATION` (`:43`).
pub const AVERAGE_SATURATION: Tuned =
    Tuned::new(10, "api/util/constant/CitizenConstants.java:43");
/// `CitizenConstants.SEEK_DOCTOR_HEALTH` (`citizenhandlers/CitizenDiseaseHandler.java:36`).
pub const SEEK_DOCTOR_HEALTH: Tuned =
    Tuned::new(6, "core/entity/citizen/citizenhandlers/CitizenDiseaseHandler.java:36");
/// `CitizenConstants.RANGE_TO_BE_HOME` (`:113`) — compared **squared** at
/// `EntityAISleep.java:120`.
pub const RANGE_TO_BE_HOME: Tuned =
    Tuned::new(16, "api/util/constant/CitizenConstants.java:113");
/// `CitizenConstants.MAX_GUARD_CALL_RANGE` (`:133`).
pub const MAX_GUARD_CALL_RANGE: Tuned =
    Tuned::new(90000, "api/util/constant/CitizenConstants.java:133");
/// `Constants.DEFAULT_SPEED` (`:63`).
pub const DEFAULT_SPEED: Tuned = Tuned::new(60, "api/util/constant/Constants.java:63");

/// The home building and the bed inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct HomeFacts {
    /// `citizenData.getHomeBuilding()`, `None` when it has none.
    pub building: Option<Pos>,
    /// `homeBuilding.isInBuilding(citizen.blockPosition())`.
    pub in_building: bool,
    /// `citizenData.getHomePosition()` — the fallback when no bed is registered
    /// (`EntityAISleep.java:119`).
    pub home_pos: Pos,
    /// `hut.getModule(BuildingModules.BED).getRegisteredBlocks()`.
    pub beds: Vec<Pos>,
    /// The index of this citizen in the hut's assigned list — which bed is *its*
    /// bed (`EntityAISleep.java:168`).
    pub bed_index: i32,
    /// The loaded-ness of each bed, same order as [`HomeFacts::beds`].
    pub beds_loaded: Vec<bool>,
    /// `state.is(BlockTags.BEDS)` per bed, same order.
    pub bed_states_valid: Vec<bool>,
    /// `state.getValue(BedBlock.PART) == HEAD` per bed, same order.
    pub bed_is_head: Vec<bool>,
    /// The block above each bed is a bed, a panel, a trapdoor or not solid
    /// (`:185`), same order.
    pub bed_has_headroom: Vec<bool>,
    /// `homeBuilding.isInBuilding(citizen.blockPosition())` for the **work**
    /// building: `EntityAIEatTask.goToHut` walks there first (`:402-406`), and
    /// `EntityAISickTask.goToHut` too (`:381-384`).
    pub arrived_at_work: bool,
    /// `WorldUtil.isBlockLoaded(level, buildingWorker.getPosition())`
    /// (`EntityAIEatTask.java:396`): a hut whose chunk is unloaded cannot be
    /// searched, so the citizen goes to a restaurant instead.
    pub work_loaded: bool,
    /// Whether the chosen bed is `OCCUPIED` by another sleeping entity
    /// (`:200-206`).
    pub bed_occupied_by_other: bool,
    /// The result of `getCitizenSleepHandler().trySleep(usedBed)`.
    pub try_sleep_ok: bool,
}

impl Default for HomeFacts {
    fn default() -> HomeFacts {
        HomeFacts {
            building: None,
            in_building: false,
            home_pos: Pos::new(0, 64, 0),
            beds: Vec::new(),
            bed_index: -1,
            beds_loaded: Vec::new(),
            bed_states_valid: Vec::new(),
            bed_is_head: Vec::new(),
            bed_has_headroom: Vec::new(),
            arrived_at_work: false,
            // A chunk is normally loaded: a test that cares says otherwise.
            work_loaded: true,
            bed_occupied_by_other: false,
            try_sleep_ok: false,
        }
    }
}

impl HomeFacts {
    /// Whether the citizen has no home building: `homeBuilding == null`.
    pub fn homeless(&self) -> bool {
        self.building.is_none()
    }

    /// The bed this citizen is assigned, if the index is in range
    /// (`EntityAISleep.java:170`).
    pub fn assigned_bed(&self) -> Option<Pos> {
        if self.bed_index < 0 {
            return None;
        }
        self.beds.get(self.bed_index as usize).copied()
    }

    /// Whether a bed at `index` passes every test `findBedAndTryToSleep` makes
    /// (`:173-190`): loaded, a bed, a head, and room above.
    pub fn bed_usable(&self, index: usize) -> bool {
        let slot = |v: &Vec<bool>| v.get(index).copied().unwrap_or(false);
        slot(&self.beds_loaded)
            && slot(&self.bed_states_valid)
            && slot(&self.bed_is_head)
            && slot(&self.bed_has_headroom)
    }
}

/// The food a citizen can reach: its own inventory, and the hut's.
#[derive(Clone, Debug, PartialEq)]
pub struct FoodFacts {
    /// `FoodUtils.getBestFoodForCitizen(...)`, or `None` for Java's `-1`.
    pub best_slot: Option<usize>,
    /// How many stacks in the inventory are eatable
    /// (`InventoryUtils.hasItemInItemHandler(..., ISCOOKABLE)` and friends).
    pub eatable: usize,
    /// How many stacks are only cookable — the `RAW_FOOD` interaction
    /// (`EntityAIEatTask.java:541`).
    pub cookable: usize,
    /// A stack in the hut's chest to take, from
    /// `FoodUtils.checkForFoodInBuilding` (`:424`).
    pub hut_stack: Option<&'static str>,
    /// A stack in the restaurant's menu (`:266`).
    pub restaurant_stack: Option<&'static str>,
    /// The item in the slot `best_slot` names — `foodStack.getItem()` in `eat`
    /// (`:192`, `:220-222`). Distinct from [`FoodFacts::restaurant_stack`], which is
    /// the stack the citizen would *take* from a cook.
    pub best_item: Option<&'static str>,
    /// The food value of the stack that would be taken
    /// (`FoodUtils.getFoodValue`, `:269`).
    pub stack_food_value: f64,
    /// How many items are left in the food slot *after* one bite — Java asks
    /// `!getStackInSlot(foodSlot).isEmpty()` before eating again
    /// (`:227`).
    pub stack_left: usize,
    /// `FoodUtils.canEat(stack, homeBuilding, workBuilding)` for the chosen stack
    /// (`:193`): a citizen's home and hut can forbid a food it would otherwise
    /// pick, and Java then goes back to `CHECK_FOR_FOOD`.
    pub can_eat: bool,
}

impl Default for FoodFacts {
    fn default() -> FoodFacts {
        FoodFacts {
            best_slot: None,
            eatable: 0,
            cookable: 0,
            hut_stack: None,
            restaurant_stack: None,
            best_item: None,
            stack_food_value: 0.0,
            stack_left: 0,
            // A best food is eatable unless the hut forbids it.
            can_eat: true,
        }
    }
}

/// The restaurant the citizen heads for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestaurantFacts {
    /// The **staffed** lookup,
    /// `getBestBuilding(searchFrom, BuildingCook.class, STAFFED_RESTAURANTS)`
    /// (`:488`), `None` for `null`.
    pub pos: Option<Pos>,
    /// The second lookup, `getBestBuilding(searchFrom, BuildingCook.class)` with no
    /// predicate (`:491`): the citizen tries a **staffed** cook first (`:488`) and
    /// only then any cook.
    pub fallback: Option<Pos>,
    /// `restaurant.isInBuilding(citizen.blockPosition())` (`:362`).
    pub we_are_inside: bool,
    /// `((BuildingCook) restaurant).getNextSittingPosition()` (`:338`).
    pub sitting_pos: Option<Pos>,
    /// `restaurantPos.distManhattan(citizen.blockPosition()) <
    /// JobCavalry.DININGHALL_HORSE_PARKING_RANGE` (`:454`).
    pub within_parking_range: bool,
    /// The result of `EntityNavigationUtils.walkToBuilding(citizen, building)`
    /// (`:466`): `true` when the walk finished, which is the only case where Java
    /// falls through to `SEARCH_RESTAURANT` instead of `GO_TO_RESTAURANT`.
    pub walk_arrived: bool,
}

/// The hospital a sick citizen heads for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HospitalFacts {
    /// `getBestBuilding(citizen, BuildingHospital.class)`.
    pub pos: Option<Pos>,
    /// `((BuildingHospital) hospital).getBedList()`.
    pub beds: Vec<Pos>,
    /// The citizen's current `getBedPos()`, `None` for `BlockPos.ZERO`.
    pub our_bed: Option<Pos>,
    /// The bed state flags, same order as [`HospitalFacts::beds`]: is a bed, is
    /// not occupied, is the head, has an empty block above
    /// (`EntityAISickTask.java:194-197`).
    pub beds_free: Vec<bool>,
    /// `hasCureItem` per cure item of the disease: `true` when a slot holds it
    /// (`:457-470`).
    pub cure_present: Vec<bool>,
    /// `getCitizenSleepHandler().getBedLocation()` is in the hospital's bed list
    /// (`:346`).
    pub asleep_bed_registered: bool,
    /// The result of `trySleep(usedBed)`.
    pub try_sleep_ok: bool,
}

/// The town hall, the graveyard and the graves a mourner visits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MournFacts {
    /// Where the mourner is.
    pub at: Pos,
    /// `colony.getServerBuildingManager().getTownHall()`.
    pub town_hall: Option<Pos>,
    /// The fallback `getMournLocation`: the home building (`:344`).
    pub home: Option<Pos>,
    /// A graveyard with a resting citizen among the deceased (`:299-305`).
    pub graveyard: Option<Pos>,
    /// The named graves of the graveyard: `(pos, textLines)`
    /// (`TileEntityNamedGrave.getTextLines`, `:198`).
    pub named_graves: Vec<(Pos, Vec<&'static str>)>,
    /// `getCitizenMournHandler().getDeceasedCitizens()`.
    pub deceased: Vec<&'static str>,
    /// The nearest other citizen within three blocks (`:257-263`).
    pub nearest_citizen: Option<Pos>,
    /// `citizen.getNavigation().isDone()`.
    pub nav_done: bool,
    /// `citizen.getRandom().nextBoolean()`.
    pub coin: bool,
    /// `citizen.getRandom().nextInt(100)` for the graveyard wander (`:174`).
    pub roll: i32,
    /// `MathUtils.RANDOM.nextInt(200)` for the stare timeout (`:249`).
    pub stare_roll: i32,
}

impl MournFacts {
    /// `getMournLocation()` (`:336-345`): the town hall when the colony has one,
    /// the home building otherwise.
    pub fn mourn_location(&self) -> Option<Pos> {
        self.town_hall.or(self.home)
    }

    /// The first named grave whose lines contain both halves of the first
    /// deceased citizen's name (`:189-203`).
    ///
    /// `StringUtils.split(deathBud)[0]` is the first name and
    /// `deathBud.replaceFirst(firstName, "")` the rest; both must appear in the
    /// grave's lines. The port splits on the first space, which is what
    /// `StringUtils.split` does for a two-word name.
    pub fn grave_of_deceased(&self) -> Option<Pos> {
        let full = self.deceased.first()?;
        let (first, last) = match full.split_once(' ') {
            Some((first, last)) => (first, last),
            None => (*full, ""),
        };
        self.named_graves
            .iter()
            .find(|(_, lines)| {
                lines.iter().any(|line| *line == first)
                    && (last.is_empty() || lines.iter().any(|line| *line == last))
            })
            .map(|(pos, _)| *pos)
    }
}

/// What kind of leisure site a wanderer found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the two building types the rule names, plus "anything else"
pub enum SiteKind {
    /// No site at all.
    #[default]
    None,
    /// `BuildingLibrary` — may be read a book (`EntityAICitizenWander.java:120`).
    Library,
    /// `BuildingUniversity` (`:125`).
    University,
    /// Any other hut: sit or stand, nothing to read.
    Other,
}

/// One seat or standing spot of a leisure site, from the blueprint tags
/// (`SchematicTagConstants`: `SITTING`, `INSIDE_SITTING`, `OUTSIDE_SITTING`,
/// `STANDING`, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    /// Where it is.
    pub pos: Pos,
    /// `INSIDE_*` — under a roof, chosen when it rains (`:191-193`).
    pub inside: bool,
    /// `*_SITTING` — a seat rather than a standing spot (`:196`).
    pub sitting: bool,
    /// `SittingEntity.isSittingPosOccupied(walkTo, level)` (`:196`).
    pub occupied: bool,
}

/// The idle wanderer: leisure site, seats, and where it already goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WanderFacts {
    /// Where the citizen is.
    pub at: Pos,
    /// `canUse()` (`:293-297`): navigation done, not a baby, not a guard.
    pub nav_done: bool,
    /// `citizen.isBaby()`.
    pub baby: bool,
    /// `getJob() instanceof AbstractJobGuard`.
    pub guard_job: bool,
    /// `getRandomLeisureSite()`, with the home and the colony centre as the
    /// fallbacks (`:269-279`).
    pub leisure_site: Option<Pos>,
    /// The kind of building at [`WanderFacts::leisure_site`].
    pub site_kind: SiteKind,
    /// The seats and standing spots of that site.
    pub seats: Vec<Seat>,
    /// `citizen.getRandom().nextInt(100)` for `LEISURE_CHANCE` (`:266`).
    pub roll: i32,
    /// `citizen.getRandom().nextInt(60 * 5)` for the wander timeout (`:152`).
    pub wander_roll: i32,
    /// `citizen.getRandom().nextInt(10) <= 0` — whether this look is the one in ten
    /// that walks at random inside the hut's corners instead of picking a spot
    /// (`:167`).
    pub spot_roll: i32,
    /// `citizen.getRandom().nextInt(size)` — which seat or standing spot is taken
    /// (`:195`, `:207`, `:214`, `:226`, `:232`).
    pub choice_roll: i32,
    /// `library.getRandomBookShelf()` / `university.getRandomBookShelf()`
    /// (`:122`, `:126`): the shelf the citizen walks to before reading.
    pub book_shelf: Option<Pos>,
    /// `citizen.getRandom().nextInt(100)` for the book timeout (`:103`).
    pub book_roll: i32,
    /// `citizen.level.isRaining()` (`:191`).
    pub raining: bool,
    /// `MathUtils.RANDOM.nextBoolean()` for the inside/outside coin (`:191`).
    pub coin: bool,
    /// What the citizen currently walks to (`walkTo`), if anything.
    pub walk_to: Option<Pos>,
    /// `EntityNavigationUtils.walkToPos(citizen, walkTo, 3, true)` succeeded.
    pub arrived: bool,
}

impl WanderFacts {
    /// The seats that are free and match the weather preference, in the order the
    /// rule tries them: inside seats first when it rains, then any seat
    /// (`:191-235`).
    pub fn candidate_seats(&self) -> Vec<Pos> {
        self.seats
            .iter()
            .filter(|seat| seat.sitting && !seat.occupied)
            .filter(|seat| !self.raining || seat.inside)
            .map(|seat| seat.pos)
            .collect()
    }

    /// The standing spots, used when no seat is free (`:205`, `:224`).
    pub fn candidate_standing(&self) -> Vec<Pos> {
        self.seats
            .iter()
            .filter(|seat| !seat.sitting && !seat.occupied)
            .filter(|seat| !self.raining || seat.inside)
            .map(|seat| seat.pos)
            .collect()
    }
}

/// The child's game: following someone, visiting huts, growing up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildFacts {
    /// Where the child is.
    pub at: Pos,
    /// `child.isBaby()` — `canUse` (`EntityAICitizenChild.java:310`).
    pub baby: bool,
    /// `actionTimer` before this tick, in ticks.
    pub action_timer: i32,
    /// `aiActiveTime` — the clock that eventually grows the child up (`:279`).
    pub active_time: i32,
    /// The first civilian or player within `START_FOLLOW_DISTANCE` (`:156-176`).
    pub follow_candidate: Option<Pos>,
    /// `getServerBuildingManager().getBuildings()` — the huts to visit (`:226`).
    pub buildings: Vec<Pos>,
    /// `colony.getCitizenManager().getCitizens().size()` — decides whether the
    /// colony is told the child grew up (`:290`).
    pub colony_citizens: usize,
    /// `rand.nextInt(10) == 0` for `isReadyForActivity` (`:134`).
    pub ready_roll: i32,
    /// `rand.nextInt(2 * 60 * 20)` — the 3-to-5-minute delay (`:148`).
    pub delay_roll: i32,
    /// `rand.nextInt(30 * 20)` — the 30-to-60-second follow time (`:181`).
    pub follow_roll: i32,
    /// `rand.nextInt((int) (70 / growthModifier) + 1)` (`:284`).
    pub grow_roll: i32,
    /// `colony.useAdditionalChildTime(BONUS_TIME_COLONY)` (`:273`).
    pub colony_granted_time: bool,
}

/// A citizen running from something.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FleeFacts {
    /// Where the citizen is.
    pub at: Pos,
    /// `citizen.getHealth()`.
    pub health: f64,
    /// `citizen.getMaxHealth()`.
    pub max_health: f64,
    /// The closest entity to avoid, if any (`getClosestToAvoid`, `:145-166`).
    pub closest: Option<Pos>,
    /// `closestLivingEntity.getId()` — a change triggers `callForHelp` (`:218`).
    pub closest_id: i32,
    /// `citizen.getNavigation().isDone()`.
    pub nav_done: bool,
    /// `moveAwayPath != null && moveAwayPath.isInProgress()` (`:175`).
    pub path_in_progress: bool,
    /// `citizen.distanceTo(closestLivingEntity)` (`:234`).
    pub distance: f64,
    /// `rand.nextInt(2)` for the small offset (`:178`).
    pub offset_roll: i32,
}

/// The look-at goal: is there something worth turning the head to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LookFacts {
    /// `ColonyConstants.rand.nextFloat() >= probability` — the goal never starts
    /// (`LookAtEntityGoal.java:51`).
    pub probability_roll: f32,
    /// `probability` (`DEFAULT_PROBABILITY` for the short constructor, `:30`).
    pub probability: f32,
    /// `mob.getTarget()`.
    pub target: Option<Pos>,
    /// The nearest entity of the looked-for class (`:62-71`).
    pub nearest: Option<Pos>,
    /// `lookDistance`.
    pub look_distance: f32,
    /// `lookAt.isAlive()`.
    pub alive: bool,
    /// `mob instanceof EntityCitizen && job instanceof AbstractJobGuard &&
    /// job.isAsleep()` (`:73-76`).
    pub guard_asleep: bool,
    /// `lookTime > 0` (`:95`).
    pub look_time: i32,
    /// `mob.distanceToSqr(lookAt)`.
    pub distance_sqr: f64,
}

/// What kind of toggle block a position holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three ToggleAble subclasses in Java
pub enum ToggleKind {
    /// `FenceToggle` — `FenceGateBlock` (`EntityAIInteractToggleAble.java:528`).
    Fence,
    /// `TrapToggle` — `TrapDoorBlock`, and only the ones it opened itself
    /// (`:552`, `:562-565`).
    TrapDoor,
    /// `DoorToggle` — `DoorBlock` (`:588`).
    Door,
}

impl ToggleKind {
    /// `onlyCloseYourOpens()` — `TrapToggle` is the only one that says yes
    /// (`:562-565`).
    pub fn closes_only_its_own(self) -> bool {
        self == ToggleKind::TrapDoor
    }
}

/// One block on the path that may have to be opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TogglePos {
    /// Where it is.
    pub pos: Pos,
    /// What it is.
    pub kind: ToggleKind,
    /// `state.hasProperty(BlockStateProperties.OPEN)` (`:386`).
    pub has_open: bool,
    /// `state.getBlock() == Blocks.AIR` (`:379`).
    pub is_air: bool,
    /// `state.getValue(BlockStateProperties.OPEN)` — `TrapToggle.canOpen` is
    /// `!OPEN` (`:556-559`).
    pub open: bool,
    /// `entity.distanceToSqr(pos)` — dropped above `MAX_DISTANCE²` (`:427`).
    pub distance_sqr: f64,
}

impl TogglePos {
    /// `isValidBlockState` (`:377-393`): not air, toggleable, and has `OPEN`.
    pub fn valid(&self) -> bool {
        !self.is_air && self.has_open
    }
}

/// The gate opener: collisions, timers and the blocks on the path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToggleFacts {
    /// `entity.isPassenger() && vehicle instanceof ICitizenJobMount` (`:121`).
    pub riding_mount: bool,
    /// `mount.hadHorizontalCollision()` (`:124`).
    pub mount_collided: bool,
    /// `entity.hadHorizontalCollission()` (`:132`).
    pub collided: bool,
    /// `entity.verticalCollision && !entity.onGround()` (`:132`).
    pub climbing: bool,
    /// The toggleable positions the path found (`toggleAblePositions`).
    pub positions: Vec<TogglePos>,
    /// `ColonyConstants.rand.nextInt(40 + offSet)` for the next update (`:405`).
    pub update_roll: i32,
    /// `entity.level.random.nextInt(posList.size())` — which block to open
    /// (`:448`).
    pub choice_roll: i32,
}

/// State of the path the combat AI gave itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// exhaustive: the four things `PathResult` can say
pub enum PathState {
    /// No path at all (`targetPath == null`).
    #[default]
    None,
    /// `isInProgress()`.
    InProgress,
    /// `isDone() && hasPath()`.
    Done,
    /// `isDone() && failedToReachDestination()`.
    Failed,
}

/// One entry of the threat table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    /// Where the entity is.
    pub pos: Pos,
    /// `getThreat()`.
    pub threat: i32,
    /// `getLastSeen()` — the game time the attacker last saw it.
    pub last_seen: i64,
    /// `entity.isAlive()`.
    pub alive: bool,
}

/// What a combat AI reads: the target, the path, the swing timer.
#[derive(Clone, Debug, PartialEq)]
pub struct CombatFacts {
    /// Where the AI's owner is.
    pub at: Pos,
    /// `user.level.getGameTime()`.
    pub game_time: i64,
    /// `user.getThreatTable().getTarget()`.
    pub threat_target: Option<Threat>,
    /// The target the AI currently holds (`target`), with its position.
    pub target: Option<Pos>,
    /// `user.distanceTo(target)`.
    pub distance: f64,
    /// `user.getSensing().hasLineOfSight(target)`.
    pub line_of_sight: bool,
    /// The vertical half-height of the search box, i.e. `getYSearchRange()`.
    /// `TargetAI` answers `Y_VISION` (`:208-211`), and a ranger or a druid on the
    /// `GUARD` task answers `Y_VISION + 25` (`RangeCombatAI.java:420-428`,
    /// `DruidCombatAI.java:336-345`).
    pub y_vision: i32,
    /// `nextAttackTime`.
    pub next_attack_time: i64,
    /// `getAttackDistance()`, which each subclass overrides.
    pub attack_distance: f64,
    /// `getAttackDelay()`, same.
    pub attack_delay: i32,
    /// `pathAttempts`.
    pub path_attempts: u32,
    /// `user.getNavigation().isDone()`.
    pub nav_done: bool,
    /// The `target.getPath().getTarget()` of the last path, for the
    /// re-path test at `AttackMoveAI.java:99`.
    pub path_target: Option<Pos>,
    /// `target instanceof FakePlayer` (`TargetAI.java:93`).
    pub fake_player: bool,
    /// `target == user.getLastHurtByMob()` (`:98`).
    pub last_hurt_by_mob: bool,
    /// `isAttackableTarget(target)`, the guard's own override.
    pub attackable: bool,
    /// `isWithinPersecutionDistance(target)` — a subclass hook.
    pub within_persecution: bool,
    /// `canAttack()` — for a guard, "is a weapon in hand".
    pub can_attack: bool,
    /// The search area's `getEntitiesOfClass` result count.
    pub entities: usize,
    /// The first entity of that scan that passed every test — the one
    /// `searchNearbyTarget` adds to the threat table (`TargetAI.java:162-166`).
    pub nearest_valid: Option<Pos>,
    /// `user.getRandom().nextInt(4) + 2` for `Direction.from3DDataValue`
    /// (`TargetAI.java:191`).
    pub direction_roll: i32,
    /// `getRandom().nextInt(FLEE_CHANCE) == 0` (`RangeCombatAI.java:167`).
    pub flee_roll: i32,
    /// `getRandom().nextDouble() < DOUBLE_ARROWS` (`:185`).
    pub double_arrow_roll: f64,
}

impl Default for CombatFacts {
    fn default() -> CombatFacts {
        CombatFacts {
            at: Pos::new(0, 64, 0),
            game_time: 0,
            threat_target: None,
            target: None,
            distance: 0.0,
            line_of_sight: false,
            y_vision: 3,
            next_attack_time: 0,
            attack_distance: 5.0,
            attack_delay: 40,
            path_attempts: 0,
            nav_done: true,
            path_target: None,
            fake_player: false,
            last_hurt_by_mob: false,
            attackable: true,
            within_persecution: true,
            can_attack: true,
            entities: 0,
            nearest_valid: None,
            direction_roll: 0,
            flee_roll: 1,
            double_arrow_roll: 1.0,
        }
    }
}

/// What a guard's own goals read, on top of [`CombatFacts`].
#[derive(Clone, Debug, PartialEq)]
pub struct GuardFacts {
    /// Where the guard is.
    pub at: Pos,
    /// `buildingGuards.getTask()`.
    pub task: GuardTask,
    /// `hasTool()` — every `toolsNeeded` has an item of the right level
    /// (`AbstractEntityAIGuard.java:611-621`).
    pub has_tool: bool,
    /// `worker.getHealth()`.
    pub health: f64,
    /// `worker.getMaxHealth()`.
    pub max_health: f64,
    /// `job.getCitizen().getCitizenDiseaseHandler().isSick()` (`:270`).
    pub sick: bool,
    /// `worker.getLastHurtByMob() != null`.
    pub last_hurt_by_mob: bool,
    /// `worker.getLastAttacker() != null` — the second interrupt test (`:778`).
    pub last_attacker: bool,
    /// The target the guard holds (`target`, `:93`).
    pub target: Option<Pos>,
    /// `fighttimer`.
    pub fight_timer: i32,
    /// `sleepTimer`.
    pub sleep_timer: i32,
    /// `wakeTimer`.
    pub wake_timer: i32,
    /// `regularActionTimer` (`:158`).
    pub regular_action_timer: i32,
    /// `buildingGuards.getRallyLocation()`.
    pub rally: Option<Pos>,
    /// Whether the rally location is reachable from the worker's location
    /// (`:707`).
    pub rally_reachable: bool,
    /// `buildingGuards.getPositionToFollow()`.
    pub follow_pos: Option<Pos>,
    /// `buildingGuards.isTightGrouping()` (`:446`).
    pub tight_grouping: bool,
    /// `buildingGuards.getNextPatrolTarget(false)`.
    pub patrol_point: Option<Pos>,
    /// `WorldUtil.isEntityBlockLoaded(world, currentPatrolPoint)`.
    pub patrol_loaded: bool,
    /// `buildingGuards.getMinePos()` (`:551`).
    pub mine_pos: Option<Pos>,
    /// `buildingGuards.getGuardPos(worker)`.
    pub guard_pos: Pos,
    /// `buildingGuards.getPosition()` — the hut's own block.
    pub building_at: Pos,
    /// `building.getBuildingLevelEquivalent()`.
    pub building_level: i32,
    /// `buildingGuards.shallRetrieveOnLowHealth()` (`:355`).
    pub retrieves_on_low_health: bool,
    /// `buildingGuards.requiresManualTarget()` (`:512`).
    pub manual_target: bool,
    /// `getTask() == GuardTaskSetting.GUARD` — the ranger's taller search box
    /// (`RangeCombatAI.java:422`).
    pub task_is_guard: bool,
    /// The thief threat table's own target distance, for `regen` (`:370`).
    pub threat_within_10: bool,
    /// `worker.hasEffect(MobEffects.REGENERATION)` (`:377`).
    pub has_regeneration: bool,
    /// `worker.hasEffect(MobEffects.MOVEMENT_SPEED)` (`:394`, `:470`).
    pub has_speed: bool,
    /// The citizen is walking to a building and has arrived (`walkToBuilding`,
    /// `:403`).
    pub arrived_at_building: bool,
    /// The patrol walk arrived (`walkToSafePos(currentPatrolPoint)`, `:514`,
    /// `:535`).
    pub arrived_at_patrol: bool,
    /// The rally walk arrived (`walkToUnSafePos`, `:464-468`).
    pub arrived_at_rally: bool,
    /// `buildingGuards.arrivedAtPatrolPoint(worker)` was already called.
    pub patrol_arrived_once: bool,
    /// The research effects this guard reads.
    pub research: Research,
    /// `worker.getCitizenData().getCitizenSkillHandler().getLevel(Skill.Adaptability)`
    /// — no field of this struct carried a skill level before, and this one is read
    /// twice: the nap chance divides the draw by `20 + Adaptability / 2`
    /// (`AbstractEntityAIGuard.java:278`) and the rally speed amplifies by
    /// `Adaptability / 30` clamped to 3 (`:477`).
    pub adaptability: i32,
    /// `getRandom().nextInt(...)` for `shouldSleep` (`:278`).
    pub sleep_roll: i32,
    /// `getRandom().nextDouble()` for the second half of `shouldSleep` (`:279`).
    pub sleep_chance: f64,
    /// `getRandom().nextInt(500) + 2500` — how long the nap lasts (`:282`).
    pub nap_roll: i32,
    /// `getRandom().nextDouble() < 0.05` — re-equip armour (`:674`).
    pub armour_roll: f64,
    /// `ColonyConstants.rand.nextInt(10) == 0` — the guard shuffle (`:427`).
    pub shuffle_roll: i32,
    /// The number of entities the search found, and how many are attackable.
    pub searched: usize,
}

impl Default for GuardFacts {
    fn default() -> GuardFacts {
        GuardFacts {
            at: Pos::new(0, 64, 0),
            task: GuardTask::Guard,
            has_tool: true,
            health: 20.0,
            max_health: 20.0,
            sick: false,
            last_hurt_by_mob: false,
            last_attacker: false,
            target: None,
            fight_timer: 0,
            sleep_timer: 0,
            wake_timer: 0,
            regular_action_timer: 0,
            rally: None,
            rally_reachable: false,
            follow_pos: None,
            tight_grouping: true,
            patrol_point: None,
            patrol_loaded: true,
            mine_pos: None,
            guard_pos: Pos::new(0, 64, 0),
            building_at: Pos::new(0, 64, 0),
            building_level: 1,
            retrieves_on_low_health: false,
            manual_target: false,
            task_is_guard: true,
            threat_within_10: false,
            has_regeneration: false,
            has_speed: false,
            arrived_at_building: false,
            arrived_at_patrol: false,
            arrived_at_rally: false,
            patrol_arrived_once: false,
            research: Research::default(),
            adaptability: 1,
            sleep_roll: 0,
            sleep_chance: 1.0,
            nap_roll: 0,
            armour_roll: 1.0,
            shuffle_roll: 1,
            searched: 0,
        }
    }
}

/// A training academy's facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrainingFacts {
    /// Where the trainee is.
    pub at: Pos,
    /// `building.getBuildingLevel()`.
    pub building_level: i32,
    /// `isSetup()` — a bow, a sword and a shield in hand (`:246-257`,
    /// `EntityAICombatTraining.java:333-351`).
    pub setup: bool,
    /// `building.hasCombatPartner(worker)` (`:97`).
    pub has_partner: bool,
    /// `getCombatPartner(worker)` / `getRandomCombatPartner(worker)`.
    pub partner: Option<Pos>,
    /// `BlockPosUtil.getDistance2D(worker.blockPosition(),
    /// trainingPartner.blockPosition())` — **Manhattan**
    /// (`EntityAICombatTraining.java:151`, `BlockPosUtil.java:423-429`).
    pub partner_distance: f64,
    /// The partner is still assigned to this job module (`:145-149`).
    pub partner_assigned: bool,
    /// `walkToWorkPos(trainingPartner.blockPosition())` (`:151`): the branch is
    /// `distance > MIN_DISTANCE_TO_TRAIN && !walkToWorkPos(...)`, so a walk that
    /// arrived falls through to the blow instead of repeating.
    pub partner_walked: bool,
    /// `getRandomCombatTarget(worker.getRandom())`.
    pub dummy: Option<Pos>,
    /// `getRandomShootingTarget(worker.getRandom())`.
    pub shooting_target: Option<Pos>,
    /// `getRandomShootingStandPosition(worker.getRandom())`.
    pub shooting_stand: Option<Pos>,
    /// `world.clip(...)` from the eye to the target hits the target itself
    /// (`EntityAIArcherTraining.java:123-124`).
    pub line_clear: bool,
    /// `targetCounter`.
    pub target_counter: i32,
    /// `currentAttackDelay`.
    pub attack_delay: i32,
    /// `worker.getRandom().nextInt(100)` for `TARGET_SEARCH_CHANCE`
    /// (`AbstractEntityAITraining.java:76`).
    pub roll: i32,
    /// `worker.getRandom().nextInt(ONE_HUNDRED_PERCENT)` for the partner coin
    /// (`EntityAICombatTraining.java:97`).
    pub partner_roll: i32,
    /// `worker.getRandom().nextBoolean()` — shield or swing (`:198`, `:303`).
    pub coin: bool,
    /// `arrowInProgress.distanceToSqr(target)` (`EntityAIArcherTraining.java:221`).
    pub arrow_distance_sqr: f64,
}

/// The cavalry guard's mount search.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CavalryFacts {
    /// Where the guard is.
    pub at: Pos,
    /// `findMount` found a horse in range (`EntityAICavalry.java:166`).
    pub horse: Option<Pos>,
    /// The horse has no reservation and no other rider (`isAvailableFor`,
    /// `:315`).
    pub horse_available: bool,
    /// `worker.getVehicle() != null` — already mounted.
    pub mounted: bool,
    /// `guard` may ride: not in a mine, in the colony (`:134-157`).
    pub may_ride: bool,
    /// `horse.isInStable()` (`:297`).
    pub in_stable: bool,
    /// The stable the horse calls home.
    pub stable: Option<Pos>,
    /// `buildingGuards.getMinePos()`.
    pub mine_pos: Option<Pos>,
    /// `level.getGameTime()` and the horse's last dismount, for
    /// `LINGER_AFTER_DISMOUNT` (`ReturnToStableGoal.java:136`).
    pub game_time: i64,
    /// `horse.getLastDismountTime()`.
    pub last_dismount: i64,
    /// `horse.getControllingPassenger() != null` (`:258`).
    pub has_passenger: bool,
    /// `horse.hasReservation()` (`:259`).
    pub reservation: bool,
    /// `cavalryHorse.getStableBuilding() instanceof BuildingStable` (`:143`).
    pub stable_is_stable: bool,
    /// `horse.distanceToSqr(stablePosition)` (`:149`).
    pub distance_to_stable_sqr: f64,
    /// `startDistance * startDistance` (`:97`).
    pub start_distance_sqr: f64,
    /// `stable.getNextStallPosition()` (`:333`).
    pub stall: Option<Pos>,
    /// The walk to the target arrived (`EntityNavigationUtils.walkToPos`, `:300`).
    pub arrived: bool,
}

/// The tavern visitor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisitorFacts {
    /// Where the visitor is.
    pub at: Pos,
    /// `isEntityLoaded()` (`EntityAIVisitor.java:224-240`): colony, citizen data,
    /// home building, and a loaded chunk.
    pub loaded: bool,
    /// `building.hasModule(TAVERN_VISITOR)` (`:232`).
    pub has_tavern: bool,
    /// The tavern building the visitor was handed (`:53`, `:234`).
    pub tavern: Option<Pos>,
    /// `tavern.getModule(TAVERN_VISITOR).getFreeSitPosition()` (`:174`).
    pub free_seat: Option<Pos>,
    /// `citizen.getCitizenData().getSaturation()` (`:97`).
    pub saturation: f64,
    /// `citizen.getTarget()` or `citizen.getLastHurtByMob()` (`:251-254`).
    pub target: Option<Pos>,
    /// `colony.isDay()` (`:172`).
    pub day: bool,
    /// `getRandom().nextInt(5)` (`:171`).
    pub roll: i32,
    /// `actionTimeoutCounter`.
    pub action_timeout: i32,
    /// `citizen.getNavigation().isInProgress()` (`:206`).
    pub nav_in_progress: bool,
    /// `citizen.hasLineOfSight(target)` (`:124`).
    pub line_of_sight: bool,
    /// `getRandom().nextInt(2500) + 3000` — the sitting timer (`:179`).
    pub sit_roll: i32,
    /// `getRandom().nextInt(1000) + 1000` — the day wander timer (`:186`).
    pub wander_roll: i32,
}

/// `BlockPosUtil.getDistance2D` (`api/util/BlockPosUtil.java:423-429`):
/// **`|dx| + |dz|`**, named `2D` in Java but Manhattan, not Euclidean, and blind to
/// `y`. Four rules compare it against a constant that looks like a square and is
/// not: `EntityAIMournCitizen.java:316` (225), `EntityAISickTask.java:353` (10),
/// `EntityAIGuard.follow` (`AbstractEntityAIGuard.java:440`, 30) and
/// `EntityAICombatTraining.java:151` (5).
pub fn manhattan(a: Pos, b: Pos) -> i64 {
    // `getDistance2D` reads `getX()` and `getZ()` only (`BlockPosUtil.java:425-426`):
    // a citizen six blocks below its town hall is at distance zero, not six.
    (a.x - b.x).abs() as i64 + (a.z - b.z).abs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_keeps_the_line_it_was_read_from() {
        assert_eq!(MAX_SATURATION.v(), 60);
        assert_eq!(MAX_SATURATION.source, "colony/ICitizenData.java:30");
        assert_eq!(AVERAGE_SATURATION.v(), 10);
        assert_eq!(RANGE_TO_BE_HOME.v(), 16);
        assert_eq!(SEEK_DOCTOR_HEALTH.v(), 6);
        assert!(AVERAGE_SATURATION.source.ends_with(".java:43"));
    }

    #[test]
    fn four_trades_count_as_guards_and_a_cook_is_not_one() {
        // `EntityAIEatTask.java:297` and `EntityAICitizenWander.java:296` both ask
        // `instanceof AbstractJobGuard`.
        assert!(JobKind::Guard.is_guard());
        assert!(JobKind::Cavalry.is_guard());
        assert!(JobKind::Druid.is_guard());
        assert!(!JobKind::Cook.is_guard());
        assert!(!JobKind::Worker.is_guard());
        assert!(!JobKind::None.has_job());
        assert!(JobKind::Cook.is_cook());
    }

    #[test]
    fn only_a_trapdoor_closes_just_what_it_opened() {
        // `EntityAIInteractToggleAble.java:562-565`.
        assert!(ToggleKind::TrapDoor.closes_only_its_own());
        assert!(!ToggleKind::Fence.closes_only_its_own());
        assert!(!ToggleKind::Door.closes_only_its_own());
        // `:377-393`: air is never valid, and the block must have `OPEN`.
        let mut pos = TogglePos {
            pos: Pos::new(1, 64, 1),
            kind: ToggleKind::Fence,
            has_open: true,
            is_air: false,
            open: false,
            distance_sqr: 4.0,
        };
        assert!(pos.valid());
        pos.is_air = true;
        assert!(!pos.valid());
        pos.is_air = false;
        pos.has_open = false;
        assert!(!pos.valid());
    }

    #[test]
    fn the_mourn_location_prefers_the_town_hall_and_the_grave_is_found_by_name() {
        // `EntityAIMournCitizen.java:336-345`.
        let mut facts = MournFacts {
            town_hall: Some(Pos::new(0, 70, 0)),
            home: Some(Pos::new(9, 64, 9)),
            ..MournFacts::default()
        };
        assert_eq!(facts.mourn_location(), Some(Pos::new(0, 70, 0)));
        facts.town_hall = None;
        assert_eq!(facts.mourn_location(), Some(Pos::new(9, 64, 9)));
        facts.home = None;
        assert_eq!(facts.mourn_location(), None);

        // `:189-203`: first name and the remainder must both be on the grave.
        let grave = Pos::new(4, 64, 4);
        facts.deceased = vec!["Jeanne Dupont"];
        facts.named_graves = vec![
            (Pos::new(1, 64, 1), vec!["Pierre", "Dupont"]),
            (grave, vec!["Jeanne", "Dupont"]),
        ];
        assert_eq!(facts.grave_of_deceased(), Some(grave));
        facts.deceased = vec!["Absent Person"];
        assert_eq!(facts.grave_of_deceased(), None);
        facts.deceased.clear();
        assert_eq!(facts.grave_of_deceased(), None);
    }

    #[test]
    fn a_bed_is_usable_only_when_all_four_tests_pass() {
        // `EntityAISleep.java:173-190`.
        let facts = HomeFacts {
            building: Some(Pos::new(0, 64, 0)),
            beds: vec![Pos::new(1, 64, 1)],
            beds_loaded: vec![true],
            bed_states_valid: vec![true],
            bed_is_head: vec![true],
            bed_has_headroom: vec![false],
            bed_index: 0,
            ..HomeFacts::default()
        };
        assert!(!facts.homeless());
        assert_eq!(facts.assigned_bed(), Some(Pos::new(1, 64, 1)));
        assert!(!facts.bed_usable(0), "no room above the pillow");
        assert!(!facts.bed_usable(3), "an index out of range is not usable");
        assert_eq!(HomeFacts::default().assigned_bed(), None);
    }

    #[test]
    fn the_seat_lists_follow_the_weather_and_the_occupancy() {
        // `EntityAICitizenWander.java:191-235`.
        let inside_seat = Seat {
            pos: Pos::new(1, 64, 1),
            inside: true,
            sitting: true,
            occupied: false,
        };
        let outside_seat = Seat {
            pos: Pos::new(5, 64, 5),
            inside: false,
            sitting: true,
            occupied: false,
        };
        let taken = Seat {
            pos: Pos::new(2, 64, 2),
            inside: true,
            sitting: true,
            occupied: true,
        };
        let standing = Seat {
            pos: Pos::new(3, 64, 3),
            inside: true,
            sitting: false,
            occupied: false,
        };
        let mut facts = WanderFacts {
            seats: vec![inside_seat, outside_seat, taken, standing],
            ..WanderFacts::default()
        };
        assert_eq!(
            facts.candidate_seats(),
            vec![inside_seat.pos, outside_seat.pos],
            "an occupied seat is never offered"
        );
        facts.raining = true;
        assert_eq!(facts.candidate_seats(), vec![inside_seat.pos]);
        assert_eq!(facts.candidate_standing(), vec![standing.pos]);
    }

    #[test]
    fn a_disease_without_cures_puts_nothing_in_hand() {
        // `EntityAISickTask.java:250-254`.
        assert!(!Disease::new("flu", &[]).has_cure());
        let flu = Disease::new("flu", &["minecraft:honey_bottle"]);
        assert!(flu.has_cure());
        assert_eq!(flu.cure_items.len(), 1);
    }

    #[test]
    fn the_default_citizen_is_fed_and_the_default_combat_has_no_target() {
        let citizen = CitizenFacts::default();
        assert_eq!(citizen.saturation, 60.0);
        assert!(citizen.nav_done);
        assert_eq!(citizen.job, JobKind::Worker);
        let combat = CombatFacts::default();
        assert!(combat.threat_target.is_none());
        assert!(combat.nav_done);
    }

    #[test]
    fn the_manhattan_helper_matches_blockposutil_and_ignores_the_height() {
        // `BlockPosUtil.java:423-429`: `|dx| + |dz|`, and `y` is not read at all.
        assert_eq!(manhattan(Pos::new(0, 64, 0), Pos::new(3, 60, 4)), 3 + 4);
        assert_eq!(
            manhattan(Pos::new(0, 64, 0), Pos::new(0, 200, 0)),
            0,
            "two hundred blocks up is distance zero"
        );
        assert_eq!(manhattan(Pos::new(0, 64, 0), Pos::new(0, 64, 0)), 0);
    }
}
