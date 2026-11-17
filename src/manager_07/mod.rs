//! Four managers of Java's request system and the visitor list, ported as what
//! they decide rather than what they call.
//!
//! Java reference: `core/colony/managers/VisitorManager.java` (337 lines),
//! `core/colony/requestsystem/data/StandardDataStoreManager.java` (173),
//! `core/colony/requestsystem/management/manager/StandardRecipeManager.java`
//! (148) and `.../StandardRequestManager.java` (682).
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `visitors` | who is in the colony without living there, and what is sent to whom | `VisitorManager.java` |
//! | `datastores` | what a token keys, what a read tolerates, where the id is written twice | `StandardDataStoreManager.java` |
//! | `recipes` | which recipes are saved, which are dropped, and what "the same recipe" means | `StandardRecipeManager.java` |
//! | `requestmanager` | the nine saved keys, the version that wipes them, the order of the writes | `StandardRequestManager.java` |
//! | `nbt` | the five tag reads all four share | `CompoundTag.getInt`/`getLong`/… |
//!
//! **Three of the four take their data from `crate::requests`, and none of them
//! duplicates it.** A request is `crate::requests::Request`, filed in
//! `crate::requests::Queue` by the id its colony handed out; this module changes
//! one only through the four moves `Request` already defines (`assign`, `begin`,
//! `deliver`, `cancel`) and answers Java's follow-up table beside them. The one
//! question `crate::requests` already answered — who supplies a request — stays
//! where it is (`src/requests/resolver.rs`); `requestmanager` does not re-ask it.
//! And `src/requests/mod.rs` says, in its own note, that the manager was
//! deliberately left out: this is that manager, arriving in its own folder.
//!
//! **What was already covered, and is therefore not written twice.**
//!
//! - `ColonyManager` — not one of the four briefed files, and already ported file
//!   by file in `crate::manager` (`index`, `claims`, `proximity`, `lifecycle`,
//!   `persist`). Nothing of it is redone here.
//! - The citizen roster (`CitizenDataManager.createFromNBT`) is
//!   `crate::persistence::roster`. A visitor's record is **not** a citizen record:
//!   `VisitorData` adds a recruit cost and a sitting position to a citizen's
//!   thirty keys (`VisitorData.java:53-70`), and this port saves the one field
//!   every rule below reads — its id (`visitors`).
//! - A request's own compound is `crate::requests::request`, and its save order is
//!   `crate::requests::queue`. The manager saves the *system* around it, not the
//!   request.
//!
//! **Pure data, pure rules.** No host call, no clock, no thread, no file — the
//! same bargain `crate::manager`, `crate::colony` and `crate::requests` made, and
//! the only one a wasm mod can keep. Every value a rule needs that Java would
//! fetch from the game arrives as a parameter: the world's answer to "is this
//! chunk loaded" is a closure (`visitors::spawn`), the assignment store's answer
//! to "does this request have a resolver" is a closure
//! (`requestmanager::Manager::load`), and the seven tokens a fresh request system
//! needs are [`Tokens`], because a wasm mod has no random source for them.
//!
//! **The four divergences, each with its cost written where it bites.**
//!
//! 1. **Tokens are `i32`, Java's are `UUID`.** The port keys everything by the
//!    same `i32` as `crate::requests`, and writes the two halves as
//!    `{Id_MSB: 0, Id_LSB: n}`. A token a real Java server wrote is **not
//!    expressible** and is skipped by name rather than truncated
//!    (`datastores::SkipReason::NotExpressible`).
//! 2. **Every map is a `BTreeMap`.** Java walks `HashMap`s and `HashBiMap`s, so
//!    its order is unspecified; a port that kept that could not be tested. This is
//!    the same divergence `crate::manager` states for its colonies.
//! 3. **A visitor's identity is text, not a hashed UUID.** Java derives it with
//!    `UUID.nameUUIDFromBytes` (`CitizenData.java:2100-2104`) and the SDK has no
//!    MD5. The *comparison* is the same one; the value written is the pre-hash
//!    triple, so it is not comparable with a UUID printed by a Java server.
//! 4. **A reset cannot mint ids.** Java's `reset` builds five stores and two
//!    resolvers from a factory; here the caller supplies them and
//!    `requestmanager::Loaded::needs_reset` says when.
//!
//! **What is left for the caller, and why that is the honest split.** Removing an
//! entity (`VisitorManager.java:81`, `:113`), sending a packet (a `Plan`), posing
//! a block-entity ticket, and every handler Java's manager calls are all outside
//! this folder: `src/manager_07/MANQUANT.md` lists each one with what it would
//! need.

mod nbt;
pub mod datastores;
pub mod recipes;
pub mod requestmanager;
pub mod visitors;

pub use datastores::{Book, Mend as DataStoreMend, Skip as DataStoreSkip, Store, Token};
pub use recipes::{RecipeBook, Skip as RecipeSkip, SkipReason as RecipeSkipReason, Storage};
pub use requestmanager::{
    follow_up, FollowUp, Kind, Manager, Tokens, TICK_ORDER, CURRENT_VERSION,
};
pub use visitors::{Outcome, Plan, TickPlan, Visitor, Visitors};

#[cfg(test)]
mod tests {
    use super::*;

    /// The four files answer four different questions; these are the four types
    /// a caller needs first, and none of them shadows another.
    #[test]
    fn the_module_exposes_one_name_per_question() {
        let visitors = Visitors::new();
        let book = Book::new();
        let mut recipes = RecipeBook::new();
        let manager = Manager::new();

        assert!(visitors.is_empty() && book.is_empty() && recipes.snapshot().is_empty());
        assert!(manager.player().is_none());
        assert_eq!(manager.version(), -1);
        assert_eq!(CURRENT_VERSION, 15);
        assert!(recipes.write().get(&"recipes".into()).is_some());
    }

    /// A fresh request system files its five stores, and the visitors manager
    /// keeps its own counter: the two are independent, which is the whole point of
    /// four managers rather than one.
    #[test]
    fn the_four_managers_do_not_share_state() {
        let tokens = Tokens { stores: [1, 2, 3, 4, 5], player: 6, retrying: 7 };
        let mut manager = Manager::new();
        manager.reset(&tokens);
        let mut visitors = Visitors::new();
        assert_eq!(visitors.create(1, "minecraft:overworld"), Ok(-1));
        assert_eq!(manager.book().len(), 5);
        assert_eq!(visitors.len(), 1);
        assert_eq!(manager.store_id(Kind::RequestIdentities), Some(1));
        assert_eq!(follow_up(crate::requests::State::Delivered), FollowUp::Completed);
    }
}
