//! `WindowCrafting` : un établi 2×2 ou 3×3, un bouton « terminé », une bascule qui
//! n'existe pas toujours.
//!
//! Java reference : `core/client/gui/containers/WindowCrafting.java` (283 lignes),
//! `api/inventory/container/ContainerCrafting.java:107-250` (la matrice, le résultat,
//! la bascule), `core/colony/buildings/moduleviews/CraftingModuleView.java:155-158`
//! (`canLearn`) et `api/crafting/ModCraftingTypes.java:11-15` (les cinq types enseignés).
//!
//! # L'écran n'a aucun panneau nommé, et un seul texte
//!
//! C'est un `AbstractContainerScreen` (`:40`), pas une `BOWindow` : pas de XML BlockUI,
//! pas de `findPaneOfTypeByID`. Le seul texte de la fenêtre est
//! `stack.drawString(font, Component.translatable("container.crafting").getString(), 97, 8,
//! 4_210_752, false)` (`:255`) — un **appel de dessin** avec la clé `container.crafting`
//! écrite en dur, qui **ignore le nom que le serveur a mis dans le paquet** (« Crafting
//! GUI », `OpenCraftingGUIMessage.java:110`). Deux noms existent, un seul est dessiné, et
//! le dessiné n'est pas celui du paquet : c'est [`super::Title::Key`], et [`FIELDS`] est
//! vide parce qu'un panneau sans id n'est pas un panneau.
//!
//! # `canLearn` n'est pas un compte de recettes, c'est une appartenance
//!
//! Le bouton s'appelle `WARNING_MAXIMUM_NUMBER_RECIPES` (« nombre maximum de recettes
//! atteint », `TranslationConstants.java:514`) quand `!module.canLearn(SMALL_CRAFTING)`
//! (`:159`), et `canLearn` répond `getSupportedCraftingTypes().contains(type)`
//! (`CraftingModuleView.java:157`). Un module qui n'enseigne pas l'établi affiche donc un
//! plafond alors que la question posée était « enseignes-tu ce type ? ». La même clé sert
//! à la brasserie (`WindowBrewingstandCrafting.java:96`, `crate::ui_window_08`).
//!
//! # Deux drapeaux pour une seule grille, et le désaccord plante
//!
//! La **taille** lue vient du conteneur : `new TransientCraftingContainer(this, 3, 3)` ou
//! `(2, 2)` selon le booléen du paquet (`ContainerCrafting.java:115-122`, lu en `:94`).
//! La **borne de la boucle** vient du module : `completeCrafting` posé par
//! `module.canLearn(LARGE_CRAFTING)` (`:141`), et la boucle
//! `for (int i = 0; i < (completeCrafting ? MAX_CRAFTING_GRID_SIZE : CRAFTING_GRID_SIZE); i++)`
//! (`:229`). Quand le module a changé entre l'ouverture du conteneur et le clic,
//! `craftMatrix.getItem(i)` lit au-delà de la matrice et le client lève
//! `IndexOutOfBoundsException` : le port refuse par [`Refusal::MatrixTooSmall`] et dit
//! combien il lui fallait.

use super::pane::Pane;
use super::refusal::{Refusal, DONE_KEY, FULL_KEY};
use super::{Button, Effect, Naming, Storage};
use crate::crafting::Teaching;

/// La classe Java.
pub const JAVA: &str = "WindowCrafting";
/// La clé du seul texte que la classe dessine (`:255`) — écrite en dur, ignorée du paquet.
pub const TITLE_KEY: &str = "container.crafting";
/// La texture 2×2 (`:42`).
pub const TEXTURE_SMALL: &str = "minecolonies:textures/gui/crafting2x2.png";
/// La texture 3×3 (`:44`).
pub const TEXTURE_LARGE: &str = "minecolonies:textures/gui/crafting3x3.png";
/// Les quatre cases d'un établi 2×2 (`:100`).
pub const SMALL_SLOTS: usize = 4;
/// Les neuf cases d'un établi 3×3 (`:105`).
pub const LARGE_SLOTS: usize = 9;
/// La taille de grille envoyée pour un 2×2 (`:244`, `completeCrafting ? 3 : 2`).
pub const SMALL_GRID: i32 = 2;
/// La taille de grille envoyée pour un 3×3 (`:244`).
pub const LARGE_GRID: i32 = 3;
/// Chaque entrée part avec **un** exemplaire : `ItemStackUtils.setSize(copy, 1)` (`:233`)
/// avant `new ItemStorage(copy)` (`:235`), dont le montant est `getSize(stack)`.
pub const INPUT_COUNT: i32 = 1;
/// Le bouton « terminé » : Java le crée avec `Button.builder(...)` (`:163`), sans id.
pub const BUTTON_DONE: &str = "done";
/// La bascule de recette : un `ImageButton` (`:173`), sans id.
pub const BUTTON_SWITCH: &str = "switch";
/// La liste des requêtes : un `ImageButton` (`:181`), sans id.
pub const BUTTON_REQUESTS: &str = "requests";

/// Aucun panneau nommé.
///
/// `WindowCrafting` est un `AbstractContainerScreen` : ses cases sont ajoutées par
/// `ContainerCrafting` (`:124-190`), son titre est un `drawString` (`:255`) et son seul
/// autre texte est l'étiquette du bouton, qui dépend de l'état ([`State::label`]). Un
/// appelant qui veut les cases part de `ContainerCrafting.java:124-190` : `src/ui/MANQUANT.md`
/// §1 dit quel widget manque pour les dessiner.
pub const FIELDS: &[Pane] = &[];

/// Les trois boutons de la fenêtre, dans l'ordre où `init` les ajoute (`:163`, `:173`,
/// `:181`).
pub const BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_DONE,
        label: DONE_KEY,
        effect: Effect::Sends("AddRemoveRecipeMessage"),
        refusals: &[Refusal::CraftingTypeNotTaught, Refusal::NoResult],
        naming: Naming::Ours,
    },
    Button {
        id: BUTTON_SWITCH,
        label: "",
        effect: Effect::Sends("SwitchRecipeCraftingTeachingMessage"),
        refusals: &[Refusal::NoAlternativeRecipe],
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

/// Ce que la fenêtre lit pour décider : les types que le module enseigne, de quoi basculer,
/// JEI, et l'id du producteur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// `module.getSupportedCraftingTypes()` — ce que `canLearn` teste
    /// (`CraftingModuleView.java:155-158`). Le type est celui de
    /// [`crate::crafting::Teaching`], donc les cinq ids ne sont écrits qu'une fois.
    pub teaches: &'static [Teaching],
    /// `menu.canSwitchRecipes()` (`:196`) : plus d'une recette pour ce contenu.
    pub switchable: bool,
    /// `Compatibility.jeiProxy.isLoaded()` (`:187`) : sans JEI, le bouton n'est pas là.
    pub jei: bool,
    /// `module.getProducer().getRuntimeID()` (`:244`), que le message porte tel quel.
    pub module_id: i32,
}

impl State {
    /// L'état d'ouverture.
    pub const fn new(
        teaches: &'static [Teaching],
        switchable: bool,
        jei: bool,
        module_id: i32,
    ) -> State {
        State { teaches, switchable, jei, module_id }
    }

    /// `module.canLearn(type)` — une **appartenance**, pas un compte
    /// (`CraftingModuleView.java:155-158`).
    pub fn can_learn(self, teaching: Teaching) -> bool {
        self.teaches.contains(&teaching)
    }

    /// Le libellé du bouton « terminé » (`:159`) : `gui.done`, ou la clé du « plafond ».
    pub fn label(self) -> &'static str {
        if self.can_learn(Teaching::SmallCrafting) {
            DONE_KEY
        } else {
            FULL_KEY
        }
    }

    /// L'état du bouton : Java le ferme exactement quand `SMALL_CRAFTING` n'est pas
    /// enseigné (`:168-171`).
    pub fn done_enabled(self) -> bool {
        self.can_learn(Teaching::SmallCrafting)
    }

    /// La taille de grille que la fenêtre envoie (`:244`), et qu'elle croit avoir.
    pub fn grid(self) -> i32 {
        if self.can_learn(Teaching::LargeCrafting) {
            LARGE_GRID
        } else {
            SMALL_GRID
        }
    }

    /// Le nombre de cases que la boucle de Java va lire (`:229`).
    pub fn matrix_slots(self) -> usize {
        if self.can_learn(Teaching::LargeCrafting) {
            LARGE_SLOTS
        } else {
            SMALL_SLOTS
        }
    }

    /// La bascule est **invisible** tant qu'il n'y a pas deux recettes (`:196`) : ce n'est
    /// pas un bouton grisé, c'est un bouton qui n'est pas dessiné (`:178` la pose
    /// invisible à la construction, `containerTick` la révèle).
    pub fn switch_visible(self) -> bool {
        self.switchable
    }

    /// Le bouton des requêtes suit JEI (`:187`).
    pub fn requests_visible(self) -> bool {
        self.jei
    }

    /// La texture que `renderBg` choisit (`:261-274`).
    pub fn texture(self) -> &'static str {
        if self.can_learn(Teaching::LargeCrafting) {
            TEXTURE_LARGE
        } else {
            TEXTURE_SMALL
        }
    }

    /// Le clic sur « terminé » (`:223-247`), et le refus nommé quand rien n'est envoyé.
    ///
    /// `matrix` est la matrice du conteneur, **dans l'ordre des cases** ; l'appelant fournit
    /// `None` pour une case vide — Java en fait un `ItemStorage` de pile vide et l'envoie
    /// quand même (`:229-236`). `output` est la sortie du résultat (`:238`) et `secondary`
    /// les sorties secondaires (`:239`, `menu.getRemainingItems()`), que le port ne sait
    /// pas calculer : le registre de recettes est celui du jeu (`MANQUANT.md` §0).
    pub fn done(
        self,
        matrix: &[Option<&'static str>],
        output: Option<&'static str>,
        secondary: &[Option<&'static str>],
    ) -> Result<Storage, Refusal> {
        // `:225` : tout le clic est sous `if (module.canLearn(SMALL_CRAFTING))`.
        if !self.can_learn(Teaching::SmallCrafting) {
            return Err(Refusal::CraftingTypeNotTaught);
        }
        let wanted = self.matrix_slots();
        if matrix.len() < wanted {
            // `:229` lirait au-delà : voir la note du module.
            return Err(Refusal::MatrixTooSmall { wanted, have: matrix.len() });
        }
        let Some(output) = output else {
            // `:241` : `!ItemStackUtils.isEmpty(primaryOutput)`. Rien n'est envoyé et rien
            // n'est dit ; le bouton, lui, est resté actif.
            return Err(Refusal::NoResult);
        };
        Ok(Storage {
            inputs: matrix[..wanted].to_vec(),
            input_count: INPUT_COUNT,
            primary: output,
            secondary: secondary.to_vec(),
            grid: self.grid(),
            // `AddRemoveRecipeMessage.java:96` : `gridSize == 1 ? FURNACE : AIR`. Une grille
            // de 2 ou 3 n'a donc aucun bloc intermédiaire.
            intermediate: None,
            module_id: self.module_id,
            remove: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL_ONLY: &[Teaching] = &[Teaching::SmallCrafting];
    const BOTH: &[Teaching] = &[Teaching::SmallCrafting, Teaching::LargeCrafting];
    const LARGE_ONLY: &[Teaching] = &[Teaching::LargeCrafting];
    const SMELTING_ONLY: &[Teaching] = &[Teaching::Smelting];

    fn small() -> State {
        State::new(SMALL_ONLY, false, false, 4)
    }

    fn large() -> State {
        State::new(BOTH, true, true, 4)
    }

    #[test]
    fn the_grid_follows_large_crafting_and_the_slots_follow_the_grid() {
        assert_eq!(small().grid(), SMALL_GRID);
        assert_eq!(small().matrix_slots(), SMALL_SLOTS);
        assert_eq!(large().grid(), LARGE_GRID);
        assert_eq!(large().matrix_slots(), LARGE_SLOTS);
        // Les deux nombres vont ensemble : trois de côté, neuf cases.
        assert_eq!(LARGE_GRID as usize * LARGE_GRID as usize, LARGE_SLOTS);
        assert_eq!(SMALL_GRID as usize * SMALL_GRID as usize, SMALL_SLOTS);
        assert_eq!(small().texture(), TEXTURE_SMALL);
        assert_eq!(large().texture(), TEXTURE_LARGE);
        assert_ne!(TEXTURE_SMALL, TEXTURE_LARGE);
        // Sans SMALL mais avec LARGE, la fenêtre croit quand même à une grille 3×3 :
        // c'est Java, qui ne teste que LARGE_CRAFTING pour cela (`:141`).
        let large_only = State::new(LARGE_ONLY, false, false, 4);
        assert_eq!(large_only.grid(), LARGE_GRID);
        assert!(!large_only.done_enabled(), "mais le bouton est fermé, lui");
    }

    #[test]
    fn the_done_button_reads_small_crafting_and_nothing_else() {
        assert_eq!(small().label(), DONE_KEY);
        assert!(small().done_enabled());
        let untutored = State::new(SMELTING_ONLY, false, false, 7);
        assert_eq!(untutored.label(), FULL_KEY);
        assert!(!untutored.done_enabled());
        // Le refus qui l'explique porte le libellé, et rien d'autre.
        assert_eq!(Refusal::CraftingTypeNotTaught.label(), Some(FULL_KEY));
        // `canLearn` est une appartenance : enseigner le four ne suffit pas.
        assert!(!untutored.can_learn(Teaching::SmallCrafting));
        assert!(untutored.can_learn(Teaching::Smelting));
        // Et le clic refuse avant même de regarder la matrice.
        assert_eq!(untutored.done(&[], None, &[]), Err(Refusal::CraftingTypeNotTaught));
    }

    #[test]
    fn a_matrix_shorter_than_the_loop_is_refused_instead_of_read_past() {
        let four = [Some("minecraft:oak_planks"); 4];
        let nine = [Some("minecraft:oak_planks"); 9];
        // Le module dit 3×3, la matrice en a quatre : Java lirait jusqu'à neuf.
        assert_eq!(
            large().done(&four, Some("minecraft:crafting_table"), &[]),
            Err(Refusal::MatrixTooSmall { wanted: LARGE_SLOTS, have: SMALL_SLOTS })
        );
        assert!(large().done(&nine, Some("minecraft:crafting_table"), &[]).is_ok());
        // Le cas inverse ne plante pas : Java lit les quatre **premières** cases d'une
        // matrice 3×3 et envoie une grille de 2, sans rien dire. Le port le laisse passer.
        let ok = small().done(&nine, Some("minecraft:crafting_table"), &[]);
        match ok {
            Ok(storage) => {
                assert_eq!(storage.grid, SMALL_GRID);
                assert_eq!(storage.inputs.len(), SMALL_SLOTS, "quatre cases lues, pas neuf");
            }
            Err(refusal) => panic!("un 3×3 lu par un module 2×2 ne doit pas être refusé : {refusal}"),
        }
    }

    #[test]
    fn an_empty_result_sends_nothing_and_leaves_the_button_active() {
        let four = [Some("minecraft:oak_planks"); 4];
        assert_eq!(small().done(&four, None, &[]), Err(Refusal::NoResult));
        assert_eq!(Refusal::NoResult.key(), None, "la fenêtre ne dit rien de cet échec");
        assert!(small().done_enabled(), "le bouton n'est pas fermé pour autant");
        assert_eq!(small().label(), DONE_KEY);
    }

    #[test]
    fn a_full_recipe_carries_one_of_each_in_screen_order() {
        let matrix = [
            Some("minecraft:oak_planks"),
            None,
            Some("minecraft:oak_planks"),
            Some("minecraft:stick"),
            None,
            None,
            None,
            None,
            Some("minecraft:iron_ingot"),
        ];
        let storage = match large().done(&matrix, Some("minecraft:crafting_table"), &[Some("minecraft:stick")]) {
            Ok(storage) => storage,
            Err(refusal) => panic!("une recette complète a été refusée : {refusal}"),
        };
        assert_eq!(storage.grid, LARGE_GRID);
        assert_eq!(storage.input_count, 1, ":233 setSize(copy, 1)");
        assert_eq!(storage.primary, "minecraft:crafting_table");
        assert_eq!(storage.module_id, 4, ":244, le producteur du module");
        assert!(!storage.remove);
        assert_eq!(storage.intermediate, None, "AddRemoveRecipeMessage.java:96 : AIR");
        // Neuf entrées, les vides comprises, dans l'ordre des cases.
        assert_eq!(storage.inputs.len(), LARGE_SLOTS);
        assert_eq!(storage.inputs[0], Some("minecraft:oak_planks"));
        assert_eq!(storage.inputs[1], None);
        assert_eq!(storage.inputs[8], Some("minecraft:iron_ingot"));
        assert_eq!(storage.secondary, [Some("minecraft:stick")]);
    }

    #[test]
    fn two_buttons_hide_themselves_and_one_refusal_is_the_reason_for_each() {
        assert_eq!(BUTTONS.len(), 3);
        let switch = BUTTONS.iter().find(|button| button.id == BUTTON_SWITCH);
        let requests = BUTTONS.iter().find(|button| button.id == BUTTON_REQUESTS);
        let done = BUTTONS.iter().find(|button| button.id == BUTTON_DONE);
        let (Some(switch), Some(requests), Some(done)) = (switch, requests, done) else {
            panic!("les trois boutons ne sont pas tous dans la table");
        };
        // `:196` et `:187` : deux boutons cachés, deux refus muets.
        assert_eq!(switch.refusals, &[Refusal::NoAlternativeRecipe][..]);
        assert_eq!(requests.refusals, &[Refusal::JeiMissing][..]);
        assert_eq!(Refusal::NoAlternativeRecipe.key(), None);
        assert_eq!(Refusal::JeiMissing.key(), None);
        assert!(!small().switch_visible());
        assert!(large().switch_visible());
        assert!(!small().requests_visible());
        assert!(large().requests_visible());
        // La bascule envoie un message **sans corps** : `toBytes` est vide (`:29-31`).
        assert_eq!(
            switch.effect,
            Effect::Sends("SwitchRecipeCraftingTeachingMessage")
        );
        assert_eq!(requests.effect, Effect::OpensJava("WindowSelectRequest"));
        assert_eq!(done.label, DONE_KEY);
        // Aucun des trois n'a d'id dans Java : ce sont les nôtres.
        for button in BUTTONS {
            assert_eq!(button.naming, Naming::Ours, "{} est créé par du code", button.id);
        }
    }
}
