# RAPPORT — `ai_work_05`, les buts d'IA de la pierre taillée, du tailleur de pierre et de l'écolier

## 1. En une phrase

Deux des trois fichiers nommés par le brief **ne contiennent aucune machine à états**
(27 et 28 lignes, une seule méthode surchargée chacun) ; leur table est celle de leur
classe mère et elle est déjà portée ailleurs. Le troisième, l'écolier, a un vrai but :
il est porté ici, en entier, avec ses cinq cibles, ses deux états neufs, ses sept
refus nommés, sa durée de leçon et 54 tests — et les deux alias reçoivent ce qu'ils
ont réellement en propre : une porte de recette, un ordre de courses, trois gardes de
four et deux sons.

## 2. La prémisse du brief, corrigée par la mesure

| Fichier nommé | Lignes | `registerTargets` | `static final` | états nommés | ce qu'il surcharge |
| --- | ---: | ---: | ---: | ---: | --- |
| `crafting/EntityAIWorkStoneSmeltery.java` | 27 | **0** | **0** | **0** | `getExpectedBuildingClass` (`:22-26`) |
| `crafting/EntityAIWorkStonemason.java` | 28 | **0** | **0** | **0** | `getExpectedBuildingClass` (`:23-27`) |
| `education/EntityAIWorkPupil.java` | 224 | 1 (5 cibles) | 1 (`:42`) | 2 (`STUDY`, `RECESS`) | `getExpectedBuildingClass` |

Les vraies sources de comportement, que le brief ne nomme pas :

- tailleur de pierre → `crafting/AbstractEntityAICrafting.java` (785 lignes,
  `registerTargets` à `:160-170`) ;
- pierre taillée → le même, **plus** `crafting/AbstractEntityAIRequestSmelter.java`
  (760 lignes, `registerTargets` à `:71-82`) ;
- et, pour les deux, la hutte : `BuildingStonemason.java` (`:53-80`) et
  `BuildingStoneSmeltery.java` (`:51-77`), qui portent **le seul vrai filtre** du
  métier — le tag d'objets des recettes ;
- et le métier sonore : `JobStonemason.java:37-41`, `JobStoneSmeltery.java:37-49`.

C'est la correction de la prémisse, pas un contournement : la table des deux premiers
métiers n'est pas « à porter », elle est **déjà portée** et ce module s'y raccorde
(§4).

### 2.1 La liste à corriger dans le générateur de missions

**Nommées à tort comme source des buts d'IA** :

| Classe nommée par le brief | Ce qu'elle est | La classe qui porte vraiment les buts |
| --- | --- | --- |
| `EntityAIWorkStoneSmeltery` (27 l.) | un alias : constructeur qui relaie + `getExpectedBuildingClass` (`:22-26`), **0 `registerTargets`, 0 constante, 0 état** | `AbstractEntityAIRequestSmelter` (760 l., 7 cibles à `:71-82`) **+** `AbstractEntityAICrafting` (785 l., 6 cibles à `:160-170`) |
| `EntityAIWorkStonemason` (28 l.) | un alias : constructeur qui relaie + `getExpectedBuildingClass` (`:23-27`), **0 `registerTargets`, 0 constante, 0 état** | `AbstractEntityAICrafting` (785 l., 6 cibles à `:160-170`) |

**Justes** : `EntityAIWorkPupil` (224 l.) est bien la source — cinq `AITarget`
(`:72-78`), quatre méthodes de décision, deux états neufs.

**Oubliées par le brief, et nécessaires** (chacune porte une règle du métier) :

| Classe | Ce qu'elle porte, et que l'IA ne dit pas | Ligne |
| --- | --- | --- |
| `AbstractEntityAIRequestSmelter` | la route du four, l'ordre des six courses, les trois refus de four | `:426-500`, `:439-463` |
| `AbstractEntityAICrafting` | la table d'atelier, la durée d'une recette, les tags de recette | `:160-170`, `:734-738` |
| `BuildingStoneSmeltery` | le tag `stonesmeltery` et le module `Smelting` (four obligatoire) | `:51-77` |
| `BuildingStonemason` | le tag `stonemason`, filtre le plus strict des douze crafters | `:53-80` |
| `JobStoneSmeltery`, `JobStonemason` | les sons du métier, seules vraies surcharges des deux `Job*` | `:37-49`, `:37-41` |
| `BuildingSchool` | `getRandomPlaceToSit` : tag `sit` puis tapis, sans quoi l'écolier reste en `NoSeat` | `:125-144` |

Le signe qui aurait dû déclencher l'alerte dans le générateur : **une classe de moins de
40 lignes qui ne surcharge que `getExpectedBuildingClass` n'est jamais la source d'un
but** — ses états sont dans son `extends`. Compter les `AITarget`/`AITarget`-like du
fichier suffit à trancher (`EntityAIWorkStoneSmeltery` : 0 ; `EntityAIWorkPupil` : 5).

## 3. Ce qui est porté, et où

| Fichier | Lignes | Tests | Ce qu'il porte |
| --- | ---: | ---: | --- |
| `mod.rs` | 215 | 0 | la mesure du brief, `Trade` (3), les constantes communes |
| `state.rs` | 371 | 5 | `PupilState` (8 états dont 6 de base empruntés), `PupilRefusal` (7), `Decision` |
| `facts.rs` | 301 | 4 | `PupilFacts`, `Seat`, `PupilMemory`, `PupilScene` |
| `pupil.rs` | 1 063 | 17 | les 5 cibles, les 4 décisions, les 13 demandes, la durée de leçon |
| `crafting_shop.rs` | 854 | 11 | le delta des deux alias, la porte de recette, l'ordre des courses, les sons, les durées |
| `tests.rs` | 434 | 17 | trois scénarios de bout en bout + la mesure des trois métiers |
| **total Rust** | **3 238** | **54** | |

Le brief visait 900–1 400 lignes. Le dépassement est assumé et localisé : deux des
trois métiers sont des alias, donc tout le volume est dans l'écolier (1 063 lignes
pour 224 lignes de Java) et dans sa documentation — le dépôt exige `fichier.java:ligne`
au-dessus de chaque règle et un champ `source` par constante, ce qui coûte environ un
tiers des lignes. Les modules voisins du même domaine sont du même ordre
(`src/ai_gather/lumberjack.rs` : 35 Ko pour un métier).

## 4. La machine de l'écolier

### 4.1 La table

| # | État | Règle | Rythme | Java |
| ---: | --- | --- | ---: | --- |
| 1 | `IDLE` | `→ START_WORKING`, sans condition | 1 | `EntityAIWorkPupil.java:73` |
| 2 | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:74` |
| 3 | `DECIDE` | `decide` | 20 | `:75` |
| 4 | `STUDY` | `study` | 20 | `:76` |
| 5 | `RECESS` | `recess` | 20 | `:77` |
| — | `INVENTORY_FULL` | `afterDump` → `IDLE` | 5 | `AbstractEntityAIBasic.java:1213-1222` |
| — | `NEEDS_ITEM` | `waitForRequests` → `IDLE` | 5 | `AbstractEntityAIBasic.java:238` |
| — | `PAUSED` | `bePaused` → `IDLE` | 20 | `AbstractEntityAIBasic.java:251` |

Huit états, dont six empruntés à `crate::ai::state::WorkerState` — `DECIDE` y est
déjà (`AIWorkerState.java:59`), il n'est pas réinventé. `EntityAIWorkPupil` en crée
deux : `STUDY` (`:439`) et `RECESS` (`:624`), tous deux `isOkayToEat = true`.

Les quatorze cibles de base ne sont **pas** dans cette table : elles sont
`crate::ai::worker::register_base` sur `WorkerState`, et une `Machine<S, F>` ne peut
pas en contenir une autre. Le pont est celui de `crate::ai_gather` — trois états de
base, trois retours vers `IDLE`.

### 4.2 La suite d'états, et pourquoi elle surprend

`IDLE → START_WORKING → STUDY → DECIDE → STUDY → (leçons enchaînées)`

- `START_WORKING` va **droit en `STUDY`** (`:222`), pas en `DECIDE` ; c'est le
  `studyPos == null` du premier `study` qui renvoie en `DECIDE` (`:129-132`). Un port
  qui ferait le saut ici décalerait la suite d'un tick.
- Une leçon finie, `study` rend `null` (`:202`) : **l'état ne change pas** et le tick
  suivant rassoit le citoyen (`:146-151`) avec une durée neuve. Un écolier ne quitte
  donc `STUDY` que par un siège occupé (`:139-144`) ou par une récréation.
- `DECIDE` sort une fois sur dix en récréation (`:89`) **avant** même de chercher un
  siège : une école sans tapis prend quand même des récréations.

### 4.3 Les deux unités de temps (le piège du fichier)

`maxSittingTicks` (`:52`) est tiré **en secondes** — `nextInt(120 / 2) + 60`, donc
60 à 119 (`:149`) — et `sittingTicks` (`:57`, `:166`) compte les passages de `study`,
dont chacun dure `TICKS_SECOND = 20` ticks (`:76`), c'est-à-dire une seconde de jeu.
La comparaison de `:167` n'est juste que parce que les deux sont des secondes, malgré
un nom qui dit « ticks ». Le port le nomme (`LessonProgress`, `sit_seconds`,
`sit_ticks`) et le teste :

| tirage | secondes | ticks passés à `Sit` |
| ---: | ---: | ---: |
| 0 | 60 | 1 200 |
| 59 | 119 | 2 380 |

Conséquence mesurée en test (`the_lesson_lasts_as_many_study_calls_as_the_draw_says`) :
avec le tirage maximal, une leçon dure **119 passages de `STUDY`**, pas 60.

### 4.4 Les refus nommés

| Refus | Ligne | Ce qui se passe |
| --- | --- | --- |
| `NoSeat` | `EntityAIWorkPupil.java:96-100` | `DECIDE` + interaction **bloquante** `PUPIL_NO_CARPET` (`TranslationConstants.java:344`) |
| `StaleCarpet` | `BuildingSchool.java:138-143` | idem, mais la cause est un tapis disparu que la hutte doit oublier |
| `SeatTaken` | `EntityAIWorkPupil.java:139-144` | `studyPos` effacé, retour en `DECIDE` |
| `NotAtBuilding` | `:218-221` | reste en `START_WORKING`, redemande la marche |
| `NotAtSeat` | `:134-137` | reste en `STUDY`, redemande la marche |
| `NoStudyPos` | `:129-132` | retour en `DECIDE` |
| `NoPaper` | `:160-164` | **n'arrête rien** : la leçon a lieu, paie 0 (`:179-196`) |

### 4.5 Les durées, chaque constante citée

| Nom | Valeur | Java |
| --- | ---: | --- |
| `STUDY_TO_RECESS_RATIO` | 10 | `EntityAIWorkPupil.java:42` |
| plancher de la leçon | 60 s | `:149` |
| amplitude du tirage | `120 / 2` | `:149` |
| durée passée à `sitDown` | `maxSittingTicks * 20` | `:150` |
| rayon de la promenade | 10 | `:118` |
| XP d'une leçon avec papier | `50.0 * (1 + TEACHING)` | `:186-188` |
| rythmes des 5 cibles | 1, 20, 20, 20, 20 | `:73-77` |

## 5. Les deux métiers de la pierre

### 5.1 Ce qu'ils ajoutent

| | tailleur de pierre | pierre taillée |
| --- | --- | --- |
| cibles ajoutées | 0 (20 au total) | 0 (27 au total : +7 du `RequestSmelter`) |
| constante déclarée | 0 | 0 |
| porte de recette | `#minecolonies:stonemason` (`BuildingStonemason.java:69`, `:77`) | `#minecolonies:stonesmeltery` (`BuildingStoneSmeltery.java:67`, `:75`) |
| route du four | aucune | oui, si `getIntermediate() == Blocks.FURNACE` (`AbstractEntityAIRequestSmelter.java:434-437`) |
| sons | `DEEPSLATE_TILES_HIT`, volume 1.0, hauteur 2.0 (`JobStonemason.java:40`) | 1/10 `REDSTONE_TORCH_BURNOUT`, sinon `LAVA_POP` + `LAVA_AMBIENT` (`JobStoneSmeltery.java:40-48`) |
| durées propres | aucune | 200 / 20 / 200 ticks de fond, 20 par four, 20 d'attente (`:73-75`, `:78-81`, `:489`) |

### 5.2 Ce que ce module ne refait pas

- **La table d'atelier** : `crate::ai_build::crafter` porte
  `AbstractEntityAICrafting.java:160-170` en entier. `crafting_shop::machine(Stonemason)`
  rend **sa** machine, et un test vérifie que la liste des règles est identique à
  `crafter::ai()` — s'il casse, c'est le crafter qui a changé, pas ce module.
- **La table des fours** : sept cibles de `AbstractEntityAIRequestSmelter.java:71-82`
  sur d'autres états et d'autres faits. Trois modules voisins les portent
  (`src/ai_work_01/furnace.rs`, `src/ai_work_02/smelter.rs`,
  `src/ai_work_03/smelter.rs`). Une quatrième copie vaudrait moins qu'une. Ce module
  porte **la décision** : `ERRAND_ORDER` (l'ordre exact de `executeCraftingAction`,
  `:465-499`) et `PRE_GUARDS` (les trois refus de four, `:439-463`), en données
  testables, avec l'état d'arrivée et la ligne de chaque course.

L'ordre est la règle et il est testé : sortir un produit fini (`:465`) passe avant
nettoyer une entrée étrangère (`:471`), qui passe avant remplir un four en carburant
(`:477`), qui passe avant attendre que ça brûle (`:486`), qui passe avant remplir un
four vide (`:493`), et le retour par défaut est `START_WORKING` (`:499`).

## 6. Les 54 tests

**Règles unitaires, à côté de la règle** (37) :

- `state.rs` (5) : les huit états et les deux états propres, `okay_to_eat` sur
  `STUDY`/`RECESS`, une ligne Java distincte par état, un seul refus avec message,
  la forme de `Decision` (`go`/`stay`/`refuse`/`because`).
- `facts.rs` (4) : le défaut est une école pauvre, les deux listes de sièges restent
  séparées, un tapis périmé compte encore dans la liste, une scène porte le monde, la
  mémoire et zéro demande.
- `pupil.rs` (17) : la durée 60–119 s et ses deux conversions ; l'XP `50 × (1+TEACHING)`
  et le zéro sans papier ; le tag `sit` prioritaire et l'indice ramené ; le tapis
  périmé oublié ; 1 récréation sur 10 ; école vide → `DECIDE` + plainte et `studyPos`
  conservé ; les deux conditions de fin de récréation ; première leçon = on s'assoit
  et on compte 1 s ; leçon sans papier ; fin de leçon (7 demandes dans l'ordre) ;
  siège occupé ; pas de siège ; pas au siège ; `START_WORKING → STUDY` direct ; les 5
  cibles + 3 retours avec leurs rythmes ; tout état a une sortie ; `LessonProgress`.
- `crafting_shop.rs` (11) : delta nul des deux alias ; chaînes = total des cibles ;
  seule la pierre taillée connaît un four ; une recette hors tag n'atteint pas les
  fours ; route four/atelier ; liste de carburants amputée des items de la recette ;
  ordre des six courses ; les trois gardes ; les six durées ; les sons 9/10 ; la table
  du tailleur est celle du crafter.

**Scénarios de bout en bout** (`tests.rs`, 17) : la suite `IDLE→START_WORKING→STUDY→
DECIDE→STUDY` tick par tick ; la première leçon ; les leçons enchaînées sans sortir de
`STUDY` ; 119 passages pour la leçon maximale ; une récréation sur dix puis le retour ;
une récréation qui ne finit pas si la marche n'est pas finie ; école vide = 25 plaintes
en 100 ticks ; tapis périmé ; siège occupé puis reprise ; leçon sans papier ; marche
vers l'école ; marche vers le siège ; tout état a une transition ; la mesure des trois
métiers ; la table du tailleur est celle du crafter ; le delta de la pierre taillée ;
la couture avec `crate::ai`.

## 7. Écarts assumés, chacun justifié

1. **`walkToWorkPos` est documenté « true while walking » (`AbstractEntityAIBasic.java:
   797`) et rend « arrivé »** (`EntityNavigationUtils.java:111`, `:129`, et le code de
   `EntityAIWorkPupil.java:134-137` qui l'utilise comme « pas encore arrivé »). Le port
   suit le code, pas la javadoc.
2. **La boîte du siège est dégénérée** : `new AABB(x, y, z, x, y, z)`
   (`EntityAIWorkPupil.java:139-140`) est un point, pas un cube. Le port en fait un
   fait booléen (`other_citizen_at_seat`) et le documente, plutôt que de recopier une
   requête qui ne peut être fidèle sans le monde.
3. **La fin de leçon a un décalage d'un dans Java** : `sittingTicks++` puis
   `sittingTicks < maxSittingTicks` (`:166-167`) fait durer une leçon de 60 exactement
   60 passages. Le port reproduit le même décalage, il ne le corrige pas.
4. **Les tirages sont des faits, ramenés par `%`** : `worker.getRandom()` et
   `ColonyConstants.rand` ne se rejouent pas. Aucun tirage hors bornes ne panique.
5. **`GATHERING_REQUIRED_MATERIALS` n'est pas dans la table** : l'écolier ne crée
   aucune requête et ne pose jamais `needsCurrently` ; annoncer cet état serait mentir
   sur la table. Il n'est donc pas dans `PupilState::ALL`.
6. **`recess` ignore le retour de `walkToRandomPosWithin`** (`:118`) : Java ne le lit
   pas non plus, le port rend la demande et ne l'interprète pas.
7. **Le défaut de `recess_roll` est `1`, pas `0`** : `0` est le tirage rare
   (`nextInt(10) < 1`) ; un défaut à `0` enverrait toutes les scènes de test en
   récréation. Le piège est documenté sur le champ.
8. **Compilation et tests : verts.** Premier `port-check ai_work_05` :
   `test result: ok. 54 passed; 0 failed`. Les quatre échecs du premier passage sont
   tranchés au §9 — quatre attentes, aucun bug de code.

## 8. Ce qui manque pour agir (voir `MANQUANT.md`)

Aucune des treize demandes de `PupilAsk` n'est exécutable avec le SDK actuel : le crate
`horizon-mod-api` ne connaît ni citoyen, ni navigation, ni monture, ni compétence, ni
satiété, ni émission de particules vers des clients — et le port n'a pas de lecteur
d'état de bloc vivant ni d'annotations de tag de schéma. Les deux derniers sont les
plus gênants : sans lecteur de tags, une école réelle n'aura que ses tapis, jamais ses
sièges de schéma, et l'écolier sera bien plus souvent en `NoSeat` qu'en jeu.

## 9. Les quatre échecs du premier passage, tranchés contre le Java

Premier `port-check` : **4 tests rouges sur 54**. Verdict : **les quatre étaient des
attentes fausses, aucun bug de code** — et trois venaient de la même méconnaissance de
la machine, celle du **compteur de transition**.

| Test | Ce qui échouait | Verdict | La ligne Java qui tranche |
| --- | --- | --- | --- |
| `a_pupil_that_is_not_in_its_school_keeps_walking_first` | `scene.refusal == Some(NotAtBuilding)` rendait `None`, relevé 40 ticks après coup | **attente** | `EntityAIWorkPupil.java:74` : la cible a le rythme `TICKS_SECOND`, elle n'est donc *appelée* qu'un tick de machine sur quatre — le refus est le fait de l'appel, pas un état qui persiste |
| `the_first_study_of_the_first_lesson_sits_the_citizen` | la demande `Sit` n'arrivait pas au tick suivant l'entrée en `STUDY` | **attente** | `TickRateStateMachine.java:118-123` + `TickingTransition.java:118-128` : le compteur appartient à la transition et n'est pas remis à zéro quand on (re)entre dans l'état ; `study` avait déjà tiré au tick 3 (`:130`, `studyPos == null`), son compteur valait encore 20 |
| `one_draw_in_ten_sends_the_pupil_to_recess` | après la fin de récréation, `STUDY` attendu sous 4 ticks ; seul `START_WORKING` venait | **attente** | même compteur : `startWorkingAtOwnBuilding` (`:74`) avait été armé au tick 2 et jamais décrémenté, la machine n'étant pas dans son état (`TickRateStateMachine.java:101`) |
| `the_three_trades_and_what_each_one_really_carries` | `job().path()` rend `stonesmeltery`, le test attendait `minecolonies:stonesmeltery` | **attente** | `jobs/job.rs:52-56` : « The registry path, **without its namespace** », `ModJobs.java:14` ; c'est `Kind::path` qui porte le préfixe (`building/kind.rs:126`) |

Le second passage a révélé une cinquième attente fausse, la mienne, écrite en corrigeant
la quatrième : pour l'écolier **le métier et la hutte n'ont pas le même identifiant** —
`pupil` (`ModJobs.java:47`) embauché par `school` (`BuildingSchool.java:32`), là où la
pierre taillée porte le même des deux côtés. Le test distingue désormais les deux
colonnes, ce qui justifie que `Trade` réponde `job()` et `building()` séparément.

**Deux tests voisins ont été durcis dans la même passe**, sans être supprimés : ils
passaient par chance de phase (refus attendu après un nombre de ticks multiple de 4) et
disent maintenant explicitement qu'ils relèvent le refus sur les ticks qui *tirent*.

Les quatre verdicts se tiennent à une preuve mécanique : **aucune ligne de `pupil.rs`,
`state.rs`, `facts.rs`, `crafting_shop.rs` ni `mod.rs` n'a bougé pour faire passer un
test** — seul `tests.rs` a changé. Quand l'attente avait tort, corriger le code aurait
été porter du Java faux.

### Comment les quatre échecs ont été invisibles pendant 20 minutes

Le `cargo test` du dépôt ne compilait plus, à cause de `src/ai_work_08/tests.rs`
(module d'un autre agent, en cours d'écriture) : le message de `port-check` ne parlait
que de lui. Les quatre tests ont donc été isolés dans une **copie jetable hors du
dépôt** (`/tmp/ai05-probe`, même `flock` de build, `ai_work_08` retiré de la copie
seulement), ce qui a donné les quatre noms et leur raison en un passage ; la
vérification finale, elle, a bien été faite dans le dépôt, `port-check ai_work_05` :
`test result: ok. 54 passed; 0 failed; ... port-check: VERT`.
