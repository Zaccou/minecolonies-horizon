# `src/items` — les items de MineColonies

Ce que chaque item **est**, combien tiennent dans une pile, ce que les outils et les
armures valent, ce que les 79 mangeables nourrissent, et à quoi servent les matériaux.
Aucune IA, aucune entité, aucun tick : que des tables `&'static`, six formules et quatre
familles de refus nommés.

Sources Java : `src/main/java/com/minecolonies/` du dépôt MineColonies. Chaque ligne du
port cite `fichier.java:ligne` au-dessus d'elle, et chaque valeur de table porte sa propre
référence dans son champ `source`.

## 1. Ce qui est porté — le compte exact

| Fichier | Question | Lignes | Tests |
| --- | --- | ---: | ---: |
| `mod.rs` | les 163 items : catégorie, pile, gabarit de déclaration | 1097 | 20 |
| `tools.rs` | 17 outils : durabilité, allonge, dégâts, points d'armure | 834 | 20 |
| `food.rs` | 79 mangeables : faim, saturation, palier, effets, maladie | 942 | 27 |
| `materials.rs` | 14 matériaux et 4 bouteilles : ce qui les fait, ce qu'ils font | 632 | 18 |
| **Total** | | **3505** | **85** |

**163 items**, en deux moitiés qui ne viennent pas du même fichier :

- **148** sont enregistrés par `ModItemsInitializer` — les **130** champs `Item` de
  `api/items/ModItems.java` **plus 18 œufs** construits à la volée (`:386-463`), 130 + 18 = 148 ;
- **15** sont les cultures, des `ItemCrop` que leur **bloc** enregistre
  (`MinecoloniesCropBlock.java:229`), donc qu'aucune ligne de l'initialiseur d'items ne
  nomme. Leur moitié *bloc* reste dans `src/blocks/ressource.rs`.

148 + 15 = 163, et `tests::the_table_holds_the_one_hundred_and_sixty_three_items` tient les
deux moitiés séparément (148 lignes citant `ModItemsInitializer.java`, 15 citant
`MinecoloniesCropBlock.java:229`).

Le test central, `tests::the_table_holds_the_one_hundred_and_sixty_three_items`,
tient le compte dans les deux sens ; `every_food_row_has_a_food_rule_and_every_other_row_does_not`,
`every_tool_row_that_has_rules_is_a_tool_and_the_reverse_holds` et
`the_materials_are_the_fourteen_the_java_lists_name` tiennent la cohérence entre table et
sous-modules (aucun item de catégorie `Food` sans règle, aucun non-`Tool` avec une règle).

## 2. Chaque item du Java a une entrée — ou une ligne ici

| Famille Java | Où | Pourquoi |
| --- | --- | --- |
| les 148 items de `ModItemsInitializer` | `mod.rs`, une ligne chacun | la mission |
| les 15 cultures (`ItemCrop`) | `mod.rs` (catégorie `Food`) **et** `food.rs` (leurs nombres) | enregistrées par le **bloc** (`MinecoloniesCropBlock.registerBlockItem`, `:229`), pas par l'initialiseur d'items |
| les objets-blocs des huttes et des blocs de colonie (`ItemBlockHut`) | `src/blocks/huts.rs` | `AbstractColonyBlock.registerBlockItem` (`:301-303`) et `AbstractBlockHut.registerBlockItem` (`:350-353`) : l'item naît du bloc |
| les autres objets-blocs (`ItemCrop` des cultures, `barrel_block`, `colonysign`, `blockminecoloniesrack`, `blockpostbox`, `blockstash`, `blockwaypoint`, `decorationcontroller`, `composted_dirt`, `blockconstructiontape`, `blockhutfield`, `blockhutplantationfield`, `farmland`, `floodedfarmland`, les deux portes) | `src/blocks/*` (86 blocs) | `AbstractBlockMinecolonies.registerBlockItem` (`:17-21`) : même mécanisme |
| `gate_iron`, `gate_wood`, `colony_banner` | `mod.rs` (catégorie `Block`) | ce sont **aussi** des items de `ModItems` (`ModItemsInitializer.java:299-301`) |

Donc : **163 items** dans `ITEMS`, dont les 3 objets-blocs que l'initialiseur d'items
enregistre lui-même (`gate_iron`, `gate_wood`, `colony_banner`). Le reste des objets-blocs est
du ressort de `src/blocks`, qui déclare les 86 blocs et leurs items. Aucun item du dépôt Java
n'est sans endroit ; aucun n'est déclaré deux fois.

## 3. Les prémisses fausses ou imprécises de la mission

1. **« sa pile maximale » : Java ne la donne presque jamais.** Sur 148 items, **25**
   seulement disent `stacksTo(n)` (5 parchemins et le potion à 16, 7 outils, le drapeau et
   les 2 déployeurs, 11 matériaux à 64). **26** disent une durabilité et aucune taille de
   pile — c'est l'**engine** qui les met à 1. **97** ne disent rien du tout. Écrire `64`
   partout aurait été un chiffre inventé qui ressemble à un fait : `StackLimit` distingue
   donc `Fixed(n)`, `Damageable` et `EngineDefault`, et `ItemEntry::stated_stack` rend
   `Ok(None)` (à demander à `Host::max_stack_size`) plutôt qu'une constante de confort.

2. **« les déclarations de `api/items/` » ne contiennent pas les chiffres.** `ModItems.java`
   ne déclare que des champs ; la nutrition, la saturation, le palier et les durabilités
   sont dans `apiimp/initializer/ModItemsInitializer.java:143-244` (les 64 `ItemFood`/
   `ItemBowlFood`/`ItemLargeBottle`) et les recettes dans
   `core/generation/defaults/DefaultRecipeProvider.java` plus les vingt-et-un
   `defaults/workers/*CraftingProvider.java`. Les fichiers de classes, eux, disent **quoi
   faire** (`finishUsingItem`, `getUseDuration`) mais pas les nombres.

3. **« les effets, dont ceux qui rendent malade » : aucun aliment de MineColonies ne rend
   malade.** Le tag `minecolonies:poisonousfood` existe (`api/items/ModTags.java:70`) et ne
   contient **que quatre items vanilla** — patate empoisonnée, poulet cru, œil d'araignée,
   chair de zombie (`DefaultItemTagsProvider.java:178`). Un citoyen à qui un joueur en donne
   attrape une **maladie aléatoire** (`EntityCitizen.java:492-499`), et il les refuse
   normalement (`FoodUtils.java:45`). Tout ce qui est négatif dans le mod est du côté des
   **parchemins** (cécité sur un `scroll_buff` raté, nausée sur un `scroll_tp` raté), pas de
   l'assiette. `food.rs` porte la règle réelle : `poisonousfood`, refus des cultures, et le
   seuil de palier (`level + 1` points de faim à partir du niveau 3).

4. **« la saturation » est un multiplicateur, pas des points.** `saturationMod(0.1F)` avec 6
   points de faim donne **1,2** point de saturation (`nutrition × mod × 2`), et le
   `chorus_bread` à `2.0F` en donne **20**. C'est de l'arithmétique vanilla
   (`net.minecraft.world.food.FoodData#add`) que ce dépôt ne contient pas : c'est écrit dans
   `MANQUANT.md` §3 et non présenté comme une ligne MineColonies.

5. **Le porcher ne produit pas de compost**, contrairement à ce qu'une relecture pressée
   laisse croire : son IA est un éleveur ordinaire (`EntityAIWorkSwineHerder.java:13`) et son
   bâtiment ne mange que des **carottes** (`BuildingSwineHerder.java:55-57`). Le seul
   producteur est le compostier, six par recette (`api/crafting/CompostRecipe.java:31`
   `COMPOST_RESULT = 6`, lu en sortie `:44`), ce que `kinds::ferme.rs:208` porte déjà. Un test
   attendait deux producteurs ; c'est l'attente qui était fausse, et le test le dit maintenant
   avec les trois lignes ci-dessus. `kinds::ferme.rs:541` — la ligne qu'on aurait pu citer —
   est un **test** du module des métiers, pas une ligne de table.

6. **La cible de 900 à 1 300 lignes n'est pas tenable pour ce périmètre**, et voici
   l'arithmétique plutôt qu'une excuse : 163 + 79 + 13 + 14 + 5 + 5 lignes de table (≈ 280),
   les 85 tests (≈ 1 000), les constantes nommées avec leur ligne (≈ 60), les formules
   (≈ 80). Le module fait **3 505 lignes dont 891 de commentaires** : il n'y a pas de
   remplissage, chaque ligne est un fait, une constante ou une assertion. Descendre à 1 300
   voudrait dire supprimer soit la table (163 items), soit les tests (la mission en exige 45).
   Le choix fait : garder les deux, et le dire ici.

## 4. Les gabarits de déclaration : cinq, et aucun deviné

Une déclaration de contenu **exige** un `template` (`content::Item.template` est un
`String`). Cinq items en portent un, et chaque fois parce que la **superclasse Java** le
nomme :

| Item | Gabarit | Ce qui le force |
| --- | --- | --- |
| `chiefsword` | `minecraft:diamond_sword` | `SwordItem(Tiers.DIAMOND, …)` (`ItemChiefSword.java:33`) |
| `iron_scimitar` | `minecraft:iron_sword` | `SwordItem(Tiers.IRON, …)` (`ItemIronScimitar.java:19`) |
| `spear` | `minecraft:trident` | `extends TridentItem` (`ItemSpear.java:26`) |
| `pharaoscepter` | `minecraft:bow` | `extends BowItem` (`ItemPharaoScepter.java:27`) |
| `firearrow` | `minecraft:arrow` | `extends ArrowItem` (`ItemFireArrow.java:17`) |

Les 143 autres rendent `Err(NoTemplate)` : leur gabarit est une décision d'**asset** (de
quel modèle l'item est le modèle), et choisir `minecraft:paper` pour le presse-papier
aurait fabriqué une règle à partir d'un goût. C'est le refus nommé, pas un `unwrap`.

## 5. Comment ces tables ont été vérifiées

Pas de compilation (règle 2), donc la vérification est **mécanique, contre les sources
Java** :

- les **148** couples (id, ligne d'enregistrement) relus dans
  `ModItemsInitializer.java` : 148/148 identiques, dans l'ordre du fichier ;
- les **60** lignes de `food.rs` (faim, saturation, palier, `ItemFood`/`ItemBowlFood`) relues
  contre les 60 affectations de l'initialiseur : 60/60 identiques ;
- les **15** cultures contre `ModBlocksInitializer.java:132-147` : noms et ordre identiques ;
- les **durabilités et protections** d'armure recalculées à la main
  (`unit × multiplier`, `MineColoniesArmorMaterial.java:21-26`, `:59`) et vérifiées par
  assertion (407/592/555/481 pour la plaque) ;
- les **15 fichiers de recettes** cités dans `materials.rs` ouverts ligne par ligne : chaque
  `inputs`/`requires`/`result` cité existe à la ligne citée ;
- les **37 constantes** de `tools.rs` relues dans leur classe (durabilités, portées,
  dégâts, effets), y compris les deux seules valeurs vanilla assumées
  (`Item.getUseDuration` = 32, `Tiers` sans nombre).

Les tests qui comptent ne comparent pas le port à lui-même : ils comparent le port à
`crate::blocks` (les 3 objets-blocs et les 15 cultures existent comme blocs),
`crate::jobs::kinds` (le fermier, la fleuriste et le planteur prennent le compost ; le
compostier et le porcher en produisent 6 ; l'alchimiste prend le gui ; le druide la potion
magique) et `crate::jobs::tool` (aucun des 17 outils n'est une des 14 sortes d'équipement,
sauf la lance qui est « sans niveau »).

## 6. Ce que le module ne fait pas

- **aucune déclaration écrite** : `ItemEntry::declaration()` rend un `mod_api::Item` ; les
  148 fichiers `data/minecolonies/horizon/item/*.json` sont du contenu, pas du code (règle 1).
- **aucun hook** : `item_use`, `item_use_on`, `item_use_finish`, `attack` ne sont pas
  souscrits par `src/lib.rs` (autre agent). Ce module porte les nombres et les refus que ces
  hooks liront ; il ne décide pas *quand*.
- **aucune recette** : `data/minecolonies/recipe/` en contient déjà 154, et `MANQUANTS.md` à
  la racine dit lesquelles nomment encore un item sans déclaration.
- **aucun modèle, aucune texture** : `assets/` est une autre tranche.
- **aucune maladie** : `DiseasesListener` est un registre de données et
  `CitizenDiseaseHandler` un tick de citoyen ; `food.rs` ne porte que la porte d'entrée
  (`poisonousfood`, palier).
