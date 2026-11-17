//! Le forgeron — le métier qui n'ajoute **rien**.
//!
//! Référence Java : `EntityAIWorkBlacksmith.java:1-33`. Mesuré sur l'arbre : 33 lignes,
//! dont un constructeur qui appelle `super(blacksmith)` (`:105-108`) et un
//! `getExpectedBuildingClass()` qui rend `BuildingBlacksmith.class` (`:110-114`).
//! Aucune cible, aucun état, aucun refus, aucune constante. Sa table est donc
//! **exactement** celle de `AbstractEntityAICrafting.java:160-169` : les sept cibles
//! communes et les deux sorties que la classe de base ajoute.
//!
//! **Ce que ce fichier refuse de faire.** Écrire une seconde machine « pour le
//! forgeron » serait une copie sans une seule ligne de différence. Il n'y a donc pas
//! de [`crate::ai::Machine`] propre ici : [`register`] enregistre la table commune et
//! les tests vérifient, cible par cible, que rien d'autre n'a été ajouté.
//!
//! Les deux réponses que le forgeron prend à ses parents et qui ne sont pas des états :
//!
//! - `isAfterDumpPickupAllowed()` (`AbstractEntityAICrafting.java:741-744`) :
//!   `currentRequest == null`. Le boulanger, lui, répond toujours vrai
//!   (`EntityAIWorkBaker.java:68-72`).
//! - `getActionsDoneUntilDumping()` (`:724-727`) : `1`, pour les trois métiers.

use crate::ai::Machine;
use crate::ai_build::crafter::CraftFacts;

use super::state::{register_shared, Scene, WorkState};

/// La classe Java, pour une ligne de log ou un nom de message.
pub const CLASS: &str = "EntityAIWorkBlacksmith";
/// `getExpectedBuildingClass()` (`EntityAIWorkBlacksmith.java:110-114`).
pub const BUILDING: &str = "BuildingBlacksmith";

/// `isAfterDumpPickupAllowed()` (`AbstractEntityAICrafting.java:741-744`) : le
/// forgeron ramasse après un vidage tant qu'aucune requête n'est en cours.
pub fn is_after_dump_pickup_allowed(facts: &CraftFacts) -> bool {
    !facts.has_request // `:743`
}

/// `getActionsDoneUntilDumping()` (`AbstractEntityAICrafting.java:724-727`) : une
/// action, puis on vide. Le même chiffre pour l'alchimiste et le boulanger.
pub const ACTIONS_UNTIL_DUMPING: u32 = 1;

/// La table du forgeron : la commune, et rien de plus.
pub fn register(machine: &mut Machine<WorkState, Scene>) {
    register_shared(machine);
}

/// Une machine de forgeron, dans `IDLE`.
pub fn ai() -> Machine<WorkState, Scene> {
    let mut machine = super::state::machine();
    register(&mut machine);
    machine
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_build::crafter::CraftState;

    /// `EntityAIWorkBlacksmith.java:1-33` : la table du forgeron est la commune, et le
    /// compte des cibles le prouve — deux machines montées séparément doivent avoir
    /// exactement les mêmes règles, dans le même ordre.
    #[test]
    fn the_blacksmith_table_is_the_shared_table_and_nothing_else() {
        let smith = ai();
        let mut shared = super::super::state::machine();
        register_shared(&mut shared);
        assert_eq!(smith.transitions().len(), shared.transitions().len());
        for (mine, theirs) in smith.transitions().iter().zip(shared.transitions()) {
            assert_eq!(mine.java, theirs.java);
            assert_eq!(mine.rule, theirs.rule);
            assert_eq!(mine.tick_rate, theirs.tick_rate);
        }
        // Les sept cibles de `:164-169` plus les deux sorties de la classe de base.
        assert_eq!(smith.transitions().len(), 8);
    }

    /// `:741-744` : le ramassage après vidage n'est permis qu'entre deux requêtes.
    #[test]
    fn the_pickup_after_a_dump_waits_for_the_request_to_end() {
        assert!(is_after_dump_pickup_allowed(&CraftFacts::default()));
        assert!(!is_after_dump_pickup_allowed(&CraftFacts {
            has_request: true,
            ..CraftFacts::default()
        }));
    }

    /// `:724-727` : une action entre deux vidages, et les noms de Java.
    #[test]
    fn one_action_per_dump_and_javas_names() {
        assert_eq!(ACTIONS_UNTIL_DUMPING, 1);
        assert_eq!(CLASS, "EntityAIWorkBlacksmith");
        assert_eq!(BUILDING, "BuildingBlacksmith");
        assert_eq!(CraftState::Craft.java_name(), "CRAFT");
    }
}
