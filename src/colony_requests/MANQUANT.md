# Ce qui manque pour finir `src/colony_requests`

Aucune API du SDK Horizon ne manque : tout ce module appelle est déjà déclaré
(`mod_api` n'est même pas importé ici — les fichiers de ce dossier ne lisent ni
n'écrivent de NBT, voir §4). Ce qui manque est **en amont**, dans le modèle de
`src/requests/`, et chaque manque est refusé par un nom au lieu d'être contourné.

## 1. Poser un détenteur qui n'a pas de position

**Ce que je voulais faire.** Assigner une requête au résolveur joueur
(`StandardPlayerRequestResolver`) ou au résolveur qui réessaie
(`StandardRetryingRequestResolver`), comme Java le fait dans
`ResolverHandler.addRequestToResolver` (`:342-354`) : l'entrée dans la carte des
assignations, puis `ASSIGNED`.

**Ce qui bloque.** `requests::Request::assign` prend une position `[i32; 3]`
(`requests/request.rs:235`), et `Request::from_compound` répare « assigné sans
personne » en `Created` (`:356`). Ces deux résolveurs n'ont pas de position : le
joueur n'est pas un bloc, et le résolveur qui réessaie est identifié par
`colonie × -20000` (`StandardRetryingRequestResolver.java:45`).

**Ce qu'il faudrait.** Une porte `Request::assign_to_token(Token)` — l'état passe
à `Assigned`, `assigned_to` reste `None`, et l'identité du détenteur vit dans
`colony_requests::Assignment`, qui est de toute façon la source de vérité de Java
(`IResolverRequestAssignmentDataStore`). Le champ `assigned_to` deviendrait alors
« la position du détenteur, quand il en a une », et non « qui détient ».

**Ce qui est en place en attendant.** `transition::assign_to` écrit l'état et la
carte pour les quatre résolveurs qui ont une position, et rend
`Refusal::NoPosition { holder }` pour les deux autres — sans rien écrire, ni d'un
côté ni de l'autre. Un test couvre ce refus (`a_holder_with_no_position_is_refused_and_writes_nothing`).
Aucune requête n'est donc perdue ni à moitié assignée ; c'est la capacité qui
manque, pas la sûreté.

## 2. Faire passer une requête `Created` en `Delivered` par un joueur

**Ce que je voulais faire.** L'« overrule » de `StandardRequestManager.overruleRequest`
(`:346-356`) : Java ne regarde pas l'état courant, un joueur peut forcer les
marchandises dans une requête que personne n'a encore prise.

**Ce qui bloque.** `Request::deliver` exige `Assigned` ou `InProgress`
(`requests/request.rs:274`). Depuis `Created`, le modèle refuse.

**Ce qu'il faudrait.** Soit `Request::overrule(&mut self, stack: Stack)` qui pose
les livraisons puis l'état final sans passer par la porte de `deliver`, soit
accepter que la table de transitions reste plus stricte que le modèle — ce que
fait ce module aujourd'hui.

**Ce qui est en place.** `transition::allowed(Created, Delivered)` est vide, donc
`advance(..., Move::Deliver, Actor::Player)` rend
`IllegalTransition { from: Created, to: Delivered }` au lieu d'appeler un modèle
qui refuserait plus loin. Le test `a_player_may_force_the_goods_in_but_not_before_someone_holds_it`
cite `:346-356` pour dire que c'est Java qui est plus permissif, pas le port qui
invente.

## 3. Un ordre de types stable (facultatif, et Java n'en a pas)

`RequestHandler.java:159` trie les résolveurs par `typeIndexList.indexOf(...)`,
où `typeIndexList` vient d'un `Set` construit par réflexion (`:134`) : **l'ordre
est non spécifié**, donc le départage entre deux résolveurs de même priorité mais
de types différents est arbitraire dans Java. Le port garde l'ordre de
déclaration, qui est stable et reproductible. Rien à demander à l'API : c'est un
choix, documenté dans l'en-tête de `chain.rs`.

## 4. Pourquoi il n'y a pas de sérialisation ici

La carte des assignations est sauvée par Java
(`StandardRequestResolverRequestAssignmentDataStore`), mais `requests::nbt` est
privé (`requests/mod.rs:40`, et son en-tête explique qu'il duplique déjà
`colony/nbt.rs`). Écrire une seconde lecture/écriture des mêmes identifiants
créerait une deuxième source de vérité pour la même donnée. Ce module reste donc
sans NBT : il opère sur des identifiants, et c'est `requests::Queue` qui sauve
les requêtes.
