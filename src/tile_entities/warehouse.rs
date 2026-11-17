//! L'entrepôt : où ranger un objet, et ce qu'il annonce quand il n'y a plus de place.
//!
//! Référence Java : `core/tileentities/TileEntityWareHouse.java` (277 lignes) sur
//! `api/tileentities/AbstractTileEntityWareHouse.java` (86 lignes).
//!
//! **C'est une hutte de bâtiment, plus une recherche.** La classe Java étend
//! `TileEntityColonyBuilding` (`:234`) : elle hérite du rattachement, du schéma
//! et de l'accès, et ce fichier n'en reprend rien — les règles communes sont
//! dans [`super::building`], et [`Warehouse`] ne porte que ce que l'entrepôt
//! ajoute : ses racks, et son compteur de messages.
//!
//! **La recherche regarde les racks de SON bâtiment.** Java parcourt
//! `getBuilding().getContainers()` (`:47`, `:77`, `:109`) ; le port prend la
//! même liste de positions en paramètre, sous forme `(position, contenu)` : une
//! liste de racks déjà lue, triée, sans jeu. Un rack non chargé est ignoré par
//! Java (`WorldUtil.isBlockLoaded`, `:49`) : le port ne peut pas le savoir, et
//! l'appelant ne met pas dans la liste un rack qu'il n'a pas lu.
//!
//! **Le choix d'un rack a trois étages** (`getRackForStack`, `:178-186`) : même
//! objet et une place libre, puis objet « similaire », puis le rack le plus vide.
//! Le deuxième étage est `hasSimilarStack` (`:240-264`), qui compare un **onglet
//! créatif** — un registre du jeu que le SDK n'expose pas : il est sauté, et
//! `MANQUANT.md` le dit. Sauter un étage change où va un objet, jamais s'il est
//! rangé : le troisième étage accepte toujours un rack vide.

use super::rack::Content;
use crate::base::Pos;
use crate::inventory::ItemName;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:35`).
pub const TYPE_ID: &str = "minecolonies:warehouse";

/// Le nombre de niveaux d'amélioration de stockage d'un entrepôt
/// (`BuildingWareHouse.java:51`).
pub const MAX_STORAGE_UPGRADE: i32 = 3;
/// Le délai entre deux messages de « plein » : cinq minutes
/// (`Constants.java:35`, testé à `TileEntityWareHouse.java:141`).
pub const NOTIFY_INTERVAL: u64 = (5 * 60 * super::TICKS_PER_SECOND) as u64;

/// Un rack tel que la recherche le voit : sa position, et son contenu déjà lu.
pub type Shelf<'a> = (Pos, &'a Content);

/// Pourquoi l'entrepôt refuse un objet. Java choisit un message
/// (`TileEntityWareHouse.java:144-158`) ; le port nomme le cas, la phrase reste
/// dans les ressources du paquet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les trois messages de Java
pub enum Full {
    /// Le bâtiment n'est pas au niveau maximum : il peut grandir
    /// (`:157`).
    CanGrow,
    /// Niveau maximum, stockage améliorable (`:146-150`).
    StorageUpgradeLeft,
    /// Niveau maximum, stockage au bout (`:152-155`).
    NothingLeft,
}

impl Full {
    /// La clé de traduction que Java envoie pour ce cas
    /// (`api/util/constant/TranslationConstants.java:57-61`), recopiée pour
    /// qu'un test la compare.
    pub fn message(self) -> &'static str {
        match self {
            Full::CanGrow => "com.minecolonies.coremod.warehouse.full",
            Full::StorageUpgradeLeft => "com.minecolonies.coremod.warehouse.full.level5",
            Full::NothingLeft => "com.minecolonies.coremod.warehouse.full.max",
        }
    }
}

/// L'état que l'entrepôt ajoute à celui d'une hutte : l'horloge des messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : un seul champ, comme la classe Java
pub struct Warehouse {
    /// Le tick du dernier message de « plein » (`:33`), `0` au premier.
    last_notification: u64,
}

impl Warehouse {
    pub fn new() -> Warehouse {
        Warehouse::default()
    }

    pub fn last_notification(&self) -> u64 {
        self.last_notification
    }

    /// La valeur posée par l'ouverture d'un entrepôt : `inWarehouse = true`
    /// (`:39`). Elle vit dans le rack (`super::rack::Rack::set_in_warehouse`) ;
    /// cette fonction la nomme pour que l'appelant n'ait pas à la deviner.
    pub fn shelves_are_in_warehouse() -> bool {
        true
    }

    /// Le message de « plein » est-il dû à ce tick ? `now - last > 6000`
    /// (`:141`) : **strictement** plus de cinq minutes, pas cinq.
    pub fn message_due(&self, now: u64) -> bool {
        now.saturating_sub(self.last_notification) > NOTIFY_INTERVAL
    }

    /// Note le message envoyé (`:143`).
    pub fn noted(&mut self, now: u64) {
        self.last_notification = now;
    }

    /// Le message qui correspond à l'état du bâtiment (`:144-158`) : niveau
    /// maximum ou non, stockage au bout ou non.
    pub fn fullness(at_max_level: bool, storage_upgrade: i32) -> Full {
        if !at_max_level {
            Full::CanGrow
        } else if storage_upgrade < MAX_STORAGE_UPGRADE {
            Full::StorageUpgradeLeft
        } else {
            Full::NothingLeft
        }
    }
}

/// **La question de l'entrepôt.** Y a-t-il au moins `count` objets qui passent
/// `predicate`, tous racks confondus ? Java s'arrête au premier rack qui suffit
/// (`:54-57`) ; le port additionne et compare, ce qui donne le même booléen.
pub fn holds(shelves: &[Shelf<'_>], predicate: impl Fn(&ItemName) -> bool, count: u32) -> bool {
    let mut total = 0u32;
    for (_, content) in shelves {
        total = total.saturating_add(content.item_count(&predicate));
        if total >= count {
            return true;
        }
    }
    false
}

/// `hasMatchingItemStackInWarehouse(stack, count, ignoreNbt, ignoreDamage, leftOver)`
/// (`:75-92`) : le total trouvé, **diminué du reliquat** que l'entrepôt doit
/// toujours garder. Un `leftOver` plus grand que le stock rend le compte négatif
/// en Java (`0 - leftOver`) ; ici la soustraction sature à zéro, et la
/// comparaison `>= count` donne le même résultat.
pub fn count_with_reserve(shelves: &[Shelf<'_>], item: &ItemName, left_over: u32) -> u32 {
    let found = shelves
        .iter()
        .map(|(_, content)| content.count(item))
        .fold(0u32, u32::saturating_add);
    found.saturating_sub(left_over)
}

/// Peut-on honorer `count` de cet objet en gardant `left_over` ?
pub fn can_supply(shelves: &[Shelf<'_>], item: &ItemName, count: u32, left_over: u32) -> bool {
    count_with_reserve(shelves, item, left_over) >= count
}

/// Les objets qui passent `predicate`, avec la position du rack qui les tient
/// (`getMatchingItemStacksInWarehouse`, `:105-126`) : un rack vide, ou dont
/// aucun objet ne passe, ne rend rien.
pub fn matching(shelves: &[Shelf<'_>], predicate: impl Fn(&ItemName) -> bool) -> Vec<(ItemName, Pos)> {
    let mut found = Vec::new();
    for (at, content) in shelves {
        if content.is_empty() {
            continue;
        }
        for (item, count) in content.counts() {
            if *count > 0 && predicate(item) {
                // Un rack qui tient l'objet une fois suffit : Java ajoute une
                // entrée par stack trouvée dans l'inventaire, le port une par
                // objet, ce qui est la même position et la même réponse.
                found.push((item.clone(), *at));
            }
        }
    }
    found
}

/// Où ranger `item` (`getRackForStack`, `:178-186`) : le premier rack qui en tient
/// déjà et a une place libre, sinon le rack le plus vide.
///
/// L'étage « objet similaire » de Java (`:181-183`) est sauté : il demande
/// l'onglet créatif du registre. [`most_empty`] rend le premier rack vide s'il y
/// en a un, comme `searchMostEmptyRack` (`:246-271`).
pub fn rack_for(shelves: &[Shelf<'_>], item: &ItemName) -> Option<Pos> {
    if let Some((at, _)) = shelves
        .iter()
        .find(|(_, content)| content.free_slots() > 0 && content.count(item) > 0)
    {
        return Some(*at);
    }
    most_empty(shelves)
}

/// Le rack le plus vide (`searchMostEmptyRack`, `:246-271`) : **le premier rack
/// vide** s'il y en a un — Java rend tout de suite (`:255-258`) —, sinon celui
/// qui a le plus de slots libres, et `None` si aucun n'en a.
pub fn most_empty(shelves: &[Shelf<'_>]) -> Option<Pos> {
    let mut best: Option<(Pos, usize)> = None;
    for (at, content) in shelves {
        let free = content.free_slots();
        if free == 0 && !content.is_empty() {
            continue;
        }
        if content.is_empty() {
            return Some(*at);
        }
        if best.is_none_or(|(_, best_free)| free > best_free) {
            best = Some((*at, free));
        }
    }
    best.map(|(at, _)| at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Stack;

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("nom d'objet valide")
    }

    fn content(entries: &[(&str, u32)], empty_slots: usize) -> Content {
        let mut slots: Vec<Option<Stack>> = entries
            .iter()
            .map(|(name, count)| Stack::new(item(name), *count).ok())
            .collect();
        slots.extend(std::iter::repeat_n(None, empty_slots));
        Content::from_slots(&slots)
    }

    #[test]
    fn un_entrepot_tient_un_objet_si_assez_de_racks_en_ont() {
        let first = content(&[("minecraft:oak_log", 10)], 26);
        let second = content(&[("minecraft:oak_log", 5)], 26);
        let shelves = [([0, 64, 0], &first), ([2, 64, 0], &second)];
        assert!(holds(&shelves, |name| name.as_str() == "minecraft:oak_log", 15));
        assert!(!holds(&shelves, |name| name.as_str() == "minecraft:oak_log", 16));
        assert!(!holds(&shelves, |_| true, 16));
    }

    #[test]
    fn le_reliquat_se_garde_et_un_reliquat_trop_gros_ne_rend_pas_negatif() {
        let rack = content(&[("minecraft:oak_log", 10)], 26);
        let shelves = [([0, 64, 0], &rack)];
        assert!(can_supply(&shelves, &item("minecraft:oak_log"), 4, 6));
        assert!(!can_supply(&shelves, &item("minecraft:oak_log"), 5, 6));
        assert_eq!(count_with_reserve(&shelves, &item("minecraft:oak_log"), 40), 0);
        assert!(!can_supply(&shelves, &item("minecraft:oak_log"), 1, 40));
    }

    #[test]
    fn la_liste_des_objets_trouves_porte_la_position_du_rack() {
        let first = content(&[("minecraft:oak_log", 3), ("minecraft:stone", 2)], 25);
        let empty = Content::default();
        let shelves = [([1, 64, 1], &first), ([9, 64, 9], &empty)];
        let found = matching(&shelves, |name| name.as_str().starts_with("minecraft:oak"));
        assert_eq!(found, vec![(item("minecraft:oak_log"), [1, 64, 1])]);
        assert!(matching(&shelves, |_| false).is_empty());
    }

    #[test]
    fn un_objet_deja_stocke_va_dans_son_rack_s_il_reste_de_la_place() {
        let full = content(&[("minecraft:oak_log", 64)], 0);
        let same_item = content(&[("minecraft:oak_log", 10)], 20);
        let other = content(&[("minecraft:stone", 1)], 26);
        let shelves = [
            ([0, 64, 0], &other),
            ([1, 64, 0], &same_item),
            ([2, 64, 0], &full),
        ];
        assert_eq!(rack_for(&shelves, &item("minecraft:oak_log")), Some([1, 64, 0]));
        // Rien de connu : direction le rack le plus vide, qui est celui d'un
        // objet inconnu et non le rack plein.
        assert_eq!(rack_for(&shelves, &item("minecraft:dirt")), Some([0, 64, 0]));
    }

    #[test]
    fn un_rack_plein_d_un_autre_objet_ne_se_propose_pas() {
        let full = content(&[("minecraft:oak_log", 64), ("minecraft:stone", 64)], 0);
        let shelves = [([0, 64, 0], &full)];
        assert_eq!(most_empty(&shelves), None);
        assert_eq!(rack_for(&shelves, &item("minecraft:dirt")), None);
    }

    #[test]
    fn le_premier_rack_vide_gagne_avant_le_plus_grand_nombre_de_slots_libres() {
        let roomy = content(&[("minecraft:stone", 64)], 26);
        let empty = Content::default();
        let shelves = [([0, 64, 0], &roomy), ([1, 64, 0], &empty)];
        assert_eq!(most_empty(&shelves), Some([1, 64, 0]));
        // Sans rack vide, c'est le plus de slots libres.
        let elsewhere = content(&[("minecraft:dirt", 64)], 24);
        let shelves = [([0, 64, 0], &roomy), ([3, 64, 0], &elsewhere)];
        assert_eq!(most_empty(&shelves), Some([0, 64, 0]));
    }

    #[test]
    fn le_message_de_plein_revient_toutes_les_cinq_minutes_passees() {
        let mut warehouse = Warehouse::new();
        assert!(
            !warehouse.message_due(0),
            "`gameTime - 0 > 6000` est faux à zéro (`:141`)"
        );
        assert!(!warehouse.message_due(6_000), "exactement 5 minutes : pas encore");
        assert!(warehouse.message_due(6_001), "une seconde de plus");
        warehouse.noted(1_000);
        assert!(!warehouse.message_due(7_000), "6 000 ticks après : pas plus");
        assert!(warehouse.message_due(7_001), "6 001 ticks après");
    }

    #[test]
    fn le_message_de_plein_depend_du_niveau_et_du_stockage() {
        assert_eq!(Warehouse::fullness(false, 0), Full::CanGrow);
        assert_eq!(Warehouse::fullness(true, 0), Full::StorageUpgradeLeft);
        assert_eq!(Warehouse::fullness(true, MAX_STORAGE_UPGRADE - 1), Full::StorageUpgradeLeft);
        assert_eq!(Warehouse::fullness(true, MAX_STORAGE_UPGRADE), Full::NothingLeft);
        let mut messages = std::collections::BTreeSet::new();
        for kind in [Full::CanGrow, Full::StorageUpgradeLeft, Full::NothingLeft] {
            assert!(messages.insert(kind.message()));
        }
        assert_eq!(messages.len(), 3, "trois cas, trois messages");
    }

    #[test]
    fn un_entrepot_met_ses_racks_dans_l_entrepot() {
        assert!(Warehouse::shelves_are_in_warehouse());
    }
}
