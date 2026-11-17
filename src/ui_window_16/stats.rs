//! `WindowStatsPage` : la population de la colonie, et ses compteurs par periode.
//!
//! Java reference : `core/client/gui/townhall/WindowStatsPage.java` (298 lignes),
//! `gui/townhall/layoutstats.xml` (26), la constante de l'effet de recherche
//! (`api/research/util/ResearchConstants.java:107`), la limite par defaut
//! (`api/util/constant/CitizenConstants.java:13`), les trois avertissements
//! (`TranslationConstants.java:544-550`) et la configuration du serveur
//! (`ServerConfiguration.java:129` pour `maxcitizenpercolony`).
//!
//! **Ce qu'elle demande** : les citoyens de la colonie (`:84`, `:161`), la limite de
//! population de la colonie (`:100`, `:102`, `getCitizenCountLimit`), la force de
//! l'effet de recherche `CITIZEN_CAP` (`:87-90`), la limite configuree du serveur
//! (`:90`, `:94`), le gestionnaire de statistiques de la colonie (`:229`, `:251`), le
//! jour de la colonie (`:255`) et, pour le tableau des metiers, **les modules de
//! travail de chaque batiment** (`:128-157`).
//!
//! Cinq faits que le source impose :
//!
//! 1. **La limite affichee n'est pas une seule limite.** Trois plafonds se combinent :
//!    l'effet de recherche (plancher 25), la limite configuree du serveur, et la limite
//!    de logement de la colonie. `citizensCap = min(max(25, effet), config)` (`:89-90`)
//!    et la limite de logement s'affiche a part (`:100`).
//! 2. **Les trois couleurs sont trois avertissements diffe rents.** Vert sous 90 % des
//!    deux plafonds (`:102-105`) ; orange sous le plafond de recherche seulement, avec
//!    « Needs Housing » (`:106-110`) ; rouge au plafond, avec « Research at University »
//!    si le plafond vient de la recherche et « Reached Configured Limit » sinon
//!    (`:111-123`).
//! 3. **Le tableau des metiers n'est pas groupe par metier mais par nom affiche
//!    minuscule** (`:151-154`). Deux metiers qui partagent un nom affiche tombent sur une
//!    seule ligne — aucun ne le fait aujourd'hui, et un test compte les cinquante.
//! 4. **Les deux modules a limite partagee se retirent mutuellement leurs places**
//!    (`:134-148`) : la ligne affiche `max - tout_le_batiment + soi`, et la somme des
//!    lignes d'un batiment n'est donc **pas** sa limite.
//! 5. **Les enfants passent avant les chomeurs.** `getElementCount` rend `metiers + 2`
//!    (`:191`) et l'indice `metiers` tombe dans la branche `else` (`:211-218`) : la
//!    ligne « Children » precede la ligne « Unemployed ».
//!
//! **L'intervalle est celui de `WindowStatsModule`, pas une deuxieme copie.**
//! `WindowStatsPage.INTERVAL` (`:41-44`) et `WindowStatsModule.INTERVAL` sont la meme
//! table, ecrite deux fois en Java ; le port garde **une** copie
//! ([`crate::ui_window_13::stats::Interval`]) et cette page la lit. Ce qui differe
//! vraiment entre les deux ecrans est ailleurs : ici il n'y a **ni filtre, ni bouton de
//! zeros** — `layoutstats.xml` n'en declare aucun — et le libelle d'une statistique est
//! une simple concatenation (`:259`), la ou l'onglet d'un batiment decoupe l'id en trois
//! formes (`WindowStatsModule.java:165-182`).

use std::collections::BTreeMap;

use crate::advancements::stats::Stats;
use crate::jobs::Job;
use crate::ui::field::Value;
use crate::ui_window_13::stats::PARTIAL_STATS_MODIFIER_NAME;

use super::pane::{Pane, Source};
use super::refusal::Refusal;

/// Re-export de la table d'intervalles **unique** du port
/// (`WindowStatsPage.java:41-44` == `WindowStatsModule.java:63-66`).
pub use crate::ui_window_13::stats::Interval;

/// La classe Java (`WindowStatsPage.java:32`).
pub const JAVA: &str = "WindowStatsPage";

/// Le XML (`:64`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/layoutstats.xml";

/// L'id du bouton de l'onglet — **`happiness`, pas `stats`**
/// (`WindowConstants.java:62`). Le libelle, lui, dit « Statistics »
/// (`windowtownhall.xml:26`), et `stats` est le nom de la liste des compteurs
/// (`layoutstats.xml:22`).
pub const TAB_ID: &str = "happiness";

/// Le libelle de cet onglet (`windowtownhall.xml:26`).
pub const TAB_KEY: &str = "com.minecolonies.coremod.gui.townhalltab.stats";

/// La ligne « n / limite » (`layoutstats.xml:4`, `WindowConstants.java:237`).
pub const TOTAL_CITIZENS: &str = "totalCitizens";

/// Le libelle dessine juste avant, dans un `text` **sans id**
/// (`layoutstats.xml:3`) — donc attache au panneau ci-dessus.
pub const TOTAL_CITIZENS_KEY: &str = "com.minecolonies.coremod.gui.townhall.population.totalcitizens";

/// La liste de la population (`layoutstats.xml:5`) — avec un tiret, comme le XML.
pub const CITIZEN_LIST: &str = "citizen-stats";

/// Le texte d'une ligne de cette liste (`layoutstats.xml:7`,
/// `WindowConstants.java:332`).
pub const CITIZEN_AMOUNT: &str = "citizensAmount";

/// La liste deroulante (`layoutstats.xml:12`, `WindowConstants.java:447`). **Ce n'est
/// pas un bouton** : la classe lui pose un `setHandler` (`:264`), elle n'appelle jamais
/// `registerButton`.
pub const DROPDOWN: &str = "intervals";

/// Le bouton **dans** la liste deroulante (`layoutstats.xml:19`) : personne ne
/// l'enregistre non plus.
pub const DROPDOWN_BUTTON: &str = "button";

/// La liste des compteurs (`layoutstats.xml:22`). `desc` y est aussi le nom d'une ligne
/// de l'onglet d'un batiment : deux fenetres, deux listes, meme id.
pub const STATS_LIST: &str = "stats";

/// Le texte d'une ligne de compteur (`layoutstats.xml:23`, ecrit a `:258`).
pub const STATS_ROW: &str = "desc";

/// Le format de la ligne « n / limite » (`TranslationConstants.java:222`).
pub const COUNT_KEY: &str = "com.minecolonies.coremod.gui.townhall.population.totalcitizens.count";

/// Le format d'une ligne de metier, trois arguments : nom, presents, places
/// (`TranslationConstants.java:226`).
pub const EACH_KEY: &str = "com.minecolonies.coremod.gui.townhall.population.each";

/// Le format de la ligne des enfants (`TranslationConstants.java:224`).
pub const CHILDS_KEY: &str = "com.minecolonies.coremod.gui.townhall.population.childs";

/// Le format de la ligne des chomeurs (`TranslationConstants.java:228`).
pub const UNEMPLOYED_KEY: &str = "com.minecolonies.coremod.gui.townhall.population.unemployed";

/// L'avertissement « il manque des logements » (`TranslationConstants.java:544`).
pub const HOUSING_WARNING_KEY: &str =
    "com.minecolonies.coremod.gui.townhall.population.totalcitizens.houselimited";

/// L'avertissement « la recherche bloque » (`TranslationConstants.java:546`).
pub const RESEARCH_WARNING_KEY: &str =
    "com.minecolonies.coremod.gui.townhall.population.totalcitizens.researchlimited";

/// L'avertissement « la configuration du serveur bloque »
/// (`TranslationConstants.java:549`).
pub const CONFIG_WARNING_KEY: &str =
    "com.minecolonies.coremod.gui.townhall.population.totalcitizens.configlimited";

/// Le plancher du plafond de recherche (`CitizenConstants.java:13`).
pub const CITIZEN_LIMIT_DEFAULT: i32 = 25;

/// L'effet de recherche qui releve le plafond (`ResearchConstants.java:107`).
pub const CITIZEN_CAP_EFFECT: &str = "minecolonies:effects/citizencapaddition";

/// La marge sous laquelle la ligne est verte : 90 % des deux plafonds
/// (`WindowStatsPage.java:102`).
pub const HEADROOM: f64 = 0.9;

/// Les trois couleurs de la ligne des habitants (`AbstractWindowTownHall.java:19-25`,
/// appliquees a `:104`, `:109`, `:122`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois branches, trois couleurs
pub enum Colour {
    /// Sous 90 % des deux plafonds.
    DarkGreen,
    /// Sous le plafond de recherche, au-dessus du seuil vert : il manque des logements.
    Orange,
    /// Au plafond : la colonie ne peut pas grandir.
    Red,
}

/// La couleur de la ligne, telle que Java la choisit (`:102-123`).
///
/// `cap` est le plafond **calcule** ([`citizen_cap`]) et `limit` la limite de logement
/// de la colonie (`getCitizenCountLimit`). Les deux sont necessaires : une colonie peut
/// etre verte pour la recherche et rouge pour le logement.
pub fn colour(citizens: i32, cap: i32, limit: i32) -> Colour {
    let green = (citizens as f64) < (cap as f64 * HEADROOM)
        && (citizens as f64) < (limit as f64 * HEADROOM);
    if green {
        Colour::DarkGreen
    } else if citizens < cap {
        Colour::Orange
    } else {
        Colour::Red
    }
}

/// L'avertissement en infobulle, `None` quand la ligne est verte.
///
/// **L'infobulle est posee dans les quatre cas** (`:124`) : verte, elle est vide. Ce
/// `None` est donc « rien a dire », pas « pas d'infobulle ».
///
/// La branche rouge choisit entre recherche et configuration en comparant le plafond
/// calcule a la limite configuree (`:113`) : un plafond de recherche qui atteint la
/// limite configuree n'est plus une limite de recherche.
pub fn warning(citizens: i32, cap: i32, limit: i32, config_max: i32) -> Option<&'static str> {
    match colour(citizens, cap, limit) {
        Colour::DarkGreen => None,
        Colour::Orange => Some(HOUSING_WARNING_KEY),
        Colour::Red => {
            if cap < config_max {
                Some(RESEARCH_WARNING_KEY)
            } else {
                Some(CONFIG_WARNING_KEY)
            }
        }
    }
}

/// Le denominateur de la ligne « n / m » (`:100`, `:121`).
///
/// Tant qu'il reste de la place, Java affiche `max(habitants, limite de logement)` —
/// une limite de logement superieure au nombre d'habitants se lit donc. Au plafond, il
/// reecrit la meme ligne avec le plafond calcule (`:121`), et le denominateur devient ce
/// plafond.
pub fn denominator(citizens: i32, cap: i32, limit: i32) -> i32 {
    if citizens < cap {
        citizens.max(limit)
    } else {
        cap
    }
}

/// Le plafond de population de la colonie (`:85-95`).
///
/// Avec l'effet : `min(max(25, force), limite configuree)`. Sans : la limite configuree.
/// Le `max(25, …)` est un **plancher de recherche** : un effet plus faible que la
/// limite par defaut ne rabaisse rien.
pub fn citizen_cap(research_effect: Option<i32>, config_max: i32) -> i32 {
    match research_effect {
        Some(strength) => strength.max(CITIZEN_LIMIT_DEFAULT).min(config_max),
        None => config_max,
    }
}

/// Un module de travail d'un batiment, tel que le tableau des metiers le lit
/// (`:132-149`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module {
    /// Le metier du module (`module.getJobDisplayName()`, `:151`).
    pub job: Job,
    /// Les places du module (`getMaxInhabitants()`, `:143`).
    pub max: i32,
    /// Les citoyens affectes (`getAssignedCitizens().size()`, `:149`).
    pub assigned: i32,
    /// Vrai quand la vue est un `CombinedHiringLimitModuleView` (`:135`) : la limite est
    /// celle du **batiment**, pas du module (`CombinedHiringLimitModuleView.java:12-15`).
    pub combined: bool,
}

/// Les quatorze modules dont la vue est un `CombinedHiringLimitModuleView`
/// (`BuildingModules.java:491`, `:508`, `:549-563`, `:569-578`, `:580-585`).
///
/// **Quatorze, pas seize.** `CONNECTION_MODULE` est declare avec ce type de vue dans
/// ses generiques (`:633`) mais fabrique un `ColonyConnectionModuleView`, qui n'herite
/// meme pas de `WorkerBuildingModuleView` : la boucle de `WindowStatsPage.java:132` ne le
/// voit jamais. Et `CAVALRY_STABLE_WORK` (`:565-567`) fabrique la vue ordinaire.
pub const COMBINED_VIEWS: [&str; 14] = [
    "miner_work",
    "quarrier_work",
    "knight_barracks_work",
    "ranger_barracks_work",
    "druid_barracks_work",
    "marksman_barracks_work",
    "huscarl_barracks_work",
    "knight_tower_work",
    "ranger_tower_work",
    "druid_tower_work",
    "marksman_tower_work",
    "huscarl_tower_work",
    "knight_gate_work",
    "ranger_gate_work",
];

/// Vrai quand ce module partage sa limite avec les autres modules du batiment
/// (`WindowStatsPage.java:135`).
pub fn is_combined(module_key: &str) -> bool {
    COMBINED_VIEWS.contains(&module_key)
}

/// Les places qu'une ligne affiche (`:134-148`), exactement :
///
/// ```text
/// int alreadyAssigned = -1;
/// if (module instanceof CombinedHiringLimitModuleView) {
///     alreadyAssigned = 0;
///     for (chaque module du batiment) alreadyAssigned += module.getAssignedCitizens().size();
/// }
/// int max = module.getMaxInhabitants();
/// if (alreadyAssigned != -1) { max -= alreadyAssigned; max += module.getAssignedCitizens().size(); }
/// ```
///
/// `siblings` est donc **tout le batiment, ce module compris**, et non les autres
/// modules. Pour un module ordinaire, `max` ne bouge pas. Le resultat peut etre negatif :
/// cinq modules de tour a une place, six gardes affectes, et une ligne affiche `-5` —
/// Java ne borne pas.
pub fn effective_max(max: i32, siblings: i32, own: i32, combined: bool) -> i32 {
    if combined {
        max - siblings + own
    } else {
        max
    }
}

/// Les lignes d'**un** batiment : son metier, ses presents et ses places affichees
/// (`:132-157`, la partie batiment).
pub fn building_lines(modules: &[Module]) -> Vec<(Job, i32, i32)> {
    let siblings: i32 = modules.iter().map(|module| module.assigned).sum();
    modules
        .iter()
        .map(|module| {
            (
                module.job,
                module.assigned,
                effective_max(module.max, siblings, module.assigned, module.combined),
            )
        })
        .collect()
}

/// Le nom affiche d'un metier, tel que Java l'utilise comme **cle de groupe**
/// (`:151`) : `getJobDisplayName().toLowerCase(Locale.US)`.
pub fn job_key(job: Job) -> String {
    job.display().to_lowercase()
}

/// La mise en capitale d'une ligne (`:204`) : `substring(0, 1).toUpperCase()` puis le
/// reste. Java **leve** sur une chaine vide ; le port nomme le refus.
pub fn capitalize(name: &str) -> Result<String, Refusal> {
    let mut chars = name.chars();
    let first = chars.next().ok_or(Refusal::EmptyJobName)?;
    let mut out: String = first.to_uppercase().collect();
    out.push_str(chars.as_str());
    Ok(out)
}

/// Une ligne du tableau de la population (`:194-219`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: une ligne de metier, puis les deux lignes fixes
pub enum Row {
    /// Un metier : nom affiche, presents, places affichees (`:206`).
    Job { name: String, assigned: i32, max: i32 },
    /// Les enfants (`:217`) — **avant** les chomeurs.
    Children { count: i32 },
    /// Les citoyens sans metier (`:213`).
    Unemployed { count: i32 },
}

impl Row {
    /// La cle du format de cette ligne.
    pub fn key(&self) -> &'static str {
        match self {
            Row::Job { .. } => EACH_KEY,
            Row::Children { .. } => CHILDS_KEY,
            Row::Unemployed { .. } => UNEMPLOYED_KEY,
        }
    }

    /// Le nom affiche d'une ligne de metier, `None` pour les deux lignes fixes.
    pub fn name(&self) -> Option<&str> {
        match self {
            Row::Job { name, .. } => Some(name.as_str()),
            Row::Children { .. } | Row::Unemployed { .. } => None,
        }
    }

    /// Les deux nombres de la ligne, `None` pour les deux lignes fixes.
    pub fn counts(&self) -> Option<(i32, i32)> {
        match self {
            Row::Job { assigned, max, .. } => Some((*assigned, *max)),
            Row::Children { .. } | Row::Unemployed { .. } => None,
        }
    }
}

/// Le tableau de la population, tel que `createAndSetStatistics` le remplit
/// (`:182-221`).
///
/// `buildings` est la liste des modules de travail batiment par batiment : Java parcourt
/// `getClientBuildingManager().getBuildings()` et, pour chacun, ses vues de module
/// (`:128-157`). Les deux compteurs sont ceux de `:159-171`, que la fenetre calcule sur
/// les citoyens et que ce modele ne porte pas.
///
/// L'ordre est celui de Java : les metiers tries par nom (`:184`), puis les enfants, puis
/// les chomeurs (`:211-218`).
pub fn population(
    buildings: &[&[Module]],
    children: i32,
    unemployed: i32,
) -> Result<Vec<Row>, Refusal> {
    let mut grouped: BTreeMap<String, (i32, i32)> = BTreeMap::new();
    for modules in buildings {
        for (job, assigned, max) in building_lines(modules) {
            let entry = grouped.entry(job_key(job)).or_insert((0, 0));
            entry.0 += assigned;
            entry.1 += max;
        }
    }
    let mut rows = Vec::with_capacity(grouped.len() + 2);
    for (key, (assigned, max)) in grouped {
        rows.push(Row::Job { name: capitalize(&key)?, assigned, max });
    }
    rows.push(Row::Children { count: children });
    rows.push(Row::Unemployed { count: unemployed });
    Ok(rows)
}

/// L'etat de la page : l'intervalle choisi, et rien d'autre.
///
/// Java garde une **cle** (`selectedInterval`, `:55`), que la liste deroulante fait
/// correspondre a son indice (`:280`). Le port garde l'intervalle lui-meme : une cle
/// inconnue devient alors impossible, la ou Java leverait (`:110`, `:157`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageState {
    pub interval: Interval,
}

impl Default for PageState {
    fn default() -> PageState {
        PageState { interval: Interval::Yesterday }
    }
}

impl PageState {
    /// La page telle qu'elle s'ouvre : « hier » (`:55`).
    pub fn new() -> PageState {
        PageState::default()
    }

    /// Choisit un intervalle ; vrai quand la liste doit etre refaite.
    ///
    /// Faux pour l'intervalle deja choisi : `onDropDownListChanged` compare les deux
    /// chaines avant d'agir (`:286-290`).
    pub fn pick(&mut self, interval: Interval) -> bool {
        if self.interval == interval {
            return false;
        }
        self.interval = interval;
        true
    }

    /// Le nombre affiche pour cette statistique (`:251-256`), la branche de
    /// `WindowStatsPage` elle-meme : somme totale, ou somme sur `jour - intervalle` a
    /// `jour`, bornes comprises.
    pub fn count(&self, stats: &Stats, id: &str, today: i32) -> i32 {
        self.interval.count(stats, id, today)
    }
}

/// La cle du libelle d'une statistique : **le prefixe et l'id, sans decoupage**
/// (`:259`).
///
/// L'onglet d'un batiment, lui, decoupe l'id sur `;` et `'`
/// (`WindowStatsModule.java:165-182`) ; cette page ne le fait pas, et un id contenant un
/// `;` donne donc ici une cle qui contient un `;`.
pub fn stat_key(id: &str) -> String {
    format!("{PARTIAL_STATS_MODIFIER_NAME}{id}")
}

/// Les lignes de la liste des compteurs, dans l'ordre du gestionnaire (`:229-260`).
///
/// Java prend `getStatTypes()` — l'ordre d'une `HashSet`, donc aucun — et le port rend
/// l'ordre trie de [`Stats::ids`]. C'est la seule divergence, et un tri est une reponse
/// stable la ou Java n'en garantit aucune.
pub fn stats_rows(state: &PageState, stats: &Stats, today: i32) -> Vec<(String, i32)> {
    stats
        .ids()
        .map(|id| (stat_key(id), state.count(stats, id, today)))
        .collect()
}

/// Les cinq panneaux de la page.
pub static PANES: [Pane; 5] = [
    Pane::labelled(
        TOTAL_CITIZENS,
        TOTAL_CITIZENS_KEY,
        Value::Occupancy,
        Source::Colony,
        ("layoutstats.xml", 4),
    ),
    Pane::new(CITIZEN_LIST, Value::Row, Source::Colony, ("layoutstats.xml", 5)),
    Pane::inside(
        CITIZEN_LIST,
        CITIZEN_AMOUNT,
        Value::Text,
        Source::Colony,
        ("layoutstats.xml", 7),
    ),
    Pane::new(STATS_LIST, Value::Row, Source::Colony, ("layoutstats.xml", 22)),
    Pane::inside(STATS_LIST, STATS_ROW, Value::Text, Source::Colony, ("layoutstats.xml", 23)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Scope;

    fn stats() -> Stats {
        let mut stats = Stats::new();
        stats.increment_by("crops_harvested", 5, 9).expect("hier");
        stats.increment_by("crops_harvested", 3, 10).expect("aujourd'hui");
        stats.increment_by("death", 1, 1).expect("il y a neuf jours");
        stats
    }

    #[test]
    fn the_interval_table_is_the_one_of_the_module_window() {
        // Les deux tables Java sont identiques : le port n'en garde qu'une.
        let days: Vec<i32> = Interval::ALL.iter().map(|interval| interval.days()).collect();
        assert_eq!(days, [1, 7, 100, -1], "WindowStatsPage.java:41-44");
        let keys: Vec<&str> = Interval::ALL.iter().map(|interval| interval.key()).collect();
        assert_eq!(
            keys[0],
            "com.minecolonies.coremod.gui.interval.yesterday",
            "la valeur initiale de selectedInterval, :55"
        );
        // La page s'ouvre sur « hier » et rechoisir cet intervalle ne refait rien.
        let mut state = PageState::new();
        assert_eq!(state.interval, Interval::Yesterday);
        assert!(!state.pick(Interval::Yesterday), ":286-290 compare avant d'agir");
        assert!(state.pick(Interval::AllTime));
        assert_eq!(state.interval, Interval::AllTime);
        assert!(!state.pick(Interval::AllTime));
    }

    #[test]
    fn the_cap_floors_the_research_at_twenty_five_and_the_config_caps_it() {
        // Sans effet : la limite configuree, quelle qu'elle soit.
        assert_eq!(citizen_cap(None, 250), 250);
        // Effet plus faible que le plancher : 25, pas 10.
        assert_eq!(citizen_cap(Some(10), 250), CITIZEN_LIMIT_DEFAULT);
        // Effet utile : il passe, mais la configuration le plafonne.
        assert_eq!(citizen_cap(Some(100), 250), 100);
        assert_eq!(citizen_cap(Some(400), 250), 250, ":90 min(effet, config)");
        assert_eq!(citizen_cap(Some(25), 250), 25, "le plancher lui-meme");
        // La limite configuree ne descend pas sous 25 en jeu
        // (ServerConfiguration.java:129), mais l'ordre des deux bornes compte.
        assert_eq!(citizen_cap(Some(100), 20), 20);
    }

    #[test]
    fn three_colours_and_three_warnings() {
        // 8 habitants, plafond 10, logement 10 : sous 90 % des deux.
        assert_eq!(colour(8, 10, 10), Colour::DarkGreen);
        assert_eq!(warning(8, 10, 10, 250), None);
        // 9 : plus sous 90 %, toujours sous le plafond de recherche.
        assert_eq!(colour(9, 10, 10), Colour::Orange);
        assert_eq!(warning(9, 10, 10, 250), Some(HOUSING_WARNING_KEY));
        // 10 : au plafond, et le plafond vient de la recherche.
        assert_eq!(colour(10, 10, 10), Colour::Red);
        assert_eq!(warning(10, 10, 10, 250), Some(RESEARCH_WARNING_KEY));
        // Le meme rouge, mais le plafond EST la configuration : autre phrase.
        assert_eq!(warning(250, 250, 250, 250), Some(CONFIG_WARNING_KEY));
        assert_ne!(RESEARCH_WARNING_KEY, CONFIG_WARNING_KEY);
        // Vert pour le logement, rouge pour la recherche : le logement suffit.
        assert_eq!(colour(10, 10, 100), Colour::Red);
        assert_eq!(colour(5, 100, 4), Colour::Orange, "90 % du logement seul");
    }

    #[test]
    fn the_denominator_shows_the_housing_limit_until_the_cap_is_reached() {
        // Sous le plafond : max(habitants, limite de logement).
        assert_eq!(denominator(5, 10, 10), 10);
        assert_eq!(denominator(5, 10, 12), 12, ":100 max(citizensSize, limit)");
        assert_eq!(denominator(5, 10, 3), 5, "jamais moins que les habitants");
        // Au plafond : la ligne est reecrite avec le plafond (:121).
        assert_eq!(denominator(12, 10, 12), 10);
        assert_eq!(denominator(12, 10, 100), 10);
    }

    #[test]
    fn a_combined_module_loses_what_its_siblings_hold() {
        // Un batiment de gardes a cinq modules d'une place par niveau, niveau 5.
        let modules = [
            Module { job: Job::Knight, max: 5, assigned: 1, combined: true },
            Module { job: Job::Ranger, max: 5, assigned: 1, combined: true },
            Module { job: Job::Druid, max: 5, assigned: 0, combined: true },
            Module { job: Job::Marksman, max: 5, assigned: 0, combined: true },
            Module { job: Job::Huscarl, max: 5, assigned: 0, combined: true },
        ];
        let lines = building_lines(&modules);
        assert_eq!(lines.len(), 5);
        // max - tout le batiment (2) + soi.
        assert_eq!(lines[0].2, 4);
        assert_eq!(lines[1].2, 4);
        assert_eq!(lines[2].2, 3);
        assert_eq!(lines[3].2, 3);
        assert_eq!(lines[4].2, 3);
        // La somme des lignes n'est pas la limite du batiment : `isFull` compare les
        // affectes du batiment entier a UNE place de module
        // (CombinedHiringLimitModuleView.java:12-15).
        let shown: i32 = lines.iter().map(|(_, _, max)| max).sum();
        assert_eq!(shown, 17);
        assert_ne!(shown, 5);
        // Un module ordinaire ne bouge pas.
        assert_eq!(effective_max(3, 2, 1, false), 3);
        // Et rien ne borne le resultat : Java peut afficher une place negative.
        let tower = [Module { job: Job::Knight, max: 1, assigned: 0, combined: true }];
        assert_eq!(building_lines(&tower)[0].2, 1);
        let crowded = [
            Module { job: Job::Knight, max: 1, assigned: 0, combined: true },
            Module { job: Job::Ranger, max: 1, assigned: 6, combined: true },
        ];
        assert_eq!(building_lines(&crowded)[0].2, -5);
        // Quatorze modules partagent leur limite, et deux faux amis n'en sont pas.
        assert_eq!(COMBINED_VIEWS.len(), 14);
        assert!(is_combined("quarrier_work"));
        assert!(is_combined("knight_gate_work"));
        assert!(!is_combined("colony_connections"), "la vue n'herite pas du type");
        assert!(!is_combined("cavalry_stable_work"));
        let mut unique = COMBINED_VIEWS.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 14, "deux fois le meme module");
    }

    #[test]
    fn the_population_rows_put_the_jobs_first_then_children_then_unemployed() {
        let one = [
            Module { job: Job::Baker, max: 2, assigned: 1, combined: false },
            // La variante s'appelle `Deliveryman`, le nom affiche est « Courier »
            // (`jobs/job.rs:98`) : le groupe est le **nom affiche** (`:151`), pas le metier.
            Module { job: Job::Deliveryman, max: 3, assigned: 2, combined: false },
        ];
        let other = [Module { job: Job::Baker, max: 2, assigned: 2, combined: false }];
        let one: &[Module] = &one;
        let other: &[Module] = &other;
        let rows = population(&[one, other], 4, 3).expect("deux noms non vides");
        // Deux metiers, mais le boulanger est dans deux batiments : deux lignes sommees.
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], Row::Job { name: "Baker".to_owned(), assigned: 3, max: 4 });
        assert_eq!(rows[1], Row::Job { name: "Courier".to_owned(), assigned: 2, max: 3 });
        assert_eq!(rows[2], Row::Children { count: 4 });
        assert_eq!(rows[3], Row::Unemployed { count: 3 });
        assert_eq!(rows[0].key(), EACH_KEY);
        assert_eq!(rows[2].key(), CHILDS_KEY);
        assert_eq!(rows[3].key(), UNEMPLOYED_KEY);
        assert_eq!(rows[0].counts(), Some((3, 4)));
        assert_eq!(rows[2].counts(), None);
        assert_eq!(rows[2].name(), None);
        // Aucun batiment : les deux lignes fixes restent.
        let empty = population(&[], 0, 0).expect("rien a nommer");
        assert_eq!(empty, [Row::Children { count: 0 }, Row::Unemployed { count: 0 }]);
        // La mise en capitale, et le vide que Java ne protege pas (:204).
        assert_eq!(capitalize("baker").expect("non vide"), "Baker");
        assert_eq!(capitalize("archer squire").expect("non vide"), "Archer squire");
        assert!(capitalize("").is_err());
        assert_eq!(capitalize(""), Err(Refusal::EmptyJobName));
        // Les cinquante noms affiches sont distincts une fois minuscules : aucune ligne
        // ne fusionne aujourd'hui, meme si le groupe est le nom et non le metier.
        let mut keys: Vec<String> = Job::ALL.iter().map(|job| job_key(*job)).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), Job::ALL.len());
        assert_eq!(keys.len(), 50);
    }

    #[test]
    fn the_stats_list_reads_the_same_store_as_the_building_tab_without_splitting_ids() {
        let stats = stats();
        let mut state = PageState::new();
        // « hier » va de 9 a 10 compris : 5 + 3, et `death` (jour 1) est hors periode.
        assert_eq!(state.count(&stats, "crops_harvested", 10), 8);
        assert_eq!(state.count(&stats, "death", 10), 0);
        let rows = stats_rows(&state, &stats, 10);
        assert_eq!(
            rows,
            [
                ("com.minecolonies.coremod.gui.townhall.stats.crops_harvested".to_owned(), 8),
                ("com.minecolonies.coremod.gui.townhall.stats.death".to_owned(), 0),
            ],
            "l'ordre est celui de Stats::ids, Java n'en garantit aucun"
        );
        // Tout l'historique : la mort revient, et la somme totale aussi.
        assert!(state.pick(Interval::AllTime));
        assert_eq!(state.count(&stats, "death", 10), 1);
        assert_eq!(state.count(&stats, "crops_harvested", 10), 8);
        // Le libelle ne decoupe rien : la ou WindowStatsModule.java:165-182 couperait.
        assert_eq!(stat_key("death"), "com.minecolonies.coremod.gui.townhall.stats.death");
        assert_eq!(
            stat_key("food;minecraft:cake"),
            "com.minecolonies.coremod.gui.townhall.stats.food;minecraft:cake",
            "WindowStatsPage.java:259 concatene l'id entier"
        );
        assert_ne!(
            stat_key("food;minecraft:cake"),
            {
                let split = crate::ui_window_13::stats::label("food;minecraft:cake")
                    .expect("un id qui se decoupe");
                split.key().to_owned()
            },
            "l'onglet d'un batiment, lui, decoupe"
        );
    }

    #[test]
    fn five_panes_two_lists_one_line_each_and_no_button() {
        assert_eq!(PANES.len(), 5);
        let lists: Vec<&str> = PANES
            .iter()
            .filter(|pane| pane.value == Value::Row)
            .map(|pane| pane.id)
            .collect();
        assert_eq!(lists, [CITIZEN_LIST, STATS_LIST]);
        for pane in PANES {
            assert_eq!(pane.source, Source::Colony);
            assert_eq!(pane.declared.0, "layoutstats.xml");
            if let Scope::Row(parent) = pane.scope {
                assert!(lists.contains(&parent), "{} n'est pas une liste", parent);
            }
        }
        // La ligne des habitants est la seule a porter un libelle, et c'est un n/max.
        let citizens = PANES[0];
        assert_eq!(citizens.id, TOTAL_CITIZENS);
        assert_eq!(citizens.label, Some(TOTAL_CITIZENS_KEY));
        assert!(citizens.value.is_computed());
        assert_eq!(citizens.value, Value::Occupancy);
        // La liste deroulante n'est pas un panneau et n'est pas un bouton : la classe lui
        // pose un handler (`:264`), elle ne l'enregistre pas.
        assert!(!PANES.iter().any(|pane| pane.id == DROPDOWN));
        assert_ne!(DROPDOWN, DROPDOWN_BUTTON);
        assert_ne!(TAB_ID, STATS_LIST, "happiness n'est pas la liste stats");
        assert_ne!(TAB_ID, "stats");
    }
}
