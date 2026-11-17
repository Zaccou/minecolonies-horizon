//! Three managers of Java's tree that answer a question no other module answers,
//! and a fourth that was already ported.
//!
//! Java reference: `core/quests/QuestManager.java` (367 lines),
//! `core/generation/DatagenLootTableManager.java` (71),
//! `core/debug/DebugPlayerManager.java` (70) and
//! `core/research/ResearchEffectManager.java` (53) — 561 lines of Java, of which
//! this module ports the rules and none of the world.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | [`quests`] | which quests a colony may start, which one it holds, how often each was finished, and what a save round-trips | `QuestManager.java:27-366` |
//! | [`loot`] | where a datagen loot table is read, and what a miss costs | `DatagenLootTableManager.java:41-70` |
//! | [`debug`] | who has debug on, and what a toggle-off forgets | `DebugPlayerManager.java:19-69` |
//! | [`nbt`] | the four tag reads `quests` needs | five other copies |
//!
//! **`ResearchEffectManager` is already ported, and this module does not repeat
//! it — the fourth question of the brief has an answer, not a file.** Its whole
//! content is one map from effect id to strength, `getEffectStrength` answering
//! `0` for an absent id (`:21-30`), `applyEffect` keeping the **stronger by
//! magnitude** because `GlobalResearchEffect.overrides` compares `Math.abs`
//! (`:32-45`, `GlobalResearchEffect.java:130-133`), and `removeAllEffects`
//! (`:49-52`). That is `crate::research::progress::Effects`, written before this
//! module and used by every research consumer: `strength`, `has`, `apply`,
//! `apply_research`, `rebuild` (which cites `resetEffects`,
//! `LocalResearchTree.java:318-341`) and `clear` (which cites `:57-60`). It even
//! keeps the two facts Java's 53 lines hide — an effect present at strength `0`
//! is not the same as an absent one, and an **equal** magnitude keeps the one
//! already held, because Java's comparison is strict. Adding a second `Effects`
//! here would be the duplication the brief forbids, so the file is not written.
//!
//! **Nothing here is wired to a hook, and Java calls each of the three from a
//! different place.** `QuestManager` is created per colony (`Colony.java:366`),
//! ticked ninth in `worldTickSlow` (`:497`, every 500 ticks —
//! [`crate::manager_06::COLONY_TICK`]), saved under `quest_manager` (`:955`) and
//! reloaded at `:783`; its `onWorldLoad` is called at `:1054` **inside**
//! `if (eventHandler == null)`, so a colony whose handler already exists never
//! tells its quests the world loaded. `DatagenLootTableManager` lives only in
//! `GatherDataHandler.java:24` — a build-time object no player ever runs.
//! `DebugPlayerManager` is a static set called from a command
//! (`CommandToggleDebug.java:38`) and from two server messages that gate
//! themselves on it (`QueryCitizenAIHistoryMessage.java:54`,
//! `DebugEnablePathfindingMessage.java:59`).
//!
//! **A second premise of the brief is false, and the port says where.** It asks
//! for "the order of the writes so that a test proves none is made before a
//! refusal". For `QuestManager` that ordering exists and is kept (availability
//! before validity, `:72-73`). For `DatagenLootTableManager` **there is no
//! refusal at all**: it catches `Throwable`, prints it and returns
//! `LootTable.EMPTY` (`:64-69`), which is why the port refuses with a name and
//! keeps `or_empty` as Java's own answer. For `DebugPlayerManager` there is
//! neither a refusal nor an ordering — a toggle always writes — so that file
//! ports the two asymmetric mutators and the `!production` flag instead of a
//! write order it does not have.
//!
//! **Pure data, pure rules**: no host call, no clock, no thread, no file. Every
//! map is a `BTreeMap`, so "which one" is decided and not left to a hash, and
//! every table that Java keeps `static` is passed in — [`quests::Registry`],
//! [`loot::Source`], the tracking map of [`debug::Debug::toggle`] — which is what
//! lets all of it be tested with no game, no wasm and no prepared resources.
//!
//! The three gaps that remain are named in [`MANQUANT.md`](MANQUANT): the quest
//! buffer, the `QuestInstance` fields the manager passes through, and the debug
//! sync packet.

mod nbt;
pub mod debug;
pub mod loot;
pub mod quests;

pub use debug::{Debug, Mode};
pub use loot::{Refusal as LootRefusal, Source, Tables, Unreadable};
pub use quests::{
    Actor, Completion, Finished, Instance, QuestBook, ReadError, Refusal as QuestRefusal, Registry,
    Template, KEY_MANAGER,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager_09::nbt;
    use mod_api::{Compound, Identifier, Tag};

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("a literal is a valid id")
    }

    /// The colony, reduced to the two answers a refused accept needs.
    struct Nobody;

    impl quests::Actor for Nobody {
        fn attempt_start(&mut self, _: &Identifier) -> Option<Compound> {
            None
        }

        fn valid(&mut self, _: &quests::Instance) -> bool {
            false
        }

        fn deleted(&mut self, _: &quests::Instance) {}

        fn reloaded(&mut self, _: &quests::Instance) {}
    }

    /// A pack with nothing in it.
    struct NoFiles;

    impl loot::Source for NoFiles {
        fn read(&mut self, _: &str) -> Result<mod_api::serde_json::Value, loot::Unreadable> {
            Err(loot::Unreadable::new("no datagen pack in a wasm mod"))
        }
    }

    /// `Colony.deserializeNBT` digs the book out of `quest_manager`
    /// (`Colony.java:783`) and `writeToNBT` puts it back there (`:955`). The
    /// round trip through that key is the only thing tying this module to a
    /// colony compound.
    #[test]
    fn the_colony_files_the_book_under_its_own_key() {
        let registry = quests::Registry::new();
        let mut book = QuestBook::new();
        book.unlock(id("q:welcome"));
        let mut colony = Compound::new();
        colony.insert(KEY_MANAGER.into(), Tag::Compound(book.serialize()));

        let nested = nbt::compound(&colony, KEY_MANAGER).expect("the key holds a compound");
        let read = QuestBook::read(nested, &registry).expect("the colony's own compound reads");
        assert!(read.is_unlocked(&id("q:welcome")));
        assert!(nbt::compound(&colony, "no_such_manager").is_none());
    }

    /// Three stores, and a refusal in each of the two that can refuse: the quest
    /// book keeps its one instance where it was and stays clean, the table cache
    /// stays empty and remembers nothing.
    #[test]
    fn a_refusal_leaves_every_store_where_it_was() {
        let mut book = QuestBook::new();
        book.inject(quests::Instance::new(Compound::from([(
            quests::KEY_ID.into(),
            Tag::String("q:a".into()),
        )])))
        .expect("the compound names an id");
        assert!(book.take_frame(false).is_some(), "clear the initial flag");
        assert!(!book.is_dirty());

        let refused = book.accept(&id("q:a"), &mut Nobody);
        assert!(refused.is_err(), "invalid is refused");
        assert_eq!(book.available().count(), 1, "still available");
        assert_eq!(book.in_progress().count(), 0);
        assert!(!book.is_dirty(), "not one write");

        let mut tables = Tables::new(NoFiles);
        assert!(tables.get(&id("minecraft:blocks/oak_log")).is_err());
        assert_eq!(tables.len(), 0);
        assert!(!tables.cached(&id("minecraft:blocks/oak_log")));
    }

    /// The tick `QuestManager` rides on is the colony's own slow transition, and
    /// the loot directory is the one the helper is asked for. Two strings a
    /// reader can check without opening Java.
    #[test]
    fn the_managers_carry_their_javas_own_values() {
        assert_eq!(KEY_MANAGER, "quest_manager", "NbtTagConstants.java:59");
        assert_eq!(crate::manager_06::COLONY_TICK, 500, "TickRateConstants.java:11");
        assert_eq!(loot::DIRECTORY, "loot_tables", "DatagenLootTableManager.java:50");
        assert_eq!(loot::SUFFIX, ".json", "DatagenLootTableManager.java:50");
        assert_eq!(
            quests::KEY_AVAILABLE,
            "availquests",
            "NbtTagConstants.java:739"
        );
        assert_eq!(
            quests::KEY_IN_PROGRESS,
            "inprogquests",
            "NbtTagConstants.java:738"
        );
        assert_eq!(
            quests::KEY_FINISHED,
            "finishedquests",
            "NbtTagConstants.java:740"
        );
        assert_eq!(
            quests::KEY_UNLOCKED,
            "unlockedquests",
            "NbtTagConstants.java:748"
        );
        assert_eq!(
            quests::KEY_REPUTATION,
            "questreputation",
            "NbtTagConstants.java:749"
        );
    }
}
