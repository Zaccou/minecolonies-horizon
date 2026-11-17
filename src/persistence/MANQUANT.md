# MANQUANT — ce que la persistance d'une colonie réclame et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md`. Chaque entrée dit **ce qui manque,
d'où ça vient, ce que ça coûte**, et — quand elle existe — la sortie exacte. Aucun code de ce module
n'appelle l'hôte : les cinq fichiers sont des données et des règles, testables sans monde.

**La règle de lecture qui vaut pour tout ce fichier.** `write` puis `read` rend **exactement** ce que
le port modélise (identité, cinq citoyens et leurs besoins, les bâtiments, la recherche, les
permissions). Sur un fichier écrit par Java, l'aller-retour est **lossy** : ce qui n'est pas modélisé
ici est perdu au moment où le port réécrit. C'est le prix des six premiers paragraphes.

---

## 1. `CitizenData` porte une trentaine de clés, le port en lit cinq

**Ce qui manque.** `CitizenData.serializeNBT` (`core/colony/CitizenData.java:1316-1395`) écrit : un
job (`:1339-1343`), une famille (`:1369-1377`), un inventaire (`:1350-1352`), sept options de chat
(`:1358-1366`), un *skill handler* (`:1330`), trois handlers (bonheur, deuil, nourriture, maladie,
`:1345-1348`), le sexe, la texture, le suffixe, le profil vocal, la position, la position de
respawn, le lit, l'enfant, la pause, `jobStatus` (`:1367`).

**D'où ça vient.** Le port a les modèles ailleurs : `crate::jobs` (le métier), `crate::inventory` (les
neuf emplacements), `crate::citizens::Needs` (la satiété, la santé, le sommeil, la maladie, le deuil).
Aucun n'a de `to_compound`, et leur donner le format de `CitizenData` est un travail par module, pas
un travail de persistance.

**Ce que fait le port.** Il écrit `id`, `name`, `saturation`, `asleep`, `justAte`
(`format::KEYS`), et laisse les autres clés au `CitizenData` de Java.

**Ce que ça coûte.** Un fichier Java lu puis réécrit par le port **perd les métiers et les
inventaires** : les citoyens reviennent sans emploi, et les bâtiments qui les employaient sont
vacants (`Repair::WorkerUnassigned` ne se déclenche même pas, puisque le worker n'est plus écrit).
Ne jamais réécrire un fichier Java avec ce module : le lire, oui ; le produire, seulement pour un
monde qui vit dans le port.

**La sortie.** Un `to_compound`/`from_compound` par domaine (job, inventaire, besoins étendus), puis
une ligne par clé dans `CitizenData` recopiée ici.

---

## 2. Aucun fichier à écrire : pas de `std::fs` dans un mod wasm

**Ce qui manque.** Java possède `minecolonies/colonies.dat` et un `colony<id>.dat` par colonie
(`core/colony/ColonyManagerConstants.java:25-37`), avec une copie de secours (`BackUpHelper`).

**D'où ça vient.** Un mod wasm n'a pas de système de fichiers et l'hôte n'expose aucun handle sur la
sauvegarde du monde (`crates/horizon-mod-vocab/src/host/` : niveaux, blocs, entités, joueurs — rien
pour un fichier).

**Ce que fait le port.** `write` rend un `Compound` de la forme exacte de `colony<id>.dat` ; l'écriture
appartient à l'appelant.

**Ce que ça coûte.** Pas de sauvegarde de secours : quand un fichier est refusé (`Refusal`), Java
retombe sur `BackUpHelper` (`core/colony/IColonyManagerCapability.java:279-284`) et le port n'a rien
où retomber. C'est la raison pour laquelle `citizenManager` et `buildingManager` sont **requis** ici
alors que Java les lit vides : un refus nommé vaut mieux qu'une colonie silencieusement vidée qu'on ne
pourra pas restaurer.

**La sortie.** Une primitive « données du monde » (lecture/écriture d'un compound nommé) dans le SDK.

---

## 3. Le nom du propriétaire n'est pas écrit

**Ce qui manque.** `Permissions.savePermissions` écrit `owner` (le **nom** du propriétaire,
`Permissions.java:610-613`) et `ownerid` (`:615-618`). Le port n'écrit que `ownerid`.

**D'où ça vient.** Le port n'a aucune table de noms de joueurs : `Members` ne tient que des `u128`
(`src/colony/permissions.rs:224-228`), et une colonie n'a jamais vu le nom de son fondateur.

**Ce que ça coûte.** À la relecture par Java, un membre n'est gardé que si le cache de profils du
serveur connaît son uuid **ou** si un nom est présent (`Permissions.java:408-417`). Une colonie
écrite par le port et lue par Java perd donc les membres que ce serveur n'a jamais vus.

**La sortie.** Une source de noms (le cache de profils, ou un `name` par membre dans `Members`).

---

## 4. Les rangs personnalisés sont refusés, pas modélisés

**Ce qui manque.** Une colonie peut ajouter ses propres rangs (`Permissions.java:1054-1080`) et
`permissionMap` en porte alors plus de cinq entrées.

**D'où ça vient.** L'échelle du port est un `enum` fermé de cinq (`src/colony/permissions.rs:36-43`),
avec les masques calculés par le *fall-through* de `loadRanks` (`:83-98`).

**Ce que fait le port.** Un membre qui tient un rang hors des cinq est refusé
(`Refusal::UnknownRank { found }`) ; une entrée de `permissionMap` hors des cinq est ignorée.

**Ce que ça coûte.** Un serveur qui a personnalisé ses rangs ne peut pas charger ses colonies dans ce
port. Refuser est un choix : démoter silencieusement un membre change qui peut casser une hutte.

**La sortie.** `Rank` en données (id, nom, masque) au lieu d'un `enum`, et `Members` qui porte la
table.

---

## 5. Un `permissionMap` modifié n'est pas appliqué

**Ce qui manque.** Java lit les drapeaux et les *applique* (`Permissions.loadPermissions:353-376`) :
une colonie qui a retiré `PLACE_HUTS` à ses officiers le garde après un redémarrage.

**D'où ça vient.** Les masques du port sont compilés (`Rank::mask`), et `Members::may` les lit
directement.

**Ce que fait le port.** Il lit les drapeaux, applique l'agrandissement d'ancienne version
(`upgradePermissions`, `:228-262`) et **compare** ; un écart produit `Repair::RanksDiffer { rank }`.

**Ce que ça coûte.** Les permissions modifiées d'une colonie Java sont perdues à la lecture : le port
applique son propre tableau. C'est le seul endroit du module où le port **ne fait pas** ce que Java
fait, et il le dit au lieu de le taire.

**La sortie.** Un masque par rang dans `Members` (même travail que le point 4).

---

## 6. Le fichier d'ordres est celui du port, pas celui de `AbstractWorkOrder`

**Ce qui manque.** Un ordre Java porte `type` (le nom de la classe de l'ordre, une chaîne),
`structurePack`, `structurePath`, `translationKey`, `rotation`, `isMirrored`, `currentLevel`,
`amountOfResources`, `iteratorType`, `cleared`, `requested`, `stage` et une boîte
(`AbstractWorkOrder.java:709-730`).

**D'où ça vient.** Le port n'a ni structure ni schéma à nommer : `crate::colony::ordre` écrit
`{id, priority, workOrderType, targetLevel, building, location, claimedByBuilding}`
(`src/colony/ordre.rs:213-251`) et le dit déjà dans son propre en-tête.

**Ce que ça coûte, dans les deux sens.**

- Java lisant un fichier du port : `AbstractWorkOrder.createFromNBT` lit `type` avec `getString`, le
  cherche dans `nameToClassBiMap` et déréférence le résultat (`:240-248`) — une chaîne vide donne une
  `NullPointerException` qui sort de `WorkManager.read`, est attrapée par `loadColony`
  (`Colony.java:740-744`) et **coûte la colonie entière**. Un fichier écrit par ce port ne doit
  jamais être donné à Java.
- Le port lisant un fichier Java : l'ordre est gardé (id, niveau cible, position, claim sont bons)
  mais `building` est vide, d'où `Repair::OrderWithoutKind { id }`.

**La sortie.** Une table `WorkType` ↔ nom de classe Java et un chemin de structure, c'est-à-dire le
module des structures avant celui-ci.

---

## 7. Le worker d'un bâtiment n'a pas la forme de Java

**Ce qui manque.** `WorkerBuildingModule` écrit `worker` comme une **liste de compounds**
`{workerId, …}` (`:124-135`) et ne réassigne un citoyen que si son métier correspond
(`:129-133`).

**D'où ça vient.** `crate::building::Building` tient un seul `Option<i32>` sous `worker`, en `Int`
(`src/building/instance.rs:60-63`, `:367-369`) — un module n'a pas de niveau, et une hutte n'a qu'un
ouvrier.

**Ce que ça coûte.** Aucune lecture croisée fausse, mais aucune embauche non plus : Java lit un `Int`
comme une liste vide (pas d'ouvrier), le port lit une liste comme un entier absent (pas d'ouvrier).
Le lien métier de Java n'est pas reproduit : le port ne vérifie que « ce citoyen existe ».

**La sortie.** Un `worker: Vec<i32>` par module, et la vérification du métier qui va avec
(`crate::citizens::employment` la porte déjà).

---

## 8. Les managers que le port n'a pas de tranche pour

Écrits par `Colony.write` (`:938-1003`) et **non écrits** ici, chacun pour la même raison — il n'y a
pas de module à qui demander le compound : `visitors` (`:938`), `animalManager` (`:942`), `graveManager`
(`:946`), `eventManager` / `statisticManager` (`:953-954`), `quest_manager` (`:956`),
`event_desc_manager` (`:957`), `raidManager` (`:958`), `waypoints` (`:973`), `freeBlocks` (`:981`),
`freePositions` (`:991`), `abandoned` (`:993`), `requestManager` (`:994`), `pack` (`:995`),
`autoDelete` (`:996`), `teamcolor` (`:997`), `colonyflag` (`:998`), `lastOnlineTime` (`:999`),
`textstyle` / `namestyle` (`:1000-1001`), `colonyday` (`:1002`), le module de réglages de l'hôtel de
ville (`:1004-1006`), `travellingData` (`:1008`), `connectionmanager` (`:1009`).

**Ce que ça coûte.** Deux choses distinctes, qu'il ne faut pas confondre : les **lire** ne manque à
rien de ce que le port modélise ; les **réécrire** les efface d'un fichier Java. `abandoned` et
`autoDelete` sont déjà écrits par `manager::persist` dans le fichier du monde
(`src/manager/persist.rs:198-210`) — c'est le bon endroit pour eux, pas ce module.

**La sortie.** Un `to_compound` par manager, au fur et à mesure que les tranches existent.

---

## 9. Un `nbt` de plus, faute d'un `pub(crate)`

**Ce qui manque.** Un seul module de lecture/écriture de tags pour tout le crate.

**D'où ça vient.** `src/colony/nbt.rs:9-11`, `src/building/nbt.rs:8-13` et `src/research/nbt.rs:8-13`
disent chacun la même chose : ils sont `mod nbt;`, privés à leur dossier, et les rendre `pub(crate)`
est une ligne dans un fichier que cet agent ne possède pas. Ce module apporte la quatrième copie, et
la seule fonction qui n'existait nulle part : `double` / `double_of` (une satiété est un `Double`, et
`Tag::Double` porte ses bits — `crates/horizon-core/src/nbt.rs:44-46`).

**Ce que ça coûte.** Quatre implémentations d'`int` / `text` / `compounds` à garder d'accord.

**La sortie.** `pub(crate) mod nbt;` dans `lib.rs` — une ligne, hors de mon dossier.

---

## 10. Le module n'est branché sur rien

**Ce qui manque.** Un appelant.

**D'où ça vient.** `wiring` ancre aujourd'hui le **record plat** de `crate::colony` dans le block
entity du bâtiment principal (`src/wiring.rs:9-44`, `:152`), et `manager::persist` écrit le même
record plat dans le fichier du monde (`src/manager/persist.rs:196-215`). Ni l'un ni l'autre ne
connaît la forme imbriquée de Java, parce qu'aucun des deux ne porte la recherche ni les permissions
telles que Java les écrit.

**Ce que fait le port.** Rien ne l'appelle : `write` et `read` sont testés et documentés, et c'est
tout. Ce n'est pas un oubli, c'est une frontière : les deux fichiers à modifier ne sont pas dans mon
dossier.

**Les deux lignes qui restent, et à qui elles appartiennent.**

1. `wiring` : remplacer `colony.to_compound()` par `persistence::write(...)` et
   `Colony::from_compound(inner)` par `persistence::read(inner)`. Coût : `wiring` doit tenir un
   `Vec<building::Building>`, un `Needs` par citoyen et un `Progress` — c'est-à-dire être appelé par
   un monde qui les a, ce qui n'est pas encore le cas (`wiring` vit sur un block entity seul).
2. `manager::persist` : écrire la même forme par colonie, pour qu'il n'y ait qu'un format. C'est une
   décision de `manager`, pas de ce module.

**La sortie.** Un propriétaire pour « où vit une colonie » qui tienne aussi la recherche : le module
`colony_world` ressemble à cet endroit.
