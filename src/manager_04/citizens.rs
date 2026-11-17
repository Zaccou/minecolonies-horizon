//! The head count: who is in the colony, which numbers are free, how many may live
//! there, and when the next one arrives.
//!
//! Java reference: `core/colony/managers/CitizenManager.java` (733 lines) and the four
//! constants it obeys (`CitizenConstants.java:13`, `Constants.java:21-22`, `:48-49`).
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | which id a new citizen gets | **the lowest free one**, scanning `1..=count+1` | `:331-340` |
//! | how many may live here | the buildings' beds, capped by research **and** by the server | `:424-459`, `:502-526` |
//! | how long between two arrivals | `500 + 60 × townHallLevel` ticks, reset to 1200 | `:596-600` |
//! | how a newcomer's sex is chosen | rolled for the first, then the **minority** sex | `:614-628` |
//! | who mourns a citizen | neither guards nor the undertaker, and only family or housemates | `:640-657` |
//! | who is told the colony is asleep | nobody until every non-guard is in bed | `:671-688` |
//!
//! **What is not duplicated.** The building walk that computes the beds is
//! `crate::building::behaviour::census` (`amelioration.rs:172-233`), ported with its two
//! branches and its `else`; the citizen's own data is `crate::citizens` (state, needs,
//! priority, employment) and `crate::persistence::roster` (the save). What is here is the
//! **manager's** arithmetic: the clamp, the id pool, the two timers, and the four
//! per-citizen decisions. [`Head`] is the five fields those decisions read and nothing
//! else — not a second `CitizenData`.
//!
//! **Two facts of this file are Java's, and neither is obvious.**
//!
//! 1. **`getMaxCitizens()` clamps three times, and the stored value does not follow.**
//!    The getter is `max(1, min(maxCitizens, min(research, config)))` (`:502-506`), while
//!    `calculateMaxCitizens` compares that **clamped** value to the raw new count and only
//!    then stores it — the two setters are inside the `if` (`:453-458`). Two consequences,
//!    both pinned by a test: a colony with no bed reports a change **on every count**
//!    (clamped 1 against raw 0), and the **first bed a colony ever builds changes
//!    nothing at all** (clamped 1 against raw 1), so the number it stores stays 0 until
//!    the second bed. The getter is right either way; the stored value is what lags.
//! 2. **The sex rule divides by two as a `double`.** `femaleCount < (size - 1) / 2.0`
//!    (`:621`) is why a colony of three does not get a third woman: `1 < 1.5` is true,
//!    `2 < 1.5` is false. [`Manager::on_tick`] hands the answer back as
//!    `Option<bool>` — `None` for the first citizen, whose sex Java **rolls**
//!    (`:617`), so the port does not pretend to know a random draw.
//!
//! **What is not here.** The entity (`registerCivilian`'s four `remove` calls,
//! `:123-160`), the packet (`sendPackets`, `:207-228` — its recipients are
//! [`crate::manager_04::subscribers`]), the event bus (`CitizenAddedModEvent`), the quest
//! injection (`tutorial/welcome`, `:611`) and the happiness handler (`:552-567`). Each is
//! a call this file answers as data, and [`MANQUANT.md`](MANQUANT) §4 lists them.

use std::collections::{BTreeMap, BTreeSet};

use crate::building::Standing;
use crate::building::behaviour::census;
use crate::citizens::is_guard;
use crate::jobs::Job;

/// `Constants.java:21`.
pub const TICKS_SECOND: i32 = 20;
/// `Constants.java:22`.
pub const SECONDS_A_MINUTE: i32 = 60;
/// The cap a colony has before any research (`CitizenConstants.java:13`).
pub const CITIZEN_LIMIT_DEFAULT: i32 = 25;
/// The first arrival's interval, `30 * TICKS_SECOND` (`CitizenManager.java:95`).
pub const RESPAWN_INTERVAL: i32 = 30 * TICKS_SECOND;
/// What it is reset to once it fires, `60 * TICKS_SECOND` (`:600`).
pub const RESPAWN_RESET: i32 = 60 * TICKS_SECOND;
/// What one colony tick takes off it, the literal `500` of `onColonyTick` (`:587`, `:596`).
pub const TIER_TICK: i32 = 500;
/// The entity refresh timer, `5 * 60 * TICKS_SECOND` (`:100`, `:589`).
pub const ENTITY_TIMER: i32 = 5 * 60 * TICKS_SECOND;
/// Where a spawned citizen stands inside its block (`Constants.java:48`).
pub const HALF_BLOCK: f64 = 0.5;
/// And how far above it (`Constants.java:49`).
pub const SLIGHTLY_UP: f64 = 0.1;

/// Where the entity of a new citizen is put: the block's middle, slightly raised
/// (`CitizenManager.java:313`).
pub fn spawn_point(at: [i32; 3]) -> [f64; 3] {
    [
        f64::from(at[0]) + HALF_BLOCK,
        f64::from(at[1]) + SLIGHTLY_UP,
        f64::from(at[2]) + HALF_BLOCK,
    ]
}

/// The colony's ceiling: what the buildings house, what research allows, what the server
/// allows — **three numbers that are clamped together**, in Java's order (`:502-526`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the four inputs of the two getters
pub struct Ceiling {
    /// `maxCitizens`: the beds the buildings give (`:455`).
    pub beds: i32,
    /// `potentialMaxCitizens - maxCitizens`: the beds nobody sleeps in yet (`:456`).
    pub spare: i32,
    /// The `CITIZEN_CAP` effect, `None` when the research tree does not hold it
    /// (`:517`).
    pub research_effect: Option<i32>,
    /// `ServerConfiguration.maxCitizenPerColony` (`:505`, `:520`).
    pub config: i32,
}

impl Ceiling {
    /// `maxCitizensFromResearch` (`:514-526`): with the effect, `max(25, strength)`
    /// capped by the server; without it, the server setting alone.
    pub fn research(&self) -> i32 {
        match self.research_effect {
            Some(strength) => CITIZEN_LIMIT_DEFAULT.max(strength).min(self.config),
            None => self.config,
        }
    }

    /// `getMaxCitizens` (`:502-506`): never below one, never above either cap.
    pub fn max(&self) -> i32 {
        self.beds.min(self.research()).max(1)
    }

    /// `getPotentialMaxCitizens` (`:508-512`): the same clamp over the beds that exist.
    pub fn potential(&self) -> i32 {
        (self.beds + self.spare).min(self.research()).max(1)
    }
}

/// The five facts about one citizen that this manager's own rules read — no name, no
/// needs, no AI: those are `crate::citizens` and `crate::persistence::roster`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five fields the manager's rules touch
pub struct Head {
    pub id: i32,
    /// `CitizenData.getUUID`, which is what an entity must match to be let in
    /// (`CitizenManager.java:137`).
    pub uuid: u128,
    pub job: Option<Job>,
    pub child: bool,
    pub asleep: bool,
    /// Whether an entity is currently attached (`data.getEntity().isPresent()`, `:147`).
    pub attached: bool,
}

impl Head {
    pub fn new(id: i32, uuid: u128) -> Head {
        Head {
            id,
            uuid,
            job: None,
            child: false,
            asleep: false,
            attached: false,
        }
    }
}

/// Why the manager did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A citizen that is not in the colony (`removeCivilian` on an id nobody holds,
    /// `:378-407`).
    UnknownCitizen { id: i32 },
    /// **Unreachable by pigeonhole**, and kept anyway: the scan of `1..=count+1` over
    /// `count` ids always finds a hole (`:333-340`). A save with two records of the same
    /// id is what would get here, and Java's `Collectors.toMap` would have thrown before
    /// the manager ever saw it (`:178-180`).
    NoFreeId { count: i32 },
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownCitizen { .. } => "manager_04.citizens.refusal.unknown_citizen",
            Refusal::NoFreeId { .. } => "manager_04.citizens.refusal.no_free_id",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Why an entity was turned away: the three branches of `registerCivilian` (`:123-160`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three ways Java calls `remove(DISCARDED)`
pub enum Why {
    /// No such citizen, or the id zero which means "none" (`:125`).
    Unknown { id: i32 },
    /// The entity's uuid is not the record's (`:137`).
    UuidMismatch { id: i32 },
    /// The citizen already has a living entity (`:149-159`).
    AlreadyAttached { id: i32 },
}

impl Why {
    pub fn name(self) -> &'static str {
        match self {
            Why::Unknown { .. } => "manager_04.citizens.discarded.unknown",
            Why::UuidMismatch { .. } => "manager_04.citizens.discarded.uuid_mismatch",
            Why::AlreadyAttached { .. } => "manager_04.citizens.discarded.already_attached",
        }
    }
}

/// What an incoming entity got: the one `setEntity` or one of the three discards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: attach or discard, which is Java's whole branch
pub enum Admission {
    Attached,
    Discarded(Why),
}

/// What a removal leaves for its caller to do (`removeCivilian`, `:378-407`), in Java's
/// order: unassign, clear work, tell the watchers, recount, save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the five effects after the record is gone
pub struct Departure {
    /// The buildings that had this citizen as their worker (`:388-394`).
    pub released: Vec<[i32; 3]>,
    /// The work orders to clear (`clearWorkForCitizen`, `:396`).
    pub cleared_work: i32,
    /// The recount changed something (`:404`).
    pub recalculated: bool,
}

/// One colony tick's input (`onColonyTick`, `:584-637`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the six facts the tick reads
pub struct Phase {
    pub has_town_hall: bool,
    pub town_hall_level: i32,
    /// `BuildingTownHall.MOVE_IN` (`:594`).
    pub move_in: bool,
    /// `ServerConfiguration.initialCitizenAmount`.
    pub initial: i32,
    pub citizens: i32,
    /// How many of them are women, counted before the newcomer (`:601-605`).
    pub females: i32,
}

/// What one tick asks the caller to do. Java does it inline; every one of these needs an
/// entity, a quest or a message, and none of them is this module's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two actions of onColonyTick
pub enum Effect {
    /// Every citizen refreshes its entity (`updateEntityIfNecessary`, `:590`).
    Refresh,
    /// One arrives. `gender` is `None` for the first, whose sex Java **rolls** (`:617`);
    /// `first` carries the tutorial quest and the log line (`:607-612`, `:633-634`).
    Spawn { gender: Option<bool>, first: bool },
}

/// Who mourns, and who is merely told (`updateCitizenMourn`, `:640-657`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the two lists the loop builds
pub struct Mourning {
    /// The citizens that record the name: family or housemates, neither guards nor the
    /// undertaker (`:646-649`).
    pub recorded: Vec<i32>,
    /// Everyone, which `citizen.onDeath(data.getId())` runs for in both branches
    /// (`:655`).
    pub notified: Vec<i32>,
}

/// What `onCitizenSleep` decided (`:671-688`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three answers of the loop
pub enum Rest {
    /// Somebody is up and is not a guard: the method returns without a word (`:676-679`).
    NotYet,
    /// Everybody is in bed and the colony already knew.
    Quiet,
    /// Everybody is in bed and the colony did not know (`:682-685`).
    Announce,
}

/// Whether a citizen mourns a death: not a guard, not the undertaker, and family or
/// housemate (`:646`).
pub fn mourns(job: Option<Job>, related_or_coresident: bool) -> bool {
    if !related_or_coresident {
        return false;
    }
    match job {
        Some(job) => !is_guard(job) && job != Job::Undertaker,
        None => true,
    }
}

/// The manager's own bookkeeping: the id pool, the ceiling, the two timers, two flags.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: `CitizenManager`'s fields that are not a citizen record
pub struct Manager {
    heads: BTreeMap<i32, Head>,
    top: i32,
    ceiling: Ceiling,
    spawn: i32,
    entities: i32,
    dirty: bool,
    sleeping: bool,
}

impl Default for Manager {
    fn default() -> Manager {
        Manager {
            heads: BTreeMap::new(),
            top: 0,
            ceiling: Ceiling::default(),
            spawn: RESPAWN_INTERVAL,
            entities: ENTITY_TIMER,
            dirty: false,
            sleeping: false,
        }
    }
}

impl Manager {
    /// A colony with no citizen, Java's two field initialisers applied
    /// (`:95`, `:100`).
    pub fn new() -> Manager {
        Manager::default()
    }

    pub fn heads(&self) -> &BTreeMap<i32, Head> {
        &self.heads
    }

    pub fn head(&self, id: i32) -> Option<&Head> {
        self.heads.get(&id)
    }

    pub fn len(&self) -> usize {
        self.heads.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }

    pub fn ceiling(&self) -> Ceiling {
        self.ceiling
    }

    /// `isCitizensDirty` (`:70`), which is `Flags::citizens` over in
    /// [`crate::manager_04::subscribers`].
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// `markDirty` (`:476-481`): the colony's flag and this one, in that order.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `clearDirty` (`:489-494`): this flag **and** every citizen's.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn are_sleeping(&self) -> bool {
        self.sleeping
    }

    /// `getCurrentCitizenCount` (`:533-537`).
    pub fn count(&self) -> i32 {
        self.heads.len() as i32
    }

    /// The id the next citizen takes: **the lowest hole in `1..=count+1`**, and
    /// `topCitizenId` follows it down (`:331-340`).
    ///
    /// Java's comment says why: *"This ensures that citizen IDs are getting reused. That's
    /// needed to prevent bugs when calling IDs that are not used."* A colony that lost
    /// citizen 1 hands 1 out again, even if it once had 40.
    pub fn next_id(&mut self, occupied: &BTreeSet<i32>) -> Result<i32, Refusal> {
        let count = occupied.len() as i32;
        for candidate in 1..=count + 1 {
            if !occupied.contains(&candidate) {
                self.top = candidate;
                return Ok(candidate);
            }
        }
        Err(Refusal::NoFreeId { count })
    }

    /// `createAndRegisterCivilianData` (`:328-347`): the id, then the record, then
    /// `initForNewCivilian` — a fresh, unattached, jobless adult, which is
    /// [`Head::new`] with a uuid the caller draws.
    ///
    /// It does **not** mark anything dirty: Java's `markDirty` sits in
    /// `spawnCitizenOnPosition` (`:324`), after the entity exists.
    pub fn create(&mut self, uuid: u128) -> Result<i32, Refusal> {
        let occupied: BTreeSet<i32> = self.heads.keys().copied().collect();
        let id = self.next_id(&occupied)?;
        self.heads.insert(id, Head::new(id, uuid));
        Ok(id)
    }

    /// `registerCivilian` (`:122-160`): four branches, and only one of them attaches.
    ///
    /// The order is Java's: the id must exist (and not be zero), the uuid must match,
    /// and only then does "already has an entity" become the third answer.
    pub fn register(&mut self, id: i32, uuid: u128) -> Admission {
        if id == 0 {
            return Admission::Discarded(Why::Unknown { id });
        }
        let Some(head) = self.heads.get_mut(&id) else {
            return Admission::Discarded(Why::Unknown { id });
        };
        if head.uuid != uuid {
            return Admission::Discarded(Why::UuidMismatch { id });
        }
        if head.attached {
            return Admission::Discarded(Why::AlreadyAttached { id });
        }
        head.attached = true;
        Admission::Attached
    }

    /// `unregisterCivilian` (`:162-170`): the citizen survives, its entity does not.
    pub fn unregister(&mut self, id: i32) {
        if let Some(head) = self.heads.get_mut(&id) {
            head.attached = false;
        }
    }

    /// `removeCivilian` (`:377-407`), in Java's order — and the refusal comes first, so a
    /// removal that names nobody writes nothing.
    pub fn remove(&mut self, id: i32, standing: &Standing) -> Result<Departure, Refusal> {
        if self.heads.remove(&id).is_none() {
            return Err(Refusal::UnknownCitizen { id });
        }
        let released: Vec<[i32; 3]> = standing
            .iter()
            .filter(|building| building.worker() == Some(id))
            .map(|building| building.at())
            .collect();
        let recalculated = self.recalculate(standing);
        self.mark_dirty();
        Ok(Departure {
            released,
            cleared_work: id,
            recalculated,
        })
    }

    /// `calculateMaxCitizens` (`:423-459`), **with Java's setters inside the `if`**.
    ///
    /// The walk itself is `crate::building::behaviour::census`; what this does is compare
    /// the *clamped* old ceiling to the *raw* new one and, only on a difference, store the
    /// raw numbers. Returns whether Java would have called `colony.markDirty()`.
    pub fn recalculate(&mut self, standing: &Standing) -> bool {
        let counted = census(standing);
        let spare = (counted.potential - counted.max).max(0);
        let changed = self.ceiling.max() != counted.max || self.ceiling.potential() != counted.potential;
        if changed {
            self.ceiling = Ceiling {
                beds: counted.max,
                spare,
                ..self.ceiling
            };
            self.mark_dirty();
        }
        changed
    }

    /// The ceiling the server and the research tree allow, set by the caller who read
    /// them (`getMaxCitizens` calls `MineColonies.getConfig()` and the research manager,
    /// `:502-526`).
    pub fn set_caps(&mut self, research_effect: Option<i32>, config: i32) {
        self.ceiling = Ceiling {
            research_effect,
            config,
            ..self.ceiling
        };
    }

    /// The two timers, one colony tick (`onColonyTick`, `:584-637`).
    ///
    /// Java short-circuits: without a town hall the entity timer does not even tick
    /// (`:587`). And the spawn timer only runs when the colony is **under** its initial
    /// count *and* takes newcomers (`:594`).
    pub fn on_tick(&mut self, phase: Phase) -> Vec<Effect> {
        let mut effects = Vec::new();
        if phase.has_town_hall {
            self.entities -= TIER_TICK;
            if self.entities < 0 {
                self.entities = ENTITY_TIMER;
                effects.push(Effect::Refresh);
            }
        }
        if !phase.move_in || !phase.has_town_hall || phase.citizens >= phase.initial {
            return effects;
        }
        self.spawn -= TIER_TICK + SECONDS_A_MINUTE * phase.town_hall_level;
        if self.spawn > 0 {
            return effects;
        }
        self.spawn = RESPAWN_RESET;
        let first = phase.citizens == 0;
        let gender = if first {
            None
        } else if phase.females * 2 < phase.citizens {
            // `femaleCount < (size - 1) / 2.0`, in integers: the double division is what
            // makes three citizens stop at one woman.
            Some(true)
        } else {
            Some(false)
        };
        effects.push(Effect::Spawn { gender, first });
        effects
    }

    /// `updateCitizenMourn` (`:639-657`): who records the name, and who is told anyway.
    ///
    /// `related` is the set of citizens that are family of, or live with, the deceased —
    /// facts this port keeps in `crate::citizens::happiness` and nowhere else, so the
    /// caller says which they are. **One line of this port's own**: the deceased is never
    /// in its own list, where Java leans on `isRelatedTo(self)` answering false.
    pub fn mourn(&self, deceased: i32, mourn: bool, related: &BTreeSet<i32>) -> Mourning {
        let mut mourning = Mourning::default();
        for head in self.heads.values() {
            mourning.notified.push(head.id);
            if !mourn {
                continue;
            }
            if mourns(head.job, related.contains(&head.id)) && head.id != deceased {
                mourning.recorded.push(head.id);
            }
        }
        mourning
    }

    /// `onCitizenSleep` (`:671-688`): the announcement is made once, and only when every
    /// non-guard is in bed.
    pub fn all_asleep(&mut self) -> Rest {
        let every = self
            .heads
            .values()
            .all(|head| head.asleep || head.job.map(is_guard).unwrap_or(false));
        if !every {
            return Rest::NotYet;
        }
        let announce = !self.sleeping;
        self.sleeping = true;
        if announce {
            Rest::Announce
        } else {
            Rest::Quiet
        }
    }

    /// `getJoblessCitizen` (`:409-421`): the first adult with no hut.
    ///
    /// Java walks a `HashMap`, so *which* jobless citizen it answers is unspecified; a
    /// `BTreeMap` answers the lowest id, which is the only order a test can hold.
    pub fn jobless(&self) -> Option<i32> {
        self.heads
            .values()
            .find(|head| head.job.is_none() && !head.child)
            .map(|head| head.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::{Building, Kind};

    fn residence(level: i32) -> Standing {
        let mut standing = Standing::new();
        let mut hut = Building::new(Kind::Residence, [10, 70, 10]);
        hut.complete(level).expect("a residence");
        standing.insert(hut);
        standing
    }

    fn staffed(citizens: &[(i32, Option<Job>)]) -> Manager {
        let mut manager = Manager::new();
        for (id, job) in citizens {
            let mut head = Head::new(*id, u128::from(*id as u32));
            head.job = *job;
            manager.heads.insert(*id, head);
        }
        manager
    }

    #[test]
    fn ids_are_reused_from_the_lowest_hole_and_the_counter_follows_them_down() {
        let mut manager = Manager::new();
        assert_eq!(manager.create(1).expect("an empty colony"), 1);
        assert_eq!(manager.create(2).expect("the second"), 2);
        assert_eq!(manager.create(3).expect("the third"), 3);
        assert_eq!(manager.count(), 3);

        // Removing the first frees the lowest id, and Java hands it out again (`:333`).
        let departure = manager.remove(1, &Standing::new()).expect("a citizen");
        assert_eq!(departure.cleared_work, 1);
        assert_eq!(manager.create(4).expect("the free one"), 1);
        assert_eq!(manager.len(), 3);

        // The counter is not a maximum: a colony that loses its highest id goes back.
        let mut high = Manager::new();
        for id in [7, 8] {
            high.heads.insert(id, Head::new(id, u128::from(id as u32)));
        }
        assert_eq!(high.create(9).expect("the hole at one"), 1);
        // The scan bounds are `1..=count+1`, and the id it answers is inside them.
        let occupied: BTreeSet<i32> = vec![1, 2, 3].into_iter().collect();
        assert_eq!(manager.next_id(&occupied), Ok(4));
        let occupied: BTreeSet<i32> = vec![1, 2, 3, 4].into_iter().collect();
        let mut probe = Manager::new();
        assert_eq!(probe.next_id(&occupied), Ok(5));
    }

    #[test]
    fn a_removal_that_names_nobody_writes_nothing_and_an_entity_is_let_in_once() {
        let mut manager = staffed(&[(4, Some(Job::Farmer))]);
        assert_eq!(
            manager.remove(9, &residence(3)),
            Err(Refusal::UnknownCitizen { id: 9 })
        );
        assert_eq!(manager.len(), 1, "no record was touched");

        assert_eq!(
            manager.register(0, 4),
            Admission::Discarded(Why::Unknown { id: 0 }),
            "the id zero means none, :125"
        );
        assert_eq!(
            manager.register(8, 4),
            Admission::Discarded(Why::Unknown { id: 8 })
        );
        assert_eq!(
            manager.register(4, 5),
            Admission::Discarded(Why::UuidMismatch { id: 4 })
        );
        assert_eq!(manager.register(4, 4), Admission::Attached);
        assert_eq!(
            manager.register(4, 4),
            Admission::Discarded(Why::AlreadyAttached { id: 4 }),
            "one citizen, one entity, :149-159"
        );
        assert!(manager.head(4).map(|head| head.attached).unwrap_or(false));
        manager.unregister(4);
        assert!(!manager.head(4).map(|head| head.attached).unwrap_or(true));
        assert_eq!(manager.register(4, 4), Admission::Attached, "and it may come back");
        assert_eq!(
            Why::UuidMismatch { id: 4 }.name(),
            "manager_04.citizens.discarded.uuid_mismatch"
        );
    }

    #[test]
    fn the_ceiling_recounts_the_beds_and_swallows_the_first_one() {
        let mut manager = Manager::new();
        manager.set_caps(None, 250);
        // Before any count, the raw zero is clamped to one (`:505`).
        assert_eq!(manager.ceiling().max(), 1);
        assert_eq!(manager.ceiling().beds, 0);

        // A colony with no bed re-stores its zero and marks dirty on **every** count,
        // because 1 (clamped) never equals 0 (raw).
        assert!(manager.recalculate(&Standing::new()));
        assert_eq!(manager.ceiling().beds, 0);

        // The first bed a colony ever builds changes nothing: 1 == 1 both ways, so the
        // setters do not run and the stored count stays 0 (`:453-458`).
        let one = residence(1);
        assert_eq!(census(&one).max, 1, "a residence at level one holds one");
        assert!(!manager.recalculate(&one));
        assert_eq!(manager.ceiling().beds, 0, "the first bed is swallowed");
        assert_eq!(manager.ceiling().max(), 1, "and the getter is still right");

        // The second bed is a change, and it stores the raw number.
        let three = residence(3);
        let expected = census(&three);
        assert!(manager.recalculate(&three));
        assert_eq!(manager.ceiling().beds, expected.max);
        assert_eq!(manager.ceiling().max(), expected.max);

        // Nothing changed: the next pass is not a change and does not mark dirty.
        manager.clear_dirty();
        assert!(!manager.recalculate(&three));
        assert!(!manager.dirty(), "an unchanged count never marks the colony");
    }

    #[test]
    fn the_two_caps_clamp_the_beds_and_never_below_one() {
        let ceiling = Ceiling {
            beds: 40,
            spare: 5,
            research_effect: Some(30),
            config: 20,
        };
        assert_eq!(ceiling.research(), 20, "min(max(25, 30), 20)");
        assert_eq!(ceiling.max(), 20);
        assert_eq!(ceiling.potential(), 20);
        // Research below the default leaves 25 as the floor, capped by the server.
        let low = Ceiling {
            research_effect: Some(10),
            ..ceiling
        };
        assert_eq!(low.research(), 20, "min(max(25, 10), 20) = 20");
        // No research effect: the server setting alone.
        let none = Ceiling {
            research_effect: None,
            ..ceiling
        };
        assert_eq!(none.research(), 20);
        // Beds below the caps: the beds win.
        let small = Ceiling {
            beds: 3,
            spare: 2,
            research_effect: None,
            config: 250,
        };
        assert_eq!(small.max(), 3);
        assert_eq!(small.potential(), 5);
        // And a colony with nothing still answers one.
        assert_eq!(Ceiling::default().max(), 1);
    }

    #[test]
    fn the_arrival_timer_takes_500_plus_sixty_a_level_and_resets_to_1200() {
        let mut manager = Manager::new();
        let phase = Phase {
            has_town_hall: true,
            town_hall_level: 1,
            move_in: true,
            initial: 4,
            citizens: 0,
            females: 0,
        };
        // 560 ticks a pass, from 600: the second pass is the one that fires.
        assert_eq!(manager.spawn, RESPAWN_INTERVAL);
        assert!(manager.on_tick(phase).is_empty());
        assert_eq!(manager.spawn, RESPAWN_INTERVAL - 560);
        let effects = manager.on_tick(phase);
        assert_eq!(effects, vec![Effect::Spawn { gender: None, first: true }]);
        assert_eq!(manager.spawn, RESPAWN_RESET, "reset to 60 seconds, :600");

        // A level-five town hall takes 800 a pass, which is more than the 600 the timer
        // holds: the very first tick already spawns.
        let mut fast = Manager::new();
        let five = Phase {
            town_hall_level: 5,
            ..phase
        };
        assert_eq!(
            fast.on_tick(five),
            vec![Effect::Spawn {
                gender: None,
                first: true
            }]
        );
        assert_eq!(fast.spawn, RESPAWN_RESET);
        assert!(
            RESPAWN_INTERVAL - (TIER_TICK + SECONDS_A_MINUTE * 5) < 0,
            "600 - 800 is negative, which is why it fires at once"
        );

        // No town hall, no newcomers, or a colony already at its initial count: the timer
        // does not move at all.
        let mut still = Manager::new();
        for blocked in [
            Phase {
                has_town_hall: false,
                ..phase
            },
            Phase {
                move_in: false,
                ..phase
            },
            Phase {
                citizens: 4,
                ..phase
            },
        ] {
            assert!(still.on_tick(blocked).is_empty());
            assert_eq!(still.spawn, RESPAWN_INTERVAL);
        }
    }

    #[test]
    fn the_entity_timer_is_6000_ticks_and_does_not_tick_without_a_town_hall() {
        let mut manager = Manager::new();
        let phase = Phase {
            has_town_hall: true,
            ..Phase::default()
        };
        assert!(manager.on_tick(phase).is_empty());
        assert_eq!(manager.entities, ENTITY_TIMER - TIER_TICK);
        let mut ticks = 1;
        loop {
            let effects = manager.on_tick(phase);
            ticks += 1;
            if !effects.is_empty() {
                assert_eq!(effects, vec![Effect::Refresh]);
                break;
            }
            assert!(ticks < 100, "the timer must fire within 6000/500 ticks");
        }
        assert_eq!(ticks, 13, "6000 / 500 = 12 ticks of 500, then the 13th is negative");
        assert_eq!(manager.entities, ENTITY_TIMER);

        let mut idle = Manager::new();
        assert!(idle
            .on_tick(Phase {
                has_town_hall: false,
                ..phase
            })
            .is_empty());
        assert_eq!(idle.entities, ENTITY_TIMER, ":587 short-circuits the decrement");
    }

    #[test]
    fn the_first_comer_s_sex_is_a_roll_then_the_minority_sex_wins_by_a_double_division() {
        let mut manager = Manager::new();
        let phase = Phase {
            has_town_hall: true,
            town_hall_level: 5,
            move_in: true,
            initial: 10,
            citizens: 0,
            females: 0,
        };
        // Force the timer to fire in one pass: level 5 takes 800 of the 600.
        let effects = manager.on_tick(phase);
        assert_eq!(effects, vec![Effect::Spawn { gender: None, first: true }]);

        // Two women of three: `2 < 3 / 2.0 = 1.5` is false, so the newcomer is male.
        let three = Phase {
            citizens: 3,
            females: 2,
            ..phase
        };
        let mut manager = Manager::new();
        assert_eq!(
            manager.on_tick(three),
            vec![Effect::Spawn {
                gender: Some(false),
                first: false
            }]
        );
        // One woman of three: `1 < 1.5` is true, so the newcomer is female — which is how
        // a colony of four gets two of each.
        let one = Phase {
            citizens: 3,
            females: 1,
            ..phase
        };
        let mut manager = Manager::new();
        assert_eq!(
            manager.on_tick(one),
            vec![Effect::Spawn {
                gender: Some(true),
                first: false
            }]
        );
        // And one of two: `1 < 1.0` is false, so the second is male.
        let two = Phase {
            citizens: 2,
            females: 1,
            ..phase
        };
        let mut manager = Manager::new();
        assert_eq!(
            manager.on_tick(two),
            vec![Effect::Spawn {
                gender: Some(false),
                first: false
            }]
        );
    }

    #[test]
    fn guards_and_the_undertaker_do_not_mourn_but_every_citizen_is_told() {
        assert!(mourns(Some(Job::Farmer), true));
        assert!(!mourns(Some(Job::Farmer), false), "a stranger does not mourn");
        assert!(!mourns(Some(Job::Knight), true), "a guard does not mourn its own");
        assert!(!mourns(Some(Job::Undertaker), true), "nor the undertaker");
        assert!(mourns(None, true), "a jobless citizen may be family");

        let manager = staffed(&[
            (1, Some(Job::Farmer)),
            (2, Some(Job::Knight)),
            (3, Some(Job::Undertaker)),
            (4, None),
            (5, Some(Job::Baker)),
        ]);
        let related: BTreeSet<i32> = vec![1, 2, 3, 5].into_iter().collect();
        let mourning = manager.mourn(5, true, &related);
        assert_eq!(
            mourning.recorded,
            vec![1],
            ":646 spares the guards, the undertaker and the stranger"
        );
        assert_eq!(
            mourning.notified,
            vec![1, 2, 3, 4, 5],
            "every citizen runs onDeath, the deceased included"
        );
        // Un-mourning touches nobody's record list, and still tells everyone.
        let mourning = manager.mourn(5, false, &related);
        assert!(mourning.recorded.is_empty());
        assert_eq!(mourning.notified.len(), 5);
    }

    #[test]
    fn the_sleep_announcement_waits_for_every_non_guard_and_is_made_once() {
        let mut manager = staffed(&[(1, Some(Job::Farmer)), (2, Some(Job::Knight))]);
        manager.heads.get_mut(&1).expect("citizen 1").asleep = true;
        // Citizen 2 is a guard and already "asleep" by the rule, so everyone counts.
        assert_eq!(manager.all_asleep(), Rest::Announce);
        assert!(manager.are_sleeping());
        assert_eq!(manager.all_asleep(), Rest::Quiet, "said once, :682");

        let mut awake = staffed(&[(1, Some(Job::Farmer)), (2, None)]);
        awake.heads.get_mut(&1).expect("citizen 1").asleep = true;
        assert_eq!(awake.all_asleep(), Rest::NotYet, ":676-679 returns without a word");

        // Jobless: the first adult with no hut, lowest id, in a map Java leaves unordered.
        assert_eq!(awake.jobless(), Some(2));
        let mut child = staffed(&[(1, None)]);
        child.heads.get_mut(&1).expect("citizen 1").child = true;
        assert_eq!(child.jobless(), None, "a child is not a jobless worker");
    }

    #[test]
    fn a_removal_frees_the_buildings_that_employed_the_citizen() {
        let mut standing = residence(3);
        let mut hut = Building::new(Kind::Farmer, [20, 70, 20]);
        hut.complete(2).expect("a farmer's hut");
        hut.hire(7).expect("the hut hires citizen 7");
        standing.insert(hut);
        let mut manager = staffed(&[(7, Some(Job::Farmer))]);
        let departure = manager.remove(7, &standing).expect("a citizen");
        assert_eq!(departure.released, vec![[20, 70, 20]]);
        assert!(manager.heads().is_empty());
        let spawn = spawn_point([1, 64, 2]);
        assert_eq!([spawn[0], spawn[2]], [1.5, 2.5], "half a block, Constants.java:48");
        assert!((spawn[1] - 64.1).abs() < 1e-9, "a tenth up, Constants.java:49");
    }
}
