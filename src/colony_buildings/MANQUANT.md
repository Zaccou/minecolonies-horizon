# MANQUANT — `src/colony_buildings`

Ce que le Java de `core/colony/buildings/` porte et que cette couche ne peut pas
exprimer, avec le chemin exact, ce qui manque et ce que ça bloque. Rien d'ici
n'est du code fictif : ce sont des trous assumés, chacun avec son coût.

La règle de lecture est celle du dépôt : **une ligne par capacité manquante**, et
si une capacité est *partiellement* exprimée, la ligne dit quelle moitié est là.

## 1. Le conteneur d'une hutte

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `core/colony/buildings/AbstractBuildingContainer.java` (229 l.) | `containerList`, `keepX` (ce que l'ouvrier garde), `priority`, `hasContainerPosition` | savoir où sont les coffres d'une hutte, donc le dépôt et le retrait d'objets. `rules::Hut` compte des **modules**, jamais des blocs |
| `:59-66` | la priorité de ramassage dans la liste de pick-up | l'ordre dans lequel un livreur vide une hutte |
| `core/colony/buildings/AbstractBuilding.java:1120-1146` | `getRequiredItemsAndAmount` (les objets gardés par niveau) | le calcul de ce qu'un ouvrier emporte ; `capacity` ne donne que le **nombre** de stacks (module four) |

## 2. Les listes et les recettes

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `modules/AbstractCraftingBuildingModule.java` (1 155 l.) | la liste de recettes elle-même : `recipes`, `disabledRecipes`, `customRecipes`, `setRecipe`, `switchRecipe`, `isRecipeCompatible` | **la seule chose qui manque vraiment au modèle** : `Capacity::Recipes` donne le nombre de recettes qu'une hutte accepte (2^niveau), pas *quelles* recettes. Il faut `crate::crafting` pour les tokens |
| `:147-151` | `canLearnRecipe` (refus silencieux au-delà du maximum) | la forme du refus ; `Refusal::Full` porte le nombre, pas la recette |
| `:187-260` | `isRecipeCompatibleWithCraftingModule`, les tags par métier (`CRAFTING_BAKER`, …) | dire si une recette *donnée* est acceptée ; `rules::Hut::may_craft` ne dit que « aucune avant le niveau 3 » pour le boulanger |
| `modules/ItemListModule.java` (162 l.) | le contenu des listes : `itemlist_fuel` (charbon + charbon de bois par défaut), `itemlist_ore`, `itemlist_sapling`, `itemlist_flower`, `itemlist_compostable` | filtrer un objet ; la couche dit qu'une liste existe et qu'elle n'a pas de plafond de niveau, pas ce qu'elle contient |
| `modules/RestaurantMenuModule.java` (266 l.) | les plats du menu, `FoodUtils.EDIBLE`, la valeur nutritive | ce que la taverne cuisine ; `Capacity::MenuPerLevel` donne la taille du menu (5/niveau) |
| `modules/MinimumStockModule.java:70-140` | la levée de requêtes qui maintient le stock minimal | le stock minimal est un **nombre** ici, pas un flux de requêtes : il faut `crate::requests` |

## 3. Les modules qui regardent le monde

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `modules/BuildingExtensionsModule.java` (311 l.) | `ownedExtensions`, l'assignation d'une parcelle | les champs du fermier (`capacity` = 1/niveau) et de la plantation (`ceil(niveau/2)`) sont comptés, pas **placés** |
| `modules/MinerLevelManagementModule.java` (292 l.) | `levels`, `MinerLevel`, l'init de structure d'un palier de mine | la profondeur d'une mine ; `miner_levels` est déclaré sans capacité |
| `modules/FurnaceUserModule.java` (147 l.) | la liste des fours trouvés dans le schéma | où sont les fours ; seule la quantité de carburant gardée (`64 × niveau`) est modélisée |
| `modules/BedHandlingModule.java` (111 l.) | la liste des lits | les lits d'une auberge ou d'une caserne ; `bed` est déclaré sans capacité |
| `modules/GraveyardManagementModule.java` (370 l.) | les tombes, leur dégradation, `Grave` | le cimetière ; `graveyard` est déclaré sans capacité |
| `modules/EnchanterStationsModule.java` (154 l.) | les bâtiments qu'un enchanteur peut relier | `enchanter_stations` est déclaré sans capacité |
| `modules/TavernBuildingModule.java` (442 l.) | l'apparition des visiteurs, le coût de recrutement (`RecruitmentItemsListener.getRandomRecruitCost`), la musique (`MAX_STORY`) | `Capacity::ThreePerLevel` donne le nombre de visiteurs (ligne 162), rien de leur venue |
| `modules/AbstractAssignedCitizenModule.java:154-164` | `serializeToView` (l'envoi au client) | rien ici : le port n'a qu'un côté |

## 4. Les réglages

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `modules/settings/*.java` (14 fichiers) | `SettingsModule`, les 8 types de réglage (`BoolSetting`, `IntSetting`, `StringSetting`, `BlockSetting`, `RecipeSetting`, `GuardTaskSetting`, `StringSettingWithDesc`, `BeakerCollectionSetting`…), leurs valeurs par défaut et leur sérialisation | `ModuleKey::is_settings` dit **quels** modules sont des pages de réglages (21 clés) ; il ne dit pas **quel** réglage chacune porte. Il faut les 25 `ISettingKey` déclarées à côté des bâtiments : `building` (`AbstractBuilding.java:108`), `useshears` (`:110`), `recipemode` (`modules/AbstractCraftingBuildingModule.java:77`), `breeding`, `milking_amount`, `stewing_amount`, `milking_days`, `milk_item` (`BuildingCowboy.java:64-79`), `fertilize` (`BuildingFarmer.java:69-70`), `fillblock`, `maxdepth` (`BuildingMiner.java:49-54`), `crushermode`, `dailylimit` (`BuildingCrusher.java:27-28`), `mode`, `buildmode` (`BuildingBuilder.java:42-43`), `replant`, `restrict`, `defoliate`, `dynamictreeharvestsize` (`BuildingLumberjack.java:56-59`), `guard_settings` (`AbstractBuildingGuards.java:68-76`), `kidspawn`, `enterleave`, `autohiring`, `autohousing`, `tape` (`BuildingTownHall.java:64-83`) |
| `modules/GuardPatrolModeSetting.java`, `GuardFollowModeSetting.java` | les valeurs énumérées d'un mode de patrouille et de suivi | régler une garde ; `ModuleKey::GuardSettings` est déclaré, pas configurable |

## 5. La recherche, à moitié

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `core/colony/managers/ResearchManager.java:135-140` | `getResearchEffectIdFrom(block)` appliqué à un **bloc** : le port n'a pas de bloc, donc `catalog::HutSpec::effect` écrit l'identifiant en dur (`blockhutlibrary`) | rien tant que la table est juste : 26 huttes sur 54 sont gatées, et un test vérifie que chaque identifiant existe dans `crate::research::effect` |
| `IResearchEffectManager.getEffectStrength` | l'état de recherche d'une colonie | `Capacity::with_research` et `Hut::research_gate` **prennent** la force en argument ; le port a l'arbre (`crate::research`) mais pas de colonie qui le porte |
| `AbstractBuilding.java:786-800` | `requestUpgrade` refusé faute de recherche | le refus d'**agrandissement** ; `crate::colony::WorkOrder` porte la commande, `catalog::HutSpec::max_level` porte le plafond, la jonction des deux n'existe pas |

## 6. Les gardes, en dehors du module

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `core/colony/buildings/AbstractBuildingGuards.java` (800+ l.) | cibles de patrouille, `guardBuildingNear`, `requiresManualTarget`, les multiplicateurs de rayon | le comportement d'une garde ; `rules::Hut` ne donne que son rayon de chunk |
| `workerbuildings/BuildingBarracks.java:162-185` | le rayon de la caserne dépend du **niveau de chacune de ses tours** (3 si toutes ≥ 4, sinon 2, et 3 quand la liste est vide) | **résolu** : `rules::barracks_claim_radius(&[niveaux])` prend les niveaux en argument et rend 3 ou 2. `claim_radius(kind, level)` rend 2, le plancher, parce qu'un seul bâtiment ne connaît pas ses tours |
| `workerbuildings/BuildingBarracksTower.java:95-99` | la tour de caserne rend **toujours 0** | modélisé : `claim_radius(BarracksTower, _) == 0` |
| `workerbuildings/BuildingTownHall.java:182-202`, `workerbuildings/BuildingGateHouse.java:75-84` | deux tables de portée supplémentaires | modélisées : `town_hall_claim_radius`, `gate_house_claim_radius` (la porte **perd** sa portée aux niveaux 4 et 5, c'est le Java) |
| `core/colony/buildings/AbstractBuilding.java:637-646` | `getAllAssignedCitizen` agrège les citoyens de **tous** les modules d'assignation | modélisé comme donnée : `rules::StaffPool` dit quels modules comptent le total du bâtiment (11 de garde + les 2 de la mine) au lieu de leur propre liste |
| `workerbuildings/BuildingGateHouse.java:97-105` | `getBuildingLevelEquivalent` (1→1, 2→3, 3→5, sinon 0) | absent : il nourrit `getMaxEquipmentLevel`, `getBonusVision` et `getBonusHealth`, qui sont des règles d'équipement et de combat, pas de module |
| `workerbuildings/BuildingGateHouse.java:175-188` | `setBuildingLevel` et `destroy` préviennent le `ConnectionManager` | absent : il n'y a pas de gestionnaire de connexions dans le port |
| `workerbuildings/PostBox.java:70-74` | `getBuildingLevel()` rend **toujours 1**, quel que soit le niveau stocké | absent, et sans effet observable : un postbox est construit au niveau 1, et `seats` refuse de toute façon une hutte qui ne tient pas debout |
| `workerbuildings/BuildingBarracksTower.java:104-150` | le lien tour → caserne (`setBarracks`, `TAG_POS`) et le refus de dépasser la caserne | `crate::building::Requirements` porte déjà un `ParentRule::Inside(Barracks)` ; le lien *réciproque* (la caserne connaît ses tours) n'est nulle part |

## 7. Les vues

| Chemin Java | Ce qui manque | Ce que ça bloque |
| --- | --- | --- |
| `core/colony/buildings/moduleviews/` (40 fichiers) + `views/AbstractBuildingView.java` | tout le côté client : `serializeToView`, `RequestTaskModuleView`, `MinerAssignmentModuleView`, `CombinedHiringLimitModuleView`, `StudentBuildingModuleView`… | `ModuleKey::is_view_only` nomme les 8 producteurs **sans moitié serveur** ; il ne dit pas ce que leur fenêtre affiche |
| `moduleviews/FloristFlowerListModuleView.java:52-66` | deux des trois refus du filtre à fleurs sont **côté client** en Java (`TOO_MANY_FILTERED_BELOW_LVL4_FLORIST`, `TOO_MANY_FILTERED_FLORIST`) | le port les remonte dans `rules::Hut::may_filter_flower` parce qu'il n'a pas de second côté. C'est une règle client déplacée serveur, et c'est écrit ici pour que ça ne se perde pas |

## 8. Ce que la couche ne redit pas, exprès

| Sujet | Où c'est déjà | Pourquoi pas ici |
| --- | --- | --- |
| les 58 modules d'ouvrier (métier, deux compétences, pluie, nombre de têtes) | `crate::jobs::employment` | une table, un endroit à corriger ; `capacity::capacity_on` la **lit**, et un test tient les deux d'accord |
| le niveau, l'étape, le parent, la pose, le module en double | `crate::building` (`instance`, `requirements`) | `Refusal::Building` réexpédie le refus tel quel, pour qu'une règle garde un seul nom |
| le refus d'agrandissement, l'ordre de travail | `crate::colony` | `rules` ne dit que « le plafond du type est X » |
| les outils et leur niveau par palier de hutte | `crate::jobs::tool` | `getMaxEquipmentLevel` n'est pas retraduit ici |
| les 106 effets de recherche et leurs échelles | `crate::research::effect` | les portes de cette couche les **nomment**, un test vérifie qu'elles existent |

## 9. Une prémisse du brief qui était fausse

Le brief demandait « module → **niveau de déblocage** ». Java n'a pas de niveau
de déblocage de module : `BuildingEntry.produceBuilding` enregistre **tous** les
modules de l'entrée, sans test de niveau
(`api/colony/buildings/registry/BuildingEntry.java:148-160`). Ce que le niveau
change est le **contenu** (une capacité de 0, 1, 2^niveau…), plus trois
capacités à seuil (tri de l'entrepôt au niveau 3, types d'artisanat du boulanger
au niveau 3, filtre à fleurs du fleuriste aux niveaux 4 et 5) et deux portes de
recherche (marksman, huscarl). `colony_buildings::Gate` nomme ces quatre
conditions, et un test vérifie qu'aucune ne décide de l'**existence** d'un
module.
