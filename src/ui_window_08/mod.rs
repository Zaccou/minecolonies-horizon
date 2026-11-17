//! Les quatre fenêtres de ce dossier : ce que chacune **demande**, ce que ses boutons
//! **rendent**, et l'état qu'un clic y change.
//!
//! Java reference : `core/client/gui/WindowsBarracksSpies.java`,
//! `core/client/gui/containers/{WindowBrewingstandCrafting,WindowBuildingInventory,WindowCitizenInventory}.java`,
//! leurs conteneurs dans `api/inventory/container/`, et les trois messages serveur qui
//! décident vraiment (`HireSpiesMessage`, `AddRemoveRecipeMessage`, `OpenInventoryMessage`).
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`pane`] | un panneau : ce qu'il porte, et d'où vient sa valeur |
//! | [`spies`] | cinq lingots contre des espions : trois refus, un tick qui paie tout seul |
//! | [`brewing`] | quatre emplacements, un bouton, un « plafond » qui n'en est pas un |
//! | [`container`] | deux inventaires : les rangées, l'armure, un `static` |
//! | [`refusal`] | pourquoi rien ne se passe, et qui l'entend |
//!
//! # Ces quatre fenêtres ne sont pas des écrans de hutte
//!
//! `src/ui` décrit les treize écrans qu'une hutte ouvre ; aucune de ces quatre classes
//! n'est du lot. Deux le sont par la **porte** : `WindowsBarracksSpies` est nommée par
//! l'action `hireSpies` de l'écran de la caserne (`src/ui::screen`, `Effect::OpensJava`),
//! et les trois autres sont ouvertes par un **message** qui n'a rien à voir avec un écran
//! de hutte ([`spies`], [`brewing`], [`container`]). C'est pourquoi [`WINDOWS`] porte
//! [`Reached`] : *comment* on arrive ici, dit une fois, plutôt que dans quatre documents.
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | **4** | [`WINDOWS`] |
//! | fichiers `WindowHut*.java` parmi elles | **0** | — |
//! | XML BlockUI | **1** | `windowbarracksspies.xml` |
//! | panneaux nommés | **3** | [`spies::FIELDS`] ; les trois conteneurs n'en ont aucun |
//! | boutons | **3** | 2 dans la caserne, 1 dans la brasserie |
//! | ids de bouton venus du XML | **2** | `cancel`, `hireSpies` ; celui de la brasserie est anonyme |
//! | écrans de conteneur | **3** | [`Menu`] |
//! | formes de titre différentes | **4** | [`Title`] |
//! | classes de message serveur nommées | **3** | `HireSpiesMessage`, `AddRemoveRecipeMessage`, `OpenInventoryMessage` (+ `OpenCraftingGUIMessage` pour atteindre la brasserie) |
//! | état `static` en Java | **1** | [`container::Active`] |
//!
//! # Les deux prémisses du brief que le source contredit
//!
//! 1. **« les actions (embaucher, renvoyer, prioriser) »** ne sont sur **aucune** de ces
//!    quatre fenêtres. Embaucher, renvoyer et prioriser sont les boutons de l'écran de la
//!    hutte de métier, que `src/ui` porte déjà (`Action::guarded("hire", …)`,
//!    `A_RECALL`, `A_PRIO_UP`, `A_PRIO_DOWN`). Ici, il y a **trois** boutons en tout :
//!    une croix, un paiement et un « terminé ».
//! 2. **« progression, stock »** n'existent pas ici non plus : la caserne affiche un prix
//!    et une description, la brasserie quatre piles, et les deux inventaires une grille.
//!    Aucune de ces quatre fenêtres ne montre un pourcentage ; le seul « stock » est une
//!    **grille d'emplacements**, que [`container::rows`] mesure en rangées.
//!
//! # Ce qui n'est pas ici
//!
//! - **le dessin** : ni position, ni taille, ni sprite, ni couleur, ni emplacement de
//!   grille. Même règle que `src/ui`, et `TRANCHE.md` §3 M2.
//! - **l'envoi** : les trois classes de message sont nommées, aucune n'est envoyée — il
//!   n'y a ni client ni transport dans ce port (`src/ui/MANQUANT.md` §5).
//! - **le registre de brassage** : la sortie d'une recette de brasserie vient de Forge,
//!   pas du port ; [`brewing::State::done`] la reçoit de l'appelant (`MANQUANT.md` §2).
//! - **les types d'équipement** : `GuardGear.test` demande le registre d'équipement, donc
//!   [`container::may_place`] modélise la **plage** de niveaux d'armure, pas l'objet
//!   (`MANQUANT.md` §3).

pub mod brewing;
pub mod container;
pub mod pane;
pub mod refusal;
pub mod spies;

pub use pane::{Pane, Source};
pub use refusal::Refusal;

use crate::ui::field::Field;

/// L'id de la fenêtre des espions dans cette table. Ce n'est pas un id de `src/ui` : la
/// caserne y nomme déjà son bouton `hireSpies`, et cette fenêtre-là n'y a pas de ligne.
pub const SPIES_ID: &str = "barracks.spies";
/// L'id de la fenêtre de brassage.
pub const BREWING_ID: &str = "hut.brewing";
/// L'id de l'inventaire d'un bâtiment.
pub const BUILDING_INVENTORY_ID: &str = "hut.container";
/// L'id de l'inventaire d'un citoyen.
pub const CITIZEN_INVENTORY_ID: &str = "citizen.container";

/// Ce qu'un clic fait, une fois que rien ne l'a refusé.
///
/// **Deux variantes là où `src/ui` en a neuf** : aucun de ces quatre écrans n'ouvre un
/// autre écran, ne renomme une hutte, ni ne met en évidence une position. Il envoie un
/// message, ou il ferme sa fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux effets de ces quatre fenêtres
pub enum Effect {
    /// Envoie un message au serveur, par **nom de classe**. Rien n'est envoyé ici : il
    /// n'y a pas de client, et [`crate::network`] porte le format de ces messages.
    Sends(&'static str),
    /// Ferme la fenêtre et rien d'autre : `this.close()` (`WindowsBarracksSpies.java:85`).
    Closes,
}

impl Effect {
    /// Un nom stable, pour une ligne de log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Effect::Sends(class) => class,
            Effect::Closes => "closes",
        }
    }
}

/// D'où vient l'id d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: le XML, ou nous
pub enum Naming {
    /// L'id est celui du XML, et Java le compare à une constante de classe
    /// (`WindowsBarracksSpies.java:29`, `:34`, `:81-95`).
    Xml,
    /// Le bouton n'a **aucun** id dans Java : la fenêtre le crée par
    /// `new Button.Builder(...)` (`WindowBrewingstandCrafting.java:100`), qui n'en donne
    /// pas. Le nom est le nôtre, et il ne peut donc pas venir du jeu.
    Ours,
}

impl Naming {
    /// Un nom stable.
    pub fn name(self) -> &'static str {
        match self {
            Naming::Xml => "xml",
            Naming::Ours => "ours",
        }
    }
}

/// Un bouton de l'une de ces fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id du bouton — celui du XML, ou le nôtre (voir [`Naming`]).
    pub id: &'static str,
    /// La clé du libellé ; `""` quand le bouton est une image sans texte (la croix de la
    /// caserne, `windowbarracksspies.xml:7`).
    pub label: &'static str,
    /// Ce que le clic fait.
    pub effect: Effect,
    /// Chaque façon dont le clic est refusé, dans l'ordre où Java les teste.
    pub refusals: &'static [Refusal],
    /// D'où vient l'id.
    pub naming: Naming,
}

impl Button {
    /// Si Java peut refuser ce bouton.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// La clé du libellé, `None` pour un bouton sans texte.
    pub fn label_key(&self) -> Option<&'static str> {
        if self.label.is_empty() {
            None
        } else {
            Some(self.label)
        }
    }
}

/// Qui décrit les panneaux d'une fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: cette tranche, ou src/ui
pub enum Panes {
    /// Cette tranche : aucune table de `src/ui` ne décrit la fenêtre.
    Own(&'static [Pane]),
    /// `src/ui` la décrit déjà, sous cet id d'écran : on y renvoie.
    Ui(&'static str),
}

/// Ce que le titre de la fenêtre affiche.
///
/// **Quatre fenêtres, quatre formes de titre, aucune partagée** : c'est le premier fait de
/// ce dossier, et il explique pourquoi `src/ui::screen::Title` (deux variantes) ne suffit
/// pas ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre titres de ces quatre fenêtres
pub enum Title {
    /// Le nom du bloc : `getBlockState().getBlock().getName()`
    /// (`TileEntityColonyBuilding.java:344-347`), dessiné à
    /// `WindowBuildingInventory.java:65`.
    Block,
    /// Le nom du citoyen, porté par le conteneur (`ContainerCitizenInventory.java:115`,
    /// `:347`) et dessiné à `WindowCitizenInventory.java:104`.
    CitizenName,
    /// Un littéral que le serveur écrit dans le paquet, sans clé de traduction
    /// (`OpenCraftingGUIMessage.java:103`).
    Literal(&'static str),
    /// Aucun titre : la caserne n'a pas de panneau de titre, sa description est un
    /// panneau (`windowbarracksspies.xml:9`).
    None,
}

/// Comment on arrive à la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un bouton de src/ui, ou un message serveur
pub enum Reached {
    /// Un bouton d'un écran de `src/ui` nomme cette classe en `Effect::OpensJava` :
    /// `{ screen, action }` sont l'id de l'écran et l'id du bouton.
    FromUi { screen: &'static str, action: &'static str },
    /// Un message serveur ouvre le conteneur, par nom de classe.
    Message(&'static str),
}

/// Le conteneur du jeu que la fenêtre affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre écrans, trois conteneurs
pub enum Menu {
    /// Pas un écran de conteneur : la caserne est une `BOWindow`
    /// (`WindowsBarracksSpies.java:24`).
    None,
    /// `ContainerBuildingInventory`, ouvert par `OpenInventoryMessage` en
    /// `INVENTORY_CHEST` (`OpenInventoryMessage.java:159-172`).
    BuildingInventory,
    /// `ContainerCitizenInventory`, ouvert par `OpenInventoryMessage` en
    /// `INVENTORY_CITIZEN` (`:140-157`).
    CitizenInventory,
    /// `ContainerCraftingBrewingstand`, ouvert par `OpenCraftingGUIMessage`
    /// (`OpenCraftingGUIMessage.java:95-113`).
    BrewingStand,
}

impl Menu {
    /// Le type de `OpenInventoryMessage` qui ouvre ce conteneur, quand c'est lui.
    ///
    /// Le type est celui de `crate::network::windows::InventoryType`, donc la jointure est
    /// vérifiée par le compilateur et non par une chaîne recopiée.
    pub fn inventory_type(self) -> Option<crate::network::windows::InventoryType> {
        match self {
            Menu::BuildingInventory => {
                Some(crate::network::windows::InventoryType::Chest)
            }
            Menu::CitizenInventory => Some(crate::network::windows::InventoryType::Citizen),
            Menu::None | Menu::BrewingStand => None,
        }
    }

    /// La classe Java du conteneur.
    pub fn java_container(self) -> Option<&'static str> {
        match self {
            Menu::None => None,
            Menu::BuildingInventory => Some("ContainerBuildingInventory"),
            Menu::CitizenInventory => Some("ContainerCitizenInventory"),
            Menu::BrewingStand => Some("ContainerCraftingBrewingstand"),
        }
    }
}

/// Une fenêtre : sa classe, son XML quand elle en a un, son titre, ses panneaux, ses
/// boutons, comment on y arrive, et le conteneur qu'elle affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. Ce n'est pas un id de `src/ui`.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML BlockUI, `None` : les trois écrans de conteneur sont dessinés par `blit`.
    pub resource: Option<&'static str>,
    /// Ce que le titre affiche.
    pub title: Title,
    /// Qui décrit ses panneaux.
    pub panes: Panes,
    /// Les boutons qu'elle enregistre.
    pub buttons: &'static [Button],
    /// Comment on y arrive.
    pub reached: Reached,
    /// Le conteneur du jeu qu'elle affiche.
    pub menu: Menu,
}

impl Window {
    /// Le panneau de cet id, copié — le premier, comme `src/ui::Screen::field`.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        match self.panes {
            Panes::Own(panes) => panes.iter().find(|pane| pane.id == id).copied(),
            Panes::Ui(_) => None,
        }
    }

    /// Le bouton de cet id, copié.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Les panneaux décrits ici, vides quand `src/ui` les porte.
    pub fn own_panes(&self) -> &'static [Pane] {
        match self.panes {
            Panes::Own(panes) => panes,
            Panes::Ui(_) => &[],
        }
    }
}

/// Les quatre fenêtres de ce dossier.
pub static WINDOWS: &[Window] = &[
    Window {
        id: SPIES_ID,
        java: spies::JAVA,
        resource: Some(spies::RESOURCE),
        title: Title::None,
        panes: Panes::Own(spies::FIELDS),
        buttons: spies::BUTTONS,
        // WindowBarracksBuilding.java:106 : `hireSpiesClicked`.
        reached: Reached::FromUi {
            screen: crate::ui::screen::HUT_BARRACKS_ID,
            action: spies::BUTTON_HIRE,
        },
        menu: Menu::None,
    },
    Window {
        id: BREWING_ID,
        java: brewing::JAVA,
        resource: None,
        title: Title::Literal(brewing::TITLE),
        panes: Panes::Own(brewing::FIELDS),
        buttons: brewing::BUTTONS,
        reached: Reached::Message("OpenCraftingGUIMessage"),
        menu: Menu::BrewingStand,
    },
    Window {
        id: BUILDING_INVENTORY_ID,
        java: container::BUILDING_JAVA,
        resource: None,
        title: Title::Block,
        panes: Panes::Own(container::BUILDING_FIELDS),
        buttons: container::BUILDING_BUTTONS,
        reached: Reached::Message("OpenInventoryMessage"),
        menu: Menu::BuildingInventory,
    },
    Window {
        id: CITIZEN_INVENTORY_ID,
        java: container::CITIZEN_JAVA,
        resource: None,
        title: Title::CitizenName,
        panes: Panes::Own(container::CITIZEN_FIELDS),
        buttons: container::CITIZEN_BUTTONS,
        reached: Reached::Message("OpenInventoryMessage"),
        menu: Menu::CitizenInventory,
    },
];

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

/// L'action de `src/ui` qui ouvre cette classe en `Effect::OpensJava`, avec son écran.
///
/// C'est la jointure avec la table déjà écrite : `src/ui` nomme la classe
/// (`Effect::OpensJava`), cette table répond ce que la fenêtre demande. Un seul de ces
/// quatre écrans est atteint comme ça.
///
/// **La comparaison se fait sur le nom porté par l'effet, pas sur un `Effect` construit
/// à partir de l'argument** : `Effect::OpensJava` prend un `&'static str` (le nom d'une
/// classe Java est une constante de la table), alors qu'un appelant peut interroger un
/// nom emprunté — la chaîne lue dans un log, par exemple.
pub fn opened_by_ui(class: &str) -> Option<(&'static crate::ui::Screen, crate::ui::Action)> {
    for screen in crate::ui::SCREENS.iter().copied() {
        for action in screen.actions {
            if let crate::ui::Effect::OpensJava(named) = action.effect {
                if named == class {
                    return Some((screen, *action));
                }
            }
        }
    }
    None
}

/// Les panneaux d'une fenêtre sous la forme de `src/ui`, quand sa source est une source de
/// `src/ui`. `None` pour un panneau venu d'un conteneur, d'un rack, d'un bloc, d'une entité
/// ou de la classe : `src/ui` n'a pas ce lecteur.
pub fn as_ui_fields(window: &Window) -> Option<Vec<Field>> {
    window.own_panes().iter().map(Pane::to_ui).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_windows_and_only_one_has_an_xml() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(!window.id.is_empty());
            assert!(
                window.java.starts_with("Window"),
                "{} n'a pas une classe de fenêtre",
                window.id
            );
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenêtres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
            }
        }
        // Une seule a un XML : les trois autres sont des AbstractContainerScreen.
        let with_xml: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.resource.is_some())
            .map(|window| window.id)
            .collect();
        assert_eq!(with_xml, [SPIES_ID]);
        assert_eq!(
            WINDOWS.iter().filter(|window| window.menu != Menu::None).count(),
            3
        );
        // Le conteneur nommé est celui du jeu, et c'est la classe que Java instancie.
        assert_eq!(
            window_by_id(BREWING_ID).map(|window| window.menu.java_container()),
            Ok(Some("ContainerCraftingBrewingstand"))
        );
        assert_eq!(window_by_id(SPIES_ID).map(|window| window.menu.java_container()), Ok(None));
        // Aucun id de cette table n'est un id de src/ui : ces fenêtres ne sont pas des
        // écrans de hutte.
        for window in WINDOWS {
            assert_eq!(
                crate::ui::screen_by_id(window.id).map(|screen| screen.id),
                Err(crate::ui::Refusal::UnknownScreen),
                "{} usurpe un id de src/ui",
                window.id
            );
        }
    }

    #[test]
    fn one_window_is_reached_from_a_src_ui_action_and_three_from_a_message() {
        let from_ui = opened_by_ui(spies::JAVA);
        let Some((screen, action)) = from_ui else {
            panic!("aucune action de src/ui n'ouvre {}", spies::JAVA);
        };
        // La jointure tombe sur l'écran de la caserne et sur son bouton d'espions.
        assert_eq!(screen.id, crate::ui::screen::HUT_BARRACKS_ID);
        assert_eq!(action.id, "hireSpies");
        // Et le refus de niveau vit là-bas, pas ici.
        assert!(action.refusals.contains(&crate::ui::Refusal::SpiesBelowLevel));
        assert!(screen.gate.is_none(), "la caserne n'a pas de porte d'écran");
        assert_eq!(spies::BARRACKS_LEVEL_FOR_BUTTON, 3);
        // Les trois autres n'ont aucune action de src/ui, et disent par quel message.
        for window in WINDOWS.iter().filter(|window| window.id != SPIES_ID) {
            assert!(opened_by_ui(window.java).is_none(), "{}", window.id);
            assert!(
                matches!(window.reached, Reached::Message(_)),
                "{} est atteinte par un message",
                window.id
            );
        }
        // Les deux inventaires passent par le même message, et c'est leur type qui les
        // sépare — le type de `crate::network`, pas une chaîne recopiée.
        let building = match window_by_id(BUILDING_INVENTORY_ID) {
            Ok(window) => window,
            Err(refusal) => panic!("{refusal}"),
        };
        let citizen = match window_by_id(CITIZEN_INVENTORY_ID) {
            Ok(window) => window,
            Err(refusal) => panic!("{refusal}"),
        };
        assert_eq!(building.reached, citizen.reached);
        assert_eq!(
            building.menu.inventory_type(),
            Some(crate::network::windows::InventoryType::Chest)
        );
        assert_eq!(
            citizen.menu.inventory_type(),
            Some(crate::network::windows::InventoryType::Citizen)
        );
        assert_ne!(building.menu.inventory_type(), citizen.menu.inventory_type());
        // Et un refus nommé pour tout le reste.
        assert_eq!(window_by_java("WindowHireWorker"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(
            window_by_java("WindowsBarracksSpies").map(|window| window.id),
            Ok(SPIES_ID)
        );
    }

    #[test]
    fn the_four_titles_are_four_different_shapes() {
        let titles: Vec<Title> = WINDOWS.iter().map(|window| window.title).collect();
        assert_eq!(
            titles,
            [
                Title::None,
                Title::Literal(brewing::TITLE),
                Title::Block,
                Title::CitizenName,
            ]
        );
        // Aucune n'est le titre à deux écritures de `src/ui` (nom puis niveau) : ces
        // quatre fenêtres n'affichent aucun niveau.
        assert_eq!(brewing::TITLE, "Brewing Crafting GUI");
        assert!(!brewing::TITLE.starts_with("com.minecolonies."), "un littéral, pas une clé");
    }

    #[test]
    fn the_three_buttons_are_two_from_the_xml_and_one_that_java_never_names() {
        let buttons: Vec<Button> = WINDOWS.iter().flat_map(|window| window.buttons.iter().copied()).collect();
        assert_eq!(buttons.len(), 3);
        let ours: Vec<&str> = buttons
            .iter()
            .filter(|button| button.naming == Naming::Ours)
            .map(|button| button.id)
            .collect();
        assert_eq!(ours, [brewing::BUTTON_DONE], "Java crée ce bouton sans id");
        assert_eq!(Naming::Xml.name(), "xml");
        assert_eq!(Naming::Ours.name(), "ours");
        // La croix n'a pas de libellé ; les deux autres en ont un.
        let cancel = WINDOWS[0].button(spies::BUTTON_CANCEL);
        assert_eq!(cancel.and_then(|button| button.label_key()), None);
        assert_eq!(cancel.map(|button| button.effect), Some(Effect::Closes));
        let hire = WINDOWS[0].button(spies::BUTTON_HIRE);
        assert_eq!(
            hire.map(|button| button.effect),
            Some(Effect::Sends("HireSpiesMessage"))
        );
        assert_eq!(hire.and_then(|button| button.label_key()), Some(spies::HIRE_LABEL_KEY));
        assert!(hire.is_some_and(|button| button.is_guarded()));
        // Le bouton de la brasserie : un seul message, deux refus, dans l'ordre de Java.
        let done = WINDOWS[1].button(brewing::BUTTON_DONE);
        assert_eq!(
            done.map(|button| button.effect),
            Some(Effect::Sends("AddRemoveRecipeMessage"))
        );
        assert_eq!(
            done.map(|button| button.refusals),
            Some(&[Refusal::CraftingTypeNotTaught, Refusal::NoBrewingRecipe][..])
        );
        // Aucun bouton des deux inventaires.
        assert!(WINDOWS[2].buttons.is_empty());
        assert!(WINDOWS[3].buttons.is_empty());
        assert_eq!(Effect::Sends("X").name(), "X");
        assert_eq!(Effect::Closes.name(), "closes");
    }

    #[test]
    fn three_windows_name_no_pane_at_all_and_the_fourth_names_three() {
        for window in WINDOWS {
            for (index, pane) in window.own_panes().iter().enumerate() {
                let repeated = window.own_panes()[..index]
                    .iter()
                    .any(|other| other.id == pane.id && other.scope == pane.scope);
                assert!(!repeated, "{} répète {}", window.id, pane.id);
                // Aucune de ces fenêtres n'a de liste : pas une seule ligne.
                assert!(!pane.is_row_pane(), "{}: {} est une ligne", window.id, pane.id);
                // Et toute source d'un panneau nommé est soit celle de src/ui, soit l'un
                // des cinq lecteurs de ce module.
                assert!(!pane.source.name().is_empty());
            }
        }
        let counts: Vec<usize> = WINDOWS.iter().map(|window| window.own_panes().len()).collect();
        assert_eq!(counts, [3, 0, 0, 0]);
        // Les trois panneaux de la caserne : une icône, un prix, une description.
        let spies_panes: Vec<&str> = spies::FIELDS.iter().map(|pane| pane.id).collect();
        assert_eq!(spies_panes, [spies::ICON, spies::AMOUNT, spies::TEXT]);
        // Aucune source de src/ui à convertir : ce sont des valeurs que la classe écrit.
        assert_eq!(as_ui_fields(&WINDOWS[0]).map(|fields| fields.len()), None);
        // Un panneau d'écran de hutte, lui, se convertit dans les deux sens.
        let worker = crate::ui::screen_by_id(crate::ui::screen::HUT_WORKER_ID);
        let Some(worker) = worker.ok() else { panic!("pas d'écran de worker") };
        let Some(field) = worker.fields.first() else { panic!("pas de panneau") };
        assert_eq!(Pane::from_ui(*field).to_ui(), Some(*field));
    }
}
