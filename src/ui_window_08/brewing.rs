//! `WindowBrewingstandCrafting` : quatre emplacements, un bouton, et un plafond de
//! recettes qui n'est pas là où le bouton le dit.
//!
//! Java reference : `core/client/gui/containers/WindowBrewingstandCrafting.java` (149
//! lignes), `api/inventory/container/ContainerCraftingBrewingstand.java:71-206`,
//! `core/network/messages/server/colony/building/worker/AddRemoveRecipeMessage.java:154-191`
//! et `core/colony/buildings/modules/AbstractCraftingBuildingModule.java:136-180`.
//!
//! # L'ordre des emplacements de l'écran n'est pas celui du handler
//!
//! Le conteneur ajoute **l'ingrédient d'abord** (`ContainerCraftingBrewingstand.java:176` :
//! `addSlot(new SlotItemHandler(brewingStandInventory, 3, ...))`), puis les trois potions
//! (`:178-180`). Donc `container.slots.get(0)` est le slot **3** du handler (l'ingrédient)
//! et `container.slots.get(3)` est le slot **2** (la troisième potion). La fenêtre en
//! dépend : quand elle calcule la sortie, elle appelle
//! `BrewingRecipeRegistry.getOutput(container.slots.get(3).getItem(),
//! container.slots.get(0).getItem())` (`:122`), c'est-à-dire **la troisième potion et
//! l'ingrédient**. Les deux autres potions ne sont jamais lues pour la recette, même
//! remplies : elles partent seulement dans la liste d'entrées (`:116-119`).
//!
//! # Le libellé « maximum de recettes atteints » ne parle pas d'un plafond
//!
//! Le bouton est désactivé et renommé quand `!module.canLearn(BREWING)`
//! (`:96`, `:102-105`). Or `canLearn` teste **l'appartenance du type de craft** au module
//! (`CraftingModuleView.java:155-158` : `getSupportedCraftingTypes().contains(type)`), pas
//! un nombre : le vrai plafond est `getMaxRecipes() > getActiveRecipes()`
//! (`AbstractCraftingBuildingModule.java:147-150`), avec
//! `getMaxRecipes() = 2^niveau × 5` (`:171-179`, le `5` de `canLearnManyRecipes`). Un
//! module qui n'enseigne pas la brasserie affiche donc « plafond atteint » alors que son
//! plafond est peut-être vide — et c'est exactement le cas d'un alchimiste dont on ouvre
//! la fenêtre par un autre chemin.
//!
//! # Par la route normale, ce bouton n'est jamais fermé
//!
//! L'écran n'est ouvert que par `OpenCraftingGUIMessage` (`:74-135`), dans cet ordre :
//! `canLearn(SMELTING)` → four, sinon `canLearn(BREWING)` → brasserie, sinon établi. On
//! n'arrive donc ici qu'avec `canLearn(BREWING)` **vrai** : l'état fermé et son libellé
//! sont hors d'atteinte par cette route. Le port les garde — un autre appelant peut ouvrir
//! la fenêtre — mais le dit au lieu de laisser croire que c'est un cas courant.
//!
//! # Deux refus muets, et un troisième qui parle
//!
//! `!canLearn` ferme le bouton ; une sortie vide n'envoie **rien du tout** et ne dit rien
//! (`:125`) ; et si le module refuse la recette côté serveur,
//! `AddRemoveRecipeMessage.java:177-181` joue un son d'erreur et dit
//! [`super::refusal::UNABLE_KEY`] au joueur. Trois refus, une seule phrase.

use super::pane::Pane;
use super::refusal::{Refusal, FULL_KEY};
use super::{Button, Effect, Naming};

/// La classe Java.
pub const JAVA: &str = "WindowBrewingstandCrafting";
/// Le titre que le serveur met dans le paquet : un **littéral** anglais, pas une clé de
/// traduction (`OpenCraftingGUIMessage.java:103`).
pub const TITLE: &str = "Brewing Crafting GUI";
/// La clé du module dont la fenêtre lit `canLearn` et le producteur.
pub const MODULE_KEY: &str = "brewing";
/// Le type de craft que le module doit enseigner (`ModCraftingTypes.java:17`, `:24`).
pub const CRAFTING_TYPE: &str = "minecolonies:brewing";
/// Le libellé du bouton quand la recette peut être apprise
/// (`BaseGameTranslationConstants.java:8`, lu à `:96`).
pub const DONE_LABEL_KEY: &str = "gui.done";
/// Le nom du bouton **dans ce port** : Java en crée un sans id
/// (`new Button.Builder(...)`, `:100`), donc il n'y a aucune chaîne à reprendre.
pub const BUTTON_DONE: &str = "done";
/// La sortie compte trois potions (`:123`).
pub const OUTPUT_COUNT: i32 = 3;
/// La taille de grille envoyée avec la recette (`:127`).
pub const GRID: i32 = 1;
/// Le bloc intermédiaire envoyé avec la recette (`Blocks.BREWING_STAND`, `:127`).
pub const INTERMEDIATE: &str = "minecraft:brewing_stand";
/// L'emplacement du handler dont la fenêtre lit la **potion** (`:122`), sur les trois.
pub const RECIPE_POTION: usize = 2;
/// L'emplacement du handler dont la fenêtre lit l'**ingrédient** (`:122`).
pub const RECIPE_INGREDIENT: usize = 3;
/// Le multiplicateur des modules qui apprennent beaucoup de recettes
/// (`AbstractCraftingBuildingModule.java:87`), et `canLearnManyRecipes()` répond `true`
/// pour tout module d'artisanat (`:1001-1004`).
pub const MANY_RECIPES_MULTIPLIER: i64 = 5;

/// Le rôle d'un emplacement de la brasserie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux rôles dans ContainerCraftingBrewingstand.isItemValid
pub enum Role {
    /// `BrewingRecipeRegistry.isValidIngredient` (`:159-162`).
    Ingredient,
    /// `BrewingRecipeRegistry.isValidInput` (`:163-166`).
    Potion,
}

impl Role {
    /// Le nom du rôle, pour un log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Role::Ingredient => "ingredient",
            Role::Potion => "potion",
        }
    }
}

/// Un des quatre emplacements : son rang **à l'écran** et son rang **dans le handler**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// L'index dans `container.slots`, celui que la fenêtre lit.
    pub screen: usize,
    /// L'index dans le handler de la brasserie, celui qui décide de la validité.
    pub handler: usize,
    /// Ce que l'emplacement accepte.
    pub role: Role,
}

/// Les quatre emplacements, dans l'ordre où le conteneur les ajoute
/// (`ContainerCraftingBrewingstand.java:176-180`).
pub const SLOTS: [Slot; 4] = [
    Slot { screen: 0, handler: 3, role: Role::Ingredient },
    Slot { screen: 1, handler: 0, role: Role::Potion },
    Slot { screen: 2, handler: 1, role: Role::Potion },
    Slot { screen: 3, handler: 2, role: Role::Potion },
];

/// L'emplacement d'écran de ce rang, ou `None` pour un rang que la brasserie n'a pas.
pub fn slot(screen: usize) -> Option<Slot> {
    SLOTS.iter().copied().find(|slot| slot.screen == screen)
}

/// L'emplacement du handler de ce rang.
pub fn slot_of_handler(handler: usize) -> Option<Slot> {
    SLOTS.iter().copied().find(|slot| slot.handler == handler)
}

/// Aucun panneau nommé : la fenêtre est un `AbstractContainerScreen`, sans XML, et ses
/// quatre emplacements sont ajoutés par le conteneur, pas décrits par un panneau. Le
/// tableau [`SLOTS`] est ce que cette fenêtre a à la place.
pub const FIELDS: &[Pane] = &[];

/// Le seul bouton. Son libellé dépend de l'état : [`State::label`].
pub const BUTTONS: &[Button] = &[Button {
    id: BUTTON_DONE,
    label: DONE_LABEL_KEY,
    effect: Effect::Sends("AddRemoveRecipeMessage"),
    refusals: &[Refusal::CraftingTypeNotTaught, Refusal::NoBrewingRecipe],
    naming: Naming::Ours,
}];

/// Ce que la fenêtre lit pour décider : le type enseigné, et l'id du producteur du module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// `module.canLearn(ModCraftingTypes.BREWING.get())` (`:96`, `:102`, `:113`).
    pub can_teach: bool,
    /// `module.getProducer().getRuntimeID()` (`:127`), que le message porte tel quel.
    pub module_id: i32,
}

impl State {
    /// L'état d'ouverture : le type enseigné ou non, et l'id du module.
    pub const fn new(can_teach: bool, module_id: i32) -> State {
        State { can_teach, module_id }
    }

    /// Le libellé du bouton dans cet état (`:96`) : `gui.done`, ou la clé du « plafond ».
    pub fn label(self) -> &'static str {
        if self.can_teach {
            DONE_LABEL_KEY
        } else {
            FULL_KEY
        }
    }

    /// L'état du bouton : Java le ferme exactement quand le type n'est pas enseigné
    /// (`:102-105`).
    pub fn button_enabled(self) -> bool {
        self.can_teach
    }

    /// Le clic sur `done` (`:108-131`), et le refus nommé quand il n'envoie rien.
    ///
    /// `stacks` sont les quatre piles de l'écran, **dans l'ordre de l'écran** (`:116-119`),
    /// et `output` est la sortie du brassage — que le port ne peut pas calculer, faute du
    /// registre de brassage (`MANQUANT.md` §2) : l'appelant la fournit, ou `None`.
    pub fn done(
        self,
        stacks: [Option<&'static str>; 4],
        output: Option<&'static str>,
    ) -> Result<Recipe, Refusal> {
        if !self.can_teach {
            return Err(Refusal::CraftingTypeNotTaught);
        }
        let Some(output) = output else {
            // `!ItemStackUtils.isEmpty(primaryOutput)` (`:125`) : rien n'est envoyé et
            // rien n'est dit. Le bouton, lui, est resté actif.
            return Err(Refusal::NoBrewingRecipe);
        };
        Ok(Recipe {
            ingredients: stacks,
            output,
            output_count: OUTPUT_COUNT,
            grid: GRID,
            intermediate: INTERMEDIATE,
            module_id: self.module_id,
            remove: false,
        })
    }
}

/// La recette que le clic envoie dans `AddRemoveRecipeMessage` (`:127`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// Les quatre piles de l'écran, dans l'ordre de l'écran — y compris les deux potions
    /// que la sortie n'a pas lues.
    pub ingredients: [Option<&'static str>; 4],
    /// La sortie, déjà à [`OUTPUT_COUNT`] exemplaires (`:123`).
    pub output: &'static str,
    /// Le nombre d'exemplaires produits.
    pub output_count: i32,
    /// La taille de grille.
    pub grid: i32,
    /// Le bloc intermédiaire.
    pub intermediate: &'static str,
    /// L'id du producteur du module (`:127`).
    pub module_id: i32,
    /// `false` : la fenêtre n'ajoute que des recettes, elle n'en retire pas.
    pub remove: bool,
}

/// Le plafond de recettes d'un bâtiment de ce niveau, sans recherche
/// (`AbstractCraftingBuildingModule.java:171-179`) : `2^niveau × 5`.
///
/// Le niveau est borné à `0..=5`, les cinq niveaux d'un bâtiment ; la force de recherche
/// (`getEffectStrength(RECIPES)`) et les recettes automatiques ne sont pas dans cette
/// tranche, d'où un plafond qui ne peut qu'être trop bas, jamais trop haut.
pub fn recipes_cap(level: i32) -> i64 {
    let level = level.clamp(0, 5);
    (1_i64 << level) * MANY_RECIPES_MULTIPLIER
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brew() -> State {
        State::new(true, 12)
    }

    #[test]
    fn the_screen_order_is_not_the_handler_order() {
        // ContainerCraftingBrewingstand.java:176-180 : l'ingrédient est ajouté d'abord.
        assert_eq!(slot(0).map(|slot| slot.handler), Some(3));
        assert_eq!(slot(3).map(|slot| slot.handler), Some(2));
        assert_eq!(
            SLOTS.iter().map(|slot| slot.handler).collect::<Vec<_>>(),
            [3, 0, 1, 2]
        );
        // Quatre emplacements, quatre rangs de handler distincts, deux rôles.
        let mut handlers: Vec<usize> = SLOTS.iter().map(|slot| slot.handler).collect();
        handlers.sort_unstable();
        assert_eq!(handlers, [0, 1, 2, 3]);
        let potions = SLOTS.iter().filter(|slot| slot.role == Role::Potion).count();
        assert_eq!(potions, 3);
        assert_eq!(SLOTS.iter().filter(|slot| slot.role == Role::Ingredient).count(), 1);
        // Le rang d'écran 4 n'existe pas, et son refus est muet.
        assert!(slot(4).is_none());
        assert_eq!(slot_of_handler(3).map(|slot| slot.role), Some(Role::Ingredient));
    }

    #[test]
    fn the_output_reads_the_third_potion_and_ignores_the_first_two() {
        // :122 — getOutput(slots.get(3), slots.get(0)) = handler 2 et handler 3.
        assert_eq!(RECIPE_POTION, 2);
        assert_eq!(RECIPE_INGREDIENT, 3);
        assert_eq!(
            slot_of_handler(RECIPE_POTION).map(|slot| slot.role),
            Some(Role::Potion),
            "la potion lue est bien l'une des trois"
        );
        assert_eq!(
            slot_of_handler(RECIPE_POTION).map(|slot| slot.screen),
            Some(3),
            "et c'est la quatrième pile de l'écran"
        );
        assert_eq!(slot_of_handler(RECIPE_INGREDIENT).map(|slot| slot.screen), Some(0));
        // Les deux autres potions ne sont jamais lues : elles partent dans les entrées.
        let unread: Vec<usize> = SLOTS
            .iter()
            .filter(|slot| slot.role == Role::Potion && slot.handler != RECIPE_POTION)
            .map(|slot| slot.handler)
            .collect();
        assert_eq!(unread, [0, 1]);
    }

    #[test]
    fn the_label_says_full_when_the_module_does_not_teach_brewing() {
        assert_eq!(brew().label(), DONE_LABEL_KEY);
        let untutored = State::new(false, 12);
        assert_eq!(untutored.label(), FULL_KEY);
        // La clé parle d'un plafond, et ce n'est pas ce que Java teste : canLearn est un
        // test d'appartenance de type (CraftingModuleView.java:155-158).
        assert_eq!(untutored.label(), "com.minecolonies.coremod.gui.recipe.full");
        // Le bouton est fermé dans cet état, et le refus qui l'explique porte le libellé.
        assert!(!untutored.button_enabled());
        assert!(brew().button_enabled());
        assert_eq!(Refusal::CraftingTypeNotTaught.label(), Some(FULL_KEY));
    }

    #[test]
    fn the_real_cap_is_five_times_two_to_the_level_and_the_window_never_reads_it() {
        // AbstractCraftingBuildingModule.java:171-179, sans recherche.
        assert_eq!(recipes_cap(0), 5);
        assert_eq!(recipes_cap(1), 10);
        assert_eq!(recipes_cap(3), 40);
        assert_eq!(recipes_cap(5), 160);
        // Un niveau hors bornes ne fait pas exploser le plafond.
        assert_eq!(recipes_cap(-2), 5);
        assert_eq!(recipes_cap(99), 160);
        // Le plafond double à chaque niveau, et c'est une puissance de deux fois cinq.
        for level in 0..5 {
            assert_eq!(recipes_cap(level + 1), recipes_cap(level) * 2);
        }
    }

    #[test]
    fn a_done_click_without_a_brew_or_without_the_type_sends_nothing() {
        // Une sortie inconnue : rien n'est envoyé, rien n'est dit (`:125`).
        let empty = brew().done([Some("minecraft:nether_wart"), None, None, None], None);
        assert_eq!(empty, Err(Refusal::NoBrewingRecipe));
        assert_eq!(Refusal::NoBrewingRecipe.key(), None);
        // Un module qui n'enseigne pas le type refuse avant même de regarder la sortie.
        let untutored =
            State::new(false, 12)
                .done([None, None, None, Some("minecraft:potion")], Some("minecraft:swiftness_potion"));
        assert_eq!(untutored, Err(Refusal::CraftingTypeNotTaught));
        assert_eq!(Refusal::CraftingTypeNotTaught.key(), None);
    }

    #[test]
    fn a_full_brew_carries_three_outputs_one_grid_and_keeps_every_input_in_screen_order() {
        let stacks = [
            Some("minecraft:nether_wart"),
            Some("minecraft:potion"),
            Some("minecraft:potion"),
            Some("minecraft:potion"),
        ];
        let recipe = match brew().done(stacks, Some("minecraft:awkward_potion")) {
            Ok(recipe) => recipe,
            Err(refusal) => panic!("un brassage complet a été refusé : {refusal}"),
        };
        assert_eq!(recipe.output, "minecraft:awkward_potion");
        assert_eq!(recipe.output_count, 3, ":123 setCount(3)");
        assert_eq!(recipe.grid, 1, ":127");
        assert_eq!(recipe.intermediate, "minecraft:brewing_stand");
        assert_eq!(recipe.module_id, 12, ":127, le producteur du module");
        assert!(!recipe.remove, "la fenêtre ne retire pas de recette");
        // Les deux potions que la sortie n'a pas lues sont quand même dans les entrées.
        assert_eq!(recipe.ingredients[1], Some("minecraft:potion"));
        assert_eq!(recipe.ingredients[2], Some("minecraft:potion"));
        assert_eq!(recipe.ingredients[0], Some("minecraft:nether_wart"));
    }
}
