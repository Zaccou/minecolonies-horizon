# Ce qui manque à `src/ai_work_01` — les trous d'API, un par un

Chaque entrée dit **ce que je voulais faire**, **ce que j'ai fait à la place**, et
**ce qu'il faudrait** pour le finir. Aucune ligne de ce module n'appelle une API
absente : tout ce qui n'existe pas est un fait que l'appelant remplit.

## 1. Deux compteurs de délai, et pas un seul

`AbstractEntityAIBasic.delay` (`:121`) est un seul champ en Java. Ici il en existe
deux : `crate::ai::worker::Memory::delay`, que la table commune du crafter utilise
déjà, et `state::WorkMemory::delay`, que ce module ajoute parce qu'une `Scene` de
`ai_work_01` ne contient pas la `Scene` de `ai::worker` (`Machine<S, F>` est
monomorphe en `F`).

Conséquence pour l'appelant : un citoyen qui tique les deux tables doit recopier
`ai::worker::Memory::delay` dans `WorkMemory::delay` (et l'inverse) avant chaque
tick, sinon un `setDelay` posé par une table n'est pas vu par l'autre.

Ce qu'il faudrait : une seule `Scene` partagée, donc un `Machine` qui accepte deux
jeux de faits — c'est un changement de `crate::ai::state`, qui n'est pas à moi.

## 2. La liste de carburants n'existe pas dans `crate::requests`

`AbstractEntityAIRequestSmelter.java:111` demande une **`StackList`** (tous les
carburants autorisés, pas un item). `crate::requests::Request` porte un seul
`ItemName` : il sait faire la `Stack` de l'alchimiste (poudre de blaze,
`EntityAIWorkAlchemist.java:543`) mais pas la `StackList` du boulanger.

Ce que j'ai fait : `furnace::check_furnace_fuel` dit *quand* demander et *combien*
(`STACKSIZE * nombre de fours`), et `src/ai_work_01` nomme le fait
(`BakerFacts::open_fuel_request`). La création de la requête reste à `crate::requests`.

Ce qu'il faudrait : une variante de requête « une liste d'items acceptables ».

## 3. Les listes de la hutte (terre, feuilles, alambics, fours)

`building.getAllSoilPositions()` (`EntityAIWorkAlchemist.java:125`),
`getAllLeavePositions()` (`:243`), `getAllBrewingStandPositions()` (`:363`, `:446`),
`building.getModule(FURNACE).getFurnaces()` (`AbstractEntityAIRequestSmelter.java:124`)
sont de l'état de **module de bâtiment** MineColonies. Le port de `crate::building`
couvre les modules du chantier (`behaviour::modules`), pas ceux de l'alchimiste ni du
boulanger : il n'y a donc rien à interroger pour remplir `AlchemistFacts::soil`,
`leaves`, `stands` et `BakerFacts::furnaces`.

Ce que j'ai fait : les quatre listes sont des `Vec` de faits, remplies par l'appelant,
dans l'ordre où la hutte les rapporte (l'ordre **est** la règle : `:215` prend le
premier alambic fini, `:552` le premier four à sortir).

Ce qu'il faudrait : le port de `BuildingAlchemist` / `BuildingBaker` et de leurs
modules (`removeSoilPosition`, `removeLeafPosition`, `removeBrewingStand`,
`removeFromFurnaces` inclus — les quatre mutations que Java fait au passage).

## 4. `BrewTime` et `isLit()` : pas d'accès typé

Le SDK donne `block_entity_data` (un `Compound` opaque),
`block_entity_item_slot` et `block_entity_set_item_slot`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/block_entities.rs:58`, `:88`,
`:176`). Il n'y a **pas** de vue typée d'un alambic ni d'un four : lire
`BrewingStandBlockEntity.brewTime` (`EntityAIWorkAlchemist.java:454`) ou
`FurnaceBlockEntity.isLit()` (`AbstractEntityAIRequestSmelter.java:533`) demande de
décoder le compound à la main, sans schéma ni constante publiée.

Ce que j'ai fait : `facts::Stand::brew_time` et `facts::Furnace::lit` sont des faits.
Le décodeur qui les remplit appartient à la couche qui parle au monde.

Ce qu'il faudrait : soit deux accesseurs typés (`brewing_stand_brew_time`,
`furnace_lit`), soit la publication des clés NBT de ces deux block entities.

## 5. Les déplacements du métier ne sont pas des buts

`walkToWorkPos(fuelPos)` (`:606`), `walkToRandomPosWithin(worker, 10, …)` quand
l'alchimiste s'ennuie (`:327`) et `walkToBuilding()` (`:331`) sont des buts de
navigation MineColonies. `crate::pathfinding::plan` donne un chemin entre deux
positions, mais il n'existe pas d'équivalent de « la position de travail étiquetée de
la hutte » ni de « une position au hasard dans les coins de la hutte ».

Ce que j'ai fait : `AlchemistFacts::at_work_pos` dit si le citoyen y est déjà, et
`WorkMemory::walk_to` retient la cible ; `Decide::Wander` et `Decide::WalkIn` portent
l'intention, pas le déplacement.

Ce qu'il faudrait : le port des buts `walkToTaggedWorkPos` / `walkToRandomPosWithin`
(voir `AbstractEntityAICrafting.java:523`, qui n'est pas porté non plus).

## 6. Les effets de bord du métier

Trois choses que Java fait et que ce module ne peut pas faire, faute d'entité :

- `CitizenItemUtils.damageItemInHand(worker, MAIN_HAND, 1)` (`:293`) : user les
  cisailles d'un coup ;
- `worker.decreaseSaturationForContinuousAction()` (`:185`, `:205`, `:288`) : la
  saturation du citoyen ;
- `worker.getCitizenData().triggerInteraction(new StandardInteraction(…,
  BAKER_HAS_NO_FURNACES_MESSAGE))` (`:898`, `:1081`, `AbstractEntityAIRequestSmelter.java:
  444`) : la bulle de dialogue qui dit au joueur qu'il n'a pas de four.

Ce que j'ai fait : `WorkRefusal::NoBrewingStand` et `WorkRefusal::NoFurnace` nomment
la même situation avec leur clé de message (`ai_work.refusal.no_brewing_stand`), et
`WorkRefusal::NoShears` nomme la cisaille manquante.

Ce qu'il faudrait : la durabilité et la saturation dans la couche entité, et le
dialogue dans `crate::citizens`.

## 7. Le tirage aléatoire, assumé comme un fait

`worker.getRandom()` (`:132`, `:250`, `:286`, `:314`, `:319`) n'est pas rejouable :
`AlchemistFacts` porte les cinq tirages (`pick`, `nether_wart_roll`,
`mistletoe_roll`, `mistletoe_roll_forty`) et `BrewItems`/`Wart` les comparent comme
Java. Un tirage hors bornes est ramené (`% range`), jamais une panique.

Ce qu'il faudrait pour s'en passer : rien — c'est le prix d'une décision testable, la
même choix que `crate::ai::facts::WorkerFacts::coin`/`roll`.
