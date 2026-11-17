# Ce que `manager_08` ne porte pas, et ce que ça coûte

Quatre gestionnaires, trois moitiés de monde. Chaque refus est nommé dans le code ;
ce fichier liste ce qui **n'a pas de code du tout**, ce qu'un appelant devra fournir
lui-même, et les trois endroits où le portage s'écarte volontairement du Java.

## 1. `IRequestManager` : le registre que les trois wrappers enveloppent

Java enveloppe une instance de `StandardRequestManager`, qui possède **neuf data
stores** (`StandardRequestManager.java:60-160`), un `FactoryController`, un
`RequestHandler`, un `ResolverHandler` et deux résolveurs attitrés (le joueur et le
retry). Le port n'a que `crate::requests::Queue` : des requêtes, dans l'ordre
d'arrivée. Ne sont donc pas là :

| Ce qui manque | Où | Ce que l'appelant doit fournir |
| --- | --- | --- |
| les **tokens** — un `IToken<?>` opaque par requête, par résolveur, par fournisseur | `api/colony/requestsystem/token/IToken.java:10-30` | `wrapped::Resolver`, qui est la position de la hutte : deux résolveurs à la même position ne peuvent pas être blacklistés séparément |
| le store **résolveur → requêtes**, qui est là où l'affectation vit réellement | `RequestHandler.java:295-308` | rien : c'est là que `reassignRequest` devient impossible, voir §2 |
| le store **type de requête → résolveurs** (`getRequestableTypeRequestResolverAssignmentDataStore`) | `:139-156` | `assign::Offer`, que l'appelant remplit |
| le `ResolverHandler` et son `isBeingRemoved` | `:169` | `Offer::can_resolve`, que l'appelant met à `false` |
| les neuf autres stores (identités, enfants, livraisons, recettes, noms…) | `:60-160` | — |
| `serialize`/`deserialize` sur `FriendlyByteBuf` et `serializeNBT` | `StandardRequestManager.java:150-198` | `crate::persistence`, qui écrit le compound de la colonie |

**Conséquence mesurable** : `AbstractWrappedRequestManager` délègue 25 méthodes et
n'en refuse qu'une. Les 25 délégations ne sont pas réécrites ici (un passe-plat sans
état ne peut pas contredire son socle) ; l'inventaire est le fichier Java, et ce que
l'appelant devra brancher un jour est la colonne de droite du tableau ci-dessus.

## 2. `reassignRequest` : l'opération que ce port ne peut pas exprimer

`WrappedBlacklistAssignmentRequestManager` existe **pour** la réaffectation : la
javadoc de `reassignRequest` (`RequestHandler.java:279-280`) promet de rendre la
requête à son résolveur d'origine si le second essai échoue, et le code ne le fait
pas — il retire l'affectation (`:295-308`), passe la requête en `REPORTED` (`:309`)
et rend `null` si la recherche échoue.

Le port refuse l'opération par son nom (`Refusal::NotPortable`) pour une raison de
structure, pas de temps :

- Java garde l'affectation dans un **store à côté** de la requête, donc la retirer est
  une écriture de carte ;
- `requests::Request` **est** l'affectation (`src/requests/request.rs:201-203`) et son
  `assign` refuse un second résolveur (`:242-245`) ; il n'existe aucune opération qui
  la retire.

Ce qu'il faudrait : une méthode `Request::release_resolver` sur le modèle de
`WorkOrder::release` (`colony/ordre.rs:164-173`), et alors les quatre règles de
`reassignRequest` deviennent portables — refus si la requête a des enfants (`:290-293`),
l'ancien résolveur prévenu par le gestionnaire **statique** (`:296`, `:307`),
`REPORTED` avant le second essai (`:309`), et la restauration que la javadoc promet.
Le test `a_reassignment_is_refused_by_name_and_its_first_check_still_answers` prouve la
prémisse au lieu de la commenter.

## 3. `WorkManager` : les quatre règles qui vivent dans un autre fichier

| Ce qui manque | Où | Pourquoi ce n'est pas ici |
| --- | --- | --- |
| `read` et `write` (le garde `newsystem`, l'enregistrement illisible, l'id dupliqué, la réclamation libérée à la lecture) | `:226-283` | **déjà porté** : `persistence::read_orders` / `written_work` (`src/persistence/mod.rs:429-505`) |
| `getWorkOrder`, `getWorkOrdersOfType`, le compteur d'id, le retrait d'un ordre fini | `:148-188`, `:308-312` | **déjà porté** : `Colony::order`, `Colony::place_order`, `Colony::finish_order`, `Colony::cancel_order` |
| les quatre transitions et le comparateur de priorité | `AbstractWorkOrder.java`, `IWorkOrder.java:19-24` | **déjà porté** : `colony::ordre` |
| le drapeau `changed` **par ordre** (`isDirty`/`resetChange`) | `AbstractWorkOrder.java:547-557`, lu à `WorkManager.java:404-407` | `colony::WorkOrder` n'a pas ce champ : la passe 2 du tick ne peut pas re-salir la colonie pour un ordre modifié. Ajouter le champ, ou passer le drapeau en paramètre |
| les avancements `CREATE_BUILD_REQUEST` et le message `OUT_OF_COLONY` | `:315-331`, `:304` | il faut un joueur : `crate::advancements` a les déclencheurs, pas l'envoi |
| les **coins du blueprint** (`ColonyUtils.calculateCorners`) | `:346-352` | dehors par construction : l'appelant les a (`structures::place::Placing::boxed`) et les passe dans `Site::bounds`. `None` = contrôle sauté, et c'est un trou, pas une règle |
| `WorkOrderMiner` et son exemption de doublon | `:290` | `WorkType` n'a pas de type mineur ; **tous** les ordres du port passent par la comparaison de position |
| les ordres **par classe** (`getUnassignedWorkOrder(Class)`, `getWorkOrdersOfType(Class)`) | `:162-188` | `first_unclaimed` prend un `WorkType`, qui est l'autre axe de Java (`AbstractWorkOrder.java:719`). Un appelant qui veut « le premier ordre de décoration » n'a pas d'équivalent ici |

## 4. Les trois faits d'une hutte que le port ne stocke pas

`work::Hut` porte six champs. Trois viennent de `building::Building` (`kind`, `level`,
`worker`), trois n'existent nulle part dans le port :

- `builder` — `building instanceof AbstractBuildingStructureBuilder` (`:441`). Le port
  le dérive de `Kind::Builder | Kind::Miner`, ce qui est la même chose tant que la
  liste des types reste celle de `building/kind.rs` ;
- `busy` — `hasWorkOrder()` (`:449-452`) : un champ d'`AbstractBuildingStructureBuilder`
  que `building::Building` n'a pas. **Par ordre de grandeur, c'est le seul des trois
  qui change une décision à chaque tick** : un chantier qui ignore qu'il a déjà un
  ordre en prendra un second ;
- `manual` — `getSetting(MODE).getValue().equals(MANUAL_SETTING)` (`:465-469`). Le port
  a la clé (`work::MODE_SETTING`) et la valeur (`work::MANUAL_SETTING`) mais aucun
  magasin de réglages : `building::Module` n'en porte pas.

## 5. Ce que personne n'appelle

`Sink::write` est le seul chemin par lequel ce module annonce une écriture. Rien ici
n'écrit `dirty` dans une colonie (`Write::ManagerDirty` et `Write::ColonyDirty` sont
des entrées de journal) : la colonie du port n'a pas de drapeau, et `crate::wiring` est
le seul endroit qui sache où la colonie vit.
