//! Les block entities de MineColonies : ce que chacune détient, ce qu'elle
//! recalcule, et qui a le droit de la toucher.
//!
//! Référence Java : `core/tileentities/` — **14 classes** `TileEntity*` plus les
//! sept classes abstraites de `api/tileentities/` qui portent la moitié des
//! champs (`AbstractTileEntityRack`, `AbstractTileEntityColonyBuilding`,
//! `AbstractTileEntityGrave`, `AbstractTileEntityNamedGrave`,
//! `AbstractTileEntityPlantationField`, `AbstractTileEntityScarecrow`,
//! `AbstractTileEntityWareHouse`). Le fichier `TileEntityInitializer.java:15-47`
//! est la table des identifiants de type, recopiée dans [`Kind`].
//!
//! | Fichier | Classe Java | Question |
//! | --- | --- | --- |
//! | `barrel` | `TileEntityBarrel` | le compost : quand il mûrit, ce qu'il prend |
//! | `building` | `TileEntityColonyBuilding` | le rattachement à la colonie, et l'accès |
//! | `colony_flag` | `TileEntityColonyFlag` | les motifs de bannière de la colonie |
//! | `colony_sign` | `TileEntityColonySign` | la chaîne de panneaux entre deux colonies |
//! | `composted_dirt` | `TileEntityCompostedDirt` | la terre compostée qui fait pousser |
//! | `decoration` | `TileEntityDecorationController` | le schéma décoratif, ses coins |
//! | `enchanter` | `TileEntityEnchanter` | l'animation du livre (client) |
//! | `grave` | `TileEntityGrave` | la décomposition d'une tombe, et son butin |
//! | `named_grave` | `TileEntityNamedGrave` | les trois lignes d'une tombe nommée |
//! | `plantation` | `TileEntityPlantationField` | les positions de travail par tag |
//! | `rack` | `TileEntityRack` | l'étagère : contenu, taille, variante double |
//! | `scarecrow` | `TileEntityScarecrow` | la taille du champ, par direction |
//! | `stash` | `TileEntityStash` | la cache qui réclame un ramassage |
//! | `warehouse` | `TileEntityWareHouse` | où ranger : même objet, similaire, plus vide |
//!
//! **Le tick passe par une seule boucle.** Le manifeste déclare
//! `[[native.loop]] block_entities.tick` (`horizon-mod.toml:19-24`), dont le slot
//! est `src/loops/hut_tick.rs` : il compte les ticks que le jeu donne à un block
//! entity de mod et les transmet par `crate::hut::note_mod_tick`. Aucun fichier
//! de ce module n'ouvre de seconde boucle : chaque tick est une **fonction pure**
//! `avance(&mut état, ticks)` que le slot peut appeler sans rien lui-même
//! décider. Le slot d'aujourd'hui ne transmet qu'un compteur, pas la position
//! touchée : `MANQUANT.md` dit ce qu'il faudrait pour router tick par tick.
//!
//! **Trois choix de portage, valables pour tout le module.**
//!
//! 1. **Les données restent dans le compound sauvegardé**, jamais dans un
//!    `BlockEntityPartState` : un mod wasm ne peut pas déclarer d'état sans
//!    codec (`docs/modding/src/concepts/parts/block-entity-types.md:95-97`), et
//!    `crate::hut` a déjà fait ce choix pour la hutte.
//! 2. **Les clés NBT sont celles de Java, fautes de frappe comprises** :
//!    `tagSIze` (`NbtTagConstants.java:443`) est écrit tel quel, sinon une
//!    sauvegarde Java ne se relit plus.
//! 3. **Aucun refus n'est un `false`** : chaque règle rend un `Result` dont
//!    l'erreur nomme ce qui a bloqué, comme `colony::Refusal` et
//!    `inventory::Refusal` avant elle.

mod nbt;
pub mod barrel;
pub mod building;
pub mod colony_flag;
pub mod colony_sign;
pub mod composted_dirt;
pub mod decoration;
pub mod enchanter;
pub mod grave;
pub mod named_grave;
pub mod plantation;
pub mod rack;
pub mod scarecrow;
pub mod stash;
pub mod warehouse;

pub use barrel::Barrel;
pub use building::{BuildingEntity, Step};
pub use colony_flag::ColonyFlag;
pub use colony_sign::ColonySign;
pub use composted_dirt::CompostedDirt;
pub use decoration::DecorationController;
pub use enchanter::Book;
pub use grave::Grave;
pub use named_grave::NamedGrave;
pub use plantation::{PositionedTags, RotationMirror};
pub use rack::{Content, Rack, RackType};
pub use scarecrow::{FieldSize, ScarecrowType};
pub use stash::Stash;
pub use warehouse::Warehouse;

/// Les ticks d'une seconde (`api/util/constant/Constants.java:21`). Recopié ici
/// parce que c'est l'unité de **tous** les compteurs de ce module, et qu'aucun
/// autre module du port ne l'expose.
pub const TICKS_PER_SECOND: u32 = 20;

/// Un tick de block entity tel que le jeu le compte : le slot natif
/// `block_entities.tick` en donne un par passage, et les règles de ce module
/// n'ont pas besoin de savoir d'où il vient.
///
/// **Pourquoi un `u64` et pas un temps de jeu.** Java compare `getGameTime()` à
/// des modules (`TileEntityBarrel.java:69`, `TileEntityColonyBuilding.java:470`) ;
/// le port reçoit ce temps du slot, qui le tient du jeu, et le passe tel quel.
/// Le type est celui du compteur de `crate::hut::MOD_TICKS`.
pub type TickCount = u64;

/// La table des identifiants de type, recopiée de `TileEntityInitializer.java:17-46`.
///
/// **Un identifiant est complet** : `minecolonies:barrel`, jamais `barrel` seul,
/// pour la raison que `src/hut.rs:22-27` donne pour la hutte — un nom nu est
/// qualifié avec l'identifiant du mod (`ported_minecolonies`), pas avec l'espace
/// de noms que le paquet déclare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustif : les 14 classes Java de `core/tileentities/`, aucune de plus
pub enum Kind {
    Barrel,
    Building,
    ColonyFlag,
    ColonySign,
    CompostedDirt,
    DecorationController,
    Enchanter,
    Grave,
    NamedGrave,
    PlantationField,
    Rack,
    Scarecrow,
    Stash,
    Warehouse,
}

impl Kind {
    /// Les 14, dans l'ordre de la table Java.
    pub const ALL: [Kind; 14] = [
        Kind::Barrel,
        Kind::Building,
        Kind::ColonyFlag,
        Kind::ColonySign,
        Kind::CompostedDirt,
        Kind::DecorationController,
        Kind::Enchanter,
        Kind::Grave,
        Kind::NamedGrave,
        Kind::PlantationField,
        Kind::Rack,
        Kind::Scarecrow,
        Kind::Stash,
        Kind::Warehouse,
    ];

    /// L'identifiant d'enregistrement (`TileEntityInitializer.java:17-46`).
    pub fn type_id(self) -> &'static str {
        match self {
            Kind::Barrel => barrel::TYPE_ID,
            Kind::Building => building::TYPE_ID,
            Kind::ColonyFlag => colony_flag::TYPE_ID,
            Kind::ColonySign => colony_sign::TYPE_ID,
            Kind::CompostedDirt => composted_dirt::TYPE_ID,
            Kind::DecorationController => decoration::TYPE_ID,
            Kind::Enchanter => enchanter::TYPE_ID,
            Kind::Grave => grave::TYPE_ID,
            Kind::NamedGrave => named_grave::TYPE_ID,
            Kind::PlantationField => plantation::TYPE_ID,
            Kind::Rack => rack::TYPE_ID,
            Kind::Scarecrow => scarecrow::TYPE_ID,
            Kind::Stash => stash::TYPE_ID,
            Kind::Warehouse => warehouse::TYPE_ID,
        }
    }

    /// Le fichier de ce module qui porte la classe. Sert de table de lecture, et
    /// de point d'ancrage au test qui vérifie qu'aucune classe n'est orpheline.
    pub fn port_file(self) -> &'static str {
        match self {
            Kind::Barrel => "barrel",
            Kind::Building => "building",
            Kind::ColonyFlag => "colony_flag",
            Kind::ColonySign => "colony_sign",
            Kind::CompostedDirt => "composted_dirt",
            Kind::DecorationController => "decoration",
            Kind::Enchanter => "enchanter",
            Kind::Grave => "grave",
            Kind::NamedGrave => "named_grave",
            Kind::PlantationField => "plantation",
            Kind::Rack => "rack",
            Kind::Scarecrow => "scarecrow",
            Kind::Stash => "stash",
            Kind::Warehouse => "warehouse",
        }
    }

    /// Le type d'un identifiant complet. `None` pour un autre namespace, et pour
    /// un nom que cette table ne connaît pas : une block entity inconnue n'est
    /// pas devinée.
    pub fn from_type_id(id: &str) -> Option<Kind> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.type_id() == id.trim())
    }

    /// Le genre Java de la classe, dit en une phrase. C'est ce que le rapport
    /// oppose à « pas eu le temps » : chaque classe a son fichier, et ceux qui
    /// n'ont pas d'équivalent complet disent ici ce qui leur manque.
    pub fn java_class(self) -> &'static str {
        match self {
            Kind::Barrel => "TileEntityBarrel",
            Kind::Building => "TileEntityColonyBuilding",
            Kind::ColonyFlag => "TileEntityColonyFlag",
            Kind::ColonySign => "TileEntityColonySign",
            Kind::CompostedDirt => "TileEntityCompostedDirt",
            Kind::DecorationController => "TileEntityDecorationController",
            Kind::Enchanter => "TileEntityEnchanter",
            Kind::Grave => "TileEntityGrave",
            Kind::NamedGrave => "TileEntityNamedGrave",
            Kind::PlantationField => "TileEntityPlantationField",
            Kind::Rack => "TileEntityRack",
            Kind::Scarecrow => "TileEntityScarecrow",
            Kind::Stash => "TileEntityStash",
            Kind::Warehouse => "TileEntityWareHouse",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_quatorze_classes_ont_un_identifiant_et_un_fichier() {
        use std::collections::BTreeSet;
        let mut ids = BTreeSet::new();
        let mut files = BTreeSet::new();
        for kind in Kind::ALL {
            assert!(ids.insert(kind.type_id()), "identifiant en double: {kind:?}");
            assert!(
                kind.type_id().starts_with("minecolonies:"),
                "{kind:?} n'est pas qualifié"
            );
            assert!(files.insert(kind.port_file()), "fichier partagé: {kind:?}");
            assert!(kind.java_class().starts_with("TileEntity"));
        }
        assert_eq!(ids.len(), 14);
        assert_eq!(files.len(), 14);
    }

    #[test]
    fn un_identifiant_complet_se_relit_et_un_nom_nu_est_refuse() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_type_id(kind.type_id()), Some(kind));
        }
        assert_eq!(Kind::from_type_id("barrel"), None, "le nom nu n'est pas la clé");
        assert_eq!(Kind::from_type_id("other:barrel"), None);
        assert_eq!(Kind::from_type_id(" minecolonies:rack "), Some(Kind::Rack));
    }
}
