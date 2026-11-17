//! `WindowResearchTree` : une branche de l'arbre de recherche, dessinée en tuiles.
//!
//! 1 110 lignes de Java, **un** bouton dans le XML (`cancel`) et **zéro** panneau à
//! id : tout le reste est construit à la main dans le `ZoomDragView`
//! (`WindowResearchTree.java:123-127`). Ce que la fenêtre **demande** : la branche,
//! l'université, la colonie et son arbre de recherche (`IGlobalResearchTree` global
//! pour les déclarations, `ILocalResearchTree` pour l'état de la colonie). Ce qu'elle
//! **rend** : dix états de tuile, dont **huit** atteignables, et **trois** cliquables.
//!
//! **Le clic sur une tuile ne fait pas ce qu'on croit.** Une tuile `AVAILABLE` envoie
//! `TryResearchMessage(..., false)` et ferme (`:216-217`). Une tuile `IN_PROGRESS` ou
//! `FINISHED` ne fait qu'**ajouter un bouton** au milieu du parent (`:221-246`,
//! `:752-821`) : c'est le clic sur `undo:<id>` qui annule (`:167-195`). Deux clics,
//! deux envois possibles, et l'état intermédiaire est une vue modifiée.
//!
//! **`LOCKED` et `TOO_MANY_PROGRESS` sont morts.** L'énumération en déclare dix
//! (`:92-101`) ; `getResearchButtonState` (`:376-421`) n'en rend jamais ces deux-là.
//! Les deux libellés `toomanyinprogress` (`:535`, `:541`) ne s'affichent donc jamais.
//! Le port garde les dix pour que le tableau de correspondance soit complet, et un
//! test compte les huit atteignables.
//!
//! **Une tuile cliquable, c'est une tuile qui a un id.** Les trois panneaux d'une
//! tuile — la barre de nom, la boîte d'icône, le bouton d'icône — reçoivent
//! `setID(research.getId().toString())` dans exactement trois états (`:507-508`,
//! `:514-515`, `:521-522`) ; les autres branches n'en posent aucun, donc la tuile
//! grise n'est pas un bouton désactivé, c'est une image.
//!
//! **Ce qui est déjà dans `crate::research` et n'est pas répété ici** : les portes
//! (`gate::{may_begin, may_undo, requirements_met}`), la visibilité d'une recherche
//! masquée (`gate::visible`), et le plafond d'une branche
//! (`progress::Progress::branch_ceiling`). Ce fichier ne porte que ce que la
//! **fenêtre** décide et que la porte ne voit pas : le niveau d'université tel que le
//! clic le teste, le compteur de recherches en cours, et l'état d'affichage.

use crate::research::{Flags, State, Tree, BASE_RESEARCH_TIME, MAX_DEPTH};
use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};

use super::refusal::Refusal;
use super::{Button, Header, Leaves, Opener, Window};

/// L'id de cette table.
pub const ID: &str = "research_tree";
/// La classe (`WindowResearchTree.java:45`).
pub const JAVA: &str = "WindowResearchTree";
/// Le XML (`:113`).
pub const RESOURCE: &str = "minecolonies:gui/windowresearch.xml";
/// Le `ZoomDragView` qui porte les tuiles (`ResearchConstants.java:75`, lu à `:123`).
pub const DRAG_VIEW: &str = "dragView";
/// La croix (`windowresearch.xml:5` ; `WindowConstants.java:477`).
pub const CANCEL: &str = "cancel";
/// La fenêtre que `cancel` rouvre (`:251`) — hors de cette table.
pub const UNIVERSITY_JAVA: &str = "UniversityModuleWindow";

/// L'en-tête d'une profondeur (`:344`).
pub const TIER_KEY: &str = "com.minecolonies.coremod.gui.research.tier.header";
/// Le même, sans heure, pour une branche `UNLOCKABLES` (`:335`).
pub const TIER_UNRESTRICTED_KEY: &str =
    "com.minecolonies.coremod.gui.research.tier.header.unrestricted";
/// Le temps restant (`:737`).
pub const TIME_KEY: &str = "com.minecolonies.coremod.gui.research.time";
/// Le libellé du bouton d'annulation d'une recherche **en cours** (`:762`).
pub const UNDO_PROGRESS_KEY: &str = "com.minecolonies.coremod.research.undo.progress";
/// Son infobulle (`:765`).
pub const UNDO_PROGRESS_TIP_KEY: &str = "com.minecolonies.coremod.research.undo.progress.tooltip";
/// Le libellé du bouton d'annulation d'une recherche **finie** (`:815`).
pub const UNDO_REMOVE_KEY: &str = "com.minecolonies.coremod.research.undo.remove";
/// Son infobulle (`:797`).
pub const UNDO_REMOVE_TIP_KEY: &str = "com.minecolonies.coremod.research.undo.remove.tooltip";
/// Ce que le bouton de remise dit quand il manque des objets (`:804`).
pub const NOT_ENOUGH_KEY: &str = "com.minecolonies.coremod.research.research.notenoughresources";
/// Le premier libellé de `TOO_MANY_PROGRESS` (`:535`) — jamais affiché.
pub const TOO_MANY_1_KEY: &str = "com.minecolonies.coremod.research.research.toomanyinprogress.1";
/// Le second (`:541`) — jamais affiché non plus.
pub const TOO_MANY_2_KEY: &str = "com.minecolonies.coremod.research.research.toomanyinprogress.2";
/// Le « ou » entre deux enfants exclusifs (`:1069`).
pub const OR_KEY: &str = "com.minecolonies.coremod.research.research.or";
/// Son infobulle (`:1072`).
pub const OR_TIP_KEY: &str = "com.minecolonies.coremod.research.research.or.tooltip";
/// Une ligne de coût d'une infobulle (`:652`).
pub const LIMIT_REQUIREMENT_KEY: &str = "com.minecolonies.coremod.research.limit.requirement";
/// « ne peut pas être annulée » (`:680`).
pub const LIMIT_IMMUTABLE_KEY: &str = "com.minecolonies.coremod.research.limit.immutable";
/// « une seule au niveau maximum par branche » (`:670`, `:674`).
pub const ONE_MAX_KEY: &str = "com.minecolonies.coremod.research.limit.onemaxperbranch";
/// « université trop basse » (`:664`).
pub const UNIVERSITY_LEVEL_KEY: &str =
    "com.minecolonies.coremod.research.requirement.university.level";

/// Le préfixe de l'id d'un bouton d'annulation (`:757`, `:814`).
pub const UNDO_PREFIX: &str = "undo:";
/// Le seul bouton du XML, avant que la classe n'ajoute les siens.
pub const XML_BUTTONS: usize = 1;
/// Les panneaux d'une tuile qui reçoivent l'id de la recherche (`:507-508`, `:514-515`,
/// `:521-522`) : la barre de nom, la boîte d'icône, le bouton d'icône.
pub const IDS_PER_TILE: usize = 3;
/// Le message des deux clics (`:192`, `:216`).
pub const SEND_MESSAGE: &str = "TryResearchMessage";
/// Le drapeau du message pour un démarrage (`:216`).
pub const START_FLAG: bool = false;
/// Le drapeau du message pour une annulation (`:192`).
pub const CANCEL_FLAG: bool = true;
/// L'espace de noms que `research.getId().toString()` écrit et que le port ne garde
/// pas (`tree::TABLE` est indexé sans lui).
pub const NAMESPACE: &str = "minecolonies:";

/// L'état d'affichage d'une tuile, dans l'ordre où Java le déclare (`:92-101`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the ten tags the enum declares, dead ones included
pub enum ButtonState {
    Available,
    InProgress,
    Finished,
    Abandoned,
    MissingParent,
    MissingRequirement,
    MissingCost,
    TooManyProgress,
    TooLowUniversity,
    Locked,
}

impl ButtonState {
    /// Les dix, dans l'ordre de déclaration.
    pub const ALL: [ButtonState; 10] = [
        ButtonState::Available,
        ButtonState::InProgress,
        ButtonState::Finished,
        ButtonState::Abandoned,
        ButtonState::MissingParent,
        ButtonState::MissingRequirement,
        ButtonState::MissingCost,
        ButtonState::TooManyProgress,
        ButtonState::TooLowUniversity,
        ButtonState::Locked,
    ];

    /// Un nom stable, pour un journal ou un test.
    pub fn name(self) -> &'static str {
        match self {
            ButtonState::Available => "available",
            ButtonState::InProgress => "in-progress",
            ButtonState::Finished => "finished",
            ButtonState::Abandoned => "abandoned",
            ButtonState::MissingParent => "missing-parent",
            ButtonState::MissingRequirement => "missing-requirement",
            ButtonState::MissingCost => "missing-cost",
            ButtonState::TooManyProgress => "too-many-progress",
            ButtonState::TooLowUniversity => "too-low-university",
            ButtonState::Locked => "locked",
        }
    }

    /// Vrai quand la tuile est un bouton : `AVAILABLE`, `IN_PROGRESS`, `FINISHED`
    /// sont les trois seuls états qui reçoivent un id (`:507-522`) et une
    /// `ButtonImage` (`:976-998`).
    pub fn is_clickable(self) -> bool {
        matches!(self, ButtonState::Available | ButtonState::InProgress | ButtonState::Finished)
    }

    /// Combien de panneaux portent l'id de la recherche : trois, ou aucun.
    pub fn ids(self) -> usize {
        if self.is_clickable() {
            IDS_PER_TILE
        } else {
            0
        }
    }

    /// Vrai quand la barre et la boîte prennent la texture grise (`:524-531`).
    pub fn is_greyed(self) -> bool {
        matches!(
            self,
            ButtonState::Locked
                | ButtonState::Abandoned
                | ButtonState::MissingParent
                | ButtonState::TooLowUniversity
        )
    }

    /// Vrai quand la tuile dessine un cadenas (`:956-967`).
    pub fn has_lock_icon(self) -> bool {
        matches!(
            self,
            ButtonState::Locked
                | ButtonState::Abandoned
                | ButtonState::TooManyProgress
                | ButtonState::MissingParent
                | ButtonState::MissingRequirement
                | ButtonState::TooLowUniversity
        )
    }

    /// Vrai quand la tuile dessine le cadenas ouvert bleu (`:968-974`).
    pub fn has_unlock_icon(self) -> bool {
        self == ButtonState::MissingCost
    }
}

/// Les faits dont `getResearchButtonState` (`:376-421`) a besoin, nommés comme ses
/// arguments et ses lectures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    /// Un ancêtre a un `onlyChild` déjà pris (`:447-451`).
    pub abandoned: bool,
    /// `tree.hasCompletedResearch(research.getParent())` (`:436`).
    pub parent_researched: bool,
    /// L'état de la colonie pour cette recherche (`:438`).
    pub state: State,
    /// `research.getDepth()`.
    pub depth: u8,
    /// Le niveau de l'université.
    pub level: i32,
    /// Son niveau maximum.
    pub max_level: i32,
    /// Une recherche de profondeur 6 est déjà enregistrée
    /// (`branchFinishedHighestLevel`, `:121`).
    pub has_max: bool,
    /// La branche est `UNLOCKABLES` (`:118`).
    pub showcase: bool,
    /// Le joueur est en créatif (`:402`).
    pub creative: bool,
    /// `isResearchRequirementsFulfilled` (`:407`).
    pub requirements_met: bool,
    /// `research.hasEnoughResources(player, pos)` (`:412`).
    pub cost_met: bool,
}

/// L'état d'affichage d'une tuile, dans l'ordre exact des tests de Java
/// (`:378-420`).
pub fn button_state(facts: Facts) -> ButtonState {
    if facts.abandoned {
        return ButtonState::Abandoned;
    }
    if !facts.parent_researched {
        return ButtonState::MissingParent;
    }
    if facts.state == State::Finished {
        return ButtonState::Finished;
    }
    if facts.state == State::InProgress {
        return ButtonState::InProgress;
    }
    // `:397-401`, trois conditions dont une négation à trois termes.
    let too_low = facts.depth as i32 > facts.level
        && !(facts.depth as i32 > facts.max_level
            && !facts.has_max
            && facts.level == facts.max_level)
        && !facts.showcase;
    if too_low {
        return ButtonState::TooLowUniversity;
    }
    if facts.creative {
        return ButtonState::Available;
    }
    if !facts.requirements_met {
        return ButtonState::MissingRequirement;
    }
    if !facts.cost_met {
        return ButtonState::MissingCost;
    }
    ButtonState::Available
}

/// Ce qu'un clic sur un bouton `undo:<id>` ajoute à la vue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two states that can carry an undo button (`:223`, `:227`)
pub enum Undo {
    /// Une recherche en cours : le bouton annule, sans coût (`:752-766`).
    Progress,
    /// Une recherche finie : le bouton coûte des objets (`:774-821`).
    Complete,
}

/// Ce qu'un clic sur une tuile fait, quand il ne refuse pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three ends of the tile branch of `onButtonClicked`
pub enum Click {
    /// La recherche commence : `TryResearchMessage(..., false)` puis fermeture
    /// (`:216-217`).
    Begin,
    /// Aucun message : la tuile gagne un bouton d'annulation et la fenêtre reste
    /// ouverte (`:225-244`).
    Reveal(Undo),
    /// Rien du tout : l'enregistrement est `NOT_STARTED`, posé par une annulation
    /// pour que la recherche ne puisse pas repartir avant la synchro (`:185-191`,
    /// `:219-246`).
    Nothing,
}

impl Click {
    /// Le drapeau que le message porterait, s'il en part un.
    pub fn flag(self) -> Option<bool> {
        match self {
            Click::Begin => Some(START_FLAG),
            Click::Reveal(_) | Click::Nothing => None,
        }
    }

    /// Vrai quand la fenêtre se ferme après le clic.
    pub fn closes(self) -> bool {
        matches!(self, Click::Begin)
    }
}

/// Les faits d'un clic sur une tuile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    /// L'id de la recherche, sans espace de noms — celui de `tree::TABLE`.
    pub id: &'static str,
    pub state: State,
    /// Vrai dès qu'un enregistrement existe, **même** `NOT_STARTED` (`:202`).
    pub recorded: bool,
    pub depth: u8,
    pub level: i32,
    pub max_level: i32,
    /// `getResearchInProgress().size()` (`:203`).
    pub running: usize,
    /// `hasEnoughResources(...) || isCreative` (`:204`).
    pub can_pay: bool,
    pub flags: Flags,
    /// Un enfant direct a déjà un enregistrement (`:236-243`).
    pub child: Option<&'static str>,
}

/// Les trois portes du clic de démarrage (`:198-204`), dans l'ordre de Java.
///
/// **Le parent, l'exclusivité et le plafond ne sont pas ici** : c'est
/// `gate::may_begin` qui les porte, et la tuile ne peut être `AVAILABLE` que si elle
/// les satisfait déjà — donc le clic les re-teste inutilement. Java ne les re-teste
/// pas non plus (`:196-218`), et le serveur les applique une troisième fois.
pub fn may_start(tile: Tile) -> Result<(), Refusal> {
    if tile.level < tile.depth as i32 && tile.level != tile.max_level {
        return Err(Refusal::UniversityTooLow { depth: tile.depth, level: tile.level });
    }
    // `building.getBuildingLevel() > getResearchInProgress().size()`, deux types
    // différents comparés tels quels (`:203`).
    if (tile.level as usize) <= tile.running {
        return Err(Refusal::TooManyInProgress { level: tile.level, running: tile.running });
    }
    if !tile.can_pay {
        return Err(Refusal::CannotPay);
    }
    Ok(())
}

/// Ce que le clic sur une tuile fait : démarrer, révéler une annulation, ou rien.
pub fn click(tile: Tile) -> Result<Click, Refusal> {
    if !tile.recorded {
        may_start(tile)?;
        return Ok(Click::Begin);
    }
    match tile.state {
        State::InProgress => Ok(Click::Reveal(Undo::Progress)),
        State::Finished => {
            // `:228-243` : immuable et autostart n'ont jamais de bouton, et un
            // enfant enregistré doit être annulé avant son parent.
            if tile.flags.immutable() {
                return Err(Refusal::Gate(crate::research::Refusal::Immutable { id: tile.id }));
            }
            if tile.flags.autostarts() {
                return Err(Refusal::Gate(crate::research::Refusal::AutoStarts { id: tile.id }));
            }
            if let Some(child) = tile.child {
                return Err(Refusal::Gate(crate::research::Refusal::ChildInTheWay { child }));
            }
            Ok(Click::Reveal(Undo::Complete))
        }
        // Une annulation a posé `NOT_STARTED` : le clic ne repart pas (`:188-191`).
        State::NotStarted => Ok(Click::Nothing),
    }
}

/// Le temps restant affiché, `"h:MM"` (`:708-734`).
///
/// Java arrondit au quart d'heure supérieur et **promeut** un reste de quatre
/// quarts en une heure de plus (`:727-734`). [`BASE_RESEARCH_TIME`] vaut 72, donc la
/// fenêtre vaut 144 et le quart 36.
pub fn time_left(to_go: u32) -> String {
    let window = BASE_RESEARCH_TIME * 2;
    let quarter = BASE_RESEARCH_TIME / 2;
    let hours = to_go / window;
    let rest = to_go % window;
    let increments = rest.div_ceil(quarter);
    if increments == 4 {
        format!("{}:00", hours + 1)
    } else {
        format!("{}:{:02}", hours, increments * 15)
    }
}

/// Ce qu'il reste à étudier, en actions : la durée de la profondeur moins la
/// progression, et zéro pour une recherche instantanée ou en créatif (`:708-716`).
pub fn to_go(base: u32, progress: u32, instant: bool, creative_completes: bool) -> u32 {
    if instant || creative_completes {
        return 0;
    }
    base.saturating_sub(progress)
}

/// Le numéro d'un en-tête de profondeur : `min(i, niveau maximum)` (`:336`, `:345`).
pub fn tier_number(depth: u8, max_level: i32) -> i32 {
    let max = max_level.max(0);
    if i32::from(depth) > max {
        max
    } else {
        i32::from(depth)
    }
}

/// Un bouton de l'arbre, une fois son id lu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four shapes `onButtonClicked` tests (`:162-249`)
pub enum Named {
    /// Une recherche : l'id est `minecolonies:<branche>/<nom>`, et le port le garde
    /// sans l'espace de noms, comme `tree::TABLE`.
    Research(&'static str),
    /// Un bouton d'annulation posé par la vue : l'id est `undo:` plus celui de la
    /// recherche (`:757`).
    Undo(&'static str),
    /// La croix (`:249`).
    Cancel,
    /// Un id vide : Java ne fait rien, et le dit en commentaire (`:158-165`).
    Empty,
}

/// Le chemin d'une `ResourceLocation` valide, ou le refus nommé.
///
/// Java laisse le soin à `ResourceLocation.isValidResourceLocation` (`:170`, `:196`)
/// ; le port le refait, avec le jeu de caractères de l'identifiant du moteur —
/// espace de noms `[a-z0-9_.-]`, chemin `[a-z0-9/._-]` —, et retire `minecolonies:`
/// pour retrouver les clés de `tree::TABLE`.
pub fn resource_path(id: &str) -> Result<&str, Refusal> {
    let (namespace, path) = match id.split_once(':') {
        Some(parts) => parts,
        None => return Err(Refusal::InvalidId { id: id.to_owned() }),
    };
    let namespaced = !namespace.is_empty()
        && namespace.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c));
    let pathed = !path.is_empty()
        && path.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/._-".contains(c));
    if !namespaced || !pathed {
        return Err(Refusal::InvalidId { id: id.to_owned() });
    }
    if namespace == "minecolonies" {
        Ok(path)
    } else {
        Err(Refusal::NotAResearch { id: id.to_owned() })
    }
}

/// Lit l'id d'un bouton cliqué, dans l'ordre de Java (`:162-249`).
///
/// **Java se tait sur un id invalide** : les quatre branches tombent, et la fonction
/// rend la main sans rien faire. Le port rend [`Refusal::InvalidId`] — un nom plutôt
/// qu'un silence, ce que le §5 du rapport assume.
pub fn parse_button(id: &str, tree: &Tree) -> Result<Named, Refusal> {
    if id.is_empty() {
        return Ok(Named::Empty);
    }
    if id.contains(UNDO_PREFIX) {
        // `substring(indexOf(':') + 1)` : Java prend le texte après le **premier**
        // deux-points, ce qui est celui de `undo:` (`:169`).
        let start = id.find(':').map(|at| at + 1).unwrap_or(0);
        let path = resource_path(&id[start..])?;
        return match tree.get(path) {
            Some(research) => Ok(Named::Undo(research.id)),
            None => Err(Refusal::NotAResearch { id: path.to_owned() }),
        };
    }
    if id == CANCEL {
        return Ok(Named::Cancel);
    }
    let path = resource_path(id)?;
    match tree.get(path) {
        Some(research) => Ok(Named::Research(research.id)),
        None => Err(Refusal::NotAResearch { id: path.to_owned() }),
    }
}

/// Le seul panneau à id : la vue qui porte les tuiles.
const F_VIEW: Field = Field::new(DRAG_VIEW, Value::Row, Source::Colony);
const PANES: &[Field] = &[F_VIEW];

const BUTTONS: &[Button] = &[Button::plain(
    CANCEL,
    "",
    Effect::OpensJava(UNIVERSITY_JAVA),
    Leaves::Reopens,
)];

/// La fenêtre.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::Tiers(TIER_KEY),
    // `UniversityModuleWindow` clique une branche (`:118`).
    opener: Opener::Java(UNIVERSITY_JAVA),
    panes: PANES,
    buttons: BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Tree {
        Tree::load().expect("la table livrée se lit")
    }

    fn facts() -> Facts {
        Facts {
            abandoned: false,
            parent_researched: true,
            state: State::NotStarted,
            depth: 1,
            level: 3,
            max_level: 5,
            has_max: false,
            showcase: false,
            creative: false,
            requirements_met: true,
            cost_met: true,
        }
    }

    fn tile() -> Tile {
        Tile {
            id: "civilian/agile",
            state: State::NotStarted,
            recorded: false,
            depth: 1,
            level: 3,
            max_level: 5,
            running: 0,
            can_pay: true,
            flags: Flags::NONE,
            child: None,
        }
    }

    #[test]
    fn ten_states_are_declared_and_eight_are_reachable() {
        assert_eq!(ButtonState::ALL.len(), 10);
        // Toutes les branches de `getResearchButtonState`, une par une.
        assert_eq!(button_state(Facts { abandoned: true, ..facts() }), ButtonState::Abandoned);
        assert_eq!(
            button_state(Facts { parent_researched: false, ..facts() }),
            ButtonState::MissingParent
        );
        assert_eq!(
            button_state(Facts { state: State::Finished, ..facts() }),
            ButtonState::Finished
        );
        assert_eq!(
            button_state(Facts { state: State::InProgress, ..facts() }),
            ButtonState::InProgress
        );
        assert_eq!(
            button_state(Facts { depth: 4, level: 2, ..facts() }),
            ButtonState::TooLowUniversity
        );
        assert_eq!(
            button_state(Facts { creative: true, requirements_met: false, ..facts() }),
            ButtonState::Available,
            "le créatif passe avant les conditions :403-404"
        );
        assert_eq!(
            button_state(Facts { requirements_met: false, ..facts() }),
            ButtonState::MissingRequirement
        );
        assert_eq!(button_state(Facts { cost_met: false, ..facts() }), ButtonState::MissingCost);
        assert_eq!(button_state(facts()), ButtonState::Available);
        // Les deux états que la fonction ne rend jamais.
        let mut reachable = Vec::new();
        for abandoned in [false, true] {
            for parent in [false, true] {
                for state in [State::NotStarted, State::InProgress, State::Finished] {
                    for depth in 1..=6u8 {
                        for level in 0..=5 {
                            for has_max in [false, true] {
                                for showcase in [false, true] {
                                    for creative in [false, true] {
                                        for requirements_met in [false, true] {
                                            for cost_met in [false, true] {
                                                reachable.push(button_state(Facts {
                                                    abandoned,
                                                    parent_researched: parent,
                                                    state,
                                                    depth,
                                                    level,
                                                    max_level: 5,
                                                    has_max,
                                                    showcase,
                                                    creative,
                                                    requirements_met,
                                                    cost_met,
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        reachable.sort_unstable();
        reachable.dedup();
        assert_eq!(reachable.len(), 8, "{reachable:?}");
        assert!(!reachable.contains(&ButtonState::Locked));
        assert!(!reachable.contains(&ButtonState::TooManyProgress));
        // Et donc les deux libellés de `TOO_MANY_PROGRESS` ne s'affichent jamais.
        assert_ne!(TOO_MANY_1_KEY, TOO_MANY_2_KEY);
    }

    #[test]
    fn only_three_states_make_a_tile_a_button_and_they_own_three_ids() {
        let clickable: Vec<&str> =
            ButtonState::ALL.iter().filter(|s| s.is_clickable()).map(|s| s.name()).collect();
        assert_eq!(clickable, ["available", "in-progress", "finished"]);
        for state in ButtonState::ALL {
            assert_eq!(state.ids(), if state.is_clickable() { 3 } else { 0 }, "{}", state.name());
        }
        assert_eq!(IDS_PER_TILE, 3);
        // Les six états sans id sont aussi les six qui montrent un cadenas, sauf un :
        // `MISSING_COST` dessine le cadenas ouvert (`:968-974`).
        let locked: Vec<&str> =
            ButtonState::ALL.iter().filter(|s| s.has_lock_icon()).map(|s| s.name()).collect();
        assert_eq!(locked.len(), 6);
        assert!(ButtonState::MissingCost.has_unlock_icon());
        assert!(!ButtonState::MissingCost.has_lock_icon());
        // Le gris est plus étroit que le cadenas : quatre états.
        let greyed: Vec<&str> =
            ButtonState::ALL.iter().filter(|s| s.is_greyed()).map(|s| s.name()).collect();
        assert_eq!(greyed, ["abandoned", "missing-parent", "too-low-university", "locked"]);
        // Un seul panneau à id dans le XML, et c'est la vue.
        assert_eq!(WINDOW.panes.len(), 1);
        assert_eq!(WINDOW.panes[0].id, DRAG_VIEW);
        assert_eq!(WINDOW.panes[0].value, Value::Row);
        assert_eq!(XML_BUTTONS, WINDOW.buttons.len());
    }

    #[test]
    fn a_max_level_university_reaches_the_deepest_research() {
        // Niveau 4 pour une profondeur 6 : trop bas.
        assert_eq!(
            button_state(Facts { depth: 6, level: 4, max_level: 5, ..facts() }),
            ButtonState::TooLowUniversity
        );
        // Niveau 5, qui est le maximum, pour une profondeur 6 : la négation de
        // `:397-398` le permet, tant qu'aucune recherche de niveau 6 n'est prise.
        assert_eq!(
            button_state(Facts { depth: 6, level: 5, max_level: 5, ..facts() }),
            ButtonState::Available
        );
        // Le même niveau 5 mais le plafond déjà pris : plus rien.
        assert_eq!(
            button_state(Facts { depth: 6, level: 5, max_level: 5, has_max: true, ..facts() }),
            ButtonState::TooLowUniversity
        );
        // Une branche `UNLOCKABLES` ignore la porte de niveau (`:398`).
        assert_eq!(
            button_state(Facts { depth: 4, level: 1, showcase: true, ..facts() }),
            ButtonState::Available
        );
        // Et l'en-tête d'une profondeur se plafonne au niveau maximum (`:336`).
        assert_eq!(tier_number(1, 5), 1);
        assert_eq!(tier_number(6, 5), 5);
        assert_eq!(tier_number(6, 0), 0);
        assert_eq!(MAX_DEPTH, 6, "ResearchConstants.java:88");
    }

    #[test]
    fn the_time_left_rounds_to_quarter_hours_and_promotes_a_full_four() {
        // Une profondeur 1 : 72 actions, soit une demi-heure.
        assert_eq!(time_left(72), "0:30");
        assert_eq!(time_left(36), "0:15");
        assert_eq!(time_left(144), "1:00");
        assert_eq!(time_left(0), "0:00");
        // Le reste de 143 se coince en quatre quarts : Java monte d'une heure.
        assert_eq!(time_left(143), "1:00");
        assert_eq!(time_left(108), "0:45");
        assert_eq!(time_left(109), "1:00", "`increments == 4` :727-734");
        // Une profondeur 6 : 2 304 actions.
        assert_eq!(time_left(2304), "16:00");
        // Et le temps à faire, qui s'arrête à zéro.
        assert_eq!(to_go(72, 0, false, false), 72);
        assert_eq!(to_go(72, 72, false, false), 0);
        assert_eq!(to_go(72, 200, false, false), 0);
        assert_eq!(to_go(72, 10, true, false), 0, "une recherche instantanée");
        assert_eq!(to_go(72, 10, false, true), 0, "le créatif qui complète");
    }

    #[test]
    fn a_button_id_is_read_the_way_java_slices_it() {
        let tree = tree();
        // Un id de recherche, avec l'espace de noms que Java écrit.
        assert_eq!(
            parse_button("minecolonies:civilian/agile", &tree),
            Ok(Named::Research("civilian/agile"))
        );
        // Un bouton d'annulation : Java prend le texte après le premier `:`.
        assert_eq!(
            parse_button("undo:minecolonies:civilian/agile", &tree),
            Ok(Named::Undo("civilian/agile"))
        );
        assert_eq!(parse_button("cancel", &tree), Ok(Named::Cancel));
        assert_eq!(parse_button("", &tree), Ok(Named::Empty), ":162-165");
        // Une ressource valide qui n'est pas une recherche connue.
        assert_eq!(
            parse_button("minecolonies:civilian/nope", &tree),
            Err(Refusal::NotAResearch { id: "civilian/nope".to_owned() })
        );
        // Un autre espace de noms n'est pas celui de l'arbre.
        assert_eq!(
            parse_button("minecraft:stone", &tree),
            Err(Refusal::NotAResearch { id: "minecraft:stone".to_owned() })
        );
        // Pas de deux-points, ou des majuscules : `ResourceLocation` refuse.
        assert!(matches!(parse_button("civilian/agile", &tree), Err(Refusal::InvalidId { .. })));
        assert!(matches!(
            parse_button("minecolonies:Civilian/Agile", &tree),
            Err(Refusal::InvalidId { .. })
        ));
        assert_eq!(resource_path("minecolonies:combat/agile"), Ok("combat/agile"));
        assert!(resource_path(":agile").is_err());
        assert!(resource_path("minecolonies:").is_err());
    }

    #[test]
    fn starting_a_research_needs_a_level_a_free_slot_and_the_items() {
        assert_eq!(click(tile()), Ok(Click::Begin));
        assert_eq!(Click::Begin.flag(), Some(START_FLAG));
        assert!(!START_FLAG, ":216 envoie `false`");
        assert!(CANCEL_FLAG, ":192 envoie `true`");
        assert!(Click::Begin.closes());
        // Niveau trop bas (`:198-199`).
        assert_eq!(
            click(Tile { depth: 4, level: 2, ..tile() }),
            Err(Refusal::UniversityTooLow { depth: 4, level: 2 })
        );
        // Le maximum atteint rattrape la profondeur.
        assert_eq!(click(Tile { depth: 5, level: 5, max_level: 5, ..tile() }), Ok(Click::Begin));
        // Autant de recherches en cours que de niveaux (`:203`).
        assert_eq!(
            click(Tile { level: 2, running: 2, ..tile() }),
            Err(Refusal::TooManyInProgress { level: 2, running: 2 })
        );
        assert_eq!(click(Tile { level: 2, running: 1, ..tile() }), Ok(Click::Begin));
        // Le prix (`:204`).
        assert_eq!(click(Tile { can_pay: false, ..tile() }), Err(Refusal::CannotPay));
    }

    #[test]
    fn a_recorded_research_reveals_a_second_button_instead_of_sending() {
        // En cours : le clic ajoute le bouton d'annulation, sans rien envoyer.
        let running = Tile { recorded: true, state: State::InProgress, ..tile() };
        assert_eq!(click(running), Ok(Click::Reveal(Undo::Progress)));
        assert_eq!(Click::Reveal(Undo::Progress).flag(), None);
        assert!(!Click::Reveal(Undo::Progress).closes());
        // Finie : le bouton coûte des objets, que le port ne sait pas chiffrer.
        let done = Tile { recorded: true, state: State::Finished, ..tile() };
        assert_eq!(click(done), Ok(Click::Reveal(Undo::Complete)));
        // Immuable et autostart n'ont jamais de bouton (`:228-234`).
        assert!(matches!(
            click(Tile { flags: Flags::IMMUTABLE, ..done }),
            Err(Refusal::Gate(_))
        ));
        assert!(matches!(click(Tile { flags: Flags::AUTOSTART, ..done }), Err(Refusal::Gate(_))));
        assert_eq!(
            click(Tile { child: Some("civilian/agile"), ..done }),
            Err(Refusal::Gate(crate::research::Refusal::ChildInTheWay { child: "civilian/agile" }))
        );
        // Une annulation laisse un enregistrement `NOT_STARTED` : le clic ne repart
        // pas avant la synchro (`:185-191`).
        assert_eq!(
            click(Tile { recorded: true, state: State::NotStarted, ..tile() }),
            Ok(Click::Nothing)
        );
        assert_eq!(Click::Nothing.flag(), None);
        // La croix rouvre l'université et n'envoie rien.
        let cancel = WINDOW.button(CANCEL).expect("cancel");
        assert_eq!(cancel.leaves, Leaves::Reopens);
        assert_eq!(cancel.effect, Some(Effect::OpensJava(UNIVERSITY_JAVA)));
        assert_eq!(cancel.sends(), None);
        // Et la progression de la colonie se lit sur `crate::research`, pas ici.
        let progress = crate::research::Progress::new();
        assert_eq!(progress.state("civilian/agile"), State::NotStarted);
        assert_eq!(progress.running().count(), 0);
        assert!(crate::research::Tree::load().expect("la table").get("civilian/agile").is_some());
    }
}
