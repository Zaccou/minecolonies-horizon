# `src/ai` — la sélection de buts d'un travailleur

Sept fichiers de Rust, **6 457 lignes** : **2 762** de code, **1 412** de commentaires, **265** de
blancs, **2 018** de tests. **106 tests** dans `ai::`, tous verts (165 dans la crate de vérification,
qui inclut `jobs` et `citizens`).
Aucune ligne écrite hors de `src/ai/`. Aucun import de `mod_api` : le module ne lit ni n'écrit le
monde, il décide.

| Fichier | Lignes | Rôle |
| --- | ---: | --- |
| `mod.rs` | 102 | la façade : ce que le module décide, et la couture avec `citizens::priority` |
| `state.rs` | 1 103 | la machine : passes, transitions, cadence, piège d'état |
| `facts.rs` | 945 | ce que l'IA a le droit de lire : `WorldFacts`, en données |
| `worker.rs` | 2 055 | la table de base : les 14 cibles de `AbstractEntityAIBasic` |
| `composter.rs` | 683 | un métier simple : 6 cibles, deux scans de barils |
| `builder.rs` | 1 091 | un métier complexe : 9 cibles, 8 étapes, 6 sorties de pas |
| `tests.rs` | 478 | les tests de bout en bout (séquence, rejeu, couture, trappe) |
| `MANQUANT.md` | — | les 5 manques d'API rencontrés, avec ce qui a été fait à la place |

---

## 1. Ce qui est porté — compte exact

### 1.1 La machine (`state.rs`)

| Java | Ce qui est porté |
| --- | --- |
| `BasicStateMachine` | les 4 passes (`AI_BLOCKING`, `EVENT`, `STATE_BLOCKING`, puis la liste de l'état courant), premier tir gagnant, `null` = « je continue le tick », la trappe d'état (`:220-228`), `reset` |
| `TickRateStateMachine` | `tick_rate`, `slownessFactor = 1.0`, `setCurrentDelay`, compteurs par transition |
| `TickingTransition` | `tick_rate` borné à `1..12 000`, décompte par le taux de la machine |
| `addTransitionGroup` | une transition de groupe passe **devant** chaque liste d'état |
| `AIWorkerState` | **18 des 127 états**, avec leur drapeau `isOkayToEat` (`canBeInterrupted`) |

**Écarté, cité :** `tickOffsetVariant` (étalement du premier tick sur 50, pur chargement — une statique
cachée rendrait un test dépendant du nombre de transitions construites avant lui) et `slownessFactor`
autres que `1.0`.

### 1.2 La table de base (`worker.rs`)

Les **14** cibles de `AbstractEntityAIBasic.java:200-260`, dans l'ordre Java, avec leur cadence :
`initSafetyChecks` (1), `holdInInit` (1), `updateVisualState` (20), `waitingForSomething` (5),
`inventoryNeedsDump` (100), `dumpInventory` (20), `checkIfNeedsItem` (20), `cleanAsync` (200),
`waitForRequests` (40), `getNeededItem` (20), `shouldRestart` (20), `unpause` (20), `bePaused` (10),
`isStartingPaused` (20).

Avec elles : **15 constantes** (`STANDARD_DELAY` 5, `REQUEST_DELAY` 60, `PICKUP_ATTEMPTS` 10,
`WALK_DELAY` 20, `NO_TOOL` -10, `TOOL_NOT_FOUND` -1, `ACTIONS_UNTIL_DUMP` 32, `MIN_OPEN_SLOTS` 5,
`CHANCE_TO_DUMP_50` 16, `CHANCE_TO_DUMP` 8, `DELAY_RECHECK` 10, `EXCEPTION_TIMEOUT` 100,
`DEFAULT_RANGE_FOR_DELAY` 4, `WOKR_IN_BUILDING_DIST` 7, `TOOL_LEVEL_WOOD_OR_GOLD` 0), **20
`Refusal`** nommées, et les méthodes pures :
`dumpOneMoreSlot`, `lookForRequests`, `getNeededItem`, `afterDump`, `checkForTool`,
`bestToolSlot`, `effectiveSkillLevel`, `hasNotDelayed`, `constructorCheck`.

### 1.3 Les deux métiers

| Métier | Cibles | Où |
| --- | ---: | --- |
| Composter | **6** | `EntityAIWorkComposter.java:95-101` |
| Builder | **9** | `AbstractEntityAIStructure.java:166-200` |

Plus les décisions pures qui vont avec : `decide_what_to_do` (les deux scans de barils),
`get_materials`, `fill_barrels`, `harvest_barrels`, `loot_multiplier`, `barrel_acceleration_ticks`,
`step_outcome` (les 6 sorties d'un pas de structure), `mining_decision` (les 7 sorties de `doMining`),
`check_for_work_order`, `check_if_canceled`, `pick_up_material`, `complete_build`,
`structure_check_if_needs_item`, et les trois tableaux d'étapes (`2`/`6`/`8` étapes, `2`/`5`/`6`
annoncées).

### 1.4 Ce que le module rend

Un `tick(scene) -> Outcome` :

```rust
pub struct Outcome {
    pub state: WorkerState,              // le but en cours
    pub fired: Option<Fired<WorkerState>>, // la règle, sa ligne Java, l'état d'avant
    pub refusal: Option<Refusal>,        // pourquoi il n'avance pas, nommé
}
```

C'est la demande du brief : **le prochain but, ou une raison de ne rien faire**. Aucun des deux n'est
inventé : `state` vient de la machine, `fired.java` d'un `fichier.java:ligne` de la table, `refusal`
d'une branche qui, en Java, fait `return getState()`.

---

## 2. Ce qui est écarté, et ce que chaque écart coûte

| Écarté | Pourquoi | Coût |
| --- | --- | --- |
| **47 des 49 métiers** (`JobAi::for_job` → `Err(Unported)`) | 29 780 lignes d'IA Java ; chaque métier a sa propre table de cibles | Un `JobAi::for_job(Job::Farmer)` **refuse** au lieu de deviner ; `JobAi::base(job)` donne la moitié réelle (voir §3) |
| **Les buts eux-mêmes** (marcher, casser, poser, ouvrir un coffre, tirer d'un baril) | ils demandent le niveau ; c'est ce que `WorldFacts` remplace | le module décide, l'appelant agit (voir `MANQUANT.md` §2-4) |
| **Le système de requêtes** | contenu, pas moteur ; `crate::requests` le possède | `ToolCheck::Asked`/`MaterialsOutcome::Asked` = « il faut demander », la création reste ailleurs |
| **Le tirage aléatoire** | aucune source aléatoire dans le SDK, et un aléa rend la décision non rejouable | deux faits (`coin`, `roll`), voir `MANQUANT.md` §1 |
| **Le blueprint** | `crate::building` + Structurize | l'appelant rapporte un `Placement`, le module en tire l'état suivant |
| **Les messages** (`triggerInteraction`) | effets sur un joueur | remplacés par `Refusal::key()` (`ai.refusal.*`) |
| **La décision citoyen** (manger, dormir, fuir) | **déjà portée** par `crate::citizens::priority` (14 barreaux) | la couture est testée : `can_go_idle` décide si `CitizenState::Work` est pris (`CitizenAI.java:250-260`) |
| **`tickOffsetVariant`, `slownessFactor`** | voir §1.1 | aucun : ce sont des réglages de charge, pas des règles |

---

## 3. Ce que « base seulement » veut dire, et pourquoi ça refuse

`JobAi::base(job)` enregistre les 14 cibles de `AbstractEntityAIBasic` et **rien d'autre**. Une
machine construite là-dessus ne peut pas quitter `IDLE` : `AbstractEntityAIBasic` ne déclare aucune
transition pour `IDLE`, chaque IA concrète la sienne (`EntityAIWorkComposter.java:97`,
`AbstractEntityAIStructure.java:186`). C'est exactement la raison pour laquelle la classe Java est
`abstract`, et le port le rend visible : la machine signale la trappe (`Fired::reset`,
`BasicStateMachine.java:220-228`) au lieu de tourner à vide.

**Les 47 métiers qui n'ont pas de table** (tout sauf Builder, Composter et le `Placeholder`, qui ne
travaille pas), dans l'ordre de `crate::jobs::job::Job` (50 variantes, `src/jobs/job.rs`) : Alchemist,
ArcherTraining, Baker, Beekeeper, Blacksmith, Cavalry, Chef, ChickenHerder, CombatTraining,
ConcreteMixer, Cook, Cowboy, Crusher, Deliveryman, Druid, Dyer, Enchanter, Farmer, Fisherman,
Fletcher, Florist, Glassblower, Healer, Huscarl, Knight, Lumberjack, Marksman, Mechanic, Miner,
NetherWorker, Planter, Pupil, Quarrier, RabbitHerder, Ranger, Researcher, Sawmill, Shepherd, Sifter,
Smelter, Stablemaster, StoneMason, StoneSmeltery, Student, SwineHerder, Teacher, Undertaker.

---

## 4. Les références Java utilisées

Toutes vérifiées : un script relit chaque `xxx.java:NNN` du module et confirme que le fichier existe
dans `/workspace/minecolonies-src` et que la ligne existe (**151 références distinctes, 0 hors
bornes**). Les fichiers lus :

| Fichier | Ce que j'y ai pris |
| --- | --- |
| `core/entity/ai/workers/AbstractAISkeleton.java` | tick, `getState`, `canBeInterrupted` (`:126`), `resetAI` |
| `core/entity/ai/workers/AbstractEntityAIBasic.java` | **la table entière** (`:200-260`), `initSafetyChecks`, `waitingForSomething`, `inventoryNeedsDump`, `dumpInventory`/`dumpOneMoreSlot`, `checkIfNeedsItem`, `lookForRequests`, `getNeededItem`, `afterDump`, `checkForToolOrWeapon`, `getMostEfficientTool`, les deux courbes de compétence, `hasNotDelayed`, `onException`, `canGoIdle` |
| `core/entity/ai/workers/AbstractEntityAIInteract.java` | `mineBlock` (`:151-243`) : air/bedrock, compteur d'actions |
| `core/entity/ai/workers/AbstractEntityAIStructure.java` | les 9 cibles, `structureStep` (les 6 sorties), `doMining` (les 7 sorties), `pickUpMaterial`, `completeBuild`, `startBuilding`, `isThereAStructureToBuild`, les 3 tableaux d'étapes |
| `core/entity/ai/workers/AbstractEntityAIStructureWithWorkOrder.java` | `loadRequirements`, `checkIfCanceled`, l'override `checkIfNeedsItem` |
| `core/entity/ai/workers/builder/EntityAIStructureBuilder.java` | `checkForWorkOrder`, `startWorkingAtOwnBuilding`, `isAfterDumpPickupAllowed`, `afterDump`, `afterRequestPickUp`, `canGoIdle`, `ACTIONS_UNTIL_DUMP = 4096` |
| `core/entity/ai/workers/production/agriculture/EntityAIWorkComposter.java` | les 6 cibles, `decideWhatToDo`, `getMaterials`, `fill`/`harvest`, `getLootMultiplier`, `accelerateBarrels`, `complain` |
| `api/entity/ai/statemachine/**` | `BasicStateMachine`, `BasicTransition`, `TickRateStateMachine`, `TickingTransition`, `AITarget`, `AIEventTarget`, `AIBlockingEventType`, `TickRateConstants` |
| `api/entity/ai/statemachine/states/AIWorkerState.java` | les 18 états et leur `isOkayToEat` |
| `api/entity/ai/JobStatus.java`, `api/util/constant/CitizenConstants.java`, `Constants.java`, `EquipmentLevelConstants.java` | les constantes nommées |
| `core/entity/ai/workers/util/BuildingProgressStage.java`, `BuildingStructureHandler.java` | les 10 étapes, `nextStage`, `isStageWork` |
| `api/util/EntityUtils.java`, `BlockPosUtil.java`, `ItemStackUtils.java` | `isLivingAtSite` (`:301-305`), `distSqr`/`getDistance2D`, `verifyEquipmentLevel` (`:264-271`) |
| `core/colony/buildings/AbstractBuilding.java` | `hasCitizenCompletedRequestsToPickup` (`:1565-1579`) |
| `core/entity/ai/workers/CitizenAI.java` | `:92-104` (le tick du métier) et `:250-260` (`canGoIdle`) — pour la couture seulement |
| `api/entity/citizen/AbstractEntityCitizen.java` | `ENTITY_AI_TICKRATE = 5` (`:64`) |

---

## 5. Les prémisses impossibles, dites au lieu d'être contournées

1. **`EntityAIWorker.java` n'existe pas.** Le brief le cite en premier. Dans l'arbre, la « classe de
   base des travailleurs » est **deux** classes : `AbstractAISkeleton` (le tick et la machine) et
   `AbstractEntityAIBasic` (les 14 cibles). `grep -rn "class EntityAIWorker|EntityAIComposter|EntityAIBuilder"`
   sur tout le dépôt Java ne rend **rien** (0 résultat, exécuté).
2. **`EntityAIComposter.java` s'appelle `EntityAIWorkComposter.java`**, et il est dans
   `workers/production/agriculture/`, pas dans `workers/`.
3. **`EntityAIBuilder.java` s'appelle `EntityAIStructureBuilder.java`** (`workers/builder/`), et la
   moitié de sa logique est dans ses deux parents (`AbstractEntityAIStructure`,
   `AbstractEntityAIStructureWithWorkOrder`), dont l'un fait 1 168 lignes à lui seul. Le brief
   annonçait « un métier complexe » : c'en est trois, plus un placeur de blueprint qui n'est pas dans
   ce dépôt (Structurize).
4. **Le brief parle de « 50 tests minimum » et « 1 500 à 2 200 lignes »** : 106 tests livrés ;
   pour les lignes, voir §6 — le total dépasse, le code seul (`2 758`) est déjà au-dessus.
5. **La priorité n'est pas une liste de `if`.** Le brief demande « la priorité entre buts, avec la
   règle Java exacte (ordre, seuils, verrous) ». Il n'y a pas de liste de priorités à transcrire :
   la priorité **est** l'ordre d'enregistrement dans quatre passes, et la cadence de chaque cible.
   C'est ce que `state.rs` porte, et ce que `worker.rs` cite ligne par ligne. Un « ordre de buts »
   écrit à la main aurait été une invention.
6. **`cleanAsync` ne peut jamais faire transitionner sa cible.** Sa condition est
   `hasCitizenCompletedRequests(...) && this.cleanAsync()`, et `cleanAsync` finit par `return false`
   (`AbstractEntityAIBasic.java:715`) après avoir accepté les requêtes asynchrones. La cible existe
   donc pour son **effet**, pas pour son état. Portée telle quelle, avec un test qui le dit.
   Idem pour `updateVisualState`, qui rend toujours `null` (`:472-479`) : elle rafraîchit l'étiquette,
   elle ne décide rien.

---

## 6. Volume : le compte exact, et ce qui serait coupable

| | Lignes |
| --- | ---: |
| Code (hors commentaires, hors blancs, hors tests) | **2 762** |
| Commentaires (références Java, tables, raisons) | **1 412** |
| Blancs | 265 |
| Tests (`tests.rs` + les `mod tests` des cinq fichiers) | **2 018** |
| **Total** | **6 457** |

Le brief annonçait 1 500-2 200 lignes. Le code seul les dépasse de 25 %, et avec ses commentaires le
module fait presque le triple. Ce qui est livré et qui n'était pas demandé nommément :
`state.rs` (la machine générique, 748 l.) — sans elle il aurait fallu deux machines ad hoc, une par
métier ; les 20 `Refusal` nommées avec leur `key()` ; `facts.rs` en entier, qui est la contrepartie
directe de « `WorldFacts` est une struct que tu définis ». Si le volume doit baisser, l'ordre de coupe
est : les trois tableaux d'étapes (`Stages`/`OrderKind`, ~120 l.), `getLootMultiplier` et
`barrelAcceleration` (~90 l.), `best_tool_slot` (~50 l.) — aucun n'est sur le chemin d'une décision de
la table de base.

---

## 7. Vérification — ce que j'ai réellement exécuté

1. **Crate isolée** (`/tmp/ai-check`, hors dépôt) : `src/ai` + `src/jobs/{job,tool,employment}.rs` +
   `src/citizens/{needs,priority,state,employment}.rs` + `src/building/kind.rs`, copiés **verbatim**,
   compilés par `rustc --edition 2024 --test`.
   → **165 tests verts, 0 échec**, dont **106** dans `ai::`. La crate temporaire a été supprimée
   après la vérification (4).
   Raison de l'isolement : à plusieurs reprises l'arbre n'était pas compilable à cause d'autres
   agents (`src/jobs/kinds/mod.rs` incomplet, puis `src/network/common.rs` en cours d'écriture).
2. **`cargo check --lib` dans le dépôt** (hôte Debian, `cargo 1.100`, `--offline`) : passe, avec mon
   module dedans.
3. **Références Java** : script qui relit les 151 `fichier.java:ligne` du module et vérifie que le
   fichier et la ligne existent → 0 hors bornes. Les 6 premières erreurs trouvées ont été corrigées
   (`BasicStateMachine` 311 l. et non 500, `AITarget` 71 l., `AIEventTarget` 47 l.,
   `TickingTransition` 128 l., `AIWorkerState` décalé de 4, `WithWorkOrder.java` mal nommé).
4. **`cargo test --lib` dans le dépôt** (cargo 1.100, `--offline`), une fois l'arbre revenu vert à la
   fin de la session : **718 tests verts, 0 échec**, dont **106** dans `ai::`. C'est la vérification
   qui compte : elle compile mon module **avec** `crate::citizens`, `crate::jobs` et le reste, et elle
   inclut le test de couture avec `citizens::priority`.

---

## 8. Ce que je ne prétends pas

- **Je n'ai pas mesuré le comportement en jeu.** Aucun de ces chemins n'a tourné dans Horizon : ils
  tournent sur des faits écrits à la main. La question « le composteur va-t-il vraiment à son baril »
  n'est pas tranchée par 106 tests ; la question « la décision est-elle celle du Java » l'est.
- **Les 47 autres métiers ne sont pas « presque faits ».** Chacun a sa propre table ; certains
  (guard, deliveryman, miner) reposent sur des machines à états qui n'ont pas la même forme.
- **`JobProfile` est une simplification assumée** de l'héritage Java : six méthodes surchargées
  deviennent six champs. C'est fidèle pour les deux métiers portés, et ça ne dit rien des autres.
