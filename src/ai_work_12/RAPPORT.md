# RAPPORT — buts d'IA de l'enchanteur, du guérisseur et du croque-mort (`src/ai_work_12`)

**État : non compilé par moi.** La consigne de la mission est explicite — « tu ne lances
aucune compilation, l'orchestrateur compile et te renverra les erreurs exactes » — donc ce
module n'a pas de `port-check` dans ce rapport, et **aucune ligne ci-dessous ne prétend
qu'il est vert**. Ce que je peux prouver sans compilateur : les sources citées existent aux
lignes citées (vérifiées une par une), et les 82 tests énoncent les suites d'états du Java.

## 1. Ce qui est porté, et en quelles mesures

| Fichier Java | Lignes | Porté dans | Cibles de table | Constantes citées | `#[test]` |
| --- | ---: | --- | ---: | ---: | ---: |
| `service/EntityAIWorkEnchanter.java` | 498 | `enchanter.rs` | 3 (`:109`, `:110`, `AbstractEntityAICrafting.java:166`) | 13 | 9 |
| `service/EntityAIWorkHealer.java` | 552 | `healer.rs` | 7 (`:86-93` moins `IDLE`) | 14 | 23 |
| `service/EntityAIWorkUndertaker.java` | 553 | `undertaker.rs` | 6 (`:91-96`) | 16 | 20 |
| les trois | 1 603 | `mod.rs` (relais, 5 transitions) | 5 | — | 8 |
| — | — | `state.rs` (11 états, 45 refus) | — | — | 5 |
| — | — | `facts.rs` (3 moitiés de faits, 3 mémoires) | — | — | 10 |
| — | — | `tests.rs` (scénarios) | — | — | 7 |
| **total** | **1 603** | **5 344 lignes** | **21** | **43** | **82** |

Il n'y a **pas** de `EntityAIWorkWorker.java` dans cette tranche : les trois fichiers de la
mission sont bien ceux de `workers/service/`, et le quatrième du dossier
(`EntityAIWorkDeliveryman.java`, 662 l.) n'est pas dans le périmètre — son IA est portée
par `crate::ai_build::delivery`.

**Les onze états ajoutés** (`state.rs`) et leur drapeau `isOkayToEat` :
`ENCHANTER_DRAIN` (`:599`, true), `ENCHANT` (`:604`, **false**), `REQUEST_CURE` (`:609`,
true), `CURE` (`:611`, true), `WANDER` (`:613`, true), `FREE_CURE` (`:615`, true),
`CURE_PLAYER` (`:617`, true), `EMPTY_GRAVE` (`:169`, **false**), `DIG_GRAVE` (`:174`,
**false**), `BURY_CITIZEN` (`:179`, **false**), `TRY_RESURRECT` (`:184`, **false**).

**Onze et non douze** : `WANDER` est un seul constant d'`AIWorkerState` que le guérisseur
(`EntityAIWorkHealer.java:93`) et le croque-mort (`EntityAIWorkUndertaker.java:92`)
enregistrent tous les deux. Le module le nomme une fois et ne le donne à aucun des deux
(`ServiceState::belongs_to` répond `false` pour les deux), ce qui est aussi pourquoi
`states_of` l'ajoute explicitement aux deux tables.

## 2. Ce que le test qui compte vérifie

La mission demande « un scénario de faits qui produit la même suite d'états que le Java ».
`tests.rs` en porte trois, un par métier, plus quatre autres :

| Scénario | Suite | Lignes Java |
| --- | --- | --- |
| guérisseur, un patient `NEW`, aucun remède | `IDLE → START_WORKING → DECIDE → REQUEST_CURE → DECIDE` | `:86`, `:87`, `:88`, `:129-133`, `:301-303` |
| croque-mort, aucune tombe | `IDLE → START_WORKING → WANDER → IDLE` | `:90`, `:91`, `:143-144`, `:167` |
| enchanteur, aucun tome, pas de station | `IDLE → START_WORKING → IDLE` + `RequestItem(ancient_tome)` | `AbstractEntityAICrafting.java:164`, `:120-205`, `:199-200` |

Les trois tournent sur `Machine::tick`, donc l'ordre des passes, les cadences
(`ENTITY_AI_TICKRATE = 5`) et la règle de piège sont celles que `src/ai` a déjà testées ;
ce module apporte la table, pas une seconde boucle.

## 3. Trois prémisses fausses, dites plutôt que contournées

### 3.1 L'enchanteur n'est pas un métier de service comme les deux autres

`EntityAIWorkEnchanter` **hérite d'`AbstractEntityAICrafting`** (`:53`). La moitié de sa
table — `IDLE → hasWorkToDo → START_WORKING`, `START_WORKING → decide`,
`QUERY_ITEMS`, `GET_RECIPE`, `CRAFT` — appartient donc à l'atelier, et cette moitié est
**déjà portée** : `crate::ai_build::crafter` (1 504 lignes, ses propres tests) porte
`next_crafting_state` (`AbstractEntityAICrafting.java:298-317`) et les sept états.
Re-porter cette boucle ici aurait été la deuxième copie d'une table testée (règle 6). Le
module **appelle** `next_crafting_state` et rend `Effect::Crafted { state }` : la décision
dit quel état d'atelier vient, l'autre machine le prend. Conséquence mesurable : la table
de l'enchanteur ici fait **3 cibles**, pas 9.

### 3.2 L'enveloppe de 900–1 400 lignes n'est pas tenable pour trois métiers

Mesure, pas opinion : 5 376 lignes pour 1 603 lignes de Java, décomposées ainsi —
**1 215** lignes de documentation citée (chaque règle porte son `fichier.java:ligne`,
règle 3), **2 299** lignes de tests (82 `#[test]`), **1 862** lignes de code et de
séparation. Le code des décisions seuls, hors tests et hors doc, fait donc environ
**1 500** lignes pour trois métiers et 45 refus nommés.
À titre de comparaison, `src/ai_work_06` porte **deux** métiers en **3 295** lignes, et
`src/ai_work_01` trois métiers de plus. Pour tenir 1 400 lignes il faudrait supprimer soit
les 45 refus nommés, soit les citations, soit les scénarios — c'est-à-dire échanger de la
vérifiabilité contre un compteur. Je ne l'ai pas fait, et je le dis ici (règle 7).

Si l'orchestrateur veut l'enveloppe, le seul arbitrage que je recommande est de garder les
trois fichiers de métier et `state.rs`, et de fusionner `facts.rs` dans `mod.rs` : environ
−300 lignes, sans perdre une règle.

### 3.3 La décision « atelier ou drain » de l'enchanteur est écrite avec une garde morte

```java
final IAIState craftState = getNextCraftingState();                  // :129
if (craftState != START_WORKING && !WorldUtil.isPastTime(world, 6000))
{                                                                    // :130
    return craftState;
}
```

`getNextCraftingState` (`AbstractEntityAICrafting.java:298-317`) a quatre `return` : `IDLE`
(`:302`), `INVENTORY_FULL` (`:308`), `QUERY_ITEMS` (`:313`), `GET_RECIPE` (`:316`). **Aucun
n'est `START_WORKING`**, donc `craftState != START_WORKING` est toujours vrai, et
l'enchanteur tient l'atelier dès que `getDayTime() % 24000 > 6000` (`WorldUtil.java:176-179`)
— c'est-à-dire l'après-midi et la nuit — et ne draine que les 6 000 premiers ticks du jour.
Le test `the_crafting_guard_can_never_refuse_…` énumère les huit combinaisons de
`current_task` / `has_recipe` / `has_request` et prouve qu'aucune ne rend
`START_WORKING` : la garde est un no-op de Java, pas une décision de portage. Le port écrit
la comparaison telle quelle.

## 4. Quatre comportements de Java qui surprennent, gardés tels quels

1. **`wantInventoryDumped()` de l'enchanteur rend `false`.** La branche `:135-139` est donc
   morte, alors que le seuil de vidage du métier est d'**une** action (`:214-218`) : le
   `return getState()` de `:138` ne peut être atteint que si la classe de base change. Le
   module garde la branche (`ServiceRefusal::DumpDue`, `EnchanterFacts::want_inventory_dumped`)
   pour qu'un appelant qui la déclenche voie le même refus.
2. **Un croque-mort de compétence 0 ne progresse jamais.** `effortCounter += getPrimarySkillLevel()`
   (`:213`, `:479`) et `+= getSecondarySkillLevel()` (`:343`) : à zéro, le compteur reste à
   zéro, `effortCounter < EFFORT_*` reste vrai, et le métier boucle indéfiniment. Java n'a
   pas de garde ; le port non plus, mais le test
   `emptying_a_grave_takes_a_hundred_effort_points_of_skill` le mesure (dix tours, compteur
   toujours à 0).
3. **`resetDraining()` écrit une position potentiellement nulle.** `setAsGathered(job.getPosToDrainFrom())`
   (`:441`) ne teste pas la cible ; le port n'émet `Effect::MarkStationGathered` que si elle
   existe, parce qu'une position nulle dans `EnchanterStationsModule` n'est pas un état que
   ce port sait produire.
4. **Le drapeau de vidage du croque-mort est consommé, pas lu.** `wantInventoryDumped()`
   (`:507-515`) remet `shouldDumpInventory` à `false` en répondant `true` : la même question
   posée deux fois ne répond pas deux fois oui. `undertaker::want_inventory_dumped` le
   reproduit, et `mod::dump_due` **lit** le drapeau sans le consommer, parce que la
   consommation appartient à la table de base (`AbstractEntityAIBasic.java:414`).

## 5. Ce qui n'est pas couvert, et son coût

- **Les cibles d'atelier de l'enchanteur** (`QUERY_ITEMS`, `GET_RECIPE`, `CRAFT`) : portées
  une fois dans `crate::ai_build::crafter`, pas ici (`MANQUANT.md` §4).
- **Les quatorze cibles de base** : `crate::ai::worker::register_base` ; le module ne porte
  que les cinq transitions de relais (`MANQUANT.md` §1).
- **Les gestes** : les 31 `Effect` sont exécutés par l'appelant, pas par ce module
  (`MANQUANT.md` §5).
- **La persistance** : `JobEnchanter.posToDrainFrom` (`JobEnchanter.java:66-83`),
  `Patient` (`Patient.java:84-93`), `GraveData` et les dossiers de l'hôpital sont des états
  sauvegardés ; le module les porte en **mémoire** (`ServiceMemory`) et ne les sérialise
  pas. C'est `crate::persistence` qui possède le compound du citoyen.
- **Le statut de métier** (`JobStatus.WORKING` / `IDLE` / `STUCK` posé à `:123`, `:138`,
  `:143` du croque-mort) n'est pas porté : il est dérivé de `(Outcome::goal(), Outcome::why_not())`
  par l'appelant. `crate::ai::worker::JobStatus` existe déjà pour ça.
- **Le multiplicateur de butin** du composteur, les particules, les sons, les statistiques
  (`StatsUtil.trackStat`) et les advancements : des effets, hors périmètre d'une décision.

## 6. Les fichiers, et ce que chacun décide

| Fichier | Lignes | Ce qu'il répond |
| --- | ---: | --- |
| `state.rs` | 559 | les 11 états de métier + 7 de base, leur nom Java, leur drapeau, 45 refus nommés |
| `facts.rs` | 825 | ce qu'une décision peut lire : 3 moitiés de faits, 3 mémoires, 1 scène |
| `enchanter.rs` | 854 | atelier ou drain, drainer 60 ticks, enchanter 300/niveau ticks |
| `healer.rs` | 1 080 | un patient à la fois, trois façons de le soigner, 7 cibles |
| `undertaker.rs` | 861 | vider 100, ressusciter 400, enterrer 400 points d'effort |
| `mod.rs` | 962 | le métier, les 5 transitions de relais, la décision et ses ordres |
| `tests.rs` | 235 | les trois suites d'états, comparées au Java |

Le module n'importe **pas** `mod_api` : aucun `use mod_api` dans la tranche, et le seul
endroit où le mot apparaît est la phrase qui le dit (`mod.rs:70`).
