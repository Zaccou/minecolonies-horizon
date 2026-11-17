//! L'enchanteur : une journée d'atelier, une nuit de drain, et l'enchantement des tomes.
//!
//! Référence Java : `core/entity/ai/workers/service/EntityAIWorkEnchanter.java` (498
//! lignes) — le constructeur (`:105-113`), `decide` (`:120-205`), `hasWorkToDo`
//! (`:207-212`), `getActionsDoneUntilDumping` (`:214-218`), `enchant` (`:225-288`),
//! `gatherAndDrain` (`:306-433`) et `resetDraining` (`:438-446`) — sur
//! `colony/jobs/JobEnchanter.java` (les ticks d'attente) et
//! `colony/buildings/modules/EnchanterStationsModule.java` (les stations).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `START_WORKING` | `decide` (écrasé par l'enchanteur) | 5 | `AbstractEntityAICrafting.java:166` |
//! | 2 | `ENCHANTER_DRAIN` | `gatherAndDrain` | 10 | `EntityAIWorkEnchanter.java:109` |
//! | 3 | `ENCHANT` | `enchant` | 20 | `:110` |
//!
//! L'enchanteur hérite de `AbstractEntityAICrafting` (`:53`), donc son `IDLE` est celui
//! de l'atelier : `IDLE → hasWorkToDo → START_WORKING` à `TICKS_SECOND`
//! (`AbstractEntityAICrafting.java:164`), et `hasWorkToDo()` rend **toujours** vrai
//! (`EntityAIWorkEnchanter.java:207-212`). C'est pourquoi un enchanteur ne s'arrête
//! jamais : il dort moins et draine la nuit.
//!
//! ## Le fait qui décide de la moitié de la journée
//!
//! `:129-133` : `if (craftState != START_WORKING && !WorldUtil.isPastTime(world, 6000))
//! return craftState;` et `isPastTime` est `getDayTime() % 24000 <= 6000`
//! (`WorldUtil.java:176-179`). La garde est **toujours vraie** : `getNextCraftingState`
//! ne peut pas rendre `START_WORKING`, ses quatre sorties étant `IDLE` (`:302`),
//! `INVENTORY_FULL` (`:308`), `QUERY_ITEMS` (`:313`) et `GET_RECIPE` (`:316`). Donc, tel
//! que Java est écrit, l'enchanteur tient **l'atelier** dès que la journée a passé 6 000
//! ticks, et ne draine qu'avant. Le port écrit la comparaison telle quelle
//! ([`decide`]) et un test prouve que la garde ne peut pas refuser — c'est un défaut de
//! Java, pas une décision de portage (`RAPPORT.md` §4).
//!
//! ## Ce que ce fichier ne fait pas
//!
//! Il ne porte **pas** la boucle d'atelier : `QUERY_ITEMS`, `GET_RECIPE` et `CRAFT`
//! appartiennent à `AbstractEntityAICrafting` et sont déjà portés une fois dans
//! `crate::ai_build::crafter` (avec `next_crafting_state`, réutilisé ici). La décision de
//! l'enchanteur rend donc [`Effect::Crafted`] avec l'état que l'atelier doit prendre — et
//! les deux autres réponses de `getNextCraftingState`, `IDLE` (`:302`) et
//! `INVENTORY_FULL` (`:308`), sont des états de **ce** module et sont rendues comme tels,
//! parce que Java fait `return craftState`. Idem pour les particules, les sons et le
//! changement d'objet en main : des ordres, pas des décisions.

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};
use crate::ai::worker::STANDARD_DELAY;
use crate::ai_build::crafter::{next_crafting_state, CraftState};

use super::facts::{EnchanterFacts, EnchanterMemory, ServiceMemory};
use super::state::{ServiceRefusal, ServiceState};
use super::{Decision, Effect, add_target, at_hut, service_action};

/// `EntityAIWorkEnchanter.MIN_DISTANCE_TO_DRAIN` (`:68`) : sous cette distance, le
/// citoyen drainé est assez près pour commencer (`:363`).
pub const MIN_DISTANCE_TO_DRAIN: f64 = 10.0;
/// `EntityAIWorkEnchanter.MAX_PROGRESS_TICKS` (`:73`) : soixante ticks de drain avant
/// le paiement (`:375`).
pub const MAX_PROGRESS_TICKS: i32 = 60;
/// `EntityAIWorkEnchanter.MAX_ENCHANTMENT_TICKS` (`:78`) : `60 * 5` ticks, **divisés**
/// par le niveau de la hutte (`:235`) — 300 ticks à niveau 1, 60 à niveau 5.
pub const MAX_ENCHANTMENT_TICKS: i32 = 60 * 5;
/// `EntityAIWorkEnchanter.MANA_REQ_PER_LEVEL` (`:83`) : la mana minimale par niveau de
/// hutte sous laquelle l'enchanteur draine au lieu d'enchanter (`:141`).
pub const MANA_REQ_PER_LEVEL: i32 = 10;
/// `EntityAIWorkEnchanter.XP_PER_DRAIN` (`:88`) : l'expérience d'un drain réussi
/// (`:427`).
pub const XP_PER_DRAIN: f64 = 10.0;
/// `EntityAIWorkEnchanter` : la cadence du drain (`:109`).
pub const DRAIN_RATE: u32 = 10;
/// `EntityAIWorkEnchanter` : la cadence de l'enchantement (`:110`).
pub const ENCHANT_RATE: u32 = 20;
/// `JobEnchanter.MAX_WAITING_TICKS` (`JobEnchanter.java:22`) : les ticks d'attente d'un
/// citoyen trop loin avant d'abandonner le drain (`:365`).
pub const MAX_WAITING_TICKS: i32 = 60;
/// `EntityAIWorkEnchanter` : `isPastTime(world, 6000)` (`:130`) — 6 000 ticks de la
/// journée.
pub const PAST_TIME: i64 = 6000;
/// `EntityAIWorkEnchanter.getActionsDoneUntilDumping` (`:214-218`) : une action entre
/// deux vidages, comme tous les métiers d'atelier (`AbstractEntityAICrafting.java:724`).
pub const ACTIONS_UNTIL_DUMPING: u32 = 1;
/// `Items.BOOK` (`:164`) et `IS_BOOK` (`:63`).
pub const BOOK: &str = "minecraft:book";
/// `ModItems.ancientTome` (`:199`) et `IS_ANCIENT_TOME` (`:58`).
pub const ANCIENT_TOME: &str = "minecolonies:ancient_tome";
/// `TranslationConstants.NO_WORKERS_TO_DRAIN_SET`, importé par
/// `EntityAIWorkEnchanter.java:46` et lu à `:150`.
pub const NO_WORKERS_TO_DRAIN_SET: &str = "com.minecolonies.coremod.enchanter.noworkers";

/// `hasWorkToDo()` (`:207-212`) : l'enchanteur a **toujours** du travail — « either craft
/// or go gather xp ».
pub fn has_work_to_do() -> bool {
    true
}

/// `decide()` (`:120-205`), condition par condition, dans l'ordre de Java.
///
/// L'ordre est la règle : la marche (`:124`), puis l'atelier (`:129`), puis le vidage
/// (`:135`), puis les deux branches qui s'excluent — drainer sous le seuil de mana
/// (`:141`), enchanter au-dessus (`:188-204`).
pub fn decide(facts: &WorldFacts, trade: &EnchanterFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = memory;
    // `worker.setItemInHand(MAIN_HAND, ItemStack.EMPTY);` (`:123`) — un ordre, pas une
    // décision, mais il passe avant tout le reste.
    let decision = Decision::stay().effect(Effect::EmptyMainHand);
    if !at_hut(facts) {
        return decision.because(ServiceRefusal::NotAtHut); // `:124-127`
    }
    let craft = next_crafting_state(&trade.craft); // `:129`
    if craft != CraftState::StartWorking && !trade.past_six_thousand {
        // `:130-133` — `return craftState`, tel quel. Deux de ses quatre réponses sont des
        // états de **ce** module et sont donc rendues comme états : `IDLE` (`:302`) et
        // `INVENTORY_FULL` (`:308`). Les trois de l'atelier (`QUERY_ITEMS`, `GET_RECIPE`,
        // `CRAFT`) ne sont pas dans `ServiceState` et sortent en ordre (`MANQUANT.md` §4).
        // La garde `craftState != START_WORKING` ne peut pas refuser : voir l'en-tête.
        return match craft {
            CraftState::Idle => decision
                .because(ServiceRefusal::CraftingDue)
                .going(ServiceState::Base(WorkerState::Idle)),
            CraftState::InventoryFull => decision
                .because(ServiceRefusal::CraftingDue)
                .going(ServiceState::Base(WorkerState::InventoryFull)),
            other => decision
                .because(ServiceRefusal::CraftingDue)
                .effect(Effect::Crafted { state: other }),
        };
    }
    if dump_due(facts, trade) {
        return decision.because(ServiceRefusal::DumpDue); // `:135-139`
    }
    if trade.needs_drain() {
        return drain_or_idle(trade, decision); // `:141-175`
    }
    // `:177-186` — une recette de tome désactivée par le joueur désactive la branche.
    if !trade.ancient_tome_crafting_disabled && trade.ancient_tomes_in_inventory == 0 {
        if trade.ancient_tomes_in_building > 0 {
            return decision
                .because(ServiceRefusal::AncientTomeInHutOnly)
                .effect(Effect::FetchItem {
                    item: ANCIENT_TOME,
                    count: 1,
                })
                .going(ServiceState::Base(WorkerState::GatheringRequiredMaterials)); // `:193-198`
        }
        return decision
            .because(ServiceRefusal::NoAncientTome)
            .effect(Effect::RequestItem {
                item: ANCIENT_TOME,
                count: 1,
            })
            .going(ServiceState::Base(WorkerState::Idle)); // `:199-200`
    }
    decision.going(ServiceState::Enchant) // `:204`
}

/// La branche de `:141-175` : « pas assez de mana, donc drainer ».
fn drain_or_idle(trade: &EnchanterFacts, decision: Decision) -> Decision {
    if trade.drain_stations.is_empty() {
        return decision
            .because(ServiceRefusal::NoDrainStation)
            .effect(Effect::Say {
                key: NO_WORKERS_TO_DRAIN_SET,
            })
            .going(ServiceState::Base(WorkerState::Idle)); // `:145-153`
    }
    if trade.books_in_inventory == 0 {
        if trade.books_in_building > 0 {
            return decision
                .because(ServiceRefusal::BookInHutOnly)
                .effect(Effect::FetchItem {
                    item: BOOK,
                    count: 1,
                })
                .going(ServiceState::Base(WorkerState::GatheringRequiredMaterials)); // `:158-163`
        }
        return decision
            .because(ServiceRefusal::NoBook)
            .effect(Effect::RequestItem {
                item: BOOK,
                count: 1,
            })
            .going(ServiceState::Base(WorkerState::Idle)); // `:164-165`
    }
    match trade.random_drain_building() {
        None => decision.because(ServiceRefusal::NoGatherableBuilding), // `:168-171`
        Some(at) => decision
            .effect(Effect::SetDrainTarget { at })
            .going(ServiceState::EnchanterDrain), // `:173-174`
    }
}

/// `inventoryNeedsDump` pour l'enchanteur (`AbstractEntityAIBasic.java:406-417`) : son
/// seuil est d'**une** action (`:214-218`) et `wantInventoryDumped()` rend faux.
pub fn dump_due(facts: &WorldFacts, trade: &EnchanterFacts) -> bool {
    facts.inventory.full // `:412`
        || facts.inventory.actions_done >= ACTIONS_UNTIL_DUMPING // `:413`
        || trade.want_inventory_dumped // `:414`
}

/// `enchant()` (`:225-288`).
///
/// `progressTicks++ < MAX_ENCHANTMENT_TICKS / niveau` : Java **post-incrémente**, donc la
/// comparaison porte sur l'ancienne valeur et le budget est exactement
/// `MAX_ENCHANTMENT_TICKS / niveau` ticks (`:235`).
pub fn enchant(facts: &WorldFacts, trade: &EnchanterFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = facts;
    if !trade.enchant_recipe_available {
        memory.enchanter.progress_ticks = 0; // `:231`
        return Decision::go(ServiceState::Base(WorkerState::StartWorking))
            .because(ServiceRefusal::NoEnchantingRecipe); // `:232`
    }
    // Un niveau 0 diviserait par zéro ; Java ne peut pas le produire (une hutte existe
    // toujours à au moins 1), le port refuse de paniquer sur une donnée.
    let level = i32::from(trade.building_level.max(1));
    let budget = MAX_ENCHANTMENT_TICKS / level; // `:235`
    let ticked = memory.enchanter.progress_ticks;
    memory.enchanter.progress_ticks = ticked.saturating_add(1);
    if ticked < budget {
        return Decision::stay().because(ServiceRefusal::Enchanting); // `:235-266`
    }
    // `:268-283` — le butin, la mana retirée, l'action comptée, puis `:285-287`.
    memory.enchanter.progress_ticks = 0; // `:286`
    Decision::go(ServiceState::Base(WorkerState::Idle)) // `:287`
        .effect(Effect::Enchanted {
            mana_spent: trade.enchant_mana_cost,
        })
        .effect(Effect::ActionsDone { count: 1 }) // `incrementActionsDoneAndDecSaturation` (`:281`)
}

/// `gatherAndDrain()` (`:306-433`) : aller au bâtiment, choisir un citoyen, le drainer.
pub fn gather_and_drain(
    facts: &WorldFacts,
    trade: &EnchanterFacts,
    memory: &mut ServiceMemory,
) -> Decision {
    let Some(target) = memory.enchanter.drain_target else {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NoDrainTarget); // `:308-311`
    };
    if !trade.at_drain_building {
        return Decision::stay().because(ServiceRefusal::NotAtHut); // `:314-317`
    }
    if !trade.drain_building_exists {
        // `:319-324` — `resetDraining()`, puis la station est retirée, puis `IDLE`.
        let mut decision = Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::DrainBuildingGone);
        decision.effects.extend(reset_draining(memory));
        decision.effects.push(Effect::DropStation { at: target });
        return decision;
    }
    // `:326-352` — choisir le citoyen une fois, puis s'en souvenir.
    let citizen = match memory.enchanter.drain_citizen {
        Some(id) => id,
        None => {
            if trade.workers.is_empty() {
                // `:341-345` — `resetDraining()` et retour au travail.
                let mut decision = Decision::go(ServiceState::Base(WorkerState::StartWorking))
                    .because(ServiceRefusal::NoWorkerToDrain);
                decision.effects.extend(reset_draining(memory));
                return decision;
            }
            // `:335-347` — plus d'un citoyen : un tirage ; un seul : le premier.
            let picked = trade.workers[trade.worker_draw % trade.workers.len()];
            memory.enchanter.drain_citizen = Some(picked.id); // `:349`
            memory.enchanter.progress_ticks = 0; // `:350`
            return Decision::stay(); // `:351`
        }
    };
    let Some(worker) = trade.worker(citizen) else {
        memory.enchanter.drain_citizen = None;
        return Decision::stay().because(ServiceRefusal::DrainTargetAbsent); // `:354-358`
    };
    if !worker.present {
        memory.enchanter.drain_citizen = None;
        return Decision::stay().because(ServiceRefusal::DrainTargetAbsent); // `:354-358`
    }
    if memory.enchanter.progress_ticks == 0 {
        // `:360-372` — trop loin : attendre, et au bout de soixante ticks abandonner.
        if facts.worker.at.dist_2d(worker.at) > MIN_DISTANCE_TO_DRAIN {
            if !increment_waiting_ticks(&mut memory.enchanter) {
                let mut decision = Decision::go(ServiceState::Base(WorkerState::StartWorking))
                    .because(ServiceRefusal::DrainWalkGaveUp);
                decision.effects.extend(reset_draining(memory));
                return decision;
            }
            return Decision::stay().because(ServiceRefusal::DrainTooFar);
        }
    }
    memory.enchanter.progress_ticks += 1; // `:374`
    if memory.enchanter.progress_ticks < MAX_PROGRESS_TICKS {
        return Decision::stay().because(ServiceRefusal::Draining); // `:375-406`
    }
    // `:408-430` — un livre en main vaut l'XP, la mana et un essai d'enchantement.
    let mut decision = Decision::stay();
    if trade.book_in_hand {
        decision = decision.effect(Effect::Drained {
            xp: XP_PER_DRAIN,
            mana_gain: 1,
            book_spent: true,
        });
    } else {
        decision = decision
            .because(ServiceRefusal::NoBookToSpend)
            .effect(Effect::Drained {
                xp: 0.0,
                mana_gain: 0,
                book_spent: false,
            });
    }
    let effects = reset_draining(memory);
    let mut decision = decision.going(ServiceState::Base(WorkerState::Idle)); // `:432`
    decision.effects.extend(effects);
    decision
}

/// `JobEnchanter.incrementWaitingTicks` (`JobEnchanter.java:110-118`) : incrémente et
/// rend faux au-delà de [`MAX_WAITING_TICKS`], en remettant le compteur à zéro.
pub fn increment_waiting_ticks(memory: &mut EnchanterMemory) -> bool {
    memory.waiting_ticks += 1;
    if memory.waiting_ticks > MAX_WAITING_TICKS {
        memory.waiting_ticks = 0;
        return false;
    }
    true
}

/// `resetDraining()` (`:438-446`) : marquer la station comme drainée, oublier le citoyen
/// et la cible, et compter une action.
///
/// Java écrit `setAsGathered(job.getPosToDrainFrom())` **sans** regarder si la cible est
/// nulle ; le port n'émet l'ordre que si elle existe, parce qu'une position nulle dans le
/// module n'est pas un état que ce port sait produire.
pub fn reset_draining(memory: &mut ServiceMemory) -> Vec<Effect> {
    let mut effects = Vec::new();
    if let Some(at) = memory.enchanter.drain_target {
        effects.push(Effect::MarkStationGathered { at }); // `:441`
    }
    memory.enchanter.drain_citizen = None; // `:442`
    memory.enchanter.drain_target = None; // `:443`
    memory.enchanter.progress_ticks = 0; // `:444`
    effects.push(Effect::ActionsDone { count: 1 }); // `:445`
    effects
}

service_action!(decide_action, Enchanter, decide);
service_action!(enchant_action, Enchanter, enchant);
service_action!(gather_and_drain_action, Enchanter, gather_and_drain);

/// Enregistre les trois cibles de l'enchanteur.
///
/// `IDLE → START_WORKING` (`AbstractEntityAICrafting.java:164`) est celle du passage de
/// relais : elle est commune aux métiers d'atelier et l'enregistrer deux fois mettrait
/// deux règles dans la liste de `IDLE`.
pub fn register(machine: &mut Machine<ServiceState, super::ServiceScene>) {
    // `:166` — `new AITarget(START_WORKING, this::decide, STANDARD_DELAY)`, dont
    // l'enchanteur écrase le corps (`:120-205`).
    add_target(
        machine,
        ServiceState::Base(WorkerState::StartWorking),
        "decide",
        "AbstractEntityAICrafting.java:166",
        STANDARD_DELAY,
        decide_action,
    );
    // `:109` — `new AITarget(ENCHANTER_DRAIN, this::gatherAndDrain, 10)`.
    add_target(
        machine,
        ServiceState::EnchanterDrain,
        "gatherAndDrain",
        "EntityAIWorkEnchanter.java:109",
        DRAIN_RATE,
        gather_and_drain_action,
    );
    // `:110` — `new AITarget(ENCHANT, this::enchant, TICKS_SECOND)`.
    add_target(
        machine,
        ServiceState::Enchant,
        "enchant",
        "EntityAIWorkEnchanter.java:110",
        ENCHANT_RATE,
        enchant_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_build::crafter::CraftFacts;
    use crate::ai_work_12::facts::{DrainWorker, EnchanterMemory, ServiceJob, ServiceScene};
    use crate::ai_work_12::ServiceAi;

    /// Un enchanteur de l'après-midi : `isPastTime(world, 6000)` est **faux**
    /// (`:130`), donc la branche d'atelier est sautée et la boucle de `:141-204`
    /// s'applique. C'est l'heure où Java draine et enchante.
    fn trade() -> EnchanterFacts {
        EnchanterFacts {
            past_six_thousand: true,
            ..EnchanterFacts::default()
        }
    }

    /// Un enchanteur sous le seuil de mana (`:141`) : `0 < 1 * 10`, avec une station.
    fn unskilled() -> EnchanterFacts {
        EnchanterFacts {
            primary_skill: 0,
            building_level: 1,
            drain_stations: vec![Pos::new(1, 64, 1)],
            ..trade()
        }
    }

    #[test]
    fn the_crafting_guard_can_never_refuse_because_the_craft_state_is_never_start_working() {
        // `EntityAIWorkEnchanter.java:129-133` : la garde `craftState != START_WORKING` est
        // ecrite telle quelle, et `getNextCraftingState` ne rend jamais `START_WORKING`
        // (`AbstractEntityAICrafting.java:300-317`).
        let mut craft = CraftFacts::default();
        let mut seen = Vec::new();
        for current_task in [false, true] {
            for has_recipe in [false, true] {
                for has_request in [false, true] {
                    craft.current_task = current_task;
                    craft.has_recipe = has_recipe;
                    craft.has_request = has_request;
                    seen.push(next_crafting_state(&craft));
                }
            }
        }
        assert!(
            !seen.contains(&CraftState::StartWorking),
            "la garde de :130 ne peut pas etre fausse"
        );
        // Et pourtant la comparaison existe : avec un atelier du et l'apres-midi, il part
        // a l'atelier, et l'ordre dit lequel des trois etats l'appelant doit prendre.
        let facts = WorldFacts::new();
        let trade = EnchanterFacts {
            craft: CraftFacts {
                current_task: true,
                ..CraftFacts::default()
            },
            past_six_thousand: false,
            ..EnchanterFacts::default()
        };
        let decision = decide(&facts, &trade, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::CraftingDue));
        assert_eq!(
            decision.effects[1],
            Effect::Crafted {
                state: CraftState::GetRecipe
            }
        );
        // La meme journee passee, sous le seuil de mana et sans station : le drain
        // reprend la main (`:141-153`).
        let late = EnchanterFacts {
            past_six_thousand: true,
            building_level: 1,
            ..trade
        };
        let decision = decide(&facts, &late, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoDrainStation));
    }

    #[test]
    fn decide_walks_the_walk_the_dump_the_station_and_the_book_in_java_s_order() {
        // `:123-174`, une branche par `return`, dans l'ordre de Java.
        // 1. `!walkToBuilding()` (`:124-127`), et `:123` passe deja l'objet en main.
        let mut facts = WorldFacts::new();
        facts.worker.at = Pos::new(40, 64, 40);
        let walking = decide(&facts, &trade(), &mut ServiceMemory::default());
        assert_eq!(walking.next, None);
        assert_eq!(walking.refusal, Some(ServiceRefusal::NotAtHut));
        assert_eq!(walking.effects, vec![Effect::EmptyMainHand]);

        // 2. `wantInventoryDumped()` (`:135-139`) : la branche que Java rend morte, parce
        // que `AbstractEntityAIBasic.wantInventoryDumped()` rend faux (`:435-438`).
        let mut facts = WorldFacts::new();
        let mut wished = trade();
        wished.want_inventory_dumped = true;
        assert_eq!(
            decide(&facts, &wished, &mut ServiceMemory::default()).refusal,
            Some(ServiceRefusal::DumpDue)
        );
        // Le seuil de vidage de l'enchanteur est d'une action (`:214-218`).
        assert!(!dump_due(&facts, &trade()), "zero action");
        facts.inventory.actions_done = 1;
        assert!(dump_due(&facts, &trade()));
        facts.inventory.actions_done = 0;

        // 3. Aucune station (`:145-153`).
        let no_station = EnchanterFacts {
            primary_skill: 0,
            building_level: 1,
            ..trade()
        };
        let decision = decide(&facts, &no_station, &mut ServiceMemory::default());
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoDrainStation));
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::Say {
                key: NO_WORKERS_TO_DRAIN_SET
            })
        );

        // 4. Aucun livre nulle part (`:155-166`).
        let decision = decide(&facts, &unskilled(), &mut ServiceMemory::default());
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoBook));
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::RequestItem {
                item: BOOK,
                count: 1
            })
        );

        // 5. Le livre est dans la hutte (`:158-163`).
        let in_hut = EnchanterFacts {
            books_in_building: 3,
            ..unskilled()
        };
        let decision = decide(&facts, &in_hut, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::BookInHutOnly));
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::GatheringRequiredMaterials))
        );
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::FetchItem {
                item: BOOK,
                count: 1
            })
        );

        // 6. Une station tiree devient la cible du metier (`:168-174`).
        let at = Pos::new(4, 65, 4);
        let ready = EnchanterFacts {
            primary_skill: 29,
            building_level: 3, // 29 < 30
            drainable: vec![at],
            books_in_inventory: 2,
            ..unskilled()
        };
        let decision = decide(&facts, &ready, &mut ServiceMemory::default());
        assert_eq!(decision.next, Some(ServiceState::EnchanterDrain));
        assert_eq!(decision.effects.last(), Some(&Effect::SetDrainTarget { at }));

        // 7. Toutes les stations sont deja drainees (`:168-171`).
        let gathered = EnchanterFacts {
            drainable: Vec::new(),
            ..ready
        };
        let decision = decide(&facts, &gathered, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoGatherableBuilding));
        assert_eq!(decision.next, None, "reste dans START_WORKING");
    }

    #[test]
    fn a_skilled_enchanter_goes_to_the_tome_or_to_the_enchantment() {
        // `:188-204` : la branche du tome, puis `ENCHANT`.
        let facts = WorldFacts::new();
        let trained = EnchanterFacts {
            primary_skill: 30,
            building_level: 3, // 30 n'est pas < 30
            ancient_tomes_in_inventory: 1,
            enchant_recipe_available: true,
            ..trade()
        };
        let decision = decide(&facts, &trained, &mut ServiceMemory::default());
        assert_eq!(decision.next, Some(ServiceState::Enchant));
        assert_eq!(decision.refusal, None);

        // Aucun tome, mais la hutte en a un (`:193-198`).
        let hut = EnchanterFacts {
            ancient_tomes_in_inventory: 0,
            ancient_tomes_in_building: 2,
            ..trained.clone()
        };
        let decision = decide(&facts, &hut, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::AncientTomeInHutOnly));
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::FetchItem {
                item: ANCIENT_TOME,
                count: 1
            })
        );

        // Aucun tome nulle part (`:199-200`).
        let none = EnchanterFacts {
            ancient_tomes_in_inventory: 0,
            ancient_tomes_in_building: 0,
            ..trained.clone()
        };
        let decision = decide(&facts, &none, &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoAncientTome));
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));

        // Recette de tome desactivee (`:177-186`) : ni tome ni requete, on enchante.
        let disabled = EnchanterFacts {
            ancient_tome_crafting_disabled: true,
            ancient_tomes_in_inventory: 0,
            ancient_tomes_in_building: 5,
            ..trained
        };
        let decision = decide(&facts, &disabled, &mut ServiceMemory::default());
        assert_eq!(decision.next, Some(ServiceState::Enchant));
        assert_eq!(decision.refusal, None);
        assert!(has_work_to_do(), "`:207-212` : l'enchanteur a toujours du travail");
    }

    #[test]
    fn the_drain_holds_while_walking_and_resets_when_the_building_or_the_worker_is_gone() {
        // `:308-358`.
        let facts = WorldFacts::new();
        // `job.getPosToDrainFrom() == null` (`:308-311`).
        let decision = gather_and_drain(&facts, &trade(), &mut ServiceMemory::default());
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoDrainTarget));
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));

        // `walkToBuilding(buildingWorker)` faux (`:314-317`).
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(Pos::new(4, 64, 4));
        let walking = EnchanterFacts {
            at_drain_building: false,
            ..EnchanterFacts::default()
        };
        let decision = gather_and_drain(&facts, &walking, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NotAtHut));
        assert_eq!(decision.next, None);

        // Le batiment n'existe plus (`:319-324`) : station marquee, puis retiree.
        let at = Pos::new(4, 64, 4);
        let gone = EnchanterFacts {
            at_drain_building: true,
            drain_building_exists: false,
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(at);
        memory.enchanter.drain_citizen = Some(9);
        let decision = gather_and_drain(&facts, &gone, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::DrainBuildingGone));
        assert_eq!(memory.enchanter.drain_target, None);
        assert!(decision.effects.contains(&Effect::MarkStationGathered { at }));
        assert!(decision.effects.contains(&Effect::DropStation { at }));

        // Aucun citoyen affecte (`:341-345`).
        let empty = EnchanterFacts {
            at_drain_building: true,
            drain_building_exists: true,
            workers: Vec::new(),
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(at);
        let decision = gather_and_drain(&facts, &empty, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoWorkerToDrain));
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::StartWorking))
        );

        // Le premier passage choisit un citoyen et s'en souvient (`:326-352`).
        let two = EnchanterFacts {
            at_drain_building: true,
            drain_building_exists: true,
            workers: vec![
                DrainWorker::present(3, Pos::new(1, 64, 1)),
                DrainWorker::present(8, Pos::new(2, 64, 2)),
            ],
            worker_draw: 1,
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(at);
        let decision = gather_and_drain(&facts, &two, &mut memory);
        assert_eq!(decision.next, None, "`:351` reste dans ENCHANTER_DRAIN");
        assert_eq!(memory.enchanter.drain_citizen, Some(8));

        // Le citoyen a perdu son entite (`:354-358`) : le choix est oublie.
        let absent = EnchanterFacts {
            workers: vec![DrainWorker {
                present: false,
                ..DrainWorker::present(3, Pos::new(1, 64, 1))
            }],
            ..two
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(at);
        memory.enchanter.drain_citizen = Some(3);
        let decision = gather_and_drain(&facts, &absent, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::DrainTargetAbsent));
        assert_eq!(memory.enchanter.drain_citizen, None);
    }

    #[test]
    fn a_drain_too_far_waits_sixty_ticks_and_then_gives_up() {
        // `:360-372` et `JobEnchanter.incrementWaitingTicks` (`:110-118`).
        let facts = WorldFacts::new(); // le citoyen est en 0,64,0
        let mut trade = EnchanterFacts {
            at_drain_building: true,
            drain_building_exists: true,
            workers: vec![DrainWorker::present(3, Pos::new(30, 64, 0))],
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(Pos::new(4, 64, 4));
        memory.enchanter.drain_citizen = Some(3);
        for _ in 0..MAX_WAITING_TICKS {
            let decision = gather_and_drain(&facts, &trade, &mut memory);
            assert_eq!(decision.refusal, Some(ServiceRefusal::DrainTooFar));
            assert_eq!(decision.next, None);
        }
        assert_eq!(memory.enchanter.waiting_ticks, MAX_WAITING_TICKS);
        let gave_up = gather_and_drain(&facts, &trade, &mut memory);
        assert_eq!(gave_up.refusal, Some(ServiceRefusal::DrainWalkGaveUp));
        assert_eq!(
            gave_up.next,
            Some(ServiceState::Base(WorkerState::StartWorking))
        );
        assert_eq!(memory.enchanter.waiting_ticks, 0, "le compteur repart de zero");
        // Un citoyen a portee ne fait pas monter le compteur (`:363`).
        trade.workers[0].at = Pos::new(3, 64, 0);
        memory.enchanter.drain_target = Some(Pos::new(4, 64, 4));
        memory.enchanter.drain_citizen = Some(3);
        let close = gather_and_drain(&facts, &trade, &mut memory);
        assert_eq!(close.refusal, Some(ServiceRefusal::Draining));
        assert_eq!(memory.enchanter.waiting_ticks, 0);
    }

    #[test]
    fn a_drain_runs_sixty_ticks_then_pays_with_the_book_or_names_the_missing_one() {
        // `:374-430` puis `resetDraining` (`:438-446`).
        let facts = WorldFacts::new();
        let ready = EnchanterFacts {
            at_drain_building: true,
            drain_building_exists: true,
            workers: vec![DrainWorker::present(3, Pos::new(1, 64, 1))],
            book_in_hand: true,
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(Pos::new(4, 64, 4));
        memory.enchanter.drain_citizen = Some(3);
        for _ in 1..MAX_PROGRESS_TICKS {
            assert_eq!(
                gather_and_drain(&facts, &ready, &mut memory).refusal,
                Some(ServiceRefusal::Draining)
            );
        }
        assert_eq!(memory.enchanter.progress_ticks, MAX_PROGRESS_TICKS - 1);
        let paid = gather_and_drain(&facts, &ready, &mut memory);
        assert_eq!(paid.next, Some(ServiceState::Base(WorkerState::Idle)));
        assert!(paid.effects.contains(&Effect::Drained {
            xp: XP_PER_DRAIN,
            mana_gain: 1,
            book_spent: true
        }));
        assert!(paid.effects.contains(&Effect::MarkStationGathered {
            at: Pos::new(4, 64, 4)
        }));
        assert!(
            paid.effects.contains(&Effect::ActionsDone { count: 1 }),
            "`resetDraining` compte une action (`:445`)"
        );
        assert_eq!(memory.enchanter.progress_ticks, 0);

        // Sans livre, le drain se termine quand meme et le nomme (`:408-430`).
        let empty_hand = EnchanterFacts {
            book_in_hand: false,
            ..ready
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.drain_target = Some(Pos::new(4, 64, 4));
        memory.enchanter.drain_citizen = Some(3);
        memory.enchanter.progress_ticks = MAX_PROGRESS_TICKS;
        let decision = gather_and_drain(&facts, &empty_hand, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoBookToSpend));
        assert!(decision.effects.contains(&Effect::Drained {
            xp: 0.0,
            mana_gain: 0,
            book_spent: false
        }));
    }

    #[test]
    fn enchanting_runs_three_hundred_ticks_at_level_one_and_resets_at_the_end() {
        // `:235` : `progressTicks++ < MAX_ENCHANTMENT_TICKS / niveau`.
        let facts = WorldFacts::new();
        let trade = EnchanterFacts {
            building_level: 1,
            enchant_recipe_available: true,
            enchant_mana_cost: 2,
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        for _ in 0..MAX_ENCHANTMENT_TICKS {
            assert_eq!(
                enchant(&facts, &trade, &mut memory).refusal,
                Some(ServiceRefusal::Enchanting)
            );
        }
        assert_eq!(memory.enchanter.progress_ticks, MAX_ENCHANTMENT_TICKS);
        let done = enchant(&facts, &trade, &mut memory);
        assert_eq!(done.next, Some(ServiceState::Base(WorkerState::Idle)));
        assert!(done.effects.contains(&Effect::Enchanted { mana_spent: 2 }));
        assert_eq!(memory.enchanter.progress_ticks, 0, "`:286` remet a zero");
        // A niveau 4, le meme enchantement coute 75 ticks.
        let quick = EnchanterFacts {
            building_level: 4,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        for _ in 0..(MAX_ENCHANTMENT_TICKS / 4) {
            assert_eq!(
                enchant(&facts, &quick, &mut memory).refusal,
                Some(ServiceRefusal::Enchanting)
            );
        }
        assert_eq!(
            enchant(&facts, &quick, &mut memory).next,
            Some(ServiceState::Base(WorkerState::Idle))
        );
    }

    #[test]
    fn enchanting_without_a_recipe_returns_to_work_and_forgets_the_progress() {
        // `:228-233`.
        let facts = WorldFacts::new();
        let trade = EnchanterFacts {
            enchant_recipe_available: false,
            ..EnchanterFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.enchanter.progress_ticks = 17;
        let decision = enchant(&facts, &trade, &mut memory);
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::StartWorking))
        );
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoEnchantingRecipe));
        assert_eq!(memory.enchanter.progress_ticks, 0);
        assert_eq!(
            ServiceMemory::default().enchanter,
            EnchanterMemory::default()
        );
        assert_eq!(ACTIONS_UNTIL_DUMPING, 1);
    }

    #[test]
    fn the_registered_table_is_two_trade_targets_and_the_shared_decide() {
        // `:109`, `:110`, `AbstractEntityAICrafting.java:166`.
        let ai = ServiceAi::new(crate::ai_work_12::ServiceTrade::Enchanter);
        let machine = ai.machine();
        assert_eq!(machine.transitions().len(), 5 + 3, "cinq de relais plus trois");
        let rates: Vec<(&str, u32)> = machine
            .transitions()
            .iter()
            .filter(|transition| {
                matches!(transition.rule, "decide" | "gatherAndDrain" | "enchant")
            })
            .map(|transition| (transition.rule, transition.tick_rate))
            .collect();
        assert_eq!(
            rates,
            vec![
                ("decide", STANDARD_DELAY),
                ("gatherAndDrain", DRAIN_RATE),
                ("enchant", ENCHANT_RATE),
            ]
        );
        assert!(machine.can_leave(ServiceState::Enchant));
        assert!(machine.can_leave(ServiceState::EnchanterDrain));
        // La scene d'un enchanteur ne porte pas les faits d'un autre metier.
        let scene = ServiceScene::new(WorldFacts::new(), ServiceJob::Enchanter(trade()));
        assert!(scene.enchanter().is_some());
        assert!(scene.healer().is_none());
    }
}
