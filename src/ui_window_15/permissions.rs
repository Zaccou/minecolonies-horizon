//! La page des permissions : le rang selectionne, la matrice, les cases libres.
//!
//! Java reference : `core/client/gui/townhall/WindowPermissionsPage.java` (751
//! lignes), `gui/townhall/layoutpermissions.xml` (trois vues dans un `switch`),
//! et `core/colony/permissions/Permissions.java` pour les deux regles que la
//! page ne fait qu'appeler : `canAlterPermission` (`:315-323`) et
//! `hasPermission(Rank, Action)` (`:639-650`).
//!
//! **Ce que le port peut tenir, et ce qu'il ne peut pas.** Les rangs du port
//! sont les cinq de [`Rank::ALL`] et leurs bits sont **calcules** par
//! [`Rank::mask`], la ou Java les stocke dans un `Rank` mutable et en cree
//! d'autres (`Permissions.java:344-348`, `:1054-1076`). La matrice des
//! permissions se lit donc, et ne s'ecrit pas ([`Refusal::RankBitsDerived`]), et
//! un rang ne se supprime pas ([`Refusal::StockRankInitial`]).

use crate::colony::permissions::{Action as PermAction, Members, Rank};

use super::refusal::Refusal;

/// Les trois vues du `switch`, dans l'ordre de l'XML
/// (`layoutpermissions.xml:5`, `:54`, `:95`).
pub const VIEWS: [&str; 3] = ["pageAddPlayer", "managePermissions", "freeBlockList"];

/// Le rang selectionne au depart : l'officier (`WindowPermissionsPage.java:117`).
pub const INITIAL_RANK: Rank = Rank::Officer;

/// Les trois etiquettes du menu « type de rang » (`:113-115`).
pub const TYPE_KEYS: [&str; 3] = [
    "com.minecolonies.coremod.gui.townhall.ranktype.colonymanager",
    "com.minecolonies.coremod.gui.townhall.ranktype.hostile",
    "com.minecolonies.coremod.gui.townhall.ranktype.none",
];

/// L'etat du bloc « rangs » : quel rang on edite.
///
/// Java part de l'officier et eteint le bouton de suppression
/// (`:117-118`) ; un clic sur un rang le selectionne et rallume le bouton sauf
/// pour un rang initial (`:416`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ranks {
    selected: Rank,
}

impl Ranks {
    /// L'etat initial.
    pub fn new() -> Ranks {
        Ranks { selected: INITIAL_RANK }
    }

    /// Le rang que la liste des boutons propose : **cinq**, proprietaire
    /// compris (`:358`).
    pub fn list() -> &'static [Rank] {
        &Rank::ALL
    }

    /// Le rang que le menu deroulant d'un joueur propose : **quatre**, le
    /// proprietaire en moins (`:349-356`). Deux listes, deux longueurs, dans la
    /// meme fenetre.
    pub fn pickable() -> [Rank; 4] {
        [Rank::Officer, Rank::Friend, Rank::Neutral, Rank::Hostile]
    }

    /// Le rang en cours d'edition.
    pub fn selected(&self) -> Rank {
        self.selected
    }

    /// Le clic sur un bouton de rang (`:408-420`).
    pub fn select(&mut self, rank: Rank) {
        self.selected = rank;
    }

    /// Le bouton `removeRank` peut-il servir ? `!isInitial()`, et les cinq rangs
    /// du port sont tous initiaux (`Permissions.java:171`).
    pub fn may_remove(&self) -> bool {
        false
    }

    /// Le clic sur `removeRank` (`:271-280`).
    ///
    /// La branche que Java prend ici est inatteignable dans ce port : elle
    /// suppose un rang non initial, et les cinq sont initiaux.
    pub fn remove(&mut self) -> Result<(), Refusal> {
        Err(Refusal::StockRankInitial)
    }

    /// L'indice du menu « type de rang » pour ce rang (`:341`, `:418`).
    pub fn type_index(&self) -> usize {
        type_index(self.selected)
    }
}

impl Default for Ranks {
    fn default() -> Ranks {
        Ranks::new()
    }
}

/// L'indice du type d'un rang : `isColonyManager()` puis `isHostile()`, sinon
/// « aucun » (`WindowPermissionsPage.java:341`).
pub fn type_index(rank: Rank) -> usize {
    if rank.is_manager() {
        0
    } else if rank.is_hostile() {
        1
    } else {
        2
    }
}

/// L'etiquette du type d'un indice.
pub fn type_key(index: usize) -> Option<&'static str> {
    TYPE_KEYS.get(index).copied()
}

/// Le bit d'une action est-il arme pour ce rang ? `hasPermission(rank, action)`
/// (`Permissions.java:639-650`), qui est [`Rank::allows`] dans ce port.
pub fn on_off(rank: Rank, action: PermAction) -> bool {
    rank.allows(action)
}

/// L'etiquette du bouton d'une action (`WindowPermissionsPage.java:622-623`).
pub fn on_off_key(on: bool) -> &'static str {
    if on {
        "com.minecolonies.coremod.gui.workerhuts.retrieveon"
    } else {
        "com.minecolonies.coremod.gui.workerhuts.retrieveoff"
    }
}

/// Le joueur peut-il changer ce bit ? `canAlterPermission(actor, rank, action)`,
/// recopie tel quel (`Permissions.java:315-323`).
///
/// Trois refus, dans l'ordre de Java : le proprietaire n'est change que par le
/// proprietaire ; il faut `EDIT_PERMISSIONS` ; et chacun ne touche pas a ses
/// propres bits `EDIT_PERMISSIONS`, `MANAGE_HUTS` ni `ACCESS_HUTS`. La
/// parenthese de Java est `a || (b && c && d)`, pas `(a || b) && c`.
pub fn can_alter(actor: Rank, target: Rank, action: PermAction) -> bool {
    if target == Rank::Owner && actor != Rank::Owner {
        return false;
    }
    actor.allows(PermAction::EditPermissions)
        && (actor != target
            || (action != PermAction::EditPermissions
                && action != PermAction::ManageHuts
                && action != PermAction::AccessHuts))
}

/// Ajouter un joueur : le bit `EDIT_PERMISSIONS` du joueur, et l'infobulle de la
/// page quand il manque (`:314-326`).
pub fn may_add_player(members: &Members, actor: u128) -> Result<(), Refusal> {
    members
        .may(actor, PermAction::EditPermissions)
        .map_err(|_| Refusal::MayNotAddPlayer)
}

/// Ajouter un rang : le meme bit, une autre phrase (`:329-331`).
pub fn may_add_rank(members: &Members, actor: u128) -> Result<(), Refusal> {
    members
        .may(actor, PermAction::EditPermissions)
        .map_err(|_| Refusal::MayNotAddRank)
}

/// Valider un nom de rang (`:248-262`).
///
/// Java compare aux rangs de `rankList`, qui **exclut le proprietaire** : un
/// rang nomme `Owner` passe donc la validation. Le port garde le trou et le
/// nomme ; il ne le comble pas, parce que ce n'est pas sa regle.
pub fn rank_name(name: &str, pickable: &[&str]) -> Result<(), Refusal> {
    if name.is_empty() {
        return Err(Refusal::RankNameEmpty);
    }
    if pickable.iter().any(|existing| *existing == name) {
        return Err(Refusal::RankNameTaken);
    }
    Ok(())
}

/// Retirer un joueur : le proprietaire ne part pas (`:736-743`).
pub fn may_remove_player(rank: Rank) -> Result<(), Refusal> {
    if rank == Rank::Owner {
        return Err(Refusal::OwnerRankImmutable);
    }
    Ok(())
}

/// Le tourne-page du squelette : quel enfant du `switch` est montre.
///
/// **L'etat initial n'est pas un etat dessine.** `setPage` n'est jamais appele a
/// la construction, donc le compteur `pageNum` garde le `label=""` de l'XML
/// (`layoutpermissions.xml:113`) jusqu'au premier clic
/// (`AbstractWindowSkeleton.java:247-250`), et les deux fleches sont allumees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pages {
    current: usize,
    turned: bool,
}

impl Pages {
    /// La premiere vue, compteur vide.
    pub fn new() -> Pages {
        Pages { current: 0, turned: false }
    }

    /// L'id de la vue montree.
    pub fn view(&self) -> &'static str {
        VIEWS[self.current]
    }

    /// L'indice de la vue montree.
    pub fn current(&self) -> usize {
        self.current
    }

    /// La fleche `nextPage` est-elle eteinte (`:241-244`) ?
    pub fn next_disabled(&self) -> bool {
        self.current + 1 >= VIEWS.len()
    }

    /// La fleche `prevPage` est-elle eteinte (`:237-240`) ?
    pub fn prev_disabled(&self) -> bool {
        self.current == 0
    }

    /// Le compteur, `n/total` (`:249`).
    ///
    /// `None` avant le premier clic : c'est le `label=""` de l'XML, pas une
    /// page 1 qu'on aurait oublie de dessiner.
    pub fn label(&self) -> Option<String> {
        if !self.turned {
            return None;
        }
        Some(format!("{}/{}", self.current + 1, VIEWS.len()))
    }

    /// La page suivante ; le `switch` borne, il ne boucle pas
    /// (`SwitchView.java:179-180`, `endless` n'est pas pose par l'XML).
    pub fn next(&mut self) -> Result<(), Refusal> {
        if self.next_disabled() {
            return Err(Refusal::LastPage);
        }
        self.current += 1;
        self.turned = true;
        Ok(())
    }

    /// La page precedente.
    pub fn prev(&mut self) -> Result<(), Refusal> {
        if self.prev_disabled() {
            return Err(Refusal::FirstPage);
        }
        self.current -= 1;
        self.turned = true;
        Ok(())
    }
}

impl Default for Pages {
    fn default() -> Pages {
        Pages::new()
    }
}

/// Une entree de la liste « blocs libres » : un bloc, ou une position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java's two lists, drawn as one
pub enum FreeEntry<'a> {
    Block(&'a str),
    Position([i32; 3]),
}

/// L'entree de cette ligne : les blocs d'abord, puis les positions
/// (`WindowPermissionsPage.java:507-524`).
pub fn free_at<'a>(
    blocks: &[&'a str],
    positions: &[[i32; 3]],
    row: usize,
) -> Result<FreeEntry<'a>, Refusal> {
    if row < blocks.len() {
        return Ok(FreeEntry::Block(blocks[row]));
    }
    let index = row - blocks.len();
    if index < positions.len() {
        return Ok(FreeEntry::Position(positions[index]));
    }
    Err(Refusal::UnknownFreeEntry { row })
}

/// Ce que le champ « ajouter » contient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two shapes `addBlock` tries, in this order
pub enum FreeKind {
    /// Un identifiant de bloc, `namespace:path` ou un chemin nu.
    Block,
    /// Trois coordonnees.
    Position([i32; 3]),
}

/// Trois entiers separes par des espaces (`BlockPosUtil.getBlockPosOfString`).
pub fn parse_position(text: &str) -> Option<[i32; 3]> {
    let mut parts = text.split_whitespace();
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;
    let z = parts.next()?.parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// Un identifiant de bloc a la forme d'un `ResourceLocation` : pas d'espace, et
/// les caracteres que Java accepte.
///
/// **Le registre n'est pas ici** : Java resout l'identifiant et ecarte l'air
/// (`WindowPermissionsPage.java:542-548`). Le port ne peut donc pas dire si le
/// bloc existe, seulement si le texte en a la forme (`MANQUANT.md` §4).
pub fn looks_like_block(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || "_.:/-".contains(ch))
}

/// Ce que le champ ajoute, ou le refus nomme (`:540-561`).
///
/// Java tente les deux lectures l'une apres l'autre et n'annonce rien quand
/// aucune ne donne : le port nomme le cas.
pub fn parse_free(text: &str) -> Result<FreeKind, Refusal> {
    if let Some(position) = parse_position(text) {
        return Ok(FreeKind::Position(position));
    }
    if looks_like_block(text) {
        return Ok(FreeKind::Block);
    }
    Err(Refusal::UnknownBlockOrPosition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rank_block_starts_on_the_officer_and_offers_five_buttons_and_four_choices() {
        let mut ranks = Ranks::new();
        assert_eq!(ranks.selected(), Rank::Officer, "WindowPermissionsPage.java:117");
        assert_eq!(Ranks::list().len(), 5, "allRankList holds the owner too");
        assert_eq!(Ranks::pickable().len(), 4, "rankList leaves the owner out");
        assert!(!Ranks::pickable().contains(&Rank::Owner));
        assert!(Ranks::list().contains(&Rank::Owner));
        // The remove button is off for all five, and the click says why.
        assert!(!ranks.may_remove());
        assert_eq!(ranks.remove(), Err(Refusal::StockRankInitial));
        ranks.select(Rank::Owner);
        assert_eq!(ranks.selected(), Rank::Owner, "the owner button is selectable");
        assert_eq!(ranks.remove(), Err(Refusal::StockRankInitial));
    }

    #[test]
    fn the_rank_type_is_manager_first_then_hostile_then_none() {
        assert_eq!(type_index(Rank::Owner), 0);
        assert_eq!(type_index(Rank::Officer), 0);
        assert_eq!(type_index(Rank::Friend), 2);
        assert_eq!(type_index(Rank::Neutral), 2);
        assert_eq!(type_index(Rank::Hostile), 1);
        assert_eq!(type_key(0), Some(TYPE_KEYS[0]));
        assert_eq!(type_key(1).map(|key| key.ends_with("hostile")), Some(true));
        assert_eq!(type_key(3), None);
        let mut ranks = Ranks::new();
        assert_eq!(ranks.type_index(), 0, "the officer is a colony manager");
        ranks.select(Rank::Hostile);
        assert_eq!(ranks.type_index(), 1);
    }

    #[test]
    fn only_the_owner_touches_his_own_three_bits_and_only_he_touches_his_rank() {
        // The owner may change his own bits, except three.
        assert!(can_alter(Rank::Owner, Rank::Owner, PermAction::MapBorder));
        assert!(!can_alter(Rank::Owner, Rank::Owner, PermAction::EditPermissions));
        assert!(!can_alter(Rank::Owner, Rank::Owner, PermAction::ManageHuts));
        assert!(!can_alter(Rank::Owner, Rank::Owner, PermAction::AccessHuts));
        // The owner may touch another rank's bit.
        assert!(can_alter(Rank::Owner, Rank::Friend, PermAction::ManageHuts));
        // An officer may **not**: `EDIT_PERMISSIONS` is the owner's own bit, and
        // it is not in `officer_bits` (Permissions.java:175-180), so it does not
        // fall through to him either.
        assert!(!can_alter(Rank::Officer, Rank::Friend, PermAction::ManageHuts));
        assert!(!can_alter(Rank::Officer, Rank::Owner, PermAction::MapBorder));
        // Nobody without EDIT_PERMISSIONS touches anything.
        assert!(!can_alter(Rank::Friend, Rank::Neutral, PermAction::AccessHuts));
        assert!(!can_alter(Rank::Neutral, Rank::Neutral, PermAction::AccessHuts));
    }

    #[test]
    fn the_matrix_reads_the_masks_and_the_button_carries_two_keys() {
        assert!(on_off(Rank::Officer, PermAction::ManageHuts));
        assert!(!on_off(Rank::Friend, PermAction::ManageHuts));
        assert!(on_off(Rank::Friend, PermAction::AccessHuts));
        assert!(on_off(Rank::Hostile, PermAction::HurtCitizen));
        assert!(!on_off(Rank::Hostile, PermAction::PlaceHuts));
        // The owner reaches the officer's bits through the fall-through mask.
        assert!(on_off(Rank::Owner, PermAction::ManageHuts));
        assert_ne!(on_off_key(true), on_off_key(false));
        assert!(on_off_key(true).ends_with("retrieveon"));
    }

    fn colony() -> Members {
        let mut members = Members::with_owner(1);
        members.set(2, Rank::Officer);
        members.set(3, Rank::Neutral);
        members
    }

    #[test]
    fn adding_a_player_or_a_rank_needs_the_same_bit_and_says_two_different_things() {
        let members = colony();
        assert_eq!(may_add_player(&members, 1), Ok(()));
        assert_eq!(may_add_rank(&members, 1), Ok(()));
        // An officer is not enough: `EDIT_PERMISSIONS` is the owner's bit alone
        // (Permissions.java:175-180), and the page only tests that one bit.
        assert_eq!(may_add_rank(&members, 2), Err(Refusal::MayNotAddRank));
        assert_eq!(may_add_player(&members, 3), Err(Refusal::MayNotAddPlayer));
        assert_eq!(may_add_rank(&members, 3), Err(Refusal::MayNotAddRank));
        // A stranger is neutral, and the same refusal comes out.
        assert_eq!(may_add_player(&members, 99), Err(Refusal::MayNotAddPlayer));
    }

    #[test]
    fn a_rank_name_is_checked_against_the_four_choiceable_ranks_and_owner_slips_through() {
        let pickable = ["Officer", "Friend", "Neutral", "Hostile"];
        assert_eq!(rank_name("", &pickable), Err(Refusal::RankNameEmpty));
        assert_eq!(rank_name("Officer", &pickable), Err(Refusal::RankNameTaken));
        assert_eq!(rank_name("officer", &pickable), Ok(()), "Java compares with equals");
        assert_eq!(rank_name("Miner", &pickable), Ok(()));
        // The hole: the owner is not in the list Java validates against.
        assert_eq!(rank_name("Owner", &pickable), Ok(()));
    }

    #[test]
    fn the_owner_is_the_one_player_that_cannot_be_removed() {
        assert_eq!(may_remove_player(Rank::Owner), Err(Refusal::OwnerRankImmutable));
        for rank in [Rank::Officer, Rank::Friend, Rank::Neutral, Rank::Hostile] {
            assert_eq!(may_remove_player(rank), Ok(()));
        }
    }

    #[test]
    fn the_page_counter_is_blank_until_the_first_click_and_stops_at_the_ends() {
        let mut pages = Pages::new();
        assert_eq!(pages.view(), "pageAddPlayer");
        assert_eq!(pages.label(), None, "label=\"\" in the XML, AbstractWindowSkeleton.java:249");
        assert!(pages.prev_disabled());
        assert!(pages.label().is_none());
        assert_eq!(pages.next(), Ok(()));
        assert_eq!(pages.view(), "managePermissions");
        assert_eq!(pages.label().as_deref(), Some("2/3"));
        assert_eq!(pages.next(), Ok(()));
        assert_eq!(pages.view(), "freeBlockList");
        assert!(pages.next_disabled());
        assert_eq!(pages.next(), Err(Refusal::LastPage));
        assert_eq!(pages.prev(), Ok(()));
        assert_eq!(pages.prev(), Ok(()));
        assert!(pages.prev_disabled());
        assert_eq!(pages.prev(), Err(Refusal::FirstPage));
        assert_eq!(pages, Pages { current: 0, turned: true }, "the first click lights the counter");
    }

    #[test]
    fn the_free_list_puts_the_blocks_first_and_the_field_takes_two_shapes() {
        let blocks = ["minecraft:stone", "minecraft:oak_door"];
        let positions = [[10, 64, 10]];
        assert_eq!(free_at(&blocks, &positions, 0), Ok(FreeEntry::Block("minecraft:stone")));
        assert_eq!(free_at(&blocks, &positions, 2), Ok(FreeEntry::Position([10, 64, 10])));
        assert_eq!(free_at(&blocks, &positions, 3), Err(Refusal::UnknownFreeEntry { row: 3 }));
        assert_eq!(parse_free("10 64 10"), Ok(FreeKind::Position([10, 64, 10])));
        assert_eq!(parse_free("minecraft:stone"), Ok(FreeKind::Block));
        assert_eq!(parse_free("stone"), Ok(FreeKind::Block), "a bare path is namespaced by Java");
        assert_eq!(parse_free(""), Err(Refusal::UnknownBlockOrPosition));
        assert_eq!(parse_free("Not A Block"), Err(Refusal::UnknownBlockOrPosition));
        assert_eq!(parse_position("1 2"), None);
        assert_eq!(parse_position("1 2 3 4"), None);
        assert_eq!(parse_position("-1 0 3"), Some([-1, 0, 3]));
    }
}
