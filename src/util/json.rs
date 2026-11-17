//! `GsonHelper`: reading a JSON field with a default, and the two places Java
//! throws where a mod cannot.
//!
//! Java reference: `core/util/GsonHelper.java` (45 lines). It extends vanilla's
//! `net.minecraft.util.GsonHelper` with three overloads so that a caller can write
//! `getAsString(object, "name", () -> something(arg))` instead of repeating the
//! `object.has(key) ? ... : default` dance. That is the whole file.
//!
//! # The mapping, and why it is not just `Value::get`
//!
//! | Java | Here |
//! | --- | --- |
//! | `getAsString(object, key, Supplier)` (`:21-24`) | [`string_or`] |
//! | `getAsString(object, key, Function, arg)` (`:16-19`) | the same — the `arg` exists only to make a Java lambda |
//! | `getAsResourceLocation(object, key)` (`:26-29`) | [`identifier_or`] with no default |
//! | `getAsResourceLocation(object, key, default)` (`:31-34`) | [`identifier_or`] with one |
//! | `getAsJsonArray(object, key, Supplier)` (`:41-44`) | [`array_or`] |
//!
//! The two `(…, Function<T, …>, T arg)` overloads are not a different rule: they
//! are there because Java cannot capture `arg` in a `Supplier` without a lambda,
//! and every call site passes `()` -> f(arg) anyway. In Rust the closure captures
//! what it needs, so one function per rule.
//!
//! # Two exceptions a mod cannot throw
//!
//! Vanilla's `convertToString` throws a `JsonSyntaxException` when a field is
//! present and is an object or an array, and `new ResourceLocation(String)` throws
//! when the text is not a valid id. Both are unchecked, both are caught by nobody
//! upstream, and neither has a counterpart in a wasm mod — a panic in a data-pack
//! reader takes the whole load down with no message a pack author can read.
//!
//! So the port **names the refusal instead**:
//!
//! - a field of the wrong shape falls back to the default, and the doc says which
//!   case that is ([`string_or`], [`array_or`]);
//! - a malformed id is an `Err` carrying the text that failed ([`identifier_or`]),
//!   because a wrong id is a data-pack bug the author must see, and *that* one has
//!   no sane default.
//!
//! `MANQUANT.md` §2 records that `GsonHelper` itself needs nothing more than
//! `serde_json`, which the API already re-exports (`mod_api::serde_json`).

use mod_api::Identifier;
use mod_api::serde_json::Value;

/// A JSON field was present but unusable.
// exhaustive: the one rule that refuses rather than defaults
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonRefusal {
    /// `key` holds `text`, which is not a registry id
    /// (`ResourceLocation`'s constructor, thrown at `GsonHelper.java:28`).
    NotAnIdentifier {
        /// The field this came from, for a message that names it.
        key: String,
        /// What the file actually said.
        text: String,
    },
}

impl JsonRefusal {
    pub fn reason(&self) -> String {
        match self {
            JsonRefusal::NotAnIdentifier { key, text } => {
                format!("{key}: {text:?} is not a registry id")
            }
        }
    }
}

/// A string field, or the default: `getAsString` (`GsonHelper.java:21-24`).
///
/// ```java
/// return object.has(key) ? convertToString(object.get(key), key) : defaultValue.get();
/// ```
///
/// The default is a **closure**, not a value: Java's `Supplier` exists so that a
/// caller does not pay for a default it will not use, and the same holds here.
///
/// Four outcomes, and the last one is the port's:
///
/// - the key is absent → the default (`object.has(key)` is false,
///   `GsonHelper.java:23`);
/// - the key holds a **string** → that string, exactly (`Value::as_str`);
/// - the key holds a **number or a boolean** → its text, which is what
///   `JsonPrimitive.getAsString` answers for a primitive;
/// - the key holds an **object, an array or a JSON `null`** → the default. Java
///   throws on all three: `object.has(key)` is **true** for a `null` value
///   (`GsonHelper.java:23` tests the member, not the value), a `JsonNull` is not
///   a `JsonPrimitive`, and vanilla's `convertToString` throws a
///   `JsonSyntaxException` for anything that is not one — an object and an array
///   take the same path.
///
/// So the divergence is **three shapes, not one** (`null`, array, object), and it
/// is a choice rather than an oversight: a data pack that writes `"name": null`
/// gets the default here and a dead load in Java. There is nobody to catch the
/// exception, and falling back is what the whole function is for.
pub fn string_or(object: &Value, key: &str, default: impl FnOnce() -> String) -> String {
    match object.get(key) {
        None | Some(Value::Null) => default(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Array(_) | Value::Object(_)) => default(),
    }
}

/// A registry id field, with an optional default: `getAsResourceLocation`
/// (`GsonHelper.java:26-34`).
///
/// ```java
/// return new ResourceLocation(getAsString(object, key));                                    // :28
/// return Optional.ofNullable(getAsString(object, key, (String) null)).map(ResourceLocation::new).orElse(defaultValue); // :33
/// ```
///
/// Both overloads are this one with `default` present or not, and the difference
/// that matters is the **null default**: `getAsString(object, key, (String) null)`
/// answers `null` for an absent key, `Optional.ofNullable` then gives an empty
/// `Optional`, and the field is simply missing. `Ok(None)` is that case.
///
/// That is the null **argument**, not a null **value**: a field written as
/// `"id": null` is *present*, `object.has(key)` is true (`GsonHelper.java:23`), and
/// Java throws in `convertToString` rather than taking the default. The port
/// answers `Ok(default)` there, the same fallback [`string_or`] takes and for the
/// same reason, and the shape it refuses with an `Err` is the one thing with no
/// sane default: an id that is not an id.
///
/// `Err` when the key is present and its text is not an id — `new ResourceLocation`
/// would have thrown. The id is checked with `mod_api::Identifier::parse`, the same
/// test the engine applies, so a name that passes here is one the game can resolve.
///
/// **A bare path is namespaced, not refused.** `new ResourceLocation("planks")`
/// gives `minecraft:planks`, and `Identifier::parse("planks")` does the same
/// (`horizon-core/src/identifier.rs:56-67`: no `:`, so the namespace is
/// `minecraft`). The two agree, and that is why `planks` comes back as
/// `minecraft:planks` instead of an `Err`. What is refused is a **character** that
/// cannot appear in an id — `not an id!` has a space and a `!`
/// (`identifier.rs:31-33`: lowercase letters, digits, `_`, `.`, `-`, and `/` in the
/// path) — and an empty path, which `from_parts` accepts but no registry key can be.
pub fn identifier_or(
    object: &Value,
    key: &str,
    default: Option<&str>,
) -> Result<Option<String>, JsonRefusal> {
    let text = match object.get(key) {
        None | Some(Value::Null) => return Ok(default.map(str::to_owned)),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        // Java reaches `new ResourceLocation(convertToString(...))` and throws
        // there; the shape is the same refusal.
        Some(Value::Array(_) | Value::Object(_)) => String::new(),
    };
    match Identifier::parse(&text) {
        Ok(id) if !id.path().is_empty() => Ok(Some(id.to_string())),
        _ => Err(JsonRefusal::NotAnIdentifier {
            key: key.to_owned(),
            text,
        }),
    }
}

/// An array field, or the default: `getAsJsonArray` (`GsonHelper.java:41-44`).
///
/// ```java
/// return object.has(key) ? convertToJsonArray(object.get(key), key) : defaultValue.get();
/// ```
///
/// Same three outcomes as [`string_or`] and the same single divergence: a field
/// that is present and **not** an array falls back to the default where Java throws
/// a `JsonSyntaxException`. An empty array is an array — Java does not treat it as
/// absent, and neither does this.
pub fn array_or<'a>(object: &'a Value, key: &str, default: &'a [Value]) -> &'a [Value] {
    match object.get(key) {
        Some(Value::Array(items)) => items,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::serde_json::from_str;

    /// A parsed literal, the way the rest of the port reads JSON
    /// (`mod_api::serde_json::from_str`, `recipes_content/excludes.rs:41`).
    fn parse(literal: &str) -> Value {
        from_str(literal).expect("a test literal is JSON")
    }

    fn document() -> Value {
        parse(
            r#"{
                "name": "baker",
                "count": 3,
                "ratio": 0.5,
                "enabled": true,
                "id": "minecolonies:baker",
                "tags": ["job=baker", "tag=oven"],
                "nested": {"a": 1},
                "nothing": null
            }"#,
        )
    }

    #[test]
    fn a_present_string_is_returned_as_it_is() {
        assert_eq!(string_or(&document(), "name", || "x".to_owned()), "baker");
    }

    /// The default is only called when it is used — that is why it is a closure.
    #[test]
    fn an_absent_key_answers_the_default_and_an_absent_build_is_never_paid_for() {
        let mut called = 0;
        let value = string_or(&document(), "missing", || {
            called += 1;
            "fallback".to_owned()
        });
        assert_eq!(value, "fallback");
        assert_eq!(called, 1);

        let value = string_or(&document(), "name", || {
            called += 1;
            "unused".to_owned()
        });
        assert_eq!(value, "baker");
        assert_eq!(called, 1, "the default must not be built when the key is there");
    }

    /// Numbers and booleans stringify, like `JsonElement.getAsString` on a
    /// primitive.
    #[test]
    fn a_primitive_is_read_as_its_text() {
        assert_eq!(string_or(&document(), "count", || "x".to_owned()), "3");
        assert_eq!(string_or(&document(), "enabled", || "x".to_owned()), "true");
        // A float keeps the form serde_json gives it.
        assert_eq!(string_or(&document(), "ratio", || "x".to_owned()), "0.5");
    }

    /// The port's one divergence: an object where a string was expected falls back
    /// instead of throwing.
    #[test]
    fn a_wrong_shape_falls_back_instead_of_throwing() {
        assert_eq!(string_or(&document(), "nested", || "x".to_owned()), "x");
        assert_eq!(string_or(&document(), "tags", || "x".to_owned()), "x");
        assert_eq!(string_or(&document(), "nothing", || "x".to_owned()), "x");
    }

    #[test]
    fn a_valid_id_is_normalised() {
        assert_eq!(
            identifier_or(&document(), "id", None),
            Ok(Some("minecolonies:baker".to_owned()))
        );
    }

    /// A bare path takes the vanilla namespace, exactly as
    /// `new ResourceLocation("planks")` does — Java and `Identifier::parse` agree
    /// on this, so it is not a divergence and not a refusal either.
    #[test]
    fn a_bare_path_takes_the_vanilla_namespace() {
        let bare = parse(r#"{"id": "planks"}"#);
        assert_eq!(
            identifier_or(&bare, "id", None),
            Ok(Some("minecraft:planks".to_owned()))
        );
    }

    /// An absent field answers the default, or nothing when there is no default.
    #[test]
    fn an_absent_id_answers_the_default_or_nothing() {
        assert_eq!(identifier_or(&document(), "missing", None), Ok(None));
        assert_eq!(
            identifier_or(&document(), "missing", Some("minecraft:stone")),
            Ok(Some("minecraft:stone".to_owned()))
        );
        // An explicit null is an absent field, like Java's `Optional.ofNullable`.
        assert_eq!(identifier_or(&document(), "nothing", None), Ok(None));
    }

    /// A malformed id is refused **with the text**, because a data-pack author has
    /// to see what they wrote.
    #[test]
    fn a_malformed_id_is_refused_with_its_text() {
        let bad = parse(r#"{"id": "not an id!"}"#);
        assert_eq!(
            identifier_or(&bad, "id", None),
            Err(JsonRefusal::NotAnIdentifier {
                key: "id".to_owned(),
                text: "not an id!".to_owned(),
            })
        );
        let refusal = identifier_or(&bad, "id", None).expect_err("a malformed id is refused");
        assert!(refusal.reason().contains("not an id!"));
    }

    /// A wrong shape is the same refusal, with an empty text rather than a panic.
    #[test]
    fn a_wrong_shape_for_an_id_is_refused_not_defaulted() {
        assert!(matches!(
            identifier_or(&document(), "nested", Some("minecraft:stone")),
            Err(JsonRefusal::NotAnIdentifier { .. })
        ));
    }

    /// An array is returned whole, and an empty one is still an array.
    #[test]
    fn an_array_is_returned_whole() {
        let document = document();
        let tags = array_or(&document, "tags", &[]);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str(), Some("job=baker"));

        let empty = parse(r#"{"tags": []}"#);
        let fallback = [parse(r#""no""#)];
        assert!(array_or(&empty, "tags", &fallback).is_empty());
    }

    /// A missing or wrongly-shaped field falls back to the caller's slice.
    #[test]
    fn a_missing_or_wrong_shape_answers_the_default_slice() {
        let document = document();
        let fallback = [parse(r#""default""#)];
        assert_eq!(array_or(&document, "missing", &fallback).len(), 1);
        assert_eq!(array_or(&document, "name", &fallback).len(), 1);
        assert_eq!(array_or(&document, "nested", &fallback).len(), 1);
    }
}
