//! The researches that start by themselves: which ones, when, and what they write.
//!
//! Java reference: `core/colony/managers/ResearchManager.java:141-265`
//! (`checkAutoStartResearch` and `startCostlessResearch`), and the four places that
//! call it (`Colony.java:791` on load, `AbstractBuilding.java:969` on an upgrade,
//! `BuildingUniversity.java:176` on a university upgrade,
//! `IResearchManager.java:64` in the interface).
//!
//! **What is not here, and should not be.** The tree, its costs, its requirements
//! and its records are `crate::research` — `tree`, `cost`, `gate`, `progress` — and
//! this file calls them instead of repeating them: `gate::requirements_met` is the
//! requirement check the autostart loop runs (`:150`), `Requirement::met_with` is
//! how the shipped `sd:sifter:3` lines are read, and `Progress::record` is the
//! record write (`LocalResearchTree.addResearch`, `:229`). What is here is the
//! **manager's** rule: which research gets a chance, in which order, and what
//! happens when it does.
//!
//! | Step | Answer | Java |
//! | --- | --- | --- |
//! | 1 | requirements unmet → **stay in the list**, try again later | `:150-153` |
//! | 2 | university too shallow → **stay in the list** — unless the branch is `UNLOCKABLES` | `:155-173` |
//! | 3 | already running or finished → **leave the list**, silently | `:175-190` |
//! | 4 | it costs something → **announce only**, never start | `:193-200` |
//! | 5 | costless → start it; `instant` or a creative player finishes it at once | `:201-205`, `:218-264` |
//! | 6 | the list is done, the colony is marked dirty **whatever happened** | `:210-211` |
//!
//! **The four shipped autostart researches are the four sifter meshes, and their
//! gate is a sifter, not a university.** `unlockable/stringmesh` wants a sifter at
//! level 1, `flintmesh` 3, `ironmesh` 4, `diamondmesh` 5, and all four carry
//! `h,a,i` — hidden, autostart, instant. So on the shipped data step 2 never fires
//! (the branch is `UNLOCKABLES`), step 4 never fires (they cost nothing), step 5
//! always finishes at once, and the `UNLOCKABLES` exception is *why* a colony with
//! a sifter and no university can unlock a mesh. The cost and the not-instant
//! branches are kept because a data pack can reach them; a test builds a synthetic
//! row for each rather than pretending the shipped data exercises them.
//!
//! **The parent is never consulted.** Java's loop checks the requirements and the
//! depth, not the tree: `unlockable/diamondmesh` names `ironmesh` as its parent and
//! starts whatever state that parent is in, as soon as the sifter is level 5. That
//! is not an oversight in the port — `:175-190` only asks whether *this* research is
//! recorded. [`crate::research::gate::may_begin`] is the rule with the parent, and it
//! is not what this loop calls.
//!
//! **One inconsistency of Java's, kept and named.** Step 2 compares the university's
//! **raw** level with the depth (`:169`), while `GlobalResearch.canResearch`
//! substitutes `Integer.MAX_VALUE` for a university **at its maximum**
//! (`GlobalResearch.java:158`, ported as `research::gate::university_level`). A
//! level-5 university is therefore enough for a depth-6 research through the
//! window, and **never** enough through the autostart loop. A test pins the two
//! answers against each other. On the shipped data it cannot bite — no autostart
//! research is on a `DEFAULT` branch — but a data pack that adds one would see it.
//!
//! **No packet, no sound, no clock.** The three messages are keys
//! ([`Message::keys`]); Java's `sendTo(colony).forAllPlayers()` and
//! `SoundUtils.playSuccessSound` need recipients a wasm mod does not have
//! (`MANQUANT.md` §2), and the `R2` draw of `RESEARCH_CONCLUDED` comes in from the
//! caller like every other draw.

use crate::research::{BranchKind, Buildings, Progress, Research, State, Tree, gate};

/// `TranslationConstants.java:400`.
pub const KEY_AVAILABLE: &str = "com.minecolonies.coremod.research.available";
/// `TranslationConstants.java:670`.
pub const KEY_STARTED: &str = "com.minecolonies.coremod.research.started";
/// `TranslationConstants.java:346` — a **prefix**, followed by the draw.
pub const KEY_CONCLUDED: &str = "com.minecolonies.coremod.university.researchconcluded.";
/// How many `researchconcluded` keys the lang file holds, so
/// `ThreadLocalRandom.current().nextInt(3)` draws from them (`:245`).
pub const CONCLUDED_VARIANTS: u8 = 3;
/// The building the university-level rule counts (`ModBuildings.university`,
/// `:161`).
pub const UNIVERSITY: &str = "university";

/// One message Java would send (`:195`, `:245-248`, `:256-259`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three announcements of this loop
pub enum Message {
    /// `RESEARCH_AVAILABLE` alone: the research has a cost, so the player is told
    /// and nothing is started (`:195`).
    Available,
    /// `RESEARCH_AVAILABLE` **and** `MESSAGE_RESEARCH_STARTED` appended: Java chains
    /// two components into one line (`:256-259`).
    Started,
    /// `RESEARCH_CONCLUDED` plus a variant, so the same news has three wordings
    /// (`:245-246`).
    Concluded { variant: u8 },
}

impl Message {
    /// The keys, in the order Java appends them.
    pub fn keys(&self) -> Vec<String> {
        match self {
            Message::Available => vec![KEY_AVAILABLE.to_owned()],
            Message::Started => vec![KEY_AVAILABLE.to_owned(), KEY_STARTED.to_owned()],
            Message::Concluded { variant } => {
                vec![format!("{KEY_CONCLUDED}{variant}")]
            }
        }
    }
}

/// What the loop decided for one research.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one variant per branch of checkAutoStartResearch
pub enum Outcome {
    /// A requirement is not met (`:150-153`): the research **stays** in the list and
    /// is tried again on the next check — a university upgrade, a building upgrade,
    /// a world load.
    Unmet { what: String },
    /// The university is too shallow (`:169-172`): it **stays** in the list too,
    /// because the check runs again on the university's upgrade (`:154`).
    TooDeep { depth: u8, university: i32 },
    /// A record exists, running or finished (`:175-190`): it **leaves** the list, in
    /// silence — Java does not announce what the player already knows.
    Recorded { finished: bool },
    /// It costs items, so Java only tells the players (`:193-200`) and leaves the
    /// decision to them; the research **leaves** the list.
    OnlyAnnounce,
    /// Costless, so it starts (`:201-205`). `instant` also says whether it finishes
    /// at once, which is `isInstant()` or a creative player with the config on
    /// (`:230`).
    Start { instant: bool },
}

impl Outcome {
    /// Does the research stay in the pending list? Java's `continue` (`:152`,
    /// `:171`) versus `removes.add(...)` (`:188`, `:208`).
    pub fn stays(&self) -> bool {
        matches!(self, Outcome::Unmet { .. } | Outcome::TooDeep { .. })
    }

    /// A stable name for logs and tests.
    pub fn name(&self) -> &'static str {
        match self {
            Outcome::Unmet { .. } => "minecolonies:autostart/unmet",
            Outcome::TooDeep { .. } => "minecolonies:autostart/too-deep",
            Outcome::Recorded { .. } => "minecolonies:autostart/recorded",
            Outcome::OnlyAnnounce => "minecolonies:autostart/announce-only",
            Outcome::Start { .. } => "minecolonies:autostart/start",
        }
    }
}

/// One write of the loop, in the order `ResearchManager.java:218-264` performs it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the writes of startCostlessResearch and its announcement
pub enum Write {
    /// `markDirty()` — the **first** line of `startCostlessResearch` (`:220`).
    MarkDirty,
    /// `tree.addResearch(branch, new LocalResearch(id, branch, depth))` (`:229`).
    AddRecord,
    /// `setProgress(baseTime(depth))` then `setState(FINISHED)` (`:233-234`).
    Complete,
    /// `tree.finishResearch(id)` (`:235`).
    Finish,
    /// One `effects.applyEffect(effect)` per effect of the research (`:236-239`).
    ApplyEffects { count: usize },
    /// `citizen.applyResearchEffects()` for every citizen (`:240-243`).
    RefreshCitizens,
    /// One message to the colony's players.
    Announce(Message),
}

impl Write {
    pub fn name(&self) -> &'static str {
        match self {
            Write::MarkDirty => "mark-dirty",
            Write::AddRecord => "add-record",
            Write::Complete => "complete",
            Write::Finish => "finish",
            Write::ApplyEffects { .. } => "apply-effects",
            Write::RefreshCitizens => "refresh-citizens",
            Write::Announce(_) => "announce",
        }
    }
}

/// Where a step's writes go. The only thing in this module that touches the colony.
pub trait Sink {
    fn write(&mut self, write: &Write);
}

/// What one research's turn wrote, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Plan {
    writes: Vec<Write>,
}

impl Plan {
    fn of(writes: Vec<Write>) -> Plan {
        Plan { writes }
    }

    pub fn writes(&self) -> &[Write] {
        &self.writes
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    pub fn apply(&self, sink: &mut dyn Sink) {
        for write in &self.writes {
            sink.write(write);
        }
    }
}

/// One research's turn: what was decided, and what that decision wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one id, one outcome, one plan
pub struct Step {
    pub id: &'static str,
    pub outcome: Outcome,
    pub plan: Plan,
}

/// The result of one check: every step, and whether the colony must be saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Check {
    pub steps: Vec<Step>,
    /// Java marks the colony dirty after the loop **whatever it did** (`:210-211`),
    /// so this is `true` for a server colony and `false` only for the client half,
    /// where `:143-146` returns before the loop at all.
    pub dirty: bool,
}

impl Check {
    /// The ids that left the list, in the order they were handled.
    pub fn handled(&self) -> Vec<&'static str> {
        self.steps
            .iter()
            .filter(|step| !step.outcome.stays())
            .map(|step| step.id)
            .collect()
    }

    /// Every write of the check, in order — one journal for a whole check.
    pub fn apply(&self, sink: &mut dyn Sink) {
        for step in &self.steps {
            step.plan.apply(sink);
        }
    }
}

/// Java's `autoStartResearch` list (`ResearchManager.java:53`, filled at `:118`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Pending {
    ids: Vec<&'static str>,
}

impl Pending {
    /// Every research the tree flags `autostart`, in file order — Java copies
    /// `getAutostartResearches()`, whose order is the order the data was read.
    pub fn from_tree(tree: &Tree) -> Pending {
        Pending {
            ids: tree
                .all()
                .filter(|research| research.flags.autostarts())
                .map(|research| research.id)
                .collect(),
        }
    }

    pub fn ids(&self) -> &[&'static str] {
        &self.ids
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// One `checkAutoStartResearch` (`:141-212`): every pending research gets its
    /// turn, in order, and the ones that were handled leave the list.
    ///
    /// `variant` is the `nextInt(3)` draw of the conclusion message (`:245`), reduced
    /// modulo [`CONCLUDED_VARIANTS`] because those are the three keys the lang file
    /// holds. `creative_player` and `creative_completion` are the two facts `:230`
    /// reads: a creative player **and** `researchCreativeCompletion`
    /// (`ServerConfiguration.java:145`, default `true`) finish a research at once.
    pub fn check(
        &mut self,
        tree: &Tree,
        progress: &Progress,
        standing: &Buildings,
        creative_player: bool,
        creative_completion: bool,
        variant: u8,
    ) -> Check {
        let variant = variant % CONCLUDED_VARIANTS;
        let ids = self.ids.clone();
        let mut steps = Vec::new();
        for id in ids {
            // A row that is not in the tree cannot be adjudicated; `Tree` refuses
            // duplicates and unknown parents at load, so this is a caller that
            // built the list itself.
            let Some(research) = tree.get(id) else {
                continue;
            };
            let (outcome, plan) = decide_one(
                research,
                progress,
                standing,
                creative_player,
                creative_completion,
                variant,
            );
            steps.push(Step {
                id,
                outcome,
                plan,
            });
        }
        let handled: Vec<&'static str> = steps
            .iter()
            .filter(|step| !step.outcome.stays())
            .map(|step| step.id)
            .collect();
        self.ids.retain(|id| !handled.contains(id));
        Check { steps, dirty: true }
    }
}

/// The six steps of the loop, for one research, in Java's order.
fn decide_one(
    research: &Research,
    progress: &Progress,
    standing: &Buildings,
    creative_player: bool,
    creative_completion: bool,
    variant: u8,
) -> (Outcome, Plan) {
    // 1. `:150-153` — the requirements, through the same reader the gate uses.
    if let Err(refusal) = gate::requirements_met(research, standing, progress) {
        let what = match refusal {
            gate::Refusal::RequirementUnmet { what } => what,
            other => other.name().to_owned(),
        };
        return (Outcome::Unmet { what }, Plan::default());
    }

    // 2. `:155-173` — the university's **raw** level, unless the branch is
    // `UNLOCKABLES` (`:155` is the exception, `:169` the comparison).
    if research.branch.info().kind != BranchKind::Unlockables {
        let university = standing.highest(UNIVERSITY).unwrap_or(0);
        if university < i32::from(research.depth) {
            return (
                Outcome::TooDeep {
                    depth: research.depth,
                    university,
                },
                Plan::default(),
            );
        }
    }

    // 3. `:175-190` — a record exists, running or finished: nothing to do, and
    // nothing to say.
    let state = progress.state(research.id);
    if state != State::NotStarted {
        return (
            Outcome::Recorded {
                finished: state == State::Finished,
            },
            Plan::default(),
        );
    }

    // 4. `:193-200` — a cost is the player's decision, never the loop's.
    if !research.costs.is_empty() {
        return (
            Outcome::OnlyAnnounce,
            Plan::of(vec![Write::Announce(Message::Available)]),
        );
    }

    // 5. `:201-205` and `startCostlessResearch` (`:218-264`).
    let instant = research.flags.instant() || (creative_player && creative_completion);
    let mut writes = vec![Write::MarkDirty, Write::AddRecord];
    if instant {
        writes.push(Write::Complete);
        writes.push(Write::Finish);
        if !research.effects.is_empty() {
            writes.push(Write::ApplyEffects {
                count: research.effects.len(),
            });
        }
        writes.push(Write::RefreshCitizens);
        writes.push(Write::Announce(Message::Concluded { variant }));
    } else {
        writes.push(Write::Announce(Message::Started));
    }
    (Outcome::Start { instant }, Plan::of(writes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::{Cost, Effects};

    /// A journal of what a check actually wrote, in order.
    #[derive(Default)]
    struct Journal(Vec<&'static str>);

    impl Sink for Journal {
        fn write(&mut self, write: &Write) {
            self.0.push(write.name());
        }
    }

    fn tree() -> Tree {
        Tree::load().expect("the shipped table loads")
    }

    fn row(id: &str) -> Research {
        tree().get(id).expect("declared").clone()
    }

    /// The four shipped autostart researches, in file order, and their gates.
    #[test]
    fn the_shipped_autostart_rows_are_the_four_meshes() {
        let pending = Pending::from_tree(&tree());
        assert_eq!(
            pending.ids(),
            &[
                "unlockable/diamondmesh",
                "unlockable/flintmesh",
                "unlockable/ironmesh",
                "unlockable/stringmesh",
            ]
        );
        let tree = tree();
        for id in pending.ids() {
            let research = tree.get(id).expect("declared");
            assert!(research.flags.autostarts() && research.flags.instant());
            assert_eq!(research.branch, crate::research::Branch::Unlockable);
            assert!(research.costs.is_empty(), "they cost nothing");
            assert_eq!(research.requirements.len(), 1, "one sifter requirement");
        }
    }

    /// No sifter, so nothing is met — and all four **stay** pending, with no write
    /// at all (`:150-153`).
    #[test]
    fn without_a_sifter_every_mesh_is_unmet_and_writes_nothing() {
        let mut pending = Pending::from_tree(&tree());
        let before = pending.clone();
        let progress = Progress::new();
        let check = pending.check(&tree(), &progress, &Buildings::new(), false, true, 0);
        assert_eq!(check.steps.len(), 4);
        for step in &check.steps {
            assert!(matches!(step.outcome, Outcome::Unmet { .. }), "{}", step.id);
            assert!(step.plan.is_empty(), "an unmet research writes nothing");
        }
        assert!(check.handled().is_empty());
        assert_eq!(pending, before, "and the list is unchanged");
        assert!(check.dirty, "the colony is marked dirty all the same (`:211`)");
    }

    /// A sifter at level 3 unlocks two of the four at once, instantly, leaving the
    /// other two pending — the shipped behaviour (`:169` skipped for `UNLOCKABLES`,
    /// `:229-248`).
    #[test]
    fn a_sifter_at_three_unlocks_the_first_two_meshes_at_once() {
        let mut pending = Pending::from_tree(&tree());
        let mut standing = Buildings::new();
        standing.add("sifter", 3);
        let mut progress = Progress::new();
        let mut effects = Effects::new();
        let check = pending.check(&tree(), &progress, &standing, false, true, 1);
        assert_eq!(
            check.handled(),
            vec!["unlockable/flintmesh", "unlockable/stringmesh"]
        );
        assert_eq!(pending.ids(), &["unlockable/diamondmesh", "unlockable/ironmesh"]);

        let mut journal = Journal::default();
        check.apply(&mut journal);
        assert_eq!(
            journal.0,
            vec![
                "mark-dirty",
                "add-record",
                "complete",
                "finish",
                "apply-effects",
                "refresh-citizens",
                "announce",
                "mark-dirty",
                "add-record",
                "complete",
                "finish",
                "apply-effects",
                "refresh-citizens",
                "announce",
            ],
            "the whole write set of two instant researches, in order"
        );

        // And the effects are the colony's now: applying the step is what gives
        // them, so a second check finds them recorded and silent.
        for step in &check.steps {
            if matches!(step.outcome, Outcome::Start { .. }) {
                let research = tree().get(step.id).expect("declared").clone();
                progress.record(&research, State::Finished, research.ticks());
                effects.apply_research(&research);
            }
        }
        assert!(
            effects.has("sifterflintunlock") && effects.has("sifterstringunlock"),
            "the two started researches gave their effects"
        );
    }

    /// A research already running or finished leaves the list **silently**
    /// (`:175-190`): it is not announced and nothing is written.
    #[test]
    fn a_recorded_research_leaves_the_list_without_a_word() {
        let mut pending = Pending::from_tree(&tree());
        let mut standing = Buildings::new();
        standing.add("sifter", 5);
        let research = row("unlockable/stringmesh");
        let mut progress = Progress::new();
        progress.record(&research, State::InProgress, 0);

        let check = pending.check(&tree(), &progress, &standing, false, true, 0);
        let recorded: Vec<&Step> = check
            .steps
            .iter()
            .filter(|step| matches!(step.outcome, Outcome::Recorded { .. }))
            .collect();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].id, "unlockable/stringmesh");
        assert_eq!(
            recorded[0].outcome,
            Outcome::Recorded { finished: false }
        );
        assert!(recorded[0].plan.is_empty());
        assert!(!pending.ids().contains(&"unlockable/stringmesh"));

        // Finished counts too, and the outcome says which — on a fresh list, since
        // the check above already took the id out of this one.
        let mut fresh = Pending::from_tree(&tree());
        let mut progress = Progress::new();
        progress.record(&research, State::Finished, research.ticks());
        let check = fresh.check(&tree(), &progress, &standing, false, true, 0);
        assert!(check.steps.iter().any(|step| step.id == "unlockable/stringmesh"
            && step.outcome == Outcome::Recorded { finished: true }));
    }

    /// A research that costs something is only announced, and never started: Java's
    /// own words, "we don't want to have items disappearing from inventories"
    /// (`:192-200`).
    #[test]
    fn a_costly_research_is_announced_and_not_started() {
        let mut research = row("civilian/ambition");
        research.flags = research.flags.with(crate::research::tree::Flags::AUTOSTART);
        assert!(!research.costs.is_empty());
        let mut progress = Progress::new();
        let mut standing = Buildings::new();
        standing.add("university", 5);
        let (outcome, plan) = decide_one(&research, &progress, &standing, false, true, 0);
        assert_eq!(outcome, Outcome::OnlyAnnounce);
        assert_eq!(plan.writes(), &[Write::Announce(Message::Available)]);
        assert_eq!(Message::Available.keys(), vec![KEY_AVAILABLE.to_owned()]);
        // And nothing was recorded, which is the whole point.
        progress.record(&research, State::InProgress, 0);
        assert_ne!(progress.state(research.id), State::NotStarted);
    }

    /// A costless research that is not `instant` starts and **keeps running**: two
    /// writes and the "started" line (`:254-263`).
    #[test]
    fn a_costless_research_that_is_not_instant_only_starts() {
        let mut research = row("civilian/ambition");
        research.costs = Vec::new();
        research.flags = research.flags.with(crate::research::tree::Flags::AUTOSTART);
        let mut standing = Buildings::new();
        standing.add("university", 5);
        let (outcome, plan) = decide_one(&research, &Progress::new(), &standing, false, true, 0);
        assert_eq!(outcome, Outcome::Start { instant: false });
        assert_eq!(
            plan.writes(),
            &[
                Write::MarkDirty,
                Write::AddRecord,
                Write::Announce(Message::Started),
            ]
        );
        assert_eq!(
            Message::Started.keys(),
            vec![KEY_AVAILABLE.to_owned(), KEY_STARTED.to_owned()],
            "Java appends the two components into one line"
        );
    }

    /// A creative player with `researchCreativeCompletion` on finishes a costless
    /// research at once, even when the data does not say `instant` (`:230`).
    #[test]
    fn a_creative_player_completes_a_costless_research_at_once() {
        let mut research = row("civilian/ambition");
        research.costs = Vec::new();
        research.flags = research.flags.with(crate::research::tree::Flags::AUTOSTART);
        let mut standing = Buildings::new();
        standing.add("university", 5);
        let (outcome, plan) = decide_one(&research, &Progress::new(), &standing, true, true, 2);
        assert_eq!(outcome, Outcome::Start { instant: true });
        assert!(plan.writes().contains(&Write::Complete));
        assert!(plan
            .writes()
            .contains(&Write::Announce(Message::Concluded { variant: 2 })));

        // The config off, and the same player no longer finishes anything.
        let (outcome, _) = decide_one(&research, &Progress::new(), &standing, true, false, 2);
        assert_eq!(outcome, Outcome::Start { instant: false });
    }

    /// The conclusion message has three wordings, and the caller's draw is reduced to
    /// them (`:245`).
    #[test]
    fn the_conclusion_message_draws_from_the_three_keys() {
        assert_eq!(
            Message::Concluded { variant: 0 }.keys(),
            vec!["com.minecolonies.coremod.university.researchconcluded.0".to_owned()]
        );
        assert_eq!(
            Message::Concluded { variant: 2 }.keys(),
            vec![format!("{KEY_CONCLUDED}2")]
        );
        assert_eq!(CONCLUDED_VARIANTS, 3);
    }

    /// The university gate is skipped for an `UNLOCKABLES` branch and applied to a
    /// `DEFAULT` one — with the **raw** level, not the `MAX_VALUE` substitution
    /// `canResearch` makes (`GlobalResearch.java:158`).
    #[test]
    fn the_university_gate_skips_unlockables_and_uses_the_raw_level() {
        let mut standing = Buildings::new();
        // No university at all, and an unlockable still passes step 2.
        let mut research = row("unlockable/stringmesh");
        research.requirements = Vec::new();
        research.costs = Vec::new();
        let (outcome, _) = decide_one(&research, &Progress::new(), &standing, false, true, 0);
        assert!(matches!(outcome, Outcome::Start { .. }), "the unlockables skip step 2");

        // The same row moved to a DEFAULT branch needs a university of its depth.
        let mut default = research.clone();
        default.branch = crate::research::Branch::Civilian;
        let (outcome, _) = decide_one(&default, &Progress::new(), &standing, false, true, 0);
        assert_eq!(
            outcome,
            Outcome::TooDeep {
                depth: 1,
                university: 0
            }
        );

        // A depth-6 row on a DEFAULT branch with a level-5 university: the window's
        // rule says yes (`university_level(5)` is `i32::MAX`), the autostart says no.
        let mut deep = row("civilian/academic");
        assert_eq!(deep.depth, 6);
        deep.requirements = Vec::new();
        deep.costs = Vec::new();
        standing.add("university", 5);
        let window = crate::research::progress::university_level(5);
        assert_eq!(window, i32::MAX);
        assert!(
            crate::research::gate::can_display(&deep, window),
            "the window lets a maximum university reach depth 6"
        );
        let (outcome, _) = decide_one(&deep, &Progress::new(), &standing, false, true, 0);
        assert_eq!(
            outcome,
            Outcome::TooDeep {
                depth: 6,
                university: 5
            },
            "the autostart loop compares the raw level"
        );
    }

    /// The parent is never consulted: a child autostarts while its parent has no
    /// record at all (`:175-190` asks only about this research).
    #[test]
    fn the_parent_research_is_not_consulted() {
        let mut research = row("unlockable/diamondmesh");
        research.requirements = Vec::new();
        research.costs = Vec::new();
        assert_eq!(research.parent, Some("unlockable/ironmesh"));
        let (outcome, _) = decide_one(&research, &Progress::new(), &Buildings::new(), false, true, 0);
        assert_eq!(outcome, Outcome::Start { instant: true });
    }

    /// Every write of a check is preceded by its decision: an unmet, too-deep or
    /// recorded step writes **nothing** (`:152`, `:171`, `:186-190`).
    #[test]
    fn a_step_that_writes_nothing_is_a_step_that_decided_not_to() {
        // A depth-1 row, so a level-1 university is enough for it — the depth-6 row
        // is the subject of its own test below.
        let mut research = row("civilian/firstaid");
        assert_eq!(research.depth, 1);
        research.requirements = Vec::new();
        research.costs = Vec::new();
        let standing = Buildings::new();
        let (too_deep, plan) = decide_one(&research, &Progress::new(), &standing, false, true, 0);
        assert_eq!(too_deep, Outcome::TooDeep { depth: 1, university: 0 });
        assert!(plan.is_empty(), "a shallow university writes nothing");

        // Now deep enough, and it starts.
        let mut standing = Buildings::new();
        standing.add("university", 1);
        let mut progress = Progress::new();
        let (start, plan) = decide_one(&research, &progress, &standing, false, true, 0);
        assert!(matches!(start, Outcome::Start { .. }));
        assert!(!plan.is_empty());

        // And once recorded, it writes nothing again.
        progress.record(&research, State::Finished, research.ticks());
        let (recorded, plan) = decide_one(&research, &progress, &standing, false, true, 0);
        assert_eq!(recorded, Outcome::Recorded { finished: true });
        assert!(plan.is_empty(), "a recorded research writes nothing");
    }

    /// A cost entry is what step 4 reads, and the cheapest row proves it: one item
    /// is enough to keep the loop's hands off.
    #[test]
    fn one_cost_entry_is_enough_to_keep_the_loop_off() {
        let mut research = row("civilian/ambition");
        research.costs = vec![Cost::Item {
            item: "minecraft:diamond",
            count: 1,
        }];
        research.requirements = Vec::new();
        let mut standing = Buildings::new();
        standing.add("university", 5);
        let (outcome, plan) = decide_one(&research, &Progress::new(), &standing, false, true, 0);
        assert_eq!(outcome, Outcome::OnlyAnnounce);
        assert_eq!(plan.writes().len(), 1);
        assert!(!plan.writes().contains(&Write::AddRecord));
    }
}
