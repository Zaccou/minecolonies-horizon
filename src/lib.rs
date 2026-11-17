//! `ported_minecolonies` — the first slice of the MineColonies port, in code.
//!
//! **One sentence.** A hut block that is placed, keeps its data, answers a
//! right click, and a citizen that appears and walks.
//!
//! Five hooks (`block_entity`, `right_click_block`, `register_commands`,
//! `run_mod_command`, `entity_join_level`), **no tick hook at all**, and no new
//! API surface: every method this crate calls is declared in
//! `crates/horizon-mod-vocab/src/host/`. Nothing here is written to
//! `crates/`, and `cargo horizon-mod sdk diff` must stay empty.
//!
//! **The hooks are thin on purpose.** They decide *when*; the modules decide
//! *what*. `wiring` is the one place that reads a colony out of the world's data
//! and writes it back — `/colony …` (`commands`), the click's summary
//! (`commands::reply`) and the hire's count (`colony::Colony`) all go through
//! it, so there is one answer to "where does the colony live" (`src/wiring.rs`,
//! *Where a colony lives*).
//!
//! The mod is **wasm** (`cargo horizon-mod build --wasm`), which is why the hut
//! keeps its state in the block entity's saved compound instead of a
//! `BlockEntityPartState`: a state without a codec is refused for a mod the game
//! loaded as wasm (`docs/modding/src/concepts/parts/block-entity-types.md:95-97`).
//!
//! Three things this slice does **not** do, each with the gap it costs:
//! no button in the menu (§3 **M1**), no slot layout (§3 **M2**), and no
//! renderer of its own: the citizen is a `minecraft:wandering_trader` wearing
//! this mod's type, so it is tracked and drawn as one.

pub mod advancements;
pub mod ai_work_01;
pub mod ai_work_02;
pub mod ai_work_03;
pub mod ai_work_04;
pub mod ai_work_05;
pub mod ai_work_06;
pub mod ai_work_07;
pub mod ai_work_08;
pub mod ai_work_09;
pub mod ai_work_10;
pub mod ai_work_11;
pub mod ai_work_12;
pub mod ai_build;
pub mod ai_gather;
pub mod ai;
pub mod base;
pub mod blueprint;
pub mod blocks;
pub mod building;
pub mod citizen;
pub mod citizens;
pub mod channel;
pub mod colony;
pub mod colony_world;
pub mod compat;
pub mod commands;
// `crafting` : une erreur de type a trancher (crafter.rs:503 / register.rs:37). Non declare tant
// qu elle n est pas resolue, pour que le depot compile. Le code est dans src/crafting/.
pub mod crafting;
pub mod entity;
pub mod events;
pub mod founding;
pub mod harness;
pub mod hire;
pub mod hut;
pub mod items;
pub mod inventory;
pub mod jobs;
pub mod manager_01;
pub mod manager_02;
pub mod manager_03;
pub mod manager_04;
pub mod manager_05;
pub mod manager_06;
pub mod manager_07;
pub mod manager_08;
pub mod manager_09;
pub mod manager;
pub mod menu;
pub mod network;
pub mod persistence;
pub mod tile_entities;
pub mod pathfinding;
pub mod research;
pub mod recipes_content;
pub mod quests;
pub mod requests;
pub mod placement;
pub mod structures;
pub mod render;
pub mod worldgen;
pub mod data_listeners;
pub mod widgets;
pub mod ui_window_01;
pub mod ui_window_02;
pub mod ui_window_03;
pub mod ui_window_04;
pub mod ui_window_05;
pub mod ui_window_06;
pub mod ui_window_07;
pub mod ui_window_08;
pub mod ui_window_09;
pub mod ui_window_10;
pub mod ui_window_11;
pub mod ui_window_12;
pub mod ui_window_13;
pub mod ui_window_14;
pub mod ui_window_15;
pub mod ui_window_16;
pub mod ui_window_17;
pub mod ui;
pub mod client_gui;
pub mod client_model;
pub mod generation;
pub mod game_events;
pub mod util;
pub mod compatibility;
pub mod colony_buildings;
pub mod network_messages;
pub mod entity_ai;
pub mod entity_pathfinding;
pub mod colony_requests;
pub mod wiring;
#[cfg(test)]
mod tests;

use mod_api::{
    EntityJoinLevelEvent, Flow, Hooks, HostSide, RegisterCommandsEvent, RegistrationContext,
    Registrar, RightClickBlockEvent, RunModCommandEvent, ServerHost,
};

/// The mod id of the package this code ships in (`horizon-mod.toml`).
pub const MOD_ID: &str = "ported_minecolonies";

/// The mod `#[horizon_mod]` subscribes to exactly the hooks it implements.
#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declarations, once: the block entity type and the block that carries it,
    /// its `BlockEntityPart`, and the block's container with the menu it opens.
    ///
    /// Both names are **full registry names**: a bare name would be qualified
    /// with the mod id (`ported_minecolonies`), not with the `minecolonies`
    /// namespace this package declares
    /// (`crates/horizon-mod-vocab/src/parts/owned_namespaces.rs:14-19`); the
    /// block itself is declared by
    /// `data/minecolonies/horizon/block/blockhutbuilder.json`.
    fn register(&self, registrar: &mut dyn Registrar, _cx: &RegistrationContext) {
        let kind = registrar
            .block_entity_type(hut::TYPE_ID)
            .blocks(&[hut::BLOCK_ID])
            .build();
        registrar
            .block_entity_part(&kind, hut::Hut)
            .expect("a mod's own block entity type takes its Part");
        let block = registrar.block_key(hut::BLOCK_ID);
        registrar.block_container(&block, hut::container());
    }

    /// The click: a status line, the colony's summary when the hut knows one,
    /// and `Continue` so the engine's own menu opens.
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        menu::right_click_block(host, event)
    }

    /// `/mcproto hire <pos>` and `/colony …`: the two roots this slice answers
    /// without a screen (a `menu_button` has nothing in content to trigger it,
    /// §3 M1). `/colony` comes from the one table `commands::SPECS` holds, so an
    /// alias and its subcommand cannot drift apart.
    fn register_commands(&self, host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        hire::register_commands(host, event);
        wiring::register_commands(host, event);
    }

    /// A parsed mod command, after permission checks and argument resolution.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        // `hire` first: it claims one word of `/mcproto`, and answers `Continue`
        // for everything else, which is what leaves `/colony` to the wiring.
        match hire::run(host, event) {
            Flow::Continue => wiring::run(host, event),
            answered => answered,
        }
    }

    /// The only window in which a citizen can be disarmed: it is already in the
    /// level's lists here, and its own tick has not started
    /// (`crates/horizon-mod-vocab/src/hooks/entity_join_level.rs:20-32`).
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        citizen::join(host, event)
    }
}
