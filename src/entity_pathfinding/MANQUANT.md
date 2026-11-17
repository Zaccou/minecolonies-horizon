# MANQUANT — `src/entity_pathfinding`

Ce que ce module voudrait faire et que le SDK ne permet pas — ou n'aide pas à faire. Chaque entrée
dit **ce que je voulais faire** (avec la ligne Java), **ce que le SDK offre réellement** (avec la
déclaration), **ce que j'ai fait à la place**, et **ce que ça coûte**.

Déclarations citées : `.horizon/sdk/api/crates/horizon-mod-vocab/src/host/`, SDK épinglé
`4dc6455d` (`Cargo.toml`). Le module n'importe pas `mod_api` du tout : il porte des règles et des
valeurs, l'appelant fournit le monde.

---

## 1. Suivre un chemin, tick par tick — `MinecoloniesAdvancedPathNavigate`

**Voulu.** Java a 1 387 lignes de navigateur : il avance l'entité d'un nœud à l'autre, garde l'index
du nœud courant (`getNextNodeIndex`), met en pause (`setPauseTicks`), recompute derrière le dos du
moteur (`recalc`), et lit sa vitesse par attribut.

**Ce que le SDK offre.** `mob_move_to_path(mob, path, speed) -> bool`
(`host/mobs.rs:319-344`) : on lui donne un chemin fini et il le fait suivre. Plus `mob_move_to`
(`:293`) et `mob_stop` (`:344`). **Aucun handle de navigation** : rien ne rend l'index courant, rien
ne met en pause, rien ne fait recomputer.

**Fait à la place.** Le contrat est « un mod fournit un chemin fini, le moteur le fait suivre »
(`arismoko/horizon#4443`), donc il n'y a rien à piloter. Le `PathFacts` de `stuck::Tick`
(`index`, `len`, `target`, `previous`, `end`, `done`) est **fourni par l'appelant**, qui est le seul
à savoir où en est le chemin qu'il a tendu.

**Coût.** Un corps qui se coince contre un mur ne peut pas « replanifier en silence » comme Java : le
module nomme l'action (`Action::ClearPath`) et c'est l'appelant qui rappelle `plan` puis
`mob_move_to_path`.

---

## 2. Les sept prix et les cinq permissions que la recherche ne peut pas dépenser

**Voulu.** `computeCost` (`AbstractPathJob.java:1018-1121`) facture, en plus des douze prix de base :
le chemin de colonie (`onPathCost`, `:1037-1040`), le rail (`onRailCost`, `:1042-1045`), la sortie de
rail (`railsExitCost`, `:1088-1091`), l'air de grotte (`caveAirCost`, `:1051-1054`), la marche dans
une forme (`walkInShapesCost`, `:1086-1087`), le plongeon (`divingCost`, `:1112-1115`) et le sens
contraire d'un bloc directionnel (`badDirectionCost`, `:1068-1077`, `:1131-1147`). Et il consulte
`canUseRails`, `canOpenDoors`, `canClimbAdvanced`, `canPassDanger`, `walkUnderWater`
(`PathingOptions.java:88-120`).

**Ce que le SDK offre.** Rien qui change la question : `Costs::step` (première version) prend
`(from, to, dy, swim_start)` — pas l'état du bloc, pas le bloc du dessous, pas le parent.

**Fait à la place.** `job::Options` porte **les 24 champs** de `PathingOptions`, valeurs Java exactes,
et `Options::costs()` rétrécit aux douze que la recherche honore. Les sept autres sont donc
**conservés et visibles** (un job de route a bien `jump = 30`), simplement pas encore dépensés. Le
test `parity::the_first_version_never_charges_for_walking_inside_a_shape` mesure la perte.

**Coût.** Un citoyen qui marche sur une route de colonie est facturé plein tarif, et un plongeur paie
`swimCost` au lieu de `swimCost + divingCost`. Pour brancher les sept, il faut élargir
`Costs::step` à un état de bloc (ce qui est un changement de la première version, pas d'API).

---

## 3. Les secours du `PathingStuckHandler` qui écrivent dans le monde

**Voulu.** Échelle de secours (`navigation/PathingStuckHandler.java`) : poser une échelle
(`:394-399`), un pont de feuilles (`:400-403`), casser le bloc devant (`:406-410`), téléporter sur
l'objectif (`:281-288`), téléporter le long du chemin (`:382-389`), blesser l'entité (`:290-293`).

**Ce que le SDK offre — les verbes existent.**
`level_set_block(level, pos, state, flags)` (`host/levels.rs:902`),
`level_remove_block(level, pos, moved)` (`:916`),
`block_state_id("minecraft:ladder[facing=north]")` (`host/block_states.rs:25`),
`entity_teleport_to(entity, pos, ...)` (`host/entities.rs:634`).

**Ce qui manque, c'est le contexte, pas le verbe.** Poser une échelle demande : la **face** (Java
`placeLadders` lit la direction du chemin suivi et les murs voisins, donc un nœud parent que le SDK
ne rend pas), l'emplacement exact relativement aux pieds, et sa **démontage** plus tard
(`removeLadder`). Le pont de feuilles demande une forme de pont (3 blocs, `:455-500`) et une
politique de nettoyage. Le « casser devant » demande `IBuilderUndestroyable`
(`PathingStuckHandler.java:453-458`) et une portée de casse (config). Le téléport sur l'objectif
demande `BlockPosUtil.findAround` (spirale de 10 × 10 filtres de surface, `:284-288`), qui est
exprimable mais appartient au navigateur. La blessure demande `damage` et un `DamageSource` nommé.

**Fait à la place.** Chaque cran est **nommé** : `Action::WorldWrite(Unstuck::PlaceLadder |
BuildLeafBridge | BreakBlocks | TeleportAlongPath)` au milieu de l'échelle, et
`Action::Complete { teleport, damage }` pour le dernier recours. Rien n'est sauté en silence : le
module dit quel cran il franchirait, l'appelant décide s'il peut.

**Coût.** Un serveur sans citoyen « casseur » verra l'échelle s'arrêter au cran 3 et le dernier
recours rendre `teleport: false` ; c'est le comportement Java avec les mêmes options éteintes
(`:83-118`), donc rien n'est perdu par défaut.

---

## 4. La géométrie fractionnaire de `prepareStart`

**Voulu.** `PathfindingUtils.prepareStart` (`:87-197`) utilise la position exacte de l'entité :
la tolérance de `0.1` entre le haut du bloc et `entity.getY()` (`:119`), la boîte englobante
(`minX..maxX`, `:106-128`), la position relative au centre de la forme pour pousser hors d'une
clôture (`:180-193`), et le test « suis-je dans cette partie de la forme » (`:138-148`).

**Ce que le SDK offre.** `entity_size` (`host/entity_size.rs`) donne largeur et hauteur, pas la
position, pas la boîte, et **aucune lecture de forme de collision** : `level_block_state` rend un
état, pas un `VoxelShape`.

**Fait à la place.** `start::Start` reçoit `feet_y: f32`, `area: Vec<Pos>` (les colonnes de la boîte,
énumérées par l'appelant) et `push: (f32, f32)` (l'écart au centre de sa cellule). Le test « dans la
forme » se replie sur « la cellule est solide » (`:133`), ce qui est exact pour une clôture pleine et
approximatif pour une dalle.

**Coût.** Un corps à cheval sur deux dalles peut être relevé d'un bloc de trop ; Java, lui, regarde la
partie de la forme où il se trouve. Le repli est documenté au point d'appel, et l'appelant qui veut
la précision peut fournir un `StartWorld` qui répond `shape_top` cellule par cellule.

---

## 5. Lire le monde sans le copier — `ChunkCache`, `CachingBlockLookup`

**Voulu.** Java photographie les chunks autour de la recherche (`world/ChunkCache.java`, 311 lignes)
et cache les derniers blocs lus (`CachingBlockLookup.java`, 186) parce qu'un A\* touche des milliers
de positions.

**Ce que le SDK offre.** `level_block_state(level, pos) -> Option<BlockState>`
(`host/levels.rs:304`) : une lecture, un aller-retour, sans copie ni cache.

**Fait à la place.** Le module ne lit **jamais** le monde : tout passe par un `WorldView` que
l'appelant implémente (`crate::pathfinding::WorldView`, étendu ici par `GoalFacts`, `StartWorld`).
Une grille de test est un monde complet ; un vrai monde est ce que l'appelant décide — copie des
chunks, ou lecture vive sur son propre thread.

**Coût.** Une recherche de 8 000 nœuds sur un `WorldView` qui appelle `level_block_state` à chaque
question fait autant d'allers-retours. La copie est le travail de l'appelant ; `ChunkCache` reste à
porter si ce coût se mesure.

---

## 6. Aucun tirage aléatoire

**Voulu.** `AbstractPathJob` jitte chaque pas (`randomnessFactor = 0.1`, `PathingOptions.java:83`,
`AbstractPathJob.java:1032-1035`), `PathJobWalkRandomEdge` tire un nœud sur dix (`:262`),
`PathJobMoveToLocation` tire `nextInt(100)` pour étendre son plafond (`:138`), et le
`PathingStuckHandler` tire quatre fois par cran (`:307`, `:318`, `:366`, `:398`).

**Ce que le SDK offre.** Aucune source aléatoire dans `host/` (aucune méthode ne prend ni ne rend de
`RandomSource`).

**Fait à la place.** Chaque tirage est un **argument** : `Job::RandomEdge.on_edge` (le nœud a-t-il
gagné son tirage), `StopPolicy::decide(..., roll)` (`0..100`), et les quatre champs de `stuck::Tick`
(`away_roll`, `level_roll`, `away_range`, `ladder_coin`). La première version a fait le même choix
pour le jitter et le dit (`Costs::step`, « the port is deterministic on purpose »).

**Coût.** Deux recherches sur le même monde rendent le même chemin — ce qui est voulu ici, et ce qui
rend chaque règle testable avec une valeur, pas une plage.

---

## 7. Rails et nœuds de coin

**Voulu.** `canUseRails` (`PathingOptions.java:88`) et la génération de nœuds de coin
(`AbstractPathJob.java:786-840`).

**Ce que le SDK offre.** Aucune description de forme de rail (`BaseRailBlock` et son `RailShape`) :
un `BlockState` est une clé plus des propriétés textuelles, mais le module n'a pas de table de
correspondance rail → direction.

**Fait à la place.** Pas de rails, et pas de nœuds de coin : une chute est un pas horizontal suivi
d'une descente dans la même colonne (`search::landing` + `fall`), ce qui donne la même suite de
positions qu'un nœud de coin Java, sans le nœud intermédiaire. `Job::Pathway` garde `rails` à `false`
comme Java par défaut.

**Coût.** Un citoyen ne prendra jamais un rail pour rentrer plus vite ; il marchera la route.

---

## 8. La recherche imbriquée de `PathJobFindWater`

**Voulu.** `PathJobFindWater.isAtDestination` lance un **second** `AbstractPathJob`
(`PathJobFindFishingPos`, `:251-306`) depuis son test d'objectif, pour vérifier qu'une rive est
atteignable depuis le point d'eau.

**Ce que le SDK offre.** Rien qui permette un test d'objectif qui recalcule : le module n'a pas de
recherche réentrante (les tests d'objectif sont des prédicats, et `plan` n'est pas réentrant dans un
`WorldView` emprunté).

**Fait à la place.** `GoalFacts::has_fishing_shore(pond) -> bool`, avec `false` par défaut. Un
appelant qui veut la règle exacte rappelle `plan` sur le point d'eau et répond ; le module ne
l'invente pas.

**Coût.** Sans cette réponse, la pêche ne trouve aucun point d'eau (le test d'objectif refuse tout) —
ce qui est un refus visible, pas un chemin faux.
