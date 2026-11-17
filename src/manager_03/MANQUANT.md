# Ce que `manager_03` ne porte pas, et ce que ça coûte

Quatre gestionnaires, quatre moitiés de monde. Chaque refus est nommé dans le
code ; ce fichier liste ce qui **n'a pas de code du tout**, et ce qu'un appelant
devra fournir lui-même.

## 1. `RaidManager` : tout ce qui touche au `Level`

Le fichier Java fait 1 253 lignes ; les règles portées dans `raid.rs` en
couvrent la décision, pas la recherche. Manquent, avec la ligne qui les nomme :

| Ce qui manque | Où | Ce que l'appelant doit fournir |
| --- | --- | --- |
| le point de spawn dans un cercle de 500 blocs, 8 essais, `BlockPosUtil.findAround` sur 30/3 puis 15/10 | `:484-625` | `Env::points` — des positions déjà trouvées |
| le choix de l'événement selon le biome, l'océan, le sable et deux jets de dé | `:336-453` | `Point::raid_type` — le type déjà choisi, ou `None` |
| la construction de l'événement de raid (`PirateRaidEvent`, `HordeRaidEvent`, …) et `colony.getEventManager().addEvent` | `:370-452` | `crate::events::Lifecycle::add` |
| le chemin des raiders vers le bâtiment le plus proche (`PathJobRaiderPathing`) | `:468-476` | rien : le pathfinding n'existe pas dans le port |
| `getRandomBuilding` et l'alerte des gardes (`setTempNextPatrolPoint`) | `:881-929` | — |
| les messages et le son (`MessageUtils`, `PlayAudioMessage`, `RaidSounds`) | `:1060-1076` | — |
| le modificateur de bonheur `RAIDWITHOUTDEATH` | `:1078-1081` | `crate::citizens::happiness` |
| `canRaid()` : monde paisible, config, événements autorisés, **un joueur important dans la colonie** | `:795-801` | `Env::can_raid`, déjà calculé |

Ce qui est **volontairement partiel**, et testé comme tel : `nightFall` ne lance
pas le raid dû (`:769`). Il rend `NightStep::RaidDue` ; l'appelant appelle
`raider_event` avec `next_settings()`, puis `raid_resolved(resultat)`. C'est la
seule façon de garder ce fichier sans `Level`, et c'est aussi ce que fait Java,
juste en un seul corps de méthode.

`setPassThroughRaid` (`:1100-1103`) n'a **aucun appelant** dans cette version du
dépôt : la méthode est portée, elle n'est branchée sur rien.

## 2. `AnimalManager` : l'entité et le réseau

| Ce qui manque | Où | Pourquoi c'est dehors |
| --- | --- | --- |
| la `WeakReference` vers l'entité et `getManagedAnimal` (qui vide la référence quand l'entité est retirée) | `AnimalData.java:258-271` | c'est un handle vivant ; `Entry::bound` dit seulement s'il y en a un |
| `initEntityValues`, `setManagedAnimal`, `getLastPosition` | `:112-124`, `:279-292`, `:382-386` | lisent `entity.blockPosition()` |
| `serializeViewNetworkData` et `ColonyViewAnimalViewDataMessage` | `AnimalData.java:196-207`, `AnimalManager.java:440-445` | paquets ; `Registry::send_plan` donne qui envoyer |
| `update(tickRate)` | `AnimalData.java:97-106` | le corps est un no-op, littéralement |
| `tickAnimalData` et `onColonyTick` | `AnimalManager.java:360-374` | boucle vides |

**Deux choses à savoir avant de brancher `write`/`read`.** Le compound écrit est
celui de la **colonie** (`{animalManager: {animals, nextID}}`), pas celui que
`Colony.write:940-942` fabrique (double imbrication, voir la note du module). Et
`Entry::uuid` est un `Option` : Java invente un UUID aléatoire à chaque
sauvegarde quand il n'en a pas (`AnimalData.java:155`), ce qu'un fichier de
colonie ne peut pas se permettre.

## 3. `JobDataManager` : le registre et le tampon

- **Le registre n'est pas ici** : c'est `crate::jobs::Job` (`Job::from_path`). Ce
  fichier ne porte que la résolution du nom et ses échecs.
- `createViewFrom` (`:72-93`) est porté pour l'ordre des trois étapes
  (`load_view`), pas pour le `FriendlyByteBuf` : la lecture `readUtf(32767)` et
  le décodage de la vue sont à l'appelant.
- La validation de charset de `ResourceLocation` (vanille, absente de cet arbre)
  n'est pas reproduite : un nom qui ne contient ni espace ni majuscule mais un
  caractère interdit par vanilla passera ici et échouerait en Java.

## 4. `InteractionResponseHandlerManager` : les objets d'interaction

- Aucune des sept classes (`StandardInteraction`, `PosBasedInteraction`,
  `RequestBasedInteraction`, `RecruitmentInteraction`, `QuestDialogueInteraction`,
  `QuestDeliveryInteraction`, `SimpleNotificationInteraction`) n'est portée : la
  table des sept types, la classe que chacun reconstruit, et le chemin sauvegardé
  sont portés ; les champs (un `pos`, une `request`, un `dialogue`) sont le
  domaine de `crate::requests` et de `crate::citizens`.
- Le protocole client d'une interaction (les réponses, `InteractionResponse`), les
  paquets de dialogue et la fenêtre sont dehors.
- `handler.getId()` (`CitizenData.java:1502`), qui indexe l'option dans la carte
  du citoyen, n'est pas modélisé : `load_options` rend les valeurs dans l'ordre du
  fichier, et l'appelant tient la carte.

## 5. Ce que les quatre fichiers partagent et n'écrivent pas

Rien n'appelle `colony.markDirty()` : `Manager::mark_dirty` et `Registry::mark_dirty`
exposent le drapeau, `Manager::set_spies` et `raider_event` le lèvent, mais c'est
l'appelant qui le propage à la colonie (`RaidManager.java:248`, `:458`).
