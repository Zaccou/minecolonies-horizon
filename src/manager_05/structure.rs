//! What an event leaves in the ground, and how the ground is given back.
//!
//! Java reference: `core/colony/managers/EventStructureManager.java` (212 lines),
//! with the anchor arithmetic of `BlueprintTagUtils.getNumberOfGroundLevels`
//! (`strukturize-src/.../blueprints/v1/BlueprintTagUtils.java:100-110`), the
//! folder name `Constants.BLUEPRINT_FOLDER`
//! (`api/java/com/ldtteam/structurize/api/util/constant/Constants.java:26`) and
//! the list key `NbtTagConstants.java:516`.
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | when a backup is refused | the event id names nothing, **before** the anchor is even computed | `:88-91` |
//! | where the anchor is | the centre of the copied box, at its floor | `:95-98` |
//! | where the copy is filed | `blueprints/structbackup/<colony>/<namespace><path>/<anchor>.blueprint` | `:100-105` |
//! | what two events on one anchor do | the second overwrites the first: the map is keyed by position | `:58`, `:112` |
//! | when a backup is given back | at `onFinish`, **before** the event leaves the map | `:194`, `EventManager.java:191-197` |
//! | what a missing event on load does | restores **immediately**, then forgets the entry | `:186-190` |
//!
//! **The `structbackup` folder is the only thing Java remembers of it.** The map
//! holds `BlockPos → eventID` and nothing else (`:58`); the blocks themselves are
//! a blueprint file on disk. A wasm mod has no `std::fs` and no
//! `StructurePacks.storeBlueprint`, so the port keeps the decision — which anchor,
//! which event, which file name — and hands the file work to the caller: what it
//! returns is a [`Plan`] the host must execute, and a [`Restore`] it must place.
//! `MANQUANT.md` §2 is the missing blueprint writer and reader.
//!
//! **Three rules the port keeps and a tidy implementation would lose.**
//!
//! 1. **The list key is `ships`** (`NbtTagConstants.java:516`). It is
//!    `TAG_SCHEMATIC_LIST`, and it holds raid ship backups as much as anything
//!    else — a backup saved under `backups` is never read by this version.
//! 2. **The dimension is concatenated, not joined.** `getNamespace() +
//!    getPath()` (`:104`) writes `minecraftoverworld`, with no separator. That is
//!    the folder the restore looks in, and [`backup_path`] reproduces it.
//! 3. **A cancelled event keeps its backup.** `EventManager.java:202-206` removes
//!    a `CANCELED` event without `onFinish`, so nothing calls
//!    [`Backups::restore`] — and the entry is still there. It is dropped at the
//!    next load, where its event is gone, and **restored then**: a cancelled
//!    pirate raid's terrain comes back after a restart. A test below holds it.
//!
//! **Pure rules**: no host call, no clock, no file. Positions in, positions out.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::events::{EventBook, KEY_EVENT_MANAGER};
use super::nbt;

/// The compound the backups live in — at the **colony** level, beside
/// `event_manager`, not inside it (`EventStructureManager.java:48`, written by
/// `EventManager.java:263`).
pub const KEY_STRUCTURE_MANAGER: &str = "eventstructure_manager";
/// The list of backups (`:175`), whose name is `ships`
/// (`NbtTagConstants.java:516`).
pub const KEY_SCHEMATIC_LIST: &str = "ships";
/// The anchor, inside one entry (`:180`, `NbtTagConstants.java:51`).
pub const KEY_POS: &str = "pos";
/// The event the backup belongs to (`:181`, `NbtTagConstants.java:122`).
pub const KEY_EVENT_ID: &str = "mc_event_id";

/// Where the copies live, under the game directory (`:53`).
pub const BACKUP_FOLDER: &str = "structbackup";
/// The folder it sits in (`structurize` `Constants.java:26`).
pub const BLUEPRINT_FOLDER: &str = "blueprints";
/// The extension every backup is written with (`:105`).
pub const BACKUP_EXTENSION: &str = "blueprint";
/// The ground-level default the anchor falls back to
/// (`BlueprintTagUtils.java:100-109`, called with `4` at `:95`).
pub const DEFAULT_GROUND_LEVELS: i32 = 4;

/// Where a backup goes and which event it belongs to: everything Java decides
/// before it writes a file.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Plan {
    /// `:112`: the key of the map, and the base name of the file.
    pub anchor: [i32; 3],
    /// The event that owns the copy.
    pub event: i32,
    /// `:97`: where the structure itself is pasted, which is not the anchor.
    pub spawn: [i32; 3],
    /// `:97`: the corner the copy starts at, which is `spawn` minus the primary
    /// block's offset.
    pub corner: [i32; 3],
}

/// Why no backup was planned. The only refusal Java has here, and it comes
/// **before** anything is computed or written (`:88-91`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one pre-write refusal, named
pub enum Refusal {
    /// `eventManager.getEventByID(eventID) == null`.
    UnknownEvent(i32),
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::UnknownEvent(_) => "unknown_event",
        }
    }

    pub fn event(&self) -> i32 {
        match self {
            Refusal::UnknownEvent(id) => *id,
        }
    }
}

/// The anchor of a backup: `EventStructureManager.java:95-98`.
///
/// `y` is the number of ground levels minus one, and the spawn point is that far
/// **below** the target, then the primary block's own height **above** it. The
/// anchor is the middle of the covered box on `x` and `z` — integer division, so
/// an even size leans one block to the low side, as Java's `/ 2` does.
pub fn anchor(
    target: [i32; 3],
    primary_offset: [i32; 3],
    size: [i32; 3],
    ground_levels: i32,
) -> ([i32; 3], [i32; 3], [i32; 3]) {
    let drop = ground_levels - 1;
    let spawn = [
        target[0],
        target[1] - drop + primary_offset[1],
        target[2],
    ];
    let corner = [
        spawn[0] - primary_offset[0],
        spawn[1] - primary_offset[1],
        spawn[2] - primary_offset[2],
    ];
    let anchor = [
        corner[0] + size[0] / 2,
        corner[1],
        corner[2] + size[2] / 2,
    ];
    (anchor, spawn, corner)
}

/// The file a backup is stored in (`:100-105`):
/// `blueprints/structbackup/<colony>/<namespace><path>/<anchor>.blueprint`.
///
/// **The dimension is concatenated without a separator** — `minecraftoverworld`,
/// not `minecraft/overworld` — because Java adds the two strings (`:104`).
///
/// **Divergence, named.** Java builds the last part with `anchor.toString()`,
/// which is a vanilla `BlockPos` detail (`BlockPos{x=1, y=2, z=3}` in the
/// mappings this tree uses); the port writes `x_y_z` instead, and says why: the
/// port has no `BlockPos` to call, and the only requirement on this name is that
/// the writer and the reader agree — they are this one function. A backup file
/// written by Java cannot be found by the port, which is what `MANQUANT.md` §2
/// records.
pub fn backup_path(colony: i32, dimension: &str, anchor: [i32; 3]) -> String {
    let folder = dimension_folder(dimension);
    format!(
        "{BLUEPRINT_FOLDER}/{BACKUP_FOLDER}/{colony}/{folder}/{}_{}_{}.{BACKUP_EXTENSION}",
        anchor[0], anchor[1], anchor[2]
    )
}

/// `namespace + path`, Java's `:104`. A dimension without a namespace keeps its
/// own name unchanged, which is what `ResourceLocation`'s default namespace would
/// have hidden.
fn dimension_folder(dimension: &str) -> String {
    match dimension.split_once(':') {
        Some((namespace, path)) => format!("{namespace}{path}"),
        None => dimension.to_owned(),
    }
}

/// What the read did `:169-193`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: whether the block of code ran at all, and what it did
pub struct Read {
    /// **False** when the colony compound has no `event_manager`: Java calls this
    /// read only from inside that guard (`EventManager.java:223-245`), so a save
    /// holding `eventstructure_manager` alone never reads its backups — they are
    /// simply lost.
    pub read: bool,
    /// The anchors whose event the save no longer holds. Java restores them
    /// **during the read** (`:186-190`), so they never enter the map.
    pub restored: Vec<[i32; 3]>,
}

/// The map of backups: anchor → event, and the two things done to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Backups {
    entries: BTreeMap<[i32; 3], i32>,
    writes: u32,
}

impl Backups {
    pub fn new() -> Backups {
        Backups::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The anchors of one event, ascending by position — Java walks a `HashMap`
    /// (`:125`) and the port orders, so a restore that touches the same blocks
    /// twice does it in the same order every reload.
    pub fn for_event(&self, event: i32) -> Vec<[i32; 3]> {
        self.entries
            .iter()
            .filter(|(_, owner)| **owner == event)
            .map(|(anchor, _)| *anchor)
            .collect()
    }

    /// `spawnTemporaryStructure` (`:83-120`), reduced to what it decides: the
    /// refusal first (`:88-91`), then the anchor, then the slot.
    ///
    /// **The put is a put**: two events writing the same anchor keep one entry,
    /// the last one, so the first event's blocks are never given back (`:112`).
    pub fn plan(
        &mut self,
        book: &EventBook,
        event: i32,
        target: [i32; 3],
        primary_offset: [i32; 3],
        size: [i32; 3],
        ground_levels: i32,
    ) -> Result<Plan, Refusal> {
        if book.get(event).is_none() {
            return Err(Refusal::UnknownEvent(event));
        }
        let (anchor, spawn, corner) = anchor(target, primary_offset, size, ground_levels);
        self.entries.insert(anchor, event);
        self.writes += 1;
        Ok(Plan {
            anchor,
            event,
            spawn,
            corner,
        })
    }

    /// `loadBackupForEvent` (`:123-166`), as the list of anchors the caller must
    /// place back and whose files it must delete.
    ///
    /// **The entry leaves even when the file cannot be read.** Java removes it
    /// with `iterator.remove()` (`:163`) *outside* the future's callback, so a
    /// missing or corrupt blueprint is logged (`:143-147`) and the backup is gone
    /// for good.
    pub fn restore(&mut self, event: i32) -> Vec<[i32; 3]> {
        let anchors = self.for_event(event);
        if anchors.is_empty() {
            return anchors;
        }
        for anchor in &anchors {
            self.entries.remove(anchor);
        }
        self.writes += 1;
        anchors
    }

    /// `writeToNBT` (`:195-211`): one entry per backup, `mc_event_id` then `pos`,
    /// under `ships`, inside `eventstructure_manager`.
    ///
    /// The compound it writes into is the **colony's** (`:210`), which is why this
    /// takes one instead of returning its own: Java's `EventManager.writeToNBT`
    /// calls it with the colony compound (`EventManager.java:263`), and a port
    /// that nested it inside `event_manager` would write where the read never
    /// looks.
    pub fn write_into(&self, compound: &mut Compound) {
        let records: Vec<Compound> = self
            .entries
            .iter()
            .map(|(anchor, event)| {
                Compound::from([
                    (KEY_EVENT_ID.into(), Tag::Int(*event)),
                    (KEY_POS.into(), nbt::position(*anchor)),
                ])
            })
            .collect();
        let mut manager = Compound::new();
        manager.insert(KEY_SCHEMATIC_LIST.into(), nbt::compound_list(records));
        compound.insert(
            KEY_STRUCTURE_MANAGER.into(),
            Tag::Compound(manager),
        );
    }

    /// `readFromNBT` (`:168-193`), which Java calls **from `EventManager`**
    /// (`:244`), not from the colony: the guard on the left is the colony
    /// compound, the guard on the right is Java's.
    ///
    /// Three rules, all of them Java's:
    ///
    /// - the map is **cleared** first (`:173`), so a reload forgets anything the
    ///   previous state held;
    /// - an entry whose event the save still holds is kept (`:182-185`);
    /// - an entry whose event is gone is **restored right now** and dropped
    ///   (`:186-190`) — not queued, not kept.
    ///
    /// A missing `pos` reads as the origin, because `BlockPosUtil.read` answers
    /// `(0, 0, 0)` for an absent compound (`api/util/BlockPosUtil.java:190-197`)
    /// and Java has no check here.
    pub fn read_from_colony(&mut self, compound: &Compound, book: &EventBook) -> Read {
        if !nbt::holds(compound, KEY_EVENT_MANAGER) {
            return Read {
                read: false,
                restored: Vec::new(),
            };
        }
        let Some(Tag::Compound(manager)) = compound.get(&KEY_STRUCTURE_MANAGER.into())
        else {
            return Read {
                read: true,
                restored: Vec::new(),
            };
        };

        self.entries.clear();
        let mut restored = Vec::new();
        let records = nbt::compounds(manager, KEY_SCHEMATIC_LIST);
        for record in records {
            let anchor = nbt::position_in(record, KEY_POS).unwrap_or([0, 0, 0]);
            let event = nbt::int(record, KEY_EVENT_ID).unwrap_or(0);
            if book.get(event).is_some() {
                self.entries.insert(anchor, event);
            } else {
                restored.push(anchor);
            }
        }
        Read {
            read: true,
            restored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager_05::events::Event;

    fn book_with(ids: &[i32]) -> EventBook {
        let mut book = EventBook::new();
        for id in ids {
            let taken = book.take_id();
            let _ = book.add_event(Event::new(taken, "pirate_raid"));
            assert_eq!(taken, *id, "the book hands ids out in order");
        }
        book
    }

    #[test]
    fn the_anchor_is_the_centre_of_the_copied_box_at_its_floor() {
        // A 10x6x8 ship whose primary block sits at [4, 1, 3], pasted at y=70,
        // with the default four ground levels: `:95-98`.
        let (anchor, spawn, corner) = anchor([100, 70, 200], [4, 1, 3], [10, 6, 8], 4);
        assert_eq!(spawn, [100, 68, 200], "y = 70 - (4 - 1) + 1");
        assert_eq!(corner, [96, 67, 197], "spawn minus the primary offset");
        assert_eq!(anchor, [101, 67, 201], "x = 96 + 10/2, z = 197 + 8/2");
    }

    #[test]
    fn a_missing_event_refuses_before_the_anchor_or_the_slot_is_touched() {
        let book = book_with(&[1]);
        let mut backups = Backups::new();
        assert_eq!(
            backups.plan(&book, 2, [100, 70, 200], [4, 1, 3], [10, 6, 8], 4),
            Err(Refusal::UnknownEvent(2)),
            ":88-91 returns false before it computes anything"
        );
        assert!(backups.is_empty(), "no slot was taken");
        assert_eq!(backups.writes, 0, "and nothing is written");
        assert_eq!(Refusal::UnknownEvent(2).name(), "unknown_event");
        assert_eq!(Refusal::UnknownEvent(2).event(), 2);
    }

    #[test]
    fn the_path_concatenates_the_dimension_and_names_the_anchor() {
        assert_eq!(
            backup_path(7, "minecraft:overworld", [101, 67, 201]),
            "blueprints/structbackup/7/minecraftoverworld/101_67_201.blueprint"
        );
        assert_eq!(
            backup_path(7, "minecraft:the_nether", [-4, 33, 0]),
            "blueprints/structbackup/7/minecraftthe_nether/-4_33_0.blueprint"
        );
        assert_eq!(
            dimension_folder("undeclared"),
            "undeclared",
            "a bare dimension has no namespace to strip"
        );
    }

    #[test]
    fn two_events_on_one_anchor_keep_the_last_one_and_the_first_is_never_given_back() {
        let book = book_with(&[1, 2]);
        let mut backups = Backups::new();
        let first = backups
            .plan(&book, 1, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("event 1 exists");
        let second = backups
            .plan(&book, 2, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("event 2 exists");
        assert_eq!(first.anchor, second.anchor);
        assert_eq!(backups.len(), 1, ":112 is a put on a map keyed by position");
        assert_eq!(backups.for_event(1), Vec::<[i32; 3]>::new(), "event 1 lost its slot");
        assert_eq!(backups.for_event(2), vec![first.anchor]);
    }

    #[test]
    fn restoring_an_event_returns_its_anchors_and_keeps_the_others() {
        let book = book_with(&[1, 2]);
        let mut backups = Backups::new();
        let low = backups
            .plan(&book, 1, [0, 64, 0], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");
        let high = backups
            .plan(&book, 1, [64, 64, 0], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");
        let other = backups
            .plan(&book, 2, [0, 64, 64], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");

        let restored = backups.restore(1);
        assert_eq!(
            restored,
            vec![low.anchor, high.anchor],
            ":125-165 walks the entries and takes the ones of that event"
        );
        assert_eq!(backups.for_event(2), vec![other.anchor], "the other event keeps its copy");
        assert_eq!(backups.restore(1), Vec::<[i32; 3]>::new(), "a second restore finds nothing");
    }

    #[test]
    fn a_cancelled_event_keeps_its_backup_until_the_next_load_restores_it() {
        let book = book_with(&[1]);
        let mut backups = Backups::new();
        let plan = backups
            .plan(&book, 1, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");

        // The event is cancelled: `EventManager.java:202-206` removes it and never
        // calls `onFinish`, so nothing restores it.
        assert_eq!(backups.for_event(1), vec![plan.anchor], "the copy is still owed");

        // It is not saved either, so the next read finds an event it does not
        // know, and restores it there (`:186-190`).
        //
        // The compound is built the way `writeToNBT` builds one: the book first,
        // which **always** puts `event_manager` (`EventManager.java:262`) even
        // with no event left, then the structure manager on the same compound
        // (`:263`). A save carrying only `eventstructure_manager` is a shape
        // Java's writer cannot produce, and `readFromNBT` would not open its
        // guard on it (`:223`).
        let saved = {
            let mut compound = EventBook::new().to_compound();
            backups.write_into(&mut compound);
            compound
        };
        let empty_book = EventBook::new();
        let mut reloaded = Backups::new();
        let read = reloaded.read_from_colony(&saved, &empty_book);
        assert!(
            read.read,
            ":223 — the counter and the list are still written, even empty (:260-262)"
        );
        assert_eq!(
            read.restored,
            vec![plan.anchor],
            "a cancelled raid's terrain comes back after a restart"
        );
        assert!(reloaded.is_empty());
    }

    #[test]
    fn a_load_keeps_the_entries_whose_event_survived_and_clears_the_rest_of_the_map() {
        let book = book_with(&[1]);
        let mut backups = Backups::new();
        let kept = backups
            .plan(&book, 1, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");
        // A colony save holds both keys: `writeToNBT` puts `event_manager`
        // unconditionally (`EventManager.java:262`) and the backups beside it
        // (`:263`). Without the first, the second is never read (`:223`, `:244`).
        let saved = {
            let mut compound = book.to_compound();
            backups.write_into(&mut compound);
            compound
        };

        // The state a reload starts from holds an entry the save never had.
        let mut reloaded = Backups::new();
        reloaded.entries.insert([999, 64, 999], 1);
        let read = reloaded.read_from_colony(&saved, &book);
        assert_eq!(read.restored, Vec::<[i32; 3]>::new(), "event 1 is still in the book");
        assert_eq!(
            reloaded.for_event(1),
            vec![kept.anchor],
            ":173 clears the map, then :182-185 keeps what the save holds"
        );
        assert_eq!(reloaded.len(), 1);
    }

    #[test]
    fn a_colony_compound_without_event_manager_never_reads_its_backups() {
        let book = book_with(&[1]);
        let mut backups = Backups::new();
        let plan = backups
            .plan(&book, 1, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");
        // A shape Java's own writer never produces — `writeToNBT` always puts
        // `event_manager` (`EventManager.java:262`) — built here to hold the guard
        // of `:223`, whose only caller is inside it (`:244`).
        let mut colony = Compound::new();
        backups.write_into(&mut colony);
        assert!(nbt::holds(&colony, KEY_STRUCTURE_MANAGER));

        let empty = EventBook::new();
        let mut read_into = Backups::new();
        let read = read_into.read_from_colony(&colony, &empty);
        assert!(!read.read, "EventManager.java:223-245 is the only caller, and its guard is shut");
        assert_eq!(read.restored, Vec::<[i32; 3]>::new(), "nothing is restored");
        assert!(read_into.is_empty(), "and nothing is read");
        let _ = plan;
    }

    #[test]
    fn a_save_names_its_list_ships_and_keeps_it_outside_event_manager() {
        let book = book_with(&[1]);
        let mut backups = Backups::new();
        let plan = backups
            .plan(&book, 1, [10, 64, 10], [0, 0, 0], [8, 4, 8], 1)
            .expect("exists");
        let mut colony = book.to_compound();
        backups.write_into(&mut colony);

        let Some(Tag::Compound(manager)) =
            colony.get(&KEY_STRUCTURE_MANAGER.into())
        else {
            panic!("the backups are written at the colony level");
        };
        assert!(manager.get(&KEY_EVENT_MANAGER.into()).is_none());
        let records = nbt::compounds(manager, KEY_SCHEMATIC_LIST);
        assert_eq!(records.len(), 1, "the list is called ships, NbtTagConstants.java:516");
        assert_eq!(nbt::int(records[0], KEY_EVENT_ID), Some(1));
        assert_eq!(nbt::position_in(records[0], KEY_POS), Some(plan.anchor));

        let Some(Tag::Compound(events)) = colony.get(&KEY_EVENT_MANAGER.into()) else {
            panic!("the book is still there");
        };
        assert!(
            events.get(&KEY_STRUCTURE_MANAGER.into()).is_none(),
            "and the backups are not inside it"
        );
    }
}
