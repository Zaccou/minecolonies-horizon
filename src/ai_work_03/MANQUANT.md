# MANQUANT — `ai_work_03` (bûcheron, souffleur de verre, mécanicien)

Ce que ce module **décide** est complet et testé sans monde. Ce qui manque, c'est de
**faire** ce qu'il décide. Chaque point nomme la ligne Java qui l'exige et l'API du SDK
(`.horizon/sdk/api/crates/`) qui s'en approche le plus, quand il y en a une.

## 1. Le tick d'IA d'un citoyen

`AbstractEntityAIBasic.java:63-66` (`AbstractAISkeleton.tick`) est appelé depuis le tick
de l'entité. Dans le SDK, le seul point d'entrée est `entity_tick_pre` / `entity_tick_post`
(`crates/horizon-mod-vocab/src/hooks/`), qui sont **globaux** : un hook par tick pour
toutes les entités, filtré par un prédicat. Il n'existe pas d'enregistrement par entité.

Conséquence : la machine de ce module sait tiquer, mais rien dans le SDK ne dit *quand*
tiquer le citoyen qui la porte. C'est déjà le trou déclaré par `crate::jobs` (« a job can
be described, hired and levelled, but not *performed* »). Sans lui, `TradeAi::new` et
`TradeAi::tick` restent une bibliothèque.

## 2. Lire un four autrement qu'en NBT

`AbstractEntityAIRequestSmelter.java` lit `getItem(SMELTABLE_SLOT | FUEL_SLOT |
RESULT_SLOT)`, `isLit()` (`:536`, `:556`, `:659`) et
`getItem(RESULT_SLOT).getMaxStackSize()` (`:562`).

Le SDK donne `block_entities::block_entity(level, pos) -> Option<BlockEntityRef>` puis
`block_entity_data(entity) -> Option<Compound>` : le compound NBT complet
(`BlockEntity.saveWithoutMetadata`). Il n'y a **pas** d'accès typé à un
`FurnaceBlockEntity` — ni `isLit`, ni un slot par index. Deux façons de remplir
[`crate::ai_work_03::facts::Furnace`] :

- relire le NBT à chaque tick (`Items`, `BurnTime`, `CookTime`), ce qui coûte un
  `Compound` par four et par tick ;
- garder un cache par position tenu par un `BlockEntityPartState` du mod, que le NBT
  n'alimente qu'au chargement (`block_entity_on_load`).

`items::max_stack_size(item)` (`ItemsHost::max_stack_size`) donne le plafond du slot de
sortie, donc [`crate::ai_work_03::facts::FurnaceStack::max_stack`] est remplissable —
c'est le seul des trois faits qui ait une API dédiée.

## 3. Accélérer un four

`AbstractEntityAIRequestSmelter.accelerateFurnaces` (`:131-137`) appelle
`FurnaceBlockEntity.serverTick(...)` sur un four allumé, `(secondaire / 10) * 2` fois.

Le plus proche dans le SDK est `levels::level_block_tick(level, pos) -> bool`
(`host/levels.rs:999`). **Il n'est pas démontré que ce soit le tick du block entity et non
celui du bloc** — la différence compte, un four allumé avance sa cuisson par son block
entity. À mesurer avant de s'en servir ; en attendant,
[`crate::ai_work_03::smelter::accelerate_furnaces`] rend la liste des
`(position, ticks)` et l'appelant décide quoi en faire.

## 4. Transférer un item dans un four

Bonne nouvelle : c'est couvert. `items::container_insert(at: BlockSide, slot, resource,
amount, tx)` et `container_extract(at, slot, max, tx)` (`host/items.rs:410-433`) sont
exactement `InventoryUtils.transferXInItemHandlerIntoSlotInItemHandler` (`:413-418`) et
`transferItemStackIntoNextFreeSlotInItemHandler` (`:618`).

Ce qui manque n'est donc pas le transfert mais le **choix de la transaction** : Java ne
transige pas (`:413`), il écrit. Le port devra décider si un transfert de four est une
transaction atomique ou deux écritures — question de conception, pas de SDK.

## 5. Créer une requête (carburant, entrée)

`checkFurnaceFuel` (`:111`) crée une `StackList`, `fillUpFurnace` (`:355`) et
`addFuelToFurnace` (`:185`) posent un `needsCurrently`. Le SDK n'a aucune API de requête :
c'est `crate::requests` qui tient le registre, comme `crate::ai_build::materials` l'a déjà
constaté pour le bâtisseur. Les actions
[`TradeAction::RequestFuel`](src/ai_work_03/state.rs) et
[`TradeAction::RequestSmeltable`](src/ai_work_03/state.rs) portent le contenu exact
(liste, taille, minimum) ; les brancher est un travail de `crate::requests`.

## 6. Parler au joueur

`:444` et `:460` déclenchent une `StandardInteraction` (`BAKER_HAS_NO_FURNACES_MESSAGE`,
`FURNACE_USER_NO_FUEL`) en `ChatPriority.BLOCKING`. Aucune zone du SDK ne parle au
citoyen. Le port substitue des refus nommés
([`TradeRefusal::NoFurnaces`](src/ai_work_03/state.rs),
[`TradeRefusal::NoFuelAllowed`](src/ai_work_03/state.rs)) qui portent la même
information dans une forme qu'un menu ou un log peut lire.

## 7. Marcher

`:198`, `:241`, `:302`, `:367` — `walkToWorkPos(furnacePos)`. Aucune API de navigation
n'est exposée. Le fait [`SmelterFacts::at_furnace`](src/ai_work_03/facts.rs) est la
réponse de l'appelant, et [`TradeAction::WalkToFurnace`](src/ai_work_03/state.rs) est la
demande.

## 8. Ce qui n'est **pas** un manque, et qu'il ne faut pas rajouter

- **Le calcul des coups** : `crate::ai_build::work_time::craft_hits` existe, avec sa
  division entière et son zéro à partir du niveau 36. Ce module l'appelle.
- **La table de recettes** : `crate::crafting` porte le format `crafterrecipes/`.
- **Les deux compétences du module** (`primary`, `secondary`) : `crate::jobs::employment`
  les a déjà, et [`Trade::primary_skill`](src/ai_work_03/facts.rs) les relit au lieu de
  les recopier.
- **Le crafter lui-même** : `crate::ai_build::crafter` porte les six cibles de
  `AbstractEntityAICrafting`. Ce module ne les réimplémente pas ; il les appelle.
