//! The host a hook sees in a test: it answers the calls this slice makes, records
//! every one of them, and panics on anything else.
//!
//! **Only what the slice calls is implemented, on purpose.** Every method left out
//! keeps the default the SDK's `#[host_area]` macro writes — a panic naming the
//! method and its load phase (`crates/horizon-mod-api-macros/src/lib.rs:285-334`,
//! filling each default with `world::phase::unprovided`). That is the
//! `unimplemented!()` a fake host should have, and a better one than a hand-written
//! stub: regenerated from the declarations, it cannot drift, and it fails in the test
//! that made the call. A fake answering everything would let a port that started
//! asking something new pass in silence, which is the failure this module exists
//! to catch.
//!
//! Handles are built through the game's own hidden constructors —
//! `__private::game::{block_entity_ref, player_ref}`, `EntityRef::new`,
//! `MobRef::new` — because a mod has no other way. `RAPPORT_PORT_HARNESS.md` says
//! which and why; nothing here adds a line to `crates/`.

use std::cell::RefCell;

use mod_api::__private::game::{BackendMutRefOf, BackendRefOf, block_entity_ref, player_ref};
use mod_api::prelude::*;
use mod_api::{
    Backend, BackendMut, BackendMutRef, BackendRef, BlockEntityError, BlockEntityRef, BlockKey,
    Compound, Commands, EntityJoinLevelEvent, EntityRef, Host, LevelHost, LevelRef, Levels,
    MobGoalEntry, MobRef, PlayerRef, Players, ServerHost, SpawnReason, Text, TextFormError,
    WorldEpoch,
};

use crate::citizen;
use crate::harness::calls::Call;
use crate::harness::level::{EPOCH, FakeLevel};

/// The entity id the acting player is, in the fake world.
pub const PLAYER_ENTITY: u32 = 100;

/// The goal selector a spawned mob starts with: a wandering trader's, the template
/// `citizen::KIND` copies. Four distinct kinds, so `remove_goal` is called four times.
const TRADER_GOALS: [(i32, &str); 4] = [
    (0, "float"),
    (1, "panic"),
    (5, "stroll"),
    (6, "look_at_player"),
];

/// The server host of a test: one world, one player, and the log of what was asked.
pub struct FakeHost {
    /// The world: blocks, block entity data, mobs.
    pub level: FakeLevel,
    acting: Option<PlayerRef>,
    /// Every call, in order. A `RefCell` because half the host reads take `&self`:
    /// recording must not need the host to be mutable.
    calls: RefCell<Vec<Call>>,
    /// The `(position, handle)` pairs handed out. A `BlockEntityRef` has no accessor
    /// for its position — `position` is itself a host read — so the fake remembers
    /// what it minted and finds it back by equality.
    handles: RefCell<Vec<([i32; 3], BlockEntityRef)>>,
    next_entity: u32,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeHost {
    /// An empty overworld, no player, no call made yet.
    pub fn new() -> Self {
        Self {
            level: FakeLevel::default(),
            acting: None,
            calls: RefCell::default(),
            handles: RefCell::default(),
            next_entity: 1,
        }
    }

    /// A player in this level, and the one a command or a click runs as.
    pub fn join(&mut self, uuid: u128) {
        let entity = EntityRef::new(WorldEpoch::new(EPOCH), PLAYER_ENTITY);
        self.acting = Some(player_ref(entity, uuid));
    }

    /// Every call the port made, in the order it made them.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    /// Their names only, for an order assertion that ignores the arguments.
    pub fn tags(&self) -> Vec<&'static str> {
        self.calls().iter().map(Call::tag).collect()
    }

    fn record(&self, call: Call) {
        self.calls.borrow_mut().push(call);
    }

    /// The position of a handle this fake minted; `None` for one of elsewhere.
    fn position(&self, entity: BlockEntityRef) -> Option<[i32; 3]> {
        self.handles
            .borrow()
            .iter()
            .find(|(_, minted)| *minted == entity)
            .map(|(pos, _)| *pos)
    }

    /// The handle for the block entity at `pos`, minted once and then always the
    /// same value, as the game gives one handle per block entity.
    fn mint(&self, level: LevelRef, pos: [i32; 3]) -> BlockEntityRef {
        let handle = block_entity_ref(level, pos, 0);
        let mut known = self.handles.borrow_mut();
        if !known.iter().any(|(_, minted)| *minted == handle) {
            known.push((pos, handle));
        }
        handle
    }
}

impl KeysHost for FakeHost {
    /// `host.block_key(hut::BLOCK_ID)`. There is no block registry here, so **every**
    /// name resolves; what the port compares is `level.block(pos)` against this key,
    /// and both sides come from `harness::block_key`.
    fn block_key(&self, id: &str) -> Option<BlockKey> {
        self.record(Call::BlockKey(id.to_owned()));
        Some(crate::harness::block_key(id))
    }
}

impl BlockEntitiesHost for FakeHost {
    /// A handle only where a block entity with saved data stands, as the engine finds
    /// nothing in a block that has none.
    fn block_entity(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockEntityRef> {
        self.record(Call::BlockEntity(pos));
        (level == self.level.level() && self.level.has_data(pos)).then(|| self.mint(level, pos))
    }
}

impl EntitiesHost for FakeHost {
    fn entity(&self, id: u32) -> Option<EntityRef> {
        self.record(Call::Entity(id));
        self.level
            .mob(id)
            .map(|_| EntityRef::new(WorldEpoch::new(EPOCH), id))
    }
}

impl MobsHost for FakeHost {
    fn mob(&self, entity: EntityRef) -> Option<MobRef> {
        self.record(Call::Mob(entity.id()));
        self.level.mob(entity.id()).map(|_| MobRef::new(entity))
    }
}

impl ServicesServerHost for FakeHost {
    fn commands(&mut self) -> &mut dyn Commands {
        self
    }
    fn levels(&self) -> &dyn Levels {
        self
    }
    fn players(&self) -> &dyn Players {
        self
    }
}

impl Commands for FakeHost {
    /// `host.commands().feedback(line)`: kept in the call log, so a test reads what
    /// the player would have seen.
    fn feedback(&mut self, line: Text) -> Result<(), TextFormError> {
        self.record(Call::Feedback(line));
        Ok(())
    }
}

impl Players for FakeHost {
    fn acting_player(&self) -> Option<PlayerRef> {
        self.record(Call::ActingPlayer);
        self.acting
    }
}

impl Levels for FakeHost {
    /// The one level a test has: the answer to "which dimension am I in".
    fn list_levels(&self) -> Vec<LevelRef> {
        vec![self.level.level()]
    }
}

impl BlockEntitiesBackend for FakeHost {
    fn block_entity_data(&self, entity: BlockEntityRef) -> Option<Compound> {
        // A handle of elsewhere records nothing: the game answers `None` for a stale
        // entity without reading a chunk, and there is no position to log.
        let pos = self.position(entity)?;
        self.record(Call::Data(pos));
        self.level.data_at(pos).cloned()
    }
}

impl LevelsBackend for FakeHost {
    fn level_block(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockKey> {
        self.record(Call::Block(pos));
        (level == self.level.level())
            .then(|| self.level.block_at(pos))
            .flatten()
    }

    fn level_block_property(&self, level: LevelRef, pos: [i32; 3], key: &str) -> Option<String> {
        self.record(Call::Property {
            at: pos,
            key: key.to_owned(),
        });
        (level == self.level.level())
            .then(|| self.level.property_at(pos, key))
            .flatten()
    }
}

impl MobsBackend for FakeHost {
    fn mob_goals(&self, mob: MobRef) -> Option<Vec<MobGoalEntry>> {
        let id = mob.entity().id();
        self.record(Call::Goals(id));
        self.level.mob(id).map(|mob| mob.goals.clone())
    }
}

impl PlayersBackend for FakeHost {
    /// The acting player's level; `None` for anyone else, as a stale player reads.
    fn player_level(&self, player: PlayerRef) -> Option<LevelRef> {
        self.record(Call::PlayerLevel);
        (Some(player) == self.acting).then(|| self.level.level())
    }
}

/// Empty: no client read is reachable from a server hook. The skip list of
/// `impl_backend_areas!` drops both `players` reads at once, so `Backend` needs this
/// one named even though nothing calls it.
impl PlayersClientBackend for FakeHost {}

impl BlockEntitiesBackendMut for FakeHost {
    /// `entity.merge_data(host, payload)`: the write a hire makes. Merging keeps every
    /// key the payload does not name, which is why `hut::with_hired` carries one key.
    fn block_entity_merge_data(
        &mut self,
        entity: BlockEntityRef,
        data: Compound,
    ) -> Result<bool, BlockEntityError> {
        let pos = self.position(entity).ok_or(BlockEntityError::Stale)?;
        self.record(Call::Merge {
            at: pos,
            data: data.clone(),
        });
        Ok(self.level.merge_at(pos, data))
    }
}

impl EntitiesBackendMut for FakeHost {
    fn entity_set_custom_name(
        &mut self,
        entity: EntityRef,
        name: Option<Text>,
    ) -> Result<bool, TextFormError> {
        self.record(Call::Name(name.clone()));
        let named = self.level.mob_mut(entity.id()).map(|mob| mob.name = name);
        Ok(named.is_some())
    }
}

impl MobsBackendMut for FakeHost {
    fn mob_remove_goal(&mut self, mob: MobRef, id: &str) -> Option<usize> {
        self.record(Call::RemoveGoal(id.to_owned()));
        let target = self.level.mob_mut(mob.entity().id())?;
        let before = target.goals.len();
        target.goals.retain(|goal| goal.id != id);
        Some(before - target.goals.len())
    }

    fn mob_set_persistence_required(&mut self, mob: MobRef) -> bool {
        let id = mob.entity().id();
        self.record(Call::Persist(id));
        self.level
            .mob_mut(id)
            .map(|mob| mob.persistent = true)
            .is_some()
    }

    fn mob_move_to(&mut self, mob: MobRef, at: [f64; 3], speed: f64) -> bool {
        let id = mob.entity().id();
        self.record(Call::MoveTo { at, speed });
        self.level
            .mob_mut(id)
            .map(|mob| mob.path = Some((at, speed)))
            .is_some()
    }
}

impl PlayersBackendMut for FakeHost {
    fn player_action_bar(&mut self, player: PlayerRef, line: Text) -> Result<bool, TextFormError> {
        if Some(player) != self.acting {
            return Ok(false);
        }
        self.record(Call::ActionBar(line));
        Ok(true)
    }
}

impl LevelsBackendMut for FakeHost {
    /// `LevelRef::spawn_mob`: a mob appears **and joins the level**, because the engine
    /// fires `entity_join_level` inside the spawn
    /// (`crates/horizon-mod-vocab/src/host/levels.rs:793-816`). Reproducing that is the
    /// point: `hire` relies on it, since the citizen's goals are gone before it orders
    /// the walk.
    fn level_spawn_mob(
        &mut self,
        level: LevelRef,
        kind: &str,
        pos: [f64; 3],
        _reason: SpawnReason,
    ) -> Option<EntityRef> {
        self.record(Call::Spawn {
            kind: kind.to_owned(),
            at: pos,
        });
        if level != self.level.level() {
            return None;
        }
        let id = self.next_entity;
        self.next_entity += 1;
        let goals = TRADER_GOALS
            .iter()
            .map(|(priority, goal)| MobGoalEntry {
                priority: *priority,
                id: *goal,
                running: false,
            })
            .collect();
        self.level.spawn(id, kind, goals);
        let mut event = EntityJoinLevelEvent {
            dimension: self.level.dimension(),
            entity: id,
            kind,
            pos,
            loaded_from_disk: false,
            cancelled: false,
        };
        citizen::join(self, &mut event);
        Some(EntityRef::new(WorldEpoch::new(EPOCH), id))
    }
}

impl Host for FakeHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(<BackendRef<'_> as BackendRefOf<'_>>::of(self))
    }
}

impl ServerHost for FakeHost {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(<BackendMutRef<'_> as BackendMutRefOf<'_>>::of(self))
    }
}

impl Backend for FakeHost {}
impl BackendMut for FakeHost {}

/// A `block_entity` Part is handed a `LevelSide`, which this hut ignores; the trait's
/// own defaults are the whole shape, with no level reads behind it.
impl LevelHost for FakeHost {}

// The areas this slice never touches keep the SDK's panicking defaults. Listing only
// the ones above is the contract: a call outside it fails loudly, in the test that
// made it, naming the method.
mod_api::impl_host_areas!(FakeHost, except(block_entities, entities, keys, mobs));
mod_api::impl_server_host_areas!(FakeHost, except(services));
mod_api::impl_backend_areas!(FakeHost, except(block_entities, levels, mobs, players));
mod_api::impl_backend_mut_areas!(
    FakeHost,
    except(block_entities, entities, levels, mobs, players)
);
