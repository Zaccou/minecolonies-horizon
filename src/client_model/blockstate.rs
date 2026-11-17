//! Reading one blockstate file, and turning a state into the models it draws.
//!
//! Java reference: `assets/minecolonies/blockstates/*.json` as vanilla's
//! `BlockStateModelLoader` reads them. The rules the port reproduces:
//!
//! | Rule | Where it comes from |
//! | --- | --- |
//! | a file has `variants` **or** `multipart`, and nothing else is a blockstate | `BlockStateModelLoader.loadBlockState` |
//! | a variant key names *some* properties; the ones it omits match every value | the wiki and 10 of this mod's own files, see [`Definition::coverage`] |
//! | `x`, `y`, `z` are degrees, multiples of 90, taken modulo 360 | `Variant::decode` |
//! | `uvlock` defaults to `false`, `weight` to `1`, and a weight of 0 is refused | `VariantSet::decode` |
//! | a `when` is a property test, `OR`, or `AND`; a term may be `a\|b` or `!a` | `Condition::decode` |
//! | the first variant selector that matches a state wins | `Definition::bind` |
//! | a weighted variant is one model picked by weight, not a list drawn together | [`Selection::roll`] |
//!
//! **Why the port has its own reader.** The game reads these files itself
//! (`crates/horizon-assets/src/blockstate.rs` in the Horizon tree); a wasm mod
//! cannot call it and cannot open `assets/`. What this module adds is the shape a
//! mod needs: a `Result` that names what is wrong instead of a log line, the
//! coverage of a file against a block's declared properties, and a roll the
//! caller supplies so a weighted pick is testable.
//!
//! **Two differences with the game's reader, both on purpose.**
//!
//! 1. `BlockStateDefinition::bind` in `horizon-assets` stops a selector's walk at
//!    the first state it would overwrite, which makes an overlapping file behave
//!    differently depending on state order. This reader takes the first matching
//!    selector and offers [`Definition::overlaps`] so a caller can refuse the file
//!    instead of inheriting that order. No file of this mod overlaps;
//!    `overlaps` returns empty on all 95 (see `variants.rs`).
//! 2. A condition that is a one-key object named `OR` or `AND` is taken as such
//!    and refused if its value is not a list of conditions. The game's reader
//!    falls back to reading it as a property test, which turns a typo into a
//!    condition that never matches.
//!
//! **What this file does not do**: it draws nothing, and it never asks the game
//! whether a model exists — [`Selection`] names model ids, the caller resolves
//! them.

use mod_api::serde_json::{Map, Value};

use crate::client_model::variants::Prop;

/// A model plus the turns and the UV lock a variant gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three fields, all read by the caller
pub struct Variant {
    /// The model id, as the file spells it: `minecolonies:block/blockhutbuilder`.
    pub model: String,
    /// Quarter turns about X, Y and Z, taken modulo four.
    pub turns: [u8; 3],
    /// Whether the textures stay still as the model turns.
    pub uvlock: bool,
}

/// What one selector applies: one model, or several picked by weight.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two shapes, and `weight` only exists in the second
pub enum VariantSet {
    /// `{"model": …}` - the common case, 87 of the 88 block files.
    Single(Variant),
    /// `[{"model": …, "weight": 40}, …]` - the rack and the citizen hut.
    Weighted(Vec<(Variant, u16)>),
}

impl VariantSet {
    /// Every variant of the set, with its weight (1 for a [`VariantSet::Single`]).
    pub fn entries(&self) -> Vec<(&Variant, u16)> {
        match self {
            VariantSet::Single(variant) => vec![(variant, 1)],
            VariantSet::Weighted(entries) => entries.iter().map(|(v, w)| (v, *w)).collect(),
        }
    }

    /// The sum of the weights: what a roll divides by.
    pub fn total_weight(&self) -> u64 {
        self.entries().iter().map(|(_, w)| u64::from(*w)).sum()
    }
}

/// One alternative inside a property test: `a`, or `!a` (`Term.negated`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two fields, both compared
pub struct Term {
    /// The value the test names.
    pub value: String,
    /// Whether the test is `!value`, which matches every *other* value.
    pub negated: bool,
}

/// A `when`: a property test, or `OR`/`AND` over tests.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three shapes, and `matches` has a branch per shape
pub enum Condition {
    /// `{"OR": [ … ]}` - any of them.
    Or(Vec<Condition>),
    /// `{"AND": [ … ]}` - all of them.
    And(Vec<Condition>),
    /// `{"north": "true", "facing": "east|west"}` - all of them, in name order.
    Properties(Vec<(String, Vec<Term>)>),
}

impl Condition {
    /// Whether a state satisfies the test. A property the state lacks never
    /// matches, except through a negated term, which matches every other value
    /// including absence - vanilla's `!a` is "not a", not "has a value that is
    /// not a".
    pub fn matches(&self, state: &[(String, String)]) -> bool {
        match self {
            Condition::Or(parts) => parts.iter().any(|part| part.matches(state)),
            Condition::And(parts) => parts.iter().all(|part| part.matches(state)),
            Condition::Properties(tests) => tests.iter().all(|(name, terms)| {
                let value = state
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.as_str())
                    .unwrap_or("");
                terms
                    .iter()
                    .any(|term| (value == term.value.as_str()) ^ term.negated)
            }),
        }
    }
}

/// One `multipart` case: an optional `when` and what it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two fields, both read
pub struct Selector {
    /// The `when`, or `None` for a case that always applies.
    pub when: Option<Condition>,
    /// The model, or the weighted models, the case draws.
    pub apply: VariantSet,
}

impl Selector {
    /// Whether the case applies to a state: no `when` always does.
    pub fn applies(&self, state: &[(String, String)]) -> bool {
        match &self.when {
            None => true,
            Some(condition) => condition.matches(state),
        }
    }
}

/// A variant key: `facing=east,variant=0perc`, or `north=true|false`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one field, and an empty one matches every state
pub struct Key {
    /// `(name, values)` per clause. Empty for the `""` key, which matches any
    /// state - `composted_dirt`, `blockwaypoint` and both banners use it.
    pub clauses: Vec<(String, Vec<String>)>,
}

impl Key {
    /// Whether the key names a state: every clause must match, and a clause the
    /// state does not carry never does. **A property the key omits is not
    /// tested**, which is the whole partial-key rule.
    pub fn matches(&self, state: &[(String, String)]) -> bool {
        self.clauses.iter().all(|(name, values)| {
            state
                .iter()
                .find(|(key, _)| key == name)
                .is_some_and(|(_, value)| values.iter().any(|named| named == value))
        })
    }

    /// The property names the key mentions, sorted.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.clauses.iter().map(|(name, _)| name.as_str()).collect();
        names.sort_unstable();
        names
    }
}

/// A blockstate file, as far as the port needs it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: a file may carry either half, and vanilla lets it carry both
pub struct Definition {
    /// The `variants` map, in file order.
    pub variants: Vec<(Key, VariantSet)>,
    /// The `multipart` list, in file order.
    pub multipart: Vec<Selector>,
}

/// What a state draws: one model, or one part per matching `multipart` case.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two fields, both read by the caller
pub struct Selection<'a> {
    /// The parts, in the order the file gives them.
    pub parts: Vec<Part<'a>>,
    /// Which half of the file answered.
    pub from: Origin,
}

/// Which half of a blockstate answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two shapes
pub enum Origin {
    /// A `variants` selector.
    Variants,
    /// The `multipart` cases.
    Multipart,
}

/// One model a state draws, borrowed from the definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four fields, all read
pub struct Part<'a> {
    /// The model id.
    pub model: &'a str,
    /// Quarter turns about X, Y and Z.
    pub turns: [u8; 3],
    /// Whether the textures stay still.
    pub uvlock: bool,
    /// The weight the part was named with; 1 outside a weighted set.
    pub weight: u16,
}

/// What a blockstate file got wrong. Every variant names its place in the file
/// and none carries a Java throwable: the game logs a line and draws nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one variant per way a file can be refused
pub enum Refusal {
    /// The text is not JSON.
    NotJson { line: usize, column: usize },
    /// The root is not an object.
    NotAnObject { at: &'static str },
    /// Neither `variants` nor `multipart`: the seven item models in
    /// `blockstates/` are exactly this.
    NoDefinition,
    /// `"variants": {}` - vanilla refuses an empty map.
    EmptyVariants,
    /// `"multipart": []`.
    EmptyMultipart,
    /// A key the file needs is not there.
    MissingKey { at: &'static str, key: &'static str },
    /// A key holds the wrong JSON type.
    WrongType {
        at: &'static str,
        key: &'static str,
        expected: &'static str,
    },
    /// A model id that is not `namespace:path` or `path`.
    BadModelId { model: String },
    /// `"x": 45` - a quarter turn the game cannot take.
    RotationNotQuarterTurn { axis: &'static str, degrees: i64 },
    /// `"weight": 0`.
    BadWeight { weight: i64 },
    /// A variant key that is not `name=value` pairs.
    BadKey { text: String },
    /// A `when` that is empty, or names `OR`/`AND` with a non-list value.
    BadCondition { at: &'static str },
    /// A property term that is empty, as in `"facing": "east|"`.
    EmptyTerm { property: String },
    /// A state no selector names. Zero occurrences in this mod, which is the
    /// result [`Definition::coverage`] exists to prove.
    NoVariant { state: String },
    /// A key naming a clause that is not `name=value`.
    NotAPair { clause: String },
}

impl Refusal {
    /// A stable name, namespaced, for a log line or a test.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::NotJson { .. } => "model.refusal.not_json",
            Refusal::NotAnObject { .. } => "model.refusal.not_an_object",
            Refusal::NoDefinition => "model.refusal.no_definition",
            Refusal::EmptyVariants => "model.refusal.empty_variants",
            Refusal::EmptyMultipart => "model.refusal.empty_multipart",
            Refusal::MissingKey { .. } => "model.refusal.missing_key",
            Refusal::WrongType { .. } => "model.refusal.wrong_type",
            Refusal::BadModelId { .. } => "model.refusal.bad_model_id",
            Refusal::RotationNotQuarterTurn { .. } => "model.refusal.not_quarter_turn",
            Refusal::BadWeight { .. } => "model.refusal.bad_weight",
            Refusal::BadKey { .. } => "model.refusal.bad_key",
            Refusal::BadCondition { .. } => "model.refusal.bad_condition",
            Refusal::EmptyTerm { .. } => "model.refusal.empty_term",
            Refusal::NoVariant { .. } => "model.refusal.no_variant",
            Refusal::NotAPair { .. } => "model.refusal.not_a_pair",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::NotJson { line, column } => write!(f, "{} at {line}:{column}", self.name()),
            Refusal::NotAnObject { at } => write!(f, "{} at {at}", self.name()),
            Refusal::MissingKey { at, key } => write!(f, "{} at {at}.{key}", self.name()),
            Refusal::WrongType { at, key, expected } => {
                write!(f, "{} at {at}.{key}, wanted {expected}", self.name())
            }
            Refusal::BadModelId { model } => write!(f, "{}: {model}", self.name()),
            Refusal::RotationNotQuarterTurn { axis, degrees } => {
                write!(f, "{}: {axis}={degrees}", self.name())
            }
            Refusal::BadWeight { weight } => write!(f, "{}: {weight}", self.name()),
            Refusal::BadKey { text } => write!(f, "{}: {text}", self.name()),
            Refusal::BadCondition { at } => write!(f, "{} at {at}", self.name()),
            Refusal::EmptyTerm { property } => write!(f, "{}: {property}", self.name()),
            Refusal::NoVariant { state } => write!(f, "{}: {state}", self.name()),
            Refusal::NotAPair { clause } => write!(f, "{}: {clause}", self.name()),
            Refusal::NoDefinition | Refusal::EmptyVariants | Refusal::EmptyMultipart => {
                f.write_str(self.name())
            }
        }
    }
}

impl std::error::Error for Refusal {}

// --- reading ----------------------------------------------------------------

fn object<'a>(value: &'a Value, at: &'static str) -> Result<&'a Map<String, Value>, Refusal> {
    value.as_object().ok_or(Refusal::NotAnObject { at })
}

fn text<'a>(
    map: &'a Map<String, Value>,
    at: &'static str,
    key: &'static str,
) -> Result<&'a str, Refusal> {
    match map.get(key) {
        None => Err(Refusal::MissingKey { at, key }),
        Some(value) => value.as_str().ok_or(Refusal::WrongType {
            at,
            key,
            expected: "a string",
        }),
    }
}

fn flag(map: &Map<String, Value>, _at: &'static str, key: &'static str) -> Option<bool> {
    map.get(key).and_then(Value::as_bool)
}

fn quarter_turns(map: &Map<String, Value>) -> Result<[u8; 3], Refusal> {
    let mut turns = [0u8; 3];
    for (index, axis) in ["x", "y", "z"].into_iter().enumerate() {
        let Some(value) = map.get(axis) else {
            continue;
        };
        let degrees = value.as_i64().ok_or(Refusal::WrongType {
            at: "variant",
            key: axis,
            expected: "an integer",
        })?;
        let degrees = degrees.rem_euclid(360);
        if degrees % 90 != 0 {
            return Err(Refusal::RotationNotQuarterTurn { axis, degrees });
        }
        turns[index] = (degrees / 90) as u8;
    }
    Ok(turns)
}

/// A model id, as `Identifier` takes it: `path`, or `namespace:path`, with no
/// capitals and no empty part. Refusing here is what stops a file that says
/// `Minecolonies:Block/X` from baking as a missing model with no reason.
fn check_model_id(model: &str) -> Result<(), Refusal> {
    let (namespace, path) = match model.split_once(':') {
        Some((namespace, path)) => (Some(namespace), path),
        None => (None, model),
    };
    let ok_part = |part: &str, extra: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(c))
    };
    let namespace_ok = namespace.is_none_or(|part| ok_part(part, "_.-"));
    if !namespace_ok || !ok_part(path, "_./-") {
        return Err(Refusal::BadModelId {
            model: model.to_owned(),
        });
    }
    Ok(())
}

fn parse_variant(value: &Value) -> Result<Variant, Refusal> {
    let map = object(value, "variant")?;
    let model = text(map, "variant", "model")?;
    check_model_id(model)?;
    Ok(Variant {
        model: model.to_owned(),
        turns: quarter_turns(map)?,
        uvlock: flag(map, "variant", "uvlock").unwrap_or(false),
    })
}

fn parse_variant_set(value: &Value) -> Result<VariantSet, Refusal> {
    let Some(values) = value.as_array() else {
        return parse_variant(value).map(VariantSet::Single);
    };
    if values.is_empty() {
        return Err(Refusal::EmptyVariants);
    }
    let mut entries = Vec::with_capacity(values.len());
    let mut total: i64 = 0;
    for value in values {
        let map = object(value, "variant")?;
        let weight = match map.get("weight") {
            None => 1,
            Some(weight) => weight.as_i64().ok_or(Refusal::WrongType {
                at: "variant",
                key: "weight",
                expected: "an integer",
            })?,
        };
        if weight <= 0 {
            return Err(Refusal::BadWeight { weight });
        }
        total = total
            .checked_add(weight)
            .ok_or(Refusal::BadWeight { weight: total })?;
        entries.push((parse_variant(value)?, weight as u16));
    }
    Ok(VariantSet::Weighted(entries))
}

fn parse_condition(value: &Value) -> Result<Condition, Refusal> {
    let map = object(value, "when")?;
    if map.is_empty() {
        return Err(Refusal::BadCondition { at: "when" });
    }
    if map.len() == 1 {
        if let Some(list) = map.get("OR") {
            return Ok(Condition::Or(parse_conditions(list)?));
        }
        if let Some(list) = map.get("AND") {
            return Ok(Condition::And(parse_conditions(list)?));
        }
    }
    let mut tests = Vec::with_capacity(map.len());
    for (name, value) in map {
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            _ => {
                return Err(Refusal::WrongType {
                    at: "when",
                    key: "a property",
                    expected: "a string, a boolean or an integer",
                });
            }
        };
        let terms = terms_of(name, &text)?;
        tests.push((name.clone(), terms));
    }
    Ok(Condition::Properties(tests))
}

fn parse_conditions(value: &Value) -> Result<Vec<Condition>, Refusal> {
    let Some(list) = value.as_array() else {
        return Err(Refusal::BadCondition { at: "when" });
    };
    let mut out = Vec::with_capacity(list.len());
    for entry in list {
        out.push(parse_condition(entry)?);
    }
    Ok(out)
}

fn terms_of(property: &str, text: &str) -> Result<Vec<Term>, Refusal> {
    let mut terms = Vec::new();
    for piece in text.split('|') {
        let (negated, value) = match piece.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, piece),
        };
        if value.is_empty() {
            return Err(Refusal::EmptyTerm {
                property: property.to_owned(),
            });
        }
        terms.push(Term {
            value: value.to_owned(),
            negated,
        });
    }
    Ok(terms)
}

/// Split a variant key: `a=b|c,d=e`.
fn parse_key(text: &str) -> Result<Key, Refusal> {
    if text.is_empty() {
        return Ok(Key {
            clauses: Vec::new(),
        });
    }
    let mut clauses = Vec::new();
    for clause in text.split(',') {
        let Some((name, values)) = clause.split_once('=') else {
            return Err(Refusal::NotAPair {
                clause: clause.to_owned(),
            });
        };
        if name.is_empty() {
            return Err(Refusal::BadKey {
                text: text.to_owned(),
            });
        }
        let mut named = Vec::new();
        for value in values.split('|') {
            if value.is_empty() {
                return Err(Refusal::EmptyTerm {
                    property: name.to_owned(),
                });
            }
            named.push(value.to_owned());
        }
        clauses.push((name.to_owned(), named));
    }
    Ok(Key { clauses })
}

impl Definition {
    /// Read a blockstate file.
    pub fn parse(text: &str) -> Result<Definition, Refusal> {
        let value: Value =
            mod_api::serde_json::from_str(text).map_err(|error| Refusal::NotJson {
                line: error.line(),
                column: error.column(),
            })?;
        let root = value
            .as_object()
            .ok_or(Refusal::NotAnObject { at: "root" })?;
        let mut definition = Definition::default();
        if let Some(variants) = root.get("variants") {
            let map = object(variants, "variants")?;
            if map.is_empty() {
                return Err(Refusal::EmptyVariants);
            }
            for (key, value) in map {
                definition
                    .variants
                    .push((parse_key(key)?, parse_variant_set(value)?));
            }
        }
        if let Some(multipart) = root.get("multipart") {
            let list = multipart.as_array().ok_or(Refusal::WrongType {
                at: "root",
                key: "multipart",
                expected: "a list",
            })?;
            if list.is_empty() {
                return Err(Refusal::EmptyMultipart);
            }
            for case in list {
                let map = object(case, "multipart")?;
                let when = match map.get("when") {
                    None => None,
                    Some(condition) => Some(parse_condition(condition)?),
                };
                let apply = map.get("apply").ok_or(Refusal::MissingKey {
                    at: "multipart",
                    key: "apply",
                })?;
                definition.multipart.push(Selector {
                    when,
                    apply: parse_variant_set(apply)?,
                });
            }
        }
        if definition.variants.is_empty() && definition.multipart.is_empty() {
            return Err(Refusal::NoDefinition);
        }
        Ok(definition)
    }

    /// The models a state draws.
    ///
    /// A `variants` map answers with its **first** matching key, as the game's
    /// `bind` does; a `multipart` file answers with every case whose `when`
    /// matches, in file order. A file may carry both, and a `variants` key wins
    /// over the list, which is also the game's order.
    pub fn resolve(&self, state: &[(String, String)]) -> Result<Selection<'_>, Refusal> {
        if let Some((_, set)) = self.variants.iter().find(|(key, _)| key.matches(state)) {
            return Ok(Selection {
                parts: set
                    .entries()
                    .into_iter()
                    .map(|(variant, weight)| Part {
                        model: variant.model.as_str(),
                        turns: variant.turns,
                        uvlock: variant.uvlock,
                        weight,
                    })
                    .collect(),
                from: Origin::Variants,
            });
        }
        let parts: Vec<Part<'_>> = self
            .multipart
            .iter()
            .filter(|case| case.applies(state))
            .flat_map(|case| case.apply.entries())
            .map(|(variant, weight)| Part {
                model: variant.model.as_str(),
                turns: variant.turns,
                uvlock: variant.uvlock,
                weight,
            })
            .collect();
        if parts.is_empty() {
            return Err(Refusal::NoVariant {
                state: state_text(state),
            });
        }
        Ok(Selection {
            parts,
            from: Origin::Multipart,
        })
    }

    /// Every state that **two** keys of a `variants` map name. Empty for all 95
    /// files of this mod; non-empty is the case the game answers by state order
    /// and this module refuses to guess.
    pub fn overlaps(&self, states: &[Vec<(String, String)>]) -> Vec<String> {
        let mut out = Vec::new();
        for state in states {
            let count = self
                .variants
                .iter()
                .filter(|(key, _)| key.matches(state))
                .count();
            if count > 1 {
                out.push(state_text(state));
            }
        }
        out
    }

    /// How many of a block's states the file names, out of how many there are.
    ///
    /// This is the measurement the table records as `covered`: it enumerates the
    /// product of the property domains and asks [`Definition::resolve`] for each.
    pub fn coverage(&self, props: &[Prop]) -> Coverage {
        let states = all_states(props);
        let total = states.len() as u32;
        let mut covered = 0;
        for state in &states {
            if self.resolve(state).is_ok() {
                covered += 1;
            }
        }
        Coverage { total, covered }
    }
}

/// How many states a file names, out of how many the block has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two counts, both read
pub struct Coverage {
    /// The number of states the block declares: the product of its domains.
    pub total: u32,
    /// The number at least one selector names.
    pub covered: u32,
}

impl<'a> Selection<'a> {
    /// The part a weighted roll lands on.
    ///
    /// The game rolls once per draw and never asks twice; the port takes the roll
    /// so a test can name it. `roll` is compared against the cumulative weights,
    /// so `roll` in `0..=sum-1` always answers and anything above refuses.
    pub fn roll(&self, roll: u64) -> Option<Part<'a>> {
        let total: u64 = self.parts.iter().map(|part| u64::from(part.weight)).sum();
        if roll >= total {
            return None;
        }
        let mut seen = 0u64;
        for part in &self.parts {
            seen += u64::from(part.weight);
            if roll < seen {
                return Some(*part);
            }
        }
        None
    }
}

/// The states of a block, in the order the properties are declared: the product
/// of the domains, last property varying fastest.
pub fn all_states(props: &[Prop]) -> Vec<Vec<(String, String)>> {
    let mut states: Vec<Vec<(String, String)>> = vec![Vec::new()];
    for prop in props {
        let mut next = Vec::with_capacity(states.len() * prop.values.len());
        for state in &states {
            for value in prop.values {
                let mut copy = state.clone();
                copy.push((prop.name.to_owned(), (*value).to_owned()));
                next.push(copy);
            }
        }
        states = next;
    }
    states
}

/// A state as `/setblock` spells it: `facing=east,variant=0perc`, properties in
/// name order, empty for a block with none.
pub fn state_text(state: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = state.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::with_capacity(state.len() * 16);
    for (index, (name, value)) in sorted.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(name);
        out.push('=');
        out.push_str(value);
    }
    out
}

/// A state from a `facing=east,hinge=left` literal, for a caller that has the
/// string the game gives (`BlockStateParser`).
pub fn state_of(text: &str) -> Result<Vec<(String, String)>, Refusal> {
    let key = parse_key(text)?;
    Ok(key
        .clauses
        .into_iter()
        .filter_map(|(name, mut values)| {
            if values.len() != 1 {
                return None;
            }
            Some((name, values.remove(0)))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_model::variants::{AGE7, BOOL2, FACING4, HALF2, HINGE2, MOISTURE8};

    /// `assets/minecolonies/blockstates/blockpostbox.json`, verbatim.
    const POSTBOX: &str = r#"{
      "variants": {
        "facing=east": { "model": "minecolonies:block/blockpostbox", "y": 270 },
        "facing=south": { "model": "minecolonies:block/blockpostbox", "y": 0 },
        "facing=west": { "model": "minecolonies:block/blockpostbox", "y": 90 },
        "facing=north": { "model": "minecolonies:block/blockpostbox", "y": 180 }
      }
    }"#;

    /// `barrel_block.json`, two of its thirty-two keys.
    const BARREL: &str = r#"{"variants":{
      "facing=east,variant=0perc":{"model":"minecolonies:block/barrel_block/0perc_crate","y":270},
      "facing=east,variant=done":[{"model":"minecolonies:block/barrel_block/100perc_crate","y":270,"weight":3},
                                    {"model":"minecolonies:block/barrel_block/100perc_crate","y":270}]
    }}"#;

    /// `colonysign.json`: two keys, eight states - `connected` is the only
    /// property the file names.
    const SIGN: &str = r#"{"variants":{
      "connected=false":{"model":"minecolonies:block/colonysign"},
      "connected=true":{"model":"minecolonies:block/colonysign2"}
    }}"#;

    fn state(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn value_of(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    fn condition_of(text: &str) -> Condition {
        parse_condition(&value_of(text)).expect("a valid condition")
    }

    fn one(text: &str, pairs: &[(&str, &str)]) -> Part<'static> {
        // Leaks a little: a test only, and it keeps `Part`'s lifetime honest.
        let definition = Box::leak(Box::new(Definition::parse(text).expect("valid")));
        definition.resolve(&state(pairs)).expect("a model").parts[0]
    }

    #[test]
    fn a_facing_file_answers_a_quarter_turn_per_facing() {
        let definition = Definition::parse(POSTBOX).expect("valid");
        for (facing, turn) in [("east", 3), ("south", 0), ("west", 1), ("north", 2)] {
            let selection = definition
                .resolve(&state(&[("facing", facing)]))
                .expect("a model");
            assert_eq!(selection.from, Origin::Variants);
            assert_eq!(selection.parts.len(), 1);
            assert_eq!(selection.parts[0].turns, [0, turn, 0]);
            assert_eq!(selection.parts[0].model, "minecolonies:block/blockpostbox");
        }
    }

    #[test]
    fn the_order_of_a_key_s_terms_does_not_matter() {
        // `barrel_block.json` writes `facing=east,variant=0perc`; a state carries
        // its properties in declaration order, which is not the key's order.
        let definition = Definition::parse(BARREL).expect("valid");
        let a = definition
            .resolve(&state(&[("facing", "east"), ("variant", "0perc")]))
            .expect("a model");
        let b = definition
            .resolve(&state(&[("variant", "0perc"), ("facing", "east")]))
            .expect("a model");
        assert_eq!(a, b);
        assert_eq!(
            a.parts[0].model,
            "minecolonies:block/barrel_block/0perc_crate"
        );
    }

    #[test]
    fn a_key_that_omits_a_property_matches_every_value_of_it() {
        // The rule ten files rely on: `colonysign` names `connected` only, so all
        // four facings resolve.
        let definition = Definition::parse(SIGN).expect("valid");
        for facing in FACING4 {
            let selection = definition
                .resolve(&state(&[("facing", facing), ("connected", "true")]))
                .expect("a model");
            assert_eq!(selection.parts[0].model, "minecolonies:block/colonysign2");
        }
    }

    #[test]
    fn the_empty_key_matches_a_block_with_no_properties_and_any_state_otherwise() {
        let definition =
            Definition::parse(r#"{"variants":{"":{"model":"minecolonies:block/composteddirt"}}}"#)
                .expect("valid");
        assert!(definition.resolve(&[]).is_ok());
        assert!(
            definition
                .resolve(&state(&[("facing", "north"), ("anything", "1")]))
                .is_ok()
        );
    }

    #[test]
    fn a_partial_key_coverage_is_the_product_of_the_properties_it_omits() {
        // `colonysign`: facing (4) x connected (2) = 8 states, and two keys name
        // them all because `facing` is a wildcard.
        let definition = Definition::parse(SIGN).expect("valid");
        let coverage = definition.coverage(&[
            Prop {
                name: "facing",
                values: FACING4,
            },
            Prop {
                name: "connected",
                values: BOOL2,
            },
        ]);
        assert_eq!(
            coverage,
            Coverage {
                total: 8,
                covered: 8
            }
        );
        // Drop `facing` from the block and the same file still covers everything.
        let coverage = definition.coverage(&[Prop {
            name: "connected",
            values: BOOL2,
        }]);
        assert_eq!(
            coverage,
            Coverage {
                total: 2,
                covered: 2
            }
        );
    }

    #[test]
    fn a_gate_file_covers_all_sixty_four_states_with_eight_keys() {
        // The finding, as a measurement: `gate_wood` names `hinge` and `facing`,
        // so `open`, `powered` and `half` are wildcards - and the open gate draws
        // the closed model.
        let gate = r#"{"variants":{
          "hinge=left,facing=east":{"model":"minecolonies:block/gate/gate_wood","y":90},
          "hinge=left,facing=south":{"model":"minecolonies:block/gate/gate_wood","y":0},
          "hinge=left,facing=west":{"model":"minecolonies:block/gate/gate_wood","y":90},
          "hinge=left,facing=north":{"model":"minecolonies:block/gate/gate_wood","y":0},
          "hinge=right,facing=east":{"model":"minecolonies:block/gate/gate_wood","y":90},
          "hinge=right,facing=south":{"model":"minecolonies:block/gate/gate_wood","y":0},
          "hinge=right,facing=west":{"model":"minecolonies:block/gate/gate_wood","y":90},
          "hinge=right,facing=north":{"model":"minecolonies:block/gate/gate_wood","y":0}
        }}"#;
        let definition = Definition::parse(gate).expect("valid");
        let props = [
            Prop {
                name: "facing",
                values: FACING4,
            },
            Prop {
                name: "hinge",
                values: HINGE2,
            },
            Prop {
                name: "open",
                values: BOOL2,
            },
            Prop {
                name: "powered",
                values: BOOL2,
            },
            Prop {
                name: "half",
                values: HALF2,
            },
        ];
        assert_eq!(
            definition.coverage(&props),
            Coverage {
                total: 64,
                covered: 64
            }
        );
        // And opening it changes nothing about the model, which is what the
        // wildcard means.
        let closed = definition
            .resolve(&state(&[
                ("facing", "north"),
                ("hinge", "left"),
                ("open", "false"),
            ]))
            .expect("a model");
        let opened = definition
            .resolve(&state(&[
                ("facing", "north"),
                ("hinge", "left"),
                ("open", "true"),
            ]))
            .expect("a model");
        assert_eq!(closed, opened);
    }

    #[test]
    fn a_state_no_key_names_is_refused_by_name() {
        let definition = Definition::parse(POSTBOX).expect("valid");
        let error = definition
            .resolve(&state(&[("facing", "up")]))
            .expect_err("no key names up");
        assert_eq!(error.name(), "model.refusal.no_variant");
        assert_eq!(error.to_string(), "model.refusal.no_variant: facing=up");
    }

    #[test]
    fn a_weighted_set_is_one_pick_not_two_parts() {
        let definition = Definition::parse(BARREL).expect("valid");
        let selection = definition
            .resolve(&state(&[("facing", "east"), ("variant", "done")]))
            .expect("a model");
        assert_eq!(selection.from, Origin::Variants);
        assert_eq!(selection.parts.len(), 2, "two weighted entries");
        // 3 + 1: the first entry wins rolls 0 to 2, the second roll 3.
        assert_eq!(selection.roll(0).map(|part| part.weight), Some(3));
        assert_eq!(selection.roll(2).map(|part| part.weight), Some(3));
        assert_eq!(selection.roll(3).map(|part| part.weight), Some(1));
        assert_eq!(selection.roll(4), None, "a roll past the total");
        let single = VariantSet::Weighted(vec![(
            parse_variant(&value_of(r#"{"model":"minecolonies:block/x"}"#)).expect("valid"),
            3,
        )]);
        assert_eq!(single.total_weight(), 3);
    }

    #[test]
    fn a_citizen_hut_picks_one_of_five_with_equal_weights() {
        let hut = r#"{"variants":{"facing=north":[
            {"model":"minecolonies:block/blockhutcitizen","y":90,"weight":20},
            {"model":"minecolonies:block/blockhutcitizen_1","y":90,"weight":20},
            {"model":"minecolonies:block/blockhutcitizen_2","y":90,"weight":20},
            {"model":"minecolonies:block/blockhutcitizen_3","y":90,"weight":20},
            {"model":"minecolonies:block/blockhutcitizen_4","y":90,"weight":20}
        ]}}"#;
        let definition = Definition::parse(hut).expect("valid");
        let selection = definition
            .resolve(&state(&[("facing", "north")]))
            .expect("a model");
        assert_eq!(selection.parts.len(), 5);
        let picked: Vec<&str> = [0, 20, 79, 80, 99]
            .iter()
            .filter_map(|roll| selection.roll(*roll).map(|part| part.model))
            .collect();
        assert_eq!(picked.len(), 5);
        assert_eq!(picked[0], "minecolonies:block/blockhutcitizen");
        assert_eq!(picked[2], "minecolonies:block/blockhutcitizen_3");
        assert_eq!(picked[4], "minecolonies:block/blockhutcitizen_4");
    }

    #[test]
    fn a_multipart_file_draws_every_case_that_applies() {
        // `blockconstructiontape.json`, mostly verbatim: a centre, four arms and
        // four corners.
        let tape = r#"{"multipart":[
          {"apply":{"model":"minecolonies:block/blockconstructiontape_center"}},
          {"when":{"north":"true"},"apply":{"model":"minecolonies:block/blockconstructiontape","uvlock":true}},
          {"when":{"east":"true"},"apply":{"model":"minecolonies:block/blockconstructiontape","y":90,"uvlock":true}},
          {"when":{"south":"true"},"apply":{"model":"minecolonies:block/blockconstructiontape","y":180,"uvlock":true}},
          {"when":{"west":"true"},"apply":{"model":"minecolonies:block/blockconstructiontape","y":270,"uvlock":true}},
          {"when":{"north":"false","east":"false","south":"true","west":"true"},
           "apply":{"model":"minecolonies:block/blockconstructiontape_corner","uvlock":true}}
        ]}"#;
        let definition = Definition::parse(tape).expect("valid");
        let selection = definition
            .resolve(&state(&[
                ("north", "true"),
                ("east", "false"),
                ("south", "false"),
                ("west", "true"),
            ]))
            .expect("a model");
        assert_eq!(selection.from, Origin::Multipart);
        // The centre always draws; north and west arms draw; the corner does not.
        assert_eq!(selection.parts.len(), 3);
        assert_eq!(
            selection.parts[0].model,
            "minecolonies:block/blockconstructiontape_center"
        );
        assert!(!selection.parts[0].uvlock, "the centre locks nothing");
        assert_eq!(selection.parts[1].turns, [0, 0, 0]);
        assert_eq!(selection.parts[2].turns, [0, 3, 0]);
        assert!(selection.parts[1].uvlock && selection.parts[2].uvlock);
    }

    #[test]
    fn a_multipart_corner_needs_all_four_arms_to_match_its_test() {
        let tape = r#"{"multipart":[
          {"when":{"north":"false","east":"false","south":"true","west":"true"},
           "apply":{"model":"minecolonies:block/blockconstructiontape_corner","uvlock":true}}
        ]}"#;
        let definition = Definition::parse(tape).expect("valid");
        let matching = definition
            .resolve(&state(&[
                ("north", "false"),
                ("east", "false"),
                ("south", "true"),
                ("west", "true"),
            ]))
            .expect("a corner");
        assert_eq!(matching.parts.len(), 1);
        let refusing = definition.resolve(&state(&[
            ("north", "true"),
            ("east", "false"),
            ("south", "true"),
            ("west", "true"),
        ]));
        assert_eq!(
            refusing.expect_err("no case applies").name(),
            "model.refusal.no_variant"
        );
    }

    #[test]
    fn an_or_condition_matches_any_branch_and_a_negation_matches_the_rest() {
        let condition = condition_of(r#"{"OR": [{"north": "true"}, {"facing": "east|west"}]}"#);
        assert!(condition.matches(&state(&[("north", "true"), ("facing", "north")])));
        assert!(condition.matches(&state(&[("north", "false"), ("facing", "west")])));
        assert!(!condition.matches(&state(&[("north", "false"), ("facing", "south")])));

        let negated = condition_of(r#"{"north": "!true"}"#);
        assert!(negated.matches(&state(&[("north", "false")])));
        assert!(!negated.matches(&state(&[("north", "true")])));
        // Absent counts as "not true": vanilla's `!a` is not-equal, not has-value.
        assert!(negated.matches(&state(&[("east", "true")])));
    }

    #[test]
    fn a_missing_definition_is_the_seven_item_models() {
        for text in [
            r#"{"parent":"builtin/generated","textures":{"layer0":"minecolonies:item/compost.png"}}"#,
            r#"{"variants":{}}"#,
            r#"{"multipart":[]}"#,
            r#"[]"#,
        ] {
            let error = Definition::parse(text).expect_err(text);
            assert!(matches!(
                error,
                Refusal::NoDefinition
                    | Refusal::EmptyVariants
                    | Refusal::EmptyMultipart
                    | Refusal::NotAnObject { .. }
            ));
        }
    }

    #[test]
    fn broken_json_names_where_it_broke() {
        let error = Definition::parse("{\"variants\": {").expect_err("truncated");
        match error {
            Refusal::NotJson { line, column } => {
                assert!(line >= 1);
                assert!(column >= 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_rotation_that_is_not_a_quarter_turn_is_refused() {
        let error =
            Definition::parse(r#"{"variants":{"":{"model":"minecolonies:block/x","x":45}}}"#)
                .expect_err("45 degrees");
        assert_eq!(
            error,
            Refusal::RotationNotQuarterTurn {
                axis: "x",
                degrees: 45
            }
        );
        // And the three axes are read, modulo 360.
        let definition = Definition::parse(
            r#"{"variants":{"":{"model":"minecolonies:block/x","x":90,"y":-90,"z":450}}}"#,
        )
        .expect("valid");
        let selection = definition.resolve(&[]).expect("a model");
        assert_eq!(selection.parts[0].turns, [1, 3, 1]);
    }

    #[test]
    fn a_zero_weight_is_refused_and_a_single_model_needs_none() {
        let error =
            Definition::parse(r#"{"variants":{"":[{"model":"minecolonies:block/x","weight":0}]}}"#)
                .expect_err("weight 0");
        assert_eq!(error, Refusal::BadWeight { weight: 0 });
        // No weight at all is weight 1, and `uvlock` defaults to false.
        let definition = Definition::parse(r#"{"variants":{"":{"model":"minecolonies:block/x"}}}"#)
            .expect("valid");
        let selection = definition.resolve(&[]).expect("a model");
        assert_eq!(selection.parts[0].weight, 1);
        assert!(!selection.parts[0].uvlock);
        assert_eq!(selection.parts[0].turns, [0, 0, 0]);
    }

    #[test]
    fn a_model_id_with_a_capital_is_refused_instead_of_baking_nothing() {
        for bad in [
            "Minecolonies:block/x",
            "minecolonies:",
            ":block/x",
            "mine colonies:block/x",
        ] {
            let text = format!(r#"{{"variants":{{"":{{"model":"{bad}"}}}}}}"#);
            let error = Definition::parse(&text).expect_err(bad);
            assert_eq!(error.name(), "model.refusal.bad_model_id", "{bad}");
        }
        // A path with no namespace is vanilla's (`"model": "block/banner"`).
        assert!(Definition::parse(r#"{"variants":{"":{"model":"block/banner"}}}"#).is_ok());
    }

    #[test]
    fn a_key_without_a_value_names_nothing_and_is_refused() {
        let error =
            Definition::parse(r#"{"variants":{"facing":{"model":"minecolonies:block/x"}}}"#)
                .expect_err("no equals sign");
        assert_eq!(
            error,
            Refusal::NotAPair {
                clause: "facing".to_owned()
            }
        );
        let error =
            Definition::parse(r#"{"variants":{"facing=":{"model":"minecolonies:block/x"}}}"#)
                .expect_err("no value");
        assert_eq!(
            error,
            Refusal::EmptyTerm {
                property: "facing".to_owned()
            }
        );
    }

    #[test]
    fn a_when_that_names_or_with_a_non_list_is_refused() {
        let error = Definition::parse(
            r#"{"multipart":[{"when":{"OR":"north"},"apply":{"model":"minecolonies:block/x"}}]}"#,
        )
        .expect_err("OR over a string");
        assert_eq!(error, Refusal::BadCondition { at: "when" });
        let error = Definition::parse(
            r#"{"multipart":[{"when":{},"apply":{"model":"minecolonies:block/x"}}]}"#,
        )
        .expect_err("empty when");
        assert_eq!(error, Refusal::BadCondition { at: "when" });
    }

    #[test]
    fn an_overlapping_file_is_reported_instead_of_guessed() {
        // The game takes the first key and, on overlap, stops that key's walk at
        // the state it would replace. This module answers the first and names the
        // overlap so a caller can refuse the file.
        let definition = Definition::parse(
            r#"{"variants":{"":{"model":"minecolonies:block/a"},"facing=north":{"model":"minecolonies:block/b"}}}"#,
        )
        .expect("valid");
        let states = all_states(&[Prop {
            name: "facing",
            values: FACING4,
        }]);
        assert_eq!(
            definition.overlaps(&states),
            vec!["facing=north".to_owned()]
        );
        // And the answer for north is the first key, not the second.
        let selection = definition
            .resolve(&state(&[("facing", "north")]))
            .expect("a model");
        assert_eq!(selection.parts[0].model, "minecolonies:block/a");
    }

    #[test]
    fn coverage_of_the_real_files_is_what_the_table_measured() {
        // Three of the 95, read here and counted here: the same numbers the
        // generated table carries, computed by a different implementation.
        let farmland = r#"{"variants":{
          "moisture=0":{"model":"minecolonies:block/farmland"},
          "moisture=1":{"model":"minecolonies:block/farmland"},
          "moisture=2":{"model":"minecolonies:block/farmland"},
          "moisture=3":{"model":"minecolonies:block/farmland"},
          "moisture=4":{"model":"minecolonies:block/farmland"},
          "moisture=5":{"model":"minecolonies:block/farmland"},
          "moisture=6":{"model":"minecolonies:block/farmland"},
          "moisture=7":{"model":"minecolonies:block/farmland_moist"}
        }}"#;
        let definition = Definition::parse(farmland).expect("valid");
        let coverage = definition.coverage(&[
            Prop {
                name: "moisture",
                values: MOISTURE8,
            },
            Prop {
                name: "waterlogged",
                values: BOOL2,
            },
        ]);
        assert_eq!(
            coverage,
            Coverage {
                total: 16,
                covered: 16
            }
        );

        let crop = || {
            let mut text = String::from("{\"variants\":{");
            for age in AGE7.iter().copied() {
                if age != "0" {
                    text.push(',');
                }
                text.push_str(&format!(
                    r#""age={age}":{{"model":"minecolonies:block/crops/onion/stage{age}"}}"#
                ));
            }
            text.push_str("}}");
            text
        };
        let definition = Definition::parse(&crop()).expect("valid");
        assert_eq!(
            definition.coverage(&[Prop {
                name: "age",
                values: AGE7
            }]),
            Coverage {
                total: 7,
                covered: 7
            }
        );
        assert!(
            definition
                .overlaps(&all_states(&[Prop {
                    name: "age",
                    values: AGE7
                }]))
                .is_empty()
        );
    }

    #[test]
    fn a_state_string_round_trips_through_the_parser() {
        let parsed = state_of("facing=east,variant=0perc").expect("valid");
        assert_eq!(state_text(&parsed), "facing=east,variant=0perc");
        // `state_text` sorts by name, so the order a caller gives does not show.
        let reversed = state_of("variant=0perc,facing=east").expect("valid");
        assert_eq!(state_text(&reversed), "facing=east,variant=0perc");
        assert_eq!(state_text(&[]), "");
        assert_eq!(
            parse_key("facing").expect_err("no value").name(),
            "model.refusal.not_a_pair"
        );
    }

    #[test]
    fn all_states_is_the_product_in_declaration_order() {
        let states = all_states(&[Prop {
            name: "facing",
            values: FACING4,
        }]);
        assert_eq!(states.len(), 4);
        assert_eq!(state_text(&states[0]), "facing=north");
        assert_eq!(state_text(&states[3]), "facing=west");
        let states = all_states(&[
            Prop {
                name: "facing",
                values: FACING4,
            },
            Prop {
                name: "hinge",
                values: HINGE2,
            },
        ]);
        assert_eq!(states.len(), 8);
        assert_eq!(state_text(&states[1]), "facing=north,hinge=right");
        assert_eq!(states[0].len(), 2);
        assert_eq!(all_states(&[]).len(), 1, "no property is one state");
    }

    #[test]
    fn a_part_drawn_from_a_single_variant_has_weight_one() {
        let part = one(POSTBOX, &[("facing", "north")]);
        assert_eq!(part.weight, 1);
        assert_eq!(part.turns, [0, 2, 0]);
        assert!(!part.uvlock);
    }
}
