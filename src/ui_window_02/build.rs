//! Les deux fenêtres de construction : `WindowBuildBuilding` et `WindowBuildDecoration`.
//!
//! Les deux lisent **le même XML** (`WindowBuildDecoration.java:116`) et n'en remplissent
//! pas les mêmes panneaux : la décoration cache la navigation de style (`:124-130`) et ne
//! montre `build` qu'une fois les ressources lues (`:253`). D'où une [`State`] et un
//! [`Flavour`] : deux fenêtres, une règle.
//!
//! Ce que la fenêtre du bâtiment demande : son niveau, son maximum, le niveau de son
//! parent (`canBeUpgraded`, `WindowBuildBuilding.java:167-171`), son pack et son chemin
//! (`:263-320`), ses ressources (`:298-355`), les builders de la colonie (`:246-258`). Ce
//! qu'elle rend : l'état de cinq boutons, la route de confirmation, deux messages.

use crate::building::Kind;
use crate::ui::Value;

use super::pane::{Action, Dropdown, Exit, Label, Message, Pane, Press, Source};
use super::refusal::Refusal;
use super::Window;

/// L'id de la fenêtre du bâtiment dans cette table.
pub const ID: &str = "build";
/// L'id de la fenêtre de décoration.
pub const DECORATION_ID: &str = "decoration";
/// La classe de la première.
pub const JAVA: &str = "WindowBuildBuilding";
/// La classe de la seconde.
pub const DECORATION_JAVA: &str = "WindowBuildDecoration";
/// La ressource des **deux** : `WindowBuildDecoration.java:116` recharge celle du bâtiment.
pub const RESOURCE: &str = "minecolonies:gui/windowbuildbuilding.xml";

// Les panneaux et les boutons, dans l'ordre de `windowbuildbuilding.xml:5-58`.
/// La liste des ressources (`:40`).
pub const RESOURCES: &str = "resources";
/// L'icône d'une ligne (`:42`).
pub const RESOURCE_ICON: &str = "resourceIcon";
/// Le nom d'une ligne (`:43`).
pub const RESOURCE_NAME: &str = "resourceName";
/// « combien il en faut », une ligne (`:44` ; `WindowConstants.java:507`).
pub const RESOURCE_QUANTITY: &str = "resourceQuantity";
/// Le style (`:7` ; `WindowConstants.java:427`).
pub const STYLE_DROPDOWN: &str = "style";
/// Les builders (`:23` ; `WindowConstants.java:432`).
pub const BUILDER_DROPDOWN: &str = "worker";
/// `:5` ; `WindowConstants.java:422`.
pub const PREVIOUS_STYLE: &str = "previousStyle";
/// `:20` ; `WindowConstants.java:437`.
pub const NEXT_STYLE: &str = "nextStyle";
/// `:37` ; `WindowConstants.java:477`.
pub const CANCEL: &str = "cancel";
/// `:47` ; `WindowConstants.java:556`.
pub const REPAIR: &str = "repair";
/// `:50` ; `WindowConstants.java:555`.
pub const BUILD: &str = "build";
/// `:53` ; `WindowConstants.java:589`.
pub const DECONSTRUCT: &str = "deconstruct";
/// `:56` ; `WindowConstants.java:594`.
pub const PICKUP: &str = "pickup";

/// `ACTION_BUILD` (`TranslationConstants.java:472`).
pub const ACTION_BUILD_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.build";
/// `ACTION_UPGRADE` (`TranslationConstants.java:474`) — aussi le libellé du XML (`:52`).
pub const ACTION_UPGRADE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.upgrade";
/// `windowbuildbuilding.xml:49`.
pub const REPAIR_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.repair";
/// `:55`.
pub const DECONSTRUCT_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.deconstruct";
/// `:58`.
pub const PICKUP_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.pickup";
/// `WindowBuildBuilding.java:209`.
pub const CONFIRM_TITLE_KEY: &str = "com.minecolonies.core.gui.build.confirm.title";
/// `OUT_OF_COLONY` (`TranslationConstants.java:354`) : le seul refus qui parle au joueur.
pub const OUT_OF_COLONY_KEY: &str = "com.minecolonies.coremod.workorder.outofcolony";
/// `BuildRequestMessage` (`:188`, `:224`, `:228`).
pub const BUILD_REQUEST_MESSAGE: &str = "BuildRequestMessage";
/// `BuildPickUpMessage` (`:178`).
pub const BUILD_PICKUP_MESSAGE: &str = "BuildPickUpMessage";
/// `BuildingSetStyleMessage` (`:221`).
pub const BUILDING_SET_STYLE_MESSAGE: &str = "BuildingSetStyleMessage";
/// Le message de la décoration n'est **pas une classe** : c'est le paramètre
/// `Function<BlockPos, IMessage>` de son constructeur (`WindowBuildDecoration.java:83`).
pub const WORK_ORDER_PARAMETER: &str = "buildRequestMessage";
/// La fenêtre redemande les ressources tous les 20 ticks tant que la liste est vide
/// (`WindowBuildBuilding.java:300`, `:481-484`).
pub const RESOURCE_RETRY_TICKS: i32 = 20;

const F_RESOURCES: Pane = Pane::new(RESOURCES, Value::Row, Source::Blueprint);
const F_ICON: Pane = Pane::in_row(RESOURCES, RESOURCE_ICON, Value::Icon, Source::Blueprint);
const F_NAME: Pane = Pane::in_row(RESOURCES, RESOURCE_NAME, Value::Name, Source::Blueprint);
const F_QUANTITY: Pane = Pane::in_row(RESOURCES, RESOURCE_QUANTITY, Value::Count, Source::Blueprint);
/// Les quatre panneaux que les deux fenêtres remplissent (`:350-351`, `:516-524`).
pub const PANES: &[Pane] = &[F_RESOURCES, F_ICON, F_NAME, F_QUANTITY];
/// Les deux listes déroulantes du bâtiment ; la décoration cache la première.
pub const DROPDOWNS: &[&str] = &[STYLE_DROPDOWN, BUILDER_DROPDOWN];

const NOTHING_SENT: &[Message] = &[];
const NO_REFUSAL: &[Refusal] = &[];
const SENDS_STYLE_THEN_REQUEST: &[Message] = &[
    Message::plain(BUILDING_SET_STYLE_MESSAGE),
    Message::plain(BUILD_REQUEST_MESSAGE),
];
const SENDS_REPAIR: &[Message] = &[Message::with(BUILD_REQUEST_MESSAGE, "REPAIR")];
const SENDS_REMOVE: &[Message] = &[Message::with(BUILD_REQUEST_MESSAGE, "REMOVE")];
const SENDS_PICKUP: &[Message] = &[Message::plain(BUILD_PICKUP_MESSAGE)];
const SENDS_WORK_ORDER: &[Message] = &[Message::plain(WORK_ORDER_PARAMETER)];

/// `cancel` rouvre le bâtiment, `building.openGui(false)` (`:195-198`).
const A_CANCEL: Action =
    Action::new(CANCEL, Label::Xml(""), Press::new(NOTHING_SENT, Exit::Reopens), NO_REFUSAL);
/// Le style : le seul cas atteignable est une liste d'un élément, donc ces deux clics ne
/// changent rien (`:263-288`, `:453-464`).
const A_PREVIOUS_STYLE: Action = Action::new(PREVIOUS_STYLE, Label::Xml("<"), Press::STAYS, NO_REFUSAL);
/// Voir [`A_PREVIOUS_STYLE`].
const A_NEXT_STYLE: Action = Action::new(NEXT_STYLE, Label::Xml(">"), Press::STAYS, NO_REFUSAL);
/// `build` : le style **puis** la demande, et la fenêtre se referme (`:219-231`). Libellé
/// écrit par la classe — `ACTION_BUILD` au niveau 0 (`:142`), `ACTION_UPGRADE` sinon
/// (`:153`), rien quand la fenêtre le cache (`:149`).
const A_BUILD: Action = Action::new(
    BUILD,
    Label::Written(ACTION_UPGRADE_KEY),
    Press::new(SENDS_STYLE_THEN_REQUEST, Exit::Reopens),
    &[Refusal::BlueprintMissing, Refusal::CannotUpgrade],
);
/// `repair` (`:236-241`) : caché au niveau 0 (`:143`) et sans blueprint (`:326`).
const A_REPAIR: Action = Action::new(
    REPAIR,
    Label::Xml(REPAIR_KEY),
    Press::new(SENDS_REPAIR, Exit::Reopens),
    &[Refusal::BlueprintMissing],
);
/// `deconstruct` (`:185-190`) : caché au niveau 0 (`:144`) et sans blueprint (`:326`).
const A_DECONSTRUCT: Action = Action::new(
    DECONSTRUCT,
    Label::Xml(DECONSTRUCT_KEY),
    Press::new(SENDS_REMOVE, Exit::Reopens),
    &[Refusal::BlueprintMissing],
);
/// `pickup` (`:176-180`) : montré au niveau 0 (`:145`), sur un bâtiment déconstruit
/// (`:159`) et sans blueprint (`:327`).
const A_PICKUP: Action = Action::new(
    PICKUP,
    Label::Xml(PICKUP_KEY),
    Press::new(SENDS_PICKUP, Exit::Closes),
    NO_REFUSAL,
);
/// Le `build` de la décoration : libellé écrit (`:123`), montré après la lecture (`:253`).
const A_DECORATION_BUILD: Action = Action::new(
    BUILD,
    Label::Written(ACTION_BUILD_KEY),
    Press::new(SENDS_WORK_ORDER, Exit::Closes),
    &[Refusal::BlueprintMissing],
);

/// Les sept boutons du bâtiment, dans l'ordre de l'XML (`:5-58`).
pub const ACTIONS: &[Action] =
    &[A_PREVIOUS_STYLE, A_NEXT_STYLE, A_CANCEL, A_REPAIR, A_BUILD, A_DECONSTRUCT, A_PICKUP];
/// Les deux boutons de la décoration, qui cache les cinq autres (`:124-130`).
pub const DECORATION_ACTIONS: &[Action] = &[A_DECORATION_BUILD, A_CANCEL];

/// La fenêtre du bâtiment, ouverte par `src/ui` (`ui/action.rs:436-441`).
pub static BUILD_WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    layout: None,
    panes: PANES,
    dropdowns: DROPDOWNS,
    actions: ACTIONS,
};

/// La fenêtre de décoration : même XML, cinq boutons de moins.
pub static DECORATION_WINDOW: Window = Window {
    id: DECORATION_ID,
    java: DECORATION_JAVA,
    resource: RESOURCE,
    layout: None,
    panes: PANES,
    dropdowns: &[BUILDER_DROPDOWN],
    actions: DECORATION_ACTIONS,
};

/// Laquelle des deux fenêtres : la seule différence de règle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
    /// `WindowBuildBuilding`, ouverte par une hutte.
    Building,
    /// `WindowBuildDecoration`, ouverte par un outil de build, une plantation ou un
    /// contrôleur (`OpenBuildWindowMessage.java:93`, `WindowDecorationController.java:144`).
    Decoration,
}

/// Où en est la lecture du blueprint : Java distingue « pas encore lu » de « absent »
/// (`WindowBuildBuilding.java:321-329`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blueprint {
    /// Lecture en cours : l'état du constructeur tient (`:300`, `:469-475`).
    Pending,
    /// Lue, et le fichier n'existe pas (`:323`).
    Missing,
    /// Lue et trouvée (`:331-353`).
    Found,
}

/// Le parent, tel que `canBeUpgraded` le lit (`:169-170`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parent {
    pub level: i32,
    pub max_level: i32,
}

/// Ce que la fenêtre sait du bâtiment, à un instant donné.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub level: i32,
    pub max_level: i32,
    /// `getParent()` résolu en vue (`:169`).
    pub parent: Option<Parent>,
    /// `isDeconstructed()` (`:156`).
    pub deconstructed: bool,
    /// `getHoverWarningForLevel()`, vide quand il n'y a pas d'avertissement.
    pub warning: Option<&'static str>,
    pub blueprint: Blueprint,
}

/// Ce qu'une fenêtre montre de ses boutons : `None` = caché, `Some(clé)` = visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub build: Option<&'static str>,
    pub repair: Option<&'static str>,
    pub deconstruct: bool,
    pub pickup: bool,
    /// `false` pour la décoration (`:127-130`).
    pub style_nav: bool,
}

/// Où mène le clic `build` : tout de suite, ou après confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// `triggerConfirmAction` part directement (`:212`).
    Direct,
    /// `WindowConfirm` d'abord (`:207-211`).
    ConfirmDialog { title: &'static str, warning: &'static str },
}

/// Le mode que `BuildRequestMessage` porte (`BuildRequestMessage.java:23-28`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Build,
    Repair,
    Remove,
}

impl State {
    /// `canBeUpgraded()` (`:167-171`) : sous son maximum **et** (pas de parent, ou sous le
    /// niveau du parent, ou un parent déjà à son maximum — la branche qui surprend).
    pub fn upgradable(&self) -> bool {
        if self.level >= self.max_level {
            return false;
        }
        match self.parent {
            None => true,
            Some(parent) => self.level < parent.level || parent.level >= parent.max_level,
        }
    }

    /// Le niveau dont la fenêtre demande les ressources (`:308-312`).
    pub fn next_level(&self) -> i32 {
        if self.upgradable() { self.level + 1 } else { self.level }
    }

    /// L'état des cinq boutons : le constructeur (`:138-160`) puis la lecture des
    /// ressources (`:323-329`), qui gagne.
    pub fn buttons(&self, flavour: Flavour) -> Buttons {
        if flavour == Flavour::Decoration {
            // Les cinq autres sont cachés (`:123-130`) ; `build` arrive à la fin (`:253`).
            return Buttons {
                build: (self.blueprint == Blueprint::Found).then_some(ACTION_BUILD_KEY),
                repair: None,
                deconstruct: false,
                pickup: false,
                style_nav: false,
            };
        }

        let mut buttons = if self.level == 0 {
            // Niveau 0 : « construire », ni entretien ni démolition, `pickup` montré
            // (`:140-146`). Ce test passe **avant** `canBeUpgraded`.
            Buttons {
                build: Some(ACTION_BUILD_KEY),
                repair: None,
                deconstruct: false,
                pickup: true,
                style_nav: true,
            }
        } else if self.upgradable() {
            Buttons {
                build: Some(ACTION_UPGRADE_KEY),
                repair: Some(REPAIR_KEY),
                deconstruct: true,
                pickup: self.deconstructed,
                style_nav: true,
            }
        } else {
            // Au maximum, ou bloqué par le parent : `build` est caché (`:147-150`).
            Buttons {
                build: None,
                repair: Some(REPAIR_KEY),
                deconstruct: true,
                pickup: self.deconstructed,
                style_nav: true,
            }
        };

        if self.deconstructed {
            // Un bâtiment déconstruit se reconstruit : `repair` prend le libellé de
            // construction et `pickup` réapparaît (`:156-160`).
            buttons.repair = Some(ACTION_BUILD_KEY);
            buttons.pickup = true;
        }
        if self.blueprint == Blueprint::Missing {
            // `blueprint == null` : `build` et `repair` cachés, `pickup` montré (`:323-329`).
            buttons.build = None;
            buttons.repair = None;
            buttons.pickup = true;
        }
        buttons
    }

    /// La route du clic `build` (`:207-211`).
    pub fn confirm_route(&self) -> Route {
        match self.warning {
            Some(warning) if !warning.is_empty() => {
                Route::ConfirmDialog { title: CONFIRM_TITLE_KEY, warning }
            }
            _ => Route::Direct,
        }
    }

    /// Le mode du clic : `REPAIR` au niveau maximum, `BUILD` sinon (`:222-229`).
    pub fn confirm_mode(&self) -> Mode {
        if self.level == self.max_level { Mode::Repair } else { Mode::Build }
    }
}

/// Les deux messages du clic `build`, dans l'ordre, avec le mode rempli (`:221-229`).
///
/// La table porte la **forme** (style puis demande), cette fonction le **contenu** : le
/// mode dépend du niveau.
pub fn confirm_press(mode: Mode) -> Press {
    const STYLE_THEN: [&[Message]; 3] = [
        &[
            Message::plain(BUILDING_SET_STYLE_MESSAGE),
            Message::with(BUILD_REQUEST_MESSAGE, "BUILD"),
        ],
        &[
            Message::plain(BUILDING_SET_STYLE_MESSAGE),
            Message::with(BUILD_REQUEST_MESSAGE, "REPAIR"),
        ],
        &[
            Message::plain(BUILDING_SET_STYLE_MESSAGE),
            Message::with(BUILD_REQUEST_MESSAGE, "REMOVE"),
        ],
    ];
    let sends = match mode {
        Mode::Build => STYLE_THEN[0],
        Mode::Repair => STYLE_THEN[1],
        Mode::Remove => STYLE_THEN[2],
    };
    Press::new(sends, Exit::Reopens)
}

/// La liste des styles : **toujours un élément**, le pack du parent s'il existe, sinon
/// celui du bâtiment (`:263-288`). Les deux branches font `new ArrayList<>(); add(...)`.
pub fn styles(pack: &str, parent_pack: Option<&str>) -> Vec<String> {
    vec![parent_pack.unwrap_or(pack).to_owned()]
}

/// Le chemin du blueprint du niveau visé : le **dernier caractère** est remplacé par le
/// niveau (`:314-320`). Java coupe par `substring`, donc par caractère.
pub fn next_blueprint_path(structure_path: &str, next_level: i32) -> Result<String, Refusal> {
    let mut characters: Vec<char> = structure_path.replace(".blueprint", "").chars().collect();
    // `if (name.isEmpty()) return;` (`:316-319`) : la fenêtre rend la main sans rien dire.
    if characters.pop().is_none() {
        return Err(Refusal::BlueprintMissing);
    }
    Ok(format!("{}{}.blueprint", characters.iter().collect::<String>(), next_level))
}

/// Un builder proposable dans la liste déroulante (`:250-255`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub hut: Kind,
    /// `getWorkerName()` (`AbstractBuildingBuilderView.java:38-41`).
    pub worker: String,
    /// `getPosition()`, la clé du tri (`:254`).
    pub at: [i32; 3],
    /// `getBuildingLevel()`, lu seulement par la décoration (`WindowBuildDecoration.java:174`).
    pub level: i32,
}

/// Pourquoi ce bâtiment n'entre pas dans la liste, dans l'ordre des tests de Java
/// (`:251-252`, puis `WindowBuildDecoration.java:174`).
///
/// Le test de la mine est **mort** : seul `BuildingBuilder.View` hérite
/// d'`AbstractBuildingBuilderView` (`BuildingBuilder.java:213`), donc `hut != Builder` l'a
/// déjà refusé. Le port le garde : c'est ce que le source dit.
pub fn candidate_allowed(candidate: &Candidate, decoration: bool) -> Result<(), Refusal> {
    if candidate.hut != Kind::Builder {
        return Err(Refusal::NotABuilderHut);
    }
    if candidate.worker.is_empty() {
        return Err(Refusal::NoWorker);
    }
    if candidate.hut == Kind::Miner {
        return Err(Refusal::MinerHut);
    }
    if decoration && candidate.level <= 0 {
        return Err(Refusal::Port(crate::ui::action::Refusal::NotBuilt));
    }
    Ok(())
}

/// La distance au carré, l'unité de `distSqr` (`:254`).
pub fn distance(from: [i32; 3], to: [i32; 3]) -> i64 {
    let (x, y, z) = (from[0] - to[0], from[1] - to[1], from[2] - to[2]);
    i64::from(x) * i64::from(x) + i64::from(y) * i64::from(y) + i64::from(z) * i64::from(z)
}

/// La liste que la fenêtre affiche, du plus proche au plus loin.
///
/// Le tri de Java est **stable** (`:254`), donc deux distances égales gardent l'ordre
/// d'entrée : le port n'invente pas de second critère.
pub fn candidates_near(input: Vec<Candidate>, from: [i32; 3], decoration: bool) -> Vec<Candidate> {
    let mut kept: Vec<Candidate> = input
        .into_iter()
        .filter(|candidate| candidate_allowed(candidate, decoration).is_ok())
        .collect();
    kept.sort_by_key(|candidate| distance(from, candidate.at));
    kept
}

/// L'index choisi dans la liste des builders : `0` veut dire « **n'importe lequel** », car
/// le premier libellé est `builder:` à `BlockPos.ZERO` (`:249`, `:187`).
pub fn builder_choice(selected: i32, count: usize) -> Result<Option<usize>, Refusal> {
    if selected == 0 {
        return Ok(None);
    }
    let index = usize::try_from(selected).map_err(|_| Refusal::IndexOutOfRange)?;
    if index >= count {
        return Err(Refusal::IndexOutOfRange);
    }
    Ok(Some(index))
}

/// Une ressource dont la construction aura besoin (`:363-382`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Needed {
    /// `getDescriptionId() + "-" + hashCode du tag` (`:369-370`).
    pub key: String,
    /// Ce qu'il en faut, cumulé (`:379`).
    pub amount: i32,
}

/// Ajoute une pile, en cumulant par clé (`:363-382`). Java revient sans rien dire sur un
/// compte nul (`:365-368`) : [`Refusal::EmptyStack`], sans clé, pour qu'on puisse le compter.
pub fn add_needed(list: &mut Vec<Needed>, id: &str, tag: i32, amount: i32) -> Result<(), Refusal> {
    if amount == 0 {
        return Err(Refusal::EmptyStack);
    }
    let key = format!("{id}-{tag}");
    match list.iter_mut().find(|needed| needed.key == key) {
        Some(needed) => needed.amount += amount,
        None => list.push(Needed { key, amount }),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(level: i32, max_level: i32) -> State {
        State {
            level,
            max_level,
            parent: None,
            deconstructed: false,
            warning: None,
            blueprint: Blueprint::Found,
        }
    }

    fn builder(name: &str, at: [i32; 3], level: i32) -> Candidate {
        Candidate { hut: Kind::Builder, worker: name.to_owned(), at, level }
    }

    #[test]
    fn upgradable_follows_the_parent_rule_of_can_be_upgraded() {
        assert!(state(3, 5).upgradable());
        assert!(!state(5, 5).upgradable(), "WindowBuildBuilding.java:170");
        let cases = [
            (0, 5, 0, 5, false), // un parent non construit bloque l'enfant
            (2, 5, 3, 5, true),  // un parent plus haut laisse passer
            (2, 5, 2, 5, false), // un parent au même niveau bloque…
            (2, 5, 2, 2, true),  // …sauf s'il est lui-même au maximum
        ];
        for (level, max, parent, parent_max, expected) in cases {
            let mut state = state(level, max);
            state.parent = Some(Parent { level: parent, max_level: parent_max });
            assert_eq!(state.upgradable(), expected, "enfant {level}/{max}, parent {parent}/{parent_max}");
        }
        // Le niveau demandé suit la même réponse (`:308-312`).
        assert_eq!(state(2, 5).next_level(), 3);
        assert_eq!(state(5, 5).next_level(), 5);
    }

    #[test]
    fn the_buttons_follow_the_constructor_then_the_blueprint() {
        // Niveau 0 : construire, ni entretien ni démolition, `pickup` montré (`:140-146`).
        let zero = state(0, 5).buttons(Flavour::Building);
        assert_eq!(zero.build, Some(ACTION_BUILD_KEY));
        assert_eq!(zero.repair, None, ":143");
        assert!(!zero.deconstruct, ":144");
        assert!(zero.pickup, ":145");
        // Au maximum : `build` caché (`:147-150`).
        let full = state(5, 5).buttons(Flavour::Building);
        assert_eq!(full.build, None, ":149");
        assert_eq!(full.repair, Some(REPAIR_KEY));
        assert!(!full.pickup);
        // Déconstruit : `repair` devient « construire » et `pickup` revient (`:156-160`).
        let mut deconstructed = state(3, 5);
        deconstructed.deconstructed = true;
        let buttons = deconstructed.buttons(Flavour::Building);
        assert_eq!(buttons.repair, Some(ACTION_BUILD_KEY), ":158");
        assert!(buttons.pickup, ":159");
        assert_eq!(buttons.build, Some(ACTION_UPGRADE_KEY));
        // Blueprint absent : `build` et `repair` cachés, `pickup` montré (`:323-329`).
        let mut missing = state(2, 5);
        missing.blueprint = Blueprint::Missing;
        let buttons = missing.buttons(Flavour::Building);
        assert_eq!(buttons.build, None, ":325");
        assert_eq!(buttons.repair, None, ":326");
        assert!(buttons.pickup, ":327");
        assert!(buttons.deconstruct, ":323-329 ne touche pas la démolition");
        // `Pending` ne cache rien : l'état du constructeur tient.
        let mut pending = state(2, 5);
        pending.blueprint = Blueprint::Pending;
        assert_eq!(pending.buttons(Flavour::Building).build, Some(ACTION_UPGRADE_KEY));
    }

    #[test]
    fn the_decoration_shows_build_only_once_the_blueprint_is_read() {
        let mut pending = state(1, 5);
        pending.blueprint = Blueprint::Pending;
        let buttons = pending.buttons(Flavour::Decoration);
        assert_eq!(buttons.build, None, "WindowBuildDecoration.java:124");
        assert_eq!(buttons.repair, None, ":126");
        assert!(!buttons.deconstruct, ":125");
        assert!(!buttons.pickup, "la décoration ne montre jamais pickup");
        assert!(!buttons.style_nav, ":127-130");
        assert_eq!(
            state(1, 5).buttons(Flavour::Decoration).build,
            Some(ACTION_BUILD_KEY),
            ":253"
        );
    }

    #[test]
    fn the_build_click_sends_the_style_then_the_request_and_may_ask_first() {
        assert_eq!(A_BUILD.sends()[0].class, BUILDING_SET_STYLE_MESSAGE, ":221");
        assert_eq!(A_BUILD.sends()[1].class, BUILD_REQUEST_MESSAGE, ":228");
        assert_eq!(A_BUILD.press.exit, Exit::Reopens);
        assert_eq!(state(3, 5).confirm_mode(), Mode::Build);
        assert_eq!(confirm_press(state(3, 5).confirm_mode()).sends[1].payload, Some("BUILD"));
        assert_eq!(state(5, 5).confirm_mode(), Mode::Repair, ":224");
        assert_eq!(confirm_press(Mode::Repair).sends[1].payload, Some("REPAIR"));
        assert_eq!(A_REPAIR.sends()[0].payload, Some("REPAIR"), ":239");
        assert_eq!(A_DECONSTRUCT.sends()[0].payload, Some("REMOVE"), ":188");
        assert_eq!(A_PICKUP.press.exit, Exit::Closes, ":179");
        assert_eq!(RESOURCE_RETRY_TICKS, 20, ":300");
        // Un avertissement de niveau insère `WindowConfirm`, une chaîne vide non (`:207-211`).
        let mut warning = state(3, 5);
        warning.warning = Some("com.minecolonies.core.gui.build.warning");
        assert_eq!(
            warning.confirm_route(),
            Route::ConfirmDialog {
                title: CONFIRM_TITLE_KEY,
                warning: "com.minecolonies.core.gui.build.warning",
            }
        );
        assert_eq!(state(3, 5).confirm_route(), Route::Direct);
        let mut empty = state(3, 5);
        empty.warning = Some("");
        assert_eq!(empty.confirm_route(), Route::Direct);
    }

    #[test]
    fn the_builder_list_filters_in_javas_order_and_sorts_by_distance() {
        let far = builder("Anna", [10, 0, 0], 2);
        let near = builder("Bob", [1, 0, 0], 2);
        let unbuilt = builder("Carl", [0, 0, 0], 0);
        let empty = builder("", [0, 0, 0], 3);
        let miner = Candidate { hut: Kind::Miner, ..builder("Dora", [0, 0, 0], 3) };
        let all = || vec![far.clone(), near.clone(), unbuilt.clone(), empty.clone(), miner.clone()];

        // Le bâtiment accepte un builder de niveau 0 ; le tri est celui de la distance.
        let kept = candidates_near(all(), [0, 0, 0], false);
        assert_eq!(
            kept.iter().map(|c| c.worker.as_str()).collect::<Vec<_>>(),
            ["Carl", "Bob", "Anna"],
            "WindowBuildBuilding.java:250-255"
        );
        // La décoration ajoute `level > 0` : le même Carl n'y est plus.
        let decorated = candidates_near(all(), [0, 0, 0], true);
        assert_eq!(
            decorated.iter().map(|c| c.worker.as_str()).collect::<Vec<_>>(),
            ["Bob", "Anna"],
            "WindowBuildDecoration.java:174"
        );
        // L'ordre des refus est celui de Java, et le test de la mine est mort.
        assert_eq!(candidate_allowed(&empty, false), Err(Refusal::NoWorker));
        assert_eq!(candidate_allowed(&miner, false), Err(Refusal::NotABuilderHut));
        assert_eq!(
            candidate_allowed(&unbuilt, true),
            Err(Refusal::Port(crate::ui::action::Refusal::NotBuilt))
        );
        assert_eq!(candidate_allowed(&unbuilt, false), Ok(()));
        // L'index 0 n'est pas un builder mais « n'importe lequel » (`:187`).
        assert_eq!(builder_choice(0, 4), Ok(None));
        assert_eq!(builder_choice(2, 4), Ok(Some(2)));
        assert_eq!(builder_choice(4, 4), Err(Refusal::IndexOutOfRange));
        assert_eq!(builder_choice(-1, 4), Err(Refusal::IndexOutOfRange));
    }

    #[test]
    fn the_style_list_has_one_entry_and_the_next_path_replaces_the_last_character() {
        assert_eq!(styles("caledonia", None), ["caledonia"]);
        assert_eq!(styles("caledonia", Some("darkoak")), ["darkoak"]);
        assert_eq!(
            styles("caledonia", None).len(),
            1,
            "updateStyles n'ajoute qu'un élément (WindowBuildBuilding.java:263-288)"
        );
        assert_eq!(next_blueprint_path("builder1.blueprint", 2), Ok("builder2.blueprint".to_owned()));
        // Le niveau est un caractère : « builder10 » perd son dernier chiffre.
        assert_eq!(
            next_blueprint_path("builder10.blueprint", 2),
            Ok("builder12.blueprint".to_owned())
        );
        // Non ASCII : le port coupe par caractère, comme `substring` (sinon il paniquerait).
        assert_eq!(
            next_blueprint_path("bâtiment1.blueprint", 3),
            Ok("bâtiment3.blueprint".to_owned())
        );
        assert_eq!(next_blueprint_path("", 2), Err(Refusal::BlueprintMissing));
        // Une liste d'un élément : `previousStyle`/`nextStyle` ne changent rien (`:453-464`).
        let mut style = match Dropdown::new(STYLE_DROPDOWN, Source::BUILDING, styles("caledonia", None), 0) {
            Ok(dropdown) => dropdown,
            Err(refusal) => panic!("un style valide est refusé : {refusal}"),
        };
        assert_eq!(style.next(), 0);
        assert_eq!(style.previous(), 0);
        assert_eq!(style.label(), Some("caledonia"));
    }

    #[test]
    fn needed_resources_cumulate_per_item_and_tag() {
        let mut list = Vec::new();
        assert_eq!(add_needed(&mut list, "minecraft:oak_planks", 0, 4), Ok(()));
        assert_eq!(add_needed(&mut list, "minecraft:oak_planks", 0, 6), Ok(()));
        assert_eq!(add_needed(&mut list, "minecraft:oak_planks", 7, 2), Ok(()));
        assert_eq!(list.len(), 2, "deux tags, deux lignes (:369-370)");
        assert_eq!(list[0], Needed { key: "minecraft:oak_planks-0".to_owned(), amount: 10 });
        assert_eq!(list[1].amount, 2);
        // Une pile de zéro n'entre pas, et Java ne le dit pas (:365-368).
        assert_eq!(add_needed(&mut list, "minecraft:stone", 0, 0), Err(Refusal::EmptyStack));
        assert_eq!(list.len(), 2);
        assert_eq!(Refusal::EmptyStack.key(), None);
    }

    #[test]
    fn the_decoration_sends_the_message_its_caller_supplies() {
        assert_eq!(A_DECORATION_BUILD.sends()[0].class, WORK_ORDER_PARAMETER);
        // Ce n'est pas une classe Java : c'est le nom du paramètre (`:83`).
        assert!(
            WORK_ORDER_PARAMETER.chars().next().is_some_and(char::is_lowercase),
            "buildRequestMessage est un paramètre, pas un message (WindowBuildDecoration.java:83)"
        );
        assert_eq!(A_DECORATION_BUILD.press.exit, Exit::Closes, ":340");
        assert_eq!(A_CANCEL.press.exit, Exit::Reopens);
        assert!(A_DECORATION_BUILD.label.is_written(), ":123 écrit ACTION_BUILD");
        assert_eq!(A_DECORATION_BUILD.label.key(), ACTION_BUILD_KEY);
        assert_eq!(DECORATION_ACTIONS.len(), 2, ":120-121");
        assert_eq!(ACTIONS.len(), 7, "windowbuildbuilding.xml:5-58");
    }
}
