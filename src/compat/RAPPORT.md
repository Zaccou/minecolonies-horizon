# `src/compat/` — noms de blocs 1.20.1 → 26.3, mesure et table

Rapport de la tranche « compatibilité des palettes ». Ce dossier ne contient que trois
fichiers Rust (`mod.rs`, `table.rs`, `states.rs`) et ce rapport ; **aucun autre module, aucun
`data/`, aucun blueprint n'a été touché**.

---

## 1. Mesure propre

### Méthode

La palette, pas le fichier. Un `.blueprint` est un NBT gzippé ; on lit la **liste `palette`**
du compound racine (celle que `cellule → index` référence), exactement ce que fait
`src/blueprint/format.rs` (`read_palette`, `Blueprint::palette`). Les `id` d'items qui traînent
dans les `tile_entities` et les `entities` ne sont jamais ramassés.

Deux mesures ont été faites, et elles se recoupent :

1. hors crate, un lecteur NBT minimal (Python) sur les 9 515 fichiers décompressés — pour les
   comptages et les empreintes ;
2. dans la crate, un test qui relit le pack avec `Blueprint::parse` (le vrai lecteur du port) sur
   un échantillon à pas constant, et un test `#[ignore]` qui le relit **entier** et retrouve les
   chiffres ci-dessous.

Le registre 26.3 est celui du client épinglé :
`["minecraft:block"]["entries"]` de `.local/horizon-client/26.3/registries.json`.

### Résultats

| Mesure | Valeur |
| --- | --- |
| fichiers `.blueprint` | 9 515 |
| entrées de palette (toutes listes confondues) | 612 400 |
| **blocs distincts** | **1 017** |
| … namespace `minecraft` | 860 |
| … namespaces de mods | 157 (`minecolonies` 69, `domum_ornamentum` 83, `structurize` 4, `multipiston` 1) |
| blocs connus du registre 26.3 | 1 286 |
| **blocs `minecraft` absents du registre 26.3** | **2** |

Les deux absents, avec leur poids réel (entrées de palette ≠ cellules) :

| id 1.20.1 | entrées de palette | fichiers concernés | cellules remplies |
| --- | --- | --- | --- |
| `minecraft:grass` | 1 715 | 1 715 | 6 192 |
| `minecraft:chain` | 1 313 | 1 196 | 3 123 |

Empreintes FNV-1a 64 (ids triés, joints par `\n`, sans saut final) : palette complète
`0x972a761eae851a4e`, sous-ensemble `minecraft` `0xbb689a70e206b72d`, registre 26.3
`0x7678fe4728f78011`. Elles sont dans `MEASURED` et vérifiées par les tests quand le fichier de
registre est présent sur la machine.

### Pourquoi le compte brut donnait 238

Un scan de **toutes** les chaînes du fichier ramasse les items et les entités. Les deux cas
connus montrent le mécanisme, mesuré sur les octets décompressés des 9 515 fichiers :

| id | occurrences brutes | entrées de palette | ce que sont les autres |
| --- | --- | --- | --- |
| `minecraft:grass` | 4 108 | 1 715 | 2 393 × `minecraft:grass_block` (préfixe) |
| `minecraft:chain` | 1 729 | 1 313 | 416 × `minecraft:chainmail_*`, items dans les `ArmorItems` d'entités |

C'est exactement le mécanisme qui produit `bread`, `arrow`, `bed` dans une liste de « blocs
manquants ». Sur la palette seule : **2**.

---

## 2. Table de correspondance (`src/compat/table.rs`)

Deux lignes, chacune avec sa preuve datée. Le nom 26.3 de chaque ligne a été vérifié **dans le
registre** (présent) et l'ancien nom vérifié **absent** en correspondance exacte — attention,
`minecraft:grass` est un préfixe de `minecraft:grass_block`, qui lui existe toujours, donc la
vérification est faite sur la clé JSON complète, pas par préfixe. Ce côté 26.3 est cité une fois
pour toutes par `table::REGISTRY_26_3` :
`.local/horizon-client/26.3/registries.json`, `["minecraft:block"]["entries"]`, 1 286 clés.

### `minecraft:grass` → `minecraft:short_grass`

- **Version** : Java 1.20.3, pré-version 1 (`1.20.3-pre1`).
- **Source** : `minecraft.wiki/w/Short_Grass`, § *Data history* — « 1.20.3 | Pre-Release 1 |
  The ID of "Grass", now "Short Grass", has been changed from `grass` to `short_grass` » ; et
  § *History* — « 1.20.3 | pre1 | "Grass" has been renamed to "Short Grass" ».
- **Nature** : c'est la **plante**, pas le bloc d'herbe. `grass_block` n'a pas bougé. La plante
  n'a aucun état de bloc, et `short_grass` non plus : les 1 715 entrées passent telles quelles.

### `minecraft:chain` → `minecraft:iron_chain`

- **Version** : Java 1.21.9, snapshot 25w35a (nom affiché changé en 25w32a, quand les chaînes
  cuivre ont été séparées).
- **Source** : `minecraft.wiki/w/Iron_Chain`, § *Data history* — « 1.21.9 | 25w35a | Renamed
  `chain` to `iron_chain` » ; § *History* — « 1.21.9 | 25w32a | "Chain" has been renamed to
  "Iron Chain" ».
- **Forme** : vérifié, **la chaîne n'a pas changé de forme**. `iron_chain` déclare `axis`
  (`x`, `y`, `z`) et `waterlogged` (`true`, `false`) — les quatre valeurs que la 1.20.1 avait
  déjà. Le pack écrit exactement `axis` = x/y/z et `waterlogged` = false.

### Aucune entrée « disparu », aucune entrée « Incertain »

Les 860 ids `minecraft` de la palette existent tous dans 26.3 **après** ces deux renommages :
aucun bloc n'a disparu (`Disposition::Disappeared` n'a aucune ligne, et un test le dit). Aucune
ligne n'est `Uncertain` : les deux cas ont une preuve datée. La variante existe pour un futur cas
non prouvable — une ligne sans preuve serait une invention, et aucun des deux cas connus n'en est
une.

### Les 157 ids de mods ne sont pas des lignes

`minecolonies`, `domum_ornamentum`, `structurize`, `multipiston` : ce sont les blocs du mod et de
ses dépendances. Ils ne peuvent pas être dans un registre vanilla, et ils ne manquent pas — c'est
du contenu à fournir. `resolve` répond `Foreign(namespace)` et l'audit les classe
`Gap::Foreign`, jamais « perdu par 26.3 ».

---

## 3. Sens inverse : interdit, et rendu impossible

La cible est 26.3. Trois garde-fous :

1. les champs s'appellent `from_1_20_1` et `to_26_3` — on ne peut pas les lire à l'envers ;
2. aucune fonction de la crate ne rend un nom 1.20.1 : `rename()` et `apply_to()` ne renvoient que
   des cibles 26.3, et `resolve("minecraft:short_grass")` répond `Kept` (le bloc est déjà bon) ;
3. un test vérifie qu'**aucune cible n'est une source** : `minecraft:grass` et `minecraft:chain`
   ne sont pas des blocs de 26.3, donc la table ne peut pas être parcourue à rebours même par
   erreur.

---

## 4. Les états

### Mesure

Les palettes portent **4 047 triplets distincts (bloc, propriété, valeur)** — 3 400 en
`minecraft` — répartis sur 629 blocs (526 `minecraft`) et 79 noms de propriétés (`facing`,
`axis`, `waterlogged`, `half`, `shape`, `age`, `instrument`, les seize `has_bottle_*`…). Chacun a
été comparé au rapport de blocs du build épinglé (`blocks.json` de la datagen vanilla, carte
`properties` par bloc, valeur par valeur).

### Résultat : zéro état perdu

**Aucune propriété du pack n'a disparu, et aucune valeur non plus**, une fois les deux blocs
renommés relus sous leur nom 26.3. Le doute du brief sur `chain` est levé par la mesure : les
1 313 entrées écrivent `axis` (x/y/z) et `waterlogged=false`, `iron_chain` accepte exactement ces
propriétés et ces valeurs, et accepte en plus `waterlogged=true` (que le pack n'utilise pas).

Rien n'a donc été corrigé — et rien ne **devait** l'être. `states::audit` ne modifie jamais un
état : il renvoie des `Gap` (bloc inconnu, propriété non déclarée, valeur refusée) et laisse la
décision au port, parce qu'un escalier dont `shape` a disparu doit être **vu**, pas transformé
silencieusement en escalier droit.

### Ce qui reste vérifiable

`states::Report` est un oracle remplissable : `SPECS_26_3` ne contient que les deux blocs que le
port atteint sous un ancien nom (transcrits depuis les pages, avec la source), et le reste est
demandé au jeu. C'est volontaire : embarquer la carte des 1 286 blocs coûterait ~20 fois la
taille de la crate et serait périmé au build suivant.

`Report::knows_block(bloc)` répond à la seule question que l'audit pose d'abord : **ce bloc est-il
connu du registre 26.3 ?** L'information est portée par le champ privé `Report::blocks`
(`BTreeMap<String, BTreeMap<String, Vec<String>>>`), et la réponse est exactement
`self.blocks.contains_key(bloc)` : `true` pour un bloc déclaré même sans aucune propriété
(`declare_block`, le cas de `short_grass`), `false` pour un bloc que personne n'a déclaré. C'est
la moitié 26.3 de chaque `Gap` : un état dont le bloc est inconnu donne `Gap::NotIn26_3`, pas un
problème de propriété. `StateOracle::knows` délègue à cette méthode, il n'y a donc qu'une seule
définition de « connu ».

---

## 5. Limites, écarts, ce que je n'ai pas fait

- **Écart de comptage sur `chain`** : le brief annonce 1 314 occurrences, la palette en donne
  **1 313** (dans 1 196 fichiers). Je n'ai pas d'explication prouvée pour cette unité ; le
  comptage brut, lui, en trouve 1 729 dont 416 `chainmail_*`. Chiffre mesuré retenu : 1 313.
- **Le « 238 » du brief n'est pas re-dérivé ici** : il vient d'un scan de chaînes, et je n'ai pas
  reproduit exactement son filtre. Ce qui est établi, c'est la palette : 2.
- **Pas de préfixe piégeux** : en 1.20.1 l'herbe (le bloc) s'appelle déjà `grass_block` depuis la
  « flattening » 1.13, donc `minecraft:grass` ne désigne que la plante ; et la vérification
  d'absence dans 26.3 se fait sur les clés complètes du registre, jamais sur du texte.
- **Rien n'appelle encore `compat`** : `structure_nbt::convert` écrit aujourd'hui les noms de la
  palette tels quels. Brancher `apply_to` sur la palette puis `audit` sur la structure est un
  changement dans `src/blueprint/`, que cette tranche n'a pas le droit de toucher — la brique est
  prête, le câblage reste à faire par le propriétaire de ce module.
- **Pas de table inverse**, pas de renommage appliqué aux blueprints ni à `data/` : la
  conversion se fait à l'exécution, par le port, avec `apply_to` + `audit`.
- **Rien n'a été compilé ici** (pas de toolchain Rust dans le conteneur) : la crate est laissée à
  l'orchestrateur. Les tests lisant le pack ou le registre sont écrits pour **sauter** proprement
  quand le fichier de registre est absent d'une machine, jamais pour échouer.

## 6. Reproduire la mesure

```sh
# l'échantillon du pack + la couverture de la table (rapide)
cargo test -p ported-minecolonies --lib compat

# la mesure entière : 9 515 fichiers, 1 017 ids, empreintes
cargo test -p ported-minecolonies --lib compat -- --ignored

# registre à un autre chemin
HORIZON_REGISTRY_26_3=/chemin/registries.json cargo test -p ported-minecolonies --lib compat
```

## 7. Inventaire des tests (26)

Le module fait **693 lignes de code** (docs comprises, hors tests) et **26 tests** (582 lignes).

`mod.rs` (7) : grass → short_grass · chain → iron_chain · un nom 26.3 reste tel quel et un id
vanilla inconnu n'est pas deviné · namespace de mod = `Foreign` · `apply_to` garde les propriétés
et ne touche pas l'entrée · id sans namespace = `minecraft` · la table n'a aucune entrée inverse.

`table.rs` (8, dont 1 `#[ignore]`) : la mesure est cohérente en interne · la table couvre
exactement l'ensemble mesuré et n'ajoute rien · aucun cas connu oublié et la correspondance reste
exacte (pas de préfixe) · chaque ligne cite une version et une page, aucune ligne devinée, aucun
bloc disparu · le renommage ne se lit pas à l'envers · **le pack réel n'a que des ids couverts par
la table** · le registre du client confirme les deux cibles (et que `grass_block` est toujours
là) · *(ignoré)* le pack entier redonne les chiffres et les empreintes enregistrés.

`states.rs` (11) : les deux blocs transcrits sont ceux de la table · un état déclaré passe ·
les deux cas connus sont réécrits puis passent (et les trois axes d'une chaîne existent) ·
`axis=w` signalé et **non corrigé** · propriété non déclarée signalée · bloc ou item inconnu
signalé · bloc de mod = `Foreign` · le même problème n'est signalé qu'une fois · une palette
s'audite directement · un `Report` ne répond que de ce qu'on lui donne (et les déclarations
s'ajoutent) · une palette vide est propre.

Les deux tests qui lisent le pack passent par `Blueprint::parse` — le lecteur du port, pas une
regex — et ceux qui ont besoin du registre **sautent** quand le fichier n'est pas là.
