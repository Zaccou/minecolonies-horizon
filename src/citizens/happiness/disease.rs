//! Falling ill, spreading it, and getting better.
//!
//! Java reference: `core/entity/citizen/citizenhandlers/CitizenDiseaseHandler.java`
//! (all 268 lines: the roll at `:100-120`, the gate at `:138-146`, the collision
//! at `:148-172`, the cure at `:222-255`), `core/datalistener/model/Disease.java`
//! (a record of id, name, rarity and cure items) and the three shipped files in
//! `src/main/resources/data/minecolonies/colony/diseases/`.
//!
//! **A disease has no duration, and the brief is wrong about that.** Nothing in
//! `Disease` is a timer and nothing in the handler counts down a disease: it is
//! caught, it stays, and it ends **only** when something cures it. The timer that
//! *does* exist is the opposite one — `immunityTicks` (`:46`, `:72`), the period
//! after a cure during which a citizen cannot fall ill again. A player who does
//! nothing therefore has a permanently ill citizen whose health regenerates only
//! to a third (`EntityCitizen.java:891`), and the doc comment that says "20 * 60 *
//! 90" for `IMMUNITY_TIME` is the only clock in the file.
//!
//! **Five ways in, one way out, and the counts.**
//!
//! | What | Rule | Java |
//! | --- | --- | --- |
//! | catching it | `nextInt(config · 33333) < modifier · 10`, every update (60 ticks) | `:110` |
//! | catching it from a neighbour | 1 in 100 per collision, masks halve it | `:153` |
//! | catching it from poisoned food | `setDisease(...)` on any poisonous item | `EntityCitizen.java:499` |
//! | cured by a doctor | 30 progress ticks, then `cure()` | `EntityAIWorkHealer.java:63`, `:434` |
//! | cured in bed | the cure items, then `cure()` | `EntityAISickTask.java:278-303` |
//! | cured by sheer time | 1 in 7 200 per 20-tick check while waiting in bed | `:333` |
//! | cured by a golden apple | 1 in 3, then `cure()` | `EntityCitizen.java:479-482` |
//!
//! **Nothing here draws a random number.** `needs.rs` already made that choice for
//! the cook (`Appetite::cook_skips`): a wasm mod that read a random source could
//! not be replayed, so the caller hands in the two draws ([`Draw`], [`Collision`])
//! and this module only applies the comparisons Java writes around them.
//!
//! **What is not here.** The item stacks behind a cure ([`Cure`] holds the registry
//! name, not a stack), the hospital bed's bookkeeping (`registerPatient`, `:296`),
//! the statistics counter (`CITIZENS_HEALED`, `:253`) and the NBT codec
//! (`:187-213`). Each needs the game; none of them changes a rule above.

use crate::citizens::needs::MAX_HEALTH;

use super::factors::FoodHistory;
use crate::jobs::Job;

/// `CitizenDiseaseHandler.DISEASE_FACTOR` (`:41`): `100000 / 3`, integer
/// division, so **33333** and not 33333.33.
///
/// The config value multiplies it: at the default `diseasemodifier = 5`
/// (`ServerConfiguration.java:136`) the bound is `166 665` and the threshold is
/// `modifier · 10`.
pub const DISEASE_FACTOR: i32 = 100_000 / 3;

/// `CitizenDiseaseHandler.IMMUNITY_TIME` (`:46`): `20 · 60 · 90`, ninety minutes.
pub const IMMUNITY_TIME: i32 = 20 * 60 * 90;

/// `CitizenDiseaseHandler.COLLISION_COOLDOWN` (`:51`): `20 · 30`, thirty seconds.
pub const COLLISION_COOLDOWN: i32 = 20 * 30;

/// `CitizenDiseaseHandler.VACCINE_MODIFIER` (`:56`): vaccines make the immunity
/// **ten times** longer.
pub const VACCINE_MODIFIER: i32 = 10;

/// The collision's one-in-`Constants.ONE_HUNDRED_PERCENT` chance
/// (`CitizenDiseaseHandler.java:153`, `Constants.java` holds the `100`).
pub const CONTAGION_IN: i32 = 100;

/// The bedridden self-cure: `nextInt(60 · 60 · 2) < 1` per check
/// (`EntityAISickTask.java:333`), checked every 20 ticks (`:128`), so one chance
/// in 7 200 every 20 ticks — about once every two hours of lying there.
pub const SELF_CURE_IN: i32 = 60 * 60 * 2;

/// The doctor's work: `MAX_PROGRESS_TICKS = 30` clicks of healing before the cure
/// lands (`EntityAIWorkHealer.java:63`, read at `:411-434`).
pub const HEALER_TICKS: i32 = 30;

/// The research effect that lets a mask block a contagion
/// (`ResearchConstants.MASKS`, `effects/masks`), read as
/// `crate::research::Effects::strength(MASKS_EFFECT)` and compared against zero
/// by [`Sickness::contact`].
pub const MASKS_EFFECT: &str = "masks";

/// The research effect that lengthens an immunity (`ResearchConstants.VACCINES`,
/// `effects/vaccines`), read as
/// `crate::research::Effects::strength(VACCINES_EFFECT)`. Only its presence
/// matters: `:162` and `:238` both test `> 0`.
pub const VACCINES_EFFECT: &str = "vaccines";

/// The default `diseasemodifier` config value (`ServerConfiguration.java:136`).
pub const DEFAULT_DISEASE_MODIFIER: i32 = 5;

/// The default `initialcitizenamount` (`ServerConfiguration.java:125`), the floor
/// under which nobody falls ill.
pub const DEFAULT_INITIAL_CITIZENS: usize = 4;

/// The gold at `update`'s first draw: `citizenModifier * 10`
/// (`CitizenDiseaseHandler.java:110`).
pub const SUSCEPTIBILITY_PER_MODIFIER: f64 = 10.0;

/// `FoodUtils`' diversity divisor in the disease modifier
/// (`CitizenFoodHandler.java:169`).
pub const DIVERSITY_FACTOR: f64 = 5.0;

/// The cap on that divisor (`CitizenFoodHandler.java:169`).
pub const DIVERSITY_CAP: f64 = 2.5;

/// The halving the food history applies before the diversity term
/// (`CitizenFoodHandler.java:169`).
pub const DIET_HALF: f64 = 0.5;

/// One item a disease needs, and how many of them.
///
/// Java's `ItemStorage` (`Disease.cureItems()`), narrowed to the registry name
/// and a count — what the JSON file says and all the rule needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Cure {
    /// The item's registry name, e.g. `minecraft:carrot`.
    pub item: &'static str,
    /// How many the cure eats (`EntityAISickTask.java:288-292` takes one slot
    /// each, so this is a request count, not a stack size).
    pub count: i32,
    /// `fichier:ligne` or the data file this pair comes from.
    pub source: &'static str,
}

/// A disease: an id, a name key, a weight, and what cures it
/// (`core/datalistener/model/Disease.java:24`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Disease {
    /// The registry id, `minecolonies:influenza` — the file name under
    /// `data/minecolonies/colony/diseases/`.
    pub id: &'static str,
    /// The translation key (`Disease.java:20`, read from the JSON's `name`).
    pub name: &'static str,
    /// The weight (`Disease.getWeight` → `Weight.of(rarity)`, `:69-74`).
    /// Higher is more common.
    pub rarity: i32,
    /// What cures it (`Disease.cureItems`, `:24`).
    pub cures: &'static [Cure],
    /// The data file this row comes from.
    pub source: &'static str,
}

impl Disease {
    /// `colony/diseases/influenza.json`: rarity 100, a carrot and a potato.
    pub const INFLUENZA: Disease = Disease {
        id: "minecolonies:influenza",
        name: "com.minecolonies.coremod.diseases.influenza",
        rarity: 100,
        cures: &[
            Cure {
                item: "minecraft:carrot",
                count: 1,
                source: "colony/diseases/influenza.json",
            },
            Cure {
                item: "minecraft:potato",
                count: 1,
                source: "colony/diseases/influenza.json",
            },
        ],
        source: "data/minecolonies/colony/diseases/influenza.json",
    };

    /// `colony/diseases/measles.json`: rarity 10, a poppy, a dandelion and kelp.
    pub const MEASLES: Disease = Disease {
        id: "minecolonies:measles",
        name: "com.minecolonies.coremod.diseases.measles",
        rarity: 10,
        cures: &[
            Cure {
                item: "minecraft:poppy",
                count: 1,
                source: "colony/diseases/measles.json",
            },
            Cure {
                item: "minecraft:dandelion",
                count: 1,
                source: "colony/diseases/measles.json",
            },
            Cure {
                item: "minecraft:kelp",
                count: 1,
                source: "colony/diseases/measles.json",
            },
        ],
        source: "data/minecolonies/colony/diseases/measles.json",
    };

    /// `colony/diseases/smallpox.json`: rarity 1, honey and a golden apple — the
    /// rarest and the only one whose cure a player cannot farm in the colony.
    pub const SMALLPOX: Disease = Disease {
        id: "minecolonies:smallpox",
        name: "com.minecolonies.coremod.diseases.smallpox",
        rarity: 1,
        cures: &[
            Cure {
                item: "minecraft:honey_bottle",
                count: 1,
                source: "colony/diseases/smallpox.json",
            },
            Cure {
                item: "minecraft:golden_apple",
                count: 1,
                source: "colony/diseases/smallpox.json",
            },
        ],
        source: "data/minecolonies/colony/diseases/smallpox.json",
    };

    /// The three the mod ships, in file order. A data pack can add more; the
    /// weight is what decides how often.
    pub const SHIPPED: [Disease; 3] = [Disease::INFLUENZA, Disease::MEASLES, Disease::SMALLPOX];
}

/// One row of a `colony/diseases/*.json`, before it is checked.
///
/// Java's `DiseasesListener` reads the three keys `name`, `rarity` and `items`
/// (`DiseasesListener.java:43-45`) and lets a malformed file through — an empty
/// `items` list becomes a disease nothing cures. The port refuses it, which is
/// the one place it is stricter than Java on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Row {
    /// The registry id.
    pub id: &'static str,
    /// The translation key.
    pub name: &'static str,
    /// The weight.
    pub rarity: i32,
    /// What cures it.
    pub cures: &'static [Cure],
    /// The file it was read from.
    pub source: &'static str,
}

/// Why a disease table cannot be built, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A cure list with nothing in it: the disease would be uncurable and the
    /// hospital would keep the patient forever.
    NoCure { id: &'static str },
    /// Two rows claim the same id; the first would silently win in a map.
    Duplicate { id: &'static str },
    /// A weight of zero or less: `WeightedRandomList` cannot draw it, and Java
    /// would quietly never pick it.
    NotPositiveRarity { id: &'static str },
    /// A cure that asks for nothing, or a negative amount.
    EmptyCure { id: &'static str },
    /// A citizen that cannot take an illness — the gate of `canBecomeSick` is
    /// shut, so a forced disease (`setDisease`) has nowhere to go.
    GateClosed { id: &'static str },
}

impl Refusal {
    /// The namespaced name of this refusal, for a log line.
    pub fn name(self) -> String {
        match self {
            Refusal::NoCure { id } => format!("citizens.happiness.refusal.no_cure({id})"),
            Refusal::Duplicate { id } => format!("citizens.happiness.refusal.duplicate({id})"),
            Refusal::NotPositiveRarity { id } => {
                format!("citizens.happiness.refusal.not_positive_rarity({id})")
            }
            Refusal::EmptyCure { id } => format!("citizens.happiness.refusal.empty_cure({id})"),
            Refusal::GateClosed { id } => format!("citizens.happiness.refusal.gate_closed({id})"),
        }
    }
}

/// Build the table from parsed rows, refusing what Java would silently accept.
pub fn table(rows: &[Row]) -> Result<Vec<Disease>, Refusal> {
    let mut table: Vec<Disease> = Vec::with_capacity(rows.len());
    for row in rows {
        if row.rarity <= 0 {
            return Err(Refusal::NotPositiveRarity { id: row.id });
        }
        if table.iter().any(|disease| disease.id == row.id) {
            return Err(Refusal::Duplicate { id: row.id });
        }
        if row.cures.is_empty() {
            return Err(Refusal::NoCure { id: row.id });
        }
        if row.cures.iter().any(|cure| cure.count <= 0) {
            return Err(Refusal::EmptyCure { id: row.id });
        }
        table.push(Disease {
            id: row.id,
            name: row.name,
            rarity: row.rarity,
            cures: row.cures,
            source: row.source,
        });
    }
    Ok(table)
}

/// The sum of the weights (`WeightedRandomList.create`'s total).
pub fn total_weight(diseases: &[Disease]) -> i32 {
    diseases.iter().map(|disease| disease.rarity).sum()
}

/// `WeightedRandomList.getRandom(random)` walked by hand: a roll in
/// `0..total_weight`, and the disease whose cumulative weight covers it.
///
/// The shipped weights are 100, 10 and 1 — so influenza is `100/111` of every
/// illness, measles `10/111`, smallpox `1/111` — and there is no way to be ill
/// with something a data pack removed.
pub fn pick(diseases: &[Disease], roll: i32) -> Option<Disease> {
    if roll < 0 {
        return None;
    }
    let mut left = roll;
    for disease in diseases {
        if left < disease.rarity {
            return Some(*disease);
        }
        left -= disease.rarity;
    }
    None
}

/// The five conditions of `canBecomeSick` (`:138-146`) that are not the citizen's
/// own state.
///
/// Java reads `citizenData.getEntity().isPresent()`, `colony.isActive()`,
/// `job instanceof JobHealer`, the immunity timer and
/// `getCurrentCitizenCount() > initialCitizenCount`. The last one is why a colony
/// of four — the default `initialcitizenamount` — never has a disease until its
/// fifth citizen arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Gate {
    /// `getEntity().isPresent()`.
    pub loaded: bool,
    /// `colony.isActive()`.
    pub colony_active: bool,
    /// `getJob() instanceof JobHealer` (`:143`).
    pub healer: bool,
    /// `getCurrentCitizenCount()`.
    pub citizens: usize,
    /// `initialCitizenAmount` from the server config.
    pub initial_citizens: usize,
}

impl Default for Gate {
    fn default() -> Gate {
        Gate {
            loaded: true,
            colony_active: true,
            healer: false,
            citizens: DEFAULT_INITIAL_CITIZENS + 1,
            initial_citizens: DEFAULT_INITIAL_CITIZENS,
        }
    }
}

impl Gate {
    /// A colony of `citizens`, past the initial gate.
    pub fn of(citizens: usize) -> Gate {
        Gate {
            citizens,
            ..Gate::default()
        }
    }
}

/// The two draws one `update` makes (`CitizenDiseaseHandler.java:110-113`).
///
/// `susceptibility` is `getRandom().nextInt(configModifier * DISEASE_FACTOR)` and
/// `disease` is `DiseasesListener.getRandomDisease(random)` — drawn only when the
/// first one lands, so a caller that cannot draw yet may pass `None` and the roll
/// simply finds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Draw {
    /// `nextInt(configModifier · DISEASE_FACTOR)`.
    pub susceptibility: i32,
    /// The disease the second draw landed on.
    pub disease: Option<Disease>,
}

/// The two answers Java draws during a collision
/// (`CitizenDiseaseHandler.java:153-156`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Collision {
    /// `nextInt(100) < 1`.
    pub infected: bool,
    /// `nextBoolean()` — only asked when masks are in play.
    pub past_masks: bool,
}

/// What a collision did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three outcomes, one per branch of :153-170
pub enum Contact {
    /// Not ill, already ill, immune, or the one-in-a-hundred missed.
    Nothing,
    /// The disease passed.
    Caught,
    /// The mask held; the citizen got a cooldown instead.
    Blocked,
}

/// A citizen's illness, its immunity timer, and whether it is sleeping in a
/// hospital bed (`CitizenDiseaseHandler`'s three fields, `:67-77`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Sickness {
    /// `disease`, `None` when healthy (`:66-67`).
    pub disease: Option<Disease>,
    /// `immunityTicks` (`:72`). Java lets it go negative; so does this.
    pub immunity: i32,
    /// `sleepsAtHospital` (`:77`).
    pub sleeps_at_hospital: bool,
}

impl Sickness {
    /// A healthy citizen with no immunity — how `CitizenDiseaseHandler` starts.
    pub fn new() -> Sickness {
        Sickness::default()
    }

    /// The bedridden citizen's own cure: `nextInt(SELF_CURE_IN) < 1`
    /// (`EntityAISickTask.java:333`), checked every 20 ticks (`:128`).
    ///
    /// `roll` is the caller's draw in `0..SELF_CURE_IN`; the function only
    /// applies Java's comparison, so a test can drive both ends of it.
    pub fn self_cures(roll: i32) -> bool {
        roll < 1
    }

    /// The doctor's cure lands once it has worked [`HEALER_TICKS`] of them
    /// (`EntityAIWorkHealer.java:411-434`).
    pub fn doctor_cures(progress: i32) -> bool {
        progress >= HEALER_TICKS
    }

    /// `isSick()` (`:181-185`): the disease is set, nothing else.
    pub fn is_sick(&self) -> bool {
        self.disease.is_some()
    }

    /// `canBecomeSick()` (`:138-146`), all six conditions in Java's order.
    pub fn can_fall_ill(&self, gate: &Gate) -> bool {
        !self.is_sick()
            && gate.loaded
            && gate.colony_active
            && !gate.healer
            && self.immunity <= 0
            && gate.citizens > gate.initial_citizens
    }

    /// `setDisease` (`:122-131`): a forced illness — poisoned food
    /// (`EntityCitizen.java:499`) — refused when the gate is closed.
    pub fn catch(&mut self, disease: Disease, gate: &Gate) -> Result<(), Refusal> {
        if self.can_fall_ill(gate) {
            self.disease = Some(disease);
            Ok(())
        } else {
            Err(Refusal::GateClosed { id: disease.id })
        }
    }

    /// Clear the illness without leaving an immunity behind — what a save
    /// editor or a healing command would do, and *not* what `cure` does.
    pub fn clear(&mut self) {
        self.disease = None;
    }

    /// The immunity a cure leaves behind, in ticks (`:222-255`).
    ///
    /// Three answers, and the middle one is the surprise: cured **in bed**, a
    /// citizen is immune for ninety minutes, `× 10` with the vaccines research;
    /// cured standing up, for forty-five — half. Java's `cure()` is called from
    /// the doctor (`EntityAIWorkHealer.java:434`), from the bed
    /// (`EntityAISickTask.java:303`) and from the golden apple
    /// (`EntityCitizen.java:482`), and only the second one has `isAsleep()` true.
    pub fn cure(&mut self, asleep: bool, vaccines: bool) {
        self.disease = None;
        self.sleeps_at_hospital = false;
        self.immunity = if asleep {
            if vaccines {
                IMMUNITY_TIME.saturating_mul(VACCINE_MODIFIER)
            } else {
                IMMUNITY_TIME
            }
        } else {
            IMMUNITY_TIME / 2
        };
    }

    /// The bound of the susceptibility draw: `configModifier · DISEASE_FACTOR`
    /// (`:110`).
    pub fn susceptibility_bound(config: i32) -> i32 {
        config.saturating_mul(DISEASE_FACTOR)
    }

    /// The threshold the draw is compared against: `citizenModifier · 10`
    /// (`:110`), as a `double` because Java compares an `int` to a `double`.
    pub fn susceptibility_threshold(modifier: f64) -> f64 {
        modifier * SUSCEPTIBILITY_PER_MODIFIER
    }

    /// `update(tickRate)` (`:100-120`), in Java's order: the catching roll first,
    /// then the immunity countdown — so the immunity a citizen has *this* update
    /// still blocks *this* update.
    ///
    /// The roll arrives in [`Draw`] and must have been drawn in
    /// `0..Self::susceptibility_bound(config)`; the bound itself is not read
    /// again here, because Java compares the *threshold* (`modifier · 10`) and
    /// never the bound.
    ///
    /// Returns the disease it caught, so a caller can log or show it.
    pub fn update(&mut self, ticks: i32, gate: &Gate, modifier: f64, draw: Draw) -> Option<Disease> {
        let mut caught = None;
        if self.can_fall_ill(gate)
            && (draw.susceptibility as f64) < Self::susceptibility_threshold(modifier)
        {
            if let Some(disease) = draw.disease {
                self.disease = Some(disease);
                caught = Some(disease);
            }
        }
        if self.immunity > 0 {
            self.immunity -= ticks;
        }
        caught
    }

    /// `onCollission(otherCitizen)` (`:148-172`): a sick neighbour, one chance in
    /// a hundred, and masks that either block it or buy a cooldown.
    ///
    /// The cooldown is doubled by the vaccines research (`:162-169`) — and it is
    /// `max`'d with what the citizen already has, so a shorter cooldown never
    /// erases a longer one.
    pub fn contact(
        &mut self,
        gate: &Gate,
        other: Option<Disease>,
        masks: f64,
        vaccines: bool,
        collision: Collision,
    ) -> Contact {
        let Some(other) = other else {
            return Contact::Nothing;
        };
        if !collision.infected || !self.can_fall_ill(gate) {
            return Contact::Nothing;
        }
        if masks <= 0.0 || collision.past_masks {
            self.disease = Some(other);
            Contact::Caught
        } else {
            let cooldown = if vaccines {
                COLLISION_COOLDOWN.saturating_mul(2)
            } else {
                COLLISION_COOLDOWN
            };
            self.immunity = self.immunity.max(cooldown);
            Contact::Blocked
        }
    }

    /// The health a sick citizen may regenerate to: `getMaxHealth() / 3`
    /// (`EntityCitizen.java:891`), which `needs::Needs::heal_amount` already
    /// applies through `Needs::sick`.
    pub fn health_ceiling(&self) -> f32 {
        if self.is_sick() { MAX_HEALTH / 3.0 } else { MAX_HEALTH }
    }
}

/// `IJob.getDiseaseModifier` (`IJob.java:202`, default `1` at
/// `AbstractJob.java:420`) and its four overrides.
///
/// | Job | Modifier | Java |
/// | --- | ---: | --- |
/// | doctor | `0` | `JobHealer.java:74-77` — never ill |
/// | miner, swineherder | `2` | `JobMiner.java:48-51`, `JobSwineHerder.java:54-57` |
/// | composter | `(int)((100 − skill) / 25)` | `JobComposter.java:43-47` |
/// | every other job | `1` | `AbstractJob.java:420` |
///
/// The composter's number is the one that moves: a level-99 composter answers
/// `0` (like a doctor) and a level-0 one answers `4`. `composter_skill` is the
/// primary skill of its work module and is read only for that job.
pub fn job_modifier(job: Option<Job>, composter_skill: i32) -> f64 {
    match job {
        Some(Job::Healer) => 0.0,
        Some(Job::Miner | Job::SwineHerder) => 2.0,
        Some(Job::Composter) => {
            // `(int)` on a non-negative quotient is a truncation.
            ((100 - composter_skill) as f64 / 25.0).trunc()
        }
        _ => 1.0,
    }
}

/// `CitizenFoodHandler.getDiseaseModifier` (`:163-170`): a varied diet makes a
/// citizen harder to infect.
///
/// ```java
/// if (lastEatenFoods.size() < 10 || baseModifier == 0) return baseModifier;
/// return baseModifier * 0.5 * Math.min(2.5, 5.0 / diversity());
/// ```
///
/// So five kinds of food is the neutral point (`0.5 · 1.0`), ten kinds is
/// `0.25`, and one kind is `1.25` — a citizen living on bread is half again as
/// likely to fall ill as one eating five things.
pub fn modifier(base: f64, food: &FoodHistory) -> f64 {
    if base == 0.0 {
        return 0.0;
    }
    let FoodHistory::Full { diversity, .. } = *food else {
        return base;
    };
    if diversity <= 0 {
        return base;
    }
    base * DIET_HALF * DIVERSITY_CAP.min(DIVERSITY_FACTOR / diversity as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_shipped_diseases_keep_their_weights_and_their_cures() {
        let shipped = Disease::SHIPPED;
        assert_eq!(shipped.len(), 3);
        assert_eq!(total_weight(&shipped), 111, "100 + 10 + 1");
        assert_eq!(shipped[0].rarity, 100, "influenza.json");
        assert_eq!(shipped[1].rarity, 10, "measles.json");
        assert_eq!(shipped[2].rarity, 1, "smallpox.json");
        assert_eq!(shipped[0].cures.len(), 2);
        assert_eq!(shipped[1].cures.len(), 3);
        assert_eq!(shipped[2].cures.len(), 2);
        assert_eq!(shipped[2].cures[1].item, "minecraft:golden_apple");
        // Every row says where it comes from.
        for disease in shipped {
            assert!(disease.source.contains("colony/diseases/"), "{}", disease.source);
            assert!(disease.id.starts_with("minecolonies:"));
            assert!(
                disease.cures.iter().all(|cure| cure.count > 0),
                "{}",
                disease.id
            );
        }
    }

    #[test]
    fn picking_walks_the_cumulative_weight_and_stops_at_the_total() {
        let shipped = Disease::SHIPPED;
        for (roll, expected) in [
            (0, Disease::INFLUENZA),
            (99, Disease::INFLUENZA),
            (100, Disease::MEASLES),
            (109, Disease::MEASLES),
            (110, Disease::SMALLPOX),
        ] {
            assert_eq!(pick(&shipped, roll).map(|d| d.id), Some(expected.id), "roll {roll}");
        }
        assert_eq!(pick(&shipped, 111), None, "past the total, nothing");
        assert_eq!(pick(&shipped, -1), None);
        assert_eq!(pick(&[], 0), None);
        // No rarity is reachable except through the list: a removed disease
        // cannot be caught even if a save mentions it.
        assert_eq!(pick(&shipped[..1], 100), None);
    }

    #[test]
    fn the_table_refuses_an_uncurable_a_duplicate_and_a_weightless_disease() {
        let ok = table(&[
            Row {
                id: Disease::INFLUENZA.id,
                name: Disease::INFLUENZA.name,
                rarity: 100,
                cures: Disease::INFLUENZA.cures,
                source: Disease::INFLUENZA.source,
            },
            Row {
                id: Disease::SMALLPOX.id,
                name: Disease::SMALLPOX.name,
                rarity: 1,
                cures: Disease::SMALLPOX.cures,
                source: Disease::SMALLPOX.source,
            },
        ]);
        let ok = ok.expect("two well-formed rows are a table");
        assert_eq!(ok.len(), 2);
        assert_eq!(total_weight(&ok), 101);

        let duplicate = table(&[
            Row {
                id: "minecolonies:flu",
                name: "a",
                rarity: 1,
                cures: Disease::MEASLES.cures,
                source: "test",
            },
            Row {
                id: "minecolonies:flu",
                name: "b",
                rarity: 1,
                cures: Disease::MEASLES.cures,
                source: "test",
            },
        ]);
        assert_eq!(
            duplicate,
            Err(Refusal::Duplicate {
                id: "minecolonies:flu"
            })
        );
        assert_eq!(
            table(&[Row {
                id: "minecolonies:nothing",
                name: "a",
                rarity: 5,
                cures: &[],
                source: "test",
            }]),
            Err(Refusal::NoCure {
                id: "minecolonies:nothing"
            })
        );
        assert_eq!(
            table(&[Row {
                id: "minecolonies:weightless",
                name: "a",
                rarity: 0,
                cures: Disease::MEASLES.cures,
                source: "test",
            }]),
            Err(Refusal::NotPositiveRarity {
                id: "minecolonies:weightless"
            })
        );
        // Every refusal names itself and stays namespaced.
        for refusal in [
            Refusal::NoCure { id: "x" },
            Refusal::Duplicate { id: "x" },
            Refusal::NotPositiveRarity { id: "x" },
            Refusal::EmptyCure { id: "x" },
            Refusal::GateClosed { id: "x" },
        ] {
            assert!(
                refusal.name().starts_with("citizens.happiness.refusal."),
                "{}",
                refusal.name()
            );
        }
    }

    #[test]
    fn the_gate_has_six_conditions_and_each_one_alone_blocks_the_illness() {
        let open = Gate::of(10);
        let healthy = Sickness::new();
        assert!(healthy.can_fall_ill(&open));
        for shut in [
            Gate {
                loaded: false,
                ..open
            },
            Gate {
                colony_active: false,
                ..open
            },
            Gate {
                healer: true,
                ..open
            },
            Gate {
                citizens: DEFAULT_INITIAL_CITIZENS,
                ..open
            },
        ] {
            assert!(!healthy.can_fall_ill(&shut), "{shut:?}");
        }
        // The immunity timer is the fifth.
        let immune = Sickness {
            immunity: 1,
            ..Sickness::new()
        };
        assert!(!immune.can_fall_ill(&open));
        let recovered = Sickness {
            immunity: 0,
            ..Sickness::new()
        };
        assert!(recovered.can_fall_ill(&open));
        // And being ill already is the sixth.
        let ill = Sickness {
            disease: Some(Disease::MEASLES),
            ..Sickness::new()
        };
        assert!(!ill.can_fall_ill(&open));
    }

    #[test]
    fn a_colony_only_starts_falling_ill_past_its_initial_head_count() {
        // Default `initialcitizenamount` is 4, and the comparison is `>`.
        for (citizens, expected) in [(0, false), (4, false), (5, true), (25, true)] {
            let gate = Gate {
                citizens,
                initial_citizens: DEFAULT_INITIAL_CITIZENS,
                ..Gate::default()
            };
            assert_eq!(
                Sickness::new().can_fall_ill(&gate),
                expected,
                "{citizens} citizens"
            );
        }
    }

    #[test]
    fn the_susceptibility_roll_is_ten_in_a_hundred_and_sixty_six_thousand() {
        assert_eq!(DISEASE_FACTOR, 33333, "integer division at :41");
        assert_eq!(Sickness::susceptibility_bound(DEFAULT_DISEASE_MODIFIER), 166_665);
        assert_eq!(Sickness::susceptibility_threshold(1.0), 10.0);
        // A well-fed citizen (modifier 0.5) is tested against half of that.
        assert_eq!(Sickness::susceptibility_threshold(0.5), 5.0);

        let gate = Gate::of(10);
        let mut sickness = Sickness::new();
        // Roll 9 is under the threshold of 10: ill.
        let caught = sickness.update(
            0,
            &gate,
            1.0,
            Draw {
                susceptibility: 9,
                disease: Some(Disease::INFLUENZA),
            },
        );
        assert_eq!(caught.map(|d| d.id), Some("minecolonies:influenza"));
        assert!(sickness.is_sick());
        // Roll 10 is not: the comparison is strict.
        let mut clean = Sickness::new();
        assert_eq!(
            clean.update(
                0,
                &gate,
                1.0,
                Draw {
                    susceptibility: 10,
                    disease: Some(Disease::INFLUENZA),
                },
            ),
            None
        );
        assert!(!clean.is_sick());
        // A healer's modifier of 0 can never land, however the die falls.
        let mut doctor = Sickness::new();
        assert_eq!(
            doctor.update(
                0,
                &gate,
                0.0,
                Draw {
                    susceptibility: 0,
                    disease: Some(Disease::INFLUENZA),
                },
            ),
            None
        );
    }

    #[test]
    fn the_immunity_counts_down_and_blocks_until_it_reaches_zero() {
        let gate = Gate::of(10);
        let mut sickness = Sickness::new();
        sickness.cure(true, false);
        assert_eq!(sickness.immunity, IMMUNITY_TIME);
        assert!(!sickness.can_fall_ill(&gate));
        // 108 000 ticks is ninety minutes; each update takes its tickRate.
        let draw = Draw {
            susceptibility: 0,
            disease: Some(Disease::MEASLES),
        };
        let mut ticks = 0;
        while sickness.immunity > 0 && ticks < IMMUNITY_TIME {
            sickness.update(60, &gate, 1.0, draw);
            ticks += 60;
            assert!(!sickness.is_sick(), "immune at {ticks}");
        }
        assert!(ticks <= IMMUNITY_TIME);
        assert!(sickness.can_fall_ill(&gate), "and then it can fall ill again");
    }

    #[test]
    fn an_ill_citizen_cannot_catch_a_second_disease() {
        let gate = Gate::of(10);
        let mut sickness = Sickness {
            disease: Some(Disease::INFLUENZA),
            ..Sickness::new()
        };
        assert_eq!(
            sickness.update(
                60,
                &gate,
                0.0,
                Draw {
                    susceptibility: 0,
                    disease: Some(Disease::SMALLPOX),
                },
            ),
            None,
            "a modifier of 0 keeps it, but the gate is what really stops it"
        );
        // With a modifier that would certainly land, the disease still does not
        // change: `canBecomeSick` refuses first.
        sickness.update(
            60,
            &gate,
            1.0,
            Draw {
                susceptibility: 0,
                disease: Some(Disease::SMALLPOX),
            },
        );
        assert_eq!(sickness.disease.map(|d| d.id), Some("minecolonies:influenza"));
    }

    #[test]
    fn contagion_is_one_in_a_hundred_and_masks_either_block_it_or_buy_a_cooldown() {
        let gate = Gate::of(10);
        let mut sickness = Sickness::new();
        // No sick neighbour: nothing, whatever the die says.
        assert_eq!(
            sickness.contact(
                &gate,
                None,
                0.0,
                false,
                Collision {
                    infected: true,
                    past_masks: true
                }
            ),
            Contact::Nothing
        );
        // The one-in-a-hundred missed.
        assert_eq!(
            sickness.contact(
                &gate,
                Some(Disease::MEASLES),
                0.0,
                false,
                Collision {
                    infected: false,
                    past_masks: true
                }
            ),
            Contact::Nothing
        );
        // No masks: it passes.
        assert_eq!(
            sickness.contact(
                &gate,
                Some(Disease::MEASLES),
                0.0,
                false,
                Collision {
                    infected: true,
                    past_masks: false
                }
            ),
            Contact::Caught
        );
        assert_eq!(sickness.disease.map(|d| d.id), Some("minecolonies:measles"));
        // Masks and a good draw: the mask holds and leaves a thirty-second
        // cooldown.
        let mut masked = Sickness::new();
        assert_eq!(
            masked.contact(
                &gate,
                Some(Disease::MEASLES),
                1.0,
                false,
                Collision {
                    infected: true,
                    past_masks: false
                }
            ),
            Contact::Blocked
        );
        assert_eq!(masked.immunity, COLLISION_COOLDOWN);
        assert!(!masked.is_sick());
        // With vaccines the same hold is worth twice as long.
        let mut vaccinated = Sickness::new();
        vaccinated.contact(
            &gate,
            Some(Disease::MEASLES),
            1.0,
            true,
            Collision {
                infected: true,
                past_masks: false
            },
        );
        assert_eq!(vaccinated.immunity, COLLISION_COOLDOWN * 2);
        // And a longer cooldown is never shortened by a later collision.
        masked.contact(
            &gate,
            Some(Disease::SMALLPOX),
            1.0,
            false,
            Collision {
                infected: true,
                past_masks: false
            },
        );
        assert_eq!(masked.immunity, COLLISION_COOLDOWN);
    }

    #[test]
    fn a_cure_in_bed_is_worth_ten_times_more_with_vaccines_and_half_standing_up() {
        let mut in_bed = Sickness {
            disease: Some(Disease::INFLUENZA),
            sleeps_at_hospital: true,
            ..Sickness::new()
        };
        in_bed.cure(true, false);
        assert!(!in_bed.is_sick());
        assert!(!in_bed.sleeps_at_hospital, "cure() clears the flag at :226");
        assert_eq!(in_bed.immunity, 108_000, "20 · 60 · 90");
        assert_eq!(in_bed.health_ceiling(), MAX_HEALTH);

        let mut vaccinated = Sickness::new();
        vaccinated.cure(true, true);
        assert_eq!(vaccinated.immunity, 1_080_000, "VACCINE_MODIFIER = 10");

        let mut upright = Sickness::new();
        upright.cure(false, false);
        assert_eq!(upright.immunity, 54_000, "IMMUNITY_TIME / 2 at :250");
        // The vaccines bonus is inside the asleep branch only (`:238-245`).
        let mut upright_vaccinated = Sickness::new();
        upright_vaccinated.cure(false, true);
        assert_eq!(upright_vaccinated.immunity, 54_000);
    }

    #[test]
    fn a_sick_citizen_only_regenerates_to_a_third() {
        let sick = Sickness {
            disease: Some(Disease::SMALLPOX),
            ..Sickness::new()
        };
        assert!((sick.health_ceiling() - MAX_HEALTH / 3.0).abs() < 1e-6);
        assert_eq!(Sickness::new().health_ceiling(), MAX_HEALTH);
    }

    #[test]
    fn the_job_modifiers_are_the_four_overrides_and_the_composter_slides() {
        assert_eq!(job_modifier(None, 50), 1.0);
        assert_eq!(job_modifier(Some(Job::Healer), 50), 0.0);
        assert_eq!(job_modifier(Some(Job::Miner), 50), 2.0);
        assert_eq!(job_modifier(Some(Job::SwineHerder), 50), 2.0);
        assert_eq!(job_modifier(Some(Job::Builder), 50), 1.0);
        // `(int)((100 - skill) / 25)` truncates: 0, 1, 2, 3, 4.
        for (skill, expected) in [(99, 0.0), (75, 1.0), (50, 2.0), (26, 2.0), (25, 3.0), (0, 4.0)]
        {
            assert_eq!(
                job_modifier(Some(Job::Composter), skill),
                expected,
                "skill {skill}"
            );
        }
    }

    #[test]
    fn a_varied_diet_makes_a_citizen_harder_to_infect() {
        let short = FoodHistory::Short;
        assert_eq!(modifier(1.0, &short), 1.0, "no history, no opinion");
        assert_eq!(modifier(0.0, &short), 0.0, "a doctor stays a doctor");
        let meal = |diversity| FoodHistory::Full {
            diversity,
            quality: 0,
        };
        // 5 kinds is the neutral point: 0.5 · min(2.5, 1.0).
        assert_eq!(modifier(1.0, &meal(5)), 0.5);
        assert_eq!(modifier(1.0, &meal(10)), 0.25);
        // One kind: 0.5 · min(2.5, 5.0) = 1.25, above the base.
        assert_eq!(modifier(1.0, &meal(1)), 1.25);
        // And the cap holds for a two-kind diet too: 0.5 · 2.5.
        assert_eq!(modifier(1.0, &meal(2)), 1.25);
        assert_eq!(modifier(2.0, &meal(5)), 1.0, "the miner's 2 halves to 1");
    }

    #[test]
    fn the_two_research_effect_ids_exist_in_the_tree() {
        for effect in [MASKS_EFFECT, VACCINES_EFFECT] {
            assert!(
                crate::research::effect::category(effect).is_some(),
                "the port's research catalogue must know {effect}"
            );
        }
    }

    #[test]
    fn the_two_slow_cures_keep_their_own_boundaries() {
        // The bedridden roll: one chance in 7 200, so only a zero cures.
        assert!(Sickness::self_cures(0));
        assert!(!Sickness::self_cures(1));
        assert!(!Sickness::self_cures(SELF_CURE_IN - 1));
        assert_eq!(SELF_CURE_IN, 7_200, "60 · 60 · 2 at EntityAISickTask.java:333");
        // The doctor's: thirty points of progress, and the thirtieth lands it.
        assert!(!Sickness::doctor_cures(HEALER_TICKS - 1));
        assert!(Sickness::doctor_cures(HEALER_TICKS));
        assert!(Sickness::doctor_cures(HEALER_TICKS + 100));
        assert_eq!(HEALER_TICKS, 30, "MAX_PROGRESS_TICKS at EntityAIWorkHealer.java:63");
        // And the collision bound the caller must stay under.
        assert_eq!(CONTAGION_IN, 100);
    }

    #[test]
    fn forced_illness_is_refused_where_the_roll_would_have_been() {
        let gate = Gate::of(10);
        let mut sickness = Sickness::new();
        assert!(sickness.catch(Disease::MEASLES, &gate).is_ok());
        assert_eq!(sickness.disease.map(|d| d.id), Some("minecolonies:measles"));
        // A second one is refused, and the refusal says which disease.
        let refused = sickness.catch(Disease::SMALLPOX, &gate);
        assert_eq!(
            refused,
            Err(Refusal::GateClosed {
                id: "minecolonies:smallpox"
            })
        );
        assert_eq!(sickness.disease.map(|d| d.id), Some("minecolonies:measles"));
        // So is a healer's own attempt.
        let doctor_gate = Gate {
            healer: true,
            ..gate
        };
        assert!(Sickness::new().catch(Disease::MEASLES, &doctor_gate).is_err());
    }
}
