# MANQUANT — ce que les buts de `production/agriculture/` voudraient faire et qui n'est pas possible

Écrit pour les trois métiers de ce module (fermier, pêcheur, fleuriste). Chaque entrée dit
**ce que je voulais faire**, **ce qui existe réellement** (avec la déclaration lue), **ce
que j'ai fait à la place** et **ce que ça coûte**. Rien n'est contourné en silence : le
module n'importe pas `mod_api`, il ne compile que sur `core` et `crate::`.

---

## 1. La moitié « métier » de `WorldFacts` est un enum fermé

**Voulu.** Poser les faits du fleuriste **dans** `WorldFacts`, à côté de ceux du composter
et du builder, pour qu'un seul type décrive les trois scènes.

**Ce qui existe.** `crate::ai::facts::JobFacts` a deux variantes utiles, `Composter` et
`Builder`, et une troisième `Unported` (`src/ai/facts.rs:780-788`). Ce fichier est hors de
la frontière d'écriture de cet agent (règle 1).

**Fait à la place.** `AgriScene` est un enum de deux scènes : celle de `crate::ai_gather`
(qui enveloppe déjà `WorldFacts`, pour le fermier et le pêcheur) et `FloristScene` (qui
enveloppe le même `WorldFacts` plus `FloristFacts`). `AgriScene::world()` rend le bloc
commun, et c'est **la même valeur** que celle que `crate::ai` consomme.

**Coût.** Trois types de scène dans le port au lieu d'un. Le jour où `JobFacts` gagne trois
variantes, `AgriScene` peut disparaître — mais c'est une modification de `src/ai/facts.rs`,
donc une décision d'orchestrateur.

---

## 2. `TileEntityCompostedDirt` n'existe pas dans le port

**Voulu.** Composter une terre, c'est-à-dire appeler
`((TileEntityCompostedDirt) entity).compost(chance, flower)`
(`EntityAIWorkFlorist.java:236`) et laisser le bloc faire pousser une fleur de temps en
temps.

**Ce qui existe.** `src/blocks/` porte les 86 blocs de MineColonies avec leurs règles
d'état, de clic, de pose et de casse (voir `src/blocks/MANQUANT.md`), mais **aucun bloc à
entité** : pas de `TileEntityCompostedDirt`, donc ni l'état `isComposted()` (`:374`), ni le
compte à rebours de croissance, ni la liste `getPlantGround()` de la hutte. Le SDK sait
déclarer tout cela — `block_entity_type(...)` et `block_entity_part_with_state::<P>()`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/parts/block_entities.rs:141`, `:163`), plus
une boucle native `block_entities.tick` — mais c'est du **contenu** (un type, une part, un
tick), pas une décision, et cela sort de ce module.

**Fait à la place.** Les quatre observations du fleuriste sont des faits que l'appelant
fournit (`PlantGround` : `pos`, `loaded`, `is_composted_dirt`, `composted`,
`flower_above`), et le compostage sort comme un ordre
(`AgriWork::Compost { position, flower, chance }`) avec la chance calculée à la ligne de
Java (`florist::growth_chance`, `:236`).

**Coût.** Aucune fleur ne pousse aujourd'hui : ce module dit *quoi* composter, *avec quelle
chance* et *quelle fleur*, et rien ne l'exécute.

---

## 3. La liste de terres de `BuildingFlorist` n'est portée nulle part

**Voulu.** Lire `building.getPlantGround()` (`:154`, `:350`, `:367`) et
`building.getFlowerToGrow()` (`:231`), et appliquer `building.removePlantableGround(pos)`
(`:381`).

**Ce qui existe.** `src/building/` porte `kind`, `cost`, `instance`, `nbt`,
`requirements` et `behaviour` — pas de `BuildingExtensionsModule`, et rien qui ressemble à
la liste de terres d'une hutte de fleuriste. `src/jobs/kinds/bois.rs` ne porte que les
*besoins* du métier (outil, matériaux, surface), pas son état.

**Fait à la place.** `FloristFacts::plant_ground` est fourni par l'appelant, et
`removePlantableGround` revient comme `AgriWork::RemovePlantableGround { position }` : une
décision pure ne peut pas modifier la hutte, elle demande.

**Coût.** Le retrait est fait par l'appelant ; un appelant qui l'ignore verra la même terre
non compostable revenir à chaque `decide` (tous les 200 ticks).

---

## 4. Les interactions joueur n'ont pas d'API dans le vocabulaire

**Voulu.** Déclencher `StandardInteraction(NO_PLANT_GROUND_FLORIST)` (`:156`),
`NO_COMPOST` (`:197`) et `NO_FLOWERS_IN_CONFIG` (`:241`), qui sont des messages bloquants
pour le joueur, et `NO_FREE_FIELDS` (`EntityAIWorkFarmer.java:247`) côté fermier.

**Ce qui existe.** Rien de tel dans `horizon-mod-vocab/src/host/` : il n'y a pas de
vocabulaire d'interaction de citoyen. `src/commands/` porte des réponses de commande, ce
qui n'est pas la même chose (un message déclenché par une IA, une seule fois, avec une
priorité).

**Fait à la place.** `AgriRefusal` nomme chaque cas avec sa ligne Java et une clé de message
(`AgriRefusal::key`), et c'est l'appelant qui décide de l'afficher ou non. Un refus nommé
est testable ; une interaction ne l'est pas sans le monde.

**Coût.** Le joueur ne voit rien tant que personne ne consomme la clé.

---

## 5. Marcher, miner et transférer sont des réponses, pas des appels

**Voulu.** Appeler `walkToWorkPos(pos)` (`:223`, `:265`), `walkToSafePos`
(`EntityAIWorkFarmer.java:530`), `walkToBuilding()` (`:167`), `mineBlock(pos)` (`:272`) et
`shrinkItemCountInItemHandler` (`:234`), comme Java le fait, et lire le booléen.

**Ce qui existe.** Aucune de ces quatre primitives n'est dans ce module : le port n'a pas
de navigation (le pathfinding est `crate::pathfinding`, et il calcule des chemins, il ne
les suit pas), et la casse de bloc est un hook serveur instantané ou annulable
(`hooks/break_block.rs`), pas une action étalée sur `getBlockMiningTime` ticks.

**Fait à la place.** `GoalAnswers` — `at_target`, `at_building`, `mined`, `transferred`,
`tool_broken` — porte la réponse de l'action du tick précédent, et `florist::mining_time`
(`:336-340`) donne le nombre de ticks que l'appelant doit laisser passer.

**Coût.** L'appelant doit tenir le compte des ticks de minage. C'est assumé : c'est
précisément ce qui rend la décision rejouable, et c'est ce que les 53 tests exploitent.

---

## 6. Le tirage aléatoire du compost

**Voulu.** Reproduire `worker.getRandom().nextInt(200 - getPrimarySkillLevel()) < 0 ||
shrinkItemCountInItemHandler(...)` (`EntityAIWorkFlorist.java:234`) littéralement.

**Ce qui existe.** Un `nextInt(n)` de Java rend un entier dans `[0, n)` : **il ne peut pas
être négatif**, donc la première moitié de la condition est toujours fausse. La skill ne
change donc jamais rien au compostage — dans le Java lui-même.

**Fait à la place.** Rien n'est tiré : la condition effective est « il reste du compost »
(`florist::compost`). Ce n'est pas un raccourci de portage, c'est la seule branche
atteignable du Java, et le commentaire du code le dit à la ligne.

**Coût.** Aucun. La variable `primary_skill` reste un fait, pour que la ligne soit citée et
pour le jour où la condition changerait.

---

## 7. Ce que le port ne modélise pas, volontairement

- **L'état visible et les métadonnées de rendu** (`setVisibleStatus(GARDENING)` `:221`,
  `setRenderMetadata(RENDER_META_FLOWERS)` `:135-142`) : cosmétique, aucune décision n'en
  dépend.
- **Les statistiques** (`StatsUtil.trackStatByName(FLOWERS_PICKED, …)` `:279-282`) et
  l'expérience (`addExperience(XP_PER_FLOWER)` `:277`) : rendues comme un champ de
  `AgriWork::Harvest { experience }`, appliquées par l'appelant.
- **Le son** (`playNeedRodSound`, `playCaughtFishSound` côté pêcheur) : cosmétique.
- **La recherche d'eau et de terre** (`searchWater` `EntityAIWorkFisherman.java:383-396`,
  `PathJobFindWater`) : déjà portée comme faits et décisions par
  `crate::ai_gather::fisherman` (`find_water`, `set_random_water`).
