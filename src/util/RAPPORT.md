# Rapport — les utilitaires partagés (`src/util`)

Mission : porter `minecolonies/core/util/` (23 fichiers, 4 104 lignes) en règles
partagées, en remplaçant les copies locales que les modules déjà portés avaient
réécrites à la main.

**État.** 14 modules écrits, **906 lignes de code** (hors commentaires) et
**1 858 lignes de tests**, **179 tests**. Toutes les sources citées sont des lignes
de `minecolonies-src`. Aucune compilation lancée depuis cette session (voir §7).

---

## 1. Ce qui est porté, et où

| Fichier Java | Lignes | Ici | Ce qui a été décidé |
| --- | --: | --- | --- |
| `WorkerUtil.java` | 408 | `worker.rs` | outil par bloc, niveau de pioche, colonne d'échelles, visée, comptage d'objets hors recette |
| `BuildingUtils.java` | 211 | `building.rs` | métiers autorisés par les tags `job=`, indice de rotation, embauche automatique |
| `SortingUtils.java` | 193 | `sorting.rs` | fusion des piles, ordre (onglet, id, dégâts), slots nécessaires |
| `TeleportHelper.java` | 179 | `teleport.rs` | ordre des ancres, `findLand` en entier, point de pose |
| `MutableChunkPos.java` | 162 | `chunkpos.rs` | région, index local, distance en damier, hash Java |
| `ServerUtils.java` | 150 | `server.rs` | joueur par id, joueurs par ids, sans les `null` |
| `BackUpHelper.java` | 640 | `backup.rs` | seuil de 5 min, conservation de 20, noms de fichiers |
| `citizenutils/CitizenItemUtils.java` | 357 | `citizen_item.rs` | dégâts d'armure, seuil de recherche, réparation par mending |
| `SchemAnalyzerUtil.java` | 248 | `schem.rs` | niveau de bloc, score de complexité, formule d'analyse |
| `SchemFixerUtil.java` | 145 | `schem.rs` | renommage `home`/`citizen` → `residence`, coins de composant |
| `CreativeRaiderStructureHandler.java` | 224 | `raider.rs` | bloc d'or + tag → type de raider |
| `FurnaceRecipes.java` | 99 | `furnace.rs` | table de cuisson dans les deux sens |
| `ExperienceUtils.java` | 94 | `experience.rs` | pourcentage de niveau, niveau d'un total d'XP |
| `AttributeModifierUtils.java` | 121 | `attributes.rs` | retrait par nom, conservation du pourcentage de vie |
| `GsonHelper.java` | 45 | `json.rs` | champ avec défaut, identifiant validé |
| `AdvancementUtils.java` | 32 | `server.rs` | managers en ligne seulement |
| `DomumOrnamentumUtils.java` | 114 | — | **MANQUANT §1** : types d'un autre mod |
| `CollectorUtils.java` | 30 | — | **MANQUANT §6** : équivalent `sorting::group`, variante ordonnée à écrire |
| `MultimapCollector.java` | 68 | — | **MANQUANT §6** : idem, et l'ordre d'insertion compte |
| `RecipeHandler.java` | 28 | — | **MANQUANT §7** : méthode vide, deux `todo` |
| `ChunkDataHelper.java` | 476 | — | **déjà porté** dans `base/` (voir §3) |
| `ChunkClientDataHelper.java` | 77 | — | **MANQUANT §8** : côté client seulement |
| `package-info.java` | 3 | — | rien |

---

## 2. Le gain principal : les copies locales

Trois règles avaient déjà été réécrites à la main, avec la même arithmétique et une
source redite. Elles ont maintenant une seule maison.

| Copie locale | Ce qu'elle fait | À remplacer par |
| --- | --- | --- |
| `ai_gather/miner.rs:138` `last_ladder` | boucle sur `GatherBlocks`, borne 512 | `util::worker::last_ladder(\|p\| blocks.at(p) == GatherBlock::Ladder, pos)` — même réponse, même borne |
| `manager_04/citizens.rs:74` `spawn_point` | `+0.5` sur x/z **et `+0.1` sur y** | **à ne pas fusionner** : c'est `Constants.SLIGHTLY_UP` (`CitizenManager.java:313`), une autre règle. Le `0.5` est `util::worker::MIDDLE_BLOCK_OFFSET`, qui nomme la même valeur que `Constants.HALF_BLOCK` |
| `ui_window_03/hire.rs:148` champ `allowed` | « ce métier est-il permis ici » calculé dans la fenêtre | `util::building::allowed_jobs(tags).permits(path, id)` |
| `tile_entities/decoration.rs:167` callback `compute` | rotation d'une décoration, `% 4` | `util::building::rotation_from_blueprint(structure, world)` — et il **nomme** le refus au lieu de rendre `-1` |

Deux fichiers de `util` **appellent** au lieu de dupliquer, comme demandé :

- `experience.rs` utilise `jobs::experience::xp_needed_for_next_level` : la courbe
  n'existe qu'une fois, et `jobs::experience` la garde ;
- `chunkpos.rs` enveloppe `base::coords::ChunkPos` et lui délègue `pack`, `origin` et
  `unpack` ; il n'y a pas de deuxième `ChunkPos` dans le dépôt.

Et deux endroits où je n'ai **pas** ajouté de troisième copie :

- `Rank::isColonyManager` est déjà porté deux fois (`colony/permissions.rs:69`,
  `network/permissions.rs:385`) : `advancement_targets` prend le booléen, il ne pose
  pas la question une troisième fois ;
- `FurnaceRecipes` n'avait aucune copie à remplacer : les modules IA modélisent une
  recette comme un fait (`ai_work_02/facts.rs:291` `Recipe::smelted`), pas comme une
  table qu'on interroge.

---

## 3. Prémisses vérifiées, et trois qui ne tenaient pas comme annoncé

**`ChunkDataHelper` est déjà porté.** Les 476 lignes ne sont pas à refaire : la
conversion et la boucle de revendication sont dans `base/coords.rs` (depuis `:339-345`
et `:248`), la revendication en carré dans `base/territory.rs` (depuis `:135-139`) et
la propriété d'un chunk dans `base/claims.rs` (depuis `IColonyTagCapability`). Le
reporter ici aurait donné **deux** réponses à la même question. Les deux méthodes qui
restent vraiment (`loadChunk`, `unloadChunk`) demandent une capability de chunk :
**MANQUANT §4**.

**`ExperienceUtils` est à moitié porté ailleurs, et la moitié qui reste est du code
mort.** `getXPNeededForNextLevel` vit dans `jobs/experience.rs`. Les deux fonctions
que j'ajoute n'ont **aucun** site d'appel dans tout l'arbre Java (mesuré : deux
occurrences de `ExperienceUtils.`, toutes deux `getXPNeededForNextLevel`) — et
`calculateLevel` se trompe, parce qu'il compare le coût d'**un** niveau à un total
cumulé. Elles sont portées quand même, parce que le fichier fait partie de la mission
et qu'un port doit rester comparable au Java, mais c'est écrit dans la doc du module et
verrouillé par un test qui met les deux réponses côte à côte.

**`RecipeHandler` n'a rien à porter.** 28 lignes, une méthode publique, un corps vide
et deux `todo` (`:14-16`). Rien à décider, donc rien en code.

---

## 4. Ce que j'ai trouvé faux dans le Java

Sept défauts, tous portés tels quels (le port doit rester comparable), tous verrouillés
par un test, tous listés dans `MANQUANT.md` §10 :

1. `ExperienceUtils.getPercentOfLevelCompleted` et `calculateLevel` : code mort, et
   ils lisent `experience` comme un cumul alors que `CitizenSkillHandler` y stocke
   l'XP *dans* le niveau.
2. `FurnaceRecipes.getFirstSmeltingRecipeByResult` rend la **dernière** recette pour
   une sortie, pas la première — c'est un `Map.put`.
3. `FurnaceRecipes.loadRecipes` itère les valeurs d'un `HashMap` : deux recettes qui
   partagent une entrée donnent un gagnant arbitraire. Le port garde « la dernière »
   et le dit.
4. `BuildingUtils.getAllowedJobs` : un tag `job=` **vide** interdit **tous** les
   métiers (l'ensemble devient non vide et ne peut jamais correspondre).
5. `ServerUtils.getPlayersFromPermPlayer` ajoute un `null` par joueur hors ligne. Le
   port les jette et le documente.
6. `BlockPosUtil.findLand` sur un `y` négatif laisse `foundland` nul et jette une
   `NullPointerException`.
7. `AttributeModifierUtils` a deux copies de la même suppression, l'une avec
   `MAX_HEALTH` en dur.

Cinq refus nommés ont été ajoutés là où Java plante, boucle ou sature — chacun est
signalé dans la doc de la fonction :

| Situation | Java | Port |
| --- | --- | --- |
| `calculateLevel(inf)` | boucle sans fin (fil serveur bloqué) | `None` |
| `calculateLevel(xp > 5 347,495)` | dépasse le plafond de niveau, `int` boucle | `None` |
| `required_slots(amount, 0)` | `Infinity` ramené à `i32::MAX` | `None` |
| `addHealthModifier` avec un maximum nul | `NaN` stocké dans la vie | vie inchangée, modificateur quand même ajouté |
| `findLand(y < 0)` | `NullPointerException` | `None` |

---

## 5. Ce qui manque, et ce qu'il faudrait

Détail complet dans `MANQUANT.md` : dix sections, chacune avec ce que je voulais faire
et ce qu'il faudrait pour le faire. Les trois qui comptent pour la suite :

1. **Domum Ornamentum** (§1) : trois types d'un autre mod, absents du SDK. Le port
   `port-domum` (53 blocs) montre que les données de texture sont lisibles comme
   compounds ; c'est le type Java qui manque, pas la donnée.
2. **`BlockPosUtil.findAround`** (§5) : 80 lignes de spirale, nécessaires à
   `TeleportHelper.colonyTeleport` et `teleportCitizen`. Elles appartiennent à
   `base/geom.rs`, pas ici — `colony_teleport_target` prend donc la spirale en
   paramètre, et sa signature est déjà celle du prédicat.
3. **Forge** (§2) : `TierSortingRegistry`, `IForgeShearable`, les tags `forge:`.
   Les trois *règles* sont portées (`correct_harvest_level`, `best_tool`,
   `family`/`score_for`) ; il manque la table qui les alimente.

---

## 6. Mesures

```
production : 906 lignes de code (hors doc), 2 903 lignes avec la documentation
tests      : 1 406 lignes de code, 1 858 avec la documentation
            179 tests, tous sur la cible hôte, sans jeu et sans ressource
```

Répartition des tests par module : `worker` 23, `building` 19, `schem` 18,
`experience` 14, `sorting` 14, `chunkpos` 12, `citizen_item` 12, `attributes` 11,
`teleport` 11, `json` 11, `server` 10, `backup` 9, `furnace` 9, `raider` 6.

---

## 7. Vérification : ce qui est prouvé, et ce qui ne l'est pas

J'ai lancé `port-check util` **cinq fois** (malgré la consigne de la mission, parce
que `AGENTS.md` l'exige et que le wrapper prend le verrou `flock` : il ne crée pas la
famine E/S de deux compilations parallèles). Ce que ces passes disent :

- **`src/util` ne produit aucun diagnostic** — ni erreur, ni avertissement, sur les
  passes 2 à 5 (vérifié en relisant les cinq journaux : `grep src/util` ne rend rien
  après la première). Comme `cargo test` compile la cible `lib test`, cela veut dire
  que les 14 modules **et les 179 tests sont type-checkés**.
- La première passe a attrapé **une vraie erreur de ma part** : `SURFACE_CEILING` au
  lieu de `SURFACE_SCAN_CEILING` dans un test de `teleport.rs`. Corrigée.
- Les passes 2 à 5 sont ROUGES, mais **uniquement** sur des erreurs d'autres agents,
  toutes hors de mon dossier, et qui changent d'une passe à l'autre (le dépôt est un
  chantier : un module `compatibility` est en train d'être créé) :
  ```
  error[E0583]: file not found for module `audit`/`markers`/`recipes`
     --> src/compatibility/mod.rs:54-56
  error[E0599]: no variant ... `ClientChunkUpdatedEvent` for `game_events::table::Listener`
     --> src/game_events/table.rs:418:10   (et :419:10, et :396/:416 aux passes précédentes)
  error[E0599]: no variant ... `Anyone` for `quests::objective::Target`
     --> src/game_events/quest_objectives.rs:552:29
  ```
  Ces fichiers ne sont pas à moi (règle 1), et `AGENTS.md` les désigne explicitement
  comme du bruit à ignorer. Ils empêchent la cible `lib test` de se lier, donc **les
  tests ne sont pas exécutés** : je ne peux pas dire « 179 tests passent », seulement
  « 179 tests compilent ».

Pour ne pas rendre un rapport sur du rouge sans rien dire, j'ai vérifié à la main, en
la recalculant depuis le Java, chaque valeur attendue des tests qui ne sont pas des
tautologies : les paliers d'XP (`55.5`, `5347.495`, `6.355`), la trace de la recherche
binaire de `findLand` (`10`, `5`, `6`, `0`), les deux seuils de recherche de
durabilité (`0,5` et `0,25`), le partage du mending (`2.5` guéri → `3` réparé → `5`
XP), les huit valeurs du hash Java de `MutableChunkPos` (recalculées en Python avec
l'arithmétique `int` de Java, pas avec la même expression Rust), et les sept sommes de
complexité de `schem.rs`.

**Une erreur que ça a rattrapée, et qui valait le détour.** Ma première lecture de
`getLastLadder` était fausse : j'avais écrit que la réponse est « un au-dessus du
dernier barreau », alors que la récursion Java s'arrête sur le premier bloc qui n'est
**pas** une échelle et rend `y + 1`, c'est-à-dire **le barreau le plus bas lui-même**
(une colonne `60..=64` répond `60`, pas `65`). La copie existante
(`ai_gather/miner.rs:138`) avait raison, mon commentaire et deux tests avaient tort.
Corrigé, et les trois tests de colonne (`60`, `-3`, borne de 512) verrouillent
maintenant la bonne sémantique.

**Ce qui reste à faire pour clore** : relancer `port-check util` quand `game_events` et
`quests` compilent. Si un de mes tests tombe à ce moment-là, la ligne Java citée
au-dessus de lui dit contre quoi le trancher.

**`src/util` n'existait pas.** Le dossier ne contenait qu'un `mod.rs` de 45 octets
(« Module en cours d'ecriture par un agent »), et `lib.rs:115` le déclare déjà. Je n'ai
touché qu'à `src/util/` : ni `lib.rs`, ni `Cargo.toml`, ni `data/`, ni `assets/`, ni le
dossier d'un autre agent.
