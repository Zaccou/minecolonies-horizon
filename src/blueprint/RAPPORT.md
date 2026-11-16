# RAPPORT — `src/blueprint` : lire les 9 515 blueprints et rendre du NBT de structure

Date : 16/11/2026. Dépôt : `/workspace/port-minecolonies` (hôte
`~/opencode-server/workspace/port-minecolonies`). Écriture limitée à `src/blueprint/` : ni `lib.rs`,
ni `Cargo.toml`, ni un autre module. **Aucune compilation lancée d'ici** (règle 2) : les chiffres de
lignes et de tests ci-dessous sont des comptes de lignes, pas des comptes de tests verts.

## 1. Ce que le module fait

```text
bytes d'un .blueprint (gzip + NBT)
  │  gzip::gunzip          RFC 1952, écrit à la main
  │  inflate::inflate      RFC 1951, écrit à la main (aucune dépendance possible)
  │  nbt::parse            arbre NBT = mod_api::{Compound, Tag}, l'arbre du moteur
  ▼
format::Blueprint          taille, palette, cellules (2 index de 16 bits par int), block entities,
  │                        entités, primary_offset — et un refus nommé par cas d'échec
  ▼
structure_nbt::convert     Structure { DataVersion, size, palette, blocks, entities } + Anchor
  │
  ├── Conversion::to_compound()      l'arbre, prêt pour un placeur
  └── Conversion::to_nbt() / to_gzipped_nbt()   les octets d'un document / d'un vrai .nbt
```

Différences **voulues** avec `port-blueprint` (le crate autonome, qui reste la référence et qui a
validé le format sur 9 515/9 515 fichiers) :

| `port-blueprint` | ici | pourquoi |
|---|---|---|
| arbre NBT propre (`Tag`/`Compound` maison) | `mod_api::{Compound, Tag}` | un payload de block entity doit être rendu au jeu tel quel : un second arbre aurait obligé à traduire deux fois |
| `Blueprint::open(path)`, `Error::Io` | `Blueprint::parse(&[u8])` seulement | les octets viennent de la pile de packs (`Host::resource`), pas d'un chemin : pas de variante `Io` à porter |
| `unwrap()` dans les tests et `expect` sur des tables | aucun `unwrap`/`expect`/`panic!` hors `#[cfg(test)]` | règle 5 : chaque refus est un variant nommé |
| `tests/` d'intégration + un binaire `bp2nbt` | tests unitaires en `#[cfg(test)]` | `cargo test --lib` (BUILD.md §5) et `src/blueprint/corpus.rs` lit le pack réel |

## 2. Fichiers, lignes, tests

| Fichier | Lignes | dont tests | Tests |
|---|---:|---:|---:|
| `mod.rs` (doc du module, pipeline, ré-exports) | 80 | 0 | — |
| `format.rs` (le modèle + le lecteur, format documenté champ par champ) | 1254 | 403 | 21 |
| `structure_nbt.rs` (conversion, ancre, écriture + relecture) | 1315 | 487 | 14 |
| `nbt.rs` (couche octets sur l'arbre du moteur) | 789 | 221 | 16 |
| `gzip.rs` (conteneur RFC 1952) | 353 | 125 | 8 |
| `inflate.rs` (DEFLATE RFC 1951) | 448 | 94 | 9 |
| `corpus.rs` (**test seulement**) | 136 | 48 | 3 |
| **total** | **4375** | **1378** | **71** |

La cible annoncée était 1 800–2 400 lignes : **dépassée de ~80 %**. Répartition honnête : ~2 990
lignes de code (dont ~450 de documentation de format, exigée par la règle 3) et ~1 380 lignes de
tests. Le code seul est proche du crate porté (3 215 lignes *tests compris*) ; l'écart vient de
`nbt.rs` (568 lignes qui n'existaient pas là-bas : il réutilisait son propre arbre) et du nombre de
tests (71 contre ~30 demandés). Rien n'a été ajouté hors sujet : pas de placement, pas de rotation.

## 3. Les blueprints réellement touchés, et pourquoi ceux-là

**35 fichiers distincts**, tous lus depuis `data/minecolonies/blueprints/minecolonies/` :

| Fichiers | Ce que le test en tire |
|---|---|
| `darkoak/fundamentals/builder1…5.blueprint` | **les niveaux 1 à 5** d'un même bâtiment : 14×14×15, 2 940 cellules, palette 44 → 143, block entities 2 → 14, ancre (7,5,3) |
| `darkoak/agriculture/husbandry/fisherman1…5.blueprint` | **les niveaux 1 à 5** d'un second bâtiment : 12×15×21, 3 780 cellules, palette 32 → 66, block entities 1 → 4, ancre (8,7,17) |
| `original/infrastructure/misc/fill.blueprint` | le seul fichier du pack **sans air** : palette à une entrée (`structurize:blocksolidsubstitution`), 8×8×8 = 512 cellules toutes non-air — le cas qui interdit de supposer « index 0 = air » |
| 24 fichiers par pas de 396 dans la liste triée | le **test de comptage** : `convert` réussit et `structure.blocks.len() == bp.blocks.len()` sur chaque fichier |

Pourquoi ce choix :

1. **Un niveau par niveau, sur deux bâtiments.** La couverture 1→5 demandée est réelle (pas un
   même fichier cinq fois), et deux bâtiments différents prouvent que le lecteur ne dépend pas
   d'une taille : l'un garde son empreinte et grossit en palette, l'autre aussi mais avec une
   autre géométrie et d'autres block entities (`rack`, `furnace`, `banner`, `colonybuilding`).
2. **`fill.blueprint`** est le contre-exemple du corpus : une palette d'une seule entrée, sans air,
   nommée `dig`. Un lecteur qui suppose `palette[0] == air` passe les 9 514 autres tests et échoue
   ici.
3. **Les 24 échantillonnés** viennent d'un pas régulier sur les 9 515 chemins triés, donc ils
   traversent **22 des 25 styles** et toutes les catégories (roads, mineshafts, military,
   craftsmanship, decorations…), au lieu de 24 voisins du même dossier. Convertis en entier, ils
   couvrent plus de 100 000 cellules — dont `original/decorations/supplies/ship.blueprint`
   (14 688 cellules, 212 block entities) — pour ~1 Mo décompressés, donc un test rapide.
   Quatre d'entre eux subissent en plus l'aller-retour NBT complet (écriture puis relecture
   identique, payloads et entités compris).

Convertir les 9 515 fichiers en test coûterait 338 Mo et ~50 s ; c'est fait une fois pour toutes
par `port-blueprint` (`tests/corpus_conversion.rs`), et c'est cité comme tel, pas refait ici.

## 4. Chiffres mesurés sur le corpus (relus ce soir, pas recopiés)

| | |
|---|---|
| fichiers | **9 515** (24,1 Mio gzippés, 337,9 Mio décompressés) |
| cellules au total | **47 564 857** |
| block entities | **1 055 050** |
| entités | **7 023**, toutes avec `Pos` (0 sans) — `minecraft:item_frame` 5 908, `armor_stand` 679, `painting` 309 |
| plus gros fichier | `original/infrastructure/mineshafts/mediumquarryshaft1.blueprint` : 942 967 octets décompressés, 34×57×41 = 79 458 cellules, 3 928 block entities |
| `mcversion` | 3465 (1.20.1) partout, `architects` absent partout |

Les valeurs épinglées dans les tests (`fisherman2` : 40 entrées de palette, 1 543 cellules non-air,
ancre (8,7,17) sur `minecolonies:blockhutfisherman[facing=south]`, entité à
`[4.5, 9.5, 17.96875]` ; `builder5` : palette 143) sortent de ces mesures, pas d'une convention.

## 5. Ce qui est vérifié, et ce qui ne l'est pas

**Vérifié par les tests écrits ici** (à faire tourner par l'orchestrateur) : le DEFLATE réel (les 24
fichiers passent par le décodeur Huffman dynamique), le conteneur gzip avec ses drapeaux, l'UTF-8
modifié (NUL et U+1F600), l'ordre des cellules y→z→x, l'entrelacement des index dans les `int`, la
cellule de bourrage quand le volume est impair, les coordonnées de block entity en short **et** en
int, les refus nommés (version ≠ 1, taille incohérente, index hors palette, palette vide, dimension
nulle, doublon de block entity, block entity hors grille, entité sans `Pos`), les deux orthographes
de palette, l'ancre déclarée et ses deux replis, l'aller-retour NBT sur trois fichiers réels.

**Non vérifié, et pourquoi** :

* **Rien n'a été compilé depuis ce conteneur** (règle 2). Si un test tombe, c'est ici qu'il faut
  regarder d'abord : les valeurs épinglées (palettes, comptes, cellules) sont mesurées, mais une
  erreur de type ou un `matches!` sur une valeur déplacée reste possible et invisible pour moi.
  Les 71 tests ont été écrits relus un par un ; aucun n'a été exécuté.
* **Le NBT produit n'a pas été relu par le jeu.** La forme suit `template.rs:276-322`
  (`StructureTemplate::save`), et l'aller-retour passe par mon propre lecteur : c'est une preuve de
  cohérence, pas une validation externe. La validation externe existe côté `port-blueprint`
  (1 837 templates vanilla relus), elle n'est pas refaite ici.
* **La pose dans le monde** : hors périmètre (voir `MANQUANT.md` §2 — le SDK ne prend pas un
  document, seulement un id de template).

## 6. Dettes et manques assumés

`MANQUANT.md` (dans ce dossier) détaille six points, chacun avec la déclaration du SDK citée : la
couche NBT octets réimplémentée faute de réexport de `read_named`/`write_named`, l'absence de
méthode qui pose un document, la palette non validée contre le registre, les data fixers non
appliqués, la rotation/miroir non réimplémentée, et la lecture de fichier par `Host::resource`
seulement (64 Mio de plafond, 943 kB au maximum réel).

**La jonction avec `src/structures/` n'est pas faite, et elle n'est pas à moi.** Ce module-ci rend
un `Conversion { structure, anchor }` (+ ses octets) à partir d'octets de blueprint ; le module
`structures` d'un autre agent résout *quel* fichier (style × bâtiment × niveau) et *où* le poser, et
travaille aujourd'hui sur les `.nbt` générés. Brancher les deux tient en un appel
(`structure_nbt::convert(&Blueprint::parse(&bytes)?)`) plus l'ancre passée au placeur ; je ne l'ai
pas écrit parce que ce serait écrire dans un module que je ne possède pas.

Deux choix qui pourraient se discuter :

* `gzip::gzip` écrit des blocs *stored* : la sortie est un membre gzip valide mais non compressé.
  Sans compresseur, un document de 3 780 cellules pèse ~10 % de plus qu'un déflate réel. Le mod
  n'écrit aucun fichier, donc c'est sans coût à l'exécution — sauf si un jour on emballe les
  documents dans un pack (à ce moment-là, il faudra un compresseur).
* Les propriétés d'un état de bloc sont **triées par nom**, pas dans l'ordre de déclaration du bloc
  (vanilla). Un lecteur de NBT lit par clé, donc rien ne casse ; c'est écrit dans la doc de
  `BlockState`.
