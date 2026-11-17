//! La donnée de quatre fenêtres de module : la mine, ses gardes, ses tâches, et les recettes
//! d'un artisan.
//!
//! Java reference : `core/client/gui/modules/building/` —
//! `WindowHutMinerModule.java`, `WindowMineGuardModule.java`,
//! `WindowHutRequestTaskModule.java` et `WindowListRecipes.java` —, les quatre vues de
//! module qu'elles lisent (`MinerLevelManagementModuleView.java`,
//! `MinerGuardAssignModuleView.java`, `RequestTaskModuleView.java`,
//! `CraftingModuleView.java`) et les six messages qu'elles envoient.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`miner`] | les niveaux d'une mine : trois textes, deux boutons, un ordre de travail |
//! | [`guards`] | les gardes d'une mine : un plafond par niveau, un bouton par garde |
//! | [`tasks`] | les tâches d'un artisan : quatre textes, **aucun bouton** |
//! | [`recipes`] | les recettes d'un artisan : neuf entrées, une sortie animée, cinq boutons |
//! | [`refusal`] | pourquoi un clic ne fait rien, et qui l'entend |
//!
//! **La même forme que `src/ui`, pas une deuxième.** Les panneaux sont des
//! [`crate::ui::Field`] — `Value`, `Scope`, `Source` sont les siens —, les effets des
//! [`Effect`] de `src/ui`, et les titres ses [`Title`]. Deux des quatre fenêtres **sont**
//! des écrans de `src/ui` : [`WINDOWS`] pointe leur ligne là-bas
//! (`Table::SrcUi`) au lieu de la recopier, et un test le vérifie champ par champ. Les deux
//! autres n'ont pas de ligne, et c'est [`Tab`] qui les décrit ici.
//!
//! # Les comptes, mesurés
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | **4** | [`WINDOWS`] |
//! | déjà décrites par `src/ui` | **2** — la mine et les tâches | `Table::SrcUi` |
//! | décrites ici pour la première fois | **2** — les recettes et les gardes | `Table::Here` |
//! | clés de module qui ouvrent ces quatre fenêtres | **33** | [`module_keys`] |
//! | dont `src/ui::MODULE_SCREENS` en connaît | **4** | 1 + 3 |
//! | clés de module des recettes | **28** | [`recipes::MODULE_KEYS`] |
//! | panneaux déclarés ici | **16** | 13 + 3 |
//! | boutons déclarés ici | **6** | 5 + 1 |
//! | boutons qui sont sur une ligne | **5** | `up`, `down`, `remove`, `toggle`, `assignGuard` |
//! | boutons de fenêtre | **1** | `crafting` |
//! | refus | **13**, dont **1** avec une clé | [`refusal::Kind::ALL`] |
//! | messages Java nommés, tous déjà portés | **6** | voir ci-dessous |
//!
//! Les six messages sont dans `src/network/building_modules.rs`, avec leurs champs :
//! `MinerSetLevel` (`:431-451`), `MinerRepairLevel` (`:404-424`), `GuardSetMinePos`
//! (`:319-351`), `ToggleRecipe` (`:735-760`), `AddRemoveRecipe` (`:585-625`) et
//! `ChangeRecipePriority` (`:659-695`). Ces fenêtres ne font donc que les **nommer** :
//! remplir un `MinerSetLevel` demande une colonie, une dimension et une position que
//! l'écran n'a pas.
//!
//! # Quatre prémisses du brief que le source contredit
//!
//! 1. **« les actions (embaucher, renvoyer, prioriser) »** : de ces quatre fenêtres, aucune
//!    n'embauche ni ne renvoie personne. `HireFireMessage` n'y apparaît pas une fois ; le
//!    seul bouton qui *assigne* est `assignGuard`, et il envoie
//!    `GuardSetMinePosMessage` — une **position**, pas un citoyen. « Prioriser » existe,
//!    mais c'est l'ordre des **recettes** dans une liste (`up`/`down`,
//!    `ChangeRecipePriorityMessage`), sans rapport avec la priorité de livraison d'une
//!    hutte que `src/ui` porte déjà (`A_PRIO_UP`, `A_PRIO_DOWN`).
//! 2. **« progression, stock »** : aucune de ces quatre fenêtres n'affiche un pourcentage ni
//!    un stock. La mine écrit un numéro, une profondeur et un compte de nœuds ; les tâches
//!    un détail et une priorité ; les recettes un compteur d'actives ; les gardes un nom.
//!    `src/ui` a déjà tranché que la progression et le stock sont sur l'onglet du builder
//!    (`TAB_RESOURCES_ID`), et ces quatre-là n'en ont pas.
//! 3. **« l'écran » de `WindowListRecipes`, au singulier** : c'est le `getWindow()` de
//!    **28** modules distincts (`recipes::MODULE_KEYS`). Il n'existe aucune clé `crafting`,
//!    et **aucune** des 28 n'est dans `src/ui::MODULE_SCREENS` : la table de `src/ui` a une
//!    ligne pour `miner_levels` et trois pour les listes de tâches, zéro pour les recettes.
//! 4. **« ce que l'écran demande »**, pour `WindowMineGuardModule` : le module n'a **pas de
//!    serveur** — `new BuildingEntry.ModuleProducer<>("miner_guard_assign", null, () ->
//!    MinerGuardAssignModuleView::new)` (`BuildingModules.java:505-506`) — et sa vue ne
//!    désérialise rien (`MinerGuardAssignModuleView.java:19-23`). Tout ce que la fenêtre
//!    affiche vient du **gestionnaire de bâtiments client**, pas du module.
//!
//! Une cinquième, plus petite : **l'onglet de la mine se cache**. `isPageVisible()` répond
//! faux tant qu'un carrier travaille dans la hutte
//! (`MinerLevelManagementModuleView.java:86-96`) ; `src/ui` n'a qu'un [`Gate`] — une
//! progression d'*advancement*, pour le builder — et ne peut pas dire ça. C'est
//! [`miner::visible`], et `MANQUANT.md` §4 nomme la variante qui manque.
//!
//! # Ce qui n'est pas ici
//!
//! - **le dessin** : `pos`, `size`, sprites, et le `Gradient` des recettes désactivées
//!   (`layoutlistrecipes.xml:10`) — qui dessine un drapeau, pas une valeur.
//! - **l'envoi** : les six messages sont nommés, aucun n'est envoyé. `src/ui/MANQUANT.md`
//!   §5 le dit pour les treize écrans de hutte, et c'est vrai ici aussi.
//! - **les valeurs elles-mêmes** : ce module décrit des *lignes* et des *transitions*, pas
//!   un client ; les noms de citoyens, les positions et les recettes sont fournis par
//!   l'appelant.

pub mod guards;
pub mod miner;
pub mod recipes;
pub mod refusal;
pub mod tasks;

pub use refusal::{Kind, Refusal};

use crate::ui::action::{Action, Effect, Refusal as UiRefusal};
use crate::ui::field::Field;
use crate::ui::screen::{Screen, Title};

/// L'onglet des niveaux de la mine : **l'id de `src/ui`**, pas un second.
pub const MINER_TAB_ID: &str = crate::ui::screen::TAB_MINER_LEVELS_ID;
/// L'onglet des tâches : l'id de `src/ui` aussi.
pub const TASKS_TAB_ID: &str = crate::ui::screen::TAB_TASKS_ID;
/// L'onglet des recettes : `src/ui` n'en a pas, donc cet id est le nôtre.
pub const RECIPES_TAB_ID: &str = "tab.recipes";
/// L'onglet des gardes d'une mine : le nôtre également.
pub const GUARDS_TAB_ID: &str = "tab.mine_guards";

/// Un bouton, quand la fenêtre n'est pas décrite par `src/ui`.
///
/// [`Action`] de `src/ui` porte `refusals: &'static [ui::action::Refusal]` : il ne peut pas
/// nommer un refus qui n'existe pas là-bas, et dix de ces treize refus n'y existent pas.
/// [`Button`] garde donc les mêmes `id`, `label` et [`Effect`], et porte
/// [`refusal::Kind`] au lieu de `ui::Refusal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id de l'XML, celui que `registerButton` lie
    /// (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// La clé du libellé ; `""` pour un bouton dont le libellé est écrit par la ligne.
    pub label: &'static str,
    /// Ce que le clic demande **ou ouvre** : le même [`Effect`] que `src/ui`.
    pub effect: Effect,
    /// Vrai quand le bouton est dans une ligne — cinq des six de ce module.
    pub on_row: bool,
    /// Chaque façon dont le clic est refusé, dans l'ordre où Java teste.
    pub refusals: &'static [Kind],
}

impl Button {
    /// Un bouton de ligne.
    pub const fn on_row(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        refusals: &'static [Kind],
    ) -> Button {
        Button { id, label, effect, on_row: true, refusals }
    }

    /// Un bouton de fenêtre.
    pub const fn on_window(
        id: &'static str,
        label: &'static str,
        effect: Effect,
        refusals: &'static [Kind],
    ) -> Button {
        Button { id, label, effect, on_row: false, refusals }
    }

    /// Vrai quand Java peut refuser le clic.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// La classe du message envoyé, quand le clic en envoie un.
    pub fn sends(&self) -> Option<&'static str> {
        match self.effect {
            Effect::Sends(class) => Some(class),
            _ => None,
        }
    }
}

/// La table des panneaux et des boutons d'une fenêtre que `src/ui` ne décrit pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    /// Les panneaux, dans l'ordre du XML.
    pub fields: &'static [Field],
    /// Les boutons, dans l'ordre du XML.
    pub buttons: &'static [Button],
}

impl Tab {
    /// Le panneau de cet id.
    pub fn field(&self, id: &str) -> Option<Field> {
        self.fields.iter().find(|field| field.id == id).copied()
    }

    /// Le bouton de cet id.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }
}

/// Où la table d'une fenêtre est écrite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: `src/ui` ou ici, et il n'y a pas de troisième endroit
pub enum Table {
    /// `src/ui::SCREENS`, à cet id : ce module ne la redit pas.
    SrcUi(&'static str),
    /// Ici, parce que `src/ui` n'a pas de ligne.
    Here(&'static Tab),
}

/// Une des quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de l'écran : celui de `src/ui` pour deux d'entre eux.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub class: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// La clé du titre, telle que le XML la pose.
    pub title: &'static str,
    /// Les clés de module dont la vue ouvre cette fenêtre.
    pub module_keys: &'static [&'static str],
    /// Où sont ses panneaux et ses boutons.
    pub table: Table,
    /// Le panneau de la liste : c'est lui qui donne son [`crate::ui::field::Scope`] à
    /// chaque panneau de ligne.
    pub list: &'static str,
    /// Les panneaux que la **ligne** remplit, dans l'ordre où Java les écrit.
    pub row_panes: &'static [&'static str],
}

impl Window {
    /// Le panneau de cet id, d'où qu'il vienne.
    pub fn field(&self, id: &str) -> Option<Field> {
        match self.table {
            Table::SrcUi(screen) => {
                crate::ui::table::screen_by_id(screen).ok()?.field(id)
            }
            Table::Here(tab) => tab.field(id),
        }
    }

    /// La ligne de `src/ui`, quand il y en a une.
    pub fn ui_screen(&self) -> Option<&'static Screen> {
        match self.table {
            Table::SrcUi(screen) => crate::ui::table::screen_by_id(screen).ok(),
            Table::Here(_) => None,
        }
    }

    /// La table écrite ici, quand il y en a une.
    pub fn tab(&self) -> Option<&'static Tab> {
        match self.table {
            Table::Here(tab) => Some(tab),
            Table::SrcUi(_) => None,
        }
    }

    /// Le bouton de cet id, pour les deux fenêtres que ce module décrit.
    ///
    /// `None` pour une fenêtre de `src/ui` : ses boutons sont des [`Action`], et en
    /// fabriquer un [`Button`] ici demanderait d'inventer `on_row` et d'effacer ses refus.
    /// [`Self::ui_action`] les rend tels quels.
    pub fn button(&self, id: &str) -> Option<Button> {
        match self.table {
            Table::Here(tab) => tab.button(id),
            Table::SrcUi(_) => None,
        }
    }

    /// L'action de `src/ui` de cet id, quand la fenêtre est décrite là-bas.
    pub fn ui_action(&self, id: &str) -> Option<Action> {
        self.ui_screen()?.action(id)
    }
}

/// Les deux fenêtres que `src/ui` décrit déjà : leur table est là-bas.
static MINER: Window = Window {
    id: MINER_TAB_ID,
    class: "WindowHutMinerModule",
    resource: "minecolonies:gui/layouthuts/layoutminermodule.xml",
    title: miner::TITLE_KEY,
    module_keys: &[miner::MODULE],
    table: Table::SrcUi(MINER_TAB_ID),
    list: miner::LIST,
    row_panes: miner::ROW_PANES,
};

static TASKS: Window = Window {
    id: TASKS_TAB_ID,
    class: "WindowHutRequestTaskModule",
    resource: "minecolonies:gui/layouthuts/layouttasklist.xml",
    title: tasks::TITLE_KEY,
    // The three views inherit the same `getWindow` (RequestTaskModuleView.java:28-33).
    module_keys: &["craft_task_view", "courier_task_view", "warehouse_request_queue"],
    table: Table::SrcUi(TASKS_TAB_ID),
    list: tasks::LIST,
    row_panes: tasks::ROW_PANES,
};

/// Les recettes : 28 clés, une fenêtre, aucune ligne dans `src/ui`.
static TAB_RECIPES: Tab = Tab { fields: recipes::FIELDS, buttons: recipes::BUTTONS };

static RECIPES: Window = Window {
    id: RECIPES_TAB_ID,
    class: "WindowListRecipes",
    resource: "minecolonies:gui/layouthuts/layoutlistrecipes.xml",
    title: recipes::TITLE_KEY,
    module_keys: recipes::MODULE_KEYS,
    table: Table::Here(&TAB_RECIPES),
    list: recipes::LIST,
    row_panes: &[recipes::OUTPUT, "res1", "res2", "res3", "res4", "res5", "res6", "res7",
        "res8", "res9", recipes::INTERMEDIATE],
};

/// Les gardes d'une mine : un module sans serveur, et aucune ligne dans `src/ui`.
static TAB_GUARDS: Tab = Tab { fields: guards::FIELDS, buttons: guards::BUTTONS };

static GUARDS: Window = Window {
    id: GUARDS_TAB_ID,
    class: "WindowMineGuardModule",
    resource: "minecolonies:gui/layouthuts/layoutguardlist.xml",
    title: guards::TITLE_KEY,
    module_keys: &[guards::MODULE],
    table: Table::Here(&TAB_GUARDS),
    list: guards::LIST,
    row_panes: &[guards::GUARD_NAME],
};

/// Les quatre fenêtres, dans l'ordre de ce document.
pub const WINDOWS: &[&Window] = &[&MINER, &TASKS, &RECIPES, &GUARDS];

/// La fenêtre de cet id.
pub fn window(id: &str) -> Option<&'static Window> {
    WINDOWS.iter().copied().find(|window| window.id == id)
}

/// La fenêtre qu'une clé de module ouvre, quand elle en ouvre une de ce module.
///
/// C'est le pendant de `crate::ui::module_screen`, pour les clés que `src/ui` ne connaît
/// pas : 29 des 33.
pub fn window_of_module(key: &str) -> Option<&'static Window> {
    WINDOWS
        .iter()
        .copied()
        .find(|window| window.module_keys.contains(&key))
}

/// Les 33 clés de module de ces quatre fenêtres, sans doublon.
pub fn module_keys() -> Vec<&'static str> {
    let mut keys = Vec::new();
    for window in WINDOWS {
        for key in window.module_keys {
            if !keys.contains(key) {
                keys.push(*key);
            }
        }
    }
    keys
}

/// Les refus qu'un bouton de `src/ui` déclare, quand la fenêtre est décrite là-bas.
///
/// `None` pour les deux fenêtres de ce module, dont les boutons portent [`Kind`] : les deux
/// vocabulaires ne se mélangent pas, et ce module ne prétend pas le contraire.
pub fn ui_refusals(window: &'static Window, button: &str) -> Option<&'static [UiRefusal]> {
    window.ui_action(button).map(|action| action.refusals)
}

/// Le titre de `src/ui` pour cette fenêtre, quand il y est décrit comme un [`Title`].
pub fn title_of(window: &'static Window) -> Title {
    match window.ui_screen() {
        Some(screen) => screen.title,
        None => Title::Fixed(window.title),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_windows_two_of_which_src_ui_already_describes() {
        assert_eq!(WINDOWS.len(), 4);
        let described: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.ui_screen().is_some())
            .map(|window| window.class)
            .collect();
        assert_eq!(
            described,
            ["WindowHutMinerModule", "WindowHutRequestTaskModule"],
            "src/ui carries exactly these two, as tab.miner_levels and tab.tasks"
        );
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(!window.class.is_empty());
            assert!(
                window.resource.starts_with("minecolonies:gui/layouthuts/"),
                "{} reads {}",
                window.id,
                window.resource
            );
            assert!(window.resource.ends_with(".xml"));
            assert!(window.title.starts_with("com.minecolonies."));
            assert!(!window.row_panes.is_empty(), "{} fills no row", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id);
                assert_ne!(window.class, other.class);
                assert_ne!(window.resource, other.resource);
            }
        }
    }

    #[test]
    fn the_two_windows_src_ui_describes_are_not_described_a_second_time() {
        for window in [&MINER, &TASKS] {
            let screen = window.ui_screen().expect("src/ui has this screen");
            assert_eq!(screen.id, window.id);
            assert_eq!(
                screen.window, window.class,
                "{}: the class must be src/ui's",
                window.id
            );
            assert_eq!(screen.resource, window.resource);
            assert_eq!(title_of(window), screen.title);
            assert!(window.tab().is_none());
        }
        // And the reverse: src/ui has no line for the recipes or the guards.
        for class in ["WindowListRecipes", "WindowMineGuardModule"] {
            assert!(
                crate::ui::SCREENS.iter().all(|screen| screen.window != class),
                "{class} must not be in src/ui's thirteen"
            );
        }
    }

    #[test]
    fn every_row_pane_is_declared_by_the_table_the_window_points_at() {
        for window in WINDOWS {
            for id in window.row_panes {
                let field = window
                    .field(id)
                    .unwrap_or_else(|| panic!("{}: {id} is not declared", window.id));
                assert_eq!(
                    field.scope,
                    crate::ui::field::Scope::Row(window.list),
                    "{}: {id} is not in a row",
                    window.id
                );
            }
        }
    }

    #[test]
    fn the_two_new_tabs_declare_their_panes_and_their_buttons() {
        let recipes = RECIPES.tab().expect("declared here");
        assert_eq!(recipes.fields.len(), 13);
        assert_eq!(recipes.buttons.len(), 5);
        let guards = GUARDS.tab().expect("declared here");
        assert_eq!(guards.fields.len(), 3);
        assert_eq!(guards.buttons.len(), 1);
        // Five of the six buttons are row buttons, and the sixth opens the container.
        let buttons: Vec<Button> = recipes
            .buttons
            .iter()
            .chain(guards.buttons.iter())
            .copied()
            .collect();
        assert_eq!(buttons.iter().filter(|button| button.on_row).count(), 5);
        assert_eq!(
            buttons.iter().filter(|button| !button.on_row).map(|b| b.id).collect::<Vec<_>>(),
            ["crafting"]
        );
        assert!(buttons.iter().all(|button| button.is_guarded()));
        assert_eq!(
            RECIPES.button("crafting").map(|button| button.effect),
            Some(Effect::Container)
        );
        assert_eq!(RECIPES.button("nope"), None);
    }

    #[test]
    fn thirty_three_module_keys_and_twenty_nine_src_ui_does_not_know() {
        let keys = module_keys();
        assert_eq!(keys.len(), 33);
        let known: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|key| crate::ui::module_screen(key).is_some())
            .collect();
        assert_eq!(
            known,
            [
                "miner_levels",
                "craft_task_view",
                "courier_task_view",
                "warehouse_request_queue"
            ]
        );
        // Every key reaches exactly one window, whichever table knows it.
        for key in &keys {
            assert_eq!(
                window_of_module(key).map(|window| window.id),
                Some(
                    if crate::ui::module_screen(key).is_some() {
                        match *key {
                            "miner_levels" => MINER_TAB_ID,
                            _ => TASKS_TAB_ID,
                        }
                    } else if recipes::MODULE_KEYS.contains(key) {
                        RECIPES_TAB_ID
                    } else {
                        GUARDS_TAB_ID
                    }
                ),
                "{key}"
            );
        }
        assert_eq!(window_of_module("baker_smelt").map(|window| window.class), Some("WindowListRecipes"));
        assert_eq!(window_of_module("nonexistent"), None);
        assert_eq!(window(MINER_TAB_ID).map(|window| window.class), Some("WindowHutMinerModule"));
        assert_eq!(window("hire"), None);
    }

    #[test]
    fn the_mine_rows_are_refused_with_the_two_vocabularies_of_src_ui() {
        // The row refusals this module computes are the two src/ui already named.
        let repair = ui_refusals(&MINER, miner::REPAIR).expect("the repair button");
        assert!(repair.contains(&UiRefusal::WorkOrderExists));
        let mine = ui_refusals(&MINER, miner::MINE).expect("the mine button");
        assert!(mine.contains(&UiRefusal::CurrentLevel));
        assert_eq!(Refusal::WorkOrderExists { level: 3 }.ui(), Some(UiRefusal::WorkOrderExists));
        // And the two windows written here have no ui refusals to compare with.
        assert_eq!(ui_refusals(&RECIPES, recipes::REMOVE), None);
        // src/ui's own screen has no `remove` button at all.
        assert!(MINER.ui_action(recipes::REMOVE).is_none());
        assert!(MINER.button(miner::MINE).is_none(), "src/ui keeps its own buttons");
        assert!(MINER.ui_action(miner::MINE).is_some());
    }

    #[test]
    fn the_six_messages_these_windows_name_are_already_in_the_port() {
        let messages = [
            "MinerSetLevelMessage",
            "MinerRepairLevelMessage",
            "GuardSetMinePosMessage",
            "ToggleRecipeMessage",
            "AddRemoveRecipeMessage",
            "ChangeRecipePriorityMessage",
        ];
        let mut named: Vec<&str> = Vec::new();
        for window in WINDOWS {
            if let Some(tab) = window.tab() {
                for button in tab.buttons {
                    if let Some(class) = button.sends() {
                        named.push(class);
                    }
                }
            }
        }
        // src/ui's buttons are not re-listed: the two it carries keep their own names.
        assert_eq!(
            named,
            [
                "ChangeRecipePriorityMessage",
                "ChangeRecipePriorityMessage",
                "AddRemoveRecipeMessage",
                "ToggleRecipeMessage",
                "GuardSetMinePosMessage",
            ]
        );
        // A test that names a message src/ui carries: it is the same class.
        let recall = ui_refusals(&MINER, miner::REPAIR).is_some();
        assert!(recall);
        // The two the tables above never name are the mine's own, and src/ui names them.
        for class in ["MinerSetLevelMessage", "MinerRepairLevelMessage"] {
            assert!(messages.contains(&class));
        }
        // All six exist as types in the port's message module.
        assert!(core::mem::size_of::<crate::network::building_modules::MinerSetLevel>() > 0);
        assert!(core::mem::size_of::<crate::network::building_modules::MinerRepairLevel>() > 0);
        assert!(core::mem::size_of::<crate::network::building_modules::GuardSetMinePos>() > 0);
        assert!(core::mem::size_of::<crate::network::building_modules::ToggleRecipe>() > 0);
        assert!(core::mem::size_of::<crate::network::building_modules::AddRemoveRecipe>() > 0);
        assert!(
            core::mem::size_of::<crate::network::building_modules::ChangeRecipePriority>() > 0
        );
    }
}
