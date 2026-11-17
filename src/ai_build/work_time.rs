//! How long one block takes: the mining delay, the placing delay, and the hits a
//! crafter needs before its recipe fires.
//!
//! Java reference: `AbstractEntityAIInteract.java:308-343` (the mining delay and
//! its two branches), `EntityAIStructureBuilder.java:278-282` (the builder's speed
//! buff, applied **after** the parent truncated), `AbstractEntityAIStructure.java:
//! 531-532` (the placing delay set at the end of every structure step),
//! `AbstractEntityAIStructure.java:86` (`BUILD_BLOCK_DELAY`),
//! `CitizenConstants.java:261` (`PROGRESS_MULTIPLIER`) and
//! `AbstractEntityAICrafting.java:73-88` + `:734-738` (the crafting progress, in
//! hits, and `HIT_DELAY` between two of them).
//!
//! **Why this is its own file.** Three AI classes ask "how long", each with a
//! formula of its own, and all three are *integer* formulas written in Java: the
//! order of the truncations is part of the answer, not a rounding detail. Java
//! computes `(int)(a / b * c)`, and `a / b` is already an `int` division in
//! `place_ticks` (`:532`) — so `place_level = 19` gives 7, not the 7.89 a
//! calculator says. A port that used doubles and truncated at the end would quietly
//! make every builder 12 % faster at the top of the scale. Every function below
//! therefore says **where** the truncation happens.
//!
//! **The two numbers that look like bugs, copied as they are.**
//!
//! - `getBlockMiningTime` in PvP mode answers a flat `BLOCK_MINING_DELAY / 2` —
//!   250 ticks — whatever the block, the tool or the level (`:322-324`). The
//!   hardness is never read. It is the server's "fast mining" switch and it is not
//!   this port's business to make it clever.
//! - `craft_hits` divides an `int` **before** multiplying: `10 / n * 3`
//!   (`AbstractEntityAICrafting.java:737`). From skill level 18 on, `10 / (18 / 2 +
//!   1) = 10 / 19 = 0`, so the crafter needs **zero** hits and the recipe fires on
//!   the first `CRAFT` tick. That is what Java does; the test
//!   `the_craft_progress_is_an_integer_division_and_can_reach_zero` holds it.
//!
//! **What is not here.** The delay *machine*: `setDelay` stores a counter and
//! `hasNotDelayed` reads it, which `crate::ai::worker` already carries
//! (`Memory::delay`, `Memory::has_delayed`, `set_delay`, `has_not_delayed`). This
//! module answers the duration and nothing else; it never decrements.
//!
//! Every function returns a `Result` rather than a number because the caller can
//! hand over facts Java never sees: a negative hardness (bedrock, which the AI
//! refuses to mine at `AbstractEntityAIInteract.java:160-166` before any delay is
//! asked for), a tool with no harvest speed, a negative skill level, a research
//! effect outside `0..=1`. Java would compute a nonsense delay or divide by zero;
//! the port names the refusal.

use core::fmt;

/// `AbstractEntityAIInteract.BLOCK_MINING_DELAY` (`AbstractEntityAIInteract.java:
/// 100`): the base of the mining formula, 500 ticks for a block of hardness 1 with
/// a tool of speed 1.
pub const BLOCK_MINING_DELAY: u32 = 500;

/// `AbstractEntityAIInteract.LEVEL_MODIFIER` (`:54`): each level of the breaking
/// skill multiplies the delay by `0.85^n`, where `n` is *half* the level (`:338`
/// passes `getBreakSpeedLevel() / 2.0`, a **double** division).
pub const LEVEL_MODIFIER: f64 = 0.85;

/// `BLOCK_MINING_DELAY / 2` (`:322-324`): the flat delay PvP mode gives every
/// block. Kept as the divisor Java writes so the two places agree.
pub const PVP_DIVISOR: u32 = 2;

/// `AbstractEntityAIStructure.BUILD_BLOCK_DELAY` (`AbstractEntityAIStructure.java:
/// 86`): the base placing delay, 15 ticks.
pub const BUILD_BLOCK_DELAY: u32 = 15;

/// `CitizenConstants.PROGRESS_MULTIPLIER` (`CitizenConstants.java:261`): the
/// constant that damps the placing delay by the placing skill (`:532`). It is
/// **also** the constant `AbstractEntityAICrafting` redeclares for its own use
/// (`AbstractEntityAICrafting.java:78`); the two have the same value and this port
/// keeps them separate, as Java does.
pub const PROGRESS_MULTIPLIER: u32 = 10;

/// `EntityAIStructureBuilder.SPEED_BUFF_0` (`EntityAIStructureBuilder.java:45`):
/// the builder's mining multiplier at depth zero (`:278-282`).
pub const SPEED_BUFF_0: f64 = 0.5;

/// `AbstractEntityAICrafting.HIT_DELAY` (`AbstractEntityAICrafting.java:73`): the
/// rate of the `CRAFT` target (`:169`), i.e. one hit every ten ticks.
pub const CRAFT_HIT_DELAY: u32 = 10;

/// `AbstractEntityAICrafting.PROGRESS_MULTIPLIER` (`:78`): the numerator of the
/// progress formula (`:737`).
pub const CRAFT_PROGRESS_MULTIPLIER: u32 = 10;

/// `AbstractEntityAICrafting.MAX_LEVEL` (`:83`): the cap on the skill term of the
/// progress formula (`:737`).
pub const CRAFT_MAX_LEVEL: u32 = 50;

/// `AbstractEntityAICrafting.HITTING_TIME` (`:88`): the progress formula is
/// multiplied by 3 (`:737`), so a level-1 crafter hits 30 times.
pub const HITTING_TIME: u32 = 3;

/// Where the numbers above come from, as one string, so a log line or a doc page
/// can print the provenance instead of restating it.
pub const SOURCE: &str = "AbstractEntityAIInteract.java:100,:54,:322-324 · \
AbstractEntityAIStructure.java:86,:531-532 · CitizenConstants.java:261 · \
EntityAIStructureBuilder.java:45,:278-282 · AbstractEntityAICrafting.java:73-88,:734-738";

/// Why a work time could not be computed, named.
///
/// Four of these are the caller handing over a value the game cannot produce (a
/// negative skill level, a tool of speed zero, a research effect of 1.5); one is
/// the block the AI must not mine at all. None is a `false`, and none is an
/// `unwrap`: a wrong number of ticks is a time the citizen spends, which is not a
/// crash but is wrong in a way nobody sees.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own refusal set for the three formulas, closed on purpose
pub enum Refusal {
    /// The block has no valid mining speed. Vanilla answers `-1` for bedrock and
    /// for anything unbreakable; `AbstractEntityAIInteract.java:160-166` returns
    /// *before* asking for a delay in that case, so reaching this refusal means the
    /// caller skipped that guard.
    Unbreakable { hardness: f64 },
    /// A tool whose `getDestroySpeed` is zero would divide the formula by zero
    /// (`:339-341`).
    ToolSpeedNotPositive { tool_speed: f64 },
    /// A skill level below zero. Java's skill handler never reports one
    /// (`AbstractEntityAIBasic.getPrimarySkillLevel`); a fact that does is a broken
    /// observation, not a slow worker.
    NegativeLevel { level: i32 },
    /// A research effect outside `0..=1`. Java subtracts it from one and multiplies
    /// the delay (`:336`, `AbstractEntityAIStructure.java:531`); above 1 that is a
    /// negative delay, below 0 an amplified one. Neither is a speed the game means.
    ReductionOutOfRange { strength: f64 },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Unbreakable { hardness } => {
                write!(f, "block hardness {hardness} is not a mining speed")
            }
            Refusal::ToolSpeedNotPositive { tool_speed } => {
                write!(f, "tool destroy speed {tool_speed} divides by zero")
            }
            Refusal::NegativeLevel { level } => write!(f, "skill level {level} is below zero"),
            Refusal::ReductionOutOfRange { strength } => {
                write!(f, "research effect {strength} is outside 0..=1")
            }
        }
    }
}

/// The facts `getBlockMiningTime` reads: the block, the tool, the skill, the
/// research, and whether this worker is the builder.
///
/// Java reference: `AbstractEntityAIInteract.java:315-343` reads
/// `state.getDestroySpeed(world, pos)`, `worker.getMainHandItem().getItem()
/// .getDestroySpeed(...)`, `getBreakSpeedLevel()` and the `BLOCK_BREAK_SPEED`
/// research strength; `EntityAIStructureBuilder.java:279-282` adds the builder's
/// own halving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiningTime {
    /// `state.getDestroySpeed(world, pos)` — the block's hardness.
    pub hardness: f64,
    /// `worker.getMainHandItem().getItem().getDestroySpeed(stack, state)`.
    pub tool_speed: f64,
    /// `getBreakSpeedLevel()`: the secondary skill for the builder
    /// (`EntityAIStructureBuilder.java:74-77`), the primary one everywhere else
    /// (`AbstractEntityAIInteract.java:350-353`).
    pub break_level: i32,
    /// `MineColonies.getConfig().getServer().pvp_mode.get()` (`:322`).
    pub pvp: bool,
    /// `getEffectStrength(BLOCK_BREAK_SPEED)`: the research **strength** itself, not
    /// the `1 - strength` Java computes at `:336`. The caller gives the number the
    /// colony stores and this port subtracts it once, here, so a test can read the
    /// Java figure.
    pub reduction: f64,
    /// `this instanceof EntityAIStructureBuilder` (`EntityAIStructureBuilder.java:
    /// 278-282`): the builder halves the answer of its parent.
    pub builder_buff: bool,
}

impl Default for MiningTime {
    /// A block of hardness 1 mined by a tool of speed 1, skill 0, no research, not
    /// a builder: the `BLOCK_MINING_DELAY` itself, 500 ticks.
    fn default() -> MiningTime {
        MiningTime {
            hardness: 1.0,
            tool_speed: 1.0,
            break_level: 0,
            pvp: false,
            reduction: 0.0,
            builder_buff: false,
        }
    }
}

/// `getBlockMiningTime` (`AbstractEntityAIInteract.java:315-325`) followed by the
/// builder's override (`EntityAIStructureBuilder.java:278-282`).
///
/// The order is Java's, and it matters twice:
///
/// 1. PvP mode answers `BLOCK_MINING_DELAY / 2` and **stops** (`:322-324`): the
///    hardness, the tool and the level are not read at all.
/// 2. Otherwise the formula is `500 × 0.85^(level/2) × hardness / tool_speed ×
///    (1 - reduction)` (`:338-342`), truncated once as a whole.
/// 3. A builder then halves the **truncated** integer (`:281`). For a halving the
///    two truncations agree, so the test asserts the composition rather than the
///    order — and says so.
pub fn mining_ticks(time: MiningTime) -> Result<u32, Refusal> {
    // `:322-324` first: PvP mode never looks at the block.
    if time.pvp {
        return Ok(BLOCK_MINING_DELAY / PVP_DIVISOR);
    }
    if time.hardness < 0.0 {
        // Vanilla's "unbreakable" sentinel; `:160-166` is where the AI refuses it.
        return Err(Refusal::Unbreakable {
            hardness: time.hardness,
        });
    }
    if time.tool_speed <= 0.0 {
        return Err(Refusal::ToolSpeedNotPositive {
            tool_speed: time.tool_speed,
        });
    }
    if time.break_level < 0 {
        return Err(Refusal::NegativeLevel {
            level: time.break_level,
        });
    }
    if !(0.0..=1.0).contains(&time.reduction) {
        return Err(Refusal::ReductionOutOfRange {
            strength: time.reduction,
        });
    }
    // `getBreakSpeedLevel() / 2.0` (`:338`) — a double division, unlike the placing
    // skill's `placeLevel / 2` (`AbstractEntityAIStructure.java:532`).
    let exponent = f64::from(time.break_level) / 2.0;
    let factor = LEVEL_MODIFIER.powf(exponent);
    // One truncation, at the end, as Java's single `(int)` cast (`:338`).
    let ticks =
        f64::from(BLOCK_MINING_DELAY) * factor * time.hardness / time.tool_speed * (1.0 - time.reduction);
    let ticks = ticks as u32;
    if time.builder_buff {
        // `(int) (super.getBlockMiningTime(state, pos) * SPEED_BUFF_0)`
        // (`EntityAIStructureBuilder.java:281`).
        Ok((f64::from(ticks) * SPEED_BUFF_0) as u32)
    } else {
        Ok(ticks)
    }
}

/// The delay `structureStep` sets after a successful block (`AbstractEntityAIStructure.java:
/// 531-532`).
///
/// Java: `(int) ((BUILD_BLOCK_DELAY * PROGRESS_MULTIPLIER / (getPlaceSpeedLevel()
/// / 2 + PROGRESS_MULTIPLIER)) * decrease)`. Every term of the first product is an
/// `int`, so the division truncates **before** `decrease` is applied — level 19
/// gives `150 / 19 = 7`, then `7 × decrease`.
///
/// `reduction` is the `BLOCK_PLACE_SPEED` research strength
/// (`AbstractEntityAIStructure.java:531`, `1 - getEffectStrength(...)`).
pub fn place_ticks(place_level: i32, reduction: f64) -> Result<u32, Refusal> {
    if place_level < 0 {
        return Err(Refusal::NegativeLevel { level: place_level });
    }
    if !(0.0..=1.0).contains(&reduction) {
        return Err(Refusal::ReductionOutOfRange {
            strength: reduction,
        });
    }
    // `getPlaceSpeedLevel() / 2` — an int division, `:532`.
    let half = place_level / 2;
    let divisor = half + PROGRESS_MULTIPLIER as i32;
    // `BUILD_BLOCK_DELAY * PROGRESS_MULTIPLIER / divisor`, all ints.
    let base = (BUILD_BLOCK_DELAY * PROGRESS_MULTIPLIER) as i32 / divisor;
    Ok((f64::from(base) * (1.0 - reduction)) as u32)
}

/// How many hits a crafter needs before its recipe fires
/// (`AbstractEntityAICrafting.java:734-738`).
///
/// Java: `PROGRESS_MULTIPLIER / Math.min(jobModifier + 1, MAX_LEVEL) * HITTING_TIME`
/// where `jobModifier = skillLevel / 2`. The division is an `int` division, so the
/// answer hits zero at skill level 20 (`10 / 11`) and stays there — `:561` then fires
/// the recipe on the very first `CRAFT` tick.
///
/// The work time in ticks is [`craft_ticks`], because `CRAFT` is looked at every
/// [`CRAFT_HIT_DELAY`] ticks (`:169`).
pub fn craft_hits(craft_speed_level: i32) -> Result<u32, Refusal> {
    if craft_speed_level < 0 {
        return Err(Refusal::NegativeLevel {
            level: craft_speed_level,
        });
    }
    // `jobModifier = ...getLevel(...) / 2` (`:735`), an int division. The level is
    // known to be at least zero here, so the cast cannot wrap.
    let modifier = craft_speed_level / 2;
    // `Math.min(jobModifier + 1, MAX_LEVEL)`: at least 1, at most 50.
    let clamped = ((modifier + 1) as u32).min(CRAFT_MAX_LEVEL);
    // `10 / clamped` truncates first; multiplying by 3 after is Java's order.
    Ok(CRAFT_PROGRESS_MULTIPLIER / clamped * HITTING_TIME)
}

/// The time one recipe takes, in game ticks: [`craft_hits`] × [`CRAFT_HIT_DELAY`].
///
/// This is a *lower bound*: `:523-526` returns early while the citizen walks to its
/// tagged work position, and `:284-288` returns early while it waits to dump. Those
/// two waits are the state machine's, not the formula's.
pub fn craft_ticks(craft_speed_level: i32) -> Result<u32, Refusal> {
    Ok(craft_hits(craft_speed_level)? * CRAFT_HIT_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AbstractEntityAIInteract.java:100`: 500 ticks for hardness 1 and tool
    /// speed 1, at skill 0.
    #[test]
    fn the_base_mining_time_is_the_java_constant() {
        assert_eq!(mining_ticks(MiningTime::default()), Ok(500));
        assert_eq!(BLOCK_MINING_DELAY, 500);
        assert_eq!(LEVEL_MODIFIER, 0.85);
    }

    /// `:338`: `500 × 0.85^(level/2) × hardness / tool`, truncated once. Level 4 is
    /// `0.85^2 = 0.7225`; hardness 2 over tool speed 4 gives `180.625` → 180.
    #[test]
    fn the_level_modifier_is_the_half_level_power() {
        let time = MiningTime {
            hardness: 2.0,
            tool_speed: 4.0,
            break_level: 4,
            ..MiningTime::default()
        };
        assert_eq!(mining_ticks(time), Ok(180));
        // An odd level keeps the half step: `:338` divides by 2.0, not by 2.
        let odd = MiningTime {
            break_level: 3,
            ..MiningTime::default()
        };
        // 500 × 0.85^1.5 = 500 × 0.7837… = 391.8… → 391.
        assert_eq!(mining_ticks(odd), Ok(391));
    }

    /// `EntityAIStructureBuilder.java:278-282`: a builder halves what its parent
    /// answered. `floor(floor(x) / 2) == floor(x / 2)` for positive `x`, so the test
    /// pins the composition, not the truncation order.
    #[test]
    fn the_builder_halves_the_mining_time() {
        let miner = MiningTime {
            hardness: 2.0,
            tool_speed: 4.0,
            break_level: 4,
            ..MiningTime::default()
        };
        let builder = MiningTime {
            builder_buff: true,
            ..miner
        };
        assert_eq!(mining_ticks(miner), Ok(180));
        assert_eq!(mining_ticks(builder), Ok(90));
        assert_eq!(SPEED_BUFF_0, 0.5);
    }

    /// `:322-324`: PvP mode is a flat `BLOCK_MINING_DELAY / 2`, and it is checked
    /// *before* anything else — so a negative hardness is not even looked at.
    #[test]
    fn pvp_mode_is_a_flat_250_ticks_whatever_the_block() {
        let pvp = MiningTime {
            hardness: -1.0,
            tool_speed: 0.0,
            break_level: -5,
            pvp: true,
            reduction: 7.0,
            builder_buff: true,
        };
        assert_eq!(mining_ticks(pvp), Ok(250));
        assert_eq!(mining_ticks(MiningTime { pvp: true, ..MiningTime::default() }), Ok(250));
    }

    /// Vanilla's `-1` hardness is refused instead of becoming a negative delay; the
    /// AI is supposed to stop at `AbstractEntityAIInteract.java:160-166`.
    #[test]
    fn an_unbreakable_block_is_refused() {
        let bedrock = MiningTime {
            hardness: -1.0,
            ..MiningTime::default()
        };
        assert_eq!(
            mining_ticks(bedrock),
            Err(Refusal::Unbreakable { hardness: -1.0 })
        );
    }

    /// `:339-341` divides by the tool's destroy speed: zero is a refusal, not a
    /// division by zero.
    #[test]
    fn a_tool_of_speed_zero_is_refused() {
        let no_tool = MiningTime {
            tool_speed: 0.0,
            ..MiningTime::default()
        };
        assert_eq!(
            mining_ticks(no_tool),
            Err(Refusal::ToolSpeedNotPositive { tool_speed: 0.0 })
        );
    }

    /// A negative skill level is a broken observation, not a fast worker.
    #[test]
    fn a_negative_level_is_refused_in_both_formulas() {
        let level = MiningTime {
            break_level: -2,
            ..MiningTime::default()
        };
        assert_eq!(
            mining_ticks(level),
            Err(Refusal::NegativeLevel { level: -2 })
        );
        assert_eq!(place_ticks(-1, 0.0), Err(Refusal::NegativeLevel { level: -1 }));
        assert_eq!(craft_hits(-1), Err(Refusal::NegativeLevel { level: -1 }));
    }

    /// The `BLOCK_BREAK_SPEED` research subtracts from one (`:336`); outside `0..=1`
    /// the delay would go negative or amplify, so it is refused by name.
    #[test]
    fn a_research_effect_outside_its_range_is_refused() {
        let half = MiningTime {
            reduction: 0.5,
            ..MiningTime::default()
        };
        assert_eq!(mining_ticks(half), Ok(250));
        assert_eq!(
            mining_ticks(MiningTime {
                reduction: 1.5,
                ..MiningTime::default()
            }),
            Err(Refusal::ReductionOutOfRange { strength: 1.5 })
        );
        assert_eq!(
            place_ticks(0, -0.1),
            Err(Refusal::ReductionOutOfRange { strength: -0.1 })
        );
    }

    /// `AbstractEntityAIStructure.java:531-532`: 150 / (level/2 + 10), an int
    /// division, then the research factor.
    #[test]
    fn the_placing_delay_is_an_integer_division() {
        assert_eq!(place_ticks(0, 0.0), Ok(15)); // 150 / 10
        assert_eq!(place_ticks(2, 0.0), Ok(13)); // 150 / 11 = 13.63 → 13
        assert_eq!(place_ticks(19, 0.0), Ok(7)); // 150 / 19 = 7.89 → 7
        assert_eq!(place_ticks(1000, 0.0), Ok(0)); // 150 / 510 = 0
    }

    /// The research factor is applied **after** the truncation: level 2 is 13, and
    /// 13 × 0.5 = 6.5 → 6, where a single division would have given 6.8 → 6 as
    /// well — the test pins the pair, and the level-19 case above shows the
    /// truncation is real.
    #[test]
    fn the_placing_delay_applies_the_research_after_truncating() {
        assert_eq!(place_ticks(2, 0.5), Ok(6));
        assert_eq!(place_ticks(0, 0.5), Ok(7)); // 15 × 0.5
        assert_eq!(place_ticks(0, 1.0), Ok(0)); // fully researched
    }

    /// `AbstractEntityAICrafting.java:737`: `10 / (level/2 + 1) * 3`, the division
    /// first. Level 0 needs 30 hits, level 2 needs 15, level 4 needs 9.
    #[test]
    fn the_craft_progress_counts_hits_per_level() {
        assert_eq!(craft_hits(0), Ok(30));
        assert_eq!(craft_hits(2), Ok(15));
        assert_eq!(craft_hits(4), Ok(9));
        assert_eq!(craft_hits(6), Ok(6)); // 10 / 4 = 2, × 3
        assert_eq!(craft_ticks(6), Ok(60)); // 6 hits × HIT_DELAY 10
    }

    /// The quirk: at skill 18 the divisor is `18/2 + 1 = 10`, so `10 / 10 = 1` and
    /// the crafter still hits 3 times; at level 20 the divisor is 11 and the product
    /// is **zero**, so the recipe fires on the first `CRAFT` tick (`:561`).
    #[test]
    fn the_craft_progress_is_an_integer_division_and_can_reach_zero() {
        assert_eq!(craft_hits(18), Ok(3));
        assert_eq!(craft_hits(20), Ok(0));
        assert_eq!(craft_hits(98), Ok(0));
        // And it cannot go below one hit worth of ticks, only to zero.
        assert_eq!(craft_ticks(20), Ok(0));
        // The skill term is capped at MAX_LEVEL: level 300 is level 50's divisor.
        assert_eq!(craft_hits(300), Ok(0));
    }

    /// The provenance string names the files a reviewer opens, and the constants it
    /// claims to cover are the ones this file holds.
    #[test]
    fn the_source_string_lists_the_files_the_constants_come_from() {
        assert!(SOURCE.contains("AbstractEntityAIInteract.java:100"));
        assert!(SOURCE.contains("AbstractEntityAIStructure.java:86"));
        assert!(SOURCE.contains("AbstractEntityAICrafting.java:73-88"));
        // A refusal prints a sentence, never a bare code.
        assert!(Refusal::Unbreakable { hardness: -1.0 }.to_string().contains("hardness"));
    }
}
