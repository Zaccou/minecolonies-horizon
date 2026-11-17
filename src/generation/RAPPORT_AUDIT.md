# Audit de fidélité — `src/generation/`

Relecture de 40 méthodes de la source Java contre le Rust de cette tranche, chacune ouverte à sa
ligne. Verdict : **fidèle**, **divergent** ou **absent**. Trois divergences réelles trouvées,
**corrigées** dans mes fichiers ; une quatrième trouvée, **non corrigeable ici** et chiffrée
ci-dessous.

Aucune compilation (l'orchestrateur compile), aucun `git`, rien touché hors de `src/generation/`.
Chaque correction porte sa source Java en commentaire au-dessus du code.

## 1. Le tableau

`B` = `defaults/DefaultBlockLootTableProvider.java`, `E` = `defaults/DefaultEntityLootProvider.java`,
`S` = `defaults/DefaultSupplyLootProvider.java`, `O` = `defaults/DefaultLuckyOreLootProvider.java`,
`F` = `defaults/workers/DefaultFishermanLootProvider.java`,
`RL` = `defaults/workers/DefaultRecipeLootProvider.java`,
`SF` = `defaults/workers/DefaultSifterCraftingProvider.java`,
`NW` = `defaults/workers/DefaultNetherWorkerLootProvider.java`,
`LA` = `core/colony/crafting/LootTableAnalyzer.java`,
`IN` = `ItemNbtCalculator.java`, `CK` = `api/items/CheckedNbtKey.java`,
`RP` = `defaults/DefaultRecipeProvider.java`, `CR` = `CustomRecipeProvider.java`,
`SL` = `SimpleLootTableProvider.java`, `BT` = `DefaultBiomeTagsProvider.java`,
`ET` = `DefaultEntityTypeTagsProvider.java`, `DT` = `DefaultDamageTagsProvider.java`.

| # | Méthode Java | Règle portée | Rust | Verdict |
|---|---|---|---|---|
| 1 | `B:50-120` `registerTables` | 83 tables de bloc, 32 bannières vanilla, 2 huttes sans table | `loot.rs:423-485` | fidèle |
| 2 | `B:67-72` règle `composted_dirt` | soie → le bloc, sinon terre si l'explosion l'épargne | `loot.rs:437-444` + `silk_touch` `:276-292` | fidèle |
| 3 | `B:74-75` `farmland`/`floodedfarmland` | terre, **sans** condition | `loot.rs:446-451` | fidèle |
| 4 | `B:78-82` les 15 cultures | âge 6 → bonus Fortune binomial (0,5714286 ; 3), sinon l'item | `loot.rs:453-460`, `age_six:293`, `fortune_binomial:302` | fidèle |
| 5 | `B:130-139` `saveBlock(block)` | `copy_name` si `AbstractBlockHut` **ou** `BlockMinecoloniesRack` | `loot.rs:487-496` (`copies_name`) | fidèle |
| 6 | `B:141-153` `saveBlock(block,cfg)` | id `blocks/<path>`, contexte `BLOCK` | `loot.rs:368-381`, `constants.rs:87-94` | fidèle |
| 7 | `B:155-168` `saveBannerBlock` | `copy_name` + `copy_nbt` (`Patterns`, `id` → `BlockEntityTag`) ; l'item est `block.asItem()` | `loot.rs:1109-1160` | **divergent → corrigé** (§2.3) |
| 8 | `E:165-178` `registerLoot` | `rolls` = 1, `ALL_PARAMS` (donc pas de `type`) | `loot.rs:794-808` | fidèle |
| 9 | `E:44-162` les 18 butins | poids, qualités, ordre des entrées | `loot.rs:673-790` (`MOBS`) | fidèle (18/18) |
| 10 | `S:40-68` `registerTables` | 2 caisses, hasard 1 %/10 %, 4 parchemins, NBT `{Placement:"instant"}` | `loot.rs:498-527` | fidèle |
| 11 | `O:33-67` `registerTables` | 5 niveaux, le 5 **est** le 4 | `loot.rs:535-562` | fidèle |
| 12 | `F:73-100` `registerStandardLoot` | racine 10/-2, 5/+2 (océan ou recherche), 85/-1 ; 3 canaux | `loot.rs:565-624` | fidèle |
| 13 | `F:102-135` `registerBonusLoot` | niveaux 1-2 `EMPTY`, 3 = éclats/cristaux, 4-5 = + éponge | `loot.rs:626-664` | fidèle |
| 14 | `F:137-149` `makeLoot` | poids vide = 1000 − somme des poids | `loot.rs:600-618` | fidèle |
| 15 | `F:151-156` `getWeightForEntry` | poids par défaut 1 dans la soustraction | `loot.rs:601` | fidèle |
| 16 | `RL:40-56` `registerTables` | 3 tables, qualités ±1 sur les fioles, pas sur le gravier | `loot.rs:815-840` | fidèle |
| 17 | `SF:329-338` `registerTables` | id `recipes/<maille>/<bloc>`, `ALL_PARAMS` | `loot.rs:985-1003` | fidèle (16/16) |
| 18 | `SF:257` nom de maille | `sifter_mesh_X` → `X` | `workers.rs:236-247` | fidèle |
| 19 | `SF:277-311` `registerRecipes` | entrées bloc + maille, sorties secondaires, table, niveau min | `workers.rs:211-227` | **divergent → corrigé** (§2.1) |
| 20 | `SF:313-327` `mesh` | 4 mailles : métier, recherche, `show-tooltip` | `workers.rs:258-281` | fidèle (4/4 au datagen) |
| 21 | `NW:71-174` `createBlocksPool` | `rolls` 3-10, bonus 0,3/0,3, 22 blocs et leurs bornes | `loot.rs:1054-1105`, `:1024-1047` | fidèle (5/5) |
| 22 | `NW:176-212` `createMobsPool` | 6 jetons, poids 5500/300/500/300/300/100, qualités | `loot.rs:1004-1012` (`TOKENS`) | fidèle (5/5) |
| 23 | `NW:214-227` `createAdventureToken` | NBT `entity-type`, `damage-done`, `xp-dropped` | `loot.rs:1080-1086` | fidèle (5/5) |
| 24 | `NW:229-270` `registerRecipes` | entrées 64/32/16, min = max = niveau, `additional-output` | `workers.rs:283-310` | **divergent, non corrigé** (§3) |
| 25 | `LA:96-134` `toDrops` | l'entrée vide ne produit rien ; tri par probabilité | `workers.rs:214-222`, `:291-297` | partiel (§3) |
| 26 | `LA:169-173` jeton d'aventure | un jeton s'affiche comme la **table du mob** qu'il nomme | `workers.rs:299` | **absent** (§3) |
| 27 | `LA:261-278` `expandAdventureToken` | `mob.getDefaultLootTable()` puis récursion | — | **absent** (§3) |
| 28 | `IN:55-146` `run` | lignes = tous les items de tous les onglets créatifs | `nbt.rs:253-260` (refus nommé) | absent, refusé |
| 29 | `IN:89-126` boucle de clés | `ignoreNBT` vide le tag ; `display`/`Enchantments`/`RepairCost` ; `Damage` retiré | `nbt.rs:163-195` | fidèle |
| 30 | `IN:116-125` fusion | un 2ᵉ stack du même item **rejoint** sa ligne | `nbt.rs:225-250` | **divergent → corrigé** (§2.2) |
| 31 | `CK:27-46` `equals` | l'égalité est `(clé, enfants)` | `nbt.rs:188-193` | **divergent → corrigé** (§2.2) |
| 32 | `IN:215-232` `createKeyFromNbt` | les enfants sont un ensemble, à chaque niveau | `nbt.rs:197-204` | fidèle (tri au lieu de hachage, documenté) |
| 33 | `IN:153-165` `serializeKeyToJson` | `children` seulement s'il y en a | `nbt.rs:76-83` | fidèle |
| 34 | `IN:128-145` écriture | chemin `compatibility/itemnbtmatching`, `checkednbtkeys` si non vide | `nbt.rs:225-250` | fidèle |
| 35 | `RP:283-299` `registerHutRecipe1` | 9 planches, outil en haut, 1 résultat | `recipes.rs:216-234` | fidèle (47/47) |
| 36 | `RP:306-322` variante tag | idem, `B` = un tag | `recipes.rs:216-234` (`Input::Tag`) | fidèle |
| 37 | `RP:329-343` `registerHutRecipe1x2` | résultat = la **hutte**, suffixe sur le **fichier** | `recipes.rs:200-234` | corrigé (mission précédente) |
| 38 | `RP:350-365` `registerHutRecipe3` | ligne du milieu entière, 1 résultat | `recipes.rs:216-234` | fidèle |
| 39 | `RP:453-541` `buildFood` | 6 pâtes × 3 cuissons, 300/100/600, ÷8 pour le cookie, 0,0475 | `recipes.rs:348-361` | fidèle (17/18 ; `baked_cake` = écart du pack) |
| 40 | `RP:264-273` `append` | 3 formes, la 2ᵉ vide ⇒ pas de suffixe | `constants.rs:116-122` | fidèle |
| 41 | `BT:30-109` `addTags` | 4 groupes, `deep_frozen_ocean` **deux fois**, 3 tags retirés du tempéré | `tags.rs:76-160` | fidèle (4/4) |
| 42 | `ET:28-36` `addTags` | 5 tags, `corpse:corpse` optionnel, 18 raiders dans l'ordre | `tags.rs:168-229` | fidèle (5/5) |
| 43 | `DT:28-31` `addTags` | 3 drapeaux vanille, 4 ids de dégâts | `tags.rs:236-263` | fidèle (3/3) |
| 44 | `SL:84-120` `itemStack` | `set_nbt` + `set_count` si le compte dépasse 1 | `loot.rs:1080-1086` | fidèle |
| 45 | `CR:271-295` `storageAsJson` | `count` omis à 1, `matchtype` seulement si `ignoreNBT()` | `workers.rs:107-118` | fidèle |
| 46 | `CR:147-170` `minResearchId` | 1 appel = chaîne, 2 appels = tableau | `workers.rs:145-160` | fidèle |
| 47 | `CR:231-235` `build` | `intermediate` **toujours** écrit | `workers.rs:163-199` | fidèle |
| 48 | `api/loot/SupplyLoot.java:59-77` `doApply` | injecte la caisse dans les coffres vanilla (modificateur global) | — | hors tranche (§4) |

Les lignes 1 à 48 valent 38 méthodes distinctes : les règles comptées une fois sont les 5
`registerTables`, les 4 règles de bloc, les 3 `registerRecipes`, les 2 boucles de clés, les 4
patrons de recette, les 3 `addTags`, les 2 helpers de `CustomRecipeProvider` et `expandAdventureToken`.

## 2. Divergences corrigées

### 2.1 L'entrée maille du tamis ne porte pas `matchtype` (`workers.rs:211-227`)

**Ce que j'avais écrit** : `matchtype: "ignore"` sur l'entrée maille, avec un commentaire affirmant
que Java l'écrit et que le pack est plus vieux.

**Le Java** : `new ItemStorage(new ItemStack(mesh.getMesh()), true, false)`
(`SF:291`) appelle le constructeur **à trois arguments**
`ItemStorage(stack, ignoreDamageValue, shouldIgnoreNBTValue)`
(`api/crafting/ItemStorage.java:90-96`) — donc `ignoreNBT = false`. Or `storageAsJson` n'écrit
`matchtype` que `if (itemStorage.ignoreNBT())` (`CR:288-291`). Le champ n'est donc **jamais** écrit
pour la maille.

**Pourquoi je m'étais trompé** : j'ai lu la signature à quatre arguments (`(stack, amount,
ignoreDamage, ignoreNbt)`) et pris `true, false` pour `(ignoreNbt, ...)`. Le pack, lui, avait raison
depuis le début : les 16 fichiers `sifter/*` n'ont pas le champ.

**Correction** : `.input(mesh_item(mesh), 1, false)` + le commentaire qui cite
`ItemStorage.java:90-96` ; le test `the_sifter_mesh_input_is_the_one_that_ignores_its_nbt` devient
`…_ignores_damage_and_not_its_nbt` et asserte l'**absence** du champ ; le module doc et
`MANQUANT.md` §2 sont réécrits (l'écart n'existe pas).

**Preuve de la correction** : les 16 recettes de tamis et les 4 de maille rejouées depuis le Rust
sont maintenant **identiques** aux 20 fichiers du datagen 1.20.1 (`crafterrecipes/sifter/*`,
`crafterrecipes/{fletcher,stonemason,blacksmith,mechanic}/sifter_mesh_*.json`).

### 2.2 `nbt::matching` doit fusionner deux stacks d'un même item (`nbt.rs:225-250`)

**Ce que j'avais écrit** : une ligne par `Facts`, triées par item. Deux `Facts` du même item
produisaient **deux** lignes dans le tableau JSON.

**Le Java** : `keyMapping` est un `TreeMap<String, Set<CheckedNbtKey>>` (`IN:88`), et un second stack
d'un item déjà vu **ajoute ses clés à la ligne existante** (`IN:116-125`). Le `Set` est un ensemble de
`CheckedNbtKey`, dont l'égalité est `(key, children)` (`CK:27-46`) : deux clés de même nom mais
d'enfants différents sont **deux** entrées, deux clés identiques une seule.

**Correction** : `matching` groupe par identifiant d'item (`BTreeMap`), fait l'union avec
déduplication exacte, trie ; `checked_keys` déduplique désormais sur la valeur complète
(`keys.dedup()`) et non plus sur le seul nom. Test ajouté :
`a_second_stack_of_one_item_joins_its_row` (un item, une ligne, deux clés).

### 2.3 Une bannière murale lâche la bannière **debout** (`loot.rs:1109-1160`)

**Ce que j'avais écrit** : l'item de la table d'une bannière murale était
`minecraft:<couleur>_wall_banner`.

**Le Java** : `saveBlock`/`saveBannerBlock` font `LootItem.lootTableItem(block)`
(`B:162`), c'est-à-dire `block.asItem()`. Une bannière murale n'a pas d'item à elle : `asItem()`
rend la bannière **debout**. Le datagen livré le dit noir sur blanc —
`minecraft/loot_tables/blocks/black_wall_banner.json` porte
`"name": "minecraft:black_banner"` alors que son `random_sequence` est bien
`minecraft:blocks/black_wall_banner` —, et les 16 fichiers muraux sont dans ce cas.

**Pourquoi je ne l'avais pas vu** : les 32 tables de bannières ne sont **pas** dans `data/`
(le fence interdit d'écrire des tables vanilla), donc aucun test ni aucune comparaison au pack ne
pouvait les couvrir ; mes rejeux précédents s'arrêtaient aux 146 tables du namespace `minecolonies`.

**Correction** : l'item vaut `minecraft:{couleur}_banner` pour les deux formes, l'id de table garde
le suffixe `_wall_banner` ; l'assertion ajoutée dans
`the_32_banner_tables_are_written_into_the_vanilla_namespace` épingle la règle.

**Preuve de la correction** : les 32 tables rejouées depuis le Rust sont maintenant **identiques**
aux 32 fichiers du datagen (`src/datagen/generated/…/data/minecraft/loot_tables/blocks/`).

## 3. Divergence trouvée, non corrigeable ici : le `additional-output` des voyages du nether

**Le Java** : `NW:241-247` construit la liste affichée avec
`LootTableAnalyzer.toDrops(lootTableManager, table)` ; l'analyseur **développe chaque jeton
d'aventure en la table de butin par défaut du mob qu'il nomme**
(`LA:169-173` puis `expandAdventureToken` `LA:261-278`), et trie le résultat par probabilité
décroissante (`LA:133`).

**Le Rust** (`workers.rs:283-310`) : pousse `minecolonies:adventure_token`, dans l'ordre de la table.

**Mesure** (5 fichiers, comparés au datagen) :

| fichier | entrées écrites | entrées du datagen |
|---|---:|---:|
| `netherworker/trip1.json` | 6 | 13 |
| `trip2` | 13 | 20 |
| `trip3` | 20 | 27 |
| `trip4` | 22 | 29 |
| `trip5` | 23 | 30 |

`netherworker/lava.json` est identique. L'écart est exactement « mes blocs + les butins des 6 mobs »
(7 butins de plus au niveau 1), ce qui confirme le mécanisme.

**Pourquoi je ne le corrige pas** : l'expansion demande les **tables de butin vanilla** des six mobs
(zombified piglin, magma cube, hoglin, ghast, enderman, blaze). Elles appartiennent au jeu, pas à ce
crate, et les inventer serait exactement ce que la règle « aucune API inventée » interdit. C'est
chiffré dans `src/generation/MANQUANT.md` §2, avec la règle exacte à implémenter le jour où ces
tables sont lisibles.

**Ce que j'ai corrigé autour** : le module doc et `MANQUANT.md` §2 affirmaient que le pack était
« plus vieux que le code ». C'était **faux** — c'est le pack qui a raison. Les deux textes disent
maintenant le mécanisme réel et désignent l'analyseur.

## 4. Ce que je n'ai pas pu vérifier

1. **Les 161 `crafterrecipes/` ne sont pas dans `data/`** : mes 26 fichiers sont donc comparés au
   datagen 1.20.1 (20 identiques, 6 dont 5 divergents par le §3). Aucun test du crate ne peut le
   faire aujourd'hui : il n'y a pas d'oracle livré pour cette famille.
2. **`ItemNbtCalculator.run`** (`IN:55-146`) : les lignes viennent des onglets créatifs
   (`CraftingUtils.forEachCreativeTabItems`), que le port n'a pas. La règle est portée et testée, la
   liste de lignes est refusée par son nom (`nbt::refusal`). `textureData` (`IN:66-78`) dépend d'un
   autre mod (Domum) : le port le prend en donnée d'entrée, rien ne l'exerce.
3. **Les probabilités de `LootTableAnalyzer`** (`LA:96-134`) : le port reproduit la *liste* des
   entrées, pas les probabilités ni leur tri. Sans effet sur les 16 tamis (vérifié : l'ordre des
   poids y est déjà décroissant, donc le tri stable ne change rien), mais c'est la cause du §3.
4. **`conditionsSeemImpossible`** (`LA:215-...`) : filtre les entrées dont les conditions ne peuvent
   jamais être remplies par un colon. Aucune de mes 26 tables n'a de condition d'entrée, donc la
   règle ne s'exerce pas ; elle n'est pas portée.
5. **`SupplyLoot.doApply`** (hors tranche) : c'est le modificateur de butin global qui injecte les
   deux caisses dans les coffres vanilla. Le moteur n'a pas cette passe
   (`crates/horizon-world/src/loot/query.rs:39` marque encore l'emplacement) ; la tranche génère les
   deux tables qu'il référence (`chests/supplycamp|supplyship`), rien de plus.
6. **Le rendu visuel / en jeu** : rien ici ne prouve qu'une table se comporte comme dans Java à
   l'exécution ; seuls le contenu et les règles sont vérifiés.

## 5. État après audit

| | |
|---|---|
| entrées auditées | 48 (≈ 40 méthodes distinctes) |
| fidèles | 37 entrées |
| divergentes corrigées | 4 entrées / 3 défauts (2.1 : appel + test + 2 docs ; 2.2 : 2 fonctions + 1 test ; 2.3 : 1 fonction + 1 test) |
| divergentes non corrigées | 1 famille de 5 fichiers (§3), chiffrée et référée dans `MANQUANT.md` §2 |
| absentes (refusées nommément) | 3 méthodes (`IN:55-146`, `LA:169-173`, `LA:261-278`) |
| hors tranche | 1 (`SupplyLoot.doApply`) |

Fichiers modifiés : `src/generation/loot.rs`, `src/generation/workers.rs`,
`src/generation/nbt.rs`, `src/generation/MANQUANT.md`, ce rapport. Aucun autre.

Rejeux de contrôle après correction (générateur transcrit depuis le Rust, comparé fichier par
fichier) : **32/32** bannières, **12/12** tags, **58/58** tables de butin (entités, tamis, mineur,
nether, recettes, pêcheur, caisses), **58/58** recettes (47 huttes + 11 spéciales) moins
`baked_cake` (écart du pack, documenté), **20/24** `crafterrecipes` (les 4 restants sont les 5
voyages du §3 et le `lava.json` identique).

---

## 6. Annexe — la tranche « recettes et NBT », audit ligne à ligne

Périmètre de cette annexe : **`recipes.rs`, `nbt.rs`** et les deux helpers de format dont ils
dépendent (`constants::append`/`cooking_variant`, `json`). C'est un second passage : les entrées 28 à
40 du tableau §1 couvrent déjà quelques-unes de ces méthodes en une ligne, celui-ci ouvre **chaque
grille et chaque règle** et ajoute les citations manquantes.

Méthode : les 47 appels `registerHutRecipe*` et les 11 grilles ont été **extraits du Java par script**
et comparés un à un aux tables Rust (patron, compte, clé par clé) ; les formules de cuisson et les
règles NBT ont été relues dans le Java et recomptées à la main. Aucune compilation.

Résultat : **48 entrées auditées** (plusieurs tiennent en une ligne, les onze grilles sont comptées
une fois), **0 divergence de code, 10 défauts de documentation corrigés** dans 3 fichiers `.rs` et un
`.md`. Les 3 divergences de code de la première passe (§2) restent valides : rien de cette annexe
ne les rouvre.

`RP` = `defaults/DefaultRecipeProvider.java`, `IN` = `generation/ItemNbtCalculator.java`,
`CK` = `api/items/CheckedNbtKey.java`, `CS` = `core/blocks/BlockColonySign.java`.

### 6.1 Le tableau

| # | Méthode Java | Règle portée | Rust | Verdict |
|---|---|---|---|---|
| 49 | `RP:69-115` les 47 `registerHutRecipe*` | nom de bloc, item, patron, suffixe | `recipes.rs:88-141` (`HUTS`, `VARIANTS`) | fidèle — **47/47** comparés par script |
| 50 | `RP:284-297` `registerHutRecipe1(ItemLike)` | `XTX/XBX/XXX`, 9 planches, outil en haut, 1 résultat | `recipes.rs:225` (`Shape::One`), `:223-242` | fidèle |
| 51 | `RP:305-324` `registerHutRecipe1(TagKey)` | idem, `B` = un tag (`TagKey` ⇒ `{"tag": …}`) | `recipes.rs:188-197` (`Input::Tag`) | fidèle |
| 52 | `RP:329-343` `registerHutRecipe1x2` | compte **2**, suffixe sur le **fichier** (`:342`), résultat = la hutte | `recipes.rs:229-241` | fidèle |
| 53 | `RP:351-365` `registerHutRecipe3` | `XTX/BBB/XXX`, 1 résultat | `recipes.rs:227` (`Shape::Three`) | fidèle |
| 54 | `RP:92` garde-tour, suffixe vide | un fichier, deux huttes, aucun suffixe | `recipes.rs:111` (`Shape::Double("")`), `:229`, `:237-239` | fidèle |
| 55 | `RP:264-270` `append(ResourceLocation,…)` | base seule si `text2` vide (`:268`) | `constants.rs:118-124` | fidèle (doc corrigée, §6.2.3) |
| 56 | `RP:272-276` `append(ItemLike,String)` | `text1` = `""` (`:275`) ⇒ `blockhutminerstone` | `recipes.rs:237-239` (`format!`) | fidèle |
| 57 | `RP:117-126` concasseur (`blockhutcrusher`) | `XTX/CBC/XXX` | `recipes.rs:262-267` | fidèle |
| 58 | `RP:128-136` cimetière | pierre + os | `recipes.rs:269-273` | fidèle |
| 59 | `RP:138-146` école | plume, `XBX` deux fois | `recipes.rs:275-279` | fidèle |
| 60 | `RP:148-156` université | livre, `XBX` deux fois | `recipes.rs:281-285` | fidèle |
| 61 | `RP:158-167` fonderie de pierre | `AFA`, fournaise | `recipes.rs:287-293` | fidèle |
| 62 | `RP:169-177` épouvantail | `Ingredient.of(foin, citrouille)` ⇒ **tableau**, pas d'outil dans la grille | `recipes.rs:295-299` | fidèle (fichier livré : `"H"` = 2 objets) |
| 63 | `RP:179-188` champ de plantation | `T/SIS/PPP` | `recipes.rs:301-307` | fidèle |
| 64 | `RP:190-199` carrière simple | pioche en fer | `recipes.rs:308-313` | fidèle |
| 65 | `RP:201-210` carrière moyenne | pioche en diamant, **même** patron | `recipes.rs:314-319` | fidèle |
| 66 | `RP:212-221` porte fortifiée (`blockhutgatehouse`) | arc **et** épée en fer | `recipes.rs:321-326` | fidèle |
| 67 | `RP:223-231` panneau de colonie | **6** par craft | `recipes.rs:328-332` | fidèle |
| 68 | `RP:243-249` marteau d'assistant | `GGG/GBG/" S "`, `forge:ingots/gold` | — | **absent, déclaré** (§6.2.1) |
| 69 | `RP:366-451` `buildOtherBlocks` | 9 grilles/listes | — | absent, hors périmètre déclaré |
| 70 | `RP:546-1108` `buildOtherItems` | 22 grilles + 44 listes | — | absent, hors périmètre déclaré |
| 71 | `RP:688-697` pépite du cimeterre | `smelting` **et** `blasting` | — | absent, hors périmètre déclaré |
| 72 | `RP:455-476` six `smelting` | 300 ticks, 0,35 (cookie : `300/8`, 0,0475) | `recipes.rs:344-359`, `:362-382` | fidèle |
| 73 | `RP:478-500` six `smoking` | 100 ticks | idem (`METHODS:357`) | fidèle |
| 74 | `RP:502-544` six `campfireCooking` | 600 ticks | idem (`METHODS:358`) | fidèle |
| 75 | `RP:467`, `:493`, `:529` `300/8`, `100/8`, `600/8` | division **entière** de `int` ⇒ 37, 12, 75 | `recipes.rs:372` (`u32 /`) | fidèle |
| 76 | `RP:466` `0.0475f` | expérience du cookie, seul écart des 18 | `recipes.rs:347` | fidèle |
| 77 | `RP:459` `RecipeCategory.FOOD` | `"category": "food"` sur les 3 gâteaux | `recipes.rs:371` | fidèle (le pack dit `blocks` sur `baked_cake`, écart documenté) |
| 78 | `RP:254-262` `hasAllOf` + `unlockedBy` | conditions de déblocage ⇒ **advancements** | — | absent, hors pack (aucun `advancements/` livré) |
| 79 | `RP:169-177` `Ingredient.toJson` | objet nu si 1 valeur, tableau sinon | `recipes.rs:188-197` | fidèle |
| 80 | `IN:57-84` `run` (les lignes) | tous les items de tous les onglets créatifs | `nbt.rs:259-267` (refus nommé) | absent, refusé |
| 81 | `IN:66-78` `textureData` | compound synthétique pour un bloc Domum, **clés seules** lues | `nbt.rs:36-44`, `Facts::compound:124-129` | partiel (doc corrigée, §6.2.4) |
| 82 | `IN:92` `hasTag && !ignoreNBT` | tag vidé ⇒ aucune clé propre | `nbt.rs:167-172` (`ignored`) | fidèle |
| 83 | `IN:93` `new HashSet<>(tag.getAllKeys())` | une clé par entrée de tag | `nbt.rs:119-126` (`Facts::keys`) | fidèle |
| 84 | `IN:95-98` `DyeableLeatherItem` | `display` | `nbt.rs:132`, `:175-177` | fidèle |
| 85 | `IN:99-102` `isEnchantable` | `Enchantments` | `nbt.rs:137`, `:178-180` | fidèle |
| 86 | `IN:103-106` `isRepairable` | `RepairCost` | `nbt.rs:142`, `:181-183` | fidèle |
| 87 | `IN:107-108` `keys.remove("Damage")` | l'usure ne compte jamais | `nbt.rs:188` | fidèle |
| 88 | `IN:110-113` `Set<CheckedNbtKey>` | une clé répétée se replie | `nbt.rs:196-197` (`sort`+`dedup`) | fidèle |
| 89 | `IN:116-125` fusion d'un 2ᵉ stack | rejoint la ligne de l'item | `nbt.rs:231-241` | fidèle (corrigé en 1ʳᵉ passe, §2.2) |
| 90 | `IN:88` `TreeMap<String,…>` | lignes triées par id d'item | `nbt.rs:232` (`BTreeMap`) | fidèle |
| 91 | `IN:133-140` écriture | `item`, puis `checkednbtkeys` **seulement** si non vide | `nbt.rs:211-220`, `:246-249` | fidèle |
| 92 | `IN:153-166` `serializeKeyToJson` | `children` seulement s'il y en a | `nbt.rs:79-85` | fidèle |
| 93 | `IN:215-232` `createKeyFromNbt` | récursion sur les compounds, un `Set` à chaque niveau | `nbt.rs:124-129`, `:203-208` | fidèle (tri au lieu de hachage, documenté) |
| 94 | `IN:128` chemin du fichier | `data/minecolonies/compatibility/itemnbtmatching.json` | `nbt.rs:251-254` | fidèle |
| 95 | `CK:28-46` `equals`/`hashCode` | l'égalité est `(key, children)` | `nbt.rs:60` (`#[derive(…, PartialEq, Eq, Ord)]`) | fidèle |
| 96 | `CK:54-80` `matches` (runtime) | comparaison à l'exécution | `src/data_listeners/nbt_keys.rs` | hors tranche — format vérifié cohérent |

### 6.2 Divergences trouvées et corrigées

Les dix corrections sont **de la documentation** : aucun désaccord de code n'a été trouvé dans cette
tranche — mais une ligne de doc fausse sur une règle est une divergence pour la relecture suivante,
et c'est exactement ce que la mission demande de trancher.

#### 6.2.1 Le module doc donnait trois bornes fausses, et « 74 » au lieu de 78

`recipes.rs:9-11` annonçait `:264-365` pour les trois patrons, `:117-186` pour les onze grilles et
`:274-364` pour la cuisson. Mesuré dans le Java :

- les trois patrons sont **`:284-365`** (`registerHutRecipe1` à `:284`, `3` finit à `:365`) ;
  `:264-276` est `append`, qui donne l'id, pas le patron ;
- les onze grilles vont de **`:117` à `:231`** (`:223-231` = le panneau de colonie) : `:186` s'arrête
  juste après l'épouvantail, et laisserait dehors le champ de plantation, les deux carrières, la
  porte et le panneau ;
- la cuisson est `buildFood`, **`:453-544`** ; `:274-364` ne contient aucune cuisson.

Et le compte : le pack livre **154** recettes, le module en écrit **76** (47 + 11 + 18), donc **78**
ne sont pas écrites — le doc disait 74. La composition, vérifiée type par type sur
`data/minecolonies/recipe/` (88 façonnées, 46 sans forme, 7 `smelting`, 6 `smoking`, 6
`campfire_cooking`, 1 `blasting`) :

| Endroit | Lignes | Fichiers |
|---|---|---|
| `buildOtherBlocks` | `:366-451` | 7 façonnées + 2 sans forme |
| `buildOtherItems` | `:546-1108` | 22 façonnées + 44 sans forme + `smelting`/`blasting` du cimeterre (`:688-697`) |
| grille restée dans `buildHutRecipes` | `:243-249` | le marteau d'assistant |

9 + 68 + 1 = **78**. Les deux bornes que le doc donnait à la place ne découpent pas ces méthodes :
`:186-263` est la fin de `buildHutRecipes` (les grilles 7 à 11 et le marteau) **plus** les deux
`hasAllOf`, et `:367-1103` commence dans `buildOtherBlocks`, englobe **tout** `buildFood` et finit
dans `buildOtherItems` — trois méthodes distinctes sous une seule borne, dont une (la cuisson) est
pourtant portée par ce module.

**Correction** : `recipes.rs:4-24` réécrit avec ces quatre lignes ; `MANQUANT.md` §5 reçoit le même
tableau et nomme le marteau.

#### 6.2.2 Le panneau de colonie *est* un id de bloc

`recipes.rs:241-242` disait que le panneau est « the only one whose id is not a block id ». C'est
faux, et la source le dit en une ligne : `CS:56` — `private static final String BLOCK_NAME =
"colonysign"` — et `CS:80` rend `new ResourceLocation(MOD_ID, BLOCK_NAME)`. `minecolonies:colonysign`
est donc un bloc comme `simplequarry` et `mediumquarry` (le port le déclare :
`src/blocks/decoration.rs:227`).

**Correction** : la phrase dit maintenant que son id est le nom de bloc du panneau (`CS:56`, rendu
par `CS:80`) et que, des onze grilles, **trois** ne s'écrivent pas `blockhut…` : `simplequarry`,
`mediumquarry` et `colonysign`.

Au passage, `recipes.rs:81-83` disait « the four `1x2` variants » alors que `registerHutRecipe1x2` est
appelée **cinq** fois : quatre avec un suffixe (`VARIANTS`), la garde-tour avec `""` (`RP:92`), qui
vit donc dans `HUTS`. Le commentaire de `HUTS` dit maintenant 42 + la garde-tour.

#### 6.2.3 `append` : qui passe `"_"`, qui passe `""`

`constants.rs:112-115` affirmait qu'une variante « is always called with `("_", suffix)` ». Faux, et
c'est le cœur du bug corrigé à la passe précédente : les quatre variantes de hutte passent par
l'overload `append(ItemLike, String)` (`RP:272-276`), qui transmet **`""`** comme premier texte
(`:275`) — d'où `blockhutminerstone` et non `blockhutminer_stone`. Le `("_", suffix)` est l'affaire
de la cuisson (`RP:455-544`) et de personne d'autre.

**Correction** : les deux appelants sont écrits séparément, avec `:264-270` + `:272-276` (et non
`:264-271`, qui déborde sur la seconde signature) et `:92` pour la garde-tour. La citation de
`cooking_variant` passe de `:279` (javadoc de `registerHutRecipe1`, sans rapport) à `:455-544`.

#### 6.2.4 Deux références mortes dans `nbt.rs`

- `nbt.rs:39` renvoyait à ``[`Facts::texture_data`]`` — **cette méthode n'existe pas** (`Facts` a
  `new`, `scalar`, `compound`, `dyeable`, `enchantable`, `repairable`, `ignored`). Le doc décrit
  maintenant le mécanisme réel : un appelant qui a la liste des composants ajoute la clé avec
  `Facts::compound`, et seules les **clés** de ce compound atteignent le fichier, parce que
  `createKeyFromNbt` (`IN:215-232`) ne lit jamais les valeurs. Le matériau par défaut de chaque
  composant n'a donc pas à être porté.
- `nbt.rs:102` annonçait « the four traits » ; le Java en ajoute **trois** (`display`,
  `Enchantments`, `RepairCost`, `IN:95-106`) avant de retirer `Damage`. Le commentaire du test disait
  déjà « three » : deux endroits du même fichier se contredisaient.

### 6.3 Ce que cette passe n'a pas pu vérifier

1. **La compilation et les tests** — consigne de la mission : rien n'est compilé ici. Les neuf
   corrections sont des commentaires et un Markdown, donc aucun changement de comportement possible,
   mais je n'ai pas de sortie verte à montrer pour cette passe.
2. **`itemnbtmatching.json` en sortie** : le fichier n'est pas livré par le pack (`DELIVERED` dit 0
   pour `compatibility/`), donc la comparaison octet-pour-octet est impossible. C'est la **règle**
   qui est vérifiée, pas une sortie.
3. **La liste des lignes** du même fichier : elle demande le registre des onglets créatifs, que ce
   port n'a pas (`nbt::refusal`, `IN:57-84`). Non vérifiable, refus nommé.
4. **`CheckedNbtKey.matches`** (comparaison à l'exécution) est dans `src/data_listeners/`, hors de
   ma tranche. J'ai seulement vérifié que le format écrit (`item`, `checkednbtkeys` optionnel) est
   celui que ce lecteur attend (`nbt_keys.rs:14-16`, `:41`).
5. **Les `unlockedBy`** produisent des advancements : ni le pack livré (`data/` n'a pas
   `advancements/`) ni ce module ne les portent. Je n'ai pas cherché si une autre tranche les couvre
   — je note l'absence, pas un oubli prouvé.
6. **L'ordre d'itération de Java** : `HashSet` (`IN:93`, `:110`, `:220`) n'a pas d'ordre
   reproductible. Le tri de ce port est une propriété **plus forte** (deux exécutions concordent),
   pas une preuve d'égalité avec une sortie Java.

### 6.4 État après cette passe

| | |
|---|---|
| entrées auditées (cette annexe) | 48 |
| fidèles | 40 |
| absentes, déclarées | 5 (marteau `RP:243-249`, `buildOtherBlocks`, `buildOtherItems`, pépites `RP:688-697`, `unlockedBy`) |
| partielle, refus nommé | 1 (`IN:66-78` `textureData`) |
| absente, refus nommé | 1 (`IN:57-84`, la liste des lignes) |
| hors tranche | 1 (`CK:54-80`) |
| divergences de code trouvées | 0 |
| défauts de documentation corrigés | 10 : 3 bornes de lignes et un compte dans le seul module doc (§6.2.1), le compte de `1x2` (§6.2.2), `append` et `cooking_variant` (§6.2.3), `Facts::texture_data` et « four traits » (§6.2.4) |
| fichiers touchés | `recipes.rs`, `constants.rs`, `nbt.rs`, `MANQUANT.md`, ce rapport |

Rejeux de contrôle de cette passe, faits depuis les sources et non depuis les tests : **47/47**
recettes de hutte identiques au pack (patron, compte, clé par clé, résultat), **11/11** grilles
spéciales identiques, **18/18** cuissons recomposées depuis `DOUGHS`/`METHODS` (temps, expérience,
catégorie, type), et les trois tables de cookies relues dans le pack (37 / 12 / 75, 0,0475).
