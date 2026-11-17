# MANQUANT — ce que les quatre gestionnaires réclament et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md`. Chaque entrée dit **ce qui manque,
d'où ça vient, ce que le port fait à la place, ce que ça coûte**, et — quand elle existe — la sortie
exacte. Les quatre fichiers de `src/manager_04/` n'appellent pas l'hôte : ce sont des tables, des
seuils, des refus nommés et des ordres d'écriture, testables sans monde. Tout ce qui suit est donc
un fait qui doit venir du dehors.

---

## 1. Aucun état de bloc : l'empilement des panneaux ne peut pas être lu

**Ce qui manque.** `findLowestPoint` (`ColonyConnectionManager.java:454-466`) descend tant que le
bloc **sous** le panneau est dans `BlockTags.FENCES` ou est `ModBlocks.blockColonySign`, au plus
`MAX_SIGN_RANGE = 5` fois (`:33`). Et `tick` casse le panneau quand le chemin échoue
(`colony.getWorld().destroyBlock(pos, true)`, `:231`, `:323`).

**D'où ça vient.** Le SDK donne des lectures de bloc (`crates/horizon-mod-vocab/src/host/blocks.rs`)
mais pas de `BlockTags`, et aucune écriture de bloc hors des hooks de pose.

**Ce que fait le port.** `MAX_SIGN_RANGE` est exporté avec sa ligne, et c'est tout : c'est au
appelant de calculer le point bas et de casser le bloc. `attempt_add_node` raisonne donc sur la
position qu'on lui donne, pas sur celle du panneau du bas.

**Ce que ça coûte.** Deux panneaux empilés sur une clôture comptent comme deux positions : le
port peut accepter une liaison que Java refuserait (le point bas est plus loin de 50 blocs).

**La sortie.** Une primitive « tag de bloc » et une primitive « casse ce bloc », ou un hook de pose
qui donne le point bas.

---

## 2. Aucun pathfinder : les connexions sont décidées, pas marchées

**Ce qui manque.** `createSignPath` (`:439-447`) démarre un `PathJobSignConnection` sur l'executor
global, et `tick` lit son résultat (`isDone`, `isPathReachingDestination`, `:211-220`). Toute la
machine de `tick` est un appel de retour de ce job.

**D'où ça vient.** Un mod wasm n'a pas de pathfinder (`crates/horizon-mod-vocab/src/host/` n'en
déclare aucun) ; il n'y a pas non plus de thread pour l'attendre.

**Ce que fait le port.** `Connections::resolve(at, reached)` prend le **seul bit** dont la décision
dépend, et rend ce qu'il reste à faire : `Refused`, `Waiting`, `Failed`, `Linked` avec la réparation
à programmer, la colonie à prévenir et le lien à établir.

**Ce que ça coûte.** Rien de la table de décision ; tout le temps de marche. Un serveur Java met
plusieurs ticks à valider un panneau, le port valide quand on lui dit que le chemin est arrivé.

**La sortie.** Un chemin asynchrone dans l'hôte, ou un job de chemin exposé par le SDK.

---

## 3. Aucun registre de colonies : les écritures à deux sont des arguments

**Ce qui manque.** Sept endroits écrivent dans **l'autre** colonie :
`getColonyByDimension` (`:264`, `:343`, `:477`, `:564`, `:726`), puis
`connectedColony.getConnectionManager().getDirectlyConnectedColonies().put(...)` (`:267`, `:397`,
`:567`, `:758`, `:781`).

**D'où ça vient.** `crate::manager::Manager` tient les colonies d'un serveur, mais `manager_04` ne
l'appelle pas : ce serait une dépendance de valeur à valeur qui rendrait les deux modules
intestables séparément.

**Ce que fait le port.** `connect(Some(&mut autre), …)`, `notice(event, Some(&mut autre))`, et
`Resolved::Linked { reciprocal, .. }` / `remove_gate_house` qui rendent les écritures distantes en
**données** (`Reciprocal { colony, link }`). `None` est le `null` de Java.

**Ce que ça coûte.** L'appelant doit résoudre l'id en gestionnaire. En échange, chaque refus peut
être testé avec deux objets explicites, et l'ordre des écritures est visible.

**La sortie.** Rien à demander au SDK : c'est un choix de port. Un `Peer` mutable suffirait si un
jour un registre unique existait.

---

## 4. Aucun message au joueur : les quatorze clés des quatre managers

**Ce qui manque.** Dix `COM_MINECOLONIES_*` dans le gestionnaire de connexions
(`SIGN_TOO_FAR :111`, `CONNECTION_PATH_PENDING :121`, `SIGN_MISSING_LINK :152`,
`SIGN_DISRUPTED :177`, `:183`, `:555`, `CONNECTION_PATH_FAILURE :232`, `:325`,
`SIGN_CONNECTED :240`, `CONNECTION_NO_COLONY :346`, `CONNECTION_BROKEN :365`,
`CONNECTION_FAIL :373`, `:391`, `CONNECTION_SUCCESS :426`), et quatre dans celui des citoyens
(`WARNING_COLONY_NO_ARRIVAL_SPACE :261`, `WARNING_MAX_CITIZENS_RESEARCH :285`,
`WARNING_MAX_CITIZENS_CONFIG :289`, `ALL_CITIZENS_ARE_SLEEPING :684`).

**D'où ça vient.** `MessageUtils.format(...).sendTo(colony).forManagers()` n'a pas d'équivalent :
un mod wasm n'a pas de `Component`, et les clés vivent dans `assets/minecolonies/lang/`, que ce
module ne possède pas.

**Ce que fait le port.** Chaque refus et chaque fin de chemin **nomme** ce qu'il aurait dit
(`Refusal::TooFar`, `Resolved::Refused`, `Resolved::Failed`, `Rest::Announce`) et laisse le
gestionnaire appelant décider.

**Ce que ça coûte.** Rien de la règle ; le joueur ne lit rien. Une quête qui compterait les
messages compterait zéro.

**La sortie.** Un envoi de message par joueur (règle 21 de `SOUL.md` : tout service passe par le
portail ; ici c'est le mod qui parle, pas un service).

---

## 5. Aucun drapeau « sale » sur un bloc : le panneau ne se sauve pas de lui-même

**Ce qui manque.** `addEventDescription` marque le tribunal sale, ou la colonie s'il n'y en a pas
(`EventDescriptionManager.java:61-68`) ; tout le gestionnaire de colis vit de ces drapeaux.

**D'où ça vient.** Un `BlockEntity` de mod wasm garde son compound, mais il n'a pas de `markDirty`
exposé : la sauvegarde est celle que l'hôte décide.

**Ce que fait le port.** `Events::push` rend la description éjectée et rien d'autre ;
`Flags` / `Subscribers::settle` modélisent les sept drapeaux en données.

**Ce que ça coûte.** Un `push` suivi d'un crash perd l'événement. Le port ne peut pas promettre
mieux sans un jour de sauvegarde.

---

## 6. Aucun destinataire de message : `computeNews` s'arrête à la phrase

**Ce qui manque.** `colony.getImportantMessageEntityPlayers()` (`:147`) — les joueurs en ligne qui
ont une permission globale — puis `MessageUtils`.

**Ce que fait le port.** `Events::news()` rend `News { entries }`, plus les trois constantes de la
phrase (`SUMMARY_PREFIX_KEY`, `SUMMARY_SEPARATOR`, `SUMMARY_TERMINATOR`).

**Ce que ça coûte.** La phrase n'est jamais prononcée. Le tri et le comptage, eux, sont exacts.

---

## 7. Le registre des descriptions est fermé à neuf types

**Ce qui manque.** `ColonyEventDescriptionTypeRegistryEntry` (`:16-60`) et
`ModColonyEventDescriptionTypeInitializer.java:27-35` : un mod peut enregistrer **son** type, et
`deserializeNBT` le retrouve par `ResourceLocation` (`:89-98`).

**Ce que fait le port.** `events::Kind` est une énumération fermée de neuf variantes avec
`from_path`. Un type inconnu est un `Skipped::UnknownType`, exactement ce que Java journalise puis
ignore (`:92-96`).

**Ce que ça coûte.** Un mod tiers qui écrit un événement le perd au rechargement. C'est une
limite assumée : un registre ouvert dans un crate sans `std` demanderait une table d'enregistrement
globale, que le SDK ne fournit pas.

**La sortie.** Un registre de types de mod dans le SDK (le même besoin que les recettes).

---

## 8. Aucune entité : `registerCivilian` et `spawnCitizenOnPosition` s'arrêtent au refus

**Ce qui manque.** Les quatre `entity.remove(RemovalReason.DISCARDED)` de `registerCivilian`
(`:123-160`), les deux `getEntity(UUID)` de `spawnCitizenOnPosition` (`:296-308`), la création de
`EntityCitizen` et ses quatre setters (`:310-322`).

**D'où ça vient.** Le seul hook d'entité du SDK est un tick global
(`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19`), et la création d'entité passe par la
commande `summon` (`src/citizen.rs` de ce port le fait déjà pour le citoyen unique de la tranche 1).

**Ce que fait le port.** `Manager::register(id, uuid)` rend `Admission::Attached` ou
`Discarded(Why::{Unknown, UuidMismatch, AlreadyAttached})` — les trois branches, à la même place
dans l'ordre de Java ; `Effect::Spawn` et `spawn_point` donnent ce qu'il faut pour créer l'entité.

**Ce que ça coûte.** L'appelant doit faire le `remove` et le `summon` lui-même. La règle « un
citoyen, une entité » est testée ; le cadavre qui traîne ne l'est pas.

---

## 9. Aucun bus, aucune quête : `CitizenAddedModEvent` et `tutorial/welcome`

**Ce qui manque.** `IMinecoloniesAPI.getInstance().getEventBus().post(new CitizenAddedModEvent(…))`
(`:373`, `:632`) et l'injection de la quête du premier citoyen (`:611`).

**Où sont les deux moitiés.** Le bus existe (`crate::events`), la quête aussi (`crate::ui_window_14`
et le gestionnaire de quêtes du port). Le module ne les appelle pas : ils ne sont pas de son
domaine, et `Effect::Spawn { first: true }` dit précisément quand l'appelant doit le faire.

---

## 10. Aucune famille, aucun logement : le deuil prend un ensemble

**Ce qui manque.** `citizen.isRelatedTo(data) || citizen.doesLiveWith(data)` (`:646`), c'est-à-dire
l'arbre familial et le logement des `CitizenData`.

**Ce que fait le port.** `Manager::mourn(deceased, mourn, related: &BTreeSet<i32>)` : l'appelant
dit qui est parent ou colocataire, le port applique la seule règle du gestionnaire — ni garde, ni
croque-mort (`mourns`), et **tout le monde** est prévenu (`notified`, Java `:655` dans les deux
branches).

**Ce que ça coûte.** Rien de la table ; le calcul de `related` est ailleurs
(`crate::citizens::happiness`, qui ne le porte pas encore).

---

## 11. Aucun gestionnaire de travail : `clearWorkForCitizen`

**Ce qui manque.** `colony.getWorkManager().clearWorkForCitizen(citizen)` (`:396`) et les
répercussions de `removeCivilian` sur le module d'assignation de chaque bâtiment (`:388-394`).

**Ce que fait le port.** `Departure { released: Vec<[i32;3]>, cleared_work: i32 }` : les positions
des bâtiments qui employaient le citoyen (`Building::worker`, testé) et l'id dont les ordres sont à
effacer. Le refus vient **avant** toute écriture : `remove` sur un id inconnu ne touche rien.

**Ce que ça coûte.** Un ordre de travail peut rester accroché à un citoyen mort jusqu'à ce que
l'appelant fasse le nettoyage — Java le fait dans la foulée.

---

## 12. Les cinq familles que le gestionnaire de colis appelle sont d'autres gestionnaires

**Ce qui manque.** `visitorManager.sendPackets`, `serverBuildingManager.sendPackets`,
`animalManager.sendPackets`, `researchManager.sendPackets` (`:183-186`, dans l'ordre), et leurs
`clearDirty` (`:196-198`).

**Ce que fait le port.** `SendPlan::gate` dit si la passe est **dans** le `if` de Java, et
`Flags::{buildings, visitors, animals, research}` portent leurs drapeaux, que `settle` remet à zéro
comme Java le fait aveuglément. Les quatre familles de charges utiles, elles, sont déjà
`crate::network` (`views`, `permissions`, `work`, `citizens`) : c'est la liste des destinataires qui
manquait, et elle est là.

**Ce que ça coûte.** Rien : l'appelant passe le même `Recipients` à ses propres gestionnaires.

---

## 13. Le gestionnaire de colis ne garde pas la dernière heure de contact

**Ce qui manque.** `lastContactInHours` / `setLastContactInHours` (`:60`, `:78-87`), et le champ
`abandoned` du fichier de colonie (`Colony.java:828`, `:993`).

**Où c'est déjà porté.** `crate::manager::lifecycle::Attendance` — avec sa constante
`UPDATE_STATE_INTERVAL`, `TICKS_HOUR`, et le défaut qu'un compteur négatif devient zéro. Ce module
ne le réécrit pas ; `Subscribers::watched()` fournit le seul bit que `Attendance::update` consomme
(`importantColonyPlayers` non vide, `:144`).
