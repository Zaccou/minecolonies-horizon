//! Les quatre fenêtres de `core/client/gui/` qui **ne sont pas des écrans de hutte** :
//! ce qu'elles demandent, ce qu'elles rendent, et l'état qu'un clic y change.
//!
//! `src/ui` décrit treize écrans de hutte et deux de ces quatre-là s'y trouvent déjà
//! (`WindowHireWorker` sous `hire`, `WindowHutAllInventory` sous `hut.inventory`) : ce
//! module **n'en réécrit pas les panneaux**, il porte ce qu'une table statique ne peut pas
//! tenir — l'état initial, les transitions, les refus nommés, l'ordre d'une liste, et le
//! message que chaque clic envoie. `Panes::Ui` dit « va voir `src/ui` » au lieu de répéter.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`confirm`] | `WindowConfirm` : deux textes, deux boutons, deux ids à l'envers |
//! | [`decoration`] | `WindowDecorationController` : un chemin, deux boutons, quatre conditions |
//! | [`hire`] | `WindowHireWorker` : la liste, ses quatre boutons par ligne, deux messages |
//! | [`inventory`] | `WindowHutAllInventory` : filtre, cinq tris, loupe, dix ticks de retard |
//! | [`refusal`] | pourquoi un clic ne fait rien, nommé, et qui parle au joueur |
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | 4 | [`WINDOWS`] |
//! | déjà décrites par `src/ui` | **2** | `hire`, `hut.inventory` |
//! | panneaux décrits ici | 3 | `title`, `warning`, `name` |
//! | boutons que ces fenêtres enregistrent | 15 | `confirm` 2, décoration 3, hire 7, inventaire 3 |
//! | classes de message nommées | 5 | [`hire`], [`decoration`] |
//! | état `static` en Java | **1** | le tri de l'inventaire ([`inventory::Session`]) |
//!
//! # La prémisse du brief, et ce que le source répond
//!
//! Le brief demande « les champs affichés (nom, niveau, progression, stock) ». Sur ces
//! quatre fenêtres :
//!
//! | Champ | Où il est vraiment |
//! | --- | --- |
//! | un **nom** | `hire` : le citoyen (`WindowHireWorker.java:478`) ; `confirm` : un titre et un avertissement (`:26-27`) ; décoration : le **chemin** du blueprint (`:58-59`) |
//! | un **niveau** | **nulle part comme panneau** : la décoration ne l'affiche qu'à l'intérieur du chemin (`builder1`), et la fenêtre d'embauche n'en lit aucun |
//! | une **progression** | **aucune des quatre** — c'est le builder et le navigateur (`src/ui_window_02`) |
//! | un **stock** | `hut.inventory` seulement : la ligne `quantities` (`WindowHutAllInventory.java:328-336`) |
//!
//! # Quatre prémisses que le source contredit
//!
//! 1. **« `WindowConfirm` confirme. »** Ses deux boutons sont croisés : l'id `cancel`
//!    porte `gui.yes` et **exécute** l'action, l'id `confirm` porte `gui.cancel` et
//!    **ferme** (`WindowConfirm.java:24-25`, `windowconfirm.xml:5-10`). Et le « oui »
//!    n'appelle pas `close()` : c'est à l'action de refermer, ce que
//!    `WindowBuildBuilding.java:216-229` ne fait pas.
//! 2. **« La fenêtre d'embauche embauche. »** Elle n'envoie rien : elle appelle
//!    `addCitizen` sur la vue du module, et c'est la vue qui envoie
//!    `HireFireMessage` (`WorkerBuildingModuleView.java:73-88`). Un citoyen qui avait
//!    déjà un métier en envoie donc **deux**, dans cet ordre : l'ancien, puis le nouveau.
//! 3. **« Être plein veut dire ne plus avoir de place. »** `isFull()` est
//!    `!allowsAssignment() || assigned >= max` (`WorkerBuildingModuleView.java:200`) :
//!    un bâtiment qui interdit l'assignation est plein à zéro employé.
//! 4. **« Le tri de l'inventaire appartient à la fenêtre. »** Il est `static`
//!    (`WindowHutAllInventory.java:63`) : il survit à la fermeture et deux fenêtres le
//!    partagent. [`inventory::Session`] est là pour que le port ne le perde pas.
//!
//! # Ce qui n'est pas ici
//!
//! - **le dessin** : positions, tailles, couleurs, sprites. Même règle que `src/ui`, et
//!   la loupe ne garde que l'**arithmétique** de la couleur (`inventory::highlight`).
//! - **l'envoi** : les cinq classes de message sont nommées, aucune n'est envoyée — même
//!   constat que `src/ui/action.rs`, et `MANQUANT.md` §6 dit ce qui manque.
//! - **la lecture d'un rack ou d'un pack** : `MANQUANT.md` §2 et §4.

pub mod confirm;
pub mod decoration;
pub mod hire;
pub mod inventory;
pub mod refusal;

pub use refusal::Refusal;

use crate::ui::field::Field;

/// L'id de la boîte de confirmation dans cette table.
pub const CONFIRM_ID: &str = "dialog.confirm";
/// L'id du contrôleur de décoration dans cette table. La fenêtre Java a un **bloc**, pas
/// une hutte : elle n'a donc pas d'id dans `src/ui`.
pub const DECORATION_ID: &str = "decoration.controller";

/// Qui décrit les panneaux d'une fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panes {
    /// Cette tranche : aucune table de `src/ui` ne décrit la fenêtre.
    Own(&'static [Field]),
    /// `src/ui` la décrit déjà, sous cet id d'écran : on y renvoie.
    Ui(&'static str),
}

/// Une fenêtre : sa classe, son XML, et qui porte ses panneaux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. Pour les deux fenêtres que `src/ui` décrit, c'est **son** id,
    /// pour que la jointure ne demande aucune traduction.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// Qui la décrit.
    pub panes: Panes,
}

/// Les quatre fenêtres de ce module.
pub static WINDOWS: &[Window] = &[
    Window {
        id: CONFIRM_ID,
        java: confirm::JAVA,
        resource: confirm::RESOURCE,
        panes: Panes::Own(confirm::FIELDS),
    },
    Window {
        id: DECORATION_ID,
        java: decoration::JAVA,
        resource: decoration::RESOURCE,
        panes: Panes::Own(decoration::FIELDS),
    },
    Window {
        id: crate::ui::screen::HIRE_ID,
        java: hire::JAVA,
        resource: hire::RESOURCE,
        panes: Panes::Ui(crate::ui::screen::HIRE_ID),
    },
    Window {
        id: crate::ui::screen::HUT_INVENTORY_ID,
        java: inventory::JAVA,
        resource: inventory::RESOURCE,
        panes: Panes::Ui(crate::ui::screen::HUT_INVENTORY_ID),
    },
];

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// C'est la jointure avec le reste du port : `src/ui` nomme ces quatre classes en
/// `Effect::OpensJava`, et cette fonction répond la ligne qui dit ce qu'elles demandent.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

/// L'enregistrement de `src/ui` qui décrit les panneaux de cette fenêtre, s'il y en a un.
pub fn screen_of(window: &Window) -> Option<&'static crate::ui::Screen> {
    match window.panes {
        Panes::Ui(id) => crate::ui::screen_by_id(id).ok(),
        Panes::Own(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Scope;

    #[test]
    fn four_windows_share_nothing_but_the_shape() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(!window.id.is_empty());
            assert!(window.java.starts_with("Window"), "{} n'a pas de classe", window.id);
            assert!(
                window.resource.starts_with("minecolonies:gui/") && window.resource.ends_with(".xml"),
                "{} lit {}",
                window.id,
                window.resource
            );
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenêtres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
            }
        }
    }

    #[test]
    fn two_of_the_four_are_already_described_by_src_ui_and_are_not_described_twice() {
        let own: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| matches!(window.panes, Panes::Own(_)))
            .map(|window| window.id)
            .collect();
        assert_eq!(own, [CONFIRM_ID, DECORATION_ID]);
        // Les deux autres renvoient à `src/ui`, et la classe de l'écran y est la même :
        // c'est la preuve que la jointure ne traduit rien.
        for window in WINDOWS {
            match screen_of(window) {
                Some(screen) => {
                    assert_eq!(screen.id, window.id);
                    assert_eq!(screen.window, window.java, "{}", window.id);
                }
                None => assert!(matches!(window.panes, Panes::Own(_)), "{}", window.id),
            }
        }
        // Et aucune fenêtre de cette tranche ne réclame l'id d'un écran de hutte qu'elle
        // ne décrit pas.
        for window in WINDOWS {
            if let Panes::Own(_) = window.panes {
                assert_eq!(
                    crate::ui::screen_by_id(window.id).map(|screen| screen.id),
                    Err(crate::ui::Refusal::UnknownScreen),
                    "{} usurpe un id de src/ui",
                    window.id
                );
            }
        }
    }

    #[test]
    fn the_three_panes_described_here_are_well_formed() {
        let mut total = 0;
        for window in WINDOWS {
            if let Panes::Own(panes) = window.panes {
                for (index, pane) in panes.iter().enumerate() {
                    assert!(!pane.id.is_empty());
                    let repeated = panes[..index]
                        .iter()
                        .any(|other| other.id == pane.id && other.scope == pane.scope);
                    assert!(!repeated, "{} répète {}", window.id, pane.id);
                    assert_eq!(pane.scope, Scope::Window, "aucune de ces fenêtres n'a de liste");
                    total += 1;
                }
            }
        }
        assert_eq!(total, 3, "title, warning, name");
    }

    #[test]
    fn the_two_joins_answer_by_id_and_by_class_and_refuse_the_rest() {
        assert_eq!(window_by_java("WindowConfirm").map(|window| window.id), Ok(CONFIRM_ID));
        assert_eq!(
            window_by_java("WindowHutAllInventory").map(|window| window.id),
            Ok("hut.inventory")
        );
        // La troisième table de `src/ui` dit la même chose que celle-ci.
        let from_ui = crate::ui::screen::HUT_WINDOWS
            .iter()
            .find(|(class, _)| *class == inventory::JAVA)
            .map(|(_, id)| *id);
        assert_eq!(from_ui, Some("hut.inventory"));
        // Un refus nommé, pas un `None` muet : ces classes existent et ne sont pas ici.
        assert_eq!(window_by_java("WindowBuildBuilding"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowInfo"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id(CONFIRM_ID).map(|window| window.java), Ok("WindowConfirm"));
    }
}
