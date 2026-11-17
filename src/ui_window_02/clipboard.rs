//! `WindowClipBoard` : les requêtes ouvertes de la colonie, et son unique bouton.
//!
//! La fenêtre ne dessine qu'un titre, un fond et **un** bouton : la liste vient du module
//! `RequestTreeWindowModule`, monté à `(16, 44)` (`WindowClipBoard.java:55`) sur
//! `layoutrequeststree.xml`. Ce que la fenêtre décide, c'est **quelles** requêtes entrent
//! dans cette liste (`getOpenRequests`, `:117-183`) et dans quel ordre.
//!
//! # Le drapeau, et les trois noms qui ne s'accordent pas
//!
//! Le champ s'appelle `showImportant` (`:44`), l'item l'enregistre sous `hideunimportant`
//! (`ItemClipboard.java:37`) et l'infobulle dit « Only show important requests »
//! (`windowclipboard.xml:8`). Or `ItemClipboard` passe `hide` au paramètre `showImportant`
//! (`:126-131`), la fenêtre **filtre quand le drapeau est faux** (`:154-168`) et le clic
//! écrit `1` quand elle montre tout (`:74`) : `hideunimportant = 1` **montre** les requêtes
//! sans importance. Rien ne casse, puisque lecture et écriture s'accordent — mais le tag dit
//! l'inverse de ce qu'il vaut. Le port garde la valeur **à l'identique** et nomme les deux
//! états d'après ce que la liste montre : [`Filter`].

use crate::ui::Value;

use super::build::distance;
use super::pane::{Action, Exit, Label, Message, Pane, Press, Source};
use super::refusal::Refusal;
use super::Window;

/// L'id de la fenêtre dans cette table.
pub const ID: &str = "clipboard";
/// La classe qui la construit.
pub const JAVA: &str = "WindowClipBoard";
/// La ressource (`:53`).
pub const RESOURCE: &str = "minecolonies:gui/windowclipboard.xml";
/// La mise en page que le module de liste monte (`RequestTreeWindowModule.java:178-182`).
pub const LAYOUT: &str = "minecolonies:gui/layouthuts/layoutrequeststree.xml";

/// La liste (`layoutrequeststree.xml:1`).
pub const REQUESTS: &str = "requests";
/// L'icône de la requête, une ligne (`:5`).
pub const REQUEST_STACK: &str = "requestStack";
/// L'image de remplacement, une ligne (`:3`).
pub const DELIVERY_IMAGE: &str = "deliveryImage";
/// Le détail court, une ligne (`:7`).
pub const SHORT_DETAIL: &str = "shortDetail";
/// Le demandeur, une ligne (`:10`).
pub const REQUESTER: &str = "requester";
/// L'icône de détail, une ligne (`:8`).
pub const DETAIL_ICON: &str = "detailIcon";
/// Le détail d'une ligne (`:4` ; `WindowConstants.java:897`).
pub const DETAIL: &str = "detail";
/// Remplir la requête (`:13` ; `WindowConstants.java:614`).
pub const FULFILL: &str = "fulfill";
/// L'annuler (`:17` ; `WindowConstants.java:619`) — même id qu'un bouton d'une autre fenêtre.
pub const CANCEL: &str = "cancel";
/// Le bouton du drapeau (`windowclipboard.xml:8` ; `WindowConstants.java:1080`).
pub const TOGGLE: &str = "important";
/// Son libellé (`windowclipboard.xml:8`).
pub const TOGGLE_LABEL: &str = "!";
/// Son infobulle (`windowclipboard.xml:8`).
pub const TOOLTIP_KEY: &str = "com.minecolonies.coremod.gui.request.hideshow";
/// L'infobulle du détail (`RequestTreeWindowModule.java:172` ; `TranslationConstants.java:883`).
pub const DETAILS_KEY: &str = "com.minecolonies.gui.details";
/// Le libellé de `fulfill` (`layoutrequeststree.xml:14`).
pub const FULFILL_KEY: &str = "com.minecolonies.coremod.gui.citizen.requestsfulfill";
/// Le libellé de `cancel` (`:18`).
pub const CANCEL_KEY: &str = "com.minecolonies.coremod.gui.citizen.requestscancel";
/// Le message qui enregistre le drapeau sur l'item (`:73-75`).
pub const ITEM_SETTING_MESSAGE: &str = "ItemSettingMessage";
/// Le message qui annule une requête (`RequestTreeWindowModule.java:334`).
pub const UPDATE_REQUEST_STATE_MESSAGE: &str = "UpdateRequestStateMessage";
/// L'état écrit dans ce message (`RequestState.CANCELLED`, `:334`).
pub const CANCELLED_STATE: &str = "CANCELLED";
/// La fenêtre que `detail` ouvre (`:388`).
pub const DETAIL_WINDOW: &str = "WindowRequestDetail";
/// La clé du réglage dans le compound de l'item (`ItemClipboard.java:37`).
pub const HIDE_UNIMPORTANT_TAG: &str = "hideunimportant";

const F_REQUESTS: Pane = Pane::new(REQUESTS, Value::Row, Source::REQUEST);
const F_STACK: Pane = Pane::in_row(REQUESTS, REQUEST_STACK, Value::Icon, Source::REQUEST);
const F_IMAGE: Pane = Pane::in_row(REQUESTS, DELIVERY_IMAGE, Value::Icon, Source::REQUEST);
const F_DETAIL: Pane = Pane::in_row(REQUESTS, SHORT_DETAIL, Value::Text, Source::REQUEST);
const F_REQUESTER: Pane = Pane::in_row(REQUESTS, REQUESTER, Value::Name, Source::REQUEST);
const F_ICON: Pane = Pane::in_row(REQUESTS, DETAIL_ICON, Value::Icon, Source::REQUEST);

/// Une liste et cinq panneaux de ligne. Le titre n'a **pas d'`id`**
/// (`windowclipboard.xml:6-7`), donc ce n'est pas un panneau.
pub const PANES: &[Pane] = &[F_REQUESTS, F_STACK, F_IMAGE, F_DETAIL, F_REQUESTER, F_ICON];
/// Aucune liste déroulante.
pub const DROPDOWNS: &[&str] = &[];

const NO_REFUSAL: &[Refusal] = &[];
const NOTHING_SENT: &[Message] = &[];
const SENDS_SETTING: &[Message] = &[Message::plain(ITEM_SETTING_MESSAGE)];
const SENDS_CANCEL: &[Message] = &[Message::with(UPDATE_REQUEST_STATE_MESSAGE, CANCELLED_STATE)];

/// Le drapeau flippe, réécrit l'item et **rafraîchit** la liste sans se fermer (`:67-78`).
const A_TOGGLE: Action =
    Action::new(TOGGLE, Label::Xml(TOGGLE_LABEL), Press::new(SENDS_SETTING, Exit::Stays), NO_REFUSAL);
/// `detail` ouvre la fenêtre de la ligne cliquée (`RequestTreeWindowModule.java:373-389`).
const A_DETAIL: Action = Action::new(
    DETAIL,
    Label::Xml(""),
    Press::new(NOTHING_SENT, Exit::OpensJava(DETAIL_WINDOW)),
    NO_REFUSAL,
);
/// `fulfill` est **toujours caché** ici : `isFulfillable` rend `false` et le module de la
/// clipboard ne le redéfinit pas (`:275-278`, `WindowClipBoard.java:100-188`).
const A_FULFILL: Action =
    Action::new(FULFILL, Label::Xml(FULFILL_KEY), Press::STAYS, &[Refusal::FulfillUnsupported]);
/// `cancel` n'est offert qu'à une racine (`:265-273`) et envoie `CANCELLED` (`:334`).
const A_CANCEL: Action =
    Action::new(CANCEL, Label::Xml(CANCEL_KEY), Press::new(SENDS_CANCEL, Exit::Stays), &[Refusal::NotRootRequest]);

/// Les quatre boutons, dans l'ordre de l'XML puis du module.
pub const ACTIONS: &[Action] = &[A_TOGGLE, A_DETAIL, A_FULFILL, A_CANCEL];

/// La fenêtre, telle que `ItemClipboard.java:131` l'ouvre.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    layout: Some(LAYOUT),
    panes: PANES,
    dropdowns: DROPDOWNS,
    actions: ACTIONS,
};

/// Ce que la liste montre, nommé d'après ce qu'elle montre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// Ni `MinimumStack`, ni requête asynchrone d'un métier (`:154-168`).
    ImportantOnly,
    /// Tout, sans filtre.
    All,
}

impl Filter {
    /// L'état d'ouverture, depuis le booléen de l'item : `hideunimportant = true` montre
    /// tout (`ItemClipboard.java:126-131` → `:51`, `:154-168`).
    pub fn from_saved(hide_unimportant: bool) -> Filter {
        if hide_unimportant { Filter::All } else { Filter::ImportantOnly }
    }

    /// Ce que le clic réécrit dans l'item (`:74`).
    pub fn to_saved(self) -> i32 {
        match self {
            Filter::All => 1,
            Filter::ImportantOnly => 0,
        }
    }

    /// L'autre état, ce que le clic produit (`:69`).
    pub fn flipped(self) -> Filter {
        match self {
            Filter::All => Filter::ImportantOnly,
            Filter::ImportantOnly => Filter::All,
        }
    }

    /// La couleur du bouton (`:86-98`) : vert quand tout est montré, rouge sinon.
    pub fn colour(self) -> &'static str {
        match self {
            Filter::All => "green",
            Filter::ImportantOnly => "red",
        }
    }

    /// Vrai quand la ligne passe le filtre.
    pub fn keeps(self, row: &Row) -> bool {
        match self {
            Filter::All => true,
            Filter::ImportantOnly => !row.minimum_stack && !row.async_request,
        }
    }
}

/// Ce que le clic écrit dans l'item après un basculement (`:69-75`).
pub fn saved_after_toggle(hide_unimportant: bool) -> i32 {
    Filter::from_saved(hide_unimportant).flipped().to_saved()
}

/// Une requête de la liste, vue par la fenêtre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Le jeton de la requête.
    pub id: i32,
    /// `request.getId().hashCode()`, le second critère du tri (`:174`).
    pub hash: i32,
    /// Un `MinimumStack` (`:154`).
    pub minimum_stack: bool,
    /// Le jeton vient d'une requête asynchrone d'un métier (`:123-130`, `:167`).
    pub async_request: bool,
    /// La position du demandeur, seule lecture du tri (`:171-174`).
    pub at: [i32; 3],
    /// Le parent, quand la requête est un enfant (`:144-148`).
    pub parent: Option<i32>,
}

/// Une ligne affichée : l'identifiant et la profondeur dans l'arbre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    pub depth: i32,
}

/// La racine de `id`, en remontant les parents (`:143-148`).
///
/// Java **écarte** la requête quand un parent est inconnu (`:149-152`) : c'est
/// [`Refusal::NoSuchRequest`]. Un cycle est coupé au bout de `rows.len()` sauts au lieu de
/// boucler comme `while (request.hasParent())` le ferait.
pub fn root_of(rows: &[Row], id: i32) -> Result<i32, Refusal> {
    let mut current = rows.iter().find(|row| row.id == id).ok_or(Refusal::NoSuchRequest)?;
    let mut hops = 0usize;
    while let Some(parent) = current.parent {
        hops += 1;
        if hops > rows.len() {
            return Err(Refusal::NoSuchRequest);
        }
        current = rows.iter().find(|row| row.id == parent).ok_or(Refusal::NoSuchRequest)?;
    }
    Ok(current.id)
}

/// La liste affichée : les racines retenues, de la plus proche du joueur à la plus loin,
/// puis leurs enfants (`:170-174`, `:245-262`).
pub fn shown(rows: &[Row], filter: Filter, player: [i32; 3]) -> Vec<Entry> {
    let mut roots: Vec<&Row> =
        rows.iter().filter(|row| row.parent.is_none() && filter.keeps(row)).collect();
    roots.sort_by_key(|row| (distance(player, row.at), row.hash));
    let mut out = Vec::new();
    for root in roots {
        push_tree(rows, root, 0, &mut out);
    }
    out
}

/// Ajoute une requête et sa descendance (`constructTreeFromRequest`, `:245-262`).
fn push_tree(rows: &[Row], row: &Row, depth: i32, out: &mut Vec<Entry>) {
    if out.iter().any(|entry| entry.id == row.id) {
        return;
    }
    out.push(Entry { id: row.id, depth });
    for child in rows.iter().filter(|other| other.parent == Some(row.id)) {
        push_tree(rows, child, depth + 1, out);
    }
}

/// `cancel` n'est offert qu'aux racines (`:265-273`).
pub fn cancellable(depth: i32) -> Result<(), Refusal> {
    if depth <= 0 { Ok(()) } else { Err(Refusal::NotRootRequest) }
}

/// `fulfill` ne l'est jamais dans cette fenêtre (`:275-278`).
pub fn fulfillable() -> Result<(), Refusal> {
    Err(Refusal::FulfillUnsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: i32, hash: i32, at: [i32; 3]) -> Row {
        Row { id, hash, minimum_stack: false, async_request: false, at, parent: None }
    }

    #[test]
    fn the_flag_round_trips_through_the_item_and_names_who_shows_what() {
        // `hideunimportant = 1` montre tout : le tag dit l'inverse de sa valeur.
        assert_eq!(Filter::from_saved(true), Filter::All);
        assert_eq!(Filter::from_saved(false), Filter::ImportantOnly);
        assert_eq!(Filter::All.to_saved(), 1, "WindowClipBoard.java:74");
        assert_eq!(Filter::from_saved(true).to_saved(), 1, "aller-retour intact");
        // Le clic flippe et écrit l'autre valeur (`:69-75`).
        assert_eq!(saved_after_toggle(true), 0);
        assert_eq!(saved_after_toggle(false), 1);
        assert_eq!(Filter::from_saved(false).colour(), "red", ":96");
        assert_eq!(Filter::from_saved(true).colour(), "green", ":92");
        assert_eq!(HIDE_UNIMPORTANT_TAG, "hideunimportant");
        // Les quatre infobulles sont des clés du mod, pas des phrases.
        for key in [TOOLTIP_KEY, DETAILS_KEY, FULFILL_KEY, CANCEL_KEY] {
            assert!(key.starts_with("com.minecolonies."), "{key} n'est pas une clé");
        }
    }

    #[test]
    fn important_only_drops_minimum_stacks_and_async_requests() {
        let plain = request(1, 0, [0, 0, 0]);
        let minimum = Row { minimum_stack: true, ..request(2, 0, [0, 0, 0]) };
        let asynchronous = Row { async_request: true, ..request(3, 0, [0, 0, 0]) };
        for row in [&plain, &minimum, &asynchronous] {
            assert!(Filter::All.keeps(row));
        }
        assert!(Filter::ImportantOnly.keeps(&plain));
        assert!(!Filter::ImportantOnly.keeps(&minimum), "WindowClipBoard.java:154");
        assert!(!Filter::ImportantOnly.keeps(&asynchronous), ":165-168");
    }

    #[test]
    fn children_follow_their_root_and_the_list_sorts_by_distance_then_hash() {
        // Deux requêtes du même bâtiment : le hachage décide, pas l'ordre d'entrée.
        let rows = [request(1, 9, [1, 0, 0]), request(2, 2, [1, 0, 0]), request(3, 0, [10, 0, 0])];
        let order: Vec<i32> =
            shown(&rows, Filter::All, [0, 0, 0]).iter().map(|entry| entry.id).collect();
        assert_eq!(order, [2, 1, 3], "thenComparingInt, :171-174");

        // Une racine filtrée emporte sa descendance : l'arbre part des racines retenues.
        let mut child = request(5, 0, [0, 0, 0]);
        child.parent = Some(4);
        let mut grandchild = request(6, 0, [0, 0, 0]);
        grandchild.parent = Some(5);
        let mut root = request(4, 0, [0, 0, 0]);
        root.minimum_stack = true;
        let tree = [root, child, grandchild];
        assert!(shown(&tree, Filter::ImportantOnly, [0, 0, 0]).is_empty());
        assert_eq!(
            shown(&tree, Filter::All, [0, 0, 0]),
            [Entry { id: 4, depth: 0 }, Entry { id: 5, depth: 1 }, Entry { id: 6, depth: 2 }]
        );
    }

    #[test]
    fn a_root_is_found_by_walking_up_and_a_broken_chain_is_named() {
        let mut child = request(2, 0, [0, 0, 0]);
        child.parent = Some(1);
        let rows = [request(1, 0, [0, 0, 0]), child];
        assert_eq!(root_of(&rows, 1), Ok(1));
        assert_eq!(root_of(&rows, 2), Ok(1), "while (request.hasParent()), :144-147");
        assert_eq!(root_of(&rows, 9), Err(Refusal::NoSuchRequest));
        // Un parent absent écarte la requête (`request == null`, :149-152).
        let orphan = Row { parent: Some(7), ..request(3, 0, [0, 0, 0]) };
        assert_eq!(root_of(&[orphan], 3), Err(Refusal::NoSuchRequest));
        assert_eq!(Refusal::NoSuchRequest.key(), None, "Java écarte sans rien dire");
    }

    #[test]
    fn cancel_is_offered_at_the_root_only_and_fulfill_is_never_offered() {
        assert_eq!(cancellable(0), Ok(()));
        assert_eq!(cancellable(1), Err(Refusal::NotRootRequest));
        assert_eq!(fulfillable(), Err(Refusal::FulfillUnsupported));
        // Le bouton caché n'a pas de clé : le joueur ne voit jamais de refus.
        assert_eq!(Refusal::FulfillUnsupported.key(), None);
        assert!(A_FULFILL.is_guarded());
        assert!(!A_TOGGLE.is_guarded(), "le toggle ne peut pas être refusé");
        assert_eq!(A_FULFILL.refusals[0], Refusal::FulfillUnsupported);
        assert_eq!(A_CANCEL.sends()[0].payload, Some(CANCELLED_STATE));
        assert_eq!(A_DETAIL.press.exit, Exit::OpensJava(DETAIL_WINDOW));
        assert_eq!(A_TOGGLE.press.exit, Exit::Stays, ":77 rafraîchit sans fermer");
        assert_eq!(A_TOGGLE.sends()[0].class, ITEM_SETTING_MESSAGE);
        assert_eq!(ACTIONS.len(), 4);
    }
}
