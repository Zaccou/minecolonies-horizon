# MANQUANT — ce que le gestionnaire de colonies réclame et que le port n'a pas

Écrit à la main pour ce module, comme `src/structures/MANQUANT.md`. Chaque entrée dit **ce qui
manque, d'où ça vient, ce que ça coûte**, et — quand elle existe — la sortie exacte. Aucun code de
ce module n'appelle l'hôte : les cinq fichiers sont des données et des règles, testables sans
monde. Tout ce qui suit est donc un fait qui doit venir du dehors.

---

## 1. La capacité de chunk n'existe pas dans le SDK — **la prémisse du module**

**Ce qui manque.** En Java l'appartenance d'un chunk est une *capacité attachée au `LevelChunk`* :
`IColonyTagCapability.Impl` porte `colonies`, `owningColony` et `claimingBuildings`
(`api/colony/IColonyTagCapability.java:110-125`), le chunk la sauve avec lui (`:295-310`) et la
rejoue au chargement (`:312-327`). `ColonyUtils.getOwningColony` la lit pour répondre « qui possède
ce bloc » (`api/util/ColonyUtils.java:170-175`).

**D'où ça vient.** Le handle de chunk du SDK ne porte que des lectures
(`crates/horizon-mod-vocab/src/host/levels.rs:13-17` : `chunk`, `chunk_forced`, `chunk_status`,
`chunk_watchers`…), et le NBT d'un chunk n'est exposé nulle part. Un mod wasm ne peut rien y
accrocher.

**Ce que fait le port.** `claims::ClaimBook` tient les trois mêmes champs, indexés par dimension +
chunk, et `persist` les sauve à côté des colonies sous la clé **`chunkClaims`**, qui est une clé du
port (Java n'en a pas besoin). Coût honnête : **un chunk revendiqué par une version qui ne
sauvegardait pas les claims revient sans claim**. C'est pour ça que `Manager::owns_chunk` retombe
sur la boîte de la colonie (`founding::Colonies::owns_chunk`) quand un chunk n'est revendiqué par
personne : une colonie ne tombe jamais en dessous de sa boîte de fondation. Java, lui, ne connaît
que le claim, donc le port est **plus strict** qu'un serveur Java dans ce cas précis (il refuse une
fondation à côté d'une colonie dont les claims manquent, Java l'autoriserait).

**La sortie.** Une capacité de données par chunk dans le SDK (écriture + lecture NBT), ou une
primitive « données d'un chunk ». Rien de moins ne ramène le comportement exact de Java.

---

## 2. La moitié client de `ColonyManager` n'a pas de sens ici

**Ce qui manque.** `ColonyManager` fait 875 lignes dont la moitié est du client : le cache
`colonyViews` (`:64`), `handleColonyViewMessage` et ses huit frères (`:700-822`),
`getColonyView`/`getClosestColonyView` (`:422-478`), le `SoundManager` (`:89`, `:624-629`),
`isSchematicDownloaded` (`:824-834`). `getColonyByPosFromDim` (`:304-308`) passe par
`ServerLifecycleHooks` pour résoudre un niveau.

**Ce que ça coûte.** Rien pour la question « qui possède cette position » : les vues sont un cache
d'envoi de paquets, pas une règle. Ce qui manque vraiment, c'est **l'abonnement** : `closeSubscribers`
et `newSubscribers` (`ColonyPackageManager.java:36-56`) et l'envoi `ColonyViewMessage`,
`ColonyViewRemoveMessage`, `UpdateChunkCapabilityMessage`. Les formes de ces messages existent dans
`crate::network` (founding.rs, views.rs) ; il n'y a pas de liste d'abonnés à qui les envoyer, donc la
suppression d'une colonie ne peut pas prévenir les joueurs (`ColonyManager.java:212-213`).

**La sortie.** Un hook « un joueur entre dans un chunk » plus une primitive d'envoi par joueur ;
alors `Dissolution` pourrait porter la liste des destinataires.

---

## 3. `Permissions.setOwnerAbandoned` n'existe pas dans `crate::colony::permissions`

**Ce qui manque.** `Permissions.java:518-526` : l'abandon d'une colonie, c'est le propriétaire
remplacé par un uuid aléatoire nommé `[abandoned]`. `crate::colony::permissions::Members` sait
poser un rang, expulser et interroger, mais n'a pas cette opération.

**D'où ça vient.** L'abandon est **deux choses distinctes en Java** : ce remplacement de propriétaire
et le compteur d'heures `abandoned` (`Colony.java:993`). Le second est porté ici
(`lifecycle::Attendance`), le premier appartient à `permissions`, module d'un autre agent.

**Ce que ça coûte.** `/colony setAbandoned` (`CommandSetAbandoned.java:35-56`) ne peut pas être
porté, et une colonie abandonnée garde son propriétaire — donc son propriétaire peut encore
`ManageHuts`. La sortie est une méthode `Members::set_owner_abandoned(&mut self, rng: u128)`, mais
l'uuid aléatoire demande une source d'aléa que le module `colony` n'importe pas non plus.

---

## 4. Il n'y a pas de fichier de sauvegarde — donc pas de `.deleted` ni de backup

**Ce qui manque.** Le gestionnaire Java possède `minecolonies/colonies.dat`, un `colony<id>.dat` par
colonie, et un `colony<id>.dat.deleted` à la suppression
(`ColonyManagerConstants.java:25-37`, `BackUpHelper.java:357-372`). `onWorldLoad` rejoue les backups
manquants (`ColonyManager.java:656-662`).

**D'où ça vient.** Pas de `std::fs` dans un mod wasm, et l'hôte n'expose aucun handle de sauvegarde
de monde.

**Ce que fait le port.** `persist::write` produit le **compound** et le laisse à l'appelant ;
`Dissolution::tombstone` produit l'enregistrement à laisser sur place (un compound dont `id` vaut
`0`, que `crate::colony` relit comme « aucune colonie »). `merge_data` ne sait pas retirer une clé
(`crates/horizon-mod-vocab/src/host/block_entities.rs:107-120`), d'où une valeur plutôt qu'une
absence.

**Ce que ça coûte.** Aucune sauvegarde de secours : une colonie dont le porteur de record est détruit
est perdue, alors que Java la retrouve dans son zip. Et **aucun raccordement n'est encore fait** :
`crate::wiring` (section *Where a colony lives*) écrit `colonyRecord` dans le block entity de
l'hôtel de ville et ne connaît pas `persist` ; sa fonction `World::read` est l'endroit exact où
appeler `persist::read`, et `world.apply` celui où appeler `persist::write` — deux lignes, dans un
fichier que ce module n'a pas le droit d'éditer.

---

## 5. Les managers de recettes et de compatibilité ne sont pas portés

**Ce qui manque.** `ColonyManager.write` sauvegarde aussi `compatibilityManager` et `recipeManager`
(`:586-598`), et `read` les relit (`:602-610`). Le port n'a ni `StandardRecipeManager` ni
`CompatibilityManager`.

**Ce que ça coûte.** Un compound du port ne contient que `dist` dans sa partie gestionnaire — donc un
serveur Java qui lirait ce fichier perdrait ses recettes. L'inverse est sans danger : `persist::read`
ignore les clés qu'il ne connaît pas.

**Note sur `dist`.** `ColonyManager.java:593` écrit un booléen sous `dist`
(`ColonyManagerConstants.java:10-12`) que **personne ne relit** dans l'arbre Java. Le port l'écrit
aussi, pour que le format reste le même, et ne prétend pas savoir à quoi il servait.

---

## 6. `getTopColonyId` n'a pas de source

**Ce qui manque.** `ColonyManager.java:856-868` renvoie le compteur `topID` de `ColonyList`, qui
survit à la suppression du plus grand id.

**D'où ça vient.** `founding::Registry` garde `top` et `freed` **privés** ; il expose `next_id()`
(`ColonyList.getNextColonyID`, exact) mais pas le compteur. Le port n'expose donc pas de
`top_id()` : répondre « le plus grand id vivant » serait faux dès qu'une colonie a été supprimée.

**La sortie.** Un `Registry::top_id()` d'une ligne dans `src/founding/colonies.rs`, module d'un autre
agent.

---

## 7. Les tickets de chunk (`addLoadedChunk`, `forceLoadColony`) ne sont pas portés

**Ce qui manque.** `Colony.addLoadedChunk` (`Colony.java:1860-1874`) enregistre les chunks chargés de
la colonie et, avec `forceLoadColony`, pose des tickets pour les garder chargés.

**Ce que ça coûte.** Une colonie dont personne n'est proche ne ticke pas : les citizens ne
travailleront pas hors ligne, alors que Java peut les garder éveillés. Le SDK ne lit que
`chunk_forced` (`host/levels.rs:186`) ; il n'y a pas de primitive pour poser un ticket, donc porter ce
comportement demanderait une API neuve (`[[native.module]]` ne change rien : ce n'est pas un problème
de mixin, c'est une primitive absente).

---

## 8. Aucun bus d'événements : `ColonyDeletedModEvent` n'a pas de destinataire

**Ce qui manque.** `ColonyManager.java:209` poste `ColonyDeletedModEvent` sur le bus de
l'API avant de retirer la colonie, et `:669`/`:696` `ColonyManagerLoadedModEvent` /
`…UnloadedModEvent`. D'autres mods s'y accrochent.

**Ce que ça coûte.** Un autre mod du port ne peut pas apprendre qu'une colonie a été supprimée autrement
qu'en relisant le store. Le contrat est pourtant simple (un id, une dimension) : si le SDK gagne un
bus d'événements de mod, `Dissolution` porte déjà tout ce qu'il faut pour le remplir.

---

## Divergences assumées avec Java, et pourquoi

Trois règles du port **ne sont pas** celles de Java. Elles sont dites ici plutôt que cachées dans le
code :

1. **Un propriétaire mort est effacé, pas laissé en place.** `removeBuildingClaim` ne remet pas
   `owningColony` à zéro quand aucun bâtiment ne tient plus le chunk
   (`IColonyTagCapability.java:239-283` n'affecte l'owner que dans la branche trouvée) : le chunk
   garde un id supprimé. Le port répond `RemoveOutcome::OwnerCleared` et efface — un refus nommé
   (`LookupRefusal::StaleOwner`) existe pour le cas où un vieux fichier en contient quand même un.
2. **Les remplacements d'owner sont déterministes.** Java prend `iterator().next()` sur un `HashSet`
   ou une `HashMap` (`:155`, `:159`, `:240`), donc deux exécutions peuvent choisir deux colonies. Le
   port prend le plus petit id. Le résultat est stable et testable ; il peut différer d'un serveur
   Java sur un chunk disputé par trois colonies, cas qui n'arrive pas dans une fondation normale.
3. **La suppression balaie le livre entier, pas seulement la boîte doublée.**
   `ChunkDataHelper.java:138` dérevendique `range * 2` chunks autour du centre ; un claim plus loin
   survit en Java et continue de répondre avec un id libéré. Le port fait la boîte **et** le balayage
   (`ClaimBook::release_colony`).
