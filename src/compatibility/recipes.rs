//! The colony's recipes, as a recipe viewer publishes them: **six categories**,
//! their ids and titles, the item list each one shows, and the slot geometry
//! four of them share.
//!
//! Java reference: `core/compatibility/jei/` (19 files, 2 056 lines),
//! `core/colony/crafting/{ToolUsage,ToolsAnalyzer,LootTableAnalyzer}.java` for
//! what the categories publish, `api/crafting/{IGenericRecipe,CompostRecipe}.java`
//! for the records themselves.
//!
//! # The published list, measured
//!
//! | Category | Recipe type id | Title | Catalyst |
//! | --- | --- | --- | --- |
//! | Crops | `minecolonies:crops` | `com.minecolonies.coremod.jei.crops` | none¹ |
//! | Composting | `minecolonies:composting` | the barrel's own name² | `minecolonies:barrel_block`, `minecolonies:blockhutcomposter` |
//! | Fishing | `minecolonies:fishing` | the fisherman job's name | `minecolonies:blockhutfisherman` |
//! | Flowers | `minecolonies:flowers` | the florist job's name | `minecolonies:blockhutflorist` |
//! | Tools | `minecolonies:tools` | `com.minecolonies.coremod.jei.tools` | none |
//! | one per job | the job's own key³ | the job's name | the job's building block |
//!
//! ¹ A crop recipe *does* carry a hoe, as a slot in the layout
//! (`CropRecipeCategory.java:104-111`), not as a recipe catalyst — a catalyst is
//! what makes the category appear when the player looks up that item
//! (`JEIPlugin.java:203-211`).
//!
//! ² `Component.translatable(ModBlocks.blockBarrel.getDescriptionId())`
//! (`CompostRecipeCategory.java:42`): the category is titled after the block, not
//! after a JEI key. The port keeps the block id and lets the game name it.
//!
//! ³ `RecipeType.create(uid.getNamespace(), uid.getPath(), IGenericRecipe.class)`
//! where `uid` is the job's registry key (`GenericRecipeCategory.java:101-105`),
//! so the id a job category publishes **is** the job's id — `minecolonies:baker`
//! and so on. [`job_category_id`] is that identity, written down once.
//!
//! The five fixed ids are `ModRecipeTypes.java:11-24`; the catalysts are
//! `JEIPlugin.java:203-211` except the per-job one, which is
//! `JobBasedRecipeCategory.java:126-129` (`new ItemStack(building.getBuildingBlock())`).
//!
//! # What one recipe of each category holds
//!
//! | Category | In | Out | Notes |
//! | --- | --- | --- | --- |
//! | Crops | the source block | the block's loot table, by chance | the block is the *input*, drawn as its own item (`CropRecipeCategory.java:112-115`) |
//! | Composting | one item | 6 × `minecolonies:compost` | **the strength 2/4/8 is not shown**: the output is fixed by `CompostRecipe`'s constructor (`CompostRecipeCategory.java:91-98`, `api/crafting/CompostRecipe.java:31`, `:44`) |
//! | Fishing | nothing | the common table **plus** one bonus table per level | drops consolidated above 18 (`FishermanRecipeCategory.java:89-99`, `:226`) |
//! | Flowers | nothing | the plantables of level 1..5, at most 24 slots | compost as a catalyst slot (`FloristRecipeCategory.java:104-115`) |
//! | Tools | nothing | — | 6 columns × 2 rows: one column per building level, plain over enchanted (`ToolRecipeCategory.java:96-109`) |
//! | per job | the recipe's inputs | primary + extra + additional outputs, then chance slots | plus a block drawn as "intermediate" and the required animal (`GenericRecipeCategory.java:129-271`) |
//!
//! **The per-job category is the only general one.** `IGenericRecipe` is what
//! `RecipeAnalyzer` makes out of a vanilla recipe, a crafter recipe, a loot table
//! or an animal's herding drops, and every crafting hut's category shows the same
//! record with a different filter. [`JobRecipe`] is that record.
//!
//! # What this file does not do
//!
//! No drawing: the citizen head, the animal, the animated barrel and the spinning
//! intermediate block are `JobBasedRecipeCategory.draw` (`:206-242`) and
//! `GenericRecipeCategory.draw` (`:273-316`), and Horizon has no primitive for a
//! viewer category at all (`MANQUANT.md` §1). What is left after that is the
//! **list**: which item, from which inputs, at which level, with what chance.
//!
//! Nothing here formats a chance as text either. Java hands a number to
//! `Component.translatable` (`JobBasedRecipeCategory.java:404-417`) and the game's
//! language file formats it; [`Chance`] keeps the number.

use crate::compatibility::Documented;
use crate::manager_02::LootDrop;

/// `PARTIAL_JEI_INFO`, `TranslationConstants.java:746`: the prefix of every key
/// these categories use.
pub const PARTIAL_JEI_INFO: Documented<&str> =
    Documented::new("com.minecolonies.coremod.jei.", "TranslationConstants.java:746");

/// The highest building level, `api/util/constant/Constants.java:20`. A recipe is unlocked at a
/// level 1..=5, and a tool has a column per level **0..=5** — six, not five,
/// because level 0 exists and carries the tools a level-1 hut already accepts
/// (`ToolRecipeCategory.java:96`).
pub const MAX_BUILDING_LEVEL: Documented<i32> = Documented::new(5, "api/util/constant/Constants.java:20");

/// Above this many drops a category shows them merged, `LootTableAnalyzer.consolidate`
/// (`GenericRecipeCategory.java:344`, `FishermanRecipeCategory.java:114`).
pub const CONSOLIDATE_ABOVE: Documented<usize> =
    Documented::new(18, "GenericRecipeCategory.java:344, FishermanRecipeCategory.java:114");

/// The most slots the florist category spends on flowers, `FloristRecipeCategory.java:44`.
pub const MAX_LOOT_SLOTS: Documented<usize> = Documented::new(24, "FloristRecipeCategory.java:44");

/// The recipe panel's width in GUI pixels, `JobBasedRecipeCategory.java:81`.
pub const WIDTH: Documented<i32> = Documented::new(167, "JobBasedRecipeCategory.java:81");

/// The recipe panel's height, `JobBasedRecipeCategory.java:82`.
pub const HEIGHT: Documented<i32> = Documented::new(120, "JobBasedRecipeCategory.java:82");

/// The citizen's box height, `JobBasedRecipeCategory.java:86`. The only layout
/// number a *rule* depends on (a grid is anchored to its bottom): the rest are
/// plain `const` with their line in the doc.
pub const CITIZEN_H: Documented<i32> = Documented::new(71, "JobBasedRecipeCategory.java:86");

/// The seven enchantments `ToolsAnalyzer` tries on a tool, in this order, at each
/// level 1..5 (`ToolsAnalyzer.java:60-66`). Only one that `canEnchant` the stack
/// *and* whose own range holds the level is applied (`:78`), so a real tool
/// carries about two of them (`:58`).
///
/// **The values are Java's field names, not registry ids, on purpose.** The
/// source says `Enchantments.UNBREAKING`; the id behind that field is 1.20.1's
/// (`minecraft:unbreaking`) and several of them were renamed or reshaped for the
/// enchantment data component — an id written here would be a 26.3 claim this file
/// cannot check. A caller maps the field name to whatever the game's registry
/// holds, once.
pub const ENCHANTMENTS: Documented<[&str; 7]> = Documented::new(
    [
        "UNBREAKING",
        "MOB_LOOTING",
        "BANE_OF_ARTHROPODS",
        "FLAMING_ARROWS",
        "BLOCK_FORTUNE",
        "ALL_DAMAGE_PROTECTION",
        "FISHING_SPEED",
    ],
    "ToolsAnalyzer.java:60-66",
);

// The citizen's box (`JobBasedRecipeCategory.java:83-86`) and the panel it sits in.
const CITIZEN_X: i32 = 2;
const CITIZEN_Y: i32 = 46;
const CITIZEN_W: i32 = 47;
/// JEI's slot drawable, `IGuiHelper.getSlotDrawable`: 18 × 18 GUI pixels. It is an
/// *input* to the geometry below, not a constant of this port — every column and
/// row count is a division by it (`GenericRecipeCategory.java:172-174`).
const SLOT: i32 = 18;

/// `CITIZEN_X + CITIZEN_W + 32` (`GenericRecipeCategory.java:86`).
const INPUT_SLOT_X: i32 = CITIZEN_X + CITIZEN_W + 32;
/// `WIDTH - INPUT_SLOT_X` (`:87`) → 86, i.e. four columns.
const INPUT_SLOT_W: i32 = WIDTH.value - INPUT_SLOT_X;
/// `CITIZEN_X + CITIZEN_W + 4` (`:84`), the same in `FishermanRecipeCategory.java:39`
/// and `FloristRecipeCategory.java:42`.
const LOOT_SLOTS_X: i32 = CITIZEN_X + CITIZEN_W + 4;
/// `WIDTH - LOOT_SLOTS_X` (`:85`; `FishermanRecipeCategory.java:40`,
/// `FloristRecipeCategory.java:43`) → 114, i.e. six columns.
const LOOT_SLOTS_W: i32 = WIDTH.value - LOOT_SLOTS_X;
/// `outputSlotX`, where a job card puts its primary output
/// (`GenericRecipeCategory.java:66`): `CITIZEN_X + CITIZEN_W + 2 + (30 - slot) / 2`
/// = 57 for an 18-pixel slot.
pub const OUTPUT_SLOT_X: i32 = CITIZEN_X + CITIZEN_W + 2 + (30 - SLOT) / 2;
/// `outputSlotY` (`:67`): `CITIZEN_Y + CITIZEN_H + 1 - slot` = 100.
pub const OUTPUT_SLOT_Y: i32 = CITIZEN_Y + CITIZEN_H.value + 1 - SLOT;
/// The crop category's own panel, `CropRecipeCategory.java:42-43`.
const CROP_WIDTH: i32 = 150;
const CROP_HEIGHT: i32 = 54;

/// The five categories whose id is fixed in `ModRecipeTypes.java:11-24`. The
/// sixth family — one category per job — has no fixed id and is [`job_category_id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Category {
    Crops,
    Composting,
    Fishing,
    Flowers,
    Tools,
}

impl Category {
    /// The recipe type id the viewer keys the category by: `MOD_ID + ":" + path`
    /// (`ModRecipeTypes.java:11-24`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Crops => "minecolonies:crops",
            Self::Composting => "minecolonies:composting",
            Self::Fishing => "minecolonies:fishing",
            Self::Flowers => "minecolonies:flowers",
            Self::Tools => "minecolonies:tools",
        }
    }

    /// What the tab is called. Two categories are named after a shipped key, two
    /// after their job, one after a block — see [`Title`].
    pub fn title(self) -> Title {
        match self {
            // `PARTIAL_JEI_INFO + "crops"` (`CropRecipeCategory.java:95`).
            Self::Crops => Title::Key("com.minecolonies.coremod.jei.crops"),
            // The barrel's description id (`CompostRecipeCategory.java:42`).
            Self::Composting => Title::Block("minecolonies:barrel_block"),
            // `job.getJobRegistryEntry().getTranslationKey()`
            // (`JobBasedRecipeCategory.java:154`), for `ModJobs.fisherman`
            // (`FishermanRecipeCategory.java:33`).
            Self::Fishing => Title::Job("minecolonies:fisherman"),
            // As above for `ModJobs.florist` (`FloristRecipeCategory.java:36`).
            Self::Flowers => Title::Job("minecolonies:florist"),
            // `PARTIAL_JEI_INFO + "tools"` (`ToolRecipeCategory.java:71`).
            Self::Tools => Title::Key("com.minecolonies.coremod.jei.tools"),
        }
    }

    /// The item stacks that bring the category up when the player looks one of
    /// them up (`JEIPlugin.java:203-206`). Empty for the two categories that are
    /// not a block's: crops are shown from the hoe slot, tools from a plate armour
    /// chest icon (`ToolRecipeCategory.java:47`), neither of which is a catalyst.
    pub fn catalysts(self) -> &'static [&'static str] {
        match self {
            // `blockBarrel` and `blockHutComposter` (`JEIPlugin.java:203-204`).
            Self::Composting => &["minecolonies:barrel_block", "minecolonies:blockhutcomposter"],
            // `blockHutFisherman` (`:205`).
            Self::Fishing => &["minecolonies:blockhutfisherman"],
            // `blockHutFlorist` (`:206`).
            Self::Flowers => &["minecolonies:blockhutflorist"],
            Self::Crops | Self::Tools => &[],
        }
    }

    /// The description key shown at the top-left of every card of the category,
    /// when it has one. Only the job-based categories have a description
    /// (`JobBasedRecipeCategory.java:102-104` — `PARTIAL_JEI_INFO + job path`), and
    /// the five fixed ones are not job-based except the two named after a job.
    pub fn description(self) -> Option<&'static str> {
        match self {
            Self::Fishing => Some("com.minecolonies.coremod.jei.fisherman"),
            Self::Flowers => Some("com.minecolonies.coremod.jei.florist"),
            Self::Crops | Self::Composting | Self::Tools => None,
        }
    }

    /// The one ingredient-info entry these categories add: the composter hut
    /// explains itself with `PARTIAL_JEI_INFO + ModJobs.COMPOSTER_ID.getPath()`
    /// (`JEIPlugin.java:165-166`, `ModJobs.java:33`).
    pub fn ingredient_info(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Composting => Some((
                "minecolonies:blockhutcomposter",
                "com.minecolonies.coremod.jei.composter",
            )),
            _ => None,
        }
    }
}

/// The id a job's own category publishes.
///
/// `RecipeType.create(uid.getNamespace(), uid.getPath(), …)` rebuilds the key it
/// was given (`GenericRecipeCategory.java:101-105`), so the answer is the job id
/// itself — written as a function because "the category is named after the job"
/// is the rule, and a caller should not have to know it is the identity.
pub fn job_category_id(job: &str) -> String {
    job.to_owned()
}

/// A job's **translation key**: `"com." + namespace + ".job." + path`
/// (`JobEntry.java:129`), which is what `getTranslationKey()` returns and what a
/// job-based category is titled with (`JobBasedRecipeCategory.java:154`).
///
/// `minecolonies:fisherman` → `com.minecolonies.job.fisherman`. `None` for a string
/// that is not an id: Java's key is a `ResourceLocation`, which always carries a
/// namespace, so a bare `fisherman` is a caller error rather than a `minecraft:`
/// job.
pub fn job_translation_key(job: &str) -> Option<String> {
    let (namespace, path) = job.split_once(':')?;
    if namespace.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("com.{namespace}.job.{path}"))
}

/// What a category's tab is called (`JobBasedRecipeCategory.java:146-155`,
/// `CompostRecipeCategory.java:42`, `CropRecipeCategory.java:95`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Title {
    /// A translation key this mod ships, complete.
    Key(&'static str),
    /// A job's id: the game names it from the job entry, so a translation change
    /// does not need this port to change (`JobBasedRecipeCategory.java:154`).
    Job(&'static str),
    /// A block's id: the game names it from the block
    /// (`CompostRecipeCategory.java:42`).
    Block(&'static str),
}

/// One tool type and the items a hut of each level accepts, `ToolUsage`
/// (`core/colony/crafting/ToolUsage.java:19-34`).
///
/// The two lists are **six** long, index = building level **0..=5**, which is
/// `MAX_BUILDING_LEVEL + 1` and not `MAX_BUILDING_LEVEL`: level 0 is the step a
/// level-1 hut accepts a wooden tool in.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ToolUsage {
    /// The equipment type's id, e.g. `minecolonies:hoe`.
    pub tool: &'static str,
    /// The plain items, per level.
    pub levels: Vec<Vec<String>>,
    /// The enchanted items, per level.
    pub enchanted: Vec<Vec<String>>,
}

impl ToolUsage {
    /// `ToolUsage.create`: six empty lists on each side (`ToolUsage.java:24-34`).
    pub fn new(tool: &'static str) -> Self {
        let levels = vec![Vec::new(); (MAX_BUILDING_LEVEL.value + 1) as usize];
        ToolUsage {
            tool,
            levels: levels.clone(),
            enchanted: levels,
        }
    }

    /// The level a stack lands in, or `None` when the tool refuses it.
    ///
    /// `ToolsAnalyzer.java:88-92`: `getMiningLevel` below zero means the item is
    /// not this tool at all; otherwise the level is the mining level **plus**
    /// `ItemStackUtils.getMaxEnchantmentLevel`, capped at the max building level.
    /// The cap is why a heavily enchanted wooden pickaxe can sit in a level-5 slot.
    ///
    /// `enchant_bonus` is [`ToolUsage::enchant_bonus`]'s answer, **not** the
    /// enchantment level: see that method for the off-by-one.
    pub fn level_of(mining_level: i32, enchant_bonus: i32) -> Option<usize> {
        if mining_level < 0 {
            return None;
        }
        let level = mining_level + enchant_bonus;
        Some(level.min(MAX_BUILDING_LEVEL.value) as usize)
    }

    /// `ItemStackUtils.getMaxEnchantmentLevel` (`ItemStackUtils.java:279-299`): the
    /// **highest enchantment level minus one**, floored at zero.
    ///
    /// That is what `ToolsAnalyzer` adds to the mining level (`:92`), so a tool
    /// carrying only Unbreaking I adds nothing, and Unbreaking III adds 2. It is
    /// deliberately not the level itself.
    pub fn enchant_bonus(highest_level: i32) -> i32 {
        (highest_level - 1).max(0)
    }

    /// Adds one stack where [`ToolUsage::level_of`] puts it
    /// (`ToolsAnalyzer.java:84-104`).
    ///
    /// `enchanted` is `stack.isEnchanted()` (`:96`), and it is a **separate
    /// answer** from `enchant_bonus`: a stack enchanted only at level I has a bonus
    /// of 0 and still belongs to the enchanted row, so the two cannot be derived
    /// from one number.
    ///
    /// Returns the level it went to, which is `None` for a refused stack — Java
    /// returns in silence (`:89-91`) and the caller only sees a missing slot.
    pub fn add(
        &mut self,
        stack: &str,
        mining_level: i32,
        enchant_bonus: i32,
        enchanted: bool,
    ) -> Option<usize> {
        let level = Self::level_of(mining_level, enchant_bonus)?;
        let target = if enchanted {
            &mut self.enchanted
        } else {
            &mut self.levels
        };
        target.get_mut(level)?.push(stack.to_owned());
        Some(level)
    }

    /// The slot name the layout uses: `"L" + level`, and `"L" + level + "e"` for
    /// the enchanted row (`ToolRecipeCategory.java:99`, `:104`).
    pub fn slot_name(level: i32, enchanted: bool) -> String {
        if enchanted {
            format!("L{level}e")
        } else {
            format!("L{level}")
        }
    }

    /// How many slots the category draws: two rows of six
    /// (`ToolRecipeCategory.java:96-109`).
    pub fn slot_count() -> usize {
        2 * (MAX_BUILDING_LEVEL.value as usize + 1)
    }
}

/// The rules `ToolsAnalyzer` applies to a stack before it lands in a slot:
/// `level_of`, plus the enchantment trial loop (`ToolsAnalyzer.java:28-50`,
/// `:52-72`).
///
/// Kept as a description rather than a builder because the trial needs
/// `Enchantment.canEnchant(stack)` and `getMinLevel`/`getMaxLevel`, which are the
/// game's — the port does not reproduce an enchantment table. What it does
/// reproduce is the shape: `enchantLevel` 1..5, the seven [`ENCHANTMENTS`], and
/// the "only if it ended up enchanted" test (`:68-71`).
pub fn enchant_levels() -> std::ops::RangeInclusive<i32> {
    // `for (int enchantLevel = 1; enchantLevel < 6; ++enchantLevel)`
    // (`ToolsAnalyzer.java:41`) — 1..=5, the same five steps as the building.
    1..=5
}

/// A crop recipe: a block whose loot table the farmer sows,
/// `CropRecipeCategory.CropRecipe` (`CropRecipeCategory.java:145`).
///
/// The category is built from the **union** of `getDroppedFrom()` over every
/// colony crop block (`:198-207`), so two crops dropping from the same block give
/// one card, not two — which is why `findRecipes` collects into a `Set<Block>`
/// first (`:200-204`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct CropRecipe {
    /// The block the drops come from, e.g. `minecraft:wheat`.
    pub source: &'static str,
}

/// One composting card: an item and the strength it carries,
/// `CompostRecipeCategory.findRecipes` (`:51-56`) over
/// `CompostRecipe.individualize` (`api/crafting/CompostRecipe.java:132-138`).
///
/// **The strength never reaches the player through this category.** The card shows
/// the input and `getResultItem(null)`, which the recipe's constructor fixes at six
/// compost (`:44`); the 2/4/8 that says how many items fill a barrel is only in the
/// CSV audit (`CraftingTagAuditor.java:290-294`). [`CompostEntry`] keeps it anyway,
/// because the port's own barrel needs it (`crate::crafting::compost::Composting`).
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct CompostEntry {
    /// The item, one per entry: `individualize` narrows the ingredient to it.
    pub item: String,
    /// `getStrength()`: 2, 4 or 8 in the three shipped files.
    pub strength: u32,
}

impl CompostEntry {
    /// What the card shows as output: `minecolonies:compost` × 6, for every input
    /// (`api/crafting/CompostRecipe.java:31`, `:44`;
    /// `CompostRecipeCategory.java:95-97`).
    pub fn output() -> (&'static str, u32) {
        ("minecolonies:compost", 6)
    }
}

/// One fishing card: a level and the drops it can give,
/// `FishermanRecipeCategory.FishingRecipe` (`:215-245`).
///
/// The drops are the **common** table plus the level's bonus table
/// (`:203-210`), merged above [`CONSOLIDATE_ABOVE`] (`:226`).
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct FishingRecipe {
    /// The bonus table's id, which is what the card is identified by
    /// (`:210`, `:230-233`).
    pub id: String,
    /// `ModLootTables.FISHERMAN_BONUS`'s key: the hut level the card is for.
    pub level: i32,
    pub drops: Vec<LootDrop>,
}

/// One florist card: a level and its plantables, `FloristRecipeCategory.FloristRecipe`
/// (`:407-409`).
///
/// `flowers` is already grouped into display slots by [`compactify`], so its
/// length is at most [`MAX_LOOT_SLOTS`] and each entry is one slot's alternatives.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct FloristRecipe {
    pub level: i32,
    pub flowers: Vec<Vec<String>>,
}

/// One card of a job's own category: the shape of `IGenericRecipe`
/// (`api/crafting/IGenericRecipe.java`), as the category reads it
/// (`GenericRecipeCategory.java:129-271`).
///
/// **This is the record that matters**: every crafting hut publishes the same
/// fields, and the twelve `GenericRecipeCategory` per job differ only in which
/// module's recipes they collect (`:348-366`).
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct JobRecipe {
    /// `getRecipeId()`: the underlying recipe's id, `None` for a synthetic one. It
    /// is shown as a tooltip line only when the player asks for advanced tooltips
    /// (`JobBasedRecipeCategory.java:380-385`).
    pub id: Option<String>,
    /// `getAllMultiOutputs()`: the primary output first, then the alternates.
    pub outputs: Vec<String>,
    /// `getAdditionalOutputs()`: the extra stacks drawn beside the primary one
    /// (`GenericRecipeCategory.java:148-154`).
    pub additional: Vec<String>,
    /// `getInputs()`: one entry per input slot, each a list of alternatives.
    pub inputs: Vec<Vec<String>>,
    /// The card's chance slots, already resolved from the loot table when it could
    /// be read (`getLootTable()` + `CustomRecipeManager.getLootDrops`, `:156-167`).
    pub loot: Option<Vec<LootDrop>>,
    /// `getIntermediate()`: a block drawn above the output, e.g. the furnace a
    /// smelting recipe needs (`:286-290`). `None` is Java's `Blocks.AIR`.
    pub intermediate: Option<&'static str>,
    /// `getRequiredTool()`: a tool type the hut demands, drawn in the top-right
    /// slot (`:135`, `JobBasedRecipeCategory.java:186-204`).
    pub tool: Option<&'static str>,
    /// `getRequiredEntity()`: the animal the recipe is about (`:292-315`).
    pub entity: Option<&'static str>,
    /// `getRestrictions()`: translation keys printed down the right edge
    /// (`:109-112`, `JobBasedRecipeCategory.java:264-290`).
    pub restrictions: Vec<&'static str>,
    /// `getLevelSort()`: the building level the card sorts under (`:362`).
    pub level_sort: i32,
}

impl JobRecipe {
    /// A card with a loot table and no primary output is drawn as a loot list
    /// rather than a recipe: `getLootTable() != null && getPrimaryOutput().isEmpty()`
    /// (`GenericRecipeCategory.java:336-339`).
    pub fn is_loot_based(&self) -> bool {
        is_loot_based(self.loot.is_some(), self.outputs.is_empty())
    }

    /// The order the category sorts its cards in: `getLevelSort`, then the primary
    /// output's item id (`:362-365`).
    ///
    /// A card with no output sorts under **`minecraft:air`**, not under the empty
    /// string: `getPrimaryOutput()` answers `ItemStack.EMPTY`
    /// (`GenericRecipe.java:394-398`) and `ForgeRegistries.ITEMS.getKey(Items.AIR)`
    /// is `minecraft:air`, so a loot-based card lands between `minecraft:air` and
    /// the first real output instead of before every id.
    pub fn sort_key(&self) -> (i32, &str) {
        (
            self.level_sort,
            self.outputs
                .first()
                .map_or("minecraft:air", String::as_str),
        )
    }
}

/// The loot-based test on its own, for a caller that has the two facts and not a
/// whole [`JobRecipe`] (`GenericRecipeCategory.java:336-339`).
pub fn is_loot_based(has_loot_table: bool, primary_output_empty: bool) -> bool {
    has_loot_table && primary_output_empty
}

/// Whether a drop list is merged before being shown: strictly more than
/// [`CONSOLIDATE_ABOVE`] drops (`GenericRecipeCategory.java:344`,
/// `FishermanRecipeCategory.java:114` — both `size() > 18`).
pub fn consolidate_needed(count: usize) -> bool {
    count > CONSOLIDATE_ABOVE.value
}

/// The key whose text carries a chance (`JobBasedRecipeCategory.java:404-405`):
/// a negative quality means skills make it worse, a positive one better.
pub fn chance_key(quality: f32) -> &'static str {
    if quality < 0.0 {
        "com.minecolonies.coremod.jei.chancenegskill.tip"
    } else if quality > 0.0 {
        "com.minecolonies.coremod.jei.chanceskill.tip"
    } else {
        "com.minecolonies.coremod.jei.chance.tip"
    }
}

/// The number the chance key is given, [`JobBasedRecipeCategory.java:404-417`]:
/// a whole percent from 1 % up, hundredths of a percent below.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Chance {
    /// `Math.round(probability * 100)`: a whole percent.
    Whole(i64),
    /// `Math.round(probability * 10000) / 100`: a fraction of a percent, so a
    /// 0.5 % drop reads as `0.5`.
    Fraction(f32),
}

/// `LootTableTooltipCallback`'s arithmetic, and nothing more
/// (`JobBasedRecipeCategory.java:406-417`).
///
/// `probability` is the drop's 0..1 chance. Java then asks the language file to
/// print it, so the port stops at the number: formatting a float is the game's.
pub fn chance_label(probability: f32) -> Chance {
    let percent = probability * 100.0;
    if percent >= 1.0 {
        Chance::Whole(percent.round() as i64)
    } else {
        Chance::Fraction((percent * 100.0).round() / 100.0)
    }
}

/// A rectangle of the recipe panel one grid is laid out in, in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Frame {
    pub left: i32,
    pub width: i32,
    /// The width the grid is centred in, which is not always `width`: the crop
    /// category centres its drops in the whole panel, not in the loot strip
    /// (`CropRecipeCategory.java:121`).
    pub centre_in: i32,
    pub top: i32,
    pub height: i32,
}

/// The loot strip of a job card, `LOOT_SLOTS_X` and `LOOT_SLOTS_W`
/// (`GenericRecipeCategory.java:84-85`), which the fisherman
/// (`FishermanRecipeCategory.java:39-40`) and the florist
/// (`FloristRecipeCategory.java:42-43`) redeclare identically.
pub const GENERIC_LOOT: Frame = Frame {
    left: LOOT_SLOTS_X,
    width: LOOT_SLOTS_W,
    centre_in: LOOT_SLOTS_W,
    top: CITIZEN_Y,
    height: CITIZEN_H.value,
};

/// The input strip of a job card, `INPUT_SLOT_X` and `INPUT_SLOT_W`
/// (`GenericRecipeCategory.java:86-87`): four columns.
pub const GENERIC_INPUT: Frame = Frame {
    left: INPUT_SLOT_X,
    width: INPUT_SLOT_W,
    centre_in: INPUT_SLOT_W,
    top: CITIZEN_Y,
    height: CITIZEN_H.value,
};

/// The crop category's drop strip, centred in the whole 150-pixel panel
/// (`CropRecipeCategory.java:118-123`).
pub const CROP_OUTPUT: Frame = Frame {
    left: 0,
    width: CROP_WIDTH - 36,
    centre_in: CROP_WIDTH,
    top: 0,
    height: CROP_HEIGHT,
};

/// The slot geometry the six categories compute four times over, in three
/// slightly different ways:
///
/// | Where | Rows | Columns | Anchored | Source |
/// | --- | --- | --- | --- | --- |
/// | job inputs | `ceil(n/4)` | `ceil(n/rows)` | top-left, centred vertically | `GenericRecipeCategory.java:172-177` |
/// | job loot | `ceil(n/6)` | `ceil(n/rows)` | centred, bottom | `:238-243` |
/// | crop drops | `ceil(n/6)` | `ceil(n/rows)` | centred in the panel, bottom | `CropRecipeCategory.java:118-123` |
/// | florist flowers | `ceil(n/6)` | `ceil(n/rows)` | centred, bottom | `FloristRecipeCategory.java:65-68` |
///
/// The `columns` recomputation is the interesting half: rows come from the
/// *maximum* columns the strip holds, then columns are re-derived so the last row
/// is as full as the others. Five drops in a six-wide strip are 2 rows of 3, not
/// a row of 5 and a row of 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Grid {
    pub rows: i32,
    pub columns: i32,
    /// The first slot's top-left corner, in GUI pixels.
    pub start_x: i32,
    pub start_y: i32,
}

impl Grid {
    /// A grid anchored to the bottom of its frame and centred in
    /// `frame.centre_in` — the three loot strips.
    ///
    /// `None` for an empty list or a frame narrower than one slot: Java guards the
    /// empty case with an `isEmpty()` test around the whole layout
    /// (`GenericRecipeCategory.java:236`, `CropRecipeCategory.java:126`) and would
    /// divide by zero without it, since `rows` is 0 and `columns` divides by it
    /// (`:239-240`).
    pub fn loot(count: usize, frame: Frame, slot: i32) -> Option<Grid> {
        let (rows, columns) = rows_and_columns(count, frame.width, slot)?;
        Some(Grid {
            rows,
            columns,
            start_x: frame.left + (frame.centre_in - columns * slot) / 2,
            start_y: frame.top + frame.height - rows * slot + 1,
        })
    }

    /// A grid anchored to the left of its frame and centred vertically in it —
    /// the input strip only (`GenericRecipeCategory.java:172-177`).
    pub fn inputs(count: usize, frame: Frame, slot: i32) -> Option<Grid> {
        let (rows, columns) = rows_and_columns(count, frame.width, slot)?;
        Some(Grid {
            rows,
            columns,
            start_x: frame.left,
            start_y: frame.top + (frame.height - rows * slot) / 2,
        })
    }

    /// Where the i-th slot goes, counted left to right then top to bottom
    /// (`:180-193`).
    pub fn slot_at(&self, index: usize, slot: i32) -> (i32, i32) {
        let index = index as i32;
        let column = if self.columns == 0 { 0 } else { index % self.columns };
        let row = if self.columns == 0 { 0 } else { index / self.columns };
        (self.start_x + column * slot, self.start_y + row * slot)
    }
}

/// The shared half of the three layouts: how many rows a strip of `width` needs
/// for `count` slots, and how wide each row ends up
/// (`GenericRecipeCategory.java:172-174`).
fn rows_and_columns(count: usize, width: i32, slot: i32) -> Option<(i32, i32)> {
    if count == 0 || width < slot || slot <= 0 {
        return None;
    }
    let initial_columns = width / slot;
    let count = count as i32;
    // `(inputs.size() + initialInputColumns - 1) / initialInputColumns`, then
    // `(inputs.size() + inputRows - 1) / inputRows` (`GenericRecipeCategory.java:173-174`,
    // `:239-240`, `CropRecipeCategory.java:119-120`): integer division, positive
    // operands, exactly as Java writes it. Not `div_ceil`, which is unstable on a
    // signed integer.
    let rows = (count + initial_columns - 1) / initial_columns;
    let columns = (count + rows - 1) / rows;
    Some((rows, columns))
}

/// Group flowers into at most [`MAX_LOOT_SLOTS`] slots,
/// `FloristRecipeCategory.compactify` (`:119-151`).
///
/// Below 24 items each gets its own slot. At or above 24 the items are dealt out
/// evenly — `size / 24` each, and the first `size % 24` slots take one more — so
/// 25 flowers are 1 slot of 2 and 23 of 1, and 49 are one of 3 and 23 of 2. The
/// test is `<`, not `<=` (`:370`), so exactly 24 takes the deal-out branch and
/// lands on 24 slots of 1 anyway.
pub fn compactify(flowers: &[String]) -> Vec<Vec<String>> {
    let max = MAX_LOOT_SLOTS.value;
    let mut slots: Vec<Vec<String>> = Vec::new();
    if flowers.len() < max {
        for flower in flowers {
            slots.push(vec![flower.clone()]);
        }
        return slots;
    }
    let per_slot = flowers.len() / max;
    let mut extra = flowers.len() % max;
    let mut next = 0;
    for _ in 0..max {
        let mut group = Vec::with_capacity(per_slot + 1);
        for _ in 0..per_slot {
            group.push(flowers[next].clone());
            next += 1;
        }
        if extra > 0 {
            group.push(flowers[next].clone());
            next += 1;
            extra -= 1;
        }
        slots.push(group);
    }
    slots
}

/// Merge drops that share a chance, the way `LootTableAnalyzer.consolidate` does
/// (`:288-296`): group by `(probability, quality, conditional)` — Java groups by
/// `hashCode`, which is `Objects.hash` of exactly those three (`:478-483`) — keep
/// the first drop's numbers, concatenate the stacks, then sort by probability
/// descending.
///
/// **Borrowed, and said out loud.** `LootTableAnalyzer` is not one of the 33 files
/// this module measures; the two categories that call it are
/// (`GenericRecipeCategory.java:344`, `FishermanRecipeCategory.java:114`), so the
/// merge lives here until the analyzer is ported and it moves there.
///
/// Java's order for two drops of the *same* probability is unspecified —
/// `Collectors.groupingBy` yields a `HashMap` and the following `sorted` is stable
/// — so this port keeps first-appearance order and a test pins it. That is a
/// determinism the Java does not promise, not a behaviour change.
pub fn consolidate(drops: &[LootDrop]) -> Vec<LootDrop> {
    let mut merged: Vec<LootDrop> = Vec::new();
    for drop in drops {
        // `position` rather than `iter_mut().find(…)`: the index lets the merge
        // and the push be two statements, so no borrow of `merged` outlives the
        // search (NLL rejects the `Option<&mut _>` form once the `None` arm needs
        // `merged` again).
        let found = merged.iter().position(|kept| {
            kept.probability.to_bits() == drop.probability.to_bits()
                && kept.quality.to_bits() == drop.quality.to_bits()
                && kept.conditional == drop.conditional
        });
        match found {
            Some(index) => merged[index].stacks.extend(drop.stacks.iter().cloned()),
            None => merged.push(drop.clone()),
        }
    }
    // `Comparator.comparing(LootDrop::getProbability).reversed()` (`:294`), stable
    // in Rust as in Java.
    merged.sort_by(|a, b| {
        b.probability
            .partial_cmp(&a.probability)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::ItemId;
    use crate::inventory::ItemName;

    fn drop_of(probability: f32, item: &str, count: u32) -> LootDrop {
        let mut drop = LootDrop::new(probability, 0.0, false);
        drop.stacks
            .push((ItemId::of(ItemName::new(item).expect("a valid id")), count));
        drop
    }

    // ---- the six categories -------------------------------------------------

    #[test]
    fn the_five_published_ids_are_the_java_ones() {
        let ids: Vec<&str> = [
            Category::Crops,
            Category::Composting,
            Category::Fishing,
            Category::Flowers,
            Category::Tools,
        ]
        .iter()
        .map(|category| category.id())
        .collect();
        assert_eq!(
            ids,
            [
                "minecolonies:crops",
                "minecolonies:composting",
                "minecolonies:fishing",
                "minecolonies:flowers",
                "minecolonies:tools",
            ]
        );
        // Five fixed ids (`JEIPlugin.java:96-100`) plus the per-job family the
        // loop at `:148-151` registers: six category *families* in all.
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn a_jobs_category_is_named_after_the_job() {
        // `RecipeType.create(uid.getNamespace(), uid.getPath(), …)` rebuilds
        // `minecolonies:baker` (`GenericRecipeCategory.java:101-105`).
        assert_eq!(job_category_id("minecolonies:baker"), "minecolonies:baker");
        assert_eq!(job_category_id("minecolonies:composter"), "minecolonies:composter");
    }

    #[test]
    fn a_jobs_translation_key_is_built_from_its_id() {
        // `"com." + key.getNamespace() + ".job." + key.getPath()`
        // (`JobEntry.java:129`), the key `JobBasedRecipeCategory.java:154` titles
        // the category with.
        assert_eq!(
            job_translation_key("minecolonies:fisherman").as_deref(),
            Some("com.minecolonies.job.fisherman")
        );
        assert_eq!(
            job_translation_key("minecolonies:florist").as_deref(),
            Some("com.minecolonies.job.florist")
        );
        // A job id without a namespace is not an id; Java's key is a
        // `ResourceLocation` and always has one.
        assert_eq!(job_translation_key("fisherman"), None);
        assert_eq!(job_translation_key("minecolonies:"), None);
        assert_eq!(job_translation_key(":florist"), None);
    }

    #[test]
    fn two_categories_are_named_after_their_job_and_one_after_a_block() {
        assert_eq!(Category::Fishing.title(), Title::Job("minecolonies:fisherman"));
        assert_eq!(Category::Flowers.title(), Title::Job("minecolonies:florist"));
        assert_eq!(
            Category::Composting.title(),
            Title::Block("minecolonies:barrel_block")
        );
    }

    #[test]
    fn the_two_shipped_title_keys_start_with_the_jei_prefix() {
        for category in [Category::Crops, Category::Tools] {
            match category.title() {
                Title::Key(key) => assert!(
                    key.starts_with(PARTIAL_JEI_INFO.value),
                    "{key} is not under {}",
                    PARTIAL_JEI_INFO.value
                ),
                other => panic!("{category:?} is titled {other:?}"),
            }
        }
    }

    #[test]
    fn the_catalysts_are_the_four_blocks_jei_registers() {
        // `JEIPlugin.java:203-206`, in that order.
        let mut all: Vec<&str> = Vec::new();
        for category in [Category::Crops, Category::Composting, Category::Fishing, Category::Flowers, Category::Tools] {
            all.extend_from_slice(category.catalysts());
        }
        assert_eq!(
            all,
            [
                "minecolonies:barrel_block",
                "minecolonies:blockhutcomposter",
                "minecolonies:blockhutfisherman",
                "minecolonies:blockhutflorist",
            ]
        );
        // Crops and tools answer to no block: a hoe is a slot, an icon is not a
        // catalyst (`CropRecipeCategory.java:104-111`, `ToolRecipeCategory.java:47`).
        assert!(Category::Crops.catalysts().is_empty());
        assert!(Category::Tools.catalysts().is_empty());
    }

    #[test]
    fn only_the_composter_hut_carries_an_ingredient_info() {
        assert_eq!(
            Category::Composting.ingredient_info(),
            Some((
                "minecolonies:blockhutcomposter",
                "com.minecolonies.coremod.jei.composter"
            ))
        );
        assert_eq!(Category::Crops.ingredient_info(), None);
    }

    #[test]
    fn only_the_two_job_based_categories_have_a_description() {
        assert_eq!(
            Category::Fishing.description(),
            Some("com.minecolonies.coremod.jei.fisherman")
        );
        assert_eq!(
            Category::Flowers.description(),
            Some("com.minecolonies.coremod.jei.florist")
        );
        assert_eq!(Category::Tools.description(), None);
    }

    // ---- tools --------------------------------------------------------------

    #[test]
    fn a_tool_has_six_levels_on_each_side() {
        let usage = ToolUsage::new("minecolonies:hoe");
        assert_eq!(usage.levels.len(), 6);
        assert_eq!(usage.enchanted.len(), 6);
        assert_eq!(ToolUsage::slot_count(), 12);
        // `"L0"` … `"L5"`, then the enchanted row (`ToolRecipeCategory.java:99`, `:104`).
        assert_eq!(ToolUsage::slot_name(0, false), "L0");
        assert_eq!(ToolUsage::slot_name(5, false), "L5");
        assert_eq!(ToolUsage::slot_name(0, true), "L0e");
        assert_eq!(ToolUsage::slot_name(5, true), "L5e");
    }

    #[test]
    fn a_tools_level_is_mining_plus_the_enchant_bonus_capped_at_five() {
        // `ToolsAnalyzer.java:88-92` with `ItemStackUtils.getMaxEnchantmentLevel`
        // (`ItemStackUtils.java:279-299`).
        assert_eq!(ToolUsage::level_of(0, 0), Some(0));
        assert_eq!(ToolUsage::level_of(2, 0), Some(2));
        // A wooden pickaxe (mining 1) with Unbreaking IV: the bonus is 4 - 1.
        assert_eq!(ToolUsage::level_of(1, ToolUsage::enchant_bonus(4)), Some(4));
        assert_eq!(ToolUsage::level_of(4, ToolUsage::enchant_bonus(3)), Some(5), "capped, not 6");
        // A stack the tool refuses: `getMiningLevel` answered -1.
        assert_eq!(ToolUsage::level_of(-1, 0), None);
    }

    #[test]
    fn the_enchant_bonus_is_the_highest_level_minus_one() {
        // `return Math.max(maxLevel - 1, 0)` (`ItemStackUtils.java:299`): a tool
        // carrying only Unbreaking I adds nothing to its building level.
        assert_eq!(ToolUsage::enchant_bonus(0), 0);
        assert_eq!(ToolUsage::enchant_bonus(1), 0);
        assert_eq!(ToolUsage::enchant_bonus(3), 2);
        assert_eq!(ToolUsage::enchant_bonus(5), 4);
    }

    #[test]
    fn an_enchanted_stack_goes_to_the_enchanted_row() {
        let mut usage = ToolUsage::new("minecolonies:hoe");
        assert_eq!(usage.add("minecraft:wooden_hoe", 0, 0, false), Some(0));
        assert_eq!(usage.add("minecraft:wooden_hoe{Enchantments:[…]}", 0, 2, true), Some(2));
        assert_eq!(usage.levels[0], ["minecraft:wooden_hoe"]);
        assert!(usage.levels[2].is_empty(), "the enchanted stack is not here");
        assert_eq!(usage.enchanted[2].len(), 1);
        assert_eq!(usage.add("minecraft:stick", -1, 0, false), None, "not a hoe");
    }

    #[test]
    fn a_stack_enchanted_at_level_one_is_enchanted_without_a_bonus() {
        // `stack.isEnchanted()` and `getMaxEnchantmentLevel` are two different
        // answers (`ToolsAnalyzer.java:88-92` against `:96`): a tool with only
        // Unbreaking I has a bonus of 0 and still belongs to the enchanted row.
        let mut usage = ToolUsage::new("minecolonies:hoe");
        let bonus = ToolUsage::enchant_bonus(1);
        assert_eq!(bonus, 0);
        assert_eq!(usage.add("minecraft:wooden_hoe{Unbreaking:I}", 1, bonus, true), Some(1));
        assert!(usage.levels[1].is_empty(), "not the plain row");
        assert_eq!(usage.enchanted[1].len(), 1);
    }

    #[test]
    fn the_enchantment_trial_runs_five_times() {
        // `for (int enchantLevel = 1; enchantLevel < 6; ++enchantLevel)`
        // (`ToolsAnalyzer.java:41`).
        assert_eq!(enchant_levels().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(ENCHANTMENTS.value.len(), 7, "`ToolsAnalyzer.java:60-66`");
        assert_eq!(ENCHANTMENTS.value[0], "UNBREAKING");
        assert_eq!(ENCHANTMENTS.value[6], "FISHING_SPEED");
    }

    // ---- composting and crops ----------------------------------------------

    #[test]
    fn composting_always_shows_six_compost_whatever_the_strength() {
        assert_eq!(CompostEntry::output(), ("minecolonies:compost", 6));
        let rich = CompostEntry { item: "minecraft:cake".to_owned(), strength: 8 };
        let poor = CompostEntry { item: "minecraft:wheat_seeds".to_owned(), strength: 2 };
        assert_ne!(rich.strength, poor.strength);
        // The card cannot tell them apart: both show `getResultItem(null)`.
        assert_eq!(CompostEntry::output(), CompostEntry::output());
    }

    #[test]
    fn a_crop_recipe_is_its_source_block() {
        assert_eq!(
            CropRecipe { source: "minecraft:wheat" },
            CropRecipe { source: "minecraft:wheat" }
        );
        assert_ne!(
            CropRecipe { source: "minecraft:wheat" },
            CropRecipe { source: "minecraft:carrots" }
        );
    }

    // ---- the job card ------------------------------------------------------

    #[test]
    fn a_card_with_a_loot_table_and_no_output_is_drawn_as_loot() {
        let empty_outputs = JobRecipe::default();
        let with_loot = JobRecipe {
            loot: Some(Vec::new()),
            ..Default::default()
        };
        assert!(with_loot.is_loot_based());
        assert!(!empty_outputs.is_loot_based(), "no loot table");
        let both = JobRecipe {
            outputs: vec!["minecraft:bread".to_owned()],
            loot: Some(Vec::new()),
            ..Default::default()
        };
        assert!(!both.is_loot_based(), "a primary output wins");
        assert!(is_loot_based(true, true));
        assert!(!is_loot_based(true, false));
        assert!(!is_loot_based(false, true));
    }

    #[test]
    fn a_cards_sort_key_is_level_then_output() {
        let low = JobRecipe { level_sort: 1, outputs: vec!["minecraft:bread".to_owned()], ..Default::default() };
        let high = JobRecipe { level_sort: 3, outputs: vec!["minecraft:air".to_owned()], ..Default::default() };
        assert!(low.sort_key() < high.sort_key(), "the level sorts first");
        let bread = JobRecipe { level_sort: 1, outputs: vec!["minecraft:bread".to_owned()], ..Default::default() };
        let apple = JobRecipe { level_sort: 1, outputs: vec!["minecraft:apple".to_owned()], ..Default::default() };
        assert!(apple.sort_key() < bread.sort_key(), "then the item id");
        // An output-less card carries `ItemStack.EMPTY`, whose item is air
        // (`GenericRecipe.java:394-398`).
        assert_eq!(JobRecipe::default().sort_key(), (0, "minecraft:air"));
        // So it sorts *after* a card whose output is `minecraft:acacia_boat`,
        // where an empty string would have put it first.
        let boat = JobRecipe { level_sort: 0, outputs: vec!["minecraft:acacia_boat".to_owned()], ..Default::default() };
        assert!(boat.sort_key() < JobRecipe::default().sort_key());
    }

    // ---- chances -----------------------------------------------------------

    #[test]
    fn a_chance_above_one_percent_is_a_whole_percent() {
        assert_eq!(chance_label(0.01), Chance::Whole(1));
        assert_eq!(chance_label(0.5), Chance::Whole(50));
        assert_eq!(chance_label(1.0), Chance::Whole(100));
        assert_eq!(chance_label(0.125), Chance::Whole(13), "rounded, not truncated");
    }

    #[test]
    fn a_chance_below_one_percent_keeps_two_decimals() {
        assert_eq!(chance_label(0.005), Chance::Fraction(0.5));
        assert_eq!(chance_label(0.0049), Chance::Fraction(0.49));
        assert_eq!(chance_label(0.0), Chance::Fraction(0.0));
    }

    #[test]
    fn the_quality_picks_the_key() {
        assert_eq!(
            chance_key(-1.0),
            "com.minecolonies.coremod.jei.chancenegskill.tip"
        );
        assert_eq!(chance_key(0.0), "com.minecolonies.coremod.jei.chance.tip");
        assert_eq!(
            chance_key(2.0),
            "com.minecolonies.coremod.jei.chanceskill.tip"
        );
        for quality in [-1.0, 0.0, 1.0] {
            assert!(chance_key(quality).starts_with(PARTIAL_JEI_INFO.value));
        }
    }

    // ---- the grids ---------------------------------------------------------

    #[test]
    fn the_output_slot_sits_under_the_citizen_and_right_of_it() {
        // `GenericRecipeCategory.java:66-67`, with an 18-pixel slot.
        assert_eq!(OUTPUT_SLOT_X, 57);
        assert_eq!(OUTPUT_SLOT_Y, 100);
        // It is inside the panel, and to the right of the citizen's box.
        assert!(OUTPUT_SLOT_X > CITIZEN_X + CITIZEN_W);
        assert!(OUTPUT_SLOT_Y + SLOT <= HEIGHT.value);
    }

    #[test]
    fn the_job_input_strip_is_four_columns_wide() {
        // `INPUT_SLOT_W = 86`, slot 18 → `initialInputColumns` 4
        // (`GenericRecipeCategory.java:172`).
        assert_eq!(GENERIC_INPUT.width / SLOT, 4);
        assert_eq!(GENERIC_INPUT.left, 81);
        let four = Grid::inputs(4, GENERIC_INPUT, SLOT).expect("four inputs");
        assert_eq!((four.rows, four.columns), (1, 4));
        assert_eq!((four.start_x, four.start_y), (81, 72));
    }

    #[test]
    fn five_inputs_are_two_rows_of_three() {
        // The `columns` recomputation (`GenericRecipeCategory.java:174`).
        let grid = Grid::inputs(5, GENERIC_INPUT, SLOT).expect("five inputs");
        assert_eq!((grid.rows, grid.columns), (2, 3));
        assert_eq!(grid.start_y, 63);
        assert_eq!(grid.slot_at(0, SLOT), (81, 63));
        assert_eq!(grid.slot_at(2, SLOT), (117, 63));
        assert_eq!(grid.slot_at(3, SLOT), (81, 81), "the fourth wraps");
    }

    #[test]
    fn the_loot_strip_is_six_columns_and_anchored_to_the_bottom() {
        // `LOOT_SLOTS_W = 114`, slot 18 → six columns (`:238`).
        assert_eq!(GENERIC_LOOT.width / SLOT, 6);
        let one = Grid::loot(1, GENERIC_LOOT, SLOT).expect("one drop");
        assert_eq!((one.rows, one.columns), (1, 1));
        assert_eq!((one.start_x, one.start_y), (101, 100));
        let eighteen = Grid::loot(18, GENERIC_LOOT, SLOT).expect("eighteen drops");
        assert_eq!((eighteen.rows, eighteen.columns), (3, 6));
        assert_eq!((eighteen.start_x, eighteen.start_y), (56, 64));
    }

    #[test]
    fn the_crop_strip_is_centred_in_the_whole_panel() {
        // `CropRecipeCategory.java:121` centres in `WIDTH`, not in the 114-pixel
        // strip: `(150 - 3 * 18) / 2 = 48`.
        let grid = Grid::loot(3, CROP_OUTPUT, SLOT).expect("three drops");
        assert_eq!((grid.rows, grid.columns), (1, 3));
        assert_eq!(grid.start_x, 48);
        // Bottom-anchored in a 54-pixel panel: `54 - 18 + 1`.
        assert_eq!(grid.start_y, 37);
    }

    #[test]
    fn an_empty_or_impossible_grid_is_refused_not_divided_by_zero() {
        assert_eq!(Grid::loot(0, GENERIC_LOOT, SLOT), None);
        assert_eq!(Grid::inputs(0, GENERIC_INPUT, SLOT), None);
        // A strip narrower than a slot would make `initial_columns` zero, which is
        // the division `:239-240` performs.
        let narrow = Frame { left: 0, width: 17, centre_in: 17, top: 0, height: 20 };
        assert_eq!(Grid::loot(1, narrow, SLOT), None);
    }

    // ---- the florist's compaction ------------------------------------------

    fn flowers(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("minecraft:flower_{index}")).collect()
    }

    #[test]
    fn below_twenty_four_flowers_each_gets_its_own_slot() {
        assert_eq!(compactify(&[]).len(), 0);
        assert_eq!(compactify(&flowers(5)).len(), 5);
        let twenty_three = compactify(&flowers(23));
        assert_eq!(twenty_three.len(), 23);
        assert!(twenty_three.iter().all(|slot| slot.len() == 1));
    }

    #[test]
    fn exactly_twenty_four_is_twenty_four_slots_even_though_it_deals_out() {
        // The test is `< MAX` (`FloristRecipeCategory.java:124`), so 24 takes the
        // other branch: 24/24 = 1 each, 0 extra.
        let slots = compactify(&flowers(24));
        assert_eq!(slots.len(), 24);
        assert!(slots.iter().all(|slot| slot.len() == 1));
    }

    #[test]
    fn above_twenty_four_the_first_slots_take_the_remainder() {
        let twenty_five = compactify(&flowers(25));
        assert_eq!(twenty_five.len(), 24);
        assert_eq!(twenty_five[0].len(), 2, "the first slot takes the extra");
        assert_eq!(twenty_five[23].len(), 1);
        let forty_nine = compactify(&flowers(49));
        assert_eq!(forty_nine.len(), 24);
        assert_eq!(forty_nine[0].len(), 3);
        assert_eq!(forty_nine[23].len(), 2);
        // Nothing is lost and nothing repeats, at any size.
        for count in [25, 47, 48, 49, 100, 241] {
            let slots = compactify(&flowers(count));
            let total: usize = slots.iter().map(Vec::len).sum();
            assert_eq!(total, count, "{count} flowers");
            assert!(slots.len() <= MAX_LOOT_SLOTS.value, "{count} fills the panel");
        }
    }

    // ---- the drop merge ----------------------------------------------------

    #[test]
    fn the_threshold_is_strictly_above_eighteen() {
        assert!(!consolidate_needed(17));
        assert!(!consolidate_needed(18), "18 drops are shown as they are");
        assert!(consolidate_needed(19));
    }

    #[test]
    fn drops_with_the_same_numbers_merge_their_stacks() {
        // `Objects.hash(probability, quality, conditional)` (`:478-483`).
        let drops = [
            drop_of(0.5, "minecraft:cod", 1),
            drop_of(0.25, "minecraft:salmon", 1),
            drop_of(0.5, "minecraft:tropical_fish", 1),
        ];
        let merged = consolidate(&drops);
        assert_eq!(merged.len(), 2, "the two 50 % drops became one");
        assert_eq!(merged[0].probability, 0.5, "highest first");
        assert_eq!(merged[0].stacks.len(), 2, "both stacks kept");
        assert_eq!(merged[1].probability, 0.25);
    }

    #[test]
    fn a_quality_or_a_condition_keeps_two_drops_apart() {
        let mut skilled = LootDrop::new(0.5, 1.0, false);
        skilled.stacks.push((ItemId::of(ItemName::new("minecraft:cod").unwrap()), 1));
        let mut plain = LootDrop::new(0.5, 0.0, false);
        plain.stacks.push((ItemId::of(ItemName::new("minecraft:cod").unwrap()), 1));
        let merged = consolidate(&[skilled, plain]);
        assert_eq!(merged.len(), 2, "same chance, different skill effect");
    }

    #[test]
    fn equal_probabilities_keep_their_first_appearance_order() {
        // The grouping key is `Objects.hash(probability, quality, conditional)`
        // (`LootTableAnalyzer.java:502-505`) — the **stacks are not in it**, which
        // is why two drops that differ only by item merge (the test above). Two
        // drops of the same chance stay apart only when the skill effect or the
        // condition differs, so that is what this pair varies.
        let mut first = LootDrop::new(0.1, 0.0, false);
        first
            .stacks
            .push((ItemId::of(ItemName::new("minecraft:first").expect("a valid id")), 1));
        let mut second = LootDrop::new(0.1, 1.0, false);
        second.stacks.push((
            ItemId::of(ItemName::new("minecraft:second").expect("a valid id")),
            1,
        ));

        let merged = consolidate(&[first, second]);
        assert_eq!(merged.len(), 2, "a different skill effect is a different drop");
        // `Comparator.comparing(LootDrop::getProbability).reversed()` (`:294`) is a
        // stable sort, so the equal pair keeps the order it was given. Java's
        // `groupingBy` is a `HashMap`, so *its* order here is unspecified; the port
        // pins Rust's stability.
        assert_eq!(merged[0].stacks[0].0.name().as_str(), "minecraft:first");
        assert_eq!(merged[1].stacks[0].0.name().as_str(), "minecraft:second");
        assert_eq!(merged[0].quality, 0.0);
        assert_eq!(merged[1].quality, 1.0);
    }
}
