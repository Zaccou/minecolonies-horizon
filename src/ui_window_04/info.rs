//! `WindowInfo` : la page d'aide d'un type de bâtiment, en entier — c'est la seule des quatre
//! fenêtres de ce module que `src/ui` ne décrit pas.
//!
//! Java reference : `core/client/gui/WindowInfo.java` (70 lignes) et `gui/windowinfo.xml` (15),
//! plus les deux moitiés du skeleton qui font le travail : la lecture du `switchView`
//! (`AbstractWindowSkeleton.java:77-94`) et le gestionnaire de pages (`:218-251`).
//!
//! # Ce que la fenêtre demande, et comment elle le demande
//!
//! Il n'y a **pas une ligne de données par page** côté serveur : la classe fabrique une page par
//! **clé de langue existante**, dans une boucle qui s'arrête à la première absente
//! (`WindowInfo.java:42-47`). Le préfixe est `"com.minecolonies.coremod.info." +
//! type.replace("com.minecolonies.building.", "") + "."` (`:33`), soit
//! [`prefix`] ; et chaque page lit **deux** clés : `…i.name` (le titre de la page, `:52`) et `…i`
//! (le corps, `:59-62`).
//!
//! **La boucle s'arrête, elle ne saute pas.** Une page 2 absente avec une page 3 présente donne
//! **deux** pages, pas trois : `break` sur `!I18n.exists` (`:44-47`). C'est la règle la plus facile
//! à casser en portant ce fichier, et [`page_count`] la reproduit à l'identique.
//!
//! **Pas de page, pas de bouton.** Le bouton `info` de toutes les fenêtres de bâtiment est caché
//! quand la page zéro n'existe pas (`AbstractBuildingMainWindow.java:56-59`) : la même question,
//! posée deux fois dans Java. [`super::minimum::open_info`] la pose une seule fois.
//!
//! # L'état, et les transitions
//!
//! La fenêtre n'a **qu'un** état : l'index de page. Il est initialisé à la page zéro
//! (`WindowInfo.java:69`, `setPage(false, 0)`), et deux boutons le font avancer ou reculer d'un cran
//! (`:86-87`). Le skeleton clampe au lieu de refuser (`SwitchView.java:178-180`), cache la flèche
//! du bord atteint (`AbstractWindowSkeleton.java:239-246`) et **cache les trois contrôles** quand il
//! y a zéro ou une page (`:227-232`). [`Pager`] est cet état ; [`click`] est la transition.
//!
//! # Ce qu'elle rend, et ce qu'elle ne rend pas
//!
//! Un titre de page ([`Value::Text`]), un corps multiligne (`:59-62` : le texte est **découpé sur
//! les retours à la ligne** et recomposé en un seul composant, donc la découpe est un fait et non un
//! choix de rendu), et un compteur `3/5` — la **seule** valeur comptée des quatre fenêtres de ce
//! module. Pas de nom de bâtiment, pas de niveau, pas de progression de construction, pas de stock :
//! `src/ui` a raison sur ce point (`ui/mod.rs`, troisième prémisse), et un test le tient ici aussi.

use crate::building::Kind;
use crate::ui::action::{Action, Effect};
use crate::ui::field::{Field, Source, Value};
use crate::ui::screen::{Gate, Screen, Title};

use super::refusal::Refusal;
use super::Owner;

/// L'id de cette fenêtre dans la table de ce module. **Pas** celui de `src/ui`, qui n'en a pas.
pub const ID: &str = "window.info";

/// La classe Java qui la construit (`WindowInfo.java:20`).
pub const JAVA: &str = "WindowInfo";

/// Le XML (`WindowInfo.java:29`).
pub const RESOURCE: &str = "minecolonies:gui/windowinfo.xml";

/// Le préfixe des clés de manuel (`TranslationConstants.java:744`, appliqué à
/// `WindowInfo.java:33`).
pub const PARTIAL: &str = "com.minecolonies.coremod.info.";

/// Le `SwitchView` qui porte les pages (`windowinfo.xml:9`, `WindowConstants.java:187`).
pub const PAGES: &str = "pages";
/// Le bouton de sortie (`windowinfo.xml:11`, `WindowConstants.java:17`).
pub const EXIT: &str = "exit";
/// La flèche de gauche (`windowinfo.xml:12`, `WindowConstants.java:560`).
pub const PREV: &str = "prevPage";
/// La flèche de droite (`windowinfo.xml:13`, `WindowConstants.java:561`).
pub const NEXT: &str = "nextPage";
/// Le compteur de pages (`windowinfo.xml:14`, `WindowConstants.java:609`).
pub const PAGE_NUM: &str = "pageNum";

/// `3/5`, tel que le skeleton l'écrit (`AbstractWindowSkeleton.java:249`) : `curPage` est
/// **1-based** et le séparateur est une barre oblique sans espace.
pub const SEPARATOR: char = '/';

/// Le plafond de pages que ce port accepte de sonder.
///
/// Java boucle **sans borne** (`for (int i = 0;; i++)`, `WindowInfo.java:42`) : une couche de langue
/// qui répondrait `Some` pour toute clé ferait tourner le client indéfiniment. Une couche de langue
/// réaliste n'a jamais plus de cinq pages par bâtiment (le maximum mesuré est `miner`, cinq), donc
/// ce plafond ne change rien pour un pack réel — il empêche seulement la boucle de ne pas finir.
/// `MANQUANT.md` §2 le dit.
pub const PAGE_CEILING: usize = 64;

/// Le préfixe complet des clés de manuel d'un type de bâtiment, point final compris.
pub fn prefix(hut: Kind) -> String {
    format!("{PARTIAL}{}.", hut.name())
}

/// Combien de pages ce type de bâtiment a, en sondant la langue comme Java le fait.
///
/// `WindowInfo.java:42-47` : à partir de zéro, tant que `prefix + i` existe, une page de plus ;
/// la première absente arrête tout. Le paramètre est une **recherche** et non une valeur : ce
/// module reste de la donnée pure et ne touche pas l'hôte, et un test peut répondre un dictionnaire
/// troué. L'appelant réel branche `ClientHost::translate`
/// (`crates/horizon-mod-vocab/src/host/config.rs:176-179`), qui rend exactement ce que
/// `I18n.exists` demande — au prix d'une allocation, faute d'un `exists` dans le SDK.
pub fn page_count(hut: Kind, translate: impl Fn(&str) -> Option<String>) -> usize {
    let prefix = prefix(hut);
    let mut pages = 0;
    while pages < PAGE_CEILING {
        let key = format!("{prefix}{pages}");
        if translate(&key).is_none() {
            return pages;
        }
        pages += 1;
    }
    pages
}

/// Le bouton `info` d'une fenêtre de bâtiment est-il visible ?
///
/// La question de `AbstractBuildingMainWindow.java:58`, qui teste **`.0` seulement**. Puisque
/// [`page_count`] part de zéro et s'arrête à la première absente, les deux questions ont la même
/// réponse : un manuel sans page zéro n'aurait aucune page. Un test le prouve sur les deux formes.
pub fn has_manual(hut: Kind, translate: impl Fn(&str) -> Option<String>) -> bool {
    page_count(hut, translate) > 0
}

/// Le titre d'une page : `…i.name` (`WindowInfo.java:52`).
pub fn page_name(hut: Kind, page: usize, translate: impl Fn(&str) -> Option<String>) -> Option<String> {
    translate(&format!("{}{page}.name", prefix(hut)))
}

/// Le corps d'une page, découpé sur les retours à la ligne comme Java le fait.
///
/// `WindowInfo.java:59-62` : `(prefix + i).split("\\n")` puis un `appendNL` par morceau. `"\\n"` en
/// Java est l'expression régulière « retour à la ligne », pas la séquence littérale `\n` — d'où
/// [`str::split`] sur `'\n'` ici. Le corps est rendu **en morceaux** et non recollé : un appelant qui
/// veut une ligne unique la fabrique, et celui qui veut la pagination verticale a déjà ses lignes.
pub fn page_body(
    hut: Kind,
    page: usize,
    translate: impl Fn(&str) -> Option<String>,
) -> Option<Vec<String>> {
    let text = translate(&format!("{}{page}", prefix(hut)))?;
    Some(text.split('\n').map(str::to_owned).collect())
}

/// Une fenêtre décrite ici : ses panneaux et ses boutons, avec `src/ui`'s types.
///
/// C'est [`crate::ui::screen::Screen`] sans `title` et sans `gate` : cette fenêtre n'écrit aucun
/// titre (le XML n'a pas de panneau de titre, seulement des images et le `switchView`) et n'a pas de
/// porte. Les autres champs sont les mêmes types, pour qu'un appelant puisse traiter les quatre
/// fenêtres de la même façon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table.
    pub id: &'static str,
    /// La classe Java.
    pub java: &'static str,
    /// Le XML.
    pub resource: &'static str,
    /// Les panneaux que la classe remplit.
    pub fields: &'static [Field],
    /// Les boutons que la classe (ou son skeleton) câble.
    pub actions: &'static [Action],
}

impl Window {
    /// Le panneau de cet id, copié.
    pub fn field(&self, id: &str) -> Option<Field> {
        self.fields.iter().find(|field| field.id == id).copied()
    }

    /// Le bouton de cet id, copié.
    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions.iter().find(|action| action.id == id).copied()
    }

    /// Vrai quand la fenêtre écrit un panneau de ce genre.
    pub fn shows(&self, value: Value) -> bool {
        self.fields.iter().any(|field| field.value == value)
    }

    /// La même chose en [`Screen`], pour qui veut traiter les quatre fenêtres d'un seul type.
    ///
    /// Le titre est [`Title::Fixed`] sur le préfixe du manuel et il n'y a pas de porte : ce sont les
    /// deux champs que cette fenêtre n'a pas. Aucun panneau de titre n'existe dans
    /// `windowinfo.xml` (que des images, le `switchView` et un compteur), donc un appelant ne doit
    /// pas peindre ce `Fixed` — le remplir d'une vraie clé inventerait un texte que Java n'écrit pas.
    pub fn as_screen(&self) -> Screen {
        Screen {
            id: self.id,
            window: self.java,
            resource: self.resource,
            title: Title::Fixed(PARTIAL),
            fields: self.fields,
            actions: self.actions,
            gate: None::<Gate>,
        }
    }
}

/// Les quatre panneaux de cette fenêtre, déclarés une fois.
///
/// **Deux des trois panneaux à valeur n'ont aucun id XML.** Le titre de page et le corps sont
/// construits par la classe (`WindowInfo.java:52-66`) : `PaneBuilders…build()` puis `putInside` — il
/// n'y a pas de `findPaneOfTypeByID` à leur donner, donc le nom de ce module est leur seul id, et
/// c'est le cas que `src/ui::field` annonce à l'envers (« un panneau sans id n'est pas un champ » :
/// ici c'est la classe qui crée le panneau). Le troisième, [`PAGE_NUM`], est bien dans le XML.
const FIELDS: &[Field] = &[
    // Le `switchView` (`windowinfo.xml:9`) : une page par enfant (`WindowInfo.java:49-50`).
    Field::new(PAGES, Value::Row, Source::Building),
    // `…i.name`, le titre de la page (`WindowInfo.java:52`) : id de ce module.
    Field::in_row(PAGES, "name", Value::Text, Source::Building),
    // `…i`, le corps (`:59-62`) : id de ce module aussi, et multiligne.
    Field::in_row(PAGES, "text", Value::Text, Source::Building),
    // `3/5`, écrit par le skeleton (`AbstractWindowSkeleton.java:249`).
    Field::new(PAGE_NUM, Value::Text, Source::Building),
];

/// Les trois boutons, et **qui les câble** : `exit` est à `WindowInfo`, les deux flèches au skeleton.
const ACTIONS: &[Action] = &[
    // `registerButton(BUTTON_EXIT, () -> building.openGui(false))` (`WindowInfo.java:31`).
    Action::plain(EXIT, "", Effect::Reopens),
    // Le skeleton les câble en lisant le `switchView` (`AbstractWindowSkeleton.java:86-87`).
    Action::plain(PREV, "", Effect::Cycles),
    Action::plain(NEXT, "", Effect::Cycles),
];

/// Qui déclare quoi : un bouton sur trois vient de `WindowInfo`, les deux autres du skeleton.
pub const BUTTON_OWNERS: &[Owner] = &[
    Owner { id: EXIT, source: "WindowInfo.java", line: 31 },
    Owner { id: PREV, source: "AbstractWindowSkeleton.java", line: 86 },
    Owner { id: NEXT, source: "AbstractWindowSkeleton.java", line: 87 },
];

/// `WindowInfo`, décrite comme une fenêtre de cette table.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    fields: FIELDS,
    actions: ACTIONS,
};

/// Un manuel de la langue : combien de pages, et où la première est déclarée.
///
/// Relevé dans `assets/minecolonies/lang/manual_en_us.json` ; `lang_line` est la ligne du
/// `…0.name`, 1-based, pour qu'une relecture puisse y aller directement. Les lignes sont groupées
/// par bâtiment mais **pas** dans l'ordre alphabétique du fichier (`builder` est ligne 1507 et
/// `baker` 1568) : l'ordre de cette table est celui de [`Kind::ALL`], pour que le parcours et la
/// comparaison soient stables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manual {
    /// Le type de bâtiment.
    pub hut: Kind,
    /// Le nombre de pages, mesuré.
    pub pages: usize,
    /// La ligne de `…0.name` dans `manual_en_us.json`.
    pub lang_line: u32,
}

/// Les **dix-huit** bâtiments qui ont un manuel, avec leur nombre de pages mesuré.
///
/// Dix-huit sur cinquante-quatre : les trente-six autres n'ont aucune clé
/// `com.minecolonies.coremod.info.<type>.0`, donc leur bouton `info` est caché
/// (`AbstractBuildingMainWindow.java:56-59`) et `WindowInfo` ne s'ouvre jamais pour eux.
///
/// Le fichier de langue contient **dix-neuf** préfixes : le dix-neuvième est `citizen`
/// (`manual_en_us.json:1516`, trois pages), qui n'est pas un [`Kind`] — aucun bâtiment ne le porte,
/// et `WindowInfo` construit son préfixe à partir d'un type de bâtiment. Ces trois pages sont donc
/// inatteignables par cette fenêtre ; elles le sont peut-être par un autre écran, et ce n'est pas
/// celui-ci.
pub const MANUALS: &[Manual] = &[
    Manual { hut: Kind::Baker, pages: 4, lang_line: 1568 },
    Manual { hut: Kind::Barracks, pages: 3, lang_line: 1658 },
    Manual { hut: Kind::BarracksTower, pages: 4, lang_line: 1649 },
    Manual { hut: Kind::Builder, pages: 4, lang_line: 1507 },
    Manual { hut: Kind::ChickenHerder, pages: 4, lang_line: 1595 },
    Manual { hut: Kind::Composter, pages: 4, lang_line: 1631 },
    Manual { hut: Kind::Cook, pages: 3, lang_line: 1561 },
    Manual { hut: Kind::Cowboy, pages: 4, lang_line: 1622 },
    Manual { hut: Kind::Deliveryman, pages: 4, lang_line: 1552 },
    Manual { hut: Kind::Farmer, pages: 4, lang_line: 1577 },
    Manual { hut: Kind::Fisherman, pages: 4, lang_line: 1586 },
    Manual { hut: Kind::Graveyard, pages: 2, lang_line: 1665 },
    Manual { hut: Kind::GuardTower, pages: 4, lang_line: 1640 },
    Manual { hut: Kind::Lumberjack, pages: 4, lang_line: 1534 },
    Manual { hut: Kind::Miner, pages: 5, lang_line: 1523 },
    Manual { hut: Kind::Shepherd, pages: 4, lang_line: 1613 },
    Manual { hut: Kind::SwineHerder, pages: 4, lang_line: 1604 },
    Manual { hut: Kind::Warehouse, pages: 4, lang_line: 1543 },
];

/// Le manuel de ce type de bâtiment, s'il en a un.
pub fn manual_of(hut: Kind) -> Option<Manual> {
    MANUALS.iter().copied().find(|manual| manual.hut == hut)
}

/// L'état de pagination d'une fenêtre : l'index courant et le nombre de pages.
///
/// L'index est **0-based**, comme `SwitchView.setView` (`SwitchView.java:171-185`) ; tout ce qui
/// s'affiche passe par [`PageState`], qui est 1-based, parce que Java l'est
/// (`AbstractWindowSkeleton.java:235`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pager {
    page: usize,
    pages: usize,
}

/// Ce que l'écran montre à un instant donné : le compteur et les deux flèches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageState {
    /// La page courante, **1-based** ; `0` quand il n'y a aucune page.
    pub current: usize,
    /// Le nombre de pages.
    pub pages: usize,
    /// La flèche de gauche est-elle là ?
    pub prev: bool,
    /// La flèche de droite est-elle là ?
    pub next: bool,
}

impl Pager {
    /// Un paginateur ouvert sur la page zéro (`WindowInfo.java:69`).
    pub fn new(pages: usize) -> Pager {
        Pager { page: 0, pages }
    }

    /// Le nombre de pages.
    pub fn pages(self) -> usize {
        self.pages
    }

    /// L'index courant, 0-based.
    pub fn index(self) -> usize {
        self.page
    }

    /// Un cran en avant (`+1`) ou en arrière (`-1`), **clampé** aux deux bouts.
    ///
    /// Java ne refuse pas un dépassement : `SwitchView.setView` clampe (`:178-180`) et le skeleton
    /// éteint la flèche du bord (`:239-246`). `/` sans `endless` ne boucle pas, et
    /// `windowinfo.xml` ne demande pas l'endless.
    pub fn turn(self, delta: isize) -> Pager {
        let last = self.pages.saturating_sub(1) as isize;
        let page = (self.page as isize + delta).clamp(0, last.max(0));
        Pager { page: page as usize, pages: self.pages }
    }

    /// Ce que l'écran montre.
    pub fn state(self) -> PageState {
        if self.pages <= 1 {
            // `switchPagesSize <= 1` éteint les deux flèches **et** le compteur
            // (`AbstractWindowSkeleton.java:227-232`) : `off()` cache le panneau
            // (`blockui/Pane.java:277-281`).
            return PageState {
                current: self.pages,
                pages: self.pages,
                prev: false,
                next: false,
            };
        }
        let current = self.page + 1;
        PageState {
            current,
            pages: self.pages,
            prev: current > 1,
            next: current < self.pages,
        }
    }
}

impl PageState {
    /// Le texte du compteur, ou `None` quand Java cache le panneau.
    ///
    /// `None` n'est pas « pas encore calculé » : c'est `pageNum.off()`, donc un panneau invisible
    /// (`AbstractWindowSkeleton.java:231`).
    pub fn label(&self) -> Option<String> {
        if self.pages <= 1 {
            return None;
        }
        Some(format!("{}{SEPARATOR}{}", self.current, self.pages))
    }
}

/// Ce qu'un clic fait à la fenêtre : avancer, reculer, ou la refermer sur son bâtiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three buttons, three outcomes
pub enum Transition {
    /// L'index a bougé ; [`PageState`] est le nouvel état à peindre.
    Page(PageState),
    /// `exit` : `building.openGui(false)` (`WindowInfo.java:31`), donc la fenêtre du bâtiment.
    Reopens,
}

/// La transition d'un clic, ou `None` pour un id que cette fenêtre ne câble pas.
///
/// C'est la seule mécanique de ces quatre fenêtres qui ait un **état** : le guide et le renommage
/// n'en ont aucun (leurs deux boutons referment, point final), et le minimum n'a que des boutons qui
/// ouvrent ailleurs.
pub fn click(pager: Pager, id: &str) -> Option<Transition> {
    match id {
        NEXT => Some(Transition::Page(pager.turn(1).state())),
        PREV => Some(Transition::Page(pager.turn(-1).state())),
        EXIT => Some(Transition::Reopens),
        _ => None,
    }
}

/// Une fenêtre qui peut recevoir un changement de page : elle a un panneau `pages`.
///
/// Les deux formes de fenêtre de ce module n'ont pas le même type — une ligne de
/// [`crate::ui::screen::SCREENS`] et un [`Window`] — et c'est la seule question qu'elles ont en
/// commun.
pub trait Paged {
    /// Vrai quand la fenêtre porte un `switchView` (`windowinfo.xml:9`).
    fn has_pages(&self) -> bool;
}

impl Paged for Screen {
    fn has_pages(&self) -> bool {
        self.field(PAGES).is_some()
    }
}

impl Paged for Window {
    fn has_pages(&self) -> bool {
        self.field(PAGES).is_some()
    }
}

/// La règle du skeleton : sans `pages`, `setPage` rend la main sans rien faire
/// (`AbstractWindowSkeleton.java:220-223`).
///
/// Ce n'est pas une erreur de programmation : toutes les fenêtres de bâtiment héritent de
/// `setPage` et seule `WindowInfo` en a l'usage. Le port le dit par un refus nommé plutôt que par un
/// silence, parce qu'un appelant qui demande une page à un écran qui n'en a pas doit l'apprendre.
pub fn may_turn_page(window: &impl Paged) -> Result<(), Refusal> {
    if window.has_pages() {
        Ok(())
    } else {
        Err(Refusal::NoPages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::action::Effect;

    /// Une couche de langue fabriquée à partir de [`MANUALS`] : chaque page a un corps et un titre.
    fn manuals() -> impl Fn(&str) -> Option<String> {
        let mut table: Vec<(String, String)> = Vec::new();
        for manual in MANUALS {
            for page in 0..manual.pages {
                table.push((format!("{}{page}", prefix(manual.hut)), format!("ligne {page}")));
                table.push((format!("{}{page}.name", prefix(manual.hut)), "titre".to_owned()));
            }
        }
        move |key: &str| {
            table
                .iter()
                .find(|(name, _)| name.as_str() == key)
                .map(|(_, text)| text.clone())
        }
    }

    #[test]
    fn the_page_loop_stops_at_the_first_gap_instead_of_skipping_it() {
        // Pages 0 et 1, trou en 2, page 3 présente : Java répond deux, pas trois.
        let holed = |key: &str| {
            let page = key.rsplit('.').next()?.parse::<usize>().ok()?;
            (page < 2 || page == 3).then(|| "un\ndeux\ntrois".to_owned())
        };
        assert_eq!(page_count(Kind::Builder, holed), 2);
        assert!(has_manual(Kind::Builder, holed));
        // Zéro page : ni manuel, ni bouton.
        let empty = |_: &str| None;
        assert_eq!(page_count(Kind::Builder, empty), 0);
        assert!(!has_manual(Kind::Builder, empty));
        // Le corps est découpé sur les retours à la ligne (`WindowInfo.java:59-62`), le titre est
        // une autre clé du même index.
        let body = page_body(Kind::Builder, 0, holed);
        assert_eq!(body.as_ref().map(Vec::len), Some(3));
        assert_eq!(body.as_deref().and_then(|lines| lines.first()).map(String::as_str), Some("un"));
        let full = manuals();
        assert_eq!(page_name(Kind::Builder, 1, &full).as_deref(), Some("titre"));
        assert_eq!(page_name(Kind::Builder, 9, &full), None, "au-delà de la dernière page");
        assert_eq!(page_body(Kind::Builder, 9, &full), None);
    }

    #[test]
    fn eighteen_of_the_fifty_four_kinds_have_a_manual_and_the_rest_hide_the_button() {
        let probe = manuals();
        let mut with_manual = Vec::new();
        for hut in Kind::ALL {
            let pages = page_count(*hut, &probe);
            match manual_of(*hut) {
                Some(manual) => {
                    assert_eq!(pages, manual.pages, "{}", hut.name());
                    with_manual.push(*hut);
                }
                None => assert_eq!(pages, 0, "{} n'a pas de manuel", hut.name()),
            }
        }
        assert_eq!(with_manual.len(), 18);
        assert_eq!(Kind::ALL.len(), 54);
        assert_eq!(Kind::ALL.len() - with_manual.len(), 36, "36 boutons info cachés");
        // La somme des pages mesurées : 69 sur 72, `citizen` n'étant pas un bâtiment.
        assert_eq!(MANUALS.iter().map(|manual| manual.pages).sum::<usize>(), 69);
        // Les lignes sont croissantes dans l'ordre de `Kind::ALL` ? Non — et c'est le fichier qui
        // décide, pas cette table : on vérifie seulement qu'aucune ne se répète.
        for (index, manual) in MANUALS.iter().enumerate() {
            for other in &MANUALS[index + 1..] {
                assert_ne!(manual.lang_line, other.lang_line, "deux manuels, une ligne");
                assert_ne!(manual.hut, other.hut);
            }
            assert!(manual.pages >= 2, "aucun manuel d'une page ({:?})", manual.hut);
        }
    }

    #[test]
    fn the_pager_counts_from_one_and_hides_everything_below_two_pages() {
        let first = Pager::new(5).state();
        assert_eq!((first.current, first.pages), (1, 5));
        assert_eq!((first.prev, first.next), (false, true));
        assert_eq!(first.label().as_deref(), Some("1/5"));
        let second = Pager::new(5).turn(1).state();
        assert_eq!((second.current, second.prev, second.next), (2, true, true));
        let last = Pager::new(5).turn(1).turn(1).turn(1).turn(1).state();
        assert_eq!((last.current, last.prev, last.next), (5, true, false));
        assert_eq!(last.label().as_deref(), Some("5/5"));
        // Le clamp : ni refus, ni bouclage (`SwitchView.java:178-180`).
        assert_eq!(Pager::new(5).turn(-1).state().current, 1);
        assert_eq!(Pager::new(5).turn(99).state().current, 5);
        // Une page ou zéro : les trois contrôles sont éteints, donc pas de texte.
        for pages in 0..=1 {
            let state = Pager::new(pages).state();
            assert!(!state.prev && !state.next);
            assert_eq!(state.label(), None, "{pages} page(s)");
        }
        // Trois pages, et le compteur n'apparaît qu'à partir de deux.
        assert_eq!(Pager::new(2).state().label().as_deref(), Some("1/2"));
    }

    #[test]
    fn a_click_advances_the_page_or_reopens_the_building_and_an_unknown_id_does_nothing() {
        let pager = Pager::new(4);
        assert_eq!(
            click(pager, NEXT),
            Some(Transition::Page(Pager::new(4).turn(1).state()))
        );
        assert_eq!(
            click(pager, PREV),
            Some(Transition::Page(Pager::new(4).turn(-1).state())),
            "au bord gauche, reculer ne change rien et ne refuse pas"
        );
        assert_eq!(click(pager, EXIT), Some(Transition::Reopens));
        assert_eq!(click(pager, "done"), None);
        assert_eq!(click(pager, PAGES), None, "un panneau n'est pas un bouton");
        // Les trois ids sont exactement ceux que la fenêtre câble.
        for action in WINDOW.actions {
            assert!(click(pager, action.id).is_some(), "{} n'a pas de transition", action.id);
        }
        assert_eq!(WINDOW.actions.len(), 3);
        assert_eq!(BUTTON_OWNERS.len(), 3);
    }

    #[test]
    fn window_info_shows_a_page_name_a_body_and_a_counter_and_no_progress_at_all() {
        assert!(WINDOW.shows(Value::Text));
        assert!(WINDOW.shows(Value::Row));
        // La trouvaille de `src/ui` tenue depuis cette fenêtre : pas de progression, pas de stock.
        assert!(!WINDOW.shows(Value::Progress));
        assert!(!WINDOW.shows(Value::Stock));
        assert!(!WINDOW.shows(Value::Level));
        assert!(!WINDOW.shows(Value::Name), "un corps de page n'est pas un nom");
        // Le compteur est du texte calculé, pas une barre : `is_computed` ne le concerne pas.
        assert_eq!(WINDOW.field(PAGE_NUM).map(|field| field.value), Some(Value::Text));
        assert!(!Value::Text.is_computed());
        // Les deux panneaux de page sont dans la portée de la liste `pages`.
        for id in ["name", "text"] {
            let field = WINDOW.field(id).unwrap_or_else(|| panic!("{id} manque"));
            assert_eq!(field.scope, crate::ui::field::Scope::Row(PAGES));
        }
        // Et aucun n'a d'id XML : `windowinfo.xml` ne déclare que `pages`, `pageNum` et les boutons.
        assert_eq!(WINDOW.field(PAGES).map(|field| field.value), Some(Value::Row));
        assert_eq!(
            WINDOW.action(EXIT).map(|action| action.effect),
            Some(Effect::Reopens)
        );
        // La forme `Screen` est la même fenêtre, pour un appelant qui n'a qu'un type.
        let screen = WINDOW.as_screen();
        assert_eq!(screen.id, ID);
        assert_eq!(screen.window, JAVA);
        assert_eq!(screen.fields, WINDOW.fields);
        assert_eq!(screen.gate, None);
        assert_eq!(may_turn_page(&screen), Ok(()));
    }
}
