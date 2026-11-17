//! `citizennames/*.json`: the style a colony draws names from, and the three ways
//! a name file can be unusable while looking fine.
//!
//! Java reference: `core/datalistener/CitizenNameListener.java` (89 lines) and
//! `core/colony/CitizenData.java:626-690`, which is where the file is actually
//! consumed. 22 files ship, in `data/minecolonies/citizennames/`, and
//! `default.json` is the one every colony falls back to
//! (`Colony.java:1949`: `getOrDefault(nameStyle, get("default"))`).
//!
//! **The listener reads five fields and checks none of them.**
//! `parts` is stored as-is (`CitizenNameListener.java:63`), the three arrays are
//! copied, and `order` is the only value that can fail — `NameOrder.valueOf`
//! throws on anything but `EASTERN`/`WESTERN` (`:64`), and the throw is caught by
//! the per-file `try` (`:84-87`), so a misspelled order costs exactly that file.
//!
//! **What the missing checks cost, measured downstream.**
//!
//! | Input | Java | Line |
//! | --- | --- | --- |
//! | `parts` is neither 2 nor 3 | silently a **one-word name**: the `else` branch keeps the first name only | `CitizenData.java:655-658` |
//! | an empty first-name or surname array | `rand.nextInt(0)` throws `IllegalArgumentException` on the first citizen | `:458-461` |
//! | more citizens than the file can name | **infinite recursion**, no depth bound | `:660-670` |
//!
//! The port refuses all three by name ([`Refusal`]) because the first is a typo
//! that changes 22 files' worth of output, the second is a crash at spawn, and the
//! third is a crash that only shows up on a small custom pack. [`room_for`] is what
//! the third refusal needs: how many distinct names a file can produce, so a caller
//! can refuse before recursing.
//!
//! **What is not here**: `nameStyle` itself (a colony field, `Colony.java:339`), the
//! client-side copy (`ColonyView.java:316-317` writes the key set, `:1365` reads the
//! map), and the `middleInitial` roll (`rand.nextInt(26) + 'A'`,
//! `CitizenData.java:469-472`) — the caller hands the letter in, because a wasm mod
//! that rolled its own could not be replayed.

use super::Refused;

/// `CitizenNameFile.NameOrder` (`CitizenNameFile.java:15-19`), both values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the enum has two constants, neither is added by a data pack
pub enum Order {
    /// `EASTERN`: surname first, no middle initial
    /// (`CitizenData.java:645-649`).
    Eastern,
    /// `WESTERN`: first name first, `parts` decides on the initial
    /// (`:651-658`).
    Western,
}

impl Order {
    /// The two spellings `valueOf` accepts, exactly (`CitizenNameFile.java:17-18`).
    pub const SPELLINGS: &'static [(&'static str, Order)] = &[
        ("EASTERN", Order::Eastern),
        ("WESTERN", Order::Western),
    ];

    /// The name Java's `valueOf` would find, or `None` — `valueOf` is
    /// case-sensitive and the two constants are the whole set.
    pub fn parse(spelling: &str) -> Option<Order> {
        Order::SPELLINGS
            .iter()
            .find(|(name, _)| *name == spelling)
            .map(|(_, order)| *order)
    }

    /// The spelling, as the JSON holds it.
    pub fn name(self) -> &'static str {
        match self {
            Order::Eastern => "EASTERN",
            Order::Western => "WESTERN",
        }
    }
}

/// A usable name file: what the listener stores and what `generateName` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five fields of `CitizenNameFile`, no more
pub struct NameFile {
    /// `parts`: 2 = "First Last", 3 = "First M. Last", ignored by `EASTERN`
    /// (`:645-658`).
    pub parts: u8,
    /// Which order the parts come in.
    pub order: Order,
    /// `male_firstname`.
    pub male: Vec<&'static str>,
    /// `female_firstname`.
    pub female: Vec<&'static str>,
    /// `surnames`.
    pub surnames: Vec<&'static str>,
}

/// One file of `citizennames/`, as the JSON reader hands it over.
///
/// `parts` and `order` are `Option` because Java reads them with `get(...).getAs…`
/// and no default: absent means an NPE inside the `try`, so an absent field is a
/// refusal by name and not a default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub parts: Option<i32>,
    pub order: Option<&'static str>,
    pub male: &'static [&'static str],
    pub female: &'static [&'static str],
    pub surnames: &'static [&'static str],
}

/// Why a name file is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A field Java reads without a default is missing
    /// (`CitizenNameListener.java:63-64`).
    MissingField { id: &'static str, field: &'static str },
    /// `order` is neither `EASTERN` nor `WESTERN`
    /// (`NameOrder.valueOf` throws, `:64`).
    UnknownOrder { id: &'static str, order: &'static str },
    /// `parts` outside 2..=3: Java silently produces a **one-word** name
    /// (`CitizenData.java:655-658`).
    UnsupportedParts { id: &'static str, parts: i32 },
    /// An empty array: `rand.nextInt(0)` throws at the first citizen
    /// (`CitizenData.java:458-461`).
    EmptyList { id: &'static str, field: &'static str },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::MissingField { id, field } => {
                format!("data_listeners.refusal.missing_field({id}.{field})")
            }
            Refusal::UnknownOrder { id, order } => {
                format!("data_listeners.refusal.unknown_name_order({id}.{order})")
            }
            Refusal::UnsupportedParts { id, parts } => {
                format!("data_listeners.refusal.unsupported_name_parts({id}.{parts})")
            }
            Refusal::EmptyList { id, field } => {
                format!("data_listeners.refusal.empty_name_list({id}.{field})")
            }
        }
    }
}

/// Reads one file, refusing what Java would accept and crash on later.
///
/// `EASTERN` ignores `parts` (`CitizenData.java:645-649` branches on the order
/// first), so an `EASTERN` file with `parts: 2` is legitimate and accepted.
pub fn parse(id: &'static str, row: &Row) -> Result<NameFile, Refusal> {
    let order = match row.order {
        None => return Err(Refusal::MissingField { id, field: "order" }),
        Some(spelling) => {
            Order::parse(spelling).ok_or(Refusal::UnknownOrder { id, order: spelling })?
        }
    };
    let parts = row.parts.ok_or(Refusal::MissingField { id, field: "parts" })?;
    if order == Order::Western && !(2..=3).contains(&parts) {
        return Err(Refusal::UnsupportedParts { id, parts });
    }
    for (field, list) in [
        ("male_firstname", row.male),
        ("female_firstname", row.female),
        ("surnames", row.surnames),
    ] {
        if list.is_empty() {
            return Err(Refusal::EmptyList { id, field });
        }
    }
    Ok(NameFile {
        parts: parts.clamp(2, 3) as u8,
        order,
        male: row.male.to_vec(),
        female: row.female.to_vec(),
        surnames: row.surnames.to_vec(),
    })
}

/// Assembles one name, line for line with `CitizenData.generateName`
/// (`:632-659`).
///
/// The middle initial is the caller's: Java draws it (`:642`) and a wasm mod that
/// drew its own could not be replayed.
pub fn compose(file: &NameFile, first: &str, initial: char, last: &str) -> String {
    match file.order {
        // `:645-649`: "don't include middle names, as their rules vary heavily".
        Order::Eastern => format!("{last} {first}"),
        Order::Western if file.parts == 3 => format!("{first} {initial}. {last}"),
        Order::Western => format!("{first} {last}"),
    }
}

/// How many distinct names a file can produce — the bound Java does not have.
///
/// `generateName` retries by recursion when a name is taken and stops only when the
/// stack does (`:660-670`). This is `male·surnames + female·surnames`, times 26 when
/// the middle initial is part of the name (`parts == 3` and `WESTERN`), which is
/// exactly the set of strings [`compose`] can return.
pub fn room_for(file: &NameFile) -> usize {
    let stems = file.male.len() + file.female.len();
    let names = stems * file.surnames.len();
    if file.order == Order::Western && file.parts == 3 {
        names * 26
    } else {
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Row {
        Row {
            parts: Some(3),
            order: Some("WESTERN"),
            male: &["John", "Adam"],
            female: &["Jane"],
            surnames: &["Smith", "Brown"],
        }
    }

    #[test]
    fn eastern_puts_the_surname_first_and_drops_the_initial() {
        let file = parse("names/japanese.json", &Row { order: Some("EASTERN"), ..row() })
            .expect("EASTERN is a valid order");
        assert_eq!(compose(&file, "Akira", 'Q', "Tanaka"), "Tanaka Akira");
        // `parts` is ignored by the eastern branch, so 4 is not a refusal here.
        assert_eq!(file.parts, 3);
    }

    #[test]
    fn western_with_three_parts_keeps_the_middle_initial_with_its_dot() {
        let file = parse("names/english.json", &row()).expect("valid");
        assert_eq!(compose(&file, "John", 'Q', "Smith"), "John Q. Smith");
    }

    #[test]
    fn western_with_two_parts_has_no_initial() {
        let file = parse("names/french.json", &Row { parts: Some(2), ..row() }).expect("valid");
        assert_eq!(compose(&file, "Jean", 'Q', "Dupont"), "Jean Dupont");
    }

    #[test]
    fn an_unknown_order_is_refused_by_name() {
        // Java: `NameOrder.valueOf("western")` throws, caught at `:84-87`, file lost.
        let refusal = parse("names/x.json", &Row { order: Some("western"), ..row() })
            .expect_err("lowercase is not a constant");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.unknown_name_order(names/x.json.western)"
        );
    }

    #[test]
    fn parts_outside_two_and_three_is_refused_instead_of_becoming_a_mononym() {
        let refusal = parse("names/x.json", &Row { parts: Some(5), ..row() })
            .expect_err("Java would name this citizen 'John' alone");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.unsupported_name_parts(names/x.json.5)"
        );
    }

    #[test]
    fn an_empty_name_list_is_refused_instead_of_throwing_at_spawn() {
        let refusal = parse(
            "names/x.json",
            &Row { surnames: &[], ..row() },
        )
        .expect_err("nextInt(0) throws");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.empty_name_list(names/x.json.surnames)"
        );
    }

    #[test]
    fn a_missing_field_is_a_refusal_and_not_a_default() {
        assert_eq!(
            parse("names/x.json", &Row { order: None, ..row() })
                .expect_err("no default in Java")
                .name(),
            "data_listeners.refusal.missing_field(names/x.json.order)"
        );
        assert_eq!(
            parse("names/x.json", &Row { parts: None, ..row() })
                .expect_err("no default in Java")
                .name(),
            "data_listeners.refusal.missing_field(names/x.json.parts)"
        );
    }

    #[test]
    fn the_name_room_counts_the_strings_compose_can_return() {
        let three = parse("names/english.json", &row()).expect("valid");
        // (2 male + 1 female) × 2 surnames × 26 initials.
        assert_eq!(room_for(&three), 156);
        let two = parse("names/french.json", &Row { parts: Some(2), ..row() }).expect("valid");
        assert_eq!(room_for(&two), 6);
        let eastern = parse("names/japanese.json", &Row { order: Some("EASTERN"), ..row() })
            .expect("valid");
        assert_eq!(room_for(&eastern), 6);
    }
}
