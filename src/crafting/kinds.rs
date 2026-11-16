//! The table of recipe types: which ones exist, which fields each one reads, and
//! which loader reads it.
//!
//! Java reference: `api/crafting/ModRecipeTypes.java` (the two storage types),
//! `api/crafting/ModCraftingTypes.java` (the five teaching types),
//! `api/crafting/registry/ModRecipeSerializer.java` (the two serializers),
//! `apiimp/initializer/ModRecipeSerializerInitializer.java` (their registration),
//! `core/colony/crafting/CustomRecipe.java` (the five crafter kinds) and
//! `core/datalistener/CrafterRecipeListener.java` (the loader that dispatches on
//! `type`).
//!
//! **Three registries, three different words for "recipe type", and only one of
//! them is a `type` field.**
//!
//! | Registry | Java entry | Count | Named by a data file? |
//! | --- | --- | ---: | --- |
//! | `RecipeSerializer` + `RecipeType` | [`Kind`] | 2 + 5 kinds | **yes**, the `type` field |
//! | `recipetypeentries` (`RecipeTypeEntry`) | [`StorageType`] | 2 | no — derived from the recipe's own outputs |
//! | `craftingtypes` (`CraftingType`) | [`Teaching`] | 5 | no — a teaching source, not a recipe |
//!
//! Confusing the three is the mistake this file exists to prevent: only the
//! first column is a `type` value, and only the first column can make the whole
//! recipe table fail to load.
//!
//! **The counts, measured.**
//!
//! | What | How many |
//! | --- | ---: |
//! | `minecolonies:composting` files in the source | 3 (`compostables`, `_poor`, `_rich`) |
//! | `minecolonies:zero_waste` files in the source | 1 (`soy_milk`) |
//! | `crafterrecipes/` files in the datagen | 161 |
//! | `crafterrecipes/` files hand-written in `src/main/resources` | 2 (`lumberjack/strip_logs`, `strip_stems`) |
//! | crafter ids those 161 files name | 16 |
//! | kinds the listener dispatches on | 5 declared, **4 handled** |
//!
//! The two hand-written files are `recipe-template`, a kind the datagen never
//! emits: it is only ever written by hand.

/// Which loader reads a file, which is the first thing [`Kind`] has to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `data/<ns>/recipe/`, the game's `RecipeManager`, a `RecipeSerializer`.
    Recipes,
    /// `data/<ns>/crafterrecipes/`, `CrafterRecipeListener`
    /// (`CrafterRecipeListener.java:31`), a crafter-specific kind.
    CrafterRecipes,
}

impl Origin {
    /// The folder a data pack puts a file of this origin in.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Recipes => "recipe",
            Self::CrafterRecipes => "crafterrecipes",
        }
    }
}

/// One of the seven values a recipe file's `type` field can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java registers this closed set, and a new kind is a change here
pub enum Kind {
    /// `minecolonies:composting` — a composting *scale*, not a crafting recipe.
    Composting,
    /// `minecolonies:zero_waste` — shapeless, and discards the leftovers.
    ZeroWaste,
    /// `recipe` — a crafter's own recipe (`CustomRecipe.java:52`).
    CrafterRecipe,
    /// `recipe-multi-out` — the same, with alternate outputs
    /// (`CustomRecipe.java:57`); the parse is one code path for both
    /// (`CrafterRecipeListener.java:52-55`).
    CrafterMultiOut,
    /// `recipe-multi-in` — **declared and never handled**. See
    /// [`Kind::handled`].
    CrafterMultiIn,
    /// `recipe-template` — one file, expanded into a recipe per tag member
    /// (`CustomRecipe.java:402-439`).
    Template,
    /// `remove` — names a recipe id to delete, and holds no recipe at all
    /// (`CrafterRecipeListener.java:59-62`).
    Remove,
}

impl Kind {
    /// Every kind, in the order the `type` constants are declared: the two
    /// serializers first, then the five crafter kinds.
    pub const ALL: [Kind; 7] = [
        Kind::Composting,
        Kind::ZeroWaste,
        Kind::CrafterRecipe,
        Kind::CrafterMultiOut,
        Kind::CrafterMultiIn,
        Kind::Template,
        Kind::Remove,
    ];

    /// The value of the `type` field, exactly as a file writes it.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Composting => "minecolonies:composting",
            Self::ZeroWaste => "minecolonies:zero_waste",
            Self::CrafterRecipe => "recipe",
            Self::CrafterMultiOut => "recipe-multi-out",
            Self::CrafterMultiIn => "recipe-multi-in",
            Self::Template => "recipe-template",
            Self::Remove => "remove",
        }
    }

    /// The kind a `type` value names, or `None`.
    ///
    /// The two serializer names carry their namespace and the five crafter kinds
    /// do not, which is not an inconsistency of this port: it is what the files
    /// say (`ModRecipeSerializerInitializer.java:21`, `:24` against
    /// `CustomRecipe.java:52-72`).
    pub fn from_type(found: &str) -> Option<Kind> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.type_name() == found)
    }

    /// Which loader reads a file of this kind.
    pub fn origin(self) -> Origin {
        match self {
            Self::Composting | Self::ZeroWaste => Origin::Recipes,
            Self::CrafterRecipe
            | Self::CrafterMultiOut
            | Self::CrafterMultiIn
            | Self::Template
            | Self::Remove => Origin::CrafterRecipes,
        }
    }

    /// Whether the game matches this kind against a crafting grid.
    ///
    /// `composting` is `false` and that is its point: it has no `result` field at
    /// all (`CompostRecipe.java:28-45` builds a fixed `compost` × 6), because a
    /// barrel ferments over time rather than matching a grid.
    /// `zero_waste` is `true` — it extends `ShapelessRecipe`
    /// (`ZeroWasteRecipe.java:38`).
    pub fn craftable(self) -> bool {
        matches!(
            self,
            Self::ZeroWaste | Self::CrafterRecipe | Self::CrafterMultiOut | Self::CrafterMultiIn
        )
    }

    /// Whether `CrafterRecipeListener`'s switch has a case for this kind.
    ///
    /// **`recipe-multi-in` is declared and never handled.** The constant exists
    /// (`CustomRecipe.java:62`) and the switch at
    /// `CrafterRecipeListener.java:50-63` has cases for `recipe`,
    /// `recipe-multi-out`, `recipe-template` and `remove` — and no `default`, so
    /// a file of this kind is dropped without a line in the log. No shipped file
    /// uses it (the datagen writes only `recipe`), which is why nobody noticed.
    /// The port reads it as a [`Kind`] so that a pack using it gets
    /// [`crate::Refusal`]-free silence no longer: `resolve` accepts it and the
    /// caller can act on [`Kind::handled`].
    pub fn handled(self) -> bool {
        !matches!(self, Self::CrafterMultiIn)
    }

    /// Where Java declares this kind, for a log line or a review.
    pub fn source(self) -> &'static str {
        match self {
            Self::Composting => "ModRecipeSerializerInitializer.java:21",
            Self::ZeroWaste => "ModRecipeSerializerInitializer.java:24",
            Self::CrafterRecipe => "CustomRecipe.java:52",
            Self::CrafterMultiOut => "CustomRecipe.java:57",
            Self::CrafterMultiIn => "CustomRecipe.java:62",
            Self::Template => "CustomRecipe.java:67",
            Self::Remove => "CustomRecipe.java:72",
        }
    }

    /// The fields this kind reads, with the line that reads each one.
    pub fn fields(self) -> &'static [Field] {
        match self {
            Self::Composting => COMPOSTING_FIELDS,
            Self::ZeroWaste => ZERO_WASTE_FIELDS,
            Self::CrafterRecipe | Self::CrafterMultiOut | Self::CrafterMultiIn => {
                CRAFTER_FIELDS
            }
            Self::Template => TEMPLATE_FIELDS,
            Self::Remove => REMOVE_FIELDS,
        }
    }
}

/// What a field holds, named after the reader that reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// The kind itself (`type`), read before dispatch.
    KindName,
    /// `GsonHelper.getAsString`.
    Text,
    /// A `ResourceLocation`, parsed rather than trusted.
    Id,
    /// An item id, which [`crate::ItemId`] splits from its NBT suffix.
    Item,
    /// `{"item": …, "count": …}`, read by `ItemStorage`'s JSON constructor
    /// (`ItemStorage.java:152-168`).
    ItemStack,
    /// An array of the row above (`CustomRecipeProvider.java:260`).
    StackList,
    /// A vanilla `Ingredient` value, or one of the two this mod adds
    /// (`FoodIngredient`, `PlantIngredient`).
    Ingredient,
    /// An array of the row above, which is how `composting` names alternatives
    /// (`CompostRecipe.java:146`, one array serialised by
    /// `CompostRecipeBuilder.java:47-53`).
    IngredientList,
    /// `GsonHelper.getAsInt`.
    Integer,
    /// A string or an array of them (`CustomRecipe.java:353-372`).
    Texts,
    /// `GsonHelper.getAsBoolean`, absent meaning `false`.
    Flag,
    /// A nested `Recipe` object (`recipe-template`'s base recipe).
    Object,
    /// `{"include": …, "exclude": …}`, or a bare string or array
    /// (`CustomRecipe.java:410-421`).
    Filter,
}

/// One field of one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// The key, exactly as the JSON writes it — dashes included
    /// (`min-building-level`, `not-research-id`).
    pub name: &'static str,
    pub kind: FieldKind,
    /// The value Java gives when the field is absent; `None` when Java's reader
    /// would throw, which makes the file invalid.
    pub default: Option<&'static str>,
    /// Where Java reads it.
    pub source: &'static str,
}

impl Field {
    /// Whether a file without this field is invalid.
    pub fn required(&self) -> bool {
        self.default.is_none()
    }
}

// `input` is one ingredient value in the file and one array after the datagen
// merges the builder's inputs (CompostRecipeBuilder.java:47-53), so the reader
// accepts both: `Ingredient.fromJson` on an array reads alternatives
// (CompostRecipe.java:146).
const COMPOSTING_FIELDS: &[Field] = &[
    Field {
        name: "type",
        kind: FieldKind::KindName,
        default: None,
        source: "ModRecipeSerializerInitializer.java:21",
    },
    Field {
        name: "input",
        kind: FieldKind::Ingredient,
        default: None,
        source: "CompostRecipe.java:146",
    },
    Field {
        name: "strength",
        kind: FieldKind::Integer,
        default: Some("1"),
        source: "CompostRecipe.java:147",
    },
];

const ZERO_WASTE_FIELDS: &[Field] = &[
    Field {
        name: "type",
        kind: FieldKind::KindName,
        default: None,
        source: "ModRecipeSerializerInitializer.java:24",
    },
    Field {
        name: "result",
        kind: FieldKind::ItemStack,
        default: None,
        source: "ZeroWasteRecipe.java:70",
    },
    Field {
        name: "ingredients",
        kind: FieldKind::IngredientList,
        default: None,
        source: "ZeroWasteRecipe.java:71",
    },
];

// One table for the three crafter kinds: `parse` is one method
// (CustomRecipe.java:253-390) and the listener sends `recipe` and
// `recipe-multi-out` to it alike (CrafterRecipeListener.java:52-55).
const CRAFTER_FIELDS: &[Field] = &[
    Field {
        name: "type",
        kind: FieldKind::KindName,
        default: None,
        source: "CustomRecipe.java:47",
    },
    Field {
        name: "crafter",
        kind: FieldKind::Text,
        default: Some("unknown"),
        source: "CustomRecipe.java:258-261",
    },
    Field {
        name: "inputs",
        kind: FieldKind::StackList,
        default: Some("[]"),
        source: "CustomRecipe.java:263-276",
    },
    Field {
        name: "result",
        kind: FieldKind::Item,
        default: Some("minecraft:air"),
        source: "CustomRecipe.java:278-285",
    },
    Field {
        name: "count",
        kind: FieldKind::Integer,
        default: Some("1"),
        source: "CustomRecipe.java:341-344",
    },
    Field {
        name: "loot-table",
        kind: FieldKind::Id,
        default: None,
        source: "CustomRecipe.java:287-290",
    },
    Field {
        name: "tool",
        kind: FieldKind::Id,
        default: Some("minecolonies:none"),
        source: "CustomRecipe.java:292-296",
    },
    Field {
        name: "additional-output",
        kind: FieldKind::StackList,
        default: Some("[]"),
        source: "CustomRecipe.java:298-317",
    },
    Field {
        name: "alternate-output",
        kind: FieldKind::StackList,
        default: Some("[]"),
        source: "CustomRecipe.java:320-339",
    },
    Field {
        name: "intermediate",
        kind: FieldKind::Id,
        default: Some("minecraft:air"),
        source: "CustomRecipe.java:345-352",
    },
    Field {
        name: "research-id",
        kind: FieldKind::Texts,
        default: Some("[]"),
        source: "CustomRecipe.java:353-362",
    },
    Field {
        name: "not-research-id",
        kind: FieldKind::Texts,
        default: Some("[]"),
        source: "CustomRecipe.java:363-372",
    },
    Field {
        name: "min-building-level",
        kind: FieldKind::Integer,
        default: Some("0"),
        source: "CustomRecipe.java:373-376",
    },
    Field {
        name: "max-building-level",
        kind: FieldKind::Integer,
        default: Some("5"),
        source: "CustomRecipe.java:377-380",
    },
    Field {
        name: "must-exist",
        kind: FieldKind::Flag,
        default: Some("false"),
        source: "CustomRecipe.java:381-384",
    },
    Field {
        name: "show-tooltip",
        kind: FieldKind::Flag,
        default: Some("false"),
        source: "CustomRecipe.java:385-388",
    },
];

// `recipe` is the base recipe and `tag` the source of the item ids. Note that
// `RECIPE_TYPE_RECIPE` doubles as this property's name — "recipe" — which is why
// the same constant appears at CustomRecipe.java:408 as a key.
const TEMPLATE_FIELDS: &[Field] = &[
    Field {
        name: "type",
        kind: FieldKind::KindName,
        default: None,
        source: "CustomRecipe.java:67",
    },
    Field {
        name: "tag",
        kind: FieldKind::Id,
        default: None,
        source: "CustomRecipe.java:407",
    },
    Field {
        name: "recipe",
        kind: FieldKind::Object,
        default: None,
        source: "CustomRecipe.java:408",
    },
    Field {
        name: "filter",
        kind: FieldKind::Filter,
        default: Some("every id"),
        source: "CustomRecipe.java:410-421",
    },
];

const REMOVE_FIELDS: &[Field] = &[
    Field {
        name: "type",
        kind: FieldKind::KindName,
        default: None,
        source: "CustomRecipe.java:72",
    },
    Field {
        name: "recipe-id-to-remove",
        kind: FieldKind::Id,
        // Java reads it with a default of "" and then builds a
        // `ResourceLocation` from it, which throws
        // (`CrafterRecipeListener.java:60-61`); the catch there is for
        // `JsonParseException` only, so a `remove` file without this field aborts
        // the whole load. Effectively required, and the port says so.
        default: None,
        source: "CustomRecipe.java:59-61",
    },
];

/// The two entries of Java's `minecolonies:recipetypeentries` registry.
///
/// These are **not** `type` values: no data file names them. A `RecipeStorage`
/// picks one from its own outputs
/// (`RecipeStorage.java:336-338`) and the entry's producer builds the object the
/// crafting GUI displays ([`StorageType::source`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageType {
    /// `minecolonies:classic` — `ClassicRecipe`, chosen when there are no
    /// alternate outputs.
    Classic,
    /// `minecolonies:multi_output` — `MultiOutputRecipe`, chosen when there is
    /// at least one.
    MultiOutput,
}

impl StorageType {
    pub const ALL: [StorageType; 2] = [StorageType::Classic, StorageType::MultiOutput];

    /// The registry name Java gives it (`ModRecipeTypes.java:11-12`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Classic => "minecolonies:classic",
            Self::MultiOutput => "minecolonies:multi_output",
        }
    }

    /// The storage type a recipe with these outputs takes: alternate outputs
    /// present means multi-output, and nothing else does
    /// (`RecipeStorage.java:336-338`).
    pub fn for_outputs(has_alternate_outputs: bool) -> StorageType {
        if has_alternate_outputs {
            StorageType::MultiOutput
        } else {
            StorageType::Classic
        }
    }

    /// Whether the output display shows the secondary outputs too.
    ///
    /// `ClassicRecipe` shows the primary output *and* every secondary output
    /// (`ClassicRecipe.java:30-41`); `MultiOutputRecipe` shows the primary and
    /// the alternates, and never the secondary ones
    /// (`MultiOutputRecipe.java:78-89`).
    pub fn shows_secondary(self) -> bool {
        matches!(self, Self::Classic)
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Classic => "ClassicRecipe.java:14-48",
            Self::MultiOutput => "MultiOutputRecipe.java:11-46",
        }
    }
}

/// The five entries of Java's `minecolonies:craftingtypes` registry — **teaching
/// sources, not recipe types**.
///
/// A `CraftingType` answers the question "which recipes can this hut teach its
/// worker?" (`CraftingType.findRecipes`, `registry/CraftingType.java:31-33`). It
/// reads vanilla's registry rather than a data file of its own:
/// [`Teaching::source`] names where. No file has a `type` equal to one of these
/// ids, and this enum is here only so the next reader does not mistake the five
/// ids for recipe types — the mistake `ModCraftingTypes` invites by living in the
/// same package as `ModRecipeTypes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Teaching {
    SmallCrafting,
    LargeCrafting,
    Smelting,
    Brewing,
    /// `domum_ornamentum:architects_cutter` — the one id outside this mod's
    /// namespace (`ModCraftingTypes.java:15`), because the cutter is Domum's.
    ArchitectsCutter,
}

impl Teaching {
    pub const ALL: [Teaching; 5] = [
        Teaching::SmallCrafting,
        Teaching::LargeCrafting,
        Teaching::Smelting,
        Teaching::Brewing,
        Teaching::ArchitectsCutter,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::SmallCrafting => "minecolonies:smallcrafting",
            Self::LargeCrafting => "minecolonies:largecrafting",
            Self::Smelting => "minecolonies:smelting",
            Self::Brewing => "minecolonies:brewing",
            Self::ArchitectsCutter => "domum_ornamentum:architects_cutter",
        }
    }

    /// Where the recipes it teaches come from.
    pub fn source(self) -> &'static str {
        match self {
            Self::SmallCrafting => {
                "RecipeCraftingType over minecraft:crafting, grid 2 (ModCraftingTypesInitializer)"
            }
            Self::LargeCrafting => {
                "RecipeCraftingType over minecraft:crafting, grid 3 (ModCraftingTypesInitializer)"
            }
            Self::Smelting => "RecipeCraftingType over minecraft:smelting (:4-6)",
            Self::Brewing => "BrewingCraftingType.java:34-60 over Forge's brewing registry",
            Self::ArchitectsCutter => {
                "ArchitectsCutterCraftingType.java:28-31, domum's architects_cutter recipes"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reads_the_type_field_first() {
        for kind in Kind::ALL {
            let fields = kind.fields();
            assert_eq!(
                fields.first().map(|field| field.name),
                Some("type"),
                "{} does not start with the field that selects it",
                kind.type_name()
            );
            assert_eq!(fields[0].kind, FieldKind::KindName);
            assert!(fields[0].required());
        }
    }

    #[test]
    fn the_type_names_are_the_ones_the_files_write() {
        assert_eq!(Kind::Composting.type_name(), "minecolonies:composting");
        assert_eq!(Kind::ZeroWaste.type_name(), "minecolonies:zero_waste");
        assert_eq!(Kind::CrafterRecipe.type_name(), "recipe");
        assert_eq!(Kind::CrafterMultiOut.type_name(), "recipe-multi-out");
        assert_eq!(Kind::CrafterMultiIn.type_name(), "recipe-multi-in");
        assert_eq!(Kind::Template.type_name(), "recipe-template");
        assert_eq!(Kind::Remove.type_name(), "remove");
        for kind in Kind::ALL {
            assert_eq!(Kind::from_type(kind.type_name()), Some(kind));
        }
    }

    #[test]
    fn a_type_nobody_declares_names_no_kind() {
        assert_eq!(Kind::from_type("minecolonies:smelting"), None);
        assert_eq!(Kind::from_type("minecraft:crafting_shaped"), None);
        assert_eq!(Kind::from_type(""), None);
    }

    #[test]
    fn the_two_serializers_live_in_the_recipe_folder_and_the_five_kinds_do_not() {
        assert_eq!(Kind::Composting.origin(), Origin::Recipes);
        assert_eq!(Kind::ZeroWaste.origin(), Origin::Recipes);
        for kind in [
            Kind::CrafterRecipe,
            Kind::CrafterMultiOut,
            Kind::CrafterMultiIn,
            Kind::Template,
            Kind::Remove,
        ] {
            assert_eq!(kind.origin(), Origin::CrafterRecipes);
        }
        assert_eq!(Origin::Recipes.folder(), "recipe");
        assert_eq!(Origin::CrafterRecipes.folder(), "crafterrecipes");
    }

    #[test]
    fn composting_is_not_craftable_and_zero_waste_is() {
        assert!(!Kind::Composting.craftable(), "it has no result field");
        assert!(Kind::ZeroWaste.craftable(), "it extends ShapelessRecipe");
        assert!(!Kind::Remove.craftable(), "it deletes, it does not make");
        assert!(!Kind::Template.craftable(), "it expands into recipes that are");
    }

    #[test]
    fn recipe_multi_in_is_declared_and_never_handled() {
        assert!(!Kind::CrafterMultiIn.handled());
        for kind in Kind::ALL {
            if kind != Kind::CrafterMultiIn {
                assert!(kind.handled(), "{} is a case of the switch", kind.type_name());
            }
        }
        // The other two crafter kinds are handled by the same case.
        assert!(Kind::CrafterRecipe.handled() && Kind::CrafterMultiOut.handled());
    }

    #[test]
    fn the_three_crafter_kinds_share_one_field_table() {
        assert_eq!(
            Kind::CrafterRecipe.fields(),
            Kind::CrafterMultiOut.fields()
        );
        assert_eq!(
            Kind::CrafterRecipe.fields(),
            Kind::CrafterMultiIn.fields()
        );
        assert_eq!(Kind::CrafterRecipe.fields().len(), 16);
    }

    #[test]
    fn the_defaults_are_the_ones_custom_recipe_uses() {
        let fields = Kind::CrafterRecipe.fields();
        assert_eq!(find(fields, "crafter").default, Some("unknown"));
        assert_eq!(find(fields, "min-building-level").default, Some("0"));
        assert_eq!(find(fields, "max-building-level").default, Some("5"));
        assert_eq!(find(fields, "tool").default, Some("minecolonies:none"));
        assert_eq!(find(fields, "intermediate").default, Some("minecraft:air"));
        assert_eq!(find(fields, "must-exist").default, Some("false"));
        assert_eq!(find(fields, "show-tooltip").default, Some("false"));
        // The one field a recipe can do without is `result`: a loot-table only
        // recipe has none (CustomRecipe.java:278-285 makes it empty).
        assert!(!find(fields, "result").required());
        assert!(!find(fields, "loot-table").required());
    }

    /// One field of a table by name, for the assertions above.
    fn find<'a>(fields: &'a [Field], name: &str) -> &'a Field {
        fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("{name} is not a field of this table"))
    }

    #[test]
    fn a_composting_file_must_name_its_input_and_may_omit_its_strength() {
        let fields = Kind::Composting.fields();
        let input = fields
            .iter()
            .find(|field| field.name == "input")
            .expect("input is a field");
        assert!(input.required(), "Ingredient.fromJson throws without it");
        assert_eq!(input.kind, FieldKind::Ingredient);
        let strength = fields
            .iter()
            .find(|field| field.name == "strength")
            .expect("strength is a field");
        assert_eq!(strength.default, Some("1"));
        assert_eq!(Kind::Composting.fields().len(), 3);
    }

    #[test]
    fn a_zero_waste_file_must_name_its_result_and_its_ingredients() {
        let fields = Kind::ZeroWaste.fields();
        assert_eq!(fields.len(), 3);
        for name in ["result", "ingredients"] {
            let field = fields
                .iter()
                .find(|field| field.name == name)
                .unwrap_or_else(|| panic!("{name} is a field"));
            assert!(field.required(), "{name} has no default in Java either");
        }
        let result = fields
            .iter()
            .find(|field| field.name == "result")
            .expect("result is a field");
        assert_eq!(result.kind, FieldKind::ItemStack, "item plus count");
    }

    #[test]
    fn a_template_names_a_tag_and_a_base_recipe() {
        let fields = Kind::Template.fields();
        for name in ["tag", "recipe"] {
            let field = fields
                .iter()
                .find(|field| field.name == name)
                .unwrap_or_else(|| panic!("{name} is a field"));
            assert!(field.required(), "GsonHelper.getAsString throws without it");
        }
        let base = fields
            .iter()
            .find(|field| field.name == "recipe")
            .expect("recipe is a field");
        assert_eq!(base.kind, FieldKind::Object);
    }

    #[test]
    fn a_storage_type_follows_the_outputs_and_not_the_file() {
        assert_eq!(StorageType::for_outputs(false), StorageType::Classic);
        assert_eq!(StorageType::for_outputs(true), StorageType::MultiOutput);
        assert_eq!(StorageType::Classic.id(), "minecolonies:classic");
        assert_eq!(StorageType::MultiOutput.id(), "minecolonies:multi_output");
        // Two display rules that differ, which is why the two types exist at all.
        assert!(StorageType::Classic.shows_secondary());
        assert!(!StorageType::MultiOutput.shows_secondary());
    }

    #[test]
    fn a_storage_type_is_never_a_type_field_value() {
        for storage in StorageType::ALL {
            assert_eq!(
                Kind::from_type(storage.id()),
                None,
                "{} is a registry entry, not a value a file writes",
                storage.id()
            );
        }
    }

    #[test]
    fn the_five_teaching_types_are_not_recipe_types_either() {
        assert_eq!(Teaching::ALL.len(), 5);
        for teaching in Teaching::ALL {
            assert_eq!(
                Kind::from_type(teaching.id()),
                None,
                "{} teaches recipes, it is not one",
                teaching.id()
            );
        }
        // The cutter is the one id outside this mod's namespace.
        assert!(Teaching::ArchitectsCutter.id().starts_with("domum_ornamentum:"));
    }
}
