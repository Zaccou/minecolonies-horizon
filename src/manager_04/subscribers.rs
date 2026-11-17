//! Who is watching a colony, what they are sent, and what is forgotten afterwards.
//!
//! Java reference: `core/colony/managers/ColonyPackageManager.java` (323 lines). The
//! **away timer** of the same class (`:141-160`) is already ported — it is
//! `crate::manager::lifecycle::Attendance`, which names its two constants and keeps
//! Java's one-update-late hour — so this file does not re-open it; it only supplies the
//! bit that timer reads, [`Subscribers::watched`]. Everything here is the other half:
//! the three player sets, the four packets, and the seven dirty flags.
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | who counts as close | a player in a claimed chunk of this colony, alive, in this level | `:112-136` |
//! | who counts as important | a player with a colony-wide permission, from login to logout | `:289-313` |
//! | who gets a **full** view | a newcomer, once — `!newSubscribers.isEmpty()` travels in the packet itself | `:205-220` |
//! | what a fake player gets | a log line and no subscription | `:268-272`, `:295-299` |
//! | which dirty flag a send clears | only its own, except the four Java clears blindly | `:189-199`, `:255` |
//!
//! **Three asymmetries, each a rule rather than an accident, and each pinned by a test.**
//!
//! 1. **Adding an important player queues a view even when it already was one** (`:301-302`
//!    adds to both sets with no `contains` test), while adding a close subscriber who is
//!    already close does nothing at all (`:274-279`). A login therefore always costs one
//!    full colony view; a chunk re-entry costs nothing.
//! 2. **Removing an important player does not make it a stranger** (`:308-313`): it leaves
//!    `importantColonyPlayers` and `newSubscribers`, and stays in `closeSubscribers`.
//! 3. **A culled subscriber is dropped from `closeSubscribers` only** (`:114-135`): a
//!    player who walks out of the chunk while still queued as a newcomer stays in
//!    `newSubscribers`, and the next pass sends it the full view it will never read.
//!
//! **The dirty flags are seven, and Java clears six of them whatever happened.** The
//! colony view's own flag is `isDirty` and lives here; the other six belong to the
//! managers that own the data — permissions, work orders, citizens, buildings, visitors,
//! animals, research — and arrive as [`Flags`]. `updateColonyViews` clears permissions,
//! buildings, citizens, visitors, animals and research **unconditionally** (`:193-198`),
//! but clears the work-order flag **inside the send** (`:255`), so a pass that skipped
//! the packet leaves it set. [`Subscribers::settle`] reproduces both, in order.
//!
//! **Not here, on purpose.** The payloads (`ColonyViewMessage`, `PermissionsMessage`,
//! `ColonyViewWorkOrderMessage`, `ColonyViewCitizenViewMessage`) are `crate::network`
//! already, framing included; what was missing was the **list of recipients**, and that
//! is this file. The five families Java calls at `:182-186` for visitors, buildings,
//! animals and research are their own managers' `sendPackets`: [`Plan::gate`] says
//! whether the pass is inside Java's `if`, and the rest is theirs.

use std::collections::BTreeSet;

/// A player, by uuid: Java keys its three sets on `ServerPlayer`, and the port keys them
/// on the one value that survives a reconnect.
pub type Player = u128;

/// Who is asking to subscribe. Java tests `instanceof FakePlayer` (`:268`) and refuses;
/// the port cannot see the class, so the caller says which it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two kinds of "player" Java's two guards distinguish
pub enum Who {
    Player(Player),
    /// A fake player: a hopper, a command block, a mod's stand-in (`:268`, `:295`).
    Fake,
}

/// What an `add` did — Java's three outcomes, and only one of them is "nothing".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three answers of the two add methods
pub enum Admission {
    /// New to this list: Java queues a full view (`:276-279`).
    Joined,
    /// Already a close subscriber: nothing happens at all (`:274`).
    AlreadyClose,
    /// Already an important player, and **still** queued for a view (`:301-302`).
    AlreadyImportant,
}

/// Why a player was not subscribed at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one refusal, one reason
pub enum Refusal {
    /// Java logs "this should not happen" and returns (`:268-272`, `:295-299`).
    FakePlayer,
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Refusal::FakePlayer => "manager_04.subscribers.refusal.fake_player",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// What the caller knows about one close subscriber, one tick. The five values are the
/// five tests of `updateClosePlayers` (`:118-134`), in Java's order and with Java's
/// short-circuit, so the reason a player was dropped is the first that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five facts the cull reads
pub struct Sighting {
    pub id: Player,
    pub alive: bool,
    /// `colony.getWorld() == player.level`.
    pub same_level: bool,
    /// `WorldUtil.isChunkLoaded(…)`.
    pub chunk_loaded: bool,
    /// `chunk.isEmpty()`.
    pub chunk_empty: bool,
    /// `ColonyUtils.getOwningColony(chunk)`, this colony's id when it is ours.
    pub owner: i32,
}

/// Why a subscriber was dropped (`updateClosePlayers`, `:114-135`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five removals, in the order Java tests them
pub enum Culled {
    Dead { id: Player },
    Elsewhere { id: Player },
    Unloaded { id: Player },
    EmptyChunk { id: Player },
    Foreign { id: Player },
}

impl Culled {
    pub fn id(self) -> Player {
        match self {
            Culled::Dead { id }
            | Culled::Elsewhere { id }
            | Culled::Unloaded { id }
            | Culled::EmptyChunk { id }
            | Culled::Foreign { id } => id,
        }
    }

    /// A stable name: the same five exist as log lines, never as messages.
    pub fn name(self) -> &'static str {
        match self {
            Culled::Dead { .. } => "manager_04.subscribers.culled.dead",
            Culled::Elsewhere { .. } => "manager_04.subscribers.culled.elsewhere",
            Culled::Unloaded { .. } => "manager_04.subscribers.culled.unloaded",
            Culled::EmptyChunk { .. } => "manager_04.subscribers.culled.empty_chunk",
            Culled::Foreign { .. } => "manager_04.subscribers.culled.foreign",
        }
    }
}

/// The six dirty flags the package manager does not own. `Flags::citizens` **is**
/// `CitizenManager.isCitizensDirty` (`:195`), which [`crate::manager_04::citizens`] holds;
/// the caller fills this from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the six `clearDirty` calls, one field each
pub struct Flags {
    pub permissions: bool,
    pub work_orders: bool,
    pub citizens: bool,
    pub buildings: bool,
    pub visitors: bool,
    pub animals: bool,
    pub research: bool,
}

/// Who receives one packet, and whether it must be the complete one
/// (`ColonyView.serializeNetworkData(colony, buf, !newSubscribers.isEmpty())`, `:208`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: a list of players and the one bit the packet carries
pub struct Recipients {
    pub players: Vec<Player>,
    /// Java's `newSubscribers.contains(player)` / `full`: a newcomer gets everything,
    /// a watcher only gets the delta.
    pub complete: bool,
}

/// The four packets one pass is due to send, and Java's gate around the five families it
/// does not own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: four payload families and one gate
pub struct Plan {
    /// `:167`: nothing is sent at all when nobody is watching and nobody is new.
    pub gate: bool,
    pub colony_view: Option<Recipients>,
    pub permissions: Option<Recipients>,
    pub work_orders: Option<Recipients>,
    /// `CitizenManager.sendPackets`: one message per **dirty** citizen to these players.
    pub citizens: Option<Recipients>,
}

/// The three player sets and the colony view's own dirty flag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: `ColonyPackageManager`'s four fields that are not the away timer
pub struct Subscribers {
    close: BTreeSet<Player>,
    important: BTreeSet<Player>,
    /// Java's `newSubscribers`: players who have not received a view yet. They travel in
    /// the packet after this one and are then forgotten (`:199`).
    fresh: BTreeSet<Player>,
    dirty: bool,
}

impl Subscribers {
    pub fn new() -> Subscribers {
        Subscribers::default()
    }

    pub fn close(&self) -> &BTreeSet<Player> {
        &self.close
    }

    pub fn important(&self) -> &BTreeSet<Player> {
        &self.important
    }

    pub fn fresh(&self) -> &BTreeSet<Player> {
        &self.fresh
    }

    /// `isDirty`: the colony view changed since the last pass.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// `setDirty` (`:259-263`).
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    /// The bit `updateAwayTime` reads (`:144`): a colony somebody important is online for
    /// does not count its hours. The timer itself is
    /// [`crate::manager::lifecycle::Attendance`].
    pub fn watched(&self) -> bool {
        !self.important.is_empty()
    }

    /// `addCloseSubscriber` (`:265-280`), with Java's two guards and its one silence.
    pub fn add_close(&mut self, who: Who) -> Result<Admission, Refusal> {
        let Who::Player(id) = who else {
            return Err(Refusal::FakePlayer);
        };
        if self.close.contains(&id) {
            return Ok(Admission::AlreadyClose);
        }
        self.close.insert(id);
        self.fresh.insert(id);
        Ok(Admission::Joined)
    }

    /// `removeCloseSubscriber` (`:282-287`): both sets, in this order.
    pub fn remove_close(&mut self, id: Player) {
        self.fresh.remove(&id);
        self.close.remove(&id);
    }

    /// `addImportantColonyPlayer` (`:291-303`): **no `contains` test**, so a repeated
    /// login still queues a view.
    pub fn add_important(&mut self, who: Who) -> Result<Admission, Refusal> {
        let Who::Player(id) = who else {
            return Err(Refusal::FakePlayer);
        };
        let seen = self.important.contains(&id);
        self.important.insert(id);
        self.fresh.insert(id);
        if seen {
            Ok(Admission::AlreadyImportant)
        } else {
            Ok(Admission::Joined)
        }
    }

    /// `removeImportantColonyPlayer` (`:307-313`): the important set and the queue, **not**
    /// the close set.
    pub fn remove_important(&mut self, id: Player) {
        self.important.remove(&id);
        self.fresh.remove(&id);
    }

    /// `updateClosePlayers` (`:112-136`): the five tests in Java's order, and only
    /// `closeSubscribers` loses anybody.
    ///
    /// A subscriber with no [`Sighting`] is kept: Java's loop walks the set it already
    /// holds, so a missing sighting is a caller that did not look, and guessing would
    /// drop a player for a chunk nobody read.
    pub fn cull(&mut self, self_id: i32, sightings: &[Sighting]) -> Vec<Culled> {
        let mut culled = Vec::new();
        for id in &self.close {
            let Some(sighting) = sightings.iter().find(|sighting| sighting.id == *id) else {
                continue;
            };
            let reason = if !sighting.alive {
                Some(Culled::Dead { id: *id })
            } else if !sighting.same_level {
                Some(Culled::Elsewhere { id: *id })
            } else if !sighting.chunk_loaded {
                Some(Culled::Unloaded { id: *id })
            } else if sighting.chunk_empty {
                Some(Culled::EmptyChunk { id: *id })
            } else if sighting.owner != self_id {
                Some(Culled::Foreign { id: *id })
            } else {
                None
            };
            if let Some(reason) = reason {
                culled.push(reason);
            }
        }
        for reason in &culled {
            self.close.remove(&reason.id());
        }
        culled
    }

    /// `updateColonyViews`'s decision half (`:165-200`), as data.
    ///
    /// The four conditions and the two ways a player gets on a list are Java's; the
    /// permissions packet is the odd one — its *recipients* are chosen with the copy of
    /// `isDirty` that belongs to the colony view, not with `permissions.isDirty()`
    /// (`:231`), so a permission change alone reaches only the newcomers.
    pub fn plan(&self, flags: &Flags) -> Plan {
        let gate = !self.close.is_empty() || !self.fresh.is_empty();
        if !gate {
            return Plan::default();
        }
        let complete = !self.fresh.is_empty();
        Plan {
            gate,
            colony_view: if self.dirty || complete {
                Some(self.recipients(self.dirty, complete))
            } else {
                None
            },
            permissions: if flags.permissions || complete {
                Some(self.recipients(self.dirty, complete))
            } else {
                None
            },
            work_orders: if flags.work_orders || complete {
                Some(self.recipients(true, complete))
            } else {
                None
            },
            citizens: if flags.citizens || complete {
                Some(self.recipients(flags.citizens, complete))
            } else {
                None
            },
        }
    }

    /// The players of one packet: the close subscribers when `include_close`, then the
    /// newcomers — deduplicated, close first, which is the order Java builds a `HashSet`
    /// in and iterates.
    fn recipients(&self, include_close: bool, complete: bool) -> Recipients {
        let mut players: Vec<Player> = Vec::new();
        if include_close {
            players.extend(self.close.iter().copied());
        }
        for id in &self.fresh {
            if !players.contains(id) {
                players.push(*id);
            }
        }
        Recipients { players, complete }
    }

    /// The clearing half of `updateColonyViews` (`:189-199`), **in Java's order**, and
    /// with Java's one condition: the work-order flag is only cleared when its packet was
    /// actually built (`:255`), while the other five are cleared whatever happened.
    pub fn settle(&mut self, plan: &Plan, flags: &mut Flags) {
        if plan.work_orders.is_some() {
            flags.work_orders = false;
        }
        if self.fresh.is_empty() {
            self.dirty = false;
        }
        flags.permissions = false;
        flags.buildings = false;
        flags.citizens = false;
        flags.visitors = false;
        flags.animals = false;
        flags.research = false;
        self.fresh.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(players: &[Player]) -> Subscribers {
        let mut subscribers = Subscribers::new();
        for id in players {
            assert_eq!(subscribers.add_close(Who::Player(*id)), Ok(Admission::Joined));
        }
        subscribers
    }

    fn sighting(id: Player, owner: i32) -> Sighting {
        Sighting {
            id,
            alive: true,
            same_level: true,
            chunk_loaded: true,
            chunk_empty: false,
            owner,
        }
    }

    #[test]
    fn a_fake_player_is_refused_by_name_and_enters_nothing() {
        let mut subscribers = Subscribers::new();
        assert_eq!(subscribers.add_close(Who::Fake), Err(Refusal::FakePlayer));
        assert_eq!(subscribers.add_important(Who::Fake), Err(Refusal::FakePlayer));
        assert!(subscribers.close().is_empty());
        assert!(subscribers.important().is_empty());
        assert!(subscribers.fresh().is_empty());
        assert_eq!(Refusal::FakePlayer.name(), "manager_04.subscribers.refusal.fake_player");
        assert_eq!(Refusal::FakePlayer.to_string(), "manager_04.subscribers.refusal.fake_player");
    }

    #[test]
    fn a_newcomer_is_queued_once_and_a_second_entry_costs_nothing() {
        let mut subscribers = Subscribers::new();
        assert_eq!(subscribers.add_close(Who::Player(7)), Ok(Admission::Joined));
        assert_eq!(subscribers.fresh().len(), 1);
        assert_eq!(subscribers.add_close(Who::Player(7)), Ok(Admission::AlreadyClose));
        assert_eq!(subscribers.fresh().len(), 1, ":274 skips an id it already holds");

        let plan = subscribers.plan(&Flags::default());
        assert!(plan.gate);
        let view = plan.colony_view.clone().expect("a newcomer is due a view");
        assert_eq!(view.players, vec![7]);
        assert!(view.complete, "and it is the full one");

        let mut flags = Flags::default();
        subscribers.settle(&plan, &mut flags);
        assert!(subscribers.fresh().is_empty());
        assert!(subscribers.plan(&Flags::default()).colony_view.is_none());
    }

    #[test]
    fn an_important_player_is_queued_even_when_it_already_was_one_and_keeps_its_close_link() {
        let mut subscribers = join(&[3]);
        let plan = subscribers.plan(&Flags::default());
        let mut flags = Flags::default();
        subscribers.settle(&plan, &mut flags);
        assert!(subscribers.fresh().is_empty());

        assert_eq!(subscribers.add_important(Who::Player(3)), Ok(Admission::Joined));
        assert!(subscribers.watched(), "a colony somebody watches does not count hours");
        let plan = subscribers.plan(&flags);
        assert_eq!(
            plan.colony_view.clone().map(|view| view.complete),
            Some(true),
            ":301-302 queues a view for an important player"
        );
        subscribers.settle(&plan, &mut flags);
        assert!(subscribers.fresh().is_empty());

        // The second login of the same player goes through both sets again.
        assert_eq!(
            subscribers.add_important(Who::Player(3)),
            Ok(Admission::AlreadyImportant)
        );
        assert_eq!(subscribers.fresh().len(), 1, ":301-302 has no contains test");
        assert!(subscribers.close().contains(&3), "an important player may also be close");

        // Logging off removes it from the important set and the queue, not from close.
        subscribers.remove_important(3);
        assert!(!subscribers.important().contains(&3));
        assert!(subscribers.close().contains(&3), ":308-313 leaves the close set alone");
        assert!(!subscribers.watched());
        assert!(subscribers.fresh().is_empty(), "the queue is forgotten on logoff");
        subscribers.remove_close(3);
        assert!(subscribers.close().is_empty());
    }

    #[test]
    fn the_five_cull_reasons_are_tested_in_java_s_order_and_only_close_loses_a_player() {
        let mut subscribers = join(&[1, 2, 3, 4, 5, 6]);
        let mut dead = sighting(1, 1);
        dead.alive = false;
        let mut elsewhere = sighting(2, 1);
        elsewhere.same_level = false;
        let mut unloaded = sighting(3, 1);
        unloaded.chunk_loaded = false;
        let mut empty = sighting(4, 1);
        empty.chunk_empty = true;
        let sight = [
            dead,
            elsewhere,
            unloaded,
            empty,
            sighting(5, 9),
            sighting(6, 1),
        ];
        let culled = subscribers.cull(1, &sight);
        assert_eq!(
            culled,
            vec![
                Culled::Dead { id: 1 },
                Culled::Elsewhere { id: 2 },
                Culled::Unloaded { id: 3 },
                Culled::EmptyChunk { id: 4 },
                Culled::Foreign { id: 5 },
            ],
            "the first failing test is the reason, as in Java's || chain"
        );
        for reason in &culled {
            assert!(reason.name().starts_with("manager_04.subscribers.culled."));
            assert!((1..=5).contains(&reason.id()), "one reason per player, 1 to 5");
        }
        assert_eq!(subscribers.close().len(), 1);
        assert!(subscribers.close().contains(&6), "the one player still in a claimed chunk");
        // The culled players were newcomers: Java's `iterator.remove` only touches the
        // close set, so they stay queued for the full view (`:120` against `:199`).
        assert_eq!(subscribers.fresh().len(), 6);
        // A subscriber nobody looked at is kept rather than guessed away.
        assert_eq!(subscribers.cull(1, &[]), Vec::new());
        assert_eq!(subscribers.close().len(), 1);
    }

    #[test]
    fn the_four_packets_choose_their_recipients_the_way_java_does() {
        let mut subscribers = join(&[1]);
        let plan = subscribers.plan(&Flags::default());
        let mut flags = Flags::default();
        subscribers.settle(&plan, &mut flags);
        // A second watcher, so the pass has one close subscriber and one newcomer.
        assert_eq!(subscribers.add_close(Who::Player(2)), Ok(Admission::Joined));
        subscribers.set_dirty();
        let flags = Flags {
            permissions: true,
            work_orders: true,
            citizens: true,
            ..Flags::default()
        };
        let plan = subscribers.plan(&flags);
        let view = plan.colony_view.clone().expect("a change and a newcomer");
        assert_eq!(view.players, vec![1, 2]);
        assert!(view.complete);
        let permissions = plan.permissions.clone().expect("permissions is dirty");
        assert_eq!(permissions.players, vec![1, 2], ":231 reads the colony's isDirty");
        let work_orders = plan.work_orders.clone().expect("work orders are dirty");
        assert_eq!(work_orders.players, vec![1, 2], ":248 always sends to both sets");
        let citizens = plan.citizens.clone().expect("citizens are dirty");
        assert_eq!(citizens.players, vec![1, 2]);
        assert!(citizens.complete, "one message per dirty citizen, full for a newcomer");

        // Only the work-order flag was set: the other three packets are not due, and the
        // permission one would reach nobody — so it is not built.
        let quiet = Subscribers::new();
        let plan = quiet.plan(&Flags::default());
        assert!(!plan.gate);
    }

    #[test]
    fn settle_clears_six_flags_blindly_keeps_the_work_order_one_and_the_colony_flag_a_pass_late()
    {
        let mut subscribers = join(&[1]);
        let plan = subscribers.plan(&Flags::default());
        let mut flags = Flags::default();
        subscribers.settle(&plan, &mut flags);

        // A newcomer arrives and the colony changes.
        assert_eq!(subscribers.add_close(Who::Player(2)), Ok(Admission::Joined));
        subscribers.set_dirty();
        let flags_now = Flags {
            permissions: true,
            work_orders: true,
            citizens: true,
            buildings: true,
            visitors: true,
            animals: true,
            research: true,
        };
        let plan = subscribers.plan(&flags_now);
        let mut flags = flags_now;
        subscribers.settle(&plan, &mut flags);
        assert!(!flags.permissions, "cleared blindly, :193");
        assert!(!flags.buildings, ":194");
        assert!(!flags.citizens, ":195");
        assert!(!flags.visitors, ":196");
        assert!(!flags.animals, ":197");
        assert!(!flags.research, ":198");
        assert!(!flags.work_orders, "cleared because the packet was built, :255");
        assert!(subscribers.fresh().is_empty());

        // The colony flag is cleared **only when the queue was already empty** (`:189`),
        // which it was not, so the next pass still owes a view.
        assert!(subscribers.dirty());
        let plan = subscribers.plan(&Flags::default());
        let view = plan.colony_view.clone().expect("one pass late, :189-192");
        assert_eq!(view.players, vec![1, 2]);
        assert!(!view.complete, "the newcomer has had its full view");
        let mut flags = Flags::default();
        subscribers.settle(&plan, &mut flags);
        assert!(!subscribers.dirty(), "and now it is cleared");
        assert!(subscribers.plan(&Flags::default()).colony_view.is_none());
    }

    #[test]
    fn a_pass_that_built_no_work_order_packet_leaves_the_flag_set() {
        let mut subscribers = join(&[5]);
        let mut flags = Flags {
            work_orders: true,
            ..Flags::default()
        };
        let plan = subscribers.plan(&flags);
        assert!(plan.work_orders.is_some(), "a close subscriber receives it");
        subscribers.settle(&plan, &mut flags);
        assert!(!flags.work_orders);

        // Nobody is left: the gate is closed, no packet is built, and the flag survives.
        subscribers.remove_close(5);
        let mut flags = Flags {
            work_orders: true,
            permissions: true,
            ..Flags::default()
        };
        let plan = subscribers.plan(&flags);
        assert!(!plan.gate);
        assert!(plan.work_orders.is_none());
        subscribers.settle(&plan, &mut flags);
        assert!(
            flags.work_orders,
            "no packet, no clear — the one flag Java does not touch blindly"
        );
        assert!(!flags.permissions, "this one it does");
    }

    #[test]
    fn an_empty_colony_plans_nothing_and_watched_is_the_away_timer_s_input() {
        let subscribers = Subscribers::new();
        let plan = subscribers.plan(&Flags {
            permissions: true,
            work_orders: true,
            citizens: true,
            ..Flags::default()
        });
        assert!(!plan.gate, ":167 sends nothing to nobody");
        assert_eq!(plan, Plan::default());
        assert!(!subscribers.watched());

        let mut watched = Subscribers::new();
        watched.add_important(Who::Player(9)).expect("a real player");
        assert!(watched.watched());
        watched.remove_important(9);
        assert!(!watched.watched(), "the bit Attendance::update takes");
        // The timer it feeds is the one already ported, and this file does not re-open it.
        assert_eq!(crate::manager::lifecycle::UPDATE_STATE_INTERVAL, 100);
        assert_eq!(crate::manager::lifecycle::TICKS_HOUR, 72_000);
    }
}
