# RAPPORT_AUDIT — relecture de fidélité de `src/game_events`

Audit de **49 entrées** (méthodes ou branches d'une même méthode) de la couche
événementielle, Java ouvert à la ligne citée, comparé au Rust, avec les deux références.
**5 divergences réelles trouvées et corrigées** (12 tests ajoutés pour les verrouiller), 3
divergences assumées et documentées, aucune compilation.

## Ce que j'ai audité, et ce que je n'ai pas audité

Toutes les méthodes viennent de `core/event/` — **ma** tranche. Le brief cite aussi
`core/colony/managers` : ces classes sont portées par `src/manager_01..09`, qui
appartiennent à d'autres agents, et la règle 3 interdit d'y écrire. Je les ai donc lues
seulement là où **ma** tranche en dépend (`ColonyUtils.getOwningColony`,
`PackageManager.addCloseSubscriber`, `EventManager.onTileEntityBreak`) et je le dis ligne
par ligne quand c'est le cas (`claim_effects`, `spawn_denied`, `onTileEntityBreak`).

Échantillon : les 49 entrées qui **portent une règle** — borne, ordre, refus, durée,
formule — réparties sur les 13 classes. Les délégations pures (`FMLEventHandler.onServerTick`
→ `IColonyManager.onServerTick`) et les enregistrements en masse (94 couches de modèle)
sont cités une fois chacun pour la forme, pas trente fois.

Légende des verdicts : **fidèle** = même règle, même ordre, mêmes bornes ; **divergent** =
une différence observable ; **absent** = pas porté, et `MANQUANT.md` dit pourquoi.

## Tableau

| # | Java | Méthode | Règle portée | Rust | Verdict |
|---:|---|---|---|---|---|
| 1 | `EventHandler.java:138-146` | `onEntityAdded` (buts d'attaque) | config && `Enemy` && !blacklist && pas une entité coloniale → buts 6 (colons) et 7 (mercenaires) | `event_handler.rs:216` `attack_goals` | fidèle |
| 2 | `EventHandler.java:143-145` | priorités et `mustSee` | 6 puis 7, `mustSee = true`, `mustReach = false` (constructeur à 3 args) | `event_handler.rs:216-244` | fidèle |
| 3 | `EventHandler.java:148-151` | doublon d'entité coloniale | `AbstractFastMinecoloniesEntity` + UUID déjà présent → annuler | `event_handler.rs:180` `join_verdict` | **divergent → corrigé** (voir D1) |
| 4 | `EventHandler.java:153-157` | colon de colonie 0 | **`instanceof EntityCitizen`** et `getColonyId() == 0` → annuler | `event_handler.rs:180-192` | **divergent → corrigé** (D1) |
| 5 | `EventHandler.java:185-186` | chance de butin | 0.001 sur `minecraft:grass`, 0.01 sinon | `event_handler.rs:112` `crop_chance` | fidèle |
| 6 | `EventHandler.java:197-219` | échelle des houes | ×10, ×8, ×6, ×5, ×4 (autre houe), ×1 (aucune), en `AlternativesEntry` donc premier qui passe | `event_handler.rs:122-140` `crop_drop_chance`/`crop_ladder` | fidèle |
| 7 | `EventHandler.java:225-234` | pool du donjon | `minecraft:chests/simple_dungeon` : chaque culture à 0.005 | `event_handler.rs:55` `DUNGEON_CROP_CHANCE` | fidèle |
| 8 | `EventHandler.java:242-258` | capacités | 1 sur chunk (`closecolony`), 2 sur level (`chunkupdate`, `colonymanager` avec drapeau overworld) | `capability_providers.rs:40-110` | fidèle (stockage) / absent (événement, `MANQUANT.md` §1) |
| 9 | `EventHandler.java:265-293` | chargement/déchargement de chunk | serveur → `loadChunk`/`unloadChunk`, client → `applyLate` | `client_chunk_updated.rs:82` `Late::apply` | fidèle |
| 10 | `EventHandler.java:300-320` | sortie de dimension | colonie du chunk quitté : `removeVisitingPlayer` + `removeCloseSubscriber` | table `table.rs` + `MANQUANT` non requis | fidèle (règle décrite, hook `entity_travel_to_dimension`) |
| 11 | `EventHandler.java:352-373` | porte d'entrée de chunk | END + serveur + `gameTime % 100 == 0` ; mémorise **avant** le test de chunk vide | `event_handler.rs:405` `chunk_entry_due` | **divergent → corrigé** (D2) |
| 12 | `EventHandler.java:387-416` | deux gardes de claim | visite si `owning != -1`, bâtiments si `owning != 0` | `event_handler.rs:443` `claim_effects` | fidèle (corrigé à la tranche précédente) |
| 13 | `EventHandler.java:450-457` | spawn dans un bâtiment | `getBuildingLevel() >= 1 && isInBuilding(pos)` → `DENY` | `event_handler.rs:466` `spawn_denied` | fidèle |
| 14 | `EventHandler.java:466-492` | entrée dans le monde | rang `isColonyManager` → 3 paquets ; bannière de ralliement → diffusion | table `table.rs` | fidèle (règle décrite) |
| 15 | `EventHandler.java:499-512` | sortie du monde | `removeCloseSubscriber` + `removeImportantColonyPlayer` par colonie ; `playerPositions.remove` **dans** la boucle | table + `quest_objectives.rs:359` `held_by` | fidèle, et le défaut est consigné |
| 16 | `EventHandler.java:517-543` | franchissement de chunk d'un colon | garde + `pvp_mode` + chunk étranger → `addGuardToAttackers` | table `table.rs` | absent (`Missing`) — aucun hook ; le Java l'appelle par **sondage 20 ticks** (`EntityCitizen.java:769-776`), pas par événement de mouvement |
| 17 | `EventHandler.java:550-574` | casse d'un spawner | `nextSpawnData` → colonie par dimension → `onTileEntityBreak` | table `table.rs` | fidèle (règle décrite, hook `break_block`) |
| 18 | `EventHandler.java:610-633` | lit occupé | clic sur le pied → tête par `FACING` ; colon endormi dont `getBedPos()` égale → annuler + message, **sans `break`** | `event_handler.rs:354-380` | fidèle |
| 19 | `EventHandler.java:635-698` | annulation d'un clic avec hutte | client : annuler ; serveur : annuler sauf si l'échelle de placement accepte | `table.rs` + `blocks` | fidèle |
| 20 | `EventHandler.java:709-753` | échelle de placement | dimension d'abord, puis hôtel de ville, puis `PLACE_HUTS` **avant** le créatif, créatif ne remplaçant que `canPlaceAt` | `event_handler.rs:311` `hut_placement` | fidèle |
| 21 | `EventHandler.java:760-784` | chargement du monde | manager + Halloween (31/10, 1-2/11) et 1er avril | table `table.rs` | fidèle pour le manager, absent pour les dates (`Missing`) |
| 22 | `EventHandler.java:805-816` | piétinement | colon `JobFarmer` + `SOFT_SHOES > 0` → annuler | table `table.rs` | absent (`Missing`) — le hook remet un `LoopContext` |
| 23 | `EventHandler.java:823-862` | conversion de villageois | zombie-villageois → villageois, taverne ≥ 1 : visiteur, annuler, interaction, son 1027, jeter le zombie | `event_handler.rs:475` `conversion_is_ours` | fidèle (décision), reste décrit en table |
| 24 | `EventHandler.java:864-875` | lenteur du tick | `tickTimes[n % 100] / 1e6`, `> 50` → `clamp(ms / 50, 1, 5)`, sinon 1 | `event_handler.rs:488-511` | fidèle |
| 25 | `ClientEventHandler.java:108-131` | copies d'entité côté client | même UUID, entité différente → jeter les ids | `client_event_handler.rs:53` `stale_copies` | fidèle |
| 26 | `ClientEventHandler.java:142-158` | voix de citoyens | `!citizenVoices` && namespace `minecolonies` && path `citizen.` → muet | `client_event_handler.rs:68` `mutes_citizen_voice` | fidèle (règle), absent (aucun hook d'annulation de son) |
| 27 | `ClientEventHandler.java:186-196` | infobulle de teinture | couleur **connue** seulement (`ifPresent`), `item.dyed` retirée, ligne insérée **à l'index 1** | `client_event_handler.rs:120` `dyed_tooltip` | **divergent → corrigé** (D3) |
| 28 | `ClientEventHandler.java:234-273` | infobulle de nourriture | non comestible → `wrongfood` et **retour** ; sinon palier, puis `nomenu` si le cuisinier ne sert pas, ou `needbetterfood` | `client_event_handler.rs:157` `food_tooltip` | fidèle |
| 29 | `ClientEventHandler.java:290-296` | recettes affichées | `showTooltip` **et** `crafter.length() >= 2` | `client_event_handler.rs:241` `recipe_is_shown` | fidèle |
| 30 | `ClientEventHandler.java:324-332` | couleur d'une recherche | colonie **et** (recherche terminée **ou** effet > 0) → aqua, sinon rouge | `client_event_handler.rs:187` `research_colour` | fidèle |
| 31 | `ClientEventHandler.java:343-371` | ligne de bâtiment | niveau > 0 → ligne « niveau N » (aqua si présent) ; niveau ≤ 0 → « disponible » gris italique | `client_event_handler.rs:199-238` | fidèle |
| 32 | `ClientEventHandler.java:451-474` | ligne F3 | colonie ici → nom + distance ; sinon `far enough` → « aucune » ; sinon la plus proche + distance minimale ; `(int) sqrt(d²)` | `client_event_handler.rs:262-290` | fidèle |
| 33 | `ClientEventHandler.java:487-503` | clic de navigation | client, main principale, `BlockItem`, `hitResult` non `MISS`, bloc navigable → annuler en `SUCCESS` | `client_event_handler.rs:311` `browse_click` | fidèle |
| 34 | `FMLEventHandler.java:45-57` | ordre des reload listeners | 9 écouteurs, cet ordre | `fml_event_handler.rs:22` `RELOAD_ORDER` | fidèle |
| 35 | `DataPackSyncEventHandler.java:52-58` | reconstruction | `FurnaceRecipes` → `compatibility.discover` → `resolveTemplates` → `buildLootData` | `data_pack_sync.rs:42` `DISCOVER_ORDER` | fidèle |
| 36 | `DataPackSyncEventHandler.java:110-114` | audit des tags | config && (rechargement **ou** personne en ligne) | `data_pack_sync.rs:124` `audit_wanted` | fidèle |
| 37 | `QuestObjectiveEventHandler.java:74-86` | casse de bloc | copie de la liste, `onBlockBreak` si la classe va, sinon **retirer + `break`** | `quest_objectives.rs:229` `block_broken` | fidèle (y compris le `break`) |
| 38 | `QuestObjectiveEventHandler.java:237-250` | recherche terminée | par colonie, sans retrait ni `break` | `quest_objectives.rs:302` `research_completed` | fidèle |
| 39 | `QuestObjectiveEventHandler.java:171-174` | désinscription | `getOrDefault(...).getOrDefault(...).remove(quest)` : première occurrence | `quest_objectives.rs:181` `forget` | fidèle |
| 40 | `ColonyStoryListener.java:88-97` | `pickRandom` | lignes valides pour le biome, une au hasard, `""` si aucune | `colony_story.rs:455` `pick` | fidèle |
| 41 | `ColonyStoryListener.java:99-125` | `apply` | **vider les 4 ensembles d'abord**, puis un fichier = un `catch` | `colony_story.rs:225` `apply` | **divergent → corrigé** (D5, le `HashSet`) |
| 42 | `ColonyStoryListener.java:127-150` | `parseStory` | `getAsString()` puis id valide, sinon fichier perdu ; id inconnu ignoré | `colony_story.rs:309-397` | **divergent → corrigé** (D4) |
| 43 | `ColonyStoryListener.java:152-184` | `parseStoryText` | `content` lu par `getAsString()` (un nombre est son texte), un `StoryText` par élément | `colony_story.rs:340-367` | **divergent → corrigé** (D4) |
| 44 | `ColonyStoryListener.java:201-233` | `BiomeFilter.or`/`parse` | `#tag` → test de tag ; id → résolution (jette si inconnu) ; `ALL` est l'identité du `or` | `colony_story.rs:394-450` | fidèle (l'effet), divergent sur le nom du refus pour un `biomes` non textuel (§« assumé ») |
| 45 | `TextureReloadListener.java:34-53` | `prepare` | `textures/entity/citizen`, chemin contenant `png`, `split("/")[3]`, dossier non vide | `texture_reload.rs:55-85` | fidèle (règle), **divergent assumé** sur l'ordre (D6) |
| 46 | `TextureReloadListener.java:56-60` | `apply` | `clear()` puis `addAll` | `texture_reload.rs:93` | fidèle |
| 47 | `ClientRegistryHandler.java:56-261` | 94 couches de modèle | `<mod>:<nom>#<nom>`, 94, toutes uniques | `client_registry.rs:38-63` | fidèle (vérifié mécaniquement contre le Java) |
| 48 | `ClientRegistryHandler.java:368-371` | propriétés d'item | `throwing` (porteur qui utilise) et `disabled` (drapeau du mod) | `client_registry.rs:127-143` | fidèle (formules) ; `throwing` absent côté hook (`MANQUANT.md` §16) |
| 49 | `GatherDataHandler.java:19-72` | 41 générateurs | cet ordre d'`addProvider`, client/serveur/les deux | `gather_data.rs:52-101` | fidèle (vérifié mécaniquement, ordre identique) |

## Divergences trouvées et corrigées

### D1 — le test « colonie 0 » s'appliquait à trop d'entités

**Java.** `EventHandler.java:148` teste `instanceof AbstractFastMinecoloniesEntity` pour le
**doublon**, et `:153` teste `instanceof EntityCitizen` pour la **colonie 0**. Deux classes
différentes, donc deux ensembles d'entités différents : un visiteur ou un mercenaire est une
entité coloniale rapide et n'est pas un `EntityCitizen`.

**Rust (avant).** Un seul drapeau `JoinKind::FastColonyEntity` portait les deux règles,
donc un visiteur avec `colony_id == 0` était refusé par le port et accepté par le Java.

**Correction.** `Join` porte deux faits (`fast_colony_entity`, `citizen`) au lieu d'un
`JoinKind`, et `join_verdict` les emploie séparément — `event_handler.rs:152-192`. Test qui
verrouille : `the_colony_test_only_applies_to_citizens`.

### D2 — la position du joueur n'était mémorisée que si le chunk était travaillé

**Java.** `:366` fait `playerPositions.put(uuid, chunkPos)` **avant** le test `chunk.isEmpty()`
de `:370-373`. Un chunk vide est donc mémorisé et ne sera plus réexaminé ; une version qui
ne mémorise que le travail refait le test toutes les 100 ticks.

**Rust (avant).** `chunk_entry_due` rendait un `bool`, donc la mémoire et le travail ne
pouvaient pas différer.

**Correction.** Elle rend `ChunkEntry { remember, work }` — `event_handler.rs:385-415` ;
un chunk vide donne `remember: true, work: false`. Tests :
`an_empty_chunk_is_remembered_but_not_entered`, `a_tick_that_fails_the_gate_remembers_nothing`.

### D3 — la teinture s'écrivait même sans couleur connue

**Java.** Tout le corps de `:186-196` est dans `getDyeColor(stack).ifPresent(c -> …)` : sans
couleur connue, il n'y a **ni** retrait de `item.dyed` **ni** ajout.

**Rust (avant).** `dyed_tooltip(lines, "red")` prenait une couleur obligatoire et réécrivait
toujours la ligne.

**Correction.** `dyed_tooltip(lines, Option<&str>) -> bool` — `client_event_handler.rs:120` ;
`None` ne touche à rien et rend `false`. Test : `an_unknown_colour_leaves_the_vanilla_line_alone`.

### D4 — `type` et `content` étaient lus comme des chaînes strictes

**Java.** `:129` lit `type` par `getAsString()` : un nombre devient son texte (`3` → `"3"`,
soit `minecraft:3`, id **valide** donc ignoré en silence) ; une chaîne qui n'est pas un
`ResourceLocation` (majuscule, espace, deux-points en trop, vide) **jette** et le `catch`
perd le fichier. `:172-184` lit `content` de la même façon : un nombre ou un booléen
devient une ligne de texte.

**Rust (avant).** Tout `type` non textuel était refusé (donc un fichier que Java ignore
était perdu), et un `content` numérique était refusé (donc une ligne que Java garde était
perdue).

**Correction.** `scalar_text`, `story_text` et `resource_location_is_valid` —
`colony_story.rs:362-412` — plus six tests (`a_numeric_type_is_its_text_and_is_ignored_not_refused`,
`an_id_resource_location_refuses_loses_the_file`, `a_bare_path_is_a_minecraft_id_and_is_ignored`,
`a_numeric_content_is_its_text`, `an_object_content_loses_the_file`,
`a_type_that_is_an_object_or_an_array_loses_the_file`).

### D5 — les quatre ensembles d'histoires ne dédupliquaient pas

**Java.** `abandonedColonyNames` et les trois autres sont des `HashSet<StoryText>` (`:61-65`)
et `StoryText` est un **record** (`:241`) sur un `BiomeFilter` qui est un record autour d'un
`Predicate` (`:189`). Un lambda n'a pas d'égalité de valeur : deux filtres ne sont égaux que
s'ils sont **la même instance**. C'est le cas d'un seul, `BiomeFilter.ALL` (`:194`), que
partagent toutes les histoires sans clé `biomes` ; `parse` fabrique un lambda neuf (`:224`,
`:231`). Conséquence : deux lignes identiques **sans biome** fusionnent, deux lignes
identiques **avec le même biome** non.

**Rust (avant).** `Vec` sans déduplication : les deux cas gardaient deux lignes, ce qui change
les probabilités de `pickRandom`.

**Correction.** `Stories::insert` — `colony_story.rs:262` — reproduit exactement ça. Tests :
`two_identical_lines_without_a_biome_collapse_like_a_hash_set`,
`two_identical_lines_with_the_same_biome_both_stay`,
`a_biome_line_and_a_global_line_with_the_same_text_both_stay`.

## Divergences assumées, non corrigées

### D6 — l'ordre de `TEXTURE_PACKS`

Java remplit un `HashSet<String>` puis `new ArrayList<>(set)` : l'ordre est celui du hachage.
Le port **trie** (`texture_reload.rs:75-90`), parce que la liste est montrée dans un
sélecteur et qu'un ordre qui change entre deux lancements est un défaut visible. L'ensemble
est identique ; seul l'ordre diffère, et c'est documenté sur place.

### D7 — un `biomes` non textuel : même effet, autre nom

Java lit `getAsString()` sur la valeur (`:159-169`) puis résout l'id, qui jette s'il est
inconnu : le fichier est perdu. Le port refuse avec `Refusal::BadBiomes` au lieu de
`Refusal::UnknownBiome`. Même issue, motif nommé différemment — je ne l'ai pas aligné parce
que la variante dit mieux ce qui s'est passé.

### D8 — deux entrées que le Java ne peut pas produire

- `dyed_tooltip` sur une infobulle **vide** : Java lèverait `IndexOutOfBoundsException` sur
  `add(1, …)`. Le port insère à 0. Atteindre ce cas demande une infobulle sans nom, ce que
  le jeu ne construit pas.
- `texture_reload::pack_of` sur un chemin qui s'arrête après `citizen/` : Java lève
  `ArrayIndexOutOfBoundsException` sur `split("/")[3]`. Le port rend `Refusal::TooShort`.
  Atteindre ce cas demande un chemin contenant `png` et moins de quatre segments.

## Ce que je n'ai pas pu vérifier

1. **Aucune exécution.** Pas de compilation (consigne), donc les 5 corrections sont
   vérifiées par lecture et par test écrit, pas par exécution. Les tests ajoutés sont
   listés au-dessus, un par correction.
2. **Les règles qui vivent dans d'autres tranches.** `ColonyUtils.getOwningColony` (`0` pour
   non réclamé), `ColonyManager.onWorldLoad`, `PackageManager.addCloseSubscriber`,
   `EventManager.onTileEntityBreak` et les `BuildingModule` ne sont pas dans mon dossier :
   je les ai lus pour savoir ce que mes lignes supposent, je ne les ai pas audités.
3. **Les valeurs de configuration.** `mobAttackCitizens`, `allowOtherDimColonies`, `pvp_mode`,
   `citizenVoices`, `showdyetooltips`, `holidayFeatures` et `auditCraftingTags` sont lues par
   le Java ; le port les prend comme des faits d'entrée et ne connaît pas leurs valeurs par
   défaut (elles appartiennent aux fichiers de config, pas à mon dossier).
4. **Les traductions.** Les clés (`wrongfood`, `nomenu`, `needbetterfood`, `PERMISSION_OPEN_HUT`…)
   sont portées, les phrases non : elles vivent dans `assets/minecolonies/lang/`.
5. **Les 7 renderers de block entity et les 94 modèles** : je vérifie les **noms et les
   comptes** contre le Java (fait, mécaniquement), pas ce qu'ils dessinent.
