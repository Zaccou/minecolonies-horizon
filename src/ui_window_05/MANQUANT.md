# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : comme `src/ui`, dont il reprend la forme, il
est de la donnée pure — `std`, `crate::ui`, `crate::colony::ordre::WorkType`. Rien n'a été
ajouté à `crates/`. Les manques ci-dessous sont donc soit **du SDK**, soit **du port**, et
chacun est nommé plutôt que contourné. Les chemins cités sont ceux du SDK épinglé,
`.horizon/sdk/api/crates/`.

## 1. Une liste, ses lignes, et la ligne sous le clic

**Ce que je voulais faire** : décrire les cinq listes de ces quatre fenêtres — les plantes
d'un champ (`windowplantationfield.xml:20-29`, 4 icônes par ligne), les items du postbox
(`windowpostboxrequest.xml:9-16`), les requêtes ouvertes (`layoutrequeststree.xml:1-21`),
le stock minimum (`windowpostboxminstock.xml:10-22`) et les boutons de réponse
(`windowinteraction.xml:5`).

**Ce qui existe** : `Widgets` (`horizon-mod-vocab/src/client/widgets.rs:300-400`) n'a que
`add_button` (`:316`), `add_text_field` (`:322`), `draw` (`:394`), et le dessin brut dans `DrawContext`
(`host/draw.rs:34-90` : `fill`, `sprite`, `text`, `item_stack`, `gui_nine_slice`,
`push_clip`/`pop_clip`). Donc **dessiner une liste est possible** ; ce qui manque est sa
mécanique : un fournisseur (`getElementCount` / `updateElement`), une recherche de ligne
depuis le panneau cliqué (`getListElementIndexByPane`, utilisée par **quatre** boutons de ce
module, `WindowPostBoxMain.java:169`, `MinimumStockModuleWindow.java:76`,
`RequestTreeWindowModule.java:312`, `:347`) et une recherche de panneau **dans** une ligne
(`findPaneOfTypeByID` résolu dans la ligne, d'où le `Scope::Row`).

**Ce que j'ai fait** : chaque liste est un [`Pane`](field.rs) de `Value::Row` plus les panes
de sa ligne, dans l'ordre où Java les remplit ; la ligne introuvable porte un nom,
`Refusal::RowNotFound`, là où Java lit `-1` puis lève.

## 2. Un bouton fabriqué à l'exécution, avec échelle et retour à la ligne

**Ce que je voulais faire** : les boutons de réponse du dialogue, créés un par un dans un
`Box` vidé à chaque interaction (`WindowInteraction.java:104-144`), avec
`setTextRenderBox`, `setTextWrap(true)` et `setTextScale(min(1, (129 * 2.0) / textLen))`
(`:129-134`) — un libellé long est réduit et replié.

**Ce qui existe** : `Button { rect, label: Text, active }`
(`client/widgets.rs:82-104`) et `Widgets::button_mut` pour changer le libellé
(`:341-347`) : de quoi faire le **toggle** `deliverAvailable` (`RED_X` ↔ `APPROVE`,
`WindowPostBoxMain.java:190-202`). Il n'y a ni échelle, ni repli, ni largeur de rendu
calculée.

**Ce que j'ai fait** : la famille de boutons est décrite **une fois** par son préfixe
(`interaction::RESPONSE_ID_PREFIX`) et son id est calculé dans les deux sens
(`response_id`, `response_index`), parce qu'un `&'static [Action]` ne peut pas contenir
`response_1`…`response_N`.

## 3. Une barre d'onglets

**Ce que je voulais faire** : les onglets que le postbox et son stock partagent
(`WindowPostBoxMain.java:207-237` : un onglet « principal » puis un par vue de module dont
`isPageVisible()` est vrai, avec un son de page tournée).

**Ce qui existe** : rien. Pas de `registerTabButton`, pas de `setTabXOffset`, pas de bande
d'onglets ; au mieux une rangée de `Button` dessinés à la main.

**Ce que j'ai fait** : `postbox::tabs(n) = 1 + n` compte les onglets et
`postbox::TABS_X_OFFSET` garde le décalage de Java ; aucun n'est dessiné.

## 4. Ouvrir un écran **nommé**

**Ce que je voulais faire** : `Effect::Opens(id)`, `Effect::OpensJava(classe)` et
`Click::ClosesThenOpens("WindowBuildDecoration")` — trois façons de dire « cette fenêtre
s'ouvre ensuite ».

**Ce qui existe** : `ClientHost::mod_screen_open(mod_id)` ouvre **un** écran par mod
(`host/screens.rs:73`, `:77`, `:107`), et `observed_screen_id()` lit l'écran **courant**
(`client/types.rs:722-725`). Il n'y a donc aucune route vers une fenêtre désignée par son
nom : ni `WindowBuildDecoration` (réparation d'un champ, `WindowPlantationField.java:90`), ni
`WindowRequestDetail` (`RequestTreeWindowModule.java:381-384`), ni `WindowSelectRes`
(`MinimumStockModuleWindow.java:95`).

**Ce que j'ai fait** : nommer la classe et l'id, et dire dans la doc de `Click` qu'un id
n'ouvre rien aujourd'hui.

## 5. L'envoi au serveur

**Ce que je voulais faire** : qu'un clic aboutisse. Huit classes de message sont nommées et
aucune n'est envoyée : `InteractionClose`, `InteractionResponse`, `PostBoxRequestMessage`,
`UpdateRequestStateMessage`, `PlantationFieldBuildRequestMessage`,
`AddMinimumStockToBuildingModuleMessage`, `RemoveMinimumStockFromBuildingModuleMessage`,
`OpenInventoryMessage`.

**Ce qui existe** : aucune route client → serveur dans `ServerHost`, et le port n'a pas de
tranche réseau (`src/ui/MANQUANT.md` §5 le dit déjà pour les huttes). Une seule action
aboutit vraiment sans réseau : `Effect::Container`, parce que `src/hut.rs` déclare déjà le
conteneur.

## 6. Les modèles que ces fenêtres lisent

**Ce qui manque, côté port** :

- **un gestionnaire d'interaction** : `IInteractionResponseHandler` — l'enquête, la liste des
  réponses, la priorité, `isVisible`, `onClientResponseTriggered`
  (`api/colony/interactionhandling/IInteractionResponseHandler.java:27-160`) ;
- **un champ de plantation** : le tile entity ne porte pas que sa position — il donne le nom
  du pack, le chemin du blueprint, la rotation et le miroir (`WindowPlantationField.java:90-102`),
  que `crate::hut::State` n'a pas ;
- **un registre d'items client** : `allItemsPlusInventory`, `getDescriptionId`,
  `getHoverName`, le premier enchantement d'un livre
  (`WindowPostBoxMain.java:252-260`). `crate::inventory::Stack` porte un nom et un compte,
  pas ces trois chaînes ;
- **une vue de module** et un **gestionnaire de requêtes** côté client
  (`IMinimumStockModuleView`, `getOpenRequestsOfBuilding`, `IBuildingView.java:173`).

**Ce que j'ai fait** : `field::Source` dit **d'où** vient chaque valeur et
`postbox::Item` / `minstock::Row` portent les seuls champs qu'une règle lit, sans prétendre
être le modèle. Inventer un `Interaction { … }` que personne ne remplit aurait été pire.

## 7. Un champ de texte avec filtre à la frappe

**Ce que je voulais faire** : le champ de quantité du postbox refuse tout sauf des chiffres,
efface `0` et plafonne à 999 **pendant** la frappe (`InputFilters.java:22-52`).

**Ce qui existe** : `TextField::new(rect, max_length)` prend une longueur
(`client/widgets.rs:106-140`) et `TextEdit` gère l'édition, mais il n'y a pas de
`TextField.Filter` avec `isAllowedCharacter` + `filter(String) -> String`.

**Ce que j'ai fait** : `postbox::filter_qty` est la règle de Java, appliquée par l'appelant
après le widget ; un test la confronte à `qty_or_default` pour prouver que la paire tient.

**Ce qui n'est pas un manque** : le compte à rebours de dix ticks (`WindowPostBoxMain.java:268-276`)
se pilote par le hook `client_tick_post` (`hooks/client_tick_post.rs:13`) ;
`postbox::State::update` est cette règle, sans le hook.
