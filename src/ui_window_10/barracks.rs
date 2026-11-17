//! La caserne : les points de spawn ennemis, le point courant, et le bouton des espions.
//!
//! Java reference : `core/client/gui/huts/WindowBarracksBuilding.java` (173 lignes),
//! `assets/minecolonies/gui/windowhutbarracks.xml:5-19`,
//! `api/util/BlockPosUtil.java:423-429` (`getDistance2D`) et `:872-937` (`calcDirection`),
//! `api/util/constant/TranslationConstants.java:11-31` et `:187-191` (les clés de distance
//! et de direction), `api/colony/IColonyView.java:286` et `:303`
//! (`getLastSpawnPoints`, `isRaiding`).
//!
//! `src/ui` porte déjà cette fenêtre (`ui::screen::HUT_BARRACKS_ID`) : ses cinq panneaux,
//! ses six boutons, son XML et ses sources. Ce fichier n'en reprend rien — il porte ce
//! que la table statique ne peut pas tenir : **l'état**, les deux conditions de niveau,
//! et les deux calculs que Java fait au moment de remplir les lignes.
//!
//! # Deux portes au même niveau, par deux mécanismes différents
//!
//! Le niveau 3 est exigé deux fois, et Java ne s'y prend pas de la même façon :
//!
//! - le **bouton** des espions et son icône sont cachés dans le **constructeur**
//!   (`:92-96`, `setVisible(false)`), donc avant tout remplissage ;
//! - la **liste** des points n'est simplement pas construite dans `onOpened`
//!   (`:113-119` : le `if` enveloppe tout le remplissage, et `return` sort sans poser
//!   de fournisseur de données).
//!
//! Les deux répondent au même seuil ([`LIST_LEVEL`]) mais l'un est un panneau caché et
//! l'autre une absence de données : [`Barracks::spies_visible`] et [`Barracks::rows`]
//! les séparent au lieu de les confondre.
//!
//! # Trois choses que la fenêtre ne teste pas, et qui surprennent
//!
//! 1. **`areSpiesEnabled()` n'est pas consulté.** L'icône est la variante d'un drapeau de
//!    colonie (`IColonyView.java:305`), mais cette fenêtre ne lit que le niveau : à
//!    partir du niveau 3, le bouton est là même si la colonie n'a pas d'espions.
//! 2. **La garde de `updateElement` est morte.** `getElementCount` rend
//!    `size - (raiding ? 1 : 0)` (`:130`) et `updateElement` refuse
//!    `raiding && index == size - 1` (`:137`) : avec `size - 1` éléments, l'index
//!    maximum est `size - 2`, donc la condition est toujours fausse. Le port ne la
//!    reproduit pas — il n'a pas d'index à protéger — et [`Barracks::rows`] dit
//!    seulement combien de lignes sortent.
//! 3. **La distance et la direction ne sont pas mesurées pareil.**
//!    `getDistance2D` additionne deux valeurs absolues (`:423-429` : `|dx| + |dz|`,
//!    Manhattan) et `calcDirection` passe par un `atan2` (`:899`, euclidien). Le texte
//!    d'une ligne est donc un mélange : un palier de distance en Manhattan et une
//!    direction en angle vrai. [`distance_2d`] et [`direction`] gardent chacune sa
//!    formule.
//!
//! Et une quatrième, plus petite : **le point courant est écrit sans numéro.** Les
//! lignes de la liste commencent par `(index + 1) + ": "` (`:139`), le texte de
//! `current` non (`:123`, `mountDistanceString` seul) — d'où [`SpawnRow::number`] qui
//! vaut `None` pour ce dernier.

use super::refusal::Refusal;

/// La classe Java.
pub const JAVA: &str = "WindowBarracksBuilding";
/// Le XML, chargé à `:86`.
pub const RESOURCE: &str = "minecolonies:gui/windowhutbarracks.xml";

/// La liste des points (`WindowBarracksBuilding.java:33`).
pub const LIST_POSITIONS: &str = "positions";
/// Le panneau d'une ligne (`:38`).
pub const LABEL_POS: &str = "pos";
/// Le panneau du point courant (`:42`).
pub const LABEL_CURRENT: &str = "current";
/// Le bouton des espions (`:47`).
pub const SPIES_BUTTON: &str = "hireSpies";
/// L'icône du bouton des espions (`:52`).
pub const SPIES_ICON: &str = "hireSpiesIcon";

/// Les deux panneaux que le niveau 3 cache **ensemble** (`:94-95`) : un bouton et son
/// icône, pas un bouton.
pub const SPIES_PANES: [&str; 2] = [SPIES_BUTTON, SPIES_ICON];

/// Le niveau à partir duquel la liste et le bouton existent (`:57`,
/// `BUILDING_LEVEL_FOR_LIST`). C'est un littéral `3` dans le test de `:92` et `:113`.
pub const LIST_LEVEL: i32 = 3;

/// En dessous, l'ennemi a spawné « assez près » (`:62`).
pub const QUITE_CLOSE: i64 = 50;
/// En dessous, « assez loin » (`:67`).
pub const QUITE_FAR: i64 = 100;

/// `QUITE_CLOSE_DESC` (`TranslationConstants.java:187`).
pub const QUITE_CLOSE_KEY: &str = "com.minecolonies.coremod.max.schematic.quiteclose";
/// `QUITE_FAR_DESC` (`:189`).
pub const QUITE_FAR_KEY: &str = "com.minecolonies.coremod.max.schematic.quitefar";
/// `REALLY_FAR_DESC` (`:191`).
pub const REALLY_FAR_KEY: &str = "com.minecolonies.coremod.max.schematic.reallyfar";

/// Le palier de distance d'un point de spawn (`:156-167`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of Java's if/else on the distance
pub enum Distance {
    /// `distance < QUITE_CLOSE`.
    QuiteClose,
    /// `QUITE_CLOSE <= distance < QUITE_FAR`.
    QuiteFar,
    /// `distance >= QUITE_FAR`.
    ReallyFar,
}

impl Distance {
    /// La clé de la phrase, telle que Java l'attache à la ligne (`:158`, `:162`, `:166`).
    pub fn key(self) -> &'static str {
        match self {
            Distance::QuiteClose => QUITE_CLOSE_KEY,
            Distance::QuiteFar => QUITE_FAR_KEY,
            Distance::ReallyFar => REALLY_FAR_KEY,
        }
    }
}

/// Les onze réponses de `BlockPosUtil.calcDirection` (`BlockPosUtil.java:942-954`), dans
/// son ordre de déclaration — `WEST` avant `EAST`, comme Java.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: eleven values in Java, and a twelfth would need its two translations
pub enum Direction {
    North,
    South,
    West,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
    Up,
    Down,
    Same,
}

/// `DIRECTION_NORTH` (`TranslationConstants.java:11`).
pub const NORTH_KEY: &str = "com.minecolonies.coremod.gui.direction.north";
/// `DIRECTION_SOUTH` (`:15`).
pub const SOUTH_KEY: &str = "com.minecolonies.coremod.gui.direction.south";
/// `DIRECTION_EAST` (`:19`).
pub const EAST_KEY: &str = "com.minecolonies.coremod.gui.direction.east";
/// `DIRECTION_WEST` (`:23`).
pub const WEST_KEY: &str = "com.minecolonies.coremod.gui.direction.west";
/// `DIRECTION_UP` (`:27`).
pub const UP_KEY: &str = "com.minecolonies.coremod.gui.direction.up";
/// `DIRECTION_DOWN` (`:29`).
pub const DOWN_KEY: &str = "com.minecolonies.coremod.gui.direction.down";
/// `DIRECTION_EXACT` (`:31`).
pub const EXACT_KEY: &str = "com.minecolonies.coremod.gui.direction.exact";

/// Une direction cardinale : **une** clé (`BlockPosUtil.java:944-947`).
const NORTH_KEYS: &[&str] = &[NORTH_KEY];
const SOUTH_KEYS: &[&str] = &[SOUTH_KEY];
const EAST_KEYS: &[&str] = &[EAST_KEY];
const WEST_KEYS: &[&str] = &[WEST_KEY];
/// `NORTH_WEST` : deux clés, jointes par `ComponentUtils.formatList`
/// (`BlockPosUtil.java:948`).
const NORTH_WEST_KEYS: &[&str] = &[NORTH_KEY, WEST_KEY];
/// `NORTH_EAST` (`:949`).
const NORTH_EAST_KEYS: &[&str] = &[NORTH_KEY, EAST_KEY];
/// `SOUTH_WEST` (`:950`).
const SOUTH_WEST_KEYS: &[&str] = &[SOUTH_KEY, WEST_KEY];
/// `SOUTH_EAST` (`:951`).
const SOUTH_EAST_KEYS: &[&str] = &[SOUTH_KEY, EAST_KEY];
/// `UP` (`:952`).
const UP_KEYS: &[&str] = &[UP_KEY];
/// `DOWN` (`:953`).
const DOWN_KEYS: &[&str] = &[DOWN_KEY];
/// `SAME` (`:954`).
const SAME_KEYS: &[&str] = &[EXACT_KEY];

impl Direction {
    /// Les clés que `getLongText()` joint, dans l'ordre — celle que le texte de la ligne
    /// utilise (`:168-171`).
    ///
    /// **Une diagonale en porte deux**, et c'est le fait : Java construit son libellé
    /// avec `ComponentUtils.formatList` sur deux `translatable`
    /// (`BlockPosUtil.java:948-951`). Les rendre comme une seule clé coupée en deux
    /// donnerait un texte faux dans la seconde moitié.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Direction::North => NORTH_KEYS,
            Direction::South => SOUTH_KEYS,
            Direction::East => EAST_KEYS,
            Direction::West => WEST_KEYS,
            Direction::NorthWest => NORTH_WEST_KEYS,
            Direction::NorthEast => NORTH_EAST_KEYS,
            Direction::SouthWest => SOUTH_WEST_KEYS,
            Direction::SouthEast => SOUTH_EAST_KEYS,
            Direction::Up => UP_KEYS,
            Direction::Down => DOWN_KEYS,
            Direction::Same => SAME_KEYS,
        }
    }
}

/// `BlockPosUtil.getDistance2D` (`:423-429`) : la somme des deux écarts absolus, en
/// `long`. **Manhattan, pas euclidien** — l'autre fenêtre de la colonie qui mesure une
/// distance, celle de l'embauche, prend la racine (`ui_window_03::hire`), et les deux
/// nombres ne sont donc pas comparables.
pub fn distance_2d(from: [i32; 3], to: [i32; 3]) -> i64 {
    let x = (from[0] as i64 - to[0] as i64).abs();
    let z = (from[2] as i64 - to[2] as i64).abs();
    (x + z).abs()
}

/// Le palier d'une distance (`:156-167`).
pub fn bucket(distance: i64) -> Distance {
    if distance < QUITE_CLOSE {
        Distance::QuiteClose
    } else if distance < QUITE_FAR {
        Distance::QuiteFar
    } else {
        Distance::ReallyFar
    }
}

/// `BlockPosUtil.calcDirection` (`:872-937`), argument pour argument.
///
/// L'angle est mesuré **du bâtiment vers le point** : `xDiff = building.x - pos.x` et
/// `zDiff = building.z - pos.z` (`:895-896`), puis `atan2(xDiff, zDiff)` — donc depuis
/// l'axe `+Z` vers `+X`, en degrés, ramené dans `[0, 360)` **après** la troncature
/// entière (`:897-901`). Une colonne identique donne `UP`, `DOWN` ou `SAME` avant tout
/// calcul (`:879-893`).
pub fn direction(building: [i32; 3], pos: [i32; 3]) -> Direction {
    if pos[2] == building[2] && pos[0] == building[0] {
        return if pos[1] > building[1] {
            Direction::Up
        } else if pos[1] < building[1] {
            Direction::Down
        } else {
            Direction::Same
        };
    }
    let x_diff = building[0] - pos[0];
    let z_diff = building[2] - pos[2];
    let mut degree =
        ((x_diff as f64).atan2(z_diff as f64) * 180.0 / std::f64::consts::PI) as i32;
    if degree < 0 {
        degree += 360;
    }
    match degree {
        d if d <= 22 || d >= 338 => Direction::North,
        d if d > 22 && d < 67 => Direction::NorthWest,
        d if d >= 67 && d <= 112 => Direction::West,
        d if d > 112 && d < 157 => Direction::SouthWest,
        d if d >= 157 && d <= 202 => Direction::South,
        d if d > 202 && d < 247 => Direction::SouthEast,
        d if d >= 247 && d <= 292 => Direction::East,
        // `:931-934` : le `else` final, qui couvre `(292, 338)`.
        _ => Direction::NorthEast,
    }
}

/// Une ligne de la liste, ou le texte du point courant.
///
/// Le texte que Java rend est **trois morceaux** : un préfixe littéral
/// `(index + 1) + ": "` (`:139`), une phrase de distance (`:158-166`) et une direction
/// longue (`:168-171`). Le port garde les trois séparément plutôt que de fabriquer une
/// phrase : il n'a pas la table de langue, et une phrase inventée serait fausse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnRow {
    /// Le rang affiché, 1-based — `None` pour le point courant, que Java écrit sans
    /// numéro (`:123`).
    pub number: Option<i32>,
    /// Le palier de distance.
    pub distance: Distance,
    /// La direction depuis le bâtiment.
    pub direction: Direction,
}

impl SpawnRow {
    /// Le préfixe littéral de la ligne (`:139`), absent du point courant.
    pub fn prefix(&self) -> Option<String> {
        self.number.map(|number| format!("{number}: "))
    }

    /// Les deux clés de traduction, dans l'ordre où Java les concatène (`:169-171`).
    pub fn keys(&self) -> Vec<&'static str> {
        let mut keys = vec![self.distance.key()];
        keys.extend_from_slice(self.direction.keys());
        keys
    }
}

/// Une ligne de liste pour le point d'index `index` (0-based, `:136-140`).
pub fn spawn_row(index: usize, building: [i32; 3], pos: [i32; 3]) -> SpawnRow {
    SpawnRow {
        number: Some(index as i32 + 1),
        distance: bucket(distance_2d(pos, building)),
        direction: direction(building, pos),
    }
}

/// La caserne ouverte : son niveau, sa position et ce que la colonie lui a déjà appris.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Barracks {
    /// `getBuildingLevel()` (`:92`, `:113`).
    pub level: i32,
    /// `buildingView.getPosition()` (`:154`), l'origine des deux mesures.
    pub at: [i32; 3],
    /// `view.isRaiding()` (`:121`, `:130`).
    pub raiding: bool,
    /// `view.getLastSpawnPoints()` (`:115`), dans l'ordre où la colonie les a spawnés.
    pub spawn_points: Vec<[i32; 3]>,
}

impl Barracks {
    /// L'état que la fenêtre lit à l'ouverture.
    pub fn initial(
        level: i32,
        at: [i32; 3],
        raiding: bool,
        spawn_points: Vec<[i32; 3]>,
    ) -> Barracks {
        Barracks { level, at, raiding, spawn_points }
    }

    /// Le bouton des espions est-il visible — et son icône avec lui (`:92-96`).
    ///
    /// **Le niveau seul** : `areSpiesEnabled()` n'est pas consulté par cette fenêtre.
    pub fn spies_visible(&self) -> bool {
        self.level >= LIST_LEVEL
    }

    /// Les lignes de la liste (`:125-142`), ou le refus qui dit pourquoi il n'y en a pas.
    ///
    /// Sous le niveau 3 Java sort de `onOpened` avant de poser un fournisseur
    /// (`:113-119`) : la liste reste vide, sans message. Quand la colonie est en train
    /// d'être raidée, **le dernier point quitte la liste** et passe dans `current`
    /// (`:121-124`, `:130`).
    pub fn rows(&self) -> Result<Vec<SpawnRow>, Refusal> {
        if !self.spies_visible() {
            return Err(Refusal::SpawnListBelowLevel { level: self.level });
        }
        if self.spawn_points.is_empty() {
            // `:116-119` : Java `return` avant de remplir quoi que ce soit.
            return Ok(Vec::new());
        }
        let hidden = if self.raiding { 1 } else { 0 };
        let count = self.spawn_points.len().saturating_sub(hidden);
        Ok((0..count)
            .map(|index| spawn_row(index, self.at, self.spawn_points[index]))
            .collect())
    }

    /// Le texte du point courant (`:121-124`), quand il y en a un.
    ///
    /// Java ne l'écrit que si la colonie est raidée **et** si la liste n'est pas vide —
    /// le `return` de `:116-119` passe avant. Le texte n'a pas de numéro.
    pub fn current(&self) -> Result<Option<SpawnRow>, Refusal> {
        if !self.spies_visible() {
            return Err(Refusal::SpawnListBelowLevel { level: self.level });
        }
        if !self.raiding || self.spawn_points.is_empty() {
            return Ok(None);
        }
        let last = self.spawn_points.len() - 1;
        Ok(Some(SpawnRow {
            number: None,
            distance: bucket(distance_2d(self.spawn_points[last], self.at)),
            direction: direction(self.at, self.spawn_points[last]),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{screen_by_id, Gate};

    fn barracks(level: i32, raiding: bool, points: &[[i32; 3]]) -> Barracks {
        Barracks::initial(level, [0, 64, 0], raiding, points.to_vec())
    }

    #[test]
    fn the_list_and_the_spies_button_share_the_level_three_gate_but_not_the_mechanism() {
        let low = barracks(2, false, &[[10, 64, 10]]);
        assert!(!low.spies_visible(), ":92-96 cache le bouton avant tout remplissage");
        assert_eq!(
            low.rows(),
            Err(Refusal::SpawnListBelowLevel { level: 2 }),
            ":113-119 ne construit pas la liste"
        );
        assert_eq!(low.current(), Err(Refusal::SpawnListBelowLevel { level: 2 }));

        let high = barracks(3, false, &[[10, 64, 10]]);
        assert!(high.spies_visible(), "le niveau 3 suffit, et areSpiesEnabled n'est pas lu");
        assert_eq!(high.rows().map(|rows| rows.len()), Ok(1));
        // Deux panneaux cachés ensemble, pas un bouton.
        assert_eq!(SPIES_PANES, ["hireSpies", "hireSpiesIcon"]);
        // Et la fenêtre de `src/ui` porte bien ce bouton-là.
        let screen = screen_by_id(crate::ui::screen::HUT_BARRACKS_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert!(screen.action(SPIES_BUTTON).is_some());
    }

    #[test]
    fn a_raid_moves_the_last_point_into_current_and_takes_it_out_of_the_list() {
        let points = [[10, 64, 0], [0, 64, 70], [0, 64, 120]];
        let raided = barracks(3, true, &points);
        let rows = raided.rows().unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(rows.len(), 2, "getElementCount rend size - 1 (:130)");
        assert_eq!(rows[0].number, Some(1));
        assert_eq!(rows[1].number, Some(2));
        assert_eq!(rows[1].distance, Distance::QuiteFar, "70 en Manhattan");
        let current = raided
            .current()
            .unwrap_or_else(|refusal| panic!("{refusal}"))
            .unwrap_or_else(|| panic!("un raid doit avoir un point courant"));
        assert_eq!(current.number, None, ":123 écrit mountDistanceString seul");
        assert_eq!(current.distance, Distance::ReallyFar, "120 >= 100");
        assert_eq!(current.prefix(), None);
        // Sans raid, les trois points sont dans la liste et `current` reste vide.
        let calm = barracks(3, false, &points);
        assert_eq!(calm.rows().map(|rows| rows.len()), Ok(3));
        assert_eq!(calm.current(), Ok(None));
        assert_eq!(
            calms_prefix(&calm),
            vec![Some("1: ".to_owned()), Some("2: ".to_owned()), Some("3: ".to_owned())]
        );
    }

    fn calms_prefix(camp: &Barracks) -> Vec<Option<String>> {
        camp.rows()
            .unwrap_or_else(|refusal| panic!("{refusal}"))
            .iter()
            .map(SpawnRow::prefix)
            .collect()
    }

    #[test]
    fn an_empty_spawn_list_leaves_the_window_with_nothing_to_show() {
        for raiding in [false, true] {
            let quiet = barracks(4, raiding, &[]);
            assert_eq!(quiet.rows(), Ok(Vec::new()), ":116-119 sort avant le fournisseur");
            assert_eq!(quiet.current(), Ok(None), "pas de dernier point à écrire");
        }
        // Un seul point, en raid : la liste se vide et `current` le prend.
        let single = barracks(3, true, &[[0, 64, 60]]);
        assert_eq!(single.rows(), Ok(Vec::new()));
        assert_eq!(
            single.current().unwrap_or_else(|refusal| panic!("{refusal}")).map(|row| row.distance),
            Some(Distance::QuiteFar)
        );
    }

    #[test]
    fn the_distance_is_manhattan_not_euclidean() {
        // |30| + |30| = 60, alors que la racine vaudrait 42,4.
        assert_eq!(distance_2d([0, 0, 0], [30, 0, 30]), 60);
        assert_eq!(bucket(60), Distance::QuiteFar);
        // La même paire en euclidien serait « assez près » : les deux formules divergent.
        let euclid = ((30.0f64 * 30.0 + 30.0 * 30.0).sqrt()) as i64;
        assert_eq!(euclid, 42);
        assert_eq!(bucket(euclid), Distance::QuiteClose);
        assert_ne!(bucket(60), bucket(euclid));
        // Les deux bornes, et l'Y qui ne compte pas.
        assert_eq!(bucket(distance_2d([0, 0, 0], [49, 0, 0])), Distance::QuiteClose);
        assert_eq!(bucket(distance_2d([0, 0, 0], [50, 0, 0])), Distance::QuiteFar);
        assert_eq!(bucket(distance_2d([0, 0, 0], [99, 0, 0])), Distance::QuiteFar);
        assert_eq!(bucket(distance_2d([0, 0, 0], [100, 0, 0])), Distance::ReallyFar);
        assert_eq!(
            distance_2d([0, 0, 0], [10, 400, 10]),
            20,
            "getDistance2D ignore l'ordonnée (:425-428)"
        );
        assert_eq!(bucket(60).key(), QUITE_FAR_KEY);
        assert_eq!(bucket(120).key(), REALLY_FAR_KEY);
        assert_eq!(bucket(1).key(), QUITE_CLOSE_KEY);
    }

    #[test]
    fn the_eight_directions_are_measured_from_the_building() {
        let building = [0, 64, 0];
        // Minecraft : nord = -Z, est = +X. `atan2(xDiff, zDiff)` depuis le bâtiment.
        assert_eq!(direction(building, [0, 64, -10]), Direction::North);
        assert_eq!(direction(building, [10, 64, 0]), Direction::East);
        assert_eq!(direction(building, [0, 64, 10]), Direction::South);
        assert_eq!(direction(building, [-10, 64, 0]), Direction::West);
        assert_eq!(direction(building, [-10, 64, -10]), Direction::NorthWest);
        assert_eq!(direction(building, [10, 64, -10]), Direction::NorthEast);
        assert_eq!(direction(building, [-10, 64, 10]), Direction::SouthWest);
        assert_eq!(direction(building, [10, 64, 10]), Direction::SouthEast);
        // La diagonale porte deux clés, jointes comme `ComponentUtils.formatList`.
        assert_eq!(Direction::NorthWest.keys().len(), 2);
        assert_eq!(Direction::NorthWest.keys()[0], NORTH_KEY);
        assert_eq!(Direction::NorthWest.keys()[1], WEST_KEY);
        assert_eq!(Direction::SouthEast.keys()[1], EAST_KEY);
        assert_eq!(Direction::North.keys().len(), 1);
        assert_eq!(Direction::North.keys()[0], NORTH_KEY);
        // Et le bâtiment lui-même n'a ni nord ni sud.
        assert_eq!(direction(building, building), Direction::Same);
    }

    #[test]
    fn up_down_and_same_need_the_same_column() {
        let building = [0, 64, 0];
        assert_eq!(direction(building, [0, 80, 0]), Direction::Up);
        assert_eq!(direction(building, [0, 40, 0]), Direction::Down);
        assert_eq!(direction(building, [0, 64, 0]), Direction::Same);
        // Une différence de colonne l'emporte sur l'ordonnée.
        assert_eq!(direction(building, [1, 200, 0]), Direction::East);
        assert_eq!(Direction::Up.keys().len(), 1);
        assert_eq!(Direction::Up.keys()[0], UP_KEY);
        assert_eq!(Direction::Same.keys()[0], EXACT_KEY);
        assert_eq!(Direction::Same.keys().len(), 1, "SAME a une seule clé");
        // Les trois partagent une distance de zéro en colonne, et le palier le plus bas.
        assert_eq!(distance_2d(building, [0, 80, 0]), 0);
        assert_eq!(bucket(0), Distance::QuiteClose);
    }

    #[test]
    fn below_level_three_the_rows_are_refused_by_name_and_silently() {
        let low = barracks(0, true, &[[10, 64, 10]]);
        let refusal = low.rows().err().unwrap_or_else(|| panic!("un refus attendu"));
        assert_eq!(refusal, Refusal::SpawnListBelowLevel { level: 0 });
        assert_eq!(refusal.name(), "ui10.refusal.spawn_list_below_level");
        assert_eq!(refusal.key(), None, "Java ne dit rien, il ne remplit pas");
        // Le refus n'est pas celui des espions : deux conditions, deux noms.
        assert_ne!(
            refusal.name(),
            crate::ui::Refusal::SpiesBelowLevel.name(),
            "src/ui nomme le bouton caché, ce module nomme la liste absente"
        );
        // La fenêtre de `src/ui` porte la même gate de niveau, du côté des boutons.
        let screen = screen_by_id(crate::ui::screen::HUT_BARRACKS_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        let spies = screen
            .action(SPIES_BUTTON)
            .unwrap_or_else(|| panic!("pas de bouton espions"));
        assert_eq!(spies.refusals.len(), 1);
        assert_eq!(spies.refusals[0], crate::ui::Refusal::SpiesBelowLevel);
        // Aucune `Gate` sur cet écran : le niveau n'est pas une advancement.
        assert_eq!(screen.gate, None);
        assert!(!matches!(screen.gate, Some(Gate::Advancement(_))));
    }
}
