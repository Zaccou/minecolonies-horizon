# RAPPORT — `src/ai_work_07` : trois buts d'IA (mineur du Nether, apiculteur, composteur)

## 0. Résumé

| Métier | Classe Java | États propres | Cibles | Fichier | Tests |
| --- | --- | ---: | ---: | --- | ---: |
| mineur du Nether | `EntityAIWorkNether` | 5 | 6 (+ 1 de hand-over) | `nether.rs` | 25 |
| apiculteur | `EntityAIWorkBeekeeper` | 2 | 5 (+ 1 de hand-over) | `beekeeper.rs` | 18 |
| composteur | `EntityAIWorkComposter` | — (déjà porté) | — | `composter.rs`, une couture | 4 |
| — | les états, les refus, les faits | — | — | `state.rs` (4), `facts.rs` (6) | 10 |
| — | le câblage, les décisions, les coutures | — | — | `mod.rs`, `tests.rs` | 12 |

**69 tests, 69 verts.** La cible de la mission était « 900 à 1400 lignes, 40 tests » : je
suis au-dessus des deux, et c'est mesuré, pas approximé — **4 825 lignes de Rust** dans
`src/ai_work_07` (dont **3 111 de code**, le reste en en-têtes et en citations java), et
**69 tests**. La raison n'est pas l'étalement : `EntityAIWorkNether.java` fait **981
lignes** à lui seul (`decide` = 131 lignes, `stayInNether` = 222), et le style de la maison
documente chaque règle par sa ligne Java — `src/ai/worker.rs` fait 82 Ko pour un fichier
Java de 1 900 lignes. Si le volume est un problème, ce qui se coupe sans perte est
`facts.rs` (les accesseurs `WorkBlocks`) et `composter.rs` (les quatre tests de la couture),
soit ~120 lignes ; le reste est le portage lui-même.

Aucune API du SDK n'est utilisée, aucune `unwrap()` sur une donnée venue d'un fichier.

## 1. Prémisse fausse, et ce que j'ai fait à la place

**La mission demande de porter `EntityAIWorkComposter`. Il est déjà porté.**

Ce n'est pas une impression, c'est mesuré : `src/ai/composter.rs` fait 26 805 octets,
14 tests, et couvre **les six cibles et les huit fonctions** du Java :

| Java | `src/ai/composter.rs` |
| --- | --- |
| `:95-102` les six cibles (`AI_BLOCKING` + cinq) | `register` (`:81-130`), 14 tests |
| `:109-132` `accelerateBarrels` | `barrel_acceleration_ticks` (`:134`) |
| `:139-196` `getMaterials` | `get_materials` (`:176`) + `MaterialsOutcome` |
| `:203-245` `decideWhatToDo` | `decide_what_to_do` (`:141`) + `BarrelChoice` |
| `:252-294` `fillBarrels` | `fill_barrels` (`:315`) |
| `:301-351` `harvestBarrels` | `harvest_barrels` (`:347`) |
| `:359-378` `getLootMultiplier` | `loot_multiplier` (`:243`) |
| `:380-384` `getActionsDoneUntilDumping` | `JobProfile::COMPOSTER` |
| `:390-402` `complain` | `complain` (`:201`) |

Le re-porter ici aurait été la **deuxième copie d'une même table**, ce que la règle 6
interdit, et deux tables pour un métier divergent toujours. J'ai donc fait deux choses :

1. **Une couture** (`composter.rs`) : `composter::ai()` **rend** la table de
   `crate::ai::composter`, et `WorkTrade::ai()` ([`WorkerAi`]) est la seule porte qui répond
   pour les trois métiers. Quatre tests vérifient que la couture rend bien l'engine de
   l'autre module et que le composteur n'a **pas** de table dans le mien.
2. **La seule règle qui manquait vraiment.** `harvest_barrels` de
   `src/ai/composter.rs` s'arrête à `facts.inventory.actions_done += 1` avec un commentaire :
   « *the barrel is emptied by whoever owns it* ». Ce qui décide du **produit** — compost,
   terre ou podzol (`EntityAIWorkComposter.java:316-344`) — n'était nulle part. C'est une
   fonction pure d'un réglage, d'un tirage et d'une recherche, donc [`harvest_product`],
   avec `compost_count` (`TileEntityBarrel.java:322`, les six compost de base) et quatre
   tests : 5 % de podzol, +5 % par recherche, et `<=` (un tirage de 5 donne du podzol).

Je n'ai **pas** touché `src/ai/composter.rs` : il est hors de ma frontière d'écriture. La
règle ajoutée vit donc dans mon dossier et cite sa source ; le jour où l'on voudra la
raccorder, c'est une ligne dans `harvest_barrels`, et c'est une décision d'orchestrateur.

---

## 2. Les deux tables réellement portées

### 2.1 Mineur du Nether — 5 états, 6 cibles

`EntityAIWorkNether` étend `AbstractEntityAICrafting`, donc il **hérite** de
`START_WORKING → decide` (`AbstractEntityAICrafting.java:166`) et n'ajoute que cinq lignes
(`:102-106`), toutes à `TICK_DELAY = 40`.

`decide()` (`:182-312`) est le cœur : **seize branches, et l'ordre est la règle**. Chaque
sortie est un refus nommé, avec sa ligne :

| Ordre | Condition | Sortie | Refus |
| ---: | --- | --- | --- |
| 1 | `isTravelling(...)` (`:186`) | `NETHER_AWAY` | `Travelling` |
| 2 | `job.isInNether()` (`:186`) | `NETHER_AWAY` | `InNether` |
| 3 | `super.decide()` a mieux à faire (`:203`) | reste | `CraftingHalfOwnsIt` |
| 4 | nourriture (`:212-216`) | `GATHERING_REQUIRED_MATERIALS` | — |
| 5 | entrées de la recette (`:232-236`) | `IDLE` + délai 60 | `MissingInputs` |
| 6 | pas de portail (`:239-244`) | `IDLE` + délai 120 | `NoPortal` |
| 7 | un des cinq outils (`:253-258`) | `IDLE` + `STUCK` | `MissingTool(kind)` |
| 8 | pas de recette (`:278`) | reste | `NoRecipe` |
| 9 | `isReadyForTrip()` faux (`:282-287`) | `IDLE` + délai 120 | `TripNotReady` |
| 10 | `walkTo` ou pas à la hutte (`:288-291`) | reste | `NotAtHut` |
| 11 | sac plein (`:292-295`) | `INVENTORY_FULL` | `NoSpace` |
| 12 | `checkForItems` = `GET_RECIPE` (`:299-305`) | `IDLE` + délai 60 | `RecipeLost` |
| 13 | `checkForItems` = `GATHER` (`:306`) | `GATHERING_REQUIRED_MATERIALS` | — |
| 14 | `CRAFT` (`:311`) | `NETHER_LEAVE` | — |

Les quatre autres règles sont portées avec leurs nombres :

- `leaveForNether` (`:317-370`) : sac plein → `INVENTORY_FULL` ; pas de recette → `STUCK`
  + `IDLE` ; portail éteint → `NETHER_OPENPORTAL` ; arrivé → `inNether`, `WORKING`,
  `NETHER_AWAY` ;
- `stayInNether` (`:375-596`) : les **deux files** dans l'ordre — `crafted` puis
  `processed` puis `NETHER_RETURN` —, avec le coût de chaque pile
  (`40 × count` pour un bloc miné, `40 × count` pour un objet simple, `40` sans outil) et
  la mort qui vide les deux files et rend `IDLE` (`:479-490`) ;
- `returnFromNether` (`:631-652`), `openPortal` (`:657-685`), `closePortal` (`:690-713`) ;
- sept fonctions pures pour les nombres du combat et du voyage
  (`incoming_damage`, `attack_damage`, `check_heal`, `heal_saturation_cost`, `xp_on_drop`,
  `vault_travel_ticks`, `gear_for`).

**Ce qui n'est pas porté, et pourquoi c'est dit.** La boucle de combat (`:409-474`) a
besoin d'un `LivingEntity`, d'une `DamageSource`, d'une table de butin et de deux
`nextBoolean()` par round : elle reste à l'appelant, et `MANQUANT.md` §5 liste les six
nombres qu'on lui fournit à la place. `hasWorkToDo()` (`:147-150`) n'est pas porté non plus :
sa première moitié appartient à la table de crafting, qui n'est pas compilée
(`src/crafting` n'est pas déclaré dans `lib.rs`).

### 2.2 Apiculteur — 2 états, 5 cibles

Six lignes de constructeur (`:108-115`), dont la première (`IDLE → START_WORKING`, 1 tick)
est le hand-over.

`decideWhatToDo` (`:200-259`) est une **échelle de quatre marches** : pas de ruche bloque
tout (`:206-211`) ; élevage actif + liste de fleurs vide bloque aussi (`:213-219`) ; une
ruche pleine passe **avant** l'élevage (`:221-226`) ; l'élevage passe avant l'oisiveté
(`:253-256`).

`isReadyForBreeding` (`:441-475`) rend `false` de quatre façons différentes en Java ; le
port les garde séparées (`Breeding::{Off, Full, FewBees, FlowersRequested, FlowersMissing,
Ready}`) parce qu'elles ne demandent pas la même chose au joueur.

Les nombres portés : `NUM_OF_ANIMALS_TO_BREED = 2`, `BEES_PER_LEVEL = 3`,
`NUM_OF_FLOWERS_TO_BREED = 2`, `NUM_OF_WANTED_FLOWERS = 16`, `EXP_PER_HARVEST = 5.0`,
`DISTANCE_TO_BREED = 10`, `HIVE_BEE_RADIUS = 22`, `DECIDING_DELAY = 40`,
`NO_{ANIMALS,HIVES,FLOWERS}_DELAY = 100`, `BREEDING_DELAY = 40`,
`ACTIONS_UNTIL_DUMPING = 5`, `HONEY_LEVEL_FULL = 5`, `1` bouteille / `3` rayons,
`400 + nextInt(400)` ticks de colère, `c(99) = (50 - dex/99*50)/100`.

---

## 3. Le test qui compte : un scénario de faits, la même suite d'états que le Java

Deux tests font tourner la **machine entière**, un tick par état (chaque cadence est
atteinte au premier regard, comme dans `Machine::tick`) :

```
NETHER    IDLE → START_WORKING → NETHER_LEAVE → NETHER_AWAY → NETHER_RETURN
          → INVENTORY_FULL → IDLE
BEEKEEPER IDLE → START_WORKING → PREPARING → DECIDE → BEEKEEPER_HARVEST → START_WORKING
          (et, quand la colonie a de la place : DECIDE → HERDER_BREED → DECIDE)
```

C'est la suite que donnent `EntityAIWorkNether.java:311` → `:364` → `:595` → `:651` et
`EntityAIWorkBeekeeper.java:192` → `:177` → `:225` → `:415`. Le test vérifie aussi que la
règle qui a produit chaque saut est celle qu'on croit (`outcome.why()` rend la ligne Java
de la transition), et qu'aucun état de la table ne piège la machine
(`ai.missing().is_none()`).

Les autres tests se lisent avec le Java à côté : chaque `assert` cite la ligne qui le
décide, et quand deux branches se ressemblent (outil présent / absent, ruche pleine / à
moitié, élevage actif / coupé) les deux sont testées.

---

## 4. Références Java : vérifiées, pas recopiées de mémoire

Un script a relu **177 références absolues** (`Fichier.java:NNN`, avec ou sans intervalle)
présentes dans les six fichiers du module : **23 fichiers distincts, 0 hors bornes**. Deux
erreurs ont été trouvées et corrigées comme ça : `GuardGearBuilder.java:37-52` (le fichier
en fait 44) et `JobStatus.java:3-24` (17 lignes).

Les citations **relatives** (`:NNN`, quand le fichier est annoncé en tête de module) ont été
comptées aussi : 231 dans `nether.rs` (max 973 sur 981 lignes), 175 dans `beekeeper.rs`
(max 608 sur 623), 31 dans `composter.rs` (max 390 sur 409). Aucune hors bornes.

Fichiers lus pour ce lot : `EntityAIWorkNether`, `EntityAIWorkBeekeeper`,
`EntityAIWorkComposter`, `AbstractEntityAICrafting`, `AbstractEntityAIBasic`,
`AIWorkerState`, `BuildingNetherWorker`, `BuildingBeekeeper`, `BuildingComposter`,
`JobBeekeeper`, `JobNetherWorker`, `GuardGearBuilder`, `GuardGear`, `StringSetting`,
`BeekeeperCollectionSetting`, `BuildingModules`, `CitizenConstants`, `GuardConstants`,
`EquipmentLevelConstants`, `ICitizenData`, `ResearchConstants`, `NbtTagConstants`,
`TileEntityBarrel`, `CitizenSkillHandler`, `JobStatus`.

---

## 5. Écarts assumés avec Java (chacun est nommé dans le code)

1. **Deux `NullPointerException` évitées.** `returnFromNether` (`:634`) et `closePortal`
   (`:692-695`) appellent `getBlockState(getPortalLocation())` sans vérifier le tag : une
   hutte enregistrée sans portail lève en Java. Le port lit « pas de portail » et continue.
   Test nommé : `a_hut_saved_without_a_portal_tag_does_not_panic`.
2. **Une division planchée.** `decideWhatToDo` (`:202`) fait `99 / getSecondarySkillLevel()`
   : une compétence à zéro (permise par `CitizenSkillHandler.java:182`) lève en Java.
   `decide_delay` plancher le diviseur à 1 et le dit.
3. **Les cinq outils sont nommés.** Java fait un OU de cinq booléens (`:253`) et ne peut pas
   dire lequel manque ; le port rend le premier manquant dans l'ordre de
   `Job::NetherWorker.tools()` — l'ordre même où les cinq lignes sont écrites.
4. **Les deux files du Nether vivent dans la mémoire, pas dans le job.** En Java ce sont des
   champs de `JobNetherWorker` ; ici c'est `NetherMemory`, et la ligne de chaque champ est
   citée (`JobNetherWorker.java:30`, `:35`, `:40`). C'est le même choix que
   `src/ai_gather` pour les compteurs de métier.
5. **Les tirages sont des faits.** `nextDouble` pour la colère des abeilles (`:400`),
   `nextInt(400)` pour sa durée (`:409`), `nextInt(100)+1` pour le podzol
   (`EntityAIWorkComposter.java:323`) — trois champs marqués « not a Java call », comme
   `WorkerFacts::coin` et `::roll` dans `src/ai`.

---

## 6. Ce que ce lot ne couvre pas

- **Aucune action.** Pas un pas de marche, pas un bloc, pas un objet transféré : la
  décision est séparée de l'action, comme dans `src/ai` et `src/ai_gather`. `MANQUANT.md`
  §6 liste les onze écritures correspondantes.
- **`hasWorkToDo` du mineur du Nether** (`:147-150`), à moitié à `src/crafting`.
- **Le combat et le butin** (voir §2.1) : quatre nombres fournis, la boucle reste dehors.
- **Le composteur**, qui a déjà sa table ailleurs : la seule règle manquante est ajoutée,
  et c'est dit au §1 plutôt que contourné en silence.
- **Les `CRAFT`/`GET_RECIPE`/`QUERY_ITEMS`** de la moitié crafting : le port les nomme
  `CraftingHalfOwnsIt` au lieu de les inventer dans `WorkState`, qui ne les a pas.

---

## 7. État de la vérification

```
ssh deb '/home/zaccou/bin/port-check ai_work_07'
test result: ok. 69 passed; 0 failed; 0 ignored; 0 measured; 2482 filtered out
port-check: VERT — tu peux terminer.
```

**VERT**, relancé après corrections. Trois passes ont été nécessaires, et deux des quatre
échecs de la deuxième étaient **mes tests**, pas le code :

| Échec | Cause | Correction |
| --- | --- | --- |
| `without_a_flower_…` attendait `START_WORKING` | la scène n'avait **qu'une** abeille, donc c'est la paire qui manquait (`:272-289`), pas la fleur | deux abeilles dans la scène |
| `xp_per_ore_…` attendait `(0, 1)` pour « autre minerai » | Java écrit `0`, pas `nextInt(0, 1)` (`:625`) — c'est le test qui avait tort | attente corrigée à `(0, 0)`, citée |
| `herder_breed_…` exigeait que **tous** les autres états soient interruptibles | `INVENTORY_FULL` est `false` aussi (`AIWorkerState.java:23`), mais c'est l'état de `crate::ai` | le test ne parle plus que des états que ce module possède |
| `the_seven_owned_states_…` attendait 2 états « apiculteur seulement » | `PREPARING` et `DECIDE` ne sont enregistrés que par le constructeur de l'apiculteur (`:111-112`) | 4, avec la raison écrite |

Le premier passage rouge, lui, était du **code** : les états propres écrits
`WorkState::Base(WorkerState::NetherAway)` au lieu de `WorkState::NetherAway`, deux `Copy`
sur des types qui portent un `Vec`, et une fonction de test `scene()` masquée par une
variable `scene` — trois erreurs que le compilateur a nommées et que j'ai corrigées sans
toucher à une attente.

Une correction de fond est venue du compilateur, et elle a amélioré le modèle :
`WorkState::belongs_to` disait « tout état de base appartient à tous les métiers », ce qui
faisait de `PREPARING` et `DECIDE` des états de la table du mineur du Nether — deux états
que rien ne peut quitter, donc deux pièges au sens de
`BasicStateMachine.java:220-228`. Corrigé, et le test de câblage vérifie maintenant que
**chaque** état d'une table a une transition sortante.

Le build des tests a été bloqué deux fois par des dossiers d'autres agents
(`src/ai_work_08` sans `tests.rs`, puis `src/ai_work_09` en cours d'écriture) : je n'ai rien
touché chez eux, j'ai relancé.
