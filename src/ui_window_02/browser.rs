//! `WindowBuildingBrowser` : le catalogue de styles d'une hutte, et son état de lecture.
//!
//! La fenêtre ne demande **rien** au serveur : elle balaie les packs installés
//! (`StructurePacks.getPackMetas()`, `WindowBuildingBrowser.java:247`), ouvre chaque
//! `.blueprint` (`:303`), le classe par bloc (`:334-350`) et cache le résultat par bloc
//! (`:219-228`). **Zéro bouton** : elle n'appelle jamais `registerButton`.
//!
//! Ce qui reste ici est ce qui se tient sans hôte : la barre (`:135-139`), les niveaux
//! (`:199-214`), le nom de fichier (`:366-374`), le regroupement (`:385-405`), la
//! classification (`:340-349`) et l'invisibilité (`:408-429`).

use crate::blueprint::Size;
use crate::ui::Value;

use super::pane::{Action, Pane, Source};
use super::Window;

/// L'id de la fenêtre dans cette table.
pub const ID: &str = "browser";
/// La classe qui la construit.
pub const JAVA: &str = "WindowBuildingBrowser";
/// La ressource (`:72`).
pub const RESOURCE: &str = "minecolonies:gui/windowbrowsebuilding.xml";

/// Le nom du bloc en haut (`:10`).
pub const CONSTRUCTION_NAME: &str = "constructionName";
/// Le mot d'attente (`:12`).
pub const LOADING: &str = "loading";
/// La barre (`:14`).
pub const PROGRESS: &str = "progress";
/// La liste (`:16`).
pub const BUILDINGS: &str = "buildings";
/// Le pack, une ligne (`:18`).
pub const PACK_NAME: &str = "packName";
/// Le chemin du blueprint, une ligne (`:19`).
pub const BUILDING_NAME: &str = "buildingName";
/// « 9 x 5 x 11 », une ligne (`:20`).
pub const BUILDING_SIZE: &str = "buildingSize";
/// Les niveaux, une ligne (`:21`).
pub const BUILDING_LEVEL: &str = "buildingLevel";
/// Le libellé du mot d'attente (`windowbrowsebuilding.xml:13`).
pub const LOADING_KEY: &str = "com.minecolonies.command.gui.browsebuilding.loading";
/// La marque d'invisibilité de l'ancre (`structurize …/Constants.java:22`, lue à `:422`).
pub const INVISIBLE_TAG: &str = "invisible";

const F_NAME: Pane = Pane::new(CONSTRUCTION_NAME, Value::Name, Source::Block);
const F_LOADING: Pane = Pane::new(LOADING, Value::Text, Source::Window);
const F_PROGRESS: Pane = Pane::new(PROGRESS, Value::Progress, Source::Packs);
const F_BUILDINGS: Pane = Pane::new(BUILDINGS, Value::Row, Source::Packs);
const F_PACK: Pane = Pane::in_row(BUILDINGS, PACK_NAME, Value::Text, Source::Packs);
const F_PATH: Pane = Pane::in_row(BUILDINGS, BUILDING_NAME, Value::Text, Source::Blueprint);
const F_SIZE: Pane = Pane::in_row(BUILDINGS, BUILDING_SIZE, Value::Text, Source::Blueprint);
const F_LEVELS: Pane = Pane::in_row(BUILDINGS, BUILDING_LEVEL, Value::Text, Source::Blueprint);

/// Les huit panneaux (`windowbrowsebuilding.xml:10-21`).
pub const PANES: &[Pane] =
    &[F_NAME, F_LOADING, F_PROGRESS, F_BUILDINGS, F_PACK, F_PATH, F_SIZE, F_LEVELS];
/// Aucune liste déroulante.
pub const DROPDOWNS: &[&str] = &[];
/// **Aucun** bouton : la fenêtre n'en enregistre pas un seul (`:63-141`).
pub const ACTIONS: &[Action] = &[];

/// La fenêtre, telle que `BuildingDataManager.java:100` l'ouvre.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    layout: None,
    panes: PANES,
    dropdowns: DROPDOWNS,
    actions: ACTIONS,
};

/// Une ligne : un bâtiment d'un pack, tous ses niveaux confondus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// `StructurePackMeta.getName()` (`:180`).
    pub pack: String,
    /// Le chemin **dans** le pack, sans le numéro de niveau (`:374`) ; c'est la clé de
    /// regroupement (`:388`).
    pub path: String,
    /// Les niveaux trouvés, croissants et sans doublon (`TreeSet`, `:368`).
    pub levels: Vec<i32>,
    /// La taille du plus grand blueprint du groupe (`:400-403`).
    pub size: Size,
    /// Vrai quand le bloc cherché est dans la palette et pas l'ancre (`:340-349`).
    pub parent: bool,
    /// Vrai quand l'ancre est marquée invisible (`:408-429`).
    pub invisible: bool,
}

/// La largeur de la barre (`:135-139`).
///
/// `total` à zéro n'arrive qu'entre l'ouverture et le premier pack — Java vaut alors
/// `Math.round(NaN)`, que le transtypage rend `0` : le port répond la même chose.
pub fn progress_width(current: i32, total: i32, full: i32) -> i32 {
    if total <= 0 {
        return 0;
    }
    ((f64::from(current) / f64::from(total)) * f64::from(full)).round() as i32
}

/// Les niveaux d'une ligne (`:199-214`) : `"3"`, `"1-3"` s'ils se suivent, `"1,3"` sinon.
/// `None` quand il n'y en a aucun — le libellé est alors **caché** (`:186-189`).
pub fn format_levels(levels: &[i32]) -> Option<String> {
    let mut sorted = levels.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let (first, last) = (*sorted.first()?, *sorted.last()?);
    if sorted.len() == 1 {
        return Some(first.to_string());
    }
    if sorted.len() as i32 == last - first + 1 {
        return Some(format!("{first}-{last}"));
    }
    Some(sorted.iter().map(i32::to_string).collect::<Vec<_>>().join(","))
}

/// Le nom de fichier d'un blueprint, coupé de son niveau (`:366-373`).
///
/// **Le niveau est un caractère** : `Integer.parseInt` d'un `substring` d'un caractère,
/// donc `builder10.blueprint` donne le chemin `builder1` et le niveau `0`. Limite du
/// source, que le port reproduit au lieu de la corriger en silence.
pub fn path_segment(file_name: &str) -> (String, Option<i32>) {
    let name = file_name.replace(".blueprint", "");
    let mut characters: Vec<char> = name.chars().collect();
    match characters.last().copied().filter(char::is_ascii_digit) {
        Some(digit) => {
            characters.pop();
            (characters.iter().collect(), digit.to_digit(10).map(|level| level as i32))
        }
        None => (name, None),
    }
}

/// Comment un blueprint se classe pour un bloc (`:334-350`) : `Some(false)` si le bloc est
/// l'ancre, `Some(true)` s'il est dans la palette (un bâtiment **parent**), `None` sinon.
pub fn classify(anchor_is_block: bool, palette_has_block: bool) -> Option<bool> {
    if anchor_is_block {
        Some(false)
    } else if palette_has_block {
        Some(true)
    } else {
        None
    }
}

/// Regroupe les lignes d'un même chemin (`:385-405`), **par pack** comme Java (`:321`).
///
/// La taille retenue est le **maximum lexicographique** de `BlockPos`, l'ordre de
/// `Comparator.naturalOrder()` (`:400-403`) : `[3,2,2]` gagne contre `[2,9,9]`.
pub fn flatten_levels(rows: Vec<Row>) -> Vec<Row> {
    let mut kept: Vec<Row> = Vec::new();
    for row in rows {
        match kept.iter_mut().find(|existing| existing.path == row.path) {
            Some(existing) => {
                for level in row.levels {
                    if !existing.levels.contains(&level) {
                        existing.levels.push(level);
                    }
                }
                existing.levels.sort_unstable();
                if (row.size.x, row.size.y, row.size.z)
                    > (existing.size.x, existing.size.y, existing.size.z)
                {
                    existing.size = row.size;
                }
            }
            None => kept.push(row),
        }
    }
    kept
}

/// Vrai quand l'ancre est invisible (`:408-429`) : son bloc le déclare (`:411-415`), ou ses
/// données portent [`INVISIBLE_TAG`] à `BlockPos.ZERO` (`:418-426`).
pub fn blueprint_is_invisible(anchor_hidden: bool, anchor_tags: &[&str]) -> bool {
    anchor_hidden || anchor_tags.contains(&INVISIBLE_TAG)
}

/// Les lignes montrées : les invisibles ne le sont qu'en créatif (`:148-149`).
pub fn rows_for(rows: &[Row], creative: bool) -> Vec<Row> {
    if creative {
        return rows.to_vec();
    }
    rows.iter().filter(|row| !row.invisible).cloned().collect()
}

/// L'état de la lecture : deux compteurs et un drapeau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scan {
    /// Les packs déjà lus (`:63`, `:322`).
    pub current: i32,
    /// Le diviseur : **100** à l'ouverture (`:100`), puis le nombre de packs (`:247`).
    pub total: i32,
    /// Vrai quand la liste est construite (`:120-131`).
    pub done: bool,
}

impl Scan {
    /// Juste après `onOpened` (`:99-100`).
    pub fn opened() -> Scan {
        Scan { current: 0, total: 100, done: false }
    }

    /// La lecture démarre : `totalProgress` devient le nombre de packs (`:247`).
    pub fn packs(self, count: usize) -> Scan {
        Scan { total: count as i32, ..self }
    }

    /// Un pack lu (`:322`).
    pub fn pack_done(self) -> Scan {
        Scan { current: self.current + 1, ..self }
    }

    /// La lecture est finie : `loading` et `progress` sont cachés (`:145-146`).
    pub fn finished(self) -> Scan {
        Scan { done: true, ..self }
    }

    /// La largeur de la barre pour cet état (`:135-139`).
    pub fn width(self, full: i32) -> i32 {
        progress_width(self.current, self.total, full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, levels: &[i32], size: (u16, u16, u16)) -> Row {
        Row {
            pack: "minecolonies".to_owned(),
            path: path.to_owned(),
            levels: levels.to_vec(),
            size: Size { x: size.0, y: size.1, z: size.2 },
            parent: false,
            invisible: false,
        }
    }

    #[test]
    fn the_progress_bar_rounds_and_never_divides_by_zero() {
        assert_eq!(progress_width(0, 4, 100), 0);
        assert_eq!(progress_width(1, 4, 100), 25);
        assert_eq!(progress_width(1, 3, 100), 33);
        // Java arrondit ici, la réserve du builder tronque (`src/ui::percent`) : deux
        // pourcentages, deux règles.
        assert_eq!(progress_width(2, 3, 100), 67);
        assert_eq!(crate::ui::percent(2, 3), 66);
        assert_eq!(progress_width(1, 0, 100), 0, "Math.round(NaN) vaut 0");
        assert!(LOADING_KEY.starts_with("com.minecolonies.command.gui."));
    }

    #[test]
    fn levels_print_as_one_number_a_range_or_a_list() {
        assert_eq!(format_levels(&[3]), Some("3".to_owned()));
        assert_eq!(format_levels(&[1, 2, 3]), Some("1-3".to_owned()));
        assert_eq!(format_levels(&[3, 1, 2]), Some("1-3".to_owned()), "TreeSet, :368");
        assert_eq!(format_levels(&[1, 3]), Some("1,3".to_owned()));
        // 4 niveaux pour un intervalle de 5 : ce n'est pas une suite.
        assert_eq!(format_levels(&[1, 2, 4, 5]), Some("1,2,4,5".to_owned()));
        assert_eq!(format_levels(&[2, 2]), Some("2".to_owned()));
        assert_eq!(format_levels(&[]), None, "le libellé est caché, :186-189");
    }

    #[test]
    fn the_file_name_loses_its_last_digit_and_a_blueprint_is_the_anchor_or_a_parent() {
        assert_eq!(path_segment("builder.blueprint"), ("builder".to_owned(), None));
        assert_eq!(path_segment("builder1.blueprint"), ("builder".to_owned(), Some(1)));
        // Un seul caractère est lu : « builder10 » donne le niveau 0.
        assert_eq!(path_segment("builder10.blueprint"), ("builder1".to_owned(), Some(0)));
        assert_eq!(path_segment("builder-01.blueprint"), ("builder-0".to_owned(), Some(1)));
        assert_eq!(classify(true, true), Some(false), "l'ancre gagne, :340-344");
        assert_eq!(classify(false, true), Some(true), ":345-349");
        assert_eq!(classify(false, false), None, "il n'entre pas dans la liste");
    }

    #[test]
    fn flattening_merges_levels_by_path_and_keeps_the_lexicographic_max_size() {
        let mut first = row("fisherman", &[1], (3, 2, 2));
        first.parent = true;
        let flat = flatten_levels(vec![first, row("fisherman", &[2], (2, 9, 9)), row("builder", &[1], (5, 5, 5))]);
        assert_eq!(flat.len(), 2, "deux chemins, deux lignes");
        assert_eq!(flat[0].levels, [1, 2], ":397-399");
        // `Comparator.naturalOrder()` sur BlockPos compare x puis y puis z.
        assert_eq!(flat[0].size, Size { x: 3, y: 2, z: 2 });
        assert!(flat[0].parent, "les drapeaux viennent de la première ligne, :404");
    }

    #[test]
    fn an_invisible_blueprint_is_hidden_outside_creative() {
        assert!(blueprint_is_invisible(true, &[]), ":411-415");
        assert!(blueprint_is_invisible(false, &[INVISIBLE_TAG]), ":418-426");
        assert!(!blueprint_is_invisible(false, &["visible"]));
        let mut hidden = row("fisherman1", &[1], (1, 1, 1));
        hidden.invisible = true;
        let rows = [hidden, row("builder1", &[1], (1, 1, 1))];
        assert_eq!(rows_for(&rows, false).len(), 1, ":148-149");
        assert_eq!(rows_for(&rows, false)[0].path, "builder1");
        assert_eq!(rows_for(&rows, true).len(), 2, "le créatif voit tout");
    }

    #[test]
    fn the_scan_starts_at_one_hundred_then_counts_packs() {
        let opened = Scan::opened();
        assert_eq!(opened, Scan { current: 0, total: 100, done: false }, ":99-100");
        let started = opened.packs(4);
        assert_eq!(started.total, 4, ":247");
        assert_eq!(started.width(100), 0);
        let half = started.pack_done().pack_done();
        assert_eq!(half.width(100), 50);
        let done = half.pack_done().pack_done().finished();
        assert_eq!(done.width(158), 158, ":137-139");
        assert!(done.done, "loading et progress sont cachés, :145-146");
    }
}
