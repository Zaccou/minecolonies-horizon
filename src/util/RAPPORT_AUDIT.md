# Audit de fidélité — `src/util`

Relecture ciblée du port `src/util` contre les sources Java de `minecolonies-src`
(1.20.1). Méthode : ouvrir chaque méthode Java à sa ligne, refaire le calcul à la
main, puis comparer au Rust. Aucune compilation lancée (règle de la mission) ;
aucun `git` ; seuls les fichiers de `src/util` ont été touchés.

**Résultat en une ligne.** 40 méthodes auditées : **38 fidèles**, **1 écart de
comportement nommé** (`string_or`, documentation corrigée), **1 divergence réelle
corrigée** (`is_hut_path`, code), et **74 citations `fichier.java:ligne` fausses
ré-ancrées** sur 8 fichiers — elles pointaient vers une version des fichiers Java
qui n'est pas celle de l'arbre.

---

## 0. Où sont réellement les sources de cette tranche

La mission donne `minecolonies-src/api/util` comme point d'ancrage. Mesuré : sur
les 17 fichiers Java que citent mes 14 modules, **un seul groupe** vient de
`api/util` — `BlockPosUtil.java` (`findLand`, `:330-361`), `EntityUtils.java`
(`updateRotation`, `:113-128`) et les constantes (`Constants.java`,
`CitizenConstants.java`, `SchematicTagConstants.java`, `ColonyManagerConstants.java`).
Tout le reste vient de `core/util/*` et de `core/util/citizenutils/`, plus
`CitizenSkillHandler.java` pour la boucle d'XP. C'est cette table-là que j'ai
auditée ; le chemin `api/util` seul n'aurait couvert que trois méthodes.

---

## 1. Les 40 méthodes auditées

Verdicts : **fidèle** = même règle, mêmes bornes, même ordre ; **écart nommé** =
comportement différent, voulu, écrit dans le code ; **divergent** = écart non voulu
(à corriger).

| # | Méthode Java | Ligne Java | Ici | Ligne Rust | Verdict |
| --: | --- | --- | --- | --: | --- |
| 1 | `WorkerUtil.getBestToolForBlock` | `core/util/WorkerUtil.java:142-171` | `worker.rs` | 123 | fidèle |
| 2 | `WorkerUtil.getCorrectHarvestLevelForBlock` | `WorkerUtil.java:179-198` | `worker.rs` | 164 | fidèle |
| 3 | `WorkerUtil.getLastLadder` | `WorkerUtil.java:340-350` | `worker.rs` | 196 | fidèle (borne `MAX_LADDER_SCAN` propre au port, écrite) |
| 4 | `WorkerUtil.faceBlock` (yaw/pitch) | `WorkerUtil.java:213-219` | `worker.rs` | 232 | fidèle |
| 5 | `EntityUtils.updateRotation` | `api/util/EntityUtils.java:113-128` | `worker.rs` | 309 | fidèle |
| 6 | `WorkerUtil.hasTooManyExternalItemsInInv` | `WorkerUtil.java:360-376` | `worker.rs` | 339 | fidèle (`> 3`, pas `>= 3`) |
| 7 | `SortingUtils.sort` (fusion des piles) | `core/util/SortingUtils.java:46-62` | `sorting.rs` | 85 | fidèle (`BTreeMap` au lieu de `HashMap` : ordre rendu déterministe, nommé) |
| 8 | `SortingUtils.calcRequiredSlots` (ceil) | `SortingUtils.java:186-188` | `sorting.rs` | 109 | fidèle |
| 9 | `SortingUtils.compare` | `SortingUtils.java:143-161` | `sorting.rs` | 133 | fidèle (onglet, id, dégâts — dégâts seulement si id égal) |
| 10 | `MutableChunkPos(BlockPos)` | `core/util/MutableChunkPos.java:19-24` | `chunkpos.rs` | 68 | fidèle (`>> 4`) |
| 11 | `getMinBlockX/Z` | `MutableChunkPos.java:68-77` | `chunkpos.rs` | 116 | fidèle (`<< 4`) |
| 12 | `getRegionLocalX/Z` | `MutableChunkPos.java:103-113` | `chunkpos.rs` | 149 | fidèle (`& 31`) |
| 13 | `hashCode` | `MutableChunkPos.java:39-45` | `chunkpos.rs` | 190 | fidèle (5 valeurs recalculées hors Rust, LCG inclus) |
| 14 | `AttributeModifierUtils.removeAllHealthModifiers` | `core/util/AttributeModifierUtils.java:18-34` | `attributes.rs` | 66 | fidèle |
| 15 | `removeHealthModifier` | `AttributeModifierUtils.java:42-60` | `attributes.rs` | 81 | fidèle (tous les homonymes, pas le premier) |
| 16 | `addHealthModifier` | `AttributeModifierUtils.java:68-81` | `attributes.rs` | 125 | fidèle (ratio de vie conservé, retrait **avant** ajout) |
| 17 | `ServerUtils.getPlayerFromUUID(UUID, Level)` | `core/util/ServerUtils.java:134-149` | `server.rs` | 68 | fidèle |
| 18 | `ServerUtils.getPlayersFromUUID` | `ServerUtils.java:57-81` | `server.rs` | 97 | fidèle (retour anticipé omis, même ensemble) |
| 19 | `AdvancementUtils.TriggerAdvancementPlayersForColony` | `core/util/AdvancementUtils.java:15-31` | `server.rs` | 139 | fidèle (managers **et** en ligne) |
| 20 | `FurnaceRecipes.loadRecipes` | `core/util/FurnaceRecipes.java:37-66` | `furnace.rs` | 107 | fidèle (exactement 1 ingrédient, chaque variante une entrée) |
| 21 | `FurnaceRecipes.getSmeltingResult` | `FurnaceRecipes.java:69-77` | `furnace.rs` | 147 | fidèle (`EMPTY` = `None`) |
| 22 | `getFirstSmeltingRecipeByResult` | `FurnaceRecipes.java:81-84` | `furnace.rs` | 160 | fidèle (« first » = dernier écrit, nommé) |
| 23 | `BuildingUtils.getAllowedJobs` | `core/util/BuildingUtils.java:79-94` | `building.rs` | 111 | fidèle (y compris le tag `job=` vide qui interdit tout) |
| 24 | `BuildingUtils.getItemStackForHutFromInventory` | `BuildingUtils.java:54-66` | `building.rs` | 137 + 183 | **divergent → corrigé** (§2.1) |
| 25 | `getRotationFromBlueprint` | `BuildingUtils.java:145-158` | `building.rs` | 269 | fidèle (`(4 + monde - plan) % 4`) |
| 26 | `getRotationFromBlock` | `BuildingUtils.java:181-192` | `building.rs` | 223 | fidèle (`get2DDataValue`, `-1` → `None`) |
| 27 | `canAutoHire` | `BuildingUtils.java:109-116` | `building.rs` | 324 | fidèle (précédence `&&` / `\|\|`, `Auto` explicite) |
| 28 | `GsonHelper.getAsString` | `core/util/GsonHelper.java:16-24` | `json.rs` | 97 | écart nommé (§2.2) |
| 29 | `getAsResourceLocation` | `GsonHelper.java:26-34` | `json.rs` | 139 | fidèle (id invalide = `Err`, pas de panique) |
| 30 | `getAsJsonArray` | `GsonHelper.java:36-44` | `json.rs` | 172 | fidèle |
| 31 | `TeleportHelper.teleportCitizen` / `WorkerUtil.setSpawnPoint` | `core/util/TeleportHelper.java:56-61`, `WorkerUtil.java:125-130` | `teleport.rs` | 59 | fidèle (`+0.5` en x/z, `y` du bloc) |
| 32 | `BlockPosUtil.findLand` | `api/util/BlockPosUtil.java:330-361` | `teleport.rs` | 117 | fidèle (recherche binaire, dernier sondage solide → `+1`) |
| 33 | `TeleportHelper.colonyTeleport` (ancres + refus) | `TeleportHelper.java:142-153`, `:157-166` | `teleport.rs` | 161, 178 | fidèle (`pos` → hôtel de ville → centre ; échec = pas de TP) |
| 34 | `SchemAnalyzerUtil.getBlockTier` | `core/util/SchemAnalyzerUtil.java:32-65` | `schem.rs` | 167 | fidèle (plus haut tag gagnant, sinon 0) |
| 35 | `SchemAnalyzerUtil.getScoreFor` | `SchemAnalyzerUtil.java:73-120` | `schem.rs` | 201 | fidèle (ordre de la chaîne `else if`, `min(27)` sur 3 branches) |
| 36 | `SchemAnalyzerUtil.analyzeSchematic` | `SchemAnalyzerUtil.java:128-199` | `schem.rs` | 269 | fidèle (500 / 40 / volume÷10, `(int)` tronque) |
| 37 | `BackUpHelper.backupColonyData` (seuil) | `core/util/BackUpHelper.java:81-85` | `backup.rs` | 71 | fidèle (`>=` : 5 min pile autorise) |
| 38 | `BackUpHelper` (rétention) | `BackUpHelper.java:141-161` | `backup.rs` | 115 | fidèle (tri stable par `lastModified`, les plus vieux d'abord) |
| 39 | `BackUpHelper.getFolderForDimension` | `BackUpHelper.java:400-403` | `backup.rs` | 145 | fidèle (`namespace/path/`) |
| 40 | `CreativeRaiderStructureHandler.triggerSuccess` | `core/util/CreativeRaiderStructureHandler.java:143-167` | `raider.rs` | 95 | fidèle (or **puis** tag, premier tag qui gagne, `map == null` coupe tout) |

**Vérifiées aussi, non tabulées** (pour ne pas dépasser l'échantillon demandé) :
`TEST_TOOL_ORDER` (`WorkerUtil.java:87-98` → `worker.rs:97`), `minimal_move`
(`:223-224` → `:254`), `Mth.wrapDegrees` → `wrap_degrees` (`worker.rs:283`),
`slots_per_tab` / `required_slot_total` (`SortingUtils.java:182-192` →
`sorting.rs:161/143`), `MutableChunkPos(long)` / `toLong` / `getBlockX(Z)` /
`getRegionX(Z)` / `getChessboardDistance` (`:26-31`, `:33-37`, `:79-89`,
`:91-101`, `:121-125` → `chunkpos.rs:78/110/132/143/160`), les deux autres
`AttributeModifierUtils` (`:89-103`, `:111-120` → `attributes.rs:148/163`),
`getPlayersFromPermPlayer` (`ServerUtils.java:95-105` → `server.rs:111`),
`blueprint_path_fallback` (`BuildingUtils.java:139-142` → `building.rs:294`),
`dimension_folder`/`export_filename`/`deletions_needed` (`BackUpHelper.java`
`:400-403`, `:51`+`:496`, `:141-148` → `backup.rs:145/130/100`),
`is_excluded` (`SchemAnalyzerUtil.java:207-210` → `schem.rs:73`),
`residence_rename`/`component_corners` (`SchemFixerUtil.java:127-131`,
`:135-138` → `schem.rs:308/333`), et les trois règles de `CitizenItemUtils`
(`:302-312`, `:223`/`:302`, `:325-356` → `citizen_item.rs:73/96/149`), plus
`ExperienceUtils` (`:32-44`, `:85-93` → `experience.rs:111/146`).

---

## 2. Divergences trouvées

### 2.1 `is_hut_path` : la préfixe n'est pas l'`instanceof` — **corrigé**

`BuildingUtils.getItemStackForHutFromInventory` (`BuildingUtils.java:54-66`) teste
trois choses : le bloc est un `BlockItem`, son bloc est un
**`instanceof AbstractBlockHut`** (`:57`), et son chemin de registre
`endsWith(hut)` (`:57-59`). Le port remplaçait le test de type par « le chemin
commence par `blockhut` », et le commentaire affirmait que c'était exact :

> « the prefix *is* the `instanceof`, not an approximation of it »

C'est faux dans **les deux sens**, et le arbre le prouve :

| Fait mesuré | Preuve |
| --- | --- |
| `minecolonies:blockhutfield` **n'est pas** un hut | c'est l'épouvantail, `BlockScarecrow extends AbstractBlockMinecoloniesDefault` (`core/blocks/BlockScarecrow.java:52`), et cette classe de base enregistre le bloc sous `REGISTRY_NAME = "blockhutfield"` (`api/blocks/huts/AbstractBlockMinecoloniesDefault.java:44`) |
| `largequarry` **est** un hut sans préfixe | `LargeQuarry extends AbstractBlockHut` (`core/blocks/huts/LargeQuarry.java:10`), `getHutName()` → `"largequarry"` (`:22`) |
| `mediumquarry`, `simplequarry` idem | `MediumQuarry.java:11` + `ModBuildings.MEDIUM_QUARRY_ID = "mediumquarry"` (`api/colony/buildings/ModBuildings.java:59`), `SimpleQuarry.java:11` + `ModBuildings.java:58` |
| 50 des 53 huts de `core/blocks/huts` portent bien le préfixe | 48 `return "blockhut…"` + les constantes `blockhutstable` / `blockhuttavern` |
| la postbox et le stash ne portent pas le préfixe et ne sont pas des huts | `BlockPostBox.java:31` (`blockpostbox`), `BlockStash.java:45` (`blockstash`) |

Correction (`building.rs`), avec les quatre exceptions nommées et sourcées :

```rust
pub const HUT_PREFIX: &str = "blockhut";
pub const PREFIXED_NON_HUTS: [&str; 1] = ["blockhutfield"];
pub const HUTS_WITHOUT_THE_PREFIX: [&str; 3] = ["largequarry", "mediumquarry", "simplequarry"];

pub fn is_hut_path(path: &str) -> bool {
    if HUTS_WITHOUT_THE_PREFIX.contains(&path) { return true; }
    path.starts_with(HUT_PREFIX) && !PREFIXED_NON_HUTS.contains(&path)
}
```

Nouveau test `the_prefix_has_four_named_exceptions_and_they_go_both_ways`, et
commentaire qui dit désormais que c'est une **approximation** de
`instanceof AbstractBlockHut` (`BuildingUtils.java:57`), avec ses quatre trous
mesurés. L'ancien paragraphe citait `blocks/huts.rs:39-44` et comptait « 50
`blockhut*` + 4 non-huts dont les deux quarries » : doublement faux (les quarries
**sont** des huts, et l'épouvantail n'y figurait pas).

Impact réel : sans les exceptions, une recherche d'outil pour `"quarry"` ne
trouvait pas la pierre de taille du quarry dans l'inventaire, et `"field"`
ramenait l'épouvantail.

### 2.2 `string_or` : un `null` JSON n'est pas un champ absent — **documentation corrigée**

`GsonHelper.getAsString` (`GsonHelper.java:21-24`) fait
`object.has(key) ? convertToString(object.get(key), key) : default.get()`.
`JsonObject.has` est **vrai** pour une valeur `null` (il teste le membre), et
vanilla `convertToString` lève une `JsonSyntaxException` pour tout ce qui n'est
pas un `JsonPrimitive` — `JsonNull` n'en est pas un. Le port, lui, répond le
défaut pour `Some(Value::Null)`, ce qui est cohérent avec sa politique (« une
forme inattendue → le défaut »), mais le commentaire affirmait que **le seul**
écart était « objet ou tableau ». Corrigé : les trois formes (`null`, tableau,
objet) sont nommées, avec `GsonHelper.java:23` cité, et `identifier_or` dit
maintenant que le `null` de `getAsString(object, key, (String) null)` est
l'**argument** par défaut, pas une valeur `null` dans le fichier.

### 2.3 Les citations ne pointaient pas sur cet arbre — **74 ré-ancrées**

Signal d'alarme : `chunkpos.rs` citait `MutableChunkPos.java:270-425` pour un
fichier de **162** lignes. Mesure ensuite systématique (script de contrôle : tout
`fichier.java:NNN` dont `NNN` dépasse la longueur réelle du fichier) : les
décalages ne sont pas constants d'un fichier à l'autre (`MutableChunkPos` +263,
`FurnaceRecipes` +287, `AttributeModifierUtils` +179, `GsonHelper` +150,
`SchemFixerUtil` +113) — donc pas une simple erreur d'offset, mais une
**version différente** des sources. Les fichiers de `worker.rs`, `sorting.rs`,
`citizen_item.rs`, `experience.rs`, `teleport.rs`, `raider.rs` et
`server.rs` (sauf `AdvancementUtils`) étaient déjà justes.

| Fichier Rust | Citations ré-ancrées | Exemple avant → après |
| --- | --: | --- |
| `chunkpos.rs` | 16 | `MutableChunkPos(int, int)` `` `:275-280` `` → `` `:12-17` `` |
| `furnace.rs` | 13 | `loadRecipes` `` `:324-353` `` → `` `:37-66` `` |
| `attributes.rs` | 12 | `addHealthModifier` `` `:247-260` `` → `` `:68-81` `` |
| `json.rs` | 11 | `getAsResourceLocation` `GsonHelper.java:176-184` → `:26-34` |
| `building.rs` | 11 | fallback `0.blueprint` `` `:133-136` `` → `` `:139-142` `` |
| `backup.rs` | 5 | filtre de rétention `BackUpHelper.java:127` → `:133` |
| `schem.rs` | 3 | `fixSchematicNameAndCorners` `SchemFixerUtil.java:240-245` → `:127-131` |
| `server.rs` | 3 | `TriggerAdvancementPlayersForColony` `` `:29-45` `` → `` `:15-31` `` |

Contrôle après coup : plus aucune citation ne dépasse la longueur de son fichier
Java (les trois restantes sont des chemins Rust — `ui/action.rs:250`,
`network/permissions.rs:385`, `identifier.rs:56-67` — et une ligne
d'`ExperienceUtils`).

---

## 3. Écarts trouvés, **non** corrigés (et pourquoi)

| Écart | Où | Pourquoi il reste |
| --- | --- | --- |
| les `null` de `getPlayersFromPermPlayer` sont jetés | `server.rs:111` ← `ServerUtils.java:95-105` | Java renvoie une liste trouée, bug assumé ; le port le dit au lieu de le copier |
| `sort` itère un `HashMap` (ordre arbitraire), le port un `BTreeMap` | `sorting.rs:85` ← `SortingUtils.java:46` | l'ordre des doublons devient déterministe ; `compare` tranche déjà tous les ex æquo, donc pas d'écart d'affichage |
| `calculate_level` refuse `xp > cost(99)` et les non-finis | `experience.rs:146` ← `ExperienceUtils.java:85-93` | Java boucle sans fin (`inf`) ou dépasse le plafond de niveau ; refus nommé |
| `addHealthModifier` ne modélise pas le `Mth.clamp(0, max)` de `LivingEntity.setHealth` | `attributes.rs:125` ← `AttributeModifierUtils.java:80` | le clamp est vanilla, pas dans `AttributeModifierUtils` ; visible seulement si `vie > max` avant l'ajout. Écrit dans le commentaire |
| `pickup_pitch` rend un `f64`, Java un `(float)` | `citizen_item.rs:113` ← `CitizenItemUtils.java:66` | la règle (Gauss ×0.7 + 1, ×2) est fidèle ; l'arrondi `f32` appartient au paquet son |
| `string_or` sur un nombre garde `serde_json` (`.to_string()`), Gson garde le texte source | `json.rs:97` ← `GsonHelper.java:23` | `"1e9"` devient `"1000000000.0"` ; atteignable seulement par un id numérique en notation scientifique |

---

## 4. Ce que je n'ai pas pu vérifier

1. **Vanilla 1.20.1**, absent de l'arbre : `Mth.wrapDegrees` (`worker.rs:283`),
   `GsonHelper.convertToString` (sémantique du `null`, §2.2),
   `AttributeInstance.getModifiers()` « copie immuable » (affirmation du
   commentaire d'`attributes.rs`), `LivingEntity.setHealth` et son clamp,
   `ResourceLocation(String)` (refus du chemin vide, `json.rs:139`),
   `SectionPos.sectionToBlockCoord` et `Direction.BY_2D_DATA`.
2. **Structurize** : `ModBlocks.java:65` (`blocksolidsubstitution`, cité par
   `schem.rs:44`), `IBlueprintDataProviderBE` (tags d'ancre, `building.rs:111`) —
   pas dans `minecolonies-src`.
3. **Domum Ornamentum** : la branche DO de `getScoreFor`
   (`SchemAnalyzerUtil.java:145-164`, `max(2, somme/3)`) reste dans
   `MANQUANT.md` §1 ; je n'ai donc pas pu auditer `score_for` sur ce cas.
4. **Aucune exécution** : la mission interdit de compiler, donc les deux
   corrections (§2.1, §2.2) sont revues à la main, pas testées. Les tests ajoutés
   sont écrits pour passer sur les valeurs calculées (§2.1) et ne changent aucune
   attente existante ; `cargo test` reste à lancer par l'orchestrateur.
5. **Comptage des huts** : 53 classes concrètes `extends AbstractBlockHut` dans
   `core/blocks/huts` (55 fichiers moins `BlockPostBox` et `BlockStash`), dont 50
   nommées `blockhut*` et 3 quarries. C'est un comptage par `grep` sur les
   `getHutName()`, pas une exécution du registre Forge.
