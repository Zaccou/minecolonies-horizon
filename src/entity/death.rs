//! The three ends of a citizen: what a death writes, when it runs away, and what
//! the world may remove.
//!
//! Java reference: `core/entity/citizen/EntityCitizen.java:1548-1637` (`die`,
//! `remove`), `core/colony/managers/CitizenManager.java:640-657` (`updateCitizenMourn`)
//! and `:378-407` (`removeCivilian`), `core/entity/ai/minimal/EntityAICitizenAvoidEntity.java`
//! (the flee task), `core/entity/ai/workers/guard/AbstractEntityAIGuard.java:349-410`,
//! `core/entity/citizen/citizenhandlers/CitizenColonyHandler.java:78-113`.
//!
//! **A death is bookkeeping.** Nothing keeps a corpse: the entity is removed at
//! `:1555`, and the statements after it write into four other objects — the
//! colony's happiness (`DEATH`, factor 3, three days, `:1558-1561`), the mourning
//! list (`:1568`), a grave (**only** inside the colony and only if visible,
//! `:1566-1587`), and the roster plus the job (`:1620-1627`). **Two citizens are
//! spared the penalty and the mourning: guards** (`:1557`) — a guard's death is
//! the job it was hired for.
//!
//! **Fleeing is two rules with different triggers.** A civilian runs from the
//! class of entity it was built to avoid, whatever its health; a **guard** below a
//! fifth of its health runs home, and only with `retreatunlock` researched
//! (`AbstractEntityAIGuard.java:353-359`).
//!
//! **A citizen never despawns**: `requiresCustomPersistence()` is `true`
//! (`EntityCitizen.java:1806-1810`), so vanilla's distance rule never applies. What
//! *can* remove it is its own registration: a zero id or an unknown colony is
//! discarded on join (`CitizenColonyHandler.java:91-113`).

use crate::citizens::is_guard;
use crate::jobs::Job;

/// `ExpirationBasedHappinessModifier(DEATH, 3.0, supplier(0.0), 3)` (`:1558-1561`).
pub const DEATH_FACTOR: f64 = 3.0;
pub const DEATH_DAYS: i32 = 3;
/// `getHealth() < ((int) getMaxHealth() * 0.2D)` (`AbstractEntityAIGuard.java:355`).
pub const RETREAT_FRACTION: f64 = 0.2;
/// `distanceToSqr(center) > 20` (`:355`): a guard at its hut does not run to it.
pub const RETREAT_DISTANCE_SQ: f64 = 20.0;
/// `getHealth() < ((int) getMaxHealth() * 0.75D)` (`:372`).
pub const REGEN_FRACTION: f64 = 0.75;
/// `REGENERATION, 200` (`:377`) — ten seconds at the hut.
pub const REGEN_TICKS: u32 = 200;
/// `MOVEMENT_SPEED, 200, effect` while fleeing (`:396-399`).
pub const FLEE_EFFECT_TICKS: u32 = 200;
/// `TOO_CLOSE_TO_MOB = 4D` (`EntityAICitizenAvoidEntity.java:33`).
pub const TOO_CLOSE: f64 = 4.0;
/// `CHECKS_BEFORE_SAFE = 20 * 10 / 5` (`:38`).
pub const CHECKS_BEFORE_SAFE: u32 = 40;
/// The `AITarget`'s period, in ticks (`:105-110`).
pub const CHECK_TICKS: u32 = 5;
/// `MIN_MOVE_AWAY_DIST`, `MED_`, `MAX_` (`:40-42`), and the `- 4` margin of the
/// first band (`:194`).
pub const MOVE_AWAY_MIN: f64 = 7.0;
pub const MOVE_AWAY_MED: f64 = 15.0;
pub const MOVE_AWAY_MAX: f64 = 20.0;
pub const MOVE_AWAY_MARGIN: f32 = 4.0;

/// Why the colony side of a death cannot be written, or a removal refused.
///
/// **No `Eq`**: three variants carry an `f32` or an `f64`, and a float is not
/// `Eq` in Rust (`NaN != NaN`). `PartialEq` is what every caller needs — these
/// are compared and logged, never used as a map key.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// No colony record: Java's `if (colony != null && data != null)` (`:1550`)
    /// writes nothing at all — no grave, no mourning — and leaves a citizen in the
    /// roster for an entity that is gone. The port reports it.
    NoColony { citizen: i32 },
    /// A guard above the health line (`AbstractEntityAIGuard.java:353-359`).
    NotWounded { health: f32 },
    /// A guard already within twenty blocks of its hut: running home from home.
    AlreadyHome { distance_sq: f64 },
    /// `retreatunlock` not researched (`ResearchConstants.java:161`): the guard
    /// stands and fights (`AbstractEntityAIGuard.java:357`).
    RetreatLocked { distance_sq: f64 },
    /// Asked to let the world despawn a citizen by distance: it may not.
    Persistent,
    /// Asked to end a flee that is not running (`EntityAICitizenAvoidEntity.java:249`).
    NotFleeing,
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(self) -> &'static str {
        match self {
            Self::NoColony { .. } => "entity.refusal.no_colony",
            Self::NotWounded { .. } => "entity.refusal.not_wounded",
            Self::AlreadyHome { .. } => "entity.refusal.already_home",
            Self::RetreatLocked { .. } => "entity.refusal.retreat_locked",
            Self::Persistent => "entity.refusal.persistent",
            Self::NotFleeing => "entity.refusal.not_fleeing",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Where the citizen stood when it died — the two questions `die` asks before it
/// writes (`:1550`, `:1566-1587`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `EntityCitizen.java:1550` and `:1566-1587`
pub enum Standing {
    InColony { inside: bool, invisible: bool },
    NoColony,
}

/// What one death writes into the colony.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: four fields, one per object Java writes into
pub struct Loss {
    /// The `DEATH` modifier: `Some((3.0, 3))` for everyone but a guard (`:1557-1561`).
    pub happiness: Option<(f64, i32)>,
    /// Whether `updateCitizenMourn(_, true)` runs for it (`:1568`).
    pub mourns: bool,
    /// Whether a grave is placed (`:1573-1577`).
    pub grave: bool,
    /// Whether the inventory spills where it fell (`:1578-1581`).
    pub drops: bool,
}

impl Loss {
    /// A citizen with no colony: nothing is written (`:1550`).
    pub const fn nothing() -> Loss {
        Loss {
            happiness: None,
            mourns: false,
            grave: false,
            drops: false,
        }
    }
}

/// `EntityCitizen.die`, reduced to what the colony keeps. The two branches are
/// Java's: a guard skips the penalty and the mourning (`:1557`, `:1568`), and the
/// grave needs a position inside the colony **and** a visible citizen.
pub fn on_death(citizen: i32, job: Option<Job>, standing: Standing) -> Result<Loss, Refusal> {
    let (inside, invisible) = match standing {
        Standing::NoColony => return Err(Refusal::NoColony { citizen }),
        Standing::InColony { inside, invisible } => (inside, invisible),
    };
    let guard = job.is_some_and(is_guard);
    Ok(Loss {
        happiness: (!guard).then_some((DEATH_FACTOR, DEATH_DAYS)),
        mourns: !guard,
        grave: !invisible && inside,
        drops: !invisible && !inside,
    })
}

/// `!guard && !JobUndertaker && (isRelatedTo || doesLiveWith)`
/// (`CitizenManager.java:646`). An undertaker does not mourn: it buries.
pub fn mourns(observer: Option<Job>, related: bool, lives_with: bool) -> bool {
    if observer.is_some_and(is_guard) || observer == Some(Job::Undertaker) {
        return false;
    }
    related || lives_with
}

/// A removal request, from the side of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the vanilla removal reasons a citizen can meet
pub enum Removal {
    /// Health reached zero: `remove(RemovalReason.KILLED)` (`:1555`).
    Died,
    /// The chunk unloaded: the entity goes, the colony record stays.
    ChunkUnloaded,
    /// Vanilla's distance despawn — the path `requiresCustomPersistence()` closes
    /// (`:1806-1810`).
    WorldDiscard,
}

/// Everything but [`Removal::WorldDiscard`] is allowed. `remove` posts
/// `CitizenRemovedModEvent` for every reason it does get (`:1632-1637`), so this
/// refusal protects an event other code listens to.
pub fn removal_allowed(reason: Removal) -> Result<Removal, Refusal> {
    match reason {
        Removal::Died | Removal::ChunkUnloaded => Ok(reason),
        Removal::WorldDiscard => Err(Refusal::Persistent),
    }
}

/// How far a fleeing citizen runs, from its own health (`EntityAICitizenAvoidEntity.java:191-203`).
pub fn move_away_distance(health: f32, max_health: f32) -> f64 {
    if health >= max_health - MOVE_AWAY_MARGIN {
        MOVE_AWAY_MIN
    } else if health >= max_health / 2.0 {
        MOVE_AWAY_MED
    } else {
        MOVE_AWAY_MAX
    }
}

/// The two speeds of the flee (`:229-241`): `Near` when the threat is under
/// [`TOO_CLOSE`] blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of the `if` at `:229`
pub enum Pace {
    Near,
    Far,
}

pub fn pace(distance_to_threat: f64) -> Pace {
    if distance_to_threat < TOO_CLOSE {
        Pace::Near
    } else {
        Pace::Far
    }
}

/// Forty quiet checks end the flee and walk the citizen back (`:120-127`, `:249-260`).
pub fn flee_is_over(quiet_checks: u32) -> bool {
    quiet_checks > CHECKS_BEFORE_SAFE
}

/// Whether the flee task may be reset: `reset` is only reached from its own states
/// (`:249-260`), so a task that is not running has nothing to cancel.
pub fn flee_may_reset(running: bool) -> Result<(), Refusal> {
    if running {
        Ok(())
    } else {
        Err(Refusal::NotFleeing)
    }
}

/// When a guard may run home instead of fighting (`AbstractEntityAIGuard.java:353-359`).
/// The three refusals are the three reasons Java's `shouldFlee` answers `false`.
pub fn guard_may_flee(
    health: f32,
    max_health: f32,
    distance_sq: f64,
    retreat_researched: bool,
) -> Result<(), Refusal> {
    if f64::from(health) >= f64::from(max_health) * RETREAT_FRACTION {
        return Err(Refusal::NotWounded { health });
    }
    if distance_sq <= RETREAT_DISTANCE_SQ {
        return Err(Refusal::AlreadyHome { distance_sq });
    }
    if !retreat_researched {
        return Err(Refusal::RetreatLocked { distance_sq });
    }
    Ok(())
}

/// `health < max * 0.75` (`AbstractEntityAIGuard.java:372`).
pub fn regen_due(health: f32, max_health: f32) -> bool {
    f64::from(health) < f64::from(max_health) * REGEN_FRACTION
}

/// The two ids an entity carries into a level (`CitizenColonyHandler.java:78-113`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two ids `getEntityData` carries
pub struct Ids {
    pub colony: i32,
    pub citizen: i32,
}

/// What is known about the colony the entity names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three answers of `getColonyByWorld` + `getWorld`
pub enum ColonyState {
    /// Found, world up: the citizen registers (`:106-108`).
    Loaded,
    /// Found, `getWorld() == null`: wait, do not discard (`:100-104`).
    Loading,
    /// `getColonyByWorld` answered null (`:96-100`).
    NotFound,
}

/// What the level must do with a citizen entity that just joined it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three outcomes of `registerWithColony`
pub enum Fate {
    /// Registered, or already was (`:84-87`).
    Persist,
    /// The colony is loading (`:102-104`).
    Wait,
    /// Discarded, with the reason Java logs.
    Discard(Why),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two discarding branches
pub enum Why {
    /// `colonyId == 0 || citizenId == 0` (`:91-94`).
    ZeroId,
    /// The colony named by the id does not exist in this level (`:96-100`).
    ColonyUnknown,
}

/// `registerWithColony`, in Java's order: already registered wins, a zero id is
/// discarded **before** the colony is looked up, and a colony without a world is
/// waited for instead of discarded.
pub fn on_join(ids: Ids, already_registered: bool, colony: ColonyState) -> Fate {
    if already_registered {
        return Fate::Persist;
    }
    if ids.colony == 0 || ids.citizen == 0 {
        return Fate::Discard(Why::ZeroId);
    }
    match colony {
        ColonyState::NotFound => Fate::Discard(Why::ColonyUnknown),
        ColonyState::Loading => Fate::Wait,
        ColonyState::Loaded => Fate::Persist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizens::{GUARD_JOBS, MAX_HEALTH};

    const IN: Standing = Standing::InColony {
        inside: true,
        invisible: false,
    };

    #[test]
    fn a_worker_death_costs_three_days_of_happiness_a_mourning_and_a_grave() {
        let loss = on_death(7, Some(Job::Farmer), IN).expect("a colony citizen");
        assert_eq!(loss.happiness, Some((3.0, 3)));
        assert!(loss.mourns);
        assert!(loss.grave);
        assert!(!loss.drops, "a grave is not a drop");
    }

    #[test]
    fn a_guard_death_skips_the_penalty_and_the_mourning() {
        for job in GUARD_JOBS {
            let loss = on_death(7, Some(job), IN).expect("a colony citizen");
            assert_eq!(loss.happiness, None, "{job:?} is a guard");
            assert!(!loss.mourns, "{job:?} is not mourned");
            assert!(loss.grave, "the grave is not a guard's privilege");
        }
    }

    #[test]
    fn outside_the_colony_the_inventory_drops_and_an_invisible_one_leaves_nothing() {
        let outside = Standing::InColony {
            inside: false,
            invisible: false,
        };
        let loss = on_death(7, Some(Job::Builder), outside).expect("a colony citizen");
        assert!(!loss.grave);
        assert!(loss.drops, "CitizenData.java:1578-1581");
        let hidden = on_death(
            7,
            Some(Job::Builder),
            Standing::InColony {
                inside: true,
                invisible: true,
            },
        )
        .expect("a colony citizen");
        assert!(!hidden.grave && !hidden.drops);
    }

    #[test]
    fn a_death_with_no_colony_writes_nothing_and_is_a_named_refusal_here() {
        assert_eq!(
            on_death(7, Some(Job::Baker), Standing::NoColony),
            Err(Refusal::NoColony { citizen: 7 })
        );
        assert_eq!(Loss::nothing().happiness, None);
        assert_eq!(
            Refusal::NoColony { citizen: 7 }.name(),
            "entity.refusal.no_colony"
        );
    }

    #[test]
    fn everyone_mourns_but_guards_and_the_undertaker_and_a_stranger_does_not() {
        assert!(mourns(Some(Job::Farmer), true, false));
        assert!(
            mourns(Some(Job::Farmer), false, true),
            "living together counts"
        );
        assert!(!mourns(Some(Job::Farmer), false, false));
        assert!(!mourns(Some(Job::Undertaker), true, true), "it buries");
        assert!(
            !mourns(Some(Job::Knight), true, true),
            "guards do not mourn"
        );
        assert!(mourns(None, true, false), "an unemployed citizen mourns");
    }

    #[test]
    fn a_citizen_may_die_and_unload_but_never_despawn_by_distance() {
        assert_eq!(removal_allowed(Removal::Died), Ok(Removal::Died));
        assert_eq!(
            removal_allowed(Removal::ChunkUnloaded),
            Ok(Removal::ChunkUnloaded)
        );
        assert_eq!(
            removal_allowed(Removal::WorldDiscard),
            Err(Refusal::Persistent)
        );
        assert_eq!(Refusal::Persistent.name(), "entity.refusal.persistent");
    }

    #[test]
    fn the_further_it_is_hurt_the_further_it_runs() {
        assert_eq!(move_away_distance(MAX_HEALTH, MAX_HEALTH), MOVE_AWAY_MIN);
        assert_eq!(
            move_away_distance(MAX_HEALTH - 3.0, MAX_HEALTH),
            MOVE_AWAY_MIN
        );
        assert_eq!(
            move_away_distance(MAX_HEALTH - 5.0, MAX_HEALTH),
            MOVE_AWAY_MED
        );
        assert_eq!(
            move_away_distance(MAX_HEALTH / 2.0, MAX_HEALTH),
            MOVE_AWAY_MED
        );
        assert_eq!(move_away_distance(0.0, MAX_HEALTH), MOVE_AWAY_MAX);
    }

    #[test]
    fn under_four_blocks_the_flee_is_the_near_pace_and_forty_checks_end_it() {
        assert_eq!(pace(3.9), Pace::Near);
        assert_eq!(pace(TOO_CLOSE), Pace::Far, "strictly under, as the `if` is");
        assert!(!flee_is_over(CHECKS_BEFORE_SAFE));
        assert!(
            flee_is_over(CHECKS_BEFORE_SAFE + 1),
            "safeTime > 40 ends it"
        );
        assert_eq!(
            CHECK_TICKS * CHECKS_BEFORE_SAFE / 20,
            10,
            "ten seconds of checks"
        );
    }

    #[test]
    fn a_guard_runs_home_only_wounded_far_enough_and_researched() {
        assert_eq!(
            guard_may_flee(MAX_HEALTH * 0.5, MAX_HEALTH, 100.0, true),
            Err(Refusal::NotWounded {
                health: MAX_HEALTH * 0.5
            })
        );
        assert_eq!(
            guard_may_flee(MAX_HEALTH * 0.1, MAX_HEALTH, 4.0, true),
            Err(Refusal::AlreadyHome { distance_sq: 4.0 })
        );
        assert_eq!(
            guard_may_flee(MAX_HEALTH * 0.1, MAX_HEALTH, 100.0, false),
            Err(Refusal::RetreatLocked { distance_sq: 100.0 })
        );
        assert_eq!(
            guard_may_flee(MAX_HEALTH * 0.1, MAX_HEALTH, 100.0, true),
            Ok(())
        );
        assert_eq!(flee_may_reset(true), Ok(()));
        assert_eq!(flee_may_reset(false), Err(Refusal::NotFleeing));
        assert_eq!(Refusal::NotFleeing.name(), "entity.refusal.not_fleeing");
    }

    #[test]
    fn a_guard_regenerates_under_three_quarters_of_its_health() {
        assert!(regen_due(MAX_HEALTH * 0.5, MAX_HEALTH));
        assert!(
            !regen_due(MAX_HEALTH * 0.75, MAX_HEALTH),
            "the line is not due"
        );
        assert_eq!(
            REGEN_TICKS, 200,
            "ten seconds, AbstractEntityAIGuard.java:377"
        );
        assert_eq!(FLEE_EFFECT_TICKS, 200);
    }

    #[test]
    fn a_zero_id_is_discarded_before_the_colony_is_looked_up_and_a_loading_one_waits() {
        let ids = Ids {
            colony: 3,
            citizen: 4,
        };
        assert_eq!(
            on_join(
                Ids {
                    colony: 0,
                    citizen: 4
                },
                false,
                ColonyState::Loaded
            ),
            Fate::Discard(Why::ZeroId)
        );
        assert_eq!(
            on_join(ids, false, ColonyState::NotFound),
            Fate::Discard(Why::ColonyUnknown)
        );
        assert_eq!(on_join(ids, false, ColonyState::Loading), Fate::Wait);
        assert_eq!(on_join(ids, false, ColonyState::Loaded), Fate::Persist);
        assert_eq!(
            on_join(
                Ids {
                    colony: 0,
                    citizen: 0
                },
                true,
                ColonyState::NotFound
            ),
            Fate::Persist,
            "already registered wins over every id"
        );
    }
}
