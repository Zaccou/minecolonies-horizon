# RAPPORT — buts d'éducation (`src/ai_work_06`)

**État : `port-check ai_work_06` → VERT, 55 tests passés, 0 échec** (2 496 autres tests du
crate filtrés). Aucune erreur ni aucun avertissement ne cite `src/ai_work_06/` : les quatre
avertissements restants du build pointent `commands/reply.rs`, `research/progress.rs` et
`ai_work_08/`, tous antérieurs ou extérieurs à ce module.

## 1. Deux des trois sources de la mission, parce que la troisième était déjà faite

La mission cite trois fichiers. Deux sont portés ici ; le troisième était **déjà porté**,
en entier, avant mon passage :

| Demandé | État mesuré | Preuve |
| --- | --- | --- |
| `education/EntityAIWorkResearcher.java` (126 l.) | **porté ici** | `researcher.rs` |
| `education/EntityAIWorkTeacher.java` (237 l.) | **porté ici** | `teacher.rs` |
| `production/EntityAIWorkLumberjack.java` | **déjà porté** | `src/ai_gather/lumberjack.rs`, 837 lignes, 15 `#[test]` |

Le bûcheron d'`ai_gather` couvre ses huit cibles (`EntityAIWorkLumberjack.java:193-201`
plus l'override de `START_WORKING` à `:224-261`), ses onze constantes avec leur ligne, sa
recherche d'arbre, sa récolte, ses deux temps d'attente et son sous-état `plantSapling`.
Le re-porter dans `src/ai_work_06` aurait été la deuxième copie d'une table déjà testée
(règle 6). Je ne l'ai donc pas fait, et je le dis ici plutôt que de le taire — c'était la
prémisse fausse de la mission (règle 7).

Le même dossier `education/` contient un quatrième fichier, `EntityAIWorkPupil.java` (224
l.), dont l'IA est en cours de port par `src/ai_work_05` (`PupilState`, `PupilRefusal`). Je
ne l'ai **pas** touché : le professeur lit seulement les élèves comme des **faits**
(`Pupil`), ce qui est la frontière correcte et évite deux machines concurrentes pour un
même état `STUDY` (`AIWorkerState.java:439`).

## 2. Ce qui est porté

| Métier | Classe Java | États de métier | Cibles | Seuil de vidage | Fichier |
| --- | --- | ---: | ---: | ---: | --- |
| chercheur | `EntityAIWorkResearcher.java:19` | 1 (`STUDY`) | 3 | 32 (base, `:425-428`) | `researcher.rs` |
| professeur | `EntityAIWorkTeacher.java:34` | 1 (`TEACH`) | 4 | 50 (`:203`) | `teacher.rs` |

Une seule machine par métier, la même que `crate::ai` : `Machine`, `Transition`, `Action`,
`Condition` (`src/ai/state.rs`), rien n'est réécrit. `EduState` tient les deux états de
métier **et** les sept états de base traversés, en un seul enum, comme `AIWorkerState` en
tient 127 dans un seul (`state.rs`).

**Les tables, ligne par ligne.**

| Métier | État | Règle | Taux | Java |
| --- | --- | --- | ---: | --- |
| chercheur | `IDLE` | → `START_WORKING` | 1 | `EntityAIWorkResearcher.java:50` |
| chercheur | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:51` |
| chercheur | `STUDY` | `study` | 20 | `:52` |
| professeur | `IDLE` | → `START_WORKING` | 1 | `EntityAIWorkTeacher.java:76` |
| professeur | `START_WORKING` | `startWorkingAtOwnBuilding` | 20 | `:77` |
| professeur | `DECIDE` | `decide` | 20 | `:78` |
| professeur | `TEACH` | `teach` | 20 | `:79` |

Plus les **cinq** transitions de raccord vers la table de base (`register_handover`), une
par état traversé, chacune citant sa ligne : `INVENTORY_FULL → IDLE`
(`AbstractEntityAIBasic.java:1213-1222`), `NEEDS_ITEM → IDLE` (`:736-739`), `PAUSED → IDLE`
(`:251`), `GATHERING_REQUIRED_MATERIALS → START_WORKING` (`:323-326`, le ramassage du papier
du professeur) et `IDLE → entrée du métier`.

**Les constantes, chacune avec sa ligne.**

| Constante | Valeur | Source |
| --- | ---: | --- |
| `STUDY_DELAY` | 1200 ticks | `EntityAIWorkResearcher.java:24` |
| `XP_PER_STUDYPOS` | 2.0 | `:29` |
| `MANA_PER_SECOND` | 144 s | `JobResearch.java:23` |
| `MAX_TICKRATE` / `_PER_SECOND` | 500 / 25 | `TickRateConstants.java:11`, `JobResearch.java:28` |
| `PAPER_TO_REQUEST` | 16 | `EntityAIWorkTeacher.java:39` |
| `ACTIONS_UNTIL_DUMPING` | 50 | `:203` |
| `TEACHER_XP` | 0.1 | `:169` |
| `XP_BASE` / `SKILL_DIVISOR` | 1.5 / 10.0 | `:163`, `:165` |
| `SIT_MODIFIER_BASE` | 100 | `:133` |
| `SIT_TICK_FACTOR` | 20 | `:136` |
| `PAPER_PER_LESSON` | 1 | `:158` |
| `TICKS_SECOND` | 20 | `Constants.java:21` |

**Les 16 refus nommés** (`state.rs`), chacun avec sa ligne Java et sa clé de message :
`NoFactsForTrade`, `NotAtBuilding`, `NotAtBookshelf`, `WalkAbandoned`,
`NoResearchInProgress`, `NoMana`, `NotAtPupil`, `PupilNotChosen`, `PupilNotSeated`,
`Sitting`, `NoPaper`, `PupilHasPaper`, `PaperMissing`, `PaperInHutOnly`, `NoPupil`,
`InventoryDumpDue`. Six d'entre eux (`still_acts()`) sont notés sur une passe qui **agit
quand même** : Java étudie là où il est et paie l'expérience, le professeur enseigne sans
papier. Un refus n'est pas un échec, et le type le dit.

**Les sept ordres** (`Effect`) que la décision laisse à l'appelant, parce que ce sont des
écritures et pas des décisions : `Studied { xp, research }`, `RequestPaper { count }`,
`FetchPaper { count }`, `Sit { at, ticks }`, `GivePaper { pupil, count }`,
`Taught { pupil, pupil_xp, teacher_xp }`.

## 3. Le seam avec `crate::ai`, dit deux fois plutôt que contourné

`Machine<S, F>` est monomorphe : une machine d'éducation ne peut pas **contenir** la table
de base. Un appelant fait donc tourner `JobAi` (base) puis `EduAi` (métier) sur le **même**
`WorldFacts` — aucune valeur n'est recopiée. Conséquence unique, écrite dans `MANQUANT.md`
§1 : `setDelay(1200)` est **enregistré** (`memory.base.delay`) mais son décompte appartient à
`waitingForSomething` (`AbstractEntityAIBasic.java:496-517`), donc à l'autre table. Le test
`the_researcher_sets_a_one_minute_delay_the_base_table_owns` épingle la valeur que
l'appelant doit respecter, plutôt que de simuler un délai qui n'est pas le mien.

## 4. Cinq constats mesurés — et ce qu'ils changent

1. **`getRandomBookShelf` ne rend jamais `null`.**
   `BuildingUniversity.java:115-128` répond `getPosition()` (la hutte elle-même) quand la
   liste de bibliothèques est vide, ou quand l'étagère tirée n'est plus un `BOOKSHELVES`. Il
   n'y a donc **pas** de refus « pas de bibliothèque » à porter : une université sans
   étagère fait étudier son chercheur à la porte. Prémisse fausse d'un refus évident.

2. **Le commentaire de Java sur la durée d'une leçon est faux.** `EntityAIWorkTeacher.java:
   132` annonce « 2-100 secondes ». Mesuré sur la formule (`:133-134`), la plage réelle est
   **1 à 99** : Mana 0 → modificateur 100 → 50..99 s ; Mana 99 → 100/49.5 = 2.02 → tronqué
   à 2 → 1 s (`nextInt(1)` = 0, plus 1). Le port copie l'arithmétique et le test
   `the_sitting_time_divides_a_hundred_by_half_the_mana_level` tient les deux bornes.

3. **`nextInt(0)` est inatteignable, mais il lèverait.** Si le modificateur valait 1,
   `maxSittingTicks = nextInt(jobModifier / 2)` diviserait par zéro et Java lèverait
   `IllegalArgumentException`. Il faut `level(Mana) > 99`, ce que le plafond de compétence
   interdit. Le port garde `half <= 0 → modifier` au lieu de diviser : une donnée corrompue
   ne fait pas tomber le serveur.

4. **La mana du chercheur ne se remplit jamais en jeu, et c'est une impasse.**
   `currentMana` n'est écrit que par `JobResearch.processOfflineTime`
   (`JobResearch.java:77-93`), c'est-à-dire quand la colonie était **hors ligne**. Le seul
   lecteur est `EntityAIWorkResearcher.java:84` : à mana 0, le chercheur arrête d'avancer
   des recherches et continue de payer son expérience. Sur un serveur qui ne s'arrête
   jamais, une université avance donc exactement `currentMana` passes puis plus rien. Ce
   n'est pas une décision de portage : `mana_after_offline` est porté et testé pour que la
   mécanique soit visible, et `NoMana` la nomme au joueur. Le corriger demanderait un
   réglage d'équilibrage, pas une ligne de plus dans ce module.

5. **Le « mana » du chercheur n'est lu nulle part ailleurs dans l'arbre.** Mesuré :
   `grep -rn "getCurrentMana\|reduceCurrentMana"` ne rend que `JobResearch.java` et
   `EntityAIWorkResearcher.java` — le commentaire de `JobResearch.java:36-37` (« tick
   randomly the research additionally ») décrit un mécanisme qui n'existe pas dans cette
   version. C'est cohérent avec le constat 4 et ça évite de chercher un consommateur absent.

## 5. Les tests : 55, dont 10 scénarios de faits

| Fichier | Tests | Ce qu'ils tiennent |
| --- | ---: | --- |
| `state.rs` | 5 | les 9 états, `isOkayToEat`, l'appartenance par métier, les 16 refus, `still_acts` |
| `facts.rs` | 7 | `next_concludes`, le tirage modulo, le filtre des élèves, le papier, une scène vide |
| `researcher.rs` | 9 | l'attente à la hutte, l'étagère, les 60 s de marche, la passe sans recherche, la mana dépensée, le délai |
| `teacher.rs` | 17 | la requête de papier, la récupération, l'élève absent/debout, l'assise, la remise, la formule d'XP, les bornes du temps d'assise |
| `mod.rs` | 7 | la table de métiers, le seuil de vidage, les 5 raccords, les faits de l'autre métier, `states_of` |
| `tests.rs` | 10 | **les scénarios** : une suite d'états comparée à Java, pas un appel isolé |

Les quatre scénarios qui comptent vraiment, avec la suite attendue :

- chercheur : `IDLE → START_WORKING → STUDY → START_WORKING → STUDY → …`, trois passes en
  20 ticks machine, 3 mana dépensée, 3 recherches désignées
  (`the_researcher_runs_idle_start_working_study_and_back`) ;
- chercheur qui n'arrive pas : `IDLE → START_WORKING → STUDY` puis **STUDY qui tient**
  pendant 10 passes, 200 de budget de marche, zéro mana dépensée, zéro effet ;
- professeur : `IDLE → START_WORKING → DECIDE → TEACH → START_WORKING → DECIDE → TEACH`,
  une feuille remise, une leçon payée, un siège de 20 ticks (`mana 99` → leçon de 1 s) ;
- école sans élève assis : `START_WORKING → DECIDE` en boucle, `NoPupil` à chaque tour,
  aucun effet — la boucle que `EntityAIWorkTeacher.java:114` fabrique réellement.

Et un test de sûreté : sur 40 ticks des deux métiers, aucun état atteint n'est un piège
(`can_leave` vrai partout, `missing()` vide), ce qui est l'invariant de
`BasicStateMachine.java:220-228`.

## 6. Ce que la vérification a réellement prouvé

`port-check ai_work_06` est **VERT** : 55 tests passés, 0 échec, et la sortie ne cite
`src/ai_work_06/` ni en erreur ni en avertissement. Les deux échecs de la première passe
verte ont été tranchés **contre le Java**, pas en réécrivant l'attente au hasard :

1. `researcher.rs` — la passe qui abandonne la marche répondait `NoResearchInProgress` là
   où mon test attendait `WalkAbandoned`. Le code avait raison : `EntityAIWorkResearcher.java:
   84` est une branche indépendante de `:80`, et une passe qui n'a rien à avancer doit dire
   *pourquoi* elle n'avance rien. Le test dit maintenant les deux cas, avec la scène qui
   prouve chacun (aucune recherche → `NoResearchInProgress` ; recherche + mana →
   `WalkAbandoned`).
2. `mod.rs` — 40 actions contre les deux seuils : le test affirmait « sous le seuil du
   chercheur » alors que le seuil du chercheur est **32** (`AbstractEntityAIBasic.java:
   425-428`) et que `:413` compare par `>=`. 40 est donc **au-dessus** de 32 et **sous** les
   50 du professeur (`EntityAIWorkTeacher.java:203`) : l'attente est corrigée dans ce sens,
   ce qui rend le test plus utile (il prouve que le seuil dépend bien du métier).

Trois erreurs de compilation de ma part, toutes corrigées avant la passe verte et toutes
visibles dans l'historique : un `mod tests` déclaré deux fois (fichier + inline, ce dernier
renommé `trade_tests`), et deux imports `Action` inutilisés.

Ce que la vérification ne prouve **pas** : le rythme réel. Les scénarios tournent à la
vitesse de la machine (`TICKS_SECOND` par cible, 4 ticks machine), pas à celle du jeu, parce
que le délai de base appartient à l'autre table (§3 et `MANQUANT.md` §1). C'est dit dans
`tests.rs` en tête de fichier, et c'est le seul écart assumé.

## 7. Ce que ce module ne fait pas

- **Aucun appel au monde.** Pas de déplacement, pas de siège créé, pas de papier déplacé,
  pas de recherche avancée, pas d'expérience écrite : tout sort en `Effect`.
- **Aucun tirage aléatoire.** Les trois tirages sont des faits (`draw`, `pupil_draw`,
  `sit_roll`), sinon un scénario ne se rejoue pas.
- **Aucune API inventée.** Rien n'est importé de `mod_api` ; les manques sont dans
  `MANQUANT.md` avec ce que je voulais faire.
- **Aucun `unwrap()`** : les seules données « venues d'un fichier » sont les identifiants de
  recherche, et `advance_target` répond `Option` ; les formules bornent leurs entrées
  (niveaux négatifs ramenés à 0) au lieu de paniquer.
- **L'élève** (`EntityAIWorkPupil`, `EntityAIStudy`) : hors périmètre, déjà pris par
  `src/ai_work_05` pour l'élève et par personne pour la bibliothèque. Le professeur lit un
  élève comme un fait, ce qui suffit à sa machine.
