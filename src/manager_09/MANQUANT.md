# manager_09 — ce qui manque, et ce que ça coûte

Trois gestionnaires portés : `QuestManager`, `DatagenLootTableManager`,
`DebugPlayerManager`. Un quatrième, `ResearchEffectManager`, était déjà porté
(`crate::research::progress::Effects`) et n'est pas dupliqué ici.

Chaque entrée dit **ce que je voulais faire**, **pourquoi je ne l'ai pas fait**,
et **quel appel d'API ou quel module lèverait le blocage**. Rien de tout ça n'est
simulé en code : la règle qui l'exige rend un refus nommé, ou un plan que
l'appelant exécute.

## 1. Le tampon réseau du carnet de quêtes

`QuestManager.serialize(FriendlyByteBuf, boolean)` écrit un booléen puis, s'il
est vrai, le compound entier (`QuestManager.java:304-312`) ; `deserialize`
(`:315-322`) relit les deux. Le `FriendlyByteBuf` est un tampon Netty : un mod
wasm n'en a pas, et le paquet de `ColonyView.java:384` n'a pas de destination
dans ce port (aucun client).

Ce que j'ai fait : `QuestBook::take_frame(has_new_subscribers)` rend **la
décision** — un compound quand le carnet est sale ou qu'un abonné arrive, rien
sinon — et efface le drapeau dans les trois cas, comme Java le fait hors de son
`if` (`:311`). C'est la seule moitié testable sans réseau, et elle l'est.

Ce qui le lèverait : un type `Buf` dans `crates/horizon-mod-vocab` (écriture d'un
compound précédé d'un booléen) plus un `send_to_subscribers` ; le paquet et le
`deserialize` client sont alors deux fonctions de trois lignes.

## 2. Les sept champs de `QuestInstance` et leur compound

`QuestManager` ne lit jamais l'intérieur d'une instance : il écrit
`getValue().serializeNBT()` (`:216`, `:223`), relit l'id (`:257`, `:273`) et
appelle `isValid`, `onDeletion`, `onWorldLoad`. [`quests::Instance`] est donc
**le compound lui-même**, passé tel quel, et rien n'est inventé.

Ce qui manque, c'est la classe qui écrit ce compound : `QuestInstance`
(`core/quests/QuestInstance.java`, 700 lignes) porte `questTemplateID`,
`questGiver`, `questParticipants`, `assignmentStart`, `objectiveProgress`,
`currentObjectiveInstance` — donc des objectifs, des participants et un jour.

Ce qui le lèverait : un module `quests::instance` qui porte
`QuestInstance.java:262-314` (les sept clés `id`, `assignstartquest`, `progress`,
`questgiver`, `questparticps`, `objective`, `player`) et une `isValid` branchée
sur `crate::citizens` et `getDay()`. Le gestionnaire n'a pas à bouger : il
appelle déjà l'acteur.

## 3. La synchronisation et les réponses du mode debug

`DebugEnableMessage` (client) et les deux réponses de chat de
`CommandToggleDebug.java:40-66` manquent : un mod wasm n'a ni client ni chat. Le
`DebugEnablePathfindingMessage` et le `QueryCitizenAIHistoryMessage` se contentent
de **lire** `hasDebugEnabled` (`:54`, `:59`) et cette lecture est portée.

Ce qu'il faudrait : un canal (`src/channel/registry.rs`) et un message serveur →
client pour le premier, une API de chat pour les seconds.

## 4. La table de loot elle-même

`DatagenLootTableManager.getLootTable` rend un `LootTable` vanilla, fabriqué par
`ForgeHooks.loadLootTable` (`:56`) — le codec des tables de butin de 1.20.1, avec
ses `pools`, `entries`, `functions`, `conditions` et `rolls`. Le port n'a aucun
de ces types, et l'analyse qui les consommerait (`LootTableAnalyzer.toDrops`,
468 lignes) est déjà déclarée manquante dans `manager_02/MANQUANT.md` §5.

Ce que `loot::Tables` rend donc : **le document JSON** de la table, avec son
cache, le chemin où il est lu (`data/<namespace>/loot_tables/<path>.json`) et
les trois réponses de Java (succès, illisible, pas un objet). Le cache, lui, est
complet et testé ; c'est le contenu qui manque.

Ce qui le lèverait : un lecteur de tables de butin dans `crates/horizon-core` (ou
un module `loot/table.rs`) puis, pour l'analyse, le port de
`LootTableAnalyzer`.

## 5. Le chargement de `GLOBAL_SERVER_QUESTS`

La table `GLOBAL_SERVER_QUESTS` (`IQuestManager.java:22`) est remplie par
`QuestJsonListener.java:101-111` — un listener de datapack qui vide la table et
y met une entrée par fichier JSON (template + objectifs + récompenses). Le port
la reçoit en argument (`quests::Registry`), ce qui est la bonne forme pour du
wasm (pas d'état global mutable), mais le **lecteur de JSON de quête** n'existe
pas.

Ce qui le lèverait : un module `quests::content` qui lit un
`data/<ns>/quests/*.json` en `Template { max_occurrence, parents }` — les deux
seuls champs que le gestionnaire lit — et laisse le reste (`attemptStart`, les
objectifs) à §2.

## Une prémisse du brief qui ne tient pas

Le brief demande « l'ordre des écritures pour qu'un test prouve qu'aucune n'est
faite avant un refus » pour les quatre gestionnaires. C'est vrai pour
`QuestManager` — la disponibilité est testée avant la validité (`:72-73`) et
`accept` le prouve avec un acteur qui panique — mais **faux pour les deux
autres** :

- `DatagenLootTableManager` n'a aucun refus : `catch (Throwable)` puis
  `LootTable.EMPTY` (`:64-69`). La règle intéressante n'est pas un ordre, c'est
  qu'un échec **n'est pas mis en cache** (`tables.put` est dans le `try`, `:59`,
  la sortie vide après). C'est ce que le test tient.
- `DebugPlayerManager` n'a ni refus ni ordre : un `toggle` écrit toujours. Sa
  règle réelle est l'asymétrie des deux mutateurs — `toggle` oublie l'entrée de
  `trackingMap` (`:43`), `set` non (`:59-69`) — et le `!production ||` de
  `hasDebugEnabled` (`:29`), qui rend tout le monde debug en développement. Les
  trois sont des tests.
