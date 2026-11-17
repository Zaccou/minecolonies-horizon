# Ce qui manque — `src/network_messages`

Ce que j'ai voulu faire et que je n'ai pas pu. Rien ici n'est contourné en silence : chaque
entrée dit **ce que le message devait déclencher**, **la recherche qui a échoué** (fichier et
ligne du SDK, ou commande dont la sortie est vide), **ce que le message fait à la place**, et
**ce qu'il faudrait** pour le porter.

Ce fichier n'est pas écrit à la main en parallèle du code : la liste des absences vient de
[`effects::absent`] et celle des demi-solutions de [`effects::partial`], et un test
(`census::tests::manquant_lists_exactly_the_absent_and_partial_capabilities`) compare les deux.
Une capacité trouvée plus tard et retirée du code ne peut donc pas rester ici sans faire
échouer le build.

Compte : **5 capacités absentes**, **3 partielles**, **1 chose que la couche réseau devra
faire** et que ce module ne fait pas. Les 20 autres capacités sont satisfaites — 10 par
`mod_api`, 10 par ce crate.

Le SDK mesuré est celui du dépôt de build :
`crates/horizon-mod-vocab/src/` (chemins relatifs à ce dossier ci-dessous).

---

## 1. `BlockParticle` — les particules d'un état de bloc

**Ce que je voulais faire.** `BlockParticleEffectMessage` demande au client de casser ou de
fissurer *un bloc* : `particleEngine.destroy(pos, block)` quand `side == BREAK_BLOCK`
(`BlockParticleEffectMessage.java:78`), `particleEngine.crack(pos, Direction.from3DDataValue(side))`
sinon (`:82`). Le message porte un `BlockState`, pas un type de particule.

**La recherche.** `host/effects.rs:54` offre `add_particle(&ParticleOptions, position, velocity)`
et `hooks/client_particles.rs` documente qu'il « preserves all option and velocity slots » :
c'est une particule **choisie**, pas la particule qu'un modèle de bloc produit quand il casse.
`grep -n 'crack\|destroy(' host/effects.rs hooks/client_particles.rs` ne renvoie que la prose
du second (`:59`, « the crackle after a blast »), aucune méthode.

**À la place.** L'effet est nommé (`Effect::ClientParticles`) et le message reste porté par
[`crate::network::particles::BlockParticleEffect`], qui garde `pos`, `block` et `side` tels
quels. Rien ne les transforme en particules.

**Ce qu'il faudrait.** Soit un appel `block_particle(pos, BlockState, Option<Face>)` dans
`ClientHost` qui atteint `ParticleEngine`, soit — et c'est la voie ouverte — un *provider*
enregistré par le mod (`register_particle_providers`) qui imite `destroy`/`crack` à partir du
modèle du bloc ; auquel cas la capacité redevient `Present` et cette entrée disparaît.

---

## 2. `Spawn` — faire apparaître une entité

**Ce que je voulais faire.** `HireMercenaryMessage` appelle
`EntityMercenary.spawnMercenariesInColony(colony)` (`HireMercenaryMessage.java:43`) : N
mercenaires apparaissent autour du bâtiment.

**La recherche.** `grep -n 'fn spawn' host/entities.rs host/mobs.rs host/levels.rs` ne renvoie
**qu'une** ligne : `entities.rs:95`, `spawn_falling_block`. Aucun `spawn_entity(type, pos)`,
aucun `spawn_mob`. Les hooks (`hooks/finalize_spawn.rs`, `hooks/mob_spawned.rs`) écoutent des
apparitions, ils n'en provoquent pas.

**À la place.** `Effect::SpawnEntity` et `Need::Spawn`, avec le verdict `Absent`. Le message
reste porté ; son corps n'est pas exécutable.

**Ce qu'il faudrait.** Un appel serveur `spawn(entity_type: &Identifier, level, pos, nbt)` et
son `Result` (type inconnu, position non chargeable, entité finale refusée par un hook), plus
la question du *propriétaire* — un mercenaire MineColonies a une colonie et un employeur, que
le SDK ne connaît pas.

---

## 3. `ChunkClaim` — une donnée de colonie attachée à un chunk

**Ce que je voulais faire.** `UpdateChunkCapabilityMessage` et
`UpdateChunkRangeCapabilityMessage` écrivent la revendication de colonie d'un chunk sur le
client (`UpdateChunkCapabilityMessage.java:73-90`, `…Range…:100-114`) : c'est
`IColonyTagCapability`, une *capability Forge posée sur `LevelChunk`*.

**La recherche.** `parts/shared.rs:133-138` : `ProvidedBy` énumère `BlockEntity`, `Block`,
`Item`, `Entity` — **pas `Chunk`, pas `Level`** ; `parts/capabilities.rs:178-181` teste ces
quatre-là. Il n'y a donc aucun porteur « chunk » pour une capability ou une pièce.

**À la place.** `Effect::ClientChunkClaim` et `Need::ChunkClaim`, verdict `Absent`. Les deux
messages restent portés avec leurs octets (`crate::network::debug::UpdateChunkCapability`), et
le client ne peut pas les appliquer.

**Ce qu'il faudrait.** Un porteur par chunk (pièce ou capability) et un moyen de le lire côté
client quand le chunk arrive. Une alternative que le port peut choisir : garder la carte
chunk → colonie **dans le mod** (une `Vec` indexée par `ChunkPos` dans l'état du mod) et
l'alimenter par les mêmes messages — coûteux en mémoire pour un monde chargé, mais sans SDK.

---

## 4. `Fragments` — recoller un message plus gros qu'une charge utile

**Ce que je voulais faire.** `SplitPacketMessage` découpe un message en fragments numérotés et
les recolle à l'arrivée (`SplitPacketMessage.java:85-154`) ; le découpage se fait à
`943718 = 90 % de 1 048 576` (`NetworkChannel.java:461-462`).

**La recherche.** `payload.rs:149-157` : la limite d'une charge utile est
`ClientboundCustomPayloadPacket.MAX_PAYLOAD_SIZE`, **1 048 576 octets**, et un envoi au-delà est
refusé (`PayloadSendError::PayloadTooLarge`). Aucune primitive de continuation, de fragmentation
ou de réassemblage n'existe : `send_payload_to_server` et `player_send_payload` prennent les
octets applicatifs d'un bloc.

**À la place.** Le module **porte lui-même** le réassemblage : [`fragments::Reassembler`], avec
la table par expéditeur, les trous refusés et les bornes que Java n'a pas. Le *découpage*
reste à faire par l'appelant, à `fragments::MAX_FRAGMENT`.

**Ce qu'il faudrait.** Rien du côté SDK : la limite de 1 Mio est la même que celle de Forge, donc
la fragmentation MineColonies se transpose telle quelle. Ce qui manque est un **expéditeur**
(§4 ci-dessous), pas une primitive.

---

## 5. `Advancement` — déclencher un trigger

**Ce que je voulais faire.** `ClickGuiButtonTriggerMessage` et `OpenGuiWindowTriggerMessage`
déclenchent un advancement pour l'expéditeur :
`AdvancementTriggers.CLICK_GUI_BUTTON.trigger(player, buttonId, resource)`
(`ClickGuiButtonTriggerMessage.java:66`), `OPEN_GUI_WINDOW.trigger(player, resource)`
(`OpenGuiWindowTriggerMessage.java:58`).

**La recherche.** `host/advancements.rs:5-14` : une seule méthode, `advancement(id)`, qui
**lit** un advancement chargé et rend `Option<AdvancementInfo>`. Rien ne déclenche de trigger,
ni ne donne un advancement à un joueur.

**À la place.** `Effect::Advancement` et `Need::Advancement`, verdict `Absent`. Les deux
messages sont portés ; l'effet ne l'est pas.

**Ce qu'il faudrait.** Un déclencheur générique (« ce joueur a rempli ce critère », avec ses
paramètres) ou, à défaut, un `grant_advancement(uuid, id)` qui donne l'advancement entier. Les
deux messages de ce lot n'existent *que* pour ça : ils sont donc, en l'état, entièrement
inertes dans le port, et il faut le dire plutôt que de les faire semblant.

---

## 6. `ScreenData` — ouvrir une fenêtre du mod *avec ses données*

**Ce que je voulais faire.** Neuf messages client ouvrent une fenêtre et lui donnent de quoi
s'afficher : `OpenBuildWindowMessage` (position, pack, chemin, rotation, miroir → une fenêtre
de prévisualisation de blueprint), `OpenDeleteAbandonColonyMessage` (nom et position de
l'ancienne colonie), `OpenCantFoundColonyWarningMessage` (un texte traduisible + une distance).

**La recherche.** `host/screens.rs:73` : `mod_screen_open(mod_id: &str) -> bool`. **Par
identifiant seulement.** Les données de la fenêtre ne traversent pas cet appel ; il n'y a pas de
`mod_screen_open_with(payload)`. Et l'appel peut échouer pour sept raisons énumérées dans sa doc
(un autre écran est ouvert, l'écran-titre, un livre, un panneau de signe, un dialogue, le
chargement du monde, un serveur dédié), sans dire laquelle.

**À la place.** `Need::ScreenData`, verdict `Partial` : la fenêtre s'ouvre (`Present`), les
données arrivent par une charge utile séparée, que le mod stocke avant d'ouvrir. **Deux appels
au lieu d'un, et une course** : si `mod_screen_open` refuse, les données sont arrivées pour rien
et il faut les jeter ou attendre le prochain tick.

**Ce qu'il faudrait.** Un `mod_screen_open` qui rende *pourquoi* il a refusé, et de préférence un
`mod_screen_open_with(mod_id, bytes)` atomique.

---

## 7. `MusicStop` — arrêter la musique sans handle

**Ce que je voulais faire.** `StopMusicMessage` arrête le gestionnaire de sons et la musique en
cours : `getSoundManager().stop()` et `getMusicManager().stopPlaying()`
(`StopMusicMessage.java:42-43`). Aucun handle, aucun identifiant : c'est un « stop » global.

**La recherche.** `host/effects.rs:91` : `sound_stop(sound_ref)` — il faut le `SoundRef` rendu
par `play_local_sound` (`:38`). Un son joué par le *jeu* (la musique d'ambiance, celle que
vanilla gère) n'a pas de `SoundRef` pour un mod.

**À la place.** `Need::MusicStop`, verdict `Partial`. `PlayMusicAtPos` et `PlayAudio` sont
portables tels quels (leur `SoundRef` est rendu par l'appel qui les joue) ; `StopMusic` ne l'est
pas.

**Ce qu'il faudrait.** `stop_all_sounds()` et `stop_music()`, ou un moyen de nommer la musique du
jeu.

---

## 8. `DiskWrite` — écrire un `.blueprint` dans le dossier du jeu

**Ce que je voulais faire.** `SaveStructureNBTMessage` reçoit un schéma en NBT et l'écrit sous
`blueprints/<joueur>/scans/<fichier>` (`SaveStructureNBTMessage.java:101-113`), puis affiche
« Scan successfully saved as … ».

**La recherche.** `instance_directory.rs:9-18` : le dossier d'instance est exposé comme une
**valeur** (`InstanceDirectory::Unix(Vec<u8>)` / `Windows(Vec<u16>)`), avec la phrase qui tranche :
« This is a path value, not filesystem authority. » Le mod sait *où*, pas *comment écrire*.

**À la place.** `Need::DiskWrite`, verdict `Partial`. Le message est porté avec ses octets NBT ;
l'écriture est l'affaire du mod, possible pour un mod natif (qui a un système de fichiers) et
impossible pour un mod wasm (qui n'en a pas). Le port ne choisit pas à la place du mod : il
nomme la limite.

**Ce qu'il faudrait.** Soit un `write_instance_file(path, bytes) -> Result<(), _>` dans l'hôte,
soit l'acte assumé que le scan est une fonctionnalité **native-mod** seulement. Dans les deux
cas, une entrée de plus dans `[[native]]`.

---

## 9. Ce que la couche réseau devra faire, et que ce module ne fait pas

Ce ne sont pas des manques du SDK : ce sont des choses qui vivent au-dessus d'un format.

1. **La table des messages.** Java associe un `int` à chaque classe
   (`NetworkChannel.java:443-446`, `messageTypeToIdMap`) et s'en sert pour décoder un fragment
   (`SplitPacketMessage.java:110`) et pour router un paquet. Le port a 134 types et **aucun
   numéro** : [`fragments::Whole`] rend l'`inner_message_id` et les octets, et il faut une table
   `i32 → décodeur` pour aller plus loin. Le modèle d'Horizon est l'inverse — une `PayloadId`
   nommée par message plutôt qu'un canal unique avec un id — et c'est un choix à faire, pas une
   donnée à recopier.
2. **L'enregistrement.** `PayloadRegistrar` / `RegisterPayloadHandlersEvent`
   (`payload.rs:11-14`) avec `PayloadDirection` et `HandlerThread` : la contrepartie de
   `SimpleChannel.registerMessage`, en un canal par message.
3. **Le fil d'exécution.** Forge fait `ctx.enqueueWork(...)` avant `onExecute`
   (`SplitPacketMessage.java:138`) : tout le corps des messages est donc sur le fil principal.
   Côté Horizon, `HandlerThread::Main` est le défaut et `PayloadContextError::WrongThread`
   existe ; la règle à écrire est que **aucun** `onExecute` ne tourne sur le fil réseau.
4. **L'expéditeur de fragments.** Rien dans le SDK ne découpe ; `fragments::fragments_for` donne
   le compte, `MAX_FRAGMENT` la taille. C'est un `fn send(message, targets)` à écrire dans la
   couche qui possède les connexions, avec la table du point 1.
