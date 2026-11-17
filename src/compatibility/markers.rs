//! Where a colony's land, its loaded chunks and its graves are, and who may see
//! them: the four things the JourneyMap integration publishes, as values.
//!
//! Java reference: `core/compatibility/journeymap/` (13 files, 2 253 lines). The
//! two versions publish **the same model** and differ only in where the
//! entity-radar rule lives — `EventListener.onUpdateEntityRadar` in `v5`, moved to
//! `JourneymapPlugin.onEntityRadarUpdateEvent` in `v6` — and in the API class
//! names (`journeymap.client.api.*` against `journeymap.api.v2.*`). This module
//! models that one model; the `v5` line numbers are noted where they differ.
//!
//! # The published list, measured
//!
//! | What | Where | Kept as |
//! | --- | --- | --- |
//! | the claim's outline, drawn as polygons on the fullscreen map and minimap | `ColonyBorderMapping.java:394-432` | [`BorderOverlay::chunks`] |
//! | the chunks the colony keeps loaded, drawn inside it | `:434-461` | [`BorderOverlay::loaded`] |
//! | a waypoint per grave, named after the citizen | `ColonyDeathpoints.java:176-184` | [`Deathpoint`] |
//! | a name, job, colour or a hide on each colonist's radar dot | `JourneymapPlugin.java:85-160` | [`radar`] |
//! | the colony the player stands in, as an info slot | `JourneymapPlugin.java:82` | [`INFO_SLOT_PERIOD`] |
//! | 13 options under JourneyMap's Options screen | `JourneymapOptions.java:35-47` | [`MarkerOptions`] |
//!
//! **The claim outline is not the colony's square, and the difference is the
//! point.** Java asks each chunk who owns it — `ColonyUtils.getOwningColony(chunk)`
//! (`ColonyBorderMapping.java:155`) — so an outline follows the *claim record* and
//! can be any shape, including holes; a load of 100 000 chunks is the normal case
//! (`PolygonHelper.createChunksPolygon(this.chunks, 256)`, `:398`). The same
//! questions about a colony's land are answered in this port by
//! `crate::base::claims::ChunkClaims` and `crate::base::territory`; what this
//! module adds is **the set the map remembers**, which survives a disconnect
//! (`border.json`, `:76-104`) and is not a recomputation.
//!
//! # `border.json`, field by field
//!
//! The file is per dimension, under the mod's own data folder
//! (`jmap.getDataPath(MOD_ID).resolve(<dimension path>)`, `Journeymap.java:98-102`),
//! and holds a list of [`BorderOverlay`] records (`ColonyBorderMapping.java:225-234`):
//!
//! | Field | Type | Default | Meaning |
//! | --- | --- | --- | --- |
//! | `dimension` | dimension key | — | `minecraft:overworld` |
//! | `id` | int | — | the colony id |
//! | `colony_name` | string? | absent | the name the map labels the outline with |
//! | `colour` | int | `-1` | the team colour |
//! | `licet` | bool | `true` | *may the player see it* — the MAP_BORDER bit at save time |
//! | `chunks` | list of longs | empty | the claimed chunks, `ChunkPos.toLong` |
//! | `loaded` | list of longs | empty | the chunks the colony keeps loaded |
//!
//! `licet` is the odd one: the field is a misspelling of "licet" for *licet*
//! ("it is allowed", Latin) and it stores a **permission answer**, not a
//! permission. The port keeps the name — a save written by a Java server has to be
//! readable — and [`BorderOverlay::permitted`] says what it holds.
//!
//! **What this module does not write.** The crate has no JSON codec (`Cargo.toml`:
//! one dependency, the SDK), so `border.json` is *described* here and not
//! serialised; Horizon's `Host::data_files()` is the root it would be written to
//! (`crates/horizon-mod-vocab/src/host/config.rs:57`). `MANQUANT.md` §3 says it.

use std::collections::{BTreeMap, BTreeSet};

use crate::base::coords::ChunkPos;
use crate::colony::{Action, Colony};
use crate::compatibility::Documented;

/// `PARTIAL_JOURNEY_MAP_INFO`, `TranslationConstants.java:748`.
pub const PARTIAL_JOURNEY_MAP_INFO: &str =
    "com.minecolonies.coremod.journeymap.";

/// How a border is drawn, `JourneymapOptions.BorderStyle` (`:115-133`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum BorderStyle {
    /// `HIDDEN`: no overlay at all.
    Hidden,
    /// `FRAMED`: the outline only.
    Framed,
    /// `FILLED`: the outline and a translucent fill.
    Filled,
}

impl BorderStyle {
    /// `!BorderStyle.HIDDEN.equals(style)` — the three tests that decide whether an
    /// overlay is created at all (`ColonyBorderMapping.java:404`, `:419`, `:444`).
    pub fn is_drawn(self) -> bool {
        self != BorderStyle::Hidden
    }
}

/// What the fullscreen map draws by default,
/// `borderFullscreenStyle = FILLED` (`JourneymapOptions.java:35`) with the
/// `orElse(FILLED)` of `:52`.
pub const BORDER_FULLSCREEN_DEFAULT: Documented<BorderStyle> =
    Documented::new(BorderStyle::Filled, "JourneymapOptions.java:35, :52");

/// What the minimap draws by default,
/// `borderMinimapStyle = FRAMED` (`:36`, `:57`).
pub const BORDER_MINIMAP_DEFAULT: Documented<BorderStyle> =
    Documented::new(BorderStyle::Framed, "JourneymapOptions.java:36, :57");

/// What the loaded chunk band draws by default,
/// `borderLoadedStyle = HIDDEN` (`:37`, `:62`) — off until the player asks.
pub const BORDER_LOADED_DEFAULT: Documented<BorderStyle> =
    Documented::new(BorderStyle::Hidden, "JourneymapOptions.java:37, :62");

/// Deathpoints are on by default (`:38`, `:67`).
pub const DEATHPOINTS_DEFAULT: Documented<bool> =
    Documented::new(true, "JourneymapOptions.java:38, :67");

/// Raiders are drawn in their hostile colour by default (`:47`, `:112`).
pub const RAIDER_DEFAULT: Documented<RaiderColor> =
    Documented::new(RaiderColor::Hostile, "JourneymapOptions.java:47, :112");

/// How often the map asks for the colony the player stands in: 2 500 ms
/// (`JourneymapPlugin.java:82`, `event.register(MOD_ID, …, 2500, …)`).
pub const INFO_SLOT_PERIOD: Documented<i32> = Documented::new(2500, "JourneymapPlugin.java:82");

/// The info slot's key (`JourneymapPlugin.java:82`) and the translation key the
/// map's HUD shows for it.
pub const INFO_SLOT_KEY: &str = "com.minecolonies.coremod.journeymap.currentcolony";

/// The colour a raider dot takes, `JourneymapOptions.RaiderColor` (`:135-164`).
///
/// Two of the six are a raw RGB the Java writes down; the other four are a vanilla
/// `ChatFormatting` entry, so this port keeps the *formatting name* rather than the
/// hex — the enum moved between versions and the game owns the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum RaiderColor {
    /// `HOSTILE`: leave the dot as JourneyMap draws it (`:137`).
    Hostile,
    /// `NONE`: hide every raider (`:138`).
    None,
    Yellow,
    Red,
    Purple,
    Orange,
}

/// A colour this module keeps: a literal, or the name of the vanilla formatting
/// entry that holds it. Never a hex invented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Color {
    /// `TextColor.fromRgb(value)` — the two literals the Java writes
    /// (`JourneymapOptions.java:137-138`).
    Rgb(u32),
    /// `TextColor.fromLegacyFormat(ChatFormatting.X)` (`:139-142`): the value is
    /// vanilla's table, read from the game.
    Legacy(&'static str),
}

impl RaiderColor {
    /// The colour the radar dot takes, `color.getColor().getValue()`
    /// (`JourneymapPlugin.java:157`).
    pub fn color(self) -> Color {
        match self {
            Self::Hostile => Color::Rgb(0xFFFF_FFFF),
            Self::None => Color::Rgb(0xFF00_0000),
            Self::Yellow => Color::Legacy("yellow"),
            Self::Red => Color::Legacy("red"),
            // `ChatFormatting.LIGHT_PURPLE`, which the option calls PURPLE
            // (`JourneymapOptions.java:141`).
            Self::Purple => Color::Legacy("light_purple"),
            // `ChatFormatting.GOLD`, which the option calls ORANGE (`:142`).
            Self::Orange => Color::Legacy("gold"),
        }
    }

    /// Whether the radar leaves the dot alone: only `HOSTILE` does
    /// (`JourneymapPlugin.java:151-158` — `NONE` hides, anything else re-colours).
    pub fn is_hostile(self) -> bool {
        self == Self::Hostile
    }
}

/// The 13 options the integration declares in JourneyMap's own Options screen,
/// `JourneymapOptions.java:35-47`, with the order they are sorted in.
///
/// Kept as a table because the *list* is what the integration publishes: a shape
/// that has to be re-declared (as mod settings, `MANQUANT.md` §5) needs its
/// members written down once.
pub const SORT_ORDER: [(&str, i32); 13] = [
    ("borderFullscreenStyle", 100),
    ("borderMinimapStyle", 101),
    ("borderLoadedStyle", 102),
    ("deathpoints", 150),
    ("colonyname", 180),
    ("colonistNameFullscreen", 200),
    ("colonistNameMinimap", 201),
    ("colonistTooltips", 202),
    ("colonistTeam", 203),
    ("guards", 300),
    ("citizens", 301),
    ("visitors", 302),
    ("raiders", 303),
];

/// The options, with the defaults `JourneymapOptions`' constructor sets
/// (`:35-47`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct MarkerOptions {
    pub border_fullscreen: BorderStyle,
    pub border_minimap: BorderStyle,
    pub border_loaded: BorderStyle,
    pub deathpoints: bool,
    pub colony_name: bool,
    pub colonist_name_minimap: bool,
    pub colonist_name_fullscreen: bool,
    pub colonist_tooltips: bool,
    pub colonist_team: bool,
    pub guards: bool,
    pub citizens: bool,
    pub visitors: bool,
    pub raiders: RaiderColor,
}

impl Default for MarkerOptions {
    fn default() -> Self {
        MarkerOptions {
            border_fullscreen: BORDER_FULLSCREEN_DEFAULT.value,
            border_minimap: BORDER_MINIMAP_DEFAULT.value,
            border_loaded: BORDER_LOADED_DEFAULT.value,
            deathpoints: DEATHPOINTS_DEFAULT.value,
            colony_name: true,
            colonist_name_minimap: true,
            colonist_name_fullscreen: true,
            colonist_tooltips: true,
            colonist_team: true,
            guards: true,
            citizens: true,
            visitors: true,
            raiders: RAIDER_DEFAULT.value,
        }
    }
}

impl MarkerOptions {
    /// Whether *any* border is drawn. The three styles are the only ones that
    /// matter here (`ColonyBorderMapping.java:375-378`).
    pub fn any_border(&self) -> bool {
        self.border_fullscreen.is_drawn()
            || self.border_minimap.is_drawn()
            || self.border_loaded.is_drawn()
    }
}

/// A set of chunks as Java's `Set<ChunkPos>` behaves for the two overlay updates:
/// `addAll` and `removeAll` each answer whether the set changed
/// (`ColonyBorderMapping.java:289-308`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct ChunkSet {
    chunks: BTreeSet<ChunkPos>,
}

impl ChunkSet {
    pub fn new() -> Self {
        ChunkSet::default()
    }

    /// `this.chunks.addAll(add)`: true when at least one chunk was new.
    pub fn add_all(&mut self, add: impl IntoIterator<Item = ChunkPos>) -> bool {
        let mut changed = false;
        for chunk in add {
            changed |= self.chunks.insert(chunk);
        }
        changed
    }

    /// `this.chunks.removeAll(remove)`: true when at least one chunk left.
    pub fn remove_all(&mut self, remove: impl IntoIterator<Item = ChunkPos>) -> bool {
        let mut changed = false;
        for chunk in remove {
            changed |= self.chunks.remove(&chunk);
        }
        changed
    }

    pub fn contains(&self, chunk: ChunkPos) -> bool {
        self.chunks.contains(&chunk)
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks.iter().copied()
    }

    /// The chunks as `border.json` stores them: `ChunkPos.toLong`
    /// (`ColonyBorderMapping.java:222-224`), sorted.
    ///
    /// Java wraps the list in a `HashSet` on both sides of the codec
    /// (`:222-224`, `:232-233`), so the file's order is whatever the hash table
    /// yields. The port sorts: a file written twice with the same land is the same
    /// file, which is worth the one line.
    pub fn packed(&self) -> Vec<i64> {
        self.chunks.iter().map(|chunk| chunk.pack()).collect()
    }

    /// Reads the chunks back — [`ChunkPos::unpack`] on each long.
    ///
    /// Total, like Java's codec: every `i64` is a valid pair of chunk coordinates,
    /// so there is no refusal to name here.
    pub fn from_packed(packed: impl IntoIterator<Item = i64>) -> ChunkSet {
        ChunkSet {
            chunks: packed.into_iter().map(ChunkPos::unpack).collect(),
        }
    }
}

/// The colour a fresh overlay starts with: `0x00ff00`, set on the fill, the stroke
/// and the label by `ColonyBorderOverlay`'s constructor
/// (`ColonyBorderMapping.java:262-283`).
///
/// It is **not** the `border.json` default: the record's `colour` field is
/// `optionalFieldOf("colour", -1)` (`:230`), so a *loaded* record whose file has no
/// colour starts at `-1`, and one real colour arrives on the first `updateInfo`.
/// This type models a live overlay (it cannot parse JSON — `MANQUANT.md` §6), so it
/// starts where Java's constructor does.
pub const INITIAL_COLOUR: Documented<i32> =
    Documented::new(0x00FF_00, "ColonyBorderMapping.java:262-283");

/// One colony's remembered outline, `ColonyBorderOverlay`
/// (`ColonyBorderMapping.java:200-474`) — the record `border.json` saves.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct BorderOverlay {
    /// The dimension key, `minecraft:overworld`.
    pub dimension: String,
    /// The colony id.
    pub id: i32,
    /// The name the outline is labelled with, `""` until the colony is known
    /// (`:217`).
    pub colony_name: String,
    /// The team colour, [`INITIAL_COLOUR`] until the colony answers (`:278`).
    /// A `border.json` read that has no `colour` field starts at `-1` instead
    /// (`:230`) — see [`INITIAL_COLOUR`].
    pub colour: i32,
    /// `licet`: the MAP_BORDER answer at the time the record was made.
    pub permitted: bool,
    /// The claimed chunks.
    pub chunks: ChunkSet,
    /// The chunks the colony keeps loaded.
    pub loaded: ChunkSet,
}

impl BorderOverlay {
    /// The overlay group name Java builds: `String.format("colony_%s_%d", …)`
    /// (`ColonyBorderMapping.java:258`).
    ///
    /// The `%s` is `dimension.location()`, a `ResourceLocation`, whose `toString()`
    /// is the **whole key** — `colony_minecraft:overworld_7`, colon included, not
    /// `colony_overworld_7`. [`BorderOverlay::dimension`] therefore holds the key
    /// and not its path.
    pub fn group_name(&self) -> String {
        format!("colony_{}_{}", self.dimension, self.id)
    }

    /// A newly-tracked colony, empty, `permitted` as Java's initial `true`
    /// (`:216`) and coloured [`INITIAL_COLOUR`] (`:278`).
    pub fn new(dimension: &str, id: i32) -> Self {
        BorderOverlay {
            dimension: dimension.to_owned(),
            id,
            colony_name: String::new(),
            colour: INITIAL_COLOUR.value,
            permitted: true,
            chunks: ChunkSet::new(),
            loaded: ChunkSet::new(),
        }
    }

    /// `updateChunks`: an owned chunk arriving, or a disowned one leaving
    /// (`:289-297`).
    pub fn update_chunks(&mut self, add: &ChunkSet, remove: &ChunkSet) -> bool {
        let added = self.chunks.add_all(add.iter());
        let removed = self.chunks.remove_all(remove.iter());
        added || removed
    }

    /// `updateLoadedChunks` (`:300-308`).
    pub fn update_loaded_chunks(&mut self, add: &ChunkSet, remove: &ChunkSet) -> bool {
        let added = self.loaded.add_all(add.iter());
        let removed = self.loaded.remove_all(remove.iter());
        added || removed
    }

    /// `updateInfo`, the half that is data (`:324-353`): true when the name, the
    /// colour or the permission changed.
    ///
    /// The comparison is against the **current** colour (`this.text.getColor()`),
    /// which a fresh overlay starts at `0x00ff00` (`:278`) — so the first call with
    /// a colony whose team colour is exactly that answers `false`, as Java does.
    /// The other half of Java's method rewrites the label of every live polygon
    /// (`:346-350`), which needs the overlays; here a changed flag is the caller's
    /// cue to redraw.
    pub fn update_info(&mut self, colony_name: &str, colour: i32, permitted: bool) -> bool {
        let changed = colony_name != self.colony_name
            || colour != self.colour
            || permitted != self.permitted;
        self.colony_name = colony_name.to_owned();
        self.colour = colour;
        self.permitted = permitted;
        changed
    }

    /// Whether the outline is drawn at all: permitted, and not all three styles
    /// hidden (`:375-378`).
    pub fn enabled(&self, options: &MarkerOptions) -> bool {
        self.permitted && options.any_border()
    }

    /// Whether there is anything to draw: enabled *and* at least one chunk known
    /// (`:381`, `:394`).
    pub fn shows(&self, options: &MarkerOptions) -> bool {
        self.enabled(options) && !self.chunks.is_empty()
    }

    /// The colour the loaded band uses: the team colour **intensified** then
    /// stripped of its alpha, `0xFFFFFF & intensify(colour)` (`:332`).
    pub fn loaded_colour(&self) -> u32 {
        0x00FF_FFFF & intensify(self.colour as u32)
    }
}

/// `ColonyBorderOverlay.intensify` (`:355-367`): push the dominant channel to
/// full and pull the others a quarter of the way to zero, so a muted team colour
/// still reads over the map's own colours.
///
/// The arithmetic is `FastColor.ARGB32.lerp(0.25f, colour, pure)` with
/// `pure` = `(alpha, 255 where the channel is the max, else 0)`; alpha is carried
/// through unchanged because both ends share it. Java's `Mth.lerpInt` truncates,
/// which no test here depends on: every expected value below is a multiple of
/// 0.25.
pub fn intensify(colour: u32) -> u32 {
    let alpha = (colour >> 24) & 0xFF;
    let channels = [(colour >> 16) & 0xFF, (colour >> 8) & 0xFF, colour & 0xFF];
    let max = channels[0].max(channels[1]).max(channels[2]);
    let mut out = alpha << 24;
    for (index, channel) in channels.iter().enumerate() {
        let pure = if *channel == max { 255 } else { 0 };
        let value = *channel as f32 + 0.25 * (pure as f32 - *channel as f32);
        out |= (value as u32) << (16 - 8 * index as u32);
    }
    out
}

/// May `player` see this colony's border? `colony.getPermissions().hasPermission(
/// player, Action.MAP_BORDER)` (`ColonyBorderMapping.java:316`) — answered by
/// `crate::colony::Colony::may`, which already owns the bit table
/// (`src/colony/permissions.rs`), so no second table lives here.
pub fn may_see_border(colony: &Colony, player: u128) -> bool {
    colony.may(player, Action::MapBorder).is_ok()
}

/// May `player` see this colony's deathpoints? `Action.MAP_DEATHS`
/// (`ColonyDeathpoints.java:87`).
pub fn may_see_deaths(colony: &Colony, player: u128) -> bool {
    colony.may(player, Action::MapDeaths).is_ok()
}

/// A grave waypoint, `ColonyDeathpoints.tryCreatingWaypoint` (`:164-189`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Deathpoint {
    /// Where the grave block is.
    pub at: [i32; 3],
    /// The name the waypoint carries.
    pub label: DeathpointLabel,
}

impl Deathpoint {
    /// The icon every deathpoint uses (`:180`), 16 × 16 (`:181`).
    pub const ICON: &'static str = "minecolonies:textures/icons/grave_icon.png";
    pub const ICON_SIZE: (u32, u32) = (16, 16);
    /// `waypoint.setColor(0x888888)` (`:182`).
    pub const COLOUR: u32 = 0x0088_8888;
}

/// The two names a grave waypoint can carry (`ColonyDeathpoints.java:176-178`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum DeathpointLabel {
    /// `deathpoint_name`, citizen name only: the grave has no job recorded.
    Name(String),
    /// `deathpoint_namejob`: the citizen's name and the job they held.
    NameJob { name: String, job: String },
}

impl DeathpointLabel {
    /// The key, prefixed with [`PARTIAL_JOURNEY_MAP_INFO`] as Java builds it
    /// (`:26`, `:177-178`).
    pub fn key(&self) -> &'static str {
        match self {
            Self::Name(_) => "com.minecolonies.coremod.journeymap.deathpoint_name",
            Self::NameJob { .. } => "com.minecolonies.coremod.journeymap.deathpoint_namejob",
        }
    }

    /// The arguments, in the order the key reads them.
    pub fn args(&self) -> Vec<&str> {
        match self {
            Self::Name(name) => vec![name.as_str()],
            Self::NameJob { name, job } => vec![name.as_str(), job.as_str()],
        }
    }
}

/// Whether a known grave's waypoint exists yet.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
enum Point {
    /// The waypoint exists.
    Built(Deathpoint),
    /// The grave is known but its waypoint is not: either the chunk was not loaded
    /// when it was found (`:158-161`), or `unload` set the value to null
    /// (`:63-67`). Java stores the second kind and not the first — `computeIfAbsent`
    /// with a function that answers null stores nothing (`:108`) — so a grave in an
    /// unloaded chunk is retried on every colony update, and this variant is what
    /// that retry looks like.
    Pending,
}

/// What one [`Deathpoints::sync`] did, so a caller can add and remove waypoints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct GraveChange {
    /// Waypoints to remove: the grave is gone, or the player may no longer see it.
    pub removed: Vec<[i32; 3]>,
    /// Every grave whose waypoint is still to be made, **on this call**: Java asks
    /// `computeIfAbsent` for every grave at every update (`:108`) and stores
    /// nothing when the answer is null, so an unbuildable grave is retried each
    /// time rather than reported once.
    pub pending: Vec<[i32; 3]>,
    /// True when every waypoint was dropped because the player is not allowed to
    /// see them (`ColonyDeathpoints.java:87-88`, `:94`).
    pub refused: bool,
}

/// One colony's deathpoints, `ColonyDeathpoints.overlays`' inner map
/// (`:34`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Deathpoints {
    points: BTreeMap<[i32; 3], Point>,
}

impl Deathpoints {
    pub fn new() -> Self {
        Deathpoints::default()
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The waypoint of a grave, when it exists.
    pub fn get(&self, at: [i32; 3]) -> Option<&Deathpoint> {
        match self.points.get(&at) {
            Some(Point::Built(point)) => Some(point),
            Some(Point::Pending) | None => None,
        }
    }

    /// The graves whose waypoint is still to be made.
    pub fn pending(&self) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.points.iter().filter_map(|(at, point)| match point {
            Point::Pending => Some(*at),
            Point::Built(_) => None,
        })
    }

    /// `updateGraves` (`ColonyDeathpoints.java:80-111`): drop every waypoint the
    /// player may not see or whose grave is gone, then mark each new grave pending.
    ///
    /// `permitted` is `MAP_DEATHS` **and** the `deathpoints` option, which Java
    /// evaluates once at `:87-88`.
    pub fn sync(&mut self, graves: &BTreeSet<[i32; 3]>, permitted: bool) -> GraveChange {
        let mut outcome = GraveChange {
            refused: !permitted,
            ..GraveChange::default()
        };
        let doomed: Vec<[i32; 3]> = self
            .points
            .keys()
            .filter(|at| !permitted || !graves.contains(*at))
            .copied()
            .collect();
        for at in doomed {
            if self.points.remove(&at).is_some_and(|point| matches!(point, Point::Built(_))) {
                outcome.removed.push(at);
            }
        }
        if permitted {
            for at in graves {
                if !self.points.contains_key(at) {
                    self.points.insert(*at, Point::Pending);
                }
            }
            // `computeIfAbsent` is called for **every** grave on every update
            // (`:108`), and stores nothing when the function answers null — so a
            // grave whose waypoint could not be built is retried on the next call.
            // Reporting only the newly-known graves would tell the caller once and
            // never again, which is not what Java does.
            outcome.pending = self.pending().collect();
        }
        outcome
    }

    /// `unload`: every waypoint is removed and remembered as pending
    /// (`:56-70`) — the cache stays, so a dimension the player comes back to does
    /// not have to be found again.
    pub fn unload(&mut self) -> Vec<[i32; 3]> {
        let mut removed = Vec::new();
        for (at, point) in self.points.iter_mut() {
            if matches!(point, Point::Built(_)) {
                removed.push(*at);
                *point = Point::Pending;
            }
        }
        removed
    }

    /// `clear`: forget the whole cache, graves and waypoints alike (`:46-49`).
    ///
    /// **It removes no waypoint from the map.** Java's own comment says so
    /// ("this does *not* remove the overlays from jmap"), and its single caller
    /// drops them all with `removeAll(MOD_ID)` on the line after
    /// (`EventListener.java:57-58`). A caller of this port that keeps its own
    /// waypoints has to do the same; returning a list here would pretend this
    /// method did it.
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// A grave's waypoint could be built: `tryCreatingWaypoint` found a grave block
    /// entity with grave data (`:170-186`).
    pub fn built(&mut self, point: Deathpoint) -> bool {
        match self.points.get_mut(&point.at) {
            Some(slot) => {
                *slot = Point::Built(point);
                true
            }
            // A grave nobody asked about: Java only ever calls this from
            // `computeIfAbsent` on a key already in the map (`:108`, `:147`).
            None => false,
        }
    }

    /// `updateChunk` (`:120-151`): the pending graves of one just-loaded chunk,
    /// which is where a grave whose chunk was unloaded gets its waypoint at last.
    ///
    /// A colony that no longer exists is *not* this method's business here: Java
    /// clears the whole map when `getColonyView` answers null (`:128-141`), and that
    /// is a colony-manager question the caller answers before calling [`clear`].
    pub fn pending_in(&self, chunk: ChunkPos) -> Vec<[i32; 3]> {
        self.pending()
            .filter(|at| ChunkPos::of(*at) == chunk)
            .collect()
    }
}

/// What the radar rule asks JourneyMap to do with one dot,
/// `JourneymapPlugin.onEntityRadarUpdateEvent` (`:85-160`; `v5` had it on
/// `EventListener.onUpdateEntityRadar`).
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Radar {
    /// `wrapper.setDisable(true)`: the entity is not drawn at all (`:99`, `:115`,
    /// `:153`).
    pub hide: bool,
    /// `setEntityToolTips([name, job])` (`:124-131`).
    pub tooltips: Option<(String, String)>,
    /// `setCustomName("")`: the dot loses its label (`:137-140`).
    pub blank_name: bool,
    /// `setColor(...)` (`:142-145`, `:157`).
    pub colour: Option<Color>,
    /// The translation key the job line uses (`:103`, `:119-121`).
    pub job_key: String,
}

/// What kind of entity the radar is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Subject<'a> {
    /// A visitor, drawn with the `visitor` key (`:92`, `:103`).
    Visitor,
    /// A colony citizen: whether the job is a guard job (`:111-113`) and its
    /// translation key, `None` for an unemployed citizen (`:107-109`, `:119-121`).
    Citizen {
        job_key: Option<&'a str>,
        guard: bool,
    },
    /// A raider: only the colour rule applies (`:147-159`).
    Raider,
}

/// The radar decision for one entity.
///
/// A visitor is hidden when `visitors` is off (`:97-101`); a guard answers to
/// `guards` and any other citizen to `citizens` (`:111-117`); the tooltip pair is
/// the entity's own name plus the job line (`:124-131`); the name is shown by which
/// map is asking (`:133-140` — minimap and fullscreen have separate options); and a
/// citizen other than a visitor takes its team colour while a raider takes the
/// option's (`:142-145`, `:147-159`).
pub fn radar(
    subject: Subject<'_>,
    name: Option<&str>,
    team_colour: Option<u32>,
    on_minimap: bool,
    options: &MarkerOptions,
) -> Radar {
    let unemployed = format!("{}unemployed", PARTIAL_JOURNEY_MAP_INFO);
    let job_key = match subject {
        Subject::Visitor => format!("{}visitor", PARTIAL_JOURNEY_MAP_INFO),
        Subject::Raider => String::new(),
        Subject::Citizen { job_key, .. } => match job_key {
            Some(key) => key.to_owned(),
            None => unemployed,
        },
    };

    if subject == Subject::Raider {
        return Radar {
            hide: options.raiders == RaiderColor::None,
            tooltips: None,
            blank_name: false,
            colour: if options.raiders.is_hostile() {
                None
            } else {
                Some(options.raiders.color())
            },
            job_key,
        };
    }

    let hidden = match subject {
        Subject::Visitor => !options.visitors,
        Subject::Citizen { guard, .. } => {
            if guard { !options.guards } else { !options.citizens }
        }
        Subject::Raider => false,
    };
    if hidden {
        return Radar {
            hide: true,
            tooltips: None,
            blank_name: false,
            colour: None,
            job_key,
        };
    }

    let show_name = if on_minimap {
        options.colonist_name_minimap
    } else {
        options.colonist_name_fullscreen
    };
    let tooltips = match (options.colonist_tooltips, name) {
        (true, Some(name)) => Some((name.to_owned(), job_key.clone())),
        _ => None,
    };
    let colour = match subject {
        Subject::Citizen { .. } if options.colonist_team => team_colour.map(Color::Rgb),
        _ => None,
    };
    Radar {
        hide: false,
        tooltips,
        blank_name: !show_name,
        colour,
        job_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(x: i32, z: i32) -> ChunkPos {
        ChunkPos::new(x, z)
    }

    fn set(chunks: &[(i32, i32)]) -> ChunkSet {
        let mut set = ChunkSet::new();
        set.add_all(chunks.iter().map(|(x, z)| chunk(*x, *z)));
        set
    }

    // ---- the options --------------------------------------------------------

    #[test]
    fn the_defaults_are_the_ones_journeymap_is_shipped_with() {
        let options = MarkerOptions::default();
        assert_eq!(options.border_fullscreen, BorderStyle::Filled);
        assert_eq!(options.border_minimap, BorderStyle::Framed);
        assert_eq!(options.border_loaded, BorderStyle::Hidden);
        assert!(options.deathpoints && options.colony_name);
        assert_eq!(options.raiders, RaiderColor::Hostile);
        assert!(options.any_border(), "framed and filled are both on");
    }

    #[test]
    fn all_three_borders_off_is_no_border() {
        let options = MarkerOptions {
            border_fullscreen: BorderStyle::Hidden,
            border_minimap: BorderStyle::Hidden,
            border_loaded: BorderStyle::Hidden,
            ..MarkerOptions::default()
        };
        assert!(!options.any_border());
        assert!(!BorderStyle::Hidden.is_drawn());
        assert!(BorderStyle::Framed.is_drawn());
    }

    #[test]
    fn the_thirteen_options_are_declared_in_a_stable_order() {
        assert_eq!(SORT_ORDER.len(), 13);
        let mut orders: Vec<i32> = SORT_ORDER.iter().map(|(_, order)| *order).collect();
        orders.sort_unstable();
        orders.dedup();
        assert_eq!(orders.len(), 13, "two options share a sort order");
        // The three border styles come first and the citizens last, as in
        // `JourneymapOptions.java:35-47`.
        assert_eq!(SORT_ORDER[0], ("borderFullscreenStyle", 100));
        assert_eq!(SORT_ORDER[12], ("raiders", 303));
    }

    #[test]
    fn two_raider_colours_are_literals_and_four_are_vanilla_formatting() {
        // `TextColor.fromRgb` at `JourneymapOptions.java:137-138`.
        assert_eq!(RaiderColor::Hostile.color(), Color::Rgb(0xFFFF_FFFF));
        assert_eq!(RaiderColor::None.color(), Color::Rgb(0xFF00_0000));
        // `TextColor.fromLegacyFormat` at `:139-142`, kept as a name.
        assert_eq!(RaiderColor::Yellow.color(), Color::Legacy("yellow"));
        assert_eq!(RaiderColor::Red.color(), Color::Legacy("red"));
        assert_eq!(RaiderColor::Purple.color(), Color::Legacy("light_purple"));
        assert_eq!(RaiderColor::Orange.color(), Color::Legacy("gold"));
        assert!(RaiderColor::Hostile.is_hostile());
        assert!(!RaiderColor::None.is_hostile());
    }

    // ---- the outline record -------------------------------------------------

    #[test]
    fn the_overlay_group_name_carries_the_whole_dimension_key() {
        // `String.format("colony_%s_%d", dimension.location(), id)`
        // (`ColonyBorderMapping.java:258`); `location()` is a `ResourceLocation`,
        // so `%s` prints `minecraft:overworld`, colon and all.
        let overlay = BorderOverlay::new("minecraft:overworld", 7);
        assert_eq!(overlay.group_name(), "colony_minecraft:overworld_7");
        assert_eq!(overlay.colour, INITIAL_COLOUR.value, "0x00ff00 until the colony answers");
        assert!(overlay.permitted, "Java starts permitted");
    }

    #[test]
    fn updating_chunks_answers_whether_the_set_changed() {
        // `ColonyBorderMapping.java:292-296`.
        let mut overlay = BorderOverlay::new("overworld", 1);
        assert!(overlay.update_chunks(&set(&[(0, 0), (1, 0)]), &set(&[])));
        assert!(!overlay.update_chunks(&set(&[(0, 0)]), &set(&[])), "already owned");
        assert!(overlay.update_chunks(&set(&[]), &set(&[(1, 0)])));
        assert_eq!(overlay.chunks.len(), 1);
        assert!(overlay.chunks.contains(chunk(0, 0)));
        assert!(!overlay.chunks.contains(chunk(1, 0)));
    }

    #[test]
    fn a_chunk_can_be_owned_and_loaded_at_once() {
        // The two sets are independent (`:289-308`): a claimed chunk that is not
        // loaded is outlined but not filled in.
        let mut overlay = BorderOverlay::new("overworld", 1);
        overlay.update_chunks(&set(&[(4, 4)]), &set(&[]));
        overlay.update_loaded_chunks(&set(&[]), &set(&[]));
        assert!(overlay.chunks.contains(chunk(4, 4)));
        assert!(overlay.loaded.is_empty());
    }

    #[test]
    fn info_changes_only_when_something_moved() {
        // `:330-331`.
        let mut overlay = BorderOverlay::new("overworld", 1);
        assert!(overlay.update_info("Aurore", 0xFF0000, true), "the first name");
        assert!(!overlay.update_info("Aurore", 0xFF0000, true));
        assert!(overlay.update_info("Aurore", 0x00FF00, true), "a new colour");
        assert!(overlay.update_info("Aurore", 0x00FF00, false), "a lost permission");
        assert_eq!(overlay.colony_name, "Aurore");
        assert!(!overlay.permitted);
    }

    #[test]
    fn a_refused_outline_is_not_drawn_even_with_chunks() {
        // `enabled = permitted && !(hidden && hidden && hidden)` (`:375-378`).
        let mut overlay = BorderOverlay::new("overworld", 1);
        overlay.update_chunks(&set(&[(0, 0)]), &set(&[]));
        assert!(overlay.shows(&MarkerOptions::default()));
        overlay.update_info("Aurore", 0xFF0000, false);
        assert!(!overlay.shows(&MarkerOptions::default()));
        let all_hidden = MarkerOptions {
            border_fullscreen: BorderStyle::Hidden,
            border_minimap: BorderStyle::Hidden,
            border_loaded: BorderStyle::Hidden,
            ..MarkerOptions::default()
        };
        let mut allowed = BorderOverlay::new("overworld", 1);
        allowed.update_chunks(&set(&[(0, 0)]), &set(&[]));
        assert!(!allowed.shows(&all_hidden), "no style is on");
        assert!(allowed.enabled(&MarkerOptions::default()));
    }

    #[test]
    fn an_empty_outline_has_nothing_to_draw() {
        let overlay = BorderOverlay::new("overworld", 1);
        assert!(!overlay.shows(&MarkerOptions::default()), "no chunks yet");
    }

    #[test]
    fn the_loaded_band_is_an_intensified_colour_without_alpha() {
        // `:332`, `0xFFFFFF & intensify(colour)`.
        let mut overlay = BorderOverlay::new("overworld", 1);
        overlay.update_info("Aurore", 0xFF80_FF80u32 as i32, true);
        assert_eq!(overlay.loaded_colour(), 0x0060_FF60);
    }

    #[test]
    fn intensify_pushes_the_dominant_channel_and_keeps_alpha() {
        // `FastColor.ARGB32.lerp(0.25f, …)` (`:355-367`). 128 → 96 is exact:
        // 128 + 0.25 * (0 - 128).
        assert_eq!(intensify(0xFF80_FF80), 0xFF60_FF60);
        let kept = intensify(0xFF00_FF00);
        assert_eq!(kept, 0xFF00_FF00, "already pure");
        assert_eq!(kept >> 24, 0xFF, "alpha carried through");
    }

    // ---- border.json --------------------------------------------------------

    #[test]
    fn chunks_round_trip_through_the_long_a_save_stores() {
        // `Codec.LONG.xmap(ChunkPos::new, ChunkPos::toLong)`
        // (`ColonyBorderMapping.java:222-224`).
        let written = set(&[(0, 0), (-1, 2), (1_000, -1_000)]);
        let packed = written.packed();
        assert_eq!(packed.len(), 3);
        let read = ChunkSet::from_packed(packed);
        assert_eq!(read, written);
        // Negative coordinates survive the pack: the whole point of the mask.
        assert!(read.contains(chunk(-1, 2)));
        assert!(read.contains(chunk(1_000, -1_000)));
    }

    #[test]
    fn the_file_order_is_sorted_not_a_hash_tables() {
        // Java wraps the list in a `HashSet` on both sides (`:222-224`, `:232-233`),
        // so its order is unspecified; the port sorts.
        let first = set(&[(5, 5), (1, 1), (3, 3)]).packed();
        let second = set(&[(3, 3), (5, 5), (1, 1)]).packed();
        assert_eq!(first, second);
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
    }

    // ---- deathpoints --------------------------------------------------------

    fn graves(points: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
        points.iter().copied().collect()
    }

    fn point(at: [i32; 3], name: &str) -> Deathpoint {
        Deathpoint {
            at,
            label: DeathpointLabel::Name(name.to_owned()),
        }
    }

    #[test]
    fn a_new_grave_is_pending_until_its_chunk_is_read() {
        // `computeIfAbsent(grave, k -> tryCreatingWaypoint(...))` (`:108`).
        let mut deathpoints = Deathpoints::new();
        let outcome = deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        assert_eq!(outcome.pending, vec![[0, 64, 0]]);
        assert_eq!(outcome.removed.len(), 0);
        assert!(deathpoints.get([0, 64, 0]).is_none());
        assert!(deathpoints.built(point([0, 64, 0], "Ada")));
        assert_eq!(
            deathpoints.get([0, 64, 0]).map(|point| point.label.clone()),
            Some(DeathpointLabel::Name("Ada".to_owned()))
        );
    }

    #[test]
    fn a_grave_that_is_gone_loses_its_waypoint() {
        // `:90-102`.
        let mut deathpoints = Deathpoints::new();
        deathpoints.sync(&graves(&[[0, 64, 0], [16, 64, 0]]), true);
        deathpoints.built(point([0, 64, 0], "Ada"));
        deathpoints.built(point([16, 64, 0], "Bob"));
        let outcome = deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        assert_eq!(outcome.removed, vec![[16, 64, 0]]);
        assert_eq!(deathpoints.len(), 1);
        assert!(deathpoints.get([0, 64, 0]).is_some());
    }

    #[test]
    fn a_player_without_the_permission_loses_every_waypoint() {
        // `!permitted` drops the lot (`:94`), and the option is folded in at `:87-88`.
        let mut deathpoints = Deathpoints::new();
        deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        deathpoints.built(point([0, 64, 0], "Ada"));
        let outcome = deathpoints.sync(&graves(&[[0, 64, 0]]), false);
        assert!(outcome.refused);
        assert_eq!(outcome.removed, vec![[0, 64, 0]]);
        assert!(deathpoints.is_empty(), "the cache is dropped too");
    }

    #[test]
    fn unloading_removes_the_waypoints_and_keeps_the_graves() {
        // `ColonyDeathpoints.unload` (`:56-70`): the cache stays so coming back to
        // a dimension does not have to find the grave again.
        let mut deathpoints = Deathpoints::new();
        deathpoints.sync(&graves(&[[0, 64, 0], [16, 64, 0]]), true);
        deathpoints.built(point([0, 64, 0], "Ada"));
        deathpoints.built(point([16, 64, 0], "Bob"));
        let removed = deathpoints.unload();
        assert_eq!(removed.len(), 2);
        assert_eq!(deathpoints.len(), 2, "the graves are still known");
        assert_eq!(deathpoints.pending().count(), 2, "and must be rebuilt");
        assert!(deathpoints.get([0, 64, 0]).is_none());
    }

    #[test]
    fn a_just_loaded_chunk_reports_its_own_pending_graves() {
        // `updateChunk` (`:143-149`): only the pending waypoints of *this* chunk.
        let mut deathpoints = Deathpoints::new();
        deathpoints.sync(&graves(&[[0, 64, 0], [40, 64, 0]]), true);
        assert_eq!(deathpoints.pending_in(chunk(0, 0)), vec![[0, 64, 0]]);
        assert_eq!(deathpoints.pending_in(chunk(2, 0)), vec![[40, 64, 0]]);
        assert_eq!(deathpoints.pending_in(chunk(9, 9)), Vec::<[i32; 3]>::new());
    }

    #[test]
    fn clearing_forgets_the_graves_without_removing_a_waypoint() {
        // `clear` only empties the cache (`:46-49`, "Note this does *not* remove
        // the overlays from jmap"); the caller drops them with `removeAll`
        // (`EventListener.java:57-58`).
        let mut deathpoints = Deathpoints::new();
        deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        deathpoints.built(point([0, 64, 0], "Ada"));
        deathpoints.clear();
        assert!(deathpoints.is_empty());
        assert_eq!(deathpoints.pending().count(), 0, "the graves are gone too");
    }

    #[test]
    fn a_grave_whose_waypoint_is_not_built_yet_is_asked_again() {
        // `computeIfAbsent` runs for every grave at every update (`:108`) and
        // stores nothing when the function answers null, so the second call asks
        // again with the same graves.
        let mut deathpoints = Deathpoints::new();
        let first = deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        let second = deathpoints.sync(&graves(&[[0, 64, 0]]), true);
        assert_eq!(first.pending, vec![[0, 64, 0]]);
        assert_eq!(second.pending, vec![[0, 64, 0]], "still unbuilt, so still asked");
        assert!(second.removed.is_empty());
        // Once it exists, the next update stops asking.
        deathpoints.built(point([0, 64, 0], "Ada"));
        assert!(deathpoints.sync(&graves(&[[0, 64, 0]]), true).pending.is_empty());
    }

    #[test]
    fn a_grave_with_a_job_names_both() {
        // `deathpoint_namejob` with the name and the job (`:178`).
        let label = DeathpointLabel::NameJob {
            name: "Ada".to_owned(),
            job: "Farmer".to_owned(),
        };
        assert_eq!(label.args(), vec!["Ada", "Farmer"]);
        assert_eq!(
            label.key(),
            "com.minecolonies.coremod.journeymap.deathpoint_namejob"
        );
        let only_name = DeathpointLabel::Name("Ada".to_owned());
        assert_eq!(only_name.args(), vec!["Ada"]);
        assert_eq!(
            only_name.key(),
            "com.minecolonies.coremod.journeymap.deathpoint_name"
        );
        assert!(label.key().starts_with(PARTIAL_JOURNEY_MAP_INFO));
    }

    #[test]
    fn a_grave_waypoint_keeps_the_icon_and_colour_java_gives_it() {
        assert_eq!(Deathpoint::ICON, "minecolonies:textures/icons/grave_icon.png");
        assert_eq!(Deathpoint::ICON_SIZE, (16, 16));
        assert_eq!(Deathpoint::COLOUR, 0x0088_8888);
    }

    // ---- the radar ----------------------------------------------------------

    #[test]
    fn a_visitor_is_hidden_by_its_own_option() {
        // `:199-203`.
        let options = MarkerOptions {
            visitors: false,
            ..MarkerOptions::default()
        };
        let ask = radar(Subject::Visitor, Some("Ana"), None, false, &options);
        assert!(ask.hide);
        assert_eq!(
            ask.job_key,
            "com.minecolonies.coremod.journeymap.visitor"
        );
    }

    #[test]
    fn a_guard_answers_to_guards_and_a_citizen_to_citizens() {
        // `job instanceof AbstractJobGuard ? guards : citizens` (`:111-117`).
        let no_guards = MarkerOptions {
            guards: false,
            ..MarkerOptions::default()
        };
        let guard = Subject::Citizen {
            job_key: Some("com.minecolonies.job.knight"),
            guard: true,
        };
        let citizen = Subject::Citizen {
            job_key: Some("com.minecolonies.job.farmer"),
            guard: false,
        };
        assert!(radar(guard, None, None, false, &no_guards).hide);
        assert!(!radar(citizen, None, None, false, &no_guards).hide);
        let no_citizens = MarkerOptions {
            citizens: false,
            ..MarkerOptions::default()
        };
        assert!(radar(citizen, None, None, false, &no_citizens).hide);
        assert!(!radar(guard, None, None, false, &no_citizens).hide);
    }

    #[test]
    fn an_unemployed_citizen_has_its_own_line() {
        // `jobEntry == null ? unemployed : …` (`:119-121`).
        let ask = radar(Subject::Citizen { job_key: None, guard: false }, Some("Ada"), None, false, &MarkerOptions::default());
        assert_eq!(
            ask.job_key,
            "com.minecolonies.coremod.journeymap.unemployed"
        );
        let tooltips = ask.tooltips.expect("tooltips are on by default");
        assert_eq!(tooltips.0, "Ada");
        assert_eq!(tooltips.1, ask.job_key);
    }

    #[test]
    fn the_name_is_shown_by_whichever_map_asks() {
        // `:235-242`: minimap and fullscreen are separate options.
        let options = MarkerOptions {
            colonist_name_minimap: false,
            colonist_name_fullscreen: true,
            ..MarkerOptions::default()
        };
        let subject = Subject::Citizen {
            job_key: Some("com.minecolonies.job.farmer"),
            guard: false,
        };
        assert!(radar(subject, Some("Ada"), None, true, &options).blank_name);
        assert!(!radar(subject, Some("Ada"), None, false, &options).blank_name);
    }

    #[test]
    fn tooltips_can_be_turned_off_and_need_a_name() {
        // `if (getShowColonistTooltip()) { name = entity.getCustomName(); if (name != null) … }`
        // (`:124-131`).
        let off = MarkerOptions {
            colonist_tooltips: false,
            ..MarkerOptions::default()
        };
        let subject = Subject::Citizen {
            job_key: Some("com.minecolonies.job.farmer"),
            guard: false,
        };
        assert_eq!(radar(subject, Some("Ada"), None, false, &off).tooltips, None);
        assert_eq!(radar(subject, None, None, false, &MarkerOptions::default()).tooltips, None);
    }

    #[test]
    fn a_citizen_takes_its_team_colour_but_a_visitor_does_not() {
        // `!isVisitor && getShowColonistTeamColour()` (`:142-145`).
        let citizen = Subject::Citizen {
            job_key: Some("com.minecolonies.job.farmer"),
            guard: false,
        };
        assert_eq!(
            radar(citizen, None, Some(0x00FF_00FFu32), false, &MarkerOptions::default()).colour,
            Some(Color::Rgb(0x00FF_00FF))
        );
        assert_eq!(
            radar(Subject::Visitor, None, Some(0xFF00_00FFu32), false, &MarkerOptions::default()).colour,
            None,
            "a visitor is not the colony's colour"
        );
        let no_team = MarkerOptions {
            colonist_team: false,
            ..MarkerOptions::default()
        };
        assert_eq!(radar(citizen, None, Some(0x00FF_FFFFu32), false, &no_team).colour, None);
    }

    #[test]
    fn a_raider_has_only_a_colour_rule() {
        // `:249-261`: NONE hides, HOSTILE leaves the dot alone, anything else recolours.
        let hidden = MarkerOptions {
            raiders: RaiderColor::None,
            ..MarkerOptions::default()
        };
        assert!(radar(Subject::Raider, None, None, false, &hidden).hide);
        let hostile = radar(Subject::Raider, None, None, false, &MarkerOptions::default());
        assert!(!hostile.hide);
        assert_eq!(hostile.colour, None, "HOSTILE means: as JourneyMap draws it");
        let purple = MarkerOptions {
            raiders: RaiderColor::Purple,
            ..MarkerOptions::default()
        };
        assert_eq!(
            radar(Subject::Raider, None, None, false, &purple).colour,
            Some(Color::Legacy("light_purple"))
        );
    }

    // ---- the info slot ------------------------------------------------------

    #[test]
    fn the_info_slot_asks_every_two_and_a_half_seconds() {
        assert_eq!(INFO_SLOT_PERIOD.value, 2500);
        assert_eq!(
            INFO_SLOT_KEY,
            "com.minecolonies.coremod.journeymap.currentcolony"
        );
        assert!(INFO_SLOT_KEY.starts_with(PARTIAL_JOURNEY_MAP_INFO));
    }
}
