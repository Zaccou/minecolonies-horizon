//! The seven reads a quest file needs, and the one refusal type every parser in
//! this module returns.
//!
//! Java reference: the `JsonObject` calls of
//! `core/datalistener/QuestJsonListener.java:122-243`, of the seven objective
//! parsers (`core/quests/objectives/*.java:76-118` for the two block ones, `:97`
//! delivery, `:74` research, `:122` building), of the eight reward parsers
//! (`core/quests/rewards/*.java:34-58`) and of the six triggers
//! (`core/quests/triggers/*.java:26-58`). Every one of those is
//! `json.get(key).getAsInt()`-shaped: **it throws**, and the throw is caught once
//! per file by `apply` (`:113-116`), which logs "Skipping quest: <id>" and moves
//! on. A refusal here carries the path inside the file, which is the part the
//! Java's `NullPointerException` message never says.
//!
//! | Java | Here | Difference |
//! | --- | --- | --- |
//! | `get(key).getAsInt()` | [`integer`] | Gson coerces `"7"`; a string is refused |
//! | `get(key).getAsDouble()` | [`number`] | same |
//! | `get(key).getAsBoolean()` | [`boolean`] | Gson coerces `"true"`; refused |
//! | `has(key) ? get(key) : default` | [`optional`] | same |
//! | `get(key).getAsJsonArray()` | [`array`] | same |
//! | `getAsJsonObject(DETAILS_KEY)` | [`object`] | a missing `details` is refused by name |
//!
//! **The one coercion the port keeps.** Gson's `getAsInt` on a JSON number with a
//! fraction truncates; [`integer`] refuses it, because a quest file that says
//! `"qty": 2.5` meant something and no shipped file does. That is a refusal where
//! Java silently proceeds with `2`, and it is the only one.

use mod_api::serde_json::{Map, Value};
use mod_api::Identifier;

use crate::inventory::ItemName;

/// Where a read happened, for the refusal: `objectives[2].details`.
///
/// Java has no such string — the whole file is one `try` block — so the port
/// builds one as it descends. [`field`] and [`element`] are the two steps.
pub fn field(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

/// `objectives[2]`, from `objectives` and `2`.
pub fn element(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

/// What a quest file got wrong. One variant per shape of failure, `at` is the
/// path inside the file, and no variant carries a Java throwable: the Java catches
/// `Exception` and logs it (`QuestJsonListener.java:113-116`), so a message is all
/// that ever reaches a player or a pack author.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
// exhaustive: every way one of the seven reads can fail
pub enum Refusal {
    /// The key is not there. Java's `NullPointerException` on `get(key).getAs…`.
    MissingKey { at: String, key: &'static str },
    /// The key is there and is not a JSON object.
    NotObject { at: String },
    /// The key is there and has another kind than the reader wanted.
    WrongType {
        at: String,
        key: &'static str,
        expected: &'static str,
    },
    /// A `ResourceLocation` the engine would not accept
    /// (`new ResourceLocation(text)` throws on a bad charset, `:204`).
    BadIdentifier {
        at: String,
        key: &'static str,
        text: String,
    },
    /// A `type` the registry does not hold — Java's `getValue(new ResourceLocation(type))`
    /// returns `null` and the following `produce` throws (`:133`, `:148`, `:191`,
    /// `:135`). `kind` names which registry: `"objective"`, `"reward"`,
    /// `"trigger"`, `"answer"`.
    UnknownType {
        at: String,
        kind: &'static str,
        text: String,
    },
    /// An item name the engine cannot parse (`ItemName::new`, `Quad`), where Java
    /// builds an `ItemStack` of registry `air` through `getHolder(..).get()`.
    BadItem {
        at: String,
        key: &'static str,
        text: String,
    },
    /// A character of a `triggerOrder` that is not a digit, `&&`, `||`, `!`, `(`
    /// or `)`.
    OrderBadToken { at: String, token: String },
    /// A `triggerOrder` that runs out of operands or leaves a brace open —
    /// Java's index arithmetic (`QuestJsonListener.java:316-382`) walks off the
    /// array instead.
    OrderUnbalanced { at: String },
    /// `!` in a `triggerOrder`: declared (`QuestParsingConstants.java:26`),
    /// built (`:335-338`), and never given a case in `evaluate` (`:401-452`), so
    /// no file can use it.
    OrderNotImplemented { at: String },
    /// The expression names a trigger the file does not have.
    OrderIndex {
        at: String,
        index: i32,
        triggers: usize,
    },
    /// The number of *distinct* indices does not equal the number of triggers
    /// (`:283-300`), which is the only sanity check Java makes on an order.
    OrderMismatch {
        at: String,
        mentioned: usize,
        triggers: usize,
    },
    /// A `skill` no `Skill` constant answers (`Skill.valueOf`,
    /// `SkillRewardTemplate.java:53`).
    UnknownSkill { at: String, text: String },
    /// An objective index the template does not have — `QuestInstance.java:227`
    /// indexes the list and throws.
    NoSuchObjective { at: String, index: i32 },
}

impl Refusal {
    /// The path inside the file, which is the part a pack author needs.
    pub fn at(&self) -> &str {
        match self {
            Refusal::MissingKey { at, .. }
            | Refusal::NotObject { at }
            | Refusal::WrongType { at, .. }
            | Refusal::BadIdentifier { at, .. }
            | Refusal::UnknownType { at, .. }
            | Refusal::BadItem { at, .. } => at,
            Refusal::OrderBadToken { at, .. }
            | Refusal::OrderUnbalanced { at }
            | Refusal::OrderNotImplemented { at }
            | Refusal::OrderIndex { at, .. }
            | Refusal::OrderMismatch { at, .. }
            | Refusal::UnknownSkill { at, .. }
            | Refusal::NoSuchObjective { at, .. } => at,
        }
    }
}

/// The JSON object under `value`, or [`Refusal::NotObject`]. Java's
/// `getAsJsonObject` answers an *empty* object for a non-object and `null` for a
/// missing key; the port refuses both, because a `details` block that is not an
/// object is a typo and an empty one silently makes every later read fail.
pub fn object<'a>(value: &'a Value, at: &str) -> Result<&'a Map<String, Value>, Refusal> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Refusal::NotObject { at: at.to_owned() }),
    }
}

/// The key's value when it is present.
pub fn optional<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key)
}

/// A string, required.
pub fn text<'a>(
    map: &'a Map<String, Value>,
    at: &str,
    key: &'static str,
) -> Result<&'a str, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey {
            at: at.to_owned(),
            key,
        }),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(Refusal::WrongType {
            at: at.to_owned(),
            key,
            expected: "a string",
        }),
    }
}

/// A whole number, required. A fractional number is refused rather than
/// truncated, which is the one coercion Java makes and this does not.
pub fn integer(map: &Map<String, Value>, at: &str, key: &'static str) -> Result<i32, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey {
            at: at.to_owned(),
            key,
        }),
        Some(value) => match value.as_i64() {
            Some(whole) => i32::try_from(whole).map_err(|_| Refusal::WrongType {
                at: at.to_owned(),
                key,
                expected: "a 32-bit whole number",
            }),
            None => Err(Refusal::WrongType {
                at: at.to_owned(),
                key,
                expected: "a whole number",
            }),
        },
    }
}

/// A number, required: `getAsDouble`, which the reputation reward and the
/// reputation trigger read (`QuestReputationRewardTemplate.java:37`,
/// `QuestReputationTriggerTemplate.java:34`).
pub fn number(map: &Map<String, Value>, at: &str, key: &'static str) -> Result<f64, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey {
            at: at.to_owned(),
            key,
        }),
        Some(value) => value.as_f64().ok_or(Refusal::WrongType {
            at: at.to_owned(),
            key,
            expected: "a number",
        }),
    }
}

/// A boolean, required — `count-existing` is the only one in the pack
/// (`BuildBuildingObjectiveTemplate.java:131`).
pub fn boolean(map: &Map<String, Value>, at: &str, key: &'static str) -> Result<bool, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey {
            at: at.to_owned(),
            key,
        }),
        Some(value) => value.as_bool().ok_or(Refusal::WrongType {
            at: at.to_owned(),
            key,
            expected: "a boolean",
        }),
    }
}

/// The array under `key`, required.
pub fn array<'a>(
    map: &'a Map<String, Value>,
    at: &str,
    key: &'static str,
) -> Result<&'a Vec<Value>, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey {
            at: at.to_owned(),
            key,
        }),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(Refusal::WrongType {
            at: at.to_owned(),
            key,
            expected: "an array",
        }),
    }
}

/// A `ResourceLocation` (`new ResourceLocation(text)`, `:204`).
pub fn identifier(
    map: &Map<String, Value>,
    at: &str,
    key: &'static str,
) -> Result<Identifier, Refusal> {
    let text = text(map, at, key)?;
    Identifier::parse(text).map_err(|_| Refusal::BadIdentifier {
        at: at.to_owned(),
        key,
        text: text.to_owned(),
    })
}

/// The same, for a value already in hand (a parent, which is a bare string in
/// the `parents` array, `:204`).
pub fn identifier_of(value: &Value, at: &str) -> Result<Identifier, Refusal> {
    match value {
        Value::String(text) => Identifier::parse(text).map_err(|_| Refusal::BadIdentifier {
            at: at.to_owned(),
            key: "parents",
            text: text.clone(),
        }),
        _ => Err(Refusal::WrongType {
            at: at.to_owned(),
            key: "parents",
            expected: "a string",
        }),
    }
}

/// An item name, as `new ItemStack(ForgeRegistries.ITEMS…getValue(id))` spells
/// it (`ItemRewardTemplate.java:46`, `DeliveryObjectiveTemplateTemplate.java:102`).
pub fn item_name(
    map: &Map<String, Value>,
    at: &str,
    key: &'static str,
) -> Result<ItemName, Refusal> {
    let text = text(map, at, key)?;
    ItemName::new(text).ok_or_else(|| Refusal::BadItem {
        at: at.to_owned(),
        key,
        text: text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("the literal parses")
    }

    #[test]
    fn a_missing_key_names_the_path_and_the_key() {
        let value = of(r#"{"details":{}}"#);
        let map = object(&value, "q").expect("an object");
        let details = object(map.get("details").expect("present"), "q.details").expect("object");
        assert_eq!(
            integer(details, "q.details", "qty"),
            Err(Refusal::MissingKey {
                at: "q.details".into(),
                key: "qty"
            })
        );
        assert_eq!(text(details, "q.details", "block").unwrap_err().at(), "q.details");
    }

    #[test]
    fn a_string_where_a_number_belongs_is_refused_and_not_coerced() {
        // Gson's `getAsInt` answers 7 for "7"; the port refuses
        // (`QuestJsonParser.java:80` and its siblings).
        let value = of(r#"{"qty":"7","fraction":2.5}"#);
        let map = object(&value, "q").unwrap();
        assert!(matches!(
            integer(map, "q", "qty"),
            Err(Refusal::WrongType { expected: "a whole number", .. })
        ));
        assert!(matches!(
            integer(map, "q", "fraction"),
            Err(Refusal::WrongType { .. })
        ));
        assert_eq!(number(map, "q", "fraction"), Ok(2.5));
    }

    #[test]
    fn the_paths_read_like_the_file() {
        assert_eq!(field("", "objectives"), "objectives");
        assert_eq!(field("objectives", "details"), "objectives.details");
        assert_eq!(element("objectives", 2), "objectives[2]");
        assert_eq!(field(&element("objectives", 2), "details"), "objectives[2].details");
    }

    #[test]
    fn an_identifier_and_an_item_name_each_refuse_what_the_engine_would_not_parse() {
        let value = of(r#"{"block":"minecraft:stone","bad":"Minecraft:Stone"}"#);
        let map = object(&value, "q").unwrap();
        assert_eq!(
            identifier(map, "q", "block").map(|id| id.to_string()),
            Ok("minecraft:stone".to_owned())
        );
        assert!(matches!(
            identifier(map, "q", "bad"),
            Err(Refusal::BadIdentifier { .. })
        ));
        assert!(matches!(
            item_name(map, "q", "block").map(|n| n.to_string()).as_deref(),
            Ok("minecraft:stone")
        ));
    }

    #[test]
    fn a_boolean_and_an_array_keep_their_shapes() {
        let value = of(r#"{"count-existing":true,"parents":["a:b"],"n":3}"#);
        let map = object(&value, "q").unwrap();
        assert_eq!(boolean(map, "q", "count-existing"), Ok(true));
        assert_eq!(array(map, "q", "parents").map(Vec::len), Ok(1));
        assert!(matches!(
            array(map, "q", "n"),
            Err(Refusal::WrongType { expected: "an array", .. })
        ));
    }
}
