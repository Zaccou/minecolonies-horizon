//! Four data managers of Java's tree: the three registries that turn a saved id
//! into a live object, and the store that holds the recipes a crafter can make.
//!
//! Java reference: `core/colony/buildingextensions/registry/BuildingExtensionDataManager.java`
//! (111 lines), `core/colony/buildings/registry/BuildingDataManager.java` (102),
//! `core/colony/buildings/registry/GuardTypeDataManager.java` (20),
//! `core/colony/crafting/CustomRecipeManager.java` (445).
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `guard_type` | which guard types exist, and what a job name that is one resolves to | `:1-20` |
//! | `building_data` | which kind a saved `type` names, and the one alias the registry does not hold | `:25-95` |
//! | `building_extension` | the three tags an extension is saved under, and what each of the two field payloads holds | `:18-111` |
//! | `custom_recipes` | what is stored per crafter and per output, and when a pending removal is applied | `:36-445` |
//!
//! **Three of the four are entry points over models that already exist, and this
//! module says so rather than repeating them.** A `BuildingDataManager` produces an `IBuilding`
//! out of a compound; the record and its rules are `crate::building::Building`
//! already (`src/building/instance.rs:350-421`), and the list that holds them is
//! `crate::persistence::workshop` (`src/persistence/workshop.rs:56-124`). What
//! those two do **not** answer is the question this file adds: which *kind* a
//! saved `type` names, including Java's `home` alias, and what a save with an
//! unknown type must do — Java's answer is to drop the entry and keep the colony
//! (`BuildingDataManager.java:35-49`, honoured at
//! `RegisteredStructureManager.java:172-176`). `building_data` is those two
//! steps, sized to their real content: [`kind_for`] and [`from_record`].
//!
//! **The fourth is not covered anywhere and was not a near miss.** `crafter.rs`
//! owns the `crafterrecipes/` *model* and says in its own note that the manager
//! keying them by crafter and by output "is a layer above"
//! (`src/crafting/crafter.rs:48-52`); `src/crafting/MANQUANT.md` §6 names the
//! same store as deliberately absent, and `Removal` — the `remove` file — is a
//! value nothing applied. `custom_recipes` is that layer, and it is the only file
//! here that holds mutable state.
//!
//! **One premise of the brief is false, stated first.** It says to port the
//! *rules* "without the world". For `GuardTypeDataManager` there is almost no rule
//! to port: 20 lines that are one registry lookup and one null check, where the
//! interesting part is the *registry contents* — the six guard types and the
//! fourteen field types — which lives in `apiimp/initializer/` and is nowhere in
//! the port yet. `BuildingExtensionDataManager` is four codecs (NBT and buffer, in
//! both directions) whose interesting half is the payload of the two classes it
//! produces. So those two files carry the tables, with their initialiser line
//! numbers, and the lookups, and they are honest about being tables:
//! [`guard_type::GUARDS`], [`building_extension::EXTENSIONS`].
//!
//! **The second false premise is the other way round**: `CustomRecipeManager` and
//! `BuildingDataManager` never refuse anything in Java — they return `null` and
//! log. A `null` is a refusal with no name. What makes it *harmless* is Java's
//! order of operations: nothing is written before it. The port keeps that order
//! and names the cause, so a test can hold both halves — the refusal is named
//! **and** the store is untouched.
//!
//! **Pure data, pure rules**, like `crate::manager`, `crate::manager_01` and
//! `crate::crafting`: no host call, no clock, no file, no thread. Every table is a
//! `&'static [..]` and every map is a `BTreeMap`, so a test can say *which* entry
//! comes out where Java's `HashMap` would leave it unspecified; each file notes
//! what that pins down.
//!
//! Nothing here is wired to a hook. Java calls these managers from the colony's
//! load path, the network handlers and the crafter menus; the port has the load
//! path (`crate::persistence`) and no client, so the client half of each is in
//! `MANQUANT.md` with what it would cost.

pub mod building_data;
pub mod building_extension;
pub mod custom_recipes;
pub mod guard_type;

/// The tag shapes the extension payloads use. Private: it is a helper of
/// `building_extension`, and `src/building/nbt.rs:8-12` already explains why a
/// fourth copy exists rather than a shared one.
mod nbt;

pub use building_data::{
    from_record, from_tile, kind_for, view_kind, Refusal as BuildingRefusal,
};
pub use building_extension::{
    Extension, FarmField, Field, Kind as ExtensionKind, Payload, PlantationField,
    Refusal as ExtensionRefusal, Side, Stage as FieldStage, DEFAULT_RANGE, EXTENSIONS, MAX_RANGE,
};
pub use custom_recipes::{
    Counts, IngestRefusal, Ingested, LootDrop, Refusal as RecipeRefusal, Store, TemplateFailure,
};
pub use guard_type::{GuardType, Refusal as GuardRefusal, GUARDS};

/// The namespace every table in this module is registered under: MineColonies'
/// own mod id, not this package's (`api/util/constant/Constants.java:13`).
///
/// It is written once here and re-used by the four files, because a registry key
/// in Java is a full `ResourceLocation` and a port that spells the namespace four
/// times can spell it wrong once. `crate::building::kind` and `crate::hut` take
/// the same decision for the same reason.
pub const NAMESPACE: &str = "minecolonies";
