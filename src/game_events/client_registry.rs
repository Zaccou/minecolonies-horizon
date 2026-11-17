//! `ClientRegistryHandler.java`: 94 model layers, 47 entity renderers, 7 block entity
//! renderers — and why almost none of it is ported.
//!
//! Java reference: `ClientRegistryHandler.java`, 388 lines, 8 branches, all in
//! [`super::table`]. It is the largest file of the fourteen by *scope* and the
//! smallest by *decision*: seven `register*` methods, each a list of registrations,
//! and two of them (the render layers and the item properties) are one line each.
//!
//! **What the port keeps: the names, because they are data.** [`LAYERS`] is the 94
//! `ModelLayerLocation`s as Java declares them, and [`layer_id`] is the id they become
//! — `<mod id>:<model>#<layer>` — which is the form
//! `register_layer_definitions` asks for (`crates/horizon-mod-vocab/src/hooks/register_layer_definitions.rs:8-16`).
//! Two facts fall out and are tested: **every model id equals its layer id** (the Java
//! writes the same string twice, 94 times), and **no two are equal**.
//!
//! **What the port does not keep: the models themselves.** Each layer is a
//! `createMesh` call into a `core/client/model/**` class, and each of the 47 renderers
//! is a `core/client/render/**` class. The port draws its citizen as a
//! `minecraft:wandering_trader` (`src/citizen.rs`), so a citizen model of its own is
//! not a registration it is missing — it is a renderer the port does not have. The
//! count still matters: it is how much of this file a second port would have to
//! write, and the table says which hooks would carry it.
//!
//! **The seven block entity renderers have no hook.** `register_entity_renderers`
//! takes entity types; a block entity's drawing is its Part's `block_entity_submit`
//! (`hooks/block_entity_submit.rs`), one Part per block entity type. Seven
//! `BlockEntityRenderers` with their own `render` methods are a different shape, and
//! `MANQUANT.md` names it.

/// The mod id every layer is namespaced with (`Constants.MOD_ID`).
pub const MOD_ID: &str = "minecolonies";

/// The 94 `ModelLayerLocation`s of `:56-155`, as Java declares them.
///
/// Every one is `new ModelLayerLocation(new ResourceLocation(MOD_ID, name), name)`,
/// so the model id and the layer id are the same string — which is why one name is
/// enough. Sorted, so a reader can find one.
pub const LAYERS: &[&str] = &[
    "amazon", "amazon_chief", "amazon_spearman", "archer_mummy", "citizen", "female_alchemist",
    "female_archer", "female_aristocrat", "female_baker", "female_beekeeper", "female_blacksmith",
    "female_builder", "female_carpenter", "female_chickenfarmer", "female_child", "female_citizen",
    "female_citizennoble", "female_composter", "female_concretemixer", "female_cook",
    "female_cowfarmer", "female_crafter", "female_deliveryman", "female_druid", "female_dyer",
    "female_enchanter", "female_farmer", "female_fisherman", "female_fletcher", "female_florist",
    "female_glassblower", "female_healer", "female_knight", "female_lumberjack",
    "female_mechanist", "female_miner", "female_netherworker", "female_pigfarmer",
    "female_planter", "female_rabbitherder", "female_settler", "female_sheepfarmer",
    "female_smelter", "female_student", "female_teacher", "female_undertaker", "male_alchemist",
    "male_archer", "male_aristocrat", "male_baker", "male_beekeeper", "male_blacksmith",
    "male_builder", "male_carpenter", "male_chickenfarmer", "male_child", "male_citizen",
    "male_citizennoble", "male_composter", "male_concretemixer", "male_cook", "male_cowfarmer",
    "male_crafter", "male_deliveryman", "male_druid", "male_dyer", "male_enchanter",
    "male_farmer", "male_fisherman", "male_fletcher", "male_florist", "male_glassblower",
    "male_healer", "male_knight", "male_lumberjack", "male_mechanist", "male_miner",
    "male_netherworker", "male_pigfarmer", "male_planter", "male_rabbitherder", "male_settler",
    "male_sheepfarmer", "male_smelter", "male_student", "male_teacher", "male_undertaker",
    "mercenary", "mummy", "norsemen_archer", "norsemen_chief", "pharao", "scarecrow",
    "shield_maiden",
];

/// The layer's id in the form `register_layer_definitions` asks for
/// (`register_layer_definitions.rs:8-16`): `minecolonies:citizen#citizen`.
pub fn layer_id(name: &str) -> String {
    format!("{MOD_ID}:{name}#{name}")
}

/// The 47 entity renderers of `:275-346`, counted from the Java.
///
/// Seven are this mod's own types (`CITIZEN`, `VISITOR`, `FISHHOOK`, `FIREARROW`,
/// `SPEAR`, `MC_NORMAL_ARROW`, `DRUID_POTION`), thirty-six are raiders (six families —
/// barbarian, pirate, mummy, norsemen, amazon, drowned — each as a plain, an archer and
/// a chief, then the same eighteen camp variants), and four are misc (the mercenary, the
/// sitting entity, the minecart and the cavalry horse).
pub const RENDERERS: u32 = 47;

/// The seven block entity renderers of `:348-354`.
pub const BLOCK_ENTITY_RENDERERS: &[&str] = &[
    "BUILDING",
    "SCARECROW",
    "ENCHANTER",
    "COLONY_FLAG",
    "NAMED_GRAVE",
    "DECO_CONTROLLER",
    "COLONY_SIGN",
];

/// The seven blocks Java forces to `cutout` besides the huts and crops (`:358-364`).
pub const CUTOUT_BLOCKS: &[&str] = &[
    "blockScarecrow",
    "blockRack",
    "blockDecorationPlaceholder",
    "blockCompostedDirt",
    "blockBarrel",
    "blockWayPoint",
    "floodedFarmland",
];

/// The two item properties of `:368-371`, as `(item, property)`.
pub const ITEM_PROPERTIES: &[(&str, &str)] = &[("spear", "throwing"), ("buildGoggles", "disabled")];

/// The two item decorations of `:265-269`.
pub const ITEM_DECORATIONS: &[(&str, &str)] =
    &[("colonyMap", "ColonyMapDecorator"), ("clipboard", "ClipBoardDecorator")];

/// The one key `ModKeyMappings.register` binds (`api/client/ModKeyMappings.java:20-29`),
/// as `(registration id, translation key)`.
///
/// Registration is **not** a hook: it is `Registrar::register_key_mapping(KeyRegistration)`
/// in a mod's `register` (`crates/horizon-mod-api/src/parts/key_mappings.rs:30-45`), which
/// is where the port's `lib.rs` would put it. The Java leaves it unbound
/// (`InputConstants.UNKNOWN`) and in the `IN_GAME` conflict context, which a
/// `KeyRegistration` carries as its default unbound state.
pub const KEYS: &[(&str, &str)] = &[("toggle_goggles", "key.minecolonies.toggle_goggles")];

/// The category every key is filed under (`:15`).
pub const KEY_CATEGORY: &str = "key.minecolonies.categories.general";

/// Where a hut's render layer comes from: `Arrays.stream(ModBlocks.getHuts())` forces
/// every hut to `cutout` **or** `solid` (`:356-357`), and every crop to `cutout`
/// (`:366`).
pub fn hut_accepts_cutout_or_solid(cutout: bool, solid: bool) -> bool {
    cutout || solid
}

/// `willRenderBlueprints()`'s inverse, which is what the goggles' `disabled` property
/// asks (`:370-371`): 0 when blueprints render, 1 when they do not.
pub fn goggles_disabled(blueprints_render: bool) -> f32 {
    if blueprints_render {
        0.0
    } else {
        1.0
    }
}

/// The spear's `throwing` property (`:368-369`): 1 while the holder is using it.
pub fn spear_throwing(using: bool, held: bool) -> f32 {
    if using && held {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_ninety_four_model_layers() {
        assert_eq!(LAYERS.len(), 94);
    }

    #[test]
    fn no_layer_appears_twice() {
        let mut sorted = LAYERS.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "a layer is declared twice");
    }

    #[test]
    fn the_list_is_sorted() {
        for pair in LAYERS.windows(2) {
            assert!(pair[0] < pair[1], "{} > {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn every_layer_id_has_the_form_the_hook_asks_for() {
        for name in LAYERS {
            let id = layer_id(name);
            assert_eq!(id, format!("minecolonies:{name}#{name}"));
            assert_eq!(id.matches('#').count(), 1);
            assert_eq!(id.matches(':').count(), 1);
        }
    }

    #[test]
    fn the_four_families_are_all_there() {
        // A name per family, so a truncated paste fails here rather than in a picker.
        for name in ["citizen", "mercenary", "scarecrow", "pharao"] {
            assert!(LAYERS.contains(&name), "{name} is missing");
        }
        assert!(LAYERS.contains(&"male_deliveryman"), "the courier's model id");
        assert!(!LAYERS.contains(&"male_courier"), "the constant is MALE_COURIER, the id is not");
    }

    #[test]
    fn the_seven_block_entity_renderers_are_the_seven_tile_entities() {
        assert_eq!(BLOCK_ENTITY_RENDERERS.len(), 7);
        assert_eq!(BLOCK_ENTITY_RENDERERS[0], "BUILDING");
        assert_eq!(BLOCK_ENTITY_RENDERERS[6], "COLONY_SIGN");
    }

    #[test]
    fn the_cutout_list_has_the_seven_blocks_and_no_hut() {
        assert_eq!(CUTOUT_BLOCKS.len(), 7);
        assert!(!CUTOUT_BLOCKS.iter().any(|name| name.contains("blockHut")));
    }

    #[test]
    fn a_hut_layer_is_cutout_or_solid_but_never_both_required() {
        assert!(hut_accepts_cutout_or_solid(true, false));
        assert!(hut_accepts_cutout_or_solid(false, true));
        assert!(hut_accepts_cutout_or_solid(true, true));
        assert!(!hut_accepts_cutout_or_solid(false, false));
    }

    #[test]
    fn the_goggles_are_disabled_only_while_no_blueprint_renders() {
        assert_eq!(goggles_disabled(true), 0.0);
        assert_eq!(goggles_disabled(false), 1.0);
    }

    #[test]
    fn the_spear_throws_only_while_it_is_used() {
        assert_eq!(spear_throwing(true, true), 1.0);
        assert_eq!(spear_throwing(true, false), 0.0, "another stack is in hand");
        assert_eq!(spear_throwing(false, true), 0.0);
    }

    #[test]
    fn the_two_decorations_and_two_properties_name_their_owner() {
        assert_eq!(ITEM_DECORATIONS.len(), 2);
        assert_eq!(ITEM_PROPERTIES.len(), 2);
        assert_eq!(ITEM_PROPERTIES[0], ("spear", "throwing"));
        assert_eq!(ITEM_PROPERTIES[1], ("buildGoggles", "disabled"));
        assert_eq!(RENDERERS, 47);
    }

    #[test]
    fn one_key_is_registered_and_it_is_unbound_in_the_java() {
        assert_eq!(KEYS.len(), 1);
        assert_eq!(KEYS[0].1, "key.minecolonies.toggle_goggles");
        assert_eq!(KEY_CATEGORY, "key.minecolonies.categories.general");
    }
}
