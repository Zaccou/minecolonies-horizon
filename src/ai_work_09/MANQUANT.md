# MANQUANT — `src/ai_work_09`

Ce que ces trois buts d'IA ne peuvent pas faire avec l'API d'aujourd'hui, ce que le port
fait à la place, et les constats mesurés sur le Java. Chaque entrée dit **où** la règle
manque (`fichier.java:ligne`) et **ce que le port substitue**, pour qu'aucune des deux
moitiés ne se perde.

---

## 1. Ce que l'API ne permet pas d'écrire

### 1.1 Le résultat d'un chemin

`walkingToAnimal` (`AbstractEntityAIHerder.java:622-632`) est
`!EntityNavigationUtils.walkToPos(worker, animal.blockPosition(), 2, true)`. Le port ne
demande pas au chemin : il compare des positions
(`herder::walking_to_pos`, `WorldFacts::worker::at` contre la position de l'animal, à
moins de `WALK_TO_ANIMAL_RANGE` = 2). **Ce que ça perd** : une entité à deux blocs dont
le chemin est encore en calcul répond `true` en Java et « arrivé » ici. Aucun état de la
table ne dépend de la différence (une action de plus ou de moins sur le même animal),
mais c'est une approximation et elle est dite au-dessus de la fonction.

Même chose pour `walkWithProxy(items.get(0).blockPosition(), 1)`
(`AbstractEntityAIHerder.java:579`), réduit à `herder::at_item` avec la portée 1.

### 1.2 `canMate`, qui touche deux entités vivantes

`canMate(first, second)` (`:493-510`) pose `setInLoveTime(5)` sur les deux bêtes, demande
`first.canMate(second)`, puis restaure. C'est une écriture sur deux entités suivie d'une
question au monde : non rejouable. Le port lit la **réponse** dans
`HerdAnimal::can_mate`, que l'appelant a mesurée avec la même manipulation. Le champ est
documenté comme tel.

### 1.3 Les deux recherches d'entités

`searchForAnimals(module::isCompatible)` (`:596-599`) est
`WorldUtil.getEntitiesWithinBuilding(world, Animal.class, building, predicate)`, et
`searchForItemsInArea()` (`:611-614`) la même chose pour `ItemEntity`. Il n'y a **pas**
d'équivalent dans le vocabulaire de faits : le port prend une liste déjà filtrée
(`HerdFacts::animals`) et le compte plus la tête de la liste d'items
(`HerdFacts::items_on_ground`, `first_item`). Ce qu'il faudrait pour s'en passer : un
hook de requête d'entités dans une zone, avec un prédicat de type — absent du catalogue
que ce module a le droit d'utiliser.

### 1.4 Les écritures au monde

Quatre actions de ces trois buts sont des écritures qui n'ont pas de fait correspondant :

| Règle | Ligne | Ce que le port fait |
| --- | --- | --- |
| `world.setBlockAndUpdate(actionPosition, blockState)` | `EntityAIWorkPlanter.java:317` | `PlanterFacts::action_done` |
| `mineBlock(position)` | `EntityAIWorkPlanter.java:371` | `PlanterFacts::action_done` |
| `planterModule.applyBonemeal(...)` | `EntityAIWorkPlanter.java:352` | rien : `BONEMEAL` est toujours `FINISHED` après le test d'item |
| `mooshroom.mobInteract(fakePlayer, MAIN_HAND)` | `EntityAIWorkCowboy.java:229` | `CowboyFacts::stew_succeeded` |

### 1.5 Le système de requêtes

`checkIfRequestForItemExistOrCreateAsync` apparaît cinq fois
(`AbstractEntityAIHerder.java:319`, `:324`, `:744` ; `EntityAIWorkPlanter.java:403` par
`needsCurrently`), et `currentDeliverable` (`:68`) est renvoyé au système à la reprise
(`:430-438`). Le port note **qu'il manque quelque chose** (`PlanterMemory::deliverable`,
`Memory::need`, `WorkRefusal::NoPlantItem` / `NoBonemeal` / `NoAxe`) et s'arrête là :
créer la requête est `crate::requests`, hors de ce dossier.

### 1.6 Les tirages

`ColonyConstants.rand.nextDouble()` (`AbstractEntityAIHerder.java:230`, `:234`, `:242`)
et `worker.getRandom().nextInt(...)` (`EntityAIWorkChickenHerder.java:67`) sont des
tirages, pas des observations. Le port les prend en faits :
`HerderDraws { pickup, butcher, feed }` et `HerdFacts::chicken_draw`. Trois
`nextDouble()` successifs étant indépendants, pré-tirer les trois rejoue exactement la
même suite de décisions ; un curseur n'aurait rien apporté.

### 1.7 L'identité d'une entité

`animalsToBreed` (`:119`) est une `List<Animal>` et `fedRecently` (`:124`) une
`Map<UUID, Long>`. Le port garde des clés `u32` (`HerdAnimal::id`) : c'est à l'appelant
de fournir une clé stable pour la même bête d'un tick à l'autre. Un partenaire dont la
clé a disparu de la liste est traité comme mort (`herder::breed_two_animals`), ce que
Java fait par `isDeadOrDying` (`:644`).

### 1.8 Les deux modules, qui décident pour de bon

`IPlantationModule.decideFieldWork`, `getNextWorkingPosition`, `getActionLimit`,
`getValidBonemeal`, `getPlantsToRequest` d'un côté ;
`AnimalHerdingModule.isCompatible`, `getBreedingItems`, `BuildingCowboy.HerdingModule
.canTryToMilk` / `canTryToStew` de l'autre. Le port lit leurs **réponses**
(`FieldWork`, une liste filtrée, deux booléens) et ne rejoue aucune de leurs six
implémentations. `onMilked()` / `onStewed()`
(`BuildingCowboy.java:271-281`) ne sont pas portés non plus : ils ne font que monter un
compteur dont la comparaison est déjà un fait.

### 1.9 L'interface citoyen

`setVisibleStatus` (`EntityAIWorkPlanter.java:136`, `EntityAIWorkCowboy.java:146`),
`setJobStatus(JobStatus.STUCK)` (`EntityAIWorkPlanter.java:107`, `:118`) et
`triggerInteraction(new StandardInteraction(NO_FREE_FIELDS, BLOCKING))` (`:116`)
apparaissent dans les branches que le port nomme (`WorkRefusal::HutNotBuilt`,
`NoFreeField`) mais ne sont pas des décisions : l'appelant les rejoue à partir du refus.
Le badge de statut visible est une donnée d'affichage, pas un but.

---

## 2. Constats mesurés sur le Java (pas des manques, des faits)

1. **`isFeedAble` n'est appelée nulle part.** `grep -rn isFeedAble src/main/java/` ne
   rend que sa déclaration (`AbstractEntityAIHerder.java:268-271`). Portée quand même
   (`HerdAnimal::is_feed_able`), et dite morte.
2. **`EntityAIWorkPlanter.canGoIdle` rend toujours `false`.**
   `if (getExtensionToWorkOn() == null) return !super.hasWorkToDo();` (`:476-479`) et
   `hasWorkToDo` est surchargé en `true` (`:468-471`) : les deux branches rendent
   `false`. `planter::can_go_idle` le dit au lieu de recopier le `if`.
3. **La branche « limite du champ » de `getActionsDoneUntilDumping` n'est pas atteinte
   depuis `decide`.** `:447-450` la garde sur `PLANTATION_WORK_FIELD` /
   `PLANTATION_DECIDE_FIELD_WORK`, alors que le seul appelant de `decide` est
   `START_WORKING` (`AbstractEntityAICrafting.java:284`). Elle n'est vivante que par
   `AbstractEntityAIBasic.inventoryNeedsDump` (`:413`), qui lit `getState()` dans l'état
   courant. Le test `the_dump_threshold_of_decide_is_always_the_base_constant` le tient.
4. **`chanceToButcher` peut rendre `Infinity`.** Avec `getBuildingLevel() == 0`,
   `maxAnimals` vaut 0 et `:695` divise par `Math.pow(0, 4) == 0.0`. Le port ne corrige
   pas, et le test l'affirme.
5. **`MILK_COOL_DOWN` ne compte pas des ticks.** `:56` vaut 10 et `:103` le décrémente
   d'une unité par passage en `DECIDE`, qui a lieu tous les `DECIDING_DELAY` = 80 ticks :
   une traite ratée bloque la traite 800 ticks, pas 10.
6. **`breedTimeOut` peut devenir négatif et le test est `== 0`.** `:205` retire 80 sans
   plancher, et `:238` teste `breedTimeOut == 0`. Depuis 1200, les valeurs sont 1120,
   1040, …, 80, 0 : le `0` arrive, donc la règle fonctionne — mais rien ne garantit que
   le pas tombe juste, et un délai non multiple de 80 sauterait le zéro. Les trois
   valeurs posées par le Java sont des multiples de 80, ce n'est donc pas un bug
   observable aujourd'hui.
7. **Un ramassage raté compte une action.** `:579-587` : seule l'arrivée rend
   `getState()` ; en marchant, le `if` tombe dans `incrementActionsDone`.
8. **`workField` lève une `NullPointerException` si `activeModuleResult` est nul**
   (`EntityAIWorkPlanter.java:208`). `prepare` (`:90-94`) l'empêche dans le jeu ; le port
   nomme `WorkRefusal::NoActiveResult` et va sur `PICK_FIELD`.
9. **`handlePlantingAction` et `handleMiningAction` testent `actionPosition != null`
   deux fois** — une fois dans `workField` (`:209`) pour la marche, une fois dans le
   handler (`:303`, `:364`) pour le travail. Le port garde les deux : elles ne protègent
   pas la même chose.

---

## 3. Raccordements et doublons possibles

- **Rien n'est dupliqué d'`ai_gather` ni d'`ai_build`.** Le moteur
  (`crate::ai::state::Machine`), les faits de base (`crate::ai::facts::WorldFacts`), la
  mémoire de base (`crate::ai::worker::Memory`) et la formule de minage
  (`crate::ai_build::work_time::mining_ticks`, appelée par `planter::mining_ticks`) sont
  réutilisés, pas recopiés.
- **Un recouvrement à arbitrer.** Le planteur étend `AbstractEntityAICrafting`
  (`EntityAIWorkPlanter.java:46`) et le port enregistre deux de ses cibles (`IDLE →
  START_WORKING`, `START_WORKING → decide`) sur **son** `WorkState`. `ai_build::crafter`
  porte la table de l'atelier sur le `WorkerState` de `crate::ai`, et `crate::ai::worker`
  porte les quatorze cibles de base. Les trois tables coexistent sans se contredire (types
  d'état différents), mais un métier qui voudrait les deux à la fois devrait choisir une
  seule énumération d'état — c'est le constat déjà écrit dans
  `crate::ai_gather` (`MANQUANT.md` §2) et il vaut ici aussi.
- **Les quatre autres éleveurs ne sont pas portés** (`EntityAIWorkShepherd`,
  `EntityAIWorkSwineHerder`, `EntityAIWorkRabbitHerder`, `EntityAIWorkStablemaster`).
  Ils étendent `AbstractEntityAIHerder` : ils se raccorderont à `herder::register` et
  n'auront, comme l'éleveur de poulets, qu'une ou deux méthodes à surcharger.
