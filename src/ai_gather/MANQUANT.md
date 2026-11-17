# MANQUANT — ce que `src/ai_gather` voulait faire et qui n'était pas possible

Écrit pour les quatre buts de récolte (bûcheron, mineur, fermier, pêcheur). Chaque entrée
dit **ce que je voulais faire**, **ce qui existe réellement** (avec la déclaration lue),
**ce que j'ai fait à la place**, et **ce que ça coûte**. Rien ici n'est contourné en
silence : le module ne compile que sur `core` et `crate::`, il n'importe pas `mod_api`.

Les déclarations citées sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé `4dc6455d`).

---

## 1. La moitié « métier » de `WorldFacts` est un enum fermé

**Voulu.** Poser les faits du bûcheron, du mineur, du fermier et du pêcheur **dans**
`WorldFacts`, à côté de ceux du composter et du builder, pour qu'un seul type décrive
toutes les scènes.

**Ce qui existe.** `crate::ai::facts::JobFacts` a exactement deux variantes, `Composter`
et `Builder`, et un `Unported` (`src/ai/facts.rs:778-788`). Ce fichier est hors de la
frontière d'écriture de cet agent (règle 1 : « tu n'écris que dans `src/ai_gather` »).

**Fait à la place.** `GatherScene` **enveloppe** `WorldFacts` au lieu de l'étendre :

```rust
pub struct GatherScene { pub facts: WorldFacts, pub job: GatherJob, pub memory: GatherMemory, pub refusal: … }
```

`facts` est la **même valeur** que celle que `crate::ai` consomme : un appelant qui fait
tourner les deux tables remplit `WorldFacts` une fois et le passe aux deux. Rien n'est
recopié.

**Coût.** Deux types de scène au lieu d'un. Le jour où l'on voudra une scène unique, il
suffira d'ajouter quatre variantes à `JobFacts` — mais c'est une modification de
`src/ai/facts.rs`, donc une décision d'orchestrateur, pas la mienne.

---

## 2. Le vocabulaire de blocs ne sait pas exprimer un tag

**Voulu.** Réutiliser `crate::ai::facts::Block` et `Blocks` tels quels.

**Ce qui existe.** `Block` (`src/ai/facts.rs:89-102`) répond à cinq questions :
air / fluide / solide / barrel / indestructible. Ce sont celles du builder et du
composter. Les règles de récolte demandent **des tags** :

- bûcheron : `BlockTags.LOGS` (`EntityAIWorkLumberjack.java:271`), `LEAVES` (`:683`),
  `SAPLINGS` (`:652`), `Blocks.NETHERRACK` (`:926`) ;
- mineur : `CompatibilityManager.isOre` (`EntityAIStructureMiner.java:266`),
  `state.isLadder` (`:366`), `Blocks.TORCH` (`:763`) ;
- fermier : `BlockTags.DIRT` / `Blocks.FARMLAND` (`EntityAIWorkFarmer.java:382`),
  `CropBlock` / `StemBlock` / `BlockScarecrow` (`:380-384`, `:728-732`),
  `Blocks.PUMPKIN` / `MELON` (`:792`).

Un enum à cinq valeurs ne peut pas les porter, et `src/ai/facts.rs` est hors frontière.

**Fait à la place.** `GatherBlock` (16 variantes) et `GatherBlocks`, la même forme de
carte que `Blocks` (position non rapportée = air, position marquée non chargée ≠ air,
`is_loaded` avant toute règle qui s'en soucie).

**Coût.** Deux vocabulaires de blocs et deux cartes dans une même scène. La fusion
correcte est une carte **générique** sur le type de bloc, ou un `Block` qui porte les
tags ; les deux touchent `src/ai/facts.rs`.

**Conséquence visible.** `isRightFarmLandForCrop` (`EntityAIWorkFarmer.java:643-653`)
distingue la farmland d'un `ItemCrop` de `Blocks.FARMLAND`. Dans ce vocabulaire les deux
sont `Farmland`, donc `is_right_farmland_for_crop` répond « est-ce de la farmland ? ». Un
mod qui ajoute une troisième farmland rendrait la règle fausse ; en vanilla elle est
exacte. Même remarque pour `getToolModifiedState(HOE_TILL)` (`:407-411`) : le port teste
`Dirt`, ce qui est exactement l'ensemble que la houe bêche en vanilla.

---

## 3. `Machine<S, F>` est monomorphe : on ne peut pas la composer

**Voulu.** Poser les tables de récolte **sur** la table de base, comme Java : le
constructeur de `AbstractEntityAIBasic` enregistre quatorze cibles
(`AbstractEntityAIBasic.java:200-260`), puis celui du bûcheron en ajoute huit
(`EntityAIWorkLumberjack.java:193-201`) dans **la même** machine.

**Ce qui existe.** `crate::ai::state::Machine<S, F>` (`src/ai/state.rs:516-533`) est
générique sur l'état **et** sur les faits, et
`crate::ai::worker::register_base` ne compile que pour
`Machine<WorkerState, Scene>` (`src/ai/worker.rs:790`). Les `Action` sont des pointeurs
de fonction `fn(&mut F, S) -> Option<S>` : on ne peut pas les adapter (une fonction ne se
réécrit pas en closure) ni les stocker dans une machine d'un autre type.

**Fait à la place.** Une machine par métier, sur un état unique `GatherState` (fidèle à
`AIWorkerState`, qui est un seul enum pour tous les métiers), et une **couture explicite** :
`GatherState::Base(WorkerState)` pour les états de base, et `register_handover` qui dit
où le contrôle revient (`IDLE` → l'entrée du métier, `INVENTORY_FULL`/`NEEDS_ITEM`/
`PAUSED`/`GATHERING_REQUIRED_MATERIALS` → `IDLE`, chacune avec sa ligne).

**Coût.** Un appelant qui veut le comportement complet fait tourner deux machines : celle
de `crate::ai` pour `INIT`/dump/requêtes/pause, et celle-ci pour le métier. Les états de
couture sont le point de rendez-vous. La correction propre est une machine dont l'état
est un enum partagé (`enum AiState { Base(WorkerState), Job(GatherState) }`) ou un
`Machine` générique sur un trait d'état — les deux touchent `src/ai/state.rs`.

**Ce que ça donne déjà.** `GatherAi::new(trade)` démarre en `IDLE` (là où
`initSafetyChecks` arrive, `AbstractEntityAIBasic.java:446-465`), et chaque table de
métier est complète pour ses propres états.

---

## 4. Les trois « recherches » de Java sont des métiers de pathfinding

**Voulu.** Porter `walkToTree` / `PathJobFindWater` / le placeur de blueprint du mineur,
c'est-à-dire la partie de l'IA qui **trouve** un arbre, une pièce d'eau ou une position
de travail.

**Ce qui existe.** Le SDK sait lire et écrire le monde :
`levels::level_block_state(level, pos) -> Option<BlockState>` (`levels.rs:304`),
`levels::level_is_loaded` (`:347`), `levels::level_set_block` (`:902`),
`levels::level_destroy_block` (`:943`, avec butin et outils),
`levels::level_roll_loot` (`:1129`), `tags::block_in_tag(block, tag)` (`tags.rs:20`), et
déplacer une entité avec `mobs::mob_move_to_path` (`mobs.rs:319`).

Ce qui n'existe **pas**, c'est une recherche de zone : « donne-moi le bloc le plus proche
qui satisfait un prédicat **et** une position accessible à côté ». Java l'a dans
`PathJobFindWater` (`EntityAIWorkFisherman.java:392`), `walkToTree`
(`EntityAIWorkLumberjack.java:409-424`) et `getWorkingPosition`
(`EntityAIWorkLumberjack.java:1011-1014`).

**Fait à la place.** Ces trois réponses sont des **faits** :
`LumberjackFacts::tree_candidate` + `PathState`, `FishermanFacts::pond` /
`pond_stand` / `pond_state` / `PathState`, `MinerFacts::node` / `parent_node` /
`at_standing`. Le module porte toutes les décisions que Java prend **à partir** de ces
réponses (le rayon de recherche qui grandit, la rotation du nœud, le choix du bloc du
puits, l'abandon d'un étang sous-optimal) et aucune décision de recherche.

**Coût.** La recherche doit être écrite une fois, côté hôte, avec le même contrat que
`PathJobFindWater` (5×2, état du pond, position de travail). Tant qu'elle n'existe pas,
un appelant ne peut pas nourrir ces quatre IA d'un vrai monde — mais il peut les tester,
ce qui est le but de ce module.

---

## 5. Les tirages aléatoires : disponibles, mais volontairement des faits

**Voulu.** Rien : c'est une **décision de conception**, pas un manque du SDK — et il faut
le dire clairement parce que `src/ai/MANQUANT.md` §1 dit « aucune source aléatoire »,
ce qui n'est plus exact.

**Ce qui existe.** `levels::level_random(level) -> Option<LegacyRandom>` (`levels.rs:606`),
`levels::level_set_random` (`:1520`), `items::item_resource_enchant_randomly` (`:398`).

**Fait à la place.** Les quatre tirages que ces métiers font sont des faits fournis par
l'appelant : `LumberjackFacts::draw_100` (`EntityAIWorkLumberjack.java:346`),
`FishermanFacts::draw_int` (`:592`), `draw_double` (`:490`), `draw_pond` (`:429`). Le
bût est la **rejouabilité** : une décision qui dépend d'un tirage ne peut pas être
testée, et ces quatre-là décident d'un état (`START_WORKING` vs
`LUMBERJACK_SEARCHING_TREE`, attente vs lancer, nouvel étang vs même étang).

**Coût.** L'appelant doit tirer avec la **même borne** : `fisherman::random_chance_bound`
la calcule (`10 + secondary / 5`), pour que la distribution soit celle de Java.

---

## 6. Les files d'attente de `Tree` restent chez l'appelant

**Voulu.** Porter `util/Tree.java` comme une structure vivante (`woodBlocks`,
`leaves`, `stumpLocations`).

**Ce qui existe.** C'est une structure de données sans dépendance de jeu : rien
n'empêche de la porter. Mais un `Tree` est **l'état du métier**
(`job.getTree()`, `EntityAIWorkLumberjack.java:375`), pas une observation du monde, et
ses opérations sont des mutations (`pollNextLog`, `Tree.java:827`) qui suivent une
cassure de bloc — donc une action.

**Fait à la place.** `TreeFacts` rapporte les **longueurs et la tête** des deux files
(`logs`, `leaves`, `next_log`, `next_leaf`) et les souches. Les décisions n'ont jamais
besoin de plus : `hasLogs` (`:887`), `hasLeaves` (`:877`), `peekNextLog` (`:857`).

**Coût.** La file elle-même (et son ordre exact, qui n'est pas FIFO : `Tree.addAndSearch`
insère en tête pour les feuilles) doit vivre côté appelant. Le port ne prétend pas la
détenir.

---

## 7. Ce qui n'est **pas** porté, et qui est nommé

Trois moitiés de métier sont laissées explicitement à d'autres tables, avec un refus
nommé plutôt qu'un état inventé :

- **La moitié « craft » du bûcheron et du fermier** (`getNextCraftingState`,
  `AbstractEntityAICrafting.java:271-291`) : `GatherRefusal::CraftingNotPorted`
  (`EntityAIWorkLumberjack.java:260`).
- **La moitié « structure » du mineur** (`START_BUILDING`, `LOAD_STRUCTURE`,
  `BUILDING_STEP`) : `GatherRefusal::StructureTableOwned`, portée par
  `crate::ai::builder` sur `WorkerState` (`AbstractEntityAIStructure.java:334-535`).
- **Le placement de blueprint d'un nœud** (`executeStructurePlacement`,
  `EntityAIStructureMiner.java:784-826`) : le port décide `LOAD_STRUCTURE` et laisse le
  placeur faire le reste (il n'y a pas de placeur dans le SDK : `level_set_block` pose un
  bloc, pas un schéma).

---

## 8. Le pêcheur ne peut pas lire les deux blocs de `tryDifferentAngles`

**Voulu.** Les deux questions que Java pose au monde dans
`EntityAIWorkFisherman.tryDifferentAngles` (`:340-352`) : le bloc sur lequel le citoyen se
tient est-il un **liquide**, et le bloc **sous** l'eau est-il **solide**.

**Pourquoi ce n'est pas dans les faits.** `FishermanFacts` porte le résultat d'une
recherche d'eau (`pond`, `pond_state`, `at_water`, `water_in_range`, `hook`) mais aucune
lecture de bloc autour du citoyen, et `GatherBlocks` décrit l'étang, pas la position du
pêcheur : la réponse n'est pas dérivable des faits actuels.

**Fait à la place.** `fisherman::try_different_angles` compte la rotation et laisse ces
deux réponses à l'appelant ; c'est `MAX_ROTATIONS` (`:110`) qui finit par jeter un étang
invariablement `SUBOPTIMAL` ou mort-né.

**Coût.** Un étang dont l'eau repose sur un bloc non solide n'est pas abandonné au premier
essai comme en Java : il l'est au bout des six rotations. Si les deux lectures arrivent un
jour dans `FishermanFacts`, le `else { executedRotations++ }` de `:350-352` pourra être
porté tel quel à côté du compteur.
