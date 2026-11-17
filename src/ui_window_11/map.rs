//! La carte de colonie : un zoom, trois panneaux, et sept sortes d'objets posés dessus.
//!
//! Java référence : `core/client/gui/map/WindowColonyMap.java` (754 lignes, dont la
//! moitié en calcul de positions). Ce que la fenêtre **demande** : les cartes de la salle
//! des fêtes (`((BuildingTownHall.View) building).getMapDataList()`, `:249`), la colonie
//! et ses citoyens (`:207`, `:602`), ses bâtiments (`:360`), les joueurs du niveau
//! (`:312`), les requêtes résolues par le joueur (`:704-752`), et la liste des colonies du
//! serveur (`ColonyListMessage`, `:224`). Ce qu'elle **rend** : les tuiles de carte, une
//! icône par bâtiment, un point par citoyen, un point par joueur, une image par colonie
//! connue, une infobulle par objet, et deux textes — `scale` et `warning`.
//!
//! **Ce n'est pas un écran de hutte.** Elle a un bouton `inventory` et un `exit`, pas de
//! titre de bâtiment, et elle s'ouvre depuis trois endroits qui ne sont pas une hutte :
//! l'item « carte de colonie » (`ItemColonyMap.java:119`, `atTownHall = false`), la page
//! principale de la salle des fêtes (`WindowMainPage.java:401`, `atTownHall = true`) et le
//! classement de prestige (`WindowColonyPrestigeRanking.java:39-40`, qui **propage** le
//! drapeau). Ce drapeau n'est pas décoratif : il cache un bouton, et il change ce que fait
//! `exit`.
//!
//! # Ce que Java fait et qui se voit ici
//!
//! 1. **`hasMaps` n'est jamais remis à faux.** `addMaps` le met à vrai dans la boucle
//!    (`:256`) mais ne le réinitialise pas en entrée (`:239-247`) : une fois vrai, il le
//!    reste, même si la liste des cartes se vide. [`has_maps`] recalcule, et
//!    [`has_maps_after_reopen`] reproduit la remontée — le `warning` de Java peut donc
//!    rester caché alors qu'il n'y a plus de carte.
//! 2. **Le zoom commence à zéro.** `currentScale = 0` (`:178`) et les tuiles sont
//!    dimensionnées `512 * currentScale` (`:262`) : la première image les crée à une
//!    taille nulle. Le libellé `scale` n'est écrit qu'au premier changement
//!    (`:337-341`, `:400`).
//! 3. **Le troisième canal de la boîte est du bleu, pas de l'alpha.** `setColor(r, g, 33)`
//!    (`:543`) passe par `Box.setColor(int, int, int)` : une maison qui a de la place
//!    reçoit une bordure (rouge variable, vert inverse, **bleu 33**), et pas une
//!    transparence.
//! 4. **Les infobulles ne sont pas traduites.** « Owner: », « Coordinates: »,
//!    « Citizens: », « : » et le `Inventory` de l'XML sont des littéraux (`:418-420`,
//!    `:475-477`, `:494`), seuls les noms de bâtiment et les deux clés de requêtes
//!    passent par une traduction. [`owner_line`], [`coordinates_line`] et
//!    [`citizens_line`] gardent l'espacement exact, virgule finale comprise.
//! 5. **`updateScale` parcourt la même liste deux fois.** Les deux boucles de `:350-358`
//!    appellent `updateColonyInfoImage` sur le même `colonies` ; la seconde ne peut rien
//!    faire que la première n'ait fait, sauf quand une icône a été créée entre-temps.
//! 6. **`HIGHLIGHT_RANGE` est déclaré et jamais lu** dans ce fichier (`:65`) : la
//!    surbrillance des citoyens passe par `HighlightManager` (`:615`), pas par cette
//!    constante.

use crate::network::views::ColonyInfo;
use crate::ui::action::{Action, Effect};
use crate::ui::field::{Field, Source, Value};

/// L'écran de cette table. **Pas** un id de `crate::ui` : la carte n'est pas une hutte,
/// donc `crate::ui::screen_by_id` la refuse par son nom (`ui.refusal.unknown_screen`).
pub const ID: &str = "map.colony";
/// La classe (`WindowColonyMap.java:63`).
pub const JAVA: &str = "WindowColonyMap";
/// Le XML (`:192`).
pub const RESOURCE: &str = "minecolonies:gui/map/windowcolonymap.xml";
/// La vue zoomable (`windowcolonymap.xml:11`, `:196`).
pub const DRAG: &str = "dragView";
/// Le facteur de zoom, en texte (`:400`).
pub const SCALE: &str = "scale";
/// L'avertissement quand la salle des fêtes n'a aucune carte (`:267`).
pub const WARNING: &str = "warning";
/// Le bouton qui ouvre le classement (`windowcolonymap.xml:4-6`).
pub const PRESTIGE: &str = "prestige";
/// Sa pastille (`windowcolonymap.xml:7-9`).
pub const PRESTIGE_ICON: &str = "prestigeicon";
/// La croix (`windowcolonymap.xml:13`).
pub const EXIT: &str = "exit";
/// Le coffre (`windowcolonymap.xml:15-17`).
pub const INVENTORY: &str = "inventory";

/// `COLONY_DETAIL_SCALE` (`:135`) : au-dessous, les icônes fines disparaissent et les
/// blobs de colonie prennent leur place.
pub const COLONY_DETAIL_SCALE: f64 = 0.3;
/// `HIGHLIGHT_RANGE` (`:65`) — déclaré, jamais lu dans ce fichier (note 6).
pub const HIGHLIGHT_RANGE: i32 = 2;
/// Le côté d'une tuile à l'échelle 1 (`:262`, `512 * currentScale`).
pub const MAP_TILE: i32 = 512;

/// La clé du message qui remplit la liste des colonies (`:224`).
pub const COLONY_LIST_MESSAGE: &str = "ColonyListMessage";
/// La clé du message d'ouverture du coffre (`:232`).
pub const OPEN_INVENTORY_MESSAGE: &str = "OpenInventoryMessage";
/// La clé de l'avertissement de l'XML (`windowcolonymap.xml:18`).
pub const WARNING_KEY: &str = "com.minecolonies.coremod.townhall.map.warning";
/// La clé de l'infobulle du bouton de prestige (`windowcolonymap.xml:6`).
pub const PRESTIGE_TIP_KEY: &str = "com.minecolonies.coremod.gui.townhall.prestige";
/// La clé de l'infobulle du bouton de la carte, chez le classement
/// (`windowcolonyprestigeranking.xml:3-5`).
pub const MAP_TIP_KEY: &str = "item.minecolonies.colonymap";
/// La clé de la ligne « requêtes résolues par le joueur » (`:503`).
pub const RESOLVED_KEY: &str = "com.minecolonies.core.item.colonymap.playerresolvedrequests";
/// La clé du compte de ces requêtes, avec un argument (`:530`).
pub const RESOLVED_COUNT_KEY: &str =
    "com.minecolonies.core.item.colonymap.playerresolvedrequests.count";

/// `Owner:` — sans espace, et c'est Java (`:418`).
pub const OWNER_PREFIX: &str = "Owner:";
/// `Coordinates: ` — avec (`, `:419`).
pub const COORDINATES_PREFIX: &str = "Coordinates: ";
/// `Citizens: ` — avec (`, `:420`).
pub const CITIZENS_PREFIX: &str = "Citizens: ";

/// Le panneau d'avertissement, seul panneau **fixe** de cette fenêtre : il vient de la
/// salle des fêtes (`:267`), la fenêtre ne fait que le montrer ou le cacher.
///
/// Les autres textes de la fenêtre ne sont pas des [`Field`] : `scale` porte le zoom que
/// la fenêtre tient elle-même, et `Source` ne nomme aucun endroit du modèle pour ça — le
/// même manque que `ui_window_02` a comblé en ajoutant `Item` à **sa** source.
pub const FIELDS: &[Field] = &[Field::new(WARNING, Value::Text, Source::Building)];

/// Les quatre boutons, dans l'ordre où Java les enregistre (`:211-222`).
///
/// `exit` est déclaré ici dans sa forme **salle des fêtes** (`Effect::Reopens`,
/// `:213`) ; depuis l'item, le même bouton ferme et ne rouvre rien, ce qu'`Effect` ne
/// sait pas dire ([`exit_effect`], et `ui_window_06/MANQUANT.md` §2).
pub const ACTIONS: &[Action] = &[
    Action::plain(PRESTIGE, PRESTIGE_TIP_KEY, Effect::Opens(crate::ui_window_11::prestige::ID)),
    Action::plain(
        PRESTIGE_ICON,
        PRESTIGE_TIP_KEY,
        Effect::Opens(crate::ui_window_11::prestige::ID),
    ),
    Action::plain(INVENTORY, "Inventory", Effect::Sends(OPEN_INVENTORY_MESSAGE)),
    Action::plain(EXIT, "close", Effect::Reopens),
];

/// Le bouton avec cet id.
pub fn action(id: &str) -> Option<Action> {
    ACTIONS.iter().find(|action| action.id == id).copied()
}

/// Le coffre est-il dessiné ?
///
/// Caché depuis l'item (`:217`), montré depuis la salle des fêtes — et **enregistré dans
/// les deux cas** (`:220`), donc un bouton caché qui répondrait encore si on le touchait.
pub fn inventory_visible(at_town_hall: bool) -> bool {
    at_town_hall
}

// `atTownHall ? building.openGui(false) : close()` (`:211-219`).
/// Ce que fait la croix, ou `None` quand elle **ferme** sans rien rouvrir.
pub fn exit_effect(at_town_hall: bool) -> Option<Effect> {
    if at_town_hall {
        Some(Effect::Reopens)
    } else {
        None
    }
}

/// Le zoom que la fenêtre tient, et l'état de son libellé.
///
/// `f64` n'est pas `Eq` : le zoom compare ses valeurs, il ne les ordonne pas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    scale: f64,
}

impl Zoom {
    /// À l'ouverture : `currentScale = 0` (`:178`).
    pub const fn new() -> Zoom {
        Zoom { scale: 0.0 }
    }

    /// Le facteur courant.
    pub const fn scale(&self) -> f64 {
        self.scale
    }

    /// `if (currentScale != dragView.getScale())` (`:337-341`) : vrai quand le libellé
    /// doit être réécrit — et c'est le seul endroit qui l'écrit.
    pub fn observe(&mut self, observed: f64) -> bool {
        if self.scale == observed {
            return false;
        }
        self.scale = observed;
        true
    }

    /// Le texte du panneau [`SCALE`] (`:400`).
    pub fn label(&self) -> String {
        scale_label(self.scale)
    }

    /// Les icônes fines sont-elles visibles ? (`:365-397`.)
    pub fn detailed(&self) -> bool {
        !(self.scale < COLONY_DETAIL_SCALE)
    }
}

impl Default for Zoom {
    fn default() -> Zoom {
        Zoom::new()
    }
}

/// `new DecimalFormat("##").format(1 / currentScale) + "x"` (`:173`, `:400`).
///
/// Le motif `##` n'a **aucune décimale** : `1/0.3 = 3,33` s'écrit `3x`, et un zoom de 2
/// (`1/2 = 0,5`) s'écrit `1x` — arrondi au pair le plus proche, comme `DecimalFormat`.
/// À zéro, la division donne l'infini et `DecimalFormat` l'écrit `∞`.
pub fn scale_label(scale: f64) -> String {
    let inverse = 1.0 / scale;
    if !inverse.is_finite() {
        return "∞x".to_owned();
    }
    format!("{}x", inverse.round_ties_even() as i64)
}

/// `worldPosToUIPos` (`:682-688`), sur un axe.
///
/// `d = joueur - monde` ; le dénominateur est `max(1, ln(|d| / 1000))`, donc **1** tant
/// que `|d| < 2718` : le logarithme népérien ne compresse la carte qu'au-delà de deux
/// mille sept cents blocs. Le `max` borne aussi le dénominateur par le bas, donc la
/// division ne peut jamais changer le signe de la distance.
pub fn offset(delta: i32, span: i32, scale: f64) -> i32 {
    let distance = (delta as f64).abs();
    let divisor = 1.0_f64.max((distance / 1000.0).ln());
    // `BlockPos.containing` tronque vers le bas, comme `MathHelper.floor`.
    (span as f64 / 2.0 - ((delta * 4) as f64 / divisor) * scale).floor() as i32
}

/// Les deux coordonnées d'un point du monde dans la vue (`:682-688`).
pub fn world_to_ui(delta_x: i32, delta_z: i32, width: i32, height: i32, scale: f64) -> (i32, i32) {
    (offset(delta_x, width, scale), offset(delta_z, height, scale))
}

/// `"map" + centerX + "-" + centerZ` (`:260`).
pub fn tile_id(center_x: i32, center_z: i32) -> String {
    format!("map{center_x}-{center_z}")
}

/// Le côté d'une tuile à cette échelle (`:262`).
pub fn tile_size(scale: f64) -> i32 {
    (MAP_TILE as f64 * scale) as i32
}

/// Une carte de la salle des fêtes, réduite à ce que la fenêtre en lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapData {
    /// `mapData.centerX` (`:259`).
    pub center_x: i32,
    /// `mapData.centerZ` (`:259`).
    pub center_z: i32,
    /// `mapData.scale` (`:251`) — **l'échelle de la carte**, pas le zoom de la vue.
    pub scale: i32,
}

/// Une tuile posée dans la vue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    /// Son id, généré (`:260`).
    pub id: String,
    /// Sa position dans la vue.
    pub at: (i32, i32),
    /// Son côté.
    pub size: i32,
}

/// Les tuiles que `addMaps` crée, et **seulement** les cartes à l'échelle 0.
///
/// `if (mapData.scale != 0) { continue; }` (`:251-254`) : une carte agrandie est ignorée,
/// sans un mot. `delta_x`/`delta_z` sont les coordonnées du joueur (`playerPos`, `:195`,
/// avec `y = 0`).
pub fn tiles(
    maps: &[MapData],
    player: (i32, i32),
    width: i32,
    height: i32,
    scale: f64,
) -> Vec<Tile> {
    let mut tiles = Vec::new();
    for map in maps {
        if map.scale != 0 {
            continue;
        }
        // La tuile est posée en `center - 64`, soit le coin haut-gauche d'une carte 128.
        let at = world_to_ui(
            player.0 - (map.center_x - 64),
            player.1 - (map.center_z - 64),
            width,
            height,
            scale,
        );
        tiles.push(Tile { id: tile_id(map.center_x, map.center_z), at, size: tile_size(scale) });
    }
    tiles
}

/// `hasMaps` tel que `addMaps` le laisse (`:256`, `:267`).
pub fn has_maps(maps: &[MapData]) -> bool {
    maps.iter().any(|map| map.scale == 0)
}

/// `hasMaps` **après un second passage** d'`addMaps` : Java ne le remet jamais à faux
/// (`:239-247`), donc une valeur vraie survit à la disparition des cartes.
pub fn has_maps_after_reopen(previous: bool, maps: &[MapData]) -> bool {
    previous || has_maps(maps)
}

// `Math.max(0, Math.min(1, residents / (double) level))` (`:539`).
/// La fraction d'occupation d'une maison, bornée à `0..=1`.
pub fn pct_full(residents: i32, level: i32) -> f64 {
    if level <= 0 {
        // Java divise par le niveau sans le garder : un niveau nul donnerait NaN ou
        // l'infini, et `min(1, …)` en fait 1.
        return 1.0;
    }
    (residents as f64 / level as f64).clamp(0.0, 1.0)
}

/// La composante rouge (`:92-106`) : de 0 à 255 sur la première moitié, puis 255.
pub fn red_from_range(pct: f64) -> i32 {
    if pct <= 0.5 {
        (255.0 * (pct / 0.5)) as i32
    } else {
        255
    }
}

/// La composante verte (`:115-129`) : 255 puis descend vers 0.
pub fn green_from_range(pct: f64) -> i32 {
    if pct <= 0.5 {
        255
    } else {
        (255.0 * (1.0 - (pct - 0.5) / 0.5)) as i32
    }
}

/// La bordure d'une maison qui a de la place, ou `None`.
///
/// La condition de Java est double — `residents < level` **et** `getRange() == 0`
/// (`:537`) : une maison qui a une portée (une auberge) n'est jamais colorée. Le
/// troisième canal est 33, et c'est du **bleu** (note 3).
pub fn capacity_colour(residents: i32, level: i32, range: i32) -> Option<[i32; 3]> {
    if !(residents < level) || range != 0 {
        return None;
    }
    let pct = pct_full(residents, level);
    Some([red_from_range(pct), green_from_range(pct), 33])
}

/// La taille du blob d'une colonie (`ColonySize.java:15-19`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois paliers dans l'énumération de Java
pub enum ColonySize {
    /// Moins de 25 citoyens (`ColonySize.java:17`).
    Small,
    /// Moins de 75 (`:18`).
    Medium,
    /// Moins de 5000 (`:19`).
    Large,
}

impl ColonySize {
    /// `getSizeByCount` (`ColonySize.java:48-58`).
    ///
    /// **Le repli est `SMALL`, pas `LARGE`** : une colonie de 5000 citoyens et plus
    /// retombe sur le plus petit dessin, parce que la boucle sort sans rien trouver et que
    /// la méthode rend `SMALL`.
    pub fn of(citizen_count: i32) -> ColonySize {
        if citizen_count < 25 {
            ColonySize::Small
        } else if citizen_count < 75 {
            ColonySize::Medium
        } else if citizen_count < 5000 {
            ColonySize::Large
        } else {
            ColonySize::Small
        }
    }

    /// Le XML du blob (`:17-19`).
    pub fn resource(self) -> &'static str {
        match self {
            ColonySize::Small => "minecolonies:gui/map/colonysmall.xml",
            ColonySize::Medium => "minecolonies:gui/map/colonymedium.xml",
            ColonySize::Large => "minecolonies:gui/map/colonylarge.xml",
        }
    }

    /// La borne haute du palier (`:17-19`).
    pub fn max_citizens(self) -> i32 {
        match self {
            ColonySize::Small => 25,
            ColonySize::Medium => 75,
            ColonySize::Large => 5000,
        }
    }
}

/// `Owner:` + le nom, **sans espace** (`:418`).
pub fn owner_line(owner: &str) -> String {
    format!("{OWNER_PREFIX}{owner}")
}

/// `Coordinates: Xx, Zz` — majuscules et virgule finale (`:419`).
pub fn coordinates_line(x: i32, z: i32) -> String {
    format!("{COORDINATES_PREFIX}{x}X, {z}Z")
}

/// `Citizens: n` (`:420`).
pub fn citizens_line(count: i32) -> String {
    format!("{CITIZENS_PREFIX}{count}")
}

/// La ligne d'un bâtiment : son nom traduit, puis `" : "` et son niveau (`:475`).
pub fn building_line(display_name: &str, level: i32) -> String {
    format!("{display_name} : {level}")
}

/// L'icône d'état d'un bâtiment, quand le joueur a des requêtes en cours dessus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    /// Le nombre de requêtes, écrit en suffixe de l'infobulle (`:530`).
    pub count: usize,
    /// La clé de la ligne d'en-tête (`:503`).
    pub key: &'static str,
}

/// L'icône, ou `None` quand il n'y a rien à signaler (`:491-531`).
pub fn resolved_marker(count: usize) -> Option<Marker> {
    if count == 0 {
        return None;
    }
    Some(Marker { count, key: RESOLVED_KEY })
}

/// Le suffixe d'une requête dans l'infobulle : `": n"`, et seulement si `n > 0`
/// (`:521-524`).
pub fn marker_suffix(count: i32) -> Option<String> {
    if count > 0 {
        Some(format!(": {count}"))
    } else {
        None
    }
}

/// Une couche d'objets posés sur la vue, avec l'id que Java lui fabrique.
///
/// **Aucun de ces ids n'est dans un XML** : ils sont calculés à l'exécution
/// (`:260`, `:467`, `:613`, `:414`, `:316`). C'est pourquoi cette table existe à côté de
/// [`FIELDS`] : un [`Field`] est un panneau qu'on retrouve par son id, une décoration est
/// un objet qu'on crée. Les deux disent la même chose — ce que l'écran rend — avec deux
/// formes différentes parce que la source diffère.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoration {
    /// La couche.
    pub layer: Layer,
    /// La forme de son id.
    pub id: &'static str,
    /// Ce qu'elle porte.
    pub value: Value,
    /// D'où vient ce qu'elle porte.
    pub source: Source,
}

/// Les sept couches de la vue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les objets que `WindowColonyMap` empile dans `dragView`
pub enum Layer {
    /// Une tuile de carte (`:258-264`).
    Map,
    /// Un citoyen (`:607-627`).
    Citizen,
    /// Un bâtiment (`:462-471`).
    Building,
    /// Une colonie connue (`:413-416`).
    Colony,
    /// Un joueur du niveau (`:324-328`).
    Player,
    /// Le point du joueur, en salle des fêtes (`:439-449`).
    Center,
    /// L'icône d'état des requêtes résolues (`:493-498`).
    Status,
}

/// Les sept couches, avec l'id que Java leur donne.
pub const DECORATIONS: &[Decoration] = &[
    Decoration { layer: Layer::Map, id: "map<centerX>-<centerZ>", value: Value::Icon, source: Source::Building },
    Decoration { layer: Layer::Citizen, id: "citizen: <citizenId>", value: Value::Icon, source: Source::Citizen },
    Decoration { layer: Layer::Building, id: "<buildingId.toShortString()>", value: Value::Icon, source: Source::Building },
    Decoration { layer: Layer::Colony, id: "<colonyId><owner>", value: Value::Icon, source: Source::Colony },
    Decoration { layer: Layer::Player, id: "<player UUID>", value: Value::Icon, source: Source::Colony },
    Decoration { layer: Layer::Center, id: "(aucun id)", value: Value::Icon, source: Source::Building },
    Decoration { layer: Layer::Status, id: "(aucun id)", value: Value::Icon, source: Source::Request },
];

/// La décoration de cette couche.
pub fn decoration(layer: Layer) -> Option<Decoration> {
    DECORATIONS.iter().find(|decoration| decoration.layer == layer).copied()
}

/// La colonie dont une image est dessinée, avec ce que son infobulle lit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColonyBlob {
    /// La taille choisie pour ce nombre de citoyens.
    pub size: ColonySize,
    /// Le nom, écrit dans le blob (`ColonySize.java:42-43`).
    pub name: String,
    /// La ligne « propriétaire ».
    pub owner: String,
    /// La ligne « coordonnées ».
    pub coordinates: String,
    /// La ligne « citoyens ».
    pub citizens: String,
}

/// Le blob d'une colonie, tel que la fenêtre le construit (`:408-434`).
pub fn blob(info: &ColonyInfo) -> ColonyBlob {
    ColonyBlob {
        size: ColonySize::of(info.citizen_count),
        name: info.name.clone(),
        owner: owner_line(&info.owner),
        coordinates: coordinates_line(info.center[0], info.center[2]),
        citizens: citizens_line(info.citizen_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: i32, name: &str, x: i32, z: i32) -> ColonyInfo {
        ColonyInfo {
            id,
            center: [x, 0, z],
            name: name.to_owned(),
            citizen_count: 30,
            owner: "Simon".to_owned(),
            prestige: 0,
        }
    }

    #[test]
    fn the_zoom_starts_at_zero_and_the_label_is_written_only_once_it_changes() {
        let mut zoom = Zoom::new();
        assert_eq!(zoom.scale(), 0.0);
        // Les tuiles de la première image sont créées à 512 * 0.
        assert_eq!(tile_size(zoom.scale()), 0);
        // `if (currentScale != dragView.getScale())` : pas de changement, pas d'écriture.
        assert!(!zoom.observe(0.0));
        assert!(zoom.observe(0.3));
        assert_eq!(zoom.label(), "3x");
        assert!(zoom.detailed(), "0,3 n'est pas < 0,3 : les icônes fines restent");
        assert!(zoom.observe(0.29));
        assert!(!zoom.detailed(), "en dessous, les icônes fines disparaissent");
    }

    #[test]
    fn the_scale_label_divides_then_rounds_to_the_even_and_says_infinity_at_zero() {
        assert_eq!(scale_label(1.0), "1x");
        assert_eq!(scale_label(0.5), "2x");
        assert_eq!(scale_label(0.3), "3x");
        assert_eq!(scale_label(0.25), "4x");
        // 1/2 = 0,5 : `DecimalFormat("##")` arrondit au pair, donc zéro décimale.
        assert_eq!(scale_label(2.0), "0x");
        assert_eq!(scale_label(0.0), "∞x");
        assert_eq!(scale_label(-0.0), "∞x");
    }

    #[test]
    fn the_projection_keeps_the_centre_and_never_divides_by_zero() {
        // Même position : le point est exactement au centre de la vue.
        assert_eq!(offset(0, 360, 0.3), 180);
        // À moins de 2718 blocs, `max(1, ln(...))` vaut 1 : la distance est linéaire.
        assert_eq!(offset(1000, 360, 1.0), 180 - 4000);
        assert_eq!(offset(-1000, 360, 1.0), 180 + 4000);
        assert_eq!(offset(2000, 360, 1.0), 180 - 8000, "ln(2) < 1, donc diviseur 1");
        assert_eq!(offset(-2000, 224, 1.0), 112 + 8000);
        // `Math.log` est le logarithme népérien : à 4000 blocs, `ln(4) ≈ 1,386` et la
        // distance est soudain compressée par un diviseur supérieur à 1.
        let far = offset(4000, 360, 1.0);
        assert!(far > 180 - 16000, "la compression a commencé");
        assert!(far < 180 - 8000, "mais elle reste sous la distance doublée");
        // Un zoom plus petit rapproche du centre.
        assert!(offset(1000, 360, 0.1) > offset(1000, 360, 1.0));
        assert_eq!(offset(1000, 360, 0.0), 180, "zoom nul : tout au centre");
    }

    #[test]
    fn only_scale_zero_maps_become_tiles_and_the_tile_id_is_built_from_the_centre() {
        let maps = [
            MapData { center_x: 0, center_z: 0, scale: 0 },
            MapData { center_x: 512, center_z: 0, scale: 0 },
            MapData { center_x: 0, center_z: 512, scale: 2 },
        ];
        let placed = tiles(&maps, (0, 0), 360, 224, 0.3);
        assert_eq!(placed.len(), 2, "la carte d'échelle 2 est ignorée (:251-254)");
        assert_eq!(placed[0].id, "map0-0");
        assert_eq!(placed[1].id, "map512-0");
        assert_eq!(placed[0].size, 153, "512 * 0,3 tronqué");
        assert_eq!(tile_id(-12, 34), "map-12-34");
        // Une carte décalée est posée à `centre - 64` (:259).
        let shifted = tiles(&[MapData { center_x: 64, center_z: 64, scale: 0 }], (0, 0), 360, 224, 1.0);
        assert_eq!(shifted[0].at, world_to_ui(0, 0, 360, 224, 1.0));
    }

    #[test]
    fn has_maps_never_goes_back_to_false_once_it_was_true() {
        let with = [MapData { center_x: 0, center_z: 0, scale: 0 }];
        let without: [MapData; 0] = [];
        assert!(!has_maps(&without));
        assert!(has_maps(&with));
        // `addMaps` ne remet pas `hasMaps` à faux (:239-247) : le second passage garde
        // le vrai, et le `warning` reste caché alors qu'il n'y a plus de carte.
        assert!(has_maps_after_reopen(true, &without));
        assert!(!has_maps_after_reopen(false, &without));
        // Une carte agrandie seule ne compte pas comme une carte.
        assert!(!has_maps(&[MapData { center_x: 0, center_z: 0, scale: 1 }]));
    }

    #[test]
    fn a_house_with_room_is_framed_in_blue_and_not_in_transparency() {
        assert_eq!(capacity_colour(0, 5, 0), Some([0, 255, 33]));
        assert_eq!(capacity_colour(5, 5, 0), None, "pleine : pas de bordure");
        assert_eq!(capacity_colour(6, 5, 0), None);
        // Une portée non nulle exclut la coloration (:537).
        assert_eq!(capacity_colour(0, 5, 3), None);
        // À moitié pleine : jaune, puis rouge à saturation.
        assert_eq!(capacity_colour(2, 4, 0), Some([255, 255, 33]));
        // Au quart : le rouge monte, le vert reste à 255.
        assert_eq!(capacity_colour(1, 4, 0), Some([127, 255, 33]));
        let full = pct_full(4, 4);
        assert_eq!((red_from_range(full), green_from_range(full)), (255, 0));
        // Le troisième canal de la boîte est du bleu : 33, jamais un alpha.
        assert_eq!(capacity_colour(1, 2, 0).map(|colour| colour[2]), Some(33));
        // Un niveau nul ne divise pas par zéro : Java rendrait NaN, ce port répond 1,0
        // (une maison sans place est pleine) — et le seul appelant l'exclut de toute
        // façon, puisque `residents < level` est faux pour un niveau nul.
        assert_eq!(pct_full(0, 0), 1.0);
        assert_eq!(capacity_colour(0, 0, 0), None);
    }

    #[test]
    fn a_colony_blob_is_sized_by_its_citizens_and_falls_back_to_small() {
        assert_eq!(ColonySize::of(0), ColonySize::Small);
        assert_eq!(ColonySize::of(24), ColonySize::Small);
        assert_eq!(ColonySize::of(25), ColonySize::Medium);
        assert_eq!(ColonySize::of(74), ColonySize::Medium);
        assert_eq!(ColonySize::of(75), ColonySize::Large);
        assert_eq!(ColonySize::of(4999), ColonySize::Large);
        // 5000 et plus : la boucle sort sans rien trouver et rend SMALL (:57).
        assert_eq!(ColonySize::of(5000), ColonySize::Small);
        assert_eq!(ColonySize::of(1_000_000), ColonySize::Small);
        assert_eq!(ColonySize::Medium.max_citizens(), 75);
        assert!(ColonySize::Large.resource().ends_with("colonylarge.xml"));
    }

    #[test]
    fn the_tooltip_lines_keep_javas_spacing_including_the_owner_colon() {
        assert_eq!(owner_line("Simon"), "Owner:Simon", "pas d'espace après Owner: (:418)");
        assert_eq!(coordinates_line(120, -340), "Coordinates: 120X, -340Z");
        assert_eq!(citizens_line(31), "Citizens: 31");
        assert_eq!(building_line("com.minecolonies.building.farmer", 3), "com.minecolonies.building.farmer : 3");
        // Le blob porte les trois lignes et le nom.
        let blob = blob(&info(1, "Bourg", 120, -340));
        assert_eq!(blob.name, "Bourg");
        assert_eq!(blob.size, ColonySize::Medium, "30 citoyens");
        assert_eq!(blob.owner, "Owner:Simon");
        assert_eq!(blob.coordinates, "Coordinates: 120X, -340Z");
        assert_eq!(blob.citizens, "Citizens: 30");
    }

    #[test]
    fn the_status_marker_exists_only_when_the_player_has_requests_here() {
        assert_eq!(resolved_marker(0), None, "`if (playerResolvedRequests.size() > 0)` (:491)");
        let marker = resolved_marker(2).expect("deux requêtes");
        assert_eq!(marker.count, 2);
        assert_eq!(marker.key, RESOLVED_KEY);
        assert_eq!(marker_suffix(3).as_deref(), Some(": 3"));
        // Le suffixe n'est écrit que si le compte est positif (:521-524).
        assert_eq!(marker_suffix(0), None);
        assert_eq!(marker_suffix(-1), None);
        // Le compte de l'infobulle est la taille de la liste, pas la somme.
        assert_ne!(RESOLVED_KEY, RESOLVED_COUNT_KEY);
    }

    #[test]
    fn the_inventory_button_is_hidden_from_the_item_and_the_exit_does_not_reopen() {
        assert!(!inventory_visible(false), "findPaneByID(BUTTON_INVENTORY).hide() (:217)");
        assert!(inventory_visible(true));
        // Depuis la salle des fêtes, la croix rouvre le bâtiment ; depuis l'item, elle
        // ferme et `Effect` ne sait pas le dire.
        assert_eq!(exit_effect(true), Some(Effect::Reopens));
        assert_eq!(exit_effect(false), None);
        // Le bouton est **enregistré** dans les deux cas (:220) : caché, il répond.
        assert!(action(INVENTORY).is_some());
        assert_eq!(
            action(INVENTORY).map(|action| action.effect),
            Some(Effect::Sends(OPEN_INVENTORY_MESSAGE))
        );
    }

    #[test]
    fn the_two_prestige_buttons_are_the_same_door_and_the_four_buttons_are_four() {
        assert_eq!(ACTIONS.len(), 4);
        assert_eq!(action(PRESTIGE).map(|action| action.effect), Some(Effect::Opens("map.prestige")));
        assert_eq!(action(PRESTIGE_ICON).map(|action| action.effect), Some(Effect::Opens("map.prestige")));
        assert_ne!(PRESTIGE, PRESTIGE_ICON, "deux boutons, une seule destination (:221-222)");
        // Aucun n'est gardé par un refus : la carte ne refuse aucun clic.
        assert!(ACTIONS.iter().all(|action| !action.is_guarded()));
        assert_eq!(action("hire"), None);
        // L'id de la carte n'est pas un écran de `src/ui` : il est refusé par son nom.
        assert!(crate::ui::screen_by_id(ID).is_err());
    }

    #[test]
    fn seven_layers_are_drawn_and_two_of_them_have_no_id_at_all() {
        assert_eq!(DECORATIONS.len(), 7);
        let without_id: Vec<&str> = DECORATIONS
            .iter()
            .filter(|decoration| decoration.id.starts_with('('))
            .map(|decoration| decoration.id)
            .collect();
        // Le point du joueur (:441-448) et l'icône d'état (:493) sont créés sans id.
        assert_eq!(without_id.len(), 2);
        assert_eq!(decoration(Layer::Center).map(|decoration| decoration.value), Some(Value::Icon));
        // Chaque couche a exactement une ligne, et chaque ligne nomme une couche.
        for layer in [
            Layer::Map,
            Layer::Citizen,
            Layer::Building,
            Layer::Colony,
            Layer::Player,
            Layer::Center,
            Layer::Status,
        ] {
            assert!(decoration(layer).is_some(), "{layer:?} n'est pas dans la table");
        }
        // Deux sources différentes pour deux choses différentes.
        assert_eq!(decoration(Layer::Status).map(|decoration| decoration.source), Some(Source::Request));
        assert_eq!(decoration(Layer::Citizen).map(|decoration| decoration.source), Some(Source::Citizen));
    }
}
