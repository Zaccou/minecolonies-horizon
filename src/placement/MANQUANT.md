# MANQUANT — ce que les handlers de placement réclament et que l'API n'a pas

Écrit pour `src/placement/`. Chaque entrée dit **ce que je voulais faire**, **ce que l'API
offre à la place**, et **ce que ça coûte**. Rien de ce qui suit n'est simulé dans le code : les
quatre premiers points sont des pertes assumées et signalées aux endroits cités, les deux
derniers sont des périmètres.

## 1. Les écritures qui dépendent du voisinage

`GeneralBlockPlacementHandler.java:52-68` et `DoBlockPlacementHandler.java:64-83` redemandent
un état de placement pour les murs, barrières, barreaux et piliers :

```java
blockState.getBlock().getStateForPlacement(new BlockPlaceContext(world, null, MAIN_HAND,
    ItemStack.EMPTY, new BlockHitResult(new Vec3(0,0,0), Direction.DOWN, pos, true)));
```

C'est ce qui fait qu'une clôture collée à une clôture se **connecte**, qu'un mur prend ses
côtés, qu'un pilier prend son axe. L'API n'a pas de `getStateForPlacement` : `edit_blocks`
prend des états complets (`crates/horizon-mod-vocab/src/host/world_edit.rs:10-20`) et
`colony_world::engine::UPDATE_FLAG` documente déjà qu'aucun drapeau n'est porté. Une clôture
d'un blueprint est donc posée **non connectée** au bloc voisin de la même clôture, et le
premier rafraîchissement de voisinage la corrige — ou ne la corrige pas.

**Ce qu'il faudrait :** soit un hook de placement vanilla, soit un `block_state_with_neighbours`
côté hôte. Aucun des deux n'existe ; inventer une table de connexions ici serait recopier le
moteur.

## 2. Les tags de bloc et les classes de bloc

Trois `canHandle` de Java ne se lisent pas dans un identifiant :

| `canHandle` | Java | Ce que le port fait |
| --- | --- | --- |
| `BlockState.is(ModTags.BLUEPRINT_BLACKLIST)` | `PlacementHandlers.java:1143` | `Rule::Tag` n'accepte **rien** : le bloc part au handler général, qui l'écrit |
| `instanceof FallingBlock \|\| instanceof Fallable` | `:336` | liste d'ids (`kind::is_falling`) — un oubli tombe au général, et le bloc tombe au premier tick |
| `instanceof BaseEntityBlock` | `:893` | liste d'ids (`kind::is_container`) — un oubli tombe au général, qui écrit aussi la donnée |

Le troisième est presque bénin : `GeneralBlockPlacementHandler` écrit la donnée de bloc
(`:80-91`), donc la seule perte est le `try/catch` du conteneur (`:904-917`), qui refuse
d'écrire quand un inventaire de blueprint ne se charge pas.

Les deux premiers ne le sont pas. **Ce qu'il faudrait :** une lecture de tags
(`#structurize:blueprint_blacklist`) et une lecture de `Block` → classe. `block_state_id`
n'en donne aucune ; la liste des 9 515 blueprints convertis donne les ids utilisés, pas les
tags.

## 3. Les 22 lignes de structurize

`Table::java` les nomme pour que l'ordre soit complet, et `decide` répond
`Verdict::Delegated`. Les porter, c'est un autre module (`placement/structurize.rs`) ; ce que
chacune coûte, en une ligne :

| ligne | ce qu'il faudrait |
| --- | --- |
| `Air` | `Change::Break` + retirer les entités non vivantes de la cellule (`:744-751`) — l'API n'a pas de suppression d'entité par boîte |
| `SolidSubstitution`, `Substitution`, `FluidSubstitution`, `TagSubstitution` | un sol choisi par le contexte (`getSolidBlockForPos`) : `crate::structures::terrain` en a la moitié |
| `BlackListed` | cf. §2 |
| `Fire` | rien : un `Place` suffit |
| `Grass`, `DirtPath` | l'herbe posée en terre en construction : un état, pas une API |
| `Door`, `Bed`, `DoublePlant` | deux cellules et une moitié : le port a `Verdict::Writes(Vec<Cell>)` et `double()` — **portables en une heure chacun** |
| `SpecialBlock` | `PASS` : rien |
| `FlowerPot` | retirer le pot en place avant de poser le pot plein (`:686-689`) |
| `Hopper` | `!world.hasNeighborSignal(pos)` : une lecture de redstone que l'API n'a pas |
| `DripStone` | écrire une **colonne** de stalactites autour de la pointe (`:1091-1111`) |
| `FallingBlock` | poser un support sous le bloc avant lui (`:381-390`) |
| `Banner` | toujours `false` en comparaison (`:1036-1038`), pour forcer la réécriture |
| `Container` | charger l'inventaire du blueprint dans l'entité de bloc : `edit_blocks` ne prend **aucune** donnée (`world_edit.rs:12-14`) — c'est la perte la plus lourde du port |

## 4. Les ticks

`DimensionFluidHandler.java:82` programme un tick de fluide
(`world.scheduleTick(pos, type, getTickDelay(world))`) pour que l'eau posée s'écoule. L'API
n'a aucun appel de tick planifié : l'eau d'un blueprint reste immobile jusqu'à ce qu'un
voisin la réveille. Le port écrit l'eau quand même, et le dit dans
`handlers::dimension_fluid`.

## 5. Le coût (`getRequiredItems`)

Chaque handler Java porte une troisième méthode, `getRequiredItems`, un tiers des 2 148
lignes du dossier. Elle n'est pas portée : c'est le **coût** d'une structure, et
`crate::building::cost` le tient déjà. Les deux règles qui ne sont pas de simples listes
d'items — une ruche déjà en place est gratuite (`BeehivePlacementHandler.java:57-60`), un rack
déjà en place aussi (`RackPlacementHandler.java:59-62`), dans les deux cas seulement en
construction — appartiennent au même endroit et sont notées ici pour que `cost` ne les
oublie pas.

## 6. Le dossier d'un bâtiment

`HutPlacementHandler.java:68-99` écrit dans la donnée de la hutte : le chemin complet du
blueprint pour la cellule d'ancrage, le fichier de niveau 1 pour une hutte secondaire, le nom
du pack pour toutes. C'est le dossier d'un bâtiment, que `crate::building` tient ; le port lui
donne `Request::placing` (le chemin est `Placing::id`) et la comparaison `at == Placing::at`
(l'ancrage). Rien n'est réimplémenté, et c'est volontaire : deux écritures du même chemin
divergeraient.
