//! The **163 items** of MineColonies: what each one is, how many fit in a slot,
//! which vanilla item its content declaration copies, and where Java says so.
//!
//! Java reference: `api/items/ModItems.java` (130 static `Item` fields),
//! `apiimp/initializer/ModItemsInitializer.java` (497 lines: every
//! `registry.register` call, 148 of them), and the 46 classes of `core/items/`
//! plus the two interfaces of `api/items/` (`IMinecoloniesFoodItem`,
//! `ISupplyItem`).
//!
//! | File | Question |
//! | --- | --- |
//! | `mod` (this one) | which items exist, of what kind, how many per slot, and the declaration each one needs |
//! | `tools` | durability, reach, damage, armour points, and what one use is worth |
//! | `food` | hunger points, saturation, tier, and what eating it does besides feeding you |
//! | `materials` | what the ingredients, crops and bottles are for — read out of `crate::jobs::kinds`, not recopied |
//! | `RAPPORT.md` | the counts, and why an item has no row where it has none |
//! | `MANQUANT.md` | what the port would need and this SDK does not give |
//!
//! **Four counts, and the arithmetic between them.** `ModItems` declares **130**
//! static `Item` fields (`api/items/ModItems.java`), the items initialiser makes
//! **148** `registry.register` calls for this namespace
//! (`ModItemsInitializer.java:246-467`), and the two numbers differ by exactly the
//! **18 spawn eggs**, which are built inline with no field of their own
//! (`:386-463`). 130 + 18 = 148.
//!
//! **Fifteen more items exist, and they are food**: the crops are `ItemCrop`s
//! created by their block (`MinecoloniesCropBlock.java:229`), so no call in the
//! items initialiser registers them. Their item half is a row here — category
//! `Food` — and in [`food`] for its numbers; their block half is
//! `crate::blocks::ressource`. **148 + 15 = 163 rows**, and
//! [`tests::the_table_holds_the_one_hundred_and_sixty_three_items`] holds both
//! halves.
//!
//! **Nothing here is a registration.** The SDK has no `Registrar::item`: a block
//! is declared with `Registrar::block_key` (`crates/horizon-mod-vocab/src/parts/blocks.rs:133`),
//! an item is **content** — a `data/<namespace>/horizon/item/<name>.json` file
//! read into `content::Item` (`content/content_declarations.rs:244-266`) — and it
//! is declared as data, not from code. So [`ItemEntry::declaration`] builds the
//! `mod_api::Item` each row needs, and writing the 163 files is the content
//! slice's job, not this module's.
//!
//! **What is *not* here.**
//!
//! - **the models and textures**: `assets/` is another slice's, and a declaration
//!   names no model.
//! - **the 163 JSON files**: [`ItemEntry::declaration`] returns the value; the
//!   file is `data/`, which this module does not own (rule 1).
//! - **what a right click on an item does**: [`tools`] holds the numbers, and the
//!   hook that uses them (`item_use`, `item_use_on`) is not implemented by
//!   `src/lib.rs` yet — see `MANQUANT.md` §2.

pub mod food;
pub mod materials;
pub mod tools;

pub use food::{Food, FoodForm, AfterEating};
pub use materials::{Material, MaterialKind};
pub use tools::{ArmourMaterial, ArmourSlot, Hammer, ToolStats};

/// The namespace every item of this table is registered under.
///
/// Re-exported from `crate::blocks::NAMESPACE` rather than written again: one
/// spelling of `minecolonies` for blocks and items, so the two cannot drift.
pub const NAMESPACE: &str = crate::blocks::NAMESPACE;

/// What an item *is*, for a caller that has to answer "may I put this in a tool
/// slot / a chest / a food trough" without a stack.
///
/// Java has no such enum: it is this port's reading of the 46 classes of
/// `core/items/`, grouped by the superclass each one builds on. Each variant
/// names the superclass that decides it, because that class — not the item's name
/// — is what an engine query (`isEdible`, `canBeDepleted`, `instanceof SwordItem`)
/// will read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: twelve superclasses cover the 163 items, and a thirteenth item
// family would be a thirteenth variant
pub enum Category {
    /// A stack the player swings or aims: `AbstractItemMinecolonies`,
    /// `SwordItem`, `TridentItem`, `BowItem`. **17 items** ([`tools`]).
    Tool,
    /// `ItemFood`, `ItemBowlFood` and the fifteen `ItemCrop`s, i.e. `Item` with
    /// `food(...)`. **79 items** ([`food`]).
    Food,
    /// A plain `Item` spent in a recipe or on a block: the seven ingredients of
    /// `getAllIngredients`, the four batters, `compost`, `mistletoe` and the
    /// adventure token. **14 items** ([`materials`]).
    Material,
    /// An `ArmorItem`: the four set pieces of three materials, the two hats and
    /// the goggles. **14 items**.
    Armour,
    /// A `BlockItem`: the two gates and the colony banner. **3 items** — a block
    /// item whose block is *not* in `crate::blocks` would be a fourth.
    Block,
    /// An `ArrowItem`. **1 item**.
    Ammo,
    /// An `AbstractItemScroll`, which is a use-and-shrink document. **5 items**.
    Scroll,
    /// A `menu` opener: `ItemClipboard`, `ItemResourceScroll`, `ItemQuestLog`,
    /// `ItemColonyMap`, `ItemAncientTome`. **5 items**.
    Document,
    /// An `ISupplyItem`: the two structures a player places. **2 items**.
    Deployer,
    /// A `ForgeSpawnEggItem`. **18 items.**
    SpawnEgg,
    /// An `ItemLargeBottle`. **4 items.**
    Container,
    /// The one `ItemMagicPotion`, which is an ingredient for a brewing stand and
    /// nothing else. **1 item.**
    Potion,
}

impl Category {
    /// Every category, in the order [`ITEMS`] is grouped by.
    pub const ALL: [Category; 12] = [
        Category::Tool,
        Category::Food,
        Category::Material,
        Category::Armour,
        Category::Block,
        Category::Ammo,
        Category::Scroll,
        Category::Document,
        Category::Deployer,
        Category::SpawnEgg,
        Category::Container,
        Category::Potion,
    ];

    /// A stable name, for a refusal or a test failure.
    pub fn name(self) -> &'static str {
        match self {
            Category::Tool => "tool",
            Category::Food => "food",
            Category::Material => "material",
            Category::Armour => "armour",
            Category::Block => "block",
            Category::Ammo => "ammo",
            Category::Scroll => "scroll",
            Category::Document => "document",
            Category::Deployer => "deployer",
            Category::SpawnEgg => "spawn_egg",
            Category::Container => "container",
            Category::Potion => "potion",
        }
    }
}

/// How many of an item one slot takes, **as Java states it**.
///
/// Three cases, and the third is the one that matters: Java is silent for most
/// items, and the number a silent item gets is the engine's answer, not the
/// port's. Writing `64` for those would be a guess that looks like a fact — and a
/// damageable item's `1` is a *different* rule from a plain item's `64`, which is
/// why they are two variants and not one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: Java states a limit, states damage instead, or states nothing
pub enum StackLimit {
    /// `Item.Properties.stacksTo(n)` in Java: `1` for the five tools that say so
    /// (`ItemScepterGuard.java:43`), `16` for the scrolls, the banner and the
    /// potion, `64` for `Constants.STACKSIZE`
    /// (`api/util/constant/Constants.java:23`).
    Fixed(u8),
    /// Java states `durability(n)` and no stack size: the engine stacks a
    /// damageable item to one. The count is therefore *derived*, not declared —
    /// `Host::max_stack_size` answers it
    /// (`crates/horizon-mod-vocab/src/host/items.rs:8-12`).
    Damageable,
    /// Java states neither: the item takes whatever its template takes, i.e.
    /// usually 64. Asked of the host, never assumed here.
    EngineDefault,
}

impl StackLimit {
    /// The number Java wrote, when it wrote one. `None` is "ask the engine",
    /// never "64".
    pub fn stated(self) -> Option<u8> {
        match self {
            StackLimit::Fixed(max) => Some(max),
            StackLimit::Damageable | StackLimit::EngineDefault => None,
        }
    }

    /// Whether one use of the item wears it out.
    pub fn is_damageable(self) -> bool {
        matches!(self, StackLimit::Damageable)
    }
}

/// One registry entry: its id, its kind, its limit, and the vanilla item a
/// content declaration copies.
///
/// `template` is `Some` **only where Java's own superclass forces it** — a
/// `SwordItem` is a sword, a `TridentItem` a trident, a `BowItem` a bow, an
/// `ArrowItem` an arrow. Everywhere else it is `None`, because the template is
/// what the *model* is a model of, and picking one would be an asset decision
/// dressed as a rule (`RAPPORT.md` §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemEntry {
    /// The registry path, without its namespace: `supplychestdeployer`.
    pub id: &'static str,
    /// What the item is.
    pub category: Category,
    /// How many fit in a slot, as Java states it.
    pub stack: StackLimit,
    /// The vanilla item the declaration copies, when Java's superclass settles it.
    pub template: Option<&'static str>,
    /// `Item.Properties.rarity`, when Java raises it above `Common`.
    pub rarity: Rarity,
    /// `fichier.java:ligne` of the `registry.register` call.
    pub source: &'static str,
}

/// The rarity a declaration carries. A mirror of `mod_api::Rarity` with a `const`
/// constructor, because the SDK's own enum is not `const`-buildable in a table
/// row and does not implement `Ord` — [`tests::the_rarity_table_matches_the_sdk`]
/// checks the mapping in both directions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the four rarities `content::Item.rarity` accepts
pub enum Rarity {
    /// `Rarity.COMMON`, and every item Java says nothing about.
    #[default]
    Common,
    /// `Rarity.UNCOMMON`: said by two items.
    Uncommon,
    /// `Rarity.RARE`: said by none.
    Rare,
    /// `Rarity.EPIC`: said by none.
    Epic,
}

impl Rarity {
    /// The SDK's value, for the declaration.
    pub fn to_sdk(self) -> mod_api::Rarity {
        match self {
            Rarity::Common => mod_api::Rarity::Common,
            Rarity::Uncommon => mod_api::Rarity::Uncommon,
            Rarity::Rare => mod_api::Rarity::Rare,
            Rarity::Epic => mod_api::Rarity::Epic,
        }
    }

    /// The SDK's value as this table's own.
    pub fn from_sdk(rarity: mod_api::Rarity) -> Rarity {
        match rarity {
            mod_api::Rarity::Common => Rarity::Common,
            mod_api::Rarity::Uncommon => Rarity::Uncommon,
            mod_api::Rarity::Rare => Rarity::Rare,
            mod_api::Rarity::Epic => Rarity::Epic,
        }
    }

    /// A stable name, for a test failure.
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
        }
    }
}

/// Why an item question has no answer — never an `unwrap`, always a named value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this module's own refusal set, closed on purpose
pub enum Refusal {
    /// No row names this id. Also what a typo answers.
    UnknownItem { id: &'static str },
    /// The item has no food rule: it is not one of the 64 edibles.
    NotFood { id: &'static str },
    /// The item has no tool rule: it is not one of the 17.
    NotATool { id: &'static str },
    /// The item has no material rule: it is neither an ingredient, a crop, a
    /// bottle nor a catalyst.
    NotAMaterial { id: &'static str },
    /// A declaration needs a template and this row does not carry one
    /// (`Item.template` is a `String`, not an `Option`).
    NoTemplate { id: &'static str },
    /// A stated stack size outside `1..=99`, the range content admission accepts
    /// (`content/content_declarations.rs:249`).
    StackOutOfRange { max: u8 },
    /// A hunger value below one, which no food has and no eating rule can use.
    NoNutrition { id: &'static str },
    /// A food tier outside `1..=3`, the three `getAllFoods` lists
    /// (`ModItems.java:186-254`).
    NoSuchTier { tier: i32 },
}

impl Refusal {
    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownItem { .. } => "unknown_item",
            Refusal::NotFood { .. } => "not_food",
            Refusal::NotATool { .. } => "not_a_tool",
            Refusal::NotAMaterial { .. } => "not_a_material",
            Refusal::NoTemplate { .. } => "no_template",
            Refusal::StackOutOfRange { .. } => "stack_out_of_range",
            Refusal::NoNutrition { .. } => "no_nutrition",
            Refusal::NoSuchTier { .. } => "no_such_tier",
        }
    }
}

/// `Constants.STACKSIZE`, the limit Java names for its own materials
/// (`api/util/constant/Constants.java:23`).
pub const STACKSIZE: u8 = 64;

/// Build a row whose template is not settled by Java's superclass.
const fn item(id: &'static str, category: Category, stack: StackLimit, source: &'static str) -> ItemEntry {
    ItemEntry { id, category, stack, template: None, rarity: Rarity::Common, source }
}

/// Build a row whose template **is** Java's superclass, with the class named in
/// the comment above it.
const fn held(
    id: &'static str,
    category: Category,
    stack: StackLimit,
    template: &'static str,
    source: &'static str,
) -> ItemEntry {
    ItemEntry { id, category, stack, template: Some(template), rarity: Rarity::Common, source }
}

/// Build an `UNCOMMON` row — the two Java raises (`ItemBuildGoggles.java:36`,
/// `ItemScanAnalyzer.java:62`).
const fn uncommon(id: &'static str, category: Category, stack: StackLimit, source: &'static str) -> ItemEntry {
    ItemEntry { id, category, stack, template: None, rarity: Rarity::Uncommon, source }
}

/// The **163** registry entries: the **148** the items initialiser registers, in
/// its own order — the five scrolls first because Java registers them first
/// (`:97-109`), then the block of `:246-384`, then the eighteen spawn eggs — and
/// then the **15 crops**, which their block registers and no item initialiser
/// line would ever list.
///
/// Every source is a `registry.register` line, not the class: the class is shared
/// by several items (four meshes, eight pirate pieces, fifteen crops) and cannot
/// tell them apart, which is the same reason `crate::blocks::ressource` cites its
/// initialiser for the fifteen crops.
pub const ITEMS: &[ItemEntry] = &[
    // -- the five scrolls, `stacksTo(16)` (`ModItemsInitializer.java:96-108`) --
    // `ItemScrollColonyTP`, 146 lines of teleport and a 1-in-10 failure.
    item("scroll_tp", Category::Scroll, StackLimit::Fixed(16), "ModItemsInitializer.java:97"),
    item("scroll_area_tp", Category::Scroll, StackLimit::Fixed(16), "ModItemsInitializer.java:100"),
    item("scroll_buff", Category::Scroll, StackLimit::Fixed(16), "ModItemsInitializer.java:103"),
    item(
        "scroll_guard_help",
        Category::Scroll,
        StackLimit::Fixed(16),
        "ModItemsInitializer.java:106",
    ),
    item(
        "scroll_highlight",
        Category::Scroll,
        StackLimit::Fixed(16),
        "ModItemsInitializer.java:109",
    ),

    // -- the tools and the documents, `:246-310` --
    // `properties.stacksTo(1)` is stated by the class (`ItemSupplyChestDeployer.java:72`).
    item("supplychestdeployer", Category::Deployer, StackLimit::Fixed(1), "ModItemsInitializer.java:246"),
    // `.rarity(Rarity.UNCOMMON)` and `durability(0)`: a scanner that never wears out.
    uncommon("scan_analyzer", Category::Tool, StackLimit::Damageable, "ModItemsInitializer.java:247"),
    item(
        "scepterpermission",
        Category::Tool,
        StackLimit::Fixed(1),
        "ModItemsInitializer.java:248",
    ),
    item("scepterguard", Category::Tool, StackLimit::Fixed(1), "ModItemsInitializer.java:249"),
    // The three hammers: 200/400/1000 durability and reach 1/2/3 (`:66-68`).
    item("assistanthammer_gold", Category::Tool, StackLimit::Damageable, "ModItemsInitializer.java:250"),
    item("assistanthammer_iron", Category::Tool, StackLimit::Damageable, "ModItemsInitializer.java:251"),
    item(
        "assistanthammer_diamond",
        Category::Tool,
        StackLimit::Damageable,
        "ModItemsInitializer.java:252",
    ),
    // `stacksTo(1).durability(0)`: the rally banner never breaks (`ItemBannerRallyGuards.java:69`).
    item(
        "banner_rally_guards",
        Category::Tool,
        StackLimit::Fixed(1),
        "ModItemsInitializer.java:253",
    ),
    item("supplycampdeployer", Category::Deployer, StackLimit::Fixed(1), "ModItemsInitializer.java:254"),
    item("ancienttome", Category::Document, StackLimit::Fixed(64), "ModItemsInitializer.java:255"),
    // `extends SwordItem` with `Tiers.DIAMOND` (`ItemChiefSword.java:33`).
    held(
        "chiefsword",
        Category::Tool,
        StackLimit::Damageable,
        "minecraft:diamond_sword",
        "ModItemsInitializer.java:256",
    ),
    item("clipboard", Category::Document, StackLimit::Fixed(64), "ModItemsInitializer.java:257"),
    item("compost", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:258"),
    item("resourcescroll", Category::Document, StackLimit::Fixed(64), "ModItemsInitializer.java:259"),
    // `extends SwordItem` with `Tiers.IRON` (`ItemIronScimitar.java:19`).
    held(
        "iron_scimitar",
        Category::Tool,
        StackLimit::Damageable,
        "minecraft:iron_sword",
        "ModItemsInitializer.java:260",
    ),
    // The two area scepters say `stacksTo(1)` and no durability
    // (`ItemScepterLumberjack.java:51`, `ItemScepterBeekeeper.java:52`).
    item("scepterlumberjack", Category::Tool, StackLimit::Fixed(1), "ModItemsInitializer.java:261"),
    // `extends BowItem`, `durability(384)` (`ItemPharaoScepter.java:36`).
    held(
        "pharaoscepter",
        Category::Tool,
        StackLimit::Damageable,
        "minecraft:bow",
        "ModItemsInitializer.java:262",
    ),
    // `extends ArrowItem` (`ItemFireArrow.java:17`).
    held(
        "firearrow",
        Category::Ammo,
        StackLimit::EngineDefault,
        "minecraft:arrow",
        "ModItemsInitializer.java:263",
    ),
    item("scepterbeekeeper", Category::Tool, StackLimit::Fixed(1), "ModItemsInitializer.java:264"),
    item("mistletoe", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:265"),
    // `extends TridentItem`, `durability(250)` (`ItemSpear.java:32`).
    held(
        "spear",
        Category::Tool,
        StackLimit::Damageable,
        "minecraft:trident",
        "ModItemsInitializer.java:266",
    ),
    item("questlog", Category::Document, StackLimit::Fixed(64), "ModItemsInitializer.java:267"),
    item("colonymap", Category::Document, StackLimit::Fixed(64), "ModItemsInitializer.java:268"),

    // -- the four doughs, `stacksTo(STACKSIZE)` in their own class (`:270-273`) --
    item("bread_dough", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:270"),
    item("cookie_dough", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:271"),
    item("cake_batter", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:272"),
    item("raw_pumpkin_pie", Category::Material, StackLimit::Fixed(64), "ModItemsInitializer.java:273"),

    // -- the four breads, each with its own class and its own effect --
    item("milky_bread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:275"),
    item("sugary_bread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:276"),
    item("golden_bread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:277"),
    item("chorus_bread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:278"),

    item("adventure_token", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:280"),

    // -- eight pirate pieces, two materials (`ItemPirateGear.java:16-30`) --
    item("pirate_hat", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:282"),
    item("pirate_top", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:283"),
    item("pirate_leggins", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:284"),
    item("pirate_boots", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:285"),
    item("pirate_cap", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:287"),
    item("pirate_chest", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:288"),
    item("pirate_legs", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:289"),
    item("pirate_shoes", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:290"),

    // -- the plate set (`ItemPlateArmor.java:18`) --
    item("plate_armor_helmet", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:292"),
    item("plate_armor_chest", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:293"),
    item("plate_armor_legs", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:294"),
    item("plate_armor_boots", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:295"),

    // `stacksTo(500 * 11)` durability and zero protection (`ItemSantaHead.java:18`).
    item("santa_hat", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:298"),
    // The two gates are registered through block constants, so their ids come from
    // `AbstractBlockGate` and not from a literal (`ModItemsInitializer.java:299-300`).
    item("gate_iron", Category::Block, StackLimit::EngineDefault, "ModItemsInitializer.java:299"),
    item("gate_wood", Category::Block, StackLimit::EngineDefault, "ModItemsInitializer.java:300"),
    item("colony_banner", Category::Block, StackLimit::Fixed(16), "ModItemsInitializer.java:301"),

    // -- four meshes, `durability(500/1000/1500/2000).setNoRepair()` (`:131-134`) --
    item(
        "sifter_mesh_string",
        Category::Tool,
        StackLimit::Damageable,
        "ModItemsInitializer.java:304",
    ),
    item(
        "sifter_mesh_flint",
        Category::Tool,
        StackLimit::Damageable,
        "ModItemsInitializer.java:305",
    ),
    item("sifter_mesh_iron", Category::Tool, StackLimit::Damageable, "ModItemsInitializer.java:306"),
    item(
        "sifter_mesh_diamond",
        Category::Tool,
        StackLimit::Damageable,
        "ModItemsInitializer.java:307",
    ),

    item("magicpotion", Category::Potion, StackLimit::Fixed(16), "ModItemsInitializer.java:309"),
    // `.setNoRepair().rarity(Rarity.UNCOMMON)`, a helmet with no protection
    // (`ItemBuildGoggles.java:36`).
    uncommon("build_goggles", Category::Armour, StackLimit::Damageable, "ModItemsInitializer.java:310"),

    // -- the seven ingredients, plain `Item` (`:233-239`) --
    item("butter", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:312"),
    item("cornmeal", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:370"),
    item("creamcheese", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:371"),
    item("soysauce", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:372"),
    item("muffin_dough", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:326"),
    item("manchet_dough", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:324"),
    item("raw_noodle", Category::Material, StackLimit::EngineDefault, "ModItemsInitializer.java:332"),

    // -- the sixty `ItemFood` rows (`:143-230`), in registration order --
    // Their numbers are in [`food`]; here they are only entries of the registry.
    item("cabochis", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:313"),
    item("cheddar_cheese", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:314"),
    item("congee", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:315"),
    item("cooked_rice", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:316"),
    item("eggplant_dolma", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:317"),
    item("feta_cheese", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:318"),
    item("flatbread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:319"),
    item("hand_pie", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:320"),
    item("lamb_stew", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:321"),
    item("lembas_scone", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:322"),
    item("manchet_bread", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:323"),
    item("muffin", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:325"),
    item("pasta_plain", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:327"),
    item("pasta_tomato", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:328"),
    item("pepper_hummus", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:329"),
    item("pita_hummus", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:330"),
    item("pottage", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:331"),
    item("rice_ball", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:333"),
    item("stew_trencher", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:334"),
    item("stuffed_pepper", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:335"),
    item("stuffed_pita", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:336"),
    item("sushi_roll", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:337"),
    item("tofu", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:338"),
    item("cheese_ravioli", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:340"),
    item("chicken_broth", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:341"),
    item("corn_chowder", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:342"),
    item(
        "spicy_grilled_chicken",
        Category::Food,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:343",
    ),
    item("kebab", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:344"),
    item("meat_ravioli", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:345"),
    item("mint_jelly", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:346"),
    item("mint_tea", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:347"),
    item("pea_soup", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:348"),
    item("polenta", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:349"),
    item("potato_soup", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:350"),
    item("squash_soup", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:351"),
    item("veggie_ravioli", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:352"),
    item("yogurt", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:353"),
    item("baked_salmon", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:354"),
    item("eggdrop_soup", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:355"),
    item("fish_n_chips", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:356"),
    item("kimchi", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:357"),
    item("pierogi", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:358"),
    item("veggie_quiche", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:359"),
    item("veggie_soup", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:360"),
    item(
        "yogurt_with_berries",
        Category::Food,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:361",
    ),
    item("borscht", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:362"),
    item("fish_dinner", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:363"),
    item("mutton_dinner", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:364"),
    item("ramen", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:365"),
    item("fried_rice", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:366"),
    item("schnitzel", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:367"),
    item("steak_dinner", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:368"),
    item("tacos", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:369"),
    item("tortillas", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:373"),
    item("apple_pie", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:374"),
    item("cheese_pizza", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:375"),
    item("mushroom_pizza", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:376"),
    item("plain_cheesecake", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:377"),
    item(
        "mintchoco_cheesecake",
        Category::Food,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:378",
    ),
    item("spicy_eggplant", Category::Food, StackLimit::EngineDefault, "ModItemsInitializer.java:379"),

    // -- the four bottles, each with `craftRemainder(large_empty_bottle)` on
    // three of them (`:241-244`) --
    item(
        "large_empty_bottle",
        Category::Container,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:381",
    ),
    item(
        "large_water_bottle",
        Category::Container,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:382",
    ),
    item(
        "large_milk_bottle",
        Category::Container,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:383",
    ),
    item(
        "large_soy_milk_bottle",
        Category::Container,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:384",
    ),

    // -- eighteen spawn eggs, built inline by the initialiser --
    item("barbarianegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:386"),
    item("barbarcheregg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:390"),
    item("barbchiefegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:394"),
    item("pirateegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:399"),
    item("piratearcheregg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:403"),
    item(
        "piratecaptainegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:407",
    ),
    item("mummyegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:412"),
    item("mummyarcheregg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:416"),
    item("pharaoegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:420"),
    item(
        "shieldmaidenegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:425",
    ),
    item(
        "norsemenarcheregg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:429",
    ),
    item(
        "norsemenchiefegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:433",
    ),
    item("amazonegg", Category::SpawnEgg, StackLimit::EngineDefault, "ModItemsInitializer.java:438"),
    item(
        "amazonspearmanegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:442",
    ),
    item(
        "amazonchiefegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:446",
    ),
    item(
        "drownedpirateegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:451",
    ),
    item(
        "drownedpiratearcheregg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:455",
    ),
    item(
        "drownedpiratecaptainegg",
        Category::SpawnEgg,
        StackLimit::EngineDefault,
        "ModItemsInitializer.java:459",
    ),

    // -- the fifteen crops: items of their **block**, `ItemCrop`, in
    // `ModBlocksInitializer.java:132-147` order. `registerBlockItem` is the one
    // line that makes each of them an item (`MinecoloniesCropBlock.java:229`), so
    // all fifteen cite it, and their numbers (1 hunger point, 0.3 saturation, no
    // tier) are in `food`, not here.
    item("bell_pepper", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("cabbage", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("chickpea", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("durum", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("eggplant", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("garlic", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("onion", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("soybean", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("tomato", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("rice", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("butternut_squash", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("corn", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("mint", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("nether_pepper", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
    item("peas", Category::Food, StackLimit::EngineDefault, "MinecoloniesCropBlock.java:229"),
];

/// The registry path a row carries, with the namespace: `minecolonies:spear`.
pub fn full_id(entry: &ItemEntry) -> String {
    format!("{}:{}", NAMESPACE, entry.id)
}

/// The row an id names.
///
/// The namespace is optional and ignored when it is [`NAMESPACE`], exactly as
/// `crate::blocks::block` reads a block path: no other mod's item is loadable by
/// this one, so a hand-written `minecolonies:spear` must find its row.
pub fn get(path: &str) -> Option<&'static ItemEntry> {
    let name = match path.split_once(':') {
        None => path,
        Some((namespace, rest)) if namespace == NAMESPACE => rest,
        Some(_) => return None,
    };
    ITEMS.iter().find(|entry| entry.id == name)
}

/// The row an id names, or a named refusal — the companion of [`get`] for a
/// caller that has an error type already.
pub fn require(path: &'static str) -> Result<&'static ItemEntry, Refusal> {
    get(path).ok_or(Refusal::UnknownItem { id: path })
}

/// Every row of a category, in table order.
pub fn of_category(category: Category) -> impl Iterator<Item = &'static ItemEntry> {
    ITEMS.iter().filter(move |entry| entry.category == category)
}

/// The number of rows a category holds.
pub fn count_of(category: Category) -> usize {
    of_category(category).count()
}

impl ItemEntry {
    /// The id with its namespace.
    pub fn full_id(&self) -> String {
        full_id(self)
    }

    /// The stack size **Java stated**, checked against the range content
    /// admission accepts.
    ///
    /// `Ok(Some(n))` for the **25** rows that state one, `Ok(None)` for the 123
    /// that do not, and `Err(StackOutOfRange)` for a table typo — never a silent
    /// clamp.
    pub fn stated_stack(&self) -> Result<Option<u8>, Refusal> {
        match self.stack.stated() {
            Some(max) if !(1..=99).contains(&max) => Err(Refusal::StackOutOfRange { max }),
            other => Ok(other),
        }
    }

    /// Whether the item is one of the 64 edibles.
    pub fn is_food(&self) -> bool {
        self.category == Category::Food
    }

    /// Whether one use wears it out.
    pub fn is_damageable(&self) -> bool {
        self.stack.is_damageable()
    }

    /// The content declaration this row asks for.
    ///
    /// `Err(NoTemplate)` for the rows whose template is an asset decision: a
    /// declaration **requires** one (`content::Item.template` is a `String`), and
    /// inventing `minecraft:paper` for the clipboard would make a guess look like
    /// a rule.
    ///
    /// Four fields are set from the table — the namespaced id, the template,
    /// `max_stack_size` from [`StackLimit::Fixed`] and `rarity` — plus
    /// `crafting_remainder` and `block` from [`materials`]. The **components**
    /// (durability, food, protection) are not built here: a component patch is
    /// `Vec<(String, Option<Tag>)>`, and a `Tag` has no constructor a `const` table
    /// can call (`MANQUANT.md` §7). The numbers they would carry are in [`tools`]
    /// and [`food`].
    pub fn declaration(&self) -> Result<mod_api::Item, Refusal> {
        let template = self.template.ok_or(Refusal::NoTemplate { id: self.id })?;
        let mut declaration = mod_api::Item::new(self.full_id(), template);
        declaration.max_stack_size = self.stated_stack()?;
        declaration.rarity = Some(self.rarity.to_sdk());
        declaration.crafting_remainder = materials::craft_remainder(self.id).map(str::to_owned);
        if let Some(block) = materials::block_of(self.id) {
            declaration.block = Some(block.to_owned());
        }
        Ok(declaration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_the_one_hundred_and_sixty_three_items() {
        assert_eq!(ITEMS.len(), 163);
        // 148 register calls by the items initialiser: 128 name a `ModItems.<field>`,
        // 2 use a block constant (`IRON_GATE`, `WOODEN_GATE`) and 18 build a spawn
        // egg inline (`ModItemsInitializer.java:246-467`).
        let from_initialiser = ITEMS
            .iter()
            .filter(|entry| entry.source.starts_with("ModItemsInitializer.java:"))
            .count();
        assert_eq!(from_initialiser, 148);
        // and the 15 crops, which their **block** registers, not the item list.
        assert_eq!(ITEMS.len() - from_initialiser, 15);
        assert_eq!(count_of(Category::SpawnEgg), 18);
        assert_eq!(count_of(Category::Block), 3);
    }

    #[test]
    fn no_item_has_two_rows_and_every_row_names_itself() {
        for (index, entry) in ITEMS.iter().enumerate() {
            for other in &ITEMS[index + 1..] {
                assert_ne!(entry.id, other.id, "{} has two rows", entry.id);
            }
            assert!(!entry.id.is_empty());
            assert!(
                entry.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{} is not a registry path",
                entry.id
            );
            assert_eq!(get(entry.id), Some(entry));
            assert_eq!(get(&entry.full_id()), Some(entry));
        }
    }

    #[test]
    fn every_row_cites_the_line_that_registers_it() {
        for entry in ITEMS {
            assert!(entry.source.contains(".java:"), "{} cites {}", entry.id, entry.source);
            assert_eq!(entry.source.matches(':').count(), 1, "{}", entry.id);
            if entry.source.starts_with("ModItemsInitializer.java:") {
                let line: u32 = entry.source.rsplit(':').next().unwrap().parse().unwrap();
                assert!((1..=497).contains(&line), "{} cites line {line}", entry.id);
            } else {
                // the fifteen crops: the one `registerBlockItem` line
                assert_eq!(entry.source, "MinecoloniesCropBlock.java:229", "{}", entry.id);
            }
        }
    }

    #[test]
    fn the_categories_cover_every_item_exactly_once() {
        assert_eq!(Category::ALL.len(), 12);
        let mut total = 0;
        for category in Category::ALL {
            total += count_of(category);
            assert!(count_of(category) > 0, "{} is empty", category.name());
        }
        assert_eq!(total, ITEMS.len());
        // The twelve counts, measured on the table above: 79 food = 64 of the item
        // initialiser + the 15 crops.
        assert_eq!(count_of(Category::Tool), 17);
        assert_eq!(count_of(Category::Food), 79);
        assert_eq!(count_of(Category::Material), 14);
        assert_eq!(count_of(Category::Armour), 14);
        assert_eq!(count_of(Category::Scroll), 5);
        assert_eq!(count_of(Category::Document), 5);
        assert_eq!(count_of(Category::Deployer), 2);
        assert_eq!(count_of(Category::Ammo), 1);
        assert_eq!(count_of(Category::Container), 4);
        assert_eq!(count_of(Category::Potion), 1);
    }

    #[test]
    fn every_category_has_a_unique_name() {
        for (index, category) in Category::ALL.iter().enumerate() {
            for other in &Category::ALL[index + 1..] {
                assert_ne!(category.name(), other.name());
            }
        }
    }

    #[test]
    fn a_stated_stack_size_is_one_sixteen_or_sixty_four() {
        for entry in ITEMS {
            match entry.stated_stack().unwrap() {
                Some(max) => assert!(
                    max == 1 || max == 16 || max == 64,
                    "{} states {max}",
                    entry.id
                ),
                None => {}
            }
        }
        assert_eq!(STACKSIZE, 64);
    }

    #[test]
    fn twenty_five_rows_state_a_stack_size_and_twenty_six_are_damageable() {
        let stated = ITEMS.iter().filter(|entry| entry.stack.stated().is_some()).count();
        let damageable = ITEMS.iter().filter(|entry| entry.is_damageable()).count();
        // 5 scrolls + 7 tools that say `stacksTo(1)` + 2 deployers + 11 materials
        // that say `STACKSIZE` = 25; and the 26 with a durability and no number.
        assert_eq!(stated, 25);
        assert_eq!(damageable, 26);
        assert_eq!(
            stated
                + damageable
                + ITEMS.iter().filter(|e| e.stack == StackLimit::EngineDefault).count(),
            ITEMS.len()
        );
    }

    #[test]
    fn a_damageable_item_states_no_number_and_a_number_states_no_damage() {
        for entry in ITEMS {
            if entry.stack.stated().is_some() {
                assert!(!entry.stack.is_damageable(), "{}", entry.id);
            }
        }
        // The four meshes are the case that reads wrong: no `stacksTo`, a
        // durability, and `.setNoRepair()`.
        assert_eq!(get("sifter_mesh_string").unwrap().stack, StackLimit::Damageable);
        assert_eq!(get("sifter_mesh_string").unwrap().stated_stack(), Ok(None));
        // The two rulers that *do* state both.
        assert_eq!(get("banner_rally_guards").unwrap().stack, StackLimit::Fixed(1));
        assert_eq!(get("scepterguard").unwrap().stack, StackLimit::Fixed(1));
    }

    #[test]
    fn an_out_of_range_stack_size_is_refused_by_name() {
        let broken = ItemEntry {
            id: "broken",
            category: Category::Material,
            stack: StackLimit::Fixed(100),
            template: None,
            rarity: Rarity::Common,
            source: "x.java:1",
        };
        assert_eq!(broken.stated_stack(), Err(Refusal::StackOutOfRange { max: 100 }));
        assert_eq!(Refusal::StackOutOfRange { max: 100 }.name(), "stack_out_of_range");
    }

    #[test]
    fn an_unknown_id_answers_none_and_a_foreign_namespace_is_not_read() {
        assert_eq!(get("no_such_item"), None);
        assert_eq!(get("minecraft:spear"), None, "not this mod's item");
        assert_eq!(get("minecolonies:spear").map(|entry| entry.id), Some("spear"));
        assert_eq!(get("Spear"), None, "the table is lower case");
        assert_eq!(require("no_such_item"), Err(Refusal::UnknownItem { id: "no_such_item" }));
        assert_eq!(Refusal::UnknownItem { id: "x" }.name(), "unknown_item");
        assert_eq!(require("spear").map(|entry| entry.id), Ok("spear"));
    }

    #[test]
    fn the_seven_ingredients_are_materials_and_the_eighteen_eggs_are_not() {
        for id in materials::INGREDIENTS {
            assert_eq!(get(id).unwrap().category, Category::Material, "{id}");
        }
        for entry in of_category(Category::SpawnEgg) {
            assert!(entry.id.ends_with("egg"), "{}", entry.id);
        }
    }

    #[test]
    fn a_declaration_copies_the_template_and_the_stated_limit() {
        let spear = get("spear").unwrap().declaration().unwrap();
        assert_eq!(spear.template, "minecraft:trident");
        assert_eq!(spear.max_stack_size, None, "Java states none for the spear");
        assert_eq!(spear.id, "minecolonies:spear");

        let sword = get("chiefsword").unwrap().declaration().unwrap();
        assert_eq!(sword.template, "minecraft:diamond_sword");
        assert_eq!(sword.rarity, Some(mod_api::Rarity::Common));

        // The one row that states both a template **and** a limit: none does —
        // every templated item is damageable, so `max_stack_size` stays `None` and
        // the engine's own one-per-slot rule applies.
        let scroll = get("scroll_tp").unwrap().declaration();
        assert_eq!(scroll, Err(Refusal::NoTemplate { id: "scroll_tp" }));
    }

    #[test]
    fn a_row_without_a_template_is_refused_rather_than_given_a_paper_one() {
        assert_eq!(
            get("clipboard").unwrap().declaration(),
            Err(Refusal::NoTemplate { id: "clipboard" })
        );
        assert_eq!(Refusal::NoTemplate { id: "clipboard" }.name(), "no_template");
        // Five rows carry a template, each because a superclass names the vanilla
        // item: two swords, a trident, a bow, an arrow.
        let templated: Vec<&str> =
            ITEMS.iter().filter(|entry| entry.template.is_some()).map(|entry| entry.id).collect();
        assert_eq!(
            templated,
            vec!["chiefsword", "iron_scimitar", "pharaoscepter", "firearrow", "spear"]
        );
    }

    #[test]
    fn the_two_uncommon_items_are_the_two_java_raises() {
        let uncommon: Vec<&str> = ITEMS
            .iter()
            .filter(|entry| entry.rarity == Rarity::Uncommon)
            .map(|entry| entry.id)
            .collect();
        assert_eq!(uncommon, vec!["scan_analyzer", "build_goggles"]);
        for entry in ITEMS {
            assert!(
                entry.rarity != Rarity::Rare && entry.rarity != Rarity::Epic,
                "{}",
                entry.id
            );
        }
    }

    #[test]
    fn the_rarity_table_matches_the_sdk() {
        assert_eq!(Rarity::Common.to_sdk(), mod_api::Rarity::Common);
        assert_eq!(Rarity::Uncommon.to_sdk(), mod_api::Rarity::Uncommon);
        assert_eq!(Rarity::Rare.to_sdk(), mod_api::Rarity::Rare);
        assert_eq!(Rarity::Epic.to_sdk(), mod_api::Rarity::Epic);
        for rarity in [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic] {
            assert_eq!(Rarity::from_sdk(rarity.to_sdk()), rarity);
            assert!(!rarity.name().is_empty());
        }
        assert_eq!(Rarity::default(), Rarity::Common);
    }

    #[test]
    fn the_block_items_name_blocks_the_blocks_module_already_has() {
        // The three block items are the only rows whose item is also a block, and
        // their block half lives in `crate::blocks`; a name that module does not
        // know would be an item nobody can place.
        let mut checked = 0;
        for entry in of_category(Category::Block) {
            let block = crate::blocks::block(entry.id);
            assert!(block.is_some(), "{} is a block item of a block we lack", entry.id);
            checked += 1;
        }
        assert_eq!(checked, 3);
        assert_eq!(crate::blocks::block("gate_iron").map(|block| block.name), Some("gate_iron"));
    }

    #[test]
    fn every_food_row_has_a_food_rule_and_every_other_row_does_not() {
        for entry in ITEMS {
            let rule = food::of(entry.id);
            if entry.is_food() {
                let rule = rule.unwrap_or_else(|| panic!("{} is food with no rule", entry.id));
                assert_eq!(rule.item, entry.id);
            } else {
                assert!(rule.is_none(), "{} is not food but has a food rule", entry.id);
                assert_eq!(
                    food::require(entry.id),
                    Err(Refusal::NotFood { id: entry.id })
                );
            }
        }
        assert_eq!(food::ALL.len(), 79);
    }

    #[test]
    fn every_tool_row_that_has_rules_is_a_tool_and_the_reverse_holds() {
        // The 17 tools: 13 have a row in `tools` and four are the meshes, which
        // are a durability table and nothing else.
        let with_rules: Vec<&str> = of_category(Category::Tool)
            .map(|entry| entry.id)
            .filter(|id| tools::stats(id).is_some())
            .collect();
        assert_eq!(with_rules.len(), 13);
        for entry in ITEMS {
            if tools::stats(entry.id).is_some() {
                assert_eq!(entry.category, Category::Tool, "{}", entry.id);
            }
        }
    }

    #[test]
    fn the_materials_are_the_fourteen_the_java_lists_name() {
        for entry in of_category(Category::Material) {
            assert!(
                materials::of(entry.id).is_some(),
                "{} is a material with no rule",
                entry.id
            );
        }
        assert_eq!(materials::ALL.len(), 14);
    }

    #[test]
    fn the_namespace_is_the_one_the_blocks_use() {
        assert_eq!(NAMESPACE, "minecolonies");
        assert_eq!(full_id(&ITEMS[0]), "minecolonies:scroll_tp");
        assert_eq!(get("scroll_tp").unwrap().full_id(), "minecolonies:scroll_tp");
    }
}
