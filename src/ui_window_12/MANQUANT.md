# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Comme `src/ui`, ce module n'appelle **aucune** API de l'hôte : il ne dépend que de `std`,
de `mod_api` (pour `serde_json`, dans un test), de `crate::ui`, `crate::requests`,
`crate::crafting`, `crate::inventory`, `crate::jobs`, `crate::colony` et
`crate::network::building_modules`. Rien n'a été ajouté à `crates/`. Les manques ci-dessous
sont donc soit **du SDK**, soit **du port**, et chacun est nommé plutôt que contourné.

## 1. Une source qui nomme vingt-huit clés de module

**Ce que je voulais faire** : dire d'où vient la donnée d'une ligne de recette. Elle vient
d'un module de la hutte, dont la clé est `farmer_work`, `baker_craft`, `smelter_orebreak`…
— **28** clés, une par module d'artisanat (`core/colony/buildings/**`, un `ModuleProducer`
`…CraftingModuleView::new` chacune), et il n'en existe aucune qui les réunisse.

**Ce qui existe** : `crate::ui::field::Source::Module(&'static str)`
(`src/ui/field.rs:120-125`) — **une** clé, fixée à la compilation. Il n'y a ni
`Source::AnyModule`, ni `Source::ModuleOf(&'static [&'static str])`.

**Ce que j'ai fait** : [`recipes::MODULE`] est la première des 28, un test vérifie qu'elle
est bien dans [`recipes::MODULE_KEYS`], et les 28 sont dans le doc. Ce n'est pas une clé
inventée — c'est une des vraies — mais c'en est **une** pour en désigner 28. Le correctif
est une variante dans `src/ui`, donc hors de mon dossier.

## 2. Ce que la requête est

Trois manques dans `crate::requests::Request` (`src/requests/request.rs`), et chacun coûte
une branche de `WindowHutRequestTaskModule` :

| Ce que Java lit | Ce que le port a | Ce que j'ai fait |
| --- | --- | --- |
| `request.getRequest() instanceof IDeliverymanRequestable` (`:109`) | rien : ni type de requétable, ni priorité de livraison ; le tag `type` (`:56`) est le **bâtiment demandeur** | [`tasks::Kind`], que l'appelant fournit |
| `request.getRequester().getRequesterDisplayName(...)` (`:82`) | `Requester.building` est un **slug** (`minecolonies:builder`), pas un nom affiché | le slug est écrit tel quel ; [`tasks::requester_name`] le dit |
| `request.getParent()` (`:65`) | rien : les chaînes de requêtes ne sont pas portées (`src/requests/mod.rs`) | [`tasks::Row::of`] reçoit le parent **déjà résolu**, et [`tasks::requester_text`] applique la règle d'arrêt (même position) |
| `getShortDisplayString()` / `getDisplayPrefix()` / `getDisplayIcon()` (`:101`, `:106`, `:116`) | un `Request` a un item, un compte et un état | [`tasks::Detail`] : c'est l'appelant qui fournit l'icône et le texte |

Le préfixe de pile (`IStackBasedTask.getDisplayPrefix`) et le compte d'affichage
(`getDisplayCount`) sont, eux aussi, des méthodes d'interface qui n'ont pas d'équivalent :
`crate::crafting::CrafterRecipe` décrit un fichier, pas une tâche en cours.

## 3. Le fait « puits principal » d'un ordre de travail

**Ce que je voulais faire** : décider si `repair` est grisé, ce qui est
`wo.getDisplayName().getString().contains("main") && wo.getLocation().getY() == depth`
(`MinerLevelManagementModuleView.java:104-115`).

**Ce qui existe** : `crate::colony::WorkOrder` (`src/colony/ordre.rs:109-126`) porte
`building`, `at`, `work`, `target_level`, `priority`, `state` — **pas** de nom de schéma ni
de clé de traduction. La chaîne « main » vient du nom du schéma
(`infrastructure/mineshafts/minermainshaft`, `MineNode.java:390-391`) passé à l'ordre
(`AbstractWorkOrder.java:338-352`), que le record du port n'a pas.

**Ce que j'ai fait** : [`miner::MineOrder`] porte `at` et `main_shaft: bool`, et c'est
l'appelant qui remplit le booléen. Ajouter `structure: String` à `WorkOrder` serait une
modification de `src/colony/`, hors de mon dossier.

## 4. Une `Gate` qui ne soit pas une advancement

**Ce que je voulais faire** : dire que l'onglet de la mine **n'est pas dessiné** tant qu'un
carrier travaille dans la hutte (`MinerLevelManagementModuleView.java:86-96`).

**Ce qui existe** : `crate::ui::screen::Gate` a **une** variante,
`Advancement(&'static str)` (`src/ui/screen.rs:76-81`), celle du builder. Rien pour une
visibilité qui dépend d'une **autre** vue de module.

**Ce que j'ai fait** : [`miner::visible`] est une fonction, pas un `Gate`, et la doc de
`src/ui` ne peut donc pas la lire. Une variante `Gate::NoQuarrier` dans `src/ui` serait la
place juste.

## 5. Le stockage des recettes enseignées

**Ce que je voulais faire** : porter `IRecipeStorage` — la sortie **et** ses variantes
d'affichage, les neuf entrées, l'outil, le bloc intermédiaire, `getRecipeSource()` et
`getRecipeType()`.

**Ce qui existe** : `crate::crafting::CrafterRecipe` (`src/crafting/crafter.rs:131-168`)
pour un fichier de `crafterrecipes/` — inputs, `result`, `count`, `tool`, `intermediate`,
`alternate_output` — et rien pour une recette **enseignée** (une recette de grille que le
joueur donne à l'artisan, donc avec un `recipeSource` non nul et pas de fichier).

**Ce que j'ai fait** : [`recipes::Row`] est la ligne d'affichage, et
[`recipes::Row::from_crafter`] la remplit depuis un `CrafterRecipe` — l'autre moitié, la
recette de grille, n'a pas de modèle et se remplit à la main. `getOutputDisplayStacks`
appartient au **type** de recette : j'ai pris `result` + `alternate_output`, ce qui est la
lecture la plus proche, et la doc le dit.

## 6. Le citoyen n'a pas de bâtiment de travail

**Ce que je voulais faire** : retrouver la tour d'un garde par
`citizen.getWorkBuilding()` (`WindowMineGuardModule.java:57`, `:156`).

**Ce qui existe** : `crate::colony::Citizen { id, name }` (`src/colony/colonie.rs:61-64`).
Aucune affectation.

**Ce que j'ai fait** : [`guards::Guards`] reçoit les tours avec leurs gardes déjà dedans, et
une ligne porte l'index de sa tour. C'est la même information, lue d'un autre côté.

## 7. Une liste, et des panneaux qu'on puisse vider

**Ce que je voulais faire** : décrire des listes de lignes réutilisées, comme
`ScrollingList`.

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que `add_button` et
`add_text_field` — `src/ui/MANQUANT.md` §1 le dit en détail, et ce module ne le répète pas.

**Ce que ce module ajoute au constat** : trois comportements de Java reposent sur la
**réutilisation** des panneaux de ligne, et un widget qui détruit puis recrée ses lignes ne
les reproduira pas — il les *corrigera* :

- la priorité d'une tâche qui n'est pas un livreur garde le nombre de la ligne précédente
  (`WindowHutRequestTaskModule.java:109-113`) ;
- le nom d'un garde non chargé garde le nom de la ligne précédente
  (`WindowMineGuardModule.java:185-189`) ;
- le libellé d'un bouton de garde n'est écrit que dans deux branches sur trois
  (`:161-182`).

C'est pourquoi [`tasks::Row::priority`] est un `Option` et
[`guards::Row::name`] aussi : le rendu doit **vider** ces panneaux, et le port le dit au
lieu de laisser un widget les remplir comme Java.

## 8. Un envoi, et de quoi remplir six messages

**Ce que je voulais faire** : qu'un clic fasse quelque chose. Les six messages de ces
quatre fenêtres sont **tous** déjà modélisés dans `src/network/building_modules.rs` :
`MinerSetLevel`, `MinerRepairLevel`, `GuardSetMinePos`, `ToggleRecipe`, `AddRemoveRecipe`,
`ChangeRecipePriority`.

**Ce qui existe** : les modèles, et aucune route client → serveur
(`src/ui/MANQUANT.md` §5 ; `ServerHost` n'a pas de « message d'un client »).

**Ce que j'ai fait** : [`super::Button::sends`] rend le **nom** de la classe, et rien n'est
envoyé. Les remplir demanderait trois choses que l'écran n'a pas : l'identifiant de colonie,
la dimension, et la position de la hutte — le `envelope` de chaque message
(`src/network/building_modules.rs:55-90`).

## 9. Le format des positions d'une infobulle

**Ce que je voulais faire** : l'infobulle de la ligne de tâche, deux positions
(`WindowHutRequestTaskModule.java:88-89`), dans le format de `BlockPos.toShortString()`.

**Ce qui existe** : rien. `crate::requests::Requester.at` est un `[i32; 3]`.

**Ce que j'ai fait** : [`tasks::short_string`] écrit `x, y, z` — la forme de `Vec3i` en
1.20.1 — sans passer par le jeu, donc sans garantie sur la locale. Ce n'est pas une règle
de jeu, c'est un format d'affichage : le corriger ne changerait aucun test de comportement.
