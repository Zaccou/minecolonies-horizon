//! `WindowFurnaceCrafting` : deux cases, un bouton, et un titre qui vient du paquet — pas
//! de la classe.
//!
//! Java reference : `core/client/gui/containers/WindowFurnaceCrafting.java` (195 lignes),
//! `api/inventory/container/ContainerCraftingFurnace.java:74-217` (les deux cases et la
//! troisième qui n'en est pas une), `OpenCraftingGUIMessage.java:74-92` (le nom du paquet)
//! et `ModCraftingTypes.java:13` (`smelting`).
//!
//! # Trois différences avec `WindowCrafting`, toutes dans le source
//!
//! 1. **Pas de bascule de recette.** `init` n'ajoute que deux widgets (`:119`, `:126`) :
//!    le four n'a pas d'équivalent de `SwitchRecipeCraftingTeachingMessage`, et un four ne
//!    rend qu'une recette pour une entrée.
//! 2. **Pas de `renderLabels`.** La classe ne redéfinit que `renderBg` (`:182-186`) :
//!    c'est `AbstractContainerScreen` qui dessine `this.title`, donc le **littéral du
//!    paquet** (`"Furnace Crafting GUI"`, `OpenCraftingGUIMessage.java:82`) — alors que
//!    l'établi dessine une clé en dur et ignore le sien (`WindowCrafting.java:255`). Deux
//!    écrans voisins, deux provenances de titre.
//! 3. **Une grille d'une case.** `gridSize` vaut `1` (`:149`) et le bloc intermédiaire est
//!    passé **explicitement** (`Blocks.FURNACE`) au constructeur qui le prend en argument
//!    (`AddRemoveRecipeMessage.java:112-125`) : pour l'établi, c'est l'autre constructeur
//!    qui le déduit (`:96`, `gridSize == 1 ? FURNACE : AIR`).
//!
//! # Le handler déclare trois emplacements et le menu en ajoute deux
//!
//! `getSlots()` répond `3` (`ContainerCraftingFurnace.java:99-102`) mais `getStackInSlot`
//! et `insertItem` ne connaissent que **deux** cases : tout index différent de `0` écrit
//! et relit la **sortie** (`:106-116`, `:118-133`). Aucune des deux n'est ajoutée au menu
//! (`:163`, `:191`), et `FURNACE_SLOTS` vaut `2` (`InventoryConstants.java:96`) : la
//! troisième est inatteignable sur l'écran et n'existe que pour le handler. Le compte de
//! l'écran est [`MENU_SLOTS`], celui du handler [`HANDLER_SLOTS`], et
//! [`tests::the_handler_declares_one_more_slot_than_the_menu_adds`] épingle l'écart.

use super::pane::Pane;
use super::refusal::{Refusal, DONE_KEY, FULL_KEY};
use super::{Button, Effect, Naming, Storage};
use crate::crafting::Teaching;

/// La classe Java.
pub const JAVA: &str = "WindowFurnaceCrafting";
/// Le titre, **du paquet** : `Component.literal("Furnace Crafting GUI")`
/// (`OpenCraftingGUIMessage.java:82`), dessiné par la classe de base faute de
/// `renderLabels` dans celle-ci.
pub const TITLE: &str = "Furnace Crafting GUI";
/// La texture (`:45`).
pub const TEXTURE: &str = "minecolonies:textures/gui/furnace.png";
/// La case d'entrée, telle que la fenêtre la lit (`:144`, `container.slots.get(0)`).
pub const INPUT_SLOT: usize = 0;
/// La case de sortie (`:145`, `container.slots.get(1)`).
pub const OUTPUT_SLOT: usize = 1;
/// La grille envoyée avec la recette (`:149`).
pub const GRID: i32 = 1;
/// Le bloc intermédiaire envoyé (`:149`, `Blocks.FURNACE`).
pub const INTERMEDIATE: &str = "minecraft:furnace";
/// Le nombre d'exemplaires de la sortie : le conteneur force `1` dans les deux sens
/// (`ContainerCraftingFurnace.java:86-87`, `:121-123`).
pub const OUTPUT_COUNT: i32 = 1;
/// Les cases que le menu ajoute vraiment (`:163`, `:191`), et que
/// `InventoryConstants.FURNACE_SLOTS` compte (`InventoryConstants.java:96`).
pub const MENU_SLOTS: usize = 2;
/// Les cases que le handler **déclare** (`:99-102`). Une de plus, et inatteignable.
pub const HANDLER_SLOTS: usize = 3;
/// Le bouton « terminé » : `new Button.Builder(...)` (`:119`), sans id.
pub const BUTTON_DONE: &str = "done";
/// La liste des requêtes : un `ImageButton` (`:126`), sans id.
pub const BUTTON_REQUESTS: &str = "requests";

/// Aucun panneau nommé.
///
/// La fenêtre n'a ni XML ni `findPaneOfTypeByID` : ses deux cases viennent du conteneur
/// (`:163`, `:191`) et son titre de `renderLabels` de la classe de base (`:106-110` de
/// `WindowGrave` fait la même chose explicitement). Un panneau sans id n'est pas un
/// panneau — `src/ui/MANQUANT.md` §1 dit quel widget manque pour les dessiner.
pub const FIELDS: &[Pane] = &[];

/// Les deux boutons, dans l'ordre de `init` (`:119`, `:126`).
pub const BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_DONE,
        label: DONE_KEY,
        effect: Effect::Sends("AddRemoveRecipeMessage"),
        refusals: &[Refusal::CraftingTypeNotTaught, Refusal::NoResult, Refusal::NoFurnaceInput],
        naming: Naming::Ours,
    },
    Button {
        id: BUTTON_REQUESTS,
        label: "",
        effect: Effect::OpensJava("WindowSelectRequest"),
        refusals: &[Refusal::JeiMissing],
        naming: Naming::Ours,
    },
];

/// Ce que la fenêtre lit : les types enseignés, JEI, l'id du producteur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// `module.getSupportedCraftingTypes()` — ce que `canLearn(SMELTING)` teste
    /// (`CraftingModuleView.java:155-158`, appelé en `:115`, `:121`, `:141`).
    pub teaches: &'static [Teaching],
    /// `Compatibility.jeiProxy.isLoaded()` (`:132`).
    pub jei: bool,
    /// `module.getProducer().getRuntimeID()` (`:149`), porté tel quel par le message.
    pub module_id: i32,
}

impl State {
    /// L'état d'ouverture.
    pub const fn new(teaches: &'static [Teaching], jei: bool, module_id: i32) -> State {
        State { teaches, jei, module_id }
    }

    /// `module.canLearn(SMELTING)`.
    pub fn can_learn(self) -> bool {
        self.teaches.contains(&Teaching::Smelting)
    }

    /// Le libellé du bouton (`:115`) : `gui.done`, ou la clé du « plafond ».
    pub fn label(self) -> &'static str {
        if self.can_learn() {
            DONE_KEY
        } else {
            FULL_KEY
        }
    }

    /// L'état du bouton : fermé quand le four n'est pas enseigné (`:121-124`).
    pub fn done_enabled(self) -> bool {
        self.can_learn()
    }

    /// Le bouton des requêtes suit JEI (`:132`).
    pub fn requests_visible(self) -> bool {
        self.jei
    }

    /// Le clic sur « terminé » (`WindowFurnaceCrafting.java:136-153`).
    ///
    /// `input` est la pile de la case 0 et `output` celle de la case 1 ; le port ne les
    /// calcule pas — la sortie d'un four vient du registre du jeu
    /// (`getSmeltingResult`, `ContainerCraftingFurnace.java:153`, `:283`), donc l'appelant
    /// les fournit (`MANQUANT.md` §0).
    pub fn done(
        self,
        input: Option<&'static str>,
        output: Option<&'static str>,
    ) -> Result<Storage, Refusal> {
        // `:141` : tout le clic est sous `if (module.canLearn(SMELTING))`.
        if !self.can_learn() {
            return Err(Refusal::CraftingTypeNotTaught);
        }
        let Some(input) = input else {
            // `:144` ne regarde pas : le port refuse l'entrée vide que Java enverrait.
            return Err(Refusal::NoFurnaceInput);
        };
        let Some(output) = output else {
            // `:147` : `!ItemStackUtils.isEmpty(primaryOutput)`, rien n'est envoyé.
            return Err(Refusal::NoResult);
        };
        Ok(Storage {
            inputs: vec![Some(input)],
            input_count: 1,
            primary: output,
            // Le four n'a pas de sortie secondaire : le constructeur de `:112-125` n'en
            // prend pas, et `.withSecondaryOutputs` n'est jamais appelé.
            secondary: Vec::new(),
            grid: GRID,
            intermediate: Some(INTERMEDIATE),
            module_id: self.module_id,
            remove: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMELTING: &[Teaching] = &[Teaching::Smelting];
    const CRAFTING: &[Teaching] = &[Teaching::SmallCrafting];

    fn furnace() -> State {
        State::new(SMELTING, false, 9)
    }

    #[test]
    fn the_furnace_has_two_widgets_and_no_switch_button() {
        assert_eq!(BUTTONS.len(), 2, ":119 et :126, deux widgets et pas un de plus");
        let switch = BUTTONS.iter().find(|button| button.id == "switch");
        assert!(switch.is_none(), "un four ne cherche pas de bascule");
        assert_eq!(super::super::crafting::BUTTONS.len(), 3, "l'établi, lui, en a trois");
        // Le premier est le bouton, le second la liste des requêtes.
        assert_eq!(BUTTONS[0].id, BUTTON_DONE);
        assert_eq!(BUTTONS[1].id, BUTTON_REQUESTS);
        for button in BUTTONS {
            assert_eq!(button.naming, Naming::Ours);
        }
        assert!(!furnace().requests_visible());
        assert!(State::new(SMELTING, true, 9).requests_visible());
    }

    #[test]
    fn the_done_button_reads_smelting_and_shows_the_same_full_key() {
        assert_eq!(furnace().label(), DONE_KEY);
        assert!(furnace().done_enabled());
        let untutored = State::new(CRAFTING, false, 9);
        assert_eq!(untutored.label(), FULL_KEY);
        assert!(!untutored.done_enabled());
        // La clé est la même que celle de l'établi, et ce n'est pas un message.
        assert_eq!(Some(untutored.label()), Refusal::CraftingTypeNotTaught.label());
        assert_eq!(Refusal::CraftingTypeNotTaught.key(), None);
        assert_eq!(
            untutored.done(Some("minecraft:raw_iron"), Some("minecraft:iron_ingot")),
            Err(Refusal::CraftingTypeNotTaught)
        );
    }

    #[test]
    fn a_click_carries_one_input_one_output_and_the_furnace_itself() {
        let storage = match furnace().done(Some("minecraft:raw_iron"), Some("minecraft:iron_ingot")) {
            Ok(storage) => storage,
            Err(refusal) => panic!("une fonte complète a été refusée : {refusal}"),
        };
        assert_eq!(storage.grid, GRID);
        assert_eq!(storage.grid, 1);
        assert_eq!(storage.inputs, [Some("minecraft:raw_iron")]);
        assert_eq!(storage.input_count, 1);
        assert_eq!(storage.primary, "minecraft:iron_ingot");
        assert_eq!(storage.module_id, 9);
        assert!(!storage.remove);
        // Le bloc intermédiaire est **passé**, pas déduit : `AddRemoveRecipeMessage.java:149`.
        assert_eq!(storage.intermediate, Some(INTERMEDIATE));
        assert_eq!(INTERMEDIATE, "minecraft:furnace");
        // Aucune sortie secondaire : le constructeur du four n'en a pas.
        assert!(storage.secondary.is_empty());
        // Et la sortie part avec un exemplaire, comme le conteneur l'impose.
        assert_eq!(OUTPUT_COUNT, 1);
    }

    #[test]
    fn an_empty_output_or_an_empty_input_sends_nothing() {
        // `:147` : la branche de Java, muette, bouton actif.
        assert_eq!(furnace().done(Some("minecraft:raw_iron"), None), Err(Refusal::NoResult));
        assert_eq!(Refusal::NoResult.key(), None);
        assert!(furnace().done_enabled());
        // L'entrée vide n'est pas une branche de Java : le port refuse.
        assert_eq!(furnace().done(None, Some("minecraft:iron_ingot")), Err(Refusal::NoFurnaceInput));
        // Et l'entrée est lue **avant** la sortie, dans l'ordre du corps de Java.
        assert_eq!(furnace().done(None, None), Err(Refusal::NoFurnaceInput));
    }

    #[test]
    fn the_handler_declares_one_more_slot_than_the_menu_adds() {
        // `:99-102` répond 3, `:163` et `:191` en ajoutent 2, et `FURNACE_SLOTS` vaut 2.
        assert_eq!(HANDLER_SLOTS, 3);
        assert_eq!(MENU_SLOTS, 2);
        assert_eq!(HANDLER_SLOTS, MENU_SLOTS + 1);
        assert_eq!(INPUT_SLOT, 0);
        assert_eq!(OUTPUT_SLOT, 1);
        assert_eq!(INPUT_SLOT.max(OUTPUT_SLOT), MENU_SLOTS - 1, "les deux cases sont 0 et 1");
        // Le titre est celui du paquet, et il n'est pas une clé de traduction.
        assert_eq!(TITLE, "Furnace Crafting GUI");
        assert!(!TITLE.starts_with("com.minecolonies."));
        assert_ne!(TITLE, super::super::crafting::TITLE_KEY);
    }
}
