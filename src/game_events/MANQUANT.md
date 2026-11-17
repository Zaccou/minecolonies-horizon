# MANQUANT — les écoutateurs d'événements

Ce que la couche événementielle de MineColonies fait et que **l'API Horizon ne peut pas
porter aujourd'hui**. Chaque entrée donne la primitive exacte qui manque, la branche Java
concernée, et ce que coûte l'absence.

Rien ici n'est imité : une règle appliquée avec une valeur inventée serait un règlement qui
a l'air fini et qui ne l'est pas. Les **17** lignes concernées sont marquées `Missing` dans
`table.rs` et `counts().missing` les compte ; les **35** hooks distincts cités par les 40
autres lignes `Hook` sont listés dans `table::HOOKS` et vérifiés par
`table::hook_exists` ; la ligne `Superseded` est expliquée en fin de document.

## 1. Il n'y a pas de moment « on attache une capacité »

**Ce qui manque :** un hook équivalent à `AttachCapabilitiesEvent<LevelChunk>` et
`AttachCapabilitiesEvent<Level>` : le moment où le jeu construit l'objet et demande aux
mods ce qu'ils veulent y accrocher.

**Ce que le port a déjà :** `AttachmentType` **accepte** un chunk et un level comme
porteurs (`entity/attachment_holder.rs:15-26` : `Chunk(ChunkRef)`, `Level(LevelRef)`), donc
le stockage est là. Un mod déclare son type et l'écrit depuis `chunk_load` / `level_load`.
`src/network/common.rs` porte déjà la forme qui traverse (`ChunkCap`).

**Ce qui ne marche pas :** la valeur ne peut pas exister **avant** le chargement du chunk,
et rien n'annonce « ce level vient d'être construit, donne-lui sa capacité ». Les trois
fichiers de `capabilityproviders/` n'ont donc pas de remplaçant exact — seulement un
remplaçant « à peu près », ce que le port refuse (`capability_providers.rs`).

**Conséquence mesurée :** `minecolonies:colonymanager` porte le drapeau `overworld`
(`MinecoloniesWorldColonyManagerCapabilityProvider.java:37`, construit avec
`EventHandler.java:257`), et `Storage.writeNBT` écrit **différemment** selon ce drapeau. Un
`AttachmentType` n'a pas de variante par dimension : il faut deux types ou un champ, et
c'est un choix que le port ne peut pas faire sans l'API.

## 2. Un hook qui remet le mauvais contexte : `farmland_trample`

**Branche :** `EventHandler.java:805-816` — un colon `JobFarmer` dont la colonie a la
recherche `SOFT_SHOES` à plus de 0 ne piétine pas la farmland.

**Ce qui manque :** le hook est déclaré `context = "loop"` (`hooks/farmland_trample.rs:9-17`)
et reçoit un `LoopContext`, qui ne donne que les registres, les réglages, la liste des mods
et le côté (`world/phase.rs:813-832`). Pas de monde, pas d'entité, pas de colonie — alors
que la règle a besoin du **métier du colon** et de la **force d'un effet de recherche**.
L'`entity: u32` du champ ne sert à rien sans un `Host` pour le résoudre.

**Primitive exacte :** `farmland_trample` avec un `Host` en lecture seule, ou au minimum
`job_of(entity)` + `research_strength(colony, effect)` dans le contexte.

## 3. Pas de coupure d'un son au démarrage

**Branche :** `ClientEventHandler.java:142-158` — `citizenVoices == false` et un son
`minecolonies:citizen.*` : `event.setSound(null)`.

**Ce qui manque :** `sound_play` existe dans `hooks/sound_play.rs` comme **notes amont**
(`trait Notes`), pas comme hook déclaré : `grep -c "fn sound_play("
generated/mod_api_generated.rs` → 0. `muffle_sounds` (`hooks/muffle_sounds.rs`) baisse
**tous** les sons avec un low-pass, ce n'est pas une annulation ciblée.

**Primitive exacte :** un hook `sound_play` (client, contrat `cancel`, au moins
`sound: &str`) : `Flow::Cancel` = pas de son.

## 4. Pas de lecture de tag de type d'entité

**Branche :** `EventHandler.java:140` (`ModTags.mobAttackBlacklist`, qui contient
`minecraft:enderman` et `minecraft:llama`, `DefaultEntityTypeTagsProvider.java:32`).

**Ce qui manque :** `TagsHost` n'expose que les tags d'item, de fluide et de bloc
(`host/tags.rs:19-65`, `world/tags.rs:24-64`). `EntityRef::tags()` existe
(`host/entities.rs:113`) mais c'est `Entity.getTags` : les étiquettes `/tag` posées sur
**une** entité, pas le tag de registre de son **type**.

**Primitive exacte :** `entity_type_in_tag(kind: &str, tag: &str) -> Option<bool>`, ou une
`Tags::entity_types`. Sans elle la moitié de la règle « les mobs attaquent les colons »
tient et l'autre non — le port ne peut pas la porter à moitié.

## 5. Pas de prédicat dans un but de ciblage

**Branche :** `EventHandler.java:144` — `new NearestAttackableTargetGoal<>(mob,
EntityCitizen.class, true, citizen -> !citizen.isInvisible())`.

**Ce qui manque :** `GoalSpec::NearestTarget { prey: MobFilter, must_see, must_reach,
interval }` (`entity/types.rs:519-531`) : `prey` est un `MobFilter` (`Player` ou
`Species(Vec<String>)`), il n'y a pas de test. `mob_add_target_goal` accepte donc le but
sans le filtre.

**Primitive exacte :** un champ `filter` dans `GoalSpec::NearestTarget`, avec au moins
`not_invisible: bool` — sinon la règle portée vise aussi les colons invisibles, ce que le
Java refuse.

## 6. Pas de franchissement de chunk par entité

**Branche :** `EventHandler.java:517-543` — `onEnteringChunkEntity` : un colon garde qui
entre dans le chunk d'une **autre** colonie est ajouté à ses attaquants, avec la cible
`getPlayerToFollowOrRally()`.

**Ce qui manque :** aucun hook ne dit « cette entité a changé de chunk ».
`entity_entering_section` existe mais c'est une **section** (16³ blocs), pas un chunk, et
il ne porte pas la réclamation de colonie ; `chunk_watch` parle des joueurs qui regardent
un chunk, pas d'un mob qui y marche.

**Primitive exacte :** `entity_chunk_changed(dimension, entity, from: [i32; 2], to:
[i32; 2])`, côté serveur, avec le `Host` de lecture.

## 7. Pas d'horloge murale

**Branche :** `EventHandler.java:770-783` — Halloween (31/10, 1-2/11) et 1er avril lus par
`LocalDateTime.now()`.

**Ce qui manque :** l'API donne le temps **du jeu** (`level_game_time`, `level_day_time`,
`host/levels.rs:398-405`), pas la date de la machine. C'est volontaire : un mod qui lirait
`SystemTime` casserait la reproductibilité d'un tick rejoué.

**Primitive exacte :** aucune n'est souhaitable. Le port doit choisir : soit un réglage
serveur (`holiday_mode = halloween|april|off`), soit rien. **Décision à prendre par
Simon**, pas par l'agent.

## 8. Pas de renderer d'overlay en espace monde

**Branches :** `ClientEventHandler.java:93-97` (`WorldEventContext.renderWorldLastEvent` :
bordure de colonie, plans, chemins) et `:99-106` (le cache « colonie proche » qui
l'alimente).

**Ce qui manque :** `render_level_stage` existe (`hooks/render_level_stage.rs`), mais ce
n'est pas là que le port pourrait brancher `WorldEventContext` : c'est une classe de rendu
de plusieurs milliers de lignes (`core/client/render/worldevent/`) qui suppose les buffers
de lignes de MineColonies. Le hook existe, la chose à brancher n'existe pas.

**Primitive exacte :** aucune nouvelle primitive ; c'est un port de rendu (module
`src/render/`), pas un trou d'API. Consigné ici pour que la ligne `Missing` ne soit pas lue
comme « l'API a un manque ».

## 9. Pas de rejointe d'entité côté client

**Branche :** `ClientEventHandler.java:108-131` — une entité coloniale rapide qui rejoint
un `ClientLevel` : toutes les copies de même UUID sont jetées.

**Ce qui manque :** `entity_join_level` est déclaré `side = "server"`
(`hooks/entity_join_level.rs:22-35`). Aucun équivalent client.

**Primitive exacte :** `client_entity_join` (client, `dimension`, `entity`, `kind`), ou
`entity_join_level` avec `side = "both"`.

## 10. Pas de table de palier de schéma

**Branche :** `ClientEventHandler.java:218-231` — en créatif, avec le scan tool :
`SchemAnalyzerUtil.getBlockTier(block)`, et le maximum sur les composants texturés d'un
bloc Domum.

**Ce qui manque :** `SchemAnalyzerUtil` lit les métadonnées des schématiques livrées (le
palier est une propriété du `.blueprint`). Le port ne livre pas ces schématiques avec cette
métadonnée, donc il n'y a rien à lire — **ce n'est pas une API manquante**.

**Primitive exacte :** aucune. C'est `src/blueprint/` qui devrait exposer le champ, et
c'est un travail de contenu.

## 11. Pas d'envoi de payload par joueur au moment du data-pack

**Branche :** `DataPackSyncEventHandler.java:66-74` — cinq messages à chaque joueur en
ligne (`sendPackets`), au `OnDatapackSyncEvent`.

**Ce qui manque :** `datapack_sync(player, recipe_types)` est un hook `collect` : il choisit
les **types de recettes** que ce client recevra, il n'envoie pas de charge utile, et « avec
personne en ligne ce hook n'est pas demandé » (`hooks/datapack_sync.rs:4-16`). La primitive
d'envoi existe pourtant (`PlayerRef::send_payload`, `host/payloads.rs:3-20`) : c'est le
**moment** qui manque.

**Primitive exacte :** `datapack_sync` demandé aussi sans joueur, avec un `ServerHost`, ou
`tags_updated(ServerDataLoad { reload })` accompagné d'un `Host` serveur pour itérer les
joueurs. Le rebuild, lui, tient déjà sur `tags_updated`.

## 12. Pas de génération de données

**Branche :** `GatherDataHandler.java:19-72`, 41 `addProvider`.

**Ce qui manque :** `runData` est une tâche Gradle qui s'exécute **avant** la construction du
jar. `cargo horizon-mod` n'a pas d'équivalent : il empaquette `data/` et `assets/` tels
quels. Le port **livre** donc le JSON que le générateur aurait écrit, et `gather_data.rs`
garde la liste des 41 pour un futur générateur.

**Primitive exacte :** une commande `cargo horizon-mod datagen` (ou un crate
`horizon-datagen`) qui exécute des `Provider` Rust et écrit dans `data/`. Hors périmètre de
ce module.

## 13. Pas de décorations d'item dans un slot

**Branche :** `ClientRegistryHandler.java:263-269` — `colonyMap` → `ColonyMapDecorator`,
`clipboard` → `ClipBoardDecorator`.

**Ce qui manque :** `register_item_models` remplace le **modèle**, pas la décoration
dessinée par-dessus l'icône dans un slot. Aucun hook ne dessine une décoration d'item.

**Primitive exacte :** `register_item_decorations` (client, une closure par item, reçoit la
matrice de GUI et la pile).

## 14. Pas d'enregistrement de renderer de block entity

**Branche :** `ClientRegistryHandler.java:348-354` — 7 `registerBlockEntityRenderer`
(`BUILDING` vide, `SCARECROW`, `ENCHANTER`, `COLONY_FLAG`, `NAMED_GRAVE`,
`DECO_CONTROLLER`, `COLONY_SIGN`).

**Ce qui manque :** `register_entity_renderers` prend des **types d'entité**.
`block_entity_submit` est une méthode de `BlockEntityPart` : elle dessine **un** type depuis
son propre Part, ce qui n'est pas un renderer général avec `render(state, pose, buffer)`.

**Primitive exacte :** `register_block_entity_renderers` (client, `(type, renderer)`), ou
un `BlockEntityPart::submit` avec l'état complet.

## 15. Pas de type de rendu de bloc

**Branche :** `ClientRegistryHandler.java:356-366` — chaque hutte et chaque culture en
`cutout`, sept blocs nommés en `cutout` (`ItemBlockRenderTypes.setRenderLayer`).

**Ce qui manque :** aucun hook ne décide de la couche de rendu d'un bloc.
`block_render_visibility` (`hooks/block_render_visibility.rs`) **cache** un état, il ne
choisit pas `cutout`/`solid`/`translucent`.

**Primitive exacte :** `register_block_render_type` (client, un état ou un bloc → une
couche), ou un champ de la déclaration de bloc dans le `.hmod`.

## 16. Une propriété d'item numérique ne voit pas son porteur

**Branche :** `ClientRegistryHandler.java:368-369` — `ItemProperties.register(spear,
"throwing", (item, world, entity, light) -> entity != null && entity.isUsingItem() &&
entity.getUseItem() == item ? 1 : 0)`.

**Ce qui manque :** `ItemModelRangePropertyEvent` porte `property`, `stack`,
`bundle_selected` et `value` (`generated/mod_api_generated.rs:15426-15431`) — **pas
d'entité**. La propriété `disabled` des goggles, elle, tient dans le hook (c'est un drapeau
que le mod possède), et c'est pour ça qu'elle est `Hook` et pas `Missing`.

**Primitive exacte :** un champ `holder: Option<LivingRef>` (ou `player`) dans
`ItemModelRangePropertyEvent`, pour que `isUsingItem()` soit lisible.

## 17. Pas d'intégration JourneyMap

**Branche :** `ClientChunkUpdatedEvent.java:22-25` — les deux seuls abonnés de l'événement
sont `compatibility/journeymap/v5/EventListener.java:78` et `v6/EventListener.java:75`, qui
dessinent les réclamations de colonie sur la carte.

**Ce qui manque :** le port n'a pas de mod de carte à notifier ; `ClientChunkUpdatedEvent`
reste donc une **valeur** (`client_chunk_updated::Updated`) sans abonné.

**Primitive exacte :** aucune. Le jour où un mod de carte Horizon existe, il lira `ChunkCap`
par son propre hook.

## Ce qui n'est pas un manque, et qu'il ne faut pas relire comme tel

- **Les 35 hooks cités existent** : `table::HOOKS` est la liste mesurée, et `hook_exists` la
  vérifie pour chaque ligne.
- **Les six lignes `Ported`** n'ajoutent pas de code parce que la règle est déjà dans
  `src/blocks`, `src/commands`, `src/data_listeners` ou `src/network` — deux copies d'une
  permission est la façon dont un mod finit par refuser un clic qu'il acceptait.
- **La durée des ticks n'est pas un trou** : `ServerHost::tick_times()` rend les cent durées
  en nanosecondes, indexées `n % 100` (`host/config.rs:96-102`), exactement
  `MinecraftServer.getTickTimesNanos`. J'avais d'abord classé cette ligne `Missing` parce que
  `ServerTickEvent` ne porte qu'un `LifecycleContext` ; c'était faux, et
  `event_handler::tick_duration_ms` lit maintenant la bonne source.
- **La sauvegarde de colonie n'est pas un trou non plus, mais un choix de conception**
  (ligne `Superseded`) : `FMLEventHandler.java:77-81` restaure un `colonies/*.dat` que le
  manager n'a pas lu. Le port n'a pas un fichier par colonie — `src/persistence` écrit les
  colonies dans les données du monde — donc il n'y a rien à restaurer. `ServerHost::world_dir()`
  existe (`host/config.rs:141-149`) si un jour le port veut un dossier à lui.
- **La génération de données** (§12) et le **palier de schéma** (§10) ne sont pas des trous
  d'API : ce sont une étape de construction et du contenu.
