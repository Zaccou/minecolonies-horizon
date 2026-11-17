//! Les quatre pages, comme donnee : titre, panneaux, boutons, et la barre laterale.
//!
//! Java reference : `core/client/gui/townhall/` — `WindowMainPage.java` (409
//! lignes), `WindowInfoPage.java` (311), `WindowCitizenPage.java` (297),
//! `WindowPermissionsPage.java` (751), et `AbstractWindowTownHall.java` (61)
//! pour la barre laterale que les quatre partagent. Les quatre XML sont sous
//! `assets/minecolonies/gui/townhall/`, et la barre est `windowtownhall.xml`,
//! posee par `<layout source=.../>` en tete de chacun.
//!
//! **Un panneau, un bouton : une declaration.** Les quatre pages n'ont presque
//! rien en commun (deux ids `name` de fenetre, un `recallone`), donc les
//! tableaux sont ecrits une fois par page plutot que mis en constantes
//! partagees comme dans `src/ui` : la ou `src/ui` a treize ecrans sur cinq
//! boutons communs, l'hotel de ville a quatre pages sur deux.
//!
//! **Le bouton porte sa portee**, ce que [`crate::ui::action::Action`] ne fait
//! pas : la page des permissions pose un bouton `removeBlock` **dans** la ligne
//! `blocks` (`layoutpermissions.xml:106`), et quatre listes de la meme fenetre
//! nomment leur ligne `name` (`:15`, `:41`, `:84`, `:105`). Un id seul ne suffit
//! donc plus a designer un bouton, exactement comme il ne suffit plus a designer
//! un panneau.

use crate::ui::action::{Action, Effect};
use crate::ui::field::{Scope, Value};

use super::pane::{Pane, Reader};
use super::refusal::Refusal;

/// Les cles de libelle que la table cite, verbatim du XML ou de
/// `TranslationConstants`.
pub const EDIT_KEY: &str = "com.minecolonies.gui.edit";
pub const CURTOWNHALL_KEY: &str = "com.minecolonies.coremod.gui.townhall.currtownhallname";
pub const MERCENARY_KEY: &str = "com.minecolonies.coremod.gui.townhall.mercenary";
/// L'XML ecrit `townHall` avec un H majuscule (`layoutactions.xml:19`), et
/// `TranslationConstants` n'a pas cette cle : l'orthographe est celle de l'XML.
pub const MAP_KEY: &str = "com.minecolonies.coremod.gui.townHall.map";
pub const PATREON_KEY: &str = "com.minecolonies.coremod.gui.townhall.patreon";
pub const BANNER_KEY: &str = "com.minecolonies.coremod.gui.townhall.edit_flag";
pub const RESET_KEY: &str = "com.minecolonies.gui.reset";
pub const RECALL_ONE_KEY: &str = "com.minecolonies.coremod.gui.townhall.recallone";
pub const ADD_KEY: &str = "com.minecolonies.coremod.gui.townhall.add";
pub const REMOVE_RANK_KEY: &str = "com.minecolonies.coremod.gui.townhall.removerank";
/// Les libelles du tourne-page, poses par `AbstractWindowSkeleton.java:81-82`.
pub const PREV_KEY: &str = "com.minecolonies.core.gui.prevpage";
pub const NEXT_KEY: &str = "com.minecolonies.core.gui.nextpage";

/// L'id de la vue qui n'est pas un enfant du `switch` : les panneaux de la
/// fenetre elle-meme. Les trois pages sans `switch` n'ont que celle-la.
pub const WINDOW: &str = "window";
/// Les trois enfants du `switch id="pages"` (`layoutpermissions.xml:5`, `:54`, `:95`).
pub const ADD_PLAYER_VIEW: &str = "pageAddPlayer";
pub const MANAGE_RANKS_VIEW: &str = "managePermissions";
pub const FREE_BLOCK_VIEW: &str = "freeBlockList";

/// Les ids des quatre pages dans cette table. Ce ne sont pas ceux de Java :
/// Java n'a pas d'id de page, il a un `getWindowId()` qui rend l'id de l'onglet.
pub const MAIN_ID: &str = "hall.actions";
pub const INFO_ID: &str = "hall.info";
pub const PERMISSIONS_ID: &str = "hall.permissions";
pub const CITIZENS_ID: &str = "hall.citizens";

/// Ce qu'un clic fait, en trois especes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set
pub enum Act {
    /// L'effet de `src/ui` : ouvrir une fenetre, envoyer un message.
    Ui(Effect),
    /// Un clic qui ne bouge que la fenetre : selectionner, tourner une page,
    /// basculer un booleen.
    Local(Local),
    /// Un gestionnaire que Java enregistre et laisse vide :
    /// `registerButton(BUTTON_CHANGE_SPEC, this::doNothing)`
    /// (`WindowMainPage.java:96`).
    Nothing,
}

/// Un clic local, qui n'envoie rien et n'ouvre rien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set
pub enum Local {
    /// Selectionne la ligne cliquee : un citoyen (`WindowCitizenPage.java:113-119`),
    /// un joueur en ligne (`WindowPermissionsPage.java:187-191`), un rang
    /// (`:408-420`).
    Selects,
    /// Tourne une page du `switch` (`AbstractWindowSkeleton.java:87-89`).
    CyclesView,
    /// Bascule un booleen local : le bouton `trigger` d'une permission
    /// (`WindowPermissionsPage.java:582-597`).
    Toggles,
}

/// Un bouton d'une page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id de bouton, ce que `registerButton` lie
    /// (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// La cle du libelle ; `""` pour un bouton sans texte ou dont la classe
    /// ecrit le texte — la convention de [`crate::ui::action::Action::label`].
    pub label: &'static str,
    /// La fenetre, ou la ligne de liste qui porte ce bouton.
    pub scope: Scope,
    /// Ce que le clic fait.
    pub act: Act,
    /// Chaque facon dont le clic est refuse, dans l'ordre ou Java les teste.
    pub refuses: &'static [Refusal],
}

impl Button {
    /// Un bouton de la fenetre, sans libelle ni refus.
    pub const fn plain(id: &'static str, act: Act) -> Button {
        Button { id, label: "", scope: Scope::Window, act, refuses: &[] }
    }

    /// Un bouton de la fenetre dont l'XML donne le libelle.
    pub const fn labelled(id: &'static str, label: &'static str, act: Act) -> Button {
        Button { id, label, scope: Scope::Window, act, refuses: &[] }
    }

    /// Un bouton que Java refuse.
    pub const fn guarded(
        id: &'static str,
        label: &'static str,
        act: Act,
        refuses: &'static [Refusal],
    ) -> Button {
        Button { id, label, scope: Scope::Window, act, refuses }
    }

    /// Un bouton d'une ligne de la liste `list`.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        label: &'static str,
        act: Act,
        refuses: &'static [Refusal],
    ) -> Button {
        Button { id, label, scope: Scope::Row(list), act, refuses }
    }

    /// Vrai quand Java peut refuser le clic.
    pub fn is_guarded(&self) -> bool {
        !self.refuses.is_empty()
    }

    /// Le bouton sous la forme de `src/ui`.
    ///
    /// `None` des qu'il refuse quelque chose, ou qu'il ne fait rien dans le
    /// vocabulaire de `src/ui` : `ui::action::Refusal` est celui des huttes, et
    /// un refus de l'hotel de ville n'y a pas de variante. **Sept** boutons sur
    /// vingt-huit se replient : les six de la page d'accueil, et la croix de la
    /// page d'information, que Java n'a jamais gardee (`WindowInfoPage.java:224-237`).
    pub fn to_ui(&self) -> Option<Action> {
        match self.act {
            Act::Ui(effect) if self.refuses.is_empty() => {
                Some(Action::plain(self.id, self.label, effect))
            }
            _ => None,
        }
    }
}

/// Une vue : la fenetre, ou un enfant du `switch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    /// [`WINDOW`] pour les panneaux de la fenetre, sinon l'id du `view` de l'XML.
    pub id: &'static str,
    pub panes: &'static [Pane],
    pub buttons: &'static [Button],
}

impl View {
    /// Vrai pour les panneaux de la fenetre elle-meme.
    pub fn is_window_level(&self) -> bool {
        self.id == WINDOW
    }
}

/// Ce que le titre lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two shapes among these four pages
pub enum Title {
    /// Le nom de la colonie (`WindowMainPage.java:371`).
    Colony,
    /// Rien : l'XML n'a pas de panneau de titre et la classe n'en ecrit pas —
    /// les trois autres pages de ce module.
    None,
}

/// Une page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: &'static str,
    /// La classe qui la construit.
    pub window: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// Ce que `getWindowId()` rend : l'id de l'onglet que la page eteint.
    pub window_id: &'static str,
    pub title: Title,
    pub views: &'static [View],
}

impl Page {
    /// Tous les panneaux, toutes vues confondues.
    pub fn panes(&self) -> Vec<&'static Pane> {
        let mut all = Vec::new();
        for view in self.views {
            all.extend(view.panes.iter());
        }
        all
    }

    /// Tous les boutons, toutes vues confondues.
    pub fn buttons(&self) -> Vec<&'static Button> {
        let mut all = Vec::new();
        for view in self.views {
            all.extend(view.buttons.iter());
        }
        all
    }

    /// Le panneau de cet `id` dans cette portee, s'il existe.
    pub fn pane(&self, id: &str, scope: Scope) -> Option<&'static Pane> {
        self.panes().into_iter().find(|pane| pane.id() == id && pane.scope() == scope)
    }

    /// Le bouton de cet `id` dans cette portee.
    pub fn button(&self, id: &str, scope: Scope) -> Option<&'static Button> {
        self.buttons().into_iter().find(|button| button.id == id && button.scope == scope)
    }

    /// La vue de cet id.
    pub fn view(&self, id: &str) -> Option<&'static View> {
        self.views.iter().find(|view| view.id == id)
    }

    /// Les enfants du `switch`, dans l'ordre de l'XML.
    pub fn switched(&self) -> Vec<&'static View> {
        self.views.iter().filter(|view| !view.is_window_level()).collect()
    }

    /// Vrai quand la page montre au moins un panneau de ce genre.
    pub fn shows(&self, value: Value) -> bool {
        self.panes().iter().any(|pane| pane.value() == value)
    }
}

// ---------------------------------------------------------------------------
// Un onglet de la barre laterale.
// ---------------------------------------------------------------------------

/// Ce qu'un onglet ouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a page of this table, or a class this table does not describe
pub enum Target {
    /// Une page de [`PAGES`], par son id.
    Page(&'static str),
    /// Une fenetre de Java que ce module ne decrit pas, par son nom de classe
    /// (`AbstractWindowTownHall.java:40-42`).
    Outside(&'static str),
}

/// Un des sept onglets que **toute** page de l'hotel de ville enregistre
/// (`AbstractWindowTownHall.java:36-42`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    /// L'id du bouton, qui est aussi le `getWindowId()` de la page qu'il ouvre.
    pub id: &'static str,
    /// La cle du libelle (`windowtownhall.xml:14-33`).
    pub key: &'static str,
    pub target: Target,
}

/// Les sept onglets, **dans l'ordre ou le XML les dessine**
/// (`windowtownhall.xml:5-11`) : c'est celui que la barre laterale montre, et
/// il n'est pas celui de l'enregistrement — `AbstractWindowTownHall.java:36-42`
/// enregistre `settings` avant `alliances`, et l'XML les dessine dans l'autre
/// sens.
pub static TABS: &[Tab] = &[
    Tab {
        id: "actions",
        key: "com.minecolonies.coremod.gui.townhalltab.actions",
        target: Target::Page(MAIN_ID),
    },
    Tab {
        id: "infopage",
        key: "com.minecolonies.coremod.gui.townhalltab.information",
        target: Target::Page(INFO_ID),
    },
    Tab {
        id: "permissions",
        key: "com.minecolonies.coremod.gui.townhalltab.permissions",
        target: Target::Page(PERMISSIONS_ID),
    },
    Tab {
        id: "citizens",
        key: "com.minecolonies.coremod.gui.townhalltab.citizens",
        target: Target::Page(CITIZENS_ID),
    },
    // `BUTTON_STATS` vaut "happiness" et ouvre `WindowStatsPage` : l'id ne dit
    // pas le nom de la classe.
    Tab {
        id: "happiness",
        key: "com.minecolonies.coremod.gui.townhalltab.stats",
        target: Target::Outside("WindowStatsPage"),
    },
    Tab {
        id: "alliances",
        key: "com.minecolonies.coremod.gui.townhalltab.alliances",
        target: Target::Outside("WindowAlliancePage"),
    },
    Tab {
        id: "settings",
        key: "com.minecolonies.coremod.gui.townhalltab.settings",
        target: Target::Outside("WindowSettings"),
    },
];

/// L'onglet de cet id.
pub fn tab(id: &str) -> Result<&'static Tab, Refusal> {
    TABS.iter().find(|tab| tab.id == id).ok_or(Refusal::UnknownWindowId)
}

/// La page qu'un onglet ouvre, ou le refus nomme.
pub fn open_tab(id: &str) -> Result<&'static Page, Refusal> {
    match tab(id)?.target {
        Target::Page(page) => page_by_id(page),
        Target::Outside(window) => Err(Refusal::PageOutsideThisSlice { window }),
    }
}

/// L'etat des trois panneaux d'un onglet de la barre laterale.
///
/// Java pose trois panneaux par onglet : l'image `id0` (le petit ruban), le
/// bouton `id`, et le bouton `id1` (le grand ruban)
/// (`windowtownhall.xml:5-33`). A l'ouverture d'une page,
/// `AbstractWindowTownHall.java:44-47` eteint les deux premiers de **son**
/// onglet et allume le troisieme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub tab: &'static str,
    pub short_ribbon: bool,
    pub icon: bool,
    pub ribbon: bool,
}

/// Les sept onglets, l'actif deplie.
pub fn sidebar(window_id: &str) -> Result<Vec<Bookmark>, Refusal> {
    tab(window_id)?;
    Ok(TABS
        .iter()
        .map(|tab| {
            let active = tab.id == window_id;
            Bookmark { tab: tab.id, short_ribbon: !active, icon: !active, ribbon: active }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Page d'accueil — `layoutactions.xml`.
// ---------------------------------------------------------------------------

const MAIN_PANES: &[Pane] = &[
    // `WindowMainPage.java:371` ecrit a l'ouverture ; le libelle est le `text`
    // sans id de `layoutactions.xml:4-5`.
    Pane::labelled(Reader::TownHall, "name", CURTOWNHALL_KEY, Value::Name, ("layoutactions.xml", 6)),
    // Le texte du bouton « actions » : nom d'affichage + niveau (`:93-94`).
    //
    // C'est le **niveau de l'hotel de ville**, et il n'a pas de panneau a lui :
    // Java l'ajoute au texte du bouton, la ou le titre de `src/ui` l'ajoute au
    // nom (`AbstractBuildingMainWindow.java:174-175`). Deux pages, deux endroits.
    Pane::new(Reader::TownHall, "actions1", Value::Text, ("layoutactions.xml", 13)),
    Pane::new(Reader::Colony, "colonyStylePicker", Value::Text, ("layoutactions.xml", 47)),
    Pane::new(Reader::Colony, "colorPicker", Value::Count, ("layoutactions.xml", 32)),
    Pane::new(Reader::Colony, "textureStylePicker", Value::Count, ("layoutactions.xml", 59)),
    Pane::new(Reader::Colony, "nameStylePicker", Value::Count, ("layoutactions.xml", 78)),
];

const MAIN_BUTTONS: &[Button] = &[
    // `:96` : le gestionnaire ne fait rien.
    Button::plain("changeSpec", Act::Nothing),
    // `:385` : l'entree de nom de l'hotel de ville, pas celle de la hutte.
    Button::labelled("rename", EDIT_KEY, Act::Ui(Effect::OpensJava("WindowTownHallNameEntry"))),
    // `:393`, refuse pendant la recharge (`:373-377`).
    Button::guarded(
        "mercenaries",
        MERCENARY_KEY,
        Act::Ui(Effect::OpensJava("WindowTownHallMercenary")),
        &[Refusal::MercenaryCooldown],
    ),
    Button::labelled("map", MAP_KEY, Act::Ui(Effect::OpensJava("WindowColonyMap"))),
    // `:356` : une confirmation de lien vanilla vers patreon.com.
    Button::labelled("patreon", PATREON_KEY, Act::Ui(Effect::OpensJava("ConfirmLinkScreen"))),
    // `:123` : la fenetre de choix de pack est dans Structurize ; son texte est
    // le nom du pack courant (`:104`), donc pas de cle.
    Button::labelled(
        "colonyStylePicker",
        "",
        Act::Ui(Effect::OpensJava("WindowSwitchPack")),
    ),
    Button::labelled("bannerPicker", BANNER_KEY, Act::Ui(Effect::OpensJava("WindowBannerPicker"))),
    // `:249` : renvoie le pack de texture numero zero.
    Button::labelled(
        "resettexture",
        RESET_KEY,
        Act::Ui(Effect::Sends("ColonyTextureStyleMessage")),
    ),
];

static MAIN: Page = Page {
    id: MAIN_ID,
    window: "WindowMainPage",
    resource: "minecolonies:gui/townhall/layoutactions.xml",
    window_id: "actions",
    title: Title::Colony,
    views: &[View { id: WINDOW, panes: MAIN_PANES, buttons: MAIN_BUTTONS }],
};

// ---------------------------------------------------------------------------
// Page d'information — `layoutinfo.xml`.
// ---------------------------------------------------------------------------

const INFO_PANES: &[Pane] = &[
    // Le menu des intervalles, ecrit a la construction (`WindowInfoPage.java:82`).
    Pane::new(Reader::Intervals, "intervals", Value::Count, ("layoutinfo.xml", 5)),
    Pane::new(Reader::ColonyEvents, "eventsList", Value::Row, ("layoutinfo.xml", 16)),
    Pane::in_row(Reader::ColonyEvents, "eventsList", "action", Value::Text, ("layoutinfo.xml", 18)),
    Pane::in_row(Reader::ColonyEvents, "eventsList", "name", Value::Name, ("layoutinfo.xml", 19)),
    Pane::in_row(Reader::ColonyEvents, "eventsList", "pos", Value::Text, ("layoutinfo.xml", 20)),
    Pane::new(Reader::WorkOrders, "workOrderList", Value::Row, ("layoutinfo.xml", 27)),
    Pane::in_row(Reader::WorkOrders, "workOrderList", "assignee", Value::Text, ("layoutinfo.xml", 34)),
    Pane::in_row(Reader::WorkOrders, "workOrderList", "work", Value::Text, ("layoutinfo.xml", 36)),
    // L'id de l'ordre, ecrit dans une ligne et relu au clic (`:193`, `:226`).
    Pane::in_row(Reader::WorkOrders, "workOrderList", "hiddenId", Value::Count, ("layoutinfo.xml", 39)),
];

const INFO_BUTTONS: &[Button] = &[
    // `:201-205` : leve la priorite au-dessus de la ligne precedente.
    Button::in_row(
        "workOrderList",
        "plus",
        "\u{21a5}",
        Act::Ui(Effect::Sends("WorkOrderChangeMessage")),
        &[Refusal::FirstRowHasNoAbove],
    ),
    // `:206-210` : descend sous la ligne suivante.
    Button::in_row(
        "workOrderList",
        "minus",
        "\u{21a7}",
        Act::Ui(Effect::Sends("WorkOrderChangeMessage")),
        &[Refusal::LastRowHasNoBelow],
    ),
    // `:224-237` : retire la ligne **avant** d'envoyer, et n'a aucune garde.
    Button::in_row(
        "workOrderList",
        "delete",
        "X",
        Act::Ui(Effect::Sends("WorkOrderChangeMessage")),
        &[],
    ),
    // `:162` : cette page cache le bouton de chaque ligne, toujours.
    Button::in_row("eventsList", "addfakeplayer", ADD_KEY, Act::Nothing, &[]),
];

static INFO: Page = Page {
    id: INFO_ID,
    window: "WindowInfoPage",
    resource: "minecolonies:gui/townhall/layoutinfo.xml",
    window_id: "infopage",
    title: Title::None,
    views: &[View { id: WINDOW, panes: INFO_PANES, buttons: INFO_BUTTONS }],
};

// ---------------------------------------------------------------------------
// Page des citoyens — `layoutcitizens.xml`.
// ---------------------------------------------------------------------------

const CITIZEN_PANES: &[Pane] = &[
    // `:232` : le titre de la joie, avec le nombre arrondi au plafond.
    Pane::new(Reader::Happiness, "happinessTitle", Value::Text, ("layoutcitizens.xml", 4)),
    Pane::new(Reader::Happiness, "happinessList", Value::Row, ("layoutcitizens.xml", 5)),
    Pane::in_row(Reader::Happiness, "happinessList", "icon", Value::Icon, ("layoutcitizens.xml", 7)),
    Pane::in_row(Reader::Happiness, "happinessList", "name", Value::Name, ("layoutcitizens.xml", 8)),
    // `:140` : le metier, en gras, du citoyen selectionne.
    Pane::new(Reader::Citizens, "job", Value::Text, ("layoutcitizens.xml", 12)),
    // `:131-138` : deux images pour un panneau.
    Pane::new(Reader::Citizens, "gender", Value::Icon, ("layoutcitizens.xml", 13)),
    Pane::new(Reader::Citizens, "health", Value::Text, ("layoutcitizens.xml", 15)),
    Pane::new(Reader::Citizens, "happinessLevel", Value::Text, ("layoutcitizens.xml", 16)),
    Pane::new(Reader::Citizens, "saturation", Value::Text, ("layoutcitizens.xml", 17)),
    // `:146-152` : l'icone de l'entite, montree seulement si elle dort.
    Pane::new(Reader::Citizens, "entity", Value::Icon, ("layoutcitizens.xml", 23)),
    // `:81-88` : le filtre, relu a chaque frappe.
    Pane::new(Reader::Citizens, "search", Value::Input, ("layoutcitizens.xml", 28)),
    Pane::new(Reader::Citizens, "citizenList", Value::Row, ("layoutcitizens.xml", 29)),
    Pane::in_row(Reader::Citizens, "citizenList", "name", Value::Name, ("layoutcitizens.xml", 31)),
];

const CITIZEN_BUTTONS: &[Button] = &[
    // `:160-167` : renvoie **un** citoyen, celui qui est selectionne.
    Button::guarded(
        "recallone",
        RECALL_ONE_KEY,
        Act::Ui(Effect::Sends("RecallSingleCitizenMessage")),
        &[Refusal::NoCitizenSelected],
    ),
    // `:113-119` : selectionne la ligne ; le bouton de la ligne deja
    // selectionnee est eteint (`:200-207`).
    Button::in_row("citizenList", "name", "", Act::Local(Local::Selects), &[]),
];

static CITIZENS: Page = Page {
    id: CITIZENS_ID,
    window: "WindowCitizenPage",
    resource: "minecolonies:gui/townhall/layoutcitizens.xml",
    window_id: "citizens",
    title: Title::None,
    views: &[View { id: WINDOW, panes: CITIZEN_PANES, buttons: CITIZEN_BUTTONS }],
};

// ---------------------------------------------------------------------------
// Page des permissions — `layoutpermissions.xml`, trois vues.
// ---------------------------------------------------------------------------

/// Les panneaux de la fenetre : le compteur de page du squelette
/// (`AbstractWindowSkeleton.java:247-250`). Les deux fleches sont des boutons
/// du squelette aussi, pas de la page.
const PERMISSION_WINDOW_PANES: &[Pane] =
    &[Pane::new(Reader::PageCounter, "pageNum", Value::Step, ("layoutpermissions.xml", 113))];

const PERMISSION_WINDOW_BUTTONS: &[Button] = &[
    Button::guarded(
        "prevPage",
        PREV_KEY,
        Act::Local(Local::CyclesView),
        &[Refusal::FirstPage],
    ),
    Button::guarded(
        "nextPage",
        NEXT_KEY,
        Act::Local(Local::CyclesView),
        &[Refusal::LastPage],
    ),
];

const ADD_PLAYER_PANES: &[Pane] = &[
    Pane::new(Reader::Permissions, "addPlayerName", Value::Input, ("layoutpermissions.xml", 7)),
    Pane::new(Reader::Permissions, "users", Value::Row, ("layoutpermissions.xml", 13)),
    Pane::in_row(Reader::Permissions, "users", "name", Value::Name, ("layoutpermissions.xml", 15)),
    // `:661-666` : le proprietaire n'a pas de menu, il a un texte.
    Pane::in_row(Reader::Permissions, "users", "rank", Value::Text, ("layoutpermissions.xml", 17)),
    Pane::in_row(Reader::Permissions, "users", "rankPicker", Value::Count, ("layoutpermissions.xml", 18)),
    Pane::new(Reader::PermissionEvents, "eventsList", Value::Row, ("layoutpermissions.xml", 38)),
    Pane::in_row(Reader::PermissionEvents, "eventsList", "action", Value::Text, ("layoutpermissions.xml", 40)),
    Pane::in_row(Reader::PermissionEvents, "eventsList", "name", Value::Name, ("layoutpermissions.xml", 41)),
    Pane::in_row(Reader::PermissionEvents, "eventsList", "pos", Value::Text, ("layoutpermissions.xml", 42)),
    Pane::new(Reader::Permissions, "playerPicker", Value::Row, ("layoutpermissions.xml", 47)),
];

const ADD_PLAYER_BUTTONS: &[Button] = &[
    Button::guarded(
        "addPlayer",
        "+",
        Act::Ui(Effect::Sends("PermissionsMessage.AddPlayer")),
        &[Refusal::MayNotAddPlayer],
    ),
    // `:163-194` : ouvre la liste des joueurs en ligne, sans envoyer.
    Button::plain("addOnlinePlayer", Act::Local(Local::Selects)),
    Button::in_row(
        "users",
        "removePlayer",
        "",
        Act::Ui(Effect::Sends("PermissionsMessage.RemovePlayer")),
        &[Refusal::OwnerRankImmutable],
    ),
    Button::in_row(
        "eventsList",
        "addfakeplayer",
        ADD_KEY,
        Act::Ui(Effect::Sends("PermissionsMessage.AddPlayerOrFakePlayer")),
        &[Refusal::MayNotAddPlayer],
    ),
    // `:187-191` : remplit le champ du nom avec le joueur choisi.
    Button::in_row("playerPicker", "playerSelectButton", "", Act::Local(Local::Selects), &[]),
];

const MANAGE_RANKS_PANES: &[Pane] = &[
    Pane::new(Reader::Permissions, "addRankName", Value::Input, ("layoutpermissions.xml", 56)),
    Pane::new(Reader::Permissions, "rankButtonList", Value::Row, ("layoutpermissions.xml", 59)),
    Pane::in_row(Reader::Permissions, "rankButtonList", "rankId", Value::Count, ("layoutpermissions.xml", 61)),
    Pane::new(Reader::Permissions, "rankTypePicker", Value::Count, ("layoutpermissions.xml", 67)),
    Pane::new(Reader::Permissions, "rankList", Value::Row, ("layoutpermissions.xml", 82)),
    Pane::in_row(Reader::Permissions, "rankList", "name", Value::Name, ("layoutpermissions.xml", 84)),
    Pane::in_row(Reader::Permissions, "rankList", "index", Value::Count, ("layoutpermissions.xml", 88)),
];

const MANAGE_RANKS_BUTTONS: &[Button] = &[
    Button::guarded(
        "buttonAddRank",
        "+",
        Act::Ui(Effect::Sends("PermissionsMessage.AddRank")),
        &[Refusal::MayNotAddRank, Refusal::RankNameTaken],
    ),
    // `:408-420` : selectionne le rang, sans envoyer.
    Button::in_row("rankButtonList", "rankButton", "", Act::Local(Local::Selects), &[]),
    Button::guarded(
        "removeRank",
        REMOVE_RANK_KEY,
        Act::Ui(Effect::Sends("PermissionsMessage.RemoveRank")),
        &[Refusal::OwnerRankImmutable],
    ),
    // `:572-598` : bascule le bit, puis envoie.
    Button::in_row(
        "rankList",
        "trigger",
        "",
        Act::Ui(Effect::Sends("PermissionsMessage.Permission")),
        &[Refusal::RankBitsDerived],
    ),
];

const FREE_BLOCK_PANES: &[Pane] = &[
    Pane::new(Reader::Permissions, "addBlockName", Value::Input, ("layoutpermissions.xml", 99)),
    Pane::new(Reader::FreeBlocks, "blocks", Value::Row, ("layoutpermissions.xml", 103)),
    Pane::in_row(Reader::FreeBlocks, "blocks", "name", Value::Name, ("layoutpermissions.xml", 105)),
];

const FREE_BLOCK_BUTTONS: &[Button] = &[
    Button::guarded(
        "addBlock",
        "+",
        Act::Ui(Effect::Sends("ChangeFreeToInteractBlockMessage")),
        &[Refusal::MayNotAddPlayer, Refusal::UnknownBlockOrPosition],
    ),
    // `:482-485` : donne le sceptre de permissions.
    Button::guarded(
        "blockTool",
        "",
        Act::Ui(Effect::Sends("GiveToolMessage")),
        &[Refusal::MayNotAddPlayer],
    ),
    Button::in_row(
        "blocks",
        "removeBlock",
        "",
        Act::Ui(Effect::Sends("ChangeFreeToInteractBlockMessage")),
        &[Refusal::MayNotAddPlayer],
    ),
];

static PERMISSIONS: Page = Page {
    id: PERMISSIONS_ID,
    window: "WindowPermissionsPage",
    resource: "minecolonies:gui/townhall/layoutpermissions.xml",
    window_id: "permissions",
    title: Title::None,
    views: &[
        View { id: WINDOW, panes: PERMISSION_WINDOW_PANES, buttons: PERMISSION_WINDOW_BUTTONS },
        View { id: ADD_PLAYER_VIEW, panes: ADD_PLAYER_PANES, buttons: ADD_PLAYER_BUTTONS },
        View { id: MANAGE_RANKS_VIEW, panes: MANAGE_RANKS_PANES, buttons: MANAGE_RANKS_BUTTONS },
        View { id: FREE_BLOCK_VIEW, panes: FREE_BLOCK_PANES, buttons: FREE_BLOCK_BUTTONS },
    ],
};

/// Les quatre pages de ce module.
pub static PAGES: &[&Page] = &[&MAIN, &INFO, &PERMISSIONS, &CITIZENS];

/// La page de cet id.
pub fn page_by_id(id: &str) -> Result<&'static Page, Refusal> {
    PAGES.iter().copied().find(|page| page.id == id).ok_or(Refusal::UnknownWindowId)
}

/// La page de cette classe Java.
pub fn page_by_java(window: &str) -> Result<&'static Page, Refusal> {
    PAGES.iter().copied().find(|page| page.window == window).ok_or(Refusal::UnknownWindowId)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str) -> &'static Page {
        page_by_id(id).expect("page of this table")
    }

    #[test]
    fn four_pages_four_classes_four_xml_and_no_two_share_a_name() {
        assert_eq!(PAGES.len(), 4);
        for (index, page) in PAGES.iter().enumerate() {
            assert!(page.resource.starts_with("minecolonies:gui/townhall/layout"));
            assert!(page.resource.ends_with(".xml"));
            assert_eq!(page_by_java(page.window).map(|found| found.id), Ok(page.id));
            for other in &PAGES[index + 1..] {
                assert_ne!(page.id, other.id);
                assert_ne!(page.window, other.window);
                assert_ne!(page.resource, other.resource);
                assert_ne!(page.window_id, other.window_id);
            }
        }
        // The main page is the only one with a title, and it is the colony's name.
        let titled: Vec<&str> =
            PAGES.iter().filter(|page| page.title != Title::None).map(|page| page.id).collect();
        assert_eq!(titled, [MAIN_ID]);
        assert_eq!(page(MAIN_ID).title, Title::Colony);
    }

    #[test]
    fn seven_tabs_and_three_of_them_leave_this_slice() {
        assert_eq!(TABS.len(), 7, "AbstractWindowTownHall.java:36-42 registers seven");
        let pages = TABS.iter().filter(|tab| matches!(tab.target, Target::Page(_))).count();
        assert_eq!(pages, 4);
        for tab in TABS {
            assert!(tab.key.starts_with("com.minecolonies.coremod.gui.townhalltab."));
            // Each tab opens the page whose `getWindowId()` is its own id.
            if let Target::Page(id) = tab.target {
                assert_eq!(page(id).window_id, tab.id, "{id} would leave its own bookmark lit");
            }
        }
        assert_eq!(
            open_tab("happiness"),
            Err(Refusal::PageOutsideThisSlice { window: "WindowStatsPage" }),
            "BUTTON_STATS is `happiness` and opens a window this module does not describe"
        );
        assert_eq!(open_tab("nope"), Err(Refusal::UnknownWindowId));
        assert_eq!(open_tab("citizens").map(|page| page.window), Ok("WindowCitizenPage"));
    }

    #[test]
    fn the_page_that_is_open_lights_its_long_ribbon_and_hides_its_icon() {
        let marks = sidebar("permissions").expect("a tab of this table");
        assert_eq!(marks.len(), 7);
        let lit: Vec<&str> = marks.iter().filter(|mark| mark.ribbon).map(|mark| mark.tab).collect();
        assert_eq!(lit, ["permissions"], "AbstractWindowTownHall.java:47 shows id1");
        for mark in &marks {
            assert_eq!(mark.ribbon, mark.tab == "permissions");
            assert_eq!(mark.icon, mark.tab != "permissions");
            assert_eq!(mark.short_ribbon, mark.tab != "permissions");
            assert_ne!(mark.icon, mark.ribbon, "the two never show together");
        }
        assert_eq!(sidebar("nowhere"), Err(Refusal::UnknownWindowId));
    }

    #[test]
    fn the_permissions_page_is_the_only_one_with_switch_children() {
        assert_eq!(page(PERMISSIONS_ID).switched().len(), 3);
        let ids: Vec<&str> = page(PERMISSIONS_ID).switched().iter().map(|view| view.id).collect();
        assert_eq!(ids, [ADD_PLAYER_VIEW, MANAGE_RANKS_VIEW, FREE_BLOCK_VIEW]);
        assert_eq!(page(PERMISSIONS_ID).views.len(), 4, "three views and the window");
        for id in [MAIN_ID, INFO_ID, CITIZENS_ID] {
            assert_eq!(page(id).views.len(), 1, "{id} has no switch");
            assert!(page(id).views[0].is_window_level());
        }
    }

    #[test]
    fn four_lists_of_one_window_name_their_row_name_and_the_scope_is_what_saves_them() {
        let names: Vec<Scope> = page(PERMISSIONS_ID)
            .panes()
            .into_iter()
            .filter(|pane| pane.id() == "name")
            .map(|pane| pane.scope())
            .collect();
        assert_eq!(
            names,
            [
                Scope::Row("users"),
                Scope::Row("eventsList"),
                Scope::Row("rankList"),
                Scope::Row("blocks")
            ]
        );
        // Four `name` panes, no two in the same scope, and none at the window level.
        assert_eq!(names.len(), 4);
        assert!(names.iter().all(|scope| *scope != Scope::Window));
        assert!(page(PERMISSIONS_ID).pane("name", Scope::Row("blocks")).is_some());
        // The citizen page has its own `name`: the title of the row, not of the window.
        assert!(page(CITIZENS_ID).pane("name", Scope::Row("citizenList")).is_some());
        assert!(page(CITIZENS_ID).pane("name", Scope::Window).is_none());
    }

    #[test]
    fn no_page_shows_a_progress_or_a_stock_and_the_page_counter_is_the_one_step() {
        for page in PAGES {
            assert!(!page.shows(Value::Progress), "{} has no percentage", page.id);
            assert!(!page.shows(Value::Stock), "{} has no stock line", page.id);
        }
        let steps: Vec<&str> = PAGES
            .iter()
            .filter(|page| page.shows(Value::Step))
            .map(|page| page.id)
            .collect();
        assert_eq!(steps, [PERMISSIONS_ID], "pageNum is n/3, AbstractWindowSkeleton.java:249");
        // The level of the building is not a pane of its own on any of the four.
        assert!(!page(MAIN_ID).shows(Value::Level));
        assert!(page(MAIN_ID).shows(Value::Text));
    }

    #[test]
    fn seven_buttons_fold_into_src_ui_and_the_last_one_is_the_cross_of_the_info_page() {
        let mut convertible = Vec::new();
        for page in PAGES {
            for button in page.buttons() {
                if button.to_ui().is_some() {
                    convertible.push((page.id, button.id));
                }
            }
        }
        assert_eq!(
            convertible,
            [
                (MAIN_ID, "rename"),
                (MAIN_ID, "map"),
                (MAIN_ID, "patreon"),
                (MAIN_ID, "colonyStylePicker"),
                (MAIN_ID, "bannerPicker"),
                (MAIN_ID, "resettexture"),
                // The only cross of the four pages, and it has no guard at all:
                // `deleteWorkOrder` removes by `hiddenId` and sends, without
                // testing anything (`WindowInfoPage.java:224-237`). A message
                // and no refusal is exactly what src/ui can say.
                (INFO_ID, "delete"),
            ],
            "the rest refuse something src/ui cannot name, or do nothing"
        );
        // A guarded button never becomes a src/ui action.
        let recall = page(CITIZENS_ID).button("recallone", Scope::Window).expect("recallone");
        assert!(recall.is_guarded());
        assert_eq!(recall.to_ui(), None);
        // Nor does a handler Java registers and leaves empty.
        let nothing = page(MAIN_ID).button("changeSpec", Scope::Window).expect("changeSpec");
        assert_eq!(nothing.act, Act::Nothing);
        assert_eq!(nothing.to_ui(), None);
    }

    #[test]
    fn the_four_pages_carry_forty_nine_panes_and_twenty_eight_buttons() {
        let panes: usize = PAGES.iter().map(|page| page.panes().len()).sum();
        let buttons: usize = PAGES.iter().map(|page| page.buttons().len()).sum();
        assert_eq!((panes, buttons), (49, 28));
        assert_eq!(page(MAIN_ID).panes().len(), 6);
        assert_eq!(page(CITIZENS_ID).panes().len(), 13);
        assert_eq!(page(INFO_ID).panes().len(), 9);
        assert_eq!(page(PERMISSIONS_ID).panes().len(), 21);
        // Every declared line names its XML and a line number above zero.
        for page in PAGES {
            for pane in page.panes() {
                assert!(pane.declared.0.ends_with(".xml"));
                assert!(pane.declared.1 > 0);
            }
        }
    }
}
