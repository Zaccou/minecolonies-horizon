# MANQUANT — ce que « où une colonie peut apparaître » réclame et que le port n'a pas

Écrit à la main pour ce module. Chaque entrée dit **ce qui manque, d'où ça vient, ce que ça
coûte, et par où ça sortirait** — jamais « à faire plus tard » sans mesure.

Les cinq fichiers de `src/worldgen/` n'appellent aucune fonction de l'hôte : ils lisent des
faits que l'appelant a observés (`WorldFacts`, `Column`, `Air`, `Biomes`). Les entrées
ci-dessous sont donc les **appels d'hôte** qu'un appelant réel devra faire pour les remplir,
et les deux règles que ces appels ne peuvent pas rendre.

---

## 1. Le tirage `random_spread` n'est pas rejouable — les candidats viennent du dehors

**Ce qui manque.** La suite de chunks candidats d'un `structure_set`. Vanilla la calcule par
région avec `RandomSource` semé par le `salt` (`RandomSpreadStructurePlacement`,
`WorldgenRandom.seed`), et `ChunkGenerator` ne l'expose pas.

**D'où ça vient.** Le sel et la géométrie sont dans les données
(`worldgen/structure_set/empty_colony.json:53-56`), mais la suite elle-même dépend du **seed
du monde**, que le mod ne peut ni lire ni rejouer (`RandomSource` est un xoroshiro interne).
Les appels d'hôte voisins ne suffisent pas : `level_find_nearest_structure`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/levels.rs:39-66`) cherche un
**commencement de structure** par id ou par tag, pas « ce chunk est-il un candidat de ce
set ».

**Ce que ça coûte.** `Question::candidates` est une liste que l'appelant fournit ; `choose`
essaie les chunks dans cet ordre et ne vérifie pas qu'ils respectent `spacing`/`separation`.
`spread::Spread` porte la géométrie (région, bornes d'offset, écart minimal) pour qu'un
appelant puisse la **vérifier** — un test le fait — mais rien ici ne peut la **produire**.

**La sortie.** Un appel d'hôte `structure_set_candidates(level, set, region) -> Vec<[i32;2]>`,
ou un `level_find_nearest_structure` par style qui, appelé à rayon 0 sur un chunk, dirait
seulement « oui/non ». Les deux sont des ajouts d'API : `src/worldgen/MANQUANT.md` est leur
seule trace côté port.

---

## 2. `getFirstOccupiedHeight` / `getFirstFreeHeight` n'ont pas d'équivalent

**Ce qui manque.** La hauteur de la surface d'une colonne, celle que lit
`EmptyColonyStructure.isFeatureChunk` (`:84`) puis `findGenerationPoint` (`:129`).

**D'où ça vient.** Ces deux méthodes appartiennent au `ChunkGenerator` et **génèrent** la
colonne (elles évaluent le bruit) : un mod chargé après la génération ne les a pas.
`level_block_floor_height` (`host/levels.rs:76-134`) est une hauteur de **collision**
(`BlockGetter.getBlockFloorHeight`), pas une heightmap : une feuille ou une plaque de neige y
changent la réponse, pas dans `WORLD_SURFACE_WG`.

**Ce que ça coûte.** `Column::top` et `Column::free` sont deux entiers que l'appelant doit
mesurer — au plus juste en balayant la colonne avec `level_block_state`
(`host/levels.rs:304-331`) du haut vers le bas, ce qui coûte une lecture par bloc et ne voit
pas la heightmap `_WG` (celle d'avant les features). Une colonne non mesurée n'est pas
« vide » : `choose` répond `NotSurveyed`.

**La sortie.** Un `level_heightmap(level, pos, kind) -> Option<i32>` pour
`WORLD_SURFACE_WG` et `MOTION_BLOCKING`, ou un `level_column(level, pos) -> Option<Column>`.

---

## 3. « Une structure commence déjà dans ce chunk » n'est pas lisible

**Ce qui manque.** La liste des `StructureStart` d'un chunk, ce que `StructureCheck` consulte
avant de placer une pièce et que `Refusal::StructureInTheWay` demande.

**D'où ça vient.** Le `ChunkAccess` porte ses starts (`getStartForStructure`), et aucun appel
d'hôte ne les expose : `level_find_nearest_structure` (`:39-66`) prend un rayon, cherche un
**id de structure** et rend un résultat ; il ne dit pas « ce chunk en contient un ».

**Ce que ça coûte.** `WorldFacts::structures` est une liste de chunks fournie par l'appelant.
Sans elle, la règle 3 de `site` est simplement muette : la colonie générée peut recouvrir un
village ou une forteresse du Nether — le cas qui compte, parce que
`warpednetherlands_colony` est la seule structure à passer par les cavernes
(`worldgen/structure/warpednetherlands_colony.json:16`).

**La sortie.** `level_structure_starts(level, chunk) -> Option<Vec<String>>`.

---

## 4. `terrain_adaptation: "beard_box"` ne peut pas être demandé par un mod

**Ce qui manque.** Le remodelage du sol sous les pièces — ce que les seize fichiers demandent
(`colonial_colony.json:14`), et la raison pour laquelle **Java n'a aucune règle de pente**.

**D'où ça vient.** C'est le `ChunkGenerator` qui applique `beard_box` pendant la génération ;
un mod chargé après coup écrit des blocs, il ne relance pas la génération.

**Ce que ça coûte.** Le port offre `Question::with_max_step`, **désactivé par défaut**, qui
refuse un site dont deux colonnes voisines diffèrent de plus que le seuil. C'est une règle du
port, pas de Java, et elle est marquée comme telle dans `Refusal::TooSteep`. Un serveur qui
l'active obtient un site plus plat que ce que le jeu produirait ; un serveur qui la laisse
éteinte (le défaut) accepte exactement ce que Java accepte.

**La sortie.** Rien côté API : c'est une limite du modèle « mod après génération ». La règle
reste une option documentée, pas une prétention de parité.

---

## 5. La sonde de caverne lit une `NoiseColumn`, le port lit des blocs

**Ce qui manque.** `ChunkGenerator.getBaseColumn(...)`, la colonne de bruit d'avant les
features.

**D'où ça vient.** `isFeatureChunkCave` interroge cette colonne
(`EmptyColonyStructure.java:166`), donc du terrain **avant** les structures, les minerais et
les features. Un mod ne voit que le monde généré : `Air` répond sur des blocs posés.

**Ce que ça coûte.** Une poche d'air de 32 blocs dans le monde fini peut avoir été remplie par
une feature, et inversement une caverne de bruit peut être bouchée par une forteresse. La
règle est la même, la mesure n'est pas prise au même moment — c'est nommé ici plutôt que
caché.

**La sortie.** Aucune réaliste : ce serait un crochet de génération, hors du modèle de mod du
port. À relire si un `[[native.loop]]` de génération apparaît côté SDK.

---

## 6. Deux bugs de Java, portés en substance et pas à la lettre

Ce ne sont pas des manques d'API mais des écarts **volontaires**, chacun avec sa source :

- **La sonde lit la mauvaise colonne.** `getBaseColumn(mutable.getX(), mutable.getZ(), …)` est
  évalué **avant** `mutable.set(curChunkX << 4, currentY, curChunkZ << 4)`
  (`EmptyColonyStructure.java:166-167`) : chaque sonde après la première lit le coin du chunk
  précédent, et la première lit l'origine du monde `(0, 0)`. `terrain::cave_site` sonde le
  coin qu'elle examine. Reproduire la lecture périmée aurait rendu la règle intestable.
- **`start_height` est analysé et jamais lu.** Le codec le prend (`:36`), le constructeur le
  stocke (`:65`), et `findGenerationPoint` ne touche jamais le champ : sur la surface le `y`
  vient de `getFirstFreeHeight` + `.above()` (`:129-130`), dans une caverne de la sonde. Donc
  `above_bottom: 32` (onze fichiers) et `below_top: 120` (le néerlandais déformé) ne changent
  rien. Le port n'invente donc pas de résolution `HeightProvider` : il expose `start_y`, le
  `y` que le code utilise réellement.

---

## 8. La conversion vers `founding::Refusal` n'est pas écrite

**Ce qui manque.** Un `impl From<worldgen::Refusal> for founding::Refusal`, ou l'inverse.

**D'où ça vient.** `worldgen::Refusal` est le type **du module** (`src/worldgen/mod.rs`), avec ses
variantes propres et ses sources ; `founding::Refusal` appartient à `src/founding/mod.rs:57` et
aucun agent n'a le droit de l'étendre. Les deux se recouvrent partiellement — `TooCloseToColony`
existe des deux côtés, `SurfaceIsFluid` et `LandTooHigh` n'existent que côté `worldgen` — mais un
`From` imposerait de choisir quoi faire des variantes sans équivalent, et ce choix appartient à
l'appelant.

**Ce que ça coûte.** Un appelant qui veut un seul type d'erreur pour une fondation **et** une
génération écrit la correspondance lui-même ; le module ne la devine pas. `site::choose` rend
`Refusals`, qui porte les refus de `worldgen` tels quels.

**La sortie.** Un `From` dans le module qui possède la décision (l'appelant), pas ici.

---

## 7. Ce qui est disponible et n'a pas eu besoin d'être inventé

Pour mémoire, ces appels existent et suffisent à remplir les faits du module :

| Fait | Appel d'hôte |
| --- | --- |
| biome d'une position | `level_biome_at` (`host/levels.rs:76-82`) |
| appartenance à un tag de biome | `level_biome_in_tag` (`:145-158`), `#` accepté |
| bornes de hauteur du niveau | `level_height` (`:355-361`) → `(min_y, height)` |
| bloc d'une position | `level_block_state` (`:304-331`) |
| fluidité d'un bloc | `state_fluid_state` (`host/tags.rs:29-33`) |

Aucune constante de ce module n'est donc une supposition sur l'hôte : les seules constantes
sont celles de `EmptyColonyStructure.java` (`200`, `32`, `10`, `30`), reprises avec leur ligne.
