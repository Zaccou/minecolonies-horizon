# MANQUANT — ce que `src/ai_work_07` voulait faire et qui n'était pas possible

Écrit pour les trois buts de ce lot (mineur du Nether, apiculteur, composteur). Chaque
entrée dit **ce que je voulais faire**, **ce qui existe réellement** (avec la déclaration
lue), **ce que j'ai fait à la place**, et **ce que ça coûte**. Rien n'est contourné en
silence : le module ne compile que sur `core`, `crate::` et `crate::jobs`, il n'importe pas
`mod_api`.

Les déclarations citées sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé `4dc6455d`).

---

## 0. Rien à demander au SDK, et c'est le résultat, pas un oubli

**Voulu.** Décider si ce lot a besoin d'une surface d'API neuve.

**Ce qui existe.** Rien de ce module ne touche `mod_api` : ni les trois tables, ni les
faits, ni les refus. Le module est un ensemble de fonctions pures sur des données, comme
`crate::ai` et `crate::ai_gather` avant lui. Les six hooks du crate (`block_entity`,
`right_click_block`, `register_commands`, `run_mod_command`, `entity_join_level` et aucun
tick) sont déclarés dans `src/lib.rs` et **aucun** ne permet de faire tourner une IA de
métier ; c'est pour ça que la décision est séparée de l'action.

**Fait à la place.** Aucune API inventée, aucune méthode supposée. Ce fichier n'a donc
aucune entrée « il me manque un type » — les entrées ci-dessous sont des **frontières de
décision**, pas des trous d'API, et c'est écrit pour qu'on ne les confonde pas.

**Coût.** Le jour où un hook de tick par entité arrivera, l'appelant de ce module sera
`crate::loops` (voir `src/loops/`) et rien ici ne changera.

---

## 1. La moitié « métier » de `WorldFacts` est un enum fermé

**Voulu.** Poser les faits du mineur du Nether et de l'apiculteur **dans** `WorldFacts`, à
côté de ceux du composteur et du constructeur, pour qu'un seul type décrive toutes les
scènes.

**Ce qui existe.** `crate::ai::facts::JobFacts` a exactement deux variantes, `Composter` et
`Builder`, plus `Unported` (`src/ai/facts.rs:778-788`). Ce fichier est hors de la frontière
d'écriture de cet agent (règle 1).

**Fait à la place.** `WorkScene` **enveloppe** `WorldFacts` au lieu de l'étendre :

```rust
pub struct WorkScene { pub facts: WorldFacts, pub job: WorkJob, pub memory: WorkMemory, pub refusal: … }
```

`facts` est la **même valeur** que celle que `crate::ai` consomme : un appelant qui fait
tourner les deux tables remplit `WorldFacts` une fois et le passe aux deux. Rien n'est
recopié. C'est le même choix que `src/ai_gather/facts.rs`, et le même coût.

**Coût.** Deux types de scène dans le crate (`Scene`, `GatherScene`, `WorkScene`). Le jour où
l'on voudra une scène unique, il suffira d'ajouter deux variantes à `JobFacts` — mais c'est
une modification de `src/ai/facts.rs`, donc une décision d'orchestrateur.

---

## 2. Le vocabulaire de blocs ne connaît pas le portail

**Voulu.** Réutiliser `crate::ai::facts::Block` et `Blocks` tels quels.

**Ce qui existe.** `Block` (`src/ai/facts.rs:89-102`) répond à cinq questions : air /
fluide / solide / barrel / indestructible. Ce sont celles du constructeur et du composteur.
Le mineur du Nether demande **`world.getBlockState(portal).is(Blocks.NETHER_PORTAL)`**
(`EntityAIWorkNether.java:271`, `:341`, `:634`, `:677`, `:695`), et un portail n'est ni de
l'air, ni un fluide, ni un bloc ordinaire, ni un barrel, ni un indestructible.

Un enum à cinq valeurs ne peut pas le porter, et `src/ai/facts.rs` est hors frontière.

**Fait à la place.** `WorkBlock` (trois variantes : `Air`, `Portal`, `Solid`) et
`WorkBlocks`, la même forme de carte que `Blocks` (position non rapportée = air, position
marquée non chargée ≠ air, `is_loaded` avant toute règle qui s'en soucie).

**Coût.** Un troisième vocabulaire de blocs dans le crate. Les trois répondent à des
questions différentes et ne se recouvrent pas ; un enum unique obligerait à fusionner des
règles qui n'ont rien en commun.

---

## 3. La ruche n'est pas un bloc, c'est un bloc entité

**Voulu.** Lire la ruche avec le vocabulaire de blocs.

**Ce qui existe.** `getHiveToHarvest` (`EntityAIWorkBeekeeper.java:314-333`) demande
`state.is(BlockTags.BEEHIVES)` (`:319`) **et**
`BeehiveBlockEntity.getHoneyLevel(world.getBlockState(pos)) >= 5` (`:321`), et
`getBeesInHives` lit `BeehiveBlockEntity.getOccupantCount()` (`:152`). Un niveau de miel et
un nombre d'occupants ne sont pas des propriétés de bloc : ils vivent dans le bloc entité,
que ce module ne peut pas lire.

**Fait à la place.** `Hive { at, valid, honey_level, occupants }` : le *tag* devient le
booléen `valid`, le niveau et les occupants arrivent comme des nombres. La suppression des
ruches invalides (`:328`) est un **écrit** du propriétaire de la ruche, donc le port la
signale plutôt que de la faire ([`beekeeper::invalid_hives`]).

**Coût.** L'appelant doit relire les ruches de la hutte à chaque tick ; le port ne peut pas
lui-même constater qu'une ruche a cessé d'en être une.

---

## 4. La moitié `crafting` n'est pas compilée

**Voulu.** Appeler `super.decide()` et `checkForItems(...)` comme le fait Java
(`EntityAIWorkNether.java:201`, `:297`).

**Ce qui existe.** `AbstractEntityAICrafting.decide` (`:271-291`) et `.checkForItems`
(`:462-504`) vivent dans `core/entity/ai/workers/crafting/`, portés dans `src/crafting/`.
Or **`src/crafting` n'est pas déclaré dans `lib.rs`** : le commentaire au-dessus de la
ligne dit qu'une erreur de type y reste à trancher (`crafter.rs:503` / `register.rs:37`), et
le module est laissé hors compilation pour que le dépôt compile.

**Fait à la place.** Deux faits, et un refus nommé :

- `NetherFacts::crafter_busy` — `super.decide()` a répondu autre chose que `IDLE` ou
  `START_WORKING` (`:203`). Le port **nomme** cet état
  ([`WorkRefusal::CraftingHalfOwnsIt`]) au lieu d'inventer `CRAFT`/`GET_RECIPE`/
  `QUERY_ITEMS`, qui ne sont pas dans `WorkerState`.
- `NetherFacts::craft_check` — `checkForItems` répond exactement trois choses utiles ici
  (`GATHERING_REQUIRED_MATERIALS`, `GET_RECIPE`, `CRAFT`), et `decide` en distingue deux
  (`:299`, `:306`). Ces trois-là sont portées ([`CraftCheck`]) parce qu'elles décident.

**Coût.** `hasWorkToDo()` (`:147-150`, `super.hasWorkToDo() || building.isReadyForTrip()`)
n'est **pas** porté : sa première moitié appartient à la table de crafting. Un appelant qui
veut la règle complète la compose avec `crate::ai` une fois `src/crafting` déclaré.

---

## 5. Le combat du Nether a besoin d'une entité, d'une source de dégâts et d'une table de butin

**Voulu.** Porter la boucle de combat de `stayInNether` (`:409-474`).

**Ce qui existe.** Elle crée un `LivingEntity` (`:402`), frappe avec
`worker.hurt(source, incomingDamage)` (`:449`), lit `mob.getLootTable()` et le fait rouler
par `world.getServer().getLootData()` (`:495-496`), et tire **deux**
`worker.getRandom().nextBoolean()` par round (`:417-418`). Aucune de ces quatre choses
n'existe à ce niveau du port, et les deux tirages sont le genre de décision qui ne se
rejoue pas — donc qui ne se teste pas.

**Fait à la place.** Les quatre nombres que la boucle calcule :

| Règle | Java | Fonction |
| --- | --- | --- |
| dégâts encaissés, réduits par la compétence | `:406-407` | [`nether::incoming_damage`] |
| dégâts infligés, épée et enchantement | `:414-432` | [`nether::attack_damage`] |
| soin par round | `:955-980` | [`nether::check_heal`] |
| saturation que le soin coûte | `:460-465` | [`nether::heal_saturation_cost`] |
| xp d'un minerai | `:599-626` | [`nether::xp_on_drop`] |
| temps d'absence au vault | `:176` | [`nether::vault_travel_ticks`] |

et **la comptabilité des files** ([`nether::stay_in_nether`]), qui est la partie qui décide
de l'état suivant. Le butin d'un mob arrive dans le token comme un fait
(`AdventureToken::loot`), rempli par l'appelant après le combat — même frontière que le
reste du crate : le port décide, l'appelant agit.

**Coût.** Un token d'aventure sans `loot` rend zéro objet même si le mob en lâche ; c'est à
l'appelant de remplir.

---

## 6. Les écritures que le port ne fait pas, et qui sont nommées

Aucune n'est une API manquante : ce sont des écritures de monde, que ce crate réserve à ses
hooks. Elles sont listées pour qu'on ne les cherche pas dans le module.

| Java | Écriture | Où elle est dite |
| --- | --- | --- |
| `EntityAIWorkNether.java:347` | `building.recordTrip()` | doc de [`nether::leave_for_nether`] |
| `:333`, `:350`, `:476`, `:499`, `:572`, `:593` | `ExpeditionLog.setStatus/setCitizen/addMob/addLoot` | idem |
| `:195-197` | `setNextRespawnPosition` sur la cible de voyage | `NetherFacts::travel_target` |
| `:845-934` | `checkAndRequestArmor` : les demandes d'armure | `nether::ARMOR_TIERS`, `nether::gear_for` |
| `:940-948` | `needsCurrently` pour la nourriture | doc de [`nether::check_and_request_food`] |
| `:479-490` | `InventoryUtils.clearItemHandler` | doc de [`nether::stay_in_nether`] |
| `EntityAIWorkBeekeeper.java:208`, `:216`, `:238` | `triggerInteraction` (message au joueur) | `WorkRefusal::asks_the_player` |
| `:461-467` | `createRequestAsync(new StackList(flowers…))` | `Breeding::FlowersRequested` |
| `:166`, `:531` | `checkForToolOrWeapon` (demande d'outil) | `WorkRefusal::MissingTool` |
| `:364`, `:428` | `walkToWorkPos` | `Decision::stay()` tant qu'on n'est pas arrivé |

---

## 7. Les deux bugs de `null` de Java, non reproduits

**Constaté.** `returnFromNether` (`:634`) et `closePortal` (`:692-695`) font
`world.getBlockState(building.getPortalLocation())` sans vérifier que la hutte a un tag
`portal`. `BuildingNetherWorker.getPortalLocation()` (`:243-251`) répond `null` quand le
tag manque : une hutte enregistrée sans son portail fait donc lever une `NullPointerException`
au premier retour de voyage.

**Fait à la place.** Un portail absent se lit « pas de bloc de portail » et la machine
continue (`return_from_nether`, `close_portal`, et un test nommé). Une `unwrap()` sur une
donnée venue d'un fichier est interdite par la règle 5 ; ici c'est la même interdiction
appliquée à une donnée venue du monde.

**Coût.** Un écart assumé avec Java, documenté ici et dans la note du module.

---

## 8. Deux divisions qui tueraient en Java

| Java | Risque | Port |
| --- | --- | --- |
| `EntityAIWorkBeekeeper.java:202` | `99 / getSecondarySkillLevel()` : `ArithmeticException` sur une compétence à zéro, ce que `CitizenSkillHandler` permet (`:182`) | [`beekeeper::decide_delay`] plancher le diviseur à 1 |
| `EntityAIWorkNether.java:176` | `size() * 400` en `int` | [`nether::vault_travel_ticks`] sature au lieu de déborder |

Le second n'est pas un bug Java sur une partie normale ; c'est une `unwrap`-libre par
prudence, et c'est écrit pour qu'on ne croie pas à une règle.
