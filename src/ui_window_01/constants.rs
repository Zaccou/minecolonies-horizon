//! `WindowConstants`: the pane ids these three windows read, with the line that declares each one.
//!
//! Java reference: `api/util/constant/WindowConstants.java`, 1 195 lines and **296**
//! `public static final` fields — 242 `String`, 51 `int`, two `ResourceLocation` and one `Size2i` — of
//! which **twenty-four** matter to this slice. The file is a flat bag of strings shared by
//! every GUI in the mod, and the three windows here read it in three different ways: one imports it
//! wholesale (`WindowAssignCitizen.java:29`), one imports nothing from it
//! (`WindowBannerRallyGuards.java:36-61`), and one is not BlockUI and reads no pane id at all
//! (`WindowBannerPicker.java:49`).
//!
//! **The file has duplicate values, and the duplicates are load-bearing.** Four constants are the
//! string `name`, three are `job`, two are `done`, two are `cancel`. A caller that keys a pane by its
//! value cannot tell the hut title (`LABEL_BUILDING_NAME`, `:559`) from the postbox's input
//! (`INPUT_NAME`, `:927`); a caller that keys it by its **field** can. That is why [`Constant`]
//! carries both and [`names_for`] exists — to make the collision visible instead of guessing.
//!
//! **`assign`'s two lists are not the home window's two lists.** `WindowAssignCitizen` reads
//! `assigned` (`:954`) and `unassigned` (`:949`); the home window's `WindowHutLiving` reads
//! `assignedCitizen` (`:1100`) and the hire window reads `unemployed` (`:994`). Four constants, four
//! different values, two of them one word away from the other pair.

/// One `WindowConstants` field: its Java name, its value, and the line that declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constant {
    /// The Java field name, exactly as written.
    pub field: &'static str,
    /// Its value.
    pub value: &'static str,
    /// The `public static final String` line, 1-based.
    pub line: u32,
}

/// The twenty-four constants this slice reads, **in the file's order**.
///
/// Ordered by line so that a reader can walk the Java file and tick them off; the test
/// [`tests::the_table_is_in_file_order`] holds the invariant.
pub const WINDOW_CONSTANTS: &[Constant] = &[
    // The town hall's banner picker button: `WindowMainPage.java:105` registers it.
    Constant { field: "BUTTON_BANNER_PICKER", value: "bannerPicker", line: 137 },
    Constant { field: "NAME_LABEL", value: "name", line: 272 },
    Constant { field: "JOB_LABEL", value: "job", line: 302 },
    // `WindowAssignCitizen.java:85` — the window's only way out.
    Constant { field: "BUTTON_CANCEL", value: "cancel", line: 477 },
    Constant { field: "BUTTON_CONFIRM", value: "confirm", line: 482 },
    Constant { field: "GUIDE_CONFIRM", value: "confirm", line: 522 },
    Constant { field: "LABEL_BUILDING_NAME", value: "name", line: 559 },
    // Same value as `WindowBannerRallyGuards.BUTTON_REMOVE`, which never names it.
    Constant { field: "BUTTON_REMOVE", value: "remove", line: 584 },
    Constant { field: "REQUEST_CANCEL", value: "cancel", line: 619 },
    Constant { field: "INPUT_NAME", value: "name", line: 927 },
    Constant { field: "BUTTON_DONE", value: "done", line: 934 },
    // `WindowAssignCitizen.java:88` — the left-hand list's assign button.
    Constant { field: "BUTTON_HIRE", value: "hire", line: 939 },
    // `WindowAssignCitizen.java:253` — the citizen's name, in a row of either list.
    Constant { field: "CITIZEN_LABEL", value: "citizen", line: 944 },
    // `WindowAssignCitizen.java:82` — the left-hand list.
    Constant { field: "UNASSIGNED_CITIZEN_LIST", value: "unassigned", line: 949 },
    // `WindowAssignCitizen.java:83` — the right-hand list.
    Constant { field: "ASSIGNED_CITIZEN_LIST", value: "assigned", line: 954 },
    Constant { field: "CITIZEN_DONE", value: "done", line: 959 },
    // `WindowAssignCitizen.java:290` — the composed job/home line of a row.
    Constant { field: "CITIZEN_JOB", value: "job", line: 964 },
    // `WindowAssignCitizen.java:86`, `:93` — the mode button and its label.
    Constant { field: "BUTTON_MODE", value: "mode", line: 979 },
    Constant { field: "BUTTON_JOB", value: "job", line: 984 },
    Constant { field: "CITIZEN_LIST_UNEMP", value: "unemployed", line: 994 },
    // `WindowAssignCitizen.java:89` — the right-hand list's unassign button.
    Constant { field: "BUTTON_FIRE", value: "fire", line: 1014 },
    Constant { field: "BUTTON_ASSIGN", value: "assign", line: 1085 },
    Constant { field: "LABEL_NAME", value: "name", line: 1090 },
    // The home window's resident list — **not** the assign window's.
    Constant { field: "LIST_CITIZEN", value: "assignedCitizen", line: 1100 },
];

impl Constant {
    /// This constant as `(field, value)`, which is what a test comparing two sources wants.
    pub fn pair(self) -> (&'static str, &'static str) {
        (self.field, self.value)
    }
}

/// The value of the constant Java names `field`, or `None`.
pub fn value_of(field: &str) -> Option<&'static str> {
    WINDOW_CONSTANTS
        .iter()
        .find(|constant| constant.field == field)
        .map(|constant| constant.value)
}

/// Every field of this table whose value is `value`, in file order.
///
/// Two or more answers are not an error — they are the trap this table exists to show.
pub fn names_for(value: &str) -> impl Iterator<Item = &'static str> + '_ {
    WINDOW_CONSTANTS
        .iter()
        .filter(move |constant| constant.value == value)
        .map(|constant| constant.field)
}

/// The six panes `WindowAssignCitizen` reads **by constant**, in the order it reads them at
/// construction (`:82-89`) and in its row providers (`:249`, `:253`, `:290`, `:345`, `:349`).
pub const ASSIGN_READS: [&str; 6] = [
    "UNASSIGNED_CITIZEN_LIST",
    "ASSIGNED_CITIZEN_LIST",
    "BUTTON_CANCEL",
    "BUTTON_MODE",
    "BUTTON_HIRE",
    "BUTTON_FIRE",
];

/// The panes of `windowassigncitizen.xml` that carry a **static label** and are never written.
///
/// `buildingassignmentmode` has an id, a `label="$(com.minecolonies.coremod.gui.buildingassignmentmode)"`
/// and that is all: no class in the tree ever calls `findPaneOfTypeByID` with that string (the only
/// occurrence of the literal in the Java tree is in the XML and in the lang file). It is the caption
/// of the `mode` button, not a value from the colony — which is why it is here and not in
/// [`WINDOW_CONSTANTS`].
pub const ASSIGN_STATIC_LABELS: [&str; 1] = ["buildingassignmentmode"];

/// The six ids `WindowBannerRallyGuards` declares for itself, with the line of each.
///
/// None of them is in [`WINDOW_CONSTANTS`]: the class imports
/// `com.minecolonies.api.util.constant.TranslationConstants.*` and `…ItemBannerRallyGuards.*`, never
/// `WindowConstants` (`:25-26`). `remove` therefore exists twice in the tree with the same value.
pub const RALLY_OWN_IDS: [Constant; 6] = [
    Constant { field: "LIST_GUARDTOWERS", value: "guardtowers", line: 36 },
    Constant { field: "ICON_GUARD", value: "guardicon", line: 41 },
    Constant { field: "BUTTON_REMOVE", value: "remove", line: 46 },
    Constant { field: "BUTTON_RALLY", value: "rally", line: 51 },
    Constant { field: "LABEL_GUARDTYPE", value: "guardtype", line: 56 },
    Constant { field: "LABEL_POSITION", value: "position", line: 61 },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_in_file_order() {
        assert_eq!(WINDOW_CONSTANTS.len(), 24);
        for pair in WINDOW_CONSTANTS.windows(2) {
            assert!(
                pair[0].line < pair[1].line,
                "{}:{} comes before {}:{}",
                pair[0].field,
                pair[0].line,
                pair[1].field,
                pair[1].line
            );
        }
        // 1 195 lines, so a line outside this range is a typo, not a constant.
        for constant in WINDOW_CONSTANTS {
            assert!(constant.line >= 1 && constant.line <= 1195, "{}", constant.field);
            assert!(!constant.value.is_empty(), "{} has no value", constant.field);
        }
    }

    #[test]
    fn five_values_are_shared_by_two_or_more_fields() {
        let count = |value: &str| names_for(value).count();
        // `name`: NAME_LABEL:272, LABEL_BUILDING_NAME:559, INPUT_NAME:927, LABEL_NAME:1090.
        assert_eq!(count("name"), 4);
        // `job`: JOB_LABEL:302, CITIZEN_JOB:964, BUTTON_JOB:984.
        assert_eq!(count("job"), 3);
        assert_eq!(count("done"), 2, "BUTTON_DONE:934 and CITIZEN_DONE:959");
        assert_eq!(count("confirm"), 2, "BUTTON_CONFIRM:482 and GUIDE_CONFIRM:522");
        assert_eq!(count("cancel"), 2, "BUTTON_CANCEL:477 and REQUEST_CANCEL:619");
        // A field that is unique answers exactly one name.
        assert_eq!(names_for("bannerPicker").collect::<Vec<_>>(), ["BUTTON_BANNER_PICKER"]);
        assert_eq!(names_for("hire").collect::<Vec<_>>(), ["BUTTON_HIRE"]);
        // The four list ids are four different values, two words apart or not.
        let mut lists = [
            value_of("ASSIGNED_CITIZEN_LIST"),
            value_of("UNASSIGNED_CITIZEN_LIST"),
            value_of("LIST_CITIZEN"),
            value_of("CITIZEN_LIST_UNEMP"),
        ];
        assert!(lists.iter().all(|value| value.is_some()));
        lists.sort_unstable();
        assert_eq!(lists, [Some("assigned"), Some("assignedCitizen"), Some("unassigned"), Some("unemployed")]);
        // Nothing in this file is named `rally`: the rally window's button is its own.
        assert_eq!(names_for("rally").count(), 0);
    }

    #[test]
    fn the_six_panes_the_assign_window_reads_are_all_in_the_table() {
        for field in ASSIGN_READS {
            assert!(value_of(field).is_some(), "{field} is not a WindowConstants field");
        }
        // And they are the panes `src/ui` already declares for the same window: the two lists and
        // the four buttons, plus the two row panes, are one description in two places.
        let assign = crate::ui::screen_by_id(crate::ui::screen::ASSIGN_ID);
        assert!(assign.is_ok(), "src/ui has no assign screen");
        if let Ok(assign) = assign {
            for id in ["assigned", "unassigned", "citizen", "job", "hire", "fire", "mode", "cancel"] {
                let declared = assign.fields.iter().any(|field| field.id == id)
                    || assign.actions.iter().any(|action| action.id == id);
                assert!(declared, "src/ui's assign screen does not declare {id}");
                // Every one of them is a value of this table, except the one static label.
                assert!(
                    value_of_any(id).is_some() || ASSIGN_STATIC_LABELS.contains(&id),
                    "{id} is neither a WindowConstants value nor the XML's static label"
                );
            }
        }
    }

    #[test]
    fn the_rally_window_owns_its_ids_and_the_one_it_shares_is_never_named() {
        assert_eq!(RALLY_OWN_IDS.len(), 6);
        // Exactly **one** of the six field names also exists in `WindowConstants`, and the rally
        // window declares it again instead of importing it (`:46` against `:584`).
        let shared: Vec<&str> = RALLY_OWN_IDS
            .iter()
            .filter(|constant| value_of(constant.field).is_some())
            .map(|constant| constant.field)
            .collect();
        assert_eq!(shared, ["BUTTON_REMOVE"]);
        for own in RALLY_OWN_IDS {
            if own.field != "BUTTON_REMOVE" {
                assert!(value_of(own.field).is_none(), "{} is not a WindowConstants field", own.field);
            }
        }
        // The one value the two files share, reached from two different fields.
        assert_eq!(value_of("BUTTON_REMOVE"), Some("remove"));
        let rally_remove = RALLY_OWN_IDS
            .iter()
            .find(|constant| constant.field == "BUTTON_REMOVE")
            .map(|constant| constant.value);
        assert_eq!(rally_remove, Some("remove"));
        // `pos` exists in WindowConstants but `position` does not: the rally row's id is the
        // window's own (`:61`) and this table does not carry it.
        assert_eq!(names_for("position").count(), 0);
        assert_eq!(names_for("pos").count(), 0);
        assert_eq!(
            RALLY_OWN_IDS
                .iter()
                .find(|constant| constant.field == "LABEL_POSITION")
                .map(|constant| constant.value),
            Some("position")
        );
    }

    /// Every field of the table with this **value**, which is what an XML id is compared against.
    fn value_of_any(id: &str) -> Option<&'static str> {
        names_for(id).next()
    }
}
