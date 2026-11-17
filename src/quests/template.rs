//! The quest template: what one file under `data/<ns>/colony/quests/` declares,
//! which fields are required, which have a default, and how the file's triggers
//! decide whether the quest may start.
//!
//! Java reference: `core/quests/QuestTemplate.java` (128 lines), the parser
//! `core/datalistener/QuestJsonListener.loadDataFromJson` (`:122-243`), the key
//! names `core/quests/QuestParsingConstants.java` (`:8-19`) and
//! `api/quests/QuestParseConstant.java` (`:11-44`), and the
//! `QuestJsonListener.parseTriggerOrder` expression reader (`:246-391`).
//!
//! | Field | Key | Required | Default | Java |
//! | --- | --- | --- | --- | --- |
//! | name | `name` | yes | — | `:182` |
//! | parents | `parents` | yes (may be empty) | — | `:200` |
//! | triggers | `triggers` | yes (may be empty) | — | `:126` |
//! | trigger order | `triggerOrder` | no | `""` = all | `:156-160` |
//! | occurrences | `max-occurrences` | no | `1` | `:162-170` |
//! | timeout | `timeout` | no | `10` | `:172-180` |
//! | objectives | `objectives` | yes | — | `:142` |
//! | rewards | `rewards` | yes | — | `:185` |
//!
//! **Four fields are required that a schema would let you omit.** `name`,
//! `triggers`, `objectives`, `rewards` and `parents` are all read with a call that
//! throws on absence — `jsonObject.get(key).getAsJsonArray()` and
//! `Component.translatable(jsonObject.get(NAME).getAsString())`. A quest file
//! with no `rewards` key is refused, not a quest with no rewards; an empty array
//! is the way to say that.
//!
//! **A quest with no triggers never starts.** `attemptStart` answers `null` when
//! the returned list is *empty* (`QuestTemplate.java:75-76`), and the default
//! order with zero triggers produces exactly that empty list. This is the reason
//! `triggers` being required is harmless.
//!
//! **`max-occurrences: 0` means "never again"**, not "unlimited": the colony tick
//! skips a quest whose finished count is `>= maxOccurrence`
//! (`QuestManager.java:123`), and every quest starts at 0 finishes.
//!
//! **`parents` is about being *finished*, not unlocked** — `QuestManager`'s tick
//! looks each parent up in `finishedQuests` (`:129-136`), the map
//! [`super::instance`]'s caller fills on completion. That rule lives in the
//! manager (`src/manager_09/quests.rs`, already ported); this file only keeps the
//! list.
//!
//! **The order is checked against the number of triggers before anything runs**:
//! the distinct indices in the expression must be exactly as many as the file has
//! triggers (`QuestJsonListener.java:296-300`), which is why an order is not an
//! independent string but a value built with the trigger list in hand.

use mod_api::serde_json::Value;
use mod_api::Identifier;

use super::json::{self, Refusal};
use super::objective::Objective;
use super::reward::Reward;
use super::trigger::{Colony, Order, Return, Trigger};

/// The datapack directory a quest lives in — `COLONY_QUESTS_DIR`
/// (`core/generation/DataGeneratorConstants.java:21`), read by the reload
/// listener's constructor (`QuestJsonListener.java:50`).
pub const DIRECTORY: &str = "colony/quests";

/// `QuestParsingConstants.NAME` (`:16`).
pub const KEY_NAME: &str = "name";
/// `QuestParsingConstants.QUEST_PARENTS` (`:19`).
pub const KEY_PARENTS: &str = "parents";
/// `QuestParsingConstants.QUEST_TRIGGERS` (`:10`).
pub const KEY_TRIGGERS: &str = "triggers";
/// `QuestParsingConstants.TRIGGER_ORDER` (`:12`).
pub const KEY_TRIGGER_ORDER: &str = "triggerOrder";
/// `QuestParsingConstants.MAX_OCC` (`:13`).
pub const KEY_MAX_OCCURRENCES: &str = "max-occurrences";
/// `QuestParsingConstants.TIMEOUT` (`:15`).
pub const KEY_TIMEOUT: &str = "timeout";
/// `QuestParsingConstants.QUEST_OBJECTIVES` (`:11`).
pub const KEY_OBJECTIVES: &str = "objectives";
/// `QuestParsingConstants.QUEST_REWARDS` (`:9`).
pub const KEY_REWARDS: &str = "rewards";

/// `maxOccurrences = 1` when the key is absent (`QuestJsonListener.java:169`).
pub const DEFAULT_MAX_OCCURRENCES: i32 = 1;

/// `questTimeout = 10` when the key is absent (`:179`), in colony days
/// (`QuestInstance.java:152`).
pub const DEFAULT_TIMEOUT: i32 = 10;

/// One quest of the pack: everything a file says, with the two parts that are
/// themselves parsed (triggers and their order, objectives, rewards).
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    id: Identifier,
    name: String,
    parents: Vec<Identifier>,
    max_occurrences: i32,
    timeout: i32,
    order: Order,
    triggers: Vec<Trigger>,
    objectives: Vec<Objective>,
    rewards: Vec<Reward>,
}

impl Template {
    /// `loadDataFromJson(questId, jsonObject)` (`:122-243`).
    ///
    /// The order of the reads is Java's, because the second one that can fail
    /// changes nothing about the first: triggers (`:126-139`), objectives
    /// (`:142-154`), the three optional numbers (`:156-180`), the name (`:182`),
    /// rewards (`:185-197`) and parents (`:200-210`), then the order expression,
    /// which needs the trigger count (`:212`).
    pub fn from_json(id: Identifier, value: &Value) -> Result<Template, Refusal> {
        let map = json::object(value, &id.to_string())?;
        let at = id.to_string();

        let mut triggers = Vec::new();
        for (index, trigger) in json::array(map, &at, KEY_TRIGGERS)?.iter().enumerate() {
            triggers.push(Trigger::from_json(
                trigger,
                &json::element(&json::field(&at, KEY_TRIGGERS), index),
            )?);
        }

        let mut objectives = Vec::new();
        for (index, objective) in json::array(map, &at, KEY_OBJECTIVES)?.iter().enumerate() {
            objectives.push(Objective::from_json(
                objective,
                &json::element(&json::field(&at, KEY_OBJECTIVES), index),
            )?);
        }

        let mut rewards = Vec::new();
        for (index, reward) in json::array(map, &at, KEY_REWARDS)?.iter().enumerate() {
            rewards.push(Reward::from_json(
                reward,
                &json::element(&json::field(&at, KEY_REWARDS), index),
            )?);
        }

        let mut parents = Vec::new();
        for (index, parent) in json::array(map, &at, KEY_PARENTS)?.iter().enumerate() {
            parents.push(json::identifier_of(
                parent,
                &json::element(&json::field(&at, KEY_PARENTS), index),
            )?);
        }

        let name = json::text(map, &at, KEY_NAME)?.to_owned();
        let max_occurrences = match map.get(KEY_MAX_OCCURRENCES) {
            Some(_) => json::integer(map, &at, KEY_MAX_OCCURRENCES)?,
            None => DEFAULT_MAX_OCCURRENCES,
        };
        let timeout = match map.get(KEY_TIMEOUT) {
            Some(_) => json::integer(map, &at, KEY_TIMEOUT)?,
            None => DEFAULT_TIMEOUT,
        };
        let order_text = match map.get(KEY_TRIGGER_ORDER) {
            Some(_) => json::text(map, &at, KEY_TRIGGER_ORDER)?,
            None => "",
        };
        let order = Order::parse(order_text, triggers.len())?;

        Ok(Template {
            id,
            name,
            parents,
            max_occurrences,
            timeout,
            order,
            triggers,
            objectives,
            rewards,
        })
    }

    pub fn id(&self) -> &Identifier {
        &self.id
    }

    /// The template's `name`: a chat component key or a literal, used by the
    /// completion message (`QuestTemplate.java:118-121`, `QuestInstance.java:256`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The quests that must be **finished** first (`QuestTemplate.java:123-127`).
    pub fn parents(&self) -> &[Identifier] {
        &self.parents
    }

    /// `getMaxOccurrence()` (`:85-89`).
    pub fn max_occurrences(&self) -> i32 {
        self.max_occurrences
    }

    /// `getQuestTimeout()` (`:79-83`), in colony days.
    pub fn timeout(&self) -> i32 {
        self.timeout
    }

    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    pub fn objectives(&self) -> &[Objective] {
        &self.objectives
    }

    pub fn rewards(&self) -> &[Reward] {
        &self.rewards
    }

    /// `getObjective(index)` (`:105-109`). Java indexes and throws; an index the
    /// file does not have is `None` here.
    pub fn objective(&self, index: i32) -> Option<&Objective> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.objectives.get(index))
    }

    /// `getObjectiveCount()` (`:111-115`).
    pub fn objective_count(&self) -> i32 {
        self.objectives.len() as i32
    }

    /// `attemptStart(colony)` (`:72-77`): `None` unless the triggers answer with a
    /// **non-empty** list.
    ///
    /// The list it answers is what picks the quest giver and the participants, in
    /// its order — see [`super::instance::Instance::new`].
    pub fn attempt_start(&self, colony: &mut dyn Colony) -> Option<Vec<Return>> {
        match self.order.evaluate(&self.triggers, &self.id, colony) {
            Some(returns) if !returns.is_empty() => Some(returns),
            _ => None,
        }
    }

    /// `unlockQuestRewards(colony, player, quest, unlockedRewards)`
    /// (`QuestTemplate.java:91-103`): the rewards whose **index** is listed, in
    /// the file's order, each once — Java walks the reward array with a counter
    /// and asks `contains`, so the order of the indices in the objective does not
    /// matter and a repeated index applies one reward.
    pub fn unlocked_rewards(&self, indices: &[usize]) -> Vec<&Reward> {
        self.rewards
            .iter()
            .enumerate()
            .filter(|(index, _)| indices.contains(index))
            .map(|(_, reward)| reward)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::instance::Instance;
    use super::super::objective::Target;
    use super::super::trigger::{Citizen, Difficulty};
    use mod_api::{Compound, Tag};

    /// A colony whose only answer is a day: enough for the triggers that need
    /// nothing, and honest about the ones that do.
    struct Quiet {
        day: i32,
        reputation: f64,
        tag: Compound,
    }

    impl Quiet {
        fn new() -> Quiet {
            Quiet {
                day: 12,
                reputation: 0.0,
                tag: Compound::new(),
            }
        }
    }

    impl Colony for Quiet {
        fn day(&self) -> i32 {
            self.day
        }

        fn reputation(&self) -> f64 {
            self.reputation
        }

        fn difficulty(&self) -> Difficulty {
            Difficulty::Normal
        }

        fn is_unlocked(&self, _: &Identifier) -> bool {
            false
        }

        fn citizens(&self) -> &[Citizen] {
            &[]
        }

        fn tag(&self) -> &Compound {
            &self.tag
        }

        fn random_below(&mut self, _: i32) -> i32 {
            0
        }
    }

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    fn parse(text: &str) -> Result<Template, Refusal> {
        Template::from_json(id("minecolonies:test/quest"), &json(text))
    }

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    /// The smallest file Java accepts: every required key, nothing optional.
    const MINIMAL: &str = r#"{
      "name": "a quest",
      "parents": [],
      "triggers": [],
      "objectives": [{"type":"minecolonies:dialogue","target":0,"text":"hi","options":[]}],
      "rewards": []
    }"#;

    /// Two triggers that both pass (`state` on a compound the colony has) and one
    /// objective that needs nothing.
    const TWO_TRIGGERS: &str = r#"{
      "name": "a quest",
      "parents": ["minecolonies:test/parent"],
      "triggers": [
        {"type":"minecolonies:state","state":{"path":"flag","match":{"on":true}}},
        {"type":"minecolonies:state","state":{"path":"flag","match":{"on":true}}}
      ],
      "triggerOrder": "1 && 2",
      "timeout": 3,
      "objectives": [{"type":"minecolonies:dialogue","target":1,"text":"hi","options":[]}],
      "rewards": [
        {"type":"minecolonies:item","details":{"item":"minecraft:stone","qty":1}},
        {"type":"minecolonies:item","details":{"item":"minecraft:dirt","qty":2}}
      ]
    }"#;

    #[test]
    fn the_two_optional_numbers_default_to_one_occurrence_and_ten_days() {
        let template = parse(MINIMAL).expect("the minimal file is valid");
        assert_eq!(template.max_occurrences(), DEFAULT_MAX_OCCURRENCES);
        assert_eq!(template.max_occurrences(), 1, "QuestJsonListener.java:169");
        assert_eq!(template.timeout(), DEFAULT_TIMEOUT);
        assert_eq!(template.timeout(), 10, "QuestJsonListener.java:179");
        assert_eq!(template.name(), "a quest");
        assert_eq!(template.objective_count(), 1);
        assert!(template.rewards().is_empty());
        assert_eq!(template.parents().len(), 0);
    }

    #[test]
    fn the_five_required_keys_are_required_even_when_they_would_be_empty() {
        // `:182`, `:200`, `:126`, `:142`, `:185` — every one of them is a call
        // that throws on an absent key.
        for key in [KEY_NAME, KEY_PARENTS, KEY_TRIGGERS, KEY_OBJECTIVES, KEY_REWARDS] {
            let mut short = json(MINIMAL).as_object().expect("an object").clone();
            short.remove(key);
            let refused = Template::from_json(id("minecolonies:test/quest"), &Value::Object(short));
            assert_eq!(
                refused,
                Err(Refusal::MissingKey {
                    at: "minecolonies:test/quest".into(),
                    key
                }),
                "removing {key} must refuse"
            );
        }
    }

    #[test]
    fn a_quest_with_no_trigger_never_starts() {
        // `attemptStart` answers null for an *empty* list (`QuestTemplate.java:75-76`).
        let template = parse(MINIMAL).expect("the minimal file is valid");
        let mut colony = Quiet::new();
        assert_eq!(template.attempt_start(&mut colony), None);
    }

    #[test]
    fn a_positive_order_starts_the_quest_and_keeps_the_trigger_order() {
        let template = parse(TWO_TRIGGERS).expect("a valid file");
        let mut colony = Quiet::new();
        colony.tag.insert("flag".into(), Tag::Compound(Compound::from([("on".into(), Tag::Byte(1))])));
        let returns = template.attempt_start(&mut colony).expect("the quest starts");
        assert_eq!(
            returns,
            vec![
                Return::Matched(true),
                Return::Matched(true)
            ],
            "the returns come in the order the triggers are written"
        );

        colony.tag.clear();
        assert_eq!(
            template.attempt_start(&mut colony),
            None,
            "and a failing trigger stops it"
        );
    }

    #[test]
    fn the_order_is_checked_against_the_trigger_count() {
        // `:296-300`.
        let file = TWO_TRIGGERS.replace(r#""triggerOrder": "1 && 2","#, r#""triggerOrder": "1 || 1","#);
        assert_eq!(
            parse(&file),
            Err(Refusal::OrderMismatch {
                at: "1 || 1".into(),
                mentioned: 1,
                triggers: 2
            })
        );
    }

    #[test]
    fn an_unknown_trigger_names_its_index_in_the_file() {
        let file = r#"{"name":"q","parents":[],
          "triggers":[
            {"type":"minecolonies:state","state":{"path":"a","match":{}}},
            {"type":"minecolonies:telepathy"}],
          "objectives":[],"rewards":[]}"#;
        let refused = parse(file).expect_err("the second trigger is unknown");
        assert_eq!(refused.at(), "minecolonies:test/quest.triggers[1]");
        assert_eq!(
            refused,
            Refusal::UnknownType {
                at: "minecolonies:test/quest.triggers[1]".into(),
                kind: "trigger",
                text: "minecolonies:telepathy".into()
            }
        );
    }

    #[test]
    fn the_optional_fields_of_a_full_file_are_kept() {
        let template = parse(TWO_TRIGGERS).expect("a valid file");
        assert_eq!(template.timeout(), 3);
        assert_eq!(template.max_occurrences(), 1);
        assert_eq!(template.parents().len(), 1);
        assert_eq!(template.parents()[0], id("minecolonies:test/parent"));
        assert_eq!(template.rewards().len(), 2);
        assert_eq!(template.triggers().len(), 2);
        assert_eq!(
            template.objective(0).map(|objective| objective.target()),
            Some(Target::Participant(1))
        );
        assert!(template.objective(1).is_none(), "there is one objective");
        assert_eq!(template.objective_count(), 1);
    }

    #[test]
    fn the_reward_unlocks_come_out_in_the_files_order_and_only_once() {
        // `QuestTemplate.java:91-103`: a counter over the reward array and a
        // `contains` on the indices.
        let template = parse(TWO_TRIGGERS).expect("a valid file");
        let unlocked = template.unlocked_rewards(&[1, 0, 1]);
        assert_eq!(unlocked.len(), 2, "two rewards, each once");
        assert!(matches!(unlocked[0], Reward::Item { count: 1, .. }));
        assert!(matches!(unlocked[1], Reward::Item { count: 2, .. }));
        assert!(template.unlocked_rewards(&[]).is_empty());
        assert!(template.unlocked_rewards(&[7]).is_empty(), "no such index");
    }

    #[test]
    fn a_start_hands_the_instance_its_giver_and_participants() {
        let template = parse(TWO_TRIGGERS).expect("a valid file");
        let instance = Instance::new(
            template.id().clone(),
            12,
            &[Return::Citizen(Some(4)), Return::Citizen(Some(9))],
        );
        assert_eq!(instance.giver(), 4);
        assert_eq!(instance.participants(), &[9][..]);
        assert_eq!(instance.citizen(Target::Participant(1)), Some(9));
        assert_eq!(instance.citizen(Target::Giver), Some(4));
        assert_eq!(instance.objective(), 0);
    }

    #[test]
    fn a_name_and_a_directory_are_the_two_strings_a_pack_needs() {
        assert_eq!(DIRECTORY, "colony/quests", "DataGeneratorConstants.java:21");
        assert_eq!(DEFAULT_TIMEOUT, 10);
    }
}
