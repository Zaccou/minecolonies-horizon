# MANQUANT — ce que `src/ai_work_12` voulait faire et qui n'était pas possible

Écrit pour les trois buts de `core/entity/ai/workers/service/` : l'**enchanteur**
(`EntityAIWorkEnchanter`), le **guérisseur** (`EntityAIWorkHealer`) et le **croque-mort**
(`EntityAIWorkUndertaker`). Chaque entrée dit **ce que je voulais faire**, **ce qui existe
réellement** (avec la déclaration lue), **ce que j'ai fait à la place**, et **ce que ça
coûte**.

Rien ici n'est contourné en silence : le module ne compile que sur `core`, `crate::` et le
SDK, et il **n'importe pas `mod_api` du tout** — la même frontière que `src/ai`,
`src/ai_build` et `src/ai_gather`.

---

## 1. `Machine<S, F>` est monomorphe : les quatorze cibles de base ne tiennent pas dedans

**Voulu.** Que la machine d'un de ces trois métiers contienne **tout** : ses cibles, le
vidage d'inventaire, la demande d'objet, la pause.

**Ce qui existe.** `crate::ai::state::Machine<S, F>` est génériqué sur **un** état et **un**
type de scène (`src/ai/state.rs:516-540`), et
`crate::ai::worker::register_base` enregistre les quatorze cibles de
`AbstractEntityAIBasic.java:204-259` sur `Machine<WorkerState, Scene>` — un autre couple
`(S, F)` que `(ServiceState, ServiceScene)`.

**Fait à la place.** `register_handover` (`src/ai_work_12/mod.rs`) enregistre **cinq**
transitions de raccord, une par état de base que ces métiers traversent, chacune citant la
ligne de base qui la possède (`afterDump` `:1213-1222`, `afterRequestPickUp` `:736-739`,
`unpause` `:251`, `getStateAfterPickUp` `:323-326`, `idleToEntry` — la ligne du
constructeur du métier).

**Coût — du temps, pas de la justesse.** `setDelay(...)` **est enregistré** mais pas
**dépensé** : `waitingForSomething` (`AbstractEntityAIBasic.java:496-517`) est une cible de
la table de base. Un appelant qui ne fait tourner que cette machine voit l'enchanteur
repasser en `IDLE` tous les quatre ticks machine au lieu d'y attendre `pendingWork` ; c'est
à lui de faire aussi tourner `crate::ai::worker::JobAi`.

---

## 2. `WorldFacts::job` est une `JobFacts` fermée à deux variantes

**Voulu.** Mettre les faits de l'enchanteur, du guérisseur et du croque-mort dans
`crate::ai::facts::JobFacts`, à côté de `Composter` et `Builder`.

**Ce qui existe.** `JobFacts` a exactement trois variantes (`Composter`, `Builder`,
`Unported`, `src/ai/facts.rs:780-788`) et `src/ai/facts.rs` est **hors de la zone d'écriture
de ce module** (règle 1 : on n'écrit que dans `src/ai_work_12`).

**Fait à la place.** `facts::ServiceJob` porte les trois moitiés de métier et
`facts::ServiceScene` **enveloppe** `WorldFacts` au lieu de l'étendre. Rien n'est recopié :
`ServiceScene::facts` est la même valeur `WorldFacts`, donc un appelant qui fait tourner
`crate::ai::worker::JobAi` en même temps lui passe celle qu'il a déjà remplie.

**Coût.** Un appelant qui n'a qu'un `WorldFacts` sous la main doit construire la scène avec
`ServiceScene::new(facts, job)`. Le jour où `JobFacts` acceptera d'autres variantes, les
deux moitiés pourront se rejoindre sans réécrire une règle : `ServiceJob` n'est qu'un
emballage.

---

## 3. `Refusal` est fermée à vingt variantes

**Voulu.** Réutiliser `crate::ai::worker::Refusal` pour les quarante-cinq refus nommés de
ces trois métiers.

**Ce qui existe.** `Refusal` a vingt variantes (`src/ai/worker.rs:116-165`) et trois d'entre
elles seulement tombent juste ici — `ToolMissing`, `HutFull`, `Paused`. Ajouter les
quarante-deux autres demanderait d'écrire dans `src/ai/worker.rs`.

**Fait à la place.** `state::ServiceRefusal`, avec 45 variantes, chacune portant sa ligne
Java (`java()`) et sa clé de message (`key()`). Le recouvrement est nommé dans la
documentation de chaque variante plutôt que dupliqué par un `match` de conversion : les
lignes ne sont pas les mêmes (`checkForToolOrWeapon(shovel)`,
`EntityAIWorkUndertaker.java:180`, n'est pas `checkForToolOrWeapon(toolType)`,
`AbstractEntityAIBasic.java:966` — l'un parle d'une pelle, l'autre de l'outil du métier).

**Coût.** Un panneau qui affiche les deux familles de refus doit lire les deux énumérations.
`ServiceRefusal::key()` suit la même convention (`ai_work_12.refusal.*`) pour que le
rassemblement soit mécanique.

---

## 4. `WorkerState` n'a pas les états d'atelier, et `CraftState` les a déjà

**Voulu.** Que l'enchanteur ait ses trois états d'atelier (`QUERY_ITEMS`, `GET_RECIPE`,
`CRAFT`) dans son énumération d'états, puisqu'il hérite de `AbstractEntityAICrafting`
(`EntityAIWorkEnchanter.java:53`).

**Ce qui existe.** `crate::ai::state::WorkerState` nomme dix-huit des 127 états de
`AIWorkerState` (`src/ai/state.rs:121-160`) et **aucun** état d'atelier ;
`crate::ai_build::crafter::CraftState` les nomme tous les sept, avec leurs lignes
(`src/ai_build/crafter.rs:74-115`).

**Fait à la place.** Aucun état d'atelier n'est écrit ici. `decide` rend
`Effect::Crafted { state: CraftState }` : la décision dit **quel** état d'atelier vient, et
`crate::ai_build::crafter` le prend. `next_crafting_state` est **appelé**, pas recopié.
Deux des quatre réponses de `getNextCraftingState` sont des états que ce module possède
déjà — `IDLE` (`AbstractEntityAICrafting.java:302`) et `INVENTORY_FULL` (`:308`) — et sont
donc rendues comme états, comme Java les rend (`return craftState`,
`EntityAIWorkEnchanter.java:132`).

**Coût.** L'appelant doit faire tourner la machine d'atelier après celle-ci. C'est le même
coût qu'au §1, pour la même raison : deux tables, deux machines.

---

## 5. Le monde : ce que la décision ne fait pas, et qui le fait

Aucune de ces trois IA n'agit : elles **décident**. Les gestes sortent en
`ai_work_12::Effect`, que l'appelant exécute. Ce n'est pas un manque du SDK pour la plupart
d'entre eux — `living_heal` (`living.rs:452`), `mob_move_to` (`mobs.rs:293`) et
`set_block_property` (`block_states.rs:69`) existent — mais un choix de frontière identique
à celui de `src/ai` : une décision qui ne peut pas être testée sans le jeu n'est pas
testable du tout.

| Geste Java | Ce que le SDK offre | Ce que le module rend |
| --- | --- | --- |
| `citizen.heal(10)` (`EntityAIWorkHealer.java:335`), `player.heal(...)` (`:463`), `citizen.heal(...)` (`:499`), `:335` | `living_heal(living, amount) -> bool` (`living.rs:452`), `living_health` (`:102`) | `Effect::Treated`, `Effect::HealedPlayer`, `Effect::HealedRemote` |
| `walkToBuilding`, `walkToSafePos`, `walkToUnSafePos`, `walkWithProxy(pos, 3)` (`:192`, `:329`, `:470`) | `mob_move_to` / `mob_move_to_path` (`mobs.rs:293`, `:319`) : le chemin est celui du jeu, pas le graphe de nœuds de MineColonies | des **faits** (`at_grave`, `at_burial`, `at_target`, `at_hospital`) |
| `InventoryUtils.transferAllItemHandler` (`:219`), `transferXOfFirstSlot...` (`:369-373`) | les opérations d'inventaire de `host/items.rs` | `Effect::EmptiedGrave`, `Effect::GiveCure` |
| `worker.getInventoryCitizen().extractItem(bookSlot, 1, false)` (`:425`) | idem | `Effect::Drained { book_spent: true }` |
| `mineBlock(position)` (`:292`) | `block_states::set_block_property` seulement : on change une propriété, on ne retire pas un bloc | fait `dig_done`, puis `Effect::Dug` |
| `resurrectCivilianData(...)` (`:363-373`) | rien : réveiller un citoyen est une règle de `crate::citizens`, pas du SDK | `Effect::Resurrected` |
| `fullfillRecipeAndCopy(...)` (`EntityAIWorkEnchanter.java:271`) | les recettes et l'artisanat sont `crate::crafting` / `crate::ai_build::crafter` | `Effect::Enchanted` |
| `createRequestAsync(new Stack(...))` (`EntityAIWorkHealer.java:296`), `checkIfRequestForItemExistOrCreateAsync` (`EntityAIWorkEnchanter.java:164`) | le système de requêtes n'est pas exposé au SDK | `Effect::RequestCure`, `Effect::RequestItem` |
| `StandardInteraction(...)` (`EntityAIWorkEnchanter.java:150`, `EntityAIWorkHealer.java:197`) | rien pour les interactions de colonie | `Effect::Say { key }` |

**Coût.** Le module ne produit aucun effet de bord : il faut un appelant pour chacune des
lignes du tableau. C'est ce qui rend les 82 tests possibles sans monde.

---

## 6. Aucune source aléatoire : cinq tirages deviennent des faits

**Voulu.** Porter `worker.getRandom()` tel quel.

**Ce qui existe.** Aucune méthode de `host/` ne prend ni ne rend un `RandomSource`
(recherche `fn random`) ; `src/ai/MANQUANT.md` §1 a déjà mesuré la même absence pour
`dumpOneMoreSlot` et `getLootMultiplier`.

**Fait à la place.** Les cinq tirages de ces trois métiers sont des **faits fournis par
l'appelant**, chacun nommant l'appel qu'il remplace :

| Tirage Java | Champ |
| --- | --- |
| `random.nextInt(drainable.size())` (`EnchanterStationsModule.java:112`) | `EnchanterFacts::drain_draw` |
| `random.nextInt(workers.size())` (`EntityAIWorkEnchanter.java:337`) | `EnchanterFacts::worker_draw` |
| `nextInt(60 * 60)` (`EntityAIWorkHealer.java:515`) | `HealerFacts::cure_roll` |
| `random.nextDouble() <= TOTEM_BREAK_CHANCE` (`EntityAIWorkUndertaker.java:351`) | `UndertakerFacts::totem_roll` |
| `chance >= random.nextDouble()` (`:357`) | `UndertakerFacts::resurrect_roll` |

**Coût.** Une partie de la décision est déléguée. C'est aussi ce qui la rend **rejouable** :
sans ça, aucun test de `gatherAndDrain`, `decide` ni `tryResurrect` ne pourrait exister.

---

## 7. `Patient` et `GraveData` : deux objets de Java qui n'ont pas d'équivalent ici

**Voulu.** Réutiliser un `Patient` et un `GraveData` déjà portés.

**Ce qui existe.** Rien : `Patient.java` (les trois états d'un dossier d'hôpital) et
`GraveData` (le NBT d'un citoyen mort, `api/colony/GraveData.java`) ne sont ni dans
`crate::citizens` ni dans `crate::entity`.

**Fait à la place.** `facts::PatientState` (`NEW`, `REQUESTED`, `TREATED`,
`Patient.java:16-21`) et `facts::Patient` vivent ici, parce que c'est la seule IA qui les
lit. `GraveData` est réduit à un **booléen** (`UndertakerFacts::last_grave_data`) : la
décision ne lit jamais que « il y a une tombe à inhumer », et les noms du citoyen sont
l'affaire de `crate::citizens`.

**Coût.** Le jour où les dossiers de patients seront persistés, `PatientState` devra
déménager dans `crate::entity` (c'est un état sauvegardé, `Patient.java:84-93`). L'énumération
est `Copy` et sans dépendance, donc le déplacement sera un `pub use`.

---

## 8. Les effets de recherche sont des faits

**Voulu.** Lire `getResearchEffects().getEffectStrength(RESURRECT_CHANCE)`
(`EntityAIWorkUndertaker.java:392`) et `USE_TOTEM` (`:412`).

**Ce qui existe.** `crate::research` porte l'arbre et les effets ; il n'y a pas de
`ResearchManager` dans `crate::ai` et la décision ne peut pas en tenir un.

**Fait à la place.** Deux faits : `UndertakerFacts::resurrect_research: f64` et
`UndertakerFacts::use_totem_research: bool`. La règle qui les combine est portée
(`resurrect_chance`, `totem_bonus`), pas l'effet.

**Coût.** Un appelant doit lire l'effet avant de construire la scène. C'est la même
frontière que `ComposterFacts::primary_skill` et `BuildingFacts::max_equipment_level`.
