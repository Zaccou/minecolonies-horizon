//! Les deux métiers de la pierre : ce qu'ils ajoutent à la table du crafter, et ce
//! qu'ils n'ajoutent pas.
//!
//! Référence Java : `crafting/EntityAIWorkStonemason.java` (28 lignes),
//! `crafting/EntityAIWorkStoneSmeltery.java` (27 lignes), et les quatre classes qui
//! portent réellement leur comportement — `AbstractEntityAICrafting.java` (`:160-170`),
//! `AbstractEntityAIRequestSmelter.java` (`:71-82` et `:426-500`),
//! `BuildingStonemason.java` (`:53-80`), `BuildingStoneSmeltery.java` (`:51-77`),
//! plus les deux `Job*` (`:37-42` de chacun).
//!
//! ## La mesure, d'abord
//!
//! | Classe | Lignes | `registerTargets` | `static final` | états nommés | méthodes surchargées |
//! | --- | ---: | ---: | ---: | ---: | --- |
//! | `EntityAIWorkStonemason` | 28 | **0** | **0** | **0** | 1 (`getExpectedBuildingClass`) |
//! | `EntityAIWorkStoneSmeltery` | 27 | **0** | **0** | **0** | 1 (`getExpectedBuildingClass`) |
//!
//! Les deux fichiers sont des alias : un constructeur qui relaie (`:17-20` / `:17-20`)
//! et une méthode qui nomme la hutte (`:23-27` / `:22-26`). **Il n'y a donc pas de
//! machine à états à porter dans ces deux fichiers** — et c'est le premier résultat de
//! ce module. Ce qui reste à porter, et qui est bien à eux :
//!
//! 1. **la porte de recette** — la seule chose qui les distingue d'un autre crafter :
//!    une recette n'entre que si sa sortie est dans un tag d'objets
//!    (`BuildingStonemason.java:74-79`, `BuildingStoneSmeltery.java:72-76`) ;
//! 2. **la route du four** — pour la pierre taillée seulement : une recette dont
//!    l'intermédiaire n'est pas un four passe par la table de l'atelier, tout le reste
//!    passe par les fours (`AbstractEntityAIRequestSmelter.java:426-500`) ;
//! 3. **les refus de four** — pas de four, fours non chargés, pas de carburant : trois
//!    plaintes ou attentes qui n'existent que dans cette classe (`:439-463`) ;
//! 4. **les sons du métier** — `JobStoneSmeltery.java:37-49` et
//!    `JobStonemason.java:37-41`.
//!
//! ## Ce que ce fichier ne fait pas, et pourquoi
//!
//! Il **ne réenregistre pas** les six cibles de `AbstractEntityAICrafting` (`:160-170`) :
//! `crate::ai_build::crafter` les porte, avec ses quatre sorties de `getRecipe`, ses
//! sept de `craft`, sa formule de durée et ses tests. [`bench_machine`] rend la sienne.
//! Il **n'ajoute pas non plus une quatrième copie** des sept cibles de
//! `AbstractEntityAIRequestSmelter` (`:71-82`) : trois modules voisins les portent
//! (`src/ai_work_01/furnace.rs`, `src/ai_work_02/smelter.rs`,
//! `src/ai_work_03/smelter.rs`), et quatre copies vaudraient moins qu'une. Ce qui est
//! porté ici est **l'ordre de décision** de `executeCraftingAction` (`:465-499`) sous
//! forme de données ([`ERRAND_ORDER`]) : six lignes de règle, pas trois cents lignes
//! de logique, et un test qui vérifie que l'ordre est celui du Java.
//!
//! ## Les durées
//!
//! | Nom | Ticks | Java |
//! | --- | ---: | --- |
//! | vérification du carburant | 200 | `AbstractEntityAIRequestSmelter.java:73` |
//! | accélération des fours | 20 | `:74` |
//! | ramassage des restes | 200 | `:75` |
//! | visite d'un four | 20 | `:78-81` |
//! | attente quand tous les fours brûlent | 20 | `:489` |
//! | frappe d'une recette | 10 | `AbstractEntityAICrafting.java:73` |
//!
//! La durée d'une recette dépend du niveau et n'est pas répétée ici :
//! `crate::ai_build::work_time::craft_hits` (`AbstractEntityAICrafting.java:734-738`)
//! la calcule — trente coups au niveau 0, un seul à partir du niveau 20.

use crate::ai::Machine;
use crate::ai_build::crafter::{self, CraftFacts, CraftState};
use crate::building::Kind;

use super::{STANDARD_DELAY, TICKS_SECOND, Trade};

/// `Constants.STACKSIZE` (`api/util/constant/Constants.java:23`) : la taille d'un
/// transfert de carburant ou d'un lot demandé (`AbstractEntityAIRequestSmelter.java:
/// 111`, `:185`, `:211`, `:395`).
pub const STACKSIZE: u32 = 64;

/// `AbstractEntityAIRequestSmelter.BASE_XP_GAIN` (`:56`) : l'expérience d'une
/// extraction réussie d'un four (`:622`).
pub const BASE_XP_GAIN: f64 = 5.0;

/// L'accélération d'un four par dix niveaux de la compétence secondaire :
/// `(level / 10) * 2` (`:122`).
pub const ACCELERATION_PER_TEN_LEVELS: u32 = 2;

/// Le rythme de `checkFurnaceFuel` (`:73`) : `TICKS_SECOND * 10`.
pub const FUEL_CHECK_TICKS: u32 = TICKS_SECOND * 10;

/// Le rythme de `accelerateFurnaces` (`:74`).
pub const ACCELERATE_TICKS: u32 = TICKS_SECOND;

/// Le rythme de `checkForLeftOvers` (`:75`) : `TICKS_SECOND * 10`.
pub const LEFTOVER_CHECK_TICKS: u32 = TICKS_SECOND * 10;

/// Le rythme des quatre cibles de four (`:78-81`).
pub const FURNACE_VISIT_TICKS: u32 = TICKS_SECOND;

/// `setDelay(TICKS_SECOND)` quand tous les fours utilisables brûlent déjà (`:489`).
pub const BURNING_WAIT_TICKS: u32 = TICKS_SECOND;

/// `TranslationConstants.BAKER_HAS_NO_FURNACES_MESSAGE`
/// (`api/util/constant/TranslationConstants.java:63`) : la plainte bloquante quand la
/// hutte n'a aucun four (`AbstractEntityAIRequestSmelter.java:439-447`).
pub const NO_FURNACES_MESSAGE: &str = "com.minecolonies.coremod.bakery.nofurnace";

/// `TranslationConstants.FURNACE_USER_NO_FUEL`
/// (`api/util/constant/TranslationConstants.java:278`) : la plainte quand la liste de
/// carburants autorisés est vide (`:455-463`) et, en `IMPORTANT` au lieu de
/// `BLOCKING`, quand la hutte n'en a plus (`:715-718`).
pub const NO_FUEL_MESSAGE: &str = "com.minecolonies.coremod.furnaceuser.nofuel";

/// Le tag d'objets du tailleur de pierre : `TagConstants.CRAFTING_STONEMASON`
/// (`api/util/constant/TagConstants.java:71`) vaut `ModJobs.STONEMASON_ID.getPath()`
/// (`ModJobs.java:39`), donc `minecolonies:stonemason`.
pub const STONEMASON_TAG: &str = "#minecolonies:stonemason";

/// Le tag d'objets de la pierre taillée : `TagConstants.CRAFTING_STONE_SMELTERY`
/// (`:72`) vaut le chemin du métier, donc `minecolonies:stonesmeltery`.
pub const STONE_SMELTERY_TAG: &str = "#minecolonies:stonesmeltery";

/// Ce qu'une des deux classes ajoute à sa mère, en nombres — la mesure du tableau du
/// module, pour qu'un test l'assert au lieu qu'un commentaire l'affirme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TradeDelta {
    /// Appels à `registerTargets` dans la classe.
    pub targets_added: usize,
    /// Constantes `static final` numériques déclarées.
    pub constants_declared: usize,
    /// États d'`AIWorkerState` nommés.
    pub states_named: usize,
    /// Méthodes surchargées.
    pub overrides: usize,
}

/// Le delta, mesuré sur le fichier : un constructeur qui relaie et une méthode.
pub fn delta(trade: Trade) -> Option<TradeDelta> {
    match trade {
        // `EntityAIWorkStonemason.java:17-27`.
        Trade::Stonemason => Some(TradeDelta {
            targets_added: 0,
            constants_declared: 0,
            states_named: 0,
            overrides: 1,
        }),
        // `EntityAIWorkStoneSmeltery.java:17-26`.
        Trade::StoneSmeltery => Some(TradeDelta {
            targets_added: 0,
            constants_declared: 0,
            states_named: 0,
            overrides: 1,
        }),
        // L'écolier n'est pas un alias : cinq cibles, quatre décisions, deux états.
        Trade::Pupil => None,
    }
}

/// Une classe de la chaîne d'héritage d'un métier, avec ce qu'elle enregistre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chain {
    /// Le nom de la classe.
    pub class: &'static str,
    /// Le nombre de cibles qu'elle ajoute à `registerTargets`.
    pub targets: u32,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

/// `AbstractEntityAIBasic` enregistre quatorze cibles : c'est le premier maillon de
/// toutes les chaînes, et `crate::ai::worker::register_base` les porte.
pub const BASE_CHAIN: Chain = Chain {
    class: "AbstractEntityAIBasic",
    targets: 14,
    java: "AbstractEntityAIBasic.java:204-259",
};

/// `AbstractEntityAIInteract` n'ajoute rien, et le dit dans un commentaire
/// (« no new targets for now », `:110-113`).
pub const INTERACT_CHAIN: Chain = Chain {
    class: "AbstractEntityAIInteract",
    targets: 0,
    java: "AbstractEntityAIInteract.java:110-113",
};

/// La chaîne du tailleur de pierre : elle s'arrête à `AbstractEntityAICrafting`.
pub const STONEMASON_CHAIN: [Chain; 3] = [
    BASE_CHAIN,
    INTERACT_CHAIN,
    Chain {
        class: "AbstractEntityAICrafting",
        targets: 6,
        java: "AbstractEntityAICrafting.java:160-170",
    },
];

/// La chaîne de la pierre taillée : un maillon de plus, celui des fours.
pub const STONESMELTERY_CHAIN: [Chain; 4] = [
    BASE_CHAIN,
    INTERACT_CHAIN,
    Chain {
        class: "AbstractEntityAICrafting",
        targets: 6,
        java: "AbstractEntityAICrafting.java:160-170",
    },
    Chain {
        class: "AbstractEntityAIRequestSmelter",
        targets: 7,
        java: "AbstractEntityAIRequestSmelter.java:71-82",
    },
];

/// La chaîne du métier, de la mère à la fille.
pub fn chain(trade: Trade) -> Option<&'static [Chain]> {
    match trade {
        Trade::Stonemason => Some(&STONEMASON_CHAIN),
        Trade::StoneSmeltery => Some(&STONESMELTERY_CHAIN),
        Trade::Pupil => None,
    }
}

/// Les six règles de `AbstractEntityAICrafting.java:160-170`, dans l'ordre où la
/// machine les vérifie. `crate::ai_build::crafter` est la seule implémentation ; cette
/// liste existe pour qu'un test puisse prouver qu'il n'y en a pas de seconde.
pub const CRAFTING_TARGETS: [&str; 6] =
    ["hasWorkToDo", "idle", "decide", "queryItems", "getRecipe", "craft"];

/// La porte d'entrée d'une recette dans une hutte de ce module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : deux modules de fabrication dans les deux huttes de ce fichier
pub enum RecipeGate {
    /// `BuildingStonemason.CraftingModule` (`:64-80`) : un atelier, pas de four.
    Bench,
    /// `BuildingStoneSmeltery.SmeltingModule` (`:61-77`) : atelier **et** fours.
    Smelting,
}

impl RecipeGate {
    /// La porte du métier.
    pub fn of(trade: Trade) -> Option<RecipeGate> {
        match trade {
            Trade::Stonemason => Some(RecipeGate::Bench),
            Trade::StoneSmeltery => Some(RecipeGate::Smelting),
            Trade::Pupil => None,
        }
    }

    /// Le tag d'objets qui filtre les recettes de la hutte.
    pub fn tag(self) -> &'static str {
        match self {
            RecipeGate::Bench => STONEMASON_TAG,
            RecipeGate::Smelting => STONE_SMELTERY_TAG,
        }
    }

    /// `fichier.java:ligne` de `isRecipeCompatible`.
    pub fn java(self) -> &'static str {
        match self {
            RecipeGate::Bench => "BuildingStonemason.java:74-79",
            RecipeGate::Smelting => "BuildingStoneSmeltery.java:72-76",
        }
    }

    /// La hutte où cette porte se trouve.
    pub fn building(self) -> Kind {
        match self {
            RecipeGate::Bench => Kind::StoneMason,
            RecipeGate::Smelting => Kind::StoneSmeltery,
        }
    }
}

/// Ce que le port sait d'une recette candidate, au moment où `getRecipe` la cherche.
///
/// Java lit tout cela dans `IRecipeStorage` et dans la hutte ; le port le reçoit en
/// faits pour que la décision se rejoue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FurnaceRecipe {
    /// `building.getCraftingModuleForRecipe(recipeID) != null`
    /// (`AbstractEntityAICrafting.java:333-339`).
    pub module_found: bool,
    /// `isRecipeCompatibleBasedOnTags(recipe, tag).orElse(false)` — la sortie est
    /// dans le tag du métier (`BuildingStonemason.java:77`). C'est le filtre le plus
    /// strict des douze crafters : pas de repli.
    pub output_in_job_tag: bool,
    /// `getIngredientValidator()` — les entrées du tag sont acceptées
    /// (`BuildingStonemason.java:69`, `BuildingStoneSmeltery.java:67`).
    pub input_in_job_tag: bool,
    /// `currentRecipeStorage.getIntermediate() == Blocks.FURNACE`
    /// (`AbstractEntityAIRequestSmelter.java:434`).
    pub intermediate_is_furnace: bool,
}

/// Où `getRecipe` envoie la recette, quand elle passe la porte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : les deux boucles de `AbstractEntityAICrafting` et du `RequestSmelter`
pub enum RecipePath {
    /// La boucle d'atelier : `QUERY_ITEMS` puis `CRAFT`
    /// (`AbstractEntityAICrafting.java:316`, `:503`).
    Bench,
    /// La boucle des fours : `FILL_UP_FURNACES` puis `RETRIEVING_…`
    /// (`AbstractEntityAIRequestSmelter.java:434-437` puis `:465-497`).
    Furnace,
}

/// Pourquoi une recette n'entre pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : une variante par `finishRequest(false)` de `getRecipe`
pub enum GateRefusal {
    /// `getCraftingModuleForRecipe` rend `null` : la recette n'est pas de cette hutte
    /// (`AbstractEntityAICrafting.java:333-339`). Le métier abandonne la tâche
    /// (`job.finishRequest(false)`) et retourne en `START_WORKING`.
    NoModule,
    /// Aucune recette ne correspond, donc `getFirstFulfillableRecipe` rend `null`
    /// (`:340-346`) : soit la sortie n'est pas dans le tag du métier, soit aucune
    /// entrée n'est disponible.
    NoCompatibleRecipe,
}

impl GateRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            GateRefusal::NoModule => "AbstractEntityAICrafting.java:333-339",
            GateRefusal::NoCompatibleRecipe => "AbstractEntityAICrafting.java:340-346",
        }
    }
}

/// `getRecipe`'s gate (`AbstractEntityAICrafting.java:324-346`), plus la bifurcation
/// four/atelier du `RequestSmelter` (`:434-437`).
///
/// L'ordre des tests est celui du Java : le module d'abord, le tag ensuite, et la
/// route du four **en dernier** — une recette hors tag n'atteint jamais le four.
pub fn check_gate(gate: RecipeGate, recipe: &FurnaceRecipe) -> Result<RecipePath, GateRefusal> {
    if !recipe.module_found {
        return Err(GateRefusal::NoModule);
    }
    if !recipe.output_in_job_tag {
        // `isRecipeCompatible` refuse : `.orElse(false)` n'a pas de repli.
        return Err(GateRefusal::NoCompatibleRecipe);
    }
    match gate {
        // Un atelier ne connaît pas de four : tout passe par la table
        // (`AbstractEntityAICrafting.java:503`).
        RecipeGate::Bench => Ok(RecipePath::Bench),
        // `:434-437` : seule une recette à intermédiaire four passe par les fours,
        // les autres retombent sur `super.executeCraftingAction`.
        RecipeGate::Smelting => {
            if recipe.intermediate_is_furnace {
                Ok(RecipePath::Furnace)
            } else {
                Ok(RecipePath::Bench)
            }
        }
    }
}

/// Pourquoi une liste de carburants ne peut pas être utilisée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : une seule issue, celle de `:713-720`
pub enum FuelRefusal {
    /// La hutte n'a **aucun** carburant autorisé : Java brandit
    /// `FURNACE_USER_NO_FUEL` en priorité `IMPORTANT` et rend une liste vide
    /// (`:713-720`). Ce n'est pas une erreur de recette : c'est un module de hutte que
    /// le joueur n'a pas rempli.
    NoFuelListed,
}

impl FuelRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            FuelRefusal::NoFuelListed => "AbstractEntityAIRequestSmelter.java:713-720",
        }
    }

    /// Le message au joueur.
    pub fn message(self) -> &'static str {
        match self {
            FuelRefusal::NoFuelListed => NO_FUEL_MESSAGE,
        }
    }
}

/// La liste de carburants que le métier utilise réellement.
///
/// Référence Java : `getActivePossibleFuels` (`AbstractEntityAIRequestSmelter.java:
/// 710-733`). La liste de la hutte (`ITEMLIST_FUEL`) est copiée, puis **deux entrées
/// en sont retirées** : la sortie principale de la recette en cours (`:724`) et sa
/// première entrée (`:726`) — sinon l'ouvrier mettrait dans le four ce qu'il est en
/// train de fabriquer. Une liste vide déclenche la plainte `FURNACE_USER_NO_FUEL`
/// (`:713-720`).
///
/// `input` est `None` quand aucune recette n'est en cours : Java ne retire alors rien
/// (`:722`).
pub fn active_fuels(
    allowed: &'static [&'static str],
    output: Option<&str>,
    input: Option<&str>,
) -> Result<Vec<&'static str>, FuelRefusal> {
    if allowed.is_empty() {
        // `:713-720` — la hutte n'a aucun carburant autorisé, et l'ouvrier le dit au
        // joueur en priorité IMPORTANT.
        return Err(FuelRefusal::NoFuelListed);
    }
    Ok(allowed
        .iter()
        .copied()
        .filter(|candidate| Some(*candidate) != output && Some(*candidate) != input)
        .collect())
}

/// Les cinq sorties de `executeCraftingAction` (`AbstractEntityAIRequestSmelter.java:
/// 426-500`), en plus du retour final.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : une variante par `return` de la méthode
pub enum Errand {
    /// `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:465-469`).
    RetrieveOutput,
    /// `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:471-475`).
    RetrieveUnrelated,
    /// `ADD_FUEL_TO_FURNACE` (`:477-484`).
    AddFuel,
    /// `setDelay(TICKS_SECOND)` puis on reste : tous les fours utilisables brûlent
    /// (`:486-491`).
    WaitForFurnaces,
    /// `FILL_UP_FURNACES` (`:493-497`).
    FillUp,
    /// `START_WORKING` (`:499`), le retour par défaut.
    StartWorking,
}

/// Une marche de l'ordre de décision : la course à faire, son état d'arrivée, le
/// sélecteur Java qui l'a choisie, et sa ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrandGuard {
    /// La course.
    pub errand: Errand,
    /// L'état où l'écolier… pardon, l'ouvrier arrive, tel que `AIWorkerState` le
    /// nomme ; `None` quand il reste où il est (`WaitForFurnaces`).
    pub target: Option<&'static str>,
    /// La méthode Java qui choisit la course.
    pub rule: &'static str,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

/// L'ordre exact de `executeCraftingAction`, tel qu'il est écrit (`:465-499`).
///
/// **L'ordre est la règle.** Les quatre premières courses sont des conditions
/// exclusives, dans cet ordre : sortir un produit fini passe avant de nettoyer une
/// entrée étrangère, qui passe avant de remplir un four en carburant, qui passe avant
/// d'attendre. Inverser deux lignes change le comportement d'un ouvrier qui a les
/// trois possibilités à la fois — et c'est précisément ce qu'un test doit tenir.
pub const ERRAND_ORDER: [ErrandGuard; 6] = [
    ErrandGuard {
        errand: Errand::RetrieveOutput,
        target: Some("RETRIEVING_END_PRODUCT_FROM_FURNACE"),
        rule: "getFurnaceToRetrieveOutputFrom",
        java: "AbstractEntityAIRequestSmelter.java:465-469",
    },
    ErrandGuard {
        errand: Errand::RetrieveUnrelated,
        target: Some("RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE"),
        rule: "getFurnaceToRetrieveUnrelatedInputFrom",
        java: "AbstractEntityAIRequestSmelter.java:471-475",
    },
    ErrandGuard {
        errand: Errand::AddFuel,
        target: Some("ADD_FUEL_TO_FURNACE"),
        rule: "getFurnaceWithoutFuel",
        java: "AbstractEntityAIRequestSmelter.java:477-484",
    },
    ErrandGuard {
        errand: Errand::WaitForFurnaces,
        target: None,
        rule: "countOfBurningFurnaces",
        java: "AbstractEntityAIRequestSmelter.java:486-491",
    },
    ErrandGuard {
        errand: Errand::FillUp,
        target: Some("FILL_UP_FURNACES"),
        rule: "getEmptyFurnaceWithFuel",
        java: "AbstractEntityAIRequestSmelter.java:493-497",
    },
    ErrandGuard {
        errand: Errand::StartWorking,
        target: Some("START_WORKING"),
        rule: "startWorking",
        java: "AbstractEntityAIRequestSmelter.java:499",
    },
];

/// Les trois gardes qui précèdent l'ordre des courses, et qui n'ont pas d'état : elles
/// renvoient l'ouvrier en `START_WORKING` ou le font attendre (`:429-463`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreGuard {
    /// Ce qui manque, en une phrase pour un journal.
    pub rule: &'static str,
    /// Le message au joueur, quand Java en brandit un.
    pub message: Option<&'static str>,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

/// Les trois refus de four, dans l'ordre où Java les teste.
pub const PRE_GUARDS: [PreGuard; 3] = [
    PreGuard {
        rule: "getFurnaces().isEmpty",
        message: Some(NO_FURNACES_MESSAGE),
        java: "AbstractEntityAIRequestSmelter.java:439-447",
    },
    PreGuard {
        rule: "areFurnacesLoaded",
        message: None,
        java: "AbstractEntityAIRequestSmelter.java:450-453",
    },
    PreGuard {
        rule: "getAllowedFuel().isEmpty",
        message: Some(NO_FUEL_MESSAGE),
        java: "AbstractEntityAIRequestSmelter.java:455-463",
    },
];

/// Un son que le métier demande au monde, avec ses quatre paramètres.
///
/// Référence Java : `EntityCitizen.queueSound` (`:1860-1869` pour la forme courte,
/// `:1872-1882` pour la forme à volume et hauteur) ; les deux `Job*` l'appellent à
/// chaque frappe de la table (`AbstractEntityAICrafting.java:662`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundRequest {
    /// L'identifiant de l'événement sonore, `minecraft:<path>`.
    pub sound: &'static str,
    /// `length` : combien de ticks le son dure côté client.
    pub length: u32,
    /// `repetitions`.
    pub repetitions: u32,
    /// `volume`, quand la forme longue est utilisée.
    pub volume: f32,
    /// `pitch`, idem.
    pub pitch: f32,
    /// `fichier.java:ligne`.
    pub java: &'static str,
}

/// Le son du tailleur de pierre : un seul, à chaque frappe
/// (`JobStonemason.java:37-41`).
pub const STONEMASON_HIT: SoundRequest = SoundRequest {
    sound: "minecraft:block.deepslate_tiles.hit",
    length: 5,
    repetitions: 1,
    volume: 1.0,
    pitch: 2.0,
    java: "JobStonemason.java:40",
};

/// Le son rare de la pierre taillée : une fois sur dix, une torche de redstone qui
/// grille (`JobStoneSmeltery.java:40-43`).
pub const SMELTERY_BURNOUT: SoundRequest = SoundRequest {
    sound: "minecraft:block.redstone_torch.burnout",
    length: 10,
    repetitions: 0,
    volume: 1.0,
    pitch: 1.0,
    java: "JobStoneSmeltery.java:42",
};

/// Le son courant de la pierre taillée : de la lave qui crépite
/// (`JobStoneSmeltery.java:44-48`).
pub const SMELTERY_LAVA_POP: SoundRequest = SoundRequest {
    sound: "minecraft:block.lava.pop",
    length: 5,
    repetitions: 0,
    volume: 1.0,
    pitch: 1.0,
    java: "JobStoneSmeltery.java:46",
};

/// Le second son de la pierre taillée (`JobStoneSmeltery.java:47`).
pub const SMELTERY_LAVA_AMBIENT: SoundRequest = SoundRequest {
    sound: "minecraft:block.lava.ambient",
    length: 5,
    repetitions: 0,
    volume: 1.0,
    pitch: 1.0,
    java: "JobStoneSmeltery.java:47",
};

/// `Job*.playSound` pour les deux métiers : ce que la frappe fait entendre.
///
/// `draw` est le `worker.getRandom().nextInt(10)` de `JobStoneSmeltery.java:40` ; il
/// est ramené dans `0..10`. Le tailleur de pierre n'a pas de tirage : il frappe
/// toujours le même bloc (`JobStonemason.java:40`).
pub fn job_sounds(trade: Trade, draw: u32) -> Vec<SoundRequest> {
    match trade {
        Trade::Stonemason => vec![STONEMASON_HIT],
        Trade::StoneSmeltery => {
            if draw % 10 < 1 {
                vec![SMELTERY_BURNOUT]
            } else {
                vec![SMELTERY_LAVA_POP, SMELTERY_LAVA_AMBIENT]
            }
        }
        // L'écolier ne touche pas d'outil : `EntityAIWorkPupil` n'appelle jamais
        // `job.playSound`, et `JobPupil` ne le surcharge pas.
        Trade::Pupil => Vec::new(),
    }
}

/// La machine d'atelier, telle que `crate::ai_build::crafter` la porte — celle que le
/// tailleur de pierre court **sans y rien changer**.
///
/// Rendue plutôt que réenregistrée : voir la note du module.
pub fn bench_machine() -> Machine<CraftState, CraftFacts> {
    crafter::ai()
}

/// La table complète du tailleur de pierre : les six cibles d'atelier
/// (`AbstractEntityAICrafting.java:160-170`), et rien de plus.
///
/// `None` pour la pierre taillée **et c'est le résultat, pas un oubli** : sa table est
/// celle du crafter *plus* les sept cibles de
/// `AbstractEntityAIRequestSmelter.java:71-82`, qui vivent sur d'autres états et
/// d'autres faits et ne peuvent pas entrer dans un `Machine<CraftState, CraftFacts>`.
/// Ce que ce module porte de cette moitié-là est [`PRE_GUARDS`] et [`ERRAND_ORDER`] ;
/// l'autre moitié est `bench_machine()`.
pub fn machine(trade: Trade) -> Option<Machine<CraftState, CraftFacts>> {
    match trade {
        Trade::Stonemason => Some(bench_machine()),
        Trade::StoneSmeltery | Trade::Pupil => None,
    }
}

/// Le délai après un échec de four : `setDelay(AbstractEntityAIBasic.STANDARD_DELAY)`
/// (`AbstractEntityAIRequestSmelter.java:446`).
pub const NO_FURNACE_DELAY: u32 = STANDARD_DELAY;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_stone_classes_are_aliases_and_this_is_measured() {
        // 28 et 27 lignes, dont un en-tête de licence : la mesure est le résultat.
        for trade in [Trade::Stonemason, Trade::StoneSmeltery] {
            let delta = delta(trade).expect("un métier de la pierre");
            assert_eq!(delta.targets_added, 0, "{trade}");
            assert_eq!(delta.constants_declared, 0, "{trade}");
            assert_eq!(delta.states_named, 0, "{trade}");
            assert_eq!(delta.overrides, 1, "{trade} ne surcharge que sa hutte");
        }
        // L'écolier, lui, n'est pas un alias.
        assert_eq!(delta(Trade::Pupil), None);
    }

    #[test]
    fn the_chains_add_up_to_the_targets_trade_reports() {
        // La somme des maillons doit égaler `Trade::targets`, sinon l'un des deux
        // ment.
        for trade in [Trade::Stonemason, Trade::StoneSmeltery] {
            let total: u32 = chain(trade).expect("une chaîne").iter().map(|link| link.targets).sum();
            assert_eq!(total, trade.targets(), "{trade}");
        }
        assert_eq!(Trade::Stonemason.targets(), 20, "14 + 6");
        assert_eq!(Trade::StoneSmeltery.targets(), 27, "14 + 6 + 7");
        assert_eq!(Trade::Pupil.targets(), 19, "14 + 0 + 5");
        // Et les lignes citées sont celles des quatre classes mères.
        assert_eq!(STONEMASON_CHAIN.len(), 3);
        assert_eq!(STONESMELTERY_CHAIN.len(), 4);
        assert_eq!(
            STONESMELTERY_CHAIN[3].java,
            "AbstractEntityAIRequestSmelter.java:71-82"
        );
    }

    #[test]
    fn only_the_stone_smeltery_knows_a_furnace() {
        assert_eq!(RecipeGate::of(Trade::Stonemason), Some(RecipeGate::Bench));
        assert_eq!(
            RecipeGate::of(Trade::StoneSmeltery),
            Some(RecipeGate::Smelting)
        );
        assert_eq!(RecipeGate::of(Trade::Pupil), None);
        assert_eq!(RecipeGate::Bench.tag(), "#minecolonies:stonemason");
        assert_eq!(RecipeGate::Smelting.tag(), "#minecolonies:stonesmeltery");
        assert_eq!(
            RecipeGate::Smelting.building(),
            Kind::StoneSmeltery
        );
    }

    #[test]
    fn a_recipe_outside_the_tag_never_reaches_the_furnaces() {
        // `BuildingStonemason.java:77` : `.orElse(false)`, aucun repli.
        let recipe = FurnaceRecipe {
            module_found: true,
            output_in_job_tag: false,
            input_in_job_tag: true,
            intermediate_is_furnace: true,
        };
        assert_eq!(
            check_gate(RecipeGate::Smelting, &recipe),
            Err(GateRefusal::NoCompatibleRecipe)
        );
        assert_eq!(
            check_gate(RecipeGate::Bench, &recipe),
            Err(GateRefusal::NoCompatibleRecipe)
        );
        // Le module passe avant le tag (`:333-339` est testé avant `:340`).
        let no_module = FurnaceRecipe {
            module_found: false,
            ..recipe
        };
        assert_eq!(
            check_gate(RecipeGate::Bench, &no_module),
            Err(GateRefusal::NoModule)
        );
    }

    #[test]
    fn a_furnace_recipe_takes_the_furnace_route_and_the_others_the_bench() {
        // `AbstractEntityAIRequestSmelter.java:434-437`.
        let smelting = FurnaceRecipe {
            module_found: true,
            output_in_job_tag: true,
            input_in_job_tag: true,
            intermediate_is_furnace: true,
        };
        assert_eq!(
            check_gate(RecipeGate::Smelting, &smelting),
            Ok(RecipePath::Furnace)
        );
        let bench_work = FurnaceRecipe {
            intermediate_is_furnace: false,
            ..smelting
        };
        assert_eq!(
            check_gate(RecipeGate::Smelting, &bench_work),
            Ok(RecipePath::Bench),
            "tout le reste retombe sur `super`"
        );
        // Le tailleur de pierre n'a pas de four du tout.
        assert_eq!(check_gate(RecipeGate::Bench, &smelting), Ok(RecipePath::Bench));
    }

    #[test]
    fn the_active_fuel_list_drops_the_recipe_s_own_items() {
        // `:722-727` : la sortie et la première entrée sortent de la liste.
        const ALLOWED: &[&str] = &["minecraft:coal", "minecraft:stone", "minecraft:oak_log"];
        assert_eq!(
            active_fuels(ALLOWED, Some("minecraft:stone"), Some("minecraft:oak_log")),
            Ok(vec!["minecraft:coal"])
        );
        // Sans recette en cours, rien n'est retiré (`:722`).
        assert_eq!(
            active_fuels(ALLOWED, None, None),
            Ok(vec!["minecraft:coal", "minecraft:stone", "minecraft:oak_log"])
        );
        // Une hutte sans carburant autorisé : la plainte IMPORTANT de `:713-720`.
        assert_eq!(active_fuels(&[], None, None), Err(FuelRefusal::NoFuelListed));
        assert_eq!(
            FuelRefusal::NoFuelListed.message(),
            "com.minecolonies.coremod.furnaceuser.nofuel"
        );
    }

    #[test]
    fn the_errand_order_is_java_s_order_and_it_matters() {
        let errands: Vec<Errand> = ERRAND_ORDER.iter().map(|guard| guard.errand).collect();
        assert_eq!(
            errands,
            vec![
                Errand::RetrieveOutput,
                Errand::RetrieveUnrelated,
                Errand::AddFuel,
                Errand::WaitForFurnaces,
                Errand::FillUp,
                Errand::StartWorking,
            ]
        );
        // Sortir un produit fini passe avant tout le reste (`:465`), et le retour par
        // défaut est le dernier (`:499`).
        assert_eq!(ERRAND_ORDER[0].target, Some("RETRIEVING_END_PRODUCT_FROM_FURNACE"));
        assert_eq!(ERRAND_ORDER[5].target, Some("START_WORKING"));
        // L'attente ne change pas d'état : elle pose un délai.
        assert_eq!(ERRAND_ORDER[3].target, None);
        assert_eq!(ERRAND_ORDER[3].errand, Errand::WaitForFurnaces);
    }

    #[test]
    fn the_three_furnace_guards_come_before_the_errands() {
        // `:439-463`, dans cet ordre : pas de four, fours non chargés, pas de
        // carburant.
        assert_eq!(PRE_GUARDS.len(), 3);
        assert_eq!(PRE_GUARDS[0].message, Some(NO_FURNACES_MESSAGE));
        assert_eq!(PRE_GUARDS[1].message, None, "aucune plainte, juste une attente");
        assert_eq!(PRE_GUARDS[2].message, Some(NO_FUEL_MESSAGE));
        assert_eq!(
            PRE_GUARDS[0].java,
            "AbstractEntityAIRequestSmelter.java:439-447"
        );
        // Le délai d'un échec de four est le `STANDARD_DELAY` de la base.
        assert_eq!(NO_FURNACE_DELAY, 5);
    }

    #[test]
    fn the_durations_are_the_six_rates_of_the_two_classes() {
        assert_eq!(FUEL_CHECK_TICKS, 200, "`:73`");
        assert_eq!(ACCELERATE_TICKS, 20, "`:74`");
        assert_eq!(LEFTOVER_CHECK_TICKS, 200, "`:75`");
        assert_eq!(FURNACE_VISIT_TICKS, 20, "`:78-81`");
        assert_eq!(BURNING_WAIT_TICKS, 20, "`:489`");
        assert_eq!(crate::ai_build::work_time::CRAFT_HIT_DELAY, 10, "`:73`");
        assert_eq!(STACKSIZE, 64);
        assert_eq!(BASE_XP_GAIN, 5.0);
        assert_eq!(ACCELERATION_PER_TEN_LEVELS, 2, "`:122`");
    }

    #[test]
    fn the_smeltery_growls_lava_nine_times_out_of_ten() {
        // `JobStoneSmeltery.java:40` : `nextInt(10) < 1`.
        assert_eq!(job_sounds(Trade::StoneSmeltery, 0), vec![SMELTERY_BURNOUT]);
        for draw in 1..10 {
            assert_eq!(
                job_sounds(Trade::StoneSmeltery, draw),
                vec![SMELTERY_LAVA_POP, SMELTERY_LAVA_AMBIENT],
                "{draw}"
            );
        }
        // Un tirage hors bornes revient dans `0..10`.
        assert_eq!(job_sounds(Trade::StoneSmeltery, 10), vec![SMELTERY_BURNOUT]);
        // Le tailleur de pierre frappe toujours le même bloc de deepslate.
        for draw in 0..10 {
            assert_eq!(job_sounds(Trade::Stonemason, draw), vec![STONEMASON_HIT]);
        }
        assert_eq!(STONEMASON_HIT.pitch, 2.0, "`:40`, hauteur 2.0");
        assert_eq!(STONEMASON_HIT.repetitions, 1);
    }

    #[test]
    fn the_mason_runs_the_crafter_s_table_unchanged() {
        // Le point du module : aucune seconde copie. La liste attendue est celle de
        // `AbstractEntityAICrafting.java:160-170` plus les deux sorties de base.
        let table = machine(Trade::Stonemason).expect("une table");
        let rules: Vec<&str> = table
            .transitions()
            .iter()
            .map(|transition| transition.rule)
            .collect();
        for target in CRAFTING_TARGETS {
            assert!(rules.contains(&target), "{target} manque");
        }
        assert_eq!(&rules[..6], &CRAFTING_TARGETS[..]);
        assert_eq!(table.state(), CraftState::Idle);
        // Et la pierre taillée n'a pas de table d'atelier seule : son delta est
        // l'ordre des courses, pas une machine de plus.
        assert!(machine(Trade::StoneSmeltery).is_none());
        assert!(machine(Trade::Pupil).is_none());
    }
}
