//! `citizenutils/CitizenItemUtils`: how much a hit costs the gear, and how much XP
//! mending eats.
//!
//! Java reference: `core/util/citizenutils/CitizenItemUtils.java` (357 lines, the
//! only file of this mission under a sub-package). Eleven of its thirteen methods
//! move an `AbstractEntityCitizen`: `swing`, `getLookControl().setLookAt`,
//! `Network.getNetwork().sendToPosition(new BlockParticleEffectMessage(...))`,
//! `world.playSound`, `spawnAtLocation`. Those are the caller's, whole.
//!
//! What is left is **three arithmetic rules** and they are the ones that decide
//! whether a colony's gear survives:
//!
//! | Rule | Java |
//! | --- | --- |
//! | a hit costs `max(1, damage / 4)` armour durability | `:312` |
//! | a durability research sometimes skips the damage entirely | `:221-231`, `:300-310` |
//! | mending spends 2 XP per point of damage, `ceil` on the repair | `:325-356` |
//!
//! # The research gate, read carefully
//!
//! ```java
//! if (getEffectStrength(TOOL_DURABILITY) > 0) {
//!     if (random.nextDouble() > (1 / (1 + getEffectStrength(TOOL_DURABILITY)))) return;
//! }
//! ```
//!
//! `nextDouble()` is uniform on `[0, 1)` and the test is **strictly greater**, so
//! the probability of *skipping* the damage is `1 - 1/(1+s)`: with `s = 1` half the
//! hits are free, with `s = 3` three quarters, and with `s = 0` the outer `if` has
//! already refused to roll at all — **the random generator is not even called**,
//! which is what the comment at `:220` says ("to avoid unnecessary calls to random
//! number generator") and what makes the gate testable without a generator.
//!
//! # The mending asymmetry
//!
//! `dmgHealed = min(localXp / 2, damage)` is a `double`, the XP charge is
//! `dmgHealed * 2`, and the repair is `(int) Math.ceil(dmgHealed)` — so **half a
//! point of healing repairs a whole point of damage** while costing exactly the
//! half point it healed. A tool at 1 damage and 1 XP left comes out undamaged. That
//! is Java's answer and the test pins it.

/// How much armour durability one hit costs at minimum:
/// `Math.max(1, (int) (damage / 4))` (`CitizenItemUtils.java:312`) — the `1` is
/// what stops light hits from being free.
pub const MIN_ARMOR_DAMAGE: i32 = 1;

/// The divisor of a hit: `damage / 4` (`:312`).
pub const ARMOR_DAMAGE_DIVISOR: f64 = 4.0;

/// The volume a pickup sound is played at, `Constants.DEFAULT_VOLUME = 0.2`
/// (`api/util/constant/Constants.java:60`, used at `CitizenItemUtils.java:65`).
pub const PICKUP_VOLUME: f64 = 0.2;

/// The pitch spread of a pickup sound, `Constants.DEFAULT_PITCH_MULTIPLIER = 0.7D`
/// (`Constants.java:61`, used at `CitizenItemUtils.java:66`).
pub const PICKUP_PITCH_MULTIPLIER: f64 = 0.7;

/// How much durability a hit takes off the armour: `updateArmorDamage`
/// (`CitizenItemUtils.java:312`).
///
/// ```java
/// final int armorDmg = Math.max(1, (int) (damage / 4));
/// ```
///
/// Two truncations and a floor: `damage / 4` is a `double`, `(int)` truncates
/// towards zero, and the `max(1, …)` means any hit at all costs at least one
/// point. So `3.9` damage costs `1`, `4.0` costs `1`, `7.9` costs `1`, `8.0` costs
/// `2`.
///
/// A **negative** damage gives a negative quotient, which `max(1, …)` flattens to
/// `1` — healing an entity with a negative hit still wears its armour by one, which
/// is Java's answer for input Java never produces.
pub fn armor_damage(damage: f64) -> i32 {
    let truncated = (damage / ARMOR_DAMAGE_DIVISOR) as i32;
    if truncated > MIN_ARMOR_DAMAGE {
        truncated
    } else {
        MIN_ARMOR_DAMAGE
    }
}

/// Does the damage actually land, given the research strength and one uniform
/// roll: the gate of `updateArmorDamage` (`:300-310`) and of `damageItemInHand`
/// (`:221-231`).
///
/// ```java
/// if (strength > 0 && roll > (1 / (1 + strength))) return;   // skipped
/// ```
///
/// `true` = the damage is applied. The comparison is **strictly greater**, so a
/// roll of exactly `1/(1+s)` still takes the damage — a measure-zero case that the
/// test pins because it is the one place a `>=` would go unnoticed.
///
/// `strength <= 0` short-circuits **before** the roll, so a caller that reads the
/// roll lazily never pays for it (`:220`).
pub fn durability_damage_applied(strength: f64, roll: f64) -> bool {
    if strength <= 0.0 {
        return true;
    }
    !(roll > 1.0 / (1.0 + strength))
}

/// The pitch a pickup sound is played at, from one Gaussian draw:
/// `CitizenItemUtils.java:66`.
///
/// ```java
/// (float) ((citizen.getRandom().nextGaussian() * DEFAULT_PITCH_MULTIPLIER + 1.0D) * 2.0D)
/// ```
///
/// A Gaussian around `1` with a spread of `0.7`, then doubled: the usual pitch
/// range of a `playSound` call. Vanilla clamps the result at the sound engine, not
/// here, so a wild draw gives a wild number — that is what this reproduces.
pub fn pickup_pitch(gaussian: f64) -> f64 {
    (gaussian * PICKUP_PITCH_MULTIPLIER + 1.0) * 2.0
}

/// Spend XP to repair enchanted gear, in slot order: `applyMending`
/// (`CitizenItemUtils.java:325-356`).
///
/// ```java
/// for (final EquipmentSlot slot : EquipmentSlot.values()) {
///     if (localXp <= 0) break;
///     final ItemStack tool = ...;
///     if (!isEmpty(tool) && tool.isDamaged() && tool.isEnchanted() && hasMending(tool)) {
///         final double dmgHealed = Math.min(localXp / 2, tool.getDamageValue());
///         localXp -= dmgHealed * 2;
///         tool.setDamageValue(tool.getDamageValue() - (int) Math.ceil(dmgHealed));
///     }
/// }
/// return localXp;
/// ```
///
/// `damages` is one entry per slot **in `EquipmentSlot.values()` order**, `Some`
/// for a tool that is damaged *and* carries mending, `None` for everything else
/// (empty slot, undamaged tool, no mending). The check `isDamaged() &&
/// isEnchanted() && hasMending` is the caller's — it reads enchantments, which is
/// the item layer's business.
///
/// Returns the XP left over; the damages are updated in place.
///
/// **Three consequences of the arithmetic, all Java's:**
///
/// 1. the loop **breaks** when the XP is gone, so later slots get nothing even if
///    an earlier one was only half repaired;
/// 2. `setDamageValue(damage - ceil(dmgHealed))` rounds the *repair* **up**, so
///    `2.5` of healing removes `3` damage — and `5` XP buys those `2.5`;
/// 3. a tool whose damage reaches `0` is repaired, and the remaining XP is **not**
///    refunded: the charge is `dmgHealed * 2`, computed before the rounding.
pub fn apply_mending(xp: f64, damages: &mut [Option<u32>]) -> f64 {
    let mut left = xp;
    for slot in damages.iter_mut() {
        if left <= 0.0 {
            break;
        }
        let damage = match *slot {
            Some(damage) => damage,
            None => continue,
        };
        let healed = (left / 2.0).min(f64::from(damage));
        left -= healed * 2.0;
        let repaired = healed.ceil() as u32;
        *slot = Some(damage.saturating_sub(repaired));
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The truncation before the floor: below 8 damage, one point.
    #[test]
    fn a_light_hit_costs_one_armour_point() {
        assert_eq!(armor_damage(0.0), 1);
        assert_eq!(armor_damage(3.9), 1);
        assert_eq!(armor_damage(4.0), 1);
        assert_eq!(armor_damage(7.9), 1);
    }

    /// From 8 damage on, the quotient is the answer.
    #[test]
    fn a_heavy_hit_costs_a_quarter_of_its_damage() {
        assert_eq!(armor_damage(8.0), 2);
        assert_eq!(armor_damage(11.9), 2);
        assert_eq!(armor_damage(12.0), 3);
        assert_eq!(armor_damage(40.0), 10);
    }

    /// A negative hit still wears the armour by one, because the floor is applied
    /// after the truncation.
    #[test]
    fn a_negative_hit_still_costs_one_point() {
        assert_eq!(armor_damage(-4.0), 1);
        assert_eq!(armor_damage(-100.0), 1);
    }

    /// No research means no roll and no skip.
    #[test]
    fn without_research_the_damage_always_lands() {
        assert!(durability_damage_applied(0.0, 0.0));
        assert!(durability_damage_applied(0.0, 0.99));
        assert!(durability_damage_applied(-1.0, 0.99));
    }

    /// The roll is compared to `1/(1+s)` with a **strict** greater-than: only a
    /// roll **above** the threshold skips the damage (`CitizenItemUtils.java:223`,
    /// `:302` — `if (random.nextDouble() > (1 / (1 + strength))) return;`).
    #[test]
    fn the_gate_splits_the_roll_at_one_over_one_plus_the_strength() {
        // s = 1: the threshold is 0.5, so the rolls above 0.5 are the ones that
        // skip. `0.49` and the boundary `0.5` still take the damage, because the
        // comparison at `:223` is `>`, not `>=`.
        assert!(durability_damage_applied(1.0, 0.49));
        assert!(durability_damage_applied(1.0, 0.5), "the boundary applies the damage");
        assert!(!durability_damage_applied(1.0, 0.51));
        // s = 3: the threshold is 0.25, so three quarters skip it.
        assert!(durability_damage_applied(3.0, 0.25));
        assert!(!durability_damage_applied(3.0, 0.26));
    }

    /// A strength so large that the threshold is under every ordinary roll makes
    /// the research total — `1/(1+1e9)` is `1e-9`.
    #[test]
    fn a_huge_strength_almost_never_damages() {
        assert!(!durability_damage_applied(1e9, 0.999_999));
        // The one roll that still lands: `0.0` is not `> 1e-9`, and the test at
        // `CitizenItemUtils.java:223` skips only on a **greater** roll.
        assert!(durability_damage_applied(1e9, 0.0));
    }

    /// The pickup pitch is doubled and centred on one.
    #[test]
    fn the_pickup_pitch_is_doubled_around_one() {
        assert_eq!(pickup_pitch(0.0), 2.0);
        assert!((pickup_pitch(1.0) - 3.4).abs() < 1e-9);
        assert!((pickup_pitch(-1.0) - 0.6).abs() < 1e-9);
    }

    /// Two XP per point of damage, spent in slot order until it runs out.
    #[test]
    fn mending_spends_two_xp_per_point_and_stops_when_the_xp_does() {
        let mut damages = [Some(3), Some(100), None];
        let left = apply_mending(10.0, &mut damages);
        // 3 damage costs 6, then 2 more of the 4 left repair 2 → 98, and 0 left.
        assert_eq!(left, 0.0);
        assert_eq!(damages[0], Some(0));
        assert_eq!(damages[1], Some(98));
        assert_eq!(damages[2], None);
    }

    /// The `ceil`: half a point of healing takes a whole point of damage off.
    #[test]
    fn half_a_point_of_healing_repairs_a_whole_point() {
        let mut damages = [Some(1)];
        let left = apply_mending(1.0, &mut damages);
        // healed = min(0.5, 1) = 0.5; charged 1.0; repaired `ceil(0.5)` = 1.
        assert_eq!(left, 0.0);
        assert_eq!(damages[0], Some(0));
    }

    /// A tool that is not a mending candidate is skipped without spending XP.
    #[test]
    fn a_slot_without_mending_costs_nothing() {
        let mut damages = [None, Some(4)];
        let left = apply_mending(8.0, &mut damages);
        assert_eq!(left, 0.0);
        assert_eq!(damages[1], Some(0));
    }

    /// More XP than damage leaves a surplus: a repair never consumes what it did
    /// not heal.
    #[test]
    fn a_surplus_of_xp_is_returned() {
        let mut damages = [Some(2)];
        let left = apply_mending(20.0, &mut damages);
        assert_eq!(left, 16.0);
        assert_eq!(damages[0], Some(0));
    }

    /// With nothing to mend, every XP comes back.
    #[test]
    fn nothing_to_mend_returns_the_xp_whole() {
        let mut damages: [Option<u32>; 0] = [];
        assert_eq!(apply_mending(7.0, &mut damages), 7.0);
        assert_eq!(apply_mending(0.0, &mut [Some(5)]), 0.0);
    }
}
