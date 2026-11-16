//! The two registrations that stop the recipe table failing to load.
//!
//! **What breaks without them.** Horizon resolves a recipe file's `type` to a
//! registered serializer before anything else, and an unknown one does not skip
//! that file — it refuses the **whole** table
//! (`crates/horizon-client/src/crafting.rs:1622-1632` through
//! `crafting/mod_kinds.rs:24-43`, "unregistered recipe serializer"). That is why
//! the four files whose types nobody registers sit in `recettes-custom/` instead
//! of `data/minecolonies/recipe/`: one of them inside the table would cost the
//! other 154 recipes.
//!
//! Java registers exactly this much
//! (`apiimp/initializer/ModRecipeSerializerInitializer.java:19-25`):
//!
//! | Name | Serializer | Its recipe type |
//! | --- | --- | --- |
//! | `minecolonies:composting` | `CompostRecipe.Serializer` | `minecolonies:composting` — its **own** type (`:22`) |
//! | `minecolonies:zero_waste` | `ZeroWasteRecipe.Serializer` | `minecraft:crafting` — none of its own, it extends `ShapelessRecipe` (`:24`) |
//!
//! **Both are registered raw, and that is a measured limitation, not a choice.**
//! `register_raw_recipe_serializer` keeps each file's JSON as text and matches
//! nothing (`crates/horizon-mod-vocab/src/parts/recipe_serializers.rs:36-52`,
//! `Raw::matches` → `false`). What that buys is the one thing that is fatal
//! today: **the four files load**. What it does not buy is behaviour — a
//! `zero_waste` recipe will not match a crafting grid and its leftovers will not
//! vanish, and a `composting` file still needs a barrel that reads it. Both need a
//! `CraftingRecipeKind` with a Wire codec for its decoded value
//! (`parts/crafting_recipes.rs:79-106`), which is the next step and is written
//! down in `src/crafting/MANQUANT.md`.
//!
//! **Who calls [`register`].** `Mod::register` in `src/lib.rs`
//! (`Hooks::register`, `src/lib.rs:85-96`) — the one hook a mod declares parts in.
//! This module cannot reach it: `lib.rs` belongs to whoever wires the package
//! together, not to `src/crafting/`.

use super::Kind;
use mod_api::{RecipeSerializersRegistrar, Registrar};

/// The serializer name, and the recipe type, of `minecolonies:composting`:
/// one string, because Java registers the same path twice
/// (`ModRecipeSerializerInitializer.java:21-22`).
pub const COMPOSTING: &str = "minecolonies:composting";

/// The serializer name of `minecolonies:zero_waste`
/// (`ModRecipeSerializerInitializer.java:24`).
pub const ZERO_WASTE: &str = "minecolonies:zero_waste";

/// The recipe type `zero_waste` files answer to: vanilla's crafting, because
/// `ShapelessRecipe.getType()` already says so and Java registers no type of its
/// own for it.
pub const CRAFTING: &str = "minecraft:crafting";

/// Declare the two recipe types this port reads.
///
/// Call once from `Hooks::register`. Registering a name twice is refused by the
/// engine at startup, naming the mod, so this is a one-shot call.
pub fn register(registrar: &mut dyn Registrar) {
    // The composting type first: `register_raw_recipe_serializer` wants its
    // recipe type declared (`parts/recipe_serializers.rs:65-69`).
    registrar.register_recipe_type(COMPOSTING);
    registrar.register_raw_recipe_serializer(COMPOSTING, COMPOSTING);
    registrar.register_raw_recipe_serializer(ZERO_WASTE, CRAFTING);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registered_names_are_the_types_the_files_write() {
        assert_eq!(COMPOSTING, Kind::Composting.type_name());
        assert_eq!(ZERO_WASTE, Kind::ZeroWaste.type_name());
    }

    #[test]
    fn the_two_names_belong_to_the_namespace_the_package_declares() {
        // `horizon-mod.toml` declares `namespaces = ["minecolonies"]`, and a mod
        // cannot register a name outside it.
        assert_eq!(COMPOSTING.split_once(':').map(|(ns, _)| ns), Some("minecolonies"));
        assert_eq!(ZERO_WASTE.split_once(':').map(|(ns, _)| ns), Some("minecolonies"));
        assert_eq!(CRAFTING, "minecraft:crafting");
    }

    #[test]
    fn the_two_serializers_do_not_share_a_recipe_type() {
        assert_ne!(
            COMPOSTING, CRAFTING,
            "composting declares a type of its own: ModRecipeSerializerInitializer.java:22"
        );
        assert!(
            !Kind::Composting.craftable(),
            "and that is why it has no crafting behaviour to give"
        );
        assert!(Kind::ZeroWaste.craftable());
    }
}
