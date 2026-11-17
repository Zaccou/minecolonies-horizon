//! `ServerUtils` and `AdvancementUtils`: which online player is this id, and which
//! of them get an advancement.
//!
//! Java reference: `core/util/ServerUtils.java` (150 lines) and
//! `core/util/AdvancementUtils.java` (32). Both are four lookups and a filter over
//! the server's player list; neither computes anything — which is exactly why they
//! are easy to get subtly wrong, and why [`Online`] exists here instead of three
//! near-copies of "find the player with this UUID".
//!
//! # The bug the port does not copy
//!
//! `getPlayersFromPermPlayer` (`ServerUtils.java:95-105`) appends the result of
//! `getPlayerFromPermPlayer` **without checking it**:
//!
//! ```java
//! for (@NotNull final Player player : players) {
//!     playerList.add(ServerUtils.getPlayerFromPermPlayer(player, world));   // may be null
//! }
//! return playerList;
//! ```
//!
//! A colony player who has logged off gives `null`, and every caller of this method
//! then has to know that. [`perm_players`] drops the offline ones and says so: an
//! `Option` in a `Vec` would push the same null one step further out.
//!
//! # The rank test is not here
//!
//! `AdvancementUtils` filters on `Rank::isColonyManager` (`:20`). That predicate
//! already has **two** ports — `colony/permissions.rs:69` and
//! `network/permissions.rs:385`, both `fn is_manager` — so [`advancement_targets`]
//! takes the answer rather than opening a third copy of the same question.

/// A player the server currently has online: the two fields these lookups compare.
// exhaustive: the id the lookups match and the name a caller reports
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Online {
    /// `player.getUUID()`, as its 128 bits. A UUID is 128 bits by definition; the
    /// port has no `Uuid` type and every comparison here is equality.
    pub id: u128,
    /// `player.getName().getString()`, for a log line or a message.
    pub name: String,
}

impl Online {
    pub fn new(id: u128, name: &str) -> Online {
        Online {
            id,
            name: name.to_owned(),
        }
    }
}

/// The online player with this id: `getPlayerFromUUID(UUID, Level)`
/// (`ServerUtils.java:134-149`) with the null guard of its twin
/// `getPlayerFromUUID(Level, UUID)` (`:37-47`).
///
/// ```java
/// if (uuid == null) return null;
/// for (@NotNull final ServerPlayer player : world.getServer().getPlayerList().getPlayers()) {
///     if (player.getUUID().equals(uuid)) return player;
/// }
/// return null;
/// ```
///
/// The two Java overloads differ **only** in the null guard (`:136-139`), which is
/// why there is one function here taking `Option<u128>`: `None` is Java's `null`
/// id and answers `None` without looking at the list.
pub fn find_player(online: &[Online], id: Option<u128>) -> Option<&Online> {
    let id = id?;
    online.iter().find(|player| player.id == id)
}

/// Every online player whose id is in `ids`: `getPlayersFromUUID`
/// (`ServerUtils.java:57-81`).
///
/// ```java
/// for (final Object o : world.players()) {
///     if (o instanceof Player) {
///         if (ids.contains(player.getGameProfile().getId())) {
///             players.add(player);
///             if (players.size() == ids.size()) return players;
///         }
///     }
/// }
/// return players;
/// ```
///
/// The order is the **server's**, not the caller's, because the loop is over the
/// player list. The early return (`:73-76`) is an optimization with one visible
/// consequence: a list of ids containing a duplicate never reaches
/// `players.size() == ids.size()` and scans the whole list — same answer, more
/// work. Kept out of the port because a `filter` cannot early-exit and the answer
/// does not change.
///
/// Ids that match nobody are skipped silently, which is the case `ServerUtils` is
/// used for: a colony's members, some of whom are offline.
pub fn players_with_ids<'a>(online: &'a [Online], ids: &[u128]) -> Vec<&'a Online> {
    online
        .iter()
        .filter(|player| ids.contains(&player.id))
        .collect()
}

/// The online players behind a list of ids, **without the nulls**:
/// `getPlayersFromPermPlayer` (`ServerUtils.java:95-105`) and
/// `getPlayerFromPermPlayer` (`:119-122`).
///
/// Java's version returns a list the same length as its input, with a `null` in
/// every slot whose player is offline. Dropping them is the port's one departure
/// here, and it is a departure from a bug: see the module note.
pub fn perm_players<'a>(
    online: &'a [Online],
    ids: impl IntoIterator<Item = u128>,
) -> Vec<&'a Online> {
    ids.into_iter()
        .filter_map(|id| find_player(online, Some(id)))
        .collect()
}

/// Which players an advancement is triggered on: `TriggerAdvancementPlayersForColony`
/// (`AdvancementUtils.java:15-31`).
///
/// ```java
/// final Predicate<Rank> predicate = Rank::isColonyManager;
/// for (final ColonyPlayer player : colony.getPermissions().getFilteredPlayers(predicate)) {
///     final ServerPlayer playerEntity = minecraftServer.getPlayerList().getPlayer(player.getID());
///     if (playerEntity != null) playerConsumer.accept(playerEntity);
/// }
/// ```
///
/// So: the colony's **managers**, of whom only the ones **currently online** get
/// the trigger. A manager who is asleep at the keyboard and logged out is not
/// woken up for it, which is the whole semantics of an advancement.
///
/// `managers` is `(id, is_manager)` per colony member — the boolean comes from
/// `Rank::isColonyManager`, which the port already answers twice (module note).
/// The `null` check at `:25-28` is what [`find_player`] does, so an offline manager
/// contributes nothing.
pub fn advancement_targets<'a>(
    online: &'a [Online],
    managers: impl IntoIterator<Item = (u128, bool)>,
) -> Vec<&'a Online> {
    managers
        .into_iter()
        .filter(|(_, is_manager)| *is_manager)
        .filter_map(|(id, _)| find_player(online, Some(id)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u128 = 0xA11CE;
    const BOB: u128 = 0xB0B;
    const CAROL: u128 = 0xCA201;

    fn server() -> Vec<Online> {
        vec![
            Online::new(BOB, "Bob"),
            Online::new(ALICE, "Alice"),
            Online::new(CAROL, "Carol"),
        ]
    }

    /// The id finds its player wherever it sits in the list.
    #[test]
    fn a_player_is_found_by_id() {
        let online = server();
        assert_eq!(find_player(&online, Some(ALICE)).map(|p| p.name.as_str()), Some("Alice"));
        assert_eq!(find_player(&online, Some(CAROL)).map(|p| p.name.as_str()), Some("Carol"));
    }

    #[test]
    fn an_unknown_id_finds_nobody() {
        assert!(find_player(&server(), Some(0xDEAD)).is_none());
    }

    /// Java's null guard: a `null` id answers `null` without scanning.
    #[test]
    fn a_missing_id_answers_nobody() {
        assert!(find_player(&server(), None).is_none());
    }

    /// The result follows the **server's** order, not the order of the ids.
    #[test]
    fn the_players_come_back_in_server_order() {
        let online = server();
        let found = players_with_ids(&online, &[CAROL, BOB]);
        assert_eq!(
            found.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["Bob", "Carol"]
        );
    }

    /// Offline ids are skipped, and an empty request asks for nothing.
    #[test]
    fn unknown_ids_are_skipped_and_an_empty_request_is_empty() {
        let online = server();
        assert_eq!(players_with_ids(&online, &[ALICE, 0xDEAD]).len(), 1);
        assert!(players_with_ids(&online, &[]).is_empty());
    }

    /// A duplicated id does not duplicate the player: it is one entry of the
    /// online list.
    #[test]
    fn a_duplicated_request_yields_one_player() {
        assert_eq!(players_with_ids(&server(), &[ALICE, ALICE]).len(), 1);
    }

    /// The departure from Java: an offline member is dropped, not appended as a
    /// null.
    #[test]
    fn an_offline_member_is_dropped_rather_than_kept_as_a_hole() {
        let online = server();
        let found = perm_players(&online, [ALICE, 0xDEAD, BOB]);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|player| player.id != 0xDEAD));
    }

    /// The request order is kept here, because `perm_players` iterates the ids and
    /// not the server list.
    #[test]
    fn the_perm_players_follow_the_request_order() {
        let online = server();
        let found = perm_players(&online, [CAROL, ALICE]);
        assert_eq!(
            found.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["Carol", "Alice"]
        );
    }

    /// Only managers, and only the ones online.
    #[test]
    fn an_advancement_goes_to_managers_who_are_online() {
        let online = server();
        let targets = advancement_targets(&online, [(ALICE, true), (BOB, false), (0xDEAD, true)]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "Alice");
    }

    /// A colony with no manager, or with managers all offline, triggers on nobody.
    #[test]
    fn an_advancement_with_no_eligible_manager_goes_nowhere() {
        let online = server();
        assert!(advancement_targets(&online, [(ALICE, false)]).is_empty());
        assert!(advancement_targets(&online, [(0xDEAD, true)]).is_empty());
        assert!(advancement_targets(&online, std::iter::empty()).is_empty());
    }
}
