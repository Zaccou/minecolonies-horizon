//! The beekeeper: prepare the shears or the bottles, look for a full hive, harvest it,
//! and breed the bees when the colony has room for more.
//!
//! Java reference: `core/entity/ai/workers/production/agriculture/EntityAIWorkBeekeeper
//! .java` — the constructor's `registerTargets` (`:108-115`), `hasMaxAnimals` (`:130-137`),
//! `getBeesInHives` (`:144-154`), `prepareForHerding` (`:161-178`),
//! `startWorkingAtOwnBuilding` (`:185-193`), `decideWhatToDo` (`:200-259`),
//! `breedAnimals` (`:266-305`), `getHiveToHarvest` (`:314-333`), `harvestHoney`
//! (`:341-416`), `isReadyForBreeding` (`:441-475`), `breedTwoAnimals` (`:483-499`),
//! `getToolSlot` (`:524-534`), `searchForAnimals` (`:587-601`) and the two harvest
//! counters (`:603-611`), on top of `JobBeekeeper`'s counter
//! (`core/colony/jobs/JobBeekeeper.java:16-74`).
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:110` |
//! | 2 | `PREPARING` | `prepareForHerding` | 1 | `:111` |
//! | 3 | `DECIDE` | `decideWhatToDo` | 1 | `:112` |
//! | 4 | `HERDER_BREED` | `breedAnimals` | 1 | `:113` |
//! | 5 | `BEEKEEPER_HARVEST` | `harvestHoney` | 20 | `:114` |
//!
//! `IDLE → START_WORKING` is the beekeeper's own line at `:109`, and
//! [`super::register_handover`] registers it.
//!
//! **`decideWhatToDo` is a four-step ladder, and the order is the rule.** No hive at all
//! blocks the job (`:206-211`); breeding on with an empty flower list blocks it too
//! (`:213-219`); a full hive is harvested before any breeding (`:221-226`); breeding comes
//! before idling (`:253-256`). A hive that is full and a colony with room both mean
//! harvest, always.
//!
//! **`isReadyForBreeding` answers five different ways**, and four of them are `false` in
//! Java (`:441-475`). The port keeps them apart ([`Breeding`]) because "the colony is
//! full", "there are not two grown bees", "we just asked for flowers" and "the setting is
//! off" are four different things for a player to read — and only some are the player's to
//! fix.
//!
//! **One division, guarded.** `decideWhatToDo`'s delay is
//! `DECIDING_DELAY + (99 / getSecondarySkillLevel() - 1)` (`:202`). A level of zero would
//! throw in Java; [`decide_delay`] floors the divisor at one so a mod cannot divide by
//! zero, and says so here rather than in a comment nobody reads.

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai::state::{Machine, WorkerState};
use crate::jobs::tool::ToolKind;

use super::facts::{Bee, BeekeeperFacts, HarvestType, Hive, WorkMemory};
use super::state::{WorkRefusal, WorkState};
use super::{add_target, at_hut, work_action, work_action_trade, Decision};

/// `NUM_OF_ANIMALS_TO_BREED` (`:58`): two grown bees are the minimum to breed.
pub const NUM_OF_ANIMALS_TO_BREED: i32 = 2;

/// `BEES_PER_LEVEL` (`:59`): the colony's bee budget is `buildingLevel * this`.
pub const BEES_PER_LEVEL: i32 = 3;

/// `NUM_OF_FLOWERS_TO_BREED` (`:64`): two flowers are the minimum to hold.
pub const NUM_OF_FLOWERS_TO_BREED: u32 = 2;

/// `NUM_OF_WANTED_FLOWERS` (`:69`): what the flower request asks for.
pub const NUM_OF_WANTED_FLOWERS: u32 = 16;

/// `EXP_PER_HARVEST` (`:74`): five experience a harvested hive.
pub const EXP_PER_HARVEST: f64 = 5.0;

/// `DISTANCE_TO_BREED` (`:79`): the two bees must be within ten blocks of each other.
pub const DISTANCE_TO_BREED: i64 = 10;

/// `HIVE_BEE_RADIUS` (`:84`): `searchForAnimals` looks this far around every hive.
pub const HIVE_BEE_RADIUS: i32 = 22;

/// `DECIDING_DELAY` (`:89`): the base wait after a decision.
pub const DECIDING_DELAY: u32 = 40;

/// `NO_ANIMALS_DELAY` (`:90`): the wait when there is no bee to work with.
pub const NO_ANIMALS_DELAY: u32 = 100;

/// `NO_HIVES_DELAY` (`:91`): the wait when the hut lists no hive.
pub const NO_HIVES_DELAY: u32 = 100;

/// `NO_FLOWERS_DELAY` (`:92`): the wait when breeding is on and the flower list is empty.
pub const NO_FLOWERS_DELAY: u32 = 100;

/// `BREEDING_DELAY` (`:93`): the wait after a breeding attempt.
pub const BREEDING_DELAY: u32 = 40;

/// `getActionsDoneUntilDumping` (`:118-122`): five harvests or breedings, then a dump.
pub const ACTIONS_UNTIL_DUMPING: u32 = 5;

/// The honey level a hive must reach before it is harvested (`:321`).
pub const HONEY_LEVEL_FULL: i32 = 5;

/// `getHoneyBottlesPerHarvest` (`:603-606`): one bottle a harvest.
pub const HONEY_BOTTLES_PER_HARVEST: u32 = 1;

/// `getHoneycombsPerHarvest` (`:608-611`): three combs a harvest.
pub const HONEYCOMBS_PER_HARVEST: u32 = 3;

/// `setRemainingPersistentAngerTime(400 + random.nextInt(400))` (`:409`): four hundred
/// ticks, plus up to four hundred more, of angry bees.
pub const ANGER_TICKS_BASE: u32 = 400;

/// How far an angered bee has to be from the worker to be spared (`:407`):
/// `distanceToSqr(bee) <= 16.0`.
pub const ANGER_RANGE_SQR: f64 = 16.0;

/// `JobBeekeeper.COUNTER_TRIGGER` (`JobBeekeeper.java:16`): once the no-bee counter passes
/// four, the colony is told.
pub const COUNTER_TRIGGER: i32 = 4;

/// `JobBeekeeper.tickNoBees`'s cap (`:50-56`): the counter never passes a hundred.
pub const COUNTER_CAP: i32 = 100;

/// `TOOL_LEVEL_WOOD_OR_GOLD` (`api/util/constant/EquipmentLevelConstants.java:20`): the
/// minimum level `getToolSlot` accepts for the shears (`:527`).
pub const TOOL_LEVEL_WOOD_OR_GOLD: i32 = 0;

/// What `isReadyForBreeding` answered (`:441-475`).
///
/// Java returns a bare `false` for four of these, which is why a player cannot tell "no
/// room" from "no bees"; the port keeps the reason and gives it a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five exits of `isReadyForBreeding`
pub enum Breeding {
    /// `!building.getSetting(BREEDING).getValue()` (`:443-446`).
    Off,
    /// `hasMaxAnimals(bees)` (`:453` over `:130-137`): the colony already holds
    /// `buildingLevel * BEES_PER_LEVEL` bees.
    Full,
    /// Fewer than [`NUM_OF_ANIMALS_TO_BREED`] bees at age zero (`:451-453`).
    FewBees,
    /// Fewer than [`NUM_OF_FLOWERS_TO_BREED`] flowers and no request open: one just went
    /// out, for [`NUM_OF_WANTED_FLOWERS`] (`:459-469`).
    FlowersRequested,
    /// Fewer than [`NUM_OF_FLOWERS_TO_BREED`] flowers with a request already open
    /// (`:471`).
    FlowersMissing,
    /// `flowerCount >= NUM_OF_FLOWERS_TO_BREED` (`:471`): breed.
    Ready,
}

impl Breeding {
    /// Whether the beekeeper may go to `HERDER_BREED` (`:471`).
    pub fn is_ready(self) -> bool {
        self == Breeding::Ready
    }

    /// The refusal this answer carries, `None` for [`Breeding::Ready`].
    pub fn refusal(self) -> Option<WorkRefusal> {
        match self {
            Breeding::Off => Some(WorkRefusal::BreedingOff),
            Breeding::Full => Some(WorkRefusal::TooManyBees),
            Breeding::FewBees => Some(WorkRefusal::NoBees),
            Breeding::FlowersRequested | Breeding::FlowersMissing => {
                Some(WorkRefusal::FlowersMissing)
            }
            Breeding::Ready => None,
        }
    }

    /// Java's line for this answer.
    pub fn java(self) -> &'static str {
        match self {
            Breeding::Off => "EntityAIWorkBeekeeper.java:443-446",
            Breeding::Full => "EntityAIWorkBeekeeper.java:453",
            Breeding::FewBees => "EntityAIWorkBeekeeper.java:451-453",
            Breeding::FlowersRequested => "EntityAIWorkBeekeeper.java:459-469",
            Breeding::FlowersMissing => "EntityAIWorkBeekeeper.java:471",
            Breeding::Ready => "EntityAIWorkBeekeeper.java:471",
        }
    }
}

/// What one `harvestHoney` call puts in the worker's inventory (`:371-397`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches Java writes, plus "nothing gained"
pub enum HarvestProducts {
    /// Shears were used: `getHoneycombsPerHarvest()` combs (`:375`, `:608-611`).
    Combs(u32),
    /// A glass bottle was used: as many honey bottles as bottles in the stack, capped at
    /// `getHoneyBottlesPerHarvest()` (`:386-391`, `:603-606`).
    Bottles(u32),
    /// The branch ran but the hand was empty (`:387`): the hive is still emptied
    /// (`:394`) and nothing is gained.
    Nothing,
}

/// `decideWhatToDo`'s delay: `DECIDING_DELAY + (99 / secondarySkill - 1)` (`:202`).
///
/// The divisor is floored at one. Java would throw `ArithmeticException` on a level of
/// zero, which `CitizenSkillHandler` allows for a skill that has no entry
/// (`core/colony/citizens/CitizenSkillHandler.java:182`); a mod must not.
pub fn decide_delay(secondary_skill: i32) -> u32 {
    let skill = secondary_skill.max(1) as u32;
    // `99 / skill - 1` is Java's integer division; `saturating_sub` because a skill of
    // 99 makes it exactly zero and a skill above 99 would make it negative.
    DECIDING_DELAY + (99 / skill).saturating_sub(1)
}

/// `hasMaxAnimals(allBees)` (`:130-137`): `(bees + beesInHives) >= buildingLevel *
/// BEES_PER_LEVEL`.
///
/// `bees` is `searchForAnimals(...).size()` (`:133`), the bees flying around the hives;
/// `bees_in_hives` is [`BeekeeperFacts::bees_in_hives`], the occupants the block entities
/// report (`:144-154`).
pub fn has_max_animals(facts: &BeekeeperFacts, bees: usize) -> bool {
    (bees as i32 + facts.bees_in_hives) >= facts.max_bees() // :136
}

/// `getHiveToHarvest()` (`:314-333`): the first hive of the hut that is a beehive and is
/// at honey level five or more.
///
/// Java also **removes** the hives that fail the tag test (`:328`); that write is the
/// caller's, and [`invalid_hives`] is how it knows which ones.
pub fn hive_to_harvest(hives: &[Hive]) -> Option<Pos> {
    hives
        .iter()
        .find(|hive| hive.valid && hive.is_full()) // :319-321
        .map(|hive| hive.at)
}

/// The hives of the hut that are no longer beehives (`:326-329`), which Java deletes.
pub fn invalid_hives(hives: &[Hive]) -> Vec<Pos> {
    hives
        .iter()
        .filter(|hive| !hive.valid)
        .map(|hive| hive.at)
        .collect()
}

/// The two bees `breedAnimals` picks (`:272-289`).
///
/// The first is any bee that is `!isBaby()` (`:272-276`); the second is one that is
/// `getAge() == 0`, at most [`DISTANCE_TO_BREED`] away, and not the first (`:283-289`).
/// Both tests are Java's: a bee on its cooldown passes the first and not the second.
pub fn breeding_pair(bees: &[Bee]) -> Option<(Bee, Bee)> {
    let first = *bees.iter().find(|bee| !bee.is_baby())?; // :272-276
    let second = bees.iter().find(|bee| {
        bee.will_breed() // :287
            && bee.at.dist_sqr(first.at) <= DISTANCE_TO_BREED * DISTANCE_TO_BREED
            && bee.at != first.at
    })?;
    Some((first, *second))
}

/// `isReadyForBreeding()` (`:441-475`).
pub fn is_ready_for_breeding(facts: &BeekeeperFacts) -> Breeding {
    if !facts.breeding_enabled {
        return Breeding::Off; // :443-446
    }
    let breedable = facts.bees.iter().filter(|bee| bee.will_breed()).count() as i32;
    // `canBreed = !hasMaxAnimals(bees) && breedableAnimals >= NUM_OF_ANIMALS_TO_BREED`
    // (`:453`).
    if has_max_animals(facts, facts.bees.len()) {
        return Breeding::Full;
    }
    if breedable < NUM_OF_ANIMALS_TO_BREED {
        return Breeding::FewBees;
    }
    if facts.flower_count() < NUM_OF_FLOWERS_TO_BREED {
        if facts.has_flower_request {
            return Breeding::FlowersMissing; // :459 — the `&&` that skips the request
        }
        // :461-467 — a `StackList` of the hut's flowers, wanted
        // `NUM_OF_WANTED_FLOWERS`, at least `NUM_OF_FLOWERS_TO_BREED`, then `false`.
        return Breeding::FlowersRequested;
    }
    Breeding::Ready // :471
}

/// `harvestHoney`'s product, once the hive is reached (`:371-397`).
///
/// `use_shears` is the branch Java takes at `:343-349`:
/// `HONEYCOMB || (BOTH && lastHarvestedBottle)`; anything else takes the bottle branch.
pub fn harvest_products(use_shears: bool, facts: &BeekeeperFacts) -> HarvestProducts {
    if use_shears {
        // `:371` — `!getHarvestTypes().equals(HONEY) && shears.checkIsEquipment(...)`.
        if facts.has_shears {
            return HarvestProducts::Combs(HONEYCOMBS_PER_HARVEST); // :375
        }
        return HarvestProducts::Nothing;
    }
    // `:384-391` — `!HONEYCOMB && itemStack.getItem() == GLASS_BOTTLE`, then one bottle
    // per item of the stack, capped by `getHoneyBottlesPerHarvest()`.
    if facts.has_bottle {
        return HarvestProducts::Bottles(HONEY_BOTTLES_PER_HARVEST);
    }
    HarvestProducts::Nothing
}

/// Which hand `harvestHoney` reaches for (`:343-356`): shears, or a glass bottle.
///
/// `HONEYCOMB` always uses shears, `HONEY` always a bottle, and `BOTH` alternates — which
/// is what `lastHarvestedBottle` is for (`:343`, `:382`, `:396`).
pub fn use_shears(harvest_type: HarvestType, last_harvested_bottle: bool) -> bool {
    harvest_type == HarvestType::Honeycomb
        || (harvest_type.is_both() && last_harvested_bottle)
}

/// Whether the harvest angers the bees (`:399-412`):
/// `(50 - (dex / 99. * 50.)) / 100 > random.nextDouble()`.
///
/// At skill zero the left side is `0.5`, so half the harvests anger them; at 99 it is zero
/// and no draw can beat it. `draw` is `worker.getRandom().nextDouble()`
/// ([`BeekeeperFacts::anger_draw`]).
pub fn bees_get_angry(primary_skill: i32, draw: f64) -> bool {
    let skill = primary_skill.clamp(0, 99) as f64;
    let chance = (50.0 - (skill / 99.0 * 50.0)) / 100.0; // :400
    chance > draw
}

/// `setRemainingPersistentAngerTime(400 + random.nextInt(400))` (`:409`).
pub fn anger_ticks(roll: u32) -> u32 {
    ANGER_TICKS_BASE + roll % ANGER_TICKS_BASE
}

/// `JobBeekeeper.tickNoBees()` (`JobBeekeeper.java:50-56`): count up to [`COUNTER_CAP`].
pub fn tick_no_bees(counter: &mut i32) {
    if *counter < COUNTER_CAP {
        *counter += 1;
    }
}

/// `JobBeekeeper.resetCounter()` (`JobBeekeeper.java:61-64`).
pub fn reset_counter(counter: &mut i32) {
    *counter = 0;
}

/// `JobBeekeeper.checkForBeeInteraction()` (`JobBeekeeper.java:71-74`):
/// `counter > COUNTER_TRIGGER`.
pub fn check_bee_interaction(counter: i32) -> bool {
    counter > COUNTER_TRIGGER
}

/// `startWorkingAtOwnBuilding()` (`:185-193`).
pub fn start_working(facts: &WorldFacts) -> Decision {
    if !at_hut(facts) {
        return Decision::refuse(WorkRefusal::NotAtHut); // :189-190
    }
    Decision::go(WorkState::Base(WorkerState::Preparing)) // :192
}

/// `prepareForHerding()` (`:161-178`).
///
/// The shears are asked for first (`:166`) and the glass bottles second (`:174`): a
/// beekeeper set to `HONEY` never wants shears, and one set to `HONEYCOMB` never wants
/// bottles. `BOTH` wants both — which is what makes the `lastHarvestedBottle` flag of
/// [`use_shears`] necessary.
///
/// Java's `checkForToolOrWeapon` **creates a request** and answers `true`; the request is
/// the caller's write, so the port records the refusal and stays in `PREPARING`.
pub fn prepare_for_herding(facts: &BeekeeperFacts, memory: &mut WorkMemory) -> Decision {
    memory.base.delay = DECIDING_DELAY as i32; // :163
    if facts.harvest_type.needs_shears() && !facts.has_shears {
        return Decision::refuse(WorkRefusal::MissingTool(ToolKind::Shears)); // :166-169
    }
    if facts.harvest_type.needs_bottles() && !facts.has_bottle {
        // :174 — the request goes out, and `DECIDE` is the answer either way.
        return Decision::go(WorkState::Base(WorkerState::Decide))
            .because(WorkRefusal::NoHarvestItem);
    }
    Decision::go(WorkState::Base(WorkerState::Decide)) // :177
}

/// `decideWhatToDo()` (`:200-259`), the four-step ladder.
pub fn decide_what_to_do(facts: &BeekeeperFacts, memory: &mut WorkMemory) -> Decision {
    use WorkState::Base;
    memory.base.delay = decide_delay(facts.secondary_skill) as i32; // :202

    // :204-211 — no hive, no job.
    if facts.hives.is_empty() {
        memory.base.delay = NO_HIVES_DELAY as i32;
        return Decision::refuse(WorkRefusal::NoHives);
    }
    // :213-219 — breeding on and no flower listed.
    if facts.flower_list_empty && facts.breeding_enabled {
        memory.base.delay = NO_FLOWERS_DELAY as i32;
        return Decision::refuse(WorkRefusal::NoFlowers);
    }
    // :221-226 — a full hive wins over everything below it.
    if hive_to_harvest(&facts.hives).is_some() {
        return Decision::go(WorkState::BeekeeperHarvest);
    }

    // :228-251 — the bees. `job.tickNoBees()` (`:235`) then a chat message once the
    // counter passes `COUNTER_TRIGGER` (`:236-239`), and a hundred-tick wait (`:245`);
    // bees in the hives or around them restart the counter (`:243`, `:250`).
    if facts.bees.is_empty() {
        if facts.bees_in_hives <= 0 {
            tick_no_bees(&mut memory.beekeeper.counter);
            memory.base.delay = NO_ANIMALS_DELAY as i32;
            if check_bee_interaction(memory.beekeeper.counter) {
                return Decision::refuse(WorkRefusal::NoBees);
            }
            return Decision::stay();
        }
        reset_counter(&mut memory.beekeeper.counter);
        memory.base.delay = NO_ANIMALS_DELAY as i32;
        return Decision::stay();
    }
    reset_counter(&mut memory.beekeeper.counter);

    // :253-256 — breed before idling.
    let breeding = is_ready_for_breeding(facts);
    if breeding.is_ready() {
        return Decision::go(WorkState::HerderBreed);
    }
    // :258 — `START_WORKING`, carrying the reason the breeding did not happen.
    let decision = Decision::go(Base(WorkerState::StartWorking));
    match breeding.refusal() {
        Some(refusal) => decision.because(refusal),
        None => decision,
    }
}

/// `breedAnimals()` (`:266-305`).
pub fn breed_animals(
    world: &mut WorldFacts,
    facts: &BeekeeperFacts,
    memory: &mut WorkMemory,
) -> Decision {
    use WorkState::Base;
    memory.base.delay = BREEDING_DELAY as i32; // :268
    if breeding_pair(&facts.bees).is_none() {
        // `:278-294` — no first or no second bee: back to the decision, with the reason
        // `isReadyForBreeding` would have given.
        let refusal = is_ready_for_breeding(facts)
            .refusal()
            .unwrap_or(WorkRefusal::NoBees);
        return Decision::go(Base(WorkerState::Decide)).because(refusal);
    }
    if !facts.has_flower {
        // `:296-299` — no flower to hold: back to `START_WORKING`, which asks for one.
        return Decision::go(Base(WorkerState::StartWorking)).because(WorkRefusal::NoBreedItem);
    }
    // `:301-303` — two bees bred, one action done, one saturation point. The action is
    // what `inventoryNeedsDump` counts (`AbstractEntityAIBasic.java:413`), and a
    // beekeeper dumps every five of them (`:118-122`).
    world.inventory.actions_done += 1;
    Decision::go(Base(WorkerState::Decide)) // :304
}

/// `harvestHoney()` (`:341-416`).
pub fn harvest_honey(
    world: &mut WorldFacts,
    facts: &BeekeeperFacts,
    memory: &mut WorkMemory,
) -> Decision {
    use WorkState::Base;
    // :343-356 — equip the shears or a bottle, or go and ask for them.
    let shears = use_shears(facts.harvest_type, memory.beekeeper.last_harvested_bottle);
    if shears && !facts.has_shears {
        return Decision::go(Base(WorkerState::Preparing)).because(WorkRefusal::NoHarvestItem);
    }
    if !shears && !facts.has_bottle {
        return Decision::go(Base(WorkerState::Preparing)).because(WorkRefusal::NoHarvestItem);
    }
    // :357-362 — a second scan, because the first one may have removed an invalid hive.
    let Some(hive) = hive_to_harvest(&facts.hives) else {
        return Decision::go(Base(WorkerState::Decide)).because(WorkRefusal::NoHiveReady);
    };
    // :364-367 — `!walkToWorkPos(hive)`.
    if !facts
        .at
        .is_living_at_site(hive, crate::ai::worker::WOKR_IN_BUILDING_DIST)
    {
        return Decision::stay();
    }
    // :369-397 — the products, and which tool the next harvest will use.
    match harvest_products(shears, facts) {
        HarvestProducts::Combs(_) => memory.beekeeper.last_harvested_bottle = false, // :382
        HarvestProducts::Bottles(_) => memory.beekeeper.last_harvested_bottle = true, // :396
        HarvestProducts::Nothing => {}
    }
    // :399-412 — the bees may come out angry: `bees_get_angry` decides, and the caller
    // releases them for `anger_ticks(facts.anger_roll)` ticks.
    // :413 — `incrementActionsDoneAndDecSaturation()`.
    world.inventory.actions_done += 1;
    Decision::go(Base(WorkerState::StartWorking)) // :415
}

/// `startWorkingAtOwnBuilding` (`:185-193`) reads the **base** facts only, which is why it
/// gets its own adapter instead of a macro.
fn start_action(scene: &mut super::WorkScene, state: WorkState) -> Option<WorkState> {
    let super::WorkScene {
        facts, job, refusal, ..
    } = scene;
    if job.beekeeper().is_none() {
        *refusal = Some(WorkRefusal::NotThisTrade);
        return Some(state);
    }
    let decision = start_working(facts);
    *refusal = decision.refusal;
    decision.next.or(Some(state))
}

work_action_trade!(prepare_action, Beekeeper, prepare_for_herding);
work_action_trade!(decide_action, Beekeeper, decide_what_to_do);
work_action!(breed_action, Beekeeper, breed_animals);
work_action!(harvest_action, Beekeeper, harvest_honey);

/// Register the beekeeper's five targets on top of the base table.
///
/// Java's constructor adds six lines (`:108-115`); the first, `IDLE → START_WORKING`, is
/// [`super::register_handover`]'s.
pub fn register(machine: &mut Machine<WorkState, super::WorkScene>) {
    // :110 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    add_target(
        machine,
        WorkState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIWorkBeekeeper.java:110",
        super::TICKS_SECOND,
        start_action,
    );
    // :111 — `new AITarget(PREPARING, this::prepareForHerding, 1)`.
    add_target(
        machine,
        WorkState::Base(WorkerState::Preparing),
        "prepareForHerding",
        "EntityAIWorkBeekeeper.java:111",
        1,
        prepare_action,
    );
    // :112 — `new AITarget(DECIDE, this::decideWhatToDo, 1)`.
    add_target(
        machine,
        WorkState::Base(WorkerState::Decide),
        "decideWhatToDo",
        "EntityAIWorkBeekeeper.java:112",
        1,
        decide_action,
    );
    // :113 — `new AITarget(HERDER_BREED, this::breedAnimals, 1)`.
    add_target(
        machine,
        WorkState::HerderBreed,
        "breedAnimals",
        "EntityAIWorkBeekeeper.java:113",
        1,
        breed_action,
    );
    // :114 — `new AITarget(BEEKEEPER_HARVEST, this::harvestHoney, TICKS_SECOND)`.
    add_target(
        machine,
        WorkState::BeekeeperHarvest,
        "harvestHoney",
        "EntityAIWorkBeekeeper.java:114",
        super::TICKS_SECOND,
        harvest_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_07::facts::{WorkJob, WorkScene};
    use crate::ai_work_07::{WorkAi, WorkTrade};

    /// A beekeeper scene at its hut (origin), one full hive two blocks away, one grown
    /// bee, shears and a bottle in the bag, breeding off.
    fn new_scene() -> WorkScene {
        let mut world = WorldFacts::new();
        world.worker.at = Pos::new(0, 64, 0);
        world.building.position = Pos::new(0, 64, 0);
        let ai = WorkAi::new(WorkTrade::Beekeeper).expect("the beekeeper has a table");
        let mut scene = ai.scene(world);
        let facts = beekeeper(&mut scene);
        facts.at = Pos::new(0, 64, 0);
        facts.building_level = 2;
        facts.hives = vec![Hive::full(Pos::new(2, 64, 0))];
        facts.bees = vec![Bee::grown(Pos::new(1, 64, 0))];
        facts.bees_in_hives = 1;
        facts.has_shears = true;
        facts.has_bottle = true;
        facts.has_flower = true;
        facts.secondary_skill = 10;
        scene
    }

    /// The trade half, for arranging a scenario. Never held across a decision call: a
    /// decision also needs the memory, and both are fields of the same scene.
    fn beekeeper(scene: &mut WorkScene) -> &mut BeekeeperFacts {
        match &mut scene.job {
            WorkJob::Beekeeper(facts) => facts,
            _ => panic!("a beekeeper scene"),
        }
    }

    // The four adapters the machine's own actions use, so a test calls a decision with
    // the scene split the same way `work_action!` splits it.
    fn decide(scene: &mut WorkScene) -> Decision {
        let WorkScene { job, memory, .. } = scene;
        let WorkJob::Beekeeper(facts) = job else {
            panic!("a beekeeper scene")
        };
        decide_what_to_do(facts, memory)
    }

    fn prepare(scene: &mut WorkScene) -> Decision {
        let WorkScene { job, memory, .. } = scene;
        let WorkJob::Beekeeper(facts) = job else {
            panic!("a beekeeper scene")
        };
        prepare_for_herding(facts, memory)
    }

    fn breed(scene: &mut WorkScene) -> Decision {
        let WorkScene {
            facts,
            job,
            memory,
            ..
        } = scene;
        let WorkJob::Beekeeper(beekeeper) = job else {
            panic!("a beekeeper scene")
        };
        breed_animals(facts, beekeeper, memory)
    }

    fn harvest(scene: &mut WorkScene) -> Decision {
        let WorkScene {
            facts,
            job,
            memory,
            ..
        } = scene;
        let WorkJob::Beekeeper(beekeeper) = job else {
            panic!("a beekeeper scene")
        };
        harvest_honey(facts, beekeeper, memory)
    }

    #[test]
    fn the_decision_delay_shrinks_with_the_secondary_skill_and_never_divides_by_zero() {
        // `:202` — `40 + (99 / skill - 1)`, with the divisor floored at one.
        assert_eq!(decide_delay(1), 40 + 98);
        assert_eq!(decide_delay(10), 40 + 8);
        assert_eq!(decide_delay(99), 40);
        assert_eq!(decide_delay(200), 40, "Java's negative `- 1` saturates");
        // Java would throw here; the port answers the base delay instead.
        assert_eq!(decide_delay(0), 40 + 98);
    }

    #[test]
    fn the_ladder_of_decide_puts_hives_first_then_flowers_then_harvest_then_bees() {
        // `:204-211` — no hive blocks everything, even a colony full of bees.
        let mut scene = new_scene();
        beekeeper(&mut scene).hives.clear();
        let decision = decide(&mut scene);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoHives));
        assert_eq!(scene.memory.base.delay, NO_HIVES_DELAY as i32);

        // `:213-219` — breeding on with an empty flower list blocks it too.
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.flower_list_empty = true;
        facts.breeding_enabled = true;
        let decision = decide(&mut scene);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoFlowers));
        assert_eq!(scene.memory.base.delay, NO_FLOWERS_DELAY as i32);

        // The same empty list with breeding off is not a refusal.
        let mut scene = new_scene();
        beekeeper(&mut scene).flower_list_empty = true;
        let decision = decide(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::BeekeeperHarvest)
        );

        // `:221-226` — a full hive wins over breeding: two grown bees and flowers in
        // stock, and the answer is still harvest.
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.breeding_enabled = true;
        facts.flowers_in_inventory = 4;
        facts.bees = vec![
            Bee::grown(Pos::new(1, 64, 0)),
            Bee::grown(Pos::new(-1, 64, 0)),
        ];
        let decision = decide(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::BeekeeperHarvest)
        );
    }

    #[test]
    fn a_hive_is_harvestable_at_level_five_and_only_if_it_is_still_a_beehive() {
        // `:314-333`.
        let full = Hive::full(Pos::new(2, 64, 0));
        assert_eq!(hive_to_harvest(&[full]), Some(Pos::new(2, 64, 0)));
        assert_eq!(hive_to_harvest(&[Hive::empty(Pos::new(2, 64, 0))]), None);
        // A full hive whose block is no longer tagged as one is not harvested; Java
        // removes it from the hut instead (`:326-329`).
        let stale = Hive {
            valid: false,
            ..full
        };
        assert_eq!(hive_to_harvest(&[stale]), None);
        assert_eq!(invalid_hives(&[stale, full]), vec![Pos::new(2, 64, 0)]);
        // The first harvestable one wins, in the hut's order.
        let second = Hive::full(Pos::new(5, 64, 0));
        assert_eq!(
            hive_to_harvest(&[Hive::empty(Pos::new(0, 65, 0)), second, full]),
            Some(Pos::new(5, 64, 0))
        );
    }

    #[test]
    fn breeding_needs_room_two_grown_bees_and_two_flowers() {
        // `:441-475`, in Java's order: the setting, the room, the bees, the flowers.
        let mut facts = BeekeeperFacts {
            breeding_enabled: false,
            ..Default::default()
        };
        assert_eq!(is_ready_for_breeding(&facts), Breeding::Off);

        // A level-1 hut holds three bees: two around and two inside is already full.
        facts.building_level = 1;
        facts.breeding_enabled = true;
        facts.bees_in_hives = 2;
        facts.bees = vec![
            Bee::grown(Pos::new(0, 65, 0)),
            Bee::grown(Pos::new(1, 65, 0)),
        ];
        assert_eq!(is_ready_for_breeding(&facts), Breeding::Full);

        // One grown bee and one baby is not two.
        facts.bees_in_hives = 0;
        facts.bees = vec![Bee::grown(Pos::new(0, 65, 0)), Bee::baby(Pos::new(1, 65, 0))];
        assert_eq!(is_ready_for_breeding(&facts), Breeding::FewBees);

        // Two grown bees, no flowers, no request: ask for sixteen, need two.
        facts.bees = vec![
            Bee::grown(Pos::new(0, 65, 0)),
            Bee::grown(Pos::new(1, 65, 0)),
        ];
        assert_eq!(is_ready_for_breeding(&facts), Breeding::FlowersRequested);
        facts.has_flower_request = true;
        assert_eq!(is_ready_for_breeding(&facts), Breeding::FlowersMissing);
        // One in the bag and one in the hut is two.
        facts.flowers_in_inventory = 1;
        facts.flowers_in_building = 1;
        assert_eq!(is_ready_for_breeding(&facts), Breeding::Ready);
        // And a bee on its cooldown does not count towards the two.
        facts.bees = vec![
            Bee::grown(Pos::new(0, 65, 0)),
            Bee {
                age: 40,
                ..Bee::grown(Pos::new(1, 65, 0))
            },
        ];
        assert_eq!(is_ready_for_breeding(&facts), Breeding::FewBees);
    }

    #[test]
    fn the_breeding_pair_skips_babies_and_bees_too_far_apart() {
        // `:272-289` — the first is `!isBaby()`, the second is `getAge() == 0` **and**
        // within ten blocks **and** not the first.
        let first = Bee::grown(Pos::new(0, 64, 0));
        let near = Bee::grown(Pos::new(0, 64, 10));
        let far = Bee::grown(Pos::new(0, 64, 11));
        assert_eq!(breeding_pair(&[first, near]), Some((first, near)));
        // Eleven blocks is outside the ten Java allows.
        assert_eq!(breeding_pair(&[first, far]), None);
        // A baby is never the first pick, and a baby never the second.
        assert_eq!(breeding_pair(&[Bee::baby(Pos::new(0, 64, 0))]), None);
        assert_eq!(breeding_pair(&[first, Bee::baby(Pos::new(1, 64, 0))]), None);
        // A bee on cooldown is not the second pick either, even close by.
        assert_eq!(
            breeding_pair(&[
                first,
                Bee {
                    age: 20,
                    ..Bee::grown(Pos::new(1, 64, 0))
                }
            ]),
            None
        );
    }

    #[test]
    fn breeding_and_harvesting_each_count_one_action_towards_the_dump() {
        // `:303` and `:413` — `incrementActionsDoneAndDecSaturation`, and
        // `getActionsDoneUntilDumping() == 5` (`:118-122`).
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.breeding_enabled = true;
        facts.flowers_in_inventory = 2;
        facts.bees = vec![
            Bee::grown(Pos::new(1, 64, 0)),
            Bee::grown(Pos::new(2, 64, 0)),
        ];
        scene.facts.inventory.actions_done = 0;
        let decision = breed(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(scene.facts.inventory.actions_done, 1);
        assert_eq!(ACTIONS_UNTIL_DUMPING, 5);

        // The harvest counts one too.
        let mut scene = new_scene();
        scene.facts.inventory.actions_done = 0;
        let decision = harvest(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::StartWorking)));
        assert_eq!(scene.facts.inventory.actions_done, 1);
    }

    #[test]
    fn without_a_flower_the_worker_goes_back_to_ask_for_one() {
        // `:296-299` — `equipBreedItem` answered false: `START_WORKING`, which is where
        // the flower request is made (`:459-469`).
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.has_flower = false;
        // Two bees, or the pair itself is what is missing (`:272-289`) and the answer
        // would be `DECIDE`, not the flower request.
        facts.bees = vec![
            Bee::grown(Pos::new(1, 64, 0)),
            Bee::grown(Pos::new(2, 64, 0)),
        ];
        facts.breeding_enabled = true;
        facts.flowers_in_inventory = 2;
        let decision = breed(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(WorkRefusal::NoBreedItem));
        assert_eq!(scene.memory.base.delay, BREEDING_DELAY as i32);
    }

    #[test]
    fn both_mode_alternates_shears_and_bottle_harvests() {
        // `:343` — `HONEYCOMB || (BOTH && lastHarvestedBottle)`.
        assert!(use_shears(HarvestType::Honeycomb, false));
        assert!(use_shears(HarvestType::Honeycomb, true));
        assert!(!use_shears(HarvestType::Honey, true));
        assert!(use_shears(HarvestType::Both, true));
        assert!(!use_shears(HarvestType::Both, false));

        // A `BOTH` hut alternates: bottle first (`lastHarvestedBottle == false`), then
        // shears, and the flag flips each harvest (`:382`, `:396`).
        let mut scene = new_scene();
        beekeeper(&mut scene).harvest_type = HarvestType::Both;
        assert!(!scene.memory.beekeeper.last_harvested_bottle);
        let decision = harvest(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        assert!(scene.memory.beekeeper.last_harvested_bottle, "bottles this time");
        let decision = harvest(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
        assert!(
            !scene.memory.beekeeper.last_harvested_bottle,
            "shears this time"
        );
    }

    #[test]
    fn a_harvest_without_its_tool_goes_back_to_preparing() {
        // `:345-355` — `equipTool` / `equipItem` answered false.
        let mut scene = new_scene();
        beekeeper(&mut scene).has_shears = false;
        let decision = harvest(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Preparing)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoHarvestItem));

        // A `HONEY` hut never needs shears, so the same missing shears are not a
        // refusal at all.
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.has_shears = false;
        facts.harvest_type = HarvestType::Honey;
        let decision = harvest(&mut scene);
        assert_eq!(
            decision.next,
            Some(WorkState::Base(WorkerState::StartWorking))
        );
    }

    #[test]
    fn harvesting_out_of_reach_stays_in_the_state_and_with_no_hive_decides() {
        // `:364-367` — `!walkToWorkPos(hive)`.
        let mut scene = new_scene();
        beekeeper(&mut scene).at = Pos::new(30, 64, 30);
        let decision = harvest(&mut scene);
        assert_eq!(decision.next, None, "still walking");

        // `:359-362` — the second scan finds nothing: back to `DECIDE`.
        let mut scene = new_scene();
        beekeeper(&mut scene).hives = vec![Hive::empty(Pos::new(2, 64, 0))];
        let decision = harvest(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoHiveReady));
    }

    #[test]
    fn the_two_harvest_counters_are_one_bottle_and_three_combs() {
        // `:603-611`, and the two branches of `:371-397`.
        let facts = BeekeeperFacts {
            has_shears: true,
            has_bottle: true,
            ..Default::default()
        };
        assert_eq!(
            harvest_products(true, &facts),
            HarvestProducts::Combs(HONEYCOMBS_PER_HARVEST)
        );
        assert_eq!(
            harvest_products(false, &facts),
            HarvestProducts::Bottles(HONEY_BOTTLES_PER_HARVEST)
        );
        // The tool is gone between the equip step and the swing: nothing is gained but
        // the hive is still emptied (`:387-394`).
        let empty = BeekeeperFacts::default();
        assert_eq!(harvest_products(true, &empty), HarvestProducts::Nothing);
        assert_eq!(harvest_products(false, &empty), HarvestProducts::Nothing);
    }

    #[test]
    fn the_bees_get_angry_on_the_bad_half_of_the_draw_and_calmer_with_skill() {
        // `:399-412` — `(50 - dex/99*50)/100 > nextDouble()`.
        assert!(bees_get_angry(0, 0.0));
        assert!(bees_get_angry(0, 0.49));
        assert!(!bees_get_angry(0, 0.5));
        // At full skill the chance is zero, so no draw can beat it.
        assert!(!bees_get_angry(99, 0.0));
        assert!(!bees_get_angry(99, 1.0));
        // Half skill is a quarter chance.
        assert!(bees_get_angry(50, 0.24));
        assert!(!bees_get_angry(50, 0.25));
        // The anger lasts 400 to 800 ticks (`:409`).
        assert_eq!(anger_ticks(0), 400);
        assert_eq!(anger_ticks(399), 799);
        assert_eq!(anger_ticks(400), 400);
        // And a bee further than four blocks from the worker is spared (`:407`).
        assert_eq!(ANGER_RANGE_SQR, 16.0);
    }

    #[test]
    fn the_no_bee_counter_climbs_to_a_hundred_and_warns_once_past_four() {
        // `JobBeekeeper.java:16`, `:50-56`, `:61-64`, `:71-74`.
        let mut counter = 0;
        for _ in 0..COUNTER_TRIGGER {
            tick_no_bees(&mut counter);
        }
        assert_eq!(counter, COUNTER_TRIGGER);
        assert!(!check_bee_interaction(counter), "four is not past four");
        tick_no_bees(&mut counter);
        assert!(check_bee_interaction(counter));
        for _ in 0..1_000 {
            tick_no_bees(&mut counter);
        }
        assert_eq!(counter, COUNTER_CAP);
        reset_counter(&mut counter);
        assert_eq!(counter, 0);
        assert!(!check_bee_interaction(counter));
    }

    #[test]
    fn deciding_with_no_bees_waits_a_hundred_ticks_and_only_complains_after_four() {
        // `:228-247` — no bee around, none in the hives: the counter climbs and the
        // colony is told only past `COUNTER_TRIGGER`.
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.hives = vec![Hive::empty(Pos::new(2, 64, 0))];
        facts.bees.clear();
        facts.bees_in_hives = 0;
        for expected in 1..=COUNTER_TRIGGER {
            let decision = decide(&mut scene);
            assert_eq!(decision.next, None);
            assert_eq!(decision.refusal, None, "no complaint yet");
            assert_eq!(scene.memory.beekeeper.counter, expected);
            assert_eq!(scene.memory.base.delay, NO_ANIMALS_DELAY as i32);
        }
        let decision = decide(&mut scene);
        assert_eq!(decision.refusal, Some(WorkRefusal::NoBees));

        // Bees inside the hives restart the counter instead (`:243`).
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.hives = vec![Hive::empty(Pos::new(2, 64, 0))];
        facts.bees.clear();
        facts.bees_in_hives = 2;
        scene.memory.beekeeper.counter = 3;
        let decision = decide(&mut scene);
        assert_eq!(decision.next, None);
        assert_eq!(scene.memory.beekeeper.counter, 0);
        assert_eq!(scene.memory.base.delay, NO_ANIMALS_DELAY as i32);
    }

    #[test]
    fn prepare_asks_for_shears_first_and_only_for_what_the_setting_needs() {
        // `:161-178`.
        // Honeycomb (the default) with no shears: a refusal that names the tool.
        let mut scene = new_scene();
        beekeeper(&mut scene).has_shears = false;
        let decision = prepare(&mut scene);
        assert_eq!(decision.next, None);
        assert_eq!(
            decision.refusal,
            Some(WorkRefusal::MissingTool(ToolKind::Shears))
        );
        assert_eq!(scene.memory.base.delay, DECIDING_DELAY as i32);

        // `HONEY`: no shears needed at all, and no bottle yet is a request, not a stop.
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.has_shears = false;
        facts.has_bottle = false;
        facts.harvest_type = HarvestType::Honey;
        let decision = prepare(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, Some(WorkRefusal::NoHarvestItem));

        // Everything in the bag: straight to `DECIDE` (`:177`).
        let mut scene = new_scene();
        let decision = prepare(&mut scene);
        assert_eq!(decision.next, Some(WorkState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, None);
    }

    #[test]
    fn the_machine_walks_idle_preparing_deciding_harvest_and_back() {
        // The whole cycle of one harvest, one state per tick (every rate is reached on
        // the first look): `IDLE → START_WORKING → PREPARING → DECIDE →
        // BEEKEEPER_HARVEST → START_WORKING`.
        let mut ai = WorkAi::new(WorkTrade::Beekeeper).expect("a table");
        let mut scene = new_scene();
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Idle));
        assert!(ai.tick(&mut scene).moved(), "IDLE → START_WORKING");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::StartWorking));
        assert!(ai.tick(&mut scene).moved(), "START_WORKING → PREPARING");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Preparing));
        assert!(ai.tick(&mut scene).moved(), "PREPARING → DECIDE");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Decide));
        assert!(ai.tick(&mut scene).moved(), "DECIDE → BEEKEEPER_HARVEST");
        assert_eq!(ai.state(), WorkState::BeekeeperHarvest);
        let outcome = ai.tick(&mut scene);
        assert!(outcome.moved(), "one harvest, then back to work");
        assert_eq!(ai.state(), WorkState::Base(WorkerState::StartWorking));
        assert_eq!(outcome.why(), Some("EntityAIWorkBeekeeper.java:114"));
        assert!(ai.missing().is_none(), "no state trapped the machine");
    }

    #[test]
    fn a_colony_with_room_breeds_before_it_idles() {
        // `:253-256` — after the harvest check, breeding decides, and `HERDER_BREED` is
        // the state a ready colony gets.
        let mut ai = WorkAi::new(WorkTrade::Beekeeper).expect("a table");
        let mut scene = new_scene();
        let facts = beekeeper(&mut scene);
        facts.hives = vec![Hive::empty(Pos::new(2, 64, 0))];
        facts.breeding_enabled = true;
        facts.flowers_in_inventory = 2;
        facts.bees = vec![
            Bee::grown(Pos::new(1, 64, 0)),
            Bee::grown(Pos::new(-1, 64, 0)),
        ];
        // IDLE, START_WORKING, PREPARING: three ticks to stand in `DECIDE`.
        for _ in 0..3 {
            let _ = ai.tick(&mut scene);
        }
        assert_eq!(ai.state(), WorkState::Base(WorkerState::Decide));
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.goal(), WorkState::HerderBreed);
        assert_eq!(outcome.why(), Some("EntityAIWorkBeekeeper.java:112"));
        // And the breed step answers `DECIDE` (`:304`).
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.goal(), WorkState::Base(WorkerState::Decide));
        assert_eq!(outcome.why(), Some("EntityAIWorkBeekeeper.java:113"));
        assert_eq!(scene.facts.inventory.actions_done, 1, "one breeding action");
    }

    #[test]
    fn a_nether_miners_facts_are_refused_by_name() {
        let mut ai = WorkAi::new(WorkTrade::Beekeeper).expect("a table");
        let mut scene = ai.scene_with(WorldFacts::new(), WorkJob::Nether(Default::default()));
        let _ = ai.tick(&mut scene);
        let outcome = ai.tick(&mut scene);
        assert_eq!(outcome.why_not(), Some(WorkRefusal::NotThisTrade));
    }
}
