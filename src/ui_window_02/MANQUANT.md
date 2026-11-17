# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : c'est de la donnée pure, comme `src/ui`,
`src/jobs` et `src/colony`. Il n'utilise que `std`, `crate::ui` (pour `Value`, `Scope`,
`Field` et les neuf sources existantes), `crate::building::Kind` et `crate::blueprint::Size`.
Rien n'a été ajouté à `crates/`. Les manques ci-dessous sont donc soit **du SDK**, soit
**du port**, et chacun est nommé plutôt que contourné.

## 1. Parcourir un dossier de packs (le navigateur)

**Ce que je voulais faire** : la découverte de `WindowBuildingBrowser` — lister les packs
(`StructurePacks.getPackMetas()`, `WindowBuildingBrowser.java:247`), marcher chaque
répertoire (`Files.walk`, `:296`), ouvrir chaque `.blueprint` (`:303`) et remplir un cache
par bloc (`:219-228`), le tout sur quatre fils (`:240-246`).

**Ce qui existe** : `Host::resource(PackType::ServerData, path)`
(`crates/horizon-mod-vocab/src/host/resources.rs:20`) lit **un** chemin, celui que la pile
de données du serveur déclare gagnant. Il n'y a ni liste de dossier, ni énumération de
packs, ni fil. `crate::blueprint::Blueprint::parse` sait lire les octets d'un fichier déjà
trouvé (`src/blueprint/format.rs:367`), donc le manque est l'**énumération**, pas la
lecture.

**Ce que j'ai fait** : toutes les règles de la découverte qui ne demandent aucun accès
disque sont ici et testées — `path_segment` (`:366-374`), `classify` (`:340-349`),
`flatten_levels` (`:385-405`), `blueprint_is_invisible` (`:408-429`), `format_levels`
(`:199-214`) et la barre (`:135-139`). Elles prennent les fichiers **déjà trouvés** ; le
jour où une route d'énumération existe, elle se branche ici sans réécrire une règle.

## 2. `DropDownList.selectNext()` / `selectPrevious()` (les deux styles)

**Ce que je voulais faire** : dire ce que font `previousStyle` et `nextStyle` au bord de la
liste (`WindowBuildBuilding.java:453-464`).

**Ce qui existe** : la méthode appelée est `DropDownList.selectNext()` de **BlockUI**, qui
n'est pas dans l'arbre des sources (`/workspace/minecolonies-src` est MineColonies ;
BlockUI est un mod séparé). Le port ne peut donc pas citer la ligne qui décide du bord.

**Ce que j'ai fait** : `Dropdown::next`/`previous` bouclent, et c'est dit dans leur doc.
Le seul cas atteignable est une liste d'**un** élément (`styles` n'en reçoit jamais deux,
`:263-288`), donc le bord ne change rien d'observable — le test le vérifie au lieu de le
supposer.

## 3. Envoyer un message client → serveur

**Ce que je voulais faire** : que les cinq classes nommées fassent quelque chose :
`BuildRequestMessage`, `BuildPickUpMessage`, `BuildingSetStyleMessage`, `ItemSettingMessage`,
`UpdateRequestStateMessage`, plus la classe que la décoration reçoit en paramètre.

**Ce qui existe** : rien. `ServerHost` n'a aucune route « message d'un client vers un
serveur », et le port n'a pas de tranche réseau. `Message` porte donc le **nom** de la
classe et, quand il décide du sens, un mot de charge utile (`BUILD`, `REPAIR`, `REMOVE`,
`CANCELLED`) — jamais un envoi.

**Ce que j'ai fait** : nommer, avec le mot qui compte, et un `Press` qui garde **l'ordre**
des deux messages du clic `build` (style puis demande, `:221-229`) : c'est la seule chose
qu'un futur envoi devra respecter et qu'un test peut tenir aujourd'hui.

## 4. `WindowConfirm`, et le piège de ses deux boutons

**Ce que je voulais faire** : décrire la boîte de confirmation qu'ouvre `build` quand le
niveau porte un avertissement (`WindowBuildBuilding.java:207-211`).

**Ce qui existe** : rien dans ce module — `Route::ConfirmDialog` la **nomme** et
`Exit::OpensJava` nomme la fenêtre, sans la décrire. Attention au piège du source : dans
`windowconfirm.xml:5-10`, le bouton d'`id` **`cancel`** porte le libellé `gui.yes` et
celui d'`id` **`confirm`** porte `gui.cancel`, et `WindowConfirm.java:24-25` lie `confirm`
à `close()` et `cancel` à l'action. Un port qui se fierait aux ids inverserait la boîte :
**c'est le libellé qui dit ce que le bouton fait.**

## 5. La position du joueur et le gestionnaire de requêtes (la clipboard)

**Ce que je voulais faire** : trier les requêtes comme Java — distance à
`Minecraft.getInstance().player.blockPosition()` puis `hashCode` du jeton
(`WindowClipBoard.java:170-174`) — et remonter d'un enfant à sa racine par le
`IRequestManager` (`:141-152`).

**Ce qui existe** : `crate::requests` porte la requête et son demandeur
(`src/requests/request.rs:130`, `:113`), mais **pas** le gestionnaire : ni les jetons, ni les
résolveurs, ni la remontée de parent. Et il n'y a pas de joueur client dans cette tranche.

**Ce que j'ai fait** : `shown(rows, filter, player)` prend la position en argument (une
position est de l'arithmétique, pas un accès au monde) et `root_of` remonte la chaîne de
parents que l'appelant fournit, avec un refus nommé quand un parent manque — Java, lui,
écarte la requête sans rien dire (`:149-152`).

## 6. Les métadonnées de pack (la décoration)

**Ce que je voulais faire** : lire le pack que `WindowBuildDecoration` reçoit en texte et
nettoie par `packMeta.replace(Minecraft.getInstance().player.getUUID().toString(), "")`
(`WindowBuildDecoration.java:132`).

**Ce qui existe** : rien. Le port ne modélise pas les métadonnées de pack — c'est un nom
que l'appelant fournit ; la substitution d'un UUID dans une chaîne est une bizarrerie du
source, pas une règle de données.

**Ce que j'ai fait** : la fenêtre de décoration ne porte que ce qu'elle **demande** — son
blueprint (trois valeurs : pas encore lu, absent, trouvé) et sa liste de builders — et
`MANQUANT.md` garde la trace du nettoyage de chaîne pour la tranche qui ouvrira le fichier.
