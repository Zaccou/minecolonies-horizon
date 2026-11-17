//! The state of one quest a colony holds: which objective is open, who is
//! carrying it, how long it has left, and what a save round-trips.
//!
//! Java reference: `core/quests/QuestInstance.java` (360 lines) — the fields
//! (`:26-64`), the constructor that reads the trigger returns (`:66-93`), the
//! acceptance reset (`:109-123`), validity (`:137-153`), deletion (`:161-183`),
//! `advanceObjective` (`:185-229`), completion (`:231-259`) and the compound
//! (`:261-314`) — plus the five keys it writes (`NbtTagConstants.java:741-747`).
//!
//! | Java | Here |
//! | --- | --- |
//! | the constructor (`:72-93`) | [`Instance::new`] |
//! | `isValid` (`:137-153`) | [`Instance::is_valid`] |
//! | `advanceObjective` (`:198-229`) | [`Instance::advance`] |
//! | `onCompletion` (`:231-259`) | [`Instance::complete`] |
//! | `onDeletion` (`:161-183`) | [`Instance::delete`] |
//! | `serializeNBT` / `deserializeNBT` (`:261-314`) | [`Instance::to_compound`], [`Instance::read`] |
//! | `getQuestTarget` (`:334-352`) | [`Instance::target`] |
//!
//! **Accepting is not what a file makes it look like.** `attemptAcceptQuest` has
//! exactly one caller in the whole mod: `QuestInstance.onStart` (`:121`), and
//! `onStart` is called from one place, `advanceObjective`'s `if (objectiveProgress == 0)`
//! (`:217-220`). So a quest sits in the colony's *available* list — with no
//! assigned player — until its **first objective is completed**; only then does it
//! become an in-progress quest and get a `UUID`. [`Step::Moved`] carries
//! `started: true` for exactly that transition, and the caller is the one that
//! tells the manager.
//!
//! **The first objective is started and its progress instance is dropped.**
//! `:92` calls `getObjective(0).startObjective(this)` and throws the answer away;
//! `currentObjectiveInstance` stays `null` until `advanceObjective` stores one for
//! the *next* objective (`:227-228`) or a load rebuilds it (`:303-308`). A
//! counting objective at index 0 is therefore registered as a listener and has
//! nowhere to count: the event handler hands its `onBlockBreak` a `null`
//! (`QuestObjectiveEventHandler.java:70-77`) and Java dereferences it at
//! `BreakBlockObjectiveTemplate.java:140`. [`Instance::progress`] is `None` in
//! that state, and [`super::objective::Seen::NoProgress`] is the honest answer.
//!
//! **`-1` completes; any other negative index throws.** `nextObjective == -1`
//! (`:211-215`) is the "the quest ends" answer every objective defaults to. A
//! `next-objective` of `-2` falls past that test, is stored as the index, and
//! `getObjective(-2)` throws at `:227`. [`Instance::advance`] refuses **before**
//! the first write, and says so, because Java has already applied the objective's
//! reward unlocks when it gets there (`:204-208`).
//!
//! **`isValid` is a conjunction of three things and nothing else**: a giver that
//! exists, every participant that exists, and `day - assignmentStart < timeout`
//! (`:152`, strictly less). It does not care whether the player is online, and a
//! quest with `timeout: 0` is invalid on the day it starts.

use mod_api::{Compound, Identifier, Tag};

use super::json::Refusal;
use super::objective::{Progress, Subject, Target};
use super::template::Template;
use super::trigger::{Citizen, Colony, Return};

/// The sentinel Java stores when no trigger named a quest giver
/// (`QuestInstance.java:39`, `Integer.MIN_VALUE`).
pub const NO_GIVER: i32 = i32::MIN;

/// `TAG_ID` (`NbtTagConstants.java:8`).
pub const KEY_ID: &str = "id";
/// `TAG_ASSIGN_START` (`:741`), the colony day the instance was born or accepted on.
pub const KEY_ASSIGN_START: &str = "assignstartquest";
/// `TAG_PROGRESS` (`:579`), which holds the **objective index**, not a counter.
pub const KEY_PROGRESS: &str = "progress";
/// `TAG_QUEST_GIVER` (`:742`).
pub const KEY_QUEST_GIVER: &str = "questgiver";
/// `TAG_PARTICIPANTS` (`:743`).
pub const KEY_PARTICIPANTS: &str = "questparticps";
/// `TAG_OBJECTIVE` (`:746`), the counter of the objective in progress.
pub const KEY_OBJECTIVE: &str = "objective";
/// `TAG_PLAYER` (`:747`), the accepting player.
pub const KEY_PLAYER: &str = "player";

/// `QuestInstance` without the colony: the same seven fields, with the counter as
/// a value instead of an object on the quest template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    template: Identifier,
    assignment_start: i32,
    objective: i32,
    giver: i32,
    participants: Vec<i32>,
    player: Option<[i32; 4]>,
    progress: Option<Progress>,
}

/// What one `advanceObjective` call did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advance {
    /// The reward indices the objective that was left behind unlocks
    /// (`:204-208`). Java applies them **before** anything else in the call, so a
    /// caller applies these first, then reads [`Step`].
    pub unlocks: Vec<usize>,
    pub step: Step,
}

/// The two outcomes of `advanceObjective`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: `:211-228` completes or moves
pub enum Step {
    /// The quest moved on. `started` is Java's `if (objectiveProgress == 0)`
    /// branch (`:217-220`): the timer was reset and the player stored, and the
    /// manager should now move the quest from available to in progress (`:121`).
    Moved {
        index: i32,
        started: bool,
        progress: Option<Progress>,
    },
    /// `next == -1` (`:211-215`), or the index ran past the last objective
    /// (`:222-226`).
    Completed,
}

/// Who to tell when a quest completes (`:232-259`): the giver and every
/// participant is asked `onQuestCompletion` (`CitizenData.java:2067-2079`), and
/// the accepting player — **only if they are online** (`:250-258`) — is sent the
/// quest's name. The key is Java's; rendering it is the caller's business.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub template: Identifier,
    /// The template's `name`, which Java passes to the completion message
    /// (`:256`).
    pub name: String,
    pub giver: i32,
    pub participants: Vec<i32>,
    pub player: Option<[i32; 4]>,
}

/// `COMPLETION_KEY`: `com.minecolonies.coremod.quest.completed` (`:256`).
pub const COMPLETION_KEY: &str = "com.minecolonies.coremod.quest.completed";

/// What a cancellation releases (`:161-183`): the citizens are told the quest is
/// gone, the current objective drops its listener, and the manager deletes the
/// quest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deletion {
    pub template: Identifier,
    pub giver: i32,
    pub participants: Vec<i32>,
    /// The event the open objective was listening for, if any — Java's
    /// `getObjective(objectiveProgress).onCancellation(this)` (`:177-181`).
    pub release: Option<Subject>,
}

impl Instance {
    /// The constructor (`:72-93`), minus the colony: the first citizen a trigger
    /// returned becomes the **giver**, every later one a **participant**, and a
    /// boolean return contributes neither (`:79-91`).
    ///
    /// A citizen returned twice is stored twice, which is Java's behaviour too —
    /// nothing deduplicates the list.
    pub fn new(template: Identifier, day: i32, returns: &[Return]) -> Instance {
        let mut giver = NO_GIVER;
        let mut participants = Vec::new();
        for answer in returns {
            let Some(citizen) = answer.citizen() else {
                continue;
            };
            if giver == NO_GIVER {
                giver = citizen;
            } else {
                participants.push(citizen);
            }
        }
        Instance {
            template,
            assignment_start: day,
            // `this.objectiveProgress = 0` (`:54`) and the instance starts there.
            objective: 0,
            giver,
            participants,
            player: None,
            // `:92` starts objective 0 and drops what it returns — see the module doc.
            progress: None,
        }
    }

    pub fn template(&self) -> &Identifier {
        &self.template
    }

    /// The index of the objective that is open (`getObjectiveIndex`, `:316-320`).
    pub fn objective(&self) -> i32 {
        self.objective
    }

    /// The counter of the open objective, when it has one (`:354-359`).
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    /// The quest giver's citizen id, or [`NO_GIVER`] (`:131-135`).
    pub fn giver(&self) -> i32 {
        self.giver
    }

    pub fn participants(&self) -> &[i32] {
        &self.participants
    }

    /// The player who accepted it, once any objective has been left (`:64`).
    pub fn player(&self) -> Option<[i32; 4]> {
        self.player
    }

    /// The day the timeout is measured from (`:49`).
    pub fn assignment_start(&self) -> i32 {
        self.assignment_start
    }

    /// `getParticipant(target)` (`:322-326`), without Java's index panic.
    pub fn citizen(&self, target: Target) -> Option<i32> {
        target.resolve(self.giver_opt(), &self.participants)
    }

    fn giver_opt(&self) -> Option<i32> {
        if self.giver == NO_GIVER {
            None
        } else {
            Some(self.giver)
        }
    }

    /// `isValid(colony)` (`:137-153`): a giver that exists, every participant that
    /// exists, and strictly less than `timeout` days since the instance began.
    pub fn is_valid(&self, colony: &dyn Colony, timeout: i32) -> bool {
        if self.giver == NO_GIVER {
            return false;
        }
        let citizens: &[Citizen] = colony.citizens();
        if !citizens.iter().any(|citizen| citizen.id == self.giver) {
            return false;
        }
        for participant in &self.participants {
            if !citizens.iter().any(|citizen| citizen.id == *participant) {
                return false;
            }
        }
        colony.day() - self.assignment_start < timeout
    }

    /// The citizen the open objective wants to talk to (`getQuestTarget`,
    /// `:334-352`): target `0` is the giver, a target past the participant list
    /// falls back to the giver, and so does an objective index that has no
    /// template.
    pub fn target(&self, template: &Template) -> i32 {
        let target = template
            .objective(self.objective)
            .map(|objective| objective.target());
        match target {
            None => self.giver,
            Some(Target::Giver) => self.giver,
            Some(Target::Participant(number)) => {
                match number
                    .checked_sub(1)
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| self.participants.get(index).copied())
                {
                    Some(citizen) => citizen,
                    None => self.giver,
                }
            }
        }
    }

    /// `advanceObjective(player, nextObjective)` (`:198-229`).
    ///
    /// The order is Java's and it is observable: the **unlocks of the objective
    /// being left** come out first (`:204-208`), then the completion test
    /// (`:211-215`), then the acceptance reset when the index was 0 (`:217-220`),
    /// then the new index and its counter (`:221-228`).
    pub fn advance(
        &mut self,
        next: i32,
        today: i32,
        player: Option<[i32; 4]>,
        template: &Template,
    ) -> Result<Advance, Refusal> {
        // Java would already have written twice by the time it throws at `:227`;
        // both checks below happen before the first write.
        let current = template
            .objective(self.objective)
            .ok_or_else(|| Refusal::NoSuchObjective {
                at: self.template.to_string(),
                index: self.objective,
            })?;
        let unlocks = current.unlocks().to_vec();

        if next == super::objective::FINAL {
            return Ok(Advance {
                unlocks,
                step: Step::Completed,
            });
        }
        if next < 0 {
            return Err(Refusal::WrongType {
                at: self.template.to_string(),
                key: "next-objective",
                expected: "a positive objective index or -1",
            });
        }

        let started = self.objective == 0;
        if started {
            // `onStart`: the timer restarts and the accepting player is stored
            // (`:116-123`). The manager's `attemptAcceptQuest` is the caller's.
            self.assignment_start = today;
            self.player = player;
        }
        self.objective = next;
        if next >= template.objective_count() {
            // `:222-226`: past the last objective is a completion, not an error.
            // Java leaves the index and the old counter in place — the instance is
            // only dropped by the manager's `completeQuest` (`:234`).
            return Ok(Advance {
                unlocks,
                step: Step::Completed,
            });
        }
        self.progress = template
            .objective(next)
            .and_then(|objective| objective.create_progress());
        Ok(Advance {
            unlocks,
            step: Step::Moved {
                index: next,
                started,
                progress: self.progress,
            },
        })
    }

    /// `onCompletion` (`:231-259`), as the list of people to tell.
    pub fn complete(&self, template: &Template) -> Completion {
        Completion {
            template: self.template.clone(),
            name: template.name().to_owned(),
            giver: self.giver,
            participants: self.participants.clone(),
            player: self.player,
        }
    }

    /// `onDeletion` (`:161-183`), as the list of people to tell and the listener
    /// to drop.
    pub fn delete(&self, template: &Template) -> Deletion {
        Deletion {
            template: self.template.clone(),
            giver: self.giver,
            participants: self.participants.clone(),
            release: template
                .objective(self.objective)
                .and_then(|objective| objective.listener()),
        }
    }

    /// `serializeNBT` (`:262-287`): five keys, the counter only when there is one,
    /// the player only once there is one.
    pub fn to_compound(&self) -> Compound {
        let mut compound = Compound::new();
        compound.insert(
            KEY_ID.into(),
            Tag::String(self.template.to_string().as_str().into()),
        );
        compound.insert(KEY_ASSIGN_START.into(), Tag::Int(self.assignment_start));
        compound.insert(KEY_PROGRESS.into(), Tag::Int(self.objective));
        compound.insert(KEY_QUEST_GIVER.into(), Tag::Int(self.giver));
        compound.insert(
            KEY_PARTICIPANTS.into(),
            Tag::List(self.participants.iter().map(|id| Tag::Int(*id)).collect()),
        );
        if let Some(progress) = self.progress {
            compound.insert(KEY_OBJECTIVE.into(), Tag::Compound(progress.to_compound()));
        }
        if let Some(player) = self.player {
            compound.insert(KEY_PLAYER.into(), Tag::IntArray(player.to_vec()));
        }
        compound
    }

    /// `deserializeNBT` (`:289-314`). Java needs `GLOBAL_SERVER_QUESTS` to rebuild
    /// the counter of the open objective (`:305`), which is why the template is a
    /// parameter here too.
    ///
    /// `getInt` on an absent key is `0`, so a compound with no `questgiver` reads
    /// giver `0` — not [`NO_GIVER`], and a colony with no citizen 0 answers
    /// "invalid" rather than "no giver" (`:140`).
    pub fn read(data: &Compound, template: &Template) -> Result<Instance, Refusal> {
        let at = template.id().to_string();
        let id = match data.get(&KEY_ID.into()) {
            Some(Tag::String(text)) => Identifier::parse(&text.to_string_lossy()).map_err(|_| {
                Refusal::BadIdentifier {
                    at: at.clone(),
                    key: KEY_ID,
                    text: text.to_string_lossy(),
                }
            })?,
            None => {
                return Err(Refusal::MissingKey {
                    at: at.clone(),
                    key: KEY_ID,
                });
            }
            Some(_) => {
                return Err(Refusal::WrongType {
                    at: at.clone(),
                    key: KEY_ID,
                    expected: "a string",
                });
            }
        };
        let objective = int_of(data, KEY_PROGRESS);
        let progress = match data.get(&KEY_OBJECTIVE.into()) {
            Some(Tag::Compound(counter)) => {
                // `getObjective(objectiveProgress)` throws past the end (`:305`).
                if template.objective(objective).is_none() {
                    return Err(Refusal::NoSuchObjective {
                        at: at.clone(),
                        index: objective,
                    });
                }
                Some(Progress::from_compound(counter))
            }
            _ => None,
        };
        let participants = match data.get(&KEY_PARTICIPANTS.into()) {
            Some(Tag::List(items)) => items
                .iter()
                .filter_map(|tag| match tag {
                    Tag::Int(value) => Some(*value),
                    Tag::Short(value) => Some(i32::from(*value)),
                    Tag::Byte(value) => Some(i32::from(*value)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let player = match data.get(&KEY_PLAYER.into()) {
            Some(Tag::IntArray(words)) if words.len() == 4 => {
                Some([words[0], words[1], words[2], words[3]])
            }
            Some(_) => {
                return Err(Refusal::WrongType {
                    at: at.clone(),
                    key: KEY_PLAYER,
                    expected: "four ints",
                });
            }
            None => None,
        };
        Ok(Instance {
            template: id,
            assignment_start: int_of(data, KEY_ASSIGN_START),
            objective,
            giver: int_of(data, KEY_QUEST_GIVER),
            participants,
            player,
            progress,
        })
    }
}

/// `CompoundTag.getInt`: every integer width, `0` when absent (`:293-295`).
fn int_of(data: &Compound, key: &str) -> i32 {
    match data.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        Some(Tag::Short(value)) => i32::from(*value),
        Some(Tag::Byte(value)) => i32::from(*value),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::objective::{Event, Seen, Subject};
    use super::super::trigger::Difficulty;
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    fn json(text: &str) -> mod_api::serde_json::Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    /// Three objectives: a dialogue (no counter, unlocks reward 0), a break-block
    /// (counter, goes to 2) and a place-block (counter, ends the quest).
    const FILE: &str = r#"{
      "name": "a quest",
      "parents": [],
      "triggers": [],
      "objectives": [
        {"type":"minecolonies:dialogue","target":0,"text":"hi","options":[],"unlocks-rewards":[0]},
        {"type":"minecolonies:breakblock","details":{"target":1,"block":"minecraft:dirt","qty":2,"next-objective":2}},
        {"type":"minecolonies:placeblock","details":{"target":0,"block":"minecraft:dirt","qty":1}}
      ],
      "rewards": [{"type":"minecolonies:item","details":{"item":"minecraft:stone","qty":1}}]
    }"#;

    fn template() -> Template {
        Template::from_json(id("minecolonies:test/quest"), &json(FILE)).expect("a valid file")
    }

    struct World {
        day: i32,
        citizens: Vec<Citizen>,
        tag: Compound,
    }

    impl World {
        fn with(ids: &[i32]) -> World {
            World {
                day: 12,
                citizens: ids
                    .iter()
                    .map(|id| Citizen {
                        id: *id,
                        on_quest: false,
                        data: Compound::new(),
                    })
                    .collect(),
                tag: Compound::new(),
            }
        }
    }

    impl Colony for World {
        fn day(&self) -> i32 {
            self.day
        }

        fn reputation(&self) -> f64 {
            0.0
        }

        fn difficulty(&self) -> Difficulty {
            Difficulty::Normal
        }

        fn is_unlocked(&self, _: &Identifier) -> bool {
            false
        }

        fn citizens(&self) -> &[Citizen] {
            &self.citizens
        }

        fn tag(&self) -> &Compound {
            &self.tag
        }

        fn random_below(&mut self, _: i32) -> i32 {
            0
        }
    }

    fn instance() -> Instance {
        Instance::new(
            id("minecolonies:test/quest"),
            12,
            &[Return::Citizen(Some(4)), Return::Citizen(Some(9))],
        )
    }

    #[test]
    fn the_first_citizen_a_trigger_names_is_the_giver_and_the_rest_are_participants() {
        // `:79-91`.
        let instance = Instance::new(
            id("minecolonies:test/quest"),
            7,
            &[
                Return::Matched(true),
                Return::Citizen(Some(4)),
                Return::Citizen(Some(9)),
                Return::Matched(true),
            ],
        );
        assert_eq!(instance.giver(), 4);
        assert_eq!(instance.participants(), &[9][..], "booleans are not people");
        assert_eq!(instance.assignment_start(), 7);
        assert_eq!(instance.objective(), 0);
        assert_eq!(
            instance.progress(),
            None,
            "the instance of objective 0 is started and dropped (`:92`)"
        );
        assert_eq!(instance.player(), None, "nobody has accepted yet");

        let nobody = Instance::new(id("minecolonies:test/quest"), 7, &[Return::Matched(true)]);
        assert_eq!(nobody.giver(), NO_GIVER);
    }

    #[test]
    fn the_timeout_is_strictly_less_than_the_templates_days() {
        // `colony.getDay() - assignmentStart < timeout` (`:152`).
        let instance = instance();
        let world = World::with(&[4, 9]);
        assert!(instance.is_valid(&world, 10), "day 12 is day 0 of the quest");
        let mut later = World::with(&[4, 9]);
        later.day = 21;
        assert!(instance.is_valid(&later, 10), "nine days later is still inside");
        later.day = 22;
        assert!(!instance.is_valid(&later, 10), "ten days is not");
        assert!(instance.is_valid(&later, 11), "eleven days is");
    }

    #[test]
    fn a_giver_or_a_participant_the_colony_lost_makes_the_quest_invalid() {
        // `:140-151`.
        let instance = instance();
        assert!(!instance.is_valid(&World::with(&[9]), 10), "no citizen 4");
        assert!(!instance.is_valid(&World::with(&[4]), 10), "no citizen 9");
        let no_giver = Instance::new(id("minecolonies:test/quest"), 12, &[]);
        assert_eq!(no_giver.giver(), NO_GIVER);
        assert!(
            !no_giver.is_valid(&World::with(&[4]), 10),
            "`questGiver == Integer.MIN_VALUE` (`:140`)"
        );
    }

    #[test]
    fn advancing_hands_back_the_unlocks_of_the_objective_left_behind() {
        // `:204-208`, before anything else in the call.
        let template = template();
        let mut instance = instance();
        let advance = instance
            .advance(1, 12, Some([1, 2, 3, 4]), &template)
            .expect("a valid move");
        assert_eq!(advance.unlocks, vec![0], "the dialogue unlocks reward 0");
        assert_eq!(
            advance.step,
            Step::Moved {
                index: 1,
                started: true,
                progress: Some(Progress::new())
            }
        );
        assert_eq!(instance.objective(), 1);
        assert_eq!(
            instance.progress().map(Progress::done),
            Some(0),
            "the break-block counter starts at zero"
        );
    }

    #[test]
    fn leaving_the_first_objective_is_what_starts_the_quest() {
        // `if (this.objectiveProgress == 0) { onStart(player, colony); }` (`:217-220`),
        // and `onStart` resets the day and stores the player (`:116-123`).
        let template = template();
        let mut instance = instance();
        assert_eq!(instance.assignment_start(), 12);
        instance
            .advance(1, 40, Some([7, 8, 9, 10]), &template)
            .expect("a valid move");
        assert_eq!(instance.assignment_start(), 40, "the timer restarted");
        assert_eq!(instance.player(), Some([7, 8, 9, 10]));
        // A second move does not touch either field.
        instance.advance(2, 99, Some([1, 1, 1, 1]), &template).expect("a valid move");
        assert_eq!(instance.assignment_start(), 40);
        assert_eq!(instance.player(), Some([7, 8, 9, 10]));
    }

    #[test]
    fn a_next_of_minus_one_and_an_index_past_the_last_objective_both_complete() {
        // `:211-215` and `:222-226`.
        let template = template();
        let mut first = instance();
        first.advance(1, 12, None, &template).expect("a valid move");
        let end = first.advance(-1, 12, None, &template).expect("a valid move");
        assert_eq!(end.step, Step::Completed);
        assert_eq!(
            first.objective(),
            1,
            "Java returns before touching the index (`:211-215`)"
        );
        assert!(first.progress().is_some(), "and before touching the counter");

        let mut past = instance();
        let beyond = past
            .advance(3, 12, None, &template)
            .expect("three is past the end");
        assert_eq!(beyond.step, Step::Completed);
        assert_eq!(beyond.unlocks, vec![0], "the unlocks still come out first");
        assert_eq!(
            past.objective(),
            3,
            "Java stores the index before it notices it ran past the end (`:221-226`)"
        );
    }

    #[test]
    fn a_negative_next_other_than_minus_one_is_refused_before_the_first_write() {
        // Java stores `-2`, then `getObjective(-2)` throws at `:227` — after it has
        // already applied the unlocks (`:204-208`). The port refuses first.
        let template = template();
        let mut instance = instance();
        let before = instance.clone();
        assert_eq!(
            instance.advance(-2, 40, Some([1, 2, 3, 4]), &template),
            Err(Refusal::WrongType {
                at: "minecolonies:test/quest".into(),
                key: "next-objective",
                expected: "a positive objective index or -1"
            })
        );
        assert_eq!(instance, before, "not one field changed");
    }

    #[test]
    fn the_counter_of_the_objective_in_progress_says_what_it_is_waiting_for() {
        let template = template();
        let mut instance = instance();
        instance.advance(1, 12, None, &template).expect("a valid move");
        let objective = template.objective(1).expect("the break-block objective");
        let mut progress = instance.progress().copied();
        let dirt = id("minecraft:dirt");
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&dirt)),
            Seen::Counting { done: 1, of: 2 }
        );
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&dirt)),
            Seen::Fulfilled
        );
        instance.advance(2, 12, None, &template).expect("a valid move");
        assert!(
            matches!(template.objective(2), Some(objective) if objective.create_progress().is_some()),
            "the place-block objective has a counter too"
        );
    }

    #[test]
    fn the_citizen_to_talk_to_falls_back_to_the_giver() {
        // `getQuestTarget` (`:334-352`).
        let template = template();
        let instance = instance();
        assert_eq!(instance.target(&template), 4, "objective 0 targets the giver");
        let mut second = instance.clone();
        second.advance(1, 12, None, &template).expect("a valid move");
        assert_eq!(second.target(&template), 9, "objective 1 targets participant 1");
        let mut missing = instance.clone();
        missing.participants.clear();
        missing.objective = 1;
        assert_eq!(
            missing.target(&template),
            4,
            "a target past the participant list is the giver"
        );
        missing.objective = 9;
        assert_eq!(missing.target(&template), 4, "and so is an index with no objective");
    }

    #[test]
    fn a_save_round_trips_the_seven_fields_and_writes_only_what_exists() {
        let template = template();
        let mut instance = instance();
        instance.advance(1, 33, Some([5, 6, 7, 8]), &template).expect("a valid move");
        let compound = instance.to_compound();
        assert_eq!(
            compound.get(&KEY_PROGRESS.into()),
            Some(&Tag::Int(1)),
            "`progress` is the objective index (`:267`)"
        );
        assert_eq!(compound.get(&KEY_ASSIGN_START.into()), Some(&Tag::Int(33)));
        assert_eq!(compound.get(&KEY_QUEST_GIVER.into()), Some(&Tag::Int(4)));
        assert_eq!(
            compound.get(&KEY_PARTICIPANTS.into()),
            Some(&Tag::List(vec![Tag::Int(9)]))
        );
        assert_eq!(
            compound.get(&KEY_PLAYER.into()),
            Some(&Tag::IntArray(vec![5, 6, 7, 8]))
        );
        assert_eq!(
            compound.get(&KEY_OBJECTIVE.into()),
            Some(&Tag::Compound(Progress::new().to_compound()))
        );
        assert_eq!(Instance::read(&compound, &template), Ok(instance));

        let fresh = instance_after_new();
        let plain = fresh.to_compound();
        assert!(
            !plain.contains_key(&KEY_OBJECTIVE.into()),
            "no counter, no key (`:276-279`)"
        );
        assert!(
            !plain.contains_key(&KEY_PLAYER.into()),
            "no player, no key (`:281-284`)"
        );
        assert_eq!(Instance::read(&plain, &template), Ok(fresh));
    }

    fn instance_after_new() -> Instance {
        Instance::new(id("minecolonies:test/quest"), 12, &[Return::Citizen(Some(4))])
    }

    #[test]
    fn a_save_naming_an_objective_the_file_does_not_have_is_refused() {
        let template = template();
        let mut compound = instance().to_compound();
        compound.insert(KEY_PROGRESS.into(), Tag::Int(9));
        compound.insert(
            KEY_OBJECTIVE.into(),
            Tag::Compound(Progress::new().to_compound()),
        );
        assert_eq!(
            Instance::read(&compound, &template),
            Err(Refusal::NoSuchObjective {
                at: "minecolonies:test/quest".into(),
                index: 9
            }),
            "Java throws here (`:305`)"
        );
    }

    #[test]
    fn a_quest_compound_without_an_id_or_with_a_bad_player_is_refused() {
        let template = template();
        let mut no_id = instance().to_compound();
        no_id.remove(&KEY_ID.into());
        assert_eq!(
            Instance::read(&no_id, &template),
            Err(Refusal::MissingKey {
                at: "minecolonies:test/quest".into(),
                key: KEY_ID
            })
        );
        let mut bad_player = instance().to_compound();
        bad_player.insert(KEY_PLAYER.into(), Tag::IntArray(vec![1, 2]));
        assert!(matches!(
            Instance::read(&bad_player, &template),
            Err(Refusal::WrongType { key: KEY_PLAYER, .. })
        ));
    }

    #[test]
    fn a_missing_giver_reads_as_citizen_zero_not_as_no_giver() {
        // `getInt(TAG_QUEST_GIVER)` answers 0 for an absent key (`:295`), and 0 is
        // a citizen id — a quest with no `questgiver` key is not a quest with no
        // giver, which is why `isValid` refuses it for the other reason (`:140`).
        let template = template();
        let mut compound = instance().to_compound();
        compound.remove(&KEY_QUEST_GIVER.into());
        let read = Instance::read(&compound, &template).expect("it reads");
        assert_eq!(read.giver(), 0);
        assert_ne!(read.giver(), NO_GIVER);
        assert!(
            read.is_valid(&World::with(&[0, 9]), 10),
            "citizen 0 is an ordinary id, so the quest is valid if the colony has one"
        );
    }

    #[test]
    fn a_deletion_names_the_listener_of_the_open_objective() {
        // `:177-181`.
        let template = template();
        let mut instance = instance();
        assert_eq!(instance.delete(&template).release, None, "a dialogue has no listener");
        instance.advance(1, 12, None, &template).expect("a valid move");
        let deletion = instance.delete(&template);
        assert_eq!(
            deletion.release,
            Some(Subject::BlockBroken(id("minecraft:dirt")))
        );
        assert_eq!(deletion.giver, 4);
        assert_eq!(deletion.participants, vec![9]);
    }

    #[test]
    fn a_completion_carries_everyone_to_tell_and_the_templates_name() {
        // `:232-259`.
        let template = template();
        let mut instance = instance();
        instance.advance(1, 12, Some([1, 2, 3, 4]), &template).expect("a valid move");
        let completion = instance.complete(&template);
        assert_eq!(completion.name, "a quest");
        assert_eq!(completion.template, id("minecolonies:test/quest"));
        assert_eq!(completion.giver, 4);
        assert_eq!(completion.participants, vec![9]);
        assert_eq!(completion.player, Some([1, 2, 3, 4]));
        assert_eq!(COMPLETION_KEY, "com.minecolonies.coremod.quest.completed");
    }
}
