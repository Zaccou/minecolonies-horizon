//! Le croque-mort : vider une tombe, creuser, enterrer, et parfois ressusciter.
//!
//! Référence Java : `core/entity/ai/workers/service/EntityAIWorkUndertaker.java` (553
//! lignes) — le constructeur (`:86-99`), `startWorking` (`:114-145`), `wander`
//! (`:153-168`), `emptyGrave` (`:176-226`), `digGrave` (`:233-277`), `digIfAble`
//! (`:286-301`), `tryResurrect` (`:309-379`), `getResurrectChance` (`:387-403`),
//! `getTotemResurrectChance` (`:410-432`), `buryCitizen` (`:440-499`) et
//! `wantInventoryDumped` (`:507-515`) — sur
//! `api/util/constant/UndertakerConstants.java` (les efforts et les chances) et
//! `colony/buildings/modules/GraveyardManagementModule.java` (`:255-332`).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | → `START_WORKING` | 60 | `:90` |
//! | 2 | `START_WORKING` | `startWorking` | 5 | `:91` |
//! | 3 | `WANDER` | `wander` | 5 | `:92` |
//! | 4 | `EMPTY_GRAVE` | `emptyGrave` | 5 | `:93` |
//! | 5 | `TRY_RESURRECT` | `tryResurrect` | 5 | `:94` |
//! | 6 | `DIG_GRAVE` | `digGrave` | 5 | `:95` |
//! | 7 | `BURY_CITIZEN` | `buryCitizen` | 5 | `:96` |
//!
//! La ligne 1 est celle du passage de relais ; sa cadence est `REQUEST_DELAY` = 60
//! (`AbstractEntityAIBasic.java:105`) et non 1 comme chez le guérisseur : un croque-mort
//! sans tombe attend trois secondes entre deux tours.
//!
//! ## Les quatre gestes et l'effort qui les mesure
//!
//! Chaque geste coûte des **points d'effort** que la compétence primaire ou la mana
//! fournit à chaque tick, et `effortCounter` remis à zéro à la fin :
//!
//! | Geste | Effort | Points par tick | Java |
//! | --- | ---: | --- | --- |
//! | vider une tombe | 100 | `getPrimarySkillLevel()` | `:210-214`, `UndertakerConstants.java:61` |
//! | ressusciter | 400 | `getSecondarySkillLevel()` (mana) | `:337-343`, `:71` |
//! | enterrer | 400 | `getPrimarySkillLevel()` | `:475-479`, `:66` |
//!
//! Un croque-mort de compétence 20 met donc cinq ticks à vider une tombe et vingt à en
//! poser une ; à compétence 0, le compteur n'avance **jamais** et le métier boucle. Java
//! n'a pas de garde : le port non plus, mais `RAPPORT.md` §4 le dit.
//!
//! ## La résurrection est un calcul, pas un tirage
//!
//! `getResurrectChance` (`:387-403`) additionne quatre termes —
//! `niveau * 0.005 + mana * 0.00125 + recherche + bonus de totem` — et plafonne à
//! `0.025 + niveau_max_du_site_mystique * 0.005 + bonus de totem`. Le tirage
//! `random.nextDouble()` (`:357`) et celui du bris de totem (`:351`) sont des **faits**
//! ([`UndertakerFacts::resurrect_roll`], `:totem_roll`) : le port garde le calcul
//! ([`resurrect_chance`]) et le bonus ([`totem_bonus`]), pas le hasard.
//!
//! ## Ce que ce fichier ne fait pas
//!
//! Il ne casse pas de bloc (`mineBlock`, `:292`), ne transfère rien (`:219`), ne pose
//! pas de tombe (`buryCitizenHere`, `:485`) et ne ressuscite personne (`:363-373`) :
//! chacun de ces gestes est un [`Effect`] que l'appelant applique.

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, WorkerState};
use crate::ai::worker::STANDARD_DELAY;

use super::facts::{ServiceMemory, UndertakerFacts};
use super::state::{ServiceRefusal, ServiceState};
use super::{add_target, at_hut, service_action, Decision, Effect};

/// `UndertakerConstants.EFFORT_EMPTY_GRAVE` (`:61`, lu à `:210`) : cent points pour
/// vider une tombe. Déjà porté par `crate::jobs::kinds::service`, réutilisé ici.
pub use crate::jobs::kinds::service::EFFORT_EMPTY_GRAVE;
/// `UndertakerConstants.EFFORT_BURY` (`:66`, lu à `:475`) : quatre cents points.
pub use crate::jobs::kinds::service::EFFORT_BURY;
/// `UndertakerConstants.EFFORT_RESURRECT` (`:71`, lu à `:337`) : quatre cents points.
pub use crate::jobs::kinds::service::EFFORT_RESURRECT;
/// `UndertakerConstants.XP_PER_DIG` (`:16`, accordé à `:270`) : 7,5 points de
/// **Force**, pas d'expérience de métier.
pub use crate::jobs::kinds::service::XP_PER_DIG;
/// `UndertakerConstants.RESURRECT_BUILDING_LVL_WEIGHT` (`:26`, lu à `:390`).
pub use crate::jobs::kinds::service::RESURRECT_BUILDING_LVL_WEIGHT;
/// `UndertakerConstants.RESURRECT_WORKER_MANA_LVL_WEIGHT` (`:31`, lu à `:391`).
pub use crate::jobs::kinds::service::RESURRECT_WORKER_MANA_LVL_WEIGHT;
/// `UndertakerConstants.MAX_RESURRECTION_CHANCE` (`:36`, lu à `:395-397`).
pub use crate::jobs::kinds::service::MAX_RESURRECTION_CHANCE;
/// `UndertakerConstants.MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS` (`:41`, lu à `:396`).
pub use crate::jobs::kinds::service::MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS;

/// `UndertakerConstants.SINGLE_TOTEM_RESURRECTION_CHANCE_BONUS` (`:46`, lu à `:423`).
pub const SINGLE_TOTEM_RESURRECTION_CHANCE_BONUS: f64 = 0.05;
/// `UndertakerConstants.MULTIPLE_TOTEMS_RESURRECTION_CHANCE_BONUS` (`:51`, lu à `:427`).
pub const MULTIPLE_TOTEMS_RESURRECTION_CHANCE_BONUS: f64 = 0.075;
/// `UndertakerConstants.TOTEM_BREAK_CHANCE` (`:56`, lu à `:351`) : un totem sur cent
/// casse à chaque tentative.
pub const TOTEM_BREAK_CHANCE: f64 = 0.01;
/// `walkWithProxy(gravePos, 3)` (`:192`, `:253`, `:329`) : trois blocs de la tombe.
pub const WALK_RANGE_TO_GRAVE: i32 = 3;
/// `walkWithProxy(burialPos.getA(), 4)` (`:470`) : quatre blocs de la place de tombe.
pub const BURIAL_WALK_RANGE: i32 = 4;
/// `walkToRandomPosWithin(worker, 10, ...)` (`:159`) : dix blocs de flânerie dans le
/// cimetière.
pub const WANDER_RANGE: u32 = 10;
/// La cadence des six états de travail (`:91-96`) : `STANDARD_DELAY`
/// (`AbstractEntityAIBasic.java:100`).
pub const WORK_RATE: u32 = STANDARD_DELAY;
/// La cadence de `IDLE → START_WORKING` (`:90`) : `REQUEST_DELAY`
/// (`AbstractEntityAIBasic.java:105`), trois secondes.
pub const IDLE_RATE: u32 = crate::ai::worker::REQUEST_DELAY;

/// `getResurrectChance` (`:387-403`) : les trois termes plus le totem, plafonnés.
pub fn resurrect_chance(trade: &UndertakerFacts) -> f64 {
    let totem = totem_bonus(trade);
    let chance = f64::from(trade.building_level) * RESURRECT_BUILDING_LVL_WEIGHT
        + f64::from(trade.mana_level) * RESURRECT_WORKER_MANA_LVL_WEIGHT
        + trade.resurrect_research
        + totem; // `:390-393`
    let cap = MAX_RESURRECTION_CHANCE
        + f64::from(trade.mystical_max_level) * MAX_RESURRECTION_CHANCE_MYSTICAL_LVL_BONUS
        + totem; // `:395-397`
    if chance > cap {
        cap // `:398-401`
    } else {
        chance
    }
}

/// `getTotemResurrectChance` (`:410-432`) : la recherche `USE_TOTEM` ouvre le bonus,
/// zéro, un ou plusieurs totems le fixent.
pub fn totem_bonus(trade: &UndertakerFacts) -> f64 {
    if !trade.use_totem_research {
        return 0.0; // `:431`
    }
    if trade.totems == 1 {
        SINGLE_TOTEM_RESURRECTION_CHANCE_BONUS // `:421-424`
    } else if trade.totems > 1 {
        MULTIPLE_TOTEMS_RESURRECTION_CHANCE_BONUS // `:425-428`
    } else {
        0.0
    }
}

/// `wantInventoryDumped()` (`:507-515`) : le drapeau posé après une inhumation, consommé
/// une fois.
pub fn want_inventory_dumped(memory: &mut ServiceMemory) -> bool {
    if memory.undertaker.should_dump_inventory {
        memory.undertaker.should_dump_inventory = false; // `:511`
        return true;
    }
    false // `:514`
}

/// `startWorking()` (`:114-145`) : une inhumation en attente d'abord, une tombe ensuite.
pub fn start_working(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = memory;
    if trade.last_grave_data {
        // `:121-125` — l'inhumation interrompue par la nuit reprend en premier.
        return Decision::go(ServiceState::BuryCitizen);
    }
    let mut decision = Decision::stay();
    if trade.grave_to_work_on.is_some() {
        if !at_hut(facts) {
            return decision.because(ServiceRefusal::NotAtHut); // `:130-133`
        }
        if trade.grave_is_grave {
            return decision.going(ServiceState::EmptyGrave); // `:136-140`
        }
        // `:141` — la tombe n'est plus une tombe : elle est oubliée.
        decision = decision.effect(Effect::ClearCurrentGrave);
    }
    decision.going(ServiceState::Wander) // `:143-144`
}

/// `wander()` (`:153-168`) : flâner dans le cimetière, ou y retourner.
pub fn wander(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = memory;
    if facts.worker.navigation_done {
        // `:155` — une marche ne se décide que quand la précédente est finie.
        if trade.in_building {
            return Decision::go(ServiceState::Base(WorkerState::Idle))
                .effect(Effect::Wander {
                    range: WANDER_RANGE,
                }); // `:157-161`
        }
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .effect(Effect::WalkToHut); // `:163`
    }
    Decision::go(ServiceState::Base(WorkerState::Idle)) // `:167`
}

/// `emptyGrave()` (`:176-226`) : sortir le contenu d'une tombe.
pub fn empty_grave(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    if trade.shovel_missing {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::ShovelMissing); // `:180-183`
    }
    let Some(grave) = trade.grave_to_work_on else {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NoGraveToWorkOn);
    };
    if !trade.at_grave {
        return Decision::stay().because(ServiceRefusal::NotAtGrave); // `:192-195`
    }
    if trade.grave_empty {
        // `:200-203` — une tombe déjà vide attend sa résurrection.
        return Decision::go(ServiceState::TryResurrect);
    }
    if facts.inventory.full {
        // `:205-208` — les poches sont pleines avant de vider la tombe.
        return Decision::go(ServiceState::Base(WorkerState::InventoryFull))
            .because(ServiceRefusal::WorkerInventoryFull);
    }
    if memory.undertaker.effort_counter < EFFORT_EMPTY_GRAVE {
        // `:210-215` — un coup de pelle de plus, `effortCounter += compétence`.
        memory.undertaker.effort_counter += trade.primary_skill;
        return Decision::stay().because(ServiceRefusal::EffortPending);
    }
    memory.undertaker.effort_counter = 0; // `:216`
    if trade.grave_emptied {
        // `:219-222` — le transfert a réussi, la résurrection suit.
        return Decision::go(ServiceState::TryResurrect).effect(Effect::EmptiedGrave { at: grave });
    }
    Decision::go(ServiceState::Base(WorkerState::Idle)).because(ServiceRefusal::NotAGrave) // `:225`
}

/// `digGrave()` (`:233-277`) avec `digIfAble` (`:286-301`) : casser la tombe.
pub fn dig_grave(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = facts;
    if trade.shovel_missing {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::ShovelMissing); // `:237-240` et `:288`
    }
    let Some(grave) = trade.grave_to_work_on else {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NoGraveToWorkOn);
    };
    if !trade.at_grave {
        return Decision::stay().because(ServiceRefusal::NotAtGrave); // `:253-256`
    }
    if !trade.grave_is_grave {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NotAGrave); // `:260-276`
    }
    if !trade.dig_done {
        // `digIfAble` a rendu faux : `return getState()` (`:264-267`).
        return Decision::stay().because(ServiceRefusal::EffortPending);
    }
    memory.undertaker.effort_counter = 0;
    Decision::go(ServiceState::BuryCitizen) // `:273`
        .effect(Effect::Dug { at: grave, xp: XP_PER_DIG }) // `:269-271`
        .effect(Effect::ClearCurrentGrave) // `:295`
        .effect(Effect::KeepGraveData) // `:296`
}

/// `tryResurrect()` (`:309-379`) : quatre cents points de mana, puis un tirage.
pub fn try_resurrect(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = facts;
    if trade.shovel_missing {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::ShovelMissing); // `:313-317`
    }
    let Some(grave) = trade.grave_to_work_on else {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NoGraveToWorkOn);
    };
    if !trade.at_grave {
        return Decision::stay().because(ServiceRefusal::NotAtGrave); // `:329-332`
    }
    let mut decision = Decision::stay();
    if memory.undertaker.effort_counter < EFFORT_RESURRECT {
        memory.undertaker.effort_counter += trade.mana_level; // `:343`
        return decision.because(ServiceRefusal::EffortPending); // `:337-345`
    }
    memory.undertaker.effort_counter = 0; // `:346`
    memory.undertaker.should_dump_inventory = true; // `:348`
    let chance = resurrect_chance(trade); // `:349`
    if totem_bonus(trade) > 0.0 && trade.totem_breaks() {
        // `:351-355` — le totem se consume, et il aide quand même au tirage.
        decision = decision.effect(Effect::TotemBurned);
    }
    if chance >= trade.resurrect_roll {
        // `:357-374` — le citoyen revient, la tombe disparaît, `INVENTORY_FULL` vide les
        // poches.
        return decision
            .going(ServiceState::Base(WorkerState::InventoryFull))
            .effect(Effect::Resurrected { at: grave })
            .effect(Effect::ForgetGraveData);
    }
    decision
        .because(ServiceRefusal::ResurrectFailed)
        .going(ServiceState::DigGrave) // `:378`
}

/// `buryCitizen()` (`:440-499`) : poser une tombe nommée à la place libre du cimetière.
pub fn bury_citizen(facts: &WorldFacts, trade: &UndertakerFacts, memory: &mut ServiceMemory) -> Decision {
    let _ = facts;
    if trade.shovel_missing {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::ShovelMissing); // `:445-448`
    }
    if !trade.last_grave_data {
        return Decision::go(ServiceState::Base(WorkerState::Idle))
            .because(ServiceRefusal::NoDiggedGrave); // `:445-448`
    }
    let Some(spot) = trade.burial_spot(memory.undertaker.burial_pos) else {
        // `:456-468` — le cimetière est plein : message au joueur, donnée oubliée.
        memory.undertaker.burial_pos = None;
        return Decision::go(ServiceState::Base(WorkerState::InventoryFull))
            .because(ServiceRefusal::NoBurialSpot)
            .effect(Effect::ForgetGraveData);
    };
    memory.undertaker.burial_pos = Some(spot); // `:451-454`
    if !trade.at_burial {
        return Decision::stay().because(ServiceRefusal::NotAtBurial); // `:470-473`
    }
    if memory.undertaker.effort_counter < EFFORT_BURY {
        // `:475-481` — un coup de pelle de plus, `effortCounter += compétence`.
        memory.undertaker.effort_counter += trade.primary_skill;
        return Decision::stay().because(ServiceRefusal::EffortPending);
    }
    memory.undertaker.effort_counter = 0; // `:482`
    if !trade.bury_succeeds {
        memory.undertaker.burial_pos = None;
        return Decision::stay().because(ServiceRefusal::BurialFailed); // `:485-489`
    }
    memory.undertaker.burial_pos = None;
    memory.undertaker.should_dump_inventory = true; // `:496`
    Decision::go(ServiceState::Base(WorkerState::InventoryFull)) // `:498`
        .effect(Effect::Buried { at: spot }) // `:485`
        .effect(Effect::ForgetGraveData) // `:494`
        .effect(Effect::DumpInventory) // `:496`
}

service_action!(start_working_action, Undertaker, start_working);
service_action!(wander_action, Undertaker, wander);
service_action!(empty_grave_action, Undertaker, empty_grave);
service_action!(dig_grave_action, Undertaker, dig_grave);
service_action!(try_resurrect_action, Undertaker, try_resurrect);
service_action!(bury_citizen_action, Undertaker, bury_citizen);

/// Enregistre les six cibles de `EntityAIWorkUndertaker.java:91-96`.
///
/// La première (`IDLE`, `:90`) est celle du passage de relais, à `REQUEST_DELAY`.
pub fn register(machine: &mut Machine<ServiceState, super::ServiceScene>) {
    // `:91` — `new AITarget(START_WORKING, this::startWorking, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::Base(WorkerState::StartWorking),
        "startWorking",
        "EntityAIWorkUndertaker.java:91",
        WORK_RATE,
        start_working_action,
    );
    // `:92` — `new AITarget(WANDER, this::wander, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::Wander,
        "wander",
        "EntityAIWorkUndertaker.java:92",
        WORK_RATE,
        wander_action,
    );
    // `:93` — `new AITarget(EMPTY_GRAVE, this::emptyGrave, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::EmptyGrave,
        "emptyGrave",
        "EntityAIWorkUndertaker.java:93",
        WORK_RATE,
        empty_grave_action,
    );
    // `:94` — `new AITarget(TRY_RESURRECT, this::tryResurrect, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::TryResurrect,
        "tryResurrect",
        "EntityAIWorkUndertaker.java:94",
        WORK_RATE,
        try_resurrect_action,
    );
    // `:95` — `new AITarget(DIG_GRAVE, this::digGrave, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::DigGrave,
        "digGrave",
        "EntityAIWorkUndertaker.java:95",
        WORK_RATE,
        dig_grave_action,
    );
    // `:96` — `new AITarget(BURY_CITIZEN, this::buryCitizen, STANDARD_DELAY)`.
    add_target(
        machine,
        ServiceState::BuryCitizen,
        "buryCitizen",
        "EntityAIWorkUndertaker.java:96",
        WORK_RATE,
        bury_citizen_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_work_12::facts::{ServiceJob, ServiceScene};
    use crate::ai_work_12::ServiceAi;

    fn trade() -> UndertakerFacts {
        UndertakerFacts {
            at_grave: true,
            at_burial: true,
            grave_is_grave: true,
            grave_to_work_on: Some(Pos::new(3, 64, 3)),
            last_grave_data: false,
            primary_skill: 25,
            mana_level: 20,
            building_level: 1,
            ..UndertakerFacts::default()
        }
    }

    #[test]
    fn a_pending_burial_is_resumed_before_any_grave() {
        // `:121-125` — le module a encore la donnée de la tombe creusée.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            last_grave_data: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = start_working(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::BuryCitizen));
        assert_eq!(decision.refusal, None);
    }

    #[test]
    fn without_a_grave_the_undertaker_wanders() {
        // `:143-144` — aucun emplacement courant : `setJobStatus(IDLE); return WANDER`.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            grave_to_work_on: None,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = start_working(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Wander));
    }

    #[test]
    fn a_grave_that_is_not_a_grave_is_cleared_and_the_wander_follows() {
        // `:135-142` — le bloc n'est pas un `TileEntityGrave`, `ClearCurrentGrave`, puis
        // `WANDER` par la fin de la méthode.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            grave_is_grave: false,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = start_working(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Wander));
        assert!(decision.effects.contains(&Effect::ClearCurrentGrave));
    }

    #[test]
    fn a_grave_with_a_grave_block_sends_the_undertaker_to_empty_it() {
        // `:136-140`.
        let facts = WorldFacts::new();
        let mut memory = ServiceMemory::default();
        let decision = start_working(&facts, &trade(), &mut memory);
        assert_eq!(decision.next, Some(ServiceState::EmptyGrave));
    }

    #[test]
    fn a_missing_shovel_stops_the_four_work_states() {
        // `:180`, `:237`, `:313`, `:445` : `checkForToolOrWeapon(shovel)` est appelé par
        // chacun des quatre états, et rend vrai quand la pelle manque.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            shovel_missing: true,
            ..trade()
        };
        let actions: [fn(&WorldFacts, &UndertakerFacts, &mut ServiceMemory) -> Decision; 4] =
            [empty_grave, dig_grave, try_resurrect, bury_citizen];
        for action in actions {
            let mut memory = ServiceMemory::default();
            let decision = action(&facts, &trade, &mut memory);
            assert_eq!(decision.refusal, Some(ServiceRefusal::ShovelMissing));
            assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));
        }
        // Un état sans tombe s'arrête aussi, sur un autre refus (`:180-183`).
        let no_grave = UndertakerFacts {
            shovel_missing: false,
            grave_to_work_on: None,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        assert_eq!(
            empty_grave(&facts, &no_grave, &mut memory).refusal,
            Some(ServiceRefusal::NoGraveToWorkOn)
        );
    }

    #[test]
    fn an_empty_grave_goes_straight_to_the_resurrection() {
        // `:200-203`.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            grave_empty: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = empty_grave(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::TryResurrect));
    }

    #[test]
    fn emptying_a_grave_takes_a_hundred_effort_points_of_skill() {
        // `:210-222` et `UndertakerConstants.java:61` : 100 points, 25 par tick à
        // compétence 25, donc quatre coups de pelle puis le transfert.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            grave_emptied: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        for _ in 0..4 {
            let pending = empty_grave(&facts, &trade, &mut memory);
            assert_eq!(pending.refusal, Some(ServiceRefusal::EffortPending));
            assert_eq!(pending.next, None);
        }
        assert_eq!(memory.undertaker.effort_counter, EFFORT_EMPTY_GRAVE);
        let emptied = empty_grave(&facts, &trade, &mut memory);
        assert_eq!(emptied.next, Some(ServiceState::TryResurrect));
        assert_eq!(emptied.effects, vec![Effect::EmptiedGrave {
            at: Pos::new(3, 64, 3)
        }]);
        assert_eq!(memory.undertaker.effort_counter, 0, "`:216` remet à zéro");
        // À compétence 0, le compteur n'avance jamais : le métier boucle.
        let untalented = UndertakerFacts {
            primary_skill: 0,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        for _ in 0..10 {
            assert_eq!(
                empty_grave(&facts, &untalented, &mut memory).refusal,
                Some(ServiceRefusal::EffortPending)
            );
        }
        assert_eq!(memory.undertaker.effort_counter, 0);
    }

    #[test]
    fn a_full_inventory_sends_the_undertaker_to_dump_before_emptying() {
        // `:205-208`.
        let mut facts = WorldFacts::new();
        facts.inventory.full = true;
        let mut memory = ServiceMemory::default();
        let decision = empty_grave(&facts, &trade(), &mut memory);
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::InventoryFull))
        );
        assert_eq!(decision.refusal, Some(ServiceRefusal::WorkerInventoryFull));
    }

    #[test]
    fn a_grave_too_far_holds_the_state_within_three_blocks() {
        // `:192-195` : `walkWithProxy(gravePos, 3)` vrai = encore en marche.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            at_grave: false,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = empty_grave(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NotAtGrave));
        assert_eq!(decision.next, None);
        assert_eq!(WALK_RANGE_TO_GRAVE, 3);
        assert_eq!(BURIAL_WALK_RANGE, 4);
    }

    #[test]
    fn digging_a_grave_keeps_its_data_and_carries_to_the_burial() {
        // `:264-273` et `digIfAble` (`:286-301`).
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            dig_done: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = dig_grave(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::BuryCitizen));
        assert_eq!(
            decision.effects,
            vec![
                Effect::Dug {
                    at: Pos::new(3, 64, 3),
                    xp: XP_PER_DIG
                },
                Effect::ClearCurrentGrave,
                Effect::KeepGraveData,
            ]
        );
        // `mineBlock` faux : `return getState()` (`:264-267`).
        let not_yet = UndertakerFacts {
            dig_done: false,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        assert_eq!(
            dig_grave(&facts, &not_yet, &mut memory).refusal,
            Some(ServiceRefusal::EffortPending)
        );
    }

    #[test]
    fn the_resurrection_effort_is_four_hundred_points_of_mana() {
        // `:337-346` : 400 points, 20 par tick à mana 20, donc vingt gestes puis le
        // tirage.
        let facts = WorldFacts::new();
        let mut trade = trade();
        trade.resurrect_roll = 1.0; // ne gagne jamais tant que la chance est < 1
        let mut memory = ServiceMemory::default();
        for _ in 0..20 {
            assert_eq!(
                try_resurrect(&facts, &trade, &mut memory).refusal,
                Some(ServiceRefusal::EffortPending)
            );
        }
        assert_eq!(memory.undertaker.effort_counter, EFFORT_RESURRECT);
        let tried = try_resurrect(&facts, &trade, &mut memory);
        assert_eq!(tried.refusal, Some(ServiceRefusal::ResurrectFailed));
        assert_eq!(tried.next, Some(ServiceState::DigGrave), "`:378`");
        assert_eq!(memory.undertaker.effort_counter, 0);
        assert!(
            memory.undertaker.should_dump_inventory,
            "`:348` est posé avant le tirage, même raté"
        );
    }

    #[test]
    fn a_won_roll_resurrects_and_dumps_and_a_lost_one_digs() {
        // `:357-374` contre `:378`.
        let facts = WorldFacts::new();
        let mut trade = trade();
        trade.resurrect_roll = 0.0; // toute chance > 0 gagne
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_RESURRECT;
        let won = try_resurrect(&facts, &trade, &mut memory);
        assert_eq!(
            won.next,
            Some(ServiceState::Base(WorkerState::InventoryFull))
        );
        assert!(won.effects.contains(&Effect::Resurrected {
            at: Pos::new(3, 64, 3)
        }));
        assert!(won.effects.contains(&Effect::ForgetGraveData));
        assert!(memory.undertaker.should_dump_inventory);
        // Une chance de 0,021 perd contre un tirage de 0,5.
        trade.resurrect_roll = 0.5;
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_RESURRECT;
        let lost = try_resurrect(&facts, &trade, &mut memory);
        assert_eq!(lost.refusal, Some(ServiceRefusal::ResurrectFailed));
        assert_eq!(lost.next, Some(ServiceState::DigGrave));
    }

    #[test]
    fn a_totem_that_breaks_is_burned_and_still_helps_the_roll() {
        // `:351-355` : le bris est un `nextDouble() <= 0.01`, le bonus reste acquis.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            use_totem_research: true,
            totems: 1,
            totem_roll: 0.0,
            resurrect_roll: 0.0,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_RESURRECT;
        let decision = try_resurrect(&facts, &trade, &mut memory);
        assert!(decision.effects.contains(&Effect::TotemBurned));
        assert!(totem_bonus(&trade) > 0.0);
        // Un tirage de bris au-dessus de 0,01 ne consume rien.
        let safe = UndertakerFacts {
            totem_roll: 0.02,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_RESURRECT;
        let decision = try_resurrect(&facts, &safe, &mut memory);
        assert!(!decision.effects.contains(&Effect::TotemBurned));
    }

    #[test]
    fn four_hundred_effort_points_bury_a_citizen() {
        // `:475-489` : 400 points, 25 par tick à compétence 25, donc seize coups de
        // pelle puis la tombe.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            last_grave_data: true,
            burial_search: Some(Pos::new(9, 64, 9)),
            burial_replaceable: true,
            bury_succeeds: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        for _ in 0..16 {
            assert_eq!(
                bury_citizen(&facts, &trade, &mut memory).refusal,
                Some(ServiceRefusal::EffortPending)
            );
        }
        assert_eq!(memory.undertaker.effort_counter, EFFORT_BURY);
        let buried = bury_citizen(&facts, &trade, &mut memory);
        assert_eq!(
            buried.next,
            Some(ServiceState::Base(WorkerState::InventoryFull))
        );
        assert_eq!(
            buried.effects,
            vec![
                Effect::Buried {
                    at: Pos::new(9, 64, 9)
                },
                Effect::ForgetGraveData,
                Effect::DumpInventory,
            ]
        );
        assert_eq!(memory.undertaker.burial_pos, None);
        assert!(memory.undertaker.should_dump_inventory);
        // La même dépense avec une seule pelletée d'avance ne suffit pas.
        assert_eq!(EFFORT_BURY, 400);
        assert_eq!(EFFORT_EMPTY_GRAVE, 100);
        assert_eq!(EFFORT_RESURRECT, 400);
    }

    #[test]
    fn without_a_free_spot_the_grave_data_is_forgotten() {
        // `:456-468` — le cimetière est plein : message, donnée oubliée, `INVENTORY_FULL`.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            last_grave_data: true,
            burial_search: None,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = bury_citizen(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoBurialSpot));
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::InventoryFull))
        );
        assert!(decision.effects.contains(&Effect::ForgetGraveData));
        // Sans donnée de tombe du tout, `:445-448` arrête avant.
        let nothing = UndertakerFacts {
            last_grave_data: false,
            ..trade
        };
        let mut memory = ServiceMemory::default();
        assert_eq!(
            bury_citizen(&facts, &nothing, &mut memory).refusal,
            Some(ServiceRefusal::NoDiggedGrave)
        );
    }

    #[test]
    fn a_failed_burial_forgets_the_spot_and_holds_the_state() {
        // `:485-489`.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            last_grave_data: true,
            burial_search: Some(Pos::new(9, 64, 9)),
            burial_replaceable: true,
            bury_succeeds: false,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_BURY;
        let decision = bury_citizen(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::BurialFailed));
        assert_eq!(decision.next, None);
        assert_eq!(memory.undertaker.burial_pos, None);
        assert!(!memory.undertaker.should_dump_inventory);
    }

    #[test]
    fn the_burial_walk_stops_before_the_effort_within_four_blocks() {
        // `:470-473` : la marche passe avant la pelletée.
        let facts = WorldFacts::new();
        let trade = UndertakerFacts {
            last_grave_data: true,
            burial_search: Some(Pos::new(9, 64, 9)),
            at_burial: false,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        memory.undertaker.effort_counter = EFFORT_BURY;
        let decision = bury_citizen(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NotAtBurial));
        assert_eq!(memory.undertaker.effort_counter, EFFORT_BURY, "rien n'est dépensé");
    }

    #[test]
    fn the_dump_flag_is_consumed_once() {
        // `:507-515`.
        let mut memory = ServiceMemory::default();
        assert!(!want_inventory_dumped(&mut memory));
        memory.undertaker.should_dump_inventory = true;
        assert!(want_inventory_dumped(&mut memory));
        assert!(!want_inventory_dumped(&mut memory), "consommé");
    }

    #[test]
    fn wandering_in_the_graveyard_asks_for_a_short_walk_and_outside_for_the_hut() {
        // `:155-167`.
        let facts = WorldFacts::new();
        let inside = UndertakerFacts {
            in_building: true,
            ..trade()
        };
        let mut memory = ServiceMemory::default();
        let decision = wander(&facts, &inside, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Idle)));
        assert_eq!(
            decision.effects,
            vec![Effect::Wander { range: WANDER_RANGE }]
        );
        let outside = UndertakerFacts {
            in_building: false,
            ..inside
        };
        let decision = wander(&facts, &outside, &mut memory);
        assert_eq!(decision.effects, vec![Effect::WalkToHut]);
        // Une marche en cours ne se remplace pas (`:155`).
        let mut walking = WorldFacts::new();
        walking.worker.navigation_done = false;
        let decision = wander(&walking, &inside, &mut memory);
        assert!(decision.effects.is_empty());
    }

    #[test]
    fn the_registered_table_is_six_trade_targets_at_five_ticks() {
        // `EntityAIWorkUndertaker.java:91-96`.
        let ai = ServiceAi::new(crate::ai_work_12::ServiceTrade::Undertaker);
        let machine = ai.machine();
        assert_eq!(machine.transitions().len(), 5 + 6);
        let rates: Vec<(&str, u32)> = machine
            .transitions()
            .iter()
            .filter(|transition| {
                matches!(
                    transition.rule,
                    "startWorking"
                        | "wander"
                        | "emptyGrave"
                        | "tryResurrect"
                        | "digGrave"
                        | "buryCitizen"
                )
            })
            .map(|transition| (transition.rule, transition.tick_rate))
            .collect();
        assert_eq!(
            rates,
            vec![
                ("startWorking", WORK_RATE),
                ("wander", WORK_RATE),
                ("emptyGrave", WORK_RATE),
                ("tryResurrect", WORK_RATE),
                ("digGrave", WORK_RATE),
                ("buryCitizen", WORK_RATE),
            ]
        );
        assert_eq!(WORK_RATE, 5);
        assert_eq!(IDLE_RATE, 60, "AbstractEntityAIBasic.java:105");
        assert!(machine.can_leave(ServiceState::BuryCitizen));
        assert!(machine.can_leave(ServiceState::TryResurrect));
        // La scène du croque-mort porte bien ses faits et pas ceux d'un autre.
        let scene = ServiceScene::new(WorldFacts::new(), ServiceJob::Undertaker(trade()));
        assert!(scene.undertaker().is_some());
        assert!(scene.enchanter().is_none());
    }
}
