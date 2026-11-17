//! The researcher: walk to a bookshelf of the university, study one pass at it, and
//! start over — the smallest machine of the education package.
//!
//! Java reference: `core/entity/ai/workers/education/EntityAIWorkResearcher.java`
//! (126 lines) — the constructor's `registerTargets` (`:49-53`), `study`
//! (`:68-105`), `startWorkingAtOwnBuilding` (`:112-125`) — on top of
//! `colony/jobs/JobResearch.java` (the mana) and
//! `colony/buildings/workerbuildings/BuildingUniversity.java` (`getRandomBookShelf`,
//! `:115-128`).
//!
//! ## The table this adds
//!
//! | # | State | Rule | Rate | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:51` |
//! | 2 | `STUDY` | `study` | 20 | `:52` |
//! | 3 | `IDLE` | → `START_WORKING` | 1 | `:50` |
//!
//! That is the whole trade: two methods and the hand-over from `IDLE`. There is no
//! tool, no request of its own, no structure — `AbstractEntityAIInteract` registers
//! **no** extra target (`:110-112`, "no new targets for now") and
//! `AbstractEntityAISkill` none either, so nothing is hidden between the lines.
//!
//! ## The two numbers a player will ask about
//!
//! **`STUDY_DELAY` = 1200** (`:24`) is the pause after every pass, in game ticks:
//! `setDelay(STUDY_DELAY)` (`:103`) and `waitingForSomething` subtracts the AI's tick
//! rate each tick (`AbstractEntityAIBasic.java:511-513`) — so one pass per minute.
//! The *same* constant is the walk budget (`:75`): `walkDelay` grows by `TICKS_SECOND`
//! per attempt (`:77`), which is 60 seconds of walking, and then the researcher studies
//! wherever it stands (`:80`).
//!
//! **Mana gates the research, not the work** (`:84`): with `currentMana == 0` the pass
//! still pays `XP_PER_STUDYPOS` (`:100`), turns its page (`:102`) and sets its delay —
//! it just advances nothing. And the only writer of the mana is
//! `JobResearch.processOfflineTime` (`JobResearch.java:77-93`), which runs when the
//! colony was **offline**. Nothing in the tree refills it while the colony is up: a
//! university on a server that never goes down researches exactly `currentMana` passes
//! and then stops for good. That is a balance bug in Java, not a port decision;
//! `RAPPORT.md` §4 states it with the measurement rather than inventing a refill here.
//!
//! ## What this file does not do
//!
//! It does not advance a research. `Progress::advance` (this crate, `src/research/
//! progress.rs:291-318`) is the one place that adds a progress point, and it takes an
//! id — so the decision answers *which* id (through [`Effect::AdvanceResearch`]) and
//! the caller calls it, with `concludes` saying whether this pass was the last one
//! (`:88-94`, `building.onSuccess`).

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};

use super::facts::{EduMemory, ResearcherFacts};
use super::state::{EduRefusal, EduState};
use super::{Decision, Effect, ResearchAdvance, TICKS_SECOND, at_hut, edu_action};

/// `EntityAIWorkResearcher.STUDY_DELAY` (`:24`): the delay after a study pass **and**
/// the walk budget, in game ticks — 60 seconds both ways.
pub const STUDY_DELAY: u32 = 1200;

/// `EntityAIWorkResearcher.XP_PER_STUDYPOS` (`:29`): the experience one pass pays,
/// `addExperience(XP_PER_STUDYPOS)` (`:100`), whatever the pass advanced.
pub const XP_PER_STUDYPOS: f64 = 2.0;

/// `JobResearch.MANA_PER_SECOND` (`JobResearch.java:23`): `(4 * 60 * 60) / 100` — the
/// seconds of research one Mana level is worth, so a level-99 researcher's tank is
/// `144 * 99 = 14 256 s`, just under four hours (`:20-23` says so in the comment).
pub const MANA_PER_SECOND: i64 = (4 * 60 * 60) / 100;

/// `TickRateConstants.MAX_TICKRATE` (`TickRateConstants.java:11`): the slow-world-tick
/// ceiling `JobResearch` divides by.
pub const MAX_TICKRATE: i64 = 500;

/// `JobResearch.MAX_TICKRATE_PER_SECOND` (`JobResearch.java:28`):
/// `MAX_TICKRATE / TICKS_SECOND` = 25. It is the unit `currentMana` is stored in.
pub const MAX_TICKRATE_PER_SECOND: i64 = MAX_TICKRATE / TICKS_SECOND as i64;

/// `EntityAIWorkResearcher.startWorkingAtOwnBuilding` (`:112-125`): get to the hut,
/// pick a bookshelf, and go study at it.
///
/// `walkToBuilding()` answered false is `return getState()` (`:114-117`), so the
/// refusal is [`EduRefusal::NotAtBuilding`] and the state does not move — the caller's
/// navigation walks this tick.
pub fn start_working(
    facts: &WorldFacts,
    trade: &ResearcherFacts,
    memory: &mut EduMemory,
) -> Decision {
    if !at_hut(facts) {
        // `if (!walkToBuilding()) return getState();` (`:114-117`).
        return Decision::refuse(EduRefusal::NotAtBuilding);
    }
    if memory.researcher.study_pos.is_none() {
        // `studyPos = building.getRandomBookShelf();` (`:119-122`).
        memory.researcher.study_pos = Some(trade.book_shelf);
    }
    Decision::go(EduState::Study) // `:124`
}

/// `EntityAIWorkResearcher.study` (`:68-105`): one study pass.
///
/// The walk comes first (`:70-80`), the research second (`:82-97`), and the bookkeeping
/// last (`:99-104`) — in that order, because a pass that cannot walk still researches,
/// and a pass that researches still pays its experience and sets its delay.
pub fn study(facts: &WorldFacts, trade: &ResearcherFacts, memory: &mut EduMemory) -> Decision {
    let _ = facts;

    if memory.researcher.study_pos.is_none() {
        // `if (studyPos == null) studyPos = building.getRandomBookShelf();` (`:70-73`).
        memory.researcher.study_pos = Some(trade.book_shelf);
    }

    if !trade.at_book_shelf && memory.researcher.walk_delay < STUDY_DELAY as i32 {
        // `walkDelay += TICKS_SECOND; return getState();` (`:75-79`).
        memory.researcher.walk_delay += TICKS_SECOND as i32;
        return Decision::refuse(EduRefusal::NotAtBookshelf);
    }

    // `:80` — `walkDelay = 0`, and the pass goes on *where the citizen stands*. Java
    // does not walk a second time; it stops asking. This is the branch that makes a
    // university without a bookshelf (its shelf is the hut's own position,
    // `BuildingUniversity.java:115-120`) still pay experience.
    let abandoned_walk = !trade.at_book_shelf;
    memory.researcher.walk_delay = 0;

    // `return START_WORKING;` (`:104`) — the state the pass ends in, whatever happened.
    let mut decision = Decision::go(EduState::Base(WorkerState::StartWorking));

    // `if (!inProgress.isEmpty() && job.getCurrentMana() > 0)` (`:84`).
    let mut advance = None;
    if trade.in_progress.is_empty() {
        decision = decision.because(EduRefusal::NoResearchInProgress);
    } else if memory.researcher.mana == 0 {
        decision = decision.because(EduRefusal::NoMana);
    } else if let Some(target) = trade.advance_target() {
        // `:86` picked it, `:88-94` researched it, `:96` paid for it.
        advance = Some(ResearchAdvance {
            id: target.id.clone(),
            concludes: target.next_concludes(),
        });
        memory.researcher.mana -= 1; // `job.reduceCurrentMana();` (`:96`)
    }

    // `:99-103` — the saturation, the experience, the sound and the delay. Only the two
    // numbers are the decision's business; the sound and the saturation are effects the
    // caller applies with the citizen in hand.
    memory.researcher.study_pos = None; // `:101`
    memory.base.delay = STUDY_DELAY as i32; // `setDelay(STUDY_DELAY);` (`:103`)

    if abandoned_walk {
        // Recorded last so a real reason (no research, no mana) wins over it.
        decision = decision.because(EduRefusal::WalkAbandoned);
    }
    decision.effect(Effect::Studied {
        xp: XP_PER_STUDYPOS,
        research: advance,
    })
}

/// The cap `JobResearch.processOfflineTime` compares against
/// (`JobResearch.java:83`): `MANA_PER_SECOND * level(Mana)` seconds.
pub fn mana_cap_seconds(mana_level: i32) -> i64 {
    MANA_PER_SECOND * i64::from(mana_level.max(0))
}

/// `JobResearch.processOfflineTime` (`JobResearch.java:77-93`), as a pure function.
///
/// Java, in its own order:
///
/// ```text
/// maxMana               = level(Mana)                                // :80
/// speed                 = level(Knowledge)                           // :81
/// currentMaxMana        = MANA_PER_SECOND * maxMana                  // :83
/// newMana               = (time / 100) * speed                       // :84
/// both                  = newMana + currentMana * 25                 // :86
/// currentMana           = min(both, currentMaxMana) / 25             // :87-89
/// ```
///
/// `time` is the offline time in **milliseconds** (`processOfflineTime(final long
/// time)`), and the `/ 100` is Java's own unit choice for it. The comparison mixes
/// seconds (`:83`) with the 25-tick unit (`:86`) — that is how Java wrote it, and the
/// port copies the arithmetic instead of tidying it, because tidying it would change
/// how much mana a returning player finds.
///
/// Negative levels clamp to `0` instead of producing a negative count: Java's
/// `Math.min` would happily build a negative and then store `/ 25` of it, and a
/// negative mana is not a state any caller should be able to reach through data.
pub fn mana_after_offline(
    current: u32,
    mana_level: i32,
    knowledge_level: i32,
    offline_ms: i64,
) -> u32 {
    let max_mana_seconds = mana_cap_seconds(mana_level); // :83
    let speed = i64::from(knowledge_level.max(0)); // :81
    let new_mana = (offline_ms.max(0) / 100) * speed; // :84
    let both = new_mana + i64::from(current) * MAX_TICKRATE_PER_SECOND; // :86
    let result = both.min(max_mana_seconds); // :87
    (result / MAX_TICKRATE_PER_SECOND).max(0) as u32 // :89
}

edu_action!(start_working_action, Researcher, start_working);
edu_action!(study_action, Researcher, study);

/// Register the three targets of `EntityAIWorkResearcher.java:49-53`.
///
/// The `IDLE → START_WORKING` one (`:50`) is `super::register_handover`'s: it is the
/// same line in both education constructors, and registering it twice would put two
/// copies in `IDLE`'s list and change which one wins.
pub fn register(machine: &mut Machine<EduState, super::EduScene>) {
    // :51 — `new AITarget(START_WORKING, this::startWorkingAtOwnBuilding, TICKS_SECOND)`.
    super::add_target(
        machine,
        EduState::Base(WorkerState::StartWorking),
        "startWorkingAtOwnBuilding",
        "EntityAIWorkResearcher.java:51",
        TICKS_SECOND,
        start_working_action,
    );
    // :52 — `new AITarget(STUDY, this::study, TICKS_SECOND)`.
    super::add_target(
        machine,
        EduState::Study,
        "study",
        "EntityAIWorkResearcher.java:52",
        TICKS_SECOND,
        study_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai::worker::Memory;
    use crate::ai_work_06::facts::{EduScene, InProgress, ResearcherMemory};

    fn in_progress(id: &str, progress: u32, total: u32) -> InProgress {
        InProgress::new(id, progress, total)
    }

    #[test]
    fn a_researcher_still_walking_to_its_hut_holds_start_working() {
        // `:114-117` — `walkToBuilding()` false is `return getState()`.
        // The hut is at the origin and the citizen is forty blocks away from it.
        let mut facts = WorldFacts::new();
        facts.worker.at = Pos::new(40, 64, 40);
        let mut memory = EduMemory::default();
        let decision = start_working(&facts, &ResearcherFacts::default(), &mut memory);
        assert_eq!(decision.next, None, "stays in START_WORKING");
        assert_eq!(decision.refusal, Some(EduRefusal::NotAtBuilding));
        assert!(memory.researcher.study_pos.is_none(), "no shelf picked yet");
    }

    #[test]
    fn an_arrived_researcher_picks_the_bookshelf_and_enters_study() {
        // `:119-124`.
        let facts = WorldFacts::new(); // at the hut by default
        let trade = ResearcherFacts {
            book_shelf: Pos::new(2, 65, 2),
            ..ResearcherFacts::default()
        };
        let mut memory = EduMemory::default();
        let decision = start_working(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(EduState::Study));
        assert_eq!(decision.refusal, None);
        assert_eq!(memory.researcher.study_pos, Some(Pos::new(2, 65, 2)));
        assert!(decision.effects.is_empty(), "the walk is the caller's");
    }

    #[test]
    fn a_walk_that_has_not_arrived_waits_one_second_a_tick_for_sixty_seconds() {
        // `:75-79` raises `walkDelay` by `TICKS_SECOND` per attempt while it is under
        // `STUDY_DELAY`; `:80` then studies where the citizen stands.
        let facts = WorldFacts::new();
        let trade = ResearcherFacts {
            at_book_shelf: false,
            ..ResearcherFacts::default()
        };
        let mut memory = EduMemory::default();

        let first = study(&facts, &trade, &mut memory);
        assert_eq!(first.next, None);
        assert_eq!(first.refusal, Some(EduRefusal::NotAtBookshelf));
        assert!(memory.researcher.study_pos.is_some(), "the shelf is remembered");
        assert_eq!(memory.researcher.walk_delay, 20);

        // 59 more seconds: still waiting, never past the budget.
        for _ in 1..(STUDY_DELAY / TICKS_SECOND) as i32 {
            let decision = study(&facts, &trade, &mut memory);
            assert_eq!(decision.refusal, Some(EduRefusal::NotAtBookshelf));
            assert_eq!(decision.next, None);
        }
        assert_eq!(memory.researcher.walk_delay, STUDY_DELAY as i32);

        // The 61st attempt: the budget is spent, the pass happens anyway — where the
        // citizen stands, without a second walk (`:80`).
        let gave_up = study(&facts, &trade, &mut memory);
        assert_eq!(gave_up.next, Some(EduState::Base(WorkerState::StartWorking)));
        assert_eq!(memory.researcher.walk_delay, 0, "`:80` resets it");
        assert_eq!(memory.base.delay, STUDY_DELAY as i32);
        // Nothing is running here, so `:84` has a reason of its own and it wins: the
        // abandoned walk is a degradation notice, not the cause of the empty pass.
        assert_eq!(gave_up.refusal, Some(EduRefusal::NoResearchInProgress));

        // With a research and mana, the abandoned walk is the only thing left to say.
        let mut paid = memory;
        paid.researcher.mana = 3;
        let working = ResearcherFacts {
            at_book_shelf: false,
            in_progress: vec![InProgress::new("harmony", 0, 10)],
            ..ResearcherFacts::default()
        };
        paid.researcher.walk_delay = STUDY_DELAY as i32;
        let abandoned = study(&facts, &working, &mut paid);
        assert_eq!(abandoned.refusal, Some(EduRefusal::WalkAbandoned));
        assert_eq!(paid.researcher.mana, 2, "the pass advanced one research");
    }

    #[test]
    fn a_pass_with_nothing_to_advance_still_pays_its_experience_and_its_delay() {
        // `:84` has two ways to advance nothing; `:99-103` runs either way.
        let facts = WorldFacts::new();
        let mut memory = EduMemory::default();
        let empty = study(&facts, &ResearcherFacts::default(), &mut memory);
        assert_eq!(empty.refusal, Some(EduRefusal::NoResearchInProgress));
        assert_eq!(
            empty.effects,
            vec![Effect::Studied {
                xp: XP_PER_STUDYPOS,
                research: None
            }]
        );
        assert_eq!(memory.base.delay, STUDY_DELAY as i32);
        assert!(memory.researcher.study_pos.is_none(), "`:101` forgets the shelf");

        // Mana at zero with a research running: the same, and the mana is not spent.
        let trade = ResearcherFacts {
            in_progress: vec![in_progress("harmony", 0, 10)],
            ..ResearcherFacts::default()
        };
        let mut memory = EduMemory::default();
        let dry = study(&facts, &trade, &mut memory);
        assert_eq!(dry.refusal, Some(EduRefusal::NoMana));
        assert_eq!(memory.researcher.mana, 0);
        assert_eq!(memory.base.delay, STUDY_DELAY as i32);
        assert!(matches!(
            dry.effects.as_slice(),
            [Effect::Studied { research: None, .. }]
        ));
    }

    #[test]
    fn a_pass_with_mana_advances_the_drawn_research_and_spends_exactly_one() {
        // `:86` draws, `:88-94` researches, `:96` pays one.
        let facts = WorldFacts::new();
        let trade = ResearcherFacts {
            in_progress: vec![
                in_progress("harmony", 3, 10),
                in_progress("sawmill", 9, 10),
            ],
            draw: 1,
            ..ResearcherFacts::default()
        };
        let mut memory = EduMemory {
            researcher: ResearcherMemory {
                mana: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let decision = study(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(EduState::Base(WorkerState::StartWorking)));
        assert_eq!(decision.refusal, None);
        assert_eq!(
            decision.effects,
            vec![Effect::Studied {
                xp: XP_PER_STUDYPOS,
                research: Some(ResearchAdvance {
                    id: "sawmill".to_string(),
                    concludes: true, // 9 + 1 >= 10
                }),
            }]
        );
        assert_eq!(memory.researcher.mana, 3, "one reduction per pass");
    }

    #[test]
    fn the_pass_before_the_last_one_does_not_conclude() {
        // `LocalResearch.research` compares `>=` (`LocalResearch.java:56-75`).
        let facts = WorldFacts::new();
        let trade = ResearcherFacts {
            in_progress: vec![in_progress("harmony", 3, 10)],
            ..ResearcherFacts::default()
        };
        let mut memory = EduMemory {
            researcher: ResearcherMemory {
                mana: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let decision = study(&facts, &trade, &mut memory);
        let advance = match decision.effects.as_slice() {
            [Effect::Studied { research: Some(advance), .. }] => advance.clone(),
            other => panic!("expected one advance, got {other:?}"),
        };
        assert_eq!(advance.id, "harmony");
        assert!(!advance.concludes);
        assert_eq!(memory.researcher.mana, 0, "the last unit is spent");
    }

    #[test]
    fn the_base_memory_is_untouched_except_for_the_delay() {
        // The researcher never calls `hasNotDelayed`; only `setDelay` (`:103`).
        let facts = WorldFacts::new();
        let trade = ResearcherFacts::default();
        let mut memory = EduMemory::default();
        let before = Memory::default();
        let _ = study(&facts, &trade, &mut memory);
        assert_eq!(memory.base.has_delayed, before.has_delayed);
        assert_eq!(memory.base.slot_at, before.slot_at);
        assert_eq!(memory.base.delay, STUDY_DELAY as i32);
    }

    #[test]
    fn mana_comes_back_only_from_offline_time_and_caps_at_the_level() {
        // `JobResearch.java:77-93`. Level 2 Mana is worth 288 seconds; the tank unit is
        // 25, so the cap is 288 / 25 = 11.
        assert_eq!(mana_cap_seconds(2), 288);
        assert_eq!(mana_cap_seconds(0), 0);
        assert_eq!(mana_cap_seconds(-5), 0, "a corrupt level does not go negative");

        // No offline time, no mana.
        assert_eq!(mana_after_offline(3, 2, 5, 0), 3);
        // An hour offline at Knowledge 5: (3_600_000 / 100) * 5 = 180_000, far past the
        // 288-second cap, so the cap wins.
        assert_eq!(mana_after_offline(0, 2, 5, 3_600_000), 11);
        // A tenth of a second is one tick of offline time: (100 / 100) * 5 = 5, plus the
        // three units already there (75), under the cap → (75 + 5) / 25 = 3.
        assert_eq!(mana_after_offline(3, 99, 5, 100), 3);
        // Knowledge 0 never fills anything.
        assert_eq!(mana_after_offline(2, 99, 0, 10_000_000), 2);
    }

    #[test]
    fn the_registered_table_is_two_one_second_targets() {
        // `:51`, `:52` — both at `TICKS_SECOND`; `IDLE`'s is the hand-over's.
        let mut machine: Machine<EduState, EduScene> =
            Machine::new(EduState::Base(WorkerState::Idle), 5);
        super::register(&mut machine);
        assert_eq!(machine.transitions().len(), 2);
        for transition in machine.transitions() {
            assert_eq!(transition.tick_rate, TICKS_SECOND, "{}", transition.rule);
            assert!(transition.java.starts_with("EntityAIWorkResearcher.java:"));
        }
        assert!(machine.can_leave(EduState::Study));
        assert!(machine.can_leave(EduState::Base(WorkerState::StartWorking)));
    }
}
