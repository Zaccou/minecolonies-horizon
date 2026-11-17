//! `EventHandler.java`: the server half, decided without a game.
//!
//! Java reference: `EventHandler.java`, 876 lines, 42 branches, all in [`super::table`].
//! This file ports **the branches whose decision is a function of values**: the
//! crop-drop ladder, the join refusal, the hut-placement ladder, the bed check, the
//! chunk-entry gate, the spawn refusal, the conversion candidate and the tick
//! slowness. Three of its branches have no function here because the value they need
//! does not exist in the API; [`super::table`] files them `Missing` and
//! `MANQUANT.md` says why.
//!
//! **The shape every function shares.** Java reads the world inside the branch
//! (`event.getLevel()`, `IColonyManager.getInstance()`, a static config); the port
//! takes the same values as a struct of facts and answers a *named* verdict. That is
//! what lets forty tests run with no level, no colony and no wasm, and it is why
//! nothing here can quietly invent a value: a fact that is not in the struct is a
//! fact the caller must supply.
//!
//! **Two findings the functions make explicit.**
//!
//! 1. **The chunk-claim branch tests a sentinel nothing writes.**
//!    `EventHandler.java:387` guards on `getOwningColony() != -1`, but
//!    `ChunkCapData` starts its field at `0` (`ChunkCapData.java:35`) and
//!    `IColonyTagCapability` at `NO_COLONY_ID`, also `0`
//!    (`IColonyTagCapability.java:120`, `ColonyManagerConstants.java:56`). `-1`
//!    appears in that one comparison and nowhere else in the tree, so the visit
//!    branch runs for unclaimed chunks too and the lookup that follows it answers
//!    `null`. And the two guards do **not** share a sentinel: the second (`:399`) is
//!    `!= 0`, so a `-1` that nothing writes would silence the visit branch and still
//!    start the building loop — [`claim_effects`] keeps all three cases.
//! 2. **Creative bypasses two different rungs.** `EventHandler.java:741` returns
//!    `player.isCreative()` *after* telling the player the hut is outside a colony,
//!    so the message is sent and the placement is allowed; `:751` lets creative skip
//!    `canPlaceAt` but **not** the `PLACE_HUTS` permission, which is checked first
//!    (`:743-748`). [`hut_placement`] returns both the answer and the notice.

/// The chunk-entry gate's period: `event.player.level.getGameTime() % 100 != 0`
/// (`EventHandler.java:352`).
pub const CHUNK_TICK_PERIOD: i64 = 100;
/// The radius the range packet carries, `UpdateChunkRangeCapabilityMessage(…, 8, true)`
/// (`EventHandler.java:380`).
pub const CHUNK_RANGE: i32 = 8;
/// The tick length over which Java slows its state machines, in milliseconds
/// (`EventHandler.java:868`).
pub const SLOWNESS_TRIGGER_MS: f64 = 50.0;
/// The ceiling the factor is clamped to (`EventHandler.java:870`): five times slower
/// is the worst the Java models.
pub const SLOWNESS_CEILING: f64 = 5.0;
/// A crop's base drop chance on any block but grass (`EventHandler.java:186`).
pub const CROP_CHANCE_OTHER: f32 = 0.01;
/// On `minecraft:grass`, because "grass blocks have a lot of crops (both
/// MineColonies and vanilla) so the base drop chance is reduced"
/// (`EventHandler.java:185-186`).
pub const CROP_CHANCE_GRASS: f32 = 0.001;
/// The dungeon pool rolls every crop at this chance (`EventHandler.java:231`).
pub const DUNGEON_CROP_CHANCE: f32 = 0.005;
/// `ColonyManagerConstants.NO_COLONY_ID` (`:56`), the value
/// `ColonyUtils.getOwningColony` answers for an unclaimed chunk.
pub const NO_COLONY_ID: i32 = 0;
/// The sentinel `EventHandler.java:387` tests, and the one no writer produces.
pub const CHUNK_CAP_NO_COLONY: i32 = -1;
/// The ring the server keeps its tick durations in: `MinecraftServer.getTickTimesNanos`
/// holds a hundred, indexed by `tickCount % 100` (`ServerHost::tick_times` documents the
/// same layout, `host/config.rs:96-102`).
pub const TICK_RING: u64 = 100;

// ── the hoe ladder ────────────────────────────────────────────────────────────

/// The hoe a player holds, as the six rungs of `EventHandler.java:197-219` see it.
///
/// `Other` is every hoe that is not one of the four named materials — Java tests
/// `HAS_HOE` with the four inverted, so a wooden, stone or modded hoe lands there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hoe {
    Netherite,
    Diamond,
    Iron,
    Golden,
    Other,
}

impl Hoe {
    /// The multiplier Java applies to the base chance (`:199-218`), in the order the
    /// `AlternativesEntry` lists them: 10, 8, 6, 5, 4, and 1 with no hoe at all.
    ///
    /// The order is the rule, not the numbers: `AlternativesEntry` takes the first
    /// entry whose conditions all pass, and each rung tests its own material. A
    /// golden hoe is therefore *worse than iron* (×5 against ×6), which is the
    /// vanilla material order and not a typo.
    pub fn factor(self) -> f32 {
        match self {
            Hoe::Netherite => 10.0,
            Hoe::Diamond => 8.0,
            Hoe::Iron => 6.0,
            Hoe::Golden => 5.0,
            Hoe::Other => 4.0,
        }
    }

    /// The rung's name, as the Java constant spells it.
    pub fn java_constant(self) -> &'static str {
        match self {
            Hoe::Netherite => "HAS_NETHERITE_HOE",
            Hoe::Diamond => "HAS_DIAMOND_HOE",
            Hoe::Iron => "HAS_IRON_HOE",
            Hoe::Golden => "HAS_GOLDEN_HOE",
            Hoe::Other => "HAS_HOE",
        }
    }
}

/// The base chance of a crop pool (`EventHandler.java:186`).
pub fn crop_chance(on_grass: bool) -> f32 {
    if on_grass {
        CROP_CHANCE_GRASS
    } else {
        CROP_CHANCE_OTHER
    }
}

/// The chance one rung of the ladder rolls (`EventHandler.java:199-219`):
/// `base * factor`, and `base` alone with no hoe.
pub fn crop_drop_chance(on_grass: bool, hoe: Option<Hoe>) -> f32 {
    let base = crop_chance(on_grass);
    match hoe {
        Some(hoe) => base * hoe.factor(),
        None => base,
    }
}

/// The six rungs, best first, as `(hoe, chance)` — the pool's `AlternativesEntry`.
pub fn crop_ladder(on_grass: bool) -> [(Option<Hoe>, f32); 6] {
    [
        (Some(Hoe::Netherite), crop_drop_chance(on_grass, Some(Hoe::Netherite))),
        (Some(Hoe::Diamond), crop_drop_chance(on_grass, Some(Hoe::Diamond))),
        (Some(Hoe::Iron), crop_drop_chance(on_grass, Some(Hoe::Iron))),
        (Some(Hoe::Golden), crop_drop_chance(on_grass, Some(Hoe::Golden))),
        (Some(Hoe::Other), crop_drop_chance(on_grass, Some(Hoe::Other))),
        (None, crop_drop_chance(on_grass, None)),
    ]
}

// ── the join refusal ─────────────────────────────────────────────────────────

/// The four facts the join branch reads.
///
/// **Two class tests, not one.** `:148` asks `instanceof AbstractFastMinecoloniesEntity`
/// (the citizen, the visitor and the mercenary) and `:153` asks `instanceof
/// EntityCitizen` (the citizen only), so the two refusals do not apply to the same set
/// of entities and a single "colony entity" flag would widen the second one. An audit
/// against the Java found exactly that: a visitor with colony id 0 is **not** refused.
#[derive(Clone, Copy, Debug)]
pub struct Join {
    /// `event.getEntity() instanceof AbstractFastMinecoloniesEntity` (`:148`).
    pub fast_colony_entity: bool,
    /// `event.getEntity() instanceof EntityCitizen` (`:153`).
    pub citizen: bool,
    /// `((ServerLevel) level).getEntity(uuid) != null`: the level already holds an
    /// entity with this UUID (`:148`).
    pub already_present: bool,
    /// `citizen.getCitizenColonyHandler().getColonyId()` (`:153`); `Some(0)` is the
    /// refusal, and the field is only read for a citizen.
    pub colony_id: Option<i32>,
}

/// Why a join is kept out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinRefusal {
    /// A fast colony entity the level already holds (`:148-151`).
    Duplicate,
    /// A citizen whose colony id is 0 (`:153-157`).
    ColonyZero,
}

/// `EventHandler.java:148-157`: the two refusals, in the Java's order — the duplicate
/// test runs first and the citizen test only when the first did not fire.
///
/// The duplicate test is the **wider** class and the colony test the narrower one, which
/// is why they cannot share a flag: a visitor or a mercenary already in the level is a
/// duplicate, and one with colony id 0 is not this rule's business.
pub fn join_verdict(join: &Join) -> Result<(), JoinRefusal> {
    if join.fast_colony_entity && join.already_present {
        return Err(JoinRefusal::Duplicate);
    }
    if join.citizen && join.colony_id == Some(0) {
        return Err(JoinRefusal::ColonyZero);
    }
    Ok(())
}

// ── the mob attack goals ─────────────────────────────────────────────────────

/// The first goal's priority (`EventHandler.java:143`).
pub const ATTACK_PRIORITY_CITIZEN: i32 = 6;
/// The second goal's priority (`EventHandler.java:145`).
pub const ATTACK_PRIORITY_MERCENARY: i32 = 7;

/// One added target goal, as the Java builds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackGoal {
    pub priority: i32,
    /// The species hunted, as a namespaced id.
    pub prey: &'static str,
    /// `mustSee`: true in both, because both constructors pass it.
    pub must_see: bool,
    /// `mustReach`: false in both, because the four-argument constructor's default
    /// applies (`NearestAttackableTargetGoal(Mob, Class, boolean mustSee,
    /// Predicate)`).
    pub must_reach: bool,
}

/// `EventHandler.java:138-146`, the guard before the goals are added.
///
/// Every term is the Java's and all four are needed: the config switch, an `Enemy`,
/// the blacklist (`:140`) and "not one of ours" (`:141`) — a colony mercenary is an
/// `Enemy` and a `Mob`, and it must not hunt its own citizens.
pub fn attack_goals(
    config_on: bool,
    is_enemy: bool,
    blacklisted: bool,
    colony_entity: bool,
) -> Vec<AttackGoal> {
    if !config_on || !is_enemy || blacklisted || colony_entity {
        return Vec::new();
    }
    vec![
        AttackGoal {
            priority: ATTACK_PRIORITY_CITIZEN,
            prey: "minecolonies:citizen",
            must_see: true,
            must_reach: false,
        },
        AttackGoal {
            priority: ATTACK_PRIORITY_MERCENARY,
            prey: "minecolonies:mercenary",
            must_see: true,
            must_reach: false,
        },
    ]
}

// ── the hut placement ladder ─────────────────────────────────────────────────

/// The sentence a refused — or *sent and then allowed* — placement produces.
///
/// Java's names are the translation constants (`TranslationConstants`,
/// `BaseGameTranslationConstants`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// `CANT_PLACE_COLONY_IN_OTHER_DIM` (`EventHandler.java:713`).
    OtherDimension,
    /// `MESSAGE_WARNING_TOWN_HALL_NOT_PRESENT` (`:734`).
    TownHallNotPresent,
    /// `MESSAGE_WARNING_TOWN_HALL_TOO_FAR_AWAY` (`:738`).
    TownHallTooFarAway,
    /// `PERMISSION_OPEN_HUT` (`:746`), with the colony's name.
    PermissionOpenHut,
}

/// What the placement ladder answers: whether the block may be placed **and** the
/// one sentence the player is sent, which is not the same thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub allow: bool,
    pub notice: Option<Notice>,
}

impl Verdict {
    fn allow() -> Verdict {
        Verdict {
            allow: true,
            notice: None,
        }
    }

    fn answer(allow: bool, notice: Notice) -> Verdict {
        Verdict {
            allow,
            notice: Some(notice),
        }
    }
}

/// The facts `EventHandler.onBlockHutPlaced` reads. Eight values, each named after the
/// call that produces it in the Java.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// `getConfig().getServer().allowOtherDimColonies.get()` (`:711`).
    pub other_dimension_allowed: bool,
    /// `WorldUtil.isOverworldType(world)` (`:711`).
    pub overworld: bool,
    /// `block instanceof BlockHutTownHall` (`:726`).
    pub town_hall: bool,
    /// `IColonyManager.getIColony(world, pos) != null` (`:722`).
    pub in_colony: bool,
    /// `getIColonyByOwner(world, player) != null` (`:732`), for the sentence only.
    pub owns_a_colony: bool,
    /// `colony.getPermissions().hasPermission(player, PLACE_HUTS)` (`:743`).
    pub may_place_huts: bool,
    /// `player.isCreative()` (`:741`, `:751`).
    pub creative: bool,
    /// `colony.getServerBuildingManager().canPlaceAt(block, pos, player)` (`:751`).
    pub can_place_at: bool,
}

/// `EventHandler.java:709-753`, in the Java's order.
///
/// The order is what makes it a ladder: the dimension is tested before anything
/// (`:711-715`), a town hall is the one hut allowed outside a colony (`:726-729`),
/// `PLACE_HUTS` is tested before `creative` can help (`:743-748`), and `creative`
/// only replaces `canPlaceAt` (`:751`).
pub fn hut_placement(place: &Placement) -> Verdict {
    if !place.other_dimension_allowed && !place.overworld {
        return Verdict::answer(false, Notice::OtherDimension);
    }
    if !place.in_colony {
        if place.town_hall {
            return Verdict::allow();
        }
        let notice = if place.owns_a_colony {
            Notice::TownHallTooFarAway
        } else {
            Notice::TownHallNotPresent
        };
        // `return player.isCreative();` (`:741`) — the warning was sent, and a
        // creative player is allowed anyway.
        return Verdict::answer(place.creative, notice);
    }
    if !place.may_place_huts {
        return Verdict::answer(false, Notice::PermissionOpenHut);
    }
    Verdict::allow().with_allow(place.creative || place.can_place_at)
}

impl Verdict {
    fn with_allow(self, allow: bool) -> Verdict {
        Verdict {
            allow,
            notice: self.notice,
        }
    }
}

// ── a citizen's bed ──────────────────────────────────────────────────────────

/// Which half of the bed the player clicked (`EventHandler.java:618`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BedHalf {
    Head,
    Foot,
}

/// The head's position, from the clicked position: Java moves a foot click one block
/// along `BedBlock.FACING` (`:620`) and keeps the position otherwise.
pub fn bed_head(half: BedHalf, clicked: [i32; 3], facing: [i32; 3]) -> [i32; 3] {
    match half {
        BedHalf::Head => clicked,
        BedHalf::Foot => [
            clicked[0] + facing[0],
            clicked[1] + facing[1],
            clicked[2] + facing[2],
        ],
    }
}

/// The indices of the citizens that block the click: the ones whose `getBedPos()`
/// equals `bed` **and** that are asleep (`EventHandler.java:624-631`).
///
/// Java does not `break` inside that loop, so two citizens claiming one bed produce
/// two messages; the caller sends one per index, which is the same behaviour without
/// a side effect in a function.
pub fn bed_blockers(bed: [i32; 3], citizens: &[([i32; 3], bool)]) -> Vec<usize> {
    citizens
        .iter()
        .enumerate()
        .filter(|(_, (pos, asleep))| *pos == bed && *asleep)
        .map(|(index, _)| index)
        .collect()
}

// ── entering a chunk ─────────────────────────────────────────────────────────

/// What one player tick does at the hundredth tick: the two side effects of
/// `EventHandler.java:352-373`, kept apart because they happen at different points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkEntry {
    /// Write `now` into `playerPositions` for this player.
    ///
    /// Java does this at `:366`, **before** the empty-chunk test at `:370-373`. A
    /// caller that only remembered a chunk it worked on would re-run the empty case
    /// every hundred ticks, so the two answers are separate on purpose.
    pub remember: bool,
    /// `ChunkDataHelper.loadChunk` (`:375`), the two packets (`:377-384`) and both
    /// claim branches (`:387-416`).
    pub work: bool,
}

/// `EventHandler.java:352-373`: whether the tick is one of the hundred and the player
/// changed chunk.
///
/// The three cheap tests, in Java's order: the tick, the remembered position, and
/// the empty chunk. `last` is `playerPositions.get(uuid)`. The gate and the position
/// test answer [`ChunkEntry::remember`] = `work` = `false`; an empty chunk answers
/// `remember` = `true` with `work` = `false`, which is Java's `put` followed by the
/// early return.
pub fn chunk_entry_due(
    game_time: i64,
    last: Option<[i32; 2]>,
    now: [i32; 2],
    chunk_empty: bool,
) -> ChunkEntry {
    let due = game_time % CHUNK_TICK_PERIOD == 0 && last != Some(now);
    ChunkEntry {
        remember: due,
        work: due && !chunk_empty,
    }
}

/// What a claimed chunk makes the handler do, from the chunk capability's owning
/// colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimEffects {
    /// `addVisitingPlayer` + `addCloseSubscriber` (`:387-396`).
    pub visit: bool,
    /// `building.onPlayerEnterNearby` for each claiming building (`:399-416`).
    pub buildings: bool,
}

/// The two tests of `EventHandler.java:387` and `:399`, kept as they are written.
///
/// `visit` is `owning != -1` and `buildings` is `owning != 0`, and the two guards do
/// not share a sentinel. Consequences, all measured:
///
/// | `owning` | `visit` | `buildings` | what happens |
/// | --- | --- | --- | --- |
/// | `7` | true | true | a claimed chunk: both halves run |
/// | [`NO_COLONY_ID`] (`0`) | **true** | false | the visit branch runs for an unclaimed chunk; the lookup that follows answers `null` |
/// | [`CHUNK_CAP_NO_COLONY`] (`-1`) | false | **true** | the visit branch is skipped and the **building loop still runs**, with colony id `-1`, which no manager holds |
///
/// The third row is why the sentinel matters: `-1` is not "no claim" for the second
/// guard, only for the first. And no writer produces it at all (`ChunkCapData.java:35`
/// starts its field at `0`, `IColonyTagCapability.java:120` at `NO_COLONY_ID`), so the
/// row describes what the code would do, not what a server ever does.
pub fn claim_effects(owning_colony: i32) -> ClaimEffects {
    ClaimEffects {
        visit: owning_colony != CHUNK_CAP_NO_COLONY,
        buildings: owning_colony != NO_COLONY_ID,
    }
}

// ── an enemy spawning inside a building ──────────────────────────────────────

/// One claiming building of the owning colony, as `:450-457` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingCover {
    /// `building.getBuildingLevel()`.
    pub level: i32,
    /// `building.isInBuilding(pos)`.
    pub covers: bool,
}

/// `EventHandler.java:424-459`: `true` when the spawn is denied.
///
/// The three early returns before the loop are the caller's (`:427-448`): not an
/// `Enemy`, a spawner's spawn, a client, an unloaded position, no colony, no
/// manager entry. What is left is the rule.
pub fn spawn_denied(buildings: &[BuildingCover]) -> bool {
    buildings
        .iter()
        .any(|building| building.level >= 1 && building.covers)
}

// ── a zombie villager the tavern can take ────────────────────────────────────

/// `EventHandler.java:827`: the conversion the handler intercepts, and the only one.
pub fn conversion_is_ours(kind: &str, outcome: &str) -> bool {
    kind == "minecraft:zombie_villager" && outcome == "minecraft:villager"
}

// ── the tick slowness ────────────────────────────────────────────────────────

/// `EventHandler.java:864-875`, the measured tick length turned into a factor.
///
/// `tickTimes[tickCount % 100]` is in nanoseconds and Java divides by `1.0E6`; the
/// caller passes milliseconds. Over 50 ms the factor is the ratio clamped to
/// `[1, 5]`; at or under it, exactly 1. The clamp is not symmetry for its own sake:
/// a factor of 20 would make a state machine skip twenty of its own ticks, so Java
/// caps how far a bad server can slow it.
pub fn slowness_factor(last_tick_ms: f64) -> f64 {
    if last_tick_ms > SLOWNESS_TRIGGER_MS {
        (last_tick_ms / SLOWNESS_TRIGGER_MS).clamp(1.0, SLOWNESS_CEILING)
    } else {
        1.0
    }
}

/// The tick length the rule reads, from the hundred durations the host keeps.
///
/// `ServerHost::tick_times()` answers the last hundred ticks **in nanoseconds**, "the
/// tick numbered `n` at index `n % 100`, 0 where none was recorded yet"
/// (`crates/horizon-mod-vocab/src/host/config.rs:96-102`) — the same layout as
/// `MinecraftServer.getTickTimesNanos`, which is what
/// `event.getServer().tickTimes[tickCount % 100]` indexes.
///
/// An index the slice does not hold answers `0.0`, which the rule reads as "a fast
/// tick". Java's array is always a hundred long, so the arm exists only because a
/// `Vec` from the host is not statically sized here.
pub fn tick_duration_ms(times_nanos: &[u64], tick: u64) -> f64 {
    let index = (tick % TICK_RING) as usize;
    times_nanos.get(index).copied().unwrap_or(0) as f64 * 1.0e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1.0e-6
    }

    #[test]
    fn grass_gets_a_tenth_of_the_chance_of_any_other_block() {
        assert!(close(crop_chance(true), 0.001));
        assert!(close(crop_chance(false), 0.01));
        assert!(close(crop_chance(false), crop_chance(true) * 10.0));
    }

    #[test]
    fn the_hoe_ladder_is_ten_eight_six_five_four_one() {
        let factors: Vec<f32> = [
            Some(Hoe::Netherite),
            Some(Hoe::Diamond),
            Some(Hoe::Iron),
            Some(Hoe::Golden),
            Some(Hoe::Other),
            None,
        ]
        .into_iter()
        .map(|hoe| match hoe {
            Some(hoe) => hoe.factor(),
            None => 1.0,
        })
        .collect();
        assert_eq!(factors, vec![10.0, 8.0, 6.0, 5.0, 4.0, 1.0]);
    }

    #[test]
    fn a_golden_hoe_is_worse_than_an_iron_one() {
        // `EventHandler.java:205-209` lists iron (×6) before golden (×5); the ladder
        // is ordered by the Java's `otherwise` chain, not by material tier.
        assert!(Hoe::Golden.factor() < Hoe::Iron.factor());
        assert!(Hoe::Golden.factor() > Hoe::Other.factor());
    }

    #[test]
    fn a_netherite_hoe_on_grass_reaches_the_ordinary_chance() {
        assert!(close(crop_drop_chance(true, Some(Hoe::Netherite)), 0.01));
        assert!(close(crop_drop_chance(false, Some(Hoe::Netherite)), 0.1));
    }

    #[test]
    fn the_ladder_has_six_rungs_best_first() {
        let ladder = crop_ladder(false);
        assert_eq!(ladder.len(), 6);
        for pair in ladder.windows(2) {
            assert!(
                pair[0].1 > pair[1].1,
                "the ladder is not strictly decreasing"
            );
        }
        assert_eq!(ladder[5].0, None, "the last rung is 'no hoe'");
    }

    #[test]
    fn the_dungeon_pool_is_half_the_plain_chance() {
        assert!(close(DUNGEON_CROP_CHANCE, CROP_CHANCE_OTHER / 2.0));
    }

    fn join(fast_colony_entity: bool, citizen: bool, already_present: bool, colony_id: Option<i32>) -> Join {
        Join {
            fast_colony_entity,
            citizen,
            already_present,
            colony_id,
        }
    }

    #[test]
    fn a_citizen_of_colony_zero_never_joins() {
        assert_eq!(
            join_verdict(&join(true, true, false, Some(0))),
            Err(JoinRefusal::ColonyZero)
        );
    }

    #[test]
    fn a_duplicate_is_refused_before_the_colony_is_looked_at() {
        assert_eq!(
            join_verdict(&join(true, true, true, Some(0))),
            Err(JoinRefusal::Duplicate)
        );
    }

    #[test]
    fn the_duplicate_test_only_applies_to_colony_entities() {
        // `event.getEntity() instanceof AbstractFastMinecoloniesEntity` (`:148`)
        assert_eq!(join_verdict(&join(false, false, true, None)), Ok(()));
    }

    #[test]
    fn the_colony_test_only_applies_to_citizens() {
        // A visitor or a mercenary is a fast colony entity, not an `EntityCitizen`
        // (`:153`), so colony id 0 is not a reason to refuse it — the audit's first
        // correction, and the reason the two class tests cannot share one flag.
        assert_eq!(join_verdict(&join(true, false, false, Some(0))), Ok(()));
    }

    #[test]
    fn a_citizen_with_a_colony_joins() {
        assert_eq!(join_verdict(&join(true, true, false, Some(4))), Ok(()));
    }

    #[test]
    fn the_attack_goals_are_priority_six_then_seven() {
        let goals = attack_goals(true, true, false, false);
        assert_eq!(goals.len(), 2);
        assert_eq!(goals[0].priority, 6);
        assert_eq!(goals[1].priority, 7);
        assert_eq!(goals[0].prey, "minecolonies:citizen");
        assert_eq!(goals[1].prey, "minecolonies:mercenary");
        assert!(goals.iter().all(|goal| goal.must_see && !goal.must_reach));
    }

    #[test]
    fn a_blacklisted_mob_gets_no_goal() {
        assert!(attack_goals(true, true, true, false).is_empty());
    }

    #[test]
    fn a_colony_entity_does_not_hunt_its_own() {
        assert!(attack_goals(true, true, false, true).is_empty());
    }

    #[test]
    fn the_config_switch_is_the_first_term() {
        assert!(attack_goals(false, true, false, false).is_empty());
        assert!(attack_goals(true, false, false, false).is_empty());
    }

    fn over_world(creative: bool) -> Placement {
        Placement {
            other_dimension_allowed: false,
            overworld: true,
            town_hall: false,
            in_colony: false,
            owns_a_colony: false,
            may_place_huts: false,
            creative,
            can_place_at: false,
        }
    }

    #[test]
    fn a_colony_in_another_dimension_is_refused_before_anything_else() {
        let place = Placement {
            overworld: false,
            ..over_world(true)
        };
        assert_eq!(
            hut_placement(&place),
            Verdict::answer(false, Notice::OtherDimension),
            "even creative does not pass (`:711-715`)"
        );
    }

    #[test]
    fn the_other_dimension_ban_can_be_switched_off() {
        let place = Placement {
            other_dimension_allowed: true,
            overworld: false,
            town_hall: true,
            ..over_world(false)
        };
        assert_eq!(hut_placement(&place), Verdict::allow());
    }

    #[test]
    fn a_town_hall_is_the_one_hut_allowed_outside_a_colony() {
        let place = Placement {
            town_hall: true,
            ..over_world(false)
        };
        assert_eq!(hut_placement(&place), Verdict::allow());
    }

    #[test]
    fn a_hut_outside_a_colony_warns_and_then_asks_creative() {
        let warned = hut_placement(&over_world(false));
        assert_eq!(warned, Verdict::answer(false, Notice::TownHallNotPresent));
        let creative = hut_placement(&over_world(true));
        assert!(
            creative.allow,
            "`return player.isCreative()` (`:741`): the message does not decide"
        );
        assert_eq!(creative.notice, Some(Notice::TownHallNotPresent));
    }

    #[test]
    fn the_warning_names_the_colony_the_player_already_owns() {
        let place = Placement {
            owns_a_colony: true,
            ..over_world(false)
        };
        assert_eq!(
            hut_placement(&place).notice,
            Some(Notice::TownHallTooFarAway)
        );
    }

    #[test]
    fn place_huts_refuses_even_a_creative_player() {
        let place = Placement {
            in_colony: true,
            may_place_huts: false,
            creative: true,
            can_place_at: true,
            ..over_world(true)
        };
        assert_eq!(
            hut_placement(&place),
            Verdict::answer(false, Notice::PermissionOpenHut),
            "`:743-748` is tested before `creative` (`:751`) can help"
        );
    }

    #[test]
    fn creative_replaces_can_place_at_and_nothing_else() {
        let refused = Placement {
            in_colony: true,
            may_place_huts: true,
            creative: false,
            can_place_at: false,
            ..over_world(false)
        };
        assert_eq!(hut_placement(&refused), Verdict::allow().with_allow(false));
        let creative = Placement {
            creative: true,
            ..refused
        };
        assert_eq!(hut_placement(&creative), Verdict::allow());
    }

    #[test]
    fn a_foot_click_resolves_to_the_head_along_facing() {
        assert_eq!(
            bed_head(BedHalf::Foot, [10, 64, 10], [0, 0, 1]),
            [10, 64, 11]
        );
        assert_eq!(
            bed_head(BedHalf::Head, [10, 64, 11], [0, 0, 1]),
            [10, 64, 11],
            "a head click stays where it clicked"
        );
    }

    #[test]
    fn only_an_asleep_citizen_whose_bed_it_is_blocks_the_click() {
        let citizens = [
            ([10, 64, 11], true),
            ([10, 64, 11], false),
            ([2, 64, 2], true),
        ];
        assert_eq!(bed_blockers([10, 64, 11], &citizens), vec![0]);
    }

    #[test]
    fn two_citizens_on_one_bed_produce_two_messages() {
        // Java has no `break` in `:624-631`.
        let citizens = [([10, 64, 11], true), ([10, 64, 11], true)];
        assert_eq!(bed_blockers([10, 64, 11], &citizens), vec![0, 1]);
    }

    fn working(due: ChunkEntry) -> bool {
        due.work
    }

    #[test]
    fn the_chunk_gate_fires_once_in_a_hundred_ticks() {
        assert!(working(chunk_entry_due(200, None, [1, 2], false)));
        assert!(!working(chunk_entry_due(201, None, [1, 2], false)));
        assert!(!working(chunk_entry_due(0, Some([1, 2]), [1, 2], false)));
        assert!(!working(chunk_entry_due(100, Some([1, 2]), [1, 2], false)));
        assert!(working(chunk_entry_due(100, Some([0, 0]), [1, 2], false)));
    }

    #[test]
    fn an_empty_chunk_is_remembered_but_not_entered() {
        // `playerPositions.put` is `:366`; the empty test that returns is `:370-373`.
        assert_eq!(
            chunk_entry_due(100, None, [1, 2], true),
            ChunkEntry {
                remember: true,
                work: false
            }
        );
    }

    #[test]
    fn a_tick_that_fails_the_gate_remembers_nothing() {
        assert_eq!(
            chunk_entry_due(101, None, [1, 2], false),
            ChunkEntry {
                remember: false,
                work: false
            }
        );
        assert_eq!(
            chunk_entry_due(100, Some([1, 2]), [1, 2], false),
            ChunkEntry {
                remember: false,
                work: false
            }
        );
    }

    #[test]
    fn the_chunk_tick_period_is_the_java_constant() {
        assert_eq!(CHUNK_TICK_PERIOD, 100);
        assert_eq!(CHUNK_RANGE, 8);
    }

    #[test]
    fn a_claimed_chunk_does_both_halves() {
        assert_eq!(
            claim_effects(7),
            ClaimEffects {
                visit: true,
                buildings: true
            }
        );
    }

    #[test]
    fn an_unclaimed_chunk_still_runs_the_visit_branch() {
        // `owning != -1` (`:387`) with `owning == 0`: true. The lookup that follows
        // answers null, so nothing happens — but the branch is taken, and a port
        // that skipped it would not be the Java's.
        assert_eq!(
            claim_effects(NO_COLONY_ID),
            ClaimEffects {
                visit: true,
                buildings: false
            }
        );
    }

    #[test]
    fn the_minus_one_sentinel_stops_only_the_first_guard() {
        // `:387` tests `!= -1` and `:399` tests `!= 0`: two sentinels, so `-1` silences
        // the visit branch and lets the building loop through with a colony id no
        // manager holds. A port that answered `false` twice would be reading the
        // author's intent instead of the code.
        assert_eq!(
            claim_effects(CHUNK_CAP_NO_COLONY),
            ClaimEffects {
                visit: false,
                buildings: true
            }
        );
        assert_eq!(NO_COLONY_ID, 0, "ColonyManagerConstants.java:56");
        assert_eq!(CHUNK_CAP_NO_COLONY, -1, "EventHandler.java:387");
        assert_ne!(
            NO_COLONY_ID, CHUNK_CAP_NO_COLONY,
            "the two guards do not share a sentinel"
        );
    }

    #[test]
    fn a_level_one_building_denies_the_spawn_it_covers() {
        assert!(spawn_denied(&[BuildingCover {
            level: 1,
            covers: true
        }]));
        assert!(!spawn_denied(&[BuildingCover {
            level: 0,
            covers: true
        }]));
        assert!(!spawn_denied(&[BuildingCover {
            level: 3,
            covers: false
        }]));
    }

    #[test]
    fn any_one_covering_building_is_enough() {
        let buildings = [
            BuildingCover {
                level: 2,
                covers: false,
            },
            BuildingCover {
                level: 1,
                covers: true,
            },
        ];
        assert!(spawn_denied(&buildings));
    }

    #[test]
    fn only_a_zombie_villager_turning_into_a_villager_is_ours() {
        assert!(conversion_is_ours(
            "minecraft:zombie_villager",
            "minecraft:villager"
        ));
        assert!(!conversion_is_ours("minecraft:zombie", "minecraft:drowned"));
        assert!(!conversion_is_ours(
            "minecraft:villager",
            "minecraft:zombie_villager"
        ));
    }

    #[test]
    fn a_tick_at_or_under_fifty_milliseconds_is_not_slowed() {
        assert_eq!(slowness_factor(49.9), 1.0);
        assert_eq!(slowness_factor(50.0), 1.0, "the test is `> 50`");
    }

    #[test]
    fn a_slow_tick_slows_the_state_machines_by_its_ratio() {
        assert!((slowness_factor(100.0) - 2.0).abs() < 1.0e-9);
        assert!((slowness_factor(75.0) - 1.5).abs() < 1.0e-9);
    }

    #[test]
    fn the_slowness_is_capped_at_five() {
        assert_eq!(slowness_factor(1_000.0), SLOWNESS_CEILING);
        assert_eq!(slowness_factor(f64::MAX), SLOWNESS_CEILING);
    }

    #[test]
    fn the_tick_duration_is_read_at_the_tick_number_modulo_a_hundred() {
        let mut times = vec![0u64; 100];
        times[23] = 75_000_000; // 75 ms
        assert!((tick_duration_ms(&times, 123) - 75.0).abs() < 1.0e-9);
        assert!((tick_duration_ms(&times, 23) - 75.0).abs() < 1.0e-9);
        assert_eq!(tick_duration_ms(&times, 24), 0.0);
    }

    #[test]
    fn a_vector_shorter_than_a_hundred_reads_as_a_fast_tick() {
        // Java's array is always 100 long; a host `Vec` need not be.
        assert_eq!(tick_duration_ms(&[], 7), 0.0);
        assert_eq!(tick_duration_ms(&[1_000_000], 5), 0.0);
        assert!((tick_duration_ms(&[2_000_000], 0) - 2.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_two_halves_of_the_rule_compose() {
        let mut times = vec![0u64; 100];
        times[4] = 120_000_000; // 120 ms
        assert!((slowness_factor(tick_duration_ms(&times, 4)) - 2.4).abs() < 1.0e-9);
        times[5] = 20_000_000; // 20 ms
        assert_eq!(slowness_factor(tick_duration_ms(&times, 5)), 1.0);
    }
}
