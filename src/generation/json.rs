//! The four shapes of JSON value a data generator writes, and the two mutation
//! helpers the loot model needs.
//!
//! Java builds its output with `Gson` (`DataGeneratorConstants.java:10` — a
//! pretty-printing, HTML-escaping-off `Gson`) and hands the result to
//! `DataProvider.saveStable`, which sorts the keys and writes two spaces of
//! indentation. The two facts that matter here are the sort and the *types*: a
//! `ConstantValue.exactly(1)` is written `1.0` and a Java `int` is written `1`,
//! and a reader can tell them apart (`data/minecolonies/loot_table/blocks/
//! blockhutbaker.json` says `"rolls": 1.0`, `data/minecolonies/recipe/
//! baked_bread.json` says `"cookingtime": 300`). So the generator keeps the two
//! numbers apart too: [`f`] for a float, [`i`] for an integer.
//!
//! Key order is *not* reproduced: `saveStable` sorts keys, and the comparison
//! the tests do is on parsed values, where `serde_json`'s map is already sorted.
//! Ordering inside an **array** is meaningful and is preserved — a loot pool
//! rolls its entries in the order they are written.

use mod_api::serde_json::{Number, Value};

pub type Json = Value;

/// A string value.
pub fn s(text: &str) -> Json {
    Value::String(text.to_owned())
}

/// A Java `int`: written without a decimal point, and not equal to [`f`] of the
/// same value (`300` ≠ `300.0` in JSON).
pub fn i(value: i64) -> Json {
    Value::Number(Number::from(value))
}

/// A Java `float`/`double`: written with a decimal point. `NaN` and the
/// infinities have no JSON spelling and become `null` rather than panicking —
/// none of them is reachable from a rule below.
pub fn f(value: f64) -> Json {
    match Number::from_f64(value) {
        Some(number) => Value::Number(number),
        None => Value::Null,
    }
}

/// An object, from its pairs, in the order given (the sort happens on write).
pub fn obj(pairs: Vec<(&str, Json)>) -> Json {
    let mut map = mod_api::serde_json::Map::new();
    for (key, value) in pairs {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// An array.
pub fn arr(items: Vec<Json>) -> Json {
    Value::Array(items)
}

/// `set(entry, "weight", …)` — replace one field of an object.
pub fn set(entry: &mut Json, key: &str, value: Json) {
    if let Value::Object(map) = entry {
        map.insert(key.to_owned(), value);
    }
}

/// `push(entry, "functions", …)` — append to an array field, creating it the
/// first time. A loot entry has one `functions` list and one `conditions` list
/// however many times a rule adds to them (`DefaultBlockLootTableProvider.java:81`
/// adds a condition *and* a function to the same entry).
pub fn push(entry: &mut Json, key: &str, value: Json) {
    if let Value::Object(map) = entry {
        match map.entry(key.to_owned()) {
            mod_api::serde_json::map::Entry::Occupied(mut slot) => {
                if let Value::Array(items) = slot.get_mut() {
                    items.push(value);
                } else {
                    slot.insert(arr(vec![value]));
                }
            }
            mod_api::serde_json::map::Entry::Vacant(slot) => {
                slot.insert(arr(vec![value]));
            }
        }
    }
}

/// `minecolonies:<path>`, which is how the generator spells its own ids.
pub fn own(path: &str) -> String {
    format!("minecolonies:{path}")
}

/// `minecraft:<path>`, for the vanilla item, block and table a rule names.
pub fn vanilla(path: &str) -> String {
    format!("minecraft:{path}")
}

/// The namespace of an id, without the path.
pub fn namespace_of(id: &str) -> &str {
    match id.split_once(':') {
        Some((namespace, _)) => namespace,
        None => "minecraft",
    }
}

/// The path of an id, without the namespace.
pub fn path_of(id: &str) -> &str {
    match id.split_once(':') {
        Some((_, path)) => path,
        None => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_integer_and_a_float_of_the_same_value_are_not_equal() {
        // The whole reason the module has two constructors: `saveStable` writes
        // a Java int as `300` and a float as `300.0`, and the pack carries
        // both (`recipe/baked_bread.json` vs `loot_table/blocks/bell_pepper.json`).
        assert_ne!(i(1), f(1.0));
        assert_eq!(i(1).to_string(), "1");
        assert_eq!(f(1.0).to_string(), "1.0");
        assert_eq!(f(1000.0).to_string(), "1000.0");
    }

    #[test]
    fn a_float_keeps_every_digit_it_was_given() {
        // `0.5714286F` is written verbatim by Gson; any rounding here would
        // change the fortune formula of every crop table.
        assert_eq!(f(0.5714286).to_string(), "0.5714286");
        assert_eq!(f(0.0475).to_string(), "0.0475");
        assert_eq!(f(0.025).to_string(), "0.025");
    }

    #[test]
    fn push_creates_the_list_then_extends_it() {
        let mut entry = obj(vec![("type", s("minecraft:item"))]);
        push(&mut entry, "functions", s("first"));
        push(&mut entry, "functions", s("second"));
        assert_eq!(
            entry["functions"].to_string(),
            "[\"first\",\"second\"]",
            "two calls append to one list"
        );
    }

    #[test]
    fn set_replaces_without_duplicating_the_key() {
        let mut entry = obj(vec![("weight", i(3))]);
        set(&mut entry, "weight", i(5));
        assert_eq!(entry.as_object().map(|map| map.len()), Some(1));
        assert_eq!(entry["weight"], i(5));
    }

    #[test]
    fn an_id_splits_the_way_the_registry_does() {
        assert_eq!(namespace_of("minecolonies:blocks/x"), "minecolonies");
        assert_eq!(path_of("minecolonies:blocks/x"), "blocks/x");
        assert_eq!(namespace_of("stone"), "minecraft", "an id with no colon");
        assert_eq!(path_of("stone"), "stone");
    }
}
