//! What a kind demands before it may be placed, and before it may grow.
//!
//! Java reference: four places, not one, because a hut is validated by whoever
//! puts it down and upgraded by the building itself.
//!
//! | Rule | Where Java says it |
//! | --- | --- |
//! | no hut without a town hall | `api/blocks/AbstractBlockHut.java:305-338` |
//! | one town hall per colony | `core/blocks/huts/BlockHutTownHall.java:219-234` |
//! | a parent must exist, and have no parent of its own | `core/colony/buildings/AbstractSchematicProvider.java:303-321` |
//! | the hut block lies inside the schematic's box, one block of slack | `:505-521` |
//! | an upgrade waits for the parent's level, and stops at 5 | `core/colony/buildings/AbstractBuilding.java:803-814` |
//!
//! **What is data and what is code.** The *rules* are code, quoted above. The
//! *values* — which kind nests in which, at which parent level, in how big a
//! box — are a pack's business, so they live in [`Rules`], filled from data and
//! serialised like everything else here. A test fills it by hand.
//!
//! **Every check returns a named refusal**, never a bool: the caller is a
//! right-click, and a hut that refuses to appear without saying why is exactly
//! what MineColonies' own messages avoid (`WARNING_TOWN_HALL_NOT_PRESENT`,
//! `WARNING_NO_UPGRADE`).

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::instance::Building;
use super::kind::Kind;
use super::nbt;
use super::{LEVEL_MIN, MAX_BUILDING_LEVEL, Refusal};

/// The table's keys in a compound: one record per kind.
pub const KEY_REQUIREMENTS: &str = "requirements";
pub const KEY_TYPE: &str = "type";
pub const KEY_PARENT: &str = "parent";
pub const KEY_PARENT_LEVEL: &str = "parentLevel";
pub const KEY_FOOTPRINT: &str = "footprint";

/// The box a schematic covers, as its two opposite corners.
///
/// Java keeps the same pair as `lowerCorner`/`higherCorner`, sorted on write by
/// `setCorners` (`AbstractSchematicProvider.java:266-271`), and falls back to the
/// hut's own position for a box it could not read (`:274-286`). [`Self::between`]
/// sorts the same way, so two corners name one box whatever order they arrive in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footprint {
    low: [i32; 3],
    high: [i32; 3],
}

impl Footprint {
    /// The box two opposite corners delimit, in blocks, both corners included.
    pub fn between(a: [i32; 3], b: [i32; 3]) -> Footprint {
        Footprint {
            low: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            high: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }

    /// A box of `size` blocks on each axis, anchored at `at`. A dimension of
    /// zero is one block: a box that covers nothing is not a box.
    pub fn at_size(at: [i32; 3], size: [u32; 3]) -> Footprint {
        let mut high = at;
        for (axis, corner) in high.iter_mut().enumerate() {
            *corner += size[axis].max(1) as i32 - 1;
        }
        Footprint { low: at, high }
    }

    /// The same box, anchored at the origin.
    pub fn of_size(size: [u32; 3]) -> Footprint {
        Footprint::at_size([0, 0, 0], size)
    }

    pub fn low(&self) -> [i32; 3] {
        self.low
    }

    pub fn high(&self) -> [i32; 3] {
        self.high
    }

    /// The number of blocks on each axis, both corners included: a box from
    /// `[0,0,0]` to `[0,0,0]` is one block, not none.
    pub fn size(&self) -> [u32; 3] {
        [
            (self.high[0] - self.low[0] + 1) as u32,
            (self.high[1] - self.low[1] + 1) as u32,
            (self.high[2] - self.low[2] + 1) as u32,
        ]
    }

    /// Whether a position is in the box, **with one block of slack** on every
    /// side: `isInBuilding` tests `corner - 1 ..= corner + 1`
    /// (`AbstractSchematicProvider.java:518-520`), because a hut block may sit
    /// against its own wall.
    pub fn contains(&self, at: [i32; 3]) -> bool {
        (0..3).all(|axis| at[axis] >= self.low[axis] - 1 && at[axis] <= self.high[axis] + 1)
    }

    /// Whether the box is at least `needs` blocks on every axis.
    pub fn covers(&self, needs: [u32; 3]) -> bool {
        let size = self.size();
        (0..3).all(|axis| size[axis] >= needs[axis])
    }
}

/// Where a kind has to stand, or has to stand inside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the port's own set, closed on purpose
pub enum ParentRule {
    /// Stands on its own; Java's parent is `BlockPos.ZERO`
    /// (`AbstractSchematicProvider.java:297-306`). A free building may still be
    /// given a parent — the rule says it does not *need* one.
    #[default]
    Free,
    /// Must be nested inside a building of this kind: a barracks tower inside
    /// barracks (`core/colony/buildings/workerbuildings/BuildingBarracksTower.java`),
    /// whose parent is the barracks it was pasted into
    /// (`core/colony/buildings/AbstractBuildingContainer.java:160-170`).
    Inside(Kind),
}

/// What one kind demands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub parent: ParentRule,
    /// The level the parent must have reached. Only read when there is a parent
    /// to test; Java has no such field, it compares the two levels
    /// (`AbstractBuilding.java:807`), and this is the stricter rule a pack may
    /// add on top.
    pub parent_level: i32,
    /// The smallest box the schematic may cover, in blocks per axis.
    pub footprint: [u32; 3],
}

impl Default for Requirements {
    /// No parent, no parent level, and a box of one block: the weakest rule, so
    /// a kind the data never mentions is not silently impossible to place.
    fn default() -> Self {
        Requirements {
            parent: ParentRule::Free,
            parent_level: 0,
            footprint: [1, 1, 1],
        }
    }
}

/// The requirements of every kind, filled from a pack's data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    by_kind: BTreeMap<Kind, Requirements>,
}

impl Rules {
    pub fn new() -> Rules {
        Rules::default()
    }

    pub fn set(&mut self, kind: Kind, requirements: Requirements) {
        self.by_kind.insert(kind, requirements);
    }

    /// The requirements of a kind; [`Requirements::default`] for one the data
    /// does not mention.
    pub fn of(&self, kind: Kind) -> Requirements {
        self.by_kind.get(&kind).copied().unwrap_or_default()
    }

    pub fn to_compound(&self) -> Compound {
        let mut records = Vec::new();
        for (kind, needs) in &self.by_kind {
            let path = kind.path();
            let mut record = Compound::from([
                (KEY_TYPE.into(), Tag::String(path.as_str().into())),
                (KEY_PARENT_LEVEL.into(), Tag::Int(needs.parent_level)),
                (
                    KEY_FOOTPRINT.into(),
                    nbt::int_list([
                        needs.footprint[0] as i32,
                        needs.footprint[1] as i32,
                        needs.footprint[2] as i32,
                    ]),
                ),
            ]);
            if let ParentRule::Inside(parent) = needs.parent {
                record.insert(KEY_PARENT.into(), Tag::String(parent.path().as_str().into()));
            }
            records.push(record);
        }
        Compound::from([(KEY_REQUIREMENTS.into(), nbt::compound_list(records))])
    }

    /// Reads the table. A record with an unknown kind or parent, or a footprint
    /// that is not three ints, falls back to the defaults rather than inventing
    /// a rule: a pack's mistake must not make a hut unplaceable for a reason
    /// nobody can read.
    pub fn from_compound(data: &Compound) -> Rules {
        let mut rules = Rules::new();
        for record in nbt::compounds(data, KEY_REQUIREMENTS) {
            let Some(kind) = nbt::text(record, KEY_TYPE)
                .as_deref()
                .and_then(Kind::from_path)
            else {
                continue;
            };
            let parent = nbt::text(record, KEY_PARENT)
                .as_deref()
                .and_then(Kind::from_path)
                .map_or(ParentRule::Free, ParentRule::Inside);
            let parent_level = nbt::int(record, KEY_PARENT_LEVEL).unwrap_or(0);
            let footprint = record
                .get(&KEY_FOOTPRINT.into())
                .and_then(nbt::ints_of)
                .map_or([1, 1, 1], |size| {
                    [
                        size[0].max(1) as u32,
                        size[1].max(1) as u32,
                        size[2].max(1) as u32,
                    ]
                });
            rules.set(
                kind,
                Requirements {
                    parent,
                    parent_level,
                    footprint,
                },
            );
        }
        rules
    }
}

/// Where a hut is about to be put: its block position, the box its schematic
/// covers, and the building it claims as its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub at: [i32; 3],
    pub footprint: Footprint,
    pub parent: Option<[i32; 3]>,
}

impl Site {
    /// A hut on its own, in a box of the size given, anchored on its position.
    pub fn alone(at: [i32; 3], size: [u32; 3]) -> Site {
        Site {
            at,
            footprint: Footprint::at_size(at, size),
            parent: None,
        }
    }

    /// The same, nested in the building at `parent`.
    pub fn inside(at: [i32; 3], size: [u32; 3], parent: [i32; 3]) -> Site {
        Site {
            parent: Some(parent),
            ..Site::alone(at, size)
        }
    }
}

/// The buildings that already stand in one colony — what every rule above
/// needs to know, and nothing else. Java's `RegisteredStructureManager`, minus
/// the world (`core/colony/managers/RegisteredStructureManager.java:563-604`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    buildings: Vec<Building>,
}

impl Standing {
    pub fn new() -> Standing {
        Standing::default()
    }

    /// Adds a building, **replacing** any building already at that position:
    /// the position is the identity (`RegisteredStructureManager.java:566` skips
    /// a position it already holds). Returns what it replaced, if anything.
    pub fn insert(&mut self, building: Building) -> Option<Building> {
        match self.buildings.iter_mut().find(|held| held.at() == building.at()) {
            Some(held) => Some(std::mem::replace(held, building)),
            None => {
                self.buildings.push(building);
                None
            }
        }
    }

    pub fn at(&self, at: [i32; 3]) -> Option<&Building> {
        self.buildings.iter().find(|building| building.at() == at)
    }

    /// The colony's town hall, which is the building at the colony's centre
    /// (`RegisteredStructureManager.java:518-527`).
    pub fn town_hall(&self) -> Option<&Building> {
        self.buildings
            .iter()
            .find(|building| building.kind().is_town_hall())
    }

    pub fn has_town_hall(&self) -> bool {
        self.town_hall().is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Building> {
        self.buildings.iter()
    }

    /// May `kind` be placed at `site`? The four rules in the order Java tests
    /// them, each with its own refusal.
    pub fn check_placement(
        &self,
        kind: Kind,
        site: &Site,
        rules: &Rules,
    ) -> Result<(), Refusal> {
        // 1. The town hall comes first, and comes alone
        //    (AbstractBlockHut.java:309-327, BlockHutTownHall.java:219-234).
        if kind.is_town_hall() {
            if let Some(town_hall) = self.town_hall() {
                return Err(Refusal::TownHallAlreadyThere {
                    at: town_hall.at(),
                });
            }
        } else if !self.has_town_hall() {
            return Err(Refusal::TownHallMissing);
        }

        // 2. The hut block is in the box its own schematic covers.
        if !site.footprint.contains(site.at) {
            return Err(Refusal::HutOutsideFootprint { at: site.at });
        }

        // 3. The box is big enough for the kind.
        let needs = rules.of(kind);
        if !site.footprint.covers(needs.footprint) {
            return Err(Refusal::FootprintTooSmall {
                required: needs.footprint,
                measured: site.footprint.size(),
            });
        }

        // 4. The parent, if the kind wants one.
        self.check_parent(&needs, site)
    }

    /// May this building grow one level? Java's condition
    /// (`AbstractBuilding.java:803-814`): a level-0 hut is a build, not an
    /// upgrade; the level stops at [`MAX_BUILDING_LEVEL`]; and a hut may not
    /// pass its parent unless the parent is already at the top.
    pub fn check_upgrade(&self, building: &Building, rules: &Rules) -> Result<(), Refusal> {
        if building.level() < LEVEL_MIN {
            // "A building placed and not built is a BUILD work order"
            // (AbstractBuilding.java:803-805).
            return Err(Refusal::NotBuilt);
        }
        if building.level() >= MAX_BUILDING_LEVEL {
            return Err(Refusal::AlreadyMaxLevel {
                level: building.level(),
            });
        }
        let needs = rules.of(building.kind());
        // Java reads the parent out of the colony and skips the test entirely
        // when it finds none (:801, :807).
        if let Some(parent_at) = building.parent() {
            let Some(parent) = self.at(parent_at) else {
                return Err(Refusal::ParentMissing { at: parent_at });
            };
            if parent.level() < needs.parent_level {
                return Err(Refusal::ParentLevelTooLow {
                    at: parent_at,
                    level: parent.level(),
                    required: needs.parent_level,
                });
            }
            if building.level() >= parent.level() && parent.level() < MAX_BUILDING_LEVEL {
                return Err(Refusal::ParentLevelAhead {
                    level: building.level(),
                    parent: parent.level(),
                });
            }
        }
        Ok(())
    }

    fn check_parent(&self, needs: &Requirements, site: &Site) -> Result<(), Refusal> {
        let ParentRule::Inside(required) = needs.parent else {
            return Ok(());
        };
        let Some(parent_at) = site.parent else {
            return Err(Refusal::ParentRequired { kind: required });
        };
        // A building is never its own parent (AbstractSchematicProvider.java:320).
        if parent_at == site.at {
            return Err(Refusal::ParentSelf { at: site.at });
        }
        let Some(parent) = self.at(parent_at) else {
            return Err(Refusal::ParentMissing { at: parent_at });
        };
        // Nesting stops at one: the parent itself must have no parent
        // (AbstractSchematicProvider.java:319-320, `isParentValid`).
        if parent.parent().is_some() {
            return Err(Refusal::ParentNested { at: parent_at });
        }
        if parent.kind() != required {
            return Err(Refusal::ParentWrongKind {
                at: parent_at,
                found: parent.kind(),
                required,
            });
        }
        if parent.level() < needs.parent_level {
            return Err(Refusal::ParentLevelTooLow {
                at: parent_at,
                level: parent.level(),
                required: needs.parent_level,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALL: [i32; 3] = [0, 70, 0];
    const BARRACKS: [i32; 3] = [40, 70, 0];
    const TOWER: [i32; 3] = [48, 70, 0];

    fn standing() -> Standing {
        let mut standing = Standing::new();
        let mut town_hall = Building::new(Kind::TownHall, HALL);
        town_hall.complete(5).expect("a town hall is built");
        standing.insert(town_hall);
        let mut barracks = Building::new(Kind::Barracks, BARRACKS);
        barracks.complete(3).expect("level three barracks");
        standing.insert(barracks);
        standing
    }

    fn nested() -> Rules {
        let mut rules = Rules::new();
        rules.set(
            Kind::BarracksTower,
            Requirements {
                parent: ParentRule::Inside(Kind::Barracks),
                parent_level: 2,
                footprint: [5, 5, 5],
            },
        );
        rules
    }

    fn site(at: [i32; 3], size: [u32; 3], parent: Option<[i32; 3]>) -> Site {
        match parent {
            None => Site::alone(at, size),
            Some(parent) => Site::inside(at, size, parent),
        }
    }

    #[test]
    fn a_footprint_sorts_its_two_corners_and_counts_both() {
        let box_ = Footprint::between([10, 64, 10], [8, 70, 12]);
        assert_eq!(box_.low(), [8, 64, 10]);
        assert_eq!(box_.high(), [10, 70, 12]);
        assert_eq!(box_.size(), [3, 7, 3]);
        assert_eq!(Footprint::between([1, 1, 1], [1, 1, 1]).size(), [1, 1, 1]);
        assert_eq!(Footprint::of_size([0, 0, 0]).size(), [1, 1, 1], "a box covers a block at least");
        assert_eq!(Footprint::of_size([3, 2, 1]).size(), [3, 2, 1]);
    }

    #[test]
    fn a_hut_sits_in_its_box_with_one_block_of_slack() {
        let box_ = Footprint::between([0, 64, 0], [4, 68, 4]);
        assert!(box_.contains([0, 64, 0]));
        assert!(box_.contains([4, 68, 4]));
        assert!(box_.contains([-1, 63, -1]), "one block of slack, as isInBuilding allows");
        assert!(box_.contains([5, 69, 5]));
        assert!(!box_.contains([6, 64, 0]));
        assert!(box_.covers([5, 5, 5]));
        assert!(!box_.covers([6, 5, 5]));
    }

    #[test]
    fn the_town_hall_comes_first_and_comes_alone() {
        let rules = Rules::new();
        let empty = Standing::new();
        assert_eq!(
            empty.check_placement(Kind::Builder, &site([1, 70, 1], [3, 3, 3], None), &rules),
            Err(Refusal::TownHallMissing),
            "no hut before the town hall"
        );
        assert_eq!(
            empty.check_placement(Kind::TownHall, &site(HALL, [9, 9, 9], None), &rules),
            Ok(()),
            "the town hall itself is the exception, and may be the first"
        );

        let standing = standing();
        assert_eq!(
            standing.check_placement(
                Kind::TownHall,
                &site([60, 70, 60], [9, 9, 9], None),
                &rules
            ),
            Err(Refusal::TownHallAlreadyThere { at: HALL }),
            "and the only one"
        );
        assert_eq!(
            standing.check_placement(Kind::Builder, &site([1, 70, 1], [3, 3, 3], None), &rules),
            Ok(())
        );
    }

    #[test]
    fn a_nested_kind_needs_its_parent_and_nesting_stops_at_one() {
        let standing = standing();
        let rules = nested();

        assert_eq!(
            standing.check_placement(Kind::BarracksTower, &site(TOWER, [5, 5, 5], None), &rules),
            Err(Refusal::ParentRequired {
                kind: Kind::Barracks
            })
        );
        assert_eq!(
            standing.check_placement(
                Kind::BarracksTower,
                &site(TOWER, [5, 5, 5], Some(TOWER)),
                &rules
            ),
            Err(Refusal::ParentSelf { at: TOWER })
        );
        assert_eq!(
            standing.check_placement(Kind::BarracksTower, &site(TOWER, [5, 5, 5], Some([41, 70, 0])), &rules),
            Err(Refusal::ParentMissing { at: [41, 70, 0] })
        );
        assert_eq!(
            standing.check_placement(Kind::BarracksTower, &site(TOWER, [5, 5, 5], Some(HALL)), &rules),
            Err(Refusal::ParentWrongKind {
                at: HALL,
                found: Kind::TownHall,
                required: Kind::Barracks
            })
        );
        assert_eq!(
            standing.check_placement(Kind::BarracksTower, &site(TOWER, [5, 5, 5], Some(BARRACKS)), &rules),
            Ok(()),
            "a barracks at level 3, a tower at level 2"
        );
    }

    #[test]
    fn a_parent_that_has_its_own_parent_cannot_take_one() {
        // A building nested in the barracks, and a tower that would nest in it.
        let mut standing = standing();
        let mut inner = Building::new(Kind::Stash, [44, 70, 0]);
        inner.complete(1).expect("a stash");
        inner.set_parent(Some(BARRACKS)).expect("nested in the barracks");
        standing.insert(inner);
        let mut rules = nested();
        rules.set(
            Kind::SimpleQuarry,
            Requirements {
                parent: ParentRule::Inside(Kind::Stash),
                parent_level: 1,
                footprint: [1, 1, 1],
            },
        );
        assert_eq!(
            standing.check_placement(Kind::SimpleQuarry, &site([46, 70, 0], [1, 1, 1], Some([44, 70, 0])), &rules),
            Err(Refusal::ParentNested { at: [44, 70, 0] })
        );
    }

    #[test]
    fn a_parent_below_the_level_the_data_asks_for_is_refused() {
        let mut standing = Standing::new();
        let mut town_hall = Building::new(Kind::TownHall, HALL);
        town_hall.complete(5).expect("a town hall");
        standing.insert(town_hall);
        let mut barracks = Building::new(Kind::Barracks, BARRACKS);
        barracks.complete(1).expect("a level one barracks");
        standing.insert(barracks);
        assert_eq!(
            standing.check_placement(
                Kind::BarracksTower,
                &site(TOWER, [5, 5, 5], Some(BARRACKS)),
                &nested()
            ),
            Err(Refusal::ParentLevelTooLow {
                at: BARRACKS,
                level: 1,
                required: 2
            })
        );
    }

    #[test]
    fn a_box_smaller_than_the_kind_needs_and_a_hut_out_of_its_own_box_are_both_named() {
        let standing = standing();
        assert_eq!(
            standing.check_placement(
                Kind::BarracksTower,
                &site(TOWER, [4, 5, 5], Some(BARRACKS)),
                &nested()
            ),
            Err(Refusal::FootprintTooSmall {
                required: [5, 5, 5],
                measured: [4, 5, 5]
            })
        );
        // A box that does not hold the hut block at all: the schematic is wrong.
        let mut plan = site(TOWER, [5, 5, 5], Some(BARRACKS));
        plan.footprint = Footprint::between([60, 70, 0], [64, 74, 4]);
        assert_eq!(
            standing.check_placement(Kind::BarracksTower, &plan, &nested()),
            Err(Refusal::HutOutsideFootprint { at: TOWER })
        );
    }

    #[test]
    fn an_upgrade_waits_for_its_parent_and_stops_at_five() {
        let mut standing = standing();
        let mut tower = Building::new(Kind::BarracksTower, TOWER);
        tower.complete(1).expect("a tower");
        tower.set_parent(Some(BARRACKS)).expect("inside the barracks");
        let rules = nested();
        assert_eq!(standing.check_upgrade(&tower, &rules), Ok(()), "level 1 of 5, parent at 3");

        // The tower catches its parent: it waits for the barracks to grow.
        tower.complete(3).expect("level three");
        standing.insert(tower.clone());
        assert_eq!(
            standing.check_upgrade(&tower, &rules),
            Err(Refusal::ParentLevelAhead { level: 3, parent: 3 })
        );
        // A parent at the top of its own range no longer holds the child back
        // (AbstractBuilding.java:807, `parent >= parent.getMaxBuildingLevel()`).
        let mut full = Building::new(Kind::Barracks, BARRACKS);
        full.complete(5).expect("level five");
        standing.insert(full);
        assert_eq!(standing.check_upgrade(&tower, &rules), Ok(()));

        // And five is the end of the road for the tower itself.
        let mut maxed = tower.clone();
        maxed.complete(5).expect("level five");
        assert_eq!(
            standing.check_upgrade(&maxed, &rules),
            Err(Refusal::AlreadyMaxLevel { level: 5 })
        );
        // A hut nobody built is a build, not an upgrade.
        let fresh = Building::new(Kind::BarracksTower, TOWER);
        assert_eq!(standing.check_upgrade(&fresh, &rules), Err(Refusal::NotBuilt));
    }

    #[test]
    fn a_position_holds_one_building_and_the_newest_replaces_it() {
        let mut standing = standing();
        let first = Building::new(Kind::Builder, [8, 70, 8]);
        assert_eq!(standing.insert(first), None);
        let mut second = Building::new(Kind::Builder, [8, 70, 8]);
        second.complete(2).expect("level two");
        assert_eq!(
            standing.insert(second).map(|replaced| replaced.level()),
            Some(0),
            "the position is the identity, as in the manager"
        );
        assert_eq!(standing.at([8, 70, 8]).map(Building::level), Some(2));
        assert_eq!(standing.at([9, 70, 8]), None);
        assert_eq!(standing.town_hall().map(Building::at), Some(HALL));
        assert_eq!(standing.iter().count(), 3, "two from the fixture, one builder");
    }

    #[test]
    fn the_requirements_table_round_trips_and_an_absent_kind_is_weakest() {
        let rules = nested();
        let read = Rules::from_compound(&rules.to_compound());
        assert_eq!(read, rules);
        let tower = read.of(Kind::BarracksTower);
        assert_eq!(tower.parent, ParentRule::Inside(Kind::Barracks));
        assert_eq!(tower.parent_level, 2);
        assert_eq!(tower.footprint, [5, 5, 5]);
        assert_eq!(read.of(Kind::Builder), Requirements::default());
        assert_eq!(read.of(Kind::Builder).parent, ParentRule::Free);
    }
}
