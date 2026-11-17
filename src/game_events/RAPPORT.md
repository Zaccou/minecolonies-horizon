# RAPPORT — les écouteurs d'événements (`src/game_events`)

## Ce qui est livré

13 fichiers Rust + 2 documents, dans `src/game_events/` seulement. Aucune écriture hors du
dossier, aucune compilation lancée (règle 2 de la mission).

| fichier | lignes | rôle |
| --- | ---: | --- |
| `mod.rs` | 166 | le plan du module, les réexports, le vocabulaire pour `lib.rs` |
| `table.rs` | 701 | **111 lignes : une par branche du Java**, avec `fichier.java:ligne` |
| `event_handler.rs` | 895 | les décisions serveur (butin, placement, lit, chunk, spawn, tick) |
| `colony_story.rs` | 658 | les quatre tables d'histoires, le filtre biome, le coût d'un fichier cassé |
| `quest_objectives.rs` | 611 | les cinq registres de quêtes, et le `break` de trois d'entre eux |
| `client_event_handler.rs` | 476 | infobulles, ligne F3, clic de navigation, copies obsolètes |
| `data_pack_sync.rs` | 256 | ce qu'un `/reload` reconstruit, qui est prévenu, les quatre ordres documentés |
| `capability_providers.rs` | 255 | trois capacités et la primitive qui les remplace |
| `client_registry.rs` | 238 | 94 couches de modèle, 47 renderers, 7 renderers de block entity, 1 touche |
| `texture_reload.rs` | 194 | quelle arborescence nomme un pack de textures citoyen |
| `gather_data.rs` | 182 | 41 générateurs qui tournent avant le jar, jamais en jeu |
| `client_chunk_updated.rs` | 178 | la notification de claim et la file qui la diffère |
| `fml_event_handler.rs` | 142 | l'ordre des 9 reload listeners et les trois moments du cycle de vie |
| `MANQUANT.md` | — | 17 trous, chacun avec la primitive exacte |
| **total** | **≈ 4 950** | dont **1 900 lignes de tests (167 tests)**, ≈ 1 150 de commentaires cités, ≈ 1 900 de code |

Mesures : `grep -c "#\[test\]"` = 167 (la mission en demandait 40) ; `counts()` dans
`table.rs` rend 111 lignes, 6 `Ported`, 47 `Here`, 40 `Hook`, 17 `Missing`, 1 `Superseded` ; la somme des
`Listener::lines()` vaut 2 947 et + `package-info.java` (4) = **2 951**, le total de la
mission.

## La prémisse fausse, et ce qu'elle change

**« 14 fichiers, 2 951 lignes »** est exact, mais **13 des 14 contiennent du code** :
`package-info.java` fait 4 lignes de javadoc de paquet. `Listener::ALL` a donc 13 entrées,
et `CODE_LINES + PACKAGE_INFO_LINES = 2 951` est un test, pas une note.

**« 700 à 1 000 lignes »** ne tient pas avec « une ligne par branche » : les 13 classes
prennent **111 décisions distinctes** dans le Java (42 dans `EventHandler` seul), et la
seule table fait 650 lignes une fois compactée à une ligne par branche. Le module livré
fait ≈ 4 820 lignes, dont **1 812 de tests** et **1 138 de commentaires cités**. Je n'ai
pas coupé les branches pour rentrer dans le budget : une table qui saute des branches est
exactement ce que la mission interdit. Si le budget de lignes comptait le *code de
production* seul, il est à ≈ 1 900 lignes — au-dessus de 1 000, mais pas d'un facteur 5.

## Comment le port est organisé, et pourquoi

**Aucun `impl Hooks` ici.** `src/lib.rs` est à l'orchestrateur, et une fonction qui prend
les champs d'un hook rend une décision nommée : `hut_placement(&Placement) -> Verdict`,
`join_verdict(&Join)`, `bed_blockers(bed, &citizens)`, `chunk_entry_due(…)`,
`slowness_factor(ms)`, `crop_drop_chance(on_grass, hoe)`, `food_tooltip(…)`,
`research_colour(…)`, `pick(&stories, biome, in_tag, roll)`, `pack_of(path)`,
`Registry::block_broken(…)`. Conséquence : ces règles se testent sans niveau, sans colonie
et sans wasm, et une règle qui a besoin d'une valeur absente de l'API devient une ligne
`Missing` **visible** au lieu d'un hook qui compile et répond faux.

**Ce qui était déjà porté n'est pas réécrit** (règle 6). Six lignes sont `Ported` :
le clic sur une hutte et son échelle de placement sont `src/blocks` (`Refusal::LacksAccess`,
`Refusal::LacksPlaceHuts`, `Refusal::OutsideColony`), les commandes sont `src/commands`
(`SPECS`), les neuf data listeners sont `src/data_listeners` **dans le même ordre**, et la
forme qui traverse est `src/network::common::ChunkCap`. La seule règle que `src/blocks`
n'avait pas — `allowOtherDimColonies` et le passage du créatif — est portée ici
(`hut_placement`), avec la nuance que le Java sépare : le créatif remplace `canPlaceAt`
mais **pas** `PLACE_HUTS` (`EventHandler.java:743-751`).

**Les registres de quêtes ne dupliquent pas `src/quests`.** `src/quests/MANQUANT.md` §7
disait que l'enregistrement était « à la charge de l'appelant » : c'est ce fichier.
`Registry` tient les cinq cartes, `Hold` remplace `IQuestInstance` par `(quest, index)`,
et ce qu'un événement fait au compteur reste `Objective::observe`.

## Les constats qui ont changé le port

1. **`farmland_trample` remet un `LoopContext`, pas un `Host`**
   (`hooks/farmland_trample.rs:9-17`, `world/phase.rs:813-832`). La règle a besoin du
   métier du colon et de la force de `SOFT_SHOES` : elle n'est **pas** portable. Ligne
   `Missing`, §2 de `MANQUANT.md`.
2. **`sound_play` n'est pas un hook dans ce SDK** : `hooks/sound_play.rs` ne contient qu'un
   `trait Notes`, et `grep -c "fn sound_play(" generated/mod_api_generated.rs` = 0. Couper
   les voix de citoyens n'a donc nulle part où vivre.

   *Correction en cours de route* : j'avais aussi classé la durée des ticks (`:864-875`)
   comme manquante, parce que `ServerTickEvent` ne porte qu'un `LifecycleContext`. C'était
   faux — `ServerHost::tick_times()` rend les cent durées en nanosecondes indexées
   `n % 100` (`host/config.rs:96-102`), et `tick_duration_ms` les lit maintenant. La ligne
   est passée de `Missing` à `Hook`, et je le signale plutôt que de laisser un trou
   commode.
3. **`EventHandler.java:387` teste `-1`, un sentinelle que personne n'écrit.**
   `ChunkCapData` part à `0` (`ChunkCapData.java:35`), `IColonyTagCapability` à
   `NO_COLONY_ID` = 0 (`:120`, `ColonyManagerConstants.java:56`), et `-1` n'apparaît que
   dans cette comparaison dans tout l'arbre. La branche « visiteur » tourne donc aussi sur
   un chunk non réclamé (le lookup rend `null`, sans effet). `claim_effects` garde les deux
   tests et le dit.
4. **`EventHandler.java:499-512` nettoie `playerPositions` à l'intérieur de la boucle sur
   les colonies** : un serveur sans colonie ne relâche jamais l'UUID. Et
   `QuestObjectiveEventHandler` ne nettoie **rien** à la déconnexion — `held_by(player)`
   mesure ce qui reste.

Deux constats plus petits, tous deux dans la table : : `ChunkCapData` et
`IColonyTagCapability` utilisent deux sentinelles différentes pour « aucune colonie » (0 et
-1), et `QuestObjectiveEventHandler` fait un `break` après avoir retiré l'objectif de
mauvaise classe (`:83`, `:107`, `:142`), ce qui **silencie** toutes les quêtes suivantes
enregistrées pour le même bloc — les deux cartes par colonie (`:237-270`) n'ont pas ce
défaut.

## Ce que l'orchestrateur doit brancher dans `lib.rs`

Rien n'est branché, et c'est volontaire (règle 1 + la mission). Les points d'entrée
existent et sont testés ; il reste à les appeler depuis le hook qui les porte :

| hook Horizon | appel |
| --- | --- |
| `entity_join_level` | `event_handler::join_verdict` (colonie 0, doublon) — `src/citizen.rs` est déjà sur ce hook |
| `player_logged_in` / `player_logged_out` | rôles de colonie (`table.rs:466-512`) |
| `entity_travel_to_dimension` / `player_changed_dimension` | visiteur + abonné (`table.rs:300-344`) |
| `player_tick_post` | `event_handler::chunk_entry_due` puis `claim_effects` ; les deux paquets sont dans `src/network` |
| `break_block` | `quest_objectives::Registry::block_broken` + l'événement spawner (`table.rs:550-574`) |
| `right_click_block` | `event_handler::bed_blockers` (le clic hutte est déjà dans `src/menu`) |
| `mob_spawn_position_check` | `event_handler::spawn_denied` |
| `living_conversion_pre` | `event_handler::conversion_is_ours` |
| `chunk_load` / `client_chunk_load` | `client_chunk_updated::Late::apply` |
| `loot_table_load` | `event_handler::crop_ladder` + `DUNGEON_CROP_CHANCE` |
| `item_tooltip` | `client_event_handler::{dyed_tooltip, food_tooltip, research_colour, building_line}` |
| `register_layer_definitions` | `client_registry::layer_id` sur `LAYERS` |
| `add_server_reload_listeners` | `fml_event_handler::RELOAD_ORDER` (déjà satisfait par `src/data_listeners`) |
| `server_tick` | `event_handler::slowness_factor(event_handler::tick_duration_ms(&host.tick_times(), tick))` |
| `register_range_select_item_model_property` | `client_registry::goggles_disabled` (propriété `disabled`) |

Aucun de ces branchements n'appelle d'API non vérifiée : les 35 hooks distincts cités par
les 40 lignes `Hook` sont listés dans `table::HOOKS` et `hook_exists` les valide, et chaque fonction ci-dessus ne prend que
des valeurs.

## Pas de compilation, et ce que cela laisse

La règle 2 de la mission interdit de lancer une compilation ; le `AGENTS.md` du dépôt
demande `port-check`. J'ai suivi la mission, donc **le module n'a pas été compilé**. Ce
qui a été vérifié à la place :

- les 94 noms de `client_registry::LAYERS` et les 41 de `gather_data::PROVIDERS` ont été
  extraits mécaniquement des `.java` et comparés (`diff` de listes : identiques, même
  ordre d'`addProvider`) ;
- les 110 citations `fichier.java:ligne` sont validées par trois tests (format, fichier du
  listener, borne de fin de fichier) ;
- les 35 hooks cités existent : `grep -c "fn <hook>(" generated/mod_api_generated.rs` = 1
  pour chacun, et `table::hook_exists` le vérifie à l'exécution des tests ;
- les chemins d'API utilisés ont été lus dans le SDK : `mod_api::{GoalSpec, MobFilter}`
  (`entity/types.rs:408`, `:393`, réexportés par `pub use entity::types::*;`),
  `mod_api::serde_json::{Value, from_str}` (déjà utilisé par `src/quests/json.rs:307`),
  `crate::network::common::ChunkCap`, `crate::quests::objective::{Objective, Target,
  Dialogue}`.

Le risque résiduel est un **nom de champ** ou une signature de `quests::objective` qui
aurait bougé pendant que j'écrivais (`Objective::BreakBlock` a sept champs, `Dialogue` en a
deux) : ce sont les seuls endroits où je dépends d'un autre module. Si la compilation
rougit là, c'est un ajustement de champ, pas un changement de règle.

## Ce que je n'ai pas fait, et pourquoi

- **Aucun `impl Hooks`, aucun appel à `Host`.** `lib.rs` appartient à l'orchestrateur, et
  un hook écrit ici aurait obligé à inventer les valeurs que `MANQUANT.md` déclare
  absentes.
- **Pas de rendu** : `WorldEventContext` (bordure, plans, chemins) et les 7 renderers de
  block entity restent `Missing` ; le port dessine son citoyen comme un
  `minecraft:wandering_trader` (`src/citizen.rs`).
- **Pas de datagen** : le port livre le JSON, `gather_data.rs` garde les 41 pour un futur
  `cargo horizon-mod datagen`.
- **Rien n'a été écrit dans `minecolonies-src`** : les sources Java sont lues comme
  spécification (leur `AGENTS.md` demande de ne pas y générer de code, c'est respecté), et
  les identifiants de `core/event/` ne sont copiés nulle part.

## Décision qui appartient à Simon

§8 de `MANQUANT.md` : la seule façon de porter Halloween et le 1er avril est de lire la
date de la machine, ce que l'API refuse par principe (un tick rejoué doit être
reproductible). Deux options honnêtes — un réglage serveur `holiday_mode`, ou abandonner
la fonctionnalité. Le port ne tranche pas.
