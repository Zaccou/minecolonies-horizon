# MANQUANT — ce que `src/recipes_content/` demande et que le port n'a pas

Écrit par l'agent qui a porté le contenu des recettes et la migration des blocs.
Tout ce qui suit est vérifié dans `.horizon/sdk/api/crates/`, dans
`/workspace/horizon/crates/horizon-client/src/` (l'arbre épinglé) ou dans
`minecolonies-src/`. Rien n'est supposé. Ce fichier ne modifie aucun autre
module : il nomme ce qu'il faudrait y ajouter.

---

## 1. Le hook `ingredient` n'existe pas dans ce port — trois des quatre fichiers sont jetés au chargement

**C'est le vrai blocage, et ce n'est pas celui qui était écrit.**

La note reprise un peu partout dit qu'un `type` de recette inconnu fait refuser
**toute** la table. C'est faux dans l'arbre épinglé :

- `Recipes::from_resolved` garde un fichier dont le `type` n'est pas
  `minecraft:` comme un *snapshot* de son type
  (`crates/horizon-client/src/crafting.rs:1625-1643`), et un mod le relit par
  `Host::recipes_of_type`
  (`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/recipes.rs:29`) ;
- donc **un** fichier `minecolonies:composting` dans `data/minecolonies/recipe/`
  ne coûte rien aux 154 autres.

Ce que le moteur refuse vraiment, c'est un fichier qui nomme un **type
d'ingrédient** que personne ne traite :

| Comportement | Ligne | Effet |
| --- | --- | --- |
| aucun mod n'écoute le hook `ingredient` | `crafting.rs:1521-1527` | le fichier est **jeté**, avec un log nommant le fichier et le kind |
| un mod écoute mais rend une liste vide | `crafting.rs:1529-1532` | le fichier est **gardé**, mais non plaçable |

Les trois `compostables*.json` écrivent `{"type": "minecolonies:food"}` et
`{"type": "minecolonies:plant"}` — les deux sérialiseurs d'ingrédient que Java
enregistre à `MineColonies.java:269-270` (`FoodIngredient.ID`
`core/recipes/FoodIngredient.java:50`, `PlantIngredient.ID`
`core/recipes/PlantIngredient.java:32`). `soy_milk.json` n'en écrit aucun : il
traverse le chargement même avec zéro enregistrement.

**Ce qu'il faudrait.** Implémenter `Hooks::ingredient`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/hooks/ingredient.rs:1-36`,
`catalog-order 0226`, `contract = "collect"`, `out = "items"`,
`context = "data_load"`) dans `src/lib.rs` — qui n'appartient pas à ce module —
et remplir `items` avec les ids d'items qui satisfont chaque kind :

- `minecolonies:food` : les bornes `min-healing` / `max-healing` /
  `min-saturation` / `max-saturation` (`FoodIngredient.java:54-55`, `:134-145`)
  se lisent sur le composant `minecraft:food` de chaque item
  (`Host::item_default_component`, `host/items.rs:30`) ;
- `minecolonies:plant` : un item-blocs dont le bloc est `CropBlock` / `StemBlock`
  (`PlantIngredient.java:36-39`) — donc une question de bloc, pas d'item.

La signature exacte du hook n'est **pas** recopiée ici : elle est engendrée dans
le trait `Hooks`, et l'écrire de mémoire serait l'inventer. `ingredient.rs` de
`src/crafting/` décrit déjà la sémantique (`Ingredient::Food`, `Ingredient::Plant`,
`Facts`), donc la valeur est prête ; c'est le câblage qui manque.

## 2. `zero_waste` charge, mais ne se fabrique pas

`src/crafting/register.rs` enregistre
`register_raw_recipe_serializer("minecolonies:zero_waste", "minecraft:crafting")`.
Un sérialiseur brut **décode** sans **matcher** : `Raw::matches` rend `false` et
`Raw::assemble` rend `None`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/parts/recipe_serializers.rs:36-52`).
Conséquence mesurable : `soy_milk.json` est lu, typé et relisible, mais l'établi
ne le reconnaît pas et ne jette pas la bouteille vide — or c'est exactement ce
que fait `ZeroWasteRecipe.getRemainingItems` (`ZeroWasteRecipe.java:47-54`).

**Il faudrait** un `CraftingRecipeKind` avec `matches`, `assemble` et
`remaining_items`
(`parts/crafting_recipes.rs:79-106`) et un codec Wire pour la valeur
(`CraftingRecipeCodec::wire`, `parts/crafting_recipes.rs:58-74`).
`ZeroWaste::matches` et `ZeroWaste::remainders` existent déjà : c'est un
`From<&ZeroWaste>` et rien de plus.

## 3. `composting` n'est lu par personne en jeu

Les trois fichiers sont un barème, pas une recette : `CompostRecipe` construit son
résultat dans le constructeur (`CompostRecipe.java:31`, `:44`) et le fichier ne
nomme que `input` et `strength`. `Composting::per_item`, `::output` et
`::ferment_time` répondent, mais aucune station ne les appelle : Java a
`AbstractTileEntityBarrel` (`MAX_ITEMS = 64`, `:14`).

**Il faudrait** une station (bloc + `[[native.loop]]` de tick) qui lit
`Composting::per_item` et rend `Composting::output`, ou un `[[native.mixin]]` sur
le baril de vanilla. Hors de `src/recipes_content/`.

## 4. L'oracle du vérificateur n'est pas câblé — mais il n'y a pas d'API à ajouter

`plan::Registry` demande une seule chose : « ce build connaît-il cet id ? ». Le
SDK l'a, en deux appels d'hôte, sans rien de neuf :

| Surface | Appel | Fichier |
| --- | --- | --- |
| bloc (palette de blueprint) | `Host::block_state_id(name) -> Option<BlockState>` | `host/block_states.rs:25` |
| item (ingrédient de recette) | `Host::item_key(&Identifier) -> Option<ItemKey>` | `host/items.rs:22` |

`plan::Registry` est implémenté d'office pour tout `compat::StateOracle`, donc
l'adaptateur tient en un `impl StateOracle` de trois lignes. Il vit **hors** de ce
module (il lui faut un `Host`), et c'est pour ça qu'il n'est pas écrit ici.

En test, et seulement en test, la source est l'export du client épinglé
(`.local/horizon-client/26.3/registries.json`) : les tests qui en dépendent
sautent quand l'arbre de build n'est pas sur la machine, ils n'échouent pas.

## 5. Limite assumée de `Excluded::readiness`

`readiness` prend la parole de l'appelant sur deux points : les sérialiseurs
enregistrés, et les kinds que son hook `ingredient` sait remplir. Elle ne peut pas
savoir si la liste rendue pour un kind est vide — c'est le comportement
`crafting.rs:1529-1532` (gardé, non plaçable), et le modèle le représente comme
`Loads::Unplaceable` seulement si l'appelant dit ne pas connaître le kind. Un
kind *connu* mais rendu vide est donc annoncé `Snapshot` alors que le moteur
avertira. Le distinguo demanderait de faire passer la taille de la liste, pas
seulement son nom.

## 6. Rien ne manque pour les renommages eux-mêmes

Les deux lignes de `compat` sont la seule source ; `src/recipes_content/` les
applique sans les recopier, et deux tests empêchent la dérive :

- `renames::RECIPE_USES` doit contenir exactement les ids de `compat::RENAMES`
  (un id mesuré sans ligne, ou une ligne sans mesure, casse le test) ;
- le côté **item** est prouvé par la section `["minecraft:item"]["entries"]`
  (1 658 clés, digest `0x2f63516e0e392bd8`) : `short_grass` et `iron_chain` y sont,
  `grass` et `chain` n'y sont plus.

Si un troisième bloc manque un jour, c'est `compat` qui le dit d'abord
(`table.rs::the_whole_pack_gives_the_recorded_numbers`), et cette table-ci doit
alors gagner une ligne — le test d'égalité des deux ensembles le rappelle.
