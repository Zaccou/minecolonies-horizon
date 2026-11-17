//! The eight quest rewards and the one quest side effect: what each one changes
//! when it is applied, said as a value the colony can act on.
//!
//! Java reference: `core/quests/rewards/` — `ItemRewardTemplate.java` (67),
//! `SkillRewardTemplate.java` (69), `HappinessRewardTemplate.java` (72),
//! `ResearchCompleteRewardTemplate.java` (49), `RaidAdjustmentRewardTemplate.java`
//! (47), `RelationshipRewardTemplate.java` (110), `UnlockQuestRewardTemplate.java`
//! (47), `QuestReputationRewardTemplate.java` (46) — and
//! `core/quests/sideeffects/` — `IQuestSideEffect.java` (48),
//! `ICitizenQuestSideEffect.java` (23), `WorkerIdleSideEffect.java` (70).
//!
//! **A reward is a function of the colony and the player, and the port returns
//! what it would do instead of doing it.** Every `applyReward` in Java reaches
//! straight into `IColony`, a `CitizenData` or the player's inventory; [`Outcome`]
//! is the same list of changes as data, so a reward can be tested without a world
//! and the caller decides when to write.
//!
//! **The happiness reward reuses the citizen module's modifier.** Java builds
//! `new ExpirationBasedHappinessModifier(QUEST, 2.0, new StaticHappinessSupplier(qty), days)`
//! (`:65`, `:69`) and `crate::citizens::happiness::Modifier::quest(amount, days)`
//! is that construction with its `HappinessRewardTemplate.java:65` citation
//! already on it. Nothing is re-encoded here.
//!
//! **Two rewards have a rule, the other six are a number.**
//! [`Reward::Relationship`] couples only when **both** citizens are free and
//! uncouples only when **both** point at each other
//! (`RelationshipRewardTemplate.java:85-102`); [`Reward::Raid`] *adds* its
//! `change`, which the shipped template sets to `-1`, so a negative reward is a
//! legal file (`:45`).
//!
//! **`couple` is unreachable in this snapshot, and that is Java's bug, not the
//! port's reading.** `RelationshipRewardTemplate.createReward` reads the
//! relationship kind from `jsonObject.get(TYPE_KEY)` (`:54`) — the **reward's own
//! type**, i.e. `"minecolonies:relationship"` — while the shipped file puts
//! `"type": "couple"` inside `details`
//! (`data/minecolonies/colony/quests/template/questtemplate.json`). The string is
//! therefore never `"couple"`, the `if` always takes its `else`, and a
//! relationship reward can only ever **remove** a partnership. The port keeps
//! Java's read exactly, so [`Reward::relationship`] answers `Couple` only for a
//! file that writes `couple` at the top level, and `MANQUANT.md` records the
//! one-line fix (`details.get(TYPE_KEY)`) that would make the shipped file mean
//! what it says.
//!
//! **The side effect subsystem is dead code in this snapshot, and the port says
//! so rather than pretending otherwise.** `IQuestSideEffect` has the three
//! lifecycle methods a file would want (`:16-29`); `WorkerIdleSideEffect` puts a
//! citizen on idle days for one day and clears it on either ending (`:36-69`);
//! and **nothing constructs one**: the `side-effects` parsing in
//! `QuestJsonListener.java:214-242` is inside a `/* … *​/` block, the line that
//! would hold the list is commented out (`QuestInstance.java:122`), and no quest
//! file in the pack has a `side-effects` key. [`WorkerIdle`] therefore ports the
//! four rules that exist, and `MANQUANT.md` records that no file can declare one.

use mod_api::serde_json::{Map, Value};
use mod_api::{Compound, Identifier};

use crate::citizens::happiness::Modifier;
use crate::inventory::ItemName;
use crate::jobs::Skill;

use super::json::{self, Refusal};
use super::objective::Target;

/// The eight reward type ids (`QuestRegistries.java:151-158`).
pub const ITEM: &str = "minecolonies:item";
pub const SKILL: &str = "minecolonies:skill";
pub const RESEARCH: &str = "minecolonies:research";
pub const RAID: &str = "minecolonies:raid";
pub const RELATIONSHIP: &str = "minecolonies:relationship";
pub const HAPPINESS: &str = "minecolonies:happiness";
pub const UNLOCK_QUEST: &str = "minecolonies:unlockquest";
pub const REPUTATION: &str = "minecolonies:questreputation";

/// The one relationship kind Java compares against
/// (`RelationshipRewardTemplate.java:85`).
pub const COUPLE: &str = "couple";

/// The quest giver and participants of one quest, in the order the triggers named
/// them (`QuestInstance.java:79-91`). `giver` is `None` for the sentinel Java
/// stores before any trigger names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cast<'a> {
    pub giver: Option<i32>,
    pub participants: &'a [i32],
}

impl Cast<'_> {
    /// The citizen a target names, or `None` when the colony no longer has one.
    pub fn citizen(&self, target: Target) -> Option<i32> {
        target.resolve(self.giver, self.participants)
    }
}

/// Who is partnered with whom — `ICitizenData.getPartner()`
/// (`RelationshipRewardTemplate.java:87`, `:95`).
pub trait Partners {
    fn partner_of(&self, citizen: i32) -> Option<i32>;
}

/// What one reward changes. One variant per `applyReward` body in Java, with the
/// numbers it computes.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the eight rewards and the one thing each changes
pub enum Outcome {
    /// `player.getInventory().add(item)` (`ItemRewardTemplate.java:63-66`). The
    /// tag is SNBT text, handed over as-is (see `MANQUANT.md`).
    GiveItem {
        item: ItemName,
        count: u32,
        nbt: Option<String>,
    },
    /// `getCitizenSkillHandler().incrementLevel(skill, qty)` (`SkillRewardTemplate.java:58-68`).
    IncrementSkill {
        citizen: i32,
        skill: Skill,
        levels: i32,
    },
    /// `getCitizenHappinessHandler().addModifier(…)` (`HappinessRewardTemplate.java:61-71`).
    Happiness { citizen: i32, modifier: Modifier },
    /// `getResearchTree().finishResearch(research)` and `markDirty`
    /// (`ResearchCompleteRewardTemplate.java:44-48`).
    FinishResearch { research: Identifier },
    /// `setNightsSinceLastRaid(getNightsSinceLastRaid() + qty)` (`RaidAdjustmentRewardTemplate.java:45`).
    NightsSinceLastRaid { change: i32 },
    /// `setPartner` on both (`RelationshipRewardTemplate.java:89-90`).
    Couple { first: i32, second: i32 },
    /// `setPartner(0)` on both (`:99-100`).
    Uncouple { first: i32, second: i32 },
    /// `getQuestManager().unlockQuest(questId)` (`UnlockQuestRewardTemplate.java:45`).
    UnlockQuest { quest: Identifier },
    /// `getQuestManager().alterReputation(quantity)` (`QuestReputationRewardTemplate.java:44`).
    AlterReputation { delta: f64 },
}

/// One reward of a quest file.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the eight registered ids (`ModQuestInitializer.java:59-66`)
pub enum Reward {
    /// `ItemRewardTemplate` (`:21-67`).
    Item {
        item: ItemName,
        count: u32,
        nbt: Option<String>,
    },
    /// `SkillRewardTemplate` (`:16-69`).
    Skill {
        target: Target,
        skill: Skill,
        levels: i32,
    },
    /// `HappinessRewardTemplate` (`:18-72`).
    Happiness {
        target: Target,
        amount: f64,
        days: i32,
    },
    /// `ResearchCompleteRewardTemplate` (`:16-49`).
    Research { research: Identifier },
    /// `RaidAdjustmentRewardTemplate` (`:15-47`).
    Raid { change: i32 },
    /// `RelationshipRewardTemplate` (`:17-110`). `relation` is Java's
    /// `jsonObject.get(TYPE_KEY)` — the reward's own type — see the module doc.
    Relationship {
        first: Target,
        second: Target,
        relation: String,
    },
    /// `UnlockQuestRewardTemplate` (`:15-47`).
    UnlockQuest { quest: Identifier },
    /// `QuestReputationRewardTemplate` (`:14-46`).
    Reputation { delta: f64 },
}

impl Reward {
    /// One element of the `rewards` array (`QuestJsonListener.java:184-197`).
    pub fn from_json(value: &Value, at: &str) -> Result<Reward, Refusal> {
        let map = json::object(value, at)?;
        let kind = json::text(map, at, "type")?;
        match kind {
            ITEM => {
                let (details, at) = details(map, at)?;
                // `item.setCount(quantity)` after the tag (`:45-60`).
                let count = u32::try_from(json::integer(details, &at, "qty")?).map_err(|_| {
                    Refusal::WrongType {
                        at: at.clone(),
                        key: "qty",
                        expected: "a non-negative count",
                    }
                })?;
                let item = json::item_name(details, &at, "item")?;
                let nbt = match details.get("nbt") {
                    Some(_) => Some(json::text(details, &at, "nbt")?.to_owned()),
                    None => None,
                };
                Ok(Reward::Item { item, count, nbt })
            }
            SKILL => {
                let (details, at) = details(map, at)?;
                let target = Target::from_number(json::integer(details, &at, "target")?);
                let levels = json::integer(details, &at, "qty")?;
                // `Skill.valueOf(text)` (`:53`): the constant's name, exact.
                let text = json::text(details, &at, "skill")?;
                let skill = Skill::ALL
                    .iter()
                    .copied()
                    .find(|skill| skill.name() == text)
                    .ok_or_else(|| Refusal::UnknownSkill {
                        at: at.clone(),
                        text: text.to_owned(),
                    })?;
                Ok(Reward::Skill {
                    target,
                    skill,
                    levels,
                })
            }
            HAPPINESS => {
                let (details, at) = details(map, at)?;
                Ok(Reward::Happiness {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    // `new StaticHappinessSupplier(qty)` (`:65`, `:69`).
                    amount: f64::from(json::integer(details, &at, "qty")?),
                    days: json::integer(details, &at, "days")?,
                })
            }
            RESEARCH => {
                let (details, at) = details(map, at)?;
                Ok(Reward::Research {
                    research: json::identifier(details, &at, "id")?,
                })
            }
            RAID => {
                let (details, at) = details(map, at)?;
                Ok(Reward::Raid {
                    change: json::integer(details, &at, "change")?,
                })
            }
            RELATIONSHIP => {
                // `jsonObject.get(TYPE_KEY)` (`:54`): the outer object, read
                // before `at` becomes the path inside `details`.
                let relation = json::text(map, at, "type")?.to_owned();
                let (details, at) = details(map, at)?;
                Ok(Reward::Relationship {
                    first: Target::from_number(json::integer(details, &at, "target1")?),
                    second: Target::from_number(json::integer(details, &at, "target2")?),
                    relation,
                })
            }
            UNLOCK_QUEST => {
                let (details, at) = details(map, at)?;
                Ok(Reward::UnlockQuest {
                    quest: json::identifier(details, &at, "id")?,
                })
            }
            REPUTATION => {
                let (details, at) = details(map, at)?;
                Ok(Reward::Reputation {
                    delta: json::number(details, &at, "qty")?,
                })
            }
            other => Err(Refusal::UnknownType {
                at: at.to_owned(),
                kind: "reward",
                text: other.to_owned(),
            }),
        }
    }

    /// `applyReward(colony, player, colonyQuest)`, answered as a value. `None`
    /// when the reward has nobody to act on: Java casts a missing citizen
    /// unchecked in three of the eight (`SkillRewardTemplate.java:62`,
    /// `:66`, `HappinessRewardTemplate.java:65`, `:69`) and catches around the
    /// relationship one (`RelationshipRewardTemplate.java:105-108`).
    pub fn apply(&self, cast: &Cast<'_>, partners: &dyn Partners) -> Option<Outcome> {
        match self {
            Reward::Item { item, count, nbt } => Some(Outcome::GiveItem {
                item: item.clone(),
                count: *count,
                nbt: nbt.clone(),
            }),
            Reward::Skill {
                target,
                skill,
                levels,
            } => Some(Outcome::IncrementSkill {
                citizen: cast.citizen(*target)?,
                skill: *skill,
                levels: *levels,
            }),
            Reward::Happiness {
                target,
                amount,
                days,
            } => Some(Outcome::Happiness {
                citizen: cast.citizen(*target)?,
                // `ExpirationBasedHappinessModifier(QUEST, 2.0, StaticHappinessSupplier(qty), days)`
                // (`:65`) — the weight and the expiry are the modifier's.
                modifier: Modifier::quest(*amount, *days),
            }),
            Reward::Research { research } => Some(Outcome::FinishResearch {
                research: research.clone(),
            }),
            Reward::Raid { change } => Some(Outcome::NightsSinceLastRaid { change: *change }),
            Reward::Relationship {
                first,
                second,
                relation,
            } => {
                let first_citizen = cast.citizen(*first)?;
                let second_citizen = cast.citizen(*second)?;
                // `if (type.equals("couple"))` (`:85`), then the two `if`s that
                // decide whether anything happens (`:87-102`).
                if relation == COUPLE {
                    if partners.partner_of(first_citizen).is_none()
                        && partners.partner_of(second_citizen).is_none()
                    {
                        return Some(Outcome::Couple {
                            first: first_citizen,
                            second: second_citizen,
                        });
                    }
                    return None;
                }
                if partners.partner_of(first_citizen) == Some(second_citizen)
                    && partners.partner_of(second_citizen) == Some(first_citizen)
                {
                    return Some(Outcome::Uncouple {
                        first: first_citizen,
                        second: second_citizen,
                    });
                }
                None
            }
            Reward::UnlockQuest { quest } => Some(Outcome::UnlockQuest {
                quest: quest.clone(),
            }),
            Reward::Reputation { delta } => Some(Outcome::AlterReputation { delta: *delta }),
        }
    }
}

/// The one quest side effect the Java ships (`WorkerIdleSideEffect.java:11-70`),
/// with the four rules it actually implements.
///
/// - `onStart` does **nothing**: the only line in it is commented out (`:36-40`).
/// - `applyToCitizen` records the citizen and sets its idle days to **1**
///   (`:60-69`).
/// - `onFinish` and `onCancel` set them back to **0** (`:42-52`), and they do it
///   unconditionally — Java dereferences `citizenData` without checking it, so an
///   effect that was never applied throws here. [`WorkerIdle::on_finish`] answers
///   `None` instead, and the caller has nothing to clear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerIdle {
    applied_to: Option<i32>,
}

/// What a side effect asks for: Java's one mutable field, `setIdleDays`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub citizen: i32,
    pub days: i32,
}

impl WorkerIdle {
    /// `WorkerIdleSideEffect.ID` (`:13`).
    pub const ID: &'static str = "minecolonies:workeridle";

    /// The one day Java sets (`:67`).
    pub const IDLE_DAYS: i32 = 1;

    /// A fresh effect: nothing applied yet.
    pub fn new() -> WorkerIdle {
        WorkerIdle { applied_to: None }
    }

    /// The citizen this was applied to, if any.
    pub fn applied_to(&self) -> Option<i32> {
        self.applied_to
    }

    /// `onStart()` (`:36-40`): the apply call is commented out, so the effect
    /// changes nothing when the quest starts.
    pub fn on_start(&self) -> Option<Change> {
        None
    }

    /// `applyToCitizen(data)` (`:60-69`).
    pub fn apply_to(&mut self, citizen: i32) -> Change {
        self.applied_to = Some(citizen);
        Change {
            citizen,
            days: WorkerIdle::IDLE_DAYS,
        }
    }

    /// `onFinish()` (`:42-46`).
    pub fn on_finish(&mut self) -> Option<Change> {
        self.clear()
    }

    /// `onCancel()` (`:48-52`).
    pub fn on_cancel(&mut self) -> Option<Change> {
        self.clear()
    }

    fn clear(&mut self) -> Option<Change> {
        let citizen = self.applied_to.take()?;
        Some(Change { citizen, days: 0 })
    }

    /// `serializeNBT()`: `IQuestSideEffect`'s default, an **empty** compound —
    /// `WorkerIdleSideEffect` does not override it (`IQuestSideEffect.java:40-47`).
    /// The citizen it was applied to is therefore not saved, which is why a
    /// reloaded effect has forgotten it.
    pub fn to_compound(self) -> Compound {
        Compound::new()
    }
}

/// The `details` block a reward's numbers live in, and the path of every read
/// inside it.
fn details<'a>(
    map: &'a Map<String, Value>,
    at: &str,
) -> Result<(&'a Map<String, Value>, String), Refusal> {
    let details_at = json::field(at, "details");
    let value = map.get("details").ok_or_else(|| Refusal::MissingKey {
        at: at.to_owned(),
        key: "details",
    })?;
    Ok((json::object(value, &details_at)?, details_at))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizens::happiness::Source;

    /// The eight rewards of `data/minecolonies/colony/quests/template/questtemplate.json`
    /// (`:497-560` of the file), copied as they are written.
    const TEMPLATE: &str = r#"[
      {"type":"minecolonies:item","details":{"item":"minecraft:gold_ingot","qty":1}},
      {"type":"minecolonies:skill","details":{"target":0,"skill":"Strength","qty":1}},
      {"type":"minecolonies:research","details":{"id":"this"}},
      {"type":"minecolonies:raid","details":{"change":-1}},
      {"type":"minecolonies:relationship","details":{"target1":0,"target2":1,"type":"couple"}},
      {"type":"minecolonies:happiness","details":{"target":0,"qty":1,"days":7}},
      {"type":"minecolonies:questreputation","details":{"qty":0.5}},
      {"type":"minecolonies:unlockquest","details":{"id":"minecolonies:questtemplate"}}
    ]"#;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    fn rewards() -> Vec<Reward> {
        let list = json(TEMPLATE);
        list.as_array()
            .expect("an array")
            .iter()
            .enumerate()
            .map(|(index, value)| {
                Reward::from_json(value, &format!("rewards[{index}]")).expect("a valid reward")
            })
            .collect()
    }

    /// A colony where the listed pairs are the only partnerships.
    struct Couples(Vec<(i32, i32)>);

    impl Partners for Couples {
        fn partner_of(&self, citizen: i32) -> Option<i32> {
            self.0
                .iter()
                .find(|(first, _)| *first == citizen)
                .map(|(_, second)| *second)
        }
    }

    /// The giver is 3; the participants are 4 and 5.
    fn cast() -> Cast<'static> {
        Cast {
            giver: Some(3),
            participants: &[4, 5],
        }
    }

    #[test]
    fn the_eight_rewards_of_the_shipped_template_parse_into_the_eight_variants() {
        let parsed = rewards();
        assert_eq!(parsed.len(), 8);
        assert!(matches!(
            parsed[0],
            Reward::Item { count: 1, ref nbt, .. } if nbt.is_none()
        ));
        assert!(matches!(
            parsed[1],
            Reward::Skill {
                target: Target::Giver,
                skill: Skill::Strength,
                levels: 1
            }
        ));
        assert!(matches!(parsed[2], Reward::Research { .. }));
        assert!(matches!(parsed[3], Reward::Raid { change: -1 }));
        assert!(matches!(parsed[4], Reward::Relationship { .. }));
        assert!(matches!(
            parsed[5],
            Reward::Happiness {
                amount,
                days: 7,
                ..
            } if amount == 1.0
        ));
        assert!(matches!(parsed[6], Reward::Reputation { delta } if delta == 0.5));
        assert!(matches!(parsed[7], Reward::UnlockQuest { .. }));
    }

    #[test]
    fn the_number_rewards_change_exactly_what_they_say() {
        let parsed = rewards();
        assert_eq!(
            parsed[6].apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::AlterReputation { delta: 0.5 })
        );
        assert_eq!(
            parsed[3].apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::NightsSinceLastRaid { change: -1 }),
            "the raid reward adds, so the shipped -1 takes a night away"
        );
        assert_eq!(
            parsed[2].apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::FinishResearch {
                research: id("minecraft:this")
            }),
            "`new ResourceLocation(\"this\")` is `minecraft:this`"
        );
        assert_eq!(
            parsed[7].apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::UnlockQuest {
                quest: id("minecolonies:questtemplate")
            })
        );
    }

    #[test]
    fn happiness_reuses_the_citizen_modules_quest_modifier() {
        // `new ExpirationBasedHappinessModifier(QUEST, 2.0, StaticHappinessSupplier(qty), days)`
        // (`HappinessRewardTemplate.java:65`) is `Modifier::quest`.
        let Some(Outcome::Happiness { citizen, modifier }) = rewards()[5].apply(&cast(), &Couples(Vec::new()))
        else {
            panic!("the happiness reward changes a modifier");
        };
        assert_eq!(citizen, 3, "target 0 is the quest giver");
        assert_eq!(modifier, Modifier::quest(1.0, 7));
        assert_eq!(modifier.weight(), 2.0, "`:65`");
        assert_eq!(modifier.source_of(), Source::Quest);
        assert_eq!(modifier.days(), 7);
    }

    #[test]
    fn a_skill_reward_levels_the_giver_or_the_named_participant() {
        let skill = Reward::Skill {
            target: Target::Giver,
            skill: Skill::Strength,
            levels: 2,
        };
        assert_eq!(
            skill.apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::IncrementSkill {
                citizen: 3,
                skill: Skill::Strength,
                levels: 2
            })
        );
        let second_participant = Reward::Skill {
            target: Target::Participant(2),
            skill: Skill::Focus,
            levels: 1,
        };
        assert_eq!(
            second_participant.apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::IncrementSkill {
                citizen: 5,
                skill: Skill::Focus,
                levels: 1
            })
        );
    }

    #[test]
    fn an_item_reward_gives_the_count_and_keeps_its_tag_as_text() {
        // `:47-60`: the tag is parsed by `TagParser`, which this port does not
        // have (`MANQUANT.md`), so the text crosses over untouched.
        let reward = Reward::from_json(
            &json(r#"{"type":"minecolonies:item","details":{"item":"minecraft:paper","qty":5,"nbt":"{display:{}}"}}"#),
            "rewards[0]",
        )
        .expect("a valid reward");
        assert_eq!(
            reward.apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::GiveItem {
                item: ItemName::new("minecraft:paper").expect("an item"),
                count: 5,
                nbt: Some("{display:{}}".to_owned())
            })
        );
    }

    #[test]
    fn the_relationship_reward_of_the_shipped_file_only_ever_uncouples() {
        // `jsonObject.get(TYPE_KEY)` (`:54`) reads the reward's own type, so the
        // `"type": "couple"` inside `details` is never seen.
        let relationship = &rewards()[4];
        let Reward::Relationship { relation, .. } = relationship else {
            panic!("the fifth reward is the relationship one");
        };
        assert_eq!(relation, "minecolonies:relationship");
        assert_ne!(relation, COUPLE);

        // Two free citizens: nothing happens, because the `if` takes its `else`.
        assert_eq!(
            relationship.apply(&cast(), &Couples(Vec::new())),
            None,
            "the shipped file asks for a couple and gets nothing"
        );
        // An existing couple is removed, which is the reachable half.
        assert_eq!(
            relationship.apply(&cast(), &Couples(vec![(3, 4), (4, 3)])),
            Some(Outcome::Uncouple { first: 3, second: 4 })
        );
    }

    #[test]
    fn a_couple_at_the_top_level_couples_two_free_citizens_and_nothing_else() {
        // The branch `if (type.equals("couple"))` (`:85-92`), reachable only from
        // a file that puts the word where Java looks for it.
        let couple = Reward::Relationship {
            first: Target::Giver,
            second: Target::Participant(1),
            relation: COUPLE.to_owned(),
        };
        assert_eq!(
            couple.apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::Couple { first: 3, second: 4 })
        );
        assert_eq!(
            couple.apply(&cast(), &Couples(vec![(4, 9)])),
            None,
            "one of them already has a partner (`:87`)"
        );
    }

    #[test]
    fn the_same_citizen_named_twice_is_coupled_to_itself() {
        // Nothing in `:85-92` checks that the two targets differ.
        let both_giver = Reward::Relationship {
            first: Target::Giver,
            second: Target::Giver,
            relation: COUPLE.to_owned(),
        };
        assert_eq!(
            both_giver.apply(&cast(), &Couples(Vec::new())),
            Some(Outcome::Couple { first: 3, second: 3 })
        );
    }

    #[test]
    fn a_reward_whose_citizen_is_gone_answers_nothing_instead_of_panicking() {
        // Java casts the `null` `getCivilian(…)` unchecked here
        // (`SkillRewardTemplate.java:62`).
        let skill = Reward::Skill {
            target: Target::Participant(3),
            skill: Skill::Focus,
            levels: 1,
        };
        assert_eq!(skill.apply(&cast(), &Couples(Vec::new())), None);
        let no_giver = Cast {
            giver: None,
            participants: &[4],
        };
        assert_eq!(
            Reward::Happiness {
                target: Target::Giver,
                amount: 1.0,
                days: 1
            }
            .apply(&no_giver, &Couples(Vec::new())),
            None
        );
    }

    #[test]
    fn a_relationship_with_a_gone_citizen_answers_nothing() {
        // `catch (Exception)` around the whole body (`:105-108`).
        let relationship = Reward::Relationship {
            first: Target::Giver,
            second: Target::Participant(3),
            relation: COUPLE.to_owned(),
        };
        assert_eq!(relationship.apply(&cast(), &Couples(Vec::new())), None);
    }

    #[test]
    fn worker_idle_sets_one_day_and_both_endings_clear_it() {
        // `:36-69`.
        let mut effect = WorkerIdle::new();
        assert_eq!(effect.applied_to(), None);
        assert_eq!(effect.on_start(), None, "the apply call is commented out (`:39`)");
        assert_eq!(
            effect.apply_to(12),
            Change {
                citizen: 12,
                days: 1
            }
        );
        assert_eq!(effect.applied_to(), Some(12));
        assert_eq!(
            effect.on_finish(),
            Some(Change {
                citizen: 12,
                days: 0
            })
        );
        assert_eq!(effect.applied_to(), None, "cleared once");
        assert_eq!(
            effect.on_cancel(),
            None,
            "Java dereferences a null citizen here; the port has nothing to clear"
        );
    }

    #[test]
    fn worker_idle_saves_nothing_about_its_citizen() {
        // `IQuestSideEffect.serializeNBT` is the default empty compound
        // (`:40-47`) and `WorkerIdleSideEffect` does not override it.
        let mut effect = WorkerIdle::new();
        effect.apply_to(12);
        assert!(effect.to_compound().is_empty());
        assert_eq!(WorkerIdle::ID, "minecolonies:workeridle", ":13");
    }

    #[test]
    fn an_unknown_reward_or_an_unknown_skill_is_refused_by_name() {
        assert_eq!(
            Reward::from_json(&json(r#"{"type":"minecolonies:teleport"}"#), "rewards[3]"),
            Err(Refusal::UnknownType {
                at: "rewards[3]".into(),
                kind: "reward",
                text: "minecolonies:teleport".into()
            })
        );
        assert_eq!(
            Reward::from_json(
                &json(r#"{"type":"minecolonies:skill","details":{"target":0,"skill":"strength","qty":1}}"#),
                "rewards[1]"
            ),
            Err(Refusal::UnknownSkill {
                at: "rewards[1].details".into(),
                text: "strength".into()
            }),
            "`Skill.valueOf` is case-sensitive"
        );
    }
}
