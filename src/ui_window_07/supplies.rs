//! Les deux fenetres des caisses de depart : le placement, et le recit qui le precede.
//!
//! Java reference : `core/client/gui/WindowSupplies.java` (195 lignes),
//! `core/client/gui/WindowSupplyStory.java` (124), leurs XML `windowsupplies.xml` et
//! `windowsupplystory.xml`, les deux verificateurs de placement
//! `core/items/ItemSupplyCampDeployer.java:130-181` / `ItemSupplyChestDeployer.java:158-204`,
//! `api/items/ISupplyItem.java:8-9` (les deux constantes du balayage),
//! `api/util/constant/Constants.java:98` / `:103` (l'etiquette NBT) et
//! `core/event/ColonyStoryListener.java:64-98` (le corpus de recits).
//!
//! **`WindowSupplies` n'est pas une fenetre a elle.** Elle etend la fenetre de
//! manipulation de Structurize (`:40`), qui enregistre **douze** boutons
//! (`AbstractBlueprintManipulationWindow.java:107-118`) ; elle en possede **un**
//! (`switch`, `:65`) et en remplace **deux** (`confirm`, `:128` ; `cancel`, `:120`). Les
//! **dix** autres font ce que fait l'outil de construction de Structurize, et ce fichier les
//! nomme sans les decrire ([`INHERITED`]).
//!
//! **`up` ne monte pas.** Les ids des quatre panneaux de deplacement mentent :
//! `BUTTON_FORWARD = "up"` et `BUTTON_UP = "plus"` (Structurize
//! `api/util/constant/WindowConstants.java:166`, `:21`), donc le panneau nomme `up` avance
//! et celui nomme `plus` monte. [`INHERITED`] porte les deux lignes, et un test les tient.
//!
//! **Rien ne se passe si le pack ou le blueprint manque**, et c'est un vrai silence :
//! `handlePlacement` revient sur `structurePack == null || blueprint == null`
//! (`:138-141`) sans message, sans son, sans fermeture — [`Outcome::NothingLoaded`].
//!
//! **Le type et l'item sont deux entrees independantes.** Le type (`"supplycamp"` ou
//! autre) choisit le blueprint et le style de sol (`:64`) ; l'item tenu en main choisit le
//! **corpus de recits** (`:81`, `:95`). Aucune ligne ne verifie qu'ils s'accordent : un item
//! de caisse avec le type `supplyship` raconte une caisse et place un navire.

use crate::ui::field::Value;

use super::pane::{Button, Exit, Label, Message, Pane, Press, Source, Window};
use super::refusal::Refusal;

/// L'id de la fenetre de placement.
pub const SUPPLIES_ID: &str = "supplies";
/// L'id de la fenetre de recit.
pub const STORY_ID: &str = "supply.story";

/// Le seul type qui n'est pas un navire (`WindowSupplies.java:64` compare a cette chaine).
pub const CAMP_TYPE: &str = "supplycamp";
/// Le type du navire, que Java ne compare jamais : tout le reste en est un (`:64`, `:161`).
pub const SHIP_TYPE: &str = "supplyship";
/// L'identifiant du gestionnaire de placement (`SuppliesHandler.ID`,
/// `core/placementhandlers/main/SuppliesHandler.java:37`, passe en `WindowSupplies.java:131`).
pub const HANDLER_ID: &str = "minecolonies:supplies";

/// `GROUNDSTYLE_LEGACY_CAMP` (Structurize `api/util/constant/Constants.java:24`).
pub const GROUNDSTYLE_CAMP: i32 = 2;
/// `GROUNDSTYLE_LEGACY_SHIP` (Structurize `api/util/constant/Constants.java:25`).
pub const GROUNDSTYLE_SHIP: i32 = 3;

/// La part du balayage qu'on tolere : un tiers de l'emprise (`ISupplyItem.java:9`, compare
/// en `:173` pour la caisse et `:196` pour le navire).
pub const TOLERANCE: f64 = 1.0 / 3.0;

/// La cle du message d'echec (`TranslationConstants.java:494`, envoye en `:183`).
pub const BAD_BLOCKS_KEY: &str = "item.supply.badblocks";
/// Le prefixe de la cle d'une faute (`TranslationConstants.java:736`) : Java y colle le nom
/// du type en minuscules (`:189`).
pub const FAULT_KEY_PREFIX: &str = "item.supply.error.";

/// La categorie des surbrillances de placement (`WindowSupplies.java:45`).
pub const HIGHLIGHT_KEY: &str = "placement";
/// La couleur d'une boite de faute, ARGB (`:190`).
pub const HIGHLIGHT_ARGB: u32 = 0x80FF_0000;
/// Sa duree, en secondes (`:191`, `Duration.ofSeconds(60)`).
pub const HIGHLIGHT_SECONDS: u64 = 60;

/// L'etiquette NBT du mode de placement (`Constants.java:98`).
pub const PLACEMENT_TAG: &str = "Placement";
/// La valeur « place tout de suite », la seule qui ajoute un recit (`Constants.java:103`,
/// comparee en `WindowSupplyStory.java:76`).
pub const INSTANT: &str = "instant";
/// La graine du tirage, sur l'item (`NbtTagConstants.java:759`, lue en `:78`).
pub const RANDOM_TAG: &str = "randkey";
/// La distance a laquelle la fenetre pose la caisse quand on ne lui donne pas de position
/// (`ISupplyItem.java:8`, utilisee en `:60`).
pub const OFFSET_DISTANCE: i32 = 10;

/// Une des deux caisses de depart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java n'a que deux branches, `:64` et `:144`/`:161`
pub enum Kind {
    Camp,
    Ship,
}

impl Kind {
    /// Le type d'apres la chaine du constructeur (`WindowSupplies.java:64`).
    ///
    /// **Tout ce qui n'est pas exactement [`CAMP_TYPE`] est un navire**, `"supplyship"`
    /// comme `"banane"` : Java teste l'egalite avec la caisse, jamais avec le navire.
    pub fn of(type_name: &str) -> Kind {
        if type_name == CAMP_TYPE {
            Kind::Camp
        } else {
            Kind::Ship
        }
    }

    /// Le type que cette caisse ecrit dans ses chemins.
    pub fn type_name(self) -> &'static str {
        match self {
            Kind::Camp => CAMP_TYPE,
            Kind::Ship => SHIP_TYPE,
        }
    }

    /// Le style de sol passe a la fenetre mere (`:64`) — **le legacy**, pas le relatif :
    /// la suggestion d'outil, elle, ouvre `GROUNDSTYLE_RELATIVE`
    /// ([`super::suggest::GROUNDSTYLE_RELATIVE`]).
    pub fn ground_style(self) -> i32 {
        match self {
            Kind::Camp => GROUNDSTYLE_CAMP,
            Kind::Ship => GROUNDSTYLE_SHIP,
        }
    }

    /// Le blueprint cherche dans le pack (`:95` pour le filtre, `:108` pour le chargement).
    pub fn blueprint_path(self) -> String {
        format!("decorations/supplies/{}.blueprint", self.type_name())
    }

    /// La ligne d'aide du recit (`WindowSupplyStory.java:97`, `:101`).
    pub fn guide_key(self) -> &'static str {
        match self {
            Kind::Camp => "com.minecolonies.core.gui.supplycamp.guide",
            Kind::Ship => "com.minecolonies.core.gui.supplyship.guide",
        }
    }

    /// Le champ d'item qui decide du corpus (`WindowSupplyStory.java:81`, `:95`) —
    /// **l'autre entree**, celle qui ne suit pas [`Kind::type_name`].
    pub fn item_field(self) -> &'static str {
        match self {
            Kind::Camp => "supplyCamp",
            Kind::Ship => "supplyChest",
        }
    }

    /// Le corpus de recits (`ColonyStoryListener.java:64-65`).
    pub fn story_set(self) -> &'static str {
        match self {
            Kind::Camp => "supplyCampStories",
            Kind::Ship => "supplyShipStories",
        }
    }

    /// Les trois familles de fautes que ce balayage peut lever.
    ///
    /// Les deux verificateurs sont disjoints : la caisse demande un sol solide
    /// (`ItemSupplyCampDeployer.java:161-169`) et le navire de l'eau
    /// (`ItemSupplyChestDeployer.java:184-191`) ; les deux verifient l'air au-dessus et la
    /// colonie.
    pub fn faults(self) -> &'static [Fault] {
        match self {
            Kind::Camp => &[Fault::NotSolid, Fault::InsideColony, Fault::NeedsAirAbove],
            Kind::Ship => &[Fault::NotWater, Fault::InsideColony, Fault::NeedsAirAbove],
        }
    }

    /// Si ce balayage peut lever cette faute.
    pub fn can_raise(self, fault: Fault) -> bool {
        self.faults().contains(&fault)
    }
}

/// Une faute de placement, telle que `PlacementError.PlacementErrorType`
/// (Structurize `placement/handlers/placement/PlacementError.java:13-19`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre types dans l'enumeration de Structurize, et quatre ici
pub enum Fault {
    /// Pas de sol solide (`ItemSupplyCampDeployer.java:197`).
    NotSolid,
    /// La position est dans une colonie ou le joueur ne peut pas poser
    /// (`ItemSupplyCampDeployer.java:200`, `ItemSupplyChestDeployer.java:235`).
    InsideColony,
    /// Un bloc solide la ou il faut de l'air
    /// (`ItemSupplyCampDeployer.java:168`, `ItemSupplyChestDeployer.java:190`).
    NeedsAirAbove,
    /// Pas d'eau — ou de lave hors de l'overworld
    /// (`ItemSupplyChestDeployer.java:225`, `:229`).
    NotWater,
}

impl Fault {
    /// Les quatre types, dans l'ordre de l'enumeration Java.
    pub const ALL: [Fault; 4] =
        [Fault::NotSolid, Fault::InsideColony, Fault::NeedsAirAbove, Fault::NotWater];

    /// La cle de la phrase d'une boite : `item.supply.error.<type en minuscules>`
    /// (`WindowSupplies.java:189`, cle de base `TranslationConstants.java:736`).
    pub fn key(self) -> &'static str {
        match self {
            Fault::NotSolid => "item.supply.error.not_solid",
            Fault::InsideColony => "item.supply.error.inside_colony",
            Fault::NeedsAirAbove => "item.supply.error.needs_air_above",
            Fault::NotWater => "item.supply.error.not_water",
        }
    }

    /// Le nom du type Java, en minuscules : ce que `getType().toString().toLowerCase()`
    /// rend (`WindowSupplies.java:189`).
    pub fn slug(self) -> &'static str {
        match self {
            Fault::NotSolid => "not_solid",
            Fault::InsideColony => "inside_colony",
            Fault::NeedsAirAbove => "needs_air_above",
            Fault::NotWater => "not_water",
        }
    }
}

/// Ce que fait `confirm` (`WindowSupplies.java:134-194`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: trois issues, et la premiere est un silence
pub enum Outcome {
    /// Ni pack ni blueprint : `return` immediat, sans un mot (`:138-141`).
    NothingLoaded,
    /// Le message part et la fenetre se ferme par `cancelClicked()` (`:157`, `:175`).
    Placed,
    /// Le joueur recoit [`BAD_BLOCKS_KEY`] et chaque faute une boite de soixante secondes
    /// (`:181-193`). Les familles sont celles de Java, dans son ordre.
    Blocked(Vec<Fault>),
}

/// Le nombre de fautes tolere : `n > emprise * 1/3` refuse (`:173`, `:196`).
///
/// La comparaison est stricte, et c'est du flottant cote Java : `3` fautes sur une emprise
/// de `9` passent, `4` non.
pub fn too_many(faults: usize, footprint: i32) -> bool {
    faults as f64 > f64::from(footprint) * TOLERANCE
}

/// Le clic `confirm`, reduit a sa regle de decision (`:134-194`).
///
/// `counted` est ce que le balayage du monde a trouve, par type de faute. Le port n'a pas
/// de monde : il a ce compte. `footprint` est `sizeX * sizeZ` (`:173`, `:196`), et
/// `loaded` repond a « pack et blueprint presents » (`:138`).
///
/// **Une faute que ce type ne peut pas lever n'est pas comptee** : Java ne peut pas la
/// produire ([`Kind::faults`]), donc la compter inventerait un refus.
pub fn attempt(kind: Kind, loaded: bool, counted: &[(Fault, usize)], footprint: i32) -> Outcome {
    if !loaded {
        return Outcome::NothingLoaded;
    }
    let count = |fault: Fault| -> usize {
        if !kind.can_raise(fault) {
            return 0;
        }
        counted.iter().filter(|(other, _)| *other == fault).map(|(_, n)| *n).sum()
    };
    let air = count(Fault::NeedsAirAbove);
    // `:173` : l'air d'un cote, le sol ou l'eau de l'autre, chaque famille toleree a part.
    let base = match kind {
        Kind::Camp => count(Fault::NotSolid) + count(Fault::InsideColony),
        Kind::Ship => count(Fault::NotWater) + count(Fault::InsideColony),
    };
    if !too_many(air, footprint) && !too_many(base, footprint) {
        return Outcome::Placed;
    }
    // `:175-176` et `:198-199` ajoutent **les deux** listes des que l'une depasse.
    let mut faults = Vec::new();
    if air > 0 {
        faults.push(Fault::NeedsAirAbove);
    }
    for fault in kind.faults() {
        if *fault != Fault::NeedsAirAbove && count(*fault) > 0 {
            faults.push(*fault);
        }
    }
    Outcome::Blocked(faults)
}

/// La position que la fenetre de recit choisit quand on ne lui en donne pas
/// (`WindowSupplyStory.java:58-65`).
///
/// Java fait **un** appel : `blockPosition().relative(getDirection(), SUPPLY_OFFSET_DISTANCE)`
/// (`:60`), et `BlockPos.relative(direction, n)` avance de `n` blocs — `n` valant
/// [`OFFSET_DISTANCE`] (`ISupplyItem.java:8`). [`Facing::offset`] est le vecteur **d'un**
/// pas, donc c'est ici qu'il est multiplie : la destination est a dix blocs, pas a un.
pub fn default_pos(pos: [i32; 3], facing: Facing) -> [i32; 3] {
    let [dx, dy, dz] = facing.offset();
    [
        pos[0] + dx * OFFSET_DISTANCE,
        pos[1] + dy * OFFSET_DISTANCE,
        pos[2] + dz * OFFSET_DISTANCE,
    ]
}

/// Le cote vers lequel le joueur regarde.
///
/// Quatre valeurs, pas six : `Entity.getDirection()` se construit sur le lacet
/// (`Direction.fromYRot`), donc un joueur ne regarde jamais vers le haut ni vers le bas.
/// C'est une regle du jeu, pas une ligne de nos sources — la ligne de notre source est
/// l'appel, `WindowSupplyStory.java:60`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre directions horizontales
pub enum Facing {
    North,
    South,
    West,
    East,
}

impl Facing {
    /// Les quatre, dans l'ordre du jeu.
    pub const ALL: [Facing; 4] = [Facing::North, Facing::South, Facing::West, Facing::East];

    /// Le vecteur d'un pas dans cette direction.
    pub fn offset(self) -> [i32; 3] {
        match self {
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::West => [-1, 0, 0],
            Facing::East => [1, 0, 0],
        }
    }
}

/// La main qui tient la caisse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux mains
pub enum Hand {
    Main,
    Off,
}

impl Hand {
    /// La charge utile du message (`WindowSupplyStory.java:113`), en clair.
    pub fn name(self) -> &'static str {
        match self {
            Hand::Main => "main_hand",
            Hand::Off => "off_hand",
        }
    }
}

/// Une ligne du panneau `text` du recit (`WindowSupplyStory.java:74-104`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq sortes de lignes que Java ajoute
pub enum StoryLine {
    /// Une phrase du corpus, tiree pour ce biome (`:83`, `:87`) : du **texte**, pas une cle.
    Rolled,
    /// `Component.empty()` (`:89`, `:93`).
    Blank,
    /// `com.minecolonies.core.gui.supplies.guide`, avec le nom de l'item (`:92`).
    Guide,
    /// L'aide de la caisse (`:97`).
    CampGuide,
    /// L'aide du navire (`:101`).
    ShipGuide,
}

impl StoryLine {
    /// La cle, quand la ligne en a une.
    pub fn key(self) -> Option<&'static str> {
        match self {
            StoryLine::Guide => Some("com.minecolonies.core.gui.supplies.guide"),
            StoryLine::CampGuide => Some(Kind::Camp.guide_key()),
            StoryLine::ShipGuide => Some(Kind::Ship.guide_key()),
            StoryLine::Rolled | StoryLine::Blank => None,
        }
    }
}

/// Les lignes du recit, dans l'ordre de Java (`:74-102`).
///
/// `instant` est la comparaison de l'etiquette NBT a [`INSTANT`] (`:76`). Sans elle le
/// panneau porte **trois** lignes, avec elle **cinq** — et les deux premieres sont la
/// phrase tiree puis une ligne vide.
pub fn story(kind: Kind, instant: bool) -> Vec<StoryLine> {
    let mut lines = Vec::new();
    if instant {
        lines.push(StoryLine::Rolled);
        lines.push(StoryLine::Blank);
    }
    lines.push(StoryLine::Guide);
    lines.push(StoryLine::Blank);
    lines.push(match kind {
        Kind::Camp => StoryLine::CampGuide,
        Kind::Ship => StoryLine::ShipGuide,
    });
    lines
}

/// Le tirage d'une phrase (`ColonyStoryListener.java:88-98`).
///
/// `matches` est `StoryText.allMatches(stories, biome)`, `draw` le `nextInt(size)` de Java.
/// **Vide rend `None`**, et Java ajoute quand meme la ligne (`:83` puis `:89`) : un recit
/// sans correspondance commence donc par deux lignes vides.
pub fn roll<'a>(matches: &[&'a str], draw: usize) -> Option<&'a str> {
    matches.get(draw).copied()
}

/// Les deux envois de `place`, un par main (`WindowSupplyStory.java:113`) : la charge utile
/// du message est la main qui tient l'item.
///
/// Deux `const`, parce qu'un `&[Message]` bati dans le corps de [`press_place`] ne serait pas
/// promu en `'static` (`E0716`) alors qu'un tableau dans un initialiseur `const` l'est. La
/// table de boutons reprend celui de la main principale, et un test verifie que les deux
/// disent la meme chose.
const MAIN_HAND_SENDS: &[Message] = &[Message::with("MarkStoryReadOnItemMessage", "main_hand")];
const OFF_HAND_SENDS: &[Message] = &[Message::with("MarkStoryReadOnItemMessage", "off_hand")];

/// Le clic de la fenetre de recit : envoyer, puis ouvrir le placement (`:111-115`).
pub fn press_place(hand: Hand) -> Press {
    let sends = match hand {
        Hand::Main => MAIN_HAND_SENDS,
        Hand::Off => OFF_HAND_SENDS,
    };
    Press::new(sends, Exit::OpensHere(SUPPLIES_ID))
}

/// Le clic de `colonyStylePicker` et de `switch` : le choix de pack
/// (`WindowSupplyStory.java:120-123`, `WindowSupplies.java:88-96`).
///
/// **Pas de filtre dans le recit**, contrairement a `WindowSupplies.switchPackClicked`
/// (`:95`) qui exige que le pack contienne `decorations/supplies/<type>.blueprint` : depuis
/// le recit, on peut choisir un pack qui n'a pas de caisse, et la fenetre suivante ne
/// chargera rien.
///
/// Const, parce que deux tables de boutons ci-dessous l'utilisent dans un initialiseur
/// constant.
pub const PACK_PRESS: Press = Press::new(&[], Exit::OpensJava("WindowSwitchPack"));

/// Le clic du choix de pack, en fonction.
pub fn press_style() -> Press {
    PACK_PRESS
}

/// Les douze boutons que la fenetre de placement herite de Structurize
/// (`AbstractBlueprintManipulationWindow.java:107-118`), par `(panneau, gestionnaire, ligne)`.
///
/// Les ids ne sont pas ceux qu'on croit : voir l'en-tete du module pour `up` et `plus`.
pub const INHERITED: &[(&str, &str, u32)] = &[
    ("confirm", "confirmClicked", 107),
    ("cancel", "cancelClicked", 108),
    ("left", "moveLeftClicked", 109),
    ("mirror", "mirrorClicked", 110),
    ("right", "moveRightClicked", 111),
    // `BUTTON_BACKWARD = "down"` (`WindowConstants.java:171`).
    ("down", "moveBackClicked", 112),
    // `BUTTON_FORWARD = "up"` (`:166`).
    ("up", "moveForwardClicked", 113),
    // `BUTTON_UP = "plus"` (`:21`).
    ("plus", "moveUpClicked", 114),
    // `BUTTON_DOWN = "minus"` (`:26`).
    ("minus", "moveDownClicked", 115),
    ("rotateRight", "rotateRightClicked", 116),
    ("rotateLeft", "rotateLeftClicked", 117),
    ("settings", "settingsClicked", 118),
];

/// Combien des douze boutons herites cette fenetre remplace : `confirm` (`:128-132`) et
/// `cancel` (`:120-126`).
pub const OVERRIDDEN: u32 = 2;

/// La fenetre de placement : un panneau a elle, trois boutons dont deux herites.
const PLACEMENT_PANES: &[Pane] = &[
    // `:116` — l'aide de manipulation, cachee des que le blueprint est charge. Le XML la
    // pose (`layoutmanipulation.xml:4`) et la fenetre mere la montre selon la position
    // (`AbstractBlueprintManipulationWindow.java:122`).
    Pane::new("tip", Value::Text, Source::Xml),
];

const PLACEMENT_BUTTONS: &[Button] = &[
    // `:65`, `:88-96` — le seul bouton que cette classe possede.
    Button {
        id: "switch",
        label: Label::Xml("structurize.gui.buildtool.switchpack"),
        press: PACK_PRESS,
        refusals: &[],
        declared: Some(("windowsupplies.xml", 7)),
    },
    // `:128-132` — la branche heureuse ; les deux autres issues sont dans `attempt`.
    Button {
        id: "confirm",
        label: Label::Image,
        press: Press::new(
            &[Message::with("BuildToolPlacementMessage", "Survival")],
            Exit::Closes,
        ),
        refusals: &[Refusal::NothingToPlace],
        declared: Some(("layoutmanipulation.xml", 31)),
    },
    // `:120-126` — nettoie les surbrillances, jette le blueprint, ferme.
    Button {
        id: "cancel",
        label: Label::Image,
        press: Press::CLOSES,
        refusals: &[],
        declared: Some(("layoutmanipulation.xml", 32)),
    },
];

/// `WindowSupplies` : le placement d'une caisse ou d'un navire.
pub static PLACEMENT_WINDOW: Window = Window {
    id: SUPPLIES_ID,
    java: "WindowSupplies",
    resource: "minecolonies:gui/windowsupplies.xml",
    title: None,
    panes: PLACEMENT_PANES,
    buttons: PLACEMENT_BUTTONS,
};

/// La fenetre de recit : deux lignes de prose et trois boutons.
const STORY_PANES: &[Pane] = &[
    // `:104` — le panneau recoit une **liste** de composants, pas une phrase.
    Pane::new("text", Value::Text, Source::Story),
];

const STORY_BUTTONS: &[Button] = &[
    // `:70` — `registerButton(BUTTON_CANCEL, this::close)`.
    Button {
        id: "cancel",
        label: Label::Image,
        press: Press::CLOSES,
        refusals: &[],
        declared: Some(("windowsupplystory.xml", 6)),
    },
    // `:71`, `:120-123` — le choix de pack, sans filtre.
    Button {
        id: "colonyStylePicker",
        label: Label::Xml("com.minecolonies.core.gui.supplies.selectstyle"),
        press: PACK_PRESS,
        refusals: &[],
        declared: Some(("windowsupplystory.xml", 10)),
    },
    // `:72`, `:111-115` — et son libelle est ecrit apres coup, avec le nom de l'item
    // (`:105`) ; la main est une donnee du constructeur, donc `press_place` la prend. La
    // valeur ci-dessous est la main principale, celle du cas courant.
    Button {
        id: "place",
        label: Label::Written("com.minecolonies.core.gui.supplies.place"),
        press: Press::new(MAIN_HAND_SENDS, Exit::OpensHere(SUPPLIES_ID)),
        refusals: &[],
        declared: Some(("windowsupplystory.xml", 15)),
    },
];

/// `WindowSupplyStory` : le recit, puis [`SUPPLIES_ID`].
pub static STORY_WINDOW: Window = Window {
    id: STORY_ID,
    java: "WindowSupplyStory",
    resource: "minecolonies:gui/windowsupplystory.xml",
    title: None,
    panes: STORY_PANES,
    buttons: STORY_BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camp_and_a_ship_are_two_ground_styles_two_paths_and_two_guides() {
        assert_eq!(Kind::Camp.type_name(), "supplycamp");
        assert_eq!(Kind::Ship.type_name(), "supplyship");
        // `:64` : le style legacy, pas le relatif de la suggestion d'outil.
        assert_eq!(Kind::Camp.ground_style(), GROUNDSTYLE_CAMP);
        assert_eq!(Kind::Ship.ground_style(), GROUNDSTYLE_SHIP);
        assert_ne!(Kind::Camp.ground_style(), super::super::suggest::GROUNDSTYLE_RELATIVE);
        assert_eq!(Kind::Camp.blueprint_path(), "decorations/supplies/supplycamp.blueprint");
        assert_eq!(Kind::Ship.blueprint_path(), "decorations/supplies/supplyship.blueprint");
        assert_eq!(Kind::Camp.guide_key(), "com.minecolonies.core.gui.supplycamp.guide");
        assert_eq!(Kind::Ship.guide_key(), "com.minecolonies.core.gui.supplyship.guide");
    }

    #[test]
    fn any_type_that_is_not_the_camp_is_a_ship_and_the_item_is_another_input() {
        // `:64` ne compare qu'a la caisse.
        assert_eq!(Kind::of(CAMP_TYPE), Kind::Camp);
        assert_eq!(Kind::of(SHIP_TYPE), Kind::Ship);
        assert_eq!(Kind::of("banane"), Kind::Ship);
        assert_eq!(Kind::of(""), Kind::Ship);
        // Le corpus vient de l'item (`:81`, `:95`), pas du type : deux entrees, deux index.
        assert_eq!(Kind::Camp.item_field(), "supplyCamp");
        assert_eq!(Kind::Ship.item_field(), "supplyChest");
        assert_ne!(Kind::Camp.item_field(), Kind::Ship.item_field());
        assert_eq!(Kind::Camp.story_set(), "supplyCampStories");
        assert_eq!(Kind::Ship.story_set(), "supplyShipStories");
    }

    #[test]
    fn a_third_of_the_footprint_is_tolerated_and_one_more_is_not() {
        // `:173` : `n > emprise * 1/3`, comparaison stricte.
        assert!(!too_many(3, 9));
        assert!(too_many(4, 9));
        assert!(!too_many(10, 30));
        assert!(too_many(11, 30));
        assert!(!too_many(0, 0));
        assert!(too_many(1, 0), "une emprise nulle ne tolere aucune faute");
    }

    #[test]
    fn a_camp_never_raises_a_water_fault_and_a_ship_never_a_missing_floor() {
        assert_eq!(
            Kind::Camp.faults(),
            &[Fault::NotSolid, Fault::InsideColony, Fault::NeedsAirAbove]
        );
        assert_eq!(
            Kind::Ship.faults(),
            &[Fault::NotWater, Fault::InsideColony, Fault::NeedsAirAbove]
        );
        assert!(!Kind::Camp.can_raise(Fault::NotWater));
        assert!(!Kind::Ship.can_raise(Fault::NotSolid));
        assert!(Kind::Camp.can_raise(Fault::InsideColony));
        assert_eq!(Fault::ALL.len(), 4);
        // Le message d'echec est celui-la, et il ne dit pas *quelles* fautes (`:183`).
        assert_eq!(BAD_BLOCKS_KEY, "item.supply.badblocks");
        // Une faute que ce type ne peut pas lever n'est pas comptee du tout.
        assert_eq!(
            attempt(Kind::Camp, true, &[(Fault::NotWater, 100)], 9),
            Outcome::Placed
        );
        for fault in Fault::ALL {
            assert_eq!(fault.key(), format!("{FAULT_KEY_PREFIX}{}", fault.slug()));
        }
    }

    #[test]
    fn an_empty_scan_places_and_a_missing_pack_places_nothing() {
        assert_eq!(attempt(Kind::Camp, true, &[], 9), Outcome::Placed);
        // `:138-141` : ni pack ni blueprint, retour muet, pas meme un `Blocked`.
        assert_eq!(attempt(Kind::Camp, false, &[], 9), Outcome::NothingLoaded);
        assert_eq!(
            attempt(Kind::Ship, false, &[(Fault::NotWater, 9)], 9),
            Outcome::NothingLoaded
        );
    }

    #[test]
    fn four_faults_on_a_nine_block_footprint_block_the_placement_and_name_the_family() {
        // Une seule famille depasse : les deux listes sont quand meme rapportees
        // (`:175-176`), dans l'ordre air puis sol.
        let outcome = attempt(
            Kind::Camp,
            true,
            &[(Fault::NeedsAirAbove, 4), (Fault::NotSolid, 1), (Fault::InsideColony, 1)],
            9,
        );
        assert_eq!(
            outcome,
            Outcome::Blocked(vec![Fault::NeedsAirAbove, Fault::NotSolid, Fault::InsideColony])
        );
        // L'autre famille depasse, et l'air reste rapporte en tete.
        let other = attempt(
            Kind::Ship,
            true,
            &[(Fault::NotWater, 5), (Fault::InsideColony, 2), (Fault::NeedsAirAbove, 1)],
            9,
        );
        assert_eq!(
            other,
            Outcome::Blocked(vec![Fault::NeedsAirAbove, Fault::NotWater, Fault::InsideColony])
        );
        // Deux fautes, deux familles, sous le tiers : ca passe.
        assert_eq!(
            attempt(Kind::Camp, true, &[(Fault::NeedsAirAbove, 2), (Fault::NotSolid, 1)], 9),
            Outcome::Placed
        );
    }

    #[test]
    fn the_instant_story_has_two_more_lines_and_starts_with_the_roll() {
        let plain = story(Kind::Camp, false);
        assert_eq!(plain, [StoryLine::Guide, StoryLine::Blank, StoryLine::CampGuide]);
        let instant = story(Kind::Camp, true);
        assert_eq!(
            instant,
            [
                StoryLine::Rolled,
                StoryLine::Blank,
                StoryLine::Guide,
                StoryLine::Blank,
                StoryLine::CampGuide
            ]
        );
        assert_eq!(instant.len(), plain.len() + 2);
        // La derniere ligne depend du type, pas de la main.
        assert_eq!(
            story(Kind::Ship, false).last().copied(),
            Some(StoryLine::ShipGuide)
        );
        // Deux lignes vides se suivent quand il n'y a pas de tirage, et une seule sinon :
        // c'est ce que `:83` + `:89` produisent.
        assert_eq!(StoryLine::Rolled.key(), None);
        assert_eq!(StoryLine::Blank.key(), None);
        assert_eq!(
            StoryLine::Guide.key(),
            Some("com.minecolonies.core.gui.supplies.guide")
        );
        // Les trois mots du source qui decident du recit (`Constants.java:98`, `:103`,
        // `NbtTagConstants.java:759`).
        assert_eq!(PLACEMENT_TAG, "Placement");
        assert_eq!(INSTANT, "instant");
        assert_eq!(RANDOM_TAG, "randkey");
    }

    #[test]
    fn a_story_with_no_match_still_takes_its_line_and_the_roll_is_a_draw() {
        assert_eq!(roll(&[], 0), None);
        assert_eq!(roll(&["a", "b"], 1), Some("b"));
        // Java tire `nextInt(size)`, donc l'index est toujours valide ; un index hors
        // bornes rend `None` plutot que de lever.
        assert_eq!(roll(&["a"], 2), None);
        // Et la ligne vide du tirage manque n'empeche pas les deux suivantes.
        assert_eq!(story(Kind::Ship, true).len(), 5);
    }

    #[test]
    fn the_default_position_is_ten_blocks_away_in_the_facing_direction() {
        let at = [100, 64, -20];
        assert_eq!(default_pos(at, Facing::North), [100, 64, -30]);
        assert_eq!(default_pos(at, Facing::South), [100, 64, -10]);
        assert_eq!(default_pos(at, Facing::West), [90, 64, -20]);
        assert_eq!(default_pos(at, Facing::East), [110, 64, -20]);
        assert_eq!(Facing::ALL.len(), 4, "un joueur ne regarde ni en haut ni en bas");
        for facing in Facing::ALL {
            let moved = default_pos(at, facing);
            let distance: i32 =
                (moved[0] - at[0]).abs() + (moved[1] - at[1]).abs() + (moved[2] - at[2]).abs();
            assert_eq!(distance, OFFSET_DISTANCE);
        }
    }

    #[test]
    fn the_inherited_buttons_say_what_each_pane_moves() {
        assert_eq!(INHERITED.len(), 12);
        let handler = |pane: &str| {
            INHERITED
                .iter()
                .find(|(id, _, _)| *id == pane)
                .map(|(_, handler, _)| *handler)
        };
        // Les deux pieges : `up` avance, `plus` monte.
        assert_eq!(handler("up"), Some("moveForwardClicked"));
        assert_eq!(handler("plus"), Some("moveUpClicked"));
        assert_eq!(handler("down"), Some("moveBackClicked"));
        assert_eq!(handler("minus"), Some("moveDownClicked"));
        // Deux boutons sont remplaces par la fenetre, dix ne le sont pas.
        assert_eq!(OVERRIDDEN, 2);
        assert_eq!(handler("confirm"), Some("confirmClicked"));
        assert_eq!(handler("cancel"), Some("cancelClicked"));
        assert_eq!(INHERITED.len() as u32 - OVERRIDDEN, 10);
    }

    #[test]
    fn the_placement_window_owns_one_button_and_the_story_window_three() {
        assert_eq!(PLACEMENT_WINDOW.buttons.len(), 3);
        assert_eq!(PLACEMENT_WINDOW.reachable(), ["switch", "confirm", "cancel"]);
        assert_eq!(PLACEMENT_WINDOW.resource, "minecolonies:gui/windowsupplies.xml");
        assert_eq!(PLACEMENT_WINDOW.title, None);
        // Un seul panneau a valeur dans cette fenetre : le reste est la fenetre mere.
        assert_eq!(PLACEMENT_WINDOW.panes.len(), 1);
        assert_eq!(
            PLACEMENT_WINDOW.pane("tip").map(|pane| pane.source),
            Some(Source::Xml)
        );
        assert!(PLACEMENT_WINDOW
            .button("confirm")
            .is_some_and(|button| button.is_guarded()));
        // Le recit : trois boutons, tous atteignables, et un seul panneau.
        assert_eq!(STORY_WINDOW.buttons.len(), 3);
        assert_eq!(STORY_WINDOW.reachable(), ["cancel", "colonyStylePicker", "place"]);
        assert_eq!(STORY_WINDOW.pane("text").map(|pane| pane.value), Some(Value::Text));
        assert_eq!(STORY_WINDOW.pane("text").map(|pane| pane.source), Some(Source::Story));
        // `place` ferme le recit et ouvre le placement, message puis fenetre.
        let place = STORY_WINDOW.button("place").unwrap_or_else(|| panic!("pas de bouton place"));
        assert_eq!(place.press.exit, Exit::OpensHere(SUPPLIES_ID));
        assert_eq!(place.sends()[0].payload, Some("main_hand"));
        assert_eq!(press_place(Hand::Off).sends[0].payload, Some("off_hand"));
        // Les deux constantes d'envoi portent le nom de leur main (`:113`).
        assert_eq!(MAIN_HAND_SENDS[0].payload, Some(Hand::Main.name()));
        assert_eq!(OFF_HAND_SENDS[0].payload, Some(Hand::Off.name()));
        // La table porte la main principale : la fonction et la constante s'accordent.
        assert_eq!(press_place(Hand::Main), place.press);
        assert!(place.label.is_written(), ":105 ecrit le libelle apres coup");
        // Le message de `confirm` ne tient qu'un mot ; le gestionnaire est une constante,
        // pas la charge utile (`:131`).
        let confirm =
            PLACEMENT_WINDOW.button("confirm").unwrap_or_else(|| panic!("pas de bouton confirm"));
        assert_eq!(confirm.sends()[0].payload, Some("Survival"));
        assert_eq!(HANDLER_ID, "minecolonies:supplies");
        // Les deux fenetres partagent le meme choix de pack, sans filtre.
        assert_eq!(press_style(), PACK_PRESS);
        assert_eq!(STORY_WINDOW.button("colonyStylePicker").map(|button| button.press), Some(PACK_PRESS));
        assert_eq!(PLACEMENT_WINDOW.button("switch").map(|button| button.press), Some(PACK_PRESS));
        // Les trois constantes du rendu d'une faute (`:45`, `:190`, `:191`).
        assert_eq!(HIGHLIGHT_KEY, "placement");
        assert_eq!(HIGHLIGHT_ARGB, 0x80FF_0000);
        assert_eq!(HIGHLIGHT_SECONDS, 60);
    }
}
