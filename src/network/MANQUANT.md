# Ce qui manque — `src/network`

Ce que j'ai voulu faire et que je n'ai pas pu : soit la donnée n'existe pas dans
`/opt/data/.../.horizon/sdk/api/crates/`, soit la classe Java n'a pas d'équivalent dans
le port, soit l'objet vit dans une couche que ce module n'a pas le droit de toucher. Rien
ici n'est contourné en silence : chaque ligne dit ce qui est porté à la place, et où.

Compte : **14 champs opaques** (dont 2 objets de fabrique), **5 objets sérialisés par une fabrique**,
**6 identifiants de registre gardés en entier**, **4 choses que la couche réseau devra
faire** et que ce module ne fait pas.

---

## 1. Les charges utiles opaques (14 champs)

Java met dans le paquet un `FriendlyByteBuf` qu'une **autre** classe a rempli. Le port
garde ces octets tels quels (`Vec<u8>`, `common::bytes` / `bytes_of`), plus le cadrage
Java autour (le compte, la longueur, la colonie, la dimension). Un lecteur sait donc à qui
appartiennent les octets, mais pas ce qu'ils disent.

| Champ | Message | Producteur Java | Ce qu'il faudrait porter |
| --- | --- | --- | --- |
| `data` | `permissions::PermissionsView` | `Permissions.serializeViewNetworkData` (`PermissionsMessage.java:70`) | `Permissions.java`, 1 091 lignes |
| `colony_data` | `views::ColonyView` | `Colony.serializeViewNetworkData` (`:79-83`) | `Colony.java`, ~1 300 lignes |
| `building_data` | `views::ColonyViewBuildingView` | `IBuilding.serializeToView` (`:78-82`) | la hiérarchie `IBuilding` + ses vues |
| `citizen_data` | `views::ColonyViewCitizenView` | `ICitizenData.serializeViewNetworkData` (`:66-73`) | `CitizenData` |
| `work_orders` | `views::ColonyViewWorkOrder` | `IServerWorkOrder.serializeViewNetworkData` (`:50-54`) | les `WorkOrder*` (une par type) |
| `extensions` | `views::ColonyViewBuildingExtensionsUpdate` | `BuildingExtensionDataManager.extensionToBuffer` (`:74`) | le registre `buildingextensions` |
| `visitors` | `views::ColonyVisitorViewData` | `IVisitorData.serializeViewNetworkData` (`:76-78`) | `VisitorData` |
| `animals` | `views::ColonyViewAnimalViewData` | `IAnimalData.serializeViewNetworkData` (`:77-79`) | `AnimalData` |
| `compatibility` | `debug::UpdateClientWithCompatibility` | `ICompatibilityManager.serialize` (`:40`) | `CompatibilityManager`, neuf tables |
| `quests` | `debug::GlobalQuestSync` | `QuestJsonListener` (`:69`) | le chargeur de quêtes |
| `diseases` | `debug::GlobalDiseaseSync` | `DiseasesListener` (`:68`) | le chargeur de maladies |
| `field_data` | `building_modules::AssignField` | `BuildingExtensionDataManager.extensionToBuffer` (`:54`) | idem `extensions` |
| `value` | `buildings::TriggerSetting` | `StandardFactoryController.serialize` (`:83`) | la fabrique de réglages (§3) |
| `recipe` | `building_modules::AddRemoveRecipe` | `StandardFactoryController.serialize` (`:149`) | `IRecipeStorage` (209 lignes) + `RecipeStorage` |

**Ce qui n'est PAS opaque** et qu'il ne faut pas confondre : l'arbre de recherche
(`ColonyViewResearchManagerView.research`) est un seul compound NBT (`:54-56`) ; le
schéma d'un scan (`items::SaveStructureNbt.schematic`) est du NBT
(`SaveStructureNBTMessage.java:78-85`) ; un nœud de debug de chemin est cinq champs nommés
(`MNode.java:102-136`, porté dans `common::PathNode`). Ces trois-là traversent entiers.

---

## 2. Les identifiants de registre gardés en entier (6)

Pour ceux-ci, la donnée *existe* dans le paquet, mais la table qui lui donne un sens n'est
pas dans le port. Je garde le nombre et je m'interdis de deviner.

| Champ | Messages | Java écrit | Pourquoi le nombre |
| --- | --- | --- | --- |
| `block` / `state` | `particles::BlockParticleEffect`, `windows::OpenSuggestionWindow`, `founding::DirectPlace`, `colony_settings::ChangeFreeToInteractBlock` | `Block.getId(state)` | une `BlockState` est un état de bloc dans une table globale de registres ; rien dans `mod_api` ne convertit un entier en état |
| `job_entry` | `building_modules::AssignUnassign` | `writeRegistryId(getJobRegistry(), …)` (`:73`, `:94`) | le registre des métiers du mod n'est pas porté |
| `module_id`, `id` | une quinzaine de messages de bâtiment | `int` | c'est un identifiant d'exécution que le client a reçu, pas un nom |
| `category` | `sounds::PlayAudio` | `SoundSource.ordinal()` | enum vanilla, 11 valeurs, aucun type dans le port |
| `sound_source` | `sounds::PlaySoundForCitizen` | idem (`:168`) | idem |
| `color_ordinal` | `colony_settings::TeamColonyColorChange` | `ChatFormatting.ordinal()` | enum vanilla, 22 valeurs ; le port refuse hors `0..=21` |

Ce dernier est le seul des six où le port ajoute une borne, parce que Java lève une
exception d'index hors tableau sur la même valeur (`:69`).

---

## 3. Les objets sérialisés par `StandardFactoryController` (5, dont 2 déjà comptés en §1)

La fabrique est un registre de sérialiseurs : un identifiant d'usine suivi des champs de
l'objet. Le port n'a ni la fabrique ni les classes, donc il garde ce que la fabrique a
produit.

| Message, champ | Type Java | Ce que le port porte | Ce qu'il faudrait |
| --- | --- | --- | --- |
| `work::UpdateRequestState.token` | `IToken<?>` | `Compound` (`nested`) | la fabrique + `IToken` |
| `work::TransferItemsToCitizenRequest.token` | `IToken<?>` | `Option<Compound>` | idem |
| `items::RemoveFromRallyingList.location` | `ILocation` | `Compound` | les six classes de `location/` |
| `buildings::TriggerSetting.value` | `ISetting` | `Vec<u8>` — compté en §1 | les réglages (bool, int, string, liste…) |
| `colony_settings::TriggerConnectionEvent.event` | `ConnectionEvent` | `Vec<u8>` | `api/colony/connections/` |

**Un token n'est pas une position et une `ILocation` non plus** : elle peut désigner une
entité vivante (`EntityLocation`). C'est pourquoi je ne les réduis pas à `[i32; 3]`.

---

## 4. Ce qui n'existe pas dans le SDK, ou pas sous la forme voulue

| Ce que je voulais | Ce qui existe | Ce que j'ai fait |
| --- | --- | --- |
| un codec `Compound` → `Text` | `mod_api::Text`, un arbre complet (contenu, style, enfants, métadonnées) — `horizon-mod-vocab/src/text/mod.rs:9-31` — mais aucun `Wire`/codec NBT utilisable ici | `common::Component` réduit à `{texte, traduisible, arguments}`, ce que les quatre messages utilisent réellement |
| un type `UUID` public | `EntityUuid(pub u128)` existe mais est un identifiant *d'entité* (`entity/types.rs:127`) | `u128` + `colony::permissions::uuid` (déjà écrit dans le crate), écrit en texte comme `UUID.toString()` |
| un type « emplacement » (resource location) | `mod_api::Identifier` existe | non utilisé : les champs restent `String`, parce que le port écrit ce que Java écrit et qu'un `Identifier` refuserait les noms que Java accepte (un `ResourceLocation` peut être malformé côté paquet) |
| une liste typée | il n'y a **pas** de type `List` exporté ; c'est `Tag::List(Vec<Tag>)` (`horizon-core/src/nbt.rs:52`) | `common::records`, `common::texts`, `common::list_of`, `common::records_of` |
| un `BlockState` | rien | voir §2 |
| un envoi de paquet | rien : pas de socket, pas de `NetworkChannel` | voir §5 |

---

## 5. Ce que la couche réseau devra faire, et que ce module ne fait pas

1. **`onExecute`.** Aucun message ne porte d'effet : ouvrir une fenêtre, muter une
   colonie, engendrer un mob exigent un `Host`. Le champ existe (tous les champs sont
   publics), la règle est à écrire dans la couche qui possède la colonie.
2. **L'enregistrement des messages et leur identifiant.** `NetworkChannel.java` associe
   un `int` à chaque classe (`Network.java`) ; ce module ne connaît pas ces numéros, et
   `SplitPacket.inner_message_id` les nomme sans les résoudre.
3. **La réassemblage des fragments.** `debug::SplitPacket` porte les cinq champs d'un
   fragment ; recoller exige la table ci-dessus (`SplitPacketMessage.java:110`).
4. **`ColonyPackageManager`.** Les paquets de vue sont envoyés par un gestionnaire
   d'abonnements par joueur (`IColonyPackageManager`), qui décide *qui* reçoit *quoi*.
   Rien de cela n'est ici, volontairement : c'est un problème de session, pas de format.

---

## 6. Trois réductions assumées (ce ne sont pas des manques, mais des choix à valider)

- **`Map` → `Vec` de paires.** Le presse-papier (`ResourceScrollSaveWarehouseSnapshot`)
  envoie un `Map<String, Integer>` et l'écran d'artisanat un `Map<Integer, ItemStack>`.
  Java n'a pas d'ordre garanti ; le port garde l'ordre d'écriture, donc deux messages qui
  ne diffèrent que par l'ordre sont deux messages. Ça ne coûte rien et ça se teste.
- **`Set<MNode>` → `Vec<PathNode>`** (`debug::SyncPath`). Même raison, plus simple : un
  `HashSet` n'a pas d'ordre stable, donc l'ordre n'est pas une propriété du message.
- **`ItemStorage` → `common::ItemStack`** (`buildings::AssignFilterableItem`). Java
  enveloppe la pile dans un `ItemStorage` dont les deux booléens sont `false` sur ce
  chemin (`:62`, `:71`) ; le fil ne transporte que la pile.
