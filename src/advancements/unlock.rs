//! What is unlocked, what unlocks what, and the named refusal.
//!
//! Java reference: no counterpart. Java never writes a mod advancement down —
//! `PlayerAdvancements` is the *game's* book, in the player's own file, and a
//! wasm mod can neither read nor write it. So the port keeps the colony side of
//! the tree itself, under one key, and this file is the only place that decides
//! what "unlocked" means here.
//!
//! **The rule, and the one interpretation in it.** An advancement is granted
//! when its parent is already granted **and** its own condition holds
//! ([`super::condition::Condition::met`]). The parent gate is the port's reading of the brief's
//! "quel advancement en débloque un autre": in vanilla a parent only decides
//! *where* an advancement is drawn, never whether it can be granted, so a game
//! would give `build_town_hall_5` to a colony that never saw `build_town_hall`.
//! The port refuses that, and says so by name ([`Refusal::Parent`]). It is the
//! one place where this module is stricter than the game.
//!
//! **A foreign parent is not a gate.** `minecraft/craft_supply` hangs under
//! `minecraft:story/root`, which the port cannot see; it is unlocked on its own
//! condition alone.
//!
//! **`unlock` names, `sweep` is silent.** [`Progress::unlock`] is the question
//! "may this one be granted, and if not why"; [`Progress::sweep`] walks the whole
//! table until nothing new opens and keeps what passed, which is what a hook
//! would call after recording an event. Two different callers, two different
//! answers to a refusal.

use std::collections::BTreeSet;

use mod_api::{Compound, Tag};

use super::condition;
use super::state::ColonyState;
use super::tree::{self, Advancement, Parent};

/// The port's one key for the granted set. Java would use the player's
/// `advancements` compound (`PlayerAdvancements`); the name is borrowed from
/// there so a reader looking for the game's key finds this one instead.
pub const KEY_UNLOCKED: &str = "advancements";

/// The advancements a colony has been granted.
///
/// Ids are the tree's own `&'static str`, so a set of 62 costs no allocation and
/// a row that is renamed breaks at compile time rather than in a save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Progress {
    unlocked: BTreeSet<&'static str>,
}

impl Progress {
    pub fn new() -> Progress {
        Progress::default()
    }

    /// Whether this advancement was granted. An id no row declares is `false`,
    /// never an error: asking is not a claim.
    pub fn is_unlocked(&self, id: &str) -> bool {
        tree::find(id).is_some_and(|advancement| self.unlocked.contains(advancement.id))
    }

    pub fn count(&self) -> usize {
        self.unlocked.len()
    }

    /// The granted ids, in id order.
    pub fn unlocked(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.unlocked.iter().copied()
    }

    /// **The resolution.** Grants `id` and returns the row, or names the refusal.
    ///
    /// Granting twice is not an error: an advancement already granted stays
    /// granted (vanilla's `award` is silent the second time), so an idempotent
    /// caller does not have to ask first.
    pub fn unlock(
        &mut self,
        id: &str,
        state: &ColonyState,
    ) -> Result<&'static Advancement, Refusal> {
        let advancement = tree::find(id).ok_or_else(|| Refusal::Unknown { id: id.to_owned() })?;
        if self.unlocked.contains(advancement.id) {
            return Ok(advancement);
        }
        if let Parent::Inside(parent) = advancement.parent {
            if !self.unlocked.contains(parent) {
                return Err(Refusal::Parent {
                    id: advancement.id,
                    parent,
                });
            }
        }
        advancement.condition.met(state)?;
        self.unlocked.insert(advancement.id);
        Ok(advancement)
    }

    /// Grants everything the tree and the state allow, repeating until nothing
    /// new opens (a child may sit above its parent in the table), and returns
    /// what was granted, in table order.
    ///
    /// Bounded: each pass grants at least one advancement or ends, and the tree
    /// has [`super::ADVANCEMENT_COUNT`] rows.
    pub fn sweep(&mut self, state: &ColonyState) -> Vec<&'static Advancement> {
        let mut opened = Vec::new();
        loop {
            let before = opened.len();
            for advancement in tree::TREE {
                if self.unlocked.contains(advancement.id) {
                    continue;
                }
                if let Ok(granted) = self.unlock(advancement.id, state) {
                    opened.push(granted);
                }
            }
            if opened.len() == before {
                return opened;
            }
        }
    }

    /// The compound: `advancements` is a list of ids, in id order.
    pub fn to_compound(&self) -> Compound {
        let ids = self
            .unlocked
            .iter()
            .map(|&id| Tag::String(id.into()))
            .collect();
        Compound::from([(KEY_UNLOCKED.into(), Tag::List(ids))])
    }

    /// The inverse. An id no row declares is dropped: a save from a tree that
    /// has since renamed a row must not resurrect the old name.
    pub fn from_compound(data: &Compound) -> Progress {
        let mut progress = Progress::new();
        let Some(Tag::List(ids)) = data.get(&KEY_UNLOCKED.into()) else {
            return progress;
        };
        for id in ids {
            let Tag::String(id) = id else {
                continue;
            };
            if let Some(advancement) = tree::find(&id.to_string_lossy()) {
                progress.unlocked.insert(advancement.id);
            }
        }
        progress
    }
}

/// Why an advancement was not granted, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// No row has that id: a typo, or a row that was renamed.
    Unknown { id: String },
    /// The parent is not granted yet, so the tree has not been walked that far.
    Parent {
        id: &'static str,
        parent: &'static str,
    },
    /// The condition itself does not hold; the reason is the condition's.
    Condition(condition::Refusal),
}

impl Refusal {
    /// A stable name for logs and tests.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unknown { .. } => "unlock.refusal.unknown",
            Self::Parent { .. } => "unlock.refusal.parent",
            Self::Condition(refusal) => refusal.name(),
        }
    }
}

impl From<condition::Refusal> for Refusal {
    fn from(refusal: condition::Refusal) -> Refusal {
        Refusal::Condition(refusal)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::super::state::Event;
    use super::*;

    /// The state the short chain below needs: a supply ship, a town hall
    /// placed and built, a builder requested and standing, one recipe.
    fn chain() -> ColonyState {
        let mut state = ColonyState::new();
        state.record(Event::SupplyPlaced);
        state.record(Event::StructurePlaced { hut: "townhall" });
        state.record(Event::BuildRequested {
            hut: "builder",
            level: 1,
        });
        state.record(Event::RecipeAdded {
            output: "minecraft:torch",
            grid: 2,
        });
        state.record(Event::GuiClicked {
            button: "fulfill",
            window: "minecolonies:gui/citizen/requests.xml",
        });
        state.build("builder", 1);
        state.build("townhall", 1);
        state
    }

    #[test]
    fn a_child_is_refused_while_its_parent_is_not_granted() {
        let mut progress = Progress::new();
        let state = chain();
        assert_eq!(
            progress.unlock("minecolonies/colony_population_5", &state),
            Err(Refusal::Parent {
                id: "minecolonies/colony_population_5",
                parent: "minecolonies/build_citizen"
            })
        );
        assert_eq!(progress.count(), 0);
    }

    #[test]
    fn a_root_without_its_condition_refuses_with_the_condition_reason() {
        let mut progress = Progress::new();
        let empty = ColonyState::new();
        assert_eq!(
            progress.unlock("minecolonies/root", &empty),
            Err(Refusal::Condition(condition::Refusal::NeverHappened {
                event: Event::SupplyPlaced
            }))
        );
        assert_eq!(
            progress.unlock("no/such/row", &empty),
            Err(Refusal::Unknown {
                id: "no/such/row".to_owned()
            })
        );
        assert_eq!(
            progress.unlock("minecolonies/root", &chain()).map(|a| a.id),
            Ok("minecolonies/root")
        );
        // Granting twice is not an error.
        assert_eq!(
            progress.unlock("minecolonies/root", &chain()).map(|a| a.id),
            Ok("minecolonies/root")
        );
        assert_eq!(progress.count(), 1);
    }

    #[test]
    fn a_sweep_opens_exactly_the_chain_the_state_allows() {
        let mut progress = Progress::new();
        let state = chain();
        let opened: Vec<&str> = progress
            .sweep(&state)
            .iter()
            .map(|advancement| advancement.id)
            .collect();
        assert_eq!(
            opened,
            [
                "minecolonies/root",
                "production/root",
                "minecolonies/place_townhall",
                "minecolonies/start_builder",
                "minecolonies/fulfill_request",
                "minecolonies/build_builder",
                "minecolonies/build_town_hall",
                "minecolonies/building_add_recipe_torch",
                "minecraft/craft_supply",
            ]
        );
        assert_eq!(progress.count(), 9);
        // A second sweep has nothing left to give.
        assert!(progress.sweep(&state).is_empty());
        assert_eq!(progress.count(), 9);
    }

    #[test]
    fn a_foreign_parent_does_not_gate_its_child() {
        let mut progress = Progress::new();
        let mut state = ColonyState::new();
        state.record(Event::SupplyPlaced);
        let granted = progress
            .unlock("minecraft/craft_supply", &state)
            .map(|advancement| advancement.id);
        assert_eq!(granted, Ok("minecraft/craft_supply"));
    }

    #[test]
    fn the_progress_survives_a_compound_and_drops_a_name_no_row_has() {
        let mut progress = Progress::new();
        progress.sweep(&chain());
        let data = progress.to_compound();
        assert_eq!(Progress::from_compound(&data), progress);
        let damaged = Compound::from([(
            KEY_UNLOCKED.into(),
            Tag::List(vec![
                Tag::String("minecolonies/root".into()),
                Tag::String("minecolonies/a_row_that_was_renamed".into()),
            ]),
        )]);
        let read = Progress::from_compound(&damaged);
        assert_eq!(read.count(), 1);
        assert!(read.is_unlocked("minecolonies/root"));
        assert_eq!(read.unlocked().collect::<Vec<_>>(), ["minecolonies/root"]);
    }
}
