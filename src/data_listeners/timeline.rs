//! What a `/reload` runs, in which order, and the one step that comes too late.
//!
//! Java reference: `core/event/DataPackSyncEventHandler.java:24-40` — the class's
//! own doc comment writes the sequence down, which is rare enough to quote:
//!
//! ```text
//! On /reload:
//!  -- JsonReloadListeners, TagsUpdatedEvent [server], OnDatapackSyncEvent [server],
//!     TagsUpdatedEvent [remote client], RecipesUpdatedEvent [client]
//! ```
//!
//! So the nine listeners run **first**, all of them, before anything else in the
//! mod sees the new pack. Then [`Step::CompatLists`] rebuilds the three caches
//! that need vanilla's tags (`:52-57`), then [`Step::Send`] ships five packets in a
//! fixed order (`:68-73`).
//!
//! **The one rule this file exists for.** `CrafterRecipeListener.apply` stores
//! `recipe-template` files raw and parses them later
//! (`CustomRecipeManager.addRecipeTemplate`, `:128-136`), because a template needs
//! the item tags to resolve (`resolveTemplates` is called from
//! [`Step::CompatLists`], `:56`, and its own doc says "must be called
//! server-side-only after tags have been loaded", `:280-281`). Between the end of
//! the listener and that step, the recipe table is **incomplete**: every craft a
//! template describes is missing. The shipped pack has 2 such files out of 163,
//! so the window is small and real.
//!
//! **No clock, no host call**: the order is a value, [`STEPS`], and a test asserts
//! it — which is the only way an order can be checked without running a server.

use crate::events::{Bus, Topic};

use super::Listener;

/// One step of a `/reload`, server side, in the order `:24-40` states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the five steps the doc comment names, in its order
pub enum Step {
    /// Every `SimpleJsonResourceReloadListener`, i.e. the nine of [`super::LISTENERS`].
    /// Nothing in the mod has read the new pack before this finishes.
    Listeners,
    /// `TagsUpdatedEvent`: item and block tags are now the new pack's.
    Tags,
    /// `OnDatapackSyncEvent` with a null player, which is the reload branch
    /// (`:88-92`).
    PackSync,
    /// [`compat_lists`]: smelting recipes, compatibility caches, templates,
    /// loot data — the four things that could not be computed without tags.
    CompatLists,
    /// [`PACKETS`], one per listener that has a client copy.
    Send,
}

/// The five steps, in order.
pub const STEPS: &[Step] = &[
    Step::Listeners,
    Step::Tags,
    Step::PackSync,
    Step::CompatLists,
    Step::Send,
];

/// The four caches `discoverCompatLists` rebuilds, in its own order (`:52-57`).
///
/// Only the third is a listener's: the first two read vanilla, the fourth reads
/// loot tables. They are here because the template resolution sits between them,
/// and a reader who moved it would break the recipe table.
pub const COMPAT: &[&str] = &[
    "FurnaceRecipes.loadRecipes (:54)",
    "CompatibilityManager.discover (:55)",
    "CustomRecipeManager.resolveTemplates (:56)",
    "CustomRecipeManager.buildLootData (:57)",
];

/// The five packets `sendPackets` sends, in its order (`:68-73`).
///
/// Note which listeners are **not** in it: study items, recruitment costs and
/// visitor data have no client copy, which is why three of the nine carry
/// [`super::Sync::None`].
pub const PACKETS: &[&str] = &[
    "UpdateClientWithCompatibilityMessage (the NBT keys among it)",
    "CustomRecipeManager (the recipes)",
    "IGlobalResearchTree (the tree)",
    "QuestJsonListener (the raw quest JSON)",
    "DiseasesListener (the parsed diseases)",
];

/// Whether `early` runs before `late`. Total on [`Step`], so the comparison is the
/// order — no separate table can drift from [`STEPS`].
pub fn before(early: Step, late: Step) -> bool {
    early < late
}

/// The step at which a listener's table is complete, or `None` if it never is
/// before the pack is shipped.
///
/// A crafter recipe table is only complete at [`Step::CompatLists`]; every other
/// listener is complete when [`Step::Listeners`] returns. This is the function a
/// caller uses instead of guessing.
pub fn complete_at(listener: &Listener) -> Step {
    if listener.class == "CrafterRecipeListener" {
        Step::CompatLists
    } else {
        Step::Listeners
    }
}

/// Whether a table can be read between the end of the listeners and the end of a
/// reload without seeing an incomplete answer.
pub fn readable_immediately(listener: &Listener) -> bool {
    complete_at(listener) == Step::Listeners
}

/// What a reload posts on the colony bus: **nothing**.
///
/// Measured: no `apply` of the nine calls `EventBus.post`, and none of them holds a
/// bus reference. The nine are **pull**-based — they replace a table that callers
/// look up afterwards — and `src/events` is therefore not on their path at all.
/// This constant exists so that fact is a value a test asserts instead of a
/// sentence nobody checks.
pub const POSTED_ON_RELOAD: &[Topic] = &[];

/// The one bus event in this neighbourhood, and why it is **not** a reload event.
///
/// `CustomRecipesReloadedEvent` is posted on **Forge's** bus from
/// `CustomRecipeManager.handleCustomRecipeManagerMessage` (`:412`, the post is at
/// `:438`) — the handler that reads the recipe manager off the network on the
/// **client**. It fires when a client has received the table, not when a data pack
/// has been read, so wiring it to a reload would announce the wrong moment.
pub const LOOKS_LIKE_RELOAD: (Topic, &str) = (
    Topic::CustomRecipesReloaded,
    "CustomRecipeManager.java:438 — client packet handler (:412), not apply",
);

/// The bus an event rides, for a test that needs to say why a subscriber on the mod
/// bus would never hear it (`topic::Bus::Mod` vs `Forge`).
pub fn bus_of(topic: Topic) -> Bus {
    topic.bus()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_listeners::listener;

    #[test]
    fn the_listeners_run_first_and_the_packets_last() {
        assert_eq!(STEPS.first(), Some(&Step::Listeners));
        assert_eq!(STEPS.last(), Some(&Step::Send));
        assert!(before(Step::Listeners, Step::Tags));
        assert!(!before(Step::Send, Step::CompatLists));
    }

    #[test]
    fn the_template_resolution_happens_after_the_listeners() {
        // `DataPackSyncEventHandler.java:56` is inside `discoverCompatLists`, which
        // the reload branch calls at `:92`, after `JsonReloadListeners` (`:30-32`).
        assert!(before(Step::Listeners, Step::CompatLists));
        assert_eq!(COMPAT[2], "CustomRecipeManager.resolveTemplates (:56)");
    }

    #[test]
    fn only_the_recipe_table_is_incomplete_when_a_listener_returns() {
        let recipes = listener("CrafterRecipeListener").expect("declared");
        assert_eq!(complete_at(recipes), Step::CompatLists);
        assert!(!readable_immediately(recipes));
        for class in [
            "CitizenNameListener",
            "DiseasesListener",
            "QuestJsonListener",
            "ResearchListener",
        ] {
            let entry = listener(class).expect("declared");
            assert_eq!(complete_at(entry), Step::Listeners, "{class}");
            assert!(readable_immediately(entry), "{class}");
        }
    }

    #[test]
    fn the_three_unsynced_listeners_are_the_ones_absent_from_the_packets() {
        let unsynced: Vec<&str> = crate::data_listeners::LISTENERS
            .iter()
            .filter(|l| l.sync == crate::data_listeners::Sync::None)
            .map(|l| l.class)
            .collect();
        assert_eq!(
            unsynced,
            vec![
                "CustomVisitorListener",
                "RecruitmentItemsListener",
                "StudyItemListener"
            ]
        );
        for class in &unsynced {
            assert!(
                !PACKETS.iter().any(|packet| packet.starts_with(*class)),
                "{class} is not in the packet list"
            );
        }
        assert_eq!(PACKETS.len(), 5);
    }

    #[test]
    fn a_reload_posts_no_bus_event_and_the_one_nearby_event_is_a_client_packet() {
        // The nine `apply` bodies hold no bus: the table is pulled by its readers.
        assert!(POSTED_ON_RELOAD.is_empty());

        let (topic, why) = LOOKS_LIKE_RELOAD;
        assert_eq!(topic, Topic::CustomRecipesReloaded);
        // Forge's bus, because the class extends Forge's `Event`
        // (`CustomRecipesReloadedEvent.java:9`), not the mod's.
        assert_eq!(bus_of(topic), Bus::Forge);
        assert!(!topic.colony_scoped());
        assert!(why.contains(":438"));
    }
}
