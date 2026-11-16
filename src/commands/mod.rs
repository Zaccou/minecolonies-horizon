//! `/colony …`: the tree a player types, who may type it, and what it answers.
//!
//! Java reference: `core/commands/` (5 694 lines) — `CommandTree.java` for the
//! tree itself, `EntryPoint.java:42-68` for the `colony` subtree,
//! `CommandArgumentNames.java` for the argument names, and one file per
//! subcommand in `core/commands/colonycommands/`.
//!
//! **No host call.** A command that runs has already been parsed by the game's
//! dispatcher (`crates/horizon-mod-vocab/src/command/mod.rs:140-170`) and this
//! module is what decides, on data alone: what the words mean, whether the rank
//! allows it, what the record becomes, and what the player reads. The two
//! methods that will send and mutate the world later are named by
//! [`Effect`], never called: this crate does not register `/colony` yet, and
//! `RAPPORT_PORT_COMMANDS.md` says what the wiring still needs.
//!
//! **The four pieces.**
//!
//! | File | Question |
//! | --- | --- |
//! | `syntax` | what do the words mean, or which named refusal? |
//! | `rules` | may this player do it, and what does the record become? |
//! | `reply` | what does the player read, with which real values? |
//! | `mod` (here) | the tree, the pipeline [`run`], and the refusals |
//!
//! **One refusal type, one name.** Java reports a bad usage through Brigadier
//! (an exception with a message) and a missing right by *not registering the
//! branch*, so a stranger's `/colony delete` is "unknown or incomplete command"
//! — which is why the port needs its own name for each case: [`Refusal`], whose
//! [`Refusal::name`] is a stable, namespaced string a log or a test can match.

pub mod reply;
pub mod rules;
pub mod syntax;

use crate::colony::Refusal as PermissionRefusal;
use crate::colony::Rank;
use mod_api::Text;

pub use rules::{Actor, Config, Effect, Fleet, Requirement};
pub use syntax::{ColonyCommand, ColonyRef, parse};

/// Why a command did nothing. Every variant is a rule that fired, never a bare
/// `false`.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// `/colony` alone: Java's dispatcher answers "incomplete command"
    /// (`CommandTree.java:60-68` builds a bare literal with no executor).
    MissingSubcommand,
    /// A first word no subcommand answers to.
    UnknownSubcommand { word: String },
    /// A required argument is absent.
    MissingArgument {
        /// The argument's name, as Java spells it (`CommandArgumentNames.java:5-16`).
        name: &'static str,
        usage: String,
    },
    /// More words than the usage takes.
    UnexpectedArgument { word: String, usage: String },
    /// A word that is of the right *sort* but cannot be read as the argument's
    /// value: a page of `0`, a third delete word that is not `true`/`false`.
    BadArgument {
        name: &'static str,
        word: String,
        expected: &'static str,
    },
    /// Three words that are not a block position.
    BadPosition { word: String },
    /// The colony reference names no colony: `getColony` reports
    /// `COMMAND_COLONY_ID_NOT_FOUND` and throws (`ColonyIdArgument.java:103-117`).
    UnknownColony { reference: String },
    /// A player name the directory does not know (`CommandAddOfficer.java:47-52`).
    UnknownPlayer { name: String },
    /// A command only a player runs, run from the console (`IMCCommand.java:87-90`).
    NotAPlayer,
    /// The actor's rank is not a colony manager (`IMCColonyOfficerCommand.java:47`).
    NotManager { rank: Rank },
    /// The colony's own rule refused it (`Permissions.hasPermission`,
    /// `Permissions.java:639-650`), passed through with its name.
    Denied(PermissionRefusal),
    /// The server's config turns the subcommand off for everyone but an
    /// operator (`ServerConfiguration.java:152-159`).
    DisabledInConfig { flag: &'static str },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry.
    pub fn name(&self) -> &'static str {
        match self {
            Self::MissingSubcommand => "colony.command.missing_subcommand",
            Self::UnknownSubcommand { .. } => "colony.command.unknown_subcommand",
            Self::MissingArgument { .. } => "colony.command.missing_argument",
            Self::UnexpectedArgument { .. } => "colony.command.unexpected_argument",
            Self::BadArgument { .. } => "colony.command.bad_argument",
            Self::BadPosition { .. } => "colony.command.bad_position",
            Self::UnknownColony { .. } => "colony.command.unknown_colony",
            Self::UnknownPlayer { .. } => "colony.command.unknown_player",
            Self::NotAPlayer => "colony.command.not_a_player",
            Self::NotManager { .. } => "colony.command.not_manager",
            Self::Denied(_) => "colony.command.denied",
            Self::DisabledInConfig { .. } => "colony.command.disabled_in_config",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

/// The eight subcommands of `/colony`, in the order the tree lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the tree is a closed list, as Java's `EntryPoint` is
pub enum Subcommand {
    Info,
    List,
    Here,
    Teleport,
    Add,
    Remove,
    Rename,
    Delete,
}

impl Subcommand {
    /// Every subcommand, in tree order.
    pub const ALL: [Subcommand; 8] = [
        Subcommand::Info,
        Subcommand::List,
        Subcommand::Here,
        Subcommand::Teleport,
        Subcommand::Add,
        Subcommand::Remove,
        Subcommand::Rename,
        Subcommand::Delete,
    ];

    /// The word Java uses, or that this port adds where Java has none.
    pub fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::List => "list",
            Self::Here => "here",
            Self::Teleport => "tp",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Rename => "rename",
            Self::Delete => "delete",
        }
    }

    /// The other words Java answers to for the same command: `teleport` is
    /// `CommandTeleport.getName` (`CommandTeleport.java:54`) and `addOfficer`
    /// is `CommandAddOfficer.getName` (`CommandAddOfficer.java:65`). Both are kept,
    /// because a player's muscle memory is on Java's spelling.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Teleport => &["teleport"],
            Self::Add => &["addOfficer"],
            _ => &[],
        }
    }

    /// The argument list, in the SDK's usage syntax
    /// (`crates/horizon-mod-vocab/src/command/types.rs:1-16`), which is also
    /// the documentation of what the subcommand takes.
    pub fn tail(self) -> &'static str {
        match self {
            // Java: `ColonyIdArgument.id()` — a word, because it answers to
            // `@here`, `@mine`, a number, a uuid and a player name
            // (`ColonyIdArgument.java:37`, `:103-117`).
            Self::Info | Self::Teleport => "<colonyID:word>",
            // Java: `IntegerArgumentType.integer(1)`, `CommandListColonies.java:139-143`.
            Self::List => "[startpage:integer]",
            // Java: no argument — `HereOption` reads the source's position
            // (`ColonyIdArgument.java:140-150`). The optional position is the
            // port's way of making that rule testable without a source.
            Self::Here => "[pos:block_pos]",
            // Java: `<colonyID> <playername>`, the name resolved through the
            // profile cache (`CommandAddOfficer.java:41-51`).
            Self::Add | Self::Remove => "<colonyID:word> <playername:word>",
            // Java has no rename command: the name is set by the town hall's
            // GUI through `IColony.setName` (`api/colony/IColony.java:64`,
            // `core/colony/Colony.java:1284`). Greedy, like
            // `CommandSetRank`'s rank argument (`:101`).
            Self::Rename => "<colonyID:word> <name:text>",
            // Java: `[keep / delete buildings] [confirm]`, two booleans
            // (`CommandDeleteColony.java:44`, `:49`, usage `:52-63`).
            Self::Delete => "<colonyID:word> [buildings:boolean] [confirm:boolean]",
        }
    }
}

/// One subcommand of the tree: what it takes, and what it demands.
#[derive(Clone, Copy, Debug)]
// exhaustive: static data, read only
pub struct Spec {
    pub kind: Subcommand,
    /// The rank clause (`IMCCommand` vs `IMCColonyOfficerCommand`).
    pub authority: Requirement,
    /// The server-config switch that can turn it off for non-operators.
    pub gate: rules::Gate,
}

impl Spec {
    /// The usage as it will be registered: `colony info <colonyID:word>`.
    pub fn usage(&self) -> String {
        self.usage_as(self.kind.name())
    }

    /// The same usage under another word, which is how an alias is declared.
    pub fn usage_as(&self, word: &str) -> String {
        format!("colony {word} {}", self.kind.tail())
    }
}

/// The tree. **The one table**: `SPECS` decides the usages, the parser's word
/// list and the two authority checks, so none of the three can drift.
pub const SPECS: [Spec; 8] = [
    // `CommandColonyInfo` is an `IMCColonyOfficerCommand` (:19) and also reads
    // `canPlayerUseShowColonyInfoCommand` (:41).
    Spec {
        kind: Subcommand::Info,
        authority: Requirement::Manager,
        gate: rules::Gate::ShowInfo,
    },
    // `CommandListColonies` is a plain `IMCCommand` (:22): every player sees
    // the colony list, no config switch.
    Spec {
        kind: Subcommand::List,
        authority: Requirement::Everyone,
        gate: rules::Gate::Open,
    },
    // `CommandHomeTeleport` is a plain `IMCCommand` (:15) gated by
    // `canPlayerUseHomeTPCommand` (:27); the destination is a rank right, so
    // the port asks for `Action::TeleportToColony` on top.
    Spec {
        kind: Subcommand::Here,
        authority: Requirement::Action(crate::colony::Action::TeleportToColony),
        gate: rules::Gate::HomeTp,
    },
    // `CommandTeleport` is an `IMCColonyOfficerCommand` (:23) gated by
    // `canPlayerUseColonyTPCommand` (:35).
    Spec {
        kind: Subcommand::Teleport,
        authority: Requirement::Manager,
        gate: rules::Gate::ColonyTp,
    },
    // `CommandAddOfficer` is an `IMCColonyOfficerCommand` (:20) gated by
    // `canPlayerUseAddOfficerCommand` (:30). Java writes the OFFICER rank
    // (:54); the port asks the colony's own invitation rule first, which is
    // the `EDIT_PERMISSIONS` bit (`PermissionsMessage.java:485-488`).
    Spec {
        kind: Subcommand::Add,
        authority: Requirement::Invite,
        gate: rules::Gate::AddOfficer,
    },
    // No Java command: removing a player is the GUI message
    // `PermissionsMessage.RemovePlayer` (:496-568), whose three clauses are
    // `Members::may_expel`.
    Spec {
        kind: Subcommand::Remove,
        authority: Requirement::Expel,
        gate: rules::Gate::Open,
    },
    // No Java command either: `IColony.setName` (`api/colony/IColony.java:64`)
    // is reached from the town hall's GUI. Renaming is the same right as any
    // other permission edit, so it asks for the bit.
    Spec {
        kind: Subcommand::Rename,
        authority: Requirement::Action(crate::colony::Action::EditPermissions),
        gate: rules::Gate::Open,
    },
    // `CommandDeleteColony` is an `IMCColonyOfficerCommand` (:34) gated by
    // `canPlayerUseDeleteColonyCommand` (:150).
    Spec {
        kind: Subcommand::Delete,
        authority: Requirement::Manager,
        gate: rules::Gate::DeleteColony,
    },
];

/// The spec of one subcommand. Every variant is in the table, which a test
/// holds, so the `expect` cannot fire on a tree this crate built.
pub fn spec_of(kind: Subcommand) -> &'static Spec {
    SPECS
        .iter()
        .find(|spec| spec.kind == kind)
        .expect("the tree lists every subcommand")
}

/// The spec a typed word names, canonical or alias.
pub fn spec_for(word: &str) -> Option<&'static Spec> {
    SPECS
        .iter()
        .find(|spec| spec.kind.name() == word || spec.kind.aliases().contains(&word))
}

/// Every usage to register, aliases included: what a future `register_commands`
/// hands to `Command::new` (`crates/horizon-mod-vocab/src/hooks/register_commands.rs`).
pub fn usages() -> Vec<String> {
    let mut usages = Vec::new();
    for spec in &SPECS {
        usages.push(spec.usage());
        for alias in spec.kind.aliases() {
            usages.push(spec.usage_as(alias));
        }
    }
    usages
}

/// What a command answered, and what the caller must still do.
///
/// `success` is Java's return value, where `0` means "did nothing" and `1`
/// "ran" (`IMCCommand.java:96-98`); `lines` are the messages in the order they
/// are shown; `effect` is the part this crate cannot do without a host.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: this port's own answer, built only by `run`
pub struct Answer {
    pub success: bool,
    pub lines: Vec<Text>,
    pub effect: Effect,
}

impl Answer {
    /// The answer of a refusal: the rule that fired, said out loud.
    pub fn refused(refusal: &Refusal) -> Answer {
        Answer {
            success: false,
            lines: reply::refusal(refusal),
            effect: Effect::None,
        }
    }
}

/// The pipeline: words in, answer out. What a `run_mod_command` hook will call
/// once `/colony` is registered — nothing here needs a game.
///
/// The order is Java's for an officer command: the colony is resolved first
/// (`ColonyIdArgument.getColony` runs before `onExecute`, `:103-117`), then the
/// config gate and the rank (`checkPreCondition`, `IMCCommand.java:87-90` +
/// `IMCColonyOfficerCommand.java:22-48`), then the work.
pub fn run(fleet: &mut Fleet, actor: Actor, config: Config, words: &[&str]) -> Answer {
    let command = match parse(words) {
        Ok(command) => command,
        Err(refusal) => return Answer::refused(&refusal),
    };
    let spec = spec_of(command.subcommand());

    let id = match command.colony() {
        Some(reference) => match fleet.resolve(reference, actor) {
            Ok(id) => Some(id),
            Err(refusal) => return Answer::refused(&refusal),
        },
        None => None,
    };

    if let Err(refusal) = rules::gate(actor, spec.gate, config) {
        return Answer::refused(&refusal);
    }

    // The player a grant or an expulsion names.
    //
    // Order, and why it differs between the two: Java's officer clause runs
    // before `onExecute` looks the profile up (`IMCColonyOfficerCommand.java:22-48`
    // is `checkPreCondition`, `CommandAddOfficer.java:54-57` is the run), so a
    // refused `add` never says "no such player". `remove` has no Java command:
    // its rule is `Members::may_expel`, which cannot be asked without the
    // target, so the name is resolved first there.
    let mut target = None;
    if let ColonyCommand::Remove { player, .. } = &command {
        match fleet.player(player) {
            Some(uuid) => target = Some(uuid),
            None => {
                return Answer::refused(&Refusal::UnknownPlayer {
                    name: player.clone(),
                });
            }
        }
    }

    if let Some(id) = id {
        let colony = fleet.get(id).expect("resolved a moment ago");
        if let Err(refusal) = rules::authorize(colony, actor, spec.authority, target) {
            return Answer::refused(&refusal);
        }
    }

    if let ColonyCommand::Add { player, .. } = &command {
        match fleet.player(player) {
            Some(uuid) => target = Some(uuid),
            None => {
                return Answer::refused(&Refusal::UnknownPlayer {
                    name: player.clone(),
                });
            }
        }
    }

    let effect = match id {
        Some(id) => {
            let colony = fleet.get_mut(id).expect("resolved a moment ago");
            rules::apply(colony, &command, target)
        }
        None => Effect::None,
    };

    // The record is read back **after** `apply`: a `rename` answers with the
    // name it just wrote. `render` takes the record, not the id, so the lookup
    // happens once, here, where the fleet is already in hand.
    let colony = id.and_then(|id| fleet.get(id));

    Answer {
        success: true,
        lines: reply::render(fleet, colony, &command),
        effect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{Action, Building, Citizen, Colony};

    const OWNER: u128 = 1;
    const OFFICER: u128 = 2;
    const FRIEND: u128 = 3;

    /// `Bridgewater`, id 7, with one member of each rank and two citizens.
    fn fleet() -> Fleet {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], OWNER);
        colony.members.set(OFFICER, Rank::Officer);
        colony.members.set(FRIEND, Rank::Friend);
        colony.citizens.push(Citizen::new(1, "Ada"));
        colony.citizens.push(Citizen::new(2, "Bram"));
        colony
            .buildings
            .push(Building::new("minecolonies:blockhuttownhall", [4, 71, 4], 1));
        Fleet::new(vec![colony])
            .with_name(OWNER, "Ada")
            .with_name(OFFICER, "Bo")
            .with_name(FRIEND, "Cy")
            .with_here([0, 64, 0], 7)
    }

    fn text(answer: &Answer) -> Vec<String> {
        answer.lines.iter().map(reply::plain).collect()
    }

    #[test]
    fn the_tree_lists_eight_subcommands_and_every_usage_is_registrable() {
        assert_eq!(SPECS.len(), Subcommand::ALL.len());
        for (spec, kind) in SPECS.iter().zip(Subcommand::ALL) {
            assert_eq!(spec.kind, kind, "the table is in tree order");
            let usage = spec.usage();
            assert!(
                usage.starts_with(format!("colony {} ", kind.name()).as_str()),
                "{usage}"
            );
            assert_eq!(spec_for(kind.name()).map(|found| found.kind), Some(kind));
            for alias in kind.aliases() {
                assert_eq!(spec_for(alias).map(|found| found.kind), Some(kind));
                assert_ne!(*alias, kind.name(), "an alias is a second word");
            }
        }
        let usages = usages();
        assert_eq!(usages.len(), 10, "8 subcommands plus 2 Java aliases");
        assert!(usages.contains(&"colony teleport <colonyID:word>".to_owned()));
        assert!(usages.contains(&"colony addOfficer <colonyID:word> <playername:word>".to_owned()));
        assert!(spec_for("bridgewater").is_none(), "no subcommand answers to a colony name");
    }

    #[test]
    fn an_owner_reads_the_summary_of_the_colony_he_stands_in() {
        let mut fleet = fleet();
        let answer = run(
            &mut fleet,
            Actor::player(OWNER),
            Config::all_enabled(),
            &["info", "@here"],
        );
        assert!(answer.success);
        assert_eq!(
            text(&answer),
            [
                "ID: 7 Name: Bridgewater",
                "Mayor: Ada",
                "Citizens: 2",
                "Coordinates: x=0 y=70 z=0",
                "Members:",
                "Owner: Ada",
                "Officer: Bo",
                "Friend: Cy",
            ]
        );
        assert_eq!(answer.effect, Effect::None);
    }

    #[test]
    fn a_friend_is_refused_a_delete_by_a_named_rule_and_nothing_changes() {
        let mut fleet = fleet();
        let answer = run(
            &mut fleet,
            Actor::player(FRIEND),
            Config::all_enabled(),
            &["delete", "7", "true", "true"],
        );
        assert!(!answer.success);
        assert_eq!(
            text(&answer),
            ["Your rank (Friend) is not a colony manager"]
        );
        assert_eq!(answer.effect, Effect::None, "no colony is deleted");
        assert_eq!(fleet.len(), 1, "the fleet still holds it");
    }

    #[test]
    fn an_owner_walks_the_guided_delete_and_only_the_last_true_deletes() {
        let mut fleet = fleet();
        let owner = Actor::player(OWNER);
        let config = Config::all_enabled();

        // Step 1: `delete 7` asks whether the buildings go too (:122-146).
        let first = run(&mut fleet, owner, config, &["delete", "7"]);
        assert!(first.success, "Java answers 1, the command ran");
        assert_eq!(first.effect, Effect::None);
        assert_eq!(text(&first).len(), 1);
        assert!(text(&first)[0].contains("[keep]"), "{}", text(&first)[0]);
        assert!(text(&first)[0].contains("[delete]"), "{}", text(&first)[0]);

        // Step 2: the boolean given, the confirmation is asked (:152-174).
        let second = run(&mut fleet, owner, config, &["delete", "7", "true"]);
        assert_eq!(second.effect, Effect::None);
        assert!(text(&second)[0].contains("true"), "{}", text(&second)[0]);

        // Step 3: `false` is a cancellation that still "ran" (:207-210).
        let cancelled = run(&mut fleet, owner, config, &["delete", "7", "true", "false"]);
        assert!(cancelled.success);
        assert_eq!(cancelled.effect, Effect::None, "confirm=false deletes nothing");

        // Step 3: `true` is the deletion the hook must perform (:212-217).
        let deleted = run(&mut fleet, owner, config, &["delete", "7", "true", "true"]);
        assert!(deleted.success);
        assert_eq!(
            deleted.effect,
            Effect::Delete {
                colony: 7,
                buildings: true
            }
        );
        assert_eq!(text(&deleted), ["Bridgewater was deleted"]);
    }

    #[test]
    fn an_officer_may_not_hand_out_a_rank_because_the_bit_is_the_owners() {
        let mut fleet = fleet();
        let answer = run(
            &mut fleet,
            Actor::player(OFFICER),
            Config::all_enabled(),
            &["add", "7", "Dee"],
        );
        assert!(!answer.success);
        assert_eq!(
            answer.lines[0],
            reply::refusal(&Refusal::Denied(PermissionRefusal::LacksAction {
                rank: Rank::Officer,
                action: Action::EditPermissions,
            }))[0]
        );
    }

    #[test]
    fn an_operator_ignores_the_config_switch_a_player_does_not() {
        let mut fleet = fleet();
        let off = Config {
            delete_colony: false,
            ..Config::default()
        };
        let refused = run(
            &mut fleet,
            Actor::player(OWNER),
            off,
            &["delete", "7", "false", "true"],
        );
        assert!(!refused.success);
        assert_eq!(refused.lines, reply::refusal(&Refusal::DisabledInConfig {
            flag: "canPlayerUseDeleteColonyCommand"
        }));
        let allowed = run(
            &mut fleet,
            Actor::operator(OWNER),
            off,
            &["delete", "7", "false", "true"],
        );
        assert!(allowed.success, "an operator is not held by the config");
        assert_eq!(
            allowed.effect,
            Effect::Delete {
                colony: 7,
                buildings: false
            }
        );
    }

    #[test]
    fn a_rename_is_written_to_the_record_and_the_reply_names_the_new_one() {
        let mut fleet = fleet();
        let answer = run(
            &mut fleet,
            Actor::player(OWNER),
            Config::all_enabled(),
            &["rename", "7", "New", "Bridge"],
        );
        assert!(answer.success);
        assert_eq!(text(&answer), ["The colony is now called New Bridge"]);
        assert_eq!(fleet.get(7).map(|colony| colony.name.as_str()), Some("New Bridge"));
    }

    #[test]
    fn a_friend_may_list_the_colonies_and_teleport_home() {
        let mut fleet = fleet();
        let listed = run(
            &mut fleet,
            Actor::player(FRIEND),
            Config::all_enabled(),
            &["list"],
        );
        assert!(listed.success);
        assert_eq!(listed.lines.len(), 4, "header, two colony lines, footer");

        let here = run(
            &mut fleet,
            Actor::player(FRIEND),
            Config::all_enabled(),
            &["here"],
        );
        assert!(here.success);
        assert_eq!(
            here.effect,
            Effect::Teleport {
                colony: 7,
                dimension: "minecraft:overworld".to_owned(),
                at: [4, 71, 4],
            },
            "the town hall's position, not the colony centre"
        );
    }
}
