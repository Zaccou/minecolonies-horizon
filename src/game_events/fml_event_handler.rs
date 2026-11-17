//! `FMLEventHandler.java`: the eight moments the mod hooks into the game's own life.
//!
//! Java reference: `FMLEventHandler.java`, 82 lines, 9 rows, all in
//! [`super::table`]. Seven of the eight are one delegation each
//! (`IColonyManager.onServerTick`, `onClientTick`, `onWorldTick`, a reload listener
//! list, three lifecycle callbacks); the eighth, `loadMissingColonies`, is the only
//! one that touches the save directory, and it is the only `Missing` row.
//!
//! **The order is the content.** `:45-57` registers nine reload listeners, and
//! `src/data_listeners` ports them **in the same order** because a listener may read
//! what the one before it wrote (`src/data_listeners/timeline.rs` says which reads
//! which). [`RELOAD_ORDER`] is that list, as Java writes it, so the two can be
//! compared without reading either.
//!
//! **Two lifecycle facts the port must not lose.** `onServerAboutToStart` resets the
//! recipe manager **before** any world exists (`:65-69`), so a world loaded from a
//! previous server does not inherit a half-filled table; and `onServerStopping`
//! shuts the pathfinding pool down (`:71-75`), which in the Java is a static because
//! the machine-wide search queue outlives the level.

/// The nine reload listeners of `FMLEventHandler.java:48-56`, in registration order.
pub const RELOAD_ORDER: &[&str] = &[
    "CrafterRecipeListener",
    "ResearchListener",
    "CustomVisitorListener",
    "CitizenNameListener",
    "QuestJsonListener",
    "ItemNbtListener",
    "StudyItemListener",
    "DiseasesListener",
    "RecruitmentItemsListener",
];

/// The four moments of a server's life the Java answers, in the order Forge fires
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `ServerAboutToStartEvent` (`:65-69`): reset the recipe manager.
    AboutToStart,
    /// `ServerStartedEvent` (`:77-81`): restore colonies whose file is missing.
    Started,
    /// `ServerStoppingEvent` (`:71-75`): shut the pathfinding pool down.
    Stopping,
}

impl Phase {
    /// What the Java does at this moment, in one sentence.
    pub fn action(self) -> &'static str {
        match self {
            Phase::AboutToStart => "IColonyManager.getRecipeManager().reset()",
            Phase::Started => "BackUpHelper.loadMissingColonies()",
            Phase::Stopping => "Pathfinding.shutdown()",
        }
    }

    /// The hook that carries this moment.
    pub fn hook(self) -> &'static str {
        match self {
            Phase::AboutToStart => "server_about_to_start",
            Phase::Started => "server_started",
            Phase::Stopping => "server_stopping",
        }
    }
}

/// The three phases, in the order a server runs them.
pub const PHASES: &[Phase] = &[Phase::AboutToStart, Phase::Started, Phase::Stopping];

/// The two per-side ticks `:22-32` delegate — and neither is a *level* tick: Java
/// forwards both phases unguarded, so the manager sees `START` and `END` and decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Server,
    Client,
}

impl Tick {
    /// The hook that carries this tick.
    pub fn hook(self) -> &'static str {
        match self {
            Tick::Server => "server_tick",
            Tick::Client => "client_tick_post",
        }
    }
}

/// `:59-63`: the level tick, both phases and both sides — the one delegation whose
/// hook in Horizon covers only the post phase (`level_tick_post`), which is the phase
/// the manager's work happens in.
pub const LEVEL_TICK_HOOK: &str = "level_tick_post";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nine_reload_listeners_are_in_the_javas_order() {
        assert_eq!(RELOAD_ORDER.len(), 9);
        assert_eq!(RELOAD_ORDER[0], "CrafterRecipeListener");
        assert_eq!(RELOAD_ORDER[8], "RecruitmentItemsListener");
        assert_eq!(
            RELOAD_ORDER[4], "QuestJsonListener",
            "the quests are read fifth, after names"
        );
    }

    #[test]
    fn no_reload_listener_is_registered_twice() {
        let mut sorted = RELOAD_ORDER.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len());
    }

    #[test]
    fn the_three_phases_are_the_order_a_server_runs_them() {
        assert_eq!(
            PHASES,
            &[Phase::AboutToStart, Phase::Started, Phase::Stopping]
        );
        assert_eq!(Phase::Started.action(), "BackUpHelper.loadMissingColonies()");
    }

    #[test]
    fn every_phase_names_a_hook_the_sdk_declares() {
        for phase in PHASES {
            assert!(super::super::table::hook_exists(phase.hook()));
        }
        assert!(super::super::table::hook_exists(Tick::Server.hook()));
        assert!(super::super::table::hook_exists(Tick::Client.hook()));
        assert!(super::super::table::hook_exists(LEVEL_TICK_HOOK));
    }

    #[test]
    fn the_recipe_reset_happens_before_the_world_exists() {
        // `:65-69` — AboutToStart, not Started: a table carried over from the last
        // server would otherwise survive the first `/reload`.
        assert_eq!(PHASES[0], Phase::AboutToStart);
        assert!(Phase::AboutToStart.action().contains("reset"));
    }
}
