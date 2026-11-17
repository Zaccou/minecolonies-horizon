//! L'étagère d'entrepôt : ce qu'elle contient, sa taille, et la variante double.
//!
//! Référence Java : `core/tileentities/TileEntityRack.java` (790 lignes) sur
//! `api/tileentities/AbstractTileEntityRack.java` (400 lignes), plus l'énumération
//! `api/blocks/types/RackType.java` (5 variantes).
//!
//! **Le contenu est un cache, pas la vérité.** Java relit ses 27 slots à chaque
//! changement et en tire une table `ItemStorage → compte` plus un nombre de
//! slots libres (`updateContent`, `:341-362`) ; toutes les questions rapides
//! (compte d'un objet, place libre, étagère vide) passent par cette table. Le
//! port garde la même séparation, mais la **source** est le conteneur du block
//! entity — Horizon tient les slots (`Registrar::block_container`, `src/hut.rs`)
//! là où Java les tient dans l'objet. [`Content::from_slots`] remplace donc
//! `updateContent`, et la liste `inventory` du compound Java n'est pas relue :
//! voir `MANQUANT.md`.
//!
//! **L'identité d'un objet est son nom.** Java compare un `ItemStorage` avec ses
//! drapeaux `ignoreDamageValue` / `ignoreNBT` (`AbstractTileEntityRack.java:203-215`) ;
//! le modèle d'objet du port n'a ni dégât ni composant (`src/inventory/stack.rs:97-101`),
//! donc les trois variantes de `getCount` répondent la même chose, et c'est un
//! fait de ce port, pas un oubli : `hasSimilarStack` (l'onglet créatif de
//! `:240-264`) reste sans équivalent, `MANQUANT.md` dit pourquoi.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;
use crate::inventory::{ItemName, Stack};

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:29`).
pub const TYPE_ID: &str = "minecolonies:rack";

/// La taille d'une étagère simple (`api/util/constant/Constants.java:143`).
pub const DEFAULT_SIZE: usize = 27;
/// Les slots qu'un agrandissement ajoute, et le pas entre deux tailles
/// (`Constants.java:163`).
pub const SLOT_PER_LINE: usize = 9;
/// La version du format (`TileEntityRack.java:66`), écrite et relue
/// (`:428`, `:461`). Une étagère d'une version antérieure est marquée à la
/// première lecture du contenu (`:493-497`).
pub const VERSION: i8 = 2;

/// La taille, **sous le nom que Java lui donne** : `tagSIze`, avec son `S`
/// majuscule (`NbtTagConstants.java:443`). La faute est recopiée telle quelle :
/// la corriger rendrait illisible une étagère sauvée par MineColonies.
pub const KEY_SIZE: &str = "tagSIze";
pub const KEY_IN_WAREHOUSE: &str = "inWarehouse";
/// La position du bâtiment propriétaire (`NbtTagConstants.java:51`, écrit à
/// `:460`).
pub const KEY_POS: &str = "pos";
pub const KEY_VERSION: &str = "version";

/// La variante du bloc (`RackType.java:14-18`), dans l'ordre de l'énumération :
/// `getInvBasedVariant` en dépend (`:57-75`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les cinq variantes de RackType
pub enum RackType {
    Empty,
    Full,
    EmptyDouble,
    FullDouble,
    NoRender,
}

impl RackType {
    pub const ALL: [RackType; 5] = [
        RackType::Empty,
        RackType::Full,
        RackType::EmptyDouble,
        RackType::FullDouble,
        RackType::NoRender,
    ];

    /// Une étagère double partage son contenu avec sa voisine (`:22-25`).
    pub fn is_double(self) -> bool {
        matches!(
            self,
            RackType::EmptyDouble | RackType::FullDouble | RackType::NoRender
        )
    }

    /// La variante qui correspond au contenu, quand il vient de changer
    /// (`getInvBasedVariant`, `:57-75`) : une simple reste simple, une double
    /// reste double, et le rendu masqué le reste.
    pub fn inv_based_variant(self, empty: bool) -> RackType {
        match self {
            RackType::Empty | RackType::Full => {
                if empty {
                    RackType::Empty
                } else {
                    RackType::Full
                }
            }
            RackType::EmptyDouble | RackType::FullDouble => {
                if empty {
                    RackType::EmptyDouble
                } else {
                    RackType::FullDouble
                }
            }
            RackType::NoRender => RackType::NoRender,
        }
    }

    /// Le nom que Java écrit dans l'état du bloc (`getName`, `:36-39`).
    pub fn name(self) -> &'static str {
        match self {
            RackType::Empty => "blockrackemptysingle",
            RackType::Full => "blockrackfullsingle",
            RackType::EmptyDouble => "blockrackempty",
            RackType::FullDouble => "blockrackfull",
            RackType::NoRender => "blockrackair",
        }
    }
}

/// Le contenu agrégé d'une étagère : un compte par objet, et les slots libres.
///
/// L'ordre est celui de `BTreeMap` — c'est-à-dire l'ordre des noms —, ce qui rend
/// deux lectures comparables et un test déterministe. Java n'a pas d'ordre : il
/// ne lit sa table que par clé.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : une table et un compte, rien d'autre
pub struct Content {
    counts: BTreeMap<ItemName, u32>,
    free_slots: usize,
}

impl Content {
    /// Relit les slots du conteneur (`updateContent`, `:341-362`) : un slot vide
    /// est une place libre, deux stacks du même objet se cumulent, et un slot
    /// occupé ne l'est pas.
    pub fn from_slots(slots: &[Option<Stack>]) -> Content {
        let mut counts: BTreeMap<ItemName, u32> = BTreeMap::new();
        let mut free_slots = 0;
        for slot in slots {
            match slot {
                None => free_slots += 1,
                Some(stack) => {
                    let entry = counts.entry(stack.item().clone()).or_insert(0);
                    *entry = entry.saturating_add(stack.count());
                }
            }
        }
        Content {
            counts,
            free_slots,
        }
    }

    pub fn counts(&self) -> &BTreeMap<ItemName, u32> {
        &self.counts
    }

    /// Combien d'exemplaires de cet objet (`getCount`, `:203-217`).
    pub fn count(&self, item: &ItemName) -> u32 {
        self.counts.get(item).copied().unwrap_or(0)
    }

    /// Le total des objets qui passent un prédicat (`getItemCount`, `:289-300`).
    pub fn item_count(&self, predicate: impl Fn(&ItemName) -> bool) -> u32 {
        self.counts
            .iter()
            .filter(|(item, _)| predicate(item))
            .map(|(_, count)| *count)
            .fold(0u32, u32::saturating_add)
    }

    /// `isEmpty` (`:395-398`) : la table est vide, même si un slot tient zéro
    /// objet — ce cas n'existe pas, un `Stack` porte au moins un objet.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Les slots libres (`getFreeSlots`, `:172`), recomptés à chaque lecture.
    pub fn free_slots(&self) -> usize {
        self.free_slots
    }
}

/// L'état sauvé d'une étagère : sa taille, son rattachement, sa version.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les quatre champs sauvés de la classe Java
pub struct Rack {
    /// Le nombre d'agrandissements, `0` pour une étagère de 27 slots (`:82`).
    size: i32,
    /// Vrai quand l'étagère est dans un entrepôt : tout ajout réveille alors les
    /// requêtes de la colonie (`AbstractTileEntityRack.java:132-146`).
    in_warehouse: bool,
    /// La position du bâtiment propriétaire, `None` pour aucune
    /// (`BlockPos.ZERO` en Java, `:45`).
    building: Option<[i32; 3]>,
    /// La version du format, relue pour être réécrite (`:428`, `:493-497`).
    version: i8,
}

impl Default for Rack {
    fn default() -> Rack {
        Rack {
            size: 0,
            in_warehouse: false,
            building: None,
            version: 0,
        }
    }
}

impl Rack {
    pub fn new() -> Rack {
        Rack::default()
    }

    pub fn from_compound(data: &Compound) -> Rack {
        Rack {
            size: nbt::int(data, KEY_SIZE).unwrap_or(0).max(0),
            in_warehouse: nbt::boolean(data, KEY_IN_WAREHOUSE).unwrap_or(false),
            building: nbt::position_in(data, KEY_POS),
            version: match data.get(&KEY_VERSION.into()) {
                Some(Tag::Byte(value)) => *value,
                _ => 0,
            },
        }
    }

    /// Ce qu'il faut fusionner pour sauver l'état (`:440-462`). La version
    /// écrite est celle de ce port : Java monte une étagère ancienne à la
    /// première lecture (`:493-497`), le port le fait à l'écriture.
    pub fn merge(&self) -> Compound {
        Compound::from([
            (KEY_SIZE.into(), Tag::Int(self.size)),
            (KEY_IN_WAREHOUSE.into(), nbt::boolean_tag(self.in_warehouse)),
            (KEY_POS.into(), crate::hut::position(self.building.unwrap_or([0, 0, 0]))),
            (KEY_VERSION.into(), Tag::Byte(VERSION)),
        ])
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    /// La version lue. `0` pour une étagère qui n'en portait pas.
    pub fn version(&self) -> i8 {
        self.version
    }

    pub fn is_in_warehouse(&self) -> bool {
        self.in_warehouse
    }

    /// `setInWarehouse` (`:165-168`), posé par l'entrepôt lui-même quand il est
    /// construit (`TileEntityWareHouse.java:39`).
    pub fn set_in_warehouse(&mut self, in_warehouse: bool) {
        self.in_warehouse = in_warehouse;
    }

    pub fn building(&self) -> Option<[i32; 3]> {
        self.building
    }

    /// `setBuildingPos` (`AbstractTileEntityRack.java:249-255`) : la position du
    /// bâtiment dont cette étagère compte le stock.
    pub fn set_building(&mut self, at: [i32; 3]) {
        self.building = Some(at);
    }

    /// Le nombre de slots de cette étagère : `DEFAULT_SIZE + size × SLOT_PER_LINE`
    /// (`:273`, `:407`).
    pub fn slots(&self) -> usize {
        DEFAULT_SIZE + (self.size.max(0) as usize) * SLOT_PER_LINE
    }

    /// `upgradeRackSize` (`:270-287`) : un cran de plus, donc neuf slots, et
    /// l'inventaire recopié dans le nouveau. Le port rend la nouvelle taille,
    /// que l'appelant donne au conteneur : les objets restent dans les slots,
    /// qui ne bougent pas d'indice.
    pub fn upgrade(&mut self) -> usize {
        self.size = self.size.saturating_add(1);
        self.slots()
    }

    /// La position de l'étagère voisine d'une étagère double (`getOtherChest`,
    /// `:366-389`) : elle est devant, dans le sens du bloc, et ce n'est une
    /// voisine que si c'est une **étagère simple** — une block entity de
    /// bâtiment est exclue explicitement (`:381`).
    ///
    /// Rend `None` pour une variante simple (`:373-376`), et pour un voisin qui
    /// n'est pas une étagère nue.
    pub fn other_chest(
        variant: RackType,
        at: [i32; 3],
        facing: [i32; 2],
        neighbour_is_plain_rack: bool,
    ) -> Option<[i32; 3]> {
        if !variant.is_double() || !neighbour_is_plain_rack {
            return None;
        }
        Some([at[0] + facing[0], at[1], at[2] + facing[1]])
    }

    /// Ce qu'un changement de contenu réveille (`updateWarehouseIfAvailable`,
    /// `AbstractTileEntityRack.java:122-160`).
    ///
    /// Trois conditions, dans l'ordre de Java : il faut être dans une colonie
    /// (`:126-130`), et être soit dans l'entrepôt soit rattaché à un bâtiment
    /// (`:132`) ; ensuite l'entrepôt réveille le gestionnaire de requêtes, un
    /// bâtiment réveille sa propre file (`:138-157`).
    pub fn notification(&self, in_colony: bool) -> Option<Notify> {
        if !in_colony || (!self.in_warehouse && self.building.is_none()) {
            return None;
        }
        if self.in_warehouse {
            Some(Notify::RequestManager)
        } else {
            Some(Notify::Building)
        }
    }
}

/// Ce qu'un ajout dans une étagère réveille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les deux branches de `updateWarehouseIfAvailable`
pub enum Notify {
    /// `colony.getRequestManager().onColonyUpdate(...)`
    /// (`AbstractTileEntityRack.java:139-146`).
    RequestManager,
    /// `building.overruleNextOpenRequestWithStack(stack)` (`:153-157`).
    Building,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("nom d'objet valide")
    }

    fn stack(name: &str, count: u32) -> Option<Stack> {
        Stack::new(item(name), count).ok()
    }

    #[test]
    fn une_etagere_simple_a_27_slots_et_un_cran_ajoute_neuf_slots() {
        let mut rack = Rack::new();
        assert_eq!(rack.slots(), 27);
        assert_eq!(rack.upgrade(), 36);
        assert_eq!(rack.upgrade(), 45);
        assert_eq!(rack.size(), 2);
        // La taille d'un compound se relit dans la même unité.
        assert_eq!(Rack::from_compound(&rack.merge()).slots(), 45);
    }

    #[test]
    fn le_contenu_cumule_les_doublons_et_compte_les_slots_libres() {
        let slots = vec![
            stack("minecraft:oak_planks", 32),
            None,
            stack("minecraft:oak_planks", 16),
            stack("minecraft:stone", 5),
            None,
        ];
        let content = Content::from_slots(&slots);
        assert_eq!(content.count(&item("minecraft:oak_planks")), 48);
        assert_eq!(content.count(&item("minecraft:stone")), 5);
        assert_eq!(content.count(&item("minecraft:dirt")), 0);
        assert_eq!(content.free_slots(), 2);
        assert!(!content.is_empty());
        assert_eq!(content.counts().len(), 2, "deux objets, pas trois slots");
    }

    #[test]
    fn un_contenu_se_compte_par_predicat() {
        let slots = vec![stack("minecraft:oak_log", 4), stack("minecraft:oak_planks", 6)];
        let content = Content::from_slots(&slots);
        let wood = content.item_count(|name| name.as_str().starts_with("minecraft:oak"));
        assert_eq!(wood, 10);
        assert_eq!(content.item_count(|_| false), 0);
        assert!(Content::from_slots(&[None, None]).is_empty());
    }

    #[test]
    fn la_variante_suit_le_contenu_et_ne_change_pas_de_famille() {
        assert_eq!(RackType::Empty.inv_based_variant(false), RackType::Full);
        assert_eq!(RackType::Full.inv_based_variant(true), RackType::Empty);
        assert_eq!(
            RackType::EmptyDouble.inv_based_variant(false),
            RackType::FullDouble
        );
        assert_eq!(
            RackType::FullDouble.inv_based_variant(true),
            RackType::EmptyDouble
        );
        assert_eq!(RackType::NoRender.inv_based_variant(true), RackType::NoRender);
        // Deux variantes doubles, deux simples, une masquée, et cinq noms uniques.
        assert_eq!(RackType::ALL.iter().filter(|k| k.is_double()).count(), 3);
        let mut names = std::collections::BTreeSet::new();
        for kind in RackType::ALL {
            assert!(names.insert(kind.name()));
        }
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn la_voisine_d_une_etagere_double_est_devant_et_doit_etre_une_etagere_nue() {
        let at = [10, 64, 10];
        assert_eq!(
            Rack::other_chest(RackType::Full, at, [0, -1], true),
            None,
            "une simple n'a pas de voisine (`:373-376`)"
        );
        assert_eq!(
            Rack::other_chest(RackType::FullDouble, at, [0, -1], true),
            Some([10, 64, 9])
        );
        assert_eq!(
            Rack::other_chest(RackType::NoRender, at, [1, 0], true),
            Some([11, 64, 10])
        );
        assert_eq!(
            Rack::other_chest(RackType::EmptyDouble, at, [0, -1], false),
            None,
            "une block entity de bâtiment n'est pas une voisine (`:381`)"
        );
    }

    #[test]
    fn l_etat_sauve_fait_un_aller_retour_avec_la_cle_telle_quelle() {
        let mut rack = Rack::new();
        rack.upgrade();
        rack.set_in_warehouse(true);
        rack.set_building([3, 70, -8]);
        let merge = rack.merge();
        assert!(merge.contains_key(&KEY_SIZE.into()));
        assert_eq!(KEY_SIZE, "tagSIze", "la faute de NbtTagConstants.java:443");
        let read = Rack::from_compound(&merge);
        assert_eq!(read.slots(), 36);
        assert!(read.is_in_warehouse());
        assert_eq!(read.building(), Some([3, 70, -8]));
        assert_eq!(read.version(), VERSION, "le port écrit la version courante");
    }

    #[test]
    fn une_etagere_ancienne_se_lit_et_sa_version_revient_a_la_courante() {
        let old = Compound::from([(KEY_VERSION.into(), Tag::Byte(0))]);
        let rack = Rack::from_compound(&old);
        assert_eq!(rack.version(), 0);
        assert_eq!(rack.slots(), DEFAULT_SIZE);
        assert_eq!(rack.building(), None);
        assert_eq!(rack.merge().get(&KEY_VERSION.into()), Some(&Tag::Byte(VERSION)));
    }

    #[test]
    fn un_ajout_ne_reveille_que_dans_une_colonie_et_si_quelque_chose_attend() {
        let mut rack = Rack::new();
        assert_eq!(rack.notification(true), None, "ni entrepôt ni bâtiment");
        assert_eq!(rack.notification(false), None);
        rack.set_in_warehouse(true);
        assert_eq!(rack.notification(false), None, "hors colonie (`:128`)");
        assert_eq!(rack.notification(true), Some(Notify::RequestManager));
        rack.set_in_warehouse(false);
        rack.set_building([1, 2, 3]);
        assert_eq!(rack.notification(true), Some(Notify::Building));
    }
}
