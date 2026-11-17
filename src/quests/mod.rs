//! The quest data model: what a quest file declares, what one colony's copy of a
//! quest remembers, and what each objective, reward and side effect does.
//!
//! Java reference: `core/quests/` — 3 460 lines over 35 files, of which this
//! module ports the **model** and none of the game. The manager's cycle is
//! already [`crate::manager_09::quests`], the file reader is
//! `core/datalistener/QuestJsonListener.java` (its rules are here, its listener
//! is not: `src/data_listeners/` has no owner yet), and the dialogue screen and
//! the event handler stay with the modules that own a player and a level.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`template`] | what one file declares, which fields default, how its triggers decide | `QuestTemplate.java` 128, `QuestJsonListener.java:122-243` |
//! | [`instance`] | the state of one colony's quest: giver, participants, index, counter, timeout, save | `QuestInstance.java` 360 |
//! | [`objective`] | the seven objective types and the rule that says each is done | `objectives/*.java` 1 226 |
//! | [`reward`] | the eight rewards, what each changes, and the one side effect | `rewards/*.java` 506, `sideeffects/*.java` 141 |
//! | [`trigger`] | when a quest may start: six triggers, the `triggerOrder` expression, the NBT matcher | `triggers/*.java` 338, `IQuestTriggerTemplate.java:52-139` |
//! | [`json`] | the seven reads a file needs and the refusal each failure names | the parsers of all four |
//!
//! **Where this meets the manager.** [`crate::manager_09::quests`] keeps each
//! quest instance as an opaque [`mod_api::Compound`], which is Java's own
//! arrangement: `QuestManager` hands the compound through untouched
//! (`QuestManager.java:209-301`) and the instance owns its codec
//! (`QuestInstance.java:261-314`). [`Instance::to_compound`] writes that compound
//! and [`Instance::read`] reads it, so a colony book and this model are two views
//! of one save. The manager's `Template` (occurrences and parents) is the part of
//! [`template::Template`] its own tick needs; the two are filled from the same
//! file.
//!
//! **Six things the Java does that a brief would not guess, all ported.**
//!
//! 1. **Accepting happens late.** `attemptAcceptQuest` has one caller,
//!    `QuestInstance.onStart`, which runs only when the **first objective is
//!    left** (`:217-221`). Until then the quest is *available*, has no player, and
//!    every listener it registered was registered for `null`.
//! 2. **The first objective's counter is dropped.** `:92` starts objective 0 and
//!    discards the instance it returns, so a counting objective at index 0 has
//!    nowhere to count and Java dereferences the `null`
//!    (`BreakBlockObjectiveTemplate.java:140`). [`objective::Seen::NoProgress`]
//!    says so instead.
//! 3. **`!` in a `triggerOrder` never worked.** The constant, the node and the
//!    tokeniser exist (`QuestParsingConstants.java:26`,
//!    `QuestJsonListener.java:335-338`); `evaluate` has no case for it
//!    (`:401-452`).
//! 4. **A relationship reward can only ever uncouple.** The kind is read from the
//!    reward's own `type` instead of `details.type`
//!    (`RelationshipRewardTemplate.java:54`), so `"type": "couple"` in the shipped
//!    file is never seen.
//! 5. **Side effects are dead code.** The class, the interface and the three
//!    lifecycle methods exist; nothing constructs one, the `side-effects` parsing
//!    sits in a comment block and no file has the key.
//! 6. **A dialogue objective reads `target` outside `details`** while the other
//!    six read it inside (`DialogueObjectiveTemplateTemplate.java:81` against
//!    `BreakBlockObjectiveTemplate.java:79`).
//!
//! **No host call, no clock, no thread, no random of its own.** The colony is
//! [`trigger::Colony`], the player's pocket is [`objective::Hand`], the
//! partnerships are [`reward::Partners`], and everything else is a value: which
//! is what lets 45 tests here run with no game, no wasm and no prepared
//! resources. What is missing is named in [`MANQUANT.md`](MANQUANT): SNBT, the
//! UUID's byte order, the dialogue screen, and the file reader itself.

pub mod instance;
pub mod json;
pub mod objective;
pub mod reward;
pub mod template;
pub mod trigger;

pub use instance::{Advance, Completion, Deletion, Instance, Step, NO_GIVER};
pub use json::Refusal;
pub use objective::{
    Answer, Dialogue, Effect, Event, Hand, NbtMode, Objective, Progress, Seen, Subject, Target,
};
pub use reward::{Cast, Change, Outcome, Partners, Reward, WorkerIdle};
pub use template::Template;
pub use trigger::{Citizen, Colony, Difficulty, Node, Order, Return, Trigger};

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Compound, Identifier};

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    /// The two layers meet where the doc says they do: the model writes the
    /// compound and `crate::manager_09::quests` carries it untouched, which is
    /// exactly what `QuestManager.serializeNBT` does to a `QuestInstance`
    /// (`QuestManager.java:213-225`).
    #[test]
    fn the_managers_opaque_instance_is_this_models_compound() {
        let file = r#"{"name":"q","parents":[],"triggers":[],
          "objectives":[{"type":"minecolonies:breakblock","details":{"target":0,"block":"minecraft:dirt","qty":1}}],
          "rewards":[]}"#;
        let template = Template::from_json(
            id("minecolonies:test/quest"),
            &mod_api::serde_json::from_str(file).expect("the literal parses"),
        )
        .expect("a valid file");
        let instance = Instance::new(
            template.id().clone(),
            3,
            &[Return::Citizen(Some(4))],
        );
        let data = instance.to_compound();

        let carried = crate::manager_09::quests::Instance::new(data.clone());
        assert_eq!(carried.data(), &data);
        assert_eq!(carried.id(), Some(id("minecolonies:test/quest")));
        assert_eq!(Instance::read(carried.data(), &template), Ok(instance));
    }

    /// The strings a pack author writes, checked against the two constant files
    /// so a rename cannot drift.
    #[test]
    fn the_keys_are_the_javas_own_spelling() {
        assert_eq!(template::KEY_MAX_OCCURRENCES, "max-occurrences", "QuestParsingConstants.java:13");
        assert_eq!(template::KEY_TRIGGER_ORDER, "triggerOrder", "QuestParsingConstants.java:12");
        assert_eq!(template::DIRECTORY, "colony/quests", "DataGeneratorConstants.java:21");
        assert_eq!(instance::KEY_ASSIGN_START, "assignstartquest", "NbtTagConstants.java:741");
        assert_eq!(instance::KEY_QUEST_GIVER, "questgiver", "NbtTagConstants.java:742");
        assert_eq!(instance::KEY_PARTICIPANTS, "questparticps", "NbtTagConstants.java:743");
        assert_eq!(instance::KEY_OBJECTIVE, "objective", "NbtTagConstants.java:746");
        assert_eq!(instance::KEY_PLAYER, "player", "NbtTagConstants.java:747");
        assert_eq!(objective::KEY_QUANTITY, "quantity", "NbtTagConstants.java:588");
        assert_eq!(trigger::MAX_TICKRATE, 500, "TickRateConstants.java:11");
        assert_eq!(reward::WorkerIdle::ID, "minecolonies:workeridle", "WorkerIdleSideEffect.java:13");
    }

    #[test]
    fn a_refusal_always_carries_the_path_it_happened_at() {
        let refusal = Refusal::MissingKey {
            at: "objectives[2].details".into(),
            key: "qty",
        };
        assert_eq!(refusal.at(), "objectives[2].details");
        let empty = Compound::new();
        assert!(empty.is_empty());
    }
}
