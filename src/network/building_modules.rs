//! The twenty messages a hut's *module* answers: which work order the builder
//! takes, which worker an enchanter serves, a field's plot and seed, a guard's mine,
//! who lives where, a miner's level, a post box's request, a research, a warehouse
//! sort, a recipe.
//!
//! Java reference: `core/network/messages/server/colony/building/` — the five
//! sub-packages `builder`, `enchanter`, `fields`, `guard`, `home`, `miner`,
//! `postbox`, `university`, `warehouse`, `worker`, twenty files, 1 175 lines.
//!
//! **Nineteen of the twenty carry the building envelope** (dimension, colony, hut)
//! and answer `MANAGE_HUTS`. The exception is `FarmFieldPlotResizeMessage`, which is
//! not a hut message at all: it acts on the scarecrow's block entity at a position
//! (`FarmFieldPlotResizeMessage.java:68-71`) and only has the fifteen-block range
//! (`:63`). The port keeps that: [`FarmFieldPlotResize`] has no colony and no hut,
//! and [`Message::RULE`] is [`Rule::None`] — a permission check without a colony
//! would have nothing to check against.
//!
//! **Two registry handles stay numbers.** A job entry travels as a *registry id*
//! (`AssignUnassignMessage.java:73`, `:94`), a numercial handle the client got from
//! the same table — not a name, and the port keeps the number rather than inventing
//! a job registry. An item stack keeps its components as `common::ItemStack` does.
//!
//! **`AddRemoveRecipeMessage` carries a whole recipe.** Java writes the
//! `IRecipeStorage` through `StandardFactoryController` (`:136`, `:149`) — a
//! 209-line interface with a builder, and the port has no recipe model at this
//! layer, so the recipe travels as bytes and `MANQUANT.md` §1 says which class wrote
//! them.

use super::buildings::HiringMode;
use super::common::{
    bool_of, building_envelope, building_envelope_of, boolean, bytes, bytes_of, envelope,
    envelope_of, int, int_in, int_of, opt_pos_of, pos_of, position, stack, stack_of, text, text_of,
    ItemStack, Message, Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::Compound;

/// The four horizontal faces of a field, in the order `Direction.get2DDataValue`
/// numbers them: south `0`, west `1`, north `2`, east `3`.
///
/// Java indexes the scarecrow's four field sizes by that number
/// (`FarmFieldPlotResizeMessage.java:72`) and writes it (`:95`). The port writes the
/// name: the number is a vanilla enum's internal ordering, and a plot that grew on
/// its west side must not depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four 2D directions of vanilla's Direction
pub enum Face {
    South,
    West,
    North,
    East,
}

impl Face {
    pub const ALL: [Face; 4] = [Face::South, Face::West, Face::North, Face::East];

    pub fn name(self) -> &'static str {
        match self {
            Face::South => "south",
            Face::West => "west",
            Face::North => "north",
            Face::East => "east",
        }
    }

    pub fn parse(name: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|face| face.name() == name)
    }

    /// `Direction.get2DDataValue()` — south `0`, west `1`, north `2`, east `3`.
    pub fn index(self) -> i32 {
        match self {
            Face::South => 0,
            Face::West => 1,
            Face::North => 2,
            Face::East => 3,
        }
    }
}

/// `builder/BuilderSelectWorkOrderMessage.java:11-60` — the builder takes this work
/// order (`:58`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuilderSelectWorkOrder {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub work_order: i32,
}

impl Message for BuilderSelectWorkOrder {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("workOrder".into(), int(self.work_order));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(BuilderSelectWorkOrder {
            colony_id,
            dimension,
            building,
            work_order: int_of(data, "workOrder")?,
        })
    }
}

/// `enchanter/EnchanterWorkerSetMessage.java:16-76` — add or remove the worker an
/// enchanter's station serves (`:67-74`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchanterWorkerSet {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub worker: [i32; 3],
    pub add: bool,
}

impl Message for EnchanterWorkerSet {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("worker".into(), position(self.worker));
        data.insert("add".into(), boolean(self.add));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(EnchanterWorkerSet {
            colony_id,
            dimension,
            building,
            worker: pos_of(data, "worker")?,
            add: bool_of(data, "add")?,
        })
    }
}

/// `fields/AssignFieldMessage.java:18-96` — assign a field to a hut's
/// `BuildingExtensionsModule`, or free it (`:85-92`).
///
/// The field travels as the buffer `BuildingExtensionDataManager.extensionToBuffer`
/// wrote (`:54`, `:80`) — a registry of extension types this port has not ported, so
/// the buffer is bytes (`MANQUANT.md` §1). Note that the whole rest of the payload is
/// that buffer: Java writes no length (`:64`), it is the tail of the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignField {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub module_id: i32,
    pub field_data: Vec<u8>,
    pub assign: bool,
}

impl Message for AssignField {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("moduleID".into(), int(self.module_id));
        data.insert("fieldData".into(), bytes(&self.field_data));
        data.insert("assign".into(), boolean(self.assign));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AssignField {
            colony_id,
            dimension,
            building,
            module_id: int_of(data, "moduleID")?,
            field_data: bytes_of(data, "fieldData")?,
            assign: bool_of(data, "assign")?,
        })
    }
}

/// `fields/AssignmentModeMessage.java:15-63` — switch a field module between manual
/// and automatic assignment (`:60`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignmentMode {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub id: i32,
    pub assignment_mode: bool,
}

impl Message for AssignmentMode {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("id".into(), int(self.id));
        data.insert("assignmentMode".into(), boolean(self.assignment_mode));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AssignmentMode {
            colony_id,
            dimension,
            building,
            id: int_of(data, "id")?,
            assignment_mode: bool_of(data, "assignmentMode")?,
        })
    }
}

/// A scarecrow's field can never be wider than
/// [`MAX_FIELD_RANGE`] blocks in total (`FarmField.MAX_RANGE`,
/// `FarmFieldPlotResizeMessage.java:74`).
pub const MAX_FIELD_RANGE: i32 = 20;

/// `fields/FarmFieldPlotResizeMessage.java:22-106` — grow or shrink one side of a
/// scarecrow's field.
///
/// **Not a hut message**: no colony, no hut, no permission gate. The sender is
/// checked for range (`:63`) and the size against
/// [`MAX_FIELD_RANGE`] (`:74`), and a negative size is refused here at the door
/// rather than dropped in silence as Java does (`:74`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmFieldPlotResize {
    pub size: i32,
    pub direction: Face,
    pub position: [i32; 3],
}

impl Message for FarmFieldPlotResize {
    const RULE: Rule = Rule::None;

    fn to_compound(&self) -> Compound {
        Compound::from([
            ("size".into(), int(self.size)),
            ("direction".into(), text(self.direction.name())),
            ("position".into(), position(self.position)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let direction = text_of(data, "direction")?;
        Ok(FarmFieldPlotResize {
            size: int_in(data, "size", 0, MAX_FIELD_RANGE)?,
            direction: Face::parse(&direction).ok_or(Refusal::Name { key: "direction" })?,
            position: pos_of(data, "position")?,
        })
    }
}

/// `fields/FarmFieldRegistrationMessage.java:17-71` — a scarecrow asked to become a
/// field of this colony (`:49-57`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmFieldRegistration {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub position: [i32; 3],
}

impl Message for FarmFieldRegistration {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("position".into(), position(self.position));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(FarmFieldRegistration { colony_id, dimension, position: pos_of(data, "position")? })
    }
}

/// `fields/FarmFieldUpdateSeedMessage.java:16-77` — what the field should plant
/// (`:58-61`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmFieldUpdateSeed {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub new_seed: ItemStack,
    pub position: [i32; 3],
}

impl Message for FarmFieldUpdateSeed {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("newSeed".into(), stack(&self.new_seed));
        data.insert("position".into(), position(self.position));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(FarmFieldUpdateSeed {
            colony_id,
            dimension,
            new_seed: stack_of(data, "newSeed")?,
            position: pos_of(data, "position")?,
        })
    }
}

/// `guard/GuardSetMinePosMessage.java:16-93` — point a guard at a mine, or clear
/// the post.
///
/// The position is written **only when it is set** (`:59-64`), which is why the
/// field is an `Option` and not a zeroed position: `BlockPos.ZERO` is a valid mine,
/// the middle of the world, and the absence is a different thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardSetMinePos {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub mine_pos: Option<[i32; 3]>,
}

impl Message for GuardSetMinePos {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        if let Some(mine) = self.mine_pos {
            data.insert("minePos".into(), position(mine));
        }
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(GuardSetMinePos {
            colony_id,
            dimension,
            building,
            mine_pos: opt_pos_of(data, "minePos")?,
        })
    }
}

/// `home/AssignUnassignMessage.java:20-132` — move a citizen into a house, or into a
/// job's module.
///
/// **`job_entry` is a registry id, not a name.** Java writes
/// `writeRegistryId(getJobRegistry(), jobEntry)` (`:94`) — a numeric handle — and
/// reads it back into the registry (`:73`); the bool that precedes it (`:87-95`) is
/// exactly the `Option` the port keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignUnassign {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub assign: bool,
    pub citizen_id: i32,
    pub job_entry: Option<i32>,
}

impl Message for AssignUnassign {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("assign".into(), boolean(self.assign));
        data.insert("citizenID".into(), int(self.citizen_id));
        if let Some(job) = self.job_entry {
            data.insert("jobEntry".into(), int(job));
        }
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AssignUnassign {
            colony_id,
            dimension,
            building,
            assign: bool_of(data, "assign")?,
            citizen_id: int_of(data, "citizenID")?,
            job_entry: match data.get(&"jobEntry".into()) {
                Some(_) => Some(int_of(data, "jobEntry")?),
                None => None,
            },
        })
    }
}

/// `miner/MinerRepairLevelMessage.java:15-56` — pay to repair one of the mine's
/// levels (`:54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinerRepairLevel {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub level: i32,
}

impl Message for MinerRepairLevel {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("level".into(), int(self.level));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(MinerRepairLevel { colony_id, dimension, building, level: int_of(data, "level")? })
    }
}

/// `miner/MinerSetLevelMessage.java:15-56` — the level the miner works at now
/// (`:54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinerSetLevel {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub level: i32,
}

impl Message for MinerSetLevel {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("level".into(), int(self.level));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(MinerSetLevel { colony_id, dimension, building, level: int_of(data, "level")? })
    }
}

/// `postbox/PostBoxRequestMessage.java:16-81` — ask the colony for a stack, through
/// the post box (`:76-79`).
///
/// `deliver_available` decides the minimum the request settles for: `1` when a
/// delivery is possible, the whole quantity otherwise (`:76`) — the one rule this
/// message carries, and the port keeps it as the flag rather than as the computed
/// minimum, because the colony is what decides whether a delivery is possible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostBoxRequest {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub stack: ItemStack,
    pub deliver_available: bool,
    pub req_quantity: i32,
}

impl Message for PostBoxRequest {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("itemStack".into(), stack(&self.stack));
        data.insert("deliverAvailable".into(), boolean(self.deliver_available));
        data.insert("reqQuantity".into(), int(self.req_quantity));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(PostBoxRequest {
            colony_id,
            dimension,
            building,
            stack: stack_of(data, "itemStack")?,
            deliver_available: bool_of(data, "deliverAvailable")?,
            req_quantity: int_in(data, "reqQuantity", 1, i32::MAX)?,
        })
    }
}

/// `university/TryResearchMessage.java:18-98` — start a research, or reset one
/// (`:82-96`).
///
/// Two registry names and a flag: the branch is the tree branch, the id the research
/// inside it (`:81`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TryResearch {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub research_id: String,
    pub branch: String,
    pub reset: bool,
}

impl Message for TryResearch {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("researchId".into(), text(&self.research_id));
        data.insert("branch".into(), text(&self.branch));
        data.insert("reset".into(), boolean(self.reset));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(TryResearch {
            colony_id,
            dimension,
            building,
            research_id: text_of(data, "researchId")?,
            branch: text_of(data, "branch")?,
            reset: bool_of(data, "reset")?,
        })
    }
}

/// `warehouse/SortBuildingMessage.java:17-61` — sort the warehouse's racks
/// (`:56-59`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortBuilding {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for SortBuilding {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(SortBuilding { colony_id, dimension, building })
    }
}

/// `warehouse/UpgradeWarehouseMessage.java:18-64` — add one more rack, paying an
/// emerald block (`:45-64`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeWarehouse {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for UpgradeWarehouse {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(UpgradeWarehouse { colony_id, dimension, building })
    }
}

/// `worker/AddRemoveRecipeMessage.java:35-192` — teach a crafter a recipe, or forget
/// one (`:169-191`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddRemoveRecipe {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    /// The `IRecipeStorage` the factory wrote (`:149`) — bytes, because a recipe
    /// model is not part of this layer (`MANQUANT.md` §1).
    pub recipe: Vec<u8>,
    pub remove: bool,
    /// The crafting module's runtime id (`:163-168`).
    pub id: i32,
}

impl Message for AddRemoveRecipe {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("recipe".into(), bytes(&self.recipe));
        data.insert("remove".into(), boolean(self.remove));
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(AddRemoveRecipe {
            colony_id,
            dimension,
            building,
            recipe: bytes_of(data, "recipe")?,
            remove: bool_of(data, "remove")?,
            id: int_of(data, "id")?,
        })
    }
}

/// `worker/BuildingHiringModeMessage.java:16-71` — the hiring mode of any module that
/// assigns citizens (`:66-69`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildingHiringMode {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub mode: HiringMode,
    pub module_id: i32,
}

impl Message for BuildingHiringMode {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("mode".into(), text(self.mode.name()));
        data.insert("moduleId".into(), int(self.module_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        let mode = text_of(data, "mode")?;
        Ok(BuildingHiringMode {
            colony_id,
            dimension,
            building,
            mode: HiringMode::parse(&mode).ok_or(Refusal::Name { key: "mode" })?,
            module_id: int_of(data, "moduleId")?,
        })
    }
}

/// `worker/ChangeRecipePriorityMessage.java:15-105` — move a recipe up or down, by
/// one step or all the way (`:93-103`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeRecipePriority {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    /// The recipe's position in the list (`:20`, `:97`).
    pub recipe_location: i32,
    pub up: bool,
    pub id: i32,
    /// `switchOrder`'s third argument: move the whole block of equal recipes
    /// (`:97`).
    pub full_move: bool,
}

impl Message for ChangeRecipePriority {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("recipeLocation".into(), int(self.recipe_location));
        data.insert("up".into(), boolean(self.up));
        data.insert("id".into(), int(self.id));
        data.insert("fullMove".into(), boolean(self.full_move));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(ChangeRecipePriority {
            colony_id,
            dimension,
            building,
            recipe_location: int_in(data, "recipeLocation", 0, i32::MAX)?,
            up: bool_of(data, "up")?,
            id: int_of(data, "id")?,
            full_move: bool_of(data, "fullMove")?,
        })
    }
}

/// `worker/RecallCitizenMessage.java:30-98` — bring the hut's workers back, and
/// respawn the ones that are gone (`:60-96`).
///
/// The twin of [`super::buildings::RecallCitizenHut`] with one difference Java keeps:
/// this one also discards a citizen whose entity has never ticked (`:79-83`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallCitizen {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
}

impl Message for RecallCitizen {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(RecallCitizen { colony_id, dimension, building })
    }
}

/// `worker/ToggleRecipeMessage.java:15-71` — turn one taught recipe on or off
/// (`:65-69`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleRecipe {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub recipe_location: i32,
    pub id: i32,
}

impl Message for ToggleRecipe {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("recipeLocation".into(), int(self.recipe_location));
        data.insert("id".into(), int(self.id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = building_envelope_of(data)?;
        Ok(ToggleRecipe {
            colony_id,
            dimension,
            building,
            recipe_location: int_in(data, "recipeLocation", 0, i32::MAX)?,
            id: int_of(data, "id")?,
        })
    }
}

/// The twenty messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/colony/building/builder/BuilderSelectWorkOrderMessage.java", "BuilderSelectWorkOrder"),
    ("server/colony/building/enchanter/EnchanterWorkerSetMessage.java", "EnchanterWorkerSet"),
    ("server/colony/building/fields/AssignFieldMessage.java", "AssignField"),
    ("server/colony/building/fields/AssignmentModeMessage.java", "AssignmentMode"),
    ("server/colony/building/fields/FarmFieldPlotResizeMessage.java", "FarmFieldPlotResize"),
    ("server/colony/building/fields/FarmFieldRegistrationMessage.java", "FarmFieldRegistration"),
    ("server/colony/building/fields/FarmFieldUpdateSeedMessage.java", "FarmFieldUpdateSeed"),
    ("server/colony/building/guard/GuardSetMinePosMessage.java", "GuardSetMinePos"),
    ("server/colony/building/home/AssignUnassignMessage.java", "AssignUnassign"),
    ("server/colony/building/miner/MinerRepairLevelMessage.java", "MinerRepairLevel"),
    ("server/colony/building/miner/MinerSetLevelMessage.java", "MinerSetLevel"),
    ("server/colony/building/postbox/PostBoxRequestMessage.java", "PostBoxRequest"),
    ("server/colony/building/university/TryResearchMessage.java", "TryResearch"),
    ("server/colony/building/warehouse/SortBuildingMessage.java", "SortBuilding"),
    ("server/colony/building/warehouse/UpgradeWarehouseMessage.java", "UpgradeWarehouse"),
    ("server/colony/building/worker/AddRemoveRecipeMessage.java", "AddRemoveRecipe"),
    ("server/colony/building/worker/BuildingHiringModeMessage.java", "BuildingHiringMode"),
    ("server/colony/building/worker/ChangeRecipePriorityMessage.java", "ChangeRecipePriority"),
    ("server/colony/building/worker/RecallCitizenMessage.java", "RecallCitizen"),
    ("server/colony/building/worker/ToggleRecipeMessage.java", "ToggleRecipe"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn hut() -> (Dimension, i32, [i32; 3]) {
        (Dimension::new("minecraft:overworld"), 9, [10, 70, 10])
    }

    fn stack_of_items(name: &str) -> ItemStack {
        ItemStack::new(name, 1).expect("a valid stack")
    }

    #[test]
    fn the_work_order_and_the_two_miner_levels_round_trip() {
        let (dimension, colony_id, building) = hut();
        assert_eq!(
            round_trip(BuilderSelectWorkOrder {
                colony_id,
                dimension: dimension.clone(),
                building,
                work_order: 42
            })
            .work_order,
            42
        );
        assert_eq!(
            round_trip(MinerSetLevel { colony_id, dimension: dimension.clone(), building, level: 3 }).level,
            3
        );
        assert_eq!(
            round_trip(MinerRepairLevel { colony_id, dimension, building, level: 0 }).level,
            0
        );
    }

    #[test]
    fn a_field_plot_resize_round_trips_every_face_and_refuses_a_negative_size() {
        let plot = FarmFieldPlotResize { size: 4, direction: Face::West, position: [0, 63, 0] };
        assert_eq!(round_trip(plot).direction, Face::West);
        for face in Face::ALL {
            let plot = FarmFieldPlotResize { size: 1, direction: face, position: [1, 2, 3] };
            assert_eq!(round_trip(plot).direction, face);
        }
        let mut data =
            FarmFieldPlotResize { size: 0, direction: Face::East, position: [0, 0, 0] }.to_compound();
        data.insert("size".into(), int(-1));
        assert_eq!(
            FarmFieldPlotResize::from_compound(&data),
            Err(Refusal::Number { key: "size", min: 0, max: i64::from(MAX_FIELD_RANGE) })
        );
        data.insert("size".into(), int(0));
        data.insert("direction".into(), text("up"));
        assert_eq!(
            FarmFieldPlotResize::from_compound(&data),
            Err(Refusal::Name { key: "direction" })
        );
    }

    #[test]
    fn the_two_field_messages_carry_the_colony_envelope_and_the_seed() {
        let (dimension, colony_id, _) = hut();
        assert_eq!(
            round_trip(FarmFieldRegistration {
                colony_id,
                dimension: dimension.clone(),
                position: [4, 63, 4]
            })
            .position,
            [4, 63, 4]
        );
        let seeded = FarmFieldUpdateSeed {
            colony_id,
            dimension,
            new_seed: stack_of_items("minecraft:carrot"),
            position: [4, 63, 4],
        };
        assert_eq!(round_trip(seeded).new_seed.item.as_str(), "minecraft:carrot");
    }

    #[test]
    fn a_field_assignment_round_trips_its_opaque_extension_buffer() {
        let (dimension, colony_id, building) = hut();
        let field = AssignField {
            colony_id,
            dimension,
            building,
            module_id: 1,
            field_data: vec![7, 0, 1],
            assign: true,
        };
        assert_eq!(round_trip(field).field_data, vec![7, 0, 1]);
        assert!(
            !round_trip(AssignmentMode {
                colony_id,
                dimension: Dimension::new("minecraft:overworld"),
                building,
                id: 1,
                assignment_mode: false
            })
            .assignment_mode
        );
    }

    #[test]
    fn a_guard_without_a_mine_is_not_a_guard_at_the_origin() {
        let (dimension, colony_id, building) = hut();
        for mine in [None, Some([0, 0, 0]), Some([100, 40, -100])] {
            let guard = GuardSetMinePos {
                colony_id,
                dimension: dimension.clone(),
                building,
                mine_pos: mine,
            };
            assert_eq!(round_trip(guard).mine_pos, mine);
        }
        let without = GuardSetMinePos { colony_id, dimension, building, mine_pos: None };
        assert!(!without.to_compound().contains_key(&"minePos".into()), "Java writes nothing when unset");
    }

    #[test]
    fn a_citizen_can_be_assigned_to_a_house_or_to_a_job() {
        let (dimension, colony_id, building) = hut();
        for job in [None, Some(3)] {
            let assign = AssignUnassign {
                colony_id,
                dimension: dimension.clone(),
                building,
                assign: true,
                citizen_id: 17,
                job_entry: job,
            };
            assert_eq!(round_trip(assign).job_entry, job);
        }
    }

    #[test]
    fn a_post_box_request_refuses_a_quantity_of_nothing() {
        let (dimension, colony_id, building) = hut();
        let request = PostBoxRequest {
            colony_id,
            dimension,
            building,
            stack: stack_of_items("minecraft:oak_log"),
            deliver_available: true,
            req_quantity: 8,
        };
        assert_eq!(round_trip(request).req_quantity, 8);
        let mut data = PostBoxRequest {
            colony_id,
            dimension: Dimension::new("minecraft:overworld"),
            building,
            stack: stack_of_items("minecraft:oak_log"),
            deliver_available: false,
            req_quantity: 1,
        }
        .to_compound();
        data.insert("reqQuantity".into(), int(0));
        assert_eq!(
            PostBoxRequest::from_compound(&data),
            Err(Refusal::Number { key: "reqQuantity", min: 1, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn a_research_a_recipe_and_a_hiring_mode_round_trip() {
        let (dimension, colony_id, building) = hut();
        let research = TryResearch {
            colony_id,
            dimension: dimension.clone(),
            building,
            research_id: "minecolonies:research_cheese".to_owned(),
            branch: "minecolonies:technology".to_owned(),
            reset: true,
        };
        assert_eq!(round_trip(research).branch, "minecolonies:technology");

        let recipe = AddRemoveRecipe {
            colony_id,
            dimension: dimension.clone(),
            building,
            recipe: vec![1, 2, 3, 4],
            remove: false,
            id: 2,
        };
        assert_eq!(round_trip(recipe).recipe, vec![1, 2, 3, 4]);

        for mode in HiringMode::ALL {
            let hiring = BuildingHiringMode {
                colony_id,
                dimension: dimension.clone(),
                building,
                mode,
                module_id: 4,
            };
            assert_eq!(round_trip(hiring).mode, mode);
        }
    }

    #[test]
    fn a_recipe_may_move_one_step_or_all_the_way() {
        let (dimension, colony_id, building) = hut();
        let move_it = ChangeRecipePriority {
            colony_id,
            dimension: dimension.clone(),
            building,
            recipe_location: 3,
            up: true,
            id: 1,
            full_move: true,
        };
        let read = round_trip(move_it);
        assert_eq!((read.recipe_location, read.full_move), (3, true));
        assert_eq!(
            round_trip(ToggleRecipe {
                colony_id,
                dimension,
                building,
                recipe_location: 3,
                id: 1
            })
            .recipe_location,
            3
        );
    }

    #[test]
    fn the_three_envelope_only_messages_round_trip() {
        let (dimension, colony_id, building) = hut();
        round_trip(SortBuilding { colony_id, dimension: dimension.clone(), building });
        round_trip(UpgradeWarehouse { colony_id, dimension: dimension.clone(), building });
        round_trip(RecallCitizen { colony_id, dimension, building });
    }

    #[test]
    fn the_enchanter_and_the_faces_hold_their_names() {
        let (dimension, colony_id, building) = hut();
        let set = EnchanterWorkerSet {
            colony_id,
            dimension,
            building,
            worker: [1, 2, 3],
            add: true,
        };
        assert_eq!(round_trip(set).worker, [1, 2, 3]);
        for (index, face) in Face::ALL.iter().enumerate() {
            assert_eq!(face.index(), index as i32, "Direction.get2DDataValue order");
            assert_eq!(Face::parse(face.name()), Some(*face));
        }
        assert_eq!(Face::parse("south_up"), None);
        assert_eq!(CENSUS.len(), 20);
    }
}
