//! In what order a structure is placed, what is skipped at each position, and why.
//!
//! Java reference: `AbstractEntityAIStructure.structureStep`
//! (`:334-535`, the switch over the ten stages), the four skip predicates
//! (`skipDecorate` `:606-610`, `skipBuilding` `:620-626`, `skipClearing` `:636-648`,
//! `skipRemoval` `:658-672`), the weak-solid iterator (`:403-413`), the
//! non-solids iterator (`:418-425`), the fluid iterators (`:414-417`, `:443-448`),
//! `DONT_TOUCH_PREDICATE` (`:121-130`), `isDecoItem` (`:324-327`) and the three stage
//! arrays `loadStructure` picks from (`:762-786`).
//!
//! ## What this file adds to what already exists
//!
//! `crate::ai::facts` already says **which** stages a work order walks
//! ([`crate::ai::Stages`], the three arrays and the two `setTotalStages` numbers that
//! disagree with them). `crate::ai::builder` already says what the AI does with the
//! *result* of a step (`BuilderStep`, one variant per `return` of `structureStep`).
//! Neither answers the question the placer asks at one position, which is the one
//! this file answers:
//!
//! > at this stage, with this blueprint block and this world block, does the builder
//! > place, remove, drain, spawn — or skip, and because of which predicate?
//!
//! That question is what makes the **order** meaningful: `CLEAR` removes, then
//! `BUILD_FRAME` places, then `BUILD_SOLID` places, and a position can be skipped at
//! one stage and acted on at the next. The ten stages are the order; the predicates
//! are the exceptions; a skip is not a failure, and the port names it anyway so a
//! test can tell "nothing to do here" from "wrong stage".
//!
//! ## Two things worth knowing before reading the match
//!
//! **The iterator walks backwards in four stages, and by layers in a fifth.**
//! `CLEAR` (`:457`), `CLEAR_NON_SOLIDS` (`:421-424`), `REMOVE_WATER` (`:447`) and
//! `REMOVE` (`:451-452`) call `decrement`; the five build-ish stages call
//! `increment`; and `CLEAR_WATER` calls neither — `clearWaterStep`
//! (`StructurePlacer.java:507-557`) takes a whole Y layer, from the top down, and
//! returns `progressPos.below()`. That is why a new build clears from the top of the
//! box down and builds from the bottom up, and it is the one fact a resumed build
//! (the stored `progressPos`, `:499`) must get right — see [`walks`].
//!
//! **A skip predicate returns `true` to *leave the block alone*.** Every `increment`
//! / `decrement` above takes a "skip" predicate, so the conditions read inside out:
//! `skipBuilding` is true for a block that must **not** be placed at the frame or
//! solid stage (`:620-626`), and `skipDecorate` is true for a block that must not be
//! placed at the decorate stage (`:606-610`). The port keeps Java's polarity and
//! calls the answers [`Skip`], never `false`.
//!
//! **`DONT_TOUCH_PREDICATE` is not a block test.** It is true for a world block that
//! is an `IBuilderUndestroyable`, bedrock, or the hut block at the blueprint's own
//! centre (`:121-130`) — the last one needs the blueprint's centre and the block
//! entity type, neither of which a fact can carry. So the caller computes it and
//! hands it over as one boolean ([`StepFacts::dont_touch`]); every predicate that
//! Java combines with it does the same here.

use crate::ai::{OrderKind, Stage};

/// Where this file's rules come from, as one string a log line can print.
pub const SOURCE: &str = "AbstractEntityAIStructure.java:121-130,:324-327,:334-535,:606-672,:762-786";

/// One world block or one blueprint block, as the five skip predicates see it.
///
/// Every field is a **vanilla or Structurize predicate Java calls by name**, not a
/// guess: `isWater` (`BlockUtils.java:325-328`), `isAnySolid` /
/// `canBlockFloatInAir` / `isWeakSolidBlock` (`BlockUtils.java:780-862`),
/// `isDecoItem` (`AbstractEntityAIStructure.java:324-327`), the three substitution
/// blocks and `IBuilderUndestroyable`/`BEDROCK` (`:645-646`, `:661-671`).
///
/// The fields are not exclusive on purpose: vanilla water is also a fluid, a
/// substitution block can also be a decoration item, and a report that says `water
/// && fluid` is telling the truth. Java asks one predicate at a time, so this record
/// holds one answer per predicate instead of forcing a label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockFacts {
    /// `state.isAir()`.
    pub air: bool,
    /// `BlockUtils.isWater(state)` (`:325-328`): exactly `Blocks.WATER`.
    pub water: bool,
    /// `!state.getFluidState().isEmpty()`: water, lava, a bubble column.
    pub fluid: bool,
    /// `state.is(BlockTags.LEAVES)` (`:1004`): free of charge for a build.
    pub leaves: bool,
    /// `BlockUtils.canBlockFloatInAir(state)` (`BlockUtils.java:780-787`): the block
    /// stays where it is put, so it belongs to `BUILD_SOLID`.
    pub floats: bool,
    /// `BlockUtils.isWeakSolidBlock(state)` (`BlockUtils.java:808-822`): it needs
    /// support, so it belongs to `WEAK_SOLID`.
    pub weak_solid: bool,
    /// `isDecoItem(state.getBlock())` (`:324-327`): a decoration tag member, or a
    /// fluid substitution block.
    pub deco_item: bool,
    /// `block.getBlock() == ModBlocks.blockDecorationPlaceholder` (`:1005`).
    pub decoration_placeholder: bool,
    /// `blockSolidSubstitution` or `blockSubstitution` (`:663-664`).
    pub substitution: bool,
    /// `blockFluidSubstitution` (`:638`, `:665`).
    pub fluid_substitution: bool,
    /// `state.getBlock() instanceof IBuilderUndestroyable || state.getBlock() ==
    /// Blocks.BEDROCK` (`:645-646`, `:671`, `:125-126`).
    pub undestroyable: bool,
}

impl BlockFacts {
    /// Air: what a position nobody recorded reads as.
    pub fn air() -> BlockFacts {
        BlockFacts {
            air: true,
            ..BlockFacts::default()
        }
    }

    /// `Blocks.WATER`: a fluid, and the only block `isWater` accepts.
    pub fn water() -> BlockFacts {
        BlockFacts {
            water: true,
            fluid: true,
            ..BlockFacts::default()
        }
    }

    /// Any other fluid (lava, a bubble column): a fluid, not water.
    pub fn fluid() -> BlockFacts {
        BlockFacts {
            fluid: true,
            ..BlockFacts::default()
        }
    }

    /// A block that stays where it is put (`canBlockFloatInAir`).
    pub fn solid() -> BlockFacts {
        BlockFacts {
            floats: true,
            ..BlockFacts::default()
        }
    }

    /// A block that needs support (`isWeakSolidBlock`): a pane, a fence, ticking
    /// leaves.
    pub fn weak_solid() -> BlockFacts {
        BlockFacts {
            weak_solid: true,
            ..BlockFacts::default()
        }
    }

    /// A decoration item (`isDecoItem`), which `DECORATE` places and the two solid
    /// stages skip.
    pub fn deco() -> BlockFacts {
        BlockFacts {
            deco_item: true,
            ..BlockFacts::default()
        }
    }

    /// `Blocks.OAK_LEAVES`: free of charge, and weak-solid when it ticks.
    pub fn leaves() -> BlockFacts {
        BlockFacts {
            leaves: true,
            weak_solid: true,
            ..BlockFacts::default()
        }
    }

    /// Bedrock or an `IBuilderUndestroyable`: nothing may touch it.
    pub fn undestroyable() -> BlockFacts {
        BlockFacts {
            undestroyable: true,
            ..BlockFacts::default()
        }
    }

    /// A Structurize substitution block (`blockSubstitution`).
    pub fn substitution() -> BlockFacts {
        BlockFacts {
            substitution: true,
            ..BlockFacts::default()
        }
    }

    /// A fluid substitution block, which is also a decoration item (`:324-327`).
    pub fn fluid_substitution() -> BlockFacts {
        BlockFacts {
            fluid_substitution: true,
            substitution: true,
            deco_item: true,
            ..BlockFacts::default()
        }
    }

    /// `BlockUtils.isAnySolid` (`BlockUtils.java:858-861`): floats **or** weak.
    pub fn is_any_solid(self) -> bool {
        self.floats || self.weak_solid
    }

    /// The water test the two clearing predicates and `isBlockFree` share
    /// (`BlockUtils.java:325-328`).
    pub fn is_water(self) -> bool {
        self.water
    }
}

/// The two facts a position needs that are not a block.
///
/// Java reference: `DONT_TOUCH_PREDICATE` (`:121-130`) and
/// `placer.getHandler().getBluePrint().getEntities().length` (`:432`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepFacts {
    /// `DONT_TOUCH_PREDICATE.test(info, worldPos, handler)`: the world block there
    /// is undestroyable, bedrock, or the hut at the blueprint's centre.
    pub dont_touch: bool,
    /// The blueprint holds at least one entity (`:432-435`).
    pub blueprint_has_entities: bool,
}

/// Why a position is left alone at this stage. One variant per predicate Java
/// evaluates, with the line that evaluates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the skip predicates of `structureStep` and nothing else
pub enum Skip {
    /// `DONT_TOUCH_PREDICATE` (`:411`, `:424`, `:440`, `:609`, `:625`).
    DontTouch,
    /// `skipBuilding` (`:623`): the blueprint block cannot exist without support, so
    /// `BUILD_FRAME` and `BUILD_SOLID` leave it to `WEAK_SOLID`.
    CannotFloatInAir,
    /// `skipBuilding` (`:624`): a decoration is placed at `DECORATE`, later.
    Decoration,
    /// `skipDecorate` (`:609`): a solid that is **not** a decoration — it was placed
    /// in an earlier stage.
    SolidNotDecoration,
    /// The `WEAK_SOLID` iterator (`:410-411`): the blueprint block is not a weak
    /// solid, so this stage is not its turn.
    NotWeakSolid,
    /// `skipRemoval` (`:662-668`): the blueprint holds air or a substitution there,
    /// so there is nothing to take away.
    NothingToRemove,
    /// `skipRemoval` (`:670-671`): the world holds air, water or something
    /// undestroyable, so the removal would be a no-op or an accident.
    AlreadyGone,
    /// `skipClearing` (`:638-641`): the blueprint wants a fluid substitution here —
    /// the water stage owns that position.
    FluidSubstitution,
    /// `skipClearing` (`:644-647`): the world block is undestroyable, bedrock, air or
    /// a fluid.
    Protected,
    /// `CLEAR_NON_SOLIDS` (`:421-424`): the blueprint block is **not** air, so this
    /// stage does not clear it — `BUILD_*` will place it.
    NotAirInBlueprint,
    /// `CLEAR_NON_SOLIDS` (`:423`): the world is already empty there.
    WorldEmpty,
    /// `SPAWN` (`:440`): the blueprint has no entity at this position.
    NoEntity,
    /// `REMOVE_WATER` (`:447`): the blueprint block carries no fluid.
    NoFluidInBlueprint,
}

/// What the placer does at a position when the stage does not skip it.
///
/// Java reference: the `StructurePlacer.Operation` each stage passes
/// (`:395-457`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four operations `structureStep` passes to the placer
pub enum Operation {
    /// `BLOCK_PLACEMENT` (`:395`, `:400`, `:408`, `:420`, `:428`).
    Place,
    /// `BLOCK_REMOVAL` (`:451`, `:457`).
    Remove,
    /// `WATER_REMOVAL`, or `clearWaterStep` (`:416`, `:446`).
    ClearWater,
    /// `SPAWN_ENTITY` (`:439`).
    SpawnEntity,
}

/// The answer for one position at one stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: act or skip
pub enum Decision {
    /// The placer acts, with this operation.
    Act(Operation),
    /// The iterator moves on, because of this predicate. Not an error: a skip is how
    /// the ten stages divide one blueprint between them.
    Skip(Skip),
}

impl Decision {
    /// The operation, when there is one.
    pub fn operation(self) -> Option<Operation> {
        match self {
            Decision::Act(operation) => Some(operation),
            Decision::Skip(_) => None,
        }
    }

    /// True when the position is left alone.
    pub fn skipped(self) -> bool {
        matches!(self, Decision::Skip(_))
    }
}

/// `isDecoItem` (`:324-327`), as the two calls that need it read it.
pub fn is_decoration(block: BlockFacts) -> bool {
    block.deco_item || block.fluid_substitution
}

/// `skipDecorate` (`:606-610`): skip unless it is a decoration item, and skip the
/// world block Java must not touch.
pub fn skip_decorate(blueprint: BlockFacts, step: StepFacts) -> Option<Skip> {
    if step.dont_touch {
        return Some(Skip::DontTouch); // `:609`
    }
    if !is_decoration(blueprint) && blueprint.is_any_solid() {
        // `(!isDecoItem(...) && BlockUtils.isAnySolid(...))`
        return Some(Skip::SolidNotDecoration);
    }
    None
}

/// `skipBuilding` (`:620-626`): the predicate both solid stages pass.
///
/// A block is placed at `BUILD_FRAME`/`BUILD_SOLID` only if it can float in air and
/// is not a decoration. Everything else is a weak solid, a decoration, or somebody
/// else's business.
///
/// Java's predicate is one boolean OR (`!canBlockFloatInAir(...) || isDecoItem(...)
/// || DONT_TOUCH(...)`), so it has no "which term" at all. The port answers one
/// [`Skip`] and must pick an order for the two block terms; it tests them in Java's
/// order, so a decoration that could not float is reported as
/// [`Skip::CannotFloatInAir`] and a decoration that **could** as [`Skip::Decoration`].
/// The decision (skip) is the same either way.
pub fn skip_building(blueprint: BlockFacts, step: StepFacts) -> Option<Skip> {
    if step.dont_touch {
        return Some(Skip::DontTouch); // `:625`
    }
    if !blueprint.floats {
        return Some(Skip::CannotFloatInAir); // `:623`
    }
    if is_decoration(blueprint) {
        return Some(Skip::Decoration); // `:624`
    }
    None
}

/// `skipClearing` (`:636-648`): the `CLEAR` stage's predicate, which looks at the
/// **world** as well.
pub fn skip_clearing(blueprint: BlockFacts, world: BlockFacts) -> Option<Skip> {
    if blueprint.fluid_substitution {
        return Some(Skip::FluidSubstitution); // `:638-641`
    }
    if world.undestroyable || world.air || world.fluid {
        return Some(Skip::Protected); // `:644-647`
    }
    None
}

/// `skipRemoval` (`:658-672`): the `REMOVE` stage's predicate.
pub fn skip_removal(blueprint: BlockFacts, world: BlockFacts) -> Option<Skip> {
    if blueprint.air
        || blueprint.substitution
        || blueprint.fluid_substitution
        || blueprint.decoration_placeholder
    {
        return Some(Skip::NothingToRemove); // `:661-668`
    }
    if world.air || world.undestroyable || world.water {
        return Some(Skip::AlreadyGone); // `:670-671`
    }
    None
}

/// What `structureStep` does at one position (`:379-459`), stage by stage.
///
/// The match is the switch, in the same order, and each arm names the iterator Java
/// builds for it.
pub fn decide(stage: Stage, blueprint: BlockFacts, world: BlockFacts, step: StepFacts) -> Decision {
    if step.dont_touch {
        // `DONT_TOUCH_PREDICATE` is part of four of the five predicates; `CLEAR_WATER`
        // has none, and is handled below.
        match stage {
            Stage::ClearWater | Stage::RemoveWater | Stage::Spawn => {}
            _ => return Decision::Skip(Skip::DontTouch),
        }
    }
    match stage {
        // `:454-458` — `decrement(this::skipClearing)`, `BLOCK_REMOVAL`.
        Stage::Clear => match skip_clearing(blueprint, world) {
            Some(skip) => Decision::Skip(skip),
            None => Decision::Act(Operation::Remove),
        },
        // `:381-397` — `frameIterator.increment(this::skipBuilding)`.
        Stage::BuildFrame => match skip_building(blueprint, step) {
            Some(skip) => Decision::Skip(skip),
            None => Decision::Act(Operation::Place),
        },
        // `:398-402` — `increment(this::skipBuilding)`.
        Stage::BuildSolid => match skip_building(blueprint, step) {
            Some(skip) => Decision::Skip(skip),
            None => Decision::Act(Operation::Place),
        },
        // `:403-413` — `increment(!isWeakSolidBlock || DONT_TOUCH)`.
        Stage::WeakSolid => {
            if step.dont_touch {
                Decision::Skip(Skip::DontTouch) // `:411`
            } else if !blueprint.weak_solid {
                Decision::Skip(Skip::NotWeakSolid)
            } else {
                Decision::Act(Operation::Place)
            }
        }
        // `:414-417` — `clearWaterStep`, no predicate at all.
        Stage::ClearWater => Decision::Act(Operation::ClearWater),
        // `:418-425` — `decrement(!air || world empty || DONT_TOUCH)`.
        Stage::ClearNonSolids => {
            if step.dont_touch {
                Decision::Skip(Skip::DontTouch) // `:424`
            } else if !blueprint.air {
                Decision::Skip(Skip::NotAirInBlueprint) // `:422`
            } else if world.air {
                Decision::Skip(Skip::WorldEmpty) // `:423`
            } else {
                Decision::Act(Operation::Remove)
            }
        }
        // `:426-430` — `increment(this::skipDecorate)`.
        Stage::Decorate => match skip_decorate(blueprint, step) {
            Some(skip) => Decision::Skip(skip),
            None => Decision::Act(Operation::Place),
        },
        // `:431-442` — `increment(info.getEntities().length == 0 || DONT_TOUCH)`;
        // an empty blueprint finishes the stage at once (`:432-435`).
        Stage::Spawn => {
            if step.dont_touch {
                Decision::Skip(Skip::DontTouch) // `:440`
            } else if !step.blueprint_has_entities {
                Decision::Skip(Skip::NoEntity)
            } else {
                Decision::Act(Operation::SpawnEntity)
            }
        }
        // `:449-453` — `decrement(this::skipRemoval)`, `BLOCK_REMOVAL`, removing.
        Stage::Remove => match skip_removal(blueprint, world) {
            Some(skip) => Decision::Skip(skip),
            None => Decision::Act(Operation::Remove),
        },
        // `:443-448` — `decrement(blueprint has no fluid)`, `WATER_REMOVAL`.
        Stage::RemoveWater => {
            if blueprint.fluid {
                Decision::Act(Operation::ClearWater)
            } else {
                Decision::Skip(Skip::NoFluidInBlueprint) // `:447`
            }
        }
    }
}

/// Which way the placer's cursor moves during a stage.
///
/// Java reference: the `increment`/`decrement` of each arm of `:379-459`, and the
/// one arm that does neither — `clearWaterStep` (`:416`, `StructurePlacer.java:507-557`)
/// sweeps **one Y layer at a time, from the top down**: it starts at
/// `size.getY() - 1`, returns `progressPos.below()` each call, and answers `FINISHED`
/// when the layer reaches zero. Four stages walk the blueprint backwards and that is
/// deliberate: a new build clears the box from its top so nothing falls on the
/// builder, and removes from the top so a wall does not collapse into the block
/// already taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two iterator methods and one layer sweep
pub enum Direction {
    /// `placer.getIterator().increment(...)`.
    Forward,
    /// `placer.getIterator().decrement(...)`.
    Backward,
    /// `placer.clearWaterStep(...)` (`StructurePlacer.java:507-557`): a whole Y layer
    /// per call, downwards.
    Layers,
}

/// The direction each stage walks (`:395-457`).
pub fn walks(stage: Stage) -> Direction {
    match stage {
        // `:457`, `:421-424`, `:447`, `:451-452`
        Stage::Clear | Stage::ClearNonSolids | Stage::RemoveWater | Stage::Remove => {
            Direction::Backward
        }
        // `:396`, `:401`, `:410`, `:428`, `:439`
        Stage::BuildFrame | Stage::BuildSolid | Stage::WeakSolid | Stage::Decorate | Stage::Spawn => {
            Direction::Forward
        }
        // `:416` — neither: `clearWaterStep` owns the whole layer.
        Stage::ClearWater => Direction::Layers,
    }
}

/// Why a stage cannot be walked by a given work order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one way to ask for a stage that does not exist in this order
pub enum Refusal {
    /// The stage is not in this order's array (`:762-786`): a `REMOVE` order has no
    /// `BUILD_SOLID`, and an upgrade has no `CLEAR`. Java would walk past it — the
    /// saved `progressPos` of a reused work order can hold it (`:360-363`), and the
    /// placer would then be asked for a stage it was never given.
    StageNotInOrder { stage: Stage, order: OrderKind },
}

/// Does this work order walk this stage at all? (`:762-786`)
///
/// The three arrays live in [`crate::ai::Stages`]; this asks them rather than
/// restating them.
pub fn accepts(order: OrderKind, stage: Stage) -> Result<(), Refusal> {
    if order.stages().list().contains(&stage) {
        Ok(())
    } else {
        Err(Refusal::StageNotInOrder { stage, order })
    }
}

/// Is `first` walked before `second` in this order? Both must be in the array.
pub fn before(order: OrderKind, first: Stage, second: Stage) -> bool {
    let list = order.stages().list();
    match (
        list.iter().position(|stage| *stage == first),
        list.iter().position(|stage| *stage == second),
    ) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// The stage list of an order, in order, as the caller can print it.
pub fn order_of(order: OrderKind) -> Vec<Stage> {
    order.stages().list().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step() -> StepFacts {
        StepFacts::default()
    }

    /// A new build clears, then frames, then builds: the order the eight-stage array
    /// gives it (`:782-786`).
    #[test]
    fn a_new_build_clears_before_it_places_anything() {
        assert!(before(OrderKind::Build, Stage::Clear, Stage::BuildFrame));
        assert!(before(OrderKind::Build, Stage::BuildFrame, Stage::BuildSolid));
        assert!(before(OrderKind::Build, Stage::BuildSolid, Stage::Decorate));
        assert!(before(OrderKind::Build, Stage::Decorate, Stage::Spawn));
        // Two stages the other way round are not "before" either way.
        assert!(!before(OrderKind::Build, Stage::Spawn, Stage::Clear));
    }

    /// An upgrade skips `CLEAR` and `BUILD_FRAME`: its array starts at
    /// `BUILD_SOLID` (`:775-778`).
    #[test]
    fn an_upgrade_has_no_clear_and_no_frame() {
        assert_eq!(
            accepts(OrderKind::Upgrade, Stage::Clear),
            Err(Refusal::StageNotInOrder {
                stage: Stage::Clear,
                order: OrderKind::Upgrade
            })
        );
        assert_eq!(
            accepts(OrderKind::Upgrade, Stage::BuildFrame),
            Err(Refusal::StageNotInOrder {
                stage: Stage::BuildFrame,
                order: OrderKind::Upgrade
            })
        );
        assert_eq!(accepts(OrderKind::Upgrade, Stage::BuildSolid), Ok(()));
        assert!(before(OrderKind::Upgrade, Stage::BuildSolid, Stage::Spawn));
    }

    /// A deconstruction holds two stages and nothing else (`:767-770`).
    #[test]
    fn a_removal_walks_two_stages_water_first() {
        assert_eq!(order_of(OrderKind::Remove), vec![Stage::RemoveWater, Stage::Remove]);
        assert!(before(OrderKind::Remove, Stage::RemoveWater, Stage::Remove));
        assert_eq!(
            accepts(OrderKind::Remove, Stage::BuildSolid),
            Err(Refusal::StageNotInOrder {
                stage: Stage::BuildSolid,
                order: OrderKind::Remove
            })
        );
    }

    /// `:396`, `:401` … against `:445`, `:451`, `:457`: the four stages that walk
    /// backwards are exactly the four removal-ish ones — and `CLEAR_WATER` is neither,
    /// because `clearWaterStep` takes a layer at a time (`StructurePlacer.java:507-557`).
    #[test]
    fn the_four_removal_stages_walk_backwards_and_the_water_stage_by_layers() {
        let backward: Vec<Stage> = [
            Stage::Clear,
            Stage::BuildFrame,
            Stage::BuildSolid,
            Stage::WeakSolid,
            Stage::ClearWater,
            Stage::ClearNonSolids,
            Stage::Decorate,
            Stage::Spawn,
            Stage::Remove,
            Stage::RemoveWater,
        ]
        .into_iter()
        .filter(|stage| walks(*stage) == Direction::Backward)
        .collect();
        assert_eq!(
            backward,
            vec![
                Stage::Clear,
                Stage::ClearNonSolids,
                Stage::Remove,
                Stage::RemoveWater
            ]
        );
        assert_eq!(walks(Stage::ClearWater), Direction::Layers);
        assert_eq!(walks(Stage::BuildSolid), Direction::Forward);
    }

    /// `:620-626`: a solid is placed at both solid stages; a weak solid is not.
    #[test]
    fn the_solid_stages_place_only_what_can_float_in_air() {
        assert_eq!(
            decide(Stage::BuildSolid, BlockFacts::solid(), BlockFacts::air(), step()),
            Decision::Act(Operation::Place)
        );
        assert_eq!(
            decide(Stage::BuildFrame, BlockFacts::weak_solid(), BlockFacts::air(), step()),
            Decision::Skip(Skip::CannotFloatInAir)
        );
        // A decoration that cannot stand on its own is reported by the first term of
        // Java's OR (`:623`), which is the one tested first.
        assert_eq!(
            decide(Stage::BuildSolid, BlockFacts::deco(), BlockFacts::air(), step()),
            Decision::Skip(Skip::CannotFloatInAir)
        );
        // A decoration that *could* stand on its own is reported by the second term
        // (`:624`) — the block waits for `DECORATE`.
        let floating_deco = BlockFacts {
            floats: true,
            deco_item: true,
            ..BlockFacts::default()
        };
        assert_eq!(
            decide(Stage::BuildSolid, floating_deco, BlockFacts::air(), step()),
            Decision::Skip(Skip::Decoration)
        );
    }

    /// `:403-413`: the `WEAK_SOLID` stage takes exactly what the solid stages
    /// refused, and nothing else.
    #[test]
    fn the_weak_solid_stage_takes_what_the_solid_stages_refused() {
        assert_eq!(
            decide(Stage::WeakSolid, BlockFacts::weak_solid(), BlockFacts::air(), step()),
            Decision::Act(Operation::Place)
        );
        assert_eq!(
            decide(Stage::WeakSolid, BlockFacts::solid(), BlockFacts::air(), step()),
            Decision::Skip(Skip::NotWeakSolid)
        );
    }

    /// `:606-610`: `DECORATE` places a decoration and skips the solid that was
    /// already placed.
    #[test]
    fn decorate_places_decorations_and_skips_the_solids() {
        assert_eq!(
            decide(Stage::Decorate, BlockFacts::deco(), BlockFacts::air(), step()),
            Decision::Act(Operation::Place)
        );
        assert_eq!(
            decide(Stage::Decorate, BlockFacts::solid(), BlockFacts::air(), step()),
            Decision::Skip(Skip::SolidNotDecoration)
        );
        // A non-solid decoration is still placed: the predicate only skips solids.
        let odd = BlockFacts {
            deco_item: true,
            ..BlockFacts::default()
        };
        assert_eq!(
            decide(Stage::Decorate, odd, BlockFacts::air(), step()),
            Decision::Act(Operation::Place)
        );
    }

    /// `:636-648`: `CLEAR` removes what is there, except bedrock, fluids and the
    /// positions a fluid substitution owns.
    #[test]
    fn clear_removes_but_never_bedrock_or_a_fluid() {
        assert_eq!(
            decide(Stage::Clear, BlockFacts::solid(), BlockFacts::solid(), step()),
            Decision::Act(Operation::Remove)
        );
        assert_eq!(
            decide(
                Stage::Clear,
                BlockFacts::solid(),
                BlockFacts::undestroyable(),
                step()
            ),
            Decision::Skip(Skip::Protected)
        );
        assert_eq!(
            decide(Stage::Clear, BlockFacts::solid(), BlockFacts::water(), step()),
            Decision::Skip(Skip::Protected)
        );
        assert_eq!(
            decide(
                Stage::Clear,
                BlockFacts::fluid_substitution(),
                BlockFacts::solid(),
                step()
            ),
            Decision::Skip(Skip::FluidSubstitution)
        );
    }

    /// `:658-672`: a deconstruction takes the blueprint's block out, unless the
    /// blueprint holds air or a substitution there, or the world has nothing left.
    #[test]
    fn remove_takes_the_blueprint_block_out_one_by_one() {
        assert_eq!(
            decide(Stage::Remove, BlockFacts::solid(), BlockFacts::solid(), step()),
            Decision::Act(Operation::Remove)
        );
        assert_eq!(
            decide(Stage::Remove, BlockFacts::air(), BlockFacts::solid(), step()),
            Decision::Skip(Skip::NothingToRemove)
        );
        assert_eq!(
            decide(
                Stage::Remove,
                BlockFacts::substitution(),
                BlockFacts::solid(),
                step()
            ),
            Decision::Skip(Skip::NothingToRemove)
        );
        assert_eq!(
            decide(Stage::Remove, BlockFacts::solid(), BlockFacts::air(), step()),
            Decision::Skip(Skip::AlreadyGone)
        );
        assert_eq!(
            decide(Stage::Remove, BlockFacts::solid(), BlockFacts::water(), step()),
            Decision::Skip(Skip::AlreadyGone)
        );
    }

    /// `:418-425`: `CLEAR_NON_SOLIDS` only touches a position the blueprint leaves
    /// empty **and** the world fills.
    #[test]
    fn clear_non_solids_only_empties_the_positions_the_blueprint_leaves_air() {
        assert_eq!(
            decide(
                Stage::ClearNonSolids,
                BlockFacts::air(),
                BlockFacts::solid(),
                step()
            ),
            Decision::Act(Operation::Remove)
        );
        assert_eq!(
            decide(
                Stage::ClearNonSolids,
                BlockFacts::solid(),
                BlockFacts::solid(),
                step()
            ),
            Decision::Skip(Skip::NotAirInBlueprint)
        );
        assert_eq!(
            decide(
                Stage::ClearNonSolids,
                BlockFacts::air(),
                BlockFacts::air(),
                step()
            ),
            Decision::Skip(Skip::WorldEmpty)
        );
    }

    /// `:443-448`: `REMOVE_WATER` only drains where the blueprint holds a fluid.
    #[test]
    fn remove_water_only_drains_a_position_the_blueprint_fills_with_water() {
        assert_eq!(
            decide(Stage::RemoveWater, BlockFacts::water(), BlockFacts::water(), step()),
            Decision::Act(Operation::ClearWater)
        );
        assert_eq!(
            decide(Stage::RemoveWater, BlockFacts::solid(), BlockFacts::water(), step()),
            Decision::Skip(Skip::NoFluidInBlueprint)
        );
    }

    /// `:414-417`: `CLEAR_WATER` has no predicate — it runs on every position.
    #[test]
    fn clear_water_has_no_predicate_at_all() {
        assert_eq!(
            decide(
                Stage::ClearWater,
                BlockFacts::air(),
                BlockFacts::undestroyable(),
                StepFacts {
                    dont_touch: true,
                    blueprint_has_entities: false
                }
            ),
            Decision::Act(Operation::ClearWater)
        );
    }

    /// `:431-442`: `SPAWN` needs entities to spawn, and skips the position otherwise.
    #[test]
    fn spawn_needs_an_entity_in_the_blueprint() {
        let with = StepFacts {
            dont_touch: false,
            blueprint_has_entities: true,
        };
        assert_eq!(
            decide(Stage::Spawn, BlockFacts::air(), BlockFacts::air(), with),
            Decision::Act(Operation::SpawnEntity)
        );
        assert_eq!(
            decide(Stage::Spawn, BlockFacts::air(), BlockFacts::air(), step()),
            Decision::Skip(Skip::NoEntity)
        );
    }

    /// `:121-130`: whatever the stage, a world block that must not be touched is
    /// skipped — `CLEAR_WATER` being the one stage with no predicate.
    #[test]
    fn the_untouchable_world_block_wins_over_every_predicate() {
        let guarded = StepFacts {
            dont_touch: true,
            blueprint_has_entities: true,
        };
        for stage in [
            Stage::Clear,
            Stage::BuildFrame,
            Stage::BuildSolid,
            Stage::WeakSolid,
            Stage::ClearNonSolids,
            Stage::Decorate,
            Stage::Spawn,
            Stage::Remove,
        ] {
            assert_eq!(
                decide(stage, BlockFacts::solid(), BlockFacts::solid(), guarded),
                Decision::Skip(Skip::DontTouch),
                "{stage:?} must not touch it"
            );
        }
        assert!(decide(Stage::ClearWater, BlockFacts::solid(), BlockFacts::solid(), guarded)
            .operation()
            .is_some());
    }

    /// Every stage answers something, and the operation is the one Java passes
    /// (`:395-457`) — no stage falls through to a default.
    #[test]
    fn each_stage_names_its_operation() {
        let blueprint = BlockFacts {
            floats: true,
            weak_solid: true,
            deco_item: true,
            air: true,
            fluid: true,
            ..BlockFacts::default()
        };
        let world = BlockFacts::solid();
        let with = StepFacts {
            dont_touch: false,
            blueprint_has_entities: true,
        };
        assert_eq!(
            decide(Stage::BuildSolid, blueprint, world, with),
            // A floating block wins over the decoration flag: `skipBuilding` tests
            // `floats` first and then the decoration (`:623-624`).
            Decision::Skip(Skip::Decoration)
        );
        assert_eq!(
            decide(Stage::WeakSolid, blueprint, world, with).operation(),
            Some(Operation::Place)
        );
        assert_eq!(
            decide(Stage::RemoveWater, blueprint, world, with).operation(),
            Some(Operation::ClearWater)
        );
        assert_eq!(
            decide(Stage::ClearNonSolids, blueprint, world, with).operation(),
            Some(Operation::Remove)
        );
        assert!(SOURCE.contains(":606-672"));
    }
}
