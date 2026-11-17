//! Le panneau de colonie : il pointe vers une colonie et compte les blocs qui
//! l'en séparent, le long d'une chaîne de panneaux.
//!
//! Référence Java : `core/tileentities/TileEntityColonySign.java` (400 lignes).
//!
//! **Onze clés sauvées** (`:132-148`), dont six caches : le nom de la colonie,
//! celui de la colonie visée, la distance aux deux, l'identifiant du panneau
//! au-dessus, et les deux voisins dans la chaîne (`prevpos`, `nextpos`). Rien
//! n'est calculé à la lecture : tout est recalculé au tick, puis sauvé.
//!
//! **Un tick par seconde, décalé par panneau.** La condition de Java est
//! `(getGameTime() + tickOffset) % TICKS_PER_SECOND * 60 == 0` (`:177`) : `%`
//! passe avant `*`, donc c'est `((t) % 20) × 60 == 0`, c'est-à-dire
//! **`t % 20 == 0`** — le `× 60` ne change jamais le résultat. Le décalage
//! `tickOffset` (`:89`) fait que deux panneaux voisins ne travaillent pas au
//! même tick. [`due`] garde l'expression écrite telle quelle, et un test prouve
//! qu'elle vaut `t % 20 == 0`.
//!
//! **Une distance qui se cumule le long de la chaîne.** La distance d'un panneau
//! est `dist(parent, moi) + distance du parent` (`:208-217`) : c'est la longueur
//! du chemin, pas la distance à vol d'oiseau, et deux panneaux séparés de mille
//! blocs peuvent annoncer la même distance s'ils sont sur la même chaîne. Le
//! port le calcule en [`chained`], à partir de la distance du voisin que
//! l'appelant a lue.
//!
//! **Une rotation géographique.** `atan2(-dz, dx)` puis `(degrés + 360) % 360`
//! (`:281-284`) : l'est vaut 0°, le nord 90°, l'ouest 180°, le sud 270°. C'est
//! la convention de l'écran de MineColonies, pas celle de Minecraft, et le port
//! la reprend sans la traduire.

use mod_api::{Compound, Tag};

use super::nbt;
use crate::base::Pos;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:46`).
pub const TYPE_ID: &str = "minecolonies:colonysign";

/// Les clés sauvées (`:104-127`, `:134-147`).
pub const KEY_COLONY: &str = "colony";
pub const KEY_NAME: &str = "name";
pub const KEY_ANCHOR: &str = "pos";
pub const KEY_ROTATION: &str = "rotation";
pub const KEY_TARGET_COLONY: &str = "targetcolony";
pub const KEY_TARGET_NAME: &str = "targetname";
pub const KEY_DISTANCE: &str = "distance";
pub const KEY_TARGET_DISTANCE: &str = "targetdistance";
pub const KEY_CACHED_ABOVE: &str = "cachedabove";
pub const KEY_PREVIOUS: &str = "prevpos";
pub const KEY_NEXT: &str = "nextpos";

/// La valeur « pas de colonie » des deux identifiants (`:29`, `:34`).
pub const NO_COLONY: i32 = -1;

/// Le tick de travail est-il dû ? L'expression de Java (`:177`), recopiée telle
/// quelle : `%` avant `*`, donc `((gameTime + offset) % 20) × 60 == 0`.
pub fn due(game_time: u64, tick_offset: u8) -> bool {
    let modulo = (game_time + u64::from(tick_offset)) % u64::from(super::TICKS_PER_SECOND);
    modulo * 60 == 0
}

/// La distance que Java écrit, tronquée : `(int) BlockPosUtil.dist(a, b)`
/// (`api/util/BlockPosUtil.java:563-569`, racine de la somme des carrés).
pub fn distance(from: Pos, to: Pos) -> i32 {
    let dx = f64::from(from[0] - to[0]);
    let dy = f64::from(from[1] - to[1]);
    let dz = f64::from(from[2] - to[2]);
    (dx * dx + dy * dy + dz * dz).sqrt() as i32
}

/// La distance cumulée d'un panneau (`:208-217`) : la sienne plus celle de son
/// parent. L'appelant passe `0` quand le bloc parent n'est pas un panneau.
pub fn chained(previous: Pos, at: Pos, previous_distance: i32) -> i32 {
    distance(previous, at).saturating_add(previous_distance)
}

/// L'angle du panneau vers `towards`, en degrés de 0 à 360 (`:279-285`) : est =
/// 0, nord = 90, ouest = 180, sud = 270.
pub fn rotation_towards(at: Pos, towards: Pos) -> f32 {
    let dx = f64::from(towards[0]) + 0.5 - (f64::from(at[0]) + 0.5);
    let dz = f64::from(towards[2]) + 0.5 - (f64::from(at[2]) + 0.5);
    let angle = (-dz).atan2(dx).to_degrees();
    ((angle + 360.0) % 360.0) as f32
}

/// Ce que le monde dit à un panneau pendant son tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : les six lectures du corps de `tick`
pub struct Neighbourhood {
    /// L'identifiant de colonie du panneau au-dessus, quand c'en est un
    /// (`:180-191`).
    pub above: Option<i32>,
    /// Le nom de sa colonie, quand elle est chargée (`:193-200`).
    pub colony_name: Option<String>,
    /// Le nœud de connexion de la colonie (`:201-206`) : ses deux voisins et sa
    /// cible.
    pub node: Option<Node>,
}

/// Le nœud de connexion d'un panneau (`:201-270`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : les cinq faits que le nœud porte
pub struct Node {
    /// Le voisin vers la colonie (`:206`).
    pub previous: Option<Pos>,
    /// La distance déjà comptée par ce voisin, `0` si ce n'est pas un panneau
    /// (`:211-216`).
    pub previous_distance: i32,
    /// Le voisin vers la colonie visée (`:231`).
    pub next: Option<Pos>,
    /// La distance déjà comptée par ce voisin vers la cible (`:247-251`).
    pub next_target_distance: i32,
    /// La colonie visée, si le panneau en vise une (`:237-238`).
    pub target: Option<Target>,
}

/// La colonie visée par un panneau.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : l'identifiant et le nom de la cible
pub struct Target {
    pub colony: i32,
    pub name: String,
}

/// Ce qu'un tick a changé, dans l'ordre de Java : chaque `true` est un
/// `setChanged()` que Java appelle (`:184`, `:198`, `:218`, `:226`, `:233`,
/// `:254`, `:262`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : les sept écritures marquées du corps de `tick`
pub struct Changed {
    pub above: bool,
    pub colony_name: bool,
    pub distance: bool,
    pub previous: bool,
    pub next: bool,
    pub target_distance: bool,
    pub target_name: bool,
}

impl Changed {
    pub fn any(&self) -> bool {
        self.above
            || self.colony_name
            || self.distance
            || self.previous
            || self.next
            || self.target_distance
            || self.target_name
    }
}

/// L'état d'un panneau de colonie. Pas d'`Eq` : la rotation est un `f32`, comme
/// chez Java.
#[derive(Clone, Debug, PartialEq)]
// exhaustif : les onze clés sauvées de la classe Java
pub struct ColonySign {
    colony: Option<i32>,
    target: Option<i32>,
    anchor: Option<Pos>,
    colony_name: String,
    target_name: String,
    rotation: f32,
    distance: i32,
    target_distance: i32,
    cached_above: Option<i32>,
    previous: Option<Pos>,
    next: Option<Pos>,
    tick_offset: u8,
}

impl Default for ColonySign {
    fn default() -> ColonySign {
        ColonySign {
            colony: None,
            target: None,
            anchor: None,
            colony_name: String::new(),
            target_name: String::new(),
            rotation: 0.0,
            distance: 0,
            target_distance: 0,
            cached_above: None,
            previous: None,
            next: None,
            tick_offset: 0,
        }
    }
}

impl ColonySign {
    /// Un panneau posé : aucun lien, aucun nom, rotation nulle (`:29-84`).
    /// `tick_offset` est le tirage de `:89`, que Java garde dans un champ final
    /// et ne sauve pas.
    pub fn new(tick_offset: u8) -> ColonySign {
        ColonySign::default().with_tick_offset(tick_offset)
    }

    /// Le décalage de tick, quand le panneau vient d'un chargement : Java le
    /// tire au constructeur et ne le sauve pas (`:69`, `:89`), donc un panneau
    /// relu doit le recevoir à nouveau.
    pub fn with_tick_offset(mut self, tick_offset: u8) -> ColonySign {
        self.tick_offset = tick_offset % super::TICKS_PER_SECOND as u8;
        self
    }

    pub fn tick_offset(&self) -> u8 {
        self.tick_offset
    }

    /// `load` (`:102-129`). Un `-1` écrit par Java est « pas de lien », et un
    /// `pos` absent est `BlockPos.ZERO`, donc pas d'ancre.
    pub fn from_compound(data: &Compound) -> ColonySign {
        let colony = nbt::int(data, KEY_COLONY).filter(|id| *id != NO_COLONY && *id > 0);
        let target = nbt::int(data, KEY_TARGET_COLONY).filter(|id| *id != NO_COLONY && *id > 0);
        ColonySign {
            colony,
            target,
            anchor: nbt::position_in(data, KEY_ANCHOR),
            colony_name: nbt::text(data, KEY_NAME).unwrap_or_default(),
            target_name: nbt::text(data, KEY_TARGET_NAME).unwrap_or_default(),
            rotation: nbt::float(data, KEY_ROTATION).unwrap_or(0.0),
            distance: nbt::int(data, KEY_DISTANCE).unwrap_or(0),
            target_distance: nbt::int(data, KEY_TARGET_DISTANCE).unwrap_or(0),
            cached_above: nbt::int(data, KEY_CACHED_ABOVE).filter(|id| *id != NO_COLONY),
            previous: nbt::position_in(data, KEY_PREVIOUS),
            next: nbt::position_in(data, KEY_NEXT),
            tick_offset: 0,
        }
    }

    /// `saveAdditional` (`:131-149`) : onze clés, les deux voisins toujours
    /// écrits (zéro zéro zéro quand il n'y en a pas).
    pub fn merge(&self) -> Compound {
        Compound::from([
            (KEY_COLONY.into(), Tag::Int(self.colony.unwrap_or(NO_COLONY))),
            (
                KEY_NAME.into(),
                Tag::String(self.colony_name.as_str().into()),
            ),
            (
                KEY_ANCHOR.into(),
                crate::hut::position(self.anchor.unwrap_or([0, 0, 0])),
            ),
            (KEY_ROTATION.into(), nbt::float_tag(self.rotation)),
            (
                KEY_TARGET_COLONY.into(),
                Tag::Int(self.target.unwrap_or(NO_COLONY)),
            ),
            (
                KEY_TARGET_NAME.into(),
                Tag::String(self.target_name.as_str().into()),
            ),
            (KEY_DISTANCE.into(), Tag::Int(self.distance)),
            (KEY_TARGET_DISTANCE.into(), Tag::Int(self.target_distance)),
            (
                KEY_CACHED_ABOVE.into(),
                Tag::Int(self.cached_above.unwrap_or(NO_COLONY)),
            ),
            (
                KEY_PREVIOUS.into(),
                crate::hut::position(self.previous.unwrap_or([0, 0, 0])),
            ),
            (
                KEY_NEXT.into(),
                crate::hut::position(self.next.unwrap_or([0, 0, 0])),
            ),
        ])
    }

    pub fn colony(&self) -> Option<i32> {
        self.colony
    }

    pub fn target_colony(&self) -> Option<i32> {
        self.target
    }

    pub fn anchor(&self) -> Option<Pos> {
        self.anchor
    }

    pub fn colony_name(&self) -> &str {
        &self.colony_name
    }

    pub fn target_colony_name(&self) -> &str {
        &self.target_name
    }

    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    pub fn distance(&self) -> i32 {
        self.distance
    }

    pub fn target_distance(&self) -> i32 {
        self.target_distance
    }

    pub fn cached_above(&self) -> Option<i32> {
        self.cached_above
    }

    pub fn previous(&self) -> Option<Pos> {
        self.previous
    }

    pub fn next(&self) -> Option<Pos> {
        self.next
    }

    /// `setColonyAndAnchor` (`:289-301`) : rattacher le panneau, pointer vers
    /// l'ancre (ou vers le centre de la colonie), et compter depuis le centre.
    pub fn place(&mut self, colony: i32, name: &str, center: Pos, anchor: Option<Pos>, at: Pos) {
        self.colony = Some(colony);
        let anchor = anchor.unwrap_or(center);
        self.anchor = Some(anchor);
        self.distance = distance(center, at);
        self.colony_name = name.to_owned();
        self.rotation = rotation_towards(at, anchor);
    }

    /// Le corps du tick (`:175-272`), avec les lectures du monde fournies.
    ///
    /// Java recalcule la rotation **à chaque tick de travail** dès qu'un nœud
    /// existe (`:206`), sans comparer : elle n'est donc pas dans [`Changed`],
    /// qui ne porte que les écritures que Java marque (`setChanged`).
    ///
    /// Deux finesses de Java sont reproduites : le cache du panneau au-dessus
    /// est mis à `-1` **sans** marquer le bloc quand il n'y a pas de panneau
    /// (`:188-191`), et le voisin précédent n'est écrit que s'il change et
    /// qu'il n'est pas l'origine (`:224-229`).
    pub fn refresh(&mut self, at: Pos, world: &Neighbourhood) -> Changed {
        let mut changed = Changed::default();

        match world.above {
            Some(id) if self.cached_above != Some(id) => {
                self.cached_above = Some(id);
                changed.above = true;
            }
            Some(_) => {}
            None => self.cached_above = None,
        }

        let Some(name) = &world.colony_name else {
            return changed;
        };
        if self.colony_name != *name {
            self.colony_name = name.clone();
            changed.colony_name = true;
        }

        let Some(node) = &world.node else {
            return changed;
        };
        if let Some(previous) = node.previous {
            self.rotation = rotation_towards(at, previous);
            let chained = chained(previous, at, node.previous_distance);
            if chained != self.distance {
                self.distance = chained;
                changed.distance = true;
            }
            if self.previous != Some(previous) {
                self.previous = Some(previous);
                changed.previous = true;
            }
        }
        if self.next != node.next {
            self.next = node.next;
            changed.next = true;
        }
        self.target = node.target.as_ref().map(|target| target.colony);
        if let (Some(next), Some(target)) = (node.next, &node.target) {
            let chained = chained(next, at, node.next_target_distance);
            if chained != self.target_distance {
                self.target_distance = chained;
                changed.target_distance = true;
            }
            if self.target_name != target.name {
                self.target_name = target.name.clone();
                changed.target_name = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_panneau_neuf_ne_pointe_vers_rien() {
        let sign = ColonySign::new(7);
        assert_eq!(sign.colony(), None);
        assert_eq!(sign.target_colony(), None);
        assert_eq!(sign.anchor(), None);
        assert_eq!(sign.colony_name(), "");
        assert_eq!(sign.rotation(), 0.0);
        assert_eq!(sign.merge().len(), 11, "les onze clés de `saveAdditional`");
    }

    #[test]
    fn le_tick_est_du_une_fois_par_seconde_decale_par_panneau() {
        // L'expression de Java : `((t + offset) % 20) * 60 == 0`.
        for offset in 0..20u8 {
            for time in 0..200u64 {
                let java = (time + u64::from(offset)) % 20 * 60 == 0;
                assert_eq!(due(time, offset), java, "t={time} offset={offset}");
            }
        }
        // Et le décalage décale bien : les panneaux 0 et 5 ne tombent pas ensemble.
        assert!(due(20, 0));
        assert!(!due(20, 5));
        assert!(due(25, 15));
    }

    #[test]
    fn l_angle_suit_la_rose_des_vents_de_l_ecran() {
        let at = [0, 64, 0];
        assert!((rotation_towards(at, [4, 64, 0]) - 0.0).abs() < 1e-3, "est = 0");
        assert!((rotation_towards(at, [0, 64, -4]) - 90.0).abs() < 1e-3, "nord = 90");
        assert!((rotation_towards(at, [-4, 64, 0]) - 180.0).abs() < 1e-3, "ouest = 180");
        assert!((rotation_towards(at, [0, 64, 4]) - 270.0).abs() < 1e-3, "sud = 270");
        // Toujours dans 0..360, jamais négatif.
        let angle = rotation_towards(at, [3, 64, 4]);
        assert!((0.0..360.0).contains(&angle));
    }

    #[test]
    fn la_distance_est_euclidienne_et_tronquee() {
        assert_eq!(distance([0, 64, 0], [3, 64, 4]), 5);
        assert_eq!(distance([0, 64, 0], [1, 64, 0]), 1);
        // Une diagonale de 1,41 donne 1 : Java tronque.
        assert_eq!(distance([0, 64, 0], [1, 64, 1]), 1);
        assert_eq!(chained([10, 64, 0], [13, 64, 4], 20), 25);
    }

    #[test]
    fn le_rattachement_pose_l_ancre_la_distance_et_l_angle() {
        let mut sign = ColonySign::new(0);
        sign.place(3, "Auberge", [100, 64, 100], None, [90, 64, 100]);
        assert_eq!(sign.colony(), Some(3));
        assert_eq!(sign.anchor(), Some([100, 64, 100]), "ancre = centre par défaut");
        assert_eq!(sign.distance(), 10);
        assert_eq!(sign.colony_name(), "Auberge");
        assert!((sign.rotation() - 0.0).abs() < 1e-3, "la cible est à l'est");
        // Une ancre explicite l'emporte sur le centre, et la distance reste celle
        // du centre (`:296`).
        sign.place(3, "Auberge", [100, 64, 100], Some([90, 64, 90]), [90, 64, 100]);
        assert_eq!(sign.anchor(), Some([90, 64, 90]));
        assert_eq!(sign.distance(), 10);
        assert!((sign.rotation() - 90.0).abs() < 1e-3);
    }

    #[test]
    fn le_tick_adopte_le_panneau_au_dessus_et_le_nom_de_sa_colonie() {
        let mut sign = ColonySign::new(0);
        let world = Neighbourhood {
            above: Some(5),
            colony_name: Some("Auberge".to_owned()),
            node: None,
        };
        let changed = sign.refresh([0, 64, 0], &world);
        assert!(changed.above && changed.colony_name);
        assert_eq!(sign.cached_above(), Some(5));
        assert_eq!(sign.colony_name(), "Auberge");
        // Rien n'a changé : aucun `setChanged`.
        assert!(!sign.refresh([0, 64, 0], &world).any());
        // Le panneau au-dessus disparaît : le cache retombe à rien, **sans** être
        // marqué (`:188-191`).
        let gone = Neighbourhood {
            above: None,
            colony_name: Some("Auberge".to_owned()),
            node: None,
        };
        let changed = sign.refresh([0, 64, 0], &gone);
        assert!(!changed.above);
        assert_eq!(sign.cached_above(), None);
    }

    #[test]
    fn la_chaine_cumule_les_distances_et_suit_les_deux_voisins() {
        let mut sign = ColonySign::new(0);
        let world = Neighbourhood {
            above: None,
            colony_name: Some("Auberge".to_owned()),
            node: Some(Node {
                previous: Some([3, 64, 4]),
                previous_distance: 20,
                next: Some([10, 64, 0]),
                next_target_distance: 30,
                target: Some(Target {
                    colony: 9,
                    name: "Moulin".to_owned(),
                }),
            }),
        };
        let changed = sign.refresh([0, 64, 0], &world);
        assert!(changed.distance && changed.previous && changed.next);
        assert!(changed.target_distance && changed.target_name);
        assert_eq!(sign.distance(), 25, "5 + la distance du parent");
        assert_eq!(sign.previous(), Some([3, 64, 4]));
        assert_eq!(sign.next(), Some([10, 64, 0]));
        assert_eq!(sign.target_colony(), Some(9));
        assert_eq!(sign.target_distance(), 40);
        assert_eq!(sign.target_colony_name(), "Moulin");
    }

    #[test]
    fn un_panneau_sans_colonie_chargee_ne_touche_a_rien() {
        let mut sign = ColonySign::new(0);
        sign.place(3, "Auberge", [0, 64, 0], None, [0, 64, 0]);
        let world = Neighbourhood {
            above: Some(2),
            colony_name: None,
            node: Some(Node {
                previous: Some([1, 64, 1]),
                previous_distance: 99,
                ..Node::default()
            }),
        };
        let changed = sign.refresh([0, 64, 0], &world);
        assert!(changed.above, "le cache du dessus se met à jour quand même");
        assert!(!changed.distance);
        assert_eq!(sign.distance(), 0, "la chaîne n'est pas lue");
    }

    #[test]
    fn les_onze_cles_font_un_aller_retour() {
        let mut sign = ColonySign::new(11);
        sign.place(3, "Auberge", [100, 64, 100], Some([90, 64, 90]), [90, 64, 100]);
        let node = Node {
            previous: Some([1, 2, 3]),
            previous_distance: 4,
            next: Some([7, 8, 9]),
            next_target_distance: 1,
            target: Some(Target {
                colony: 9,
                name: "Moulin".to_owned(),
            }),
        };
        sign.refresh([90, 64, 100], &Neighbourhood {
            above: Some(4),
            colony_name: Some("Auberge".to_owned()),
            node: Some(node.clone()),
        });
        let read = ColonySign::from_compound(&sign.merge());
        assert_eq!(read.colony(), Some(3));
        assert_eq!(read.target_colony(), Some(9));
        assert_eq!(read.anchor(), Some([90, 64, 90]));
        assert_eq!(read.colony_name(), "Auberge");
        assert_eq!(read.target_colony_name(), "Moulin");
        assert_eq!(read.distance(), sign.distance());
        assert_eq!(read.target_distance(), sign.target_distance());
        assert_eq!(read.cached_above(), Some(4));
        assert_eq!(read.previous(), Some([1, 2, 3]));
        assert_eq!(read.next(), Some([7, 8, 9]));
        // La rotation survit en f32 : on compare les bits.
        assert_eq!(read.rotation().to_bits(), sign.rotation().to_bits());
    }

    #[test]
    fn le_decalage_de_tick_ne_se_sauve_pas_et_se_redonne() {
        let sign = ColonySign::new(13);
        assert_eq!(sign.tick_offset(), 13);
        assert_eq!(sign.clone().with_tick_offset(45).tick_offset(), 5);
        assert_eq!(ColonySign::from_compound(&Compound::new()).tick_offset(), 0);
    }

    #[test]
    fn un_identifiant_moins_un_ou_absent_vaut_aucun_lien() {
        let data = Compound::from([
            (KEY_COLONY.into(), Tag::Int(-1)),
            (KEY_TARGET_COLONY.into(), Tag::Int(-1)),
            (KEY_CACHED_ABOVE.into(), Tag::Int(-1)),
        ]);
        let sign = ColonySign::from_compound(&data);
        assert_eq!(sign.colony(), None);
        assert_eq!(sign.target_colony(), None);
        assert_eq!(sign.cached_above(), None);
        // Et ce qui n'est pas écrit garde sa valeur par défaut.
        assert_eq!(sign.rotation(), 0.0);
        assert_eq!(sign.distance(), 0);
    }
}
