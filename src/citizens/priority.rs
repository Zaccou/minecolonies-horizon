//! The order in which a citizen's states win, and why that one and not another.
//!
//! Java reference: `core/entity/ai/workers/CitizenAI.java:134-261`
//! (`calculateNextState`, the table) and `:268-298` (`shouldEat`), with `:305-327`
//! for the rain exemption.
//!
//! **A transcription, not a design.** Java checks fourteen conditions in a fixed
//! order and returns at the first one that holds; [`Rung`] is that order, one
//! variant per `if`, and [`Situation::first_rung`] is the same list of `if`s.
//! Nothing here decides *whether* hunger outranks rain — the Java says so by
//! checking `shouldEat` at line 211 and the weather at 232, and the port keeps
//! that order and cites it.
//!
//! **The two branches.** A guard (`:136-151`) is a different citizen: it eats, it
//! is sick when its AI may be interrupted, and otherwise it works. Its three rungs
//! are therefore checked before line 154 is ever reached.
//!
//! **What is not decided here.** Every `triggerInteraction` in the Java — the raid
//! warning (`:163`), the mourning notice (`:221-224`), the rain complaint (`:238`)
//! — is a chat message, and every `onWakeUp` (`:193`, `:198`) or
//! `setCurrentDelay` (`:174`) is a side effect on an entity that does not exist
//! yet. The decision returns the state and the icon; the hooks carry the rest.

use super::needs::{Appetite, NIGHT, NOON, Needs, World, should_go_sleep};
use super::state::{Branch, CitizenState, Decision, Status};

/// The fourteen checks of `calculateNextState`, in the order Java makes them. Read
/// it top to bottom: the first rung whose condition holds is the answer, and the
/// line numbers are the `if` that fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: one variant per `if` of CitizenAI.java:134-261
pub enum Rung {
    /// A guard is hungry (`:138-141`) — **before** everything else, because the
    /// guard branch is entered at `:136` and returns from inside itself.
    GuardEats,
    /// A guard is sick and its AI may be interrupted (`:144-148`).
    GuardSick,
    /// A guard is neither: it works (`:150`). Not "idles", not "sleeps".
    GuardWorks,
    /// Ill and already in a hospital bed (`:154-158`).
    SickAtHospital,
    /// The colony is being raided (`:161-166`): everyone is sent to `SLEEP`, which
    /// is how Java hides citizens during a raid.
    Raided,
    /// Already asleep and the night is not over (`:171-176`).
    KeepSleeping,
    /// Late enough, and the walk home short enough (`:178-182`).
    GoToSleep,
    /// Ill, or hurt enough to want a doctor (`:204-208`).
    Sick,
    /// Hungry (`:211-214`).
    Eat,
    /// Bereaved (`:217-229`).
    Mourn,
    /// It rains, this worker may not work in it, and it is not the Nether
    /// (`:232-241`). The state is `IDLE`; the icon is `BAD_WEATHER`.
    Rain,
    /// A pupil child, in the afternoon (`:244-248`).
    PupilAfternoon,
    /// A job whose AI cannot go idle, with no leisure left or no interruption
    /// allowed (`:250-257`).
    Work,
    /// Nothing else held: idle at home (`:259-260`).
    Idle,
}

impl Rung {
    /// The fourteen rungs, in Java's order.
    pub const ALL: [Rung; 14] = [
        Rung::GuardEats,
        Rung::GuardSick,
        Rung::GuardWorks,
        Rung::SickAtHospital,
        Rung::Raided,
        Rung::KeepSleeping,
        Rung::GoToSleep,
        Rung::Sick,
        Rung::Eat,
        Rung::Mourn,
        Rung::Rain,
        Rung::PupilAfternoon,
        Rung::Work,
        Rung::Idle,
    ];

    /// Where this rung sits, `0` being the highest priority. The array order **is**
    /// the rank — one table expanded once, so it cannot disagree with itself.
    pub fn rank(self) -> usize {
        Rung::ALL
            .iter()
            .position(|rung| *rung == self)
            .expect("every rung is in ALL")
    }

    /// Which branch of `CitizenAI` this rung belongs to.
    pub fn branch(self) -> Branch {
        match self {
            Rung::GuardEats | Rung::GuardSick | Rung::GuardWorks => Branch::Guard,
            _ => Branch::Ordinary,
        }
    }

    /// The state and icon this rung returns — the `setVisibleStatus` call sitting
    /// next to the `return` in Java. Eating arrives bare: its icon is the eat AI's.
    pub fn outcome(self) -> Decision {
        match self {
            Rung::GuardEats | Rung::Eat => Decision::bare(CitizenState::Eating),
            Rung::GuardSick | Rung::SickAtHospital | Rung::Sick => {
                Decision::with(CitizenState::Sick, Status::Sick)
            }
            Rung::GuardWorks | Rung::Work => Decision::with(CitizenState::Work, Status::Working),
            Rung::Raided => Decision::with(CitizenState::Sleep, Status::Raided),
            Rung::KeepSleeping => Decision::with(CitizenState::Sleep, Status::Sleep),
            Rung::GoToSleep => Decision::bare(CitizenState::Sleep),
            Rung::Mourn => Decision::with(CitizenState::Mourn, Status::Mourning),
            Rung::Rain => Decision::with(CitizenState::Idle, Status::BadWeather),
            Rung::PupilAfternoon | Rung::Idle => {
                Decision::with(CitizenState::Idle, Status::House)
            }
        }
    }
}

/// Everything `calculateNextState` and `shouldEat` read, in one value.
///
/// Java reads a live `EntityCitizen` and a colony; a wasm mod cannot, so the caller
/// fills this from whatever the hook gave it. [`Situation::default`] is a jobless,
/// full, unhurt citizen on a dry peaceful afternoon with no home and no leisure —
/// the shortest way to write a test of one rung.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Situation {
    /// The guard branch or the ordinary one (`:136`).
    pub branch: Branch,
    /// Saturation, health, disease, sleep, mourning.
    pub needs: Needs,
    /// Day time, weather, dimension, raid.
    pub world: World,
    /// `CitizenData.getLeisureTime()` (`:252`).
    pub leisure_time: i32,
    /// `job.canAIBeInterrupted()` (`:253` → `AbstractJob.java:353-359`).
    pub interruptible: bool,
    /// `AbstractEntityAIBasic.canGoIdle()` (`:251`): `false` in the base class
    /// (`AbstractEntityAIBasic.java:1921-1924`), overridden by the AIs that hand
    /// their worker back to the colony when there is nothing to do.
    pub can_go_idle: bool,
    /// `getColonyJob() != null` and its AI is an `AbstractEntityAIBasic` (`:250`).
    pub has_job: bool,
    /// `citizen.isBaby() && job instanceof JobPupil` (`:244`).
    pub pupil_child: bool,
    /// `shouldWorkWhileRaining()`'s three answers (`:305-327`): the server config,
    /// the `WORKING_IN_RAIN` research, or the module's own flag — any one is `true`.
    pub works_in_rain: bool,
    /// The cook's one-in-two-hundred draw (`:285`); see [`Appetite::cook_skips`].
    pub cook_skips: bool,
    /// The home position `shouldGoSleep` measures against (`:234`).
    pub home: Option<[i32; 3]>,
    /// Where the citizen stands.
    pub at: [i32; 3],
    /// `WORK_LONGER` research strength, in thousands of ticks (`:271`).
    pub work_longer: i64,
    /// `citizen.isInvisible()` (`CitizenSleepHandler.java:240`).
    pub invisible: bool,
}

impl Default for Situation {
    fn default() -> Situation {
        Situation {
            branch: Branch::Ordinary,
            needs: Needs::new(),
            world: World::default(),
            leisure_time: 0,
            interruptible: true,
            can_go_idle: false,
            has_job: false,
            pupil_child: false,
            works_in_rain: false,
            cook_skips: false,
            home: None,
            at: [0, 0, 0],
            work_longer: 0,
            invisible: false,
        }
    }
}

impl Situation {
    /// The guard branch, everything else at its default.
    pub fn guard() -> Situation {
        Situation {
            branch: Branch::Guard,
            ..Situation::default()
        }
    }

    /// A hired citizen standing at home — the shape the sleep rungs need.
    pub fn worker(at: [i32; 3]) -> Situation {
        Situation {
            has_job: true,
            home: Some(at),
            at,
            ..Situation::default()
        }
    }

    /// `shouldEat`'s four arguments, from the situation (`:268-298`).
    pub fn appetite(&self, last: CitizenState) -> Appetite {
        Appetite {
            has_job: self.has_job,
            interruptible: self.interruptible,
            was_eating: last == CitizenState::Eating,
            cook_skips: self.cook_skips,
        }
    }

    /// The first rung whose condition holds — `calculateNextState` itself.
    pub fn first_rung(&self, last: CitizenState) -> Rung {
        // :136 — a guard is a whole branch of its own.
        if self.branch == Branch::Guard {
            if self.needs.should_eat(&self.appetite(last)) {
                return Rung::GuardEats; // :138-141
            }
            if self.needs.sick && self.interruptible {
                return Rung::GuardSick; // :144-148
            }
            return Rung::GuardWorks; // :150
        }

        if self.needs.sick && self.needs.sleeps_at_hospital {
            return Rung::SickAtHospital; // :154-158
        }
        if self.world.raided {
            return Rung::Raided; // :161-166
        }
        // :169 — `!isPastTime(NIGHT - 2000)`: from late afternoon on.
        if !self.world.is_past_time(NIGHT - 2000) {
            if last == CitizenState::Sleep {
                return Rung::KeepSleeping; // :171-176
            }
            if should_go_sleep(self.home, self.at, &self.world, self.work_longer, self.invisible) {
                return Rung::GoToSleep; // :178-182
            }
        }
        // The `else` at :184-201 is the wake-up: `onWakeUp` twice over. An effect
        // on the entity, not a state, so it belongs to the caller; the decision
        // carries on to the same rungs either way.
        if self.needs.sick || self.needs.hurt(self.branch == Branch::Guard) {
            return Rung::Sick; // :204-208
        }
        if self.needs.should_eat(&self.appetite(last)) {
            return Rung::Eat; // :211-214
        }
        if self.needs.mourning {
            return Rung::Mourn; // :217-229
        }
        if self.world.raining && !self.works_in_rain && !self.world.nether {
            return Rung::Rain; // :232-241
        }
        if self.pupil_child && self.world.time_of_day() > NOON {
            return Rung::PupilAfternoon; // :244-248
        }
        if self.has_job && !self.can_go_idle && (self.leisure_time <= 0 || !self.interruptible) {
            return Rung::Work; // :250-257
        }
        Rung::Idle // :259-260
    }

    /// What one pass of the decision answers: the state, and the icon.
    pub fn decide(&self, last: CitizenState) -> Decision {
        self.first_rung(last).outcome()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizens::needs::LOW_SATURATION;

    fn raining() -> World {
        World {
            raining: true,
            ..World::at_day_time(NOON)
        }
    }

    /// A citizen that is hungry and nothing else.
    fn hungry() -> Situation {
        Situation {
            needs: Needs {
                saturation: 2.0,
                ..Needs::new()
            },
            ..Situation::default()
        }
    }

    #[test]
    fn fourteen_rungs_and_their_ranks_are_total() {
        assert_eq!(Rung::ALL.len(), 14);
        let ranks: Vec<usize> = Rung::ALL.iter().map(|rung| rung.rank()).collect();
        let mut sorted = ranks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 14, "two rungs share a rank");
        assert_eq!(Rung::GuardEats.rank(), 0);
        assert_eq!(Rung::Idle.rank(), 13);
        assert_eq!(Rung::GuardWorks.branch(), Branch::Guard);
        assert_eq!(Rung::SickAtHospital.branch(), Branch::Ordinary);
    }

    #[test]
    fn hunger_beats_rain_and_sickness_beats_both() {
        // The brief's own mixed case: hungry *and* raining.
        let situation = Situation {
            world: raining(),
            ..hungry()
        };
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::Eat);
        assert_eq!(
            situation.decide(CitizenState::Idle).state,
            CitizenState::Eating,
            "CitizenAI.java:211 comes before :232"
        );

        let mut sick = situation;
        sick.needs.sick = true;
        assert_eq!(sick.first_rung(CitizenState::Idle), Rung::Sick);
        assert_eq!(
            sick.decide(CitizenState::Idle),
            Decision::with(CitizenState::Sick, Status::Sick)
        );
    }

    #[test]
    fn a_raid_beats_sickness_hunger_and_the_night_but_not_a_hospital_bed() {
        let mut raided = Situation {
            world: World {
                raided: true,
                ..raining()
            },
            ..hungry()
        };
        raided.needs.sick = true;
        assert_eq!(raided.first_rung(CitizenState::Idle), Rung::Raided);
        assert_eq!(
            raided.decide(CitizenState::Idle),
            Decision::with(CitizenState::Sleep, Status::Raided)
        );
        // In a hospital bed the citizen is SICK first: :154 is before :161.
        raided.needs.sleeps_at_hospital = true;
        assert_eq!(raided.first_rung(CitizenState::Idle), Rung::SickAtHospital);
    }

    #[test]
    fn the_night_beats_hunger_but_only_once_the_walk_home_is_due() {
        let at = [0, 64, 0];
        let mut situation = Situation {
            world: World::at_day_time(NIGHT - 1),
            home: Some(at),
            at,
            ..hungry()
        };
        assert_eq!(
            situation.first_rung(CitizenState::Idle),
            Rung::Eat,
            "601 ticks before NIGHT with no walk to make"
        );
        situation.world = World::at_day_time(NIGHT);
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::GoToSleep);
        assert_eq!(
            situation.decide(CitizenState::Idle),
            Decision::bare(CitizenState::Sleep),
            "GoToSleep sets no icon of its own (CitizenAI.java:181)"
        );
        // And a sleeper stays asleep: :171 comes before :211.
        assert_eq!(
            situation.first_rung(CitizenState::Sleep),
            Rung::KeepSleeping
        );
        // With no home, shouldGoSleep answers false and the meal wins.
        let homeless = Situation {
            world: World::at_day_time(NIGHT + 100),
            ..hungry()
        };
        assert_eq!(homeless.first_rung(CitizenState::Idle), Rung::Eat);
    }

    #[test]
    fn rain_only_idles_a_worker_that_may_not_work_in_it() {
        let idle = Situation {
            world: raining(),
            ..Situation::default()
        };
        assert_eq!(idle.first_rung(CitizenState::Idle), Rung::Rain);
        assert_eq!(
            idle.decide(CitizenState::Idle),
            Decision::with(CitizenState::Idle, Status::BadWeather)
        );
        // The same rain, a worker that may work in it, and a job to do.
        let sheltered = Situation {
            world: raining(),
            works_in_rain: true,
            ..Situation::worker([0, 64, 0])
        };
        assert_eq!(sheltered.first_rung(CitizenState::Idle), Rung::Work);
        assert_eq!(
            sheltered.decide(CitizenState::Idle),
            Decision::with(CitizenState::Work, Status::Working)
        );
        // In the Nether the rain rule does not exist at all (:232).
        let nether = Situation {
            world: World {
                nether: true,
                ..raining()
            },
            ..Situation::worker([0, 64, 0])
        };
        assert_eq!(nether.first_rung(CitizenState::Idle), Rung::Work);
    }

    #[test]
    fn a_worker_that_may_go_idle_or_has_leisure_time_idles_unless_frozen() {
        let at_home = Situation::worker([0, 64, 0]);
        assert_eq!(at_home.first_rung(CitizenState::Idle), Rung::Work);
        assert_eq!(
            Situation {
                can_go_idle: true,
                ..at_home
            }
            .first_rung(CitizenState::Idle),
            Rung::Idle
        );
        assert_eq!(
            Situation {
                leisure_time: 500,
                ..at_home
            }
            .first_rung(CitizenState::Idle),
            Rung::Idle
        );
        assert_eq!(
            Situation {
                leisure_time: 500,
                interruptible: false,
                ..at_home
            }
            .first_rung(CitizenState::Idle),
            Rung::Work,
            "!canAIBeInterrupted() makes the leisure clause irrelevant"
        );
    }

    #[test]
    fn a_guard_eats_sickens_or_works_and_never_anything_else() {
        assert_eq!(
            Situation::guard().decide(CitizenState::Idle),
            Decision::with(CitizenState::Work, Status::Working),
            "the guard branch returns WORK at :150"
        );
        let hungry_guard = Situation {
            needs: Needs {
                saturation: 1.0,
                ..Needs::new()
            },
            ..Situation::guard()
        };
        assert_eq!(hungry_guard.first_rung(CitizenState::Idle), Rung::GuardEats);

        let sick_guard = Situation {
            needs: Needs {
                sick: true,
                ..Needs::new()
            },
            interruptible: false,
            ..Situation::guard()
        };
        assert_eq!(sick_guard.first_rung(CitizenState::Idle), Rung::GuardWorks);
        assert_eq!(
            Situation {
                interruptible: true,
                ..sick_guard
            }
            .first_rung(CitizenState::Idle),
            Rung::GuardSick
        );

        // Midnight, raided, mourning — and still at work: no guard rung for any.
        let night_guard = Situation {
            world: World {
                raided: true,
                ..World::at_day_time(NIGHT)
            },
            needs: Needs {
                mourning: true,
                ..Needs::new()
            },
            ..Situation::guard()
        };
        assert_eq!(night_guard.first_rung(CitizenState::Idle), Rung::GuardWorks);
    }

    #[test]
    fn mourning_sits_between_food_and_the_rain() {
        let mut situation = Situation {
            world: raining(),
            needs: Needs {
                mourning: true,
                ..Needs::new()
            },
            ..Situation::default()
        };
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::Mourn);
        assert_eq!(
            situation.decide(CitizenState::Idle),
            Decision::with(CitizenState::Mourn, Status::Mourning)
        );
        situation.needs.saturation = 1.0;
        assert_eq!(
            situation.first_rung(CitizenState::Idle),
            Rung::Eat,
            ":211 before :217"
        );
    }

    #[test]
    fn a_pupil_child_idles_past_noon_only() {
        let mut situation = Situation {
            world: World::at_day_time(NOON + 1),
            ..Situation::worker([0, 64, 0])
        };
        situation.pupil_child = true;
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::PupilAfternoon);
        situation.world = World::at_day_time(NOON);
        assert_eq!(
            situation.first_rung(CitizenState::Idle),
            Rung::Work,
            "the comparison is strictly past noon (CitizenAI.java:244)"
        );
    }

    #[test]
    fn a_hurt_citizen_is_sick_unless_it_is_starving() {
        let mut situation = hungry();
        situation.needs.saturation = LOW_SATURATION + 1.0;
        situation.needs.health = 4.0;
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::Sick);
        // `isHurt` wants saturation above LOW_SATURATION
        // (CitizenDiseaseHandler.java:175-180): at 5 it is no longer hurt, and
        // under 6 with health under 6 it eats.
        situation.needs.saturation = LOW_SATURATION - 1.0;
        assert_eq!(situation.first_rung(CitizenState::Idle), Rung::Eat);
    }

    #[test]
    fn the_cooks_draw_only_suppresses_the_eat_rung() {
        let situation = Situation {
            cook_skips: true,
            ..hungry()
        };
        assert_eq!(
            situation.first_rung(CitizenState::Idle),
            Rung::Idle,
            "the cook waits instead (CitizenAI.java:285-288)"
        );
    }

    #[test]
    fn every_decision_is_a_transition_the_state_table_allows() {
        let situations = [
            Situation::default(),
            Situation::worker([0, 64, 0]),
            Situation {
                world: raining(),
                ..hungry()
            },
            Situation {
                world: World::at_day_time(NIGHT),
                home: Some([0, 64, 0]),
                ..Situation::default()
            },
            Situation::guard(),
        ];
        for from in CitizenState::ALL {
            for situation in situations {
                let decision = situation.decide(from);
                assert_eq!(
                    from.may_become(decision.state, situation.branch),
                    Ok(()),
                    "{from} -> {} is refused but the decision can pick it",
                    decision.state
                );
            }
        }
    }
}
