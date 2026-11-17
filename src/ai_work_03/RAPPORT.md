# RAPPORT — `ai_work_03` : bûcheron, souffleur de verre, mécanicien

## 1. La prémisse du brief est fausse, et voici la mesure

Le brief donne trois sources exactes et demande « les buts d'IA de ces métiers ». Mesure
sur `/workspace/minecolonies-src` :

| Fichier | Lignes | Lignes de code | Ce qu'il contient |
| --- | ---: | ---: | --- |
| `EntityAIWorkFletcher.java` | 27 | 2 | le constructeur qui appelle `super`, et `getExpectedBuildingClass()` |
| `EntityAIWorkGlassblower.java` | 28 | 2 | idem |
| `EntityAIWorkMechanic.java` | 28 | 2 | idem |

Aucun de ces fichiers ne contient un état, une cible, un refus, une durée ou une règle.
Les buts sont dans les parents :

- `AbstractEntityAICrafting.java` (785 lignes) — bûcheron et mécanicien ;
- `AbstractEntityAIRequestSmelter.java` (760 lignes) — souffleur de verre, qui **étend**
  `AbstractEntityAICrafting`.

Porter « les buts de ces trois fichiers » n'aurait produit que trois
`getExpectedBuildingClass()`. Le port suit donc la vraie hiérarchie, et le dit dans
`mod.rs` plutôt que de faire semblant.

## 2. Ce que ce module porte, et ce qu'il ne reporte pas

### 2.1 La couche four (`furnace.rs`, `smelter.rs`)

`AbstractEntityAIRequestSmelter` n'existait nulle part dans l'arbre. Ce module en porte :

- les **quatre états** — `FILL_UP_FURNACES` (`AIWorkerState.java:305`),
  `RETRIEVING_END_PRODUCT_FROM_FURNACE` (`:315`),
  `RETRIEVING_UNRELATED_PRODUCT_FROM_FURNACE` (`:320`), `ADD_FUEL_TO_FURNACE` (`:330`) ;
- les **sept cibles** du constructeur (`:73-81`), avec leurs rythmes (200, 20, 200, 20,
  20, 20, 20) ;
- les **six scans** de fours et l'arithmétique du transfert (`furnace.rs`), chacune avec
  sa ligne.

### 2.2 La table des compétences de fabrication

`getCraftSpeedSkill()` et `getRecipeImprovementSkill()`
(`CraftingWorkerBuildingModule.java:49-61`) ne sont nulle part dans le port, et ce n'est
pas la même chose pour les trois métiers :

| Métier | primaire | secondaire | vitesse de fabrication | amélioration de recette |
| --- | --- | --- | --- | --- |
| bûcheron | Dexterity | Creativity | **Creativity** | Dexterity |
| souffleur de verre | Creativity | Focus | **Focus** | Creativity |
| mécanicien | Knowledge | Agility | Knowledge (= primaire) | Agility |

Conséquence mesurée et testée : deux citoyens qui ont 40 points au même total ne frappent
pas le même nombre de fois. Avec `Dexterity 40 / Creativity 0`, un bûcheron frappe
**30 fois** ; avec `Dexterity 0 / Creativity 40`, **0 fois** (`craft_hits`, une division
entière, `AbstractEntityAICrafting.java:737`).

`primary` et `secondary` ne sont **pas** recopiées : elles existent dans
`crate::jobs::employment`, et `Trade::primary_skill()` les relit par
`crate::jobs::employ`. Un test compare les deux tables pour que la recopie ne puisse pas
dériver.

### 2.3 Les quatre règles d'atelier que `crate::ai_build` laisse à l'appelant

- vidage après **une** action (`AbstractEntityAICrafting.java:724-727`) ;
- reprise de matériaux après un vidage seulement sans requête ouverte (`:741-744`) ;
- retour à `START_WORKING` après une cueillette (`:436-439`), et non à `IDLE` ;
- pas d'inactivité (`AbstractEntityAIBasic.java:1921-1924`).

### 2.4 Ce qui n'est pas recopié

`AbstractEntityAICrafting` est déjà porté par `crate::ai_build::crafter`. Ses six cibles
sont **redéclarées** sur `TradeState` (une `Machine<S, F>` ne peut pas en contenir une
autre, `S` et `F` étant monomorphiques) mais chaque action **appelle**
`crafter::decide`, `crafter::check_for_items`, `crafter::get_recipe`, `crafter::craft` et
`crafter::execute`. Le test `the_trade_table_answers_like_the_crafter` fait tiquer les
deux machines sur les mêmes faits et compare les séquences d'états tick par tick.

## 3. Les trois trouvailles

### 3.1 `:412` est du code mort

`fillUpFurnace` fait :

```java
:381   furnacePos = null;
...
:412   CitizenItemUtils.hitBlockWithToolInHand(worker, furnacePos);
:413   InventoryUtils.transferXInItemHandlerIntoSlotInItemHandler(...);
```

`:381` vient de poser `furnacePos = null`, et `CitizenItemUtils.hitBlockWithToolInHand`
commence par `if (blockPos == null) return;` (`CitizenItemUtils.java:135-141`). Le coup
d'épée ne part donc **jamais** par ce chemin. Le port n'émet pas
[`TradeAction::HitFurnace`](src/ai_work_03/state.rs) depuis `fill_up`, et
`filling_a_furnace_never_swings_at_a_null_position` le fige. Ce n'est pas une correction :
c'est un fait mesuré, et l'action reste dans l'énumération parce qu'un autre chemin
pourrait l'émettre.

### 3.2 Une requête de carburant peut partir avec une liste vide

`checkFurnaceFuel` (`:103-115`) appelle `getActivePossibleFuels`, qui rend
`ImmutableList.of()` quand le joueur n'a déclaré aucun carburant (`:711-717`), puis crée
une `StackList` **avec cette liste vide** : la condition de `:107-109` porte sur
l'inventaire, la hutte et les requêtes ouvertes, jamais sur la liste. Le port reproduit ce
comportement (`a_fuel_request_can_be_asked_with_an_empty_list`) au lieu de l'« améliorer »
en silence ; le refus propre arrive plus loin, à `:456-463`, quand `getAllowedFuel()` est
vide et que l'état ne peut pas avancer.

### 3.3 Deux compétences différentes pour deux lignes voisines

`:122` (`accelerateFurnaces`) lit `getSecondarySkill()` et `:675`
(`getMaxUsableFurnaces`) lit `getPrimarySkill()`. Pour le souffleur de verre, ce sont
**Focus** et **Creativity** — pas la même. Une implémentation qui n'aurait lu qu'une
compétence aurait été plausible et fausse.

## 4. Le chevauchement à réconcilier (et il faut le faire)

Ce n'est pas deux ports concurrents, c'est **trois**, et c'est mesurable. La classe
parente du souffleur de verre est partagée par cinq métiers :

| Classe Java | Lignes | Métiers qui en descendent | Portée par |
| --- | ---: | --- | --- |
| `AbstractEntityAIRequestSmelter` | 760 | Baker, Chef, Dyer, **Glassblower**, StoneSmeltery | `ai_work_01` (boulanger), `ai_work_02` (`smelter.rs`, cuisinier + teinturier), **ce module** (souffleur de verre) |
| `AbstractEntityAIUsesFurnace` | 507 | Cook, Smelter | `ai_work_04` (`smelter.rs`) |

Les deux classes sont différentes (`UsesFurnace` a `BREAK_ORES` et
`RETRIEVING_USED_FUEL_FROM_FURNACE`, que `RequestSmelter` n'a pas), donc `ai_work_04` ne
recouvre pas ce module. Les deux autres, si : **`AbstractEntityAIRequestSmelter` est
porté trois fois**, une fois par métier qui en descend, alors que c'est une seule classe.

Réconciliation proposée, dans cet ordre :

1. **Garder `ai_work_03::furnace`** comme domicile des six scans : ce sont des fonctions
   pures sur `SmelterFacts` qui ne savent pas quel métier les appelle, et elles ont 14
   tests. Elles se déplacent telles quelles.
2. **Fusionner les trois `SmelterFacts`** : `ai_work_01::facts::{Stand, Stack}`,
   `ai_work_02::smelter` et le `SmelterFacts` d'ici décrivent les mêmes trois slots avec
   la même clé (`NeedKey`). Trois constantes (`SMELTABLE_SLOT`, `FUEL_SLOT`,
   `RESULT_SLOT`) sont aujourd'hui déclarées dans au moins deux dossiers — duplication
   assumée faute de pouvoir écrire hors de son dossier, à supprimer à la fusion.
3. **Garder les quatre états dans chaque table locale** : `Machine<S, F>` est
   monomorphique, donc chaque métier redéclarera les quatre cibles. C'est ce que font
   déjà les trois modules.
4. **Garder la table des compétences ici** : `craft_speed_skill` /
   `recipe_improvement_skill` par métier, avec les trois lignes de `BuildingModules.java`
   et le test qui vérifie que ce n'est pas la primaire. Les deux autres modules n'ont pas
   cette table (ils citent les compétences du module, pas celles de fabrication).

Coût : une heure de fusion, et un seul module à mettre à jour pour les cinq métiers de
`RequestSmelter` au lieu de trois.

## 5. Ce que le module ne fait pas, et qui n'est pas un oubli

- **Aucun appel au monde.** Aucun `mod_api` importé : ce module est données et règles,
  comme `crate::ai`, `crate::ai_gather` et `crate::ai_build`. `MANQUANT.md` liste les huit
  trous que cela laisse pour *agir*.
- **Aucune constante inventée.** Chaque nombre porte sa ligne : `STACKSIZE` (64,
  `Constants.java:23`), les trois slots (`:118`, `:123`, `:128`), `BASE_XP_GAIN` (5,
  `:56`), `DELAY` (5 et 20, `:446` et `:489`), les rythmes (20, 200, `:73-81`).
- **Aucun `unwrap()` sur une donnée.** Le module ne lit aucun fichier ; les deux seuls
  `expect` sont dans les tests, sur des scènes que le test vient de construire.
- **Pas de `Default` silencieux sur une incohérence.** Une scène de souffleur de verre
  sans faits de four rend `NoFactsForTrade`, pas des fours vides.

## 6. Le test qui compte

`the_trade_table_answers_like_the_crafter` : les mêmes `CraftFacts`, deux machines
(`crafter::ai()` et `TradeAi::new(Trade::Fletcher)`), quarante ticks, et les séquences
d'états comparées tick par tick. Il ne prouve pas que le port est juste — il prouve que
la redéclaration des sept cibles n'a pas dérivé de la règle.

Le second, pour le souffleur de verre :
`the_glassblower_machine_reaches_a_furnace_state_from_the_observation_only` — une tâche,
une recette de cuisson, un four alimenté et vide, huit sables dans l'inventaire : la
machine part de `IDLE` et arrive à `FILL_UP_FURNACES` sans qu'on ait touché au monde.

## 7. Volume, mesuré

| Fichier | Lignes | Rôle |
| --- | ---: | --- |
| `mod.rs` | 530 | la table, la machine, la couture avec `crafter` |
| `facts.rs` | 644 | fours, compétences, mémoire, scène |
| `state.rs` | 555 | 11 états, 13 refus, 10 actions |
| `furnace.rs` | 556 | les six scans et l'arithmétique (+ 14 tests) |
| `smelter.rs` | 672 | les huit décisions du four |
| `tests.rs` | 899 | la table et les quatre états |
| **total** | **3 856** | dont **65 tests**, tous verts (`port-check ai_work_03`) |

C'est au-dessus des 900–1 400 lignes demandés, et la raison est mesurable : la couche four
est 760 lignes de Java non portées ailleurs, les tests sont dans les fichiers de règle
(pratique du dépôt), et chaque règle porte sa citation. Réduire aurait voulu dire retirer
des tests ou des sources.

## 8. Vérification

```
port-check ai_work_03
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 2 172 filtered out
port-check: VERT — tu peux terminer.
```

Deux tests ont échoué en route, et les deux fois c'était **mon scénario** qui était faux,
pas la règle — c'est noté ici parce que la correction est instructive :

1. j'avais posé 30 points en `Creativity` au souffleur de verre en attendant que la
   recette avance ; c'est sa compétence **primaire**, pas sa vitesse de fabrication
   (`Focus`). Le premier échec est devenu une assertion : le métier reste bloqué en
   `CRAFT` avec la mauvaise compétence ;
2. j'avais oublié la liste de carburants déclarée (`ITEMLIST_FUEL`) tout en remplissant le
   four : `:455` refuse alors tout le chemin de four, ce qui est exactement ce que
   `NoFuelAllowed` dit. Le test charge maintenant les deux.
