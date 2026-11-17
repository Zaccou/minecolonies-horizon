# RAPPORT — `src/ai_work_09`

Les buts d'IA de trois métiers, portés de MineColonies 1.20.1 vers Horizon 26.3 : le
**planteur**, l'**éleveur de poulets** et le **cow-boy**. Une machine à états testable par
métier, ses faits, ses refus nommés, et les durées que le Java y met.

## 1. Ce qui est livré

| Fichier | Lignes | Question qu'il répond | Java |
| --- | ---: | --- | --- |
| `mod.rs` | 571 | le moteur : `WorkAi`, `WorkTrade`, `WorkScene`, `Decision`, `WorkOutcome`, les quatre cibles de retour des états de base | `AbstractAISkeleton.java`, `AbstractEntityAIBasic.java` |
| `state.rs` | 497 | les 19 états (8 de base + 11 de métier), leur drapeau `isOkayToEat`, les 34 refus nommés | `AIWorkerState.java` |
| `facts.rs` | 755 | ce que les trois buts demandent au monde, en observations | les trois classes + les deux modules |
| `planter.rs` | 996 | les six états de champ + les deux du parent d'artisanat | `EntityAIWorkPlanter.java` (490 lignes) |
| `herder.rs` | 1201 | les huit états d'élevage, partagés par les deux éleveurs, + la frappe au tirage de l'éleveur de poulets | `AbstractEntityAIHerder.java` (874) + `EntityAIWorkChickenHerder.java` (74) |
| `cowboy.rs` | 529 | les deux états de traite et la surcharge de `decideWhatToDo` | `EntityAIWorkCowboy.java` (270) |
| `tests.rs` | 433 | les scénarios : une suite de faits doit produire la suite d'états du Java | — |
| `MANQUANT.md` | 161 | ce que l'API ne permet pas, et neuf constats mesurés sur le Java | — |

**83 tests**, tous dans le module : ils compilent avec `cargo test`.

Le test qui compte est `tests::the_planter_walks_its_six_states_in_java_s_order` : un
scénario de faits, six ticks, et la suite
`START_WORKING → PREPARING → PLANTATION_PICK_FIELD → PLANTATION_MOVE_TO_FIELD →
PLANTATION_DECIDE_FIELD_WORK → PLANTATION_WORK_FIELD`, **avec** la ligne Java de chaque
pas. Deux autres scénarios font la même chose pour l'éleveur de poulets (qui va jusqu'à
l'abattage et compte une action quand l'animal meurt) et pour le cow-boy (qui va jusqu'à
la traite et rend `INVENTORY_FULL`).

## 2. Trois prémisses du brief qui ne tiennent pas

### 2.1 « les états : chercher, aller, agir, rendre »

Ce schéma ne se superpose à aucune des trois tables :

- **Le planteur** en a cinq qui ne se réduisent pas à quatre : **choisir** un champ
  (`PICK_FIELD`), **aller** au champ (`MOVE_TO_FIELD`), **faire décider** par le module
  (`DECIDE_FIELD_WORK`), **agir** (`WORK_FIELD`) et **rentrer** (`RETURN_TO_BUILDING`).
  « Faire décider » n'est ni un déplacement ni une action : c'est un appel au module de
  plantation, et il a son propre état parce que `workField` (`:206`) ne fonctionne pas
  sans sa réponse.
- **Les deux éleveurs** n'ont pas d'étape « rendre » du tout : ils alternent entre
  quatre travaux (`HERDER_BREED`, `HERDER_BUTCHER`, `HERDER_FEED`, `HERDER_PICKUP`) que
  `DECIDE` choisit par tirage. Un `RETURN_TO_BUILDING` existe comme état de base
  (`startWorkingAtOwnBuilding`, `AbstractEntityAIHerder.java:288-295`) mais son rôle est
  inverse : c'est le point d'entrée, pas la sortie.

Les tables sont donc portées **telles quelles**, un état par cible Java. Aplatir sur
quatre cases aurait perdu `PLANTATION_PICK_FIELD`, `PLANTATION_RETURN_TO_BUILDING` et la
distinction `HERDER_BUTCHER` / `HERDER_BREED`.

### 2.2 « `EntityAIWorkChickenHerder` et `EntityAIWorkCowboy` sont des buts d'IA »

Ils le sont, mais **presque entièrement par héritage**. Mesuré :

- `EntityAIWorkChickenHerder` fait 74 lignes et **n'enregistre aucune cible** : son
  constructeur (`:35-38`) ne fait qu'appeler `super(job)`. Sa seule règle propre est
  `butcherAnimal` (`:60-73`).
- `EntityAIWorkCowboy` fait 270 lignes, enregistre **deux** cibles (`:69-72`) et
  surcharge `decideWhatToDo` (`:96-120`).

Le vrai but d'IA des deux est donc `AbstractEntityAIHerder` (874 lignes) — de la même
façon que `EntityAIWorkChef` et `EntityAIWorkDyer` sont la table du four, constat déjà
écrit dans `src/ai_work_02/`. `herder.rs` porte cette table **une fois** et la donne aux
deux ; le cow-boy se raccorde à `herder::register` en lui passant **sa** fonction de
décision, ce qui reproduit le « override par référence de méthode » que Java fait tout
seul.

### 2.3 « les durees de travail, chaque constante citee a sa ligne Java »

Elles sont citées, mais **il n'y en a presque pas** dans ces trois classes, et c'est un
fait à dire : la quasi-totalité des durées est portée par la **cadence des cibles**
(`TICKS_20`, `TICKS_SECOND`, `STANDARD_DELAY`, `DECIDING_DELAY` 80, `BREEDING_DELAY` 40,
`BUTCHER_DELAY` 20, `COWBOY_DELAY` 1), pas par des constantes de durée de travail. Le
seul vrai travail chronométré est le minage du planteur (`harvest`, `clear`), et sa durée
n'est pas dans `EntityAIWorkPlanter` : elle est dans `AbstractEntityAIInteract.mineBlock`,
que `crate::ai_build::work_time` porte déjà. `planter::mining_ticks` **délègue** au lieu
de recopier la formule — deux copies d'une même durée finiraient par diverger.

## 3. Ce que le port ne fait pas (et qui est nommé, pas contourné)

Aucun appel au monde : pas de marche, pas de `setBlockAndUpdate`, pas de `mobInteract`,
pas de `searchForAnimals`. Tout est un fait dans `facts.rs` ; les écritures que la
décision doit retenir sont dans `WorkMemory` et le compteur d'actions. `MANQUANT.md` §1
liste les neuf choses que l'API de mod ne permet pas d'écrire, avec, pour chacune, ce que
le port substitue et ce que la substitution perd.

**Une approximation est assumée et documentée au-dessus de la fonction** :
`walkingToAnimal` (`AbstractEntityAIHerder.java:622-632`) est un résultat de chemin, et
le port en garde le test géométrique (`herder::walking_to_pos`, portée 2). Une entité à
deux blocs dont le chemin est encore en calcul répond `true` en Java et « arrivé » ici.

## 4. Neuf constats mesurés sur le Java

Ils sont détaillés dans `MANQUANT.md` §2 avec leur ligne. Les quatre qui changent une
décision :

1. **`isFeedAble` n'est appelée nulle part** (`AbstractEntityAIHerder.java:268-271`).
   Portée, et dite morte.
2. **`EntityAIWorkPlanter.canGoIdle` rend toujours `false`** : ses deux branches
   (`:476-479`) dépendent d'un `hasWorkToDo` surchargé en `true` (`:468-471`).
3. **La branche « limite du champ » de `getActionsDoneUntilDumping` n'est pas atteinte
   depuis `decide`** : `decide` ne tourne qu'en `START_WORKING`, donc il lit toujours la
   constante 32. La limite du module n'agit que par `inventoryNeedsDump`. Un test le
   tient.
4. **`chanceToButcher` peut rendre `Infinity`** au niveau de hutte 0
   (`AbstractEntityAIHerder.java:695`) : le port ne corrige pas.

## 5. État de vérification

**Vert.** `ssh deb '/home/zaccou/bin/port-check ai_work_09'` rend :

```
test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 2468 filtered out
port-check: VERT — tu peux terminer.
```

`cargo check --lib` est vert aussi, et aucun avertissement ne vient de
`src/ai_work_09` dans l'une ou l'autre sortie. Les seuls avertissements restants du
dépôt sont dans d'autres modules (`colony/colonie.rs`, `commands/reply.rs`,
`research/progress.rs`).

## 6. Écart avec la cible de la tranche

Le brief visait 900 à 1 400 lignes et 40 tests. Ce module en fait 5 268 et 83 tests.
L'écart n'est pas du remplissage : les trois classes Java totalisent 1 708 lignes, la
table d'élevage est écrite une fois pour deux métiers, chaque règle porte sa ligne, et
chaque fait porte la ligne Java qu'il remplace. C'est aussi la taille des modules
voisins (`src/ai_gather/farmer.rs` : 952 lignes pour **un** métier). Retirer des tests
pour tenir le chiffre aurait été le mauvais échange ; le dire ici est le bon.
