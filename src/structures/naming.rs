//! What a blueprint is *called*: a style, a folder, a base name and a level.
//!
//! Java reference: `api/compatibility/newstruct/BlueprintMapping.java` — the
//! whole naming rule of MineColonies is four `HashMap`s in that one class
//! (`styleMapping` `:25-55`, `foldersToHuts` `:57-72`, `pathMapping` `:110-121`,
//! and the two generators `runForStyle` `:166-175` / `runForAlt` `:150-159`).
//! The path a building then asks the pack for is
//! `BlueprintMapping.getPathMapping(style, getSchematicName()) + buildingLevel + ".blueprint"`
//! (`core/colony/buildings/AbstractSchematicProvider.java:231`), and
//! `getSchematicName()` is the building's registry path
//! (`api/blocks/AbstractBlockHut.java:344-347`).
//!
//! **Three names, not one.** A style has a *key* (the name a save holds, e.g.
//! `stone`), a *folder* (the pack directory, e.g. `original`) and an
//! *alternative* flag (the ten `…alternative` keys of `:46-55`). This file is
//! the only place that translates between them, because a save written in 1.12
//! and a save written today both have to reach the same file.
//!
//! **The corpus renamed two spellings, and the corpus wins.** Java writes the
//! alternative variant as `alt<name>` (`:150-159` → `fundamentals/altbuilder`),
//! and the corpus has exactly that (5 styles); but it *also* has
//! `<name>alt` (`shire/fundamentals/builderalt1.nbt`, 2 styles) and
//! `<name>_alt` (`darkoak/military/barracks_alt1.nbt`). [`candidates`] returns
//! them in that order — Java's spelling first, so a file that exists under both
//! names is read the way the mod reads it, then the two corpus spellings, then
//! the plain base, which is the last resort and always the pre-1.14 behaviour.
//!
//! **The extension is not part of the name.** Java strips `.blueprint` before
//! reading the level (`api/util/Utils.java:201`); the corpus is `.nbt`. Both are
//! stripped here, so a name read from either container is the same name.

use crate::building::Kind;

use super::{EXTENSION, MIN_LEVEL, NAMESPACE, Refusal};

/// `BlueprintMapping.foldersToHuts` (`:57-72`), one row per schematic name.
///
/// **49 rows for 54 kinds.** `gatehouse`, `kitchen`, `postbox`, `stable` and
/// `stash` are missing from Java's table (`:57-72`), so [`folder_of`] answers
/// `None` for them and the caller has to look for the base name anywhere in the
/// style — which is what [`super::Index`] does. Measured on the corpus: the three
/// of those five that have blueprints at all are `military/gatehouse`,
/// `craftsmanship/luxury/kitchen` and `military/stable`.
///
/// Sorted by schematic name; a test holds both facts.
const FOLDERS: &[(&str, &str)] = &[
    ("alchemist", "craftsmanship/luxury"),
    ("archery", "military"),
    ("baker", "craftsmanship/luxury"),
    ("barracks", "military"),
    ("barrackstower", "military"),
    ("beekeeper", "agriculture/husbandry"),
    ("blacksmith", "craftsmanship/metallurgy"),
    ("builder", "fundamentals"),
    ("chickenherder", "agriculture/husbandry"),
    ("combatacademy", "military"),
    ("composter", "agriculture/horticulture"),
    ("concretemixer", "craftsmanship/luxury"),
    ("cook", "fundamentals"),
    ("cowboy", "agriculture/husbandry"),
    ("crusher", "craftsmanship/masonry"),
    ("deliveryman", "craftsmanship/storage"),
    ("dyer", "craftsmanship/luxury"),
    ("enchanter", "mystic"),
    ("farmer", "agriculture/horticulture"),
    ("fisherman", "agriculture/husbandry"),
    ("fletcher", "craftsmanship/carpentry"),
    ("florist", "agriculture/horticulture"),
    ("glassblower", "craftsmanship/luxury"),
    ("graveyard", "mystic"),
    ("guardtower", "military"),
    ("hospital", "fundamentals"),
    ("library", "education"),
    ("lumberjack", "fundamentals"),
    ("mechanic", "craftsmanship/metallurgy"),
    ("mediumquarry", "infrastructure/mineshafts"),
    ("miner", "fundamentals"),
    ("mysticalsite", "mystic"),
    ("netherworker", "mystic"),
    ("plantation", "agriculture/horticulture"),
    ("rabbithutch", "agriculture/husbandry"),
    ("residence", "fundamentals"),
    ("sawmill", "craftsmanship/carpentry"),
    ("school", "education"),
    ("shepherd", "agriculture/husbandry"),
    ("sifter", "craftsmanship/masonry"),
    ("simplequarry", "infrastructure/mineshafts"),
    ("smeltery", "craftsmanship/metallurgy"),
    ("stonemason", "craftsmanship/masonry"),
    ("stonesmeltery", "craftsmanship/masonry"),
    ("swineherder", "agriculture/husbandry"),
    ("tavern", "fundamentals"),
    ("townhall", "fundamentals"),
    ("university", "education"),
    ("warehouse", "craftsmanship/storage"),
];

/// The 23 style folders the corpus carries, measured under
/// `data/minecolonies/structure/minecolonies/`.
///
/// 19 of them are reachable through `BlueprintMapping.styleMapping` (`:25-55`);
/// `ancientathens`, `cavern`, `colonial` and `shire` are **not in Java's table**
/// — they belong to packs newer than the mapping — and `pagoda` is reached
/// through the legacy key `asian`.
const STYLE_FOLDERS: &[&str] = &[
    "acacia",
    "ancientathens",
    "birch",
    "caledonia",
    "cavern",
    "colonial",
    "darkoak",
    "fortress",
    "incan",
    "jungle",
    "lostcity",
    "medievalbirch",
    "medievaldarkoak",
    "medievaloak",
    "medievalspruce",
    "nordic",
    "original",
    "pagoda",
    "sandstone",
    "shire",
    "spacewars",
    "truedwarven",
    "warped",
];

/// The keys `BlueprintMapping.styleMapping` knows that are **not** a folder:
/// three pre-1.14 aliases (`:41-44`) and the ten `…alternative` keys (`:46-55`).
///
/// `stone` and `wooden` both mean "Minecolonies Original", whose folder is
/// `original`; `asian` means "Pagoda", whose folder is `pagoda`. Sorted by key.
const STYLE_ALIASES: &[(&str, &str, bool)] = &[
    ("asian", "pagoda", false),
    ("asianalternative", "pagoda", true),
    ("caledoniaalternative", "caledonia", true),
    ("fortressalternative", "fortress", true),
    ("junglealternative", "jungle", true),
    ("medievalbirchalternative", "medievalbirch", true),
    ("medievaldarkoakalternative", "medievaldarkoak", true),
    ("medievaloakalternative", "medievaloak", true),
    ("medievalsprucealternative", "medievalspruce", true),
    ("stone", "original", false),
    ("stonealternative", "original", true),
    ("woodalternative", "original", true),
    ("wooden", "original", false),
];

/// `pathMapping`'s seven explicit entries, keyed by the style **key** and the
/// schematic name (`BlueprintMapping.java:113-121`).
///
/// They are what a legacy `…alternative` world gets instead of the `alt<name>`
/// generated variant: `woodalternative`'s shepherd is `shepherdalt`, its courier
/// is `deliverymanalt`, and its houses are `altresidence`. Two of the seven name
/// `farmeralt`, which **does not exist in the corpus** (0 files, measured) — the
/// fallback to the plain base is what saves them.
const OVERRIDES: &[(&str, &str, &str)] = &[
    ("caledoniaalternative", "farmer", "agriculture/horticulture/farmeralt"),
    ("caledoniaalternative", "guardtower", "military/guardtoweralt"),
    ("junglealternative", "farmer", "agriculture/horticulture/farmeralt"),
    ("woodalternative", "citizen", "fundamentals/altresidence"),
    ("woodalternative", "deliveryman", "craftsmanship/storage/deliverymanalt"),
    ("woodalternative", "shepherd", "agriculture/husbandry/shepherdalt"),
    ("woodalternative", "warehouse", "craftsmanship/storage/warehousealt"),
];

/// `pathMapping`'s style-independent entries (`BlueprintMapping.java:110-111`):
/// a citizen's house is the residence, and a deco scan named `home` too.
const DEFAULTS: &[(&str, &str)] = &[
    ("citizen", "fundamentals/residence"),
    ("home", "fundamentals/residence"),
];

/// A style: the key it was named by, the folder it lives in, and whether it asks
/// for the alternative variant.
///
/// Java keeps three things apart — the map key, the pack's display name
/// (`"Minecolonies Original"`, `:44-45`) and the flag its generators encode in
/// the name — and only the key and the flag ever reach the file system. This
/// type is those two plus the folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Style {
    key: &'static str,
    folder: &'static str,
    alternative: bool,
}

impl Style {
    /// The 23 styles of the corpus, in folder order.
    pub fn all() -> impl Iterator<Item = Style> {
        STYLE_FOLDERS.iter().map(|folder| Style::from_folder(*folder))
    }

    fn from_folder(folder: &'static str) -> Style {
        Style {
            key: folder,
            folder,
            alternative: false,
        }
    }

    /// The style a save's `style` key names: a folder, or one of the thirteen
    /// legacy keys. `None` for anything else — a mod's own style is not this
    /// mod's to guess.
    pub fn parse(name: &str) -> Option<Style> {
        for folder in STYLE_FOLDERS {
            if *folder == name {
                return Some(Style::from_folder(*folder));
            }
        }
        for (key, folder, alternative) in STYLE_ALIASES {
            if *key == name {
                return Some(Style {
                    key: *key,
                    folder: *folder,
                    alternative: *alternative,
                });
            }
        }
        None
    }

    /// The folder inside `data/minecolonies/structure/minecolonies/`.
    pub fn folder(&self) -> &'static str {
        self.folder
    }

    /// The name the save used: `stone` for a 1.12 world, `original` for a new one.
    ///
    /// Java keys its explicit table on this, and two worlds that resolve to the
    /// same folder may need different files (`BlueprintMapping.java:113-121`).
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// Whether this style asks for the `…alternative` variant first
    /// (`BlueprintMapping.java:46-55`).
    pub fn is_alternative(&self) -> bool {
        self.alternative
    }
}

/// One way of naming a file: a folder and a base name without its level.
///
/// An **empty folder** means "anywhere in the style": Java has no folder for
/// `gatehouse`, `kitchen`, `stable`, `postbox` or `stash`, and the corpus does
/// have three of them, so the caller scans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    /// The folder inside the style, or `""` for "any".
    pub folder: String,
    /// The file name without its level and without its extension.
    pub base: String,
}

impl Named {
    /// A name with an explicit folder.
    pub fn new(folder: &str, base: &str) -> Named {
        Named {
            folder: folder.to_owned(),
            base: base.to_owned(),
        }
    }

    /// A name read from a `folder/base` path, the shape `pathMapping` holds.
    pub fn from_path(path: &str) -> Named {
        match path.rsplit_once('/') {
            Some((folder, base)) => Named::new(folder, base),
            None => Named::new("", path),
        }
    }

    /// Whether the folder is unknown and the caller has to look anywhere.
    pub fn is_any_folder(&self) -> bool {
        self.folder.is_empty()
    }

    /// The path of one level's file inside the style, without the extension:
    /// `fundamentals/builder3`.
    pub fn file(&self, level: u8) -> String {
        if self.is_any_folder() {
            format!("{}{level}", self.base)
        } else {
            format!("{}/{}{level}", self.folder, self.base)
        }
    }

    /// The template id the engine reads: `minecolonies:original/fundamentals/builder3`,
    /// which is `data/minecolonies/structure/minecolonies/original/fundamentals/builder3.nbt`
    /// (`NOTES_TEMPLATE_NBT.md` §1.1).
    pub fn id(&self, style: &Style, level: u8) -> String {
        format!("{NAMESPACE}:{}/{}", style.folder(), self.file(level))
    }
}

/// The folder a schematic name lives in, or `None` when Java's table has no row.
pub fn folder_of(schematic: &str) -> Option<&'static str> {
    FOLDERS
        .iter()
        .find(|(name, _)| *name == schematic)
        .map(|(_, folder)| *folder)
}

/// `BlueprintMapping.getPathMapping(style, hut)` (`:140-143`), as a list.
///
/// The list is what the caller tries, in order, until a file answers:
///
/// 1. Java's explicit entry for `(style key, name)` — `woodalternative`'s
///    shepherd is `shepherdalt` (`:113-121`);
/// 2. for an `…alternative` style, the three spellings of the variant:
///    `alt<name>` (Java's own, `:150-159`), `<name>alt` and `<name>_alt` (both
///    measured in the corpus);
/// 3. the plain base name, in its folder, or anywhere when Java's table has none.
///
/// **Never empty**, and the last element is always the plain base: a style that
/// has no variant at all is not an error, it is the pre-1.14 behaviour.
pub fn candidates(style: &Style, schematic: &str) -> Vec<Named> {
    if let Some(path) = override_path(style.key(), schematic) {
        return vec![Named::from_path(path)];
    }

    let folder = folder_of(schematic);
    let make = |base: String| match folder {
        Some(folder) => Named::new(folder, &base),
        None => Named::new("", &base),
    };
    let mut out = Vec::with_capacity(4);
    if style.is_alternative() {
        // `runForAlt`: `entry.getKey() + "/" + "alt" + hut` (`:150-159`).
        out.push(make(format!("alt{schematic}")));
        // The corpus' other two spellings, both measured (see the module doc).
        out.push(make(format!("{schematic}alt")));
        out.push(make(format!("{schematic}_alt")));
    }
    out.push(make(schematic.to_owned()));
    out
}

/// The same, for a building kind: `candidates(style, kind.name())`.
pub fn named(style: &Style, kind: Kind) -> Vec<Named> {
    candidates(style, kind.name())
}

/// Java's explicit and default entries, one lookup.
fn override_path(style_key: &str, schematic: &str) -> Option<&'static str> {
    for (key, name, path) in OVERRIDES {
        if *key == style_key && *name == schematic {
            return Some(*path);
        }
    }
    DEFAULTS
        .iter()
        .find(|(name, _)| *name == schematic)
        .map(|(_, path)| *path)
}

/// The level a file name ends with: `Utils.getBlueprintLevel`
/// (`api/util/Utils.java:199-219`).
///
/// Java strips `.blueprint`, then matches **one** trailing digit and parses it.
/// So `builder3` is 3, `builder12` is **2** (the last digit only, a real quirk of
/// the mod), and `apiary` is `None` — `getBlueprintLevel` returns `-1` there and
/// `SurvivalHandler.java:270-277` uses that to mean "this is a decoration, ask
/// which level". The port keeps the quirk and the `None`.
pub fn level_of(file: &str) -> Option<u8> {
    let name = strip_extension(file);
    let last = name.chars().last()?;
    last.to_digit(10).map(|digit| digit as u8)
}

/// The name without its level and without its extension: the key an
/// [`super::Index`] files a level under.
pub fn strip_level(file: &str) -> &str {
    let name = strip_extension(file);
    match name.chars().last() {
        Some(last) if last.is_ascii_digit() => &name[..name.len() - 1],
        _ => name,
    }
}

/// The same path, retargeted to another level: `WorkOrderBuilding.java:63-65`.
///
/// Java does `path.replace(".blueprint", "")`, then writes `targetLevel` over the
/// last character — which is why the level has to be the last character of the
/// name. The port keeps the shape and refuses the two cases Java would corrupt:
/// a path that does not end in a digit ([`Refusal::NotALeveledPath`]) and a level
/// no single digit can carry ([`Refusal::NoSuchLevel`], 1..=9).
pub fn with_level(path: &str, level: u8) -> Result<String, Refusal> {
    let name = strip_extension(path);
    if level > 9 {
        return Err(Refusal::NoSuchLevel {
            base: strip_level(name).to_owned(),
            asked: level,
            lowest: MIN_LEVEL,
            highest: 9,
        });
    }
    if level < MIN_LEVEL {
        return Err(Refusal::NoSuchLevel {
            base: strip_level(name).to_owned(),
            asked: level,
            lowest: MIN_LEVEL,
            highest: 9,
        });
    }
    match name.chars().last() {
        Some(last) if last.is_ascii_digit() => {
            Ok(format!("{}{level}", &name[..name.len() - 1]))
        }
        _ => Err(Refusal::NotALeveledPath {
            path: name.to_owned(),
        }),
    }
}

/// The name without a container extension, `nbt` or `blueprint`.
fn strip_extension(file: &str) -> &str {
    if let Some(name) = file.strip_suffix(EXTENSION) {
        return name;
    }
    file.strip_suffix(".blueprint").unwrap_or(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_table_holds_forty_nine_of_the_fifty_four_kinds() {
        assert_eq!(FOLDERS.len(), 49, "`foldersToHuts` names 49 huts");
        let mut names: Vec<&str> = FOLDERS.iter().map(|(name, _)| *name).collect();
        let sorted = names.clone();
        names.sort_unstable();
        assert_eq!(names, sorted, "the table is stored sorted, lookups are a scan");
        names.dedup();
        assert_eq!(names.len(), 49, "a schematic has one folder, not two");
    }

    #[test]
    fn the_five_kinds_java_leaves_out_are_the_five_the_corpus_knows_best() {
        // `BlueprintMapping.java:57-72` has no row for these five.
        let missing: Vec<&str> = Kind::ALL
            .iter()
            .map(|kind| kind.name())
            .filter(|name| folder_of(name).is_none())
            .collect();
        assert_eq!(
            missing,
            vec!["gatehouse", "kitchen", "postbox", "stable", "stash"],
            "measured: the corpus has military/gatehouse, craftsmanship/luxury/kitchen \
             and military/stable, and no file at all for postbox or stash"
        );
    }

    #[test]
    fn every_kind_with_a_row_agrees_with_the_corpus_folder() {
        // Three folders of the corpus, chosen because each one is the only place
        // the building appears (measured with `find . -name '<kind>1.nbt'`).
        assert_eq!(folder_of("builder"), Some("fundamentals"));
        assert_eq!(folder_of("shepherd"), Some("agriculture/husbandry"));
        assert_eq!(folder_of("townhall"), Some("fundamentals"));
        assert_eq!(folder_of("simplequarry"), Some("infrastructure/mineshafts"));
        assert_eq!(folder_of("mystic"), None, "mystic is a folder, not a hut");
    }

    #[test]
    fn every_folder_of_the_table_is_a_folder_of_the_corpus() {
        // Measured on `data/minecolonies/structure/minecolonies/original/`:
        // agriculture, craftsmanship, decorations, education, fundamentals,
        // infrastructure, military, mystic, walls. No hut lives in `decorations`
        // or `walls`, and every folder below is one of the other seven (with a
        // sub-folder for the two that need one).
        const CATEGORIES: &[&str] = &[
            "agriculture",
            "craftsmanship",
            "education",
            "fundamentals",
            "infrastructure",
            "military",
            "mystic",
        ];
        for (name, folder) in FOLDERS {
            let first = folder.split('/').next().unwrap_or("");
            assert!(
                CATEGORIES.contains(&first),
                "{name} → {folder}: {first} is not a corpus folder"
            );
            assert!(
                !folder.starts_with('/') && !folder.ends_with('/'),
                "{folder} is a relative path inside a style"
            );
        }
    }

    #[test]
    fn the_twenty_three_folders_are_the_twenty_three_measured_ones() {
        let styles: Vec<&str> = Style::all().map(|style| style.folder()).collect();
        assert_eq!(styles.len(), 23);
        assert_eq!(Style::all().count(), 23);
        for folder in [
            "ancientathens",
            "cavern",
            "colonial",
            "shire",
            "original",
            "pagoda",
        ] {
            assert!(
                styles.contains(&folder),
                "{folder} is one of the corpus' folders"
            );
        }
    }

    #[test]
    fn a_legacy_key_and_a_folder_reach_the_same_pack() {
        // `styleMapping` (`BlueprintMapping.java:25-44`): stone and wooden are
        // both "Minecolonies Original", asian is "Pagoda".
        let stone = Style::parse("stone").expect("a known legacy key");
        let wooden = Style::parse("wooden").expect("a known legacy key");
        let asian = Style::parse("asian").expect("a known legacy key");
        assert_eq!(stone.folder(), "original");
        assert_eq!(wooden.folder(), "original");
        assert_eq!(asian.folder(), "pagoda");
        // The key is what the explicit table is keyed on, so it must survive.
        assert_eq!(stone.key(), "stone");
        assert_eq!(stone.folder(), Style::parse("original").unwrap().folder());
        // A folder parses to itself.
        let original = Style::parse("original").unwrap();
        assert_eq!(original.key(), "original");
        assert_eq!(original.folder(), "original");
        assert!(!original.is_alternative());
        // Nothing else is guessed: a pack this mod does not ship is not here.
        assert_eq!(Style::parse("mycustomstyle"), None);
        assert_eq!(Style::parse(""), None);
        assert_eq!(Style::parse("Original"), None, "the charset is lower case");
    }

    #[test]
    fn every_alias_is_alternative_or_not_and_none_shadows_a_folder() {
        for (key, folder, alternative) in STYLE_ALIASES {
            let style = Style::parse(key).unwrap_or_else(|| panic!("{key} does not parse"));
            assert_eq!(style.key(), *key);
            assert_eq!(style.folder(), *folder, "{key}");
            assert_eq!(style.is_alternative(), *alternative, "{key}");
            assert!(
                !STYLE_FOLDERS.contains(key),
                "{key} is a folder too: the two tables would disagree"
            );
            assert!(
                STYLE_FOLDERS.contains(folder),
                "{key} points at {folder}, which is not a folder of the corpus"
            );
        }
        assert_eq!(STYLE_ALIASES.len(), 13, "3 pre-1.14 keys, 10 alternative ones");
    }

    #[test]
    fn a_plain_style_has_exactly_one_candidate_and_it_is_the_base() {
        let original = Style::parse("original").unwrap();
        let list = named(&original, Kind::Builder);
        assert_eq!(list.len(), 1, "no variant to try");
        assert_eq!(list[0], Named::new("fundamentals", "builder"));
        // A builder in every style lands in fundamentals: measured, 23/23 styles
        // have `fundamentals/builder1.nbt`.
        for style in Style::all() {
            assert_eq!(
                named(&style, Kind::Builder)[0].folder,
                "fundamentals",
                "{style:?}"
            );
        }
    }

    #[test]
    fn an_alternative_style_tries_three_spellings_before_the_base() {
        let oak = Style::parse("medievaloakalternative").unwrap();
        assert!(oak.is_alternative());
        let list = named(&oak, Kind::Builder);
        assert_eq!(
            list,
            vec![
                // `runForAlt` (`BlueprintMapping.java:150-159`) — the file exists:
                // medievaloak/fundamentals/altbuilder1.nbt
                Named::new("fundamentals", "altbuilder"),
                // shire/fundamentals/builderalt1.nbt, sandstone/…/builderalt1.nbt
                Named::new("fundamentals", "builderalt"),
                // darkoak/military/barracks_alt1.nbt is the only one of these
                Named::new("fundamentals", "builder_alt"),
                // the base, always last
                Named::new("fundamentals", "builder"),
            ]
        );
        assert_eq!(list[0].file(1), "fundamentals/altbuilder1");
        assert_eq!(list[3].file(5), "fundamentals/builder5");
    }

    #[test]
    fn javas_explicit_entries_win_over_the_generated_variant() {
        // `woodalternative:shepherd` → `agriculture/husbandry/shepherdalt`
        // (`BlueprintMapping.java:114`), and the file exists in original/.
        let wood = Style::parse("woodalternative").unwrap();
        assert_eq!(
            named(&wood, Kind::Shepherd),
            vec![Named::new("agriculture/husbandry", "shepherdalt")]
        );
        // `woodalternative:citizen` → `fundamentals/altresidence` (`:113`); there
        // is no `citizen` kind here, so the entry is reachable by name only.
        assert_eq!(
            candidates(&wood, "citizen"),
            vec![Named::new("fundamentals", "altresidence")]
        );
        // `caledoniaalternative:guardtower` → `military/guardtoweralt` (`:121`),
        // which exists in caledonia: measured.
        let caledonia = Style::parse("caledoniaalternative").unwrap();
        assert_eq!(
            named(&caledonia, Kind::GuardTower),
            vec![Named::new("military", "guardtoweralt")]
        );
        // An explicit entry does not leak to the plain style of the same folder.
        let original = Style::parse("original").unwrap();
        assert_eq!(
            named(&original, Kind::Shepherd),
            vec![Named::new("agriculture/husbandry", "shepherd")]
        );
    }

    #[test]
    fn the_two_default_entries_are_the_residence() {
        // `pathMapping.put(":home", "fundamentals/residence")` (`:110-111`).
        let any = Style::parse("original").unwrap();
        assert_eq!(
            candidates(&any, "home"),
            vec![Named::new("fundamentals", "residence")]
        );
        assert_eq!(
            candidates(&any, "citizen"),
            vec![Named::new("fundamentals", "residence")]
        );
        // A default entry outranks the variant list, even in an alt style.
        let alt = Style::parse("stonealternative").unwrap();
        assert_eq!(
            candidates(&alt, "citizen"),
            vec![Named::new("fundamentals", "residence")],
            "`:citizen` is registered with the empty style key, and Java's \
             getOrDefault finds it before any generated row"
        );
    }

    #[test]
    fn a_kind_without_a_folder_still_gets_a_candidate_and_it_is_anywhere() {
        let original = Style::parse("original").unwrap();
        let list = named(&original, Kind::GateHouse);
        assert_eq!(list, vec![Named::new("", "gatehouse")]);
        assert!(list[0].is_any_folder());
        assert_eq!(list[0].file(3), "gatehouse3", "no folder to prefix");
        assert_eq!(list[0].id(&original, 3), "minecolonies:original/gatehouse3");
        // An alt style still asks for the variant first, in every spelling.
        let alt = Style::parse("woodalternative").unwrap();
        let alt_list = named(&alt, Kind::Stable);
        assert_eq!(
            alt_list
                .iter()
                .map(|named| named.base.as_str())
                .collect::<Vec<&str>>(),
            vec!["altstable", "stablealt", "stable_alt", "stable"]
        );
        assert!(alt_list.iter().all(|named| named.is_any_folder()));
    }

    #[test]
    fn the_level_is_the_last_digit_and_only_the_last_one() {
        // `Utils.getBlueprintLevel` (`:199-219`): `[0-9]$` on the stripped name.
        assert_eq!(level_of("fundamentals/builder1"), Some(1));
        assert_eq!(level_of("fundamentals/builder5.nbt"), Some(5));
        assert_eq!(level_of("fundamentals/builder12"), Some(2), "one digit only");
        assert_eq!(level_of("decorations/apiary"), None);
        assert_eq!(level_of("infrastructure/roads/basicroad4straight"), None);
        assert_eq!(level_of(""), None);
        // Java reads `.blueprint`, the corpus is `.nbt`; both are stripped.
        assert_eq!(level_of("fundamentals/builder4.blueprint"), Some(4));
    }

    #[test]
    fn the_base_name_is_the_name_without_its_level() {
        assert_eq!(strip_level("fundamentals/builder3"), "fundamentals/builder");
        assert_eq!(strip_level("fundamentals/builder3.nbt"), "fundamentals/builder");
        assert_eq!(strip_level("agriculture/husbandry/shepherdalt5"), "agriculture/husbandry/shepherdalt");
        // A name that does not end in a digit keeps all of it.
        assert_eq!(strip_level("decorations/apiary"), "decorations/apiary");
        assert_eq!(strip_level("decorations/apiary.nbt"), "decorations/apiary");
    }

    #[test]
    fn retargeting_a_path_writes_over_the_last_digit() {
        // `WorkOrderBuilding.java:63-65`.
        assert_eq!(
            with_level("fundamentals/builder1", 4).unwrap(),
            "fundamentals/builder4"
        );
        assert_eq!(
            with_level("fundamentals/builder1.nbt", 4).unwrap(),
            "fundamentals/builder4"
        );
        // A path that does not end in a digit: Java writes over the last
        // character and corrupts the name, the port refuses.
        assert_eq!(
            with_level("decorations/apiary", 1),
            Err(Refusal::NotALeveledPath {
                path: String::from("decorations/apiary")
            })
        );
        // A level no single digit can carry reads back as another level.
        assert_eq!(
            with_level("fundamentals/builder1", 10),
            Err(Refusal::NoSuchLevel {
                base: String::from("fundamentals/builder"),
                asked: 10,
                lowest: 1,
                highest: 9,
            })
        );
        assert_eq!(
            with_level("fundamentals/builder1", 0),
            Err(Refusal::NoSuchLevel {
                base: String::from("fundamentals/builder"),
                asked: 0,
                lowest: 1,
                highest: 9,
            }),
            "level 0 is a hut that has never been built, and no file is named 0"
        );
    }

    #[test]
    fn the_id_is_the_path_the_engine_reads() {
        // `data/minecolonies/structure/<ns>/<style>/<folder>/<base><level>.nbt`
        // is what `Game::read_template_tag` resolves (`NOTES_TEMPLATE_NBT.md` §1.1).
        let original = Style::parse("original").unwrap();
        let builder = named(&original, Kind::Builder);
        assert_eq!(
            builder[0].id(&original, 5),
            "minecolonies:original/fundamentals/builder5"
        );
        assert_eq!(
            format!("{}.nbt", builder[0].id(&original, 5).split_once(':').unwrap().1),
            "original/fundamentals/builder5.nbt",
            "the id is the path under data/minecolonies/structure/minecolonies/"
        );
        // The legacy key names a folder, not an id: a 1.12 save pastes the same
        // file, which is the entire point of the alias table.
        let stone = Style::parse("stone").unwrap();
        assert_eq!(
            named(&stone, Kind::TownHall)[0].id(&stone, 1),
            "minecolonies:original/fundamentals/townhall1"
        );
    }
}
