# manager_05 — ce qui manque, et ce que ça coûte

Quatre gestionnaires de `core/colony/managers/` : `EventManager`,
`EventStructureManager`, `GraveManager`, `RegisteredStructureManager`.
Chaque entrée dit **ce que je voulais faire**, **pourquoi je ne l'ai pas fait**,
et **quel appel d'API lèverait le blocage**. Rien de tout ça n'est simulé en
code : les règles qui l'exigent renvoient un refus nommé et un `Plan` que
l'appelant exécute.

## 1. `Lifecycle` ne peut pas relire son compteur

`src/events/raid.rs` porte la carte et le tick de `EventManager`, mais son
compteur est privé : `Lifecycle::new()` démarre à `FIRST_ID`, et il n'y a pas de
`set_counter`. Un `Lifecycle` reconstruit depuis une sauvegarde rendrait donc
l'id 1 une deuxième fois et entrerait en collision avec chaque événement déjà
sauvegardé.

Ce que je voulais faire : `EventBook::read` → `Lifecycle` avec son compteur.
Ce que j'ai fait : `EventBook` garde le compteur (`IdCounter`, qui appelle
`take_from` pour ne pas dupliquer la règle de bouclage) et les enregistrements ;
le tick reste celui de `Lifecycle`, et l'appelant tient les deux côte à côte.

Ce qui le lèverait : `Lifecycle::set_counter(i32)` (ou un
`Lifecycle::from_parts(counter, raids)`) dans `src/events/raid.rs`. C'est un
module d'un autre agent, donc pas touché ici.

## 2. Pas d'écriture ni de lecture de blueprint

`EventStructureManager` copie les blocs dans un fichier
(`BlueprintUtil.createBlueprint`, `StructurePacks.storeBlueprint`, `:107-110`) et
les remet avec `PlaceStructureOperation` (`:141-161`). Un mod wasm n'a ni
`std::fs` ni `StructurePacks`.

Ce que je voulais faire : écrire et relire le `.blueprint` de sauvegarde.
Ce que j'ai fait : `Backups::plan` décide **quel** anchor, **quel** événement et
**quel** chemin (`backup_path`), et rend un `Plan { anchor, event, spawn, corner }`
que l'appelant exécute ; `Backups::restore` rend les anchors à replacer, dans
l'ordre. Le nom de fichier est `x_y_z.blueprint` là où Java écrit
`anchor.toString()`, qui est un détail de `BlockPos` vanilla : l'écrivain et le
lecteur sont la même fonction, donc le port se relit, mais **un fichier écrit
par Java n'est pas retrouvé par le port** et réciproquement.

Ce qui le lèverait : une capacité « écrire un fichier sous `blueprints/` » et un
`place_blueprint(at, rotation, mirror)` dans l'API de l'hôte.

## 3. Aucun accès au bloc ni à l'inventaire pour les tombes

`GraveManager.createCitizenGrave` fait quatre choses que le port ne peut pas
faire : poser le bloc (`:294`), transférer l'inventaire
(`InventoryUtils.transferAllItemHandler`, `:297`), faire la boucle d'armure
(`:301-308`) et retarder la décomposition (`:311`).

Ce que j'ai fait : `find_site` porte les trois refus (lave, eau, pas de sol) avec
leur **ordre**, `find_around` porte la recherche en anneau de structurize
(`BlockPosUtil.java:1035-1127`), `Decay` porte la pourriture
(`TileEntityGrave.java:228-251`), et `Graves::fill` rend le journal ordonné des
sept écritures (`Deposit`).

Ce qui le lèverait : `set_block(pos, state)`, `transfer_items(from, to) -> u32`
et `spawn_item(pos, stack)` dans l'API de l'hôte.

## 4. Pas de paquets, donc pas de moitié client

`RegisteredStructureManager` envoie `ColonyViewBuildingViewMessage`,
`ColonyViewRemoveBuildingMessage` et
`ColonyViewBuildingExtensionsUpdateMessage` (`:748-786`), et
`EventManager` n'en envoie aucun. Un mod sans client n'a personne à qui envoyer.

Ce que je voulais faire : les deux moitiés. Ce que j'ai fait : la moitié serveur
seulement, et les deux drapeaux `isBuildingsDirty` / `isBuildingExtensionsDirty`
sont remplacés par le compteur `Registry::writes`.

Ce qui le lèverait : une API de charge utile client. `src/manager_01::views`
porte déjà les structures de vue ; il manque le transport.

## 5. `getRandomLeisureSite` : l'échelle aléatoire n'est pas portée

`RegisteredStructureManager.java:344-408` choisit un site de loisir avec
`RANDOM.nextInt(4)`, deux `RANDOM.nextBoolean()` et `world.isRaining()`, et
compare la classe de chaque bâtiment (`BuildingMysticalSite`, `BuildingLibrary`,
`BuildingUniversity`, `BuildingTownHall`, un module `TAVERN_VISITOR`).

Ce que je n'ai pas fait : les soixante lignes de cette échelle. Chaque branche
rend « une position » ; la seule règle de refus y est « pas de bâtiment → la
suivante », et les six `if` successifs tombent tous sur `leisureSites`. Le coût
d'un port est élevé (six tirages, un test de pluie) pour une règle de sélection
que rien ne peut contredire.

Ce que le port a : `Registry::leisure_sites`, `add_leisure_site`,
`remove_leisure_site`, et `cleanUpBuildings`'s règle de retrait
(`:378-385`, voir §6).

Ce qui le lèverait : rien de technique — c'est un choix, et il se défait en une
heure si le comportement des PNJ en dépend. Il exigerait `is_raining` et les
niveaux par position, que l'appelant a déjà.

## 6. `cleanUpBuildings` : le ménage dépend du monde

`RegisteredStructureManager.java:302-336` retire un bâtiment dont le bloc ne
correspond plus, une extension hors colonie ou invalide, et un site de loisir
sans `TileEntityDecorationController`. Les trois tests demandent `isBlockLoaded`,
`getBlockState` et `isCoordInColony`.

Ce que j'ai fait : la partie qui est une règle — la comparaison des positions
(`guard_cover`, `covered_by_guard`) et `remove` — plus l'avertissement
« tous les bâtiments disparaissent d'un coup » (`:329-334`), qui est un test sur
des tailles et se porterait en trois lignes.

Ce qui le lèverait : les mêmes trois appels que §3.

## 7. Le module attendu d'une extension n'est pas vérifiable

`:192-211` remet `owner = None` quand le bâtiment propriétaire n'existe plus,
**et** quand son `BuildingExtensionsModule` n'attend pas ce type d'extension
(`extension.getClass().equals(extensionsModule.getExpectedExtensionType())`). Le
port fait la première moitié (`orphaned_extensions`) ; la seconde demande la
table « quel type de module pour quel type de bâtiment », que
`src/building/mod.rs:27-31` déclare explicitement absente (« no per-kind module
list »).

Ce qui le lèverait : la table de modules par type de bâtiment,
`apiimp/initializer/ModBuildingsInitializer.java:95-107`.
