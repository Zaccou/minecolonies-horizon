# `colony_world` — ce qui manque, et ce que je voulais faire

Chaque entrée : ce que le module voudrait faire, la ligne du SDK qui l'en empêche, et le
coût pour le port. Aucune de ces fonctions n'est écrite en code fictif.

## 1. `place_template` n'est pas joignable depuis un mod

**Voulu** : poser un bâtiment d'un coup, comme `StructurePlacer` le fait côté jeu, avec
les données de block entity, les entités du template, les mises à jour de voisins et le
maintien des chunks chargés.

**Réalité** : `place_template` existe dans le moteur
(`crates/horizon-world/src/level/bonemeal.rs:162`,
`horizon-client/src/game/administration/structure_blocks.rs:384`) mais **aucune méthode
hôte ni hook ne l'expose**. La zone `world_edit` du SDK déclare exactement six appels :
`edit_blocks`, `upgrade_block_entity_nbt`, `upgrade_entity_nbt`, `surface_heights`,
`read_columns`, `structure_template`
(`crates/horizon-mod-vocab/src/host/world_edit.rs`).

**Conséquence** : un mod lit le template avec `structure_template` et écrit chaque bloc
avec `edit_blocks`. Ce que cette recomposition perd :

| Perdu | Source | Conséquence |
| --- | --- | --- |
| les données des block entities | `game.rs:28` — « NBT is not supported » | un coffre pasté est vide ; d'où `Change::Keep` (`intent.rs`) et `Refusal::WouldDropContents` |
| les entités du template | `world_edit.rs:74-79` — « the template's entities are left out » | pas de porte-armure, pas d'item frame, pas d'animal dans un bâtiment |
| les mises à jour de voisins | `world_edit.rs:12` — « no neighbour is told » | un mur pasté ne se connecte pas ; `UPDATE_FLAG` (`engine.rs`) est porté mais non utilisable |
| les tickets de chunk | `Colony.java:543`, `:596` — Java garde les chunks de la colonie chargés | un `edit_blocks` dans un chunk déchargé est **silencieusement ignoré** (`world_edit.rs:10-20`) ; d'où `Refusal::Unloaded` |

## 2. `edit_blocks` ne prend aucun flag

**Voulu** : choisir `Level.setBlock`'s flags, comme Java le fait avec `UPDATE_FLAG`
(`api/util/constant/Constants.java:32`, `0x03`).

**Réalité** : la signature est `edit_blocks(&self, dimension, edit: &BlockEdit)`
(`world_edit.rs:19-20`) — pas de paramètre `flags`.

**Conséquence** : `engine::UPDATE_FLAG` est porté comme valeur documentée, jamais passée.
Toute écriture du pont se comporte comme `fill … strict`. Un appelant qui a besoin des
voisins doit le demander hors du pont.

## 3. Aucune chute n'est produite

**Voulu** : casser un bloc comme `Level.destroyBlock`, qui émet les drops dans
l'inventaire de la colonie.

**Réalité** : il n'y a pas d'équivalent `destroyBlock` dans le SDK ; `edit_blocks` écrit
un état, donc écrire `minecraft:air` (`PathingStuckHandler.java:506`) détruit le bloc sans
rien produire.

**Conséquence** : `Change::Break` dit exactement ce qu'il fait (« the block is gone, and
its drops are not produced », `intent.rs`). Un chantier qui démonte un bâtiment ne
récupère rien. À traiter quand un `ai_build` voudra rendre les matériaux.

## 4. Le résultat d'un lot n'est pas attribuable

**Voulu** : savoir quelles positions n'ont pas été écrites, pour ne pas marquer un
chantier comme fini.

**Réalité** : `edit_blocks` répond `Result<u64, String>` — **le nombre** de blocs changés,
pas lesquels (`world_edit.rs:20`). Le `Err(String)` est une raison globale, pas par run.

**Conséquence** : après un lot de `n` écritures, un `u64 < n` dit qu'il en manque mais pas
où. La reprise doit relire les colonnes (`read_columns`) pour retrouver les positions.

## 5. Lire une zone coûte cher et par colonnes

**Voulu** : lire le contenu d'un plot d'un coup, pour décider `Occupant` sur tout un
gabarit.

**Réalité** : `read_columns` limite à 1 048 576 blocs et tourne sur le tick du serveur
(`world_edit.rs:60-68`), et ne lit que des colonnes entières.

**Conséquence** : `guard::World` est une requête **par position** ; une lecture de plot
complète est à faire par l'appelant, en morceaux, pas par ce module.

## 6. Les claims d'un chunk ne sont pas sur le chunk

**Voulu** : lire et écrire les claims là où Java les met, la capability `LevelChunk`
(`IColonyTagCapability.java:295-310`).

**Réalité** : le handle de chunk du SDK n'a nulle part où accrocher une donnée de mod
(`crates/horizon-mod-vocab/src/host/levels.rs:13-17`) et n'expose aucun NBT de chunk.

**Conséquence** : [`crate::manager::ClaimBook`] garde les mêmes trois champs et les
sauvegarde à côté des colonies (déjà traité et documenté par `manager` ; ce module ne fait
que passer par lui). Un monde ouvert par une version du port qui ne les sauvait pas
revient sans claims : un `Mark::ClaimGround` doit alors être rejoué.

## 7. Aucune permission n'est vérifiée ici

**Voulu** : que « poser un bloc » demande `Action::PlaceHuts` ou `Action::ModifyBlocks`
comme Java le fait dans `SurvivalHandler.java:118-124`.

**Réalité** : ce n'est pas une limite du SDK, c'est une décision de découpage : une
permission demande un joueur, et `Guard` n'en a pas.

**Conséquence** : c'est à l'appelant qui a un joueur de poser la question, par
`colony::Colony::may`. Ce module ne la duplique pas.
