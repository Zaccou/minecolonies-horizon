//! When a colony is raided: the thresholds, the arithmetic of a raid's size and
//! difficulty, the night counter, and the save.
//!
//! Java reference: `core/colony/events/raid/RaidManager.java` (1 253 lines),
//! with `api/colony/managers/interfaces/IRaiderManager.java:19-25` (the result
//! enum) and `:191-207` (the settings record).
//!
//! **What is here and what is not.** A raid is two halves. The *world* half
//! searches a spawn point in a circle 500 blocks out (`:484-625`), finds ground
//! around it (`:526-562`), and picks one of eight event classes from the biome
//! and a coin flip (`:336-453`) — no port of that, it is `Level` all the way
//! down. The *rule* half is everything a refusal is made of: whether a raid may
//! happen at all, how many raiders, how hard, how many nights must pass, what a
//! death costs, and what the colony remembers afterwards. That is this file, and
//! it needs no world: the search is handed in as [`Point`]s, the coin as a
//! `roll: f64` in `[0, 1)`, the clock as `now: i64`.
//!
//! **The order of the writes is the rule.** `raiderEvent` (`:254-460`) refuses
//! four times before it writes anything — no world (`:256`), not allowed to raid
//! (`:260`), too small (`:275`), no spawn point (`:317`) — and only then pushes a
//! [`History`], zeroes the nights, drops the pending raid, and marks the colony
//! dirty (`:458`). A test below walks all four refusals and checks that the
//! manager is untouched after each.
//!
//! **One refusal is *not* atomic, and the port keeps it.** Deep inside the
//! event-type ladder, an explicit raid type the ladder does not compare against
//! falls to `return RaidSpawnResult.NO_SPAWN_POINT` (`:442-445`) — after the
//! history entry and the night reset. [`known_raid_type`] names the six types
//! the ladder accepts; `pirate_ground_raid` and `norsemen_ship_raid` are types it
//! *produces* and can never be *asked* for, which is what makes that path
//! reachable. [`Manager::raider_event`] reproduces it as
//! [`SpawnResult::NoSpawnPoint`] with the side effects already applied.
//!
//! **Three numbers that look alike and are not:**
//!
//! | Distance | Formula | Where |
//! | --- | --- | --- |
//! | building to spawn point | **Manhattan**, `abs(dx) + abs(dz)` | `BlockPosUtil.java:423-429` |
//! | spawn circle around the colony | Euclidean, radius 500 | `RaidManager.java:512-513` |
//! | guard's help radius | squared, `75 * 75` | `RaidManager.java:903` |
//!
//! [`spawn_is_allowed`] uses the first, and a test pins it: a point 20 blocks
//! east and 20 north is 40 away by Java's rule and 28.3 by the one that looks
//! obvious, so it is *allowed* where Euclid would refuse it.
//!
//! **A fact worth saying before a reader hunts for it:** `setPassThroughRaid`
//! (`:1100-1103`) has **no caller** in this tree, and once called
//! `passingThroughRaidTime` is never reset, so `passingThroughRaidTime > 0`
//! (`:740`) stays true for the rest of the colony's life. Both are Java's, both
//! are kept, and [`Manager::set_pass_through`] is the only way in.

use mod_api::{Compound, Tag};

use crate::events::Lifecycle;

// ---------------------------------------------------------------------------
// The thresholds, each with the line that fixes it.
// ---------------------------------------------------------------------------

/// Attacks per raid level, and the divisor of the raider count
/// (`RaidManager.java:81`).
pub const SPAWN_MODIFIER: f64 = 60.0;

/// Closest a spawn point may be to any building before the per-building bonus
/// (`RaidManager.java:86`).
pub const MIN_BUILDING_SPAWN_DIST: i32 = 35;

/// Above this share of the colony lost, the difficulty goes down
/// (`RaidManager.java:91`).
pub const LOST_CITIZEN_DIFF_REDUCE_PCT: f64 = 0.15;

/// Below this share, it goes up (`RaidManager.java:92`).
pub const LOST_CITIZEN_DIFF_INCREASE_PCT: f64 = 0.05;

/// The difficulty ladder's floor (`RaidManager.java:97`).
pub const MIN_RAID_DIFFICULTY: i32 = 1;

/// And its ceiling (`RaidManager.java:98`).
pub const MAX_RAID_DIFFICULTY: i32 = 14;

/// The floor of the difficulty *modifier*, added after the division by ten
/// (`RaidManager.java:103`).
pub const MIN_DIFFICULTY_MODIFIER: f64 = 0.2;

/// Below this raid level no raid happens at all (`RaidManager.java:115`).
pub const MIN_REQUIRED_RAID_LEVEL: i32 = 75;

/// Extra raiders per non-spectator player in range (`RaidManager.java:120`).
pub const INCREASE_PER_PLAYER: f64 = 0.05;

/// One chance in this many to ignore the biome when picking the event type
/// (`RaidManager.java:125`).
pub const IGNORE_BIOME_CHANCE: i32 = 2;

/// The difficulty a colony starts at (`RaidManager.java:130`).
pub const INITIAL_RAID_DIFFICULTY: i32 = 7;

/// The modifier a colony starts at (`RaidManager.java:140`).
pub const INITIAL_SPAWN_COUNT_ADJUSTED: f64 = 1.0;

/// A colony may be raided until a command says otherwise
/// (`RaidManager.java:145`).
pub const INITIAL_CAN_HAVE_RAIDER_EVENTS: bool = true;

/// The night counter a fresh colony starts on (`RaidManager.java:155`).
pub const INITIAL_NIGHTS_SINCE_LAST_RAID: i32 = 0;

/// `Constants.java:21`, the tick definition the pass-through window is written
/// with.
pub const TICKS_SECOND: i64 = 20;

/// `TICKS_SECOND * 20`, the length of a pass-through raid
/// (`RaidManager.java:1102`).
pub const PASS_THROUGH_TICKS: i64 = TICKS_SECOND * 20;

/// Above this many raiders a raid is split into several (`:297`).
pub const BIG_HORDE_SIZE: i32 = 20;

/// At or below this many, the victory sound is the short one (`:1075`).
pub const SMALL_HORDE_SIZE: i32 = 5;

/// The config's "normal" raid difficulty, the divisor of the config factor
/// (`Constants.java:27`).
pub const DEFAULT_BARBARIAN_DIFFICULTY: f64 = 5.0;

/// Share of the colony lost at which every raid of the wave is ended
/// (`:963`).
pub const MERCY_SHARE: f64 = 0.5;

/// What the colony compound stores (`NbtTagConstants.java:205`, `:210`, `:215`).
pub const KEY_RAIDABLE: &str = "raidable";
pub const KEY_NIGHTS_SINCE_LAST_RAID: &str = "nightsRaid";
pub const KEY_RAID_HISTORY: &str = "raidhistory";

/// The three keys the manager writes that Java keeps private
/// (`RaidManager.java:108-110`).
pub const KEY_DIFFICULTY: &str = "difficulty";
pub const KEY_DELAY: &str = "delay";

/// One history record's keys (`RaidManager.java:1123-1127`, `:1215`).
pub const KEY_LOST_CITIZENS: &str = "lostCitizens";
pub const KEY_RAIDER_AMOUNT: &str = "raiderAmount";
pub const KEY_RAID_TIME: &str = "raidTime";
pub const KEY_SPAWN_INFO: &str = "spawnInfo";
pub const KEY_RAID_TYPE: &str = "raidtype";

/// The six raid types an explicit request may name
/// (`RaidManager.java:346`, `:367`, `:393`, `:408`, `:413`, `:438`).
///
/// `pirate_ground_raid` (`PirateGroundRaidEvent.java:30`) and
/// `norsemen_ship_raid` (`NorsemenShipRaidEvent.java:27`) are **not** in it: the
/// ladder produces them (`:424`, `:370`) and never compares an incoming type
/// against them, so asking for one falls through to `:444`.
pub const REQUESTABLE_RAID_TYPES: &[&str] = &[
    "barbarian_raid",
    "drowned_pirate_raid",
    "egyptian_raid",
    "amazon_raid",
    "norsemen_raid",
    "pirate_raid",
];

/// Whether an explicit raid type survives the ladder (`:344-445`).
pub fn known_raid_type(name: &str) -> bool {
    REQUESTABLE_RAID_TYPES.contains(&name)
}

// ---------------------------------------------------------------------------
// What a raid attempt answers.
// ---------------------------------------------------------------------------

/// `IRaiderManager.RaidSpawnResult` (`:19-25`), in Java's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: Java's enum, five constants and nothing else
pub enum SpawnResult {
    Success,
    TooSmall,
    CannotRaid,
    NoSpawnPoint,
    Error,
}

impl SpawnResult {
    /// A stable name for a log line and a test.
    pub fn name(self) -> &'static str {
        match self {
            Self::Success => "raid.spawn.success",
            Self::TooSmall => "raid.spawn.too_small",
            Self::CannotRaid => "raid.spawn.cannot_raid",
            Self::NoSpawnPoint => "raid.spawn.no_spawn_point",
            Self::Error => "raid.spawn.error",
        }
    }

    /// Whether a raid actually started. Only one answer means it did, which is
    /// what `CommandRaid.java:180` tests.
    pub fn started(self) -> bool {
        matches!(self, Self::Success)
    }

    /// Whether the attempt **consumed** tonight's raid slot
    /// (`RaidManager.java:770`): success, and too small — Java clears the slot
    /// for both, so a colony that has shrunk below 75 does not retry forever.
    pub fn resolves_tonight(self) -> bool {
        matches!(self, Self::Success | Self::TooSmall)
    }
}

// ---------------------------------------------------------------------------
// Buildings, and how far a spawn point must stay from one.
// ---------------------------------------------------------------------------

/// The four cases `isValidSpawnPoint` distinguishes (`:658-673`), in its order:
/// the tower first, the house second, the town hall third, everything else last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four branches of `:658-673`, and the final `else`
pub enum BuildingKind {
    GuardTower,
    Living,
    TownHall,
    Other,
}

/// A building, as the distance rule reads it: what kind, what level, where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Building {
    pub kind: BuildingKind,
    pub level: i32,
    pub at: [i32; 3],
}

impl Building {
    pub fn new(kind: BuildingKind, level: i32, at: [i32; 3]) -> Building {
        Building { kind, level, at }
    }
}

/// The distance a spawn point must keep from one building,
/// `isValidSpawnPoint` (`:655-673`): 35, plus the level times a per-kind
/// multiplier — 7 for a guard tower, 4 for a house, 8 for the town hall, 2 for
/// anything else.
///
/// The order of those branches is a rule: a building that is both a tower and
/// has a living module is a tower, because Java tests in this order.
pub fn min_spawn_distance(kind: BuildingKind, level: i32) -> i32 {
    let per_level = match kind {
        BuildingKind::GuardTower => 7,
        BuildingKind::Living => 4,
        BuildingKind::TownHall => 8,
        BuildingKind::Other => 2,
    };
    MIN_BUILDING_SPAWN_DIST + level * per_level
}

/// `isValidSpawnPoint` (`:646-682`), whole: a level-0 building is skipped, every
/// other one must be at least [`min_spawn_distance`] away, and the distance is
/// `BlockPosUtil.getDistance2D` — **Manhattan**, `abs(dx) + abs(dz)`
/// (`BlockPosUtil.java:423-429`), not the Euclidean one the name suggests.
pub fn spawn_is_allowed(buildings: &[Building], at: [i32; 3]) -> bool {
    for building in buildings {
        if building.level == 0 {
            continue;
        }
        if manhattan(building.at, at) < min_spawn_distance(building.kind, building.level) {
            return false;
        }
    }
    true
}

/// `BlockPosUtil.getDistance2D` (`:423-429`): `Math.abs(xDiff + zDiff)` over
/// longs, which is the Manhattan distance. The horizontal axis only, and the
/// height never enters it.
pub fn manhattan(a: [i32; 3], b: [i32; 3]) -> i32 {
    let dx = i64::from(a[0]) - i64::from(b[0]);
    let dz = i64::from(a[2]) - i64::from(b[2]);
    let sum = dx.abs() + dz.abs();
    i32::try_from(sum).unwrap_or(i32::MAX)
}

// ---------------------------------------------------------------------------
// The request and the world's answers.
// ---------------------------------------------------------------------------

/// `IRaiderManager.RaidSettings` (`:191-207`), field for field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub forced: bool,
    pub raid_type: Option<String>,
    pub allow_ships: bool,
    pub raider_amount: Option<i32>,
    pub location: Option<[i32; 3]>,
}

impl Default for Settings {
    /// `RaidSettings.defaultRaidSettings()` (`:203-206`): not forced, no type,
    /// ships allowed, no fixed amount, no location.
    fn default() -> Settings {
        Settings {
            forced: false,
            raid_type: None,
            allow_ships: true,
            raider_amount: None,
            location: None,
        }
    }
}

/// One spawn point, with the raid type the world's ladder chose for it
/// (`:336-453`, recorded at `:455`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub at: [i32; 3],
    pub raid_type: Option<String>,
}

impl Point {
    pub fn new(at: [i32; 3], raid_type: Option<&str>) -> Point {
        Point {
            at,
            raid_type: raid_type.map(str::to_owned),
        }
    }
}

/// Everything Java reads from the world, the config and the colony, handed in.
pub struct Env<'a> {
    /// `colony.getWorld() == null` (`:256`).
    pub world_present: bool,
    /// `canRaid()` (`:260`, `:795-801`), already answered.
    pub can_raid: bool,
    /// `getColonyRaidLevel()` (`:265`, ported as [`colony_raid_level`]).
    pub raid_level: i32,
    /// `maxRaiders` from the config (`:281`).
    pub max_raiders: i32,
    /// Non-spectator players in the colony (`:704-711`).
    pub players: i32,
    /// `ColonyConstants.rand.nextDouble()` (`:717`), as a value in `[0, 1)`.
    pub roll: f64,
    /// The config's `raidDifficulty` (`:935`).
    pub config_raid_difficulty: f64,
    /// `world.getDifficulty().getId()` as a double (`:936`).
    pub vanilla_difficulty: f64,
    /// The spawn points the world search found (`:288-315`).
    pub points: &'a [Point],
}

/// What a successful attempt produced: the split count and the spawn points,
/// with the difficulty Java stamps into the history record (`:456`).
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    /// Raiders per spawn point, `ceil(amount / points)` (`:325`).
    pub per_point: i32,
    /// The points, in the order they were handed in.
    pub points: Vec<Point>,
    /// `((int) (modifier * 100)) / 100.0` (`:456`): a double, truncated to two
    /// decimals, because Java casts an int in the middle.
    pub difficulty: f64,
}

/// One raid, as `RaidHistory` keeps it (`:1118-1205`).
#[derive(Clone, Debug, PartialEq)]
pub struct History {
    pub lost_citizens: i32,
    pub raider_amount: i32,
    pub dead_raiders: i32,
    pub raid_time: i64,
    pub difficulty: f64,
    pub spawns: Vec<Point>,
}

impl History {
    /// `new RaidHistory(amount, world.getGameTime())` (`:322`, `:1159-1163`).
    pub fn new(raider_amount: i32, raid_time: i64) -> History {
        History {
            lost_citizens: 0,
            raider_amount,
            dead_raiders: 0,
            raid_time,
            difficulty: 0.0,
            spawns: Vec::new(),
        }
    }
}

/// One step of `onNightFall` (`:738-780`), so a test reads what the night did
/// instead of guessing from the fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NightStep {
    /// The difficulty moved, and this is its new value (`:750`, `:755`).
    Difficulty(i32),
    /// The next raid was pushed back by that many nights (`:751`).
    Delay(i32),
    /// The night counter's new value (`:760`, `:764`).
    Nights(i32),
    /// `nextRaid` was set: the caller runs [`Manager::raider_event`] with
    /// [`Manager::next_settings`] (`:769`) and then calls
    /// [`Manager::raid_resolved`] (`:770-774`).
    RaidDue,
    /// No raid was pending and one was drawn for tomorrow, or not
    /// (`:778`, `:806-816`).
    Scheduled(bool),
}

/// What `onRaidEventFinished` asks its caller to do (`:1048-1082`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Finished {
    /// Another raid of the wave is still going, so nothing happened (`:1050-1056`).
    pub still_running: bool,
    /// The mercy ending: the oldest raid of the wave cost more than half the
    /// colony (`:1058-1062`).
    pub mercy: bool,
    /// The colony lost nobody, so the happiness modifier is injected
    /// (`:1078-1081`).
    pub unblemished: bool,
    /// The horde the victory sound is chosen by (`:1075`): the oldest record's
    /// size.
    pub raiders: i32,
}

/// Why a read or a completion did not happen, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A spawn record has no `raidtype`, or an empty one:
    /// `new ResourceLocation("")` throws (`:1245`), *outside* the `try` of
    /// `Colony.loadColony`, so the colony is lost. The port names the record.
    SpawnWithoutType { record: usize, spawn: usize },
    /// `onRaidEventFinished` dereferences `raidHistories.get(0)` (`:1058`) with
    /// no emptiness check; the port refuses instead of panicking.
    NoHistory,
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SpawnWithoutType { .. } => "raid.refusal.spawn_without_type",
            Self::NoHistory => "raid.refusal.no_history",
        }
    }
}

/// A read's repairs: what was defaulted or dropped, which is the half of
/// compatibility a `Result` cannot say (`:992-1024` has no refusal of its own
/// except the spawn type above).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own repair set
pub enum Repair {
    /// The key was absent, so Java's `getInt` answered 0 and `Mth.clamp` moved
    /// it to 1 — not to the initial 7 (`:1013`).
    DifficultyClamped { to: i32 },
    /// No `raidhistory` list: the histories are kept as they were (`:1015`).
    NoHistoryList,
    /// An entry of the history list that is not a compound: skipped, as vanilla's
    /// typed `getList` skips it.
    NotACompound { index: usize },
    /// No `nightsRaid`: the counter keeps its current value (`:1003-1006`).
    NoNights,
    /// No `raidable`: the colony is raidable (`:998-1001`).
    NoRaidable,
}

impl Repair {
    pub fn name(self) -> &'static str {
        match self {
            Self::DifficultyClamped { .. } => "raid.repair.difficulty_clamped",
            Self::NoHistoryList => "raid.repair.no_history_list",
            Self::NotACompound { .. } => "raid.repair.not_a_compound",
            Self::NoNights => "raid.repair.no_nights",
            Self::NoRaidable => "raid.repair.no_raidable",
        }
    }
}

// ---------------------------------------------------------------------------
// The manager.
// ---------------------------------------------------------------------------

/// One colony's raid state, as `RaidManager`'s fields hold it (`:135-201`).
#[derive(Clone, Debug, PartialEq)]
pub struct Manager {
    difficulty: i32,
    spawn_count_adjusted: f64,
    have_raider_events: bool,
    nights_since_last_raid: i32,
    spies: bool,
    extra_days: i32,
    passing_through: i64,
    next_raid: Option<Settings>,
    history: Vec<History>,
    dirty: bool,
}

impl Default for Manager {
    fn default() -> Manager {
        Manager::new()
    }
}

impl Manager {
    /// A fresh colony (`:130-160`).
    pub fn new() -> Manager {
        Manager {
            difficulty: INITIAL_RAID_DIFFICULTY,
            spawn_count_adjusted: INITIAL_SPAWN_COUNT_ADJUSTED,
            have_raider_events: INITIAL_CAN_HAVE_RAIDER_EVENTS,
            nights_since_last_raid: INITIAL_NIGHTS_SINCE_LAST_RAID,
            spies: false,
            extra_days: 0,
            passing_through: 0,
            next_raid: None,
            history: Vec::new(),
            dirty: false,
        }
    }

    pub fn can_have_raider_events(&self) -> bool {
        self.have_raider_events
    }

    /// `setCanHaveRaiderEvents` (`:225-229`): no dirty mark.
    pub fn set_can_have_raider_events(&mut self, can_have: bool) {
        self.have_raider_events = can_have;
    }

    pub fn will_raid_tonight(&self) -> bool {
        self.next_raid.is_some()
    }

    /// `setRaidNextNight` (`:231-235`), `None` meaning "stop the next one".
    pub fn set_raid_next_night(&mut self, settings: Option<Settings>) {
        self.next_raid = settings;
    }

    pub fn next_settings(&self) -> Option<Settings> {
        self.next_raid.clone()
    }

    pub fn spies(&self) -> bool {
        self.spies
    }

    /// `setSpiesEnabled` (`:243-251`): the colony is marked dirty **only** when
    /// the value moves, which is the one place in this file with that rule.
    pub fn set_spies(&mut self, enabled: bool) {
        if self.spies != enabled {
            self.dirty = true;
        }
        self.spies = enabled;
    }

    pub fn difficulty(&self) -> i32 {
        self.difficulty
    }

    pub fn spawn_count_adjusted(&self) -> f64 {
        self.spawn_count_adjusted
    }

    pub fn nights_since_last_raid(&self) -> i32 {
        self.nights_since_last_raid
    }

    /// `setNightsSinceLastRaid` (`:788-792`).
    pub fn set_nights_since_last_raid(&mut self, nights: i32) {
        self.nights_since_last_raid = nights;
    }

    pub fn extra_days(&self) -> i32 {
        self.extra_days
    }

    pub fn history(&self) -> &[History] {
        &self.history
    }

    /// `getLastRaid` (`:1089-1097`), `None` instead of `null`.
    pub fn last_raid(&self) -> Option<&History> {
        self.history.last()
    }

    /// `getLostCitizen` (`:1027-1035`): the **last** record's losses.
    pub fn lost_citizens(&self) -> i32 {
        self.history.last().map_or(0, |raid| raid.lost_citizens)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// `isRaided` (`:721-735`): the pass-through window first, then any raid the
    /// colony still holds in an active status. `events` is
    /// `crate::events::Lifecycle`, which already answers
    /// `IColonyRaidEvent.isRaidActive()` for one id.
    pub fn is_raided(&self, now: i64, events: &Lifecycle) -> bool {
        if now <= self.passing_through {
            return true;
        }
        events.ids().into_iter().any(|id| events.is_active(id))
    }

    /// `getRaidDifficultyModifier` (`:932-938`): four factors, and the order
    /// matters only for readability, not for the value.
    ///
    /// ```text
    /// (difficulty / 10 + 0.2) * (config / 5) * (vanilla / 2) * spawnCountAdjusted
    /// ```
    pub fn difficulty_modifier(
        &self,
        config_raid_difficulty: f64,
        vanilla_difficulty: f64,
    ) -> f64 {
        ((f64::from(self.difficulty) / 10.0) + MIN_DIFFICULTY_MODIFIER)
            * (config_raid_difficulty / DEFAULT_BARBARIAN_DIFFICULTY)
            * (vanilla_difficulty / 2.0)
            * self.spawn_count_adjusted
    }

    /// `raiderEvent` (`:254-460`), with the world handed in as [`Env`].
    ///
    /// The four refusals come first and write nothing; the fifth
    /// ([`SpawnResult::NoSpawnPoint`] for an unrequestable type) comes after the
    /// writes, because Java's does (`:444`).
    pub fn raider_event(
        &mut self,
        settings: &Settings,
        env: &Env<'_>,
        now: i64,
    ) -> Result<Spawn, SpawnResult> {
        if !env.world_present {
            return Err(SpawnResult::Error);
        }
        if !settings.forced && !env.can_raid {
            return Err(SpawnResult::CannotRaid);
        }

        let amount = match settings.raider_amount {
            // A forced amount skips both the level floor and the size check
            // (`:267-271`).
            Some(fixed) => {
                self.spawn_count_adjusted = INITIAL_SPAWN_COUNT_ADJUSTED;
                fixed
            }
            None => {
                let modifier = self.difficulty_modifier(
                    env.config_raid_difficulty,
                    env.vanilla_difficulty,
                );
                let amount = raider_amount(env.raid_level, env.players, modifier, env.roll, env.max_raiders);
                if amount <= 0 || env.raid_level < MIN_REQUIRED_RAID_LEVEL {
                    return Err(SpawnResult::TooSmall);
                }
                // Only now, and never before the refusal above (`:280-285`).
                self.spawn_count_adjusted = INITIAL_SPAWN_COUNT_ADJUSTED;
                if amount >= env.max_raiders {
                    self.spawn_count_adjusted =
                        f64::from(amount) / f64::from(env.max_raiders);
                }
                amount
            }
        };

        if env.points.is_empty() {
            return Err(SpawnResult::NoSpawnPoint);
        }

        // ---- everything above refuses; everything below writes. ----
        self.history.push(History::new(amount, now));
        self.nights_since_last_raid = 0;
        self.next_raid = None;
        let per_point = ceil_div(amount, env.points.len());

        let difficulty = truncate_hundredths(
            self.difficulty_modifier(env.config_raid_difficulty, env.vanilla_difficulty),
        );
        let mut points = Vec::with_capacity(env.points.len());
        // The ladder refuses a type it does not compare against (`:442-445`),
        // and it refuses it after `:322-324` already ran. An explicit request is
        // checked once; a type the world handed in is checked per point.
        let mut refusal = match settings.raid_type.as_deref() {
            Some(name) if !known_raid_type(name) => Some(SpawnResult::NoSpawnPoint),
            _ => None,
        };
        for point in env.points {
            if refusal.is_some() {
                break;
            }
            if point
                .raid_type
                .as_deref()
                .is_some_and(|name| !known_raid_type(name))
            {
                refusal = Some(SpawnResult::NoSpawnPoint);
                continue;
            }
            points.push(point.clone());
        }

        if let Some(history) = self.history.last_mut() {
            history.difficulty = difficulty;
            history.spawns = points.clone();
        }

        match refusal {
            // `:444` returns **before** `:458`'s `colony.markDirty()`: the
            // history entry and the zeroed nights are in memory and will not be
            // saved by this call. The port keeps the leak, and says so.
            Some(result) => Err(result),
            None => {
                self.dirty = true;
                Ok(Spawn {
                    per_point,
                    points,
                    difficulty,
                })
            }
        }
    }

    /// `onNightFall` (`:738-780`), cut where the world begins: the difficulty
    /// bend, the delay, the night counter, and whether a raid is due.
    ///
    /// A due raid is **not** spawned here — [`NightStep::RaidDue`] says so, the
    /// caller runs it, and then calls [`Self::raid_resolved`], because
    /// `raiderEvent` needs a world this file does not have.
    #[allow(clippy::too_many_arguments)]
    pub fn night_fall(
        &mut self,
        now: i64,
        events: &Lifecycle,
        max_citizens: i32,
        min_nights: i32,
        avg_nights: i32,
        roll: f64,
        can_raid: bool,
    ) -> Vec<NightStep> {
        let mut steps = Vec::new();
        // `!isRaided() || passingThroughRaidTime > 0` (`:740`) — the second half
        // is nearly always true once a pass-through has been set, and Java never
        // resets it.
        if !self.is_raided(now, events) || self.passing_through > 0 {
            // Only the night right after a raid bends the difficulty
            // (`:742`): the counter is 0 for exactly one night.
            let first_night = self.nights_since_last_raid == 0;
            if let Some(last) = self.history.last().filter(|_| first_night) {
                let lost = f64::from(last.lost_citizens) / f64::from(max_citizens);
                if lost > LOST_CITIZEN_DIFF_REDUCE_PCT {
                    let drop = (lost / LOST_CITIZEN_DIFF_REDUCE_PCT) as i32;
                    self.difficulty = MIN_RAID_DIFFICULTY.max(self.difficulty - drop);
                    steps.push(NightStep::Difficulty(self.difficulty));
                    self.extra_days = (f64::from(avg_nights) * 0.4).ceil() as i32;
                    steps.push(NightStep::Delay(self.extra_days));
                } else if lost < LOST_CITIZEN_DIFF_INCREASE_PCT {
                    self.difficulty = MAX_RAID_DIFFICULTY.min(self.difficulty + 1);
                    steps.push(NightStep::Difficulty(self.difficulty));
                }
            }
            self.nights_since_last_raid += 1;
            steps.push(NightStep::Nights(self.nights_since_last_raid));
        } else {
            self.nights_since_last_raid = 0;
            steps.push(NightStep::Nights(0));
        }

        if self.next_raid.is_some() {
            steps.push(NightStep::RaidDue);
        } else {
            let raid = can_raid && self.raid_this_night(min_nights, avg_nights, roll);
            self.set_raid_next_night(if raid { Some(Settings::default()) } else { None });
            steps.push(NightStep::Scheduled(raid));
        }
        steps
    }

    /// `raidThisNight` (`:863-877`): before the minimum plus the delay, never;
    /// past the average by two, always; in between, one chance in
    /// `average - minimum`.
    pub fn raid_this_night(&self, min_nights: i32, avg_nights: i32, roll: f64) -> bool {
        if self.nights_since_last_raid < min_nights + self.extra_days {
            return false;
        }
        if self.nights_since_last_raid > avg_nights + 2 {
            return true;
        }
        roll < 1.0 / f64::from(avg_nights - min_nights)
    }

    /// What `onNightFall` does with the attempt it just made (`:769-774`): only
    /// success and "too small" clear the delay and the pending raid.
    pub fn raid_resolved(&mut self, result: SpawnResult) {
        if result.resolves_tonight() {
            self.extra_days = 0;
            self.next_raid = None;
        }
    }

    /// `onLostCitizen` (`:941-973`): a guard costs one, anyone else two, and
    /// past half the colony every raid of the wave is ended.
    ///
    /// `raided` is `is_raided(...)` at the caller's clock, passed in so this
    /// stays a pure rule. `None` is Java's early `return`: no raid, or no
    /// history, means the death is not this manager's business.
    pub fn lost_citizen(
        &mut self,
        guard: bool,
        raided: bool,
        max_citizens: i32,
    ) -> Option<bool> {
        if !raided {
            return None;
        }
        let last = self.history.last_mut()?;
        last.lost_citizens += if guard { 1 } else { 2 };
        Some(f64::from(last.lost_citizens) / f64::from(max_citizens) > MERCY_SHARE)
    }

    /// `onRaiderDeath` (`:1038-1045`): one more dead raider on the last record.
    pub fn raider_died(&mut self) {
        if let Some(last) = self.history.last_mut() {
            last.dead_raiders += 1;
        }
    }

    /// `onRaidEventFinished` (`:1048-1082`): the running check first, then the
    /// **oldest** record's losses (`raidHistories.get(0)`, `:1058`) — not the
    /// last one, which is what the rest of the class uses. Keeping it is the
    /// point: it is Java's observable behaviour, and it decides the mercy
    /// ending.
    pub fn raid_finished(
        &mut self,
        still_running: bool,
        avg_nights: i32,
        max_citizens: i32,
    ) -> Result<Finished, Refusal> {
        if still_running {
            return Ok(Finished {
                still_running: true,
                ..Finished::default()
            });
        }
        let oldest = self.history.first().ok_or(Refusal::NoHistory)?.clone();
        let mercy =
            f64::from(oldest.lost_citizens) / f64::from(max_citizens) > MERCY_SHARE;
        if mercy {
            self.extra_days = avg_nights * 2;
        }
        Ok(Finished {
            still_running: false,
            mercy,
            unblemished: self.lost_citizens() == 0,
            raiders: oldest.raider_amount,
        })
    }

    /// `setPassThroughRaid` (`:1100-1103`): the colony counts as raided for
    /// [`PASS_THROUGH_TICKS`] ticks. No caller in this tree.
    pub fn set_pass_through(&mut self, now: i64) {
        self.passing_through = now + PASS_THROUGH_TICKS;
    }

    /// `write` (`:976-989`): four flat keys and the history list.
    ///
    /// The `delay` key is written but never read by Java's own `read`? It is
    /// read (`:1008-1011`); what is never written is the pass-through clock and
    /// the pending raid, so both are lost across a restart.
    pub fn write(&self) -> Compound {
        let records: Vec<Tag> = self
            .history
            .iter()
            .map(|history| Tag::Compound(history_compound(history)))
            .collect();
        Compound::from([
            (
                KEY_RAIDABLE.into(),
                Tag::Byte(if self.have_raider_events { 1 } else { 0 }),
            ),
            (
                KEY_NIGHTS_SINCE_LAST_RAID.into(),
                Tag::Int(self.nights_since_last_raid),
            ),
            (KEY_DIFFICULTY.into(), Tag::Int(self.difficulty)),
            (KEY_DELAY.into(), Tag::Int(self.extra_days)),
            (KEY_RAID_HISTORY.into(), Tag::List(records)),
        ])
    }

    /// `read` (`:992-1024`), with the two things a `Result` cannot say kept as
    /// [`Repair`]s.
    ///
    /// **One consequence of Java's clamp.** A save with no `difficulty` key
    /// comes back at [`MIN_RAID_DIFFICULTY`], not at the initial 7, because
    /// `getInt` answers 0 and `Mth.clamp` lifts it to 1 (`:1013`). A colony
    /// saved by an older format therefore raids softer than a new one.
    pub fn read(&mut self, compound: &Compound) -> Result<Vec<Repair>, Refusal> {
        let mut repairs = Vec::new();

        match bool_at(compound, KEY_RAIDABLE) {
            Some(can) => self.set_can_have_raider_events(can),
            None => {
                self.set_can_have_raider_events(true);
                repairs.push(Repair::NoRaidable);
            }
        }

        match int_at(compound, KEY_NIGHTS_SINCE_LAST_RAID) {
            Some(nights) => self.set_nights_since_last_raid(nights),
            None => repairs.push(Repair::NoNights),
        }

        if let Some(delay) = int_at(compound, KEY_DELAY) {
            self.extra_days = delay;
        }

        let read = int_at(compound, KEY_DIFFICULTY).unwrap_or(0);
        let clamped = read.clamp(MIN_RAID_DIFFICULTY, MAX_RAID_DIFFICULTY);
        self.difficulty = clamped;
        if clamped != read {
            repairs.push(Repair::DifficultyClamped { to: clamped });
        }

        match compound.get(&KEY_RAID_HISTORY.into()) {
            Some(Tag::List(records)) => {
                self.history.clear();
                for (index, record) in records.iter().enumerate() {
                    let Tag::Compound(record) = record else {
                        repairs.push(Repair::NotACompound { index });
                        continue;
                    };
                    self.history.push(history_from(record, self.history.len())?);
                }
            }
            _ => repairs.push(Repair::NoHistoryList),
        }

        Ok(repairs)
    }
}

// ---------------------------------------------------------------------------
// The two counts Java computes from the colony.
// ---------------------------------------------------------------------------

/// One citizen, as `getColonyRaidLevel` reads it (`:828-840`): whether it is a
/// child, and the sum of its skill levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resident {
    pub child: bool,
    pub skill_sum: i32,
}

impl Resident {
    pub fn adult(skill_sum: i32) -> Resident {
        Resident {
            child: false,
            skill_sum,
        }
    }

    pub fn child() -> Resident {
        Resident {
            child: true,
            skill_sum: 0,
        }
    }
}

/// `getColonyRaidLevel` (`:823-854`), whole:
///
/// ```text
/// resident:  5 + skillSum / 100, adults only
/// building:  5 + level * level / 5, level > 0 only
/// research:  3 per completed research
/// result:    (int) (levels * min(1, citizens / maxCitizens))
/// ```
///
/// The last line is a **double** multiplication cast back to `int`, and the
/// share is capped at 1: a colony over its cap does not get easier to raid, it
/// just stops getting harder.
pub fn colony_raid_level(
    residents: &[Resident],
    buildings: &[Building],
    completed_research: usize,
    citizens: i32,
    max_citizens: i32,
) -> i32 {
    let mut levels: i32 = 0;
    for resident in residents {
        if resident.child {
            continue;
        }
        levels += 5;
        levels += resident.skill_sum / 100;
    }
    for building in buildings {
        if building.level > 0 {
            levels += 5 + (building.level * building.level) / 5;
        }
    }
    let research = i32::try_from(completed_research).unwrap_or(i32::MAX);
    levels = levels.saturating_add(research * 3);

    // Java divides two doubles and `Math.min(1, NaN)` is NaN, whose `(int)` cast
    // is 0; Rust's `f64::min` returns the other operand for NaN, so the zero cap
    // is written out instead of relied on.
    let share = if max_citizens <= 0 {
        0.0
    } else {
        (f64::from(citizens) / f64::from(max_citizens)).min(1.0)
    };
    (f64::from(levels) * share) as i32
}

/// `calculateRaiderAmount` (`:702-718`):
///
/// ```text
/// 1 + min(maxRaiders, (int) ((level / 60) * modifier * (1 + players * 0.05)
///                            * (roll * 0.3 + 0.85)))
/// ```
///
/// The cast to `int` happens **before** the clamp and the `+1` after it, which is
/// why the answer is never below 1 even for a colony of raid level 0.
pub fn raider_amount(
    raid_level: i32,
    players: i32,
    modifier: f64,
    roll: f64,
    max_raiders: i32,
) -> i32 {
    let scaled = (f64::from(raid_level) / SPAWN_MODIFIER)
        * modifier
        * (1.0 + f64::from(players) * INCREASE_PER_PLAYER)
        * ((roll * 0.3) + 0.85);
    let truncated = scaled as i32;
    1 + truncated.min(max_raiders)
}

/// `(int) Math.ceil((float) amount / sizes)` (`:325`): the division is done in
/// `f32`, which is what Java's cast to `float` makes of it.
fn ceil_div(amount: i32, sizes: usize) -> i32 {
    ((amount as f32) / (sizes as f32)).ceil() as i32
}

/// `((int) (x * 100)) / 100.0` (`:456`): two decimals, truncated, not rounded.
fn truncate_hundredths(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}

// ---------------------------------------------------------------------------
// The record's own NBT.
// ---------------------------------------------------------------------------

fn history_compound(history: &History) -> Compound {
    let spawns: Vec<Tag> = history
        .spawns
        .iter()
        .map(|spawn| {
            Tag::Compound(Compound::from([
                (
                    KEY_RAID_TYPE.into(),
                    Tag::String(
                        spawn
                            .raid_type
                            .clone()
                            .unwrap_or_default()
                            .as_str()
                            .into(),
                    ),
                ),
                ("x".into(), Tag::Int(spawn.at[0])),
                ("y".into(), Tag::Int(spawn.at[1])),
                ("z".into(), Tag::Int(spawn.at[2])),
            ]))
        })
        .collect();
    Compound::from([
        (KEY_LOST_CITIZENS.into(), Tag::Int(history.lost_citizens)),
        (KEY_RAIDER_AMOUNT.into(), Tag::Int(history.raider_amount)),
        (KEY_RAID_TIME.into(), Tag::Long(history.raid_time)),
        (
            KEY_DIFFICULTY.into(),
            Tag::Double(history.difficulty.to_bits()),
        ),
        (KEY_SPAWN_INFO.into(), Tag::List(spawns)),
    ])
}

/// `RaidHistory.fromNBT` (`:1181-1193`): every key defaults, one does not.
fn history_from(record: &Compound, index: usize) -> Result<History, Refusal> {
    let mut history = History::new(
        int_at(record, KEY_RAIDER_AMOUNT).unwrap_or(0),
        long_at(record, KEY_RAID_TIME).unwrap_or(0),
    );
    history.lost_citizens = int_at(record, KEY_LOST_CITIZENS).unwrap_or(0);
    history.difficulty = double_at(record, KEY_DIFFICULTY).unwrap_or(0.0);
    if let Some(Tag::List(spawns)) = record.get(&KEY_SPAWN_INFO.into()) {
        for (spawn_index, spawn) in spawns.iter().enumerate() {
            let Tag::Compound(spawn) = spawn else {
                continue;
            };
            let raid_type = text_at(spawn, KEY_RAID_TYPE).unwrap_or_default();
            if raid_type.is_empty() {
                return Err(Refusal::SpawnWithoutType {
                    record: index,
                    spawn: spawn_index,
                });
            }
            history.spawns.push(Point::new(
                [
                    int_at(spawn, "x").unwrap_or(0),
                    int_at(spawn, "y").unwrap_or(0),
                    int_at(spawn, "z").unwrap_or(0),
                ],
                Some(&raid_type),
            ));
        }
    }
    Ok(history)
}

// ---------------------------------------------------------------------------
// Tolerant reads, one place.
// ---------------------------------------------------------------------------

fn bool_at(data: &Compound, key: &str) -> Option<bool> {
    match data.get(&key.into())? {
        Tag::Byte(value) => Some(*value != 0),
        // Java's `getBoolean` is type-exact and answers false for these; the
        // port reads the three widths a hand-edited save may use, the same
        // tolerance `crate::colony::nbt` applies to its integers.
        Tag::Int(value) => Some(*value != 0),
        Tag::Short(value) => Some(*value != 0),
        _ => None,
    }
}

fn int_at(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into())? {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

fn long_at(data: &Compound, key: &str) -> Option<i64> {
    match data.get(&key.into())? {
        Tag::Long(value) => Some(*value),
        Tag::Int(value) => Some(i64::from(*value)),
        _ => None,
    }
}

fn double_at(data: &Compound, key: &str) -> Option<f64> {
    match data.get(&key.into())? {
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        _ => None,
    }
}

fn text_at(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into())? {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Raid, Status};

    fn env<'a>(points: &'a [Point]) -> Env<'a> {
        Env {
            world_present: true,
            can_raid: true,
            raid_level: 100,
            max_raiders: 100,
            players: 1,
            roll: 0.5,
            config_raid_difficulty: 5.0,
            vanilla_difficulty: 2.0,
            points,
        }
    }

    fn two_points() -> Vec<Point> {
        vec![
            Point::new([10, 64, 10], Some("barbarian_raid")),
            Point::new([-10, 64, -10], Some("norsemen_raid")),
        ]
    }

    fn fresh_colony() -> Lifecycle {
        Lifecycle::new()
    }

    #[test]
    fn the_five_results_and_what_each_one_consumes() {
        // `IRaiderManager.java:19-25`, and `:770` for the slot.
        assert_eq!(SpawnResult::Success.name(), "raid.spawn.success");
        assert_eq!(SpawnResult::TooSmall.name(), "raid.spawn.too_small");
        assert_eq!(SpawnResult::CannotRaid.name(), "raid.spawn.cannot_raid");
        assert_eq!(SpawnResult::NoSpawnPoint.name(), "raid.spawn.no_spawn_point");
        assert_eq!(SpawnResult::Error.name(), "raid.spawn.error");

        assert!(SpawnResult::Success.started());
        for other in [
            SpawnResult::TooSmall,
            SpawnResult::CannotRaid,
            SpawnResult::NoSpawnPoint,
            SpawnResult::Error,
        ] {
            assert!(!other.started());
        }
        assert!(SpawnResult::Success.resolves_tonight());
        assert!(SpawnResult::TooSmall.resolves_tonight());
        assert!(!SpawnResult::CannotRaid.resolves_tonight());
        assert!(!SpawnResult::NoSpawnPoint.resolves_tonight());
        assert!(!SpawnResult::Error.resolves_tonight());
    }

    #[test]
    fn the_thresholds_are_the_java_ones() {
        assert_eq!(SPAWN_MODIFIER, 60.0);
        assert_eq!(MIN_BUILDING_SPAWN_DIST, 35);
        assert_eq!(MIN_RAID_DIFFICULTY, 1);
        assert_eq!(MAX_RAID_DIFFICULTY, 14);
        assert_eq!(MIN_REQUIRED_RAID_LEVEL, 75);
        assert_eq!(INITIAL_RAID_DIFFICULTY, 7);
        assert_eq!(PASS_THROUGH_TICKS, 400);
        assert_eq!(BIG_HORDE_SIZE, 20);
        assert_eq!(SMALL_HORDE_SIZE, 5);
        assert_eq!(DEFAULT_BARBARIAN_DIFFICULTY, 5.0);
        assert_eq!(MERCY_SHARE, 0.5);
        // The ladder's six, and the two it produces but cannot be asked for.
        assert_eq!(REQUESTABLE_RAID_TYPES.len(), 6);
        assert!(known_raid_type("barbarian_raid"));
        assert!(known_raid_type("drowned_pirate_raid"));
        assert!(!known_raid_type("pirate_ground_raid"));
        assert!(!known_raid_type("norsemen_ship_raid"));
        assert!(!known_raid_type(""));
    }

    #[test]
    fn the_raider_count_truncates_before_the_clamp_and_adds_one_after() {
        // level 120, one player, modifier 1.0, roll 0.5, no cap:
        // (120/60) * 1 * 1.05 * (0.15+0.85) = 2.1 → 2, +1 = 3.
        assert_eq!(raider_amount(120, 1, 1.0, 0.5, 1000), 3);
        // The cap applies to the truncated count, so the answer is cap + 1.
        assert_eq!(raider_amount(120, 1, 1.0, 0.5, 2), 3);
        assert_eq!(raider_amount(120, 1, 1.0, 0.5, 1), 2);
        // Roll 0 is the floor of the 0.85..1.15 band: (2.0 * 1.05 * 0.85) = 1.785 → 1.
        assert_eq!(raider_amount(120, 1, 1.0, 0.0, 1000), 2);
        // Level 0 still answers 1, never 0.
        assert_eq!(raider_amount(0, 0, 1.0, 0.5, 1000), 1);
    }

    #[test]
    fn the_difficulty_modifier_is_a_product_of_four_factors() {
        let mut manager = Manager::new();
        // (7/10 + 0.2) * (5/5) * (2/2) * 1.0 = 0.9
        assert!((manager.difficulty_modifier(5.0, 2.0) - 0.9).abs() < 1e-9);
        // Hard (3) instead of normal (2): times 1.5.
        assert!((manager.difficulty_modifier(5.0, 3.0) - 1.35).abs() < 1e-9);
        // Half the config difficulty.
        assert!((manager.difficulty_modifier(2.5, 2.0) - 0.45).abs() < 1e-9);
        // The size adjustment multiplies last.
        manager.spawn_count_adjusted = 2.0;
        assert!((manager.difficulty_modifier(5.0, 2.0) - 1.8).abs() < 1e-9);
    }

    #[test]
    fn the_raid_level_adds_five_per_adult_and_scales_by_the_population() {
        let residents = [Resident::adult(250), Resident::adult(50), Resident::child()];
        let buildings = [
            Building::new(BuildingKind::Other, 3, [0, 64, 0]),
            Building::new(BuildingKind::TownHall, 0, [1, 64, 1]),
        ];
        // adults: (5 + 2) + (5 + 0) = 12; child: 0;
        // building: 5 + 9/5 = 6; level 0 building: 0; research 2 * 3 = 6 → 24.
        assert_eq!(colony_raid_level(&residents, &buildings, 2, 2, 4), 12);
        // Full population: no scaling.
        assert_eq!(colony_raid_level(&residents, &buildings, 2, 4, 4), 24);
        // Over the cap: still capped at 1, not above.
        assert_eq!(colony_raid_level(&residents, &buildings, 2, 9, 4), 24);
        // Only child + buildings level 3: 0 + 6 + 2 research * 3 = 12, at a
        // quarter population → 3.
        assert_eq!(
            colony_raid_level(&[Resident::child()], &buildings, 2, 1, 4),
            3
        );
    }

    #[test]
    fn a_colony_with_no_citizen_cap_reports_zero_instead_of_dividing_by_it() {
        let residents = [Resident::adult(0)];
        assert_eq!(colony_raid_level(&residents, &[], 0, 0, 0), 0);
    }

    #[test]
    fn the_spawn_distance_is_manhattan_and_grows_with_the_building() {
        // A point 20 east and 20 north: Manhattan 40, Euclidean 28.3.
        let at = [20, 64, 20];
        let plain = [Building::new(BuildingKind::Other, 1, [0, 64, 0])];
        // 35 + 2 = 37 ≤ 40: allowed, where Euclid would have refused it.
        assert!(spawn_is_allowed(&plain, at));
        // One level more: 35 + 4 = 39 ≤ 40, still allowed.
        assert!(spawn_is_allowed(&[Building::new(BuildingKind::Other, 2, [0, 64, 0])], at));
        // 35 + 6 = 41 > 40: refused.
        assert!(!spawn_is_allowed(&[Building::new(BuildingKind::Other, 3, [0, 64, 0])], at));

        // The four multipliers, at level 2, on the same point.
        let tower = min_spawn_distance(BuildingKind::GuardTower, 2);
        let living = min_spawn_distance(BuildingKind::Living, 2);
        let hall = min_spawn_distance(BuildingKind::TownHall, 2);
        let other = min_spawn_distance(BuildingKind::Other, 2);
        assert_eq!((tower, living, hall, other), (49, 43, 51, 39));

        // A level-0 building is skipped entirely (`:650-653`).
        let unbuilt = [Building::new(BuildingKind::TownHall, 0, [20, 64, 20])];
        assert!(spawn_is_allowed(&unbuilt, at));
    }

    #[test]
    fn no_write_happens_before_a_refusal() {
        let points = two_points();
        let settings = Settings::default();

        // 1. no world (`:256`).
        let mut manager = Manager::new();
        let mut nowhere = env(&points);
        nowhere.world_present = false;
        assert_eq!(
            manager.raider_event(&settings, &nowhere, 1_000),
            Err(SpawnResult::Error)
        );
        assert!(manager.history().is_empty());
        assert_eq!(manager.nights_since_last_raid(), INITIAL_NIGHTS_SINCE_LAST_RAID);
        assert!(!manager.is_dirty());

        // 2. not allowed to raid (`:260`), only when the request is not forced.
        let mut manager = Manager::new();
        let mut forbidden = env(&points);
        forbidden.can_raid = false;
        assert_eq!(
            manager.raider_event(&settings, &forbidden, 1_000),
            Err(SpawnResult::CannotRaid)
        );
        assert!(manager.history().is_empty());
        assert_eq!(manager.spawn_count_adjusted(), INITIAL_SPAWN_COUNT_ADJUSTED);
        assert!(!manager.is_dirty());

        // 3. too small (`:275`): below the level floor, nothing is written —
        //    including the size adjustment, which comes after this check.
        let mut manager = Manager::new();
        manager.spawn_count_adjusted = 0.25;
        let mut small = env(&points);
        small.raid_level = MIN_REQUIRED_RAID_LEVEL - 1;
        assert_eq!(
            manager.raider_event(&settings, &small, 1_000),
            Err(SpawnResult::TooSmall)
        );
        assert_eq!(manager.spawn_count_adjusted(), 0.25);
        assert!(manager.history().is_empty());

        // 4. no spawn point (`:317`), after the size was already written.
        let mut manager = Manager::new();
        assert_eq!(
            manager.raider_event(&settings, &env(&[]), 1_000),
            Err(SpawnResult::NoSpawnPoint)
        );
        assert_eq!(manager.spawn_count_adjusted(), 1.0);
        assert!(manager.history().is_empty());
        assert!(!manager.is_dirty());
    }

    #[test]
    fn a_successful_raid_splits_rounding_up_and_records_before_it_starts() {
        let points = two_points();
        let mut manager = Manager::new();
        manager.set_nights_since_last_raid(4);
        manager.set_raid_next_night(Some(Settings::default()));

        let spawn = manager
            .raider_event(&Settings::default(), &env(&points), 5_000)
            .expect("the world allows it");

        // level 120, one player, modifier 0.9, roll 0.5:
        // (2 * 0.9 * 1.05 * 1.0) = 1.89 → 1, + 1 = 2 raiders over 2 points.
        assert_eq!(spawn.per_point, 1);
        assert_eq!(spawn.points.len(), 2);
        // `7 / 10.0 + 0.2` is 0.8999999999999999 in binary, so Java's
        // `((int) (x * 100)) / 100.0` (`:456`) truncates 89.99… to **0.89** —
        // two decimals, not rounded, and not the 0.9 it looks like.
        assert!((spawn.difficulty - 0.89).abs() < 1e-9);

        // The history holds the *total*, before the split (`:322` then `:325`).
        let history = manager.last_raid().expect("a raid was recorded");
        assert_eq!(history.raider_amount, 2);
        assert_eq!(history.raid_time, 5_000);
        assert_eq!(history.spawns.len(), 2);
        assert_eq!(manager.nights_since_last_raid(), 0);
        assert!(manager.next_settings().is_none());
        assert!(manager.is_dirty());
    }

    #[test]
    fn an_unrequestable_type_refuses_after_the_writes_because_java_does() {
        let points = two_points();
        let mut manager = Manager::new();
        let settings = Settings {
            raid_type: Some("pirate_ground_raid".to_owned()),
            ..Settings::default()
        };
        // `:442-445` is reached with the history already pushed and the nights
        // already zeroed: the port keeps that, and says so.
        assert_eq!(
            manager.raider_event(&settings, &env(&points), 700),
            Err(SpawnResult::NoSpawnPoint)
        );
        assert_eq!(manager.history().len(), 1);
        assert_eq!(manager.nights_since_last_raid(), 0);
        // And the colony was **not** marked dirty: `:444` returns before
        // `:458`, so the entry above is in memory only.
        assert!(!manager.is_dirty());
    }

    #[test]
    fn a_forced_amount_skips_the_level_floor_but_not_the_spawn_points() {
        let points = two_points();
        let settings = Settings {
            forced: true,
            raider_amount: Some(41),
            ..Settings::default()
        };
        let mut small = env(&points);
        small.raid_level = 0;
        small.can_raid = false;

        let mut manager = Manager::new();
        let spawn = manager
            .raider_event(&settings, &small, 900)
            .expect("forced spawns ignore the floor and the permission");
        // 41 over 2 points: 20.5 → 21.
        assert_eq!(spawn.per_point, 21);
        assert_eq!(manager.last_raid().map(|raid| raid.raider_amount), Some(41));

        // Forced does not invent a spawn point.
        let mut manager = Manager::new();
        assert_eq!(
            manager.raider_event(&settings, &env(&[]), 900),
            Err(SpawnResult::NoSpawnPoint)
        );
    }

    #[test]
    fn the_nights_move_the_difficulty_both_ways_and_reset_on_a_raid() {
        let events = fresh_colony();

        // A raid that cost 2 of 10 citizens: 20 % > 15 %, so the difficulty
        // drops by (0.2 / 0.15) = 1, and the next raid waits 40 % longer.
        let mut manager = Manager::new();
        manager.history.push(History::new(4, 100));
        if let Some(last) = manager.history.last_mut() {
            last.lost_citizens = 2;
        }
        let steps = manager.night_fall(200, &events, 10, 2, 5, 0.9, true);
        assert_eq!(steps[0], NightStep::Difficulty(6));
        assert_eq!(steps[1], NightStep::Delay(2)); // ceil(5 * 0.4) = 2
        assert_eq!(manager.extra_days(), 2);
        assert_eq!(steps[2], NightStep::Nights(1));

        // Nobody lost: the difficulty goes up by one, and only once per raid.
        let mut manager = Manager::new();
        manager.history.push(History::new(4, 100));
        let steps = manager.night_fall(200, &events, 10, 2, 5, 0.9, true);
        assert_eq!(steps[0], NightStep::Difficulty(8));
        // The second night does not bend it again: the counter is no longer 0.
        let steps = manager.night_fall(300, &events, 10, 2, 5, 0.9, true);
        assert_eq!(steps, vec![NightStep::Nights(2), NightStep::Scheduled(false)]);

        // Floor and ceiling hold.
        let mut manager = Manager::new();
        manager.difficulty = MIN_RAID_DIFFICULTY;
        manager.history.push(History::new(1, 0));
        manager.history[0].lost_citizens = 10;
        assert_eq!(
            manager.night_fall(100, &events, 10, 2, 5, 0.9, true)[0],
            NightStep::Difficulty(1)
        );
        let mut manager = Manager::new();
        manager.difficulty = MAX_RAID_DIFFICULTY;
        manager.history.push(History::new(1, 0));
        assert_eq!(
            manager.night_fall(100, &events, 10, 2, 5, 0.9, true)[0],
            NightStep::Difficulty(14)
        );
    }

    #[test]
    fn a_raid_in_progress_zeroes_the_counter_instead_of_counting() {
        let mut events = fresh_colony();
        let id = events.take_id();
        events.add(Raid::new(id, "minecolonies:barbarian_raid")).unwrap();
        events.set_status(id, Status::Progressing).unwrap();

        let mut manager = Manager::new();
        manager.set_nights_since_last_raid(3);
        let steps = manager.night_fall(100, &events, 10, 2, 5, 0.9, true);
        assert_eq!(
            steps,
            vec![NightStep::Nights(0), NightStep::Scheduled(false)]
        );
        assert_eq!(manager.nights_since_last_raid(), 0);
    }

    #[test]
    fn a_due_raid_is_reported_not_spawned_and_only_a_success_clears_it() {
        let events = fresh_colony();
        let mut manager = Manager::new();
        manager.set_raid_next_night(Some(Settings {
            forced: true,
            ..Settings::default()
        }));

        let steps = manager.night_fall(50, &events, 10, 2, 5, 0.0, true);
        assert!(steps.contains(&NightStep::RaidDue));
        // Nothing was spawned: the world's call belongs to the caller.
        assert!(manager.history().is_empty());
        assert!(manager.will_raid_tonight());

        // A refusal that is not "too small" keeps it for tomorrow.
        manager.raid_resolved(SpawnResult::NoSpawnPoint);
        assert!(manager.will_raid_tonight());

        // Success and "too small" both clear it (`:770-774`).
        manager.raid_resolved(SpawnResult::TooSmall);
        assert!(!manager.will_raid_tonight());
        assert_eq!(manager.extra_days(), 0);
    }

    #[test]
    fn raid_this_night_obeys_the_minimum_the_average_and_one_chance() {
        let mut manager = Manager::new();
        manager.set_nights_since_last_raid(1);
        // Below the minimum: never.
        assert!(!manager.raid_this_night(2, 5, 0.99));
        // Past the average by two: always, whatever the roll.
        manager.set_nights_since_last_raid(8);
        assert!(manager.raid_this_night(2, 5, 0.99));
        // In between: one chance in (5 - 2).
        manager.set_nights_since_last_raid(3);
        assert!(manager.raid_this_night(2, 5, 0.3));
        assert!(!manager.raid_this_night(2, 5, 0.34));
        // The delay pushes the minimum out.
        manager.extra_days = 2;
        manager.set_nights_since_last_raid(3);
        assert!(!manager.raid_this_night(2, 5, 0.0));
    }

    #[test]
    fn a_guard_costs_one_and_anyone_else_two_and_half_the_colony_ends_the_wave() {
        let mut manager = Manager::new();
        // No history: the death is not this manager's business.
        assert_eq!(manager.lost_citizen(true, true, 10), None);
        // Not raided: ignored.
        manager.history.push(History::new(5, 0));
        assert_eq!(manager.lost_citizen(true, false, 10), None);

        assert_eq!(manager.lost_citizen(true, true, 10), Some(false));
        assert_eq!(manager.lost_citizens(), 1);
        assert_eq!(manager.lost_citizen(false, true, 10), Some(false));
        assert_eq!(manager.lost_citizens(), 3);
        // 3/10 is not past half; 6/10 is.
        assert_eq!(manager.lost_citizen(false, true, 10), Some(false));
        assert_eq!(manager.lost_citizen(false, true, 10), Some(true));
    }

    #[test]
    fn the_mercy_ending_reads_the_oldest_record_and_the_running_check_comes_first() {
        // The running check is before the history dereference (`:1050-1056`),
        // so a still-running raid on a colony with no history is not a refusal.
        let mut manager = Manager::new();
        let finished = manager.raid_finished(true, 5, 10).expect("nothing to read");
        assert!(finished.still_running);
        assert!(!finished.mercy);

        // With no raid running and no history, Java throws; the port refuses.
        assert_eq!(
            manager.raid_finished(false, 5, 10),
            Err(Refusal::NoHistory)
        );

        // Oldest lost 6 of 10, the newest lost none: the mercy verdict comes
        // from the oldest (`:1058`), the blemish check from the newest (`:1078`).
        manager.history.push(History::new(3, 0));
        manager.history.push(History::new(40, 100));
        manager.history[0].lost_citizens = 6;
        let finished = manager.raid_finished(false, 5, 10).expect("both records");
        assert!(finished.mercy);
        assert!(finished.unblemished);
        // The victory sound is chosen by the oldest horde's size (`:1075`).
        assert_eq!(finished.raiders, 3);
        // Mercy pushes the next raid by twice the average (`:1061`).
        assert_eq!(manager.extra_days(), 10);
    }

    #[test]
    fn a_pass_through_marks_the_colony_raided_for_four_hundred_ticks() {
        let events = fresh_colony();
        let mut manager = Manager::new();
        assert!(!manager.is_raided(100, &events));
        // A colony at game time 0 already counts as raided, because
        // `0 <= passingThroughRaidTime` with the field at its initial 0
        // (`:723`): the first night of a young colony never counts.
        assert!(manager.is_raided(0, &events));
        manager.set_pass_through(100);
        assert!(manager.is_raided(100, &events));
        assert!(manager.is_raided(499, &events));
        assert!(!manager.is_raided(501, &events));
        // And the night counter stops counting while it is set, because
        // `passingThroughRaidTime > 0` stays true (`:740`).
        manager.set_nights_since_last_raid(0);
        let steps = manager.night_fall(10_000, &events, 10, 2, 5, 0.0, true);
        assert_eq!(steps[0], NightStep::Nights(1));
    }

    #[test]
    fn the_save_round_trips_clamps_and_refuses_a_spawn_without_a_type() {
        let mut manager = Manager::new();
        manager.history.push(History::new(7, 2_000));
        manager.history[0].lost_citizens = 3;
        manager.history[0].dead_raiders = 2;
        manager.history[0].difficulty = 0.9;
        manager.history[0].spawns = two_points();
        manager.set_nights_since_last_raid(3);
        manager.extra_days = 2;
        manager.set_can_have_raider_events(false);
        manager.difficulty = 9;

        let compound = manager.write();
        {
            // Java's own keys, flat (`:976-989`).
            assert!(matches!(compound.get(&"raidable".into()), Some(Tag::Byte(0))));
            assert!(matches!(
                compound.get(&"nightsRaid".into()),
                Some(Tag::Int(3))
            ));
            assert!(matches!(compound.get(&"delay".into()), Some(Tag::Int(2))));
            assert!(matches!(
                compound.get(&"difficulty".into()),
                Some(Tag::Int(9))
            ));
        }

        let mut read = Manager::new();
        let repairs = read.read(&compound).expect("a good save reads");
        assert!(repairs.is_empty(), "{repairs:?}");
        // `RaidHistory.write` (`:1165-1179`) saves four numbers and the spawns:
        // **`deadRaiders` is not one of them**, so a restart forgets the kills of
        // the last raid and only that. The rest comes back whole.
        assert_eq!(read.history[0].dead_raiders, 0);
        manager.history[0].dead_raiders = 0;
        assert_eq!(read, manager);

        // Missing difficulty: Java reads 0 and clamps to 1, not to the initial 7.
        let bare = Compound::from([("nightsRaid".into(), Tag::Int(1))]);
        let mut read = Manager::new();
        let repairs = read.read(&bare).expect("no spawn record, no refusal");
        assert!(repairs.contains(&Repair::DifficultyClamped { to: 1 }));
        assert!(repairs.contains(&Repair::NoRaidable));
        assert!(repairs.contains(&Repair::NoHistoryList));
        assert_eq!(read.difficulty(), MIN_RAID_DIFFICULTY);
        assert!(read.can_have_raider_events());

        // A spawn record without `raidtype`: Java throws inside `Colony.read`.
        let broken = Compound::from([(
            "raidhistory".into(),
            Tag::List(vec![Tag::Compound(Compound::from([(
                "spawnInfo".into(),
                Tag::List(vec![Tag::Compound(Compound::from([("x".into(), Tag::Int(1))]))]),
            )]))]),
        )]);
        let mut read = Manager::new();
        assert_eq!(
            read.read(&broken),
            Err(Refusal::SpawnWithoutType { record: 0, spawn: 0 })
        );
    }
}
