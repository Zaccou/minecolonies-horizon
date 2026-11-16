# Ce que les règles de `src/blocks/` ne peuvent pas faire, et ce qu'il faudrait

Les 86 blocs de MineColonies ont maintenant leur table, leurs règles de clic, de
pose et de casse, et l'inventaire de leurs propriétés d'état (`src/blocks/`,
1 700 lignes, 43 tests). Ce fichier dit **ce qui manque autour**, avec la ligne de
Java ou la ligne du SDK qui le dit.

---

## 1. Le câblage : `src/lib.rs` n'est pas à cet agent

Les règles sont pures et rendent un [`Answer`], un `Placed` ou un `f32` ; personne
ne les appelle encore. Chaque règle a **un** point d'entrée dans le SDK épinglé, et
un seul :

| Règle | Hook | Contrat | Où |
| --- | --- | --- | --- |
| `click` | `use_without_item` | `server`, `replace`, `scope = "block"`, `key = "state"` | `crates/horizon-mod-vocab/src/hooks/block_use_without_item.rs` |
| `click` (client, l'écran qui s'ouvre) | `right_click_block` | `both`, `cancel` | `crates/horizon-mod-vocab/src/hooks/right_click_block.rs` |
| `placement` (la direction) | `state_for_placement` | `server`, `replace`, `scope = "block"` | `crates/horizon-mod-vocab/src/hooks/state_for_placement.rs` |
| `placement` (le lien à la colonie) | `block_changed` | `server`, `listen` | `crates/horizon-mod-vocab/src/hooks/block_changed.rs` |
| `break_progress` et `break_warning` | `player_break_speed` | `both`, `cancel` | `crates/horizon-mod-vocab/src/hooks/player_break_speed.rs` |
| la casse effective | `break_block` / `player_destroyed_block` | `server`, `cancel` / `listen` | `hooks/break_block.rs`, `hooks/player_destroyed_block.rs` |

**Quatre de ces hooks ne sont pas des méthodes de `BlockPart`** : `break_block`,
`player_destroyed_block`, `block_drops` et `player_break_speed` sont déclarés sans
`scope`, donc ils vont sur `impl Hooks for Mod` dans `src/lib.rs` — un fichier que
quatre autres agents écrivent en ce moment. C'est la raison pour laquelle la casse
est ici un nombre et un nom de message, et rien de plus.

(`block_break_progress_update`, malgré son nom, est l'**animation** de fissure d'un
invité — `progress` de 0 à 9, `server`/`listen` — et pas la vitesse de destruction :
il ne rendrait pas `break_progress`.)

**`on_block_placed_by` est `scope = "item"`** (« Only Parts attached to the stack's
item hear this server notice », `hooks/on_block_placed_by.rs`) : un `BlockPart` ne
peut donc pas entendre la pose d'un bloc, et le lien `Placed::linked` devra être
écrit depuis le hook d'item ou depuis `block_changed`.

## 2. Le tick d'un bloc de colonie

`AbstractColonyBlock` implémente `ITickableBlockMinecolonies`
(`api/blocks/AbstractColonyBlock.java:54`), dont le tick est celui du block entity
(`api/blocks/interfaces/ITickableBlockMinecolonies.java:12-18`). Le port n'a
**aucun** tick de block entity : `src/lib.rs` déclare cinq hooks et aucun
`block_entity_tick`, et `src/hut.rs` le dit déjà dans son en-tête. Sans lui, rien
n'avance : ni l'ouverture d'un écran qui attend, ni la réactivation d'un bâtiment
désactivé.

## 3. Le nœud de connexion du panneau de colonie

Casser `colonysign` retire un nœud du `ConnectionManager` de la colonie
(`core/blocks/BlockColonySign.java:112-131`). Le port n'a pas ce gestionnaire : il
a la **clé** du module (`colony_connections`,
`crate::building::behaviour::modules`) et pas le graphe. `decoration::removes_connection_node`
dit quelle casse le demande ; qui l'applique reste à écrire.

## 4. `age` : un état entier que le contenu ne sait pas déclarer

Les quinze cultures ont `IntegerProperty.create("age", 0, 6)`
(`core/blocks/MinecoloniesCropBlock.java:71`). Le format de contenu ne déclare que
`boolean` et `enum` (`crates/horizon-mod-package/src/content.rs:356-363`), et le
gabarit évident — `minecraft:wheat` — a `age` de 0 à **7**, une valeur de trop.
C'est `StateIn::Gap(..)` sur les quinze lignes, et le rapport de famille le répète.

**Il faudrait** : soit un type `integer` dans `StateProperty`
(`content.rs:358-363`), soit un gabarit de culture dont l'`age` s'arrête à 6. Le
second ne demande aucune API, le premier en demande une.

Les deux autres entiers vont bien : `moisture` (0..7) vient du gabarit
`minecraft:farmland`, et `rotation` (0..15) du gabarit `minecraft:white_banner`.

## 5. Les 83 fichiers de contenu

`data/minecolonies/horizon/block/` en porte trois sur 86 : `blockhutbuilder.json`,
`blockwaypoint.json`, `farmland.json`. Ce module ne peut pas les écrire (règle 1) ;
il dit ce que chacun doit contenir :

- `properties` : recopier `Block::props` (`hardness`, `explosion_resistance`,
  `requires_tool`, `sounds`, `map_color`), les cinq clés que les trois fichiers
  déjà présents utilisent ;
- `state_properties` + `default_state` : les `State` dont
  `StateIn::Declared` — `facing` des 54 blocs de colonie est déjà écrit dans
  `blockhutbuilder.json`, et c'est le même pour les 53 autres ;
- `shapes` / `collision_shapes` : `Form::box_of()` donne la boîte quand il y en a
  une, et `Form` dit « à l'état près » pour les autres.

Deux pièges que la table porte déjà : `blockhutfield` est l'identifiant de la
**scarecrow** (`REGISTRY_NAME` hérité, `AbstractBlockMinecoloniesDefault.java:44`)
et pas `blockscarecrow` ; et `decorationcontroller` a un `facing` à **six** valeurs
(`DirectionalBlock`), pas les quatre des huts.

## 6. Ce que Java fait au clic et que le port ne peut pas encore rendre

- **`getRequirements`** (`api/blocks/AbstractBlockHut.java:132-161`) compare la
  recherche globale de la colonie ; le port n'a pas d'arbre de recherche.
- **la fenêtre de réactivation** (`AbstractColonyBlock.java:205`) n'existe pas
  côté client ; `Answer::Open { window: Reactivation }` nomme l'intention.
- **`onBlockPlacedByBuildTool`** (`AbstractBlockHut.java:102-121`) écrit le miroir,
  le style et le chemin du blueprint dans le block entity : c'est la colle de la
  build tool, qui pose un schéma — `src/structures/` et `crate::blueprint`.
- **`getDestroyProgress` du town hall en pvp** envoie un message à 25, 50, 75 et
  90 % (`BlockHutTownHall.java:75-133`) : quatre paliers, pas un.

## 7. Une correction au brief

Le brief dit que 56 blocs descendent d'`AbstractColonyBlock`. Mesuré : **54**.
Les 56 sont ceux qui portent `IBuilderUndestroyable` — les 54, plus
`BlockDecorationController` (`core/blocks/BlockDecorationController.java:51`) et
`BlockPlantationField` (`core/blocks/BlockPlantationField.java:46`). Les deux
nombres sont des constantes (`COLONY_COUNT`, `BUILDER_UNDESTROYABLE_COUNT`) et deux
tests les tiennent séparés.
