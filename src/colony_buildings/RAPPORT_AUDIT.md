# Audit de fidélité — `src/colony_buildings`

Relecture ligne à ligne de **40 méthodes distinctes**, réparties sur **36
fichiers Java** de `core/colony/buildings/`, contre la tranche Rust — en priorité
celles qui portent une règle : bornes, ordres, refus, durées, formules. Le
tableau du §1 a 48 lignes parce que les quatre portées de chunk comptent pour
quatre entrées d'une même famille.

Méthode : le Java est ouvert à la ligne citée, la valeur ou la condition est
relue, puis comparée à la fonction Rust. Les verdicts :

| Verdict | Sens |
| --- | --- |
| **fidel** | même condition, même valeur, même borne |
| **fidel (vue)** | la règle de Java est côté client ; le port l'a déplacée côté serveur faute de second côté |
| **partiel** | la règle est là, une partie de son entrée manque, et c'est écrit |
| **absent → corrigé** | la règle n'existait pas dans ma tranche ; elle y est maintenant |
| **divergent → corrigé** | la règle existait et disait autre chose ; elle est alignée sur le Java |

Fichiers Java lus : `AbstractBuilding`, `AbstractSchematicProvider`,
`DefaultBuildingInstance`, `workerbuildings/{Stash,PostBox,BuildingTownHall,
BuildingBarracks,BuildingBarracksTower,BuildingGateHouse,BuildingGuardTower,
BuildingWareHouse,BuildingBaker,BuildingCrusher,BuildingSifter,BuildingFarmer,
BuildingPlantation}`, `modules/{LivingBuildingModule,TavernLivingBuildingModule,
TavernBuildingModule,HomeBuildingModule,CourierAssignmentModule,
MinimumStockModule,RestaurantMenuModule,AbstractCraftingBuildingModule,
SimpleCraftingModule,FurnaceUserModule,AbstractAssignedCitizenModule,
GuardBuildingModule,StableCavalryBuildingModule,MinerBuildingModule,
QuarryModule,BuildingExtensionsModule,WorkerBuildingModule,ExpeditionLogModule}`,
`moduleviews/FloristFlowerListModuleView`.

## 1. Les quarante méthodes auditées

| # | Méthode Java | Ligne | Rust | Verdict |
| --- | --- | --- | --- | --- |
| 1 | `AbstractBuilding.getMaxBuildingLevel` | `AbstractBuilding.java:1154-1157` | `catalog.rs:190` (`spec_of`), `rules.rs:207` | fidel |
| 2 | `AbstractBuilding.getClaimRadius` (défaut) | `AbstractBuilding.java:654-667` | `rules.rs:98` | fidel |
| 3 | `AbstractBuilding.hasModule` | `AbstractBuilding.java:192-195` | `catalog.rs:204` | fidel |
| 4 | `AbstractBuilding.registerModule` | `AbstractBuilding.java:210-219` | `rules.rs:607` (`attach`) | fidel — le doublon reste à `crate::building` |
| 5 | `AbstractBuilding.getAllAssignedCitizen` | `AbstractBuilding.java:637-646` | `rules.rs:573` (`staff_pool`) | **absent → corrigé** (donnée) |
| 6 | `AbstractBuilding.requestUpgrade` | `AbstractBuilding.java:784-814` | `rules.rs:217`, `rules.rs:241` | **absent → corrigé** (la porte de recherche de la hutte n'existait pas) |
| 7 | `AbstractSchematicProvider.setBuildingLevel` | `:480-490` | `rules.rs:217` | **divergent → corrigé** (plafond du **type**, pas 5) |
| 8 | `DefaultBuildingInstance.getMaxBuildingLevel` | `:35-40` | `catalog.rs` (lignes `Residence`/`Tavern`/`Quarry`) | fidel |
| 9 | `Stash.getMaxBuildingLevel` | `Stash.java:54-57` | `catalog.rs` (Stash 0) | fidel |
| 10 | `PostBox.getMaxBuildingLevel` | `PostBox.java:63-67` | `catalog.rs` (Postbox 1) | fidel |
| 11 | `LivingBuildingModule.getModuleMax` | `:103-106` | `capacity.rs:222` | fidel |
| 12 | `TavernLivingBuildingModule.getModuleMax` | `:15-18` | `capacity.rs:226` | fidel |
| 13 | `TavernBuildingModule.getMaxInhabitants` | `:97-106` | `capacity.rs:226` | fidel |
| 14 | `TavernBuildingModule` (capi visiteurs) | `:162` | `capacity.rs:228` | fidel |
| 15 | `CourierAssignmentModule.getModuleMax` | `:98-101` | `capacity.rs:267` (via `employment`) | fidel |
| 16 | `HomeBuildingModule.getMaxInhabitants` | `:19-22` | `capacity.rs:224` | partiel — Java est un pli `max(prev, niveau)` |
| 17 | `MinimumStockModule.minimumStockSize` | `:56-61` | `capacity.rs:214` | fidel |
| 18 | `MinimumStockModule.addMinimumStock` | `:64-69` | `rules.rs:293` | **absent → corrigé** (l'entrée existante passe) |
| 19 | `RestaurantMenuModule.addMenuItem` | `:90-101` | `rules.rs:306`, `capacity.rs:218` | **absent → corrigé** (aucune exception) |
| 20 | `AbstractCraftingBuildingModule.getMaxRecipes` | `:171-179` | `capacity.rs:245` | **divergent → corrigé** (×5 manquait) |
| 21 | `AbstractCraftingBuildingModule.EXTRA_RECIPE_MULTIPLIER` | `:87` | `Capacity::ManyRecipes` (`capacity.rs:102`) | **absent → corrigé** |
| 22 | `AbstractCraftingBuildingModule.canLearnManyRecipes` (défaut) | `:1001-1004` | `key.rs:336` | **absent → corrigé** |
| 23 | `SimpleCraftingModule.canLearnManyRecipes` | `:46-49` | `key.rs:336` | **absent → corrigé** |
| 24 | `AbstractCraftingBuildingModule.hasSpaceForMoreRecipes` | `:147-151` | `rules.rs:317` | partiel — Java ne compte que les recettes **actives** (`:154-167`) |
| 25 | `FurnaceUserModule.alterItemsToBeKept` | `:93-96` | `capacity.rs:234` | fidel |
| 26 | `AbstractAssignedCitizenModule.isFull` | `:106-109` | `rules.rs:421` | fidel |
| 27 | `AbstractAssignedCitizenModule.assignCitizen` | `:55-66` | `rules.rs:437` | partiel — 1 condition sur 3 (`isFull`) |
| 28 | `GuardBuildingModule.isFull` | `:77-81` | `rules.rs:573` | **divergent → corrigé** (pool du bâtiment) |
| 29 | `StableCavalryBuildingModule.isFull` | `:34-41` | `rules.rs:573` | **absent → corrigé** (retour au pool du module) |
| 30 | `MinerBuildingModule.isFull` | `:28-32` | `rules.rs:573` | **absent → corrigé** (la mine partage aussi) |
| 31 | `WorkerBuildingModule.getModuleMax` | `:229-232` | `capacity.rs:267` | fidel — `sizeLimit` vient d'`employment` |
| 32 | `WorkerBuildingModule.canWorkDuringTheRain` | `:263-266` | `rules.rs:271` | **absent → corrigé** |
| 33 | `WorkerBuildingModule.onColonyTick` (recherche) | `:164-175` | `rules.rs:288` | fidel |
| 34 | `ExpeditionLogModule.serializeToView` (`unlocked`) | `:46-56` | `mod.rs:169`, `rules.rs:288` | fidel (vue) |
| 35 | `BuildingFarmer.getMaxExtensionCount` | `:281-284` | `capacity.rs:230` | fidel |
| 36 | `BuildingPlantation.getMaxExtensionCount` | `:246-252` | `capacity.rs:232` | fidel |
| 37 | `BuildingExtensionsModule.canAssignExtension` | `:234-238` | `rules.rs:421` | partiel — pas de liste d'extensions possédées |
| 38 | `BuildingWareHouse.canSort` | `:194-197` | `rules.rs:449` | fidel |
| 39 | `BuildingBaker.getSupportedCraftingTypes` | `:161-165` | `rules.rs:471` | fidel — `building == null` n'a pas d'équivalent (un `Hut` porte toujours un bâtiment) |
| 40 | `BuildingCrusher.getMaxDailyQuantity` | `:88-96` | `rules.rs:489` | fidel |
| 41 | `BuildingSifter.getMaxDailyQuantity` | `:80-88` | `rules.rs:489` | fidel |
| 42 | `BuildingGuardTower.getClaimRadius` | `:52-72` | `rules.rs:46` | fidel |
| 43 | `BuildingTownHall.getClaimRadius` | `:182-202` | `rules.rs:59` | **absent → corrigé** |
| 44 | `BuildingGateHouse.getClaimRadius` | `:75-84` | `rules.rs:74` | **absent → corrigé** (0 aux niveaux 4-5) |
| 45 | `BuildingBarracksTower.getClaimRadius` | `:95-99` | `rules.rs:114` | **absent → corrigé** (toujours 0) |
| 46 | `BuildingBarracks.getClaimRadius` | `:162-185` | `rules.rs:88` | **absent → corrigé** (prend les niveaux des tours) |
| 47 | `FloristFlowerListModuleView.addItem` | `:48-68` | `rules.rs:521` | fidel (vue) |
| 48 | `QuarryModule.getModuleMax` | `:135-139` | `capacity.rs:267` (`One`) | fidel |

(48 lignes pour 40 méthodes distinctes : les portées comptent pour quatre entrées
d'une même famille.)

## 2. Les huit divergences, et ce qui les prouve

### D1 — les recettes ne valent pas `2^niveau` mais `5 × 2^niveau`

`getMaxRecipes` multiplie par `EXTRA_RECIPE_MULTIPLIER = 5`
(`AbstractCraftingBuildingModule.java:87`) dès que `canLearnManyRecipes()` est
vrai (`:174-178`), et le défaut de la classe est **vrai** (`:1001-1004`). Seuls
les deux modules bâtis sur `SimpleCraftingModule` répondent faux
(`SimpleCraftingModule.java:46-49`) : l'atelier du constructeur et la table du
mineur (`BuildingModules.java:440-441`, `:495-496`).

Mon `Capacity::Recipes` appliquait `2^niveau` partout. Corrigé : variante
`Capacity::ManyRecipes` (×5, `capacity.rs:102`), `capacity()` choisit d'après
`key.rs:336` (`learns_many_recipes`). Un atelier de maçon au niveau 3 passe de
8 à **40** recettes ; celui du constructeur reste à 8.

### D2 — le plafond de niveau est celui du type, pas 5

`setBuildingLevel` refuse au-delà de `getMaxBuildingLevel()`
(`AbstractSchematicProvider.java:480-485`) — donc au-delà de **3** pour une
taverne (`ModBuildingsInitializer.java:521`), **1** pour un postbox ou une
carrière, **0** pour un stash. `crate::building::Building::set_level` ne connaît
que le 5 global : rien ne pouvait dire non. Corrigé par `Hut::may_reach`
(`rules.rs:217`) et le refus `LevelAboveHutMax`.

### D3 — cinq types ont leur propre portée de chunk

`getClaimRadius` est surchargé **cinq** fois, pas une : tour de garde
(`BuildingGuardTower.java:52-72`), hôtel de ville (`BuildingTownHall.java:182-202`,
qui monte à **5** au niveau 5), porte de ville (`BuildingGateHouse.java:75-84`,
qui **retombe à 0** aux niveaux 4 et 5), tour de caserne (**toujours 0**,
`BuildingBarracksTower.java:95-99`) et caserne (`BuildingBarracks.java:162-185`,
3 si toutes les tours sont ≥ 4, sinon 2, et 3 quand la liste est vide).

Je n'en portais qu'une. Corrigé : les quatre tables manquantes
(`rules.rs:59`, `:74`, `:88`) et la répartition (`rules.rs:114`). La caserne garde
une fonction qui prend les niveaux des tours en argument — c'est la seule façon
honnête de le faire sans monde.

### D4 — un pool d'ouvriers partagé, et un qui ne l'est pas

`GuardBuildingModule.isFull` compare **tous** les citoyens du bâtiment à
`getModuleMax()` (`GuardBuildingModule.java:77-81`), et `MinerBuildingModule`
fait pareil (`MinerBuildingModule.java:29-32`) : les cinq modules de garde d'une
tour de caserne partagent un seul pool, comme les deux métiers de la mine. Mais
`StableCavalryBuildingModule.isFull` revient au compte local
(`StableCavalryBuildingModule.java:34-38`), donc une écurie tient un palefrenier
**plus** ses cavaliers.

Corrigé : `StaffPool` + `staff_pool` (`rules.rs:573`), 13 clés partagées
(11 de garde + `miner_work` + `quarrier_work`), et l'écurie explicitement à part.
`hire_room` garde son arithmétique et documente que `held` doit être le total du
bâtiment quand le pool est partagé.

### D5 — une entrée de stock existante passe même quand la liste est pleine

`addMinimumStock` teste `minimumStock.containsKey(item) || minimumStock.size() <
minimumStockSize()` (`MinimumStockModule.java:64-69`). Mon `Full` refusait tout.
Corrigé : `Hut::may_add_stock(held, already_listed)` (`rules.rs:293`).

### D6 — le menu, lui, n'a pas cette exception

`addMenuItem` ne teste que `menu.size() >= building.getBuildingLevel() *
STOCK_PER_LEVEL` (`RestaurantMenuModule.java:96-101`) : un plat déjà au menu est
refusé comme un nouveau. Corrigé : `Hut::may_add_menu_item` (`rules.rs:306`),
volontairement distinct de D5, avec un test qui oppose les deux.

### D7 — au niveau maximum du type, la pluie ne compte plus

`canWorkDuringTheRain` rend `true` dès que
`building.getBuildingLevel() >= building.getMaxBuildingLevel()`
(`WorkerBuildingModule.java:263-266`). La règle n'existait nulle part : le port
ne gardait que le drapeau déclaré par le module. Corrigé :
`Hut::works_in_the_rain` (`rules.rs:271`) — et c'est bien ici, parce qu'elle a
besoin du **plafond du type** (taverne 3, boulanger 5).

### D8 — l'agrandissement est gaté par l'effet de **la hutte**

`requestUpgrade` refuse deux fois sur l'effet de la hutte elle-même :
`getEffectStrength(hutResearch) < 1` (pas débloquée,
`AbstractBuilding.java:786-792`) puis `<= getBuildingLevel()` (pas le droit de
monter, `:793-800`). Mon catalogue portait `HutSpec::effect` sans aucun
consommateur — un champ qui ne servait à rien. Corrigé : `Hut::may_upgrade`
(`rules.rs:241`) et le refus `HutResearchTooLow`, avec les deux cas distincts.

## 3. Ce qui reste partiel ou absent, et pourquoi

Aucune de ces lignes n'est un refus de travailler : chacune dit ce qui manque à
l'entrée, pas à la règle.

| Manque | Pourquoi |
| --- | --- |
| `getActiveRecipes` (`AbstractCraftingBuildingModule.java:154-167`) | Java ne compte pas les recettes désactivées ni les automatiques ; le port prend le nombre d'occupants en argument (`held`), donc le filtrage revient à l'appelant |
| les 2 autres conditions d'`assignCitizen` (`:55-66`) | « déjà assigné » et « `null` » demandent la fiche du citoyen, qui n'est pas dans cette tranche |
| le pli `max(prev, niveau)` de `HomeBuildingModule` (`:19-22`) | c'est une statistique de colonie ; le port rend la valeur par hutte |
| `BuildingGateHouse.getBuildingLevelEquivalent` (`:97-105`) | nourrit l'équipement, la vision et les PV, pas les modules |
| `PostBox.getBuildingLevel` forcé à 1 (`:70-74`) | sans effet observable : le postbox est construit au niveau 1 et `seats` refuse une hutte qui ne tient pas debout |
| `BuildingGateHouse.setBuildingLevel`/`destroy` et le `ConnectionManager` (`:175-188`) | pas de gestionnaire de connexions dans le port |
| `building == null` dans `BuildingBaker.getSupportedCraftingTypes` (`:161-165`) | un module détaché n'existe pas ici : un `Hut` porte toujours un bâtiment |
| les deux refus « client » du filtre à fleurs (`FloristFlowerListModuleView.java:56-66`) | portés côté serveur, faute de second côté — c'est dit dans le code |
| conteneurs, listes d'objets, réglages, requêtes, patrouilles de garde | cf. `MANQUANT.md` §1 à §7, inchangé |

## 4. Ce que je n'ai pas pu vérifier

- **Aucune compilation, aucun test lancé** (consigne). Les corrections sont
  écrites contre la ligne Java, mais c'est l'orchestrateur qui dira si elles
  compilent ; les chiffres ci-dessus (40 à 160 recettes, portées 0/2/3/4/5, 13
  clés de pool) sont calculés à la main depuis les constantes Java.
- **Les valeurs d'effet de recherche** viennent des fichiers de données
  (`researches/effects/*.json`) tels que `src/research/effect.rs` les a déjà
  encodés ; je n'ai pas relu les 26 JSON de hutte un par un, seulement vérifié
  que chaque identifiant cité existe dans cette table.
- **`BuildingBarracks.getClaimRadius`** ne peut être vérifié qu'avec les niveaux
  des tours : la fonction les prend en argument, mais rien dans le port ne tient
  la liste des tours (cf. `MANQUANT.md` §6).

Une erreur de compilation a été trouvée pendant l'audit, et tranchée contre le
Java, pas contre le compilateur : `Refusal::HutResearchTooLow` porte la force de
l'effet, or `getEffectStrength` rend un **`double`**
(`api/research/IResearchEffectManager.java:17`), donc la valeur est bien
flottante en source et `f64` n'est pas `Eq`. Le type garde `strength: f64` et
perd `Eq` (`PartialEq` suffit à tout ce que le module en fait : `assert_eq!` sur
des `Result<(), Refusal>` et des littéraux recopiés, jamais calculés).

## 5. État après audit

| | Avant | Après |
| --- | --- | --- |
| tests dans `src/colony_buildings` | 72 | **79** |
| lignes de code hors tests | 1 634 | 1 917 |
| refus nommés | 12 | 14 |
| formules de contenance | 11 | 12 |
| divergences connues | 8 | 0 |

Les sept tests ajoutés : `a_hut_refuses_a_level_above_its_kinds_ceiling`,
`a_hut_needs_its_own_research_to_unlock_and_to_grow`,
`the_tower_and_the_mine_share_one_pool_and_the_stable_does_not`,
`an_existing_stock_entry_may_be_updated_but_a_menu_dish_may_not`,
`a_non_crafting_module_refuses_a_recipe_by_name`,
`a_hut_at_its_kinds_top_level_works_in_the_rain`,
`the_builder_and_the_miner_learn_five_times_fewer_recipes` — plus la réécriture
de `every_kind_that_overrides_its_claim_radius_answers_its_own_table` et de
`a_crafter_holds_recipes_and_the_two_simple_ones_hold_five_times_fewer`.

Une trouvaille de plus, faite en relisant la liste des refus : `NotACraftingModule`
était **déclaré sans producteur**. Il a maintenant le sien — `Hut::may_add_recipe`
(`rules.rs:317`), qui refuse une clé qui ne tient pas de liste de recettes avant
même de compter, comme le fait la classe en Java (`AbstractCraftingBuildingModule.java:147-151`).
