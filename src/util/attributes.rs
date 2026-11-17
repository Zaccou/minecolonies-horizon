//! `AttributeModifierUtils`: which modifier goes, and what the entity's health
//! does when one does.
//!
//! Java reference: `core/util/AttributeModifierUtils.java` (121 lines, five
//! static methods). Every one of them takes a `LivingEntity` and calls
//! `entity.getAttribute(Attributes.MAX_HEALTH)`; what is left once that handle is
//! gone is **a list of named modifiers and one ratio**, which is what this file
//! is. The attribute itself — its base value, its operation, its clamp — is
//! vanilla's and stays there.
//!
//! # The one rule that is easy to lose
//!
//! `addHealthModifier` (`:247-260`) does not just add: it remembers the entity's
//! **health percentage** first, removes any modifier of the same name, adds the
//! new one, and puts the entity back at the same *fraction* of its new maximum.
//!
//! ```java
//! final float prevHealthPct = entity.getHealth() / entity.getMaxHealth();
//! removeHealthModifier(entity, modifier.getName());
//! entity.getAttribute(Attributes.MAX_HEALTH).addTransientModifier(modifier);
//! entity.setHealth(entity.getMaxHealth() * prevHealthPct);
//! ```
//!
//! A guard that grants `+10` max health therefore heals a wounded guard by the
//! same proportion it was wounded — from `10/20` (half) to `15/30` (half) — and a
//! guard at `0/20` stays at `0/30`. A port that only raised the maximum would heal
//! every soldier every time it changed post, which is a balance bug that looks
//! like a rounding difference.
//!
//! # `getModifiers()` is a copy, and that matters
//!
//! In 1.20.1 `AttributeInstance.getModifiers()` returns an **immutable copy** of
//! the live set, which is why Java may remove while iterating it (`:204-207`,
//! `:228-234`). The port takes `&mut Vec` and removes in one pass, with no
//! iteration hazard to survive — the outcome is the same set of survivors, which
//! is what the tests pin.

/// One attribute modifier, reduced to what these five methods read: the name they
/// match on (`AttributeModifier.getName`), and the amount the attribute adds.
// exhaustive: the two fields the matching and the health ratio use
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    /// The name `removeHealthModifier` matches (`:230`).
    pub name: String,
    /// The amount, as the attribute reads it. The *operation* (add, multiply
    /// base, multiply total) is the attribute's business and is not modelled.
    pub amount: f64,
}

impl Modifier {
    pub fn new(name: &str, amount: f64) -> Modifier {
        Modifier {
            name: name.to_owned(),
            amount,
        }
    }
}

/// Remove every modifier, health or otherwise:
/// `removeAllHealthModifiers` (`:197-213`).
///
/// Returns how many went, which Java cannot report and a caller that wants to
/// know whether anything changed does need. The second half of the Java method —
/// "if the entity's health is now above its maximum, put it back" — is
/// [`clamped_health`], because the maximum lives outside this list.
pub fn remove_all(modifiers: &mut Vec<Modifier>) -> usize {
    let removed = modifiers.len();
    modifiers.clear();
    removed
}

/// Remove every modifier with this name: `removeHealthModifier` (`:221-239`).
///
/// **Every** one, not the first: Java iterates the whole set and removes each
/// match, and vanilla's `addTransientModifier` can leave two modifiers with the
/// same name and different UUIDs. Removing one would leave the other adding health
/// for ever.
///
/// Returns how many were removed, so a caller can tell "nothing to do" from
/// "something was wrong".
pub fn remove_named(modifiers: &mut Vec<Modifier>, name: &str) -> usize {
    let before = modifiers.len();
    modifiers.retain(|modifier| modifier.name != name);
    before - modifiers.len()
}

/// The entity's health, capped at its maximum: the tail of
/// `removeAllHealthModifiers` and `removeHealthModifier` (`:209-212`, `:235-238`).
///
/// ```java
/// if (entity.getHealth() > entity.getMaxHealth()) entity.setHealth(entity.getMaxHealth());
/// ```
///
/// A cap and not a clamp: health **below** the maximum is left exactly where it
/// is, which is why a guard that lost its gear keeps its wounds. A NaN health
/// stays NaN — the comparison is false, Java does nothing, and so does this.
pub fn clamped_health(current: f64, max: f64) -> f64 {
    if current > max { max } else { current }
}

/// The health after adding a modifier, keeping the same fraction of the maximum:
/// `addHealthModifier` (`:247-260`).
///
/// ```java
/// final float prevHealthPct = entity.getHealth() / entity.getMaxHealth();
/// removeHealthModifier(entity, modifier.getName());
/// entity.getAttribute(Attributes.MAX_HEALTH).addTransientModifier(modifier);
/// entity.setHealth(entity.getMaxHealth() * prevHealthPct);
/// ```
///
/// The two maxima are both arguments because they are two *readings* of the same
/// attribute: `max_before` is what the entity had when the percentage was taken,
/// `max_after` is what the attribute reports once the modifier is in. Passing the
/// same number twice is a caller that did not re-read, not an error here.
///
/// The modifier list is updated in Java's order — **remove before add** — so the
/// "overwrite a modifier of the same name" behaviour comes out of the two calls
/// rather than out of a special case.
///
/// **The degenerate maximum.** A maximum of `0` (or less) makes the Java ratio
/// `current / 0`: `+Infinity`, `-Infinity` or `NaN`, and `setHealth` then stores
/// it. No living entity's max health is 0 — the attribute's base is 20 — so the
/// port answers the health it was given, unchanged, and says so here instead of
/// propagating a NaN into every later comparison.
pub fn add_health_modifier(
    modifiers: &mut Vec<Modifier>,
    modifier: Modifier,
    current_health: f64,
    max_before: f64,
    max_after: f64,
) -> f64 {
    let name = modifier.name.clone();
    remove_named(modifiers, &name);
    modifiers.push(modifier);
    if max_before <= 0.0 {
        return current_health;
    }
    let fraction = current_health / max_before;
    max_after * fraction
}

/// Remove a named modifier of **any** attribute: `removeModifier` (`:268-282`).
///
/// The same body as [`remove_named`], and that is the finding: Java has two copies
/// because one of them hard-codes `Attributes.MAX_HEALTH` (`:221-239`) and the
/// other takes the attribute as a parameter (`:268-282`). A list of modifiers does
/// not know which attribute it belongs to, so one function serves both.
pub fn remove_modifier(modifiers: &mut Vec<Modifier>, name: &str) -> usize {
    remove_named(modifiers, name)
}

/// Add a modifier of any attribute, replacing one of the same name:
/// `addModifier` (`:290-299`).
///
/// ```java
/// removeModifier(entity, modifier.getName(), attribute);
/// entity.getAttribute(attribute).addTransientModifier(modifier);
/// ```
///
/// Same shape as [`add_health_modifier`] minus the health ratio — which is the
/// other half of the finding above: of the five methods, exactly one touches
/// health.
pub fn add_modifier(modifiers: &mut Vec<Modifier>, modifier: Modifier) {
    let name = modifier.name.clone();
    remove_named(modifiers, &name);
    modifiers.push(modifier);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> Modifier {
        Modifier::new(name, 1.0)
    }

    #[test]
    fn removing_everything_empties_the_list_and_says_how_many_went() {
        let mut modifiers = vec![named("a"), named("b"), named("c")];
        assert_eq!(remove_all(&mut modifiers), 3);
        assert!(modifiers.is_empty());
        assert_eq!(remove_all(&mut modifiers), 0);
    }

    /// The named removal is exact: a prefix or a case change is a different name.
    #[test]
    fn only_the_exact_name_is_removed() {
        let mut modifiers = vec![named("guard_health"), named("guard_health_2")];
        assert_eq!(remove_named(&mut modifiers, "guard_health"), 1);
        assert_eq!(modifiers, vec![named("guard_health_2")]);
        assert_eq!(remove_named(&mut modifiers, "Guard_Health"), 0);
    }

    /// Two modifiers with the same name both go: one survivor would keep adding
    /// health for ever.
    #[test]
    fn duplicates_of_one_name_are_all_removed() {
        let mut modifiers = vec![named("dup"), named("dup"), named("other")];
        assert_eq!(remove_named(&mut modifiers, "dup"), 2);
        assert_eq!(modifiers, vec![named("other")]);
    }

    /// Health above the maximum is pulled back; health below it is not touched.
    #[test]
    fn the_health_cap_only_lowers() {
        assert_eq!(clamped_health(25.0, 20.0), 20.0);
        assert_eq!(clamped_health(20.0, 20.0), 20.0);
        assert_eq!(clamped_health(3.0, 20.0), 3.0);
        assert_eq!(clamped_health(-2.0, 20.0), -2.0);
    }

    /// A NaN health is not above the maximum, so Java leaves it: so does this.
    #[test]
    fn a_nan_health_is_not_capped() {
        assert!(clamped_health(f64::NAN, 20.0).is_nan());
    }

    /// The rule: half health stays half health when the maximum grows.
    #[test]
    fn adding_a_modifier_keeps_the_health_fraction() {
        let mut modifiers = vec![named("armour")];
        let health = add_health_modifier(&mut modifiers, Modifier::new("gear", 10.0), 10.0, 20.0, 30.0);
        assert_eq!(health, 15.0);
        assert_eq!(modifiers.len(), 2);
    }

    /// A modifier of the same name replaces the old one instead of stacking.
    #[test]
    fn adding_a_modifier_of_the_same_name_replaces_it() {
        let mut modifiers = vec![Modifier::new("gear", 4.0)];
        let health =
            add_health_modifier(&mut modifiers, Modifier::new("gear", 10.0), 20.0, 20.0, 30.0);
        assert_eq!(modifiers, vec![Modifier::new("gear", 10.0)]);
        assert_eq!(health, 30.0);
    }

    /// A dead entity stays dead: `0/20` of the way is still `0` at any maximum.
    #[test]
    fn a_dead_entity_is_not_healed_by_a_new_modifier() {
        let mut modifiers = Vec::new();
        let health = add_health_modifier(&mut modifiers, Modifier::new("gear", 5.0), 0.0, 20.0, 25.0);
        assert_eq!(health, 0.0);
    }

    /// A maximum of zero is refused rather than answered with a NaN.
    #[test]
    fn a_zero_maximum_answers_the_health_unchanged() {
        let mut modifiers = Vec::new();
        let health = add_health_modifier(&mut modifiers, Modifier::new("gear", 5.0), 7.0, 0.0, 0.0);
        assert_eq!(health, 7.0);
        // The modifier is still added: only the ratio is refused.
        assert_eq!(modifiers, vec![Modifier::new("gear", 5.0)]);
    }

    /// The generic pair does the same list work and nothing to health — which is
    /// what separates them from the health pair.
    #[test]
    fn the_generic_add_and_remove_do_no_health_arithmetic() {
        let mut modifiers = vec![Modifier::new("speed", 0.1)];
        add_modifier(&mut modifiers, Modifier::new("speed", 0.2));
        assert_eq!(modifiers, vec![Modifier::new("speed", 0.2)]);
        assert_eq!(remove_modifier(&mut modifiers, "speed"), 1);
        assert!(modifiers.is_empty());
    }

    /// The two named removals are the same function: Java's duplication is not a
    /// difference in behaviour.
    #[test]
    fn the_two_named_removals_agree() {
        let mut health_side = vec![named("x"), named("y")];
        let mut generic_side = vec![named("x"), named("y")];
        assert_eq!(
            remove_named(&mut health_side, "x"),
            remove_modifier(&mut generic_side, "x")
        );
        assert_eq!(health_side, generic_side);
    }
}
