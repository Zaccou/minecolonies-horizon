# `compatibility` — ce qui manque, avec la recherche faite

Chaque entrée : ce que l'intégration publie (`fichier.java:ligne`), la primitive Horizon qui
le couvrirait — ou la preuve qu'aucune ne le couvre — et ce que le port fait à la place.
Aucune de ces fonctions n'est écrite en code fictif.

**La recherche, pour toutes les entrées ci-dessous.** Trois passes, refaites à chaque fois :

1. `ls crates/horizon-mod-vocab/src/hooks/` — **719 fichiers de hooks**, lus par nom
   (`minimap`, `map_decorations`, `map_decoration_types`, `panel_event`, `recipe_*`,
   `menu_transfer`, `recipe_sources`, `screen_exclusion_areas`, `gui_layer_*`, `render_gui_*`).
2. `grep -n "^    fn " crates/horizon-mod-vocab/src/host/*.rs` — les méthodes hôte des
   39 zones (`recipes`, `items`, `tags`, `levels`, `players`, `entities`, `client_render`,
   `draw`, `screens`, `config`, …).
3. Une recherche par mot sur toute la crate, pour chaque notion qui manque :
   `polygon`, `overlay`, `marker`, `radar`, `hud`, `info slot`, `waypoint`, `minimap`.

Les résultats qui tranchent : `polygon` n'apparaît que dans les couches de modèle 3D
(`client/model_layers.rs`), `radar` **nulle part**, `waypoint` seulement comme icône de barre
de localisation d'une entité vivante (`host/living.rs:667-684`), `info slot` nulle part.

---

## 1. Une catégorie de recette avec sa mise en page

**Publié** : six familles de catégories, chacune avec ses emplacements, son fond, son icône,
son animation et ses infobulles — `JEIPlugin.java:90-160`, `JobBasedRecipeCategory.java:206-242`,
`GenericRecipeCategory.java:114-334`. C'est la plus grosse part des 2 056 lignes de `jei/`.

**Réalité** : le SDK le dit lui-même, mot pour mot :

> « `Recipe` does not define display pages, per-result chances or fluid input/output slots: a
> viewer interprets its own machine-recipe JSON and draws those details itself. »
> — `crates/horizon-mod-vocab/src/hooks/notes.rs:2744-2748`

Ce qui **existe** : `RecipesClientHost::panel_items` affiche un panneau d'objets à côté des
écrans de conteneur (8192 objets au plus, pages et recherche), avec `panel_style` dont
`PanelStyle::Classic` imite JEI 1.12 (`host/recipes.rs:74-96`,
`client/types.rs:889-912`). C'est une **liste d'objets**, pas une page de recette : aucun
emplacement d'entrée/sortie, aucun tirage animé, aucune chance par sortie.

`PanelEvent` nomme pourtant les gestes d'une page de recette —
`SlotClicked { recipe, slot, stack, right }`, `Action(u32)`, `Page(u32)`, `Back`, `Tab(u32)`
(`client/types.rs:929-956`) — mais **aucun hook de cette révision ne publie de page** :
`grep -rn "panel_recipes"` ne trouve que le commentaire du type, et
`hooks/panel_event.rs:5` ne cite que `native_panel_items`. Sur les sept variantes de
`PanelEvent`, trois seulement sont atteignables (`Clicked`, `Search`, `Key`).

**Ce que le port fait** : `recipes.rs` garde la donnée que la page montrerait — niveau, outil,
entrées, sorties, chances, bloc intermédiaire, animal — et `mod.rs` dit lequel des six
publie quoi. Le jour où un hook `native_panel_recipes` existe, `JobRecipe` et `Grid`
(`recipes.rs`) donnent la liste et la géométrie déjà mesurées.

## 2. Un polygone sur une carte, et une carte qu'on ne remplace pas

**Publié** : l'outline du claim, trous compris, dessiné en polygones sur la carte plein écran
et la minimap — `ColonyBorderMapping.java:394-432` — et la bande des chunks maintenus chargés,
en plus clair — `:434-461`. Le polygone vient de `PolygonHelper.createChunksPolygon(chunks,
256)` (`:398`), donc d'une forme **quelconque** tirée d'un ensemble de chunks.

**Réalité** : aucune primitive de forme sur une carte. Ce qui s'en approche, et pourquoi ce
n'est pas la même chose :

| Primitive | Ce qu'elle donne | Ce qui manque |
| --- | --- | --- |
| `minimap` (`hooks/minimap.rs`) | **remplacer** la minimap du jeu : « set `minimap` when it is none yet » | le mod doit *être* la minimap ; deux mods ne peuvent pas coexister, et la carte plein écran n'est pas concernée |
| `map_decorations` (`hooks/map_decorations.rs`) | des icônes sur un **objet carte** tenu ou dans un cadre | une carte n'a pas de polygone, seulement des sprites posés à des demi-pixels ; rien sur la minimap ni la carte plein écran |
| `map_decoration_types` (`hooks/map_decoration_types.rs`) | enregistrer un type d'icône de carte | idem : des points, pas des surfaces |

**Ce que le port fait** : `markers.rs` garde l'ensemble de chunks par colonie
(`BorderOverlay::chunks`), son encodage de sauvegarde (`ChunkSet::packed`, `ChunkPos.toLong`),
la règle d'affichage (`enabled`, `shows`) et la couleur de la bande chargée (`intensify`).
Tout ce qui reste est le tracé.

## 3. Un waypoint de carte

**Publié** : un waypoint par tombe, nommé d'après le citoyen et son métier, avec l'icône
`minecolonies:textures/icons/grave_icon.png`, 16 × 16, couleur `0x888888` —
`ColonyDeathpoints.java:164-189`.

**Réalité** : **aucun waypoint de carte**. Le mot `waypoint` dans la crate désigne autre
chose : l'icône de la barre de localisation d'une entité vivante,
`LivingHost::living_set_waypoint_style` / `living_set_waypoint_color`
(`host/living.rs:667-684`), pilotée par `/waypoint modify` — c'est un marqueur de joueur vu
en jeu, pas une épingle sur une carte, et il ne survit pas à la mort de l'entité.

**Ce que le port fait** : `Deathpoint` (position + libellé + constante d'icône et de couleur)
et `Deathpoints`, qui porte les **trois transitions** de Java — apparition en attente
(`sync`), construction quand le chunk se charge (`pending_in`), disparition (`unload`,
`clear`). C'est la partie qui se teste ; l'appel à `addWaypoint` n'a pas de cible.

## 4. Un point d'entrée « radar d'entités »

**Publié** : pour chaque citoyen vu sur la carte, un nom, une ligne de métier, une couleur
d'équipe, et la possibilité de le cacher ; pour chaque raider, une couleur ou rien —
`JourneymapPlugin.java:187-262` (`v5` : `EventListener.onUpdateEntityRadar`).
Le tout piloté par 4 des 13 options.

**Réalité** : `radar` n'apparaît **nulle part** dans la crate. Aucun hook ne dit « comment
cette entité est dessinée sur une carte » : `entity_outline`, `render_name_tag`,
`entity_renderers` concernent le rendu du monde, pas une carte.

**Ce que le port fait** : `radar()` est la **table de décision** seule — entrée
(visiteur / citoyen avec métier / raider, nom, couleur d'équipe, quelle carte regarde),
sortie (cacher, infobulles, effacer le nom, couleur) — et elle est testée sur les huit cas
que Java distingue. Sans cible, la décision reste vraie et vérifiable.

## 5. Une ligne d'info dans le HUD de la carte

**Publié** : la colonie où se tient le joueur, demandée toutes les 2 500 ms et affichée dans
le HUD de JourneyMap — `JourneymapPlugin.java:184`,
`event.register(MOD_ID, "…currentcolony", 2500, ColonyBorderMapping::getCurrentColony)`.

**Réalité** : pas de registre de lignes de HUD. Ce qui existe :

- `render_gui_post` dessine **après la minimap** (`hooks/notes.rs:2357`) : on peut écrire du
  texte au même endroit de l'écran, mais pas dans le cadre, l'ordre ni le format que
  JourneyMap impose à ses lignes ;
- `register_gui_layers` + `gui_layer_draw` donnent une couche à soi
  (`hooks/gui_layer_draw.rs:5-12`), même limite.

**Ce que le port fait** : `INFO_SLOT_PERIOD` (2 500 ms) et `INFO_SLOT_KEY` sont écrits, et
`markers.rs` répond à la question que la ligne pose — « quelle colonie contient cette
position » — à partir des ensembles de chunks qu'il tient déjà.

## 6. Écrire `border.json`

**Publié** : un fichier par dimension, sous le dossier de données du mod, liste de
`ColonyBorderOverlay` (`ColonyBorderMapping.java:76-104`, `:222-234`), via un `Codec`
Mojang et `Journeymap.saveData` (`Journeymap.java:339-365`).

**Réalité** : la racine existe — `Host::data_files()` et `ServerHost::world_data_files()`
rendent un `ConfigFiles` dont `write` prend des **octets**
(`crates/horizon-mod-vocab/src/content/config_files.rs:18-45`,
`host/config.rs:50-60`) — mais la crate n'a **aucun codec JSON** : `Cargo.toml` ne déclare
qu'une dépendance, `horizon-mod-api`. Écrire le fichier demande donc un encodeur JSON, et le
port n'en a pas.

**Ce que le port fait** : `markers.rs` nomme les **sept champs** du record (`dimension`,
`id`, `colony_name`, `colour`, `licet`, `chunks`, `loaded`) avec leur défaut Java, et
`ChunkSet::packed` / `from_packed` font l'aller-retour exact que le codec fait
(`Codec.LONG.xmap(ChunkPos::new, ChunkPos::toLong)`). Le jour où un encodeur existe, la
donnée est prête ; aujourd'hui rien n'est écrit, et aucun fichier n'est fabriqué.

## 7. Le niveau **minimal** d'un enchantement, pour l'essai des outils

**Publié** : `ToolsAnalyzer` essaie sept enchantements aux niveaux 1 à 5 et ne garde que
ceux qui passent `enchantLevel >= enchantment.getMinLevel() && enchantLevel <=
enchantment.getMaxLevel()` **et** `enchantment.canEnchant(stack)`
(`ToolsAnalyzer.java:41-44`, `:85-91`).

**Réalité** : deux des trois tests sont couverts — `ItemsClientHost::enchantment_supports(id,
item)` pour `canEnchant` (`host/items.rs:163-164`), et `EnchantmentInfo::max_level`
(`item/types.rs:56-64`, documenté « the registry facts a recipe viewer needs »). **Le niveau
minimal n'est exposé nulle part** : `EnchantmentInfo` n'a que `id`, `max_level`, `curse`.

**Ce que le port fait** : `ToolUsage::level_of` (le calcul du palier, `ToolsAnalyzer.java:90-96`)
est porté et testé ; `enchant_levels()` dit 1..=5 et `ENCHANTMENTS` donne les **sept noms de
champ Java** sans inventer leur id 26.3. L'essai lui-même attend `getMinLevel`.

## 8. Deux gestes de JEI qui ne sont pas des transferts

Deux classes de `transfer/` en sortent, et elles ne sont pas la même primitive :

| Classe | Ce qu'elle fait | Horizon |
| --- | --- | --- |
| `IRecipeTransferHandler` (`PrivateCraftingTeachingTransferHandler.java:24-40`, idem fourneau et alambic) | le « + » d'une recette vers la grille de l'atelier | **couvert** par `menu_transfer`, dont le champ `neoforge` dit « JEI `IRecipeTransferHandler`, which each mod implements for its menus » (`hooks/menu_transfer.rs:52-58`) |
| `IGuiContainerHandler` (`AbstractTeachingGuiHandler.java:24-38`) | les zones que le panneau recouvre | **couvert** par `screen_exclusion_areas`, nommé d'après `IGuiContainerHandler.getGuiExtraAreas` (`hooks/screen_exclusion_areas.rs:5-22`) |
| `IGhostIngredientHandler` (`AbstractTeachingGuiHandler.java:26`, `getTargetsTyped` `:85-113`) | glisser un objet du panneau **dans un emplacement précis** de l'écran | **rien**. `screen_click` et `menu_clicked` existent, mais aucun hook ne dit « le lecteur de recettes a déposé cet objet dans cet emplacement » |
| `IJeiProxy.showRecipes(stacks)` (`JEIPlugin.java:58-74`) | ouvrir le lecteur **sur la sortie** d'un objet (bouton des GUI d'atelier, `ui_window_09/crafting.rs:117`) | **rien** pour « ouvrir le panneau sur cet objet » ; `panel_items` + `PanelEvent::Clicked` en sont le plus proche |

**Ce que le port fait** : rien à porter pour les deux premiers (la primitive est meilleure que
l'original : elle passe par le menu du serveur). Les deux derniers sont des **gestes d'écran
sans cible** ; les nommer ici évite qu'un futur agent les croie faits.

---

## Ce qui n'est **pas** manquant, et qu'il ne faut pas réécrire

| Java publie | Primitive Horizon | Preuve |
| --- | --- | --- |
| les recettes elles-mêmes, comme données | `RecipesHost::recipes_of_type`, `recipe_ingredient`, `recipe_result` | `host/recipes.rs:29-48` |
| les 13 options de `JourneymapOptions` | `ConfigHost::setting` / `set_setting` (une chaîne par clé) | `host/config.rs:32-38` |
| les 10 CSV de `CraftingTagAuditor` | `ServerHost::world_data_files()` → `ConfigFiles::write` | `host/config.rs:155`, `content/config_files.rs:18-45` |
| le transfert « + » | `menu_transfer` | `hooks/menu_transfer.rs` |
| les zones recouvertes par un panneau | `screen_exclusion_areas` | `hooks/screen_exclusion_areas.rs` |

## Une fonction empruntée, pas manquante

`recipes::consolidate` est `LootTableAnalyzer.consolidate` (`LootTableAnalyzer.java:288-296`),
qui n'est pas dans les 33 fichiers de cette tranche mais que deux des six catégories appellent
(`GenericRecipeCategory.java:344`, `FishermanRecipeCategory.java:226`). Elle vit ici
provisoirement, et son commentaire dit où elle retournera quand l'analyseur sera porté.
