//! Quatre fenêtres : **ce qu'elles demandent, ce qu'elles rendent, et l'état qu'elles
//! tiennent**. Pas le dessin.
//!
//! Java référence : `core/client/gui/` —
//! `huts/WindowHutWorkerModulePlaceholder.java` (24 lignes) et sa base
//! `AbstractWindowWorkerModuleBuilding.java` (204),
//! `modules/building/WindowBuilderResModule.java` (264),
//! `map/WindowColonyMap.java` (754), `map/WindowColonyPrestigeRanking.java` (69).
//!
//! **Même forme que `src/ui`, pas une deuxième.** Les panneaux sont des
//! [`crate::ui::Field`], les boutons des [`crate::ui::action::Action`], les refus de clic
//! ceux de [`crate::ui::action::Refusal`]. **Deux des quatre fenêtres sont déjà des
//! écrans de `src/ui`** — `hut.worker` et `tab.resources` — et ce module ne les
//! redéclare pas : [`Window::fields`] et [`Window::actions`] rendent alors *les tranches
//! de `src/ui` elles-mêmes*, et un test vérifie que c'est la même adresse mémoire. Ce que
//! ce module ajoute est ce qu'une table statique ne peut pas porter : **l'état initial et
//! les transitions** (la priorité qui monte, la liste qui se retrie, l'horloge qui
//! rafraîchit, le zoom qui se réécrit).
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`refusal`] | pourquoi une lecture est refusée, et qui le dit au joueur |
//! | [`worker`] | la priorité de livraison et la ligne de chaque ouvrier |
//! | [`builder`] | les quatre états d'une ressource, l'ordre de la liste, l'horloge |
//! | [`map`] | le zoom, les sept couches, et les infobulles non traduites |
//! | [`prestige`] | une liste triée à l'ouverture, et rien qui la mette à jour |
//!
//! # Les comptes, mesurés
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | **4** | [`WINDOWS`] |
//! | dont déjà un écran de `src/ui` | **2** | `hut.worker`, `tab.resources` |
//! | dont dans aucune table | **2** | [`map::ID`], [`prestige::ID`] |
//! | panneaux fixes | **19** | 5 + 11 + 1 + 2 |
//! | boutons | **18** | 10 + 1 + 4 + 3 |
//! | boutons gardés par un refus | **4** | tous dans `src/ui` |
//! | boutons de ces deux fenêtres-ci qui refusent | **0** | carte et classement |
//! | états d'une ressource | 5 | [`builder::Availability`] (réutilisé) |
//! | couches de la carte | **7** | [`map::DECORATIONS`] |
//! | refus nommés par ce module | 9 | [`refusal::Refusal`] |
//! | classes de message nommées | **6** | `ChangeDeliveryPriority`, `RecallCitizen`, `ForcePickup`, `MarkBuildingDirty`, `TransferItemsRequest`, `OpenInventory` |
//!
//! # Quatre prémisses que la source contredit
//!
//! 1. **« Ce sont quatre écrans de hutte. »** Deux le sont. La carte et le classement ont
//!    un bouton `inventory` et une croix, pas un titre de bâtiment, et ils s'ouvrent
//!    depuis un **item** (`ItemColonyMap.java:119`) ou depuis la page principale de la
//!    salle des fêtes (`WindowMainPage.java:401`) — pas par `IBuildingView.getWindow()`.
//!    [`crate::ui::screen_by_id`] les refuse donc par leur nom
//!    (`ui.refusal.unknown_screen`), et c'est [`window_by_id`] qui les résout.
//! 2. **« Le classement de prestige classe du meilleur au pire. »** Il trie **croissant**
//!    (`WindowColonyPrestigeRanking.java:43`) : le meilleur est la **dernière** ligne.
//!    Voir [`prestige::Ranking::new`].
//! 3. **« L'onglet du builder trie comme la liste de ressources. »** Non : les deux
//!    lisent le même comparateur avec des arguments différents, et l'onglet montre **ce
//!    qu'il a déjà** en premier quand le rouleau montre le plus rare. Voir
//!    [`builder::ORDER`] et `ui_window_06::resources::ORDER`.
//! 4. **« `WindowHutWorkerModulePlaceholder` est une fenêtre. »** C'est une coquille de
//!    vingt-quatre lignes : un constructeur qui passe un XML. Tout son comportement est
//!    dans `AbstractWindowWorkerModuleBuilding`, et `src/ui` décrit déjà ses panneaux et
//!    ses boutons. Ce module n'ajoute donc que son état ([`worker::Priority`],
//!    [`worker::Row`]).
//!
//! # Ce qui n'est pas ici
//!
//! - **le dessin** : `pos`, `size`, sprites, et la position calculée du bouton d'une ligne
//!   (`WindowBuilderResModule.java:193-196`).
//! - **les valeurs** : huit sources du modèle (`crate::ui::field::Source`) et trois
//!   objets que le port n'a pas — une vue de bâtiment cliente, un gestionnaire de
//!   requêtes client, un inventaire de joueur. `MANQUANT.md` dit lequel manque et ce que
//!   chaque manque coûte.
//! - **l'envoi** : six classes de message sont **nommées**, aucune n'est envoyée. Le port
//!   n'a ni client ni réseau.

pub mod builder;
pub mod map;
pub mod prestige;
pub mod refusal;
pub mod worker;

pub use refusal::Refusal;

use crate::ui::action::{Action, Refusal as UiRefusal};
use crate::ui::field::Field;
use crate::ui::screen::{Title, HUT_WORKER_ID, TAB_RESOURCES_ID};

/// Ce que la fenêtre écrit en haut, quand elle écrit quelque chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre formes de titre dans ces quatre fenêtres
pub enum Header {
    /// Aucun titre : la carte n'en dessine pas, son fond est une image
    /// (`windowcolonymap.xml:10`).
    None,
    /// Un libellé de l'XML, avec ou sans id, que **aucune classe ne réécrit**
    /// (`windowcolonyprestigeranking.xml:10`).
    Xml(&'static str),
    /// `nom` + ` ` + niveau, écrit par `onOpened`
    /// (`AbstractBuildingMainWindow.java:171-176`) : c'est le titre de `src/ui`, pas une
    /// seconde règle.
    BuildingName,
    /// Le titre vient de l'écran de `src/ui` avec cet id — [`crate::ui::Screen::title`].
    SrcUi(&'static str),
}

/// Une fenêtre de cette table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. **Pas celui de Java**, qui n'en a pas : une fenêtre s'ouvre
    /// par sa classe, un module ou un item.
    pub id: &'static str,
    /// La classe qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// Ce que le haut de la fenêtre lit.
    pub header: Header,
    /// Qui l'ouvre, par classe Java ou par item — jamais « un bouton » : le bouton est
    /// une [`Action`], l'ouvreur est un point d'entrée.
    pub openers: &'static [&'static str],
    /// L'écran de `src/ui` que cette fenêtre **est**, quand elle en est un.
    pub in_src_ui: Option<&'static str>,
    /// Ses panneaux, quand elle n'est pas un écran de `src/ui` — vide sinon.
    own_fields: &'static [Field],
    /// Ses boutons, dans le même cas.
    own_actions: &'static [Action],
}

impl Window {
    /// Les panneaux : ceux de `src/ui` quand c'est un de ses écrans, les siens sinon.
    pub fn fields(&self) -> &'static [Field] {
        match self.in_src_ui {
            // La **même** tranche, pas une copie : c'est ce qu'un test compare par
            // l'adresse.
            Some(id) => crate::ui::table::screen_by_id(id).map(|screen| screen.fields).unwrap_or(&[]),
            None => self.own_fields,
        }
    }

    /// Les boutons, même règle.
    pub fn actions(&self) -> &'static [Action] {
        match self.in_src_ui {
            Some(id) => crate::ui::table::screen_by_id(id).map(|screen| screen.actions).unwrap_or(&[]),
            None => self.own_actions,
        }
    }

    /// Le panneau avec cet id.
    pub fn field(&self, id: &str) -> Option<Field> {
        self.fields().iter().find(|field| field.id == id).copied()
    }

    /// Le bouton avec cet id.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions().iter().find(|action| action.id == id).copied()
    }

    /// Vrai quand un bouton au moins porte un refus.
    pub fn guards_something(&self) -> bool {
        self.actions().iter().any(Action::is_guarded)
    }

    /// Tous les refus de ses boutons, dans l'ordre des boutons.
    pub fn refusals(&self) -> Vec<UiRefusal> {
        let mut refusals = Vec::new();
        for action in self.actions() {
            for refusal in action.refusals {
                if !refusals.contains(refusal) {
                    refusals.push(*refusal);
                }
            }
        }
        refusals
    }
}

/// Le premier : la hutte à module de travailleur, **déjà décrite par `src/ui`**.
const WORKER: Window = Window {
    id: worker::SCREEN,
    java: worker::JAVA,
    resource: worker::RESOURCE,
    header: Header::SrcUi(worker::SCREEN),
    // `AbstractBuildingView.getWindow()` : 43 sortes de huttes répondent cet écran.
    openers: &["AbstractBuildingView.getWindow()"],
    in_src_ui: Some(worker::SCREEN),
    own_fields: &[],
    own_actions: &[],
};

/// L'onglet des ressources du builder, **déjà décrit par `src/ui`**.
const BUILDER: Window = Window {
    id: builder::SCREEN,
    java: builder::JAVA,
    resource: builder::RESOURCE,
    header: Header::SrcUi(builder::SCREEN),
    // Une vue de module dont `isPageVisible()` est vrai dessine un onglet
    // (`AbstractBuildingWindow.java:67-83`), et le bouton appelle `view.getWindow()`.
    openers: &["AbstractBuildingWindow (onglet de module)"],
    in_src_ui: Some(builder::SCREEN),
    own_fields: &[],
    own_actions: &[],
};

/// La carte de colonie : dans aucune table, et ouverte par trois portes.
const MAP: Window = Window {
    id: map::ID,
    java: map::JAVA,
    resource: map::RESOURCE,
    header: Header::None,
    openers: &["ItemColonyMap", "WindowMainPage", "WindowColonyPrestigeRanking"],
    in_src_ui: None,
    own_fields: map::FIELDS,
    own_actions: map::ACTIONS,
};

/// Le classement de prestige : dans aucune table, et ouvert par la carte seule.
const PRESTIGE: Window = Window {
    id: prestige::ID,
    java: prestige::JAVA,
    resource: prestige::RESOURCE,
    // La classe ne touche jamais le panneau `title` : c'est un libellé de XML.
    header: Header::Xml(prestige::TITLE_KEY),
    openers: &["WindowColonyMap"],
    in_src_ui: None,
    own_fields: prestige::FIELDS,
    own_actions: prestige::ACTIONS,
};

/// Les quatre fenêtres de cette tranche.
pub const WINDOWS: &[Window] = &[WORKER, BUILDER, MAP, PRESTIGE];

/// La fenêtre avec cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// Vrai quand cette tranche décrit cette classe Java.
pub fn is_described(java: &str) -> bool {
    WINDOWS.iter().any(|window| window.java == java)
}

/// Le titre de `src/ui` qu'un écran porte, quand cette fenêtre en est un.
///
/// Sert à une seule chose : prouver que [`Header::SrcUi`] et `src/ui::Screen::title` ne
/// peuvent pas diverger.
pub fn src_ui_title(id: &str) -> Option<Title> {
    crate::ui::table::screen_by_id(id).ok().map(|screen| screen.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    #[test]
    fn four_windows_two_of_which_are_already_screens_of_src_ui() {
        assert_eq!(WINDOWS.len(), 4);
        let in_src_ui: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.in_src_ui.is_some())
            .map(|window| window.id)
            .collect();
        assert_eq!(in_src_ui, [HUT_WORKER_ID, TAB_RESOURCES_ID]);
        // Les deux autres n'ont pas de ligne dans `src/ui` : il les refuse par leur nom.
        for id in [map::ID, prestige::ID] {
            assert_eq!(
                crate::ui::screen_by_id(id).err(),
                Some(crate::ui::Refusal::UnknownScreen),
                "{id} "
            );
            assert!(window_by_id(id).is_ok());
        }
        // Chaque fenêtre a son id, sa classe et son XML, et aucun doublon.
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(!window.java.is_empty(), "{} n'a pas de classe", window.id);
            assert!(window.resource.starts_with("minecolonies:gui/"), "{}", window.id);
            assert!(window.resource.ends_with(".xml"));
            assert!(!window.openers.is_empty(), "{} n'a pas d'ouvreur", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id);
                assert_ne!(window.java, other.java);
            }
        }
        assert!(is_described("WindowColonyMap"));
        assert!(!is_described("WindowMainPage"));
    }

    #[test]
    fn the_two_screens_of_src_ui_lend_their_panes_instead_of_copying_them() {
        for window in WINDOWS.iter().filter(|window| window.in_src_ui.is_some()) {
            let id = window.in_src_ui.unwrap_or("");
            let screen = crate::ui::table::screen_by_id(id).expect("un écran de src/ui");
            // La même adresse : aucune copie ne peut donc diverger.
            assert!(std::ptr::eq(window.fields().as_ptr(), screen.fields.as_ptr()));
            assert!(std::ptr::eq(window.actions().as_ptr(), screen.actions.as_ptr()));
            // Et les tranches propres restent vides, pour que la règle soit lisible.
            assert!(window.own_fields.is_empty());
            assert!(window.own_actions.is_empty());
        }
        // Les deux autres portent les leurs, et elles ne sont pas vides.
        for window in WINDOWS.iter().filter(|window| window.in_src_ui.is_none()) {
            assert!(!window.own_fields.is_empty(), "{} ne montre rien", window.id);
            assert!(std::ptr::eq(window.fields().as_ptr(), window.own_fields.as_ptr()));
        }
    }

    #[test]
    fn the_header_of_a_src_ui_screen_is_src_uis_own_title() {
        for window in WINDOWS {
            match window.header {
                Header::SrcUi(id) => {
                    assert_eq!(window.in_src_ui, Some(id));
                    let title = src_ui_title(id).expect("le titre de src/ui");
                    // La hutte du travailleur : nom + niveau, donc `BuildingName`.
                    if id == HUT_WORKER_ID {
                        assert_eq!(title, Title::BuildingName);
                    }
                    // L'onglet : un libellé fixe, et c'est celui de `src/ui`.
                    if id == TAB_RESOURCES_ID {
                        assert!(matches!(title, Title::Fixed(_)), "l'onglet a un titre fixe");
                    }
                }
                Header::Xml(key) => {
                    assert!(key.starts_with("com.minecolonies."), "{key} n'est pas une clé");
                    assert!(window.in_src_ui.is_none());
                }
                Header::None => assert!(window.in_src_ui.is_none()),
                Header::BuildingName => {
                    panic!("{} écrit le titre lui-même : ce n'est pas une de ces quatre", window.id)
                }
            }
        }
        // Le classement écrit le sien dans l'XML et ne le réécrit jamais.
        assert_eq!(
            window_by_id(prestige::ID).map(|window| window.header),
            Ok(Header::Xml(prestige::TITLE_KEY))
        );
        assert_eq!(window_by_id(map::ID).map(|window| window.header), Ok(Header::None));
    }

    #[test]
    fn nineteen_panes_and_eighteen_buttons_and_only_four_guard_anything() {
        let mut panes = 0;
        let mut buttons = 0;
        let mut guarded = 0;
        for window in WINDOWS {
            panes += window.fields().len();
            buttons += window.actions().len();
            guarded += window.actions().iter().filter(|action| action.is_guarded()).count();
        }
        assert_eq!(panes, 19, "5 (worker) + 11 (onglet) + 1 (carte) + 2 (classement)");
        assert_eq!(buttons, 18, "10 + 1 + 4 + 3");
        // Les quatre boutons qui refusent sont tous dans `src/ui` : la carte et le
        // classement ne refusent aucun clic.
        assert_eq!(guarded, 4);
        for window in WINDOWS.iter().filter(|window| window.in_src_ui.is_none()) {
            assert!(!window.guards_something(), "{} garde un bouton", window.id);
            assert!(window.refusals().is_empty());
        }
        let worker = window_by_id(worker::SCREEN).expect("hut.worker");
        assert_eq!(worker.refusals().len(), 3, "hire, recall, forcePickup");
        assert_eq!(worker.refusals()[0], crate::ui::Refusal::NotBuilt);
        let builder = window_by_id(builder::SCREEN).expect("tab.resources");
        assert_eq!(builder.refusals(), [crate::ui::Refusal::ResourceMissing, crate::ui::Refusal::ResourceNotNeeded]);
    }

    #[test]
    fn every_pane_of_these_four_windows_is_known_by_src_ui_or_by_this_table() {
        for window in WINDOWS {
            for field in window.fields() {
                // Un panneau de ligne nomme une liste de la même fenêtre.
                if let crate::ui::field::Scope::Row(list) = field.scope {
                    assert!(
                        window.field(list).is_some_and(|pane| pane.value == Value::Row),
                        "{}.{} est une ligne de {list}, qui n'est pas une liste",
                        window.id,
                        field.id
                    );
                }
                assert_eq!(window.field(field.id).map(|pane| pane.id), Some(field.id));
            }
            for action in window.actions() {
                assert_eq!(window.action(action.id).map(|found| found.id), Some(action.id));
            }
        }
        // Les ids de panneau de deux fenêtres différentes ne se mélangent pas : `name`
        // est un titre dans la hutte et un nom de ligne dans la carte de la hutte.
        assert!(window_by_id(worker::SCREEN).ok().and_then(|window| window.field("name")).is_some());
        assert!(window_by_id(builder::SCREEN).ok().and_then(|window| window.field("name")).is_none());
    }

    #[test]
    fn the_map_and_the_ranking_open_each_other_and_nothing_outside_this_table() {
        let map = window_by_id(map::ID).expect("la carte");
        let prestige = window_by_id(prestige::ID).expect("le classement");
        // La carte ouvre le classement, et le classement ouvre la carte : deux arêtes,
        // et les deux se résolvent ici puisque `src/ui` ne les connaît pas.
        let opens: Vec<&str> = map
            .actions()
            .iter()
            .filter_map(|action| match action.effect {
                crate::ui::action::Effect::Opens(target) => Some(target),
                _ => None,
            })
            .collect();
        assert_eq!(opens, [prestige.id, prestige.id]);
        for target in opens {
            assert!(window_by_id(target).is_ok(), "{target} n'existe pas ici");
        }
        let back = prestige
            .action(prestige::MAP)
            .and_then(|action| match action.effect {
                crate::ui::action::Effect::Opens(target) => Some(target),
                _ => None,
            });
        assert_eq!(back, Some(map.id));
        // L'ouvreur est un point d'entrée, pas un bouton : le classement n'est ouvert que
        // par la carte, et la carte par trois choses dont une seule est une fenêtre.
        assert_eq!(prestige.openers.len(), 1, "la carte est la seule porte du classement");
        assert_eq!(prestige.openers.first(), Some(&"WindowColonyMap"));
        assert_eq!(map.openers.len(), 3);
        assert!(map.openers.contains(&"ItemColonyMap"));
    }

    #[test]
    fn an_unknown_id_is_refused_by_name_and_not_by_a_guess() {
        assert_eq!(window_by_id("map.colony").map(|window| window.id), Ok(map::ID));
        let unknown = window_by_id("hut.worker");
        assert!(unknown.is_ok(), "hut.worker est décrit ici aussi");
        assert_eq!(window_by_id("map").err(), Some(Refusal::UnknownWindow));
        assert_eq!(Refusal::UnknownWindow.key(), None);
        assert_eq!(Refusal::UnknownWindow.name(), "map.refusal.unknown_window");
        assert_eq!(window_by_id("").err(), Some(Refusal::UnknownWindow));
    }
}
