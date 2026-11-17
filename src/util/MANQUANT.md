# MANQUANT — `src/util`

Ce que les 23 fichiers de `core/util` demandent et que le port ne peut pas écrire
ici : une API absente, un type Forge, ou un handle que le `Host` ne donne pas. Pour
chaque entrée : **ce que je voulais faire**, et **ce qu'il faudrait pour le faire**.

Rien de ce qui suit n'est écrit en code fictif. Ce qui *a* été porté est dans les
modules voisins, et la ligne dit où.

---

## 1. Domum Ornamentum : `IMateriallyTexturedBlock`, `MaterialTextureData`

**Fichiers :** `DomumOrnamentumUtils.java` (114 lignes) en entier, plus trois
branches qui en dépendent :
`SchemAnalyzerUtil.analyzeSchematic` (`:145-164`) et
`WorkerUtil.getBestToolForBlock` (`:155-161`).

**Ce que je voulais faire.** `DomumOrnamentumUtils` est fait de quatre lectures :

```java
stack.getItem() instanceof BlockItem bi && bi.getBlock() instanceof IMateriallyTexturedBlock doBlock
block instanceof IDOBlock
stack.getOrCreateTag().getCompound("textureData") -> MaterialTextureData.deserializeFromNBT(tag)
```

et `SchemAnalyzerUtil` s'en sert pour chiffrer un bloc DO :

```java
for (final Block doBlockPart : textureData.getTexturedComponents().values()) {
    doComplexity += getScoreFor(doBlockPart);
}
blockComplexity = Math.max(2, doComplexity / 3);   // :158
```

**Pourquoi ce n'est pas ici.** `IMateriallyTexturedBlock`, `IDOBlock` et
`MaterialTextureData` sont des types de Domum Ornamentum, un **autre mod**. Le SDK
pinné n'exporte aucun type d'un autre mod : `mod_api` décrit des blocs, des états,
des entités de bloc et des piles d'objets, pas l'interface d'un voisin
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/`). Les écrire demanderait d'inventer
les trois types — exactement ce que la règle 4 interdit.

**Ce qu'il faudrait.** Deux choses, dans cet ordre :

1. que Domum Ornamentum expose ses données de texture comme **données** et non
   comme type Java — c'est-à-dire que `textureData` soit un `Compound` que le port
   sait déjà lire (`crate::blueprint::nbt`) avec une convention stable
   (`{ "type": "do", "parts": { "0": "minecraft:stone", ... } }`). Le contenu
   `port-domum` (53 blocs, 363 fichiers) prouve déjà que le port lit ces compounds ;
2. alors `getScoreFor` s'applique tel quel aux blocs des parties, et la formule
   `max(2, somme / 3)` est une ligne.

En attendant, `schem::Cell::score` est **fourni par l'appelant** : un port DO peut
passer l'estimation qu'il veut, et `SchemAnalyzerUtil` lui dit laquelle employer.

---

## 2. Forge : `TierSortingRegistry`, `IForgeShearable`, `Tags.Blocks.*`

**Fichiers :** `WorkerUtil.getBestToolForBlock` (`:142-171`),
`WorkerUtil.getCorrectHarvestLevelForBlock` (`:179-198`),
`SchemAnalyzerUtil.getScoreFor` (`:79-117`).

**Ce que je voulais faire.** Trois tables de tags :

| Java | Ce que c'est |
| --- | --- |
| `TierSortingRegistry.getSortedTiers()` | l'ordre **total** des niveaux de pioche de Forge, avec leur tag de bloc |
| `IForgeShearable` | une interface **Forge** sur un bloc, pas un tag |
| `Tags.Blocks.STORAGE_BLOCKS`, `Tags.Blocks.BOOKSHELVES` | deux tags du namespace `forge:` |

**Pourquoi ce n'est pas ici.** `minecolonies-src` ne contient ni Forge ni vanilla :
ces trois-là sont hors du dépôt de référence, et le port n'a **aucune** table de
tags `forge:` — c'est déjà écrit dans `MANQUANT.md` à la racine du dépôt : « `forge:`
est le namespace de tags de Forge ; Horizon 26.3 n'en définit aucun (ses conventions
sont en `c:`) ».

**Ce qu'il faudrait.** Le pendant est déjà décidé pour les recettes : les tags
`forge:` demandent un tag `c:` équivalent ou une autre source. Pour ces trois-là :
`c:storage_blocks`, `c:bookshelves`, et l'ordre des tiers, qui est **une constante
du jeu** dans 26.3 (bois, pierre, fer, diamant, netherite) et peut donc vivre dans
`base::constants` avec ses cinq lignes de source vanilla.

Ce qui **est** porté sans eux :

- `worker::correct_harvest_level(required, glazed)` — les deux corrections, une fois
  le tag résolu ;
- `worker::best_tool(hardness, shears, correct)` — l'ordre hoe/shovel/axe/pickaxe et
  les deux gardes (shears d'abord, dureté 0 court-circuite) ;
- `schem::family(...)` — la chaîne `else if` complète, en tant que valeur, et
  `schem::score_for(tier, famille)`, le calcul entier.

---

## 3. `BuildingUtils.commonBuildingFromPosition` et `getItemStackForHutFromInventory`

**Fichier :** `BuildingUtils.java:54-71`, `:201-211`.

**Ce que je voulais faire.** Les deux seuls méthodes de `BuildingUtils` qui restent :

```java
IColony colony = IColonyManager.getInstance().getIColony(level, pos);
return colony.getCommonBuildingManager().getBuilding(pos);        // :205-208
InventoryUtils.findFirstSlotInProviderNotEmptyWith(inventory.player, item -> ...);  // :55-60
```

**Pourquoi ce n'est pas ici.** La première a besoin du **registre de colonies** —
`IColonyManager.getInstance()`, un singleton serveur — et du gestionnaire de
bâtiments d'une colonie vivante. Le port a les colonies comme **données**
(`crate::colony`), sans singleton : la question « quelle colonie possède ce bloc »
est `crate::base::claims::ChunkClaims`, et « quel bâtiment est ici » est une lecture
dans le `BTreeMap` d'une colonie que l'appelant tient déjà. Une fonction ici
demanderait le registre en paramètre pour ne rien décider de plus.

La seconde est un balayage de slots `net.minecraft.world.entity.player.Inventory`
(37 emplacements, main + armure + off-hand) et son unique règle est
`path.endsWith(hut)` — c'est [`crate::util::building::hut_path_matches`], et
`is_hut_path` pour l'autre moitié de la condition.

---

## 4. `ChunkDataHelper` : les deux fonctions qui ont besoin d'une capability

**Fichier :** `ChunkDataHelper.java` (476 lignes), déjà porté **en grande partie**
dans `crate::base` — voir la note de `util/chunkpos.rs`. Restent deux méthodes :

```java
public static void loadChunk(final LevelChunk chunk, final Level world)      // :52-85
public static void unloadChunk(final LevelChunk chunk, final Level world)    // :92-106
```

**Ce que je voulais faire.** `loadChunk` récupère `CHUNK_STORAGE_UPDATE_CAP` et
`COLONY_MANAGER_CAP` (deux **capabilities Forge**) et applique les
`ChunkLoadStorage` en attente au chunk qui vient d'être chargé ; `unloadChunk`
retire le chunk du livre `loadedChunks` de sa colonie.

**Pourquoi ce n'est pas ici.** Les capabilities Forge n'existent pas dans l'API.
Le port a l'équivalent : une `AttachmentType` posée sur un chunk
(`docs/modding/src/concepts/attachments.md`), et c'est **elle** que le port
utiliserait. Mais un `AttachmentType` se déclare à l'enregistrement et se lit par
le `Host` : la traduction de `ChunkLoadStorage.applyToCap` demande d'écrire ce
storage en premier, et ce storage (`:1-120` de `ChunkLoadStorage.java`) n'est pas
dans cette tranche.

**Ce qu'il faudrait.** Un `AttachmentType<ChunkClaims>` déclaré dans
`Hooks::register`, puis `loadChunk` devient une écriture d'attachment et
`unloadChunk` une lecture. `ChunkLoadStorage` lui-même
(`api/util/ChunkLoadStorage.java`, 353 lignes) est un simple compound de revendications
en attente — il serait portable, mais il n'a de sens qu'une fois la capability
remplacée. C'est un chantier de `colony_world`, pas d'utilitaire.

---

## 5. `BlockPosUtil.findAround` (et `EntityUtils.checkValidSpawn`)

**Fichiers :** `api/util/BlockPosUtil.java:1035-1112` (`findAround`),
`api/util/EntityUtils.java:203-260` environ (`checkValidSpawn`, appelé à
`EntityUtils.java:178` et `:183`), `EntityUtils.java:175-195` (`getSpawnPoint`).

**Ce que je voulais faire.** `TeleportHelper.colonyTeleport` (`:157-166`) et
`teleportCitizen` (`:43-47`) ne peuvent pas atterrir sans eux :

```java
position = BlockPosUtil.findAround(world, position, 5, 5, (w, p) -> p.getBlockState().isAir() && ...above().isAir());
final BlockPos spawnPoint = EntityUtils.getSpawnPoint(world, location);
```

`findAround` est une spirale de 80 lignes : le point de départ, puis les quatre
voisins horizontaux, puis des carrés de rayon croissant, `verticalRange + 2` fois,
avec un `y` qui alterne 0, +1, -1, +2, -2.

**Pourquoi ce n'est pas ici.** Ce n'est pas un fichier de cette mission — c'est
`api/util/BlockPosUtil.java`, et `base` est le module qui possède déjà ses distances
(`base/geom.rs` cite ses lignes). Le porter ici en aurait fait une **deuxième**
maison pour `BlockPosUtil`, ce que la règle 6 interdit.

**Ce qu'il faudrait.** Le poser dans `base/geom.rs` (ou un `base/search.rs` à côté),
avec un prédicat `impl Fn([i32; 3]) -> bool`. À ce moment-là,
`util::teleport::colony_teleport_target` n'a plus besoin de son callback : la
signature est déjà celle de la spirale, `impl FnOnce([i32;3]) -> Option<[i32;3]>`.

Ce qui **est** porté sans elle : l'ordre des trois ancres (préféré → hôtel de
ville → centre), le refus quand la recherche échoue, et
`BlockPosUtil.findLand` (`:330-361`) en entier, parce que c'est une recherche
binaire pure et que `surfaceTeleport` n'existe pas sans elle.

---

## 6. `MultimapCollector` et `CollectorUtils.toShuffledList`

**Fichiers :** `MultimapCollector.java` (68 lignes), `CollectorUtils.java` (30).

**Ce que je voulais faire, et où c'est déjà.**

| Java | Équivalent |
| --- | --- |
| `toMultimap(keyGetter, valueGetter)` | `util::sorting::group` — **trié**, pas dans l'ordre d'insertion |
| `toShuffledList()` | un `shuffle` en fin de flux |

Deux appels réels existent en Java : `MobAIRegistry.java:64` et `:122` pour le
multimap, `PercentageHarvestPlantModule.java:132` et `TreeSidePlantModule.java:169`
pour la liste mélangée. **Aucun des deux domaines n'est porté** (l'IA des mobs et la
plantation), donc il n'y a pas encore de code pour les recevoir — mais la différence
compte :

`ArrayListMultimap` **préserve l'ordre d'insertion**, et `MobAIRegistry` s'en sert
comme d'une **priorité de tâches** : deux tâches du même type doivent sortir dans
l'ordre où elles ont été enregistrées. `sorting::group` trie par clé, ce qui est
juste pour un entrepôt et **faux** pour ce registre. Quand l'IA des mobs sera portée,
il faudra le variant ordonné :

```rust
pub fn group_ordered<K, V>(stacks: impl IntoIterator<Item = (K, V)>) -> Vec<(K, Vec<V>)>
```

Cinq lignes avec un `HashMap<K, usize>` d'index vers un `Vec`. Il n'est pas écrit
maintenant parce que personne ne l'appelle, et un helper sans appelant est du code
à relire sans raison.

`toShuffledList` est, lui, **possible** : `mod_api` réexporte
`horizon_core::random::LegacyRandom`
(`horizon-mod-vocab/src/exports/horizon_core.rs:1`), donc `Vec::shuffle` avec le RNG
du jeu est une fonction de trois lignes. `Collections.shuffle` et le RNG de vanilla
ne tirent pas les mêmes nombres dans le même ordre, mais la seule exigence du Java
est « dans un ordre différent à chaque tick », pas la parité — c'est écrit ici pour
que la décision soit explicite le jour où la plantation arrive.

---

## 7. `RecipeHandler.init`

**Fichier :** `RecipeHandler.java:14-16` — **il n'y a rien dedans** :

```java
public static void init(final boolean enableInDevelopmentFeatures, final boolean supplyChests) {
    // Register the hust
    //todo disable indev features
    //todo disable supplychest if config
}
```

Deux paramètres, zéro ligne, deux `todo` qui datent. Il n'y a pas de règle à porter
et `MANQUANT.md` à la racine du dépôt dit déjà que les recettes sont dans
`data/minecolonies/recipe/` (154 fichiers, convertis par `convertir_recettes.py`).
Rien à faire ici, ni maintenant ni plus tard.

---

## 8. `ChunkClientDataHelper` — client seulement

**Fichier :** `ChunkClientDataHelper.java` (77 lignes).

Il applique côté **client** les `UpdateChunkCapabilityMessage` reçus (`:1-77`), et
il n'est appelé que depuis un gestionnaire de paquets client. Le port est un mod
**wasm côté serveur** (`lib.rs:16-20`) : il n'y a pas de côté client à mettre à jour,
et une capability de chunk n'y existe pas (voir §4). Aucun code à écrire.

---

## 9. Ce que `BackUpHelper` fait et qu'aucun port ne fera

**Fichier :** `BackUpHelper.java` (640 lignes). La **politique** est portée
(`util::backup` : le seuil de 5 minutes, les 20 conservés, le nom des fichiers, le
dossier de dimension). Le reste ne peut pas l'être, et deux choses méritent d'être
écrites noir sur blanc :

1. **`backupColonyData` modifie la sauvegarde qu'il sauvegarde.** Pour chaque colonie
   dont le manager est absent, il appelle `markColonyDeleted(i, dimension)`
   (`:107-111`), ce qui renomme `colony<i>.dat` en `colony<i>.dat_deleted` **avant**
   de zipper. C'est intentionnel (le zip doit porter le bon nom) et c'est un effet de
   bord d'une opération de sauvegarde. Un port qui le reproduirait sans le dire
   ferait la même chose en silence ; il est donc documenté et **pas** reproduit.
2. **`lastBackupTime` est posé avant le travail** (`:85`), donc une sauvegarde qui
   lève à mi-chemin consomme quand même la fenêtre de cinq minutes. `backup_due` est
   une fonction pure sur deux horodatages : c'est à l'appelant de décider quand il
   pose le sien, et la doc dit que le port pose après un échec.

Le reste — `ZipOutputStream`, `NbtIo.readCompressed`, `ServerLifecycleHooks`, la
marche de répertoire — demande un système de fichiers et un serveur. Un mod wasm n'a
ni l'un ni l'autre.

---

## 10. Ce que j'ai trouvé faux dans le Java, et qui est porté tel quel

Ce ne sont pas des manques mais des **défauts**, et ils sont dans le code porté pour
que `diff` contre le Java reste possible. Chacun est verrouillé par un test.

| Où | Quoi | Test qui le verrouille |
| --- | --- | --- |
| `ExperienceUtils.java:32-44`, `:85-93` | les deux fonctions sont **du code mort** (2 sites d'appel dans tout l'arbre, tous deux `getXPNeededForNextLevel`) **et** lisent `experience` comme un cumul alors que `CitizenSkillHandler` y stocke l'XP *dans* le niveau | `calculate_level_disagrees_with_the_curve_it_inverts`, `a_negative_level_makes_the_percentage_nan_not_a_hundred` |
| `SortingUtils.java:366-371` | `getFirstSmeltingRecipeByResult`… est dans `FurnaceRecipes` (`:366-371`) et rend la **dernière** recette, pas la première : c'est un `Map.put` | `a_duplicate_output_answers_the_last_recipe_despite_the_name` |
| `FurnaceRecipes.java:328-352` | `recipeManager.byType(...).values()` est un `HashMap` : deux recettes qui partagent une entrée donnent un gagnant **arbitraire** | `a_duplicate_input_keeps_the_last_recipe` (le port choisit « la dernière », déterministe) |
| `BuildingUtils.java:82-91` | un tag `job=` **vide** rend `jobTags` non vide et interdit donc **tous** les métiers | `an_empty_job_tag_forbids_every_job_the_way_java_does` |
| `ServerUtils.java:95-105` | `getPlayersFromPermPlayer` ajoute `null` pour chaque joueur hors ligne | `an_offline_member_is_dropped_rather_than_kept_as_a_hole` |
| `BlockPosUtil.java:355-360` | `findLand` sur un `y` négatif laisse `foundland` nul et jette une `NullPointerException` | `a_negative_start_is_refused` |
| `AttributeModifierUtils.java:221-239` et `:268-282` | deux copies de la même suppression, l'une avec `MAX_HEALTH` en dur | `the_two_named_removals_agree` |
