//! `WindowSelectRequest` : les requetes ouvertes qu'un atelier peut prendre en
//! charge, et les deux boutons qui rendent la main.
//!
//! Java reference : `core/client/gui/modules/building/WindowSelectRequest.java` (207
//! lignes), `gui/layouthuts/layoutselectrequest.xml` (24), et les trois appelants qui
//! lui donnent son filtre et son retour (`WindowCrafting.java:185`,
//! `WindowFurnaceCrafting.java:130`, `DOCraftingWindow.java:112`).
//!
//! **Ce qu'elle demande** : rien qu'elle-meme — un predicat et un consommateur a la
//! construction (`:54-55`), le reste vient du gestionnaire de requetes de la colonie
//! (`:88-94`). **Ce qu'elle rend** : une requete, ou rien, a la fenetre qui l'a
//! ouverte (`WindowCrafting.java:212-221`). C'est la seule des trois qui ne rende ni
//! un ecran ni un message.
//!
//! Trois faits qu'un dessin cacherait :
//!
//! 1. **La liste est recollectee au clic.** `select` rappelle `getOpenRequests()`
//!    (`:132`) alors que les lignes viennent du dernier `getElementCount()`
//!    (`:150-154`) : l'indice clique et l'indice resolu peuvent designer deux
//!    requetes. [`chosen`] porte ce desaccord au lieu de le corriger.
//! 2. **L'ordre n'est pas une regle** : les racines sont un `HashSet` de jetons
//!    (`:92-94`).
//! 3. **Deux libelles restent en arriere.** `requester` n'est ecrit que dans la
//!    branche des piles (`:174-180`) et `shortDetail` pas du tout pour un
//!    `ItemTagRequest` sans pile (`:192-199`) ; BlockUI reutilise les panneaux de
//!    ligne, donc ces deux-la gardent le texte de la ligne precedente.

use crate::requests::{Queue, Request};
use crate::ui::action::Effect;
use crate::ui::field::{Scope, Value};

use super::pane::{Pane, Source};
use super::refusal::Refusal;
use super::{Button, By, Returns};

/// La classe Java (`:39`).
pub const JAVA: &str = "WindowSelectRequest";

/// Le XML (`:56`), sous `assets/minecolonies/`.
pub const RESOURCE: &str = "minecolonies:gui/layouthuts/layoutselectrequest.xml";

/// Le titre, dans un `text` **sans id** (`layoutselectrequest.xml:4-5`) : la classe
/// mere cherche `desc` (`AbstractModuleWindow.java:63-69`), absent de ce XML.
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.citizen.requests";

/// La liste des requetes (`layoutselectrequest.xml:9`).
pub const LIST: &str = "requests";

/// L'icone de l'objet demande (`WindowConstants.java:872`, ecrite a `:170-178`).
pub const REQUEST_STACK: &str = "requestStack";

/// L'icone de repli du resolveur (`WindowConstants.java:877`, ecrite a `:184-189`).
pub const DELIVERY_IMAGE: &str = "deliveryImage";

/// Le detail court (`WindowConstants.java:902`, ecrit a `:196`, `:202`).
pub const SHORT_DETAIL: &str = "shortDetail";

/// Qui demande (`WindowConstants.java:912`, ecrit a `:179` — et la seulement).
pub const REQUESTER: &str = "requester";

/// Le bouton de ligne (`WindowConstants.java:417`, lie a `:61`).
pub const BUTTON_SELECT: &str = "select";

/// La croix (`WindowConstants.java:477`, liee a `:62`).
pub const BUTTON_CANCEL: &str = "cancel";

/// Le libelle du bouton de ligne (`layoutselectrequest.xml:20`) ; la croix porte le
/// litteral `X` (`:6`).
pub const SELECT_LABEL_KEY: &str = "com.minecolonies.coremod.gui.scantool.select";

/// Les ticks par pile affichee (`WindowConstants.java:922`, employe a `:178`, `:197`).
pub const LIFE_COUNT_DIVIDER: i32 = 30;

/// Le nombre de pas d'une chaine de parents avant de s'arreter.
///
/// **Divergence de surete** : Java n'a aucune borne (`:100-114`), donc deux requetes
/// qui se declarent mutuellement parents et que le predicat refuse toutes deux font
/// tourner le client en rond. Le port s'arrete a soixante-quatre pas.
pub const MAX_DEPTH: usize = 64;

/// `requester` est-il ecrit sur une requete sans pile d'affichage ? Non : `:179` est
/// dans la branche des piles, et la branche d'icone (`:181-190`) n'ecrit que le logo.
pub const WRITES_REQUESTER_WITHOUT_STACKS: bool = false;

/// `shortDetail` est-il ecrit sur un `ItemTagRequest` sans pile ? Non : `:194` garde
/// la meme condition, et `:200-203` est le `else` du test de type.
pub const WRITES_SHORT_DETAIL_WITHOUT_STACKS: bool = false;

/// Les cinq panneaux.
pub const FIELDS: &[Pane] = &[
    Pane::new(LIST, Value::Row, Source::Requests, ("layoutselectrequest.xml", 9)),
    Pane::in_row(LIST, REQUEST_STACK, Value::Icon, Source::Requests, ("layoutselectrequest.xml", 12)),
    Pane::in_row(LIST, DELIVERY_IMAGE, Value::Icon, Source::Requests, ("layoutselectrequest.xml", 13)),
    Pane::in_row(LIST, SHORT_DETAIL, Value::Text, Source::Requests, ("layoutselectrequest.xml", 16)),
    Pane::in_row(LIST, REQUESTER, Value::Name, Source::Requests, ("layoutselectrequest.xml", 17)),
];

/// Les deux boutons : la croix de la fenetre et le `select` de chaque ligne.
///
/// `select` n'agit pas dans la fenetre, il rend une requete a l'appelant
/// ([`Button::returns`]). Son seul refus est celui de Java, un **silence** : hors
/// bornes, le clic ne rend rien (`:134`).
pub const BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_CANCEL,
        label: "",
        effect: Effect::Reopens,
        scope: Scope::Window,
        registered: By::Window,
        returns: Returns::NoRequest,
        refusals: &[],
    },
    Button {
        id: BUTTON_SELECT,
        label: SELECT_LABEL_KEY,
        effect: Effect::Reopens,
        scope: Scope::Row(LIST),
        registered: By::Window,
        returns: Returns::Request,
        refusals: &[Refusal::RowOutOfRange],
    },
];

/// La collecte de `getOpenRequests` (`:84-118`), sur les donnees du port.
///
/// `queue` tient les requetes, `roots` les jetons declares assignes par les deux
/// resolveurs (`:92-94`), `parent` le lien de parente — la seule chose que
/// `crate::requests::Request` ne porte pas (`MANQUANT.md` §2) — et `keeps` le predicat
/// de l'appelant. La marche suit Java : on remonte la chaine, on **s'arrete** sur une
/// requete deja retenue, et un refus du predicat **n'arrete pas** la remontee
/// (`:102-113`) : une requete refusee peut avoir un parent retenu.
pub fn collect<'a, P, K>(queue: &'a Queue, roots: &[i32], parent: P, keeps: K) -> Vec<&'a Request>
where
    P: Fn(&Request) -> Option<i32>,
    K: Fn(&Request) -> bool,
{
    let mut collected: Vec<&Request> = Vec::new();
    for token in roots {
        let mut cursor = queue.get(*token);
        let mut depth = 0;
        while let Some(request) = cursor {
            if collected.iter().any(|seen| seen.id() == request.id()) {
                break;
            }
            if keeps(request) {
                collected.push(request);
            }
            depth += 1;
            if depth >= MAX_DEPTH {
                break;
            }
            cursor = parent(request).and_then(|id| queue.get(id));
        }
    }
    collected
}

/// Ce que le clic `select` rend : la requete de la **collecte fraiche**, a l'indice de
/// la ligne **dessinee** (`:131-137`).
///
/// `Err(RowOutOfRange)` est le `if (row >= 0 && row < requests.size())` de `:134`, qui
/// sort sans rien rendre — un refus silencieux, pas un message.
pub fn chosen<'a>(fresh: &[&'a Request], row: i32) -> Result<&'a Request, Refusal> {
    if row < 0 {
        return Err(Refusal::RowOutOfRange);
    }
    fresh.get(row as usize).copied().ok_or(Refusal::RowOutOfRange)
}

/// L'indice de la pile affichee a ce tick : `(lifeCount / 30) % size` (`:178`, `:197`).
///
/// `None` quand il n'y a rien a afficher — Java teste `!displayStacks.isEmpty()` avant
/// (`:174`) — et pour un compteur negatif, que Java ne produit pas (`:70-73`).
pub fn icon_index(life_count: i32, displayed: usize) -> Option<usize> {
    if life_count < 0 || displayed == 0 {
        return None;
    }
    Some(((life_count / LIFE_COUNT_DIVIDER) as usize) % displayed)
}

/// Qui demande, tel que le port le sait : la **nature** du batiment.
///
/// Java ecrit `getRequesterDisplayName(manager, request)` (`:179`), le nom du
/// batiment ; `crate::requests::Requester` ne porte que sa nature et sa position. La
/// ligne affiche donc `builder` la ou Java affiche « Builder's Hut ».
pub fn requester_name(request: &Request) -> &str {
    &request.requester().building
}

/// Le detail court d'une ligne : le nombre et l'objet.
///
/// Java ecrit la pile survolee ou `getShortDisplayString()` prive de son `§f`
/// (`:196`, `:202`). Sans table de traduction, le port ecrit les deux champs que Java
/// imprime dans la branche qui compte (`StandardRequests.java:189`, `:196-198`).
pub fn short_detail(request: &Request) -> String {
    format!("{} {}", request.count(), request.item().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::Requester;

    fn request(id: i32) -> Request {
        let item = ItemName::new("minecraft:iron_ingot").expect("un id d'objet valide");
        Request::new(id, item, 4, 1, Requester::new("builder", [1, 2, 3]))
            .expect("quatre lingots demandes")
    }

    fn queue(wanted: &[i32]) -> Queue {
        Queue::with_requests(wanted.iter().map(|id| request(*id)).collect())
    }

    fn ids(collected: Vec<&Request>) -> Vec<i32> {
        collected.iter().map(|request| request.id()).collect()
    }

    #[test]
    fn the_walk_climbs_the_parents_dedups_and_stops_at_the_bound() {
        let queue = queue(&[1, 2, 3]);
        // 1 -> 2 -> 3, et rien apres.
        let parent = |request: &Request| match request.id() {
            1 | 2 => Some(request.id() + 1),
            _ => None,
        };
        assert_eq!(ids(collect(&queue, &[1], parent, |_| true)), [1, 2, 3], ":100-114");
        // Une racine qui repasse par une requete deja retenue s'arrete la (`:102-104`).
        assert_eq!(ids(collect(&queue, &[2, 1], parent, |_| true)), [2, 3, 1]);
        // Un refus du predicat ne coupe pas la remontee : seul le break de :103 coupe.
        assert_eq!(ids(collect(&queue, &[1], parent, |request| request.id() == 3)), [3]);
        // Le cas que le source permet : deux requetes mutuellement parents, refusees.
        let cycle = |request: &Request| Some(if request.id() == 1 { 2 } else { 1 });
        assert!(collect(&queue, &[1], cycle, |_| false).is_empty());
    }

    #[test]
    fn select_guards_the_row_and_hands_back_the_fresh_collection() {
        let first = request(7);
        let second = request(8);
        let fresh = [&first, &second];
        assert_eq!(chosen(&fresh, 0).map(|request| request.id()), Ok(7));
        assert_eq!(chosen(&fresh, 1).map(|request| request.id()), Ok(8));
        // Les deux bords du garde de :134.
        assert_eq!(chosen(&fresh, -1), Err(Refusal::RowOutOfRange));
        assert_eq!(chosen(&fresh, 2), Err(Refusal::RowOutOfRange));
        let empty: [&Request; 0] = [];
        assert_eq!(chosen(&empty, 0), Err(Refusal::RowOutOfRange));
        // La course : la ligne dessinee etait `7`, la collecte fraiche commence par
        // `8` — l'indice 0 rend la requete que le joueur n'a pas cliquee.
        let drawn = [&first, &second];
        let fresh = [&second, &first];
        assert_eq!(drawn[0].id(), 7);
        assert_eq!(chosen(&fresh, 0).map(|request| request.id()), Ok(8));
    }

    #[test]
    fn the_item_icon_changes_every_thirty_ticks() {
        assert_eq!(icon_index(0, 3), Some(0));
        assert_eq!(icon_index(29, 3), Some(0));
        assert_eq!(icon_index(30, 3), Some(1));
        assert_eq!(icon_index(60, 3), Some(2));
        assert_eq!(icon_index(90, 3), Some(0), "la pile revient");
        assert_eq!(icon_index(0, 0), None);
        assert_eq!(icon_index(-1, 3), None, "un compteur que Java ne produit pas");
        assert_eq!(LIFE_COUNT_DIVIDER, 30, "WindowConstants.java:922");
    }

    #[test]
    fn the_row_panes_read_the_ports_request_and_two_of_them_dangle() {
        let ids: Vec<&str> = FIELDS.iter().map(|pane| pane.id).collect();
        assert_eq!(ids, [LIST, REQUEST_STACK, DELIVERY_IMAGE, SHORT_DETAIL, REQUESTER]);
        for pane in FIELDS.iter().filter(|pane| pane.id != LIST) {
            assert_eq!(pane.scope, Scope::Row(LIST), "{} n'est pas dans la ligne", pane.id);
        }
        // Deux icones distinctes, jamais visibles ensemble (`:174-190`).
        assert_ne!(REQUEST_STACK, DELIVERY_IMAGE);
        // Les deux faits du source : des constantes, pas des oublis.
        assert!(!WRITES_REQUESTER_WITHOUT_STACKS, ":179 est dans la branche des piles");
        assert!(!WRITES_SHORT_DETAIL_WITHOUT_STACKS, ":194 garde la meme condition");
        // Et ce que la ligne lit quand elle est ecrite.
        let request = request(7);
        assert_eq!(requester_name(&request), "builder");
        assert_eq!(short_detail(&request), "4 minecraft:iron_ingot");
        assert_eq!(short_detail(&request), format!("{} {}", request.count(), request.item()));
    }

    #[test]
    fn the_two_buttons_are_a_row_and_a_corner_and_both_reopen() {
        assert_eq!(BUTTONS.len(), 2);
        let cancel = BUTTONS[0];
        let select = BUTTONS[1];
        assert_eq!((cancel.id, cancel.label), ("cancel", ""), "layoutselectrequest.xml:6");
        assert_eq!(cancel.returns, Returns::NoRequest);
        assert!(cancel.refusals.is_empty(), "la croix ne refuse rien");
        assert_eq!(select.id, "select");
        assert_eq!(select.label, SELECT_LABEL_KEY);
        assert_eq!(select.returns, Returns::Request);
        // Un seul refus, et c'est celui du garde de :134.
        assert_eq!(select.refusals.len(), 1);
        assert_eq!(select.refusals.first(), Some(&Refusal::RowOutOfRange));
        assert_eq!(select.scope, Scope::Row(LIST));
        // Les deux rouvrent la fenetre qui les a ouverts, aucun n'envoie de message.
        for button in BUTTONS {
            assert_eq!(button.effect, Effect::Reopens);
            assert_eq!(button.registered, By::Window);
            assert!(!button.sends_message());
        }
    }
}
