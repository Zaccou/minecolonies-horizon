//! L'analyseur de schema : deux selections, un score, une liste de ressources.
//!
//! Java reference : `core/client/gui/WindowSchematicAnalyzer.java` (394 lignes),
//! `core/util/SchemAnalyzerUtil.java:120-247` (le calcul et le porteur de resultat),
//! `core/items/ItemScanAnalyzer.java:43` (`TEMP_SCAN`) et `:56` (le `static Blueprint`),
//! et les deux XML `assets/minecolonies/gui/analyzer/windowanalyze.xml` /
//! `analyzedisplay.xml`.
//!
//! **Les neuf panneaux existent deux fois.** `windowanalyze.xml:4-10` inclut
//! `analyzedisplay.xml` **deux fois**, une sous la boite `left`, une sous `right` : la
//! fenetre porte donc dix-huit panneaux a valeur pour neuf ids, et un id seul ne dit pas
//! lequel. C'est pourquoi `switchSelectionTo` prend un `Box` en premier argument
//! (`:323-386`) et cherche toujours **depuis** lui (`:334`, `:342`, `:360`).
//!
//! **Le bouton `viewschem` ne clique rien.** C'est un `<button>` (`analyzedisplay.xml:6`)
//! qu'aucun `registerButton` ne cable : la classe ne fait que lui ecrire un texte
//! (`:343`, `:358`). Le clic est donc mort alors que le panneau existe — l'inverse exact du
//! bouton `direct` de [`super::suggest`], cable sans panneau.
//!
//! **`cancel` est declare trois fois** (`windowanalyze.xml:12` et, par les deux inclusions,
//! `analyzedisplay.xml:25` deux fois) pour **un** gestionnaire (`:72`) : n'importe lequel
//! des trois ferme.
//!
//! **Ce qui n'est pas porte** : le cout par bloc. `SchemAnalyzerUtil.getScoreFor`
//! (`:67-115`) multiplie un score de base par huit tags de bloc (`BlockTags.LOGS`,
//! `Tags.Blocks.STORAGE_BLOCKS`, `BlockTags.ANVIL`…) et six tags de tier
//! (`ModTags.tier1blocks` … `tier6blocks`) qui n'existent pas dans ce port. Ce fichier
//! porte la **fin** de la formule, celle qui ne depend d'aucun tag (`:190-198`), et prend la
//! somme des couts de blocs en argument ([`complexity`]).

use crate::ui::field::Value;

use super::pane::{Button, Exit, Label, Pane, Press, Source, Window};
use super::refusal::Refusal;

/// L'id de la fenetre dans cette table.
pub const ANALYZER_ID: &str = "analyzer";

/// Les deux boites, et les deux ids que `isLeft` compare
/// (`WindowSchematicAnalyzer.java:40-41`, `:204-216`).
pub const SIDE_LEFT: &str = "left";
pub const SIDE_RIGHT: &str = "right";

/// Le nom du blueprint temporaire qu'un scan ecrit (`ItemScanAnalyzer.java:43`, `:205`).
pub const TEMP_SCAN: &str = "selection.blueprint";

/// La racine de chemin que `switchSelectionTo` retire de l'affichage
/// (`WindowSchematicAnalyzer.java:355`).
pub const BLUEPRINT_ROOT: &str = "blueprints/minecolonies/";

/// Le suffixe que Java retire **toujours**, meme quand il n'y a pas de chemin (`:357`).
pub const BLUEPRINT_SUFFIX: &str = ".blueprint";

/// Le mot que Java ecrit en dur quand une selection est nulle (`:343`).
///
/// `Component.literal("none")` : ce n'est pas une cle, donc pas de traduction, et un
/// joueur non anglophone lit bien `none`.
pub const NONE_LABEL: &str = "none";

/// Le poids d'un batiment contenu, dans le score (`:190`).
pub const BUILDING_WEIGHT: i32 = 500;
/// Le poids d'un bloc different — la meme constante que la bulle du panneau
/// `blockcounts` (`:193`, `:369`).
pub const BLOCK_WEIGHT: i32 = 40;
/// Le diviseur du volume (`:196`).
pub const VOLUME_DIVISOR: f64 = 10.0;

/// Le poids que Java ajoute pour chaque bloc (`SchemAnalyzerUtil.java:190-198`).
pub fn complexity(block_score: f64, buildings: i32, unique_blocks: usize, volume: i32) -> i32 {
    (block_score
        + f64::from(buildings) * f64::from(BUILDING_WEIGHT)
        + unique_blocks as f64 * f64::from(BLOCK_WEIGHT)
        + f64::from(volume) / VOLUME_DIVISOR) as i32
}

/// Une ligne de ressource : un item, un nombre de blocs, une taille de pile.
///
/// `ItemStorage.amount` est le nombre de blocs trouves et la pile porte
/// `max(1, cout du bloc)` (`SchemAnalyzerUtil.java:183-187`) — deux nombres differents, ce
/// qui explique les deux colonnes du panneau et la bulle a trois nombres.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// L'id de l'item (`minecraft:stone`).
    pub item: String,
    /// `ItemStorage.getAmount()` : combien de blocs de cet item.
    pub amount: i32,
    /// La taille de pile (`getItemStack().getCount()`).
    pub count: i32,
}

/// Le resultat d'une analyse, sans le `Blueprint` : ses champs utiles
/// (`SchemAnalyzerUtil.SchematicAnalyzationResult`, `:212-217`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analysis {
    /// `blueprint.getName()` — c'est **la** cle de tri et d'identite (`:294`).
    pub name: String,
    /// `blueprint.getFilePath()`, quand il y en a un (`:352`).
    pub path: Option<String>,
    /// `blueprint.getFileName()`, quand il y en a un (`:352`).
    pub file: Option<String>,
    /// `getSizeX/Y/Z` (`:374`).
    pub size: [i32; 3],
    /// `costScore` (`:214`).
    pub cost_score: i32,
    /// `containedBuildings` (`:216`).
    pub contained_buildings: i32,
    /// `differentBlocks` — l'ordre de ce vecteur est le seul reproductible, voir
    /// [`Analysis::ordered_resources`].
    pub resources: Vec<Resource>,
}

/// Ce que la boite ecrit pour une ressource (`WindowSchematicAnalyzer.java:145-156`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceRow {
    /// Le texte du panneau `resourceCount` : le nombre de **blocs**, pas la pile (`:147`).
    pub count_label: i32,
    /// La bulle du meme panneau : `(taille de pile, taille * blocs)` (`:150-151`).
    pub tooltip: (i32, i32),
    /// La pile que l'icone montre : **une** unite (`:154-156`).
    pub icon_count: i32,
}

impl Analysis {
    /// Le nombre de blocs differents — `differentBlocks.size()` (`:365`).
    pub fn unique_blocks(&self) -> usize {
        self.resources.len()
    }

    /// `sizeX * sizeY * sizeZ`, le volume que Java divise par dix (`:196`).
    pub fn volume(&self) -> i32 {
        self.size[0] * self.size[1] * self.size[2]
    }

    /// Le nom que porte le bouton `viewschem` (`:351-358`).
    ///
    /// Trois regles, dans cet ordre : un chemin et un nom de fichier presents donnent
    /// `chemin sans la racine / dernier segment` ; sinon le nom du blueprint ; et
    /// **toujours** le suffixe `.blueprint` retire, meme quand il n'y a pas de chemin.
    pub fn display_name(&self) -> String {
        let mut name = match (self.path.as_deref(), self.file.as_deref()) {
            (Some(path), Some(file)) => {
                let last = file.rsplit('/').next().unwrap_or(file);
                format!("{}/{}", path.replace(BLUEPRINT_ROOT, ""), last)
            }
            _ => self.name.clone(),
        };
        name = name.replace(BLUEPRINT_SUFFIX, "");
        name
    }

    /// La liste des ressources dans l'ordre de `showResourcesFor` (`:130-131`) :
    /// `amount * count` decroissant.
    ///
    /// Java trie une `ArrayList` construite depuis un `Set`, donc les ex aequo suivent
    /// l'ordre — arbitraire — du `Set` ; ici les ex aequo gardent l'ordre de
    /// [`Analysis::resources`], qui est le seul que l'appelant controle.
    pub fn ordered_resources(&self) -> Vec<&Resource> {
        let mut ordered: Vec<&Resource> = self.resources.iter().collect();
        ordered.sort_by_key(|resource| std::cmp::Reverse(resource.amount * resource.count));
        ordered
    }

    /// La bulle du panneau `blockcounts` (`:368-371`) : `(blocs differents * 40, score)`.
    pub fn score_tooltip(&self) -> (i32, i32) {
        (self.unique_blocks() as i32 * BLOCK_WEIGHT, self.cost_score)
    }

    /// Les quatre lignes de texte de la boite, dans l'ordre de Java (`:360-380`) :
    /// complexite, blocs differents, taille, batiments.
    pub fn texts(&self) -> [(&'static str, String); 4] {
        [
            (COMPLEXITY_KEY, self.cost_score.to_string()),
            (BLOCK_COUNTS_KEY, self.unique_blocks().to_string()),
            (SIZE_KEY, size_text(self.size)),
            (BUILDINGS_KEY, self.contained_buildings.to_string()),
        ]
    }

    /// Une ligne de la liste des ressources (`:145-156`).
    pub fn row(resource: &Resource) -> ResourceRow {
        ResourceRow {
            count_label: resource.amount,
            tooltip: (resource.count, resource.count * resource.amount),
            icon_count: 1,
        }
    }
}

/// La taille telle que Java l'ecrit : `[x y z]` (`:374-375`).
pub fn size_text(size: [i32; 3]) -> String {
    format!("[{} {} {}]", size[0], size[1], size[2])
}

/// Laquelle des deux boites un clic atteint (`WindowSchematicAnalyzer.java:202-219`).
///
/// Java remonte **trois** niveaux : le panneau clique, son parent, le parent de son parent
/// (`:204`, `:208`, `:213`). Un quatrieme niveau n'est jamais regarde. Et quand aucun des
/// trois n'est `left`, `isLeft` rend `false`, donc **la boite de droite** — un panneau hors
/// des deux boites est traite comme celui de droite, ce qui n'arrive en pratique qu'aux
/// boutons de la fenetre elle-meme.
pub fn side_of(chain: &[&str]) -> Side {
    if chain.iter().take(3).any(|id| *id == SIDE_LEFT) {
        Side::Left
    } else {
        Side::Right
    }
}

/// Le cote d'une boite : deux selections independantes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux boites dans le XML, et deux seulement
pub enum Side {
    Left,
    Right,
}

impl Side {
    /// Les deux cotes, dans l'ordre du XML (`windowanalyze.xml:4-10`).
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];

    /// L'id de la boite.
    pub fn id(self) -> &'static str {
        match self {
            Side::Left => SIDE_LEFT,
            Side::Right => SIDE_RIGHT,
        }
    }

    /// L'autre boite.
    pub fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// La portee sous laquelle les neuf panneaux de ce cote vivent.
    pub fn scope(self) -> crate::ui::field::Scope {
        crate::ui::field::Scope::Row(self.id())
    }
}

/// La liste suivante, ou la meme quand on est au bout (`getNextFor`, `:250-264`).
///
/// Trois branches, et la troisieme est celle qu'on oublie : une selection **absente** de la
/// liste (`index >= len`) repart a la **premiere**, et une selection nulle reste nulle.
pub fn next(len: usize, current: Option<usize>) -> Option<usize> {
    let index = current?;
    if index >= len {
        return if len == 0 { Some(index) } else { Some(0) };
    }
    if index + 1 < len {
        Some(index + 1)
    } else {
        Some(index)
    }
}

/// La liste precedente, ou la meme quand on est au debut (`getPrevFor`, `:272-286`).
pub fn prev(len: usize, current: Option<usize>) -> Option<usize> {
    let index = current?;
    if index >= len {
        return if len == 0 { Some(index) } else { Some(0) };
    }
    if index >= 1 {
        Some(index - 1)
    } else {
        Some(index)
    }
}

/// Ce que le constructeur fait de la selection de gauche : `prev(next(x))`
/// (`WindowSchematicAnalyzer.java:113`).
///
/// L'aller-retour ne rend pas `x` : une selection sur le **dernier** element recule d'un
/// cran, parce que `next` bute puis `prev` redescend. C'est le comportement de Java, et il
/// ne se voit qu'a la **deuxieme** ouverture d'une fenetre, la selection etant `static`.
pub fn normalise(len: usize, current: Option<usize>) -> Option<usize> {
    prev(len, next(len, current))
}

/// Trie les analyses par nom, sans la casse (`:291-295`).
pub fn sort(analyses: &mut [Analysis]) {
    analyses.sort_by(|a, b| cmp_ignore_case(&a.name, &b.name));
}

/// `String.compareToIgnoreCase`, en clair.
///
/// Java compare des `char` UTF-16 un a un apres `toLowerCase` ; ceci compare des `char`
/// Rust un a un apres `to_lowercase`. La difference ne peut apparaitre que sur des noms de
/// blueprints qui contiennent des caracteres hors BMP, ce que le format n'autorise pas.
fn cmp_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut left = a.chars().flat_map(char::to_lowercase);
    let mut right = b.chars().flat_map(char::to_lowercase);
    loop {
        match (left.next(), right.next()) {
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => continue,
                other => return other,
            },
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

/// Ce que le constructeur fait quand l'item vient de scanner (`:95-111`).
///
/// Trois temps : l'ancienne analyse nommee [`TEMP_SCAN`] est retiree, la nouvelle est
/// ajoutee, tout est retrie, et l'index de l'analyse temporaire devient la selection de
/// **droite** (`selectedRight`). L'appelant remet ensuite le `static` de l'item a `null`
/// (`:108`), ce que cette fonction ne peut pas faire : elle ne connait pas l'item.
pub fn adopt_scan(analyses: &mut Vec<Analysis>, scan: Option<Analysis>) -> Option<usize> {
    let scan = scan?;
    analyses.retain(|analysis| analysis.name != TEMP_SCAN);
    analyses.push(scan);
    sort(analyses);
    analyses.iter().position(|analysis| analysis.name == TEMP_SCAN)
}

// ---------------------------------------------------------------------------
// Les cles de langue, une par panneau ecrit.
// ---------------------------------------------------------------------------

/// Le libelle du bouton de selection (`analyzedisplay.xml:7`).
pub const SELECT_KEY: &str = "com.minecolonies.coremod.gui.analyzer.select";
/// Le libelle du bouton des ressources — ecrit par le XML puis reecrit a l'identique a
/// chaque changement de selection (`analyzedisplay.xml:14`, `:384`).
pub const SHOW_RES_KEY: &str = "com.ldtteam.structurize.gui.scantool.showres";
/// La ligne du score (`:361`).
pub const COMPLEXITY_KEY: &str = "com.minecolonies.coremod.gui.analyzer.complexity";
/// La ligne des blocs differents (`:365`).
pub const BLOCK_COUNTS_KEY: &str = "com.minecolonies.coremod.gui.analyzer.blockcounts";
/// La bulle des deux panneaux comptes (`:149`, `:369`) : « Score: %s total: %s ».
pub const SCORE_KEY: &str = "com.minecolonies.coremod.gui.analyzer.score";
/// La ligne de taille (`:374`).
pub const SIZE_KEY: &str = "com.minecolonies.coremod.gui.analyzer.size";
/// La ligne des batiments (`:379`).
pub const BUILDINGS_KEY: &str = "com.minecolonies.coremod.gui.analyzer.buildings";

// ---------------------------------------------------------------------------
// Le panneau ecrit mais non cable, et le bouton declare trois fois.
// ---------------------------------------------------------------------------

/// `viewschem` : un `<button>` que personne ne cable (`analyzedisplay.xml:6`).
///
/// La classe lui ecrit un texte (`:343`, `:358`) et ne l'enregistre jamais : le clic ne
/// fait rien. Le triplet est `(id, fichier, ligne)`.
pub const UNWIRED_PANE: (&str, &str, u32) = ("viewschem", "analyzer/analyzedisplay.xml", 6);

/// Combien de panneaux portent l'id `cancel` dans cette fenetre : `windowanalyze.xml:12`,
/// plus `analyzedisplay.xml:25` une fois par inclusion (`windowanalyze.xml:5`, `:9`).
pub const CANCEL_DECLARATIONS: u32 = 3;

// ---------------------------------------------------------------------------
// La fenetre.
// ---------------------------------------------------------------------------

/// Les neuf panneaux d'une boite, ecrits depuis la boite `left` : la boite `right` porte
/// les **memes** ids, et c'est [`Side`] qui dit laquelle on ecrit.
const PANES: &[Pane] = &[
    // `:358` — le nom, ou `none` (`:343`).
    Pane::in_row(SIDE_LEFT, "viewschem", Value::Name, Source::Cache),
    // `:360-362` — « Complexity-score: %d ».
    Pane::in_row(SIDE_LEFT, "score", Value::Count, Source::Cache),
    // `:373-377` — « Size [x y z] », bati de trois entiers du blueprint.
    Pane::in_row(SIDE_LEFT, "size", Value::Text, Source::Blueprint),
    // `:364-366` — « Different blocks : %d ».
    Pane::in_row(SIDE_LEFT, "blockcounts", Value::Count, Source::Cache),
    // `:378-380` — « Buildings: %d ».
    Pane::in_row(SIDE_LEFT, "buildings", Value::Count, Source::Cache),
    // `:382` — la liste, cachee a chaque changement de selection.
    Pane::in_row(SIDE_LEFT, "resources", Value::Row, Source::Cache),
    // `:154-156` — l'icone, avec une seule unite.
    Pane::in_row("resources", "resourceIcon", Value::Icon, Source::Cache),
    // `:153` — le nom de survol de l'item.
    Pane::in_row("resources", "resourceName", Value::Name, Source::Cache),
    // `:147` — le nombre de blocs.
    Pane::in_row("resources", "resourceCount", Value::Count, Source::Cache),
];

const BUTTONS: &[Button] = &[
    // `:72` — ferme. Le meme id est porte par trois panneaux.
    Button {
        id: "cancel",
        label: Label::Image,
        press: Press::CLOSES,
        refusals: &[],
        declared: Some(("analyzer/analyzedisplay.xml", 25)),
    },
    // `:73-83` — ouvre l'outil de construction etendu, qui rend la main a cet ecran par son
    // rappel (`:77`), d'ou un simple `OpensJava`.
    Button {
        id: "selectschematic",
        label: Label::Xml(SELECT_KEY),
        press: Press::new(&[], Exit::OpensJava("WindowExtendedBuildTool")),
        refusals: &[],
        declared: Some(("analyzer/analyzedisplay.xml", 7)),
    },
    // `:85-87` — recule d'une analyse, sans boucler.
    Button {
        id: "selectleft",
        label: Label::Literal("<"),
        press: Press::STAYS,
        refusals: &[],
        declared: Some(("analyzer/analyzedisplay.xml", 4)),
    },
    // `:89-91` — avance d'une analyse, sans boucler.
    Button {
        id: "selectright",
        label: Label::Literal(">"),
        press: Press::STAYS,
        refusals: &[],
        declared: Some(("analyzer/analyzedisplay.xml", 5)),
    },
    // `:93`, `:124-159` — ouvre la liste des ressources du cote clique ; une selection
    // nulle leve (`:129-131`).
    Button {
        id: "showresources",
        label: Label::Written(SHOW_RES_KEY),
        press: Press::STAYS,
        refusals: &[Refusal::NothingSelected],
        declared: Some(("analyzer/analyzedisplay.xml", 14)),
    },
];

/// `WindowSchematicAnalyzer`, ses neuf panneaux par cote et ses cinq boutons.
pub static WINDOW: Window = Window {
    id: ANALYZER_ID,
    java: "WindowSchematicAnalyzer",
    resource: "minecolonies:gui/analyzer/windowanalyze.xml",
    title: None,
    panes: PANES,
    buttons: BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(item: &str, amount: i32, count: i32) -> Resource {
        Resource { item: item.to_owned(), amount, count }
    }

    fn analysis(name: &str, resources: Vec<Resource>) -> Analysis {
        Analysis {
            name: name.to_owned(),
            path: None,
            file: None,
            size: [1, 1, 1],
            cost_score: 0,
            contained_buildings: 0,
            resources,
        }
    }

    #[test]
    fn the_complexity_adds_500_a_building_40_a_block_and_volume_over_ten() {
        // `SchemAnalyzerUtil.java:190-198`, sans le cout des blocs (MANQUANT.md §1).
        assert_eq!(complexity(0.0, 2, 5, 1000), 1000 + 200 + 100);
        assert_eq!(complexity(0.0, 0, 0, 999), 99, "le volume est divise par dix puis tronque");
        // Le cout des blocs entre avant la troncature, pas apres.
        assert_eq!(complexity(0.9, 0, 0, 0), 0);
        assert_eq!(complexity(1.9, 0, 0, 0), 1);
        // Le 40 de la bulle est le meme que celui de la formule (`:193`, `:369`).
        assert_eq!(BLOCK_WEIGHT, 40);
        assert_eq!(BUILDING_WEIGHT, 500);
    }

    #[test]
    fn the_results_are_sorted_by_name_ignoring_case() {
        let mut analyses = vec![
            analysis("Builder", Vec::new()),
            analysis("apple", Vec::new()),
            analysis("Citizen", Vec::new()),
            analysis("apple2", Vec::new()),
        ];
        sort(&mut analyses);
        let names: Vec<&str> = analyses.iter().map(|analysis| analysis.name.as_str()).collect();
        assert_eq!(names, ["apple", "apple2", "Builder", "Citizen"]);
        // Un tri qui tiendrait compte de la casse mettrait `Builder` en tete.
        assert_ne!(names[0], "Builder");
    }

    #[test]
    fn next_stops_at_the_end_and_prev_stops_at_the_start() {
        assert_eq!(next(3, Some(0)), Some(1));
        assert_eq!(next(3, Some(2)), Some(2), "pas de boucle : :258-263");
        assert_eq!(prev(3, Some(2)), Some(1));
        assert_eq!(prev(3, Some(0)), Some(0), "pas de boucle : :280-285");
        // Une selection nulle reste nulle : `getNextFor(null)` rend `null`.
        assert_eq!(next(3, None), None);
        assert_eq!(prev(3, None), None);
    }

    #[test]
    fn a_selection_that_is_not_in_the_list_falls_back_to_the_first_one() {
        // `currentIndex == -1` avec une liste non vide : `:253-256` et `:275-278`.
        assert_eq!(next(3, Some(7)), Some(0));
        assert_eq!(prev(3, Some(7)), Some(0), "meme repli dans les deux sens");
        // Une liste vide garde la selection telle quelle : `:263` et `:285`.
        assert_eq!(next(0, Some(0)), Some(0));
        assert_eq!(prev(0, Some(0)), Some(0));
    }

    #[test]
    fn the_first_open_normalises_with_next_then_prev() {
        // `:113` : `selectedLeft = getPrevFor(getNextFor(selectedLeft))`.
        assert_eq!(normalise(3, None), None, "une fenetre neuve n'a pas de selection");
        assert_eq!(normalise(3, Some(0)), Some(0));
        assert_eq!(normalise(3, Some(1)), Some(1), "au milieu, l'aller-retour ne bouge pas");
        // Le defaut que l'aller-retour fabrique : sur le dernier, il recule d'un cran.
        assert_eq!(normalise(3, Some(2)), Some(1));
        // Et une selection absente est ramenee a la premiere.
        assert_eq!(normalise(3, Some(9)), Some(0));
    }

    #[test]
    fn the_clicked_pane_is_looked_for_three_levels_up_and_no_further() {
        // `isLeft` : le panneau, son parent, le parent de son parent (`:204-216`).
        assert_eq!(side_of(&["showresources", "results", SIDE_LEFT]), Side::Left);
        assert_eq!(side_of(&["showresources", "results", SIDE_RIGHT]), Side::Right);
        assert_eq!(side_of(&["showresources", SIDE_LEFT]), Side::Left);
        // Un quatrieme niveau n'est jamais regarde : Java rend `false`, donc droite.
        assert_eq!(side_of(&["score", "results", "box", SIDE_LEFT]), Side::Right);
        // Hors des deux boites — `cancel`, par exemple — c'est la droite par defaut.
        assert_eq!(side_of(&["cancel"]), Side::Right);
        assert_eq!(Side::ALL, [Side::Left, Side::Right]);
        assert_eq!(Side::Left.other(), Side::Right);
        assert_eq!(Side::Left.id(), SIDE_LEFT);
        assert_eq!(Side::Left.scope(), crate::ui::field::Scope::Row(SIDE_LEFT));
    }

    #[test]
    fn the_displayed_name_drops_the_root_and_always_the_suffix() {
        let mut hut = analysis("hut", Vec::new());
        assert_eq!(hut.display_name(), "hut");
        hut.name = "builder1.blueprint".to_owned();
        assert_eq!(hut.display_name(), "builder1");
        // Avec un chemin et un nom de fichier, Java prefere le chemin (`:351-356`).
        hut.path = Some("blueprints/minecolonies/medieval/oak".to_owned());
        hut.file = Some("builder1.blueprint".to_owned());
        assert_eq!(hut.display_name(), "medieval/oak/builder1");
        // Un nom de fichier qui contient des separateurs : seul le dernier segment reste.
        hut.file = Some("sub/dir/builder1.blueprint".to_owned());
        assert_eq!(hut.display_name(), "medieval/oak/builder1");
        // Le scan temporaire, lui, n'a pas de chemin : `selection.blueprint` -> `selection`.
        let scan = analysis(TEMP_SCAN, Vec::new());
        assert_eq!(scan.display_name(), "selection");
    }

    #[test]
    fn the_resource_list_orders_by_amount_times_unit_count_descending() {
        let result = analysis(
            "hut",
            vec![resource("a", 1, 1), resource("b", 2, 3), resource("c", 6, 1), resource("d", 9, 0)],
        );
        // a = 1, b = 6, c = 6, d = 0 : b et c sont ex aequo et gardent leur ordre.
        let order: Vec<&str> =
            result.ordered_resources().iter().map(|resource| resource.item.as_str()).collect();
        assert_eq!(order, ["b", "c", "a", "d"]);
        assert_eq!(result.unique_blocks(), 4);
    }

    #[test]
    fn a_resource_row_counts_blocks_and_shows_a_single_unit_in_the_icon() {
        let row = Analysis::row(&resource("minecraft:stone", 64, 3));
        assert_eq!(row.count_label, 64, ":147 ecrit le nombre de blocs");
        assert_eq!(row.tooltip, (3, 192), ":150-151 ecrit (pile, pile * blocs)");
        assert_eq!(row.icon_count, 1, ":154-156 force une unite");
        assert_eq!(Analysis::row(&resource("x", 0, 0)).tooltip, (0, 0));
    }

    #[test]
    fn the_four_text_lines_and_the_blockcounts_tooltip() {
        let mut result = analysis("hut", vec![resource("a", 1, 1), resource("b", 1, 1)]);
        result.cost_score = 1234;
        result.contained_buildings = 3;
        result.size = [11, 7, 4];
        let texts = result.texts();
        assert_eq!(texts[0], (COMPLEXITY_KEY, "1234".to_owned()));
        assert_eq!(texts[1], (BLOCK_COUNTS_KEY, "2".to_owned()));
        assert_eq!(texts[2], (SIZE_KEY, "[11 7 4]".to_owned()));
        assert_eq!(texts[3], (BUILDINGS_KEY, "3".to_owned()));
        assert_eq!(result.volume(), 308);
        // `:369` : blocs differents * 40, puis le score.
        assert_eq!(result.score_tooltip(), (80, 1234));
        assert_eq!(size_text([0, 0, 0]), "[0 0 0]");
        // La bulle des deux comptes est une seule cle, ecrite deux fois (`:149`, `:369`).
        assert_eq!(SCORE_KEY, "com.minecolonies.coremod.gui.analyzer.score");
    }

    #[test]
    fn a_scanned_blueprint_replaces_the_previous_scan_and_becomes_the_right_selection() {
        let mut analyses = vec![
            analysis("apple", Vec::new()),
            analysis(TEMP_SCAN, Vec::new()),
            analysis("zebra", Vec::new()),
        ];
        let scan = analysis(TEMP_SCAN, Vec::new());
        let index = adopt_scan(&mut analyses, Some(scan));
        // `:97` retire l'ancienne, donc la liste garde trois entrees et non quatre.
        assert_eq!(analyses.len(), 3);
        assert_eq!(analyses.iter().filter(|a| a.name == TEMP_SCAN).count(), 1);
        // L'analyse temporaire est la seule `selection`, et l'index rendu la designe.
        assert_eq!(index, Some(1));
        assert_eq!(analyses[index.unwrap_or_default()].name, TEMP_SCAN);
        // Sans scan, rien ne bouge et aucune selection de droite n'est proposee (`:95`).
        assert_eq!(adopt_scan(&mut analyses, None), None);
        assert_eq!(analyses.len(), 3);
    }

    #[test]
    fn the_window_shows_scores_and_a_name_and_no_level_no_progress_no_stock() {
        assert_eq!(WINDOW.id, ANALYZER_ID);
        assert_eq!(WINDOW.java, "WindowSchematicAnalyzer");
        assert_eq!(WINDOW.resource, "minecolonies:gui/analyzer/windowanalyze.xml");
        assert_eq!(WINDOW.title, None, "aucun panneau de titre dans windowanalyze.xml");
        assert_eq!(WINDOW.panes.len(), 9);
        // Ce que la fenetre montre : un nom (celui d'un blueprint), des comptes, un texte.
        assert!(WINDOW.shows(Value::Name));
        assert!(WINDOW.shows(Value::Count));
        assert!(WINDOW.shows(Value::Text));
        assert!(WINDOW.shows(Value::Icon));
        assert!(WINDOW.shows(Value::Row));
        // Ce qu'elle ne montre pas, et qui est la premisse a corriger : ni niveau, ni
        // progression, ni stock (`src/ui/field.rs` reserve les deux derniers au builder).
        assert!(!WINDOW.shows(Value::Level));
        assert!(!WINDOW.shows(Value::Progress));
        assert!(!WINDOW.shows(Value::Stock));
        assert_eq!(WINDOW.values().len(), 5);
        // Les deux panneaux de la liste vivent sous `resources`, pas sous la boite.
        let icon = WINDOW.pane("resourceIcon").unwrap_or_else(|| panic!("pas d'icone"));
        assert_eq!(icon.scope, crate::ui::field::Scope::Row("resources"));
        let score = WINDOW.pane("score").unwrap_or_else(|| panic!("pas de score"));
        assert_eq!(score.scope, crate::ui::field::Scope::Row(SIDE_LEFT));
        assert_eq!(score.source, Source::Cache);
        let size = WINDOW.pane("size").unwrap_or_else(|| panic!("pas de taille"));
        assert_eq!(size.source, Source::Blueprint);
    }

    #[test]
    fn viewschem_is_a_button_in_the_xml_that_nobody_wires() {
        let (id, file, line) = UNWIRED_PANE;
        assert_eq!(id, "viewschem");
        assert_eq!(file, "analyzer/analyzedisplay.xml");
        assert_eq!(line, 6);
        // Il n'est pas dans les boutons : la classe ne l'enregistre jamais.
        assert_eq!(WINDOW.button(id), None);
        // Et il est bien un panneau, dans la boite : c'est un texte ecrit par la classe.
        let pane = WINDOW.pane(id).unwrap_or_else(|| panic!("viewschem n'est pas un panneau"));
        assert_eq!(pane.value, Value::Name);
        assert_eq!(pane.source, Source::Cache);
        // Les cinq boutons cables ont tous un panneau, sauf aucun ici : analyseur complet.
        assert_eq!(WINDOW.reachable().len(), 5);
        assert_eq!(WINDOW.buttons.len(), 5);
        // Le mot de la selection nulle n'est pas une cle (`Component.literal`, `:343`), et
        // le meme id `cancel` est porte par trois panneaux pour un gestionnaire (`:72`).
        assert_eq!(NONE_LABEL, "none");
        assert_eq!(CANCEL_DECLARATIONS, 3);
    }
}
