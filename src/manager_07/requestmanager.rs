//! The request manager: the nine keys of its save, the version that wipes it, the
//! order of its writes, and what follows a state change.
//!
//! Java reference: `core/colony/requestsystem/management/manager/StandardRequestManager.java`
//! (682 lines), with the update ladder in
//! `core/colony/requestsystem/management/handlers/UpdateHandler.java:23-28` and
//! the state table it calls in
//! `core/colony/requestsystem/management/handlers/RequestHandler.java:328-520`.
//!
//! **What is ported, and what is not.** The manager itself is nine token slots,
//! five data stores, a version and a dirty flag: that is what this file owns,
//! because it is the only thing in the class that is not a call on a handler. The
//! five handlers (`UpdateHandler`, `TokenHandler`, `ResolverHandler`,
//! `RequestHandler`, `ProviderHandler`, `:96-108`) are **not** ported: they walk
//! resolvers, providers and children, and neither exists in this crate —
//! `src/requests/mod.rs` says so for the resolver, the children and the network,
//! and its `resolver.rs` already answers the one pure question (who supplies).
//! What the handlers *decide* is here: [`follow_up`] is the state table, and the
//! two ordering rules below are the manager's own.
//!
//! **The version is a wipe, not a counter.** `UpdateHandler.getCurrentVersion` is
//! the highest `updatesToVersion` of the four steps it holds — `0`, `13`, `14`,
//! `15` (`UpdateHandler.java:23-28`, `InitialUpdate.java:14`,
//! `ResetRSToStoreJobInResolvers.java:16`, `ResetRSToUpdateRestaurantResolver.java:16`,
//! `ResetRSToRemoveAssistantCookResolver.java:16`) — so [`CURRENT_VERSION`] is
//! **15**. A manager starts at `-1` (`:110`) and `updateIfRequired` resets every
//! save whose version is below that (`:663-669`): a save from an older request
//! system does not migrate, it is thrown away and rebuilt. That is why the port
//! calls it a wipe.
//!
//! **A reset needs tokens, and a factory is what Java uses to make them.** `setup`
//! mints five store tokens and two resolver tokens and files each store under its
//! own id (`:124-149`). A wasm mod has no random source in this crate, so
//! [`Manager::reset`] takes the seven ids from its caller and [`Loaded`] says
//! when one is needed: a save with no `DataStores`, a save with a missing or
//! unreadable key, and a save older than 15 all ask for it.
//!
//! **Two orders, and they are opposite on purpose.**
//!
//! - `assignRequest` reads the request (which throws when it is unknown), **then**
//!   assigns, **then** marks dirty (`:230-234`): a refusal leaves the manager
//!   clean, and an assignment that finds nobody still marks it.
//! - `reassignRequest` reads the request, **then** marks dirty, **then** tries
//!   (`:253-260`): the flag is set even when the attempt comes back empty-handed,
//!   because the old assignment has already been taken away by then
//!   (`RequestHandler.java:296-307`).
//!
//! Both are tested, because a port that marked dirty in one place for both would
//! pass every test of the *result* and lose the difference.
//!
//! **The save is nine keys** (`:54-62`) — a version, the store book, five store
//! ids, two resolver ids — and they are written in that order (`:434-447`).
//! [`Manager::write`] refuses to produce one from a manager that was never set
//! up, where Java would hand `controller.serialize(null)` a null and crash inside
//! the call.
//!
//! **What the port does with the requests.** They live in `crate::requests::Queue`
//! and are handed in, the way `manager`'s colonies are handed to `persist`: Java's
//! manager owns the identities store, and this crate already has one owner for a
//! request (`src/requests/queue.rs`). Nothing is duplicated; the manager only
//! changes a request through the four moves `Request` already defines.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use crate::inventory::{ItemName, Stack};
use crate::requests::{Queue, Request, State};

use super::datastores::{Book, Skip, Store, Token};
use super::nbt;

/// The version this port writes and the lowest it reads: the highest step of
/// `UpdateHandler`'s ladder (`UpdateHandler.java:23-28`), 15.
pub const CURRENT_VERSION: i32 = 15;

/// `NBT_VERSION` (`StandardRequestManager.java:62`).
pub const KEY_VERSION: &str = "Version";
/// `NBT_DATASTORE` (`:54`).
pub const KEY_DATA_STORES: &str = "DataStores";
/// `NBT_ID_REQUEST_IDENTITIES` (`:55`).
pub const KEY_REQUEST_IDENTITIES: &str = "RequestIdentitiesStoreId";
/// `NBT_ID_REQUEST_RESOLVER_IDENTITIES` (`:56`).
pub const KEY_RESOLVER_IDENTITIES: &str = "RequestResolverIdentitiesStoreId";
/// `NBT_ID_PROVIDER_ASSIGNMENTS` (`:57`).
pub const KEY_PROVIDER_ASSIGNMENTS: &str = "ProviderAssignmentsStoreId";
/// `NBT_ID_REQUEST_RESOLVER_ASSIGNMENTS` (`:58`).
pub const KEY_RESOLVER_ASSIGNMENTS: &str = "RequestResolverAssignmentsStoreId";
/// `NBT_ID_REQUESTABLE_TYPE_ASSIGNMENTS` (`:59`).
pub const KEY_REQUESTABLE_ASSIGNMENTS: &str = "RequestableTypeAssignmentsStoreId";
/// `NBT_ID_PLAYER` (`:60`).
pub const KEY_PLAYER: &str = "PlayerRequestResolverId";
/// `NBT_ID_RETRYING` (`:61`).
pub const KEY_RETRYING: &str = "RetryingRequestResolverId";

/// The five stores a request system is built on, in the order `setup` makes them
/// (`:129-133`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the five stores Java registers at setup
pub enum Kind {
    /// Who asked what (`REQUEST_IDENTITIES_DATA_STORE`).
    RequestIdentities,
    /// Who may supply (`REQUEST_RESOLVER_IDENTITIES_DATA_STORE`).
    ResolverIdentities,
    /// Which provider answers which resolver (the third store of `:131`).
    ProviderAssignments,
    /// Which resolver holds which request (`:132`).
    ResolverAssignments,
    /// Which requestable type goes to which resolver (`:133`).
    RequestableTypeAssignments,
}

impl Kind {
    /// The five, in setup order.
    pub const ALL: [Kind; 5] = [
        Kind::RequestIdentities,
        Kind::ResolverIdentities,
        Kind::ProviderAssignments,
        Kind::ResolverAssignments,
        Kind::RequestableTypeAssignments,
    ];

    /// The key this store's id is saved under (`:55-59`).
    pub fn key(self) -> &'static str {
        match self {
            Self::RequestIdentities => KEY_REQUEST_IDENTITIES,
            Self::ResolverIdentities => KEY_RESOLVER_IDENTITIES,
            Self::ProviderAssignments => KEY_PROVIDER_ASSIGNMENTS,
            Self::ResolverAssignments => KEY_RESOLVER_ASSIGNMENTS,
            Self::RequestableTypeAssignments => KEY_REQUESTABLE_ASSIGNMENTS,
        }
    }

    /// A stable name, for a log line that has to say which store it means.
    pub fn name(self) -> &'static str {
        match self {
            Self::RequestIdentities => "requests.store.identities",
            Self::ResolverIdentities => "requests.store.resolvers",
            Self::ProviderAssignments => "requests.store.providers",
            Self::ResolverAssignments => "requests.store.assignments",
            Self::RequestableTypeAssignments => "requests.store.types",
        }
    }
}

/// The seven ids a fresh manager needs: five stores, then the two resolvers
/// (`:129-142`). Java mints them from a factory; a caller here supplies them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub stores: [Token; 5],
    pub player: Token,
    pub retrying: Token,
}

impl Tokens {
    /// The id of a store, by kind.
    pub fn store(&self, kind: Kind) -> Token {
        let index = Kind::ALL.iter().position(|known| *known == kind).unwrap_or(0);
        self.stores[index]
    }
}

/// What Java does after a state change (`:304-331`).
///
/// Java's switch has six cases and no `default` body. `RESOLVED` asks the handler
/// for follow-up requests (`:306-309`), `COMPLETED` notifies parent and requester
/// (`:310-313`), `OVERRULED` notifies parent, children and requester
/// (`:314-317`), `FAILED` **and** `CANCELLED` share one handler
/// (`RequestHandler.onRequestCancelled`, `:318-325`), and `RECEIVED` cleans the
/// request out of the system (`:326-329`). Four of those states have no
/// equivalent here — this crate keeps five of Java's thirteen
/// (`src/requests/request.rs`, "Five states, and the reason there are not
/// thirteen") — so [`FollowUp`] carries the three that do:
///
/// | Port state | Java state | Follow-up |
/// | --- | --- | --- |
/// | `Created`, `Assigned`, `InProgress` | `CREATED`, `ASSIGNED`, `IN_PROGRESS` | none (`:330`) |
/// | `Delivered` | `COMPLETED` | [`FollowUp::Completed`] |
/// | `Cancelled` | `CANCELLED` | [`FollowUp::Cancelled`] |
///
/// The port's `Delivered` is Java's `COMPLETED`: Java has no `DELIVERED` state at
/// all (`RequestState.java:13-78`), and `COMPLETED` is its "the work is done"
/// terminal one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three follow-ups the five port states can reach
pub enum FollowUp {
    /// Nothing: Java's `default` (`:330-331`).
    None,
    /// `onRequestCompleted` (`:310-313`).
    Completed,
    /// `onRequestCancelled` (`:318-325`).
    Cancelled,
}

impl FollowUp {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "requests.follow_up.none",
            Self::Completed => "requests.follow_up.completed",
            Self::Cancelled => "requests.follow_up.cancelled",
        }
    }
}

/// The follow-up a state change earns (`:304-331`).
pub fn follow_up(state: State) -> FollowUp {
    match state {
        State::Created | State::Assigned | State::InProgress => FollowUp::None,
        State::Delivered => FollowUp::Completed,
        State::Cancelled => FollowUp::Cancelled,
    }
}

/// The two steps of a tick, in Java's order (`:592-596`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickStep {
    /// `getRetryingRequestResolver().updateManager(this)` (`:594`).
    UpdateManager,
    /// `getRetryingRequestResolver().tick()` (`:595`).
    Retry,
}

/// The order the two run in: the resolver learns about the manager before it
/// advances (`:594-595`).
pub const TICK_ORDER: [TickStep; 2] = [TickStep::UpdateManager, TickStep::Retry];

/// Why this manager refused an operation, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the manager's own refusals plus the request's, wrapped
pub enum Refusal {
    /// No request with this id: Java's `getRequest` throws
    /// (`RequestHandler.java:57-70`), and every one of the manager's entry points
    /// calls it first.
    Unknown { id: i32 },
    /// The request itself refused the move, with its own name
    /// (`src/requests/mod.rs`, `Refusal`).
    Request(crate::requests::Refusal),
    /// The manager was never set up, so one of the nine keys has no value to
    /// write. Java would pass a null token to the factory controller
    /// (`:438-445`).
    NotSetUp { key: &'static str },
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown { .. } => "requestmanager.refusal.unknown",
            Self::Request(refusal) => refusal.name(),
            Self::NotSetUp { .. } => "requestmanager.refusal.not_set_up",
        }
    }
}

impl From<crate::requests::Refusal> for Refusal {
    fn from(refusal: crate::requests::Refusal) -> Refusal {
        Refusal::Request(refusal)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::Unknown { id } => write!(f, " ({id})"),
            Self::Request(refusal) => write!(f, " ({refusal})"),
            Self::NotSetUp { key } => write!(f, " ({key})"),
        }
    }
}

impl std::error::Error for Refusal {}

/// A key this read could not take, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mend {
    pub key: &'static str,
    pub reason: MendReason,
}

/// Why one of the nine keys was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MendReason {
    /// Absent: `contains` fails and `markForUpdate` runs (`:557-561`).
    Missing,
    /// Present, but not something the extractor reads. For a token this is
    /// `deserialize` throwing and the `catch` at `:563-572` marking for update;
    /// for `Version` it is `getInt` answering `0`, which is under 15 and wipes the
    /// system just the same.
    Unreadable,
}

impl MendReason {
    pub fn name(self) -> &'static str {
        match self {
            Self::Missing => "requestmanager.mend.missing",
            Self::Unreadable => "requestmanager.mend.unreadable",
        }
    }
}

/// What a load had to report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    /// Every key that was not read as saved.
    pub mends: Vec<Mend>,
    /// Requests dropped by the orphan sweep (`:504-518`), in arrival order.
    pub orphans: Vec<Token>,
    /// Whether the caller must call [`Manager::reset`] with fresh ids: no store
    /// book, a key that was not read, or a version under [`CURRENT_VERSION`].
    pub needs_reset: bool,
    /// Entries of the store book the read refused or had to repair.
    pub skipped: Vec<Skip>,
    /// What the store book itself had to report (`datastores`' own repairs).
    pub book_mends: Vec<super::datastores::Mend>,
}

/// The seven ids and the four fields Java keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manager {
    version: i32,
    dirty: bool,
    book: Book,
    stores: BTreeMap<Kind, Token>,
    player: Option<Token>,
    retrying: Option<Token>,
    overruled: BTreeMap<i32, ItemName>,
}

impl Default for Manager {
    fn default() -> Self {
        Manager::new()
    }
}

impl Manager {
    /// A manager before `reset`: version `-1` (`:110`) and **dirty**
    /// (`private boolean dirty = true`, `:84`), because it has not been saved yet.
    pub fn new() -> Manager {
        Manager {
            version: -1,
            dirty: true,
            book: Book::new(),
            stores: BTreeMap::new(),
            player: None,
            retrying: None,
            overruled: BTreeMap::new(),
        }
    }

    /// The saved version (`getCurrentVersion`, `:671-675`).
    pub fn version(&self) -> i32 {
        self.version
    }

    /// `isDirty` (`:206-210`).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `markDirty` (`:195-199`).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `setDirty` (`:212-221`): Java marks the **colony** dirty every time the
    /// flag is set to true, and the port answers whether it must — one `bool` for
    /// the caller, which is where the colony is.
    pub fn set_dirty(&mut self, dirty: bool) -> bool {
        self.dirty = dirty;
        dirty
    }

    /// The store book (`getDataStoreManager`, `:405-410`).
    pub fn book(&self) -> &Book {
        &self.book
    }

    /// The store book, to change it.
    pub fn book_mut(&mut self) -> &mut Book {
        &mut self.book
    }

    /// The store filed under this kind's id, if the manager was set up.
    pub fn store(&self, kind: Kind) -> Option<&Store> {
        let token = *self.stores.get(&kind)?;
        self.book.get(token)
    }

    /// The id the kind's store is filed under (`:129-133`).
    pub fn store_id(&self, kind: Kind) -> Option<Token> {
        self.stores.get(&kind).copied()
    }

    /// The player resolver's id (`:141`).
    pub fn player(&self) -> Option<Token> {
        self.player
    }

    /// The retrying resolver's id (`:142`).
    pub fn retrying(&self) -> Option<Token> {
        self.retrying
    }

    /// The stack a player overruled a request with, if any.
    ///
    /// Java keeps it **on the request**
    /// (`request.overrideCurrentDeliveries(ImmutableList.of(stack))`, `:352`),
    /// and `crate::requests::Request` has no delivery list, so this is the one
    /// piece of request state the manager owns. One place, no duplicate.
    pub fn overruled_by(&self, id: i32) -> Option<&ItemName> {
        self.overruled.get(&id)
    }

    /// `setup` (`:124-149`) and `reset` (`:412-424`), with the ids supplied by the
    /// caller instead of minted.
    ///
    /// Java's order is: mint the book, mint and register five stores, then mint
    /// and register the two resolvers (`:126-142`); then `version = -1` and the
    /// update ladder runs, which leaves the version at [`CURRENT_VERSION`]
    /// (`:422-423`, `:50-65` of `UpdateHandler`).
    ///
    /// **What a reset does not do here.** Java's reset builds a new store book,
    /// so the requests — which live in the identities store — go with it. The
    /// port's reset cannot touch a `Queue` it does not own, so a caller that
    /// resets must clear its queue too.
    pub fn reset(&mut self, tokens: &Tokens) {
        self.book = Book::new();
        self.stores.clear();
        for kind in Kind::ALL {
            let token = tokens.store(kind);
            self.book.get_or_create(token);
            self.stores.insert(kind, token);
        }
        self.player = Some(tokens.player);
        self.retrying = Some(tokens.retrying);
        // Java sets `version = -1` (:422) and then runs the update ladder, whose
        // four steps all apply to a version that low and whose last one writes 15
        // (`UpdateHandler.java:50-65`). One assignment says the same thing.
        self.version = CURRENT_VERSION;
    }

    /// `serializeNBT` (`:432-448`): nine keys, in Java's order.
    ///
    /// Refused when a value is missing, where Java would serialize a null token.
    pub fn write(&self) -> Result<Compound, Refusal> {
        let mut saved = Compound::from([(KEY_VERSION.into(), Tag::Int(self.version))]);
        saved.insert(KEY_DATA_STORES.into(), Tag::Compound(self.book.write()));
        for kind in Kind::ALL {
            let Some(token) = self.stores.get(&kind).copied() else {
                return Err(Refusal::NotSetUp { key: kind.key() });
            };
            saved.insert(kind.key().into(), super::datastores::token_tag(token));
        }
        let Some(player) = self.player else {
            return Err(Refusal::NotSetUp { key: KEY_PLAYER });
        };
        let Some(retrying) = self.retrying else {
            return Err(Refusal::NotSetUp { key: KEY_RETRYING });
        };
        saved.insert(KEY_PLAYER.into(), super::datastores::token_tag(player));
        saved.insert(KEY_RETRYING.into(), super::datastores::token_tag(retrying));
        Ok(saved)
    }

    /// `deserializeNBT` (`:456-521`): nine keys, each read or marked, then the
    /// orphan sweep, then the version test.
    ///
    /// **The order matters and is Java's.** The keys are read first (`:458-497`),
    /// the sweep runs on what was read (`:504-518`), and only then does
    /// `updateIfRequired` decide whether the whole thing is wiped (`:520`). A
    /// request whose resolver assignment is absent is dropped **and counted**
    /// (`:515-518`); `assigned` is the caller's answer to
    /// `getAssignmentForValue(id) != null`, which lives in a store this port does
    /// not model.
    pub fn load(
        &mut self,
        compound: &Compound,
        queue: &mut Queue,
        assigned: &dyn Fn(i32) -> bool,
    ) -> Loaded {
        let mut loaded = Loaded::default();

        // `:458-461` — the version, read with `getInt`.
        match compound.get(&KEY_VERSION.into()) {
            None => {
                loaded.mends.push(Mend { key: KEY_VERSION, reason: MendReason::Missing });
                self.version = -1;
            }
            Some(_) => match nbt::int(compound, KEY_VERSION) {
                Some(version) => self.version = version,
                None => {
                    loaded.mends.push(Mend { key: KEY_VERSION, reason: MendReason::Unreadable });
                    // `getInt` answers 0 for a tag that is not a number, and 0 is
                    // under 15.
                    self.version = 0;
                }
            }
        }

        // `:463-466` — the book. Java's `if (dataStoreManager == null) reset()`
        // is at `:499-502`: a present but empty book is *not* null.
        let mut book_missing = false;
        match nbt::compound(compound, KEY_DATA_STORES) {
            None => {
                loaded.mends.push(Mend { key: KEY_DATA_STORES, reason: MendReason::Missing });
                self.book = Book::new();
                book_missing = true;
            }
            Some(saved) => {
                let (book, read) = Book::read(saved);
                self.book = book;
                loaded.book_mends = read.mends;
                loaded.skipped = read.skipped;
            }
        }

        // `:468-497` — five store ids, then the player and the retrying resolver.
        self.stores.clear();
        for kind in Kind::ALL {
            if let Some(token) = read_token(compound, kind.key(), &mut loaded.mends) {
                self.stores.insert(kind, token);
            }
        }
        self.player = read_token(compound, KEY_PLAYER, &mut loaded.mends);
        self.retrying = read_token(compound, KEY_RETRYING, &mut loaded.mends);

        // `:504-518` — the orphan sweep.
        let orphans: Vec<Token> = queue
            .iter()
            .map(Request::id)
            .filter(|id| !assigned(*id))
            .collect();
        for id in &orphans {
            queue.remove(*id);
        }
        loaded.orphans = orphans;

        // `:663-669` — a save older than the ladder is reset, not migrated.
        loaded.needs_reset = book_missing || self.version < CURRENT_VERSION;
        loaded
    }

    /// `assignRequest` (`:230-234`): read, assign, mark dirty — in that order, so
    /// a refusal writes nothing.
    ///
    /// Java's `assignRequest` also leaves the request in `ASSIGNING` before it
    /// looks for a resolver (`RequestHandler.java:130`) and returns null when none
    /// is found (`:218-221`), state untouched. This crate has neither the state
    /// nor the search: `crate::requests::resolver::choose` answers who supplies,
    /// and this call files the answer.
    pub fn assign(&mut self, queue: &mut Queue, id: i32, by: [i32; 3]) -> Result<(), Refusal> {
        let Some(request) = queue.get_mut(id) else {
            return Err(Refusal::Unknown { id });
        };
        request.assign(by)?;
        self.mark_dirty();
        Ok(())
    }

    /// `begin` of the work: Java's `updateRequestState(token, IN_PROGRESS)`.
    pub fn begin(&mut self, queue: &mut Queue, id: i32) -> Result<FollowUp, Refusal> {
        let Some(request) = queue.get_mut(id) else {
            return Err(Refusal::Unknown { id });
        };
        request.begin()?;
        let state = request.state();
        self.mark_dirty();
        Ok(follow_up(state))
    }

    /// Hand `stack` in: Java's delivery, and the state change it causes.
    ///
    /// The follow-up is `Completed` when this delivery is the one that fills the
    /// request, `None` while it is partial — Java sets `COMPLETED` at that moment,
    /// never before (`AbstractRequest.java:137-154` has no such rule, so it is the
    /// resolver that decides; the port decides at the count).
    pub fn deliver(
        &mut self,
        queue: &mut Queue,
        id: i32,
        stack: &Stack,
    ) -> Result<(u32, FollowUp), Refusal> {
        let Some(request) = queue.get_mut(id) else {
            return Err(Refusal::Unknown { id });
        };
        let taken = request.deliver(stack)?;
        let follow = follow_up(request.state());
        self.mark_dirty();
        Ok((taken, follow))
    }

    /// `updateRequestState(token, CANCELLED)` (`:322-325`).
    pub fn cancel(&mut self, queue: &mut Queue, id: i32) -> Result<FollowUp, Refusal> {
        let Some(request) = queue.get_mut(id) else {
            return Err(Refusal::Unknown { id });
        };
        request.cancel()?;
        let state = request.state();
        self.mark_dirty();
        Ok(follow_up(state))
    }

    /// `reassignRequest` (`:253-260`): read, **mark dirty**, then try.
    ///
    /// The attempt itself is not here. Java cancels the old resolver's assignment
    /// and asks the resolver handler for another one (`RequestHandler.java:288-313`);
    /// this crate's `Request::assign` has no way back from `Assigned`, so the
    /// reopen is a gap, not an omission — `MANQUANT.md` §5 names it. What is here
    /// is the ordering rule, which is the half a caller cannot get wrong silently:
    /// the flag is set before the attempt, so a failed reassignment is still saved.
    pub fn reassign(&mut self, queue: &Queue, id: i32) -> Result<(), Refusal> {
        if queue.get(id).is_none() {
            return Err(Refusal::Unknown { id });
        }
        self.mark_dirty();
        Ok(())
    }

    /// `overruleRequest` (`:345-356`): the player's stack replaces what will
    /// arrive, then the request is overruled.
    ///
    /// **The refusal comes first here, which is the port's own order.** Java
    /// writes the override (`:350-353`) and then changes the state (`:355`), and
    /// its `setState` cannot fail — so its order is unobservable. The port's
    /// cancel **can** refuse (a request already `Delivered` or `Cancelled`), and a
    /// refused overrule that had already recorded the item would leave a stack
    /// nobody will ever receive. So the terminal check runs before the write, and
    /// a refusal leaves [`Manager::overruled_by`] empty.
    pub fn overrule(
        &mut self,
        queue: &mut Queue,
        id: i32,
        item: Option<ItemName>,
    ) -> Result<FollowUp, Refusal> {
        let Some(request) = queue.get_mut(id) else {
            return Err(Refusal::Unknown { id });
        };
        if request.state().is_done() {
            return Err(Refusal::Request(crate::requests::Refusal::IllegalTransition {
                from: request.state(),
                to: State::Cancelled,
            }));
        }
        if let Some(item) = item {
            self.overruled.insert(id, item);
        }
        request.cancel()?;
        let state = request.state();
        self.mark_dirty();
        Ok(follow_up(state))
    }
}

/// One of the eight token keys, read or marked (`:468-497`).
fn read_token(compound: &Compound, key: &'static str, mends: &mut Vec<Mend>) -> Option<Token> {
    match compound.get(&key.into()) {
        None => {
            mends.push(Mend { key, reason: MendReason::Missing });
            None
        }
        Some(tag) => match super::datastores::token_of(tag) {
            Ok(token) => Some(token),
            Err(_) => {
                mends.push(Mend { key, reason: MendReason::Unreadable });
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::Requester;

    const TOKENS: Tokens =
        Tokens { stores: [11, 12, 13, 14, 15], player: 16, retrying: 17 };
    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn set_up() -> Manager {
        let mut manager = Manager::new();
        manager.reset(&TOKENS);
        manager
    }

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn asking(id: i32, count: u32) -> Request {
        Request::new(id, ladder(), count, count, Requester::new("minecolonies:blockhutbuilder", HUT))
            .expect("a nonzero request")
    }

    fn queue(requests: Vec<Request>) -> Queue {
        let mut queue = Queue::new();
        for request in requests {
            queue.push(request).expect("a fresh id");
        }
        queue
    }

    fn stack(count: u32) -> Stack {
        Stack::new(ladder(), count).expect("a nonzero stack")
    }

    #[test]
    fn a_new_manager_is_unset_dirty_and_at_version_minus_one() {
        let manager = Manager::new();
        assert_eq!(manager.version(), -1, ":110");
        assert!(manager.is_dirty(), ":84 `dirty = true`");
        assert!(manager.player().is_none());
        assert!(manager.store(Kind::RequestIdentities).is_none());
        assert_eq!(manager.write(), Err(Refusal::NotSetUp { key: Kind::RequestIdentities.key() }));
    }

    #[test]
    fn a_reset_files_the_five_stores_under_the_ids_it_is_given() {
        let manager = set_up();
        assert_eq!(manager.version(), CURRENT_VERSION, ":422-423 plus the ladder");
        assert_eq!(manager.player(), Some(TOKENS.player));
        assert_eq!(manager.retrying(), Some(TOKENS.retrying));
        for kind in Kind::ALL {
            assert_eq!(manager.store_id(kind), Some(TOKENS.store(kind)), "{kind:?}");
            let store = manager.store(kind).expect("just created");
            assert_eq!(store.id(), TOKENS.store(kind), ":52 id set to the key");
        }
        assert_eq!(manager.book().len(), 5);
    }

    #[test]
    fn the_save_is_nine_keys_and_the_read_brings_them_all_back() {
        let manager = set_up();
        let saved = manager.write().expect("a set-up manager writes");
        for key in [
            KEY_VERSION,
            KEY_DATA_STORES,
            KEY_REQUEST_IDENTITIES,
            KEY_RESOLVER_IDENTITIES,
            KEY_PROVIDER_ASSIGNMENTS,
            KEY_RESOLVER_ASSIGNMENTS,
            KEY_REQUESTABLE_ASSIGNMENTS,
            KEY_PLAYER,
            KEY_RETRYING,
        ] {
            assert!(nbt::has(&saved, key), "{key} is missing from the save");
        }
        let mut read = Manager::new();
        let mut queue = Queue::new();
        let loaded = read.load(&saved, &mut queue, &|_: i32| true);
        assert_eq!(loaded.mends, Vec::new(), "an intact save marks nothing");
        assert!(!loaded.needs_reset);
        assert_eq!(read.version(), CURRENT_VERSION);
        assert_eq!(read.store_id(Kind::ResolverAssignments), Some(14));
        assert_eq!(read.player(), Some(16));
        assert_eq!(read.book().len(), 5);
    }

    #[test]
    fn a_save_older_than_the_ladder_is_wiped_not_migrated() {
        let mut manager = Manager::new();
        let saved = Compound::from([(KEY_VERSION.into(), Tag::Int(4))]);
        let mut queue = Queue::new();
        let loaded = manager.load(&saved, &mut queue, &|_: i32| true);
        assert!(loaded.needs_reset, ":665 version < getCurrentVersion()");
        assert_eq!(manager.version(), 4, "the read version stays until the caller resets");
    }

    #[test]
    fn a_missing_book_or_a_missing_key_asks_for_a_reset_and_names_the_key() {
        let saved = Compound::from([(KEY_VERSION.into(), Tag::Int(CURRENT_VERSION))]);
        let mut manager = Manager::new();
        let loaded = manager.load(&saved, &mut Queue::new(), &|_: i32| true);
        assert!(loaded.needs_reset, ":499-502 a null book is reset");
        let keys: Vec<&'static str> = loaded.mends.iter().map(|mend| mend.key).collect();
        assert_eq!(keys[0], KEY_DATA_STORES);
        assert!(keys.contains(&KEY_PLAYER));
        assert!(loaded
            .mends
            .iter()
            .all(|mend| mend.reason == MendReason::Missing));
        assert_eq!(loaded.mends.len(), 8, "the book and the seven token keys");
    }

    #[test]
    fn a_version_of_the_wrong_kind_reads_as_zero_and_wipes_the_system() {
        let saved = Compound::from([(KEY_VERSION.into(), Tag::String("15".into()))]);
        let mut manager = Manager::new();
        let loaded = manager.load(&saved, &mut Queue::new(), &|_: i32| true);
        assert_eq!(
            loaded.mends[0],
            Mend { key: KEY_VERSION, reason: MendReason::Unreadable }
        );
        assert_eq!(manager.version(), 0, "getInt answers 0 for a tag it cannot read");
        assert!(loaded.needs_reset);
    }

    #[test]
    fn the_orphan_sweep_drops_a_request_nobody_was_assigned_and_counts_it() {
        let manager = set_up();
        let saved = manager.write().expect("a set-up manager writes");
        let mut queue = queue(vec![asking(1, 4), asking(2, 4), asking(3, 4)]);
        let mut read = Manager::new();
        let loaded = read.load(&saved, &mut queue, &|id| id != 2);
        assert_eq!(loaded.orphans, vec![2], ":508-512 assignment absent");
        assert_eq!(queue.iter().map(Request::id).collect::<Vec<_>>(), vec![1, 3]);
        assert!(queue.get(2).is_none());
    }

    #[test]
    fn assign_refuses_an_unknown_request_without_write_and_keeps_the_state() {
        let mut manager = set_up();
        manager.set_dirty(false);
        let mut queue = queue(vec![asking(1, 4)]);
        assert_eq!(manager.assign(&mut queue, 99, HUT), Err(Refusal::Unknown { id: 99 }));
        assert!(!manager.is_dirty(), ":232 getRequest throws before markDirty");
        // A request already held by another hut: the move is refused, and nothing
        // is marked either.
        manager.assign(&mut queue, 1, OTHER).expect("a fresh request");
        manager.set_dirty(false);
        let refused = manager.assign(&mut queue, 1, HUT);
        assert!(matches!(refused, Err(Refusal::Request(crate::requests::Refusal::AlreadyAssigned { .. }))));
        assert!(!manager.is_dirty(), "a refused move marks nothing");
        assert_eq!(
            queue.get(1).expect("still there").assigned_to(),
            Some(OTHER),
            "the first assignment stands"
        );
    }

    #[test]
    fn begin_needs_an_assigned_request_and_marks_dirty_only_on_success() {
        let mut manager = set_up();
        let mut queue = queue(vec![asking(1, 4)]);
        manager.set_dirty(false);
        assert!(manager.begin(&mut queue, 1).is_err(), "Created has no door to InProgress");
        assert!(!manager.is_dirty());
        assert_eq!(queue.get(1).expect("still there").state(), State::Created);
        manager.assign(&mut queue, 1, HUT).expect("assignable");
        manager.set_dirty(false);
        assert_eq!(manager.begin(&mut queue, 1), Ok(FollowUp::None));
        assert!(manager.is_dirty(), ":302 marks dirty after the write");
    }

    #[test]
    fn deliver_completes_only_on_the_stack_that_fills_the_request() {
        let mut manager = set_up();
        let mut queue = queue(vec![asking(1, 4)]);
        manager.assign(&mut queue, 1, HUT).expect("assignable");
        manager.begin(&mut queue, 1).expect("begun");
        assert_eq!(manager.deliver(&mut queue, 1, &stack(3)), Ok((3, FollowUp::None)));
        let (taken, follow) = manager.deliver(&mut queue, 1, &stack(3)).expect("the rest fits");
        assert_eq!((taken, follow), (1, FollowUp::Completed), "COMPLETED, :310-313");
        assert_eq!(queue.get(1).expect("still there").state(), State::Delivered);
    }

    #[test]
    fn a_wrong_item_is_refused_before_anything_is_written() {
        let mut manager = set_up();
        let mut queue = queue(vec![asking(1, 4)]);
        manager.assign(&mut queue, 1, HUT).expect("assignable");
        manager.set_dirty(false);
        let other = ItemName::new("minecraft:stone").expect("a valid name");
        let refused = manager.deliver(&mut queue, 1, &Stack::new(other, 2).expect("a stack"));
        assert!(matches!(refused, Err(Refusal::Request(crate::requests::Refusal::WrongItem))));
        let request = queue.get(1).expect("still there");
        assert_eq!(request.delivered(), 0, "nothing was taken");
        assert!(!manager.is_dirty());
    }

    #[test]
    fn cancel_is_terminal_and_a_second_one_is_refused_without_a_write() {
        let mut manager = set_up();
        let mut queue = queue(vec![asking(1, 4)]);
        assert_eq!(manager.cancel(&mut queue, 1), Ok(FollowUp::Cancelled), ":322-325");
        manager.set_dirty(false);
        assert!(manager.cancel(&mut queue, 1).is_err(), ":322 only an open request moves");
        assert!(!manager.is_dirty());
    }

    #[test]
    fn overrule_records_the_stack_then_cancels_and_writes_nothing_when_it_refuses() {
        let mut manager = set_up();
        let mut queue = queue(vec![asking(1, 4), asking(2, 4)]);
        // A live request: the stack is recorded and the request becomes terminal.
        let item = ItemName::new("minecraft:oak_planks").expect("a valid name");
        assert_eq!(manager.overrule(&mut queue, 1, Some(item.clone())), Ok(FollowUp::Cancelled));
        assert_eq!(manager.overruled_by(1), Some(&item), ":352 overrideCurrentDeliveries");
        // A request that is already terminal: refused, and **nothing** recorded.
        assert!(manager.overrule(&mut queue, 1, Some(item.clone())).is_err());
        assert_eq!(
            manager.overruled_by(1),
            Some(&item),
            "the first stack stands, no second write"
        );
        assert_eq!(manager.overrule(&mut queue, 2, None), Ok(FollowUp::Cancelled));
        assert_eq!(manager.overruled_by(2), None, "no stack means no override at all");
        assert_eq!(manager.overrule(&mut queue, 99, None), Err(Refusal::Unknown { id: 99 }));
    }

    #[test]
    fn reassign_marks_dirty_before_the_attempt_where_assign_marks_it_after() {
        let mut manager = set_up();
        let queue = queue(vec![asking(1, 4)]);
        // Reassign: unknown refuses first (:257), so nothing is marked.
        manager.set_dirty(false);
        assert_eq!(manager.reassign(&queue, 99), Err(Refusal::Unknown { id: 99 }));
        assert!(!manager.is_dirty());
        // A known request is marked **before** the attempt (:258).
        assert_eq!(manager.reassign(&queue, 1), Ok(()));
        assert!(manager.is_dirty(), "the flag is set even though nothing moved");
    }

    #[test]
    fn set_dirty_answers_whether_the_colony_must_be_marked_and_does_it_every_time() {
        let mut manager = set_up();
        assert!(manager.set_dirty(true), ":219 colony.markDirty() on every true");
        assert!(!manager.set_dirty(false), ":215 storing false marks nothing");
        assert!(!manager.is_dirty());
        assert!(manager.set_dirty(true));
        assert!(manager.is_dirty());
    }

    #[test]
    fn the_tick_learns_about_the_manager_before_it_retries() {
        assert_eq!(TICK_ORDER, [TickStep::UpdateManager, TickStep::Retry], ":594-595");
    }

    #[test]
    fn every_state_has_the_follow_up_java_gives_it() {
        assert_eq!(follow_up(State::Created), FollowUp::None, ":330 the default");
        assert_eq!(follow_up(State::Assigned), FollowUp::None);
        assert_eq!(follow_up(State::InProgress), FollowUp::None);
        assert_eq!(follow_up(State::Delivered), FollowUp::Completed, ":310-313");
        assert_eq!(follow_up(State::Cancelled), FollowUp::Cancelled, ":322-325");
        let names = [FollowUp::None.name(), FollowUp::Completed.name(), FollowUp::Cancelled.name()];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two follow-ups share a name");
            }
        }
    }

    #[test]
    fn every_kind_has_its_own_key_and_name() {
        let keys: Vec<&'static str> = Kind::ALL.iter().map(|kind| kind.key()).collect();
        for (index, key) in keys.iter().enumerate() {
            for other in &keys[index + 1..] {
                assert_ne!(key, other, "two stores share a key, so one is lost");
            }
        }
        for kind in Kind::ALL {
            assert!(kind.name().starts_with("requests.store."), "{kind:?}");
        }
        assert_eq!(Kind::ALL[0].key(), KEY_REQUEST_IDENTITIES, ":129 setup order");
        assert_eq!(Kind::ALL[4].key(), KEY_REQUESTABLE_ASSIGNMENTS, ":133");
    }

    #[test]
    fn every_refusal_and_mend_has_its_own_name() {
        let names = [
            Refusal::Unknown { id: 1 }.name(),
            Refusal::NotSetUp { key: KEY_PLAYER }.name(),
            MendReason::Missing.name(),
            MendReason::Unreadable.name(),
        ];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two names collide, so a log line lies");
            }
        }
        // A wrapped request refusal keeps its own name, dotted and all.
        assert_eq!(
            Refusal::Request(crate::requests::Refusal::WrongItem).name(),
            "requests.refusal.item"
        );
    }
}
