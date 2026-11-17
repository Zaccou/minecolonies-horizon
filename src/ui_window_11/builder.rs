//! L'onglet « ressources » du builder : quatre états par ligne, un ordre qui n'est pas
//! celui du rouleau, et une horloge de rafraîchissement.
//!
//! Java référence : `core/client/gui/modules/building/WindowBuilderResModule.java`
//! (264 lignes, `AbstractModuleWindow` — donc un **onglet** de barre latérale, ouvert par
//! `BuildingResourcesModuleView.getWindow()`, pas une fenêtre de hutte). Ce que l'onglet
//! **demande** : la vue de module `buildingresources` (`:64`, `:80`), l'inventaire du
//! client (`:76`), le drapeau créatif (`:77`), l'ordre de travail (`:136-145`) et, à
//! chaque image, une vue fraîche (`MarkBuildingDirtyMessage`, `:134`). Ce qu'il **rend** :
//! une ligne de progression, une ligne d'étape, et une ligne par ressource.
//!
//! **Les panneaux sont ceux de `src/ui`** : l'écran `tab.resources` y est déjà, avec ses
//! onze panneaux et son bouton `resourceAdd`. Ce module ajoute ce qu'une table statique
//! ne peut pas porter.
//!
//! # 1. L'ordre de la liste n'est pas celui du rouleau de ressources
//!
//! Les deux écrans trient avec `BuildingBuilderResource.ResourceComparator`, mais **pas
//! avec les mêmes arguments** :
//!
//! | Fenêtre | Argument | Résultat |
//! | --- | --- | --- |
//! | `WindowResourceList` (`:131`) | cinq états, dans l'ordre `NOT_NEEDED, HAVE_ENOUGH, IN_DELIVERY, NEED_MORE, DONT_HAVE` | le plus rare devant |
//! | cet onglet (`:108`, `:260`) | **aucun** | l'ordre de déclaration **renversé** |
//!
//! Sans argument, la `HashMap` d'ordre reste vide (`BuildingBuilderResource.java:213-219`)
//! et `compare` tombe sur sa dernière ligne : `resource2.getAvailabilityStatus()
//! .compareTo(resource1...)` (`:239`), c'est-à-dire l'ordinal **décroissant** —
//! `HAVE_ENOUGH` (4) d'abord, `NOT_NEEDED` (0) dernier. L'onglet du builder montre donc
//! **ce qu'il a déjà en premier**, l'inverse du rouleau. L'état
//! [`crate::ui_window_06::resources::ORDER`] est celui du rouleau ; [`ORDER`] est celui-ci.
//!
//! **L'égalité se tranche par le nom** dans les deux cas (`:229-232`), et c'est le seul
//! point où les deux fenêtres s'accordent.
//!
//! # 2. `IN_DELIVERY` tombe dans le `default`
//!
//! Le `switch` de `updateResourcePane` traite quatre états sur cinq (`:164-191`) :
//! `DONT_HAVE`, `NEED_MORE`, `HAVE_ENOUGH`, et `NOT_NEEDED` — qui partage sa branche avec
//! `default`. Une ressource **en cours de livraison** prend donc la branche « pas
//! nécessaire » : bouton **éteint**, texte **noir**. Elle est en route et l'onglet refuse
//! de l'ajouter, ce qui est correct, mais il l'affiche comme une ressource dont on n'a
//! plus besoin. [`Row::button`] reproduit les cinq cas, [`may_add`] nomme le refus.
//!
//! # 3. L'horloge
//!
//! `tickToInventoryUpdate++ == 20` (`:222-226`) : post-incrément, donc l'intervalle réel
//! est de **21 images**, pas 20. Et un clic pose `-20 * 3 = -60` (`:249`) pour laisser au
//! serveur le temps de répondre : de -60 jusqu'à 20, il faut **81 images**, soit ~4 s, et
//! la valeur affichée après le clic est périmée pendant ce temps. [`Clock`] le compte.
//!
//! # 4. Le pourcentage du joueur n'est pas celui du module
//!
//! `supplied` somme `min(available, amount)` — donc l'**entrepôt**, pas l'inventaire du
//! joueur (`:98`). Le joueur n'entre que dans l'état de la ligne et dans le manque
//! (`:70-73`). Le second argument de la traduction est `moduleView.getProgress()`, qui
//! vaut `max(100 - (int)(progress * 100), 0)` (`BuildingResourcesModuleView.java:92-103`),
//! et non `progress * 100`. [`module_progress`] porte cette inversion.
//!
//! # 5. Le mode créatif ne change pas la progression
//!
//! En créatif, `amountToSet = resource.getAmount()` (`:87-90`) : le joueur « porte » tout
//! ce qui est demandé. Cela change l'état de chaque ligne (plus rien n'est `DONT_HAVE`)
//! et le manque, mais **pas** `supplied`, qui lit `getAvailable()`.

use std::cmp::Ordering;

use crate::ui::action::Refusal as UiRefusal;
use crate::ui::field::{percent, stock_text, PROGRESS_KEY, STEP_KEY};
use crate::ui::screen::{Screen, TAB_RESOURCES_ID};

// L'état et la cascade viennent de `crate::ui_window_06::resources` : c'est **la même**
// méthode Java (`BuildingBuilderResource.getAvailabilityStatus`, `:95-118`) que lit le
// rouleau de ressources. Deux copies pourraient diverger ; il n'y en a qu'une.
pub use crate::ui_window_06::resources::{missing_from_player, status, Availability};

/// La classe (`WindowBuilderResModule.java:36`).
pub const JAVA: &str = "WindowBuilderResModule";
/// La classe de base : un onglet de module (`:36`).
pub const BASE: &str = "AbstractModuleWindow";
/// Le XML (`:63`).
pub const RESOURCE: &str = "minecolonies:gui/layouthuts/layoutbuilderres.xml";
/// L'écran de `src/ui` que cet onglet est.
pub const SCREEN: &str = TAB_RESOURCES_ID;
/// La clé de module lue (`BuildingResourcesModuleView.java:20`), la même que
/// `src/ui::table::MODULE_SCREENS`.
pub const MODULE: &str = "buildingresources";
/// Le libellé du haut, écrit dans le constructeur (`:64`) **et** par
/// `AbstractModuleWindow.setHeader` (`AbstractModuleWindow.java:50`) : deux écritures,
/// un panneau.
pub const DESC: &str = "desc";
/// Le nom de l'ordre de travail (`:141`).
pub const CONSTRUCTION: &str = "constructionName";
/// La ligne d'étape (`:147`).
pub const STEP: &str = "stepprogress";
/// La ligne de progression (`:105`).
pub const PROGRESS: &str = "progress";
/// La liste (`:117`).
pub const LIST: &str = "resources";
/// L'icône d'une ligne (`:215`).
pub const ICON: &str = "resourceIcon";
/// Le nom d'une ligne (`:159`).
pub const NAME: &str = "resourceName";
/// Le manque du joueur — écrit **seulement** s'il est négatif (`:199-208`).
pub const MISSING: &str = "resourceMissing";
/// « disponible / demandé » (`:209`).
pub const AVAILABLE_NEEDED: &str = "resourceAvailableNeeded";
/// Le bouton de la ligne (`:162`).
pub const ADD: &str = "resourceAdd";
/// L'index de la ligne, un panneau d'un pixel (`:210`).
pub const ID_PANE: &str = "resourceId";
/// `amount - available`, un panneau d'un pixel (`:211`).
pub const QUANTITY: &str = "resourceQuantity";
/// Le message qui redemande une vue fraîche (`:134`).
pub const REFRESH_MESSAGE: &str = "MarkBuildingDirtyMessage";
/// Le message d'un clic sur [`ADD`] (`:261`).
pub const TRANSFER_MESSAGE: &str = "TransferItemsRequestMessage";

/// L'ordre de **cet** onglet : `ResourceComparator()` sans argument, donc l'ordinal
/// décroissant de `BuildingBuilderResource.java:194-201`.
pub const ORDER: [Availability; 5] = [
    Availability::HaveEnough,
    Availability::NeedMore,
    Availability::DontHave,
    Availability::InDelivery,
    Availability::NotNeeded,
];

/// L'écran de `src/ui`, pour ses panneaux.
pub fn screen() -> Option<&'static Screen> {
    crate::ui::table::screen_by_id(SCREEN).ok()
}

/// Le rang dans [`ORDER`], c'est-à-dire la place dans la liste.
pub fn rank(state: Availability) -> usize {
    ORDER.iter().position(|other| *other == state).unwrap_or(ORDER.len())
}

/// La couleur qu'une ligne prend dans cet état (`:164-191`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre couleurs du fichier (`:41-44`), une par branche
pub enum Tint {
    /// `RED` : rien en stock ny chez le joueur.
    Red,
    /// `ORANGE` : le joueur en a, pas assez.
    Orange,
    /// `DARKGREEN` : le joueur en a assez pour finir la ligne.
    DarkGreen,
    /// `BLACK` : plus rien à faire — ou déjà en livraison (voir la note 2).
    Black,
}

/// L'état du bouton [`ADD`] d'une ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowButton {
    /// `Button.enable()` / `disable()`.
    pub active: bool,
    /// La couleur des trois textes de la ligne.
    pub tint: Tint,
}

impl RowButton {
    /// Le bouton est dessiné et cliquable.
    pub fn is_clickable(&self) -> bool {
        self.active
    }
}

// `switch (resource.getAvailabilityStatus())` — cinq états, quatre branches, et
// `NOT_NEEDED` partage la sienne avec `default` (`:164-191`).
/// L'état du bouton pour cet état de ressource.
pub fn button(state: Availability) -> RowButton {
    match state {
        Availability::DontHave => RowButton { active: false, tint: Tint::Red },
        Availability::NeedMore => RowButton { active: true, tint: Tint::Orange },
        Availability::HaveEnough => RowButton { active: true, tint: Tint::DarkGreen },
        // `IN_DELIVERY` n'a pas de branche : il tombe dans `default`, avec
        // `NOT_NEEDED`. Voir la note 2 du module.
        Availability::NotNeeded | Availability::InDelivery => {
            RowButton { active: false, tint: Tint::Black }
        }
    }
}

/// Pourquoi le bouton de cette ligne est éteint, avec les refus de `src/ui`.
///
/// `ResourceMissing` est la clé que `src/ui` donne à `DONT_HAVE` (`:166-171`) et
/// `ResourceNotNeeded` celle de la branche `default` (`:184-190`). **`IN_DELIVERY` prend
/// le nom de `NOT_NEEDED`** : Java n'a pas d'autre nom pour lui, puisqu'il ne dit rien.
pub fn may_add(state: Availability) -> Result<(), UiRefusal> {
    match state {
        Availability::NeedMore | Availability::HaveEnough => Ok(()),
        Availability::DontHave => Err(UiRefusal::ResourceMissing),
        Availability::NotNeeded | Availability::InDelivery => Err(UiRefusal::ResourceNotNeeded),
    }
}

/// Une ligne de [`LIST`], avec les cinq nombres que Java lui donne.
///
/// Ce n'est **pas** `crate::ui_window_06::resources::Row` : celui-ci porte un sixième
/// champ (`warehouse`, l'instantané du rouleau) et une pastille de livraison/entrepôt que
/// cet onglet ne dessine jamais. La logique, elle, est partagée : [`status`] et
/// [`missing_from_player`] sont réexportés de `ui_window_06`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// `getName()`, le nom affiché de l'item — et la clé d'égalité du tri (`:231`).
    pub name: String,
    /// Combien le build en demande (`getAmount()`).
    pub amount: i32,
    /// Combien le coffre et le builder en ont (`getAvailable()`, `:209`).
    pub available: i32,
    /// Combien le joueur en porte (`getPlayerAmount()`, écrit à chaque rafraîchissement,
    /// `:97`).
    pub player: i32,
    /// Combien sont déjà en route (`getAmountInDelivery()`).
    pub in_delivery: i32,
}

impl Row {
    /// Une ligne telle que `pullResourcesFromHut` la construit (`:79-101`).
    pub fn new(name: &str, amount: i32, available: i32, player: i32, in_delivery: i32) -> Row {
        Row { name: name.to_owned(), amount, available, player, in_delivery }
    }

    /// L'état, recalculé à chaque image (`:164`).
    pub fn status(&self) -> Availability {
        status(self.amount, self.available, self.player, self.in_delivery)
    }

    /// `player + available - amount` (`BuildingBuilderResource.java:70-73`).
    pub fn missing(&self) -> i32 {
        missing_from_player(self.player, self.available, self.amount)
    }

    /// Le texte de [`MISSING`] : `Some` **seulement** si le manque est négatif — donc
    /// Java n'y écrit jamais le manque, il y écrit le surplus (`:199-208`).
    pub fn missing_text(&self) -> Option<String> {
        let missing = self.missing();
        if missing < 0 {
            Some(missing.to_string())
        } else {
            None
        }
    }

    /// « disponible / demandé » (`:209`), par la fonction de `src/ui`.
    pub fn stock_text(&self) -> String {
        stock_text(self.available, self.amount)
    }

    /// Ce que le panneau caché [`QUANTITY`] porte : `amount - available` (`:211`).
    pub fn quantity(&self) -> i32 {
        self.amount - self.available
    }

    /// L'état du bouton de la ligne.
    pub fn button(&self) -> RowButton {
        button(self.status())
    }
}

/// L'ordre de la liste : même comparateur Java, **argument vide** (note 1).
pub fn compare(left: &Row, right: &Row) -> Ordering {
    if left.status() == right.status() {
        // `String.compareTo` sur les noms d'items, tous en ASCII.
        return left.name.cmp(&right.name);
    }
    rank(left.status()).cmp(&rank(right.status()))
}

/// Trie les lignes sur place (`:108`, `:260`).
pub fn sort(rows: &mut [Row]) {
    rows.sort_by(compare);
}

/// Le montant que `:87-96` donne au joueur : tout, en créatif, sinon ce qu'il compte.
pub fn player_amount(creative: bool, counted: i32, amount: i32) -> i32 {
    if creative {
        amount
    } else {
        counted
    }
}

/// `supplied` : la somme des `min(available, amount)` — l'entrepôt, pas l'inventaire
/// (`:98`).
pub fn supplied(rows: &[Row]) -> i32 {
    rows.iter().map(|row| row.available.min(row.amount)).sum()
}

/// `total` : la somme des `amount` (`:100`).
pub fn total(rows: &[Row]) -> i32 {
    rows.iter().map(|row| row.amount).sum()
}

/// La ligne [`PROGRESS`], ou `None` quand Java n'écrit rien.
///
/// Deux arguments pour la clé de `src/ui` : le pourcentage du joueur (arrondi par
/// troncature, [`percent`]) et celui du module, déjà en pourcentage.
pub fn progress_args(rows: &[Row], module: i32) -> Option<(String, String)> {
    let total = total(rows);
    if total <= 0 {
        // `if (total > 0)` : le panneau garde le `label=""` de l'XML (`:103`).
        return None;
    }
    Some((format!("{}%", percent(supplied(rows), total)), format!("{module}%")))
}

/// La clé de [`PROGRESS`], réexportée pour que l'appelant n'ait pas deux noms.
pub const PROGRESS_LABEL: &str = PROGRESS_KEY;

/// Les deux arguments de [`STEP`] : `getCurrentStage()`, `getTotalStages()` (`:147`),
/// écrits **toujours**, sans condition.
pub fn step_args(current: i32, total: i32) -> (i32, i32) {
    (current, total)
}

/// La clé de [`STEP`].
pub const STEP_LABEL: &str = STEP_KEY;

/// `moduleView.getProgress()` (`BuildingResourcesModuleView.java:92-103`).
///
/// `progress` arrive en fraction (`0.25` = un quart fait) et la fenêtre affiche
/// `100 - 25` : c'est un **reste**, pas un avancement.
pub fn module_progress(raw: f64, finished_stages: i32, total_stages: i32) -> i32 {
    let local = (100 - (raw * 100.0) as i32).max(0);
    // `if (finishedStages == 0) { if (totalStages == finishedStages) { return 0; } }`
    // (`:95-102`) : deux étages à zéro, ce que le défaut de `totalStages` (1) rend
    // presque impossible.
    if finished_stages == 0 && total_stages == finished_stages {
        return 0;
    }
    local
}

/// L'horloge de `tickToInventoryUpdate` (`:55`, `:222-226`, `:249`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    counter: i32,
}

impl Clock {
    /// Une horloge neuve : `0` (`:55`).
    pub const fn new() -> Clock {
        Clock { counter: 0 }
    }

    /// La valeur brute, pour qui veut la comparer à celle de Java.
    pub const fn counter(&self) -> i32 {
        self.counter
    }

    /// Une image de plus ; vrai quand la liste doit être retirée de l'inventaire.
    ///
    /// C'est `tickToInventoryUpdate++ == 20` : le test lit la valeur **avant** de
    /// l'incrémenter, et la remise à zéro se fait après.
    pub fn tick(&mut self) -> bool {
        let refresh = self.counter == 20;
        self.counter += 1;
        if refresh {
            self.counter = 0;
        }
        refresh
    }

    /// Le clic : trois secondes de silence (`:249`).
    pub fn hold(&mut self) {
        self.counter = -20 * 3;
    }
}

impl Default for Clock {
    fn default() -> Clock {
        Clock::new()
    }
}

/// Ce qu'un clic sur [`ADD`] a changé dans la ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    /// L'index lu dans le panneau [`ID_PANE`] — l'index **d'avant** le tri, puisque
    /// Java retrie juste après le clic (`:260`).
    pub index: i32,
    /// La quantité **lue** dans [`QUANTITY`] et envoyée au serveur (`:256`, `:261`).
    /// Java ne la revalide pas : la ligne, elle, se base sur `needed`.
    pub quantity: i32,
    /// `amount - available` avant le clic (`:257`).
    pub needed: i32,
    /// L'état de la ligne après le clic.
    pub available: i32,
    /// L'inventaire du joueur après le clic.
    pub player: i32,
}

/// Le clic sur [`ADD`] : lire deux panneaux cachés, puis appliquer (`:235-262`).
///
/// Java éteint le bouton avant de lire (`:238`) pour qu'un double clic ne passe pas deux
/// fois ; ici, c'est au clic suivant d'être refusé, par [`may_add`].
pub fn apply_click(
    rows: &mut [Row],
    index_text: &str,
    quantity_text: &str,
) -> Result<Applied, super::refusal::Refusal> {
    use super::refusal::Refusal;
    // `Integer.parseInt(idLabel.getTextAsString())` (`:240`).
    let index: i32 = index_text.parse().map_err(|_| Refusal::BadRowIndex {
        text: index_text.to_owned(),
    })?;
    let quantity: i32 = quantity_text.parse().map_err(|_| Refusal::BadQuantity {
        text: quantity_text.to_owned(),
    })?;
    if index < 0 || index as usize >= rows.len() {
        return Err(Refusal::UnknownResource { index });
    }
    let slot = index as usize;
    let needed = rows[slot].amount - rows[slot].available;
    // `setAvailable(min(amount, available + playerAmount))` (`:258`).
    let available = rows[slot].amount.min(rows[slot].available + rows[slot].player);
    // `setPlayerAmount(max(0, playerAmount - needed))` (`:259`).
    let player = (rows[slot].player - needed).max(0);
    rows[slot].available = available;
    rows[slot].player = player;
    Ok(Applied { index, quantity, needed, available, player })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_state(rows: &[Row]) -> Vec<Availability> {
        rows.iter().map(Row::status).collect()
    }

    #[test]
    fn the_tab_sorts_what_it_has_first_and_the_scroll_sorts_it_last() {
        let mut rows = vec![
            Row::new("dont_have", 64, 0, 0, 0),
            Row::new("have_enough", 64, 0, 64, 0),
            Row::new("not_needed", 4, 9, 0, 0),
            Row::new("in_delivery", 8, 2, 0, 3),
            Row::new("need_more", 8, 2, 1, 0),
        ];
        sort(&mut rows);
        assert_eq!(
            sorted_state(&rows),
            [
                Availability::HaveEnough,
                Availability::NeedMore,
                Availability::DontHave,
                Availability::InDelivery,
                Availability::NotNeeded,
            ],
            "ResourceComparator() sans argument compare les ordinaux à l'envers (:239)"
        );
        // Le rouleau, lui, met le plus rare devant : les deux ordres sont opposés.
        assert_eq!(
            crate::ui_window_06::resources::ORDER,
            [
                Availability::NotNeeded,
                Availability::HaveEnough,
                Availability::InDelivery,
                Availability::NeedMore,
                Availability::DontHave,
            ]
        );
        assert_eq!(rank(Availability::HaveEnough), 0);
        assert_eq!(rank(Availability::NotNeeded), 4);
    }

    #[test]
    fn two_rows_in_the_same_state_are_ordered_by_name() {
        let mut rows = vec![Row::new("Zinc", 4, 4, 0, 0), Row::new("Apple", 4, 4, 0, 0)];
        sort(&mut rows);
        assert_eq!(rows[0].name, "Apple");
        assert_eq!(rows[1].name, "Zinc");
        // Le nom ne tranche que l'égalité d'état (:229-232).
        assert_eq!(compare(&rows[0], &rows[1]), Ordering::Less);
    }

    #[test]
    fn in_delivery_takes_the_not_needed_branch_and_its_button_is_dead() {
        let row = Row::new("oak", 8, 2, 0, 3);
        assert_eq!(row.status(), Availability::InDelivery);
        let button = row.button();
        assert!(!button.is_clickable(), "default: addButton.disable() (:186)");
        assert_eq!(button.tint, Tint::Black);
        // Et il porte le nom du refus de « pas nécessaire », faute d'un autre.
        assert_eq!(may_add(Availability::InDelivery), Err(UiRefusal::ResourceNotNeeded));
        assert_eq!(
            may_add(Availability::DontHave),
            Err(UiRefusal::ResourceMissing),
            "DONT_HAVE éteint le bouton (:167)"
        );
        assert_eq!(may_add(Availability::NeedMore), Ok(()));
        assert_eq!(may_add(Availability::HaveEnough), Ok(()));
    }

    #[test]
    fn the_four_branches_give_four_colours() {
        assert_eq!(button(Availability::DontHave).tint, Tint::Red);
        assert_eq!(button(Availability::NeedMore).tint, Tint::Orange);
        assert_eq!(button(Availability::HaveEnough).tint, Tint::DarkGreen);
        assert_eq!(button(Availability::NotNeeded).tint, Tint::Black);
        // Deux états éteignent le bouton, trois ne l'allument pas.
        let lit = ORDER.iter().filter(|state| button(**state).active).count();
        assert_eq!(lit, 2);
    }

    #[test]
    fn the_missing_pane_holds_the_surplus_or_nothing() {
        // `getMissingFromPlayer() = player + available - amount`.
        let short = Row::new("stone", 64, 10, 4, 0);
        assert_eq!(short.missing(), -50);
        assert_eq!(short.missing_text().as_deref(), Some("-50"));
        // Exactement assez : manque nul, panneau vide.
        let exact = Row::new("stone", 64, 60, 4, 0);
        assert_eq!(exact.missing(), 0);
        assert_eq!(exact.missing_text(), None);
        // Plus que nécessaire : le manque est **positif** et Java n'écrit quand même
        // rien (`:199-208` ne teste que `< 0`) — le panneau ne montre donc jamais un
        // manque, seulement un surplus.
        let over = Row::new("stone", 10, 0, 100, 0);
        assert_eq!(over.missing(), 90);
        assert_eq!(over.missing_text(), None);
    }

    #[test]
    fn a_row_reads_available_over_amount_and_the_hidden_quantity_is_what_is_missing() {
        let row = Row::new("planks", 32, 12, 0, 0);
        assert_eq!(row.stock_text(), "12 / 32");
        assert_eq!(row.quantity(), 20);
        // Une ligne qui a dépassé la demande garde son stock et une quantité négative.
        let over = Row::new("planks", 8, 20, 0, 0);
        assert_eq!(over.stock_text(), "20 / 8");
        assert_eq!(over.quantity(), -12);
    }

    #[test]
    fn the_progress_line_is_the_warehouse_and_the_module_rest() {
        let rows = vec![
            Row::new("a", 10, 5, 0, 0),
            Row::new("b", 10, 10, 0, 0),
        ];
        assert_eq!(supplied(&rows), 15);
        assert_eq!(total(&rows), 20);
        assert_eq!(
            progress_args(&rows, 40),
            Some(("75%".to_owned(), "40%".to_owned()))
        );
        // Le joueur ne compte pas dans `supplied` : son inventaire est plein, rien ne
        // bouge (`:98`).
        let with_player = vec![Row::new("a", 10, 5, 99, 0)];
        assert_eq!(supplied(&with_player), 5);
        assert_eq!(progress_args(&with_player, 0), Some(("50%".to_owned(), "0%".to_owned())));
        // Aucun total : le panneau n'est pas écrit (`:103`).
        assert_eq!(progress_args(&[], 12), None);
        assert_eq!(progress_args(&[Row::new("a", 0, 0, 0, 0)], 12), None);
    }

    #[test]
    fn the_module_progress_is_a_rest_and_not_an_advance() {
        assert_eq!(module_progress(0.0, 1, 3), 100);
        assert_eq!(module_progress(0.25, 1, 3), 75);
        assert_eq!(module_progress(1.0, 3, 3), 0);
        // Au-delà de 1, le `max(..., 0)` de Java ne suffit pas à lui seul : il borne la
        // partie gauche, donc le reste ne descend pas sous zéro.
        assert_eq!(module_progress(1.5, 2, 3), 0);
        // `(int)` tronque vers zéro, comme le cast de Java.
        assert_eq!(module_progress(0.339, 1, 3), 67);
        // La branche `finishedStages == 0` ne se déclenche que si le total est nul aussi.
        assert_eq!(module_progress(0.9, 0, 1), 10);
        assert_eq!(module_progress(0.9, 0, 0), 0);
        assert_eq!(step_args(2, 5), (2, 5));
    }

    #[test]
    fn the_clock_refreshes_every_twenty_one_frames_and_waits_eighty_one_after_a_click() {
        let mut clock = Clock::new();
        let mut frames = 0;
        while !clock.tick() {
            frames += 1;
            assert!(frames < 100, "l'horloge ne s'est jamais déclenchée");
        }
        // `tickToInventoryUpdate++ == 20` : 21 images pour la première, puis 21 encore.
        assert_eq!(frames + 1, 21);
        let mut next = 0;
        while !clock.tick() {
            next += 1;
        }
        assert_eq!(next + 1, 21);
        // Le clic pose -60 (:249) : 60 images de retard et 21 de cycle.
        clock.hold();
        assert_eq!(clock.counter(), -60);
        let mut after_click = 0;
        while !clock.tick() {
            after_click += 1;
        }
        assert_eq!(after_click + 1, 81);
    }

    #[test]
    fn a_click_moves_the_player_stock_to_the_row_and_sorts_it_after() {
        let mut rows = vec![Row::new("oak", 16, 4, 8, 0), Row::new("stone", 64, 0, 64, 0)];
        // Le panneau caché porte des chaînes, et Java les relit par parseInt (:240).
        let applied = apply_click(&mut rows, "0", "12").expect("l'index 0 existe");
        assert_eq!(applied.index, 0);
        assert_eq!(applied.quantity, 12, "la quantité envoyée est celle lue (:256)");
        assert_eq!(applied.needed, 12);
        assert_eq!(rows[0].available, 12, "min(16, 4 + 8)");
        assert_eq!(rows[0].player, 0, "max(0, 8 - 12)");
        // Il manque encore 4 : la ligne reste `DONT_HAVE`, et le tri la fait
        // **descendre** derrière celle qui n'a plus besoin de rien.
        assert_eq!(rows[0].status(), Availability::DontHave);
        assert_eq!(rows[0].stock_text(), "12 / 16");
        sort(&mut rows);
        assert_eq!(rows[0].name, "stone");
        assert_eq!(rows[1].name, "oak");
        // L'index du panneau caché ne vaut donc que jusqu'au tri suivant (:260).
        assert_eq!(ordered_names(&rows), ["stone", "oak"]);
    }

    fn ordered_names(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.name.as_str()).collect()
    }

    #[test]
    fn a_click_on_a_hidden_pane_is_refused_by_name() {
        use super::super::refusal::Refusal as MapRefusal;
        let mut rows = vec![Row::new("oak", 16, 4, 8, 0)];
        let bad_index = apply_click(&mut rows, "x", "1").err();
        assert_eq!(bad_index, Some(MapRefusal::BadRowIndex { text: "x".to_owned() }));
        assert_eq!(bad_index.as_ref().map(|refusal| refusal.key()), Some(None));
        // Un index hors liste : Java journalise et ne fait rien (:242-245).
        assert_eq!(
            apply_click(&mut rows, "9", "1").err(),
            Some(MapRefusal::UnknownResource { index: 9 })
        );
        assert_eq!(
            apply_click(&mut rows, "-1", "1").err(),
            Some(MapRefusal::UnknownResource { index: -1 })
        );
        assert_eq!(
            apply_click(&mut rows, "0", "").err(),
            Some(MapRefusal::BadQuantity { text: String::new() })
        );
        // Aucun refus n'a modifié la ligne.
        assert_eq!(rows[0], Row::new("oak", 16, 4, 8, 0));
    }

    #[test]
    fn creative_mode_fills_the_player_side_and_leaves_the_warehouse_alone() {
        assert_eq!(player_amount(true, 3, 64), 64);
        assert_eq!(player_amount(false, 3, 64), 3);
        // Une ressource que le builder n'a pas reste `DONT_HAVE` en créatif ? Non :
        // le joueur « porte » la demande entière, donc la ligne passe en HAVE_ENOUGH.
        let row = Row::new("stone", 64, 0, player_amount(true, 0, 64), 0);
        assert_eq!(row.status(), Availability::HaveEnough);
        // Mais `supplied` ne lit pas le joueur : la progression reste à zéro.
        assert_eq!(supplied(&[row]), 0);
    }

    #[test]
    fn the_eleven_panes_and_the_one_button_are_those_of_src_ui() {
        let screen = screen().expect("tab.resources est un écran de src/ui");
        assert_eq!(screen.window, JAVA);
        assert_eq!(screen.resource, RESOURCE);
        for id in [DESC, CONSTRUCTION, STEP, PROGRESS] {
            assert!(screen.field(id).is_some(), "{id} n'est pas un panneau de l'écran");
        }
        for id in [ICON, NAME, MISSING, AVAILABLE_NEEDED, ID_PANE, QUANTITY] {
            assert!(screen.field(id).is_some(), "{id} n'est pas un panneau de ligne");
        }
        assert!(screen.field(LIST).is_some());
        assert_eq!(screen.actions.len(), 1, "un seul bouton : resourceAdd");
        assert_eq!(screen.action(ADD).map(|action| action.is_guarded()), Some(true));
        // Les deux clés de libellé sont celles de `src/ui`, pas des copies.
        assert_eq!(PROGRESS_LABEL, crate::ui::field::PROGRESS_KEY);
        assert_eq!(STEP_LABEL, crate::ui::field::STEP_KEY);
    }
}
