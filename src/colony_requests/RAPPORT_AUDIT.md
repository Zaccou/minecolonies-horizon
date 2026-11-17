# Audit de fidélité — `src/colony_requests`

**Périmètre.** Les 6 fichiers de ma tranche (`mod.rs`, `token.rs`, `assignment.rs`,
`transition.rs`, `chain.rs`, `bounds.rs`), relus contre le Java de
`minecolonies-src`.

**Note sur le chemin.** La mission indique `core/colony/requests` ; l'arbre réel
est `core/colony/requestsystem/` (65 fichiers) **plus** `api/colony/requestsystem/`
(73 fichiers) — les deux sont cités ci-dessous, et c'est de là que viennent toutes
mes références depuis le début.

**Méthode.** 39 méthodes choisies pour ce qu'elles portent (une borne, un ordre,
un refus, une durée, une formule), lues à la ligne exacte dans le Java, puis
comparées au Rust. Verdicts :

- **fidèle** — même règle, même résultat, y compris sur les cas limites ;
- **divergent** — le Rust et le Java ne disent pas la même chose ;
- **absent** — la règle n'existe pas dans le port (assumé ou à corriger).

## Tableau des 39 méthodes auditées

| # | Méthode Java | ligne Java | Rust | ligne Rust | verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `RequestHandler.assignRequest` (aiguillage de stratégie) | `RequestHandler.java:96-110` | `chain::pick` | `chain.rs:319` | absent (équivalent : `FASTEST_FIRST` retombe sur le même code, `:102-106`) |
| 2 | `RequestHandler.assignRequestDefault` | `RequestHandler.java:121-225` | `chain::order` + `chain::pick` | `chain.rs:290`, `:319` | fidèle |
| 3 | `RequestHandler.resolve` | `RequestHandler.java:235-276` | `transition::advance` + `parent_can_resolve` | `transition.rs:203`, `:296` | fidèle (adoption des enfants absente) |
| 4 | `RequestHandler.reassignRequest` | `RequestHandler.java:287-313` | `transition::may_resolve` + `Assignment::take_all` | `transition.rs:357`, `assignment.rs:178` | partiel — le refus existe, la composition est à l'appelant |
| 5 | `RequestHandler.onRequestCompleted` | `RequestHandler.java:365-396` | `transition::parent_can_resolve` | `transition.rs:296` | fidèle |
| 6 | `RequestHandler.onRequestOverruled` | `RequestHandler.java:403-431` | `Move::Deliver` / `Actor::Player` | `transition.rs:166` | partiel (pas de chaîne de requêtes) |
| 7 | `RequestHandler.processDirectCancellationOf` | `RequestHandler.java:491-523` | `transition::end` | `transition.rs:382` | **divergent → corrigé (C2)** |
| 8 | `RequestHandler.resolveRequest` (3 préconditions) | `RequestHandler.java:531-555` | `transition::may_resolve` | `transition.rs:357` | **absent → corrigé (C1)** |
| 9 | `RequestHandler.isAssigned` | `RequestHandler.java:322-325` | `Assignment::holder_of` | `assignment.rs:143` | fidèle |
| 10 | `RequestHandler.registerRequest` (refus du doublon) | `RequestHandler.java:61-73` | `requests::Queue::push` | hors dossier | absent ici, couvert ailleurs |
| 11 | `RequestHandler.cleanRequestData` | `RequestHandler.java:564-581` | `Assignment::release` | `assignment.rs:113` | partiel (l'identité vit dans `requests::Queue`) |
| 12 | `RequestHandler.removeRequester` | `RequestHandler.java:583-593` | `requests::Queue::remove_for` | hors dossier | absent ici, couvert ailleurs |
| 13 | `ResolverHandler.registerResolver` | `ResolverHandler.java:75-103` | `Assignment::register` | `assignment.rs:71` | divergent accepté (A1) |
| 14 | `ResolverHandler.removeResolver(IToken)` | `ResolverHandler.java:130-139` | `Assignment::release_holder` | `assignment.rs:162` | fidèle |
| 15 | `ResolverHandler.removeResolver(resolver)` | `ResolverHandler.java:150-169` | `Assignment::release_holder` | `assignment.rs:162` | fidèle |
| 16 | `ResolverHandler.removeResolverWithAssignedRequests` | `ResolverHandler.java:217-249` | `Assignment::take_all` | `assignment.rs:178` | fidèle |
| 17 | `ResolverHandler.addRequestToResolver` | `ResolverHandler.java:341-354` | `transition::assign_to` | `transition.rs:328` | **divergent → corrigé (C3)** |
| 18 | `ResolverHandler.removeRequestFromResolver` | `ResolverHandler.java:366-386` | `Assignment::release_from` | `assignment.rs:129` | **absent → corrigé (C4)** |
| 19 | `ResolverHandler.getResolverForRequest` | `ResolverHandler.java:395-407` | `transition::holder_may` | `transition.rs:305` | fidèle |
| 20 | `TokenHandler.generateNewToken` | `TokenHandler.java:32-37` | `Tokens::take` | `token.rs:113` | divergent accepté (A2) |
| 21 | `StandardToken.equals` / `hashCode` | `StandardToken.java:64-85` | `Token` (`Eq`, `Hash` dérivés) | `token.rs:33` | fidèle |
| 22 | `AbstractTokenFactory.serialize` (`Id_MSB`/`Id_LSB`) | `AbstractTokenFactory.java:36-46` | `Token::from_halves`/`msb`/`lsb` | `token.rs:44`, `:69`, `:74` | fidèle |
| 23 | `RequestState.serialize` / `deserialize` (ordinal 0-12) | `RequestState.java:105-118` | `requests::State::as_str` | hors dossier | **divergent hors dossier (N2)** |
| 24 | `IAssignmentDataStore.getAssignmentForValue` | `IAssignmentDataStore.java:31-35` | `Assignment::holder_of` | `assignment.rs:143` | fidèle, plus strict |
| 25 | `StandardRequestSystemBuildingDataStore.moveToSyncCitizen` | `…DataStore.java:105-130` | `Book::move_to_citizen` | `assignment.rs:289` | fidèle |
| 26 | `StandardRetryingRequestResolver.canResolveRequest` | `StandardRetryingRequestResolver.java:92-97` | `Attempt::accepts_more` | `bounds.rs:237` | **citation divergente → corrigée (C5)** |
| 27 | `…getCurrentReassignmentAttempt` | `…:73-77` | — | — | absent (sans objet, voir C5) |
| 28 | `…resolveRequest` (pose le délai) | `…:106-111` | `Attempt::park` | `bounds.rs:192` | fidèle |
| 29 | `…tick` (décrémente, réassigne, se blackliste) | `…:145-195` | `Attempt::tick` + `blocklist` | `bounds.rs:207`, `:253` | **absent en partie → corrigé (C5)** |
| 30 | `…onAssignedRequestBeingCancelled` | `…:121-130` | `Attempt` possédé | `bounds.rs:172` | fidèle par possession |
| 31 | `…getMaximalTries` / `getMaximalDelayBetweenRetriesInTicks` | `…:61-71` | `MAX_TRIES` / `RETRY_DELAY_TICKS` | `bounds.rs:61`, `:64` | fidèle |
| 32 | `AbstractWarehouseRequestResolver.getSuitabilityMetric` | `…WarehouseRequestResolver.java:351-361` | `requests::resolver::metric` | hors dossier | **divergent hors dossier (N1)** |
| 33 | `AbstractWarehouseRequestResolver.canResolveRequest` | `…:71-130` | `requests::resolver::eligible` | hors dossier | partiel hors dossier (somme inter-entrepôts absente) |
| 34 | `isRequestChainValid` | `…:138-154` | — | — | absent (ne détecte aucun cycle : retourne vrai) |
| 35 | `BuildingRequestResolver.canResolveForBuilding` | `BuildingRequestResolver.java:64-94` | `requests::resolver::eligible` | hors dossier | **partiel hors dossier (N3)** |
| 36 | `BuildingRequestResolver.getPriority` | `BuildingRequestResolver.java:40-44` | `Resolver::priority` | `chain.rs:185` | fidèle (`Source::Building` = 200) |
| 37 | `AbstractCraftingRequestResolver.createsCraftingCycle` | `…CraftingRequestResolver.java:226-252` | `chain::creates_cycle` | `chain.rs:405` | fidèle |
| 38 | `AbstractCraftingRequestResolver.getRequestType` | `…:79-83` | `Type::Deliverable` + `Resolver::building` | `chain.rs:66`, `:110`, `:141` | fidèle |
| 39 | `AbstractDeliverymanRequestable.incrementPriorityDueToAging` / `getMaxAgingPriority` | `…:95-101`, `:73-76` | `MAX_AGING_PRIORITY` (+ `requests::Request::raise_priority`) | `bounds.rs:49` | fidèle (`min(14, p+1)`) |

## Divergences réelles corrigées (7)

**C1 — `resolveRequest` : trois préconditions nommées, absentes.** Java refuse
en nommant chaque cas : non assignée (`RequestHandler.java:536-539`), pas
`IN_PROGRESS` (`:541-544`), enfants ouverts (`:546-549`). Le port n'avait que
`NotAssigned` ; un appelant ne pouvait pas distinguer « un enfant le retient » de
« il n'est pas commencé ». Corrigé : `Refusal::WrongState` et
`Refusal::HasChildren` (`mod.rs:91`), plus `transition::may_resolve`
(`transition.rs:357`) qui demande les trois dans l'ordre de Java.
→ `transition::tests::resolving_demands_assigned_in_progress_and_no_open_child`.

**C2 — l'annulation laissait l'assignation en place.** `processDirectCancellationOf`
retire la requête de l'assignation (`:502-509`) **puis** passe `CANCELLED`
(`:517`). Le port avait `advance(Move::End)` et `Assignment::release` séparés :
rien n'empêchait d'annuler une requête en la laissant attribuée à un entrepôt.
Corrigé : `transition::end` (`transition.rs:382`) fait la vérification d'acteur
**avant** d'écrire, retire l'assignation, puis annule — et un refus ne touche à
rien.
→ `ending_drops_the_assignment_before_the_state`, `a_refused_end_leaves_the_book_exactly_as_it_was`.

**C3 — `assign_to` écrivait l'état avant de consulter la carte.** `assignRequest`
documente qu'une requête déjà assignée doit lever (`RequestHandler.java:79-80`),
et une requête n'a qu'un détenteur (`IAssignmentDataStore.java:32-35`). Le port
posait `Assigned` **puis** `bind` : une entrée périmée dans la carte laissait la
requête assignée à personne. Corrigé : la carte est consultée d'abord
(`transition.rs:328`), la réponse est `AlreadyAssigned`, rien n'est écrit.
→ `a_stale_book_entry_does_not_leave_the_request_assigned_to_nobody` (et
`a_second_assignment_changes_neither_the_state_nor_the_book`, dont l'attente est
passée de `IllegalTransition` à `AlreadyAssigned` pour cette raison).

**C4 — `removeRequestFromResolver` : les deux refus absents.** Java lève quand le
résolveur est inconnu (`ResolverHandler.java:369-372`) et quand il ne détient pas
la requête (`:374-377`). Le port n'avait que `release(id)`, forme souple sans
détenteur. Corrigé : `Assignment::release_from` (`assignment.rs:129`).
→ `giving_one_request_up_from_a_named_holder_is_strict`.

**C5 — la borne du résolveur qui réessaie était mal attribuée, et le blacklist
manquait.** Mon commentaire citait `canResolveRequest`
(`StandardRetryingRequestResolver.java:92-97`) comme la borne des trois essais.
C'est faux : `:95` court-circuite sur `current == null`, et `:76` rend `-1` pendant
une réassignation — donc le troisième terme `-1 < 3` est toujours vrai et
**`canResolveRequest` ne refuse jamais**. Ce qui arrête le quatrième essai est
`:159`, l'auto-blacklist `assignedRequests.get(t) < getMaximalTries() ? {} : {id}`.
Corrigé : citation refaite (`bounds.rs:237`), et `Attempt::blocklist`
(`bounds.rs:253`) qui rend exactement cette liste, à passer au `skip` de
`Chain::pick`.
→ `the_resolver_blacklists_itself_only_once_the_budget_is_spent`.

**C6 — `Origin::clamp` citait un `setPriority` qui n'existe pas.** Aucun
`setPriority` dans `api/colony/requestsystem/` : le champ n'est écrit que par le
constructeur (`AbstractDeliverymanRequestable.java:27-30`) et par le vieillissement
(`:100`) ; `WindowHutRequestTaskModule.java:112` ne fait qu'afficher. Le plancher
à zéro de `clamp` est donc le mien, et la doc le dit maintenant
(`bounds.rs:106`), en citant `:46-47` pour le plafond.

**C7 — la doc se contredisait sur `OVERRULED`.** L'en-tête rangeait `OVERRULED`
dans `→ Cancelled` **et**, quinze lignes plus bas, dans `Move::Deliver` par
`Actor::Player`. Java tranche : « forcefully fullfilled by a player »
(`RequestState.java:55-58`), c'est un succès, pas un retrait. Corrigé : la ligne
du tableau ne garde que `CANCELLED`, `FAILED`, `FINALIZING`.

## Divergences réelles que je ne peux pas corriger (hors de mon dossier)

**N1 — la métrique de l'entrepôt sans entrepôt
(`AbstractWarehouseRequestResolver.java:352-359`).** Java rend **la distance
brute** quand le bâtiment entrepôt est `null` :

```java
final int distance = (int) BlockPosUtil.getDistance(...);
if (wareHouse == null) { return distance; }
return Math.max(distance/10, 1) + …getMutableRequestList().size();
```

`src/requests/resolver.rs` applique `(blocks / 10).max(1) + queue` dans tous les
cas. Aucune donnée ne permet de distinguer les deux situations dans le port
(`Candidate` porte soit un inventaire, soit rien), donc c'est peut-être
inobservable — mais la formule conditionnelle n'est pas portée. Correction
nécessaire dans `src/requests/resolver.rs`, que je n'ai pas le droit de toucher.

**N2 — l'état sauvegardé (`RequestState.java:105-118`).** Java sauve l'**index**
dans l'énumération à treize valeurs, avec un commentaire explicite « This should
never be changed ! ». `src/requests/request.rs` sauve un nom. Documenté là-bas,
non recodable ici (le port n'a que cinq états).

**N3 — `canBeResolvedByBuilding` (`BuildingRequestResolver.java:72-75`).** Un
`IDeliverable` peut refuser d'être résolu par le bâtiment qui le demande
(`IDeliverable.java:60-63`, défaut `true`). `requests::resolver::eligible` ne
connaît pas ce drapeau : une requête qui le met à `false` serait servie par le
bâtiment. Correction nécessaire dans `src/requests/resolver.rs`.

## Divergences acceptées, et pourquoi

**A1 — `registerResolver` lève sur un jeton déjà enregistré
(`ResolverHandler.java:78-86`).** `Assignment::register` est idempotent : un
`insert` dans un ensemble n'a pas besoin de jeter, et `Chain::order` déduplique de
toute façon (`:161`). Jeter demanderait une variante de refus qui n'apprendrait
rien à l'appelant.

**A2 — `TokenHandler` tire un UUID au hasard (`:36`).** Le port compte
(`token.rs:113`). Rien dans les règles ne dépend de l'imprévisibilité, seulement
de l'unicité (`RequestHandler.java:64-68`), et un compteur rend un sauvegarde et
les tests reproductibles.

**A3 — `AbstractRequest.setState` écrit sans vérifier (`:137-154`).** C'est
précisément ce que ce module ajoute : la table (`transition.rs:166`) et
`advance` refusent un couple que Java accepterait. Documenté dans l'en-tête de
`transition.rs`.

## Ce que je n'ai pas pu vérifier

1. **Aucune exécution** : la mission interdit de compiler. Les 69 tests du module
   sont écrits, pas lancés ; les verdicts ci-dessus portent sur la lecture, pas
   sur un résultat mesuré.
2. **Rien qui demande une colonie** : les valeurs réelles de `getMinimumCount`,
   les stocks, la distance entre deux bâtiments, et tout ce que
   `IRequestResolver.canResolveRequest` décide en lisant le monde. Mon `Supply`
   est une interface, ses réponses ne sont pas auditées.
3. **L'ordre de `typeIndexList`** (`RequestHandler.java:134`, `:159`) : il vient
   d'un `Set` construit par réflexion, donc le départage entre deux résolveurs de
   même priorité mais de types différents est **non spécifié** dans Java. Je n'ai
   pas cherché à le reproduire (j'ai gardé l'ordre de déclaration, ce qui est
   dit dans l'en-tête de `chain.rs`) ; impossible de prouver la fidélité d'un
   ordre que la source ne fixe pas.
4. **`ProviderHandler` et les fournisseurs de résolveurs** (`:55-63`) : rien dans
   le port ne les modélise, donc leur contrat n'est vérifié que par lecture.
5. **`RequestUtils.requestChainNeedsPlayer`** (`:30-55`) et
   `AbstractRequest.addDelivery` (`:402-414`) : absents du port (affichage /
   liste de livraisons), non couverts ici.
