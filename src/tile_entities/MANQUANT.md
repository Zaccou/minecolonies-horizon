# Ce qui manque aux block entities, et ce que je voulais en faire

Chaque ligne : **ce qui manque**, **où je l'ai rencontré**, et **ce que le code fait à la place**.
Aucune de ces lignes n'est un « pas eu le temps » : toutes sont une API absente du SDK, ou du code
vanilla qui n'est pas dans `minecolonies-src`.

L'inventaire des types disponibles est `.horizon/sdk/api/crates/` ; les numéros de ligne des fiches
`host/` cités ici sont ceux de cette copie du SDK.

---

## 1. Router le tick par block entity

**Manque :** le slot natif `block_entities.tick` (`src/loops/hut_tick.rs:15-24`) transmet un
**compteur**, pas la block entity touchée. Il appelle `hut::note_mod_tick(self.seen)` avec le nombre de
ticks vus depuis le chargement du monde.

**Ce que je voulais faire :** que chaque règle de ce module (`Barrel::advance_sample`,
`CompostedDirt::advance`, `Grave::on_colony_tick`, `BuildingEntity::steps`…) soit appelée pour **la**
block entity que le jeu vient de ticker.

**Ce que le code fait à la place :** chaque règle de tick est une fonction pure de son état et de ses
entrées (`ticks`, `draw`, `above_is_air`, `tick`), prête à être appelée par le slot. La boucle reste
unique, comme la consigne le demande : aucun fichier de `src/tile_entities/` n'ouvre de seconde boucle.

**Ce qu'il faudrait :** dans le slot, `tick.pos` (ou `tick.entity`) et la lecture du compound par
`block_entity_data`, puis un `match` sur `tick.kind` / le type de la block entity. Le SDK donne les deux
briques : `BlockEntitiesHost::block_entity(level, pos)`
(`crates/horizon-mod-vocab/src/host/block_entities.rs:11`) et `block_entity_data(entity)` (`:58`). Le
slot est hors de mon dossier (`src/loops/`), donc je ne l'ai pas modifié.

## 2. Le hook `block_entity_tick` n'est pas souscrit par la hutte

**Manque :** `hut::Hut` n'implémente pas `block_entity_tick`, le hook qui donne à une Part son tick avec
son compound (`mod_api_generated.rs:8624`). C'est pourtant la route naturelle, par entité, du point 1.

**Ce que je voulais faire :** y appeler la règle du type de la block entity, avec `event.nbt` en entrée
et en sortie.

**Ce que le code fait :** les règles existent et sont testées ; la souscription est une méthode à
ajouter dans `src/hut.rs`, que cet agent n'a pas le droit de toucher.

## 3. Les schémas : `StructurePacks` et `Blueprint`

**Manque :** `StructurePacks.getBlueprintFuture`, `getStructurePack`, `hasPack`, et la lecture d'un
`Blueprint` (`getPrimaryBlockOffset`, `getBlockInfoAsMap`, `setRotationMirror`). Aucune API de schéma
dans le SDK.

**Où :** `TileEntityColonyBuilding.reactivate` (`:681-717`), `processBlueprint` (`:719-757`) — le calcul
de rotation d'une hutte réactivée ; `TileEntityDecorationController.getRotation`
(`:259-266`) qui appelle `BuildingUtils.getRotationFromBlueprint`.

**Ce que le code fait :** il porte la **décision** (`BuildingEntity::steps` → `ApplyBlueprint`) et la
rotation comme donnée (`RotationMirror`, et le cache de `DecorationController::rotation`, qui prend le
calcul en `FnOnce`).

## 4. `BlueprintMapping` : style → pack + chemin

**Manque :** `BlueprintMapping.getStyleMapping` / `getPathMapping`, la table qui traduit un nom de
style en pack de structures et en chemin.

**Où :** `TileEntityColonyBuilding.load` (`:361-426`) s'en sert pour le chemin par défaut d'une hutte.

**Ce que le code fait :** `building::normalise_path` garde la seule règle qui ne dépend pas de la table
(pas de chemin vide, pas de `null`, extension `.blueprint`). Le défaut `builder1.blueprint` de Java reste
à calculer par l'appelant, qui a la table.

## 5. La recette de compost

**Manque :** `IColonyManager.getCompatibilityManager().getCopyOfCompostRecipes()`, et `CompostRecipe`
(`getStrength`, la durée de fermentation).

**Où :** `TileEntityBarrel.findCompostRecipe` (`:179-185`).

**Ce que le code fait :** `Barrel::feed(count, strength)` prend la force en paramètre. Un objet qui n'a
pas de recette n'est pas reconnu par ce module : c'est l'appelant qui consulte le registre.

## 6. Le registre des extensions de champ

**Manque :** `BuildingExtensionRegistries`, qui associe un tag de schéma (`field`, `water`) à une
extension de plantation (`IPlantationModule.getFieldTag`).

**Où :** `TileEntityPlantationField.getPlantationFieldTypes` (`:95-104`).

**Ce que le code fait :** `PositionedTags::tag_names` rend les **tags** ; la traduction en extensions
est dehors.

## 7. L'onglet créatif, pour « objet similaire »

**Manque :** `ICompatibilityManager.getCreativeTab(ItemStorage)`, l'onglet créatif d'un objet.

**Où :** `TileEntityRack.hasSimilarStack` (`:240-264`), utilisé par l'entrepôt pour choisir un rack
(`TileEntityWareHouse.getPositionOfChestWithSimilarItemStack`, `:217-240`).

**Ce que le code fait :** `warehouse::rack_for` saute l'étage « similaire » et va de « même objet » à
« rack le plus vide ». Sauter un étage change **où** va un objet, jamais s'il est rangé : le rack le plus
vide accepte toujours.

## 8. Dégâts et NBT dans l'identité d'un objet

**Manque :** un modèle d'objet avec dégâts et composants ; `ItemStorage(ignoreDamageValue, ignoreNBT)`.

**Où :** `TileEntityRack.getCount` (`:186-217`), `hasItemStack` (`:174-180`).

**Ce que le code fait :** l'identité est le nom (`src/inventory/stack.rs:97-101` dit pourquoi). Les
trois variantes de `getCount` répondent donc la même chose, et c'est un fait du port, pas un oubli.

## 9. `GraveData` : le citoyen mort

**Manque :** `api/colony/GraveData.java` — nom, position de mort, compétences, inventaire du citoyen.

**Où :** `TileEntityGrave` (`:51`, `:145-168`), clé `gravedata`.

**Ce que le code fait :** le tag `gravedata` n'est **ni lu ni écrit** ; il survit à une fusion
(`merge_data` fusionne, ne remplace pas). Un test le vérifie. Rien ne l'interprète, donc rien ne le
corrompt.

## 10. Les bannières

**Manque :** `BannerBlockEntity.createPatterns`, `DyeColor`, et les deux objets
`ModBlocks.blockColonyBanner` / `ModItems.flagBanner`.

**Où :** `TileEntityColonyFlag.getPatterns` (`:85-108`) et `getItemClient` / `getItemServer`
(`:112-142`).

**Ce que le code fait :** `ColonyFlag` porte les motifs comme données (`BannerLayer { pattern, color }`)
et la règle du cache ; la construction d'un `ItemStack` de bannière reste à l'appelant.

## 11. Le drapeau de la colonie vient d'ailleurs

**Manque :** `IColonyManager.getColonyView(...).getColonyFlag()` — le blason est une donnée de la
colonie, et `crate::colony::Colony` ne l'a pas.

**Où :** `TileEntityColonyFlag.getPatterns` (`:93-99`).

**Ce que le code fait :** `ColonyFlag::sync_from_colony(&[BannerLayer])` prend la liste en paramètre et
rend `true` quand elle a changé — exactement la condition du `setChanged()` de Java.

## 12. Le gestionnaire de connexions des panneaux

**Manque :** `colony.getConnectionManager().getNode(pos)` et `ColonyConnectionNode` (voisin précédent,
voisin suivant, colonie visée), plus `IColonyManager.getColonyByDimension`.

**Où :** `TileEntityColonySign.tick` (`:201-270`).

**Ce que le code fait :** `ColonySign::refresh(at, &Neighbourhood)` prend ces faits en paramètre
(`Neighbourhood`, `Node`, `Target`) et rend ce qui a changé (`Changed`). La règle de distance cumulée et
la rotation sont testées.

## 13. Le plus proche joueur, pour l'enchanter

**Manque :** une recherche de joueur (`Level.getNearestPlayer(x, y, z, 3.0, false)`). Le SDK a
`level_online_players`, `level_entities_in` et `level_all_entities`
(`crates/horizon-mod-vocab/src/host/levels.rs:296`, `:282`, `:262`) mais aucun « le plus proche de ».

**Où :** `TileEntityEnchanter.tick` (`:62-67`).

**Ce que le code fait :** `Book::tick(player_angle, rng)` reçoit l'angle. L'appelant peut le calculer
avec `level_entities_in` et `atan2`, au prix d'un tri qu'une API dédiée éviterait.

## 14. L'écriture sur un panneau

**Manque :** `SignBlockEntity.setText`, `SignText`, et le découpage de texte
(`Minecraft.getInstance().font.getSplitter().splitLines`).

**Où :** `TileEntityColonyBuilding.tick` (`:470-503`) écrit le nom et le niveau du bâtiment sur les
panneaux `building_sign`.

**Ce que le code fait :** `BuildingEntity::steps` dit **quand** (`RefreshSigns`, une fois par seconde) ;
le texte et son découpage sont dehors, et dépendent d'une police.

## 15. L'inventaire combiné d'un bâtiment

**Manque :** `CombinedItemHandler`, qui présente la hutte **et** tous ses racks comme un seul
`IItemHandler` (`TileEntityColonyBuilding.getCapability`, `:630-668`).

**Ce que le code fait :** `super::rack::Rack` décrit chaque étagère ; la composition en un seul
inventaire est un appelant qui lit les positions de `building.getContainers()`, comme
`warehouse::Shelf`.

## 16. La liste `inventory` d'une étagère n'est pas relue

**Fait du port, à dire :** Java sauve l'inventaire d'un rack slot par slot, avec un marqueur `empty`
(`TileEntityRack.saveAdditional`, `:440-462`). Dans Horizon, **le conteneur du block entity est la
source de vérité** (`Registrar::block_container`, `src/hut.rs:101-108`) : les slots vivent dans le jeu,
pas dans le compound. `Content::from_slots` remplace donc `updateContent`, et le tag `inventory` d'une
sauvegarde Java n'a pas de lecteur.

**Conséquence assumée :** aucun monde MineColonies ne se relit dans Horizon (moteur différent, blocs
différents) ; il n'y a donc pas de migration à écrire.

## 17. Rendu et ressources

**Manque, et hors périmètre par nature :** `MaterialTextureData` / `refreshTextureCache`
(`TileEntityRack`, `:600-760`) qui choisit les textures d'une étagère selon son contenu, et
`getModelData`. C'est du rendu de client ; `src/render/` est un autre module.

---

## Ce qui existe et n'est donc **pas** ici

Pour éviter de réclamer une API qui existe :

- **particules et sons** : `level_send_particles` et `level_play_sound`
  (`crates/horizon-mod-vocab/src/host/levels.rs:1219`, `:1201`) ;
- **lire un bloc** : `level_block_state` (`levels.rs:304`) et `level_block` (`:325`), ce qui suffit au
  test `AirBlock` de la terre compostée ;
- **écrire un bloc** : `world_edit::WorldEditServerHost::edit_blocks` (`host/world_edit.rs:17`), pour
  la fleur qui pousse et la tombe qui disparaît ;
- **marquer un bloc modifié** : `block_entity_set_changed`
  (`host/block_entities.rs:136`) — le `setChanged()` de Java ;
- **fusionner des données** : `block_entity_merge_data` (`:116`), qui est ce que ce module écrit.
