//! Les quatre fenêtres que `src/ui` **nomme sans les décrire** : la donnée qu'elles
//! demandent, et ce qu'un clic y fait.
//!
//! `src/ui/mod.rs` s'arrête à treize écrans de huttes et laisse ceux-là au bord, nommés par
//! classe dans `Effect::OpensJava` : « `WindowInfo`, `WindowBuildBuilding`,
//! `WindowHutGuide`'s spy list and the build tool are named by class and not described ».
//! Ce module décrit `WindowBuildBuilding` et les trois qui vont avec. Pas le dessin, pour
//! la même raison que `src/ui` : un `pos` et une couleur appartiennent au XML.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`refusal`] | pourquoi une lecture ou un clic est refusé, nommé |
//! | [`pane`] | un panneau, une liste déroulante, un bouton, et la source d'une valeur |
//! | [`build`] | `WindowBuildBuilding` + `WindowBuildDecoration` : deux fenêtres, une règle |
//! | [`browser`] | `WindowBuildingBrowser` : le catalogue de styles, sans un bouton |
//! | [`clipboard`] | `WindowClipBoard` : les requêtes ouvertes, et un drapeau inversé |
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | 4 | [`WINDOWS`] |
//! | qui partagent **un seul** XML | 2 | `WindowBuildDecoration.java:116` |
//! | boutons de la fenêtre du bâtiment | 7 | `windowbuildbuilding.xml:5-58` |
//! | boutons de la décoration | 2 | `WindowBuildDecoration.java:120-121` |
//! | boutons du navigateur | **0** | `WindowBuildingBrowser.java:63-141` |
//! | panneaux du navigateur | 8 | `windowbrowsebuilding.xml:10-21` |
//! | classes de message nommées | 5, plus le paramètre de la décoration | [`build`], [`clipboard`] |
//! | refus | 15, dont 1 qui reprend `src/ui` | [`refusal::Refusal`] |
//!
//! # Quatre prémisses que le source contredit
//!
//! 1. **« `WindowBuildBuilding` est un écran de hutte. »** C'est un écran **de blueprint** :
//!    il ne lit aucun *getter* de module, il calcule ses ressources en ouvrant le fichier du
//!    niveau suivant (`WindowBuildBuilding.java:298-355`) et trie les builders par distance
//!    (`:254`). D'où son `Effect::OpensJava` dans `src/ui` (`ui/action.rs:436-441`).
//! 2. **« La décoration est une autre fenêtre. »** Même XML (`:116`), cinq boutons de moins
//!    (`:124-130`) : une [`build::State`], un [`build::Flavour`], une règle.
//! 3. **« Le navigateur liste les bâtiments du joueur. »** Aucun bouton, aucune donnée du
//!    serveur : il lit les packs installés côté client
//!    (`StructurePacks.getPackMetas()`, `WindowBuildingBrowser.java:247`, `:303`). Son seul
//!    état est une barre et un cache par bloc (`:219-228`).
//! 4. **« `hideunimportant` cache les requêtes sans importance. »** Il les **montre** : `1`
//!    est écrit quand la liste n'est pas filtrée (`WindowClipBoard.java:74`) et le filtre
//!    ne s'applique que sur la valeur fausse (`:154-168`). [`clipboard::Filter`] nomme les
//!    deux états d'après ce que la liste montre, et la valeur sur disque ne change pas.
//!
//! # Ce qui n'est pas ici
//!
//! Le **dessin** (positions, tailles, sprites) et les **quatre couleurs** du navigateur
//! (`:53-56`, `:182-183`) : ce que le port garde, c'est la classification `parent` ×
//! `invisible` qui choisit la branche. Et **l'envoi** : les cinq classes de message sont
//! nommées, aucune n'est envoyée — `MANQUANT.md` §3 dit ce qui manque.

pub mod browser;
pub mod build;
pub mod clipboard;
pub mod pane;
pub mod refusal;

pub use browser::Row as BrowserRow;
pub use clipboard::{Entry, Filter, Row as RequestRow};
pub use pane::{Action, Dropdown, Exit, Label, Message, Pane, Press, Source};
pub use refusal::Refusal;

/// Une fenêtre de cette table : sa classe, son XML, ses panneaux, ses boutons.
///
/// C'est la question de `src/ui::screen::Screen` (id, classe, ressource, panneaux, boutons)
/// avec deux champs de plus — [`Window::layout`], le module de liste que la clipboard monte,
/// et [`Window::dropdowns`], qui ne sont pas des panneaux à valeur — et deux de moins : ces
/// fenêtres n'écrivent pas de titre et n'ont pas de porte (seul le guide du builder en a une).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. **Pas celui de Java**, qui n'en a pas : une fenêtre s'ouvre par
    /// sa classe.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// Le module de mise en page monté, quand il y en a un (`WindowClipBoard.java:55`).
    pub layout: Option<&'static str>,
    pub panes: &'static [Pane],
    /// Les ids des listes déroulantes.
    pub dropdowns: &'static [&'static str],
    pub actions: &'static [Action],
}

impl Window {
    /// Le panneau de cet id, copié ; le **premier**, puisqu'un id peut revenir.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copié.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions.iter().find(|action| action.id == id).copied()
    }

    /// Vrai quand la fenêtre montre au moins un panneau de ce genre.
    pub fn shows(&self, value: crate::ui::Value) -> bool {
        self.panes.iter().any(|pane| pane.value == value)
    }

    /// Vrai quand la fenêtre monte un module de liste.
    pub fn has_layout(&self) -> bool {
        self.layout.is_some()
    }
}

/// Les quatre fenêtres de ce module.
pub static WINDOWS: &[&Window] =
    &[&build::BUILD_WINDOW, &build::DECORATION_WINDOW, &browser::WINDOW, &clipboard::WINDOW];

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// C'est la jointure avec `src/ui` : `ui::action::BuildLabel::effect` rend
/// `Effect::OpensJava("WindowBuildBuilding")`, et cette fonction répond la ligne qui décrit
/// ce que cette fenêtre demande.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::{Scope, Value};

    #[test]
    fn the_four_windows_each_name_their_class_their_xml_and_their_layout() {
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
        // Une seule fenêtre monte un module de liste, et c'est la clipboard.
        let with_layout: Vec<&str> =
            WINDOWS.iter().filter(|w| w.has_layout()).map(|w| w.id).collect();
        assert_eq!(with_layout, [clipboard::ID], "WindowClipBoard.java:55");
        // Les deux fenêtres de construction partagent un XML et restent deux lignes.
        assert_eq!(build::BUILD_WINDOW.resource, build::DECORATION_WINDOW.resource);
        assert_ne!(build::BUILD_WINDOW.java, build::DECORATION_WINDOW.java);
        // Le navigateur n'a aucun bouton : fait du source, pas oubli du port.
        assert_eq!(browser::WINDOW.actions.len(), 0, "WindowBuildingBrowser.java:63-141");
        assert_eq!(build::DECORATION_WINDOW.actions.len(), 2);
        assert_eq!(build::BUILD_WINDOW.actions.len(), 7);
    }

    #[test]
    fn src_ui_names_the_build_window_and_this_table_answers_with_its_data() {
        // La jointure telle que `src/ui` la publie (`ui/action.rs:436-441`).
        let class = match crate::ui::action::BuildLabel::BuildOrRepair.effect() {
            crate::ui::action::Effect::OpensJava(class) => class,
            other => panic!("le bouton de build de src/ui ouvre {other:?}"),
        };
        let window = match window_by_java(class) {
            Ok(window) => window,
            Err(refusal) => panic!("{class} n'est pas décrit : {refusal}"),
        };
        assert_eq!(window.id, build::ID);
        assert_eq!(window.java, "WindowBuildBuilding");
        // La réciproque : un refus nommé, pas un `None` muet.
        assert_eq!(window_by_java("WindowInfo"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowsBarracksSpies"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id("hire"), Err(Refusal::UnknownWindow), "les 13 écrans de src/ui");
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
                        window
                            .panes
                            .iter()
                            .any(|other| other.id == list && other.value == Value::Row),
                        "{}: {} est une ligne de {list}, qui n'est pas une liste",
                        window.id,
                        pane.id
                    );
                }
            }
            // Une liste déroulante n'est pas un panneau : aucun id ne vit deux fois.
            for dropdown in window.dropdowns {
                assert_eq!(window.pane(*dropdown), None, "{dropdown} est aussi un panneau");
            }
        }
        // Le navigateur est la seule fenêtre avec une barre de progression (`:135-139`).
        let with_progress: Vec<&str> =
            WINDOWS.iter().filter(|w| w.shows(Value::Progress)).map(|w| w.id).collect();
        assert_eq!(with_progress, [browser::ID]);
        assert!(build::BUILD_WINDOW.shows(Value::Count));
        assert!(!build::BUILD_WINDOW.shows(Value::Progress));
        // Le seul bouton de fenêtre de la clipboard se retrouve par son id.
        assert_eq!(clipboard::WINDOW.action(clipboard::TOGGLE).map(|a| a.id), Some("important"));
        assert_eq!(clipboard::WINDOW.action("absent"), None);
    }
}
