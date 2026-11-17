//! The event listeners: **what each one hears, on which side, and what it refuses**.
//!
//! Java reference: `core/event/` — **2 951** lines over 14 files, thirteen with code.
//! `EventHandler` (876) is the server half on Forge's bus, `ClientEventHandler` (506)
//! the client half, and the other eleven are small: two load-order classes, two client
//! reload listeners, three capability providers, a quest registry, an event class, a
//! client registry of models and renderers, and a data generator that never runs in a
//! game.
//!
//! **The shape of this module is a table plus seven decisions.** [`table`] holds one row
//! per branch of the Java — the event, the side, the rule, what it refuses, and where
//! the port keeps it — so a reader can answer "what does this listener do" without
//! reading 2 951 lines of Java or ≈ 4 800 of Rust. What cannot be a row is a *decision*,
//! and those are functions:
//!
//! | File | The question it answers | Java |
//! | --- | --- | --- |
//! | [`table`] | every branch: event, side, rule, refusal, carrier | all fourteen files |
//! | [`event_handler`] | crop lumber, the join refusal, hut placement, beds, the chunk gate, spawns, tick slowness | `EventHandler.java` |
//! | [`client_event_handler`] | tooltips (dyed, food, research, building), the F3 line, the browse click, stale copies | `ClientEventHandler.java` |
//! | [`quest_objectives`] | the five quest registries, and the `break` three of them take | `QuestObjectiveEventHandler.java` |
//! | [`colony_story`] | the four story tables, the biome OR-tree, and what a bad file costs | `ColonyStoryListener.java` |
//! | [`texture_reload`] | which folder names a citizen texture pack | `TextureReloadListener.java` |
//! | [`client_registry`] | 94 model layer ids, 47 renderers, 7 block entity renderers, two item properties | `ClientRegistryHandler.java` |
//! | [`fml_event_handler`] | the reload order and the three lifecycle moments | `FMLEventHandler.java` |
//! | [`data_pack_sync`] | what a reload rebuilds, who is told, and the four documented event orders | `DataPackSyncEventHandler.java` |
//! | [`gather_data`] | 41 data generators that run before the jar, never in a game | `GatherDataHandler.java` |
//! | [`client_chunk_updated`] | the claim notification and the queue that defers it | `ClientChunkUpdatedEvent.java` |
//! | [`capability_providers`] | three capabilities, and the one primitive that replaces them | `capabilityproviders/` |
//!
//! **No hook body, and that is a decision, not an omission.** `src/lib.rs` is where the
//! one `impl Hooks` lives and the orchestrator owns it; every function here takes values
//! and answers a verdict, so `lib.rs` can hand it a hook's fields without this module
//! ever calling the host. Three consequences, all intended:
//!
//! - a rule can be tested with no level, no colony and no wasm — the module's 163 tests
//!   do exactly that;
//! - a rule that needs a value the API cannot give is a **visible** gap (`Carrier::Missing`
//!   in [`table`]) instead of a hook that compiles and answers the wrong thing;
//! - the same decision can be reached from two hooks (Java reaches hut permission from
//!   both the block and the event) without being written twice.
//!
//! **Where the port already had the rule.** Six rows are [`table::Carrier::Ported`]:
//! the hut click and its placement ladder are `src/blocks`, the commands are
//! `src/commands`, the nine data listeners are `src/data_listeners` in the same order,
//! and the chunk claim record is `src/network::common::ChunkCap`. Those rows add no
//! code on purpose: two copies of a permission rule is how a mod ends up refusing a
//! click it used to accept.
//!
//! **Three findings that are the reason to port an event layer at all**, each with its
//! source in [`table`] and its consequence in `MANQUANT.md`:
//!
//! 1. `farmland_trample` is handed a `LoopContext`, not a `Host`: the rule needs the
//!    citizen's job and the colony's research, so it cannot be read, let alone ported.
//! 2. `sound_play` is upstream notes in this SDK, not a declared hook, so muting the
//!    citizen voices — one `if` in Java — has nowhere to live.
//! 3. The chunk-claim branch tests `-1`, a sentinel no writer produces, and the logout
//!    branch cleans its position map **inside** a loop over all colonies.
//!
//! **One premise I had to correct while porting.** I first filed the tick slowness
//! (`EventHandler.java:864-875`) as a gap, because `ServerTickEvent` carries only a
//! `LifecycleContext`. It is not a gap: `ServerHost::tick_times()` answers the same
//! hundred nanosecond durations `getTickTimesNanos` keeps, indexed the same way
//! (`host/config.rs:96-102`), and [`event_handler::tick_duration_ms`] reads them. A
//! `Missing` row that stays missing only because nobody looked is the worst kind.
//!
//! None of the three is worked around. A port that muted sounds by not playing them, or
//! that guessed a sentinel, would be a mod that looks finished and is not.

pub mod capability_providers;
pub mod client_chunk_updated;
pub mod client_event_handler;
pub mod client_registry;
pub mod colony_story;
pub mod data_pack_sync;
pub mod event_handler;
pub mod fml_event_handler;
pub mod gather_data;
pub mod quest_objectives;
pub mod table;
pub mod texture_reload;

pub use capability_providers::{Capability, Holder};
pub use client_event_handler::{Colour, DebugLine, FoodLine, Line};
pub use client_registry::LAYERS;
pub use colony_story::{Filter, Kind as StoryKind, Refusal as StoryRefusal, Stories, StoryText};
pub use data_pack_sync::{Call, Sequence};
pub use event_handler::{
    bed_blockers, bed_head, claim_effects, crop_chance, crop_drop_chance, crop_ladder,
    hut_placement, join_verdict, slowness_factor, spawn_denied, tick_duration_ms, BedHalf,
    ClaimEffects, Hoe, Join, JoinKind, JoinRefusal, Notice, Placement, Verdict,
};
pub use fml_event_handler::{Phase, RELOAD_ORDER};
pub use quest_objectives::{Dispatch, Hold, Owner, QuestMap, Registry};
pub use table::{counts, rows_of, without_hook, Carrier, Counts, Kind, Listener, Row, Side, ROWS};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_are_the_ones_the_module_header_describes() {
        let counts = counts();
        // The header says "six rows are Ported"; the number is measured here so a row
        // that changes carrier cannot leave the prose behind.
        assert_eq!(counts.ported, 6, "the ported rows");
        assert_eq!(counts.superseded, 1, "the one the port's design removes");
        assert_eq!(
            counts.ported
                + counts.here
                + counts.hook
                + counts.missing
                + counts.superseded,
            counts.rows
        );
        assert!(counts.rows >= 100, "one row per branch: {}", counts.rows);
    }

    #[test]
    fn every_listener_of_the_brief_is_in_the_table() {
        assert_eq!(Listener::ALL.len(), 13);
        for listener in Listener::ALL {
            assert!(
                rows_of(*listener).next().is_some(),
                "{} has no row",
                listener.java_file()
            );
        }
    }

    #[test]
    fn the_module_exports_the_vocabulary_a_caller_needs() {
        // A compile-time check with a body: if a re-export disappears, this fails to
        // build rather than failing at a call site in `lib.rs`.
        let _: fn(bool) -> f32 = crop_chance;
        let _: fn(f64) -> f64 = slowness_factor;
        let _: fn(&[u64], u64) -> f64 = tick_duration_ms;
        let _: Kind = Carrier::missing("x").kind();
        let _: Side = Side::Server;
        let _: Listener = Listener::EventHandler;
        let _: &[Row] = ROWS;
        let _: &[Capability] = Capability::ALL;
        let _: &[&str] = LAYERS;
        let _: &[&str] = RELOAD_ORDER;
    }

    #[test]
    fn the_two_halves_are_both_present() {
        let server = rows_of(Listener::EventHandler).count();
        let client = rows_of(Listener::ClientEventHandler).count();
        assert!(server >= 30, "the server half: {server}");
        assert!(client >= 15, "the client half: {client}");
    }

    #[test]
    fn the_missing_rows_are_a_minority_of_the_hooks_that_exist() {
        let counts = counts();
        assert!(
            counts.hook > counts.missing,
            "hooks {} vs gaps {}",
            counts.hook,
            counts.missing
        );
        assert_eq!(without_hook().count(), counts.missing);
    }
}
