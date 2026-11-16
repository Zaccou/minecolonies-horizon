//! What the player reads: the lines of each answer, with the real values.
//!
//! Java reference: `CommandColonyInfo.java:48-56` and
//! `CommandListColonies.java:95-125`, which build their lines from **literal**
//! format strings, and `CommandTranslationConstants.java:13-105` for the keys of
//! the messages that are translated.
//!
//! **Two kinds of line, as in Java.** A line Java composes from literals
//! (`ID: 7 Name: Bridgewater`, the page header, the coordinate line) is a
//! [`Text::literal`] here, with the same text in the same order. A line Java
//! takes from the language file is a translatable component carrying the same
//! key, and — because this crate owns no lang file and `src/commands/` may not
//! write one — an English `fallback`, so the line reads as a sentence even
//! before the key is added. The report lists every key.
//!
//! **What a mod cannot do.** Java styles the coordinate line green, makes the
//! `list` lines clickable and turns the two delete answers into buttons
//! (`CommandDeleteColony.java:96-120`). `TextStyle` is `#[non_exhaustive]` and
//! has no setter (`crates/horizon-mod-vocab/src/text/mod.rs:107-135`), so a mod
//! can neither colour nor attach a click: those lines are plain here, and the
//! report says so instead of pretending.

use mod_api::text::TextContent;
use mod_api::Text;

use crate::colony::permissions::{uuid, Refusal as PermissionRefusal};
use crate::colony::Colony;

use super::rules::{Fleet, owner};
use super::syntax::ColonyCommand;
use super::Refusal;

/// One message: Java's key, and the English a caller reads while the key is not
/// in the language file. The fallback is a placeholder, not a translation.
#[derive(Clone, Copy, Debug)]
// exhaustive: static message data, read only
pub struct Msg {
    pub key: &'static str,
    pub english: &'static str,
}

impl Msg {
    /// The component, with `arguments` in the order the template uses them.
    pub fn text(&self, arguments: Vec<Text>) -> Text {
        Text::new(TextContent::Translatable {
            key: self.key.to_owned(),
            fallback: Some(self.english.to_owned()),
            arguments,
        })
    }
}

/// Java's message keys, kept verbatim (`CommandTranslationConstants.java`).
pub mod key {
    use super::Msg;

    pub const TELEPORT_SUCCESS: Msg = Msg {
        key: "com.minecolonies.command.teleport.success",
        english: "Teleported to %s",
    };
    pub const OFFICER_ADD_SUCCESS: Msg = Msg {
        key: "com.minecolonies.command.addofficer.success",
        english: "%s is now an officer of %s",
    };
    pub const DELETE_SUCCESS: Msg = Msg {
        key: "com.minecolonies.command.delete.success",
        english: "%s was deleted",
    };
    pub const DELETE_CONFIRM_BUILDING: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.building",
        english: "Delete the buildings too? %s keep them (false), %s delete them (true)",
    };
    pub const DELETE_CONFIRM_BUILDING_KEEP: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.building.keep",
        english: "keep",
    };
    pub const DELETE_CONFIRM_BUILDING_DELETE: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.building.delete",
        english: "delete",
    };
    pub const DELETE_CONFIRM_FINAL: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.final",
        english: "Confirm the deletion: %s %s (add true to run it)",
    };
    pub const DELETE_CONFIRM_FINAL_BUILDING: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.final.building",
        english: "and every building",
    };
    pub const DELETE_CONFIRM_FINAL_HERE: Msg = Msg {
        key: "com.minecolonies.command.delete.confirm.final.here",
        english: "confirm",
    };
    pub const DISABLED_IN_CONFIG: Msg = Msg {
        key: "com.minecolonies.command.notenabledinconfig",
        english: "That command is disabled in the server's config (%s)",
    };
    pub const COLONY_ID_NOT_FOUND: Msg = Msg {
        key: "com.minecolonies.command.colonyidnotfound",
        english: "There is no colony %s",
    };
    pub const PLAYER_NOT_FOUND: Msg = Msg {
        key: "com.minecolonies.command.playernotfound",
        english: "There is no player named %s",
    };
}

/// The port's own keys: the tree has three subcommands Java has no command for
/// (`remove`, `rename`) and refuses in ways Java leaves to Brigadier or to a
/// silent non-registration, so those lines have no Java key to reuse.
pub mod port_key {
    use super::Msg;

    pub const MEMBERS: Msg = Msg {
        key: "colony.command.info.members",
        english: "Members:",
    };
    pub const MEMBER: Msg = Msg {
        key: "colony.command.info.member",
        english: "%s: %s",
    };
    pub const RENAME_SUCCESS: Msg = Msg {
        key: "colony.command.rename.success",
        english: "The colony is now called %s",
    };
    pub const REMOVE_SUCCESS: Msg = Msg {
        key: "colony.command.remove.success",
        english: "%s was removed from %s",
    };
    pub const UNKNOWN_SUBCOMMAND: Msg = Msg {
        key: "colony.command.unknown_subcommand",
        english: "Unknown colony subcommand: %s",
    };
    pub const USAGE: Msg = Msg {
        key: "colony.command.usage",
        english: "Usage: /colony info, list, here, tp, add, remove, rename, delete",
    };
    pub const MISSING_ARGUMENT: Msg = Msg {
        key: "colony.command.missing_argument",
        english: "Missing argument %s. Usage: %s",
    };
    pub const UNEXPECTED_ARGUMENT: Msg = Msg {
        key: "colony.command.unexpected_argument",
        english: "Unexpected word %s. Usage: %s",
    };
    pub const BAD_ARGUMENT: Msg = Msg {
        key: "colony.command.bad_argument",
        english: "%s cannot be %s (expected %s)",
    };
    pub const BAD_POSITION: Msg = Msg {
        key: "colony.command.bad_position",
        english: "%s is not a block position",
    };
    pub const NOT_A_PLAYER: Msg = Msg {
        key: "colony.command.not_a_player",
        english: "Only a player can run this command",
    };
    pub const NOT_MANAGER: Msg = Msg {
        key: "colony.command.not_manager",
        english: "Your rank (%s) is not a colony manager",
    };
    pub const DENIED_LACKS: Msg = Msg {
        key: "colony.command.denied.action",
        english: "Your rank (%s) is not allowed to %s here",
    };
    pub const DENIED_OWNER: Msg = Msg {
        key: "colony.command.denied.owner",
        english: "The colony's owner can never be demoted or removed",
    };
    pub const DENIED_NOT_MEMBER: Msg = Msg {
        key: "colony.command.denied.not_member",
        english: "That player is not a member of this colony",
    };
}

/// The lines one command answers. `colony` is the record the words resolved to;
/// it is `None` only for `list`, which reads the whole fleet.
pub fn render(fleet: &Fleet, colony: Option<&Colony>, command: &ColonyCommand) -> Vec<Text> {
    if let ColonyCommand::List { page } = command {
        return list(fleet, *page);
    }
    let Some(colony) = colony else {
        // Unreachable through `run`, which resolves every other subcommand's
        // colony before it renders; an empty answer is better than a lie.
        return Vec::new();
    };
    match command {
        ColonyCommand::Info { .. } => info(fleet, colony),
        ColonyCommand::Here { .. } | ColonyCommand::Teleport { .. } => {
            vec![key::TELEPORT_SUCCESS.text(vec![Text::literal(colony.name.as_str())])]
        }
        ColonyCommand::Add { player, .. } => vec![key::OFFICER_ADD_SUCCESS.text(vec![
            Text::literal(player.as_str()),
            Text::literal(colony.name.as_str()),
        ])],
        ColonyCommand::Remove { player, .. } => vec![port_key::REMOVE_SUCCESS.text(vec![
            Text::literal(player.as_str()),
            Text::literal(colony.name.as_str()),
        ])],
        ColonyCommand::Rename { name, .. } => vec![port_key::RENAME_SUCCESS.text(vec![
            Text::literal(name.as_str()),
        ])],
        ColonyCommand::Delete {
            buildings, confirm, ..
        } => delete(colony, *buildings, *confirm),
        // Handled above, so this arm only exists to keep the match exhaustive.
        ColonyCommand::List { .. } => Vec::new(),
    }
}

/// The four lines of `info`, then one line per member.
///
/// Java prints `Citizens: 3/8`; the maximum is
/// `CitizenManager.getMaxCitizens` = `min(housing, research, config)`
/// (`CitizenManager.java:503-506`), and neither research nor the config is in
/// the colony record, so the port shows the count alone instead of inventing a
/// denominator.
fn info(fleet: &Fleet, colony: &Colony) -> Vec<Text> {
    let center = colony.center;
    let mut lines = vec![
        Text::literal(format!("ID: {} Name: {}", colony.id, colony.name)),
        Text::literal(format!("Mayor: {}", name_of(fleet, owner(colony)))),
        Text::literal(format!("Citizens: {}", colony.citizens.len())),
        Text::literal(format!(
            "Coordinates: x={} y={} z={}",
            center[0], center[1], center[2]
        )),
        port_key::MEMBERS.text(Vec::new()),
    ];
    for (player, rank) in colony.members.iter() {
        lines.push(port_key::MEMBER.text(vec![
            Text::literal(rank.name()),
            Text::literal(name_of(fleet, Some(player))),
        ]));
    }
    lines
}

/// The page of the colony list, with Java's own arithmetic: nine a page, a page
/// out of range falls back to the first (`CommandListColonies.java:63-91`).
fn list(fleet: &Fleet, page: i32) -> Vec<Text> {
    const PER_PAGE: i32 = 9;
    let colonies: Vec<&Colony> = fleet.iter().collect();
    let count = colonies.len() as i32;
    let half = if count % PER_PAGE == 0 { 0 } else { 1 };
    let pages = count / PER_PAGE + half;
    let page = if page < 1 || page > pages { 1 } else { page };

    let start = (PER_PAGE * (page - 1)).max(0) as usize;
    let stop = ((PER_PAGE * page) as usize).min(colonies.len());
    let shown: &[&Colony] = if start >= colonies.len() {
        &[]
    } else {
        &colonies[start..stop]
    };

    let mut lines = vec![Text::literal(format!(
        "   ------------------ page {page} of {pages} ------------------"
    ))];
    for colony in shown {
        let center = colony.center;
        lines.push(Text::literal(format!(
            "ID: {}  Name: {} Mayor: {}",
            colony.id,
            colony.name,
            name_of(fleet, owner(colony))
        )));
        lines.push(Text::literal(format!(
            "Citizens:{} Coordinates: x={} y={} z={}",
            colony.citizens.len(),
            center[0],
            center[1],
            center[2]
        )));
    }
    // The two page buttons are click events in Java (`:113-120`); a mod cannot
    // set one, so the line keeps Java's text and the page is asked for by
    // number. The link targets Java puts there are `max(1, page-1)` and
    // `min(page+1, count/9 + half)` (`:81-82`), and another page name is a
    // `ColonyCommand::List` away.
    lines.push(Text::literal(
        " ---------------- <- prev | next ->  ----------------",
    ));
    lines
}

/// The guided deletion, one step per missing answer
/// (`CommandDeleteColony.java:92-146`).
fn delete(colony: &Colony, buildings: Option<bool>, confirm: Option<bool>) -> Vec<Text> {
    match (buildings, confirm) {
        (None, _) => vec![key::DELETE_CONFIRM_BUILDING.text(vec![
            brace(&key::DELETE_CONFIRM_BUILDING_KEEP),
            brace(&key::DELETE_CONFIRM_BUILDING_DELETE),
        ])],
        (Some(buildings), None) => {
            let detail = if buildings {
                key::DELETE_CONFIRM_FINAL_BUILDING.text(Vec::new())
            } else {
                Text::literal("")
            };
            vec![key::DELETE_CONFIRM_FINAL.text(vec![
                detail,
                brace(&key::DELETE_CONFIRM_FINAL_HERE),
            ])]
        }
        // Both answers given. Java deletes on `true`, and on `false` it returns
        // 1 **without a message** (`CommandDeleteColony.java:167-170`): the
        // cancellation is silent, and the port keeps it silent.
        (Some(_), Some(false)) => Vec::new(),
        (Some(buildings), Some(true)) => vec![key::DELETE_SUCCESS.text(vec![
            Text::literal(colony.name.as_str()),
        ])],
    }
}

/// Java's `braceButtonComponent`: the button between square brackets
/// (`CommandDeleteColony.java:81-88`).
fn brace(label: &Msg) -> Text {
    Text::literal("[")
        .append(label.text(Vec::new()))
        .append(Text::literal("]"))
}

/// The lines of a refusal: the rule that fired, and what to type instead when
/// the usage is what was wrong.
pub fn refusal(refusal: &Refusal) -> Vec<Text> {
    // `Text::literal` is spelled out at each use: it takes `impl Into<String>`,
    // so one binding could not serve both the `&str` and the `String` values
    // below.
    match refusal {
        Refusal::MissingSubcommand => vec![port_key::USAGE.text(Vec::new())],
        Refusal::UnknownSubcommand { word } => vec![
            port_key::UNKNOWN_SUBCOMMAND.text(vec![Text::literal(word.as_str())]),
            port_key::USAGE.text(Vec::new()),
        ],
        Refusal::MissingArgument { name, usage } => {
            vec![port_key::MISSING_ARGUMENT.text(vec![
                Text::literal(*name),
                Text::literal(usage.as_str()),
            ])]
        }
        Refusal::UnexpectedArgument { word, usage } => {
            vec![port_key::UNEXPECTED_ARGUMENT.text(vec![
                Text::literal(word.as_str()),
                Text::literal(usage.as_str()),
            ])]
        }
        Refusal::BadArgument {
            name,
            word,
            expected,
        } => vec![port_key::BAD_ARGUMENT.text(vec![
            Text::literal(*name),
            Text::literal(word.as_str()),
            Text::literal(*expected),
        ])],
        Refusal::BadPosition { word } => vec![port_key::BAD_POSITION.text(vec![Text::literal(
            word.as_str(),
        )])],
        Refusal::UnknownColony { reference } => vec![key::COLONY_ID_NOT_FOUND.text(vec![
            Text::literal(reference.as_str()),
        ])],
        Refusal::UnknownPlayer { name } => vec![key::PLAYER_NOT_FOUND.text(vec![
            Text::literal(name.as_str()),
        ])],
        Refusal::NotAPlayer => vec![port_key::NOT_A_PLAYER.text(Vec::new())],
        Refusal::NotManager { rank } => {
            vec![port_key::NOT_MANAGER.text(vec![Text::literal(rank.name())])]
        }
        Refusal::Denied(denied) => match denied {
            PermissionRefusal::LacksAction { rank, action } => {
                vec![port_key::DENIED_LACKS.text(vec![
                    Text::literal(rank.name()),
                    Text::literal(format!("{action:?}")),
                ])]
            }
            PermissionRefusal::TargetIsOwner => vec![port_key::DENIED_OWNER.text(Vec::new())],
            PermissionRefusal::NotManager { rank } => {
                vec![port_key::NOT_MANAGER.text(vec![Text::literal(rank.name())])]
            }
            PermissionRefusal::NotAMember => vec![port_key::DENIED_NOT_MEMBER.text(Vec::new())],
        },
        Refusal::DisabledInConfig { flag } => {
            vec![key::DISABLED_IN_CONFIG.text(vec![Text::literal(*flag)])]
        }
    }
}

/// A player's name, or the uuid the record knows them by when no name was
/// given: a real value either way, never a guess.
fn name_of(fleet: &Fleet, player: Option<u128>) -> String {
    match player {
        Some(player) => fleet
            .name_of(player)
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::to_text(player)),
        None => "?".to_owned(),
    }
}

/// A line as one string, resolving a translation to its fallback: what a test
/// can compare against, and what the player reads until the key is translated.
#[cfg(test)]
pub(crate) fn plain(text: &Text) -> String {
    let mut out = match &text.content {
        TextContent::Literal(value) => value.clone(),
        TextContent::Translatable {
            key,
            fallback,
            arguments,
        } => {
            let template = fallback.as_deref().unwrap_or(key.as_str());
            let mut rendered = template.to_owned();
            for argument in arguments {
                rendered = rendered.replacen("%s", &plain(argument), 1);
            }
            rendered
        }
        TextContent::Keybind(key) => key.clone(),
        TextContent::Selector { pattern, .. } => pattern.clone(),
        TextContent::Score { name, objective } => format!("{name}:{objective}"),
        TextContent::Nbt { path, .. } => path.clone(),
        TextContent::Object(_) => String::new(),
    };
    for child in &text.children {
        out.push_str(&plain(child));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{Building, Citizen, Rank};
    use crate::commands::syntax::ColonyRef;

    const OWNER: u128 = 1;
    const FRIEND: u128 = 3;

    fn colony(id: i32) -> Colony {
        let mut colony = Colony::new(
            id,
            &format!("Colony {id}"),
            "minecraft:overworld",
            [id, 70, 0],
            OWNER,
        );
        colony.members.set(FRIEND, Rank::Friend);
        colony.citizens.push(Citizen::new(1, "Ada"));
        colony
            .buildings
            .push(Building::new("minecolonies:townhall", [1, 71, 1], 1));
        colony
    }

    fn lines(colony: Option<&Colony>, command: &ColonyCommand) -> Vec<String> {
        render(&Fleet::new(Vec::new()), colony, command).iter().map(plain).collect()
    }

    #[test]
    fn info_prints_javas_four_lines_then_one_line_per_member() {
        let colony = colony(7);
        let fleet = Fleet::new(vec![]).with_name(OWNER, "Ada");
        let command = ColonyCommand::Info {
            colony: ColonyRef::Id(7),
        };
        let read: Vec<String> = render(&fleet, Some(&colony), &command)
            .iter()
            .map(plain)
            .collect();
        assert_eq!(
            read,
            [
                "ID: 7 Name: Colony 7",
                "Mayor: Ada",
                "Citizens: 1",
                "Coordinates: x=7 y=70 z=0",
                "Members:",
                "Owner: Ada",
                "Friend: 00000000-0000-0000-0000-000000000003",
            ],
            "a member without a known name is shown by uuid, not by a guess"
        );
    }

    #[test]
    fn list_shows_nine_colonies_a_page_and_clamps_a_page_that_does_not_exist() {
        let fleet = Fleet::new((1..=10).map(colony).collect()).with_name(OWNER, "Ada");
        let page_one = list(&fleet, 1);
        assert_eq!(
            plain(&page_one[0]),
            "   ------------------ page 1 of 2 ------------------"
        );
        assert_eq!(page_one.len(), 2 + 9 * 2, "header, 18 lines, footer");
        assert_eq!(
            plain(&page_one[1]),
            "ID: 1  Name: Colony 1 Mayor: Ada",
            "two spaces after the id, as Java writes it"
        );
        assert_eq!(
            plain(&page_one[2]),
            "Citizens:1 Coordinates: x=1 y=70 z=0"
        );
        assert_eq!(
            plain(&page_one[page_one.len() - 1]),
            " ---------------- <- prev | next ->  ----------------"
        );

        let page_two = list(&fleet, 2);
        assert_eq!(
            plain(&page_two[0]),
            "   ------------------ page 2 of 2 ------------------"
        );
        assert_eq!(page_two.len(), 4, "header, one colony, footer");

        let beyond = list(&fleet, 9);
        assert_eq!(plain(&beyond[0]), plain(&page_one[0]), "a page out of range is page one");
    }

    #[test]
    fn an_empty_fleet_is_page_one_of_zero_with_no_colony_line() {
        let fleet = Fleet::new(Vec::new());
        let lines = list(&fleet, 1);
        assert_eq!(lines.len(), 2, "header and footer");
        assert_eq!(
            plain(&lines[0]),
            "   ------------------ page 1 of 0 ------------------",
            "Java's own arithmetic: floor(0/9) + 0"
        );
    }

    #[test]
    fn the_delete_prompt_names_both_answers_and_the_confirmation() {
        let colony = colony(7);
        let step_one = lines(
            Some(&colony),
            &ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: None,
                confirm: None,
            },
        );
        assert_eq!(step_one.len(), 1);
        assert!(step_one[0].contains("[keep]"), "{}", step_one[0]);
        assert!(step_one[0].contains("[delete]"), "{}", step_one[0]);

        let step_two = lines(
            Some(&colony),
            &ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: Some(true),
                confirm: None,
            },
        );
        assert!(step_two[0].contains("[confirm]"), "{}", step_two[0]);

        let kept = lines(
            Some(&colony),
            &ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: Some(false),
                confirm: None,
            },
        );
        assert!(
            !kept[0].contains("and every building"),
            "the buildings clause is added only when they go: {}",
            kept[0]
        );

        let done = lines(
            Some(&colony),
            &ColonyCommand::Delete {
                colony: ColonyRef::Id(7),
                buildings: Some(true),
                confirm: Some(true),
            },
        );
        assert_eq!(done, ["Colony 7 was deleted"]);
    }

    #[test]
    fn each_refusal_says_the_rule_that_fired() {
        let cases = [
            (
                Refusal::MissingSubcommand,
                "Usage: /colony info, list, here, tp, add, remove, rename, delete",
            ),
            (
                Refusal::UnknownSubcommand {
                    word: "destroy".to_owned(),
                },
                "Unknown colony subcommand: destroy",
            ),
            (
                Refusal::MissingArgument {
                    name: "colonyID",
                    usage: "colony info <colonyID:word>".to_owned(),
                },
                "Missing argument colonyID. Usage: colony info <colonyID:word>",
            ),
            (
                Refusal::BadPosition {
                    word: "~".to_owned(),
                },
                "~ is not a block position",
            ),
            (
                Refusal::UnknownColony {
                    reference: "@here".to_owned(),
                },
                "There is no colony @here",
            ),
            (
                Refusal::UnknownPlayer {
                    name: "Zoe".to_owned(),
                },
                "There is no player named Zoe",
            ),
            (
                Refusal::NotManager { rank: Rank::Friend },
                "Your rank (Friend) is not a colony manager",
            ),
            (
                Refusal::Denied(PermissionRefusal::LacksAction {
                    rank: Rank::Friend,
                    action: crate::colony::Action::EditPermissions,
                }),
                "Your rank (Friend) is not allowed to EditPermissions here",
            ),
            (
                Refusal::Denied(PermissionRefusal::TargetIsOwner),
                "The colony's owner can never be demoted or removed",
            ),
            (
                Refusal::DisabledInConfig {
                    flag: "canPlayerUseDeleteColonyCommand",
                },
                "That command is disabled in the server's config (canPlayerUseDeleteColonyCommand)",
            ),
        ];
        for (case, expected) in cases {
            assert_eq!(plain(&refusal(&case)[0]), expected, "{case:?}");
            assert!(
                case.name().starts_with("colony.command."),
                "{} is not namespaced",
                case.name()
            );
        }
        assert_eq!(Refusal::NotAPlayer.name(), "colony.command.not_a_player");
    }

    #[test]
    fn a_teleport_add_and_rename_answer_with_the_values_they_used() {
        let colony = colony(7);
        let teleport = lines(
            Some(&colony),
            &ColonyCommand::Teleport {
                colony: ColonyRef::Id(7),
            },
        );
        assert_eq!(teleport, ["Teleported to Colony 7"]);

        let added = lines(
            Some(&colony),
            &ColonyCommand::Add {
                colony: ColonyRef::Id(7),
                player: "Cy".to_owned(),
            },
        );
        assert_eq!(added, ["Cy is now an officer of Colony 7"]);

        let renamed = lines(
            Some(&colony),
            &ColonyCommand::Rename {
                colony: ColonyRef::Id(7),
                name: "New Bridge".to_owned(),
            },
        );
        assert_eq!(renamed, ["The colony is now called New Bridge"]);
    }

    #[test]
    fn an_answer_that_names_no_colony_is_empty_rather_than_invented() {
        // `run` resolves the colony of every subcommand but `list` before it
        // renders, so this path is a programming error, and an empty answer is
        // better than a line about a colony nobody named.
        assert!(render(
            &Fleet::new(Vec::new()),
            None,
            &ColonyCommand::Info {
                colony: crate::commands::ColonyRef::Id(7),
            },
        )
        .is_empty());
        // `list` is the one command whose answer is not about one colony.
        assert_eq!(
            render(
                &Fleet::new(Vec::new()),
                None,
                &ColonyCommand::List { page: 1 }
            )
            .len(),
            2
        );
    }
}
