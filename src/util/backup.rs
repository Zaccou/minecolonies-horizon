//! `BackUpHelper`: when a backup is due, what it is called, and which old ones go.
//!
//! Java reference: `core/util/BackUpHelper.java` (640 lines). It is the largest
//! file of the mission and almost none of it is portable: `backupColonyData`
//! opens a `ZipOutputStream`, walks the world's save directory, and calls
//! `ServerLifecycleHooks.getCurrentServer()`; `loadManagerBackup`,
//! `loadColonyBackup` and `loadMissingColonies` do the reverse. A wasm mod has no
//! save directory and no server object.
//!
//! What is portable is the **policy**, which is four numbers and one comparison:
//!
//! | Rule | Java |
//! | --- | --- |
//! | at most one backup every five minutes | `:81-85` |
//! | keep twenty, delete the oldest first | `:141-158` |
//! | the file is `colonies-<yyyy-MM-dd_HH.mm.ss>.zip` | `:236-239` |
//! | a colony export is `colony<id>Export.zip` | `:51`, `:496` |
//!
//! # One order that matters, and one side effect that should not be there
//!
//! `backupColonyData` sets `lastBackupTime` **before** doing any work
//! (`:85`), so a backup that throws halfway still consumes the five-minute
//! window and the next attempt is refused. That is a debatable design and it is
//! Java's, so [`backup_due`] is a pure test on two timestamps and the *caller*
//! decides whether to stamp before or after — the port's answer is "after a
//! failure, stamp anyway", said out loud in the doc instead of hidden in an
//! assignment order.
//!
//! Worse, the same method **marks colonies deleted while it is taking a backup**
//! (`:107-111`): a colony whose manager is missing gets its file renamed to
//! `colony<id>.dat_deleted` and that *is* the backup's job (so the zip holds the
//! right name), but it means a backup of a broken save mutates the save. Nothing
//! of that is portable and nothing of it is copied; recorded in the report.

/// At most one automatic backup every five minutes:
/// `MAX_TIME_TO_NEXT_BACKUP = 1000 * 60 * 5` (`BackUpHelper.java:62`), in
/// milliseconds.
pub const BACKUP_INTERVAL_MS: i64 = 1000 * 60 * 5;

/// How many backups are kept: `MAX_BACKUPS = 20` (`BackUpHelper.java:57`).
pub const MAX_BACKUPS: usize = 20;

/// How many colony files a recovery scan will try before giving up:
/// `MAX_COLONY_LOAD = 5000` (`BackUpHelper.java:41`), used by
/// `loadMissingColonies` (`:204`) together with a "five misses in a row" stop.
pub const MAX_COLONY_LOAD: usize = 5000;

/// The prefix every backup file carries: `FILENAME_MINECOLONIES_BACKUP =
/// "colonies-%s.zip"` (`api/util/constant/ColonyManagerConstants.java:26`), which
/// is what the retention loop filters on (`BackUpHelper.java:127`).
pub const BACKUP_PREFIX: &str = "colonies-";

/// The export file of one colony: `FILENAME_EXPORT = "colony%sExport.zip"`
/// (`BackUpHelper.java:51`), formatted at `:496` and `:638`.
pub const EXPORT_NAME_PATTERN: &str = "colony%sExport.zip";

/// Is this backup due? `backupColonyData` (`BackUpHelper.java:81-85`).
///
/// ```java
/// if (System.currentTimeMillis() - lastBackupTime < MAX_TIME_TO_NEXT_BACKUP) return false;
/// lastBackupTime = System.currentTimeMillis();
/// ```
///
/// The comparison is **strictly less than**: exactly five minutes after the last
/// one, a backup is allowed. A clock that went backwards gives a negative
/// elapsed time, which is also `< interval`, so a clock correction delays the
/// backup instead of firing a burst of them — Java's outcome, kept.
///
/// `last_backup_ms` of `0` is Java's initial value (`:61`), which on any real
/// clock means "long overdue": the first backup after a server start always runs.
pub fn backup_due(last_backup_ms: i64, now_ms: i64) -> bool {
    now_ms - last_backup_ms >= BACKUP_INTERVAL_MS
}

/// Does this file name look like a backup: the filter of the retention loop
/// (`BackUpHelper.java:127`).
///
/// ```java
/// if (current.isDirectory() || !current.exists() || !current.getName().contains("colonies-")) continue;
/// ```
///
/// A **`contains`**, not a prefix test, and the directory test is separate — so
/// the caller passes a name it already knows is a file. `colonies-2026-11-17_13.05.00.zip`
/// matches; `colony5.dat` does not; `my-colonies-backup.zip` **does**, because
/// Java never anchors the pattern.
pub fn is_backup_file(name: &str) -> bool {
    name.contains(BACKUP_PREFIX)
}

/// How many backups to delete: `fileList.size() - MAX_BACKUPS` (`:148`).
///
/// ```java
/// if (fileList.size() <= MAX_BACKUPS) return true;
/// int deleteCount = fileList.size() - MAX_BACKUPS;
/// ```
///
/// The loop that follows sorts by `lastModified` **ascending** and deletes the
/// first `deleteCount` — the oldest. So this is the count, and
/// [`oldest_to_delete`] is which ones.
pub fn deletions_needed(file_count: usize) -> usize {
    file_count.saturating_sub(MAX_BACKUPS)
}

/// Which files go, given their modification times: the `deleteCount` **oldest**
/// (`BackUpHelper.java:151-158`).
///
/// Returns indices into `modified_ms`, oldest first, capped at
/// [`deletions_needed`]. Java's `List.sort(Comparator.comparingLong(File::lastModified))`
/// is stable, so two backups with the same timestamp keep their list order and the
/// earlier one goes first — reproduced here by sorting on `(time, index)`.
///
/// At the boundary nothing is deleted: 20 backups are kept as they are, and the
/// 21st pushes exactly one out. A list already sorted newest-first is the case
/// Java's sort fixes; it is also the case a caller is most likely to hand over.
pub fn oldest_to_delete(modified_ms: &[i64]) -> Vec<usize> {
    let count = deletions_needed(modified_ms.len());
    let mut order: Vec<usize> = (0..modified_ms.len()).collect();
    order.sort_by_key(|index| (modified_ms[*index], *index));
    order.truncate(count);
    order
}

/// The file name of a colony export: `FILENAME_EXPORT` formatted with the colony
/// id (`BackUpHelper.java:496`).
///
/// `String.format("colony%sExport.zip", id)` — `%s` and not `%d`, so the id is
/// formatted as a string; the answer is the same for a positive id and differs for
/// nothing a colony id can be. Kept as a plain concatenation because `format!`
/// with a `{}` is the same string without a format-specifier to get wrong.
pub fn export_filename(colony_id: i32) -> String {
    format!("colony{}Export.zip", colony_id)
}

/// The folder a dimension's colony files live in:
/// `getFolderForDimension` (`BackUpHelper.java:400-403`).
///
/// ```java
/// return location.getNamespace() + File.separator + location.getPath() + File.separator;
/// ```
///
/// `minecraft` + `/` + `overworld` + `/`, so `minecraft/overworld/` — a
/// **relative** path under the save's `minecolonies` folder, which is why the
/// separator matters and why a dimension with a nested path
/// (`minecraft/dimensions/the_nether`) gives three levels.
pub fn dimension_folder(namespace: &str, path: &str) -> String {
    format!("{namespace}/{path}/")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first backup after a start is always due, and the boundary is the
    /// interval itself.
    #[test]
    fn a_backup_is_due_at_the_interval_and_not_a_millisecond_before() {
        let start = 1_000_000;
        assert!(backup_due(0, start));
        assert!(!backup_due(start, start + BACKUP_INTERVAL_MS - 1));
        assert!(backup_due(start, start + BACKUP_INTERVAL_MS));
        assert!(backup_due(start, start + BACKUP_INTERVAL_MS * 2));
    }

    /// A clock that jumped backwards delays the backup instead of firing a burst.
    #[test]
    fn a_clock_that_went_backwards_delays_rather_than_bursts() {
        let last = 1_000_000;
        assert!(!backup_due(last, last - 60_000));
        assert!(!backup_due(last, last - BACKUP_INTERVAL_MS));
        // Even a whole interval backwards is still "not yet", because the elapsed
        // time is negative and the comparison is `>=`.
        assert!(!backup_due(last, last - BACKUP_INTERVAL_MS - 1));
    }

    /// The name filter is a `contains`, so a hand-renamed backup still counts.
    #[test]
    fn the_name_filter_is_a_substring_and_not_a_prefix() {
        assert!(is_backup_file("colonies-2026-11-17_13.05.00.zip"));
        assert!(is_backup_file("my-colonies-backup.zip"));
        assert!(!is_backup_file("colony5.dat"));
        assert!(!is_backup_file("colonies.zip"));
    }

    /// Twenty is the cap: 20 deletes none, 21 deletes one, 25 deletes five.
    #[test]
    fn the_retention_count_is_the_surplus_over_twenty() {
        assert_eq!(deletions_needed(0), 0);
        assert_eq!(deletions_needed(MAX_BACKUPS), 0);
        assert_eq!(deletions_needed(MAX_BACKUPS + 1), 1);
        assert_eq!(deletions_needed(MAX_BACKUPS + 5), 5);
    }

    /// The **oldest** go, whatever order the list came in.
    #[test]
    fn the_oldest_backups_are_the_ones_deleted() {
        // 21 files, given newest-first so the sort has to do the work.
        let mut times: Vec<i64> = (0..21i64).rev().map(|i| 1_000 + i).collect();
        let doomed = oldest_to_delete(&times);
        assert_eq!(doomed, vec![20]); // the last element, 1_000 — the oldest
        times.swap(0, 20);
        // Same set of times, same oldest one, however the list is ordered.
        assert_eq!(oldest_to_delete(&times), vec![0]);
    }

    /// Nothing is deleted while the table is at or under the cap.
    #[test]
    fn a_full_table_at_the_cap_deletes_nothing() {
        let times: Vec<i64> = (0..MAX_BACKUPS as i64).collect();
        assert!(oldest_to_delete(&times).is_empty());
    }

    /// Two backups with the same timestamp keep their list order, like Java's
    /// stable sort.
    #[test]
    fn a_timestamp_tie_keeps_the_list_order() {
        let mut times = vec![500i64; 21];
        times[0] = 400; // the only genuinely older one
        let doomed = oldest_to_delete(&times);
        assert_eq!(doomed.len(), 1);
        assert_eq!(doomed[0], 0);
    }

    /// The export name is `colony<id>Export.zip`, `%s` and all.
    #[test]
    fn the_export_name_carries_the_colony_id() {
        assert_eq!(export_filename(1), "colony1Export.zip");
        assert_eq!(export_filename(0), "colony0Export.zip");
        assert_eq!(export_filename(-3), "colony-3Export.zip");
    }

    /// A dimension's folder is namespace/path, with a trailing separator.
    #[test]
    fn the_dimension_folder_is_namespace_then_path() {
        assert_eq!(dimension_folder("minecraft", "overworld"), "minecraft/overworld/");
        assert_eq!(
            dimension_folder("minecraft", "dimensions/the_nether"),
            "minecraft/dimensions/the_nether/"
        );
    }
}
