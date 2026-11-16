//! The words a player typed, read as a command — or as the rule that says why
//! they are not one.
//!
//! Java reference: `CommandArgumentNames.java` (each argument's spelling),
//! `ColonyIdArgument.java` (the colony argument, which is not a number),
//! `CommandDeleteColony.java:52-63` (the two booleans) and
//! `MultipleOptionsArgument.java:83-118`.
//!
//! **Why words and not a `CommandInvocation`.** An invocation exists only
//! inside a running server, once a usage is registered
//! (`crates/horizon-mod-vocab/src/command/mod.rs:140-170`). Whether the words
//! name a command at all is a rule, and a rule must be testable with no game:
//! so [`parse`] takes the words **after the root literal** — `/colony info 7`
//! arrives as `["info", "7"]`, exactly as `hire::is_hire` reads
//! `invocation.literals[0]`. The report says how the hook will rebuild them
//! (`literals` for the words, `argument(..)` for the values).

use crate::colony::permissions::uuid;

use super::{Refusal, Subcommand, spec_for};

/// A colony, as `ColonyIdArgument` answers one of five ways.
///
/// Java tries the options **in the order it declares them** and keeps the first
/// that matches (`MultipleOptionsArgument.java:83-118`), which is why `7` is an
/// id and not an owner name: `ColonyIdArgument.java:37`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the five options of `ColonyIdArgument.java:37`
pub enum ColonyRef {
    /// `@here`: the colony the position is in (`:128-152`). The position comes
    /// from the command source in Java; the port carries it when one is given,
    /// so the rule can be tested without a source.
    Here { at: Option<[i32; 3]> },
    /// `@mine`: the colony this player owns (`:154-173`).
    Mine,
    /// A bare number, `getColonyByWorld(id, level)` (`:175-202`).
    Id(i32),
    /// A uuid: the colony **owned** by that player (`:204-230`).
    OwnerUuid(u128),
    /// A player name: the same, through the profile cache (`:232-253`).
    OwnerName(String),
}

/// One subcommand with its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: a closed list of commands, mirrored by `Subcommand::ALL`
pub enum ColonyCommand {
    /// `info <colonyID>`
    Info { colony: ColonyRef },
    /// `list [startpage]`
    List { page: i32 },
    /// `here [pos]` — always a [`ColonyRef::Here`], with or without a position.
    Here { colony: ColonyRef },
    /// `tp <colonyID>`
    Teleport { colony: ColonyRef },
    /// `add <colonyID> <playername>`
    Add { colony: ColonyRef, player: String },
    /// `remove <colonyID> <playername>`
    Remove { colony: ColonyRef, player: String },
    /// `rename <colonyID> <name>`
    Rename { colony: ColonyRef, name: String },
    /// `delete <colonyID> [buildings] [confirm]`
    Delete {
        colony: ColonyRef,
        /// `keep / delete buildings` (`CommandDeleteColony.java:44`): absent
        /// until the player answers the guided prompt.
        buildings: Option<bool>,
        /// `confirm` (`:49`): absent until the second prompt is answered.
        confirm: Option<bool>,
    },
}

impl ColonyCommand {
    /// The subcommand this command came from.
    pub fn subcommand(&self) -> Subcommand {
        match self {
            Self::Info { .. } => Subcommand::Info,
            Self::List { .. } => Subcommand::List,
            Self::Here { .. } => Subcommand::Here,
            Self::Teleport { .. } => Subcommand::Teleport,
            Self::Add { .. } => Subcommand::Add,
            Self::Remove { .. } => Subcommand::Remove,
            Self::Rename { .. } => Subcommand::Rename,
            Self::Delete { .. } => Subcommand::Delete,
        }
    }

    /// The colony the command names, or `None` for the one command that names
    /// no colony: `list` reads the whole colony list
    /// (`CommandListColonies.java:83`).
    pub fn colony(&self) -> Option<&ColonyRef> {
        match self {
            Self::Info { colony }
            | Self::Here { colony }
            | Self::Teleport { colony }
            | Self::Add { colony, .. }
            | Self::Remove { colony, .. }
            | Self::Rename { colony, .. }
            | Self::Delete { colony, .. } => Some(colony),
            Self::List { .. } => None,
        }
    }
}

/// Read the words after the root literal. The usage a refusal names is the one
/// the table declares, so an error can never quote a command that does not
/// exist.
pub fn parse(words: &[&str]) -> Result<ColonyCommand, Refusal> {
    let Some(word) = words.first().copied() else {
        return Err(Refusal::MissingSubcommand);
    };
    let spec = spec_for(word).ok_or_else(|| Refusal::UnknownSubcommand {
        word: word.to_owned(),
    })?;
    let rest = &words[1..];
    let usage = spec.usage();

    match spec.kind {
        Subcommand::Info => {
            let colony = reference(rest, &usage)?;
            only(rest, 1, &usage)?;
            Ok(ColonyCommand::Info { colony })
        }
        Subcommand::List => {
            let page = match rest.first().copied() {
                None => 1,
                Some(word) => word
                    .parse::<i32>()
                    .ok()
                    .filter(|page| *page >= 1)
                    .ok_or_else(|| Refusal::BadArgument {
                        name: "startpage",
                        word: word.to_owned(),
                        expected: "an integer of at least 1",
                    })?,
            };
            only(rest, 1, &usage)?;
            Ok(ColonyCommand::List { page })
        }
        Subcommand::Here => {
            let at = match rest.len() {
                0 => None,
                3 => Some(position(rest)?),
                len if len < 3 => {
                    return Err(Refusal::MissingArgument {
                        name: "pos",
                        usage,
                    });
                }
                _ => {
                    return Err(Refusal::UnexpectedArgument {
                        word: rest[3].to_owned(),
                        usage,
                    });
                }
            };
            Ok(ColonyCommand::Here {
                colony: ColonyRef::Here { at },
            })
        }
        Subcommand::Teleport => {
            let colony = reference(rest, &usage)?;
            only(rest, 1, &usage)?;
            Ok(ColonyCommand::Teleport { colony })
        }
        Subcommand::Add => Ok(ColonyCommand::Add {
            colony: reference(rest, &usage)?,
            player: named_player(rest, &usage)?,
        }),
        Subcommand::Remove => Ok(ColonyCommand::Remove {
            colony: reference(rest, &usage)?,
            player: named_player(rest, &usage)?,
        }),
        Subcommand::Rename => {
            let colony = reference(rest, &usage)?;
            // The name is greedy: `rest[1..]`, as `CommandSetRank`'s rank is
            // (`:101`). An empty one is not a name.
            required(rest, 1, "name", &usage)?;
            let name = rest[1..].join(" ");
            if name.trim().is_empty() {
                return Err(Refusal::BadArgument {
                    name: "name",
                    word: name,
                    expected: "a non-empty colony name",
                });
            }
            Ok(ColonyCommand::Rename { colony, name })
        }
        Subcommand::Delete => {
            let colony = reference(rest, &usage)?;
            let buildings = match rest.get(1).copied() {
                Some(word) => Some(boolean(word, "buildings")?),
                None => None,
            };
            let confirm = match rest.get(2).copied() {
                Some(word) => Some(boolean(word, "confirm")?),
                None => None,
            };
            only(rest, 3, &usage)?;
            Ok(ColonyCommand::Delete {
                colony,
                buildings,
                confirm,
            })
        }
    }
}

/// The colony argument, in `ColonyIdArgument`'s option order.
pub fn colony_ref(word: &str) -> ColonyRef {
    if word == "@here" {
        ColonyRef::Here { at: None }
    } else if word == "@mine" {
        ColonyRef::Mine
    } else if let Ok(id) = word.parse::<i32>() {
        ColonyRef::Id(id)
    } else if let Some(owner) = uuid::parse(word) {
        ColonyRef::OwnerUuid(owner)
    } else {
        // The outermost fallback (`ColonyIdArgument.java:232`): any other word
        // is a player name, and a name nobody answers to fails later, as
        // Java's `ERROR_UNKNOWN_PLAYER` does.
        ColonyRef::OwnerName(word.to_owned())
    }
}

/// Three words that are a block position. `~` and `^` are not read here: they
/// are resolved against the source by the dispatcher before a mod sees them
/// (`crates/horizon-mod-vocab/src/command/mod.rs:88-92`), so a raw `~` in these
/// words is a caller's mistake, not a relative coordinate.
fn position(words: &[&str]) -> Result<[i32; 3], Refusal> {
    let mut at = [0i32; 3];
    for (axis, word) in words.iter().take(3).enumerate() {
        at[axis] = word.parse::<i32>().map_err(|_| Refusal::BadPosition {
            word: (*word).to_owned(),
        })?;
    }
    Ok(at)
}

/// A `true`/`false` word. Java's `BoolArgumentType.bool()` is what both delete
/// arguments parse (`CommandDeleteColony.java:200-204`), even though the
/// suggestions are labelled keep/delete (`:206-216`): the word typed is `true`.
fn boolean(word: &str, name: &'static str) -> Result<bool, Refusal> {
    match word {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Refusal::BadArgument {
            name,
            word: word.to_owned(),
            expected: "true or false",
        }),
    }
}

/// The colony argument, then the refusal when it is absent.
fn reference(rest: &[&str], usage: &str) -> Result<ColonyRef, Refusal> {
    Ok(colony_ref(required(rest, 0, "colonyID", usage)?))
}

/// The player name, with the two words the usage takes checked.
fn named_player(rest: &[&str], usage: &str) -> Result<String, Refusal> {
    let name = required(rest, 1, "playername", usage)?;
    only(rest, 2, usage)?;
    Ok(name.to_owned())
}

/// One argument, by position.
fn required<'a>(
    rest: &[&'a str],
    index: usize,
    name: &'static str,
    usage: &str,
) -> Result<&'a str, Refusal> {
    rest.get(index)
        .copied()
        .ok_or_else(|| Refusal::MissingArgument {
            name,
            usage: usage.to_owned(),
        })
}

/// No word past `taken`.
fn only(rest: &[&str], taken: usize, usage: &str) -> Result<(), Refusal> {
    match rest.get(taken) {
        Some(word) => Err(Refusal::UnexpectedArgument {
            word: (*word).to_owned(),
            usage: usage.to_owned(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_subcommand_reads_the_arguments_its_usage_declares() {
        assert_eq!(
            parse(&["info", "7"]),
            Ok(ColonyCommand::Info {
                colony: ColonyRef::Id(7)
            })
        );
        assert_eq!(parse(&["list"]), Ok(ColonyCommand::List { page: 1 }));
        assert_eq!(parse(&["list", "3"]), Ok(ColonyCommand::List { page: 3 }));
        assert_eq!(
            parse(&["here"]),
            Ok(ColonyCommand::Here {
                colony: ColonyRef::Here { at: None }
            })
        );
        assert_eq!(
            parse(&["here", "1", "64", "-3"]),
            Ok(ColonyCommand::Here {
                colony: ColonyRef::Here {
                    at: Some([1, 64, -3])
                }
            })
        );
        assert_eq!(
            parse(&["tp", "@mine"]),
            Ok(ColonyCommand::Teleport {
                colony: ColonyRef::Mine
            })
        );
        assert_eq!(
            parse(&["add", "7", "Ada"]),
            Ok(ColonyCommand::Add {
                colony: ColonyRef::Id(7),
                player: "Ada".to_owned()
            })
        );
        assert_eq!(
            parse(&["rename", "7", "New", "Bridge"]),
            Ok(ColonyCommand::Rename {
                colony: ColonyRef::Id(7),
                name: "New Bridge".to_owned()
            })
        );
        assert_eq!(
            parse(&["delete", "7"]),
            Ok(ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: None,
                confirm: None
            })
        );
        assert_eq!(
            parse(&["delete", "7", "true", "false"]),
            Ok(ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: Some(true),
                confirm: Some(false)
            })
        );
    }

    #[test]
    fn javas_two_other_names_read_as_the_same_command() {
        assert_eq!(
            parse(&["teleport", "7"]),
            Ok(ColonyCommand::Teleport {
                colony: ColonyRef::Id(7)
            })
        );
        assert_eq!(
            parse(&["addOfficer", "7", "Ada"]),
            Ok(ColonyCommand::Add {
                colony: ColonyRef::Id(7),
                player: "Ada".to_owned()
            })
        );
        assert_eq!(
            parse(&["teleport", "7"]).map(|command| command.subcommand()),
            Ok(Subcommand::Teleport)
        );
    }

    #[test]
    fn the_colony_argument_tries_its_five_options_in_javas_order() {
        assert_eq!(colony_ref("@here"), ColonyRef::Here { at: None });
        assert_eq!(colony_ref("@mine"), ColonyRef::Mine);
        assert_eq!(colony_ref("12"), ColonyRef::Id(12));
        assert_eq!(colony_ref("-3"), ColonyRef::Id(-3));
        let id = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;
        let text = uuid::to_text(id);
        assert_eq!(colony_ref(&text), ColonyRef::OwnerUuid(id));
        assert_eq!(
            colony_ref("Ada"),
            ColonyRef::OwnerName("Ada".to_owned()),
            "an unmatched word is a name, not an error: the owner is looked up later"
        );
        // A number wins over a name, and a uuid over a longer name: the first
        // matching option is kept (`MultipleOptionsArgument.java:97-105`).
        assert_eq!(colony_ref("0"), ColonyRef::Id(0));
    }

    #[test]
    fn the_root_alone_and_an_unknown_word_are_named_refusals() {
        assert_eq!(parse(&[]), Err(Refusal::MissingSubcommand));
        assert_eq!(
            parse(&["destroy", "7"]),
            Err(Refusal::UnknownSubcommand {
                word: "destroy".to_owned()
            })
        );
    }

    #[test]
    fn a_missing_argument_names_the_argument_and_quotes_its_usage() {
        assert_eq!(
            parse(&["info"]),
            Err(Refusal::MissingArgument {
                name: "colonyID",
                usage: "colony info <colonyID:word>".to_owned()
            })
        );
        assert_eq!(
            parse(&["add", "7"]),
            Err(Refusal::MissingArgument {
                name: "playername",
                usage: "colony add <colonyID:word> <playername:word>".to_owned()
            })
        );
        assert_eq!(
            parse(&["here", "1", "64"]),
            Err(Refusal::MissingArgument {
                name: "pos",
                usage: "colony here [pos:block_pos]".to_owned()
            })
        );
        assert_eq!(
            parse(&["rename", "7"]),
            Err(Refusal::MissingArgument {
                name: "name",
                usage: "colony rename <colonyID:word> <name:text>".to_owned()
            })
        );
    }

    #[test]
    fn a_surplus_word_is_refused_instead_of_ignored() {
        assert_eq!(
            parse(&["info", "7", "extra"]),
            Err(Refusal::UnexpectedArgument {
                word: "extra".to_owned(),
                usage: "colony info <colonyID:word>".to_owned()
            })
        );
        assert_eq!(
            parse(&["here", "1", "2", "3", "4"]),
            Err(Refusal::UnexpectedArgument {
                word: "4".to_owned(),
                usage: "colony here [pos:block_pos]".to_owned()
            })
        );
        assert_eq!(
            parse(&["list", "1", "2"]),
            Err(Refusal::UnexpectedArgument {
                word: "2".to_owned(),
                usage: "colony list [startpage:integer]".to_owned()
            })
        );
    }

    #[test]
    fn a_page_is_at_least_one_and_the_two_delete_words_are_booleans() {
        for word in ["0", "-1", "two", "1.5", ""] {
            assert_eq!(
                parse(&["list", word]),
                Err(Refusal::BadArgument {
                    name: "startpage",
                    word: word.to_owned(),
                    expected: "an integer of at least 1"
                })
            );
        }
        assert_eq!(
            parse(&["delete", "7", "keep"]),
            Err(Refusal::BadArgument {
                name: "buildings",
                word: "keep".to_owned(),
                expected: "true or false"
            }),
            "the suggestion is labelled keep, the word typed is true"
        );
        assert_eq!(
            parse(&["delete", "7", "true", "yes"]),
            Err(Refusal::BadArgument {
                name: "confirm",
                word: "yes".to_owned(),
                expected: "true or false"
            })
        );
    }

    #[test]
    fn three_words_that_are_not_a_position_are_a_named_refusal() {
        assert_eq!(
            parse(&["here", "1", "2", "z"]),
            Err(Refusal::BadPosition {
                word: "z".to_owned()
            })
        );
        assert_eq!(
            parse(&["here", "~", "64", "0"]),
            Err(Refusal::BadPosition {
                word: "~".to_owned()
            }),
            "a relative coordinate is the dispatcher's job, not this parser's"
        );
    }

    #[test]
    fn a_name_of_only_spaces_is_not_a_name() {
        assert_eq!(
            parse(&["rename", "7", "  "]),
            Err(Refusal::BadArgument {
                name: "name",
                word: "  ".to_owned(),
                expected: "a non-empty colony name"
            })
        );
    }
}
