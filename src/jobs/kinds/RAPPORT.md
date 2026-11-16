# `src/jobs/kinds` — les règles des 49 métiers

Ce que chaque métier **fait** : les items qu'il accepte, ce qu'il produit et en combien de
temps, l'endroit où il doit se tenir, ce qu'une action vaut en expérience, et le seuil au-delà
duquel il retourne vider son inventaire. Aucune IA, aucun monde, aucun tick : que des tables
`&'static` et deux formules.

Sources Java : `src/main/java/com/minecolonies/` du dépôt MineColonies. Chaque ligne du port
cite `fichier.java:ligne` en commentaire au-dessus d'elle, et chaque valeur de table porte sa
propre référence dans son champ `source`.

## 1. Ce qui est porté — le compte exact

| Fichier | Famille | Métiers | Items acceptés | Produits | Constantes nommées | Grants d'XP |
| --- | --- | --- | --- | --- | --- | ---: |
| `bois.rs` | ce qui pousse | 4 | 11 | 5 | 12 | 7 |
| `mine.rs` | ce qui se creuse et se cuit | 8 | 29 | 8 | 8 | 11 |
| `ferme.rs` | le champ, le troupeau, l'eau | 9 | 22 | 10 | 21 | 11 |
| `artisanat.rs` | les artisans | 12 | 27 | 16 | 4 | 17 |
| `service.rs` | courrier, médecine, école, chantier | 8 | 17 | 2 | 18 | 13 |
| `militaire.rs` | gardes et entraînement | 8 | 21 | 5 | 31 | 16 |
| **Total** | **6 familles** | **49** | **127** | **46** | **94** | **75** |

49 lignes de `JobRules`, une par métier **qui travaille dans une hutte**. 50 variantes de
`Job` existent dans `src/jobs/job.rs`, la cinquantième est `Placeholder` (voir §4).

**87 tests** dans le module (`mod.rs` 27, `militaire` 13, `service` 12, `ferme` 10,
`artisanat` 9, `mine` 9, `bois` 7). Le test central est
`tests::every_job_that_works_has_rules_and_only_the_placeholder_does_not` : il vérifie les deux
sens de la phrase « 49 métiers ont une entrée, `Placeholder` n'en a pas », et
`every_row_cites_a_java_line_somewhere` refuse une ligne sans référence.

## 2. Le modèle, et pourquoi il a cette forme

```rust
JobRules {
    job, family,
    places:        &[Place],      // un ou deux endroits, jamais zéro
    accepts:       &[Accepted],   // item ou tag + rôle + niveau d'outil
    produces:      &[Product],    // sortie + Work (durée) + ligne
    durations:     &[Constant],   // les nombres nommés, avec leur UNITÉ
    xp:            &[Xp],         // un ou deux grants, avec leur ligne
    dump:          Dump,          // actions avant de vider l'inventaire
    saturation, source,
}
```

Cinq décisions qui viennent des sources, pas du goût :

- **`places` est une liste** parce qu'un boulanger travaille à un établi **et** à ses fours
  (`AbstractEntityAIRequestSmelter.java:439`) : une place unique aurait dû en jeter une.
- **`Work` a cinq formes** parce que Java calcule une durée de cinq façons : un nombre de ticks,
  [`block_mining_ticks`], [`crafting_ticks`], la fonte vanilla, ou le compteur propre du métier
  (sifflet, concasseur, mineur du Nether).
- **`Constant` porte une unité** (`Ticks`, `Effort`, `ChanceDenominator`, `Count`, `Blocks`)
  parce que trois métiers déclarent des nombres qui **ne sont pas des durées** et qu'un lecteur
  prendrait pour telles : les 100/400/400 du croque-mort sont des **points d'effort**
  (`UndertakerConstants.java:61-71`), le `DELAY_TO_HARVEST_NETHERWART = 30` de l'alchimiste est
  un **1 sur 30** (`EntityAIWorkAlchemist.java:71`, lu en `nextInt(30) <= 1`), et le
  `ONE_IN_X_CHANCE = 8` de l'étudiant un **1 sur 8** (`EntityAIStudy.java:49`).
- **`xp` est une liste** parce que quatre métiers paient deux fois : cinq XP par extraction de
  four **et** la moitié du compte de la requête (`AbstractEntityAIRequestSmelter.java:622`,
  `AbstractEntityAICrafting.java:643`).
- **`Output` a quatre formes** : un stack nommé, les drops d'un bloc, la recette des données, ou
  une **entité** — le palefrenier produit une monture (`EntityAIWorkStableMaster.java:281`) et
  le croque-mort un citoyen ressuscité (`EntityAIWorkUndertaker.java:357`), et ni l'un ni
  l'autre n'est un item.

Les refus sont nommés (`RuleRefusal`), jamais des `unwrap` : `Unemployed`, `NoSaturation`,
`NoToolSpeed`, `BlockBreakResearchAboveOne`, `NegativeXp`. Tous sont atteignables depuis une
valeur fournie par l'appelant (un niveau, un effet de recherche, une saturation).

## 3. Les deux formules, et les trois compteurs propres

**Minage** — `AbstractEntityAIInteract.java:319-343`, la même pour le mineur, le carrier, le
bûcheron, le fermier, le mixeur à béton et le mineur du Nether (le **fleuriste** est la seule
exception de la famille `bois` : il remplace 500 par 10, voir sa ligne) :

```text
ticks = 500 × 0,85^(niveau de la compétence / 2) × dureté / vitesse de l'outil × (1 − recherche)
```

Quatre détails fidèles : le terme de niveau est `level / 2.0` (donc **chaque** niveau compte),
le cast est une troncature (`as i32` comme `(int)`), `pvp_mode` court-circuite à **250 ticks**
fixes, et une main vide renvoie la dureté brute. Une dureté négative (bloc incassable) traverse
telle quelle, comme en Java.

**Artisanat** — `AbstractEntityAICrafting.java:734-741`, lue à `:561` :

```text
coups = 10 / min(niveau / 2 + 1, 50) × 3      (division ENTIÈRE, les trois termes)
durée = max(1, coups) × HIT_DELAY (10)
```

La division entière est le fait du dossier : `10 / 11 == 0`, donc **à partir du niveau 20 un
artisan termine sa recette au premier coup**. La courbe est un escalier
300 → 150 → 90 → 60 → 30 → 10 ticks, pas une pente. Un port qui écrirait `30 / terme`
inventerait une huitième courbe que Java n'a pas.

**Trois compteurs propres**, qui ne sont ni un bloc ni une recette :

| Métier | Formule | Durée |
| --- | --- | --- |
| Tamiseur | `progress++` tous les 10 ticks jusqu'à `> 50 − compétence/2` | 510 ticks au niveau 0, 10 au niveau 99 |
| Concasseur | `progress += 40` tous les 40 ticks contre `50 − min(compétence/2+1, 50)` | 2 passes (80 ticks) sous le niveau 20, **1 passe (40 ticks) au-dessus** |
| Mineur du Nether | `TICK_DELAY = 40` par bloc, `40 × compte` par item non-bloc | 40 ticks et plus |

L'incohérence entre les deux premiers est réelle et conservée : le tamiseur gagne **1** par état,
le concasseur **40**, pour un seuil comparable.

**XP** — `CitizenExperienceHandler.java:73-128`, dans `xp_grant` : le bâtiment et la maison
multiplient par `1 + (niveauTravail + niveauMaison) / 10`, l'Intelligence ajoute son pourcentage,
la recherche `LEVELING` multiplie après, une saturation nulle annule tout, puis la compétence
primaire touche `local`, sa complémentaire un dixième, son adverse **en perd** un dixième, la
secondaire la moitié et ses deux voisines un vingtième. La courbe de niveau, elle, est déjà dans
`src/jobs/experience.rs` et n'est pas recopiée ici.

## 4. Ce qui est écarté, et pourquoi

- **`Placeholder`** — aucune règle. Java ne lui donne pas d'IA (`JobPlaceholder.java:31-34`
  renvoie `null`), pas de clé de traduction, pas de hutte. `rules_for` répond `None` et un test
  le tient.
- **Les constantes mortes**, listées pour qu'on ne les cherche pas : `MIN_DISTANCE_TO_WAREHOUSE`
  (`EntityAIWorkDeliveryman.java:62`), `XP_PER_WANDER` (`UndertakerConstants.java:21`),
  `BASE_XP_GAIN = 0.5` du fleuriste (`EntityAIWorkFlorist.java:65`),
  `BASE_XP_GAIN = 2` d'`AbstractEntityAIUsesFurnace` (jamais lu par le fondeur, qui a le sien à
  5), `GUARD_POS_RANGE` (`GuardConstants.java:18`), `CAVALRY_PATROL_RANGE` (`:154`),
  `PREPARE_DELAY_SECONDS`, `PHYSICAL_ATTACK_DELAY_MIN/BASE` (`:45-55`), et
  `BuildingCrusher.getMaxDailyQuantity` (`:88-95`), que l'IA n'appelle jamais.
- **Les seuils en pourcentage** ne sont pas des durées et vivent en commentaire :
  `MIN_PERCENTAGE_TO_CRAFT = 0.75` du bûcheron (`BuildingSawmill.java:39`),
  `PERCENT_CHANGE_FOR_GROWTH = 0.2` du fleuriste (`EntityAIWorkFlorist.java:60`),
  `TRAINING_CHANCE = 0.25` du palefrenier (`EntityAIWorkStableMaster.java:66`),
  la table de multiplicateurs du composteur (`EntityAIWorkComposter.java:359-378`).
- **Les tables de butin** : le mineur, le bûcheron et les six éleveurs produisent « les drops du
  bloc ou du mob ». `Output::Drops` le dit et le port ne recopie pas les tables, qui vivent dans
  `data/`.
- **Les recettes** : `Output::Recipe` renvoie aux données (`data/minecolonies/recipe/`). Le tag
  qui décide quelles recettes une hutte accepte est un nom, pas un contenu :
  `artisanat::recipe_tag` le rend (`minecolonies:sawmill`, …) et les tags eux-mêmes sont dans
  `data/minecolonies/tags/item/`.
- **La géométrie de la mine et de la carrière** est du contenu : les couloirs sont des blueprints
  (`minerx2top`, `minerx4`, …) et Java ne porte que `SHAFT_RADIUS = 3`, `NODE_DISTANCE = 7` et la
  hauteur du `QuarryModule` (32 ou 64), cités en commentaire plutôt qu'inventés en blocs.
- **Le modèle de rendu** (`IJob.getModel`), les particules, les sons, les statistiques de hutte :
  client, ou hors du périmètre d'une règle.
- **L'IA** : aucun état, aucune cible, aucun chemin. Le port n'a pas encore de tick par citoyen,
  donc un métier se décrit mais ne s'exécute pas. C'est la limite que `src/jobs/mod.rs` nomme
  déjà, et ce module ne la change pas.

## 5. Les références Java utilisées

**Socle partagé** (dans `mod.rs`, nommé une seule fois, avec sa ligne dans `SHARED`) :
`Constants.java`, `CitizenConstants.java`, `AbstractEntityAIBasic.java`,
`AbstractEntityAIInteract.java`, `AbstractEntityAICrafting.java`, `AbstractEntityCitizen.java`,
`EntityNavigationUtils.java`, `AbstractEntityAIStructure.java`, `AbstractEntityAIUsesFurnace.java`,
`AbstractEntityAIRequestSmelter.java`, `CitizenExperienceHandler.java`, `IJob.java`,
`AbstractJob.java`.

**Par famille** — les classes de métier de `core/colony/jobs/`, les classes d'IA de
`core/entity/ai/workers/` (et `guard/`, `production/agriculture/`, `production/herders/`,
`education/`, `service/`, `builder/`, `crafting/`), les huttes de
`core/colony/buildings/workerbuildings/`, plus `GuardConstants.java`,
`UndertakerConstants.java`, `TagConstants.java`, `ModTags.java`, `EquipmentLevelConstants.java`,
`NewBobberEntity.java`, `TileEntityBarrel.java`, `AbstractTileEntityBarrel.java`,
`DefaultCrusherCraftingProvider.java`, `DefaultSifterCraftingProvider.java`,
`DefaultConcreteMixerCraftingProvider.java`, `DefaultItemTagsProvider.java`, `BuildingModules.java`,
`ModItemsInitializer.java`, `CompatibilityManager.java`, `WorldUtil.java`.

Chaque ligne du code porte la sienne ; `grep -c "\.java:" src/jobs/kinds/*.rs` en donne le
décompte.

## 6. Manques rencontrés

Voir `MANQUANT.md` dans ce dossier. Aucun manque **du SDK** : ce module n'appelle aucune API de
l'hôte, il n'utilise que `std` et `crate::jobs`. Les manques sont des types que **le port** n'a
pas encore (un identifiant d'item const, un point de recherche, une entité), et chacun est
contourné par une valeur nommée plutôt que par une invention silencieuse.

## 7. Vérification

- `cargo check --tests` sur la crate entière : **aucune erreur attribuée à `src/jobs/kinds/`**.
  Les erreurs restantes (`src/ai/`, `src/network/`) sont des fichiers en cours d'écriture par
  d'autres agents et ne touchent pas ce module.
- Les tests ont été exécutés sur une copie réduite de la crate (modules `base`, `building`,
  `inventory`, `jobs`) dans `/tmp/kindcheck` de l'hôte, la crate réelle ne liant pas encore à
  cause des deux modules ci-dessus : **247 tests passent, 0 échec**, dont **87** dans
  `jobs::kinds`.
  ```sh
  ssh deb 'cd /tmp/kindcheck && PATH=$HOME/.cargo/bin:/usr/bin:/bin cargo test --lib jobs::kinds'
  # test result: ok. 87 passed; 0 failed
  ```
- Trois erreurs trouvées par les tests et corrigées, pas par relecture : la variante s'appelle
  `Job::StoneMason` et non `Stonemason` ; `ToolKind::name` n'est pas `const`, donc une table
  `tool_name` locale le double et un test compare les quatorze entrées ; le druide n'avait pas
  son grant de réveil.

## 8. Ce que le port ne dit pas encore, et qui n'est pas de son ressort

- **Combien d'items une requête demande** : le porteur demande « toutes les requêtes de même
  source et même cible » jusqu'à `1 + Adaptabilité/5`, le fermier 64 composts, le soigneur 16
  remèdes, le maître d'école 8 items d'élevage de plus que la paire. Ces nombres sont dans les
  commentaires ; le système de requêtes (`crate::requests`) ne les lit pas encore.
- **Les compétences** de chaque métier vivent dans `crate::jobs::employment`, pas ici : une ligne
  de `JobRules` ne les répète pas, `xp_grant` va les chercher par `Job::employments()`.
- **Le niveau d'outil maximal** appartient à la hutte (`IBuilding.getMaxEquipmentLevel`), déjà
  porté dans `src/jobs/tool.rs` : `Accepted.tool_level` ne porte que le **minimum**.
