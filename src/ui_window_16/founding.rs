//! Les deux fenetres de fondation : le pacte, et le refus de fonder.
//!
//! Java reference : `core/client/gui/townhall/WindowTownHallColonyManage.java` (87
//! lignes) et `WindowTownHallCantCreateColony.java` (52), leurs deux XML
//! (`gui/townhall/windowcolonymanagement.xml`, `:12` puis `windowcantfoundcolony.xml`),
//! les trois messages qui les ouvrent
//! (`OpenColonyFoundingCovenantMessage.java:40`, `OpenCantFoundColonyWarningMessage.java:50`,
//! `GetColonyInfoMessage.java:92-124`) et les deux qui repondent au clic
//! (`CreateColonyMessage.java:105-198`, `PickUpTownhallMessage.java:61-93`).
//!
//! **Ces deux fenetres ne sont pas des pages de l'hotel de ville.** Elles heritent
//! d'`AbstractWindowSkeleton` (`:22`, `:30`) et non d'`AbstractWindowTownHall` : pas de
//! barre d'onglets, pas de titre, pas de bouton de construction, et pas de batiment.
//! Tout ce qu'elles savent est arrive dans le message qui les a ouvertes — c'est
//! [`Source::Packet`], la source que ce module ajoute a `src/ui`.
//!
//! Quatre faits que le source impose :
//!
//! 1. **Le nom propose n'est presque jamais vide, et il n'est pas toujours le nom du
//!    joueur.** En fondation, `preName` vaut `""`
//!    (`OpenColonyFoundingCovenantMessage.java:40`) et le champ prend
//!    `translatable(DEFAULT_COLONY_NAME, joueur)` (`:54`). En **reactivation**, c'est
//!    `WindowTownHallColonyReactivate` qui construit la meme fenetre avec un nom tire au
//!    hasard dans `ColonyStoryListener.abandonedColonyNames`, **graine = la position de
//!    l'hotel de ville** (`WindowTownHallColonyReactivate.java:45-48`, `:60`) — donc le
//!    meme nom a chaque fois pour un meme bloc.
//! 2. **Le champ ne verifie rien.** Ni vide, ni longueur : la seule limite est
//!    `maxlength="32"` du XML (`windowcolonymanagement.xml:12`), et le serveur cree la
//!    colonie avec la chaine telle quelle (`CreateColonyMessage.java:180-181`). Vider le
//!    champ donne une colonie sans nom.
//! 3. **Le pacte date la fondation en « annees apres creeper », et l'unite n'est pas une
//!    annee.** `gameTime / 20 / 60 / 100` (`:55`), c'est-a-dire un jour de jeu (24 000
//!    ticks) **divise par cinq** — 120 000 ticks par unite — avec un plancher de 13.
//! 4. **Le refus de fonder ne vient pas de la fenetre.** Le six refus sont decides par
//!    `GetColonyInfoMessage` avant l'ouverture (dont trois ouvrent l'autre fenetre) et par
//!    `CreateColonyMessage` apres le clic. [`Refusal`] les nomme une fois pour les deux.
//!
//! Ce que la fenetre d'alerte ne fait pas, et qui surprend : son bouton `pickup` declare
//! une texture `disabled` (`windowcantfoundcolony.xml:11`) que **personne ne desactive** —
//! aucun `setEnabled(false)` n'existe dans la classe. Le bouton est toujours cliquable, et
//! c'est le serveur qui refuse (`PickUpTownhallMessage.java:71`, `:76`, `:82-89`).

use crate::ui::field::Value;

use super::pane::{Pane, Source};
use super::refusal::Refusal;

/// La classe de l'alerte (`WindowTownHallCantCreateColony.java:22`).
pub const CANT_JAVA: &str = "WindowTownHallCantCreateColony";

/// Son XML (`:31`).
pub const CANT_RESOURCE: &str = "minecolonies:gui/townhall/windowcantfoundcolony.xml";

/// Le texte de l'alerte (`:36`, `windowcantfoundcolony.xml:7`).
pub const ALERT_TEXT: &str = "text1";

/// Le bouton qui ramasse l'hotel de ville (`:35`, `:47-51`).
pub const PICKUP: &str = "pickup";

/// Son libelle (`windowcantfoundcolony.xml:12`).
pub const PICKUP_LABEL_KEY: &str = "com.minecolonies.core.founding.pickup";

/// Le bouton qui ferme (`:34`).
pub const CANT_CANCEL: &str = "cancel";

/// L'infobulle « Based on Config Setting », posee quand la limite vient de la
/// configuration (`:38-41`, `windowcantfoundcolony.xml:7`).
pub const CONFIG_TOOLTIP_KEY: &str = "com.minecolonies.core.configsetting";

/// Le message du ramassage (`:49`).
pub const PICKUP_MESSAGE: &str = "PickUpTownhallMessage";

/// La classe du pacte (`WindowTownHallColonyManage.java:30`).
pub const MANAGE_JAVA: &str = "WindowTownHallColonyManage";

/// Son XML (`:46`).
pub const MANAGE_RESOURCE: &str = "minecolonies:gui/townhall/windowcolonymanagement.xml";

/// Le libelle du titre, dans un `text` avec id (`windowcolonymanagement.xml:7`).
pub const MANAGE_TITLE_KEY: &str = "com.minecolonies.core.settlementcovenant.title";

/// Le champ du nom (`:54`, `windowcolonymanagement.xml:12`).
pub const NAME_INPUT: &str = "colonyname";

/// La longueur que le XML laisse taper (`windowcolonymanagement.xml:12`) — la classe,
/// elle, n'en verifie aucune.
pub const NAME_FIELD_MAX: usize = 32;

/// Le premier paragraphe du pacte, un argument : l'annee (`:55`).
pub const COVENANT_YEAR_KEY: &str = "com.minecolonies.core.settlementcovenant1";

/// Le paragraphe de la colonie voisine, deux arguments **en `%s`** : son nom et sa
/// distance (`:58`, `manual_en_us.json:2710`).
pub const CLOSE_KEY: &str = "com.minecolonies.core.settlementcovenant3.hasclose";

/// Le paragraphe quand aucune colonie n'est a moins de 1000 blocs (`:62`).
pub const NO_CLOSE_KEY: &str = "com.minecolonies.core.settlementcovenant3.noclose";

/// Le nom par defaut, un argument : le nom du joueur (`:54`).
pub const DEFAULT_NAME_KEY: &str = "com.minecolonies.coremod.gui.townhall.defaultname";

/// Le bouton qui fonde (`:52`, `windowcolonymanagement.xml:23`).
pub const CREATE: &str = "create";

/// Son libelle (`windowcolonymanagement.xml:26`).
pub const CREATE_LABEL_KEY: &str = "com.minecolonies.coremod.gui.colony.create";

/// Le bouton qui ferme (`:51`).
pub const MANAGE_CANCEL: &str = "cancel";

/// Le message du clic de fondation (`:78-83`).
pub const CREATE_MESSAGE: &str = "CreateColonyMessage";

/// L'annee plancher du pacte (`:55`).
pub const MIN_YEAR: i64 = 13;

/// Les ticks d'une unite du pacte : `TICKS_SECOND * 60 * 100` (`Constants.java:21`).
pub const TICKS_PER_YEAR_UNIT: i64 = 120_000;

/// Sous cette distance, le pacte nomme la colonie voisine (`:56`) : c'est la constante
/// du code, pas la portee d'une colonie.
pub const NEAR_DISTANCE: i32 = 1000;

/// L'alerte que la fenetre affiche, telle que le message qui l'ouvre la fabrique
/// (`GetColonyInfoMessage.java:98`, `:102`, `:123`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois textes, trois arguments
pub enum Warning {
    /// Trop pres du spawn : l'argument est le nombre de blocs qui manquent (`:98`).
    TooCloseToSpawn(i32),
    /// Trop loin du spawn : l'argument est le nombre de blocs a revenir (`:102`).
    TooFarFromSpawn(i32),
    /// Trop pres d'une colonie : l'argument est la distance a respecter (`:123`).
    TooCloseToColony(i32),
}

impl Warning {
    /// Le refus que cette alerte raconte.
    pub fn refusal(self) -> Refusal {
        match self {
            Warning::TooCloseToSpawn(_) => Refusal::TooCloseToSpawn,
            Warning::TooFarFromSpawn(_) => Refusal::TooFarFromSpawn,
            Warning::TooCloseToColony(_) => Refusal::TooCloseToColony,
        }
    }

    /// La cle du texte, celle que `text1` recoit (`:36-37`) — `None` n'arrive pas, les
    /// trois refus sont des `Said`, et un test le tient.
    pub fn key(self) -> Option<&'static str> {
        self.refusal().key()
    }

    /// L'argument du texte, toujours un entier.
    pub fn arg(self) -> i32 {
        match self {
            Warning::TooCloseToSpawn(missing)
            | Warning::TooFarFromSpawn(missing)
            | Warning::TooCloseToColony(missing) => missing,
        }
    }

    /// Vrai quand l'alerte porte l'infobulle « Based on Config Setting » : les deux
    /// limites de spawn viennent de la configuration, celle des colonies non
    /// (`:98`, `:102` valent `true` ; `:123` vaut `false`).
    pub fn shows_config_tooltip(self) -> bool {
        !matches!(self, Warning::TooCloseToColony(_))
    }
}

/// L'alerte de distance au spawn, ou aucune (`GetColonyInfoMessage.java:95-103`).
///
/// Les deux bornes sont incluses : a exactement `min` ou `max`, la fondation passe.
pub fn spawn_warning(distance: f64, min: f64, max: f64) -> Option<Warning> {
    if distance < min {
        Some(Warning::TooCloseToSpawn((min - distance) as i32))
    } else if distance > max {
        Some(Warning::TooFarFromSpawn((distance - max) as i32))
    } else {
        None
    }
}

/// La distance a respecter quand une colonie est trop proche
/// (`GetColonyInfoMessage.java:120-123`).
///
/// `blockRange` est `max(minColonyDistance, initialColonySize) << 4` en blocs, et le
/// resultat est **plancher a 100** : quel que soit le reglage, le jeu ne demande jamais
/// de reculer de moins de cent blocs.
pub fn colony_shortfall(distance: i32, min_colony_distance: i32, initial_size: i32) -> i32 {
    let block_range = min_colony_distance.max(initial_size) << 4;
    (block_range - distance).max(100)
}

/// La distance affichee pour la colonie la plus proche (`OpenColonyFoundingCovenantMessage`
/// est construit avec `getDistance(centre, pos) - (initialColonySize << 4)`,
/// `GetColonyInfoMessage.java:110`).
///
/// Sans colonie, Java passe `Integer.MAX_VALUE` : le pacte affiche alors la phrase
/// « aucune colonie proche » (`:56`). Le port garde le meme nombre plutot qu'un `Option`,
/// parce que c'est la valeur que la fenetre teste.
pub const NO_COLONY: i32 = i32::MAX;

/// La distance affichee, ou [`NO_COLONY`].
pub fn closest_distance(raw: i32, initial_size: i32) -> i32 {
    if raw == i32::MAX {
        NO_COLONY
    } else {
        raw - (initial_size << 4)
    }
}

/// Ce que le troisieme paragraphe du pacte dit (`:56-63`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux branches, deux textes
pub enum Closest {
    /// Une colonie est a moins de [`NEAR_DISTANCE`] : elle est nommee, en rouge.
    Near { distance: i32 },
    /// Aucune, ou trop loin pour etre nommee.
    Far,
}

impl Closest {
    /// La cle du paragraphe.
    pub fn key(self) -> &'static str {
        match self {
            Closest::Near { .. } => CLOSE_KEY,
            Closest::Far => NO_CLOSE_KEY,
        }
    }

    /// La distance quand il y en a une.
    pub fn distance(self) -> Option<i32> {
        match self {
            Closest::Near { distance } => Some(distance),
            Closest::Far => None,
        }
    }
}

/// Le paragraphe de la colonie voisine (`:56-63`).
///
/// **La distance part en `%s`, pas en `%d`** : Java l'enveloppe dans un
/// `Component.literal(String.valueOf(closestDistance))` pour la colorer en rouge
/// (`:58`), et la cle de traduction declare deux `%s` (`manual_en_us.json:2710`).
pub fn closest_line(distance: i32) -> Closest {
    if distance < NEAR_DISTANCE {
        Closest::Near { distance }
    } else {
        Closest::Far
    }
}

/// Le nom que le champ porte a l'ouverture (`:54`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: le nom du joueur, ou celui qu'on a donne
pub enum InitialName {
    /// `preName` vide : `translatable(DEFAULT_COLONY_NAME, joueur)`.
    Player { player: String },
    /// Un nom deja choisi — la reactivation en passe un (`:54`, `WindowTownHallColonyReactivate.java:60`).
    Given(String),
}

impl InitialName {
    /// Le texte du champ, ou la cle et son argument quand c'est le nom par defaut.
    pub fn text(&self) -> String {
        match self {
            InitialName::Player { player } => player.clone(),
            InitialName::Given(name) => name.clone(),
        }
    }

    /// La cle de traduction, `None` pour un nom deja ecrit.
    pub fn key(&self) -> Option<&'static str> {
        match self {
            InitialName::Player { .. } => Some(DEFAULT_NAME_KEY),
            InitialName::Given(_) => None,
        }
    }
}

/// Le nom du champ a l'ouverture (`WindowTownHallColonyManage.java:54`).
pub fn initial_name(pre_name: &str, player: &str) -> InitialName {
    if pre_name.is_empty() {
        InitialName::Player { player: player.to_owned() }
    } else {
        InitialName::Given(pre_name.to_owned())
    }
}

/// L'annee du pacte (`:55`) : `Math.max(13, gameTime / 20 / 60 / 100)`.
///
/// Les quatre divisions sont entieres et successives, sur un `long` : pour un temps
/// positif, c'est une division par 120 000. Un jour de jeu fait 24 000 ticks, donc
/// **l'unite du pacte vaut un cinquieme de jour** — le pacte date en jours de jeu
/// divises par cinq, pas en annees reelles.
pub fn covenant_year(ticks: i64) -> i64 {
    (ticks / TICKS_PER_YEAR_UNIT).max(MIN_YEAR)
}

/// L'etat de la fenetre du pacte : le nom en cours de frappe et la reactivation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manage {
    /// Le texte du champ (`colonyname`).
    pub name: String,
    /// Vrai quand on reactive une colonie abandonnee (`:42`, `:48`).
    pub reactivate: bool,
}

impl Manage {
    /// La fenetre telle qu'elle s'ouvre.
    pub fn new(pre_name: &str, player: &str, reactivate: bool) -> Manage {
        Manage { name: initial_name(pre_name, player).text(), reactivate }
    }

    /// Le champ tel que le joueur le laisse (`getText()`, `:71`).
    ///
    /// Aucun controle : Java n'en fait aucun, ni sur le vide ni sur la longueur.
    pub fn set_name(&mut self, text: &str) {
        self.name = text.to_owned();
    }

    /// Ce que le clic envoie (`:78-83`).
    ///
    /// Le quatrieme argument est `reactivate ? "" : StructurePacks.selectedPack.getName()`
    /// : une reactivation ne propose **aucun** pack, et c'est le serveur qui garde celui
    /// du bloc (`CreateColonyMessage.java:129-132`). Le cinquieme est le chemin de
    /// blueprint lu sur le bloc.
    pub fn payload(&self, selected_pack: Option<&str>, blueprint: &str) -> Payload {
        let pack = if self.reactivate { None } else { selected_pack.map(str::to_owned) };
        Payload {
            name: self.name.clone(),
            reactivate: self.reactivate,
            pack,
            blueprint: blueprint.to_owned(),
        }
    }
}

/// Ce que `CreateColonyMessage` emporte (`:79-83`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    /// Le nom, tel quel.
    pub name: String,
    /// Vrai pour une reactivation (`claim` dans le message).
    pub reactivate: bool,
    /// Le pack propose, `None` = la chaine vide que Java envoie.
    pub pack: Option<String>,
    /// Le chemin de blueprint du bloc (`TileEntityColonyBuilding.getBlueprintPath`).
    pub blueprint: String,
}

/// Les quatre panneaux de l'alerte (`windowcantfoundcolony.xml:7`).
pub static ALERT_PANES: [Pane; 1] = [Pane::new(
    ALERT_TEXT,
    Value::Text,
    Source::Packet,
    ("windowcantfoundcolony.xml", 7),
)];

/// Les trois panneaux du pacte : le champ du nom, l'annee, la colonie voisine.
pub static MANAGE_PANES: [Pane; 3] = [
    Pane::new(NAME_INPUT, Value::Input, Source::Player, ("windowcolonymanagement.xml", 12)),
    Pane::new(COVENANT_YEAR, Value::Text, Source::Clock, ("windowcolonymanagement.xml", 10)),
    Pane::new(CLOSEST_TEXT, Value::Text, Source::Packet, ("windowcolonymanagement.xml", 16)),
];

/// Le panneau du premier paragraphe (`:55`).
pub const COVENANT_YEAR: &str = "text1";

/// Le panneau du paragraphe de la colonie voisine (`:56`, `:62`).
pub const CLOSEST_TEXT: &str = "text3";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Scope;

    #[test]
    fn the_covenant_year_floors_at_thirteen_and_divides_by_a_fifth_of_a_day() {
        assert_eq!(covenant_year(0), 13);
        assert_eq!(covenant_year(TICKS_PER_YEAR_UNIT - 1), 13);
        assert_eq!(covenant_year(TICKS_PER_YEAR_UNIT), 13, "un, mais le plancher gagne");
        assert_eq!(covenant_year(14 * TICKS_PER_YEAR_UNIT), 14);
        // Un jour de jeu = 24 000 ticks, donc cinq jours de jeu pour une unite : quatre
        // jours de jeu ne suffisent pas a quitter le plancher.
        assert_eq!(covenant_year(5 * 24_000), 13);
        assert_eq!(covenant_year(20 * 24_000), 13, "quatre unites, plancher");
        assert_eq!(covenant_year(100 * 24_000), 20, "vingt unites = cent jours de jeu");
        assert_eq!(TICKS_PER_YEAR_UNIT, 20 * 60 * 100, "Constants.java:21");
        assert!(MIN_YEAR > 0);
    }

    #[test]
    fn the_closest_colony_is_named_under_a_thousand_blocks() {
        assert_eq!(closest_line(999), Closest::Near { distance: 999 });
        assert_eq!(closest_line(1000), Closest::Far, ":56 teste strictement");
        assert_eq!(closest_line(0), Closest::Near { distance: 0 });
        assert_eq!(Closest::Near { distance: 999 }.key(), CLOSE_KEY);
        assert_eq!(Closest::Far.key(), NO_CLOSE_KEY);
        assert_ne!(CLOSE_KEY, NO_CLOSE_KEY);
        assert_eq!(Closest::Near { distance: 12 }.distance(), Some(12));
        assert_eq!(Closest::Far.distance(), None);
        // Sans colonie, Java passe Integer.MAX_VALUE : le pacte ne la nomme pas.
        assert_eq!(closest_distance(NO_COLONY, 4), NO_COLONY);
        assert_eq!(closest_line(closest_distance(NO_COLONY, 4)), Closest::Far);
        // Sinon la distance est celle du centre, moins la taille initiale en blocs.
        assert_eq!(closest_distance(500, 4), 500 - 64);
        assert_eq!(closest_distance(64, 4), 0);
    }

    #[test]
    fn the_two_spawn_limits_and_the_hundred_block_floor() {
        // Bornes incluses : a min et a max, la fondation passe.
        assert_eq!(spawn_warning(0.0, 0.0, 30_000.0), None);
        assert_eq!(spawn_warning(30_000.0, 0.0, 30_000.0), None);
        assert_eq!(spawn_warning(100.0, 200.0, 30_000.0), Some(Warning::TooCloseToSpawn(100)));
        assert_eq!(spawn_warning(30_500.0, 0.0, 30_000.0), Some(Warning::TooFarFromSpawn(500)));
        assert_eq!(Warning::TooCloseToSpawn(100).arg(), 100);
        assert_eq!(Warning::TooFarFromSpawn(500).arg(), 500);
        // Configuration par defaut : 8 troncons de colonie, 4 de taille initiale.
        assert_eq!(colony_shortfall(10, 8, 4), 118);
        assert_eq!(colony_shortfall(127, 8, 4), 100, "plancher a 100 blocs");
        assert_eq!(colony_shortfall(1000, 8, 4), 100, "une distance negative reste a 100");
        // Une taille initiale plus grande que la distance minimale gagne.
        assert_eq!(colony_shortfall(0, 1, 15), 15 << 4);
    }

    #[test]
    fn only_the_two_spawn_warnings_show_the_config_tooltip() {
        assert!(Warning::TooCloseToSpawn(1).shows_config_tooltip());
        assert!(Warning::TooFarFromSpawn(1).shows_config_tooltip());
        assert!(
            !Warning::TooCloseToColony(100).shows_config_tooltip(),
            "GetColonyInfoMessage.java:123 passe false"
        );
        assert_eq!(CONFIG_TOOLTIP_KEY, "com.minecolonies.core.configsetting");
        // Chaque alerte nomme le refus qui la produit, et sa cle.
        assert_eq!(Warning::TooCloseToSpawn(1).refusal(), Refusal::TooCloseToSpawn);
        assert_eq!(
            Warning::TooCloseToSpawn(1).key(),
            Some("com.minecolonies.core.founding.tooclosetospawn")
        );
        assert_eq!(
            Warning::TooCloseToColony(1).key(),
            Some("com.minecolonies.core.founding.tooclosetocolony")
        );
        for warning in [
            Warning::TooCloseToSpawn(1),
            Warning::TooFarFromSpawn(1),
            Warning::TooCloseToColony(1),
        ] {
            assert!(warning.refusal().is_said(), "{warning:?} doit avoir une cle");
        }
    }

    #[test]
    fn the_field_starts_on_the_player_name_or_on_the_name_it_was_given() {
        let default = initial_name("", "Simon");
        assert_eq!(default.key(), Some(DEFAULT_NAME_KEY));
        assert_eq!(default.text(), "Simon");
        let given = initial_name("Nouvelle-Aquitaine", "Simon");
        assert_eq!(given.key(), None);
        assert_eq!(given.text(), "Nouvelle-Aquitaine");
        // La reactivation passe un nom, la fondation non (les deux seuls appelants).
        let manage = Manage::new("", "Simon", false);
        assert_eq!(manage.name, "Simon");
        assert!(!manage.reactivate);
        let revived = Manage::new("Vieux-Port", "Simon", true);
        assert_eq!(revived.name, "Vieux-Port");
        assert!(revived.reactivate);
        // Rien ne verifie le champ : vide accepte, 40 caracteres acceptes par la classe.
        let mut manage = Manage::new("", "Simon", false);
        manage.set_name("");
        assert_eq!(manage.name, "");
        let long = "x".repeat(40);
        manage.set_name(&long);
        assert_eq!(manage.name.chars().count(), 40);
        assert_eq!(NAME_FIELD_MAX, 32, "la seule limite est celle du XML");
        assert!(manage.name.chars().count() > NAME_FIELD_MAX);
    }

    #[test]
    fn a_reactivation_proposes_no_structure_pack() {
        let founding = Manage::new("", "Simon", false);
        let payload = founding.payload(Some("minecolonies:medieval_oak"), "townhall1");
        assert_eq!(payload.name, "Simon");
        assert!(!payload.reactivate);
        assert_eq!(payload.pack.as_deref(), Some("minecolonies:medieval_oak"));
        assert_eq!(payload.blueprint, "townhall1");
        // Reactivation : la chaine vide de WindowTownHallColonyManage.java:82.
        let revived = Manage::new("Vieux-Port", "Simon", true);
        let payload = revived.payload(Some("minecolonies:medieval_oak"), "townhall1");
        assert!(payload.reactivate);
        assert_eq!(payload.pack, None);
        assert_eq!(payload.blueprint, "townhall1", "le chemin est toujours envoye");
        // Aucun pack selectionne n'est pas None : c'est le pack qui manque.
        let bare = Manage::new("", "Simon", false).payload(None, "");
        assert_eq!(bare.pack, None);
        assert_eq!(bare.blueprint, "");
    }

    #[test]
    fn four_panes_one_of_them_a_clock_and_none_of_them_nested() {
        assert_eq!(ALERT_PANES.len() + MANAGE_PANES.len(), 4);
        for pane in ALERT_PANES.iter().chain(MANAGE_PANES.iter()).copied() {
            assert_eq!(pane.scope, Scope::Window);
            assert!(!pane.is_nested());
            assert!(pane.declared.1 > 0);
            assert!(
                pane.declared.0.starts_with("window"),
                "{} vient de {}",
                pane.id,
                pane.declared.0
            );
        }
        // Le champ du nom et le paragraphe de l'annee sont sur la meme fenetre, a deux
        // sources differentes : l'un vient du joueur, l'autre de l'horloge du monde.
        let name = MANAGE_PANES[0];
        assert_eq!(name.id, NAME_INPUT);
        assert_eq!(name.value, Value::Input);
        assert_eq!(name.source, Source::Player);
        assert_eq!(MANAGE_PANES[1].source, Source::Clock);
        assert_eq!(ALERT_PANES[0].source, Source::Packet);
        assert_eq!(MANAGE_PANES[2].id, CLOSEST_TEXT);
        // Ici aucun panneau n'est calcule : un `n / max` n'existe pas dans ces fenetres.
        for pane in ALERT_PANES.iter().chain(MANAGE_PANES.iter()) {
            assert!(!pane.value.is_computed(), "{} se calculerait", pane.id);
        }
        // Et le libelle du titre est dans le XML, sur un panneau que la classe ne touche
        // jamais : ce n'est donc pas un panneau de cette table.
        assert!(!MANAGE_PANES.iter().any(|pane| pane.id == "title"));
        assert_eq!(MANAGE_TITLE_KEY, "com.minecolonies.core.settlementcovenant.title");
    }
}
