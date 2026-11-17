//! Every entity the mod draws: which renderer, which model, which file, which
//! layers, and what the file index means.
//!
//! Java reference: `core/event/ClientRegistryHandler.doClientStuff`
//! (`:284-345`, the 47 `registerEntityRenderer` calls),
//! `apiimp/initializer/EntityInitializer.java` (the 47 registry paths),
//! and the 27 renderer classes under `core/client/render/` and
//! `core/client/render/mobs/`.
//!
//! **47 entity types, 27 renderers, and 18 of the types are the same mob twice.**
//! A raid spawns `minecolonies:barbarian`, the barbarian *camp* spawns
//! `minecolonies:campbarbarian`, and both are drawn by `RendererBarbarian`
//! (`:299`, `:311`) — one row with two ids, because that is how Java writes it.
//!
//! **The texture is a function of `entity.getTextureId()` for four families only.**
//! `AbstractEntityPirateRaider` sets `textureId = new Random().nextInt(4)`
//! (`:47`), the norsemen `nextInt(3)` (`AbstractEntityNorsemenRaider.java:29`), and
//! every other raider keeps the field at its default 0, which is why the mummy
//! renderer ignores it. Three rules, not one:
//!
//! | Family | Rule | Where |
//! | --- | --- | --- |
//! | pirate, drowned pirate | `switch`: 0-2 named, anything else the fourth file | `RendererPirate.java:60-73` |
//! | norsemen | `== 1 ? second : first` — id 2 **and** 0 draw the first file | `RendererArcherNorsemen.java:51-56` |
//! | everything else | one file, `textureId` unread | `RendererMummy.java:33-37` |
//!
//! **What is *not* here**: the citizen model and its texture, which is a table of
//! 42 and lives in [`crate::render::models`] because a job chooses it; and the
//! block entities, in [`crate::render::blocks`].

/// Where an entity's model comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four sources, and "none" is one of them
pub enum Model {
    /// A layer the mod registers, by its `ModelLayerLocation` id.
    Own(&'static str),
    /// Vanilla's player model (`ModelLayers.PLAYER`), which the barbarians, the
    /// pirates and the mercenary's armour layer bake.
    Player,
    /// A vanilla layer the mod bakes for its own model (`minecraft:trident#main`
    /// for the spear).
    Vanilla(&'static str),
    /// No model: the renderer draws a flat quad, an item, or nothing.
    None,
}

/// How an entity's texture file is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: five rules, one per shape in the tree
pub enum Texture {
    /// One file for the whole type.
    Single(&'static str),
    /// `switch (getTextureId())` with a `default` that is the last file
    /// (`RendererPirate.java:60-73`).
    ByIndex(&'static [&'static str]),
    /// `getTextureId() == 1` picks the second file, everything else the first
    /// (`RendererArcherNorsemen.java:51-56`).
    SecondOnOne(&'static str, &'static str),
    /// The 42-model table of [`crate::render::models`].
    Citizen,
    /// Nothing drawn (`RenderSitting.java:35-40`).
    None,
}

impl Texture {
    /// The file this id draws, or `None` when the renderer names none.
    pub fn resolve(&self, texture_id: usize) -> Option<&'static str> {
        match self {
            Texture::Single(path) => Some(*path),
            Texture::ByIndex(list) => {
                if list.is_empty() {
                    return None;
                }
                list.get(texture_id).or_else(|| list.last()).copied()
            }
            Texture::SecondOnOne(first, second) => {
                if texture_id == 1 {
                    Some(*second)
                } else {
                    Some(*first)
                }
            }
            Texture::Citizen | Texture::None => None,
        }
    }

    /// How many distinct files this rule can draw, when that is a property of the
    /// rule at all — a citizen's count depends on its model type, so it answers
    /// `None` and [`crate::render::models::ModelType::texture`] decides.
    pub fn distinct(&self) -> Option<usize> {
        match self {
            Texture::Single(_) => Some(1),
            Texture::ByIndex(list) => Some(list.len()),
            Texture::SecondOnOne(..) => Some(2),
            Texture::Citizen => None,
            Texture::None => Some(0),
        }
    }
}

/// A render layer added after the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three layers the raiders and the citizen wear
pub enum Layer {
    /// `ItemInHandLayer` (`HumanoidMobRenderer`'s own).
    ItemInHand,
    /// `HumanoidArmorLayer` over the vanilla player armour models.
    Armor,
    /// `CavalryOverlayLayer`: the horse's readiness, see [`cavalry_segments`].
    CavalryReadiness,
}

/// One `registerEntityRenderer` call.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: eight fields, all read
pub struct Renderer {
    /// The Java class, which is also the row's name.
    pub name: &'static str,
    /// Every entity id this renderer draws, full registry names.
    pub entities: &'static [&'static str],
    /// Where the model is baked from.
    pub model: Model,
    /// How the file is chosen.
    pub texture: Texture,
    /// The layers drawn after the model.
    pub layers: &'static [Layer],
    /// `shadowRadius`, the constructor's third argument.
    pub shadow: f32,
    /// How many values `getTextureId()` can take, 0 when nothing reads it.
    pub variants: u8,
    /// `fichier.java:ligne` of the registration.
    pub source: &'static str,
}

/// Every entity the mod draws, in registration order.
///
/// The eighteen `*camp*` ids sit beside their raider twin on purpose: one row,
/// two ids, because Java registers one renderer for both.
pub const RENDERERS: &[Renderer] = &[
    Renderer {
        name: "RenderBipedCitizen",
        entities: &["minecolonies:citizen", "minecolonies:visitor"],
        model: Model::Player,
        texture: Texture::Citizen,
        layers: &[Layer::Armor, Layer::ItemInHand],
        shadow: 0.5,
        variants: 0,
        source: "RenderBipedCitizen.java:40-47",
    },
    Renderer {
        name: "RenderFishHook",
        entities: &["minecolonies:fishhook"],
        model: Model::None,
        texture: Texture::Single("minecraft:textures/entity/fishing_hook.png"),
        layers: &[],
        shadow: 0.0,
        variants: 0,
        source: "RenderFishHook.java:32",
    },
    Renderer {
        name: "FireArrowRenderer",
        entities: &["minecolonies:firearrow"],
        model: Model::Vanilla("minecraft:arrow#main"),
        texture: Texture::ByIndex(&[
            "textures/item/magicalarrows/magical_arrow1.png",
            "textures/item/magicalarrows/magical_arrow2.png",
            "textures/item/magicalarrows/magical_arrow3.png",
            "textures/item/magicalarrows/magical_arrow4.png",
            "textures/item/magicalarrows/magical_arrow5.png",
            "textures/item/magicalarrows/magical_arrow6.png",
        ]),
        layers: &[],
        shadow: 0.0,
        variants: 6,
        source: "projectile/FireArrowRenderer.java:20-42",
    },
    Renderer {
        name: "RendererSpear",
        entities: &["minecolonies:spear"],
        model: Model::Vanilla("minecraft:trident#main"),
        texture: Texture::Single("textures/entity/spear.png"),
        layers: &[],
        shadow: 0.0,
        variants: 0,
        source: "projectile/RendererSpear.java:28-34",
    },
    Renderer {
        name: "TippableArrowRenderer",
        entities: &["minecolonies:mcnormalarrow"],
        model: Model::Vanilla("minecraft:arrow#main"),
        texture: Texture::None,
        layers: &[],
        shadow: 0.0,
        variants: 0,
        source: "ClientRegistryHandler.java:291",
    },
    Renderer {
        name: "ThrownItemRenderer",
        entities: &["minecolonies:druidpotion"],
        model: Model::None,
        texture: Texture::None,
        layers: &[],
        shadow: 1.0,
        variants: 0,
        source: "ClientRegistryHandler.java:292",
    },
    Renderer {
        name: "RendererBarbarian",
        entities: &[
            "minecolonies:barbarian",
            "minecolonies:archerbarbarian",
            "minecolonies:campbarbarian",
            "minecolonies:camparcherbarbarian",
        ],
        model: Model::Player,
        texture: Texture::Single("textures/entity/raiders/barbarian1.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/barbarians/RendererBarbarian.java:19-35",
    },
    Renderer {
        name: "RendererChiefBarbarian",
        entities: &[
            "minecolonies:chiefbarbarian",
            "minecolonies:campchiefbarbarian",
        ],
        model: Model::Player,
        texture: Texture::Single("textures/entity/raiders/barbarianchief1.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/barbarians/RendererChiefBarbarian.java:19-32",
    },
    Renderer {
        name: "RendererPirate",
        entities: &["minecolonies:pirate", "minecolonies:camppirate"],
        model: Model::Player,
        texture: Texture::ByIndex(&[
            "textures/entity/raiders/pirate1.png",
            "textures/entity/raiders/pirate2.png",
            "textures/entity/raiders/pirate3.png",
            "textures/entity/raiders/pirate4.png",
        ]),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 4,
        source: "mobs/pirates/RendererPirate.java:60-73",
    },
    Renderer {
        name: "RendererArcherPirate",
        entities: &["minecolonies:archerpirate", "minecolonies:camparcherpirate"],
        model: Model::Player,
        texture: Texture::ByIndex(&[
            "textures/entity/raiders/pirate5.png",
            "textures/entity/raiders/pirate6.png",
            "textures/entity/raiders/pirate7.png",
            "textures/entity/raiders/pirate8.png",
        ]),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 4,
        source: "mobs/pirates/RendererArcherPirate.java:60-73",
    },
    Renderer {
        name: "RendererChiefPirate",
        entities: &["minecolonies:chiefpirate", "minecolonies:campchiefpirate"],
        model: Model::Player,
        texture: Texture::ByIndex(&[
            "textures/entity/raiders/pirate_nude1.png",
            "textures/entity/raiders/pirate_nude2.png",
            "textures/entity/raiders/pirate_nude3.png",
            "textures/entity/raiders/pirate_nude4.png",
        ]),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 4,
        source: "mobs/pirates/RendererChiefPirate.java:61-74",
    },
    Renderer {
        name: "RendererMummy",
        entities: &["minecolonies:mummy", "minecolonies:campmummy"],
        model: Model::Own("minecolonies:mummy"),
        texture: Texture::Single("textures/entity/raiders/mummy.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/egyptians/RendererMummy.java:24-38",
    },
    Renderer {
        name: "RendererArcherMummy",
        entities: &["minecolonies:archermummy", "minecolonies:camparchermummy"],
        model: Model::Own("minecolonies:archer_mummy"),
        texture: Texture::Single("textures/entity/raiders/archer_mummy.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/egyptians/RendererArcherMummy.java:24-36",
    },
    Renderer {
        name: "RendererPharao",
        entities: &["minecolonies:pharao", "minecolonies:camppharao"],
        model: Model::Own("minecolonies:pharao"),
        texture: Texture::Single("textures/entity/raiders/pharao.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/egyptians/RendererPharao.java:24-36",
    },
    Renderer {
        name: "RendererShieldmaidenNorsemen",
        entities: &["minecolonies:shieldmaiden", "minecolonies:campshieldmaiden"],
        model: Model::Own("minecolonies:shield_maiden"),
        texture: Texture::SecondOnOne(
            "textures/entity/raiders/norsemen_shieldmaiden1.png",
            "textures/entity/raiders/norsemen_shieldmaiden2.png",
        ),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 3,
        source: "mobs/norsemen/RendererShieldmaidenNorsemen.java:50-57",
    },
    Renderer {
        name: "RendererArcherNorsemen",
        entities: &[
            "minecolonies:norsemenarcher",
            "minecolonies:campnorsemenarcher",
        ],
        model: Model::Own("minecolonies:norsemen_archer"),
        texture: Texture::SecondOnOne(
            "textures/entity/raiders/norsemen_archer1.png",
            "textures/entity/raiders/norsemen_archer2.png",
        ),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 3,
        source: "mobs/norsemen/RendererArcherNorsemen.java:51-56",
    },
    Renderer {
        name: "RendererChiefNorsemen",
        entities: &["minecolonies:norsemenchief", "minecolonies:campnorsemenchief"],
        model: Model::Own("minecolonies:norsemen_chief"),
        texture: Texture::Single("textures/entity/raiders/norsemen_chief.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/norsemen/RendererChiefNorsemen.java:24-36",
    },
    Renderer {
        name: "RendererAmazon",
        entities: &["minecolonies:amazon", "minecolonies:campamazon"],
        model: Model::Own("minecolonies:amazon"),
        texture: Texture::Single("textures/entity/raiders/amazon.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/amazon/RendererAmazon.java:24-36",
    },
    Renderer {
        name: "RendererChiefAmazon",
        entities: &["minecolonies:amazonchief", "minecolonies:campamazonchief"],
        model: Model::Own("minecolonies:amazon_chief"),
        texture: Texture::Single("textures/entity/raiders/amazon_chief.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/amazon/RendererChiefAmazon.java:24-36",
    },
    Renderer {
        name: "RendererAmazonSpearman",
        entities: &[
            "minecolonies:amazonspearman",
            "minecolonies:campamazonspearman",
        ],
        model: Model::Own("minecolonies:amazon_spearman"),
        texture: Texture::Single("textures/entity/raiders/amazon_spearman.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/amazon/RendererAmazonSpearman.java:24-36",
    },
    Renderer {
        name: "RendererDrownedPirate",
        entities: &["minecolonies:drownedpirate", "minecolonies:campdrownedpirate"],
        model: Model::Player,
        texture: Texture::ByIndex(&[
            "textures/entity/raiders/drowned_pirate1.png",
            "textures/entity/raiders/drowned_pirate2.png",
            "textures/entity/raiders/drowned_pirate3.png",
            "textures/entity/raiders/drowned_pirate4.png",
        ]),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 4,
        source: "mobs/drownedpirates/RendererDrownedPirate.java:61-74",
    },
    Renderer {
        name: "RendererDrownedArcherPirate",
        entities: &[
            "minecolonies:drownedarcherpirate",
            "minecolonies:campdrownedarcherpirate",
        ],
        model: Model::Player,
        texture: Texture::ByIndex(&[
            "textures/entity/raiders/drowned_pirate5.png",
            "textures/entity/raiders/drowned_pirate6.png",
            "textures/entity/raiders/drowned_pirate7.png",
            "textures/entity/raiders/drowned_pirate8.png",
        ]),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 4,
        source: "mobs/drownedpirates/RendererDrownedArcherPirate.java:60-73",
    },
    Renderer {
        name: "RendererDrownedChiefPirate",
        entities: &[
            "minecolonies:drownedchiefpirate",
            "minecolonies:campdrownedchiefpirate",
        ],
        model: Model::Player,
        texture: Texture::Single("textures/entity/raiders/drowned_pirate_nude.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/drownedpirates/RendererDrownedChiefPirate.java:39-44",
    },
    Renderer {
        name: "RenderMercenary",
        entities: &["minecolonies:mercenary"],
        model: Model::Own("minecolonies:mercenary"),
        texture: Texture::Single("textures/entity/citizen/default/settlermale1_b.png"),
        layers: &[Layer::ItemInHand, Layer::Armor],
        shadow: 0.5,
        variants: 0,
        source: "mobs/RenderMercenary.java:32-47",
    },
    Renderer {
        name: "RenderSitting",
        entities: &["minecolonies:sittingentity"],
        model: Model::None,
        texture: Texture::None,
        layers: &[],
        shadow: 0.0,
        variants: 0,
        source: "RenderSitting.java:35-40",
    },
    Renderer {
        name: "MinecartRenderer",
        entities: &["minecolonies:mcminecart"],
        model: Model::Vanilla("minecraft:minecart#main"),
        texture: Texture::None,
        layers: &[],
        shadow: 0.0,
        variants: 0,
        source: "ClientRegistryHandler.java:343",
    },
    Renderer {
        name: "HorseRenderer",
        entities: &["minecolonies:cavalry_horse"],
        model: Model::Vanilla("minecraft:horse#main"),
        texture: Texture::None,
        layers: &[Layer::CavalryReadiness],
        shadow: 0.0,
        variants: 0,
        source: "ClientRegistryHandler.java:344-348",
    },
];

/// The overlay texture a cavalry horse wears, `0` to `5`, from its combat
/// readiness (`CavalryOverlayLayer.java:44-72`).
pub const CAVALRY_SEGMENTS: u8 = 5;

/// `CavalryHorseEntity.COMBAT_READINESS_THRESHOLD` (`:107`): two thirds of the
/// horse's health is the cooldown that means "ready".
pub const COMBAT_READINESS_THRESHOLD: f32 = 0.66;

/// The overlay's alpha: `0.85`, the fourth colour argument of `renderToBuffer`
/// (`CavalryOverlayLayer.java:71`).
pub const CAVALRY_ALPHA: f32 = 0.85;

/// Which of the six overlay files a horse wears.
///
/// `readiness = clamp(1 - cooldown / max(0.001, maxHealth * 0.66), 0, 1)` then
/// `segments = clamp(floor(readiness * 5 + 0.0001), 0, 5)` (`:56-62`), and the
/// file name is `cavalry_overlay_layer<segments>.png` (`:64`) — **six** files
/// from a "5", because the counter starts at 0.
pub fn cavalry_segments(max_health: f32, cooldown: f32) -> u8 {
    let threshold = max_health * COMBAT_READINESS_THRESHOLD;
    let denominator = if threshold < 0.001 { 0.001 } else { threshold };
    let readiness = (1.0 - cooldown.max(0.0) / denominator).clamp(0.0, 1.0);
    let segments = (readiness * f32::from(CAVALRY_SEGMENTS) + 0.0001).floor();
    segments.clamp(0.0, f32::from(CAVALRY_SEGMENTS)) as u8
}

/// The overlay's file name for a readiness segment.
pub fn cavalry_texture(segment: u8) -> String {
    format!("textures/entity/horse/cavalry_overlay_layer{segment}.png")
}

/// The renderer that draws an entity id, or `None`.
pub fn renderer_for(entity: &str) -> Option<&'static Renderer> {
    RENDERERS
        .iter()
        .find(|row| row.entities.contains(&entity))
}

/// Every entity id this table draws, in table order.
pub fn entities() -> Vec<&'static str> {
    RENDERERS
        .iter()
        .flat_map(|row| row.entities.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_seven_renderers_over_forty_seven_entities() {
        assert_eq!(RENDERERS.len(), 27);
        let ids = entities();
        assert_eq!(ids.len(), 47);
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 47, "an entity id is drawn by two rows");
    }

    #[test]
    fn eighteen_entities_are_a_raider_and_its_camp_twin() {
        let camp: Vec<&str> = entities()
            .into_iter()
            .filter(|id| id.contains(":camp"))
            .collect();
        assert_eq!(camp.len(), 18);
        for id in camp {
            let twin = id.replacen(":camp", ":", 1);
            let row = renderer_for(id).expect("the camp row");
            let twin_row = renderer_for(&twin).unwrap_or_else(|| panic!("{twin} has no row"));
            assert_eq!(row.name, twin_row.name, "{id} and {twin}");
            assert!(
                std::ptr::eq(row, twin_row),
                "{id} and {twin} are two rows"
            );
        }
    }

    #[test]
    fn every_row_carries_a_source_and_at_least_one_entity() {
        for row in RENDERERS {
            assert!(!row.entities.is_empty(), "{}", row.name);
            assert!(row.source.contains(".java:"), "{}", row.name);
            assert!(
                row.entities.iter().all(|id| id.starts_with("minecolonies:")),
                "{}",
                row.name
            );
        }
    }

    #[test]
    fn every_layer_is_a_persistent_item_in_hand_and_armour_or_the_cavalry_one() {
        for row in RENDERERS {
            let has_item = row.layers.contains(&Layer::ItemInHand);
            let has_armor = row.layers.contains(&Layer::Armor);
            // Java adds both or neither: `AbstractRendererX` constructors are three
            // identical bodies (`AbstractRendererAmazon.java:26-32` and the four
            // siblings), and the two non-humanoid renderers add none.
            assert_eq!(has_item, has_armor, "{}", row.name);
        }
        assert!(RENDERERS
            .iter()
            .any(|row| row.layers.contains(&Layer::CavalryReadiness)));
    }

    #[test]
    fn the_shadow_is_half_a_block_wherever_java_sets_one() {
        // The citizen's `SHADOW_SIZE` is `0.5F` (`RenderBipedCitizen.java:30`) and
        // every humanoid raider passes `0.5F`; the projectiles and the display
        // entities leave the engine's default, which is not a number this mod sets.
        for row in RENDERERS {
            assert!(
                row.shadow == 0.0 || row.shadow == 0.5 || row.shadow == 1.0,
                "{}: {}",
                row.name,
                row.shadow
            );
            if row.layers.contains(&Layer::Armor) {
                assert_eq!(row.shadow, 0.5, "{}", row.name);
            }
        }
    }

    #[test]
    fn a_pirate_has_four_files_and_an_index_past_the_end_takes_the_last() {
        let pirate = renderer_for("minecolonies:pirate").expect("a row");
        assert_eq!(pirate.variants, 4);
        assert_eq!(pirate.texture.distinct(), Some(4));
        assert_eq!(
            pirate.texture.resolve(0),
            Some("textures/entity/raiders/pirate1.png")
        );
        assert_eq!(
            pirate.texture.resolve(3),
            Some("textures/entity/raiders/pirate4.png")
        );
        assert_eq!(
            pirate.texture.resolve(9),
            Some("textures/entity/raiders/pirate4.png"),
            "the switch default is the fourth file"
        );
    }

    #[test]
    fn a_norseman_has_three_ids_but_two_files_and_id_two_draws_the_first() {
        let archer = renderer_for("minecolonies:norsemenarcher").expect("a row");
        assert_eq!(archer.variants, 3, "nextInt(3)");
        assert_eq!(archer.texture.distinct(), Some(2));
        assert_eq!(
            archer.texture.resolve(0),
            Some("textures/entity/raiders/norsemen_archer1.png")
        );
        assert_eq!(
            archer.texture.resolve(1),
            Some("textures/entity/raiders/norsemen_archer2.png")
        );
        assert_eq!(
            archer.texture.resolve(2),
            Some("textures/entity/raiders/norsemen_archer1.png"),
            "the `== 1` test sends 2 back to the first file"
        );
    }

    #[test]
    fn only_the_four_families_read_a_texture_id() {
        let mut with_variants: Vec<&str> = RENDERERS
            .iter()
            .filter(|row| row.variants > 0)
            .map(|row| row.name)
            .collect();
        with_variants.sort_unstable();
        assert_eq!(
            with_variants,
            vec![
                "FireArrowRenderer",
                "RendererArcherNorsemen",
                "RendererArcherPirate",
                "RendererChiefPirate",
                "RendererDrownedArcherPirate",
                "RendererDrownedPirate",
                "RendererPirate",
                "RendererShieldmaidenNorsemen",
            ]
        );
        // …and the six-file arrow is the odd one: it cycles on `tickCount % 6`
        // (`FireArrowRenderer.java:41`), not on a random id.
        let arrow = renderer_for("minecolonies:firearrow").expect("a row");
        assert_eq!(arrow.variants, 6);
        assert_eq!(
            arrow.texture.resolve(5),
            Some("textures/item/magicalarrows/magical_arrow6.png")
        );
    }

    #[test]
    fn the_sitting_entity_draws_nothing_at_all() {
        let sitting = renderer_for("minecolonies:sittingentity").expect("a row");
        assert_eq!(sitting.model, Model::None);
        assert_eq!(sitting.texture, Texture::None);
        assert_eq!(sitting.texture.resolve(0), None);
        assert_eq!(sitting.texture.distinct(), Some(0));
    }

    #[test]
    fn the_mercenary_wears_the_settler_texture_the_citizen_builder_produces() {
        // The join between this table and `render::models`: the mercenary's file
        // is the settler's male texture 1, which the builder must be able to
        // produce, or the two tables disagree.
        let mercenary = renderer_for("minecolonies:mercenary").expect("a row");
        let built = crate::render::models::model_by_path("settler")
            .expect("the settler row")
            .texture(false, 0, crate::render::models::DEFAULT_SUFFIX, crate::render::models::DEFAULT_FOLDER)
            .expect("three textures");
        assert_eq!(mercenary.texture.resolve(0), Some(built.primary.as_str()));
    }

    #[test]
    fn the_spear_and_the_cavalry_horse_borrow_vanilla_models() {
        let spear = renderer_for("minecolonies:spear").expect("a row");
        assert_eq!(spear.model, Model::Vanilla("minecraft:trident#main"));
        let horse = renderer_for("minecolonies:cavalry_horse").expect("a row");
        assert_eq!(horse.model, Model::Vanilla("minecraft:horse#main"));
        assert_eq!(horse.layers.len(), 1);
        assert!(horse.layers.contains(&Layer::CavalryReadiness));
    }

    #[test]
    fn a_cavalry_horse_wears_one_of_six_overlays_from_its_cooldown() {
        // A ready horse (cooldown 0) is fully lit: the last file.
        assert_eq!(cavalry_segments(20.0, 0.0), 5);
        // A horse at its threshold wears nothing.
        assert_eq!(cavalry_segments(20.0, 20.0 * 0.66), 0);
        // Past the threshold the cooldown is clamped, not negative.
        assert_eq!(cavalry_segments(20.0, 100.0), 0);
        // A zero-health horse does not divide by zero.
        assert_eq!(cavalry_segments(0.0, 0.0), 5);
        assert_eq!(
            cavalry_texture(0),
            "textures/entity/horse/cavalry_overlay_layer0.png"
        );
        assert_eq!(
            cavalry_texture(CAVALRY_SEGMENTS),
            "textures/entity/horse/cavalry_overlay_layer5.png"
        );
        assert!((CAVALRY_ALPHA - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn the_readiness_segments_step_once_per_fifth() {
        // readiness = 1 - cooldown / (20 * 0.66) = 1 - cooldown / 13.2
        let mut seen = Vec::new();
        for step in 0..=20 {
            let cooldown = step as f32 * 0.66;
            let segment = cavalry_segments(20.0, cooldown);
            if seen.last() != Some(&segment) {
                seen.push(segment);
            }
        }
        assert_eq!(seen, vec![5, 4, 3, 2, 1, 0], "one segment per fifth, downward");
    }
}
