//! Pourquoi un clic de ces quatre fenêtres ne fait rien — et le fait qu'aucune ne parle.
//!
//! Java reference : `core/client/gui/containers/{WindowCrafting (283 lignes),
//! WindowFurnaceCrafting (195), WindowField (374), WindowGrave (169)}.java`.
//!
//! **Ces quatre classes n'écrivent pas un mot au joueur.** Pas un `MessageUtils.format`,
//! pas un `player.sendSystemMessage`, pas un `Component` de refus : elles désactivent un
//! bouton (`WindowCrafting.java:170`), le rendent invisible (`:178`, `:187`,
//! `WindowFurnaceCrafting.java:132`), ou sortent en silence (`WindowField.java:173`,
//! `:181`, `:214`). [`Refusal::key`] répond donc `None` **quatorze fois sur quatorze**,
//! et c'est un fait de ce groupe de fenêtres, pas un oubli : le seul texte que le joueur
//! voit venir d'un de ces écrans est écrit par le **serveur**, dans
//! `AddRemoveRecipeMessage.java:180` (`UNIQUE_KEY`, déjà portée par
//! [`crate::ui_window_08::refusal::UNABLE_KEY`]) et `:186` (le succès). Un appelant qui
//! veut une phrase va la chercher là ; il ne doit pas se rabattre sur
//! [`Refusal::name`], qui est un nom de code (`ui09.refusal.…`) et jamais une phrase.
//!
//! **Deux refus ne sont pas dans Java, et le disent** : [`Refusal::MatrixTooSmall`] et
//! [`Refusal::FieldSizeDivideByZero`] remplacent deux exceptions — `IndexOutOfBounds`
//! et `ArithmeticException` — que Java laisse échapper du client. Le port refuse au lieu
//! de planter, et ne fait pas semblant que c'était une branche.

/// « Nombre maximum de recettes atteint pour ce niveau de hutte »
/// (`TranslationConstants.java:514`), lu par les deux fenêtres d'artisanat pour
/// **étiqueter** leur bouton quand le module n'enseigne pas le type
/// (`WindowCrafting.java:159`, `WindowFurnaceCrafting.java:115`).
///
/// Le libellé d'un bouton, jamais un message : voir [`Refusal::label`]. La même clé est
/// déjà nommée par [`crate::ui_window_08::refusal::FULL_KEY`], et
/// [`tests::the_full_key_is_the_one_ui_window_08_names`] vérifie que les deux constantes
/// ne peuvent pas diverger.
pub const FULL_KEY: &str = "com.minecolonies.coremod.gui.recipe.full";

/// La clé du bouton « terminé » quand la recette peut être apprise
/// (`BaseGameTranslationConstants.java:8`, lue à `WindowCrafting.java:159` et
/// `WindowFurnaceCrafting.java:115`).
pub const DONE_KEY: &str = "gui.done";

/// Pourquoi un clic, un choix de graine ou une ouverture a échoué.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un refus par branche de ces quatre classes, plus les trois du port
pub enum Refusal {
    /// Le module n'enseigne pas le type demandé. Deux types, deux fenêtres, une même
    /// règle : `!module.canLearn(SMALL_CRAFTING)` ferme le bouton de l'établi
    /// (`WindowCrafting.java:168-171`, et `:225` pour l'envoi) et
    /// `!module.canLearn(SMELTING)` celui du four (`WindowFurnaceCrafting.java:121-124`,
    /// `:141`). `canLearn` teste **l'appartenance du type** au module
    /// (`CraftingModuleView.java:155-158`), pas un nombre de recettes : le libellé qui
    /// parle d'un plafond ([`FULL_KEY`]) ne décrit donc pas ce que Java teste.
    CraftingTypeNotTaught,
    /// Il n'y a rien à enseigner : la sortie lue est vide. Une seule branche de Java pour
    /// les deux fenêtres — `if (!ItemStackUtils.isEmpty(primaryOutput))`
    /// (`WindowCrafting.java:241`, `WindowFurnaceCrafting.java:147`). Le bouton, lui,
    /// reste actif : la fenêtre ne dit rien (`:241`) et n'envoie rien.
    NoResult,
    /// L'entrée du four est vide. **Pas une branche de Java** : `:144` emballe
    /// `container.slots.get(0).getItem()` dans un `ItemStorage` sans regarder, et le
    /// serveur recevrait une recette dont l'entrée unique est vide. Le port refuse ce que
    /// Java enverrait.
    NoFurnaceInput,
    /// La grille de craft est plus petite que ce que la fenêtre va lire. **Pas une
    /// branche de Java, une exception.** Java borne sa boucle sur le drapeau du
    /// **module** (`:229`, `completeCrafting` posé en `:141`) et lit une matrice dont la
    /// taille vient du **conteneur** (`ContainerCrafting.java:115-122`, lue du paquet en
    /// `:94`) : quand les deux désaccordent, `craftMatrix.getItem(i)` dépasse la matrice
    /// et le client lève `IndexOutOfBoundsException`. Le port nomme le désaccord.
    MatrixTooSmall {
        /// Le nombre d'emplacements que la boucle de Java va lire.
        wanted: usize,
        /// Le nombre que la matrice en possède.
        have: usize,
    },
    /// Aucune autre recette pour ce contenu : le bouton de bascule reste **invisible**
    /// (`this.switchButton.visible = this.menu.canSwitchRecipes()`,
    /// `WindowCrafting.java:196`), et `canSwitchRecipes` répond vrai au-delà d'une seule
    /// recette (`ContainerCrafting.java:238-241`). Silencieux.
    NoAlternativeRecipe,
    /// JEI n'est pas chargé : le bouton des requêtes n'existe pas
    /// (`requestsButton.visible = Compatibility.jeiProxy.isLoaded()`,
    /// `WindowCrafting.java:187`, `WindowFurnaceCrafting.java:132`). Silencieux — le
    /// bouton n'est pas grisé, il est en dehors de l'écran.
    JeiMissing,
    /// La colonie n'est pas connue du client : `getCurrentColony()` répond `null`
    /// (`WindowField.java:339-346`) et la fenêtre cache alors l'exploitant, la graine et
    /// la loupe (`:253-256`) au lieu de les écrire. Silencieux.
    NoColony,
    /// Aucune extension de champ ne correspond au bloc de l'épouvantail :
    /// `updateFarmField` laisse `farmField` à `null` (`WindowField.java:237-243`). Les
    /// deux appelants ne s'accordent pas sur ce que cela veut dire — `setSeed` ne fait
    /// rien du tout (`:213`), `selectSeed` déréférence `farmField.getSeed()` (`:157`) et
    /// lève une `NullPointerException`. Le port refuse les deux par ce nom.
    NoFieldForBlock,
    /// Le bouton est fermé : `if (!button.isEnabled()) return;`
    /// (`WindowField.java:171-174`). **Aucune ligne de cette classe ne ferme ces quatre
    /// boutons** : le refus est celui du XML ou d'une autre fenêtre, et il est ici parce
    /// que c'est la première branche que Java teste.
    ButtonDisabled,
    /// L'id du bouton porte une direction qui n'est pas une des quatre
    /// (`WindowField.java:176-182` : `Direction.Plane.HORIZONTAL.stream()…findFirst()`
    /// puis `return` si vide). Silencieux.
    UnknownDirection,
    /// La largeur à écrire n'existe pas : Java calcule
    /// `currentValue % Math.min(currentValue + leftOver, MAX_RANGE)` (`:189`) et lève une
    /// `ArithmeticException` quand le diviseur vaut zéro — c'est-à-dire quand ce côté
    /// est à `0` alors que les quatre côtés totalisent déjà `MAX_RANGE`. Atteignable
    /// seulement depuis un composé bricolé (`TileEntityScarecrow.java:93-97` relit le
    /// tableau sans vérifier sa longueur ni ses valeurs). Le port refuse.
    FieldSizeDivideByZero {
        /// Le côté cliqué, celui dont la largeur ne peut pas être calculée.
        face: crate::network::building_modules::Face,
    },
    /// Un clic sur un id que la fenêtre n'enregistre pas. `WindowGrave` n'en enregistre
    /// **aucun** (`WindowGrave.java:88-100`), donc tout clic y est celui du conteneur.
    UnknownButton,
    /// Un id ou une classe que ce module ne décrit pas.
    UnknownWindow,
}

impl Refusal {
    /// Un nom stable, préfixé, qui ne peut pas être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::CraftingTypeNotTaught => "ui09.refusal.not_taught",
            Refusal::NoResult => "ui09.refusal.no_result",
            Refusal::NoFurnaceInput => "ui09.refusal.no_input",
            Refusal::MatrixTooSmall { .. } => "ui09.refusal.matrix_too_small",
            Refusal::NoAlternativeRecipe => "ui09.refusal.no_alternative",
            Refusal::JeiMissing => "ui09.refusal.no_jei",
            Refusal::NoColony => "ui09.refusal.no_colony",
            Refusal::NoFieldForBlock => "ui09.refusal.no_field",
            Refusal::ButtonDisabled => "ui09.refusal.disabled",
            Refusal::UnknownDirection => "ui09.refusal.unknown_direction",
            Refusal::FieldSizeDivideByZero { .. } => "ui09.refusal.zero_modulus",
            Refusal::UnknownButton => "ui09.refusal.unknown_button",
            Refusal::UnknownWindow => "ui09.refusal.unknown_window",
        }
    }

    /// La clé du **message** que la fenêtre envoie au joueur, quand elle en envoie un.
    ///
    /// Toujours `None` pour ces quatre fenêtres : aucune n'écrit au joueur. `None` ne veut
    /// pas dire « pas de raison », il veut dire « la fenêtre n'a rien dit », et un appelant
    /// ne doit donc pas imprimer [`Self::name`] à la place d'une phrase.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::CraftingTypeNotTaught
            | Refusal::NoResult
            | Refusal::NoFurnaceInput
            | Refusal::MatrixTooSmall { .. }
            | Refusal::NoAlternativeRecipe
            | Refusal::JeiMissing
            | Refusal::NoColony
            | Refusal::NoFieldForBlock
            | Refusal::ButtonDisabled
            | Refusal::UnknownDirection
            | Refusal::FieldSizeDivideByZero { .. }
            | Refusal::UnknownButton
            | Refusal::UnknownWindow => None,
        }
    }

    /// La clé du **libellé de bouton** dans l'état que ce refus décrit, quand il y en a un.
    ///
    /// Un seul refus de ce groupe renomme quelque chose : quand le module n'enseigne pas le
    /// type, le bouton « terminé » s'appelle « plafond de recettes atteint »
    /// (`WindowCrafting.java:159`, `WindowFurnaceCrafting.java:115`). Rien n'est envoyé au
    /// joueur pour autant — [`Self::key`] reste `None`.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Refusal::CraftingTypeNotTaught => Some(FULL_KEY),
            Refusal::NoResult
            | Refusal::NoFurnaceInput
            | Refusal::MatrixTooSmall { .. }
            | Refusal::NoAlternativeRecipe
            | Refusal::JeiMissing
            | Refusal::NoColony
            | Refusal::NoFieldForBlock
            | Refusal::ButtonDisabled
            | Refusal::UnknownDirection
            | Refusal::FieldSizeDivideByZero { .. }
            | Refusal::UnknownButton
            | Refusal::UnknownWindow => None,
        }
    }

    /// Les treize refus de ce module, dans l'ordre de la déclaration.
    pub const ALL: [Refusal; 13] = [
        Refusal::CraftingTypeNotTaught,
        Refusal::NoResult,
        Refusal::NoFurnaceInput,
        Refusal::MatrixTooSmall { wanted: 9, have: 4 },
        Refusal::NoAlternativeRecipe,
        Refusal::JeiMissing,
        Refusal::NoColony,
        Refusal::NoFieldForBlock,
        Refusal::ButtonDisabled,
        Refusal::UnknownDirection,
        Refusal::FieldSizeDivideByZero {
            face: crate::network::building_modules::Face::North,
        },
        Refusal::UnknownButton,
        Refusal::UnknownWindow,
    ];
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        for (index, refusal) in Refusal::ALL.iter().enumerate() {
            assert!(refusal.name().starts_with("ui09.refusal."), "{refusal}");
            for other in &Refusal::ALL[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(Refusal::ALL.len(), 13);
        // Le nom ne dépend pas des données portées.
        assert_eq!(
            Refusal::MatrixTooSmall { wanted: 9, have: 4 }.name(),
            Refusal::MatrixTooSmall { wanted: 4, have: 0 }.name()
        );
        assert_eq!(
            Refusal::MatrixTooSmall { wanted: 9, have: 4 }.to_string(),
            "ui09.refusal.matrix_too_small"
        );
        // Et il ne s'écrit pas « ui08 » : les deux modules ont leurs noms à eux.
        for refusal in Refusal::ALL {
            assert!(!refusal.name().starts_with("ui08."), "{refusal}");
        }
    }

    #[test]
    fn not_one_of_these_four_windows_speaks_to_the_player() {
        let spoken: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.key().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert!(spoken.is_empty(), "aucune de ces quatre classes n'écrit au joueur");
        // Le seul texte du groupe vient du serveur, et il est déjà nommé ailleurs.
        assert_eq!(
            crate::ui_window_08::refusal::UNABLE_KEY,
            "com.minecolonies.coremod.recipe.unable",
            "AddRemoveRecipeMessage.java:180"
        );
        assert_eq!(Refusal::NoResult.key(), None);
        assert_eq!(Refusal::CraftingTypeNotTaught.key(), None);
    }

    #[test]
    fn one_refusal_relabels_a_button_and_it_is_not_a_message() {
        let labelled: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.label().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(labelled, ["ui09.refusal.not_taught"]);
        assert_eq!(Refusal::CraftingTypeNotTaught.label(), Some(FULL_KEY));
        // Le piège du groupe : la clé est vraie, le plafond ne l'est pas.
        assert_ne!(FULL_KEY, DONE_KEY);
        assert_ne!(FULL_KEY, crate::ui_window_08::refusal::UNABLE_KEY);
        for key in [FULL_KEY, DONE_KEY] {
            assert!(!key.is_empty());
        }
        assert!(FULL_KEY.starts_with("com.minecolonies."), "{FULL_KEY}");
        assert_eq!(DONE_KEY, "gui.done", "une clé du jeu de base, pas de MineColonies");
    }

    #[test]
    fn the_full_key_is_the_one_ui_window_08_names() {
        // Une seule clé, deux modules : le test l'épingle pour qu'ils ne divergent pas.
        assert_eq!(FULL_KEY, crate::ui_window_08::refusal::FULL_KEY);
        assert_eq!(
            crate::ui_window_08::refusal::Refusal::CraftingTypeNotTaught.label(),
            Some(FULL_KEY),
            "la brasserie et l'établi étiquettent leur bouton avec la même clé"
        );
    }
}
