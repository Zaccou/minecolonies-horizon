//! Which of the 9 515 files answer a name — built once, asked many times.
//!
//! Java reference: there is no index in Java. `StructurePacks` lists a pack's
//! files and `StructurePacks.getBlueprintFuture(pack, path)` reads one by name
//! (`structurize/storage/StructurePacks.java:147-151`), so *finding* a blueprint
//! is a directory lookup the file system does. A wasm mod has no file system and
//! no clock, so the port needs the other shape: a list the caller read once, and
//! a lookup that answers from memory.
//!
//! **What the index adds over [`super::naming`].** The naming rule says what a
//! file *would* be called. The corpus says what exists: `tavern` stops at level 3
//! in all 23 styles, `gatehouse` at 3, `mediumquarry` at 1, and `cavern` is the
//! one style whose military buildings sit in a `default/` or `megahall/`
//! sub-folder instead of `military/` itself. [`Index`] is where "Java's name" and
//! "the file that is there" are reconciled, and where a level the pack does not
//! have becomes either a cap or a named refusal instead of a `null` blueprint.
//!
//! **The four answers**, in the order a resolution tries them:
//!
//! 1. the candidate's own folder, at the level asked for, or at the highest level
//!    below it — `capped` says which of the two answered;
//! 2. the same base name in **any** folder of the style, which is how a kind
//!    Java's table has no row for (`gatehouse`, `kitchen`, `stable`) and how
//!    `cavern` are found;
//! 3. the next candidate of [`super::naming::candidates`] (the alternative
//!    variant, then the plain base);
//! 4. a named [`Refusal`] — never a silent `None`, because a hut that does not
//!    appear has to say whether the style, the building or the level is at fault.
//!
//! **Ambiguity is refused, not resolved.** When two folders of one style hold the
//! same base, the port cannot know which file Java would have meant: `cavern`
//! carries `military/default/archery1.nbt` *and* `military/megahall/archery1.nbt`,
//! and Java's own table names neither. [`Refusal::Ambiguous`] names both folders
//! and the caller decides.

use std::collections::{BTreeMap, BTreeSet};

use crate::building::Kind;

use super::naming::{self, Named, Style};
use super::{EXTENSION, MIN_LEVEL, NAMESPACE, Refusal};

/// One file of the corpus, in the shape a lookup needs.
///
/// The size is carried here on purpose: pasting needs the box of the template
/// ([`super::place::Placing::boxed`]), and the size is a fact only the file has.
/// Measuring it once, when the index is built, is what lets a caller answer
/// "does this fit" without opening a file — which is exactly what a wasm mod
/// cannot do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    style: String,
    path: String,
    folder: String,
    base: String,
    level: u8,
    size: [u32; 3],
}

impl Entry {
    /// One file, as `style`, the path inside the style (`fundamentals/builder3`,
    /// with or without `.nbt`) and its size in blocks.
    ///
    /// A file whose name ends in no digit is **not** an entry: `Utils.getBlueprintLevel`
    /// returns `-1` for it (`api/util/Utils.java:218`), and every decoration,
    /// road and field of the corpus is one of those (1 691 of 9 515 files,
    /// measured). Refusing it is what keeps the index to levelled buildings —
    /// [`Refusal::NotALeveledPath`] says so rather than dropping the row.
    pub fn parse(style: &str, file: &str, size: [u32; 3]) -> Result<Entry, Refusal> {
        if style.is_empty() {
            return Err(Refusal::UnknownStyle {
                style: String::new(),
            });
        }
        let path = trim(file);
        let level = naming::level_of(path).ok_or_else(|| Refusal::NotALeveledPath {
            path: path.to_owned(),
        })?;
        if level < MIN_LEVEL {
            return Err(Refusal::NoSuchLevel {
                base: naming::strip_level(path).to_owned(),
                asked: level,
                lowest: MIN_LEVEL,
                highest: 9,
            });
        }
        let (folder, base) = match naming::strip_level(path).rsplit_once('/') {
            Some((folder, base)) => (folder.to_owned(), base.to_owned()),
            None => (String::new(), naming::strip_level(path).to_owned()),
        };
        Ok(Entry {
            style: style.to_owned(),
            path: path.to_owned(),
            folder,
            base,
            level,
            size,
        })
    }

    /// The style folder this file lives in.
    pub fn style(&self) -> &str {
        &self.style
    }

    /// The path inside the style, without the extension: `fundamentals/builder3`.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The folder inside the style, `""` when the caller gave a bare name.
    pub fn folder(&self) -> &str {
        &self.folder
    }

    /// The file name without its level and its folder: `builder`.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The level the name ends in (`Utils.getBlueprintLevel`).
    pub fn level(&self) -> u8 {
        self.level
    }

    /// The template's size in blocks, as the `.nbt` stores it: `[x, y, z]`,
    /// unrotated. Measured, never guessed — `size` is the first tag of the file
    /// (`NOTES_TEMPLATE_NBT.md` §1.3).
    pub fn size(&self) -> [u32; 3] {
        self.size
    }

    /// The template id the engine resolves:
    /// `minecolonies:original/fundamentals/builder3`, i.e.
    /// `data/minecolonies/structure/minecolonies/original/fundamentals/builder3.nbt`.
    pub fn id(&self) -> String {
        format!("{NAMESPACE}:{}/{}", self.style, self.path)
    }

    /// The key a resolution files this entry under: `folder/base`, or `base`
    /// when the file sits at the root of a style.
    fn key(&self) -> String {
        if self.folder.is_empty() {
            self.base.clone()
        } else {
            format!("{}/{}", self.folder, self.base)
        }
    }
}

/// A name that answered, and what it cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved<'a> {
    /// The file itself.
    pub entry: &'a Entry,
    /// The level that was asked for.
    pub asked: u8,
    /// True when the answer is a variant (`altbuilder`, `builderalt`, …) rather
    /// than the plain base name.
    pub variant: bool,
}

impl<'a> Resolved<'a> {
    /// The level of the file that answered, which may be below [`Self::asked`].
    pub fn level(&self) -> u8 {
        self.entry.level()
    }

    /// True when the pack has no file at [`Self::asked`] and answered with the
    /// highest level below it instead. Java's own answer in that case is a `null`
    /// blueprint and a hidden *Build* button (`WindowBuildBuilding.java:323-329`);
    /// the port keeps the file and says it capped.
    pub fn capped(&self) -> bool {
        self.entry.level() < self.asked
    }

    /// The id to paste: the answering file, not the one that was asked for.
    pub fn id(&self) -> String {
        self.entry.id()
    }

    /// Whether the file that answered is a variant (`altbuilder`, `builderalt`,
    /// `barracks_alt`) rather than the plain base name.
    ///
    /// The field of the same name carries the answer; this is the accessor the
    /// resolution and its callers read, so `variant` stays one piece of data with
    /// one way in.
    pub fn variant(&self) -> bool {
        self.variant
    }
}

/// The corpus, filed by style and by name. Built once (9515 files in, a lookup
/// out), and never written to afterwards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    /// `(style, "folder/base")` → level → file.
    by_path: BTreeMap<(String, String), BTreeMap<u8, Entry>>,
    /// `(style, base)` → the folders of that style holding that base name.
    by_base: BTreeMap<(String, String), BTreeSet<String>>,
    /// Every style with at least one file.
    styles: BTreeSet<String>,
}

impl Index {
    /// An index with no file in it. Every question about a style is then
    /// [`Refusal::UnknownStyle`], which is the honest answer for a corpus nobody
    /// has read.
    pub fn new() -> Index {
        Index::default()
    }

    /// Build the index from `(style, file, size)` triples the caller read.
    ///
    /// The first file that cannot be filed stops the build: a half-built index
    /// would answer confidently for the rows it got and "unknown structure" for
    /// the rest, which is the one outcome a caller cannot tell from a real miss.
    pub fn from_files<I>(files: I) -> Result<Index, Refusal>
    where
        I: IntoIterator<Item = (String, String, [u32; 3])>,
    {
        let mut index = Index::new();
        for (style, file, size) in files {
            index.insert(Entry::parse(&style, &file, size)?);
        }
        Ok(index)
    }

    /// File one entry. The same `(style, path)` twice replaces: a caller that
    /// re-reads a folder does not double its buildings.
    pub fn insert(&mut self, entry: Entry) {
        let style = entry.style.clone();
        let base = entry.base.clone();
        let folder = entry.folder.clone();
        let key = entry.key();
        let level = entry.level;
        self.styles.insert(style.clone());
        self.by_base
            .entry((style.clone(), base))
            .or_default()
            .insert(folder);
        self.by_path
            .entry((style, key))
            .or_default()
            .insert(level, entry);
    }

    /// How many files were filed.
    pub fn len(&self) -> usize {
        self.by_path.values().map(|levels| levels.len()).sum()
    }

    /// True when no file was filed.
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// Every style the index holds, sorted.
    pub fn styles(&self) -> impl Iterator<Item = &'_ str> + '_ {
        self.styles.iter().map(String::as_str)
    }

    /// The answer for a building kind: the whole point of the module.
    ///
    /// `level` is the level that will be pasted. A level the pack does not have is
    /// capped to the highest below it ([`Resolved::capped`]), never rounded up: a
    /// tavern has no level 4 file in the corpus, and pasting `tavern3` while
    /// claiming level 4 would make the building's box lie about its own content.
    pub fn resolve(&self, style: &Style, kind: Kind, level: u8) -> Result<Resolved<'_>, Refusal> {
        self.resolve_named(style, kind.name(), level)
    }

    /// The same, for a name rather than a kind — `home` and `citizen` are
    /// `pathMapping` entries with no building behind them
    /// (`BlueprintMapping.java:110-111`).
    pub fn resolve_named(
        &self,
        style: &Style,
        schematic: &str,
        level: u8,
    ) -> Result<Resolved<'_>, Refusal> {
        if !self.styles.contains(style.folder()) {
            return Err(Refusal::UnknownStyle {
                style: style.folder().to_owned(),
            });
        }

        let list = naming::candidates(style, schematic);
        let last = list.len() - 1;
        for (index, candidate) in list.iter().enumerate() {
            let variant = index != last;
            // 1. the folder the naming rule names.
            if let Some(entry) = self.exact(style.folder(), candidate, level) {
                return Ok(Resolved {
                    entry,
                    asked: level,
                    variant,
                });
            }
            // 2. the same base anywhere in the style: the only way to reach the
            //    kinds Java's table leaves out, and the only way into `cavern`.
            if let Some(entry) = self.anywhere(style.folder(), &candidate.base, level)? {
                return Ok(Resolved {
                    entry,
                    asked: level,
                    variant,
                });
            }
        }

        // Nothing answered. Say whether the building or the level is at fault.
        let base = naming::strip_level(schematic).to_owned();
        match self.level_range(style.folder(), &base) {
            Some((lowest, highest)) => Err(Refusal::NoSuchLevel {
                base,
                asked: level,
                lowest,
                highest,
            }),
            None => Err(Refusal::UnknownStructure {
                style: style.folder().to_owned(),
                name: schematic.to_owned(),
                level,
            }),
        }
    }

    /// The levels the style actually has for this kind, lowest first — what a
    /// *Build* or *Upgrade* button needs to know before it is shown
    /// (`WindowBuildBuilding.java:310-329` asks the pack the same question one
    /// level at a time).
    pub fn levels(&self, style: &Style, kind: Kind) -> Result<Vec<u8>, Refusal> {
        if !self.styles.contains(style.folder()) {
            return Err(Refusal::UnknownStyle {
                style: style.folder().to_owned(),
            });
        }
        for candidate in naming::candidates(style, kind.name()) {
            if !candidate.is_any_folder() {
                let key = (style.folder().to_owned(), format!("{}/{}", candidate.folder, candidate.base));
                if let Some(levels) = self
                    .by_path
                    .get(&key)
                    .map(|levels| levels.keys().copied().collect())
                {
                    return Ok(levels);
                }
            }
            for folder in self.folders_of(style.folder(), &candidate.base) {
                if let Some(levels) = self
                    .by_path
                    .get(&(style.folder().to_owned(), format!("{folder}/{}", candidate.base)))
                    .map(|levels| levels.keys().copied().collect())
                {
                    return Ok(levels);
                }
            }
        }
        Err(Refusal::UnknownStructure {
            style: style.folder().to_owned(),
            name: kind.name().to_owned(),
            level: MIN_LEVEL,
        })
    }

    /// The best file for a candidate in the candidate's own folder.
    fn exact(&self, style: &str, candidate: &Named, level: u8) -> Option<&Entry> {
        if candidate.is_any_folder() {
            return None;
        }
        let key = (style.to_owned(), format!("{}/{}", candidate.folder, candidate.base));
        self.by_path.get(&key)?.range(..=level).next_back().map(|(_, e)| e)
    }

    /// The best file for a base name anywhere in the style, refusing to pick
    /// between two folders.
    fn anywhere(
        &self,
        style: &str,
        base: &str,
        level: u8,
    ) -> Result<Option<&Entry>, Refusal> {
        let folders = self.folders_of(style, base);
        if folders.len() > 1 {
            return Err(Refusal::Ambiguous {
                style: style.to_owned(),
                base: base.to_owned(),
                first: folders[0].clone(),
                second: folders[1].clone(),
            });
        }
        let Some(folder) = folders.first() else {
            return Ok(None);
        };
        let key = (style.to_owned(), format!("{folder}/{base}"));
        let Some(levels) = self.by_path.get(&key) else {
            return Ok(None);
        };
        Ok(levels.range(..=level).next_back().map(|(_, entry)| entry))
    }

    /// Every folder of `style` holding a file named `base`, sorted.
    fn folders_of(&self, style: &str, base: &str) -> Vec<String> {
        self.by_base
            .get(&(style.to_owned(), base.to_owned()))
            .map(|folders| folders.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The lowest and highest level the style has for this base, over every
    /// folder holding it.
    fn level_range(&self, style: &str, base: &str) -> Option<(u8, u8)> {
        let folders = self.folders_of(style, base);
        let mut range: Option<(u8, u8)> = None;
        for folder in folders {
            if let Some(levels) = self
                .by_path
                .get(&(style.to_owned(), format!("{folder}/{base}")))
            {
                for level in levels.keys() {
                    range = Some(match range {
                        Some((low, high)) => (low.min(*level), high.max(*level)),
                        None => (*level, *level),
                    });
                }
            }
        }
        range
    }
}

/// The path as the corpus writes it: no leading separator, no `./`, no extension.
fn trim(file: &str) -> &str {
    let path = file.trim_start_matches("./").trim_start_matches('/');
    path.strip_suffix(EXTENSION)
        .or_else(|| path.strip_suffix(".blueprint"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The style every hand-written test below works in, unless it says otherwise.
    fn original() -> Style {
        Style::parse("original").unwrap()
    }

    /// A corpus of four styles and eight files, with real sizes measured from
    /// the converted blueprints.
    ///
    /// | file | size | measured in |
    /// | --- | --- | --- |
    /// | `original/fundamentals/builder1.nbt` | `[15, 8, 13]` | that file |
    /// | `original/fundamentals/builder5.nbt` | `[15, 8, 13]` | that file |
    /// | `original/fundamentals/tavern1.nbt` | `[13, 14, 15]` | that file |
    /// | `original/fundamentals/tavern3.nbt` | `[13, 14, 15]` | that file |
    /// | `original/military/gatehouse1.nbt` | `[16, 15, 7]` | that file |
    /// | `original/military/gatehouse3.nbt` | `[16, 15, 7]` | that file |
    /// | `original/military/stable1.nbt` | `[16, 15, 23]` | that file |
    /// | `cavern/military/default/archery1.nbt` | `[19, 8, 28]` | that file |
    /// | `cavern/military/megahall/archery1.nbt` | `[19, 8, 28]` | that file |
    fn index() -> Index {
        let mut index = Index::new();
        for (style, file, size) in [
            ("original", "fundamentals/builder1", [15, 8, 13]),
            ("original", "fundamentals/builder2", [15, 8, 13]),
            ("original", "fundamentals/builder5", [15, 8, 13]),
            ("original", "fundamentals/tavern1", [13, 14, 15]),
            ("original", "fundamentals/tavern3", [13, 14, 15]),
            ("original", "military/gatehouse1", [16, 15, 7]),
            ("original", "military/gatehouse3", [16, 15, 7]),
            ("original", "military/stable1", [16, 15, 23]),
            ("cavern", "military/default/archery1", [19, 8, 28]),
            ("cavern", "military/megahall/archery1", [19, 8, 28]),
        ] {
            index.insert(Entry::parse(style, file, size).unwrap());
        }
        index
    }

    #[test]
    fn an_entry_files_the_name_the_folder_and_the_level() {
        let entry = Entry::parse("original", "fundamentals/builder5.nbt", [15, 8, 13]).unwrap();
        assert_eq!(entry.style(), "original");
        assert_eq!(entry.path(), "fundamentals/builder5");
        assert_eq!(entry.folder(), "fundamentals");
        assert_eq!(entry.base(), "builder");
        assert_eq!(entry.level(), 5);
        assert_eq!(entry.size(), [15, 8, 13]);
        assert_eq!(entry.id(), "minecolonies:original/fundamentals/builder5");
        // The id is the on-disk path, and a test that only compared two strings
        // the same code produced would prove nothing — so compare the layout.
        assert_eq!(
            format!(
                "data/{}/structure/{}/{}.nbt",
                NAMESPACE,
                entry.style(),
                entry.path()
            ),
            "data/minecolonies/structure/original/fundamentals/builder5.nbt"
        );
    }

    #[test]
    fn a_file_without_a_trailing_digit_is_not_filed() {
        // 1 691 of the corpus' 9 515 files are decorations, roads and fields:
        // `Utils.getBlueprintLevel` returns -1 for them (`Utils.java:218`).
        assert_eq!(
            Entry::parse("original", "decorations/apiary.nbt", [4, 3, 4]),
            Err(Refusal::NotALeveledPath {
                path: String::from("decorations/apiary")
            })
        );
        assert_eq!(
            Entry::parse("original", "infrastructure/roads/basicroad4straight.nbt", [1, 1, 1]),
            Err(Refusal::NotALeveledPath {
                path: String::from("infrastructure/roads/basicroad4straight")
            })
        );
        // A leading slash and a `./` are the same path.
        let entry = Entry::parse("original", "./fundamentals/builder1.nbt", [15, 8, 13]).unwrap();
        assert_eq!(entry.path(), "fundamentals/builder1");
        // An empty style never files: a file with no pack cannot be pasted.
        assert!(Entry::parse("", "fundamentals/builder1", [1, 1, 1]).is_err());
    }

    #[test]
    fn the_index_counts_what_it_filed_and_knows_its_styles() {
        let index = index();
        assert_eq!(index.len(), 10);
        assert!(!index.is_empty());
        let styles: Vec<&str> = index.styles().collect();
        assert_eq!(styles, vec!["cavern", "original"], "sorted, deduplicated");
        assert!(Index::new().is_empty());
        assert_eq!(Index::new().len(), 0);
    }

    #[test]
    fn the_exact_level_answers_and_the_variant_flag_is_false() {
        let index = index();
        let resolved = index.resolve(&original(), Kind::Builder, 2).unwrap();
        assert_eq!(resolved.level(), 2);
        assert!(!resolved.capped());
        assert!(!resolved.variant(), "the plain base is not a variant");
        assert_eq!(resolved.id(), "minecolonies:original/fundamentals/builder2");
        assert_eq!(resolved.entry.size(), [15, 8, 13], "the file's own size");
    }

    #[test]
    fn a_level_the_pack_does_not_have_is_capped_down_and_says_so() {
        let index = index();
        // Measured: `tavern` has levels 1..3 in all 23 styles.
        let resolved = index.resolve(&original(), Kind::Tavern, 4).unwrap();
        assert_eq!(resolved.asked, 4);
        assert_eq!(resolved.level(), 3);
        assert!(resolved.capped(), "a tavern has no level 4 file");
        assert_eq!(resolved.id(), "minecolonies:original/fundamentals/tavern3");
        // Level 0 is a hut that has never been built, and no file is named 0.
        assert_eq!(
            index.resolve(&original(), Kind::Tavern, 0),
            Err(Refusal::NoSuchLevel {
                base: String::from("tavern"),
                asked: 0,
                lowest: 1,
                highest: 3,
            })
        );
    }

    #[test]
    fn a_kind_java_has_no_folder_for_is_found_anywhere_in_the_style() {
        let index = index();
        // `folder_of("gatehouse")` is None (`BlueprintMapping.java:57-72`), and
        // the corpus keeps it in military/: `original/military/gatehouse3.nbt`.
        assert_eq!(naming::folder_of("gatehouse"), None);
        let resolved = index.resolve(&original(), Kind::GateHouse, 3).unwrap();
        assert_eq!(
            resolved.entry.path(),
            "military/gatehouse3",
            "found by base name, in whatever folder the style used"
        );
        // Level 4 does not exist: `BuildingGateHouse.MAX_LEVEL = 3`
        // (`BuildingGateHouse.java:34`), and the corpus agrees — 19 styles, 3
        // levels each.
        assert!(index.resolve(&original(), Kind::GateHouse, 4).unwrap().capped());
        // Same for the stable, which Java's table also leaves out.
        assert_eq!(
            index.resolve(&original(), Kind::Stable, 1).unwrap().entry.path(),
            "military/stable1"
        );
    }

    #[test]
    fn two_folders_of_one_style_are_refused_rather_than_guessed() {
        let index = index();
        let cavern = Style::parse("cavern").unwrap();
        // `cavern` is the one style with a `default/` and a `megahall/` military
        // folder (measured), and its `military/archery1.nbt` does not exist.
        assert_eq!(
            index.resolve(&cavern, Kind::Archery, 1),
            Err(Refusal::Ambiguous {
                style: String::from("cavern"),
                base: String::from("archery"),
                first: String::from("military/default"),
                second: String::from("military/megahall"),
            })
        );
    }

    #[test]
    fn a_style_with_no_file_in_it_is_unknown_not_empty() {
        let index = index();
        let sherwood = Style::parse("spacewars").unwrap();
        assert_eq!(
            index.resolve(&sherwood, Kind::Builder, 1),
            Err(Refusal::UnknownStyle {
                style: String::from("spacewars")
            })
        );
        assert!(Index::new().resolve(&original(), Kind::Builder, 1).is_err());
    }

    #[test]
    fn a_building_the_style_does_not_have_is_unknown_not_level_less() {
        let index = index();
        // `original` has no kitchen in this corpus (the real one has it in 5 of
        // 23 styles: ancientathens, caledonia, darkoak, fortress, pagoda).
        assert_eq!(
            index.resolve(&original(), Kind::Kitchen, 1),
            Err(Refusal::UnknownStructure {
                style: String::from("original"),
                name: String::from("kitchen"),
                level: 1,
            })
        );
        // A name with no building behind it, and no default entry either.
        assert!(index.resolve_named(&original(), "castle", 1).is_err());
    }

    #[test]
    fn the_levels_of_a_kind_are_the_levels_the_pack_has() {
        let index = index();
        // builder: 1, 2, 5 in this corpus — the list is what exists, not 1..=max.
        assert_eq!(
            index.levels(&original(), Kind::Builder).unwrap(),
            vec![1, 2, 5]
        );
        assert_eq!(
            index.levels(&original(), Kind::Tavern).unwrap(),
            vec![1, 3],
            "measured: tavern stops at 3"
        );
        assert_eq!(
            index.levels(&original(), Kind::GateHouse).unwrap(),
            vec![1, 3]
        );
        assert_eq!(
            index.levels(&original(), Kind::Kitchen),
            Err(Refusal::UnknownStructure {
                style: String::from("original"),
                name: String::from("kitchen"),
                level: MIN_LEVEL,
            })
        );
    }

    #[test]
    fn reading_the_same_folder_twice_does_not_double_the_buildings() {
        let mut index = Index::new();
        let first = Entry::parse("original", "fundamentals/builder1", [15, 8, 13]).unwrap();
        index.insert(first);
        assert_eq!(index.len(), 1);
        // A second read of the same path replaces, and keeps the new size: the
        // file was re-converted, not duplicated.
        let again = Entry::parse("original", "fundamentals/builder1.nbt", [16, 9, 14]).unwrap();
        index.insert(again);
        assert_eq!(index.len(), 1);
        assert_eq!(
            index.resolve(&original(), Kind::Builder, 1).unwrap().entry.size(),
            [16, 9, 14]
        );
    }

    #[test]
    fn an_index_is_built_from_files_and_refuses_the_first_bad_row() {
        let good = Index::from_files([
            (
                String::from("original"),
                String::from("fundamentals/builder1"),
                [15, 8, 13],
            ),
            (
                String::from("original"),
                String::from("fundamentals/builder2"),
                [15, 8, 13],
            ),
        ])
        .unwrap();
        assert_eq!(good.len(), 2);
        // One decoration in the middle of the list stops the build: a half index
        // answers "unknown structure" for a building it simply never read.
        let bad = Index::from_files([
            (
                String::from("original"),
                String::from("fundamentals/builder1"),
                [15, 8, 13],
            ),
            (
                String::from("original"),
                String::from("decorations/apiary"),
                [4, 3, 4],
            ),
        ]);
        assert_eq!(
            bad,
            Err(Refusal::NotALeveledPath {
                path: String::from("decorations/apiary")
            })
        );
    }

    #[test]
    fn an_alternative_style_answers_with_the_variant_and_flags_it() {
        let mut index = Index::new();
        // `original/fundamentals/altresidence1.nbt` exists (8 styles have it).
        index.insert(Entry::parse("original", "fundamentals/residence1", [14, 17, 15]).unwrap());
        index.insert(Entry::parse("original", "fundamentals/altresidence1", [15, 18, 16]).unwrap());
        // No override for `residence` in `woodalternative`, so the generated
        // `altresidence` is tried first (`BlueprintMapping.java:150-159`).
        let wood = Style::parse("woodalternative").unwrap();
        let resolved = index.resolve(&wood, Kind::Residence, 1).unwrap();
        assert!(resolved.variant());
        assert_eq!(resolved.entry.path(), "fundamentals/altresidence1");
        // The plain style still reads the plain residence.
        let plain = index.resolve(&original(), Kind::Residence, 1).unwrap();
        assert!(!plain.variant());
        assert_eq!(plain.entry.path(), "fundamentals/residence1");
    }
}
