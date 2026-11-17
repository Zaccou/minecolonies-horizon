//! The render data: which model each entity asks for, what state turns which part
//! on, what draws above a head, and which block entity draws what.
//!
//! Java reference: `core/client/render/` — **62 files**, 388 KB, ported here as
//! data and rules. Five of those files are `package-info.java`; the other 57 are
//! split by the question they answer:
//!
//! | File | Answers | From |
//! | --- | --- | --- |
//! | `models` | which of the 42 models a job wears, and its texture name | `IModelType`, `ModModelTypeInitializer`, the 50 `getModel()` |
//! | `states` | the 17 marks, the 10 part conditions, the arm poses | `CitizenModel`, `RenderUtils`, the `RENDER_META_*` |
//! | `above` | the icon over a head, the two item badges | `RenderBipedCitizen.renderNameTag`, `CitizenDataView`, the two decorators |
//! | `entities` | the 47 entity ids, their 27 renderers, textures and layers | `ClientRegistryHandler.doClientStuff` and the 18 raider renderers |
//! | `blocks` | the 7 block entities that draw, the 1 that does not, both text layouts | the 7 `TileEntity*Renderer` |
//! | `overlay` | the world overlays: borders, boxes, path nodes, highlight time | the 14 `worldevent/` files |
//!
//! **Not the GPU.** Horizon draws; this module decides *what* is drawn, *with
//! which file*, *when* and *where*. Every call that would need the graphics
//! pipeline — a `VertexConsumer`, a `RenderType`, a `VertexBuffer`, a
//! `PoseStack` — is in `MANQUANT.md` with what it was for. That file is the honest
//! half of the port: Java's rendering is 388 KB because half of it is OpenGL
//! plumbing that a wasm mod reaches through a different API entirely.
//!
//! # Four premises of the brief, checked against the source
//!
//! **1. "la table des entités du mod" — true, and bigger than one table.** 47
//! entity ids, 27 renderers, and 18 of the ids are the same mob twice (a raider and
//! its camp twin). A second table sits under it: the 42 citizen model types, which
//! a *job* chooses, not an entity ([`models::JOB_MODELS`]).
//!
//! **2. "les états d'animation … (marche, travail, sommeil, portage)" — three of
//! the four are the mod's, and walking is not.** There is no walk state in the
//! mod: the leg swing is vanilla's `walkAnimationPos`/`walkAnimationSpeed`, which
//! the engine fills for every living entity
//! (`crates/horizon-mod-vocab/src/client/entity_renderers.rs:52-56`). What the mod
//! owns is *travail* (`working` plus 16 marks), *sommeil* (`Pose.SLEEPING`, which
//! hides a hat, a straw, a seed box and a satchel) and *portage* (logs, backpack,
//! carrots, bucket — and the courier's 10° lean).
//!
//! **3. "les couleurs et variantes" — true, and five unrelated rules wear that
//! name**: a colony's team colour on its border, a scarecrow's two heads, a
//! raider's `textureId` (three different index rules), a cavalry horse's six
//! readiness overlays, and one text colour that is not the grey it looks like.
//!
//! **4. "où se dessine un élément au-dessus d'une entité (nom, barre, icône)" — il
//! n'y a pas de barre.** Nothing in `core/client/render/` draws a health bar, a
//! progress bar or a boss bar over a citizen (grep for `BossBar`/`HealthHud` over
//! the client tree: no hit). Above a head there are exactly two things: the
//! **vanilla** name tag, whose anchor is the engine's `getNameTagOffsetY()`
//! (never overridden by the mod — grep: one call site, no declaration), and the
//! mod's 10×10 status icon at that anchor + 0.3 ([`above`]).
//!
//! # What the port cannot follow, in one line each
//!
//! The detail is in `MANQUANT.md`; these are the four that change a design:
//!
//! - **entity textures are not atlas sprites.** A citizen's skin is drawn with
//!   `RenderType.entityCutoutNoCull` from a standalone PNG; Horizon's draw helpers
//!   take an atlas sprite ([`models`] gives the two paths, `MANQUANT` §1 gives the
//!   gap).
//! - **`riding` and `attackTime` have no field** in `LivingRenderState`, so a
//!   citizen on a horse, or swinging, cannot be posed as Java poses it (§3).
//! - **`getNameTagOffsetY` is the engine's**, so the icon's y is an input the port
//!   takes rather than a value it computes (§2).
//! - **the seam is `lib.rs`**: `register_entity_renderers`, `render_name_tag` and
//!   the block-entity submit hooks all exist in this build, and none of them can be
//!   subscribed from `src/render` alone (§5).

pub mod above;
pub mod blocks;
pub mod entities;
pub mod models;
pub mod overlay;
pub mod states;

pub use above::{
    ABOVE_NAMETAG, Above, BLOCKING_ICON, Badge, HALF_WIDTH, HEIGHT, MAX_DISTANCE_SQ, PENDING_ICON,
    SCALE, above as status_above, badge_shown, clipboard_badge, clipboard_count, icon_anchor_y,
    icon_corners, map_badge, visible, within_icon_range,
};
pub use blocks::{
    BLOCK_RENDERERS, BlockRenderer, EMPTY, GRAVE_LAYOUT, SIGN_FACES, SIGN_LAYOUT, ScarecrowHead,
    SignRow, SignText, TextLayout, UNKNOWN_CITIZEN, UNKNOWN_COLONY, grave_rows, scarecrow_head,
    scarecrow_texture, sign_rows, text_channels, truncate,
};
pub use entities::{
    COMBAT_READINESS_THRESHOLD, Layer, Model, RENDERERS, Renderer, Texture, cavalry_segments,
    cavalry_texture, renderer_for,
};
pub use models::{
    CITIZEN_PATH, DEFAULT_FOLDER, DEFAULT_SUFFIX, FALLBACK_JOBS, HALLOWEEN_STYLE, JOB_MODELS,
    JobModel, MODEL_TYPES, MULTI_TEXTURE, ModelType, Part, Refusal, StyleCache, TexturePath,
    april_first, april_hidden_part, citizen_model, halloween_window, job_model_row, model_by_path,
    model_for, style_folder,
};
pub use overlay::{
    ANCHOR_BOX, BLUEPRINT_RULES, BUILDER_BOX, BlueprintRule, OWN_COLOUR, OTHER_COLOUR, PathNode,
    WORK_ORDER_BOX, border_colour, border_corners, border_range, border_sides, border_step,
    halved, highlight_expired, highlight_ticks,
};
pub use states::{
    ArmPose, CARRY_LEAN, Condition, Ctx, Hand, HatInputs, MODEL_INPUTS, MARKS, Mark, SHOWN, Shown,
    UseAnim, WORKING, arm_pose, body_rotation, display_hat, effective_hand, is_working,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The test the mission names: **every entity of the mod is covered, or named
    /// as not drawn**, with the reason.
    #[test]
    fn every_entity_of_the_mod_is_drawn_or_named_as_nothing() {
        let ids = entities::entities();
        assert_eq!(ids.len(), 47, "the 47 registry paths of EntityInitializer");

        let mut undrawn = Vec::new();
        for id in &ids {
            let row = renderer_for(id).unwrap_or_else(|| panic!("{id} has no renderer"));
            assert!(!row.name.is_empty(), "{id}");
            if row.model == Model::None || row.texture == Texture::None {
                undrawn.push((*id, row.name));
            }
        }
        undrawn.sort_unstable();
        // The entities whose drawing is not a model: the fish hook (a quad and a
        // line), the sitting entity (nothing at all), and the three that borrow a
        // vanilla renderer whole.
        assert_eq!(
            undrawn,
            vec![
                ("minecolonies:cavalry_horse", "HorseRenderer"),
                ("minecolonies:druidpotion", "ThrownItemRenderer"),
                ("minecolonies:fishhook", "RenderFishHook"),
                ("minecolonies:mcminecart", "MinecartRenderer"),
                ("minecolonies:mcnormalarrow", "TippableArrowRenderer"),
                ("minecolonies:sittingentity", "RenderSitting"),
            ]
        );
    }

    #[test]
    fn every_block_entity_of_the_mod_is_drawn_or_named_as_empty() {
        assert_eq!(BLOCK_RENDERERS.len(), 7);
        let empty: Vec<&str> = BLOCK_RENDERERS
            .iter()
            .filter(|row| row.draws.starts_with("nothing"))
            .map(|row| row.block)
            .collect();
        assert_eq!(empty, vec![EMPTY]);
    }

    #[test]
    fn every_citizen_model_a_job_can_wear_exists() {
        // 42 model types, 46 job overrides, and no job pointing at a model the
        // table does not declare.
        assert_eq!(MODEL_TYPES.len(), 42);
        for job in crate::jobs::Job::ALL {
            let model = model_for(*job).unwrap_or_else(|err| panic!("{}: {err}", job.path()));
            assert!(
                model_by_path(model.path).is_some(),
                "{} resolved to a row outside the table",
                job.path()
            );
        }
    }

    #[test]
    fn the_three_tables_do_not_share_a_name_by_accident() {
        // A model path, an entity id and a block entity id are three different
        // namespaces; a copy-paste between the tables would show here.
        for row in MODEL_TYPES {
            assert!(!row.path.contains(':'), "{} is not a bare path", row.path);
            assert!(!row.path.starts_with("minecolonies"));
        }
        for row in entities::RENDERERS {
            for id in row.entities {
                assert!(id.starts_with("minecolonies:"), "{id}");
                assert_ne!(*id, row.name, "{} names itself", row.name);
            }
        }
        for row in BLOCK_RENDERERS {
            assert!(row.block.starts_with("minecolonies:"), "{}", row.block);
        }
    }

    #[test]
    fn the_counts_of_the_module_doc_are_the_counts_of_the_code() {
        // The doc claims 42 / 47 / 27 / 7 / 17 / 16; every one is a table here,
        // so the prose cannot drift away from the data unnoticed.
        assert_eq!(MODEL_TYPES.len(), 42);
        assert_eq!(entities::RENDERERS.len(), 27);
        assert_eq!(entities::entities().len(), 47);
        assert_eq!(BLOCK_RENDERERS.len(), 7);
        assert_eq!(MARKS.len(), 17);
        assert_eq!(JOB_MODELS.len(), 46);
        assert_eq!(MULTI_TEXTURE.len(), 6);
    }

    #[test]
    fn nothing_in_this_module_calls_a_host_or_opens_a_file() {
        // A grep over this module's own sources: the port's rule is that the
        // rules are testable with no game. `std::` is allowed for strings and
        // sets; `mod_api::` is not.
        let sources = [
            include_str!("mod.rs"),
            include_str!("models.rs"),
            include_str!("states.rs"),
            include_str!("above.rs"),
            include_str!("entities.rs"),
            include_str!("blocks.rs"),
            include_str!("overlay.rs"),
        ];
        for source in sources {
            // Skip this test's own body: it mentions the two strings it forbids.
            let rule = source.split("fn nothing_in_this_module_calls_a_host").next().unwrap_or("");
            assert!(!rule.contains("mod_api::"), "a host call in the rules");
            assert!(!rule.contains("std::fs"), "a file opened in the rules");
            assert!(!rule.contains("std::net"), "a socket in the rules");
        }
    }
}
