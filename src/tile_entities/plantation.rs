//! Le champ de plantation : ses positions de travail, lues dans les tags du
//! schéma, et l'orientation qui les place.
//!
//! Référence Java : `core/tileentities/TileEntityPlantationField.java` (331
//! lignes) sur `api/tileentities/AbstractTileEntityPlantationField.java` (86
//! lignes), plus le codec de tags commun aux block entities de schéma :
//! `IBlueprintDataProviderBE.java` (327 lignes, dans Structurize).
//!
//! **Un champ est un ensemble de tags, pas un rectangle.** Le schéma porte des
//! marqueurs (`field`, `water`, `storage`…) posés à des coordonnées **relatives**
//! au bloc ; `getWorkingPositions(tag)` (`:107-116`) rend les positions
//! absolues des marqueurs qui portent ce tag, sans doublon, et les garde en
//! cache. Le port calcule la même chose sans cache (une lecture de `BTreeMap` ne
//! coûte rien) et rend l'ordre déterministe, ce que `HashMap` ne promet pas.
//!
//! **Deux block entities partagent ce codec.** [`PositionedTags`] est le
//! `IBlueprintDataProviderBE` que la plantation et le contrôleur de décoration
//! implémentent tous deux (`TileEntityDecorationController.java:32-33`,
//! `TileEntityPlantationField.java:29-30`) ; il est écrit ici une fois, et
//! `decoration.rs` s'en sert. La hutte de bâtiment l'implémente aussi en Java,
//! mais elle n'en lit que le nom de schéma : voir [`super::building`].
//!
//! **Ce qui n'est pas ici.** `getPlantationFieldTypes` (`:95-104`) passe par
//! `BuildingExtensionRegistries`, un registre du jeu : le port rend les **tags**
//! et pas les extensions qu'ils désignent, et `MANQUANT.md` porte la ligne. La
//! transformation d'une position relative en position du monde après rotation
//! (`applyToPos`) est `StructureTemplate.transform`, du code vanilla absent de
//! `minecolonies-src` : le port garde la rotation et le miroir comme données,
//! pas comme application.

use std::collections::{BTreeMap, BTreeSet};

use mod_api::{Compound, Tag};

use super::building::normalise_path;
use super::nbt;
use crate::base::Pos;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:19`).
pub const TYPE_ID: &str = "minecolonies:plantationfield";

/// La clé du sous-compound de schéma (`IBlueprintDataProviderBE.java:26`).
pub const KEY_BLUEPRINT_DATA: &str = "blueprintDataProvider";
pub const KEY_SCHEMATIC_NAME: &str = "schematicName";
pub const KEY_CORNER_ONE: &str = "corner1";
pub const KEY_CORNER_TWO: &str = "corner2";
/// La liste des positions marquées (`:25`).
pub const KEY_TAG_POS_MAP: &str = "posTagMap";
pub const KEY_TAG_POS: &str = "tagPos";
pub const KEY_TAG_NAME_LIST: &str = "tagNameList";
pub const KEY_TAG_NAME: &str = "tagName";

/// La rotation sauvegardée (`:297`) et le miroir (`:298`).
pub const KEY_ROTATION: &str = "rotation";
pub const KEY_MIRROR: &str = "mirror";
pub const KEY_NAME: &str = "name";
pub const KEY_PATH: &str = "path";
pub const KEY_PACK: &str = "pack";

/// Une rotation et un miroir, dans l'ordre de `RotationMirror`
/// (`structurize/util/RotationMirror.java:14-21`) : quatre rotations, puis les
/// mêmes quatre en miroir.
///
/// Le port garde deux champs là où Java a huit variantes, parce que la table de
/// Java est exactement `rotation + 4 × miroir` : [`RotationMirror::index`] la
/// reconstruit, et les tables de `rotate` et `mirrorate` (`:33-89`) sont
/// reproduites par deux opérations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : la rotation et le miroir de RotationMirror
pub struct RotationMirror {
    /// `0` aucune, `1` quart de tour, `2` demi-tour, `3` trois quarts.
    rotation: u8,
    mirrored: bool,
}

impl RotationMirror {
    pub const NONE: RotationMirror = RotationMirror {
        rotation: 0,
        mirrored: false,
    };

    /// `RotationMirror.of(rotation, mirror)` (`:145-148`) : le miroir d'abord,
    /// la rotation ensuite.
    pub fn of(rotation: u8, mirrored: bool) -> RotationMirror {
        RotationMirror {
            rotation: rotation % 4,
            mirrored,
        }
    }

    /// La rotation sauvegardée est un ordinal de `Rotation`
    /// (`TileEntityPlantationField.java:267-268`) : `NONE`, `CLOCKWISE_90`,
    /// `CLOCKWISE_180`, `COUNTERCLOCKWISE_90`.
    pub fn from_compound(data: &Compound) -> RotationMirror {
        let rotation = nbt::int(data, KEY_ROTATION).unwrap_or(0).rem_euclid(4) as u8;
        RotationMirror::of(rotation, nbt::boolean(data, KEY_MIRROR).unwrap_or(false))
    }

    pub fn merge(&self) -> Compound {
        Compound::from([
            (KEY_ROTATION.into(), Tag::Int(i32::from(self.rotation))),
            (KEY_MIRROR.into(), nbt::boolean_tag(self.mirrored)),
        ])
    }

    /// Le quart de tour dans le sens des aiguilles (`rotate(CLOCKWISE_90)`,
    /// `:66-73`).
    pub fn rotate_clockwise(self) -> RotationMirror {
        RotationMirror {
            rotation: (self.rotation + 1) % 4,
            mirrored: self.mirrored,
        }
    }

    /// `rotate(rotation)` (`:98-107`) : un quart de tour `times` fois.
    pub fn rotate(self, times: u8) -> RotationMirror {
        let mut result = self;
        for _ in 0..(times % 4) {
            result = result.rotate_clockwise();
        }
        result
    }

    /// `mirrorate(FRONT_BACK)` (`:44-51`, `:112-122`) : le miroir inverse le
    /// sens de rotation **et** bascule le drapeau — `R90` donne `MIR_R270`,
    /// `R180` donne `MIR_R180`.
    pub fn mirror_front_back(self) -> RotationMirror {
        RotationMirror {
            rotation: (4 - self.rotation) % 4,
            mirrored: !self.mirrored,
        }
    }

    pub fn rotation(self) -> u8 {
        self.rotation
    }

    pub fn is_mirrored(self) -> bool {
        self.mirrored
    }

    /// L'indice dans l'énumération Java (`:14-21`).
    pub fn index(self) -> u8 {
        self.rotation + if self.mirrored { 4 } else { 0 }
    }
}

/// Les positions marquées d'un schéma, **relatives** au bloc.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : une seule table, comme `tagPosMap` en Java
pub struct PositionedTags {
    tags: BTreeMap<Pos, Vec<String>>,
}

impl PositionedTags {
    pub fn new() -> PositionedTags {
        PositionedTags::default()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Les tags d'une position relative, dans l'ordre où ils ont été ajoutés
    /// (`getPositionedTags`, `TileEntityPlantationField.java:196-199`).
    pub fn at(&self, pos: Pos) -> &[String] {
        self.tags.get(&pos).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Toute la table, pour un appelant qui doit la parcourir.
    pub fn all(&self) -> &BTreeMap<Pos, Vec<String>> {
        &self.tags
    }

    /// `addTag` (`IBlueprintDataProviderBE.java:220-234`) : le tag s'ajoute à la
    /// liste de la position, ou la crée.
    pub fn add(&mut self, pos: Pos, tag: &str) {
        let entry = self.tags.entry(pos).or_default();
        if !entry.iter().any(|existing| existing == tag) {
            entry.push(tag.to_owned());
        }
    }

    /// `removeTag` (`:201-213`) : retirer un tag vide **et supprime la
    /// position**, comme Java.
    pub fn remove(&mut self, pos: Pos, tag: &str) {
        if let Some(tags) = self.tags.get_mut(&pos) {
            tags.retain(|existing| existing != tag);
            if tags.is_empty() {
                self.tags.remove(&pos);
            }
        }
    }

    /// Les positions de travail d'un tag, en absolu (`getWorkingPositions`,
    /// `TileEntityPlantationField.java:107-116`) : les positions marquées,
    /// distinctes, décalées par la position du bloc.
    pub fn working_positions(&self, tag: &str, at: Pos) -> Vec<Pos> {
        let mut found = BTreeSet::new();
        for (relative, tags) in &self.tags {
            if tags.iter().any(|existing| existing == tag) {
                found.insert([at[0] + relative[0], at[1] + relative[1], at[2] + relative[2]]);
            }
        }
        found.into_iter().collect()
    }

    /// Tous les tags du schéma. Java en tire les extensions de champ
    /// (`:95-104`) ; le port rend les tags eux-mêmes, faute du registre.
    pub fn tag_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for tags in self.tags.values() {
            for tag in tags {
                names.insert(tag.clone());
            }
        }
        names
    }

    /// Le sous-compound `blueprintDataProvider`, tel que Structurize l'écrit
    /// (`writeSchematicDataToNBT`, `:83-95`).
    pub fn to_blueprint_data(&self, schematic_name: &str, corners: [Pos; 2]) -> Compound {
        let mut records = Vec::new();
        for (pos, tags) in &self.tags {
            let lines = tags
                .iter()
                .map(|tag| {
                    Compound::from([(KEY_TAG_NAME.into(), Tag::String(tag.as_str().into()))])
                })
                .collect();
            records.push(Compound::from([
                (KEY_TAG_POS.into(), crate::hut::position(*pos)),
                (KEY_TAG_NAME_LIST.into(), nbt::compound_list(lines)),
            ]));
        }
        Compound::from([
            (
                KEY_SCHEMATIC_NAME.into(),
                Tag::String(schematic_name.into()),
            ),
            (KEY_CORNER_ONE.into(), crate::hut::position(corners[0])),
            (KEY_CORNER_TWO.into(), crate::hut::position(corners[1])),
            (KEY_TAG_POS_MAP.into(), nbt::compound_list(records)),
        ])
    }

    /// Lit la table depuis le sous-compound (`readTagPosMapFrom`, `:156-194`).
    /// Un enregistrement sans position lisible est sauté : une position
    /// incomplète n'est pas l'origine.
    pub fn from_blueprint_data(data: &Compound) -> PositionedTags {
        let mut tags: BTreeMap<Pos, Vec<String>> = BTreeMap::new();
        for record in nbt::compounds(data, KEY_TAG_POS_MAP) {
            let Some(pos) = nbt::position_in(record, KEY_TAG_POS) else {
                continue;
            };
            let entry = tags.entry(pos).or_default();
            for line in nbt::compounds(record, KEY_TAG_NAME_LIST) {
                if let Some(name) = nbt::text(line, KEY_TAG_NAME) {
                    if !entry.iter().any(|existing| existing == &name) {
                        entry.push(name);
                    }
                }
            }
        }
        PositionedTags { tags }
    }
}

/// Le champ de plantation : ses tags, son orientation, son schéma.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : les quatre champs sauvés de la classe Java
pub struct Plantation {
    pub tags: PositionedTags,
    pub placement: RotationMirror,
    /// Les deux coins du schéma, relatifs au bloc (`:240-244`). Deux zéros
    /// signifient « pas de schéma » en Java (`:250-256`).
    pub corners: [Pos; 2],
    name: String,
    path: String,
    pack: String,
}

impl Plantation {
    pub fn new() -> Plantation {
        Plantation::default()
    }

    /// `load` (`:263-289`) : la table de tags vient du sous-compound, la
    /// rotation et le miroir de leurs clés, et le chemin est normalisé.
    pub fn from_compound(data: &Compound) -> Plantation {
        let inner = match data.get(&KEY_BLUEPRINT_DATA.into()) {
            Some(Tag::Compound(inner)) => inner.clone(),
            _ => Compound::new(),
        };
        let path = nbt::text(data, KEY_PATH)
            .or_else(|| nbt::text(&inner, KEY_PATH))
            .unwrap_or_default();
        Plantation {
            tags: PositionedTags::from_blueprint_data(&inner),
            placement: RotationMirror::from_compound(data),
            corners: [
                nbt::position_in(&inner, KEY_CORNER_ONE).unwrap_or([0, 0, 0]),
                nbt::position_in(&inner, KEY_CORNER_TWO).unwrap_or([0, 0, 0]),
            ],
            name: nbt::text(data, KEY_NAME)
                .or_else(|| nbt::text(&inner, KEY_SCHEMATIC_NAME))
                .unwrap_or_default(),
            path: normalise_path(&path).unwrap_or_default(),
            pack: nbt::text(data, KEY_PACK)
                .or_else(|| nbt::text(&inner, KEY_PACK))
                .unwrap_or_default(),
        }
    }

    /// `saveAdditional` (`:291-303`) : le sous-compound de schéma
    /// (`writeSchematicDataToNBT`), puis **au premier niveau** la rotation, le
    /// miroir, le nom, le chemin et le pack — Java écrit ces cinq-là sur le
    /// compound de la block entity, pas dans le sous-compound (`:295-301`), et
    /// `load` les y relit (`:267`).
    pub fn merge(&self) -> Compound {
        let mut inner = self.tags.to_blueprint_data(&self.name, self.corners);
        // `writeSchematicDataToNBT` écrit aussi le pack et le chemin **dans** le
        // sous-compound (`IBlueprintDataProviderBE.java:93-94`).
        inner.insert(KEY_PACK.into(), Tag::String(self.pack.as_str().into()));
        inner.insert(KEY_PATH.into(), Tag::String(self.path.as_str().into()));
        let mut data = Compound::from([
            (KEY_BLUEPRINT_DATA.into(), Tag::Compound(inner)),
            (KEY_NAME.into(), Tag::String(self.name.as_str().into())),
            (KEY_PATH.into(), Tag::String(self.path.as_str().into())),
            (KEY_PACK.into(), Tag::String(self.pack.as_str().into())),
        ]);
        data.extend(self.placement.merge());
        data
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

    /// `setSchematicName` (`:191-195`).
    pub fn set_schematic_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    /// `setBlueprintPath` (`:316-329`) : ici, contrairement à la hutte de
    /// bâtiment (`TileEntityColonyBuilding.java:588-593`), Java ajoute
    /// l'extension quand elle manque.
    pub fn set_blueprint_path(&mut self, path: &str) {
        self.path = normalise_path(path).unwrap_or_default();
    }

    /// `setPackName` (`:333-337`).
    pub fn set_pack(&mut self, pack: &str) {
        self.pack = pack.to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le sous-compound que Structurize écrit, réduit à ce que ce fichier lit.
    fn blueprint_data() -> Compound {
        let record = |pos: Pos, tags: &[&str]| {
            Compound::from([
                (KEY_TAG_POS.into(), crate::hut::position(pos)),
                (
                    KEY_TAG_NAME_LIST.into(),
                    nbt::compound_list(
                        tags.iter()
                            .map(|tag| {
                                Compound::from([(
                                    KEY_TAG_NAME.into(),
                                    Tag::String((*tag).into()),
                                )])
                            })
                            .collect(),
                    ),
                ),
            ])
        };
        Compound::from([
            (
                KEY_SCHEMATIC_NAME.into(),
                Tag::String("plantation".into()),
            ),
            (KEY_CORNER_ONE.into(), crate::hut::position([0, 0, 0])),
            (KEY_CORNER_TWO.into(), crate::hut::position([8, 3, 8])),
            (
                KEY_TAG_POS_MAP.into(),
                nbt::compound_list(vec![
                    record([1, 0, 1], &["field"]),
                    record([2, 0, 1], &["field", "water"]),
                    record([0, 0, 3], &["storage"]),
                ]),
            ),
        ])
    }

    fn at(tags: &PositionedTags, pos: Pos) -> Vec<String> {
        tags.at(pos).to_vec()
    }

    #[test]
    fn les_tags_d_un_schema_se_lisent_et_se_relisent() {
        let tags = PositionedTags::from_blueprint_data(&blueprint_data());
        assert_eq!(tags.len(), 3);
        assert_eq!(at(&tags, [1, 0, 1]), vec!["field"]);
        assert_eq!(at(&tags, [2, 0, 1]), vec!["field", "water"]);
        assert_eq!(tags.at([9, 9, 9]).len(), 0);
        let written = tags.to_blueprint_data("plantation", [[0, 0, 0], [8, 3, 8]]);
        assert_eq!(PositionedTags::from_blueprint_data(&written), tags);
    }

    #[test]
    fn les_positions_de_travail_sont_absolues_distinctes_et_triees() {
        let tags = PositionedTags::from_blueprint_data(&blueprint_data());
        assert_eq!(
            tags.working_positions("field", [100, 64, -100]),
            vec![[101, 64, -99], [102, 64, -99]]
        );
        assert_eq!(
            tags.working_positions("storage", [100, 64, -100]),
            vec![[100, 64, -97]]
        );
        assert!(tags.working_positions("absent", [0, 0, 0]).is_empty());
    }

    #[test]
    fn les_noms_de_tags_sont_l_ensemble_des_marqueurs() {
        let tags = PositionedTags::from_blueprint_data(&blueprint_data());
        let names = tags.tag_names();
        assert_eq!(names.len(), 3, "field, storage, water");
        assert!(names.contains("water"));
        // Java en tire des extensions de champ ; le port n'a que les tags et le dit.
        assert!(!names.contains("minecolonies:plantation"));
    }

    #[test]
    fn ajouter_puis_retirer_un_tag_supprime_la_position_quand_elle_vide() {
        let mut tags = PositionedTags::new();
        tags.add([1, 2, 3], "field");
        tags.add([1, 2, 3], "field");
        assert_eq!(at(&tags, [1, 2, 3]), vec!["field"], "pas de doublon");
        tags.add([1, 2, 3], "water");
        assert_eq!(tags.at([1, 2, 3]).len(), 2);
        tags.remove([1, 2, 3], "field");
        assert_eq!(at(&tags, [1, 2, 3]), vec!["water"]);
        tags.remove([1, 2, 3], "water");
        assert!(tags.is_empty(), "la position s'en va avec son dernier tag");
    }

    #[test]
    fn l_orientation_se_relit_dans_l_ordre_de_java() {
        let data = Compound::from([
            (KEY_ROTATION.into(), Tag::Int(1)),
            (KEY_MIRROR.into(), nbt::boolean_tag(true)),
        ]);
        let placement = RotationMirror::from_compound(&data);
        assert_eq!(placement, RotationMirror::of(1, true));
        assert_eq!(placement.index(), 5, "MIR_R90 (`:18`)");
        // Un ordinal hors bornes se ramène dans le tour.
        let strange = Compound::from([(KEY_ROTATION.into(), Tag::Int(6))]);
        assert_eq!(RotationMirror::from_compound(&strange).rotation(), 2);
    }

    #[test]
    fn quatre_quarts_de_tour_font_un_tour_et_le_miroir_inverse_le_sens() {
        let start = RotationMirror::NONE;
        let mut turned = start;
        for _ in 0..4 {
            turned = turned.rotate_clockwise();
        }
        assert_eq!(turned, start);
        assert_eq!(start.rotate(5), start.rotate(1), "un tour ne compte pas");
        // `mirrorFB` : R90 → MIR_R270 (`RotationMirror.java:44-51`).
        assert_eq!(
            RotationMirror::of(1, false).mirror_front_back(),
            RotationMirror::of(3, true)
        );
        // Deux miroirs rendent l'original.
        assert_eq!(
            RotationMirror::of(1, false).mirror_front_back().mirror_front_back(),
            RotationMirror::of(1, false)
        );
        // `of` applique le miroir puis la rotation (`:145-148`).
        assert_eq!(RotationMirror::of(1, true).index(), 5);
    }

    #[test]
    fn le_champ_fait_un_aller_retour_avec_son_schema() {
        // Un champ **posé** vient d'un schéma : son nom est le `schematicName`
        // du sous-compound (`:274-282`), il n'est pas inventé à la construction.
        let placed = Compound::from([(KEY_BLUEPRINT_DATA.into(), Tag::Compound(blueprint_data()))]);
        let mut field = Plantation::from_compound(&placed);
        assert_eq!(field.name(), "plantation");
        field.placement = RotationMirror::of(2, false);
        let mut corners = field.corners;
        corners[0] = [-2, 0, -2];
        corners[1] = [6, 4, 6];
        field.corners = corners;
        let data = field.merge();
        // La rotation est écrite **au premier niveau**, comme `saveAdditional`
        // (`TileEntityPlantationField.java:297-298`), et non dans
        // `blueprintDataProvider` : un aller-retour seul ne le dirait pas, une
        // relecture fautive pouvant retrouver la bonne valeur au mauvais endroit.
        assert_eq!(nbt::int(&data, KEY_ROTATION), Some(2));
        assert_eq!(nbt::boolean(&data, KEY_MIRROR), Some(false));
        let read = Plantation::from_compound(&data);
        assert_eq!(read.tags, field.tags);
        assert_eq!(read.corners, corners, "les deux coins font l'aller-retour");
        assert_eq!(
            read.placement, field.placement,
            "la rotation et le miroir sont au premier niveau (`:297-298`)"
        );
        assert_eq!(read.name(), "plantation");
        // Le chemin d'un champ neuf est vide : il n'y a pas de schéma par défaut.
        assert_eq!(read.path(), "");
        assert_eq!(read.pack(), "");
    }

    #[test]
    fn un_chemin_de_schema_gagne_son_extension_via_le_meme_normaliseur_que_la_hutte() {
        let data = Compound::from([
            (KEY_PATH.into(), Tag::String("fields/wheat1".into())),
            (KEY_NAME.into(), Tag::String("wheat".into())),
        ]);
        let field = Plantation::from_compound(&data);
        assert_eq!(field.path(), "fields/wheat1.blueprint");
        assert_eq!(field.name(), "wheat");
    }

    #[test]
    fn les_poseurs_de_schema_normalisent_le_chemin_et_ne_touchent_pas_au_reste() {
        let mut field = Plantation::new();
        field.set_schematic_name("wheat");
        field.set_blueprint_path("fields/wheat1");
        field.set_pack("medieval");
        assert_eq!(field.name(), "wheat");
        // `setBlueprintPath` ajoute l'extension (`:316-329`), contrairement à
        // celui de la hutte de bâtiment (`TileEntityColonyBuilding.java:588-593`).
        assert_eq!(field.path(), "fields/wheat1.blueprint");
        assert_eq!(field.pack(), "medieval");
        // Un chemin vide reste vide : `normalise_path` refuse un chemin sans nom.
        field.set_blueprint_path("");
        assert_eq!(field.path(), "");
    }
}
