//! The seven kinds of quest objective: what a file declares, what counts as
//! done, and which game event each one is waiting for.
//!
//! Java reference: `core/quests/objectives/` — `DialogueObjectiveTemplateTemplate.java`
//! (135, the base every other type extends), `BreakBlockObjectiveTemplate.java`
//! (210), `PlaceBlockObjectiveTemplate.java` (208), `KillEntityObjectiveTemplateTemplate.java`
//! (206), `DeliveryObjectiveTemplateTemplate.java` (152), `BuildBuildingObjectiveTemplate.java`
//! (345), `ResearchObjectiveTemplate.java` (165), plus the six one-method
//! interfaces (`IBreakBlockObjectiveTemplate.java:10-19` and its five siblings)
//! and the dialogue tree itself (`api/quests/IDialogueObjectiveTemplate.java:32-137`).
//!
//! | Objective | What counts it | What "done" means |
//! | --- | --- | --- |
//! | [`Objective::BreakBlock`] | `BlockEvent.BreakEvent` on that block | `qty` of them |
//! | [`Objective::PlaceBlock`] | `BlockEvent.EntityPlaceEvent` on that block | `qty` of them |
//! | [`Objective::KillEntity`] | `LivingDeathEvent` of that entity type | `qty` of them |
//! | [`Objective::BuildBuilding`] | a building of that type reaching `lvl` | `qty` of them, or `lvl` levels summed |
//! | [`Objective::Research`] | the research finishing | the research is complete |
//! | [`Objective::Delivery`] | the player handing the stack over | a dialogue answer, not a counter |
//! | [`Objective::Dialogue`] | the answer taken in a tree | the answer says which objective is next |
//!
//! **Four of the seven own a counter, and the counter lives in the quest
//! instance.** `createObjectiveInstance` answers `null` for dialogue, delivery and
//! research (`DialogueObjectiveTemplateTemplate.java:108-120` has no override,
//! `DeliveryObjectiveTemplateTemplate` none either, `ResearchObjectiveTemplate.java:86-102`
//! returns `null`), so [`Progress`] is `Option` and each counter serialises as the
//! one `quantity` key (`BreakBlockObjectiveTemplate.java:191-196`).
//!
//! **A building objective counts two different things under the same field.**
//! `qty > 0` means "`qty` buildings of this type at level `lvl` or better"; `qty
//! <= 0` means "`lvl` levels in total across every building of this type"
//! (`BuildBuildingObjectiveTemplate.java:166-191`, `:319-323`). With `qty == 0`
//! **every** upgrade counts, even one that stays below `lvl` (`:291-293`), and
//! `getMissingQuantity` answers `0` whatever the progress is (`:325-329`).
//!
//! **A dialogue objective reads `target` from the top level, not from
//! `details`.** Compare `DialogueObjectiveTemplateTemplate.java:81`
//! (`jsonObject.get(TARGET_KEY)`) with `BreakBlockObjectiveTemplate.java:79`
//! (`details.get(TARGET_KEY)`) — the three block/entity objectives, delivery,
//! building and research all read it inside `details`. A file that puts `target`
//! in `details` for a dialogue gets Java's `NullPointerException`.
//!
//! **`next-objective` defaults to `-1`, and `-1` means "the quest ends"**
//! (`:82`, `QuestInstance.java:211-215`). `unlocks-rewards` is read from the top
//! level of every objective, not from `details` (`:91-106`), and lists *indices*
//! into the quest's `rewards` array.
//!
//! **The `nbt` of a delivery or an item is SNBT text**, parsed by
//! `TagParser.parseTag(GsonHelper.getAsString(details, NBT_KEY))`
//! (`DeliveryObjectiveTemplateTemplate.java:105-114`). This port keeps the text
//! verbatim and hands it to the caller: writing an SNBT reader is not a quest
//! rule, and it is listed in `MANQUANT.md`. What it *is* used for is decided by
//! `nbt-mode`: `"any"` ignores both the tag and the damage
//! (`:123`, `:129` — one flag feeds `ignoreNBT` and `ignoreDamage`), anything else
//! requires both.

use mod_api::serde_json::{Map, Value};
use mod_api::{Compound, Identifier, Tag};

use crate::inventory::ItemName;

use super::json::{self, Refusal};

/// The seven objective type ids (`QuestRegistries.java:160-166`).
pub const DIALOGUE: &str = "minecolonies:dialogue";
pub const BREAKBLOCK: &str = "minecolonies:breakblock";
pub const PLACEBLOCK: &str = "minecolonies:placeblock";
pub const KILLENTITY: &str = "minecolonies:killentity";
pub const DELIVERY: &str = "minecolonies:delivery";
pub const BUILD_BUILDING: &str = "minecolonies:buildbuilding";
pub const RESEARCH: &str = "minecolonies:research";

/// The four dialogue answer type ids (`QuestRegistries.java:175-178`).
pub const ANSWER_DIALOGUE: &str = "minecolonies:dialogue";
pub const ANSWER_RETURN: &str = "minecolonies:return";
pub const ANSWER_CANCEL: &str = "minecolonies:cancel";
pub const ANSWER_ADVANCE: &str = "minecolonies:advanceobjective";

/// `next-objective`'s default (`BreakBlockObjectiveTemplate.java:82`), which
/// `QuestInstance.advanceObjective` reads as "complete the quest" (`:211-215`).
pub const FINAL: i32 = -1;

/// `TAG_QUANTITY` (`NbtTagConstants.java:588`), the key every counter writes
/// (`BreakBlockObjectiveTemplate.java:194`).
pub const KEY_QUANTITY: &str = "quantity";

/// Who an objective is talking to: `0` is the quest giver, `n` is the `n`-th
/// participant (`DialogueObjectiveTemplateTemplate.java:27-29`, `:111-118`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: Java's `target == 0 ? giver : participant(target)`
pub enum Target {
    Giver,
    Participant(i32),
}

impl Target {
    /// `int target` as the file spells it (`:79`).
    pub fn from_number(number: i32) -> Target {
        if number == 0 {
            Target::Giver
        } else {
            Target::Participant(number)
        }
    }

    /// The number back, for a save or a test.
    pub fn number(self) -> i32 {
        match self {
            Target::Giver => 0,
            Target::Participant(number) => number,
        }
    }

    /// The citizen this names, if the colony still has one. Java's
    /// `getParticipant(target)` indexes the list and throws past its end
    /// (`QuestInstance.java:323-326`); an answer of `None` is the port's.
    pub fn resolve(self, giver: Option<i32>, participants: &[i32]) -> Option<i32> {
        match self {
            Target::Giver => giver,
            Target::Participant(number) => {
                usize::try_from(number - 1)
                    .ok()
                    .and_then(|index| participants.get(index).copied())
            }
        }
    }
}

/// `nbt-mode` (`DeliveryObjectiveTemplateTemplate.java:116`, `:123`): the only
/// two answers Java distinguishes, because `!text.equals("any")` is the whole
/// test and an absent key is `""`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: `"any"` or "anything else"
pub enum NbtMode {
    /// The tag and the damage are part of the match.
    Exact,
    /// `"any"`: the item id alone.
    Any,
}

impl NbtMode {
    fn from_text(text: &str) -> NbtMode {
        if text == "any" {
            NbtMode::Any
        } else {
            NbtMode::Exact
        }
    }

    /// What a hand-in is asked for under this mode: `any` does not consult the
    /// tag at all, which is what setting both flags does in Java.
    pub fn tag<'a>(self, nbt: Option<&'a str>) -> Option<&'a str> {
        match self {
            NbtMode::Exact => nbt,
            NbtMode::Any => None,
        }
    }
}

/// One node of a dialogue tree: what the citizen says and the replies offered
/// (`IDialogueObjectiveTemplate.DialogueElement`, `:32-99`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialogue {
    pub text: String,
    pub replies: Vec<Reply>,
}

/// One offered reply (`AnswerElement`, `:104-137`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub text: String,
    pub answer: Answer,
}

/// Where a reply leads (`IQuestDialogueAnswer.java:11-70`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the four registered answer ids
pub enum Answer {
    /// Another node of the same tree (`DialogueElement`, registered at
    /// `ModQuestInitializer.java:68`).
    Dialogue(Box<Dialogue>),
    /// `NextObjectiveDialogueAnswer`: advance to `go-to` (`:39-58`).
    Advance { go_to: i32 },
    /// `CloseUIDialogueAnswer`: close, change nothing (`:16-23`).
    Return,
    /// `QuestCancellationDialogueAnswer`: `quest.onDeletion()` (`:63-70`).
    Cancel,
}

/// What taking a reply does to the quest, decided without the quest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Close the screen; the quest does not move.
    Nothing,
    /// `quest.advanceObjective(player, go_to)`.
    Advance(i32),
    /// `quest.onDeletion()`.
    Cancel,
}

impl Answer {
    pub fn effect(&self) -> Effect {
        match self {
            Answer::Dialogue(_) => Effect::Nothing,
            Answer::Advance { go_to } => Effect::Advance(*go_to),
            Answer::Return => Effect::Nothing,
            Answer::Cancel => Effect::Cancel,
        }
    }
}

impl Dialogue {
    /// `DialogueElement.parse` (`:60-69`) for the node, `AnswerElement.parse`
    /// (`:132-136`) for each reply.
    pub fn from_json(value: &Value, at: &str) -> Result<Dialogue, Refusal> {
        let map = json::object(value, at)?;
        let text = json::text(map, at, "text")?.to_owned();
        let options = json::array(map, at, "options")?;
        let mut replies = Vec::with_capacity(options.len());
        for (index, option) in options.iter().enumerate() {
            let reply_at = json::element(&json::field(at, "options"), index);
            let reply = json::object(option, &reply_at)?;
            let text = json::text(reply, &reply_at, "answer")?.to_owned();
            let result_at = json::field(&reply_at, "result");
            let result = match reply.get("result") {
                Some(result) => result,
                None => {
                    return Err(Refusal::MissingKey {
                        at: reply_at.clone(),
                        key: "result",
                    });
                }
            };
            replies.push(Reply {
                text,
                answer: answer_from_json(result, &result_at)?,
            });
        }
        Ok(Dialogue { text, replies })
    }

    /// `getOptionResult(responseId)` (`:94-98`): out of range is `null`, and a
    /// negative index throws in Java where this answers `None`.
    pub fn reply(&self, index: usize) -> Option<&Reply> {
        self.replies.get(index)
    }
}

fn answer_from_json(value: &Value, at: &str) -> Result<Answer, Refusal> {
    let map = json::object(value, at)?;
    let kind = json::text(map, at, "type")?;
    match kind {
        ANSWER_DIALOGUE => Ok(Answer::Dialogue(Box::new(Dialogue::from_json(value, at)?))),
        // `jsonObject.get(GO_TO_ID).getAsInt()` (`IQuestDialogueAnswer.java:48-51`).
        ANSWER_ADVANCE => Ok(Answer::Advance {
            go_to: json::integer(map, at, "go-to")?,
        }),
        // Both registered with an ignoring lambda (`ModQuestInitializer.java:69`,
        // `:71`).
        ANSWER_RETURN => Ok(Answer::Return),
        ANSWER_CANCEL => Ok(Answer::Cancel),
        other => Err(Refusal::UnknownType {
            at: at.to_owned(),
            kind: "answer",
            text: other.to_owned(),
        }),
    }
}

/// Which game event an objective is waiting for. Java needs two pieces to
/// answer this — the objective's class (which decides the map) and the id it was
/// registered with (`QuestObjectiveEventHandler.java:62-107`) — so the port
/// spells the pair out.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five event kinds the handler subscribes to
pub enum Subject {
    BlockBroken(Identifier),
    BlockPlaced(Identifier),
    EntityKilled(Identifier),
    BuildingUpgraded(Identifier),
    ResearchCompleted(Identifier),
}

/// One game event, as the handler would hand it to an objective
/// (`QuestObjectiveEventHandler.java:70-77`, `:99-102`, `:132-137`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    BlockBroken(&'a Identifier),
    BlockPlaced(&'a Identifier),
    EntityKilled(&'a Identifier),
    /// `onBuildingUpgrade(progress, quest, level)` (`IBuildingUpgradeObjectiveTemplate.java:18`)
    /// — the level the building **reached**, not the level it had.
    BuildingUpgraded { building: &'a Identifier, level: i32 },
    ResearchCompleted(&'a Identifier),
}

/// What one event did to the objective that was listening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five answers `onBlockBreak` and its siblings can give
pub enum Seen {
    /// The event names something else; nothing happened.
    Ignored,
    /// The counter was already full, and Java returns before bumping it
    /// (`BreakBlockObjectiveTemplate.java:140-143`).
    AlreadyDone,
    /// Counted, not enough yet.
    Counting { done: i32, of: i32 },
    /// Reached the total: the caller advances to `next-objective`
    /// (`:146-150`).
    Fulfilled,
    /// A counter objective whose instance carries no counter. Java reaches this
    /// state through its own constructor (see [`super::instance::Instance`]) and
    /// dereferences `null` at `:140`; the port names it.
    NoProgress,
}

/// An inventory a delivery can be counted in and taken from —
/// `InventoryUtils.getItemCountInItemHandler` and
/// `attemptReduceStackInItemHandler` (`DeliveryObjectiveTemplateTemplate.java:123`,
/// `:129`), with the stack tag as text (see the module doc).
pub trait Hand {
    fn count(&self, item: &ItemName, nbt: Option<&str>) -> u32;
    fn drain(&mut self, item: &ItemName, nbt: Option<&str>, qty: u32) -> bool;
}

/// One quest objective.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the seven registered ids (`ModQuestInitializer.java:37-50`)
pub enum Objective {
    /// `DialogueObjectiveTemplateTemplate` (`:24-135`).
    Dialogue {
        target: Target,
        tree: Dialogue,
        unlocks: Vec<usize>,
    },
    /// `BreakBlockObjectiveTemplate` (`:27-209`).
    BreakBlock {
        target: Target,
        block: Identifier,
        count: i32,
        next: i32,
        unlocks: Vec<usize>,
    },
    /// `PlaceBlockObjectiveTemplate` (`:28-207`).
    PlaceBlock {
        target: Target,
        block: Identifier,
        count: i32,
        next: i32,
        unlocks: Vec<usize>,
    },
    /// `KillEntityObjectiveTemplateTemplate` (`:28-205`).
    KillEntity {
        target: Target,
        entity: Identifier,
        count: i32,
        next: i32,
        unlocks: Vec<usize>,
    },
    /// `DeliveryObjectiveTemplateTemplate` (`:29-151`).
    Delivery {
        target: Target,
        item: ItemName,
        count: i32,
        nbt: Option<String>,
        mode: NbtMode,
        next: i32,
        unlocks: Vec<usize>,
        /// The `waiting` tree — what `getDialogueTree` answers when the player
        /// has nothing to hand over (`:132-136`).
        waiting: Dialogue,
        /// The `ready` tree, with the `NextObjectiveDialogueAnswer` (`:138-142`).
        ready: Dialogue,
    },
    /// `BuildBuildingObjectiveTemplate` (`:27-344`).
    BuildBuilding {
        target: Target,
        building: Identifier,
        level: i32,
        count: i32,
        count_existing: bool,
        next: i32,
        unlocks: Vec<usize>,
    },
    /// `ResearchObjectiveTemplate` (`:25-164`).
    Research {
        target: Target,
        research: Identifier,
        next: i32,
        unlocks: Vec<usize>,
    },
}

impl Objective {
    /// One element of the `objectives` array (`QuestJsonListener.java:141-154`).
    pub fn from_json(value: &Value, at: &str) -> Result<Objective, Refusal> {
        let map = json::object(value, at)?;
        let kind = json::text(map, at, "type")?;
        // `parseRewards(jsonObject)` reads the top level only (`:91-106`).
        let unlocks = unlocks(map, at)?;
        match kind {
            DIALOGUE => {
                // `jsonObject.get(TARGET_KEY)` — top level (`:81`).
                let target = Target::from_number(json::integer(map, at, "target")?);
                let tree = Dialogue::from_json(value, at)?;
                Ok(Objective::Dialogue {
                    target,
                    tree,
                    unlocks,
                })
            }
            BREAKBLOCK => {
                let (details, at) = details(map, at)?;
                Ok(Objective::BreakBlock {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    block: json::identifier(details, &at, "block")?,
                    count: json::integer(details, &at, "qty")?,
                    next: next_of(details, &at)?,
                    unlocks,
                })
            }
            PLACEBLOCK => {
                let (details, at) = details(map, at)?;
                Ok(Objective::PlaceBlock {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    block: json::identifier(details, &at, "block")?,
                    count: json::integer(details, &at, "qty")?,
                    next: next_of(details, &at)?,
                    unlocks,
                })
            }
            KILLENTITY => {
                let (details, at) = details(map, at)?;
                Ok(Objective::KillEntity {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    entity: json::identifier(details, &at, "entity-type")?,
                    count: json::integer(details, &at, "qty")?,
                    next: next_of(details, &at)?,
                    unlocks,
                })
            }
            DELIVERY => {
                let (details, at) = details(map, at)?;
                let target = Target::from_number(json::integer(details, &at, "target")?);
                let item = json::item_name(details, &at, "item")?;
                let count = json::integer(details, &at, "qty")?;
                let nbt = match details.get("nbt") {
                    Some(_) => Some(json::text(details, &at, "nbt")?.to_owned()),
                    None => None,
                };
                let mode = match details.get("nbt-mode") {
                    Some(_) => NbtMode::from_text(json::text(details, &at, "nbt-mode")?),
                    None => NbtMode::Exact,
                };
                let next = next_of(details, &at)?;
                let (waiting, ready) = delivery_trees(next);
                Ok(Objective::Delivery {
                    target,
                    item,
                    count,
                    nbt,
                    mode,
                    next,
                    unlocks,
                    waiting,
                    ready,
                })
            }
            BUILD_BUILDING => {
                let (details, at) = details(map, at)?;
                Ok(Objective::BuildBuilding {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    building: json::identifier(details, &at, "type")?,
                    level: json::integer(details, &at, "lvl")?,
                    count: json::integer(details, &at, "qty")?,
                    // Required, not defaulted (`:131`).
                    count_existing: json::boolean(details, &at, "count-existing")?,
                    next: next_of(details, &at)?,
                    unlocks,
                })
            }
            RESEARCH => {
                let (details, at) = details(map, at)?;
                Ok(Objective::Research {
                    target: Target::from_number(json::integer(details, &at, "target")?),
                    // The research id is read from `type` (`:79`), the key the
                    // building objective uses for its building.
                    research: json::identifier(details, &at, "type")?,
                    next: next_of(details, &at)?,
                    unlocks,
                })
            }
            other => Err(Refusal::UnknownType {
                at: at.to_owned(),
                kind: "objective",
                text: other.to_owned(),
            }),
        }
    }

    /// The citizen this objective names (`getTarget()`, `:64-71`).
    pub fn target(&self) -> Target {
        match self {
            Objective::Dialogue { target, .. }
            | Objective::BreakBlock { target, .. }
            | Objective::PlaceBlock { target, .. }
            | Objective::KillEntity { target, .. }
            | Objective::Delivery { target, .. }
            | Objective::BuildBuilding { target, .. }
            | Objective::Research { target, .. } => *target,
        }
    }

    /// The reward indices this objective unlocks when it is left behind
    /// (`getRewardUnlocks`, `:62-66`).
    pub fn unlocks(&self) -> &[usize] {
        match self {
            Objective::Dialogue { unlocks, .. }
            | Objective::BreakBlock { unlocks, .. }
            | Objective::PlaceBlock { unlocks, .. }
            | Objective::KillEntity { unlocks, .. }
            | Objective::Delivery { unlocks, .. }
            | Objective::BuildBuilding { unlocks, .. }
            | Objective::Research { unlocks, .. } => unlocks,
        }
    }

    /// Where a fulfilment goes next; `None` for a dialogue, which moves through
    /// its answers instead.
    pub fn next(&self) -> Option<i32> {
        match self {
            Objective::Dialogue { .. } => None,
            Objective::BreakBlock { next, .. }
            | Objective::PlaceBlock { next, .. }
            | Objective::KillEntity { next, .. }
            | Objective::Delivery { next, .. }
            | Objective::BuildBuilding { next, .. }
            | Objective::Research { next, .. } => Some(*next),
        }
    }

    /// The event this objective listens for, or `None` when it needs none
    /// (`startObjective` / `onWorldLoad` register it, `:94`, `:160`).
    pub fn listener(&self) -> Option<Subject> {
        match self {
            Objective::Dialogue { .. } | Objective::Delivery { .. } => None,
            Objective::BreakBlock { block, .. } => Some(Subject::BlockBroken(block.clone())),
            Objective::PlaceBlock { block, .. } => Some(Subject::BlockPlaced(block.clone())),
            Objective::KillEntity { entity, .. } => Some(Subject::EntityKilled(entity.clone())),
            Objective::BuildBuilding { building, .. } => {
                Some(Subject::BuildingUpgraded(building.clone()))
            }
            Objective::Research { research, .. } => {
                Some(Subject::ResearchCompleted(research.clone()))
            }
        }
    }

    /// The total a counter counts to, or `None` for the three objectives that
    /// carry no counter (`createObjectiveInstance`, see the module doc).
    pub fn total(&self) -> Option<i32> {
        match self {
            Objective::BreakBlock { count, .. }
            | Objective::PlaceBlock { count, .. }
            | Objective::KillEntity { count, .. } => Some(*count),
            // `qty > 0 ? qty : lvl` (`:319-323`).
            Objective::BuildBuilding { level, count, .. } => Some(if *count > 0 {
                *count
            } else {
                *level
            }),
            Objective::Dialogue { .. }
            | Objective::Delivery { .. }
            | Objective::Research { .. } => None,
        }
    }

    /// `createObjectiveInstance()`: a counter at zero for the four objectives that
    /// own one, `None` for the three that answer `null` (`:113-116` for the block
    /// ones, `ResearchObjectiveTemplate.java:86-102`, and the two that inherit the
    /// base's `null`).
    pub fn create_progress(&self) -> Option<Progress> {
        self.total().map(|_| Progress::new())
    }

    /// `startObjective`'s initial scan when the objective says `count-existing`
    /// (`:138-158`, `:166-191`): the counter starts at what the colony already
    /// has — one per building of that type at `lvl` or better when `qty > 0`, the
    /// sum of their levels when `qty <= 0`.
    ///
    /// Java advances on the spot when that scan already fulfils the objective
    /// (`:147-150`); the caller checks [`Progress::is_fulfilled`] and skips the
    /// objective, which is the same decision without the recursion.
    pub fn initial_progress(&self, buildings: &[(Identifier, i32)]) -> Option<Progress> {
        match self {
            Objective::BuildBuilding {
                building,
                level,
                count,
                count_existing: true,
                ..
            } => {
                let mut done = 0;
                for (kind, at_level) in buildings {
                    if kind != building {
                        continue;
                    }
                    if *count > 0 {
                        if *at_level >= *level {
                            done += 1;
                        }
                    } else {
                        done += *at_level;
                    }
                }
                Some(Progress { done })
            }
            _ => self.create_progress(),
        }
    }

    /// The one rule of `onBuildingUpgrade` (`:271-296`), the only counter whose
    /// increment is conditional: with `qty > 0` an upgrade below `lvl` is ignored,
    /// with `qty <= 0` it counts.
    pub fn counts_upgrade(&self, reached: i32) -> bool {
        match self {
            Objective::BuildBuilding { level, count, .. } => *count <= 0 || reached >= *level,
            _ => false,
        }
    }

    /// `onBlockBreak` / `onBlockPlace` / `onEntityKill` / `onBuildingUpgrade` /
    /// `onResearchCompletion`, in one place: bump the counter, say whether the
    /// objective is now fulfilled.
    pub fn observe(&self, progress: &mut Option<Progress>, event: &Event<'_>) -> Seen {
        match (self, event) {
            (Objective::BreakBlock { block, .. }, Event::BlockBroken(hit)) if block == *hit => {
                self.count(progress)
            }
            (Objective::PlaceBlock { block, .. }, Event::BlockPlaced(hit)) if block == *hit => {
                self.count(progress)
            }
            (Objective::KillEntity { entity, .. }, Event::EntityKilled(hit)) if entity == *hit => {
                self.count(progress)
            }
            (Objective::BuildBuilding { building, .. }, Event::BuildingUpgraded { building: hit, level })
                if building == *hit =>
            {
                if !self.counts_upgrade(*level) {
                    return Seen::Ignored;
                }
                self.count(progress)
            }
            // The research has no counter: reaching the completion is the
            // fulfilment (`ResearchObjectiveTemplate.java:159-164`).
            (Objective::Research { research, .. }, Event::ResearchCompleted(hit))
                if research == *hit =>
            {
                Seen::Fulfilled
            }
            _ => Seen::Ignored,
        }
    }

    fn count(&self, progress: &mut Option<Progress>) -> Seen {
        let of = match self.total() {
            Some(of) => of,
            None => return Seen::Ignored,
        };
        match progress {
            None => Seen::NoProgress,
            Some(progress) => {
                if progress.done >= of {
                    return Seen::AlreadyDone;
                }
                progress.done += 1;
                if progress.done >= of {
                    Seen::Fulfilled
                } else {
                    Seen::Counting {
                        done: progress.done,
                        of,
                    }
                }
            }
        }
    }

    /// `hasItem(player, quest)` (`DeliveryObjectiveTemplateTemplate.java:120-124`):
    /// does the hand hold `qty` of the stack?
    pub fn has_item(&self, hand: &dyn Hand) -> bool {
        match self {
            Objective::Delivery {
                item, count, nbt, mode, ..
            } => hand.count(item, mode.tag(nbt.as_deref())) >= u32::try_from(*count).unwrap_or(0),
            _ => false,
        }
    }

    /// `tryDiscountItem` (`:126-130`): take the stack, if it is there.
    pub fn take_item(&self, hand: &mut dyn Hand) -> bool {
        match self {
            Objective::Delivery {
                item, count, nbt, mode, ..
            } => {
                let qty = u32::try_from(*count).unwrap_or(0);
                hand.drain(item, mode.tag(nbt.as_deref()), qty)
            }
            _ => false,
        }
    }

    /// The tree to play for a delivery: `ready` when the hand holds the stack,
    /// `waiting` otherwise (`:132-142`). `None` for every other objective, whose
    /// single tree is the `tree` field of a dialogue objective.
    pub fn delivery_tree(&self, hand: &dyn Hand) -> Option<&Dialogue> {
        match self {
            Objective::Delivery { ready, waiting, .. } => {
                if self.has_item(hand) {
                    Some(ready)
                } else {
                    Some(waiting)
                }
            }
            _ => None,
        }
    }
}

/// `buildDialogueTrees` (`DeliveryObjectiveTemplateTemplate.java:75-90`): the two
/// trees are built from the objective's own item and `next`, never read from the
/// file.
///
/// Both texts are translation keys and nothing else: Java passes the item's
/// display name as the key's one argument (`:77`, `:84`), and the item is the
/// objective's own field, so the port keeps the key clean instead of inventing a
/// formatted variant of it.
fn delivery_trees(next: i32) -> (Dialogue, Dialogue) {
    let waiting = Dialogue {
        text: "com.minecolonies.coremod.questobjectives.delivery.waiting".to_owned(),
        replies: vec![
            Reply {
                text: "com.minecolonies.coremod.questobjectives.answer.later".to_owned(),
                answer: Answer::Return,
            },
            Reply {
                text: "com.minecolonies.coremod.questobjectives.delivery.waiting.cancel".to_owned(),
                answer: Answer::Cancel,
            },
        ],
    };
    let ready = Dialogue {
        text: "com.minecolonies.coremod.questobjectives.delivery.ready".to_owned(),
        replies: vec![
            Reply {
                text: "com.minecolonies.coremod.questobjectives.delivery.ready.give".to_owned(),
                answer: Answer::Advance { go_to: next },
            },
            Reply {
                text: "com.minecolonies.coremod.questobjectives.delivery.ready.later".to_owned(),
                answer: Answer::Return,
            },
        ],
    };
    (waiting, ready)
}

/// The `details` block, and the path of every read inside it.
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

/// `details.has(NEXT_OBJ_KEY) ? … : -1` (`BreakBlockObjectiveTemplate.java:82`).
fn next_of(details: &Map<String, Value>, at: &str) -> Result<i32, Refusal> {
    match details.get("next-objective") {
        Some(_) => json::integer(details, at, "next-objective"),
        None => Ok(FINAL),
    }
}

/// `parseRewards` (`DialogueObjectiveTemplateTemplate.java:91-106`): the
/// `unlocks-rewards` array, empty when absent.
fn unlocks(map: &Map<String, Value>, at: &str) -> Result<Vec<usize>, Refusal> {
    if map.get("unlocks-rewards").is_none() {
        return Ok(Vec::new());
    }
    let list = json::array(map, at, "unlocks-rewards")?;
    let mut out = Vec::with_capacity(list.len());
    for (index, value) in list.iter().enumerate() {
        let entry_at = json::element(&json::field(at, "unlocks-rewards"), index);
        let number = value.as_i64().ok_or(Refusal::WrongType {
            at: entry_at,
            key: "unlocks-rewards",
            expected: "a reward index",
        })?;
        out.push(usize::try_from(number).map_err(|_| Refusal::WrongType {
            at: json::element(&json::field(at, "unlocks-rewards"), index),
            key: "unlocks-rewards",
            expected: "a non-negative reward index",
        })?);
    }
    Ok(out)
}

/// The counter of the objective in progress: Java's
/// `BlockMiningProgressInstance` (`BreakBlockObjectiveTemplate.java:167-209`) and
/// its two siblings, which all hold one `currentProgress` and serialise it as
/// `quantity`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    done: i32,
}

impl Progress {
    /// `createObjectiveInstance`, starting at zero (`:169`).
    pub fn new() -> Progress {
        Progress { done: 0 }
    }

    pub fn done(&self) -> i32 {
        self.done
    }

    /// `isFulfilled()` (`:184-188` for the block objectives, `:319-323` for the
    /// building one, which compares against `lvl` instead of `qty` when `qty` is
    /// not positive).
    pub fn is_fulfilled(&self, objective: &Objective) -> bool {
        objective
            .total()
            .is_some_and(|total| self.done >= total)
    }

    /// `getMissingQuantity()` — and the building objective answers `0` always
    /// (`:325-329`), even when it is not fulfilled, which is a difference the
    /// caller must not paper over.
    pub fn missing(&self, objective: &Objective) -> i32 {
        if matches!(objective, Objective::BuildBuilding { .. }) {
            return 0;
        }
        match objective.total() {
            Some(total) if total > self.done => total - self.done,
            _ => 0,
        }
    }

    /// `serializeNBT` (`:191-196`): one `quantity`.
    pub fn to_compound(&self) -> Compound {
        Compound::from([(KEY_QUANTITY.into(), Tag::Int(self.done))])
    }

    /// `deserializeNBT` (`:205-208`): `getInt` on an absent key is `0`.
    pub fn from_compound(data: &Compound) -> Progress {
        let done = match data.get(&KEY_QUANTITY.into()) {
            Some(Tag::Int(value)) => *value,
            Some(Tag::Short(value)) => i32::from(*value),
            Some(Tag::Byte(value)) => i32::from(*value),
            _ => 0,
        };
        Progress { done }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    fn item(text: &str) -> ItemName {
        ItemName::new(text).expect("a literal is an item")
    }

    fn break_dirt() -> Objective {
        Objective::from_json(
            &json(r#"{"type":"minecolonies:breakblock","details":{"target":0,"block":"minecraft:dirt","qty":2}}"#),
            "objectives[0]",
        )
        .expect("a valid objective")
    }

    /// A pocket that holds one stack and remembers what it was asked for. The
    /// tag it holds is not compared — that is the caller's half of the rule (see
    /// `MANQUANT.md` on SNBT), so what it records is *which* tag it was asked for.
    struct Pocket {
        item: ItemName,
        count: u32,
        asked: Vec<Option<String>>,
    }

    impl Hand for Pocket {
        fn count(&self, item: &ItemName, nbt: Option<&str>) -> u32 {
            let _ = nbt;
            if item != &self.item {
                return 0;
            }
            self.count
        }

        fn drain(&mut self, item: &ItemName, nbt: Option<&str>, qty: u32) -> bool {
            self.asked.push(nbt.map(str::to_owned));
            if item != &self.item || self.count < qty {
                return false;
            }
            self.count -= qty;
            true
        }
    }

    #[test]
    fn a_dialogue_objective_reads_target_from_the_top_level() {
        // `jsonObject.get(TARGET_KEY)` (`DialogueObjectiveTemplateTemplate.java:81`).
        let value = json(
            r#"{"type":"minecolonies:dialogue","target":1,"text":"hi","options":[{"answer":"ok","result":{"type":"minecolonies:return"}}]}"#,
        );
        let objective = Objective::from_json(&value, "objectives[3]").expect("a valid objective");
        assert_eq!(objective.target(), Target::Participant(1));
        assert_eq!(
            Objective::from_json(
                &json(r#"{"type":"minecolonies:dialogue","details":{"target":1},"text":"hi","options":[]}"#),
                "objectives[3]"
            ),
            Err(Refusal::MissingKey {
                at: "objectives[3]".into(),
                key: "target"
            }),
            "a dialogue objective has no details block"
        );
    }

    #[test]
    fn an_absent_next_objective_means_the_quest_ends() {
        // `details.has(NEXT_OBJ_KEY) ? … : -1` (`BreakBlockObjectiveTemplate.java:82`).
        let objective = break_dirt();
        assert_eq!(objective.next(), Some(FINAL));
        assert_eq!(FINAL, -1);
        assert_eq!(objective.total(), Some(2));
        assert_eq!(objective.listener(), Some(Subject::BlockBroken(id("minecraft:dirt"))));
    }

    #[test]
    fn a_counter_bumps_only_for_its_own_subject() {
        let objective = break_dirt();
        let mut progress = Some(Progress::new());
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&id("minecraft:stone"))),
            Seen::Ignored
        );
        assert_eq!(progress.unwrap().done(), 0);
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&id("minecraft:dirt"))),
            Seen::Counting { done: 1, of: 2 }
        );
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&id("minecraft:dirt"))),
            Seen::Fulfilled
        );
        assert_eq!(
            objective.observe(&mut progress, &Event::BlockBroken(&id("minecraft:dirt"))),
            Seen::AlreadyDone,
            "Java returns before bumping a full counter (`:140-143`)"
        );
        assert_eq!(progress.unwrap().done(), 2, "the third break changed nothing");
    }

    #[test]
    fn a_counter_without_an_instance_says_so_instead_of_dereferencing_null() {
        // `QuestInstance.java:92` starts objective 0 and drops the instance it
        // returns, so the event handler passes `null` here (`:70-77`).
        let objective = break_dirt();
        let mut nothing: Option<Progress> = None;
        assert_eq!(
            objective.observe(&mut nothing, &Event::BlockBroken(&id("minecraft:dirt"))),
            Seen::NoProgress
        );
    }

    #[test]
    fn a_building_objective_counts_buildings_at_a_level_or_levels_in_total() {
        // `:283-293` and `:319-323`.
        let one_at_three = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":3,"qty":1,"count-existing":false}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        assert_eq!(one_at_three.total(), Some(1));
        assert!(one_at_three.counts_upgrade(3));
        assert!(!one_at_three.counts_upgrade(2), "a level-2 upgrade is ignored");

        let three_levels_total = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":3,"qty":0,"count-existing":true}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        assert_eq!(three_levels_total.total(), Some(3));
        assert!(
            three_levels_total.counts_upgrade(1),
            "with qty 0 every upgrade counts, even below lvl"
        );

        let mut progress = Some(Progress::new());
        let tier = id("minecolonies:builder");
        assert_eq!(
            three_levels_total.observe(
                &mut progress,
                &Event::BuildingUpgraded {
                    building: &tier,
                    level: 1
                }
            ),
            Seen::Counting { done: 1, of: 3 }
        );
    }

    #[test]
    fn a_building_progress_reports_zero_missing_whatever_it_holds() {
        // `getMissingQuantity()` answers 0 (`:325-329`).
        let building = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":3,"qty":1,"count-existing":false}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        let fresh = Progress::new();
        assert!(!fresh.is_fulfilled(&building));
        assert_eq!(fresh.missing(&building), 0);
        assert_eq!(
            Progress::new().missing(&break_dirt()),
            2,
            "the block counters do report what is missing"
        );
    }

    #[test]
    fn a_research_objective_is_fulfilled_by_its_own_completion_and_carries_no_counter() {
        // `ResearchObjectiveTemplate.java:86-102`, `:159-164`.
        let objective = Objective::from_json(
            &json(r#"{"type":"minecolonies:research","details":{"target":0,"type":"minecolonies:research/blockhutbuilder"}}"#),
            "objectives[1]",
        )
        .expect("a valid objective");
        assert_eq!(objective.total(), None, "no progress instance");
        assert_eq!(
            objective.listener(),
            Some(Subject::ResearchCompleted(id(
                "minecolonies:research/blockhutbuilder"
            )))
        );
        let mut nothing: Option<Progress> = None;
        assert_eq!(
            objective.observe(
                &mut nothing,
                &Event::ResearchCompleted(&id("minecolonies:research/blockhutbuilder"))
            ),
            Seen::Fulfilled
        );
        assert_eq!(
            objective.observe(
                &mut nothing,
                &Event::ResearchCompleted(&id("minecolonies:research/other"))
            ),
            Seen::Ignored
        );
    }

    #[test]
    fn a_delivery_hands_the_stack_over_and_any_ignores_its_tag() {
        // `:120-130`, with `nbt-mode` feeding both match flags.
        let exact = Objective::from_json(
            &json(r#"{"type":"minecolonies:delivery","details":{"target":0,"item":"minecraft:apple","qty":2,"nbt":"{}"}}"#),
            "objectives[1]",
        )
        .expect("a valid objective");
        let mut pocket = Pocket {
            item: item("minecraft:apple"),
            count: 2,
            asked: Vec::new(),
        };
        assert!(exact.has_item(&pocket));
        assert!(exact.take_item(&mut pocket));
        assert_eq!(pocket.asked, vec![Some("{}".to_owned())], "the tag is asked for");
        assert_eq!(pocket.count, 0);
        assert!(!exact.has_item(&pocket), "the stack is gone");

        let any = Objective::from_json(
            &json(r#"{"type":"minecolonies:delivery","details":{"target":0,"item":"minecraft:apple","qty":1,"nbt":"{}","nbt-mode":"any"}}"#),
            "objectives[2]",
        )
        .expect("a valid objective");
        let mut loose = Pocket {
            item: item("minecraft:apple"),
            count: 1,
            asked: Vec::new(),
        };
        assert!(any.take_item(&mut loose));
        assert_eq!(loose.asked, vec![None], "\"any\" does not consult the tag");
    }

    #[test]
    fn the_ready_tree_is_played_only_when_the_hand_holds_the_stack() {
        // `getReadyDialogueTree` (`:138-142`), whose "give" answer advances to
        // `next` (`:79`).
        let delivery = Objective::from_json(
            &json(r#"{"type":"minecolonies:delivery","details":{"target":0,"item":"minecraft:apple","qty":2,"next-objective":3}}"#),
            "objectives[1]",
        )
        .expect("a valid objective");
        let mut pocket = Pocket {
            item: item("minecraft:apple"),
            count: 2,
            asked: Vec::new(),
        };
        let ready = delivery.delivery_tree(&pocket).expect("a delivery tree");
        assert_eq!(
            ready.text,
            "com.minecolonies.coremod.questobjectives.delivery.ready"
        );
        assert_eq!(
            ready.replies[0].answer.effect(),
            Effect::Advance(3),
            "\"give\" advances to the objective's next"
        );
        pocket.count = 1;
        let waiting = delivery.delivery_tree(&pocket).expect("a delivery tree");
        assert_eq!(
            waiting.text,
            "com.minecolonies.coremod.questobjectives.delivery.waiting"
        );
        assert_eq!(waiting.replies[1].answer.effect(), Effect::Cancel);
    }

    #[test]
    fn a_dialogue_tree_parses_its_replies_and_what_each_one_does() {
        // `DialogueElement.parse` (`:60-69`), `AnswerElement.parse` (`:132-136`).
        let value = json(
            r#"{"type":"minecolonies:dialogue","target":0,"text":"hi",
                "options":[
                  {"answer":"bring an apple","result":{"type":"minecolonies:dialogue","text":"nice","options":[{"answer":"sure","result":{"type":"minecolonies:advanceobjective","go-to":2}}]}},
                  {"answer":"later","result":{"type":"minecolonies:return"}},
                  {"answer":"no","result":{"type":"minecolonies:cancel"}}
                ]}"#,
        );
        let objective = Objective::from_json(&value, "objectives[0]").expect("a valid objective");
        let Objective::Dialogue { tree, unlocks, .. } = &objective else {
            panic!("it is a dialogue");
        };
        assert!(unlocks.is_empty());
        assert_eq!(tree.replies.len(), 3);
        assert_eq!(tree.reply(1).expect("a reply").answer.effect(), Effect::Nothing);
        assert_eq!(tree.reply(2).expect("a reply").answer.effect(), Effect::Cancel);
        let nested = match &tree.reply(0).expect("a reply").answer {
            Answer::Dialogue(nested) => nested,
            other => panic!("expected a nested tree, got {other:?}"),
        };
        assert_eq!(
            nested.reply(0).expect("a reply").answer.effect(),
            Effect::Advance(2)
        );
        assert!(tree.reply(3).is_none(), "out of range is null in Java");
    }

    #[test]
    fn an_unknown_answer_type_is_refused_by_name() {
        let value = json(
            r#"{"type":"minecolonies:dialogue","target":0,"text":"hi","options":[{"answer":"x","result":{"type":"minecolonies:teleport"}}]}"#,
        );
        assert_eq!(
            Objective::from_json(&value, "objectives[0]"),
            Err(Refusal::UnknownType {
                at: "objectives[0].options[0].result".into(),
                kind: "answer",
                text: "minecolonies:teleport".into()
            })
        );
    }

    #[test]
    fn unlocks_rewards_sit_on_the_objective_not_in_its_details() {
        // `parseRewards(jsonObject)` (`:91-106`), and the same call from every
        // other objective's `createObjective`.
        let value = json(
            r#"{"type":"minecolonies:placeblock","unlocks-rewards":[0,2],"details":{"target":0,"block":"minecraft:dirt","qty":10}}"#,
        );
        let objective = Objective::from_json(&value, "objectives[4]").expect("a valid objective");
        assert_eq!(objective.unlocks(), &[0usize, 2][..]);

        let inside = json(
            r#"{"type":"minecolonies:placeblock","details":{"target":0,"block":"minecraft:dirt","qty":10,"unlocks-rewards":[0]}}"#,
        );
        assert!(
            Objective::from_json(&inside, "objectives[4]")
                .expect("a valid objective")
                .unlocks()
                .is_empty(),
            "a list inside details is not read"
        );
    }

    #[test]
    fn count_existing_is_required_where_every_other_number_has_a_default() {
        // `details.get(COUNT_EXIST_KEY).getAsBoolean()` (`BuildBuildingObjectiveTemplate.java:131`)
        // — no `has` guard, unlike `next-objective`.
        assert_eq!(
            Objective::from_json(
                &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":3,"qty":1}}"#),
                "objectives[6]"
            ),
            Err(Refusal::MissingKey {
                at: "objectives[6].details".into(),
                key: "count-existing"
            })
        );
    }

    #[test]
    fn the_counter_round_trips_through_the_quantity_key() {
        // `serializeNBT` / `deserializeNBT` (`:191-208`).
        let detail = Compound::from([(KEY_QUANTITY.into(), Tag::Int(7))]);
        let progress = Progress::from_compound(&detail);
        assert_eq!(progress.done(), 7);
        assert_eq!(progress.to_compound(), detail);
        assert_eq!(
            Progress::from_compound(&Compound::new()).done(),
            0,
            "getInt on an absent key is 0"
        );
    }

    #[test]
    fn count_existing_starts_the_counter_at_what_the_colony_has() {
        // `checkInitialObjectiveProgress` (`:166-191`).
        let builder = id("minecolonies:builder");
        let mine = id("minecolonies:miner");
        let colony = vec![(builder.clone(), 5), (mine, 9), (builder, 1)];

        let at_five = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":5,"qty":1,"count-existing":true}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        let progress = at_five.initial_progress(&colony).expect("a counter");
        assert_eq!(progress.done(), 1, "only the level-5 builder counts");
        assert!(progress.is_fulfilled(&at_five));

        let levels = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":8,"qty":0,"count-existing":true}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        assert_eq!(
            levels.initial_progress(&colony).expect("a counter").done(),
            6,
            "5 + 1 levels of builder, and the miner is not a builder"
        );

        let fresh = Objective::from_json(
            &json(r#"{"type":"minecolonies:buildbuilding","details":{"target":0,"type":"minecolonies:builder","lvl":8,"qty":0,"count-existing":false}}"#),
            "objectives[6]",
        )
        .expect("a valid objective");
        assert_eq!(
            fresh.initial_progress(&colony).expect("a counter").done(),
            0,
            "without count-existing the scan does not happen"
        );
    }

    #[test]
    fn an_unknown_objective_type_is_refused_and_so_is_a_missing_details_block() {
        assert_eq!(
            Objective::from_json(&json(r#"{"type":"minecolonies:fly"}"#), "objectives[0]"),
            Err(Refusal::UnknownType {
                at: "objectives[0]".into(),
                kind: "objective",
                text: "minecolonies:fly".into()
            })
        );
        assert!(matches!(
            Objective::from_json(
                &json(r#"{"type":"minecolonies:breakblock","target":0}"#),
                "objectives[0]"
            ),
            Err(Refusal::MissingKey { key: "details", .. })
        ));
    }
}
