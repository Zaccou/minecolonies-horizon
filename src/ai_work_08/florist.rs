//! Le fleuriste : cueillir les fleurs poussées, composter les terres nues.
//!
//! Java référence : `core/entity/ai/workers/production/agriculture/
//! EntityAIWorkFlorist.java` — la table du constructeur (`:128-134`), `decide`
//! (`:151-207`), `compost` (`:214-249`), `harvest` (`:256-288`), `checkOrEquipShears`
//! (`:307-321`), `getActionsDoneUntilDumping` (`:323-327`), `getBlockMiningTime`
//! (`:336-340`), `areThereFlowersToGather` (`:347-358`) et `getFirstNotCompostedLand`
//! (`:365-386`), sur `BuildingFlorist` et `TileEntityCompostedDirt`.
//!
//! ## La table de ce métier
//!
//! | # | État | Règle | Ticks | Java |
//! | ---: | --- | --- | ---: | --- |
//! | 1 | `IDLE` | vers `START_WORKING` | 1 | `:129` |
//! | 2 | `START_WORKING` | vers `DECIDE` | 20 | `:130` |
//! | 3 | `DECIDE` | `decide` | 200 | `:131` |
//! | 4 | `FLORIST_HARVEST` | `harvest` | 20 | `:132` |
//! | 5 | `FLORIST_COMPOST` | `compost` | 20 | `:133` |
//! | 6 | *(événement)* | `inventoryNeedsDump` → `INVENTORY_FULL` | 100 | `AbstractEntityAIBasic.java:225` |
//! | 7 | `INVENTORY_FULL` | `dumpInventory` | 20 | `AbstractEntityAIBasic.java:226` |
//! | 8 | `GATHERING_REQUIRED_MATERIALS` | `getNeededItem` | 5 | `AbstractEntityAIBasic.java:242` |
//! | 9 | `PAUSED` | `bePaused` | 20 | `AbstractEntityAIBasic.java:251` |
//!
//! **Les deux états de transfert ne sont pas une invention du port.** Java les enregistre
//! dans la classe de base (`:225-226`, `:242`, `:251`) ; un `Machine` de ce port ne peut
//! pas hériter de cette table (le moteur est monomorphique en son état et ses faits, voir
//! `crate::ai_gather::mod` §« le seam »), donc le fleuriste enregistre les quatre lignes
//! qu'il franchit, avec leurs numéros.
//!
//! **Le cycle, vu par ce métier.** `decide` choisit entre trois choses : cueillir une
//! fleur (`:187-191`), aller chercher du compost (`:176-180`) ou poser du compost
//! (`:202-206`). `harvest` et `compost` marchent d'abord (`:223`, `:265`), agissent
//! ensuite, et rendent `START_WORKING` — c'est le « chercher / aller / agir » que
//! `crate::ai_work_08::cycle::phase_of` lit sur l'état et les réponses du tick.
//!
//! **Ce que le port ne fait pas.** Il ne casse pas de bloc, il ne fait pas pousser de
//! fleur, il ne pose pas de compost : [`decide`], [`harvest`] et [`compost`] rendent des
//! [`AgriWork`] que l'appelant exécute. Une décision qui n'agit pas est une décision
//! qu'on peut rejouer, et c'est ce que les tests font.

use crate::ai::state::{Action, Blocking, Fired, Machine, Transition, ENTITY_AI_TICKRATE};
use crate::ai_gather::{GatherState, TICKS_SECOND};

use super::cycle;
use super::facts::{FloristFacts, FloristScene, PlantGround};
use super::state::{AgriDecision, AgriRefusal, AgriState, AgriWork};
use crate::ai::facts::Pos;

/// `MAX_DISTANCE` (`EntityAIWorkFlorist.java:50`) : au-delà de cinquante blocs de sa
/// hutte, le fleuriste rentre avant de jardiner (`:166-170`). Java le déclare `long`,
/// `getDistance2D` rend un `long` ; le port garde le type et compare en `f64`.
pub const MAX_DISTANCE: i64 = 50;
/// `HARVEST_ACTIONS_TO_DUMP` (`:55`) : dix cueillettes **par niveau de hutte** avant le
/// vidage (`:323-327`).
pub const HARVEST_ACTIONS_TO_DUMP: u32 = 10;
/// `PERCENT_CHANGE_FOR_GROWTH` (`:60`) : la chance de croissance d'une terre compostée,
/// dont la hutte retire un centième par niveau (`:236`).
pub const PERCENT_CHANGE_FOR_GROWTH: f64 = 0.2;
/// `BASE_XP_GAIN` (`:65`) : **Java la déclare et ne la lit nulle part** — l'expérience
/// réellement donnée est `XP_PER_FLOWER` (`:277`). Le port garde la constante pour que
/// la ligne soit citée, et ne l'utilise pas : c'est ce que fait Java.
pub const BASE_XP_GAIN: f64 = 0.5;
/// `COMPOST_REQUEST_QTY` (`:70`) : la taille de la requête de compost (`:183`).
pub const COMPOST_REQUEST_QTY: u32 = 16;
/// `BASE_BLOCK_MINING_DELAY` (`:75`) : la base de `getBlockMiningTime` (`:336-340`).
pub const BASE_BLOCK_MINING_DELAY: u32 = 10;
/// `PER_LEVEL_BONUS` (`:80`) : ce que chaque demi-niveau de dextérité retire au minage
/// (`:339`).
pub const PER_LEVEL_BONUS: f64 = 0.1;
/// `MAX_BONUS` (`:85`) : le plafond du bonus de minage, cinq (`:339`).
pub const MAX_BONUS: f64 = 5.0;
/// `XP_PER_FLOWER` (`:96`) : deux points par fleur cueillie (`:277`).
pub const XP_PER_FLOWER: f64 = 2.0;

/// Le taux de `IDLE → START_WORKING` (`:129`).
pub const IDLE_DELAY: u32 = 1;
/// Le taux de `START_WORKING → DECIDE` (`:130`), `TICKS_SECOND`.
pub const START_WORKING_DELAY: u32 = TICKS_SECOND;
/// Le taux de `DECIDE` (`:131`), écrit en dur dans le constructeur.
pub const DECIDE_DELAY: u32 = 200;
/// Le taux de `FLORIST_HARVEST` (`:132`), `TICKS_SECOND`.
pub const HARVEST_DELAY: u32 = TICKS_SECOND;
/// Le taux de `FLORIST_COMPOST` (`:133`), `TICKS_SECOND`.
pub const COMPOST_DELAY: u32 = TICKS_SECOND;
/// Le taux du test de vidage (`AbstractEntityAIBasic.java:225`), écrit en dur.
pub const DUMP_CHECK_DELAY: u32 = 100;
/// Le taux de `dumpInventory` (`AbstractEntityAIBasic.java:226`), écrit en dur.
pub const DUMP_DELAY: u32 = 20;

/// `getActionsDoneUntilDumping` (`EntityAIWorkFlorist.java:323-327`) :
/// `HARVEST_ACTIONS_TO_DUMP * building.getBuildingLevel()`.
///
/// Le seul seuil des trois métiers qui dépende du niveau de la hutte ; celui du fermier
/// et du pêcheur sont des constantes (`crate::ai_work_08::cycle::TradeThreshold`).
pub fn actions_until_dumping(building_level: u32) -> u32 {
    HARVEST_ACTIONS_TO_DUMP * building_level
}

/// `TileEntityCompostedDirt.compost(PERCENT_CHANGE_FOR_GROWTH - buildingLevel * 0.01,
/// flower)` (`EntityAIWorkFlorist.java:236`) : la chance qu'une terre compostée fasse
/// pousser une fleur.
pub fn growth_chance(building_level: u32) -> f64 {
    PERCENT_CHANGE_FOR_GROWTH - (building_level as f64 * 0.01)
}

/// `getBlockMiningTime` (`EntityAIWorkFlorist.java:336-340`) : le temps de cueillette
/// d'une fleur, en ticks.
///
/// Java calcule `BASE_BLOCK_MINING_DELAY * (int)(1 + max(0, MAX_BONUS -
/// PER_LEVEL_BONUS * (secondarySkill / 2.0)))` : la troncature est **à l'intérieur** de
/// la multiplication, et le port la garde là (`as u32` sur la parenthèse, pas sur le
/// produit). Dextérité zéro : soixante ticks ; dextérité dix : cinquante.
pub fn mining_time(secondary_skill: u32) -> u32 {
    let bonus = (MAX_BONUS - PER_LEVEL_BONUS * (secondary_skill as f64 / 2.0)).max(0.0);
    BASE_BLOCK_MINING_DELAY * ((1.0 + bonus) as u32)
}

/// `areThereFlowersToGather` (`EntityAIWorkFlorist.java:347-358`) : la première terre
/// qui porte quelque chose au-dessus d'elle.
///
/// Java rend `pos.above()` — la fleur, pas la terre — et s'arrête à la première, dans
/// l'ordre de la liste de la hutte. Une terre dans un chunk déchargé compte comme les
/// autres ici : Java ne teste que `isEmptyBlock`, et `isEmptyBlock` sur un chunk
/// déchargé répond « vide » sans être vrai — le port demande donc le fait au lieu de le
/// deviner (`PlantGround::loaded` n'est pas lu par cette règle, exactement comme Java).
pub fn flowers_to_gather(facts: &FloristFacts) -> Option<Pos> {
    facts
        .plant_ground
        .iter()
        .find(|ground| ground.flower_above)
        .and_then(PlantGround::flower_position)
}

/// Ce qu'un parcours des terres a trouvé — et ce qu'il a retiré de la liste de la hutte.
///
/// Java référence : `getFirstNotCompostedLand` (`:365-386`) et `isThereCompostedLand`
/// (`WorkerUtil.java:310-330`) parcourent `building.getPlantGround()` en retirant les
/// blocs qui ne sont pas des `TileEntityCompostedDirt` (`:381`, `WorkerUtil.java:326`).
/// Une méthode qui modifie la hutte ne peut pas être une fonction pure : le port rend la
/// liste des retraits, à l'appelant de les appliquer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroundScan {
    /// La terre cherchée : la première non compostée (`:376`), ou la première compostée
    /// (`WorkerUtil.java:321`). `None` quand le parcours n'a rien trouvé.
    pub position: Option<Pos>,
    /// Les positions que Java aurait retirées de la liste de la hutte.
    pub removed: Vec<Pos>,
}

/// `getFirstNotCompostedLand` (`EntityAIWorkFlorist.java:365-386`) : la première terre
/// aménagée qui n'a pas encore son compost.
///
/// Trois cas, dans l'ordre de Java : la terre n'est pas chargée, le parcours la saute en
/// silence (`:369`) ; elle est chargée mais ce n'est pas un `TileEntityCompostedDirt`, le
/// parcours la **retire** (`:379-382`) ; c'est une terre aménagée non compostée, il la
/// rend (`:374-377`).
pub fn first_not_composted_land(facts: &FloristFacts) -> GroundScan {
    let mut scan = GroundScan::default();
    for ground in &facts.plant_ground {
        if !ground.loaded {
            continue; // :369
        }
        if !ground.is_composted_dirt {
            scan.removed.push(ground.pos); // :381
            continue;
        }
        if !ground.composted {
            scan.position = Some(ground.pos); // :376
            return scan;
        }
    }
    scan
}

/// `WorkerUtil.isThereCompostedLand` (`WorkerUtil.java:310-330`) : la première terre
/// **déjà** compostée, et les blocs à retirer de la liste au passage.
///
/// Même parcours que [`first_not_composted_land`], avec la condition inverse (`:319`) :
/// c'est la question que `decide` pose avant de se plaindre du compost (`:195`).
pub fn composted_land(facts: &FloristFacts) -> GroundScan {
    let mut scan = GroundScan::default();
    for ground in &facts.plant_ground {
        if !ground.loaded {
            continue; // :314
        }
        if !ground.is_composted_dirt {
            scan.removed.push(ground.pos); // :326
            continue;
        }
        if ground.composted {
            scan.position = Some(ground.pos); // :319-322
            return scan;
        }
    }
    scan
}

/// `checkOrEquipShears` (`EntityAIWorkFlorist.java:307-321`) : les ciseaux sont-ils là ?
///
/// Java répond faux dans deux cas et vrai dans un : `checkForToolOrWeapon` a répondu
/// vrai (`:309-312`, une requête est partie, c'est-à-dire qu'il n'y a pas de ciseaux
/// utilisables), ou aucun slot ne porte de ciseaux d'un niveau suffisant
/// (`:314-320`). Le port rend la raison au lieu d'un booléen, pour que le refus soit
/// nommé.
pub fn shears_ready(facts: &FloristFacts) -> Result<(), AgriRefusal> {
    if facts.tool_requested {
        return Err(AgriRefusal::ShearsMissing); // :309-312
    }
    if facts.shears_slot.is_some() {
        return Ok(()); // :315-319
    }
    Err(AgriRefusal::ShearsMissing) // :320
}

/// `decide` (`EntityAIWorkFlorist.java:151-207`) : quoi faire maintenant.
///
/// L'ordre des questions est celui de Java, et il compte : les terres avant les ciseaux,
/// les ciseaux avant la distance, la distance avant le compost, le compost avant la
/// fleur, la fleur avant la terre à composter.
pub fn decide(scene: &mut FloristScene) -> AgriDecision {
    let FloristScene {
        world,
        facts,
        memory,
        answers,
        ..
    } = scene;

    // :154-158 — aucune terre : le joueur est prévenu (`NO_PLANT_GROUND_FLORIST`) et le
    // fleuriste s'arrête. Une interaction bloquante, que l'appelant déclenche.
    if facts.plant_ground.is_empty() {
        return AgriDecision::go_after(idle(), AgriRefusal::NoPlantGround);
    }

    // :160-163 — pas de ciseaux : `IDLE`, et la requête est partie si elle devait.
    if let Err(refusal) = shears_ready(facts) {
        return AgriDecision::go_after(idle(), refusal);
    }

    // :165 — `worker.setItemInHand(MAIN_HAND, EMPTY)` : la main se vide pour prendre
    // les ciseaux du slot trouvé. C'est un geste, l'appelant le fait.

    // :166-170 — trop loin de la hutte : il rentre d'abord (`DECIDE` le temps du
    // retour, et le taux de `DECIDE` s'applique).
    if world.worker.at.dist_2d(world.building.position) > MAX_DISTANCE as f64
        && !answers.at_building
    {
        return AgriDecision::go_after(decide_state(), AgriRefusal::NotAtHut);
    }

    // :172-180 — du compost dans les coffres de la hutte et rien dans l'inventaire :
    // il va le prendre (`GATHERING_REQUIRED_MATERIALS`).
    if facts.compost_in_inventory == 0 && facts.compost_in_building > 0 {
        return AgriDecision::go_after(gathering_materials(), AgriRefusal::CompostInHut);
    }

    // :181-184 — aucune réserve nulle part : une requête de seize part, et le fleuriste
    // continue avec ce que le sol donne déjà. C'est le seul cas où la décision porte un
    // ordre **et** un état.
    let compost_requested = facts.compost_in_inventory == 0;

    // :187-191 — une fleur pousse : elle passe avant tout le reste.
    if let Some(position) = flowers_to_gather(facts) {
        memory.harvest_position = Some(position); // :187
        let mut decision = AgriDecision::go(harvest_state()); // :190
        if compost_requested {
            decision = decision
                .with_work(AgriWork::RequestCompost {
                    quantity: COMPOST_REQUEST_QTY,
                }) // :183
                .because(AgriRefusal::CompostRequested);
        }
        return decision;
    }

    // :193-201 — pas de compost porté.
    if facts.compost_in_inventory == 0 {
        let scan = composted_land(facts);
        // :195-198 — aucune terre compostée : le joueur est prévenu (`NO_COMPOST`) et le
        // fleuriste repart de `START_WORKING`. Les terres qui ne sont pas des blocs à
        // composter sortent de la liste au passage, comme dans `WorkerUtil.java:326`.
        if scan.position.is_none() {
            let mut decision = AgriDecision::go_after(start_working(), AgriRefusal::NoCompost);
            for position in scan.removed {
                decision = decision.with_work(AgriWork::RemovePlantableGround { position });
            }
            return decision;
        }
        // :200 — la terre pousse toute seule : il repasse dans 200 ticks.
        let mut decision = AgriDecision::go(decide_state());
        for position in scan.removed {
            decision = decision.with_work(AgriWork::RemovePlantableGround { position }); // WorkerUtil.java:326
        }
        return decision;
    }

    // :202-206 — du compost en main : la première terre qui n'en a pas.
    let scan = first_not_composted_land(facts);
    memory.compost_position = scan.position; // :204
    let mut decision = AgriDecision::go(compost_state()); // :205
    for position in scan.removed {
        decision = decision.with_work(AgriWork::RemovePlantableGround { position }); // :381
    }
    if scan.position.is_none() {
        // Java part quand même vers `FLORIST_COMPOST`, qui répondra `START_WORKING`
        // (`:216-219`). Le refus est nommé ici pour que la trace le montre une fois.
        decision = decision.because(AgriRefusal::NoGroundToCompost);
    }
    decision
}

/// `compost` (`EntityAIWorkFlorist.java:214-249`) : marcher jusqu'à la terre et y poser
/// une unité de compost.
///
/// La chance de croissance est `PERCENT_CHANGE_FOR_GROWTH - buildingLevel * 0.01`
/// (`:236`) et la fleur est celle que la hutte a choisie (`:231`). Le port rend les deux
/// dans [`AgriWork::Compost`] et ne fait pas pousser la fleur.
pub fn compost(scene: &mut FloristScene) -> AgriDecision {
    let FloristScene {
        facts,
        memory,
        answers,
        ..
    } = scene;

    // :216-219 — aucune terre retenue par `decide` : rien à faire, retour au début.
    let Some(position) = memory.compost_position else {
        return AgriDecision::go_after(start_working(), AgriRefusal::NoGroundToCompost);
    };

    // :223-226 — `if (!walkToWorkPos(compostPosition)) return getState();`.
    if !answers.at_target {
        return AgriDecision::refuse(AgriRefusal::NotAtTarget);
    }

    // :228-229 — `world.getBlockEntity(compostPosition) instanceof TileEntityCompostedDirt`.
    // Java ne fait rien d'autre si ce n'est pas le cas : il compte une action et repart
    // (`:245-248`). Le port nomme le cas au lieu de le taire.
    let Some(ground) = facts.plant_ground.iter().find(|ground| ground.pos == position) else {
        memory.compost_position = None;
        return AgriDecision::go_after(start_working(), AgriRefusal::GroundGone);
    };
    if !ground.is_composted_dirt {
        memory.compost_position = None;
        return AgriDecision::go_after(start_working(), AgriRefusal::NotCompostedDirt);
    }

    // :231-232 puis :239-242 — la fleur à faire pousser. Sans elle, Java prévient le
    // joueur (`NO_FLOWERS_IN_CONFIG`) et repart sans rien poser.
    let Some(flower) = facts.flower_to_grow.clone() else {
        memory.compost_position = None;
        return AgriDecision::go_after(start_working(), AgriRefusal::NoFlowersInConfig);
    };

    // :234 — `worker.getRandom().nextInt(200 - getPrimarySkillLevel()) < 0 || shrink(…)`.
    // Le tirage ne peut pas être négatif : la condition est donc **toujours** le
    // `shrinkItemCountInItemHandler`, c'est-à-dire « il reste du compost ».
    if facts.compost_in_inventory == 0 {
        memory.compost_position = None;
        return AgriDecision::go_after(start_working(), AgriRefusal::CompostGoneWhileComposting);
    }

    // :236 — une unité de compost, une chance de croissance.
    let chance = growth_chance(facts.building_level);
    memory.compost_position = None; // :247
    AgriDecision::go(start_working()).with_work(AgriWork::Compost {
        position,
        flower,
        chance,
    })
}

/// `harvest` (`EntityAIWorkFlorist.java:256-288`) : marcher jusqu'à la fleur, la casser,
/// compter l'expérience.
///
/// `mineBlock` demande **deux** passages quand le bloc n'est pas immédiatement cassable
/// (`AbstractEntityAIInteract.java:141-149`) : le premier rend faux et Java ressort du
/// tick. C'est `GoalAnswers::mined` qui porte cette réponse.
pub fn harvest(scene: &mut FloristScene) -> AgriDecision {
    let FloristScene {
        facts,
        memory,
        answers,
        ..
    } = scene;

    // :258-261 — aucune position, ou les ciseaux ont disparu entre-temps.
    let Some(position) = memory.harvest_position else {
        return AgriDecision::go_after(start_working(), AgriRefusal::NoFlowerToHarvest);
    };
    if let Err(refusal) = shears_ready(facts) {
        return AgriDecision::go_after(start_working(), refusal);
    }

    // :265-268 — en route.
    if !answers.at_target {
        return AgriDecision::refuse(AgriRefusal::NotAtTarget);
    }

    // :272-275 — `if (!mineBlock(harvestPosition)) return getState();`.
    if !answers.mined {
        return AgriDecision::refuse(AgriRefusal::MiningIncomplete);
    }

    // :277 — `addExperience(XP_PER_FLOWER)` ; :279-282 — la statistique `FLOWERS_PICKED` ;
    // :284-287 — une action comptée, la position oubliée, retour au début.
    memory.harvest_position = None;
    let mut decision = AgriDecision::go(start_working()).with_work(AgriWork::Harvest {
        position,
        experience: XP_PER_FLOWER,
    });
    // `damageItemInHand` (`CitizenItemUtils`, appelé par `mineBlock` avec `damageTool`)
    // peut casser les ciseaux : Java ne le dit pas, le port le nomme.
    if answers.tool_broken {
        decision = decision.because(AgriRefusal::ToolBroken);
    }
    decision
}

/// `registerTargets` du constructeur (`EntityAIWorkFlorist.java:128-134`), plus les
/// quatre cibles de base que ce métier franchit.
pub fn register(machine: &mut Machine<AgriState, FloristScene>) {
    // :129 — `new AITarget(IDLE, START_WORKING, 1)`.
    machine.add(Transition::goto(
        idle(),
        start_working(),
        "idleToStartWorking",
        "EntityAIWorkFlorist.java:129",
        IDLE_DELAY,
    ));
    // :130 — `new AITarget(START_WORKING, DECIDE, TICKS_SECOND)`.
    machine.add(Transition::goto(
        start_working(),
        decide_state(),
        "startWorkingToDecide",
        "EntityAIWorkFlorist.java:130",
        START_WORKING_DELAY,
    ));
    // :131 — `new AITarget(DECIDE, this::decide, 200)`.
    machine.add(Transition::target(
        decide_state(),
        "decide",
        "EntityAIWorkFlorist.java:131",
        DECIDE_DELAY,
        Action::Run(decide_action),
    ));
    // :132 — `new AITarget(FLORIST_HARVEST, this::harvest, TICKS_SECOND)`.
    machine.add(Transition::target(
        harvest_state(),
        "harvest",
        "EntityAIWorkFlorist.java:132",
        HARVEST_DELAY,
        Action::Run(harvest_action),
    ));
    // :133 — `new AITarget(FLORIST_COMPOST, this::compost, TICKS_SECOND)`.
    machine.add(Transition::target(
        compost_state(),
        "compost",
        "EntityAIWorkFlorist.java:133",
        COMPOST_DELAY,
        Action::Run(compost_action),
    ));
    // `AbstractEntityAIBasic.java:225` — `new AIEventTarget(STATE_BLOCKING,
    // this::inventoryNeedsDump, INVENTORY_FULL, 100)` : le vidage, depuis n'importe quel
    // état, et il passe **avant** la liste de l'état courant.
    machine.add(Transition::event_when(
        Blocking::StateBlocking,
        "inventoryNeedsDump",
        "AbstractEntityAIBasic.java:225",
        DUMP_CHECK_DELAY,
        cycle::inventory_needs_dump,
        Action::Fixed(Some(inventory_full())),
    ));
    // `AbstractEntityAIBasic.java:226` — `new AITarget(INVENTORY_FULL, this::dumpInventory, 20)`.
    machine.add(Transition::target(
        inventory_full(),
        "dumpInventory",
        "AbstractEntityAIBasic.java:226",
        DUMP_DELAY,
        Action::Run(dump_action),
    ));
    // `AbstractEntityAIBasic.java:242` — `new AITarget(GATHERING_REQUIRED_MATERIALS,
    // this::getNeededItem, STANDARD_DELAY)` : `decide` y entre dès qu'il y a du compost à
    // prendre dans la hutte (`EntityAIWorkFlorist.java:179`).
    machine.add(Transition::target(
        gathering_materials(),
        "getNeededItem",
        "AbstractEntityAIBasic.java:242",
        crate::ai::worker::STANDARD_DELAY,
        Action::Run(get_needed_item_action),
    ));
    // `AbstractEntityAIBasic.java:251` — `new AITarget(PAUSED, this::bePaused, TICKS_SECOND)`.
    // Sans elle, `afterDump` renverrait `PAUSED` (`:1213-1222`) dans un état piège.
    machine.add(Transition::goto(
        paused(),
        idle(),
        "bePaused",
        "AbstractEntityAIBasic.java:251",
        TICKS_SECOND,
    ));
}

/// Le fleuriste : sa machine à états, ses faits, ses ordres.
#[derive(Debug)]
pub struct FloristAi {
    machine: Machine<AgriState, FloristScene>,
}

impl FloristAi {
    /// Le fleuriste, dans `IDLE`, à `ENTITY_AI_TICKRATE`.
    ///
    /// Java commence dans `INIT` et `initSafetyChecks` l'envoie dans `IDLE`
    /// (`AbstractEntityAIBasic.java:446-465`) ; `INIT` et ces contrôles appartiennent à
    /// `crate::ai`, donc cette machine démarre là où cette table-là arrive.
    pub fn new() -> FloristAi {
        let mut machine = Machine::new(idle(), ENTITY_AI_TICKRATE);
        register(&mut machine);
        FloristAi { machine }
    }

    /// L'état courant (`AbstractAISkeleton.getState`, `:120-126`).
    pub fn state(&self) -> AgriState {
        self.machine.state()
    }

    /// La table, pour un appelant qui veut la lire.
    pub fn machine(&self) -> &Machine<AgriState, FloristScene> {
        &self.machine
    }

    /// Le piège du dernier tick, si un état n'avait aucune sortie
    /// (`BasicStateMachine.java:220-228`).
    pub fn missing(&self) -> Option<crate::ai::state::MissingTransition<AgriState>> {
        self.machine.missing()
    }

    /// `AbstractAISkeleton.resetAI` (`:145-152`).
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Un tick : la scène est nettoyée, la machine tourne, et le tick rapporte ce qu'il a
    /// décidé.
    pub fn tick(&mut self, scene: &mut FloristScene) -> FloristTick {
        scene.clear_tick();
        let fired = self.machine.tick(scene);
        FloristTick {
            fired,
            refusal: scene.refusal,
            works: scene.works.clone(),
        }
    }
}

impl Default for FloristAi {
    fn default() -> FloristAi {
        FloristAi::new()
    }
}

/// Ce qu'un tick du fleuriste a produit : la transition s'il y en a une, le refus, et les
/// ordres au monde.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloristTick {
    /// La transition qui a bougé la machine, `None` quand le tick n'a rien pu faire
    /// (compte à rebours, ou aucune cible ne s'applique).
    pub fired: Option<Fired<AgriState>>,
    /// La raison pour laquelle l'action s'est arrêtée.
    pub refusal: Option<AgriRefusal>,
    /// Ce que le tick demande au monde.
    pub works: Vec<AgriWork>,
}

/// `Action::Run` du `DECIDE` (`:131`).
fn decide_action(scene: &mut FloristScene, from: AgriState) -> Option<AgriState> {
    let decision = decide(scene);
    apply(scene, decision, from)
}

/// `Action::Run` de `FLORIST_HARVEST` (`:132`).
fn harvest_action(scene: &mut FloristScene, from: AgriState) -> Option<AgriState> {
    let decision = harvest(scene);
    apply(scene, decision, from)
}

/// `Action::Run` de `FLORIST_COMPOST` (`:133`).
fn compost_action(scene: &mut FloristScene, from: AgriState) -> Option<AgriState> {
    let decision = compost(scene);
    apply(scene, decision, from)
}

/// Écrire la décision dans la scène, et rendre l'état — Java's `return getState()` quand
/// la décision n'en change pas.
fn apply(scene: &mut FloristScene, decision: AgriDecision, from: AgriState) -> Option<AgriState> {
    scene.refusal = decision.refusal;
    scene.works = decision.works;
    Some(decision.next.unwrap_or(from))
}

/// `dumpInventory` (`AbstractEntityAIBasic.java:1180-1204`) : vider une pile, puis
/// `afterDump` (`:1213-1222`).
///
/// Le port ne déplace pas d'objet : il lit `GoalAnswers::transferred` — le transfert
/// est fini — et rend `IDLE` ou `PAUSED`. Tant qu'il ne l'est pas, il retient l'état et
/// nomme le refus.
fn dump_action(scene: &mut FloristScene, from: AgriState) -> Option<AgriState> {
    if !scene.answers.transferred {
        scene.refusal = Some(AgriRefusal::TransferIncomplete); // :1230-1235
        return Some(from);
    }
    // :1215-1221 — `isPaused()` : `PAUSED`, sinon `IDLE`.
    Some(if scene.world.worker.paused {
        paused()
    } else {
        idle()
    })
}

/// `getNeededItem` (`AbstractEntityAIBasic.java:242`, corps à `:323-326`) : prendre la
/// pile dans le coffre de la hutte, puis `IDLE`.
fn get_needed_item_action(scene: &mut FloristScene, from: AgriState) -> Option<AgriState> {
    if !scene.answers.transferred {
        scene.refusal = Some(AgriRefusal::TransferIncomplete); // :1230-1235
        return Some(from);
    }
    Some(idle()) // :326
}

// Les sept états que ce métier nomme, une fois chacun, pour que les lignes au-dessus
// restent lisibles et qu'un état mal orthographié ne compile pas.
fn idle() -> AgriState {
    AgriState::Gather(GatherState::Base(crate::ai::state::WorkerState::Idle))
}
fn start_working() -> AgriState {
    AgriState::Gather(GatherState::Base(
        crate::ai::state::WorkerState::StartWorking,
    ))
}
fn decide_state() -> AgriState {
    AgriState::Gather(GatherState::Base(crate::ai::state::WorkerState::Decide))
}
fn inventory_full() -> AgriState {
    AgriState::Gather(GatherState::Base(
        crate::ai::state::WorkerState::InventoryFull,
    ))
}
fn gathering_materials() -> AgriState {
    AgriState::Gather(GatherState::Base(
        crate::ai::state::WorkerState::GatheringRequiredMaterials,
    ))
}
fn paused() -> AgriState {
    AgriState::Gather(GatherState::Base(crate::ai::state::WorkerState::Paused))
}
fn harvest_state() -> AgriState {
    AgriState::FloristHarvest
}
fn compost_state() -> AgriState {
    AgriState::FloristCompost
}
