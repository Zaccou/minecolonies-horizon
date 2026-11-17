//! The four operations, the batch, and the order it is handed over in.
//!
//! **Four operations, and Java has three.** A placement in Java is one call,
//! `BlockPosUtil.setBlock` (`api/util/BlockPosUtil.java:664-667`, straight to
//! `Level.setBlock`), and whether it *overwrites* something is not the call's business:
//! `BlockState.canBeReplaced()` decides, and every handler that cares asks it first
//! (`DirectPlaceMessage.java:117` asks the narrow air form). The port names the
//! distinction, because the two cases have different refusals and different costs.
//!
//! | Operation | Java | What it costs |
//! | --- | --- | --- |
//! | [`Change::Place`] | `setBlock` into a replaceable cell | the block named is written |
//! | [`Change::Replace`] | `setBlock` over a solid cell | the block that stood there is gone |
//! | [`Change::Keep`] | nothing — the handler returns without writing | nothing, and the data survives |
//! | [`Change::Break`] | `setBlock(pos, air, UPDATE_FLAG)` (`PathingStuckHandler.java:506`) | the block is gone, and **its drops are not produced** |
//!
//! **`Keep` is the one Java spells as an absence**: a handler that decides a cell is
//! already right simply returns, and one that must keep a container's data rewrites the
//! block and restores it (`RackPlacementHandler.java:46-52`). The port asks for the
//! decision explicitly, so a batch can say "leave this cell alone" and a test can prove
//! nothing was written.
//!
//! **A drop is not a write.** Java's break path is `Level.destroyBlock`, which spawns the
//! drops; `setBlock(pos, air)` does not, and neither does `edit_blocks`. A colony that
//! "breaks" a block in this bridge destroys it, which is why [`Change::Break`] refuses a
//! container.
//!
//! **The batch, and its two guarantees.** [`Intent::plan`] answers every change before
//! any is emitted, and [`Intent`] has no other constructor: a refusal means no write,
//! proven by construction. And the accepted batch is put in a **canonical order**, `y`
//! then `x` then `z` — the order `structure_template` reports a template in
//! (`host/world_edit.rs:74-79`) — so two calls naming the same cells hand the engine the
//! same sequence. Java has no such rule; a paste there follows the blueprint's own
//! iteration.

use crate::colony::Colony;

use super::guard::{Guard, Occupant, State};
use super::refusal::Refusal;

/// One change a colony wants the world to take.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the four operations this bridge knows, closed on purpose
pub enum Change {
    /// Write `block` where a placement replaces what is there
    /// (`BlockState.canBeReplaced`, `host/block_states.rs:120-123`).
    ///
    /// `block` is a registry name `/setblock` reads, properties included —
    /// `minecraft:oak_stairs[facing=east]` — because the caller resolves it with
    /// `block_state_id` (`host/block_states.rs:16-25`). A `String`, **not** a
    /// `BlockState`: the ordinal of one is the host's and a test cannot fabricate it
    /// (`block/block_state.rs:5-9`).
    Place { at: [i32; 3], block: String },
    /// Write `block` over a cell that already holds something a placement would not
    /// replace. The same Java call; a different decision here, because the block that
    /// stood there is lost.
    Replace { at: [i32; 3], block: String },
    /// Leave the cell exactly as the world holds it. No engine call, whatever the cell
    /// holds — this is the operation a container needs.
    Keep { at: [i32; 3] },
    /// Clear the cell: `minecraft:air`, and no drops
    /// (`PathingStuckHandler.java:506`).
    Break { at: [i32; 3] },
}

impl Change {
    /// The cell this change is about.
    pub fn at(&self) -> [i32; 3] {
        match self {
            Self::Place { at, .. }
            | Self::Replace { at, .. }
            | Self::Keep { at }
            | Self::Break { at } => *at,
        }
    }
}

/// A refusal, and the place in the batch that produced it.
///
/// **`index` counts the caller's order**, the one `plan` was given, because that is the
/// only number the caller can act on. The accepted batch is re-ordered (see [`order`]),
/// so `index` does not index [`Intent::writes`].
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct RefusalAt {
    pub index: usize,
    pub at: [i32; 3],
    pub refusal: Refusal,
}

impl RefusalAt {
    /// The name of the refusal underneath, for a log line.
    pub fn name(&self) -> &'static str {
        self.refusal.name()
    }
}

impl std::fmt::Display for RefusalAt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "change {} at {:?}: {}", self.index, self.at, self.refusal)
    }
}

impl std::error::Error for RefusalAt {}

/// A batch every question accepted, in canonical order. **The only value
/// [`super::engine::Sink`] will take**, and only [`Intent::plan`] builds one.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Intent {
    writes: Vec<Change>,
}

/// The three numbers of the canonical order, in the order they count: `y`, then `x`,
/// then `z` (`host/world_edit.rs:74-79`, "each by y, x, z").
pub fn key(at: [i32; 3]) -> (i32, i32, i32) {
    (at[1], at[0], at[2])
}

/// The canonical comparison of two changes, for `sort_by`. Equal keys keep the caller's
/// order, which [`Intent::plan`] has already refused: one cell carries one change.
pub fn order(one: &Change, other: &Change) -> std::cmp::Ordering {
    key(one.at()).cmp(&key(other.at()))
}

impl Intent {
    /// Decides the whole batch, and builds the [`Intent`] only if every change was
    /// accepted. Per change, in the order given:
    ///
    /// 1. the cell is not already named by an earlier change — one cell, one change
    ///    ([`Refusal::ConflictingWrite`]), because `edit_blocks` runs a batch in one go
    ///    (`host/world_edit.rs:10-16`) and the second write would describe a world the
    ///    first one made;
    /// 2. the five questions of [`Guard::may_write`], in their order;
    /// 3. the operation's own precondition, against the [`Occupant`] the guard already
    ///    read, so no cell is read twice.
    ///
    /// The first change that fails stops the batch: nothing is emitted, and the refusal
    /// names which change it was.
    pub fn plan(
        colony: &Colony,
        guard: &Guard<'_>,
        state: State,
        changes: Vec<Change>,
    ) -> Result<Intent, RefusalAt> {
        let mut seen: Vec<[i32; 3]> = Vec::with_capacity(changes.len());
        let mut accepted = Vec::with_capacity(changes.len());

        for (index, change) in changes.into_iter().enumerate() {
            let at = change.at();
            if let Some(first) = seen.iter().position(|cell| *cell == at) {
                return Err(RefusalAt {
                    index,
                    at,
                    refusal: Refusal::ConflictingWrite { at, first, second: index },
                });
            }
            seen.push(at);

            let occupant = guard
                .may_write(colony, state, at)
                .map_err(|refusal| RefusalAt { index, at, refusal })?;

            preconditions(&change, occupant, guard.dimension)
                .map_err(|refusal| RefusalAt { index, at, refusal })?;

            accepted.push(change);
        }

        accepted.sort_by(order);
        Ok(Intent { writes: accepted })
    }

    /// The accepted changes, in canonical order.
    pub fn writes(&self) -> &[Change] {
        &self.writes
    }

    pub fn len(&self) -> usize {
        self.writes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    /// How many of them the engine actually writes, [`Change::Keep`] excluded.
    pub fn emitted(&self) -> usize {
        self.writes
            .iter()
            .filter(|change| super::engine::handled(change) == super::engine::Handled::Engine)
            .count()
    }
}

/// **The sixth question**, and the only one that needs to know which operation was asked
/// for.
///
/// A placement writes only where a placement replaces; a replacement and a break need
/// something standing there; a keep needs nothing but a loaded cell, which the guard has
/// already checked. **A container is refused by the three writes** — `edit_blocks` loses
/// its data and a break drops nothing — and kept by none, which is why
/// [`Change::Keep`] exists.
///
/// [`Occupant::NotLoaded`] cannot arrive, the guard refusing it first; it is answered
/// rather than `panic!`ed on, because a crash in a wasm mod is not cheap.
fn preconditions(change: &Change, occupant: Occupant, dimension: &str) -> Result<(), Refusal> {
    let at = change.at();
    let unloaded = || Refusal::Unloaded {
        dimension: dimension.to_owned(),
        at,
    };
    match (change, occupant) {
        (Change::Place { .. }, Occupant::Replaceable) => Ok(()),
        (Change::Place { .. }, Occupant::Solid) => Err(Refusal::Occupied { at }),
        (Change::Replace { .. } | Change::Break { .. }, Occupant::Solid) => Ok(()),
        (Change::Replace { .. } | Change::Break { .. }, Occupant::Replaceable) => {
            Err(Refusal::Empty { at })
        }
        (Change::Keep { .. }, Occupant::NotLoaded) => Err(unloaded()),
        (Change::Keep { .. }, _) => Ok(()),
        (_, Occupant::Container) => Err(Refusal::WouldDropContents { at }),
        (_, Occupant::NotLoaded) => Err(unloaded()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony_world::engine::Sink;
    use crate::colony_world::fakes::{Cells, Mixed, OWNED, OVERWORLD, Unclaimed, colony};
    use crate::colony_world::guard::Open;

    /// A sink that remembers everything it was handed.
    #[derive(Default)]
    struct Recorder {
        writes: Vec<([i32; 3], String)>,
    }

    impl Sink for Recorder {
        fn write(&mut self, at: [i32; 3], block: &str) {
            self.writes.push((at, block.to_owned()));
        }
    }

    fn place(at: [i32; 3], block: &str) -> Change {
        Change::Place { at, block: block.to_owned() }
    }

    #[test]
    fn the_four_operations_are_accepted_where_the_rules_allow_them() {
        // Air where the placement goes, blocks where the replacement and the break go.
        let world = Mixed::new(&[([9, 70, 8], Occupant::Solid), ([10, 70, 8], Occupant::Solid)]);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let intent = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 70, 8], "minecraft:stone"),
                Change::Replace { at: [9, 70, 8], block: "minecraft:oak_planks".to_owned() },
                Change::Break { at: [10, 70, 8] },
            ],
        )
        .expect("a placement on air, a replacement and a break on blocks");
        assert_eq!(intent.len(), 3);
        assert_eq!(intent.emitted(), 3, "all three are the engine's");
    }

    #[test]
    fn a_placement_needs_a_cell_a_placement_replaces() {
        let world = Cells::of(Occupant::Solid);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let refusal = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![place([8, 70, 8], "minecraft:stone")],
        )
        .unwrap_err();
        assert_eq!(refusal.refusal, Refusal::Occupied { at: [8, 70, 8] });
        assert_eq!(refusal.index, 0);
        assert_eq!(refusal.name(), "colony_world.refusal.occupied");
    }

    #[test]
    fn a_replacement_and_a_break_need_a_block_to_act_on() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        for change in [
            Change::Replace { at: [8, 70, 8], block: "minecraft:stone".to_owned() },
            Change::Break { at: [8, 70, 8] },
        ] {
            let refusal = Intent::plan(&colony(), &guard, State::Active, vec![change]).unwrap_err();
            assert_eq!(refusal.refusal, Refusal::Empty { at: [8, 70, 8] });
        }
    }

    #[test]
    fn a_container_is_refused_by_the_three_writes_and_kept_by_the_keeper() {
        let world = Cells::of(Occupant::Container);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        for change in [
            place([8, 70, 8], "minecraft:stone"),
            Change::Replace { at: [8, 70, 8], block: "minecraft:stone".to_owned() },
            Change::Break { at: [8, 70, 8] },
        ] {
            let refusal = Intent::plan(&colony(), &guard, State::Active, vec![change]).unwrap_err();
            assert_eq!(
                refusal.refusal,
                Refusal::WouldDropContents { at: [8, 70, 8] },
                "edit_blocks takes no NBT and a break drops nothing"
            );
        }
        let kept = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![Change::Keep { at: [8, 70, 8] }],
        )
        .expect("a keep is what a container is for");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept.emitted(), 0, "and it writes nothing");
    }

    #[test]
    fn the_five_guard_questions_run_before_the_operation_is_looked_at() {
        // A cell both outside every claim and solid: the claim answers, because a
        // placement is not even meaningful until the land belongs to the colony.
        let world = Cells::of(Occupant::Solid);
        let guard = Guard::new(OVERWORLD, &Unclaimed, &world);
        let refusal = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![place([8, 70, 8], "minecraft:stone")],
        )
        .unwrap_err();
        assert_eq!(refusal.name(), "manager.refusal.no_colony_at");
    }

    #[test]
    fn an_inactive_colony_writes_nothing_whatever_the_cell_holds() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let refusal = Intent::plan(
            &colony(),
            &guard,
            State::Inactive,
            vec![place([8, 70, 8], "minecraft:stone")],
        )
        .unwrap_err();
        assert_eq!(refusal.name(), "colony_world.refusal.inactive");
    }

    #[test]
    fn the_refusal_names_the_change_the_caller_gave_not_the_sorted_one() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        // Two cells are in the claim, the third is not: the batch fails at index 1.
        let refusal = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 70, 8], "minecraft:stone"),
                place([40, 70, 8], "minecraft:stone"),
                place([9, 70, 8], "minecraft:stone"),
            ],
        )
        .unwrap_err();
        assert_eq!(refusal.index, 1);
        assert_eq!(refusal.at, [40, 70, 8]);
    }

    #[test]
    fn nothing_reaches_the_engine_when_one_change_is_refused() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let mut sink = Recorder::default();
        let planned = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 70, 8], "minecraft:stone"),
                place([9, 70, 8], "minecraft:stone"),
                place([300, 70, 8], "minecraft:stone"),
            ],
        );
        // There is no `Intent` to emit, so the sink cannot be reached: the guarantee is
        // the constructor's, not the caller's care.
        if let Ok(intent) = planned {
            intent.emit(&mut sink);
        }
        assert!(sink.writes.is_empty(), "no write before a refusal");
    }

    #[test]
    fn an_accepted_batch_is_emitted_in_the_canonical_order() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let intent = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([9, 72, 0], "minecraft:stone"),
                place([8, 71, 0], "minecraft:dirt"),
                place([8, 72, 0], "minecraft:sand"),
            ],
        )
        .expect("all three are in the claim and on air");
        let mut sink = Recorder::default();
        intent.emit(&mut sink);
        assert_eq!(
            sink.writes.iter().map(|(at, _)| *at).collect::<Vec<[i32; 3]>>(),
            vec![[8, 71, 0], [8, 72, 0], [9, 72, 0]],
            "y first, then x, then z: the order structure_template reports"
        );
    }

    #[test]
    fn the_canonical_key_is_y_then_x_then_z() {
        assert_eq!(key([5, 6, 7]), (6, 5, 7));
        // Le y le plus bas trie en premier : y=1 avant y=2. L assertion precedente comparait
        // (2,0,0) < (1,0,0), qui est faux — l attendee etait inversee, pas la cle.
        assert!(key([0, 1, 0]) < key([0, 2, 0]), "a lower y sorts first");
        assert!(key([1, 70, 0]) > key([0, 70, 0]), "then x");
        assert!(key([0, 70, 1]) > key([0, 70, 0]), "then z");
    }

    #[test]
    fn two_changes_on_one_cell_are_a_conflict_and_the_first_index_is_named() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let refusal = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 70, 8], "minecraft:stone"),
                place([9, 70, 8], "minecraft:stone"),
                Change::Keep { at: [8, 70, 8] },
            ],
        )
        .unwrap_err();
        assert_eq!(
            refusal.refusal,
            Refusal::ConflictingWrite { at: [8, 70, 8], first: 0, second: 2 }
        );
        assert_eq!(refusal.index, 2);
    }

    #[test]
    fn a_keep_never_reaches_the_sink_and_a_break_writes_air() {
        let world = Cells::of(Occupant::Solid);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let intent = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![Change::Break { at: [8, 70, 8] }, Change::Keep { at: [9, 70, 8] }],
        )
        .expect("a break on a solid block, a keep on another");
        assert_eq!(intent.len(), 2);
        assert_eq!(intent.emitted(), 1);
        let mut sink = Recorder::default();
        intent.emit(&mut sink);
        assert_eq!(
            sink.writes,
            vec![([8, 70, 8], "minecraft:air".to_owned())],
            "the break is a write of air, the keep is not a write at all"
        );
    }

    #[test]
    fn two_callers_naming_the_same_cells_hand_the_engine_the_same_sequence() {
        let world = Cells::of(Occupant::Replaceable);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let forwards = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 70, 8], "minecraft:stone"),
                place([9, 70, 8], "minecraft:dirt"),
                place([8, 71, 8], "minecraft:sand"),
            ],
        )
        .expect("three cells in the claim, on air");
        let backwards = Intent::plan(
            &colony(),
            &guard,
            State::Active,
            vec![
                place([8, 71, 8], "minecraft:sand"),
                place([9, 70, 8], "minecraft:dirt"),
                place([8, 70, 8], "minecraft:stone"),
            ],
        )
        .expect("the same three cells, the other way round");
        let mut one = Recorder::default();
        let mut other = Recorder::default();
        forwards.emit(&mut one);
        backwards.emit(&mut other);
        assert_eq!(one.writes, other.writes, "one canonical order, not two");
    }

    #[test]
    fn an_empty_batch_plans_to_nothing_and_emits_nothing() {
        let guard = Guard::new(OVERWORLD, &OWNED, &Open);
        let intent =
            Intent::plan(&colony(), &guard, State::Active, Vec::new()).expect("nothing to refuse");
        assert!(intent.is_empty());
        assert_eq!(intent.emitted(), 0);
        let mut sink = Recorder::default();
        intent.emit(&mut sink);
        assert!(sink.writes.is_empty());
    }
}
