# Rapport — les messages réseau de MineColonies, portés en données

Périmètre : `src/main/java/com/minecolonies/core/network/messages/`, **128 fichiers
`.java`** (mesure : `find … -name '*.java' | wc -l` sur l'arbre), 12 783 lignes de Java.
Livré : `src/network/`, **16 fichiers `.rs`** (`mod.rs`, les 14 familles — `common`
comprise — et `census.rs`), 8 761 lignes dont 4 284 de code, **134 types** et **119 tests**. Rien d'autre n'a été touché — ni `lib.rs`, ni
`Cargo.toml`, ni un autre module, ni `data/`.

---

## 1. Ce que j'ai porté (compte exact)

| Fichier | Famille | Fichiers Java | Lignes Java | Types | Tests |
| --- | --- | ---: | ---: | ---: | ---: |
| `permissions.rs` | `PermissionsMessage` (9 classes imbriquées) | 1 | 724 | 9 | 8 |
| `founding.rs` | fondation, ramassage, réactivation, abandon + 4 fenêtres de l'hôtel de ville | 11 | 1 074 | 11 | 9 |
| `colony_settings.rs` | nom, 3 styles, bannière, couleur, libre-accès, mercenaires, espions, téléportation | 11 | 880 | 11 | 10 |
| `buildings.rs` | les 21 messages qu'un bâtiment traite directement | 21 | 1 776 | 21 | 11 |
| `building_modules.rs` | les 20 que traite un module (champs, recettes, niveaux, filtres) | 20 | 1 729 | 20 | 11 |
| `work.rs` | ordres de travail, état de requête, livraisons, recettes, 2 demandes de plan | 9 | 1 135 | 9 | 11 |
| `citizens.rs` | compétences, pause, rappel, redémarrage, 2 réponses de dialogue | 6 | 559 | 6 | 5 |
| `views.rs` | le flux de vue de colonie + 1 ouverture de GUI côté serveur | 14 | 1 334 | 14 | 9 |
| `particles.rs` | les 8 effets de particules | 8 | 915 | 8 | 7 |
| `sounds.rs` | 2 sons, le stop, musique et voix de citoyen | 4 | 479 | 4 | 6 |
| `windows.rs` | les 7 « ouvre cet écran » | 7 | 581 | 7 | 6 |
| `items.rs` | les 6 messages qu'un objet envoie pour lui-même | 6 | 528 | 6 | 5 |
| `debug.rs` | chemins, claims de chunk, 3 charges entières, le porteur de fragments | 8 | 821 | 8 | 6 |
| `common.rs` | les 11 codecs, le refus, les 2 enveloppes, 3 valeurs partagées | **2** (les bases abstraites) | 248 | — | 10 |
| `census.rs` | le recensement : 128 fichiers, vérifiés | — | — | — | 5 |
| `mod.rs` | le module, sans type | — | — | — | — |
| **Total** | | **128** | **12 783** | **134** | **119** |

**Pourquoi 128 fichiers font 134 types.** 126 fichiers sont des messages, à une classe
par fichier — sauf `PermissionsMessage.java`, qui est un espace de noms contenant **neuf**
classes imbriquées (`:35-723`) : 125 + 9 = 134. Les **deux** fichiers restants sont les
bases abstraites, et elles ne sont pas des types : `AbstractColonyServerMessage` est
`common::envelope` + `common::Message::RULE` + `common::RANGE`,
`AbstractBuildingServerMessage` est `common::building_envelope` + `BUILDING_RANGE`.
`census.rs` vérifie l'arithmétique : `files() + ABSTRACT_BASES == 128`, `LIGNES == 134`,
un seul fichier à plusieurs lignes, aucun type nommé deux fois, aucun fichier fantôme.
J'ai aussi vérifié la table contre l'arbre avec un script externe : **0 manquant, 0
en trop, 0 doublon**.

**La forme de chaque type.** Un struct à champs publics + `impl Message` :
`to_compound()` écrit chaque champ sous une clé camelCase **nommée d'après le champ
Java** (`colonyId`, `buildingId`, `dimensionId`, `townHallPos`…), `from_compound()` les
relit ou rend un `Refusal` nommé. Le contrat testé partout est
`from_compound(to_compound(m)) == Ok(m)`, plus une seconde écriture identique (le test
vérifie l'idempotence, pas seulement l'aller-retour).

---

## 2. Ce que j'ai écarté, et pourquoi

| Écarté | Raison |
| --- | --- |
| `onExecute` de tous les messages | ouvre une fenêtre, mute une colonie, engendre un mob : il faut un `Host`. Le champ est porté, l'effet est pour la couche qui possède la colonie. |
| l'envoi de paquets (`NetworkChannel`, `Network`, `IMessage.toBytes`) | il n'y a pas de socket dans un mod de ce port ; `to_compound` remplace `toBytes` et `FriendlyByteBuf`. |
| `ColonyPackageManager` / `sendColonyViewPackets` | décide *qui* reçoit *quoi* : problème de session, pas de format. |
| le réassemblage des fragments (`SplitPacketMessage.onExecute`) | exige la table des types de messages (`:110`). `debug::SplitPacket` porte le fragment, pas le recolleur. |
| la classe enveloppe `PermissionsMessage` | elle n'a aucun état (`:35-36`), seulement neuf classes imbriquées : c'est le module, pas un type. |
| `OpenBuildWindowMessage` (abstraite) | portée comme `windows::OpenBuildWindow`, la base ; les deux sous-classes sont des newtypes qui **écrivent les mêmes octets** — deux écrans, un fil. |
| `QuarryHiringModeMessage.moduleID` | Java ne le stocke pas dans son constructeur (bug, §5) — le champ est porté (le fil le transporte), le bug n'est pas reproduit. |
| `ownerOnly()` de la base | sa branche est inversée en Java (§5) : le port implémente l'intention, pas le code. |
| `ItemStorage` (2 booléens) | faux sur ce chemin ; la pile suffit (§6 de `MANQUANT.md`). |
| `Map`/`Set` → `Vec` | pas d'ordre garanti côté Java ; le port garde l'ordre d'écriture (§6 de `MANQUANT.md`). |

---

## 3. Les références Java utilisées

Chaque type porte, dans son commentaire, `fichier.java:ligne` **de la méthode qui écrit
ou lit les octets**, pas seulement le nom de la classe. Exemples de ce qui est cité
systématiquement :

- `toBytes` / `fromBytes` de chaque message (la source des clés et de leur ordre) ;
- les constantes : `TownHallRenameMessage.java:17-18` (25 caractères), `IRangeRestrictedPacket.java:11`
  (15 blocs), `AbstractBuildingServerMessage.java:70-73` (5 blocs), `MNode.java:102-136`
  (les cinq champs d'un nœud), `ChunkCapData.java:56-72` (les quatre champs écrits, et le
  `todo` qui explique pourquoi le cinquième ne l'est pas) ;
- les enums : `Action.java:9-43`, `RequestState.java:16-78`, `Skill.java:10-150`,
  `WorkOrderType.java:10-13`, `HiringMode.java:10-13`, `Direction.get2DDataValue` ;
- les règles du socle : `AbstractColonyServerMessage.java:60-64` et `:122-141`,
  `AbstractBuildingServerMessage.java:61` et `:80`.

Trois sources ont été lues **hors** du répertoire des messages parce qu'un champ les
traverse : `api/util/ChunkCapData.java`, `core/entity/pathfinding/MNode.java`,
`api/colony/permissions/{Action,Rank}.java`, plus `api/colony/requestsystem/request/RequestState.java`.

---

## 4. Les manques d'API rencontrés

Détail complet dans `src/network/MANQUANT.md`. Résumé :

1. **14 charges utiles opaques** — un `FriendlyByteBuf` rempli par une autre classe
   (vue de colonie, de bâtiment, de citoyen, ordres, extensions, visiteurs, animaux,
   compatibilité, quêtes, maladies, champ, réglage, recette). Portées en `Vec<u8>` avec
   leur cadrage ; `MANQUANT.md` §1 nomme le producteur de chacune et ce qu'il faudrait
   porter pour les lire.
2. **5 objets de fabrique** (`IToken`, `ILocation`, `ISetting`, `IRecipeStorage`,
   `ConnectionEvent`) : portés comme `Compound` ou `Vec<u8>`.
3. **6 identifiants de registre gardés en entier** (état de bloc, id de métier, id de
   module, `ChatFormatting`, `SoundSource`) : la table qui leur donne un sens n'est pas
   dans le port, et je ne devine pas.
4. **Pas de codec `Compound` ↔ `Text`** : `mod_api::Text` porte tout un arbre, mais rien
   ne le convertit ici ; `common::Component` est réduit à `{texte, traduisible, arguments}`,
   ce que les quatre messages utilisent réellement.
5. **Pas de type `List`** exporté : c'est `Tag::List(Vec<Tag>)`, donc quatre helpers
   (`records`, `texts`, `list_of`, `records_of`).

Aucune API n'a été inventée : tout ce que ce module appelle est `Compound`, `Tag`,
`Identifier` (indirectement, via `inventory::ItemName`) et les modules déjà écrits du
crate (`base::Dimension`, `colony::{Rank, Action, Members}`, `colony::permissions::uuid`,
`inventory::ItemName`, `requests::State`).

---

## 5. Ce que j'ai trouvé faux dans Java en portant

Quatre choses, constatées en lisant la source, pas supposées :

1. **`AbstractColonyServerMessage` refuse le propriétaire** (`:132-141`) : la condition
   `ownerOnly() && (player == null || owner.equals(player.getUUID()))` **puis** envoie
   « permission refusée » — donc le propriétaire est refusé et tout non-propriétaire
   passe. Aucun message du répertoire ne met `ownerOnly()` à `true`, donc la branche ne
   se déclenche jamais : le bug est dormant. Le port implémente l'intention
   (`Rule::OwnerOnly`) et le dit dans `common::Rule`.
2. **`QuarryHiringModeMessage`** : le constructeur prend un `moduleID` et ne le stocke
   pas (`:42-46`), donc le champ part toujours à `0`. Le port garde le champ.
3. **`TransferRecipeCraftingTeachingMessage`** : dans la branche « incomplète », les
   quatre slots lisent les clés `0, 1, 3, 4` — la clé `2` n'est jamais lue et la clé `4`
   n'existe pas dans une grille de quatre (`:107-110`). Décalage d'indice.
4. **`OpenDeleteAbandonColonyMessage`** envoie `oldColonyId` (`:63`) que la fenêtre
   qu'elle ouvre n'utilise pas (`:54`). Le port garde le champ : c'est le fil, pas
   l'écran, qui définit un message.

Deux détails d'implémentation, sans gravité, notés au passage : `ItemSettingMessage`
n'a **pas** de `getExecutionSide` (`:13-62`), contrairement à tous les autres, et
`HireMercenaryMessage.permissionNeeded()` ne fait que rappeler `super` (`:27-32`).

---

## 6. Vérification

```sh
cargo test --lib network::            # 119 tests, 0 échec
cargo clippy --lib --tests            # 0 avertissement dans src/network
```

Les 119 tests couvrent, par famille : un aller-retour par type (les 134 types
apparaissent tous dans au moins un test — vérifié par script), un test de refus par
famille au moins (champ manquant, mauvaise sorte de NBT, nom d'enum inconnu, borne
dépassée), et cinq tests transverses : le refus et ses noms, les cinq portes de
permission, les deux enveloppes, le recensement, la table des noms d'`Action`.

Trois points de méthode, pour que la confiance soit placée au bon endroit :

- **Les tests sont des aller-retours, pas des comparaisons avec un client Java.** Ils
  prouvent que la lecture est l'inverse exacte de l'écriture et que les refus sont
  nommés — pas que les clés choisies sont celles qu'un client MineColonies
  reconnaîtrait. Ce module n'a pas de correspondant Java en face : la seule preuve
  possible, c'est la citation de la ligne qui écrit chaque champ.
- **Les `f64` et `f32` sont stockés en bits** (`Tag::Double(u64)`), donc `-0.0` survit à
  l'aller-retour ; deux `NaN` se comparent égaux au niveau du tag, ce qui est un
  artefact de l'encodage et pas une règle du message.
- **Le volume annoncé (1 800–2 500 lignes) n'est pas tenu** : 8 761 lignes, dont 4 284 de
  code, 2 164 de tests et 1 480 de documentation. À ~32 lignes de code et de test par
  message (134 types, 119 tests, une citation Java par type), l'estimation supposait
  environ 15 lignes par message, ce qui n'inclut ni la citation de la ligne qui écrit le
  champ, ni le test d'aller-retour. J'ai préféré le compte exact à la densité : si le
  volume est une contrainte, ce sont les commentaires (1 480 lignes) qu'il faut couper,
  pas les tests.

---

## 7. Limites, dites franchement

1. **Rien n'est câblé.** `lib.rs` déclare déjà `pub mod network;` ; aucun autre module
   n'appelle encore `src/network`. Je n'ai pas touché `lib.rs` (règle 1) : brancher le
   réseau sur les crochets (`player_logged_in`, un envoi côté serveur) est une décision
   de l'orchestrateur, et elle demande la couche session qui n'existe pas.
2. **Les 14 champs opaques sont le vrai trou**, et il est nommé, pas masqué : tant que
   `Colony.serializeViewNetworkData` n'est pas porté, `views::ColonyView` transporte un
   blob — un aller-retour marche, une lecture ne dit rien.
3. **`Rule::allows` n'est testé que contre `colony::Members`**, pas contre une colonie
   réelle : il n'y a pas de colonie dans ce module, et `canPlaceAt`/`getOwner` vivent
   ailleurs.
4. **Trois messages n'ont pas de côté d'exécution dans le port** (`ItemSetting` en Java
   n'en a pas non plus), et aucun ne le déclare : `Message` n'a pas de `side()`. C'est un
   choix : un côté est une propriété de l'envoi, et l'envoi n'est pas ici.
