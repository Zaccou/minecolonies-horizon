//! Les quatre fenêtres que ni `src/ui` ni `src/ui_window_02` ne décrivent : la
//! réactivation d'un bâtiment, le détail d'une requête, la liste des ressources du
//! builder, et l'arbre de recherche.
//!
//! **Même forme que `src/ui`, pas une deuxième.** Les panneaux sont des
//! [`crate::ui::Field`] et les libellés/effets des [`crate::ui::action::Effect`] :
//! `Value`, `Scope`, `Source`, `Stock`, `Progress` et `percent` sont **les siens**,
//! et un test le prouve sur la ligne des ressources. Ce qui est propre à ce module
//! est ce qu'aucune des deux tables n'a : [`Button::leaves`], « où le clic laisse la
//! fenêtre », parce que `Effect` ne sait pas dire *envoie puis se ferme*.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`refusal`] | pourquoi une lecture ou un clic est refusé, nommé |
//! | [`reactivate`] | `WindowReactivateBuilding` : deux boutons, deux messages |
//! | [`request`] | `WindowRequestDetail` : huit panneaux, trois boutons dont un mort |
//! | [`resources`] | `WindowResourceList` : cinq états de ressource, aucun bouton |
//! | [`research`] | `WindowResearchTree` : dix états de tuile, trois cliquables |
//!
//! # Les comptes, mesurés
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | 4 | [`WINDOWS`] |
//! | boutons au total | **6** | [`WINDOWS`] |
//! | boutons qui ne font rien | **1** | `request::BACK` |
//! | fenêtres sans aucun bouton | 1 | `resources::WINDOW` |
//! | classes de message nommées | **5** | [`reactivate`], [`request`], [`research`], [`resources`] |
//! | panneaux statiques | **23** | [`WINDOWS`] (1 + 7 + 14 + 1) |
//! | états de ressource | 5 | `resources::Availability` |
//! | états de tuile de recherche | 10 déclarés, **8 atteignables** | `research::ButtonState` |
//!
//! # Cinq prémisses que le source contredit
//!
//! 1. **« `WindowResourceList` est l'onglet des ressources du builder. »** Non :
//!    l'onglet est `WindowBuilderResModule` (`src/ui::TAB_RESOURCES_ID`), et cette
//!    fenêtre-ci s'ouvre avec l'**item** rouleau de ressources
//!    (`core/items/ItemResourceScroll.java:100`). Les deux lisent le même module
//!    (`buildingresources`), ce qui est le vrai lien entre elles.
//! 2. **« Le bouton `back` revient en arrière. »** Il est dessiné
//!    (`windowrequestdetail.xml:19-20`), enregistré nulle part :
//!    `WindowRequestDetail.onButtonClicked` ne teste que `fulfill` et `cancel`
//!    (`:192-203`) et n'appelle même pas `super`. Le clic ne fait **rien**.
//! 3. **« Cliquer une recherche en cours l'annule. »** Le premier clic ne fait
//!    qu'**ajouter** un bouton d'annulation sur la tuile (`:221-226`,
//!    `:752-766`) ; c'est le clic sur ce second bouton, `undo:<id>`, qui envoie
//!    `TryResearchMessage` (`:192`).
//! 4. **« Les dix états de tuile servent. »** `getResearchButtonState` (`:376-421`)
//!    n'en rend jamais deux : `LOCKED` et `TOO_MANY_PROGRESS` sont déclarés
//!    (`:101`, `:99`) et morts. Et `TOO_MANY_PROGRESS` est le seul état qui écrirait
//!    les deux libellés `toomanyinprogress` (`:532-544`), donc ces deux clés ne
//!    s'affichent jamais.
//! 5. **« La ligne d'entrepôt écrit son montant dans son propre panneau. »** Non :
//!    `WindowResourceList.java:230` écrit `IN_DELIVERY_AMOUNT`, alors que `:217`
//!    vient de vider `IN_WAREHOUSE_AMOUNT`. L'icône est celle de l'entrepôt, le
//!    nombre est celui du panneau de livraison — vert au lieu de jaune, à la même
//!    position. Le port reproduit ce comportement et le nomme
//!    (`resources::Mark::Warehouse`), il ne le corrige pas.
//!
//! # Ce qui n'est pas ici
//!
//! Le **dessin** : `pos`, `size`, sprites, et les `Gradient` de l'arbre de recherche
//! (`:571-591`). Les **onze couleurs** de `ResearchConstants.java:118-123` : elles
//! ne décident d'aucune branche, `resources::Availability` les remplace par ce
//! qu'elles veulent dire. Et **l'envoi** : cinq classes de message sont nommées,
//! aucune n'est envoyée — `MANQUANT.md` §1 dit ce qui manque, et §4 dit la règle de
//! recherche que le port ne peut pas honorer.

pub mod reactivate;
pub mod refusal;
pub mod request;
pub mod research;
pub mod resources;

pub use refusal::Refusal;

use crate::ui::action::Effect;
use crate::ui::field::{Field, Value};

/// Ce que l'XML écrit en haut de la fenêtre, quand il y a quelque chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three shapes in these four windows, and a fourth would be a new one
pub enum Header {
    /// Aucun titre : `windowreactivatebuilding.xml` et `windowresearch.xml` n'en
    /// dessinent pas.
    None,
    /// Un libellé de l'XML, sans id, que la classe ne réécrit jamais
    /// (`windowrequestdetail.xml:7-8`, `windowresourcescroll.xml:10-11`).
    Xml(&'static str),
    /// La classe en écrit **un par profondeur**, de 1 à `MAX_DEPTH`
    /// (`WindowResearchTree.java:328-365`) : six libellés, aucun dans l'XML.
    Tiers(&'static str),
}

/// Qui ouvre la fenêtre — le point d'entrée, pas le bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two kinds of opener in these four windows
pub enum Opener {
    /// Un écran que ce port décrit déjà, par son id de table.
    Window(&'static str),
    /// Une classe ou un item de Java qu'aucune des trois tables ne décrit.
    Java(&'static str),
}

/// Où un clic laisse la fenêtre, ce que [`Effect`] ne sait pas dire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four ends for a click, and `Nothing` is one of them on purpose
pub enum Leaves {
    /// Rien ne change d'écran : le clic ne fait qu'agir sur la fenêtre
    /// (`WindowResearchTree.java:141-156`).
    Stays,
    /// La fenêtre se ferme après coup (`WindowReactivateBuilding.java:62`).
    Closes,
    /// La fenêtre qui l'a ouverte se rouvre (`WindowResearchTree.java:251`) —
    /// c'est le mot de `src/ui::action::Effect::Reopens`, tenu à part parce qu'il
    /// s'ajoute à un envoi.
    Reopens,
    /// Le bouton n'a **aucun** gestionnaire : le clic est avalé
    /// (`WindowRequestDetail.java:191-204`).
    Nothing,
}

/// Un bouton : l'id de l'XML, son libellé, son effet, où il laisse la fenêtre, et
/// ses refus.
///
/// `src/ui::action::Action` porte les quatre premiers champs mais son `refusals` est
/// un `&[ui::Refusal]` — une énumération fermée que ce module ne peut pas étendre —
/// et il n'a pas de [`Leaves`]. Les deux refus de ces fenêtres que `src/ui` ne nomme
/// pas (`refusal::NotCancellable`, `refusal::InvalidId`) n'auraient nulle part où
/// aller, d'où cette structure plutôt qu'un second `Action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id que l'XML donne et que `registerButton` lie
    /// (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// La clé du libellé, `""` pour une image nue.
    pub label: &'static str,
    /// Ce que le clic demande — le type de `src/ui`, pas un second. `None` quand il ne
    /// demande **rien** : une fermeture (`WindowReactivateBuilding.java:69-72`), et
    /// `Effect` n'a aucune variante qui sache le dire. Voir `MANQUANT.md` §2.
    pub effect: Option<Effect>,
    /// Où le clic laisse la fenêtre.
    pub leaves: Leaves,
    /// Chaque façon dont le clic est refusé, dans l'ordre où Java teste.
    pub refusals: &'static [Refusal],
}

impl Button {
    /// Un bouton que Java ne refuse jamais côté fenêtre.
    pub const fn plain(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        leaves: Leaves,
    ) -> Button {
        Button { id, label, effect: Some(effect), leaves, refusals: &[] }
    }

    /// Un bouton qui ne demande rien : il ne fait que laisser la fenêtre là où
    /// [`Leaves`] dit.
    pub const fn dismiss(id: &'static str, label: &'static str, leaves: Leaves) -> Button {
        Button { id, label, effect: None, leaves, refusals: &[] }
    }

    /// Un bouton que Java refuse dans au moins un état.
    pub const fn guarded(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        leaves: Leaves,
        refusals: &'static [Refusal],
    ) -> Button {
        Button { id, label, effect: Some(effect), leaves, refusals }
    }

    /// Vrai quand Java peut montrer le bouton sans effet.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// La classe du message envoyé, quand le clic en envoie un.
    pub fn sends(&self) -> Option<&'static str> {
        match self.effect {
            Some(Effect::Sends(class)) => Some(class),
            _ => None,
        }
    }

    /// Vrai quand le bouton est dessiné et ne fait rien — le cas de `back`.
    pub fn is_dead(&self) -> bool {
        self.leaves == Leaves::Nothing
    }
}

/// Une fenêtre de cette table : sa classe, son XML, son en-tête, ses panneaux, ses
/// boutons, et qui l'ouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. **Pas celui de Java**, qui n'en a pas : une fenêtre
    /// s'ouvre par sa classe ou par un item.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    pub header: Header,
    pub opener: Opener,
    pub panes: &'static [Field],
    pub buttons: &'static [Button],
}

impl Window {
    /// Le panneau de cet id, copié ; le **premier**, puisqu'un id peut revenir.
    pub fn pane(&self, id: &str) -> Option<Field> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copié.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Vrai quand la fenêtre montre au moins un panneau de ce genre.
    pub fn shows(&self, value: Value) -> bool {
        self.panes.iter().any(|pane| pane.value == value)
    }

    /// Les boutons qui ne font rien, dans l'ordre de la fenêtre.
    pub fn dead_buttons(&self) -> Vec<&'static str> {
        self.buttons.iter().filter(|button| button.is_dead()).map(|button| button.id).collect()
    }
}

/// Les quatre fenêtres de ce module.
pub static WINDOWS: &[&Window] = &[
    &reactivate::WINDOW,
    &request::WINDOW,
    &resources::WINDOW,
    &research::WINDOW,
];

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// C'est la jointure inverse : `src/ui_window_02::clipboard` nomme
/// `WindowRequestDetail` par sa classe (`clipboard.rs:76`) sans la décrire, et cette
/// fonction répond la ligne qui décrit ce que ce détail demande.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::{Scope, Source};

    #[test]
    fn the_four_windows_name_their_class_their_xml_and_their_header() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(window.java.starts_with("Window"), "{} n'a pas de classe", window.id);
            assert!(
                window.resource.starts_with("minecolonies:gui/") && window.resource.ends_with(".xml"),
                "{} lit {}",
                window.id,
                window.resource
            );
            assert!(!window.panes.is_empty(), "{} ne montre rien", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenêtres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
            }
        }
        // Deux en-têtes viennent de l'XML, un est écrit par la classe, un manque.
        let headers: Vec<&str> = WINDOWS
            .iter()
            .map(|window| match window.header {
                Header::None => "none",
                Header::Xml(_) => "xml",
                Header::Tiers(_) => "tiers",
            })
            .collect();
        assert_eq!(headers, ["none", "xml", "xml", "tiers"]);
        assert_eq!(
            research::WINDOW.header,
            Header::Tiers(research::TIER_KEY),
            "WindowResearchTree.java:344"
        );
    }

    #[test]
    fn src_ui_window_02_names_the_detail_window_and_this_table_answers_it() {
        // La jointure telle que `src/ui_window_02` la publie, sans la décrire.
        let class = crate::ui_window_02::clipboard::DETAIL_WINDOW;
        let window = match window_by_java(class) {
            Ok(window) => window,
            Err(refusal) => panic!("{class} n'est pas décrit : {refusal}"),
        };
        assert_eq!(window.id, request::ID);
        assert_eq!(window.opener, Opener::Window(crate::ui_window_02::clipboard::ID));
        // La réciproque : les trois autres fenêtres de `src/ui_window_02` ne sont pas ici.
        for other in ["WindowBuildBuilding", "WindowClipBoard", "WindowBuildingBrowser"] {
            assert_eq!(window_by_java(other), Err(Refusal::UnknownWindow), "{other}");
        }
        assert_eq!(window_by_id("hire"), Err(Refusal::UnknownWindow), "les 13 écrans de src/ui");
    }

    #[test]
    fn both_resource_screens_read_the_same_module_and_are_two_windows() {
        // `src/ui::table::MODULE_SCREENS` envoie la clé vers l'onglet du builder.
        let tab = crate::ui::table::module_screen(resources::MODULE).map(|screen| screen.id);
        assert_eq!(tab, Some(crate::ui::screen::TAB_RESOURCES_ID));
        // La fenêtre de ce module lit la même clé et n'est pas cet onglet.
        let scroll = resources::WINDOW;
        assert_eq!(scroll.pane(resources::LIST).map(|pane| pane.source), Some(Source::Module(resources::MODULE)));
        assert_ne!(scroll.id, crate::ui::screen::TAB_RESOURCES_ID);
        assert!(!crate::ui::SCREENS.iter().any(|screen| screen.id == scroll.id));
        // L'onglet a un bouton, le rouleau aucun : c'est la différence, pas le dessin.
        assert_eq!(scroll.buttons.len(), 0);
        assert!(crate::ui::table::module_screen(resources::MODULE).is_some_and(|s| !s.actions.is_empty()));
    }

    #[test]
    fn the_out_turn_value_and_the_percent_are_src_uis_not_copies() {
        // Un panneau d'ici est un `src/ui::Field`, et sa valeur est un `Value` de
        // `src/ui` : les deux formes s'emboîtent sans conversion.
        let stock = resources::WINDOW.pane(resources::AVAILABLE_NEEDED);
        assert_eq!(stock.map(|pane| pane.value), Some(Value::Stock));
        assert!(Value::Stock.is_computed());
        assert_eq!(resources::stock_text(12, 64), crate::ui::field::stock_text(12, 64));
        assert_eq!(resources::progress_percent(1, 3), crate::ui::field::percent(1, 3));
        // Et la clé de l'en-tête de progression est celle que `src/ui` déclare.
        assert_eq!(
            resources::PROGRESS_KEY,
            crate::ui::field::PROGRESS_KEY,
            "WindowResourceList.java:126"
        );
        // La ligne « disponible / demandé » n'a **pas** de libellé : l'XML ne lui en
        // donne aucun (`windowresourcescroll.xml:34`) et la classe n'écrit qu'un texte
        // (`:269`). Le panneau existe, son libellé est vide — deux choses différentes,
        // et l'ancienne assertion comparait un `Option<Option<&str>>` à `None`.
        assert!(stock.is_some());
        assert_eq!(stock.and_then(|pane| pane.label), None);
    }

    #[test]
    fn every_row_pane_names_a_list_of_its_own_window() {
        for window in WINDOWS {
            for (index, pane) in window.panes.iter().enumerate() {
                let repeated = window.panes[..index]
                    .iter()
                    .any(|other| other.id == pane.id && other.scope == pane.scope);
                assert!(!repeated, "{} répète {} dans la même portée", window.id, pane.id);
                if let Scope::Row(list) = pane.scope {
                    assert!(
                        window.panes.iter().any(|other| other.id == list && other.value == Value::Row),
                        "{}: {} est une ligne de {list}, qui n'est pas une liste",
                        window.id,
                        pane.id
                    );
                }
            }
        }
    }

    #[test]
    fn one_button_of_the_six_is_drawn_and_does_nothing() {
        let total: usize = WINDOWS.iter().map(|window| window.buttons.len()).sum();
        assert_eq!(total, 6, "2 + 3 + 0 + 1");
        let dead: Vec<&str> = WINDOWS
            .iter()
            .flat_map(|window| window.dead_buttons().into_iter())
            .collect();
        assert_eq!(dead, [request::BACK], "windowrequestdetail.xml:19-20");
        // Le bouton mort n'envoie rien et n'a aucun refus : il est simplement muet.
        let back = match request::WINDOW.button(request::BACK) {
            Some(button) => button,
            None => panic!("le bouton back a disparu"),
        };
        assert_eq!(back.sends(), None);
        assert_eq!(back.leaves, Leaves::Nothing);
        assert!(!back.is_guarded());
        // Et son id est une vraie clé de `WindowConstants`, pas une invention.
        assert_eq!(back.id, "back");
    }

    #[test]
    fn three_messages_leave_by_a_button_and_two_by_a_constant() {
        // Les vingt-trois panneaux statiques des quatre fenêtres.
        let panes: usize = WINDOWS.iter().map(|window| window.panes.len()).sum();
        assert_eq!(panes, 23, "1 reactivate + 7 request + 14 resources + 1 research");
        // Cinq classes de message nommées : trois partent d'un bouton…
        let mut classes: Vec<&str> =
            WINDOWS.iter().flat_map(|window| window.buttons.iter()).filter_map(|b| b.sends()).collect();
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(
            classes,
            [
                reactivate::MESSAGE,
                request::FULFILL_MESSAGE,
                request::CANCEL_MESSAGE
            ],
            "l'ordre du tri, pas celui du source"
        );
        // …et deux ne sont pas des clics : le rafraîchissement de la liste et le
        // message des tuiles de recherche, qui n'ont pas d'id statique.
        assert_ne!(research::SEND_MESSAGE, resources::REFRESH_MESSAGE);
        assert!(!classes.contains(&research::SEND_MESSAGE));
        assert!(!classes.contains(&resources::REFRESH_MESSAGE));
    }

    #[test]
    fn a_click_that_closes_the_window_still_says_what_it_asked_for() {
        for window in WINDOWS {
            for button in window.buttons {
                match button.leaves {
                    Leaves::Closes => assert!(
                        button.sends().is_some() || button.effect.is_none(),
                        "{}.{} se ferme en demandant quelque chose qui n'est pas un message",
                        window.id,
                        button.id
                    ),
                    Leaves::Reopens => assert_eq!(button.sends(), None, "{}", button.id),
                    Leaves::Nothing => {
                        assert_eq!(button.sends(), None);
                        assert_eq!(button.effect, None);
                    }
                    // Un bouton qui laisse la fenêtre ouverte doit envoyer quelque
                    // chose : aucun des trois ne se contente de ne rien faire.
                    Leaves::Stays => assert!(button.sends().is_some(), "{} ne fait rien", button.id),
                }
            }
            // Un id de bouton est unique dans sa fenêtre.
            for (index, button) in window.buttons.iter().enumerate() {
                for other in &window.buttons[index + 1..] {
                    assert_ne!(button.id, other.id, "{}: deux boutons {}", window.id, button.id);
                }
            }
        }
        // Une seule fermeture ne demande rien, et c'est la croix du bâtiment.
        let silent: Vec<&str> = WINDOWS
            .iter()
            .flat_map(|window| window.buttons.iter())
            .filter(|button| button.leaves == Leaves::Closes && button.effect.is_none())
            .map(|button| button.id)
            .collect();
        assert_eq!(silent, [reactivate::CANCEL]);
    }
}
