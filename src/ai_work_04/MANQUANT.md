# MANQUANT — ce que ce module voulait faire et que l'état du dépôt ne permet pas

Écrit pour `src/ai_work_04` (les buts du scieur, du cribleur et du fondeur). Chaque entrée dit **ce que
je voulais faire**, **ce qui existe réellement** (avec la déclaration), **ce que j'ai fait à la place**
et **ce que ça coûte**. Rien n'est contourné en silence : le module n'importe pas `mod_api` du tout, il
ne lit ni n'écrit le monde, il décide.

Les déclarations citées vers le SDK sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé, `Cargo.toml` du port).

---

## 1. Le demi-fait qui manque : `JobFacts` n'a pas de variante pour ces trois métiers

**Voulu.** Une seule machine à états par métier, comme Java : la table de base
(`AbstractEntityAIBasic.java:204-259`, quatorze cibles) **plus** la table du métier
(`AbstractEntityAIUsesFurnace.java:76-82` + `EntityAIWorkSmelter.java:69` pour le fondeur), les deux
sur le même `Scene`, avec les mêmes faits.

**Ce qui existe.** `crate::ai::facts::JobFacts` a trois variantes —
`Composter`, `Builder`, `Unported` (`src/ai/facts.rs`) — et `WorldFacts` est le seul type de faits que
`crate::ai::worker::{Scene, Machine}` acceptent. `crate::ai::worker::JobAi::for_job` répond
`Err(Unported { job })` pour `Job::Sawmill`, `Job::Sifter` et `Job::Smelter`.

**Fait à la place.** Le demi-métier vit dans son propre type de scène :
`ai_work_04::facts::Scene<F>` = `{ facts: F, memory: crate::ai::worker::Memory, refusal }`, avec
`F = SiftFacts` ou `SmelterFacts`. La mémoire de l'IA est **réutilisée** (`Memory`, pas recopiée), donc
`delay`, `walk_to` et `ticks_to_complain` sont les mêmes champs que ceux de la table de base.
`SiftState::base()` et `SmeltState::base()` donnent la correspondance des quatre états que les deux
tables partagent (`IDLE`, `START_WORKING`, `INVENTORY_FULL`, `GATHERING_REQUIRED_MATERIALS`).

**Ce que ça coûte.** Un hôte doit piloter **deux** machines et projeter ses faits deux fois : les
`WorldFacts` de la base et les faits du métier. Le correctif propre est une quatrième variante de
`JobFacts` (`Sifter(SiftFacts)`, `Smelter(SmelterFacts)`) et un `JobAi::for_job` qui enregistre aussi
la table du métier — donc une modification de `src/ai/facts.rs` et `src/ai/worker.rs`, hors du dossier
de cet agent. Les types sont écrits pour que ce raccord soit mécanique : rien dans `facts.rs` ne dépend
de `crate::ai::worker::Scene`, et les deux machines sont des `Machine<_, Scene<_>>` ordinaires.

---

## 2. Le tirage du cribleur — `worker.getRandom()`

**Voulu.** `worker.getRandom().nextInt(ONE_HUNDRED_PERCENT) < CHANCE_TO_DUMP_INV`
(`EntityAIWorkSifter.java:192`) : une chance sur cent de vider l'inventaire après une sortie.

**Ce qui existe.** Le SDK expose l'aléa **du niveau**, pas celui du citoyen :
`levels::level_random(level) -> Option<LegacyRandom>` (`levels.rs:606`, « vanilla's `Level.random` »)
avec `LevelRef::with_random` pour tirer et réécrire le flux. Aucune méthode ne prend un
`RandomSource` d'entité (recherche `fn .*random` dans `host/` : `level_random`,
`level_block_random_tick`, `level_set_random`, `item_resource_enchant_randomly`,
`living_random_teleport` — rien qui vise un citoyen).

**Fait à la place.** Le tirage est un **fait** : `SiftFacts::roll`, comparé au seuil Java. Même choix
que `crate::ai::facts::WorkerFacts::coin`/`roll` pour `dumpOneMoreSlot`.

**Ce que ça coûte.** L'appelant doit tirer avec la bonne distribution (`0..99`). C'est aussi ce qui rend
`Sift::Produced { dump }` testable : un tirage vivant rendrait la décision non rejouable, et un test de
la branche « quota atteint » (`:192`, premier terme) ne dirait rien de la branche « chance ».

---

## 3. Accélérer un four — le tick de l'entité de bloc

**Voulu.** `furnace.serverTick(world, pos, world.getBlockState(pos), furnace)`, appelé
`(primarySkill / 10) * 2` fois par seconde sur chaque four allumé
(`AbstractEntityAIUsesFurnace.java:295-301`).

**Ce qui existe.** `levels::level_block_tick(level, pos) -> bool` (`levels.rs:999`) exécute
`ServerLevel.tickBlock` : « the block's part first, else `BlockState.tick` » — c'est le **tick planifié
du bloc**, pas `FurnaceBlockEntity.serverTick`. Les entités de bloc sont lisibles et modifiables par
slot (`block_entities::block_entity_item_slot`, `block_entity_set_item_slot`), ce qui suffit pour les
faits, mais pas pour avancer une cuisson.

**Fait à la place.** `smelter::acceleration_ticks(primary_skill)` rend le **nombre** de ticks gagnés
(`(skill / 10) * 2`, `:285`) ; la règle est portée, l'effet est laissé à l'hôte. Même frontière que
`crate::ai::composter::barrel_acceleration_ticks` pour les barils.

**Ce que ça coûte.** Un fondeur porté sans tick d'entité de bloc cuit à la vitesse vanilla. Ce n'est pas
un choix esthétique : `level_block_tick` n'est pas `serverTick`, et l'écrire à sa place changerait le
comportement (le tick planifié du bloc four ne cuit rien).

---

## 4. Les interactions du joueur — `SIFTER_NO_MESH`, `FURNACE_USER_NO_ORE`

**Voulu.** `citizenData.triggerInteraction(new StandardInteraction(Component.translatable(
TranslationConstants.SIFTER_NO_MESH), ChatPriority.IMPORTANT))` (`EntityAIWorkSifter.java:143`) et les
deux messages bloquants du fondeur (`AbstractEntityAIUsesFurnace.java:197`, `:205` ;
`EntityAIWorkSmelter.java:224`).

**Ce qui existe.** Rien qui porte ce concept : recherche `interaction` / `dialogue` dans `host/` →
zéro résultat. Il y a de la parole de joueur (`players.rs`) et des sons
(`levels::level_play_sound`, `entities::entity_play_sound`), mais pas de « interaction de citoyen »
avec priorité et attente de réponse.

**Fait à la place.** Chaque message est un **refus nommé** : `Refusal::NoMesh` (avec son
`ai.work.refusal.no_mesh`), `NoFuelList`, `NoFurnace`, `NoOre`, et `Refusal::key()` est la clé que
l'hôte peut traduire. Le module ne décide pas *où* le message s'affiche.

**Ce que ça coûte.** Un joueur ne voit pas le message tant que l'hôte ne l'affiche pas ; en revanche le
délai de `NO_MESH_DELAY = 100` (`:144`, `:57`) est bien posé dans `Memory::delay`, donc le rythme Java
est respecté même sans interface.

---

## 5. Les écritures du monde — et pourquoi ce ne sont pas des manques

Trois écritures Java passent par l'hôte, et il vaut mieux le dire que les appeler des trous de SDK :

| Java | Ce que le SDK offre | Ce que le module rend |
| --- | --- | --- |
| `module.fullFillRecipe(...)` (`EntityAIWorkSmelter.java:101`) | `block_entities::block_entity_item_slot` / `_set_item_slot`, `items::*`, transactions `tx: &Transaction` | `BreakOres::{Broken, Failed}` lu dans `SmelterFacts::ore_fulfilled` |
| `createRequestAsync(...)` (`:210`, `:230`) | le système de requêtes est **une invention de MineColonies** (« request manager »), pas de vanilla | `Demand::{SmeltableOre, StackList, ComplainNoOre}` |
| `building.setCurrentDailyQuantity(...)` (`EntityAIWorkSifter.java:191`) | le quota d'un hut est une donnée MineColonies | `Sift::Produced` + `SiftFacts::daily_quantity`, mis à jour par l'action |

**Ce que ça coûte.** La décision est complète et testable, l'effet est le contrat de l'appelant. C'est la
même frontière que `crate::ai::facts::Placement` pour le placeur de blueprint.
