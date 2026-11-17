//! `WindowResourceList` : ce que le build du builder demande encore, et où le
//! trouver.
//!
//! 286 lignes de Java, **aucun bouton**. Ce que la fenêtre **demande** : le module
//! `BuildingResourcesModuleView` du builder (`WindowResourceList.java:80`), les
//! requêtes de livraison ouvertes (`:84-88`), l'inventaire du joueur (`:81`), et un
//! instantané d'entrepôt que l'**item** lui passe (`:72`). Ce qu'elle **rend** :
//! une liste triée par urgence, une ligne de progression, et rien à cliquer.
//!
//! **Elle ne s'ouvre pas depuis l'onglet du builder.** C'est le rouleau de
//! ressources qui l'ouvre (`core/items/ItemResourceScroll.java:100`), et l'onglet
//! est une autre fenêtre, `WindowBuilderResModule` (`src/ui::TAB_RESOURCES_ID`).
//! Les deux lisent la **même** clé de module (`buildingresources`), ce qui est le
//! seul lien entre elles — et ce module le teste plutôt que de le supposer.
//!
//! **Cinq états, et l'ordre du tri n'est pas celui de l'énumération.** Java déclare
//! `NOT_NEEDED, IN_DELIVERY, DONT_HAVE, NEED_MORE, HAVE_ENOUGH`
//! (`BuildingBuilderResource.java:196-202`) puis trie avec l'ordre
//! `NOT_NEEDED, HAVE_ENOUGH, IN_DELIVERY, NEED_MORE, DONT_HAVE`
//! (`WindowResourceList.java:131`), et son comparateur **inverse** ce rang
//! (`BuildingBuilderResource.java:265-268`) : la ressource la plus rare passe
//! devant. Le commentaire de Java dit « disponible d'abord » (`:255-257`) ; le code
//! fait l'inverse, et c'est le code qui décide.
//!
//! **Le montant d'entrepôt atterrit dans le panneau de livraison.** Voir
//! [`Mark::pane`] : reproduit à l'identique, pas corrigé.

use std::cmp::Ordering;

use crate::ui::field::{Field, Source, Value};

use super::{Header, Opener, Window};

// Ré-exports : la ligne « disponible / demandé » et le pourcentage sont **ceux de
// `src/ui`**, pas des copies qui pourraient diverger.
pub use crate::ui::field::{percent as progress_percent, stock_text, PROGRESS_KEY};

/// L'id de cette table. `resource_scroll` et non `resources` : l'onglet du builder a
/// déjà ce nom dans `src/ui`.
pub const ID: &str = "resource_scroll";
/// La classe (`WindowResourceList.java:42`).
pub const JAVA: &str = "WindowResourceList";
/// Le XML (`:69`).
pub const RESOURCE: &str = "minecolonies:gui/windowresourcescroll.xml";
/// La clé du module lu, la même que `src/ui::table::MODULE_SCREENS` (`:80`).
pub const MODULE: &str = "buildingresources";
/// Le libellé de l'XML, sans id (`windowresourcescroll.xml:10-11`).
pub const HEADER_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.resourcelist";
/// Le nom du builder (`:187`).
pub const WORKER: &str = "workerName";
/// Le nom du chantier (`:193`), tronqué à la première ligne par Java (`:194`).
pub const CONSTRUCTION: &str = "constructionName";
/// La ligne de progression (`windowresourcescroll.xml:26`).
pub const PROGRESS: &str = "progress";
/// La liste (`:28` ; `WindowConstants.java:499`).
pub const LIST: &str = "resources";
/// L'icône d'un item d'une ligne (`:30`).
pub const ICON: &str = "resourceIcon";
/// Le nom d'un item d'une ligne (`:34`).
pub const NAME: &str = "resourceName";
/// Le manque du joueur, écrit seulement s'il est négatif (`:259-267`).
pub const MISSING: &str = "resourceMissing";
/// « disponible / demandé » (`:269`).
pub const AVAILABLE_NEEDED: &str = "resourceAvailableNeeded";
/// La pastille « en livraison » (`:31`).
pub const DELIVERY_ICON: &str = "indeliveryicon";
/// Son montant (`:35`).
pub const DELIVERY_AMOUNT: &str = "indeliveryamount";
/// La pastille « à l'entrepôt » (`:32`).
pub const WAREHOUSE_ICON: &str = "inWarehouseIcon";
/// Son montant (`:36`) — que Java ne remplit **jamais**.
pub const WAREHOUSE_AMOUNT: &str = "inWarehouseAmount";
/// L'index de la ligne, un panneau d'un pixel (`:38`).
pub const ID_PANE: &str = "resourceId";
/// `amount - available`, un panneau d'un pixel (`:39`).
pub const QUANTITY: &str = "resourceQuantity";
/// Le message qui rafraîchit la vue côté serveur (`:185`).
pub const REFRESH_MESSAGE: &str = "MarkBuildingDirtyMessage";
/// L'ordre du tri, **dans l'ordre où Java le passe au comparateur** (`:131`).
pub const ORDER: [Availability; 5] = [
    Availability::NotNeeded,
    Availability::HaveEnough,
    Availability::InDelivery,
    Availability::NeedMore,
    Availability::DontHave,
];

/// L'état d'une ressource, dans l'ordre où Java le déclare
/// (`BuildingBuilderResource.java:196-202`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the five tags the enum declares, in its declaration order
pub enum Availability {
    NotNeeded,
    InDelivery,
    DontHave,
    NeedMore,
    HaveEnough,
}

/// Le manque du joueur : `player + available - amount` (`:79-83`).
///
/// Négatif quand il en a trop, zéro quand il a juste ce qu'il faut, positif quand il
/// en manque — et Java n'affiche que le négatif ([`missing_text`]).
pub fn missing_from_player(player: i32, available: i32, amount: i32) -> i32 {
    player + available - amount
}

impl Availability {
    /// Un nom stable, pour un journal ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Availability::NotNeeded => "not-needed",
            Availability::InDelivery => "in-delivery",
            Availability::DontHave => "dont-have",
            Availability::NeedMore => "need-more",
            Availability::HaveEnough => "have-enough",
        }
    }

    /// Le rang dans [`ORDER`] — l'ordre du tri, pas celui de la déclaration.
    pub fn rank(self) -> u8 {
        ORDER.iter().position(|other| *other == self).unwrap_or(0) as u8
    }
}

/// L'état d'une ressource, exactement la cascade de Java (`:104-123`).
///
/// `amount` demandé, `available` dans le coffre et le builder, `player` dans
/// l'inventaire du client, `in_delivery` dans les livraisons en cours.
pub fn status(amount: i32, available: i32, player: i32, in_delivery: i32) -> Availability {
    if amount > available {
        // `amountPlayer == 0`, pas `<= 0` : c'est Java (`:108`).
        if player == 0 {
            return if in_delivery > 0 { Availability::InDelivery } else { Availability::DontHave };
        }
        if player < amount - available {
            return if in_delivery > 0 { Availability::InDelivery } else { Availability::NeedMore };
        }
        return Availability::HaveEnough;
    }
    Availability::NotNeeded
}

/// Ce qu'une ligne de la liste affiche, une fois triée.
///
/// Les six nombres viennent de trois endroits (`:90-122`) : `amount` et `available`
/// du module du builder, `player` de l'inventaire du client, `in_delivery` des
/// requêtes de livraison ouvertes, `warehouse` de l'instantané de l'item. Le port ne
/// construit pas la ligne : il la reçoit, puisque `BuildingResourcesModuleView` n'a
/// pas d'équivalent ici.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// `getName()`, le nom affiché de l'item — et la clé du tri à état égal (`:264`).
    pub name: String,
    /// Combien le build en demande (`getAmount()`).
    pub amount: i32,
    /// Combien le coffre et le builder en ont (`getAvailable()`).
    pub available: i32,
    /// Combien le joueur en porte (`getPlayerAmount()`, écrit à l'ouverture, `:109`).
    pub player: i32,
    /// Combien sont en route (`getAmountInDelivery()`, `:111-118`).
    pub in_delivery: i32,
    /// Combien l'entrepôt en avait au moment de l'instantané (`:220`).
    pub warehouse: i32,
}

impl Row {
    /// L'état, recalculé à chaque image (`:233`).
    pub fn status(&self) -> Availability {
        status(self.amount, self.available, self.player, self.in_delivery)
    }

    /// Le manque du joueur, négatif quand il en a de reste.
    pub fn missing(&self) -> i32 {
        missing_from_player(self.player, self.available, self.amount)
    }

    /// Le texte du panneau `resourceMissing` : écrit **seulement** si le manque est
    /// négatif (`:259-267`) — donc Java n'y met jamais le manque, il y met le
    /// surplus. Le nom du panneau dit l'inverse de ce qu'il montre.
    pub fn missing_text(&self) -> Option<String> {
        let missing = self.missing();
        if missing < 0 {
            Some(missing.to_string())
        } else {
            None
        }
    }

    /// « disponible / demandé » (`:269`).
    pub fn stock_text(&self) -> String {
        stock_text(self.available, self.amount)
    }

    /// Ce que le panneau caché `resourceQuantity` porte : `amount - available`
    /// (`:271`).
    pub fn quantity_missing(&self) -> i32 {
        self.amount - self.available
    }

    /// Quelle pastille la ligne montre : livraison, entrepôt, ou rien.
    pub fn mark(&self) -> Mark {
        if self.in_delivery > 0 {
            Mark::Delivery(self.in_delivery)
        } else if self.warehouse > 0 {
            Mark::Warehouse(self.warehouse)
        } else {
            Mark::None
        }
    }
}

/// La pastille d'une ligne : une seule des deux, et la livraison gagne (`:222-231`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of Java's `if / else if`
pub enum Mark {
    None,
    Delivery(i32),
    Warehouse(i32),
}

impl Mark {
    /// L'icône affichée.
    pub fn icon(self) -> Option<&'static str> {
        match self {
            Mark::None => None,
            Mark::Delivery(_) => Some(DELIVERY_ICON),
            Mark::Warehouse(_) => Some(WAREHOUSE_ICON),
        }
    }

    /// Le panneau **de texte** que Java remplit, et c'est là que le source étonne :
    /// la branche entrepôt écrit `IN_DELIVERY_AMOUNT` (`:230`) alors que `:217` vient
    /// de vider `IN_WAREHOUSE_AMOUNT`. Le nombre s'affiche donc en vert, à côté de
    /// l'icône de caisse, et `inWarehouseAmount` reste vide pour toujours.
    /// **Reproduit, pas corrigé** : `WAREHOUSE_AMOUNT` n'est écrit par aucune ligne.
    pub fn pane(self) -> Option<&'static str> {
        match self {
            Mark::None => None,
            Mark::Delivery(_) | Mark::Warehouse(_) => Some(DELIVERY_AMOUNT),
        }
    }

    /// Le montant, quand il y en a un.
    pub fn amount(self) -> Option<i32> {
        match self {
            Mark::None => None,
            Mark::Delivery(count) | Mark::Warehouse(count) => Some(count),
        }
    }
}

/// L'ordre de la liste : `BuildingBuilderResource.ResourceComparator` (`:235-270`),
/// dont le rang s'inverse et dont l'égalité se tranche par le nom.
pub fn compare(left: &Row, right: &Row) -> Ordering {
    let (a, b) = (left.status(), right.status());
    if a == b {
        // `String.compareTo` : les deux langages comparent les unités de code, et
        // tous les noms d'items sont en ASCII.
        return left.name.cmp(&right.name);
    }
    // `order.get(resource2).compareTo(order.get(resource1))` (`:265-268`) : le rang
    // de `right` contre celui de `left`, donc l'ordre décroissant — la ressource la
    // plus rare d'abord.
    b.rank().cmp(&a.rank())
}

/// Trie les lignes en place (`:131`).
pub fn sort(rows: &mut [Row]) {
    rows.sort_by(compare);
}

/// La ligne de progression, ou l'absence de ligne.
///
/// Java n'écrit le panneau que si le total est positif (`:124-129`) ; sinon il reste
/// avec le `label=""` de l'XML, donc vide. Le pourcentage du joueur est
/// [`progress_percent`], celui du module vient du serveur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `if (total > 0)`
pub enum ProgressLabel {
    Unset,
    Shown { player: i32, module: i32 },
}

/// La ligne de progression pour ces trois nombres.
pub fn progress_label(supplied: i32, total: i32, module: i32) -> ProgressLabel {
    if total <= 0 {
        // Java garde aussi les totaux négatifs hors de la division.
        return ProgressLabel::Unset;
    }
    ProgressLabel::Shown { player: progress_percent(supplied, total), module }
}

impl ProgressLabel {
    /// Les deux arguments de la traduction, `"33%"` et `"78%"` (`:126-128`).
    pub fn args(self) -> Option<(String, String)> {
        match self {
            ProgressLabel::Unset => None,
            ProgressLabel::Shown { player, module } => {
                Some((format!("{player}%"), format!("{module}%")))
            }
        }
    }
}

const F_WORKER: Field = Field::new(WORKER, Value::Name, Source::Building);
const F_CONSTRUCTION: Field = Field::new(CONSTRUCTION, Value::Text, Source::WorkOrder);
const F_PROGRESS: Field = Field::new(PROGRESS, Value::Progress, Source::Module(MODULE));
const F_LIST: Field = Field::new(LIST, Value::Row, Source::Module(MODULE));
const F_ICON: Field = Field::in_row(LIST, ICON, Value::Icon, Source::Module(MODULE));
const F_NAME: Field = Field::in_row(LIST, NAME, Value::Name, Source::Module(MODULE));
const F_MISSING: Field = Field::in_row(LIST, MISSING, Value::Count, Source::PlayerInventory);
const F_STOCK: Field = Field::in_row(LIST, AVAILABLE_NEEDED, Value::Stock, Source::Module(MODULE));
const F_DELIVERY_ICON: Field =
    Field::in_row(LIST, DELIVERY_ICON, Value::Icon, Source::Module(MODULE));
const F_DELIVERY_AMOUNT: Field =
    Field::in_row(LIST, DELIVERY_AMOUNT, Value::Count, Source::Module(MODULE));
const F_WAREHOUSE_ICON: Field =
    Field::in_row(LIST, WAREHOUSE_ICON, Value::Icon, Source::Module(MODULE));
// L'instantané d'entrepôt vient du compound de l'item qui a ouvert la fenêtre
// (`ItemResourceScroll.java:85-95`) ; `ui::Source` n'a pas de variante pour un item,
// et `PlayerInventory` est la seule qui dise « côté client, hors du bâtiment ».
const F_WAREHOUSE_AMOUNT: Field =
    Field::in_row(LIST, WAREHOUSE_AMOUNT, Value::Count, Source::PlayerInventory);
const F_ID: Field = Field::in_row(LIST, ID_PANE, Value::Count, Source::Module(MODULE));
const F_QUANTITY: Field = Field::in_row(LIST, QUANTITY, Value::Count, Source::Module(MODULE));

const PANES: &[Field] = &[
    F_WORKER,
    F_CONSTRUCTION,
    F_PROGRESS,
    F_LIST,
    F_ICON,
    F_NAME,
    F_MISSING,
    F_STOCK,
    F_DELIVERY_ICON,
    F_DELIVERY_AMOUNT,
    F_WAREHOUSE_ICON,
    F_WAREHOUSE_AMOUNT,
    F_ID,
    F_QUANTITY,
];

/// La fenêtre. **Aucun bouton** : le rouleau ne fait que lire.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::Xml(HEADER_KEY),
    opener: Opener::Java("ItemResourceScroll"),
    panes: PANES,
    buttons: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, amount: i32, available: i32, player: i32, in_delivery: i32) -> Row {
        Row { name: name.to_owned(), amount, available, player, in_delivery, warehouse: 0 }
    }

    #[test]
    fn the_five_states_follow_the_amounts_the_player_and_the_delivery() {
        // Rien à faire : le coffre a déjà tout.
        assert_eq!(status(4, 4, 0, 0), Availability::NotNeeded);
        assert_eq!(status(4, 9, 0, 0), Availability::NotNeeded);
        // Il manque, le joueur n'a rien, et rien n'est en route.
        assert_eq!(status(64, 0, 0, 0), Availability::DontHave);
        // Rien en poche mais quelque chose en route : c'est la livraison qui gagne.
        assert_eq!(status(64, 0, 0, 8), Availability::InDelivery);
        // Le joueur en a, mais pas assez.
        assert_eq!(status(64, 0, 10, 0), Availability::NeedMore);
        assert_eq!(status(64, 0, 10, 8), Availability::InDelivery, ":110-115");
        // Le joueur comble exactement le manque.
        assert_eq!(status(64, 4, 60, 0), Availability::HaveEnough);
        assert_eq!(status(64, 4, 90, 0), Availability::HaveEnough);
        // Un joueur à zéro pile prend la branche « je n'ai rien », pas « pas assez ».
        assert_eq!(status(10, 5, 0, 0), Availability::DontHave);
        assert_eq!(status(10, 5, 1, 0), Availability::NeedMore);
    }

    #[test]
    fn the_scarcest_resource_comes_first_and_the_useless_one_last() {
        // L'ordre du tri, du premier au dernier (`:131` + `:265-268`).
        let by_rank: Vec<&str> = ORDER.iter().copied().rev().map(Availability::name).collect();
        assert_eq!(by_rank, ["dont-have", "need-more", "in-delivery", "have-enough", "not-needed"]);
        let mut rows = vec![
            row("air", 1, 1, 0, 0),      // NOT_NEEDED, dernier
            row("cobble", 64, 0, 0, 0),  // DONT_HAVE, premier
            row("oak", 32, 0, 40, 0),    // HAVE_ENOUGH
            row("sand", 32, 0, 3, 0),    // NEED_MORE
            row("stone", 32, 0, 0, 5),   // IN_DELIVERY
        ];
        sort(&mut rows);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["cobble", "sand", "stone", "oak", "air"]);
        // À état égal, l'ordre alphabétique du nom (`:264`).
        let mut ties = vec![row("zinc", 8, 0, 0, 0), row("aluminium", 8, 0, 0, 0)];
        sort(&mut ties);
        assert_eq!(ties[0].name, "aluminium");
        assert_eq!(compare(&ties[0], &ties[1]), Ordering::Less);
    }

    #[test]
    fn a_warehouse_amount_is_written_into_the_delivery_pane() {
        // La livraison gagne, sinon l'entrepôt (`:222-231`).
        assert_eq!(row("a", 8, 0, 0, 3).mark(), Mark::Delivery(3));
        let mut both = row("a", 8, 0, 0, 3);
        both.warehouse = 9;
        assert_eq!(both.mark(), Mark::Delivery(3), "le `else if` de Java");
        let mut store = row("a", 8, 0, 0, 0);
        store.warehouse = 9;
        assert_eq!(store.mark(), Mark::Warehouse(9));
        assert_eq!(row("a", 8, 0, 0, 0).mark(), Mark::None);
        // L'icône est la bonne…
        assert_eq!(Mark::Warehouse(9).icon(), Some(WAREHOUSE_ICON));
        // …et le texte va dans le panneau de livraison : `:230` écrit
        // `IN_DELIVERY_AMOUNT` là où `:217` a vidé `IN_WAREHOUSE_AMOUNT`.
        assert_eq!(Mark::Warehouse(9).pane(), Some(DELIVERY_AMOUNT));
        assert_ne!(Mark::Warehouse(9).pane(), Some(WAREHOUSE_AMOUNT));
        assert_eq!(Mark::Delivery(3).pane(), Some(DELIVERY_AMOUNT));
        assert_eq!(Mark::None.pane(), None);
        assert_eq!(Mark::Warehouse(9).amount(), Some(9));
        // Le panneau de l'entrepôt existe dans le XML et n'est jamais rempli.
        assert!(WINDOW.pane(WAREHOUSE_AMOUNT).is_some());
        assert!(WINDOW.pane(WAREHOUSE_ICON).is_some());
    }

    #[test]
    fn the_missing_line_only_prints_a_negative_number() {
        // Aucune livraison en route dans ces quatre lignes : le dernier argument du
        // constructeur de test est le montant en livraison (`:111`).
        // Le joueur en a trop : le panneau porte le surplus, en négatif.
        let surplus = row("cobble", 10, 4, 12, 0);
        assert_eq!(surplus.missing(), 6);
        assert_eq!(surplus.missing_text(), None, "un manque positif n'est pas écrit");
        let too_much = row("cobble", 10, 4, 1, 0);
        assert_eq!(too_much.missing(), -5);
        assert_eq!(too_much.missing_text().as_deref(), Some("-5"));
        // Juste ce qu'il faut : zéro, donc rien.
        assert_eq!(row("cobble", 10, 10, 0, 0).missing(), 0);
        assert_eq!(row("cobble", 10, 10, 0, 0).missing_text(), None);
        // Et les deux autres textes de la ligne.
        let line = row("cobble", 64, 12, 0, 0);
        assert_eq!(line.stock_text(), "12 / 64");
        assert_eq!(line.quantity_missing(), 52);
        assert!(line.quantity_missing() > 0, "le panneau caché compte le manque, lui");
    }

    #[test]
    fn the_progress_line_is_absent_when_there_is_nothing_to_supply() {
        let shown = progress_label(1, 3, 78);
        assert_eq!(shown, ProgressLabel::Shown { player: 33, module: 78 });
        assert_eq!(shown.args(), Some(("33%".to_owned(), "78%".to_owned())));
        // `total > 0` est la seule porte (`:124`) : zéro n'écrit pas la ligne.
        assert_eq!(progress_label(0, 0, 50), ProgressLabel::Unset);
        assert_eq!(progress_label(5, 0, 50).args(), None);
        // La ligne complète : les deux nombres viennent de deux endroits.
        assert_eq!(progress_label(3, 3, 100).args(), Some(("100%".to_owned(), "100%".to_owned())));
        // Le pourcentage est celui de `src/ui`, pas un second calcul.
        assert_eq!(progress_percent(1, 3), crate::ui::field::percent(1, 3));
        assert_eq!(PROGRESS_KEY, crate::ui::field::PROGRESS_KEY);
    }

    #[test]
    fn the_list_reads_the_builder_module_and_names_its_row_panes() {
        assert_eq!(WINDOW.panes.len(), 14);
        for id in [LIST, ICON, NAME, MISSING, AVAILABLE_NEEDED, DELIVERY_ICON, DELIVERY_AMOUNT] {
            assert!(WINDOW.pane(id).is_some(), "{id} manque");
        }
        assert_eq!(WINDOW.pane(LIST).map(|pane| pane.source), Some(Source::Module(MODULE)));
        assert_eq!(WINDOW.pane(PROGRESS).map(|pane| pane.value), Some(Value::Progress));
        assert_eq!(WINDOW.pane(WORKER).map(|pane| pane.source), Some(Source::Building));
        assert_eq!(WINDOW.pane(CONSTRUCTION).map(|pane| pane.source), Some(Source::WorkOrder));
        // Le manque se lit chez le joueur, le reste chez le module.
        assert_eq!(WINDOW.pane(MISSING).map(|pane| pane.source), Some(Source::PlayerInventory));
        assert_eq!(WINDOW.header, Header::Xml(HEADER_KEY));
        assert_eq!(WINDOW.buttons.len(), 0, "le rouleau ne clique rien");
        assert_eq!(REFRESH_MESSAGE, "MarkBuildingDirtyMessage");
    }
}
