# Ce qui manque — `src/channel`

Ce que j'ai voulu faire et que je n'ai pas pu, avec pour chaque ligne ce qui est porté à la
place et où. Deux choses seulement sont **bloquantes** : une paire de fonctions du moteur
non réexportée, et deux points de câblage qui vivent dans `lib.rs` (hors de mon périmètre).

Compte : **1 paire de fonctions** manquante (§1), **1 famille de distributeurs** sans
équivalent mod (§2), **2 appels de câblage** hors périmètre (§3), **1 trait qui vaudrait 132
implémentations** (§4), **2 moitiés client** qui n'ont pas encore de session (§5).

Rien ici n'est contourné en silence : le registre, l'aiguillage, le sens, la portée et le
découpage sont portés et testés (43 tests) ; ce qui manque est l'octet.

---

## 1. Le codec NBT réseau : `write_network` / `read_network`

**Ce que je voulais faire.** `Compound → PayloadBytes` à l'envoi, l'inverse à la réception :
c'est la seule conversion qui sépare `route::Envelope` (un compound) du corps d'un
`PayloadBytes` (des octets), et donc la seule chose qui empêche le canal d'être *vivant*.

**Ce qui existe dans le moteur.** `horizon_core::nbt::read_network(bytes, limits)` et
`write_network(tag, limits)` (`horizon-core/src/nbt.rs:835`, `:1046`) — le NBT réseau, varint,
racine sans nom, exactement ce qu'un paquet porte. Ils sont publics dans `horizon-core`.

**Ce qui est réexporté à un mod.** Seulement deux items de ce module :
`pub use horizon_core::nbt::{Error as NbtError, NETWORK_NBT_QUOTA};`
(`crates/horizon-mod-vocab/src/exports/horizon_core.rs:13`). Ni les fonctions, ni `Limits`,
ni `NamedTag`. Et `horizon_core` n'est pas une dépendance de ce paquet : `Cargo.toml:14`
déclare `horizon-mod-api` et rien d'autre, par choix (aucune dépendance de registre pour un
mod). Je n'ai pas le droit de toucher `Cargo.toml`.

**Porté à la place, et ce n'est pas un pis-aller.** C'est la même séparation que Java :
`handleSplitting` sérialise le message puis traite le résultat comme des **octets opaques**
(`NetworkChannel.java:443-448`), et `SplitPacketMessage` ne décode jamais rien. Donc :
`route::Envelope` porte un `Compound` (la monnaie de `src/network`), `split::Fragment` porte
des `&[u8]` (ce que Java découpe), et les deux moitiés sont testées séparément — y compris
le test qui prouve que l'identifiant sorti des fragments est celui que l'aiguillage route.

**Ce qu'il faudrait, au choix.** Réexporter `read_network`/`write_network`/`Limits` depuis
`horizon-mod-api` ; ou un host `PayloadBytes ↔ Compound`. Sur la route native du `.hmod`
(`[[native.bridge]]`), la fonction du moteur est atteignable directement — c'est une
décision d'emballage, pas une invention à faire dans ce module.

**Ce que j'ai vérifié avant d'écrire ça, pour ne pas rater une porte ouverte.** Le SDK a
bien un codec de charge utile : `encode_payload<T: PayloadCodec>(&T, usize)` et
`decode_payload<T: PayloadCodec>(&[u8])` (`surface.json`, `fn:encode_payload` /
`fn:decode_payload`), et le trait `PayloadCodec` (`decode(&mut PayloadReader)`,
`encode(&PayloadWriter)`). Mais la surface ne déclare **que deux implémentations** :
`VarInt` et `VarLong`. Un `Compound` n'en est pas une, et il ne peut pas le devenir —
c'est un alias de `BTreeMap<JavaString, Tag>` (`horizon-core/src/nbt.rs:35`) dans une
crate dont ce paquet n'est pas dépendant. Donc `encode_payload` ne porte pas un message,
et c'est bien le codec NBT qui manque, pas un emballage.

---

## 2. Les distributeurs : dimension, tous, entité suivie, chunk

Java a sept façons d'envoyer (`NetworkChannel.java:315-426`) : `sendToServer`,
`sendToPlayer`, `sendToOrigin`, `sendToDimension`, `sendToPosition`, `sendToEveryone`,
`sendToTrackingEntity`, `sendToTrackingEntityAndSelf`, `sendToTrackingChunk`. Le côté mod du
SDK n'en a **deux** : `player_send_payload(player, id, bytes)`
(`crates/horizon-mod-vocab/src/host/payloads.rs:14-19`) et `send_payload_to_server`
(`:31-35`), plus `IPayloadContext::reply` pour répondre sur la même connexion
(`payload.rs:228-232`).

Conséquence pour le canal : `route::Channel` ne décide **pas** qui reçoit. Une diffusion
« à tous ceux du chunk », « dans la dimension » ou « autour de cette position » est une
boucle sur les joueurs, et cette boucle appartient à la couche session — c'est écrit dans le
doc de `Channel`. Ce que le canal fournit pour cette décision est la ligne `range` de la
table : 38 messages à 5 blocs (`AbstractBuildingServerMessage.java:70-74`), 7 à 15
(`IRangeRestrictedPacket.java:11`), 94 sans portée, et `Routed::in_range` qui applique la
comparaison inclusive de Java.

---

## 3. Les deux appels de câblage (hors périmètre)

Je n'écris que dans `src/channel`. Ces deux lignes vivent donc ailleurs, et sans elles rien
ne s'enregistre :

1. **La déclaration.** Dans `Hooks::register` (`src/lib.rs:78-88`), un appel — le canal n'a
   qu'une seule déclaration, pas 139 :
   ```rust
   registrar.payload_handlers().registrar(channel::CHANNEL_VERSION)
       .play_bidirectional(id, handler, None);
   ```
   avec `id = PayloadId(channel::CHANNEL.to_owned())`. `route::Channel::declaration()`
   donne exactement ce `PayloadDeclaration` (une voie, `Play`, `Bidirectional`, requise,
   thread principal) et un test le vérifie.
2. **Le gestionnaire.** `IPayloadHandler::handle(PayloadBytes, &mut dyn IPayloadContext)`
   (`payload.rs:198-205`) est le point d'entrée à écrire : `split::Assembly::accept` sur un
   fragment, puis `route::Channel::receive` sur le corps recomposé. Rien de tout cela n'est
   inventé — les deux sont testés — mais l'implémentation du trait a besoin d'un `Arc` et
   d'un état par session, qui n'est pas dans ce module.

---

## 4. Le lien type Rust ↔ ligne de la table (1 trait = 132 implémentations)

`route::Routed::decode::<M>()` décode avec le codec du type qu'on lui donne ; il **ne
vérifie pas** que `M` est le type que la ligne nomme. Un `decode::<MarkStoryReadOnItem>()`
sur une enveloppe de `ItemSettingMessage` rend le refus du codec
(`Missing { key: "hand" }`), jamais un message faux — c'est testé, et c'est le pire cas réel.

Ce qu'il faudrait pour que ce soit vérifié à la compilation : un `trait Registered { const
ID: MessageId; }` implémenté 132 fois. Je ne l'ai pas fait, parce que ça recopierait dans
132 endroits un fait qui doit rester dans **une** table (c'est tout l'objet du registre), et
parce qu'une ligne fausse dans une implémentation serait plus dure à voir qu'une ligne
fausse dans le tableau. Si le besoin se confirme, l'implémentation est mécanique : `ID` =
`registry::ALL[i].id` de la ligne dont `rust` vaut `module::Type`.

---

## 5. Les deux moitiés client, qui n'ont pas encore de session

Le canal est **complet** pour ce qui est décidé ici, mais deux choses lui manquent côté
client, et aucune n'est un manque d'API :

- Un message `ToClient` a besoin d'un gestionnaire client, déclaré par
  `client_payload_handlers().register(&id, handler)` ou par l'événement
  `RegisterClientPayloadHandlersEvent` (`payload/registrar.rs:173-194`). Ce port n'a pas
  encore de session cliente (`lib.rs` ne branche aucun hook client), donc la place est
  libre et rien ne peut encore l'occuper.
- `HandlerThread::Main` est demandé pour la déclaration (le `ctx.enqueueWork` de Java,
  `SplitPacketMessage.java:138-148`). Le jour où un gestionnaire est écrit, il ne doit pas
  toucher au monde depuis un autre fil : la déclaration le lui interdit déjà.

---

## État, pour être clair

| Morceau | État |
| --- | --- |
| registre (139 lignes, doublon refusé nommé) | porté, 15 tests |
| aiguillage (id + corps → ligne, ou refus nommé) | porté, 12 tests |
| sens (86 serveur, 41 client, 12 les deux) | porté, lu dans `getExecutionSide()` |
| portée (38 × 5 blocs, 7 × 15, 94 sans) | porté, `in_range` inclusif |
| découpage (fragments, recomposition, trous refusés) | porté, 16 tests |
| enveloppe (id + compound) | porté, aller-retour testé |
| `Compound ↔ octets` | **manquant**, §1 |
| `[[native]]` / câblage du hook | **hors périmètre**, §3 |
