# MANQUANT — ce que les écrans client auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : `src/client_gui` est de la donnée et de la
règle pures, comme `src/ui`. Il n'utilise que `std` et deux fonctions déjà écrites
(`crate::ui::field::percent`, `crate::ui::table::module_screen`). Rien n'a été ajouté à
`.horizon/sdk/`. Les manques ci-dessous sont donc soit **du SDK**, soit **du port** — et
chacun est nommé plutôt que contourné.

## 1. Un fichier Java absent du checkout

**Ce que je voulais faire** : décrire le huitième conteneur, celui du champ de ferme
(`ContainerField`), comme les sept autres.

**Ce qui existe** : `api/inventory/container/ContainerField.java` fait **0 octet** dans ce
checkout — le fichier est là, vide, et **rien ne le référence** : `grep -rn ContainerField`
sur `src/main/java` ne rend aucune occurrence, et `core/client/gui/containers/WindowField.java`
n'importe aucun conteneur (il n'ouvre aucune fenêtre de coffre). Le côté serveur du champ
passe donc par autre chose, hors de ce dossier.

**Ce que j'ai fait** : `containers::EMPTY_CONTAINER_FILE` porte le nom du fichier, et
`containers::MENUS` n'a que sept lignes. Un test le compte
(`seven_menus_are_readable_and_the_eighth_file_is_empty`). Il n'y a **pas** de huitième
ligne inventée.

## 2. Un widget de liste

**Ce que je voulais faire** : rendre les listes que ce module décrit — les quinze fenêtres
de module, la page citoyens de l'hôtel de ville, les trois pages du journal de quêtes.

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que `add_button` et
`add_text_field`, plus un `draw`. Pas de liste, pas de ligne, pas de barre de défilement,
pas de libellé. `src/ui/MANQUANT.md` §1 dit déjà la même chose pour `src/ui` ; c'est le
même manque, vu d'un autre module.

**Ce que j'ai fait** : la règle de liste est une fonction pure (`item_list_matches`,
`entity_list_matches`, `allowed_first`, `switch_label`, `work_order_visible`) et la ligne
est décrite par ses identifiants de panneaux. Aucun `Widget` n'est construit.

## 3. Une mesure de texte

**Ce que je voulais faire** : décider si la ligne d'une quête a besoin d'une infobulle,
comme Java le fait — `label.getRenderedTextWidth() > label.getWidth()`
(`WindowQuestLogInProgressQuestQuestModule.java:80-84`).

**Ce qui existe** : la règle est **déjà portée**, avec ses deux nombres en arguments, par
`ui_window_13::quest_log::tooltip` (`src/ui_window_13/quest_log.rs:406`). Ce module ne la
réécrit pas : il vérifie dans un test que les deux modules tombent d'accord sur la borne.

Ce qui manque reste le **rendu** : le SDK n'a pas de panneau de texte, donc pas de largeur
mesurée, et un appelant qui n'a pas la mesure ne peut pas l'appeler par erreur.

## 4. Une barre latérale d'onglets

**Ce que je voulais faire** : décrire les onglets d'un citoyen comme Java les construit —
cinq boutons toujours là plus deux conditionnels (`AbstractWindowCitizen.java:41-87`).

**Ce qui existe** : `TabsWindowModule` (`core/client/gui/modules/TabsWindowModule.java`) est
du BlockUI : il crée des `ButtonImage` à la main, avec des décalages en pixels
(`TAB_X_OFFSET = 12`, `TAB_Y_SPACING = 2`, icône 20 × 20). Le SDK n'a ni onglet ni barre
latérale.

**Ce que j'ai fait** : `citizen::tabs(workplace_is_building, workplace_is_library, debug)`
rend la **liste ordonnée** des onglets, et `Tab::id`/`icon`/`tip`/`effect` portent les
quatre choses qu'un onglet sait. Les décalages en pixels restent dans le XML.

## 5. Une valeur d'état de module

**Ce que je voulais faire** : un type qui réponde « ce module est-il en train de faire
quelque chose, à quel point, et pourquoi refuse-t-il ? ».

**Ce qui existe** : rien. Java n'a pas ce type : la progression est écrite à la main
(`WarehouseOptionsModuleWindow.java:118`), le refus est un `disable()`
(`:125`) ou une couleur (`:124-129`), et le niveau est celui du module ou celui du bâtiment
selon la ligne lue. Côté port, `crate::building::Module` ne porte qu'une clé et un niveau
(`src/building/instance.rs:78-100`), et `crate::hut::State` ne porte pas de module du tout.

**Ce que j'ai fait** : `modules::State` a **trois champs séparés** (niveau, progression,
refus) et `modules::Refusal` nomme les refus que Java ne nomme pas. Ce n'est pas un
`ModuleView` : c'est ce qu'une fenêtre consomme d'un `ModuleView`, et un appelant doit le
remplir lui-même.

**Ce qui bloque la suite** : la jointure inverse — « quel module ce bâtiment porte-t-il ? »
— est `ModBuildingsInitializer` côté Java, et le port ne l'a pas. C'est déjà le manque §3
de `src/ui/MANQUANT.md`, vu d'un autre côté.

## 6. Le registre de brassage

**Ce que je voulais faire** : décider seul si une pile est un ingrédient ou une potion,
pour reproduire `ContainerCraftingBrewingstand.quickMoveStack` sans appelant.

**Ce qui existe** : `BrewingRecipeRegistry.isValidIngredient` / `isValidInput`
(`:341`, `:346`), deux fonctions de Forge sur un registre de recettes vanilla. Le port n'a
pas ce registre.

**Ce que j'ai fait** : `containers::brewing_shift(index, valid_ingredient, valid_input,
count)` prend les deux prédicats en arguments. La règle — dont la branche destructrice —
est portée ; la classification des piles ne l'est pas.

## 7. Le contenu d'une carte

**Ce que je voulais faire** : rendre une tuile de carte à partir des données sauvegardées.

**Ce qui existe** : `MapItemSavedData.colors` (16 384 octets) et
`MapColor.getColorFromPackedId`, qui est du code vanilla. Le port n'a ni l'un ni l'autre :
`crate::network::views` ne porte pas d'image.

**Ce que j'ai fait** : `map_data::Map` valide **la taille** du tableau (16 384 octets) et
rend l'octet emballé d'un pixel ; la conversion en couleur est laissée à l'appelant, avec
la ligne Java qui la fait. Un tableau de la mauvaise taille est refusé par
`map.refusal.wrong_pixels`, jamais tronqué en silence.

## 8. Le message d'un citoyen vers son bâtiment

**Ce que je voulais faire** : qu'un clic sur « retirer une requête » fasse quelque chose.

**Ce qui existe** : `TransferItemsToCitizenRequestMessage`
(`RequestWindowCitizen.java:234`), `RecallSingleCitizenMessage` (`WindowCitizenPage.java:166`),
`AdjustSkillCitizenMessage` (`MainWindowCitizen.java:109`), `GiveToolMessage`
(`ToolModuleWindow.java:37`) — des messages `core/network/messages/server/`. Le port n'a
**aucune** route client → serveur : `ServerHost` n'en déclare pas, et `src/ui/MANQUANT.md`
§5 dit déjà que seize actions de hutte portent un nom de classe et rien d'autre.

**Ce que j'ai fait** : les effets sont nommés par la classe Java
(`citizen::Effect::Sends`, `citizen::Effect::Opens`), et rien n'est envoyé. Un appelant qui
lit `Effect::Sends("OpenInventoryMessage")` sait exactement ce qui manque.
