//! The teacher: check the school's paper, pick a seated pupil, walk to it, sit down
//! beside it, and give one lesson — a machine that spends most of its time *waiting*
//! on purpose.
//!
//! Java reference: `core/entity/ai/workers/education/EntityAIWorkTeacher.java`
//! (237 lines) — `registerTargets` (`:75-80`), `decide` (`:89-115`), `teach`
//! (`:117-176`), `teachPupil` (`:184-198`), `getActionsDoneUntilDumping`
//! (`:200-204`), `requestPaper` (`:209-216`) and `startWorkingAtOwnBuilding`
//! (`:229-236`) — on top of `core/entity/other/SittingEntity.java` (`sitDown`).
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:77` |
//! | 2 | `DECIDE` | `decide` | 20 | `:78` |
//! | 3 | `TEACH` | `teach` | 20 | `:79` |
//! | 4 | `IDLE` | → `START_WORKING` | 1 | `:76` |
//!
//! ## The lesson is counted in seconds, not ticks
//!
//! `teach` runs once per `TICKS_SECOND` (`:79`), so `sittingTicks` is a **second**
//! counter: `sittingTicks++` then `if (sittingTicks < maxSittingTicks) return
//! getState()` (`:139-143`). The `SittingEntity` it mounts is given
//! `maxSittingTicks * 20` **game** ticks (`:136`), which is the same duration — and if
//! the two ever disagreed the citizen would be dropped off its seat mid-lesson, so the
//! multiplication is part of the rule and is copied with its cite.
//!
//! `maxSittingTicks` itself (`:130-137`) is `nextInt(jobModifier / 2) + jobModifier / 2`
//! where `jobModifier = (int) (100 / max(1, level(Mana) / 2.0))`. Java's comment says
//! "2-100 seconds"; measured on the formula the real range is **1 to 99**: at Mana 0 the
//! modifier is 100 and the sitting time is 50..99, at Mana 99 the modifier is 2 and the
//! sitting time is 1. The comment is off by one at both ends and `RAPPORT.md` §4 says
//! so; the port copies the arithmetic, not the comment.
//!
//! **`nextInt(0)` throws, and the port cannot.** If `jobModifier` were 1 the
//! `jobModifier / 2` divisor would be zero and Java would raise
//! `IllegalArgumentException` inside `nextInt`. Reaching 1 needs `level(Mana)` above 99
//! (`100 / 50.5 = 1`, `100 / 50 = 2`), which the skill cap forbids, so the branch is
//! unreachable in Java — [`max_sitting_ticks`] answers `jobModifier` there instead of
//! dividing by zero, and a test pins it.
//!
//! ## What this file does not do
//!
//! `teachPupil` (`:184-198`) is written as a separate step because it writes to another
//! citizen: the decision answers [`Effect::Taught`] with the experience already
//! computed, and the caller applies it to the pupil's skill handler and to the
//! building's statistics (`LEVELS_GAINED`, `LESSONS_GIVEN`). `SittingEntity.sitDown` is
//! [`Effect::Sit`], for the same reason: it spawns an entity.

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};

use super::facts::{EduMemory, TeacherFacts};
use super::state::{EduRefusal, EduState};
use super::{Decision, Effect, TICKS_SECOND, at_hut, edu_action};

/// `EntityAIWorkTeacher.PAPER_TO_REQUEST` (`:39`): the `Stack` size `requestPaper`
/// asks for, and the `needsCurrently` amount when the paper is only in the school.
pub const PAPER_TO_REQUEST: u32 = 16;

/// `EntityAIWorkTeacher.getActionsDoneUntilDumping` (`:200-204`): fifty lessons before
/// the teacher empties its pockets, where the base class's threshold is 32
/// (`AbstractEntityAIBasic.java:425-428`).
pub const ACTIONS_UNTIL_DUMPING: u32 = 50;

/// `EntityAIWorkTeacher`'s own experience per lesson (`:169`):
/// `addExperience(0.1)`. The pupil's share is [`teach_xp`] and is much larger.
pub const TEACHER_XP: f64 = 0.1;

/// The `1.5` of the pupil's formula (`:163`).
pub const XP_BASE: f64 = 1.5;

/// The `/ 10.0` of the same formula, on Intelligence (`:163`) and on Knowledge
/// (`:165`). Both factors are `(1 + level / 10)`.
pub const SKILL_DIVISOR: f64 = 10.0;

/// The `100` of the sitting formula (`:133`).
pub const SIT_MODIFIER_BASE: f64 = 100.0;

/// The `* 20` of `SittingEntity.sitDown(..., maxSittingTicks * 20)` (`:136`): the seat's
/// lifetime is in game ticks, the counter is in seconds.
pub const SIT_TICK_FACTOR: u32 = 20;

/// One sheet per lesson: `transferXOfFirstSlotInItemHandlerWithIntoNextFreeSlotInItemHandler
/// (..., 1, ...)` (`:158`).
pub const PAPER_PER_LESSON: u32 = 1;

/// `EntityAIWorkTeacher.startWorkingAtOwnBuilding` (`:229-236`): get to the school,
/// then decide.
pub fn start_working(
    facts: &WorldFacts,
    trade: &TeacherFacts,
    memory: &mut EduMemory,
) -> Decision {
    let _ = (trade, memory);
    if !at_hut(facts) {
        // `if (!walkToBuilding()) return getState();` (`:231-235`).
        return Decision::refuse(EduRefusal::NotAtBuilding);
    }
    Decision::go(EduState::Base(WorkerState::Decide)) // `:235`
}

/// `EntityAIWorkTeacher.decide` (`:89-115`): paper first, pupils second, and the
/// `START_WORKING` loop when neither answers.
///
/// The paper branch does **not** return (`:93-102`): a school with no paper still looks
/// for a pupil and still teaches — it just has nothing to hand out. The fetch branch
/// does return, because the base class's `GATHERING_REQUIRED_MATERIALS` owns the walk.
pub fn decide(facts: &WorldFacts, trade: &TeacherFacts, memory: &mut EduMemory) -> Decision {
    let _ = facts;
    let mut decision = Decision::stay();

    // `:91-96` — `paperInBuilding + paperInInv <= 0` asks the colony for paper.
    if trade.paper_available() == 0 {
        if !trade.open_paper_request {
            // `requestPaper` (`:209-216`) creates at most one `Stack` request.
            decision = decision.effect(Effect::RequestPaper {
                count: PAPER_TO_REQUEST,
            });
        }
        decision = decision.because(EduRefusal::PaperMissing);
    }

    // `:98-102` — the paper is in the school's racks, so walk to it.
    if trade.paper_in_inventory == 0 && trade.paper_in_building > 0 {
        // `needsCurrently = new Tuple<>(PAPER, PAPER_TO_REQUEST);` (`:100`), which is
        // `AbstractEntityAIBasic.needsCurrently` (`:156`).
        memory.base.need = Some(PAPER_TO_REQUEST);
        return decision
            .going(EduState::Base(WorkerState::GatheringRequiredMaterials))
            .effect(Effect::FetchPaper {
                count: PAPER_TO_REQUEST,
            })
            .because(EduRefusal::PaperInHutOnly);
    }

    // `:104-112` — a seated pupil of the right job inside the school.
    if let Some(pupil) = trade.chosen_pupil() {
        memory.teacher.pupil_to_teach = Some(pupil.id);
        return decision.going(EduState::Teach);
    }

    // `:114` — nothing to teach: Java sends the teacher back to `START_WORKING`, which
    // walks to the hut again and comes straight back here.
    decision
        .going(EduState::Base(WorkerState::StartWorking))
        .because(EduRefusal::NoPupil)
}

/// `EntityAIWorkTeacher.teach` (`:117-176`): one second of one lesson.
///
/// The order is Java's and each step can end the tick: a pupil that vanished or stood
/// up (`:119-122`), a walk that has not arrived (`:125-128`), a seat that is not over
/// (`:139-143`), and only then the handout, the experience and the reset
/// (`:151-174`).
pub fn teach(facts: &WorldFacts, trade: &TeacherFacts, memory: &mut EduMemory) -> Decision {
    // `:119` — `pupilToTeach == null`.
    let Some(id) = memory.teacher.pupil_to_teach else {
        return Decision::go(EduState::Base(WorkerState::StartWorking))
            .because(EduRefusal::PupilNotChosen);
    };
    // `:119` — the entity is gone from the school.
    let Some(pupil) = trade.pupil(id) else {
        return Decision::go(EduState::Base(WorkerState::StartWorking))
            .because(EduRefusal::PupilNotChosen);
    };
    // `:119-122` — `pupilToTeach.vehicle == null`: it stood up.
    if !pupil.seated {
        return Decision::go(EduState::Base(WorkerState::StartWorking))
            .because(EduRefusal::PupilNotSeated);
    }

    // `:125-128` — still walking to the pupil.
    if !trade.at_pupil {
        return Decision::refuse(EduRefusal::NotAtPupil);
    }

    let mut decision = Decision::stay();

    // `:130-137` — sit down, once. Java re-seats when the vehicle is gone even though
    // the timer still runs, which is why `teacher_seated` is a fact.
    if memory.teacher.max_sitting_ticks == 0 || !trade.teacher_seated {
        let seconds = max_sitting_ticks(trade.mana_level, trade.sit_roll);
        memory.teacher.max_sitting_ticks = seconds;
        decision = decision.effect(Effect::Sit {
            at: facts.worker.at,
            ticks: seconds.max(0) as u32 * SIT_TICK_FACTOR,
        });
    }

    // `:139-143` — one second of the lesson.
    memory.teacher.sitting_ticks += 1;
    if memory.teacher.sitting_ticks < memory.teacher.max_sitting_ticks {
        return decision.because(EduRefusal::Sitting);
    }

    // `:151-160` — hand one sheet over, if the teacher has one and the pupil has none.
    if trade.paper_in_inventory == 0 {
        decision = decision.because(EduRefusal::NoPaper);
    } else if pupil.carries_paper {
        decision = decision.because(EduRefusal::PupilHasPaper);
    } else {
        decision = decision.effect(Effect::GivePaper {
            pupil: id,
            count: PAPER_PER_LESSON,
        });
    }

    // `:162-167` — the experience the pupil gains, then `teachPupil` (`:184-198`).
    let pupil_xp = teach_xp(
        trade.intelligence_level,
        trade.teaching_effect,
        trade.knowledge_level,
    );
    decision = decision.effect(Effect::Taught {
        pupil: id,
        pupil_xp,
        teacher_xp: TEACHER_XP,
    });

    // `:173-174` — the lesson is over, the counters go back to zero.
    memory.teacher.max_sitting_ticks = 0;
    memory.teacher.sitting_ticks = 0;
    decision.going(EduState::Base(WorkerState::StartWorking)) // `:175`
}

/// `(int) (100 / Math.max(1, getSecondarySkillLevel() / 2.0))` (`:133`).
///
/// The Java cast truncates towards zero and the port keeps that: `as i32` on a
/// non-negative `f64` truncates the same way. At Mana 0 the divisor is `max(1, 0) = 1`
/// and the modifier is 100; at Mana 99 it is `100 / 49.5 = 2.02 → 2`.
pub fn sitting_job_modifier(mana_level: i32) -> i32 {
    let half_level = f64::from(mana_level.max(0)) / 2.0;
    (SIT_MODIFIER_BASE / half_level.max(1.0)) as i32
}

/// `maxSittingTicks = worker.getRandom().nextInt(jobModifier / 2) + jobModifier / 2`
/// (`:134`), with the draw supplied by the caller.
///
/// `roll` stands for what `nextInt(jobModifier / 2)` answered, so the port takes it
/// modulo the half — `nextInt` cannot answer outside `0..half` — and a caller that
/// hands a larger number gets a legal lesson instead of an out-of-range one.
///
/// **The `jobModifier / 2 == 0` guard.** Java would call `nextInt(0)` and throw
/// `IllegalArgumentException` there. `jobModifier == 1` needs `level(Mana)` above 99
/// (`100 / 50.0 = 2`, `100 / 50.5 = 1.98 → 1`), which the skill cap forbids, so the
/// branch is unreachable from a legal level; [`sitting_job_modifier`] still answers `0`
/// for a corrupt level far above the cap, and the guard keeps the arithmetic total
/// instead of dividing by zero.
pub fn max_sitting_ticks(mana_level: i32, roll: u32) -> i32 {
    let modifier = sitting_job_modifier(mana_level);
    let half = modifier / 2;
    if half <= 0 {
        return modifier;
    }
    half + (roll % half as u32) as i32
}

/// The pupil's experience for one lesson (`:162-166`).
///
/// ```text
/// xp  = 1.5 * (1 + level(Intelligence) / 10)          // :163
/// xp *= 1 + effectStrength(TEACHING)                  // :164
/// xp *= 1 + getPrimarySkillLevel() / 10               // :165, Knowledge
/// ```
///
/// The three factors multiply, so a teacher of Knowledge 40 with the `teachingmultiplier`
/// research at 0.25 gives a pupil of Intelligence 12 `1.5 * 2.2 * 1.25 * 5 = 20.625`
/// experience. Negative levels are clamped to zero: Java would accept them and hand a
/// pupil a negative experience, which `addXpToSkill` then subtracts.
pub fn teach_xp(intelligence_level: i32, teaching_effect: f64, knowledge_level: i32) -> f64 {
    let intelligence = f64::from(intelligence_level.max(0));
    let knowledge = f64::from(knowledge_level.max(0));
    let mut xp = XP_BASE * (1.0 + intelligence / SKILL_DIVISOR);
    xp *= 1.0 + teaching_effect;
    xp *= 1.0 + knowledge / SKILL_DIVISOR;
    xp
}

edu_action!(start_working_action, Teacher, start_working);
edu_action!(decide_action, Teacher, decide);
edu_action!(teach_action, Teacher, teach);

/// Register the three targets of `EntityAIWorkTeacher.java:75-80`.
///
/// The `IDLE → START_WORKING` one (`:76`) is `super::register_handover`'s.
pub fn register(machine: &mut Machine<EduState, super::EduScene>) {
    // :77 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    super::add_target(
        machine,
        EduState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIWorkTeacher.java:77",
        TICKS_SECOND,
        start_working_action,
    );
    // :78 — `new AITarget(DECIDE, this::decide, TICKS_SECOND)`.
    super::add_target(
        machine,
        EduState::Base(WorkerState::Decide),
        "decide",
        "EntityAIWorkTeacher.java:78",
        TICKS_SECOND,
        decide_action,
    );
    // :79 — `new AITarget(TEACH, this::teach, TICKS_SECOND)`.
    super::add_target(
        machine,
        EduState::Teach,
        "teach",
        "EntityAIWorkTeacher.java:79",
        TICKS_SECOND,
        teach_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_work_06::facts::{EduScene, Pupil, TeacherMemory};
    use crate::ai_work_06::{Decision, Effect};

    fn facts_with(paper_school: u32, paper_pocket: u32, pupils: Vec<Pupil>) -> TeacherFacts {
        TeacherFacts {
            paper_in_building: paper_school,
            paper_in_inventory: paper_pocket,
            pupils,
            ..TeacherFacts::default()
        }
    }

    fn memory_with(pupil: Option<u32>) -> EduMemory {
        EduMemory {
            teacher: TeacherMemory {
                pupil_to_teach: pupil,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn a_teacher_still_walking_to_the_school_holds_start_working() {
        // `:231-235`.
        let mut facts = WorldFacts::new();
        facts.worker.at = Pos::new(30, 64, 30);
        let mut memory = EduMemory::default();
        let decision = start_working(&facts, &TeacherFacts::default(), &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(EduRefusal::NotAtBuilding));
    }

    #[test]
    fn an_arrived_teacher_decides() {
        // `:235`.
        let facts = WorldFacts::new();
        let mut memory = EduMemory::default();
        let decision = start_working(&facts, &TeacherFacts::default(), &mut memory);
        assert_eq!(decision.next, Some(EduState::Base(WorkerState::Decide)));
        assert_eq!(decision.refusal, None);
    }

    #[test]
    fn a_school_without_paper_asks_the_colony_once_and_still_looks_for_a_pupil() {
        // `:93-96` requests; `:104-112` still runs because the branch does not return.
        let at = Pos::new(3, 65, 3);
        let mut memory = EduMemory::default();
        let facts = facts_with(0, 0, vec![Pupil::seated(7, at)]);
        let decision = decide(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(EduState::Teach));
        assert_eq!(decision.refusal, Some(EduRefusal::PaperMissing));
        assert_eq!(
            decision.effects,
            vec![Effect::RequestPaper {
                count: PAPER_TO_REQUEST
            }]
        );
        assert_eq!(memory.teacher.pupil_to_teach, Some(7));

        // A request already on its way is not duplicated (`:211-212`).
        let mut memory = EduMemory::default();
        let asked = TeacherFacts {
            open_paper_request: true,
            ..facts.clone()
        };
        let again = decide(&WorldFacts::new(), &asked, &mut memory);
        assert_eq!(again.refusal, Some(EduRefusal::PaperMissing));
        assert!(again.effects.is_empty(), "no second request");
    }

    #[test]
    fn paper_that_is_only_in_the_school_sends_the_teacher_to_fetch_sixteen() {
        // `:98-102` — `needsCurrently = (PAPER, PAPER_TO_REQUEST)` and
        // `GATHERING_REQUIRED_MATERIALS`.
        let mut memory = EduMemory::default();
        let facts = facts_with(5, 0, vec![Pupil::seated(1, Pos::new(1, 65, 1))]);
        let decision = decide(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(
            decision.next,
            Some(EduState::Base(WorkerState::GatheringRequiredMaterials))
        );
        assert_eq!(decision.refusal, Some(EduRefusal::PaperInHutOnly));
        assert_eq!(memory.base.need, Some(PAPER_TO_REQUEST));
        assert_eq!(
            decision.effects,
            vec![Effect::FetchPaper {
                count: PAPER_TO_REQUEST
            }]
        );
        assert_eq!(memory.teacher.pupil_to_teach, None, "the fetch comes first");
    }

    #[test]
    fn paper_in_the_pocket_goes_straight_to_the_pupil() {
        // `:98` is false when `paperInInv > 0`, so the pupil branch runs.
        let mut memory = EduMemory::default();
        let facts = facts_with(5, 2, vec![Pupil::seated(4, Pos::new(1, 65, 1))]);
        let decision = decide(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, Some(EduState::Teach));
        assert_eq!(decision.refusal, None);
        assert!(decision.effects.is_empty(), "nothing to ask for");
    }

    #[test]
    fn a_school_with_no_seated_pupil_sends_the_teacher_back_to_the_hut() {
        // `:104-114` — an empty list and a list of standing children answer the same.
        let loose = vec![
            Pupil {
                seated: false,
                ..Pupil::seated(1, Pos::new(1, 65, 1))
            },
            Pupil {
                baby: false,
                ..Pupil::seated(2, Pos::new(2, 65, 2))
            },
        ];
        let mut memory = EduMemory::default();
        let facts = facts_with(4, 1, loose);
        let decision = decide(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(
            decision.next,
            Some(EduState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(EduRefusal::NoPupil));
        assert_eq!(memory.teacher.pupil_to_teach, None);
    }

    #[test]
    fn a_lesson_without_a_chosen_pupil_or_a_seated_one_stops_before_walking() {
        // `:119-122`.
        let facts = TeacherFacts::default();
        let mut nothing = EduMemory::default();
        let decision = teach(&WorldFacts::new(), &facts, &mut nothing);
        assert_eq!(
            decision.next,
            Some(EduState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(EduRefusal::PupilNotChosen));

        let at = Pos::new(1, 65, 1);
        let stood_up = TeacherFacts {
            pupils: vec![Pupil {
                seated: false,
                ..Pupil::seated(1, at)
            }],
            ..TeacherFacts::default()
        };
        let mut chosen = memory_with(Some(1));
        let decision = teach(&WorldFacts::new(), &stood_up, &mut chosen);
        assert_eq!(
            decision.next,
            Some(EduState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(EduRefusal::PupilNotSeated));
        assert!(decision.effects.is_empty(), "no lesson, no experience");
    }

    #[test]
    fn a_teacher_who_has_not_arrived_at_the_pupil_holds_teach() {
        // `:125-128`.
        let facts = TeacherFacts {
            pupils: vec![Pupil::seated(1, Pos::new(8, 65, 8))],
            at_pupil: false,
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(1));
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(EduRefusal::NotAtPupil));
        assert_eq!(memory.teacher.max_sitting_ticks, 0, "no seat booked yet");
    }

    #[test]
    fn the_first_tick_at_the_pupil_books_a_seat_for_the_whole_lesson() {
        // `:130-137` then `:139-143`: the seat is taken, the counter starts at one.
        let facts = TeacherFacts {
            pupils: vec![Pupil::seated(1, Pos::new(1, 65, 1))],
            paper_in_inventory: 1,
            mana_level: 0, // jobModifier 100, so 50..99 seconds
            sit_roll: 0,   // the low end: 50 seconds
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(1));
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(EduRefusal::Sitting));
        assert_eq!(memory.teacher.max_sitting_ticks, 50);
        assert_eq!(memory.teacher.sitting_ticks, 1);
        assert_eq!(
            decision.effects,
            vec![Effect::Sit {
                at: WorldFacts::new().worker.at,
                ticks: 50 * SIT_TICK_FACTOR,
            }]
        );
    }

    #[test]
    fn a_teacher_whose_seat_vanished_takes_a_new_one_mid_lesson() {
        // `:130` — `maxSittingTicks == 0 || worker.vehicle == null`: the second half is
        // why a lost seat restarts the sitting, and it does not restart the counter.
        let facts = TeacherFacts {
            pupils: vec![Pupil::seated(1, Pos::new(1, 65, 1))],
            paper_in_inventory: 1,
            teacher_seated: false,
            sit_roll: 0,
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(1));
        memory.teacher.max_sitting_ticks = 50;
        memory.teacher.sitting_ticks = 10;
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.refusal, Some(EduRefusal::Sitting));
        assert_eq!(memory.teacher.sitting_ticks, 11, "the lesson goes on");
        assert_eq!(
            decision.effects,
            vec![Effect::Sit {
                at: WorldFacts::new().worker.at,
                ticks: 50 * SIT_TICK_FACTOR,
            }],
            "a new seat, for the same length"
        );
    }

    #[test]
    fn the_last_second_hands_one_sheet_over_and_pays_both_experiences() {
        // `:151-174`.
        let at = Pos::new(1, 65, 1);
        let facts = TeacherFacts {
            pupils: vec![Pupil::seated(3, at)],
            paper_in_inventory: 2,
            intelligence_level: 12,
            knowledge_level: 40,
            teaching_effect: 0.25,
            sit_roll: 0,
            teacher_seated: true,
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(3));
        // One second from the end of a fifty-second lesson.
        memory.teacher.max_sitting_ticks = 50;
        memory.teacher.sitting_ticks = 49;
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(
            decision.next,
            Some(EduState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, None);
        assert_eq!(
            decision.effects,
            vec![
                Effect::GivePaper { pupil: 3, count: 1 },
                Effect::Taught {
                    pupil: 3,
                    pupil_xp: 20.625,
                    teacher_xp: TEACHER_XP,
                },
            ]
        );
        // The counters are back to zero (`:173-174`).
        assert_eq!(memory.teacher.max_sitting_ticks, 0);
        assert_eq!(memory.teacher.sitting_ticks, 0);
        assert_eq!(memory.teacher.pupil_to_teach, Some(3), "Java keeps the pupil");
    }

    #[test]
    fn a_lesson_without_paper_is_still_a_lesson() {
        // `:151-171` — the handout is skipped, the experience is not.
        let facts = TeacherFacts {
            pupils: vec![Pupil::seated(2, Pos::new(1, 65, 1))],
            paper_in_inventory: 0,
            sit_roll: 0,
            teacher_seated: true,
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(2));
        memory.teacher.max_sitting_ticks = 1;
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.refusal, Some(EduRefusal::NoPaper));
        assert!(EduRefusal::NoPaper.still_acts());
        assert!(
            decision
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Taught { .. })),
            "the pupil still learns"
        );
        assert!(
            !decision
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::GivePaper { .. })),
            "but nothing is handed over"
        );
    }

    #[test]
    fn a_pupil_that_already_holds_paper_gets_no_second_sheet() {
        // `:152-153` — the `&&` of the transfer condition.
        let facts = TeacherFacts {
            pupils: vec![Pupil {
                carries_paper: true,
                ..Pupil::seated(5, Pos::new(1, 65, 1))
            }],
            paper_in_inventory: 4,
            sit_roll: 0,
            teacher_seated: true,
            ..TeacherFacts::default()
        };
        let mut memory = memory_with(Some(5));
        memory.teacher.max_sitting_ticks = 1;
        let decision = teach(&WorldFacts::new(), &facts, &mut memory);
        assert_eq!(decision.refusal, Some(EduRefusal::PupilHasPaper));
        assert!(
            !decision
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::GivePaper { .. }))
        );
    }

    #[test]
    fn the_sitting_time_divides_a_hundred_by_half_the_mana_level() {
        // `:133-134`. Measured range is 1..99, not the "2-100" of Java's comment.
        assert_eq!(sitting_job_modifier(0), 100);
        assert_eq!(sitting_job_modifier(1), 100, "max(1, 0.5) = 1");
        assert_eq!(sitting_job_modifier(2), 100, "max(1, 1.0) = 1");
        assert_eq!(sitting_job_modifier(10), 20, "max(1, 5.0) = 5");
        assert_eq!(sitting_job_modifier(98), 2);
        assert_eq!(sitting_job_modifier(99), 2, "100 / 49.5 truncates to 2");
        assert_eq!(sitting_job_modifier(-3), 100, "a corrupt level clamps to 0");

        // The roll is the low half of the sum, so the answer spans half..modifier-1.
        assert_eq!(max_sitting_ticks(0, 0), 50);
        assert_eq!(max_sitting_ticks(0, 49), 99);
        assert_eq!(max_sitting_ticks(0, 60), 60, "nextInt cannot answer 60");
        assert_eq!(max_sitting_ticks(99, 0), 1);
        // The branch Java would throw on: `jobModifier` 1 needs Mana above 99, and the
        // guard answers the modifier instead of calling `nextInt(0)`.
        assert_eq!(sitting_job_modifier(10_000), 0, "truncated towards zero");
        assert_eq!(max_sitting_ticks(10_000, 0), 0);
    }

    #[test]
    fn the_pupil_experience_multiplies_three_factors() {
        // `:163-165`.
        assert_eq!(teach_xp(0, 0.0, 0), 1.5);
        assert_eq!(teach_xp(10, 0.0, 0), 3.0, "1.5 * (1 + 1)");
        assert_eq!(teach_xp(0, 1.0, 0), 3.0, "the research doubles it");
        assert_eq!(teach_xp(0, 0.0, 10), 3.0, "Knowledge 10 doubles it");
        assert_eq!(teach_xp(10, 1.0, 10), 12.0, "2 * 2 * 2 * 1.5");
        // A negative level would have been a negative experience in Java.
        assert_eq!(teach_xp(-5, 0.0, -5), 1.5);
    }

    #[test]
    fn the_registered_table_is_three_one_second_targets() {
        // `:77-79`.
        let mut machine: Machine<EduState, EduScene> =
            Machine::new(EduState::Base(WorkerState::Idle), 5);
        super::register(&mut machine);
        assert_eq!(machine.transitions().len(), 3);
        for transition in machine.transitions() {
            assert_eq!(transition.tick_rate, TICKS_SECOND, "{}", transition.rule);
            assert!(transition.java.starts_with("EntityAIWorkTeacher.java:"));
        }
        assert!(machine.can_leave(EduState::Base(WorkerState::Decide)));
        assert!(machine.can_leave(EduState::Teach));
    }

    #[test]
    fn the_decision_builder_keeps_the_first_reason_and_asks_for_nothing_else() {
        // The `because` of `Decision` is used by `decide` twice on the same path.
        let mut decision = Decision::stay().because(EduRefusal::PaperMissing);
        assert_eq!(decision.refusal, Some(EduRefusal::PaperMissing));
        decision = decision.because(EduRefusal::NoPupil);
        assert_eq!(
            decision.refusal,
            Some(EduRefusal::PaperMissing),
            "the actionable reason wins"
        );
    }
}
