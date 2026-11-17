//! Le guérisseur : un patient à la fois, un remède à la fois, et trois façons de soigner.
//!
//! Référence Java : `core/entity/ai/workers/service/EntityAIWorkHealer.java` (552
//! lignes) — le constructeur (`:82-97`), `decide` (`:104-234`), `requestCure`
//! (`:241-304`), `cure` (`:311-383`), `freeCure` (`:390-438`), `curePlayer` (`:445-467`),
//! `getStateAfterPickUp` (`:470-473`), `wander` (`:480-506`), `testRandomCureChance`
//! (`:513-516`), `hasCureInInventory` (`:525-535`) — sur
//! `core/entity/ai/workers/util/Patient.java` (les trois états d'un dossier).
//!
//! ## La table que ce fichier ajoute
//!
//! | # | État | Règle | Cadence | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | → `START_WORKING` | 1 | `:86` |
//! | 2 | `START_WORKING` | → `DECIDE` | 1 | `:87` |
//! | 3 | `DECIDE` | `decide` | 20 | `:88` |
//! | 4 | `CURE` | `cure` | 20 | `:89` |
//! | 5 | `FREE_CURE` | `freeCure` | 20 | `:90` |
//! | 6 | `CURE_PLAYER` | `curePlayer` | 20 | `:91` |
//! | 7 | `REQUEST_CURE` | `requestCure` | 20 | `:92` |
//! | 8 | `WANDER` | `wander` | 20 | `:93` |
//!
//! La ligne 1 est celle du passage de relais (`IDLE`), la 2 est un `goto` constant : le
//! guérisseur ne décide pas dans `START_WORKING`, il y passe. C'est une différence avec
//! l'enchanteur et le croque-mort, qui décident tous les deux **dans** `START_WORKING`.
//!
//! ## Les trois façons de soigner, dans l'ordre de Java
//!
//! `decide` (`:118-215`) parcourt les dossiers de patients **dans l'ordre du bâtiment** et
//! s'arrête au premier qui demande quelque chose. Trois issues :
//!
//! - dossier `NEW` → [`ServiceState::RequestCure`] (`:129-133`) : il manque les remèdes ;
//! - dossier `REQUESTED` → `CURE` si le remède est là (`:151-156`), `FREE_CURE` si le
//!   tirage de guérison magique passe (`:143-147`), `GATHERING_REQUIRED_MATERIALS` si le
//!   remède est dans les coffres (`:164-169`) ;
//! - dossier `TREATED` → `CURE` seulement pour rendre dix points de vie sans maladie
//!   (`:203-207`).
//!
//! Le tirage de guérison magique est `nextInt(60 * 60) <= max(1, mana / 20)`
//! (`:513-516`) : une chance sur 3 600 par décision à mana 0, une sur 3 600 à mana 19,
//! puis une sur 1 800 à mana 20. Le port prend le tirage en fait
//! ([`HealerFacts::cure_roll`]) et garde le seuil ([`free_cure_roll`]), parce qu'un
//! tirage ne se rejoue pas.
//!
//! ## Ce que ce fichier ne fait pas
//!
//! Il ne soigne personne : `citizen.heal(10)` (`:335`), `cure()` (`:434`),
//! `transferXOfFirstSlotInItemHandlerWithIntoNextFreeSlotInItemHandler` (`:369-373`) et
//! `player.heal(...)` (`:463`) sont des [`Effect`]s que l'appelant applique avec le
//! citoyen en main. Il ne crée pas de dossier non plus : `checkOrCreatePatientFile`
//! (`:115`) devient [`Effect::CreatePatientFile`].

use crate::ai::facts::WorldFacts;
use crate::ai::state::{Machine, Transition, WorkerState};
use crate::jobs::kinds::service::{
    CURE_REQUEST_COUNT, FREE_CURE_PROGRESS_TICKS, HEALER_XP,
};

use super::facts::{count_of, holds, HealerFacts, PatientState};
use super::state::{ServiceRefusal, ServiceState};
use super::{add_target, service_action, Decision, Effect};

/// `EntityAIWorkHealer.BASE_XP_GAIN` (`:43`, payé à `:336`, `:379`, `:433`) — déjà porté
/// comme [`HEALER_XP`] par `crate::jobs::kinds::service`, réutilisé ici plutôt que
/// réécrit.
pub const BASE_XP_GAIN: f64 = HEALER_XP;
/// `EntityAIWorkHealer.REQUEST_COUNT` (`:48`, lu à `:296`) : seize remèdes par requête.
pub const REQUEST_COUNT: u32 = CURE_REQUEST_COUNT as u32;
/// `EntityAIWorkHealer.MAX_PROGRESS_TICKS` (`:63`, lu à `:412`) : trente ticks de
/// guérison magique, à une décision toutes les vingt ticks.
pub const MAX_PROGRESS_TICKS: i32 = FREE_CURE_PROGRESS_TICKS;
/// `EntityAIWorkHealer` : `nextInt(60 * 60)` (`:515`) — la borne du tirage.
pub const CURE_ROLL_BOUND: i32 = 60 * 60;
/// `EntityAIWorkHealer` : `getSecondarySkillLevel() / 20` (`:515`) — les niveaux de mana
/// qui font gagner une chance.
pub const MANA_PER_ROLL: i32 = 20;
/// `EntityAIWorkHealer` : `player.getMaxHealth() - 10 - (2 * building.getBuildingLevel())`
/// (`:220`) — la marge avant qu'un joueur soit soigné.
pub const PLAYER_MIN_WOUND: f64 = 10.0;
/// `EntityAIWorkHealer` : `player.getMaxHealth() - player.getHealth() - 5 - level`
/// (`:463`, `:499`) — les cinq points que le guérisseur laisse toujours manquer.
pub const PLAYER_HEAL_MARGIN: f64 = 5.0;
/// `EntityAIWorkHealer` : `50 * 50` (`:452`) — un joueur plus loin est oublié.
pub const PLAYER_RANGE: i64 = 50;
/// Le carré de [`PLAYER_RANGE`], tel que `distSqr` le compare (`:452`).
pub const PLAYER_RANGE_SQR: i64 = PLAYER_RANGE * PLAYER_RANGE;
/// `EntityAIWorkHealer` : `data.getEntity().get().getHealth() < 10.0` (`:227`) — le seuil
/// du citoyen lointain.
pub const REMOTE_HEALTH: f64 = 10.0;
/// `EntityAIWorkHealer` : `building.getBuildingLevel() * 40` (`:228`) — le rayon du
/// citoyen lointain, 200 blocs à niveau 5.
pub const REMOTE_RANGE_PER_LEVEL: f64 = 40.0;
/// `TranslationConstants.PATIENT_FULL_INVENTORY`, importé par
/// `EntityAIWorkHealer.java:33` et lu à `:197`, `:365`.
pub const PATIENT_FULL_INVENTORY: &str = "com.minecolonies.coremod.healer.fullinv";
/// `EntityAIWorkHealer` : la cadence de `START_WORKING → DECIDE` (`:87`).
pub const START_TO_DECIDE_RATE: u32 = 1;
/// `EntityAIWorkHealer` : la cadence des six états de travail (`:88-93`).
pub const WORK_RATE: u32 = 20;

/// `testRandomCureChance()` (`:513-516`) : `roll <= max(1, mana / 20)`.
///
/// `roll` est `nextInt(60 * 60)`, donc la probabilité est `(seuil + 1) / 3600` : une
/// chance sur 3 600 à mana 0 comme à mana 19, une sur 1 800 dès mana 20.
pub fn free_cure_roll(mana_level: i32, roll: i32) -> bool {
    roll <= (mana_level / MANA_PER_ROLL).max(1)
}

/// `getStateAfterPickUp()` (`:470-473`) : après avoir pris des remèdes, le guérisseur
/// va **soigner**, pas reprendre le travail. C'est le seul des trois métiers à changer
/// cette réponse.
pub fn state_after_pickup() -> ServiceState {
    ServiceState::Cure
}

/// `decide()` (`:104-234`), dans l'ordre de Java.
pub fn decide(facts: &WorldFacts, trade: &HealerFacts, memory: &mut super::facts::ServiceMemory) -> Decision {
    if !trade.at_hospital {
        return Decision::refuse(ServiceRefusal::NotAtHospital); // `:106-109`
    }
    let mut decision = Decision::stay();
    // `:112-116` — chaque citoyen malade dans le bâtiment reçoit un dossier.
    for id in &trade.sick_nearby {
        decision = decision.effect(Effect::CreatePatientFile { citizen: *id });
    }
    for patient in &trade.patients {
        if !patient.present || !patient.sick {
            // `:121-125` — le dossier ne désigne plus un malade : il est retiré.
            decision = decision.effect(Effect::DropPatientFile { citizen: patient.id });
            continue;
        }
        match patient.state {
            PatientState::New => {
                // `:129-133` — il faut demander les remèdes avant tout le reste.
                memory.healer.current_patient = Some(patient.id);
                return decision.going(ServiceState::RequestCure);
            }
            PatientState::Requested => {
                let Some(disease) = patient.disease else {
                    // `:137-141` — plus de maladie : une visite suffit.
                    memory.healer.current_patient = Some(patient.id);
                    return decision.going(ServiceState::Cure);
                };
                if free_cure_roll(trade.mana_level, trade.cure_roll) {
                    // `:143-147` — la guérison magique passe avant les remèdes.
                    memory.healer.current_patient = Some(patient.id);
                    return decision.going(ServiceState::FreeCure);
                }
                if !patient.has_space {
                    // `:195-198` — le guérisseur le dit, et **continue** la boucle.
                    decision = decision
                        .because(ServiceRefusal::PatientInventoryFull)
                        .effect(Effect::Say {
                            key: PATIENT_FULL_INVENTORY,
                        });
                    continue;
                }
                if holds(&trade.worker_cures, &disease) || holds(&trade.hut_cures, &disease) {
                    // `:151-156`.
                    memory.healer.current_patient = Some(patient.id);
                    return decision.going(ServiceState::Cure);
                }
                for cure in disease.cures {
                    if count_of(&trade.worker_cures, cure.item) > 0 {
                        continue; // `:162` — ce remède est déjà en poche.
                    }
                    let need = cure.count.max(0) as u32;
                    if count_of(&trade.hut_cures, cure.item) >= need {
                        // `:164-169` — il est dans les coffres : aller le chercher.
                        return decision
                            .because(ServiceRefusal::CureInHutOnly)
                            .effect(Effect::FetchItem {
                                item: cure.item,
                                count: need,
                            })
                            .going(ServiceState::Base(WorkerState::GatheringRequiredMaterials));
                    }
                    if !trade.cure_requested(cure.item) {
                        // `:187-191` — aucune requête : elle est créée et le patient
                        // repasse à `NEW`.
                        decision = decision
                            .because(ServiceRefusal::CureRequested)
                            .effect(Effect::SetPatientState {
                                patient: patient.id,
                                state: PatientState::New,
                            });
                        break;
                    }
                    decision = decision.because(ServiceRefusal::CureAlreadyRequested); // `:170-186`
                }
            }
            PatientState::Treated => {
                let Some(disease) = patient.disease else {
                    // `:203-207` — une visite pour rendre dix points de vie.
                    memory.healer.current_patient = Some(patient.id);
                    return decision.going(ServiceState::Cure);
                };
                if !holds(&patient.cures, &disease) {
                    // `:209-213` — le patient n'a pas les remèdes : il repasse à `NEW`.
                    return decision
                        .because(ServiceRefusal::CureMissing)
                        .effect(Effect::SetPatientState {
                            patient: patient.id,
                            state: PatientState::New,
                        })
                        .going(ServiceState::Base(WorkerState::Decide));
                }
            }
        }
    }
    if let Some(player) = trade.wounded_player() {
        // `:217-224` — un joueur blessé dans l'hôpital.
        memory.healer.player_to_heal = Some(player.id);
        return decision.going(ServiceState::CurePlayer);
    }
    if let Some(remote) = trade.remote_in_reach(facts.building.position) {
        // `:226-232` — un citoyen sous dix points de vie, dans le rayon.
        memory.healer.remote_patient = Some(remote.id);
        return decision.going(ServiceState::Wander);
    }
    decision.because(ServiceRefusal::NoPatient) // `:233`
}

/// Le dossier de l'état courant, ou la raison de s'arrêter : les quatre premiers tests de
/// `requestCure` (`:243-259`), `cure` (`:313-329`) et `freeCure` (`:392-409`) sont les
/// mêmes.
macro_rules! current_patient {
    ($trade:expr, $memory:expr, $id:ident, $patient:ident) => {
        let Some($id) = $memory.healer.current_patient else {
            return Decision::go(ServiceState::Base(WorkerState::Decide))
                .because(ServiceRefusal::NoPatient);
        };
        let Some($patient) = $trade.patient($id) else {
            $memory.healer.current_patient = None;
            return Decision::go(ServiceState::Base(WorkerState::Decide))
                .because(ServiceRefusal::PatientGone);
        };
        if !$patient.present || !$patient.sick {
            $memory.healer.current_patient = None;
            return Decision::go(ServiceState::Base(WorkerState::Decide))
                .because(ServiceRefusal::PatientGone);
        }
    };
}

/// `requestCure()` (`:241-304`) : une requête `Stack` par remède manquant.
pub fn request_cure(
    facts: &WorldFacts,
    trade: &HealerFacts,
    memory: &mut super::facts::ServiceMemory,
) -> Decision {
    let _ = facts;
    current_patient!(trade, memory, id, patient);
    if !trade.at_target {
        return Decision::stay().because(ServiceRefusal::NotAtPatient); // `:256-259`
    }
    let mut decision = Decision::stay();
    let Some(disease) = patient.disease else {
        // `:261-267` — plus de maladie : le dossier passe à `REQUESTED` et on repart.
        memory.healer.current_patient = None;
        return decision
            .effect(Effect::SetPatientState {
                patient: id,
                state: PatientState::Requested,
            })
            .going(ServiceState::Base(WorkerState::Decide));
    };
    for cure in disease.cures {
        // `:274-298` — ni en poche, ni dans les coffres, ni déjà demandé.
        if count_of(&trade.worker_cures, cure.item) == 0
            && count_of(&trade.hut_cures, cure.item) == 0
            && !trade.cure_requested(cure.item)
        {
            decision = decision.effect(Effect::RequestCure {
                item: cure.item,
                count: REQUEST_COUNT,
            });
        }
    }
    memory.healer.current_patient = None;
    decision
        .effect(Effect::SetPatientState {
            patient: id,
            state: PatientState::Requested,
        })
        .going(ServiceState::Base(WorkerState::Decide)) // `:301-303`
}

/// `cure()` (`:311-383`) : donner les remèdes, ou rendre dix points de vie.
pub fn cure(
    facts: &WorldFacts,
    trade: &HealerFacts,
    memory: &mut super::facts::ServiceMemory,
) -> Decision {
    let _ = facts;
    current_patient!(trade, memory, id, patient);
    if !trade.at_target {
        return Decision::stay().because(ServiceRefusal::NotAtPatient); // `:326-329`
    }
    let mut decision = Decision::stay();
    let Some(disease) = patient.disease else {
        // `:332-338` — `citizen.heal(10)` et l'XP, sans remède.
        memory.healer.current_patient = None;
        return decision
            .because(ServiceRefusal::HealedWithoutCure)
            .effect(Effect::Treated {
                patient: id,
                xp: BASE_XP_GAIN,
            })
            .going(ServiceState::Base(WorkerState::Decide));
    };
    if !holds(&trade.worker_cures, &disease) {
        if holds(&trade.hut_cures, &disease) {
            // `:342-352` — un remède manque en poche et la hutte l'a : aller le prendre.
            for cure in disease.cures {
                if count_of(&trade.worker_cures, cure.item) < cure.count.max(0) as u32 {
                    return decision
                        .because(ServiceRefusal::CureInHutOnly)
                        .effect(Effect::FetchItem {
                            item: cure.item,
                            count: 1,
                        })
                        .going(ServiceState::Base(WorkerState::GatheringRequiredMaterials));
                }
            }
        }
        // `:353-354` — personne n'a le remède : le dossier est abandonné.
        memory.healer.current_patient = None;
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::CureMissing);
    }
    if !holds(&patient.cures, &disease) {
        // `:357-376` — le patient n'a pas ses remèdes : les lui transférer.
        for cure in disease.cures {
            let need = cure.count.max(0) as u32;
            if count_of(&patient.cures, cure.item) < need {
                if !patient.has_space {
                    // `:363-368` — l'inventaire du patient est plein.
                    memory.healer.current_patient = None;
                    return decision
                        .because(ServiceRefusal::PatientInventoryFull)
                        .effect(Effect::Say {
                            key: PATIENT_FULL_INVENTORY,
                        })
                        .going(ServiceState::Base(WorkerState::Decide));
                }
                decision = decision.effect(Effect::GiveCure {
                    patient: id,
                    item: cure.item,
                    count: need,
                });
            }
        }
    }
    memory.healer.current_patient = None;
    decision
        .effect(Effect::Treated {
            patient: id,
            xp: BASE_XP_GAIN,
        }) // `:378-379`
        .effect(Effect::SetPatientState {
            patient: id,
            state: PatientState::Treated,
        }) // `:380`
        .going(ServiceState::Base(WorkerState::Decide)) // `:382`
}

/// `freeCure()` (`:390-438`) : trente ticks de particules, puis la maladie s'en va.
pub fn free_cure(
    facts: &WorldFacts,
    trade: &HealerFacts,
    memory: &mut super::facts::ServiceMemory,
) -> Decision {
    let _ = facts;
    current_patient!(trade, memory, id, patient);
    if !trade.at_target {
        memory.healer.progress_ticks = 0; // `:407` — un retour en arrière remet à zéro.
        return Decision::stay().because(ServiceRefusal::NotAtPatient);
    }
    memory.healer.progress_ticks += 1; // `:411`
    if memory.healer.progress_ticks < MAX_PROGRESS_TICKS {
        return Decision::stay().because(ServiceRefusal::FreeCureRunning); // `:412-429`
    }
    memory.healer.progress_ticks = 0; // `:431`
    memory.healer.current_patient = None;
    decision_with_cure(id)
}

/// Les trois ordres de `:432-437`, dans l'ordre de Java : les statistiques et l'XP
/// (`:432-433`), puis la guérison (`:434`), puis l'état du dossier (`:435`).
fn decision_with_cure(id: u32) -> Decision {
    Decision::go(ServiceState::Base(WorkerState::Decide)) // `:437`
        .effect(Effect::Treated {
            patient: id,
            xp: BASE_XP_GAIN,
        }) // `:432-433`
        .effect(Effect::CureDisease { patient: id }) // `:434`
        .effect(Effect::SetPatientState {
            patient: id,
            state: PatientState::Treated,
        }) // `:435`
}

/// `curePlayer()` (`:445-467`) : soigner un joueur, à condition qu'il soit resté là.
pub fn cure_player(
    facts: &WorldFacts,
    trade: &HealerFacts,
    memory: &mut super::facts::ServiceMemory,
) -> Decision {
    let Some(id) = memory.healer.player_to_heal else {
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::NoPlayerToHeal); // `:447-450`
    };
    let Some(player) = trade.player(id) else {
        memory.healer.player_to_heal = None;
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::PlayerOutOfRange);
    };
    if player.at.dist_sqr(facts.building.position) > PLAYER_RANGE_SQR {
        memory.healer.player_to_heal = None;
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::PlayerOutOfRange); // `:452-456`
    }
    if !trade.at_target {
        return Decision::stay().because(ServiceRefusal::NotAtPatient); // `:458-461`
    }
    Decision::go(ServiceState::Base(WorkerState::Decide)) // `:466`
        .effect(Effect::HealedPlayer {
            player: id,
            amount: player.healed_by(trade.building_level),
        }) // `:463`
}

/// `wander()` (`:480-506`) : marcher vers le citoyen lointain et le soigner.
pub fn wander(
    facts: &WorldFacts,
    trade: &HealerFacts,
    memory: &mut super::facts::ServiceMemory,
) -> Decision {
    let _ = facts;
    let Some(id) = memory.healer.remote_patient else {
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::NoRemotePatient); // `:482-485`
    };
    let Some(remote) = trade.remote else {
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::NoRemotePatient);
    };
    if remote.id != id {
        return Decision::go(ServiceState::Base(WorkerState::Decide))
            .because(ServiceRefusal::NoRemotePatient);
    }
    if !trade.at_target {
        return Decision::stay().because(ServiceRefusal::NotAtPatient); // `:488-491`
    }
    memory.healer.remote_patient = None; // `:503`
    Decision::go(ServiceState::Base(WorkerState::StartWorking)) // `:505`
        .effect(Effect::HealedRemote {
            citizen: id,
            amount: remote.healed_by(trade.building_level),
        }) // `:499`
}

service_action!(decide_action, Healer, decide);
service_action!(request_cure_action, Healer, request_cure);
service_action!(cure_action, Healer, cure);
service_action!(free_cure_action, Healer, free_cure);
service_action!(cure_player_action, Healer, cure_player);
service_action!(wander_action, Healer, wander);

/// Enregistre les sept cibles de `EntityAIWorkHealer.java:86-93`.
///
/// La première (`IDLE`) est celle du passage de relais : elle est commune aux trois
/// métiers et l'enregistrer ici la mettrait deux fois dans la liste de `IDLE`.
pub fn register(machine: &mut Machine<ServiceState, super::ServiceScene>) {
    // `:87` — `new AITarget(START_WORKING, DECIDE, 1)` : un `goto` constant.
    machine.add(Transition::goto(
        ServiceState::Base(WorkerState::StartWorking),
        ServiceState::Base(WorkerState::Decide),
        "startWorkingToDecide",
        "EntityAIWorkHealer.java:87",
        START_TO_DECIDE_RATE,
    ));
    // `:88` — `new AITarget(DECIDE, this::decide, TICKS_SECOND)`.
    add_target(
        machine,
        ServiceState::Base(WorkerState::Decide),
        "decide",
        "EntityAIWorkHealer.java:88",
        WORK_RATE,
        decide_action,
    );
    // `:89-93` — les cinq états de travail, tous à `TICKS_SECOND`.
    add_target(
        machine,
        ServiceState::Cure,
        "cure",
        "EntityAIWorkHealer.java:89",
        WORK_RATE,
        cure_action,
    );
    add_target(
        machine,
        ServiceState::FreeCure,
        "freeCure",
        "EntityAIWorkHealer.java:90",
        WORK_RATE,
        free_cure_action,
    );
    add_target(
        machine,
        ServiceState::CurePlayer,
        "curePlayer",
        "EntityAIWorkHealer.java:91",
        WORK_RATE,
        cure_player_action,
    );
    add_target(
        machine,
        ServiceState::RequestCure,
        "requestCure",
        "EntityAIWorkHealer.java:92",
        WORK_RATE,
        request_cure_action,
    );
    add_target(
        machine,
        ServiceState::Wander,
        "wander",
        "EntityAIWorkHealer.java:93",
        WORK_RATE,
        wander_action,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::facts::Pos;
    use crate::ai_work_12::facts::{
        CureStock, InjuredPlayer, Patient, RemotePatient, ServiceJob, ServiceMemory, ServiceScene,
    };
    use crate::ai_work_12::ServiceAi;
    use crate::citizens::happiness::disease::Disease;

    fn sick(state: PatientState) -> Patient {
        Patient::sick_of(11, state, Disease::INFLUENZA)
    }

    fn hospital(patients: Vec<Patient>) -> HealerFacts {
        HealerFacts {
            at_hospital: true,
            patients,
            // `walkToSafePos(...)` a rendu vrai (`:256`, `:326`, `:405`) : le guérisseur
            // est au chevet de sa cible, ce que tout test de soin suppose.
            at_target: true,
            // Le tirage de guérison magique (`:515`) passe **avant** les remèdes : à
            // `nextInt(3600)`, un tirage de 3 599 ne gagne jamais contre le plancher de 1,
            // donc les tests qui ne parlent pas de cette branche ne la voient pas.
            cure_roll: CURE_ROLL_BOUND - 1,
            ..HealerFacts::default()
        }
    }

    #[test]
    fn a_doctor_still_walking_to_the_hospital_holds_decide() {
        // `:106-109`.
        let facts = WorldFacts::new();
        let trade = HealerFacts::default(); // `at_hospital: false`
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NotAtHospital));
    }

    #[test]
    fn a_stale_patient_file_is_dropped_and_a_sick_neighbour_gets_one() {
        // `:112-125` : un dossier par malade présent, un retrait par dossier mort.
        let facts = WorldFacts::new();
        let dead = Patient {
            present: false,
            ..sick(PatientState::New)
        };
        let trade = HealerFacts {
            at_hospital: true,
            sick_nearby: vec![4],
            patients: vec![dead],
            ..HealerFacts::default()
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert!(decision
            .effects
            .contains(&Effect::CreatePatientFile { citizen: 4 }));
        assert!(decision
            .effects
            .contains(&Effect::DropPatientFile { citizen: 11 }));
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoPatient));
    }

    #[test]
    fn a_new_patient_is_requested_before_anything_else() {
        // `:129-133` — même avec une maladie nulle, un dossier `NEW` passe par
        // `REQUEST_CURE`.
        let facts = WorldFacts::new();
        let mut patient = sick(PatientState::New);
        patient.disease = None;
        let trade = hospital(vec![patient]);
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::RequestCure));
        assert_eq!(memory.healer.current_patient, Some(11));
    }

    #[test]
    fn a_requested_patient_with_the_cure_in_hand_is_cured() {
        // `:151-156` : les remèdes sont en poche, on va soigner.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            worker_cures: vec![
                CureStock::new("minecraft:carrot", 1),
                CureStock::new("minecraft:potato", 1),
            ],
            ..hospital(vec![sick(PatientState::Requested)])
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Cure));
        assert_eq!(memory.healer.current_patient, Some(11));
    }

    #[test]
    fn a_requested_patient_with_only_part_of_the_cure_in_the_hut_fetches_that_part() {
        // `:160-169` : `hasCureInInventory(disease, hut)` est **faux** (il manque la pomme
        // de terre), donc la boucle par remède s'ouvre et la carotte, que la hutte a, part
        // en cueillette. Une hutte qui aurait **tous** les remèdes rendrait `CURE`
        // (`:151-156`) : c'est la nuance que ce test fixe.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            hut_cures: vec![CureStock::new("minecraft:carrot", 2)],
            ..hospital(vec![sick(PatientState::Requested)])
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::CureInHutOnly));
        assert_eq!(
            decision.next,
            Some(ServiceState::Base(WorkerState::GatheringRequiredMaterials))
        );
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::FetchItem {
                item: "minecraft:carrot",
                count: 1
            })
        );
        // La hutte complète, elle, envoie directement soigner.
        let complete = HealerFacts {
            hut_cures: vec![
                CureStock::new("minecraft:carrot", 2),
                CureStock::new("minecraft:potato", 2),
            ],
            ..trade
        };
        let mut memory = ServiceMemory::default();
        assert_eq!(
            decide(&facts, &complete, &mut memory).next,
            Some(ServiceState::Cure)
        );
    }

    #[test]
    fn an_unrequested_cure_resets_the_patient_to_new() {
        // `:187-191` — aucune requête ouverte : elle est créée, le dossier repasse à
        // `NEW`, et la boucle passe au patient suivant.
        let facts = WorldFacts::new();
        let trade = hospital(vec![sick(PatientState::Requested)]);
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::CureRequested));
        assert_eq!(
            decision.effects.last(),
            Some(&Effect::SetPatientState {
                patient: 11,
                state: PatientState::New
            })
        );
        assert_eq!(decision.next, None, "rien à demander : la boucle continue");
    }

    #[test]
    fn a_cure_already_requested_does_not_ask_twice() {
        // `:170-186` : la requête est ouverte ou servie, aucun doublon n'est créé.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            requested_cures: vec!["minecraft:carrot", "minecraft:potato"],
            ..hospital(vec![sick(PatientState::Requested)])
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::CureAlreadyRequested));
        assert!(!decision
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::SetPatientState { .. })));
    }

    #[test]
    fn a_full_patient_inventory_is_reported_and_the_loop_goes_on() {
        // `:195-198` : l'inventaire du patient est plein, le guérisseur le signale et
        // passe au patient suivant, sans changer d'état.
        let facts = WorldFacts::new();
        let full = Patient {
            has_space: false,
            ..sick(PatientState::Requested)
        };
        let trade = hospital(vec![full]);
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::PatientInventoryFull));
        assert!(decision.effects.contains(&Effect::Say {
            key: PATIENT_FULL_INVENTORY
        }));
        assert_eq!(memory.healer.current_patient, None, "aucun patient choisi");
    }

    #[test]
    fn the_free_cure_roll_needs_mana_in_steps_of_twenty() {
        // `:513-516` : `nextInt(3600) <= Math.max(1, getSecondarySkillLevel() / 20)`. Le
        // seuil est `mana / 20` **entier**, plancher 1 : à mana 19 il vaut 1, à mana 20
        // il vaut 1 aussi, et c'est mana 40 qui le fait passer à 2.
        assert_eq!(CURE_ROLL_BOUND, 3600);
        assert!(free_cure_roll(0, 1), "mana 0 : une chance sur 3 600");
        assert!(!free_cure_roll(0, 2));
        assert!(free_cure_roll(19, 1), "19 / 20 = 0, donc le plancher de 1");
        assert!(!free_cure_roll(19, 2));
        assert!(free_cure_roll(20, 1), "20 / 20 = 1 : roll 0 et 1 passent");
        assert!(!free_cure_roll(20, 2));
        assert!(free_cure_roll(40, 2), "40 / 20 = 2");
        assert!(!free_cure_roll(40, 3));
        assert!(free_cure_roll(99, 4), "99 / 20 = 4");
    }

    #[test]
    fn a_drawn_free_cure_sends_the_doctor_to_the_magic_healing() {
        // `:143-147`.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            cure_roll: 0,
            ..hospital(vec![sick(PatientState::Requested)])
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::FreeCure));
    }

    #[test]
    fn a_free_cure_runs_thirty_ticks_then_cures_without_any_item() {
        // `:411-437`.
        let facts = WorldFacts::new();
        let trade = hospital(vec![sick(PatientState::Requested)]);
        let mut memory = ServiceMemory::default();
        memory.healer.current_patient = Some(11);
        for _ in 1..MAX_PROGRESS_TICKS {
            let running = free_cure(&facts, &trade, &mut memory);
            assert_eq!(running.refusal, Some(ServiceRefusal::FreeCureRunning));
        }
        assert_eq!(memory.healer.progress_ticks, MAX_PROGRESS_TICKS - 1);
        let done = free_cure(&facts, &trade, &mut memory);
        assert_eq!(done.next, Some(ServiceState::Base(WorkerState::Decide)));
        assert!(done.effects.contains(&Effect::CureDisease { patient: 11 }));
        assert!(done
            .effects
            .contains(&Effect::SetPatientState {
                patient: 11,
                state: PatientState::Treated
            }));
        assert_eq!(memory.healer.progress_ticks, 0);
        // `walkToSafePos` faux annule la progression (`:405-409`).
        let trade = HealerFacts {
            at_target: false,
            ..trade
        };
        memory.healer.current_patient = Some(11);
        memory.healer.progress_ticks = 5;
        let lost = free_cure(&facts, &trade, &mut memory);
        assert_eq!(lost.refusal, Some(ServiceRefusal::NotAtPatient));
        assert_eq!(memory.healer.progress_ticks, 0);
    }

    #[test]
    fn a_treated_patient_without_the_cure_goes_back_to_new() {
        // `:209-213`.
        let facts = WorldFacts::new();
        let trade = hospital(vec![sick(PatientState::Treated)]);
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::CureMissing));
        assert!(decision.effects.contains(&Effect::SetPatientState {
            patient: 11,
            state: PatientState::New
        }));
    }

    #[test]
    fn a_cure_with_the_items_in_hand_transfers_them_and_marks_the_patient() {
        // `:340-382` : les remèdes sont en poche, le patient n'en a pas.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            worker_cures: vec![
                CureStock::new("minecraft:carrot", 1),
                CureStock::new("minecraft:potato", 1),
            ],
            ..hospital(vec![sick(PatientState::Requested)])
        };
        let mut memory = ServiceMemory::default();
        memory.healer.current_patient = Some(11);
        let decision = cure(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Decide)));
        assert_eq!(
            decision.effects,
            vec![
                Effect::GiveCure {
                    patient: 11,
                    item: "minecraft:carrot",
                    count: 1
                },
                Effect::GiveCure {
                    patient: 11,
                    item: "minecraft:potato",
                    count: 1
                },
                Effect::Treated {
                    patient: 11,
                    xp: BASE_XP_GAIN
                },
                Effect::SetPatientState {
                    patient: 11,
                    state: PatientState::Treated
                },
            ]
        );
        assert_eq!(memory.healer.current_patient, None);
    }

    #[test]
    fn a_cure_without_the_items_and_without_the_hut_gives_the_patient_up() {
        // `:353-354`.
        let facts = WorldFacts::new();
        let trade = hospital(vec![sick(PatientState::Requested)]);
        let mut memory = ServiceMemory::default();
        memory.healer.current_patient = Some(11);
        let decision = cure(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::CureMissing));
        assert_eq!(memory.healer.current_patient, None);
    }

    #[test]
    fn a_patient_without_a_disease_receives_ten_hit_points_and_no_item() {
        // `:332-338`.
        let facts = WorldFacts::new();
        let mut patient = sick(PatientState::Requested);
        patient.disease = None;
        let trade = hospital(vec![patient]);
        let mut memory = ServiceMemory::default();
        memory.healer.current_patient = Some(11);
        let decision = cure(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::HealedWithoutCure));
        assert_eq!(
            decision.effects,
            vec![Effect::Treated {
                patient: 11,
                xp: BASE_XP_GAIN
            }]
        );
    }

    #[test]
    fn a_gone_patient_file_is_dropped_by_every_state_that_needs_it() {
        // `:243-253`, `:313-323`, `:392-402` : les trois états vérifient le dossier.
        let facts = WorldFacts::new();
        let trade = hospital(Vec::new());
        let actions: [fn(&WorldFacts, &HealerFacts, &mut ServiceMemory) -> Decision; 3] =
            [request_cure, cure, free_cure];
        for action in actions {
            let mut memory = ServiceMemory::default();
            memory.healer.current_patient = Some(11);
            let decision = action(&facts, &trade, &mut memory);
            assert_eq!(decision.refusal, Some(ServiceRefusal::PatientGone));
            assert_eq!(memory.healer.current_patient, None);
        }
        // Sans patient en cours, c'est `NoPatient`.
        let mut memory = ServiceMemory::default();
        assert_eq!(
            cure(&facts, &trade, &mut memory).refusal,
            Some(ServiceRefusal::NoPatient)
        );
    }

    #[test]
    fn requesting_a_cure_creates_one_request_per_missing_item_of_sixteen() {
        // `:272-299` et `REQUEST_COUNT` = 16 (`:48`).
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            at_target: true,
            requested_cures: vec!["minecraft:carrot"],
            ..hospital(vec![sick(PatientState::New)])
        };
        let mut memory = ServiceMemory::default();
        memory.healer.current_patient = Some(11);
        let decision = request_cure(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Base(WorkerState::Decide)));
        assert_eq!(
            decision.effects,
            vec![
                Effect::RequestCure {
                    item: "minecraft:potato",
                    count: 16
                },
                Effect::SetPatientState {
                    patient: 11,
                    state: PatientState::Requested
                },
            ],
            "la carotte est déjà demandée, seule la pomme de terre manque"
        );
    }

    #[test]
    fn a_wounded_player_is_healed_to_five_below_max_minus_the_level() {
        // `:217-220` et `:463-466`.
        let facts = WorldFacts::new();
        let player = InjuredPlayer {
            id: 5,
            at: Pos::new(1, 64, 1),
            health: 4.0,
            max_health: 20.0,
        };
        let trade = HealerFacts {
            players: vec![player],
            at_target: true,
            at_hospital: true,
            building_level: 2,
            ..HealerFacts::default()
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::CurePlayer));
        assert_eq!(memory.healer.player_to_heal, Some(5));
        let healed = cure_player(&facts, &trade, &mut memory);
        assert_eq!(
            healed.effects,
            vec![Effect::HealedPlayer {
                player: 5,
                amount: 20.0 - 4.0 - 5.0 - 2.0
            }]
        );
        assert_eq!(
            memory.healer.player_to_heal,
            Some(5),
            "`:463-466` ne remet pas `playerToHeal` à zéro : seule la sortie de portée le \
             fait (`:453`)"
        );
    }

    #[test]
    fn a_player_further_than_fifty_blocks_is_dropped() {
        // `:452-456` — `distSqr > 50 * 50`.
        let mut facts = WorldFacts::new();
        facts.building.position = Pos::new(0, 64, 0);
        let player = InjuredPlayer {
            id: 5,
            at: Pos::new(51, 64, 0),
            health: 1.0,
            max_health: 20.0,
        };
        let trade = HealerFacts {
            players: vec![player],
            at_target: true,
            ..HealerFacts::default()
        };
        let mut memory = ServiceMemory::default();
        memory.healer.player_to_heal = Some(5);
        let decision = cure_player(&facts, &trade, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::PlayerOutOfRange));
        assert_eq!(memory.healer.player_to_heal, None);
        // Exactement 50 blocs : `2500 > 2500` est faux, donc il reste.
        let at_range = InjuredPlayer {
            at: Pos::new(50, 64, 0),
            ..player
        };
        let trade = HealerFacts {
            players: vec![at_range],
            ..trade
        };
        memory.healer.player_to_heal = Some(5);
        let kept = cure_player(&facts, &trade, &mut memory);
        assert_eq!(kept.refusal, None);
    }

    #[test]
    fn a_remote_patient_below_ten_health_is_healed_after_a_walk() {
        // `:226-232` puis `:480-506`.
        let facts = WorldFacts::new();
        let remote = RemotePatient {
            id: 9,
            at: Pos::new(5, 64, 0),
            health: 3.0,
            max_health: 20.0,
        };
        let trade = HealerFacts {
            remote: Some(remote),
            at_target: true,
            at_hospital: true,
            building_level: 1,
            ..HealerFacts::default()
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, Some(ServiceState::Wander));
        assert_eq!(memory.healer.remote_patient, Some(9));
        let healed = wander(&facts, &trade, &mut memory);
        assert_eq!(
            healed.next,
            Some(ServiceState::Base(WorkerState::StartWorking)),
            "`:505` rend START_WORKING, pas DECIDE"
        );
        assert_eq!(
            healed.effects,
            vec![Effect::HealedRemote {
                citizen: 9,
                amount: 20.0 - 3.0 - 5.0 - 1.0
            }]
        );
        // Un citoyen à dix points de vie n'est pas « critique » (`:227`).
        let healthy = HealerFacts {
            remote: Some(RemotePatient {
                health: 10.0,
                ..remote
            }),
            ..trade
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &healthy, &mut memory);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoPatient));
    }

    #[test]
    fn with_nothing_to_do_the_doctor_stays_in_decide() {
        // `:233` — `return DECIDE`.
        let facts = WorldFacts::new();
        let trade = HealerFacts {
            at_hospital: true,
            ..HealerFacts::default()
        };
        let mut memory = ServiceMemory::default();
        let decision = decide(&facts, &trade, &mut memory);
        assert_eq!(decision.next, None);
        assert_eq!(decision.refusal, Some(ServiceRefusal::NoPatient));
    }

    #[test]
    fn the_pickup_after_a_fetch_goes_to_the_cure_not_to_the_work() {
        // `:470-473` : le seul des trois métiers qui écrase `getStateAfterPickUp`.
        assert_eq!(state_after_pickup(), ServiceState::Cure);
        let ai = ServiceAi::new(crate::ai_work_12::ServiceTrade::Healer);
        // La cible de relais `GATHERING_REQUIRED_MATERIALS → CURE` existe donc.
        let machine = ai.machine();
        assert!(machine.can_leave(ServiceState::Base(WorkerState::GatheringRequiredMaterials)));
        let scene = ServiceScene::new(WorldFacts::new(), ServiceJob::Healer(HealerFacts::default()));
        assert!(scene.healer().is_some());
        assert!(scene.undertaker().is_none());
    }

    #[test]
    fn the_registered_table_is_seven_trade_targets_plus_the_hand_over() {
        // `EntityAIWorkHealer.java:86-93`.
        let ai = ServiceAi::new(crate::ai_work_12::ServiceTrade::Healer);
        let machine = ai.machine();
        assert_eq!(machine.transitions().len(), 5 + 7);
        let rates: Vec<(&str, u32)> = machine
            .transitions()
            .iter()
            .filter(|transition| {
                matches!(
                    transition.rule,
                    "startWorkingToDecide"
                        | "decide"
                        | "cure"
                        | "freeCure"
                        | "curePlayer"
                        | "requestCure"
                        | "wander"
                )
            })
            .map(|transition| (transition.rule, transition.tick_rate))
            .collect();
        assert_eq!(
            rates,
            vec![
                ("startWorkingToDecide", START_TO_DECIDE_RATE),
                ("decide", WORK_RATE),
                ("cure", WORK_RATE),
                ("freeCure", WORK_RATE),
                ("curePlayer", WORK_RATE),
                ("requestCure", WORK_RATE),
                ("wander", WORK_RATE),
            ]
        );
        assert!(machine.can_leave(ServiceState::Base(WorkerState::Decide)));
        assert!(machine.can_leave(ServiceState::Wander));
    }
}
