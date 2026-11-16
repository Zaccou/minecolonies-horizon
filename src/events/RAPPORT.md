# RAPPORT — `src/events`, le bus d'événements de colonie

## 0. État de vérification, à lire en premier

**Rien n'a été compilé.** La règle 2 l'interdit : l'orchestrateur compile et renvoie les erreurs.
Ce que je garantis : chaque item est écrit d'après une lecture complète des sources Java citées, et
chaque `fichier.java:ligne` a été lu dans l'arbre (sauf les deux exceptions de `MANQUANT.md` §6-7).
Ce que je ne garantis pas : la compilation. À lancer de votre côté :

```sh
ssh deb 'cd /home/zaccou/opencode-server/workspace/port-minecolonies && PATH=$HOME/.cargo/bin:$PATH \
  cargo test events 2>&1 | tail -40'
```

Le module n'importe que `mod_api::{Compound, Tag}` et `crate::colony` — pas de dépendance ajoutée,
pas de fichier hors de `src/events` touché.

## 1. Trois prémisses du brief qui sont fausses, mesurées

1. **`api/colony/events/` n'existe pas.** J'ai fait `find` sur les quatre chemins annoncés : le
   dossier est `api/eventbus/events/` (20 classes + 4 classes abstraites). `api/colony/colonyEvents/`
   est autre chose — les raids et les spawns **sauvegardés** en NBT, pas des messages de bus.
   `core/colony/events/` ne contient que `raid/` (4 377 lignes), aucun `IModEvent`.
2. **« Le Java a une règle de priorité » : vrai seulement pour Forge.** `DefaultEventBus` n'a aucune
   priorité : `subscribe` ajoute en fin de `ArrayList` (`:26`) et `post` itère dans cet ordre
   (`:40`). `EventPriority` n'apparaît que sur des handlers `@SubscribeEvent` Forge
   (`journeymap/v6/EventListener.java:61,64`, `QuestObjectiveEventHandler.java:61`), et **une seule**
   classe du paquet voyage sur le bus Forge : `CustomRecipesReloadedEvent` (`:9`, postée à
   `CustomRecipeManager.java:438`). Le port a donc deux ordres : inscription pour le bus du mod,
   priorité pour Forge — et une priorité sur un sujet du bus mod est **refusée** avec un nom.
3. **« Une attaque commence » n'est pas un événement de bus.** Aucun fichier de
   `core/colony/events/raid/` ne poste quoi que ce soit. Voir `raid.rs` et `MANQUANT.md` §3.

Deux autres écarts trouvés en lisant, qui changent le code :

- `CitizenBuriedModEvent` (`:12`) et `CitizenRemovedModEvent` (`:11`) étendent
  **`AbstractColonyModEvent`**, pas `AbstractCitizenModEvent`. Un mod abonné à la classe citoyenne
  ne reçoit donc ni l'un ni l'autre, malgré leur paquet. Idem pour `PlayerEntering`/`PlayerLeaving`
  (`:11`) : paquet `…colony.permissions`, famille colonie.
- `CitizenAddedSource.BORN` (`:49`) est déclaré et **jamais posté** dans cet arbre :
  `ReproductionManager.java:213` déclenche l'événement de *description*, pas celui du bus.

## 2. Ce qui est porté

| Fichier | Lignes | Contenu |
| --- | ---: | --- |
| `topic.rs` | 213 | les 20 classes (`Topic`), les 5 familles abstraites (`Family`), les 2 bus |
| `payload.rs` | 483 | les 20 charges (`ColonieEvent`), champs Java un par un, + `EventId`, `Notices`, `CitizenAddedSource`, `RemovalReason` |
| `bus.rs` | 454 | abonnement, livraison, priorités Forge, `Refusal` nommé, `Ctx`, `Delivery` |
| `trace.rs` | 176 | la trace : events postés, livraisons, étapes de raid, requêtes de lecture |
| `raid.rs` | 303 | `EventStatus`, `Lifecycle` = `EventManager` + `onColonyTick`, `RaidStep` |
| `tests.rs` | 783 | **39 tests** |
| `mod.rs` | 78 | la table, la décision, les corrections |

Total ≈ 2 490 lignes, dont 783 de tests. **C'est au-delà des 600-900 demandés**, et l'écart est
mesurable, pas rhétorique : le paquet compte 20 classes concrètes (aucune ne peut être sautée sans
laisser un trou dans la liste), la moitié du fichier est la doc qui porte les `fichier.java:ligne`,
et les tests seuls font déjà 783 lignes à 39 tests (~20 lignes par test, chacun avec sa source). La
seule coupe que j'ai faite pour tenir : `Topic` est généré par une macro-table (`topics!`, comme
`jobs!` dans `src/jobs/job.rs`), donc `java_class` est `concat!`é du paquet et du nom simple au lieu
d'être recopié vingt fois.

## 3. Les règles de livraison, et leur source

| Règle | Source | Où |
| --- | --- | --- |
| un abonnement est une **classe exacte**, jamais un supertype | `DefaultEventBus.java:32` | `bus::EventBus::post`, testé (`subscribing_to_an_abstract_class_is_accepted_and_never_fires`) |
| ordre = ordre d'inscription sur le bus du mod | `:26` + `:40` | testé (`..._once_in_registration_order`) |
| même handler inscrit deux fois ⇒ appelé deux fois | `:26` | testé (`a_handler_that_answer_twice...`) |
| un handler qui échoue est journalisé **et la boucle continue** | `:42-50` | `Outcome::Failed`, testé |
| aucun abonné ⇒ `post` ne fait rien | `:33-35` | testé |
| Forge : `Highest` d'abord, puis l'inscription | contrat Forge (hors arbre, `MANQUANT.md` §7) | testé |
| une boucle est **refusée avec un nom** | aucune règle Java : `:40-44` récurse jusqu'au `StackOverflowError`, qui est un `Error` donc non attrapé par `:46` | `Refusal::Reentered`, `Refusal::TooDeep` |

Les trois refus nommés : `Reentered` (un handler poste un sujet déjà en cours de livraison),
`TooDeep` (plus de 8 posts imbriqués de sujets *différents* — constante **du port**, avec sa source
dite comme telle), `WrongBus` (priorité demandée sur le bus du mod, qui n'en a pas :
`EventBus.java:17`).

## 4. La trace, et ce qu'elle prouve

`Trace` garde trois listes : les événements **tels qu'ils ont été postés** (donc avant que les
handlers les modifient), une ligne par livraison avec son issue, et les étapes du raid. Deux
requêtes servent aux tests :

- `Trace::sequence(id)` / `labels(id)` — « qui a entendu cet événement, dans quel ordre » ;
- `EventBus::replay(&trace)` — reposte les mêmes événements, ids compris (un id déjà attribué est
  conservé), et un bus recâblé à l'identique rend **une trace identique**
  (`a_trace_replayed_into_a_bus_wired_the_same_way_gives_the_same_trace`).

C'est la partie qui rend le reste du port testable : un événement émis peut être rejoué hors du jeu
et ses effets comparés, sans horloge, sans aléa, sans monde.

## 5. Ce que le module ne fait pas

- **Aucun abonné réel n'est porté** : les deux seuls `subscribe` de l'arbre sont
  `journeymap/v5/EventListener.java:54` et `v6/EventListener.java:51` (mort de masse + bordures de
  colonie sur la carte), qui parlent à JourneyMap. Ils sont cités comme forme de la règle, pas
  portés.
- **Aucun `post` n'est branché** : les 25 sites d'émission sont inventoriés avec leur ligne dans
  `payload.rs`, mais c'est `crate::wiring` qui les appellera, pas ce module.
- **Rien de la sauvegarde** : `EventManager.readFromNBT`/`writeToNBT` (`:221-264`) sérialise des
  `IColonyEvent` par `ResourceLocation` de type ; ce n'est pas du bus et ce n'est pas porté.
- Les accesseurs de famille ne répondent pas `getColonyManager()` ni `getSavedCitizenNbt()`
  (le champ `saved` est public) : ce sont des champs, pas des questions.

## 6. Points à trancher par un humain

1. **L'écart de volume** (§2) : garder la couverture complète des 20 classes, ou couper.
2. **`EventId` en compteur** au lieu d'un UUID : c'est ce qui rend le rejeu possible, mais deux
   processus produiront les mêmes ids (`MANQUANT.md` §1).
3. **`Refusal::TooDeep`** refuse une chaîne de 8 posts imbriqués qui n'est pas une boucle : Java
   l'autorise. J'ai préféré un nom à un dépassement de pile wasm, mais c'est un refus que Java n'a
   pas.
4. **`Lifecycle::tick` ordonne par id** : Java itère une `HashMap` (`:54`, `:186`) donc sans ordre.
   Un test dépend de ce choix (`the_machine_visits_its_events_in_id_order...`).
