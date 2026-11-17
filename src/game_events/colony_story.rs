//! `ColonyStoryListener.java`: the four story tables, and what a bad file costs.
//!
//! Java reference: `ColonyStoryListener.java`, 264 lines, 6 branches, all in
//! [`super::table`]. A client reload listener over `colony_stories/**:/*.json` that
//! fills four static sets: abandoned colony names, abandoned colony stories, supply
//! ship stories, supply camp stories. The town hall and the supply camp read them to
//! pick one line for the player.
//!
//! **The four things this file decides, and they are not the obvious four.**
//!
//! 1. **A file is either one story or an array of them** (`:111-118`), and both forms
//!    are a file of `type` + `biomes` + `content`. An array element that is not an
//!    object is a `getAsJsonObject` throw, caught at the file level.
//! 2. **An unknown `type` is ignored silently, on purpose** (`:146-149`): "in case
//!    it's something intended for a different mod". So the four tables are the four
//!    keys and a fifth key is not an error.
//! 3. **A bad file costs more than itself, and less than the reload.** The four sets
//!    are cleared **first** (`:102-105`), then files are read one by one with
//!    `catch (Throwable)` (`:120-123`). A file that throws loses only its own
//!    stories — but the reload has already wiped the previous table, so a reload
//!    where every file is bad leaves **nothing**, not the table that was there
//!    before. [`Stories::apply`] answers the refusals instead of logging them.
//! 4. **A bare biome id is resolved against the server's registry and throws when it
//!    is unknown** (`:228-231`), while a `#tag` is taken as written (`:221-224`). So a
//!    typo in a biome id costs the file, and a typo in a tag costs nothing until the
//!    filter asks and never matches.
//!
//! **The biome filter is an OR tree with `ALL` as its identity** (`:189-234`):
//! `biomes` may be one string or an array, and the array's filters are OR-ed. Java
//! starts from `ALL` and folds `or` over the elements, which is why an **empty**
//! array is `ALL` (matches every biome) and not "matches none". [`Filter::or`]
//! reproduces that identity exactly, which a `Vec<Filter>` of alternatives would
//! lose.

use mod_api::serde_json::Value;

/// The four names `type` may hold, with Java's ids (`:44-56`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `minecolonies:abandoned_name` (`:44`).
    AbandonedName,
    /// `minecolonies:abandoned` (`:48`).
    Abandoned,
    /// `minecolonies:camp` (`:52`).
    Camp,
    /// `minecolonies:ship` (`:56`).
    Ship,
}

impl Kind {
    /// The four, in the order the Java declares them.
    pub const ALL: &'static [Kind] = &[Kind::AbandonedName, Kind::Abandoned, Kind::Camp, Kind::Ship];

    /// The id a file writes.
    pub fn id(self) -> &'static str {
        match self {
            Kind::AbandonedName => "minecolonies:abandoned_name",
            Kind::Abandoned => "minecolonies:abandoned",
            Kind::Camp => "minecolonies:camp",
            Kind::Ship => "minecolonies:ship",
        }
    }

    /// The id back, or `None` for a key of another mod — which is ignored, not
    /// refused (`:146-149`).
    pub fn from_id(text: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|kind| kind.id() == text)
    }
}

/// Why one file produced no story.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `content` is missing, or is a container: `getAsString()` on an object or an array
    /// throws, and a missing key is a `null` dereference (`:172-184`).
    MissingContent,
    /// `biomes` names a biome the registry does not hold (`:230`:
    /// `getHolderOrThrow`).
    UnknownBiome(String),
    /// `biomes` is neither a string nor an array of strings (`:159-169` asks
    /// `getAsString` on whatever it found).
    BadBiomes,
    /// The value is neither a story object nor an array of them.
    NotAStory,
    /// `type` mentions a name that is not a Minecraft resource location
    /// (`:129`: `new ResourceLocation` throws on `""`).
    BadType,
}

/// A biome filter, `ColonyStoryListener.BiomeFilter` (`:189-234`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    /// `BiomeFilter.ALL` (`:194`): every biome, and the identity of [`Filter::or`].
    All,
    /// `#ns:tag` (`:221-224`): the biome is in the tag.
    Tag(String),
    /// `ns:biome` (`:226-232`): the biome is this one, resolved at parse.
    Biome(String),
    /// Two filters OR-ed (`:203`).
    Any(Box<Filter>, Box<Filter>),
}

impl Filter {
    /// Whether the biome `id` passes, given a way to ask about tags.
    ///
    /// `in_tag(biome, tag)` is `Holder::is(TagKey)` — the caller owns the tag table
    /// because it is the server's.
    pub fn matches(&self, biome: &str, in_tag: impl Fn(&str, &str) -> bool + Copy) -> bool {
        match self {
            Filter::All => true,
            Filter::Tag(tag) => in_tag(biome, tag),
            Filter::Biome(wanted) => wanted == biome,
            Filter::Any(left, right) => {
                left.matches(biome, in_tag) || right.matches(biome, in_tag)
            }
        }
    }

    /// `this || other`, with `ALL` as the identity (`:201-204`):
    ///
    /// ```text
    /// return (this == ALL) ? other : (other == ALL) ? this : new BiomeFilter(a || b);
    /// ```
    ///
    /// The identity is not an optimisation: starting from `ALL` and folding over an
    /// array is how Java keeps an empty array meaning "every biome".
    pub fn or(self, other: Filter) -> Filter {
        match (self, other) {
            (Filter::All, other) => other,
            (this, Filter::All) => this,
            (left, right) => Filter::Any(Box::new(left), Box::new(right)),
        }
    }
}

/// One line of story text, valid for the biomes its filter passes
/// (`StoryText`, `:241-251`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryText {
    pub filter: Filter,
    pub content: String,
}

impl StoryText {
    /// Whether this line may be told in that biome (`:248-251`).
    pub fn matches(&self, biome: &str, in_tag: impl Fn(&str, &str) -> bool + Copy) -> bool {
        self.filter.matches(biome, in_tag)
    }
}

/// What the caller must know about biomes to parse a file: the registry, and the
/// tags.
///
/// Java reaches the first through `ServerLifecycleHooks.getCurrentServer()` (`:228`)
/// and the second through the holder itself; both are the server's, so the port takes
/// them as a trait instead of reading a static.
pub trait Biomes {
    /// Whether the registry holds this biome id (`registryOrThrow(...).getHolderOrThrow`).
    fn knows(&self, biome: &str) -> bool;
    /// `Holder::is(TagKey)` for `#tag` filters.
    fn in_tag(&self, biome: &str, tag: &str) -> bool;
}

/// A biome set that knows nothing: what a unit test uses when the stories all say
/// `ALL`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoBiomes;

impl Biomes for NoBiomes {
    fn knows(&self, _biome: &str) -> bool {
        false
    }

    fn in_tag(&self, _biome: &str, _tag: &str) -> bool {
        false
    }
}

/// The four tables, as one value: what the four statics add up to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stories {
    pub names: Vec<StoryText>,
    pub abandoned: Vec<StoryText>,
    pub camps: Vec<StoryText>,
    pub ships: Vec<StoryText>,
}

impl Stories {
    /// An empty table.
    pub fn new() -> Stories {
        Stories::default()
    }

    /// The table of one kind, for reading.
    pub fn of(&self, kind: Kind) -> &[StoryText] {
        match kind {
            Kind::AbandonedName => &self.names,
            Kind::Abandoned => &self.abandoned,
            Kind::Camp => &self.camps,
            Kind::Ship => &self.ships,
        }
    }

    fn of_mut(&mut self, kind: Kind) -> &mut Vec<StoryText> {
        match kind {
            Kind::AbandonedName => &mut self.names,
            Kind::Abandoned => &mut self.abandoned,
            Kind::Camp => &mut self.camps,
            Kind::Ship => &mut self.ships,
        }
    }

    /// How many lines the four tables hold.
    pub fn len(&self) -> usize {
        self.names.len() + self.abandoned.len() + self.camps.len() + self.ships.len()
    }

    /// Whether the reload left nothing at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `apply` (`:99-125`): **clear first**, then read each file, and keep the
    /// refusals. A refused file loses only its own lines; the ones before it stay,
    /// which is exactly the cost the class header in [`super`] describes.
    pub fn apply(
        &mut self,
        files: impl IntoIterator<Item = Value>,
        biomes: &dyn Biomes,
    ) -> Vec<Refusal> {
        for kind in Kind::ALL {
            self.of_mut(*kind).clear();
        }
        let mut refusals = Vec::new();
        for file in files {
            match parse_file(&file, biomes) {
                Ok(stories) => {
                    for (kind, story) in stories {
                        self.insert(kind, story);
                    }
                }
                Err(refusal) => refusals.push(refusal),
            }
        }
        refusals
    }

    /// Add one line the way Java's `HashSet<StoryText>.add` does (`:61-65`).
    ///
    /// `StoryText` is a **record** (`:241`), so its equality is the pair's:
    /// `BiomeFilter`'s and `String`'s. `BiomeFilter` is a record around a `Predicate`
    /// (`:189`), and a lambda does not override `equals` — so two filters are equal
    /// only when they are *the same instance*. That happens for exactly one filter,
    /// `BiomeFilter.ALL` (`:194`), the constant every story without a `biomes` key
    /// shares; `parse` builds a fresh lambda per call (`:224`, `:231`). The result is
    /// a rule a brief would never guess, and the port keeps it:
    ///
    /// - two identical lines with no `biomes` key **collapse**, because both are `ALL`;
    /// - two identical lines naming the same biome or tag **do not**, because their
    ///   filters are two different objects.
    ///
    /// The list, not a set, keeps Java's insertion order for the lines that survive.
    fn insert(&mut self, kind: Kind, story: StoryText) {
        // `==` on the filter, not `matches!`: both sides are read through a reference,
        // so nothing is moved out of `story` before it is pushed.
        let repeated = story.filter == Filter::All
            && self
                .of(kind)
                .iter()
                .any(|held| held.filter == Filter::All && held.content == story.content);
        if !repeated {
            self.of_mut(kind).push(story);
        }
    }
}

/// One file: an array of stories, or one story (`:107-124`).
///
/// An array element that is not an object is a refusal for the whole file, because
/// Java's `getAsJsonObject` throws inside the `forEach` and the `catch` is around the
/// file.
pub fn parse_file(file: &Value, biomes: &dyn Biomes) -> Result<Vec<(Kind, StoryText)>, Refusal> {
    let mut out = Vec::new();
    match file.as_array() {
        Some(elements) => {
            for element in elements {
                out.extend(parse_story(element, biomes)?);
            }
        }
        None => out.extend(parse_story(file, biomes)?),
    }
    Ok(out)
}

/// One story object (`:127-184`).
///
/// An unknown `type` answers `Ok(None)`-shaped emptiness, not a refusal: the file is
/// somebody else's. What separates "unknown" from "refused" is Java's own split
/// (`:129`):
///
/// - `type` is read with `getAsString()`, so a **number or a boolean is its text**
///   (`3` → `"3"`) and `new ResourceLocation("3")` accepts it as `minecraft:3` — an
///   unknown id, silently ignored;
/// - a missing `type` becomes `""` (`Objects.requireNonNullElse`), and an id that
///   `ResourceLocation` refuses (empty, uppercase, a space, a stray colon) **throws**,
///   which the file-level `catch` turns into a lost file;
/// - an object or an array as `type` makes `getAsString()` throw, same outcome.
///
/// [`resource_location_is_valid`] is that charset rule. A first version of this
/// function refused every non-string `type`, which dropped a file Java ignores; the
/// audit found it.
pub fn parse_story(
    value: &Value,
    biomes: &dyn Biomes,
) -> Result<Vec<(Kind, StoryText)>, Refusal> {
    let Some(object) = value.as_object() else {
        return Err(Refusal::NotAStory);
    };
    let type_text = story_text(object.get("type"))?;
    if !resource_location_is_valid(&type_text) {
        return Err(Refusal::BadType);
    }
    let Some(kind) = Kind::from_id(&type_text) else {
        return Ok(Vec::new());
    };
    let filter = parse_biomes(object.get("biomes"), biomes)?;
    let contents = content_list(object.get("content"))?;
    Ok(contents
        .into_iter()
        .map(|content| {
            (
                kind,
                StoryText {
                    filter: filter.clone(),
                    content,
                },
            )
        })
        .collect())
}

/// `content` as a list of texts: one value, or an array of them (`:172-184`).
///
/// Java reads each one with `getAsString()`, so a number or a boolean is its own text;
/// a missing `content` is a `null` dereference and an object is a
/// `getAsString()` throw, and both lose the file. A first version of this function
/// refused numbers, which lost a line Java keeps.
fn content_list(content: Option<&Value>) -> Result<Vec<String>, Refusal> {
    match content {
        None => Err(Refusal::MissingContent),
        Some(Value::Array(elements)) => {
            let mut out = Vec::with_capacity(elements.len());
            for element in elements {
                out.push(scalar_text(element)?);
            }
            Ok(out)
        }
        Some(value) => Ok(vec![scalar_text(value)?]),
    }
}

/// `JsonElement.getAsString()`: a string as itself, a number or a boolean as its JSON
/// text, and anything else throws.
fn scalar_text(value: &Value) -> Result<String, Refusal> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => Err(Refusal::MissingContent),
    }
}

/// The `type` key as Java reads it (`:129`): `Objects.requireNonNullElse(json.get("type"),
/// new JsonPrimitive("")).getAsString()`.
///
/// Absent is the empty string, which [`resource_location_is_valid`] refuses; a scalar
/// is its text; a container makes `getAsString()` throw.
fn story_text(value: Option<&Value>) -> Result<String, Refusal> {
    match value {
        None => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        Some(Value::Bool(flag)) => Ok(flag.to_string()),
        // `getAsString()` on a container throws, and the file is lost for its `type`.
        Some(Value::Null | Value::Array(_) | Value::Object(_)) => Err(Refusal::BadType),
    }
}

/// Whether `ResourceLocation` accepts this text (`ResourceLocation.tryParse`, the
/// constructor `:129` calls): `namespace:path` with no second colon, a namespace of
/// `[a-z0-9_.-]` and a path of `[a-z0-9/._-]`, both non-empty; a bare path is accepted
/// and defaulted to `minecraft`.
///
/// This is a refusal rule and not a formatting one: a file whose `type` fails it is
/// lost whole, while a file whose `type` passes it and names something unknown is
/// ignored in silence.
pub fn resource_location_is_valid(text: &str) -> bool {
    let (namespace, path) = match text.split_once(':') {
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", text),
    };
    if path.contains(':') {
        return false;
    }
    let namespace_ok = !namespace.is_empty()
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte));
    let path_ok = !path.is_empty()
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"/._-".contains(&byte)
        });
    namespace_ok && path_ok
}

/// `biomes` as a filter (`:152-170`): absent → `ALL`; a string → its filter; an array
/// → the OR of its elements, starting from `ALL`.
fn parse_biomes(biomes: Option<&Value>, registry: &dyn Biomes) -> Result<Filter, Refusal> {
    let Some(biomes) = biomes else {
        return Ok(Filter::All);
    };
    match biomes {
        Value::String(text) => parse_filter(text, registry),
        Value::Array(elements) => {
            let mut filter = Filter::All;
            for element in elements {
                let Value::String(text) = element else {
                    return Err(Refusal::BadBiomes);
                };
                filter = filter.or(parse_filter(text, registry)?);
            }
            Ok(filter)
        }
        _ => Err(Refusal::BadBiomes),
    }
}

/// One filter string (`:219-233`): `#ns:tag` is a tag, anything else is a biome id
/// that the registry must hold.
fn parse_filter(text: &str, registry: &dyn Biomes) -> Result<Filter, Refusal> {
    match text.strip_prefix('#') {
        Some(tag) => Ok(Filter::Tag(tag.to_owned())),
        None => {
            if registry.knows(text) {
                Ok(Filter::Biome(text.to_owned()))
            } else {
                Err(Refusal::UnknownBiome(text.to_owned()))
            }
        }
    }
}

/// `pickRandom` (`:88-97`): one of the lines valid in this biome, or `""`.
///
/// `roll` is the random draw, already taken: `rng.nextInt(matches.size())` is
/// `roll % len`. An empty match list answers the empty string, which is what the
/// callers treat as "no story" (they do not test for `null`).
pub fn pick<'a>(
    stories: &'a [StoryText],
    biome: &str,
    in_tag: impl Fn(&str, &str) -> bool + Copy,
    roll: u64,
) -> &'a str {
    let matches: Vec<&'a str> = stories
        .iter()
        .filter(|story| story.matches(biome, in_tag))
        .map(|story| story.content.as_str())
        .collect();
    if matches.is_empty() {
        return "";
    }
    matches[(roll % matches.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("valid json")
    }

    #[derive(Default)]
    struct TestBiomes {
        biomes: Vec<&'static str>,
        tags: Vec<(&'static str, &'static str)>,
    }

    impl Biomes for TestBiomes {
        fn knows(&self, biome: &str) -> bool {
            self.biomes.contains(&biome)
        }

        fn in_tag(&self, biome: &str, tag: &str) -> bool {
            self.tags.contains(&(biome, tag))
        }
    }

    #[test]
    fn the_four_ids_are_the_four_minecraft_locations() {
        assert_eq!(Kind::from_id("minecolonies:abandoned_name"), Some(Kind::AbandonedName));
        assert_eq!(Kind::from_id("minecolonies:abandoned"), Some(Kind::Abandoned));
        assert_eq!(Kind::from_id("minecolonies:camp"), Some(Kind::Camp));
        assert_eq!(Kind::from_id("minecolonies:ship"), Some(Kind::Ship));
        assert_eq!(Kind::from_id("othermod:story"), None);
        assert_eq!(Kind::ALL.len(), 4);
    }

    #[test]
    fn all_is_the_identity_of_or() {
        assert_eq!(Filter::All.or(Filter::Tag("a".into())), Filter::Tag("a".into()));
        assert_eq!(
            Filter::Biome("minecraft:plains".into()).or(Filter::All),
            Filter::Biome("minecraft:plains".into())
        );
    }

    #[test]
    fn two_real_filters_are_or_ed_into_one() {
        let both = Filter::Tag("a".into()).or(Filter::Biome("b".into()));
        assert!(matches!(both, Filter::Any(_, _)));
        assert!(both.matches("x", |_, _| true), "the tag arm passes");
        assert!(both.matches("b", |_, _| false), "the biome arm passes");
        assert!(!both.matches("c", |_, _| false));
    }

    #[test]
    fn an_empty_biome_array_matches_every_biome() {
        let biomes = TestBiomes::default();
        let stories = parse_story(
            &json(r#"{"type":"minecolonies:camp","biomes":[],"content":"hello"}"#),
            &biomes,
        )
        .expect("parsed");
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].1.filter, Filter::All);
    }

    #[test]
    fn an_array_of_biomes_is_or_ed() {
        let biomes = TestBiomes {
            biomes: vec!["minecraft:plains", "minecraft:desert"],
            tags: Vec::new(),
        };
        let stories = parse_story(
            &json(r#"{"type":"minecolonies:ship","biomes":["minecraft:plains","minecraft:desert"],"content":"hi"}"#),
            &biomes,
        )
        .expect("parsed");
        let filter = &stories[0].1.filter;
        assert!(filter.matches("minecraft:desert", |_, _| false));
        assert!(!filter.matches("minecraft:ocean", |_, _| false));
    }

    #[test]
    fn an_unknown_biome_id_costs_the_whole_file() {
        let biomes = TestBiomes {
            biomes: vec!["minecraft:plains"],
            tags: Vec::new(),
        };
        let refusal = parse_story(
            &json(r#"{"type":"minecolonies:camp","biomes":["minecraft:plains","minecraft:nowhere"],"content":"hi"}"#),
            &biomes,
        );
        assert_eq!(
            refusal,
            Err(Refusal::UnknownBiome("minecraft:nowhere".into())),
            "`getHolderOrThrow` (`:230`) throws, and the file is lost"
        );
    }

    #[test]
    fn a_tag_is_taken_as_written_even_when_it_matches_nothing() {
        let biomes = TestBiomes::default();
        let stories = parse_story(
            &json(r##"{"type":"minecolonies:camp","biomes":"#minecolonies:dry","content":"hi"}"##),
            &biomes,
        )
        .expect("a tag is never resolved at parse");
        assert_eq!(stories[0].1.filter, Filter::Tag("minecolonies:dry".into()));
        assert!(!stories[0].1.matches("minecraft:desert", |_, _| false));
    }

    #[test]
    fn a_content_array_makes_one_story_per_element() {
        let stories = parse_story(
            &json(r#"{"type":"minecolonies:abandoned_name","content":["Ashford","Dunmore"]}"#),
            &NoBiomes,
        )
        .expect("parsed");
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].1.content, "Ashford");
        assert_eq!(stories[1].1.content, "Dunmore");
    }

    #[test]
    fn a_missing_content_is_a_refusal_and_not_an_empty_line() {
        assert_eq!(
            parse_story(&json(r#"{"type":"minecolonies:camp"}"#), &NoBiomes),
            Err(Refusal::MissingContent)
        );
    }

    #[test]
    fn a_type_of_another_mod_is_ignored_and_is_not_a_refusal() {
        // `:146-149`: "ignore without warning in case it's something intended for a
        // different mod".
        let stories = parse_story(
            &json(r#"{"type":"othermod:story","content":"hi"}"#),
            &NoBiomes,
        )
        .expect("ignored");
        assert!(stories.is_empty());
    }

    #[test]
    fn a_numeric_type_is_its_text_and_is_ignored_not_refused() {
        // `getAsString()` on a number is "3"; `new ResourceLocation("3")` is
        // `minecraft:3`, which matches no story type — so the file survives and its
        // story is somebody else's.
        let stories = parse_story(&json(r#"{"type":3,"content":"hi"}"#), &NoBiomes)
            .expect("a numeric type is not a refusal");
        assert!(stories.is_empty());
    }

    #[test]
    fn a_type_that_is_an_object_or_an_array_loses_the_file() {
        assert_eq!(
            parse_story(&json(r#"{"type":{"a":1},"content":"hi"}"#), &NoBiomes),
            Err(Refusal::BadType)
        );
    }

    #[test]
    fn an_id_resource_location_refuses_loses_the_file() {
        // Uppercase, an empty namespace and a second colon are all
        // `ResourceLocationException` in `:129`.
        for bad in [
            r#"{"type":"MineColonies:camp","content":"hi"}"#,
            r#"{"type":":camp","content":"hi"}"#,
            r#"{"type":"a:b:camp","content":"hi"}"#,
            r#"{"type":"camp ing","content":"hi"}"#,
        ] {
            assert_eq!(
                parse_story(&json(bad), &NoBiomes),
                Err(Refusal::BadType),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn a_bare_path_is_a_minecraft_id_and_is_ignored() {
        // `new ResourceLocation("camp")` is `minecraft:camp`: valid, and not ours.
        assert!(parse_story(&json(r#"{"type":"camp","content":"hi"}"#), &NoBiomes)
            .expect("valid id")
            .is_empty());
    }

    #[test]
    fn a_numeric_content_is_its_text() {
        // `getAsString()` on the number 12 is "12".
        let stories = parse_story(
            &json(r#"{"type":"minecolonies:camp","content":[12,true,"x"]}"#),
            &NoBiomes,
        )
        .expect("parsed");
        assert_eq!(stories.len(), 3);
        assert_eq!(stories[0].1.content, "12");
        assert_eq!(stories[1].1.content, "true");
        assert_eq!(stories[2].1.content, "x");
    }

    #[test]
    fn an_object_content_loses_the_file() {
        assert_eq!(
            parse_story(&json(r#"{"type":"minecolonies:camp","content":{"a":1}}"#), &NoBiomes),
            Err(Refusal::MissingContent)
        );
    }

    #[test]
    fn a_missing_type_is_the_empty_id_which_the_java_refuses() {
        assert_eq!(
            parse_story(&json(r#"{"content":"hi"}"#), &NoBiomes),
            Err(Refusal::BadType)
        );
    }

    #[test]
    fn a_file_is_a_story_or_an_array_of_them() {
        let one = parse_file(
            &json(r#"{"type":"minecolonies:camp","content":"one"}"#),
            &NoBiomes,
        )
        .expect("one story");
        assert_eq!(one.len(), 1);
        let many = parse_file(
            &json(r#"[{"type":"minecolonies:camp","content":"one"},{"type":"minecolonies:ship","content":"two"}]"#),
            &NoBiomes,
        )
        .expect("two stories");
        assert_eq!(many.len(), 2);
    }

    #[test]
    fn an_array_containing_a_non_object_loses_the_whole_file() {
        let refusal = parse_file(&json(r#"[{"type":"minecolonies:camp","content":"ok"},42]"#), &NoBiomes);
        assert_eq!(refusal, Err(Refusal::NotAStory));
    }

    #[test]
    fn a_reload_clears_before_it_reads() {
        let mut stories = Stories::new();
        let first = parse_file(
            &json(r#"{"type":"minecolonies:camp","content":"old"}"#),
            &NoBiomes,
        )
        .expect("parsed");
        for (kind, story) in first {
            stories.of_mut(kind).push(story);
        }
        assert_eq!(stories.len(), 1);
        // The second reload has one bad file and one good one.
        let refusals = stories.apply(
            vec![
                json(r#"{"type":"minecolonies:camp"}"#),
                json(r#"{"type":"minecolonies:camp","content":"new"}"#),
            ],
            &NoBiomes,
        );
        assert_eq!(refusals, vec![Refusal::MissingContent]);
        assert_eq!(stories.len(), 1, "the old line is gone, the new one is there");
        assert_eq!(stories.camps[0].content, "new");
    }

    #[test]
    fn two_identical_lines_without_a_biome_collapse_like_a_hash_set() {
        // `BiomeFilter.ALL` is one static instance (`:194`), so the two records are
        // equal and `HashSet.add` drops the second.
        let mut stories = Stories::new();
        let refusals = stories.apply(
            vec![
                json(r#"{"type":"minecolonies:camp","content":"same"}"#),
                json(r#"{"type":"minecolonies:camp","content":"same"}"#),
            ],
            &NoBiomes,
        );
        assert!(refusals.is_empty());
        assert_eq!(stories.camps.len(), 1, "one line, as in Java");
    }

    #[test]
    fn two_identical_lines_with_the_same_biome_both_stay() {
        // `parse` makes a new `BiomeFilter(b -> …)` per call (`:231`), and a lambda has
        // no value equality: the records are not equal, so both are kept.
        let biomes = TestBiomes {
            biomes: vec!["minecraft:plains"],
            tags: Vec::new(),
        };
        let mut stories = Stories::new();
        stories.apply(
            vec![
                json(r#"{"type":"minecolonies:ship","biomes":"minecraft:plains","content":"same"}"#),
                json(r#"{"type":"minecolonies:ship","biomes":"minecraft:plains","content":"same"}"#),
            ],
            &biomes,
        );
        assert_eq!(stories.ships.len(), 2, "two different filter objects");
    }

    #[test]
    fn a_biome_line_and_a_global_line_with_the_same_text_both_stay() {
        let biomes = TestBiomes {
            biomes: vec!["minecraft:desert"],
            tags: Vec::new(),
        };
        let mut stories = Stories::new();
        stories.apply(
            vec![
                json(r#"{"type":"minecolonies:abandoned","content":"same"}"#),
                json(r#"{"type":"minecolonies:abandoned","biomes":"minecraft:desert","content":"same"}"#),
            ],
            &biomes,
        );
        assert_eq!(stories.abandoned.len(), 2, "different filters, different records");
    }

    #[test]
    fn a_reload_where_every_file_is_bad_leaves_nothing() {
        let mut stories = Stories {
            camps: vec![StoryText {
                filter: Filter::All,
                content: "old".into(),
            }],
            ..Stories::new()
        };
        let refusals = stories.apply(vec![json(r#"{"type":"minecolonies:camp"}"#)], &NoBiomes);
        assert_eq!(refusals.len(), 1);
        assert!(
            stories.is_empty(),
            "the clear runs before the first read (`:102-105`)"
        );
    }

    #[test]
    fn a_pick_only_offers_the_lines_of_this_biome() {
        let stories = vec![
            StoryText {
                filter: Filter::Biome("minecraft:desert".into()),
                content: "dry".into(),
            },
            StoryText {
                filter: Filter::All,
                content: "any".into(),
            },
        ];
        assert_eq!(pick(&stories, "minecraft:desert", |_, _| false, 0), "dry");
        assert_eq!(pick(&stories, "minecraft:ocean", |_, _| false, 0), "any");
    }

    #[test]
    fn a_pick_with_no_match_is_the_empty_string() {
        let stories = vec![StoryText {
            filter: Filter::Biome("minecraft:desert".into()),
            content: "dry".into(),
        }];
        assert_eq!(pick(&stories, "minecraft:ocean", |_, _| false, 7), "");
    }

    #[test]
    fn the_roll_wraps_around_the_matching_lines() {
        let stories = vec![
            StoryText {
                filter: Filter::All,
                content: "a".into(),
            },
            StoryText {
                filter: Filter::All,
                content: "b".into(),
            },
        ];
        assert_eq!(pick(&stories, "x", |_, _| false, 0), "a");
        assert_eq!(pick(&stories, "x", |_, _| false, 1), "b");
        assert_eq!(pick(&stories, "x", |_, _| false, 2), "a");
        assert_eq!(pick(&stories, "x", |_, _| false, 9), "b");
    }

    #[test]
    fn a_tag_filter_asks_the_tag_table() {
        let stories = vec![StoryText {
            filter: Filter::Tag("minecolonies:dry".into()),
            content: "dry".into(),
        }];
        assert_eq!(
            pick(&stories, "minecraft:desert", |_, tag| tag == "minecolonies:dry", 0),
            "dry"
        );
        assert_eq!(pick(&stories, "minecraft:desert", |_, _| false, 0), "");
    }

    #[test]
    fn a_biomes_value_that_is_neither_string_nor_array_is_refused() {
        assert_eq!(
            parse_story(
                &json(r#"{"type":"minecolonies:camp","biomes":3,"content":"hi"}"#),
                &NoBiomes
            ),
            Err(Refusal::BadBiomes)
        );
    }
}
