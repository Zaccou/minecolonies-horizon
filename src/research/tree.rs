//! The 217 researches: parent, depth, flags, costs, effects, requirements.
//!
//! Java reference: `core/research/GlobalResearch.java` (372 lines, one research)
//! and `core/datalistener/ResearchListener.java` (the loader that reads the 217
//! JSON files and drops the rows that do not hold).
//!
//! **One line per research, in the encoding Java would have to parse anyway.**
//! [`TABLE`] is `id|parent|branch|depth|flags|costs|effects|requirements`, one
//! row per file, in file-name order. It is *data*, exactly as the JSON was data;
//! [`Tree::load`] is the reader, and it refuses what Java refuses:
//!
//! | Java | Line | Here |
//! | --- | --- | --- |
//! | a research with no `branch` is skipped | `:245-252` | [`LoadError::MissingField`] |
//! | a parent that does not exist drops the row | `:581-586` | [`LoadError::MissingParent`] |
//! | a parent that is not exactly one depth up drops the row | `:588-603` | [`LoadError::DepthGap`] |
//! | an effect above its category's top is dropped | `:437-443` | [`LoadError::EffectLevelAboveMax`] |
//! | two files for one id: the last wins, with an error logged | `GlobalResearchTree.java:139-142` | [`LoadError::DuplicateId`] |
//!
//! Refusing instead of skipping is a deliberate difference: Java runs inside a
//! game whose data pack may be wrong, and a wrong row must not stop the world
//! from loading. This table is compiled into the mod, so a wrong row is a bug in
//! the port, and a load error is the only way to see it.
//!
//! **The encoding, field by field.**
//!
//! - `id` — the path, `civilian/academic`. Not always the folder: `civilian/moq`
//!   is a **technology** research whose file sits in `civilian/`; its `branch`
//!   field says so, and Java keys the tree by the field (`:245`), not by the
//!   path. The id is what `parentResearch` names, so both are kept.
//! - `parent` — `-` for the 19 roots, else the parent's id.
//! - `branch` — `c`, `k`, `t`, `u` ([`Branch::tag`]).
//! - `depth` — `researchLevel`, 1 to 6.
//! - `flags` — `-`, or a comma list of `h` `hidden`, `a` `autostart`, `i`
//!   `instant`, `n` `no-reset` (kept as [`Flags::IMMUTABLE`]), `x`
//!   `exclusiveChildResearch`, and `s<N>` for `sortOrder`. 4 rows are hidden, 4
//!   autostart, 4 instant, 3 immutable, 5 exclusive; 54 carry a sort order and
//!   the rest use Java's default of 1000 (`ResearchListener.java:283`).
//! - `costs` — `-`, or a comma list of `item:count`, `#tag:count` and
//!   `{a+b}:count`. Counted from the right, because an item id has a colon in
//!   it.
//! - `effects` — `-`, or a comma list of `effect:level`, the effect being the
//!   path after `minecolonies:effects/`. Every row has exactly one.
//! - `requirements` — `-`, or a comma list of `bd:kind:level`,
//!   `sd:kind:level`, `alt:a+b:level`, `rs:id`.
//!
//! **What is dropped, and why.** `icon` (a display stack), `subtitle` (a
//! translation key on 115 rows), `name` (no row has one) and `hidden`'s client
//! meaning are all content: the port keeps the flags that change a *decision* and
//! lets the caller draw whatever it draws.
//!
//! **Each row is its own source.** Its `id` names the file it came from:
//! `researches/<id>.json` under
//! `src/datagen/generated/minecolonies/data/minecolonies/`, so a `source` field
//! on 217 rows would repeat the id; the branches, which are four, carry one
//! ([`super::branch::BranchInfo::source`]).

use std::collections::BTreeMap;

use super::branch::Branch;
use super::cost::{Cost, Requirement};
use super::effect::{EffectCategory, LevelError};
use super::{MAX_DEPTH, RESEARCH_COUNT};

/// The five booleans and the sort order a research carries, as one byte plus a
/// number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    pub const NONE: Flags = Flags(0);
    /// `hidden`: only visible when its requirements are met
    /// (`ResearchListener.java:90`).
    pub const HIDDEN: Flags = Flags(1 << 0);
    /// `autostart`: begins by itself when it becomes available
    /// (`ResearchListener.java:96`).
    pub const AUTOSTART: Flags = Flags(1 << 1);
    /// `instant`: finishes on its first study action
    /// (`ResearchListener.java:101`).
    pub const INSTANT: Flags = Flags(1 << 2);
    /// `no-reset`, Java's `immutable`: cannot be undone
    /// (`ResearchListener.java:136`, `:303`).
    pub const IMMUTABLE: Flags = Flags(1 << 3);
    /// `exclusiveChildResearch`: its parent allows one researched child only
    /// (`ResearchListener.java:126`, `GlobalResearch.java:164-165`).
    pub const EXCLUSIVE: Flags = Flags(1 << 4);

    pub const fn with(self, other: Flags) -> Flags {
        Flags(self.0 | other.0)
    }

    pub const fn hidden(self) -> bool {
        self.0 & Flags::HIDDEN.0 != 0
    }

    pub const fn autostarts(self) -> bool {
        self.0 & Flags::AUTOSTART.0 != 0
    }

    pub const fn instant(self) -> bool {
        self.0 & Flags::INSTANT.0 != 0
    }

    pub const fn immutable(self) -> bool {
        self.0 & Flags::IMMUTABLE.0 != 0
    }

    pub const fn exclusive_child(self) -> bool {
        self.0 & Flags::EXCLUSIVE.0 != 0
    }
}

/// One effect a research grants, at one level of its category's ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlock {
    /// The path after `minecolonies:effects/`.
    pub effect: &'static str,
    pub level: u8,
}

/// One research, as the JSON declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct Research {
    pub id: &'static str,
    pub branch: Branch,
    pub parent: Option<&'static str>,
    /// `researchLevel`.
    pub depth: u8,
    /// `sortOrder`, 1000 by default.
    pub sort: u16,
    pub flags: Flags,
    pub costs: Vec<Cost>,
    pub effects: Vec<Unlock>,
    pub requirements: Vec<Requirement>,
}

impl Research {
    /// How long it takes in study actions, at its branch's base time
    /// (`GlobalResearchBranch.java:83-86`).
    pub fn ticks(&self) -> u32 {
        self.branch.ticks(self.depth)
    }

    /// The GUI's estimate, in hours (`:89-92`).
    pub fn hours(&self) -> f64 {
        self.branch.hours(self.depth)
    }

    /// Is it a root — one of the 19 rows with no parent?
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// The one effect this row grants, when it grants exactly one (every row
    /// does).
    pub fn unlock(&self) -> Option<Unlock> {
        match self.effects.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }
}

/// Why the table does not load. Every variant says which row, because a data
/// error without a line number is a scavenger hunt.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one variant per refusal the loader has
pub enum LoadError {
    /// A row does not have the eight fields the encoding declares.
    FieldCount { row: usize, found: usize },
    /// A field that cannot be empty is.
    MissingField { row: usize, field: &'static str },
    /// `branch` is not one of the four tags.
    UnknownBranch { row: usize, tag: String },
    /// A flag tag that is neither one of the five nor `s<N>`.
    UnknownFlag { row: usize, tag: String },
    /// `depth` is not a number in `1..=MAX_DEPTH`.
    BadDepth { row: usize, value: String },
    /// A cost entry that does not read as `item:count` and the like.
    BadCost { row: usize, spec: String },
    /// A cost of zero items: Java clamps it to one (`SimpleItemCost.java:63`),
    /// the data should not say it.
    ZeroCost { row: usize, spec: String },
    /// An effect entry that does not read as `effect:level`.
    BadEffect { row: usize, spec: String },
    /// An effect no category declares (`ResearchListener.java:428-436`).
    UnknownEffect { row: usize, effect: String },
    /// An effect level above its category's top (`:437-443`).
    EffectLevelAboveMax {
        row: usize,
        effect: String,
        level: u8,
        max: u8,
    },
    /// A requirement entry that does not read as one of the four forms.
    BadRequirement { row: usize, spec: String },
    /// Two rows with one id (`GlobalResearchTree.java:139-142`).
    DuplicateId { id: String },
    /// A parent no row declares (`ResearchListener.java:581-586`).
    MissingParent { id: String, parent: String },
    /// A parent that is not exactly one depth up (`:588-603`).
    DepthGap {
        id: String,
        depth: u8,
        parent: String,
        parent_depth: u8,
    },
}

impl LoadError {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(&self) -> &'static str {
        match self {
            LoadError::FieldCount { .. } => "minecolonies:research/field-count",
            LoadError::MissingField { .. } => "minecolonies:research/missing-field",
            LoadError::UnknownBranch { .. } => "minecolonies:research/unknown-branch",
            LoadError::UnknownFlag { .. } => "minecolonies:research/unknown-flag",
            LoadError::BadDepth { .. } => "minecolonies:research/bad-depth",
            LoadError::BadCost { .. } => "minecolonies:research/bad-cost",
            LoadError::ZeroCost { .. } => "minecolonies:research/zero-cost",
            LoadError::BadEffect { .. } => "minecolonies:research/bad-effect",
            LoadError::UnknownEffect { .. } => "minecolonies:research/unknown-effect",
            LoadError::EffectLevelAboveMax { .. } => "minecolonies:research/effect-level-above-max",
            LoadError::BadRequirement { .. } => "minecolonies:research/bad-requirement",
            LoadError::DuplicateId { .. } => "minecolonies:research/duplicate-id",
            LoadError::MissingParent { .. } => "minecolonies:research/missing-parent",
            LoadError::DepthGap { .. } => "minecolonies:research/depth-gap",
        }
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = match self {
            LoadError::FieldCount { row, found } => {
                format!("row {row} has {found} fields, not 8")
            }
            LoadError::MissingField { row, field } => format!("row {row} has no {field}"),
            LoadError::UnknownBranch { row, tag } => format!("row {row} names branch {tag:?}"),
            LoadError::UnknownFlag { row, tag } => format!("row {row} names flag {tag:?}"),
            LoadError::BadDepth { row, value } => format!("row {row} has depth {value:?}"),
            LoadError::BadCost { row, spec } => format!("row {row} has cost {spec:?}"),
            LoadError::ZeroCost { row, spec } => format!("row {row} costs nothing: {spec:?}"),
            LoadError::BadEffect { row, spec } => format!("row {row} has effect {spec:?}"),
            LoadError::UnknownEffect { row, effect } => {
                format!("row {row} names effect {effect:?}")
            }
            LoadError::EffectLevelAboveMax {
                row,
                effect,
                level,
                max,
            } => format!("row {row} asks {effect} level {level}, above {max}"),
            LoadError::BadRequirement { row, spec } => format!("row {row} has requirement {spec:?}"),
            LoadError::DuplicateId { id } => format!("{id} is declared twice"),
            LoadError::MissingParent { id, parent } => format!("{id} hangs under {parent}, absent"),
            LoadError::DepthGap {
                id,
                depth,
                parent,
                parent_depth,
            } => format!("{id} is depth {depth} under {parent}, depth {parent_depth}"),
        };
        write!(formatter, "{}: {detail}", self.name())
    }
}

/// The tree: the loaded rows, indexed by id, plus one index per parent and per
/// effect. Java builds the same three indexes by hand
/// (`GlobalResearchTree.java:34-52`).
#[derive(Clone, Debug)]
pub struct Tree {
    rows: Vec<Research>,
    by_id: BTreeMap<&'static str, usize>,
    children: BTreeMap<&'static str, Vec<usize>>,
}

impl Tree {
    /// Reads [`TABLE`]. Every refusal Java performs is a refusal here, with the
    /// row number attached.
    pub fn load() -> Result<Tree, LoadError> {
        Tree::parse(TABLE)
    }

    /// Reads any table in [`TABLE`]'s encoding. Split from [`Self::load`] so a
    /// test can hand the reader three bad rows and see which refusal comes back,
    /// instead of having to break the shipped table to exercise it.
    pub fn parse(table: &'static str) -> Result<Tree, LoadError> {
        let mut rows: Vec<Research> = Vec::with_capacity(RESEARCH_COUNT);
        let mut by_id: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (index, line) in table.lines().enumerate() {
            let row = index + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let research = parse_row(line, row)?;
            if by_id.insert(research.id, rows.len()).is_some() {
                return Err(LoadError::DuplicateId {
                    id: research.id.to_string(),
                });
            }
            rows.push(research);
        }

        // Parents and depths, in the order Java validates them: a parent that
        // does not exist first, then a parent that is not one level up
        // (`ResearchListener.java:581-603`).
        for research in &rows {
            let Some(parent_id) = research.parent else {
                continue;
            };
            let Some(&parent_index) = by_id.get(parent_id) else {
                return Err(LoadError::MissingParent {
                    id: research.id.to_string(),
                    parent: parent_id.to_string(),
                });
            };
            let parent_depth = rows[parent_index].depth;
            if research.depth != parent_depth + 1 {
                return Err(LoadError::DepthGap {
                    id: research.id.to_string(),
                    depth: research.depth,
                    parent: parent_id.to_string(),
                    parent_depth,
                });
            }
        }

        let mut children: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();
        for (index, research) in rows.iter().enumerate() {
            if let Some(parent) = research.parent {
                children.entry(parent).or_default().push(index);
            }
        }

        Ok(Tree {
            rows,
            by_id,
            children,
        })
    }

    /// How many researches the tree holds — [`RESEARCH_COUNT`] for the table
    /// this crate ships.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// A tree is never empty: [`Tree::load`] on an empty table succeeds and a
    /// test says so, because `is_empty` has to exist for the lint.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// One research by id.
    pub fn get(&self, id: &str) -> Option<&Research> {
        self.by_id.get(id).map(|index| &self.rows[*index])
    }

    /// Every row, in file order.
    pub fn all(&self) -> impl Iterator<Item = &Research> {
        self.rows.iter()
    }

    /// Every row of one branch, in file order.
    pub fn in_branch(&self, branch: Branch) -> impl Iterator<Item = &Research> {
        self.rows.iter().filter(move |row| row.branch == branch)
    }

    /// The rows with no parent, sorted by id — Java's `getPrimaryResearch`
    /// (`GlobalResearchTree.java:203-215`).
    pub fn roots(&self) -> impl Iterator<Item = &Research> {
        let mut roots: Vec<&Research> = self.rows.iter().filter(|row| row.is_root()).collect();
        roots.sort_by(|left, right| left.id.cmp(right.id));
        roots.into_iter()
    }

    /// The rows hanging under one id, direct children only.
    pub fn children(&self, id: &str) -> impl Iterator<Item = &Research> {
        self.children
            .get(id)
            .into_iter()
            .flatten()
            .map(|index| &self.rows[*index])
    }

    /// The rows that grant one effect — Java's `researchEffectsIds`
    /// (`GlobalResearchTree.java:186-196`).
    pub fn of_effect(&self, effect: &str) -> impl Iterator<Item = &Research> {
        self.rows
            .iter()
            .filter(move |row| row.effects.iter().any(|unlock| unlock.effect == effect))
    }

    /// The category an effect id names, or `None`.
    pub fn category(&self, effect: &str) -> Option<&'static EffectCategory> {
        super::effect::category(effect)
    }
}

/// The 217 researches, one per line, in file-name order.
///
/// `id|parent|branch|depth|flags|costs|effects|requirements` — see the module
/// documentation for the encoding and for the line numbers of every rule the
/// loader enforces. `civilian/` 69, `combat/` 68, `technology/` 76,
/// `unlockable/` 4.
pub const TABLE: &str = "\
civilian/academic|civilian/reflective|c|6|-|minecraft:book:128|levelingmultiplier:5|-
civilian/agile|civilian/nimble|c|4|-|minecraft:rabbit_foot:8|walkingmultiplier:3|sd:townhall:4
civilian/air|civilian/ambition|c|2|s1|minecraft:heart_of_the_sea:1|air:1|-
civilian/ambition|-|c|1|s5|minecraft:diamond:1|blockhutmysticalsite:1|-
civilian/athlete|civilian/swift|c|6|-|minecraft:rabbit_foot:64|walkingmultiplier:5|-
civilian/bachelor|civilian/bookworm|c|4|-|minecraft:bookshelf:12|teachingmultiplier:3|bd:library:3
civilian/bandages|civilian/healingcream|c|4|-|minecraft:golden_carrot:16|regenerationmultiplier:3|bd:library:4
civilian/bandaid|civilian/stamina|c|2|-|minecraft:golden_carrot:1|regenerationmultiplier:1|bd:library:2
civilian/beanstalk|civilian/growth|c|6|-|minecraft:cooked_chicken:512|growthmultiplier:5|-
civilian/bookworm|civilian/morebooks|c|3|-|minecraft:bookshelf:6|teachingmultiplier:2|bd:school:3
civilian/cast|civilian/compress|c|6|-|minecraft:golden_carrot:64|regenerationmultiplier:5|-
civilian/circus|civilian/firstaid|c|2|s2|minecraft:cake:1|happinessmultiplier:1|bd:cook:2
civilian/city|civilian/village|c|5|-|minecraft:cooked_beef:512|citizencapaddition:5|sd:townhall:5
civilian/compress|civilian/bandages|c|5|-|minecraft:golden_carrot:32|regenerationmultiplier:4|bd:library:5
civilian/diligent|civilian/keen|c|2|s2|minecraft:book:6|levelingmultiplier:1|bd:library:2
civilian/epicure|civilian/stuffer|c|5|-|minecraft:cookie:256|saturationmultiplier:4|bd:cook:5
civilian/festival|civilian/circus|c|3|-|minecraft:cake:9|happinessmultiplier:2|bd:cook:3
civilian/firstaid|-|c|1|s3|minecraft:hay_block:8|healthaddition:1|sd:townhall:1
civilian/firstaid2|civilian/firstaid|c|2|-|minecraft:hay_block:16|healthaddition:2|sd:townhall:2
civilian/fortitude|civilian/vitality|c|5|-|minecraft:golden_apple:32|healingsaturationlimitaddition:4|bd:cook:5
civilian/glutton|civilian/epicure|c|6|-|minecraft:cookie:512|saturationmultiplier:5|-
civilian/gorger|civilian/gourmand|c|3|-|minecraft:cookie:64|saturationmultiplier:2|bd:cook:3
civilian/gourmand|civilian/firstaid|c|2|s3|minecraft:cookie:32|saturationmultiplier:1|bd:cook:2
civilian/gravedecaybonus1|civilian/remembrance|c|2|s2|minecraft:rotten_flesh:64|gravedecaybonus:1|bd:graveyard:3
civilian/gravedecaybonus2|civilian/gravedecaybonus1|c|3|s2|minecraft:nether_wart_block:8|gravedecaybonus:2|bd:graveyard:5
civilian/growth|civilian/puberty|c|5|-|minecraft:cooked_chicken:256|growthmultiplier:4|bd:library:5
civilian/guardianangel|civilian/lifesaver2|c|5|-|minecraft:hay_block:128|healthaddition:5|sd:townhall:5
civilian/guardianangel2|civilian/guardianangel|c|6|-|minecraft:hay_block:256|healthaddition:6|-
civilian/hamlet|civilian/outpost|c|3|-|minecraft:cooked_beef:128|citizencapaddition:3|bd:residence:5
civilian/haste|civilian/keen|c|2|s3|minecraft:rabbit_foot:1|walkingmultiplier:1|sd:townhall:3
civilian/healingcream|civilian/bandaid|c|3|-|minecraft:golden_carrot:8|regenerationmultiplier:2|bd:library:3
civilian/higherlearning|-|c|1|x,s4|minecraft:book:3|blockhutschool:1|bd:residence:3
civilian/hormones|civilian/nurture|c|3|-|minecraft:cooked_chicken:64|growthmultiplier:2|bd:school:3
civilian/indefatigability|civilian/fortitude|c|6|-|minecraft:golden_apple:64|healingsaturationlimitaddition:5|-
civilian/keen|-|c|1|s2|minecraft:book:3|blockhutlibrary:1|bd:residence:3
civilian/lifesaver|civilian/firstaid2|c|3|-|minecraft:hay_block:32|healthaddition:3|sd:townhall:3
civilian/lifesaver2|civilian/lifesaver|c|4|-|minecraft:hay_block:64|healthaddition:4|sd:townhall:4
civilian/masks|civilian/firstaid2|c|3|-|minecraft:white_wool:32|masks:1|bd:library:3
civilian/master|civilian/bachelor|c|5|-|minecraft:bookshelf:32|teachingmultiplier:4|bd:library:5
civilian/moq|technology/memoryaid|t|3|s3|minecolonies:clipboard:1,minecraft:book:16|min_order:1|bd:deliveryman:9
civilian/morebooks|civilian/higherlearning|c|2|-|minecraft:book:6|teachingmultiplier:1|bd:school:1
civilian/nightowl|civilian/circus|c|3|s2|minecraft:golden_carrot:25|workingdayhaddition:1|bd:library:2
civilian/nightowl2|civilian/nightowl|c|4|s2|minecraft:golden_carrot:75|workingdayhaddition:2|sd:townhall:3
civilian/nimble|civilian/haste|c|3|s3|minecraft:rabbit_foot:4|walkingmultiplier:2|sd:townhall:3
civilian/nurture|civilian/higherlearning|c|2|s2|minecraft:cooked_chicken:32|growthmultiplier:1|bd:school:1
civilian/opera|civilian/spectacle|c|5|-|minecraft:cake:27|happinessmultiplier:4|bd:cook:5
civilian/outpost|civilian/keen|c|2|-|minecraft:cooked_beef:64|citizencapaddition:2|bd:residence:4
civilian/phd|civilian/master|c|6|-|minecraft:bookshelf:64|teachingmultiplier:5|-
civilian/puberty|civilian/hormones|c|4|-|minecraft:cooked_chicken:128|growthmultiplier:3|bd:library:3
civilian/rails|civilian/keen|c|2|s4|minecraft:rail:64|railsunlock:1|bd:deliveryman:3
civilian/raisingthedead|civilian/resurrectchance2|c|4|s1|minecraft:totem_of_undying:1|usetotemunlock:1|bd:graveyard:5
civilian/reflective|civilian/scholarly|c|5|-|minecraft:book:48|levelingmultiplier:4|bd:library:5
civilian/remembrance|-|c|1|s6|minecraft:bone:8|blockhutgraveyard:1|sd:townhall:2
civilian/resilience|civilian/resistance|c|3|-|minecraft:golden_apple:8|healingsaturationlimitaddition:2|bd:cook:3
civilian/resistance|civilian/stamina|c|2|s2|minecraft:golden_apple:1|healingsaturationlimitaddition:1|bd:cook:2
civilian/resurrectchance1|civilian/remembrance|c|2|s1|minecraft:ghast_tear:1|resurrectchanceaddition:1|bd:graveyard:3
civilian/resurrectchance2|civilian/resurrectchance1|c|3|s1|minecraft:chorus_fruit:16|resurrectchanceaddition:2|bd:graveyard:5
civilian/scholarly|civilian/studious|c|4|-|minecraft:book:24|levelingmultiplier:3|bd:library:4
civilian/spectacle|civilian/festival|c|4|-|minecraft:cake:18|happinessmultiplier:3|bd:cook:4
civilian/stamina|-|c|1|x|minecraft:carrot:1|blockhuthospital:1|-
civilian/studious|civilian/diligent|c|3|-|minecraft:book:12|levelingmultiplier:2|bd:library:3
civilian/stuffer|civilian/gorger|c|4|-|minecraft:cookie:128|saturationmultiplier:3|bd:cook:4
civilian/swift|civilian/agile|c|5|-|minecraft:rabbit_foot:32|walkingmultiplier:4|sd:townhall:5
civilian/theater|civilian/opera|c|6|-|minecraft:enchanted_golden_apple:16|happinessmultiplier:5|-
civilian/undertakeremergency|civilian/remembrance|c|2|s3|minecraft:iron_boots:1|undertakerrun:1|bd:graveyard:2
civilian/vaccines|civilian/masks|c|4|-|minecraft:egg:64|vaccines:1|bd:hospital:3
civilian/village|civilian/hamlet|c|4|-|minecraft:cooked_beef:256|citizencapaddition:4|sd:townhall:4
civilian/vines|civilian/keen|c|2|s5|minecraft:vine:64|vinesunlock:1|bd:residence:3
civilian/vitality|civilian/resilience|c|4|-|minecraft:golden_apple:16|healingsaturationlimitaddition:3|bd:cook:4
combat/accuracy|-|k|1|x|minecraft:iron_ingot:16|guardcrit:1|bd:guardtower:1
combat/advancedbarding|combat/improvedbarding|k|5|-|minecraft:golden_horse_armor:1,minecraft:diamond:5|mountarmor:3|bd:stable:5,bd:blacksmith:5
combat/agilearcher|combat/improvedevasion|k|6|-|minecraft:diamond:64|archerarmormultiplier:5|-
combat/arrowpierce|combat/arrowuse|k|3|-|minecraft:arrow:64,minecraft:redstone:64|piercingarrowsunlock:1|bd:archery:1
combat/arrowuse|combat/taunt|k|2|-|minecraft:arrow:64|consumearrowsunlock:1|bd:guardtower:2
combat/avoid|combat/regeneration|k|3|s2|minecraft:emerald:8|fleeingspeedaddition:1|bd:guardtower:4
combat/avoidance|-|k|1|x,s3|minecraft:iron_block:3|shieldusageunlock:1|bd:guardtower:3
combat/barding|combat/mountedcombat|k|3|-|minecraft:leather_horse_armor:8,minecraft:iron_ingot:5|mountarmor:1|bd:stable:3,bd:blacksmith:3
combat/boiledleather|combat/improvedleather|k|2|-|minecraft:leather:64|armordurabilitymultiplier:2|sd:townhall:2
combat/bucephalus|combat/exerciseregimen|k|6|-|minecraft:nether_star:1,minecraft:black_banner:1|mounthealth:4|bd:stable:5
combat/captainoftheguard|combat/captaintraining|k|6|-|minecraft:shield:27|blockattacksmultiplier:4|-
combat/captaintraining|combat/knighttraining|k|5|-|minecraft:shield:16|blockattacksmultiplier:3|bd:combatacademy:5
combat/cleave|combat/powerattack|k|4|-|minecraft:iron_block:8|meleedamageaddition:3|bd:guardtower:10
combat/coffee|combat/tactictraining|k|2|-|minecraft:golden_carrot:4|sleeplessmultiplier:1|-
combat/deadlyaim|combat/woundingshot|k|6|-|minecraft:flint:256|archerdamageaddition:5|-
combat/diamondskin|combat/steelarmor|k|6|-|minecraft:diamond:64|armordurabilitymultiplier:6|-
combat/dodge|combat/avoidance|k|2|s2|minecraft:leather:16|archerarmormultiplier:1|bd:smeltery:1
combat/druidpotion|combat/arrowuse|k|3|-|minecolonies:mistletoe:64|consumepotions:1|bd:barracks:3
combat/duelist|combat/riposte|k|4|-|minecraft:iron_ingot:64|meleearmormultiplier:3|bd:smeltery:3
combat/evade|combat/avoid|k|4|-|minecraft:emerald:16|fleeingspeedaddition:2|bd:guardtower:8
combat/evasion|combat/improveddodge|k|4|-|minecraft:leather:64|archerarmormultiplier:3|bd:smeltery:3
combat/exerciseregimen|combat/healthdiet|k|5|-|minecraft:golden_apple:4,minecraft:golden_carrot:4|mounthealth:3|bd:stable:4
combat/fear|combat/feint|k|4|-|minecraft:emerald:16|fleeingdamagemultiplier:2|bd:guardtower:8
combat/feint|combat/regeneration|k|3|-|minecraft:emerald:8|fleeingdamagemultiplier:1|bd:guardtower:4
combat/flee|combat/evade|k|5|-|minecraft:emerald:32|fleeingspeedaddition:3|bd:guardtower:12
combat/fullretreat|combat/retreat|k|6|-|minecraft:emerald:64|fleeingdamagemultiplier:4|-
combat/healthdiet|combat/ivermectin|k|4|-|minecraft:hay_block:2,minecraft:apple:8|mounthealth:2|bd:stable:3
combat/hotfoot|combat/flee|k|6|-|minecraft:emerald:64|fleeingspeedaddition:4|-
combat/improvedbarding|combat/barding|k|4|-|minecraft:iron_horse_armor:1,minecraft:gold_ingot:5|mountarmor:2|bd:stable:4,bd:blacksmith:4
combat/improvedbows|combat/tactictraining|k|2|s2|minecraft:iron_block:6|blockhutarchery:1|bd:barracks:3
combat/improveddodge|combat/dodge|k|3|-|minecraft:leather:32|archerarmormultiplier:2|bd:archery:1
combat/improvedevasion|combat/evasion|k|5|-|minecraft:diamond:16|archerarmormultiplier:4|bd:archery:5
combat/improvedleather|-|k|1|s4|minecraft:leather:32|armordurabilitymultiplier:1|sd:townhall:1
combat/improvedswords|combat/tactictraining|k|2|-|minecraft:iron_block:6|blockhutcombatacademy:1|bd:barracks:3
combat/ironarmor|combat/ironskin|k|4|-|minecraft:iron_ingot:32|armordurabilitymultiplier:4|sd:townhall:4
combat/ironskin|combat/boiledleather|k|3|-|minecraft:iron_ingot:16|armordurabilitymultiplier:3|sd:townhall:3
combat/ivermectin|combat/mountedcombat|k|3|-|minecraft:red_mushroom:8,minecraft:warped_fungus:8|mounthealth:1|bd:stable:2,bd:hospital:2
combat/knighttraining|combat/squiretraining|k|4|-|minecraft:shield:8|blockattacksmultiplier:2|bd:combatacademy:4
combat/masterbarding|combat/advancedbarding|k|6|-|minecraft:diamond_horse_armor:1,minecraft:netherite_ingot:5|mountarmor:4|bd:stable:5,bd:blacksmith:5
combat/masterbowman|combat/rapidshot|k|6|-|minecraft:bow:27|doublearrowsmultiplier:4|-
combat/masterswordsman|combat/provost|k|6|-|minecraft:diamond:64|meleearmormultiplier:5|-
combat/mightycleave|combat/cleave|k|5|-|minecraft:iron_block:16|meleedamageaddition:4|bd:barracks:5
combat/mountedcombat|combat/tactictraining|k|2|-|minecraft:saddle:1,minecraft:lead:1|blockhutstable:1|-
combat/multishot|combat/trickshot|k|4|-|minecraft:bow:9|doublearrowsmultiplier:2|bd:archery:4
combat/parry|combat/avoidance|k|2|-|minecraft:iron_ingot:16|meleearmormultiplier:1|bd:smeltery:1
combat/penetratingshot|combat/preciseshot|k|3|-|minecraft:flint:32|archerdamageaddition:2|bd:archery:3
combat/piercingshot|combat/penetratingshot|k|4|-|minecraft:flint:64|archerdamageaddition:3|bd:guardtower:10
combat/platearmor|combat/ironarmor|k|5|s2|minecraft:iron_ingot:32|platearmorunlock:1|sd:blacksmith:4
combat/powerattack|combat/quickdraw|k|3|-|minecraft:iron_block:4|meleedamageaddition:2|bd:combatacademy:3
combat/preciseshot|combat/accuracy|k|2|s2|minecraft:flint:16|archerdamageaddition:1|bd:barracks:3
combat/provost|combat/duelist|k|5|-|minecraft:diamond:16|meleearmormultiplier:4|bd:combatacademy:5
combat/quickdraw|combat/accuracy|k|2|-|minecraft:iron_block:2|meleedamageaddition:1|bd:barracks:3
combat/rapidshot|combat/multishot|k|5|-|minecraft:bow:18|doublearrowsmultiplier:3|bd:archery:5
combat/regeneration|combat/improvedleather|k|2|x,s2|minecraft:emerald:1|retreatunlock:1|bd:guardtower:2
combat/retreat|combat/fear|k|5|-|minecraft:emerald:32|fleeingdamagemultiplier:3|bd:guardtower:12
combat/riposte|combat/parry|k|3|-|minecraft:iron_ingot:32|meleearmormultiplier:2|bd:combatacademy:1
combat/savagestrike|combat/mightycleave|k|6|-|minecraft:iron_block:32|meleedamageaddition:5|-
combat/slicedanddecided|combat/improvedswords|k|3|-|minecraft:diamond_axe:1|huscarl:1|bd:combatacademy:3
combat/squiretraining|combat/improvedswords|k|3|-|minecraft:shield:4|blockattacksmultiplier:1|bd:combatacademy:3
combat/standard|combat/telescope|k|5|-|minecraft:emerald:32|standard:1|bd:barracks:4
combat/steelarmor|combat/ironarmor|k|5|s1|minecraft:iron_ingot:64|armordurabilitymultiplier:5|sd:townhall:5
combat/tactictraining|-|k|1|s2|minecraft:iron_block:3|blockhutbarracks:1|bd:guardtower:3
combat/taunt|-|k|1|s5|minecraft:rotten_flesh:8,minecraft:bone:8,minecraft:spider_eye:8|knighttauntmobsunlock:1|bd:guardtower:1
combat/telescope|combat/ironskin|k|4|-|minecraft:emerald:16|telescope:1|bd:barracks:3
combat/thathitthemark|combat/improvedbows|k|3|-|minecraft:crossbow:5|marksman:1|bd:archery:3
combat/trickshot|combat/improvedbows|k|3|-|minecraft:bow:5|doublearrowsmultiplier:1|bd:archery:3
combat/whirlwind|combat/arrowpierce|k|4|-|minecraft:redstone:64,minecraft:gold_ingot:64,minecraft:lapis_lazuli:128|whirlwindabilityunlock:1|bd:barracks:4
combat/woundingshot|combat/piercingshot|k|5|-|minecraft:flint:128|archerdamageaddition:4|bd:barracks:5
technology/ability|technology/hittingiron|t|2|-|minecraft:iron_ingot:64|blockplacespeedmultiplier:1|bd:miner:1
technology/alchemist|technology/opennether|t|3|-|minecraft:nether_wart:16|blockhutalchemist:1|-
technology/amazingveins|technology/richveins|t|5|-|#minecraft:gold_ores:64|moreoresmultiplier:4|bd:miner:5
technology/assistanthammers|technology/hittingiron|t|2|-|minecraft:iron_block:2|assistanthammerunlock:1|bd:blacksmith:1
technology/biodegradable|-|t|1|-|minecraft:bone_meal:64|blockhutcomposter:1|bd:farmer:3
technology/bonemeal|technology/biodegradable|t|2|s3|minecraft:wheat_seeds:64|farmingmultiplier:1|alt:farmer+composter:3
technology/buildermodes|technology/memoryaid|t|3|s4|minecraft:diamond_axe:1|buildermodeunlock:1|bd:builder:3
technology/capacity|technology/space|t|4|-|minecolonies:blockminecoloniesrack:32|minimumstockmultiplier:2|bd:miner:4
technology/cheatsheet|technology/memoryaid|t|3|s1|minecraft:paper:64|recipesmultiplier:2|bd:sawmill:2
technology/compost|technology/dung|t|4|-|minecraft:wheat_seeds:256|farmingmultiplier:3|alt:farmer+composter:5
technology/croprotation|technology/letitgrow|t|3|-|minecraft:sugar_cane:32,minecraft:cactus:32|plantationlarge:1|bd:plantation:3
technology/deeppockets|technology/cheatsheet|t|4|n,s2|minecraft:emerald:64|citizeninvslotsaddition:1|bd:library:4
technology/depthknowledge|technology/rockingroll|t|3|-|minecraft:deepslate:64|knowledgeofthedepthsunlock:1|bd:crusher:3
technology/diamondcoated|technology/steelbracing|t|6|-|minecraft:diamond:128|tooldurabilitymultiplier:5|-
technology/dung|technology/bonemeal|t|3|-|minecraft:wheat_seeds:128|farmingmultiplier:2|alt:farmer+composter:4
technology/enhanced_gates1|technology/whatyaneed|t|3|-|minecolonies:gate_wood:64,minecolonies:ancienttome:2,minecraft:iron_block:5|enhancesgatedurabilityaddition:1|-
technology/enhanced_gates2|technology/enhanced_gates1|t|4|-|minecolonies:gate_iron:64,minecolonies:ancienttome:2,minecraft:obsidian:32|enhancesgatedurabilityaddition:2|-
technology/exoticfruits|technology/letitgrow|t|3|-|minecraft:glow_berries:32|plantationexotic:1|bd:plantation:3
technology/fertilizer|technology/compost|t|5|-|minecraft:wheat_seeds:512|farmingmultiplier:4|bd:smeltery:3
technology/flowerpower|technology/biodegradable|t|2|-|minecolonies:compost:64|blockhutflorist:1|bd:composter:3
technology/fullstock|technology/capacity|t|5|-|minecolonies:blockminecoloniesrack:64|minimumstockmultiplier:3|bd:miner:5
technology/gargamel|technology/croprotation|t|4|-|minecraft:crimson_fungus:16,minecraft:warped_fungus:16|plantationnether:1|bd:plantation:3,bd:netherworker:3
technology/gildedhammer|technology/rockingroll|t|3|-|minecraft:gravel:64,minecraft:sand:64,minecraft:clay:64|crushing11unlock:1|bd:crusher:3
technology/goodveins|technology/veinminer|t|3|-|#minecraft:iron_ores:64|moreoresmultiplier:2|bd:miner:2
technology/greenrevolution|technology/biodegradable|t|2|s3|minecolonies:compost:32|greenrevolution:1|bd:farmer:4
technology/hardened|technology/strong|t|3|-|minecraft:diamond:16|tooldurabilitymultiplier:2|bd:blacksmith:2
technology/heavilyloaded|technology/loaded|t|6|n|minecraft:emerald:256|citizeninvslotsaddition:3|-
technology/heavymachinery|technology/redstonepowered|t|4|-|minecraft:redstone:512|blockbreakspeedmultiplier:3|-
technology/hittingiron|-|t|1|s7|minecraft:anvil:1|blockhutblacksmith:1|bd:miner:3
technology/honeypot|technology/rainbowheaven|t|3|-|minecraft:beehive:16|beekeep2unlock:1|bd:beekeeper:3
technology/hot|-|t|1|s6|minecraft:lava_bucket:4|blockhutsmeltery:1|bd:miner:2
technology/hotboots|technology/stringwork|t|3|-|minecraft:leather:32,minecraft:iron_ingot:16|minerfireresunlock:1|bd:fletcher:1
technology/isthisredstone|technology/hot|t|2|-|minecraft:redstone:128|blockbreakspeedmultiplier:1|-
technology/junglemaster|technology/letitgrow|t|3|-|minecraft:bamboo:16,minecraft:cocoa_beans:16,minecraft:vine:16|plantationjungle:1|bd:plantation:2
technology/knowtheend|technology/theflintstones|t|3|-|minecraft:chorus_fruit:64|knowledgeoftheendunlock:1|bd:baker:3
technology/letitgrow|technology/biodegradable|t|2|s2|minecolonies:compost:16|blockhutplantation:1|bd:farmer:3
technology/lightning|technology/whatisthisspeed|t|6|-|minecraft:redstone:2048|blockbreakspeedmultiplier:5|-
technology/loaded|technology/deeppockets|t|5|n|minecraft:emerald:128|citizeninvslotsaddition:2|bd:library:5
technology/madness|technology/seemsautomatic|t|6|-|minecraft:iron_ingot:1024|blockplacespeedmultiplier:5|-
technology/magiccompost|technology/fertilizer|t|6|-|minecraft:wheat_seeds:2048|farmingmultiplier:5|-
technology/memoryaid|technology/woodwork|t|2|s3|minecraft:paper:32|recipesmultiplier:1|bd:sawmill:1
technology/morescrolls|-|t|1|s3|minecraft:paper:64,minecolonies:ancienttome:1,minecraft:lapis_lazuli:64|morescrollsunlock:1|bd:enchanter:3
technology/motherlode|technology/amazingveins|t|6|-|#minecraft:diamond_ores:64|moreoresmultiplier:5|-
technology/netherlog|technology/opennether|t|3|s4|minecraft:ender_eye:16,minecolonies:ancienttome:1|netherexpeditionlog:1|bd:netherworker:1
technology/oceanheart|technology/alchemist|t|4|s4|minecraft:heart_of_the_sea:1|fishingtreasure:1|bd:fisherman:4
technology/opennether|technology/morescrolls|t|2|s1|minecraft:gilded_blackstone:3|blockhutnetherworker:1|-
technology/pavetheroad|technology/rockingroll|t|3|s2|#minecolonies:concrete:32|blockhutconcretemixer:1|bd:crusher:1
technology/podzolchance|technology/biodegradable|t|2|-|minecraft:podzol:8|podzolchancemultiplier:1|bd:composter:2
technology/podzolchance2|technology/podzolchance|t|3|-|minecraft:podzol:32|podzolchancemultiplier:2|bd:composter:3
technology/rainbowheaven|technology/biodegradable|t|2|-|minecraft:poppy:64|blockhutdyer:1|bd:composter:3
technology/rainman|technology/rtm|t|6|-|minecraft:salmon_bucket:27|workinginrainunlock:1|-
technology/recipebook|technology/cheatsheet|t|4|-|minecraft:paper:128|recipesmultiplier:3|bd:sawmill:3
technology/redstonepowered|technology/isthisredstone|t|3|-|minecraft:redstone:256|blockbreakspeedmultiplier:2|-
technology/reinforced|technology/hardened|t|4|-|minecraft:diamond:32|tooldurabilitymultiplier:3|bd:blacksmith:3
technology/richveins|technology/goodveins|t|4|-|#minecraft:gold_ores:32|moreoresmultiplier:3|bd:miner:4
technology/rockingroll|technology/stonecake|t|2|-|minecraft:stone:64|blockhutcrusher:1|bd:stonemason:1
technology/rtm|technology/recipebook|t|5|-|minecraft:paper:256|recipesmultiplier:4|bd:sawmill:4
technology/seemsautomatic|technology/tools|t|5|-|minecraft:iron_ingot:512|blockplacespeedmultiplier:4|bd:blacksmith:5
technology/sieving|technology/woodwork|t|2|s2|minecraft:string:64|blockhutsifter:1|bd:fisherman:3
technology/skilledbutcher|technology/biodegradable|t|2|-|minecraft:diamond_axe:1,minecraft:enchanted_book:1,minecraft:anvil:1|looting:1|bd:enchanter:2,bd:blacksmith:2
technology/skills|technology/ability|t|3|-|minecraft:iron_ingot:128|blockplacespeedmultiplier:2|bd:miner:2
technology/softshoes|-|t|1|s2|minecraft:white_wool:16,minecraft:feather:16|softshoesunlock:1|-
technology/space|technology/sieving|t|3|-|minecolonies:blockminecoloniesrack:16|minimumstockmultiplier:1|bd:miner:3
technology/steelbracing|technology/reinforced|t|5|-|minecraft:diamond:64|tooldurabilitymultiplier:4|bd:blacksmith:5
technology/stonecake|-|t|1|s4|minecraft:chiseled_stone_bricks:64|blockhutstonemason:1|bd:miner:3
technology/stringwork|technology/woodwork|t|2|-|minecraft:string:16|blockhutfletcher:1|bd:sawmill:1
technology/strong|technology/hittingiron|t|2|-|minecraft:diamond:8|tooldurabilitymultiplier:1|bd:blacksmith:1
technology/takingdivinglessons|technology/letitgrow|t|3|-|minecraft:kelp:16,minecraft:seagrass:16,minecraft:sea_pickle:16|plantationsea:1|bd:plantation:2
technology/theflintstones|technology/stonecake|t|2|-|minecraft:brick:64|blockhutstonesmeltery:1|bd:stonemason:1
technology/thoselungs|technology/hot|t|2|-|minecraft:glass:64|blockhutglassblower:1|bd:smeltery:3
technology/tools|technology/skills|t|4|-|minecraft:iron_ingot:256|blockplacespeedmultiplier:3|bd:blacksmith:4
technology/veinminer|technology/hittingiron|t|2|-|#minecraft:iron_ores:32|moreoresmultiplier:1|bd:miner:1
technology/warehousemaster|technology/memoryaid|t|3|s2|minecolonies:blockminecoloniesrack:3|recipemodeunlock:1|bd:warehouse:3,alt:sawmill+stonemason:3
technology/whatisthisspeed|technology/heavymachinery|t|5|-|minecraft:redstone:1024|blockbreakspeedmultiplier:4|-
technology/whatyaneed|technology/hittingiron|t|2|-|minecraft:redstone:64|blockhutmechanic:1|bd:blacksmith:3
technology/woodwork|-|t|1|s5|#minecraft:planks:64|blockhutsawmill:1|bd:lumberjack:3
unlockable/diamondmesh|unlockable/ironmesh|u|4|h,a,i|-|sifterdiamondunlock:1|sd:sifter:5
unlockable/flintmesh|unlockable/stringmesh|u|2|h,a,i|-|sifterflintunlock:1|sd:sifter:3
unlockable/ironmesh|unlockable/flintmesh|u|3|h,a,i|-|sifterironunlock:1|sd:sifter:4
unlockable/stringmesh|-|u|1|h,a,i|-|sifterstringunlock:1|sd:sifter:1
";

fn parse_row(line: &'static str, row: usize) -> Result<Research, LoadError> {
    let fields: Vec<&'static str> = line.split('|').collect();
    if fields.len() != 8 {
        return Err(LoadError::FieldCount {
            row,
            found: fields.len(),
        });
    }
    let id = required(fields[0], row, "id")?;
    let parent = optional(fields[1]);
    let branch = Branch::from_tag(fields[2]).ok_or_else(|| LoadError::UnknownBranch {
        row,
        tag: fields[2].to_string(),
    })?;
    let depth: u8 = fields[3].parse().map_err(|_| LoadError::BadDepth {
        row,
        value: fields[3].to_string(),
    })?;
    if depth == 0 || depth > MAX_DEPTH {
        return Err(LoadError::BadDepth {
            row,
            value: fields[3].to_string(),
        });
    }
    let (flags, sort) = parse_flags(fields[4], row)?;

    let mut costs = Vec::new();
    for spec in list(fields[5]) {
        costs.push(parse_cost(spec, row)?);
    }
    let mut effects = Vec::new();
    for spec in list(fields[6]) {
        effects.push(parse_effect(spec, row)?);
    }
    let mut requirements = Vec::new();
    for spec in list(fields[7]) {
        requirements.push(parse_requirement(spec, row)?);
    }

    Ok(Research {
        id,
        branch,
        parent,
        depth,
        sort,
        flags,
        costs,
        effects,
        requirements,
    })
}

/// A field that must be present and non-empty.
fn required(field: &'static str, row: usize, name: &'static str) -> Result<&'static str, LoadError> {
    if field.is_empty() {
        return Err(LoadError::MissingField { row, field: name });
    }
    Ok(field)
}

/// `-` is "absent"; anything else is the value.
fn optional(field: &'static str) -> Option<&'static str> {
    if field == "-" {
        None
    } else {
        Some(field)
    }
}

/// The entries of a comma list, `-` being an empty one.
fn list(field: &'static str) -> Vec<&'static str> {
    match optional(field) {
        Some(value) => value.split(',').collect(),
        None => Vec::new(),
    }
}

fn parse_flags(field: &'static str, row: usize) -> Result<(Flags, u16), LoadError> {
    let mut flags = Flags::NONE;
    let mut sort = 1000;
    for token in list(field) {
        if let Some(number) = token.strip_prefix('s') {
            sort = number.parse().map_err(|_| LoadError::UnknownFlag {
                row,
                tag: token.to_string(),
            })?;
            continue;
        }
        flags = match token {
            "h" => flags.with(Flags::HIDDEN),
            "a" => flags.with(Flags::AUTOSTART),
            "i" => flags.with(Flags::INSTANT),
            "n" => flags.with(Flags::IMMUTABLE),
            "x" => flags.with(Flags::EXCLUSIVE),
            _ => {
                return Err(LoadError::UnknownFlag {
                    row,
                    tag: token.to_string(),
                });
            }
        };
    }
    Ok((flags, sort))
}

fn parse_cost(spec: &'static str, row: usize) -> Result<Cost, LoadError> {
    // From the right: `minecraft:book:128` has two colons.
    let (head, count) = spec.rsplit_once(':').ok_or_else(|| LoadError::BadCost {
        row,
        spec: spec.to_string(),
    })?;
    let count: u32 = count.parse().map_err(|_| LoadError::BadCost {
        row,
        spec: spec.to_string(),
    })?;
    if count == 0 {
        return Err(LoadError::ZeroCost {
            row,
            spec: spec.to_string(),
        });
    }
    if let Some(tag) = head.strip_prefix('#') {
        return Ok(Cost::Tag {
            tag: required(tag, row, "cost tag")?,
            count,
        });
    }
    if let Some(inside) = head.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) {
        return Ok(Cost::Any {
            items: required(inside, row, "cost items")?,
            count,
        });
    }
    Ok(Cost::Item {
        item: required(head, row, "cost item")?,
        count,
    })
}

fn parse_effect(spec: &'static str, row: usize) -> Result<Unlock, LoadError> {
    let (effect, level) = spec.rsplit_once(':').ok_or_else(|| LoadError::BadEffect {
        row,
        spec: spec.to_string(),
    })?;
    let level: u8 = level.parse().map_err(|_| LoadError::BadEffect {
        row,
        spec: spec.to_string(),
    })?;
    let effect = required(effect, row, "effect")?;
    let category = super::effect::category(effect).ok_or_else(|| LoadError::UnknownEffect {
        row,
        effect: effect.to_string(),
    })?;
    match category.strength(level) {
        Ok(_) => Ok(Unlock { effect, level }),
        Err(LevelError::AboveMax { max, .. }) => Err(LoadError::EffectLevelAboveMax {
            row,
            effect: effect.to_string(),
            level,
            max,
        }),
    }
}

fn parse_requirement(spec: &'static str, row: usize) -> Result<Requirement, LoadError> {
    let bad = || LoadError::BadRequirement {
        row,
        spec: spec.to_string(),
    };
    if let Some(research) = spec.strip_prefix("rs:") {
        return Ok(Requirement::Research {
            research: required(research, row, "requirement research")?,
        });
    }
    let (head, level) = spec.rsplit_once(':').ok_or_else(|| bad())?;
    let level: i32 = level.parse().map_err(|_| bad())?;
    if let Some(building) = head.strip_prefix("bd:") {
        return Ok(Requirement::Building {
            building: required(building, row, "requirement building")?,
            level,
        });
    }
    if let Some(building) = head.strip_prefix("sd:") {
        return Ok(Requirement::SingleBuilding {
            building: required(building, row, "requirement building")?,
            level,
        });
    }
    if let Some(buildings) = head.strip_prefix("alt:") {
        return Ok(Requirement::AlternateBuilding {
            buildings: required(buildings, row, "requirement buildings")?,
            level,
        });
    }
    Err(bad())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Tree {
        Tree::load().expect("the shipped table loads")
    }

    #[test]
    fn the_table_loads_and_holds_217_rows() {
        assert_eq!(tree().len(), RESEARCH_COUNT);
        assert_eq!(TABLE.lines().count(), RESEARCH_COUNT);
    }

    #[test]
    fn every_row_has_eight_fields() {
        for (index, line) in TABLE.lines().enumerate() {
            assert_eq!(
                line.split('|').count(),
                8,
                "row {} is {line}",
                index + 1
            );
        }
    }

    #[test]
    fn a_root_has_no_parent_and_a_child_names_one() {
        let tree = tree();
        let academic = tree.get("civilian/academic").expect("declared");
        assert_eq!(academic.parent, Some("civilian/reflective"));
        assert!(!academic.is_root());
        let ambition = tree.get("civilian/ambition").expect("declared");
        assert_eq!(ambition.parent, None);
        assert!(ambition.is_root());
    }

    #[test]
    fn the_moq_research_is_technology_in_a_civilian_file() {
        let tree = tree();
        let moq = tree.get("civilian/moq").expect("declared");
        assert_eq!(moq.branch, Branch::Technology);
        assert_eq!(moq.parent, Some("technology/memoryaid"));
        assert_eq!(moq.depth, 3);
        assert_eq!(
            tree.get("technology/memoryaid").map(|row| row.depth),
            Some(2),
            "its parent is one level up, in the branch the field names"
        );
    }

    #[test]
    fn a_row_reads_its_four_parts() {
        let tree = tree();
        let academic = tree.get("civilian/academic").expect("declared");
        assert_eq!(academic.branch, Branch::Civilian);
        assert_eq!(academic.depth, 6);
        assert_eq!(
            academic.costs,
            vec![Cost::Item {
                item: "minecraft:book",
                count: 128
            }]
        );
        assert_eq!(
            academic.effects,
            vec![Unlock {
                effect: "levelingmultiplier",
                level: 5
            }]
        );
        assert!(academic.requirements.is_empty());
        assert_eq!(academic.ticks(), 2304);
        assert_eq!(academic.hours(), 16.0);
    }

    #[test]
    fn a_tag_cost_reads_from_the_right() {
        let tree = tree();
        let planks = tree.get("technology/woodwork").expect("declared");
        assert_eq!(
            planks.costs,
            vec![Cost::Tag {
                tag: "minecraft:planks",
                count: 64
            }]
        );
        assert_eq!(planks.requirements.len(), 1);
    }

    #[test]
    fn the_flat_flags_are_the_ones_the_files_set() {
        let tree = tree();
        let hidden = tree.all().filter(|row| row.flags.hidden()).count();
        let autostart = tree.all().filter(|row| row.flags.autostarts()).count();
        let instant = tree.all().filter(|row| row.flags.instant()).count();
        let immutable = tree.all().filter(|row| row.flags.immutable()).count();
        let exclusive = tree.all().filter(|row| row.flags.exclusive_child()).count();
        assert_eq!(
            (hidden, autostart, instant, immutable, exclusive),
            (4, 4, 4, 3, 5)
        );
        for id in [
            "unlockable/stringmesh",
            "unlockable/flintmesh",
            "unlockable/ironmesh",
            "unlockable/diamondmesh",
        ] {
            let row = tree.get(id).expect("declared");
            assert!(row.flags.hidden() && row.flags.autostarts() && row.flags.instant());
            assert!(
                row.costs.is_empty(),
                "an unlockable costs nothing, it autostarts"
            );
        }
    }

    #[test]
    fn a_sort_order_is_read_and_the_rest_default_to_1000() {
        let tree = tree();
        assert_eq!(tree.get("civilian/air").map(|row| row.sort), Some(1));
        assert_eq!(tree.get("technology/hittingiron").map(|row| row.sort), Some(7));
        // `combat/avoidance` carries both `x` and `s3`.
        let avoidance = tree.get("combat/avoidance").expect("declared");
        assert_eq!(avoidance.sort, 3);
        assert!(avoidance.flags.exclusive_child());
        // No sort order in the file: Java's default (`ResearchListener.java:283`).
        assert_eq!(tree.get("civilian/academic").map(|row| row.sort), Some(1000));
    }

    #[test]
    fn the_roots_are_19_and_come_back_sorted() {
        let tree = tree();
        let roots: Vec<&str> = tree.roots().map(|row| row.id).collect();
        assert_eq!(roots.len(), 19);
        let mut sorted = roots.clone();
        sorted.sort_unstable();
        assert_eq!(roots, sorted);
        assert!(roots.contains(&"unlockable/stringmesh"));
        assert!(roots.contains(&"technology/woodwork"));
    }

    #[test]
    fn children_are_direct_and_point_one_level_down() {
        let tree = tree();
        // `combat/accuracy.json:25` declares `"researchLevel": 1`, and the only two
        // files that name it as parent — `combat/preciseshot.json:17` and
        // `combat/quickdraw.json:17` — declare `"researchLevel": 2` (`:25`). One
        // level down from 1 is 2; a 3 could not even load, since `Tree::load`
        // refuses a parent more than one depth up (`:318-326`). The count of two
        // was already right: no other file in the source names this parent.
        assert_eq!(tree.get("combat/accuracy").map(|row| row.depth), Some(1));
        let children: Vec<&str> = tree.children("combat/accuracy").map(|row| row.id).collect();
        assert_eq!(children.len(), 2);
        assert!(children.contains(&"combat/preciseshot"));
        assert!(children.contains(&"combat/quickdraw"));
        for child in tree.children("combat/accuracy") {
            assert_eq!(child.depth, 2);
        }
        assert_eq!(tree.children("civilian/academic").count(), 0);
    }

    #[test]
    fn every_parent_has_at_least_one_child_and_every_child_one_parent() {
        let tree = tree();
        for row in tree.all() {
            if let Some(parent) = row.parent {
                assert!(
                    tree.get(parent).is_some(),
                    "{} hangs under {parent}",
                    row.id
                );
            }
            if !row.is_root() {
                let parent = row.parent.expect("a child names one");
                assert_eq!(
                    row.depth,
                    tree.get(parent).map(|row| row.depth).unwrap_or(0) + 1
                );
            }
        }
    }

    /// One effect is **a ladder of levels, one rung per research** — it is not
    /// granted by one research only. `minecolonies:effects/levelingmultiplier`
    /// has five granting files: `civilian/diligent.json` (level 1),
    /// `civilian/studious.json` (2), `civilian/scholarly.json` (3),
    /// `civilian/reflective.json` (4), `civilian/academic.json` (5);
    /// `healthaddition` and `armordurabilitymultiplier` have six each. Java has no
    /// uniqueness rule either: `GlobalResearchTree` keys the effect index by id
    /// and every research adds its own entry. What **is** unique in the data is
    /// the pair (effect, level): 217 rows, 106 effects, no duplicate rung.
    #[test]
    fn each_effect_level_has_exactly_one_granting_research_per_row() {
        let tree = tree();
        for row in tree.all() {
            assert_eq!(row.effects.len(), 1, "{} grants one effect", row.id);
            assert!(row.unlock().is_some());
            let unlock = row.unlock().expect("one effect");
            let rung = tree
                .of_effect(unlock.effect)
                .filter(|other| {
                    other
                        .unlock()
                        .is_some_and(|theirs| theirs.level == unlock.level)
                })
                .count();
            assert_eq!(
                rung, 1,
                "{} level {} is granted by {} only",
                unlock.effect, unlock.level, row.id
            );
        }
        // The same effect comes back on several rows, one level apart.
        assert_eq!(tree.of_effect("levelingmultiplier").count(), 5);
        assert_eq!(tree.of_effect("healthaddition").count(), 6);
    }

    #[test]
    fn the_three_requirement_readings_are_all_in_the_data() {
        let tree = tree();
        let mut building = 0;
        let mut single = 0;
        let mut alternate = 0;
        for row in tree.all() {
            for requirement in &row.requirements {
                match requirement {
                    Requirement::Building { .. } => building += 1,
                    Requirement::SingleBuilding { .. } => single += 1,
                    Requirement::AlternateBuilding { .. } => alternate += 1,
                    Requirement::Research { .. } => panic!("no row declares one"),
                }
            }
        }
        assert_eq!((building, single, alternate), (159, 23, 4));
    }

    #[test]
    fn the_cost_and_requirement_totals_are_the_files_own() {
        let tree = tree();
        let costs: usize = tree.all().map(|row| row.costs.len()).sum();
        let requirements: usize = tree.all().map(|row| row.requirements.len()).sum();
        let tags = tree
            .all()
            .flat_map(|row| row.costs.iter())
            .filter(|cost| matches!(**cost, Cost::Tag { .. }))
            .count();
        assert_eq!(costs, 247);
        assert_eq!(requirements, 186);
        assert_eq!(tags, 7, "seven tag costs, no list cost");
        assert_eq!(
            tree.all().filter(|row| row.costs.is_empty()).count(),
            4,
            "the four unlockables"
        );
        assert_eq!(
            tree.all().filter(|row| row.requirements.is_empty()).count(),
            39
        );
    }

    #[test]
    fn a_duplicate_id_is_refused() {
        let error = Tree::parse(
            "civilian/keen|-|c|1|-|minecraft:book:3|blockhutlibrary:1|-\n\
             civilian/keen|-|c|2|-|-|levelingmultiplier:1|-",
        )
        .expect_err("the same id twice");
        assert_eq!(
            error,
            LoadError::DuplicateId {
                id: "civilian/keen".to_string()
            }
        );
    }

    #[test]
    fn a_row_whose_parent_is_absent_is_refused_with_both_ids() {
        let error = Tree::parse("civilian/keen|civilian/nosuch|c|2|-|-|-|-")
            .expect_err("no such parent");
        assert_eq!(
            error,
            LoadError::MissingParent {
                id: "civilian/keen".to_string(),
                parent: "civilian/nosuch".to_string()
            }
        );
        assert_eq!(error.name(), "minecolonies:research/missing-parent");
        assert_eq!(
            error.to_string(),
            "minecolonies:research/missing-parent: civilian/keen hangs under civilian/nosuch, absent"
        );
    }

    #[test]
    fn a_parent_that_is_not_one_depth_up_is_refused() {
        let error = Tree::parse(
            "civilian/keen|-|c|1|-|-|blockhutlibrary:1|-\n\
             civilian/bookworm|civilian/keen|c|3|-|-|levelingmultiplier:1|-",
        )
        .expect_err("two levels down");
        assert_eq!(
            error,
            LoadError::DepthGap {
                id: "civilian/bookworm".to_string(),
                depth: 3,
                parent: "civilian/keen".to_string(),
                parent_depth: 1
            }
        );
        // One level down is the only shape that loads.
        assert!(Tree::parse(
            "civilian/keen|-|c|1|-|-|blockhutlibrary:1|-\n\
             civilian/bookworm|civilian/keen|c|2|-|-|levelingmultiplier:1|-",
        )
        .is_ok());
    }

    #[test]
    fn a_field_count_off_by_one_is_named_with_its_row() {
        let error = parse_row("civilian/x|-|c|1|-", 7).expect_err("five fields");
        assert_eq!(error, LoadError::FieldCount { row: 7, found: 5 });
        assert_eq!(error.name(), "minecolonies:research/field-count");
        assert_eq!(
            error.to_string(),
            "minecolonies:research/field-count: row 7 has 5 fields, not 8"
        );
    }

    #[test]
    fn a_bad_branch_a_bad_depth_and_a_bad_flag_are_named() {
        assert_eq!(
            parse_row("civilian/x|-|z|1|-|-|-|-", 1),
            Err(LoadError::UnknownBranch {
                row: 1,
                tag: "z".to_string()
            })
        );
        assert_eq!(
            parse_row("civilian/x|-|c|7|-|-|-|-", 1),
            Err(LoadError::BadDepth {
                row: 1,
                value: "7".to_string()
            })
        );
        assert_eq!(
            parse_row("civilian/x|-|c|0|-|-|-|-", 1),
            Err(LoadError::BadDepth {
                row: 1,
                value: "0".to_string()
            })
        );
        assert_eq!(
            parse_row("civilian/x|-|c|1|q|-|-|-", 1),
            Err(LoadError::UnknownFlag {
                row: 1,
                tag: "q".to_string()
            })
        );
    }

    #[test]
    fn a_cost_that_reads_wrongly_or_costs_nothing_is_refused() {
        assert_eq!(
            parse_row("civilian/x|-|c|1|-|minecraft:book|-|-", 1),
            Err(LoadError::BadCost {
                row: 1,
                spec: "minecraft:book".to_string()
            })
        );
        assert_eq!(
            parse_row("civilian/x|-|c|1|-|minecraft:book:0|-|-", 1),
            Err(LoadError::ZeroCost {
                row: 1,
                spec: "minecraft:book:0".to_string()
            })
        );
    }

    #[test]
    fn an_effect_no_category_declares_is_refused() {
        assert_eq!(
            parse_row("civilian/x|-|c|1|-|-|nosucheffect:1|-", 1),
            Err(LoadError::UnknownEffect {
                row: 1,
                effect: "nosucheffect".to_string()
            })
        );
    }

    #[test]
    fn an_effect_level_above_the_ladder_is_refused() {
        assert_eq!(
            parse_row("civilian/x|-|c|1|-|-|levelingmultiplier:6|-", 1),
            Err(LoadError::EffectLevelAboveMax {
                row: 1,
                effect: "levelingmultiplier".to_string(),
                level: 6,
                max: 5
            })
        );
    }

    #[test]
    fn the_four_requirement_forms_read() {
        assert_eq!(
            parse_requirement("bd:guardtower:10", 1),
            Ok(Requirement::Building {
                building: "guardtower",
                level: 10
            })
        );
        assert_eq!(
            parse_requirement("sd:sifter:5", 1),
            Ok(Requirement::SingleBuilding {
                building: "sifter",
                level: 5
            })
        );
        assert_eq!(
            parse_requirement("alt:sawmill+stonemason:3", 1),
            Ok(Requirement::AlternateBuilding {
                buildings: "sawmill+stonemason",
                level: 3
            })
        );
        assert_eq!(
            parse_requirement("rs:civilian/keen", 1),
            Ok(Requirement::Research {
                research: "civilian/keen"
            })
        );
        assert_eq!(
            parse_requirement("zz:sawmill:3", 1),
            Err(LoadError::BadRequirement {
                row: 1,
                spec: "zz:sawmill:3".to_string()
            })
        );
    }

    #[test]
    fn a_row_with_no_costs_effects_or_requirements_reads_as_empty() {
        let row = parse_row("civilian/x|-|c|1|-|-|-|-", 1).expect("a bare row");
        assert!(row.costs.is_empty());
        assert!(row.effects.is_empty());
        assert!(row.requirements.is_empty());
        assert_eq!(row.unlock(), None);
        assert_eq!(row.sort, 1000);
    }

    #[test]
    fn an_empty_table_loads_as_an_empty_tree() {
        // The same reader Java uses, on nothing at all: not an error.
        let tree = Tree::parse("").expect("an empty table");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.get("civilian/academic"), None);
        assert_eq!(tree.roots().count(), 0);
        assert_eq!(tree.children("civilian/keen").count(), 0);
        assert_eq!(tree.of_effect("air").count(), 0);
    }

    #[test]
    fn the_four_branch_and_effect_counts_are_consistent_with_the_table() {
        let tree = tree();
        assert_eq!(super::super::BRANCH_COUNT, 4);
        assert_eq!(super::super::EFFECT_COUNT, 106);
        assert_eq!(tree.len(), RESEARCH_COUNT);
    }
}
