//! Who may run a subcommand, and what the colony record becomes once it runs.
//!
//! Java reference: `core/commands/commandTypes/` — `IMCCommand.java:16-114` (the
//! player-or-operator precondition), `IMCColonyOfficerCommand.java:22-48` (the
//! manager clause), and `api/configuration/ServerConfiguration.java:152-159`
//! (the five switches). The rank rules are **not** re-implemented here: they are
//! `colony::permissions`, and this file only decides which one a subcommand
//! asks for.
//!
//! **Two layers, as Java has them.** A command first passes a *gate* — the
//! server's config, which an operator ignores — then a *rank clause*. `info`
//! needs a manager (`IMCColonyOfficerCommand.java:47`), `list` needs nothing
//! (`IMCCommand.java:87-90`), `add` needs the `EDIT_PERMISSIONS` bit
//! (`PermissionsMessage.java:485`), `remove` needs the three clauses of an
//! expulsion (`PermissionsMessage.java:562-566`).
//!
//! **The fleet** is the port's `IColonyManager`: the colonies a command can
//! reach, plus the one place the world's answers cannot come from — where a
//! position is, and what a player is called. No claim geometry exists in this
//! port yet, so [`Fleet::with_here`] records the single
//! position→colony answer the caller has, and a position it was not told about
//! is *unknown* rather than guessed.

use std::collections::BTreeMap;

use crate::colony::permissions::uuid;
use crate::colony::{Action, Colony, Rank, Refusal as PermissionRefusal};

use super::syntax::{ColonyCommand, ColonyRef};
use super::Refusal;

/// Who is running the command. Java reads the same two facts: the entity is a
/// player or not (`IMCCommand.java:87-90`), and the permission level is 4
/// (`:16`, `:107-114`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two facts, read from the command source
pub struct Actor {
    /// The player's uuid, or `None` for the console and a command block.
    pub player: Option<u128>,
    /// Permission level 4, or the player being on the ops list
    /// (`IMCCommand.isPlayerOped`, `:107-114`). Either bypasses the config and
    /// every rank clause.
    pub op: bool,
}

impl Actor {
    /// A player with no operator rights.
    pub fn player(player: u128) -> Actor {
        Actor {
            player: Some(player),
            op: false,
        }
    }

    /// A player at permission level 4.
    pub fn operator(player: u128) -> Actor {
        Actor {
            player: Some(player),
            op: true,
        }
    }

    /// The server console: no player, permission level 4
    /// (`crates/horizon-mod-vocab/src/command/mod.rs:40-44`).
    pub fn console() -> Actor {
        Actor {
            player: None,
            op: true,
        }
    }
}

/// The server-config switch a subcommand reads, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five switches the colony tree reads
pub enum Gate {
    /// No switch: the command is always available.
    Open,
    ShowInfo,
    HomeTp,
    ColonyTp,
    AddOfficer,
    DeleteColony,
}

impl Gate {
    /// The config field's name, exactly as `ServerConfiguration` spells it, so
    /// a refusal can tell an admin which line to change.
    pub fn flag(self) -> Option<&'static str> {
        match self {
            Self::Open => None,
            Self::ShowInfo => Some("canPlayerUseShowColonyInfoCommand"),
            Self::HomeTp => Some("canPlayerUseHomeTPCommand"),
            Self::ColonyTp => Some("canPlayerUseColonyTPCommand"),
            Self::AddOfficer => Some("canPlayerUseAddOfficerCommand"),
            Self::DeleteColony => Some("canPlayerUseDeleteColonyCommand"),
        }
    }
}

/// The five switches, with Java's defaults — which are **not** all on:
/// `canplayerusecolonytpcommand` and `canplayerusedeletecolonycommand` are
/// `false` (`ServerConfiguration.java:152`, `:158`), so a stock server leaves
/// those two to operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: five booleans, matching the config fields
pub struct Config {
    pub show_info: bool,
    pub home_tp: bool,
    pub colony_tp: bool,
    pub add_officer: bool,
    pub delete_colony: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            show_info: true,
            home_tp: false,
            colony_tp: false,
            add_officer: true,
            delete_colony: false,
        }
    }
}

impl Config {
    /// Every switch on: the test setting, and what an operator sees anyway.
    pub fn all_enabled() -> Config {
        Config {
            show_info: true,
            home_tp: true,
            colony_tp: true,
            add_officer: true,
            delete_colony: true,
        }
    }

    /// Whether the switch lets a non-operator through.
    pub fn allows(self, gate: Gate) -> bool {
        match gate {
            Gate::Open => true,
            Gate::ShowInfo => self.show_info,
            Gate::HomeTp => self.home_tp,
            Gate::ColonyTp => self.colony_tp,
            Gate::AddOfficer => self.add_officer,
            Gate::DeleteColony => self.delete_colony,
        }
    }
}

/// The rank clause a subcommand demands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four clauses the tree uses
pub enum Requirement {
    /// Any player (`IMCCommand.java:87-90`).
    Everyone,
    /// Owner or officer (`Rank::is_manager`, `IMCColonyOfficerCommand.java:47`).
    Manager,
    /// The colony's own bit for an action (`Permissions.hasPermission`,
    /// `Permissions.java:639-650`).
    Action(Action),
    /// Handing out a rank: the `EDIT_PERMISSIONS` bit, and never ownership
    /// (`PermissionsMessage.java:485`).
    Invite,
    /// Taking a player out: the three clauses of `RemovePlayer`
    /// (`PermissionsMessage.java:562-566`).
    Expel,
}

/// The config gate: an operator ignores it (`CommandColonyInfo.java:41` reads
/// the switch only after `hasPermission(OP_PERM_LEVEL)` failed).
pub(crate) fn gate(actor: Actor, gate: Gate, config: Config) -> Result<(), Refusal> {
    let Some(flag) = gate.flag() else {
        return Ok(());
    };
    if actor.op || config.allows(gate) {
        Ok(())
    } else {
        Err(Refusal::DisabledInConfig { flag })
    }
}

/// The rank clause. `target` is the player a grant or an expulsion names, and
/// is unused by the other three.
pub fn authorize(
    colony: &Colony,
    actor: Actor,
    requirement: Requirement,
    target: Option<u128>,
) -> Result<(), Refusal> {
    let Some(player) = actor.player else {
        // No rank to read: Java lets a non-player through only at level 4.
        return if actor.op {
            Ok(())
        } else {
            Err(Refusal::NotAPlayer)
        };
    };
    // `hasPermission(OP_PERM_LEVEL)` is checked first and short-circuits the
    // rank (`IMCColonyOfficerCommand.java:22-27`).
    if actor.op {
        return Ok(());
    }
    match requirement {
        Requirement::Everyone => Ok(()),
        Requirement::Manager => {
            let rank = colony.members.rank_of(player);
            if rank.is_manager() {
                Ok(())
            } else {
                Err(Refusal::NotManager { rank })
            }
        }
        Requirement::Action(action) => colony.may(player, action).map_err(Refusal::Denied),
        Requirement::Invite => colony
            .may_invite(player, Rank::Officer)
            .map_err(Refusal::Denied),
        Requirement::Expel => {
            // `may_expel` takes the target, so an unresolved one is the
            // `NotAMember` case it answers for an unknown uuid
            // (`permissions.rs:311-315`).
            let target = target
                .ok_or(PermissionRefusal::NotAMember)
                .map_err(Refusal::Denied)?;
            colony.may_expel(player, target).map_err(Refusal::Denied)
        }
    }
}

/// What the caller must still do, once the record has been written. Both
/// variants are world actions this crate cannot perform: no host is called
/// here (see the module doc of `commands`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the two world actions a colony command needs
pub enum Effect {
    /// Nothing outside the colony record changed.
    None,
    /// Move the player who ran it (`TeleportHelper.colonyTeleport`,
    /// `core/util/TeleportHelper.java:140-195`).
    Teleport {
        colony: i32,
        /// `colony.getDimension()` (`Colony.java:917`).
        dimension: String,
        at: [i32; 3],
    },
    /// Remove the colony and, when `buildings`, its huts
    /// (`IColonyManager.deleteColonyByDimension`,
    /// `CommandDeleteColony.java:172-175`).
    Delete { colony: i32, buildings: bool },
}

/// Write what the command changes in the record, and answer with the rest.
///
/// The mutations are the four Java makes: a name (`Colony.setName`,
/// `Colony.java:1284`), a rank (`Permissions.setPlayerRank`, `:723`), a
/// removal (`removePlayer`, `:876-885`) and a deletion, which is the manager's
/// to perform (`CommandDeleteColony.java:167-175`).
pub fn apply(colony: &mut Colony, command: &ColonyCommand, target: Option<u128>) -> Effect {
    match command {
        ColonyCommand::Teleport { .. } | ColonyCommand::Here { .. } => Effect::Teleport {
            colony: colony.id,
            dimension: colony.dimension.clone(),
            at: teleport_target(colony),
        },
        ColonyCommand::Add { .. } => {
            if let Some(target) = target {
                colony.members.set(target, Rank::Officer);
            }
            Effect::None
        }
        ColonyCommand::Remove { .. } => {
            // Already authorized; the owner case is refused by `remove` itself
            // (`Permissions.java:876-885`) and never reaches here.
            if let Some(target) = target {
                let _ = colony.members.remove(target);
            }
            Effect::None
        }
        ColonyCommand::Rename { name, .. } => {
            colony.name = name.clone();
            Effect::None
        }
        ColonyCommand::Delete {
            buildings, confirm, ..
        } => match (buildings, confirm) {
            // `if (!confirmation) { return 1; }` — a cancelled delete ran and
            // changed nothing (`CommandDeleteColony.java:167-170`).
            (Some(buildings), Some(true)) => Effect::Delete {
                colony: colony.id,
                buildings: *buildings,
            },
            _ => Effect::None,
        },
        ColonyCommand::Info { .. } | ColonyCommand::List { .. } => Effect::None,
    }
}

/// The colonies a command can reach, and the two answers only the world has.
#[derive(Clone, Debug, Default)]
// exhaustive: this port's own view of `IColonyManager`
pub struct Fleet {
    colonies: Vec<Colony>,
    /// uuid → name. Java reads the server's profile cache
    /// (`ColonyIdArgument.java:242-244`); the port has no cache, so the caller
    /// fills this from the host and a missing name stays missing.
    names: BTreeMap<u128, String>,
    /// One answer to "which colony is at this position": the claim map is a
    /// later module and the port must not guess it.
    here: Option<([i32; 3], i32)>,
}

impl Fleet {
    /// A fleet over the given colonies.
    pub fn new(colonies: Vec<Colony>) -> Fleet {
        Fleet {
            colonies,
            names: BTreeMap::new(),
            here: None,
        }
    }

    /// Records a player's name, as the profile cache would have it.
    pub fn with_name(mut self, player: u128, name: &str) -> Fleet {
        self.names.insert(player, name.to_owned());
        self
    }

    /// Records that `at` is inside the colony `colony` (`getIColony(level, pos)`,
    /// `ColonyIdArgument.java:140-150`).
    pub fn with_here(mut self, at: [i32; 3], colony: i32) -> Fleet {
        self.here = Some((at, colony));
        self
    }

    /// How many colonies the fleet holds.
    pub fn len(&self) -> usize {
        self.colonies.len()
    }

    /// Whether the fleet holds none.
    pub fn is_empty(&self) -> bool {
        self.colonies.is_empty()
    }

    /// A colony by id.
    pub fn get(&self, id: i32) -> Option<&Colony> {
        self.colonies.iter().find(|colony| colony.id == id)
    }

    /// A colony by id, to write in.
    pub fn get_mut(&mut self, id: i32) -> Option<&mut Colony> {
        self.colonies.iter_mut().find(|colony| colony.id == id)
    }

    /// Every colony, in the fleet's order — the order `list` pages through
    /// (`IColonyManager.getAllColonies`, `CommandListColonies.java:83`).
    pub fn iter(&self) -> impl Iterator<Item = &Colony> + '_ {
        self.colonies.iter()
    }

    /// A player's name, when the caller gave one.
    pub fn name_of(&self, player: u128) -> Option<&str> {
        self.names.get(&player).map(String::as_str)
    }

    /// The uuid a typed name names, case-insensitively — `GameProfileCache`
    /// keys names in lower case, which is why `ada` and `Ada` are one player
    /// (`CommandAddOfficer.java:41-51`).
    pub fn player(&self, name: &str) -> Option<u128> {
        self.names
            .iter()
            .find(|(_, known)| known.eq_ignore_ascii_case(name))
            .map(|(player, _)| *player)
    }

    /// The colony owned by a player: `getIColonyByOwner` (`:119-127`).
    pub fn owned_by(&self, player: u128) -> Option<&Colony> {
        self.colonies
            .iter()
            .find(|colony| owner(colony) == Some(player))
    }

    /// **The resolution.** Which colony id does this reference name?
    /// `Ok(id)` is the answer; every error is the rule that fired, and the
    /// string in it is what the player typed.
    pub fn resolve(&self, reference: &ColonyRef, actor: Actor) -> Result<i32, Refusal> {
        match reference {
            ColonyRef::Here { at } => match self.here {
                Some((known, id))
                    if at.is_none_or(|asked| asked == known) && self.get(id).is_some() =>
                {
                    Ok(id)
                }
                _ => Err(Refusal::UnknownColony {
                    reference: match at {
                        Some(at) => position_text(*at),
                        None => "@here".to_owned(),
                    },
                }),
            },
            ColonyRef::Mine => {
                let player = actor.player.ok_or(Refusal::NotAPlayer)?;
                self.owned_by(player)
                    .map(|colony| colony.id)
                    .ok_or_else(|| Refusal::UnknownColony {
                        reference: "@mine".to_owned(),
                    })
            }
            ColonyRef::Id(id) => self
                .get(*id)
                .map(|colony| colony.id)
                .ok_or_else(|| Refusal::UnknownColony {
                    reference: id.to_string(),
                }),
            ColonyRef::OwnerUuid(owner) => self
                .owned_by(*owner)
                .map(|colony| colony.id)
                .ok_or_else(|| Refusal::UnknownColony {
                    reference: uuid::to_text(*owner),
                }),
            ColonyRef::OwnerName(name) => {
                // Two distinct Java errors: a name nobody answers to is
                // `ERROR_UNKNOWN_PLAYER`, a name that answers but owns nothing
                // is `ERROR_UNKNOWN_COLONY` (`:119-121`, `:250-254`).
                let owner = self
                    .player(name)
                    .ok_or_else(|| Refusal::UnknownPlayer { name: name.clone() })?;
                self.owned_by(owner)
                    .map(|colony| colony.id)
                    .ok_or_else(|| Refusal::UnknownColony {
                        reference: name.clone(),
                    })
            }
        }
    }
}

/// The colony's owner: the one member whose rank is [`Rank::Owner`]
/// (`Permissions.getOwner`, `Permissions.java:538`).
pub fn owner(colony: &Colony) -> Option<u128> {
    colony
        .members
        .iter()
        .find(|(_, rank)| *rank == Rank::Owner)
        .map(|(player, _)| player)
}

/// The town hall's position, when the record holds one.
///
/// Two spellings meet here: the colony record's tests key a building by its
/// **block** id (`minecolonies:blockhutbuilder`), while
/// `NbtTagConstants.java:140` stores the building's **type** path
/// (`minecolonies:townhall`). Both are read.
pub fn town_hall(colony: &Colony) -> Option<[i32; 3]> {
    colony
        .buildings
        .iter()
        .find(|building| is_town_hall(&building.kind))
        .map(|building| building.at)
}

/// Where a teleport lands: the town hall, else the colony's centre
/// (`TeleportHelper.colonyTeleport`, `core/util/TeleportHelper.java:140-160`).
pub fn teleport_target(colony: &Colony) -> [i32; 3] {
    town_hall(colony).unwrap_or(colony.center)
}

fn is_town_hall(kind: &str) -> bool {
    let path = kind.strip_prefix("minecolonies:").unwrap_or(kind);
    path.strip_prefix("blockhut").unwrap_or(path) == "townhall"
}

/// A position as the player typed it, for a refusal.
fn position_text(at: [i32; 3]) -> String {
    format!("{} {} {}", at[0], at[1], at[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Building;

    const OWNER: u128 = 1;
    const OFFICER: u128 = 2;
    const FRIEND: u128 = 3;
    const STRANGER: u128 = 4;

    fn colony() -> Colony {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], OWNER);
        colony.members.set(OFFICER, Rank::Officer);
        colony.members.set(FRIEND, Rank::Friend);
        colony.buildings.push(Building::new(
            "minecolonies:blockhuttownhall",
            [4, 71, 4],
            1,
        ));
        colony
    }

    fn fleet() -> Fleet {
        Fleet::new(vec![colony()])
            .with_name(OWNER, "Ada")
            .with_name(FRIEND, "Cy")
            .with_here([0, 64, 0], 7)
    }

    #[test]
    fn javas_defaults_leave_colony_teleport_and_delete_to_operators() {
        let config = Config::default();
        assert_eq!(
            (
                config.show_info,
                config.home_tp,
                config.colony_tp,
                config.add_officer,
                config.delete_colony
            ),
            (true, false, false, true, false),
            "ServerConfiguration.java:152-159"
        );
        assert!(!config.allows(Gate::DeleteColony));
        assert!(config.allows(Gate::Open), "a command with no switch is on");
        assert_eq!(Gate::Open.flag(), None);
        assert_eq!(
            Gate::DeleteColony.flag(),
            Some("canPlayerUseDeleteColonyCommand")
        );
    }

    #[test]
    fn an_operator_ignores_the_switch_and_a_player_does_not() {
        let off = Config::default();
        assert_eq!(
            gate(Actor::player(OWNER), Gate::ColonyTp, off),
            Err(Refusal::DisabledInConfig {
                flag: "canPlayerUseColonyTPCommand"
            })
        );
        assert_eq!(gate(Actor::operator(OWNER), Gate::ColonyTp, off), Ok(()));
        assert_eq!(gate(Actor::player(OWNER), Gate::Open, off), Ok(()));
    }

    #[test]
    fn the_rank_ladder_answers_the_four_clauses() {
        let colony = colony();
        let manager = |requirement, actor| authorize(&colony, actor, requirement, None);

        assert_eq!(
            manager(Requirement::Manager, Actor::player(OWNER)),
            Ok(())
        );
        assert_eq!(
            manager(Requirement::Manager, Actor::player(OFFICER)),
            Ok(())
        );
        assert_eq!(
            manager(Requirement::Manager, Actor::player(FRIEND)),
            Err(Refusal::NotManager { rank: Rank::Friend })
        );
        assert_eq!(
            manager(Requirement::Everyone, Actor::player(STRANGER)),
            Ok(()),
            "a stranger may list the colonies"
        );
        assert_eq!(
            manager(
                Requirement::Action(Action::TeleportToColony),
                Actor::player(FRIEND)
            ),
            Ok(()),
            "the friend mask carries the teleport bit"
        );
        assert_eq!(
            manager(
                Requirement::Action(Action::TeleportToColony),
                Actor::player(STRANGER)
            ),
            Err(Refusal::Denied(PermissionRefusal::LacksAction {
                rank: Rank::Neutral,
                action: Action::TeleportToColony
            }))
        );
        assert_eq!(
            manager(Requirement::Invite, Actor::player(OFFICER)),
            Err(Refusal::Denied(PermissionRefusal::LacksAction {
                rank: Rank::Officer,
                action: Action::EditPermissions
            })),
            "the command's manager clause is not enough to hand out a rank"
        );
    }

    #[test]
    fn an_expulsion_needs_the_target_and_the_owner_is_never_expelled() {
        let colony = colony();
        assert_eq!(
            authorize(
                &colony,
                Actor::player(OWNER),
                Requirement::Expel,
                Some(FRIEND)
            ),
            Ok(())
        );
        assert_eq!(
            authorize(
                &colony,
                Actor::player(FRIEND),
                Requirement::Expel,
                Some(OFFICER)
            ),
            Err(Refusal::Denied(PermissionRefusal::LacksAction {
                rank: Rank::Friend,
                action: Action::EditPermissions
            }))
        );
        assert_eq!(
            authorize(
                &colony,
                Actor::player(OWNER),
                Requirement::Expel,
                Some(OWNER)
            ),
            Err(Refusal::Denied(PermissionRefusal::TargetIsOwner))
        );
        assert_eq!(
            authorize(&colony, Actor::player(OWNER), Requirement::Expel, None),
            Err(Refusal::Denied(PermissionRefusal::NotAMember))
        );
    }

    #[test]
    fn the_console_has_no_rank_and_only_an_operator_passes() {
        let colony = colony();
        assert_eq!(
            authorize(&colony, Actor::console(), Requirement::Manager, None),
            Ok(()),
            "the console runs at level 4"
        );
        let unprivileged = Actor {
            player: None,
            op: false,
        };
        assert_eq!(
            authorize(&colony, unprivileged, Requirement::Everyone, None),
            Err(Refusal::NotAPlayer)
        );
    }

    #[test]
    fn the_fleet_resolves_the_five_references_and_names_what_it_cannot() {
        let fleet = fleet();
        let ada = Actor::player(OWNER);
        assert_eq!(fleet.resolve(&ColonyRef::Here { at: None }, ada), Ok(7));
        assert_eq!(
            fleet.resolve(
                &ColonyRef::Here {
                    at: Some([0, 64, 0])
                },
                ada
            ),
            Ok(7)
        );
        assert_eq!(
            fleet.resolve(&ColonyRef::Here { at: Some([9, 9, 9]) }, ada),
            Err(Refusal::UnknownColony {
                reference: "9 9 9".to_owned()
            })
        );
        assert_eq!(fleet.resolve(&ColonyRef::Mine, ada), Ok(7));
        assert_eq!(
            fleet.resolve(&ColonyRef::Mine, Actor::player(STRANGER)),
            Err(Refusal::UnknownColony {
                reference: "@mine".to_owned()
            })
        );
        assert_eq!(fleet.resolve(&ColonyRef::Id(7), ada), Ok(7));
        assert_eq!(
            fleet.resolve(&ColonyRef::Id(8), ada),
            Err(Refusal::UnknownColony {
                reference: "8".to_owned()
            })
        );
        assert_eq!(fleet.resolve(&ColonyRef::OwnerUuid(OWNER), ada), Ok(7));
        assert_eq!(
            fleet.resolve(&ColonyRef::OwnerName("ADA".to_owned()), ada),
            Ok(7),
            "names are read case-insensitively"
        );
        assert_eq!(
            fleet.resolve(&ColonyRef::OwnerName("Zoe".to_owned()), ada),
            Err(Refusal::UnknownPlayer {
                name: "Zoe".to_owned()
            })
        );
        assert_eq!(
            fleet.resolve(&ColonyRef::OwnerName("Cy".to_owned()), ada),
            Err(Refusal::UnknownColony {
                reference: "Cy".to_owned()
            }),
            "Cy exists but owns nothing: Java tells the two errors apart"
        );
    }

    #[test]
    fn a_teleport_lands_on_the_town_hall_and_falls_back_to_the_centre() {
        let mut colony = colony();
        assert_eq!(town_hall(&colony), Some([4, 71, 4]));
        assert_eq!(teleport_target(&colony), [4, 71, 4]);
        colony.buildings.clear();
        assert_eq!(town_hall(&colony), None);
        assert_eq!(teleport_target(&colony), [0, 70, 0]);
        // The other spelling of the same building.
        colony
            .buildings
            .push(Building::new("minecolonies:townhall", [8, 72, 8], 2));
        assert_eq!(teleport_target(&colony), [8, 72, 8]);
    }

    #[test]
    fn apply_writes_a_rename_a_rank_and_a_removal_and_never_the_owner() {
        let mut colony = colony();

        let renamed = ColonyCommand::Rename {
            colony: ColonyRef::Id(7),
            name: "New Bridge".to_owned(),
        };
        assert_eq!(apply(&mut colony, &renamed, None), Effect::None);
        assert_eq!(colony.name, "New Bridge");

        let added = ColonyCommand::Add {
            colony: ColonyRef::Id(7),
            player: "Cy".to_owned(),
        };
        apply(&mut colony, &added, Some(STRANGER));
        assert_eq!(colony.members.rank_of(STRANGER), Rank::Officer);

        let removed = ColonyCommand::Remove {
            colony: ColonyRef::Id(7),
            player: "Cy".to_owned(),
        };
        apply(&mut colony, &removed, Some(FRIEND));
        assert!(!colony.members.contains(FRIEND));

        apply(&mut colony, &removed, Some(OWNER));
        assert!(colony.members.contains(OWNER), "an owner is never removed");
    }
}
