//! The boundary: which half carries a change out, and what the engine cannot carry.
//!
//! **The bridge has two halves.** This module decides *whether* and *in what order*; the
//! engine writes. [`Handled`] is the whole split, one value per operation, and [`Sink`] is
//! what a caller implements to hand an accepted batch over.
//!
//! **`place_template` is not reachable from a mod, and this port does not pretend it is.**
//! The SDK's `world_edit` area declares six methods
//! (`crates/horizon-mod-vocab/src/host/world_edit.rs`): `edit_blocks`,
//! `upgrade_block_entity_nbt`, `upgrade_entity_nbt`, `surface_heights`, `read_columns` and
//! `structure_template`. The last one **reads** a template — blocks, states and saved NBT,
//! in vanilla's placement order, "without placing anything" (`:70-79`) — and `edit_blocks`
//! **writes** complete states (`:10-20`). `place_template` exists in the engine
//! (`crates/horizon-world/src/level/bonemeal.rs:162`,
//! `horizon-client/src/game/administration/structure_blocks.rs:384`) and is **not part of
//! the mod API**: no host method and no hook reaches it. Writing `place_template(...)`
//! here would be writing a function that does not exist.
//!
//! **What a paste through the mod API therefore loses**, and what this module must hand
//! back to the caller (`MANQUANT.md` §1, with what was wanted for each):
//!
//! * **the block entities' data** — `structure_template` carries it
//!   (`world/structure_template.rs:47-55`), `edit_blocks` takes none ("NBT is not
//!   supported", `game.rs:28`). Java's handlers write the state **then** re-apply the saved
//!   data (`RackPlacementHandler.java:46-52`), a two-call sequence the port can describe
//!   but not perform;
//! * **the template's entities** — the snapshot "leaves out" the template's entities
//!   (`world_edit.rs:74-79`), and no SDK call spawns them;
//! * **the neighbour updates** — `edit_blocks` says "no neighbour is told" (`:12`); a
//!   `Level.setBlock` with `UPDATE_FLAG` (`Constants.java:32`) tells them;
//! * **the chunk tickets** — Java keeps a colony's chunks loaded (`Colony.java:543`,
//!   `:596`); the mod API has no ticket call, and an edit in an unloaded chunk is silently
//!   left out (`world_edit.rs:10-20`), which is why
//!   [`super::refusal::Refusal::Unloaded`] exists.

use super::intent::{Change, Intent};

/// The block a break writes: `Blocks.AIR.defaultBlockState()` through
/// `Level.setBlockAndUpdate` (`PathingStuckHandler.java:506`), and the state `/setblock ~ ~
/// ~ air` names (`crates/horizon-mod-vocab/src/host/block_states.rs:16-25`).
///
/// **A break and a placement of air are the same write**, which is the engine's truth
/// rather than a shortcut: what differs — whether the colony was allowed to clear the cell,
/// and whether the block dropped anything — is a decision, made in [`super::intent`] before
/// this name exists.
pub const AIR: &str = "minecraft:air";

/// Java's `UPDATE_FLAG` (`api/util/constant/Constants.java:32`, `0x03`): the two low bits of
/// `Level.setBlock`'s flags, `UPDATE_NEIGHBORS | UPDATE_CLIENTS`.
///
/// **Carried as a documented value, not as a flag anyone passes.** `edit_blocks` takes no
/// flags (`host/world_edit.rs:19-20`), so a write through this bridge always behaves as
/// `fill … strict` does; a caller that needs the neighbours told must ask outside the
/// bridge.
pub const UPDATE_FLAG: i32 = 0x03;

/// Which half of the bridge carries one change out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two halves of one bridge
pub enum Handled {
    /// The engine writes it: `edit_blocks`, for a placement, a replacement or a break
    /// (`host/world_edit.rs:10-20`).
    Engine,
    /// Nothing is written: the cell keeps exactly what the world holds. This is
    /// [`Change::Keep`], the port's answer where Java rewrites a block only to restore its
    /// data afterwards (`RackPlacementHandler.java:46-52`).
    Decided,
}

/// Which half carries `change` out.
pub fn handled(change: &Change) -> Handled {
    match change {
        Change::Place { .. } | Change::Replace { .. } | Change::Break { .. } => Handled::Engine,
        Change::Keep { .. } => Handled::Decided,
    }
}

/// What a caller implements to carry an accepted batch into the world.
///
/// **One call per write, in the order [`Intent`] holds them**, which is vanilla's placement
/// order (`host/world_edit.rs:74-79`: "each by y, x, z"). A `Keep` never arrives: there is
/// nothing to write, and calling a sink for it would make a decision indistinguishable from
/// a write in every log.
///
/// The real implementation resolves `block` with `block_state_id` — a complete typed state,
/// never a command string (`host/block_states.rs:16-25`) — collects the runs and calls
/// `edit_blocks` once per batch (`:10-20`). It cannot call it *from* here: that is a host
/// call, and this module is pure.
pub trait Sink {
    /// Write `block` at `at`. `block` is a registry name `/setblock` reads,
    /// `minecraft:air` for a break.
    fn write(&mut self, at: [i32; 3], block: &str);
}

impl Intent {
    /// Hands every write of an accepted batch to `sink`, in order.
    ///
    /// **Only an `Intent` can be emitted**, and one only exists when [`Intent::plan`]
    /// accepted every change, so no write can reach a sink before a refusal. That is the
    /// guarantee, and it is a property of the constructors rather than a promise: there is
    /// no way to build a partial `Intent` and no way to emit one.
    pub fn emit(&self, sink: &mut dyn Sink) {
        for change in self.writes() {
            match change {
                Change::Place { at, block } | Change::Replace { at, block } => {
                    sink.write(*at, block)
                }
                Change::Break { at } => sink.write(*at, AIR),
                Change::Keep { .. } => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sink that remembers everything it was handed.
    #[derive(Default)]
    struct Recorder(Vec<([i32; 3], String)>);

    impl Sink for Recorder {
        fn write(&mut self, at: [i32; 3], block: &str) {
            self.0.push((at, block.to_owned()));
        }
    }

    #[test]
    fn a_placement_a_replacement_and_a_break_are_the_engines_a_keep_is_the_ports() {
        let stone = Change::Place {
            at: [0, 0, 0],
            block: "minecraft:stone".to_owned(),
        };
        assert_eq!(handled(&stone), Handled::Engine);
        assert_eq!(
            handled(&Change::Replace {
                at: [0, 0, 0],
                block: "minecraft:stone".to_owned()
            }),
            Handled::Engine
        );
        assert_eq!(handled(&Change::Break { at: [0, 0, 0] }), Handled::Engine);
        assert_eq!(
            handled(&Change::Keep { at: [0, 0, 0] }),
            Handled::Decided,
            "a keep is a decision: the cell keeps what it already holds"
        );
    }

    #[test]
    fn a_break_is_handed_to_the_sink_as_air() {
        let mut sink = Recorder::default();
        sink.write([1, 2, 3], AIR);
        assert_eq!(sink.0, vec![([1, 2, 3], "minecraft:air".to_owned())]);
        assert_eq!(AIR, "minecraft:air", "the name /setblock reads");
    }

    #[test]
    fn java_update_flag_is_the_two_low_bits_and_is_not_passed_to_the_engine() {
        assert_eq!(UPDATE_FLAG, 0x03);
        assert_eq!(UPDATE_FLAG & 1, 1, "UPDATE_NEIGHBORS");
        assert_eq!(UPDATE_FLAG & 2, 2, "UPDATE_CLIENTS");
    }
}
