//! `DataPackSyncEventHandler.java`: what a data pack reload rebuilds, and who is told.
//!
//! Java reference: `DataPackSyncEventHandler.java`, 156 lines, 7 branches, all in
//! [`super::table`]. The class is two nested subscriber classes, and its own header
//! (`:28-38`) writes the event order down, because the order is the only way to know
//! when it is safe to rebuild a table:
//!
//! ```text
//! Single player, startup:
//!   JsonReloadListeners, TagsUpdatedEvent, FMLServerAboutToStart,
//!   FMLServerStarted, OnDatapackSyncEvent, RecipesUpdatedEvent
//! Dedicated server, startup:
//!   JsonReloadListeners, TagsUpdatedEvent, FMLServerAboutToStart, FMLServerStarted
//! Remote client, login:
//!   OnDatapackSyncEvent [server], PlayerLoggedInEvent [server],
//!   RecipesUpdatedEvent [client], TagsUpdatedEvent [client]
//! /reload:
//!   JsonReloadListeners, TagsUpdatedEvent [server], OnDatapackSyncEvent [server],
//!   TagsUpdatedEvent [remote client], RecipesUpdatedEvent [client]
//! ```
//!
//! [`SEQUENCES`] is that table, kept as data, and the one fact a port needs from it is
//! **where the smelting recipes become readable**: `onServerStarted` (`:117-128`)
//! exists only because `FurnaceRecipes` needs them and they are not there at
//! `OnDatapackSyncEvent` on a dedicated server's first start.
//!
//! **The owner test is an identity test.** `:99` and `:105` compare `GameProfile`s
//! with `!=`, which on an object is `!equals` only because `GameProfile` does not
//! override `equals`… it does — but the comparison here is `!=` on the reference, so
//! in single player the *same instance* is skipped and any other profile is not.
//! [`sends_to`] keeps that shape: an argument named `same_instance`, not `is_owner`.
//!
//! **What the port cannot do.** The reload half (`:90-104`) rebuilds four tables and
//! then **sends five payloads to every online player**. Horizon's `datapack_sync`
//! fires per joining player, collects *recipe type names* and pushes nothing; it is
//! never asked with nobody online. So the rebuild hangs on `tags_updated` (same
//! moment, `ServerDataLoad`), and the send has no hook — filed `Missing` in
//! [`super::table`] with `PlayerRef::send_payload` named as the primitive the port
//! would need to be *called at that moment*.

/// The four rebuilds `discoverCompatLists` runs, in order (`:52-58`).
pub const DISCOVER_ORDER: &[&str] = &[
    "FurnaceRecipes.loadRecipes(recipeManager, overworld)",
    "ColonyManager.getCompatibilityManager().discover(recipeManager, overworld)",
    "CustomRecipeManager.resolveTemplates()",
    "CustomRecipeManager.buildLootData(lootData, overworld)",
];

/// The five payloads `sendPackets` sends, in order (`:66-74`).
pub const SEND_ORDER: &[&str] = &[
    "UpdateClientWithCompatibilityMessage",
    "CustomRecipeManager.sendCustomRecipeManagerPackets",
    "IGlobalResearchTree.sendGlobalResearchTreePackets",
    "QuestJsonListener.sendGlobalQuestPackets",
    "DiseasesListener.sendGlobalDiseasesPackets",
];

/// One of the four orders the class documents (`:28-38`).
#[derive(Clone, Copy, Debug)]
pub struct Sequence {
    /// When it runs, as the comment names it.
    pub when: &'static str,
    /// The events, in order.
    pub order: &'static [&'static str],
}

/// The class's own event-order table, kept as data.
pub const SEQUENCES: &[Sequence] = &[
    Sequence {
        when: "single player, startup",
        order: &[
            "JsonReloadListeners",
            "TagsUpdatedEvent",
            "FMLServerAboutToStart",
            "FMLServerStarted",
            "OnDatapackSyncEvent",
            "RecipesUpdatedEvent",
        ],
    },
    Sequence {
        when: "dedicated server, startup",
        order: &[
            "JsonReloadListeners",
            "TagsUpdatedEvent",
            "FMLServerAboutToStart",
            "FMLServerStarted",
        ],
    },
    Sequence {
        when: "remote client, login",
        order: &[
            "OnDatapackSyncEvent [server]",
            "PlayerLoggedInEvent [server]",
            "RecipesUpdatedEvent [client]",
            "TagsUpdatedEvent [client]",
        ],
    },
    Sequence {
        when: "/reload",
        order: &[
            "JsonReloadListeners",
            "TagsUpdatedEvent [server]",
            "OnDatapackSyncEvent [server]",
            "TagsUpdatedEvent [remote client]",
            "RecipesUpdatedEvent [client]",
        ],
    },
];

/// Whether the joining player gets the five payloads (`:99`, `:105-108`).
///
/// `same_instance` is Java's `getGameProfile() != owner` in the negative: the
/// single-player owner is skipped, and on a dedicated server there is no owner at
/// all (`getSingleplayerProfile()` answers `null`, and no profile equals it).
pub fn sends_to(same_instance_as_owner: bool) -> bool {
    !same_instance_as_owner
}

/// Whether the crafting-tag audit runs (`:110-114`).
///
/// Three terms: the config switch, and then "a reload **or** nobody online" — the
/// audit walks every recipe and is only worth doing when no player is paying for the
/// tick.
pub fn audit_wanted(config_on: bool, reload: bool, players_online: usize) -> bool {
    config_on && (reload || players_online == 0)
}

/// Which half of `onDataPackSync` a call is: the reload (`event.getPlayer() == null`)
/// or one player's join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// `player == null`: rebuild, then tell everybody.
    Reload,
    /// `player != null`: tell that one player.
    Join,
}

/// `:90-108`, as one decision: what a call does beyond sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Work {
    /// Rebuild the four tables (`:93`).
    pub discover: bool,
    /// Send to every online player (`:97-103`).
    pub to_everyone: bool,
    /// Send to the one player (`:105-108`).
    pub to_that_player: bool,
}

/// `:90-108`: the two shapes of a call.
pub fn work_of(call: Call) -> Work {
    match call {
        Call::Reload => Work {
            discover: true,
            to_everyone: true,
            to_that_player: false,
        },
        Call::Join => Work {
            discover: false,
            to_everyone: false,
            to_that_player: true,
        },
    }
}

/// The client half's skip rule (`:144-151`): the single-player owner's own client
/// already has the server's recipes, so only a remote client reloads them.
pub fn client_reloads(singleplayer_owner: bool) -> bool {
    !singleplayer_owner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rebuild_is_four_steps_and_the_order_matters() {
        assert_eq!(DISCOVER_ORDER.len(), 4);
        assert!(
            DISCOVER_ORDER[0].starts_with("FurnaceRecipes"),
            "the smelting table is read first"
        );
        assert!(
            DISCOVER_ORDER[3].starts_with("CustomRecipeManager.buildLootData"),
            "loot data needs the templates resolved before it"
        );
    }

    #[test]
    fn the_five_payloads_are_sent_in_this_order() {
        assert_eq!(SEND_ORDER.len(), 5);
        assert_eq!(SEND_ORDER[0], "UpdateClientWithCompatibilityMessage");
        assert_eq!(SEND_ORDER[4], "DiseasesListener.sendGlobalDiseasesPackets");
    }

    #[test]
    fn the_class_documents_four_event_orders() {
        assert_eq!(SEQUENCES.len(), 4);
        let reload = SEQUENCES
            .iter()
            .find(|sequence| sequence.when == "/reload")
            .expect("the reload row");
        assert_eq!(reload.order.len(), 5);
        assert_eq!(
            reload.order[2], "OnDatapackSyncEvent [server]",
            "the sync comes after the server's own tags and before the client's"
        );
    }

    #[test]
    fn the_dedicated_start_has_no_recipes_updated_event() {
        let dedicated = SEQUENCES
            .iter()
            .find(|sequence| sequence.when == "dedicated server, startup")
            .expect("the dedicated row");
        assert!(!dedicated
            .order
            .iter()
            .any(|event| *event == "RecipesUpdatedEvent"));
        assert!(
            dedicated.order.contains(&"FMLServerStarted"),
            "which is why `onServerStarted` reloads the compat lists (`:117-128`)"
        );
    }

    #[test]
    fn the_single_player_owner_is_skipped_by_identity() {
        assert!(!sends_to(true));
        assert!(sends_to(false));
    }

    #[test]
    fn the_audit_runs_on_a_reload_or_with_nobody_online() {
        assert!(audit_wanted(true, true, 4));
        assert!(audit_wanted(true, false, 0));
        assert!(!audit_wanted(true, false, 1));
        assert!(!audit_wanted(false, true, 0));
    }

    #[test]
    fn a_reload_rebuilds_and_broadcasts() {
        let work = work_of(Call::Reload);
        assert!(work.discover && work.to_everyone && !work.to_that_player);
    }

    #[test]
    fn a_join_sends_to_that_player_only_and_rebuilds_nothing() {
        let work = work_of(Call::Join);
        assert!(!work.discover && !work.to_everyone && work.to_that_player);
    }

    #[test]
    fn a_remote_client_reloads_its_smelting_table() {
        assert!(client_reloads(false));
        assert!(!client_reloads(true));
    }
}
