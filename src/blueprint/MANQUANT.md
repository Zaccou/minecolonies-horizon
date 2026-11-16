# MANQUANT — ce que `src/blueprint` voulait faire et que le SDK ne permet pas

Écrit pour `src/blueprint` (lecture des 9 515 blueprints + conversion en NBT de structure).
Chaque entrée dit **ce que je voulais faire**, **ce que le SDK offre réellement** (avec la
déclaration citée), **ce que j'ai fait à la place**, et **ce que ça coûte**.

Les déclarations citées sont dans `.horizon/sdk/api/crates/horizon-mod-vocab/src/` (SDK épinglé
`4dc6455d`, `Cargo.toml`).

---

## 1. Lire et écrire du NBT octet par octet — réimplémenté ici

**Voulu.** `Blueprint::parse(&[u8])` reçoit les octets d'un fichier (gzip autour d'un document
NBT) et doit en sortir un arbre ; `convert().to_nbt()` doit rendre les octets d'un document. Le
moteur sait faire les deux : `horizon_core::nbt::read_named` (`horizon-core/src/nbt.rs:743`) et
`horizon_core::nbt::write_named` (`:1026`), avec `Limits` (`:223`).

**Ce que le SDK offre.** L'arbre seulement : `pub use horizon_core::nbt::Compound;` et
`pub use horizon_core::nbt::Tag;` (`exports/horizon_world.rs:7-8`), plus
`pub use horizon_core::nbt::{Error as NbtError, NETWORK_NBT_QUOTA};`
(`exports/horizon_core.rs:13`). Aucun `read_named`, aucun `write_named`, aucun `Limits` : un mod
ne peut ni lire ni écrire un document. `horizon-core` n'est pas non plus une dépendance de cette
crate (`Cargo.toml`, une seule dépendance, et cette tranche n'en ajoute aucune).

**Fait à la place.** `src/blueprint/nbt.rs` (568 lignes) : lecteur et écrivain au-dessus de
`mod_api::{Compound, Tag}`, avec les mêmes règles que le moteur — ids 1..12, longueurs `i32`
signées, **UTF-8 modifié** (U+0000 en `C0 80`, supplémentaire en deux surrogates,
`horizon-core/src/nbt.rs:709-739` et `:892-921`), listes homogènes, liste vide de type 0,
profondeur maximale 512.

**Coût.** Une logique dupliquée, et un détail qui ne peut pas être fidèle : `JavaString` **n'est
pas réexporté**, donc son type ne peut pas être nommé ici. Les chaînes passent par `&str` à
l'écriture et `to_string_lossy()` à la lecture : un surrogate isolé devient U+FFFD. Aucun fichier
du corpus n'en contient (ids, propriétés, clés, chemins sont ASCII), mais ce n'est pas gratuit en
théorie. Le correctif est une réexportation : `pub use horizon_core::nbt::{read_named,
write_named, Limits, JavaString};` dans `exports/horizon_core.rs`.

---

## 2. Poser une structure convertie — aucune méthode ne prend un document

**Voulu.** Une fois la conversion faite, la poser dans le monde : c'est tout l'objet du port.

**Ce que le SDK offre.** `Host::structure_template(&mut self, id: &str, at: [i32; 3], rotation:
StructureRotation, mirror: StructureMirror) -> Option<StructureTemplateSnapshot>`
(`host/world_edit.rs:84-91`) : elle prend un **identifiant de template déjà installé** (trouvé
comme `StructureTemplateManager.get`), **sans poser** et **sans accepter d'octets**. Côté modèle,
`StructureTemplateSnapshot` / `StructureTemplateBlock` (`world/structure_template.rs:36-56`)
portent des `BlockState` **résolus** (`block_state.rs`), qu'un document NBT ne contient pas : il
contient des *noms* de blocs.

**Fait à la place.** Le convertisseur s'arrête honnêtement à deux formes utilisables par
l'appelant : `Conversion::to_compound()` (l'arbre du document) et `Conversion::to_nbt()` /
`to_gzipped_nbt()` (ses octets). L'ancre est rendue à côté (`Conversion::anchor`), parce que le
format vanilla n'a pas de champ pour elle.

**Coût.** Il manque un chaînon : soit une méthode du genre `Host::place_structure(document,
at, rotation, mirror)`, soit une résolution de palette hors contexte de génération
(`BlockMetadata::state(name, properties) -> Option<BlockState>`), soit l'écriture du document dans
un pack. Tant que ce chaînon n'existe pas, aucun code de ce module ne peut poser un bloc — et il
ne prétend pas le contraire.

---

## 3. Valider les blocs d'une palette — pas de registre à ce niveau

**Voulu.** Savoir si `structurize:blocksolidsubstitution` (palette de
`original/infrastructure/misc/fill.blueprint`) ou `domum_ornamentum:double_crossed` (1 185 blocs de
`mediumquarryshaft1`) existent, et si leurs propriétés sont légales.

**Ce que le SDK offre.** `BlockMetadata` (`block_metadata.rs`) : `fn state(&self, key: BlockKey)
-> BlockState` et un résolveur qui **refuse** un état inconnu — mais il n'est atteignable que
depuis `GenerationContext::metadata()` (`world/structure_type.rs`) ou `PieceContext::metadata()`,
c'est-à-dire **pendant une génération de structure**, jamais depuis un mod ordinaire. Aucune
méthode de `Host` ne prend un id de bloc en texte pour rendre un `BlockKey`.

**Fait à la place.** La palette sort telle quelle : nom + propriétés triées par nom
(`BlockState`), sans validation. Le convertisseur dit ce qu'il ne fait pas (module `format`, §« What
this module does not do »).

**Coût.** Un blueprint d'un mod absent produit un document syntaxiquement valide dont une partie
des blocs ne se résoudra pas au placement, et c'est le placeur qui le découvrira, plus tard.

---

## 4. Appliquer les data fixers — exposé, jamais appliqué

**Voulu.** `readBlueprintFromNBT` remonte une palette d'une vieille version de données
(`blueprints/v1/BlueprintUtil.java:285-359`, `:575-578`) quand `mcversion` est absent (défaut 1343
= 1.12.2, `:43`).

**Ce que le SDK offre.** Rien : aucune API de mise à niveau d'une palette ou d'un `Compound` de
bloc (`upgrade_entity_nbt` existe pour les **entités**, `host/world_edit.rs:40-44`, pas pour les
blocs).

**Fait à la place.** `data_version` est lu, exposé et écrit (`DataVersion` du template), jamais
utilisé pour réécrire la palette.

**Coût.** Nul sur le corpus : **les 9 515 fichiers portent `mcversion` 3465** (1.20.1). Un pack
tiers qui livrerait des blueprints 1.12 serait lu avec ses vieux ids (`Blockshingle_…`) et
refusé au placement, pas converti.

---

## 5. Tourner ou mirroir un blueprint — le placer est possible, pas le tourner

**Voulu.** Une hut posée « facing: east » demande la structure tournée ; Java tourne le blueprint
lui-même (`blueprints/v1/Blueprint.java:628-770`, `rotate`/`mirror` avec réécriture des propriétés
`facing`, `axis`, `north/south/east/west`).

**Ce que le SDK offre.** `StructureRotation` / `StructureMirror`
(`world/structure_template.rs:8-30`) n'existent que comme **arguments** de
`Host::structure_template` (par id de template). Aucune fonction ne tourne un document en mémoire,
et `StructureTemplateSnapshot` est le résultat déjà tourné d'un template du jeu.

**Fait à la place.** `convert` rend la structure **telle que scannée** et le dit dans la doc du
module. `PaletteDialect` ne change que l'écriture de la palette, jamais la géométrie.

**Coût.** Le port devra soit écrire les quatre orientations à la conversion (propriétés de blocs
comprises — c'est un travail de table, pas d'API), soit attendre une API de rotation de document.

---

## 6. Lire un fichier de blueprint — une seule voie, et elle suffit

**Voulu.** Lire `data/minecolonies/blueprints/…/*.blueprint` à la demande.

**Ce que le SDK offre.** `Host::resource(&self, pack_type: PackType, identifier: &Identifier) ->
Option<Vec<u8>>` (`host/resources.rs:20`) : les octets gagnants d'un chemin dans la pile de packs
de données du serveur, plafonnés à `MAX_RESOURCE_BYTES` = 64 MiB
(`client/pack_finders.rs:8`). C'est la bonne source et pas un chemin disque : elle suit la pile de
packs du serveur (donc les data packs de mods et `/datapack`), là où `std::fs` lirait un fichier que
rien ne surcharge.

**Fait à la place.** Rien à faire : c'est la bonne voie, et `Blueprint::parse` prend exactement
`&[u8]`. Le corpus des tests, lui, lit `data/` par `std::fs` — **sous `#[cfg(test)]` uniquement**
(`src/blueprint/corpus.rs`, `mod.rs` le déclare en `#[cfg(test)]`), donc absent du binaire du mod.

**Coût.** Aucun mesuré : le plus gros blueprint du corpus fait **943 kB décompressés**
(`original/infrastructure/mineshafts/mediumquarryshaft1.blueprint`, 79 458 cellules, 3 928 block
entities), soit 68 fois sous le plafond.
