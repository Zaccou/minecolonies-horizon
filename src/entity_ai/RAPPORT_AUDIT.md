# RAPPORT_AUDIT — relecture de fidélité de `src/entity_ai`

Périmètre : `core/entity/ai/` (hors métiers). Méthode : pour chaque méthode retenue,
ouverture du Java à la ligne exacte, lecture du corps, comparaison au Rust, verdict.
**40 méthodes** auditées sur 13 fichiers Java. `fichier.java:ligne` et
`fichier.rs:ligne` des deux côtés.

Trois verdicts :

- **fidèle** — mêmes bornes, même ordre de branches, même refus, même formule ;
- **divergent** — la règle existe des deux côtés mais ne dit pas la même chose ;
- **absent** — la règle Java n'a pas d'équivalent (le plus souvent faute de primitive
  ou de fait ; voir `MANQUANT.md`).

Chaque divergence réelle a été corrigée dans mes fichiers, la correction portant sa
ligne Java. Aucune compilation, aucun `git`.

---

## 1. Le tableau des 40 méthodes

| # | Méthode Java | Ligne | Rust | Ligne | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `EntityAIEatTask.getFood` | `:169-177` | `eat::get_food` | `eat.rs:138` | fidèle |
| 2 | `EntityAIEatTask.eat` | `:184-243` | `eat::eat` | `eat.rs:151` | **divergent → corrigé** (a) |
| 3 | `EntityAIEatTask.getFoodYourself` | `:250-278` | `eat::get_food_yourself` | `eat.rs:202` | **divergent → corrigé** (b) |
| 4 | `EntityAIEatTask.goToEatingPlace` | `:285-324` | `eat::go_to_eating_place` | `eat.rs:235` | fidèle |
| 5 | `EntityAIEatTask.waitForFood` | `:350-386` | `eat::wait_for_food` | `eat.rs:277` | **divergent → corrigé** (c) |
| 6 | `EntityAIEatTask.goToHut` | `:393-442` | `eat::go_to_hut` | `eat.rs:304` | **divergent → corrigé** (d) |
| 7 | `EntityAIEatTask.goToRestaurant` | `:449-473` | `eat::go_to_restaurant` | `eat.rs:341` | fidèle (corrigé au tour précédent) |
| 8 | `EntityAIEatTask.searchRestaurant` | `:480-512` | `eat::search_restaurant` | `eat.rs:367` | **divergent → corrigé** (e) |
| 9 | `EntityAIEatTask.hasFood` | `:519-546` | `eat::has_food` + `eat::complaint` | `eat.rs:108` / `:118` | fidèle |
| 10 | `EntityAISleep.walkHome` | `:114-133` | `sleep::walk_home` | `sleep.rs:105` | fidèle |
| 11 | `EntityAISleep.findBed` | `:140-148` | `sleep::find_bed` | `sleep.rs:122` | fidèle (condition inversée respectée) |
| 12 | `EntityAISleep.findBedAndTryToSleep` | `:159-220` | `sleep::find_bed_and_try_to_sleep` | `sleep.rs:131` | **divergent → corrigé** (f) |
| 13 | `EntityAISleep.sleep` | `:225-243` | `sleep::sleep` | `sleep.rs:181` | fidèle |
| 14 | `EntityAISickTask.checkForCure` | `:445-472` | `sick::check_for_cure` | `sick.rs:102` | fidèle |
| 15 | `EntityAISickTask.applyCure` + `cure` | `:237-273`, `:278-306` | `sick::apply_cure` | `sick.rs:125` | **divergent → corrigé** (g) |
| 16 | `EntityAISickTask.waitForCure` | `:313-364` | `sick::wait_for_cure` | `sick.rs:149` | fidèle |
| 17 | `EntityAISickTask.findEmptyBed` | `:165-230` | `sick::find_empty_bed` | `sick.rs:188` | fidèle |
| 18 | `EntityAISickTask.goToHut` | `:371-386` | `sick::go_to_hut` | `sick.rs:243` | fidèle |
| 19 | `EntityAISickTask.searchHospital` | `:413-438` | `sick::search_hospital` | `sick.rs:271` | fidèle |
| 20 | `EntityAIMournCitizen.decide` | `:285-322` | `mourn::decide` | `mourn.rs:92` | fidèle |
| 21 | `EntityAIMournCitizen.wanderAtGraveyard` | `:154-209` | `mourn::wander_at_graveyard` | `mourn.rs:175` | fidèle |
| 22 | `EntityAIMournCitizen.stare` | `:247-278` | `mourn::stare` | `mourn.rs:142` | fidèle |
| 23 | `EntityAICitizenWander.canUse` | `:293-297` | `idle::can_use` | `idle.rs:92` | fidèle |
| 24 | `EntityAICitizenWander.decide` | `:259-291` | `idle::decide` | `idle.rs:97` | fidèle |
| 25 | `EntityAICitizenWander.wanderAtLeisureSite` | `:150-257` | `idle::wander_at_leisure_site` | `idle.rs:129` | **divergent → corrigé** (h) |
| 26 | `EntityAICitizenWander.readABook` | `:88-131` | `idle::read_a_book` | `idle.rs:216` | **divergent → corrigé** (i) |
| 27 | `EntityAICitizenChild.isReadyForActivity` | `:127-140` | `idle::is_ready_for_activity` | `idle.rs:312` | fidèle |
| 28 | `EntityAICitizenChild.tryGrowUp` | `:264-303` | `idle::try_grow_up` | `idle.rs:328` | fidèle |
| 29 | `EntityAICitizenChild.visitHuts` | `:215-257` | `idle::visit_huts` | `idle.rs:406` | fidèle |
| 30 | `EntityAICitizenAvoidEntity.isEntityClose` | `:118-138` | `avoid::is_entity_close` | `avoid.rs:108` | fidèle |
| 31 | `EntityAICitizenAvoidEntity.updateMoving` | `:211-244` | `avoid::update_moving` | `avoid.rs:139` | divergent assumé (j) |
| 32 | `EntityAICitizenAvoidEntity.getMoveAwayDist` | `:193-204` | `avoid::move_away_dist` | `avoid.rs:97` | fidèle |
| 33 | `EntityAIInteractToggleAble.canUse` | `:118-146` | `avoid::toggle_can_use` | `avoid.rs:250` | fidèle (post-décrément corrigé au tour précédent) |
| 34 | `EntityAIInteractToggleAble.tick` | `:398-462` | `avoid::toggle_tick` | `avoid.rs:305` | fidèle (porte du tick corrigée au tour précédent) |
| 35 | `TargetAI.isEntityValidTarget` | `:91-104` | `combat::is_entity_valid_target` | `combat.rs:159` | fidèle |
| 36 | `TargetAI.searchNearbyTarget` | `:135-170` | `combat::search_nearby_target` | `combat.rs:270` | fidèle |
| 37 | `TargetAI.getSearchArea` | `:188-201` | `combat::search_box` | `combat.rs:234` | fidèle |
| 38 | `TargetAI.getYSearchRange` (+ `RangeCombatAI:420-428`, `DruidCombatAI:336-345`) | `:208-211` | `combat::y_search_range` + `search_box` | `combat.rs:221` / `:234` | **absent → ajouté** (k) |
| 39 | `AttackMoveAI.move` | `:55-107` | `combat::move_step` | `combat.rs:291` | fidèle |
| 40 | `AttackMoveAI.tryAttack` | `:133-154` | `combat::try_attack` | `combat.rs:347` | fidèle |
| 41 | `MeleeCombatAI.getAttackDamage` | `:310-352` | `combat::melee_attack_damage` | `combat.rs:434` | **divergent → corrigé** (l) |
| 42 | `MeleeCombatAI.getAttackDelay` | `:360-366` | `combat::melee_attack_delay` | `combat.rs:422` | fidèle (division entière) |
| 43 | `RangeCombatAI.getAttackDistance` | `:230-258` | `combat::range_attack_distance` | `combat.rs:465` | fidèle |
| 44 | `AbstractEntityAIGuard.shouldSleep` | `:268-290` | `guard::should_sleep` | `guard.rs:200` | fidèle |
| 45 | `AbstractEntityAIGuard.shouldFlee` | `:353-361` | `guard::should_flee` | `guard.rs:253` | fidèle (voir §4) |
| 46 | `AbstractEntityAIGuard.regen` | `:368-385` | `guard::regen` | `guard.rs:268` | fidèle |
| 47 | `AbstractEntityAIGuard.decide` | `:664-720` | `guard::decide` | `guard.rs:407` | **divergent → corrigé** (m) |
| 48 | `AbstractEntityAIGuard.patrol` | `:510-542` | `guard::patrol` | `guard.rs:366` | **divergent → corrigé** (n) |
| 49 | `AbstractEntityAIGuard.getPersecutionDistance` | `:757-773` | `guard::persecution_distance` | `guard.rs:457` | fidèle |
| 50 | `AbstractEntityAIGuard.wakeUpGuard` | `:229-261` | `guard::wake_up_guard` | `guard.rs:301` | fidèle |
| 51 | `AbstractEntityAIGuard.startWorkingAtOwnBuilding` | `:487-494` | `guard::machine` règle `start_working` | `guard.rs:523`+ | **divergent → corrigé** (o) |
| 52 | `AbstractEntityAIFight.prepare` | `:118-143` | `guard::machine` règle `prepare` | `guard.rs:523`+ | divergent assumé (p) |
| 53 | `EntityAIArcherTraining.selectTarget` | `:114-133` | `training::select_target` | `training.rs:168` | fidèle |
| 54 | `EntityAICombatTraining.trainWithPartner` | `:143-157` | `training::train_with_partner` | `training.rs:246` | **divergent → corrigé** (q) |
| 55 | `EntityAICombatTraining.attack` | `:164-234` | `training::attack_partner` | `training.rs:263` | fidèle |
| 56 | `ReturnToStableGoal.canUse` | `:127-159` | `cavalry::can_use` | `cavalry.rs:189` | fidèle |
| 57 | `EntityAIVisitor.decide` | `:164-191` | `visitor::decide` | `visitor.rs:124` | fidèle (précédence `\|\|`/`&&` vérifiée) |
| 58 | `EntityAIVisitor.reduceTime` | `:93-107` | `visitor::reduce_time` | `visitor.rs:111` | fidèle |

58 lignes : les 40 premières sont les méthodes « porteuses de règle » demandées, les
suivantes sont les formules et les cas de garde vérifiés dans la même passe.

## 2. Divergences trouvées et corrigées

**(a) `eat` — l'item mangé** (`EntityAIEatTask.java:218-222`). J'enregistrais dans
`eatenFood` / `lastEaten` la pile du **menu du restaurant** ; Java prend
`foodStack.getItem()`, la pile **du slot** (`:192`). Ajout de
`FoodFacts::best_item` (`facts.rs`) et `eat` l'utilise (`eat.rs:151`).
Second point dans la même méthode : le refus `!FoodUtils.canEat(stack, maison, hut)`
(`:193-196`) manquait — ajout de `FoodFacts::can_eat` et de la branche
« retour à `CHECK_FOR_FOOD` ».

**(b) `getFoodYourself`** (`:261-266`). Je testais `restaurant.we_are_inside` là où
Java teste le résultat de `walkToBuilding(citizen, cookBuilding)` : un citoyen
*arrivé devant* le restaurant repartait en boucle. Remplacé par
`RestaurantFacts::walk_arrived` (`eat.rs:202`).

**(c) `waitForFood`** (`:354`). La recherche de `waitForFood` n'a **pas** le prédicat
`STAFFED_RESTAURANTS` : elle prend le meilleur cook, point. Je réutilisais `pos`
(la recherche dotée). `wait_for_food` prend maintenant `fallback.or(pos)`
(`eat.rs:277`).

**(d) `goToHut`** (`:396-399`). La branche `!WorldUtil.isBlockLoaded(level,
buildingWorker.getPosition())` → `SEARCH_RESTAURANT` manquait. Ajout de
`HomeFacts::work_loaded` (défaut `true`) et de la garde (`eat.rs:304`).

**(e) `searchRestaurant`** (`:488-492`). Java fait **deux** recherches : un cook
*doté* d'un cuisinier, puis n'importe quel cook. J'avais un champ `staffed: bool`
jamais lu — c'est-à-dire un mensonge. Remplacé par `RestaurantFacts::fallback`
(la seconde recherche) et `search_restaurant` fait `pos.or(fallback)`
(`eat.rs:367`).

**(f) `findBedAndTryToSleep`** (`:165`, `:184-189`). Deux écarts : Java ne
re-sélectionne le lit que si `usedBed == null || usedBed == homePos` (le repli sur la
position de la maison est donc réessayé à chaque tick), et quand une couchette
utilisable est trouvée il **`return`** — la marche n'a lieu qu'au tick suivant. Les
deux sont maintenant dans `sleep.rs:131`.

**(g) `cure`** (`:293-301`). Il manquait `registerPatient(usedBed, 0)` : le lit
d'hôpital n'était jamais rendu. `apply_cure` renseigne `patient_released`
(`sick.rs:125`).

**(h) `wanderAtLeisureSite`** (`:167-178`) — **la plus grosse divergence de la
passe**. `READ_A_BOOK` est un **`else if`** derrière la balade aléatoire
`nextInt(10) <= 0` de `:167`. Mon code traitait le tirage comme un simple
aiguillage de siège et rendait `READ_A_BOOK` dans le cas minoritaire inverse. Réécrit
dans l'ordre de Java : fin de visite (`:152`, qui efface **les deux** champs),
marche en cours (`:159`), pas de données de blueprint (`:256`), balade 1-sur-10
(`:167`), **puis** livre 5-sur-100 (`:172`), puis choix de place (`:181-242`), puis
`sitDown` à l'arrivée (`:243-251`).

**(i) `readABook`** (`:118-131`). Je choisissais une *place assise* comme destination
de lecture ; Java appelle `library/uiversity.getRandomBookShelf()`. Ajout de
`WanderFacts::book_shelf` et `read_a_book` l'utilise (`idle.rs:216`).

**(j) `updateMoving`** (`:218-223`) — **divergence assumée, non corrigée**. Java
compare `newClosest.getId() != closestLivingEntity.getId()` ; je compare des
**positions**, faute d'identifiant d'entité dans les faits. Deux entités distinctes au
même bloc ne déclencheraient donc pas `callForHelp` chez moi. C'est écrit dans
`avoid.rs:139` et dans le test ; la primitive manquante est un id d'entité
(`MANQUANT.md` §4).

**(k) `getYSearchRange`** (`TargetAI.java:208-211`, `RangeCombatAI.java:420-428`,
`DruidCombatAI.java:336-345`) — **absent**. Un ranger ou un druide **à son poste**
(`GUARD`) cherche sur `Y_VISION + 25` = 28 blocs de haut, un patrouilleur sur
`Y_VISION` = 3. Ma boîte utilisait 3 partout. Ajout de `combat::y_search_range`
(`combat.rs:221`), du fait `CombatFacts::y_vision` et de son usage dans `search_box`
(`combat.rs:234`).

**(l) `getAttackDamage`** (`:316-335`). `BASE_PHYSICAL_DAMAGE` n'est ajouté que
**dans** la branche `doesItemServeAsWeapon(heldItem)` — un garde qui tient autre chose
qu'une arme ne gagne pas ces 3 points. Je les ajoutais toujours. Le bonus d'arme est
devenu un `Option<f64>` (`combat.rs:434`) : `None` ⇒ 0 point de base.

**(m) `decide`** (`:668`). `if (regularActionTimer++ > ACTION_INCREASE_INTERVAL)`
compare la valeur **avant** incrément : la remise à zéro tombe au douzième passage, pas
au onzième. Corrigé (`guard.rs:407`).

**(n) `patrol`** (`:535-539`). La condition d'arrivée est
`!isEntityBlockLoaded(...) || walkToSafePos(...)`, et les deux appellent
`setCurrentDelay(10)` puis `arrivedAtPatrolPoint(worker)`. Je ne regardais que le
chunk déchargé : un garde dont le chunk était chargé ne signalait jamais son arrivée.
Corrigé (`guard.rs:366`).

**(o) `startWorkingAtOwnBuilding`** (`:487-494`) — divergence de **hiérarchie**. La
classe parente (`AbstractEntityAIFight.java:92-99`) marche jusqu'au hut puis rend
`PREPARING` ; `AbstractEntityAIGuard` **écrase** la méthode et rend `DECIDE` sans
marcher, après avoir posé `setTempNextPatrolPoint(getPosition())`. Ma machine suivait
la version parente : un garde qui rentre passait par `PREPARING` (ré-équipement)
au lieu de décider. Corrigé (`guard.rs:523`+, champ `Scene::temp_patrol_point`), et la
table `GUARD_TABLE` le dit maintenant (`goals.rs`, règle 2).

**(q) `trainWithPartner`** (`:151`). La condition est
`distance > 5 && !walkToWorkPos(...)` : si la marche **a abouti**, le garde frappe.
Je m'arrêtais sur la distance seule. Ajout de `TrainingFacts::partner_walked` et de la
double condition (`training.rs:246`).

## 3. Ce que je n'ai pas corrigé, et pourquoi

**(p) `AbstractEntityAIFight.prepare`** (`:118-143`). Java : pour chaque outil, si
`checkForToolOrWeapon` attend encore, on **reste** en `PREPARING` (`:122-125`) ; puis
`equipInventoryArmor()` ; puis **si le garde est à plus de `distSqr > 50` du hut** →
`DECIDE` sans aller plus loin (`:136-139`) ; sinon `atBuildingActions()` → `DECIDE`.
Mon `prepare` se résume à « outil en main → `DECIDE`, sinon rien ». Il manque la
branche des 50 blocs, qui demande un fait de distance au hut ; je ne l'ai pas
inventée. Les deux autres branches sont couvertes autrement (`has_tool`,
`regardable` via `atBuildingActions` absent).

Autres points relevés mais **sans effet sur une décision** (aucune correction) :

- `EntityAIEatTask.waitForFood:359` — `storeCustomer(citizenData)` : effet de bord sur
  la file du cuisinier, aucune règle de l'IA n'en dépend. Absent.
- `EntityAISickTask.applyCure:250-254` — `setItemInHand` de la pile de remède :
  visuel seulement. `Scene::cure_index_roll` est là pour ça, non consommé.
- `EntityAIArcherTraining.shoot:194-197` — la pièce `nextBoolean()` qui use l'arc :
  pas de modèle de durabilité. Absent.
- `AbstractEntityAIGuard.shouldFlee:355` — Java mesure
  `distanceToSqr(building.getID().getCenter())` (centre du hut, en double) ; je mesure
  `dist_sqr` sur le bloc du hut. Écart d'au plus ~1 bloc sur un seuil de 20.
- `EntityAICitizenWander.wanderAtLeisureSite:191` — la préférence intérieur/extérieur
  selon la pluie est modélisée par le filtre `candidate_seats`, pas par la cascade
  exacte des quatre listes (dedans assis / dedans debout / dehors assis / dehors
  debout). L'issue est la même, l'ordre des tirages n'est pas identique.
- `readABook:118-131` — si le bâtiment n'est ni bibliothèque ni université, Java laisse
  `walkTo` nul et boucle ; je replie sur la position du site pour ne pas boucler.

## 4. Ce que je n'ai pas pu vérifier

- **Les tirages.** Vérifier `nextInt(60 * 5) < 1` suppose de connaître la distribution ;
  j'ai vérifié le **seuil** (`< 1`, `<= 0`, `== 1`, `> 10`) et la borne, pas la suite
  produite. Les tirages restent des faits fournis par l'appelant.
- **Les valeurs de constantes hors de ma tranche** :
  `JobCavalry.DININGHALL_HORSE_PARKING_RANGE` (40, ligne 37 lue) et
  `Constants.GLOW_EFFECT_DURATION` (600, ligne 168) sont vérifiés ; en revanche les
  seuils de recherche (`getBuildingLevelEquivalent`, `MAX_BUILDING_LEVEL`) viennent
  du module bâtiment et n'ont pas été relus.
- **`EntityAIInteractToggleAble.getHeightToCheck` / `checkPathBlocksCollided`**
  (`:168-334`) : ce sont des calculs sur `Path` vanilla (nœuds, collisions). Sans
  accès au chemin dans les faits, je n'ai vérifié que le contrat (`positions` entrantes
  → `Toggled` sortant), pas la construction de la liste.
- **`EntityAIFloat.tick`** (`:81-117`) : relu, mais la partie
  `PathJobEscapeWater`/`setPauseTicks` est de la navigation et reste hors du modèle
  (le port ne rend que `Escape` / `StopEscape`).
- **Le rendu et les particules** de tout le module : hors périmètre, non relus.
- Les méthodes de `workers/util/*` (itérateurs de blueprint) : elles appartiennent au
  builder, déjà couvert par `src/ai/builder.rs`, et n'ont pas été réauditées ici.

## 5. Effet sur les tests

Les corrections de comportement ont demandé de mettre à jour **mes** tests, chaque
fois avec la ligne Java qui contredit l'ancienne attente :

| Test | Ancienne attente | Nouvelle | Preuve |
| --- | --- | --- | --- |
| `idle::a_library_visit_can_end_in_a_book_and_a_plain_hut_cannot` | livre sur `spot_roll = 0` | livre seulement hors du tirage 1-sur-10, et jamais dans un hut ordinaire | `EntityAICitizenWander.java:167-178` |
| `idle::five_in_a_hundred_…` (helper) | `spot_roll = 0` par défaut | `3`, pour ne pas déclencher la balade aléatoire | `:167` |
| `sleep::a_block_that_is_no_longer_a_bed_…` | repli sur la position de la maison | `used_bed = None` : Java `return` après `removeBed` | `:178-182` |
| `sleep::a_failed_walk_…`, `a_bed_somebody_else_…`, `a_refused_sleep_…` | lit implicite | lit déjà choisi, la marche n'a lieu qu'au tick suivant | `:184-189` |
| `guard::the_regular_action_counter_…` | remise à zéro à 10 | à 11 (post-incrément) | `:668` |
| `guard::a_guard_without_a_weapon_lands_on_preparing_…` | `START_WORKING → PREPARING` | `START_WORKING → DECIDE → NO_TARGET → PREPARING` | `AbstractEntityAIGuard.java:487-494` |
| `combat::a_badly_hurt_knight_hits_twice_as_hard` & co | bonus d'arme `f64` | `Option<f64>`, et `None` ⇒ 0 | `MeleeCombatAI.java:316-335` |

Nouveaux tests ajoutés pour couvrir les règles qui viennent d'être portées :
`idle::a_reached_seat_is_sat_on_and_the_walk_is_forgotten`,
`idle::a_standing_spot_is_reached_without_sitting`,
`combat::a_ranger_at_its_post_searches_twenty_five_blocks_higher`.
