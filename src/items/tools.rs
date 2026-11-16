//! The **17 tools** of the mod: what wears out, how far they reach, what they hit
//! for, and what one use is worth.
//!
//! Java reference: `core/items/ItemAssistantHammer.java` (429 lines),
//! `ItemSpear.java` (117), `ItemChiefSword.java` (56), `ItemIronScimitar.java`
//! (21), `ItemPharaoScepter.java` (148), `ItemScepterGuard.java` (132),
//! `ItemScepterPermission.java` (269), `ItemScepterLumberjack.java` (204),
//! `ItemScepterBeekeeper.java` (135), `ItemScanAnalyzer.java` (212),
//! `ItemBannerRallyGuards.java` (554), `ItemSifterMesh.java` (21), and the armour
//! of `MineColoniesArmorMaterial.java` (99) with its five materials
//! (`ItemPlateArmor`, `ItemPirateGear`, `ItemSantaHead`, `ItemBuildGoggles`).
//!
//! **The three numbers that matter, and where each one comes from.**
//!
//! | What | Value | Java |
//! | --- | --- | --- |
//! | hammer reach | 1, 2, 3 blocks | `ModItemsInitializer.java:66-68`, third argument |
//! | hammer cooldown | 5 ticks | `ItemAssistantHammer.java:236` |
//! | hammer wear | 1 per block placed | `ItemAssistantHammer.java:189` |
//!
//! **A hammer places one block per use, never a batch.** The loop skips any
//! position whose required items are not **exactly one** stack
//! (`ItemAssistantHammer.java:286`), then places one and returns
//! (`:404`): the reach is how far it *looks*, not how much it does.
//!
//! **Swords have no durability here, and that is deliberate.** `ItemChiefSword`
//! is `SwordItem(Tiers.DIAMOND, 3, -2.4f)` (`:33`) and `ItemIronScimitar` is
//! `SwordItem(Tiers.IRON, 3, -2.4f)` (`:19`): the durability is the **tier's**
//! table, which this tree does not carry. Writing 1561 and 250 would be vanilla
//! numbers with a MineColonies line above them — see `MANQUANT.md` §3.
//!
//! **Armour is not a tool, and it is here anyway.** `crate::jobs::tool` states why
//! armour is not a `ToolKind` (`core/entity/ai/workers/guard/AbstractEntityAIFight.java:150-160`):
//! a guard asks its building for it, it is not a job's own tool. The numbers a
//! guard's gear check needs are still armour numbers, so they live beside the
//! sword rather than in the jobs module.
//!
//! **What is not here.** No swing: `attack_damage` and `attack_speed` are the
//! attributes an `Item.Properties` or a data component sets, and the hook that
//! runs an attack is not in `src/lib.rs` (`MANQUANT.md` §2). No enchantment: an
//! enchantment is data, and `Host::enchantments_compatible` is the only
//! enchantment call the SDK has (`crates/horizon-mod-vocab/src/host/items.rs:43-45`).

use super::Refusal;

// ---------------------------------------------------------------------------
// The constants, each with its line.
// ---------------------------------------------------------------------------

/// `ItemAssistantHammer`: the cooldown one use puts on the item, in ticks
/// (`ItemAssistantHammer.java:236`), a quarter of a second — long enough that a
/// held right click places five blocks a second, not twenty.
pub const HAMMER_COOLDOWN: i32 = 5;

/// `ItemAssistantHammer`: the wear of **one placed block**
/// (`ItemAssistantHammer.java:189`), which is why a diamond hammer is worth
/// exactly five times a gold one.
pub const HAMMER_WEAR: i32 = 1;

/// `ItemAssistantHammer`: one use in twenty also cheers the builder up, a
/// notification the code rolls at `ColonyConstants.rand.nextInt(20) == 0`
/// (`ItemAssistantHammer.java:359`).
pub const HAMMER_ENCOURAGE_ONE_IN: i32 = 20;

/// `ItemSpear.SPEAR_BASE_DAMAGE`: the base damage of a thrown or swung spear
/// (`ItemSpear.java:28`), read by `getDamage` (`:115`).
pub const SPEAR_BASE_DAMAGE: i32 = 3;

/// `ItemSpear`: the charge a throw needs, in ticks — `usedForDuration >= 10`
/// (`ItemSpear.java:55`).
pub const SPEAR_THROW_TICKS: i32 = 10;

/// `ItemSpear`: the durability, `properties.durability(250)` (`ItemSpear.java:32`).
pub const SPEAR_DURABILITY: i32 = 250;

/// `ItemChiefSword`: the bonus damage the constructor states
/// (`ItemChiefSword.java:33`), on top of the diamond tier's own.
pub const CHIEF_SWORD_DAMAGE_BONUS: i32 = 3;

/// `ItemChiefSword`: the attack speed modifier, `-2.4f` (`ItemChiefSword.java:33`).
pub const CHIEF_SWORD_ATTACK_SPEED: f64 = -2.4;

/// `ItemChiefSword.LEVITATION_EFFECT_DURATION`: ten seconds
/// (`ItemChiefSword.java:23`).
pub const LEVITATION_TICKS: i32 = 200;

/// `ItemChiefSword.LEVITATION_EFFECT_MULTIPLIER`: level **III**
/// (`ItemChiefSword.java:24`) — the amplifier, so `2`.
pub const LEVITATION_AMPLIFIER: i32 = 2;

/// `Constants.GLOW_EFFECT_DURATION`: thirty seconds
/// (`api/util/constant/Constants.java:168`).
pub const GLOW_TICKS: i32 = 600;

/// `Constants.GLOW_EFFECT_DISTANCE`: sixty blocks
/// (`api/util/constant/Constants.java:169`).
pub const GLOW_DISTANCE: i32 = 60;

/// `ItemIronScimitar`: the bonus damage of the iron tier's sword
/// (`ItemIronScimitar.java:19`).
pub const SCIMITAR_DAMAGE_BONUS: i32 = 3;

/// `ItemIronScimitar`: its attack speed modifier (`ItemIronScimitar.java:19`).
pub const SCIMITAR_ATTACK_SPEED: f64 = -2.4;

/// `ItemPharaoScepter`: the durability, `properties.durability(384)`
/// (`ItemPharaoScepter.java:36`).
pub const PHARAO_DURABILITY: i32 = 384;

/// `ItemPharaoScepter`: the arrow's velocity scale, `speed * 3.0F`
/// (`ItemPharaoScepter.java:77`) — one and a half times vanilla's bow.
pub const PHARAO_ARROW_VELOCITY: f64 = 3.0;

/// The two scepters that state a durability: `durability(2)`, and no repair
/// (`ItemScepterGuard.java:43`, `ItemScepterPermission.java:73`). Two uses, which
/// is what "place this once and it is spent" costs.
pub const SCEPTER_DURABILITY: i32 = 2;

/// `ItemScanAnalyzer.TIMEOUT_DELAY`: two minutes, after which a selection is
/// forgotten (`ItemScanAnalyzer.java:49`).
pub const ANALYZER_TIMEOUT: i32 = 20 * 60 * 2;

/// The four meshes' durabilities, in `ModItemsInitializer.java:131-134` order:
/// string, flint, iron, diamond.
pub const MESH_DURABILITIES: [i32; 4] = [500, 1000, 1500, 2000];

/// `MineColoniesArmorMaterial.HEALTH_FUNCTION_FOR_TYPE`: the durability **unit**
/// of a slot, multiplied by the material's own factor
/// (`MineColoniesArmorMaterial.java:21-26`).
///
/// This is the whole armour durability rule: Java has no per-item table, only
/// `13/15/16/11 × multiplier` (`:57-60`), in Java's enum-map order — **boots,
/// leggings, chestplate, helmet**, which is not the order a player says them in.
pub const ARMOUR_HEALTH_UNIT: [i32; 4] = [13, 15, 16, 11];

// ---------------------------------------------------------------------------
// The hammer.
// ---------------------------------------------------------------------------

/// One assistant hammer: what it reaches, how long it lasts, and where the two
/// numbers are stated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hammer {
    /// The registry path.
    pub item: &'static str,
    /// `properties.durability(n)` (`ModItemsInitializer.java:66-68`).
    pub durability: i32,
    /// The reach, i.e. the radius of the ball the hammer looks through
    /// (`ItemAssistantHammer.java:69`, third constructor argument).
    pub reach: i32,
    /// The material the name gives it.
    pub material: &'static str,
    /// `fichier.java:ligne` of the registration line carrying both numbers.
    pub source: &'static str,
}

/// The three hammers, in `ModItemsInitializer.java:66-68` order.
///
/// The two numbers grow together — 200/400/1000 durability and 1/2/3 reach — which
/// is what makes the diamond hammer three times the *area* and five times the
/// *lifetime* of the gold one.
pub const HAMMERS: &[Hammer] = &[
    Hammer {
        item: "assistanthammer_gold",
        durability: 200,
        reach: 1,
        material: "gold",
        source: "ModItemsInitializer.java:66",
    },
    Hammer {
        item: "assistanthammer_iron",
        durability: 400,
        reach: 2,
        material: "iron",
        source: "ModItemsInitializer.java:67",
    },
    Hammer {
        item: "assistanthammer_diamond",
        durability: 1000,
        reach: 3,
        material: "diamond",
        source: "ModItemsInitializer.java:68",
    },
];

/// The hammer an id names, or `None`.
pub fn hammer(item: &str) -> Option<&'static Hammer> {
    HAMMERS.iter().find(|hammer| hammer.item == item)
}

impl Hammer {
    /// How many positions one use can look at: the integer points of the closed
    /// ball of radius [`Hammer::reach`], which is `7`, `33` and `123`.
    ///
    /// Java's triple loop visits some of them more than once
    /// (`ItemAssistantHammer.java:238-259`: `currentDistance` grows, and a point at
    /// distance 1.4 is inside `currentDistance = 2`), but it returns on the first
    /// placement, so the positions it can touch are the ball itself.
    pub fn positions(&self) -> i32 {
        reach_points(self.reach)
    }

    /// How many blocks one hammer places before it breaks: `durability / wear`.
    pub fn placements(&self) -> i32 {
        self.durability / HAMMER_WEAR
    }
}

/// The integer points of a closed ball, `7`, `33`, `123`, `257` for radii 1 to 4.
///
/// A negative radius is zero positions, which no hammer has.
pub fn reach_points(reach: i32) -> i32 {
    if reach < 0 {
        return 0;
    }
    let limit = reach * reach;
    let mut count = 0;
    for x in -reach..=reach {
        for y in -reach..=reach {
            for z in -reach..=reach {
                if x * x + y * y + z * z <= limit {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Why a hammer may not assist — the three messages Java sends instead of placing
/// (`ItemAssistantHammer.java:100-140`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java refuses for exactly three reasons, and each has its own message
pub enum AssistRefusal {
    /// No colony claims this position: `view == null` (`:106`), and the permission
    /// check that follows it.
    NotAColony,
    /// Not a claimed work order of a builder: Java walks the work orders for a
    /// claimed one whose building is a builder (`:110-116`).
    NoBuilderOrder,
    /// The work order is still clearing: `stage == CLEAR || stage == CLEAR_NON_SOLIDS`
    /// refuses with "not cleared" (`:155-160`).
    StillClearing,
}

impl AssistRefusal {
    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            AssistRefusal::NotAColony => "not_a_colony",
            AssistRefusal::NoBuilderOrder => "no_builder_order",
            AssistRefusal::StillClearing => "still_clearing",
        }
    }
}

/// Whether a hammer may place a block here, in Java's order: inside a colony,
/// inside a claimed builder's work order, past the clearing stage.
///
/// The order matters: a position in no colony is "not a colony" before it is
/// "not a builder", which is what the player is told.
pub fn may_assist(
    in_colony: bool,
    claimed_builder_order: bool,
    cleared: bool,
) -> Result<(), AssistRefusal> {
    if !in_colony {
        return Err(AssistRefusal::NotAColony);
    }
    if !claimed_builder_order {
        return Err(AssistRefusal::NoBuilderOrder);
    }
    if !cleared {
        return Err(AssistRefusal::StillClearing);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The rest of the tools.
// ---------------------------------------------------------------------------

/// A tool's numbers, with `None` where Java does not state one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolStats {
    /// The registry path.
    pub item: &'static str,
    /// The durability, when Java states a number.
    pub durability: Option<i32>,
    /// The bonus attack damage over the empty hand.
    pub damage: Option<i32>,
    /// The attack speed modifier.
    pub attack_speed: Option<f64>,
    /// The tier whose own durability the weapon borrows, when it borrows one.
    pub tier: Option<&'static str>,
    /// `fichier.java:ligne` of the constructor that states the numbers.
    pub source: &'static str,
}

/// A tool with a durability and nothing else.
const fn simple(item: &'static str, durability: i32, source: &'static str) -> ToolStats {
    ToolStats { item, durability: Some(durability), damage: None, attack_speed: None, tier: None, source }
}

/// A tiered weapon: `SwordItem(Tiers.X, damage, speed)`. The durability is the
/// tier's own, so it is left out rather than guessed.
const fn weapon(
    item: &'static str,
    damage: i32,
    attack_speed: f64,
    tier: &'static str,
    source: &'static str,
) -> ToolStats {
    ToolStats {
        item,
        durability: None,
        damage: Some(damage),
        attack_speed: Some(attack_speed),
        tier: Some(tier),
        source,
    }
}

/// A tool that never wears out: `stacksTo(1)` and no durability at all.
const fn endless(item: &'static str, source: &'static str) -> ToolStats {
    ToolStats { item, durability: None, damage: None, attack_speed: None, tier: None, source }
}

/// The **13** tools that have numbers of their own — the four meshes are
/// [`MESH_DURABILITIES`] and nothing else, and they are asserted separately.
pub const TOOLS: &[ToolStats] = &[
    simple("spear", SPEAR_DURABILITY, "ItemSpear.java:32"),
    // `SwordItem(Tiers.DIAMOND, 3, -2.4f)`: damage and speed stated, durability the
    // tier's (`ItemChiefSword.java:33`).
    weapon("chiefsword", CHIEF_SWORD_DAMAGE_BONUS, CHIEF_SWORD_ATTACK_SPEED, "diamond", "ItemChiefSword.java:33"),
    // `SwordItem(Tiers.IRON, 3, -2.4f)` (`ItemIronScimitar.java:19`).
    weapon("iron_scimitar", SCIMITAR_DAMAGE_BONUS, SCIMITAR_ATTACK_SPEED, "iron", "ItemIronScimitar.java:19"),
    simple("pharaoscepter", PHARAO_DURABILITY, "ItemPharaoScepter.java:36"),
    simple("scepterpermission", SCEPTER_DURABILITY, "ItemScepterPermission.java:73"),
    simple("scepterguard", SCEPTER_DURABILITY, "ItemScepterGuard.java:43"),
    // `stacksTo(1)` and **no** durability: a marked area outlives the tool.
    endless("scepterlumberjack", "ItemScepterLumberjack.java:51"),
    endless("scepterbeekeeper", "ItemScepterBeekeeper.java:52"),
    // `stacksTo(1).durability(0)`: a zero durability is "never breaks", not "breaks
    // at once" (`ItemBannerRallyGuards.java:69`).
    simple("banner_rally_guards", 0, "ItemBannerRallyGuards.java:69"),
    // `durability(0).setNoRepair()`: the scanner is eternal and unrepairable
    // (`ItemScanAnalyzer.java:62`).
    simple("scan_analyzer", 0, "ItemScanAnalyzer.java:62"),
    simple("assistanthammer_gold", 200, "ModItemsInitializer.java:66"),
    simple("assistanthammer_iron", 400, "ModItemsInitializer.java:67"),
    simple("assistanthammer_diamond", 1000, "ModItemsInitializer.java:68"),
];

/// The stats of a tool, or `None` for anything else — including the four meshes.
pub fn stats(item: &str) -> Option<&'static ToolStats> {
    TOOLS.iter().find(|stats| stats.item == item)
}

/// The stats of a tool, or a named refusal.
pub fn require(item: &'static str) -> Result<&'static ToolStats, Refusal> {
    stats(item).ok_or(Refusal::NotATool { id: item })
}

/// Whether a spear can be thrown — the two guards of `ItemSpear`:
///
/// ```java
/// if (itemstack.getDamageValue() >= itemstack.getMaxDamage() - 1) return fail;   // :86-89
/// if (usedForDuration >= 10) { … throw … }                                      // :55
/// ```
///
/// A spear at its last point of durability is refused **before** the charge is
/// read: Java's `use` returns `fail` there, so no amount of holding helps.
pub fn spear_may_throw(charge_ticks: i32, damage: i32, max_damage: i32) -> bool {
    damage < max_damage - 1 && charge_ticks >= SPEAR_THROW_TICKS
}

/// What a hit with the chief sword does, beyond damage.
///
/// Two effects, both on the **barbarian** side of the war
/// (`ItemChiefSword.java:37-54`):
///
/// - the holder glows **barbarians** within [`GLOW_DISTANCE`] blocks, for
///   [`GLOW_TICKS`], through `RaiderMobUtils.getBarbariansCloseToEntity`
///   (`:41-43`) — an `inventoryTick`, so it costs nothing per hit;
/// - a hit on an `AbstractEntityBarbarianRaider` levitates it for
///   [`LEVITATION_TICKS`] at level [`LEVITATION_AMPLIFIER`] (`:51`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChiefSwordHit {
    /// The levitation duration in ticks.
    pub levitation_ticks: i32,
    /// The levitation amplifier.
    pub levitation_amplifier: i32,
    /// The glow duration in ticks, for the holder's passive scan.
    pub glow_ticks: i32,
    /// The glow radius in blocks.
    pub glow_distance: i32,
}

/// The two effects of [`ChiefSwordHit`], from the four constants above.
pub const CHIEF_SWORD_HIT: ChiefSwordHit = ChiefSwordHit {
    levitation_ticks: LEVITATION_TICKS,
    levitation_amplifier: LEVITATION_AMPLIFIER,
    glow_ticks: GLOW_TICKS,
    glow_distance: GLOW_DISTANCE,
};

// ---------------------------------------------------------------------------
// The armour.
// ---------------------------------------------------------------------------

/// One of the four armour slots, in the order
/// `MineColoniesArmorMaterial.HEALTH_FUNCTION_FOR_TYPE` lists them (boots first,
/// `:21-26`) — **not** the order a player says them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: `ArmorItem.Type` has exactly four values
pub enum ArmourSlot {
    /// `Type.BOOTS`, health unit 13.
    Boots,
    /// `Type.LEGGINGS`, health unit 15.
    Leggings,
    /// `Type.CHESTPLATE`, health unit 16.
    Chestplate,
    /// `Type.HELMET`, health unit 11.
    Helmet,
}

impl ArmourSlot {
    /// The four, in Java's enum-map order.
    pub const ALL: [ArmourSlot; 4] = [
        ArmourSlot::Boots,
        ArmourSlot::Leggings,
        ArmourSlot::Chestplate,
        ArmourSlot::Helmet,
    ];

    /// The index into [`ARMOUR_HEALTH_UNIT`] and a material's protection array.
    pub fn index(self) -> usize {
        match self {
            ArmourSlot::Boots => 0,
            ArmourSlot::Leggings => 1,
            ArmourSlot::Chestplate => 2,
            ArmourSlot::Helmet => 3,
        }
    }

    /// The slot's name, as `ArmorItem.Type` spells it.
    pub fn name(self) -> &'static str {
        match self {
            ArmourSlot::Boots => "boots",
            ArmourSlot::Leggings => "leggings",
            ArmourSlot::Chestplate => "chestplate",
            ArmourSlot::Helmet => "helmet",
        }
    }

    /// The durability unit of the slot, before the material's multiplier.
    pub fn health_unit(self) -> i32 {
        ARMOUR_HEALTH_UNIT[self.index()]
    }
}

/// One `MineColoniesArmorMaterial`: five of them, four used to their full extent
/// (the santa hat and the goggles are helmets only, and protect nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmourMaterial {
    /// The name Java hands the material: `minecolonies:plate_armor`.
    pub name: &'static str,
    /// The durability multiplier (`MineColoniesArmorMaterial.java:57-60`).
    pub durability_multiplier: i32,
    /// The protection of the four slots, in [`ArmourSlot`] order.
    pub protection: [i32; 4],
    /// The enchantment value.
    pub enchantment: i32,
    /// The toughness, the one number the pirate set 2 raises.
    pub toughness: i32,
    /// `fichier.java:ligne` of the constructor call.
    pub source: &'static str,
}

/// Build an armour material row.
const fn armour(
    name: &'static str,
    durability_multiplier: i32,
    protection: [i32; 4],
    enchantment: i32,
    toughness: i32,
    source: &'static str,
) -> ArmourMaterial {
    ArmourMaterial { name, durability_multiplier, protection, enchantment, toughness, source }
}

/// The five materials, in the order a reader meets them.
pub const ARMOUR: &[ArmourMaterial] = &[    // `PLATE_ARMOR`: multiplier 37, protection 3/6/8/3, enchantment 9, iron repair.
    armour("minecolonies:plate_armor", 37, [3, 6, 8, 3], 9, 0, "ItemPlateArmor.java:18"),
    // `PIRATE_ARMOR_1`: multiplier 33, protection 2/5/6/2, enchantment 5.
    armour("minecolonies:pirate", 33, [2, 5, 6, 2], 5, 0, "ItemPirateGear.java:16"),
    // `PIRATE_ARMOR_2`: multiplier 15 — the **same protection** as the plate set
    // and half its durability. That is the set's whole trade.
    armour("minecolonies:pirate2", 15, [3, 6, 8, 3], 5, 2, "ItemPirateGear.java:23"),
    // `SANTA_HAT`: multiplier 500 and **zero** protection — 5 500 durability of
    // nothing.
    armour("minecolonies:santa_hat", 500, [0, 0, 0, 0], 5, 0, "ItemSantaHead.java:18"),
    // `GOGGLES`: multiplier 2 and zero protection, but the highest enchantment
    // value of the five.
    armour("minecolonies:build_goggles", 2, [0, 0, 0, 0], 20, 0, "ItemBuildGoggles.java:19"),
];

impl ArmourMaterial {
    /// The durability of one slot: `health_unit × multiplier`
    /// (`MineColoniesArmorMaterial.java:59`).
    pub fn durability(&self, slot: ArmourSlot) -> i32 {
        slot.health_unit() * self.durability_multiplier
    }

    /// The protection of one slot (`:62-65`), in armour points.
    pub fn protection(&self, slot: ArmourSlot) -> i32 {
        self.protection[slot.index()]
    }

    /// The four pieces' total protection, which is what a player compares.
    pub fn set_protection(&self) -> i32 {
        self.protection.iter().sum()
    }
}

/// The material an armour id names, or `None`.
pub fn armour_material(item: &str) -> Option<&'static ArmourMaterial> {
    let name = match item {
        "plate_armor_helmet" | "plate_armor_chest" | "plate_armor_legs" | "plate_armor_boots" => {
            "minecolonies:plate_armor"
        }
        "pirate_hat" | "pirate_top" | "pirate_leggins" | "pirate_boots" => "minecolonies:pirate",
        "pirate_cap" | "pirate_chest" | "pirate_legs" | "pirate_shoes" => "minecolonies:pirate2",
        "santa_hat" => "minecolonies:santa_hat",
        "build_goggles" => "minecolonies:build_goggles",
        _ => return None,
    };
    ARMOUR.iter().find(|material| material.name == name)
}

/// The slot an armour id is worn in, or `None`.
pub fn armour_slot(item: &str) -> Option<ArmourSlot> {
    match item {
        "plate_armor_helmet" | "pirate_hat" | "pirate_cap" | "santa_hat" | "build_goggles" => {
            Some(ArmourSlot::Helmet)
        }
        "plate_armor_chest" | "pirate_top" | "pirate_chest" => Some(ArmourSlot::Chestplate),
        "plate_armor_legs" | "pirate_leggins" | "pirate_legs" => Some(ArmourSlot::Leggings),
        "plate_armor_boots" | "pirate_boots" | "pirate_shoes" => Some(ArmourSlot::Boots),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{get, Category, Refusal};

    #[test]
    fn the_three_hammers_climb_in_reach_and_in_durability_together() {
        assert_eq!(HAMMERS.len(), 3);
        assert_eq!(
            HAMMERS.iter().map(|hammer| hammer.reach).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            HAMMERS.iter().map(|hammer| hammer.durability).collect::<Vec<_>>(),
            vec![200, 400, 1000]
        );
        assert_eq!(hammer("assistanthammer_gold").unwrap().material, "gold");
        assert_eq!(hammer("assistanthammer_iron").unwrap().material, "iron");
        assert_eq!(hammer("assistanthammer_diamond").unwrap().material, "diamond");
        assert_eq!(hammer("spear"), None);
        // every hammer is a tool of the item table
        for hammer in HAMMERS {
            assert_eq!(get(hammer.item).unwrap().category, Category::Tool);
            assert!(get(hammer.item).unwrap().is_damageable());
        }
    }

    #[test]
    fn a_hammer_looks_at_seven_thirty_three_or_a_hundred_and_twenty_three_positions() {
        assert_eq!(reach_points(0), 1, "reach 0 is the block itself");
        assert_eq!(reach_points(1), 7);
        assert_eq!(reach_points(2), 33);
        assert_eq!(reach_points(3), 123);
        assert_eq!(reach_points(4), 257);
        assert_eq!(reach_points(-1), 0, "no hammer reaches backwards");
        assert_eq!(HAMMERS[0].positions(), 7);
        assert_eq!(HAMMERS[1].positions(), 33);
        assert_eq!(HAMMERS[2].positions(), 123);
    }

    #[test]
    fn a_hammer_places_one_block_per_use_for_as_many_uses_as_its_durability() {
        assert_eq!(HAMMER_COOLDOWN, 5);
        assert_eq!(HAMMER_WEAR, 1);
        assert_eq!(HAMMERS[0].placements(), 200);
        assert_eq!(HAMMERS[2].placements(), 1000);
        assert_eq!(HAMMER_ENCOURAGE_ONE_IN, 20);
        // five ticks a use is four uses a second, twenty a second without it
        assert_eq!(20 / HAMMER_COOLDOWN, 4);
    }

    #[test]
    fn the_hammer_refuses_in_java_s_order() {
        assert_eq!(may_assist(true, true, true), Ok(()));
        assert_eq!(may_assist(false, true, true), Err(AssistRefusal::NotAColony));
        assert_eq!(may_assist(false, false, false), Err(AssistRefusal::NotAColony), "colony first");
        assert_eq!(may_assist(true, false, true), Err(AssistRefusal::NoBuilderOrder));
        assert_eq!(may_assist(true, true, false), Err(AssistRefusal::StillClearing));
        assert_eq!(AssistRefusal::StillClearing.name(), "still_clearing");
        assert_eq!(AssistRefusal::NotAColony.name(), "not_a_colony");
        assert_eq!(AssistRefusal::NoBuilderOrder.name(), "no_builder_order");
        for refusal in [AssistRefusal::NotAColony, AssistRefusal::NoBuilderOrder, AssistRefusal::StillClearing] {
            assert!(refusal.name().chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    #[test]
    fn a_spear_needs_ten_ticks_of_charge_and_one_point_of_durability() {
        assert_eq!(SPEAR_BASE_DAMAGE, 3);
        assert_eq!(SPEAR_DURABILITY, 250);
        assert_eq!(SPEAR_THROW_TICKS, 10);
        assert!(spear_may_throw(10, 0, 250));
        assert!(!spear_may_throw(9, 0, 250), "nine ticks is not a throw");
        assert!(spear_may_throw(60, 248, 250), "one point left is enough");
        assert!(!spear_may_throw(60, 249, 250), "the last point refuses before the charge");
        assert!(!spear_may_throw(60, 250, 250));
        // ten ticks is half a second
        assert_eq!(SPEAR_THROW_TICKS, 20 / 2);
    }

    #[test]
    fn the_chief_sword_levitates_barbarians_and_lights_them_up() {
        assert_eq!(CHIEF_SWORD_DAMAGE_BONUS, 3);
        assert!((CHIEF_SWORD_ATTACK_SPEED + 2.4).abs() < 1e-9, "minus two point four");
        assert_eq!(CHIEF_SWORD_HIT.levitation_ticks, 200);
        assert_eq!(CHIEF_SWORD_HIT.levitation_amplifier, 2);
        assert_eq!(CHIEF_SWORD_HIT.glow_ticks, 600);
        assert_eq!(CHIEF_SWORD_HIT.glow_distance, 60);
        // ten seconds of levitation, thirty of glow, sixty blocks of it
        assert_eq!(CHIEF_SWORD_HIT.levitation_ticks, 20 * 10);
        assert_eq!(CHIEF_SWORD_HIT.glow_ticks, 20 * 30);
        // level III is amplifier 2
        assert_eq!(CHIEF_SWORD_HIT.levitation_amplifier + 1, 3);
    }

    #[test]
    fn the_scimitar_is_the_iron_tier_and_states_the_same_bonus_as_the_chief_sword() {
        assert_eq!(SCIMITAR_DAMAGE_BONUS, CHIEF_SWORD_DAMAGE_BONUS);
        assert!((SCIMITAR_ATTACK_SPEED - CHIEF_SWORD_ATTACK_SPEED).abs() < 1e-9);
        let scimitar = stats("iron_scimitar").unwrap();
        let chief = stats("chiefsword").unwrap();
        assert_eq!(scimitar.tier, Some("iron"));
        assert_eq!(chief.tier, Some("diamond"));
        // the durability is the tier's, so nothing is claimed here
        assert_eq!(scimitar.durability, None);
        assert_eq!(chief.durability, None);
    }

    #[test]
    fn the_pharaoh_scepter_is_a_bow_with_its_own_durability() {
        assert_eq!(PHARAO_DURABILITY, 384);
        assert!((PHARAO_ARROW_VELOCITY - 3.0).abs() < 1e-9);
        assert_eq!(stats("pharaoscepter").unwrap().durability, Some(384));
        // the template is the bow, so the engine's bow rules apply
        assert_eq!(get("pharaoscepter").unwrap().template, Some("minecraft:bow"));
    }

    #[test]
    fn the_two_scepters_live_for_two_uses_and_the_two_area_ones_never_wear_out() {
        assert_eq!(SCEPTER_DURABILITY, 2);
        assert_eq!(stats("scepterpermission").unwrap().durability, Some(2));
        assert_eq!(stats("scepterguard").unwrap().durability, Some(2));
        // the two area scepters say `stacksTo(1)` and no durability at all
        assert_eq!(stats("scepterlumberjack").unwrap().durability, None);
        assert_eq!(stats("scepterbeekeeper").unwrap().durability, None);
    }

    #[test]
    fn a_zero_durability_is_never_breaking_not_breaking_at_once() {
        // `.durability(0)` on the banner and on the scanner
        assert_eq!(stats("banner_rally_guards").unwrap().durability, Some(0));
        assert_eq!(stats("scan_analyzer").unwrap().durability, Some(0));
        assert_eq!(ANALYZER_TIMEOUT, 2_400);
        assert_eq!(ANALYZER_TIMEOUT, 20 * 60 * 2);
    }

    #[test]
    fn the_four_meshes_are_five_hundred_to_two_thousand_uses_and_no_repair() {
        assert_eq!(MESH_DURABILITIES, [500, 1000, 1500, 2000]);
        for name in ["sifter_mesh_string", "sifter_mesh_flint", "sifter_mesh_iron", "sifter_mesh_diamond"] {
            assert!(stats(name).is_none(), "{name} has a durability and no tool row");
            assert!(get(name).unwrap().is_damageable());
        }
        // 500 to 2000 is a factor of four, and the four tiers are the sifter's
        assert_eq!(MESH_DURABILITIES[3] / MESH_DURABILITIES[0], 4);
    }

    #[test]
    fn every_tool_row_is_a_tool_of_the_item_table_and_the_thirteen_are_covered() {
        assert_eq!(TOOLS.len(), 13);
        for stats in TOOLS {
            assert_eq!(get(stats.item).unwrap().category, Category::Tool, "{}", stats.item);
            assert!(stats.source.contains(".java:"), "{}", stats.item);
            assert_eq!(super::stats(stats.item), Some(stats));
        }
        // 17 tools in the table, 13 with stats, 4 meshes without
        assert_eq!(crate::items::count_of(Category::Tool), TOOLS.len() + 4);
    }

    #[test]
    fn a_non_tool_is_refused_by_name() {
        assert_eq!(require("compost"), Err(Refusal::NotATool { id: "compost" }));
        assert_eq!(Refusal::NotATool { id: "compost" }.name(), "not_a_tool");
        assert_eq!(stats("cheddar_cheese"), None);
        assert_eq!(stats("sifter_mesh_iron"), None, "a mesh is not a tool here");
    }

    #[test]
    fn the_armour_durability_is_the_health_unit_times_the_multiplier() {
        assert_eq!(ARMOUR_HEALTH_UNIT, [13, 15, 16, 11]);
        let plate = &ARMOUR[0];
        assert_eq!(plate.durability(ArmourSlot::Helmet), 407);
        assert_eq!(plate.durability(ArmourSlot::Chestplate), 592);
        assert_eq!(plate.durability(ArmourSlot::Leggings), 555);
        assert_eq!(plate.durability(ArmourSlot::Boots), 481);
        // the chest piece is always the toughest piece
        for material in ARMOUR {
            assert!(
                material.durability(ArmourSlot::Chestplate) > material.durability(ArmourSlot::Helmet)
            );
        }
    }

    #[test]
    fn the_santa_hat_is_five_thousand_five_hundred_durability_of_nothing() {
        let santa = ARMOUR.iter().find(|m| m.name == "minecolonies:santa_hat").unwrap();
        assert_eq!(santa.durability(ArmourSlot::Helmet), 5_500);
        assert_eq!(santa.protection(ArmourSlot::Helmet), 0);
        assert_eq!(santa.set_protection(), 0);
        let goggles = ARMOUR.iter().find(|m| m.name == "minecolonies:build_goggles").unwrap();
        assert_eq!(goggles.durability(ArmourSlot::Helmet), 22);
        assert_eq!(goggles.protection(ArmourSlot::Helmet), 0);
        assert_eq!(goggles.enchantment, 20, "the highest of the five");
        assert_eq!(santa.enchantment, 5);
    }

    #[test]
    fn the_pirate_set_2_protects_like_the_plate_set_and_lasts_half_as_long() {
        let plate = &ARMOUR[0];
        let pirate2 = &ARMOUR[2];
        assert_eq!(plate.protection, pirate2.protection);
        assert_eq!(pirate2.durability_multiplier * 2, 30);
        assert!(plate.durability_multiplier > pirate2.durability_multiplier * 2);
        // and it is the only material with toughness
        assert_eq!(pirate2.toughness, 2);
        for material in ARMOUR {
            if material.name != "minecolonies:pirate2" {
                assert_eq!(material.toughness, 0, "{}", material.name);
            }
        }
        // the five set-protection totals: 20, 15, 20, 0, 0
        assert_eq!(
            ARMOUR.iter().map(ArmourMaterial::set_protection).collect::<Vec<_>>(),
            vec![20, 15, 20, 0, 0]
        );
    }

    #[test]
    fn every_armour_item_of_the_table_has_a_material_and_a_slot() {
        let mut seen = 0;
        for entry in crate::items::of_category(Category::Armour) {
            let material = armour_material(entry.id).unwrap_or_else(|| panic!("{}", entry.id));
            let slot = armour_slot(entry.id).unwrap_or_else(|| panic!("{}", entry.id));
            assert!(material.durability(slot) > 0, "{}", entry.id);
            assert!(entry.is_damageable(), "{}", entry.id);
            seen += 1;
        }
        assert_eq!(seen, 14);
        assert_eq!(armour_material("spear"), None);
        assert_eq!(armour_slot("spear"), None);
    }

    #[test]
    fn the_eight_pirate_pieces_are_two_materials_of_four_pieces() {
        let pirate = ["pirate_hat", "pirate_top", "pirate_leggins", "pirate_boots"];
        let pirate2 = ["pirate_cap", "pirate_chest", "pirate_legs", "pirate_shoes"];
        for id in pirate {
            assert_eq!(armour_material(id).unwrap().name, "minecolonies:pirate");
        }
        for id in pirate2 {
            assert_eq!(armour_material(id).unwrap().name, "minecolonies:pirate2");
        }
        // one piece per slot in each set
        for set in [pirate, pirate2] {
            let mut slots: Vec<ArmourSlot> =
                set.iter().map(|id| armour_slot(id).unwrap()).collect();
            slots.sort();
            slots.dedup();
            assert_eq!(slots.len(), 4);
        }
    }

    #[test]
    fn the_armour_slots_are_java_s_order_and_each_has_its_own_unit() {
        for (index, slot) in ArmourSlot::ALL.iter().enumerate() {
            assert_eq!(slot.index(), index);
            assert_eq!(slot.health_unit(), ARMOUR_HEALTH_UNIT[index]);
            assert!(!slot.name().is_empty());
        }
        assert_eq!(ArmourSlot::Boots.name(), "boots");
        assert_eq!(ArmourSlot::Helmet.name(), "helmet");
        // the unit table is 13/15/16/11 in that order, which is not a guess:
        // boots, leggings, chestplate, helmet (`MineColoniesArmorMaterial.java:21-26`)
        assert_eq!(ArmourSlot::Boots.health_unit(), 13);
        assert_eq!(ArmourSlot::Helmet.health_unit(), 11);
    }

    #[test]
    fn none_of_the_seventeen_tools_is_one_of_the_fourteen_equipment_kinds() {
        // The equipment registry is a different list, and the port already has it:
        // a hammer is not a pickaxe, and the spear is the one kind that matches.
        use crate::jobs::tool::ToolKind;
        assert_eq!(ToolKind::from_name("assistanthammer_diamond"), None);
        assert_eq!(ToolKind::from_name("sifter_mesh_iron"), None);
        assert_eq!(ToolKind::from_name("scepterguard"), None);
        assert_eq!(ToolKind::from_name("spear"), Some(ToolKind::Spear));
        // and the spear is level-less, which is why its row in `kinds` says `-1`
        assert_eq!(ToolKind::Spear.min_level(), crate::jobs::tool::TOOL_LEVEL_HAND);
    }
}
