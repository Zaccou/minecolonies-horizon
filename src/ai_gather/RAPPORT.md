# RAPPORT — buts de récolte (`src/ai_gather`)

**État : `port-check ai_gather` → VERT, 89 tests passés, 0 échec.** Les seuls
avertissements restants du build pointent vers d'autres modules (`crafting`, `colony`,
`commands`, `research`, `persistence`), aucun vers `src/ai_gather/`.

## 1. Ce qui est porté

Quatre métiers, une machine par métier — la même que `crate::ai` (`Machine`,
`Transition`, `Action`, `Condition` : rien n'est réécrit) :

| Métier | Classe Java | États | Cibles | Seuil de vidage | Fichier |
| --- | --- | ---: | ---: | ---: | --- |
| bûcheron | `EntityAIWorkLumberjack` | 6 | 8 | 32 | `lumberjack.rs` |
| mineur | `EntityAIStructureMiner` | 6 | 9 | 64 × niveau | `miner.rs` |
| fermier | `EntityAIWorkFarmer` | 3 | 5 | 64 | `farmer.rs` |
| pêcheur | `EntityAIWorkFisherman` | 4 | 7 | 10 | `fisherman.rs` |

- `state.rs` : `GatherState`, un seul enum comme `AIWorkerState` — 19 états de métier +
  10 états de base (`Base(WorkerState)`), le drapeau `isOkayToEat`, et **30 refus nommés**
  (`NoTreeFound`, `NoWater`, `NoField`, `NoNode`, `ToolMissing`, `InventoryDumpDue`,
  `NoSapling`, `SuboptimalPond`, `RotationsExhausted`, `ShaftBlocked`, …) avec `java()` et
  `key()`.
- `facts.rs` : `WorldFacts` **réutilisé** (même valeur, rien recopié) + le demi-métier
  (`LumberjackFacts`, `MinerFacts`, `FarmerFacts`, `FishermanFacts`) + la mémoire
  (`delay`/`hasDelayed` réutilisés, `searchIncrement`, `timeWaited`, `checkedInHut`,
  `skippedState`, `workingOffset`/`cell`, `executedRotations`, `stuckCounter`,
  `startingLevelShaft`).
- `mod.rs` : `GatherAi`, `GatherTrade`, `register_handover` (la couture avec la table de
  base), `add_target`, `gather_action!`, `dump_due`, `at_hut`.
- Les trois fonctions « pures » qui portent les constantes : `sapling_step` (les trois
  sorties de `plantSapling`), `next_valid_cell` (la spirale du champ),
  `next_block_in_shaft` (le scan 7×7 du puits) ; plus `last_ladder`, `depth_limit`,
  `rotation_for`, `needs_securing`, `surrounding_ore_or`.
- Tirages : `draw_100`, `draw_int`, `draw_double`, `draw_pond` sont des faits — c'est ce
  qui rend les 89 tests possibles.

## 2. Comptes

| | |
| --- | ---: |
| Lignes du module (`.rs`) | 6 000 |
| dont `MANQUANT.md` + `RAPPORT.md` | +327 |
| Tests | **89** (verts) |
| Répartition | facts 7 · state 4 · lumberjack 15 · miner 19 · farmer 18 · pêcheur 15 · câblure 11 |

La consigne visait 1 200 à 1 800 lignes et 50 tests : le dépassement vient de deux
choses, la règle 3 (chaque règle porte sa ligne, donc chaque fonction a son en-tête) et
une **double écriture** — voir §5.

## 3. Ce que le compilateur et les tests ont tranché

Douze points où le code et l'attente ne disaient pas la même chose, et l'arbitrage :

1. **`f64: Eq`.** `FishermanFacts` contenait `draw_double`, donc `Eq` était impossible.
   `Eq` retiré de `FishermanFacts`, `GatherJob` et `GatherScene` (`PartialEq` suffit :
   aucune de ces structures n'est une clé de table, et `GatherState`, la seule clé qu'un
   `Machine` prend, est un enum d'entiers). C'est dit dans le code.
2. **`advance_ladder` : 2 ou 3 arguments ?** J'ai d'abord crus devoir l'aligner sur le
   macro (3). Faux : Java n'a **aucun `AITarget`** pour `advanceLadder` — c'est
   `doShaftMining` qui l'appelle (`:453-456`). Le `gather_action!` en trop a été supprimé,
   la fonction est revenue à 2 arguments, et le commentaire cite la ligne.
3. **E0501, la fermeture `add`.** Les quatre `register` avaient une fermeture qui tenait
   `machine` emprunté puis un `machine.add(...)` juste après. Une seule fonction
   `mod::add_target` remplace les quatre fermetures.
4. **La spirale ne recule pas.** `nextValidCell` (`:470-480`) compare
   `setCell(...) == getLargestCell()` : un second appel *après* le `null` dépasse la fin
   du champ pour toujours — en Java l'`int` déborde en silence, en Rust debug ça panique.
   Le port compare `>=` et sature l'indice. Divergence assumée et documentée : un champ
   déjà parcouru répond `None`.
5. **L'ordre de l'anneau.** Mes attentes étaient décalées d'un indice : l'indice 0 donne
   `(1, 1)`, l'indice 1 `(0, 1)`, l'indice 2 `(-1, 1)`. L'arithmétique de Java a gagné, et
   les tests le disent — dont le fait que **la cellule centrale n'est jamais travaillée**.
6. **`depth_limit` niveau 4.** `-100` est rogné par `getMinBuildHeight() + 5` = `-59` :
   `normalizeMaxDepth` (`BuildingMiner.java:205-208`) gagne. C'était mon test qui était
   faux (`-100` attendu).
7. **Le puits « profond » du test de shaft.** Le même rognage fait que la limite vaut
   `-59` : pour passer sous elle, la colonne d'échelles doit descendre sous `-59`, pas
   sous `-16`.
8. **`repairLadder` travaille à `lastLadder - 1`.** Colonne 59..60 → la réparation se fait
   en y=58, pas 59. Le code avait raison, le test regardait la mauvaise cellule.
9. **`advanceLadder` d'abord répare.** Il vérifie `ladderDamaged()` (`:477-480`) avant la
   dalle de sécurité : sans sol sous l'échelle, il part réparer. Le test devait donc
   déclarer ce sol.
10. **`doMining` et la chaîne de minerai.** Tant que la cible est encore du minerai dans
    les faits, la chaîne tient (`:283-286`) ; Java, lui, relit un bloc devenu air. La
    sortie `BUILDING_STEP` exige donc une cible non-minerai : c'est le contrat des faits,
    écrit dans le test.
11. **`sapling_step`.** La hutte est interrogée quand `timeWaited >= 25` **en entrée**,
    soit à 30 dans la suite 0→10→20→30 — pas à 20. Mon test attendait 20.
12. **Défoliation.** La branche feuille finit par `return getState()` (le tick ne bouge
    pas) alors que la branche bûche redemande l'état : c'est cette différence qui prouve
    l'ordre (`:554-562` avant `:563-596`), et le test l'utilise.
13. **Un test, une mémoire.** `the_field_stage_picks_the_state` réutilisait la même
    `GatherMemory` pour trois cas : la spirale ne revenant pas en arrière, le deuxième cas
    partait trop loin. Chaque cas a maintenant sa mémoire.

## 4. Prémisses fausses de la consigne

**a. Il n'y a pas de `EntityAIWorkMiner`.** Le mineur est `EntityAIStructureMiner`
(`production/EntityAIStructureMiner.java:64`), une IA de structure. Conséquence : trois
de ses états (`LOAD_STRUCTURE`, `BUILDING_STEP`, et `MINE_BLOCK` sauf `doMining`, qui est
surchargé) appartiennent à la table de structure que `crate::ai::builder` porte déjà sur
`WorkerState`. Le port les **rend** par la couture au lieu de porter deux fois la table.

**b. Les chemins de la consigne n'existent pas.** Ni `core/entity/ai/goals/` ni
`core/entity/ai/basic/` : les classes sont dans `core/entity/ai/workers/`
(`AbstractEntityAIBasic.java`, `AbstractAISkeleton.java`, `AbstractEntityAIStructure*.java`)
et `workers/production/`. Toutes les citations utilisent les vrais chemins.

**c. « Il manque les buts concrets » est vrai, le raccordement ne l'est pas gratuitement.**
`Machine<S, F>` est monomorphe et les `Action` sont des pointeurs de fonction : une table
de métier ne peut pas *contenir* la table de base, elle s'y raccorde par
`GatherState::Base(WorkerState)` + `register_handover`. Et `JobFacts` est un enum fermé
dans `src/ai/facts.rs` (hors de ma frontière d'écriture), donc `GatherScene` enveloppe
`WorldFacts` au lieu de l'étendre.

**d. `src/ai/MANQUANT.md` §1 est périmé** (« aucune source aléatoire ») : le SDK a
`levels::level_random -> Option<LegacyRandom>` (`levels.rs:606`) et `level_set_random`
(`:1520`). Les tirages-faits sont un choix de rejouabilité, pas un manque.

**e. Politique du dépôt MineColonies** (`minecolonies-src/AGENTS.md`) : elle vise le code
MineColonies et les addons qui le mixent. Ceci est un **port** vers un autre moteur, le
Java servant de spécification, sous GPL-3.0 ; rien n'est proposé en amont au projet.

## 5. Ce qu'il faut savoir sur la session

Un **autre agent a écrit dans ce dossier** pendant mon travail : `fisherman.rs` et
`tests.rs` ont été réécrits par lui, et `mod.rs` a reçu son correctif `$crate` dans
`gather_action!`. J'ai réconcilié : `entry()` vaut `Base(StartWorking)` pour les quatre
métiers (les quatre classes mères enregistrent `IDLE → START_WORKING`, chacune à sa ligne,
exposée par `entry_java()`), `dump_due` lit `&scene.job`, le pêcheur garde les branches
« debout dans l'eau / fond du pond » que les faits portent, et j'ai ajouté le test
« chaque état a une sortie ». C'est de là que vient le volume.

## 6. Ce qui reste dans `MANQUANT.md`

Sept entrées, chacune avec ce qui manque et le coût :

1. `JobFacts` fermé → `GatherScene` enveloppe `WorldFacts` (le correctif touche
   `src/ai/facts.rs`).
2. `crate::ai::facts::Block` ne sait pas exprimer un tag → second vocabulaire
   `GatherBlock`/`GatherBlocks`, et `isRightFarmLandForCrop` / `HOE_TILL` approchés par
   `Dirt`/`Farmland` (exact en vanilla, approximatif pour un mod qui ajoute une farmland).
3. `Machine` monomorphe → couture explicite au lieu d'une composition.
4. Les trois recherches (`walkToTree`, `PathJobFindWater`, `getWorkingPosition`) sont des
   jobs de pathfinding : faits, avec ce que le SDK offre à côté (`level_block_state`,
   `level_destroy_block`, `level_roll_loot`, `block_in_tag`, `mob_move_to_path`).
5. Tirages : disponibles (`level_random`), mais passés en faits pour la rejouabilité, avec
   `random_chance_bound` pour que la distribution reste celle de Java.
6. Les files de `Tree` (ordre d'insertion compris) restent chez l'appelant ; le port lit
   longueurs + tête.
7. Les trois moitiés non portées, nommées par un refus : craft du bûcheron/fermier
   (`CraftingNotPorted`), structure du mineur (`StructureTableOwned`), placement de
   blueprint d'un nœud.
