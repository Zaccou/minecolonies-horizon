//! Les quatre fenetres de l'hotel de ville qui ne sont pas la page d'accueil : les
//! reglages, les statistiques, le pacte de fondation et le refus de fonder.
//!
//! Java reference : `core/client/gui/townhall/` —
//! `WindowSettings.java` (70 lignes), `WindowStatsPage.java` (298),
//! `WindowTownHallColonyManage.java` (87), `WindowTownHallCantCreateColony.java` (52),
//! la classe mere qui porte les sept onglets (`AbstractWindowTownHall.java`, 70 lignes),
//! les quatre XML sous `src/main/resources/assets/minecolonies/gui/townhall/` et les cinq
//! messages qui les ouvrent ou leur repondent.
//!
//! **Meme forme que `src/ui`, pas une deuxieme.** Un panneau est un [`Pane`]
//! ([`Pane::to_ui`] rend un `crate::ui::Field`), une valeur un [`crate::ui::field::Value`],
//! une portee un [`Scope`], un effet un [`crate::ui::action::Effect`]. Ce qui est propre a
//! ce module est ce qu'aucune table n'a : [`Click::Closes`] (le mot que `Effect` n'a pas),
//! [`Tab`] (la barre d'onglets de l'hotel de ville, sept entrees dont deux seulement sont
//! decrites ici), [`Pane::declared`] (le XML et la ligne qui creent l'id), et trois
//! sources que `src/ui` n'a pas.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`pane`] | un panneau : ce qu'il porte, d'ou vient sa valeur, la ligne qui le declare |
//! | [`settings`] | cinq bascules booleennes, deux infobulles chacune, un message par clic |
//! | [`stats`] | trois plafonds de population, un tableau de metiers, une periode |
//! | [`founding`] | le pacte, son annee, la colonie voisine, et les six refus de fonder |
//! | [`refusal`] | dix-sept refus en quatre especes, dont deux plantages et quatre silences |
//!
//! # Les comptes, mesures
//!
//! | Quoi | Combien | Ou |
//! | --- | --- | --- |
//! | fenetres decrites | **4** | [`WINDOWS`] |
//! | dont pages de l'hotel de ville | 2 | [`Window::shell`] |
//! | onglets de la barre | **7** | [`TABS`] |
//! | onglets qui menent a une fenetre d'ici | 2 (statistiques, reglages) | [`TABS`] |
//! | panneaux | **14** | 5 + 5 + 1 + 3 |
//! | dont listes | 2 | `citizen-stats`, `stats` |
//! | boutons | **9** | 5 bascules + 0 + 2 + 2 |
//! | boutons qui envoient un message | 7 | 5 `TriggerSettingMessage`, `CreateColonyMessage`, `PickUpTownhallMessage` |
//! | boutons qui ferment la fenetre | 4 | 2 croix, 1 ramassage, 1 fondation |
//! | valeurs de `src/ui` employees | 4 | `Text`, `Row`, `Occupancy`, `Input` |
//! | sources de `src/ui` employees | 4 sur 9 | `Colony`, `Citizen`, `Hut`, `Module` |
//! | refus | **17** | [`Refusal::ALL`] |
//! | messages serveur nommes | 3 | `TriggerSettingMessage`, `CreateColonyMessage`, `PickUpTownhallMessage` |
//!
//! # Les quatre premisses du brief que le source corrige
//!
//! 1. **« les champs affiches (nom, niveau, progression, stock) »** : **aucun des
//!    quatre** n'est sur ces fenetres. `AbstractBuildingMainWindow` cherche `name`
//!    (`:53`) et `build` (`:54`) puis **verifie avant d'ecrire** (`:135`, `:171`), et
//!    les quatre XML d'ici ne declarent ni l'un ni l'autre : le seul ecran de l'hotel
//!    de ville qui porte le titre et le bouton de construction est celui des actions
//!    (`layoutactions.xml:6`, `:15`). Ce que ces fenetres affichent est une population
//!    (`n / plafond`), cinq etats booleens, une annee, une distance et une alerte.
//! 2. **« les actions (embaucher, renvoyer, prioriser) »** ne sont sur aucune des quatre :
//!    ce sont les boutons d'un ecran de hutte, que `src/ui` porte deja
//!    (`Action::guarded("hire", …)`, `A_RECALL`, `A_PRIO_UP`). Les neuf boutons d'ici
//!    basculent un reglage, envoient le pacte, ramassent le bloc ou ferment.
//! 3. **« l'etat initial et les transitions »** n'est pas un seul etat : il y en a
//!    quatre, un par fenetre, et deux d'entre eux viennent du **message d'ouverture**
//!    ([`Source::Packet`]) et non d'un modele. Le nom du pacte n'est pas toujours celui
//!    du joueur, et le champ `colonyname` n'est jamais vide a l'ouverture.
//! 4. **« les quatre fichiers du brief » ne forment pas un ecran** : deux sont des pages
//!    de l'hotel de ville (`settings`, `happiness`) et deux sont des fenetres autonomes
//!    sans barre d'onglets (`AbstractWindowSkeleton`, `:22`, `:30`). Les comparer comme
//!    quatre pages de la meme chose force a inventer un titre et une barre qui n'y sont
//!    pas.
//!
//! # Ce qui n'est pas ici, et ce que chaque trou coute
//!
//! - **le dessin** : ni position, ni taille, ni sprite (`TRANCHE.md` §3 M2). Les sept
//!   rubans et les sept pastilles de cire sont nommes par [`Tab`], pas dessines.
//! - **l'envoi** : sept boutons portent le **nom** d'un message et rien n'est envoye ;
//!   il n'y a ni client ni paquet dans ce port.
//! - **les trois plafonds de population** demandent la limite configuree du serveur
//!   (`ServerConfiguration.java:129`), qui n'a pas de contrepartie dans le port : elle
//!   arrive en argument ([`stats::citizen_cap`]).
//! - **les deux compteurs de citoyens** (enfants, chomeurs) que
//!   `WindowStatsPage.java:159-171` calcule : `crate::colony::Citizen` ne porte qu'un id
//!   et un nom, donc `isChild()` et `getJobView()` n'ont pas de reponse ici, et la
//!   fenetre les prend en argument ([`stats::population`]).
//! - **la liste deroulante** de la periode : `src/ui::Value` n'a pas de valeur pour elle ;
//!   son choix vit dans [`stats::PageState`], pas dans un panneau. `MANQUANT.md` §2 dit
//!   ce qu'une valeur de plus aurait ete.

pub mod founding;
pub mod pane;
pub mod refusal;
pub mod settings;
pub mod stats;

pub use pane::{Pane, Source};
pub use refusal::{Kind, Refusal};

use crate::ui::action::Effect;
use crate::ui::field::Scope;

/// L'id des reglages dans cette table.
pub const SETTINGS_ID: &str = "townhall.settings";

/// L'id des statistiques.
pub const STATS_ID: &str = "townhall.stats";

/// L'id de l'alerte de fondation impossible.
pub const ALERT_ID: &str = "townhall.cant_found";

/// L'id du pacte de fondation.
pub const MANAGE_ID: &str = "townhall.manage";

/// Ce qu'un clic fait, avec le vocabulaire de `src/ui` la ou il suffit.
///
/// **`Closes` est le seul mot que `Effect` n'a pas.** `Effect::Reopens` dit « rouvre
/// l'ecran qui m'a ouvert » (`WindowAssignCitizen.java:174-178`) ; la croix d'une
/// fenetre autonome ne rouvre rien : elle se ferme (`WindowTownHallCantCreateColony.java:34`,
/// `:50`, `WindowTownHallColonyManage.java:51`, `:85`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois effets de src/ui, plus la fermeture
pub enum Click {
    /// Envoie un message au serveur, par nom de classe, et ferme la fenetre quand Java
    /// le fait dans le meme gestionnaire (`:49-50`, `:78-85`).
    Sends { message: &'static str, closes: bool },
    /// Ouvre une fenetre de cette table, par [`Window::id`].
    Opens(&'static str),
    /// Ouvre une fenetre que cette table ne decrit pas, par nom de classe : les cinq
    /// autres pages de l'hotel de ville.
    OpensJava(&'static str),
    /// Ferme la fenetre, sans rien envoyer ni rouvrir.
    Closes,
}

impl Click {
    /// L'effet equivalent de `src/ui` ; `None` pour [`Click::Closes`], qui n'y a pas de
    /// mot.
    pub fn effect(self) -> Option<Effect> {
        match self {
            Click::Sends { message, .. } => Some(Effect::Sends(message)),
            Click::Opens(id) => Some(Effect::Opens(id)),
            Click::OpensJava(class) => Some(Effect::OpensJava(class)),
            Click::Closes => None,
        }
    }

    /// Vrai quand le gestionnaire ferme la fenetre a la fin.
    pub fn closes(self) -> bool {
        matches!(self, Click::Closes) || matches!(self, Click::Sends { closes: true, .. })
    }

    /// Vrai quand le clic envoie un message. **Faux pour la liste deroulante** : elle
    /// n'est pas un bouton et son gestionnaire ne fait que relire.
    pub fn sends_message(self) -> bool {
        matches!(self, Click::Sends { .. })
    }
}

/// Un bouton d'une de ces fenetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id du XML, celui que `registerButton` lie
    /// (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// La cle du libelle ; `""` pour une croix sans texte
    /// (`windowcantfoundcolony.xml:5`).
    pub label: &'static str,
    /// Ce que le clic fait.
    pub click: Click,
    /// La fenetre, ou la `view` d'un reglage.
    pub scope: Scope,
    /// Chaque facon dont le clic est refuse, dans l'ordre ou le serveur les donne. **Vide
    /// pour les neuf** : aucun de ces boutons ne se refuse a lui-meme, et deux d'entre eux
    /// (les deux croix) ne peuvent pas etre refuses — ils ferment. Les refus d'un envoi
    /// sont ceux du message, et ils sont dans [`Refusal`] : les six de la fondation, les
    /// quatre du ramassage, le silence d'un reglage inactif.
    pub refusals: &'static [Refusal],
    /// Le fichier XML et la ligne qui declarent l'id, 1-based.
    pub declared: (&'static str, u32),
}

impl Button {
    /// La cle du libelle, `None` pour un bouton qui n'est qu'une image.
    pub fn label_key(&self) -> Option<&'static str> {
        if self.label.is_empty() {
            None
        } else {
            Some(self.label)
        }
    }

    /// Vrai quand le clic ferme la fenetre.
    pub fn closes(&self) -> bool {
        self.click.closes()
    }

    /// Vrai quand le clic envoie un message au serveur.
    pub fn sends_message(&self) -> bool {
        self.click.sends_message()
    }
}

/// Ce qu'ouvre un onglet de la barre de l'hotel de ville.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: une page decrite ici, ou une fenetre que la table ne decrit pas
pub enum Target {
    /// Une fenetre de [`WINDOWS`], par id.
    Page(&'static str),
    /// Une classe Java que cette table ne decrit pas.
    Java(&'static str),
}

/// Un onglet de la barre laterale (`AbstractWindowTownHall.java:20-26`,
/// `windowtownhall.xml:5-70`).
///
/// L'id est celui de la **pastille de cire** ; le ruban ouvert est `id1`, l'image
/// cachee `id0` et le ruban au survol `idExt` — trois panneaux par onglet, que
/// `AbstractWindowTownHall.java:29-34` manipule pour n'en laisser qu'un ouvert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    /// L'id du bouton (`windowtownhall.xml:57-70`).
    pub id: &'static str,
    /// La cle du libelle (`windowtownhall.xml:14`, `:17`, …).
    pub label: &'static str,
    /// Ce que le clic ouvre.
    pub target: Target,
    /// La ligne du bouton dans le XML.
    pub declared: u32,
}

impl Tab {
    /// L'image cachee de l'onglet courant, `getWindowId() + "0"` (`:31`).
    pub fn image_id(self) -> String {
        format!("{}0", self.id)
    }

    /// La pastille, cachee elle aussi (`:32`).
    pub fn button_id(self) -> String {
        self.id.to_owned()
    }

    /// Le ruban ouvert de l'onglet courant, `getWindowId() + "1"` (`:34`).
    pub fn ribbon_id(self) -> String {
        format!("{}1", self.id)
    }

    /// Le ruban au survol (`onHoverId`, `windowtownhall.xml:57`).
    pub fn hover_id(self) -> String {
        format!("{}Ext", self.id)
    }
}

/// Les sept onglets, dans l'ordre ou `AbstractWindowTownHall` les enregistre
/// (`:20-26`).
///
/// **Deux seulement menent a une fenetre decrite ici.** Les cinq autres sont des pages
/// de l'hotel de ville que ce module ne porte pas, nommees par leur classe plutot que
/// decrites : c'est l'honnete reponse de [`Target::Java`].
///
/// L'onglet des statistiques s'appelle **`happiness`** (`WindowConstants.java:62`) alors
/// que son libelle dit « Statistics » (`windowtownhall.xml:26`) : le nom vient de la
/// place qu'il occupait, pas de son contenu.
pub const TABS: [Tab; 7] = [
    Tab {
        id: "actions",
        label: "com.minecolonies.coremod.gui.townhalltab.actions",
        target: Target::Java("WindowMainPage"),
        declared: 57,
    },
    Tab {
        id: "infopage",
        label: "com.minecolonies.coremod.gui.townhalltab.information",
        target: Target::Java("WindowInfoPage"),
        declared: 59,
    },
    Tab {
        id: "permissions",
        label: "com.minecolonies.coremod.gui.townhalltab.permissions",
        target: Target::Java("WindowPermissionsPage"),
        declared: 61,
    },
    Tab {
        id: "citizens",
        label: "com.minecolonies.coremod.gui.townhalltab.citizens",
        target: Target::Java("WindowCitizenPage"),
        declared: 63,
    },
    Tab {
        id: stats::TAB_ID,
        label: stats::TAB_KEY,
        target: Target::Page(STATS_ID),
        declared: 65,
    },
    Tab {
        id: "alliances",
        label: "com.minecolonies.coremod.gui.townhalltab.alliances",
        target: Target::Java("WindowAlliancePage"),
        declared: 67,
    },
    Tab {
        id: settings::TAB_ID,
        label: settings::TAB_KEY,
        target: Target::Page(SETTINGS_ID),
        declared: 69,
    },
];

/// Par quelle porte on arrive a une fenetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois portes dans ces quatre fenetres
pub enum Door {
    /// Un onglet de la barre, par son id ([`TABS`]).
    Tab(&'static str),
    /// Un message du serveur, par nom de classe : la fenetre n'existe pas avant lui.
    Message(&'static str),
    /// Un bouton d'une autre fenetre, par classe et ligne.
    Button { from: &'static str, at: u32 },
}

/// Une fenetre : sa classe, son XML, son titre, ses panneaux, ses boutons, ses portes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. Ce n'est pas un id de `src/ui`.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML BlockUI, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// La cle du titre, quand la fenetre en a un. **`None` pour l'alerte** : son XML
    /// n'a ni titre ni `text` de titre (`windowcantfoundcolony.xml:1-13`).
    pub title: Option<&'static str>,
    /// Les panneaux que la classe remplit.
    pub panes: &'static [Pane],
    /// Les boutons que la classe enregistre elle-meme. **La barre d'onglets n'est pas
    /// ici** : c'est la classe mere qui l'enregistre ([`TABS`]).
    pub buttons: &'static [Button],
    /// Vrai quand la fenetre herite d'`AbstractWindowTownHall` et porte donc la barre.
    pub shell: bool,
    /// Par ou l'on entre, dans l'ordre ou Java les enumere.
    pub doors: &'static [Door],
}

impl Window {
    /// Le panneau de cet id, copie — le premier, comme `crate::ui::Screen::field`.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copie.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Vrai quand la fenetre montre au moins un panneau de ce genre.
    pub fn shows(&self, value: crate::ui::field::Value) -> bool {
        self.panes.iter().any(|pane| pane.value == value)
    }

    /// Vrai quand un bouton de la fenetre ferme la fenetre.
    pub fn closes_on_a_button(&self) -> bool {
        self.buttons.iter().any(Button::closes)
    }
}

/// Les boutons de l'alerte (`WindowTownHallCantCreateColony.java:34-35`).
const ALERT_BUTTONS: &[Button] = &[
    Button {
        id: founding::CANT_CANCEL,
        label: "",
        click: Click::Closes,
        scope: Scope::Window,
        refusals: &[],
        declared: ("windowcantfoundcolony.xml", 5),
    },
    Button {
        id: founding::PICKUP,
        label: founding::PICKUP_LABEL_KEY,
        click: Click::Sends { message: founding::PICKUP_MESSAGE, closes: true },
        scope: Scope::Window,
        refusals: &[],
        declared: ("windowcantfoundcolony.xml", 9),
    },
];

/// Les boutons du pacte (`WindowTownHallColonyManage.java:51-52`).
const MANAGE_BUTTONS: &[Button] = &[
    Button {
        id: founding::MANAGE_CANCEL,
        label: "",
        click: Click::Closes,
        scope: Scope::Window,
        refusals: &[],
        declared: ("windowcolonymanagement.xml", 5),
    },
    Button {
        id: founding::CREATE,
        label: founding::CREATE_LABEL_KEY,
        click: Click::Sends { message: founding::CREATE_MESSAGE, closes: true },
        scope: Scope::Window,
        refusals: &[],
        declared: ("windowcolonymanagement.xml", 23),
    },
];

/// Les quatre fenetres de ce module.
pub static WINDOWS: &[Window] = &[
    Window {
        id: SETTINGS_ID,
        java: settings::JAVA,
        resource: settings::RESOURCE,
        title: Some(settings::TAB_KEY),
        panes: &settings::PANES,
        buttons: &settings::BUTTONS,
        shell: true,
        // WindowSettings.java:66-69 : l'onglet des reglages se cache lui-meme.
        doors: &[Door::Tab(settings::TAB_ID)],
    },
    Window {
        id: STATS_ID,
        java: stats::JAVA,
        resource: stats::RESOURCE,
        title: Some(stats::TAB_KEY),
        panes: &stats::PANES,
        // Aucun bouton : la liste deroulante est un `setHandler` (`:264`), pas un
        // `registerButton`.
        buttons: &[],
        shell: true,
        doors: &[Door::Tab(stats::TAB_ID)],
    },
    Window {
        id: ALERT_ID,
        java: founding::CANT_JAVA,
        resource: founding::CANT_RESOURCE,
        title: None,
        panes: &founding::ALERT_PANES,
        buttons: ALERT_BUTTONS,
        shell: false,
        // OpenCantFoundColonyWarningMessage.java:50 : le message porte le texte et le
        // drapeau de l'infobulle, la fenetre ne sait rien d'autre.
        doors: &[Door::Message("OpenCantFoundColonyWarningMessage")],
    },
    Window {
        id: MANAGE_ID,
        java: founding::MANAGE_JAVA,
        resource: founding::MANAGE_RESOURCE,
        title: Some(founding::MANAGE_TITLE_KEY),
        panes: &founding::MANAGE_PANES,
        buttons: MANAGE_BUTTONS,
        shell: false,
        // Deux portes : la fondation (OpenColonyFoundingCovenantMessage.java:40) et la
        // reactivation, qui rouvre la meme fenetre depuis une autre
        // (WindowTownHallColonyReactivate.java:60).
        doors: &[
            Door::Message("OpenColonyFoundingCovenantMessage"),
            Door::Button { from: "WindowTownHallColonyReactivate", at: 60 },
        ],
    },
];

/// La fenetre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenetre de cette classe Java, ou [`Refusal::UnknownWindow`] : la jointure inverse
/// refuse les cinq pages que ce module ne decrit pas.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

/// L'onglet de cet id, ou [`Refusal::UnknownTab`].
pub fn tab_by_id(id: &str) -> Result<Tab, Refusal> {
    TABS.iter().copied().find(|tab| tab.id == id).ok_or(Refusal::UnknownTab)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    #[test]
    fn four_windows_with_their_own_class_xml_and_id() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(window.java.starts_with("Window"), "{}", window.id);
            assert!(
                window.resource.starts_with("minecolonies:gui/townhall/"),
                "{} lit {}",
                window.id,
                window.resource
            );
            assert!(window.resource.ends_with(".xml"));
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenetres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
                assert_ne!(window.resource, other.resource, "deux fenetres, un XML");
            }
            // Aucun id d'ici n'est un ecran de src/ui, et aucun n'est une hutte.
            assert_eq!(
                crate::ui::screen_by_id(window.id).map(|screen| screen.id),
                Err(crate::ui::Refusal::UnknownScreen),
                "{} usurpe un id de src/ui",
                window.id
            );
            assert_eq!(window_by_id(window.id).map(|found| found.id), Ok(window.id));
            assert_eq!(window_by_java(window.java).map(|found| found.id), Ok(window.id));
        }
        // Deux refus nommes pour le reste.
        assert_eq!(window_by_id("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowHireWorker"), Err(Refusal::UnknownWindow));
        // Les deux pages de l'hotel de ville nomment la meme classe mere.
        let pages: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.shell)
            .map(|window| window.java)
            .collect();
        assert_eq!(pages, ["WindowSettings", "WindowStatsPage"]);
    }

    #[test]
    fn seven_tabs_and_two_of_them_lead_into_this_table() {
        assert_eq!(TABS.len(), 7);
        let pages: Vec<&str> = TABS
            .iter()
            .filter_map(|tab| match tab.target {
                Target::Page(id) => Some(id),
                Target::Java(_) => None,
            })
            .collect();
        assert_eq!(pages, [STATS_ID, SETTINGS_ID], "WindowStatsPage puis WindowSettings");
        let classes: Vec<&str> = TABS
            .iter()
            .filter_map(|tab| match tab.target {
                Target::Java(class) => Some(class),
                Target::Page(_) => None,
            })
            .collect();
        assert_eq!(
            classes,
            [
                "WindowMainPage",
                "WindowInfoPage",
                "WindowPermissionsPage",
                "WindowCitizenPage",
                "WindowAlliancePage",
            ],
            "AbstractWindowTownHall.java:20-26"
        );
        // Le piege du nom : l'onglet des statistiques s'appelle happiness.
        assert_eq!(TABS[4].id, "happiness");
        assert_eq!(TABS[4].id, stats::TAB_ID);
        assert_ne!(TABS[4].id, "stats");
        assert_eq!(TABS[4].label, "com.minecolonies.coremod.gui.townhalltab.stats");
        // Les sept ids sont distincts et chacun nomme ses quatre panneaux XML.
        let mut ids: Vec<&str> = TABS.iter().map(|tab| tab.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 7);
        for tab in TABS {
            assert_eq!(tab.image_id(), format!("{}0", tab.id));
            assert_eq!(tab.ribbon_id(), format!("{}1", tab.id));
            assert_eq!(tab.hover_id(), format!("{}Ext", tab.id));
            assert_eq!(tab.button_id(), tab.id);
            assert!(tab.declared > 0);
            assert_eq!(tab_by_id(tab.id).map(|found| found.id), Ok(tab.id));
        }
        assert_eq!(tab_by_id("happiness0"), Err(Refusal::UnknownTab));
        assert_eq!(tab_by_id("statistics"), Err(Refusal::UnknownTab));
    }

    #[test]
    fn only_the_two_pages_carry_the_tab_bar_and_only_the_alert_has_no_title() {
        let shells: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.shell)
            .map(|window| window.id)
            .collect();
        assert_eq!(shells, [SETTINGS_ID, STATS_ID]);
        // Une fenetre autonome n'a pas de barre : aucune de ses portes n'est un onglet.
        for window in WINDOWS.iter().filter(|window| !window.shell) {
            assert!(
                !window.doors.iter().any(|door| matches!(door, Door::Tab(_))),
                "{} est autonome et a une porte d'onglet",
                window.id
            );
            assert!(window.closes_on_a_button(), "{} ne se ferme pas", window.id);
        }
        // L'alerte n'a aucun titre ; les trois autres en ont un.
        let untitled: Vec<&str> =
            WINDOWS.iter().filter(|window| window.title.is_none()).map(|window| window.id).collect();
        assert_eq!(untitled, [ALERT_ID]);
        for window in WINDOWS.iter().filter(|window| window.title.is_some()) {
            let title = window.title.unwrap_or("");
            assert!(title.starts_with("com.minecolonies."), "{} : {title}", window.id);
        }
        assert_eq!(settings::TAB_ID, "settings", "WindowConstants.java:42");
        assert_eq!(stats::TAB_ID, "happiness", "WindowConstants.java:62");
    }

    #[test]
    fn nine_buttons_four_of_them_closing_and_seven_sending() {
        let total: usize = WINDOWS.iter().map(|window| window.buttons.len()).sum();
        assert_eq!(total, 9, "5 bascules + 0 statistiques + 2 + 2");
        let closing = WINDOWS
            .iter()
            .flat_map(|window| window.buttons.iter())
            .filter(|button| button.closes())
            .count();
        assert_eq!(closing, 4, "2 croix, 1 ramassage, 1 fondation");
        let sending = WINDOWS
            .iter()
            .flat_map(|window| window.buttons.iter())
            .filter(|button| button.sends_message())
            .count();
        assert_eq!(sending, 7, "5 TriggerSettingMessage + 2 du pacte");
        for window in WINDOWS {
            for button in window.buttons {
                // Chaque clic a un effet, sauf la fermeture qui n'a pas de mot dans src/ui.
                if button.click == Click::Closes {
                    assert_eq!(button.click.effect(), None);
                } else {
                    assert!(button.click.effect().is_some(), "{}.{}", window.id, button.id);
                }
                // Aucun bouton ne refuse par lui-meme (les refus sont ceux du message).
                assert!(button.refusals.is_empty(), "{}.{}", window.id, button.id);
                assert!(button.declared.1 > 0);
                assert!(
                    window.resource.ends_with(button.declared.0),
                    "{}.{} ne vient pas de {}",
                    window.id,
                    button.id,
                    window.resource
                );
                assert_eq!(button.label_key().is_some(), !button.label.is_empty());
            }
            // Deux boutons portent le meme id dans deux fenetres, jamais dans une.
            for (index, button) in window.buttons.iter().enumerate() {
                for other in &window.buttons[index + 1..] {
                    assert!(
                        button.id != other.id || button.scope != other.scope,
                        "{}: deux fois {} dans la meme portee",
                        window.id,
                        button.id
                    );
                }
            }
        }
        // Deux croix sans libelle, et cinq bascules dont le texte est ecrit par la classe.
        let bare: Vec<&str> = WINDOWS
            .iter()
            .flat_map(|window| window.buttons.iter())
            .filter(|button| button.label_key().is_none())
            .map(|button| button.id)
            .collect();
        assert_eq!(bare, ["trigger", "trigger", "trigger", "trigger", "trigger", "cancel", "cancel"]);
        assert_eq!(
            settings::BUTTONS[0].label_key(),
            None,
            "le XML met Off, la classe ecrit On ou Off : pas de cle fixe"
        );
    }

    #[test]
    fn fourteen_panes_each_declared_by_its_own_window() {
        let total: usize = WINDOWS.iter().map(|window| window.panes.len()).sum();
        assert_eq!(total, 14, "5 reglages + 5 statistiques + 1 alerte + 3 pacte");
        for window in WINDOWS {
            assert!(!window.panes.is_empty(), "{} ne montre rien", window.id);
            for pane in window.panes {
                assert!(pane.declared.1 > 0, "{} sans ligne", pane.id);
                assert!(
                    window.resource.ends_with(pane.declared.0),
                    "{}.{} lit {} hors de {}",
                    window.id,
                    pane.id,
                    pane.declared.0,
                    window.resource
                );
            }
        }
        // Deux pannes de source differente sur deux fenetres differentes.
        let stats = window_by_id(STATS_ID).expect("les statistiques");
        assert_eq!(stats.pane("totalCitizens").map(|pane| pane.value), Some(Value::Occupancy));
        assert_eq!(stats.pane(stats::DROPDOWN), None, "la liste deroulante n'est pas un panneau");
        assert_eq!(stats.button(stats::DROPDOWN), None);
        assert_eq!(stats.button(stats::DROPDOWN_BUTTON), None);
        let manage = window_by_id(MANAGE_ID).expect("le pacte");
        assert_eq!(manage.pane(founding::NAME_INPUT).map(|pane| pane.value), Some(Value::Input));
        assert_eq!(manage.pane(founding::COVENANT_YEAR).map(|pane| pane.source), Some(Source::Clock));
        let alert = window_by_id(ALERT_ID).expect("l'alerte");
        assert_eq!(alert.panes.len(), 1);
        assert_eq!(alert.pane(founding::ALERT_TEXT).map(|pane| pane.source), Some(Source::Packet));
    }

    #[test]
    fn every_nested_pane_names_a_parent_of_its_own_window() {
        for window in WINDOWS {
            let lists: Vec<&str> = window
                .panes
                .iter()
                .filter(|pane| pane.value == Value::Row)
                .map(|pane| pane.id)
                .collect();
            let views: Vec<&str> = settings::Row::ALL.iter().map(|row| row.view()).collect();
            for pane in window.panes {
                let Scope::Row(parent) = pane.scope else {
                    continue;
                };
                assert!(
                    lists.contains(&parent) || (window.id == SETTINGS_ID && views.contains(&parent)),
                    "{}.{} est dans {parent}, qui n'est ni une liste ni une view de cette fenetre",
                    window.id,
                    pane.id
                );
            }
        }
        // Les deux listes de la page des statistiques, et les cinq views des reglages.
        let stats = window_by_id(STATS_ID).expect("les statistiques");
        let lists: Vec<&str> = stats
            .panes
            .iter()
            .filter(|pane| pane.value == Value::Row)
            .map(|pane| pane.id)
            .collect();
        assert_eq!(lists, [stats::CITIZEN_LIST, stats::STATS_LIST]);
        let settings = window_by_id(SETTINGS_ID).expect("les reglages");
        assert_eq!(settings.panes.len(), 5);
        for (index, row) in settings::Row::ALL.iter().enumerate() {
            assert_eq!(settings.panes[index].id, settings::TRIGGER);
            assert_eq!(settings.panes[index].scope, Scope::Row(row.view()));
        }
    }

    #[test]
    fn no_window_of_the_town_hall_shows_a_name_a_level_a_progress_or_a_stock() {
        // La troisieme premisse du brief, tenue : les quatre XML d'ici ne declarent ni
        // `name` ni `build` (layoutactions.xml:6, :15 les a pour la page d'accueil).
        for window in WINDOWS {
            assert_eq!(window.pane("name"), None, "{} a un titre de batiment", window.id);
            assert_eq!(window.pane("build"), None, "{} a un bouton de construction", window.id);
            assert!(!window.shows(Value::Name), "{}", window.id);
            assert!(!window.shows(Value::Level), "{}", window.id);
            assert!(!window.shows(Value::Progress), "{}", window.id);
            assert!(!window.shows(Value::Stock), "{}", window.id);
            assert_eq!(window.button("build"), None, "{}", window.id);
        }
        // Ce qu'elles montrent vraiment : quatre valeurs sur douze.
        let mut values: Vec<Value> = WINDOWS
            .iter()
            .flat_map(|window| window.panes.iter().map(|pane| pane.value))
            .collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values, [Value::Occupancy, Value::Row, Value::Input, Value::Text]);
        // La seule valeur calculee est la ligne des habitants.
        let computed: Vec<&str> = WINDOWS
            .iter()
            .flat_map(|window| window.panes.iter())
            .filter(|pane| pane.value.is_computed())
            .map(|pane| pane.id)
            .collect();
        assert_eq!(computed, [stats::TOTAL_CITIZENS]);
    }

    #[test]
    fn four_doors_and_the_reactivation_opens_the_pact_from_another_window() {
        let doors: usize = WINDOWS.iter().map(|window| window.doors.len()).sum();
        assert_eq!(doors, 5, "2 onglets + 1 message + 2 portes du pacte");
        for window in WINDOWS {
            assert!(!window.doors.is_empty(), "{} n'a pas de porte", window.id);
            for door in window.doors {
                match door {
                    Door::Tab(id) => {
                        assert_eq!(tab_by_id(id).map(|tab| tab.target), Ok(Target::Page(window.id)));
                    }
                    Door::Message(class) => assert!(class.starts_with("Open"), "{class}"),
                    Door::Button { from, at } => {
                        assert!(from.starts_with("Window"), "{from}");
                        assert!(*at > 0);
                        assert_ne!(*from, window.java, "une fenetre ne s'ouvre pas elle-meme");
                    }
                }
            }
        }
        let manage = window_by_id(MANAGE_ID).expect("le pacte");
        assert_eq!(manage.doors.len(), 2);
        assert_eq!(
            manage.doors[1],
            Door::Button { from: "WindowTownHallColonyReactivate", at: 60 }
        );
        // L'alerte n'a qu'une porte : le message, qui porte deja son texte.
        let alert = window_by_id(ALERT_ID).expect("l'alerte");
        assert_eq!(alert.doors, [Door::Message("OpenCantFoundColonyWarningMessage")]);
    }
}
