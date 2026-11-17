# Ce que cette génération n'écrit pas, et ce qu'il faudrait pour l'écrire

Mesuré le 17/11/2026 sur `data/` (le contenu livré, 154 recettes + 146 tables +
98 tags) et sur `minecolonies-src/src/datagen/generated/` (la sortie datagen
1.20.1 : 158 + 146 + 161 + 98 + 1). Chaque manque dit **ce qu'il coûte** et **ce
qui le débloquerait** — aucun n'est un « TODO » sans prix.

## 1. Les cinq tables de l'enchanter (5 fichiers de 146)

`DefaultEnchanterCraftingProvider.java:51-435` : les cinq niveaux du bâtiment sont
cinq **listes écrites à la main**, pas une boucle. Le niveau 2 répète les 22 livres
du niveau 1 avec un autre poids et une autre puissance (`:151-198`), le niveau 3
rajoute `fortune` à poids 1 et `raider_damage_enchant`, le niveau 4 **retire** les
livres de niveau 1, le niveau 5 met tout à poids 1. Les cinq lots mesurés sur le
contenu livré : 22, 44, 68, 69, 75 entrées.

Porter cela, c'est recopier 117 triplets `(livre, niveau, poids)` — du contenu, pas
une règle : aucune sous-structure ne permet de les déduire (le niveau 3 contient
`fortune` à poids 1 au milieu de 23 entrées à poids 15, et le niveau 5 remet tous
les livres à poids 1 dans un ordre qui n'est pas celui des niveaux précédents).
Le module refuse donc la famille par son nom (`loot::enchanter_refusal`) plutôt
que d'écrire une approximation : une table d'enchanter fausse change ce que
coûte un livre au marché d'une colonie, et rien ne le signalerait.

**Débloqué par** : la transcription des 5 lots, dans `loot.rs`, avec une
comparaison octet-à-octet contre
`data/minecolonies/loot_table/recipes/enchanter1..5.json` (les 5 fichiers sont
déjà livrés : la comparaison est donc entièrement faisable).

## 2. Une divergence assumée : le `additional-output` des voyages du nether

Le contenu livré **ne contient aucun `crafterrecipes/`** : `data/minecolonies/` n'a
ni `crafterrecipes/` ni `compatibility/`. Les seuls exemplaires sont dans la
sortie datagen 1.20.1, 161 fichiers.

Un champ de mes six recettes de voyage diverge, et **c'est moi qui ai tort** :

| Fait | Source Java | Ce que fait ce module |
| --- | --- | --- |
| `netherworker/tripN.json` → `additional-output` | `DefaultNetherWorkerLootProvider.java:241-247` construit la liste avec `LootTableAnalyzer.toDrops` sur la table du voyage ; l'analyseur **développe un jeton d'aventure en la table de butin du mob qu'il nomme** (`LootTableAnalyzer.java:171-180`, `expandAdventureToken` `:261-278`) et trie par probabilité (`:133`) | une seule entrée `minecolonies:adventure_token` |

Le pack livré (`trip1.json` → 13 butins bruts : rotten flesh, gunpowder, ghast
tear…) est donc **ce que le Java actuel écrit**, pas une version périmée — c'est
la version de ce module qui est fausse. Elle n'est pas corrigée ici parce que
l'expansion demande les six tables de butin vanilla (zombified piglin, magma cube,
hoglin, ghast, enderman, blaze), qui appartiennent au jeu et que ce crate n'a pas.

**Débloqué par** : les six tables de butin d'entité du jeu, lisibles par la
génération ; la règle est alors `drops = union des tables des jetons du niveau`,
triée par probabilité décroissante — exactement ce que l'analyseur fait.

Un second écart que le pack semblait montrer — `matchtype` sur l'entrée maille du
tamis — **n'existe pas** : `new ItemStorage(stack, true, false)` est
`(stack, ignoreDamage, ignoreNbt)` (`ItemStorage.java:90-96`), donc le NBT est
comparé et le champ n'est jamais écrit. Le module ne l'écrit plus, et la source,
le pack et lui sont d'accord.

## 3. Les 135 autres `crafterrecipes/` (contenu)

161 au total, 26 générés ici. Les 135 restants sont un métier par dossier
(`baker` 25, `chef` 40, `blacksmith`, `stonemason`, `farmer`, `dyer`,
`fletcher`, `alchemist`, `mechanic`, `crusher`, `concretemixer`, `lumberjack`,
`enchanter`) et chaque entrée est une recette manuelle : ingrédients, niveau
minimum, porte de recherche. Aucune règle ne les relie — le générateur Java est
littéralement une suite de `CustomRecipeBuilder.create(...)`.

Le **constructeur** est porté (`workers::CrafterRecipe`), donc la transcription
est mécanique : un appel par recette.

## 4. Les 89 fichiers de tags d'objets et de blocs (sur 98 livrés)

`DefaultItemTagsProvider.java` (596 lignes) et `DefaultBlockTagsProvider.java`
(232) écrivent 73 tags d'objets et 16 de blocs. Leur contenu est fait de listes :
les 16 couleurs de béton, les 16 de béton en poudre, les 70 blocs constructibles,
et surtout le trio `_ingredient` / `_product` / `_excluded` de quinze métiers, qui
décide de ce qu'un artisan accepte et de ce qu'il ne doit pas produire.

Ce module porte les tags où le **contenu est une règle** (4 biomes, 5 relations
d'entités, 3 drapeaux de dégâts = 12 fichiers) et laisse les 89 autres. Deux
faits mesurés méritent d'être gardés quand ils seront transcrits :

- `dangerousblocks.json` est `{"values": []}` : le tag existe **vide**, comme
  point d'accroche pour les packs — un générateur qui saute les tags vides le
  casse ;
- aucun des 98 fichiers ne porte `"replace": true`. Le seul `replace` du code est
  un `.remove(...)` sur le groupe tempéré (`DefaultBiomeTagsProvider.java:67`),
  qui retire trois tags du groupe et n'écrit rien.

**Débloqué par** : la transcription des listes ; la comparaison est directe, les
98 fichiers sont livrés.

## 5. Les 78 recettes qui ne sont pas des huttes ni des cuissons

Trois endroits, comptés sur le pack (154 fichiers livrés, 76 générés) :

| Endroit | Lignes | Fichiers |
|---|---|---|
| `buildOtherBlocks` | `:366-451` | 7 façonnées + 2 sans forme |
| `buildOtherItems` | `:546-1108` | 22 façonnées + 44 sans forme + les deux recettes de pépite du cimeterre (`:688-697`, `smelting` et `blasting`) |
| la grille restée dans `buildHutRecipes` | `:243-249` | le marteau d'assistant (`assistanthammer_gold`) |

Un bloc de baril, un coffre de départ, une armure de mailles, quarante plats : chaque
entrée est une grille ou une liste d'ingrédients écrite à la main.

Ce module écrit les 47 recettes de huttes au patron, les 11 grilles de blocs de colonie
et les 18 cuissons (76). Les 78 restantes sont livrées dans
`data/minecolonies/recipe/` et **lues** par `crate::recipes_content` : elles ne sont pas
perdues, elles ne sont pas régénérées.

Le marteau mérite sa ligne : c'est la **seule** recette de `buildHutRecipes` que ce
module ne génère pas, et la seule de cette méthode qui produit un **objet**
(`ModItems.assistantHammer_Gold`) et non un bloc de colonie. Le critère du module est
donc « un bloc de colonie », pas « une méthode Java » — et c'est ce qui met cette grille
avec les 77 autres plutôt qu'avec les 11.

## 6. Les lignes de `itemnbtmatching.json` (1 fichier)

La règle est portée et testée (`nbt::checked_keys`) : les clés du tag, `display`
pour un objet teignable, `Enchantments` pour un enchantable, `RepairCost` pour un
réparable, et **`Damage` retiré** — c'est le but du fichier, deux pioches d'usure
différente sont le même objet pour une colonie.

Les **lignes** viennent de `CraftingUtils.forEachCreativeTabItems`
(`ItemNbtCalculator.java:62`) : tous les objets de tous les onglets créatifs, plus
un `textureData` synthétique pour chaque bloc Domum Ornamentum (`:66-78`). Ce port
a la table des objets (`crate::items`) mais **pas le registre des onglets
créatifs**, ni celui de Domum. Le fichier n'est pas livré dans `data/`
(`DELIVERED` dit 0), donc rien ne le compare : `nbt::matching` prend les lignes en
argument et `nbt::refusal` explique le refus.

**Débloqué par** : un accès au registre d'objets (`RegistryRef` / `creative tab`)
dans l'API du SDK, ou une liste d'objets exportée. La règle, elle, est prête.

## 7. Rien n'est écrit sur le disque

`Pack::build()` rend 287 `File { path, json }`. Écrire les octets (indentation
Gson, tri des clés, `SHA1` du pack) est le travail du build du pack, dans
`src/lib.rs`, que ce module ne possède pas (règle 1 du brief). La forme des
valeurs est celle que `DataProvider.saveStable` produit — clés triées par
`serde_json`, flottants distincts des entiers — donc l'écriture est mécanique.

## 8. Les trois recettes de compost ne sont pas générées ici

`DefaultRecipeProvider.java:52-64` écrit trois recettes de force 2, 4 et 8. Leur
`input` est un tableau d'ingrédients fusionnés de types *custom*
(`minecolonies:food`, `minecolonies:plant`), que `CraftingHelper` fusionne — le
fichier produit n'est pas reconstructible sans le sérialiseur de ces types.

Elles vivent dans `recettes-custom/` et leur **lecture** est
`crate::crafting::compost` (forces, fourchette de saturation, compte par objet).
La génération n'ajoute rien : c'est le même fait, et le dupliquer ici aurait
créé deux vérités pour la même recette.

## 9. Un seul fichier livré contredit le code qui l'a écrit : `baked_cake.json`

Les trois recettes de gâteau sont produites par le même appel, avec
`RecipeCategory.FOOD` (`DefaultRecipeProvider.java:281-284`, `:311-314`,
`:341-344`). Le contenu livré donne :

| Fichier | `category` |
| --- | --- |
| `baked_cake.json` (four) | `blocks` |
| `baked_cake_smoking.json` | `food` |
| `baked_cake_campfire_cooking.json` | `food` |

C'est le **seul** des 76 fichiers de recettes que ce module écrit dont la sortie
diffère du contenu livré, et la différence est d'un champ. Le port écrit `food`
(ce que dit Java) et le test `the_delivered_baked_cake_is_the_one_file_that_says_blocks`
le prouve : il balaie les 76 fichiers, compare chacun à son homologue livré, et
exige que la liste des différences soit exactement `["baked_cake.json"]`. Un
générateur qui recopierait `blocks` passerait ce test en mentant.

## 10. Deux détails de sérialisation qui ne se devinent pas

Ils sont portés, mesurés, et méritent d'être écrits parce qu'ils se ressemblent
tout en produisant du JSON différent :

- **un ingrédient à un seul item est un objet, une alternative est un tableau** :
  `"B": {"item": …}` pour la hutch du boulanger, `"H": [{…},{…}]` pour l'épouvantail
  (`Ingredient.toJson`). Écrire un tableau à un élément casse les 47 recettes ;
- **un `bonus_rolls` constant est un nombre, un `bonus_rolls` uniforme est un
  objet**, même quand ses deux bornes sont égales : le sifter écrit `0.025`
  (`ConstantValue`), le voyage du Nether écrit
  `{"type":"minecraft:uniform","max":0.3,"min":0.3}` (`UniformGenerator.between(0.3F, 0.3F)`).
  Les cinq tables du Nether ne correspondaient pas tant que ce n'était pas séparé.
