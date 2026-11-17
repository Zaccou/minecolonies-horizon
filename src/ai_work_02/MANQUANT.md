# Ce qui manque à `src/ai_work_02`, et pourquoi

Trois métiers : le cuisinier (`EntityAIWorkChef`), le teinturier (`EntityAIWorkDyer`) et
le concasseur (`EntityAIWorkCrusher`). Ce fichier ne liste pas les TODO du port : il
liste ce que **l'API de mod ne permet pas d'écrire**, avec, pour chaque ligne, ce que le
port fait à la place. Une ligne sans réponse n'existe pas ici — soit la règle est
écrite, soit elle est dans cette liste.

---

## 1. Créer une requête de carburant

**Java** : `AbstractEntityAIRequestSmelter.java:111`

```java
worker.getCitizenData().createRequestAsync(new StackList(possibleFuels,
    RequestSystemTranslationConstants.REQUESTS_TYPE_BURNABLE, STACKSIZE * module.getFurnaces().size(), 1));
```

**Ce que je voulais faire** : créer la `StackList` pour que le livreur apporte du
charbon.

**Ce qui manque** : la création de requête est un appel d'hôte. `crate::requests` porte
la *résolution* d'une requête (les cinq étages, `ai_build::materials`), pas sa création
depuis une IA.

**Ce que le port fait** : `check_furnace_fuel` lève
`TradeMemory::fuel_request_made` et `TradeMemory::needed = Need::fuel(fuels, STACKSIZE ×
fours)`. L'appelant qui a le `ServerHost` crée la requête ; la règle a décidé *quand* et
*combien*, et un test l'affirme (`the_fuel_request_needs_all_three_gaps`).

---

## 2. Accélérer un `FurnaceBlockEntity`

**Java** : `AbstractEntityAIRequestSmelter.java:120-142`

```java
FurnaceBlockEntity.serverTick(furnace.getLevel(), entity.getBlockPos(), entity.getBlockState(), furnace);
```

**Ce que je voulais faire** : faire avancer la cuisson de `(secondary / 10) × 2` ticks par
four allumé, toutes les secondes.

**Ce qui manque** : `serverTick` est une méthode statique de vanilla sur un type vanilla.
Aucun hook de mod ne laisse un mod appeler le tick d'un bloc vanilla d'un autre chunk, et
le port n'a pas la classe en main.

**Ce que le port fait** : `accelerate_furnaces` calcule le nombre exact de ticks
demandés et l'accumule dans `TradeMemory::accelerated` (produit des ticks par four
allumé). Le calcul, lui, est testé (`the_acceleration_counts_ticks_per_lit_furnace`) :
6 ticks pour `secondary = 30` sur deux fours allumés.

---

## 3. Retirer un bloc du module `FURNACE`

**Java** : `AbstractEntityAIRequestSmelter.java:595-599`

```java
building.getModule(FURNACE).removeFromFurnaces(pos);
```

**Ce que je voulais faire** : oublier un four qui n'en est plus un (un bloc cassé, un
`FurnaceBlockEntity` remplacé par autre chose).

**Ce qui manque** : c'est une écriture sur les données d'un bâtiment, pas une décision.
Le port n'écrit pas dans la colonie depuis une règle.

**Ce que le port fait** : `execute_crafting_action` répond
`TradeRefusal::StaleFurnace` et pose la position dans
`TradeMemory::dropped_from_module`. Un test vérifie les deux
(`a_stale_block_leaves_the_module`).

---

## 4. Remettre le quota à zéro au réveil du bâtiment

**Java** : `BuildingCrusher.java:98-103`

```java
@Override public void onWakeUp() { super.onWakeUp(); this.currentDailyQuantity = 0; }
```

**Ce que je voulais faire** : que le quota du concasseur reparte de zéro chaque matin.

**Ce qui manque** : `onWakeUp` est un événement du bâtiment vanilla. `crate::building`
porte l'ordre des écritures d'une pose et d'une amélioration, pas ce réveil-là, et un mod
ne s'abonne pas à lui.

**Ce que le port fait** : rien, et c'est dit. `CrusherFacts::daily_quantity` est lu comme
un fait ; le jour où `crate::building` porte `onWakeUp`, c'est là que la remise à zéro
ira, pas ici.

---

## 5. Le délai de `idle`

**Java** : `AbstractEntityAICrafting.java:193`

```java
setDelay(TICKS_20 * 20);
```

**Ce que je voulais faire** : qu'un citoyen qui rentre dans son bâtiment attende 400
ticks avant de choisir un passe-temps.

**Ce qui manque** : `crate::ai_build::crafter::IdleDecision` rend le pas choisi
(`Stay`, `WalkToBuilding`, `SitDown`, `StandAt`, `Wander`) et la position, mais **pas** le
délai : la fonction voisine a décidé de ne pas le modéliser, et ce module ne peut pas
l'inventer sans lire Java à sa place. Corriger cela demande une ligne dans
`ai_build/crafter.rs`, qui n'est pas mon dossier.

**Ce que le port fait** : il rend le pas (`TradeMemory::idle_step`) et laisse l'appelant
appliquer les 400 ticks s'il les veut. Un test affirme le pas, pas le délai
(`the_idle_step_is_reported_not_performed`).

---

## 6. Deux citations fausses repérées dans `crate::ai_build::crafter` (pas corrigées)

En cherchant les lignes des états, j'ai mesuré que `ai_build/crafter.rs` cite
`AIWorkerState.java:69`, `:73` et `:77` pour `QUERY_ITEMS`, `CRAFT` et `GET_RECIPE`.
Dans l'arbre (`minecolonies-src`, `api/entity/ai/statemachine/states/AIWorkerState.java`)
ces trois constantes sont aux lignes **523**, **528** et **518** ; les lignes 69, 73 et 77
sont deux lignes de javadoc et une ligne vide, dans le bloc `###FISHERMAN###`
(`FISHERMAN_SEARCHING_WATER` à `:70`, `FISHERMAN_START_FISHING` à `:76`). Ce module cite
les lignes mesurées, et signale l'écart au lieu de le recopier.

## 7. Une branche de Java qui ne peut pas se déclencher

`EntityAIWorkCrusher.java:158-161` :

```java
final int requestQty = Math.min((dailyLimit - crusherBuilding.getCurrentDailyQuantity()) * 2, STACKSIZE);
if (requestQty <= 0) { return START_WORKING; }
```

`:121` renvoie déjà `START_WORKING` quand `getState() != CRAFT` **et**
`currentDailyQuantity >= dailyLimit`. Donc à `:158`, hors de `CRAFT`, il reste toujours
au moins un item, et `requestQty` vaut au moins 2 : la branche est morte. Le port garde
`TradeRefusal::RequestQuantityZero` comme garde sur des faits que Java ne produit pas, et
un test dit pourquoi elle est inatteignable
(`the_zero_request_branch_is_preempted_by_the_limit_check`).

## 8. Deux ports de `AbstractEntityAIRequestSmelter` coexistent (arbitrage à faire)

`EntityAIWorkBaker` (`ai_work_01`), `EntityAIWorkBlacksmith` (`ai_work_01`) et le
cuisinier / le teinturier (ici) étendent tous `AbstractEntityAIRequestSmelter` : les
quatre règles de four (`fillUpFurnace`, `addFuelToFurnace`,
`retrieveProductFromFurnace`, `retrieveUnrelatedProductFromFurnace`) et les trois
événements (`checkFurnaceFuel`, `accelerateFurnaces`, `checkForLeftOvers`) sont **la même
classe Java**.

Mesuré pendant cette tranche : `src/ai_work_01/furnace.rs` (écrit en parallèle par un
autre agent, et déclaré dans `lib.rs`) porte déjà ces règles, sous une autre forme — des
énumérations de résultat (`FuelCheck`, `Stoke`, `Product`, `Unrelated`, `Fill`,
`Execute`) sur un `BakerFacts`. `smelter.rs` les porte ici sous forme de [`Decision`] sur
un `SmelterFacts`.

**Pourquoi les deux existent encore.** `Machine<S, F>` est monomorphe en `S` **et** en
`F` : la machine du boulanger tique sur `(WorkState, Scene)` de `ai_work_01`, la mienne
sur `(WorkState, Scene)` d'ici. Réutiliser leurs règles demanderait de convertir mon
`SmelterFacts` en `BakerFacts` à chaque tick — un couplage à un fichier qu'un autre agent
était en train d'écrire au même moment, que je n'ai ni le droit de modifier ni le moyen de
compiler. J'ai donc écrit ma table et je le dis, plutôt que de dépendre d'une signature
qui bougeait.

**Ce qu'il faut faire, et qui ne peut pas être fait d'ici** : garder **un** port des sept
règles de `AbstractEntityAIRequestSmelter` et le partager par un type de faits commun
(`FurnaceFacts`, indépendant du métier), les trois métiers n'ajoutant que leur
`getExpectedBuildingClass` et leur nom de statistique — exactement ce que Java fait avec
l'héritage. C'est un arbitrage d'orchestrateur : il voit les deux fichiers compilés, moi
je n'en vois qu'un.

## 9. Les métiers de fonte hors tranche

`EntityAIWorkSifter` a son propre état (`CRUSH` chez moi, `SIFT` chez lui) et son propre
quota ; `EntityAIWorkSmelter`, lui, utilise `AbstractEntityAIUsesFurnace` (une autre
classe de four, `:331`, `:481`), pas `AbstractEntityAIRequestSmelter`. Aucun des deux
n'est dans cette tranche, et aucun code de ce module ne prétend le contraire.

