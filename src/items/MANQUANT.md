# MANQUANT — ce que les items de MineColonies voudraient et que le port n'a pas

**Rien de manquant du SDK pour lire ces tables.** Le module n'appelle aucune API de l'hôte :
il n'utilise que `std`, `mod_api::{Item, Rarity}` (deux *types de contenu*, constructibles
sans hôte) et `crate::{blocks, building, jobs}`. `cargo horizon-mod sdk diff` reste vide.

Les manques ci-dessous sont ceux du **port**, et chacun est contourné par une valeur nommée,
jamais par une invention silencieuse.

## 1. Déclarer un item : c'est du contenu, et le contenu n'est pas dans ce module

**Ce que je voulais faire** : enregistrer les 163 items depuis `Hooks::register`, comme un
bloc se déclare avec `Registrar::block_key`.

**Ce qui existe** : `Registrar::block_key` pour les blocs
(`crates/horizon-mod-vocab/src/parts/blocks.rs:133`) et **aucun `Registrar::item`**. Un item
est une **déclaration de contenu** — `content::Item`
(`content/content_declarations.rs:244-266`) lue depuis
`data/<namespace>/horizon/item/<name>.json` (`template`, `max_stack_size`, `rarity`,
`components`, `crafting_remainder`, `block`, `arm_pose`).

**Ce que j'ai fait** : `ItemEntry::declaration()` rend le `mod_api::Item` que chaque ligne
demande (`max_stack_size`, `rarity`, `crafting_remainder`, `block`), et refuse par
`Refusal::NoTemplate` les 158 lignes dont le gabarit est une décision d'asset. Écrire les 163
fichiers JSON est la tranche *contenu* (règle 1 : pas de `data/`).

## 2. Aucun hook d'item n'est souscrit

**Ce que je voulais faire** : brancher `ItemAssistantHammer.useOn`, `ItemSpear.releaseUsing`,
`ItemCompost.useOn`, `ItemLargeBottle.use`/`interactLivingEntity`, les cinq parchemins.

**Ce qui existe** : `src/lib.rs` souscrit cinq hooks (`block_entity`, `right_click_block`,
`register_commands`, `run_mod_command`, `entity_join_level`) et rien de la famille `item_*`.
Côté SDK, les déclarations existent (`crates/horizon-mod-vocab/src/hooks/`) ; côté port, le
`Mod` de `src/lib.rs` appartient à un autre agent.

**Ce que j'ai fait** : les règles pures que ces hooks liront, sans les appeler —
`tools::may_assist`, `tools::spear_may_throw`, `materials::bottle_becomes`,
`food::citizen_can_eat`. Aucune n'a besoin d'un monde, d'un tick ni d'un `LevelRef`.

## 3. Trois nombres vanilla que ce dépôt ne contient pas

Trois valeurs sont de **vanilla**, pas de MineColonies, et sont donc déclarées comme telles
et non citées d'un fichier du dépôt :

| Valeur | D'où elle vient | Où elle sert ici |
| --- | --- | --- |
| `Item.getUseDuration` d'un mangeable = **32 ticks** | `net.minecraft.world.item.Item` | `food::ITEM_USE_TICKS`, multiplié par le palier (`ItemFood.java:75`) |
| la saturation ajoutée = `nutrition × saturationMod × 2` | `net.minecraft.world.food.FoodData#add` | `food::saturation_points` |
| la durabilité d'une épée de `Tiers.DIAMOND` / `Tiers.IRON` (1561 / 250) | `net.minecraft.world.item.Tiers` | **pas** écrite : `ToolStats.durability` reste `None` pour les deux épées, et `tier` dit laquelle |

Les deux premières sont assumées dans le code avec un commentaire qui nomme la classe
vanilla ; la troisième n'est **pas** écrite du tout, parce qu'un chiffre absent est plus
honnête qu'un chiffre deviné. Une assertion vérifie que la durabilité des deux épées est
`None`.

## 4. Un `ItemName` `const`

**Ce que je voulais faire** : `ItemEntry { id: ItemName, … }` pour qu'un id refusé le soit à
la compilation.

**Ce qui existe** : `crate::inventory::ItemName` (`src/inventory/stack.rs:34`) valide le
format mais possède une `String` et se construit par `ItemName::new`, donc pas dans une
`const`.

**Ce que j'ai fait** : `id: &'static str`, identique à `crate::jobs::kinds::Accepted.item` et
à `crate::blocks::Block.name` — trois tables, une convention. La validation du charset est un
test, pas un type (`tests::no_item_has_two_rows_and_every_row_names_itself`).

## 5. Un accès aux *tags* d'items

**Ce que je voulais faire** : dire « les quatre `poisonousfood` » par le tag plutôt que par
quatre noms, et « les quatre mailles » par le tag `minecolonies:meshes`.

**Ce qui existe** : `Host::entity_type_in_tag` (`host/items.rs`) pour les **entités** ; aucun
appel d'hôte ne lit un tag d'**item**, et `crate::blocks` n'en porte pas non plus (ses tags de
blocs vivent en données, `data/minecraft/tags/block/`).

**Ce que j'ai fait** : des `&[&str; 4]` nommés et leur source
(`food::POISONOUS_FOOD`, `tools::MESH_DURABILITIES`). Le jour où `item_in_tag` arrive, la
liste de quatre reste la source et le tag la vérifie.

## 6. La recette comme objet

**Ce que je voulais faire** : `materials::Material { made_by_recipe: Recipe, … }` avec les
ingrédients.

**Ce qui existe** : `data/minecolonies/recipe/` (154 fichiers) et
`crate::jobs::kinds::Output::Recipe` — un `Product` qui dit « la recette décide » sans nommer
l'item. Aucun type `Recipe` dans le port, et `MANQUANTS.md` à la racine compte déjà les 143
items que ces recettes nomment sans déclaration.

**Ce que j'ai fait** : `Material::uses: &[Use]` avec l'**id de recette** et la **ligne du
datagen** qui la déclare, plus `job_consumers`/`job_producers` qui lisent
`crate::jobs::kinds` au lieu de le recopier.

## 7. Ce qui manque côté Java, et qui n'est donc pas porté

- **le contenu des 163 fichiers de déclaration** : il n'existe pas dans le dépôt Java, il est
  à écrire depuis ces tables (§1).
- **les modèles et textures** : `assets/` est une autre tranche, et aucune déclaration n'en
  nomme un.
- **les composants** (`minecraft:food`, `minecraft:max_damage`, les attributs de dégâts) :
  une déclaration les prend en `Vec<(String, Option<Tag>)>`, et un `Tag` n'a pas de
  constructeur accessible depuis une table `const`. Les nombres sont donc ici, les composants
  sont à générer depuis eux.
