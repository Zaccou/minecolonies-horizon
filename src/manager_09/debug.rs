//! Who has MineColonies' debug overlays on: a set of player ids, a build flag,
//! and the one entry a toggle-off also forgets.
//!
//! Java reference: `core/debug/DebugPlayerManager.java` (70 lines) — three static
//! methods over two statics — and its five callers:
//! `EntityCitizen.java:407` (the path renderer),
//! `AbstractWindowCitizen.java:61` (the debug tab),
//! `QueryCitizenAIHistoryMessage.java:54` and
//! `DebugEnablePathfindingMessage.java:59` (two server-side messages that only
//! answer a debug-enabled player), and `CommandToggleDebug.java:38`, the command.
//!
//! **One rule makes this class stranger than it looks: the flag is an `or`.**
//! `hasDebugEnabled` is `!FMLEnvironment.production || debugPlayers.contains(uuid)`
//! (`:29`), so in a development run **every** player has debug, and the set cannot
//! take it away. A toggle-off in dev still removes the id and still answers
//! `false` (`:40-44`) — it just changes nothing a caller can observe through
//! `hasDebugEnabled`. The port keeps both halves and pins the confusion in a test.
//!
//! **The two mutators are not the same operation.**
//!
//! - `toggleDebugModeFor` (`:38-51`) removes **and** forgets the player's
//!   path-tracking entry (`PathfindingUtils.trackingMap.remove(player)`, `:43`),
//!   then answers the new state — `false` for a disable, `true` for an enable.
//! - `setDebugModeFor` (`:59-69`) only adds or removes the id. It never touches
//!   the tracking map, so a `set(player, false)` leaves a stale tracking entry
//!   where a `toggle` would have cleaned it. That asymmetry is in Java, is
//!   reachable (`DebugEnableMessage.java:54` calls `set`, the command calls
//!   `toggle`), and is a test below.
//!
//! **The tracking map has no writer in the shipped source.** `trackingMap` is
//! declared once (`PathfindingUtils.java:51`,
//! `static final Map<UUID, UUID> trackingMap = new ConcurrentHashMap<>()`) and
//! `DebugPlayerManager` is its only mention — the code that would fill it is not
//! in the 1.20.1 tree. The port therefore does not own the map: [`Debug::toggle`]
//! takes it as a parameter, and what a test can hold is the removal, not a
//! population. Inventing a `track` method would be inventing the missing half.
//!
//! **What is not here** (`MANQUANT.md` §3): the `DebugEnableMessage` client sync
//! and the two command replies (`CommandToggleDebug.java:40-66`), which need a
//! network and a chat message. The set itself, its two mutators and the
//! production flag are the whole of the manager.

use std::collections::{BTreeMap, BTreeSet};

/// The answer of a toggle, which is also what the command prints: green for
/// `true`, red for `false` (`CommandToggleDebug.java:40-49`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: `toggleDebugModeFor` has two branches (`:40-50`)
pub enum Mode {
    Enabled,
    Disabled,
}

impl Mode {
    /// Java's `boolean` return of `toggleDebugModeFor` (`:45`, `:49`).
    pub fn is_enabled(self) -> bool {
        matches!(self, Mode::Enabled)
    }
}

/// The debug registry, Java's `debugPlayers` set (`:19`) plus the build flag of
/// `hasDebugEnabled` (`:29`).
///
/// No `Default`: [`Debug::new`] is a *production* build and
/// [`Debug::development`] is not, so a silent default would pick one of the two
/// answers for a caller that did not choose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Debug {
    /// `FMLEnvironment.production`: `true` in a shipped build, `false` in a
    /// development run. Java tests `!production`, so the field is stored as
    /// "is this a production build" and [`Debug::new`] starts `true` (the shipped
    /// mod is not a dev environment).
    production: bool,
    /// `debugPlayers` (`:19`), in id order.
    players: BTreeSet<u128>,
}

impl Debug {
    /// The shipped default: a production build, where only the set grants debug.
    pub fn new() -> Debug {
        Debug {
            production: true,
            players: BTreeSet::new(),
        }
    }

    /// A development run, `FMLEnvironment.production == false`: every player has
    /// debug whatever the set says (`:29`).
    pub fn development() -> Debug {
        Debug {
            production: false,
            players: BTreeSet::new(),
        }
    }

    /// Whether the build is a production one.
    pub fn is_production(&self) -> bool {
        self.production
    }

    /// `hasDebugEnabled(player)` (`:27-30`), on the id the method really reads
    /// (`player.getUUID()`).
    pub fn has(&self, player: u128) -> bool {
        !self.production || self.players.contains(&player)
    }

    /// `toggleDebugModeFor(player)` (`:38-51`): the new state, and on a disable
    /// the path-tracking entry is removed (`:43`).
    ///
    /// `tracking` is `PathfindingUtils.trackingMap` (`PathfindingUtils.java:51`),
    /// a `UUID` → `UUID` map the manager only ever removes from.
    pub fn toggle(&mut self, player: u128, tracking: &mut BTreeMap<u128, u128>) -> Mode {
        if self.players.remove(&player) {
            tracking.remove(&player);
            Mode::Disabled
        } else {
            self.players.insert(player);
            Mode::Enabled
        }
    }

    /// `setDebugModeFor(player, enable)` (`:59-69`): the id only, never the
    /// tracking map.
    pub fn set(&mut self, player: u128, enable: bool) {
        if enable {
            self.players.insert(player);
        } else {
            self.players.remove(&player);
        }
    }

    /// How many players are in the set — Java reads the set directly everywhere,
    /// so there is no accessor there; a test needs to see the size.
    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn players(&self) -> impl Iterator<Item = u128> + '_ {
        self.players.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u128 = 0x1111_1111_1111_1111_1111_1111_1111_1111;
    const BOB: u128 = 0x2222_2222_2222_2222_2222_2222_2222_2222;

    #[test]
    fn in_production_only_the_set_grants_debug() {
        let mut debug = Debug::new();
        assert!(!debug.has(ALICE), "the set starts empty");
        debug.set(ALICE, true);
        assert!(debug.has(ALICE) && !debug.has(BOB));
    }

    #[test]
    fn in_development_every_player_has_it_and_a_toggle_off_cannot_take_it_away() {
        let mut debug = Debug::development();
        let mut tracking = BTreeMap::new();
        assert!(debug.has(BOB), "`!production` alone answers true (`:29`)");
        assert_eq!(debug.toggle(BOB, &mut tracking), Mode::Enabled);
        assert_eq!(debug.toggle(BOB, &mut tracking), Mode::Disabled);
        assert!(
            debug.has(BOB),
            "the second half of the `||` cannot win in development"
        );
        assert_eq!(debug.len(), 0, "the set did remove the id");
    }

    #[test]
    fn a_toggle_answers_the_new_state_and_twice_returns_to_disabled() {
        let mut debug = Debug::new();
        let mut tracking = BTreeMap::new();
        assert_eq!(debug.toggle(ALICE, &mut tracking), Mode::Enabled);
        assert!(debug.has(ALICE));
        assert_eq!(debug.toggle(ALICE, &mut tracking), Mode::Disabled);
        assert!(!debug.has(ALICE));
        assert!(debug.is_empty());
    }

    #[test]
    fn a_disable_forgets_the_path_tracking_entry_and_an_enable_keeps_it() {
        let mut debug = Debug::new();
        let mut tracking = BTreeMap::from([(ALICE, BOB)]);
        debug.toggle(ALICE, &mut tracking);
        assert_eq!(
            tracking.get(&ALICE),
            Some(&BOB),
            "an enable touches nothing (`:48-50`)"
        );
        debug.toggle(ALICE, &mut tracking);
        assert_eq!(
            tracking.get(&ALICE),
            None,
            "a disable removes `trackingMap[player]` (`:43`)"
        );
    }

    #[test]
    fn set_never_touches_the_tracking_map_where_toggle_would() {
        let mut debug = Debug::new();
        let mut tracking = BTreeMap::from([(ALICE, BOB)]);
        debug.set(ALICE, true);
        debug.set(ALICE, false);
        debug.set(ALICE, false);
        assert!(!debug.has(ALICE));
        assert_eq!(debug.len(), 0, "the two writes are idempotent");
        assert_eq!(
            tracking.get(&ALICE),
            Some(&BOB),
            "`setDebugModeFor` has no tracking-map line (`:59-69`)"
        );
    }
}
