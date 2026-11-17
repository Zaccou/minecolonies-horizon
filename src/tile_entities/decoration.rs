//! Le contrôleur de décoration : le schéma décoratif d'un bloc, ses coins, son
//! orientation.
//!
//! Référence Java : `core/tileentities/TileEntityDecorationController.java` (277
//! lignes). Il implémente les deux interfaces que Structurize définit — le codec
//! de schéma (`IBlueprintDataProviderBE`, `:32-33`) et la rotation
//! (`IRotatableBlockEntity`, `:246-253`) — et ne fait rien d'autre : **aucun
//! tick, aucun inventaire, aucun accès**. C'est un porte-données.
//!
//! **La rotation est calculée une fois puis gardée.** `cachedRotation` vaut
//! `-1` au chargement (`:156`) et après chaque rotation (`:244-247`) ; la
//! première lecture appelle `BuildingUtils.getRotationFromBlueprint`
//! (`:259-266`), qui lit l'orientation du bloc primaire du schéma. Le port garde
//! la même paresse, mais prend le calcul en paramètre : la fonction de Java lit
//! un schéma, ce que le port n'a pas.
//!
//! **Trois clés de compatibilité ascendante.** `load` lit d'abord le
//! sous-compound, puis retombe sur des clés de premier niveau (`:163-181`), et
//! si un nom existe sans chemin il en fait le chemin et vide le nom
//! (« setup for recovery »). Ces trois règles sont reproduites, parce qu'elles
//! décident si un vieux bloc se relit.

use mod_api::{Compound, Tag};

use super::building::normalise_path;
use super::nbt;
use super::plantation::{
    KEY_BLUEPRINT_DATA, KEY_CORNER_ONE, KEY_CORNER_TWO, KEY_MIRROR, KEY_NAME, KEY_PACK, KEY_PATH,
    PositionedTags,
};
use crate::base::Pos;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:25`).
pub const TYPE_ID: &str = "minecolonies:decorationcontroller";

/// La clé du nom de schéma dans le sous-compound.
pub const KEY_SCHEMATIC_NAME: &str = super::plantation::KEY_SCHEMATIC_NAME;
/// La valeur qui dit « rotation inconnue » (`:54`).
pub const ROTATION_UNKNOWN: i32 = -1;

/// L'état d'un contrôleur de décoration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : les champs sauvés de la classe Java, moins la rotation calculée
pub struct DecorationController {
    pub tags: PositionedTags,
    /// Les deux coins, relatifs au bloc (`:48-49`).
    pub corners: [Pos; 2],
    mirrored: bool,
    /// La rotation calculée, gardée une fois connue (`:54`) : `None` est le
    /// `-1` de Java.
    cached_rotation: Option<u8>,
    name: String,
    path: String,
    pack: String,
}

impl DecorationController {
    pub fn new() -> DecorationController {
        DecorationController::default()
    }

    /// `load` (`:152-188`).
    pub fn from_compound(data: &Compound) -> DecorationController {
        let inner = match data.get(&KEY_BLUEPRINT_DATA.into()) {
            Some(Tag::Compound(inner)) => inner.clone(),
            _ => Compound::new(),
        };
        let mut name = nbt::text(&inner, KEY_SCHEMATIC_NAME).unwrap_or_default();
        let mut path = nbt::text(&inner, KEY_PATH).unwrap_or_default();
        let mut pack = nbt::text(&inner, KEY_PACK).unwrap_or_default();
        // Les trois rattrapages de `:163-181`, dans l'ordre de Java.
        if let Some(legacy) = nbt::text(data, KEY_PATH) {
            if path.is_empty() {
                path = legacy;
            }
        }
        if let Some(legacy) = nbt::text(data, KEY_PACK) {
            if pack.is_empty() {
                pack = legacy;
            }
        }
        if let Some(legacy) = nbt::text(data, KEY_NAME) {
            if name.is_empty() {
                name = legacy;
                if path.is_empty() {
                    // « Setup for recovery » : le nom devient le chemin, et le
                    // nom se vide, parce qu'un chemin suffit à recharger.
                    path = name;
                    name = String::new();
                }
            }
        }
        DecorationController {
            tags: PositionedTags::from_blueprint_data(&inner),
            corners: [
                nbt::position_in(&inner, KEY_CORNER_ONE).unwrap_or([0, 0, 0]),
                nbt::position_in(&inner, KEY_CORNER_TWO).unwrap_or([0, 0, 0]),
            ],
            mirrored: nbt::boolean(data, KEY_MIRROR).unwrap_or(false),
            cached_rotation: None,
            name,
            path: normalise_path(&path).unwrap_or_default(),
            pack,
        }
    }

    /// `saveAdditional` (`:190-196`) : le sous-compound de schéma, plus le
    /// miroir. La rotation calculée n'est **pas** sauvée : elle se refait.
    pub fn merge(&self) -> Compound {
        let mut inner = self.tags.to_blueprint_data(&self.name, self.corners);
        inner.insert(KEY_PACK.into(), Tag::String(self.pack.as_str().into()));
        inner.insert(KEY_PATH.into(), Tag::String(self.path.as_str().into()));
        Compound::from([
            (KEY_BLUEPRINT_DATA.into(), Tag::Compound(inner)),
            (KEY_MIRROR.into(), nbt::boolean_tag(self.mirrored)),
        ])
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn pack(&self) -> &str {
        &self.pack
    }

    /// `setSchematicName` (`:86-91`).
    pub fn set_schematic_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    /// `setBlueprintPath` (`:206-213`) : le chemin, et l'extension si elle
    /// manque — même normaliseur que la hutte.
    pub fn set_blueprint_path(&mut self, path: &str) {
        self.path = normalise_path(path).unwrap_or_default();
    }

    /// `setPackName` (`:216-221`).
    pub fn set_pack(&mut self, pack: &str) {
        self.pack = pack.to_owned();
    }

    /// Les coins en coordonnées **du monde**, ou la position du bloc deux fois
    /// quand ils ne sont pas connus (`:122-131` : deux zéros veulent dire « pas
    /// encore posé »).
    pub fn corners_world(&self, at: Pos) -> [Pos; 2] {
        if self.corners == [[0, 0, 0], [0, 0, 0]] {
            return [at, at];
        }
        [
            [at[0] + self.corners[0][0], at[1] + self.corners[0][1], at[2] + self.corners[0][2]],
            [at[0] + self.corners[1][0], at[1] + self.corners[1][1], at[2] + self.corners[1][2]],
        ]
    }

    /// `rotate` (`:244-247`) : la rotation connue est oubliée.
    pub fn invalidate_rotation(&mut self) {
        self.cached_rotation = None;
    }

    /// `getRotation` (`:259-266`) : la rotation gardée, sinon celle que
    /// `compute` rend — une seule fois, comme Java avec
    /// `BuildingUtils.getRotationFromBlueprint`.
    pub fn rotation(&mut self, compute: impl FnOnce() -> u8) -> u8 {
        match self.cached_rotation {
            Some(rotation) => rotation,
            None => {
                let rotation = compute() % 4;
                self.cached_rotation = Some(rotation);
                rotation
            }
        }
    }

    /// `mirror` (`:249-253`) : `mirror != Mirror.NONE`.
    pub fn set_mirrored(&mut self, mirrored: bool) {
        self.mirrored = mirrored;
    }

    pub fn is_mirrored(&self) -> bool {
        self.mirrored
    }

    pub fn is_rotation_cached(&self) -> bool {
        self.cached_rotation.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller_with_schema() -> Compound {
        let inner = Compound::from([
            (KEY_SCHEMATIC_NAME.into(), Tag::String("deco".into())),
            (KEY_CORNER_ONE.into(), crate::hut::position([-1, 0, -1])),
            (KEY_CORNER_TWO.into(), crate::hut::position([3, 2, 3])),
            (KEY_PACK.into(), Tag::String("medieval".into())),
            (KEY_PATH.into(), Tag::String("deco/well1".into())),
        ]);
        Compound::from([
            (KEY_BLUEPRINT_DATA.into(), Tag::Compound(inner)),
            (KEY_MIRROR.into(), nbt::boolean_tag(true)),
        ])
    }

    #[test]
    fn le_sous_compound_se_lit_et_le_chemin_gagne_son_extension() {
        let deco = DecorationController::from_compound(&controller_with_schema());
        assert_eq!(deco.name(), "deco");
        assert_eq!(deco.path(), "deco/well1.blueprint");
        assert_eq!(deco.pack(), "medieval");
        assert!(deco.is_mirrored());
        assert!(!deco.is_rotation_cached(), "la rotation se calcule à la demande");
        assert_eq!(deco.corners, [[-1, 0, -1], [3, 2, 3]]);
    }

    #[test]
    fn un_nom_sans_chemin_devient_le_chemin_et_le_nom_se_vide() {
        let data = Compound::from([(KEY_NAME.into(), Tag::String("old/thing1".into()))]);
        let deco = DecorationController::from_compound(&data);
        assert_eq!(deco.name(), "");
        assert_eq!(deco.path(), "old/thing1.blueprint");
        // Avec un chemin déjà là, le nom reste un nom.
        let both = Compound::from([
            (KEY_NAME.into(), Tag::String("deco".into())),
            (KEY_PATH.into(), Tag::String("deco/well2".into())),
        ]);
        let deco = DecorationController::from_compound(&both);
        assert_eq!(deco.name(), "deco");
        assert_eq!(deco.path(), "deco/well2.blueprint");
    }

    #[test]
    fn des_coins_absents_valent_la_position_du_bloc() {
        let deco = DecorationController::new();
        assert_eq!(deco.corners_world([7, 64, -7]), [[7, 64, -7], [7, 64, -7]]);
        let placed = DecorationController::from_compound(&controller_with_schema());
        assert_eq!(
            placed.corners_world([10, 64, 10]),
            [[9, 64, 9], [13, 66, 13]]
        );
    }

    #[test]
    fn la_rotation_se_calcule_une_fois_puis_se_oublie_a_la_rotation() {
        let mut deco = DecorationController::new();
        let mut calls = 0;
        assert_eq!(
            deco.rotation(|| {
                calls += 1;
                2
            }),
            2
        );
        assert_eq!(
            deco.rotation(|| {
                calls += 1;
                3
            }),
            2,
            "la seconde lecture ne recalcule pas"
        );
        assert_eq!(calls, 1);
        deco.invalidate_rotation();
        assert!(!deco.is_rotation_cached());
        assert_eq!(
            deco.rotation(|| {
                calls += 1;
                3
            }),
            3
        );
        assert_eq!(calls, 2);
        // Un calcul hors bornes est ramené dans le tour.
        deco.invalidate_rotation();
        assert_eq!(deco.rotation(|| 6), 2);
    }

    #[test]
    fn le_miroir_se_lit_se_change_et_se_sauve() {
        let mut deco = DecorationController::new();
        assert!(!deco.is_mirrored());
        deco.set_mirrored(true);
        assert!(deco.is_mirrored());
        let read = DecorationController::from_compound(&deco.merge());
        assert!(read.is_mirrored());
    }

    #[test]
    fn l_etat_fait_un_aller_retour_avec_nom_chemin_pack_et_tags() {
        let mut deco = DecorationController::from_compound(&controller_with_schema());
        deco.tags.add([0, 1, 0], "deco");
        deco.set_schematic_name("well");
        deco.set_blueprint_path("deco/well2");
        deco.set_pack("ancient");
        let read = DecorationController::from_compound(&deco.merge());
        assert_eq!(read.name(), "well");
        assert_eq!(read.path(), "deco/well2.blueprint");
        assert_eq!(read.pack(), "ancient");
        assert_eq!(read.tags, deco.tags);
        assert_eq!(read.corners, deco.corners);
    }
}
