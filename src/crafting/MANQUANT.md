# MANQUANT — ce que `src/crafting/` demande et que le port n'a pas

Écrit par l'agent qui a porté les types de recettes propres à MineColonies. Tout ce
qui suit est vérifié dans `minecolonies-src/` ou dans le SDK
`.horizon/sdk/api/crates/` ; rien n'est supposé.

---

## 1. Une ligne reste à écrire dans `src/lib.rs` : l'appel au `register`

`src/lib.rs` appartient à l'orchestrateur, pas à `src/crafting/`. Le module **est
maintenant déclaré** (`pub mod crafting;`, `src/lib.rs:50`), donc tout ce fichier
compile et ses tests tournent. Il reste **un** appel à écrire, dans
`Hooks::register` (le seul hook où un mod déclare quoi que ce soit,
`src/lib.rs:85-96`) — vérifié le 16/11/2026 : aucune ligne de `src/lib.rs` ne cite
`crafting::` hors de la déclaration :

```rust
crafting::register::register(registrar);
```

Cette déclaration a été trouvée **recommantée** dans l'arbre de travail le 16/11 à
23h20 (`// pub mod crafting;`) alors que `HEAD` la déclare : elle a été remise telle
que `HEAD` la porte. Ce n'était pas cosmétique — `src/recipes_content/excluded.rs:57`
importe `crate::crafting`, donc le module commenté cassait un autre dossier que le
mien.

Tant qu'il manque, les quatre fichiers bloqués ne sont pas déclarés : `composting`
et `zero_waste` restent dans `recettes-custom/` et ne chargent pas.

`register` fait exactement ce que fait
`apiimp/initializer/ModRecipeSerializerInitializer.java:19-25` :

| Appel | Effet |
| --- | --- |
| `register_recipe_type("minecolonies:composting")` | déclare le type de recette propre au compost (`:22`) |
| `register_raw_recipe_serializer("minecolonies:composting", "minecolonies:composting")` | charge les 3 fichiers |
| `register_raw_recipe_serializer("minecolonies:zero_waste", "minecraft:crafting")` | charge `soy_milk.json` (pas de type propre : `ZeroWasteRecipe` étend `ShapelessRecipe`) |

Sans ces trois appels, **un seul** des quatre fichiers dans
`data/minecolonies/recipe/` ferait refuser **toute** la table de recettes
(`crates/horizon-client/src/crafting.rs:1622-1632`).

**L'import n'est pas nécessaire, c'est vérifié par compilation.**
`register_recipe_type` est une méthode de `RecipeSerializersRegistrar`
(`horizon-mod-vocab/src/parts/crafting_recipes.rs:181`), et `Registrar` en hérite
comme supertrait (`horizon-mod-vocab/generated/parts_modules.rs:82` liste
`+ crate::parts::RecipeSerializersRegistrar` parmi les siens) : les méthodes d'un
supertrait s'appellent sur `&mut dyn Registrar` sans `use` séparé. Le
`use mod_api::RecipeSerializersRegistrar;` a été retiré de `register.rs` pour cette
raison ; `register_raw_recipe_serializer` est une méthode inhérente à
`impl dyn Registrar` (`parts/recipe_serializers.rs:53-70`) et n'a jamais demandé
d'import.

---

## 2. `register_raw_recipe_serializer` charge, mais ne fait rien

C'est **la** limite à connaître de ce lot. Un serialiseur brut garde le JSON du
fichier comme texte et ne matche rien : `Raw::matches` rend `false`,
`Raw::assemble` rend `None` (`parts/recipe_serializers.rs:36-52`).

Ce qui marche avec ce lot, une fois le §1 fait :

- les 4 fichiers **chargent** (c'était le blocage fatal) ;
- ils sont lus typés par `crate::crafting::resolve`, donc une station de compostage
  ou un test peut les interroger.

Ce qui ne marche pas encore :

| Manque | Pourquoi | Ce qu'il faudrait |
| --- | --- | --- |
| `zero_waste` ne se fabrique pas dans un établi | pas de `matches`/`assemble` | un `CraftingRecipeKind` (`parts/crafting_recipes.rs:79-106`) avec `matches`, `assemble` et `remaining_items` câblés sur `ZeroWaste::matches` et `ZeroWaste::remainders` |
| l'établi ne rend pas les bouteilles vides quand il ne connaît pas la recette | idem | idem (`remaining_items` est ce qui distingue `zero_waste` d'un shapeless) |
| `composting` n'est lu par personne en jeu | Java a un baril (`AbstractTileEntityBarrel`) et un `RecipeType` à lui | une station qui lit `Composting::per_item` et `Composting::output` |

Le `CraftingRecipeKind` exige un codec Wire pour sa valeur
(`CraftingRecipeCodec::wire`, `parts/crafting_recipes.rs:58-74`), donc une valeur
dérivable : `#[derive(mod_api::Wire)]` est bien la voie documentée du SDK
(`generated/mod_api_exports_root.rs:136`), mais elle demande de dériver `Wire` sur
un type à plat (des `String` et des `u32`, pas `ItemId` qui enveloppe `ItemName`).
C'est un fichier de plus, pas une réécriture : `ZeroWaste` ne bouge pas, on ajoute
un jumeau Wire et un `From<&ZeroWaste>`.

---

## 3. `Facts` n'a pas d'implémentation : trois appels d'hôte

`crate::crafting::Facts` est le seul endroit où une recette a besoin du jeu. Ses
trois questions ont chacune une réponse dans le SDK, aucune n'est inventée :

| Question | Appel d'hôte | Source |
| --- | --- | --- |
| `in_tag(tag, item)` | `host.tags().key(&id)` puis `Tags::is_in_tag(item_key, tag_key)` | `crates/horizon-mod-vocab/src/world/tags.rs:28`, `:35` |
| `food(item)` | `Host::item_default_component(item_key, "minecraft:food")` → le compound `nutrition` / `saturation` | `crates/horizon-mod-vocab/src/host/items.rs:30` |
| `crop(item)` | aucun appel direct : Java teste `BlockItem instanceof CropBlock \|\| StemBlock` (`PlantIngredient.java:36-39`) ; le SDK n'expose pas la classe du bloc d'un item | **manquant**, voir ci-dessous |

**`crop` est le vrai trou.** `Host::item_key` donne une clé, pas un bloc ; il n'y a
pas de « le bloc de cet item est-il une culture ? » dans l'API. Deux voies :
`BlockItem` → id de bloc, puis une liste des blocs-cultures (vanille : blé, carottes,
pommes de terre, betterave, courges, pastèque, nether wart, chorus, sweet berry…)
tenue par le port, ou une demande d'API (`item_block` par exemple). Tant que ce
n'est pas tranché, `PlantIngredient` ne matche **rien** : `compostables.json` perd
son deuxième membre, donc tout légume compostable n'est compostable que s'il tombe
dans une plage de saturation ou dans le tag `minecolonies:compostables`.

Le food marche mais avec une nuance de 26.3 : le composant `minecraft:food` porte
`nutrition` (entier) et `saturation` (flottant, **le modificateur**, l'ancien
`getSaturationModifier()`). C'est exactement ce que `Food { nutrition, saturation }`
attend.

---

## 4. Le NBT d'un id d'item reste du texte

`ItemId` garde le suffixe `{...}` d'un id comme texte, sans le parser :
`horizon_core::nbt::parse_snbt` existe
(`crates/horizon-core/src/nbt.rs:1110`) mais **n'est pas réexporté** aux mods
(`generated/mod_api_exports_root.rs:144-145` n'exporte que `Compound` et `Tag`).

Conséquence : un id `minecolonies:potion{Magic:1}` s'analyse et se compare
(correspondance exacte du texte, ce que fait
`ItemStackUtils.compareItemStacksIgnoreStackSize(..., ignoreNBT = false)`), mais il
ne devient **pas** un `ItemStack`. Pour cela il faut `Host::item_from_snbt`
(`host/items.rs:64`), qui prend la chaîne SNBT seule — donc l'appelant doit retirer
le préfixe d'item avant. Ou demander l'export de `parse_snbt` au SDK.

Aucun des 4 fichiers bloqués n'a de NBT : c'est un manque pour la suite
(`crafterrecipes/`), pas pour ce lot.

---

## 5. Les 163 fichiers `crafterrecipes/` ne sont pas portés

Compté, pas estimé :

| Où | Combien |
| --- | ---: |
| `minecolonies-src/src/datagen/generated/minecolonies/data/minecolonies/crafterrecipes/` | **161** |
| `minecolonies-src/src/main/resources/data/minecolonies/crafterrecipes/` (écrits à la main) | **2** (`lumberjack/strip_logs.json`, `strip_stems.json`, tous deux `recipe-template`) |
| portés dans `data/minecolonies/…` | **0** |

`src/crafting/crafter.rs` porte le format (les champs, la validation, l'expansion
des templates) mais **pas les données**. La conversion est un autre travail, et
`convertir_recettes.py` ne sait pas le faire : il lit
`src/datagen/generated/minecolonies/data/minecolonies/recipes` (`:33-35`) et écrit
`data/minecolonies/recipe/`, alors qu'un fichier de crafter va dans un dossier
`crafterrecipes/` que le jeu ne charge pas du tout (ce n'est pas un
`RecipeSerializer`, c'est `CrafterRecipeListener`). Un convertisseur pour ces 163
fichiers doit donc :

1. les copier dans un dossier de données que le port lira lui-même (aucun chargeur
   de `crafterrecipes/` n'existe côté Horizon : `RecipeSerializer::decode` ne reçoit
   que les fichiers `recipe/`), ou les transformer en table Rust ;
2. résoudre `result` (une chaîne) vs `result` (un objet `{item, count}`) selon le
   type — les deux formats coexistent dans la source ;
3. pour les 2 templates, fournir la liste des membres de `minecraft:logs` (le tag
   appartient au jeu) ;
4. garder les 16 clés de crafter telles quelles (`baker_smelting`, …), c'est
   l'identité que `CustomRecipeManager` utilise.

Les 16 clés et leurs nombres de fichiers sont dans la doc de `crafter.rs`.

---

## 6. Ce que ce lot ne fait pas exprès

- **`CustomRecipeManager`** (le magasin par crafter et par item produit, les
  `remove` appliqués, `CustomRecipeManager.java:87-119`) : c'est un magasin d'état,
  pas un type. `Removal` porte le fichier, rien ne l'applique.
- **La synchronisation client** (35 champs sur un `FriendlyByteBuf`,
  `CustomRecipe.java:880-980`) : elle a besoin d'un paquet, donc du réseau.
- **L'exécution d'une fabrication** : `ZeroWaste::matches` et
  `ZeroWaste::remainders` disent quoi faire ; les appliquer demande des slots, donc
  `crate::inventory`.
- **Le matching des recettes de crafter** : aucune fonction ne répond « quelle
  recette pour cette grille » — Java le fait avec `RecipeStorage` et un vrai
  inventaire.

---

## 7. Une prémisse du brief qui est fausse, et deux quirks de la source

1. **« les `crafterrecipes/` de la source » ne sont pas 4 fichiers** : il y en a
   **163**, et aucun n'est porté. Les 4 fichiers bloqués sont des recettes
   ordinaires (`recipes/`) avec un `type` maison, ce qui n'a rien à voir avec le
   dossier `crafterrecipes/`. Les deux chantiers sont indépendants.
2. `recipe-multi-in` est déclaré (`CustomRecipe.java:62`) et **jamais traité** :
   le `switch` de `CrafterRecipeListener.java:50-63` n'a pas de cas pour lui et pas
   de `default`, donc un fichier de ce type est ignoré sans une ligne de log.
   `Kind::handled()` le dit ; aucun fichier livré ne l'utilise.
3. `not-research-id` est un **ET** : `allMatch` puis négation
   (`CustomRecipe.java:726-731`), donc il faut que **toutes** les recherches
   exclues soient débloquées pour désactiver une recette. Ça ressemble à un bug de
   la source ; le port garde le comportement et le signale (test
   `one_exclusion_is_enough_to_disable_and_two_are_not`). À trancher en amont, pas
   ici.

Il y a aussi une chose que j'ai volontairement faite **plus stricte** que Java,
parce que la coercition cache une faute de données et qu'aucun fichier livré n'en
dépend :

| Cas | Java | Ici |
| --- | --- | --- |
| un nombre là où un texte est attendu | `getAsString` le convertit | `Refusal::WrongType` |
| un id d'item avec trois segments (`a:b:c`) | log une erreur et garde `a:b` | `Refusal::UnparsableItem` |
| `composting` avec `strength: 0` | `ArithmeticException` au premier tick du baril | `Refusal::StrengthNotPositive`, au chargement |
| `remove` sans `recipe-id-to-remove` | `new ResourceLocation("")` **lève**, et le `catch` n'attrape que `JsonParseException` → tout le chargement des recettes de crafter avorte | `Refusal::MissingField`, le fichier seul est refusé |

---

## 8. Le bug de fidélité de `same_recipe`, et pourquoi il est clos

Signalé par l'agent précédent : `same_recipe` comparait l'`amount`, alors que Java
l'ignore. Mesuré sur la source, la règle est bien : **la quantité ne compte pas**.

`isPrecursorRecipeMissing` (`CustomRecipe.java:771-796`) reconnaît une recette
précurseur si sa source est la même **ou** si

```text
ItemStackUtils.compareItemStacksIgnoreStackSize(storage.getPrimaryOutput(), compareStorage.getPrimaryOutput(), false, true)
  && storage.getCleanedInput().containsAll(compareStorage.getCleanedInput())
  && compareStorage.getCleanedInput().containsAll(storage.getCleanedInput())
```

et un élément de ces listes est un `ItemStorage`, dont `equals` est
`compareItemStacksIgnoreStackSize(...)` (`ItemStorage.java:255-267`) : un stack de 1
laine et un de 3 laines sont **le même** élément. Un `Set` de ces `ItemStorage` ne
distingue donc pas les quantités.

L'état actuel de `crafter.rs` suit la source :

| Ce qui est comparé | Code | Java |
| --- | --- | --- |
| la sortie principale, nom seulement (ni count, ni NBT) | `same_recipe`, `mine.name() == theirs.name()` | `compareItemStacksIgnoreStackSize(..., false, true)` (`:783`) |
| les entrées comme **ensemble** (air retiré, doublons fondus, quantités ignorées) | `cleaned_items()` puis `==` | `containsAll` dans les deux sens (`:784-786`) |

Le test `the_same_recipe_ignores_counts_but_not_inputs` (`crafter.rs:998`) le
verrouille : 1 laine vs 3 laines avec des sorties à 1 vs 9 → même recette, et une
entrée en plus → pas la même. Ce qui reste hors de `same_recipe` est la **première**
branche de Java, l'égalité des `recipeSource` (`:782-783`) : le port la laisse à
l'appelant, qui tient `CrafterRecipe::id` (`with_id`, `:87-93` de
`CustomRecipeManager.java`), parce que deux fichiers distincts peuvent décrire la
même recette et que Java les reconnaît alors par la seconde branche.

