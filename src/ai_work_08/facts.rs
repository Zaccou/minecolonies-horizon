//! Ce que les trois buts de `production/agriculture/` demandent au monde, et ce qu'ils
//! gardent d'un tick à l'autre.
//!
//! Java référence : `EntityAIWorkFlorist.java` (`harvestPosition` `:111`,
//! `compostPosition` `:116`, `building.getPlantGround()` `:154`, `getFlowerToGrow()`
//! `:231`), `EntityAIWorkFarmer.java` (`FarmerFacts`, déjà dans `crate::ai_gather`) et
//! `EntityAIWorkFisherman.java` (`FishermanFacts`, idem).
//!
//! **Rien n'est réécrit.** Le fermier et le pêcheur ont leurs faits dans
//! `crate::ai_gather::facts` et ce module les consomme tels quels, à travers
//! [`AgriScene::Gather`]. Le fleuriste n'existait nulle part : [`FloristFacts`] est donc
//! le seul jeu de faits neuf, et il ne décrit que ce que `EntityAIWorkFlorist` lit
//! vraiment — pas un bloc de plus.
//!
//! **Aucun appel au monde, et deux réponses que le port doit demander.** Java écrit
//! `if (!walkToWorkPos(pos))` (`:223`) et `if (!mineBlock(pos))` (`:272`) : le résultat
//! de ces deux appels n'est pas une observation du monde, c'est la **réponse** de
//! l'action du tick précédent. Elles vivent dans [`GoalAnswers`], avec le transfert de
//! coffre et deux faits que seul l'appelant connaît (un outil cassé, le joueur qui a
//! déplacé la hutte).

use crate::ai::facts::{Pos, WorldFacts};
use crate::ai_gather::{
    FarmerFacts, FishermanFacts, GatherJob, GatherMemory, GatherScene, GatherTrade,
};

use super::state::{AgriRefusal, AgriWork};

/// Une terre de `building.getPlantGround()`, avec tout ce que les deux parcours du
/// fleuriste lui demandent.
///
/// Java référence : `areThereFlowersToGather` (`EntityAIWorkFlorist.java:348-358`) lit
/// `world.isEmptyBlock(pos.above())`, `getFirstNotCompostedLand` (`:365-386`) lit
/// `WorldUtil.isEntityBlockLoaded`, `instanceof TileEntityCompostedDirt` et
/// `isComposted()`. Quatre observations, quatre champs — le port ne peut pas les
/// redemander au monde, donc il les reçoit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlantGround {
    /// La terre elle-même ; la fleur est au-dessus, `pos.above()` (`:354`).
    pub pos: Pos,
    /// `WorldUtil.isEntityBlockLoaded(world, pos)` (`:369`) et `WorkerUtil.java:314` :
    /// une terre dans un chunk déchargé n'est ni lue ni retirée.
    pub loaded: bool,
    /// `world.getBlockEntity(pos) instanceof TileEntityCompostedDirt` (`:372`).
    pub is_composted_dirt: bool,
    /// `((TileEntityCompostedDirt) entity).isComposted()` (`:374`) : elle a déjà son
    /// compost, elle pousse.
    pub composted: bool,
    /// `!world.isEmptyBlock(pos.above())` (`:352`) : quelque chose pousse dessus, et
    /// c'est une fleur pour le fleuriste.
    pub flower_above: bool,
}

impl PlantGround {
    /// Une terre chargée, vide, qui n'est pas un bloc compostable : le cas où Java la
    /// retire de la liste de la hutte (`:381`).
    pub fn bare(pos: Pos) -> PlantGround {
        PlantGround {
            pos,
            loaded: true,
            ..PlantGround::default()
        }
    }

    /// Une terre aménagée par le joueur, pas encore compostée.
    pub fn composted_dirt(pos: Pos) -> PlantGround {
        PlantGround {
            is_composted_dirt: true,
            ..PlantGround::bare(pos)
        }
    }

    /// Une terre aménagée et déjà compostée : elle pousse, le fleuriste laisse faire.
    pub fn growing(pos: Pos) -> PlantGround {
        PlantGround {
            composted: true,
            ..PlantGround::composted_dirt(pos)
        }
    }

    /// Une terre avec une fleur dessus : à cueillir avant tout le reste (`:187-191`).
    pub fn with_flower(pos: Pos) -> PlantGround {
        PlantGround {
            flower_above: true,
            ..PlantGround::growing(pos)
        }
    }

    /// Une terre dans un chunk que le serveur n'a pas chargé.
    pub fn unloaded(pos: Pos) -> PlantGround {
        PlantGround {
            loaded: false,
            ..PlantGround::composted_dirt(pos)
        }
    }

    /// La position de la fleur, quand il y en a une (`pos.above()`, `:354`).
    pub fn flower_position(&self) -> Option<Pos> {
        if self.flower_above {
            return Some(Pos::new(self.pos.x, self.pos.y + 1, self.pos.z));
        }
        None
    }
}

/// Les faits du fleuriste : ceux de `EntityAIWorkFlorist.java`, et aucun autre.
///
/// Chaque champ cite la ligne qui le lit. Les deux niveaux de compétence sont là parce
/// que Java en tire deux nombres : la chance de croissance passe par le tirage
/// `nextInt(200 - getPrimarySkillLevel())` (`:234`, jamais négatif, voir [`super::florist::compost`])
/// et la durée de minage par `getSecondarySkillLevel()` (`:339`).
#[derive(Clone, Debug, PartialEq)]
pub struct FloristFacts {
    /// `building.getBuildingLevel()` (`:236`, `:326`) : la hutte du fleuriste.
    pub building_level: u32,
    /// `building.getPlantGround()` (`:154`, `:350`, `:367`) : les terres du joueur.
    pub plant_ground: Vec<PlantGround>,
    /// `checkForToolOrWeapon(ModEquipmentTypes.shears)` (`:309`) : vrai quand une
    /// requête de ciseaux est partie, c'est-à-dire quand le fleuriste **n'a pas** de
    /// ciseaux utilisables.
    pub tool_requested: bool,
    /// `getFirstSlotOfItemHandlerContainingEquipment(…, shears, 0, maxEquipmentLevel)`
    /// (`:314`), `None` quand la réponse est `-1`.
    pub shears_slot: Option<usize>,
    /// `getItemCountInItemHandler(worker.getInventoryCitizen(), IS_COMPOST)` (`:172`).
    pub compost_in_inventory: u32,
    /// `hasBuildingEnoughElseCount(building, IS_COMPOST, 1)` (`:175`) : ce que les
    /// coffres de la hutte en tiennent.
    pub compost_in_building: u32,
    /// `building.getFlowerToGrow()` (`:231`) : l'identifiant de la fleur à faire
    /// pousser, `None` quand le joueur n'en a pas choisi.
    pub flower_to_grow: Option<String>,
    /// `getPrimarySkillLevel()` (`:234`).
    pub primary_skill: u32,
    /// `getSecondarySkillLevel()` (`:339`).
    pub secondary_skill: u32,
    /// `building.getMaxEquipmentLevel()` (`:314`).
    pub max_equipment_level: u8,
}

impl Default for FloristFacts {
    /// Une hutte de niveau un, aucune terre, aucun compost, aucune fleur choisie — un
    /// fleuriste qui vient d'être embauché dans une hutte bâtie. Pas zéro : une hutte de
    /// niveau zéro n'embauche personne, et `getActionsDoneUntilDumping` (`:326`)
    /// vaudrait zéro, donc un vidage à chaque tick.
    fn default() -> FloristFacts {
        FloristFacts {
            building_level: 1,
            plant_ground: Vec::new(),
            tool_requested: false,
            shears_slot: Some(0),
            compost_in_inventory: 0,
            compost_in_building: 0,
            flower_to_grow: None,
            primary_skill: 0,
            secondary_skill: 0,
            max_equipment_level: 5,
        }
    }
}

impl FloristFacts {
    /// Une hutte de niveau `level`, avec des ciseaux en main et une fleur choisie : le
    /// plus court chemin vers un fleuriste qui peut travailler.
    pub fn ready(level: u32) -> FloristFacts {
        FloristFacts {
            building_level: level,
            flower_to_grow: Some("minecraft:poppy".to_string()),
            ..FloristFacts::default()
        }
    }

    /// Ajouter une terre.
    pub fn with_ground(mut self, ground: PlantGround) -> FloristFacts {
        self.plant_ground.push(ground);
        self
    }

    /// Le compost que le fleuriste porte.
    pub fn with_compost(mut self, count: u32) -> FloristFacts {
        self.compost_in_inventory = count;
        self
    }

    /// Les terres déjà compostées (`WorkerUtil.isThereCompostedLand`,
    /// `WorkerUtil.java:310-330`).
    pub fn composted_ground(&self) -> impl Iterator<Item = &PlantGround> {
        self.plant_ground
            .iter()
            .filter(|ground| ground.loaded && ground.is_composted_dirt && ground.composted)
    }
}

/// Les réponses que le monde donne au but pendant **ce** tick.
///
/// Java pose ces questions lui-même et lit un booléen :
///
/// | Champ | Ce que Java appelle | Ligne |
/// | --- | --- | --- |
/// | [`GoalAnswers::at_target`] | `walkToWorkPos` / `walkToSafePos` | `EntityAIWorkFlorist.java:223`, `:265` ; `EntityAIWorkFarmer.java:529` ; `EntityAIWorkFisherman.java:317` |
/// | [`GoalAnswers::at_building`] | `walkToBuilding()` | `EntityAIWorkFlorist.java:167` |
/// | [`GoalAnswers::mined`] | `mineBlock(...)` | `EntityAIWorkFlorist.java:272` ; `AbstractEntityAIInteract.java:135-138` |
/// | [`GoalAnswers::transferred`] | la fin de `dumpInventory` / `getNeededItem` | `AbstractEntityAIBasic.java:1213-1222`, `:323-326` |
/// | [`GoalAnswers::tool_broken`] | **rien** : Java ne le distingue pas | voir le champ |
///
/// Le port ne peut pas marcher ni miner : il reçoit la réponse et décide avec elle.
/// C'est ce qui rend le scénario rejouable — la même suite de faits produit la même
/// suite d'états.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoalAnswers {
    /// `walkToWorkPos(...)` a répondu vrai : le citoyen est sur la position de travail.
    pub at_target: bool,
    /// `walkToBuilding()` a répondu vrai : le citoyen est à sa hutte.
    pub at_building: bool,
    /// `mineBlock(...)` a répondu vrai : le bloc est cassé, l'action est finie.
    pub mined: bool,
    /// Le transfert avec les coffres de la hutte est fini — le vidage comme la prise
    /// d'une pile.
    pub transferred: bool,
    /// **Pas un appel Java.** `damageItemInHand` (`EntityAIWorkFlorist.java:294-298`)
    /// détruit un outil sans le dire à l'IA : au tick suivant `checkForToolOrWeapon`
    /// (`:309`) répond « il en faut un », exactement comme pour un outil jamais eu. Le
    /// port nomme donc le cas, et c'est l'appelant qui répond.
    pub tool_broken: bool,
}

impl GoalAnswers {
    /// Le début d'un tick : le citoyen marche, rien n'est cassé, rien n'est transféré.
    pub fn walking() -> GoalAnswers {
        GoalAnswers::default()
    }

    /// Le citoyen est arrivé sur sa position de travail.
    pub fn arrived() -> GoalAnswers {
        GoalAnswers {
            at_target: true,
            ..GoalAnswers::default()
        }
    }

    /// Arrivé, le bloc est cassé : l'action du tick a abouti.
    pub fn broken() -> GoalAnswers {
        GoalAnswers {
            mined: true,
            ..GoalAnswers::arrived()
        }
    }

    /// Arrivé à la hutte et le transfert est fini.
    pub fn transferred() -> GoalAnswers {
        GoalAnswers {
            at_building: true,
            transferred: true,
            ..GoalAnswers::default()
        }
    }

    /// Ajouter la réponse d'un déplacement vers la hutte.
    pub fn with_at_building(mut self, at_building: bool) -> GoalAnswers {
        self.at_building = at_building;
        self
    }

    /// Ajouter la réponse d'une cueillette ou d'un minage.
    pub fn with_mined(mut self, mined: bool) -> GoalAnswers {
        self.mined = mined;
        self
    }
}

/// Ce que le fleuriste garde d'un tick à l'autre — **pas** un fait sur le monde.
///
/// Java référence : `harvestPosition` (`EntityAIWorkFlorist.java:111`) et
/// `compostPosition` (`:116`). Les deux valent `null` après l'action (`:247`, `:286`),
/// et c'est le `null` que `compost` (`:216-219`) et `harvest` (`:258-261`) testent en
/// premier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgriMemory {
    /// `harvestPosition` (`:111`) : la fleur que `decide` a choisie (`:187`).
    pub harvest_position: Option<Pos>,
    /// `compostPosition` (`:116`) : la terre que `decide` a choisie (`:204`).
    pub compost_position: Option<Pos>,
}

/// La scène du fleuriste : le monde, ses faits, sa mémoire, les réponses du tick, et ce
/// qu'il a décidé de demander.
#[derive(Clone, Debug)]
pub struct FloristScene {
    /// Le citoyen, son inventaire, sa hutte et les blocs (`crate::ai::facts`).
    pub world: WorldFacts,
    /// Les observations du fleuriste.
    pub facts: FloristFacts,
    /// Ce qu'il retient entre deux ticks.
    pub memory: AgriMemory,
    /// Les réponses du tick en cours, recopiées par [`AgriScene::set_answers`].
    pub answers: GoalAnswers,
    /// Le refus du tick, remis à zéro au début de chaque tick.
    pub refusal: Option<AgriRefusal>,
    /// Les ordres du tick (`RequestCompost`, `Compost`, `Harvest`,
    /// `RemovePlantableGround`), remis à zéro au début de chaque tick.
    pub works: Vec<AgriWork>,
}

impl FloristScene {
    /// Une scène sur `world` avec les faits par défaut d'un fleuriste embauché.
    pub fn new(world: WorldFacts) -> FloristScene {
        FloristScene::with_facts(world, FloristFacts::default())
    }

    /// Une scène sur `world` et `facts`.
    pub fn with_facts(world: WorldFacts, facts: FloristFacts) -> FloristScene {
        FloristScene {
            world,
            facts,
            memory: AgriMemory::default(),
            answers: GoalAnswers::default(),
            refusal: None,
            works: Vec::new(),
        }
    }

    /// Remettre à zéro ce qu'un tick produit — pas ce qu'il retient.
    pub fn clear_tick(&mut self) {
        self.refusal = None;
        self.works.clear();
    }
}

/// La scène d'un métier : celle de `crate::ai_gather` pour le fermier et le pêcheur,
/// celle de ce module pour le fleuriste.
///
/// **Pourquoi un enum et pas une seule structure.** `crate::ai_gather::GatherScene`
/// porte `GatherJob`, qui n'a pas de variante fleuriste et ne peut pas en recevoir (ce
/// module n'écrit pas dans `src/ai_gather`). Mettre un `GatherJob::Farmer` dans une
/// scène de fleuriste serait un fait qui ment. Deux variantes disent la vérité, et
/// [`AgriScene::world`] donne ce que les deux partagent.
pub enum AgriScene {
    /// La scène du fermier ou du pêcheur, telle que `crate::ai_gather` la définit.
    Gather(Box<GatherScene>),
    /// La scène du fleuriste.
    Florist(Box<FloristScene>),
}

impl AgriScene {
    /// La scène du fermier, sur `world`.
    pub fn farmer(world: WorldFacts) -> AgriScene {
        AgriScene::Gather(Box::new(GatherScene::new(
            world,
            GatherJob::Farmer(FarmerFacts::default()),
        )))
    }

    /// La scène du pêcheur, sur `world`.
    pub fn fisherman(world: WorldFacts) -> AgriScene {
        AgriScene::Gather(Box::new(GatherScene::new(
            world,
            GatherJob::Fisherman(FishermanFacts::default()),
        )))
    }

    /// La scène du fleuriste, sur `world`.
    pub fn florist(world: WorldFacts) -> AgriScene {
        AgriScene::Florist(Box::new(FloristScene::new(world)))
    }

    /// La scène du fleuriste avec ses faits.
    pub fn florist_with(world: WorldFacts, facts: FloristFacts) -> AgriScene {
        AgriScene::Florist(Box::new(FloristScene::with_facts(world, facts)))
    }

    /// Le citoyen, son inventaire et sa hutte — le seul bloc que les trois métiers
    /// partagent.
    pub fn world(&self) -> &WorldFacts {
        match self {
            AgriScene::Gather(scene) => &scene.facts,
            AgriScene::Florist(scene) => &scene.world,
        }
    }

    /// La scène de `crate::ai_gather`, si c'en est une.
    pub fn gather(&self) -> Option<&GatherScene> {
        match self {
            AgriScene::Gather(scene) => Some(scene),
            AgriScene::Florist(_) => None,
        }
    }

    /// La scène de `crate::ai_gather`, en écriture.
    pub fn gather_mut(&mut self) -> Option<&mut GatherScene> {
        match self {
            AgriScene::Gather(scene) => Some(scene),
            AgriScene::Florist(_) => None,
        }
    }

    /// La scène du fleuriste, si c'en est une.
    pub fn florist_scene(&self) -> Option<&FloristScene> {
        match self {
            AgriScene::Gather(_) => None,
            AgriScene::Florist(scene) => Some(scene),
        }
    }

    /// La scène du fleuriste, en écriture.
    pub fn florist_scene_mut(&mut self) -> Option<&mut FloristScene> {
        match self {
            AgriScene::Gather(_) => None,
            AgriScene::Florist(scene) => Some(scene),
        }
    }

    /// Les réponses du tick, quand la scène en porte.
    pub fn answers(&self) -> GoalAnswers {
        match self {
            AgriScene::Gather(_) => GoalAnswers::default(),
            AgriScene::Florist(scene) => scene.answers,
        }
    }

    /// Recopier les réponses du tick dans la scène, pour que les actions les lisent.
    pub fn set_answers(&mut self, answers: GoalAnswers) {
        if let AgriScene::Florist(scene) = self {
            scene.answers = answers;
        }
    }

    /// Le métier de `crate::ai_gather` que cette scène porte.
    pub fn gather_trade(&self) -> Option<GatherTrade> {
        match self {
            AgriScene::Gather(scene) => Some(match &scene.job {
                GatherJob::Farmer(_) => GatherTrade::Farmer,
                GatherJob::Fisherman(_) => GatherTrade::Fisherman,
                // Les deux autres métiers ne sont pas ceux de ce module, mais la scène
                // peut en porter un : on le nomme plutôt que de mentir.
                GatherJob::Lumberjack(_) => GatherTrade::Lumberjack,
                GatherJob::Miner(_) => GatherTrade::Miner,
            }),
            AgriScene::Florist(_) => None,
        }
    }

    /// La mémoire de `crate::ai_gather` : celle du fermier et du pêcheur vit là, le
    /// fleuriste garde la sienne dans [`AgriMemory`].
    pub fn gather_memory(&self) -> Option<&GatherMemory> {
        self.gather().map(|scene| &scene.memory)
    }
}
