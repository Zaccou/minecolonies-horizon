//! Les trois fenetres de bout en bout : les requetes qu'un atelier peut prendre en
//! charge, ce qu'un batiment compte, et le journal de quetes avec ses trois pages.
//!
//! Java reference : `core/client/gui/modules/building/WindowSelectRequest.java` (207
//! lignes), `.../WindowStatsModule.java` (240), `core/client/gui/questlog/WindowQuestLog.java`
//! (164) et `.../WindowQuestLogAvailableQuestModule.java` (85) — les quatre sources du
//! brief — plus leurs trois XML et les classes qu'elles lisent.
//!
//! **Meme forme que `src/ui`, pas une deuxieme.** Un panneau est un
//! [`crate::ui::Field`] ([`Pane::to_ui`]), une valeur un [`crate::ui::Value`], une
//! portee un [`Scope`], un effet un [`crate::ui::action::Effect`]. Ce qui est propre a
//! ce module est ce qu'aucune table n'a : [`Pane::declared`] (le XML et la ligne qui
//! creent l'id), [`Source::QuestTemplates`] (le seul lecteur absent de `src/ui`),
//! [`Button::returns`] (ce que le clic rend a l'appelant) et [`Button::registered`] (qui
//! enregistre le clic).
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`pane`] | un panneau : ce qu'il porte, d'ou vient sa valeur, la ligne qui le declare |
//! | [`select_request`] | les requetes ouvertes, la marche dans les parents, la course du clic |
//! | [`stats`] | l'intervalle, le filtre qui est une photo, le libelle a trois formes |
//! | [`quest_log`] | trois pages, un localisateur qui ne vise pas toujours le donneur |
//! | [`refusal`] | huit refus en trois especes, dont quatre sont des plantages |
//!
//! Les comptes, mesures : **4 sources → 3 fenetres + 1 moteur de page**, 3 XML,
//! **17 panneaux** (5 + 2 + 10) dont 5 listes, **7 boutons** (2 + 1 + 2 fleches + 2
//! localisateurs), 1 liste deroulante de 4 etats, 3 pages, **0 message serveur**, 8 refus,
//! et 4 des 12 valeurs de `src/ui` employees (`Row`, `Icon`, `Text`, `Name`).
//!
//! # Les trois premisses du brief que le source contredit
//!
//! 1. **« les actions (embaucher, renvoyer, prioriser) »** ne sont sur **aucune** de ces
//!    sources : ce sont les boutons de l'ecran de la hutte de metier, que `src/ui` porte
//!    deja (`Action::guarded("hire", …)`, `A_RECALL`, `A_PRIO_UP`, `A_PRIO_DOWN`). Ici
//!    les sept boutons sont tous locaux : rendre la main, basculer un booleen, tourner
//!    une page, surligner un citoyen.
//! 2. **« nom, niveau, progression, stock »** ne sont pas ici non plus : aucun panneau
//!    n'est un [`crate::ui::Value::Level`], la seule « progression » est le texte
//!    d'objectif d'une quete en cours et le seul « stock » est le nombre d'une
//!    statistique.
//! 3. **`WindowQuestLogAvailableQuestModule` n'est pas une fenetre** : ni XML, ni titre,
//!    ni bouton — c'est un `WindowQuestLogQuestModule<IQuestInstance>` (`:24`) qui
//!    remplit les lignes d'une page. [`Window::pages`] est ou il vit, et
//!    `window_by_java` le refuse.
//!
//! Ce qui n'est pas ici : le **dessin** (ni position, ni taille, ni sprite — `TRANCHE.md`
//! §3 M2), l'**envoi** (aucun bouton n'envoie de message), le **gestionnaire de quetes**
//! (`MANQUANT.md` §3), et le **texte d'objectif** d'une quete en cours (`MANQUANT.md` §4).

pub mod pane;
pub mod quest_log;
pub mod refusal;
pub mod select_request;
pub mod stats;

pub use pane::{Pane, Source};
pub use refusal::Refusal;

use crate::ui::action::Effect;
use crate::ui::field::Scope;

/// L'id de la fenetre des requetes dans cette table : une fenetre d'atelier, pas un
/// ecran de hutte.
pub const SELECT_REQUEST_ID: &str = "craft.select_request";

/// L'id de l'onglet des statistiques ; il comble le trou de `src/ui::MODULE_SCREENS`,
/// qui ne mappe pas la cle `stats_module` (un test le constate).
pub const STATS_ID: &str = "tab.stats";

/// L'id du journal de quetes.
pub const QUEST_LOG_ID: &str = "questlog";

/// Les deux messages que **toute** fenetre BlockUI envoie, quel que soit son contenu :
/// le premier a la construction (`AbstractWindowSkeleton.java:96`), le second a chaque
/// clic enregistre (`:196`). Ce ne sont pas des messages de ces fenetres.
pub const GENERIC_TRIGGERS: [&str; 2] =
    ["OpenGuiWindowTriggerMessage", "ClickGuiButtonTriggerMessage"];

/// Qui enregistre le gestionnaire d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: la classe, ou la classe mere
pub enum By {
    /// La classe de la fenetre (`WindowSelectRequest.java:61-62`,
    /// `WindowStatsModule.java:89`, `WindowQuestLog.java:42`).
    Window,
    /// `AbstractWindowSkeleton`, qui cherche `prevPage` et `nextPage` des qu'il trouve
    /// une vue a onglets (`:77-88`).
    Base,
}

/// Ce que le clic rend a la fenetre qui l'a ouvert.
///
/// Une seule des trois fenetres rend quelque chose : `WindowSelectRequest` recoit un
/// `Consumer<IRequest<?>>` a sa construction (`:54-55`) et l'appelle avec la requete
/// choisie (`:136`) ou avec `null` (`:122`). C'est une sortie de fenetre que `Effect` ne
/// sait pas dire — `Effect::Reopens` dit qu'on revient, pas avec quoi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois retours, et l'absence explicite en est un
pub enum Returns {
    /// Rien n'est rendu.
    Nothing,
    /// La requete de la ligne cliquee (`WindowSelectRequest.java:136`).
    Request,
    /// **Rien**, explicitement : l'appelant s'en sert pour effacer la requete courante
    /// et n'afficher aucune recette (`WindowCrafting.java:212-221`).
    NoRequest,
}

/// Un bouton d'une de ces fenetres.
///
/// Quatre champs de plus qu'un `src/ui::Action` : `scope` (un bouton de ligne),
/// `registered` (qui l'a lie), `returns` (ce qu'il rend) et `refusals`, dont le type est
/// celui de ce module — `crate::ui::Refusal` est une enumeration fermee que huit refus
/// de plus ne peuvent pas etendre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id du XML, celui que `registerButton` lie.
    pub id: &'static str,
    /// La cle du libelle ; `""` pour une image nue (la croix, les fleches, les
    /// localisateurs).
    pub label: &'static str,
    /// Ce que le clic demande, avec le vocabulaire de `src/ui`.
    pub effect: Effect,
    /// Si le panneau est dans la fenetre ou dans une ligne de liste.
    pub scope: Scope,
    /// Qui enregistre le clic.
    pub registered: By,
    /// Ce que le clic rend a l'appelant.
    pub returns: Returns,
    /// Chaque facon dont le clic est refuse, dans l'ordre ou Java teste.
    pub refusals: &'static [Refusal],
}

impl Button {
    /// Vrai quand Java peut refuser le clic.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// La cle du libelle, `None` pour un bouton qui n'est qu'une image.
    pub fn label_key(&self) -> Option<&'static str> {
        if self.label.is_empty() {
            None
        } else {
            Some(self.label)
        }
    }

    /// Vrai quand le clic envoie un message au serveur — **faux partout ici** : les sept
    /// boutons agissent dans la fenetre, aucun n'appelle `Network.sendToServer`.
    pub fn sends_message(&self) -> bool {
        matches!(self.effect, Effect::Sends(_))
    }
}

/// Comment on arrive a la fenetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois portes dans ces trois fenetres
pub enum Opened {
    /// Un bouton d'une autre fenetre, par **classe et ligne** : celui de l'atelier n'a
    /// aucun id, c'est un `ImageButton` vanilla (`WindowCrafting.java:176-186`).
    Button { from: &'static str, at: u32 },
    /// Un onglet de la barre laterale d'un batiment, par cle de module
    /// (`AbstractBuildingWindow.java:67-83` appelle `view.getWindow()`).
    Tab(&'static str),
    /// L'item en main (`ItemQuestLog.java:125`).
    Item(&'static str),
}

/// Une fenetre : sa classe, son XML, son titre, ses panneaux, ses boutons, ses pages, et
/// la porte par laquelle on entre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. Ce n'est pas un id de `src/ui`.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML BlockUI, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// La cle du titre que le XML dessine dans un `text` sans id.
    pub title: &'static str,
    /// Les panneaux de la fenetre elle-meme, hors pages.
    pub panes: &'static [Pane],
    /// Les boutons qu'elle enregistre, ou que sa classe mere enregistre.
    pub buttons: &'static [Button],
    /// Les pages de sa vue a onglets ; vide quand elle n'en a pas.
    pub pages: &'static [quest_log::Page],
    /// La porte d'entree.
    pub opened: Opened,
}

impl Window {
    /// Le panneau de cet id, copie — le premier, comme `src/ui::Screen::field`.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copie.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Les boutons de ses pages, toutes pages confondues.
    pub fn page_buttons(&self) -> Vec<Button> {
        self.pages.iter().flat_map(|page| page.buttons().iter().copied()).collect()
    }

    /// La page de cet id de vue, ou [`Refusal::UnknownPage`] — y compris quand la page
    /// existe mais n'est pas de cette fenetre.
    pub fn page(&self, view: &str) -> Result<quest_log::Page, Refusal> {
        let page = quest_log::Page::of(view)?;
        if self.pages.contains(&page) {
            Ok(page)
        } else {
            Err(Refusal::UnknownPage)
        }
    }

    /// Vrai quand un bouton de la fenetre ou d'une de ses pages envoie un message.
    pub fn sends_message(&self) -> bool {
        self.buttons.iter().any(Button::sends_message)
            || self.page_buttons().iter().any(Button::sends_message)
    }
}

/// Les trois fenetres de ce module.
pub static WINDOWS: &[Window] = &[
    Window {
        id: SELECT_REQUEST_ID,
        java: select_request::JAVA,
        resource: select_request::RESOURCE,
        title: select_request::TITLE_KEY,
        panes: select_request::FIELDS,
        buttons: select_request::BUTTONS,
        pages: &[],
        // WindowCrafting.java:185 ; WindowFurnaceCrafting.java:130 et
        // DOCraftingWindow.java:112 le disent depuis deux autres ecrans.
        opened: Opened::Button { from: "WindowCrafting", at: 185 },
    },
    Window {
        id: STATS_ID,
        java: stats::JAVA,
        resource: stats::RESOURCE,
        title: stats::TITLE_KEY,
        panes: stats::FIELDS,
        buttons: stats::BUTTONS,
        pages: &[],
        // BuildingStatisticsModuleView.java:30-33 : c'est la vue du module qui fabrique
        // la fenetre, donc c'est l'onglet du module qui l'ouvre.
        opened: Opened::Tab(stats::MODULE),
    },
    Window {
        id: QUEST_LOG_ID,
        java: quest_log::JAVA,
        resource: quest_log::RESOURCE,
        title: quest_log::TITLE_KEY,
        panes: &[],
        buttons: quest_log::SHELL_BUTTONS,
        pages: &[
            quest_log::Page::InProgress,
            quest_log::Page::Available,
            quest_log::Page::Finished,
        ],
        opened: Opened::Item("ItemQuestLog"),
    },
];

/// La fenetre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenetre de cette classe Java, ou [`Refusal::UnknownWindow`] : la jointure inverse
/// marche pour trois classes et **refuse la quatrieme source du brief**.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    #[test]
    fn three_windows_the_fourth_source_is_a_page_and_no_id_is_a_src_ui_screen() {
        assert_eq!(WINDOWS.len(), 3);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(window.java.starts_with("Window"), "{}", window.id);
            assert!(
                window.resource.starts_with("minecolonies:gui/") && window.resource.ends_with(".xml"),
                "{} lit {}",
                window.id,
                window.resource
            );
            assert!(window.title.starts_with("com.minecolonies."), "{}", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenetres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
                assert_ne!(window.resource, other.resource, "deux fenetres, un XML");
            }
            assert_eq!(window_by_id(window.id).map(|found| found.id), Ok(window.id));
            assert_eq!(window_by_java(window.java).map(|found| found.id), Ok(window.id));
            // Aucun id d'ici n'est un ecran de src/ui.
            assert_eq!(
                crate::ui::screen_by_id(window.id).map(|screen| screen.id),
                Err(crate::ui::Refusal::UnknownScreen),
                "{} usurpe un id de src/ui",
                window.id
            );
        }
        // La quatrieme source n'est pas une fenetre : c'est une page du journal.
        assert_eq!(window_by_java(quest_log::AVAILABLE_JAVA), Err(Refusal::UnknownWindow));
        assert_eq!(
            WINDOWS[2].page(QUEST_PAGE_AVAILABLE).map(|page| page.module()),
            Ok(quest_log::AVAILABLE_JAVA)
        );
        // Et deux refus nommes pour le reste.
        assert_eq!(window_by_id("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowHireWorker"), Err(Refusal::UnknownWindow));
    }

    #[test]
    fn seven_buttons_and_none_of_them_sends_a_message() {
        let shell: usize = WINDOWS.iter().map(|window| window.buttons.len()).sum();
        let rows: usize = WINDOWS.iter().map(|window| window.page_buttons().len()).sum();
        assert_eq!(shell + rows, 7, "2 + 1 + 2 + 2");
        for window in WINDOWS {
            assert!(!window.sends_message(), "{} envoie un message", window.id);
            for button in window.buttons.iter().chain(window.page_buttons().iter()) {
                assert!(!button.sends_message(), "{}.{}", window.id, button.id);
                assert_ne!(button.effect, Effect::Sends(""));
            }
        }
        // Les deux seuls messages sont ceux de BlockUI, et aucun bouton ne les porte.
        assert_eq!(
            GENERIC_TRIGGERS,
            ["OpenGuiWindowTriggerMessage", "ClickGuiButtonTriggerMessage"]
        );
    }

    #[test]
    fn every_row_button_names_a_list_of_its_own_window() {
        for window in WINDOWS {
            let mut buttons: Vec<Button> = window.buttons.to_vec();
            buttons.extend(window.page_buttons());
            for (index, button) in buttons.iter().enumerate() {
                for other in &buttons[index + 1..] {
                    assert!(
                        button.id != other.id || button.scope != other.scope,
                        "{}: deux fois {} dans la meme portee",
                        window.id,
                        button.id
                    );
                }
                let Scope::Row(list) = button.scope else {
                    continue;
                };
                // La liste existe : c'est un panneau du meme ecran, de valeur `Row`.
                let list_pane = window.pane(list).or_else(|| {
                    window
                        .pages
                        .iter()
                        .filter(|page| Some(page.view()) == page_of(list))
                        .flat_map(|page| page.panes().iter().copied())
                        .find(|pane| pane.id == quest_log::LIST)
                });
                let list_pane = match list_pane {
                    Some(pane) => pane,
                    None => panic!("{}: {list} n'est pas une liste", window.id),
                };
                assert_eq!(list_pane.value, Value::Row);
                assert!(list.contains(list_pane.id), "{list} ne nomme pas la liste");
            }
        }
    }

    #[test]
    fn the_three_windows_are_reached_three_ways() {
        let opened: Vec<Opened> = WINDOWS.iter().map(|window| window.opened).collect();
        assert_eq!(
            opened,
            [
                Opened::Button { from: "WindowCrafting", at: 185 },
                Opened::Tab("stats_module"),
                Opened::Item("ItemQuestLog"),
            ]
        );
        // Chaque porte cite sa source : une classe et une ligne, une cle, ou un item.
        for window in WINDOWS {
            match window.opened {
                Opened::Button { from, at } => {
                    assert!(from.starts_with("Window"), "{from}");
                    assert!(at > 0, "{from} sans ligne");
                }
                Opened::Tab(key) => assert!(!key.is_empty()),
                Opened::Item(class) => assert!(class.starts_with("Item"), "{class}"),
            }
        }
        // L'onglet des stats n'a pas d'id de src/ui : c'est le trou que ce module comble.
        assert_eq!(crate::ui::table::module_screen(stats::MODULE), None);
    }

    #[test]
    fn only_the_quest_log_has_pages_and_each_page_belongs_to_it() {
        let with_pages: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| !window.pages.is_empty())
            .map(|window| window.id)
            .collect();
        assert_eq!(with_pages, [QUEST_LOG_ID]);
        let journal = match window_by_id(QUEST_LOG_ID) {
            Ok(window) => window,
            Err(refusal) => panic!("{refusal}"),
        };
        let pages: [quest_log::Page; 3] = quest_log::Page::ALL;
        assert_eq!(journal.pages, pages, "les trois cles de moduleMap");
        assert_eq!(journal.page(QUEST_PAGE_IN_PROGRESS), Ok(quest_log::Page::InProgress));
        assert_eq!(journal.page(QUEST_PAGE_FINISHED), Ok(quest_log::Page::Finished));
        assert_eq!(journal.page("pageElsewhere"), Err(Refusal::UnknownPage));
        // Le journal n'a aucun panneau de fenetre : tout est dans ses pages.
        assert!(journal.panes.is_empty());
        // Et une page de ce journal n'est pas atteignable depuis une autre fenetre.
        for window in WINDOWS.iter().filter(|window| window.id != QUEST_LOG_ID) {
            assert_eq!(window.page(QUEST_PAGE_AVAILABLE), Err(Refusal::UnknownPage));
        }
    }

    #[test]
    fn the_seventeen_panes_use_four_values_and_the_one_dropdown_is_not_a_button() {
        let all: Vec<Pane> = WINDOWS
            .iter()
            .flat_map(|window| {
                window
                    .panes
                    .iter()
                    .copied()
                    .chain(window.pages.iter().flat_map(|page| page.panes().iter().copied()))
            })
            .collect();
        assert_eq!(all.len(), 17, "5 + 2 + 10");
        let mut values: Vec<Value> = all.iter().map(|pane| pane.value).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values, [Value::Name, Value::Row, Value::Icon, Value::Text]);
        // Ni niveau, ni progression, ni stock : la deuxieme premisse du brief tombe.
        for absent in [Value::Level, Value::Progress, Value::Stock] {
            assert!(!all.iter().any(|pane| pane.value == absent), "{absent:?} est la");
        }
        // Chaque panneau cite un des trois XML et une ligne.
        let resources: Vec<&str> = WINDOWS.iter().map(|window| window.resource).collect();
        for pane in &all {
            assert!(pane.declared.1 > 0, "{} sans ligne", pane.id);
            assert!(
                resources.iter().any(|resource| resource.ends_with(pane.declared.0)),
                "{} ne vient pas d'une de ces fenetres",
                pane.declared.0
            );
        }
        // La liste deroulante n'est enregistree par personne ; le bouton des zeros, si.
        let window = match window_by_id(STATS_ID) {
            Ok(window) => window,
            Err(refusal) => panic!("{refusal}"),
        };
        assert_eq!(window.button(stats::DROPDOWN), None);
        assert_eq!(window.button(stats::DROPDOWN_BUTTON), None);
        assert_eq!(
            window.button(stats::HIDE_ZERO).map(|button| button.effect),
            Some(Effect::Toggles)
        );
        assert_eq!(stats::View::new().interval, stats::Interval::Yesterday);
        assert_eq!(stats::Interval::ALL.len(), 4);
    }

    /// Les ids de page que le XML dessine (`windowquestlog.xml:17`, `:30`, `:42`).
    const QUEST_PAGE_IN_PROGRESS: &str = "pageInProgress";
    const QUEST_PAGE_AVAILABLE: &str = "pageAvailable";
    const QUEST_PAGE_FINISHED: &str = "pageFinished";

    /// La page dont la portee de ligne est `list`.
    fn page_of(list: &str) -> Option<&'static str> {
        quest_log::Page::ALL
            .iter()
            .find(|page| page.rows() == list)
            .map(|page| page.view())
    }
}
