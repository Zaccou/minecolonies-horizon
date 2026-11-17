# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module **n'appelle aucune API de l'hôte** : c'est de la donnée et des règles, comme
`src/ui` et `src/ui_window_08`. Il n'utilise que `std`, `crate::ui` (ses types de panneau et
de refus), `crate::crafting::Teaching` (les cinq ids de `ModCraftingTypes`, pour que
`canLearn` soit une appartenance et pas une chaîne recopiée), `crate::network::
building_modules::{Face, MAX_FIELD_RANGE}` (l'ordre 2D de `Direction` et la largeur
maximale d'un champ, déjà nommés par le port) et `crate::channel::Registry` (dans les tests
seulement, pour prouver qu'aucun nom de message n'est inventé). Les manques ci-dessous sont
donc soit **du SDK**, soit **du port**.

## 0. Ce qui manque n'est pas ce qu'on croit : trois lecteurs existent déjà

C'est le premier point, parce qu'il change la façon de lire la suite. Le SDK **sait** lire
les trois valeurs les plus « impossibles » de ce groupe ; ce qui manque, c'est le **côté**
où ces fenêtres vivent.

| Ce que la fenêtre lit | Ce que le SDK a | Pourquoi ça ne suffit pas |
| --- | --- | --- |
| le biome du bloc (`WindowField.java:124-125`) | `level_client_biome(level, pos) -> Option<String>` (`host/levels.rs:130-133`), **côté client** | rien ne manque : un futur lecteur l'appellera |
| l'orientation du joueur, pour le nom relatif des flèches (`:354-366`) | `entity_rotation(entity) -> Option<[f32; 2]>`, yaw et pitch (`host/entities.rs:152-155`) | rien ne manque non plus : `Direction.orderedByNearest` est une comparaison que l'appelant refait avec le yaw |
| la sortie d'une recette (`:238`) | `level_recipe_for(…)` : « match and assemble through the engine's recipe matcher », grilles de 1 à 3 (`host/levels.rs:226-237`) | **côté serveur seulement** (`#[access(WorldOnly)]`), et ces quatre fenêtres sont côté client |
| les emplacements du conteneur (`container.slots`, `menu.craftMatrix`) | `menu_slot(menu, slot)`, `menu_slot_count` (`host/menus.rs:86-93`) | **côté serveur seulement** aussi ; l'hôte client n'a que `local_player` (`host/menus.rs:62-68`) |

Conséquence assumée : [`crate::ui_window_09::pane::Source`] nomme les six lecteurs, les
états **reçoivent** les valeurs (`crafting::State::done` prend la matrice, `grave::Layout`
prend le nombre d'emplacements) et aucune ligne de ce module ne traverse l'hôte. Un slice
serveur pourra calculer une sortie de recette ; un slice client ne le pourra pas tant que
`MenusClientHost` n'aura pas de lecteur d'emplacement.

## 1. Un widget pour un panneau nommé : une icône, un texte en lecture seule, une image

**Ce que je voulais faire** : dessiner les neuf panneaux de `windowfield.xml` — deux textes
écrits par la classe (`biome`, `climate`), un texte d'état (`current-farmer`), deux
`ItemIcon` (`current-seed`, `dir-center`) et **quatre nombres sur quatre boutons**
(`dir-resize-*`, `WindowField.java:312`).

**Ce qui existe** : `mod_api::client::widgets::Widget` a **exactement deux** variants —
`Button` et `TextField` (`crates/horizon-mod-vocab/src/client/widgets.rs:268-273`) — et
`Widgets` deux constructeurs, `add_button` (`:316`) et `add_text_field` (`:322`). Pas
d'icône d'objet, pas de texte en lecture seule, pas d'image, pas de grille, et aucun moyen
d'écrire un nombre **sur** un bouton : c'est `Button` qui porte son libellé, et le libellé
d'un `Button` de l'API est un texte fixe.

**Ce que j'ai fait** : [`plot::FIELDS`] dit quel panneau montre quoi et d'où vient la valeur,
[`plot::BUTTONS`] dit ce que le clic fait, et les quatre ids figurent dans les deux tables
parce que dans BlockUI un `ButtonImage` **est** un panneau. Ce qui ne peut pas être dit, et
qui manque vraiment, c'est le rendu et l'écriture d'un nombre sur un bouton existant.

## 2. Le « plafond de recettes » n'a pas de registre à interroger

**Ce que je voulais faire** : reproduire `module.canLearn(type)` exactement — une
appartenance du type à `getSupportedCraftingTypes()` (`CraftingModuleView.java:155-158`).

**Ce qui existe** : rien pour un **module client**. Le port n'a pas de vue de module cliente
(`src/ui/MANQUANT.md` le dit pour les treize écrans de hutte, et c'est la même chose ici) :
`crate::building::Module` décrit la clé, pas la liste des types enseignés.

**Ce que j'ai fait** : `State::teaches` est une tranche de `crate::crafting::Teaching` que
l'appelant fournit. Les cinq ids ne sont donc écrits qu'une fois dans tout le crate — dans
`crafting/kinds.rs:528-556` — et `can_learn` est bien une appartenance, ce qui est le fait
intéressant (la clé affichée parle d'un plafond, le test ne compte rien).

## 3. Un registre de langue, pour `I18n.exists`

**Ce que je voulais faire** : décider comme Java si le biome s'écrit par son nom traduit ou
par son chemin brut : `I18n.exists(biomeLangKey) ? Component.translatable(key) :
Component.literal(path)` (`WindowField.java:129`).

**Ce qui existe** : `host/client_text.rs` et `host/text.rs` portent le texte côté client,
mais rien ne répond à « cette clé existe-t-elle dans la langue du joueur ? » — `exists` est
une question sur la table de traduction chargée, pas une méthode d'hôte.

**Ce que j'ai fait** : [`plot::biome_text`] prend `translated: Option<&str>` ; `None` est
exactement la branche `!I18n.exists`. Le refus de deviner est le même que pour les recettes :
le port peut se tromper sur *qui* décide, pas sur *quoi* écrire quand on lui donne la
réponse.

## 4. Le registre des extensions de bâtiment, pour retrouver le champ

**Ce que je voulais faire** : retrouver le `FarmField` du bloc, comme `updateFarmField`
(`:224-244`) : `colonyView.getClientBuildingManager().getBuildingExtensions(otherField ->
otherField.getBuildingExtensionType().equals(BuildingExtensionRegistries.farmField.get()) &&
otherField.getPosition().equals(pos))`.

**Ce qui existe** : rien. Une extension de bâtiment (`IBuildingExtension`) n'est ni un
bâtiment, ni un module, ni un citoyen : le port n'a ni le registre
`BuildingExtensionRegistries` ni le gestionnaire client qui les range par position.

**Ce que j'ai fait** : `Snapshot::has_field` est un booléen que l'appelant pose, et
[`Refusal::NoFieldForBlock`] est le refus des deux appelants qui ne s'accordent pas — `setSeed`
qui ne fait rien (`:213`) et `selectSeed` qui déréférence sans tester (`:157`). Le test
`a_missing_field_refuses_the_seed_instead_of_dereferencing_it` tient la différence.

## 5. Les quatre textures au-delà de huit rangées n'existent que sur le disque

**Ce que je voulais faire** : rendre [`grave::texture`] complet, y compris la branche
`gui<nombre d'emplacements>.png` (`WindowGrave.java:150-160`).

**Ce qui existe** : les quatre fichiers `gui81`, `gui90`, `gui99`, `gui108` sous
`assets/minecolonies/textures/gui/` — et **rien au-delà de douze rangées**, alors que le
motif se forme pour n'importe quel nombre. La fonction est portée en entier et deux tests
disent que la tombe de ce mod (54 emplacements, six rangées) n'en prend jamais qu'un chemin :
un appelant qui passe 117 obtiendrait un nom de fichier qui n'existe pas, ce que Java ferait
aussi (une texture manquante, pas une erreur).

## 6. Ce qui manque vraiment, en une ligne

- **un widget** : icône d'objet, texte non éditable, nombre écrit sur un bouton (§1) ;
- **un lecteur d'emplacement côté client** (§0) : sans lui, aucune fenêtre de conteneur ne
  peut lire ses propres cases ni calculer une sortie de recette ;
- **une vue de module cliente** qui dise les types enseignés (§2), et **un registre de
  langue** qui réponde `exists` (§3) ;
- **le registre des extensions de bâtiment** côté client (§4).
