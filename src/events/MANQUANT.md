# MANQUANT — ce que ce module voulait faire et que le SDK ne permet pas

Écrit pour `src/events` (bus d'événements de colonie). Chaque entrée dit **ce que je voulais
faire**, **ce que le SDK offre réellement**, **ce que j'ai fait à la place**, et **ce que ça coûte**.
Rien ici n'est contourné en silence : le module n'importe que `mod_api::{Compound, Tag}` (et
`crate::colony`), et les deux seules choses qu'il ne peut pas exprimer sont nommées ci-dessous.

Déclarations citées : `.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé
`4dc6455d`, `Cargo.toml`).

---

## 1. Un identifiant d'événement — `UUID.randomUUID()`

**Voulu.** `AbstractModEvent.java:16-23` : chaque événement naît avec `eventId =
UUID.randomUUID()`, et `IModEvent.getEventId()` (`:15`) est la seule méthode de l'interface. Un
abonné s'en sert pour dédupliquer ou pour corréler deux réactions au même événement.

**Ce que le SDK offre.** Aucune source d'aléa ni de génération d'UUID pour un mod : `host/` ne
déclare que des UUID *fournis* (joueur `players.rs:18`, entité `entities.rs:583`), jamais un
générateur. `uuid` et `rand` ne sont pas des dépendances de l'API, et en ajouter une sortirait du
contrat « aucune dépendance de registre » de `Cargo.toml`.

**Fait à la place.** `EventId(u64)` attribué par `EventBus::post` depuis un compteur
(`bus.rs`, `Inner::next_event`), sur le premier post seulement, et **conservé** si l'événement en a
déjà un. `EventId::UNASSIGNED` est l'état d'un événement fraîchement construit.

**Coût.** L'id n'est pas unique entre deux processus, et il n'existe pas avant le premier `post`.
En échange il est **reproductible**, ce qui est exactement ce qui rend `EventBus::replay` possible :
un id aléatoire rendrait la trace non rejouable (`trace.rs`, en-tête).

---

## 2. S'abonner depuis un handler — la `ConcurrentModificationException` de Java

**Voulu.** Reproduire le seul défaut de livraison de `DefaultEventBus` : `subscribe` fait
`computeIfAbsent(...).add(handler)` (`:26`) sur la **même** `ArrayList` que la boucle `for` de
`post` itère (`:40`). S'abonner depuis un handler change donc `modCount`, et le
`ConcurrentModificationException` est levé par `iterator.next()` — **hors** du `try` de `:42-50` —
donc il s'échappe de `post` et coupe les handlers restants.

**Ce que le SDK offre.** Rien à voir : c'est de la structure d'API, pas un appel hôte. Le problème
est côté port.

**Fait à la place.** Un handler reçoit `(event, &Ctx)`, et `Ctx` n'a **qu'un verbe** : `post`. Il
n'y a pas de `subscribe` sur `Ctx`, donc l'abonnement en cours de livraison est impossible — pas
refusé, *impossible*.

**Coût.** Un mod qui voulait s'abonner à la volée (par exemple au premier `ColonyCreated`) doit le
faire avant le premier post. La contrepartie est que la liste peut sortir de la map pendant la
livraison sans qu'un `RefCell` explose (`Open`, `bus.rs`), et que les trois cas de boucle qui
*existent* en Java sont refusés avec un nom (`Refusal::Reentered`, `Refusal::TooDeep`) au lieu d'un
`StackOverflowError` — lequel est un `Error`, donc **non** attrapé par le `catch (Exception)` de
`:46`.

---

## 3. Un événement de bus pour « une attaque commence »

**Voulu.** Le brief annonce « une attaque commence » comme événement de colonie. C'était le cas
d'usage qui justifiait de porter `core/colony/events/`.

**Ce que le Java offre.** **Rien.** Mesuré : aucun fichier de `core/colony/events/raid/` ne poste
d'`IModEvent` ; `grep '\.post('` sur tout l'arbre ne rend que des
`IMinecoloniesAPI.getInstance().getEventBus().post(...)` des vingt classes
d'`api/eventbus/events`, et aucune n'est une rafale. `RaidManager` crée des `IColonyEvent`
(`api/colony/colonyEvents/`), qui sont des objets **sauvegardés** dans le NBT de la colonie, pas des
messages. Un mod observe un raid en interrogeant `IEventManager.getEvents()`
(`EventManager.java:215`) et `IColonyRaidEvent.isRaidActive()` (`:50-55`).

**Fait à la place.** `raid.rs` porte la machine qui rend cette observation possible :
`Lifecycle` = la map par colonie + `onColonyTick` (`:184-212`), et `RaidStep` = ce qu'un tick a fait
à un raid (`onStart`, `onUpdate`, `onFinish`, retrait silencieux). Les étapes atterrissent dans
`Trace::notes()`, pas dans les abonnés.

**Coût.** Un abonné ne peut pas *entendre* un raid démarrer ; il peut seulement faire tourner la
machine et lire les étapes. Le jour où l'amont ajoute un `RaidStartedModEvent`, il suffira d'une
variante dans `Topic` + `ColonieEvent` + un `post` dans `Lifecycle::tick` — le reste du module ne
bouge pas.

---

## 4. Tenir les objets que Java passe — `IColony`, `IColonyView`, `IBuilding`, `ICitizenData`, `IColonyManager`

**Voulu.** Les constructeurs Java passent des objets vivants : `IColony` (`:23`), `ICitizenData`
(`:24`), `IBuilding` (`:21`), `IColonyView` (`:16`), `IColonyManager` (`:20`). Un abonné lit
`getColony().getName()`, `getCitizen().getId()`…

**Ce que le SDK offre.** Des handles de type `LevelRef`, `EntityRef`, `BlockEntityRef`
(`host/levels.rs`, `host/entities.rs`), et rien pour une colonie : concept MineColonies, absent du
vocabulaire Horizon.

**Fait à la place.** Une coupe, la même que `crate::base::ident` a déjà faite : la colonie est son
`i32`, le bâtiment est sa position `[i32; 3]` (son identité dans le port, `Identified`), le citoyen
est son `i32`, et `IColonyManager` devient `ManagerRef(u32)`. Les accesseurs `colony()`,
`citizen()`, `building()`, `player()` répondent les questions des classes abstraites
(`AbstractColonyModEvent.java:32`, `AbstractCitizenModEvent.java:36`,
`AbstractBuildingModEvent.java:32`).

**Coût.** Un abonné ne peut pas lire la colonie depuis l'événement ; il doit la demander au monde
(`crate::wiring`) avec l'id. C'est aussi ce qui rend les événements `Clone` et donc la trace
rejouable.

---

## 5. Savoir si le joueur est spectateur — `Player.isSpectator()`

**Voulu.** `PlayerEnteringModEvent.shouldShowNotification()` (`:53-56`) lit l'état spectateur du
joueur **vivant** :
`shouldShowNotification && (!player.isSpectator() || shouldShowForSpectators)`.

**Ce que le SDK offre.** `players.rs` donne `player(uuid)`, `level(host)`, mais l'état spectateur
n'y est pas exposé comme un fait lisible pour un événement.

**Fait à la place.** `Notices::should_show(spectator: bool)` prend le fait en argument, et
`ColonieEvent::player()` donne l'UUID. L'appelant (le hook qui poste) connaît le joueur, donc c'est
lui qui répond.

**Coût.** La règle est testée (`tests.rs`) mais c'est l'appelant qui doit fournir la valeur juste.
Même choix que `crate::ai::facts`, qui transforme `world.getBlockState` en observation.

---

## 6. `Entity.RemovalReason` — une énumération vanilla, hors de l'arbre

**Voulu.** `CitizenRemovedModEvent.getRemovalReason()` (`:54`) rend `Entity.RemovalReason`, dont la
valeur dit *pourquoi* le citoyen a quitté le niveau (tué, jeté, chunk déchargé…).

**Ce que le Java de l'arbre offre.** Le type est cité (`CitizenRemovedModEvent.java:22`) mais sa
déclaration est dans Minecraft, pas dans `minecolonies-src` : je ne peux donc pas citer
`Entity.java:ligne`.

**Fait à la place.** `payload::RemovalReason` redonne les cinq valeurs de la vanilla (`KILLED`,
`DISCARDED`, `UNLOADED_TO_CHUNK`, `UNLOADED_WITH_PLAYER`, `CHANGED_DIMENSION`), et le module dit
explicitement que la source n'est pas dans l'arbre — plutôt que d'inventer un `fichier.java:ligne`.

**Coût.** Aucun pour le port ; c'est une citation manquante, pas une règle manquante. Si la
référence exacte compte, il faut la jarre vanilla décompilée, comme pour
`crates/horizon-*/` côté jeu.

---

## 7. L'ordre `EventPriority` — contrat de Forge, hors de l'arbre

**Voulu.** Justifier par une ligne de source l'ordre de livraison de la seule classe Forge du
paquet, `CustomRecipesReloadedEvent` (`:9`, posté à `CustomRecipeManager.java:438`).

**Ce que l'arbre offre.** Les usages (`journeymap/v6/EventListener.java:61` et `:64`,
`QuestObjectiveEventHandler.java:61`) mais **pas** la définition de `EventPriority` ni l'ordre dans
lequel Forge trie ses listeners — c'est du code Forge, absent de `minecolonies-src`.

**Fait à la place.** `bus::Priority` reprend les cinq constantes dans l'ordre de l'énumération Forge
(`Highest` … `Lowest`), et l'ordre de livraison est `Highest` d'abord, puis l'ordre d'inscription
dans une même priorité — le contrat documenté de Forge, dit comme tel dans la doc du type.

**Coût.** Cet ordre n'est pas prouvable depuis cet arbre. S'il est faux, un seul test le dit
(`forge_s_bus_orders_by_priority_and_registration_order_inside_one_priority`) et une seule fonction
le porte (`EventBus::subscribe_by_priority`).
