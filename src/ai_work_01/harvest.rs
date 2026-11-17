//! Les deux cueillettes de l'alchimiste : la verrue du nether et le gui.
//!
//! Référence Java : `EntityAIWorkAlchemist.java:121-218` (`harvestNetherWart`) et
//! `:234-304` (`harvestMistletoe`), plus `getNetherwartDrops` (`:220-227`).
//!
//! Les deux méthodes sont des machines à deux temps : d'abord choisir une case au
//! hasard et y marcher, ensuite agir sur place. Le temps 1 est ici une décision
//! (`Picked`, `Removed`, `Unloaded`), le temps 2 une autre, et la mémoire
//! (`WorkMemory::walk_to`) est ce qui les sépare — exactement le champ `walkTo` de
//! Java (`:91`).

use crate::ai::Pos;

use super::facts::{Above, AlchemistFacts};
use super::state::{craft_state, Scene, WorkMemory, WorkRefusal, WorkState};
use super::brewing::MISTLETOE_CHANCE;
use super::TICKS_SECOND;
use crate::ai_build::crafter::CraftState;

/// Ce que `harvestNetherWart` (`:121-218`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: une variante par `return` de `harvestNetherWart`
pub enum Wart {
    /// La hutte ne liste aucune terre (`:127-130`).
    NoSoil,
    /// La requête de verrue n'a pas pu être faite (`:145-148`, `:191-195`).
    NoWartRequest,
    /// Une terre a été choisie, le citoyen y va (`:138-141`, `:150-157`).
    Picked {
        /// La terre visée.
        at: Pos,
    },
    /// La case listée n'est plus de la soul sand : la hutte l'oublie (`:152-155`).
    Removed {
        /// La case oubliée.
        at: Pos,
    },
    /// La case tirée n'est pas chargée (`:134` faux) : le prochain tick retire.
    Unloaded,
    /// Pas encore sur place (`:162-165`).
    Walking,
    /// La verrue n'est pas mûre (`:170-174`).
    NotRipe {
        /// La terre portant la plante.
        at: Pos,
    },
    /// La plante est récoltée (`:176-187`).
    Mined {
        /// La terre qui la portait.
        at: Pos,
    },
    /// `mineBlock` n'a pas fini : le tick reste sur place (`:178` faux, `:217`).
    Mining {
        /// La terre portant la plante.
        at: Pos,
    },
    /// Aucune verrue en inventaire pour replanter (`:197-202`).
    NoWart,
    /// La verrue est replantée (`:204-208`).
    Planted {
        /// La terre replantée.
        at: Pos,
    },
    /// La case n'est plus de la soul sand ou n'est plus chargée (`:211-215`).
    Gone,
}

/// Ce que `harvestMistletoe` (`:234-304`) décide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: une variante par `return` de `harvestMistletoe`
pub enum Mistletoe {
    /// Pas de cisailles assez bonnes (`:236-239`).
    NoShears,
    /// La hutte ne liste aucune feuille (`:245-248`).
    NoLeaves,
    /// Une feuille a été choisie, le citoyen y va (`:250-256`).
    Picked {
        /// Le bloc visé.
        at: Pos,
    },
    /// La case listée n'est plus des feuilles : la hutte l'oublie (`:257-260`).
    Removed {
        /// La case oubliée.
        at: Pos,
    },
    /// La case tirée n'est pas chargée (`:251` faux).
    Unloaded,
    /// Pas encore sur place (`:267-270`).
    Walking,
    /// Le coup est donné, rien n'est tombé (`:303`).
    Hit,
    /// Un gui est tombé et l'inventaire est plein (`:286-294`).
    Found,
    /// Le bloc n'est plus des feuilles (`:297-301`).
    Gone,
}

/// `harvestNetherWart` (`:121-218`).
pub fn harvest_nether_wart(facts: &AlchemistFacts, memory: &WorkMemory) -> Wart {
    let Some(at) = memory.walk_to else {
        let Some(cell) = facts.picked_soil() else {
            return Wart::NoSoil; // `:127-130`
        };
        if !cell.loaded {
            return Wart::Unloaded; // `:134` faux → `:157`
        }
        if !cell.soul_sand {
            return Wart::Removed { at: cell.at }; // `:152-155`
        }
        if let Above::Empty = cell.above {
            if !facts.wart_request_ok {
                return Wart::NoWartRequest; // `:145-148`
            }
        }
        return Wart::Picked { at: cell.at }; // `:138-141`, `:150`
    };
    let Some(cell) = facts.soil.iter().find(|cell| cell.at == at) else {
        return Wart::Gone; // `:211-215`
    };
    if !cell.loaded || !cell.soul_sand {
        return Wart::Gone; // `:160`, `:211-215`
    }
    if !facts.at_work_pos {
        return Wart::Walking; // `:162-165`
    }
    match cell.above {
        // `:168-170` : une verrue pas mûre se laisse tranquille.
        Above::NetherWart { age } if age < 2 => Wart::NotRipe { at },
        // `:176-187` : le reste se casse, et le tick s'arrête sur place.
        Above::NetherWart { .. } | Above::Other => {
            if facts.mined {
                Wart::Mined { at }
            } else {
                Wart::Mining { at }
            }
        }
        // `:189-208` : la case est libre, on replante.
        Above::Empty => {
            if !facts.wart_request_ok {
                return Wart::NoWartRequest; // `:191-195`
            }
            if facts.wart_in_inventory == 0 {
                return Wart::NoWart; // `:197-202`
            }
            Wart::Planted { at } // `:204-208`
        }
    }
}

/// `harvestMistletoe` (`:234-304`).
///
/// `MISTLETOE_CHANCE` est le `nextInt(40) <= 0` de `:286`, c'est-à-dire « le tirage
/// vaut zéro », une chance sur quarante par coup.
pub fn harvest_mistletoe(facts: &AlchemistFacts, memory: &WorkMemory) -> Mistletoe {
    if facts.shears_missing {
        return Mistletoe::NoShears; // `:236-239`
    }
    let Some(at) = memory.walk_to else {
        let Some(leaf) = facts.picked_leaf() else {
            return Mistletoe::NoLeaves; // `:245-248`
        };
        if !leaf.loaded {
            return Mistletoe::Unloaded; // `:251` faux → `:262`
        }
        if !leaf.leaves {
            return Mistletoe::Removed { at: leaf.at }; // `:257-260`
        }
        return Mistletoe::Picked { at: leaf.at }; // `:255`
    };
    let Some(leaf) = facts.leaf_at(at) else {
        return Mistletoe::Gone; // `:297-301`
    };
    if !leaf.loaded || !leaf.leaves {
        return Mistletoe::Gone; // `:265`, `:297-301`
    }
    if !facts.at_work_pos {
        return Mistletoe::Walking; // `:267-270`
    }
    if facts.mistletoe_roll_forty % MISTLETOE_CHANCE == 0 {
        return Mistletoe::Found; // `:286-294`
    }
    Mistletoe::Hit // `:303`
}

/// `:112` — `harvestNetherWart`, avec les écritures de Java au passage.
pub fn nether_wart_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .alchemist()
        .map(|facts| harvest_nether_wart(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Wart::Picked { at } => {
            scene.memory.walk_to = Some(at); // `:150`
            Some(state) // `:157`
        }
        Wart::Removed { .. } | Wart::Unloaded => Some(state), // `:152-157`
        Wart::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:164`
            Some(state)
        }
        Wart::Mining { .. } => Some(state), // `:217`
        Wart::NoSoil => {
            scene.refuse(WorkRefusal::NoSoil); // `:129`
            Some(craft_state(CraftState::Idle))
        }
        Wart::NoWartRequest => {
            scene.memory.walk_to = None; // `:193`, `:200`
            scene.refuse(WorkRefusal::NoNetherWart); // `:147`, `:194`, `:201`
            Some(craft_state(CraftState::Idle))
        }
        Wart::NoWart => {
            scene.memory.walk_to = None; // `:200`
            scene.refuse(WorkRefusal::NoNetherWart);
            Some(craft_state(CraftState::Idle))
        }
        Wart::NotRipe { .. } => {
            scene.memory.walk_to = None; // `:172`
            scene.refuse(WorkRefusal::WartNotRipe);
            Some(craft_state(CraftState::Idle))
        }
        Wart::Mined { .. } | Wart::Planted { .. } => {
            scene.memory.walk_to = None; // `:184`, `:207`
            Some(craft_state(CraftState::Idle)) // `:186`, `:208`
        }
        Wart::Gone => {
            scene.memory.walk_to = None; // `:213`
            Some(craft_state(CraftState::Idle)) // `:214`
        }
    }
}

/// `:111` — `harvestMistletoe`.
pub fn mistletoe_action(scene: &mut Scene, state: WorkState) -> Option<WorkState> {
    let answer = scene
        .alchemist()
        .map(|facts| harvest_mistletoe(facts, &scene.memory));
    let Some(answer) = answer else {
        scene.refuse(WorkRefusal::NoFactsForTrade);
        return Some(state);
    };
    match answer {
        Mistletoe::Picked { at } => {
            scene.memory.walk_to = Some(at); // `:255`
            Some(state) // `:262`
        }
        Mistletoe::Removed { .. } | Mistletoe::Unloaded | Mistletoe::Hit => Some(state), // `:259-262`, `:303`
        Mistletoe::Walking => {
            scene.refuse(WorkRefusal::NotAtWorkPos); // `:269`
            Some(state)
        }
        Mistletoe::NoShears => {
            scene.refuse(WorkRefusal::NoShears); // `:238`
            Some(craft_state(CraftState::Idle))
        }
        Mistletoe::NoLeaves => {
            scene.refuse(WorkRefusal::NoLeaves); // `:247`
            Some(craft_state(CraftState::Idle))
        }
        Mistletoe::Found => {
            scene.memory.walk_to = None; // `:292`
            Some(craft_state(CraftState::InventoryFull)) // `:294`
        }
        Mistletoe::Gone => {
            scene.memory.walk_to = None; // `:299`
            Some(craft_state(CraftState::Idle)) // `:300`
        }
    }
}

/// Le rythme de la cible `HARVEST_MISTLETOE` / `HARVEST_NETHERWART`
/// (`EntityAIWorkAlchemist.java:111-112`, `TICKS_SECOND`).
pub const HARVEST_TICK_RATE: u32 = TICKS_SECOND;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_01::facts::{Leaf, Soil};
    use crate::ai_work_01::state::Scene;
    use crate::ai_work_01::facts::TradeFacts;

    fn facts() -> AlchemistFacts {
        AlchemistFacts::default()
    }

    /// `:211-215` : une case qui n'est plus de la soul sand est oubliée et le tick
    /// rend `IDLE`.
    #[test]
    fn a_soil_cell_that_is_no_longer_soul_sand_is_forgotten() {
        let mut facts = facts();
        facts.soil.push(Soil {
            at: Pos::new(1, 60, 1),
            loaded: true,
            soul_sand: false,
            above: Above::Empty,
        });
        assert_eq!(
            harvest_nether_wart(&facts, &WorkMemory::default()),
            Wart::Removed {
                at: Pos::new(1, 60, 1)
            }
        );
        let mut scene = Scene::new(
            crate::ai_build::crafter::CraftFacts::default(),
            TradeFacts::Alchemist(facts),
        );
        assert_eq!(
            nether_wart_action(&mut scene, WorkState::HarvestNetherWart),
            Some(WorkState::HarvestNetherWart)
        );
    }

    /// `:189-208` : sur une case libre, il faut la requête **et** une verrue en main.
    #[test]
    fn replanting_needs_a_wart_in_hand() {
        let mut facts = facts();
        facts.soil.push(Soil {
            at: Pos::new(1, 60, 1),
            loaded: true,
            soul_sand: true,
            above: Above::Empty,
        });
        facts.wart_request_ok = false;
        let memory = WorkMemory {
            walk_to: Some(Pos::new(1, 60, 1)),
            ..WorkMemory::default()
        };
        assert_eq!(harvest_nether_wart(&facts, &memory), Wart::NoWartRequest);
        facts.wart_request_ok = true;
        facts.wart_in_inventory = 1;
        assert_eq!(
            harvest_nether_wart(&facts, &memory),
            Wart::Planted {
                at: Pos::new(1, 60, 1)
            }
        );
    }

    /// `:265-303` : une feuille qui n'est plus des feuilles rend `Gone`, et un gui
    /// trouvé remplit l'inventaire.
    #[test]
    fn a_leaf_that_changed_sends_the_alchemist_home() {
        let mut facts = facts();
        facts.leaves.push(Leaf {
            at: Pos::new(2, 70, 2),
            loaded: true,
            leaves: false,
        });
        let memory = WorkMemory {
            walk_to: Some(Pos::new(2, 70, 2)),
            ..WorkMemory::default()
        };
        assert_eq!(harvest_mistletoe(&facts, &memory), Mistletoe::Gone);
        facts.leaves[0].leaves = true;
        facts.mistletoe_roll_forty = 0;
        assert_eq!(harvest_mistletoe(&facts, &memory), Mistletoe::Found);
        let mut scene = Scene::new(
            crate::ai_build::crafter::CraftFacts::default(),
            TradeFacts::Alchemist(facts),
        );
        let mut memory_scene = memory;
        memory_scene.walk_to = Some(Pos::new(2, 70, 2));
        scene.memory = memory_scene;
        assert_eq!(
            mistletoe_action(&mut scene, WorkState::HarvestMistletoe),
            Some(craft_state(CraftState::InventoryFull))
        );
    }

    /// `:250-260` : la case tirée décide entre marcher et oublier, et l'ordre
    /// `empty → leaves` de Java est celui des deux prédicats.
    #[test]
    fn picking_a_leaf_checks_the_block_before_walking() {
        let mut facts = facts();
        facts.leaves.push(Leaf {
            at: Pos::new(3, 70, 3),
            loaded: false,
            leaves: true,
        });
        assert_eq!(
            harvest_mistletoe(&facts, &WorkMemory::default()),
            Mistletoe::Unloaded
        );
        facts.leaves[0].loaded = true;
        assert_eq!(
            harvest_mistletoe(&facts, &WorkMemory::default()),
            Mistletoe::Picked {
                at: Pos::new(3, 70, 3)
            }
        );
        assert_eq!(HARVEST_TICK_RATE, 20, "`TICKS_SECOND`, `:111`");
    }
}
