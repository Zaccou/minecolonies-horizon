# MANQUANT — `src/ai_work_11`

Ce que ces trois buts d'IA ne peuvent pas faire avec l'API d'aujourd'hui, ce que le port
fait à la place, et les constats mesurés sur le Java. Chaque entrée dit **où** la règle
manque (`fichier.java:ligne`) et **ce que le port substitue**, pour qu'aucune des deux
moitiés ne se perde.

---

## 1. Ce que l'API ne permet pas d'écrire

### 1.1 Le résultat d'un chemin

`walkToBuilding()` (`AbstractEntityAIBasic.java:757-766`), `walkToWorkPos(pos)`
(`:800-806`) et `walkingToAnimal(animal)` (`AbstractEntityAIHerder.java:622-632`) sont des
demandes au chemin. Le port ne les demande pas : il compare des positions
(`Scene::at_hut`, `Scene::at_work_pos`, et `herder::walking_to_pos` pour l'élevage), avec
la même règle (`EntityUtils.isLivingAtSite`, `EntityUtils.java:301-305`, strictement
moins de `range`).

**Ce que ça perd** : une entité à deux blocs dont le chemin est encore en calcul répond
« arrivé » ici et « en marche » en Java. Aucun état des trois tables ne dépend de la
différence — c'est une action de service de plus ou de moins sur la même cible — mais
c'est une approximation, et les trois faits concernés (`at_building`,
`at_walk_target`, `at_work_pos`) sont documentés comme des réponses de l'appelant.

### 1.2 Les recherches d'entités et de blocs

`WorldUtil.getEntitiesWithinBuilding(world, EntityCitizen.class, building, null)`
(`EntityAIWorkCook.java:368`), la même chose pour `Player` (`:341-345`), et
`world.getBlockEntity(pos) instanceof FurnaceBlockEntity`
(`AbstractEntityAIUsesFurnace.java:133`) n'ont pas d'équivalent dans le vocabulaire de
faits. Le port prend donc :

| Règle | Ligne | Fait substitué |
| --- | --- | --- |
| les citoyens de la hutte | `EntityAIWorkCook.java:368` | `CookFacts::citizens` (déjà filtrés par la hutte, dans l'ordre du monde) |
| les joueurs affamés et autorisés | `:341-345` | `CookFacts::players` (prédicat de faim et de permission appliqué par l'appelant) |
| les fours enregistrés | `AbstractEntityAIUsesFurnace.java:131`, `:159`, `:323` | `SmelterFacts::furnaces` |

Ce qu'il faudrait : un hook de requête d'entités dans une zone avec un prédicat de type,
et un accès aux positions du module de fours. Aucun des deux n'est dans le catalogue que
ce module a le droit d'utiliser. **Conséquence mesurable** : un citoyen qui entre dans la
hutte entre deux ticks n'est vu qu'au tick suivant, alors que Java le voit au tick où il
regarde.

### 1.3 `FoodUtils`, deux cents lignes de score

`getBestFoodForCitizen` (`FoodUtils.java:147-200`) note chaque plat par diversité, qualité
et `IMinecoloniesFoodItem`, avec un détour par la nourriture de la maison et du travail ;
`hasBestOptionInInv` (`:275-...`) et `checkForFoodInBuilding` (`:...`) ont la même forme.
Le port lit les **réponses** : `ServeCitizen::food_slot`, `CitizenHere::has_best_option`,
`CitizenHere::food_in_building`. Rejouer ces règles serait porter un module de deux cents
lignes pour aucun état de plus, et il dépend de `ICitizenFoodHandler`, qui n'est pas dans
`crate::citizens`.

Ce qui **est** porté, parce que la quantité qu'il rend est une décision :
`InventoryUtils.transferFoodUpToSaturation` (`InventoryUtils.java:3222-3282`) →
`cook::plan_food_transfer`.

### 1.4 Les transferts d'objets

Six appels écrivent dans un `IItemHandler` : `transferXOfItemStackIntoNextFreeSlotInItemHandler`
(`EntityAIWorkCook.java:240`), `extractItem` (`:197`), `transferFoodUpToSaturation`
(`:288`), `transferItemStackIntoNextFreeSlotInItemHandler` (`AbstractEntityAIUsesFurnace.java:99`,
`EntityAIWorkCook.java:116`), `transferItemStackIntoNextBestSlotInItemHandler`
(`EntityAIWorkDeliveryman.java:257`) et `dumpInventoryIntoWareHouse` (`:310`).

Le port les remplace par trois [`crate::ai_work_11::Effect`] pour les cas où la **quantité**
est décidée ici (les trois du cuisinier) et par une observation pour les autres : ce qui
est entré dans un four se relit dans `SmelterFacts::furnaces`, ce qui est sorti d'un coffre
se relit dans `DeliveryFacts::gathered` / `extracted`.

### 1.5 Le système de requêtes

`createRequestAsync(new StackList(getAllowedFuel(), REQUESTS_TYPE_BURNABLE, STACKSIZE ×
fours, 1))` (`AbstractEntityAIUsesFurnace.java:245-247`),
`createRequestAsync(new Food(STACKSIZE, building.getBuildingLevel()))`
(`EntityAIWorkCook.java:434-437`), et les quatre `needsCurrently` (`:251`, `:256`,
`EntityAIWorkCook.java:386`, `:404`) sont des appels à `crate::requests`. Le port les
nomme : [`crate::ai_work_11::cook::Gather`] (quatre variantes) et les champs `ask_food` /
`ask_fuel` de [`crate::ai_work_11::cook::Start`]. La requête elle-même reste à l'appelant.

### 1.6 Les interactions

Cinq `triggerInteraction` de ces trois classes ne changent aucun état : elles préviennent
le joueur. Le port les porte comme des booléens ou des refus nommés, parce qu'un joueur
qui ne les voit pas ne comprend pas son ouvrier :

| Interaction | Ligne | Port |
| --- | --- | --- |
| `FURNACE_USER_NO_FOOD` | `EntityAIWorkCook.java:155` | `Start::ask_food` |
| `FURNACE_USER_NO_FUEL` | `AbstractEntityAIUsesFurnace.java:197` | `Start::complain_no_fuel_list` |
| `BAKER_HAS_NO_FURNACES_MESSAGE` | `:205`, `:459` | `Start::complain_no_furnaces`, `Refusal::NoFurnace` |
| `POOR_MENU_INTERACTION` | `EntityAIWorkCook.java:364` | `cook::Important::poor_menu` |
| `POOR_RESTAURANT_INTERACTION` | `:233` | `Refusal::PoorRestaurant`, posé **avec** l'effet de service |
| `COM_MINECOLONIES_COREMOD_JOB_DELIVERYMAN_NAMEDCHESTFULL` | `EntityAIWorkDeliveryman.java:422-435` | `DeliveryFacts::chest_refused_same_stack` (déjà porté par `ai_build::delivery`) |
| `COM_MINECOLONIES_COREMOD_JOB_DELIVERYMAN_NOWAREHOUSE` | `:657` | `Refusal::Courier(NoWarehouse)` |

### 1.7 Les statistiques, l'expérience et la saturation

`getStatisticsManager().increment(FOOD_SERVED, …)` (`:199`, `:242`, `:297`),
`StatsUtil.trackStatByName(building, …)` (`:200`, `:243`, `:298`, `:446`),
`addExperience` (`:119`, `:244`, `:301` ; `EntityAIWorkDeliveryman.java:186`, `:461`) et
`decreaseSaturationForContinuousAction` (`:245`, `:302`, `:453`, `:462`) sont des effets
que l'appelant exécute. Le port en donne la seule partie qui est un nombre décidé ici :
`cook::extraction()` → `{ xp: 2,0, actions: 1 }` (`:119-120`).

### 1.8 L'entrepôt du livreur

`job.findWareHouse()` (`EntityAIWorkDeliveryman.java:322-325`) demande au gestionnaire de
colonie, qui n'est pas une donnée de ce module. Les deux réponses sont des faits déjà
portés par `ai_build::delivery` : `DeliveryFacts::warehouse` et
`::warehouse_has_chest`. Rien à ajouter.

### 1.9 Les dégâts de l'abattage

`animal.hurt(ds, (float) getButcheringAttackDamage())`
(`AbstractEntityAIHerder.java:796`) tue ou blesse une entité vivante. Le port ne calcule
que le **nombre** — `swine::butchering_attack_damage`, qui vaut
`max(1,0, compétence / 10,0)` pour le porc (`EntityAIWorkSwineHerder.java:42-46`) et 5 pour
la classe mère (`AbstractEntityAIHerder.java:71`) — et laisse l'application à l'appelant,
qui rapporte ensuite `HerdFacts::killed`.

---

## 2. Deux tables sur trois étaient déjà portées, et c'est mesuré

La consigne demandait « une machine à états testable » pour les trois métiers. Deux
existaient déjà :

| Métier | Domicile existant | Mesure |
| --- | --- | --- |
| livreur | `crate::ai_build::delivery` | 1 089 lignes, `DeliveryState` (6 états), `check_if_execute`, `decide`, `prepare`, `deliver`, `pickup`, `dump`, `next_pickup`, `cannot_hold_more_items`, `max_parallel_deliveries`, et sa propre `Machine` (`delivery::ai()`) |
| éleveur de porcs | `crate::ai_work_09::herder` | 1 201 lignes, les 8 cibles d'`AbstractEntityAIHerder.java:140-148`, les 7 décisions, 33 refus nommés, et le `WorkAi` qui les fait tourner |

`EntityAIWorkSwineHerder` lui-même **n'enregistre aucune cible** : son constructeur est
`super(job)` (`:20-23`) et la classe ne fait que 47 lignes, dont trois surcharges
(`updateRenderMetaData` `:25-34`, `getExpectedBuildingClass` `:36-40`,
`getButcheringAttackDamage` `:42-46`). Écrire une deuxième machine pour lui aurait été la
cinquième copie d'une table qui n'a qu'un propriétaire.

Ce module **raccorde** donc les deux et le prouve : `tests::the_swine_table_answers_like_the_herder_table`
et `tests::the_courier_table_answers_like_the_delivery_table` font tourner les deux
machines en parallèle sur les mêmes faits et comparent les états tick par tick. Le
cuisinier, lui, est neuf : six cibles héritées du four et deux qui n'existaient nulle
part.

---

## 3. Constats mesurés sur le Java (copiés, pas corrigés)

1. **La file des citoyens n'est jamais vidée en entrant dans `checkForImportantJobs`.**
   `EntityAIWorkCook.java:379` fait `citizenToServe.add(citizen)` sans test de doublon, et
   `:165` ne lit que `isEmpty()`. Un citoyen encore en file au tick suivant y entre donc
   **deux fois** et peut recevoir deux portions. `CookMemory::queue_citizen` n'ajoute pas
   de dédoublonnage, et un test le fixe
   (`facts::tests::queueing_the_same_citizen_twice_keeps_both_copies`). Java n'est pas
   corrigé ici : ce serait un changement de comportement déguisé en port.
2. **Un citoyen qui travaille n'est jamais nourri.** `shouldBeFed` exige `!isWorking()`
   (`:422`). Un citoyen à son poste avec 2 de saturation attend la fin de sa journée.
   Copié (`cook::should_be_fed`), testé.
3. **Le menu vide n'arrête rien.** `requestSmeltable` (`:149-157`) lève une interaction et
   ne change aucun état : le cuisinier continue de cuisiner ce qu'il a. La plainte est donc
   un booléen de `Start`, pas un `Refusal`.
4. **`POOR_MENU_INTERACTION` ne dépend d'aucune faim** : au niveau 3, il suffit que le menu
   n'ait aucun plat `IMinecoloniesFoodItem` (`:350-366`), même si personne n'a faim.
5. **La comparaison du logement est `>`** (`:231`) : un citoyen logé un niveau au-dessus du
   restaurant ne se plaint pas ; deux niveaux, si.
6. **La portion est tronquée avant d'être majorée.** `:237` fait `(int) Math.max(1.0,
   (FULL − saturation) / valeur)` puis `:239` `Math.ceil(qty × 1,5)`. À saturation 0 avec un
   aliment qui vaut 7 : `60 / 7 = 8,571` → `8` → `12`. Sans la troncature on aurait `13`.
   `cook::portion_for` copie l'ordre, et le test le fixe.
7. **Le porc frappe cinq fois moins fort qu'une poule sous 10 de compétence.** 1,0 contre
   5,0 (`EntityAIWorkSwineHerder.java:45` contre `AbstractEntityAIHerder.java:71`), et il
   faut 50 de compétence primaire pour que le porc rattrape la classe mère.
8. **Le porc ne tire pas son coup, la poule si.** `EntityAIWorkSwineHerder` ne surcharge
   pas `butcherAnimal`, donc `AbstractEntityAIHerder.java:805-838` s'applique et le coup
   porte toujours. `EntityAIWorkChickenHerder.java:67` fait au contraire
   `nextInt(1 + (100 − compétence) / 5) <= 1`. `swine::HIT_IS_CERTAIN` dit lequel des deux
   ce métier suit, et le test compare avec `herder::chicken_hit_lands`.
9. **Le seuil de nourriture gardée est `>`**, alors que celui du parent est `<=` :
   `food_kept > max(1, niveau²) × 9` (`:143-146`) d'un côté, `empty_slots <= 3`
   (`AbstractEntityAIUsesFurnace.java:120`) de l'autre. Les deux vivent dans
   `cook::reached_max_to_keep`, dans l'ordre du Java.
10. **La limite de sac du livreur disparaît au niveau maximum.** `cannotHoldMoreItems`
    (`EntityAIWorkDeliveryman.java:269-276`) sort `false` quand
    `building.getBuildingLevel() >= building.getMaxBuildingLevel()` : un livreur de niveau
    maximum porte tout. Déjà porté par `ai_build::delivery`, cité ici parce que la table de
    ce module l'appelle.

---

## 4. Ce qui reste à faire pour que ces trois métiers *agissent*

Rien de ce qui est ci-dessus n'est un état manquant : ce sont les écritures. Pour qu'un
citoyen de ces trois métiers se déplace vraiment, il faudra, dans l'ordre :

1. un hook de **navigation** qui réponde `at_building` / `at_work_pos` /
   `at_walk_target` après un vrai chemin (aujourd'hui : une comparaison de positions) ;
2. un hook d'**inventaire** qui applique les trois `Effect` et rapporte les compteurs
   (`actions_done`, `smeltable_in_inventory`, `food_kept`) mis à jour ;
3. un hook de **requêtes** qui crée les `StackList` / `Food` demandés par
   `cook::Start` (`ask_food`, `ask_fuel`, `gather`) ;
4. un hook d'**entités** qui réponde `CookFacts::citizens` et `::players` dans l'ordre du
   monde, avec le prédicat de type.

Tant que ces quatre n'existent pas, ces machines répondent « quel état, pourquoi, et
combien » — ce qui est exactement ce qu'un test peut vérifier sans un monde.
