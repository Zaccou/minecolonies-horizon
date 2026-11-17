# MANQUANT — ce que les écrans de huttes auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : `src/ui` est de la donnée pure, comme
`src/jobs` et `src/colony`. Il n'utilise que `std`, `crate::jobs`, `crate::building` et
`crate::hut::State` (cité, pas appelé). Rien n'a été ajouté à `crates/`. Les manques
ci-dessous sont donc soit **du SDK**, soit **du port** — et chacun est nommé plutôt que
contourné.

## 1. Un widget de liste

**Ce que je voulais faire** : décrire une liste scrollable, puisqu'elle est le cœur de
sept des treize écrans — les workers (`layouthuts/layouthutpageactions.xml:9-15`), les
résidents d'une maison (`windowhuthome.xml:6-12`), les points de spawn de la caserne
(`windowhutbarracks.xml:10-14`), les ressources du builder
(`layoutbuilderres.xml:9-22`), les niveaux de la mine (`layoutminermodule.xml:7-22`), les
tâches d'un crafter (`layouttasklist.xml:7-17`) et tout l'inventaire
(`windowhutallinventory.xml:11-20`).

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que **deux**
constructeurs, `add_button` et `add_text_field`, et un `draw` (`:394`). Il n'y a ni liste,
ni ligne, ni barre de défilement, ni libellé. Les hooks d'entrée couvrent le clic, le
relâchement, le glisser, la molette, la touche et le caractère
(`crates/horizon-mod-vocab/src/client/widgets.rs:1-10`), donc **l'entrée d'une liste
existe** ; c'est son rendu qui n'existe pas.

**Ce que j'ai fait** : chaque liste est un [`Field`] de [`Value::Row`] plus les champs de
sa ligne, dans l'ordre où Java les remplit. Un pont vers `Widgets` demanderait le widget
manquant et n'est pas dans cette tranche.

## 2. Un identifiant d'écran dans le SDK

**Ce que je voulais faire** : nommer un écran par un id que le SDK connaîtrait, pour que
`Effect::Opens` et la table métier → écran s'appuient sur un type du SDK.

**Ce qui existe** : `ClientHost::mod_screen_open(mod_id)`
(`crates/horizon-mod-vocab/src/host/screens.rs:73`, `:2187`) ouvre **un** écran par mod, et
`native_mod_screen()` dit lequel est ouvert. Il n'y a pas de notion d'écran nommé, encore
moins d'écran par hutte ou d'onglet.

**Ce que j'ai fait** : un id `&'static str` du port (`hut.worker`, `hire`, `tab.tasks`…),
et une résolution [`crate::ui::screen_by_id`] qui répond `Refusal::UnknownScreen` pour un id
absent. Un test parcourt toutes les lignes et tous les `Effect::Opens` pour prouver qu'aucun
id n'est cassé.

## 3. Un onglet de barre latérale

**Ce que je voulais faire** : décrire la barre latérale d'une hutte comme Java la construit
— un onglet par vue de module dont `isPageVisible()` est vrai
(`AbstractBuildingWindow.java:67-83`).

**Ce qui existe** : rien. Le SDK n'a pas de barre latérale, et le port n'a pas de vue de
bâtiment cliente : `crate::building::Building` ne porte qu'un niveau et des modules
sauvegardés (`crate::building::Module`), pas la liste des vues qu'un client énumérerait.

**Ce que j'ai fait** : la jointure est la **clé de module**, la même que
`crate::building::Module::key()` sauvegarde, et [`crate::ui::MODULE_SCREENS`] la traduit en
écran (5 clés → 3 écrans). Ce qui manque n'est donc pas la table mais *quels modules une
hutte porte* — c'est `ModBuildingsInitializer` côté Java, et le port ne l'a pas encore.

## 4. Un type de formulaire

**Ce que je voulais faire** : un écran qui demande une saisie (le renommage,
`windowhutnameentry.xml:6`, et le filtre de l'inventaire, `windowhutallinventory.xml:7`).

**Ce qui existe** : `TextField` dans `Widgets` (`widgets.rs:106-216`) et
`Widgets::add_text_field` (`:322`) — donc **la saisie existe**, avec son `maxlength`
(`TextField::new([x,y,w,h], 32)` prend la longueur en second argument).

**Ce que j'ai fait** : un [`Field`] de [`Value::Input`], et les deux longueurs du nom de
hutte en constantes (`MAX_NAME_LENGTH = 15` pour la règle,
`NAME_FIELD_MAX_LENGTH = 25` pour le champ). La différence entre les deux est une bizarrerie
de Java, pas un manque du SDK.

## 5. Un envoi au serveur

**Ce que je voulais faire** : qu'un clic fasse vraiment quelque chose. Seize actions de ce
module portent le nom d'un message Java (`HireFireMessage`, `RecallCitizenMessage`,
`ForcePickupMessage`, `AssignUnassignMessage`, `BuildingHiringModeMessage`,
`TransferItemsRequestMessage`, `MinerSetLevelMessage`, `MinerRepairLevelMessage`,
`BuildRequestMessage`, `PauseCitizenMessage`, `RestartCitizenMessage`,
`ChangeDeliveryPriorityMessage`…).

**Ce qui existe** : côté hôte, `ServerHost` n'a aucune route « message d'un client vers un
serveur » ; le port n'a pas de tranche réseau du tout, et `src/hut.rs` +
`src/colony` sont la seule autorité sur l'état d'une hutte. `Effect::Sends` porte donc le
**nom** de la classe Java et rien d'autre.

**Ce que j'ai fait** : nommer la classe, et dire dans la doc de [`Effect::Sends`] que rien
n'est envoyé. Une action `Container` est la seule qui aboutit vraiment, parce que le port
déclare déjà le conteneur de la hutte (`hut::container`, expliqué dans `src/menu.rs`).

## 6. Les valeurs d'un écran

**Ce que je voulais faire** : qu'un écran rende les vraies valeurs (le nom du worker, son
niveau, le pourcentage du builder).

**Ce qui existe** : [`crate::hut::State`] — niveau, embauchés, colonie, position du record —
et `crate::colony::Citizen` — identifiant et nom. Aucun des deux ne porte la liste des
workers d'une hutte, les ressources d'un build, les niveaux d'une mine ni une tâche.

**Ce que j'ai fait** : [`Source`] dit **d'où** vient la valeur, et chaque champ porte le
nom du panneau XML que Java écrit. Un écran complet demanderait qu'un côté client
sérialise ces six sources ; c'est une tranche à part, et la dire incomplète vaut mieux que
d'inventer un `Worker { name, level }` que personne ne remplit.

## 7. Deux écrans qui ne sont pas des huttes

**Ce que je voulais faire** : une ligne de table pour chaque bâtiment.

**Ce qui existe** : l'hôtel de ville ouvre `WindowMainPage`
(`BuildingTownHall.java:248`) et la boîte aux lettres `WindowPostBoxMain`
(`PostBox.java:125`). Ces deux fenêtres ne sont pas des `WindowHut*`, ne montrent aucun
métier et ne sont pas dans cette tranche.

**Ce que j'ai fait** : elles refusent par un nom — [`Refusal::NotAHutScreen`] — qui ne
porte **aucune** clé de traduction, puisque le joueur ne voit jamais ce refus : le bâtiment
s'ouvre normalement, simplement ailleurs.
