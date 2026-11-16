//! The twenty-one messages a hut answers directly: stock, the restaurant's menu,
//! filters, build requests, style, delivery priority, hiring, names, the crafting
//! screen, the miner's levels, the settings pane.
//!
//! Java reference: `core/network/messages/server/colony/building/` — twenty-one
//! files, 1 674 lines. Every one of them extends
//! `AbstractBuildingServerMessage<…>` (`:14`), which means every one carries the
//! **same three-field envelope** — dimension, colony id, hut position
//! (`AbstractColonyServerMessage.java:85-86`, `AbstractBuildingServerMessage.java:61`)
//! — and every one inherits the same two rules: a gate of `MANAGE_HUTS`
//! (`AbstractColonyServerMessage.java:60-64`) and a **five-block** range
//! (`AbstractBuildingServerMessage.java:70-73`), shorter than the fifteen a
//! location-only message gets (`IRangeRestrictedPacket.java:11`).
//!
//! **The envelope is why these are cheap to port and dangerous to fake.** A
//! message here says "this hut" and then one value; the hut is found by position
//! (`AbstractBuildingServerMessage.java:80`) and the module by an id the client was
//! told about. The port keeps the position as a field and the module id as an `i32`
//! — a module id is a runtime handle the client received, never a name.
//!
//! **`TriggerSettingMessage` is the exception.** It extends the *colony* message and
//! carries the hut position as its own field (`TriggerSettingMessage.java:44`),
//! because a setting can belong to the colony itself: `BlockPos.ZERO` means "the
//! colony's settings", anything else means "that hut's"
//! (`:91-101`). The port keeps the position and the same rule.
//!
//! **Two values have no equivalent and are carried as they are.** A `BlockState`
//! (nothing here), and an `ISetting` (`TriggerSettingMessage.java:34`), serialized
//! by `StandardFactoryController` (`:83`) — a factory this port does not have, so
//! the setting travels as bytes (`MANQUANT.md` §1).

use super::common::{
    boolean, building_envelope, building_envelope_of, bytes_of, int, int_of, pos_of, position,
    stack, stack_of, text, text_of, ItemStack, Message, Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::Compound;

/// `BuildRequestMessage.java:23-28` — what the client asks the builder to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three-value enum of Java's Mode
pub enum BuildMode {
    /// `BUILD` (`:25`) — upgrade to the next level.
    Build,
    /// `REPAIR` (`:26`) — fix the current one.
    Repair,
    /// `REMOVE` (`:27`) — take it down, children included (`:94-103`).
    Remove,
}

impl BuildMode {
    pub const ALL: [BuildMode; 3] = [BuildMode::Build, BuildMode::Repair, BuildMode::Remove];

    pub fn name(self) -> &'static str {
        match self {
            BuildMode::Build => "BUILD",
            BuildMode::Repair => "REPAIR",
            BuildMode::Remove => "REMOVE",
        }
    }

    pub fn parse(name: &str) -> Option<BuildMode> {
        BuildMode::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// `api/colony/buildings/HiringMode.java:10-13` — who may hire into a module.
///
/// Four values, and `LOCKED` is not "no hiring": it is "nobody, and the colony
/// manager cannot change it" (`BuildingHiringModeMessage.java:66-69`). The port
/// keeps all four because a message may set any of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four-value enum of Java's HiringMode
pub enum HiringMode {
    /// `DEFAULT` (`:10`) — the colony's configured default.
    Default,
    /// `AUTO` (`:11`).
    Auto,
    /// `MANUAL` (`:12`).
    Manual,
    /// `LOCKED` (`:13`).
    Locked,
}

impl HiringMode {
    pub const ALL: [HiringMode; 4] =
        [HiringMode::Default, HiringMode::Auto, HiringMode::Manual, HiringMode::Locked];

    pub fn name(self) -> &'static str {
        match self {
            HiringMode::Default => "DEFAULT",
            HiringMode::Auto => "AUTO",
            HiringMode::Manual => "MANUAL",
            HiringMode::Locked => "LOCKED",
        }
    }

    pub fn parse(name: &str) -> Option<HiringMode> {
        HiringMode::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// `AddMinimumStockToBuildingModuleMessage.java:16-73` — "keep at least this many
/// in stock".
///
/// No module id: Java reaches the module by class
/// (`getFirstModuleOccurance(MinimumStockModule.class)`, `:67`), and a hut has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddMinimumStockToBuildingModule {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub stack: ItemStack,
    pub quantity: i32,
}

impl Message for AddMinimumStockToBuildingModule {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("quantity".into(), int(self.quantity));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AddMinimumStockToBuildingModule {
            colony_id,
            dimension,
            building,
            stack: stack_of(data, "itemStack")?,
            quantity: int_of(data, "quantity")?,
        })
    }
}

/// `AlterRestaurantMenuItemMessage.java:16-111` — add or remove a dish from the
/// restaurant's menu, on the module the client named (`:99`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlterRestaurantMenuItem {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub stack: ItemStack,
    /// The module's runtime id (`:26`, `:76`).
    pub id: i32,
    /// `true` for `addMenuItem` (`:48-51`), `false` for `removeMenuItem`
    /// (`:60-63`).
    pub add: bool,
}

impl Message for AlterRestaurantMenuItem {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("id".into(), int(self.id));
        data.insert("add".into(), boolean(self.add));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AlterRestaurantMenuItem {
            colony_id,
            dimension,
            building,
            stack: stack_of(data, "itemStack")?,
            id: int_of(data, "id")?,
            add: super::common::bool_of(data, "add")?,
        })
    }
}

/// `AssignFilterableEntityMessage.java:16-89` — the entity filter of a hut's
/// `EntityListModule`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignFilterableEntity {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub assign: bool,
    /// A registry name, `minecraft:cow`.
    pub entity: String,
    pub id: i32,
}

impl Message for AssignFilterableEntity {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("assign".into(), boolean(self.assign));
        data.insert("entity".into(), text(&self.entity));
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AssignFilterableEntity {
            colony_id,
            dimension,
            building,
            assign: super::common::bool_of(data, "assign")?,
            entity: text_of(data, "entity")?,
            id: int_of(data, "id")?,
        })
    }
}

/// `AssignFilterableItemMessage.java:16-91` — the item filter of an
/// `ItemListModule`.
///
/// Java wraps the stack in an `ItemStorage` on both ends (`:62`, `:71`), and
/// `ItemStorage` is a stack plus two booleans that are `false` here; the wire is
/// the stack, so the port carries the stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignFilterableItem {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub assign: bool,
    pub item: ItemStack,
    pub id: i32,
}

impl Message for AssignFilterableItem {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("assign".into(), boolean(self.assign));
        data.insert("item".into(), stack(&self.item));
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AssignFilterableItem {
            colony_id,
            dimension,
            building,
            assign: super::common::bool_of(data, "assign")?,
            item: stack_of(data, "item")?,
            id: int_of(data, "id")?,
        })
    }
}

/// `BuildPickUpMessage.java:15-53` — put the hut back in the player's inventory
/// (`:51`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPickUp {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for BuildPickUp {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(BuildPickUp { colony_id, dimension, building })
    }
}

/// `BuildRequestMessage.java:18-109` — build, repair or remove, by the builder at
/// `builder` (`:33`).
///
/// The `builder` position is where the build tool was clicked, not the hut: it is
/// the position the work order will stand on (`:55-60`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildRequest {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub mode: BuildMode,
    pub builder: [i32; 3],
}

impl Message for BuildRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("mode".into(), text(self.mode.name()));
        data.insert("builder".into(), position(self.builder));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        let mode = text_of(data, "mode")?;
        Ok(BuildRequest {
            colony_id,
            dimension,
            building,
            mode: BuildMode::parse(&mode).ok_or(Refusal::Name { key: "mode" })?,
            builder: pos_of(data, "builder")?,
        })
    }
}

/// `BuildingSetStyleMessage.java:14-63` — a hut's structure pack, allowed only
/// while the hut is unbuilt (`:56-59`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildingSetStyle {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub structure_pack: String,
}

impl Message for BuildingSetStyle {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("structurePack".into(), text(&self.structure_pack));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(BuildingSetStyle {
            colony_id,
            dimension,
            building,
            structure_pack: text_of(data, "structurePack")?,
        })
    }
}

/// `ChangeDeliveryPriorityMessage.java:13-78` — move a hut up or down the
/// deliveryman's round (`:69`, `:73`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeDeliveryPriority {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    /// `true` is `alterPickUpPriority(1)` (`:69`), `false` is `-1` (`:73`).
    pub up: bool,
}

impl Message for ChangeDeliveryPriority {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("up".into(), boolean(self.up));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(ChangeDeliveryPriority {
            colony_id,
            dimension,
            building,
            up: super::common::bool_of(data, "up")?,
        })
    }
}

/// `CourierHiringModeMessage.java:16-71` — the hiring mode of a courier module
/// (`:66-69`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CourierHiringMode {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub mode: HiringMode,
    pub id: i32,
}

impl Message for CourierHiringMode {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("mode".into(), text(self.mode.name()));
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        let mode = text_of(data, "mode")?;
        Ok(CourierHiringMode {
            colony_id,
            dimension,
            building,
            mode: HiringMode::parse(&mode).ok_or(Refusal::Name { key: "mode" })?,
            id: int_of(data, "id")?,
        })
    }
}

/// `ForcePickupMessage.java:21-72` — make the deliveryman collect what the hut
/// offers now, without waiting for the round (`:56-60`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForcePickup {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for ForcePickup {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(ForcePickup { colony_id, dimension, building })
    }
}

/// `GiveToolMessage.java:23-81` — hand the player the hut's tool, pointed at the
/// hut (`:75-77`).
///
/// Java sends `new ItemStack(item, 1)` and reads back only `getItem()` (`:52`,
/// `:58`): the stack is a way to carry an item name, and the port says so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GiveTool {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub item: ItemStack,
}

impl Message for GiveTool {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("item".into(), stack(&self.item));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(GiveTool { colony_id, dimension, building, item: stack_of(data, "item")? })
    }
}

/// `HireFireMessage.java:16-99` — hire a citizen into a module, or let them go
/// (`:85-97`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HireFire {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub hire: bool,
    pub citizen_id: i32,
    pub module_id: i32,
}

impl Message for HireFire {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("hire".into(), boolean(self.hire));
        data.insert("citizenID".into(), int(self.citizen_id));
        data.insert("moduleId".into(), int(self.module_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(HireFire {
            colony_id,
            dimension,
            building,
            hire: super::common::bool_of(data, "hire")?,
            citizen_id: int_of(data, "citizenID")?,
            module_id: int_of(data, "moduleId")?,
        })
    }
}

/// `HutRenameMessage.java:14-60` — the hut's own name, which the colony shows
/// beside its kind.
///
/// No length cut here, unlike the colony's (`colony_settings::TownHallRename`):
/// Java writes whatever it was given (`:45-53`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HutRename {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub name: String,
}

impl Message for HutRename {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("name".into(), text(&self.name));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(HutRename { colony_id, dimension, building, name: text_of(data, "name")? })
    }
}

/// `MarkBuildingDirtyMessage.java:15-47` — "save this hut"
/// (`building.markDirty()`, `:45`).
///
/// A message whose whole payload is its envelope: the click is what happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkBuildingDirty {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for MarkBuildingDirty {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(MarkBuildingDirty { colony_id, dimension, building })
    }
}

/// `OpenCraftingGUIMessage.java:27-138` — open the crafting screen of the module
/// the client named (`:74`).
///
/// The screen chosen (furnace, brewing stand, crafting table) depends on what the
/// module can teach (`:76-135`) — a rule the caller applies, not the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCraftingGUI {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub id: i32,
}

impl Message for OpenCraftingGUI {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(OpenCraftingGUI { colony_id, dimension, building, id: int_of(data, "id")? })
    }
}

/// `QuarryHiringModeMessage.java:16-70` — the quarry's hiring mode (`:65-68`).
///
/// The constructor forgets to store `moduleID` (`:42-46` sets `mode` only) — a
/// Java bug that sends `0` to the server; the port keeps the field because the wire
/// carries it, and `RAPPORT.md` §"ce que je n'ai pas copié" records the bug instead
/// of reproducing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuarryHiringMode {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub mode: HiringMode,
    pub module_id: i32,
}

impl Message for QuarryHiringMode {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("mode".into(), text(self.mode.name()));
        data.insert("moduleID".into(), int(self.module_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        let mode = text_of(data, "mode")?;
        Ok(QuarryHiringMode {
            colony_id,
            dimension,
            building,
            mode: HiringMode::parse(&mode).ok_or(Refusal::Name { key: "mode" })?,
            module_id: int_of(data, "moduleID")?,
        })
    }
}

/// `RecallCitizenHutMessage.java:27-87` — bring every citizen assigned to the hut
/// back to it, respawning the ones that went AWOL (`:52-73`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallCitizenHut {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for RecallCitizenHut {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(RecallCitizenHut { colony_id, dimension, building })
    }
}

/// `RemoveMinimumStockFromBuildingModuleMessage.java:16-71` — drop a minimum-stock
/// line (`:66-69`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveMinimumStockFromBuildingModule {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub stack: ItemStack,
    pub module_id: i32,
}

impl Message for RemoveMinimumStockFromBuildingModule {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("moduleId".into(), int(self.module_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(RemoveMinimumStockFromBuildingModule {
            colony_id,
            dimension,
            building,
            stack: stack_of(data, "itemStack")?,
            module_id: int_of(data, "moduleId")?,
        })
    }
}

/// `ResetFilterableItemMessage.java:15-63` — put a hut's item filter back to its
/// defaults (`:58-61`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetFilterableItem {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub id: i32,
}

impl Message for ResetFilterableItem {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(ResetFilterableItem { colony_id, dimension, building, id: int_of(data, "id")? })
    }
}

/// `TransferItemsRequestMessage.java:29-145` — move items from the player's
/// inventory into the hut (`:109-124`).
///
/// **A quantity of zero or less is refused at the door.** Java checks it inside
/// `onExecute` and logs a warning (`:89-93`); the port refuses the decode, because
/// a transfer of nothing is a request the sender should not have made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferItemsRequest {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub stack: ItemStack,
    pub quantity: i32,
    /// Whether the delivery should also overrule the next open request
    /// (`:134-137`).
    pub attempt_resolve: bool,
}

impl Message for TransferItemsRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("quantity".into(), int(self.quantity));
        data.insert("attemptResolve".into(), boolean(self.attempt_resolve));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(TransferItemsRequest {
            colony_id,
            dimension,
            building,
            stack: stack_of(data, "itemStack")?,
            quantity: super::common::int_in(data, "quantity", 1, i32::MAX)?,
            attempt_resolve: super::common::bool_of(data, "attemptResolve")?,
        })
    }
}

/// `TriggerSettingMessage.java:24-104` — one setting changed in a settings pane.
///
/// The key is a `ResourceLocation` (`:29`), the value is an `ISetting` the factory
/// serialized (`:83`) and the port carries as bytes, and `BlockPos.ZERO` as
/// `building_pos` means the colony's own settings (`:91-94`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerSetting {
    pub colony_id: i32,
    pub dimension: Dimension,
    /// The setting key's registry name, `minecolonies:worker_hiring_mode`.
    pub key: String,
    pub value: Vec<u8>,
    pub module_id: i32,
    pub building_pos: [i32; 3],
}

impl Message for TriggerSetting {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        super::common::envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("key".into(), text(&self.key));
        data.insert("value".into(), super::common::bytes(&self.value));
        data.insert("moduleID".into(), int(self.module_id));
        data.insert("buildingPos".into(), position(self.building_pos));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = super::common::envelope_of(data)?;
        Ok(TriggerSetting {
            colony_id,
            dimension,
            key: text_of(data, "key")?,
            value: bytes_of(data, "value")?,
            module_id: int_of(data, "moduleID")?,
            building_pos: pos_of(data, "buildingPos")?,
        })
    }
}

/// The twenty-one messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/colony/building/AddMinimumStockToBuildingModuleMessage.java", "AddMinimumStockToBuildingModule"),
    ("server/colony/building/AlterRestaurantMenuItemMessage.java", "AlterRestaurantMenuItem"),
    ("server/colony/building/AssignFilterableEntityMessage.java", "AssignFilterableEntity"),
    ("server/colony/building/AssignFilterableItemMessage.java", "AssignFilterableItem"),
    ("server/colony/building/BuildPickUpMessage.java", "BuildPickUp"),
    ("server/colony/building/BuildRequestMessage.java", "BuildRequest"),
    ("server/colony/building/BuildingSetStyleMessage.java", "BuildingSetStyle"),
    ("server/colony/building/ChangeDeliveryPriorityMessage.java", "ChangeDeliveryPriority"),
    ("server/colony/building/CourierHiringModeMessage.java", "CourierHiringMode"),
    ("server/colony/building/ForcePickupMessage.java", "ForcePickup"),
    ("server/colony/building/GiveToolMessage.java", "GiveTool"),
    ("server/colony/building/HireFireMessage.java", "HireFire"),
    ("server/colony/building/HutRenameMessage.java", "HutRename"),
    ("server/colony/building/MarkBuildingDirtyMessage.java", "MarkBuildingDirty"),
    ("server/colony/building/OpenCraftingGUIMessage.java", "OpenCraftingGUI"),
    ("server/colony/building/QuarryHiringModeMessage.java", "QuarryHiringMode"),
    ("server/colony/building/RecallCitizenHutMessage.java", "RecallCitizenHut"),
    ("server/colony/building/RemoveMinimumStockFromBuildingModuleMessage.java", "RemoveMinimumStockFromBuildingModule"),
    ("server/colony/building/ResetFilterableItemMessage.java", "ResetFilterableItem"),
    ("server/colony/building/TransferItemsRequestMessage.java", "TransferItemsRequest"),
    ("server/colony/building/TriggerSettingMessage.java", "TriggerSetting"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn hut() -> (Dimension, i32, [i32; 3]) {
        (Dimension::new("minecraft:overworld"), 7, [120, 64, -40])
    }

    fn stack_of_items(name: &str, count: i32) -> ItemStack {
        ItemStack::new(name, count).expect("a valid stack")
    }

    #[test]
    fn the_four_stack_carrying_messages_round_trip() {
        let (dimension, colony_id, building) = hut();
        round_trip(AddMinimumStockToBuildingModule {
            colony_id,
            dimension: dimension.clone(),
            building,
            stack: stack_of_items("minecraft:oak_planks", 32),
            quantity: 64,
        });
        round_trip(AlterRestaurantMenuItem {
            colony_id,
            dimension: dimension.clone(),
            building,
            stack: stack_of_items("minecraft:baked_potato", 1),
            id: 3,
            add: false,
        });
        round_trip(AssignFilterableItem {
            colony_id,
            dimension: dimension.clone(),
            building,
            assign: true,
            item: stack_of_items("minecraft:wheat", 1),
            id: 0,
        });
        round_trip(RemoveMinimumStockFromBuildingModule {
            colony_id,
            dimension,
            building,
            stack: stack_of_items("minecraft:stone", 4),
            module_id: 2,
        });
    }

    #[test]
    fn the_five_envelope_only_messages_round_trip_to_the_three_shared_keys() {
        let (dimension, colony_id, building) = hut();
        for data in [
            BuildPickUp { colony_id, dimension: dimension.clone(), building }.to_compound(),
            ForcePickup { colony_id, dimension: dimension.clone(), building }.to_compound(),
            MarkBuildingDirty { colony_id, dimension: dimension.clone(), building }.to_compound(),
            RecallCitizenHut { colony_id, dimension, building }.to_compound(),
        ] {
            assert_eq!(data.len(), 3, "dimension, colony, building and nothing else");
        }
        round_trip(BuildPickUp { colony_id, dimension: Dimension::new("minecraft:overworld"), building });
        round_trip(ForcePickup { colony_id, dimension: Dimension::new("minecraft:overworld"), building });
        round_trip(MarkBuildingDirty {
            colony_id,
            dimension: Dimension::new("minecraft:overworld"),
            building,
        });
        round_trip(RecallCitizenHut { colony_id, dimension: Dimension::new("minecraft:overworld"), building });
    }

    #[test]
    fn a_build_request_keeps_its_mode_and_builder() {
        let (dimension, colony_id, building) = hut();
        for mode in BuildMode::ALL {
            let request = BuildRequest {
                colony_id,
                dimension: dimension.clone(),
                building,
                mode,
                builder: [1, 2, 3],
            };
            assert_eq!(round_trip(request).mode, mode);
        }
        let mut data = BuildRequest {
            colony_id,
            dimension,
            building,
            mode: BuildMode::Build,
            builder: [0, 0, 0],
        }
        .to_compound();
        data.insert("mode".into(), text("DESTROY"));
        assert_eq!(BuildRequest::from_compound(&data), Err(Refusal::Name { key: "mode" }));
    }

    #[test]
    fn the_two_hiring_modes_round_trip_every_value() {
        let (dimension, colony_id, building) = hut();
        for mode in HiringMode::ALL {
            let courier = CourierHiringMode {
                colony_id,
                dimension: dimension.clone(),
                building,
                mode,
                id: 1,
            };
            assert_eq!(round_trip(courier).mode, mode);
            let quarry = QuarryHiringMode {
                colony_id,
                dimension: dimension.clone(),
                building,
                mode,
                module_id: 1,
            };
            assert_eq!(round_trip(quarry).mode, mode);
        }
    }

    #[test]
    fn a_style_name_and_a_tool_round_trip() {
        let (dimension, colony_id, building) = hut();
        assert_eq!(
            round_trip(BuildingSetStyle {
                colony_id,
                dimension: dimension.clone(),
                building,
                structure_pack: "medieval".to_owned()
            })
            .structure_pack,
            "medieval"
        );
        assert_eq!(
            round_trip(HutRename {
                colony_id,
                dimension: dimension.clone(),
                building,
                name: "Ferme du nord".to_owned()
            })
            .name,
            "Ferme du nord"
        );
        assert_eq!(
            round_trip(GiveTool {
                colony_id,
                dimension,
                building,
                item: stack_of_items("minecraft:iron_axe", 1)
            })
            .item
            .item
            .as_str(),
            "minecraft:iron_axe"
        );
    }

    #[test]
    fn hiring_firing_and_filtering_round_trip() {
        let (dimension, colony_id, building) = hut();
        let fire = HireFire {
            colony_id,
            dimension: dimension.clone(),
            building,
            hire: false,
            citizen_id: 11,
            module_id: 2,
        };
        let read = round_trip(fire);
        assert_eq!((read.hire, read.citizen_id, read.module_id), (false, 11, 2));
        let entity = AssignFilterableEntity {
            colony_id,
            dimension: dimension.clone(),
            building,
            assign: true,
            entity: "minecraft:cow".to_owned(),
            id: 4,
        };
        assert_eq!(round_trip(entity).entity, "minecraft:cow");
        assert_eq!(
            round_trip(ResetFilterableItem { colony_id, dimension: dimension.clone(), building, id: 4 }).id,
            4
        );
        assert_eq!(
            round_trip(OpenCraftingGUI { colony_id, dimension, building, id: 4 }).id,
            4
        );
    }

    #[test]
    fn a_transfer_refuses_a_quantity_of_nothing() {
        let (dimension, colony_id, building) = hut();
        let mut data = TransferItemsRequest {
            colony_id,
            dimension,
            building,
            stack: stack_of_items("minecraft:cobblestone", 64),
            quantity: 64,
            attempt_resolve: true,
        }
        .to_compound();
        assert_eq!(
            TransferItemsRequest::from_compound(&data).map(|message| message.quantity),
            Ok(64)
        );
        data.insert("quantity".into(), int(0));
        assert_eq!(
            TransferItemsRequest::from_compound(&data),
            Err(Refusal::Number { key: "quantity", min: 1, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn a_setting_round_trips_beside_its_hut_or_at_the_origin() {
        let (dimension, colony_id, _) = hut();
        for building_pos in [[120, 64, -40], [0, 0, 0]] {
            let setting = TriggerSetting {
                colony_id,
                dimension: dimension.clone(),
                key: "minecolonies:worker_hiring_mode".to_owned(),
                value: vec![1, 0, 0, 0],
                module_id: 2,
                building_pos,
            };
            assert_eq!(round_trip(setting).building_pos, building_pos);
        }
    }

    #[test]
    fn every_building_message_is_gated_on_manage_huts_and_needs_its_hut() {
        assert_eq!(BuildPickUp::RULE, Rule::ManageHuts);
        assert_eq!(TriggerSetting::RULE, Rule::ManageHuts, "the colony-level one too");
        let (dimension, colony_id, building) = hut();
        let mut data = BuildPickUp { colony_id, dimension, building }.to_compound();
        data.remove(&"buildingId".into());
        assert_eq!(
            BuildPickUp::from_compound(&data),
            Err(Refusal::Missing { key: super::super::common::KEY_BUILDING })
        );
    }

    #[test]
    fn a_delivery_priority_move_round_trips_both_directions() {
        let (dimension, colony_id, building) = hut();
        for up in [true, false] {
            let move_it = ChangeDeliveryPriority {
                colony_id,
                dimension: dimension.clone(),
                building,
                up,
            };
            assert_eq!(round_trip(move_it).up, up);
        }
    }

    #[test]
    fn every_mode_name_parses_back() {
        for mode in BuildMode::ALL {
            assert_eq!(BuildMode::parse(mode.name()), Some(mode));
        }
        for mode in HiringMode::ALL {
            assert_eq!(HiringMode::parse(mode.name()), Some(mode));
        }
        assert_eq!(HiringMode::parse("default"), None);
        assert_eq!(CENSUS.len(), 21);
    }
}
