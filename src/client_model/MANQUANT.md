# Ce qui manque aux modèles de blocs et d'objets, et ce que je voulais faire

Chaque entrée dit **ce qui n'est pas possible**, **pourquoi** (avec la ligne), et
**quel code aurait été écrit**. Rien de tout cela n'est du code fictif dans les
`.rs` : ce fichier est la liste des trous.

Les références SDK sont relatives à
`.horizon/sdk/api/crates/horizon-mod-vocab/src/`.

---

## 1. `ModelData` : le rack retexture son modèle à l'exécution

**Ce que Java fait.** `TileEntityRack.getModelData` (`core/tileentities/TileEntityRack.java:756`,
`:766-774`) renvoie un `ModelData` portant `MATERIAL_TEXTURE_PROPERTY` et un
`MaterialTextureData` recalculé quand le contenu du rack change (`refreshTextureCache`).
`BlockMinecoloniesRack implements IMateriallyTexturedBlock`
(`core/blocks/BlockMinecoloniesRack.java:50`) : le modèle `blockrackfull` **échange ses
textures de planches** selon ce que le rack contient. C'est le seul `ModelData` de tout
le mod (3 occurrences dans `src/main/java`, aucune dans `client/model/`).

**Ce que l'API offre.** `host/client_render.rs` donne des handles de modèle
(`ModelRef`, résolus par `model_baking_completed`) et `BlockEntityDrawContext::model(model, tint)`
(`host/draw.rs`), plus `register_runtime_sprites` (`hooks/register_runtime_sprites.rs:8-27`)
pour pousser un PNG dans l'atlas bloc. **Aucun** point d'entrée ne prend « ce modèle, avec
ces textures remplacées pour cette entité de bloc » : un `ModelRef` est immuable une fois cuit.

**Ce que j'aurais écrit.** Un `register_runtime_sprites` pour chaque variante de planches du
rack, puis, dans `block_entity_submit`, un choix de `ModelRef` parmi les modèles déjà cuits
selon le `variant` **et** le contenu (`RackType` porte déjà `emptysingle` / `fullsingle` /
`empty` / `full` / `dontrender`, `api/blocks/types/RackType.java:11-15`) : le port couvre
les cinq modèles pilotés par l'état (`variants.rs`, `blockminecoloniesrack`, 20 sélecteurs,
poids 40/30/30) et **pas** le retexturage par le contenu.

**Ce que ça coûte.** Un rack plein de planches de chêne garde le modèle « plein » générique :
la silhouette est juste, la matière ne l'est pas.

---

## 2. `uvlock` sur un `x` ou un `z`

**Ce que Java fait.** `uvlock` verrouille la texture au monde pour **n'importe quelle**
rotation de variante ; le jeu calcule l'inverse de la rotation du plan de la face
(`BlockModelRotation`, `BlockElementRotation`).

**Ce que j'ai écrit.** `transforms::uv_locked` ne traite que le `y` : c'est le seul axe
qu'un fichier `uvlock` de ce mod utilise (mesuré : `blockconstructiontape`, `uvlock: true`,
`y` ∈ 0/90/180/270, `x` = 0, aucun autre fichier ne pose `uvlock`). Un `x: 90` + `uvlock`
serait rendu **sans** rotation d'UV, donc faux.

**Ce que j'aurais écrit.** Une table `(face, turns) -> (sens de l'UV dans le plan)`, dérivée
des six normales de `transforms::Face` : c'est faisable, mais aucun fichier du mod ne
l'exerce et une table non exercée est une table qui ment sans que personne ne le voie.

---

## 3. `rescale` sur une rotation d'élément

**Ce que Java fait.** `BlockElementRotation` porte un `rescale` : le jeu agrandit l'élément
pour qu'il reste dans le cube de la blockbench, avec un facteur calculé par le *baker*.

**Ce que j'ai écrit.** `transforms::ElementRotation::rescale` **enregistre** le drapeau et
`is_noop` dit qu'il ne change rien seul. Un seul élément du mod le pose (`y: 45`, 22
occurrences, le même élément recopié dans les huttes).

**Ce que j'aurais écrit.** Le facteur, si sa formule était dans la source du mod : elle est
dans le jeu. Le deviner produirait une échelle plausible et fausse.

---

## 4. La couche de rendu d'un bloc (`ItemBlockRenderTypes`)

**Ce que Java fait.** `core/event/ClientRegistryHandler.java:356-366` force
`RenderType.cutout()` sur le barrel, le rack, le decoration controller, la dirt compostée,
le waypoint, la farmland inondée, le scarecrow et **tous les crops**
(`Arrays.stream(ModBlocks.getCrops())`), et `cutout() | solid()` sur toutes les huttes.

**Ce que l'API offre.** Rien : la couche de rendu d'un terrain est une décision du jeu
(`client/terrain_draw.rs` décrit les passes `Solid`/`Cutout`/`Translucent`, il ne les
choisit pas par bloc). Un mod wasm n'a pas de point d'entrée pour la changer.

**Ce que ça coûte.** Sans `cutout`, une texture de culture (feuille sur fond transparent)
dessine son fond. C'est **le** trou visible de ce module, et il n'est pas dans le code de
modèle : il est dans la déclaration de couche, qui appartient à l'hôte.

**Ce que j'aurais écrit.** Un `hooks/block_render_layer.rs` (`fn block_render_layer(&self, block: BlockKey) -> RenderLayer`)
et 95 lignes de table ; c'est une **demande d'API**, pas un contournement.

---

## 5. Les chargeurs de modèle Forge

**Ce que Java fait.**
- `models/item/spear.json` : `"loader": "forge:separate_transforms"` avec une section
  `base` et une section `perspectives` - un chargeur **Forge**, pas vanilla.
- `models/item/spear_in_hand.json` : `"parent": "builtin/entity"` - le modèle d'entité
  spécial (le rendu en main passe par `SpearItemTileEntityRenderer`).
- `models/item/supplychestdeployer.json` : `"parent": "builtin/generated"` - un parent
  vanilla **retiré** en 1.21 (l'ancien `builtin/generated`).

**Ce que l'API offre.** `register_model_loaders` + `bake_model_loader`
(`hooks/bake_model_loader.rs`) pour un `loader` **à soi**, et `standalone_models` pour un
modèle composé (`Rotated`/`Combined`). Un `loader` nommé `forge:*` n'est pas à nous : le
jeu ne l'enregistre pas.

**Ce que j'ai écrit.** `spear` reste un modèle d'objet ordinaire : ses trois sections
(`base`, `perspectives`) ne sont pas lues, et `textures::Textures::parse` garde ce qu'il
comprend. `spear_in_hand` et `supplychestdeployer` sont des modèles d'objet comme les
autres.

**Ce que j'aurais écrit.** Une `StandaloneModelForm::Combined { id, parts }` pour la lance
(le manche + la pointe), et `register_item_models` pour choisir entre les deux formes -
les deux hooks existent, mais la lance n'est pas le sujet de cette tranche.

---

## 6. Ce qui, lui, se raccorde, et où

Ce ne sont pas des trous, ce sont les coutures ; elles sont nommées pour que
l'orchestrateur n'ait pas à les chercher.

| Ce que ce module produit | Le hook qui le consomme |
| --- | --- |
| `blockstate::Selection` (modèle + tours + uvlock) | `register_block_state_models` (`hooks/register_block_state_models.rs`) pour un type de définition à nous ; sinon le jeu lit les JSON lui-même |
| `variants::Family::models` par état | la donnée de contenu (`data/`, `assets/`) : la table est la **liste de contrôle** de ce que le `.hmod` doit livrer |
| `transforms::Displays` (8 contextes) | `register_item_models` + `item_model_property` quand la lance et les goggles seront portées |
| `textures::Textures::merged` | rien à raccorder : la fusion parent/enfant est faite par le jeu à la cuisson |

---

## 7. Les 91 modèles d'entité de `client/model/`

Le dossier source de la commande (95 fichiers, 9 168 lignes) est fait de
82 modèles de citoyens (`CitizenModel`), du mercenaire, de l'épouvantail, de la lance et
des 9 raiders : des squelettes animés, pas des modèles de blocs. Ils ne sont pas portés ici
et ne doivent pas l'être : `crate::render` porte déjà les **types** de modèle, leurs
textures et les marques d'état (`src/render/models.rs`, `states.rs`), et l'animation d'un
squelette appartient au moteur (`EntityRenderState`), pas à une règle testable.

Deux fichiers de ce dossier ne sont référencés par aucun blockstate ni aucun item :
`ScarecrowModel.java` (le bloc `blockScarecrow` est enregistré,
`ModBlocksInitializer.java:66`, **sans** `blockstates/scarecrow.json`) et `SpearModel.java`
(la lance est un item, son modèle est `models/item/spear.json`, §5). Mesuré : 95 fichiers
de blockstates, aucun ne s'appelle `scarecrow`.
