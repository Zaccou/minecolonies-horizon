//! The 86 blocks of MineColonies: what block `X` is, what a click on it does,
//! what a placement writes, and how long a break takes.
//!
//! Java reference: `api/blocks/` (the abstract classes and the registry) and
//! `core/blocks/` (the concrete ones). **86 blocks are registered** — the
//! initialiser calls `registerBlock` 87 times and one call is commented out
//! (`apiimp/initializer/ModBlocksInitializer.java:151`, the `largeQuarry` line),
//! which is also why `ModBlocks.blockLargeQuarry` does not exist
//! (`api/blocks/ModBlocks.java:71`). `LargeQuarry.java` is on disk and declares a
//! class no registry ever sees.
//!
//! **54 descend from `AbstractColonyBlock`** (305 lines): the 52 hut blocks of
//! `core/blocks/huts/` — 50 `BlockHut*` plus the two registered quarries — and
//! `BlockPostBox` and `BlockStash`, which are colony blocks without being huts.
//! The brief says 56; measured, the class has **54** descendants and **56** blocks
//! carry the `IBuilderUndestroyable` marker the class implements — the two extra
//! are `BlockDecorationController` (`core/blocks/BlockDecorationController.java:51`)
//! and `BlockPlantationField` (`core/blocks/BlockPlantationField.java:46`). Both
//! counts are asserted in `tests`, so the difference cannot be lost.
//!
//! | File | Question |
//! | --- | --- |
//! | `huts` | the 54 colony blocks, and the common behaviour of `AbstractColonyBlock` |
//! | `chantier` | the three blocks a builder hangs, and the anchor it builds by |
//! | `decoration` | the nine that dress a colony: gates, banners, graves, sign, rack, barrel |
//! | `ressource` | the twenty the colony farms: farmland, crops, compost, scarecrow, plot |
//! | `MANQUANT.md` | what a rule needs and the pinned SDK does not give |
//!
//! **Rules, not world calls.** Nothing here reads a level or writes one: a click
//! is an [`Answer`], a placement a [`Placed`] or a [`Refusal`], a break a number.
//! That is what lets every branch of Java's `use` be tested on the host target
//! with no game, no wasm and no prepared resources — the same choice
//! `src/colony/` and `src/building/` make. [`Click`] says which `use` override a
//! block follows; turning an [`Answer`] into a screen is the wiring's job, in
//! `src/lib.rs`, which another agent owns (`MANQUANT.md` §1).
//!
//! **What is *not* here, and what each gap costs.**
//!
//! - **no content file**: `data/minecolonies/horizon/block/` holds three blocks
//!   (`blockhutbuilder.json`, `blockwaypoint.json`, `farmland.json`) out of 86.
//!   [`State`] says, block by block, which state properties that file must
//!   declare and which the template already carries; writing the 83 others is a
//!   content job, not this module's.
//! - **no removal**: what a broken hut *gives back* is
//!   `crate::building::behaviour::casse` — this module answers how long the break
//!   takes, who may not make it, and whether the contents drop.
//! - **no placement validity**: the footprint, the parent and the level are
//!   `crate::founding::placement` and `crate::building::Standing`. What is here is
//!   the *block's* half of Java's placement: the facing it takes, and the
//!   refusals that live in the block class itself.

pub mod chantier;
pub mod decoration;
pub mod huts;
pub mod ressource;

use crate::building::{self, Kind};
use crate::colony::Colony;
use crate::colony::permissions::{Action, Rank};

pub use chantier::BLOCKS as CHANTIER;
pub use decoration::BLOCKS as DECORATION;
pub use huts::BLOCKS as HUTS;
pub use ressource::BLOCKS as RESSOURCE;

/// The namespace every block is registered under: MineColonies' mod id, not this
/// package's (`AbstractColonyBlock.getRegistryName`,
/// `api/blocks/AbstractColonyBlock.java:288-291`).
pub const NAMESPACE: &str = "minecolonies";

/// How many blocks Java registers (`ModBlocksInitializer.java:60-150`, one call
/// commented out at `:151`).
pub const COUNT: usize = 86;

/// How many of them descend from `AbstractColonyBlock`
/// (`api/blocks/AbstractColonyBlock.java:54`).
pub const COLONY_COUNT: usize = 54;

/// How many carry `IBuilderUndestroyable`: the 54 above, plus the decoration
/// controller and the plantation field.
pub const BUILDER_UNDESTROYABLE_COUNT: usize = 56;

/// The hut hardness, `AbstractColonyBlock.HARDNESS`
/// (`api/blocks/AbstractColonyBlock.java:70`).
pub const HUT_HARDNESS: f32 = 10.0;

/// What the pvp mode divides the break progress by,
/// `AbstractColonyBlock.HARDNESS_PVP_FACTOR` (`:60`).
pub const HARDNESS_PVP_FACTOR: f32 = 4.0;

/// What the pvp mode divides the town hall's by again
/// (`core/blocks/huts/BlockHutTownHall.java:129-130`).
pub const TOWN_HALL_PVP_FACTOR: f32 = 12.0;

/// The divisor every colony block's break progress carries
/// (`AbstractColonyBlock.java:114`).
pub const PROGRESS_DIVISOR: f32 = 30.0;

/// How old the last child-building warning must be for the next one to be held
/// back (`AbstractColonyBlock.java:108`).
pub const BREAK_WARNING_WINDOW: i64 = 100;

/// The message constant `getDestroyProgress` sends, spelled as
/// `TranslationConstants` declares it
/// (`api/util/constant/TranslationConstants.java:454`).
pub const BREAK_WARNING_KEY: &str = "HUT_BREAK_WARNING_CHILD_BUILDINGS";

/// `BlockStateProperties.FACING`'s four horizontal values, in vanilla's order
/// (`Direction`'s `by2DData`: south, west, north, east — hence the order of
/// [`FACING_VALUES`] being the *declaration* order and not the index order).
pub const FACING_VALUES: &[&str] = &["north", "south", "east", "west"];

/// The four families, which are the four files. The cut is by **what the player
/// does with the block**, not by Java package: the tape, the decoration
/// controller and the waypoint all live outside `core/blocks/huts/` yet are the
/// builder's own, and the plantation field and the scarecrow are farm furniture
/// even though Java files them under `core/blocks/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: a closed set of four files, one match arm each
pub enum Family {
    /// A colony block: a hut, or the stash and the post box.
    Huts,
    /// A block of the build site.
    Chantier,
    /// A block that dresses a colony.
    Decoration,
    /// A block the colony harvests.
    Ressource,
}

impl Family {
    pub const ALL: [Family; 4] = [
        Family::Huts,
        Family::Chantier,
        Family::Decoration,
        Family::Ressource,
    ];

    /// The file name, which is also the module name.
    pub fn name(self) -> &'static str {
        match self {
            Family::Huts => "huts",
            Family::Chantier => "chantier",
            Family::Decoration => "decoration",
            Family::Ressource => "ressource",
        }
    }

    /// The blocks of this family.
    pub fn blocks(self) -> &'static [Block] {
        match self {
            Family::Huts => HUTS,
            Family::Chantier => CHANTIER,
            Family::Decoration => DECORATION,
            Family::Ressource => RESSOURCE,
        }
    }
}

/// A block's shape, as `getShape` answers it. Every value names the box Java
/// builds, because that box is what the content file reproduces in `shapes` /
/// `collision_shapes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per shape family among the 86 blocks
pub enum Form {
    /// `Shapes.box(0.1, 0.1, 0.1, 0.9, 0.9, 0.9)` — all 54 colony blocks
    /// (`AbstractColonyBlock.java:80`), and the two graves and the colony sign,
    /// which build the same box themselves (`BlockMinecoloniesGrave.java:64`,
    /// `BlockColonySign.java:66`).
    SmallBox,
    /// The same box, 1.9 high: the graveyard hut alone
    /// (`BlockHutGraveyard.java:22`).
    TallBox,
    /// Half the block on the facing side, four boxes
    /// (`BlockPostBox.java:22-25`, `BlockStash.java:37-40`).
    HalfBox,
    /// The full footprint, with a height of its own: 1.5 for the barrel
    /// (`BlockBarrel.java:96-99`), 1.1 for the named grave
    /// (`BlockMinecoloniesNamedGrave.java:89`), the cube for the rack
    /// (`BlockMinecoloniesRack.java:103-108`).
    Full,
    /// A door leaf, 0.3 to 0.7 across (`AbstractBlockGate.java:60-61`).
    Gate,
    /// A hanging cloth
    /// (`core/blocks/decorative/BlockColonyFlagBanner.java:46-50`).
    Banner,
    /// A two-plane cross, grown by age (`MinecoloniesCropBlock.java:72-79`).
    Crop,
    /// A tape with no collision at all (`BlockConstructionTape.java:54-57`).
    Tape,
    /// A plate held against one face, one box per direction: the decoration
    /// controller (`BlockDecorationController.java:76-86`, `:104-134`).
    Plate,
    /// A scarecrow, 0.1 to 0.9 across and 2.2 high
    /// (`api/blocks/huts/AbstractBlockMinecoloniesDefault.java:32-40`).
    TwoTall,
    /// A plot's frame (`BlockPlantationField.java:139-157`).
    Field,
    /// A bookcase's cube (`BlockMinecoloniesRack.java:103-108`).
    Rack,
    /// Nothing at all: `collision_shapes.default` is empty
    /// (`BlockWaypoint.java:12-40`, and the content file already says so).
    Empty,
}

impl Form {
    /// The box, in **block units**, exactly as `Shapes.box` in Java takes them,
    /// or `None` when the shape is not one box — which includes
    /// [`Form::Empty`], whose content file writes
    /// `"collision_shapes": {"default": []}` and no box at all.
    pub fn box_of(self) -> Option<[f32; 6]> {
        match self {
            // `AbstractColonyBlock.java:80`
            Form::SmallBox => Some([0.1, 0.1, 0.1, 0.9, 0.9, 0.9]),
            // `BlockHutGraveyard.java:22`
            Form::TallBox => Some([0.1, 0.1, 0.1, 0.9, 1.9, 0.9]),
            // `AbstractBlockGate.java:60-61`, the east-west leaf
            Form::Gate => Some([0.3, 0.0, 0.0, 0.7, 1.0, 1.0]),
            // no box, or a shape the content file spells per state
            _ => None,
        }
    }
}

/// One state property a block declares, and where it lands in content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// The property name, which is vanilla's and the content file's.
    pub name: &'static str,
    pub values: Values,
    /// The value a placed block starts at, `defaultBlockState().setValue(...)`.
    pub default: &'static str,
    /// Java's declaration, `file.java:line`.
    pub source: &'static str,
    /// How the content file gets it.
    pub content: StateIn,
}

/// What a state property holds.
///
/// **A boolean and a closed list of names are what content can declare**
/// (`crates/horizon-mod-package/src/content.rs:356-363`); Java's blocks here also
/// use integers, which no content file can hold — [`Values::Integer`] is how that
/// third kind is written down, and [`StateIn`] says where it goes instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three property kinds among the 86 blocks
pub enum Values {
    Boolean,
    /// The values, in the order the state space is built in.
    Enum(&'static [&'static str]),
    /// `IntegerProperty.create(name, min, max)`, both ends included.
    Integer { min: i32, max: i32 },
}

impl Values {
    /// The values as names, in order. **Empty for an integer**, which has no list
    /// to give: use [`Values::allows`] to test one value.
    pub fn names(self) -> &'static [&'static str] {
        match self {
            Values::Boolean => &["false", "true"],
            Values::Enum(values) => values,
            Values::Integer { .. } => &[],
        }
    }

    /// Whether `value` is one this property can hold.
    pub fn allows(self, value: &str) -> bool {
        match self {
            Values::Boolean => matches!(value, "false" | "true"),
            Values::Enum(values) => values.contains(&value),
            Values::Integer { min, max } => value
                .parse::<i32>()
                .map(|number| number >= min && number <= max)
                .unwrap_or(false),
        }
    }
}

/// Where a state property comes from in
/// `data/minecolonies/horizon/block/<block>.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three fates a Java property has in content
pub enum StateIn {
    /// Write it in `state_properties`.
    Declared,
    /// The template block already declares it; only `default_state` is needed.
    Template(&'static str),
    /// **The content format cannot hold it.** The string says what is missing;
    /// `MANQUANT.md` repeats it.
    Gap(&'static str),
}

/// A block's material facts: what `Properties` sets, and what the content file's
/// `properties` object repeats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Props {
    /// `Properties.strength(destroyTime)`, in vanilla's units.
    pub hardness: f32,
    /// `Properties.strength(_, explosionResistance)`.
    pub resistance: f32,
    /// The block whose sound group it borrows, spelled as the content files
    /// spell it (`"sounds": "minecraft:oak_planks"` for `SoundType.WOOD`).
    pub sounds: &'static str,
    /// `MapColor`'s id. Two are corroborated by the files already in the
    /// repository: `farmland.json` says 10 for `MapColor.DIRT`, and
    /// `blockhutbuilder.json` 13 for `MapColor.WOOD`.
    pub map_color: u8,
    /// Whether the block has a collision box at all (`noCollission()`).
    pub collision: bool,
    /// `requiresCorrectToolForDrops()`. **False for all 86**: no block here calls
    /// it, and a test holds that.
    pub requires_tool: bool,
}

/// `MapColor.WOOD`, corroborated by `blockhutbuilder.json`.
pub const MAP_COLOR_WOOD: u8 = 13;
/// `MapColor.DIRT`, corroborated by `farmland.json`.
pub const MAP_COLOR_DIRT: u8 = 10;
/// `MapColor.PLANT`.
pub const MAP_COLOR_PLANT: u8 = 7;
/// `MapColor.STONE`.
pub const MAP_COLOR_STONE: u8 = 11;

impl Props {
    /// Wooden, as `Properties.of().mapColor(WOOD).sound(WOOD)`.
    pub const fn wood(hardness: f32, resistance: f32) -> Props {
        Props {
            hardness,
            resistance,
            sounds: "minecraft:oak_planks",
            map_color: MAP_COLOR_WOOD,
            collision: true,
            requires_tool: false,
        }
    }

    /// Stone, as `Properties.of().mapColor(STONE).sound(STONE)`.
    pub const fn stone(hardness: f32, resistance: f32) -> Props {
        Props {
            hardness,
            resistance,
            sounds: "minecraft:stone",
            map_color: MAP_COLOR_STONE,
            collision: true,
            requires_tool: false,
        }
    }

    /// The same material with no collision box (`noCollission()`).
    pub const fn thin(mut self) -> Props {
        self.collision = false;
        self
    }

    /// `Properties.of().mapColor(PLANT).sound(WOOD)`: the construction tape's.
    pub const fn plant(hardness: f32, resistance: f32) -> Props {
        Props {
            hardness,
            resistance,
            sounds: "minecraft:oak_planks",
            map_color: MAP_COLOR_PLANT,
            collision: true,
            requires_tool: false,
        }
    }

    /// `Properties.of().mapColor(DIRT).sound(GRAVEL)`: the farmland's.
    pub const fn dirt(hardness: f32, resistance: f32) -> Props {
        Props {
            hardness,
            resistance,
            sounds: "minecraft:gravel",
            map_color: MAP_COLOR_DIRT,
            collision: true,
            requires_tool: false,
        }
    }

    /// A crop: `mapColor(PLANT).noCollission().instabreak().sound(CROP)`
    /// (`MinecoloniesCropBlock.java:93`).
    pub const fn crop() -> Props {
        Props {
            hardness: 0.0,
            resistance: 0.0,
            sounds: "minecraft:wheat",
            map_color: MAP_COLOR_PLANT,
            collision: false,
            requires_tool: false,
        }
    }
}

/// What a click on a block opens. The enum is the answer of [`click`]; no block
/// *declares* a window, it declares a [`Click`] rule that answers one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: every screen the 86 blocks can open
pub enum Window {
    /// The building's own screen, `IBuildingView.openGui`
    /// (`AbstractColonyBlock.java:228`).
    Hut,
    /// The colony overview, or the founding screen when there is no colony
    /// (`BlockHutTownHall.java:206-211`).
    ColonyOverview,
    /// The reactivation screen of a deactivated building
    /// (`AbstractColonyBlock.java:205`).
    Reactivation,
    /// The gate house's connection module, which opens **without** `ACCESS_HUTS`
    /// (`BlockHutGateHouse.java:62-68`).
    ConnectionModule,
    /// A colony building's inventory (`BlockStash.java:95`).
    BuildingInventory,
    /// The plot assignment of a plantation field
    /// (`BlockPlantationField.java:127`).
    PlantationField,
    /// A scarecrow's field (`BlockScarecrow.java:106`).
    Field,
    /// A rack's chest (`BlockMinecoloniesRack.java:234-241`).
    Rack,
    /// A decoration controller's settings
    /// (`BlockDecorationController.java:165`).
    Decoration,
    /// A grave's inventory (`BlockMinecoloniesGrave.java:145-170`).
    Grave,
}

/// Which rule a block's click follows. One variant per `use` override among the
/// 86 classes: everything else inherits the one at
/// `AbstractColonyBlock.java:174-231`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per `use` override among the 86 blocks
pub enum Click {
    /// `AbstractColonyBlock.use`: the building's screen, `ACCESS_HUTS` required.
    Hut,
    /// `BlockHutTownHall.use`: the colony overview, no permission.
    TownHall,
    /// `BlockHutGateHouse.use`: the hut screen, and the connection module for a
    /// player who may not open huts.
    GateHouse,
    /// `BlockStash.use`: the building's inventory, no screen.
    Stash,
    /// `BlockPlantationField.use`.
    PlantationField,
    /// `BlockScarecrow.use`: a lantern first, the field screen otherwise.
    Scarecrow,
    /// `BlockMinecoloniesRack.use`: the rack's chest.
    Rack,
    /// `BlockDecorationController.use`.
    Decoration,
    /// `AbstractBlockGate.use`: the gate opens or closes, and nothing else
    /// (`AbstractBlockGate.java:92-97`).
    Gate,
    /// `BlockMinecoloniesGrave.use`: the grave's inventory, for anyone outside a
    /// colony (`BlockMinecoloniesGrave.java:145-170`).
    Grave,
    /// `BlockBarrel.use`: the held stack goes in and the fullness follows
    /// (`BlockBarrel.java:75-90`).
    Barrel,
    /// No `use` override, or one that answers nothing of ours.
    None,
}

/// What the hand holds, for the one block that reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three cases Java tells apart
pub enum Held {
    Nothing,
    /// A vanilla lantern (`BlockScarecrow.java:94`).
    Lantern,
    /// Anything else.
    Other,
}

/// The question a click sends to the server instead of opening a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one message among the 86 classes
pub enum Question {
    /// `GetColonyInfoMessage` (`BlockHutTownHall.java:210`).
    ColonyInfo,
}

/// What a click does. The caller turns it into a screen, a message or a state
/// write; the rule says which, and never touches the world.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the port's own answer set, closed on purpose
pub enum Answer {
    /// Open this screen. `shift` is Java's `player.isShiftKeyDown()`.
    Open { window: Window, shift: bool },
    /// Ask the server, then open.
    Ask(Question),
    /// Change the block and use the item (`BlockScarecrow.java:94-101`): the
    /// property and its new value.
    Consume {
        sets: Option<(&'static str, &'static str)>,
    },
    /// Flip this boolean property (`AbstractBlockGate.java:92-97`).
    Toggle { property: &'static str },
    /// The block entity takes the held stack and its own variant follows
    /// (`BlockBarrel.java:81-87`, `TileEntityBarrel.useBarrel`).
    Insert,
    /// Nothing of ours: the engine's own use runs, which for a block with a
    /// container is the menu it opens (`src/menu.rs`).
    Pass,
    /// Refused, with the message Java sends.
    Refused(Refusal),
}

/// Why a click, a placement or a lookup was refused, named.
///
/// A variant is **not** a `lang` key: the sentences live in
/// `assets/minecolonies/lang/`, which this module does not own. Java's own
/// constant name is quoted where it has one, and [`Refusal::java`] gives the line
/// the rule is written on.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the port's own refusal set, closed on purpose
pub enum Refusal {
    /// The click came through the off hand and Java answers `FAIL` so a second
    /// screen cannot open (`AbstractColonyBlock.java:187-190`).
    OffHand,
    /// A deactivated building whose chunk belongs to no colony
    /// (`AbstractColonyBlock.java:197-201`, `MISSING_COLONY`).
    MissingColony,
    /// Nothing is registered at this position
    /// (`AbstractColonyBlock.java:210-214`, `HUT_BLOCK_MISSING_BUILDING`).
    NoBuilding,
    /// The building exists but knows no colony
    /// (`AbstractColonyBlock.java:216-220`, `HUT_BLOCK_MISSING_COLONY`).
    NoColony,
    /// The block entity the click needed is not there
    /// (`BlockPlantationField.java:129-132`, `BlockMinecoloniesRack.java:232-245`).
    NoBlockEntity { block: &'static str },
    /// The rank does not carry `Action.ACCESS_HUTS`
    /// (`AbstractColonyBlock.java:222-226`, `PERMISSION_DENIED`).
    LacksAccess { rank: Rank },
    /// A real block of the 86 with no building to look up: the 32 that are not
    /// colony blocks answer this, and it is not an error in the data.
    NotAColonyBlock { block: &'static str },
    /// No block of the 86 has this path.
    UnknownBlock { path: String },
    /// A hut placed outside a colony (`AbstractBlockHut.java:309-327`): the two
    /// messages are `MESSAGE_WARNING_TOWN_HALL_NOT_PRESENT` when the player owns
    /// no colony, and `..._TOO_FAR_AWAY` when he owns one elsewhere.
    OutsideColony { owns_one: bool },
    /// The rank does not carry `Action.PLACE_HUTS`
    /// (`AbstractBlockHut.java:328-333`, `PERMISSION_OPEN_HUT`).
    LacksPlaceHuts { rank: Rank },
    /// A rule of the `building` module refused the placement. The one the block
    /// asks with, and the reason this variant exists,
    /// is `building::Refusal::TownHallAlreadyThere`: Java's
    /// `BlockHutTownHall.canPlaceAt` (`:219-234`) and
    /// `building::Standing::check_placement` are the same rule, so the port keeps
    /// **one** name for it and this module reuses it.
    Building(building::Refusal),
    /// A second tavern in one colony (`BlockHutTavern.java:50-65`,
    /// `WARNING_DUPLICATE_TAVERN`). Only a tavern carries the `tavern_visitor`
    /// module (`crate::building::behaviour::modules::attached_to`), so "a building
    /// with that module" and "a tavern" are the same set — and the `building`
    /// module has no name for this rule, so the port has one here.
    TavernAlreadyThere { at: [i32; 3] },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::OffHand => "blocks.refusal.off_hand",
            Self::MissingColony => "blocks.refusal.missing_colony",
            Self::NoBuilding => "blocks.refusal.no_building",
            Self::NoColony => "blocks.refusal.no_colony",
            Self::NoBlockEntity { .. } => "blocks.refusal.no_block_entity",
            Self::LacksAccess { .. } => "blocks.refusal.lacks_access_huts",
            Self::NotAColonyBlock { .. } => "blocks.refusal.not_a_colony_block",
            Self::UnknownBlock { .. } => "blocks.refusal.unknown_block",
            Self::OutsideColony { .. } => "blocks.refusal.outside_colony",
            Self::LacksPlaceHuts { .. } => "blocks.refusal.lacks_place_huts",
            Self::Building(_) => "blocks.refusal.building",
            Self::TavernAlreadyThere { .. } => "blocks.refusal.tavern_already_there",
        }
    }

    /// The Java line this refusal is written on, `file.java:line`, so a log line
    /// says where to look. `src/ai/` uses the same convention.
    pub fn java(&self) -> &'static str {
        match self {
            Self::OffHand => "AbstractColonyBlock.java:187-190",
            Self::MissingColony => "AbstractColonyBlock.java:197-201",
            Self::NoBuilding => "AbstractColonyBlock.java:210-214",
            Self::NoColony => "AbstractColonyBlock.java:216-220",
            Self::NoBlockEntity { .. } => "BlockPlantationField.java:129-132",
            Self::LacksAccess { .. } => "AbstractColonyBlock.java:222-226",
            Self::NotAColonyBlock { .. } => "AbstractColonyBlock.java:136",
            Self::UnknownBlock { .. } => "ModBlocksInitializer.java:60-150",
            Self::OutsideColony { .. } => "AbstractBlockHut.java:316-324",
            Self::LacksPlaceHuts { .. } => "AbstractBlockHut.java:328-333",
            Self::Building(_) => "BlockHutTownHall.java:219-234",
            Self::TavernAlreadyThere { .. } => "BlockHutTavern.java:50-65",
        }
    }
}

impl std::fmt::Display for Refusal {
    /// The name, and for [`Self::Building`] the rule that said no: several
    /// building rules share that variant, and a line that hid which one would be
    /// unreadable. `founding::Refusal` formats the same way.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Building(inner) => write!(f, "{}: {inner}", self.name()),
            _ => f.write_str(self.name()),
        }
    }
}

impl std::error::Error for Refusal {}

/// One block of the 86.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    /// The registry path **without** its namespace: `blockhutbuilder`,
    /// `barrel_block`, `simplequarry`. Java's `getHutName()` for the colony
    /// blocks, the `BLOCK_NAME` constant for the others.
    pub name: &'static str,
    /// The concrete class and the line it is declared on, `file.java:line`, so a
    /// row can be checked against the source it claims.
    pub source: &'static str,
    pub family: Family,
    pub form: Form,
    pub click: Click,
    /// The building this block *is*, when it is one. `None` for the 32 that are
    /// not colony blocks — which is what [`kind_of`] refuses with
    /// [`Refusal::NotAColonyBlock`].
    pub building: Option<Kind>,
    pub props: Props,
    /// The state properties the block declares, in Java's declaration order.
    pub states: &'static [State],
    /// Whether the block entity holds an inventory Java drops when the block goes
    /// (`AbstractColonyBlock.onRemove`, `:148-156`).
    pub contents: bool,
    /// Whether a builder must leave it alone: **yes** for the 56 that carry
    /// `IBuilderUndestroyable`
    /// (`api/entity/ai/workers/util/IBuilderUndestroyable.java:6`).
    pub undestroyable: bool,
    /// The farmland a crop is sown on, as a block name. `None` for everything
    /// that is not a crop.
    pub sown_on: Option<&'static str>,
}

impl Block {
    /// A colony block: the hut shape, the hut material, `FACING` only, and the
    /// building it stands for. `Click::Hut` unless the concrete class overrides
    /// `use`, which [`Block::custom`] is for.
    pub const fn hut(name: &'static str, source: &'static str, kind: Kind) -> Block {
        Block {
            name,
            source,
            family: Family::Huts,
            form: Form::SmallBox,
            click: Click::Hut,
            building: Some(kind),
            // `AbstractColonyBlock.java:99`: WOOD map colour, WOOD sound,
            // `strength(10, ∞)`, `noOcclusion`
            props: Props::wood(HUT_HARDNESS, f32::INFINITY),
            states: &[FACING],
            contents: true,
            undestroyable: true,
            sown_on: None,
        }
    }

    /// The same, with the parts a concrete colony class overrides: the shape and
    /// the click rule.
    pub const fn custom(
        name: &'static str,
        source: &'static str,
        kind: Kind,
        form: Form,
        click: Click,
    ) -> Block {
        let mut block = Block::hut(name, source, kind);
        block.form = form;
        block.click = click;
        block
    }

    /// A block that is not a colony block: no building, no `ACCESS_HUTS`, and a
    /// builder may remove it.
    pub const fn plain(
        name: &'static str,
        source: &'static str,
        family: Family,
        form: Form,
        click: Click,
        props: Props,
        states: &'static [State],
    ) -> Block {
        Block {
            name,
            source,
            family,
            form,
            click,
            building: None,
            props,
            states,
            contents: false,
            undestroyable: false,
            sown_on: None,
        }
    }

    /// Marks a block a builder must not remove (`IBuilderUndestroyable`).
    pub const fn undestroyable_block(mut self) -> Block {
        self.undestroyable = true;
        self
    }

    /// Marks a block whose block entity holds an inventory.
    pub const fn with_contents(mut self) -> Block {
        self.contents = true;
        self
    }

    /// Marks a crop, and the farmland it wants.
    pub const fn sown(mut self, soil: &'static str) -> Block {
        self.sown_on = Some(soil);
        self
    }

    /// The full registry path, `minecolonies:blockhutbuilder`.
    pub fn path(&self) -> String {
        format!("{NAMESPACE}:{}", self.name)
    }

    /// Whether a builder may remove it.
    pub fn builder_may_break(&self) -> bool {
        !self.undestroyable
    }
}

/// `BlockStateProperties.FACING`, as every colony block declares it
/// (`AbstractColonyBlock.java:65`, default `NORTH` at `:100`).
pub const FACING: State = State {
    name: "facing",
    values: Values::Enum(FACING_VALUES),
    default: "north",
    source: "AbstractColonyBlock.java:65",
    content: StateIn::Declared,
};

/// Every block of the 86, family by family.
pub fn all() -> impl Iterator<Item = &'static Block> {
    Family::ALL.into_iter().flat_map(Family::blocks)
}

/// The block a path names, or `None`.
///
/// The namespace is optional, and ignored when it is [`NAMESPACE`]: a path
/// written by hand is read the only way that can work, since no other mod's block
/// is loadable by this one.
pub fn block(path: &str) -> Option<&'static Block> {
    let name = match path.split_once(':') {
        None => path,
        Some((namespace, rest)) if namespace == NAMESPACE => rest,
        Some(_) => return None,
    };
    all().find(|block| block.name == name)
}

/// The building a **block path** names.
///
/// This is the one conversion between the two spellings a colony record carries:
/// `NbtTagConstants.java:140` stores the building's type path
/// (`minecolonies:townhall`), while a `colony::Building` built by this port's own
/// tests holds the **block** path (`minecolonies:blockhuttownhall`,
/// `src/commands/rules.rs:452-454`). Both are read here; the building-path side
/// stays [`crate::founding::placement::hut_of`]'s.
///
/// **Not a copy of `Kind::from_path`**: that one reads a building path and answers
/// `None` for every hut block. This one strips the block prefix first, and the
/// exceptions come from the Java classes, not from a rule —
/// `blockhutcitizen` is the residence (`ModBuildings.HOME_ID = "residence"`,
/// `api/colony/buildings/ModBuildings.java:25`) and the two quarries have no
/// `blockhut` prefix at all (`SimpleQuarry.java:23`, `MediumQuarry.java:23`).
pub fn kind_of(path: &str) -> Result<Kind, Refusal> {
    if let Some(block) = block(path) {
        return block
            .building
            .ok_or(Refusal::NotAColonyBlock { block: block.name });
    }
    // No block carries this name, so it is the **building** spelling
    // (`minecolonies:townhall`, `ModBuildings.TOWNHALL_ID`, `ModBuildings.java:37`,
    // against the block's `blockhuttownhall`, `BlockHutTownHall.java:159`), and
    // `Kind::from_path` is what reads one. A name neither spells is unknown.
    Kind::from_path(path)
        .filter(|kind| block_of(*kind).is_some())
        .ok_or_else(|| Refusal::UnknownBlock {
            path: path.to_owned(),
        })
}

/// The block that stands for a building, when one does. A test holds that all 54
/// kinds answer one.
pub fn block_of(kind: Kind) -> Option<&'static Block> {
    all().find(|block| block.building == Some(kind))
}

/// The facing a hut takes when a player places it.
///
/// Java asks `Direction.fromYRot(player.getYRot())` and **not**
/// `getHorizontalDirection().getOpposite()` as `HorizontalDirectionalBlock` does
/// (`AbstractColonyBlock.java:237`), so a hut faces the way its placer looks and
/// not the way he looks *at* it. A dispenser has no player, and Java answers
/// `NORTH` for it (`:237`).
///
/// The formula is vanilla's `Direction.fromYRot`; Horizon implements it at
/// `crates/horizon-world/src/level/place.rs:152-156`, which is where the
/// four-way rounding below is read from. It is not callable from a mod (it is
/// `pub(crate)`), so the rule is written out here.
pub fn facing_from_yaw(yaw: Option<f32>) -> &'static str {
    let Some(yaw) = yaw else {
        return "north";
    };
    // `Direction.from2DDataValue`: 0 = south, 1 = west, 2 = north, 3 = east.
    let index = ((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as usize;
    ["south", "west", "north", "east"][index]
}

/// The facts a click reads: one field per thing Java asks the world, so a test
/// answers each of them without a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    /// The click came through the off hand.
    pub off_hand: bool,
    /// `player.isShiftKeyDown()`.
    pub sneaking: bool,
    /// `IColonyManager.getBuilding(level, pos) != null`.
    pub building: bool,
    /// `building.getColony() != null` (`AbstractColonyBlock.java:216`).
    pub building_has_colony: bool,
    /// `ColonyUtils.getOwningColony(chunk) != 0` (`:197`).
    pub chunk_owned: bool,
    /// The position is one of `getAllClaimingBuildings`' (`:203`).
    pub claiming_building_here: bool,
    /// `BlockPos.ZERO`'s positioned tags hold `DEACTIVATED`
    /// (`:195`, `BuildingConstants.java:83`).
    pub deactivated: bool,
    /// The clicking player's effective rank.
    pub rank: Rank,
    /// The block entity the click needs is there.
    pub block_entity: bool,
    /// What the hand holds.
    pub held: Held,
    /// The scarecrow's `lantern` state (`BlockScarecrow.java:94`).
    pub lantern: bool,
    /// A colony claims this position (`getIColony(level, pos) != null`), which is
    /// what the rack asks (`BlockMinecoloniesRack.java:230`).
    pub colony_covers: bool,
}

impl Facts {
    /// A well-formed click by a member who may open huts: a building with a
    /// colony, nothing deactivated, a free main hand. Tests start here and break
    /// one field at a time.
    pub fn member() -> Facts {
        Facts {
            off_hand: false,
            sneaking: false,
            building: true,
            building_has_colony: true,
            chunk_owned: true,
            claiming_building_here: false,
            deactivated: false,
            rank: Rank::Owner,
            block_entity: true,
            held: Held::Nothing,
            lantern: false,
            colony_covers: true,
        }
    }

    /// The same click by a stranger.
    pub fn stranger(mut self) -> Facts {
        self.rank = Rank::Neutral;
        self
    }

    fn may_open_huts(&self) -> bool {
        self.rank.allows(Action::AccessHuts)
    }
}

/// What a click on `block` does. The order is Java's, branch for branch.
///
/// `AbstractColonyBlock.use` runs on the **client** (`:185`) and only the client
/// opens screens, which is why the rule says *what to open* and not *what to
/// send*: on the server the same click answers `SUCCESS` and nothing else. Two
/// exceptions: the town hall, which asks the server when it does not yet know the
/// colony (`BlockHutTownHall.java:206-211`), and the rack, which opens on the
/// server (`BlockMinecoloniesRack.java:234-241`).
pub fn click(block: &Block, facts: &Facts) -> Answer {
    match block.click {
        Click::Hut => colony_click(facts, facts.sneaking),
        Click::TownHall => town_hall_click(facts),
        Click::GateHouse => gate_house_click(facts),
        Click::Stash => stash_click(facts),
        Click::Rack => rack_click(block, facts),
        Click::PlantationField => field_click(block, facts, Window::PlantationField, true),
        Click::Scarecrow => {
            if facts.held == Held::Lantern && !facts.lantern {
                // `BlockScarecrow.java:94-101`: a lantern goes into the block and
                // one comes off the hand.
                return Answer::Consume {
                    sets: Some(("lantern", "true")),
                };
            }
            field_click(block, facts, Window::Field, false)
        }
        Click::Decoration => field_click(block, facts, Window::Decoration, true),
        // `AbstractBlockGate.use`, `:92-97`: no permission check in the block, no
        // off-hand check, and the level event 1005 or 1011 says which way it went.
        Click::Gate => Answer::Toggle { property: "open" },
        Click::Grave => grave_click(block, facts),
        // `BlockBarrel.use`, `:75-90`: the work happens in the block entity, on
        // the server, and the block still answers `SUCCESS` when there is none.
        Click::Barrel => {
            if !facts.block_entity {
                return Answer::Pass;
            }
            Answer::Insert
        }
        Click::None => Answer::Pass,
    }
}

/// `AbstractColonyBlock.use`, `:174-231`.
fn colony_click(facts: &Facts, shift: bool) -> Answer {
    if facts.off_hand {
        return Answer::Refused(Refusal::OffHand);
    }
    if facts.deactivated && !facts.building {
        if !facts.chunk_owned {
            return Answer::Refused(Refusal::MissingColony);
        }
        if !facts.claiming_building_here {
            return Answer::Open {
                window: Window::Reactivation,
                shift,
            };
        }
    }
    if !facts.building {
        return Answer::Refused(Refusal::NoBuilding);
    }
    if !facts.building_has_colony {
        return Answer::Refused(Refusal::NoColony);
    }
    if !facts.may_open_huts() {
        return Answer::Refused(Refusal::LacksAccess { rank: facts.rank });
    }
    Answer::Open {
        window: Window::Hut,
        shift,
    }
}

/// `BlockHutTownHall.use`, `:181-215`: the overview, no `ACCESS_HUTS`, and the
/// server asked when the client does not know the colony yet.
fn town_hall_click(facts: &Facts) -> Answer {
    if facts.building && facts.building_has_colony && facts.may_open_huts() {
        return Answer::Open {
            window: Window::ColonyOverview,
            shift: facts.sneaking,
        };
    }
    Answer::Ask(Question::ColonyInfo)
}

/// `BlockHutGateHouse.use`, `:56-77`. The off hand is refused first (`:59-62`),
/// then a player who may not open huts gets the connection module instead of a
/// refusal (`:63-68`) — that is what a gate house is for. Anyone else takes the
/// hut path, which re-checks everything.
fn gate_house_click(facts: &Facts) -> Answer {
    if facts.off_hand {
        return Answer::Refused(Refusal::OffHand);
    }
    if facts.building && !facts.may_open_huts() {
        return Answer::Open {
            window: Window::ConnectionModule,
            shift: false,
        };
    }
    colony_click(facts, facts.sneaking)
}

/// `BlockStash.use`, `:87-105`: the inventory, and **nothing at all** when the
/// player may not open it — Java returns `SUCCESS` with no message, so this is
/// not a refusal and there is no sentence to send.
fn stash_click(facts: &Facts) -> Answer {
    if facts.building && facts.building_has_colony && facts.may_open_huts() {
        return Answer::Open {
            window: Window::BuildingInventory,
            shift: false,
        };
    }
    Answer::Pass
}

/// `BlockMinecoloniesRack.use`, `:221-245`: a rack **outside** a colony opens for
/// anyone (`colony == null || ACCESS_HUTS`), and one inside a colony needs the
/// bit.
fn rack_click(block: &Block, facts: &Facts) -> Answer {
    if !facts.block_entity {
        return Answer::Refused(Refusal::NoBlockEntity { block: block.name });
    }
    if !facts.colony_covers || facts.may_open_huts() {
        return Answer::Open {
            window: Window::Rack,
            shift: false,
        };
    }
    Answer::Refused(Refusal::LacksAccess { rank: facts.rank })
}

/// `BlockMinecoloniesGrave.use`, `:145-170`: the same rule as the rack's, with
/// the grave's own screen — a grave outside a colony opens for anyone.
fn grave_click(block: &Block, facts: &Facts) -> Answer {
    if !facts.block_entity {
        return Answer::Refused(Refusal::NoBlockEntity { block: block.name });
    }
    if !facts.colony_covers || facts.may_open_huts() {
        return Answer::Open {
            window: Window::Grave,
            shift: false,
        };
    }
    Answer::Refused(Refusal::LacksAccess { rank: facts.rank })
}

/// The shape the three block-entity screens share: a missing block entity is
/// refused, and the screen then opens with no permission check at all
/// (`BlockPlantationField.java:114-133`, `BlockScarecrow.java:103-115`,
/// `BlockDecorationController.java:154-169`). `off_hand_refused` is Java's own
/// difference between them: the plantation field refuses the off hand
/// (`:118-121`), the scarecrow does not.
fn field_click(block: &Block, facts: &Facts, window: Window, off_hand_refused: bool) -> Answer {
    if off_hand_refused && facts.off_hand {
        return Answer::Refused(Refusal::OffHand);
    }
    if !facts.block_entity {
        return Answer::Refused(Refusal::NoBlockEntity { block: block.name });
    }
    Answer::Open { window, shift: false }
}

/// Which path put the block down. Java has two, and they refuse different things.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two placement paths Java has
pub enum PlacePath {
    /// A block item, `BlockItem.place` → `setPlacedBy`
    /// (`AbstractColonyBlock.java:249-276`). **Refuses nothing.**
    Item,
    /// The build tool's paste, `AbstractBlockHut.setup` → `canPaste`
    /// (`:305-338`). Refuses four things.
    BuildTool,
}

/// What a placement reads.
pub struct Place<'a> {
    /// The placer's yaw in degrees; `None` for a dispenser
    /// (`AbstractColonyBlock.java:237`).
    pub yaw: Option<f32>,
    /// The colony that covers the position, when one does
    /// (`IColonyManager.getColonyByPosFromWorld`).
    pub colony: Option<&'a Colony>,
    /// Whether the placer owns a colony somewhere, which is what tells Java's two
    /// messages apart (`AbstractBlockHut.java:316-324`).
    pub owns_colony: bool,
    /// The placer's effective rank in `colony`.
    pub rank: Rank,
    pub path: PlacePath,
}

/// A placement the rules let through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The facing the block takes, one of [`FACING_VALUES`].
    pub facing: &'static str,
    /// The building it stands for.
    pub building: Option<Kind>,
    /// Whether it is written into a colony: `setPlacedBy` links it when a colony
    /// covers the position, and a hut placed outside one stands **unlinked**
    /// (`AbstractColonyBlock.java:269-274`) — which is what
    /// `crate::hut::KEY_COLONY` means by absent.
    pub linked: bool,
}

/// What placing `block` does.
pub fn placement(block: &Block, place: &Place<'_>) -> Result<Placed, Refusal> {
    let facing = facing_from_yaw(place.yaw);
    match place.path {
        // `setPlacedBy` refuses nothing: the hut stands, and the colony link is
        // written only when a colony is there (`:269-274`).
        PlacePath::Item => Ok(Placed {
            facing,
            building: block.building,
            linked: place.colony.is_some(),
        }),
        PlacePath::BuildTool => build_tool(block, place, facing),
    }
}

/// `AbstractBlockHut.canPaste`, `:305-338`, plus the two `canPlaceAt` overrides
/// (`BlockHutTownHall.java:219-234`, `BlockHutTavern.java:50-65`).
fn build_tool(block: &Block, place: &Place<'_>, facing: &'static str) -> Result<Placed, Refusal> {
    let Some(kind) = block.building else {
        // `setup` and `canPaste` are `AbstractBlockHut`'s: the build tool has
        // nothing to paste for a block with no building.
        return Err(Refusal::NotAColonyBlock { block: block.name });
    };
    let Some(colony) = place.colony else {
        // A town hall is the one hut placed outside a colony
        // (`AbstractBlockHut.java:311-314`).
        if kind.is_town_hall() {
            return Ok(Placed {
                facing,
                building: Some(kind),
                linked: false,
            });
        }
        return Err(Refusal::OutsideColony {
            owns_one: place.owns_colony,
        });
    };
    if !place.rank.allows(Action::PlaceHuts) {
        return Err(Refusal::LacksPlaceHuts { rank: place.rank });
    }
    if kind.is_town_hall() {
        if let Some(at) = first_of(colony, Kind::TownHall) {
            return Err(Refusal::Building(building::Refusal::TownHallAlreadyThere {
                at,
            }));
        }
    }
    if kind == Kind::Tavern {
        if let Some(at) = first_of(colony, Kind::Tavern) {
            return Err(Refusal::TavernAlreadyThere { at });
        }
    }
    Ok(Placed {
        facing,
        building: Some(kind),
        linked: true,
    })
}

/// Where the first building of this kind stands in the colony, if one does. A
/// record whose kind no longer reads is skipped rather than guessed at: it is not
/// a building of that kind, whatever it was.
fn first_of(colony: &Colony, kind: Kind) -> Option<[i32; 3]> {
    colony
        .buildings
        .iter()
        .find(|building| kind_of(&building.kind) == Ok(kind))
        .map(|building| building.at)
}

/// How fast a player breaks `block`, as `getDestroyProgress` answers it.
///
/// `None` means the block does not override it: every block but the 54 colony
/// blocks is that case. The colony blocks answer
/// `(pvp ? 1/(10*4) : 1/10) / 30` (`AbstractColonyBlock.java:105-115`), except the
/// two thin ones, which answer `1/30` flat and ignore the pvp mode
/// (`BlockStash.java:63-67`, `BlockPostBox.java:41-45`), and the town hall, which
/// divides its own answer by 12 again in pvp
/// (`BlockHutTownHall.java:129-130`) — the hardest block in the mod.
pub fn break_progress(block: &Block, pvp: bool) -> Option<f32> {
    if matches!(block.building, Some(Kind::Stash) | Some(Kind::Postbox)) {
        return Some(1.0 / PROGRESS_DIVISOR);
    }
    if block.building.is_none() {
        return None;
    }
    let factor = if pvp { HARDNESS_PVP_FACTOR } else { 1.0 };
    let base = 1.0 / (HUT_HARDNESS * factor) / PROGRESS_DIVISOR;
    if block.building == Some(Kind::TownHall) && pvp {
        return Some(base / TOWN_HALL_PVP_FACTOR);
    }
    Some(base)
}

/// The message `getDestroyProgress` sends while a player breaks a building that
/// has children, or `None`.
///
/// **Java's test is a hold, not a cooldown**: it warns when the last warning is
/// *less* than [`BREAK_WARNING_WINDOW`] ticks old
/// (`AbstractColonyBlock.java:108-112`), so a player who keeps hitting is warned
/// on every hit, and one who pauses for 100 ticks is not warned on the next. The
/// port keeps it as written and a test pins it, so changing it is a decision.
pub fn break_warning(block: &Block, children: bool, since_last_warn: i64) -> Option<&'static str> {
    let warns = block.building.is_some() && children && since_last_warn < BREAK_WARNING_WINDOW;
    warns.then_some(BREAK_WARNING_KEY)
}

/// Every state property the 86 blocks use, once per name, with how many blocks
/// declare it — the inventory the content files share.
///
/// **A name is not a value set.** `facing` is declared by the huts with four
/// values and by the decoration controller with six
/// (`DirectionalBlock`), and `variant` is three different enums (the grave's two,
/// the rack's five, the barrel's eight). The count is per name, and
/// [`Block::states`] is what says which values each block gives it.
pub fn state_inventory() -> Vec<(&'static str, usize)> {
    let mut inventory: Vec<(&'static str, usize)> = Vec::new();
    for block in all() {
        for state in block.states {
            if let Some(slot) = inventory.iter_mut().find(|slot| slot.0 == state.name) {
                slot.1 += 1;
            } else {
                inventory.push((state.name, 1));
            }
        }
    }
    inventory.sort_unstable_by(|a, b| a.0.cmp(b.0));
    inventory
}

/// The values a name is declared with across the 86 blocks, deduplicated: the
/// way to see that `facing` has two spellings and `variant` three.
pub fn value_sets(name: &str) -> Vec<Values> {
    let mut sets: Vec<Values> = Vec::new();
    for block in all() {
        for state in block.states {
            if state.name == name && !sets.contains(&state.values) {
                sets.push(state.values);
            }
        }
    }
    sets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builder() -> &'static Block {
        block("minecolonies:blockhutbuilder").expect("the builder hut is a block")
    }

    fn colony_with(kinds: &[&str]) -> Colony {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], 1);
        for (index, kind) in kinds.iter().enumerate() {
            colony
                .buildings
                .push(crate::colony::Building::new(kind, [index as i32, 70, 0], 1));
        }
        colony
    }

    #[test]
    fn the_table_holds_the_eighty_six_blocks_java_registers() {
        assert_eq!(all().count(), COUNT, "87 calls, one commented out at :151");
        // The families hold the blocks, and the counts add up.
        assert_eq!(
            Family::ALL.iter().map(|f| f.blocks().len()).sum::<usize>(),
            COUNT
        );
        assert_eq!(HUTS.len(), COLONY_COUNT);
        assert_eq!(CHANTIER.len(), 3);
        assert_eq!(DECORATION.len(), 9);
        assert_eq!(RESSOURCE.len(), 20);
        for family in Family::ALL {
            for block in family.blocks() {
                assert_eq!(block.family, family, "{} is filed under {}", block.name, family.name());
            }
        }
    }

    #[test]
    fn no_two_blocks_share_a_name_and_each_keeps_its_class() {
        let mut names: Vec<&str> = all().map(|block| block.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "a save keys a block by its name");
        for block in all() {
            assert!(
                !block.name.contains(':') && !block.name.is_empty(),
                "{}: the path, not the name, carries the namespace",
                block.name
            );
            assert_eq!(block.path(), format!("minecolonies:{}", block.name));
            assert!(
                block.source.ends_with(".java") || block.source.contains(".java:"),
                "{} names no Java line: {}",
                block.name,
                block.source
            );
        }
        // The one block this port already has in content and in code.
        assert_eq!(builder().name, crate::hut::BLOCK_ID.trim_start_matches("minecolonies:"));
        assert_eq!(block(&crate::hut::BLOCK_ID).map(|b| b.name), Some("blockhutbuilder"));
    }

    #[test]
    fn the_fifty_four_kinds_and_the_fifty_four_blocks_are_one_to_one() {
        let mut kinds: Vec<Kind> = all().filter_map(|block| block.building).collect();
        assert_eq!(kinds.len(), COLONY_COUNT, "54 colony blocks");
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), COLONY_COUNT, "two blocks claim the same building");
        for kind in Kind::ALL {
            assert!(
                block_of(*kind).is_some(),
                "{kind} has no block, so a building of that kind has no hut"
            );
        }
        assert_eq!(
            all().filter_map(|block| block.building).count(),
            Kind::ALL.len(),
            "every kind once, and no block twice"
        );
    }

    #[test]
    fn the_five_blocks_whose_name_is_not_their_building_name() {
        // `ModBuildings.HOME_ID = "residence"`, `ModBuildings.java:25`.
        assert_eq!(kind_of("minecolonies:blockhutcitizen"), Ok(Kind::Residence));
        // The two quarries have no `blockhut` prefix: `SimpleQuarry.java:23`.
        assert_eq!(kind_of("minecolonies:simplequarry"), Ok(Kind::SimpleQuarry));
        assert_eq!(kind_of("minecolonies:mediumquarry"), Ok(Kind::MediumQuarry));
        // The two colony blocks that are not huts keep their own prefix.
        assert_eq!(kind_of("minecolonies:blockpostbox"), Ok(Kind::Postbox));
        assert_eq!(kind_of("minecolonies:blockstash"), Ok(Kind::Stash));
        // Both spellings a colony record can hold are read: the block's
        // (`blockhuttownhall`, `BlockHutTownHall.java:159`) and the building's
        // (`townhall`, `ModBuildings.java:37`).
        assert_eq!(kind_of("minecolonies:townhall"), Ok(Kind::TownHall));
        assert_eq!(kind_of("townhall"), Ok(Kind::TownHall));
        assert_eq!(kind_of("minecolonies:blockhuttownhall"), Ok(Kind::TownHall));
        // Including the one place the two names really differ.
        assert_eq!(kind_of("minecolonies:residence"), Ok(Kind::Residence));
        assert_eq!(kind_of("minecolonies:blockhutcitizen"), Ok(Kind::Residence));
        // A name that is neither a block nor a building is still unknown.
        assert_eq!(block("minecolonies:townhall"), None, "no block goes by that name");
        assert_eq!(kind_of("builder"), Ok(Kind::Builder), "the namespace is optional");
    }

    #[test]
    fn a_block_without_a_building_and_a_name_without_a_block_are_two_refusals() {
        assert_eq!(
            kind_of("minecolonies:gate_iron"),
            Err(Refusal::NotAColonyBlock { block: "gate_iron" })
        );
        assert_eq!(
            kind_of("minecolonies:nonesuch"),
            Err(Refusal::UnknownBlock {
                path: "minecolonies:nonesuch".to_owned()
            })
        );
        assert_eq!(block("minecraft:stone"), None, "another mod's namespace is refused");
        assert_eq!(block("stone"), None);
        assert_eq!(block(""), None);
        // The scarecrow's id is the inherited `REGISTRY_NAME`, not its class name.
        assert_eq!(
            block("minecolonies:blockhutfield").map(|b| b.source),
            Some("core/blocks/BlockScarecrow.java:52")
        );
        assert_eq!(block("minecolonies:blockhutscarecrow"), None);
    }

    #[test]
    fn fifty_six_blocks_are_never_broken_by_a_builder() {
        let undestroyable: Vec<&str> = all()
            .filter(|block| !block.builder_may_break())
            .map(|block| block.name)
            .collect();
        assert_eq!(undestroyable.len(), BUILDER_UNDESTROYABLE_COUNT);
        assert_eq!(
            undestroyable.len(),
            COLONY_COUNT + 2,
            "the 54 colony blocks, plus the decoration controller and the plantation field"
        );
        for name in ["minecolonies:decorationcontroller", "minecolonies:blockhutplantationfield"] {
            assert!(!block(name).expect(name).builder_may_break(), "{name}");
        }
        // The tape is the builder's to remove, so it is not on the list.
        assert!(block("minecolonies:blockconstructiontape").unwrap().builder_may_break());
    }

    #[test]
    fn every_java_state_property_lands_in_content_or_says_why_not() {
        let mut declared = 0;
        let mut templated = 0;
        let mut gaps = 0;
        for block in all() {
            for state in block.states {
                match state.content {
                    StateIn::Declared => declared += 1,
                    StateIn::Template(template) => {
                        assert!(!template.is_empty(), "{} {}: no template named", block.name, state.name);
                        templated += 1;
                    }
                    StateIn::Gap(need) => {
                        assert!(!need.is_empty(), "{} {}: no need named", block.name, state.name);
                        gaps += 1;
                    }
                }
                assert!(
                    state.values.allows(state.default),
                    "{} {}: the default {} is not one of its values",
                    block.name,
                    state.name,
                    state.default
                );
                // An integer cannot be declared in content: it comes from the
                // template or it is a gap.
                if matches!(state.values, Values::Integer { .. }) {
                    assert!(
                        !matches!(state.content, StateIn::Declared),
                        "{} {}: content has no integer property",
                        block.name,
                        state.name
                    );
                }
                assert!(!state.source.ends_with(".java") || state.source.contains(".java:"), "{} {}", block.name, state.name);
            }
        }
        assert!(declared > 0 && templated > 0 && gaps > 0, "{declared} {templated} {gaps}");
        // No block of the 86 asks for a tool, and that is one fact, not 86.
        assert!(all().all(|block| !block.props.requires_tool));
    }

    #[test]
    fn the_state_inventory_names_every_property_the_86_blocks_use() {
        let inventory = state_inventory();
        let names: Vec<&str> = inventory.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            vec![
                "age",
                "connected",
                "corner",
                "east",
                "facing",
                "half",
                "hinge",
                "lantern",
                "mirror",
                "moisture",
                "north",
                "open",
                "powered",
                "rotation",
                "south",
                "variant",
                "waterlogged",
                "west",
            ],
            "18 properties, in name order"
        );
        // `facing` is the one every colony block declares, so it is the highest.
        let facing = inventory
            .iter()
            .find(|(name, _)| *name == "facing")
            .map(|(_, count)| *count)
            .expect("facing is used");
        assert!(facing >= COLONY_COUNT, "{facing} blocks declare a facing");
        assert_eq!(
            inventory.iter().map(|(_, count)| count).sum::<usize>(),
            all().map(|block| block.states.len()).sum::<usize>()
        );
        // A name is not a value set: `facing` has two spellings, `variant` three.
        let facings = value_sets("facing");
        assert_eq!(facings.len(), 2);
        assert_eq!(facings[0].names(), &["north", "south", "east", "west"]);
        assert_eq!(
            facings[1].names().len(),
            6,
            "the decoration controller's `DirectionalBlock` facing"
        );
        assert_eq!(value_sets("variant").len(), 3, "grave, rack and barrel");
        assert!(value_sets("nonesuch").is_empty());
    }

    #[test]
    fn the_shape_inventory_says_which_blocks_are_one_box() {
        // The four boxes Java spells in one call, and the two forms that are not.
        assert_eq!(Form::SmallBox.box_of(), Some([0.1, 0.1, 0.1, 0.9, 0.9, 0.9]));
        assert_eq!(Form::TallBox.box_of(), Some([0.1, 0.1, 0.1, 0.9, 1.9, 0.9]));
        assert_eq!(Form::Gate.box_of(), Some([0.3, 0.0, 0.0, 0.7, 1.0, 1.0]));
        assert_eq!(Form::Empty.box_of(), None, "`collision_shapes.default` is `[]`");
        for form in [
            Form::HalfBox,
            Form::Full,
            Form::Banner,
            Form::Crop,
            Form::Tape,
            Form::Plate,
            Form::TwoTall,
            Form::Field,
            Form::Rack,
        ] {
            assert_eq!(form.box_of(), None, "{form:?} is spelled per state");
        }
        // Every form a block uses is one of the thirteen, and each is used.
        for form in [
            Form::SmallBox,
            Form::TallBox,
            Form::HalfBox,
            Form::Full,
            Form::Gate,
            Form::Banner,
            Form::Crop,
            Form::Tape,
            Form::Plate,
            Form::TwoTall,
            Form::Field,
            Form::Rack,
            Form::Empty,
        ] {
            assert!(
                all().any(|block| block.form == form),
                "{form:?} is declared and used by no block"
            );
        }
    }

    #[test]
    fn a_hut_faces_the_way_its_placer_looks() {
        // `Direction.fromYRot`: 0 is south, 90 west, 180 north, -90 east.
        assert_eq!(facing_from_yaw(Some(0.0)), "south");
        assert_eq!(facing_from_yaw(Some(90.0)), "west");
        assert_eq!(facing_from_yaw(Some(180.0)), "north");
        assert_eq!(facing_from_yaw(Some(-90.0)), "east");
        // The rounding is vanilla's: the quarter turn nearest the yaw wins.
        assert_eq!(facing_from_yaw(Some(44.0)), "south");
        assert_eq!(facing_from_yaw(Some(46.0)), "west");
        // Any full turn comes back to the same facing.
        assert_eq!(facing_from_yaw(Some(360.0)), facing_from_yaw(Some(0.0)));
        // A dispenser has no yaw, and Java answers north.
        assert_eq!(facing_from_yaw(None), "north");
        assert!(FACING_VALUES.contains(&facing_from_yaw(Some(123.0))));
    }

    #[test]
    fn a_hut_placed_by_an_item_stands_even_outside_a_colony() {
        let place = Place {
            yaw: Some(180.0),
            colony: None,
            owns_colony: false,
            rank: Rank::Neutral,
            path: PlacePath::Item,
        };
        let placed = placement(builder(), &place).expect("an item refuses nothing");
        assert_eq!(placed.facing, "north");
        assert_eq!(placed.building, Some(Kind::Builder));
        assert!(!placed.linked, "no colony covers the position, so nothing is written");
    }

    #[test]
    fn the_build_tool_refuses_what_the_item_lets_through() {
        let hut = builder();
        let outside = Place {
            yaw: None,
            colony: None,
            owns_colony: false,
            rank: Rank::Owner,
            path: PlacePath::BuildTool,
        };
        assert_eq!(
            placement(hut, &outside),
            Err(Refusal::OutsideColony { owns_one: false }),
            "MESSAGE_WARNING_TOWN_HALL_NOT_PRESENT"
        );
        let elsewhere = Place {
            owns_colony: true,
            ..outside
        };
        assert_eq!(
            placement(hut, &elsewhere),
            Err(Refusal::OutsideColony { owns_one: true }),
            "MESSAGE_WARNING_TOWN_HALL_TOO_FAR_AWAY"
        );

        // A town hall is the one hut the build tool places outside a colony.
        let hall = block("minecolonies:blockhuttownhall").unwrap();
        let placed = placement(hall, &outside).expect("a town hall may be placed anywhere");
        assert_eq!(placed.building, Some(Kind::TownHall));
        assert!(!placed.linked);

        // A block that is not a colony block has nothing to paste.
        let gate = block("minecolonies:gate_iron").unwrap();
        assert_eq!(
            placement(gate, &outside),
            Err(Refusal::NotAColonyBlock { block: "gate_iron" })
        );
    }

    #[test]
    fn a_member_places_a_hut_inside_his_colony_and_a_stranger_does_not() {
        let colony = colony_with(&["minecolonies:blockhuttownhall"]);
        let mut place = Place {
            yaw: Some(-90.0),
            colony: Some(&colony),
            owns_colony: true,
            rank: Rank::Officer,
            path: PlacePath::BuildTool,
        };
        let placed = placement(builder(), &place).expect("an officer may place huts");
        assert_eq!(placed.facing, "east");
        assert!(placed.linked);

        place.rank = Rank::Neutral;
        assert_eq!(
            placement(builder(), &place),
            Err(Refusal::LacksPlaceHuts {
                rank: Rank::Neutral
            })
        );
    }

    #[test]
    fn one_town_hall_and_one_tavern_per_colony() {
        let colony = colony_with(&["minecolonies:blockhuttownhall", "minecolonies:blockhuttavern"]);
        let place = Place {
            yaw: None,
            colony: Some(&colony),
            owns_colony: true,
            rank: Rank::Owner,
            path: PlacePath::BuildTool,
        };
        // The town hall's refusal is the `building` module's own name, reused.
        assert_eq!(
            placement(block("minecolonies:blockhuttownhall").unwrap(), &place),
            Err(Refusal::Building(building::Refusal::TownHallAlreadyThere {
                at: [0, 70, 0]
            }))
        );
        assert_eq!(
            placement(block("minecolonies:blockhuttavern").unwrap(), &place),
            Err(Refusal::TavernAlreadyThere { at: [1, 70, 0] })
        );
        // Any other hut is fine, twice.
        assert!(placement(builder(), &place).is_ok());
    }

    #[test]
    fn a_break_is_a_number_and_the_town_hall_is_the_slowest() {
        let hut = break_progress(builder(), false).expect("a hut overrides getDestroyProgress");
        let pvp = break_progress(builder(), true).expect("pvp is still a break");
        assert!(
            (hut / pvp - HARDNESS_PVP_FACTOR).abs() < 1e-6,
            "the pvp mode divides by four: {hut} {pvp}"
        );
        // 1/10/30, exactly.
        assert!((hut - 1.0 / 300.0).abs() < 1e-9);

        let hall = block("minecolonies:blockhuttownhall").unwrap();
        let hall_pvp = break_progress(hall, true).expect("a town hall breaks slowly");
        assert!(
            (pvp / hall_pvp - TOWN_HALL_PVP_FACTOR).abs() < 1e-4,
            "twelve times the hut's pvp progress"
        );

        // The stash and the post box answer 1/30 flat, pvp or not.
        for name in ["minecolonies:blockstash", "minecolonies:blockpostbox"] {
            let block = block(name).unwrap();
            assert_eq!(break_progress(block, false), Some(1.0 / 30.0));
            assert_eq!(break_progress(block, true), Some(1.0 / 30.0));
            assert!(break_progress(block, false).unwrap() > hut, "1/30 breaks faster than 1/300");
        }

        // Everything that is not a colony block leaves it to vanilla.
        assert_eq!(break_progress(block("minecolonies:gate_iron").unwrap(), false), None);
        assert_eq!(block("minecolonies:farmland").map(|b| break_progress(b, false)), Some(None));
    }

    #[test]
    fn the_child_warning_is_java_s_hold_and_not_a_cooldown() {
        let hut = builder();
        assert_eq!(
            break_warning(hut, true, 0),
            Some(BREAK_WARNING_KEY),
            "the last warning is fresh, so Java warns again"
        );
        assert_eq!(break_warning(hut, true, 99), Some(BREAK_WARNING_KEY));
        assert_eq!(
            break_warning(hut, true, BREAK_WARNING_WINDOW),
            None,
            "a 100-tick pause silences it — the condition is `< 100`"
        );
        assert_eq!(break_warning(hut, false, 0), None, "no children, no message");
        assert_eq!(
            break_warning(block("minecolonies:gate_iron").unwrap(), true, 0),
            None,
            "only a colony block has a building to warn about"
        );
    }

    #[test]
    fn the_contents_drop_only_where_java_drops_them() {
        let with_contents: Vec<&str> = all()
            .filter(|block| block.contents)
            .map(|block| block.name)
            .collect();
        assert!(with_contents.len() >= COLONY_COUNT, "the 54 colony blocks at least");
        for name in [
            "minecolonies:blockminecoloniesrack",
            "minecolonies:blockminecoloniesgrave",
        ] {
            assert!(block(name).unwrap().contents, "{name} drops its inventory");
        }
        // A banner, a sign and a tape hold no inventory.
        for name in [
            "minecolonies:colony_banner",
            "minecolonies:colonysign",
            "minecolonies:blockconstructiontape",
        ] {
            assert!(!block(name).unwrap().contents, "{name}");
        }
    }

    #[test]
    fn every_answer_screen_belongs_to_a_block_that_declares_it() {
        let mut windows: Vec<Window> = Vec::new();
        for block in all() {
            match click(block, &Facts::member()) {
                Answer::Open { window, .. } => windows.push(window),
                Answer::Ask(_)
                | Answer::Consume { .. }
                | Answer::Toggle { .. }
                | Answer::Insert
                | Answer::Pass => {}
                Answer::Refused(refusal) => {
                    panic!("a member is refused {}: {refusal}", block.name)
                }
            }
        }
        assert!(windows.contains(&Window::Hut));
        assert!(windows.contains(&Window::ColonyOverview));
        // 62 blocks answer a click: the 54 colony blocks and eight others. The
        // other 24 leave it to the engine.
        assert_eq!(
            all().filter(|block| block.click == Click::None).count(),
            COUNT - COLONY_COUNT - 8,
            "the rack, the grave, the barrel, the two gates, the plantation field, \
             the scarecrow and the decoration controller"
        );
    }
}
