//! The three tags an extension is saved under, the fourteen kinds, and what the
//! two field payloads hold.
//!
//! Java reference: `core/colony/buildingextensions/registry/BuildingExtensionDataManager.java`
//! (111 lines), the two classes it produces — `core/colony/buildingextensions/FarmField.java`
//! (288) and `core/colony/buildingextensions/PlantationField.java` (127) — their
//! base `core/colony/buildingextensions/AbstractBuildingExtensionModule.java` (211),
//! the registry `api/colony/buildingextensions/registry/BuildingExtensionRegistries.java`
//! and the initialiser that fills it, `apiimp/initializer/ModBuildingExtensionsInitializer.java:23-71`.
//!
//! **The frame is three keys, and the payload is inside the third.**
//! `extensionToCompound` writes `name` (the registry id), `position`, then `data`
//! — which is the extension's *own* `serializeNBT`, where the owner lives
//! (`BuildingExtensionDataManager.java:103-110`, `AbstractBuildingExtensionModule.java:132-138`).
//! So an owner saved at the top level is not an owner, and [`Extension::to_compound`]
//! has no way to produce one: the test holds both the count and the nesting.
//!
//! **Produce, then fill — and a failure in the fill costs the extension, not half
//! of it.** `compoundToExtension` reads the name and the position, produces the
//! instance, and only then calls `deserializeNBT` on it (`:36-44`). Three of those
//! deserialisers can throw: `FarmField`'s `Stage.valueOf` on an unknown or missing
//! stage (`FarmField.java:119`), and `ItemStack.of` on a seed compound without an
//! id, which answers `EMPTY` instead of throwing. **Nothing catches any of it**,
//! unlike `BuildingDataManager`'s `try`/`catch` (`:40-51`): the throw leaves
//! `compoundToExtension` and reaches whoever was loading the module. The port reads
//! the whole payload into values **before** it builds an [`Extension`], so the same
//! failure produces no value at all and a named refusal.
//!
//! **Two numbers with a default and no floor.** A field's radius is
//! `Math.min(radius, MAX_RANGE)` (`FarmField.java:40`, `:209-212`): twenty at most
//! and **no lower bound**, so a negative radius is kept. The default is five in
//! each of four directions (`:41`, `:58`), and the four are in
//! `Direction.get2DDataValue` order — south, west, north, east — which is what
//! [`Side`] is.
//!
//! **A missing `radius` array is an empty array, and Java then throws.** Java reads
//! it with `getIntArray` (`:118`), which answers `int[0]` for an absent or
//! wrongly-typed tag, and `getRadius` then indexes `radii[direction]`
//! (`:200-203`) — an `ArrayIndexOutOfBoundsException` the next time a farmer asks
//! for a direction. The port keeps the array as a `Vec`, answers `None` from
//! [`FarmField::radius`], and names the refusal from [`FarmField::set_radius`];
//! a save with a four-int array behaves exactly as Java's.
//!
//! **The seed is an `ItemStack`, and the port keeps one field of it.** Java writes
//! `seed.serializeNBT()` (`:107`) — vanilla's `id`, `Count`, and an optional `tag`
//! — and reads it with `ItemStack.of` (`:117`). A seed on a field is one item
//! (`setSeed` copies and forces `Count` to 1, `:157-161`, and `getSeed` forces it
//! again on the way out, `:146-150`), so the port stores [`ItemId`], whose
//! `{…}` NBT suffix already covers the third key (`src/crafting/ingredient.rs:38-54`).
//! The `Count` is written as 1 and is not read: Java's own getter overrides it.
//!
//! **What is not here**: the network form (`bufferToExtension`/`extensionToBuffer`,
//! `:73-95`), which writes a *registry index* a mod cannot reproduce —
//! `MANQUANT.md` §3 — and `isValidPlacement` (`FarmField.java:97-104`,
//! `PlantationField.java:53-60`), which asks the world which block stands at the
//! position, so it belongs to a caller with a world.

use crate::blueprint::nbt::{as_text, tag};
use crate::crafting::ItemId;
use mod_api::{Compound, Identifier, Tag};

use super::nbt;

/// `BuildingExtensionDataManager.TAG_EXTENSION_NAME` (`:20`): the registry id the
/// extension is an instance of.
pub const TAG_NAME: &str = "name";
/// `TAG_EXTENSION_POSITION` (`:21`).
pub const TAG_POSITION: &str = "position";
/// `TAG_EXTENSION_DATA` (`:22`): the extension's own compound.
pub const TAG_DATA: &str = "data";
/// `NbtTagConstants.TAG_OWNER` (`api/util/constant/NbtTagConstants.java:284`),
/// written inside the payload by `AbstractBuildingExtensionModule.java:132-138`.
pub const TAG_OWNER: &str = "owner";
/// The field's own keys (`FarmField.java:43-46`).
pub const TAG_SEED: &str = "seed";
pub const TAG_RADII: &str = "radius";
pub const TAG_STAGE: &str = "stage";
/// The plantation's own key (`PlantationField.java:23`).
pub const TAG_WORKING_POSITIONS: &str = "workingPositions";
/// The seed stack's two keys, vanilla's `ItemStack.save`.
pub const TAG_SEED_ID: &str = "id";
pub const TAG_SEED_COUNT: &str = "Count";

/// The widest a field may be set to (`FarmField.java:40`).
pub const MAX_RANGE: i32 = 20;
/// The radius a field starts with, in each direction (`FarmField.java:41`).
pub const DEFAULT_RANGE: i32 = 5;

/// Which of the two classes an entry produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two producers in the initialiser, `FarmField::new` and `PlantationField::new`
pub enum Field {
    /// `FarmField` — the scarecrow (`ModBuildingExtensionsInitializer.java:23`).
    Farm,
    /// `PlantationField` — one per crop kind (`:25-71`).
    Plantation,
}

/// One entry of the `minecolonies:buildingextensions` registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed registry, like `crate::building::Kind`
pub struct Kind {
    /// The registry path without its namespace, as `BuildingExtensionRegistries`
    /// declares it (`:24-37`).
    pub path: &'static str,
    /// The class `produceExtension` builds, and therefore the payload the `data`
    /// compound holds.
    pub field: Field,
}

impl Kind {
    /// The full id, `minecolonies:farmfield`, which is what `name` holds
    /// (`BuildingExtensionDataManager.java:106`).
    pub fn identifier(&self) -> Identifier {
        Identifier::from_parts(super::NAMESPACE, self.path)
            .expect("the fourteen paths below are lower-case ASCII, which the engine accepts")
    }

    /// The entry a full id names, or `None` — Java's registry `getValue`
    /// (`:56`), which answers `null` for a foreign namespace too.
    pub fn from_identifier(id: &Identifier) -> Option<&'static Kind> {
        if id.namespace() != super::NAMESPACE {
            return None;
        }
        Kind::from_path(id.path())
    }

    /// The entry a bare path names, or `None`.
    pub fn from_path(path: &str) -> Option<&'static Kind> {
        EXTENSIONS.iter().find(|kind| kind.path == path)
    }
}

/// The registry, in the order `ModBuildingExtensionsInitializer`'s static block
/// registers it (`:23-71`) — the order the network's registry indices follow.
pub const EXTENSIONS: &[Kind] = &[
    // `BuildingExtensionRegistries.java:24`, `ModBuildingExtensionsInitializer.java:23`
    Kind { path: "farmfield", field: Field::Farm },
    // `:25`, `:25-27`
    Kind { path: "plantation_sugar_cane", field: Field::Plantation },
    // `:26`, `:29-30`
    Kind { path: "plantation_cactus", field: Field::Plantation },
    // `:27`, `:32-33`
    Kind { path: "plantation_bamboo", field: Field::Plantation },
    // `:28`, `:35-37`
    Kind { path: "plantation_cocoa_beans", field: Field::Plantation },
    // `:29`, `:39-40`
    Kind { path: "plantation_vines", field: Field::Plantation },
    // `:30`, `:42-43`
    Kind { path: "plantation_kelp", field: Field::Plantation },
    // `:31`, `:45-47`
    Kind { path: "plantation_seagrass", field: Field::Plantation },
    // `:32`, `:49-51`
    Kind { path: "plantation_sea_pickles", field: Field::Plantation },
    // `:33`, `:53-55`
    Kind { path: "plantation_glowberries", field: Field::Plantation },
    // `:34`, `:57-59`
    Kind { path: "plantation_weeping_vines", field: Field::Plantation },
    // `:35`, `:61-63`
    Kind { path: "plantation_twisting_vines", field: Field::Plantation },
    // `:36`, `:65-67`
    Kind { path: "plantation_crimson_plants", field: Field::Plantation },
    // `:37`, `:69-71`
    Kind { path: "plantation_warped_plants", field: Field::Plantation },
];

/// The four horizontal directions, in `Direction.get2DDataValue` order:
/// south 0, west 1, north 2, east 3 — the order `FarmField.radii` is read in
/// (`FarmField.java:56-58`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the four a field has a radius for; a vertical one has none
pub enum Side {
    South,
    West,
    North,
    East,
}

impl Side {
    /// Every side, in the array's order.
    pub const ALL: [Side; 4] = [Side::South, Side::West, Side::North, Side::East];

    /// The index into `radii` (`Direction.get2DDataValue`).
    pub fn index(self) -> usize {
        match self {
            Side::South => 0,
            Side::West => 1,
            Side::North => 2,
            Side::East => 3,
        }
    }
}

/// The three stages a farm field moves through (`FarmField.Stage`, `:241-247`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: three constants in Java, in declaration order
pub enum Stage {
    Empty,
    Hoed,
    Planted,
}

impl Stage {
    /// Every stage, in `Stage.values()` order — which is the cycle [`Self::next`]
    /// walks.
    pub const ALL: [Stage; 3] = [Stage::Empty, Stage::Hoed, Stage::Planted];

    /// The constant's name, which is what `serializeNBT` writes (`:109`) and what
    /// `valueOf` reads back.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Empty => "EMPTY",
            Stage::Hoed => "HOED",
            Stage::Planted => "PLANTED",
        }
    }

    /// `Stage.valueOf(text)` (`:119`), as a refusal instead of an
    /// `IllegalArgumentException`. **Case matters**: `valueOf` is exact.
    pub fn parse(text: &str) -> Result<Stage, Refusal> {
        Stage::ALL
            .into_iter()
            .find(|stage| stage.name() == text)
            .ok_or_else(|| Refusal::StageUnknown {
                found: text.to_owned(),
            })
    }

    /// The stage after this one, wrapping to the first (`:166-175`, `:279-287`):
    /// the last stage cycles, it does not stick.
    pub fn next(self) -> Stage {
        let position = self as usize;
        Stage::ALL
            .get(position + 1)
            .copied()
            .unwrap_or(Stage::ALL[0])
    }

    /// The icon the field's window shows for this stage (`:242-244`):
    /// `("minecraft", "textures/item/iron_hoe.png")` for an empty field,
    /// `("minecraft", "textures/item/wheat_seeds.png")` for a hoed one, and the
    /// mod's own durum sprite for a planted one. The `.png` is part of the path
    /// Java's constant holds, so it is kept.
    pub fn icon(self) -> (&'static str, &'static str) {
        match self {
            Stage::Empty => ("minecraft", "textures/item/iron_hoe.png"),
            Stage::Hoed => ("minecraft", "textures/item/wheat_seeds.png"),
            Stage::Planted => ("minecolonies", "textures/item/crops/durum.png"),
        }
    }
}

/// The scarecrow's payload: one seed, four radii, one stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmField {
    /// Java's `seed`, `ItemStack.EMPTY` when the field has none (`:51`).
    pub seed: Option<ItemId>,
    /// Java's `int[] radii` (`:58`), kept as a `Vec` because a save can hold
    /// fewer than four — see the module note.
    pub radii: Vec<i32>,
    pub stage: Stage,
}

impl Default for FarmField {
    /// A field nobody has touched: no seed, [`DEFAULT_RANGE`] in each of the four
    /// directions, [`Stage::Empty`] (`FarmField.java:51-63`).
    fn default() -> Self {
        Self {
            seed: None,
            radii: vec![DEFAULT_RANGE; 4],
            stage: Stage::Empty,
        }
    }
}

impl FarmField {
    /// The seed, or `None`. Java's getter forces the count to 1 on the way out
    /// (`:146-150`), which is a write the port does not need: there is no count.
    pub fn seed(&self) -> Option<&ItemId> {
        self.seed.as_ref()
    }

    /// `setSeed` (`:155-159`): copy, and hold exactly one.
    pub fn set_seed(&mut self, seed: Option<ItemId>) {
        self.seed = seed;
    }

    /// The radius in one direction, or `None` when the saved array is short
    /// (Java: an `ArrayIndexOutOfBoundsException` at `:200-203`).
    pub fn radius(&self, side: Side) -> Option<i32> {
        self.radii.get(side.index()).copied()
    }

    /// `setRadius` (`:209-212`): **twenty at most, and no floor** — `Math.min`
    /// keeps a negative radius, which the test holds. Refused when the array has
    /// no such direction, which Java answers by throwing.
    pub fn set_radius(&mut self, side: Side, radius: i32) -> Result<(), Refusal> {
        let index = side.index();
        if index >= self.radii.len() {
            return Err(Refusal::RadiusMissing {
                side,
                count: self.radii.len(),
            });
        }
        self.radii[index] = radius.min(MAX_RANGE);
        Ok(())
    }

    /// `nextState` (`:166-175`): one step, and the last loops back to `EMPTY`.
    pub fn next_stage(&mut self) {
        self.stage = self.stage.next();
    }

    /// The two payloads out of the `data` compound: the seed (`:117`), the radii
    /// (`:118`) and the stage (`:119`), each read **before** anything is built.
    fn read(data: &Compound) -> Result<Self, Refusal> {
        let seed = read_seed(data)?;
        // `getIntArray` on an absent tag is `int[0]`: the field keeps the empty
        // array and every direction answers `None` until someone rewrites it.
        let radii = nbt::ints_in(data, TAG_RADII).unwrap_or_default();
        let stage = Stage::parse(tag(data, TAG_STAGE).and_then(as_text).unwrap_or_default().as_str())?;
        Ok(Self { seed, radii, stage })
    }

    /// The payload compound, in `serializeNBT`'s order: the base class writes the
    /// owner first (`:132-138`), then the seed, the radii and the stage
    /// (`FarmField.java:105-110`).
    fn write(&self, data: &mut Compound) {
        let mut seed = Compound::new();
        if let Some(seed_id) = &self.seed {
            seed.insert(TAG_SEED_ID.into(), Tag::String(seed_id.to_string().as_str().into()));
            // Vanilla's byte, and the value `getSeed`/`setSeed` force anyway.
            seed.insert(TAG_SEED_COUNT.into(), Tag::Byte(1));
        }
        data.insert(TAG_SEED.into(), Tag::Compound(seed));
        data.insert(TAG_RADII.into(), nbt::int_array(&self.radii));
        data.insert(
            TAG_STAGE.into(),
            Tag::String(self.stage.name().into()),
        );
    }
}

/// `ItemStack.of(compound.getCompound(TAG_SEED))` (`FarmField.java:117`), with the
/// one difference the module note gives: an id the engine cannot parse is a named
/// refusal here, where vanilla answers `ItemStack.EMPTY` in silence.
fn read_seed(data: &Compound) -> Result<Option<ItemId>, Refusal> {
    let Some(Tag::Compound(seed)) = tag(data, TAG_SEED) else {
        return Ok(None);
    };
    let Some(text) = tag(seed, TAG_SEED_ID).and_then(as_text) else {
        // No `id`: vanilla's `ItemStack.of` answers `EMPTY`, and so does this.
        return Ok(None);
    };
    let id = ItemId::parse(TAG_SEED_ID, &text).map_err(|_| Refusal::SeedUnreadable { found: text })?;
    Ok(Some(id))
}

/// The plantation's payload: the tagged working positions of one field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlantationField {
    /// Java's `workingPositions` (`PlantationField.java:28`), in file order.
    pub working: Vec<[i32; 3]>,
}

impl PlantationField {
    fn read(data: &Compound) -> Self {
        // `BlockPosUtil.readPosListFromNBT` (`:251-262`): an absent list is empty.
        Self {
            working: nbt::positions_in(data, TAG_WORKING_POSITIONS),
        }
    }

    fn write(&self, data: &mut Compound) {
        data.insert(
            TAG_WORKING_POSITIONS.into(),
            nbt::positions(&self.working),
        );
    }
}

/// What an extension holds, decided by its kind and by nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two producers, one entry each
pub enum Payload {
    Farm(FarmField),
    Plantation(PlantationField),
}

/// One building extension: its kind, where it stands, which building owns it, and
/// its payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    /// The registry entry — a `&'static` table row, as Java holds the entry
    /// object (`IBuildingExtension.getBuildingExtensionType`).
    pub kind: &'static Kind,
    /// Where it stands (`:37`).
    pub at: [i32; 3],
    /// The building that owns it, or `None` — Java's `buildingId`, `null` until
    /// `setBuilding` is called (`AbstractBuildingExtensionModule.java:40-41`).
    pub owner: Option<[i32; 3]>,
    pub payload: Payload,
}

impl Extension {
    /// Java's `compoundToExtension` (`:34-45`).
    ///
    /// Java's order, and the refusals in it: the name and the position are read
    /// first (`:36-37`), the registry is asked before anything is produced
    /// (`:39`, `:56-62`), and the payload is read last. **Every read happens
    /// before the value exists**, so a bad stage or a bad seed produces no
    /// extension rather than a half-filled one.
    pub fn from_compound(data: &Compound) -> Result<Extension, Refusal> {
        let name = tag(data, TAG_NAME)
            .and_then(as_text)
            .ok_or(Refusal::NameMissing)?;
        if name.is_empty() {
            // `new ResourceLocation("")` throws (`:36`) where the engine's parser
            // accepts an empty path; the empty case is refused here.
            return Err(Refusal::NameInvalid { found: name });
        }
        let id = Identifier::parse(&name).map_err(|_| Refusal::NameInvalid {
            found: name.clone(),
        })?;
        let kind = Kind::from_identifier(&id).ok_or(Refusal::UnknownType { found: name })?;
        let at = nbt::position_in(data, TAG_POSITION);

        // `getCompound` answers an empty compound for an absent or mistyped key
        // (`:42`), so an extension with no payload reads its defaults.
        let empty = Compound::new();
        let payload_data: &Compound = match tag(data, TAG_DATA) {
            Some(Tag::Compound(inner)) => inner,
            _ => &empty,
        };
        // `deserializeNBT` tests `compound.contains(TAG_OWNER)` and otherwise
        // sets `null` (`AbstractBuildingExtensionModule.java:152-162`).
        let owner = match tag(payload_data, TAG_OWNER) {
            None => None,
            Some(found @ Tag::Compound(_)) => {
                Some(nbt::position_of(found).ok_or(Refusal::OwnerNotAPosition)?)
            }
            Some(_) => return Err(Refusal::OwnerNotAPosition),
        };
        let payload = match kind.field {
            Field::Farm => Payload::Farm(FarmField::read(payload_data)?),
            Field::Plantation => Payload::Plantation(PlantationField::read(payload_data)),
        };
        Ok(Extension {
            kind,
            at,
            owner,
            payload,
        })
    }

    /// Java's `extensionToCompound` (`:103-110`): exactly three keys, the payload
    /// under `data` and the owner inside it.
    pub fn to_compound(&self) -> Compound {
        let mut payload = Compound::new();
        if let Some(owner) = self.owner {
            payload.insert(TAG_OWNER.into(), nbt::position(owner));
        }
        match &self.payload {
            Payload::Farm(farm) => farm.write(&mut payload),
            Payload::Plantation(plantation) => plantation.write(&mut payload),
        }
        Compound::from([
            (
                TAG_NAME.into(),
                Tag::String(self.kind.identifier().to_string().as_str().into()),
            ),
            (TAG_POSITION.into(), nbt::position(self.at)),
            (TAG_DATA.into(), Tag::Compound(payload)),
        ])
    }

    /// `isTaken` (`AbstractBuildingExtensionModule.java:118-123`): owned or free.
    pub fn is_taken(&self) -> bool {
        self.owner.is_some()
    }

    /// `getSqDistance` (`:126-129`) — which is **not** a squared distance:
    /// `(int) Math.sqrt(BlockPosUtil.getDistanceSquared(position, other))`, so the
    /// Euclidean distance floored. `(0,0,0)` to `(3,4,0)` is 5, and to `(1,1,0)`
    /// is 1, not 2.
    pub fn distance_to(&self, at: [i32; 3]) -> i32 {
        let dx = i64::from(self.at[0]) - i64::from(at[0]);
        let dy = i64::from(self.at[1]) - i64::from(at[1]);
        let dz = i64::from(self.at[2]) - i64::from(at[2]);
        let squared = dx * dx + dy * dy + dz * dz;
        // `BlockPosUtil.getDistanceSquared` is a Java `double` product, and the
        // cast to `int` truncates, which for a non-negative distance is `floor`.
        (squared as f64).sqrt() as i32
    }
}

/// Why an extension could not be produced, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five ways `compoundToExtension` fails, plus the two payload refusals
pub enum Refusal {
    /// No `name` string (`:36` reads it with `getString`, which would answer `""`
    /// and then throw inside `new ResourceLocation`).
    NameMissing,
    /// A `name` that is not a resource location (`:36`).
    NameInvalid { found: String },
    /// A well-formed id no entry of `minecolonies:buildingextensions` holds —
    /// Java logs it and answers `null` (`:58-62`).
    UnknownType { found: String },
    /// The stage a field was saved with is not one of the three constants
    /// (`FarmField.java:119` throws `IllegalArgumentException`).
    StageUnknown { found: String },
    /// A radius for a direction the saved array does not have
    /// (`FarmField.java:209-212` throws `IndexOutOfBoundsException`).
    RadiusMissing { side: Side, count: usize },
    /// A seed id the engine cannot parse (`:117` silently answers `EMPTY`).
    SeedUnreadable { found: String },
    /// An `owner` tag that is not a position (`AbstractBuildingExtensionModule.java:154`
    /// would read `(0,0,0)` and call the extension owned by the origin).
    OwnerNotAPosition,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse_namespaced(text).expect("a literal id parses")
    }

    fn extension(kind: &'static Kind, payload: Payload) -> Extension {
        Extension {
            kind,
            at: [10, 64, -3],
            owner: None,
            payload,
        }
    }

    fn farm() -> FarmField {
        FarmField::default()
    }

    #[test]
    fn the_registry_holds_fourteen_ids_and_one_of_them_is_a_scarecrow() {
        // `BuildingExtensionRegistries.java:24-37`: fourteen `ResourceLocation`
        // constants, all registered (`ModBuildingExtensionsInitializer.java:23-71`).
        assert_eq!(EXTENSIONS.len(), 14);
        assert_eq!(
            EXTENSIONS.iter().filter(|kind| kind.field == Field::Farm).count(),
            1
        );
        assert_eq!(Kind::from_path("farmfield").map(|kind| kind.field), Some(Field::Farm));
        assert_eq!(Kind::from_path("plantation_kelp").map(|kind| kind.field), Some(Field::Plantation));
        assert_eq!(Kind::from_path("plantation_unicorn"), None);
        // The namespace is part of the key, as it is for a forge registry.
        assert_eq!(Kind::from_identifier(&id("minecolonies:farmfield")).map(|k| k.path), Some("farmfield"));
        assert_eq!(Kind::from_identifier(&id("mystery:farmfield")), None);
    }

    #[test]
    fn a_scarecrow_round_trips_with_its_three_keys_and_nothing_more() {
        let mut field = farm();
        field.set_seed(Some(ItemId::parse("seed", "minecraft:wheat_seeds").expect("a seed")));
        field.set_radius(Side::North, 9).expect("four directions");
        let mut ext = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(field));
        ext.owner = Some([1, 2, 3]);

        let data = ext.to_compound();
        // `:103-110`: three keys, and the owner is inside the third — a top-level
        // `owner` would not be one.
        assert_eq!(data.len(), 3);
        assert!(!data.contains_key(&TAG_OWNER.into()));
        let Some(Tag::Compound(payload)) = tag(&data, TAG_DATA) else {
            panic!("data is a compound");
        };
        assert_eq!(nbt::position_of(tag(payload, TAG_OWNER).expect("owner")).unwrap(), [1, 2, 3]);

        let read = Extension::from_compound(&data).expect("the same extension");
        assert_eq!(read, ext);
    }

    #[test]
    fn a_plantation_keeps_its_working_positions_in_order() {
        let mut field = PlantationField::default();
        field.working = vec![[1, 70, 2], [-3, 70, 4]];
        let extension = extension(
            Kind::from_path("plantation_sugar_cane").expect("sugar cane"),
            Payload::Plantation(field),
        );
        let read = Extension::from_compound(&extension.to_compound()).expect("the same extension");
        assert_eq!(read, extension);
        assert_eq!(read.at, [10, 64, -3]);
    }

    #[test]
    fn the_frame_writes_the_registry_id_and_not_the_bare_path() {
        // `:106-108`: the name is the *registry id*, not the bare path.
        let data = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(farm())).to_compound();
        assert_eq!(
            tag(&data, TAG_NAME).and_then(as_text).as_deref(),
            Some("minecolonies:farmfield")
        );
        assert_eq!(nbt::position_in(&data, TAG_POSITION), [10, 64, -3]);
    }

    #[test]
    fn an_unknown_name_is_refused_and_no_extension_is_produced() {
        // `:56-62`: the registry answers null, the manager logs and returns null.
        let data = Compound::from([
            (TAG_NAME.into(), Tag::String("minecolonies:plantation_unicorn".into())),
            (TAG_POSITION.into(), nbt::position([0, 64, 0])),
        ]);
        assert_eq!(
            Extension::from_compound(&data),
            Err(Refusal::UnknownType {
                found: "minecolonies:plantation_unicorn".to_owned()
            })
        );
    }

    #[test]
    fn a_missing_or_unparsable_name_is_named_apart_from_an_unknown_one() {
        let none = Compound::from([(TAG_POSITION.into(), nbt::position([0, 64, 0]))]);
        assert_eq!(Extension::from_compound(&none), Err(Refusal::NameMissing));
        let empty = Compound::from([(TAG_NAME.into(), Tag::String("".into()))]);
        assert_eq!(
            Extension::from_compound(&empty),
            Err(Refusal::NameInvalid {
                found: String::new()
            })
        );
        let shouty = Compound::from([(TAG_NAME.into(), Tag::String("Minecolonies:FarmField".into()))]);
        assert_eq!(
            Extension::from_compound(&shouty),
            Err(Refusal::NameInvalid {
                found: "Minecolonies:FarmField".to_owned()
            })
        );
    }

    #[test]
    fn a_field_with_no_stage_is_refused_because_java_throws_on_value_of_of_the_empty_string() {
        // `FarmField.java:117-120`: `getString` on an absent key is `""`, and
        // `Stage.valueOf("")` throws — nothing catches it.
        let data = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(farm())).to_compound();
        let mut payload = match tag(&data, TAG_DATA).expect("data") {
            Tag::Compound(payload) => payload.clone(),
            _ => panic!("data is a compound"),
        };
        payload.insert(TAG_STAGE.into(), Tag::String("HARVESTED".into()));
        let mut frame = data.clone();
        frame.insert(TAG_DATA.into(), Tag::Compound(payload));
        assert_eq!(
            Extension::from_compound(&frame),
            Err(Refusal::StageUnknown {
                found: "HARVESTED".to_owned()
            })
        );
    }

    #[test]
    fn a_payload_that_is_charred_leaves_the_frame_readable() {
        // `:42`: `getCompound(TAG_EXTENSION_DATA)` on an absent key is an empty
        // compound, so the extension reads with its defaults — and the stage,
        // which has no default, is what refuses.
        let data = Compound::from([
            (TAG_NAME.into(), Tag::String("minecolonies:farmfield".into())),
            (TAG_POSITION.into(), nbt::position([4, 70, 4])),
        ]);
        assert_eq!(
            Extension::from_compound(&data),
            Err(Refusal::StageUnknown {
                found: String::new()
            })
        );
        // A plantation has no such required field, so it reads.
        let data = Compound::from([
            (TAG_NAME.into(), Tag::String("minecolonies:plantation_cactus".into())),
            (TAG_POSITION.into(), nbt::position([4, 70, 4])),
        ]);
        let read = Extension::from_compound(&data).expect("a plantation with no payload");
        assert_eq!(read.at, [4, 70, 4]);
        assert_eq!(read.payload, Payload::Plantation(PlantationField::default()));
    }

    #[test]
    fn a_seed_round_trips_and_an_unreadable_one_is_named() {
        let mut field = farm();
        field.set_seed(Some(ItemId::parse("seed", "minecraft:wheat_seeds").expect("a seed")));
        let extension = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(field.clone()));
        let data = extension.to_compound();
        let Some(Tag::Compound(payload)) = tag(&data, TAG_DATA) else {
            panic!("data is a compound");
        };
        let Some(Tag::Compound(seed)) = tag(payload, TAG_SEED) else {
            panic!("seed is a compound");
        };
        // Vanilla's `ItemStack.save`: an id and a count, and the count is one.
        assert_eq!(tag(seed, TAG_SEED_ID).and_then(as_text).as_deref(), Some("minecraft:wheat_seeds"));
        assert_eq!(tag(seed, TAG_SEED_COUNT), Some(&Tag::Byte(1)));

        let mut broken = payload.clone();
        broken.insert(
            TAG_SEED.into(),
            Tag::Compound(Compound::from([(
                TAG_SEED_ID.into(),
                Tag::String("Wheat Seeds!".into()),
            )])),
        );
        let mut frame = data.clone();
        frame.insert(TAG_DATA.into(), Tag::Compound(broken));
        assert_eq!(
            Extension::from_compound(&frame),
            Err(Refusal::SeedUnreadable {
                found: "Wheat Seeds!".to_owned()
            })
        );
    }

    #[test]
    fn an_owner_that_is_not_a_position_is_refused_and_an_absent_one_is_free() {
        let free = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(farm()));
        let read = Extension::from_compound(&free.to_compound()).expect("read back");
        assert_eq!(read.owner, None);
        assert!(!read.is_taken());

        let mut data = free.to_compound();
        let mut payload = match tag(&data, TAG_DATA).expect("data") {
            Tag::Compound(payload) => payload.clone(),
            _ => panic!("data is a compound"),
        };
        payload.insert(TAG_OWNER.into(), Tag::Int(7));
        data.insert(TAG_DATA.into(), Tag::Compound(payload));
        assert_eq!(Extension::from_compound(&data), Err(Refusal::OwnerNotAPosition));
    }

    #[test]
    fn the_radius_has_no_floor_and_a_short_array_refuses_a_direction() {
        // `:209-212`: `Math.min(radius, MAX_RANGE)` and nothing else.
        let mut field = FarmField::default();
        assert_eq!(field.radius(Side::South), Some(DEFAULT_RANGE));
        field.set_radius(Side::East, 40).expect("east exists");
        assert_eq!(field.radius(Side::East), Some(MAX_RANGE));
        field.set_radius(Side::East, -3).expect("east exists");
        assert_eq!(field.radius(Side::East), Some(-3), "Java keeps a negative radius");

        // `:118` on an absent tag is `int[0]`, and `:200-203` then throws.
        let mut short = FarmField::default();
        short.radii = vec![1, 2];
        assert_eq!(short.radius(Side::South), Some(1));
        assert_eq!(short.radius(Side::North), None);
        assert_eq!(
            short.set_radius(Side::North, 3),
            Err(Refusal::RadiusMissing {
                side: Side::North,
                count: 2
            })
        );
        // And the write always produces four, whatever the read held.
        let extension = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(short));
        let data = extension.to_compound();
        let Some(Tag::Compound(payload)) = tag(&data, TAG_DATA) else {
            panic!("data is a compound");
        };
        assert_eq!(nbt::ints_in(payload, TAG_RADII), Some(vec![1, 2]));
    }

    #[test]
    fn the_stage_cycles_and_its_icon_is_part_of_the_constant() {
        assert_eq!(Stage::Empty.next(), Stage::Hoed);
        assert_eq!(Stage::Hoed.next(), Stage::Planted);
        assert_eq!(Stage::Planted.next(), Stage::Empty);
        assert_eq!(Stage::parse("HOED"), Ok(Stage::Hoed));
        assert_eq!(Stage::parse("hoed"), Err(Refusal::StageUnknown { found: "hoed".to_owned() }));
        assert_eq!(Stage::Empty.icon(), ("minecraft", "textures/item/iron_hoe.png"));
        assert_eq!(
            Stage::Planted.icon(),
            ("minecolonies", "textures/item/crops/durum.png")
        );
        let mut field = FarmField::default();
        field.next_stage();
        assert_eq!(field.stage, Stage::Hoed);
    }

    #[test]
    fn the_distance_is_floored_and_not_squared() {
        // `:126-129` is `(int) Math.sqrt(getDistanceSquared(..))`.
        let extension = extension(Kind::from_path("farmfield").expect("farmfield"), Payload::Farm(farm()));
        assert_eq!(extension.at, [10, 64, -3]);
        assert_eq!(extension.distance_to([10, 64, -3]), 0);
        assert_eq!(extension.distance_to([10, 67, -7]), 5);
        assert_eq!(extension.distance_to([10, 65, -3]), 1);
        // 1.41 floors to 1, which is the whole point of the name.
        assert_eq!(extension.distance_to([11, 65, -3]), 1);
    }

    #[test]
    fn the_sides_are_in_the_order_the_radii_array_is_saved_in() {
        // `FarmField.java:56-58`: "the same order as Direction: S, W, N, E".
        assert_eq!(Side::ALL.map(Side::index), [0, 1, 2, 3]);
        assert_eq!(Side::South.index(), 0);
        assert_eq!(Side::East.index(), 3);
    }
}
