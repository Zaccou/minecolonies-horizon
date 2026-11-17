//! Le block entity d'une hutte : son rattachement à une colonie, son accès, et
//! ce que son tick recalcule.
//!
//! Référence Java : `core/tileentities/TileEntityColonyBuilding.java` (758 lignes)
//! sur `api/tileentities/AbstractTileEntityColonyBuilding.java` (318 lignes).
//!
//! **Une classe, cinq block entities.** En Java, la hutte d'un bâtiment, la
//! **warehouse**, l'**enchanter** et la **stash** sont la même classe par
//! héritage (`TileEntityWareHouse.java:234`, `TileEntityEnchanter.java:17`,
//! `TileEntityStash.java:34` déclarent tous `extends TileEntityColonyBuilding`) :
//! elles héritent du rattachement, du miroir, du schéma et de l'accès. Le port
//! n'a pas d'héritage : ce fichier porte ces règles une fois, et
//! `warehouse.rs`, `enchanter.rs` et `stash.rs` les reprennent en appelant
//! [`BuildingEntity`] au lieu de les recopier.
//!
//! **Deux moitiés dans le Java.** Le bloc de `tick()` (`:449-525`) fait trois
//! choses de natures différentes : adopter la colonie quand le bloc est posé
//! hors colonie, rafraîchir les panneaux du bâtiment une fois par seconde, et
//! appliquer un blueprint dont le chargement asynchrone vient de finir. Le port
//! ne garde que la **décision** — quel travail est dû ce tick-là — dans
//! [`BuildingEntity::steps`] ; l'effet lui-même demande le jeu (poser un bloc,
//! écrire sur un panneau, charger un schéma) et reste à l'appelant, ce que
//! `MANQUANT.md` chiffre.
//!
//! **Ce que la classe retient** (`:83-124`) : l'identifiant de colonie, le
//! miroir, le pack et le chemin du schéma, et le nom de registre du bâtiment.
//! Les clés sauvées sont `colony`, `mirror`, `pack`, `path`, `type`
//! (`:75-79`, `:433-448`). Le reste (`colony`, `building`, les caches) est
//! reconstruit, jamais sauvé.

use mod_api::{Compound, Tag};

use super::nbt;
use crate::colony::permissions::{Action, Members, Refusal};

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:23`).
pub const TYPE_ID: &str = "minecolonies:colonybuilding";

/// Les clés sauvées (`TileEntityColonyBuilding.java:75-79`), plus le type de
/// bâtiment (`NbtTagConstants.java:140`, écrit à `:433-448`).
pub const KEY_COLONY: &str = crate::hut::KEY_COLONY;
pub const KEY_MIRROR: &str = "mirror";
pub const KEY_PACK: &str = "pack";
pub const KEY_PATH: &str = "path";
pub const KEY_BUILDING_TYPE: &str = "type";
/// Le nom du schéma, dans le sous-compound d'un blueprint que Structurize écrit
/// (`IBlueprintDataProviderBE.java:19`, lu par `:425`). Le port le connaît comme
/// clé, pas comme schéma chargé.
pub const KEY_SCHEMATIC_NAME: &str = "schematicName";
/// L'extension que Java ajoute à un chemin qui ne l'a pas (`:422-426`).
pub const BLUEPRINT_SUFFIX: &str = ".blueprint";

/// Ce que le tick d'un bâtiment a à faire. Java l'écrit comme un corps de
/// méthode (`:449-525`) ; le port nomme les trois travaux pour qu'un tick se
/// teste sans jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les trois travaux du tick Java
pub enum Step {
    /// Le bloc est posé dans une colonie sans la connaître : adopter son
    /// identifiant par la position (`:460-465`).
    AdoptColony,
    /// La colonie est connue mais son objet n'est pas encore résolu
    /// (`:508-511`).
    ResolveColony,
    /// Écrire le nom et le niveau du bâtiment sur les panneaux `building_sign`
    /// (`:470-503`), une fois par seconde.
    RefreshSigns,
    /// Un blueprint asynchrone est arrivé et attend d'être lu (`:513-519`).
    ApplyBlueprint,
}

/// L'état sauvé d'un bâtiment.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les cinq champs sauvés de la classe Java
pub struct BuildingEntity {
    /// L'identifiant de colonie, `None` quand le bloc n'en connaît aucune :
    /// Java garde `0` (`:90`) et le port nomme l'absence avec la même règle que
    /// `crate::hut::colony_of` — zéro est un lien effacé, pas la colonie zéro.
    colony: Option<i32>,
    /// Le bâtiment est-il posé en miroir ? (`:100`, `:553-557`).
    mirror: bool,
    /// Le pack de structures du schéma (`:104`).
    pack: String,
    /// Le chemin du schéma dans le pack (`:107`).
    path: String,
    /// Le nom de registre du bâtiment, quand il est connu (`:114`);
    /// `None` tant que la colonie ne l'a pas donné (`:203-206`).
    building_type: Option<String>,
}

impl Default for BuildingEntity {
    fn default() -> BuildingEntity {
        BuildingEntity {
            colony: None,
            mirror: false,
            pack: String::new(),
            path: String::new(),
            building_type: None,
        }
    }
}

impl BuildingEntity {
    /// Le bloc tel qu'il est posé : il ne connaît encore aucune colonie. Java
    /// écrit `colonyId = 0` et laisse `mirror` faux, `pack` et `path` vides
    /// (`:90-107`) ; le lien est écrit plus tard, par qui connaît la colonie.
    pub fn new() -> BuildingEntity {
        BuildingEntity::default()
    }

    /// Lectures tolérantes : une clé absente prend la valeur par défaut de Java
    /// (`:361-435`). Un `colony` à zéro ou négatif est un lien absent.
    pub fn from_compound(data: &Compound) -> BuildingEntity {
        BuildingEntity {
            colony: crate::hut::colony_of(data),
            mirror: nbt::boolean(data, KEY_MIRROR).unwrap_or(false),
            pack: nbt::text(data, KEY_PACK).unwrap_or_default(),
            path: nbt::text(data, KEY_PATH).unwrap_or_default(),
            building_type: nbt::text(data, KEY_BUILDING_TYPE).filter(|name| !name.is_empty()),
        }
    }

    /// Ce qu'il faut fusionner pour sauver l'état (`saveAdditional`, `:438-451`).
    ///
    /// **Java écrit `colony` même quand il n'y en a pas** — `putInt(TAG_COLONY,
    /// colonyId)` sans condition, `:441` — donc un zéro, et `0` se relit comme
    /// aucun lien (`crate::hut::colony_of`). `type` est la seule clé
    /// conditionnelle (`:446-448` : `if (registryName != null)`).
    pub fn merge(&self) -> Compound {
        let mut data = Compound::from([
            (
                KEY_COLONY.into(),
                Tag::Int(self.colony.unwrap_or(0)),
            ),
            (KEY_MIRROR.into(), nbt::boolean_tag(self.mirror)),
            (KEY_PACK.into(), Tag::String(self.pack.as_str().into())),
            (KEY_PATH.into(), Tag::String(self.path.as_str().into())),
        ]);
        if let Some(kind) = &self.building_type {
            data.insert(KEY_BUILDING_TYPE.into(), Tag::String(kind.as_str().into()));
        }
        data
    }

    pub fn colony(&self) -> Option<i32> {
        self.colony
    }

    pub fn is_mirrored(&self) -> bool {
        self.mirror
    }

    pub fn pack(&self) -> &str {
        &self.pack
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn building_type(&self) -> Option<&str> {
        self.building_type.as_deref()
    }

    /// Adopter une colonie (`setColony`, `:262-268`) : Java écrit l'identifiant
    /// et marque le bloc modifié. Le port rend le payload à fusionner, la
    /// position du bâtiment principal restant celle que `crate::hut` écrit — un
    /// lien sans position n'est pas suivable (`crate::hut::with_colony`).
    pub fn link(&mut self, colony: i32, main: [i32; 3]) -> Compound {
        self.colony = Some(colony);
        crate::hut::with_colony(colony, main)
    }

    /// Le miroir du schéma (`:549-557`).
    pub fn set_mirrored(&mut self, mirror: bool) {
        self.mirror = mirror;
    }

    /// Le nom de registre du bâtiment, une fois la colonie résolue
    /// (`:199-206`).
    pub fn set_building_type(&mut self, name: &str) {
        self.building_type = Some(name.to_owned());
    }

    /// `setPackName` (`:595-600`) : le pack de structures du schéma. Il n'y a pas
    /// de variante qui normalise le chemin ici, contrairement au contrôleur de
    /// décoration : Java affecte tel quel dans les deux cas
    /// (`TileEntityColonyBuilding.java:588-593` ne touche pas au suffixe).
    pub fn set_pack(&mut self, pack: &str) {
        self.pack = pack.to_owned();
    }

    /// `setBlueprintPath` (`:588-593`) : le chemin du schéma, **sans**
    /// normalisation.
    pub fn set_path(&mut self, path: &str) {
        self.path = path.to_owned();
    }

    /// **La règle d'accès.** `hasAccessPermission` (`:539-543`) est
    /// `building == null || colony.permissions.hasPermission(player, ACCESS_HUTS)` :
    /// un bloc qui n'appartient à aucun bâtiment ne refuse personne, et un bloc
    /// rattaché demande le bit `ACCESS_HUTS` que seul `Friend` et au-dessus
    /// portent (`Permissions.java:118-127`).
    ///
    /// Le refus est celui de `colony::permissions`, nommé : un `Neutral` est
    /// refusé par `LacksAction { rank: Neutral, action: AccessHuts }`, pas par un
    /// `false`.
    pub fn access(&self, members: &Members, player: u128) -> Result<(), Refusal> {
        if self.colony.is_none() {
            return Ok(());
        }
        members.may(player, Action::AccessHuts)
    }

    /// `isUsableByPlayer` (`:527-531`) : c'est la même règle, sans second chemin.
    pub fn is_usable_by(&self, members: &Members, player: u128) -> bool {
        self.access(members, player).is_ok()
    }

    /// Les travaux que ce tick demande, dans l'ordre où Java les fait
    /// (`:449-525`).
    ///
    /// Les deux premiers s'excluent : Java teste `colonyId == 0` puis un `else`
    /// (`:460`, `:467`), donc un bloc qui vient d'adopter sa colonie ne
    /// rafraîchit pas les panneaux au même tick.
    ///
    /// `has_blueprint` dit qu'un chargement asynchrone est terminé — le port ne
    /// lance aucun chargement, il dit quand l'appliquer. `colony_resolved` dit
    /// que l'objet colonie est déjà en mémoire, ce que Java teste par
    /// `colony == null` (`:508`).
    pub fn steps(&self, tick: u64, has_blueprint: bool, colony_resolved: bool) -> Vec<Step> {
        let mut steps = Vec::new();
        if self.colony.is_none() {
            steps.push(Step::AdoptColony);
        } else {
            if tick % u64::from(super::TICKS_PER_SECOND) == 0 {
                steps.push(Step::RefreshSigns);
            }
            if !colony_resolved {
                steps.push(Step::ResolveColony);
            }
        }
        if has_blueprint {
            steps.push(Step::ApplyBlueprint);
        }
        steps
    }
}

/// Le chemin d'un schéma, normalisé comme Java le normalise (`:416-426`) :
/// vide ou porteur de `null` n'est pas un chemin, et l'extension est ajoutée si
/// elle manque.
///
/// Rend `None` là où Java retombe sur un chemin par défaut construit par
/// `BlueprintMapping` : cette table vient du pack de structures, pas du code
/// (`MANQUANT.md`).
pub fn normalise_path(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || path.contains("null") {
        return None;
    }
    if path.ends_with(BLUEPRINT_SUFFIX) {
        Some(path.to_owned())
    } else {
        Some(format!("{path}{BLUEPRINT_SUFFIX}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::permissions::Rank;

    const OWNER: u128 = 1;
    const OFFICER: u128 = 2;
    const FRIEND: u128 = 3;
    const STRANGER: u128 = 4;
    const MAIN: [i32; 3] = [10, 64, -10];

    fn colony() -> Members {
        let mut members = Members::with_owner(OWNER);
        members.set(OFFICER, Rank::Officer);
        members.set(FRIEND, Rank::Friend);
        members
    }

    #[test]
    fn un_bloc_pose_ecrit_une_colonie_a_zero_et_pas_de_type() {
        let building = BuildingEntity::new();
        assert_eq!(building.colony(), None);
        assert!(!building.is_mirrored());
        assert_eq!(building.pack(), "");
        assert_eq!(building.path(), "");
        assert_eq!(building.building_type(), None);
        let merge = building.merge();
        // `putInt(TAG_COLONY, colonyId)` est inconditionnel (`:441`) : un bloc
        // sans colonie écrit **zéro**, et `0` se relit comme aucun lien.
        assert_eq!(merge.len(), 4, "colony, mirror, pack, path");
        assert_eq!(nbt::int(&merge, KEY_COLONY), Some(0));
        assert_eq!(BuildingEntity::from_compound(&merge).colony(), None);
        // `type` est la seule clé conditionnelle (`:446-448`).
        assert!(!merge.contains_key(&KEY_BUILDING_TYPE.into()));
    }

    #[test]
    fn le_lien_s_ecrit_avec_la_position_du_batiment_principal() {
        let mut building = BuildingEntity::new();
        let payload = building.link(7, MAIN);
        assert_eq!(building.colony(), Some(7));
        assert_eq!(crate::hut::colony_of(&payload), Some(7));
        assert_eq!(
            crate::hut::position_in(&payload, crate::hut::KEY_MAIN),
            Some(MAIN)
        );
    }

    #[test]
    fn les_cinq_cles_font_un_aller_retour() {
        let mut building = BuildingEntity::new();
        building.link(3, MAIN);
        building.set_mirrored(true);
        building.set_building_type("minecolonies:builder");
        // Le pack et le chemin sont posés **sur l'entité** : `saveAdditional` les
        // écrit toujours (`:443-445`), donc une valeur posée seulement dans un
        // compound serait écrasée par la sauvegarde, comme en Java.
        building.set_pack("medieval_oak");
        building.set_path("builder1.blueprint");
        // L'écriture est éprouvée **avant** la relecture : `saveAdditional`
        // (`TileEntityColonyBuilding.java:444-445`) pose `pack` et `path` sous
        // ces deux noms-là, et un aller-retour seul ne distingue pas « clé
        // jamais écrite » de « clé relue sous un autre nom ».
        let merge = building.merge();
        assert_eq!(nbt::text(&merge, KEY_PACK).as_deref(), Some("medieval_oak"));
        assert_eq!(
            nbt::text(&merge, KEY_PATH).as_deref(),
            Some("builder1.blueprint")
        );
        let read = BuildingEntity::from_compound(&merge);
        assert_eq!(read.colony(), Some(3));
        assert!(read.is_mirrored());
        assert_eq!(read.building_type(), Some("minecolonies:builder"));
        assert_eq!(read.pack(), "medieval_oak");
        assert_eq!(read.path(), "builder1.blueprint");
    }

    #[test]
    fn une_colonie_zero_ou_absente_est_un_lien_absent() {
        let zero = Compound::from([(KEY_COLONY.into(), Tag::Int(0))]);
        assert_eq!(BuildingEntity::from_compound(&zero).colony(), None);
        let negative = Compound::from([(KEY_COLONY.into(), Tag::Int(-4))]);
        assert_eq!(BuildingEntity::from_compound(&negative).colony(), None);
        let byte = Compound::from([(KEY_COLONY.into(), Tag::Byte(9))]);
        assert_eq!(BuildingEntity::from_compound(&byte).colony(), Some(9));
    }

    #[test]
    fn un_bloc_sans_batiment_ne_refuse_personne() {
        let building = BuildingEntity::new();
        for player in [OWNER, OFFICER, FRIEND, STRANGER] {
            assert_eq!(building.access(&Members::new(), player), Ok(()));
            assert!(building.is_usable_by(&Members::new(), player));
        }
    }

    #[test]
    fn un_bloc_rattache_demande_le_bit_acces_aux_huttes() {
        let mut building = BuildingEntity::new();
        building.link(1, MAIN);
        let members = colony();
        assert_eq!(building.access(&members, OWNER), Ok(()));
        assert_eq!(building.access(&members, FRIEND), Ok(()), "Friend porte ACCESS_HUTS");
        // Un officier hérite de Friend par la table de `Permissions.java:173-219`.
        assert_eq!(building.access(&members, OFFICER), Ok(()));
        assert_eq!(
            building.access(&members, STRANGER),
            Err(Refusal::LacksAction {
                rank: Rank::Neutral,
                action: Action::AccessHuts
            })
        );
        assert!(!building.is_usable_by(&members, STRANGER));
    }

    #[test]
    fn le_tick_d_un_bloc_sans_colonie_l_adopte_et_ne_rafraichit_rien() {
        let building = BuildingEntity::new();
        assert_eq!(building.steps(20, false, false), vec![Step::AdoptColony]);
        assert_eq!(building.steps(0, false, false), vec![Step::AdoptColony]);
        // Adopter et rafraîchir s'excluent : même à un tick de seconde, un bloc
        // sans colonie ne fait que l'adopter (`:460-466`).
        assert!(!building.steps(20, false, false).contains(&Step::RefreshSigns));
    }

    #[test]
    fn les_panneaux_se_rafraichissent_une_fois_par_seconde() {
        let mut building = BuildingEntity::new();
        building.link(1, MAIN);
        assert_eq!(building.steps(20, false, false), vec![Step::RefreshSigns, Step::ResolveColony]);
        assert_eq!(building.steps(19, false, false), vec![Step::ResolveColony]);
        assert_eq!(building.steps(40, false, true).first(), Some(&Step::RefreshSigns));
        // Une colonie déjà résolue n'est pas re-résolue (`:508`).
        assert_eq!(building.steps(19, false, true), Vec::<Step>::new());
    }

    #[test]
    fn un_blueprint_arrive_s_applique_en_plus_et_a_tout_tick() {
        let mut building = BuildingEntity::new();
        building.link(1, MAIN);
        assert_eq!(
            building.steps(7, true, false),
            vec![Step::ResolveColony, Step::ApplyBlueprint]
        );
        assert_eq!(
            BuildingEntity::new().steps(7, true, false),
            vec![Step::AdoptColony, Step::ApplyBlueprint]
        );
        assert_eq!(building.steps(7, true, true), vec![Step::ApplyBlueprint]);
    }

    #[test]
    fn un_chemin_de_schema_gagne_son_extension_une_seule_fois() {
        assert_eq!(normalise_path("builder1").as_deref(), Some("builder1.blueprint"));
        assert_eq!(
            normalise_path("builder1.blueprint").as_deref(),
            Some("builder1.blueprint")
        );
        assert_eq!(normalise_path("  builder1  ").as_deref(), Some("builder1.blueprint"));
        assert_eq!(normalise_path(""), None);
        assert_eq!(normalise_path("null1"), None, "Java teste `contains(\"null\")`");
    }

    #[test]
    fn le_chemin_d_une_hutte_n_est_normalise_qu_a_la_lecture() {
        // `setBlueprintPath` de la hutte affecte sans condition
        // (`TileEntityColonyBuilding.java:588-593`) ; c'est `load` qui ajoute
        // l'extension (`:422-426`). Le contrôleur de décoration, lui, normalise
        // dans son poseur (`TileEntityDecorationController.java:206-213`) : la
        // différence entre les deux classes est reproduite, pas lissée.
        let mut building = BuildingEntity::new();
        building.set_path("builder1");
        assert_eq!(building.path(), "builder1");
        assert_eq!(normalise_path(building.path()).as_deref(), Some("builder1.blueprint"));
    }
}
