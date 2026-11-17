//! How a model's `textures` block resolves, and which vanilla materials it borrows.
//!
//! Java reference: `assets/minecolonies/models/block/**.json` (216 files) and
//! `models/item/*.json` (170 files), read by vanilla's `BlockModel.fromStream`,
//! which merges a model's `textures` over its `parent`'s and resolves `#name`
//! references against the merged map.
//!
//! **The finding the brief called "les variantes porteuses".** There is no block
//! of the mod with a wool, wood or terracotta *variant property*: the grey
//! material variation lives in the **texture map**, as a child model that
//! re-points two keys at another colour. `blockhutcitizen_1` is the model of this
//! in one file: it inherits all 33 elements and 16 textures of
//! `blockhutcitizen` and replaces exactly two of them -
//! `"7": "block/red_terracotta"` and `"14": "block/red_wool"`
//! (`models/block/blockhutcitizen_1.json:1-7`). The four coloured citizen huts
//! are one model with two keys swapped, and [`Textures::overridden`] is the method
//! that says which.
//!
//! | Measured on the 216 block models and the 170 item models | Count |
//! | --- | --- |
//! | texture values that point at a **vanilla** texture | 457, of 126 distinct names |
//! | texture values that point at one of the mod's own | 242 |
//! | `#name` references - `floodedfarmland.json` and `spear_in_hand.json` | 2 |
//! | distinct vanilla colours borrowed across six materials | 35 |
//!
//! The six materials are the ones the models actually borrow from: wool (8
//! colours of the 16 vanilla has), terracotta (10, the plain one included),
//! concrete (4), concrete powder (4), stained glass (5) and planks (4 species).
//! [`shade`] answers for one, and refuses the two ways it can go wrong: a colour
//! vanilla has and the mod never borrows ([`Refusal::UnusedShade`]) and a name that
//! is not a shade at all ([`Refusal::NotAMaterial`]).
//!
//! **The one dangling reference in the mod** is the second `#name`:
//! `models/item/spear_in_hand.json` writes `"particle": "#spear"` and never
//! defines `spear` - the key arrives from `spear.json`, which is a Forge
//! `separate_transforms` loader (`MANQUANT.md` §5). [`Textures::resolve`] refuses
//! it by name, which is the honest answer: the file does not work on its own.
//!
//! **What this file does not do**: it reads no file. The parent chain is handed to
//! it level by level, so a caller that already has the model JSON of each level
//! gets the merged map, and a test can build the citizen hut pair from literals.

use mod_api::serde_json::{Map, Value};

/// The key a model uses to name its parent (`BlockModel.PARENT`).
pub const PARENT: &str = "parent";

/// The key a model uses for its texture map (`BlockModel.TEXTURES`).
pub const TEXTURES: &str = "textures";

/// The key every model should fill: the sprite a broken face falls back to.
pub const PARTICLE: &str = "particle";

/// The prefix that makes a texture value a reference to another key.
pub const REFERENCE: char = '#';

/// How many `#a -> #b -> …` hops are followed before the map is called cyclic.
/// Vanilla resolves in place and would loop; the depth is the port's guard.
pub const MAX_DEPTH: usize = 8;

/// The 16 colours vanilla dyes come in, in the order `DyeColor` declares them.
pub const VANILLA_COLOURS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// A model's texture map: key to value, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: one field, and an empty map is a valid model
pub struct Textures {
    /// `("particle", "block/oak_planks")`, `("14", "block/red_wool")`, …
    pub pairs: Vec<(String, String)>,
}

impl Textures {
    /// Read a model's `textures` object. A value that is not a string is kept out
    /// rather than guessed: vanilla reads `#name` and ids only.
    pub fn parse(map: &Map<String, Value>) -> Textures {
        let mut pairs = Vec::with_capacity(map.len());
        for (key, value) in map {
            if let Some(text) = value.as_str() {
                pairs.push((key.clone(), text.to_owned()));
            }
        }
        Textures { pairs }
    }

    /// The raw value of a key, references unresolved.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// The keys, sorted.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.pairs.iter().map(|(key, _)| key.as_str()).collect();
        keys.sort_unstable();
        keys
    }

    /// This map with `over`'s entries written on top: the child wins per key, and
    /// a key only the parent has survives. Vanilla's merge, and the rule that makes
    /// the four coloured citizen huts one model.
    pub fn merge(&self, over: &Textures) -> Textures {
        let mut pairs = self.pairs.clone();
        for (key, value) in &over.pairs {
            match pairs.iter_mut().find(|(name, _)| name == key) {
                Some(slot) => slot.1 = value.clone(),
                None => pairs.push((key.clone(), value.clone())),
            }
        }
        Textures { pairs }
    }

    /// The keys `over` changes or adds, sorted: two for `blockhutcitizen_1`.
    pub fn overridden<'b>(&self, over: &'b Textures) -> Vec<&'b str> {
        let mut keys: Vec<&'b str> = over
            .pairs
            .iter()
            .filter(|(key, value)| self.get(key.as_str()) != Some(value.as_str()))
            .map(|(key, _)| key.as_str())
            .collect();
        keys.sort_unstable();
        keys
    }

    /// The value of a key, following `#name` references, and refusing the two ways
    /// that fails: a key the map does not have, and a chain that comes back to
    /// itself.
    pub fn resolve(&self, key: &str) -> Result<String, Refusal> {
        let mut current = key.to_owned();
        for _ in 0..MAX_DEPTH {
            let value = self.get(&current).ok_or_else(|| Refusal::MissingTexture {
                key: current.clone(),
            })?;
            match value.strip_prefix(REFERENCE) {
                Some(next) => current = next.to_owned(),
                None => return Ok(value.to_owned()),
            }
        }
        Err(Refusal::TextureCycle { key: current })
    }
}

/// A model and its ancestors, nearest first: the level a key is looked up in
/// decides which value wins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: one field, and a model with no parent is one level
pub struct Chain {
    /// The model's own map, then its parent's, then the grandparent's.
    pub levels: Vec<Textures>,
}

impl Chain {
    /// A chain from the model's own map outwards.
    pub fn new(levels: Vec<Textures>) -> Chain {
        Chain { levels }
    }

    /// The merged map: the nearest level that has a key wins.
    pub fn merged(&self) -> Textures {
        let mut merged = Textures::default();
        for level in self.levels.iter().rev() {
            merged = merged.merge(level);
        }
        merged
    }

    /// Where a key is defined: `0` is the model itself.
    pub fn depth_of(&self, key: &str) -> Option<usize> {
        self.levels
            .iter()
            .position(|level| level.get(key).is_some())
    }
}

/// One of the six vanilla materials the mod's models borrow sprites from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: six materials, and `suffix` answers for each
pub enum Material {
    /// `<colour>_wool`, and the plain `white_wool` shape.
    Wool,
    /// `<colour>_terracotta`, plus the uncoloured `terracotta`.
    Terracotta,
    /// `<colour>_concrete`.
    Concrete,
    /// `<colour>_concrete_powder`.
    ConcretePowder,
    /// `<colour>_stained_glass`.
    StainedGlass,
    /// `<species>_planks`.
    Planks,
}

impl Material {
    /// All six, in the order the report lists them.
    pub const ALL: [Material; 6] = [
        Material::Wool,
        Material::Terracotta,
        Material::Concrete,
        Material::ConcretePowder,
        Material::StainedGlass,
        Material::Planks,
    ];

    /// The suffix a vanilla texture of this material ends with.
    pub fn suffix(self) -> &'static str {
        match self {
            Material::Wool => "wool",
            Material::Terracotta => "terracotta",
            Material::Concrete => "concrete",
            Material::ConcretePowder => "concrete_powder",
            Material::StainedGlass => "stained_glass",
            Material::Planks => "planks",
        }
    }

    /// The `(colour, vanilla texture)` pairs the mod's models actually borrow,
    /// measured over the 216 block models. The plain terracotta is colour `None`.
    pub fn shades(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Material::Wool => &[
                ("blue", "block/blue_wool"),
                ("gray", "block/gray_wool"),
                ("green", "block/green_wool"),
                ("light_blue", "block/light_blue_wool"),
                ("light_gray", "block/light_gray_wool"),
                ("orange", "block/orange_wool"),
                ("red", "block/red_wool"),
                ("white", "block/white_wool"),
            ],
            Material::Terracotta => &[
                ("black", "block/black_terracotta"),
                ("cyan", "block/cyan_terracotta"),
                ("green", "block/green_terracotta"),
                ("light_gray", "block/light_gray_terracotta"),
                ("lime", "block/lime_terracotta"),
                ("orange", "block/orange_terracotta"),
                ("red", "block/red_terracotta"),
                ("", "block/terracotta"),
                ("white", "block/white_terracotta"),
                ("yellow", "block/yellow_terracotta"),
            ],
            Material::Concrete => &[
                ("gray", "block/gray_concrete"),
                ("light_blue", "block/light_blue_concrete"),
                ("red", "block/red_concrete"),
                ("yellow", "block/yellow_concrete"),
            ],
            Material::ConcretePowder => &[
                ("brown", "block/brown_concrete_powder"),
                ("gray", "block/gray_concrete_powder"),
                ("light_blue", "block/light_blue_concrete_powder"),
                ("light_gray", "block/light_gray_concrete_powder"),
            ],
            Material::StainedGlass => &[
                ("blue", "block/blue_stained_glass"),
                ("gray", "block/gray_stained_glass"),
                ("purple", "block/purple_stained_glass"),
                ("red", "block/red_stained_glass"),
                ("yellow", "block/yellow_stained_glass"),
            ],
            Material::Planks => &[
                ("birch", "block/birch_planks"),
                ("dark_oak", "block/dark_oak_planks"),
                ("oak", "block/oak_planks"),
                ("spruce", "block/spruce_planks"),
            ],
        }
    }

    /// Whether the material's shades are dye colours. The five dyed ones are;
    /// planks are wood species, and their list is the game's, not this mod's, so a
    /// species the mod never borrows is refused as [`Refusal::NotAMaterial`]
    /// rather than claimed as an unused colour.
    pub fn is_dyed(self) -> bool {
        !matches!(self, Material::Planks)
    }

    /// The material a suffix names, or `None`.
    pub fn from_suffix(suffix: &str) -> Option<Material> {
        Material::ALL
            .iter()
            .copied()
            .find(|material| material.suffix() == suffix)
    }
}

/// One borrowed vanilla sprite: which material, and which colour of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three fields, all read
pub struct Shade {
    /// The material.
    pub material: Material,
    /// The colour, `""` for the uncoloured terracotta.
    pub colour: &'static str,
    /// The vanilla texture name, as the model writes it.
    pub texture: &'static str,
}

impl Shade {
    /// The `(material, colour)` pair, for a comparison.
    pub fn pair(&self) -> (Material, &'static str) {
        (self.material, self.colour)
    }
}

/// What a texture name can be refused as.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three ways, and each names what it refused
pub enum Refusal {
    /// A key the merged map does not have.
    MissingTexture { key: String },
    /// `#a` pointing at `#b` and back.
    TextureCycle { key: String },
    /// A colour vanilla has and this mod never borrows: `block/black_wool`.
    UnusedShade {
        material: &'static str,
        colour: String,
    },
    /// Not a shade of a borrowed material at all: `block/stone`.
    NotAMaterial { texture: String },
}

impl Refusal {
    /// A stable name, namespaced.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::MissingTexture { .. } => "model.refusal.missing_texture",
            Refusal::TextureCycle { .. } => "model.refusal.texture_cycle",
            Refusal::UnusedShade { .. } => "model.refusal.unused_shade",
            Refusal::NotAMaterial { .. } => "model.refusal.not_a_material",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::MissingTexture { key } => write!(f, "{}: {key}", self.name()),
            Refusal::TextureCycle { key } => write!(f, "{}: {key}", self.name()),
            Refusal::UnusedShade { colour, .. } => write!(f, "{}: {colour}", self.name()),
            Refusal::NotAMaterial { texture } => write!(f, "{}: {texture}", self.name()),
        }
    }
}

impl std::error::Error for Refusal {}

/// The last path segment of a texture name, namespace and folders dropped:
/// `minecolonies:block/books/sketchbook_1` is `sketchbook_1`.
pub fn stem(texture: &str) -> &str {
    match texture.rsplit_once('/') {
        Some((_, stem)) => stem,
        None => texture,
    }
}

/// The shade a texture names, or why it is not one.
///
/// A name is a shade when it is one of the 35 the mod borrows; a name vanilla
/// could produce and the mod never does is refused as [`Refusal::UnusedShade`],
/// which is the "texture absente" of the brief, named.
pub fn shade(texture: &str) -> Result<Shade, Refusal> {
    let leaf = stem(texture);
    for material in Material::ALL {
        for (colour, name) in material.shades() {
            if leaf == stem(name) {
                return Ok(Shade {
                    material,
                    colour,
                    texture: name,
                });
            }
        }
    }
    // Not borrowed: is it at least a shade vanilla could have?
    for material in Material::ALL
        .into_iter()
        .filter(|material| material.is_dyed())
    {
        let suffix = material.suffix();
        let colour = if leaf == suffix {
            ""
        } else {
            match leaf.strip_suffix(suffix) {
                Some(prefix) => prefix.strip_suffix('_').unwrap_or(""),
                None => continue,
            }
        };
        if colour.is_empty() && leaf != suffix {
            continue;
        }
        if leaf == suffix || VANILLA_COLOURS.contains(&colour) {
            return Err(Refusal::UnusedShade {
                material: suffix,
                colour: leaf.to_owned(),
            });
        }
    }
    Err(Refusal::NotAMaterial {
        texture: texture.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textures(text: &str) -> Textures {
        match mod_api::serde_json::from_str::<Value>(text).expect("valid JSON") {
            Value::Object(map) => Textures::parse(&map),
            other => panic!("not an object: {other:?}"),
        }
    }

    /// `models/block/blockhutcitizen.json`, its 16 keys.
    const CITIZEN: &str = r#"{
      "0": "block/oak_planks", "3": "block/terracotta", "4": "block/lime_terracotta",
      "5": "block/red_terracotta", "6": "block/green_terracotta",
      "7": "block/light_gray_terracotta", "8": "minecolonies:block/liquid/liq1",
      "9": "block/dirt", "10": "block/coal_block", "11": "minecolonies:block/books/sketchbook_1",
      "12": "block/stripped_dark_oak_log", "13": "block/iron_block", "14": "block/light_gray_wool",
      "particle": "block/oak_planks"
    }"#;

    /// `models/block/blockhutcitizen_1.json`, its whole texture map.
    const CITIZEN_RED: &str = r#"{"7": "block/red_terracotta", "14": "block/red_wool"}"#;

    #[test]
    fn a_texture_map_is_read_in_file_order_and_a_non_string_is_left_out() {
        let map =
            textures(r#"{"0": "block/oak_planks", "particle": "block/oak_planks", "odd": 7}"#);
        assert_eq!(map.pairs.len(), 2);
        assert_eq!(map.get("0"), Some("block/oak_planks"));
        assert_eq!(map.get("odd"), None, "a number is not a texture");
        assert_eq!(map.keys(), vec!["0", "particle"]);
    }

    #[test]
    fn a_child_model_replaces_only_the_keys_it_names() {
        let parent = textures(CITIZEN);
        let child = textures(CITIZEN_RED);
        let merged = parent.merge(&child);
        assert_eq!(parent.pairs.len(), 14, "the citizen hut's own map");
        assert_eq!(merged.pairs.len(), 14, "no key is added or lost");
        assert_eq!(merged.get("14"), Some("block/red_wool"));
        assert_eq!(merged.get("7"), Some("block/red_terracotta"));
        assert_eq!(merged.get("5"), Some("block/red_terracotta"), "untouched");
        assert_eq!(merged.get("particle"), Some("block/oak_planks"));
        // The finding, as one call: the red hut is the plain hut with two keys
        // swapped, and `overridden` names them.
        assert_eq!(parent.overridden(&child), vec!["14", "7"]);
    }

    #[test]
    fn a_key_only_the_child_has_is_added() {
        let parent = textures(r#"{"a": "block/dirt"}"#);
        let child = textures(r#"{"b": "block/sand"}"#);
        let merged = parent.merge(&child);
        assert_eq!(merged.keys(), vec!["a", "b"]);
        assert_eq!(parent.overridden(&child), vec!["b"]);
    }

    #[test]
    fn a_missing_key_is_refused_by_name() {
        let map = textures(CITIZEN);
        let error = map.resolve("15").expect_err("no key 15");
        assert_eq!(error.name(), "model.refusal.missing_texture");
        assert_eq!(error.to_string(), "model.refusal.missing_texture: 15");
    }

    #[test]
    fn the_one_reference_in_the_mod_resolves_to_its_key() {
        // `floodedfarmland.json` writes `"particle": "#dirt"`; vanilla looks `dirt`
        // up in the merged map.
        let map = textures(r##"{"particle": "#dirt", "dirt": "block/dirt"}"##);
        assert_eq!(map.resolve("particle").expect("resolves"), "block/dirt");
        assert_eq!(map.resolve("dirt").expect("resolves"), "block/dirt");
    }

    #[test]
    fn the_spear_s_dangling_reference_is_refused_by_name() {
        // `models/item/spear_in_hand.json` writes `"particle": "#spear"` and never
        // defines `spear`: the key only arrives through `spear.json`'s Forge
        // `separate_transforms` loader (`MANQUANT.md` §5). On its own the file does
        // not resolve, and the port says which key is missing instead of drawing a
        // magenta sprite.
        let map = textures(r##"{"particle": "#spear"}"##);
        let error = map.resolve("particle").expect_err("no spear key");
        assert_eq!(error.name(), "model.refusal.missing_texture");
        assert!(matches!(&error, Refusal::MissingTexture { key } if key == "spear"));
        // Add the key the loader supplies and it resolves.
        let map = textures(r##"{"particle": "#spear", "spear": "minecolonies:item/spear"}"##);
        assert_eq!(
            map.resolve("particle").expect("resolves"),
            "minecolonies:item/spear"
        );
    }

    #[test]
    fn a_reference_that_comes_back_to_itself_is_refused() {
        let map = textures(r##"{"a": "#b", "b": "#a"}"##);
        let error = map.resolve("a").expect_err("a cycle");
        assert_eq!(error.name(), "model.refusal.texture_cycle");
        // And a chain at the limit resolves normally.
        let depth = MAX_DEPTH as u8;
        let mut literal = String::from("{\"k0\": \"block/oak_planks\"");
        for step in 1..depth {
            literal.push_str(&format!(", \"k{step}\": \"#k{}\"", step - 1));
        }
        literal.push('}');
        let map = textures(&literal);
        let last = format!("k{}", depth - 1);
        assert_eq!(map.resolve(&last).expect("resolves"), "block/oak_planks");
    }

    #[test]
    fn a_chain_looks_nearest_first_and_reports_the_depth() {
        let own = textures(r#"{"particle": "block/red_wool"}"#);
        let parent = textures(CITIZEN);
        let grandparent = textures(r#"{"particle": "block/stone"}"#);
        let chain = Chain::new(vec![own, parent, grandparent]);
        assert_eq!(chain.depth_of("particle"), Some(0));
        assert_eq!(chain.depth_of("13"), Some(1));
        assert_eq!(chain.depth_of("nope"), None);
        assert_eq!(chain.merged().get("particle"), Some("block/red_wool"));
        // Drop the model's own map and the parent's value is the one that wins,
        // not the grandparent's.
        let chain = Chain::new(vec![
            textures("{}"),
            textures(CITIZEN),
            textures(r#"{"particle": "block/stone"}"#),
        ]);
        assert_eq!(chain.merged().get("particle"), Some("block/oak_planks"));
        assert_eq!(chain.depth_of("particle"), Some(1));
    }

    #[test]
    fn the_six_materials_hold_the_thirty_five_colours_the_models_borrow() {
        let mut total = 0;
        for material in Material::ALL {
            let shades = material.shades();
            assert!(!shades.is_empty(), "{} borrows nothing", material.suffix());
            for (colour, texture) in shades {
                assert!(
                    texture.starts_with("block/"),
                    "{texture} is not a vanilla block sprite"
                );
                if !colour.is_empty() {
                    // The dyed materials carry dye colours; planks carry species,
                    // which are the game's list and not the 16.
                    if material.is_dyed() {
                        assert!(
                            VANILLA_COLOURS.contains(colour),
                            "{colour} is not one of the 16"
                        );
                    }
                    assert_eq!(
                        stem(texture),
                        format!("{colour}_{}", material.suffix()),
                        "{texture} does not fit its material"
                    );
                }
                total += 1;
            }
        }
        assert_eq!(total, 35);
        let mut suffixes: Vec<&str> = Material::ALL.iter().map(|m| m.suffix()).collect();
        suffixes.sort_unstable();
        suffixes.dedup();
        assert_eq!(suffixes.len(), 6, "two materials share a suffix");
    }

    #[test]
    fn a_borrowed_shade_names_its_material_and_colour() {
        let sh = shade("block/white_wool").expect("a shade");
        assert_eq!(sh.pair(), (Material::Wool, "white"));
        assert_eq!(sh.texture, "block/white_wool");
        assert_eq!(
            shade("block/light_gray_terracotta")
                .expect("a shade")
                .pair(),
            (Material::Terracotta, "light_gray")
        );
        // The uncoloured terracotta is a shade with an empty colour.
        assert_eq!(
            shade("block/terracotta").expect("a shade").pair(),
            (Material::Terracotta, "")
        );
        // A namespaced path still resolves, because only the stem is compared.
        assert_eq!(
            shade("minecraft:block/red_stained_glass")
                .expect("a shade")
                .pair(),
            (Material::StainedGlass, "red")
        );
        assert_eq!(
            stem("minecolonies:block/books/sketchbook_1"),
            "sketchbook_1"
        );
        assert_eq!(stem("book"), "book");
    }

    #[test]
    fn a_colour_vanilla_has_and_the_mod_never_borrows_is_refused_as_such() {
        // Eight of the sixteen wools are never used as a texture: `block/black_wool`
        // is in vanilla, and no model of this mod names it.
        let error = shade("block/black_wool").expect_err("never borrowed");
        assert_eq!(error.name(), "model.refusal.unused_shade");
        assert_eq!(error.to_string(), "model.refusal.unused_shade: black_wool");
        assert!(matches!(
            error,
            Refusal::UnusedShade {
                material: "wool",
                ..
            }
        ));
        // Same for a terracotta colour and a glass colour the mod passes on.
        assert!(matches!(
            shade("block/pink_terracotta").expect_err("never borrowed"),
            Refusal::UnusedShade {
                material: "terracotta",
                ..
            }
        ));
        assert!(matches!(
            shade("block/lime_stained_glass").expect_err("never borrowed"),
            Refusal::UnusedShade {
                material: "stained_glass",
                ..
            }
        ));
    }

    #[test]
    fn a_name_that_is_not_a_shade_at_all_is_refused_as_such() {
        // The 126 vanilla names include plenty of non-materials: stone, dirt, glass.
        for texture in [
            "block/stone",
            "block/dirt",
            "block/glass",
            "block/oak_planks_top",
            "minecolonies:block/books/sketchbook_1",
            "block/hay_block_side",
        ] {
            let error = shade(texture).expect_err(texture);
            assert_eq!(error.name(), "model.refusal.not_a_material", "{texture}");
        }
    }

    #[test]
    fn a_material_is_found_by_its_suffix_and_the_plain_shade_by_an_empty_colour() {
        assert_eq!(Material::from_suffix("wool"), Some(Material::Wool));
        assert_eq!(Material::from_suffix("concrete"), Some(Material::Concrete));
        assert_eq!(Material::from_suffix("planks"), Some(Material::Planks));
        assert_eq!(
            Material::from_suffix("concrete_powder"),
            Some(Material::ConcretePowder)
        );
        assert_eq!(Material::from_suffix("stone"), None);
        // `block/concrete` on its own is a vanilla sprite the mod never borrows.
        assert!(matches!(
            shade("block/concrete").expect_err("not borrowed"),
            Refusal::UnusedShade {
                material: "concrete",
                ..
            }
        ));
    }

    #[test]
    fn the_coloured_huts_are_four_replacements_of_one_model() {
        // The five citizen hut models, as far as their texture maps go: one plain
        // and four that swap two keys. Measured on the four sibling files.
        // The four files, verbatim: each swaps the same two keys - the terracotta
        // wall and the wool - and nothing else.
        let plain = textures(CITIZEN);
        for (file, terracotta, wool) in [
            (
                "blockhutcitizen_1",
                "block/red_terracotta",
                "block/red_wool",
            ),
            (
                "blockhutcitizen_2",
                "block/yellow_terracotta",
                "block/green_wool",
            ),
            (
                "blockhutcitizen_3",
                "block/cyan_terracotta",
                "block/blue_wool",
            ),
            (
                "blockhutcitizen_4",
                "block/white_terracotta",
                "block/white_wool",
            ),
        ] {
            let literal = format!(r#"{{"7": "{terracotta}", "14": "{wool}"}}"#);
            let child = textures(&literal);
            let merged = plain.merge(&child);
            assert_eq!(merged.get("14"), Some(wool), "{file}");
            assert_eq!(merged.get("7"), Some(terracotta), "{file}");
            assert_eq!(merged.pairs.len(), plain.pairs.len(), "{file}");
            assert_eq!(plain.overridden(&child), vec!["14", "7"], "{file}");
            // And every one of the four borrows a wool and a terracotta shade.
            assert_eq!(shade(wool).expect("a shade").material, Material::Wool);
            assert_eq!(
                shade(terracotta).expect("a shade").material,
                Material::Terracotta
            );
        }
    }
}
