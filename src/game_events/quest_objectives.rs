//! `QuestObjectiveEventHandler.java`: the five registries, and the bug in three of them.
//!
//! Java reference: `QuestObjectiveEventHandler.java`, 319 lines, 11 branches, all in
//! [`super::table`]. The class holds **five static maps** and nothing else: three are
//! keyed by the game object a player touches (`Block` → `UUID` → quests, twice, and
//! `EntityType` → `UUID` → quests), two by a colony-owned fact (`BuildingEntry` →
//! `IColony` → quests, and `ResourceLocation` → `IColony` → quests).
//!
//! **This module owns the registries, not the objectives.** What one event does to a
//! counter is `crate::quests::objective::Objective::observe`; what is registered where
//! is here. `src/quests/MANQUANT.md` §7 says the registration was left to "the
//! caller", and the caller is this file: [`Registry`] is the five maps as one type,
//! so a quest manager adds and removes through it and the event branch asks it who
//! cares.
//!
//! **The three event maps share a bug, and the port keeps it.**
//! `:74-86`, `:98-110` and `:132-144` iterate over a **copy** of the list and, on an
//! objective of the wrong class, both remove the quest from the live list and
//! `break`. `break` leaves the loop, so the *remaining* quests registered for the
//! same block are not told about the event at all — an objective that changed class
//! silences everyone after it. The two colony-keyed maps (`:237-270`) have no such
//! removal and no `break`: an objective of the wrong class there is simply skipped.
//! [`Dispatch`] keeps the difference: [`Dispatch::stopped`] is the `break`, and it is
//! reported instead of being swallowed.
//!
//! **Nothing is ever pruned, and nothing is cleared.** `removeQuest…` uses
//! `getOrDefault(block, new HashMap<>()).getOrDefault(player, new ArrayList<>())`,
//! so three throw-away collections are allocated per call and an emptied entry stays
//! in the map for the life of the process; there is no `onWorldLoad` hook in this
//! class at all (the claim in `src/quests/MANQUANT.md` that the maps are cleared on
//! world load is wrong — the Java clears nothing). [`Registry::len`] measures what is
//! held and [`Registry::keys`] shows the half this representation closes: a key cannot
//! outlive its last hold, where Java's map keeps it with an empty list.

use crate::quests::objective::Objective;

/// Which of the five maps a registration belongs to.
///
/// Java decides it by the objective's class, and the class also decides *which id*
/// the map is keyed by (`:62-107`): a `BreakBlockObjectiveTemplate` goes in
/// `breakBlockObjectives` under its block, a `ResearchObjectiveTemplate` in
/// `researchObjectives` under its research. [`Subject`](crate::quests::objective::Subject)
/// already spells the pair out, so this enum is `Subject` without the id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestMap {
    /// `breakBlockObjectives`: `Block` → `UUID` → quests (`:44`).
    BreakBlock,
    /// `placeBlockObjectives`: `Block` → `UUID` → quests (`:54`).
    PlaceBlock,
    /// `entityKillObjectives`: `EntityType` → `UUID` → quests (`:49`).
    KillEntity,
    /// `buildBuildingObjectives`: `BuildingEntry` → `IColony` → quests (`:34`).
    BuildBuilding,
    /// `researchObjectives`: `ResourceLocation` → `IColony` → quests (`:39`).
    Research,
}

impl QuestMap {
    /// Which of the five the Java would choose for an objective, or `None` for the
    /// two that register nothing (`Dialogue`, `Delivery` —
    /// `Objective::listener()` answers `None` for both).
    pub fn of(objective: &Objective) -> Option<QuestMap> {
        match objective {
            Objective::BreakBlock { .. } => Some(QuestMap::BreakBlock),
            Objective::PlaceBlock { .. } => Some(QuestMap::PlaceBlock),
            Objective::KillEntity { .. } => Some(QuestMap::KillEntity),
            Objective::BuildBuilding { .. } => Some(QuestMap::BuildBuilding),
            Objective::Research { .. } => Some(QuestMap::Research),
            Objective::Dialogue { .. } | Objective::Delivery { .. } => None,
        }
    }

    /// Whether this map is keyed by a player's UUID rather than a colony's.
    ///
    /// The three event maps are; that is why a player who logs out leaves his
    /// registrations behind — Java removes them from `playerPositions`
    /// (`EventHandler.java:509`) and from no quest map.
    pub fn per_player(self) -> bool {
        matches!(self, QuestMap::BreakBlock | QuestMap::PlaceBlock | QuestMap::KillEntity)
    }
}

/// One quest's hold on one objective: what the maps store.
///
/// Java stores the `IQuestInstance` itself, from which it reads `getId()` and
/// `getObjectiveIndex()` on every event (`:76`, `:100`, `:134`). The port stores the
/// two numbers, because they are all the dispatch needs and a quest instance is not
/// this module's to own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hold {
    /// The quest template id.
    pub quest: u32,
    /// `instance.getObjectiveIndex()` — which objective of the template is open.
    pub objective: i32,
}

/// Whose event a registration answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    /// A player, for the three event maps.
    Player(u128),
    /// A colony, for the two colony maps.
    Colony(i32),
}

/// What one event produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dispatch {
    /// The quests the event reached, in registration order.
    pub reached: Vec<Hold>,
    /// `true` when an objective of the wrong class made Java `break` (`:83`, `:107`,
    /// `:142`): every quest registered **after** the offending one was skipped, and
    /// the offending one was removed.
    pub stopped: bool,
}

impl Dispatch {
    /// Whether the event reached nothing and stopped on nobody.
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty() && !self.stopped
    }
}

/// The five maps, in one type.
///
/// Keys are kept as text (`"minecraft:dirt"`, `"minecraft:zombie"`,
/// `"minecolonies:blockhutbuilder"`, `"minecolonies:research"`), which is what the
/// game's identifiers are and what a save keeps.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    break_block: Vec<(String, Owner, Hold)>,
    place_block: Vec<(String, Owner, Hold)>,
    kill_entity: Vec<(String, Owner, Hold)>,
    build_building: Vec<(String, Owner, Hold)>,
    research: Vec<(String, Owner, Hold)>,
}

impl Registry {
    /// An empty registry: what a freshly started server has.
    pub fn new() -> Registry {
        Registry::default()
    }

    fn map(&self, map: QuestMap) -> &Vec<(String, Owner, Hold)> {
        match map {
            QuestMap::BreakBlock => &self.break_block,
            QuestMap::PlaceBlock => &self.place_block,
            QuestMap::KillEntity => &self.kill_entity,
            QuestMap::BuildBuilding => &self.build_building,
            QuestMap::Research => &self.research,
        }
    }

    fn map_mut(&mut self, map: QuestMap) -> &mut Vec<(String, Owner, Hold)> {
        match map {
            QuestMap::BreakBlock => &mut self.break_block,
            QuestMap::PlaceBlock => &mut self.place_block,
            QuestMap::KillEntity => &mut self.kill_entity,
            QuestMap::BuildBuilding => &mut self.build_building,
            QuestMap::Research => &mut self.research,
        }
    }

    /// `addQuestMineObjectiveListener` and its four siblings (`:155-162`,
    /// `:183-190`, `:211-218`, `:277-284`, `:301-308`): append.
    ///
    /// Java appends to the end of the list and never de-duplicates, so a quest that
    /// re-tracks the same subject is told twice per event. The port appends too.
    pub fn listen(&mut self, map: QuestMap, key: &str, owner: Owner, hold: Hold) {
        self.map_mut(map)
            .push((key.to_owned(), owner, hold));
    }

    /// `removeQuestMineObjectiveListener` and its four siblings (`:171-174`,
    /// `:199-202`, `:227-230`, `:291-294`, `:315-318`): remove the **first**
    /// occurrence, and leave the key behind.
    ///
    /// Java's `List.remove(Object)` removes the first equal element and answers
    /// whether it did; the map entry survives, empty. Answering `false` here is the
    /// same information without the throw-away allocations.
    pub fn forget(&mut self, map: QuestMap, key: &str, owner: Owner, hold: Hold) -> bool {
        let entries = self.map_mut(map);
        let Some(index) = entries
            .iter()
            .position(|(k, o, h)| k == key && *o == owner && *h == hold)
        else {
            return false;
        };
        entries.remove(index);
        true
    }

    /// How many registrations one map holds, empty keys included — the leak,
    /// measured.
    pub fn len(&self, map: QuestMap) -> usize {
        self.map(map).len()
    }

    /// Whether the registry holds nothing at all.
    pub fn is_empty(&self) -> bool {
        QuestMap::ALL.iter().all(|map| self.len(*map) == 0)
    }

    /// The distinct keys one map currently holds, in first-seen order.
    ///
    /// Java's map keeps a key whose list `removeQuest…` emptied: the entry is never
    /// removed, so `breakBlockObjectives.size()` counts blocks nobody is waiting on
    /// any more. A flat list of triples cannot hold a key without a hold, so the port
    /// closes that part of the leak by construction — and this function is how a test
    /// says so.
    pub fn keys(&self, map: QuestMap) -> Vec<&str> {
        let mut keys: Vec<&str> = Vec::new();
        for (key, _, _) in self.map(map) {
            if !keys.contains(&key.as_str()) {
                keys.push(key.as_str());
            }
        }
        keys
    }

    /// `BlockEvent.BreakEvent` (`:61-88`): every quest registered for `block` under
    /// `player`, in registration order, stopping at the first hold whose objective is
    /// not a break-block one.
    ///
    /// `objective_of` resolves `(quest, objective index)` to the objective, which in
    /// Java is `IQuestManager.GLOBAL_SERVER_QUESTS.get(id).getObjective(index)`. An
    /// unreadable one is `None` and behaves as the wrong class does — Java would
    /// throw on the `null` and, with no `try`, take the whole event down.
    pub fn block_broken(
        &mut self,
        block: &str,
        player: u128,
        mut objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        let mut dispatch = Dispatch::default();
        let holds = Self::holds_for(&self.break_block, block, Owner::Player(player));
        for hold in holds {
            match objective_of(hold) {
                Some(objective) if QuestMap::of(&objective) == Some(QuestMap::BreakBlock) => {
                    dispatch.reached.push(hold)
                }
                _ => {
                    self.forget(QuestMap::BreakBlock, block, Owner::Player(player), hold);
                    dispatch.stopped = true;
                    break;
                }
            }
        }
        dispatch
    }

    /// `BlockEvent.EntityPlaceEvent` (`:119-146`).
    pub fn block_placed(
        &mut self,
        block: &str,
        player: u128,
        objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        self.same_shape(QuestMap::PlaceBlock, block, player, objective_of)
    }

    /// `LivingDeathEvent` (`:90-112`): keyed by the **entity type** of the dead entity,
    /// and only when the killer is a player (`:93`).
    pub fn entity_killed(
        &mut self,
        kind: &str,
        player: u128,
        objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        self.same_shape(QuestMap::KillEntity, kind, player, objective_of)
    }

    fn same_shape(
        &mut self,
        map: QuestMap,
        key: &str,
        player: u128,
        mut objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        let mut dispatch = Dispatch::default();
        let holds = Self::holds_for(self.map(map), key, Owner::Player(player));
        for hold in holds {
            match objective_of(hold) {
                Some(objective) if QuestMap::of(&objective) == Some(map) => {
                    dispatch.reached.push(hold)
                }
                _ => {
                    self.forget(map, key, Owner::Player(player), hold);
                    dispatch.stopped = true;
                    break;
                }
            }
        }
        dispatch
    }

    /// `onResearchComplete` (`:237-250`): the colony's quests waiting on that
    /// research.
    ///
    /// No `break` and no removal here: an objective of the wrong class is skipped and
    /// the next one is still read. The port's `Dispatch::stopped` stays `false`.
    pub fn research_completed(
        &self,
        research: &str,
        colony: i32,
        mut objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        let mut dispatch = Dispatch::default();
        for hold in Self::holds_for(&self.research, research, Owner::Colony(colony)) {
            if let Some(objective) = objective_of(hold) {
                if QuestMap::of(&objective) == Some(QuestMap::Research) {
                    dispatch.reached.push(hold);
                }
            }
        }
        dispatch
    }

    /// `onBuildingUpgradeComplete` (`:257-270`): the same shape, keyed by the
    /// building entry.
    pub fn building_upgraded(
        &self,
        building: &str,
        colony: i32,
        mut objective_of: impl FnMut(Hold) -> Option<Objective>,
    ) -> Dispatch {
        let mut dispatch = Dispatch::default();
        for hold in Self::holds_for(&self.build_building, building, Owner::Colony(colony)) {
            if let Some(objective) = objective_of(hold) {
                if QuestMap::of(&objective) == Some(QuestMap::BuildBuilding) {
                    dispatch.reached.push(hold);
                }
            }
        }
        dispatch
    }

    /// The registrations of one key, copied before the loop — `new ArrayList<>(…)` at
    /// `:74`, `:98` and `:132`, which is what lets the loop remove from the live list.
    fn holds_for(entries: &[(String, Owner, Hold)], key: &str, owner: Owner) -> Vec<Hold> {
        entries
            .iter()
            .filter(|(held, held_owner, _)| held.as_str() == key && *held_owner == owner)
            .map(|(_, _, hold)| *hold)
            .collect()
    }

    /// Everything one player has registered, across the three event maps — what a
    /// logout would release if Java released it. It does not: this function exists so
    /// the port can say how much is left behind.
    pub fn held_by(&self, player: u128) -> Vec<(QuestMap, String, Hold)> {
        let mut out = Vec::new();
        for map in QuestMap::ALL.iter().filter(|map| map.per_player()) {
            for (key, owner, hold) in self.map(*map) {
                if *owner == Owner::Player(player) {
                    out.push((*map, key.clone(), *hold));
                }
            }
        }
        out
    }
}

impl QuestMap {
    /// The five, in the order the Java declares them.
    pub const ALL: &'static [QuestMap] = &[
        QuestMap::BuildBuilding,
        QuestMap::Research,
        QuestMap::BreakBlock,
        QuestMap::KillEntity,
        QuestMap::PlaceBlock,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests::objective::{Dialogue, Target};
    use mod_api::Identifier;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a namespaced id")
    }

    fn break_block(block: &str) -> Objective {
        Objective::BreakBlock {
            target: Target::Giver,
            block: id(block),
            count: 3,
            next: 1,
            unlocks: Vec::new(),
        }
    }

    fn place_block(block: &str) -> Objective {
        Objective::PlaceBlock {
            target: Target::Giver,
            block: id(block),
            count: 1,
            next: 1,
            unlocks: Vec::new(),
        }
    }

    fn research(research: &str) -> Objective {
        Objective::Research {
            target: Target::Giver,
            research: id(research),
            next: 1,
            unlocks: Vec::new(),
        }
    }

    fn hold(quest: u32, objective: i32) -> Hold {
        Hold { quest, objective }
    }

    const PLAYER: u128 = 42;

    #[test]
    fn the_five_maps_are_keyed_the_way_the_java_keys_them() {
        assert!(QuestMap::BreakBlock.per_player());
        assert!(QuestMap::PlaceBlock.per_player());
        assert!(QuestMap::KillEntity.per_player());
        assert!(!QuestMap::BuildBuilding.per_player());
        assert!(!QuestMap::Research.per_player());
        assert_eq!(QuestMap::ALL.len(), 5);
    }

    #[test]
    fn the_two_objectives_without_a_listener_land_in_no_map() {
        let dialogue = Objective::Dialogue {
            target: Target::Giver,
            tree: Dialogue {
                text: "hello".into(),
                replies: Vec::new(),
            },
            unlocks: Vec::new(),
        };
        assert_eq!(QuestMap::of(&dialogue), None);
        assert_eq!(QuestMap::of(&place_block("minecraft:dirt")), Some(QuestMap::PlaceBlock));
    }

    #[test]
    fn a_registered_quest_is_reached_once_per_event() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        let dispatch = registry.block_broken("minecraft:dirt", PLAYER, |_| {
            Some(break_block("minecraft:dirt"))
        });
        assert_eq!(dispatch.reached, vec![hold(1, 0)]);
        assert!(!dispatch.stopped);
    }

    #[test]
    fn a_quest_of_another_player_is_not_reached() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(7), hold(1, 0));
        let dispatch = registry
            .block_broken("minecraft:dirt", PLAYER, |_| Some(break_block("minecraft:dirt")));
        assert!(dispatch.is_empty());
    }

    #[test]
    fn an_objective_of_the_wrong_class_is_removed_and_stops_the_rest() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(2, 0));
        // The first hold resolves to a *place* objective: `:83` removes it and breaks.
        let dispatch = registry.block_broken("minecraft:dirt", PLAYER, |held| {
            if held.quest == 1 {
                Some(place_block("minecraft:dirt"))
            } else {
                Some(break_block("minecraft:dirt"))
            }
        });
        assert!(dispatch.stopped, "the `break` is reported, not hidden");
        assert!(dispatch.reached.is_empty(), "quest 2 was never read");
        assert_eq!(registry.len(QuestMap::BreakBlock), 1, "the wrong one is gone");
    }

    #[test]
    fn an_unreadable_objective_behaves_as_the_wrong_class_does() {
        // Java would `NullPointerException` on `getObjective(index)`; the port takes
        // the same branch the wrong class takes and says which quest stopped it.
        let mut registry = Registry::new();
        registry.listen(QuestMap::KillEntity, "minecraft:zombie", Owner::Player(PLAYER), hold(5, 2));
        let dispatch = registry.entity_killed("minecraft:zombie", PLAYER, |_| None);
        assert!(dispatch.stopped);
        assert_eq!(registry.len(QuestMap::KillEntity), 0);
    }

    #[test]
    fn the_same_quest_can_be_registered_twice_and_is_told_twice() {
        let mut registry = Registry::new();
        for _ in 0..2 {
            registry.listen(QuestMap::PlaceBlock, "minecraft:stone", Owner::Player(PLAYER), hold(1, 0));
        }
        let dispatch = registry.block_placed("minecraft:stone", PLAYER, |_| {
            Some(place_block("minecraft:stone"))
        });
        assert_eq!(dispatch.reached.len(), 2, "Java never de-duplicates");
    }

    #[test]
    fn forgetting_removes_one_occurrence_and_answers_whether_it_did() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        assert!(registry.forget(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0)));
        assert!(!registry.forget(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0)));
        assert_eq!(registry.len(QuestMap::BreakBlock), 0);
    }

    #[test]
    fn research_reaches_every_quest_and_stops_on_nobody() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::Research, "minecolonies:soft_shoes", Owner::Colony(3), hold(1, 0));
        registry.listen(QuestMap::Research, "minecolonies:soft_shoes", Owner::Colony(3), hold(2, 0));
        // The first objective is the wrong class, and `:237-250` has no `break`.
        let dispatch = registry.research_completed("minecolonies:soft_shoes", 3, |held| {
            if held.quest == 1 {
                Some(place_block("minecraft:dirt"))
            } else {
                Some(research("minecolonies:soft_shoes"))
            }
        });
        assert!(!dispatch.stopped);
        assert_eq!(dispatch.reached, vec![hold(2, 0)], "the second one is still read");
        assert_eq!(registry.len(QuestMap::Research), 2, "nothing is removed here");
    }

    #[test]
    fn a_colony_does_not_hear_another_colonys_research() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::Research, "minecolonies:soft_shoes", Owner::Colony(3), hold(1, 0));
        let dispatch = registry.research_completed("minecolonies:soft_shoes", 4, |_| {
            Some(research("minecolonies:soft_shoes"))
        });
        assert!(dispatch.is_empty());
    }

    #[test]
    fn a_building_upgrade_is_keyed_by_entry_and_colony() {
        let mut registry = Registry::new();
        registry.listen(
            QuestMap::BuildBuilding,
            "minecolonies:blockhutbuilder",
            Owner::Colony(3),
            hold(9, 1),
        );
        // The target is an **int** read from `details` (`BuildBuildingObjectiveTemplate.java:126`,
        // `details.get(TARGET_KEY).getAsInt()`), and every shipped building objective
        // writes `0` — measured over all 19 of them (`guards.json`, `farm.json`,
        // `builder2.json`, `housing2.json`…): `0` is the quest giver
        // (`DialogueObjectiveTemplateTemplate.java:111`), i.e. `Target::Giver` through
        // `Target::from_number`. No other value occurs in the data, and none is invented
        // here.
        let upgrade = Objective::BuildBuilding {
            target: Target::from_number(0),
            building: id("minecolonies:blockhutbuilder"),
            level: 2,
            count: 0,
            count_existing: false,
            next: 1,
            unlocks: Vec::new(),
        };
        assert_eq!(upgrade.target(), Target::Giver, "0 is the giver, not a third variant");
        let dispatch =
            registry.building_upgraded("minecolonies:blockhutbuilder", 3, |_| {
                Some(upgrade.clone())
            });
        assert_eq!(dispatch.reached, vec![hold(9, 1)]);
    }

    #[test]
    fn a_player_leaves_his_registrations_behind() {
        // `EventHandler.java:499-512` cleans its own position map and none of these;
        // this is the size of what it leaves.
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        registry.listen(QuestMap::PlaceBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(2, 0));
        registry.listen(QuestMap::KillEntity, "minecraft:zombie", Owner::Player(PLAYER), hold(3, 0));
        registry.listen(QuestMap::Research, "minecolonies:x", Owner::Colony(1), hold(4, 0));
        let held = registry.held_by(PLAYER);
        assert_eq!(held.len(), 3);
        assert!(held.iter().all(|(map, _, _)| map.per_player()));
    }

    #[test]
    fn a_key_with_no_hold_left_is_not_kept() {
        // The one part of Java's leak this representation closes by construction:
        // `breakBlockObjectives` would still hold `minecraft:dirt` with an empty list.
        let mut registry = Registry::new();
        registry.listen(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        assert_eq!(registry.keys(QuestMap::BreakBlock), vec!["minecraft:dirt"]);
        registry.forget(QuestMap::BreakBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(1, 0));
        assert!(registry.keys(QuestMap::BreakBlock).is_empty());
        assert!(registry.is_empty());
    }

    #[test]
    fn the_keys_of_two_blocks_come_back_once_each() {
        let mut registry = Registry::new();
        registry.listen(QuestMap::PlaceBlock, "minecraft:stone", Owner::Player(PLAYER), hold(1, 0));
        registry.listen(QuestMap::PlaceBlock, "minecraft:stone", Owner::Player(7), hold(2, 0));
        registry.listen(QuestMap::PlaceBlock, "minecraft:dirt", Owner::Player(PLAYER), hold(3, 0));
        assert_eq!(
            registry.keys(QuestMap::PlaceBlock),
            vec!["minecraft:stone", "minecraft:dirt"]
        );
    }

    #[test]
    fn the_registry_starts_empty_and_every_map_reads_zero() {
        let registry = Registry::new();
        assert!(registry.is_empty());
        for map in QuestMap::ALL {
            assert_eq!(registry.len(*map), 0);
        }
    }
}
