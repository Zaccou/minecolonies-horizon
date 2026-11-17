# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Comme `src/ui`, ce module n'appelle **aucune** API de l'hôte : il ne dépend que de
`std`, `crate::ui`, `crate::research` et `crate::building`. Rien n'a été ajouté à
`crates/`. Les manques ci-dessous sont donc soit **du SDK**, soit **du port**, et
chacun est nommé plutôt que contourné.

## 1. Un envoi au serveur

**Ce que je voulais faire** : qu'un clic fasse quelque chose. Cinq classes de
message sont nommées dans ce module — `ReactivateBuildingMessage`
(`WindowReactivateBuilding.java:61`), `TransferItemsToCitizenRequestMessage`
(`RequestWindowCitizen.java:237`), `UpdateRequestStateMessage`
(`RequestTreeWindowModule.java:332`), `TryResearchMessage`
(`WindowResearchTree.java:192`, `:216`), et `MarkBuildingDirtyMessage`
(`WindowResourceList.java:185`) — et le même `TryResearchMessage` porte deux sens
selon son troisième booléen (`research::START_FLAG`, `research::CANCEL_FLAG`).

**Ce qui existe** : rien. `ServerHost` n'a aucune route « message d'un client vers un
serveur », et `src/hut.rs` + `src/colony` sont la seule autorité sur l'état.

**Ce que j'ai fait** : `Button::sends()` rend le **nom** de la classe et rien d'autre,
et le booléen de `TryResearchMessage` est exposé par `Click::flag()`.

## 2. Un `Effect` qui sache fermer

**Ce que je voulais faire** : dire qu'un clic ne demande rien et ne fait que fermer
la fenêtre — `cancelClicked` de `WindowReactivateBuilding.java:69-72` est exactement
ça, et `back` de la fenêtre des requêtes est pire encore (il ne fait rien du tout).

**Ce qui existe** : `ui::action::Effect` a neuf variantes, aucune qui dise « fermer
sans rien demander » ; la plus proche, `Reopens`, dit le contraire (elle rouvre la
fenêtre d'où l'on vient).

**Ce que j'ai fait** : `Button::effect` est un `Option<Effect>` — `None` veut dire
« ne demande rien » — et `Button::leaves` porte la fin du clic (`Leaves::Closes`,
`Reopens`, `Nothing`, `Stays`). Les deux champs sont séparés parce qu'un clic peut
envoyer **et** fermer (`reactivate`, `fulfill`, `cancel`, un démarrage de recherche).

## 3. Une source pour l'item tenu en main

**Ce que je voulais faire** : dire d'où vient le montant d'entrepôt d'une ligne de
ressources — du compound de l'item rouleau, écrit côté serveur puis relu par
`ItemResourceScroll.java:85-95`.

**Ce qui existe** : `ui::field::Source` a neuf variantes, aucune pour un item.
`ui_window_02::pane::Source` en a ajouté une (`Item`) pour la clipboard, mais c'est
une **seconde** énumération que ce module n'utilise pas.

**Ce que j'ai fait** : `Source::PlayerInventory` pour `inWarehouseAmount`, avec le
commentaire qui dit pourquoi (c'est la seule variante qui dise « côté client, hors du
bâtiment »). Une variante `Item` dans `ui::Source` serait la vraie réponse ; elle
appartient à `src/ui`, que je n'écris pas.

## 4. Le coût de réinitialisation d'une recherche

**Ce que je voulais faire** : chiffrer le bouton d'annulation d'une recherche
**finie** — `IGlobalResearchTree.getResearchResetCosts()` et la comparaison avec
l'inventaire du joueur (`WindowResearchTree.java:776-810`).

**Ce qui existe** : `server.researchResetCost` est une liste d'ids d'items avec NBT
(`GlobalResearchTree.java:302-344`), lue d'une config serveur que le port n'a pas.
`research::gate::may_undo` refuse ce que l'arbre refuse **et ne facture rien** — le
module `research` le dit déjà.

**Ce que j'ai fait** : `Refusal::ResetCostUnknown`. Le clic sur une recherche finie
révèle bien le bouton (`Undo::Complete`), mais son prix n'est pas calculable : le
port s'arrête là plutôt que de l'afficher à zéro ou de l'interdire.

## 5. Le module de fenêtre qui décide de `fulfill`

**Ce que je voulais faire** : savoir si `fulfill` est allumé. La réponse dépend du
module qui a ouvert la fenêtre : `RequestTreeWindowModule.isFulfillable` rend `false`
(`:274-277`), `RequestWindowCitizen.CitizenRequestTreeWindowModule` le remplace
(`:156-175`).

**Ce qui existe** : `AbstractWindowSkeleton implements IModuleContainer<IWindowModule>`
(`:26`) et enregistre ses modules à l'ouverture ; le port n'a ni conteneur de modules
ni hiérarchie de fenêtres — `Window` y est une valeur, pas un objet avec un parent.

**Ce que j'ai fait** : `Fulfil` est un **paramètre** (`Clipboard` / `Citizen`) et
`may_fulfil` applique la règle de celui qu'on lui donne. `opener` ne nomme que le
premier des deux ouvreurs, et la doc dit lequel manque.

## 6. Une vue qui porte des panneaux à id dynamique

**Ce que je voulais faire** : décrire l'arbre de recherche comme les autres fenêtres,
un panneau par tuile.

**Ce qui existe** : `WindowResearchTree` fabrique **environ treize panneaux par
recherche** et leur donne l'id de la recherche à l'exécution (`setID(...)`,
`:507-508`, `:514-515`, `:521-522`, `:980`, `:988`, `:996`) : aucune constante ne
peut les nommer. Le SDK n'a que `add_button` et `add_text_field`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`), donc pas de vue
défilante ni de zoom.

**Ce que j'ai fait** : `dragView` est le seul panneau de la fenêtre, et la structure
d'une tuile est décrite par `ButtonState` — dont trois états seulement posent un id,
donc trois panneaux cliquables par tuile (`research::IDS_PER_TILE`).

## 7. Une vue de bâtiment pour l'université et le builder

**Ce que je voulais faire** : lire le niveau et le niveau maximum de l'université,
et `getWorkerName()` du builder.

**Ce qui existe** : `crate::building::Building` porte un niveau et des modules
sauvegardés, pas la vue cliente (`IBuildingView`) que ces fenêtres lisent.

**Ce que j'ai fait** : `Facts` et `Tile` prennent `level`, `max_level` et `depth` en
paramètres. Un appelant qui a la vue les remplit ; le port n'invente pas un
`University { level, max }` que personne ne remplit.
