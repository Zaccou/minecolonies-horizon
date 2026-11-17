//! La liste des tâches d'un artisan : quatre textes, une icône, **aucun bouton**.
//!
//! Java reference : `core/client/gui/modules/building/WindowHutRequestTaskModule.java`
//! (le fournisseur de données, `:46-119`) et
//! `core/colony/buildings/moduleviews/RequestTaskModuleView.java` (les jetons, `:47-51`),
//! sur `gui/layouthuts/layouttasklist.xml` (`:7-17`).
//!
//! **Ce que cette fenêtre demande** : une liste de jetons de requête, le gestionnaire de
//! requêtes de la colonie pour les résoudre, et — pour la ligne — la position du
//! demandeur, son nom affiché, l'icône de la requête, son détail court et sa priorité.
//! **Ce qu'elle rend** : quatre textes et deux images par ligne, et **rien à cliquer** :
//! le constructeur n'appelle aucun `registerButton` (`:41-44`).
//!
//! # Trois choses que Java fait et qui se voient dans la donnée
//!
//! 1. **La liste se nettoie pendant qu'on la lit.** `getElementCount` retire les jetons
//!    dont la requête a disparu (`:56`) : le compte peut donc changer entre l'appel et
//!    le rendu de la ligne, et Java irait jusqu'au `null` (`:63`). [`Tasks::prune`] fait
//!    le même nettoyage, mais explicitement, avant le rendu.
//! 2. **La priorité n'est écrite que pour une requête de livreur** (`:109-113`).
//!    `ScrollingList` réutilise ses panneaux de ligne : une tâche qui n'est pas un
//!    `IDeliverymanRequestable` **garde donc la priorité de la ligne précédente**, à
//!    l'écran. [`Row::priority`] vaut `None` et c'est au rendu de vider le panneau.
//! 3. **La couleur dit un seul état** : `DARK_GREEN` si `IN_PROGRESS`, `BLACK` sinon
//!    (`:101`, `:106`) — livrée, annulée et assignée sont toutes noires.
//!
//! # Le demandeur est une chaîne remontée
//!
//! `:65-91` : Java remonte les parents **tant qu'ils viennent de la même position** que
//! la requête, puis affiche « demandeuse -> parente » avec, en infobulle, les deux
//! positions (`:88-89`). Une requête de la même hutte n'affiche donc que son propre nom.
//! Le port n'a **pas de parent** (`crate::requests::Request` n'en porte aucun :
//! `src/requests/mod.rs`, « request chains »), donc [`Row::of`] reçoit le parent déjà
//! résolu, et [`requester_text`] applique la règle d'arrêt.

use crate::inventory::ItemName;
use crate::requests::{Queue, Request, State};

use super::refusal::Refusal;

/// La liste (`WindowHutRequestTaskModule.java:36`).
pub const LIST: &str = "tasks";
/// Le demandeur (`WindowConstants.java:912`).
pub const REQUESTER: &str = "requester";
/// Le détail court (`WindowConstants.java:902`).
pub const SHORT_DETAIL: &str = "shortDetail";
/// La priorité (`WindowConstants.java:907`).
pub const PRIORITY: &str = "priority";
/// L'icône de la requête (`WindowConstants.java:877`).
pub const DELIVERY_IMAGE: &str = "deliveryImage";
/// L'icône de l'objet de la tâche, sans constante dans Java : la chaîne est écrite à
/// `WindowHutRequestTaskModule.java:96`, `:105` et dans le XML
/// (`layouttasklist.xml:12`).
pub const DETAIL_ICON: &str = "detailIcon";

/// Les cinq panneaux que la ligne remplit, dans l'ordre où Java les écrit.
pub const ROW_PANES: &[&str] =
    &[REQUESTER, SHORT_DETAIL, PRIORITY, DELIVERY_IMAGE, DETAIL_ICON];

/// Le titre (`layouttasklist.xml:4-5`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.crafter.tasks";
/// Le texte de la liste vide (`layouttasklist.xml:7`) : « No tasks remaining »
/// (`manual_en_us.json:2007`), un attribut `emptytext` et non un panneau.
pub const EMPTY_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.crafter.tasks.empty";
/// L'étiquette de la priorité (`TranslationConstants.java:47`), écrite **collée** à la
/// valeur : `append(String.valueOf(prio))`, sans `": "` (`:111-112`).
pub const PRIORITY_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.deliveryman.priority";

/// Le caractère que Java retire du détail court (`:106`, `replace("§f", "")`).
///
/// C'est le blanc de `ChatFormatting.WHITE`, et c'est le **seul** code retiré : les
/// autres couleurs passent. Une chaîne qui contiendrait « §f » deux fois les perd toutes
/// les deux, ce que `String.replace` fait aussi.
pub const WHITE_CODE: &str = "§f";

/// Ce que la requête est, du point de vue de cet écran : le test
/// `request.getRequest() instanceof IDeliverymanRequestable` (`:109`).
///
/// Le port ne peut pas le déduire : `crate::requests::Request` n'a ni type de
/// requétable ni priorité de livraison, et son tag `type`
/// (`src/requests/request.rs:56`, `KEY_TYPE`) porte le **bâtiment demandeur**, pas la
/// requête. L'appelant le dit, et `MANQUANT.md` §2 nomme le champ qui manque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux branches du `instanceof`
pub enum Kind {
    /// Une requête que le livreur peut porter : la ligne de priorité existe.
    Deliverable,
    /// Tout le reste : la ligne de priorité n'est pas écrite.
    Other,
}

/// La couleur du texte de détail : deux `ChatFormatting` (`:101`, `:106`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: l'état IN_PROGRESS, et tous les autres
pub enum Colour {
    /// `ChatFormatting.DARK_GREEN` : la tâche est commencée.
    DarkGreen,
    /// `ChatFormatting.BLACK` : tout le reste, y compris livrée et annulée.
    Black,
}

impl Colour {
    /// La couleur que l'état de la requête impose.
    ///
    /// Un seul état est vert : `IN_PROGRESS`. `crate::requests::State` n'en a qu'un qui
    /// corresponde (`State::InProgress`) ; Java teste `request.getState()` contre une
    /// constante de son énumération à treize valeurs.
    pub fn of(state: State) -> Colour {
        match state {
            State::InProgress => Colour::DarkGreen,
            State::Created | State::Assigned | State::Delivered | State::Cancelled => Colour::Black,
        }
    }

    /// Le nom `ChatFormatting`, pour un test ou une ligne de log.
    pub fn name(self) -> &'static str {
        match self {
            Colour::DarkGreen => "DARK_GREEN",
            Colour::Black => "BLACK",
        }
    }
}

/// Le détail de la tâche : les deux branches de `:94-107`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: `IStackBasedTask` ou non
pub enum Detail {
    /// Une tâche à pile : l'icône est **visible** et reçoit l'objet avec son compte
    /// d'affichage (`:95-101`), et le texte est le préfixe de la tâche.
    Stack {
        item: ItemName,
        /// `getDisplayCount()` (`:98`) — le compte montré, pas la quantité demandée.
        count: u32,
        /// `getDisplayPrefix()` (`:101`).
        prefix: String,
    },
    /// Tout le reste : l'icône est **cachée** (`:105`) et le texte est le détail court de
    /// la requête, [`WHITE_CODE`] retiré (`:106`).
    Text(String),
}

impl Detail {
    /// Le préfixe, ou le détail court nettoyé.
    pub fn text(&self) -> String {
        match self {
            Detail::Stack { prefix, .. } => prefix.clone(),
            Detail::Text(text) => without_white(text),
        }
    }

    /// L'icône de la ligne, quand il y en a une.
    pub fn icon(&self) -> Option<(ItemName, u32)> {
        match self {
            Detail::Stack { item, count, .. } => Some((item.clone(), *count)),
            Detail::Text(_) => None,
        }
    }
}

/// Une ligne rendue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Le jeton de la requête (`IToken<?>`), qui est aussi l'identité du message qui la
    /// modifierait — aucun n'est envoyé par cette fenêtre.
    pub id: i32,
    /// `request.getDisplayIcon()` (`:115-116`) : toujours écrite, jamais vide dans Java.
    pub icon: ItemName,
    /// L'icône du détail, `None` quand elle est cachée (`:105`).
    pub detail_icon: Option<(ItemName, u32)>,
    /// Le panneau `shortDetail`.
    pub detail: String,
    /// La couleur du panneau `shortDetail`.
    pub colour: Colour,
    /// Le panneau `priority`, pour une requête de livreur **seulement** (`:109-113`).
    pub priority: Option<i32>,
    /// Le panneau `requester`.
    pub requester: String,
    /// L'infobulle du panneau `requester`, deux positions (`:88-89`), quand il y a un
    /// parent ailleurs.
    pub requester_tip: Option<String>,
}

impl Row {
    /// Une ligne, à partir de la requête, de son type d'écran, de son détail et du parent
    /// déjà résolu.
    ///
    /// Le parent n'est **pas** cherché ici : `crate::requests::Request` n'en porte pas
    /// (`src/requests/request.rs:118-135`), et le remonter est le travail de l'appelant
    /// qui a la file.
    pub fn of(request: &Request, kind: Kind, detail: Detail, parent: Option<&Request>) -> Row {
        let requester = requester_text(request, parent);
        Row {
            id: request.id(),
            icon: request.item().clone(),
            detail_icon: detail.icon(),
            detail: detail.text(),
            colour: Colour::of(request.state()),
            priority: match kind {
                Kind::Deliverable => Some(request.priority()),
                Kind::Other => None,
            },
            requester,
            requester_tip: parent.map(|parent| {
                format!(
                    "{} -> {}",
                    short_string(request.requester().at),
                    short_string(parent.requester().at)
                )
            }),
        }
    }
}

/// Le nom affiché du demandeur, seul.
///
/// Java appelle `getRequesterDisplayName(manager, request)` (`:82`), qui répond le nom du
/// bâtiment, du citoyen ou de la colonie selon le demandeur. Le port n'a que le **slug du
/// type de bâtiment** (`crate::requests::Requester.building`, `src/requests/request.rs:120-123`)
/// et aucune table de noms : la ligne montre le slug, et `MANQUANT.md` §2 dit pourquoi.
pub fn requester_name(request: &Request) -> String {
    request.requester().building.clone()
}

/// Le texte du panneau `requester` : le nom, ou « nom -> parent »
/// (`WindowHutRequestTaskModule.java:80-91`).
///
/// **La flèche dépend de `parent != null`, pas d'une comparaison de positions.** Le
/// gestionnaire remonte les parents tant qu'ils viennent de la même position (`:67`), et
/// sort de trois façons : parent nul (pas de flèche, `:80-83`), parent d'une autre position
/// (flèche, `:84-91`), ou parent de la même position **sans grand-parent** — et là il
/// `break` **sans mettre le parent à `null`** (`:69-78`), donc la flèche est dessinée et
/// les deux noms sont les mêmes. Comme le port reçoit le parent déjà résolu par cette
/// boucle, il n'a plus qu'à tester `Some`.
pub fn requester_text(request: &Request, parent: Option<&Request>) -> String {
    let name = requester_name(request);
    match parent {
        Some(parent) => format!("{} -> {}", name, requester_name(parent)),
        None => name,
    }
}

/// Une position en trois nombres séparés par « , » : `BlockPos.toShortString()`, la forme
/// que Java met dans l'infobulle (`:89`).
pub fn short_string(at: [i32; 3]) -> String {
    format!("{}, {}, {}", at[0], at[1], at[2])
}

/// Le détail court sans son code blanc (`:106`).
pub fn without_white(text: &str) -> String {
    text.replace(WHITE_CODE, "")
}

/// Les jetons que le module tient, et le nettoyage que Java fait en lisant.
///
/// Java modifie la liste **du module** (`getTasks()` rend la liste elle-même,
/// `RequestTaskModuleView.java:47-51`) à chaque appel de `getElementCount`
/// (`WindowHutRequestTaskModule.java:56`) : une requête terminée disparaît donc de la
/// fenêtre sans que rien ne la notifie, et le compte change sous le rendu. Ici le
/// nettoyage est une méthode, et elle rend les jetons qu'elle a jetés.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tasks {
    tokens: Vec<i32>,
}

impl Tasks {
    pub fn new(tokens: Vec<i32>) -> Tasks {
        Tasks { tokens }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn tokens(&self) -> &[i32] {
        &self.tokens
    }

    /// Les jetons dont la requête n'existe plus, **sans les retirer** — ce que
    /// `getElementCount` teste avant de modifier.
    pub fn missing(&self, requests: &Queue) -> Vec<i32> {
        self.tokens
            .iter()
            .copied()
            .filter(|id| requests.get(*id).is_none())
            .collect()
    }

    /// Le nettoyage de `:56` : retire les jetons orphelins et les rend.
    pub fn prune(&mut self, requests: &Queue) -> Vec<i32> {
        let missing = self.missing(requests);
        self.tokens.retain(|id| requests.get(*id).is_some());
        missing
    }

    /// Les lignes, dans l'ordre des jetons, sans les orphelins.
    ///
    /// Le type et le détail d'une requête ne se déduisent pas de [`Request`] : Java les
    /// demande à l'objet requétable (`:94`, `:109`). L'appelant les fournit par une
    /// fermeture, et une requête dont il ne sait rien est un
    /// [`Refusal::TaskMissing`] plutôt qu'une ligne vide.
    pub fn rows<F, G>(&self, requests: &Queue, kind: F, detail: G) -> (Vec<Row>, Vec<Refusal>)
    where
        F: Fn(&Request) -> Kind,
        G: Fn(&Request) -> Detail,
    {
        let mut rows = Vec::new();
        let mut refused = Vec::new();
        for id in &self.tokens {
            match requests.get(*id) {
                Some(request) => rows.push(Row::of(request, kind(request), detail(request), None)),
                None => refused.push(Refusal::TaskMissing { id: *id }),
            }
        }
        (rows, refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::Requester;
    use crate::ui::field::Value;
    use crate::ui::screen::TAB_TASKS_ID;
    use crate::ui::table::screen_by_id;

    fn request(id: i32, at: [i32; 3]) -> Request {
        request_from(id, "minecolonies:builder", at)
    }

    fn request_from(id: i32, building: &str, at: [i32; 3]) -> Request {
        let item = ItemName::new("minecraft:oak_planks").expect("a valid item name");
        Request::new(id, item, 8, 4, Requester::new(building, at))
            .expect("a request for something is a request")
    }

    #[test]
    fn the_row_panes_are_the_ones_src_ui_declares() {
        let screen = screen_by_id(TAB_TASKS_ID).expect("src/ui carries the task tab");
        for id in ROW_PANES {
            assert!(
                screen.field(id).is_some(),
                "{id} is not a pane of src/ui's task tab"
            );
        }
        assert_eq!(screen.field(LIST).map(|field| field.value), Some(Value::Row));
        assert_eq!(
            screen.field(PRIORITY).map(|field| field.value),
            Some(Value::Priority),
            "src/ui knows the pane is a priority"
        );
        assert_eq!(
            screen.field(REQUESTER).map(|field| field.value),
            Some(Value::Name)
        );
        // The screen has no button at all, and that is the point of the window.
        assert!(screen.actions.is_empty());
    }

    #[test]
    fn only_an_in_progress_task_is_green() {
        assert_eq!(Colour::of(State::InProgress), Colour::DarkGreen);
        for state in [State::Created, State::Assigned, State::Delivered, State::Cancelled] {
            assert_eq!(Colour::of(state), Colour::Black, "{} is black", state.as_str());
        }
        assert_eq!(Colour::DarkGreen.name(), "DARK_GREEN");
        assert_eq!(Colour::Black.name(), "BLACK");
    }

    #[test]
    fn a_deliverable_task_shows_a_priority_and_anything_else_does_not() {
        let request = request(3, [10, 64, 10]);
        let detail = Detail::Text("planks".into());
        let deliverable = Row::of(&request, Kind::Deliverable, detail.clone(), None);
        assert_eq!(deliverable.priority, Some(0), "a fresh request starts at zero");
        let other = Row::of(&request, Kind::Other, detail, None);
        assert_eq!(
            other.priority, None,
            "Java never writes the pane, so the previous row's number stays on screen"
        );
        // The pane is written only for a deliverable, glued to its label (no separator).
        assert_eq!(
            PRIORITY_KEY,
            "com.minecolonies.coremod.gui.workerhuts.deliveryman.priority"
        );
    }

    #[test]
    fn a_stack_task_shows_an_icon_and_a_plain_task_hides_it() {
        let request = request(4, [1, 2, 3]);
        let item = ItemName::new("minecraft:bread").expect("item");
        let stack = Detail::Stack { item: item.clone(), count: 3, prefix: "bake".into() };
        let row = Row::of(&request, Kind::Other, stack, None);
        assert_eq!(row.detail_icon, Some((item, 3)));
        assert_eq!(row.detail, "bake");
        let plain = Detail::Text("Make §fbread§f".into());
        let row = Row::of(&request, Kind::Other, plain, None);
        assert_eq!(row.detail_icon, None, "the icon is hidden for a plain task");
        assert_eq!(row.detail, "Make bread");
        // Only the white code is stripped: another colour stays.
        assert_eq!(without_white("§aa§fb"), "§aab");
        assert_eq!(WHITE_CODE, "§f");
    }

    #[test]
    fn any_resolved_parent_appends_an_arrow_and_a_tooltip() {
        let child = request(5, [10, 64, 10]);
        let parent = request_from(6, "minecolonies:warehouse", [10, 70, 12]);
        assert_eq!(requester_text(&child, None), "minecolonies:builder");
        let with_parent =
            Row::of(&child, Kind::Other, Detail::Text("x".into()), Some(&parent));
        assert_eq!(
            with_parent.requester,
            "minecolonies:builder -> minecolonies:warehouse"
        );
        assert_eq!(
            with_parent.requester_tip.as_deref(),
            Some("10, 64, 10 -> 10, 70, 12")
        );
        // A parent at the same position that has no parent of its own: Java's loop breaks
        // without clearing it (:69-78), so the arrow is still drawn, twice the same name.
        let same_place = request_from(7, "minecolonies:builder", [10, 64, 10]);
        assert_eq!(
            requester_text(&child, Some(&same_place)),
            "minecolonies:builder -> minecolonies:builder"
        );
        assert_eq!(short_string([0, -1, 2]), "0, -1, 2");
    }

    #[test]
    fn a_token_whose_request_is_gone_is_pruned_and_named() {
        let mut queue = Queue::new();
        queue.push(request(1, [0, 0, 0])).expect("fresh queue");
        queue.push(request(2, [0, 0, 0])).expect("fresh queue");
        let mut tasks = Tasks::new(vec![1, 99, 2]);
        assert_eq!(tasks.missing(&queue), vec![99]);
        assert_eq!(tasks.len(), 3, "missing() reads without touching");
        assert_eq!(tasks.prune(&queue), vec![99]);
        assert_eq!(tasks.tokens(), [1, 2]);
        let (rows, refused) = tasks.rows(&queue, |_| Kind::Other, |_| Detail::Text("t".into()));
        assert_eq!(rows.len(), 2);
        assert!(refused.is_empty());
        // A row that cannot be resolved is a named refusal, not an empty line.
        let stale = Tasks::new(vec![42]);
        let (rows, refused) = stale.rows(&queue, |_| Kind::Other, |_| Detail::Text("t".into()));
        assert!(rows.is_empty());
        assert_eq!(refused, [Refusal::TaskMissing { id: 42 }]);
        assert!(Tasks::new(Vec::new()).is_empty());
    }
}
