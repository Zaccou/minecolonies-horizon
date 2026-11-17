//! `visitors/*.json`: which named visitor steps out of the tavern, and the four
//! values Java reads without checking.
//!
//! Java reference: `core/datalistener/CustomVisitorListener.java` (292 lines) and
//! its one caller, `core/colony/buildings/modules/TavernBuildingModule.java:180-188`.
//! 29 files ship. A visitor file describes a citizen the tavern may hand out
//! instead of its generic recruit: a skin, a name, a story key, a chance, and the
//! skills it starts with.
//!
//! **The selection rule, and why its order is not the pack's.** `chanceCustomVisitors`
//! walks the list and stops at the **first** visitor whose `rand.nextDouble() <
//! chance` holds (`:254-262`), then returns `true`. So which visitor appears is
//! decided by position, and the position is not the file's: the list is rebuilt
//! with `ImmutableList.builder().addAll(visitorDataPack).add(entry)` inside a loop
//! over `jsonElementMap.entrySet()` (`:87-90`, `:146`), a `HashMap`. Two visitors
//! at 0.5 each do not split 50/50 — the first one in hash order takes its half
//! first, and the second only sees what is left. [`first_chance`] reproduces the
//! walk with the rolls handed in, so a test can pin the order the caller chose.
//!
//! **Four values read without a check, and what each costs.**
//!
//! | Field | Java | Line |
//! | --- | --- | --- |
//! | `gender` | `substring(0,1)`, then `m`/`f` or the file is dropped — an empty string throws instead | `:137-144` |
//! | `chance` | no range: `2.0` always wins, `-1` never, both silently | `:110-113`, `:256` |
//! | `recruitcostcount` | applied only if `recruitcost` exists, otherwise ignored in silence | `:118-124` |
//! | `primaryskill` / `secondaryskill` | `Skill.valueOf` **at spawn time, uncaught** — a typo breaks the tavern, not the file | `:200-205` |
//!
//! The last one is the worst: it sits in `applyToVisitor`, which runs from
//! `chanceCustomVisitors`, which `TavernBuildingModule` calls with no `try`
//! around it. An invalid skill name is therefore not a refused file, it is an
//! `IllegalArgumentException` in the middle of a visitor spawn, every time the
//! roll reaches that visitor. The port refuses it while reading, where the file
//! name is still known.
//!
//! **What is not here**: `RecruitmentInteraction` (a `ChatPriority.IMPORTANT` chat
//! message, `:196-198`), `markDirty` and the entity rename (`:188-192`, `:219`),
//! and the voice profile, which `applyToVisitor` reads and throws away
//! (`:207-210`: `visitorData.getVoiceProfile()` — no setter is called).

use super::Refused;

/// `CitizenSkillHandler`'s eleven skills, in declaration order
/// (`api/entity/citizen/Skill.java:10-150`), because `Skill.valueOf` is
/// case-sensitive and this list is what it accepts.
pub const SKILLS: &[&str] = &[
    "Athletics",
    "Dexterity",
    "Strength",
    "Agility",
    "Stamina",
    "Mana",
    "Adaptability",
    "Focus",
    "Creativity",
    "Knowledge",
    "Intelligence",
];

/// `IVisitorData.setGender(gender.equals("f"))` (`:135-138`), as a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: `applyToVisitor` asks one boolean, `f` or not
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    /// Java takes the **first character** of the field (`:139`), so `male` and
    /// `m` are both `Male`.
    pub fn parse(field: &str) -> Option<Gender> {
        match field.chars().next()? {
            'm' => Some(Gender::Male),
            'f' => Some(Gender::Female),
            _ => None,
        }
    }
}

/// One file of `visitors/`, as the JSON reader hands it over.
///
/// Every field is optional and defaults to Java's initialiser (`:72-105`): no
/// texture, no name, `chance` 0, no cost, voice profile −1.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub texture: Option<&'static str>,
    pub name: Option<&'static str>,
    pub story: Option<&'static str>,
    pub chance: Option<f64>,
    pub suffix: Option<&'static str>,
    pub recruit_cost: Option<&'static str>,
    pub recruit_cost_count: Option<i32>,
    pub gender: Option<&'static str>,
    pub voice_profile: Option<i32>,
    pub primary_skill: Option<&'static str>,
    pub secondary_skill: Option<&'static str>,
}

/// A checked visitor: what [`super::load`] stores.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the fields `applyToVisitor` actually uses
pub struct Visitor {
    pub texture: Option<&'static str>,
    pub name: Option<&'static str>,
    pub story: Option<&'static str>,
    /// `0.0` when the field is absent (`:82`); a visitor with no chance never
    /// appears, which is why the shipped files always set it.
    pub chance: f64,
    pub suffix: Option<&'static str>,
    /// The cost and its count together: `:118-124` ignores a count without a cost.
    pub recruit_cost: Option<(&'static str, i32)>,
    pub gender: Option<Gender>,
    pub voice_profile: i32,
    pub primary_skill: Option<&'static str>,
    pub secondary_skill: Option<&'static str>,
}

/// Why a visitor file is refused, named.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: this port's own refusal set, closed on purpose
// (no `Eq`: one variant carries the `f64` it refused)
pub enum Refusal {
    /// `gender` is present but starts with neither `m` nor `f` (`:140-144`), or is
    /// empty — which throws at `substring(0,1)` and loses the file anyway.
    BadGender { id: &'static str, got: &'static str },
    /// `chance` outside `0..=1`. **Stricter than Java**, which accepts `2.0`
    /// (always) and `-1` (never) and says nothing.
    ChanceOutOfRange { id: &'static str, chance: f64 },
    /// A count with no cost: Java drops it silently (`:119-122`).
    CountWithoutCost { id: &'static str },
    /// A skill name `Skill.valueOf` would reject — at spawn time, in Java
    /// (`:200-205`).
    UnknownSkill { id: &'static str, skill: &'static str },
    /// A `recruitcost` item id no registry knows. Java builds the stack anyway and
    /// the file is lost inside the `catch` (`:145-148`).
    UnknownItem { id: &'static str, item: &'static str },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::BadGender { id, got } => {
                format!("data_listeners.refusal.bad_visitor_gender({id}.{got})")
            }
            Refusal::ChanceOutOfRange { id, chance } => {
                format!("data_listeners.refusal.visitor_chance_out_of_range({id}.{chance})")
            }
            Refusal::CountWithoutCost { id } => {
                format!("data_listeners.refusal.visitor_count_without_cost({id})")
            }
            Refusal::UnknownSkill { id, skill } => {
                format!("data_listeners.refusal.unknown_skill({id}.{skill})")
            }
            Refusal::UnknownItem { id, item } => {
                format!("data_listeners.refusal.unknown_visitor_item({id}.{item})")
            }
        }
    }
}

/// Reads one file. `known_item` is the registry's answer, supplied by the caller:
/// this module holds no registry.
pub fn parse(id: &'static str, row: &Row, known_item: &dyn Fn(&str) -> bool) -> Result<Visitor, Refusal> {
    let gender = match row.gender {
        None => None,
        Some(field) => match Gender::parse(field) {
            Some(gender) => Some(gender),
            None => return Err(Refusal::BadGender { id, got: field }),
        },
    };
    let chance = row.chance.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&chance) {
        return Err(Refusal::ChanceOutOfRange { id, chance });
    }
    let recruit_cost = match (row.recruit_cost, row.recruit_cost_count) {
        (Some(item), count) => {
            if !known_item(item) {
                return Err(Refusal::UnknownItem { id, item });
            }
            Some((item, count.unwrap_or(1)))
        }
        // Java's `if (dataEntry.recruitCost != null)` guard (`:119`).
        (None, Some(_)) => return Err(Refusal::CountWithoutCost { id }),
        (None, None) => None,
    };
    for skill in [row.primary_skill, row.secondary_skill].into_iter().flatten() {
        if !SKILLS.contains(&skill) {
            return Err(Refusal::UnknownSkill { id, skill });
        }
    }
    Ok(Visitor {
        texture: row.texture,
        name: row.name,
        story: row.story,
        chance,
        suffix: row.suffix,
        recruit_cost,
        gender,
        voice_profile: row.voice_profile.unwrap_or(-1),
        primary_skill: row.primary_skill,
        secondary_skill: row.secondary_skill,
    })
}

/// `chanceCustomVisitors` (`:254-262`): the index of the **first** visitor whose
/// roll is under its chance, walked in list order.
///
/// `rolls[i]` is the draw Java makes for the i-th visitor; Java draws one per
/// visitor until one succeeds, so the caller hands exactly that many. A missing
/// roll (a shorter slice) means the walk stopped, and `None` is Java's `false`:
/// the tavern falls back to its generic recruit (`TavernBuildingModule.java:182`).
pub fn first_chance(pack: &[Visitor], rolls: &[f64]) -> Option<usize> {
    pack.iter()
        .zip(rolls)
        .position(|(visitor, roll)| *roll < visitor.chance)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Row {
        Row {
            texture: None,
            name: None,
            story: None,
            chance: Some(0.5),
            suffix: None,
            recruit_cost: None,
            recruit_cost_count: None,
            gender: None,
            voice_profile: None,
            primary_skill: None,
            secondary_skill: None,
        }
    }

    fn known(_: &str) -> bool {
        true
    }

    #[test]
    fn the_gender_takes_the_first_letter_and_accepts_both_spellings() {
        assert_eq!(Gender::parse("male"), Some(Gender::Male));
        assert_eq!(Gender::parse("m"), Some(Gender::Male));
        assert_eq!(Gender::parse("female"), Some(Gender::Female));
        assert_eq!(Gender::parse("f"), Some(Gender::Female));
        assert_eq!(Gender::parse("x"), None);
        assert_eq!(Gender::parse(""), None);
    }

    #[test]
    fn an_empty_or_unknown_gender_is_refused_by_name() {
        let refusal = parse("visitors/x.json", &Row { gender: Some(""), ..row() }, &known)
            .expect_err("substring(0,1) throws in Java");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.bad_visitor_gender(visitors/x.json.)"
        );
    }

    #[test]
    fn a_chance_outside_zero_and_one_is_refused() {
        let refusal = parse("visitors/x.json", &Row { chance: Some(2.0), ..row() }, &known)
            .expect_err("2.0 would always win");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.visitor_chance_out_of_range(visitors/x.json.2)"
        );
        assert!(parse("visitors/x.json", &Row { chance: Some(0.0), ..row() }, &known).is_ok());
        assert!(parse("visitors/x.json", &Row { chance: Some(1.0), ..row() }, &known).is_ok());
    }

    #[test]
    fn a_count_without_a_cost_is_refused_instead_of_ignored() {
        let refusal = parse(
            "visitors/x.json",
            &Row { recruit_cost_count: Some(4), ..row() },
            &known,
        )
        .expect_err("Java ignores the count");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.visitor_count_without_cost(visitors/x.json)"
        );
        let with_cost = parse(
            "visitors/x.json",
            &Row { recruit_cost: Some("minecraft:bread"), recruit_cost_count: Some(4), ..row() },
            &known,
        )
        .expect("a cost with a count is the normal case");
        assert_eq!(with_cost.recruit_cost, Some(("minecraft:bread", 4)));
    }

    #[test]
    fn an_unknown_skill_is_refused_while_reading_not_while_spawning() {
        let refusal = parse(
            "visitors/x.json",
            &Row { primary_skill: Some("Athletic"), ..row() },
            &known,
        )
        .expect_err("Skill.valueOf throws at spawn in Java");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.unknown_skill(visitors/x.json.Athletic)"
        );
        assert!(SKILLS.contains(&"Athletics"));
    }

    #[test]
    fn the_first_visitor_to_pass_its_roll_wins_and_the_later_ones_are_never_drawn() {
        let pack = vec![
            Visitor { chance: 0.5, ..parse("a", &row(), &known).expect("valid") },
            Visitor { chance: 0.9, ..parse("b", &row(), &known).expect("valid") },
        ];
        // First visitor fails (0.7 > 0.5), second passes.
        assert_eq!(first_chance(&pack, &[0.7, 0.1]), Some(1));
        // First passes: Java never draws for the second.
        assert_eq!(first_chance(&pack, &[0.1]), Some(0));
        // Nobody passes: the tavern falls back to its generic recruit.
        assert_eq!(first_chance(&pack, &[0.9, 0.95]), None);
    }

    #[test]
    fn order_decides_which_of_two_equal_visitors_appears() {
        let named = |name: &'static str| Visitor {
            name: Some(name),
            ..parse(name, &row(), &known).expect("valid")
        };
        let rolls = [0.4, 0.4];
        let winner = |pack: &[Visitor]| first_chance(pack, &rolls).map(|at| pack[at].name);
        // Same two visitors and the same two rolls; swapping the list swaps the
        // winner, which is what a `HashMap`-ordered pack decides by accident
        // (`CustomVisitorListener.java:87-90`).
        assert_eq!(winner(&[named("a"), named("b")]), Some(Some("a")));
        assert_eq!(winner(&[named("b"), named("a")]), Some(Some("b")));
    }
}
