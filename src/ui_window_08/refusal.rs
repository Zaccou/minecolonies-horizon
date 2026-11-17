//! Pourquoi un clic de ces quatre fenêtres ne fait rien — nommé, et dit au joueur ou pas.
//!
//! **Trois sortes de textes, et les confondre est l'erreur à ne pas faire.** Java
//! *parle* au joueur dans deux cas seulement : `MessageUtils.format(...)` pour un refus
//! du serveur (`AddRemoveRecipeMessage.java:180`) et la même chose côté succès
//! (`:186`). Java *étiquette* un bouton dans un troisième cas, sans rien dire
//! (`WindowBrewingstandCrafting.java:96`) ; et partout ailleurs il **désactive** un
//! bouton ou **refuse** un emplacement en silence. C'est pourquoi [`Refusal::key`] (ce
//! que le joueur lit comme message), [`Refusal::label`] (ce que le bouton affiche) et
//! [`Refusal::name`] (notre nom stable) sont trois fonctions distinctes : imprimer notre
//! nom à la place d'une phrase, ou la clé d'un libellé à la place d'un message, serait
//! mentir au joueur dans sa langue.
//!
//! Le cas qui montre pourquoi c'est utile : le bouton de la brasserie affiche
//! « nombre maximum de recettes atteint » quand le module n'enseigne pas la brasserie du
//! tout. La clé est vraie — c'est bien celle du libellé dans cet état — mais ce n'est pas
//! un plafond de recettes, et ce n'est pas un message.

/// « Nombre maximum de recettes atteint pour ce niveau de hutte »
/// (`TranslationConstants.java:514`, lue à `WindowBrewingstandCrafting.java:96`).
///
/// Le libellé d'un **bouton**, pas un message : voir [`Refusal::label`].
pub const FULL_KEY: &str = "com.minecolonies.coremod.gui.recipe.full";

/// « Impossible de stocker la recette… »
/// (`TranslationConstants.java:274`, dite à `AddRemoveRecipeMessage.java:180`).
pub const UNABLE_KEY: &str = "com.minecolonies.coremod.recipe.unable";

/// « Recette enregistrée ! » (`TranslationConstants.java:616`, dite à
/// `AddRemoveRecipeMessage.java:186`) — le succès, gardé ici pour que le test du refus
/// puisse prouver que les deux clés diffèrent.
pub const SAVED_KEY: &str = "com.minecolonies.coremod.gui.recipe.done";

/// Pourquoi un clic, un dépôt d'objet ou une ouverture a échoué.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les refus de ces quatre fenêtres, un par branche de Java
pub enum Refusal {
    /// La colonie n'est pas attaquée : `!colony.isRaiding()` ferme le bouton
    /// (`WindowsBarracksSpies.java:71`). `isRaiding()` est la copie cliente du drapeau,
    /// écrite depuis `RaiderManager.isRaided()` (`ColonyView.java:364`).
    NotRaiding,
    /// Les espions sont déjà en place : `colony.areSpiesEnabled()`
    /// (`WindowsBarracksSpies.java:71`, `ColonyView.java:1326-1329`).
    SpiesAlreadyEnabled,
    /// Moins de cinq lingots d'or entre le joueur et le rack
    /// (`WindowsBarracksSpies.java:67-71`). Le compte est la somme des deux.
    NotEnoughGold { have: i32, need: i32 },
    /// Le module n'enseigne pas le type `minecolonies:brewing` : `!canLearn(BREWING)`
    /// (`WindowBrewingstandCrafting.java:96`, `:102`, `:113`), et `canLearn` teste
    /// l'**appartenance du type** au module (`CraftingModuleView.java:155-158`), pas un
    /// nombre de recettes.
    CraftingTypeNotTaught,
    /// Aucune recette de brassage ne donne de résultat pour ce contenu : Java garde la
    /// sortie vide après `setCount(3)` et n'envoie rien, sans rien dire
    /// (`WindowBrewingstandCrafting.java:125`).
    NoBrewingRecipe,
    /// Le module a refusé la recette : `module.addRecipe(token)` est faux
    /// (`AddRemoveRecipeMessage.java:177-181`), et Java dit [`UNABLE_KEY`] au joueur.
    RecipeRefusedByModule,
    /// Le module visé n'est pas un module d'artisanat : Java sort en silence
    /// (`AddRemoveRecipeMessage.java:163-166`).
    NotACraftingModule,
    /// L'emplacement d'armure refuse l'objet : `Slot.mayPlace` est faux
    /// (`ContainerCitizenInventory.java:256-270`), et l'objet reste sur le curseur.
    SlotRefusesStack,
    /// Aucune armure n'est acceptée, quel que soit l'objet : le citoyen n'a pas de
    /// bâtiment de travail, ou son bâtiment est de niveau 0, donc Java construit une
    /// liste d'armures **vide** (`WindowCitizenInventory.java:154-162`, `default ->
    /// Collections.emptyList()`) et `mayPlace` répond faux à tout.
    ArmourSlotLocked,
    /// Le bloc ouvert n'a plus d'entité : `(TileEntityColonyBuilding)
    /// getBlockEntity(pos)` (`ContainerBuildingInventory.java:67`) lèverait une exception
    /// côté Java ; le port refuse par ce nom.
    BlockGone,
    /// La colonie ou le citoyen du conteneur a disparu : Java **sort avant d'ajouter le
    /// moindre emplacement** (`ContainerCitizenInventory.java:99-103`), ce qui laisse
    /// `items.size()` à zéro.
    ColonyUnloaded,
    /// L'entité du citoyen n'est pas dans le niveau : `menu.getEntity()` est vide et Java
    /// ne dessine aucun aperçu (`WindowCitizenInventory.java:139`).
    EntityNotLoaded,
    /// Un clic sur un id que la fenêtre n'enregistre pas
    /// (`WindowsBarracksSpies.java:81-95` ne connaît que deux ids).
    UnknownButton,
    /// Une classe de fenêtre que ce module ne décrit pas.
    UnknownWindow,
}

impl Refusal {
    /// Un nom stable, préfixé, qui ne peut pas être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NotRaiding => "ui08.refusal.not_raiding",
            Refusal::SpiesAlreadyEnabled => "ui08.refusal.spies_enabled",
            Refusal::NotEnoughGold { .. } => "ui08.refusal.gold",
            Refusal::CraftingTypeNotTaught => "ui08.refusal.not_taught",
            Refusal::NoBrewingRecipe => "ui08.refusal.no_recipe",
            Refusal::RecipeRefusedByModule => "ui08.refusal.recipe_refused",
            Refusal::NotACraftingModule => "ui08.refusal.not_crafting",
            Refusal::SlotRefusesStack => "ui08.refusal.slot",
            Refusal::ArmourSlotLocked => "ui08.refusal.armour_locked",
            Refusal::BlockGone => "ui08.refusal.block_gone",
            Refusal::ColonyUnloaded => "ui08.refusal.colony_gone",
            Refusal::EntityNotLoaded => "ui08.refusal.no_entity",
            Refusal::UnknownButton => "ui08.refusal.unknown_button",
            Refusal::UnknownWindow => "ui08.refusal.unknown_window",
        }
    }

    /// La clé du **message** que Java envoie au joueur, quand il en envoie un.
    ///
    /// Un seul refus de ce module est dit : l'échec d'enregistrement d'une recette
    /// (`AddRemoveRecipeMessage.java:180`). `None` ne veut pas dire « pas de raison » :
    /// cela veut dire « Java n'a rien dit », et un appelant ne doit donc pas se rabattre
    /// sur [`Self::name`] comme s'il s'agissait d'une phrase.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::RecipeRefusedByModule => Some(UNABLE_KEY),
            Refusal::NotRaiding
            | Refusal::SpiesAlreadyEnabled
            | Refusal::NotEnoughGold { .. }
            | Refusal::CraftingTypeNotTaught
            | Refusal::NoBrewingRecipe
            | Refusal::NotACraftingModule
            | Refusal::SlotRefusesStack
            | Refusal::ArmourSlotLocked
            | Refusal::BlockGone
            | Refusal::ColonyUnloaded
            | Refusal::EntityNotLoaded
            | Refusal::UnknownButton
            | Refusal::UnknownWindow => None,
        }
    }

    /// La clé du **libellé de bouton** dans l'état que ce refus décrit, quand il y en a
    /// un. Ce n'est pas un message : rien n'est envoyé au joueur, le bouton change de
    /// texte (`WindowBrewingstandCrafting.java:96`).
    pub fn label(self) -> Option<&'static str> {
        match self {
            Refusal::CraftingTypeNotTaught => Some(FULL_KEY),
            Refusal::NotRaiding
            | Refusal::SpiesAlreadyEnabled
            | Refusal::NotEnoughGold { .. }
            | Refusal::NoBrewingRecipe
            | Refusal::RecipeRefusedByModule
            | Refusal::NotACraftingModule
            | Refusal::SlotRefusesStack
            | Refusal::ArmourSlotLocked
            | Refusal::BlockGone
            | Refusal::ColonyUnloaded
            | Refusal::EntityNotLoaded
            | Refusal::UnknownButton
            | Refusal::UnknownWindow => None,
        }
    }

    /// Les quatorze refus de ce module, dans l'ordre de la déclaration.
    pub const ALL: [Refusal; 14] = [
        Refusal::NotRaiding,
        Refusal::SpiesAlreadyEnabled,
        Refusal::NotEnoughGold { have: 0, need: 0 },
        Refusal::CraftingTypeNotTaught,
        Refusal::NoBrewingRecipe,
        Refusal::RecipeRefusedByModule,
        Refusal::NotACraftingModule,
        Refusal::SlotRefusesStack,
        Refusal::ArmourSlotLocked,
        Refusal::BlockGone,
        Refusal::ColonyUnloaded,
        Refusal::EntityNotLoaded,
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
            assert!(refusal.name().starts_with("ui08.refusal."), "{refusal}");
            for other in &Refusal::ALL[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(Refusal::ALL.len(), 14);
        // Le nom d'un refus qui porte des données ne dépend pas des données.
        assert_eq!(
            Refusal::NotEnoughGold { have: 1, need: 5 }.name(),
            Refusal::NotEnoughGold { have: 4, need: 9 }.name()
        );
        assert_eq!(
            Refusal::NotEnoughGold { have: 1, need: 5 }.to_string(),
            "ui08.refusal.gold"
        );
    }

    #[test]
    fn exactly_one_refusal_is_spoken_to_the_player() {
        let spoken: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.key().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(spoken, ["ui08.refusal.recipe_refused"]);
        assert_eq!(Refusal::RecipeRefusedByModule.key(), Some(UNABLE_KEY));
        assert_eq!(
            UNABLE_KEY,
            "com.minecolonies.coremod.recipe.unable",
            "TranslationConstants.java:274"
        );
        // Les treize autres sont silencieux : bouton fermé, emplacement qui refuse,
        // ou sortie anticipée du serveur.
        assert_eq!(Refusal::NotRaiding.key(), None);
        assert_eq!(Refusal::NoBrewingRecipe.key(), None);
        assert_eq!(Refusal::ArmourSlotLocked.key(), None);
    }

    #[test]
    fn exactly_one_refusal_relabels_a_button_and_it_is_not_a_message() {
        let labelled: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.label().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(labelled, ["ui08.refusal.not_taught"]);
        assert_eq!(Refusal::CraftingTypeNotTaught.label(), Some(FULL_KEY));
        // Le piège : ce refus n'est pas un plafond de recettes et il n'est pas dit.
        assert_eq!(Refusal::CraftingTypeNotTaught.key(), None);
        assert_ne!(FULL_KEY, UNABLE_KEY);
        assert_ne!(FULL_KEY, SAVED_KEY, "le libellé d'échec et le message de succès");
        assert!(FULL_KEY.starts_with("com.minecolonies."), "{FULL_KEY}");
    }
}
