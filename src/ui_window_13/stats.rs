//! `WindowStatsModule` : ce qu'un batiment compte, sur une periode choisie.
//!
//! Java reference : `core/client/gui/modules/building/WindowStatsModule.java` (240
//! lignes), `gui/layouthuts/layoutstatsmodule.xml` (26), la vue qui l'ouvre
//! (`BuildingStatisticsModuleView.java:30-33`) et les deux cles de module qui la
//! produisent (`BuildingModules.java:62-68`).
//!
//! **Ce qu'elle demande** : le gestionnaire de statistiques **du batiment** (`:104`),
//! les ids qu'il connait (`:105`) et le jour courant de la colonie (`:118`, `:160`).
//! Le stockage n'est pas ici : c'est [`crate::advancements::stats::Stats`], qui porte
//! deja `total`, `in_period` et `ids`.
//!
//! Quatre faits que le source impose :
//!
//! 1. **Le filtre est une photo, le compte ne l'est pas.** Les ids gardes sont
//!    calcules une fois dans l'initialiseur du fournisseur (`:108-136`) ;
//!    `updateElement` ne recalcule que le **nombre** des ids deja gardes
//!    (`:156-161`). Une statistique tombee a zero au filtre ne revient pas sans un
//!    nouvel `updateStats()`, que seuls le bouton et la liste deroulante declenchent.
//! 2. **« Hier » couvre deux jours** : `INTERVAL` donne `1` (`:63`) et la periode est
//!    `getStatsInPeriod(id, day - 1, day)` (`:118`), bornes **incluses**
//!    (`StatisticsManager.java:67-77`).
//! 3. **Rechoisir l'intervalle courant ne fait rien** (`:210-214`).
//! 4. **La ligne porte toujours une infobulle** (`:183`), la ou celle du journal de
//!    quetes n'en a une que si le texte deborde.

use crate::advancements::stats::Stats;
use crate::ui::action::Effect;
use crate::ui::field::{Scope, Value};

use super::pane::{Pane, Source};
use super::refusal::Refusal;
use super::{Button, By, Returns};

/// La classe Java (`:29`).
pub const JAVA: &str = "WindowStatsModule";

/// Le XML (`:88`).
pub const RESOURCE: &str = "minecolonies:gui/layouthuts/layoutstatsmodule.xml";

/// Le titre, dans un `text` sans id (`layoutstatsmodule.xml:4-5`) : la classe mere
/// cherche `desc` (`AbstractModuleWindow.java:63-69`), que ce XML ne declare qu'**a
/// l'interieur** des lignes (`:18`).
pub const TITLE_KEY: &str = "com.minecolonies.core.gui.modules.stats";

/// Le texte de la liste vide (`layoutstatsmodule.xml:17`).
pub const EMPTY_KEY: &str = "com.minecolonies.core.gui.modules.stats.empty";

/// Le libelle du bouton des zeros (`layoutstatsmodule.xml:21`).
pub const HIDE_ZERO_LABEL_KEY: &str = "com.minecolonies.core.gui.modules.stats.hidezerolabel";

/// La liste des statistiques (`layoutstatsmodule.xml:17`).
pub const LIST: &str = "stats";

/// La ligne d'une statistique (`layoutstatsmodule.xml:18`, ecrite a `:163`, `:172`,
/// `:176`, `:181`).
pub const ROW_DESC: &str = "desc";

/// La liste deroulante (`WindowConstants.java:447`, `layoutstatsmodule.xml:7`).
pub const DROPDOWN: &str = "intervals";

/// Le bouton **dans** la liste deroulante (`layoutstatsmodule.xml:14`) : ce n'est pas
/// lui que la classe enregistre.
pub const DROPDOWN_BUTTON: &str = "button";

/// Le bouton des zeros (`:39`, lie a `:89`, clique a `:222`).
pub const HIDE_ZERO: &str = "hidezero";

/// Le prefixe des cles de statistique (`TranslationConstants.java:752`, employe a
/// `:172`, `:176`, `:181`).
pub const PARTIAL_STATS_MODIFIER_NAME: &str = "com.minecolonies.coremod.gui.townhall.stats.";

/// La cle du module d'un batiment quelconque (`BuildingModules.java:63`).
pub const MODULE: &str = "stats_module";

/// La cle du module de la caserne (`BuildingModules.java:67`) : meme vue, meme
/// fenetre, deux cles.
pub const BARRACKS_MODULE: &str = "barracks_stats_module";

/// La ligne porte-t-elle toujours une infobulle ? Oui : `:183` la pose sans condition.
pub const ROW_ALWAYS_HAS_TOOLTIP: bool = true;

/// Les deux panneaux : la liste et la ligne.
pub const FIELDS: &[Pane] = &[
    Pane::new(LIST, Value::Row, Source::BuildingStats, ("layoutstatsmodule.xml", 17)),
    Pane::in_row(LIST, ROW_DESC, Value::Text, Source::BuildingStats, ("layoutstatsmodule.xml", 18)),
];

/// Le bouton des zeros : il bascule un booleen de fenetre et refait la liste.
///
/// `Effect::Toggles` est le mot de `src/ui` pour ce geste (`:225`) ; aucun refus, le
/// clic fait toujours quelque chose.
pub const BUTTONS: &[Button] = &[Button {
    id: HIDE_ZERO,
    label: HIDE_ZERO_LABEL_KEY,
    effect: Effect::Toggles,
    scope: Scope::Window,
    registered: By::Window,
    returns: Returns::Nothing,
    refusals: &[],
}];

/// Une periode d'affichage, avec le nombre de jours que Java lui associe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre entrees de INTERVAL, dans son ordre d'insertion
pub enum Interval {
    /// Un jour en arriere — donc aujourd'hui compris (`:63`).
    Yesterday,
    /// Sept jours (`:64`).
    LastWeek,
    /// Cent jours (`:65`).
    HundredDays,
    /// Tout, code par `-1` (`:66`).
    AllTime,
}

impl Interval {
    /// Les quatre intervalles, dans l'ordre de la `LinkedHashMap` (`:63-66`) — c'est
    /// cet ordre que la liste deroulante affiche (`:199-203`).
    pub const ALL: [Interval; 4] =
        [Interval::Yesterday, Interval::LastWeek, Interval::HundredDays, Interval::AllTime];

    /// Le nombre de jours (`:63-66`) ; `-1` veut dire « tout ».
    pub fn days(self) -> i32 {
        match self {
            Interval::Yesterday => 1,
            Interval::LastWeek => 7,
            Interval::HundredDays => 100,
            Interval::AllTime => -1,
        }
    }

    /// La cle du libelle, celle que `INTERVAL` porte (`:63-66`).
    pub fn key(self) -> &'static str {
        match self {
            Interval::Yesterday => "com.minecolonies.coremod.gui.interval.yesterday",
            Interval::LastWeek => "com.minecolonies.coremod.gui.interval.lastweek",
            Interval::HundredDays => "com.minecolonies.coremod.gui.interval.100days",
            Interval::AllTime => "com.minecolonies.coremod.gui.interval.alltime",
        }
    }

    /// L'intervalle de cette cle, ou [`Refusal::UnknownInterval`].
    ///
    /// Java indexe `INTERVAL` par une cle de sa propre table (`:209`), donc une cle
    /// etrangere rendrait `null` et le `int` qui le deroule leverait (`:110`, `:157`).
    pub fn from_key(key: &str) -> Result<Interval, Refusal> {
        Interval::ALL
            .iter()
            .copied()
            .find(|interval| interval.key() == key)
            .ok_or(Refusal::UnknownInterval)
    }

    /// Le nombre affiche pour cette statistique sur cette periode : la branche de
    /// `:156-161`, `in_period(id, today - days, today)` ou `total(id)` pour « tout ».
    pub fn count(self, stats: &Stats, id: &str, today: i32) -> i32 {
        let days = self.days();
        if days > 0 {
            stats.in_period(id, today - days, today)
        } else {
            stats.total(id)
        }
    }
}

/// L'etat de la fenetre : l'intervalle choisi et le sort des zeros.
///
/// Les deux valeurs par defaut de Java : « hier » (`:72`) et zeros caches (`:79`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub interval: Interval,
    pub hide_zero: bool,
}

impl Default for View {
    fn default() -> View {
        View { interval: Interval::Yesterday, hide_zero: true }
    }
}

impl View {
    /// La fenetre telle qu'elle s'ouvre (`:72`, `:79`).
    pub fn new() -> View {
        View::default()
    }

    /// Les lignes affichees, avec leur nombre.
    ///
    /// `stats.ids()` trie la ou Java rend l'ordre de sa table de hachage
    /// (`StatisticsManager.java:79-82`) : c'est la seule divergence, et un tri est une
    /// reponse stable la ou Java n'en garantit aucune.
    pub fn visible<'a>(&self, stats: &'a Stats, today: i32) -> Vec<(&'a str, i32)> {
        stats
            .ids()
            .filter_map(|id| {
                let count = self.interval.count(stats, id, today);
                if self.hide_zero && count <= 0 {
                    None
                } else {
                    Some((id, count))
                }
            })
            .collect()
    }

    /// Choisit un intervalle ; vrai quand la liste doit etre refaite.
    ///
    /// Faux pour l'intervalle deja choisi : Java compare avant d'agir (`:210-214`).
    pub fn pick(&mut self, interval: Interval) -> bool {
        if self.interval == interval {
            return false;
        }
        self.interval = interval;
        true
    }

    /// Bascule le sort des zeros ; toujours vrai, Java rappelle `updateStats()` sans
    /// condition apres avoir bascule (`:225-238`).
    pub fn toggle_hide_zero(&mut self) -> bool {
        self.hide_zero = !self.hide_zero;
        true
    }
}

/// Le libelle d'une ligne de statistique (`:172-182`).
///
/// La cle est `PARTIAL_STATS_MODIFIER_NAME` suivi du **premier** morceau de l'id ; les
/// arguments sont le nombre, et parfois une seconde traduction.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: les deux formes de Component.translatable de :172-181
pub enum Label {
    /// `translatable(prefixe + id, stat)` — un argument, le nombre (`:181`).
    Count(String),
    /// `translatable(prefixe + modifier, stat, translatable(argument))` (`:172`, `:176`).
    CountAnd(String, String),
}

impl Label {
    /// La cle de traduction de la ligne.
    pub fn key(&self) -> &str {
        match self {
            Label::Count(key) | Label::CountAnd(key, _) => key,
        }
    }
}

/// Le libelle de cette statistique, ou [`Refusal::BrokenStatId`].
///
/// Trois formes (`:165-182`) : sans `;`, la cle est le prefixe suivi de l'id et le
/// nombre est le seul argument ; avec `;`, le premier morceau est le modificateur et le
/// second une seconde traduction ; avec `;` **et** une apostrophe, seule la partie
/// apres l'apostrophe survit — le morceau entre les deux est **abandonne**
/// (`:170-173`, un `todo remove in 1.20.4`).
///
/// Trois ids cassent ce decoupage et Java ne les protege pas : `"a;"` et `"a;b'"`
/// donnent un tableau d'un element la ou Java lit `[1]`, et `"a';b"` fait decouper
/// `split[1]` — un morceau sans apostrophe — parce que le test porte sur l'id entier.
pub fn label(id: &str) -> Result<Label, Refusal> {
    if !id.contains(';') {
        return Ok(Label::Count(format!("{PARTIAL_STATS_MODIFIER_NAME}{id}")));
    }
    let parts = java_split(id, ';');
    let Some(modifier) = parts.first() else {
        return Err(Refusal::BrokenStatId);
    };
    let Some(rest) = parts.get(1) else {
        return Err(Refusal::BrokenStatId);
    };
    let key = format!("{PARTIAL_STATS_MODIFIER_NAME}{modifier}");
    if id.contains('\'') {
        let inner = java_split(*rest, '\'');
        let Some(argument) = inner.get(1) else {
            return Err(Refusal::BrokenStatId);
        };
        Ok(Label::CountAnd(key, (*argument).to_owned()))
    } else {
        Ok(Label::CountAnd(key, (*rest).to_owned()))
    }
}

/// `String.split` de Java, qui **retire les morceaux vides de la fin** — la difference
/// avec `str::split` de Rust, qui les garde.
fn java_split(text: &str, separator: char) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> Stats {
        let mut stats = Stats::new();
        stats.increment_by("crops_harvested", 5, 9).expect("hier");
        stats.increment_by("crops_harvested", 3, 10).expect("aujourd'hui");
        stats.increment_by("crops_harvested", 7, 4).expect("il y a six jours");
        stats.increment_by("items_delivered", 2, 10).expect("aujourd'hui");
        stats
    }

    #[test]
    fn four_intervals_in_the_java_order_and_all_time_is_minus_one() {
        let days: Vec<i32> = Interval::ALL.iter().map(|interval| interval.days()).collect();
        assert_eq!(days, [1, 7, 100, -1], "WindowStatsModule.java:63-66");
        let keys: Vec<&str> = Interval::ALL.iter().map(|interval| interval.key()).collect();
        assert_eq!(
            keys,
            [
                "com.minecolonies.coremod.gui.interval.yesterday",
                "com.minecolonies.coremod.gui.interval.lastweek",
                "com.minecolonies.coremod.gui.interval.100days",
                "com.minecolonies.coremod.gui.interval.alltime",
            ]
        );
        for interval in Interval::ALL {
            assert_eq!(Interval::from_key(interval.key()), Ok(interval));
        }
        assert_eq!(
            Interval::from_key("com.minecolonies.gui.interval.today"),
            Err(Refusal::UnknownInterval)
        );
        assert_eq!(Interval::from_key(""), Err(Refusal::UnknownInterval));
    }

    #[test]
    fn yesterday_is_two_days_and_hiding_the_zeros_drops_the_empty_rows() {
        let stats = stats();
        // today = 10 : « hier » va de 9 a 10 compris, donc 5 + 3 = 8, pas 3.
        assert_eq!(Interval::Yesterday.count(&stats, "crops_harvested", 10), 8);
        assert_eq!(Interval::LastWeek.count(&stats, "crops_harvested", 10), 15);
        assert_eq!(Interval::AllTime.count(&stats, "crops_harvested", 10), 15);
        assert_eq!(Interval::AllTime.count(&stats, "death", 10), 0, "jamais comptee");
        // Par defaut : hier, zeros caches (:72, :79).
        let mut view = View::new();
        assert_eq!(view.interval, Interval::Yesterday);
        assert!(view.hide_zero);
        assert_eq!(
            view.visible(&stats, 10),
            [("crops_harvested", 8), ("items_delivered", 2)],
            "les ids sont tries, Java n'en garantit aucun ordre"
        );
        // Zeros montres : les ids connus reviennent ; une statistique jamais comptee
        // n'a pas d'id du tout, donc elle n'apparait toujours pas.
        view.hide_zero = false;
        assert_eq!(view.visible(&stats, 10), [("crops_harvested", 8), ("items_delivered", 2)]);
        view.interval = Interval::AllTime;
        assert_eq!(view.visible(&stats, 10), [("crops_harvested", 15), ("items_delivered", 2)]);
    }

    #[test]
    fn picking_the_same_interval_rebuilds_nothing_and_the_toggle_always_does() {
        let mut view = View::new();
        assert!(!view.pick(Interval::Yesterday), ":210-214 compare avant d'agir");
        assert!(view.pick(Interval::LastWeek));
        assert_eq!(view.interval, Interval::LastWeek);
        assert!(!view.pick(Interval::LastWeek));
        assert!(view.toggle_hide_zero(), ":225-238 rappelle updateStats() a chaque clic");
        assert!(!view.hide_zero);
        assert!(view.toggle_hide_zero());
        assert!(view.hide_zero);
        assert_eq!(view.interval, Interval::LastWeek, "le bouton ne touche pas la periode");
    }

    #[test]
    fn the_label_has_three_shapes_and_three_ids_that_break_it() {
        assert_eq!(
            label("death"),
            Ok(Label::Count("com.minecolonies.coremod.gui.townhall.stats.death".to_owned()))
        );
        assert_eq!(
            label("food;minecraft:cake"),
            Ok(Label::CountAnd(
                "com.minecolonies.coremod.gui.townhall.stats.food".to_owned(),
                "minecraft:cake".to_owned()
            ))
        );
        // L'apostrophe : le morceau entre `;` et `'` est abandonne (:170-173).
        assert_eq!(
            label("food;STALE'com.minecolonies.tag.cake"),
            Ok(Label::CountAnd(
                "com.minecolonies.coremod.gui.townhall.stats.food".to_owned(),
                "com.minecolonies.tag.cake".to_owned()
            ))
        );
        assert_eq!(
            label("death").map(|label| label.key().to_owned()),
            Ok(format!("{PARTIAL_STATS_MODIFIER_NAME}death"))
        );
        // Les trois ids qui levent chez Java.
        assert_eq!(label("food;"), Err(Refusal::BrokenStatId), ":167 split[1]");
        assert_eq!(label("food;cake'"), Err(Refusal::BrokenStatId), ":171 split2[1]");
        assert_eq!(label("food';cake"), Err(Refusal::BrokenStatId), ":171 mauvais morceau");
        // Un id vide ne casse rien : la cle est le prefixe seul, comme en Java.
        assert_eq!(label(""), Ok(Label::Count(PARTIAL_STATS_MODIFIER_NAME.to_owned())));
    }

    #[test]
    fn one_button_always_tooltipped_and_the_stats_tab_is_not_in_src_ui() {
        assert_eq!(BUTTONS.len(), 1);
        let button = BUTTONS[0];
        assert_eq!(button.id, HIDE_ZERO, "WindowStatsModule.java:39");
        assert_eq!(button.effect, Effect::Toggles);
        assert!(button.refusals.is_empty());
        assert_eq!(button.registered, By::Window);
        // Le bouton de la liste deroulante est dans le XML, personne ne l'enregistre.
        assert!(!BUTTONS.iter().any(|button| button.id == DROPDOWN_BUTTON));
        assert_ne!(DROPDOWN, DROPDOWN_BUTTON);
        // La ligne, elle, porte toujours son infobulle (:183), contrairement a celle
        // du journal de quetes qui n'en a une que si le texte deborde.
        assert!(ROW_ALWAYS_HAS_TOOLTIP);
        assert!(crate::ui_window_13::quest_log::tooltip(400, 130));
        assert!(!crate::ui_window_13::quest_log::tooltip(120, 130));
        // Deux cles de module, une vue, une fenetre ; et aucune n'est dans src/ui.
        assert_ne!(MODULE, BARRACKS_MODULE);
        assert_eq!(crate::ui::table::module_screen(MODULE), None);
        assert_eq!(crate::ui::table::module_screen(BARRACKS_MODULE), None);
        assert_eq!(Source::BuildingStats.to_ui(), Some(crate::ui::Source::Module(MODULE)));
        // Et la fenetre n'affiche que deux panneaux.
        let ids: Vec<&str> = FIELDS.iter().map(|pane| pane.id).collect();
        assert_eq!(ids, [LIST, ROW_DESC]);
        assert_eq!(FIELDS[1].scope, Scope::Row(LIST));
    }
}
