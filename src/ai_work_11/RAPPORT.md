# RAPPORT — `src/ai_work_11` : éleveur de porcs, cuisinier, livreur

Le port des buts d'IA de trois métiers de service et d'élevage de MineColonies 1.20.1 vers
Horizon 26.3 : leur machine à états, ce qu'ils demandent au monde, et pourquoi ils
s'arrêtent.

---

## 1. Premier point : deux prémisses de la mission ne tiennent pas

### 1.1 Le livreur et l'éleveur de porcs avaient déjà leur machine

La mission demande « une machine à états testable » pour les trois métiers. Deux existent
déjà, avec leurs tests :

| Métier | Ce qui existait | Mesure |
| --- | --- | --- |
| livreur | `crate::ai_build::delivery` | 1 089 lignes ; les 6 cibles des lignes `113-124`, `checkIfExecute` (`:638-661`), `decide` (`:591-631`), `prepareDelivery` (`:468-552`), `deliver` (`:327-466`), `pickup` (`:145-207`), `dump` (`:291-314`), `cannotHoldMoreItems` (`:264-276`), `maxParallelDeliveries` (`JobDeliveryman.java:511-518`), et sa machine `delivery::ai()` |
| éleveur de porcs | `crate::ai_work_09::herder` | 1 201 lignes ; les 8 cibles d'`AbstractEntityAIHerder.java:140-148`, les 7 décisions (`:199-249`, `:288-295`, `:302-328`, `:335-390`, `:419-484`, `:517-568`, `:575-588`), 33 refus nommés |

Et `EntityAIWorkSwineHerder` ne fait que **47 lignes**, dont trois surcharges : son
constructeur est `super(job)` (`:20-23`) et il n'enregistre **aucune cible**. Sa machine
*est* celle d'`AbstractEntityAIHerder`.

Réécrire ces deux tables aurait été la deuxième copie d'une table qui a déjà ses tests —
la règle 6 du port l'interdit. Ce module **raccorde** les deux, et le **prouve** : deux
tests de parité font tourner en parallèle la machine de ce module et celle du module
voisin, sur les mêmes faits, et comparent les états à chaque tick
(`tests::the_swine_table_answers_like_the_herder_table`,
`tests::the_courier_table_answers_like_the_delivery_table`). Le cuisinier, lui, est
vraiment neuf : 438 lignes dont **six cibles héritées du four et deux qui n'existaient
nulle part** (`AIWorkerState.java:344`, `:348`).

### 1.2 « chercher, aller, agir, rendre » n'est la forme d'aucune des trois tables

- **Éleveur de porcs** : quatre états de métier qui se chevauchent (accoupler, abattre,
  nourrir, ramasser) plus `PREPARING` / `DECIDE` et les états de base. Pas d'étape
  « rendre ».
- **Cuisinier** : son point d'entrée est `START_WORKING`, pas `IDLE` ; il a les six cibles
  du four **et** ses deux états de service, et c'est `startWorking` qui choisit entre les
  deux (`checkForImportantJobs`, `:338-412`).
- **Livreur** : un **dispatcheur** à quatre courses, dont deux sont des détours —
  `DUMPING` avant une livraison quand le sac n'est pas vide (`:615-621`) et avant une
  collecte quand il est plein (`:156-161`).

Les tables sont donc portées telles quelles, un état par cible Java. Aplatir sur quatre
cases aurait perdu `HERDER_BREED`, `PREPARE_DELIVERY`, `DUMPING` et les deux états du
cuisinier.

---

## 2. Ce que ce module apporte

### 2.1 Les tables, cible par cible

**Éleveur de porcs** — `swine::register`, 8 cibles + 4 retours :

| # | État | Règle | Cadence | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | → `START_WORKING` | 1 | `AbstractEntityAIHerder.java:140` |
| 2 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:141` |
| 3 | `PREPARING` | `prepareForHerding` | 20 | `:142` |
| 4 | `DECIDE` | `decideWhatToDo` | 80 | `:143` |
| 5 | `HERDER_BREED` | `breedAnimals` | 40 | `:144` |
| 6 | `HERDER_BUTCHER` | `butcherAnimals` | 20 | `:145` |
| 7 | `HERDER_PICKUP` | `pickupItems` | 20 | `:146` |
| 8 | `HERDER_FEED` | `feedAnimal` | 20 | `:147` |

**Cuisinier** — `cook::register`, 8 cibles + 2 retours :

| # | État | Règle | Cadence | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | → `START_WORKING` | 5 | `AbstractEntityAIUsesFurnace.java:77` |
| 2 | `START_WORKING` | `startWorking` | 60 | `:78` |
| 3 | `FILL_UP_FURNACES` | `fillUpFurnace` | 5 | `:79` |
| 4 | `AI_BLOCKING` | `accelerateFurnaces` (rend `null`, ne coupe jamais le tick) | 20 | `:80` |
| 5 | `RETRIEVING_END_PRODUCT_FROM_FURNACE` | `retrieveSmeltableFromFurnace` | 5 | `:81` |
| 6 | `RETRIEVING_USED_FUEL_FROM_FURNACE` | `retrieveUsedFuel` | 5 | `:82` |
| 7 | `COOK_SERVE_FOOD_TO_CITIZEN` | `serveFoodToCitizen` | 30 | `EntityAIWorkCook.java:96` |
| 8 | `COOK_SERVE_FOOD_TO_PLAYER` | `serveFoodToPlayer` | 30 | `:97` |

**Livreur** — `courier::register`, 6 cibles, chacune appelant `ai_build::delivery` :

| # | État | Règle | Cadence | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | → `START_WORKING` | 1 | `EntityAIWorkDeliveryman.java:117` |
| 2 | `START_WORKING` | porte `checkIfExecute` puis `decide` | 100 | `:118` |
| 3 | `PREPARE_DELIVERY` | `prepareDelivery` | 5 | `:119` |
| 4 | `DELIVERY` | `deliver` | 5 | `:120` |
| 5 | `PICKUP` | `pickup` | 5 | `:121` |
| 6 | `DUMPING` | `dump` | 20 | `:122` |

### 2.2 Le cuisinier, qui est le vrai travail neuf

Toutes les règles de `EntityAIWorkCook.java`, chacune avec sa ligne :

| Règle | Ligne | Fonction |
| --- | --- | --- |
| `isSmeltable` | `:130-134` | `cook::is_smeltable` |
| `reachedMaxToKeep` (surcharge) | `:136-147` | `cook::reached_max_to_keep`, `::max_food_to_keep`, `::food_kept_over_limit` |
| `requestSmeltable` | `:149-157` | `Start::ask_food` (+ `Refusal::NoFoodInMenu` comme nom) |
| `serveFoodToCitizen` | `:163-249` | `cook::serve_food_to_citizen` |
| `serveFoodToPlayer` | `:255-304` | `cook::serve_food_to_player` |
| `checkForImportantJobs` | `:338-412` | `cook::important_jobs` → `cook::ImportantJob` |
| `shouldBeFed` | `:419-425` | `cook::should_be_fed` |
| `getActionsDoneUntilDumping` | `:427-431` | `cook::ACTIONS_UNTIL_DUMP` = 1 |
| `getSmeltAbleClass` | `:433-437` | `cook::food_request` |
| la portion | `:236-239` | `cook::portion_for` |
| les items consommés sur place | `:192-211` | `cook::feed_items` |
| le transfert à un joueur | `:288-290` | `cook::plan_food_transfer` (`InventoryUtils.java:3222-3282`) |

**`startWorking` est la seule règle réécrite, et elle le doit** : le parent appelle
`checkForImportantJobs()` au milieu de son corps
(`AbstractEntityAIUsesFurnace.java:210-214`), et le cuisinier surcharge cette méthode.
`crate::ai_work_04::smelter::start_working` code la version du mineur et renverrait
`BREAK_ORES`. Ce qui est écrit ici est donc la **séquence** (sept étapes, dans l'ordre du
parent) ; chaque analyse est un appel au module voisin —
`oven_to_retrieve_fuel_from`, `oven_to_retrieve_from`, `check_if_able_to_smelt`,
`reached_max_to_keep`. Un test vérifie que les sous-règles répondent comme là-bas
(`cook::tests::the_furnace_half_is_the_one_ai_work_04_already_answered`).

### 2.3 Les constantes, chacune à sa ligne

| Constante | Valeur | Source |
| --- | ---: | --- |
| `TICKS_SECOND` | 20 | `Constants.java:21` |
| `STACKSIZE` | 64 | `Constants.java:23` |
| `SLOT_PER_LINE` | 9 | `Constants.java:163` |
| `STANDARD_DELAY` | 5 | `AbstractEntityAIBasic.java:100` |
| `SATURATION_TO_SERVE` | 16 | `EntityAIWorkCook.java:59` |
| `SERVE_DELAY` | 30 | `EntityAIWorkCook.java:64` |
| `LEVEL_TO_FEED_PLAYER` | 10 | `EntityAIWorkCook.java:69` |
| `FEEDING_ATTEMPTS` | 10 | `EntityAIWorkCook.java:192` |
| `PORTION_FACTOR` | 1,5 | `EntityAIWorkCook.java:239` |
| `LEVEL_TO_CHECK_MENU` | 3 | `EntityAIWorkCook.java:350` |
| `AVERAGE_SATURATION` | 10 | `CitizenConstants.java:43` |
| `FULL_SATURATION` | 60 | `CitizenConstants.java:53`, `ICitizenData.java:30` |
| `BASE_XP_GAIN` | 2,0 | `AbstractEntityAIUsesFurnace.java:55` |
| `START_WORKING_RATE` | 60 | `AbstractEntityAIUsesFurnace.java:78` |
| `ACTIONS_UNTIL_DUMP` | 1 | `EntityAIWorkCook.java:427-431` |
| `BUTCHERING_ATTACK_DAMAGE` | 5,0 | `AbstractEntityAIHerder.java:71` |
| `SKILL_PER_DAMAGE_POINT` | 10,0 | `EntityAIWorkSwineHerder.java:45` |
| `RENDER_META_WORKING` | `"working"` | `AbstractEntityAIInteract.java:44` |
| `RENDER_META_CARROT` | `"carrot"` | `EntityAIWorkRabbitHerder.java:25` |
| `DECISION_DELAY` | 100 | `EntityAIWorkDeliveryman.java:64-67` (repris de `ai_build::delivery`) |
| `PICKUP_DELAY` | 5 | `EntityAIWorkDeliveryman.java:72` (repris) |
| `PRIORITY_FORCING_DUMP` | 10 | `EntityAIWorkDeliveryman.java:80-82` (repris) |
| `MIN_DISTANCE_TO_WAREHOUSE` | 5 | `EntityAIWorkDeliveryman.java:60-62` |
| `SLOT_HAND` | 0 | `EntityAIWorkDeliveryman.java:74-77` |

### 2.4 Les refus nommés

`state::Refusal` a **quatorze** variantes propres au cuisinier et aux erreurs d'appelant,
plus **trois** enveloppantes qui délèguent aux modules propriétaires :
`Refusal::Herd(WorkRefusal)` (33 raisons), `Refusal::Furnace(ai_work_04::states::Refusal)`
(24 raisons), `Refusal::Courier(ai_build::delivery::Refusal)` (2 portes). Une raison
recopiée serait une deuxième vérité pour une branche : `Refusal::java()` et `::key()`
délèguent, et un test le vérifie
(`state::tests::a_wrapped_refusal_keeps_the_other_modules_line_and_key`).

Les refus propres couvrent ce que la mission demandait — « rien à faire » (file vide,
`CitizenQueueEmpty`, `PlayerQueueEmpty`, `FeedNotDue` chez l'éleveur), « outil cassé »
(`NoAxe` et `ToolMissing`, chez l'éleveur), « inventaire plein »
(`TargetInventoryFull`, `PlayerInventoryFull`, et `InventoryFull` du four).

---

## 3. La forme du code, et pourquoi

| Fichier | Lignes | Rôle |
| --- | ---: | --- |
| `mod.rs` | 505 | `Trade`, `TradeAi`, `TradeOutcome`, `Decision`, `Effect`, `dump_due`, `at_hut` |
| `state.rs` | 512 | `TradeState` (4 variantes enveloppantes), `CookState`, `Refusal` |
| `facts.rs` | 623 | `SwineFacts`, `CookFacts`, `ServeCitizen`, `ServePlayer`, `FoodStack`, `CookMemory`, `TradeJob`, `Scene` |
| `cook.rs` | 1 658 | les douze règles du cuisinier et ses huit cibles |
| `swine.rs` | 417 | les trois surcharges du porc et ses huit cibles |
| `courier.rs` | 393 | les six cibles du livreur et la porte nommée |
| `tests.rs` | 759 | les 15 scénarios, les 2 tests de parité et les 9 invariants de table |
| **total** | **4 867** | **81 tests** (`#[test]`) |

**L'écart avec la cible de 900-1 400 lignes est assumé et mesuré** : les voisins de ce
module font la même taille pour le même périmètre — `src/ai_work_09` = 4 077 lignes pour
trois métiers, `src/ai_work_04` ≈ 5 000 — parce que la densité de documentation
(`fichier.java:ligne` sur chaque règle) et de tests est le style de la maison. 81 tests
pour 3 tables, c'est 27 par table ; le cuisinier seul en porte 34.

**Pourquoi les états sont enveloppés et non recopiés.** `Machine<S, F>` est monorphe en
`S` : une machine ne peut pas *contenir* celle d'un autre module. Une **variante** peut
contenir son énumération d'états — `TradeState::Herd(ai_work_09::WorkState)`,
`::Furnace(ai_work_04::SmeltState)`, `::Courier(ai_build::DeliveryState)`,
`::Cook(CookState)` — et c'est tout ce dont un appelant a besoin. Les 19 + 8 + 6 états des
voisins ne sont pas redits, et `TradeState::java_name()`/`okay_to_eat()`/`base()`
délèguent.

**Pourquoi `Effect` existe.** Une action d'IA ne rend qu'un état, alors que Java écrit dans
le monde au passage. Trois branches décident d'une **quantité** qui ne se relit nulle part
(la portion d'un citoyen, les items mangés sur place, les items donnés à un joueur) : sans
`Effect`, un appelant devrait redeviner la formule et un test ne pourrait pas l'assertir.
Les transferts de four, eux, ne sont pas dans `Effect` : ce qui est entré se relit dans
`SmelterFacts::furnaces`.

**Une décision de fidélité, assumée.** La porte du livreur (`checkIfExecute`) est vérifiée
**dans l'action** et non comme `Transition::target_when`, parce qu'une `Condition` reçoit
les faits en lecture seule et ne peut donc pas nommer le refus. La suite d'états est
identique (`START_WORKING` n'a qu'une transition), mais un joueur reçoit
`Refusal::Courier(NoWarehouse)` au lieu de rien. C'est documenté en tête de `courier.rs`
et fixé par un test.

---

## 4. Les tests

81 tests, dont **15 scénarios** qui vérifient une *suite* d'états et non une valeur :

| Test | Ce qu'il prouve |
| --- | --- |
| `the_pig_walks_idle_preparing_decide_breed_then_idle` | les 5 états d'un accouplement, dans l'ordre, et `breedTimeOut = 1200` |
| `four_adults_make_the_pig_butcher_instead_of_breeding` | `chanceToButcher` = 8 au niveau 1, l'abattage gagne avant l'accouplement, la bête meurt (`:383`) |
| `a_pig_that_has_not_arrived_yet_keeps_walking` | `walkingToAnimal` : rien n'est compté tant qu'il n'est pas à portée |
| `the_swine_table_answers_like_the_herder_table` | **parité** avec `ai_work_09::WorkAi`, 8 ticks comparés |
| `the_cook_serves_a_hungry_citizen_then_goes_back_to_the_furnace` | la file, la marche, la portion (18 items), le retour |
| `the_cook_serves_a_player_and_fills_sixteen_saturation_per_level` | 2 niveaux × 16 = 32 de saturation → 8 items de 4 |
| `the_cook_reaches_both_of_its_own_states_and_the_two_furnace_retrievals` | les 2 états neufs + `:81` (+1 action) + `:82` (0 action) |
| `a_cook_with_a_full_backpack_is_sent_dumping_by_the_base_rule` | le seuil est celui du métier, la transition est celle de la base |
| `the_courier_table_answers_like_the_delivery_table` | **parité** avec `ai_build::delivery::ai()`, 6 ticks |
| `a_courier_without_a_warehouse_stays_stuck_and_says_why` | la porte, et son refus nommé |
| `a_courier_empties_a_hut_then_dumps_when_the_request_was_important` | priorité ≥ 10 → `DUMPING` |
| `every_transition_of_every_table_names_its_java_line` | **la règle 3, mécaniquement** : aucun `Transition` sans `fichier.java:ligne` |
| `no_table_falls_into_a_trap_on_a_thirty_tick_walk` | aucun piège (`BasicStateMachine.java:220-228`) |
| `the_same_facts_replayed_twice_give_the_same_states` | les tirages sont des faits, donc le scénario est rejouable |
| `a_scene_always_starts_with_no_refusal_and_no_effect` | les deux champs volatils sont effacés par tick |

Les 66 autres tests couvrent chaque règle une par une : les formules
(`portion_for`, `feed_items`, `plan_food_transfer`, `chance_to_butcher` via le module
voisin, `butchering_attack_damage`), les six branches de `should_be_fed` et de
`important_jobs`, les sept sorties de `serve_food_to_citizen` et les six de
`serve_food_to_player`, le seuil de nourriture, les drapeaux `isOkayToEat` des quatre
tables, et chaque refus (« chaque refus cite un fichier Java »).

---

## 5. Ce que le premier build a corrigé

`cargo check --all-targets` a rendu **cinq** erreurs sur ce module, toutes corrigées :

1. **`E0428` — `mod tests` déclaré deux fois.** `mod.rs` avait à la fois
   `#[cfg(test)] mod tests;` (le fichier `tests.rs`) et un module `mod tests { … }` en fin
   de fichier. Les neuf tests de ce module inline (le pont `Trade`, la forme d'une
   `Decision`, la quantité d'un `Effect`, le seuil de vidage) ont été déplacés dans
   `tests.rs`, sous « la table, pas la règle » : un seul module `tests` reste déclaré.
2. **`E0433` ×3 — `TradeState` non importé dans `tests.rs`.** L'import manquait depuis la
   réécriture du fichier ; il est là, avec `dump_due`, `Decision` et `Effect` dans le même
   bloc `use super::{…}`.
3. **`E0277` — `ai_build::delivery::Refusal` n'implémente pas `Hash`.** [`state::Refusal`]
   dérivait `Hash` pour envelopper ce type : la dérivation est tombée (et c'est le bon
   choix — un refus est une raison, pas une clé de table de hachage ; rien ici ne le
   hache). `TradeState` garde son `Hash`, ses quatre énumérations enveloppées l'ont.

`cargo check` n'a alors plus rien à dire de ce module. Aucun test n'a été modifié pour
passer : les deux corrections de test sont un **déplacement de module** et un **import
manquant**, pas un changement d'attente.

## 6. Ce que `cargo test` a corrigé — cinq attentes fausses, aucune règle changée

Cinq tests ont échoué, et dans les cinq cas **c'est l'attente du test qui était fausse**,
pas le code : les trois tests de couture et les deux du cuisinier contredisaient une ligne
du Java. Les corrections sont donc dans les attentes, chacune avec sa preuve.

### 6.1 Le cuisinier sort la tête de file **avant** de savoir s'il peut servir

| Test | Attente écrite | Ce que le Java fait |
| --- | --- | --- |
| `a_plate_that_does_not_fit_the_citizen_leaves_the_queue_alone` | la file garde `[4]` | `:184` `final AbstractEntityCitizen citizen = citizenToServe.poll();` — le poll est **après** le test de marche `:179-182` et **avant** le test de plat `:220-229` |
| `a_player_with_a_full_bag_or_left_the_hut_is_dropped` | la file garde `[7]` | `:275` `final Player player = playerToServe.poll();` — avant `isItemHandlerFull(handler)` de `:279` |

Donc, quand `foodSlot == -1` (`:221`) ou quand le sac du joueur est plein (`:279`), le
citoyen est **déjà** sorti de la file et il rentre par `checkForImportantJobs` au tick
suivant (qui le ré-ajoute, doublon compris — voir `MANQUANT.md` §3.1). Les deux tests
attendent maintenant la file vide, et le disent avec la ligne.

Le premier test est renommé `a_plate_that_does_not_fit_the_citizen_is_dropped_without_serving` :
« leaves the queue alone » était le nom d'un comportement que le Java n'a pas.

### 6.2 Le livreur ne prend **aucune** décision pendant cent ticks

`DECISION_DELAY = TICKS_SECOND * 5` (`EntityAIWorkDeliveryman.java:64-67`) est la cadence
de `new AITarget(START_WORKING, this::checkIfExecute, this::decide, DECISION_DELAY)`
(`:118`). Trois tests réutilisaient la **même machine** pour un second scénario juste après
que `decide` a tiré : le compte à rebours était à 100, la cible n'était plus regardée, et
l'état restait `START_WORKING`.

C'est la cadence du Java, pas un défaut : les trois tests
(`the_door_refuses_before_any_decision`, `decide_dumps_a_loaded_backpack_before_a_delivery`,
`nothing_to_do_loiters_at_the_warehouse_and_dumps_if_loaded`) construisent maintenant une
machine neuve par scénario, comme le fait un citoyen qui vient d'être embauché. Tester
autrement aurait demandé de contourner `DECISION_DELAY`, c'est-à-dire de ne plus tester le
Java.

## 7. État de vérification — à lire avant de merger

**Je n'ai lancé aucune compilation**, conformément à la consigne du poste de travail (« tu ne
lances AUCUNE compilation, l'orchestrateur compile et te renverra les erreurs exactes »),
qui prime sur l'invitation de `AGENTS.md` à utiliser `port-check`. La machine a gelé deux
fois sur des builds concurrents. Ce module est donc **non compilé** à l'heure de ce
rapport, et voici ce qu'il faut regarder en premier si le build tombe :

1. **Les chemins d'import vers les voisins.** Ce module lit
   `crate::ai::{facts, state, worker}`, `crate::ai_work_04::{facts, smelter, states}`,
   `crate::ai_work_09::{facts, herder, state, Decision, WorkAi, WorkJob, WorkTrade}`,
   `crate::ai_build::delivery` et `crate::jobs::job::Job`. Tous ces éléments étaient `pub`
   à la lecture ; si un voisin a bougé depuis, c'est là que ça cassera.
2. **`ai_work_09::herder` et `ai_work_04::smelter` : fonctions `pub(crate)`/`pub`.** Les
   sept décisions d'élevage (`decide_what_to_do`, `butcher_animals`, …) sont appelées
   directement avec leurs quatre arguments `(&mut WorldFacts, &HerdFacts, &mut
   WorkMemory, WorkState)` ; les scans de four (`fill_up_furnace`, `retrieve_product`,
   `retrieve_used_fuel`, `check_if_able_to_smelt`, `oven_to_retrieve_from`,
   `oven_to_retrieve_fuel_from`, `reached_max_to_keep`) avec `&SmelterFacts`.
3. **`Scene` dans `Scene`.** `Scene::effect` est un `Option<Effect>` ajouté pour porter les
   trois quantités ; la macro `cook_action!` déstructure les cinq champs
   (`job`, `memory`, `refusal`, `effect`, `..`) — c'est le point d'emprunt le plus
   délicat du module.
4. **Deux `unwrap`/`expect` : aucun.** Pas une seule occurrence dans le module ; les deux
   erreurs d'appelant (faits d'un autre métier, file sans observation de la tête) sont des
   refus nommés.
5. **Aucune API inventée.** Le module n'importe pas `mod_api` : il ne dépend que de types
   du dépôt. Les trous d'API sont dans `MANQUANT.md`.

Si un test échoue, il se tranche **contre le Java** : chaque assertion cite la ligne qui la
décide, et les deux calculs de quantité (`portion_for`, `feed_items`) portent l'ordre exact
des conversions de `EntityAIWorkCook.java:237-239`.
