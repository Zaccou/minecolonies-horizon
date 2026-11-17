//! Pourquoi une lecture de ces quatre fenêtres est refusée — nommé.
//!
//! **Aucune de ces quatre fenêtres ne refuse un clic avec une phrase.** Java dit non de
//! quatre façons, et trois sont muettes : un bouton qu'on éteint
//! (`WindowBuilderResModule.java:167`, `:186`), un panneau qu'on cache
//! (`WindowColonyMap.java:217`), et une écriture qu'on saute
//! (`AbstractWindowWorkerModuleBuilding.java:192-197` — `if (worker != null)` — puis
//! `WindowColonyMap.java:481-486` pour le même test sur un citoyen). La quatrième parle, et elle
//! ne vient pas d'une fenêtre : c'est l'item qui refuse d'ouvrir la carte
//! (`ItemColonyMap.java:118-121`, `COM_MINECOLONIES_MAP_NEED_COLONY`).
//!
//! D'où [`Refusal::key`] qui répond `None` **presque partout**. Les refus de **clic** de
//! ces fenêtres sont déjà ceux de `src/ui` et ne sont pas recopiés ici : `NotBuilt` pour
//! l'embauche (`AbstractWindowWorkerModuleBuilding.java:145-149`), `RecallFailed` pour le
//! rappel (`:157-160`), `ResourceMissing` et `ResourceNotNeeded` pour la ligne de
//! ressource (`WindowBuilderResModule.java:166-171`, `:184-190`). Ce fichier ne nomme
//! que ce que `src/ui` ne peut pas nommer : les refus de **lecture**, qui n'ont pas de
//! bouton.

use std::fmt;

/// Un refus propre à ces quatre fenêtres.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: ce que ces quatre fenêtres refusent en plus de `src/ui`, fermé exprès
pub enum Refusal {
    /// Un id de fenêtre que cette table ne décrit pas.
    UnknownWindow,
    /// L'item « carte de colonie » n'a aucune colonie liée : Java n'ouvre pas la
    /// fenêtre et affiche un message dans la barre d'action
    /// (`ItemColonyMap.java:118-121`).
    MapNeedsColony,
    /// La salle des fêtes n'a aucune carte dans ses données : `hasMaps` reste faux et
    /// le panneau `warning` s'affiche à sa place (`WindowColonyMap.java:267`).
    /// **Muette** : le joueur lit l'avertissement de l'XML, pas un refus.
    NoMaps,
    /// La fenêtre est ouverte depuis l'item et non depuis la salle des fêtes : le
    /// bouton `inventory` est **caché** (`WindowColonyMap.java:217`).
    NotAtTownHall,
    /// La ligne de travailleur nomme un citoyen que la colonie du client ne connaît
    /// pas : Java saute l'écriture (`WindowColonyMap.java:481-486`,
    /// `AbstractWindowWorkerModuleBuilding.java:192-197`) et le panneau garde le texte
    /// de la ligne recyclée par la liste.
    UnknownCitizen { id: i32 },
    /// Le panneau caché `resourceId` ne porte pas un entier : Java appellerait
    /// `Integer.parseInt` et lèverait `NumberFormatException`
    /// (`WindowBuilderResModule.java:240`).
    BadRowIndex { text: String },
    /// Le panneau caché `resourceQuantity` ne porte pas un entier
    /// (`WindowBuilderResModule.java:256`).
    BadQuantity { text: String },
    /// L'index lu ne nomme aucune ligne de la liste : Java journalise un
    /// avertissement et ne fait rien (`WindowBuilderResModule.java:242-245`).
    UnknownResource { index: i32 },
    /// La récupération des requêtes résolues par le joueur a levé : Java attrape,
    /// remet le gestionnaire à zéro et rend une liste vide
    /// (`WindowColonyMap.java:744-749`). L'icône d'état ne s'affiche donc pas.
    RequestLookupFailed,
}

impl Refusal {
    /// Un nom stable, préfixé par la fenêtre à laquelle il appartient.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "map.refusal.unknown_window",
            Refusal::MapNeedsColony => "map.refusal.needs_colony",
            Refusal::NoMaps => "map.refusal.no_maps",
            Refusal::NotAtTownHall => "map.refusal.not_at_town_hall",
            Refusal::UnknownCitizen { .. } => "worker.refusal.unknown_citizen",
            Refusal::BadRowIndex { .. } => "builder.refusal.bad_row_index",
            Refusal::BadQuantity { .. } => "builder.refusal.bad_quantity",
            Refusal::UnknownResource { .. } => "builder.refusal.unknown_resource",
            Refusal::RequestLookupFailed => "map.refusal.request_lookup",
        }
    }

    /// La clé que le joueur lit, quand il en lit une.
    ///
    /// `None` n'est pas « aucune raison » : c'est « Java éteint un bouton ou saute une
    /// écriture », donc un appelant ne doit **jamais** imprimer [`Self::name`] là où le
    /// joueur attend une phrase. Un seul refus de ce fichier parle, et il ne vient pas
    /// d'une fenêtre.
    pub fn key(&self) -> Option<&'static str> {
        match self {
            Refusal::MapNeedsColony => Some("com.minecolonies.core.item.colonymap.needcolony"),
            Refusal::UnknownWindow
            | Refusal::NoMaps
            | Refusal::NotAtTownHall
            | Refusal::UnknownCitizen { .. }
            | Refusal::BadRowIndex { .. }
            | Refusal::BadQuantity { .. }
            | Refusal::UnknownResource { .. }
            | Refusal::RequestLookupFailed => None,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Refusal> {
        vec![
            Refusal::UnknownWindow,
            Refusal::MapNeedsColony,
            Refusal::NoMaps,
            Refusal::NotAtTownHall,
            Refusal::UnknownCitizen { id: 7 },
            Refusal::BadRowIndex { text: "x".into() },
            Refusal::BadQuantity { text: "".into() },
            Refusal::UnknownResource { index: 3 },
            Refusal::RequestLookupFailed,
        ]
    }

    #[test]
    fn nine_refusals_and_each_has_its_own_name() {
        let refusals = all();
        assert_eq!(refusals.len(), 9);
        for (index, refusal) in refusals.iter().enumerate() {
            // Un refus dont le nom ressemble à une phrase finirait à l'écran : le
            // préfixe et les points l'en empêchent.
            assert!(refusal.name().contains(".refusal."), "{refusal} ");
            assert!(!refusal.name().contains(' '));
            for other in &refusals[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "deux refus partagent un nom");
            }
        }
    }

    #[test]
    fn one_refusal_only_is_ever_spoken_and_it_is_the_items() {
        let keyed: Vec<&str> = all()
            .iter()
            .filter(|refusal| refusal.key().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(keyed, ["map.refusal.needs_colony"]);
        assert_eq!(
            Refusal::MapNeedsColony.key(),
            Some("com.minecolonies.core.item.colonymap.needcolony")
        );
        // Le refus le plus visible de ces fenêtres — pas de carte — est muet : Java
        // affiche l'avertissement de l'XML, pas un message de refus.
        assert_eq!(Refusal::NoMaps.key(), None);
        assert_eq!(Refusal::NotAtTownHall.key(), None);
    }

    #[test]
    fn a_variant_that_carries_data_keeps_one_name() {
        assert_eq!(
            Refusal::UnknownCitizen { id: 1 }.name(),
            Refusal::UnknownCitizen { id: 42 }.name()
        );
        // Deux textes fautifs différents restent deux valeurs différentes.
        assert_ne!(
            Refusal::BadRowIndex { text: "a".into() },
            Refusal::BadRowIndex { text: "b".into() }
        );
        assert_eq!(Refusal::UnknownResource { index: 0 }.to_string(), "builder.refusal.unknown_resource");
    }
}
