//! L'etat d'un module de batiment **tel que sa fenetre le consomme** : un niveau, une
//! progression, un refus.
//!
//! Java reference: `core/client/gui/modules/building/` — **24** fenetres de module, de
//! `ToolModuleWindow.java` (40 lignes) a `RestaurantMenuModuleWindow.java` (575), toutes
//! montees sur `AbstractModuleWindow` (`core/client/gui/AbstractModuleWindow.java:23-53`),
//! et les vues de module qu'elles lisent (`core/colony/buildings/moduleviews/`).
//!
//! **Neuf de ces vingt-quatre fenetres sont deja decrites ailleurs, et ce module ne les
//! redécrit pas** : `WindowBuilderResModule` ([`crate::ui_window_11`]), `WindowHutMinerModule`,
//! `WindowMineGuardModule`, `WindowHutRequestTaskModule` et `WindowListRecipes`
//! ([`crate::ui_window_12`]), `WindowSelectRequest` et `WindowStatsModule`
//! ([`crate::ui_window_13`]), `MinimumStockModuleWindow` ([`crate::ui_window_05`]),
//! `RequestTreeWindowModule` ([`crate::ui_window_05`]). Le tableau [`WINDOWS`] ne garde donc
//! que les **quinze** autres, et un test verifie qu'aucune des neuf ne s'y est glissee.
//!
//! # Ce que la fenetre lit d'un module, et qui n'est ni un panneau ni un bouton
//!
//! Trois choses, toujours les memes, et aucune n'est un [`crate::ui::Field`] :
//!
//! 1. **un niveau** — celui du module (`getStorageUpgradeLevel`, `getBuildingLevel`), qui
//!    n'est pas le niveau de la hutte et qui decide d'un bouton ;
//! 2. **une progression** — deux entiers que la fenetre divise (l'amelioration de
//!    l'entrepot, `WarehouseOptionsModuleWindow.java:118-120`), ou une liste qui se
//!    remplit petit a petit ;
//! 3. **un refus** — un bouton desactive, une ligne en rouge, une fenetre qui ne s'ouvre
//!    pas. Java n'a pas de type pour ca : c'est un `disable()` par-ci
//!    (`WarehouseOptionsModuleWindow.java:125`), un `setText` rouge par-la, et un
//!    `tooltipBuilder` ailleurs. [`Refusal`] leur donne un nom chacun.
//!
//! # Quatre regles que la source dit et qui surprennent
//!
//! 1. **Le bouton de tri de l'entrepot s'arrete au niveau 3**, un litteral local
//!    (`WarehouseOptionsModuleWindow.java:42`, `BUILDING_LEVEL_FOR_SORTING = 3`), et le
//!    message a deux lignes de cle (`:72-73`).
//! 2. **La ligne de l'entrepot ne parle que de blocs d'emeraude**, en dur
//!    (`new ItemStack(Blocks.EMERALD_BLOCK, 1)`, `:91`) : c'est l'amelioration de
//!    stockage, pas une ressource.
//! 3. **Le restaurant desactive puis reactive le meme bouton** : `if (hasReachedLimit)
//!    switchButton.disable()` (`:555-560`) est suivi d'un `if (isInMenu) disable else
//!    enable` (`:562-569`) qui **ecrase** le premier. Un plat qui n'est pas encore au
//!    menu reste donc ajoutable a la limite, avec l'infobulle "limite atteinte" dessus.
//! 4. **La consommation du restaurant divise par zero** quand le menu est vide :
//!    `(avgCustomerConsumption * 10 * numerOfCustomers) / saturationSum`
//!    (`:319`), sans garde. En `double` Java rend `Infinity` ou `NaN`, il ne leve pas ;
//!    [`restaurant_consumption`] rend `None` et nomme le cas.
//!
//! # Ce qui n'est pas ici
//!
//! Les panneaux eux-memes : `layoutsettings.xml` et les quatorze autres XML ne sont pas
//! portes, et `src/ui/MANQUANT.md` §1 dit deja qu'il n'y a **pas de widget de liste**.
//! Les quinze fenetres sont donc decrites par ce qu'elles lisent et par leur regle, pas
//! par leur dessin.

/// La cle de la description d'un outil : `"com.minecolonies.coremod.gui.tooldesc." +
/// <chemin de l'item>` (`ToolModuleWindow.java:30-31`).
pub const TOOL_DESC_PREFIX: &str = "com.minecolonies.coremod.gui.tooldesc.";

/// La cle d'une option de reglage : `"com.minecolonies.coremod.setting." +
/// <id unique>` (`SettingsModuleWindow.java:107`).
pub const SETTING_PREFIX: &str = "com.minecolonies.coremod.setting.";

/// Le panneau `id` d'une ligne de reglage, celui qui permet de recycler la ligne
/// (`SettingsModuleWindow.java:81-99`).
pub const SETTING_ID_PANE: &str = "id";

/// Le panneau `desc` d'une ligne de reglage (`SettingsModuleWindow.java:104-108`).
pub const SETTING_DESC_PANE: &str = "desc";

/// Le panneau ou vont les reglages (`WindowConstants.java:1159`, `settingslist`).
pub const SETTINGS_LIST: &str = "settingslist";

/// Le bouton d'inventaire de l'universite (`WindowConstants.java:563`, `invresearch`), que
/// la fenetre enregistre pour ouvrir l'inventaire (`UniversityModuleWindow.java:50`).
pub const UNIVERSITY_INVENTORY: &str = "invresearch";

/// La clé du libellé "1 sur 3" de l'amélioration d'entrepôt
/// (`TranslationConstants.java:534`, `com.minecolonies.coremod.gui.xofz`), dont le texte
/// anglais est `"%d of %d"` (`lang/manual_en_us.json:2029`).
pub const X_OF_Z_KEY: &str = "com.minecolonies.coremod.gui.xofz";

/// La cle de la liste des ressources filtree (`WindowConstants.java:1124`).
pub const FILTER_INPUT: &str = "input";

/// La cle du bouton de bascule des listes filtrees (`WindowConstants.java:1134`).
pub const SWITCH_BUTTON: &str = "switch";

/// La cle du bouton de remise a zero (`WindowConstants.java:1139`).
pub const RESET_BUTTON: &str = "resetDefault";

/// La cle du bouton "on" (`TranslationConstants.java:71`).
pub const ON_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.retrieveon";

/// La cle du bouton "off" (`TranslationConstants.java:73`).
pub const OFF_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.retrieveoff";

/// La clé du compteur de champs : deux arguments, possédés et maximum
/// (`GuiTranslationConstants.java:19`), texte anglais `"%d/%d fields in use"`
/// (`lang/manual_en_us.json:626`).
pub const FIELD_COUNT_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.fields.fieldcount";

/// La clé d'une distance de champ, deux arguments : la distance et la direction courte
/// (`GuiTranslationConstants.java:27`), texte anglais `"Distance: %s %s"`
/// (`lang/manual_en_us.json:629`).
pub const FIELD_DISTANCE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.fields.distance";

/// La cle du statut de champ (`TranslationConstants.java:528`).
pub const FIELD_STATUS_KEY: &str = "com.minecolonies.coremod.gui.field.status";

/// Le libelle "plus disponible" des deux menus a liste limitee
/// (`MinimumStockModuleWindow.java:34`, `RestaurantMenuModuleWindow.java:52`).
pub const LIMIT_REACHED_KEY: &str = "com.minecolonies.coremod.gui.warehouse.limitreached";

/// La clé commune des trois raisons de refuser un ordre de travail
/// (`WorkOrderModuleWindow.java:203`, `:208`, `:213`) : trois clés distinctes, toutes sous
/// `tile.blockhut.*` (`TranslationConstants.java:832`, `:834`, `:838`).
pub const NO_WORKER_KEY: &str = "tile.blockhut.noworkerassigned";
/// `MESSAGE_WARNING_ALREADY_CLAIMED`.
pub const ALREADY_CLAIMED_KEY: &str = "tile.blockhut.alreadyclaimed";
/// `MESSAGE_WARNING_CANNOTBUILD`.
pub const CANNOT_BUILD_KEY: &str = "tile.blockhut.cannotbuild";

/// Le libelle du bouton d'un ordre deja pris par cette hutte
/// (`WorkOrderModuleWindow.java:219`).
pub const CANCEL_KEY: &str = "com.minecolonies.coremod.gui.builder.cancel";
/// Le libelle quand c'est le joueur qui choisit (`:224`).
pub const SELECT_KEY: &str = "com.minecolonies.coremod.gui.builder.select";

/// Le niveau d'entrepot a partir duquel le tri est disponible
/// (`WarehouseOptionsModuleWindow.java:42`).
pub const SORTING_LEVEL: i32 = 3;

/// Le nombre maximal d'ameliorations de stockage (`BuildingWareHouse.java:51`) : **la
/// constante est celle de `ui_window_10::rack`**, pas une seconde definition.
pub use crate::ui_window_10::rack::MAX_STORAGE_UPGRADE;

/// Le delai avant de re-filtrer une liste, en ticks
/// (`ItemListModuleWindow.java:80`, `EntityListModuleWindow.java:77`,
/// `RestaurantMenuModuleWindow.java:123` : `this.tick = 10`).
pub const FILTER_DEBOUNCE: i32 = 10;

/// Le rafraichissement de la liste des ordres de travail, en ticks
/// (`WorkOrderModuleWindow.java:104` : `if (tick++ == 20)`).
pub const WORK_ORDER_REFRESH: i32 = 20;

/// Le rafraichissement du journal d'expedition, en ticks
/// (`ExpeditionLogModuleWindow.java:44-53` : `tick = 20` puis
/// `MarkBuildingDirtyMessage`).
pub const EXPEDITION_REFRESH: i32 = 20;

/// Le cout d'un saut de porte, en pepites d'or **par bloc de 125**
/// (`ConnectionModuleWindow.java:114` : `final int itemCount = externalPlayer ?
/// dist/125 : 0`).
pub const GATE_COST_PER_BLOCKS: i32 = 125;

/// Pourquoi une fenetre de module refuse quelque chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les refus que ces quinze fenetres nomment, et pas un de plus
pub enum Refusal {
    /// Le batiment n'a pas le niveau qu'un bouton demande
    /// (`WarehouseOptionsModuleWindow.java:68-76` : tri sous le niveau 3).
    LevelTooLow { need: i32, got: i32 },
    /// Aucun citoyen n'est assigne a la hutte, donc l'ordre ne peut pas etre pris
    /// (`WorkOrderModuleWindow.java:200-206`).
    NoWorker,
    /// L'ordre est deja pris ailleurs (`:206-210`).
    AlreadyClaimed,
    /// La hutte ne peut pas encore construire cet ordre (`:211-216`).
    CannotBuildYet,
    /// La liste est a son maximum (`MinimumStockModuleWindow.java:64-69`).
    AtLimit,
    /// L'entree est deja dans le menu (`RestaurantMenuModuleWindow.java:564-571`).
    AlreadyListed,
    /// Le module refuse la pile : la ligne passe en rouge et le bouton s'eteint
    /// (`WarehouseOptionsModuleWindow.java:124-129`, statut `DONT_HAVE`).
    Missing,
    /// La fenetre de ce module n'est pas dans le tableau : un id inconnu, refuse par un
    /// nom plutot que par un panneau vide.
    UnknownWindow { java: &'static str },
}

impl Refusal {
    /// Un nom stable, prefixes.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::LevelTooLow { .. } => "module.refusal.level",
            Refusal::NoWorker => "module.refusal.no_worker",
            Refusal::AlreadyClaimed => "module.refusal.claimed",
            Refusal::CannotBuildYet => "module.refusal.cannot_build",
            Refusal::AtLimit => "module.refusal.at_limit",
            Refusal::AlreadyListed => "module.refusal.already_listed",
            Refusal::Missing => "module.refusal.missing",
            Refusal::UnknownWindow { .. } => "module.refusal.unknown_window",
        }
    }

    /// La cle de traduction que Java affiche, quand il en affiche une.
    ///
    /// `None` n'est pas "pas de raison" : c'est "Java desactive un bouton et ne dit
    /// rien" (le tri sous le niveau 3 a bien une cle, la ligne rouge d'une ressource
    /// manquante n'en a pas).
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::NoWorker => Some(NO_WORKER_KEY),
            Refusal::AlreadyClaimed => Some(ALREADY_CLAIMED_KEY),
            Refusal::CannotBuildYet => Some(CANNOT_BUILD_KEY),
            Refusal::AtLimit => Some(LIMIT_REACHED_KEY),
            Refusal::LevelTooLow { .. }
            | Refusal::AlreadyListed
            | Refusal::Missing
            | Refusal::UnknownWindow { .. } => None,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Ce qu'une fenetre de module tient d'un module : son niveau, sa progression, son refus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// Le niveau que la fenetre lit. Ce n'est pas forcement celui de la hutte : l'entrepot
    /// lit son **amelioration de stockage** (`getStorageUpgradeLevel`) pour la ligne
    /// "1 sur 3" et le **niveau du batiment** pour le bouton de tri.
    pub level: i32,
    /// L'avancement, quand il y en a un.
    pub progress: Option<Progress>,
    /// Le refus en cours, s'il y en a un.
    pub refusal: Option<Refusal>,
}

/// Une progression en deux entiers, comme Java la garde avant de la mettre en texte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Ce qui est fait.
    pub step: i32,
    /// Ce qu'il faut.
    pub steps: i32,
}

impl Progress {
    /// Le pourcentage, par **la meme regle que le reste du port**
    /// ([`crate::ui::field::percent`]) : plancher, et zero quand il n'y a rien a faire.
    pub fn percent(self) -> i32 {
        crate::ui::field::percent(self.step, self.steps)
    }

    /// Le libellé "x sur z" (`WarehouseOptionsModuleWindow.java:118-120`), rendu comme le
    /// texte anglais de `com.minecolonies.coremod.gui.xofz` : `"%d of %d"`
    /// (`lang/manual_en_us.json:2029`).
    pub fn x_of_z(self) -> String {
        format!("{} of {}", self.step, self.steps)
    }

    /// Si la progression est finie — et "rien a faire" compte comme fini, comme dans
    /// [`crate::ui::field::percent`].
    pub fn done(self) -> bool {
        self.steps <= 0 || self.step >= self.steps
    }
}

/// La regle qu'une fenetre porte, nommee par la fonction qui la tient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : sept formes de regle, une par famille de fenetre
pub enum Rule {
    /// Une liste qui se filtre et dont chaque ligne a une bascule (item, entite, plat).
    FilteredList,
    /// Un reglage par ligne, avec recyclage de la ligne.
    SettingRows,
    /// Un inventaire d'entree vers l'entrepot, et un tri (niveau 3).
    WarehouseStorage,
    /// Un choix de branche de recherche, dont certaines cachees.
    ResearchBranches,
    /// Une liste de champs ou de tombes, avec une distance.
    PlacedThings,
    /// Un ordre de travail, avec trois raisons de refuser.
    WorkOrders,
    /// Une liste de citoyens a qui donner un ordre ou un outil.
    Citizens,
}

/// Une fenetre de module : sa classe, ses XML, la vue qu'elle lit, et sa regle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// La classe, sous `core/client/gui/modules/building/`.
    pub java: &'static str,
    /// Les XML sous `assets/minecolonies/gui/layouthuts/` : **un ou deux**. Deux fenetres
    /// prennent le leur en argument, et elles n'en ont pas le meme nombre :
    /// `SpecialAssignmentModuleWindow` en monte deux selon la vue qui l'ouvre
    /// ([`ASSIGNMENT_RESOURCES`]), `ItemListModuleWindow` un seul en pratique
    /// ([`ITEM_LIST_RESOURCE`]).
    pub resources: &'static [&'static str],
    /// La vue de module lue, par nom de classe. C'est une **vue de module** partout sauf
    /// pour `ConnectionModuleWindow`, qui prend une vue de batiment (`:64`) : la porte de
    /// connexion n'est pas un module du batiment, c'est la colonie.
    pub view: &'static str,
    /// La regle de la fenetre.
    pub rule: Rule,
}

/// Le seul XML que la liste filtrable monte en pratique : le view le passe au
/// constructeur (`ItemListModuleView.java:144`), et ce view est le seul a l'ouvrir.
pub const ITEM_LIST_RESOURCE: &str = "layoutfilterablelist.xml";

/// Les **deux** XML de la meme fenetre d'assignation, choisis par la vue qui l'ouvre :
/// le courier (`CourierAssignmentModuleView.java:125`) et le mineur
/// (`MinerAssignmentModuleView.java:124`). Une classe, deux ressources — et un troisieme
/// metier en ajouterait une troisieme.
pub const ASSIGNMENT_RESOURCES: [&'static str; 2] =
    ["layoutcourierassignment.xml", "layoutminerassignment.xml"];

/// Les **quinze** fenetres de module que les autres modules ne decrivent pas.
///
/// Un test verifie que les neuf autres ne sont pas ici : la liste est la difference entre
/// les vingt-quatre fichiers de `modules/building/` et les neuf deja portes.
pub const WINDOWS: &[Window] = &[
    Window {
        java: "ToolModuleWindow",
        resources: &["layouttool.xml"],
        view: "ToolModuleView",
        rule: Rule::Citizens,
    },
    Window {
        java: "SettingsModuleWindow",
        resources: &["layoutsettings.xml"],
        view: "SettingsModuleView",
        rule: Rule::SettingRows,
    },
    Window {
        java: "UniversityModuleWindow",
        resources: &["layoutuniversity.xml"],
        view: "UniversityResearchModuleView",
        rule: Rule::ResearchBranches,
    },
    Window {
        java: "WarehouseOptionsModuleWindow",
        resources: &["layoutwarehouseoptions.xml"],
        view: "WarehouseOptionsModuleView",
        rule: Rule::WarehouseStorage,
    },
    Window {
        java: "RestaurantMenuModuleWindow",
        resources: &["layoutfoodstock.xml"],
        view: "RestaurantMenuModuleView",
        rule: Rule::FilteredList,
    },
    Window {
        java: "ItemListModuleWindow",
        resources: &[ITEM_LIST_RESOURCE],
        view: "IItemListModuleView",
        rule: Rule::FilteredList,
    },
    Window {
        java: "EntityListModuleWindow",
        resources: &["layoutfilterableentitylist.xml"],
        view: "IEntityListModuleView",
        rule: Rule::FilteredList,
    },
    Window {
        java: "FarmFieldsModuleWindow",
        resources: &["layoutfarmfields.xml"],
        view: "FieldsModuleView",
        rule: Rule::PlacedThings,
    },
    Window {
        java: "PlantationFieldsModuleWindow",
        resources: &["layoutplantationfields.xml"],
        view: "FieldsModuleView",
        rule: Rule::PlacedThings,
    },
    Window {
        java: "GraveyardManagementWindow",
        resources: &["layoutgraveyard.xml"],
        view: "GraveyardManagementModuleView",
        rule: Rule::PlacedThings,
    },
    Window {
        java: "ExpeditionLogModuleWindow",
        resources: &["layoutexpeditionlog.xml"],
        view: "ExpeditionLogModuleView",
        rule: Rule::Citizens,
    },
    Window {
        java: "WorkOrderModuleWindow",
        resources: &["layoutworkorders.xml"],
        view: "WorkOrderListModuleView",
        rule: Rule::WorkOrders,
    },
    Window {
        java: "SpecialAssignmentModuleWindow",
        resources: &ASSIGNMENT_RESOURCES,
        view: "IAssignmentModuleView",
        rule: Rule::Citizens,
    },
    Window {
        java: "ConnectionModuleWindow",
        resources: &["layoutcolonyconnection.xml"],
        view: "IBuildingView",
        rule: Rule::Citizens,
    },
    Window {
        java: "DOCraftingWindow",
        resources: &["layoutdocrafting.xml"],
        view: "DOCraftingModuleView",
        rule: Rule::FilteredList,
    },
];

/// La fenetre de cette classe.
pub fn window(java: &str) -> Option<&'static Window> {
    WINDOWS.iter().find(|window| window.java == java)
}

/// La description d'un outil : la cle, ou `None` pour un identifiant d'item vide
/// (`ToolModuleWindow.java:30-31`).
pub fn tool_desc_key(item_path: &str) -> Option<String> {
    if item_path.is_empty() {
        return None;
    }
    Some(format!("{TOOL_DESC_PREFIX}{item_path}"))
}

/// Une ligne de reglage : faut-il defaire la ligne avant de la remplir ?
///
/// Java vide les enfants **seulement** si le panneau `id` existe et ne porte pas deja
/// l'identifiant de ce reglage (`SettingsModuleWindow.java:83-85`), puis remplit si la
/// ligne est vide (`:87-101`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : trois etats, le recyclage, la pose et le remplissage
pub enum Row {
    /// La ligne porte deja ce reglage : rien a reconstruire.
    Reuse,
    /// La ligne portait un autre reglage : on la vide et on la reconstruit.
    Relayout,
    /// La ligne n'a pas de panneau `id` : Java ne peut pas savoir ce qu'elle porte et
    /// **la reconstruit a chaque rafraichissement** (`:96-101`, avec un avertissement au
    /// log). Le port nomme cet etat au lieu de le logguer.
    NoIdentityPane,
}

/// Ce qu'il faut faire d'une ligne de reglage.
pub fn setting_row(has_id_pane: bool, text_matches: bool, children_empty: bool) -> Row {
    if !has_id_pane {
        return Row::NoIdentityPane;
    }
    if !text_matches {
        return Row::Relayout;
    }
    if children_empty {
        return Row::Relayout;
    }
    Row::Reuse
}

/// La description d'une ligne de reglage : `setting.<id unique>`
/// (`SettingsModuleWindow.java:107`).
pub fn setting_desc_key(unique_id: &str) -> Option<String> {
    if unique_id.is_empty() {
        return None;
    }
    Some(format!("{SETTING_PREFIX}{unique_id}"))
}

/// Une recherche primaire d'une branche : cachee ou non, et ses prerequis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Primary {
    /// `isHidden()` sur la recherche (`UniversityModuleWindow.java:79`).
    pub hidden: bool,
    /// Un drapeau par prerequis : satisfait ou non (`:112-119`).
    pub requirements: &'static [bool],
}

/// Une ligne de la liste des exigences d'une branche cachee
/// (`UniversityModuleWindow.java:74-109`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les trois sortes de ligne que Java empile
pub enum Line {
    /// Le mot "verrouille", une fois par branche (`:86-89`).
    Locked,
    /// "Ou" entre deux recherches primaires (`:90-93`).
    Or,
    /// Un prerequis, avec sa couleur : rouge s'il manque, cyan s'il est la (`:97-104`).
    Requirement { fulfilled: bool },
}

/// Si une branche apparait dans la liste de l'universite
/// (`UniversityModuleWindow.java:54-62`).
///
/// Une branche est **visible** quand sa liste d'exigences est vide — c'est-a-dire quand
/// **au moins une** de ses recherches primaires est visible (`:74-83`) — ou quand elle
/// n'est pas cachee. Une branche cachee dont toutes les primaires sont cachees et non
/// satisfaites disparait donc de la liste, et une branche **sans primaire** reste : la
/// boucle de Java ne tourne pas, donc la liste d'exigences est vide.
pub fn university_branch_visible(branch_hidden: bool, primaries: &[Primary]) -> bool {
    if !branch_hidden || primaries.is_empty() {
        return true;
    }
    primaries.iter().any(|primary| !primary.hidden || primary.requirements.iter().all(|done| *done))
}

/// Les lignes que la fenetre affiche pour une branche cachee
/// (`UniversityModuleWindow.java:86-105`).
pub fn university_lines(primaries: &[Primary]) -> Vec<Line> {
    let mut lines = Vec::new();
    for (index, primary) in primaries.iter().enumerate() {
        if index == 0 {
            lines.push(Line::Locked);
        } else {
            lines.push(Line::Or);
        }
        for done in primary.requirements {
            lines.push(Line::Requirement { fulfilled: *done });
        }
    }
    lines
}

/// Le statut d'une ressource dans l'entrepot (`BuildingBuilderResource.RessourceAvailability`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre statuts de BuildingBuilderResource
pub enum Availability {
    /// Rien en stock et la hutte en veut : bouton eteint, lignes rouges
    /// (`WarehouseOptionsModuleWindow.java:124-129`).
    DoNotHave,
    /// Il en manque : bouton allume, lignes rouges (`:130-135`).
    NeedMore,
    /// Assez : bouton allume, lignes vert fonce (`:136-141`).
    HaveEnough,
    /// Plus rien a faire : bouton eteint, lignes noires (`:142-171`).
    NotNeeded,
}

/// Le statut affiche, **avec les trois forcages de Java**
/// (`WarehouseOptionsModuleWindow.java:113-116`).
///
/// Le statut est force a [`Availability::NotNeeded`] quand l'amelioration est au maximum,
/// quand le batiment n'est pas encore au niveau maximal, ou quand la fenetre vient
/// d'envoyer un transfert (`lockUpgrade`, `:191-192`).
pub fn warehouse_availability(
    status: Availability,
    storage_level: i32,
    level: i32,
    max_level: i32,
    locked: bool,
) -> Availability {
    if storage_level >= MAX_STORAGE_UPGRADE || level < max_level || locked {
        Availability::NotNeeded
    } else {
        status
    }
}

/// Le bouton du transfert est-il allume ? (`WarehouseOptionsModuleWindow.java:124-171`)
pub fn warehouse_button(status: Availability) -> bool {
    matches!(status, Availability::NeedMore | Availability::HaveEnough)
}

/// La troisieme colonne de la ligne : `amount - available`
/// (`WarehouseOptionsModuleWindow.java:175`).
pub fn quantity_missing(amount: i32, available: i32) -> i32 {
    amount - available
}

/// La deuxieme colonne : le nombre de ressources qui manquent au **joueur**, ecrit
/// seulement s'il est negatif (`WarehouseOptionsModuleWindow.java:164-172`).
///
/// Java ecrit `getMissingFromPlayer()` quand il est negatif — donc un solde negatif, qui
/// veut dire "le joueur en a plus que necessaire" — et **efface** la ligne quand il est
/// positif ou nul. C'est l'inverse du nom de la methode, et le port garde la regle.
pub fn missing_from_player_text(missing: i32) -> Option<String> {
    if missing < 0 {
        Some(missing.to_string())
    } else {
        None
    }
}

/// Le tri de l'entrepot est-il disponible ? (`WarehouseOptionsModuleWindow.java:68`,
/// `:201`).
pub fn sorting_available(level: i32) -> bool {
    level >= SORTING_LEVEL
}

/// Si une pile passe le filtre d'une liste d'items
/// (`ItemListModuleWindow.java:139-141`).
///
/// Deux champs sont testes, en minuscules et sans casse : l'identifiant de description de
/// la pile et son nom affiche. **Pas** l'identifiant de registre.
pub fn item_list_matches(filter: &str, description_id: &str, hover_name: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    description_id.to_lowercase().contains(&needle) || hover_name.to_lowercase().contains(&needle)
}

/// Si une entite passe le filtre de sa liste
/// (`EntityListModuleWindow.java:141`).
///
/// Deux champs la aussi, mais pas les memes : le **nom d'affichage** du type d'entite et
/// son identifiant de registre complet.
pub fn entity_list_matches(filter: &str, description: &str, registry_id: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    description.to_lowercase().contains(&needle) || registry_id.to_lowercase().contains(&needle)
}

/// L'ordre de deux lignes d'une liste filtree
/// (`ItemListModuleWindow.java:157-176`, `EntityListModuleWindow.java:158-178`).
///
/// Les elements **autorises** passent devant, sauf si la liste est inversee. Le tri est
/// stable et ne departage rien d'autre : deux elements du meme camp gardent leur ordre
/// (`else { return 0; }`).
pub fn allowed_first(a_allowed: bool, b_allowed: bool, inverted: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if !a_allowed && b_allowed {
        if inverted { Ordering::Less } else { Ordering::Greater }
    } else if a_allowed && !b_allowed {
        if inverted { Ordering::Greater } else { Ordering::Less }
    } else {
        Ordering::Equal
    }
}

/// Le libelle de la bascule d'une ligne : `ON` quand la ligne est dans l'etat actif
/// (`ItemListModuleWindow.java:215-222`).
pub fn switch_label(allowed: bool, inverted: bool) -> &'static str {
    if (inverted && !allowed) || (!inverted && allowed) {
        ON_KEY
    } else {
        OFF_KEY
    }
}

/// Ce qu'un clic sur la bascule fait (`ItemListModuleWindow.java:107-119`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux effets, ajouter ou retirer
pub enum Switch {
    /// `moduleView.addItem(item)`.
    Add,
    /// `moduleView.removeItem(item)`.
    Remove,
}

/// Ajouter ou retirer, **lu sur le libelle du bouton** comme Java le fait
/// (`ItemListModuleWindow.java:109-112`).
///
/// Le test de Java est `on = button.getText().equals(translatable(ON))` puis
/// `add = (on && inverted) || (!on && !inverted)` : le libelle est donc l'entree, pas une
/// sortie, et l'addition n'a lieu que si le libelle **s'accorde** avec l'inversion de la
/// liste. C'est ce qui rend la regle inversable sans toucher au clic.
pub fn switch_action(label_is_on: bool, inverted: bool) -> Switch {
    if label_is_on == inverted {
        Switch::Add
    } else {
        Switch::Remove
    }
}

/// La ligne de distance d'un champ
/// (`FarmFieldsModuleWindow.java:184-191`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux branches, verticale ou non
pub enum FieldLine {
    /// Une direction verticale : Java n'ecrit que le texte long, **sans distance**
    /// (`:188`).
    Vertical,
    /// Une direction horizontale : la distance et la direction courte (`:189`).
    Distance,
}

/// La ligne d'un champ, selon la direction
/// (`FarmFieldsModuleWindow.java:187-191`).
pub fn field_line(vertical: bool) -> FieldLine {
    if vertical {
        FieldLine::Vertical
    } else {
        FieldLine::Distance
    }
}

/// Le texte de distance d'un champ : **la distance au carrée, suffixée "m"**, qui est le
/// premier `%s` de `FIELD_DISTANCE_KEY` (`FarmFieldsModuleWindow.java:184`, `:189`).
///
/// `field.getSqDistance(buildingView)` rend des blocs au carré et Java n'en prend pas la
/// racine : un champ à 30 blocs s'affiche "900m". Ce n'est pas une erreur d'unité du port,
/// c'est celle de Java, et le test la fige.
pub fn field_distance_argument(squared: i32) -> String {
    format!("{squared}m")
}

/// Les deux arguments du compteur de champs (`FarmFieldsModuleWindow.java:146`).
///
/// La phrase est `"%d/%d fields in use"` (`lang/manual_en_us.json:626`) : le port rend les
/// deux nombres, pas la phrase, parce qu'il n'a pas de moteur de traduction.
pub fn field_count_arguments(owned: usize, max: usize) -> (usize, usize) {
    (owned, max)
}

/// Le bouton d'assignation d'un champ est-il actif ?
/// (`FarmFieldsModuleWindow.java:195` : `setEnabled(moduleView.assignFieldManually())`).
pub fn field_assign_enabled(manual: bool) -> bool {
    manual
}

/// Le groupe d'affichage d'un ordre de travail
/// (`WorkOrderModuleWindow.java:157-166`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustif : trois groupes, tries dans cet ordre
pub enum WorkOrderGroup {
    /// Celui que la hutte construit en ce moment (`:154-157`).
    Current,
    /// Deja pris par cette hutte (`:158-161`).
    Mine,
    /// Libre, ou a quelqu'un d'autre (`:162`).
    Other,
}

/// Le groupe d'un ordre.
pub fn work_order_group(is_current: bool, claimed_by_self: bool) -> WorkOrderGroup {
    if is_current {
        WorkOrderGroup::Current
    } else if claimed_by_self {
        WorkOrderGroup::Mine
    } else {
        WorkOrderGroup::Other
    }
}

/// Si un ordre apparait dans la liste (`WorkOrderModuleWindow.java:117-136`).
///
/// `shouldShow` puis `inRange` pour tout le monde ; puis, en mode manuel, "pris par moi
/// **ou** libre", et sinon "pris par moi" seulement.
pub fn work_order_visible(
    manual: bool,
    should_show: bool,
    in_range: bool,
    claimed_by_self: bool,
    unclaimed: bool,
) -> bool {
    if !should_show || !in_range {
        return false;
    }
    if manual {
        claimed_by_self || unclaimed
    } else {
        claimed_by_self
    }
}

/// La raison pour laquelle le bouton d'un ordre est eteint, **la derniere gagnant**
/// (`WorkOrderModuleWindow.java:197-213`).
///
/// Java teste les trois conditions dans cet ordre et **ecrase** `disabledMessage` a chaque
/// fois. Le joueur ne voit donc que la derniere vraie : un ordre sans ouvrier **et** deja
/// pris affiche "deja pris". L'ordre est la regle.
pub fn work_order_refusal(
    no_worker: bool,
    claimed: bool,
    cannot_build: bool,
) -> Option<Refusal> {
    if cannot_build {
        Some(Refusal::CannotBuildYet)
    } else if claimed {
        Some(Refusal::AlreadyClaimed)
    } else if no_worker {
        Some(Refusal::NoWorker)
    } else {
        None
    }
}

/// Le libelle du bouton d'un ordre (`WorkOrderModuleWindow.java:217-228`).
///
/// Le bouton dit "annuler" des que l'ordre est **pris par cette hutte**, meme en mode
/// automatique ; sinon il dit "choisir", en mode manuel seulement.
pub fn work_order_button(claimed_by_self: bool) -> &'static str {
    if claimed_by_self {
        CANCEL_KEY
    } else {
        SELECT_KEY
    }
}

/// Le cout d'un saut de porte, en pepites d'or
/// (`ConnectionModuleWindow.java:104-114`).
pub fn gate_cost(distance: i32, external_player: bool) -> i32 {
    if external_player {
        distance / GATE_COST_PER_BLOCKS
    } else {
        0
    }
}

/// Ce qu'un clic sur "voyager" demande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les trois etats du bouton de porte
pub enum Gate {
    /// Le bouton fait le saut.
    Travel,
    /// Le bouton est eteint et porte l'infobulle du cout
    /// (`ConnectionModuleWindow.java:167-172`).
    Disabled { missing: bool },
}

/// L'etat du bouton de porte (`ConnectionModuleWindow.java:163-178`).
///
/// Deux branches, et la seconde decide de **tout le reste** : si le joueur est externe et
/// n'a pas de quoi payer, le bouton est eteint avec l'infobulle du cout ; **sinon** il est
/// allume si et seulement si la colonie est alliee et si sa position n'est pas nulle. Un
/// joueur interne et un joueur externe solvable suivent donc exactement la meme regle
/// d'alliance — ce que le nom `externalPlayer` ne laisse pas deviner.
///
/// **Ce qui est casse n'est pas l'alliance, c'est le silence** : un joueur externe qui a de
/// quoi payer mais dont la colonie n'est pas alliee recoit un bouton eteint **sans
/// infobulle** (`missing: false`), et l'icone du cout lui montre quand meme un prix
/// (`:157-176`).
pub fn gate_button(external: bool, nuggets: i32, cost: i32, allies: bool, at_origin: bool) -> Gate {
    if external && nuggets <= cost {
        Gate::Disabled { missing: true }
    } else if allies && !at_origin {
        Gate::Travel
    } else {
        Gate::Disabled { missing: false }
    }
}

/// La consommation d'un plat par citoyen et par cycle
/// (`RestaurantMenuModuleWindow.java:319`).
///
/// `None` quand la somme de saturation est nulle : Java divise en `double` et rend
/// `Infinity` ou `NaN`, ce que le port refuse de propager dans un libelle.
pub fn restaurant_consumption(
    average_customer: f64,
    customers: i32,
    saturation_sum: i32,
) -> Option<f64> {
    if saturation_sum == 0 {
        return None;
    }
    let value = (average_customer * 10.0 * f64::from(customers)) / f64::from(saturation_sum);
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// L'etat du bouton d'ajout d'un plat, **avec l'ecrasement de Java**
/// (`RestaurantMenuModuleWindow.java:555-561`, `:564-571`).
///
/// La limite eteint le bouton et pose son infobulle ; le test "deja au menu" **reallume**
/// ensuite le bouton des que le plat n'y est pas. Le couple rendu est donc
/// `(allume, infobulle)`, et l'allumage peut contredire l'infobulle.
pub fn restaurant_switch(at_limit: bool, already_listed: bool) -> (bool, Option<Refusal>) {
    let tooltip = if at_limit { Some(Refusal::AtLimit) } else { None };
    (!already_listed, tooltip)
}

/// L'univers d'un plat depend du niveau du batiment **moins un**
/// (`RestaurantMenuModuleWindow.java:116` : `getEdibles(buildingLevel - 1)`).
///
/// A niveau zero, l'indice passe a `-1`. Le port le rend comme une valeur signee pour que
/// l'appelant voie le cas au lieu de le voir se faire refuser silencieusement.
pub fn edibles_level(building_level: i32) -> i32 {
    building_level - 1
}

/// Les lignes de la liste des citoyens d'une hutte a plusieurs metiers
/// (`SpecialAssignmentModuleWindow.java:88-98`).
///
/// Une ligne par citoyen **de chaque module d'assignation**, dans l'ordre des modules puis
/// des citoyens, avec le metier du module devant le nom (`:115-117`).
pub fn assignment_rows(modules: &[(&str, &[i32])]) -> Vec<(String, i32)> {
    let mut rows = Vec::new();
    for (job, citizens) in modules {
        for citizen in *citizens {
            rows.push(((*job).to_owned(), *citizen));
        }
    }
    rows
}

/// Le texte d'une ligne d'assignation : `<metier>: <nom>`
/// (`SpecialAssignmentModuleWindow.java:115-117`).
pub fn assignment_text(job: &str, name: &str) -> String {
    format!("{job}: {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    /// Les neuf fenetres que d'autres modules portent deja.
    const ELSEWHERE: [&str; 9] = [
        "WindowBuilderResModule",
        "WindowHutMinerModule",
        "WindowMineGuardModule",
        "WindowHutRequestTaskModule",
        "WindowListRecipes",
        "WindowSelectRequest",
        "WindowStatsModule",
        "MinimumStockModuleWindow",
        "RequestTreeWindowModule",
    ];

    #[test]
    fn fifteen_windows_and_not_one_of_them_is_already_described_elsewhere() {
        assert_eq!(WINDOWS.len(), 15);
        for entry in WINDOWS {
            assert!(!ELSEWHERE.contains(&entry.java), "{} est deja porte", entry.java);
            assert!(entry.java.ends_with("Window"), "{}", entry.java);
            assert!(entry.view.ends_with("View") || entry.view.starts_with('I'));
            // Une ou deux ressources, jamais zero : les deux fenetres parametrees en ont.
            assert!(!entry.resources.is_empty(), "{} n'a pas de XML", entry.java);
            for resource in entry.resources {
                assert!(resource.starts_with("layout"), "{resource}");
                assert!(resource.ends_with(".xml"));
            }
        }
        // 24 fichiers dans modules/building, 9 ailleurs, 15 ici.
        assert_eq!(WINDOWS.len() + ELSEWHERE.len(), 24);
    }

    #[test]
    fn every_window_is_found_by_its_class_and_none_is_twice() {
        for (index, entry) in WINDOWS.iter().enumerate() {
            assert_eq!(window(entry.java).map(|w| w.java), Some(entry.java));
            for other in &WINDOWS[index + 1..] {
                assert_ne!(entry.java, other.java);
                for resource in entry.resources {
                    assert!(
                        !other.resources.contains(resource),
                        "deux fenetres, un XML : {resource}"
                    );
                }
            }
        }
        assert!(window("WindowInfo").is_none(), "WindowInfo n'est pas une fenetre de module");
        assert!(window("Nope").is_none());
        assert_eq!(
            Refusal::UnknownWindow { java: "Nope" }.name(),
            "module.refusal.unknown_window"
        );
    }

    #[test]
    fn the_two_windows_that_take_their_xml_as_an_argument_are_named_with_it() {
        // `ItemListModuleView.java:144` : une seule liste filtrable, donc un seul XML.
        let item = window("ItemListModuleWindow").unwrap_or_else(|| panic!("no item list"));
        assert_eq!(item.resources.len(), 1);
        assert_eq!(item.resources[0], ITEM_LIST_RESOURCE);
        assert_eq!(ITEM_LIST_RESOURCE, "layoutfilterablelist.xml");
        // `SpecialAssignmentModuleWindow` en a **deux**, un par vue qui l'ouvre.
        let assignment =
            window("SpecialAssignmentModuleWindow").unwrap_or_else(|| panic!("no assignment"));
        assert_eq!(assignment.resources, &ASSIGNMENT_RESOURCES[..]);
        assert_eq!(ASSIGNMENT_RESOURCES.len(), 2);
        assert_ne!(ASSIGNMENT_RESOURCES[0], ASSIGNMENT_RESOURCES[1]);
        // Et les **quatorze** autres en ont exactement une : quinze fenetres, une seule
        // (`SpecialAssignmentModuleWindow`) en porte deux.
        let single = WINDOWS
            .iter()
            .filter(|entry| entry.resources.len() == 1)
            .count();
        assert_eq!(single, 14);
        assert_eq!(single + 1, WINDOWS.len());
        // La fenetre d'artisanat d'ordre lit sa propre vue, pas celle du crafter.
        let doc = window("DOCraftingWindow").unwrap_or_else(|| panic!("no docrafting"));
        assert_eq!(doc.view, "DOCraftingModuleView");
    }

    #[test]
    fn the_seven_rules_are_all_used_and_four_windows_are_filtered_lists() {
        let filtered = WINDOWS.iter().filter(|w| w.rule == Rule::FilteredList).count();
        assert_eq!(filtered, 4, "item, entite, restaurant et artisanat de l'ordre");
        assert_eq!(WINDOWS.iter().filter(|w| w.rule == Rule::PlacedThings).count(), 3);
        assert_eq!(WINDOWS.iter().filter(|w| w.rule == Rule::Citizens).count(), 4);
        for rule in [
            Rule::SettingRows,
            Rule::WarehouseStorage,
            Rule::ResearchBranches,
            Rule::WorkOrders,
        ] {
            assert_eq!(WINDOWS.iter().filter(|w| w.rule == rule).count(), 1, "{rule:?}");
        }
    }

    #[test]
    fn a_tool_description_is_a_key_built_from_the_item_path() {
        assert_eq!(
            tool_desc_key("sceptergold").as_deref(),
            Some("com.minecolonies.coremod.gui.tooldesc.sceptergold")
        );
        // Un item sans chemin n'a pas de cle du tout : Java concatenerait du vide.
        assert_eq!(tool_desc_key(""), None);
        assert!(TOOL_DESC_PREFIX.ends_with('.'));
    }

    #[test]
    fn a_setting_row_is_rebuilt_only_when_its_id_disagrees_or_it_is_empty() {
        // La ligne porte deja ce reglage et elle a des enfants : rien a faire.
        assert_eq!(setting_row(true, true, false), Row::Reuse);
        // Elle porte un autre reglage : on la vide.
        assert_eq!(setting_row(true, false, false), Row::Relayout);
        // Elle est vide : on la remplit.
        assert_eq!(setting_row(true, true, true), Row::Relayout);
        // Sans panneau `id`, Java reconstruit toujours (`:96-101`).
        assert_eq!(setting_row(false, true, false), Row::NoIdentityPane);
        assert_eq!(setting_row(false, false, true), Row::NoIdentityPane);
        // Le panneau `id` et le panneau `desc` sont deux noms differents.
        assert_ne!(SETTING_ID_PANE, SETTING_DESC_PANE);
        assert_eq!(SETTING_ID_PANE, "id");
        assert_eq!(SETTINGS_LIST, "settingslist");
    }

    #[test]
    fn a_setting_description_is_a_key_built_from_its_unique_id() {
        assert_eq!(
            setting_desc_key("builder:mode").as_deref(),
            Some("com.minecolonies.coremod.setting.builder:mode")
        );
        assert_eq!(setting_desc_key(""), None);
        assert!(SETTING_PREFIX.ends_with('.'));
    }

    #[test]
    fn a_branch_is_visible_when_it_is_not_hidden_or_when_one_primary_is() {
        let hidden_primary = Primary { hidden: true, requirements: &[false] };
        let visible_primary = Primary { hidden: false, requirements: &[] };
        // Branche cachee, une primaire cachee et non satisfaite : elle disparait.
        assert!(!university_branch_visible(true, &[hidden_primary]));
        // Branche cachee, une primaire visible : elle reste.
        assert!(university_branch_visible(true, &[visible_primary]));
        // Branche cachee mais ses prerequis sont tous satisfaits : elle reste aussi.
        let satisfied = Primary { hidden: true, requirements: &[true, true] };
        assert!(university_branch_visible(true, &[satisfied]));
        // Une branche non cachee est toujours la.
        assert!(university_branch_visible(false, &[]));
        // Aucune primaire du tout : la liste est vide, donc la branche passe (Java rend
        // une liste vide pour `any` sur un ensemble vide).
        assert!(university_branch_visible(true, &[]));
    }

    #[test]
    fn the_locked_lines_put_one_locked_then_or_between_primaries() {
        let a = Primary { hidden: true, requirements: &[false, true] };
        let b = Primary { hidden: true, requirements: &[false] };
        let lines = university_lines(&[a, b]);
        assert_eq!(
            lines,
            [
                Line::Locked,
                Line::Requirement { fulfilled: false },
                Line::Requirement { fulfilled: true },
                Line::Or,
                Line::Requirement { fulfilled: false },
            ]
        );
        // Une branche sans primaire n'a aucune ligne.
        assert!(university_lines(&[]).is_empty());
        // Le "verrouille" n'apparait qu'une fois, meme avec deux primaires.
        assert_eq!(
            university_lines(&[a, b]).iter().filter(|line| **line == Line::Locked).count(),
            1
        );
    }

    #[test]
    fn the_warehouse_forces_not_needed_in_three_cases() {
        // Assez en stock, tout est au maximum : le statut reste.
        assert_eq!(
            warehouse_availability(Availability::HaveEnough, 1, 5, 5, false),
            Availability::HaveEnough
        );
        // L'amelioration est au maximum.
        assert_eq!(
            warehouse_availability(Availability::NeedMore, MAX_STORAGE_UPGRADE, 5, 5, false),
            Availability::NotNeeded
        );
        // Le batiment n'est pas au niveau maximal.
        assert_eq!(
            warehouse_availability(Availability::NeedMore, 1, 4, 5, false),
            Availability::NotNeeded
        );
        // La fenetre vient d'envoyer un transfert.
        assert_eq!(
            warehouse_availability(Availability::NeedMore, 1, 5, 5, true),
            Availability::NotNeeded
        );
        // Et le bouton suit le statut, pas l'inverse.
        assert!(warehouse_button(Availability::NeedMore));
        assert!(warehouse_button(Availability::HaveEnough));
        assert!(!warehouse_button(Availability::DoNotHave));
        assert!(!warehouse_button(Availability::NotNeeded));
    }

    #[test]
    fn the_warehouse_line_shows_a_missing_count_only_when_it_is_negative() {
        // `getMissingFromPlayer()` negatif = le joueur en a plus qu'il n'en faut.
        assert_eq!(missing_from_player_text(-5), Some("-5".to_owned()));
        assert_eq!(missing_from_player_text(0), None);
        assert_eq!(missing_from_player_text(7), None);
        assert_eq!(quantity_missing(64, 40), 24);
        assert_eq!(quantity_missing(64, 80), -16, "un surplus est un negatif, il s'affiche");
    }

    #[test]
    fn the_sort_button_needs_the_third_level_and_nothing_else() {
        assert_eq!(SORTING_LEVEL, 3);
        assert!(!sorting_available(0));
        assert!(!sorting_available(2));
        assert!(sorting_available(3));
        assert!(sorting_available(5));
        // La progression de l'amelioration est "x sur z" avec trois paliers.
        let progress = Progress { step: 1, steps: MAX_STORAGE_UPGRADE };
        assert_eq!(progress.x_of_z(), "1 of 3");
        assert_eq!(progress.percent(), 33);
        assert!(!progress.done());
        assert!(Progress { step: 3, steps: 3 }.done());
        assert!(Progress { step: 0, steps: 0 }.done(), "rien a faire est fini");
        assert_eq!(X_OF_Z_KEY, "com.minecolonies.coremod.gui.xofz");
    }

    #[test]
    fn a_progress_uses_the_same_percentage_as_the_builder_tab() {
        for (step, steps) in [(0, 0), (1, 3), (2, 3), (3, 3), (5, 0)] {
            assert_eq!(
                Progress { step, steps }.percent(),
                crate::ui::field::percent(step, steps)
            );
        }
    }

    #[test]
    fn an_item_filter_tests_the_description_id_and_the_display_name_only() {
        assert!(item_list_matches("", "minecraft:oak_log", "Oak Log"));
        assert!(item_list_matches("log", "minecraft:oak_log", "Oak Log"));
        assert!(item_list_matches("OAK", "minecraft:oak_log", "Oak Log"), "sans casse");
        assert!(item_list_matches("oak", "minecraft:oak_log", "Oak Log"));
        assert!(!item_list_matches("birch", "minecraft:oak_log", "Oak Log"));
        // Le nom affiche suffit a lui seul.
        assert!(item_list_matches("bûche", "minecraft:oak_log", "Bûche de chêne"));
    }

    #[test]
    fn an_entity_filter_tests_the_display_name_and_the_registry_id() {
        assert!(entity_list_matches("", "Zombie", "minecraft:zombie"));
        assert!(entity_list_matches("zombie", "Zombie", "minecraft:zombie"));
        assert!(entity_list_matches("minecraft:zomb", "Zombie", "minecraft:zombie"));
        assert!(!entity_list_matches("skeleton", "Zombie", "minecraft:zombie"));
        assert_eq!(FILTER_DEBOUNCE, 10);
    }

    #[test]
    fn allowed_rows_come_first_and_inversion_flips_them() {
        use Ordering::{Equal, Greater, Less};
        assert_eq!(allowed_first(true, false, false), Less);
        assert_eq!(allowed_first(false, true, false), Greater);
        assert_eq!(allowed_first(true, false, true), Greater);
        assert_eq!(allowed_first(false, true, true), Less);
        // Deux lignes du meme camp ne sont pas departagees : le tri reste stable.
        assert_eq!(allowed_first(true, true, false), Equal);
        assert_eq!(allowed_first(false, false, false), Equal);
    }

    #[test]
    fn the_switch_label_and_the_switch_action_agree_on_the_four_cases() {
        // Le libelle est ON exactement quand l'element est dans son etat actif.
        assert_eq!(switch_label(true, false), ON_KEY);
        assert_eq!(switch_label(false, false), OFF_KEY);
        assert_eq!(switch_label(false, true), ON_KEY, "liste inversee : autorise = OFF");
        assert_eq!(switch_label(true, true), OFF_KEY);
        // Et le clic lit ce libelle : dans une liste normale, ON veut dire "deja la",
        // donc cliquer retire ; dans une liste inversee, ON veut dire "pas encore la",
        // donc cliquer ajoute. L'invariant n'est donc pas "ON retire" mais
        // `add = (on && inverted) || (!on && !inverted)`.
        assert_eq!(switch_action(true, false), Switch::Remove);
        assert_eq!(switch_action(false, false), Switch::Add);
        assert_eq!(switch_action(true, true), Switch::Add);
        assert_eq!(switch_action(false, true), Switch::Remove);
        // Les quatre combinaisons sont couvertes et deux donnent chacune des deux
        // reponses : aucune n'est morte.
        let adds = [(true, false), (false, false), (true, true), (false, true)]
            .iter()
            .filter(|pair| switch_action(pair.0, pair.1) == Switch::Add)
            .count();
        assert_eq!(adds, 2);
    }

    #[test]
    fn a_vertical_field_has_no_distance_and_the_other_ones_do() {
        assert_eq!(field_line(true), FieldLine::Vertical);
        assert_eq!(field_line(false), FieldLine::Distance);
        // La distance affichee est celle au carre, suffixee "m" : Java ne prend pas la
        // racine (FarmFieldsModuleWindow.java:184).
        assert_eq!(field_distance_argument(900), "900m");
        assert_eq!(field_distance_argument(0), "0m");
        assert_eq!(field_count_arguments(2, 4), (2, 4));
        assert!(field_assign_enabled(true));
        assert!(!field_assign_enabled(false), "le bouton est eteint en mode auto");
        assert_eq!(FIELD_STATUS_KEY, "com.minecolonies.coremod.gui.field.status");
        assert_ne!(FIELD_COUNT_KEY, FIELD_DISTANCE_KEY);
    }

    #[test]
    fn a_work_order_is_grouped_current_then_mine_then_the_rest() {
        assert_eq!(work_order_group(true, true), WorkOrderGroup::Current);
        assert_eq!(work_order_group(true, false), WorkOrderGroup::Current);
        assert_eq!(work_order_group(false, true), WorkOrderGroup::Mine);
        assert_eq!(work_order_group(false, false), WorkOrderGroup::Other);
        // L'ordre de l'enumeration est l'ordre de tri.
        assert!(WorkOrderGroup::Current < WorkOrderGroup::Mine);
        assert!(WorkOrderGroup::Mine < WorkOrderGroup::Other);
    }

    #[test]
    fn the_manual_mode_widens_the_list_to_unclaimed_orders_and_the_auto_mode_does_not() {
        // Hors champ ou hors portee : jamais la.
        assert!(!work_order_visible(true, false, true, true, true));
        assert!(!work_order_visible(true, true, false, true, true));
        // Manuel : les ordres libres sont proposes.
        assert!(work_order_visible(true, true, true, false, true));
        // Automatique : seuls ceux de cette hutte.
        assert!(!work_order_visible(false, true, true, false, true));
        assert!(work_order_visible(false, true, true, true, false));
        assert_eq!(WORK_ORDER_REFRESH, 20);
    }

    #[test]
    fn the_last_failing_reason_is_the_one_the_player_reads() {
        // Les trois a la fois : "ne peut pas construire" gagne.
        assert_eq!(
            work_order_refusal(true, true, true),
            Some(Refusal::CannotBuildYet)
        );
        // Deux : "deja pris" gagne sur "pas d'ouvrier".
        assert_eq!(work_order_refusal(true, true, false), Some(Refusal::AlreadyClaimed));
        assert_eq!(work_order_refusal(true, false, false), Some(Refusal::NoWorker));
        assert_eq!(work_order_refusal(false, false, false), None);
        // Et la cle suit, pour les trois raisons.
        assert_eq!(
            work_order_refusal(false, false, true).and_then(|r| r.key()),
            Some(CANNOT_BUILD_KEY)
        );
        assert_eq!(work_order_refusal(true, false, false).and_then(|r| r.key()), Some(NO_WORKER_KEY));
        assert_eq!(
            work_order_refusal(false, true, false).and_then(|r| r.key()),
            Some(ALREADY_CLAIMED_KEY)
        );
    }

    #[test]
    fn the_work_order_button_says_cancel_as_soon_as_this_hut_holds_it() {
        assert_eq!(work_order_button(true), CANCEL_KEY);
        assert_eq!(work_order_button(false), SELECT_KEY);
        assert_ne!(CANCEL_KEY, SELECT_KEY);
    }

    #[test]
    fn a_gate_costs_one_nugget_per_hundred_and_twenty_five_blocks() {
        assert_eq!(gate_cost(0, true), 0);
        assert_eq!(gate_cost(124, true), 0, "division entiere");
        assert_eq!(gate_cost(125, true), 1);
        assert_eq!(gate_cost(1000, true), 8);
        // Le proprietaire ne paie pas.
        assert_eq!(gate_cost(1000, false), 0);
        assert_eq!(GATE_COST_PER_BLOCKS, 125);
    }

    #[test]
    fn the_gate_button_needs_an_alliance_whether_the_player_is_external_or_not() {
        // Interne, colonie alliee et placee : on voyage.
        assert_eq!(gate_button(false, 0, 0, true, false), Gate::Travel);
        // Interne, non alliee : eteint, sans infobulle de cout.
        assert_eq!(gate_button(false, 0, 0, false, false), Gate::Disabled { missing: false });
        // Externe sans assez de pepites : eteint, avec l'infobulle du cout.
        assert_eq!(gate_button(true, 2, 8, true, false), Gate::Disabled { missing: true });
        // Exactement le cout : eteint aussi (Java compare `<=`).
        assert_eq!(gate_button(true, 8, 8, true, false), Gate::Disabled { missing: true });
        // Externe solvable et colonie alliee : on voyage.
        assert_eq!(gate_button(true, 9, 8, true, false), Gate::Travel);
        // Externe solvable mais colonie NON alliee : eteint, et sans infobulle — le
        // `else` de :175 applique le test d'alliance aux deux joueurs.
        assert_eq!(gate_button(true, 9, 8, false, false), Gate::Disabled { missing: false });
        // Une colonie a la position nulle n'est jamais une destination, payeur ou non.
        assert_eq!(gate_button(true, 99, 8, true, true), Gate::Disabled { missing: false });
        assert_eq!(gate_button(false, 0, 0, true, true), Gate::Disabled { missing: false });
    }

    #[test]
    fn the_restaurant_refuses_to_divide_by_an_empty_menu() {
        assert_eq!(restaurant_consumption(3.0, 4, 0), None);
        assert_eq!(restaurant_consumption(0.0, 0, 0), None);
        let some = restaurant_consumption(1.0, 5, 50);
        assert!(some.is_some_and(|value| (value - 1.0).abs() < 1e-12));
        // Un menu qui n'est plus vide donne un nombre, lui.
        assert!(restaurant_consumption(2.0, 3, 60).is_some());
        // L'univers depend du niveau moins un, et le niveau zero est un cas.
        assert_eq!(edibles_level(0), -1);
        assert_eq!(edibles_level(3), 2);
    }

    #[test]
    fn the_restaurant_switch_is_reenabled_after_the_limit_turned_it_off() {
        // A la limite, plat pas encore au menu : le bouton finit ALLUME, avec l'infobulle.
        assert_eq!(restaurant_switch(true, false), (true, Some(Refusal::AtLimit)));
        // A la limite, plat deja au menu : eteint.
        assert_eq!(restaurant_switch(true, true), (false, Some(Refusal::AtLimit)));
        // Pas a la limite : le bouton suit la seule liste.
        assert_eq!(restaurant_switch(false, false), (true, None));
        assert_eq!(restaurant_switch(false, true), (false, None));
        assert_eq!(Refusal::AtLimit.key(), Some(LIMIT_REACHED_KEY));
    }

    #[test]
    fn an_assignment_row_is_one_citizen_of_one_module_in_module_order() {
        let modules: &[(&str, &[i32])] = &[("knight", &[4, 7]), ("archer", &[9])];
        let rows = assignment_rows(modules);
        assert_eq!(
            rows,
            [
                ("knight".to_owned(), 4),
                ("knight".to_owned(), 7),
                ("archer".to_owned(), 9),
            ]
        );
        assert_eq!(assignment_text("knight", "Anna"), "knight: Anna");
        // Un module sans personne n'ajoute pas de ligne.
        assert_eq!(assignment_rows(&[("druid", &[] as &[i32])]).len(), 0);
        assert_eq!(EXPEDITION_REFRESH, 20);
    }

    #[test]
    fn every_refusal_has_a_name_and_only_four_carry_java_key() {
        let all = [
            Refusal::LevelTooLow { need: 3, got: 1 },
            Refusal::NoWorker,
            Refusal::AlreadyClaimed,
            Refusal::CannotBuildYet,
            Refusal::AtLimit,
            Refusal::AlreadyListed,
            Refusal::Missing,
            Refusal::UnknownWindow { java: "X" },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("module.refusal."), "{refusal}");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name());
            }
        }
        let keyed = all.iter().filter(|refusal| refusal.key().is_some()).count();
        assert_eq!(keyed, 4, "no worker, claimed, cannot build, at limit");
        assert_eq!(Refusal::LevelTooLow { need: 3, got: 1 }.key(), None);
        assert_eq!(Refusal::Missing.key(), None, "la ligne rouge n'a pas de cle");
        assert_eq!(
            Refusal::LevelTooLow { need: 3, got: 1 }.to_string(),
            "module.refusal.level"
        );
    }

    #[test]
    fn a_state_carries_a_level_a_progress_and_a_refusal_and_nothing_else() {
        let state = State {
            level: 2,
            progress: Some(Progress { step: 1, steps: 3 }),
            refusal: Some(Refusal::NoWorker),
        };
        assert_eq!(state.level, 2);
        assert_eq!(state.progress.map(Progress::percent), Some(33));
        assert_eq!(state.refusal.and_then(Refusal::key), Some(NO_WORKER_KEY));
        // Un module sans progression ni refus est un etat valide, pas un trou.
        let bare = State { level: 0, progress: None, refusal: None };
        assert_eq!(bare.progress, None);
        assert_eq!(bare.refusal, None);
    }
}
