//! `GatherDataHandler.java`: the one listener that never runs in a game.
//!
//! Java reference: `GatherDataHandler.java`, 73 lines, **41 providers** added to a
//! `DataGenerator` under the `runData` Gradle task (`:14-18`: "this does not run
//! during normal client operations, only during building"). It is the only file of
//! the fourteen whose side is [`super::table::Side::Datagen`], and the only one the
//! port answers with "no hook can carry this, and none should".
//!
//! **Why it is still in the table.** A port has to say what it does with each branch,
//! and "nothing, at runtime" is an answer only if it names the build step that
//! replaces it. Horizon has none: `cargo horizon-mod` packages `data/` and `assets/`
//! as written. So the port **ships** the JSON this handler would have generated, and
//! the generator's own value — that a resource pack's JSON is regenerated from Java
//! constants — is lost. `MANQUANT.md` says so with the primitive (`a datagen step in
//! the mod toolchain`), and this file keeps the list, because a later datagen step
//! will need it.
//!
//! **The two `includeClient() && includeServer()` rows are not a category mistake.**
//! `QuestTranslationProvider` (`:31`) and `ItemNbtCalculator` (`:71`) are added only
//! when the run generates both sides, because the first writes translations the
//! client reads and ids the server resolves, and the second's NBT depends on the tags
//! the server side emits. A generator that added them unconditionally would write a
//! file whose references do not resolve.

/// Which half of a datagen run a provider belongs to (`event.includeClient()` /
/// `event.includeServer()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Client,
    Server,
    /// `includeClient() && includeServer()` (`:31`, `:71`).
    Both,
}

/// One `generator.addProvider` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    /// The class the Java constructs.
    pub name: &'static str,
    /// The half it is added to.
    pub half: Half,
}

/// The one thing the handler does before generating (`:21`).
///
/// `SchemFixerUtil.fixSchematics()` rewrites the shipped schematics in place. It runs
/// on the developer's checkout and has no runtime meaning; the port copies the
/// schematics verbatim instead.
pub const PRE_STEP: &str = "SchemFixerUtil.fixSchematics()";

/// The 41 providers, in the order `:27-71` adds them.
pub const PROVIDERS: &[Provider] = &[
    // client, `:27-30`
    Provider { name: "DefaultSoundProvider", half: Half::Client },
    Provider { name: "DefaultItemModelProvider", half: Half::Client },
    Provider { name: "DefaultEntityIconProvider", half: Half::Client },
    Provider { name: "DefaultStoriesProvider", half: Half::Client },
    // both, `:31`
    Provider { name: "QuestTranslationProvider", half: Half::Both },
    // server, `:33-46`
    Provider { name: "DefaultBlockLootTableProvider", half: Half::Server },
    Provider { name: "DefaultEntityLootProvider", half: Half::Server },
    Provider { name: "DefaultSupplyLootProvider", half: Half::Server },
    Provider { name: "DefaultDamageTypeProvider", half: Half::Server },
    Provider { name: "DefaultAdvancementsProvider", half: Half::Server },
    Provider { name: "DefaultBlockTagsProvider", half: Half::Server },
    Provider { name: "DefaultItemTagsProvider", half: Half::Server },
    Provider { name: "DefaultEntityTypeTagsProvider", half: Half::Server },
    Provider { name: "DefaultDamageTagsProvider", half: Half::Server },
    Provider { name: "DefaultResearchProvider", half: Half::Server },
    Provider { name: "DefaultRecipeProvider", half: Half::Server },
    Provider { name: "DefaultBiomeTagsProvider", half: Half::Server },
    Provider { name: "DefaultLuckyOreLootProvider", half: Half::Server },
    Provider { name: "DefaultRecruitmentItemsProvider", half: Half::Server },
    // the workers, one crafting provider each, `:49-69`
    Provider { name: "DefaultRecipeLootProvider", half: Half::Server },
    Provider { name: "DefaultAlchemistCraftingProvider", half: Half::Server },
    Provider { name: "DefaultBakerCraftingProvider", half: Half::Server },
    Provider { name: "DefaultBlacksmithCraftingProvider", half: Half::Server },
    Provider { name: "DefaultConcreteMixerCraftingProvider", half: Half::Server },
    Provider { name: "DefaultChefCraftingProvider", half: Half::Server },
    Provider { name: "DefaultCrusherCraftingProvider", half: Half::Server },
    Provider { name: "DefaultDyerCraftingProvider", half: Half::Server },
    Provider { name: "DefaultEnchanterCraftingProvider", half: Half::Server },
    Provider { name: "DefaultFarmerCraftingProvider", half: Half::Server },
    Provider { name: "DefaultFishermanLootProvider", half: Half::Server },
    Provider { name: "DefaultFletcherCraftingProvider", half: Half::Server },
    Provider { name: "DefaultGlassblowerCraftingProvider", half: Half::Server },
    Provider { name: "DefaultLumberjackCraftingProvider", half: Half::Server },
    Provider { name: "DefaultMechanicCraftingProvider", half: Half::Server },
    Provider { name: "DefaultNetherWorkerLootProvider", half: Half::Server },
    Provider { name: "DefaultPlanterCraftingProvider", half: Half::Server },
    Provider { name: "DefaultSawmillCraftingProvider", half: Half::Server },
    Provider { name: "DefaultSifterCraftingProvider", half: Half::Server },
    Provider { name: "DefaultStonemasonCraftingProvider", half: Half::Server },
    Provider { name: "DefaultStoneSmelteryCraftingProvider", half: Half::Server },
    // both, added last, `:71`: its NBT depends on the tags the server providers write
    Provider { name: "ItemNbtCalculator", half: Half::Both },
];

/// Which providers a run generates, given the two flags (`:27-71`).
pub fn selected(include_client: bool, include_server: bool) -> Vec<&'static str> {
    PROVIDERS
        .iter()
        .filter(|provider| match provider.half {
            Half::Client => include_client,
            Half::Server => include_server,
            Half::Both => include_client && include_server,
        })
        .map(|provider| provider.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handler_adds_forty_one_providers() {
        assert_eq!(PROVIDERS.len(), 41);
    }

    #[test]
    fn the_halves_are_four_client_two_both_and_thirty_five_server() {
        let count = |half: Half| PROVIDERS.iter().filter(|p| p.half == half).count();
        assert_eq!(count(Half::Client), 4);
        assert_eq!(count(Half::Both), 2);
        assert_eq!(count(Half::Server), 35);
        assert_eq!(4 + 2 + 35, PROVIDERS.len());
    }

    #[test]
    fn a_client_only_run_writes_no_loot_table() {
        let names = selected(true, false);
        assert!(names.contains(&"DefaultStoriesProvider"));
        assert!(!names.contains(&"DefaultBlockLootTableProvider"));
        assert!(
            !names.contains(&"QuestTranslationProvider"),
            "both-sided providers need both halves"
        );
    }

    #[test]
    fn a_server_only_run_writes_no_sound_json() {
        let names = selected(false, true);
        assert!(names.contains(&"DefaultRecipeProvider"));
        assert!(!names.contains(&"DefaultSoundProvider"));
        assert!(!names.contains(&"ItemNbtCalculator"));
    }

    #[test]
    fn a_full_run_generates_every_provider_exactly_once() {
        let names = selected(true, true);
        assert_eq!(names.len(), PROVIDERS.len());
        let mut sorted = names.clone();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "a provider is added twice");
    }

    #[test]
    fn the_workers_craft_eighteen_times_and_loot_three_times() {
        let crafting = PROVIDERS
            .iter()
            .filter(|provider| provider.name.ends_with("CraftingProvider"))
            .count();
        assert_eq!(crafting, 18, "measured on `:50-69`");
        // `DefaultRecipeLootProvider`, `DefaultFishermanLootProvider` and
        // `DefaultNetherWorkerLootProvider` are the three workers whose file is a
        // loot table rather than a recipe.
        let worker_loot: Vec<&Provider> = PROVIDERS
            .iter()
            .filter(|provider| {
                provider.name.ends_with("LootProvider")
                    && !provider.name.starts_with("DefaultEntity")
                    && !provider.name.starts_with("DefaultSupply")
            })
            .collect();
        assert_eq!(worker_loot.len(), 4, "3 workers, plus DefaultLuckyOreLootProvider");
    }
}
