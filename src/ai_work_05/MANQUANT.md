# MANQUANT — ce que `ai_work_05` ne peut pas faire faute d'API

Aucune de ces entrées n'est un appel fictif : le code de ce module ne les utilise
pas. Il rend des **demandes** (`PupilAsk`) que l'appelant exécutera, ou il reçoit le
résultat en **fait** (`PupilFacts`). La liste dit ce qu'il faudrait pour que ces
demandes puissent être exécutées par une IA branchée sur le jeu.

Mesure faite sur `.horizon/sdk/api/crates/` (les cinq crates du SDK) et sur
`src/` du port.

## 1. Aucune API de déplacement de citoyen

**Ce que je voulais faire.** Les quatre marches du métier :
`EntityNavigationUtils.walkToBuilding(worker, building)`
(`EntityAIWorkPupil.java:218`), `walkToPosInBuilding(worker, pos, building, 7)`
(`:134`, via `AbstractEntityAIBasic.walkToWorkPos`, `:800-813`),
`walkToRandomPosWithin(worker, 10, DEFAULT_SPEED, corners)` (`:118`) et
`worker.getNavigation().isDone()` (`:113`).

**Ce qui manque.** Le SDK n'expose ni entité pilotable, ni navigation, ni
chemin : `grep -rn "Navigat" horizon-mod-api/src` ne rend rien, et le crate ne
connaît pas le mot « citizen ». `crate::pathfinding` du port calcule un chemin, il
ne le fait pas suivre.

**Ce que fait le port.** `PupilAsk::WalkToBuilding`, `WalkToSeat(pos)`,
`Wander { radius, corners }`, et le fait `PupilFacts::at_seat` / `at_building` /
`navigation_done` que l'appelant renseigne.

## 2. Aucune API de monture ni de siège

**Ce que je voulais faire.** `SittingEntity.sitDown(studyPos, worker, ticks)`
(`:150`), `worker.stopRiding()` + `worker.setPos(y + 1)` (`:175-176`), et la lecture
`worker.vehicle != null` (`:146`).

**Ce qui manque.** Rien dans le SDK ne nomme une entité, encore moins son véhicule.

**Ce que fait le port.** `PupilAsk::Sit { pos, ticks }` et `PupilAsk::StandUp` ;
la mémoire `PupilMemory::riding` tient le rôle de `worker.vehicle != null`.

## 3. Aucune API de compétence, de satiété ni de statistique

**Ce que je voulais faire.** `addXpToSkill(Skill.Intelligence, 50 * (1 + TEACHING))`
(`:188`), `decreaseSaturationForContinuousAction()` (`:198`), et les deux
statistiques `ITEM_USED` (`:184`) et `LEVELS_GAINED` (`:194`).

**Ce qui manque.** `grep -rn "experience\|saturation\|citizen"` sur
`horizon-mod-api/src` : aucun résultat. Le seul état que le SDK porte est celui des
registres (items, blocs, particules, menus), pas celui d'un citoyen.

**Ce que fait le port.** `PupilAsk::PayLesson { xp }` (le montant est calculé par
`lesson_xp`, la recherche `TEACHING` étant un fait), `PupilAsk::ConsumePaper`,
`PupilAsk::DecreaseSaturation`. Le gain de niveau et la statistique qui en découle
sont laissés au porteur de compétences — c'est lui qui connaît le niveau d'avant.

## 4. Aucune particule envoyée à un client

**Ce que je voulais faire.**
`Network.sendToTrackingEntity(CircleParticleEffectMessage(worker.position().add(0, 1, 0),
ParticleTypes.ENCHANT | HAPPY_VILLAGER, sittingTicks), worker)` (`:158`, `:163`).

**Ce qui manque.** `parts/particles.rs` n'expose que des **clés de particule**
d'enregistrement (`fn particle_key(&mut self, id: &str)`), pas l'émission d'un anneau
autour d'une entité pour les clients qui la voient.

**Ce que fait le port.** `PupilAsk::Particle { kind, count }` avec
`Particle::Enchant | HappyVillager` ; le port ne dit pas *où* (Java aussi passe la
position du citoyen, pas celle du siège).

## 5. Aucun accès au monde vivant : état de bloc et requête d'entités

**Ce que je voulais faire.** Deux lectures que le métier fait à chaque tick :

- `BuildingSchool.isCarpet(colony.getWorld().getBlockState(pos))`
  (`BuildingSchool.java:138-141`, puis `:151-154`) : le tapis tiré au sort est-il
  encore un `WoolCarpetBlock` ? C'est ce qui produit le refus `StaleCarpet`.
- `world.getEntitiesOfClass(EntityCitizen.class, new AABB(pos, pos))` (`:139-141`) :
  y a-t-il un citoyen sur le siège ? C'est le refus `SeatTaken`.

**Ce qui manque.** `crate::blocks` est un **registre** (une table de blocs et de
leurs propriétés), pas un lecteur d'instance ; aucune API du SDK ne lit l'état d'un
bloc à une position ni n'énumère des entités dans une boîte.

**Ce que fait le port.** Le fait `Seat { pos, still_a_carpet }` (l'appelant a déjà
posé la question) et le fait `PupilFacts::other_citizen_at_seat`.

## 6. Aucun lecteur des positions marquées d'un schéma

**Ce que je voulais faire.** `building.getLocationsFromTag(TAG_SITTING)`
(`BuildingSchool.java:127`, `TAG_SITTING = "sit"`, `SchematicTagConstants.java:11`) :
la liste de sièges **prioritaire**, celle du schéma de l'école.

**Ce qui manque.** `crate::structures` résout quel fichier correspond à une hutte et
à un niveau ; `crate::blueprint` lit les blocs et la palette, mais rien ne lit les
**annotations de tag** d'un schéma (les positions nommées `sit`, `sit_in`,
`stand_out`, `work`…). `building/instance.rs` n'a aucune notion de tag.

**Ce que fait le port.** `PupilFacts::sitting_tag: Vec<Pos>` — le fait est reçu
prêt. Conséquence à ne pas oublier : tant que ce lecteur n'existe pas, une école
réelle n'aura **que** ses tapis, jamais ses sièges de schéma, et l'écolier sera plus
souvent en `NoSeat` qu'en jeu.

## 7. Ce qui n'est pas un manque du SDK mais du port

Rien à faire côté Horizon, mais à savoir avant de brancher :

- **`Trade::StoneSmeltery` n'a pas de machine complète ici.** Sa table est celle du
  crafter (`crate::ai_build::crafter`) **plus** les sept cibles de
  `AbstractEntityAIRequestSmelter.java:71-82`, qui vivent sur d'autres états
  (`RETRIEVING_END_PRODUCT_FROM_FURNACE`, `FILL_UP_FURNACES`, `ADD_FUEL_TO_FURNACE`,
  `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE`) et d'autres faits. Ces cibles sont
  portées par `src/ai_work_01/furnace.rs`, `src/ai_work_02/smelter.rs` et
  `src/ai_work_03/smelter.rs` ; ce module porte l'**ordre de décision**
  (`ERRAND_ORDER`) et les **gardes** (`PRE_GUARDS`), pas une quatrième table.
- **`Machine<S, F>` est monomorphique en `S` et en `F`.** Un métier ne peut donc pas
  *contenir* la table de base (`crate::ai::worker::register_base`, sur
  `WorkerState`/`Scene`). Le pont est le même que `crate::ai_gather` : trois états de
  base et trois retours vers `IDLE`, cités ligne à ligne dans `pupil.rs`.
- **Le quadruple de la table de base n'est pas vérifié ici.** `PupilState::ALL`
  annonce huit états et `pupil::register` en couvre huit ; mais rien, à la
  compilation, n'empêche un métier futur d'en annoncer un de plus. Un
  `debug_assert` de couverture par `Machine::can_leave` (que les tests utilisent) est
  la seule garde.
