//! Putting a hut down: the checks first, then the writes **in Java's order**.
//!
//! Java reference: `core/colony/managers/RegisteredStructureManager.java:556-619`
//! (`addNewBuilding`, the method a hut block's registration runs),
//! `core/colony/buildings/AbstractBuilding.java:305-313` (`onPlacement`) and
//! `:789-830` (`requestUpgrade`'s sibling, `requestRemoval`),
//! `core/blocks/huts/BlockHutTavern.java:50-64` and
//! `BlockHutTownHall.java:219-234` (the two per-kind placement refusals), and
//! `api/blocks/AbstractBlockHut.java:305-338` (the colony's own rules, already
//! ported in `src/building/requirements.rs` and reached here through
//! `src/founding/placement.rs`, not re-done).
//!
//! **What this file adds to `founding::placement::plan`.** That function answers
//! *may this hut stand here*; it is the check half of one of the four steps below
//! and it is reused, not copied. What was missing is what happens **after** the
//! answer is yes: Java then makes seven writes, in an order that matters because
//! three of them are what make the hut real (the record, the level, the claim) and
//! two are what make it reachable (the resolvers, the chunk extent).
//!
//! **The order, and where each line comes from.**
//!
//! | # | [`Write`] | Java |
//! | ---: | --- | --- |
//! | 1 | `Record` | `RegisteredStructureManager.java:562-570` — `createFrom`, then the map insert |
//! | 2 | `SideList` | `:573-590` — `addBuilding`'s three lists |
//! | 3 | `Level` | `:571` — `upgradeBuildingLevelToSchematicData` |
//! | 4 | `Parent` | `AbstractSchematicProvider.java:309-321` |
//! | 5 | `ClaimChunks` | `AbstractBuilding.java:307-313` — `onPlacement` |
//! | 6 | `RegisterResolvers` | `:606` — `onProviderAddedToColony` |
//! | 7 | `ExtendExtent` | `:611` — `setMaxChunk` |
//!
//! **A refusal costs nothing, by construction.** Every check runs before the
//! journal is built and before the colony record is touched; the colony is taken
//! as `&mut` only so the one write the port can make — appending the record — has
//! somewhere to go. A test proves it by cloning the colony first and comparing
//! after the refusal, which is a measurement and not a promise.
//!
//! **What is deliberately not a check here.** The player's right to place a hut
//! (`Action::PlaceHuts`, `AbstractBlockHut.java:331-336`) belongs to the hook that
//! has a player, and `founding::placement` says the same about its own layer. The
//! world's height limits (`AbstractBuilding.java:473-486`) belong to the caller
//! that has a world.

use std::collections::BTreeMap;

use crate::building::{self, Building, Footprint, Kind, Site};
use crate::colony::{Building as Record, Colony};
use crate::founding::{self, Ground};

use super::{Refusal, claim_box, claim_radius, chunk_of, modules};

/// The three side lists `addBuilding` keeps beside the position map
/// (`RegisteredStructureManager.java:573-590`).
///
/// They are not decoration: `hasTownHall`, `hasWareHouse` and `hasMysticalSite`
/// are read on the colony tick, and the colony's town hall is *how* its centre is
/// known (`:518-527`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three lists in Java, and a fourth would be a new field there too
pub enum SideList {
    TownHall,
    Warehouse,
    MysticalSite,
}

impl SideList {
    /// The list a kind joins, if any. Java tests `instanceof`, which is one
    /// comparison per override; the port tests the kind, which is the same set.
    pub fn of(kind: Kind) -> Option<SideList> {
        match kind {
            Kind::TownHall => Some(SideList::TownHall),
            Kind::Warehouse => Some(SideList::Warehouse),
            Kind::MysticalSite => Some(SideList::MysticalSite),
            _ => None,
        }
    }
}

/// One write the placement makes, in the order it makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the seven writes of addNewBuilding, closed on purpose
pub enum Write {
    /// The building joins the colony's list. Its identity is its position, so
    /// this is the map insert of `RegisteredStructureManager.java:564`.
    Record { at: [i32; 3], kind: Kind },
    /// A town hall, a warehouse or a mystical site also joins its side list.
    SideList { at: [i32; 3], list: SideList },
    /// The level read from the schematic (`AbstractSchematicProvider.java:524-566`).
    /// `0` for a hut placed as a block with no blueprint read: Java's "placed and
    /// not built" (`:64`).
    Level { at: [i32; 3], level: i32 },
    /// The parent link, written when the tile entity registers its children
    /// (`BuildingBarracks.java:112-126`, `AbstractSchematicProvider.java:309-321`).
    Parent { at: [i32; 3], parent: [i32; 3] },
    /// The chunks the hut holds: `claimBuildingChunks(colony, true, position,
    /// getClaimRadius(level), corners)` (`AbstractBuilding.java:307-313`), read
    /// with [`claim_box`].
    ClaimChunks {
        at: [i32; 3],
        radius: i32,
        plot: Footprint,
    },
    /// The hut's request resolvers are added to the colony's request manager
    /// (`RegisteredStructureManager.java:606`). Without it the hut can ask for
    /// nothing, which is why it is a write and not a formality.
    RegisterResolvers { at: [i32; 3] },
    /// The colony's chunk extent grows to include the hut's chunk
    /// (`RegisteredStructureManager.java:613`, `setMaxChunk` at `:729-748`).
    ExtendExtent { chunk: [i32; 2] },
}

/// A placement the rules accepted, and everything it wrote.
///
/// The journal is the deliverable: six of the seven writes are world state the
/// port cannot touch, so the caller applies them and the test asserts them.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Journal {
    at: [i32; 3],
    kind: Kind,
    writes: Vec<Write>,
}

impl Journal {
    pub fn at(&self) -> [i32; 3] {
        self.at
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The writes, in order.
    pub fn writes(&self) -> &[Write] {
        &self.writes
    }

    /// The writes as names, which is what a test compares when it only cares
    /// about the order: `["record", "level", "claim_chunks", …]`.
    pub fn order(&self) -> Vec<&'static str> {
        self.writes
            .iter()
            .map(|write| match write {
                Write::Record { .. } => "record",
                Write::SideList { .. } => "side_list",
                Write::Level { .. } => "level",
                Write::Parent { .. } => "parent",
                Write::ClaimChunks { .. } => "claim_chunks",
                Write::RegisterResolvers { .. } => "register_resolvers",
                Write::ExtendExtent { .. } => "extend_extent",
            })
            .collect()
    }
}

/// The pack's colony-level gate: the level a colony must have reached before a
/// kind may be placed. Empty by default, which is Java's behaviour — no gate.
///
/// **A rule of this port**, added because the only colony-wide gate Java has is
/// research, and it is asked on an upgrade rather than on a placement
/// (`AbstractBuilding.java:786-799`). A pack that ships a hut which should only
/// appear in a grown colony has nowhere else to say so; a pack that says nothing
/// gets [`Refusal::ColonyLevelTooLow`] never.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Gates {
    by_kind: BTreeMap<Kind, i32>,
}

impl Gates {
    pub fn new() -> Gates {
        Gates::default()
    }

    /// The same table with one more gate, for a builder chain.
    pub fn with(mut self, kind: Kind, level: i32) -> Gates {
        self.set(kind, level);
        self
    }

    /// Sets a gate. A level of `0` or less is no gate, and removes the entry, so
    /// "the data said zero" and "the data said nothing" answer the same.
    pub fn set(&mut self, kind: Kind, level: i32) {
        if level <= 0 {
            self.by_kind.remove(&kind);
        } else {
            self.by_kind.insert(kind, level);
        }
    }

    /// The level a colony must have reached for this kind; `0` when the data says
    /// nothing, which is every kind until a pack speaks.
    pub fn of(&self, kind: Kind) -> i32 {
        self.by_kind.get(&kind).copied().unwrap_or(0)
    }
}

/// Everything a placement needs: what, where, on what ground, and by whose rules.
pub struct Plan<'a> {
    pub kind: Kind,
    pub site: Site,
    /// The world, asked only "is this cell free" — `founding::Ground`.
    pub ground: &'a dyn Ground,
    /// The pack's requirements table (`src/building/requirements.rs`).
    pub requirements: &'a building::Rules,
    /// The founding configuration, read for the colony's radius.
    pub rules: &'a founding::Rules,
    /// The port's own colony-level gate. [`Gates::new`] is Java's behaviour.
    pub gates: &'a Gates,
}

/// Put a hut down: the three checks, then the seven writes.
///
/// The checks, with where each comes from:
///
/// 1. **one tavern per colony** — `BlockHutTavern.canPlaceAt` walks every
///    building, refuses the second one carrying a `tavern_visitor` module, and
///    names the first (`:50-64`). It runs *before* the paste in Java, because it
///    is part of `canPaste`; so it runs first here.
/// 2. **the colony's rules and the founding checks** — the town hall, the hut
///    inside its own box, the box big enough, the parent, the position free, the
///    plot clear. All of it is `founding::plan`, quoted, not copied.
/// 3. **the pack's colony-level gate** — [`Gates`]. Last because it is the port's
///    own rule, and because Java's nearest equivalent (research) is asked at the
///    end of the chain, when an upgrade is requested.
///
/// On success the record is appended to the colony and the journal is returned.
/// On refusal **nothing** is written: the checks all run before the first `push`,
/// and the journal is not built until the record is.
pub fn pose(colony: &mut Colony, plan: &Plan<'_>) -> Result<Journal, Refusal> {
    // 1. One tavern (BlockHutTavern.java:50-64). The module is what Java looks
    //    for, not the kind: a building that lost its tavern module is not a
    //    tavern, and that is the check as written.
    if plan.kind == Kind::Tavern {
        if let Some(at) = colony
            .buildings
            .iter()
            .filter(|record| {
                Kind::from_path(&record.kind)
                    .map(|kind| modules::has(kind, "tavern_visitor"))
                    .unwrap_or(false)
            })
            .map(|record| record.at)
            .next()
        {
            return Err(Refusal::TavernTaken { at });
        }
    }

    // 2. The colony's rules (one town hall, the box, the parent) and the founding
    //    checks around them (inside the colony, the position free, the plot
    //    clear). `founding::plan` is the port's own implementation of both and is
    //    not repeated here.
    let accepted: founding::Placed = founding::plan(
        colony,
        plan.kind,
        &plan.site,
        plan.ground,
        plan.requirements,
        plan.rules,
    )
    .map_err(Refusal::Placement)?;

    // 3. The pack's gate. The colony's level is the town hall's level
    //    (`src/colony/colonie.rs`: a colony has no level of its own).
    let required = plan.gates.of(plan.kind);
    if colony.level() < required {
        return Err(Refusal::ColonyLevelTooLow {
            level: colony.level(),
            required,
        });
    }

    // The checks are over. Everything below writes.
    let at = accepted.building.at();
    let level = accepted.building.level();
    let mut writes = Vec::with_capacity(7);

    // 1. The record (RegisteredStructureManager.java:562-570).
    writes.push(Write::Record {
        at,
        kind: plan.kind,
    });
    // 2. The side list, when the kind has one (:573-590).
    if let Some(list) = SideList::of(plan.kind) {
        writes.push(Write::SideList { at, list });
    }
    // 3. The level from the schematic (:571). A hut placed as a block has none.
    writes.push(Write::Level { at, level });
    // 4. The parent (AbstractSchematicProvider.java:309-321).
    if let Some(parent) = accepted.building.parent() {
        writes.push(Write::Parent { at, parent });
    }
    // 5. The claim: `if (getBuildingLevel() == 0 && !hasParent())`
    //    (AbstractBuilding.java:307-313). A nested hut claims nothing of its own,
    //    and neither does a hut whose level came from a blueprint — the upgrade
    //    hook claims for that one (:940-947).
    if level == 0 && accepted.building.parent().is_none() {
        let radius = claim_radius(plan.kind, level);
        writes.push(Write::ClaimChunks {
            at,
            radius,
            plot: claim_box(at, radius),
        });
    }
    // 6. The resolvers (:606).
    writes.push(Write::RegisterResolvers { at });
    // 7. The colony's extent (:611, :729-748).
    writes.push(Write::ExtendExtent { chunk: chunk_of(at) });

    // The one write this crate can make: the colony's own list.
    colony.buildings.push(Record::new(&plan.kind.path(), at, level));

    Ok(Journal {
        at,
        kind: plan.kind,
        writes,
    })
}

/// The building the journal describes, as the `building` module holds it — what a
/// caller needs straight after a placement, without reading its own colony back.
pub fn placed(journal: &Journal) -> Building {
    let mut building = Building::new(journal.kind, journal.at);
    for write in journal.writes() {
        if let Write::Level { level, .. } = write {
            if *level > 0 {
                // The level came from the schematic, so the hut is built: Java's
                // `onUpgradeComplete` sets `isBuilt` on the same path
                // (AbstractSchematicProvider.java:558-562).
                let _ = building.complete(*level);
            }
        }
        if let Write::Parent { parent, .. } = write {
            let _ = building.set_parent(Some(*parent));
        }
    }
    building
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Stage;
    use crate::founding::Air;

    const HALL: [i32; 3] = [100, 70, 100];
    const OVERWORLD: &str = "minecraft:overworld";

    /// A colony with a town hall and nothing else: what every placement is tried
    /// against.
    fn colony_fixture() -> Colony {
        let mut colony = Colony::new(1, "Bridgewater", OVERWORLD, HALL, 7);
        colony.set_level(1);
        colony
            .buildings
            .push(Record::new("minecolonies:townhall", HALL, 5));
        colony
    }

    fn site(offset: i32, size: [u32; 3]) -> Site {
        Site::alone([HALL[0] + offset, HALL[1], HALL[2] + offset], size)
    }

    fn plan<'a>(
        kind: Kind,
        site: Site,
        requirements: &'a building::Rules,
        gates: &'a Gates,
    ) -> Plan<'a> {
        Plan {
            kind,
            site,
            ground: &Air,
            requirements,
            rules: &founding::Rules::DEFAULT,
            gates,
        }
    }

    #[test]
    fn a_hut_on_clear_ground_writes_seven_times_in_javas_order() {
        let mut colony = colony_fixture();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        let journal = pose(
            &mut colony,
            &plan(Kind::Builder, site(10, [5, 5, 5]), &requirements, &gates),
        )
        .expect("free ground, inside the colony, town hall standing");

        assert_eq!(journal.kind(), Kind::Builder);
        assert_eq!(journal.at(), [110, 70, 110]);
        assert_eq!(
            journal.order(),
            vec![
                "record",
                "level",
                "claim_chunks",
                "register_resolvers",
                "extend_extent"
            ],
            "a builder has no side list and no parent"
        );
        assert!(matches!(
            journal.writes()[0],
            Write::Record {
                at: [110, 70, 110],
                kind: Kind::Builder
            }
        ));
        assert!(
            matches!(journal.writes()[1], Write::Level { level: 0, .. }),
            "a hut placed as a block is level 0 (AbstractSchematicProvider.java:64)"
        );
        assert!(matches!(
            journal.writes()[2],
            Write::ClaimChunks { radius: 0, .. }
        ));
        assert!(matches!(
            journal.writes()[3],
            Write::RegisterResolvers { .. }
        ));
        assert!(matches!(
            journal.writes()[4],
            Write::ExtendExtent { chunk: [6, 6] }
        ));

        // And the one write this crate makes: the record is in the colony.
        let record = colony.building_at([110, 70, 110]).expect("the hut is recorded");
        assert_eq!(record.kind, "minecolonies:builder");
        assert_eq!(record.level, 0);
    }

    #[test]
    fn a_refusal_writes_nothing_at_all() {
        // No town hall: the colony's own first rule (AbstractBlockHut.java:309-327).
        let mut colony = Colony::new(1, "Bridgewater", OVERWORLD, HALL, 7);
        let before = colony.clone();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        assert_eq!(
            pose(
                &mut colony,
                &plan(Kind::Builder, site(10, [5, 5, 5]), &requirements, &gates)
            ),
            Err(Refusal::Placement(founding::Refusal::Building(
                building::Refusal::TownHallMissing
            )))
        );
        assert_eq!(colony, before, "the record is untouched, and so is the rest");

        // And the same for a refusal from the plot: a tree in the way.
        let mut colony = colony_fixture();
        let before = colony.clone();
        let tree = [111, 72, 111];
        let blocked = Plan {
            kind: Kind::Builder,
            site: site(10, [5, 5, 5]),
            ground: &OneBlock(tree),
            requirements: &requirements,
            rules: &founding::Rules::DEFAULT,
            gates: &gates,
        };
        assert_eq!(
            pose(&mut colony, &blocked),
            Err(Refusal::Placement(founding::Refusal::PlotBlocked { at: tree }))
        );
        assert_eq!(colony, before, "a blocked plot costs no write");
    }

    #[test]
    fn a_second_tavern_is_refused_at_the_first_one() {
        let mut colony = colony_fixture();
        colony
            .buildings
            .push(Record::new("minecolonies:tavern", [120, 70, 120], 2));
        let before = colony.clone();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        assert_eq!(
            pose(
                &mut colony,
                &plan(Kind::Tavern, site(30, [9, 9, 9]), &requirements, &gates)
            ),
            Err(Refusal::TavernTaken { at: [120, 70, 120] }),
            "BlockHutTavern.java:52-58 names the tavern that is already there"
        );
        assert_eq!(colony, before);

        // A building that looks like a tavern but carries no visitor module is
        // not one, which is the check Java actually writes.
        let mut colony = colony_fixture();
        colony
            .buildings
            .push(Record::new("minecolonies:builder", [120, 70, 120], 2));
        assert!(
            pose(
                &mut colony,
                &plan(Kind::Tavern, site(30, [9, 9, 9]), &requirements, &gates)
            )
            .is_ok(),
            "no tavern visitor module anywhere"
        );
    }

    #[test]
    fn the_colony_level_gate_is_the_packs_to_set_and_off_by_default() {
        let gates = Gates::new();
        assert_eq!(gates.of(Kind::University), 0, "no data, no gate: Java's behaviour");
        let gates = Gates::new().with(Kind::University, 3);
        assert_eq!(gates.of(Kind::University), 3);
        assert_eq!(gates.of(Kind::School), 0, "one kind's gate is not every kind's");

        let mut colony = colony_fixture(); // level 1
        let before = colony.clone();
        let requirements = building::Rules::new();
        assert_eq!(
            pose(
                &mut colony,
                &plan(Kind::University, site(30, [9, 9, 9]), &requirements, &gates)
            ),
            Err(Refusal::ColonyLevelTooLow {
                level: 1,
                required: 3
            })
        );
        assert_eq!(colony, before);

        colony.set_level(3);
        assert!(
            pose(
                &mut colony,
                &plan(Kind::University, site(30, [9, 9, 9]), &requirements, &gates)
            )
            .is_ok(),
            "the gate is a floor, not a ban"
        );
        // A gate set to zero is the same as no gate at all.
        let mut gates = Gates::new().with(Kind::University, 4);
        gates.set(Kind::University, 0);
        assert_eq!(gates.of(Kind::University), 0);
        assert_eq!(gates, Gates::new());
    }

    #[test]
    fn the_three_side_lists_take_their_own_kind_and_no_other() {
        assert_eq!(SideList::of(Kind::TownHall), Some(SideList::TownHall));
        assert_eq!(SideList::of(Kind::Warehouse), Some(SideList::Warehouse));
        assert_eq!(SideList::of(Kind::MysticalSite), Some(SideList::MysticalSite));
        assert_eq!(SideList::of(Kind::Builder), None);
        assert_eq!(SideList::of(Kind::Barracks), None, "a barracks is in no list");

        // A warehouse is placed and says so in its journal.
        let mut colony = colony_fixture();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        let journal = pose(
            &mut colony,
            &plan(Kind::Warehouse, site(12, [9, 9, 9]), &requirements, &gates),
        )
        .expect("a warehouse, well inside the colony");
        assert_eq!(
            journal.writes()[1],
            Write::SideList {
                at: [112, 70, 112],
                list: SideList::Warehouse
            }
        );
        assert_eq!(
            journal.order(),
            vec![
                "record",
                "side_list",
                "level",
                "claim_chunks",
                "register_resolvers",
                "extend_extent"
            ]
        );
    }

    #[test]
    fn a_nested_hut_records_its_parent_and_claims_no_chunk_of_its_own() {
        // A barracks at level 3, and a tower that nests in it.
        let mut colony = colony_fixture();
        colony
            .buildings
            .push(Record::new("minecolonies:barracks", [130, 70, 100], 3));
        let mut requirements = building::Rules::new();
        requirements.set(
            Kind::BarracksTower,
            building::Requirements {
                parent: building::ParentRule::Inside(Kind::Barracks),
                parent_level: 1,
                footprint: [5, 5, 5],
            },
        );
        let gates = Gates::new();
        let plan = Plan {
            kind: Kind::BarracksTower,
            site: Site::inside([132, 70, 100], [5, 5, 5], [130, 70, 100]),
            ground: &Air,
            requirements: &requirements,
            rules: &founding::Rules::DEFAULT,
            gates: &gates,
        };
        let journal = pose(&mut colony, &plan).expect("a tower inside a level-3 barracks");
        assert_eq!(
            journal.order(),
            vec![
                "record",
                "level",
                "parent",
                "register_resolvers",
                "extend_extent"
            ],
            "no claim: onPlacement claims only for a level-0 hut without a parent"
        );
        assert_eq!(
            journal.writes()[2],
            Write::Parent {
                at: [132, 70, 100],
                parent: [130, 70, 100]
            }
        );
        assert!(
            !journal
                .writes()
                .iter()
                .any(|write| matches!(write, Write::ClaimChunks { .. }))
        );
    }

    #[test]
    fn the_journal_describes_the_same_hut_the_building_module_would_hold() {
        let mut colony = colony_fixture();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        let journal = pose(
            &mut colony,
            &plan(Kind::Farmer, site(8, [5, 5, 5]), &requirements, &gates),
        )
        .expect("a farmer");
        let hut = placed(&journal);
        assert_eq!(hut.kind(), Kind::Farmer);
        assert_eq!(hut.at(), journal.at());
        assert_eq!(hut.level(), 0);
        assert_eq!(hut.stage(), Stage::Unbuilt, "placed is not built");
        assert_eq!(hut.parent(), None);
    }

    #[test]
    fn the_claim_a_hut_makes_is_the_chunk_it_stands_in_and_nothing_more() {
        let mut colony = colony_fixture();
        let requirements = building::Rules::new();
        let gates = Gates::new();
        let journal = pose(
            &mut colony,
            &plan(Kind::Builder, site(10, [5, 5, 5]), &requirements, &gates),
        )
        .expect("a builder");
        let Write::ClaimChunks { radius, plot, .. } = journal.writes()[2] else {
            panic!("the claim is the third write: {:?}", journal.order());
        };
        assert_eq!(radius, 0, "a level-0 hut claims a radius of zero chunks");
        assert_eq!(
            plot,
            claim_box([110, 70, 110], 0),
            "which is still a chunk-aligned box, 16 blocks on each axis"
        );
        assert_eq!(plot.size(), [16, 16, 16]);
        assert!(matches!(
            journal.writes()[4],
            Write::ExtendExtent {
                chunk: [6, 6]
            }
        ));
    }

    /// A ground with exactly one occupied cell.
    struct OneBlock([i32; 3]);

    impl Ground for OneBlock {
        fn is_free(&self, at: [i32; 3]) -> bool {
            at != self.0
        }
    }
}
