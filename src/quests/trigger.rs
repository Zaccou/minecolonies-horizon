//! What has to be true before a colony may start a quest, and how a file says
//! it: six trigger types, the `triggerOrder` expression, and the NBT-versus-JSON
//! matcher the two matching triggers share.
//!
//! Java reference: `core/quests/triggers/` — `RandomQuestTriggerTemplate.java`
//! (45), `StateQuestTriggerTemplate.java` (84), `CitizenQuestTriggerTemplate.java`
//! (92), `UnlockQuestTriggerTemplate.java` (42), `QuestReputationTriggerTemplate`
//! (42), `WorldDifficultyTriggerTemplate` (43), the two return wrappers
//! (`BooleanTriggerReturnData.java` 35, `CitizenTriggerReturnData.java` 36) and
//! `IQuestTriggerTemplate.matchNbt` (`api/quests/IQuestTriggerTemplate.java:52-139`);
//! plus the expression parser `QuestJsonListener.parseTriggerOrder` (`:246-391`)
//! and its evaluator (`:399-453`).
//!
//! | Java | Here |
//! | --- | --- |
//! | `canTriggerQuest(colony)` + the `questId` overload (`:21`, `:29-32`) | [`Trigger::can_trigger`], `Option<Identifier>` argument |
//! | `BooleanTriggerReturnData` / `CitizenTriggerReturnData` | [`Return`] |
//! | `ColonyConstants.rand`, `getWorld().random`, `Collections.shuffle` | [`Colony::random_below`] |
//! | `parseTriggerOrder` (`:246-391`) | [`Order::parse`] |
//! | `evaluate` (`:399-453`) | [`Order::evaluate`] |
//!
//! **`!` is declared, built, and never evaluated — the port refuses it.** The
//! constant exists (`QuestParsingConstants.java:26`), the builder makes a node for
//! it like any binary operator (`:335-338`), and `evaluate`'s `switch` has cases
//! for `||` and `&&` only (`:401-452`); a `!` node falls to `default`, which does
//! `triggerMap.get("!")` on a map that only ever holds `"1"`, `"2"`, … (`:302-306`)
//! and dereferences `null`. So no quest file can use `!`, and
//! [`Order::parse`] says so by name instead of reproducing the crash.
//!
//! **The trigger count is checked against the *distinct* indices in the
//! expression.** `:283-299` collects the numbers it sees into a list without
//! duplicates and refuses when that size differs from `triggers.size()`. A
//! one-trigger quest may therefore write `"1 && 1"`, and two-trigger quest may not
//! write `"1 || 1"` — even though both would evaluate. On a mismatch Java returns
//! a `null` function which is called on the first colony tick
//! (`QuestTemplate.attemptStart`, `:75`) and throws there; the port refuses at
//! load time, which is where the file's author can still see it.
//!
//! **A numeric match is a minimum, not an equality.** `matchNbt` compares
//! `((NumericTag) nbtTag).getAsDouble() >= matchTag.getAsDouble()` (`:134-137`),
//! so `"level": 3` is true for a level-5 building. Strings are exact (`:125-128`)
//! and a boolean matches a `ByteTag` through `(byte == 0) != expected`
//! (`:129-132`), i.e. `1` is true.
//!
//! **Two of the six never fire the way a file expects.** `unlock` answers `false`
//! from the colony-only call (`UnlockQuestTriggerTemplate.java:32-35`) and only
//! answers `isUnlocked` from the two-argument one (`:38-41`), which is the one the
//! default AND path uses (`:256`); [`Trigger::can_trigger`] takes the quest id for
//! exactly that reason, and `None` reproduces the colony-only call. `citizen` with
//! no `match`/`notmatch` picks uniformly from **every** citizen and ignores
//! `hasQuestAssignment`, while the filtered path skips a busy citizen first
//! (`:62-91`) — so a file that means "any free citizen" must write `{}` and not
//! omit `state`.

use mod_api::serde_json::{Map, Value};
use mod_api::{Compound, Identifier, Tag};

use super::json::{self, Refusal};

/// `MAX_TICKRATE` (`api/entity/ai/statemachine/tickratestatemachine/TickRateConstants.java:11`),
/// read by the random trigger (`RandomQuestTriggerTemplate.java:8`, `:43`).
pub const MAX_TICKRATE: i32 = 500;

/// The six trigger type ids (`QuestRegistries.java:168-173`), spelled as a file
/// spells them.
pub const RANDOM: &str = "minecolonies:random";
pub const STATE: &str = "minecolonies:state";
pub const CITIZEN: &str = "minecolonies:citizen";
pub const UNLOCK: &str = "minecolonies:unlock";
pub const REPUTATION: &str = "minecolonies:questreputation";
pub const DIFFICULTY: &str = "minecolonies:difficulty";

/// `net.minecraft.world.Difficulty`, as `Difficulty.valueOf` reads it
/// (`WorldDifficultyTriggerTemplate.java:35`). Java's `valueOf` is the enum
/// constant's name, so the file says `"NORMAL"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the four difficulties of the game
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    /// `Difficulty.valueOf(text)`, i.e. exact and case-sensitive.
    pub fn from_name(text: &str) -> Option<Difficulty> {
        match text {
            "PEACEFUL" => Some(Difficulty::Peaceful),
            "EASY" => Some(Difficulty::Easy),
            "NORMAL" => Some(Difficulty::Normal),
            "HARD" => Some(Difficulty::Hard),
            _ => None,
        }
    }

    /// The constant's name, for a test or a log line.
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "PEACEFUL",
            Difficulty::Easy => "EASY",
            Difficulty::Normal => "NORMAL",
            Difficulty::Hard => "HARD",
        }
    }
}

/// One citizen as a trigger sees it: its id, whether it is already on a quest,
/// and the compound `matchNbt` is handed
/// (`CitizenQuestTriggerTemplate.java:78-83`, `CitizenData.java:2082-2085`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Citizen {
    pub id: i32,
    pub on_quest: bool,
    pub data: Compound,
}

/// The colony, reduced to the seven answers a trigger asks. Java reaches all of
/// them through `IColony`; a parameter here for the same reason `loot::Source` is
/// one — a wasm mod has no live colony to point at.
pub trait Colony {
    /// `colony.getDay()`, the counter `QuestInstance.isValid` measures against
    /// (`QuestInstance.java:152`).
    fn day(&self) -> i32;

    /// `colony.getQuestManager().getReputation()` (`QuestReputationTriggerTemplate.java:40`).
    fn reputation(&self) -> f64;

    /// `colony.getWorld().getDifficulty()` (`WorldDifficultyTriggerTemplate.java:41`).
    fn difficulty(&self) -> Difficulty;

    /// `colony.getQuestManager().isUnlocked(questId)` (`UnlockQuestTriggerTemplate.java:40`).
    fn is_unlocked(&self, quest: &Identifier) -> bool;

    /// `colony.getCitizenManager().getCitizens()` (`CitizenQuestTriggerTemplate.java:61`).
    fn citizens(&self) -> &[Citizen];

    /// `colony.getColonyTag()`, which the state trigger walks
    /// (`StateQuestTriggerTemplate.java:63-74`).
    fn tag(&self) -> &Compound;

    /// A draw in `[0, bound)`: `nextInt(bound)` (`RandomQuestTriggerTemplate.java:43`)
    /// and the shuffle both come from here. Java has two `Random`s and a shuffle;
    /// one source is what makes the rules testable. `bound` is always positive
    /// when this is called.
    fn random_below(&mut self, bound: i32) -> i32;
}

/// What a trigger answers. Java has two wrappers because one carries a citizen
/// id and the other a boolean; the boolean one is *negative* when it says false,
/// and a citizen one is negative when it carries `null`
/// (`CitizenTriggerReturnData.java:26-29`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: `BooleanTriggerReturnData` and `CitizenTriggerReturnData`
pub enum Return {
    /// `BooleanTriggerReturnData.isPositive()` (`:25-28`).
    Matched(bool),
    /// `CitizenTriggerReturnData` (`:26-34`).
    Citizen(Option<i32>),
}

impl Return {
    /// `ITriggerReturnData.isPositive()` (`api/quests/ITriggerReturnData.java:11`).
    pub fn is_positive(self) -> bool {
        match self {
            Return::Matched(matched) => matched,
            Return::Citizen(citizen) => citizen.is_some(),
        }
    }

    /// The citizen this answer names, if any — the only content Java reads
    /// (`QuestInstance.java:79-91`).
    pub fn citizen(self) -> Option<i32> {
        match self {
            Return::Matched(_) => None,
            Return::Citizen(citizen) => citizen,
        }
    }
}

/// One trigger of a quest file.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the six registered ids (`ModQuestInitializer.java:52-57`)
pub enum Trigger {
    /// `RandomQuestTriggerTemplate` (`:14-45`). `rarity` is the `oneInChance`
    /// field: one attempt in that many is taken, once every
    /// [`MAX_TICKRATE`] ticks.
    Random { rarity: i32 },
    /// `StateQuestTriggerTemplate` (`:17-84`): walk [`Colony::tag`] down `path`,
    /// then match `want` there.
    State {
        path: Vec<String>,
        want: Value,
        count: i32,
    },
    /// `CitizenQuestTriggerTemplate` (`:19-92`).
    Citizen {
        want: Option<Value>,
        avoid: Option<Value>,
    },
    /// `UnlockQuestTriggerTemplate` (`:12-42`).
    Unlock,
    /// `QuestReputationTriggerTemplate` (`:13-42`).
    Reputation { min: f64 },
    /// `WorldDifficultyTriggerTemplate` (`:14-43`).
    Difficulty { difficulty: Difficulty },
}

impl Trigger {
    /// One trigger of the `triggers` array (`QuestJsonListener.java:126-139`).
    pub fn from_json(value: &Value, at: &str) -> Result<Trigger, Refusal> {
        let map: &Map<String, Value> = json::object(value, at)?;
        let kind = json::text(map, at, "type")?;
        match kind {
            RANDOM => {
                // `randomQuestTriggerJson.get(RARITY_ID).getAsInt()` (`:36`).
                let rarity = json::integer(map, at, "rarity")?;
                Ok(Trigger::Random { rarity })
            }
            STATE => {
                // `subObj.get(PATH_ID)`, `get(MATCH_ID)`, `has(COUNT_ID) ? … : 1`
                // (`StateQuestTriggerTemplate.java:53-57`).
                let state = json::object(state_of(map, at)?, &json::field(at, "state"))?;
                let path_at = json::field(at, "state");
                let path = json::text(state, &path_at, "path")?
                    .split('/')
                    .map(str::to_owned)
                    .collect();
                let want = state.get("match").cloned().ok_or_else(|| Refusal::MissingKey {
                    at: path_at.clone(),
                    key: "match",
                })?;
                let count = match state.get("count") {
                    Some(_) => json::integer(state, &path_at, "count")?,
                    None => 1,
                };
                Ok(Trigger::State { path, want, count })
            }
            CITIZEN => {
                // Both keys are optional and read from the `state` object
                // (`:37-45`), which is the object itself, not a `details` block.
                let state = json::object(state_of(map, at)?, &json::field(at, "state"))?;
                let want = state.get("match").cloned();
                let avoid = state.get("notmatch").cloned();
                Ok(Trigger::Citizen { want, avoid })
            }
            // `UnlockQuestTriggerTemplate.createUnlockTrigger(ignoreJson)` — the
            // parameter's name is the whole rule (`:26-29`).
            UNLOCK => Ok(Trigger::Unlock),
            // `jsonObj.get(QUANTITY_KEY).getAsDouble()` (`:34`).
            REPUTATION => Ok(Trigger::Reputation {
                min: json::number(map, at, "qty")?,
            }),
            // `Difficulty.valueOf(jsonObj.get(DIFFICULTY_KEY).getAsString())`
            // (`:35`): an unknown name throws `IllegalArgumentException`.
            DIFFICULTY => {
                let at_key = json::field(at, "difficulty");
                let text = json::text(map, at, "difficulty")?;
                let difficulty = Difficulty::from_name(text).ok_or(Refusal::UnknownType {
                    at: at_key,
                    kind: "difficulty",
                    text: text.to_owned(),
                })?;
                Ok(Trigger::Difficulty { difficulty })
            }
            other => Err(Refusal::UnknownType {
                at: at.to_owned(),
                kind: "trigger",
                text: other.to_owned(),
            }),
        }
    }

    /// `canTriggerQuest` — the one-argument overload when `quest` is `None`, the
    /// two-argument one otherwise (`IQuestTriggerTemplate.java:21-32`). Only
    /// [`Trigger::Unlock`] tells them apart; it answers `false` to the first
    /// (`UnlockQuestTriggerTemplate.java:32-35`).
    pub fn can_trigger(&self, quest: Option<&Identifier>, colony: &mut dyn Colony) -> Return {
        match self {
            // `oneInChance > 0 && nextInt(oneInChance) < MAX_TICKRATE` (`:43`):
            // the guard is what stops `nextInt(0)`, which throws.
            Trigger::Random { rarity } => {
                if *rarity <= 0 {
                    Return::Matched(false)
                } else {
                    Return::Matched(colony.random_below(*rarity) < MAX_TICKRATE)
                }
            }
            Trigger::State { path, want, count } => {
                Return::Matched(state_matches(colony.tag(), path, want, *count))
            }
            Trigger::Citizen { want, avoid } => citizen_trigger(want, avoid, colony),
            // `isUnlocked(questId)` (`:40`); the colony-only call refuses.
            Trigger::Unlock => Return::Matched(quest.is_some_and(|id| colony.is_unlocked(id))),
            Trigger::Reputation { min } => Return::Matched(colony.reputation() >= *min),
            Trigger::Difficulty { difficulty } => {
                Return::Matched(colony.difficulty() == *difficulty)
            }
        }
    }
}

/// The `state` object of a `state`/`citizen` trigger, required by both
/// (`StateQuestTriggerTemplate.java:53`, `CitizenQuestTriggerTemplate.java:54`).
fn state_of<'a>(map: &'a Map<String, Value>, at: &str) -> Result<&'a Value, Refusal> {
    map.get("state").ok_or_else(|| Refusal::MissingKey {
        at: at.to_owned(),
        key: "state",
    })
}

/// The two branches of `CitizenQuestTriggerTemplate.canTriggerQuest`
/// (`:59-91`): the unfiltered one picks among every citizen, the filtered one
/// skips a citizen already on a quest before looking at its compound.
fn citizen_trigger(want: &Option<Value>, avoid: &Option<Value>, colony: &mut dyn Colony) -> Return {
    let pool: Vec<i32> = if want.is_none() && avoid.is_none() {
        // `citizenDataList.get(rand.nextInt(citizenDataList.size()))` (`:64-68`).
        colony.citizens().iter().map(|citizen| citizen.id).collect()
    } else {
        // `hasQuestAssignment()` first (`:74-77`), then `match`, then `notmatch`.
        colony
            .citizens()
            .iter()
            .filter(|citizen| !citizen.on_quest)
            .filter(|citizen| want.as_ref().is_none_or(|w| compound_matches(&citizen.data, w)))
            .filter(|citizen| avoid.as_ref().is_none_or(|a| !compound_matches(&citizen.data, a)))
            .map(|citizen| citizen.id)
            .collect()
    };
    if pool.is_empty() {
        // `return new CitizenTriggerReturnData(null)` — a *negative* answer.
        return Return::Citizen(None);
    }
    // Java shuffles the list and takes the first that qualifies (`:71-89`);
    // drawing uniformly among the qualifying set is the same distribution and
    // makes the rule testable.
    let draw = colony.random_below(pool.len() as i32);
    let index = (draw.max(0) as usize) % pool.len();
    Return::Citizen(Some(pool[index]))
}

/// `StateQuestTriggerTemplate.canTriggerQuest` (`:61-83`): descend the compound
/// along `path`, answer `false` at the first missing step, then match.
fn state_matches(root: &Compound, path: &[String], want: &Value, count: i32) -> bool {
    let mut steps = path.iter();
    let mut current: &Tag = match steps.next() {
        None => return false,
        Some(first) => match root.get(&first.as_str().into()) {
            Some(tag) => tag,
            None => return false,
        },
    };
    for step in steps {
        current = match current {
            Tag::Compound(inner) => match inner.get(&step.as_str().into()) {
                Some(tag) => tag,
                None => return false,
            },
            _ => return false,
        };
    }
    matches(current, want, count)
}

/// `matchNbt(nbtTag, matchTag, count)` (`IQuestTriggerTemplate.java:52-139`): a
/// JSON description is a *partial* pattern over a tag, and `count` is how many
/// list elements must match.
///
/// Three rules a reader is likely to get wrong, all kept:
///
/// - a **number is a floor** (`>=`), so `"level": 3` also matches 5 (`:134-137`);
/// - a **boolean is a `ByteTag` test**, and any non-zero byte is `true`
///   (`:129-132`, written as `(value == 0) != expected`);
/// - matching a **compound only checks the keys the pattern names** — that is what
///   makes `{"type": "minecolonies:townhall"}` a filter over a building record.
pub fn matches(nbt: &Tag, want: &Value, count: i32) -> bool {
    match nbt {
        Tag::Compound(inner) => match want {
            Value::Object(pattern) => object_matches(inner, pattern),
            _ => false,
        },
        Tag::List(items) => {
            // "Find `count` elements of the list that match" (`:78-91`).
            let mut found = 0;
            for item in items {
                if matches(item, want, 1) {
                    found += 1;
                    if found >= count {
                        return true;
                    }
                }
            }
            // Or "each element of the pattern array matches some element" (`:93-115`).
            match want {
                Value::Array(pattern) => pattern
                    .iter()
                    .all(|one| items.iter().any(|item| matches(item, one, 1))),
                _ => false,
            }
        }
        _ => match want {
            Value::String(text) => match nbt {
                Tag::String(found) => found.to_string_lossy() == *text,
                _ => false,
            },
            Value::Bool(expected) => match nbt {
                Tag::Byte(value) => (*value != 0) == *expected,
                _ => false,
            },
            Value::Number(number) => match (numeric(nbt), number.as_f64()) {
                (Some(found), Some(floor)) => found >= floor,
                _ => false,
            },
            _ => false,
        },
    }
}

/// [`matches`] against a compound that is already in hand — the citizen case
/// (`CitizenQuestTriggerTemplate.java:78`), where wrapping it in a `Tag` would
/// mean copying it.
pub fn compound_matches(data: &Compound, want: &Value) -> bool {
    match want {
        Value::Object(pattern) => object_matches(data, pattern),
        _ => false,
    }
}

fn object_matches(data: &Compound, pattern: &Map<String, Value>) -> bool {
    pattern.iter().all(|(key, sub)| {
        data.get(&key.as_str().into())
            .is_some_and(|found| matches(found, sub, 1))
    })
}

/// `NumericTag.getAsDouble()` (`:136`): every numeric width widens.
fn numeric(tag: &Tag) -> Option<f64> {
    match tag {
        Tag::Byte(value) => Some(f64::from(*value)),
        Tag::Short(value) => Some(f64::from(*value)),
        Tag::Int(value) => Some(f64::from(*value)),
        Tag::Long(value) => Some(*value as f64),
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

/// The `triggerOrder` expression of a quest file.
///
/// An absent key is [`Order::All`] (`QuestJsonListener.java:249-268`: every
/// trigger must pass, in file order), and so is a key holding only whitespace —
/// Java splits that into one empty token and builds a tree whose only node is not
/// a trigger index, which throws on the first tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Order {
    /// No expression: every trigger, in file order.
    All,
    /// A parsed expression over 1-based trigger numbers.
    Expr(Node),
}

/// One node of a `triggerOrder` expression.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the operators the builder makes nodes for
pub enum Node {
    /// A trigger index, 0-based after parsing.
    Index(usize),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
}

impl Order {
    /// `parseTriggerOrder` (`:246-391`), with the two refusals Java turns into a
    /// crash: `!` (never evaluated, see the module doc) and an index the trigger
    /// list does not have.
    pub fn parse(text: &str, triggers: usize) -> Result<Order, Refusal> {
        let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if squeezed.is_empty() {
            return Ok(Order::All);
        }
        let tokens = tokenize(&squeezed, text)?;
        let mut parser = Parser {
            tokens,
            at: 0,
            text: text.to_owned(),
        };
        let node = parser.expression()?;
        if parser.at != parser.tokens.len() {
            return Err(Refusal::OrderUnbalanced {
                at: text.to_owned(),
            });
        }
        // The distinct-index count must equal the trigger count (`:283-300`).
        let mut seen: Vec<usize> = Vec::new();
        collect(&node, &mut seen);
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != triggers {
            return Err(Refusal::OrderMismatch {
                at: text.to_owned(),
                mentioned: seen.len(),
                triggers,
            });
        }
        // A number outside `1..=triggers` (`triggerMap.get` would answer null).
        if let Some(bad) = seen.iter().find(|index| **index >= triggers) {
            return Err(Refusal::OrderIndex {
                at: text.to_owned(),
                index: *bad as i32 + 1,
                triggers,
            });
        }
        Ok(Order::Expr(node))
    }

    /// `evaluate` (`:399-453`) for a whole order: `None` when the quest may not
    /// start, the positive return datas otherwise, **in the order Java collects
    /// them** — which is the order the operands are written in, and the order that
    /// decides who is quest giver and who is a participant
    /// (`QuestInstance.java:79-91`).
    pub fn evaluate(
        &self,
        triggers: &[Trigger],
        quest: &Identifier,
        colony: &mut dyn Colony,
    ) -> Option<Vec<Return>> {
        match self {
            Order::All => {
                let mut returns = Vec::with_capacity(triggers.len());
                for trigger in triggers {
                    let answer = trigger.can_trigger(Some(quest), colony);
                    if !answer.is_positive() {
                        return None;
                    }
                    returns.push(answer);
                }
                Some(returns)
            }
            Order::Expr(node) => {
                // Java caches one answer per index (`:445`, `triggerDataCache`), so
                // a trigger written twice is asked once.
                let mut cache: Vec<Option<Return>> = vec![None; triggers.len()];
                evaluate(node, triggers, quest, colony, &mut cache)
            }
        }
    }
}

fn evaluate(
    node: &Node,
    triggers: &[Trigger],
    quest: &Identifier,
    colony: &mut dyn Colony,
    cache: &mut Vec<Option<Return>>,
) -> Option<Vec<Return>> {
    match node {
        Node::Index(index) => {
            let answer = match cache.get(*index).copied().flatten() {
                Some(cached) => cached,
                None => {
                    let answer = triggers.get(*index)?.can_trigger(Some(quest), colony);
                    if let Some(slot) = cache.get_mut(*index) {
                        *slot = Some(answer);
                    }
                    answer
                }
            };
            answer.is_positive().then(|| vec![answer])
        }
        // Both operands are tried left to right, and the left list comes first —
        // the order Java's own builder ends up with for the shipped files, which
        // all parenthesise (`:419-440`).
        Node::And(left, right) => {
            let mut left_returns = evaluate(left, triggers, quest, colony, cache)?;
            let right_returns = evaluate(right, triggers, quest, colony, cache)?;
            left_returns.extend(right_returns);
            Some(left_returns)
        }
        Node::Or(left, right) => match evaluate(left, triggers, quest, colony, cache) {
            Some(left_returns) => Some(left_returns),
            None => evaluate(right, triggers, quest, colony, cache),
        },
    }
}

fn collect(node: &Node, out: &mut Vec<usize>) {
    match node {
        Node::Index(index) => out.push(*index),
        Node::And(left, right) | Node::Or(left, right) => {
            collect(left, out);
            collect(right, out);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Index(usize),
    And,
    Or,
    Not,
    Open,
    Close,
}

fn tokenize(text: &str, original: &str) -> Result<Vec<Token>, Refusal> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_digit() {
            let start = at;
            while at < bytes.len() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            let digits = &text[start..at];
            let number: usize = digits.parse().map_err(|_| Refusal::OrderBadToken {
                at: original.to_owned(),
                token: digits.to_owned(),
            })?;
            // Written 1-based (`triggerMap.put(String.valueOf(i+1), …)`, `:302-306`).
            if number == 0 {
                return Err(Refusal::OrderBadToken {
                    at: original.to_owned(),
                    token: digits.to_owned(),
                });
            }
            tokens.push(Token::Index(number - 1));
            continue;
        }
        match byte {
            b'&' if bytes.get(at + 1) == Some(&b'&') => {
                tokens.push(Token::And);
                at += 2;
            }
            b'|' if bytes.get(at + 1) == Some(&b'|') => {
                tokens.push(Token::Or);
                at += 2;
            }
            b'!' => {
                tokens.push(Token::Not);
                at += 1;
            }
            b'(' => {
                tokens.push(Token::Open);
                at += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                at += 1;
            }
            _ => {
                return Err(Refusal::OrderBadToken {
                    at: original.to_owned(),
                    token: (byte as char).to_string(),
                });
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    text: String,
}

impl Parser {
    /// `expression := term ('||' term)*`.
    fn expression(&mut self) -> Result<Node, Refusal> {
        let mut left = self.term()?;
        while self.peek() == Some(&Token::Or) {
            self.at += 1;
            let right = self.term()?;
            left = Node::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `term := unary ('&&' unary)*` — `&&` binds tighter than `||`, which no
    /// shipped file tests because they all parenthesise.
    fn term(&mut self) -> Result<Node, Refusal> {
        let mut left = self.unary()?;
        while self.peek() == Some(&Token::And) {
            self.at += 1;
            let right = self.unary()?;
            left = Node::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `unary := '!' unary | '(' expression ')' | index`.
    fn unary(&mut self) -> Result<Node, Refusal> {
        match self.next() {
            Some(Token::Not) => Err(Refusal::OrderNotImplemented {
                at: self.text.clone(),
            }),
            Some(Token::Open) => {
                let node = self.expression()?;
                if self.next() != Some(Token::Close) {
                    return Err(Refusal::OrderUnbalanced {
                        at: self.text.clone(),
                    });
                }
                Ok(node)
            }
            Some(Token::Index(index)) => Ok(Node::Index(index)),
            Some(_) | None => Err(Refusal::OrderUnbalanced {
                at: self.text.clone(),
            }),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        if token.is_some() {
            self.at += 1;
        }
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    /// A colony that answers from fields and records what a trigger asked it for.
    #[derive(Default)]
    struct TestColony {
        day: i32,
        reputation: f64,
        difficulty: Difficulty,
        unlocked: Vec<Identifier>,
        citizens: Vec<Citizen>,
        tag: Compound,
        draw: Vec<i32>,
        asked: Vec<i32>,
    }

    impl Colony for TestColony {
        fn day(&self) -> i32 {
            self.day
        }

        fn reputation(&self) -> f64 {
            self.reputation
        }

        fn difficulty(&self) -> Difficulty {
            self.difficulty
        }

        fn is_unlocked(&self, quest: &Identifier) -> bool {
            self.unlocked.contains(quest)
        }

        fn citizens(&self) -> &[Citizen] {
            &self.citizens
        }

        fn tag(&self) -> &Compound {
            &self.tag
        }

        fn random_below(&mut self, bound: i32) -> i32 {
            self.asked.push(bound);
            if self.draw.is_empty() {
                0
            } else {
                self.draw.remove(0)
            }
        }
    }

    impl Default for Difficulty {
        fn default() -> Difficulty {
            Difficulty::Normal
        }
    }

    #[test]
    fn a_zero_rarity_never_draws_and_a_positive_one_draws_once_in_that_many() {
        // `oneInChance > 0 && …` (`RandomQuestTriggerTemplate.java:43`).
        let mut colony = TestColony::default();
        let never = Trigger::Random { rarity: 0 };
        assert_eq!(
            never.can_trigger(None, &mut colony),
            Return::Matched(false)
        );
        assert!(colony.asked.is_empty(), "nextInt(0) throws in Java");

        let one_in_ten_thousand = Trigger::Random { rarity: 10_000 };
        assert_eq!(
            one_in_ten_thousand.can_trigger(None, &mut colony),
            Return::Matched(true),
            "a draw of 0 is below MAX_TICKRATE"
        );
        assert_eq!(colony.asked, vec![10_000], "the rarity is the bound");

        colony.draw = vec![MAX_TICKRATE];
        assert_eq!(
            one_in_ten_thousand.can_trigger(None, &mut colony),
            Return::Matched(false),
            "the comparison is strict, so 500 does not pass"
        );
    }

    #[test]
    fn a_number_in_a_pattern_is_a_floor_and_a_string_is_exact() {
        // `((NumericTag) nbtTag).getAsDouble() >= matchTag.getAsDouble()` (`:134-137`),
        // and `equals` for a string (`:125-128`).
        let level_five = Compound::from([("level".into(), Tag::Int(5))]);
        assert!(compound_matches(&level_five, &want(r#"{"level":3}"#)));
        assert!(!compound_matches(&level_five, &want(r#"{"level":6}"#)));
        assert!(compound_matches(&level_five, &want(r#"{"level":5}"#)));

        let named = Compound::from([("type".into(), Tag::String("minecolonies:townhall".into()))]);
        assert!(compound_matches(&named, &want(r#"{"type":"minecolonies:townhall"}"#)));
        assert!(!compound_matches(&named, &want(r#"{"type":"minecolonies:Townhall"}"#)));
        assert!(
            !compound_matches(&named, &want(r#"{"absent":1}"#)),
            "a compound pattern only checks the keys it names, but it does check them"
        );
    }

    #[test]
    fn a_boolean_pattern_matches_a_nonzero_byte() {
        // `((ByteTag) nbtTag).getAsByte() == 0) != matchTag.getAsBoolean()` (`:129-132`).
        let male = Compound::from([("female".into(), Tag::Byte(0))]);
        let female = Compound::from([("female".into(), Tag::Byte(1))]);
        assert!(compound_matches(&male, &want(r#"{"female":false}"#)));
        assert!(!compound_matches(&male, &want(r#"{"female":true}"#)));
        assert!(compound_matches(&female, &want(r#"{"female":true}"#)));
        // A `ByteTag` is also a `NumericTag`, so a number compares as a floor
        // (`:134-137`): 0 and 1 are both `>= 0`, but only 1 is `>= 1`.
        assert!(compound_matches(&male, &want(r#"{"female":0}"#)));
        assert!(compound_matches(&female, &want(r#"{"female":0}"#)));
        assert!(!compound_matches(&male, &want(r#"{"female":1}"#)));
        assert!(compound_matches(&female, &want(r#"{"female":1}"#)));
    }

    #[test]
    fn a_list_pattern_counts_matches_or_accepts_any_order_of_subset() {
        // Two branches of the list case (`:76-116`).
        let buildings = Tag::List(vec![
            Tag::Compound(Compound::from([
                ("type".into(), Tag::String("minecolonies:builder".into())),
                ("level".into(), Tag::Int(1)),
            ])),
            Tag::Compound(Compound::from([
                ("type".into(), Tag::String("minecolonies:builder".into())),
                ("level".into(), Tag::Int(5)),
            ])),
        ]);
        let at_three = want(r#"{"type":"minecolonies:builder","level":3}"#);
        assert!(
            matches(&buildings, &at_three, 1),
            "the level-5 element answers level 3, because the number is a floor"
        );
        let another_trade = want(r#"{"type":"minecolonies:miner","level":1}"#);
        assert!(!matches(&buildings, &another_trade, 1), "no element is a miner");
        let level_one = want(r#"{"type":"minecolonies:builder","level":1}"#);
        assert!(matches(&buildings, &level_one, 1));
        assert!(
            matches(&buildings, &level_one, 2),
            "both elements reach level 1, because the number is a floor"
        );
        let level_four = want(r#"{"type":"minecolonies:builder","level":4}"#);
        assert!(!matches(&buildings, &level_four, 2), "only one reaches level 4");
        assert!(matches(&buildings, &level_four, 1));
        assert!(matches(&buildings, &level_four, 0), "count 0 is satisfied at once");

        let subset = want(r#"[{"level":1},{"level":5}]"#);
        assert!(matches(&buildings, &subset, 1), "each pattern finds an element");
        let too_many = want(r#"[{"level":1},{"level":9}]"#);
        assert!(!matches(&buildings, &too_many, 1));
    }

    #[test]
    fn the_state_trigger_walks_the_colony_compound_and_stops_at_the_first_missing_step() {
        // `StateQuestTriggerTemplate.java:63-83`.
        let townhall = Compound::from([
            ("type".into(), Tag::String("minecolonies:townhall".into())),
            ("level".into(), Tag::Int(3)),
        ]);
        let mut colony = TestColony::default();
        colony.tag.insert(
            "buildingManager".into(),
            Tag::Compound(Compound::from([("townhall".into(), Tag::Compound(townhall))])),
        );
        let trigger = Trigger::State {
            path: vec!["buildingManager".into(), "townhall".into()],
            want: want(r#"{"level":3}"#),
            count: 1,
        };
        assert_eq!(
            trigger.can_trigger(None, &mut colony),
            Return::Matched(true)
        );
        let deeper = Trigger::State {
            path: vec!["buildingManager".into(), "absent".into()],
            want: want(r#"{}"#),
            count: 1,
        };
        assert_eq!(
            deeper.can_trigger(None, &mut colony),
            Return::Matched(false)
        );
    }

    #[test]
    fn a_citizen_trigger_without_a_pattern_ignores_who_is_already_busy() {
        // `matchTag == null && notMatchTag == null` branch (`:62-69`).
        let mut colony = TestColony::default();
        colony.citizens = vec![
            Citizen {
                id: 4,
                on_quest: true,
                data: Compound::new(),
            },
            Citizen {
                id: 7,
                on_quest: false,
                data: Compound::new(),
            },
        ];
        let trigger = Trigger::Citizen {
            want: None,
            avoid: None,
        };
        assert_eq!(
            trigger.can_trigger(None, &mut colony),
            Return::Citizen(Some(4)),
            "the busy citizen is picked first because nothing filters it"
        );
    }

    #[test]
    fn a_filtered_citizen_trigger_skips_the_busy_one_and_the_notmatch() {
        let mut colony = TestColony::default();
        colony.citizens = vec![
            Citizen {
                id: 4,
                on_quest: true,
                data: Compound::from([("female".into(), Tag::Byte(1))]),
            },
            Citizen {
                id: 7,
                on_quest: false,
                data: Compound::from([("female".into(), Tag::Byte(0))]),
            },
            Citizen {
                id: 9,
                on_quest: false,
                data: Compound::from([("female".into(), Tag::Byte(1))]),
            },
        ];
        let free_female = Trigger::Citizen {
            want: Some(want(r#"{"female":true}"#)),
            avoid: None,
        };
        assert_eq!(
            free_female.can_trigger(None, &mut colony),
            Return::Citizen(Some(9)),
            "4 is busy and 7 is male"
        );
        let not_female = Trigger::Citizen {
            want: None,
            avoid: Some(want(r#"{"female":true}"#)),
        };
        assert_eq!(
            not_female.can_trigger(None, &mut colony),
            Return::Citizen(Some(7))
        );
        let nobody = Trigger::Citizen {
            want: Some(want(r#"{"female":true}"#)),
            avoid: Some(want(r#"{"female":true}"#)),
        };
        assert_eq!(
            nobody.can_trigger(None, &mut colony),
            Return::Citizen(None),
            "no citizen can both match and avoid the same pattern"
        );
        assert!(!Return::Citizen(None).is_positive(), ":26-29");
    }

    #[test]
    fn the_unlock_trigger_answers_false_to_the_colony_only_call() {
        // `:32-41`.
        let mut colony = TestColony::default();
        colony.unlocked.push(id("minecolonies:questtemplate"));
        let trigger = Trigger::Unlock;
        assert_eq!(
            trigger.can_trigger(None, &mut colony),
            Return::Matched(false),
            "the one-argument overload hardcodes false"
        );
        assert_eq!(
            trigger.can_trigger(Some(&id("minecolonies:questtemplate")), &mut colony),
            Return::Matched(true)
        );
        assert_eq!(
            trigger.can_trigger(Some(&id("minecolonies:other")), &mut colony),
            Return::Matched(false)
        );
    }

    #[test]
    fn reputation_is_a_floor_not_an_equality() {
        // `getReputation() >= minQuantity` (`QuestReputationTriggerTemplate.java:40`).
        let mut colony = TestColony::default();
        colony.reputation = 0.5;
        let half = Trigger::Reputation { min: 0.5 };
        assert_eq!(half.can_trigger(None, &mut colony), Return::Matched(true));
        let more = Trigger::Reputation { min: 0.75 };
        assert_eq!(more.can_trigger(None, &mut colony), Return::Matched(false));
    }

    #[test]
    fn an_unknown_difficulty_name_is_refused_and_the_known_ones_compare_exactly() {
        // `Difficulty.valueOf(text)` (`WorldDifficultyTriggerTemplate.java:35`).
        let at = "triggers[0]";
        let good = want(r#"{"type":"minecolonies:difficulty","difficulty":"HARD"}"#);
        let trigger = Trigger::from_json(&good, at).expect("a known name");
        let mut colony = TestColony {
            difficulty: Difficulty::Hard,
            ..TestColony::default()
        };
        assert_eq!(trigger.can_trigger(None, &mut colony), Return::Matched(true));
        colony.difficulty = Difficulty::Easy;
        assert_eq!(
            trigger.can_trigger(None, &mut colony),
            Return::Matched(false)
        );

        let low = want(r#"{"type":"minecolonies:difficulty","difficulty":"hard"}"#);
        assert_eq!(
            Trigger::from_json(&low, at),
            Err(Refusal::UnknownType {
                at: "triggers[0].difficulty".into(),
                kind: "difficulty",
                text: "hard".into()
            })
        );
        assert_eq!(Difficulty::from_name("NORMAL"), Some(Difficulty::Normal));
    }

    #[test]
    fn an_absent_order_requires_every_trigger_and_keeps_the_file_order() {
        // `parseTriggerOrder`'s default branch (`:249-268`).
        let mut colony = TestColony::default();
        let triggers = [
            Trigger::Reputation { min: 0.0 },
            Trigger::Reputation { min: 1.0 },
        ];
        let order = Order::parse("", triggers.len()).expect("the default is All");
        assert_eq!(order, Order::All);
        assert_eq!(
            order.evaluate(&triggers, &id("q:a"), &mut colony),
            None,
            "the second trigger refuses and Java returns null"
        );

        colony.reputation = 2.0;
        assert_eq!(
            order.evaluate(&triggers, &id("q:a"), &mut colony),
            Some(vec![Return::Matched(true), Return::Matched(true)])
        );
    }

    #[test]
    fn a_parenthesised_order_picks_the_branch_that_answers() {
        // The `||` then `&&` cases of `evaluate` (`:403-441`).
        let mut colony = TestColony::default();
        // Two triggers, because an order has to name exactly as many distinct
        // indices as the file has triggers (`:296-300`).
        let triggers = [
            Trigger::Reputation { min: 10.0 }, // 0: refuses
            Trigger::Difficulty {
                difficulty: Difficulty::Normal,
            }, // 1: passes
        ];
        let either = Order::parse("1 || 2", 2).expect("a valid order");
        assert_eq!(
            either.evaluate(&triggers, &id("q:a"), &mut colony),
            Some(vec![Return::Matched(true)]),
            "the left operand refuses, the right one passes, and only its list is kept"
        );

        let both = Order::parse("1 && 2", 2).expect("a valid order");
        assert_eq!(
            both.evaluate(&triggers, &id("q:a"), &mut colony),
            None,
            "an AND with one refusing side is null"
        );

        let branch = Order::parse("(1 || 2) && 2", 2).expect("a valid order");
        assert_eq!(
            branch.evaluate(&triggers, &id("q:a"), &mut colony),
            Some(vec![Return::Matched(true), Return::Matched(true)]),
            "the left branch answers with 2, then 2 is asked again from the cache"
        );
    }

    #[test]
    fn not_is_refused_because_java_never_evaluates_it() {
        // `QuestParsingConstants.NOT` (`:26`) has no case in `evaluate`
        // (`:401-452`), so the node would look up `triggerMap.get("!")`.
        assert_eq!(
            Order::parse("1 && !2", 2),
            Err(Refusal::OrderNotImplemented {
                at: "1 && !2".into()
            })
        );
    }

    #[test]
    fn the_distinct_index_count_must_match_the_trigger_count() {
        // `types.size() != triggers.size()` (`:296-300`).
        assert_eq!(
            Order::parse("1 || 1", 2),
            Err(Refusal::OrderMismatch {
                at: "1 || 1".into(),
                mentioned: 1,
                triggers: 2
            })
        );
        assert!(Order::parse("1 || 1", 1).is_ok(), "one trigger, one index");
        assert_eq!(
            Order::parse("1 || 2", 3),
            Err(Refusal::OrderMismatch {
                at: "1 || 2".into(),
                mentioned: 2,
                triggers: 3
            })
        );
    }

    #[test]
    fn an_index_the_file_does_not_have_and_a_stray_character_are_both_refused() {
        assert_eq!(
            Order::parse("1 && 2", 1),
            Err(Refusal::OrderMismatch {
                at: "1 && 2".into(),
                mentioned: 2,
                triggers: 1
            }),
            "the count is checked before the range, like `:296-300`"
        );
        assert!(matches!(
            Order::parse("1 & 2", 2),
            Err(Refusal::OrderBadToken { .. })
        ));
        assert!(matches!(
            Order::parse("(1 || 2", 2),
            Err(Refusal::OrderUnbalanced { .. })
        ));
    }

    #[test]
    fn a_trigger_written_twice_is_asked_once() {
        // `triggerDataCache.computeIfAbsent` (`:445`).
        let mut colony = TestColony::default();
        colony.reputation = 1.0;
        let triggers = [Trigger::Reputation { min: 0.0 }];
        let order = Order::parse("1 && 1", 1).expect("one distinct index");
        assert!(order.evaluate(&triggers, &id("q:a"), &mut colony).is_some());
        assert_eq!(colony.asked.len(), 0, "the reputation trigger does not draw");
        // A trigger that draws is the observable case.
        let drawing = [Trigger::Random { rarity: 1000 }];
        let order = Order::parse("1 && 1", 1).expect("one distinct index");
        assert!(order.evaluate(&drawing, &id("q:a"), &mut colony).is_some());
        assert_eq!(colony.asked, vec![1000], "asked once, not twice");
    }
}
