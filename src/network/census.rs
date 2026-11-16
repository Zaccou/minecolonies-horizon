//! The census: every Java file of `core/network/messages` accounted for, and every
//! Rust type that carries one named once.
//!
//! This module holds no message. It exists so that "**128 files**" is a fact the
//! build checks rather than a number in a report nobody re-measures: the tree has
//! 128 `.java` files under `com/minecolonies/core/network/messages/`, and this
//! module's tables must add up to them.
//!
//! **The two numbers that are not the same number, and why.**
//!
//! - **128 files** in the tree.
//! - **126 of them are messages**: one class, one file, except
//!   `PermissionsMessage.java`, which is a namespace holding nine nested classes
//!   (`PermissionsMessage.java:35-723`).
//! - **2 of them are the abstract bases**: `AbstractColonyServerMessage.java` and
//!   `AbstractBuildingServerMessage.java`. They carry **no class of their own** in
//!   this port: their two fields are `common::envelope` / `common::building_envelope`
//!   and their gate is `common::Message::RULE`, because a message that is not a
//!   message has nothing to round-trip. `ABSTRACT_BASES` names them so the arithmetic
//!   closes.
//! - **134 rows** in the tables below: 125 files with one type each, plus the nine
//!   nested permission classes from the one file that has them.
//!
//! `125 + 1 + 2 = 128` files, `125 + 9 = 134` rows. A test asserts both sums.

/// The two Java files that carry no type of their own: their fields became
/// `common::envelope` and `common::building_envelope`.
pub const ABSTRACT_BASES: [&str; 2] = [
    "server/AbstractColonyServerMessage.java",
    "server/AbstractBuildingServerMessage.java",
];

/// Every family's census, in the order `mod.rs` lists them.
pub fn all() -> Vec<&'static (&'static str, &'static str)> {
    let mut rows: Vec<&'static (&'static str, &'static str)> = Vec::new();
    for table in [
        super::permissions::CENSUS,
        super::founding::CENSUS,
        super::colony_settings::CENSUS,
        super::buildings::CENSUS,
        super::building_modules::CENSUS,
        super::work::CENSUS,
        super::citizens::CENSUS,
        super::views::CENSUS,
        super::particles::CENSUS,
        super::sounds::CENSUS,
        super::windows::CENSUS,
        super::items::CENSUS,
        super::debug::CENSUS,
    ] {
        rows.extend(table.iter());
    }
    rows
}

/// The distinct Java files the tables name, sorted.
pub fn files() -> Vec<&'static str> {
    let mut files: Vec<&'static str> = all().into_iter().map(|(file, _)| *file).collect();
    files.sort_unstable();
    files.dedup();
    files
}

/// The distinct Rust type names the tables name, sorted.
pub fn types() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = all().into_iter().map(|(_, name)| *name).collect();
    types.sort_unstable();
    types.dedup();
    types
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree's file count, measured with
    /// `find src/main/java/com/minecolonies/core/network/messages -name '*.java' | wc -l`
    /// on the source this slice was cut from.
    const JAVA_FILES: usize = 128;

    #[test]
    fn the_tables_account_for_every_java_file_in_the_tree() {
        let files = files();
        assert_eq!(
            files.len() + ABSTRACT_BASES.len(),
            JAVA_FILES,
            "{} message files and {} abstract bases must make {JAVA_FILES}",
            files.len(),
            ABSTRACT_BASES.len()
        );
        for base in ABSTRACT_BASES {
            assert!(!files.contains(&base), "{base} is an abstract base, not a message");
            assert!(base.ends_with(".java"));
        }
    }

    #[test]
    fn the_nested_permission_classes_are_nine_rows_of_one_file() {
        let rows = all();
        assert_eq!(rows.len(), 134, "125 single-class files plus nine nested ones");
        let permissions: Vec<_> = rows.iter().filter(|(file, _)| *file == "PermissionsMessage.java").collect();
        assert_eq!(permissions.len(), 9, "PermissionsMessage.java holds nine classes");
        assert_eq!(
            rows.len() - permissions.len() + 1,
            126,
            "the other rows are one per file"
        );
    }

    #[test]
    fn no_java_file_is_claimed_twice_and_no_type_is_named_twice() {
        let mut by_file: Vec<&'static str> = Vec::new();
        for (file, name) in all() {
            assert!(!name.is_empty(), "{file} has a row with no type");
            assert!(file.ends_with(".java"), "{name} is filed under {file}");
            if *file != "PermissionsMessage.java" {
                assert!(
                    !by_file.contains(file),
                    "{file} is claimed by two tables — each message file has one class"
                );
                by_file.push(file);
            }
        }
        assert_eq!(types().len(), all().len(), "two rows share a Rust type name");
    }

    #[test]
    fn every_family_declares_the_count_its_file_holds() {
        for (family, table, expected) in [
            ("permissions", super::super::permissions::CENSUS, 9),
            ("founding", super::super::founding::CENSUS, 11),
            ("colony_settings", super::super::colony_settings::CENSUS, 11),
            ("buildings", super::super::buildings::CENSUS, 21),
            ("building_modules", super::super::building_modules::CENSUS, 20),
            ("work", super::super::work::CENSUS, 9),
            ("citizens", super::super::citizens::CENSUS, 6),
            ("views", super::super::views::CENSUS, 14),
            ("particles", super::super::particles::CENSUS, 8),
            ("sounds", super::super::sounds::CENSUS, 4),
            ("windows", super::super::windows::CENSUS, 7),
            ("items", super::super::items::CENSUS, 6),
            ("debug", super::super::debug::CENSUS, 8),
        ] {
            assert_eq!(table.len(), expected, "the {family} family is miscounted");
        }
    }

    #[test]
    fn the_file_paths_are_the_trees_own_relative_paths() {
        // `find` prints them from `messages/` without the leading `./`, which is what
        // these rows hold: `client/colony/ColonyViewMessage.java`, not
        // `messages/client/colony/ColonyViewMessage.java`.
        for file in files() {
            assert!(!file.starts_with('/'), "{file} is absolute");
            assert!(!file.starts_with("./"), "{file} carries a dot prefix");
            assert!(!file.contains("messages/"), "{file} carries the root");
        }
        assert!(files().contains(&"clients".replace("clients", "client/StopMusicMessage.java").as_str()));
    }
}
