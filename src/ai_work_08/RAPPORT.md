# RAPPORT — les buts d'IA de `production/agriculture/` (fermier, pêcheur, fleuriste)

Agent `ai_work_08`. Sources Java : `EntityAIWorkFarmer.java` (921 lignes),
`EntityAIWorkFisherman.java` (733) et `EntityAIWorkFlorist.java` (416), dans
`core/entity/ai/workers/production/agriculture/`.

**État : `port-check ai_work_08` = VERT.** 53 tests, 0 échec, 0 avertissement dans ce
dossier.

---

## 1. Ce qui est écrit, fichier par fichier

| Fichier | Lignes | dont code | Ce qu'il porte |
| --- | ---: | ---: | --- |
| `mod.rs` | 371 | 214 | `AgriTrade` (les trois métiers), `AgriAi` (la façade sur deux moteurs), `AgriOutcome`, le doc du cycle |
| `state.rs` | 586 | 359 | `AgriState` (31 états), `Phase` (les 4 + `Wait`), `AgriWork`, `AgriDecision`, `AgriRefusal` (18 refus nommés) |
| `facts.rs` | 447 | 255 | `PlantGround`, `FloristFacts`, `GoalAnswers`, `AgriMemory`, `FloristScene`, `AgriScene` |
| `cycle.rs` | 432 | 268 | `phase_of`, `TradeThreshold`/`threshold`, `dump_due`/`inventory_needs_dump`, `Duration`/`durations`, `AgriTrace` |
| `florist.rs` | 660 | 388 | **la table du fleuriste** : les neuf constantes, `decide`/`compost`/`harvest`, les deux parcours de terres, `register`, `FloristAi` |
| `tests.rs` | 888 | 686 | 46 tests de scénario et de règle (+ 7 dans `state.rs`) |
| `MANQUANT.md` | 145 | — | les sept choses que ce module ne peut pas faire, avec la déclaration lue |
| `RAPPORT.md` | 210 | — | ce fichier |
| **total** | **3 739** | **2 170** | « code » = lignes hors commentaires et hors lignes vides |

**Le format demandé (900–1 400 lignes) est dépassé, et je préfère le dire.** Le module
fait 2 170 lignes de code pour 53 tests, contre 900–1 400 visés ; les modules frères sont
du même ordre (`ai_work_02` ≈ 4 900 lignes hors Markdown, `ai_work_01` ≈ 5 200). Ce qui
gonfle le volume, dans l'ordre : le style de documentation du dépôt (chaque règle porte sa
ligne Java, comme dans `ai_gather` et `ai`), les six états de base du fleuriste qui doivent
être enregistrés un par un (`Machine` ne sait pas hériter d'une table), et les scénarios de
machine, qui ont besoin de dix ticks chacun pour traverser un cycle complet.

---

## 2. Une prémisse du brief était fausse, et je ne l'ai pas contournée

Le brief demande de porter « une machine à états testable » pour les trois métiers. **Deux
des trois existaient déjà** :

- `crate::ai_gather::farmer` — la table de `EntityAIWorkFarmer.java:136-142`, ses faits,
  ses refus, `next_valid_cell`, `work_at_field`, `level_delay` ;
- `crate::ai_gather::fisherman` — la table de `EntityAIWorkFisherman.java:168-176`,
  `find_water`, `get_to_water`, `do_fishing`, `throw_or_retrieve_hook`.

Écrire ici `FARMER_HOE`, `FarmerFacts` et `workAtField` une seconde fois aurait été
exactement la duplication que la règle 6 interdit. Ce module **compose** donc :

- [`AgriAi::new`] répond `AgriAi::Gather(GatherAi)` pour le fermier et le pêcheur, et tique
  la machine existante sans la modifier ; `AgriOutcome` enveloppe son `GatherOutcome` et
  ne réécrit que les deux types d'état et de refus ;
- `AgriAi::Florist` est la seule machine neuve, parce que le fleuriste n'existait nulle
  part : `grep -rn Florist src/` ne rendait que des données de métier — le `Job`
  (`src/jobs/job.rs:105`), la hutte (`src/blocks/huts.rs:170`), l'emploi
  (`src/jobs/employment.rs:311`) et les besoins (`src/jobs/kinds/bois.rs`) —, aucune
  décision d'IA.

**Ce qui manquait réellement aux trois, et qui est ici :**

1. **La phase.** Java ne nomme pas chercher/aller/agir/rendre : il replie le déplacement
   dans l'état d'action (`if (!walkToSafePos(...)) { return getState(); }`,
   `EntityAIWorkFarmer.java:529-533`). Résultat : une trace d'exécution ne dit pas si le
   citoyen travaille ou s'il marche. `cycle::phase_of` lit les quatre phases sur
   `(état, réponses du tick)` — c'est une **lecture**, pas un second automate.
2. **La table du fleuriste**, complète (voir §3).
3. **Le cycle de vidage pour le fleuriste** : `inventoryNeedsDump` vit dans
   `crate::ai::worker` sur `WorkerState`, et `Machine<S, F>` est monomorphique — la machine
   du fleuriste ne peut pas hériter de cette table. `cycle::inventory_needs_dump` en
   restitue les cinq conditions avec le seuil du métier, citées à `:411-415`.
4. **La trace.** `AgriTrace` garde la suite `(état, phase)` et supprime les répétitions :
   sans elle, un test ne peut pas distinguer « le citoyen a agi » de « le citoyen a
   attendu vingt ticks ».

**Conséquence assumée** : pour le fermier et le pêcheur, ce module n'ajoute pas de
décision — il ajoute une lecture et une composition. Le détail de leurs règles reste dans
`crate::ai_gather`, testé là-bas.

---

## 3. La table du fleuriste (`florist.rs`), ligne par ligne

| # | État | Règle | Ticks | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | vers `START_WORKING` | 1 | `:129` |
| 2 | `START_WORKING` | vers `DECIDE` | 20 | `:130` |
| 3 | `DECIDE` | `decide` | 200 | `:131` |
| 4 | `FLORIST_HARVEST` | `harvest` | 20 | `:132` |
| 5 | `FLORIST_COMPOST` | `compost` | 20 | `:133` |
| 6 | *(événement `STATE_BLOCKING`)* | `inventoryNeedsDump` → `INVENTORY_FULL` | 100 | `AbstractEntityAIBasic.java:225` |
| 7 | `INVENTORY_FULL` | `dumpInventory` | 20 | `AbstractEntityAIBasic.java:226` |
| 8 | `GATHERING_REQUIRED_MATERIALS` | `getNeededItem` | 5 | `AbstractEntityAIBasic.java:242` |
| 9 | `PAUSED` | `bePaused` | 20 | `AbstractEntityAIBasic.java:251` |

`decide` (`:151-207`) suit l'ordre de Java, et l'ordre compte : terres vides → ciseaux
(`:160-163`, `:307-321`) → distance à la hutte (`:166-170`) → compost dans la hutte
(`:172-180`) → requête de seize (`:181-184`) → fleur à cueillir (`:187-191`) → aucune terre
compostée (`:193-198`) → première terre nue (`:202-206`).

Les neuf constantes de Java portent leur ligne : `MAX_DISTANCE` `:50`, `HARVEST_ACTIONS_TO_DUMP`
`:55`, `PERCENT_CHANGE_FOR_GROWTH` `:60`, `BASE_XP_GAIN` `:65` (**déclarée et jamais lue
par Java** — le port la garde citée et ne s'en sert pas), `COMPOST_REQUEST_QTY` `:70`,
`BASE_BLOCK_MINING_DELAY` `:75`, `PER_LEVEL_BONUS` `:80`, `MAX_BONUS` `:85`,
`XP_PER_FLOWER` `:96`, plus les cinq taux `:129-133`.

**Trois nombres sont calculés, pas recopiés** : `actions_until_dumping` (`10 × niveau`,
`:324-327`), `growth_chance` (`0,2 − niveau × 0,01`, `:236`) et `mining_time`
(`:336-340`, avec la troncature **à l'intérieur** de la multiplication comme en Java :
60 ticks à dextérité 0, 50 à 10, 10 au plafond).

---

## 4. Ce que les buts demandent au monde, en faits

Deux entrées, deux sorties, jamais un appel au monde.

**Entrée — `GoalAnswers`** (ce que le tick précédent a répondu) :

| Champ | Java | Ligne |
| --- | --- | --- |
| `at_target` | `walkToWorkPos` / `walkToSafePos` | `EntityAIWorkFlorist.java:223`, `:265` ; `EntityAIWorkFarmer.java:530` |
| `at_building` | `walkToBuilding()` | `EntityAIWorkFlorist.java:167` |
| `mined` | `mineBlock(...)` | `EntityAIWorkFlorist.java:272` ; `AbstractEntityAIInteract.java:135-149` |
| `transferred` | fin de `dumpInventory` / `getNeededItem` | `AbstractEntityAIBasic.java:1213-1222`, `:323-326` |
| `tool_broken` | **rien** — Java ne distingue pas « cassé » de « jamais eu » | `:294-298` (casse) vs `:309` (requête) |

**Entrée — `FloristFacts`** : `building_level` (`:236`), `plant_ground` (`:154`),
`tool_requested` (`:309`), `shears_slot` (`:314`), `compost_in_inventory` (`:172`),
`compost_in_building` (`:175`), `flower_to_grow` (`:231`), `primary_skill` (`:234`),
`secondary_skill` (`:339`), `max_equipment_level` (`:314`).

**Sortie — `AgriWork`** : `RequestCompost { quantity }` (`:183`), `Compost { position,
flower, chance }` (`:236`), `Harvest { position, experience }` (`:272-282`),
`RemovePlantableGround { position }` (`:381`). Une décision peut en porter plusieurs : Java
crée la requête **et** part cueillir dans le même tick (`:183` puis `:187-191`).

**Sortie — `AgriRefusal`**, 18 refus nommés avec leur ligne : `NoPlantGround` (`:154`),
`ShearsMissing` (`:160`), `NotAtHut` (`:167`), `CompostInHut` (`:176`), `CompostRequested`
(`:183`), `NoCompost` (`:195`), `NoGroundToCompost` (`:216`), `GroundGone` (`:228`),
`NotCompostedDirt` (`:229`), `NoFlowersInConfig` (`:231`), `CompostGoneWhileComposting`
(`:234`), `NoFlowerToHarvest` (`:258`), `NotAtTarget` (`:223`), `MiningIncomplete`
(`:272`), `TransferIncomplete` (`AbstractEntityAIBasic.java:1230`), `ToolBroken`
(`:294`), `SceneForAnotherTrade`, plus `Gather(...)` qui réutilise les 30 refus déjà
nommés par `crate::ai_gather` pour le fermier et le pêcheur.

**« Inventaire plein » n'est pas un refus de `decide`, et c'est Java qui le dit.** Le
brief le range parmi les refus nommés ; dans `AbstractEntityAIBasic.java:225`, c'est un
événement `STATE_BLOCKING` qui **interrompt** l'état courant avant que `decide` ne tourne :
le fleuriste ne peut donc jamais « refuser pour inventaire plein ». Ce qui est nommé ici,
c'est l'autre moitié : `TransferIncomplete` (`:1230`, le vidage n'est pas fini) pour le
fleuriste, et `AgriRefusal::Gather(GatherRefusal::InventoryDumpDue)`
(`AbstractEntityAIBasic.java:409-416`) pour le fermier, où `ai_gather` le produit
réellement (`farmer::decide`). Rien n'est inventé pour cocher la case.

Aucun `unwrap()` : les scans de terres rendent un `Result`-like nommé
(`shears_ready -> Result<(), AgriRefusal>`), les `let … else` portent tous un refus nommé,
et le seul `expect` du module est dans un utilitaire de test.

---

## 5. Les 53 tests

Ce qu'ils mesurent, et pas seulement qu'ils passent :

- **7 sur le vocabulaire** (`state.rs`) : les 31 états, le drapeau `isOkayToEat` contre
  `AIWorkerState.java:15`, `:23`, `:76`, `:585`, `:590`, les 6 états qui agissent, les
  ordres multiples, la ligne de chaque refus.
- **5 sur les phases** : marcher/agir sur le même état, `FISHERMAN_WALKING_TO_WATER`,
  les deux transferts, `PAUSED`/`NEEDS_ITEM`, et les 31 états qui ont tous une phase.
- **5 sur les terres** : `pos.above()` (`:352-355`), le retrait des blocs qui ne sont pas
  des terres aménagées (`:381`, `WorkerUtil.java:326`), les chunks déchargés (`:369`), le
  premier non composté (`:374`), l'arrêt du scan au premier trouvé.
- **7 sur les seuils et les durées** : `10 × niveau` (`:326`), les deux constantes du
  fermier et du pêcheur reprises de `ai_gather`, les sept cibles du fleuriste avec leurs
  ticks, la troncature de `mining_time` (6 valeurs), la perte de 1 % de chance par niveau,
  et les huit états de la table qui ont tous une sortie (`can_leave`).
- **9 sur `decide`** : les branches, chacune avec son refus et son état de retour.
- **6 sur `compost`** : marcher d'abord, la chance de niveau 4 (0,16), pas de fleur
  configurée, plus de compost, bloc non compostable (Java compte quand même une action),
  terre disparue, aucune terre retenue.
- **4 sur `harvest`** : les deux passages de `mineBlock`, la marche, la position oubliée,
  les ciseaux cassés.
- **10 sur les scénarios et la composition** : la séquence `START_WORKING → DECIDE →
  FLORIST_HARVEST`, le cycle complet chercher → aller → agir → rendre, l'interruption par
  le vidage et le retour à `IDLE`, la pause, les deux erreurs de scène croisée, un ordre qui
  ne vit qu'un tick, la recomposition du fermier et du pêcheur.

### Un test a échoué, et c'est le Java qui l'a tranché

`a_harvest_scenario_searches_walks_acts_then_searches_again` attendait la suite
`START_WORKING, DECIDE, FLORIST_HARVEST, START_WORKING`. La machine en a produit cinq pas,
avec `FLORIST_HARVEST` **deux fois** : la phase passait de `Travel` à `Act` sans que
l'état change. C'est exactement ce que fait Java (`:265-268` : `walkToWorkPos` faux →
`return getState()`), donc **c'est mon attente qui était fausse**, pas le code. Le test a
été corrigé en citant la ligne, et il vérifie maintenant que l'état se répète pendant que
la phase change — c'est-à-dire la raison d'être de `Phase`.

---

## 6. Ce que ce module ne couvre pas

- **Les décisions du fermier et du pêcheur** : elles restent dans `crate::ai_gather`, avec
  leurs propres tests. Ici elles sont composées, pas réécrites.
- **Le contenu** : `blockCompostedDirt` et `TileEntityCompostedDirt` (le bloc, la part, la
  croissance) ne sont pas dans le port — voir `MANQUANT.md` §2. Rien ne fait pousser de
  fleur aujourd'hui.
- **La liste de terres de la hutte** : `BuildingFlorist.getPlantGround()` n'existe pas
  encore dans `src/building/` (`MANQUANT.md` §3) ; elle arrive par les faits.
- **Les interactions joueur** (`NO_PLANT_GROUND_FLORIST`, `NO_COMPOST`,
  `NO_FLOWERS_IN_CONFIG`) : nommées par une clé, affichées par personne (`MANQUANT.md` §4).
- **Le câblage** : aucun hook n'appelle `AgriAi` — `src/lib.rs` n'est pas à cet agent. La
  façade est prête : un appelant fournit `WorldFacts` + les faits du métier, appelle
  `tick(scene, answers)` et exécute les `AgriWork` rendus.

---

## 7. Vérification

```
$ ssh deb '/home/zaccou/bin/port-check ai_work_08'
test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 2498 filtered out
port-check: VERT — tu peux terminer.
```

Deux corrections ont été nécessaires avant ce vert, toutes deux dans mon dossier :
un helper de test nommé `world()` masqué par une variable locale (`E0618`), et trois
fonctions de `florist` appelées sans leur module dans les tests. Les erreurs `ai_work_09`
apparues entre-temps venaient d'un autre agent et n'ont pas été touchées.
