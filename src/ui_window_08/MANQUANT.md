# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : c'est de la donnée et des règles, comme
`src/ui`. Il n'utilise que `std`, `crate::ui` (ses types de panneau et de refus),
`crate::network::windows::InventoryType` (pour que la jointure
`OpenInventoryMessage` soit vérifiée par le compilateur) et `crate::ui::Effect`
(cité, pas construit). Les manques ci-dessous sont donc soit **du SDK**, soit **du port**.

## 1. Un panneau de conteneur : une grille d'emplacements

**Ce que je voulais faire** : décrire ce que les trois écrans de conteneur montrent —
quatre piles de brasserie (`WindowBrewingstandCrafting.java:116-122`), `n` rangées de neuf
pour l'inventaire d'un bâtiment (`WindowBuildingInventory.java:47`), l'inventaire et les
quatre emplacements d'armure d'un citoyen (`WindowCitizenInventory.java:82`, `:193-196`).

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que deux constructeurs,
`add_button` (`:316`) et `add_text_field` (`:322`), et un `draw` (`:394`). Pas de grille,
pas de case, pas d'icône d'objet, pas d'image, pas même un libellé. Les hooks d'entrée
existent (clic, relâchement, glisser, molette, touche, caractère,
`crates/horizon-mod-vocab/src/client/widgets.rs:1-10`), c'est le **rendu** qui manque.

**Ce que j'ai fait** : [`crate::ui_window_08::brewing::SLOTS`] et
[`crate::ui_window_08::container`] disent le nombre d'emplacements, leur rôle et l'ordre
du handler ; [`crate::ui_window_08::Pane`] reste vide pour ces trois fenêtres
(`BUILDING_FIELDS`, `CITIZEN_FIELDS`, `brewing::FIELDS`) parce qu'aucun de leurs panneaux
n'a d'id dans Java : ils sont ajoutés par le conteneur ou dessinés par `renderLabels`.

## 2. Le registre de brassage

**Ce que je voulais faire** : calculer la sortie d'une recette de brasserie comme Java —
`BrewingRecipeRegistry.getOutput(slots.get(3).getItem(), slots.get(0).getItem()).copy()`
(`WindowBrewingstandCrafting.java:122`) — et valider ce qu'un emplacement accepte
(`ContainerCraftingBrewingstand.java:159-166` : `isValidIngredient`, `isValidInput`).

**Ce qui existe** : rien. Ce registre est celui de **Forge** (`net.minecraftforge.common.
brewing`), il vit dans le jeu, et le SDK n'expose ni recettes de brassage, ni test
d'ingrédient, ni potion.

**Ce que j'ai fait** : [`crate::ui_window_08::brewing::State::done`] **reçoit** la sortie
(`output: Option<&'static str>`) au lieu de la calculer. Une sortie inconnue est le refus
nommé `NoBrewingRecipe`, qui est exactement la branche `!ItemStackUtils.isEmpty(...)` de
Java — le port ne peut donc pas se tromper sur *qui* décide, il peut seulement ne pas
savoir. Le rôle d'un emplacement (`brewing::Role`) dit la règle sans l'appliquer.

## 3. Le registre d'équipement, pour l'armure d'un citoyen

**Ce que je voulais faire** : décider si une armure entre dans l'un des quatre
emplacements, comme `Slot.mayPlace` (`ContainerCitizenInventory.java:255-270`) : bonne
partie du corps **et** `GuardGear.test(stack)` vrai.

**Ce qui existe** : rien côté port. `GuardGear` teste un `ModEquipmentTypes` (le registre
d'équipement de MineColonies) contre `stack.getItem()` : c'est un registre de contenu, pas
une table que le port tient.

**Ce que j'ai fait** : [`crate::ui_window_08::container::may_place`] modélise la **plage de
niveaux d'armure** que le niveau du bâtiment de travail autorise
(`WindowCitizenInventory.java:154-162`, les six branches de Java), et refuse par
`ArmourSlotLocked` quand la liste est vide — c'est-à-dire pour tout citoyen sans bâtiment
de travail, où Java refuse **toute** armure sans le dire. Le test `GuardGear.test`
lui-même manque ; il ne changerait que la borne haute, jamais le fait qu'un citoyen sans
bâtiment n'a pas d'armure.

## 4. Dessiner une entité dans une fenêtre

**Ce que je voulais faire** : l'aperçu du citoyen, qui suit la souris
(`WindowCitizenInventory.java:138-190` : `renderEntityInInventoryFollowsMouse`, trois
quaternions, un `EntityRenderDispatcher`).

**Ce qui existe** : rien. Le SDK a des renderers d'entité dans le monde
(`crates/horizon-mod-vocab/src/client/entity_renderers.rs`) et rien pour une entité dans une
interface.

**Ce que j'ai fait** : le refus `EntityNotLoaded` dit le cas où l'entité n'est pas dans le
niveau (Java ne dessine alors rien, `:139`), et l'aperçu lui-même n'est pas dans cette
tranche. `WindowCitizenInventory` n'a donc **aucun panneau** ici, pas même pour l'aperçu.

## 5. Un écran de conteneur nommé

**Ce que je voulais faire** : dire « cet écran, c'est le conteneur d'un rack de huit
rangées », pour que le client l'ouvre.

**Ce qui existe** : `ClientHost::mod_screen_open(mod_id)`
(`crates/horizon-mod-vocab/src/host/screens.rs:73`, `:2187`) ouvre **un** écran par mod ;
et un conteneur de mod n'a d'écran que par un **menu vanilla** de la table `MOD_MENUS`
(`crates/horizon-client-render/src/chunk/block_entity.rs:119-131`), qui s'arrête à
`minecraft:generic_9x6`, soit **54 emplacements**. C'est pour ça que la hutte du port
déclare `minecraft:generic_3x3` (`src/hut.rs`, `MENU_ID`) : un menu de contenu sans
`native_screen` est refermé par le client
(`crates/horizon-client/src/game/network/remote/mod.rs:9833-9846`).

**Ce que j'ai fait** : rien de plus que de le dire. Un rack de magasin
(`Constants.java:143` + `:163` : `27 + 5 × 9 = 72` emplacements) **n'a pas de menu
vanilla** : six rangées au maximum, et [`crate::ui_window_08::container::slots_shown`] dit
que le conteneur de Java, lui, les ajoute toutes. Un écran de rack à huit rangées demande
soit un `native_screen`, soit un menu de contenu déclaré — hors de cette tranche.

## 6. Les trois messages

**Ce que je voulais faire** : que les trois boutons fassent quelque chose —
`HireSpiesMessage` (`WindowsBarracksSpies.java:91`), `AddRemoveRecipeMessage`
(`WindowBrewingstandCrafting.java:127`), et l'ouverture d'un conteneur par
`OpenInventoryMessage` / `OpenCraftingGUIMessage`.

**Ce qui existe** : les **formats** de ces quatre messages sont portés
(`crate::network::colony_settings::HireSpies`,
`crate::network::building_modules::AddRemoveRecipe`,
`crate::network::windows::OpenInventory`,
`crate::network::buildings::OpenCraftingGUI`), et ils font l'aller-retour en test. Ce qui
manque est le **transport** : aucun `ServerHost` ne route un message d'un client vers un
serveur, comme `src/ui/MANQUANT.md` §5 le dit déjà.

**Ce que j'ai fait** : [`crate::ui_window_08::Effect::Sends`] porte le nom de la classe et
rien d'autre. Les refus serveur sont modélisés quand même — `RecipeRefusedByModule` est
précisément la branche `!module.addRecipe(token)` de
`AddRemoveRecipeMessage.java:177-181` — pour qu'un futur transport n'ait pas à réinventer
la règle.

## 7. L'état de raid et le drapeau des espions

**Ce que je voulais faire** : lire le raid et les espions sur la colonie, comme
`IColonyView.isRaiding()` et `areSpiesEnabled()` (`ColonyView.java:1302-1304`, `:1326-1329`).

**Ce qui existe** : `crate::colony::Colony` porte un id, un nom, un niveau, une dimension, un
centre, des citoyens, des bâtiments, des ordres et des permissions — **pas** d'état de raid,
pas de drapeau d'espions.

**Ce que j'ai fait** : [`crate::ui_window_08::spies::State`] reçoit les deux booléens de
l'appelant, et la règle du tick (`BuildingBarracks.onColonyTick`, `:136-159`) est portée en
entier : c'est elle qui rend le bouton presque inutile, et elle ne dépend que des deux
booléens et de l'or du rack.

## 8. Un `static` partagé entre deux fenêtres

**Ce que je voulais faire** : `public static WindowCitizenInventory activeCitizenInventory`
(`WindowCitizenInventory.java:67`), que le reste du client lit.

**Ce qui existe** : un `static mut` serait une erreur de compilation en édition 2024, et un
`static` à verrou dans un mod wasm n'a pas d'état de processus à partager entre deux écrans
d'un même client.

**Ce que j'ai fait** : [`crate::ui_window_08::container::Active`], une valeur que l'appelant
tient. Le **trou** de Java est reproduit exprès : la fermeture de la première fenêtre
efface la seconde (`:195` remet `null` sans regarder laquelle), et un test le fixe pour que
personne ne le « corrige » en silence.
