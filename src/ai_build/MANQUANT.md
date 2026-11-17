# MANQUANT — ce que `src/ai_build` voulait faire et que ce dépôt ne permet pas

Écrit par l'agent des buts du chantier et de l'artisanat. Chaque entrée dit **ce que je voulais
faire**, **ce qui existe réellement** (avec la ligne), **ce que j'ai fait à la place** et **ce que ça
coûte**. Le module n'importe pas `mod_api` du tout : il ne compile que sur `core` et `crate::`.

Les cinq fichiers sont compilés dès maintenant : `pub mod ai_build;` est déjà dans `src/lib.rs:30`.

---

## 1. Les états d'atelier n'existent pas dans `crate::ai::WorkerState` — **structurel**

**Voulu.** Un seul type d'état pour tous les métiers, pour que `crate::ai::Machine` serve partout.

**Ce qui existe.** `src/ai/state.rs:121-160` énumère **18** des 127 valeurs de `AIWorkerState` : celles
du builder et du composter. `CRAFT`, `QUERY_ITEMS`, `GET_RECIPE`, `PREPARE_DELIVERY`, `DELIVERY`,
`PICKUP` et `DUMPING` n'y sont pas. `src/ai/` appartient à un autre agent (règle 1), je n'y touche pas.

**Fait à la place.** `crafter::CraftState` (7 états) et `delivery::DeliveryState` (6 états), sur le
**même** moteur : `crate::ai::{Machine, Transition, Action}`. Les quatre passes, les compteurs de tick
et la règle du piège sont le code du builder, pas une copie.

**Coût.** Un état ne peut pas passer d'une machine à l'autre, et `JobAi::for_job` ne peut pas rendre
ces machines (voir §2). La duplication de code, elle, est nulle : seuls les *noms* sont neufs.

---

## 2. `JobAi::for_job` répond `Unported` pour l'artisanat et la livraison

**Voulu.** `JobAi::for_job(Job::Sawmill)` et `for_job(Job::Deliveryman)` rendant une machine qui tick.

**Ce qui existe.** `src/ai/worker.rs:583-596` : deux bras, `Job::Composter` et `Job::Builder`, et
`other => return Err(Unported { job: other })` pour les 48 autres.

**Fait à la place.** Deux constructeurs autonomes, `crafter::ai()` et `delivery::ai()`, utilisables
tels quels mais **non branchés** sur `JobAi`.

**Ce qui débloque, et c'est une ligne dans un fichier d'un autre agent** :

1. `src/ai/worker.rs` : deux bras de plus, `Job::Deliveryman => delivery::ai()`, et un bras couvrant
   les **19** IA concrètes qui descendent de `AbstractEntityAICrafting` (mesuré : 20 `extends`, dont
   `AbstractEntityAIRequestSmelter` qui est abstrait) — `Alchemist`, `Baker`, `Blacksmith`, `Chef`,
   `ConcreteMixer`, `Crusher`, `Dyer`, `Enchanter`, `Farmer`, `Fletcher`, `Glassblower`, `Lumberjack`,
   `Mechanic`, `Nether`, `Planter`, `Sawmill`, `Sifter`, `StoneSmeltery`, `Stonemason`.
   **Attention** : `Smelter` et `Cook` n'en font pas partie — ils descendent d'`AbstractEntityAIUsesFurnace`
   (`AbstractEntityAIUsesFurnace.java:50`), qui étend `AbstractEntityAISkill`, une autre branche que ni
   `src/ai` ni ce module ne portent.
2. `JobProfile` : les deux profils manquent. Un artisanat **dump après une action**
   (`AbstractEntityAICrafting.java:724-727`, `getActionsDoneUntilDumping() == 1`) et
   `isAfterDumpPickupAllowed` y vaut `currentRequest == null` (`:727-731`) ; le livreur n'a aucun dump
   d'atelier mais un `afterDump` qui vide son sac dans l'entrepôt (`EntityAIWorkDeliveryman.java:291-314`).

**Coût.** Tant que le branchement n'est pas fait, aucune IA de citoyen ne tourne sur ces deux tables :
elles sont testées, pas actives.

---

## 3. Écrire ou casser un bloc : aucune API dans le SDK

**Voulu.** Poser un bloc (`BLOCK_PLACEMENT`), en retirer un (`BLOCK_REMOVAL`), vider une couche d'eau
(`clearWaterStep`).

**Ce qui existe.** Mesuré dans `src/ai/MANQUANT.md` §2, et vérifié à nouveau ici : `host/block_states.rs`
n'offre que `set_block_property(pos, key, value) -> bool`. Ni `set_block`, ni `break_block`, ni
`place_block`.

**Fait à la place.** `order::decide` répond `Operation::{Place, Remove, ClearWater, SpawnEntity}` ou un
`Skip` nommé, et `work_time` répond une durée en ticks. L'appelant exécute.

**Coût.** La décision est complète, l'effet ne l'est pas ; c'est la même frontière que
`crate::ai::builder` a déjà choisie avec `Placement`.

---

## 4. Marcher : trois navigations, trois faits

**Voulu.** `walkToBuilding`, `walkToSafePos`, `walkToTaggedWorkPos`, `walkToConstructionSite`.

**Ce qui existe.** Ce sont des méthodes de `AbstractEntityAIBasic` / `AbstractEntityAIStructure` qui
appellent la navigation de l'entité ; aucune n'a d'équivalent dans `host/`.

**Fait à la place.** Des faits : `at_building`, `at_target`, `at_work_position`, `navigation_done`,
`idle_pos_reachable`, `at_warehouse`, `at_construction_site` (ce dernier déjà dans
`crate::ai::BuilderFacts`).

**Coût.** Le module ne peut pas distinguer « pas encore arrivé » de « chemin impossible » : la
distinction est dans `unreachable`, que l'appelant seul peut remplir (`isReachableFromLocation`,
`EntityAIWorkDeliveryman.java:346`, `:520`).

---

## 5. Transférer des items : un booléen là où Java a une boucle

**Voulu.** `InventoryUtils.forceItemStackToItemHandler(handler, stack, keepPredicate)`, puis le test
`ItemStack.matches(insertionResultStack, stack)` qui dit si le stack est revenu tel quel.

**Ce qui existe.** Rien dans `host/` ne touche un `IItemHandler`.

**Fait à la place.** `DeliveryFacts::extracted` (au moins un stack est sorti du sac, `:366`, `:390`) et
`DeliveryFacts::chest_refused_same_stack` (`:413-417`). Le test « le même stack est revenu » est le seul
cas où Java met `success = false` (`:413-441`) ; les autres refus de transfert laissent `success` vrai.

**Coût.** Le slot-par-slot de `deliver` (`:371-447`) se réduit à ces deux faits : le module ne peut pas
dire *quel* slot a été refusé.

---

## 6. L'identité d'un stack : deux moitiés au lieu d'une chaîne, et un hash

**Voulu.** La même clé que Java : `stack.getDescriptionId() + "-" + hashCode`
(`AbstractEntityAIStructureWithWorkOrder.java:349-350`), et le même test d'égalité qu'`ItemStorage`
(item + NBT + *capability*).

**Ce qui existe.** `crate::inventory` porte un `Inventory` et des piles, mais pas l'identité complète
d'un `ItemStorage` (composants de données, capabilité d'objet).

**Fait à la place.** `materials::NeedKey { description_id, tag_hash }`, avec `Display` qui rend
exactement la chaîne de Java (`:349`) et un `PartialEq` qui compare les deux moitiés. `tag_hash` est le
`hashCode` **du tag**, comme Java l'écrit — pas un hash de composants.

**Coût.** Deux stacks qui ne diffèrent que par un composant de données que le hash ignore fusionnent en
une seule entrée. Java aurait le même comportement pour son `hashCode`, mais `ItemStorage` les
distinguerait ; ce module ne le peut pas. Non mesuré sur le corpus.

---

## 7. Les tags de blueprint (`h-frame` / `v-frame`) ne sont pas lisibles ici

**Voulu.** `createFrameIteratorForBlueprint` (`AbstractEntityAIStructure.java:542-569`) : lire les tags
`TAG_H_FRAME` / `TAG_V_FRAME` et décider si le premier stage de **requête** (`SOLID_FRAME`,
`AbstractEntityAIStructureWithWorkOrder.java:264-278`) a quelque chose à demander.

**Ce qui existe.** Le parseur de blueprint est hors de ce dépôt (crate privé `Zaccou/port-blueprint`) et
`crate::structures` résout un nom de fichier vers une entrée d'index, sans exposer les tags.

**Fait à la place.** `materials::first_stage(level, has_frame_tags)`, où `has_frame_tags` est le fait
« la blueprint a au moins deux tags h-frame ou un v-frame ». C'est exactement la condition de Java
(`:563-566`), sans la lecture.

**Coût.** Un étage de requête peut être sauté si l'appelant se trompe sur ce fait ; aucun autre étage
n'en dépend.

---

## 8. `getNeededResources()` : un montant, pas un registre

**Voulu.** Lire `BuildingBuilderResource` — l'item, le montant, ce qui est déjà réservé, ce qui est
disponible.

**Ce qui existe.** Le registre des réservations vit dans `crate::requests` (et dans le bâtiment, côté
`crate::building`), et ce module n'a pas le droit d'y écrire.

**Fait à la place.** `materials::Need` porte le montant et trois disponibilités — `in_inventory`,
`in_bucket`, `carried` — plus les deux compteurs de requêtes. `reduced` dit quand une entrée disparaît
(le montant tombe à zéro), ce que fait `building.reduceNeededResource(stack, 1)` (`:473-477`).

**Coût.** La comptabilité (quel coffre, quelle réservation, quel montant déjà livré) reste à l'appelant ;
le module ne décide que du *fait* « il en manque ».

---

## 9. La table du builder n'est pas dans ce module, et c'est volontaire

**Voulu par le brief.** « Porte pour chacun une machine à états testable », builder compris.

**Ce qui existe déjà.** `src/ai/builder.rs` (1 091 lignes, 22 tests) : les neuf cibles de
`AbstractEntityAIStructure.java:166-200`, les cinq dérogations de `EntityAIStructureBuilder`, les six
sorties de `structureStep` (`step_outcome`) et les sept de `doMining`. `src/ai/worker.rs` porte la table
de base (`register_base`) et `JobAi::for_job(Job::Builder)` rend la machine complète.

**Fait à la place.** Rien : `src/ai_build/mod.rs` donne la carte des quatre occupations du chantier vers
ces tables, et ce module ajoute ce qui manquait vraiment — le temps par bloc, la résolution de la requête
de matériau, l'ordre de pose et le motif de chaque refus, puis les deux métiers absents.

**Si l'orchestrateur voulait les buts du chantier **dans** `ai_build`**, c'est un déménagement de
1 091 lignes et de leurs tests, pas une réécriture : à décider explicitement, pas à improviser. Le
recopier aurait donné deux tables à corriger à chaque fois.

---

## 10. Les tirages aléatoires de `idle` sont des faits

`AbstractEntityAICrafting.idle` tire trois fois : `MathUtils.RANDOM.nextBoolean()` (`:208`), encore lui
(`:231`) et `nextInt(size)` (`:216`, `:225`, `:238`, `:247`). Aucune source aléatoire dans `host/`
(mesuré dans `src/ai/MANQUANT.md` §1), donc `coin_a`, `coin_b` et `pick` sont des faits. `pick` est pris
**modulo la taille de la liste** : un fait hors bornes ne doit pas faire paniquer la machine (test
`an_out_of_range_pick_is_wrapped_not_panicked_on`).
