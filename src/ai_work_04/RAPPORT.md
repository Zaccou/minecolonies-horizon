# `src/ai_work_04` — les buts du scieur, du cribleur et du fondeur

Sept fichiers Rust, **3 682 lignes** : **2 207** de code, **1 475** de tests, **69 tests**. Aucune ligne
écrite hors de `src/ai_work_04/`. Aucun import de `mod_api` : le module ne lit ni n'écrit le monde, il
décide.

| Fichier | Lignes | Rôle |
| --- | ---: | --- |
| `mod.rs` | 132 | la façade : les deux tables, ce qui est réutilisé, les deux coutures |
| `facts.rs` | 590 | ce que les trois métiers ont le droit de lire, en données |
| `states.rs` | 411 | les 4 + 8 états, les 22 refus nommés |
| `sifter.rs` | 638 | le cribleur : 4 cibles, une recette, un compteur de coups |
| `smelter.rs` | 1 411 | le fondeur : 7 cibles, cinq décisions de four, un module de minerai |
| `sawmill.rs` | 183 | le scieur : ce qu'une classe de 45 lignes ajoute (rien d'utile à rejouer) |
| `tests.rs` | 317 | les transcriptions de scénario |
| `MANQUANT.md` | — | les 5 manques rencontrés, avec ce qui a été fait à la place |

---

## 1. Trois prémisses du brief étaient fausses — dites, pas contournées

### 1.1 Le scieur n'a pas de machine à états à porter : elle est déjà portée

`EntityAIWorkSawmill.java` fait **45 lignes** : `package`, huit imports, un javadoc de classe. Il
déclare :

- **zéro** `static final` numérique (`:20-21` est une `VisibleCitizenStatus`, une icône et une clé de
  traduction) ;
- **zéro** `AITarget`, **zéro** appel à `registerTargets` (`:28-31` ne fait que `super(sawmill)`) ;
- **zéro** override de `decide` ni de `getActionsDoneUntilDumping` — et son seul override, `craft()`,
  ne fait que poser le statut puis appeler `super.craft()` (`:39-44`) ;
- trois membres utiles : le statut visible (`:20-21`), `getExpectedBuildingClass()`
  (`:33-37`, → `BuildingSawmill`), et `craft()` qui pose le statut puis appelle `super.craft()`
  (`:39-44`).

Sa table est donc celle de `AbstractEntityAICrafting` (785 lignes), **déjà portée** par
`crate::ai_build::crafter` (six cibles, huit sorties de `getRecipe`, huit sorties de `craft`,
25 tests) avec sa durée dans `crate::ai_build::work_time::craft_hits`
(`AbstractEntityAICrafting.java:734-738`). Recopier cette table ici aurait été une seconde copie d'une
table déjà prouvée, ce que la règle 6 interdit. `sawmill.rs` porte donc **le delta** : le statut
visible, la hutte attendue, et `machine()` qui rend la table du crafter. Un test vérifie que le delta
est vide (`targets_added: 0, constants_declared: 0, states_named: 0`) au lieu d'un commentaire qui
l'affirme.

### 1.2 « outil cassé » n'est la règle d'aucun de ces trois métiers

Le cribleur n'a pas d'outil qui s'use : son « outil » est un **mesh** (`ModTags.meshes`,
`EntityAIWorkSifter.java:133-135`), rangé dans la main secondaire (`:176-182`) et jamais endommagé. Le
fondeur n'en a pas non plus. Le seul cas « l'outil casse » du voisinage est
`AbstractEntityAICrafting.java:604-628` (`damageItemInHand` puis `getHeldItem(...).isEmpty()` →
`finishRequest(false)`) : il est porté par `ai_build::crafter::Execution::ToolBroke`, pas ici. Les refus
que ces trois métiers **nomment** réellement sont les 22 de `states.rs` — dont « rien à faire »
(`NothingToCraft`), « inventaire plein » (`InventoryFull`) et « quota du jour atteint »
(`QuotaReached`).

### 1.3 900–1 400 lignes était une estimation pour trois classes de 45/223/239 lignes

Mesuré : le fondeur seul demande 794 lignes de règles, parce que sa classe mère
`AbstractEntityAIUsesFurnace` fait **507 lignes** et que `startWorking` (`:182-261`) a **dix sorties**,
chacune avec son effet (deux messages, deux requêtes, deux cueillettes, une position de marche). Le
module fait 3 682 lignes avec 69 tests au lieu de 900–1 400 et 40 : **2 207 de code, 1 475 de tests**.
Réduire aurait voulu dire perdre des règles réelles ou leurs tests ; j'ai préféré dépasser et le dire.
Le détail par méthode est ci-dessous, pour que chacun puisse juger ligne par ligne.

---

## 2. Ce qui est porté — compte exact

### 2.1 Les deux nouvelles tables

**Cribleur** — 4 cibles atteignables (`sifter.rs`) :

| # | État | Règle | Cadence | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | `hasWorkToDo` (+ quota) → `START_WORKING` | 20 | `AbstractEntityAICrafting.java:164`, `EntityAIWorkSifter.java:97-100` |
| 2 | `IDLE` | `idle` (Returns null) | 20 | `AbstractEntityAICrafting.java:165` |
| 3 | `START_WORKING` | `decide` → `SIFT` | 5 | `:166`, `EntityAIWorkSifter.java:91-94` |
| 4 | `SIFT` | `sift` | 10 | `EntityAIWorkSifter.java:73` |
| — | `INVENTORY_FULL` | `afterDump` → `IDLE` | — | `AbstractEntityAIBasic.java:1213-1222` |

`QUERY_ITEMS`, `GET_RECIPE` et `CRAFT` sont enregistrés par Java (`:167-169`) mais **inatteignables**
pour ce métier : `decide()` répond `SIFT` sur toutes ses branches (`:91-94`) et rien d'autre n'entre
dans ces états. Ils ne sont donc pas enregistrés ici ; le test
`sifter::tests::the_table_has_four_targets_and_each_state_can_be_left` tient la liste des cibles, et
`states::tests::the_sifter_has_four_states_and_three_of_them_exist_in_the_base_table` la liste des
états.

**Fondeur** — 7 cibles (`smelter.rs`) :

| # | Passe / état | Règle | Cadence | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | → `START_WORKING`, **sans condition** | 5 | `AbstractEntityAIUsesFurnace.java:77` |
| 2 | `START_WORKING` | `startWorking` | 60 | `:78` |
| 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 5 | `:79` |
| 4 | `AI_BLOCKING` | `accelerateFurnaces` (rend null : n'arrête rien) | 20 | `:80` |
| 5 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveSmeltableFromFurnace` | 5 | `:81` |
| 6 | `RETRIEVING_USED_FUEL_FROM_FURNACE` | `retrieveUsedFuel` | 5 | `:82` |
| 7 | `BREAK_ORES` | `breakOres` | 20 | `EntityAIWorkSmelter.java:69` |
| — | `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS` | sorties de base | — | `AbstractEntityAIBasic.java:1213-1222`, `:323-326` |

### 2.2 Les décisions pures, une par sortie de méthode Java

| Fonction | Sorties nommées | Java |
| --- | ---: | --- |
| `sifter::sift` | 9 (`Sift`) | `EntityAIWorkSifter.java:107-222` |
| `smelter::start_working` | 10 (`Start`, avec `walk_to`, `gather` et deux demandes) | `:182-261` |
| `smelter::check_if_able_to_smelt` | 3 conditions + liste des fours retirés | `:320-348` |
| `smelter::fill_up_furnace` | 4 (`Fill::Filled{smeltable, fuel}`) | `:452-499` |
| `smelter::retrieve_product` / `retrieve_used_fuel` | 5 (`Retrieve`) | `:388-445` |
| `smelter::break_ores` | 4 (`BreakOres`) | `EntityAIWorkSmelter.java:78-122` |
| `smelter::request_smeltable` | 3 + « rien à demander » (`Demand`) | `:200-238` |
| `smelter::is_smeltable` | 4 tests | `:186-197` |
| `smelter::check_for_important_jobs` | 2 (`ImportantJob`) | `:155-177` |
| `smelter::oven_to_retrieve_from` / `…_fuel_from` | 3 + 1 conditions | `AbstractEntityAIUsesFurnace.java:129-173` |

### 2.3 Les durées, chaque nombre à sa ligne

| Constante | Valeur | Java |
| --- | ---: | --- |
| `SIFT_TICK_DELAY` | 10 | `EntityAIWorkSifter.java:47` |
| `SIFT_MAX_LEVEL` | 50 | `:42` |
| `CHANCE_TO_DUMP_INV` | 10 | `:52` |
| `NO_MESH_DELAY` | 100 | `:57` |
| `ONE_HUNDRED_PERCENT` | 100 | `Constants.java:50` |
| `TICKS_SECOND` | 20 | `Constants.java:21` |
| `BUILDING_LEVEL_MULTIPLIER` | 64 | `BuildingSifter.java:22` |
| `SIFTER_MAX_BUILDING_LEVEL` | 5 | `:32` |
| `RETRIEVE_SMELTABLE_IF_MORE_THAN` | 10 | `AbstractEntityAIUsesFurnace.java:60` |
| `STORAGE_BUFFER` | 3 | `:66` |
| `STACKSIZE` | 64 | `Constants.java:23` |
| `ACCELERATION_PER_TEN_LEVELS` | 2 | `AbstractEntityAIUsesFurnace.java:285` |
| `START_WORKING_RATE` | 60 | `:78` |
| `BASE_XP_GAIN` (fondeur) | 5,0 | `EntityAIWorkSmelter.java:54` |
| `ORE_BREAK_XP_GAIN` | 0,2 | `:109` |
| `SIFT_XP_GAIN` | 0,2 | `EntityAIWorkSifter.java:211` |
| `RESULT/SMELTABLE/FUEL_SLOT` | 2 / 0 / 1 | `Constants.java:118`, `:123`, `:128` |

**Les deux formules de durée.** `sifter::sift_hits(level)` = `MAX_LEVEL -
effective(level) / 2 + 1` coups (division entière, `:188`), soit **51 coups à 0, 1 coup à 99** ;
`sift_ticks` = coups × 10, donc **510 ticks → 10 ticks**. La courbe `effective` est
`crate::ai::worker::effective_skill_level` (`AbstractEntityAIBasic.java:682-685`), réutilisée : elle
n'est pas réécrite.

### 2.4 Les refus nommés — 22

`NothingToCraft`, `QuotaReached`, `InventoryFull`, `NoMesh`, `MeshInInventory`, `NoRecipe`,
`RecipeIncomplete`, `RecipeFailed`, `BrokenSkillLevel`, `Walking`, `NotArrived`, `NoFurnace`,
`NoFuelList`, `NoOre`, `NoOreRecipe`, `NoOreInput`, `OreBreakFailed`, `FurnaceGone`, `ResultEmpty`,
`FuelEmpty`, `TargetNotAFurnace`, `NothingToFill`. Chacun porte `java()` (la ligne qui le décide) et
`key()` (la clé de message), et un test vérifie qu'aucune clé n'est dupliquée.

---

## 3. Ce qui est réutilisé, et pas recopié

- **Le moteur** : `crate::ai::state::{Machine, Transition, Action, Blocking}` — quatre passes,
  comptes à rebours, trappe d'état.
- **La mémoire de l'IA** : `crate::ai::worker::Memory` (`delay`, `walk_to`, `ticks_to_complain`) —
  réutilisée telle quelle par `facts::Scene<F>`.
- **Les règles du crafter** : `crate::ai_build::crafter::CraftFacts` est **embarqué** dans
  `SiftFacts::craft` (douze champs non recopiés) et `crafter::{has_work_to_do, idle}` sont appelés.
  `sawmill::machine()` rend cette table sans la retoucher.
- **La courbe de niveau** : `crate::ai::worker::effective_skill_level`.
- **Les types de position et de blocs** : `crate::ai::{Pos, Block, Blocks}`.

---

## 4. Le test qui compte : une suite d'états par métier

`tests.rs` déroule, sur des faits écrits à la main, la chaîne d'états que Java produit :

1. **Cribleur** — `hasWorkToDo` → `decide` → `sift` (un coup, l'état tient) ; au 51ᵉ coup
   `Produced`, quota +1, compteur à zéro, état tenu (`:186-221`).
2. **Fondeur** — `idleToStartWorking` → `startWorking` (`GATHERING_REQUIRED_MATERIALS`) →
   `getStateAfterPickUp` → `startWorking` (carburant) → `getStateAfterPickUp` → `startWorking`
   (`FILL_UP_FURNACES`) → `fillUpFurnace` → `START_WORKING`, avec la cadence 60 respectée (12 ticks
   machine).
3. **Scieur** — `hasWorkToDo` → `decide` → `getRecipe` → `queryItems` → `craft` → `INVENTORY_FULL` →
   `afterDump` → `IDLE`.
4. **L'événement d'accélération** n'arrête pas le tick : il rend `null` et la règle de l'état courant
   passe le même tick.

Les 69 tests couvrent, en plus : les quatre sorties de `break_ores`, les cinq de chaque récupération,
les quatre de `fillUpFurnace` (dont le four vide qui reçoit **les deux** transferts — la lecture
séquentielle de `:480`/`:489`), les trois conditions de `check_if_able_to_smelt`, les trois formes de
`request_smeltable`, les quatre tests de `is_smeltable`, les trois conditions de l'analyse de four, la
formule de quota, les deux formules de durée, l'absence de trappe dans les deux tables et le fait que
chaque règle cite un `fichier.java:ligne`.

---

## 5. Ce qui n'est pas couvert, et qui le sait

- **Aucune action sur le monde** : marche, transferts de piles, exécution de recette, particules, sons,
  statistiques, création de requête. Chacune est un fait (`facts.rs`) ou un effet de l'hôte.
- **Le tick d'entité de bloc du four** (`serverTick`, `AbstractEntityAIUsesFurnace.java:299`) :
  `acceleration_ticks` rend le nombre de ticks, l'effet reste à l'hôte. `MANQUANT.md` §3 dit pourquoi
  `levels::level_block_tick` n'est pas équivalent.
- **Le frère `AbstractEntityAIRequestSmelter`** (boulanger, briquetier, teinturier, souffleur de verre)
  est un **autre** parent, `extends AbstractEntityAICrafting` et non `…UsesFurnace` ; il est traité par
  `src/ai_work_01/furnace.rs`. Le cuisinier (`EntityAIWorkCook`) partage `…UsesFurnace` avec le fondeur
  et n'est traité ni ici ni là : sa seule différence est `extractFromFurnace` (XP 2 au lieu de 5).
- **Le quota du jour remis à zéro** (`BuildingSifter.onWakeUp`, `BuildingSifter.java:94-97`) : c'est la
  hutte qui le remet, pas l'IA.
- **Le demi-fait manquant** : `JobFacts` n'a pas de variante pour ces métiers, donc les deux tables ne
  peuvent pas encore être assemblées en un seul `JobAi`. `MANQUANT.md` §1 donne la forme exacte du
  raccord (et les deux mappages déjà écrits, `SiftState::base`, `SmeltState::base`).

---

## 6. Vérification : boucle jusqu'au vert

**Fait ici** : `rustfmt --edition 2024` en analyse seule sur les sept fichiers (aucune compilation),
puis `ssh deb '/home/zaccou/bin/port-check ai_work_04'` en boucle après l'autorisation de
l'orchestrateur. Trois itérations :

| Itération | Ce que la machine a dit | Ce que j'ai corrigé |
| --- | --- | --- |
| 1 | `error[E0599]: no variant named BrokenSkill found for enum ai_work_04::states::Refusal` | `sifter.rs` appelait `Refusal::BrokenSkill` ; la variante s'appelle `BrokenSkillLevel` (`Sift::BrokenSkill` est l'autre type, il restait juste) |
| 2 | `ai_work_04::smelter::tests::a_furnace_missing_one_side_is_the_one_to_fill` : `left: GatheringRequiredMaterials, right: FillUpFurnaces` | **Le test avait tort, pas le code** : `:249-258` (les deux cueillettes) passent avant `:260` (`checkIfAbleToSmelt`), donc avec un inventaire vide et une hutte pleine Java cueille d'abord. Le test a été corrigé en citant ces lignes — les poches sont désormais approvisionnées pour atteindre le remplissage |
| 3 | vert, puis une relecture indépendante a signalé un `unused_imports` invisible en `cargo test` : `Furnace` importé dans `smelter.rs` n'y sert que dans `mod tests` | import retiré du niveau module, ajouté dans `mod tests` — un `cargo build` sans `cfg(test)` n'a plus d'import mort |

**Résultat** : `port-check: VERT`, **69 tests passés, 0 échec** (`test result: ok. 69 passed; 0 failed;
2168 filtered out` — c'est bien le binaire de la lib, pas les doctests), aucun avertissement
attribuable à `src/ai_work_04` (après reconstruction forcée du module ; les deux `unused variable` qui
restent dans la sortie sont dans `src/colony/colonie.rs:384` et `src/research/progress.rs:745`, d'autres
agents).


