//! The nine data listeners: what each one replaces, in which order, and why a
//! refused entry never restores the copy it overwrote.
//!
//! Java reference: `core/datalistener/`, **nine**
//! `SimpleJsonResourceReloadListener`s plus one model record (`model/Disease.java`),
//! 2 231 lines measured with `wc -l`. A listener is not a parser with a cache: it
//! is the object that hears "the data pack reloaded" and re-computes the table
//! that depends on it.
//!
//! | File | Question |
//! | --- | --- |
//! | here | the table of the nine, and the three ways one of them mutates its store |
//! | [`timeline`] | what a `/reload` runs, in which order, and why a recipe table is incomplete when its listener returns |
//! | [`citizen_names`] | `citizennames/*.json` → the style a colony names its citizens with |
//! | [`visitors`] | `visitors/*.json` → which visitor steps out of the tavern door |
//! | [`recruitment`] | `colony/recruitment_items/*.json` → what a recruit costs, **per tavern level** |
//! | [`study_items`] | `study_items/*.json` → what the library desk accepts, and two percentages |
//! | [`nbt_keys`] | `compatibility/*.json` → which NBT keys make two stacks differ |
//! | [`crafter_recipes`] | `crafterrecipes/*.json` → five declared types, four handled |
//! | [`research_plan`] | `researches/*.json` → five phases, and a removal that cascades |
//! | [`trigger`] | `colony/quests/*.json` → the `triggerOrder` expression, checked before it is used |
//!
//! **The finding that organises the module.** The nine listeners do not share one
//! reload discipline, they share three, and which one a listener uses decides
//! what a bad file *costs*:
//!
//! | Discipline | Who | What a refused entry costs |
//! | --- | --- | --- |
//! | [`Discipline::ClearFirst`] | names, visitors, NBT keys, quests | the pack was already wiped: the previous copy is **gone**, the survivors are a partial table |
//! | [`Discipline::ResetThenFill`] | crafter recipes, research | same, at tree level: the reset is part of the reload, not of the success |
//! | [`Discipline::BuildThenSwap`] | diseases, study items, recruitment | the new table is built aside and swapped at the end, so one bad row costs nothing — **unless** the pack aborts, and then the old table survives whole while the rest of the pack is silently dropped |
//!
//! None of the three matches "an invalid datum is refused by its name and the old
//! copy stays" for every case, which is the rule this port states and enforces
//! ([`load`]): a [`Verdict::Refuse`] loses that one row and nothing else, and a
//! [`Verdict::Stop`] — Java's `return` inside the loop — is the case the table
//! above makes expensive, kept because the cost is Java's and a port that hid it
//! would look better than its source.
//!
//! **No listener is per colony, and that answers the fourth question.** Measured:
//! all nine write a `static` table and not one of them touches a colony's saved
//! data (`grep` for a colony write in the nine files returns the quest trigger
//! *read* only). A reload replaces the **specification**; what a colony sees change
//! is what it looks up afterwards, and a colony's own state — a citizen's name
//! once given, a running research's progress, a visitor already spawned with its
//! recruit cost, a request already filed — survives untouched. The one place
//! where the global table answers differently per colony is
//! [`recruitment::band`]: the same nine tiers, a reachable slice that grows with
//! the tavern's level, and that is where Java throws (`:100-101`).

pub mod citizen_names;
pub mod crafter_recipes;
pub mod nbt_keys;
pub mod recruitment;
pub mod research_plan;
pub mod study_items;
pub mod timeline;
pub mod trigger;
pub mod visitors;

/// How a listener replaces the table it owns, measured on the `apply` body.
///
/// Not a style: the variant decides what survives a bad file, and
/// [`load`] implements exactly one behaviour per variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three measured bodies, no fourth in the nine files
pub enum Discipline {
    /// `store.clear()` **before** the loop (`CitizenNameListener.java:40`,
    /// `CustomVisitorListener.java:87`, `ItemNbtListener.java:47`,
    /// `QuestJsonListener.java:101`).
    ///
    /// A refused entry costs the previous copy of the file it came from: it was
    /// already erased. This is the most expensive of the three and four listeners
    /// use it.
    ClearFirst,
    /// A reset call that is part of the reload (`CustomRecipeManager.reset()` at
    /// `CrafterRecipeListener.java:48`, `researchTree.reset()` at
    /// `ResearchListener.java:568`).
    ///
    /// Mechanically the same cost as [`Discipline::ClearFirst`] on the store; the
    /// difference is that these two also own **derived** structures — the
    /// crafter-keyed index and the output index, the tree's branches and child
    /// links — and the reset clears those too (`CustomRecipeManager.java:138-146`).
    ResetThenFill,
    /// A local table, built row by row, assigned at the end
    /// (`DiseasesListener.java:180`, `StudyItemListener.java:148`,
    /// `RecruitmentItemsListener.java:169`).
    ///
    /// A refused row costs nothing. An **abort** costs the whole pack: Java's
    /// `return` leaves the loop and skips the assignment, so the previous table
    /// survives while every file after the bad one is dropped in silence.
    BuildThenSwap,
}

impl Discipline {
    /// A stable name, for a test and for a report line.
    pub fn name(self) -> &'static str {
        match self {
            Discipline::ClearFirst => "clear_first",
            Discipline::ResetThenFill => "reset_then_fill",
            Discipline::BuildThenSwap => "build_then_swap",
        }
    }

    /// Whether a bad row can still be walked back when this listener returns.
    ///
    /// True for one variant only, and that is the whole reason the distinction
    /// exists.
    pub fn swap_is_atomic(self) -> bool {
        self == Discipline::BuildThenSwap
    }
}

/// How the client learns the table.
///
/// Three different contracts, all measured, and the difference matters: the
/// client can re-refuse what it re-parses, but cannot re-refuse what it is handed
/// whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one of each shape exists in the nine files
pub enum Sync {
    /// Nothing is sent: the table is server-only because only the server decides
    /// with it (study items, recruitment costs).
    None,
    /// The client is sent the **keys** and keeps its own copy
    /// (`ColonyView.java:316-317` writes `nameFileMap.size()` and its key set;
    /// `CompatibilityManager.java:210-213` writes one entry per item).
    TableScan,
    /// The client is sent the parsed rows and rebuilds a table it cannot check
    /// (`DiseasesListener.java:60-75` sends id, name, rarity and cures;
    /// `:94-125` rebuilds without validating).
    ParsedRows,
    /// The client is sent the **raw JSON** and runs the listener again, refusals
    /// included (`QuestJsonListener.java:57-67` sends the bytes, `:73-82` calls
    /// `apply` on what it read).
    ReParsedRawJson,
}

/// One row of [`LISTENERS`]: a listener, its directory, its store, and its cost.
#[derive(Clone, Copy, Debug)]
// exhaustive: nine rows, one per Java file, and no field is optional
pub struct Listener {
    /// The Java class, without its package.
    pub class: &'static str,
    /// The data-pack directory the constructor passes to
    /// `SimpleJsonResourceReloadListener` — the string is the whole registration.
    pub directory: &'static str,
    /// How it replaces its store.
    pub discipline: Discipline,
    /// The field it owns, as Java names it.
    pub store: &'static str,
    /// Who reads the store back, and where — the only reason the listener exists.
    pub feeds: &'static str,
    /// How the client gets it.
    pub sync: Sync,
    /// `Fichier.java:ligne` of the `apply` and of the clear/assign.
    pub source: &'static str,
}

/// The nine listeners, in the order `core/datalistener/` lists them.
///
/// The counts are the ones in the data, not in the code: 22 citizen-name files,
/// 29 visitor files, 3 diseases, 2 study items, 1 compatibility file, 2 231 lines
/// of Java for the nine. The recipe and research counts belong to their own
/// modules (`crafting::crafter`, `research::tree`) and are named there.
pub const LISTENERS: &[Listener] = &[
    Listener {
        class: "CitizenNameListener",
        directory: "citizennames",
        discipline: Discipline::ClearFirst,
        store: "nameFileMap",
        feeds: "Colony.java:1949, by the colony's nameStyle",
        sync: Sync::TableScan,
        source: "CitizenNameListener.java:38-50",
    },
    Listener {
        class: "CrafterRecipeListener",
        directory: "crafterrecipes",
        discipline: Discipline::ResetThenFill,
        store: "CustomRecipeManager.getInstance()",
        feeds: "AbstractCraftingBuildingModule.java:296, per crafter key",
        sync: Sync::ParsedRows,
        source: "CrafterRecipeListener.java:33-49",
    },
    Listener {
        class: "CustomVisitorListener",
        directory: "visitors",
        discipline: Discipline::ClearFirst,
        store: "visitorDataPack",
        feeds: "TavernBuildingModule.java:182, chanceCustomVisitors",
        sync: Sync::None,
        source: "CustomVisitorListener.java:83-90",
    },
    Listener {
        class: "DiseasesListener",
        directory: "colony/diseases",
        discipline: Discipline::BuildThenSwap,
        store: "DISEASES",
        feeds: "CitizenDiseaseHandler.java:112, at the roll",
        sync: Sync::ParsedRows,
        source: "DiseasesListener.java:50, :137-181",
    },
    Listener {
        class: "ItemNbtListener",
        directory: "compatibility",
        discipline: Discipline::ClearFirst,
        store: "ItemStackUtils.CHECKED_NBT_KEYS",
        feeds: "ItemStackUtils.java:580, item comparison in every request",
        sync: Sync::TableScan,
        source: "ItemNbtListener.java:38-50",
    },
    Listener {
        class: "QuestJsonListener",
        directory: "colony/quests",
        discipline: Discipline::ClearFirst,
        store: "IQuestManager.GLOBAL_SERVER_QUESTS",
        feeds: "QuestObjectiveEventHandler.java:76, every quest step",
        sync: Sync::ReParsedRawJson,
        source: "QuestJsonListener.java:84-101",
    },
    Listener {
        class: "RecruitmentItemsListener",
        directory: "colony/recruitment_items",
        discipline: Discipline::BuildThenSwap,
        store: "RECRUIT_COSTS",
        feeds: "TavernBuildingModule.java:196, per tavern level",
        sync: Sync::None,
        source: "RecruitmentItemsListener.java:117-170",
    },
    Listener {
        class: "ResearchListener",
        directory: "researches",
        discipline: Discipline::ResetThenFill,
        store: "IGlobalResearchTree",
        feeds: "ResearchManager.java:38, and the university tree",
        sync: Sync::ParsedRows,
        source: "ResearchListener.java:155-186",
    },
    Listener {
        class: "StudyItemListener",
        directory: "study_items",
        discipline: Discipline::BuildThenSwap,
        store: "ACTIVE_LIST",
        feeds: "BuildingLibrary.java:53, EntityAIStudy.java:124",
        sync: Sync::None,
        source: "StudyItemListener.java:60, :130-149",
    },
];

/// The listener whose Java class is `class`, or `None` for anything else — the
/// table is a closed list, not a pattern.
pub fn listener(class: &str) -> Option<&'static Listener> {
    LISTENERS.iter().find(|entry| entry.class == class)
}

/// Something that refused one row, and can say which one and why.
///
/// Each listener has its own refused set — a rarity out of range is not a gender
/// that is not `m` — and this is the only thing [`load`] needs from them.
pub trait Refused {
    /// The namespaced name of the refusal, id included:
    /// `data_listeners.refusal.rarity_out_of_range(tavern/bread/12)`.
    fn name(&self) -> String;
}

/// What a listener does with one file of the pack.
///
/// Java writes these three with `continue`, `continue` and `return`; the port
/// gives the third its own variant because the two are not the same cost and
/// Java's own comments never say so.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three outcomes, matching the three statements in the Java loops
pub enum Verdict<E, R> {
    /// The row parsed: the store takes it, replacing any row of the same id
    /// (`CitizenNameListener.java:82` is a `Map.put`).
    Keep(E),
    /// The row is invalid: it is dropped **by name** and the loop carries on
    /// (`CitizenNameListener.java:84-87`, `:65-68` of `StudyItemListener`).
    Refuse(R),
    /// Java's `return` inside `apply` (`DiseasesListener.java:145`,
    /// `StudyItemListener.java:141`, `RecruitmentItemsListener.java:137`): the
    /// loop is abandoned, and whether the old table survives depends on the
    /// discipline.
    Stop(R),
}

/// What one `apply` did to the store.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: a report is what `load` returns, nothing else reads it
pub struct Report<R> {
    /// The discipline the listener uses.
    pub discipline: Discipline,
    /// The ids the store now holds, in pack order.
    pub accepted: Vec<&'static str>,
    /// The rows dropped, each with its own name.
    pub refused: Vec<R>,
    /// The row that abandoned the pack, if one did.
    pub stopped: Option<R>,
    /// Rows in the store before the reload.
    pub before: usize,
    /// Rows in the store after it.
    pub after: usize,
}

impl<R: Refused> Report<R> {
    /// The names of every refusal, refusals and stop together, in order.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.refused.iter().map(Refused::name).collect();
        if let Some(stop) = &self.stopped {
            names.push(stop.name());
        }
        names
    }

    /// Whether the reload was fully accepted: no refusal and no abort.
    pub fn complete(&self) -> bool {
        self.refused.is_empty() && self.stopped.is_none()
    }

    /// Whether the table is the one it was before the reload.
    ///
    /// Only possible under [`Discipline::BuildThenSwap`] and only after an abort:
    /// the one path where Java loses nothing.
    pub fn unchanged(&self) -> bool {
        self.after == self.before && self.accepted.len() == self.before
    }
}

/// Applies one pack to one store, with the discipline of the listener that owns it.
///
/// The three behaviours, each the one Java has:
///
/// - [`Discipline::ClearFirst`] and [`Discipline::ResetThenFill`] wipe the store
///   before reading a single row. A refusal therefore costs what the old copy
///   held, and a [`Verdict::Stop`] leaves the store **partial** — the rows before
///   it, none after.
/// - [`Discipline::BuildThenSwap`] fills a fresh table and swaps it at the end.
///   A refusal costs nothing; a [`Verdict::Stop`] abandons the fresh table, so the
///   **old store survives whole** and every file after the bad one is dropped
///   without a line of log (`RecruitmentItemsListener.java:134-138`: an empty pack
///   keeps the previous one, by the same `return`).
///
/// Rows are stored in pack order and a repeated id replaces the earlier row in
/// place, which is `HashMap.put` in `CitizenNameListener.java:82`.
pub fn load<E, R: Refused>(
    discipline: Discipline,
    pack: Vec<(&'static str, Verdict<E, R>)>,
    store: &mut Vec<(&'static str, E)>,
) -> Report<R> {
    let before = store.len();
    let mut accepted: Vec<&'static str> = Vec::with_capacity(pack.len());
    let mut refused: Vec<R> = Vec::new();
    let mut stopped: Option<R> = None;

    match discipline {
        Discipline::ClearFirst | Discipline::ResetThenFill => {
            store.clear();
            for (id, verdict) in pack {
                match verdict {
                    Verdict::Keep(row) => {
                        put(store, &mut accepted, id, row);
                    }
                    Verdict::Refuse(why) => refused.push(why),
                    Verdict::Stop(why) => {
                        stopped = Some(why);
                        break;
                    }
                }
            }
        }
        Discipline::BuildThenSwap => {
            let mut fresh: Vec<(&'static str, E)> = Vec::with_capacity(pack.len());
            let mut fresh_ids: Vec<&'static str> = Vec::with_capacity(pack.len());
            for (id, verdict) in pack {
                match verdict {
                    Verdict::Keep(row) => {
                        put(&mut fresh, &mut fresh_ids, id, row);
                    }
                    Verdict::Refuse(why) => refused.push(why),
                    Verdict::Stop(why) => {
                        stopped = Some(why);
                        break;
                    }
                }
            }
            // Java assigns after the loop, so an abandoned loop assigns nothing
            // (`DiseasesListener.java:180` is the only write to `DISEASES`).
            if stopped.is_none() {
                *store = fresh;
                accepted = fresh_ids;
            } else {
                accepted.clear();
            }
        }
    }

    Report {
        discipline,
        accepted,
        refused,
        stopped,
        before,
        after: store.len(),
    }
}

fn put<E>(store: &mut Vec<(&'static str, E)>, accepted: &mut Vec<&'static str>, id: &'static str, row: E) {
    match store.iter_mut().find(|(existing, _)| *existing == id) {
        Some(slot) => slot.1 = row,
        None => {
            store.push((id, row));
            accepted.push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refusal whose name repeats its id, so a test can read it back.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Why(&'static str);

    impl Refused for Why {
        fn name(&self) -> String {
            format!("data_listeners.refusal.test({})", self.0)
        }
    }

    fn store() -> Vec<(&'static str, i32)> {
        vec![("a", 1), ("b", 2)]
    }

    #[test]
    fn the_table_holds_nine_listeners_and_their_disciplines() {
        assert_eq!(LISTENERS.len(), 9);
        let count = |d: Discipline| LISTENERS.iter().filter(|l| l.discipline == d).count();
        // Measured, not guessed: four clear, two reset, three swap.
        assert_eq!(count(Discipline::ClearFirst), 4);
        assert_eq!(count(Discipline::ResetThenFill), 2);
        assert_eq!(count(Discipline::BuildThenSwap), 3);
    }

    #[test]
    fn every_listener_names_its_directory_and_the_line_it_comes_from() {
        for entry in LISTENERS {
            assert!(!entry.directory.is_empty(), "{}", entry.class);
            assert!(entry.source.contains(".java:"), "{}", entry.class);
            // `feeds` names the reader **and** its line: `Colony.java:1949` is what
            // makes a table of nine rows checkable against the tree.
            assert!(entry.feeds.contains(".java:"), "{}", entry.class);
            assert!(!entry.store.is_empty(), "{}", entry.class);
        }
        assert_eq!(listener("DiseasesListener").map(|l| l.directory), Some("colony/diseases"));
        assert_eq!(listener("Nope").map(|l| l.directory), None);
    }

    #[test]
    fn clear_first_loses_the_previous_copy_of_a_refused_entry() {
        let mut s = store();
        let pack = vec![
            ("a", Verdict::<i32, Why>::Keep(10)),
            ("b", Verdict::Refuse(Why("b"))),
        ];
        let report = load(Discipline::ClearFirst, pack, &mut s);
        // `a` is the new row, `b` is gone: the clear came first.
        assert_eq!(s, vec![("a", 10)]);
        assert_eq!(report.names(), vec!["data_listeners.refusal.test(b)".to_string()]);
        assert!(!report.unchanged());
        assert_eq!((report.before, report.after), (2, 1));
    }

    #[test]
    fn build_then_swap_keeps_the_whole_previous_table_when_the_pack_aborts() {
        let mut s = store();
        let pack = vec![
            ("a", Verdict::<i32, Why>::Keep(10)),
            ("c", Verdict::Stop(Why("c"))),
            ("d", Verdict::Keep(40)),
        ];
        let report = load(Discipline::BuildThenSwap, pack, &mut s);
        // The fresh table is abandoned with `a` in it; `d` never parsed.
        assert_eq!(s, store());
        assert_eq!(report.stopped, Some(Why("c")));
        assert!(report.accepted.is_empty());
        assert_eq!((report.before, report.after), (2, 2));
    }

    #[test]
    fn clear_first_leaves_a_partial_table_when_the_pack_aborts() {
        let mut s = store();
        let pack = vec![
            ("a", Verdict::<i32, Why>::Keep(10)),
            ("c", Verdict::Stop(Why("c"))),
            ("d", Verdict::Keep(40)),
        ];
        let report = load(Discipline::ClearFirst, pack, &mut s);
        // `a` landed before the return, `d` never did, and the old `b` is gone.
        assert_eq!(s, vec![("a", 10)]);
        assert_eq!(report.stopped, Some(Why("c")));
        assert!(!report.complete());
    }

    #[test]
    fn a_refused_row_does_not_stop_the_ones_after_it() {
        let mut s = store();
        let pack = vec![
            ("a", Verdict::<i32, Why>::Refuse(Why("a"))),
            ("b", Verdict::Keep(20)),
            ("c", Verdict::Keep(30)),
        ];
        let report = load(Discipline::BuildThenSwap, pack, &mut s);
        assert_eq!(s, vec![("b", 20), ("c", 30)]);
        assert!(report.stopped.is_none());
        assert_eq!(report.refused.len(), 1);
    }

    #[test]
    fn the_last_row_for_one_id_wins_and_is_listed_once() {
        let mut s: Vec<(&'static str, i32)> = Vec::new();
        let pack = vec![
            ("a", Verdict::<i32, Why>::Keep(1)),
            ("a", Verdict::Keep(2)),
        ];
        let report = load(Discipline::BuildThenSwap, pack, &mut s);
        assert_eq!(s, vec![("a", 2)]);
        assert_eq!(report.accepted, vec!["a"]);
    }

    #[test]
    fn only_the_swapping_discipline_can_walk_a_reload_back() {
        assert!(Discipline::BuildThenSwap.swap_is_atomic());
        assert!(!Discipline::ClearFirst.swap_is_atomic());
        assert!(!Discipline::ResetThenFill.swap_is_atomic());
        assert_eq!(Discipline::ResetThenFill.name(), "reset_then_fill");
    }
}
