//! What a colony becomes when nobody comes back: an away timer, and a deletion
//! that says what it takes down.
//!
//! Java reference: `core/colony/managers/ColonyPackageManager.java:141-160`
//! (the away timer), `api/util/constant/Constants.java:33` and
//! `api/util/constant/ColonyConstants.java:56` (the two tick constants),
//! `core/colony/Colony.java:828` (where the timer is read) and `:993` (where it
//! is written), `core/colony/ColonyManager.java:353-365` (`getColoniesAbandonedSince`),
//! `:140-220` (the deletion) and `core/util/BackUpHelper.java:357-372` (the
//! `.deleted` tombstone).
//!
//! **"Abandoned" is two unrelated things in Java, and this module only owns one.**
//! The away timer counts the hours nobody with a colony-wide permission has been
//! near the colony (`ColonyPackageManager.java:141-160`), and it is saved under the
//! key `abandoned` (`Colony.java:993`). The *other* one,
//! `Permissions.setOwnerAbandoned` (`permissions/Permissions.java:518-526`),
//! replaces the owner with a random uuid named `[abandoned]`; that is a permissions
//! rule and belongs to `crate::colony::permissions`, which has no such function yet
//! (`MANQUANT.md` §3).
//!
//! **Nothing in this version deletes a colony on its own.** `getColoniesAbandonedSince`
//! has no caller in the Java tree, and `canBeAutoDeleted` is only printed by
//! `/colony printstats` (`CommandColonyPrintStats.java:63`): the flag exists, the
//! automation does not. The port keeps the same shape — the question is answered,
//! the deletion is always asked for — rather than inventing a sweeper.

use mod_api::{Compound, Tag};

use crate::colony::{Building, Citizen};

/// One update of the away timer is this many ticks
/// (`ColonyConstants.java:56`): the colony manager ticks every hundred ticks.
pub const UPDATE_STATE_INTERVAL: i32 = 100;

/// One hour, in ticks (`Constants.java:33`: `20 * 60 * 60`).
pub const TICKS_HOUR: i32 = 72_000;

/// The hours nobody has been near a colony, and the ticks towards the next one —
/// Java's `lastContactInHours` and `ticksPassed` (`ColonyPackageManager.java:63-76`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Attendance {
    ticks: i32,
    hours: i32,
}

impl Attendance {
    pub fn new() -> Attendance {
        Attendance::default()
    }

    /// The timer a save carries: `Colony.java:828` reads the `abandoned` key
    /// straight into the hour count, and the ticks towards the next hour start at
    /// zero. A negative count — a hand-edited save — is read as zero.
    pub fn from_hours(hours: i32) -> Attendance {
        Attendance {
            ticks: 0,
            hours: hours.max(0),
        }
    }

    pub fn hours(&self) -> i32 {
        self.hours
    }

    pub fn ticks(&self) -> i32 {
        self.ticks
    }

    /// One update, `updateAwayTime` (`ColonyPackageManager.java:141-160`).
    ///
    /// `watched` is Java's "`importantColonyPlayers` is not empty": a colony
    /// somebody is watching has its timer reset outright (`:152-158`), an empty one
    /// counts (`:143-151`).
    ///
    /// **Java counts the hour one update late, and the port keeps the quirk.** The
    /// counter is compared to [`TICKS_HOUR`] *before* the interval is added (`:145-151`),
    /// so the hour lands on the 721st update, not the 720th: 72 000 ticks of updates
    /// give 0 hours and the next one gives 1. Fixing it would move every saved
    /// colony's timer by one update against a running Java server; the test below
    /// pins the behaviour instead.
    pub fn update(&mut self, watched: bool) -> bool {
        if watched {
            let changed = self.hours != 0 || self.ticks != 0;
            self.hours = 0;
            self.ticks = 0;
            return changed;
        }
        let mut changed = false;
        if self.ticks >= TICKS_HOUR {
            self.ticks = 0;
            self.hours += 1;
            changed = true;
        }
        self.ticks += UPDATE_STATE_INTERVAL;
        changed
    }

    /// The same, `updates` times — a server that skipped ticks catches up.
    pub fn advance(&mut self, watched: bool, updates: u32) {
        for _ in 0..updates {
            self.update(watched);
        }
    }
}

/// One colony's deletion, described rather than performed: what
/// `ColonyManager.deleteColony` (`:140-220`) takes down, in the order it takes it.
///
/// The list is the deliverable. Java does all of it inline, with the world in hand;
/// a wasm mod cannot, so the caller applies this and answers the host calls. Every
/// field says which line of Java it comes from, and none of them is a guess.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Dissolution {
    pub id: i32,
    pub dimension: String,
    pub center: [i32; 3],
    /// The chunks the colony still claimed, released, in `(x, z)` order.
    ///
    /// Java walks a box of twice the founding radius (`:159` →
    /// `ChunkDataHelper.java:135-139`). The port releases that box **and** whatever
    /// else the book held for the colony, because a claim past the box would keep
    /// answering with an id the list has just freed.
    pub chunks: Vec<[i32; 2]>,
    /// The citizens the deletion kills (`:161-165`, a console damage source).
    pub citizens: Vec<Citizen>,
    /// The buildings it removes (`:168-189`).
    pub buildings: Vec<Building>,
    /// Whether the buildings' blocks go with them: Java's `canDestroy`, which turns
    /// `building.deconstruct()` on (`:174-177`). `false` only drops the records, which
    /// is what `/colony delete <id> false` asks for (`CommandDeleteColony.java:160-173`).
    pub cleared: bool,
    /// The id the store hands out next, now that this one is free
    /// (`ColonyList.java:63-71`, `:148-162`).
    pub freed_id: i32,
}

impl Dissolution {
    /// The record to write where the colony stood, so a later read does not
    /// resurrect it: a colony compound whose `id` is `0`, which `crate::colony`
    /// reads back as nothing at all (`colony/mod.rs:41-44` in `crate::wiring`
    /// makes the same choice for a deleted record).
    ///
    /// Java renames the colony's file to `colony<id>.dat.deleted` instead
    /// (`BackUpHelper.java:357-372`); the port has no file, and a block entity's
    /// `merge_data` cannot remove a key (`crates/horizon-mod-vocab/src/host/block_entities.rs:107-120`),
    /// so the tombstone has to be a value rather than an absence.
    pub fn tombstone(&self) -> Compound {
        Compound::from([(
            crate::colony::colonie::KEY_ID.into(),
            Tag::Int(crate::manager::NO_COLONY_ID),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Colony;

    #[test]
    fn seventy_two_thousand_ticks_are_an_hour_and_java_counts_it_one_update_late() {
        let mut attendance = Attendance::new();
        attendance.advance(false, 720);
        assert_eq!(attendance.ticks(), TICKS_HOUR, "720 updates of 100 ticks");
        assert_eq!(attendance.hours(), 0, "the counter is compared before it is added");
        attendance.update(false);
        assert_eq!(attendance.hours(), 1, "so the hour lands on the 721st update");
        assert_eq!(attendance.ticks(), UPDATE_STATE_INTERVAL);
        attendance.advance(false, 720);
        assert_eq!(attendance.hours(), 2);
    }

    #[test]
    fn an_update_says_whether_the_saved_hour_count_changed() {
        let mut attendance = Attendance::new();
        assert!(!attendance.update(false), "no hour yet, nothing to save");
        attendance.advance(false, 719);
        assert_eq!(
            (attendance.ticks(), attendance.hours()),
            (TICKS_HOUR, 0),
            "72 000 ticks in, and the hour has not landed yet"
        );
        assert!(attendance.update(false), "the hour moved, the colony is dirty");
        assert_eq!((attendance.ticks(), attendance.hours()), (UPDATE_STATE_INTERVAL, 1));
        assert!(!attendance.update(false), "and the next update changes nothing again");
        assert!(attendance.update(true), "a watcher resetting a non-zero timer is a change");
        assert!(!attendance.update(true), "resetting a zero timer is not");
    }

    #[test]
    fn a_watched_colony_forgets_its_absence_and_an_unwatched_one_keeps_counting() {
        let mut attendance = Attendance::from_hours(5);
        attendance.advance(false, 3);
        assert_eq!(attendance.hours(), 5, "counting on");
        assert!(attendance.ticks() > 0);
        attendance.update(true);
        assert_eq!((attendance.hours(), attendance.ticks()), (0, 0), "a watcher resets both");
        attendance.advance(false, 721);
        assert_eq!(attendance.hours(), 1, "and the count starts again");
    }

    #[test]
    fn a_saved_hour_count_is_read_back_and_a_negative_one_is_no_absence() {
        assert_eq!(Attendance::from_hours(12).hours(), 12);
        assert_eq!(Attendance::from_hours(12).ticks(), 0, "the ticks are not saved");
        assert_eq!(Attendance::from_hours(-3).hours(), 0, "a hand-edited save cannot un-age");
    }

    #[test]
    fn the_tombstone_reads_as_no_colony_at_all() {
        let dissolution = Dissolution {
            id: 7,
            dimension: "minecraft:overworld".to_owned(),
            center: [0, 70, 0],
            chunks: Vec::new(),
            citizens: Vec::new(),
            buildings: Vec::new(),
            cleared: false,
            freed_id: 7,
        };
        let record = Colony::from_compound(&dissolution.tombstone());
        assert_eq!(record.id, 0, "an id of zero is `no colony`, not colony zero");
        assert!(record.name.is_empty());
        assert!(record.citizens.is_empty());
        assert!(
            record.members.iter().next().is_none(),
            "a tombstone has no owner, so nobody inherits a deleted colony"
        );
    }
}
