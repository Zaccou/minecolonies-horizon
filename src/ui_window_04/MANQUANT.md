# MANQUANT — ce que ces quatre fenêtres demandent et que le port n'a pas

`src/ui_window_04` n'appelle **aucune API de l'hôte** : comme `src/ui`, c'est de la donnée pure, et
la seule couche de langue qu'il touche est un paramètre `impl Fn(&str) -> Option<String>` qu'un test
remplit avec un dictionnaire. Rien n'a été ajouté à `crates/`. Les manques ci-dessous sont donc soit
**du SDK**, soit **du port**, et chacun est nommé plutôt que contourné.

## 1. Un widget de texte et un widget d'image

**Ce que je voulais faire** : poser les panneaux de ces quatre fenêtres. Ce ne sont pas des listes
(le manque n° 1 de `src/ui/MANQUANT.md`), ce sont des **textes** et une **image** : les trois
phrases du guide (`windowhutguide.xml:7-9`), son fond 960×540 (`:5`), le titre de page et le corps
multiligne de `WindowInfo` (`WindowInfo.java:52-66`), son compteur `3/5`
(`AbstractWindowSkeleton.java:249`), le titre du minimum (`AbstractBuildingMainWindow.java:171-176`)
et le libellé du champ de renommage (`windowhutnameentry.xml:4-5`).

**Ce qui existe** : `Widgets` (`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) a **deux**
constructeurs, `add_button` et `add_text_field`. Il n'y a ni `Text`, ni `Image`, ni libellé, ni
compteur — donc aucun des panneaux ci-dessus n'a de destination. `Button::new(rect, Text)`
(`:82-93`) prend bien un texte, mais c'est le libellé **d'un bouton**, et un bouton qui ne fait rien
n'est pas un panneau.

**Ce que j'ai fait** : décrire chaque panneau comme un [`crate::ui::field::Field`] (id, valeur,
origine), et m'arrêter là. Aucun pont vers `Widgets` n'est possible sans le widget.

## 2. Un `exists` de clé de langue, sans allocation

**Ce que je voulais faire** : reproduire la boucle qui compte les pages de `WindowInfo`
(`WindowInfo.java:42-47`) : `if (!I18n.exists(prefix + i)) break;`.

**Ce qui existe** : `ConfigClientHost::translate(&self, key) -> Option<String>`
(`crates/horizon-mod-vocab/src/host/config.rs:176-179`). Il rend le texte **ou** `None`, ce qui suffit
logiquement — `I18n.exists` n'est que `translate(...).is_some()`. Le prix est une `String` allouée
par page sondée, et il n'y a pas d'autre porte.

**Ce que j'ai fait** : [`crate::ui_window_04::info::page_count`] prend la recherche en paramètre
(`impl Fn(&str) -> Option<String>`), donc l'appelant réel branche `translate` et un test branche un
dictionnaire troué. Deux conséquences assumées :

- le plafond [`crate::ui_window_04::info::PAGE_CEILING`] (64) borne une boucle que Java laisse
  **infinie** (`for (int i = 0;; i++)`) ; un pack réel s'arrête à cinq (`miner`), donc le plafond ne
  coupe rien, il empêche seulement de ne pas finir ;
- [`crate::ui_window_04::info::MANUALS`] est un **relevé** de `manual_en_us.json` (18 bâtiments,
  69 pages) et non une lecture : un pack qui ajoute des pages ne le met pas à jour. C'est le seul
  endroit de ce module où une donnée de langue est écrite en dur, et la ligne du fichier est dans
  chaque ligne de la table pour qu'une relecture la retrouve.

## 3. Une valeur qui vient du XML, et non du modèle

**Ce que je voulais faire** : dire d'où vient chaque panneau du guide. Ils viennent **du fichier de
mise en page** : `text="$(clé)"` est traduit par BlockUI au chargement (`windowhutguide.xml:7-9`) et
`source="…png"` nomme une texture (`:5`). Aucune ligne de `WindowHutGuide` ne les écrit.

**Ce qui existe** : `crate::ui::field::Source`, neuf variants, **tous** une lecture de modèle
(`Building`, `Colony`, `Hut`, `Module`, `Citizen`, `PlayerInventory`, `WorkOrder`, `Request`,
`Advancement`). Aucun ne dit « le fichier le porte ».

**Ce que j'ai fait** : [`crate::ui_window_04::guide::Origin`], deux variants (`XmlLiteral`,
`XmlImage`), et [`crate::ui_window_04::guide::PANE_ORIGINS`] qui donne l'origine réelle des quatre
panneaux. C'est aussi la raison du désaccord documenté avec `src/ui`, qui les déclare
`Source::Advancement` (`ui/screen.rs`) : l'avancement ne remplit rien, il décide de l'ouverture.

Le même manque touche `WindowInfo` : ses deux panneaux à valeur sont du **texte de langue**, et je
les ai typés `Source::Building` parce que la clé se dérive du type de bâtiment
(`WindowInfo.java:33`). C'est le moins faux des neuf, pas le bon.

## 4. Le nom personnalisé d'une hutte

**Ce que je voulais faire** : l'état initial de la fenêtre de renommage — `prefill` lit
`building.getCustomName()` (`WindowHutNameEntry.java:49`) — et le titre d'une fenêtre de bâtiment,
qui lit `getBuildingDisplayName()` (`IBuildingView.java:83-86`).

**Ce qui existe** : `crate::hut::State` porte `style`, `mirror`, `blueprint`, `level`, `hired`,
`colony`, `main` ; `crate::building::Building` porte `kind`, `at`, `level`, `built`, `deconstructed`,
`parent`, `modules`, `worker`. **Aucun des deux ne porte de nom.**

**Ce que j'ai fait** : [`crate::ui_window_04::rename::prefill`] et
[`crate::ui_window_04::minimum::title_text`] prennent le nom en argument et ne le cherchent pas. Un
appelant réel n'existe pas encore : il faudra une clé `name` dans `hut::State`, ou un champ dans
`building::Building`, et ce n'est pas mon dossier.

## 5. Le nom d'affichage d'une commande de travail

**Ce que je voulais faire** : dire ce que le serveur fait d'un renommage. Il ne garde pas seulement
le nom : il **renomme toutes les commandes de travail en attente** de la hutte
(`AbstractBuilding.java:752-763` → `WorkOrderBuilding.setCustomName`, `:118-133`), y compris la
ligne du bâtiment parent.

**Ce qui existe** : `crate::colony::WorkOrder` porte `id`, `building`, `at`, `work`, `target_level`,
`priority`, `state`. **Pas de nom**, ni de nom de parent.

**Ce que j'ai fait** : [`crate::ui_window_04::rename::SERVER`] est une description
(`renames_work_orders: true`), pas une opération : le port n'a pas de champ à écrire. Le jour où
`WorkOrder` portera un `custom_name`, cette constante deviendra la règle de la propagation et non un
commentaire.

## 6. Un client qui envoie un message

**Ce que je voulais faire** : que les clics aboutissent. Trois messages Java traversent ces quatre
fenêtres : `HutRenameMessage` (`HutRenameMessage.java`, soixante lignes), `ClickGuiButtonTriggerMessage`
(`:12-69`, le clic qui accorde l'avancement du guide) et `OpenGuiWindowTriggerMessage`
(`AbstractWindowSkeleton.java:96`, l'annonce d'ouverture de **chaque** fenêtre du skeleton).

**Ce qui existe** : `crate::network::windows` décode déjà `ClickGuiButtonTrigger { button_id,
resource }` et `OpenGuiWindowTrigger { resource }` — le port sait lire ces deux messages et
[`crate::ui_window_04::guide::trigger`] construit le premier. Et il n'y a **pas de route
client → serveur** : `ServerHost` n'a aucune méthode « reçois ce message », et `src/ui/MANQUANT.md`
§5 le dit déjà pour les seize actions des écrans de huttes. `HutRenameMessage`, lui, n'est même pas
décodé dans `src/network/`.

**Ce que j'ai fait** : nommer les classes et les construire quand le type existe, sans prétendre à
un envoi. Conséquence à garder en tête : sans `ClickGuiButtonTriggerMessage`, l'avancement
`check_out_guide` ne peut pas être accordé par un clic, donc le guide s'ouvrirait à chaque visite de
l'atelier du builder.

## 7. La progression d'un avancement, côté client

**Ce que je voulais faire** : la porte du guide telle que Java la lit —
`Minecraft.getInstance().player.connection.getAdvancements()…isDone()`
(`WindowHutBuilderModule.java:57-58`), sur le client.

**Ce qui existe** : côté **serveur**, `advancement(id)` (`host/advancements.rs:8-12`) et
`player_trigger_criterion(player, trigger, test)` (`host/players.rs:735-744`), qui sont de quoi
accorder et interroger ; côté **client**, rien. Et le port a son propre système
(`crate::advancements` : `TREE`, `ColonyState`, `Event`) qui décrit **ce qu'une colonie débloque**,
pas la liste d'avancements d'un joueur.

**Ce que j'ai fait** : [`crate::ui_window_04::guide::should_open`] prend le booléen
(`advancement_done`) et ne le lit pas. C'est la signature honnête d'une information que le côté
client ne peut pas obtenir, et [`crate::ui_window_04::guide::clicked`] fournit l'événement qu'un
côté serveur enregistrerait.
