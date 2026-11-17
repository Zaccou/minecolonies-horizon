//! Le classement de prestige des colonies : une liste triée, trois boutons, zéro requête.
//!
//! Java référence : `core/client/gui/map/WindowColonyPrestigeRanking.java` (69 lignes —
//! la plus petite des quatre). Ce que la fenêtre **demande** : la liste statique
//! `WindowColonyMap.colonies` (`:42`), que seul `ColonyListMessage` remplit
//! (`ColonyListMessage.java:79-82`). Ce qu'elle **rend** : une ligne par colonie, et rien
//! d'autre — elle n'envoie **aucun** message, ne demande aucune vue fraîche et ne
//! s'abonne à rien.
//!
//! # Ce que Java fait et qui se voit ici
//!
//! 1. **Le classement monte, il ne descend pas.** `Comparator.comparing(
//!    ColonyInfo::getPrestige)` (`:43`) est un tri **croissant** : la colonie la plus
//!    prestigieuse est la **dernière** de la liste. Rien dans le fichier ne renverse
//!    l'ordre, et le titre de la fenêtre ne promet rien de plus. [`Ranking::new`] garde
//!    l'ordre de Java, et le test le montre sur quatre colonies.
//! 2. **Le tri est stable, et l'ordre initial est celui du serveur.** `List.sort` est
//!    stable (TimSort) : deux colonies à prestige égal gardent l'ordre de la liste
//!    qu'elles avaient, c'est-à-dire l'ordre de `IColonyManager.getColonies(...)`
//!    (`ColonyListMessage.java:53-73`). [`Ranking::new`] utilise `sort_by` — stable en
//!    Rust aussi — sur une copie.
//! 3. **C'est un instantané.** `new ArrayList<>(WindowColonyMap.colonies)` (`:42`) copie
//!    la liste à la construction ; une colonie qui se crée pendant que la fenêtre est
//!    ouverte n'y apparaît pas, et il faut la rouvrir. Le tri ne touche pas la liste de
//!    la carte : la carte et le classement peuvent donc être dans deux ordres différents
//!    en même temps.
//! 4. **Le bouton d'infobulle n'a pas d'id.** Le XML en dessine un quatrième
//!    (`windowcolonyprestigeranking.xml:11-13`) avec une infobulle et **aucun** `id` :
//!    `registerButton` ne peut pas le lier, et rien ne le fait. Ce n'est donc ni un
//!    [`Action`] ni un [`Field`] — un panneau sans id n'a pas de poignée.
//! 5. **`desc` est le même id de panneau que celui de l'onglet du builder.** Les deux
//!    fenêtres ne sont jamais ouvertes ensemble, et un id de ligne n'a de sens que dans sa
//!    liste ; le port ne peut pas les confondre parce que la liste, elle, a un id
//!    qu'elles ne partagent pas ([`COLONIES`] contre `resources`).
//! 6. **Les deux boutons de la carte mènent à la carte.** `map` et `mapicon` sont deux
//!    dessins d'une seule porte (`:39-40`), et ils propagent le drapeau de la salle des
//!    fêtes sans le changer.

use crate::network::views::ColonyInfo;
use crate::ui::action::{Action, Effect};
use crate::ui::field::{Field, Source, Value};

/// L'écran de cette table, ouvrable par [`crate::ui::action::Effect::Opens`] depuis la
/// carte. **Pas** un id de `crate::ui` : le classement n'est pas une hutte.
pub const ID: &str = "map.prestige";
/// La classe (`WindowColonyPrestigeRanking.java:20`).
pub const JAVA: &str = "WindowColonyPrestigeRanking";
/// Le XML (`:29`).
pub const RESOURCE: &str = "minecolonies:gui/map/windowcolonyprestigeranking.xml";
/// Le titre, écrit par l'XML et par personne d'autre (`windowcolonyprestigeranking.xml:10`
/// — un `label` fixe, jamais réécrit par la classe).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.townhall.prestige";
/// L'infobulle du bouton sans id (`windowcolonyprestigeranking.xml:11-13`).
pub const TITLE_TIP_KEY: &str = "com.minecolonies.coremod.gui.townhall.prestige.tooltip";
/// La liste (`WindowColonyPrestigeRanking.java:45`).
pub const COLONIES: &str = "colonies";
/// Le texte d'une ligne (`WindowColonyPrestigeRanking.java:66`).
pub const DESC: &str = "desc";
/// Le bouton qui ramène à la carte (`WindowColonyPrestigeRanking.java:39`).
pub const MAP: &str = "map";
/// Sa pastille (`WindowColonyPrestigeRanking.java:40`).
pub const MAP_ICON: &str = "mapicon";
/// La croix (`WindowColonyPrestigeRanking.java:33`, `:37`).
pub const EXIT: &str = "exit";

/// Les deux panneaux que la classe écrit.
///
/// `title` n'y est pas : il a un id mais la classe ne le touche jamais
/// (`:27-68` ne contient aucune référence à `"title"`), donc c'est un libellé de XML et
/// non une valeur que le modèle fournit.
pub const FIELDS: &[Field] = &[
    Field::new(COLONIES, Value::Row, Source::Colony),
    Field::in_row(COLONIES, DESC, Value::Text, Source::Colony),
];

/// Les trois boutons, dans l'ordre où Java les enregistre (`:31-40`).
///
/// `exit` est déclaré dans sa forme **salle des fêtes** (`Effect::Reopens`, `:33`) ;
/// depuis l'item il ferme, ce qu'`Effect` ne sait pas dire — c'est
/// [`crate::ui_window_11::map::exit_effect`], la même question, une seule réponse.
pub const ACTIONS: &[Action] = &[
    Action::plain(MAP, crate::ui_window_11::map::MAP_TIP_KEY, Effect::Opens(crate::ui_window_11::map::ID)),
    Action::plain(
        MAP_ICON,
        crate::ui_window_11::map::MAP_TIP_KEY,
        Effect::Opens(crate::ui_window_11::map::ID),
    ),
    Action::plain(EXIT, "close", Effect::Reopens),
];

/// Le bouton avec cet id.
pub fn action(id: &str) -> Option<Action> {
    ACTIONS.iter().find(|action| action.id == id).copied()
}

/// La liste triée, telle que la fenêtre la tient à l'ouverture.
///
/// Construite **une fois** (`:42-43`) : rien ne la met à jour, et la fenêtre n'a pas de
/// `onUpdate`. Un `Ranking` est donc un instantané, et c'est la seule chose que cette
/// fenêtre ait à dire sur son état.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranking {
    rows: Vec<ColonyInfo>,
}

impl Ranking {
    /// Copie puis trie par prestige **croissant** (`:42-43`).
    ///
    /// Le tri est stable : à prestige égal, l'ordre reçu est conservé.
    pub fn new(colonies: &[ColonyInfo]) -> Ranking {
        let mut rows: Vec<ColonyInfo> = colonies.to_vec();
        rows.sort_by_key(|row| row.prestige);
        Ranking { rows }
    }

    /// Le nombre de lignes.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Vrai quand aucune colonie n'est connue — la liste reste alors vide, sans un mot.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// La colonie d'une ligne.
    pub fn row(&self, index: usize) -> Option<&ColonyInfo> {
        self.rows.get(index)
    }

    /// Le texte de la ligne, `name + ": " + prestige` (`:66`).
    pub fn text(&self, index: usize) -> Option<String> {
        self.rows.get(index).map(row_text)
    }

    /// Le prestige du dernier : le plus haut, puisque le tri monte.
    pub fn highest(&self) -> Option<&ColonyInfo> {
        self.rows.last()
    }

    /// Tous les textes, dans l'ordre de la liste.
    pub fn texts(&self) -> Vec<String> {
        self.rows.iter().map(row_text).collect()
    }
}

/// Le texte d'une ligne (`WindowColonyPrestigeRanking.java:66`).
pub fn row_text(info: &ColonyInfo) -> String {
    format!("{}: {}", info.name, info.prestige)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_window_11::map;

    fn colony(name: &str, prestige: i32) -> ColonyInfo {
        ColonyInfo {
            id: prestige,
            center: [0, 0, 0],
            name: name.to_owned(),
            citizen_count: 10,
            owner: "Simon".to_owned(),
            prestige,
        }
    }

    #[test]
    fn the_ranking_climbs_so_the_best_colony_is_the_last_row() {
        let ranking = Ranking::new(&[
            colony("Alpha", 900),
            colony("Bravo", 100),
            colony("Charlie", 450),
        ]);
        assert_eq!(ranking.len(), 3);
        assert_eq!(
            ranking.texts(),
            ["Bravo: 100", "Charlie: 450", "Alpha: 900"],
            "Comparator.comparing(...) est croissant : le meilleur est en bas (:43)"
        );
        // Le plus haut n'est pas en tête, et c'est la ligne de Java.
        assert_eq!(ranking.highest().map(|row| row.name.as_str()), Some("Alpha"));
        assert_eq!(ranking.row(0).map(|row| row.name.as_str()), Some("Bravo"));
    }

    #[test]
    fn the_sort_is_stable_so_equal_prestige_keeps_the_servers_order() {
        let ranking = Ranking::new(&[
            colony("Premier", 200),
            colony("Deuxieme", 200),
            colony("Troisieme", 200),
        ]);
        assert_eq!(
            ranking.texts(),
            ["Premier: 200", "Deuxieme: 200", "Troisieme: 200"],
            "TimSort est stable, et `sort_by_key` aussi"
        );
        assert_eq!(ranking.row(2).map(|row| row.name.as_str()), Some("Troisieme"));
    }

    #[test]
    fn sorting_does_not_touch_the_maps_list_because_it_is_a_copy() {
        let shared = vec![colony("Alpha", 900), colony("Bravo", 100)];
        let ranking = Ranking::new(&shared);
        assert_eq!(ranking.texts()[0], "Bravo: 100");
        // La carte garde son ordre : les deux fenêtres peuvent diverger sans se voir.
        assert_eq!(
            shared.iter().map(|row| row.name.as_str()).collect::<Vec<&str>>(),
            ["Alpha", "Bravo"]
        );
        // Et une colonie ajoutée après la construction n'apparaît pas : instantané.
        let mut later = shared.clone();
        later.push(colony("Charlie", 50));
        assert_eq!(ranking.len(), 2);
        assert_eq!(Ranking::new(&later).len(), 3);
    }

    #[test]
    fn a_row_reads_the_name_then_the_prestige() {
        assert_eq!(row_text(&colony("Bourg", 42)), "Bourg: 42");
        // Un prestige négatif s'écrit tel quel, sans parenthèses ni signe ajouté.
        assert_eq!(row_text(&colony("Bourg", -7)), "Bourg: -7");
        // Une colonie sans nom ne produit pas un texte vide.
        assert_eq!(row_text(&colony("", 0)), ": 0");
    }

    #[test]
    fn an_empty_ranking_answers_nothing_and_does_not_panic() {
        let ranking = Ranking::new(&[]);
        assert!(ranking.is_empty());
        assert_eq!(ranking.len(), 0);
        assert_eq!(ranking.text(0), None);
        assert_eq!(ranking.highest(), None);
        assert!(ranking.texts().is_empty());
    }

    #[test]
    fn the_window_shows_a_list_and_a_row_text_and_nothing_else() {
        assert_eq!(FIELDS.len(), 2, "`title` n'est pas un panneau que la classe écrit");
        assert_eq!(FIELDS[0].id, COLONIES);
        assert_eq!(FIELDS[0].value, Value::Row);
        assert_eq!(FIELDS[1].id, DESC);
        assert!(FIELDS[1].is_row_pane(), "`desc` est dans la ligne, pas dans la fenêtre");
        // L'id `desc` est aussi celui de l'onglet du builder, dans une autre fenêtre.
        assert_eq!(crate::ui_window_11::builder::DESC, DESC);
        // Les sources : les deux lignes lisent la colonie, jamais le bâtiment.
        assert!(FIELDS.iter().all(|field| field.source == Source::Colony));
    }

    #[test]
    fn the_two_map_buttons_are_one_door_and_the_exit_reopens_the_town_hall() {
        assert_eq!(ACTIONS.len(), 3, "le quatrième bouton de l'XML n'a pas d'id");
        assert_eq!(action(MAP).map(|action| action.effect), Some(Effect::Opens(map::ID)));
        assert_eq!(action(MAP_ICON).map(|action| action.effect), Some(Effect::Opens(map::ID)));
        assert_eq!(action(EXIT).map(|action| action.effect), Some(Effect::Reopens));
        assert!(ACTIONS.iter().all(|action| !action.is_guarded()));
        // Aucun bouton du classement n'envoie de message : la fenêtre n'appelle rien.
        assert!(ACTIONS.iter().all(|action| action.effect != Effect::Reopens || action.id == EXIT));
        assert!(action("mapicon") != action("nowhere"));
        // Son id n'est pas un écran de `src/ui`, comme celui de la carte.
        assert!(crate::ui::screen_by_id(ID).is_err());
        assert_ne!(ID, map::ID);
    }
}
