//! When a colony tries for a child, why it is refused, and **the order of its
//! writes**.
//!
//! Java reference: `core/colony/managers/ReproductionManager.java` (266 lines), the
//! constants it obeys (`TickRateConstants.java:11`, `ServerConfiguration.java:125`),
//! the two things it asks the colony for (`RegisteredStructureManager.java:493-512`,
//! `LivingBuildingModule.java:103-106`) and the two it delegates
//! (`CitizenData.java:685-740`, `CitizenSkillHandler.java:69-113`).
//!
//! | Question | Answer | Java |
//! | --- | --- | --- |
//! | how long before the first try | `6000 + roll(12000)` ticks | `:38`, `:43`, `:72` |
//! | how often it tries again | the same, **times the colony's fill ratio** | `:80` |
//! | when a child is refused | five refusals, in one order | `:92-102` |
//! | who the parents are | one resident, then his partner, then a neighbour within 50 | `:122-155` |
//! | which suffix the child gets | his parents', or all four | `:166-205` |
//! | what a birth writes | fifteen writes, in one order | `:159-217` |
//!
//! **The gate is one `if` and two early returns in Java; here it is five named
//! refusals.** `:92` is a single condition of three clauses and `:94`/`:99` are two
//! `if (…) return;` — so a colony refused for *both* reasons never learns the
//! second. [`Gate::admit`] answers the first that applies, in Java's order, and
//! names it: `MoveInOff`, `AtCapacity`, `BelowMinimum`, `NoBioParents`,
//! `NoSpareBed`.
//!
//! **`checkForBioParents` carries its two flags across the two lists.** `:228-264`
//! scans the citizens and then the *visitors* with the same `hasMale`/`hasFemale`,
//! so a colony whose only woman is a citizen and whose only man is a visitor does
//! reproduce. [`Gate`] holds the two flags rather than the two lists, which is what
//! keeps that rule visible: the caller ORs them, and a test pins the case.
//!
//! **`getAssignedCitizen()` returns a copy, and that is why `:107` is safe.** The
//! module hands out `new ArrayList<>(assignedCitizen)`
//! (`AbstractAssignedCitizenModule.java:90-93`), so `removeIf(ICitizen::isChild)`
//! filters the *local* list and the house keeps its children. [`choose_parents`]
//! takes `&[Candidate]` and therefore cannot mutate anything — the same behaviour,
//! by construction rather than by hope.
//!
//! **One rule of Java's is a bug, and the port keeps it.** The timer reset at `:80`
//! is `(6000 + roll) * (citizens / max(4, maxCitizens))` in **integer** arithmetic:
//! a colony with fewer citizens than `max(4, maxCitizens)` multiplies by a quotient
//! of **0**, so the timer resets to 0 and the colony tries again on *every* colony
//! tick (the interval is 500 game ticks, `TickRateConstants.java:11`) until it
//! grows. [`reset_timer`] reproduces that and a test says so instead of clamping it
//! away.
//!
//! **No clock, no random, no world.** The two draws Java makes (an index and a
//! coin) come in through [`Draws`]; the day comes in through [`Situation::day`],
//! like `crate::advancements::stats` takes it. [`Plan`] is the whole write set and
//! [`Plan::apply`] is the only thing that performs it, which is what makes
//! "nothing is written before a refusal" a test rather than a claim.

/// Ticks before a colony's first attempt, `20 * 60 * 5`
/// (`ReproductionManager.java:38`).
pub const MIN_TIME_BEFORE_SPAWNTRY: i32 = 20 * 60 * 5;

/// The width of the random part of that wait, `20 * 60 * 10`
/// (`ReproductionManager.java:43`).
pub const CHILD_SPAWN_INTERVAL: i32 = 20 * 60 * 10;

/// The fewest citizens a colony needs, before the config is considered
/// (`ReproductionManager.java:48`).
pub const MIN_SIZE_FOR_REPRO: i32 = 2;

/// One colony tick, and the amount the timer loses per tick
/// (`TickRateConstants.java:11`: `MAX_TICKRATE`).
pub const MAX_TICKRATE: i32 = 500;

/// `ServerConfiguration.java:125`: `initialcitizenamount`, default **4**, range
/// 1..10. Only its default is here; the value is read from the config by the
/// caller and passed in [`Gate::initial_citizen_amount`].
pub const DEFAULT_INITIAL_CITIZEN_AMOUNT: i32 = 4;

/// The floor of the divisor in the timer reset, `Math.max(4, …)`
/// (`ReproductionManager.java:80`).
pub const MIN_HOUSING_DIVISOR: i32 = 4;

/// How far a neighbour's house may be for a second parent to come from it,
/// `getDistance2D(…) < 50` (`ReproductionManager.java:135`).
pub const ALT_HOUSE_REACH: i32 = 50;

/// The floor of the child's bonus skill points, `25 + rand.nextInt(25)`
/// (`CitizenSkillHandler.java:95`).
pub const BONUS_FLOOR: i32 = 25;

/// How many skills a child inherits (`Skill.java:10-150`, eleven constants).
pub const SKILL_COUNT: usize = 11;

/// The four texture suffixes, in the order `SUFFIXES` declares them
/// (`CitizenData.java:98`). Java never says what the letters mean; the port keeps
/// them as the strings the game's textures are named with.
pub const SUFFIXES: [&str; 4] = ["_b", "_d", "_a", "_w"];

/// `_w` and `_d` together earn a third suffix, `_b` (`:195-198`).
pub const SUFFIX_W: &str = "_w";
/// The other half of that pair (`:195-198`).
pub const SUFFIX_D: &str = "_d";
/// The suffix the pair adds (`:197`).
pub const SUFFIX_BOTH: &str = "_b";

/// The message a colony gets for its first child
/// (`TranslationConstants.java:608`).
pub const MESSAGE_NEW_CHILD_BORN: &str = "com.minecolonies.coremod.progress.newchild";

/// Why a colony did not get a child this time, named.
///
/// A variant is not a `lang` key: those live in `assets/minecolonies/lang/`, which
/// this module does not own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// `!getSetting(MOVE_IN).getValue()` — the town hall's `kidspawn` switch is
    /// off (`ReproductionManager.java:92`, `BuildingTownHall.java:64`).
    MoveInOff,
    /// `getCurrentCitizenCount() >= getMaxCitizens()` (`:92`). The two numbers are
    /// carried because the message Java never sends would need both.
    AtCapacity { citizens: i32, max: i32 },
    /// `getCurrentCitizenCount() < Math.min(2, initialCitizenAmount)` (`:92`).
    BelowMinimum { citizens: i32, needed: i32 },
    /// No man and a woman between the citizens and the visitors (`:94`, `:226-265`).
    NoBioParents,
    /// `getHouseWithSpareBed()` answered `null`: no living module with a free spot
    /// whose hiring is not locked (`:99`, `RegisteredStructureManager.java:493-512`).
    NoSpareBed,
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::MoveInOff => "minecolonies:reproduction/move-in-off",
            Refusal::AtCapacity { .. } => "minecolonies:reproduction/at-capacity",
            Refusal::BelowMinimum { .. } => "minecolonies:reproduction/below-minimum",
            Refusal::NoBioParents => "minecolonies:reproduction/no-bio-parents",
            Refusal::NoSpareBed => "minecolonies:reproduction/no-spare-bed",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Everything the gate reads (`ReproductionManager.java:92-102`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the seven facts the gate needs
pub struct Gate {
    /// `BuildingTownHall.MOVE_IN` (`:92`).
    pub move_in: bool,
    /// `getCitizenManager().getCurrentCitizenCount()` (`:92`).
    pub citizens: i32,
    /// `getCitizenManager().getMaxCitizens()`, already clamped by that getter
    /// (`CitizenManager.java:503-506`); the port does not clamp it a second time.
    pub max_citizens: i32,
    /// `ServerConfiguration.initialCitizenAmount` (`:92`).
    pub initial_citizen_amount: i32,
    /// Some citizen, or some visitor, is not a woman (`:230-240`, `:248-258`).
    pub has_male: bool,
    /// Some citizen, or some visitor, is a woman (`:233-237`, `:250-254`).
    pub has_female: bool,
    /// `getHouseWithSpareBed()` (`:99`): the position of the house the child is
    /// born into, `None` when there is none.
    pub spare_bed_house: Option<[i32; 3]>,
}

impl Gate {
    /// The count a colony must reach: `Math.min(MIN_SIZE_FOR_REPRO, initial)`
    /// (`:92`). With the shipped config (`initial` 4) it is 2; a server configured
    /// with `initialcitizenamount: 1` asks for **one** citizen, not two.
    pub fn minimum(&self) -> i32 {
        self.initial_citizen_amount.min(MIN_SIZE_FOR_REPRO)
    }

    /// The gate, in the order Java evaluates it: one condition of three clauses at
    /// `:92`, then `checkForBioParents` at `:94`, then the house at `:99`.
    pub fn admit(&self) -> Result<[i32; 3], Refusal> {
        if !self.move_in {
            return Err(Refusal::MoveInOff);
        }
        if self.citizens >= self.max_citizens {
            return Err(Refusal::AtCapacity {
                citizens: self.citizens,
                max: self.max_citizens,
            });
        }
        if self.citizens < self.minimum() {
            return Err(Refusal::BelowMinimum {
                citizens: self.citizens,
                needed: self.minimum(),
            });
        }
        if !(self.has_male && self.has_female) {
            return Err(Refusal::NoBioParents);
        }
        self.spare_bed_house.ok_or(Refusal::NoSpareBed)
    }
}

/// The timer that decides *when* a colony tries (`ReproductionManager.java:53`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Clock {
    timer: i32,
}

impl Clock {
    /// The constructor's own line (`:72`): `random.nextInt(12000) + 6000`. `draw`
    /// is that roll, which the caller makes.
    pub fn new(draw: i32) -> Clock {
        Clock {
            timer: MIN_TIME_BEFORE_SPAWNTRY + draw,
        }
    }

    /// A timer a caller already holds.
    pub fn from_timer(timer: i32) -> Clock {
        Clock { timer }
    }

    pub fn timer(&self) -> i32 {
        self.timer
    }

    /// One colony tick (`:76-83`): subtract [`MAX_TICKRATE`], and when that leaves
    /// nothing, reset and answer `true` — the caller then runs [`decide`].
    ///
    /// **The subtraction happens first and the comparison second** (`:78`), so a
    /// timer of exactly 500 fires on this tick, not the next.
    pub fn on_tick(&mut self, citizens: i32, max_citizens: i32, draw: i32) -> bool {
        self.timer -= MAX_TICKRATE;
        if self.timer > 0 {
            return false;
        }
        self.timer = reset_timer(draw, citizens, max_citizens);
        true
    }
}

/// The timer after an attempt (`:80`), in Java's integer arithmetic.
///
/// `(6000 + draw) * (citizens / max(4, maxCitizens))`. The division is integer
/// division, so a colony with fewer citizens than `max(4, maxCitizens)` gets a
/// **zero** timer and tries again on every colony tick — see the module note.
pub fn reset_timer(draw: i32, citizens: i32, max_citizens: i32) -> i32 {
    let divisor = max_citizens.max(MIN_HOUSING_DIVISOR);
    let share = citizens / divisor;
    (MIN_TIME_BEFORE_SPAWNTRY + draw) * share
}

/// One resident of a house, with the family facts `ICitizenData.isRelatedTo`
/// reads (`CitizenData.java:791-795`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Candidate {
    pub id: i32,
    pub name: String,
    /// `isChild()` (`:107`, `:142`).
    pub is_child: bool,
    /// `getPartner()`, the id — or `None` (`:125`).
    pub partner: Option<i32>,
    /// `getSiblings()` — ids (`:793`).
    pub siblings: Vec<i32>,
    /// `getChildren()` — ids (`:793`).
    pub children: Vec<i32>,
    /// `getParents()` — the two **names**, not ids (`:793-794`).
    pub parents: [String; 2],
    /// `getTextureSuffix()` (`:171`, `:178`).
    pub suffix: String,
}

impl Candidate {
    /// `ICitizenData.isRelatedTo(data)` (`CitizenData.java:791-795`), **as Java
    /// wrote it**: it looks at *this* record's siblings, children, partner and
    /// parents' names, never at the other one's. The asymmetry is real — a citizen
    /// whose parents are named in `data` is not related to `data` by this test —
    /// and a test pins it rather than smoothing it over.
    pub fn related_to(&self, other: &Candidate) -> bool {
        self.siblings.contains(&other.id)
            || self.children.contains(&other.id)
            || self.partner == Some(other.id)
            || self.parents[0] == other.name
            || self.parents[1] == other.name
    }
}

/// A house and who lives in it (`LivingBuildingModule`'s assigned list).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Household {
    pub at: [i32; 3],
    pub residents: Vec<Candidate>,
}

/// The two draws Java makes, made by the caller: this module has no RNG.
pub trait Draws {
    /// `random.nextInt(len)`, called with `len > 0`.
    fn index(&mut self, len: usize) -> usize;
    /// `random.nextBoolean()`.
    fn coin(&mut self) -> bool;
}

/// A draw that came back out of range is clamped to the first element rather than
/// panicking: a bad caller picks the first candidate, which is a wrong parent and
/// not a crashed colony.
fn pick(draws: &mut dyn Draws, len: usize) -> usize {
    let at = draws.index(len);
    if at < len { at } else { 0 }
}

/// Who the child's parents are (`:122-155`), and which house the second came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Chosen {
    pub first: Option<i32>,
    pub second: Option<i32>,
    /// The position of the *other* house the second parent came from, when he did
    /// not live with the first (`:135-147`).
    pub second_house: Option<[i32; 3]>,
}

/// The parent choice, in Java's order (`:122-155`).
///
/// 1. the house's residents without the children (`:107`);
/// 2. one of them, drawn (`:124`);
/// 3. his partner if one is known (`:125`) — **no draw at all** in that case;
/// 4. otherwise a resident with no partner, a different name and no family tie
///    (`:128`), drawn only if a coin says so (`:129`);
/// 5. otherwise another house within [`ALT_HOUSE_REACH`] (`:135`), whose residents
///    are filtered for child, partner and family tie (`:142`), and one drawn
///    (`:145`).
///
/// The house at step 5 is a *random* one among the matching buildings in Java
/// (`getRandomBuilding`), so the caller passes the matching houses and this
/// function draws which of them.
pub fn choose_parents(home: &Household, alt: &[Household], draws: &mut dyn Draws) -> Chosen {
    let adults: Vec<&Candidate> = home.residents.iter().filter(|who| !who.is_child).collect();
    if adults.is_empty() {
        // `:151-155`: no adult in the new house, so no first parent at all.
        return Chosen::default();
    }
    let first = adults[pick(draws, adults.len())];
    if let Some(partner) = first.partner {
        return Chosen {
            first: Some(first.id),
            second: Some(partner),
            second_house: None,
        };
    }
    let pool: Vec<&Candidate> = adults
        .iter()
        .copied()
        .filter(|who| {
            who.partner.is_none()
                && who.name != first.name
                && !who.related_to(first)
                && !who.is_child
        })
        .collect();
    // `:129` short-circuits: the coin is not drawn when the pool is empty.
    if !pool.is_empty() && draws.coin() {
        let second = pool[pick(draws, pool.len())];
        return Chosen {
            first: Some(first.id),
            second: Some(second.id),
            second_house: None,
        };
    }
    if alt.is_empty() {
        return Chosen {
            first: Some(first.id),
            second: None,
            second_house: None,
        };
    }
    let house = &alt[pick(draws, alt.len())];
    let others: Vec<&Candidate> = house
        .residents
        .iter()
        .filter(|who| !who.is_child && who.partner.is_none() && !who.related_to(first))
        .collect();
    let second = if others.is_empty() {
        None
    } else {
        Some(others[pick(draws, others.len())].id)
    };
    Chosen {
        first: Some(first.id),
        second,
        second_house: Some(house.at),
    }
}

/// The suffixes the child may take (`:166-205`).
///
/// The pool is the parents' own strings — Java copies them without checking them
/// against [`SUFFIXES`] (`:171`, `:178`), so a save that carries `_z` gives `_z` —
/// plus `_b` when both `_w` and `_d` are present (`:195-198`), and the four
/// constants when there is no parent at all (`:200-203`).
pub fn suffixes(first: Option<&str>, second: Option<&str>) -> Vec<String> {
    let mut pool: Vec<String> = Vec::new();
    for suffix in [first, second].into_iter().flatten() {
        pool.push(suffix.to_owned());
    }
    if pool.iter().any(|suffix| suffix == SUFFIX_W)
        && pool.iter().any(|suffix| suffix == SUFFIX_D)
    {
        pool.push(SUFFIX_BOTH.to_owned());
    }
    if pool.is_empty() {
        pool.extend(SUFFIXES.iter().map(|suffix| (*suffix).to_owned()));
    }
    pool
}

/// What a child takes from his two role models (`CitizenSkillHandler.java:69-113`).
///
/// `base` is the eleven rolls of `init(levelCap)` (`:49-66`: `random.nextInt(cap - 1)
/// + 1`, or 1 everywhere when the cap is 1 or less). `models` are the parents, or
/// the two *random* citizens Java substitutes for a missing parent (`:74-90`) — a
/// draw, so the caller picks them. Every skill then grows by its share of the two
/// models' points, out of `bonus` (`:95-112`).
///
/// **A zero total is not a division by zero.** Java divides `(a + b) / total` in
/// `double`, so `0.0 / 0.0` is `NaN` and `(int) (NaN * bonus)` is **0**: a colony
/// whose two role models have no points gives nothing, without an exception. The
/// port says that outright instead of relying on a NaN to cast itself away.
pub fn inherit_skills(
    base: [i32; SKILL_COUNT],
    models: [&[i32; SKILL_COUNT]; 2],
    bonus: i32,
) -> [i32; SKILL_COUNT] {
    let total: i32 = models
        .iter()
        .flat_map(|model| model.iter())
        .map(|level| (*level).max(0))
        .sum();
    let mut skills = base;
    if total == 0 {
        return skills;
    }
    for (skill, slot) in skills.iter_mut().enumerate() {
        let pair = models[0][skill].max(0) + models[1][skill].max(0);
        // `(int)` of the double Java computes, kept as integer arithmetic.
        *slot += (pair * bonus) / total;
    }
    skills
}

/// One write of a birth, in the order `ReproductionManager.java:159-217` performs
/// them. Whoever holds the world performs them; nothing here touches it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the writes of trySpawnChild
pub enum Write {
    /// `firstParent.setPartner(second.getId())` and the mirror (`:159-160`).
    SetPartner { of: i32, to: i32 },
    /// `getCitizenSkillHandler().init(colony, first, second, random)` (`:163`).
    InheritSkills {
        first: Option<i32>,
        second: Option<i32>,
        happiness: i32,
    },
    /// `setIsChild(true)` (`:164`).
    MarkChild { citizen: i32 },
    /// `newCitizen.addSiblings(first.getChildren())` (`:169`): `count` ids copied
    /// from the parent's own list.
    InheritSiblings {
        citizen: i32,
        from: i32,
        count: usize,
    },
    /// `firstParent.addChildren(newCitizen.getId())` (`:170`).
    AddChild { parent: i32, child: i32 },
    /// `setParents(firstName, secondName)` (`:181`).
    SetParents { first: String, second: String },
    /// `generateName(random, first, second, citizenNameFile)` (`:182`).
    GenerateName { first: String, second: String },
    /// `module.assignCitizen(newCitizen)` (`:184`), which sets the home building and
    /// recalculates the colony's cap (`LivingBuildingModule.java:89-93`).
    Assign { citizen: i32, house: [i32; 3] },
    /// `siblingData.addSiblings(newCitizen.getId())` for each of the child's
    /// siblings (`:186-193`).
    LinkSibling { sibling: i32, citizen: i32 },
    /// `setSuffix(possibleSuffixes.get(random.nextInt(…)))` (`:195-205`).
    SetSuffix { suffix: String },
    /// `COLONY_POPULATION.trigger(player, populationCount)` (`:207-208`).
    TriggerPopulation { count: i32 },
    /// `spawnOrCreateCitizen(newCitizen, world, newHome.getPosition())` (`:210`).
    Spawn { citizen: i32, at: [i32; 3] },
    /// `MESSAGE_NEW_CHILD_BORN`, only when this is the colony's first child
    /// (`:211-214`).
    AnnounceBirth { citizen: i32 },
    /// `CitizenBornEvent(newHome.getPosition(), name)` (`:216`).
    DescribeBirth { citizen: i32, at: [i32; 3] },
    /// `statistics.increment(BIRTH, colony.getDay())` (`:217`).
    CountBirth { day: i32 },
}

impl Write {
    /// A write's short name, for a journal a test reads.
    pub fn name(&self) -> &'static str {
        match self {
            Write::SetPartner { .. } => "set-partner",
            Write::InheritSkills { .. } => "inherit-skills",
            Write::MarkChild { .. } => "mark-child",
            Write::InheritSiblings { .. } => "inherit-siblings",
            Write::AddChild { .. } => "add-child",
            Write::SetParents { .. } => "set-parents",
            Write::GenerateName { .. } => "generate-name",
            Write::Assign { .. } => "assign",
            Write::LinkSibling { .. } => "link-sibling",
            Write::SetSuffix { .. } => "set-suffix",
            Write::TriggerPopulation { .. } => "trigger-population",
            Write::Spawn { .. } => "spawn",
            Write::AnnounceBirth { .. } => "announce-birth",
            Write::DescribeBirth { .. } => "describe-birth",
            Write::CountBirth { .. } => "count-birth",
        }
    }
}

/// Where a plan's writes go. The only thing in this module that touches the world.
pub trait Sink {
    fn write(&mut self, write: &Write);
}

/// The writes of one birth, in order. Only [`decide`] can build one, and it builds
/// one only after every refusal has been answered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Plan {
    writes: Vec<Write>,
}

impl Plan {
    pub fn writes(&self) -> &[Write] {
        &self.writes
    }

    pub fn len(&self) -> usize {
        self.writes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    /// Performs every write, in order.
    pub fn apply(&self, sink: &mut dyn Sink) {
        for write in &self.writes {
            sink.write(write);
        }
    }
}

/// Everything one attempt reads (`ReproductionManager.java:89-217`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the ten facts of one attempt
pub struct Situation {
    pub gate: Gate,
    /// The house the child is born into: [`Gate::spare_bed_house`].
    pub home: Household,
    /// The other living houses within [`ALT_HOUSE_REACH`], in any order — Java
    /// draws one of them (`:135`).
    pub alt: Vec<Household>,
    /// The rest of the colony's adults. `getPartner()` is a colony-wide lookup, so
    /// a first parent's partner may live in neither house (`:125`).
    pub elsewhere: Vec<Candidate>,
    /// The id the new citizen already has: Java registers it at `:119`, before any
    /// of the writes below.
    pub citizen: i32,
    /// `colony.getDay()` (`:217`).
    pub day: i32,
    /// `getCurrentCitizenCount()` **after** the registration at `:119`, which is
    /// what `:207` reads and why the newborn is counted.
    pub population: i32,
    /// Whether the colony has no child yet, computed at `:109-117` — before the
    /// newborn exists, and used at `:211`.
    pub only_child: bool,
    /// `(int) colony.getOverallHappiness()`, the cap of the skills' first roll
    /// (`CitizenSkillHandler.java:92`).
    pub happiness: i32,
}

/// One candidate, wherever he lives — Java looks partners up on the whole colony
/// (`:125`), not on a house.
fn resident(situation: &Situation, id: i32) -> Option<&Candidate> {
    situation
        .home
        .residents
        .iter()
        .chain(situation.alt.iter().flat_map(|house| house.residents.iter()))
        .chain(situation.elsewhere.iter())
        .find(|who| who.id == id)
}

/// One attempt: the gate, then the parents, then the writes — in Java's order.
///
/// Two draws at most for the parents ([`choose_parents`]) and one for the suffix,
/// so a caller with a seeded RNG gets the same child Java would.
pub fn decide(situation: &Situation, draws: &mut dyn Draws) -> Result<Plan, Refusal> {
    let house = situation.gate.admit()?;
    let chosen = choose_parents(&situation.home, &situation.alt, draws);
    let first = chosen.first.and_then(|id| resident(situation, id));
    let second = chosen.second.and_then(|id| resident(situation, id));

    let mut writes = Vec::new();
    // `:159-161` — both sides set, and only when there are two.
    if let (Some(one), Some(two)) = (chosen.first, chosen.second) {
        writes.push(Write::SetPartner { of: one, to: two });
        writes.push(Write::SetPartner { of: two, to: one });
    }
    // `:163-164`.
    writes.push(Write::InheritSkills {
        first: chosen.first,
        second: chosen.second,
        happiness: situation.happiness,
    });
    writes.push(Write::MarkChild {
        citizen: situation.citizen,
    });

    // `:166-179`, first parent then second: her children become the newborn's
    // siblings, and he is added to hers.
    let mut siblings: Vec<i32> = Vec::new();
    for parent in [first, second].into_iter().flatten() {
        writes.push(Write::InheritSiblings {
            citizen: situation.citizen,
            from: parent.id,
            count: parent.children.len(),
        });
        writes.push(Write::AddChild {
            parent: parent.id,
            child: situation.citizen,
        });
        siblings.extend(parent.children.iter().copied());
    }

    // `:181-184`.
    let first_name = first.map(|who| who.name.clone()).unwrap_or_default();
    let second_name = second.map(|who| who.name.clone()).unwrap_or_default();
    writes.push(Write::SetParents {
        first: first_name.clone(),
        second: second_name.clone(),
    });
    writes.push(Write::GenerateName {
        first: first_name,
        second: second_name,
    });
    writes.push(Write::Assign {
        citizen: situation.citizen,
        house,
    });

    // `:186-193`: the siblings learn the newborn's id, in the order they were
    // collected — Java does not de-duplicate, and a shared child of both parents
    // therefore appears twice.
    for sibling in &siblings {
        writes.push(Write::LinkSibling {
            sibling: *sibling,
            citizen: situation.citizen,
        });
    }

    // `:195-205`.
    let pool = suffixes(
        first.map(|who| who.suffix.as_str()),
        second.map(|who| who.suffix.as_str()),
    );
    writes.push(Write::SetSuffix {
        suffix: pool[pick(draws, pool.len())].clone(),
    });

    // `:207-217`.
    writes.push(Write::TriggerPopulation {
        count: situation.population,
    });
    writes.push(Write::Spawn {
        citizen: situation.citizen,
        at: house,
    });
    if situation.only_child {
        writes.push(Write::AnnounceBirth {
            citizen: situation.citizen,
        });
    }
    writes.push(Write::DescribeBirth {
        citizen: situation.citizen,
        at: house,
    });
    writes.push(Write::CountBirth {
        day: situation.day,
    });

    Ok(Plan { writes })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A draw script: indices and coins, in the order the caller asks for them.
    /// Running out of indices answers 0, and the assertion inside says when an
    /// index was asked for that is not below the length.
    #[derive(Default)]
    struct Script {
        indices: Vec<usize>,
        coins: Vec<bool>,
        coins_drawn: usize,
    }

    impl Script {
        fn new(indices: &[usize], coins: &[bool]) -> Script {
            Script {
                indices: indices.to_vec(),
                coins: coins.to_vec(),
                coins_drawn: 0,
            }
        }

        fn coin_called(&self) -> bool {
            self.coins_drawn > 0
        }
    }

    impl Draws for Script {
        fn index(&mut self, len: usize) -> usize {
            let next = if self.indices.is_empty() {
                0
            } else {
                self.indices.remove(0)
            };
            assert!(next < len, "a drawn index {next} is not below {len}");
            next
        }

        fn coin(&mut self) -> bool {
            self.coins_drawn += 1;
            if self.coins.is_empty() {
                false
            } else {
                self.coins.remove(0)
            }
        }
    }

    /// A journal: what a plan actually performed, in order.
    #[derive(Default)]
    struct Journal(Vec<&'static str>);

    impl Sink for Journal {
        fn write(&mut self, write: &Write) {
            self.0.push(write.name());
        }
    }

    fn candidate(id: i32, name: &str) -> Candidate {
        Candidate {
            id,
            name: name.to_owned(),
            suffix: SUFFIXES[id as usize % SUFFIXES.len()].to_owned(),
            ..Candidate::default()
        }
    }

    fn allowed() -> Gate {
        Gate {
            move_in: true,
            citizens: 4,
            max_citizens: 20,
            initial_citizen_amount: DEFAULT_INITIAL_CITIZEN_AMOUNT,
            has_male: true,
            has_female: true,
            spare_bed_house: Some([10, 70, 20]),
        }
    }

    fn house(residents: Vec<Candidate>) -> Household {
        Household {
            at: [10, 70, 20],
            residents,
        }
    }

    fn situation(home: Household) -> Situation {
        Situation {
            gate: allowed(),
            home,
            alt: Vec::new(),
            elsewhere: Vec::new(),
            citizen: 9,
            day: 12,
            population: 5,
            only_child: true,
            happiness: 5,
        }
    }

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    /// The three constants are Java's own arithmetic, not numbers retyped from a
    /// comment (`:38`, `:43`, `TickRateConstants.java:11`).
    #[test]
    fn the_timer_constants_are_java_arithmetic() {
        assert_eq!(MIN_TIME_BEFORE_SPAWNTRY, 6000);
        assert_eq!(CHILD_SPAWN_INTERVAL, 12000);
        assert_eq!(MAX_TICKRATE, 500);
        assert_eq!(20 * 60 * 5, MIN_TIME_BEFORE_SPAWNTRY);
        assert_eq!(20 * 60 * 10, CHILD_SPAWN_INTERVAL);
    }

    /// Consecutive ticks take `MAX_TICKRATE` each, and the comparison happens after
    /// the subtraction: 6000 ticks fire on the twelfth tick of a fresh clock.
    #[test]
    fn a_timer_of_six_thousand_fires_on_the_twelfth_tick() {
        let mut clock = Clock::new(0);
        assert_eq!(clock.timer(), 6000);
        for _ in 0..11 {
            assert!(!clock.on_tick(20, 20, 0), "still waiting");
        }
        assert!(clock.on_tick(20, 20, 0), "the twelfth tick tries");
        assert_eq!(clock.timer(), 6000, "(6000 + 0) * (20 / 20) again");
    }

    /// The divider is `max(4, maxCitizens)`, in integers, and a colony below it
    /// resets to a **zero** timer — Java's bug, kept and pinned (`:80`).
    #[test]
    fn the_reset_multiplies_by_the_fill_ratio_and_a_small_colony_retries_every_tick() {
        assert_eq!(reset_timer(0, 20, 20), 6000);
        assert_eq!(reset_timer(0, 40, 20), 12000);
        assert_eq!(reset_timer(6000, 60, 20), 36000);
        assert_eq!(reset_timer(0, 19, 20), 0, "19 / 20 is 0 in integers");
        assert_eq!(reset_timer(0, 3, 2), 0, "3 / max(4, 2) is 0");
        assert_eq!(reset_timer(0, 4, 0), 6000, "4 / max(4, 0) is 1");

        let mut clock = Clock::new(0);
        while !clock.on_tick(3, 20, 0) {}
        assert_eq!(clock.timer(), 0);
        assert!(clock.on_tick(3, 20, 0), "the very next tick tries again");
    }

    /// The five refusals, each alone, in Java's order — and the first that applies
    /// is the one reported.
    #[test]
    fn each_gate_refusal_is_named_and_the_first_one_wins() {
        let mut gate = allowed();
        gate.move_in = false;
        assert_eq!(gate.admit(), Err(Refusal::MoveInOff));

        let mut gate = allowed();
        gate.citizens = 20;
        assert_eq!(
            gate.admit(),
            Err(Refusal::AtCapacity {
                citizens: 20,
                max: 20
            })
        );

        let mut gate = allowed();
        gate.citizens = 1;
        assert_eq!(
            gate.admit(),
            Err(Refusal::BelowMinimum {
                citizens: 1,
                needed: 2
            })
        );

        let mut gate = allowed();
        gate.has_female = false;
        assert_eq!(gate.admit(), Err(Refusal::NoBioParents));

        let mut gate = allowed();
        gate.spare_bed_house = None;
        assert_eq!(gate.admit(), Err(Refusal::NoSpareBed));

        // Three reasons at once: the earliest in Java's line is the one answered.
        let mut gate = allowed();
        gate.move_in = false;
        gate.citizens = 99;
        gate.has_male = false;
        assert_eq!(gate.admit(), Err(Refusal::MoveInOff));
        assert_eq!(
            Refusal::NoSpareBed.name(),
            "minecolonies:reproduction/no-spare-bed"
        );
    }

    /// The minimum is `min(2, initialCitizenAmount)`, so a server configured with
    /// one initial citizen asks for one, not two (`:92`).
    #[test]
    fn the_minimum_follows_the_config_and_caps_at_two() {
        let mut gate = allowed();
        gate.citizens = 1;
        gate.initial_citizen_amount = 1;
        assert_eq!(gate.minimum(), 1);
        assert!(gate.admit().is_ok());

        gate.initial_citizen_amount = 10;
        assert_eq!(gate.minimum(), 2);
        assert!(gate.admit().is_err());
    }

    /// A man among the citizens and a woman among the visitors (or the other way
    /// round) is enough: the two flags travel across both lists (`:231-264`).
    #[test]
    fn a_male_visitor_and_a_female_citizen_are_enough() {
        let mut gate = allowed();
        gate.has_female = false;
        assert!(gate.admit().is_err());
        // The visitor list is ORed into the same two flags by the caller.
        gate.has_female = true;
        assert!(gate.admit().is_ok());
    }

    /// **No write before a refusal.** The plan is the only thing that writes, and a
    /// refused attempt produces none: the journal stays empty for all five.
    #[test]
    fn a_refused_attempt_writes_nothing_at_all() {
        let mut cases = Vec::new();
        for broken in 0..5 {
            let mut case = situation(house(vec![candidate(1, "Ada")]));
            match broken {
                0 => case.gate.move_in = false,
                1 => case.gate.citizens = 20,
                2 => case.gate.citizens = 1,
                3 => case.gate.has_female = false,
                _ => case.gate.spare_bed_house = None,
            }
            cases.push(case);
        }
        assert_eq!(cases.len(), 5);
        for case in cases {
            let mut journal = Journal::default();
            let outcome = decide(&case, &mut Script::new(&[0], &[false]));
            if let Ok(plan) = &outcome {
                plan.apply(&mut journal);
            }
            assert!(outcome.is_err(), "the gate refused");
            assert!(journal.0.is_empty(), "and nothing was written");
        }
    }

    /// The whole write set of a one-parent birth, in Java's order (`:159-217`).
    #[test]
    fn the_plan_writes_in_java_s_order() {
        let mut ada = candidate(1, "Ada");
        ada.children = vec![7];
        let mut case = situation(house(vec![ada]));
        case.only_child = false;
        let mut journal = Journal::default();
        let plan = decide(&case, &mut Script::new(&[0], &[false]))
            .expect("a colony with a spare bed gets a child");
        plan.apply(&mut journal);
        assert_eq!(
            journal.0,
            vec![
                "inherit-skills",
                "mark-child",
                "inherit-siblings",
                "add-child",
                "set-parents",
                "generate-name",
                "assign",
                "link-sibling",
                "set-suffix",
                "trigger-population",
                "spawn",
                "describe-birth",
                "count-birth",
            ],
            "one parent: no partner write, no birth message"
        );
        assert_eq!(plan.writes()[3], Write::AddChild { parent: 1, child: 9 });
        assert_eq!(plan.writes()[7], Write::LinkSibling { sibling: 7, citizen: 9 });
        assert_eq!(plan.writes()[10], Write::Spawn { citizen: 9, at: [10, 70, 20] });
        assert_eq!(plan.writes()[12], Write::CountBirth { day: 12 });
    }

    /// A partner on the first resident is used **without drawing**, and the two
    /// partner writes come before the skills (`:159-163`).
    #[test]
    fn a_known_partner_is_taken_without_a_draw_and_named_first() {
        let mut ada = candidate(1, "Ada");
        ada.partner = Some(2);
        let bob = candidate(2, "Bob");
        let case = situation(house(vec![ada, bob]));
        let mut script = Script::new(&[0], &[]);
        let chosen = choose_parents(&case.home, &case.alt, &mut script);
        assert_eq!(chosen.first, Some(1));
        assert_eq!(chosen.second, Some(2));
        assert!(script.indices.is_empty(), "no second index was drawn");

        let plan = decide(&case, &mut Script::new(&[0, 0], &[])).expect("admitted");
        assert_eq!(plan.writes()[0], Write::SetPartner { of: 1, to: 2 });
        assert_eq!(plan.writes()[1], Write::SetPartner { of: 2, to: 1 });
        assert_eq!(
            plan.writes()[2],
            Write::InheritSkills {
                first: Some(1),
                second: Some(2),
                happiness: 5
            }
        );
    }

    /// The pool filters a partner, a namesake and a family tie; an empty pool draws
    /// no coin; and with a neighbour the second parent comes from there, in the
    /// position the plan reports (`:128-147`).
    #[test]
    fn the_second_parent_comes_from_the_filtered_pool_or_from_a_neighbour() {
        let ada = candidate(1, "Ada");
        let mut cousin = candidate(3, "Cleo");
        cousin.siblings = vec![1];
        let namesake = candidate(4, "Ada");
        let mut taken = candidate(5, "Dora");
        taken.partner = Some(8);
        let child = Candidate {
            id: 6,
            name: "Eve".to_owned(),
            is_child: true,
            ..Candidate::default()
        };
        let home = house(vec![ada, cousin, namesake, taken, child]);

        let mut script = Script::new(&[0], &[]);
        let chosen = choose_parents(&home, &[], &mut script);
        assert_eq!(chosen.first, Some(1));
        assert_eq!(chosen.second, None);
        assert!(!script.coin_called(), "an empty pool draws no coin");

        let mut neighbour = Household {
            at: [40, 70, 20],
            residents: vec![candidate(8, "Fay"), candidate(1, "Ada")],
        };
        neighbour.residents.push(candidate(6, "Eve"));
        let chosen = choose_parents(&home, &[neighbour], &mut Script::new(&[0, 0, 0], &[]));
        assert_eq!(chosen.second, Some(8));
        assert_eq!(chosen.second_house, Some([40, 70, 20]));
    }

    /// `isRelatedTo` looks at *its own* family, never the other one's: the
    /// asymmetry is Java's (`CitizenData.java:791-795`).
    #[test]
    fn related_to_reads_only_its_own_side() {
        let mut parent = candidate(1, "Ada");
        parent.parents = ["Bea".to_owned(), "Cy".to_owned()];
        let bea = candidate(2, "Bea");
        assert!(parent.related_to(&bea), "Ada's parent list holds Bea's name");
        assert!(
            !bea.related_to(&parent),
            "Bea's own list holds nothing, so the reverse is false — Java's \
             `isRelatedTo` is one-directional (`CitizenData.java:791-795`)"
        );
        let mut sister = candidate(3, "Dee");
        sister.siblings = vec![1];
        assert!(sister.related_to(&parent));
        assert!(!sister.related_to(&bea));
    }

    /// The suffix pool is the parents' own strings, `_b` arrives with `_w` + `_d`,
    /// and no parent at all means all four (`:166-205`).
    #[test]
    fn the_suffix_pool_is_the_parents_then_the_bridge_then_all_four() {
        assert_eq!(
            suffixes(Some(SUFFIX_W), Some(SUFFIX_D)),
            owned(&["_w", "_d", "_b"])
        );
        assert_eq!(suffixes(Some(SUFFIX_W), Some(SUFFIX_W)), owned(&["_w", "_w"]));
        assert_eq!(suffixes(Some("_z"), None), owned(&["_z"]), "Java copies it unchecked");
        assert_eq!(suffixes(None, None), owned(&["_b", "_d", "_a", "_w"]));
        assert!(!suffixes(Some(SUFFIX_D), None).contains(&SUFFIX_BOTH.to_owned()));
    }

    /// The skills take their share of the two role models, and a zero total is
    /// **no change** rather than a NaN that happens to cast to zero.
    #[test]
    fn the_skills_take_their_share_and_a_zero_total_changes_nothing() {
        let flat = [0; SKILL_COUNT];
        let fours = [4; SKILL_COUNT];
        // total = 88, each pair = 8, bonus 44 -> each skill +4.
        assert_eq!(inherit_skills([1; SKILL_COUNT], [&fours, &fours], 44), [5; SKILL_COUNT]);
        let mut lopsided = [0; SKILL_COUNT];
        lopsided[0] = 10;
        let skills = inherit_skills([0; SKILL_COUNT], [&lopsided, &flat], 50);
        assert_eq!(skills[0], 50, "every point goes to the only skill with any");
        assert_eq!(skills[1], 0);
        assert_eq!(
            inherit_skills([1; SKILL_COUNT], [&flat, &flat], 25),
            [1; SKILL_COUNT],
            "0.0 / 0.0 is NaN in Java and (int) (NaN * 25) is 0"
        );
    }

    /// The message is for the colony's **first** child only, and the statistic is
    /// the last write, with the day the caller owns (`:211-217`).
    #[test]
    fn the_birth_message_is_for_the_first_child_and_the_statistic_comes_last() {
        let first = situation(house(vec![candidate(1, "Ada")]));
        let plan = decide(&first, &mut Script::new(&[0, 0], &[])).expect("admitted");
        assert!(plan
            .writes()
            .iter()
            .any(|write| matches!(write, Write::AnnounceBirth { citizen: 9 })));
        assert_eq!(plan.writes().last(), Some(&Write::CountBirth { day: 12 }));

        let mut later = situation(house(vec![candidate(1, "Ada")]));
        later.only_child = false;
        let plan = decide(&later, &mut Script::new(&[0, 0], &[])).expect("admitted");
        assert!(!plan
            .writes()
            .iter()
            .any(|write| matches!(write, Write::AnnounceBirth { .. })));
    }

    /// A house with no adult is Java's `:151-155` branch: no parent at all, and the
    /// plan keeps the eleven writes that need none.
    #[test]
    fn a_house_of_children_gets_no_parents_and_no_partner_writes() {
        let child = Candidate {
            id: 6,
            name: "Eve".to_owned(),
            is_child: true,
            ..Candidate::default()
        };
        let case = situation(house(vec![child]));
        let plan = decide(&case, &mut Script::new(&[0], &[])).expect("admitted");
        assert_eq!(
            plan.writes()[0],
            Write::InheritSkills {
                first: None,
                second: None,
                happiness: 5
            }
        );
        assert!(!plan
            .writes()
            .iter()
            .any(|write| matches!(write, Write::SetPartner { .. })));
        // The suffix pool falls back to the four constants, and the draw is an index.
        assert!(matches!(
            &plan.writes()[5],
            Write::SetSuffix { suffix } if suffix.as_str() == "_b"
        ));
        assert_eq!(plan.len(), 11);
    }

    /// The candidate list is read, never written: `getAssignedCitizen()` hands Java
    /// a copy (`AbstractAssignedCitizenModule.java:90-93`), and `&[Candidate]` is
    /// the same promise. The house still has its child after the birth.
    #[test]
    fn the_house_keeps_its_children_after_a_birth() {
        let child = Candidate {
            id: 6,
            name: "Eve".to_owned(),
            is_child: true,
            ..Candidate::default()
        };
        let case = situation(house(vec![candidate(1, "Ada"), child]));
        let before = case.clone();
        assert_eq!(case.home.residents.len(), 2);
        let _ = decide(&case, &mut Script::new(&[0, 0], &[]));
        assert_eq!(case, before);
        assert!(case.home.residents.iter().any(|who| who.is_child));
    }

    /// A partner who lives in neither house is still found: `getPartner()` is a
    /// colony-wide lookup (`:125`), and the plan must not name the child after
    /// nobody.
    #[test]
    fn a_partner_from_elsewhere_is_found_and_named() {
        let mut ada = candidate(1, "Ada");
        ada.partner = Some(30);
        let mut case = situation(house(vec![ada]));
        case.elsewhere = vec![candidate(30, "Zoe")];
        let plan = decide(&case, &mut Script::new(&[0, 0], &[])).expect("admitted");
        assert_eq!(plan.writes()[0], Write::SetPartner { of: 1, to: 30 });
        assert_eq!(
            plan.writes()[8],
            Write::SetParents {
                first: "Ada".to_owned(),
                second: "Zoe".to_owned()
            }
        );
    }
}
