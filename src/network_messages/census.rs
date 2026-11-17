//! The cross-checks: the four files of this module against each other and against
//! [`crate::network`], and `MANQUANT.md` against the table it is computed from.
//!
//! Holds no message and no rule. Its job is that the counts in the module note are facts
//! the build checks rather than numbers in a report, in the same spirit as
//! the census tables `src/network` keeps (one per family, `pub const CENSUS`).
//!
//! One test reads `MANQUANT.md` at compile time (`include_str!`) and holds its five
//! absences and three partials to [`effects::absent`] and [`effects::partial`]. That is
//! deliberate: `MANQUANT.md` is the one file of this module that a compiler cannot see, so
//! the risk there is not a typo but a *stale* list — a capability found later, dropped
//! from the code and left in the document. The reverse is caught too.

/// Every `(file, type)` pair the thirteen CENSUS tables of [`crate::network`] hold.
///
/// `network::census` is a private module (`mod census;` in `src/network/mod.rs:102`), so
/// this module rebuilds the same union from the family tables, which are public. The
/// arithmetic is the same one `network::census` checks for itself: `125 + 9 = 134` rows,
/// `126` distinct files.
pub fn network_rows() -> Vec<(&'static str, &'static str)> {
    let mut rows: Vec<(&'static str, &'static str)> = Vec::new();
    for table in [
        crate::network::permissions::CENSUS,
        crate::network::founding::CENSUS,
        crate::network::colony_settings::CENSUS,
        crate::network::buildings::CENSUS,
        crate::network::building_modules::CENSUS,
        crate::network::work::CENSUS,
        crate::network::citizens::CENSUS,
        crate::network::views::CENSUS,
        crate::network::particles::CENSUS,
        crate::network::sounds::CENSUS,
        crate::network::windows::CENSUS,
        crate::network::items::CENSUS,
        crate::network::debug::CENSUS,
    ] {
        rows.extend_from_slice(table);
    }
    rows
}

/// The distinct Java files [`crate::network`] holds a type for, sorted.
pub fn network_files() -> Vec<&'static str> {
    let mut files: Vec<&'static str> = network_rows().into_iter().map(|(file, _)| file).collect();
    files.sort_unstable();
    files.dedup();
    files
}

/// The distinct Rust type names [`crate::network`] holds, sorted.
pub fn network_types() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = network_rows().into_iter().map(|(_, name)| name).collect();
    types.sort_unstable();
    types.dedup();
    types
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_messages::bounds;
    use crate::network_messages::catalogue::{self, Side};
    use crate::network_messages::effects::{self, Effect, Verdict};
    use crate::network_messages::fragments;

    const MANQUANT: &str = include_str!("MANQUANT.md");
    const RAPPORT: &str = include_str!("RAPPORT.md");

    #[test]
    fn the_catalogue_and_network_name_the_same_files() {
        let mut ours = catalogue::files();
        let mut theirs = network_files();
        ours.sort_unstable();
        theirs.sort_unstable();
        assert_eq!(ours.len(), 126, "126 message files: 128 less the two bases");
        assert_eq!(ours, theirs, "a file is in one table and not the other");
    }

    #[test]
    fn the_catalogue_and_network_name_the_same_types() {
        let mut ours: Vec<&str> = catalogue::CATALOGUE.iter().map(|row| row.rust).collect();
        ours.sort_unstable();
        let mut theirs = network_types();
        theirs.sort_unstable();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn each_family_of_network_is_reached_by_the_rows_it_should_be() {
        // one row per type is already checked in `catalogue`; here the question is
        // whether the rows are spread over the families the way the tree is.
        let permissions = catalogue::of_file("PermissionsMessage.java").len();
        let client = catalogue::CATALOGUE
            .iter()
            .filter(|row| row.file.starts_with("client/"))
            .count();
        let server = catalogue::CATALOGUE
            .iter()
            .filter(|row| row.file.starts_with("server/"))
            .count();
        let splitting = catalogue::CATALOGUE
            .iter()
            .filter(|row| row.file.starts_with("splitting/"))
            .count();
        assert_eq!(permissions, 9);
        assert_eq!(client, 42, "42 files under client/");
        assert_eq!(server, 82, "82 files under server/");
        assert_eq!(splitting, 1);
        assert_eq!(client + server + splitting + 1, 126, "125 files plus the nine");
    }

    #[test]
    fn every_capability_a_verb_needs_has_a_verdict() {
        for effect in Effect::ALL {
            for need in effects::needs(effect) {
                let finding = effects::Finding::of(*need);
                assert_eq!(finding.need, *need);
            }
        }
    }

    #[test]
    fn an_absent_capability_is_absent_because_a_message_needs_it() {
        for finding in effects::absent() {
            let users: Vec<Effect> = Effect::ALL
                .into_iter()
                .filter(|effect| effects::needs(*effect).contains(&finding.need))
                .collect();
            assert!(
                !users.is_empty(),
                "{:?} is absent and no verb asks for it: drop the row instead",
                finding.need
            );
        }
    }

    #[test]
    fn manquant_lists_exactly_the_absent_and_partial_capabilities() {
        for finding in effects::absent().into_iter().chain(effects::partial()) {
            let name = format!("{:?}", finding.need);
            assert!(
                MANQUANT.contains(&name),
                "{name} is {} in the table and not in MANQUANT.md",
                match finding.verdict {
                    Verdict::Absent => "absent",
                    Verdict::Partial => "partial",
                    _ => "present",
                }
            );
        }
    }

    #[test]
    fn manquant_does_not_claim_a_capability_the_code_calls_present() {
        for finding in effects::VERDICTS
            .iter()
            .filter(|f| f.verdict == Verdict::Present)
        {
            let name = format!("{:?}", finding.need);
            // a present capability may be *mentioned* in MANQUANT.md (as the closest
            // thing that exists), but it may not have a heading of its own
            assert!(
                !MANQUANT.contains(&format!("## {name}")),
                "{name} has a MANQUANT heading and a Present verdict"
            );
        }
    }

    #[test]
    fn the_rapport_names_the_defects_the_bounds_module_encodes() {
        // `RAPPORT.md` §3 lists the Java defects found on the way. Each is a rule this
        // module had to take a position on, so each must appear in both.
        for anchor in [
            "remainder",
            "levels.get",
            "removeItem",
            "equals",
            "maxStage",
            "communicatio",
        ] {
            assert!(
                RAPPORT.contains(anchor),
                "RAPPORT.md lost the {anchor} finding that bounds.rs encodes"
            );
        }
    }

    #[test]
    fn the_split_arithmetic_crosses_the_payload_cap() {
        assert_eq!(fragments::MAX_FRAGMENT, 943_718);
        assert!(fragments::MAX_FRAGMENT < fragments::MAX_PAYLOAD);
        assert!(
            fragments::fragments_for(fragments::MAX_PAYLOAD) <= fragments::MAX_FRAGMENTS as usize,
            "a whole payload must fit in the fragment bound the reassembler allows"
        );
        assert_eq!(fragments::MAX_FRAGMENTS, 2);
    }

    #[test]
    fn the_bounds_constants_are_the_ones_the_rows_expect() {
        // each row of the catalogue whose class has a bound in `bounds` is listed here,
        // so that a renamed class breaks a test rather than a comment
        // the bounds of a server message are applied on the server
        for class in [
            "FarmFieldPlotResize",
            "TransferItemsRequest",
            "TransferItemsToCitizenRequest",
            "PostBoxRequest",
            "HireSpies",
            "TeleportToColony",
            "DirectPlace",
            "EditRankType",
            "AdjustSkillCitizen",
            "MinerRepairLevel",
            "ChangeRecipePriority",
            "UpgradeWarehouse",
            "HireFire",
            "AssignUnassign",
            "GuardSetMinePos",
            "RemovePlayer",
        ] {
            let row = catalogue::of_class(class).unwrap_or_else(|| panic!("{class} has no row"));
            assert_ne!(row.side, Side::Client, "{class} is checked on the server");
        }
        // and the two particle bounds are applied wherever the message lands, which is
        // the client for the eight particle messages
        for class in ["BlockParticleEffect", "StreamParticleEffect"] {
            let row = catalogue::of_class(class).unwrap_or_else(|| panic!("{class} has no row"));
            assert_eq!(row.side, Side::Client, "{class} runs on the client");
        }
        assert_eq!(bounds::MAX_FIELD_RANGE, 20);
        assert_eq!(bounds::SPIES_GOLD_COST, 5);
    }
}
