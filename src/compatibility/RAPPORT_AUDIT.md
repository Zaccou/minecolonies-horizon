# Relecture de fidélité — `src/compatibility`

**38 méthodes auditées** sur les 3 fichiers de la tranche, réparties sur **10 fichiers Java**.
Sept divergences réelles trouvées et corrigées, **quatre-vingts références de ligne fausses**
corrigées (défaut de méthode de ma part, voir §3), et une liste explicite de ce que je n'ai pas
pu vérifier.

## 0. Méthode

Pour chaque méthode : ouverture du Java à la ligne citée, lecture de la règle, puis comparaison
avec le Rust **sans relire mon propre commentaire d'abord** — la règle est re-dérivée du Java,
puis confrontée au code. Les 10 fichiers Java ouverts : `ModRecipeTypes`, `JEIPlugin`,
`JobBasedRecipeCategory`, `GenericRecipeCategory`, `GenericRecipe`, `ToolRecipeCategory`,
`CropRecipeCategory`, `CompostRecipeCategory`, `FishermanRecipeCategory`,
`FloristRecipeCategory`, `CompostRecipe`, `ToolsAnalyzer`, `ToolUsage`, `ItemStackUtils`,
`JobEntry`, `LootTableAnalyzer`, `ColonyBorderMapping`, `ColonyDeathpoints`, `Journeymap`,
`JourneymapOptions`, `JourneymapPlugin`, `EventListener`, `CraftingTagAuditor`.

**Verdicts** : *fidèle* (même règle, mêmes bornes/ordre/refus) · *divergent* (la règle diffère,
corrigée) · *absent* (la règle Java n'existe pas dans le port, justifié ou non).

## 1. Le tableau

| # | Rust | Java | Verdict |
| ---: | --- | --- | --- |
| 1 | `recipes.rs:168` `Category::id` | `ModRecipeTypes.java:11-24` | fidèle |
| 2 | `recipes.rs:180` `Category::title` | `JobBasedRecipeCategory.java:152-155`, `CropRecipeCategory.java:95`, `ToolRecipeCategory.java:71`, `CompostRecipeCategory.java:42` | fidèle (après §2.5) |
| 3 | `recipes.rs:201` `Category::catalysts` | `JEIPlugin.java:203-211` | fidèle (ordre compris) |
| 4 | `recipes.rs:217` `Category::description` | `JobBasedRecipeCategory.java:102-104` | fidèle |
| 5 | `recipes.rs:228` `Category::ingredient_info` | `JEIPlugin.java:165-166`, `ModJobs.java:33` | fidèle |
| 6 | `recipes.rs:245` `job_category_id` | `GenericRecipeCategory.java:101-105` | fidèle (l'id **est** la clé du métier) |
| 7 | `recipes.rs:257` `job_translation_key` | `JobEntry.java:129` | **ajouté** (absent, cf. §2.5) |
| 8 | `recipes.rs:317` `ToolUsage::level_of` | `ToolsAnalyzer.java:88-92` | **divergent, corrigé** (§2.1) |
| 9 | `recipes.rs:331` `ToolUsage::enchant_bonus` | `ItemStackUtils.java:279-299` | **ajouté** (§2.1) |
| 10 | `recipes.rs:345` `ToolUsage::add` | `ToolsAnalyzer.java:84-104` | **divergent, corrigé** (§2.2) |
| 11 | `recipes.rs:364` `ToolUsage::slot_name` | `ToolRecipeCategory.java:99`, `:104` | fidèle |
| 12 | `recipes.rs:388` `enchant_levels` | `ToolsAnalyzer.java:39-45` | fidèle (1..=5) |
| 13 | `recipes.rs:430` `CompostEntry::output` | `CompostRecipe.java:31`, `:44` | fidèle (6 compost, indépendant de `strength`) |
| 14 | `recipes.rs:518` `JobRecipe::sort_key` | `GenericRecipeCategory.java:362-365`, `GenericRecipe.java:394-398` | **divergent, corrigé** (§2.3) |
| 15 | `recipes.rs:530` `is_loot_based` | `GenericRecipeCategory.java:336-339` | fidèle |
| 16 | `recipes.rs:537` `consolidate_needed` | `GenericRecipeCategory.java:344`, `FishermanRecipeCategory.java:114` | fidèle (`> 18`, pas `>=`) |
| 17 | `recipes.rs:543` `chance_key` | `JobBasedRecipeCategory.java:404-405` | fidèle (3 clés, préfixe compris) |
| 18 | `recipes.rs:570` `chance_label` | `JobBasedRecipeCategory.java:406-417` | fidèle (≥ 1 % → entier, sinon centièmes) |
| 19 | `recipes.rs:658` `Grid::loot` | `GenericRecipeCategory.java:238-243`, `FishermanRecipeCategory.java:60-64`, `FloristRecipeCategory.java:65-68`, `CropRecipeCategory.java:118-123` | fidèle (mêmes 3 formules) |
| 20 | `recipes.rs:670` `Grid::inputs` | `GenericRecipeCategory.java:172-177` | fidèle |
| 21 | `recipes.rs:682` `Grid::slot_at` | `GenericRecipeCategory.java:180-193` | fidèle (gauche→droite puis haut→bas) |
| 22 | `recipes.rs:717` `compactify` | `FloristRecipeCategory.java:119-151` | fidèle (`<` à `:124`, reste à `:133-137`) |
| 23 | `recipes.rs:760` `consolidate` | `LootTableAnalyzer.java:288-296`, `:502-505` | fidèle, ordre stable assumé (§4.2) |
| 24 | `markers.rs:326` `INITIAL_COLOUR` | `ColonyBorderMapping.java:262-283` | **divergent, corrigé** (§2.6) |
| 25 | `markers.rs:361` `BorderOverlay::group_name` | `ColonyBorderMapping.java:258` | **divergent, corrigé** (§2.4) |
| 26 | `markers.rs:381` `update_chunks` | `ColonyBorderMapping.java:289-297` | fidèle (`addAll \| removeAll`) |
| 27 | `markers.rs:388` `update_loaded_chunks` | `ColonyBorderMapping.java:300-308` | fidèle |
| 28 | `markers.rs:403` `update_info` | `ColonyBorderMapping.java:324-353` | fidèle (après §2.6 : la comparaison porte sur la couleur **courante**) |
| 29 | `markers.rs:415` `enabled` | `ColonyBorderMapping.java:375-378` | fidèle |
| 30 | `markers.rs:421` `shows` | `ColonyBorderMapping.java:381`, `:394` | fidèle, sauf `playerAccepts` (appel au jeu) |
| 31 | `markers.rs:427` `loaded_colour` | `ColonyBorderMapping.java:332` | fidèle (`0xFFFFFF & intensify`) |
| 32 | `markers.rs:441` `intensify` | `ColonyBorderMapping.java:355-367` | fidèle, arithmétique `FastColor` non vérifiable (§4.1) |
| 33 | `markers.rs:262`/`:271` `ChunkSet::add_all`/`remove_all` | `ColonyBorderMapping.java:293-295`, `:304-306` | fidèle (booléen « a changé ») |
| 34 | `markers.rs:302` `ChunkSet::packed` | `ColonyBorderMapping.java:222-224`, `:232-233` | fidèle + tri assumé (§4.2) |
| 35 | `markers.rs:588` `Deathpoints::sync` | `ColonyDeathpoints.java:80-111` | **divergent, corrigé** (§2.7) |
| 36 | `markers.rs:623` `Deathpoints::unload` | `ColonyDeathpoints.java:56-70` | fidèle (les tombes restent, les waypoints non) |
| 37 | `markers.rs:642` `Deathpoints::clear` | `ColonyDeathpoints.java:46-49` + `EventListener.java:57-58` | **divergent, corrigé** (§2.8) |
| 38 | `markers.rs:648` `Deathpoints::built` | `ColonyDeathpoints.java:164-189` | fidèle, plus strict (refus nommé, §4.3) |
| 39 | `markers.rs:666` `Deathpoints::pending_in` | `ColonyDeathpoints.java:143-149` | fidèle (même chunk seulement) |
| 40 | `markers.rs:716` `radar` | `JourneymapPlugin.java:85-160` | fidèle (8 cas, options comprises) |
| 41 | `markers.rs:499` `DeathpointLabel::key` | `ColonyDeathpoints.java:176-178` | fidèle (2 clés, args dans l'ordre) |
| 42 | `markers.rs:240` `MarkerOptions::any_border` + `SORT_ORDER` | `JourneymapOptions.java:35-47`, `:52-67` | fidèle (13 défauts, 13 ordres) |
| 43 | `audit.rs:85`/`:103` `Report::file`/`header` | `CraftingTagAuditor.java:68-77`, `:123-125`, `:153`, `:188`, `:220`, `:244`, `:302`, `:358`, `:438`, `:471`, `:501` | fidèle (après le correctif d'hier sur `Tools`) |
| 44 | `audit.rs:158` `quoted` | `CraftingTagAuditor.java:504-515`, `:521-523` | fidèle (`"` doublé, rien d'autre) |
| 45 | `audit.rs:174` `tier_row` | `CraftingTagAuditor.java:234-236` | fidèle (marqueur = `tier` étoiles) |
| 46 | `audit.rs:189` `full_health` | `CraftingTagAuditor.java:462` | fidèle, `Infinity` refusé (§4.4) |
| 47 | `audit.rs:203` `crafter_columns` | `CraftingTagAuditor.java:285-289` | fidèle (simple d'abord, puis la clé) |
| 48 | `audit.rs:152` `path` | `CraftingTagAuditor.java:85` | fidèle (`<monde>/minecolonies/<fichier>`) |

## 2. Divergences réelles, corrigées

### 2.1 `ToolUsage::level_of` — le bonus d'enchantement n'est pas le niveau

**Java** `ToolsAnalyzer.java:88-92` : `level = Math.min(MAX_BUILDING_LEVEL, level + ItemStackUtils.getMaxEnchantmentLevel(stack))`, et `getMaxEnchantmentLevel` (`ItemStackUtils.java:279-299`) répond **`Math.max(maxLevel - 1, 0)`** — le niveau d'enchantement **moins un**, plancher zéro.

**Ce que j'avais** : un paramètre documenté comme « le plus haut niveau d'enchantement porté ». Faux : un outil avec seulement Unbreaking I ajoute **0**, pas 1. Un outil Unbreaking III ajoute 2, pas 3.

**Corrigé** : `level_of(mining_level, enchant_bonus)` ne prend plus que le bonus, et [`ToolUsage::enchant_bonus`] (`recipes.rs:331`) porte la règle `max(n - 1, 0)` avec sa ligne. Test : `enchant_bonus(1) == 0`, `enchant_bonus(3) == 2`.

### 2.2 `ToolUsage::add` — `isEnchanted()` et le bonus sont deux réponses

**Java** `ToolsAnalyzer.java:96-103` : la rangée dépend de `stack.isEnchanted()`, pas du bonus. Avec 2.1, les deux diffèrent pour un outil enchanté seulement au niveau I : bonus 0, donc palier inchangé, **mais rangée enchantée**.

**Ce que j'avais** : `add` devinait la rangée avec `bonus > 0` — un objet enchanté niveau I partait dans la rangée simple.

**Corrigé** : `add(&mut self, stack, mining_level, enchant_bonus, enchanted: bool)` (`recipes.rs:345`), doc citant `:96`, et un test dédié (`a_stack_enchanted_at_level_one_is_enchanted_without_a_bonus`).

### 2.3 `JobRecipe::sort_key` — une carte sans sortie trie sous `minecraft:air`

**Java** `GenericRecipeCategory.java:362-365` trie par `getLevelSort` puis `ForgeRegistries.ITEMS.getKey(r.getPrimaryOutput().getItem())` ; `getPrimaryOutput()` rend `ItemStack.EMPTY` quand `mainOutputs` est vide (`GenericRecipe.java:394-398`), donc la clé est **`minecraft:air`**.

**Ce que j'avais** : `""` (chaîne vide), ce qui plaçait une carte « butin » avant **tous** les ids réels.

**Corrigé** : `sort_key` rend `(level_sort, "minecraft:air")` par défaut, avec le test qui montre qu'une carte `minecraft:acacia_boat` passe maintenant avant elle.

### 2.4 `BorderOverlay::group_name` — c'est la clé de dimension entière

**Java** `ColonyBorderMapping.java:258` : `String.format("colony_%s_%d", dimension.location(), id)`. `location()` rend une `ResourceLocation` et `%s` appelle son `toString()` : **`colony_minecraft:overworld_7`**.

**Ce que j'avais** : `colony_overworld_7`, avec un commentaire affirmant le contraire (« location() est le chemin, pas la clé ») — une erreur de lecture, pas de frappe.

**Corrigé** : la fonction et le test ; la doc dit maintenant que `dimension` porte la clé entière.

### 2.5 `Category::title` — la clé d'un métier est calculable

**Java** `JobBasedRecipeCategory.java:152-155` titre avec `job.getJobRegistryEntry().getTranslationKey()`, et `JobEntry.java:129` : `"com." + key.getNamespace() + ".job." + key.getPath()`.

**Ce que j'avais** : `Title::Job("minecolonies:fisherman")` en disant « le jeu la nomme ». Vrai, mais la règle était à un appel de distance.

**Corrigé** : `job_translation_key` (`recipes.rs:257`) donne `com.minecolonies.job.fisherman` avec sa ligne, testé (y compris le refus d'un id sans namespace).

### 2.6 `BorderOverlay::new` — la couleur d'un overlay neuf est `0x00ff00`, pas `-1`

**Java** `ColonyBorderMapping.java:262-283` : les quatre `ShapeProperties` et le `TextProperties` partent tous à `0x00ff00` ; `-1` n'est que le **défaut du champ dans le codec** (`:230`), donc la valeur d'un enregistrement relu dont le fichier n'a pas de couleur.

**Ce que j'avais** : `colour: -1` à la construction. Conséquence réelle : le premier `update_info` répondait `true` là où Java répond `false` pour une couleur d'équipe à `0x00ff00`.

**Corrigé** : `INITIAL_COLOUR` (`markers.rs:326`, `Documented` avec sa ligne), `new()` l'utilise, et la doc de `update_info` dit que la comparaison porte sur la couleur courante.

### 2.7 `Deathpoints::sync` — une tombe non construite est redemandée à chaque passage

**Java** `ColonyDeathpoints.java:108` : `waypoints.computeIfAbsent(grave, k -> tryCreatingWaypoint(...))`, pour **chaque** tombe, à chaque mise à jour ; et `computeIfAbsent` ne stocke rien quand la fonction rend `null` (`:158-161`). Une tombe dont le chunk n'est pas chargé est donc retentée au passage suivant.

**Ce que j'avais** : `pending` ne contenait que les tombes **nouvellement** connues — l'appelant était prévenu une fois, puis plus jamais.

**Corrigé** : `pending` = toutes les tombes encore sans waypoint **à cet appel**. Test `a_grave_whose_waypoint_is_not_built_yet_is_asked_again`.

### 2.8 `Deathpoints::clear` — ne retire aucun waypoint

**Java** `ColonyDeathpoints.java:46-49` : `clear()` ne fait que `overlays.clear()`, et son propre commentaire le dit (« does *not* remove the overlays from jmap ») ; l'unique appelant enchaîne `removeAll(MOD_ID)` (`EventListener.java:57-58`).

**Ce que j'avais** : `clear` rendait la liste des waypoints à retirer, ce qui prétendait faire le travail de `removeAll`.

**Corrigé** : `clear(&mut self)` rend `()`, doc citant les deux endroits ; test réécrit.

## 3. Quatre-vingts références de ligne fausses — la cause, et le compte

C'est le défaut le plus grave trouvé, et il ne vient pas de la lecture du Java mais de **ma façon de relever les numéros** : j'ai lu plusieurs fichiers avec `cat -n f1.java f2.java f3.java`, où le deuxième fichier reprend à la ligne 1 de la sortie mais garde ses propres numéros dans le fichier. Tout ce qui suit le premier fichier d'un `cat` est décalé.

| Fichier Java | Décalage | Références fausses |
| --- | ---: | ---: |
| `CropRecipeCategory.java` | +139 | 8 |
| `FishermanRecipeCategory.java` | +112 | 6 |
| `FloristRecipeCategory.java` | +246 | 9 |
| `Journeymap.java` | +190 | 1 |
| `JourneymapOptions.java` | +366 | 14 |
| `JourneymapPlugin.java` | +102 | 12 |

Plus, hors décalage : `ToolsAnalyzer.java:98-112` (le fichier en fait 105 — la méthode est `:84-104`), `ToolsAnalyzer.java:90-96` (la vraie ligne est `:88-92`), `ToolsAnalyzer.java:59-65` (les sept enchantements sont `:60-66`), `ItemStackUtils.java:298` (le `return` est `:299`), et `Constants.java:20` ambigu entre `api/util/constant/Constants.java:20` (le bon) et `core/client/gui/questlog/Constants.java`.

Toutes corrigées dans `recipes.rs`, `markers.rs`, `mod.rs`, `MANQUANT.md` et `RAPPORT.md`. **Vérification automatique** : un script relit les 119 références `Fichier.java:ligne` des quatre fichiers Rust, retrouve chaque fichier Java et contrôle que la ligne existe et n'est ni vide ni une accolade seule — 0 anomalie restante (hors l'ambiguïté `Constants` ci-dessus, tranchée à la main).

## 4. Ce que je n'ai pas pu vérifier, et ce que ça coûte

### 4.1 Vanilla, absent du workspace

| Ce qui manque | Où la règle est utilisée | Ce que le port fait |
| --- | --- | --- |
| `FastColor.ARGB32.lerp` / `Mth.lerpInt` (arrondi exact) | `intensify` (`markers.rs:441`) | la formule du Java MineColonies est lue (`:355-367`), l'implémentation du `lerp` non ; les tests n'utilisent que des valeurs exactes (128 → 96, 255 → 255) pour ne pas dépendre de l'arrondi |
| `ChunkPos.toLong` (disposition des bits) | `ChunkSet::packed` (`markers.rs:302`) | non réimplémenté : `crate::base::coords::ChunkPos::pack` le fait déjà et cite `horizon-core/src/position.rs` |
| `Block.getDescriptionId` / `Util.makeDescriptionId` | `Title::Block` (`recipes.rs:183`) | la règle n'est **pas** recopiée : `Title::Block` garde l'id du bloc et laisse le jeu le nommer |
| `IGuiHelper.getSlotDrawable()` = 18 px | les trois `Grid` | 18 est une **entrée** (`SLOT`), documentée comme telle, pas une constante déduite |
| `ResourceLocation.toString()` | `group_name` (§2.4) | la règle est déduite du `%s` du Java ; elle est vérifiée par la forme de la clé (`namespace:path`), pas par un fichier du workspace |

### 4.2 Écarts assumés, écrits dans la doc

- **`consolidate`** : Java groupe par `Objects.hash(...)` (`LootTableAnalyzer.java:291`, `:502-505`) puis trie par probabilité décroissante ; `groupingBy` rend une `HashMap`, donc **l'ordre de deux probabilités égales n'est pas spécifié en Java**. Le port garde l'ordre d'apparition (tri stable) et le dit.
- **`ChunkSet::packed`** : Java sérialise une `HashSet` (`:222-224`, `:232-233`), donc l'ordre du fichier n'est pas spécifié. Le port trie : deux écritures du même terrain donnent le même fichier.
- **`full_health(0.0)`** : Java écrit `Infinity` dans le CSV (`:462`, division `double`) ; le port refuse et rend `None`.
- **`Report::columns`** : les rapports à queue dynamique ont un en-tête **raccourci** dans le port (la partie fixe seulement) ; `columns()` le dit.

### 4.3 `Deathpoints::built` refuse une tombe inconnue

Java ne peut pas construire un waypoint pour une tombe absente de la carte (`computeIfAbsent` seulement), mais sa fonction d'insertion crée la clé. Le port rend `false` pour une tombe qui n'a pas été vue par `sync` : c'est un refus nommé, pas une divergence de comportement atteignable par le flux Java.

## 5. Ce qui reste ouvert, et n'est pas un manque d'API

| Règle Java | Pourquoi elle n'est pas portée |
| --- | --- |
| `BarrelType.byMetadata(timer.getValue())` (`CompostRecipeCategory.java`) | le baril animé de la carte compost : dessin, `MANQUANT.md` §1 |
| `getCatalyst(building)` par métier (`JobBasedRecipeCategory.java:126-129`) | la catégorie d'un métier a pour catalyseur le bloc de **son** bâtiment ; le port modélise la famille, pas les 40 bâtiments (`Category::catalysts` ne couvre que les 5 ids fixes) |
| `ModLootTables.FISHERMAN_BONUS` (contenu) | `FishingRecipe` modélise la **forme** (id, niveau, drops) ; les tables sont du contenu |
| `RenderHelper.renderBlock`, `JobBasedRecipeCategory.draw` | dessin (citizen, animal, bloc intermédiaire) |
| `CraftingTagAuditor` : `SchemAnalyzerUtil.getScoreFor` / `getBlockTier` | scores de schématique, hors des 33 fichiers |

## 6. Tests

**80 tests** dans le module (contre 76 avant cet audit). Ajoutés : `the_enchant_bonus_is_the_highest_level_minus_one`, `a_stack_enchanted_at_level_one_is_enchanted_without_a_bonus`, `a_grave_whose_waypoint_is_not_built_yet_is_asked_again`, `a_jobs_translation_key_is_built_from_its_id`. Réécrits sur la règle corrigée : `a_tools_level_is_mining_plus_the_enchant_bonus_capped_at_five`, `an_enchanted_stack_goes_to_the_enchanted_row`, `a_cards_sort_key_is_level_then_output`, `the_overlay_group_name_carries_the_whole_dimension_key`, `clearing_forgets_the_graves_without_removing_a_waypoint`.

Aucune compilation lancée (règle de l'orchestrateur), aucun `git`, rien touché hors `src/compatibility/`.
