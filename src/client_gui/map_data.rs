//! La carte de colonie, cote **donnee** : ce que porte une image de carte, et
//! l'arithmetique de zoom et de glissement qui la place.
//!
//! Java reference: `core/client/gui/map/` — `MinecraftMap.java` (85 lignes, le panneau
//! d'image), `ZoomDragMap.java` (230, la vue zoomable), `ColonySize.java` (59, la taille
//! du blob d'une colonie) et `WindowColonyMap.java` (754, la fenetre).
//!
//! **Ce module ne redécrit pas `WindowColonyMap`.** Ce qu'elle demande, ses sept sortes
//! d'objets, ses infobulles et ses deux textes sont dans `ui_window_11::map`
//! ([`crate::ui_window_11::map`]) ; `ColonySize` y est aussi (`ui_window_11::map::ColonySize`,
//! `ColonySize.java:15-19`). Ce qui restait dehors est la **mecanique** que ce module
//! porte, et elle n'est ni un panneau ni un bouton :
//!
//! | Question | Java | Ici |
//! | --- | --- | --- |
//! | une image de carte, c'est quoi ? | `MinecraftMap.setMapData`, `:44-61` | [`Map`] |
//! | ou tombe un clic dans une carte dezoommee ? | `ZoomDragMap.calcRelativeX`, `:82-89` | [`relative`] |
//! | que fait la molette ? | `ZoomDragMap.scrollInput`, `:198-212` | [`zoom`] |
//! | que fait un glissement ? | `ZoomDragMap.onMouseDrag`, `:185-196` | [`drag`] |
//! | quel enfant est dessine ? | `ZoomDragMap.childIsVisible`, `:57-61` | [`visible`] |
//! | que lit le libelle `scale` ? | `WindowColonyMap.java:400` | deja dans `ui_window_11::map::scale_label` |
//!
//! # Quatre choses que la source dit et qui ne se devinent pas
//!
//! 1. **La molette n'utilise pas la souris.** `scrollInput(wheel, mx, my)` recoit la
//!    position du curseur et ne la lit pas : le point fixe du zoom est
//!    `((width - scrollX) / 2, (height - scrollY) / 2)` (`:200`, `:203-206`), une
//!    fonction du **defilement**, pas du curseur. [`zoom`] prend donc la taille de la vue
//!    et non un couple `mx`/`my`, et le test
//!    `the_wheel_ignores_the_mouse_and_fixes_a_point_that_moves_with_the_scroll` le dit.
//! 2. **Un negatif zoome en arriere.** `wheel < 0 ? scale / factor : scale * factor`
//!    (`:207`) : c'est l'inverse de l'intuition "negatif = avant", et c'est la seule
//!    lecture que le code permet.
//! 3. **Deux planchers differents.** `minScale` vaut `0.02` (`:29`) et sert au zoom
//!    (`:208`) ; le **dessin** borne a `0.26219988382999904` (`:149`, `:166`), un litteral
//!    sans nom qui n'apparait nulle part ailleurs. Entre les deux, la carte est dessinee
//!    plus grande que son echelle reelle : [`relative`] continue de convertir avec la
//!    vraie valeur, donc un clic tombe ailleurs que sur l'icone cliquee.
//! 4. **Une carte est une texture de 128 par 128, et son nom est l'id du panneau.**
//!    `new DynamicTexture(128, 128, false)` et `register("minecolonies_map/" + id)`
//!    (`MinecraftMap.java:48`, `:59`) : deux panneaux de carte qui partageraient un id
//!    ecraseraient la meme texture. [`Map::texture_name`] garde ce nom, et `Map::new`
//!    refuse un id vide plutot que de fabriquer un nom.

/// Le cote d'une texture de carte (`MinecraftMap.java:48`).
pub const MAP_SIDE: usize = 128;

/// Le nombre de pixels d'une carte : `colors` est un tableau plat de 16 384 octets
/// (`MinecraftMap.java:54`, `mapData.colors[x + y * 128]`).
pub const MAP_PIXELS: usize = MAP_SIDE * MAP_SIDE;

/// L'echelle de depart de la vue (`ZoomDragMap.java:19`).
pub const DEFAULT_SCALE: f64 = 1.0;

/// Le facteur de glissement (`ZoomDragMap.java:24`).
pub const DEFAULT_DRAG_FACTOR: f64 = 1.0;

/// Le facteur de zoom d'un cran de molette (`ZoomDragMap.java:27`).
pub const DEFAULT_ZOOM_FACTOR: f64 = 1.1;

/// Le plancher du **zoom** (`ZoomDragMap.java:29`).
pub const MIN_SCALE: f64 = 0.02;

/// Le plafond, commun au zoom et au dessin (`ZoomDragMap.java:30`, `:149`).
pub const MAX_SCALE: f64 = 6.0;

/// Le plancher du **dessin**, sans nom dans Java (`ZoomDragMap.java:149`, `:166`).
///
/// Il vaut environ treize fois `MIN_SCALE`. Le litteral est repris tel quel : le remplacer
/// par un arrondi changerait le rendu de Java, et ce port ne corrige pas Java.
pub const RENDER_MIN_SCALE: f64 = 0.262_199_883_829_999_04;

/// Pourquoi une carte ne peut pas etre construite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : ferme sur les deux refus de construction
pub enum Refusal {
    /// Le tableau de pixels n'a pas la taille d'une carte
    /// (`MinecraftMap.java:51-56` : la double boucle lit exactement 128 x 128).
    WrongPixelCount { got: usize, want: usize },
    /// Un panneau sans id n'a pas de nom de texture (`MinecraftMap.java:59`) et
    /// enregistrer sous la chaine vide n'est pas un nom.
    NamelessPane,
}

impl Refusal {
    /// Un nom stable, prefixes.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::WrongPixelCount { .. } => "map.refusal.wrong_pixels",
            Refusal::NamelessPane => "map.refusal.nameless_pane",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Une carte chargee : l'id du panneau, la position du bloc de carte, et ses pixels.
///
/// `colors` est le tableau de `MapItemSavedData`, un octet par pixel, indexe
/// `x + y * 128` (`MinecraftMap.java:54`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pane: String,
    at: [i32; 3],
    colors: Vec<u8>,
}

impl Map {
    /// Une carte, si le tableau a exactement [`MAP_PIXELS`] octets et si le panneau a un
    /// id.
    pub fn new(pane: &str, at: [i32; 3], colors: Vec<u8>) -> Result<Map, Refusal> {
        if pane.is_empty() {
            return Err(Refusal::NamelessPane);
        }
        if colors.len() != MAP_PIXELS {
            return Err(Refusal::WrongPixelCount { got: colors.len(), want: MAP_PIXELS });
        }
        Ok(Map { pane: pane.to_owned(), at, colors })
    }

    /// Le nom de texture que Java enregistre (`MinecraftMap.java:59`).
    pub fn texture_name(&self) -> String {
        format!("minecolonies_map/{}", self.pane)
    }

    /// La position du bloc de carte, telle que la fenetre la lit pour placer la tuile.
    pub fn at(&self) -> [i32; 3] {
        self.at
    }

    /// L'indice du pixel `(x, y)`, ou `None` hors de la carte.
    pub fn index(x: usize, y: usize) -> Option<usize> {
        if x < MAP_SIDE && y < MAP_SIDE {
            Some(x + y * MAP_SIDE)
        } else {
            None
        }
    }

    /// La couleur emballee du pixel, ou `None` hors carte.
    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        Map::index(x, y).and_then(|index| self.colors.get(index).copied())
    }

    /// Le cote dessine d'une tuile a cette echelle (`WindowColonyMap.java:262`).
    ///
    /// A l'echelle zero — celle avec laquelle la fenetre s'ouvre
    /// (`WindowColonyMap.java:178`, `private double currentScale = 0`) — la tuile mesure
    /// zero pixel. C'est Java, et c'est pourquoi le libelle `scale` n'est ecrit qu'au
    /// premier changement.
    pub fn tile_size(scale: f64) -> i32 {
        (crate::ui_window_11::map::MAP_TILE as f64 * scale) as i32
    }
}

/// L'echelle du dessin : le zoom borne au plancher du rendu, pas a celui du zoom
/// (`ZoomDragMap.java:149`).
///
/// Au-dessus de [`RENDER_MIN_SCALE`] c'est l'identite, en dessous c'est un blocage, et
/// au-dela de [`MAX_SCALE`] c'est [`MAX_SCALE`] : la fonction est donc **non injective**
/// sous 0.2622, ce qui est le point.
pub fn render_scale(scale: f64) -> f64 {
    scale.clamp(RENDER_MIN_SCALE, MAX_SCALE)
}

/// L'echelle apres un cran de molette.
///
/// `wheel < 0` **divise** (`ZoomDragMap.java:207`), puis le resultat est borne a
/// `[minScale, maxScale]` (`:208`).
pub fn zoom_scale(scale: f64, wheel: f64, factor: f64, min: f64, max: f64) -> f64 {
    let scaled = if wheel < 0.0 { scale / factor } else { scale * factor };
    scaled.clamp(min, max)
}

/// Le point que le zoom garde fixe, en coordonnees de vue
/// (`ZoomDragMap.java:200`, `:203-204`).
///
/// **Il depend du defilement**, pas de la souris : `(width - scrollX) / 2`.
pub fn zoom_anchor(view: f64, scroll: f64) -> f64 {
    (view - scroll) / 2.0
}

/// Le defilement apres un zoom d'un cran.
///
/// Reproduction de `:203-210` : on repasse le point fixe en coordonnees d'enfant avec
/// l'ancienne echelle, on applique la nouvelle, et on en deduit le defilement.
pub fn zoom(
    scale: f64,
    scroll: f64,
    view: f64,
    wheel: f64,
    factor: f64,
    min: f64,
    max: f64,
) -> (f64, f64) {
    let anchor = zoom_anchor(view, scroll);
    let child = (anchor + scroll) / scale;
    let new_scale = zoom_scale(scale, wheel, factor, min, max);
    (new_scale, child * new_scale - anchor)
}

/// Le defilement apres un glissement de `delta` pixels (`ZoomDragMap.java:191-193`).
///
/// Java soustrait `x * dragFactor` **sans borner** : rien n'empeche de pousser la carte
/// hors de la vue, et rien ne la ramene.
pub fn drag(scroll: f64, delta: f64, factor: f64) -> f64 {
    scroll - delta * factor
}

/// Une coordonnee d'evenement ramenee en coordonnees d'enfant
/// (`ZoomDragMap.java:82-89`).
///
/// `(x_in - origin + scroll) / scale + origin` : le defilement est retire **avant** la
/// division, donc a une echelle inferieure a 1 un pixel d'ecran vaut plusieurs pixels
/// d'enfant.
pub fn relative(x_in: f64, origin: f64, scroll: f64, scale: f64) -> f64 {
    (x_in - origin + scroll) / scale + origin
}

/// Si un enfant est dessine, selon les quatre bornes de `childIsVisible`
/// (`ZoomDragMap.java:57-61`).
///
/// Le test est **strict** en haut (`< interior`) et **large** en bas (`>= 0`) : un enfant
/// dont le bord gauche est exactement a droite de la vue disparait, un enfant dont le
/// bord droit est exactement a gauche reste dessine.
pub fn visible(
    child: (f64, f64, f64, f64),
    scale: f64,
    scroll: (f64, f64),
    interior: (f64, f64),
) -> bool {
    let (cx, cy, cw, ch) = child;
    let (sx, sy) = scroll;
    let (iw, ih) = interior;
    let left = cx * scale - sx;
    let top = cy * scale - sy;
    let right = (cx + cw) * scale - sx;
    let bottom = (cy + ch) * scale - sy;
    left < iw && top < ih && right >= 0.0 && bottom >= 0.0
}

// Le libelle du panneau `scale` (`WindowColonyMap.java:400`) **n'est pas reecrit ici** :
// `ui_window_11::map::scale_label` le porte deja, avec le `DecimalFormat("##")` et
// l'infini de l'echelle zero. Ce module s'y raccorde par la fonction, pas par une copie,
// et un test de ce fichier verifie que les deux tombent d'accord sur la valeur d'ouverture.

#[cfg(test)]
mod tests {
    use super::*;

    fn map_fixture() -> Map {
        Map::new("mapTile", [10, 64, -4], vec![0u8; MAP_PIXELS]).unwrap_or_else(|_| panic!("refused"))
    }

    #[test]
    fn a_map_is_128_square_and_the_pixels_are_indexed_x_plus_y_times_128() {
        assert_eq!(MAP_PIXELS, 16_384);
        assert_eq!(Map::index(0, 0), Some(0));
        assert_eq!(Map::index(127, 0), Some(127));
        assert_eq!(Map::index(0, 1), Some(128));
        assert_eq!(Map::index(127, 127), Some(MAP_PIXELS - 1));
        // Hors carte, pas d'indice du tout.
        assert_eq!(Map::index(128, 0), None);
        assert_eq!(Map::index(0, 128), None);
    }

    #[test]
    fn a_pixel_table_of_the_wrong_size_is_refused_by_name() {
        let short = Map::new("tile", [0, 0, 0], vec![0u8; 100]);
        // Le nom du refus, puis ses deux nombres (`as_ref` pour ne pas consommer le
        // `Result`, qui n'est pas `Copy`).
        assert_eq!(
            short.as_ref().map_err(|refusal| refusal.name()),
            Err("map.refusal.wrong_pixels")
        );
        match short {
            Err(Refusal::WrongPixelCount { got, want }) => {
                assert_eq!(got, 100);
                assert_eq!(want, MAP_PIXELS);
            }
            other => panic!("{other:?}"),
        }
        assert!(Map::new("tile", [0, 0, 0], vec![0u8; MAP_PIXELS]).is_ok());
    }

    #[test]
    fn a_pane_without_an_id_cannot_name_its_texture() {
        assert_eq!(
            Map::new("", [0, 0, 0], vec![0u8; MAP_PIXELS]).map_err(|r| r.name()),
            Err("map.refusal.nameless_pane")
        );
        assert_eq!(map_fixture().texture_name(), "minecolonies_map/mapTile");
        assert_eq!(map_fixture().at(), [10, 64, -4]);
        assert_eq!(map_fixture().pixel(0, 0), Some(0));
        assert_eq!(map_fixture().pixel(128, 0), None);
    }

    #[test]
    fn a_tile_measures_512_pixels_times_the_scale_so_zero_gives_zero() {
        // Le cote de la tuile est la constante de `ui_window_11::map` (`:262`,
        // `512 * currentScale`) : ce module ne la redefinit pas.
        assert_eq!(Map::tile_size(1.0), crate::ui_window_11::map::MAP_TILE);
        assert_eq!(Map::tile_size(0.5), 256);
        assert_eq!(
            Map::tile_size(0.0),
            0,
            "WindowColonyMap.java:178 ouvre a l'echelle zero"
        );
        assert_eq!(Map::tile_size(MAX_SCALE), crate::ui_window_11::map::MAP_TILE * 6);
    }

    #[test]
    fn the_wheel_divides_on_a_negative_and_multiplies_otherwise() {
        assert_eq!(zoom_scale(1.0, -1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE), 1.0 / 1.1);
        assert_eq!(zoom_scale(1.0, 1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE), 1.1);
        // Le zero n'est pas un "non" : il multiplie, donc il zoome en avant.
        assert_eq!(zoom_scale(1.0, 0.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE), 1.1);
        assert!(zoom_scale(1.0, -1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE) < 1.0);
    }

    #[test]
    fn the_zoom_stops_at_two_different_floors() {
        let mut scale = 1.0;
        for _ in 0..80 {
            scale = zoom_scale(scale, -1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        }
        assert!((scale - MIN_SCALE).abs() < 1e-12, "le zoom s'arrete a 0.02, pas plus bas");
        let mut high = 1.0;
        for _ in 0..80 {
            high = zoom_scale(high, 1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        }
        assert!((high - MAX_SCALE).abs() < 1e-12);
        // Mais le dessin s'arrete bien plus haut, et c'est le litteral de :149.
        assert!(RENDER_MIN_SCALE > MIN_SCALE);
        assert!((render_scale(0.1) - RENDER_MIN_SCALE).abs() < 1e-12);
        assert_eq!(render_scale(0.5), 0.5, "au-dessus du plancher de dessin, identite");
        assert_eq!(render_scale(9.0), MAX_SCALE);
    }

    #[test]
    fn below_the_render_floor_the_picture_and_the_clicks_disagree() {
        // La carte est dessinee a 0.2622 mais convertie a 0.02 : un clic a 100 pixels de
        // l'origine tombe a (100 + 0) / 0.02 + 0 = 5000 dans la carte.
        let drawn = relative(100.0, 0.0, 0.0, render_scale(0.02));
        let converted = relative(100.0, 0.0, 0.0, 0.02);
        assert!((drawn - 381.0).abs() < 1.0);
        assert_eq!(converted, 5000.0);
        assert_ne!(drawn, converted);
    }

    #[test]
    fn the_wheel_ignores_the_mouse_and_fixes_a_point_that_moves_with_the_scroll() {
        // Le point fixe est (width - scroll) / 2 : deux defilements, deux points.
        assert_eq!(zoom_anchor(400.0, 0.0), 200.0);
        assert_eq!(zoom_anchor(400.0, 100.0), 150.0);
        // Un zoom a scroll nul garde le centre de la vue.
        let (scale, scroll) = zoom(1.0, 0.0, 400.0, 1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        assert!((scale - 1.1).abs() < 1e-12);
        assert!((scroll - 20.0).abs() < 1e-9, "200 * 1.1 - 200 = 20");
        // Et le meme zoom avec un defilement de 100 ne garde pas le meme point :
        // childX = (400 - 100) / 2 = 150, oldX = (150 + 100) / 1 = 250, et
        // 250 * 1.1 - 150 = 125.
        let (_, other) = zoom(1.0, 100.0, 400.0, 1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        assert!((other - 125.0).abs() < 1e-9);
        assert_ne!(scroll, other);
    }

    #[test]
    fn a_zoom_out_does_not_put_the_scroll_back_because_the_anchor_moved() {
        let (mid, scroll) = zoom(1.0, 0.0, 400.0, 1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        let (back, scroll_back) =
            zoom(mid, scroll, 400.0, -1.0, DEFAULT_ZOOM_FACTOR, MIN_SCALE, MAX_SCALE);
        assert!((back - 1.0).abs() < 1e-12, "l'echelle, elle, revient");
        // Le point fixe du retour est (400 - 20) / 2 = 190, pas 200 : le defilement ne
        // revient donc pas a zero. C'est la consequence de :200, qui lit `scrollX`.
        assert!((scroll_back - (210.0 / 1.1 - 190.0)).abs() < 1e-9);
        assert!(scroll_back > 0.9, "il reste environ 0.9 pixel de defilement");
    }

    #[test]
    fn a_drag_moves_the_content_the_other_way_and_never_comes_back() {
        assert_eq!(drag(0.0, 10.0, DEFAULT_DRAG_FACTOR), -10.0);
        assert_eq!(drag(0.0, -10.0, DEFAULT_DRAG_FACTOR), 10.0);
        assert_eq!(drag(5.0, 10.0, 2.0), -15.0, "le facteur multiplie le delta");
        // Rien ne borne : deux glissements de 10000 sortent la carte pour de bon.
        let far = drag(drag(0.0, 10_000.0, 1.0), 10_000.0, 1.0);
        assert_eq!(far, -20_000.0);
    }

    #[test]
    fn a_relative_conversion_removes_the_origin_divides_then_adds_it_back() {
        // `(xIn - x + scrollX) / scale + x` (`ZoomDragMap.java:82-84`) : l'origine est
        // retiree, le defilement ajoute, la division faite, puis l'origine revient.
        assert_eq!(relative(100.0, 20.0, 0.0, 1.0), 100.0);
        // Avec un defilement de 40 : (100 - 20 + 40) / 1 + 20 = 140.
        assert_eq!(relative(100.0, 20.0, 40.0, 1.0), 140.0);
        assert_eq!(relative(100.0, 20.0, 0.0, 2.0), 60.0);
        // A l'echelle 0.5, un pixel d'ecran vaut deux pixels d'enfant.
        assert_eq!(relative(120.0, 20.0, 0.0, 0.5), 220.0);
        // Le defilement s'ajoute avant la division : (50 - 20 + 40) / 1 + 20 = 90.
        assert_eq!(relative(50.0, 20.0, 40.0, 1.0), 90.0);
    }

    #[test]
    fn an_enfant_off_the_right_edge_disappears_and_one_on_the_edge_stays() {
        let interior = (400.0, 300.0);
        // Entierement visible.
        assert!(visible((10.0, 10.0, 50.0, 50.0), 1.0, (0.0, 0.0), interior));
        // Bord gauche exactement a droite : `left < iw` est faux a 400 < 400.
        assert!(!visible((400.0, 0.0, 10.0, 10.0), 1.0, (0.0, 0.0), interior));
        // Bord droit exactement a gauche : `right >= 0` est vrai a 0 >= 0.
        assert!(visible((-10.0, 0.0, 10.0, 10.0), 1.0, (0.0, 0.0), interior));
        // Le defilement deplace l'enfant : ce qui etait dehors revient.
        assert!(visible((400.0, 0.0, 10.0, 10.0), 1.0, (100.0, 0.0), interior));
    }

    #[test]
    fn the_scale_label_is_the_one_ui_window_11_already_carries() {
        // Le port ne reecrit pas la fonction : il la reutilise, et ce test est la couture
        // entre les deux modules (`WindowColonyMap.java:400`, `DecimalFormat("##")`).
        use crate::ui_window_11::map::scale_label;
        assert_eq!(scale_label(1.0), "1x");
        assert_eq!(scale_label(0.5), "2x");
        assert_eq!(scale_label(DEFAULT_ZOOM_FACTOR), "1x", "1 / 1.1 arrondi a l'entier");
        // A l'ouverture, `currentScale` vaut zero (`WindowColonyMap.java:178`).
        assert_eq!(scale_label(0.0), "∞x");
        assert_eq!(scale_label(-0.0), "∞x");
    }

    #[test]
    fn every_map_refusal_has_its_own_prefixed_name() {
        let all = [Refusal::WrongPixelCount { got: 1, want: 2 }, Refusal::NamelessPane];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("map.refusal."), "{refusal}");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name());
            }
        }
        assert_eq!(Refusal::NamelessPane.to_string(), "map.refusal.nameless_pane");
    }
}
