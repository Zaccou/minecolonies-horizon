//! The table: one row per branch of the thirteen Java classes of `core/event/`, and
//! what the port does with it.
//!
//! Java reference: `core/event/` — **2 951** lines over 14 files, measured with
//! `wc -l`. Thirteen of them hold code (2 947 lines); `package-info.java` holds four
//! lines of package documentation and nothing else, which is why [`Listener::ALL`]
//! has thirteen entries and [`CODE_LINES`] plus [`PACKAGE_INFO_LINES`] is the 2 951
//! the brief counted.
//!
//! **Why a table and not thirteen modules.** A listener here is not a subsystem: it
//! is a thin `@SubscribeEvent` method that reads a Forge event and either cancels it
//! or calls into a class the port already has (`ColonyManager`, `ColonyUtils`,
//! `ChunkDataHelper`, `BuildingModule`…). Writing thirteen Rust files that each wrap
//! one `host.call` would have duplicated the *decisions* in thirteen places and the
//! *citations* in none. So the decisions that are worth a function are functions
//! (`event_handler`, `quest_objectives`, `colony_story`, `texture_reload`), and every
//! branch the Java takes is one row below, with its `fichier.java:ligne`, its side,
//! the rule it applies, what it refuses, and **where the port keeps it**.
//!
//! [`Carrier`] is the interesting column, and it has five values:
//!
//! | Carrier | Meaning |
//! | --- | --- |
//! | [`Carrier::Ported`] | another module of this port already applies this rule; the row adds no code, it records where the rule lives |
//! | [`Carrier::Here`] | this module applies it, as a function the caller feeds |
//! | [`Carrier::Hook`] | the rule needs the game, and a hook of `crates/horizon-mod-vocab/src/hooks/` carries it |
//! | [`Carrier::Missing`] | **no hook can carry it**; the primitive that would is named in `MANQUANT.md` |
//! | [`Carrier::Superseded`] | the port's own design removes the failure the Java defends against |
//!
//! **The counts are the finding, not the decoration.** 40 of the rows are `Hook`, 47
//! are `Here`, 6 `Ported`, 17 `Missing` and one `Superseded`: the event layer leans on
//! Forge's own bus more than any other part of MineColonies, and one branch in six has
//! no counterpart at all. Four of those gaps organise `MANQUANT.md`:
//!
//! 1. **No capability-attachment event.** `AttachCapabilitiesEvent<LevelChunk>` and
//!    `<Level>` (`:242-258`) are how three of the thirteen classes exist. Horizon has
//!    `AttachmentType` on chunk and level holders — the *storage* exists — but a mod
//!    declares an attachment and reads it; nothing announces "a level was built".
//! 2. **No data generator.** `GatherDataHandler` runs under `runData`, never in game:
//!    it is not a missing hook, it is a missing build step (`cargo horizon-mod` has
//!    no datagen).
//! 3. **A hook that hands the wrong context.** `farmland_trample` is given a
//!    `LoopContext` (registries, settings, mod list) and no world, so the two facts
//!    its rule needs — the citizen's job and the colony's `SOFT_SHOES` research —
//!    cannot be read, and the branch cannot be ported honestly.
//! 4. **A property hook that carries no holder.** `ItemModelRangePropertyEvent` gives
//!    the stack, not the entity, so the spear's `throwing` (which asks
//!    `entity.isUsingItem()`) has no input — while the goggles' `disabled`, a flag the
//!    mod owns, does.
//!
//! **One gap the first reading invented, and the second closed.** `server_tick` was
//! filed `Missing` for want of a tick duration; `ServerHost::tick_times()`
//! (`host/config.rs:96-102`) answers the same hundred durations as
//! `getTickTimesNanos`, laid out `n % 100`, and `event_handler::tick_duration_ms`
//! reads them. The row is `Hook` and `MANQUANT.md` says why it is not a gap: a row
//! that stays missing only because nobody looked is the worst kind of row.
//!
//! Every `Missing` row names the primitive that would close it. None of them is
//! imitated: an imitation would be a rule the port applies with a value it invented.

/// The thirteen classes of `core/event/` that hold code.
///
/// `package-info.java` is not one of them: it declares a package and documents it
/// (`:1-4`), and the port has nothing to run for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: mirrors a Java directory, not a `mod_api` type
pub enum Listener {
    /// `EventHandler.java`, 876 lines, the Forge-bus half of the mod: 26 branches.
    EventHandler,
    /// `ClientEventHandler.java`, 506 lines, `@OnlyIn(Dist.CLIENT)`.
    ClientEventHandler,
    /// `FMLEventHandler.java`, 82 lines, the load-order and per-tick delegation.
    FmlEventHandler,
    /// `DataPackSyncEventHandler.java`, 156 lines, two nested subscriber classes.
    DataPackSync,
    /// `GatherDataHandler.java`, 73 lines, `runData` only.
    GatherData,
    /// `QuestObjectiveEventHandler.java`, 319 lines, five registries.
    QuestObjectives,
    /// `ClientChunkUpdatedEvent.java`, 35 lines, an event class and no listener.
    ClientChunkUpdated,
    /// `ColonyStoryListener.java`, 264 lines, a client reload listener.
    ColonyStory,
    /// `TextureReloadListener.java`, 76 lines, a client reload listener.
    TextureReload,
    /// `ClientRegistryHandler.java`, 388 lines, model layers and renderers.
    ClientRegistry,
    /// `capabilityproviders/MinecoloniesChunkCapabilityProvider.java`, 55 lines.
    ChunkCapability,
    /// `capabilityproviders/MinecoloniesWorldCapabilityProvider.java`, 56 lines.
    WorldChunkCapability,
    /// `capabilityproviders/MinecoloniesWorldColonyManagerCapabilityProvider.java`, 61 lines.
    WorldColonyManagerCapability,
}

impl Listener {
    /// All thirteen, in the order `ls` gives them.
    pub const ALL: &'static [Listener] = &[
        Listener::EventHandler,
        Listener::ClientEventHandler,
        Listener::FmlEventHandler,
        Listener::DataPackSync,
        Listener::GatherData,
        Listener::QuestObjectives,
        Listener::ClientChunkUpdated,
        Listener::ColonyStory,
        Listener::TextureReload,
        Listener::ClientRegistry,
        Listener::ChunkCapability,
        Listener::WorldChunkCapability,
        Listener::WorldColonyManagerCapability,
    ];

    /// The Java file, as `wc -l` named it.
    pub fn java_file(self) -> &'static str {
        match self {
            Listener::EventHandler => "EventHandler.java",
            Listener::ClientEventHandler => "ClientEventHandler.java",
            Listener::FmlEventHandler => "FMLEventHandler.java",
            Listener::DataPackSync => "DataPackSyncEventHandler.java",
            Listener::GatherData => "GatherDataHandler.java",
            Listener::QuestObjectives => "QuestObjectiveEventHandler.java",
            Listener::ClientChunkUpdated => "ClientChunkUpdatedEvent.java",
            Listener::ColonyStory => "ColonyStoryListener.java",
            Listener::TextureReload => "TextureReloadListener.java",
            Listener::ClientRegistry => "ClientRegistryHandler.java",
            Listener::ChunkCapability => "capabilityproviders/MinecoloniesChunkCapabilityProvider.java",
            Listener::WorldChunkCapability => {
                "capabilityproviders/MinecoloniesWorldCapabilityProvider.java"
            }
            Listener::WorldColonyManagerCapability => {
                "capabilityproviders/MinecoloniesWorldColonyManagerCapabilityProvider.java"
            }
        }
    }

    /// The file's measured length, `wc -l`. The sum plus
    /// [`PACKAGE_INFO_LINES`] is the 2 951 the brief counted.
    pub fn lines(self) -> u32 {
        match self {
            Listener::EventHandler => 876,
            Listener::ClientEventHandler => 506,
            Listener::FmlEventHandler => 82,
            Listener::DataPackSync => 156,
            Listener::GatherData => 73,
            Listener::QuestObjectives => 319,
            Listener::ClientChunkUpdated => 35,
            Listener::ColonyStory => 264,
            Listener::TextureReload => 76,
            Listener::ClientRegistry => 388,
            Listener::ChunkCapability => 55,
            Listener::WorldChunkCapability => 56,
            Listener::WorldColonyManagerCapability => 61,
        }
    }
}

/// `package-info.java`, four lines and no code.
pub const PACKAGE_INFO_LINES: u32 = 4;

/// The thirteen files' lines, summed: 2 947.
pub const CODE_LINES: u32 = 2947;

/// Which side a branch runs on, as the Java states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five answers the fourteen files give
pub enum Side {
    /// `!event.getLevel().isClientSide()`, or a `Server*` event.
    Server,
    /// `@OnlyIn(Dist.CLIENT)`, a `Client*` event, or `isClientSide()`.
    Client,
    /// The same method answers both, and the branch says which.
    Both,
    /// Runs while the server's data packs load, not on a tick.
    ServerDataLoad,
    /// Runs under `runData`, never in a game.
    Datagen,
}

/// Where the port keeps the rule of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four places a rule can live, and no fifth
pub enum Carrier {
    /// Another module of this port already applies it.
    Ported {
        /// The module, as `crate::…`.
        module: &'static str,
        /// The item in it, named the way that module names it.
        item: &'static str,
    },
    /// This module applies it; the entry point is named.
    Here {
        /// The function, as `module::function`.
        entry: &'static str,
    },
    /// A hook of `crates/horizon-mod-vocab/src/hooks/` carries it.
    Hook {
        /// The hook's method name.
        hook: &'static str,
    },
    /// No hook carries it; `MANQUANT.md` names the primitive that would.
    Missing {
        /// The primitive, in one sentence.
        needs: &'static str,
    },
    /// The port's own design makes the branch unnecessary, and this is why.
    Superseded {
        /// The module whose design supersedes it.
        module: &'static str,
        /// What that module does instead.
        item: &'static str,
        /// Why the Java's rule has no failure mode to cover here.
        why: &'static str,
    },
}

impl Carrier {
    /// A rule another module already applies.
    pub const fn ported(module: &'static str, item: &'static str) -> Carrier {
        Carrier::Ported { module, item }
    }

    /// A rule this module applies.
    pub const fn here(entry: &'static str) -> Carrier {
        Carrier::Here { entry }
    }

    /// A rule a hook carries.
    pub const fn hook(hook: &'static str) -> Carrier {
        Carrier::Hook { hook }
    }

    /// A rule no hook carries.
    pub const fn missing(needs: &'static str) -> Carrier {
        Carrier::Missing { needs }
    }

    /// A rule the port's design makes unnecessary.
    pub const fn superseded(
        module: &'static str,
        item: &'static str,
        why: &'static str,
    ) -> Carrier {
        Carrier::Superseded { module, item, why }
    }

    /// Which of the four this is, for counting.
    pub fn kind(self) -> Kind {
        match self {
            Carrier::Ported { .. } => Kind::Ported,
            Carrier::Here { .. } => Kind::Here,
            Carrier::Hook { .. } => Kind::Hook,
            Carrier::Missing { .. } => Kind::Missing,
            Carrier::Superseded { .. } => Kind::Superseded,
        }
    }
}

/// The four carriers, without their payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ported,
    Here,
    Hook,
    Missing,
    Superseded,
}

/// One branch of one Java method.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// The class the branch is written in.
    pub listener: Listener,
    /// `fichier.java:ligne` or `fichier.java:début-fin`, one source per row.
    pub at: &'static str,
    /// The event the Java subscribes to, as Forge names it.
    pub event: &'static str,
    /// The side the branch runs on.
    pub side: Side,
    /// What the branch does, in the Java's own terms.
    pub rule: &'static str,
    /// What it refuses, or `—` when it refuses nothing.
    pub refuses: &'static str,
    /// Where the port keeps it.
    pub carrier: Carrier,
}

impl Row {
    /// The three numbers the row cites: file, first line, last line.
    ///
    /// `"EventHandler.java:133-159"` → `("EventHandler.java", 133, 159)`, and
    /// `"…:352"` → `(…, 352, 352)`. A row whose `at` does not parse answers
    /// `None`, which the tests use to keep the citation format honest.
    pub fn lines(&self) -> Option<(&'static str, u32, u32)> {
        let (file, range) = self.at.split_once(':')?;
        let mut parts = range.split('-');
        let first: u32 = parts.next()?.parse().ok()?;
        let last: u32 = match parts.next() {
            Some(text) => text.parse().ok()?,
            None => first,
        };
        if last < first || parts.next().is_some() {
            return None;
        }
        Some((file, first, last))
    }

    /// Whether the branch names a hook the SDK does not have.
    pub fn missing(&self) -> bool {
        matches!(self.carrier, Carrier::Missing { .. })
    }
}

/// One row, in three lines instead of nine. `$carrier` is a `const fn` call.
macro_rules! row {
    ($listener:ident, $at:literal, $event:literal, $side:ident, $rule:literal, $refuses:literal, $carrier:expr) => {
        Row {
            listener: Listener::$listener,
            at: $at,
            event: $event,
            side: Side::$side,
            rule: $rule,
            refuses: $refuses,
            carrier: $carrier,
        }
    };
}

/// The table. One row per branch; the order is the Java's.
pub const ROWS: &[Row] = &[
    // ── EventHandler.java — the Forge bus, server ──────────────────────────────
    row!(EventHandler, "EventHandler.java:123-126", "RegisterCommandsEvent", Server, "`EntryPoint.register(dispatcher)`: the mod's command tree, no event data read", "—", Carrier::ported("crate::commands", "SPECS, the one table of roots and subcommands")),
    row!(EventHandler, "EventHandler.java:138-146", "EntityJoinLevelEvent (HIGHEST)", Server, "config `mobAttackCitizens` && Mob && Enemy && not blacklisted && not a fast colony entity: add target goals 6 (citizens, `!isInvisible`) and 7 (mercenaries)", "—", Carrier::hook("entity_join_level")),
    row!(EventHandler, "EventHandler.java:140", "EntityJoinLevelEvent", Server, "`!(entityType.is(ModTags.mobAttackBlacklist))` — the blacklist tag holds `minecraft:enderman` and `minecraft:llama`", "a blacklisted mob gets no goal", Carrier::missing("`EntityType::is(TagKey<EntityType<?>>)`: `ModTags.java:77` declares `mobAttackBlacklist` over entity types (two members, `DefaultEntityTypeTagsProvider.java:32`) and `TagsHost` serves item, fluid and block tags only (`world/tags.rs:34-70`)")),
    row!(EventHandler, "EventHandler.java:144", "EntityJoinLevelEvent", Server, "the citizen goal's `citizen -> !citizen.isInvisible()` predicate", "an invisible citizen is not targeted", Carrier::missing("a target-goal predicate: `GoalSpec::NearestTarget` takes a species filter (`MobFilter::Species`) and no test")),
    row!(EventHandler, "EventHandler.java:148-151", "EntityJoinLevelEvent", Server, "a fast colony entity whose UUID the level already holds: `setCanceled(true)`", "the duplicate join", Carrier::hook("entity_join_level")),
    row!(EventHandler, "EventHandler.java:153-157", "EntityJoinLevelEvent", Server, "a citizen whose `getCitizenColonyHandler().getColonyId() == 0`: log, then `setCanceled(true)`", "a citizen that belongs to no colony", Carrier::here("event_handler::join_verdict")),
    row!(EventHandler, "EventHandler.java:161-180", "LootTableLoadEvent", ServerDataLoad, "lazily build `cropDrops`: each crop block's `getDroppedFrom()` blocks' loot tables → the crops", "—", Carrier::here("event_handler::crop_table")),
    row!(EventHandler, "EventHandler.java:182-186", "LootTableLoadEvent", ServerDataLoad, "a matching table gets one pool per crop, base chance **0.001** on `minecraft:grass` and **0.01** elsewhere", "—", Carrier::here("event_handler::crop_chance")),
    row!(EventHandler, "EventHandler.java:191-194", "LootTableLoadEvent", ServerDataLoad, "`when(EntityInBiomeTag.of(crop.getPreferredBiome()))` when the crop declares one", "the crop does not drop outside its biome", Carrier::hook("loot_table_load")),
    row!(EventHandler, "EventHandler.java:195", "LootTableLoadEvent", ServerDataLoad, "`when(ModLootConditions.HAS_NO_SHEARS_OR_SILK_TOUCH)` on every crop pool", "no drop from a shears or silk-touch harvest", Carrier::hook("loot_condition")),
    row!(EventHandler, "EventHandler.java:197-219", "LootTableLoadEvent", ServerDataLoad, "the six-rung hoe ladder, `AlternativesEntry` in this order: netherite ×10, diamond ×8, iron ×6, golden ×5, any other hoe ×4, no hoe ×1", "a worse hoe cannot roll the better rung, and none of them stacks", Carrier::here("event_handler::crop_drop_chance")),
    row!(EventHandler, "EventHandler.java:225-234", "LootTableLoadEvent", ServerDataLoad, "`minecraft:chests/simple_dungeon` gets one extra pool: every crop at **0.005**", "—", Carrier::here("event_handler::DUNGEON_CROP_CHANCE")),
    row!(EventHandler, "EventHandler.java:242-246", "AttachCapabilitiesEvent<LevelChunk>", Server, "add `minecolonies:closecolony` → `MinecoloniesChunkCapabilityProvider`, backed by `IColonyTagCapability`", "—", Carrier::here("capability_providers::CLOSE_COLONY")),
    row!(EventHandler, "EventHandler.java:253-258", "AttachCapabilitiesEvent<Level>", Server, "add `minecolonies:chunkupdate` → `IChunkmanagerCapability`, and `minecolonies:colonymanager` → `IColonyManagerCapability` with `overworld = (dimension == OVERWORLD)`", "—", Carrier::here("capability_providers::CHUNK_UPDATE")),
    row!(EventHandler, "EventHandler.java:265-271", "ChunkEvent.Load", Server, "a `ServerLevel`: `ChunkDataHelper.loadChunk(chunk, level)`", "—", Carrier::hook("chunk_load")),
    row!(EventHandler, "EventHandler.java:272-278", "ChunkEvent.Load", Client, "a `ClientLevel`: `ChunkClientDataHelper.applyLate(chunk)` drains the queued `ChunkCapData`s whose `x`/`z` match the new chunk", "—", Carrier::hook("client_chunk_load")),
    row!(EventHandler, "EventHandler.java:286-293", "ChunkEvent.Unload", Server, "`ChunkDataHelper.unloadChunk(chunk, level)`", "—", Carrier::hook("chunk_unload")),
    row!(EventHandler, "EventHandler.java:300-320", "EntityTravelToDimensionEvent (LOWEST)", Server, "a `ServerPlayer` that is not already cancelled leaves the colony owning his old chunk: `removeVisitingPlayer` + `getPackageManager().removeCloseSubscriber`", "—", Carrier::hook("entity_travel_to_dimension")),
    row!(EventHandler, "EventHandler.java:327-344", "PlayerChangedDimensionEvent", Server, "the colony owning the chunk he arrives in: `addVisitingPlayer` + `addCloseSubscriber`", "—", Carrier::hook("player_changed_dimension")),
    row!(EventHandler, "EventHandler.java:352-355", "TickEvent.PlayerTickEvent (END)", Server, "the gate: END phase, not client, and `getGameTime() % 100 == 0`", "99 ticks in 100 do nothing", Carrier::here("event_handler::chunk_entry_due")),
    row!(EventHandler, "EventHandler.java:360-366", "TickEvent.PlayerTickEvent", Server, "the same chunk as last time → return; otherwise `playerPositions.put(uuid, chunkPos)`", "a player who did not move a chunk", Carrier::here("event_handler::chunk_entry_due")),
    row!(EventHandler, "EventHandler.java:368-373", "TickEvent.PlayerTickEvent", Server, "an empty chunk → return before any load or packet", "an empty chunk", Carrier::here("event_handler::chunk_entry_due")),
    row!(EventHandler, "EventHandler.java:375-381", "TickEvent.PlayerTickEvent", Server, "`ChunkDataHelper.loadChunk`, then `UpdateChunkRangeCapabilityMessage(world, x, z, 8, true)` to that player", "—", Carrier::hook("player_tick_post")),
    row!(EventHandler, "EventHandler.java:383-384", "TickEvent.PlayerTickEvent", Server, "`UpdateChunkCapabilityMessage(chunkCapData)` to that player — the four synced fields", "—", Carrier::ported("crate::network::common", "ChunkCap, the record that crosses")),
    row!(EventHandler, "EventHandler.java:387-396", "TickEvent.PlayerTickEvent", Server, "`if (chunkCapData.getOwningColony() != -1)`: `addVisitingPlayer` + `addCloseSubscriber` — **`-1` is never written anywhere** (see `chunk_entry_claim`)", "—", Carrier::here("event_handler::chunk_entry_claim")),
    row!(EventHandler, "EventHandler.java:399-416", "TickEvent.PlayerTickEvent", Server, "a claimed chunk: for each claiming building of each colony, `building.onPlayerEnterNearby(player)`", "—", Carrier::hook("player_tick_post")),
    row!(EventHandler, "EventHandler.java:424-459", "MobSpawnEvent.PositionCheck", Server, "an `Enemy` that is not from a spawner, in a loaded chunk, inside a level-1+ building of the owning colony: `Result.DENY`", "the spawn, inside a building", Carrier::hook("mob_spawn_position_check")),
    row!(EventHandler, "EventHandler.java:466-492", "PlayerEvent.PlayerLoggedInEvent", Server, "for every colony whose rank he has and `isColonyManager()`: `addImportantColonyPlayer` + colony view + permission packets; then any rally banner in his inventory broadcasts his position", "—", Carrier::hook("player_logged_in")),
    row!(EventHandler, "EventHandler.java:499-512", "PlayerEvent.PlayerLoggedOutEvent", Server, "for every colony: `removeCloseSubscriber` + `removeImportantColonyPlayer`; the `playerPositions.remove(uuid)` sits **inside** that loop, so a server with no colony never forgets a UUID", "—", Carrier::hook("player_logged_out")),
    row!(EventHandler, "EventHandler.java:517-543", "EntityCitizen chunk crossing (called by the entity)", Server, "`pvp_mode` and a guard job: a foreign colony's chunk → `colony.addGuardToAttackers(guard, playerToFollowOrRally)`. The Java calls it from a **20-tick poll** that memos the last chunk (`EntityCitizen.java:769-776`), not from a movement event", "—", Carrier::missing("a per-citizen chunk-crossing moment: `entity_entering_section` is a section, not a chunk, and carries no colony claim")),
    row!(EventHandler, "EventHandler.java:550-574", "BlockEvent.BreakEvent", Server, "a `SpawnerBlock` whose spawn data names `TAG_COLONY_ID`: `colony.getEventManager().onTileEntityBreak(TAG_EVENT_ID, spawner)`", "—", Carrier::hook("break_block")),
    row!(EventHandler, "EventHandler.java:591-608", "PlayerInteractEvent.RightClickBlock", Both, "not sneaking (or a sneak-bypass item) and the block is an `AbstractBlockHut`: `canRightClickWithoutPermissions` returns, else no `ACCESS_HUTS` → cancel", "the click, without `ACCESS_HUTS`", Carrier::ported("crate::blocks", "click → Refusal::LacksAccess")),
    row!(EventHandler, "EventHandler.java:610-633", "PlayerInteractEvent.RightClickBlock", Both, "a bed in a colony, foot half resolved to head: a citizen whose `getBedPos()` equals it and `isAsleep()` → cancel + `BASE_BED_OCCUPIED`", "sleeping in a citizen's bed", Carrier::here("event_handler::bed_blocks")),
    row!(EventHandler, "EventHandler.java:635-698", "PlayerInteractEvent.RightClickBlock", Both, "a held hut or scarecrow: client cancels outright, server cancels unless `onBlockHutPlaced` accepts", "the placement the ladder below refuses", Carrier::ported("crate::blocks", "place → the same ladder")),
    row!(EventHandler, "EventHandler.java:709-718", "PlayerInteractEvent (hut placement)", Server, "`allowOtherDimColonies == false` and the level is not an overworld type → `CANT_PLACE_COLONY_IN_OTHER_DIM`, refuse", "a colony in a non-overworld dimension", Carrier::here("event_handler::hut_placement")),
    row!(EventHandler, "EventHandler.java:720-753", "PlayerInteractEvent (hut placement)", Server, "no colony: a town hall is allowed; otherwise the player is told `TOWN_HALL_NOT_PRESENT` or `…_TOO_FAR_AWAY` and **only creative** passes. In a colony: no `PLACE_HUTS` refuses with `PERMISSION_OPEN_HUT` even for creative; then `creative || canPlaceAt`", "the placement, with the message the ladder names", Carrier::here("event_handler::hut_placement")),
    row!(EventHandler, "EventHandler.java:760-784", "LevelEvent.Load (HIGHEST)", Both, "server: `IColonyManager.onWorldLoad(level)`. Client, with `holidayFeatures`: Oct 31 / Nov 1-2 clears the ghost flag, Apr 1 sets the April flag", "—", Carrier::hook("level_load")),
    row!(EventHandler, "EventHandler.java:770-783", "LevelEvent.Load (client)", Client, "the two holiday windows read `LocalDateTime.now()`", "—", Carrier::missing("the host's wall clock: a mod reads the game's time (`level_day_time`), not the machine's date, so Halloween and April 1st cannot be detected")),
    row!(EventHandler, "EventHandler.java:791-798", "LevelEvent.Unload", Server, "`IColonyManager.onWorldUnload(level)`", "—", Carrier::hook("level_unload")),
    row!(EventHandler, "EventHandler.java:805-816", "BlockEvent.FarmlandTrampleEvent", Server, "a citizen with `JobFarmer` whose colony's `SOFT_SHOES` effect strength is > 0 → cancel", "the trample", Carrier::missing("a `Host` in `farmland_trample`: the hook is handed a `LoopContext`, so neither the entity's job nor the colony's research can be read")),
    row!(EventHandler, "EventHandler.java:823-862", "LivingConversionEvent.Pre", Server, "a `ZombieVillager` becoming a `VILLAGER` in a colony with a level-1+ tavern: `spawnVisitor`, cancel, recruitment interaction, move the visitor onto the zombie, level event 1027, discard the zombie", "the conversion, when a tavern can take the visitor", Carrier::hook("living_conversion_pre")),
    row!(EventHandler, "EventHandler.java:864-875", "TickEvent.ServerTickEvent", Server, "`tickTimes[tickCount % 100] / 1e6` ms over **50** clamps `slownessFactor` to `[1, 5]`, else 1 — the array is `ServerHost::tick_times()`, same layout (`event_handler::tick_duration_ms`)", "—", Carrier::hook("server_tick")),
    // ── ClientEventHandler.java ───────────────────────────────────────────────
    row!(ClientEventHandler, "ClientEventHandler.java:93-97", "RenderLevelStageEvent (LOWEST)", Client, "`WorldEventContext.INSTANCE.renderWorldLastEvent(event)`: the colony border, blueprint and path overlays", "—", Carrier::missing("the world-space overlay renderer of `core/client/render/worldevent/`: Horizon draws per-entity and per-block, not a colony-wide pass")),
    row!(ClientEventHandler, "ClientEventHandler.java:99-106", "TickEvent.LevelTickEvent (END)", Client, "one tick in twenty (`ColonyConstants.rand.nextInt(20) == 0`): `WorldEventContext.checkNearbyColony`", "—", Carrier::missing("`WorldEventContext.checkNearbyColony` (`ClientEventHandler.java:104`) feeds a cache the overlay renderer owns, and that renderer is not ported")),
    row!(ClientEventHandler, "ClientEventHandler.java:108-131", "EntityJoinLevelEvent (HIGHEST)", Client, "a fast colony entity joining a client level: every entity with the same UUID is removed, so the server stays authoritative", "the stale client copy", Carrier::missing("a client-side entity-join moment: `entity_join_level` is server-only")),
    row!(ClientEventHandler, "ClientEventHandler.java:133-140", "ClientPlayerNetworkEvent.LoggingOut", Client, "`ColonyBorderRenderer.cleanup()`, `WindowBuildingBrowser.clearCache()`, `IColonyManager.resetColonyViews()`", "—", Carrier::hook("client_player_logging_out")),
    row!(ClientEventHandler, "ClientEventHandler.java:142-158", "PlaySoundEvent (HIGH)", Client, "`citizenVoices == false` and the sound is `minecolonies:` + `CITIZEN_SOUND_EVENT_PREFIX`: `event.setSound(null)`", "the citizen voice", Carrier::missing("a sound-start cancellation: `sound_play` is upstream notes in this SDK, not a declared hook")),
    row!(ClientEventHandler, "ClientEventHandler.java:174-177", "ItemTooltipEvent", Client, "`getEntity() == null` → return: the recipe book asks before a player exists", "every tooltip the recipe book pre-populates", Carrier::here("client_event_handler::tooltips")),
    row!(ClientEventHandler, "ClientEventHandler.java:181-184", "ItemTooltipEvent", Client, "`extraItemTooltips` — a map no shipped code fills — is appended when it has the item", "—", Carrier::hook("item_tooltip")),
    row!(ClientEventHandler, "ClientEventHandler.java:186-196", "ItemTooltipEvent", Client, "a `DyeableLeatherItem` with `showdyetooltips` **and a colour the manager knows** (`getDyeColor(stack).ifPresent`, `:188`): the `item.dyed` line is removed and a coloured one is inserted **at index 1**; no colour, nothing at all", "the vanilla dye line, when the colour is known", Carrier::here("client_event_handler::dyed_tooltip")),
    row!(ClientEventHandler, "ClientEventHandler.java:198-207", "ItemTooltipEvent", Client, "the colony at the player's position, else the one he owns; none → no colony-dependent line at all", "—", Carrier::here("client_event_handler::tooltips")),
    row!(ClientEventHandler, "ClientEventHandler.java:283-373", "ItemTooltipEvent", Client, "a custom recipe whose output is this item and `showTooltip` and `crafter.length() >= 2`: one research line per required research (aqua when completed or effect > 0, else red), then one line per crafting building (level > 0: aqua when the colony has it at that level, else red; level 0: italic grey \"available\")", "—", Carrier::here("client_event_handler::recipe_lines")),
    row!(ClientEventHandler, "ClientEventHandler.java:210-232", "ItemTooltipEvent", Client, "a hut block: two research lines when the colony lacks the effect and the global tree has it; in creative with a scan tool, the schematic tier", "—", Carrier::here("client_event_handler::hut_research_lines")),
    row!(ClientEventHandler, "ClientEventHandler.java:218-231", "ItemTooltipEvent", Client, "the scan tool is found by `InventoryUtils.hasItemInItemHandler(InvWrapper, ModItems.scanTool)`; the tier is `max` over a Domum block's textured components", "—", Carrier::missing("a schematic-tier table: `SchemAnalyzerUtil.getBlockTier` reads the mod's own schematic metadata, which the port does not ship")),
    row!(ClientEventHandler, "ClientEventHandler.java:234-273", "ItemTooltipEvent", Client, "with the citizen inventory open and a food stack: inedible → red `wrongfood` and stop; else the tier, then (edible at the home level) the tier line plus `nomenu` in red when the nearest cook's restaurant menu lacks it, or red `needbetterfood`", "—", Carrier::here("client_event_handler::food_tooltip")),
    row!(ClientEventHandler, "ClientEventHandler.java:381-389", "ItemTooltipEvent", Client, "`getFullBuildingName`: the display name of the mod that owns the block's namespace, then the block's name", "—", Carrier::here("client_event_handler::building_name")),
    row!(ClientEventHandler, "ClientEventHandler.java:396-411", "ItemTooltipEvent (lazy)", Client, "`crafterToBuilding`: for every registered building, every module producer whose module is an `ICraftingBuildingModule` with a crafting job, keyed by `getCustomRecipeKey()`", "—", Carrier::here("client_event_handler::crafter_to_building")),
    row!(ClientEventHandler, "ClientEventHandler.java:421-437", "ItemTooltipEvent", Client, "a hut's research effect with strength 0 and a global research for it: `HUT_NEEDS_RESEARCH_TOOLTIP_1` and `_2`", "—", Carrier::here("client_event_handler::hut_research_lines")),
    row!(ClientEventHandler, "ClientEventHandler.java:442-475", "CustomizeGuiOverlayEvent.DebugText", Client, "F3 open: the colony at the position and its distance; else `DEBUG_NO_CLOSE_COLONY` when far enough, else the closest colony, its distance and `getMinimumDistanceBetweenTownHalls()`", "—", Carrier::hook("register_debug_entries")),
    row!(ClientEventHandler, "ClientEventHandler.java:487-493", "PlayerInteractEvent.RightClickItem (LOW)", Client, "a `BlockItem` whose `hitResult` is not a `MISS` is filtered out: Forge fires this for block clicks too", "—", Carrier::here("client_event_handler::browse_click")),
    row!(ClientEventHandler, "ClientEventHandler.java:495-503", "PlayerInteractEvent.RightClickItem", Client, "an `IBuildingBrowsableBlock` whose `shouldBrowseBuildings(event)`: open the building browser and cancel with `InteractionResult.SUCCESS`", "the item's own use", Carrier::hook("right_click_item")),
    // ── FMLEventHandler.java ──────────────────────────────────────────────────
    row!(FmlEventHandler, "FMLEventHandler.java:22-26", "TickEvent.ServerTickEvent", Both, "`IColonyManager.onServerTick(event)` — every phase, unguarded", "—", Carrier::hook("server_tick")),
    row!(FmlEventHandler, "FMLEventHandler.java:28-32", "TickEvent.ClientTickEvent", Client, "`IColonyManager.onClientTick(event)`", "—", Carrier::hook("client_tick_post")),
    row!(FmlEventHandler, "FMLEventHandler.java:34-43", "PlayerEvent.PlayerLoggedInEvent", Server, "`getIColonyByOwner(level, player)`: reloads the colony whose owner is this player if the lookup failed — the return value is dropped", "—", Carrier::hook("player_logged_in")),
    row!(FmlEventHandler, "FMLEventHandler.java:45-57", "AddReloadListenerEvent", ServerDataLoad, "nine listeners, in this order: `CrafterRecipeListener`, `ResearchListener`, `CustomVisitorListener`, `CitizenNameListener`, `QuestJsonListener`, `ItemNbtListener`, `StudyItemListener`, `DiseasesListener`, `RecruitmentItemsListener`", "—", Carrier::ported("crate::data_listeners", "the nine, in the same order")),
    row!(FmlEventHandler, "FMLEventHandler.java:45-57", "AddReloadListenerEvent", ServerDataLoad, "the registration itself: `event.addListener(new …)`", "—", Carrier::hook("add_server_reload_listeners")),
    row!(FmlEventHandler, "FMLEventHandler.java:59-63", "TickEvent.LevelTickEvent", Both, "`IColonyManager.onWorldTick(event)` — both phases and both sides", "—", Carrier::hook("level_tick_post")),
    row!(FmlEventHandler, "FMLEventHandler.java:65-69", "ServerAboutToStartEvent", Server, "`getRecipeManager().reset()` before any world exists", "—", Carrier::hook("server_about_to_start")),
    row!(FmlEventHandler, "FMLEventHandler.java:71-75", "ServerStoppingEvent", Server, "`Pathfinding.shutdown()` — the pool is a static in the Java", "—", Carrier::hook("server_stopping")),
    row!(FmlEventHandler, "FMLEventHandler.java:77-81", "ServerStartedEvent", Server, "`BackUpHelper.loadMissingColonies()`: colonies whose file the manager did not read are restored from their backup", "—", Carrier::superseded("crate::persistence", "colonies live in the world's saved data, not one file per colony", "there is no `colonies/*.dat` to fail to read, so the restore has nothing to cover; `ServerHost::world_dir()` does exist for a mod that wanted its own folder")),
    // ── DataPackSyncEventHandler.java ─────────────────────────────────────────
    row!(DataPackSync, "DataPackSyncEventHandler.java:52-58", "OnDatapackSyncEvent / ServerStartedEvent", ServerDataLoad, "`discoverCompatLists`, in this order: `FurnaceRecipes.loadRecipes`, `CompatibilityManager.discover`, `CustomRecipeManager.resolveTemplates`, `CustomRecipeManager.buildLootData`", "—", Carrier::ported("crate::data_listeners", "timeline, the same order")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:66-74", "OnDatapackSyncEvent", Server, "`sendPackets`, in this order: compatibility message, custom recipes, global research tree, global quests, global diseases", "—", Carrier::missing("a per-player payload send at the data-pack moment: `datapack_sync` collects recipe *types* for the joining player and sends nothing of its own")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:90-104", "OnDatapackSyncEvent (player == null)", ServerDataLoad, "`/reload`: rebuild the four lists, then send to every player **whose `GameProfile` is not the single-player owner's** — compared with `!=`, so only the identical instance is skipped", "—", Carrier::hook("tags_updated")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:105-108", "OnDatapackSyncEvent (player != null)", Server, "a joining player that is not the owner: the same five packets, with a fresh compatibility message", "—", Carrier::hook("datapack_sync")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:110-114", "OnDatapackSyncEvent", ServerDataLoad, "`auditCraftingTags` and nobody online (or a reload): `CraftingTagAuditor.doRecipeAudit`", "—", Carrier::here("data_pack_sync::audit_wanted")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:124-128", "ServerStartedEvent", ServerDataLoad, "`discoverCompatLists` again, because the smelting recipes only exist once the server started", "—", Carrier::hook("server_started")),
    row!(DataPackSync, "DataPackSyncEventHandler.java:141-154", "RecipesUpdatedEvent", Client, "the single-player owner skips the reload; every other client reloads `FurnaceRecipes` from the synced recipe manager", "—", Carrier::hook("recipes_received")),
    // ── GatherDataHandler.java ────────────────────────────────────────────────
    row!(GatherData, "GatherDataHandler.java:19-72", "GatherDataEvent", Datagen, "`SchemFixerUtil.fixSchematics()` then **41** providers: 4 client, 2 both, 35 server (of which 21 are one per worker)", "—", Carrier::missing("a data-generation step: `runData` runs before the jar is built and `cargo horizon-mod` has no equivalent, so the port ships its JSON instead of generating it")),
    // ── QuestObjectiveEventHandler.java ───────────────────────────────────────
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:61-88", "BlockEvent.BreakEvent (LOWEST)", Server, "a block with a registered objective for this player: each quest's current objective, `onBlockBreak` when it is a `IBreakBlockObjectiveTemplate`, else **remove it and `break`** (the remaining quests of that block are skipped)", "—", Carrier::here("quest_objectives::Registry::block_broken")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:90-112", "LivingDeathEvent (LOWEST)", Server, "the killer is a `Player` and the entity type has a registered objective for him: same shape, `onEntityKill`", "—", Carrier::here("quest_objectives::Registry::entity_killed")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:119-146", "BlockEvent.EntityPlaceEvent (LOWEST)", Server, "the placer is a `Player`: `onBlockPlace` for each registered objective of the placed block", "—", Carrier::here("quest_objectives::Registry::block_placed")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:155-162", "— (called by the objective)", Server, "`addQuestMineObjectiveListener`: `breakBlockObjectives[block][player] += quest`", "—", Carrier::here("quest_objectives::Registry::listen")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:171-174", "— (called by the objective)", Server, "`removeQuestMineObjectiveListener`: `getOrDefault(...).getOrDefault(...).remove(quest)` — three default maps are allocated and thrown away, and the entry is never pruned", "—", Carrier::here("quest_objectives::Registry::forget")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:183-202", "— (called by the objective)", Server, "the same pair for `placeBlockObjectives`", "—", Carrier::here("quest_objectives::Registry::listen")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:211-230", "— (called by the objective)", Server, "the same pair for `entityKillObjectives`, keyed by `EntityType`", "—", Carrier::here("quest_objectives::Registry::entity_killed")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:237-250", "called when a research completes", Server, "`researchObjectives[id][colony]`: `onResearchCompletion` for each `IResearchObjectiveTemplate`; an objective of another class is **not** removed here, unlike the three event maps", "—", Carrier::here("quest_objectives::Registry::research_completed")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:257-270", "called when a building reaches a level", Server, "`buildBuildingObjectives[entry][colony]`: `onBuildingUpgrade(progress, quest, level)`", "—", Carrier::here("quest_objectives::Registry::building_upgraded")),
    row!(QuestObjectives, "QuestObjectiveEventHandler.java:277-318", "— (called by the objective)", Server, "`trackBuildingLevelUp` / `stopTrackingBuildingLevelUp` / `trackResearch` / `stopTrackingResearch`", "—", Carrier::here("quest_objectives::Registry::listen")),
    // ── ClientChunkUpdatedEvent.java ──────────────────────────────────────────
    row!(ClientChunkUpdated, "ClientChunkUpdatedEvent.java:13-34", "ClientChunkUpdatedEvent (the mod's own)", Client, "an event class and a getter, nothing more: posted from `ChunkClientDataHelper.applyCap` (`ChunkClientDataHelper.java:75`) after the capability is written, **even when no colony owns the chunk and even when nothing changed**", "—", Carrier::here("client_chunk_updated")),
    row!(ClientChunkUpdated, "ClientChunkUpdatedEvent.java:22-25", "ClientChunkUpdatedEvent", Client, "two subscribers only, both JourneyMap integrations (`compatibility/journeymap/v5/EventListener.java:78`, `v6/EventListener.java:75`)", "—", Carrier::missing("a client map integration: the only subscribers are `journeymap/v5/EventListener.java:78` and `v6/EventListener.java:75`, which draw claims on a JourneyMap overlay, and the port has no map mod to notify")),
    // ── ColonyStoryListener.java ──────────────────────────────────────────────
    row!(ColonyStory, "ColonyStoryListener.java:67-71", "RegisterClientReloadListenersEvent", Client, "`event.registerReloadListener(new ColonyStoryListener())` — a client listener on `colony_stories/`", "—", Carrier::hook("add_client_reload_listeners")),
    row!(ColonyStory, "ColonyStoryListener.java:99-125", "reload", Client, "`apply`: clear the four sets, then per file — an array is parsed element by element, an object as one story; any `Throwable` is logged and **only that file is lost**, the sets stay cleared", "nothing; a bad file costs the files already read? no — the sets are cleared first, so survivors are the files read after it", Carrier::here("colony_story::parse_all")),
    row!(ColonyStory, "ColonyStoryListener.java:127-150", "reload", Client, "`parseStory`: `type` is read with `getAsString()` (a number is its text) and must be a valid `ResourceLocation`, else the file is lost; it then picks one of `abandoned_name`, `abandoned`, `camp`, `ship`, and any other id is **ignored silently**, on purpose, for other mods' files", "an unknown `type`", Carrier::here("colony_story::Kind")),
    row!(ColonyStory, "ColonyStoryListener.java:152-184", "reload", Client, "`parseStoryText`: `biomes` (string or array, OR-ed) then `content`, each read with `getAsString()` (a number or a boolean is its own text) and one `StoryText` per element; a missing `content` throws and the file is dropped", "a file with no `content`", Carrier::here("colony_story::parse_story")),
    row!(ColonyStory, "ColonyStoryListener.java:189-234", "reload", Client, "`BiomeFilter`: `#ns:tag` matches the tag, a bare id resolves the biome through the server's registry **or throws**; `ALL` is the identity of `or`", "a biome id no registry holds, by dropping the whole file", Carrier::here("colony_story::Filter")),
    row!(ColonyStory, "ColonyStoryListener.java:88-97", "— (called by the town hall and the camp)", Client, "`pickRandom`: every story matching the biome, then one at random; no match answers the empty string", "no match, with `\"\"`", Carrier::here("colony_story::pick")),
    // ── TextureReloadListener.java ────────────────────────────────────────────
    row!(TextureReload, "TextureReloadListener.java:70-74", "RegisterClientReloadListenersEvent", Client, "register the listener; it is a `SimplePreparableReloadListener`, so the scan and the swap are two phases", "—", Carrier::hook("add_client_reload_listeners")),
    row!(TextureReload, "TextureReloadListener.java:34-53", "reload (prepare)", Client, "list `textures/entity/citizen`, keep the paths that contain `png`, and take `split(\"/\")[3]` — the folder under `citizen/` — as a pack name; empty folders are skipped", "a path with fewer than four segments (it throws), and an empty folder name", Carrier::here("texture_reload::packs_in")),
    row!(TextureReload, "TextureReloadListener.java:56-60", "reload (apply)", Client, "`TEXTURE_PACKS.clear()` then `addAll(packs)`: the swap is the whole write", "—", Carrier::here("texture_reload::TEXTURE_PACKS")),
    // ── ClientRegistryHandler.java ────────────────────────────────────────────
    row!(ClientRegistry, "ClientRegistryHandler.java:56-155", "EntityRenderersEvent.RegisterLayerDefinitions", Client, "**94** `ModelLayerLocation`s, every one `<mod>:<name>#<name>`; `:159-261` registers each with its model's `createMesh`", "—", Carrier::hook("register_layer_definitions")),
    row!(ClientRegistry, "ClientRegistryHandler.java:263-269", "RegisterItemDecorationsEvent", Client, "`colonyMap` → `ColonyMapDecorator`, `clipboard` → `ClipBoardDecorator` (an item overlay drawn in the slot)", "—", Carrier::missing("per-item slot decorations: no hook draws an item's slot overlay (`register_item_models` replaces the model, not the decoration)")),
    row!(ClientRegistry, "ClientRegistryHandler.java:271-372", "EntityRenderersEvent.RegisterRenderers", Client, "**47** entity renderers (citizen, visitor, raiders ×4 families and their camp variants, mercenary, sitting entity, minecart, cavalry horse), **7** block entity renderers (`BUILDING` empty, scarecrow, enchanter, colony flag, named grave, deco controller, colony sign), the cutout/solid render layers, and two item properties (`throwing`, `disabled`)", "—", Carrier::hook("register_entity_renderers")),
    row!(ClientRegistry, "ClientRegistryHandler.java:348-354", "EntityRenderersEvent.RegisterRenderers", Client, "the seven block entity renderers", "—", Carrier::missing("a block-entity renderer registration: `block_entity_submit` draws one block entity type's Part, and nothing registers a renderer *type* for the seven")),
    row!(ClientRegistry, "ClientRegistryHandler.java:356-366", "EntityRenderersEvent.RegisterRenderers", Client, "the render layers: every hut and crop as `cutout`, seven named blocks as `cutout`", "—", Carrier::missing("a block render type registration (`ItemBlockRenderTypes.setRenderLayer`): no hook decides whether a block's layer is cutout, solid or translucent")),
    row!(ClientRegistry, "ClientRegistryHandler.java:368-371", "EntityRenderersEvent.RegisterRenderers", Client, "`ItemProperties.register(buildGoggles, \"disabled\", …)`: 1 when `ColonyBlueprintRenderer.willRenderBlueprints()` is false — a mod-owned client flag, so a numeric item-model property carries it", "—", Carrier::hook("register_range_select_item_model_property")),
    row!(ClientRegistry, "ClientRegistryHandler.java:368-369", "EntityRenderersEvent.RegisterRenderers", Client, "`ItemProperties.register(spear, \"throwing\", …)`: 1 while `entity.isUsingItem() && entity.getUseItem() == item`, which needs the **holder**", "—", Carrier::missing("a numeric item-model property that can see its holder: `ItemModelRangePropertyEvent` carries the stack, `bundle_selected` and `value`, and no entity, so `isUsingItem()` cannot be read")),
    row!(ClientRegistry, "ClientRegistryHandler.java:374-380", "RegisterRecipeBookCategoriesEvent", Client, "the compost recipe type's category finder answers `UNKNOWN`, which hides the warnings the book would log", "—", Carrier::hook("recipe_book_categories")),
    row!(ClientRegistry, "ClientRegistryHandler.java:382-387", "RegisterKeyMappingsEvent", Client, "`ModKeyMappings.register(event)`: one key, `key.minecolonies.toggle_goggles`, `IN_GAME`, unbound by default", "—", Carrier::here("client_registry::KEYS")),
    // ── capabilityproviders/ ─────────────────────────────────────────────────
    row!(ChunkCapability, "capabilityproviders/MinecoloniesChunkCapabilityProvider.java:17-55", "AttachCapabilitiesEvent<LevelChunk>", Server, "serialise and deserialise `IColonyTagCapability` as `CLOSE_COLONY_CAP`; answer it for any direction, `LazyOptional.empty()` for every other capability", "any other capability", Carrier::here("capability_providers::ChunkClaim")),
    row!(WorldChunkCapability, "capabilityproviders/MinecoloniesWorldCapabilityProvider.java:17-55", "AttachCapabilitiesEvent<Level>", Server, "serialise and deserialise `IChunkmanagerCapability` as `CHUNK_STORAGE_UPDATE_CAP`", "any other capability", Carrier::here("capability_providers::ChunkUpdate")),
    row!(WorldColonyManagerCapability, "capabilityproviders/MinecoloniesWorldColonyManagerCapabilityProvider.java:17-61", "AttachCapabilitiesEvent<Level>", Server, "serialise and deserialise `IColonyManagerCapability` as `COLONY_MANAGER_CAP`, with the `overworld` flag deciding what is written", "any other capability", Carrier::here("capability_providers::ColonyManager")),
];

/// The hook names the table cites, as the SDK declares them.
///
/// Measured, not guessed: every entry is a method of `Hooks` in
/// `crates/horizon-mod-vocab/src/hooks/`, generated into
/// `generated/mod_api_generated.rs`. The list is here so a `Hook` row that names a
/// hook nobody declared fails a test instead of shipping as prose.
pub const HOOKS: &[&str] = &[
    "add_client_reload_listeners",
    "add_server_reload_listeners",
    "break_block",
    "chunk_load",
    "chunk_unload",
    "client_chunk_load",
    "client_tick_post",
    "client_player_logging_out",
    "datapack_sync",
    "entity_join_level",
    "entity_travel_to_dimension",
    "item_tooltip",
    "level_load",
    "level_unload",
    "level_tick_post",
    "living_conversion_pre",
    "loot_condition",
    "loot_table_load",
    "mob_spawn_position_check",
    "player_changed_dimension",
    "player_logged_in",
    "player_logged_out",
    "player_tick_post",
    "recipe_book_categories",
    "recipes_received",
    "register_debug_entries",
    "register_entity_renderers",
    "register_layer_definitions",
    "register_range_select_item_model_property",
    "right_click_item",
    "server_about_to_start",
    "server_started",
    "server_stopping",
    "server_tick",
    "tags_updated",
];

/// Whether the SDK declares `hook`.
pub fn hook_exists(hook: &str) -> bool {
    HOOKS.contains(&hook)
}

/// How many rows each carrier holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub rows: usize,
    pub ported: usize,
    pub here: usize,
    pub hook: usize,
    pub missing: usize,
    pub superseded: usize,
}

/// The table, counted.
pub fn counts() -> Counts {
    let mut counts = Counts {
        rows: ROWS.len(),
        ..Counts::default()
    };
    for row in ROWS {
        match row.carrier.kind() {
            Kind::Ported => counts.ported += 1,
            Kind::Here => counts.here += 1,
            Kind::Hook => counts.hook += 1,
            Kind::Missing => counts.missing += 1,
            Kind::Superseded => counts.superseded += 1,
        }
    }
    counts
}

/// The rows of one listener, in the Java's order.
pub fn rows_of(listener: Listener) -> impl Iterator<Item = &'static Row> {
    ROWS.iter().filter(move |row| row.listener == listener)
}

/// Every row with no hook behind it, which is exactly what `MANQUANT.md` lists.
pub fn without_hook() -> impl Iterator<Item = &'static Row> {
    ROWS.iter().filter(|row| row.missing())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fourteen_files_are_counted_from_the_brief() {
        let lines: u32 = Listener::ALL.iter().map(|listener| listener.lines()).sum();
        assert_eq!(lines, CODE_LINES, "thirteen files with code");
        assert_eq!(lines + PACKAGE_INFO_LINES, 2951, "the brief's 2 951");
        assert_eq!(Listener::ALL.len(), 13, "package-info holds no code");
    }

    #[test]
    fn every_row_cites_a_java_file_and_a_line_range() {
        for row in ROWS {
            let (file, first, last) = row
                .lines()
                .unwrap_or_else(|| panic!("unparsable citation: {}", row.at));
            assert!(file.ends_with(".java"), "{} names no Java file", row.at);
            assert!(first >= 1, "{} starts at 0", row.at);
            assert!(last >= first, "{} ends before it starts", row.at);
        }
    }

    #[test]
    fn every_row_names_the_file_its_listener_lives_in() {
        for row in ROWS {
            let (file, _, _) = row.lines().expect("parsed above");
            assert_eq!(
                file,
                row.listener.java_file(),
                "a row filed under the wrong listener"
            );
        }
    }

    #[test]
    fn no_row_points_past_the_end_of_its_file() {
        for row in ROWS {
            let (_, _, last) = row.lines().expect("parsed above");
            assert!(
                last <= row.listener.lines(),
                "{} cites line {last} of a {} line file",
                row.at,
                row.listener.lines()
            );
        }
    }

    #[test]
    fn every_hook_the_table_cites_is_declared_by_the_sdk() {
        for row in ROWS {
            if let Carrier::Hook { hook } = row.carrier {
                assert!(hook_exists(hook), "{} cites no hook", row.at);
            }
        }
    }

    #[test]
    fn the_hook_list_has_no_duplicate() {
        let mut seen = HOOKS.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "the same hook listed twice");
    }

    #[test]
    fn a_missing_row_says_what_is_missing() {
        for row in without_hook() {
            let Carrier::Missing { needs } = row.carrier else {
                panic!("{} is not missing", row.at);
            };
            assert!(needs.len() > 40, "{} states no problem: {needs}", row.at);
            // A refusal names the thing that is missing: a type, a function, a hook or a
            // Java class, written in backticks. `MANQUANT.md` expands it; the row has to
            // be readable on its own, which "a gap in the API" is not.
            assert!(
                needs.matches('`').count() >= 2,
                "{} names no type, function or class: {needs}",
                row.at
            );
        }
    }

    #[test]
    fn the_counts_add_up_to_the_table() {
        let counts = counts();
        assert_eq!(
            counts.ported
                + counts.here
                + counts.hook
                + counts.missing
                + counts.superseded,
            counts.rows
        );
        assert_eq!(counts.rows, ROWS.len());
    }

    #[test]
    fn a_superseded_row_names_the_module_that_replaces_it() {
        for row in ROWS {
            if let Carrier::Superseded { module, item, why } = row.carrier {
                assert!(module.starts_with("crate::"), "{}: {module}", row.at);
                assert!(!item.is_empty(), "{} names no replacement", row.at);
                assert!(
                    why.len() > 40,
                    "{} does not say why the failure cannot happen: {why}",
                    row.at
                );
            }
        }
    }

    #[test]
    fn every_listener_has_at_least_one_row() {
        for listener in Listener::ALL {
            let rows = rows_of(*listener).count();
            assert!(rows > 0, "{} has no row", listener.java_file());
        }
    }

    #[test]
    fn the_two_big_files_carry_most_of_the_table() {
        // A reader of this module wants to know where the work is: `EventHandler` and
        // `ClientEventHandler` are two thirds of the Java and the two thirds of the
        // rows, while the three capability files are three rows each whatever the port
        // does.
        let event_handler = rows_of(Listener::EventHandler).count();
        let client = rows_of(Listener::ClientEventHandler).count();
        assert!(event_handler > client, "the server half is the bigger one");
        assert!(client >= 15, "the client half is not a footnote");
        let total = ROWS.len();
        assert!(
            event_handler + client > total / 2,
            "the two halves are less than half the table"
        );
    }

    #[test]
    fn no_two_rows_describe_the_same_line_of_the_same_file_in_the_same_way() {
        // Two rows may share a range when the Java really has two rules there
        // (`:387` has the sentinel test and the visit), but a verbatim copy of a
        // whole row is a paste, and a paste is a bug in the table.
        for (index, row) in ROWS.iter().enumerate() {
            for other in &ROWS[index + 1..] {
                assert!(
                    !(row.at == other.at && row.rule == other.rule),
                    "{} is listed twice",
                    row.at
                );
            }
        }
    }

    #[test]
    fn the_datagen_row_is_the_only_one_that_never_runs_in_a_game() {
        let datagen: Vec<&Row> = ROWS.iter().filter(|row| row.side == Side::Datagen).collect();
        assert_eq!(datagen.len(), 1);
        assert_eq!(datagen[0].listener, Listener::GatherData);
    }
}
