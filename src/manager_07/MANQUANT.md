# MANQUANT — ce que ces quatre gestionnaires réclament et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md`. Chaque entrée dit **ce qui
manque, d'où ça vient, ce que ça coûte**, et — quand elle existe — la sortie exacte. Aucun code des
cinq fichiers n'appelle l'hôte : tout est données et règles, testable sans monde.

---

## 1. Une entité n'est qu'un handle : rien ne peut la retirer

**Ce qui manque.** `registerCivilian` retire l'entité qu'il refuse (`VisitorManager.java:81`, `:93`,
`:123`) et remplace l'ancienne quand une vivante arrive (`:113`). `removeCivilian` retire celle du
record supprimé (`:310`). Le port ne rend qu'un [`Outcome`](../src/manager_07/visitors.rs) et un
`Option<Visitor>` : c'est **l'appelant** qui retire.

**D'où ça vient.** `crates/horizon-mod-vocab/src/host/entities.rs` n'expose aucune suppression, et un
mod wasm n'a de toute façon qu'un handle, jamais une référence. La comparaison « même objet » de Java
(`existingCitizen.get() == visitor`, `:106`) est donc portée par le handle que l'appelant fournit.

**Ce que ça coûte.** Un appelant qui ignore l'`Outcome` laisse des entités fantômes qui ne
représentent aucun record. C'est le seul contrat non vérifiable par un type.

**La sortie.** Une primitive `Host::entity_remove(handle)` ou, à défaut, un drapeau
`Discarded` que le lanceur lit. Ce n'est pas un problème de mixin : c'est une primitive absente.

---

## 2. Le monde physique du spawn : point de spawn, chunk chargé, voyage

**Ce qui manque.** `spawnOrCreateCivilian` vérifie `WorldUtil.isEntityBlockLoaded` (`:258`), calcule un
point avec `EntityUtils.getSpawnPoint` (`:260`), crée l'entité (`:263`), la pose avec les deux offsets
(`:271`), et saute le spawn d'un visiteur en voyage
(`CitizenData.java:936-939`).

**D'où ça vient.** Aucune de ces quatre choses n'est une règle : ce sont des appels au niveau.

**Ce que fait le port.** [`spawn`](../src/manager_07/visitors.rs) prend deux fermetures — `loaded` et
`point` — et rend la position calculée, ou un refus nommé. C'est exactement la décision de Java, sans
le monde : l'ordre des candidats (`:245-262`), le saut de l'origine (`:253-256`), et le test
d'annonce (`:286`) sont dedans.

**Ce que ça coûte.** L'offset final (`HALF_A_BLOCK`, `SLIGHTLY_UP`, `:271`) et le voyage ne sont pas
appliqués : c'est à l'appelant de les ajouter et de consulter son gestionnaire de voyage, qui n'existe
pas (`TravellingManager.java` n'est pas porté).

---

## 3. Les paquets : un plan, jamais un envoi

**Ce qui manque.** `sendPackets` (`:171-216`) construit un `ColonyVisitorViewDataMessage` et l'envoie
à chaque abonné. Le formulaire du message existe (`src/network/views.rs:308-345`), mais il n'y a
**aucune liste d'abonnés** : `closeSubscribers` / `newSubscribers` viennent de
`ColonyPackageManager`, qui n'est pas porté.

**Ce que fait le port.** [`Visitors::send_plan`](../src/manager_07/visitors.rs) rend qui reçoit quoi
(`refresh`, ids, joueurs) et **efface les drapeaux comme Java**, y compris quand rien n'est envoyé
(`:187-200` avant le test de vide `:202`).

**Ce que ça coûte.** Aucun client n'est jamais mis à jour. Une session de jeu qui n'appelle pas
`send_plan` garde aussi ses drapeaux sales, ce qui est le comportement inverse — c'est pourquoi la
règle est testée.

**La sortie.** Un hook « un joueur entre/sort d'un chunk » plus `Host::send_to_player`.

---

## 4. Le formulaire réseau des magasins désynchronise, et le port ne peut pas le refuser

**Ce qui manque.** `StandardDataStoreManager.Factory.serialize(controller, buffer)` (`:137-145`) écrit
un compte puis les paires `(token, value)`, et `deserialize` (`:147-165`) lit les deux appels
**dans un seul `try`** : si le second lève, le premier a déjà consommé ses octets et la boucle
continue sur un flux décalé. Java ne rend pas une erreur, il rend un livre faux.

**D'où ça vient.** Ce crate n'a pas de tampon de paquet, et le contenu d'un magasin traverse déjà le
fil comme des octets opaques (`src/network/building_modules.rs:592`).

**Ce que ça coûte.** Rien aujourd'hui (aucun envoi), et un piège le jour où un envoi existera : un
lecteur naïf reproduira la désynchronisation. La forme NBT, elle, est portée et tolérante par entrée
(`Book::read`, `:122-131`).

**La sortie.** Un lecteur qui garde le compte **et** la position, et refuse la paire entière quand
l'un des deux manque — plutôt que de continuer.

---

## 5. `IRecipeStorage` n'est pas modélisé : une recette est un compound et un jeton

**Ce qui manque.** `RecipeStorage` fait 780 lignes (209 pour l'interface) : entrées nettoyées,
sorties secondaires, outils, `equals` sur quatorze champs (`:529-594`), `hashCode` sur neuf
(`:598-614`). Le port ne lit de la recette que ce que le filtre de `read` demande : au moins une
entrée qui nomme un item, une sortie primaire (`id` non-air), un `loot-table`, des sorties
alternatives (`recipes.rs`, `has_input`/`has_output`).

**D'où ça vient.** Un `RecipeStorage` se construit à partir d'une recette de données et d'une grille :
c'est `crate::crafting` (types, ingrédients, résolution) plus un inventaire, pas une structure de
sauvegarde.

**Ce que ça coûte, dit sans détour.** L'identité d'une recette est ici **l'égalité du compound moins
son `Token`**, et c'est plus strict que `equals` en deux endroits : le compte de la sortie primaire
(ignoré par `equals`, `:547`) et la fusion des entrées dupliquées
(`RecipeStorage.java:350-420`). Conséquence : deux recettes que Java tiendrait pour identiques
peuvent être deux recettes ici.

**Le défaut de Java que le port corrige, et son prix.** `hashCode` ignore `recipeSource`, le
`loot-table`, le type et le NBT de la sortie primaire, tous comparés par `equals` — le contrat
`equals`/`hashCode` est cassé, donc la table inverse (`recipes.inverse().get(storage)`, `:93`) peut
**manquer** une recette que `containsValue` trouve. Le port compare une seule identité dans les deux
sens : il trouve ce que Java manquait (`checkOrAddRecipe` ajoutait un doublon que `read` refusait).
Le coût est une différence de comportement sur une sauvegarde qui contient deux telles recettes.

---

## 6. Une requête n'a ni « réouverture » ni liste de livraisons

**Ce qui manque.**
1. `RequestHandler.reassignRequest` (`:288-313`) **retire** l'ancienne affectation, remet l'état à
   `REPORTED`, puis en cherche une autre. `crate::requests::Request::assign` n'a pas de retour depuis
   `Assigned` : rouvrir une requête demanderait un `reopen()` dans `src/requests/`, module d'un autre
   agent.
2. `overruleRequest` écrit la livraison du joueur **sur la requête**
   (`request.overrideCurrentDeliveries(ImmutableList.of(stack))`, `:352`). `Request` n'a pas de liste
   de livraisons, donc le port la garde dans le gestionnaire
   (`Manager::overruled_by`) — un seul endroit, mais pas celui de Java.

**Ce que ça coûte.** `Manager::reassign` ne porte que **l'ordre des écritures** (le drapeau sale est
posé **avant** la tentative, `:258`, contrairement à `assign`, `:232`) : la tentative elle-même est
absente. Et une requête surrégée ne garde l'item du joueur que tant que le gestionnaire vit.

**La sortie.** `Request::reopen()` et un champ `overrides: Vec<Stack>` sur `Request`.

---

## 7. Cinq états sur treize, et quatre suites impossibles

**Ce qui manque.** `RequestState` en a treize (`RequestState.java:13-78`) ; `crate::requests` en garde
cinq et le dit déjà (`src/requests/mod.rs`). La table de suites de `updateRequestState`
(`:304-331`) couvre six cas ; trois existent ici (`FollowUp::{None, Completed, Cancelled}`), et
`RESOLVED` (demande de suites, `:306-309`), `OVERRULED` (`:314-317`) et `RECEIVED` (nettoyage,
`:326-329`) n'ont pas d'état d'accueil.

**Ce que ça coûte.** Pas de requêtes-enfants, pas de surrèglement distinct de l'annulation, pas de
nettoyage après réception. Le port de `Delivered` sur `COMPLETED` est dit dans
`requestmanager.rs` : Java n'a **pas** d'état `DELIVERED`.

**La sortie.** Les treize états avec leur ordinal sauvé, et les trois gestionnaires correspondants.

---

## 8. Un `reset` a besoin de jetons, et un mod wasm n'a pas de source d'aléa

**Ce qui manque.** `setup` (`:124-149`) frappe cinq jetons de magasin et deux de résolveur via
`StandardFactoryController.getNewInstance(TypeConstants.ITOKEN)`, et Java en a besoin à chaque
`reset` (`:413-424`), donc à chaque sauvegarde trop vieille (`:663-669`).

**Ce que fait le port.** [`Tokens`](../src/manager_07/requestmanager.rs) est fourni par l'appelant ;
`Loaded::needs_reset` dit quand.

**Ce que ça coûte.** Un appelant qui ignore `needs_reset` garde des magasins qui ne correspondent plus
à aucune requête : Java les aurait reconstruits. Et `Manager::reset` ne peut pas vider la file de
requêtes (elle ne lui appartient pas), donc un appelant qui réinitialise **doit** vider sa `Queue` —
c'est écrit dans la doc de `reset`.

---

## 9. Les six gestionnaires que le `StandardRequestManager` appelle

**Ce qui manque.** `UpdateHandler`, `TokenHandler`, `ResolverHandler`, `RequestHandler` et
`ProviderHandler` (`:96-108`), plus les six magasins typés. Ils parcourent des résolveurs, des
fournisseurs et des enfants — trois choses que ce crate n'a pas (`src/requests/mod.rs`).

**Ce que le port garde d'eux.** La version (`CURRENT_VERSION = 15`), la table des suites, le balayage
des orphelins (`:504-518`), et les deux ordres d'écriture d'`assign` et `reassign`. C'est la part
décidable sans monde.

**Ce que ça coûte.** Aucune affectation réelle : le port ne choisit pas de résolveur, il enregistre
celui que l'appelant a choisi (`src/requests/resolver.rs` fait le choix).

---

## 10. Le record de visiteur n'a que son `id`

**Ce qui manque.** `VisitorData` sauve `rcost`, `rcostqty` et `sitting` en plus du record citoyen
(`VisitorData.java:53-70`), et `initForNewCivilian` (`:299`) remplit nom, famille et saturation.

**D'où ça vient.** `rcost` est un `ItemStack` (donc `crate::inventory`), `sitting` une position, et le
reste appartient à `crate::persistence::roster`.

**Ce que ça coûte.** Un visiteur relu perd son prix de recrutement et sa position assise : le port ne
sauve que `id`, et `read` recalcule l'identité depuis `(id, colonie, dimension)` comme Java
(`CitizenData.java:2100-2104`). Un serveur Java qui lirait ce fichier trouverait un record sans ces
trois clés ; un fichier Java relu ici les ignore.

**La sortie.** Deux lignes dans `write`/`read` le jour où `ItemStack` et les positions de siège sont
modélisés dans ce module.
