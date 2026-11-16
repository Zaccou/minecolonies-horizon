//! What a colony has researched, what it is researching, and how it reads back.
//!
//! Java reference: `core/research/LocalResearchTree.java` (428 lines, the
//! colony's record), `LocalResearch.java` (116, one record), and the two
//! serialisers `LocalResearchFactory.java:42-70` and
//! `LocalResearchTree.java:344-357`.
//!
//! **A record is four numbers and two names.** `LocalResearchFactory.serialize`
//! writes `state` (the enum's ordinal), `id`, `branch`, `progress` and `depth`
//! (`:49-53`); the records are a list under `researchTree`
//! (`LocalResearchTree.java:344-357`, `TAG_RESEARCH_TREE` in
//! `ResearchConstants.java:21`). [`Progress`] keeps exactly that, and
//! [`Progress::to_compound`] writes it back tag for tag so an existing save reads
//! and a written one could be read by Java.
//!
//! **Three sets, which Java keeps as three.** `researchTree` (branch → id →
//! record), `inProgress` and `isComplete` are three views of the same records
//! (`:24-40`); here they are one map and three queries, because the invariants
//! Java maintains by hand — a record is in exactly one of the two sets — become
//! facts of the type.
//!
//! **Keyed by id, where Java nests branch → id.** `researchTree.get(branch).get(id)`
//! (`:64-73`) is two lookups because a data pack may ship the same id on two
//! branches; [`super::tree::Tree::load`] refuses a duplicate id outright, and
//! `parentResearch` names a research without its branch, so one key answers both
//! readings. The branch is still stored per record, because the save holds it
//! (`LocalResearchFactory.java:51`) and a record may name a branch this port has
//! no enum for.
//!
//! **Progress is counted in study actions, not ticks.** [`Progress::advance`]
//! adds 1 and compares with [`super::Branch::ticks`] (`LocalResearch.java:56-75`,
//! `progress++` at `:60`); Java's worker calls it once per pass
//! (`EntityAIWorkResearcher.java:84-99`). An `instant` research starts with its
//! progress already at the top and finishes on its first advance — that is
//! `GlobalResearch.startResearch`'s own behaviour (`:227-241`: it sets the
//! progress, and `research()` still has to run), not a shortcut.
//!
//! **Effects keep the strongest, never the last.** `ResearchEffectManager.applyEffect`
//! only replaces an effect whose absolute strength is *lower*
//! (`:32-45`, `GlobalResearchEffect.overrides` at `:130-133`), so finishing
//! `levelingmultiplier` level 2 after level 5 leaves 1.0 in place. [`Effects`]
//! keeps that comparison, and rebuilding from the finished list gives the same
//! answer as applying them in order — which is what `resetEffects`
//! (`LocalResearchTree.java:318-341`) relies on.
//!
//! **No clock, no random, no colony.** Java's researcher picks a random
//! in-progress research each pass (`EntityAIWorkResearcher.java:86`) and consults
//! the university's inventory; [`Progress::advance`] takes the id and
//! [`Progress::begin`] takes the two inventories, so every rule here is testable
//! with no game.

use std::collections::BTreeMap;

use super::branch::Branch;
use super::cost::Buildings;
use super::effect;
use super::gate::{self, Refusal};
use super::nbt;
use super::tree::{Research, Tree};
use super::{MAX_DEPTH, MAX_UNIVERSITY_LEVEL};
use crate::inventory::Inventory;
use mod_api::{Compound, Tag};

/// `ResearchState` (`api/research/util/ResearchState.java:7-12`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three states the enum declares
pub enum State {
    NotStarted,
    InProgress,
    Finished,
}

impl State {
    /// Java's ordinal, which is what the save file holds
    /// (`LocalResearchFactory.java:49`, `:62`).
    pub fn ordinal(self) -> i32 {
        match self {
            State::NotStarted => 0,
            State::InProgress => 1,
            State::Finished => 2,
        }
    }

    /// The state of an ordinal, or `None` for a number no `ResearchState` holds.
    /// Java indexes `ResearchState.values()[state]` and throws on a bad one
    /// (`LocalResearchFactory.java:66`); [`Progress::from_compound`] drops the
    /// record instead.
    pub fn from_ordinal(ordinal: i32) -> Option<State> {
        match ordinal {
            0 => Some(State::NotStarted),
            1 => Some(State::InProgress),
            2 => Some(State::Finished),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            State::NotStarted => "not-started",
            State::InProgress => "in-progress",
            State::Finished => "finished",
        }
    }
}

/// One colony's record of one research. `LocalResearch`'s five fields, in the
/// order its serialiser writes them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Local {
    /// `branch`, as the id's path — a `String`, because a save may name a branch
    /// this port has no enum for and Java keeps such a record (`:359-410`).
    branch: String,
    /// `depth`, as recorded when the research started.
    depth: u8,
    state: State,
    /// `progress`, in study actions.
    progress: u32,
}

/// Everything a colony has researched, plus the effect strengths that follow.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    records: BTreeMap<String, Local>,
    /// How many records the last read refused, so a caller can tell "the colony
    /// researched nothing" from "the save did not read".
    dropped: u32,
}

/// The compound key the records live under, `TAG_RESEARCH_TREE`
/// (`api/research/util/ResearchConstants.java:21`).
pub const KEY_TREE: &str = "researchTree";
/// The five record keys, `ResearchConstants.java:28-40` and `:48`.
pub const KEY_STATE: &str = "state";
pub const KEY_ID: &str = "id";
pub const KEY_BRANCH: &str = "branch";
pub const KEY_PROGRESS: &str = "progress";
pub const KEY_DEPTH: &str = "depth";

impl Progress {
    pub fn new() -> Progress {
        Progress::default()
    }

    /// The state of one research; [`State::NotStarted`] when there is no record.
    pub fn state(&self, id: &str) -> State {
        self.records
            .get(id)
            .map(|local| local.state)
            .unwrap_or(State::NotStarted)
    }

    pub fn is_finished(&self, id: &str) -> bool {
        self.state(id) == State::Finished
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.state(id) == State::InProgress
    }

    /// How far along one research is, in study actions. `0` when there is no
    /// record.
    pub fn progress_of(&self, id: &str) -> u32 {
        self.records.get(id).map(|local| local.progress).unwrap_or(0)
    }

    /// How many records the colony holds, of any state.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// A colony that has researched nothing — which is not a save that did not
    /// read; see [`Self::dropped`].
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// How many records a read refused, for a caller that wants to log it. Java
    /// logs one line per unknown research when `researchDebugLog` is on
    /// (`LocalResearchTree.java:382-390`).
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Every finished research, sorted by id.
    pub fn finished(&self) -> impl Iterator<Item = &str> {
        self.records
            .iter()
            .filter(|(_, local)| local.state == State::Finished)
            .map(|(id, _)| id.as_str())
    }

    /// Every research in progress, sorted by id. Java's
    /// `getResearchInProgress` (`:121-126`).
    pub fn running(&self) -> impl Iterator<Item = &str> {
        self.records
            .iter()
            .filter(|(_, local)| local.state == State::InProgress)
            .map(|(id, _)| id.as_str())
    }

    /// Every record the tree does not know — a save from a data pack that has
    /// since changed. Java keeps them and warns (`:359-410`).
    pub fn unknown<'a>(&'a self, tree: &Tree) -> Vec<&'a str> {
        self.records
            .keys()
            .filter(|id| tree.get(id).is_none())
            .map(String::as_str)
            .collect()
    }

    /// The first direct child of `id` that has a record, whatever its state —
    /// `hasResearchedChild` (`GlobalResearch.java:325-341`) and the undo rule
    /// (`WindowResearchTree.java:235-243`) ask the same question.
    pub fn recorded_child(&self, tree: &Tree, id: &str) -> Option<&'static str> {
        tree.children(id)
            .find(|child| self.records.contains_key(child.id))
            .map(|child| child.id)
    }

    /// The depth-6 record a branch already holds, if any. Named for
    /// `branchFinishedHighestLevel` (`LocalResearchTree.java:119-123`), which
    /// `canResearch` calls — but Java's set is filled by *adding* a depth-6
    /// record (`:113-116`), so a running research closes the branch too.
    pub fn branch_ceiling(&self, branch: Branch) -> Option<&str> {
        self.records
            .iter()
            .find(|(_, local)| local.depth >= MAX_DEPTH && local.branch == branch.path())
            .map(|(id, _)| id.as_str())
    }

    /// Writes a record directly, the way `LocalResearchTree.addResearch` does
    /// (`:88-117`).
    ///
    /// [`Self::begin`] is the path a game takes — it refuses, it charges. This
    /// one is for a save being read back and for a caller that has already run
    /// the rules; it does not check that the research exists.
    pub fn record(&mut self, research: &Research, state: State, progress: u32) {
        self.records.insert(
            research.id.to_string(),
            Local {
                branch: research.branch.path().to_string(),
                depth: research.depth,
                state,
                progress,
            },
        );
    }

    /// Runs every rule of `attemptBeginResearch` and, if they hold, takes the
    /// cost and writes the record.
    ///
    /// The order is Java's (`LocalResearchTree.java:147-210`): the tree first,
    /// then the cost, then the requirements — which is why an affordable research
    /// with a missing building answers [`Refusal::RequirementUnmet`] and not the
    /// other way round.
    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        &mut self,
        tree: &Tree,
        research: &Research,
        university_level: i32,
        standing: &Buildings,
        university: &mut Inventory<'_>,
        player: &mut Inventory<'_>,
    ) -> Result<(), Refusal> {
        gate::may_start(
            tree,
            research,
            university_level,
            self,
            standing,
            university,
            player,
        )?;
        gate::pay(research, university, player)?;
        let progress = if research.flags.instant() {
            research.ticks()
        } else {
            0
        };
        self.record(research, State::InProgress, progress);
        Ok(())
    }

    /// One study action on one research. `Ok(true)` when that action finished
    /// it — `LocalResearch.research`'s own return value (`:56-75`).
    ///
    /// The caller chooses which research the action applies to; Java draws one
    /// at random from the ones in progress (`EntityAIWorkResearcher.java:86`),
    /// and a port with no RNG does not pretend to.
    pub fn advance(
        &mut self,
        tree: &Tree,
        id: &str,
        effects: &mut Effects,
    ) -> Result<bool, Refusal> {
        let research = tree.get(id).ok_or_else(|| Refusal::UnknownResearch {
            id: id.to_string(),
        })?;
        let Some(local) = self.records.get_mut(id) else {
            return Err(Refusal::NotRunning {
                id: id.to_string(),
            });
        };
        if local.state != State::InProgress {
            return Err(Refusal::NotRunning {
                id: id.to_string(),
            });
        }
        local.progress = local.progress.saturating_add(1);
        if local.progress >= research.ticks() {
            local.state = State::Finished;
            effects.apply_research(research);
            return Ok(true);
        }
        Ok(false)
    }

    /// Stops a research that is running, dropping its record —
    /// `attemptResetResearch`'s first branch (`:226-236`). The cost paid is
    /// **not** returned: Java does not give it back either.
    pub fn cancel(&mut self, id: &str) -> Result<(), Refusal> {
        if !self.is_running(id) {
            return Err(Refusal::NotRunning {
                id: id.to_string(),
            });
        }
        self.records.remove(id);
        Ok(())
    }

    /// Undoes a finished research, then recomputes every effect from what is
    /// left (`:225-320`, `resetEffects` at `:318-341`).
    pub fn undo(&mut self, tree: &Tree, id: &str, effects: &mut Effects) -> Result<(), Refusal> {
        let research = tree.get(id).ok_or_else(|| Refusal::UnknownResearch {
            id: id.to_string(),
        })?;
        if !self.is_finished(id) {
            return Err(Refusal::NotFinished {
                id: id.to_string(),
            });
        }
        gate::may_undo(tree, research, self)?;
        self.records.remove(id);
        effects.rebuild(tree, self);
        Ok(())
    }

    /// Every effect of every finished research, applied again from scratch.
    /// `resetEffects` does exactly this rather than unwinding one step
    /// (`:320-325`: "There's no guarantee that undoing a research will only push
    /// its effects back one strength grade").
    pub fn rebuild_effects(&self, tree: &Tree) -> Effects {
        let mut effects = Effects::new();
        effects.rebuild(tree, self);
        effects
    }

    /// The records as a compound: `{researchTree: [{state, id, branch, depth,
    /// progress}, …]}`, tag for tag `LocalResearchFactory.serialize`'s layout.
    pub fn to_compound(&self) -> Compound {
        let mut records = Vec::with_capacity(self.records.len());
        for (id, local) in &self.records {
            records.push(Compound::from([
                (KEY_STATE.into(), Tag::Int(local.state.ordinal())),
                (KEY_ID.into(), Tag::String(id.as_str().into())),
                (KEY_BRANCH.into(), Tag::String(local.branch.as_str().into())),
                (KEY_PROGRESS.into(), Tag::Int(local.progress.min(i32::MAX as u32) as i32)),
                (KEY_DEPTH.into(), Tag::Int(i32::from(local.depth))),
            ]));
        }
        Compound::from([(KEY_TREE.into(), nbt::compound_list(records))])
    }

    /// Reads the records back. A record without an id, with a state no
    /// `ResearchState` holds, or otherwise unreadable is **dropped and counted**
    /// ([`Self::dropped`]): the save is wrong, not the tree, and one bad record
    /// must not cost a colony every other research. Java throws on the state
    /// ordinal (`LocalResearchFactory.java:66`) and only warns on a research it
    /// no longer knows (`LocalResearchTree.java:382-390`).
    pub fn from_compound(data: &Compound) -> Progress {
        let mut progress = Progress::new();
        for record in nbt::compounds(data, KEY_TREE) {
            let (Some(id), Some(state), Some(branch)) = (
                nbt::text(record, KEY_ID),
                nbt::int(record, KEY_STATE).and_then(State::from_ordinal),
                nbt::text(record, KEY_BRANCH),
            ) else {
                progress.dropped += 1;
                continue;
            };
            let depth = nbt::int(record, KEY_DEPTH).unwrap_or(1).clamp(0, i32::from(MAX_DEPTH)) as u8;
            let progress_count = nbt::int(record, KEY_PROGRESS).unwrap_or(0).max(0) as u32;
            // A `NOT_STARTED` record is not a record: `addResearch` never writes
            // one (`LocalResearchTree.java:96-116`), and a hand-edited save that
            // does gets the same treatment as no entry at all.
            if state == State::NotStarted {
                progress.dropped += 1;
                continue;
            }
            progress.records.insert(
                id,
                Local {
                    branch,
                    depth,
                    state,
                    progress: progress_count,
                },
            );
        }
        progress
    }
}

/// The colony's effect strengths, keyed by effect id.
/// `ResearchEffectManager` (`core/research/ResearchEffectManager.java`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    strengths: BTreeMap<String, f64>,
}

impl Effects {
    pub fn new() -> Effects {
        Effects::default()
    }

    /// The strength of one effect, `0.0` when the colony has none — Java's
    /// `getEffectStrength` returns 0 for an absent effect (`:21-30`).
    pub fn strength(&self, effect: &str) -> f64 {
        self.strengths.get(effect).copied().unwrap_or(0.0)
    }

    /// Whether the colony has that effect at all. Different from
    /// `strength(..) != 0.0`: `fleeingspeedaddition` level 1 is a real zero.
    pub fn has(&self, effect: &str) -> bool {
        self.strengths.contains_key(effect)
    }

    pub fn len(&self) -> usize {
        self.strengths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strengths.is_empty()
    }

    /// Every effect and its strength, sorted by id.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> {
        self.strengths
            .iter()
            .map(|(effect, strength)| (effect.as_str(), *strength))
    }

    /// Applies one strength, keeping the **strongest by magnitude** —
    /// `applyEffect` and `overrides` (`ResearchEffectManager.java:32-45`,
    /// `GlobalResearchEffect.java:130-133`). An equal magnitude keeps the one
    /// already held, because Java's comparison is strict.
    pub fn apply(&mut self, effect: &str, strength: f64) {
        // `.copied()` on purpose: matching the borrowed `get` would keep the map
        // borrowed across the arm that inserts into it.
        match self.strengths.get(effect).copied() {
            Some(held) if held.abs() >= strength.abs() => {}
            _ => {
                self.strengths.insert(effect.to_string(), strength);
            }
        }
    }

    /// Every effect one research grants, at the strength its category's ladder
    /// holds for the level it asks.
    pub fn apply_research(&mut self, research: &Research) {
        for unlock in &research.effects {
            let Some(category) = effect::category(unlock.effect) else {
                // Unreachable: `Tree::load` refuses an effect no category
                // declares (`tree::LoadError::UnknownEffect`). A `Research` built
                // by hand can still get here, and an effect with no ladder is
                // worth no number at all.
                continue;
            };
            if let Ok(strength) = category.strength(unlock.level) {
                self.apply(unlock.effect, strength);
            }
        }
    }

    /// Clears and applies every finished research again — `resetEffects`
    /// (`LocalResearchTree.java:318-341`). Because [`Self::apply`] keeps the
    /// strongest, the result does not depend on the order the finished list
    /// comes back in.
    pub fn rebuild(&mut self, tree: &Tree, progress: &Progress) {
        self.strengths.clear();
        for id in progress.finished() {
            if let Some(research) = tree.get(id) {
                self.apply_research(research);
            }
        }
    }

    /// Forgets every effect, `removeAllEffects` (`:57-60`).
    pub fn clear(&mut self) {
        self.strengths.clear();
    }
}

/// The level a caller should pass as `university_level`: the building's own,
/// with Java's maximum-level substitution (`GlobalResearch.java:158`).
pub fn university_level(level: i32) -> i32 {
    gate::university_level(level, MAX_UNIVERSITY_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{SingleLimit, Stack};
    use crate::research::cost::Cost;

    fn tree() -> Tree {
        Tree::load().expect("the shipped table loads")
    }

    fn row(id: &str) -> Research {
        tree().get(id).expect("declared").clone()
    }

    fn inventory(contents: &[(&str, u32)]) -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        let slots = contents
            .iter()
            .map(|(name, count)| {
                let item = crate::inventory::ItemName::new(name).expect("a valid name");
                Some(Stack::new(item, *count).expect("a positive count"))
            })
            .collect();
        Inventory::with_slots(slots, &LIMITS)
    }

    fn empty() -> Inventory<'static> {
        static LIMITS: SingleLimit = SingleLimit(64);
        Inventory::new(8, &LIMITS)
    }

    fn ready() -> Buildings {
        let mut standing = Buildings::new();
        standing.add("residence", 3);
        standing
    }

    #[test]
    fn the_three_states_are_javas_ordinals() {
        assert_eq!(State::NotStarted.ordinal(), 0);
        assert_eq!(State::InProgress.ordinal(), 1);
        assert_eq!(State::Finished.ordinal(), 2);
        assert_eq!(State::from_ordinal(2), Some(State::Finished));
        assert_eq!(State::from_ordinal(3), None);
        assert_eq!(State::from_ordinal(-1), None);
    }

    #[test]
    fn an_empty_progress_is_not_a_failure_to_read() {
        let progress = Progress::new();
        assert!(progress.is_empty());
        assert_eq!(progress.len(), 0);
        assert_eq!(progress.dropped(), 0);
        assert_eq!(progress.state("civilian/keen"), State::NotStarted);
        assert_eq!(progress.progress_of("civilian/keen"), 0);
        assert!(!progress.is_finished("civilian/keen"));
        assert!(!progress.is_running("civilian/keen"));
    }

    #[test]
    fn beginning_a_research_writes_a_record_and_takes_the_cost() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        let mut university = empty();
        let mut player = inventory(&[("minecraft:book", 5)]);
        progress
            .begin(&tree, &keen, 5, &ready(), &mut university, &mut player)
            .expect("three books and a residence");
        assert!(progress.is_running("civilian/keen"));
        assert_eq!(progress.progress_of("civilian/keen"), 0);
        assert_eq!(
            player.count(&crate::inventory::ItemName::new("minecraft:book").expect("valid")),
            2,
            "three books were taken"
        );
    }

    #[test]
    fn a_research_cannot_be_begun_twice() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        let mut university = empty();
        let mut player = inventory(&[("minecraft:book", 8)]);
        progress
            .begin(&tree, &keen, 5, &ready(), &mut university, &mut player)
            .expect("first time");
        let again = progress
            .begin(&tree, &keen, 5, &ready(), &mut university, &mut player)
            .expect_err("second time");
        assert_eq!(again, Refusal::AlreadyStarted { id: "civilian/keen" });
        assert_eq!(
            player.count(&crate::inventory::ItemName::new("minecraft:book").expect("valid")),
            5,
            "a refused start takes nothing"
        );
    }

    #[test]
    fn an_instant_research_starts_at_the_top_and_finishes_on_the_first_action() {
        let tree = tree();
        let stringmesh = row("unlockable/stringmesh");
        assert!(stringmesh.flags.instant());
        let mut progress = Progress::new();
        let mut university = empty();
        let mut player = empty();
        let mut standing = Buildings::new();
        standing.add("sifter", 1);
        progress
            .begin(&tree, &stringmesh, 5, &standing, &mut university, &mut player)
            .expect("costs nothing, sifter is there");
        assert!(progress.is_running("unlockable/stringmesh"));
        assert_eq!(
            progress.progress_of("unlockable/stringmesh"),
            stringmesh.ticks(),
            "started at the top, as `startResearch` does"
        );
        let mut effects = Effects::new();
        assert_eq!(
            progress.advance(&tree, "unlockable/stringmesh", &mut effects),
            Ok(true)
        );
        assert!(progress.is_finished("unlockable/stringmesh"));
        assert_eq!(effects.strength("sifterstringunlock"), 1.0);
    }

    #[test]
    fn a_slow_research_takes_one_action_per_tick_of_its_branch() {
        let tree = tree();
        let keen = row("civilian/keen");
        assert_eq!(keen.ticks(), 72);
        let mut progress = Progress::new();
        progress.record(&keen, State::InProgress, 0);
        let mut effects = Effects::new();
        for expected in 1..72u32 {
            assert_eq!(
                progress.advance(&tree, "civilian/keen", &mut effects),
                Ok(false),
                "action {expected}"
            );
            assert_eq!(progress.progress_of("civilian/keen"), expected);
        }
        assert_eq!(
            progress.advance(&tree, "civilian/keen", &mut effects),
            Ok(true),
            "the 72nd action finishes it"
        );
        assert!(progress.is_finished("civilian/keen"));
        assert_eq!(effects.strength("blockhutlibrary"), 5.0);
    }

    #[test]
    fn an_unknown_or_idle_research_cannot_advance() {
        let tree = tree();
        let mut progress = Progress::new();
        let mut effects = Effects::new();
        assert_eq!(
            progress.advance(&tree, "civilian/nosuch", &mut effects),
            Err(Refusal::UnknownResearch {
                id: "civilian/nosuch".to_string()
            })
        );
        assert_eq!(
            progress.advance(&tree, "civilian/keen", &mut effects),
            Err(Refusal::NotRunning {
                id: "civilian/keen".to_string()
            })
        );
    }

    #[test]
    fn cancelling_drops_a_running_record_and_keeps_a_finished_one() {
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        progress.record(&keen, State::InProgress, 10);
        assert_eq!(progress.cancel("civilian/keen"), Ok(()));
        assert_eq!(progress.state("civilian/keen"), State::NotStarted);
        progress.record(&keen, State::Finished, 72);
        assert_eq!(
            progress.cancel("civilian/keen"),
            Err(Refusal::NotRunning {
                id: "civilian/keen".to_string()
            }),
            "cancelling is for a running research; a finished one is undone"
        );
    }

    #[test]
    fn undoing_a_finished_research_removes_its_effect() {
        let tree = tree();
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        progress.record(&keen, State::Finished, 72);
        let mut effects = progress.rebuild_effects(&tree);
        assert_eq!(effects.strength("blockhutlibrary"), 5.0);
        progress
            .undo(&tree, "civilian/keen", &mut effects)
            .expect("no child, not immutable, not autostart");
        assert!(!progress.is_finished("civilian/keen"));
        assert_eq!(effects.strength("blockhutlibrary"), 0.0);
        assert!(effects.is_empty());
    }

    #[test]
    fn undoing_something_that_is_not_finished_is_refused() {
        let tree = tree();
        let mut progress = Progress::new();
        let mut effects = Effects::new();
        assert_eq!(
            progress.undo(&tree, "civilian/keen", &mut effects),
            Err(Refusal::NotFinished {
                id: "civilian/keen".to_string()
            })
        );
    }

    #[test]
    fn an_exclusive_chain_records_one_child_at_a_time() {
        let tree = tree();
        let accuracy = row("combat/accuracy");
        let quickdraw = row("combat/quickdraw");
        let mut progress = Progress::new();
        progress.record(&accuracy, State::Finished, accuracy.ticks());
        assert_eq!(progress.recorded_child(&tree, "combat/accuracy"), None);
        progress.record(&quickdraw, State::Finished, quickdraw.ticks());
        assert_eq!(
            progress.recorded_child(&tree, "combat/accuracy"),
            Some("combat/quickdraw")
        );
    }

    #[test]
    fn a_running_depth_six_research_already_closes_its_branch() {
        let tree = tree();
        let beanstalk = row("civilian/beanstalk");
        assert_eq!(beanstalk.depth, 6);
        let mut progress = Progress::new();
        assert_eq!(progress.branch_ceiling(Branch::Civilian), None);
        progress.record(&beanstalk, State::InProgress, 3);
        assert_eq!(
            progress.branch_ceiling(Branch::Civilian),
            Some("civilian/beanstalk")
        );
        assert_eq!(progress.branch_ceiling(Branch::Combat), None);
    }

    #[test]
    fn the_strongest_effect_wins_whatever_the_order() {
        let mut effects = Effects::new();
        effects.apply("levelingmultiplier", 0.05);
        effects.apply("levelingmultiplier", 1.0);
        effects.apply("levelingmultiplier", 0.25);
        assert_eq!(effects.strength("levelingmultiplier"), 1.0);
        // Java compares magnitudes strictly, so an equal one does not replace.
        effects.apply("healingsaturationlimitaddition", -1.0);
        effects.apply("healingsaturationlimitaddition", -1.0);
        assert_eq!(effects.strength("healingsaturationlimitaddition"), -1.0);
    }

    #[test]
    fn the_strongest_is_the_strongest_in_magnitude_not_the_largest() {
        let mut effects = Effects::new();
        effects.apply("healingsaturationlimitaddition", -0.5);
        effects.apply("healingsaturationlimitaddition", -2.0);
        assert_eq!(effects.strength("healingsaturationlimitaddition"), -2.0);
        assert!(effects.has("healingsaturationlimitaddition"));
        assert!(!effects.has("happinessmultiplier"));
    }

    #[test]
    fn a_declared_zero_is_held_as_an_effect() {
        let mut effects = Effects::new();
        // `fleeingspeedaddition` level 1 is a real 0.0.
        effects.apply("fleeingspeedaddition", 0.0);
        assert!(effects.has("fleeingspeedaddition"));
        assert_eq!(effects.strength("fleeingspeedaddition"), 0.0);
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn rebuilding_gives_the_same_answer_as_applying_in_order() {
        let tree = tree();
        let keen = row("civilian/keen");
        let diligent = row("civilian/diligent");
        let scholarly = row("civilian/scholarly");
        let mut progress = Progress::new();
        progress.record(&keen, State::Finished, keen.ticks());
        progress.record(&diligent, State::Finished, diligent.ticks());
        progress.record(&scholarly, State::Finished, scholarly.ticks());
        let rebuilt = progress.rebuild_effects(&tree);
        let mut in_order = Effects::new();
        in_order.apply_research(&keen);
        in_order.apply_research(&diligent);
        in_order.apply_research(&scholarly);
        assert_eq!(rebuilt, in_order);
        assert_eq!(rebuilt.strength("levelingmultiplier"), 0.25);
        assert_eq!(rebuilt.len(), 2, "levelingmultiplier and blockhutlibrary");
    }

    #[test]
    fn the_report_writes_the_five_keys_a_record_needs() {
        let keen = row("civilian/keen");
        let mut progress = Progress::new();
        progress.record(&keen, State::InProgress, 40);
        let data = progress.to_compound();
        let records = nbt::compounds(&data, KEY_TREE);
        assert_eq!(records.len(), 1);
        let record = records[0];
        assert_eq!(record.len(), 5, "state, id, branch, progress, depth");
        assert_eq!(nbt::int(record, KEY_STATE), Some(1));
        assert_eq!(nbt::text(record, KEY_ID).as_deref(), Some("civilian/keen"));
        assert_eq!(nbt::text(record, KEY_BRANCH).as_deref(), Some("civilian"));
        assert_eq!(nbt::int(record, KEY_PROGRESS), Some(40));
        assert_eq!(nbt::int(record, KEY_DEPTH), Some(1));
    }

    #[test]
    fn the_report_round_trips() {
        let tree = tree();
        let keen = row("civilian/keen");
        let academic = row("civilian/academic");
        let mut progress = Progress::new();
        progress.record(&keen, State::Finished, 72);
        progress.record(&academic, State::InProgress, 12);
        let read = Progress::from_compound(&progress.to_compound());
        assert_eq!(read.len(), 2);
        assert_eq!(read.state("civilian/keen"), State::Finished);
        assert_eq!(read.state("civilian/academic"), State::InProgress);
        assert_eq!(read.progress_of("civilian/academic"), 12);
        assert_eq!(read.dropped(), 0);
        assert_eq!(read.finished().collect::<Vec<_>>(), vec!["civilian/keen"]);
        assert_eq!(read.running().collect::<Vec<_>>(), vec!["civilian/academic"]);
        // Only the finished one gives its effect: `academic` is still running,
        // and `resetEffects` applies the finished list only
        // (`LocalResearchTree.java:325-340`).
        let effects = read.rebuild_effects(&tree);
        assert_eq!(effects.strength("blockhutlibrary"), 5.0);
        assert_eq!(effects.strength("levelingmultiplier"), 0.0);
    }

    #[test]
    fn a_broken_record_is_dropped_and_counted() {
        let good = Compound::from([
            (KEY_STATE.into(), Tag::Int(2)),
            (KEY_ID.into(), Tag::String("civilian/keen".into())),
            (KEY_BRANCH.into(), Tag::String("civilian".into())),
            (KEY_PROGRESS.into(), Tag::Int(72)),
            (KEY_DEPTH.into(), Tag::Int(1)),
        ]);
        let no_id = Compound::from([
            (KEY_STATE.into(), Tag::Int(2)),
            (KEY_BRANCH.into(), Tag::String("civilian".into())),
        ]);
        let bad_state = Compound::from([
            (KEY_STATE.into(), Tag::Int(9)),
            (KEY_ID.into(), Tag::String("civilian/air".into())),
            (KEY_BRANCH.into(), Tag::String("civilian".into())),
        ]);
        let not_started = Compound::from([
            (KEY_STATE.into(), Tag::Int(0)),
            (KEY_ID.into(), Tag::String("civilian/haste".into())),
            (KEY_BRANCH.into(), Tag::String("civilian".into())),
        ]);
        let data = Compound::from([(
            KEY_TREE.into(),
            nbt::compound_list(vec![good, no_id, bad_state, not_started]),
        )]);
        let progress = Progress::from_compound(&data);
        assert_eq!(progress.len(), 1);
        assert_eq!(progress.dropped(), 3);
        assert!(progress.is_finished("civilian/keen"));
    }

    #[test]
    fn a_record_of_a_research_the_tree_lost_is_kept_and_named() {
        let tree = tree();
        let data = Compound::from([(
            KEY_TREE.into(),
            nbt::compound_list(vec![Compound::from([
                (KEY_STATE.into(), Tag::Int(2)),
                (KEY_ID.into(), Tag::String("civilian/removed".into())),
                (KEY_BRANCH.into(), Tag::String("civilian".into())),
                (KEY_PROGRESS.into(), Tag::Int(10)),
                (KEY_DEPTH.into(), Tag::Int(2)),
            ])]),
        )]);
        let progress = Progress::from_compound(&data);
        assert_eq!(progress.len(), 1, "kept, as Java keeps it");
        assert_eq!(progress.unknown(&tree), vec!["civilian/removed"]);
        // Its effect is not applied: the tree is what says what it came with.
        assert!(progress.rebuild_effects(&tree).is_empty());
    }

    #[test]
    fn a_compound_with_no_tree_key_reads_as_nothing() {
        let progress = Progress::from_compound(&Compound::new());
        assert!(progress.is_empty());
        assert_eq!(progress.dropped(), 0);
    }

    #[test]
    fn the_university_level_is_the_substituted_one() {
        assert_eq!(university_level(5), i32::MAX);
        assert_eq!(university_level(3), 3);
        assert_eq!(university_level(MAX_UNIVERSITY_LEVEL), i32::MAX);
    }

    #[test]
    fn a_research_with_a_tag_cost_can_be_begun_from_either_kind_of_ore() {
        let tree = tree();
        let veinminer = row("technology/veinminer");
        assert!(matches!(veinminer.costs[0], Cost::Tag { .. }));
        let mut standing = Buildings::new();
        standing.add("miner", 1);
        let mut university = empty();
        let mut player = inventory(&[("minecraft:deepslate_iron_ore", 32)]);
        let progress = progress_begin(&tree, &veinminer, &standing, &mut university, &mut player);
        assert!(progress.is_running("technology/veinminer"));
        assert_eq!(player.count(&crate::inventory::ItemName::new("minecraft:deepslate_iron_ore").expect("valid")), 0);
    }

    /// Hands `begin` a colony whose research ancestors are already finished.
    ///
    /// The test above is about a **cost**, but `begin` runs Java's tree rules
    /// first and one of them is "the parent is finished" (`gate.rs:223-229`,
    /// `GlobalResearch.canResearch:163-164`). `technology/veinminer.json:17`
    /// hangs under `technology/hittingiron`, so without this the call answers
    /// `Refusal::ParentUnfinished` and never reaches the price. The tag itself is
    /// not the problem: `cost::TAGS` carries both ores for
    /// `minecraft:iron_ores` (`cost.rs:99-103`), and the passing test
    /// `gate::tests::paying_a_tag_cost_spreads_over_its_members` already pays
    /// this same cost out of `minecraft:iron_ore` and
    /// `minecraft:deepslate_iron_ore` together.
    fn progress_begin(
        tree: &Tree,
        research: &Research,
        standing: &Buildings,
        university: &mut Inventory<'_>,
        player: &mut Inventory<'_>,
    ) -> Progress {
        let mut progress = Progress::new();
        let mut ancestor = research.parent;
        while let Some(id) = ancestor {
            let parent = tree.get(id).expect("a declared parent");
            progress.record(parent, State::Finished, parent.ticks());
            ancestor = parent.parent;
        }
        progress
            .begin(tree, research, 5, standing, university, player)
            .expect("the cost is there");
        progress
    }
}
