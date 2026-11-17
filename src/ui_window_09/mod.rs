//! Les quatre fenêtres de ce dossier : ce que chacune **demande**, ce que ses boutons
//! **rendent**, et l'état qu'un clic y change.
//!
//! Java reference : `core/client/gui/containers/WindowCrafting.java` (283 lignes),
//! `WindowFurnaceCrafting.java` (195), `WindowField.java` (374), `WindowGrave.java` (169),
//! leurs conteneurs dans `api/inventory/container/`, `core/tileentities/
//! {TileEntityScarecrow,TileEntityGrave}.java`, `core/colony/buildingextensions/FarmField.java`
//! et les messages `OpenCraftingGUIMessage`, `AddRemoveRecipeMessage`,
//! `SwitchRecipeCraftingTeachingMessage`, `FarmFieldPlotResizeMessage`,
//! `FarmFieldUpdateSeedMessage`.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`pane`] | un panneau : ce qu'il porte, et d'où vient sa valeur |
//! | [`crafting`] | un établi 2×2 ou 3×3, et deux drapeaux qui peuvent se contredire |
//! | [`furnace`] | deux cases, un titre qui vient du paquet, trois emplacements pour deux |
//! | [`plot`] | un épouvantail : six valeurs, cinq boutons, un modulo qui divise par zéro |
//! | [`grave`] | deux titres, zéro bouton, et trois branches mortes |
//! | [`refusal`] | pourquoi rien ne se passe — et pourquoi aucune de ces fenêtres ne parle |
//!
//! # Aucune de ces quatre fenêtres n'est un écran de hutte
//!
//! `src/ui` décrit les treize écrans qu'une hutte ouvre ; aucune de ces quatre classes n'en
//! est. Deux sont ouvertes par un **message** ([`Reached::Message`]) : l'établi et le four
//! par `OpenCraftingGUIMessage`, qui choisit entre les trois fenêtres d'artisanat selon ce
//! que le module enseigne (`:76-135`). Deux sont ouvertes par un **clic de bloc**
//! ([`Reached::Block`]) : l'épouvantail (`BlockScarecrow.java:103-113`, côté client
//! seulement) et la tombe. [`opened_by_ui`] rend `None` pour les quatre, et un test le
//! vérifie.
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | **4** | [`WINDOWS`] |
//! | fichiers `WindowHut*.java` parmi elles | **0** | — |
//! | XML BlockUI | **1** | `windowfield.xml` (les trois autres sont dessinés par `blit`) |
//! | panneaux nommés | **9** | [`plot::FIELDS`] ; les trois conteneurs n'en ont aucun |
//! | boutons | **10** | 5 dans le champ, 3 dans l'établi, 2 dans le four, 0 dans la tombe |
//! | ids de bouton venus du XML | **5** | ceux du champ, tous (`windowfield.xml:14-27`) |
//! | ids de bouton qui n'existent pas dans Java | **5** | les deux « terminé », les deux listes de requêtes, la bascule |
//! | écrans de conteneur | **3** | [`Menu`] |
//! | formes de titre différentes | **3** pour **4** titres | [`Title`] : une clé en dur, un libellé de XML, deux littéraux |
//! | refus nommés | **13** | [`Refusal::ALL`], dont **aucun** n'est dit au joueur |
//! | branches de dessin mortes | **3** | [`grave`] : deux seuils et une texture |
//!
//! # Les trois prémisses du brief que le source contredit
//!
//! 1. **« les champs affichés (nom, niveau, progression, stock) » n'existent sur aucune des
//!    quatre.** Pas un niveau, pas un pourcentage, pas une ligne « disponible / requis » :
//!    la valeur la plus proche d'un « stock » est une **grille de 54 emplacements**
//!    ([`grave::GRAVE_SLOTS`]) que la fenêtre ne compte pas, et la plus proche d'une
//!    « progression » est une **largeur de champ** en blocs ([`plot::Snapshot::sizes`]).
//!    Le seul « nom » de ce groupe est le nom de l'exploitant d'un champ
//!    ([`plot::FarmerText`]). Un appelant qui attend une fiche de bâtiment se trompe de
//!    tranche : `src/ui` la porte, et son `Value::Progress` n'apparaît que sur l'onglet des
//!    réserves du constructeur.
//!
//! 2. **« les actions (embaucher, renvoyer, prioriser) » ne sont ici non plus.** Embaucher,
//!    renvoyer et prioriser sont les boutons de l'écran de la hutte de métier, déjà portés
//!    par `src/ui` (`ui::action::Action::guarded("hire", …)`, `A_RECALL`, `A_PRIO_UP`).
//!    Ces quatre fenêtres ont **dix** boutons, et pas un ne touche à un citoyen : deux
//!    « terminé » qui envoient une recette, une bascule de recette, deux listes de
//!    requêtes, un choix de graine et quatre flèches qui changent la taille d'un champ.
//!
//! 3. **« un titre par fenêtre » est faux deux fois.** L'établi dessine
//!    `Component.translatable("container.crafting")` en dur et **ignore** le nom que le
//!    serveur a mis dans le paquet (`crafting::TITLE_KEY`) ; le champ n'a pas de titre de
//!    classe du tout, c'est le XML qui en porte un (`plot::HEADER_KEY`). Le four et la
//!    tombe, eux, dessinent bien le nom du paquet — et pour la tombe ce nom est un
//!    **littéral** (`"Grave"`), donc intraduisible.
//!
//! # Ce qui n'est pas ici
//!
//! - **le dessin** : ni position, ni taille, ni sprite, ni emplacement de grille. Même
//!   règle que `src/ui`, et `TRANCHE.md` §3 M2.
//! - **l'envoi** : les cinq classes de message sont nommées, aucune n'est envoyée — il n'y
//!   a ni client ni transport dans ce port. Un test vérifie que chacune est une vraie
//!   ligne du registre de `crate::channel`, donc qu'aucun nom n'est inventé.
//! - **le contenu des recettes** : la sortie d'un établi et d'un four vient du registre de
//!   recettes du jeu, que le port n'a pas. Les deux fenêtres **reçoivent** leur sortie au
//!   lieu de la calculer (`MANQUANT.md` §0).
//! - **le registre de langue** : `I18n.exists` décide si le biome s'écrit par son nom ou par
//!   son chemin (`MANQUANT.md` §3).

pub mod crafting;
pub mod furnace;
pub mod grave;
pub mod pane;
pub mod plot;
pub mod refusal;

pub use pane::{Pane, Source};
pub use refusal::Refusal;

use crate::ui::field::Field;

/// L'établi d'artisanat. Ce n'est pas un id de `src/ui`.
pub const CRAFTING_ID: &str = "craft.crafting";
/// Le four.
pub const FURNACE_ID: &str = "craft.furnace";
/// L'écran de l'épouvantail.
pub const FIELD_ID: &str = "field.scarecrow";
/// La tombe.
pub const GRAVE_ID: &str = "grave.inventory";

/// Ce qu'un clic fait, une fois que rien ne l'a refusé.
///
/// **Trois variantes là où `src/ui` en a neuf** : aucune de ces fenêtres ne renomme une
/// hutte, ne met en évidence une position, n'ouvre un écran de `src/ui`, ni ne bascule un
/// booléen local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois effets de ces quatre fenêtres
pub enum Effect {
    /// Envoie un message au serveur, par **nom de classe**. Rien n'est envoyé ici :
    /// [`crate::network`] porte le format, [`crate::channel`] le registre.
    Sends(&'static str),
    /// Ouvre une fenêtre Java que cette tranche ne décrit pas, par nom de classe : le
    /// sélecteur de ressources de Structurize (`WindowField.java:154-161`) et le
    /// sélecteur de requêtes (`WindowCrafting.java:185`, `WindowFurnaceCrafting.java:130`).
    OpensJava(&'static str),
    /// Ne fait rien d'autre qu'écrire un état local, que l'appelant envoie ensuite :
    /// aucune de ces quatre classes n'a ce cas, la variante est déclarée pour que
    /// [`Effect`] dise ce que le groupe ne fait pas.
    None,
}

impl Effect {
    /// Un nom stable, pour une ligne de log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Effect::Sends(class) => class,
            Effect::OpensJava(class) => class,
            Effect::None => "none",
        }
    }

    /// La classe Java qu'un `Sends` nomme, `None` pour les deux autres.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Effect::Sends(class) => Some(class),
            Effect::OpensJava(_) | Effect::None => None,
        }
    }
}

/// D'où vient l'id d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: le XML, ou nous
pub enum Naming {
    /// L'id est celui du XML, et la classe le compare à une chaîne
    /// (`WindowField.java:64`, `:118-122`).
    Xml,
    /// Le bouton n'a **aucun** id dans Java : la classe le crée par
    /// `Button.builder(...)` ou `new ImageButton(...)`, qui n'en donnent pas. Le nom est le
    /// nôtre, et il ne peut donc pas venir du jeu.
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

/// Un bouton de l'une de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id du bouton — celui du XML, ou le nôtre (voir [`Naming`]).
    pub id: &'static str,
    /// La clé du libellé ; `""` quand le bouton est une image sans texte (une flèche du
    /// champ, `windowfield.xml:20-27`, ou la bascule de recette, `WindowCrafting.java:173`).
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

/// Ce que le titre de la fenêtre affiche — **trois formes pour quatre titres**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois provenances de titre de ces quatre classes
pub enum Title {
    /// Une clé de traduction écrite **en dur** dans la classe, qui ignore le nom du paquet
    /// (`WindowCrafting.java:255`).
    Key(&'static str),
    /// Le libellé du XML, que la classe ne réécrit pas (`windowfield.xml:8`) :
    /// `label="$(block.minecolonies.blockhutfield)"`.
    Xml(&'static str),
    /// Un **littéral** que le serveur a mis dans le paquet, pas une clé : il ne se traduit
    /// pas (`OpenCraftingGUIMessage.java:82`, `TileEntityGrave.java:218`).
    Literal(&'static str),
}

/// Comment on arrive à la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un message serveur, ou un clic de bloc
pub enum Reached {
    /// Un message ouvre le conteneur, par nom de classe.
    Message(&'static str),
    /// Un clic de bloc ouvre la fenêtre : la variante de [`crate::blocks::Window`] que
    /// `crate::blocks::click` rend déjà.
    Block(crate::blocks::Window),
}

/// Le conteneur du jeu que la fenêtre affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois écrans de conteneur et une BOWindow
pub enum Menu {
    /// Pas un écran de conteneur : le champ est une `BOWindow`
    /// (`WindowField.java:59`, `AbstractWindowSkeleton`).
    None,
    /// `ContainerCrafting` (`:40`), ouvert par `OpenCraftingGUIMessage` en dernier recours
    /// (`:113-135`).
    CraftingTable,
    /// `ContainerCraftingFurnace` (`:43`), ouvert quand le module enseigne `smelting`.
    Furnace,
    /// `ContainerGrave` (`:16`), ouvert par `TileEntityGrave.createMenu`.
    Grave,
}

impl Menu {
    /// La classe Java du conteneur.
    pub fn java_container(self) -> Option<&'static str> {
        match self {
            Menu::None => None,
            Menu::CraftingTable => Some("ContainerCrafting"),
            Menu::Furnace => Some("ContainerCraftingFurnace"),
            Menu::Grave => Some("ContainerGrave"),
        }
    }
}

/// Ce que les deux fenêtres d'artisanat envoient dans `AddRemoveRecipeMessage` : le
/// `IRecipeStorage` que Java construit (`AddRemoveRecipeMessage.java:87-125`).
///
/// **Deux constructeurs, deux façons de poser le bloc intermédiaire** : celui de l'établi
/// le déduit de la taille de grille (`:96`, `gridSize == 1 ? FURNACE : AIR`) et celui du
/// four le reçoit en argument (`:112-125`). Le port garde les deux résultats dans un seul
/// type — [`Storage::intermediate`] vaut `None` pour l'air — et c'est [`crafting`] ou
/// [`furnace`] qui décide lequel des deux il imite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
    /// Les entrées, dans l'ordre des cases, vides comprises : Java envoie un `ItemStorage`
    /// par case lue, même vide (`WindowCrafting.java:229-236`).
    pub inputs: Vec<Option<&'static str>>,
    /// Le nombre d'exemplaires de chaque entrée : `ItemStackUtils.setSize(copy, 1)`
    /// (`:233`), donc toujours `1`.
    pub input_count: i32,
    /// La sortie principale (`:238`).
    pub primary: &'static str,
    /// Les sorties secondaires (`:239`, `menu.getRemainingItems()`).
    pub secondary: Vec<Option<&'static str>>,
    /// La taille de grille envoyée (`:244`).
    pub grid: i32,
    /// Le bloc intermédiaire, `None` pour l'air.
    pub intermediate: Option<&'static str>,
    /// L'id de producteur du module (`:244`).
    pub module_id: i32,
    /// `false` : les deux fenêtres ne font qu'ajouter une recette, jamais en retirer.
    pub remove: bool,
}

/// Une fenêtre : sa classe, son XML quand elle en a un, son titre, ses panneaux, ses
/// boutons, comment on y arrive et le conteneur qu'elle affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. Ce n'est ni un id de `src/ui` ni un id de la fenêtre en jeu.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML BlockUI, `None` pour les trois écrans de conteneur.
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
        self.own_panes().iter().find(|pane| pane.id == id).copied()
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

    /// Le nombre de panneaux nommés.
    pub fn pane_count(&self) -> usize {
        self.own_panes().len()
    }
}

/// Les quatre fenêtres de ce dossier.
pub const WINDOWS: &[Window] = &[
    Window {
        id: CRAFTING_ID,
        java: crafting::JAVA,
        resource: None,
        title: Title::Key(crafting::TITLE_KEY),
        panes: Panes::Own(crafting::FIELDS),
        buttons: crafting::BUTTONS,
        reached: Reached::Message("OpenCraftingGUIMessage"),
        menu: Menu::CraftingTable,
    },
    Window {
        id: FURNACE_ID,
        java: furnace::JAVA,
        resource: None,
        title: Title::Literal(furnace::TITLE),
        panes: Panes::Own(furnace::FIELDS),
        buttons: furnace::BUTTONS,
        reached: Reached::Message("OpenCraftingGUIMessage"),
        menu: Menu::Furnace,
    },
    Window {
        id: FIELD_ID,
        java: plot::JAVA,
        resource: Some(plot::RESOURCE),
        title: Title::Xml(plot::HEADER_KEY),
        panes: Panes::Own(plot::FIELDS),
        buttons: plot::BUTTONS,
        reached: Reached::Block(crate::blocks::Window::Field),
        menu: Menu::None,
    },
    Window {
        id: GRAVE_ID,
        java: grave::JAVA,
        resource: None,
        title: Title::Literal(grave::TITLE),
        panes: Panes::Own(grave::FIELDS),
        buttons: grave::BUTTONS,
        reached: Reached::Block(crate::blocks::Window::Grave),
        menu: Menu::Grave,
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

/// L'action de `src/ui` qui ouvrirait cette classe en `Effect::OpensJava`, avec son écran.
///
/// **Aucune ne l'est** : c'est le fait de ce dossier, et le test
/// [`tests::not_one_of_these_four_is_reached_from_a_hut_screen`] le vérifie pour les quatre.
/// La fonction existe pour la même raison que chez `crate::ui_window_08` — pour que le jour
/// où une des treize fenêtres de hutte nommerait cette classe, la jointure soit déjà là.
///
/// **On compare, on ne construit pas.** `Effect::OpensJava` porte une `&'static str` : un
/// `Effect::OpensJava(class)` avec un argument emprunté demanderait à `class` de vivre plus
/// longtemps que l'appel, alors que la question posée est « un des neuf effets nomme-t-il
/// cette classe ? ». La réponse tient dans une comparaison de chaînes.
pub fn opened_by_ui(class: &str) -> Option<(&'static crate::ui::Screen, crate::ui::Action)> {
    for screen in crate::ui::SCREENS.iter().copied() {
        for action in screen.actions {
            // Un seul effet de `src/ui` nomme une classe ; les huit autres ne répondent pas.
            let crate::ui::Effect::OpensJava(opened) = action.effect else {
                continue;
            };
            if opened == class {
                return Some((screen, *action));
            }
        }
    }
    None
}

/// La ligne du registre de `crate::channel` pour cette classe Java, quand le port la
/// connaît. Sert à prouver qu'aucun nom de message de cette table n'est inventé.
pub fn message_of(class: &str) -> Option<&'static crate::channel::Entry> {
    crate::channel::Registry::new().of_java(class)
}

/// Les panneaux d'une fenêtre sous la forme de `src/ui`, quand leur source est une source
/// de `src/ui`.
pub fn as_ui_fields(window: &Window) -> Option<Vec<Field>> {
    window.own_panes().iter().map(Pane::to_ui).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_windows_and_only_the_field_has_an_xml() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(!window.id.is_empty());
            assert!(window.java.starts_with("Window"), "{} n'est pas une classe de fenêtre", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenêtres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
            }
        }
        let with_xml: Vec<&str> =
            WINDOWS.iter().filter(|window| window.resource.is_some()).map(|window| window.id).collect();
        assert_eq!(with_xml, [FIELD_ID]);
        assert_eq!(
            window_by_id(FIELD_ID).map(|window| window.resource),
            Ok(Some(plot::RESOURCE))
        );
        assert!(plot::RESOURCE.ends_with("windowfield.xml"));
        // Trois conteneurs, un seul écran BlockUI.
        assert_eq!(WINDOWS.iter().filter(|window| window.menu != Menu::None).count(), 3);
        assert_eq!(window_by_id(FIELD_ID).map(|window| window.menu.java_container()), Ok(None));
        assert_eq!(
            window_by_id(CRAFTING_ID).map(|window| window.menu.java_container()),
            Ok(Some("ContainerCrafting"))
        );
        assert_eq!(
            window_by_id(GRAVE_ID).map(|window| window.menu.java_container()),
            Ok(Some("ContainerGrave"))
        );
    }

    #[test]
    fn not_one_of_these_four_is_reached_from_a_hut_screen() {
        // Aucune des treize fenêtres de src/ui ne nomme ces quatre classes.
        for window in WINDOWS {
            assert!(
                opened_by_ui(window.java).is_none(),
                "{} est atteinte depuis un écran de hutte",
                window.id
            );
        }
        // Deux par un message, deux par un clic de bloc.
        let by_message: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| matches!(window.reached, Reached::Message(_)))
            .map(|window| window.id)
            .collect();
        assert_eq!(by_message, [CRAFTING_ID, FURNACE_ID]);
        // Et c'est le **même** message : OpenCraftingGUIMessage choisit la fenêtre d'après
        // ce que le module enseigne (`:76-135`).
        assert_eq!(
            window_by_id(CRAFTING_ID).map(|window| window.reached),
            window_by_id(FURNACE_ID).map(|window| window.reached)
        );
        // Les deux écrans de bloc sont ceux que `crate::blocks` nomme déjà : le port les
        // joint par le type, pas par une chaîne recopiée.
        assert_eq!(
            window_by_id(FIELD_ID).map(|window| window.reached),
            Ok(Reached::Block(crate::blocks::Window::Field))
        );
        assert_eq!(
            window_by_id(GRAVE_ID).map(|window| window.reached),
            Ok(Reached::Block(crate::blocks::Window::Grave))
        );
        assert_ne!(
            crate::blocks::Window::Field,
            crate::blocks::Window::Grave,
            "deux blocs, deux écrans"
        );
        // Un refus nommé pour tout le reste.
        assert_eq!(window_by_java("WindowHutWorkerModulePlaceholder"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java(plot::JAVA).map(|window| window.id), Ok(FIELD_ID));
    }

    #[test]
    fn the_four_titles_come_from_four_places() {
        let titles: Vec<Title> = WINDOWS.iter().map(|window| window.title).collect();
        assert_eq!(
            titles,
            [
                Title::Key(crafting::TITLE_KEY),
                Title::Literal(furnace::TITLE),
                Title::Xml(plot::HEADER_KEY),
                Title::Literal(grave::TITLE),
            ]
        );
        // La clé de l'établi n'est pas celle du paquet : deux noms, un seul dessiné.
        assert_eq!(crafting::TITLE_KEY, "container.crafting");
        assert!(!crafting::TITLE_KEY.starts_with("com.minecolonies."));
        // Le titre du champ vient du XML, et c'est une clé de bloc.
        assert_eq!(plot::HEADER_KEY, "block.minecolonies.blockhutfield");
        // Les deux littéraux n'en sont pas.
        assert_eq!(furnace::TITLE, "Furnace Crafting GUI");
        assert_eq!(grave::TITLE, "Grave");
        assert_ne!(furnace::TITLE, grave::TITLE);
    }

    #[test]
    fn every_message_this_group_names_is_a_real_row_of_the_port() {
        let mut seen = 0;
        for window in WINDOWS {
            for button in window.buttons {
                let Some(class) = button.effect.message() else { continue };
                let entry = message_of(class);
                let Some(entry) = entry else {
                    panic!("{class} n'est pas une ligne du registre de crate::channel");
                };
                // Un message qu'une fenêtre envoie part **du client**. Trois arrivent au
                // serveur ; `FarmFieldPlotResizeMessage` est bidirectionnel
                // (`registry::ALL` le dit `Both`), et `comes_from` couvre les deux cas.
                assert!(
                    entry.comes_from(mod_api::LogicalSide::Client),
                    "{class} ne part pas du client"
                );
                assert!(entry.rust_type().is_some(), "{class} n'a pas de type dans le port");
                seen += 1;
            }
            if let Reached::Message(class) = window.reached {
                assert!(message_of(class).is_some(), "{class} est un nom inventé");
            }
        }
        assert!(seen >= 6, "les cinq boutons du champ et les deux « terminé »");
        // Le nom exact, et le type qui le porte — aucune chaîne recopiée à la main.
        assert_eq!(
            message_of("AddRemoveRecipeMessage").and_then(|entry| entry.rust_type()),
            Some("building_modules::AddRemoveRecipe")
        );
        assert_eq!(
            message_of(plot::RESIZE_MESSAGE).and_then(|entry| entry.rust_type()),
            Some("building_modules::FarmFieldPlotResize")
        );
        assert_eq!(
            message_of(plot::SEED_MESSAGE).and_then(|entry| entry.rust_type()),
            Some("building_modules::FarmFieldUpdateSeed")
        );
        assert_eq!(
            message_of("SwitchRecipeCraftingTeachingMessage").and_then(|entry| entry.rust_type()),
            Some("work::SwitchRecipeCraftingTeaching")
        );
        assert_eq!(
            message_of("OpenCraftingGUIMessage").and_then(|entry| entry.rust_type()),
            Some("buildings::OpenCraftingGUI")
        );
        // Les deux étapes du champ nomment leurs messages, et pas un de plus.
        assert_eq!(
            plot::Step::Resize { face: crate::network::building_modules::Face::North, value: 2 }
                .message(),
            Some(plot::RESIZE_MESSAGE)
        );
        assert_eq!(plot::Step::Seed("minecraft:wheat_seeds").message(), Some(plot::SEED_MESSAGE));
        assert_eq!(plot::Step::PickSeed.message(), None);
        // Un nom qui n'existe pas ne passe pas pour un message.
        assert!(message_of("WindowHireWorker").is_none());
        assert_eq!(Effect::None.name(), "none");
        assert_eq!(Effect::None.message(), None);
        assert_eq!(Effect::OpensJava("X").message(), None);
        assert_eq!(Effect::Sends("Y").message(), Some("Y"));
    }

    #[test]
    fn the_ten_buttons_are_five_from_the_xml_and_five_that_java_never_names() {
        let buttons: Vec<Button> =
            WINDOWS.iter().flat_map(|window| window.buttons.iter().copied()).collect();
        assert_eq!(buttons.len(), 10);
        let ours: Vec<&str> =
            buttons.iter().filter(|b| b.naming == Naming::Ours).map(|b| b.id).collect();
        assert_eq!(ours, ["done", "switch", "requests", "done", "requests"]);
        let xml: Vec<&str> =
            buttons.iter().filter(|b| b.naming == Naming::Xml).map(|b| b.id).collect();
        assert_eq!(xml.len(), 5);
        assert_eq!(xml[0], plot::BUTTON_SELECT_SEED);
        // Deux ids sont réutilisés par deux fenêtres, et c'est Java : `done` et `requests`
        // existent dans l'établi et dans le four, chacun avec ses propres refus.
        assert_eq!(buttons.iter().filter(|button| button.id == "done").count(), 2);
        let crafting_done =
            window_by_id(CRAFTING_ID).ok().and_then(|window| window.button("done"));
        let furnace_done = window_by_id(FURNACE_ID).ok().and_then(|window| window.button("done"));
        assert!(crafting_done.is_some() && furnace_done.is_some());
        assert_ne!(
            crafting_done.map(|button| button.refusals),
            furnace_done.map(|button| button.refusals),
            "le four refuse une entrée vide en plus"
        );
        // Les libellés : sept boutons sans texte, trois avec une clé.
        let silent: Vec<&str> = buttons.iter().filter(|b| b.label_key().is_none()).map(|b| b.id).collect();
        assert_eq!(silent.len(), 7, "deux « requests », la bascule, et les quatre flèches");
        for id in &silent {
            assert!(
                *id == "switch" || *id == "requests" || id.starts_with("dir-resize"),
                "{id} n'a pas de texte sans raison"
            );
        }
        let labelled: Vec<&str> =
            buttons.iter().filter(|b| b.label_key().is_some()).map(|b| b.id).collect();
        assert_eq!(labelled, ["done", "done", plot::BUTTON_SELECT_SEED]);
        assert_eq!(Naming::Xml.name(), "xml");
        assert_eq!(Naming::Ours.name(), "ours");
    }

    #[test]
    fn one_window_has_nine_panes_and_three_have_none() {
        let counts: Vec<usize> = WINDOWS.iter().map(Window::pane_count).collect();
        assert_eq!(counts, [0, 0, 9, 0]);
        // Aucun panneau de ligne : les quatre fenêtres n'ont pas de liste.
        for window in WINDOWS {
            for (index, pane) in window.own_panes().iter().enumerate() {
                assert!(!pane.is_row_pane(), "{}: {} est une ligne", window.id, pane.id);
                for other in &window.own_panes()[..index] {
                    assert_ne!(pane.id, other.id, "{} répète {}", window.id, pane.id);
                }
            }
        }
        // Et aucune de leurs sources n'est une source de src/ui : ce sont des lecteurs que
        // ce module a dû ajouter, sauf le citoyen du panneau de l'exploitant.
        let field = match window_by_id(FIELD_ID) {
            Ok(window) => window,
            Err(refusal) => panic!("{refusal}"),
        };
        let ported = field
            .own_panes()
            .iter()
            .filter(|pane| pane.source.is_ported_here())
            .count();
        assert_eq!(ported, 8, "seul current-farmer lit un citoyen, comme src/ui le nomme");
        // La conversion vers src/ui échoue sur ce panneau-là, comme sur les huit autres.
        assert_eq!(as_ui_fields(field), None);
        // Un écran de hutte, lui, se convertit.
        let worker = crate::ui::screen_by_id(crate::ui::screen::HUT_WORKER_ID);
        let worker = match worker {
            Ok(screen) => screen,
            Err(refusal) => panic!("{refusal}"),
        };
        let first = match worker.fields.first() {
            Some(field) => *field,
            None => panic!("pas de panneau"),
        };
        assert_eq!(Pane::from_ui(first).to_ui(), Some(first));
    }

    #[test]
    fn the_four_ids_collide_with_neither_src_ui_nor_ui_window_08() {
        for window in WINDOWS {
            assert_eq!(
                crate::ui::screen_by_id(window.id).map(|screen| screen.id),
                Err(crate::ui::Refusal::UnknownScreen),
                "{} usurpe un id de src/ui",
                window.id
            );
            assert_eq!(
                crate::ui_window_08::window_by_id(window.id).map(|other| other.id),
                Err(crate::ui_window_08::Refusal::UnknownWindow),
                "{} usurpe un id du dossier 08",
                window.id
            );
        }
        // Et les quatre fenêtres du dossier 08 ne sont pas les nôtres.
        for other in crate::ui_window_08::WINDOWS {
            assert!(window_by_id(other.id).is_err(), "{} est dans les deux tables", other.id);
        }
    }
}
