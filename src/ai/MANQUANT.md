# MANQUANT — ce que ce module voulait faire et que le SDK ne permet pas

Écrit pour `src/ai` (sélection de buts). Chaque entrée dit **ce que je voulais faire**, **ce que le
SDK offre réellement** (avec la déclaration), **ce que j'ai fait à la place**, et **ce que ça coûte**.
Rien ici n'est contourné en silence : le module ne compile que sur `core` et `crate::`, il n'importe
pas `mod_api` du tout.

Les déclarations citées sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé `4dc6455d`, `Cargo.toml`).

---

## 1. Un tirage aléatoire — `worker.getRandom()`

**Voulu.** `AbstractEntityAIBasic.dumpOneMoreSlot` tire deux fois au hasard
(`AbstractEntityAIBasic.java:1260`, `:1264`) pour décider de jeter une pile quand l'inventaire est
presque plein, et `EntityAIWorkComposter.getLootMultiplier` tire `nextInt(100)` (`:362`).

**Ce que le SDK offre.** Aucune source aléatoire : aucune méthode ne prend ni ne rend un
`RandomSource` dans `host/` (recherche `fn random`, `RandomSource` : zéro résultat).

**Fait à la place.** Les deux tirages de `dumpOneMoreSlot` sont des **faits fournis par l'appelant** :
`WorkerFacts::coin` (`nextBoolean`) et `WorkerFacts::roll` (`nextInt(count)`), que le module compare
aux seuils Java (`16` puis `8`). `getLootMultiplier` prend `dice_result` en argument.

**Coût.** Une partie de la décision est déléguée. C'est aussi ce qui la rend **rejouable** : sans ça,
aucun test de `dumpOneMoreSlot` ne pourrait exister. Le tirage doit être fait par l'appelant, avec la
même distribution (0..count-1).

---

## 2. Poser ou casser un bloc — le pas de structure du builder

**Voulu.** `AbstractEntityAIStructure.structureStep` (`:334-535`) exécute un pas de placement, et
`doMining` (`:704-734`) casse un bloc.

**Ce que le SDK offre.** En lecture, `levels::level_block_state(level, pos) -> Option<BlockState>`
(`levels.rs:304`) et `levels::level_is_loaded(level, pos) -> bool` (`levels.rs:347`) : c'est
suffisant pour remplir `facts::Blocks`. En écriture, **seulement**
`block_states::set_block_property(pos, key, value) -> bool` (`block_states.rs:69`) et sa variante :
on peut changer une propriété d'un état existant, pas **poser un état** ni retirer un bloc. Il n'y a
ni `set_block`, ni `break_block`, ni `place_block` dans `host/`.

**Fait à la place.** `structureStep` est porté comme une **décision sur le résultat d'un pas** :
l'appelant exécute le pas (son propre placeur de blueprint) et rapporte un `facts::Placement`
(`Placed`, `Finished`, `Fail`, `LimitReached`, `MissingItems`, `BreakBlock(pos)`). Le module applique
la règle Java à cette réponse. De même, `mineBlock` devient `mining_done: bool` dans les faits.

**Coût.** Le module décide *quoi faire du résultat* mais ne peut pas produire le résultat. Le jour où
le SDK aura une écriture de bloc, `Placement` restera la bonne frontière : c'est le placeur qui sait
quel bloc va où, pas l'IA MineColonies.

---

## 3. L'inventaire du citoyen — 27 emplacements

**Voulu.** `worker.getInventoryCitizen()` : `isInventoryFull()` (`:412`), `getSlots()`,
`getStackInSlot(slotAt)` (`:1239-1250`), `openSlotCount(...)` (`:1255`),
`getItemCountInItemHandler(...)` (`:1683`), `hasSpace()` (`AbstractEntityAIStructure.java:346`).

**Ce que le SDK offre.** `players::player_inventory(player) -> Option<Vec<ItemStack>>`
(`players.rs:351`) et `player_inventory_slots` (`:360`) — **joueurs seulement**. Pour une entité
vivante : `living::living_equipment(living, slot)` (`living.rs:195`) et
`living::living_set_equipment(...)` (`:533`), c'est-à-dire l'équipement (main, armure), pas un
inventaire.

**Fait à la place.** L'inventaire du travailleur est un **fait** (`facts::InventoryFacts` : `full`,
`slots`, `open_slots`, `actions_done`, `compostable`, `dump: Vec<DumpSlot>`), rempli par l'appelant.
`DumpSlot { index, count, wanted }` porte exactement ce que `dumpOneMoreSlot` lit d'une pile : son
index, son nombre, et ce que la hutte en veut (`buildingRequiresCertainAmountOfItem`).

**Coût.** L'inventaire du citoyen doit vivre dans les données du mod (`crate::inventory`,
`crate::colony`), pas dans l'entité. C'est déjà le choix du port pour la colonie et la hutte.

---

## 4. Accélérer un baril — `barrel.tick()`

**Voulu.** `EntityAIWorkComposter.accelerateBarrels` (`:109-132`) appelle `barrel.tick()` jusqu'à
`(primarySkill / 10) * 2` fois par tick d'IA : un composteur expérimenté fait composter plus vite.

**Ce que le SDK offre.** `hooks/block_entity_tick.rs` et `block_entity_ticks.rs` : le mod reçoit le
tick de **ses propres** block entities. Aucune méthode ne demande au jeu de tiquer une block entity
existante.

**Fait à la place.** `composter::barrel_acceleration_ticks(primary_skill)` rend le nombre
(`0..18`, Java `:111`), et rien n'est appelé. Le baril doit être la block entity **du port** pour que
ce tick lui appartienne.

**Coût.** Nul si le baril est porté comme block entity Horizon (ce sera le cas : c'est un bloc
MineColonies) ; total si on espère accélérer un baril vanilla.

---

## 5. Le système de requêtes — `createRequest`, `createRequestAsync`

**Voulu.** `checkForToolOrWeapon` crée une `Tool` request (`:997-999`), `checkIfRequestForItemExistOrCreate`
crée une `Stack` request (`:1707-1715`), le composteur crée une `StackList` (`EntityAIWorkComposter.java:186-191`),
`dumpInventory` crée une requête de ramassage (`:1181`, `:1202`).

**Ce que le SDK offre.** Rien : la résolution de requêtes est un système MineColonies (colony, résolveurs,
livreurs), il n'a pas d'équivalent d'hôte et n'en aura pas — c'est du contenu, pas du moteur.

**Fait à la place.** Les requêtes sont des **faits** (`BuildingFacts::has_open_sync_request`,
`completed_requests`, `completed_sync`, `completed_async`, `completed_deliverable`,
`open_tool_requests`, `pickup_priority`, `pickup_has_entity`, `pickup_items_present`) et le module
**décide** de demander (`ToolCheck::Asked { created }`, `MaterialsOutcome::Asked`) sans créer la
requête. `crate::requests` possède déjà la file ; c'est lui qui la créera.

**Coût.** Un `asked` n'est pas une requête : l'appelant doit créer l'objet. C'est la même frontière que
`crate::requests` a déjà choisie.

---

## 6. Ce qui n'est *pas* un manque, et que je ne réclame pas

- **Marcher** : `mobs::mob_wanted_position(mob) -> Option<[f64; 3]>` (`mobs.rs:97`) et
  `mob_navigation_speed` (`:71`) existent. Le module ne marche pas (ce n'est pas une décision), mais un
  appelant peut le faire sans nouvelle API.
- **Lire le monde** : `level_block_state`, `level_is_loaded`, `block_entities::block_entity` +
  `block_entity_data` (pour l'état d'un baril) suffisent à remplir tous les faits. Vérifié : oui.
- **Parler au joueur** (les `triggerInteraction` de Java) : la zone `players` sait envoyer un message.
  Le module rend des `Refusal` nommées ; c'est l'appelant qui les affiche. Rien à demander.
- **Le tick du citoyen** : `hooks/entity_tick_pre.rs` existe (global). Un tick par citoyen n'est pas une
  API manquante mais un coût d'ordonnancement, déjà noté par `src/jobs/mod.rs`.

---

## 7. Ce que je n'ai pas pu vérifier

- **`cargo test` sur l'arbre entier** : à plusieurs reprises pendant la session, `src/jobs/kinds/mod.rs`
  puis `src/network/common.rs` (tests) étaient en cours d'écriture par d'autres agents et ne compilaient
  pas. Le module a donc été vérifié dans une crate isolée (`/tmp/ai-check` : `src/ai` + `src/jobs` +
  `src/citizens` + `src/building/kind.rs`, tous copiés verbatim) puis, quand l'arbre est revenu vert,
  par `cargo check --lib` dans le dépôt. Voir `RAPPORT.md` §7.
