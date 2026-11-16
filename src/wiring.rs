//! The wiring: where the hooks meet `colony`, `commands` and the world.
//!
//! **One module, because the four hooks would otherwise repeat the same three
//! steps**: read a colony out of the world's data, hand it to `commands` or to
//! the menu, and write it back. Nothing here is a rule — the rules are in
//! `colony` and `commands` — and nothing here is a new host method: every call
//! is declared in `crates/horizon-mod-vocab/src/host/`.
//!
//! # Where a colony lives, and why
//!
//! **The port has no colony store, and this slice does not invent one.** In Java
//! the record is a file the manager writes (`core/colony/ColonyManager`, one
//! `Colonies.dat` per world), and a wasm mod cannot read a save file: there is
//! no `std::fs` here, and the host exposes no world-save handle. What a wasm mod
//! *does* get, and what the hut already writes its own state to, is a **block
//! entity's saved compound** — persistent, chunked, no Wire depth limit
//! (`docs/modding/src/concepts/parts/block-entity-types.md:95-107`).
//!
//! So the port anchors the record where MineColonies anchors the colony in the
//! world: **on the main building**. In Java the town hall's block entity holds
//! the colony's id and every hut points at it (`TileEntityColonyBuilding`); here
//! it holds the record itself, nested under [`KEY_RECORD`], and every hut points
//! at it with the two keys `crate::hut::with_colony` writes:
//!
//! ```text
//! builder's hut (block entity at H)      main building (block entity at M)
//!   colony: 7            ← id             colonyRecord: { id: 7, name: "…",
//!   main:   {x,y,z} = M  ← the anchor       citizens: […], buildings: […] }
//! ```
//!
//! Three consequences, all of them honest costs:
//!
//! - **One copy.** A hut stores two ints and a position, never a second copy of
//!   the record; the record is read through the link each time, and a stale link
//!   (the record's own `id` disagrees with the hut's) is not followed.
//! - **No index.** Nothing enumerates the colonies of a world: `/colony` reads
//!   the one whose block entity the command source stands on (see [`anchor_at`]),
//!   not all of them. `list` therefore shows one line, and an id nobody can
//!   reach is `UnknownColony`. A claim map (`getIColony(level, pos)`) is the
//!   missing module; when it exists, only [`World::read`] changes.
//! - **A deleted record is an empty compound**, because `merge_data` merges and
//!   cannot remove a key (`crates/horizon-mod-vocab/src/host/block_entities.rs:107-120`).
//!   [`record_in`] reads an id of `0` as "no colony", which is what
//!   `Colony::from_compound` gives an empty compound; the block entity stays.

use crate::colony::permissions::uuid;
use crate::colony::Colony;
use crate::commands::{self, Actor, ColonyCommand, ColonyRef, Config, Effect, Fleet};
use crate::hut;
use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, CommandSource, Compound, DimensionKey, Flow,
    Host, LevelRef, PermissionLevel, RegisterCommandsEvent, RunModCommandEvent, ServerHost, Tag,
    Text,
};

/// The root literal of the tree this module registers, as Java spells it
/// (`CommandTree.java:60-68`).
pub const ROOT: &str = "colony";
/// The key a colony record is nested under, in the main building's compound.
/// Not `colony`: that key, on the main building, is the hut's own link.
pub const KEY_RECORD: &str = "colonyRecord";
/// What separates the click's status line from the colony's summary on the
/// action bar, the only line a right click can show (`crate::menu`).
pub const SEPARATOR: &str = " | ";

/// Why a record could not be written back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal, closed on purpose
pub enum WriteRefusal {
    /// Nothing stands where the record belongs: no block entity there, or the
    /// engine refused the merge (a stale handle, a full compound).
    NoRecordHolder,
}

/// `register_commands`: the ten usages of the tree, aliases included, from the
/// one table `commands::SPECS` holds.
pub fn register_commands(_host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
    event.commands.extend(
        commands::usages()
            .into_iter()
            .map(|usage| Command::new(&usage)),
    );
}

/// `run_mod_command`: `/colony …`, answered by `commands::run` and written back.
///
/// `Flow::Continue` for a command that is not this tree's: a mod sharing the
/// root literal keeps its own words, and `/mcproto` is answered before this runs
/// (`crate::lib`).
pub fn run(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let invocation = event.command;
    if !is_colony(invocation) {
        return Flow::Continue;
    }
    let words = words(invocation);
    let asked: Vec<&str> = words.iter().map(String::as_str).collect();

    let mut world = World::read(&*host, invocation);
    let answer = commands::run(&mut world.fleet, actor(event.source), Config::default(), &asked);
    for line in &answer.lines {
        let _ = host.commands().feedback(line.clone());
    }
    if answer.success {
        world.apply(&mut *host, &answer.effect, &asked, event.source);
    }
    // The port answered, refusal included: no other mod is asked about a word
    // this tree claimed.
    Flow::Cancel
}

/// The colony a hut names, read back through its link: the id the hut stores,
/// the record at the main building, and `None` when the two disagree.
///
/// `None` covers two cases a caller must tell apart: a hut that belongs to no
/// colony (`state.colony` is `None`), and a hut whose colony is gone
/// (`state.colony` is `Some`). `crate::hire` refuses the second and keeps the
/// first on the hut's own count.
pub fn colony_of(
    host: &dyn Host,
    level: LevelRef,
    state: &hut::State,
) -> Option<(Colony, [i32; 3])> {
    let (id, main) = (state.colony?, state.main?);
    let colony = record_at(host, level, main)?;
    (colony.id == id).then_some((colony, main))
}

/// The id the next citizen of `colony` takes: the highest saved id plus one,
/// recomputed on read and never stored, exactly as the work-order counter is
/// (`WorkManager.java:274`, `:305-311`).
pub fn next_citizen_id(colony: &Colony) -> i32 {
    colony
        .citizens
        .iter()
        .map(|citizen| citizen.id)
        .max()
        .unwrap_or(0)
        + 1
}

/// Writes `colony` back where it was read: one merge of one key, so the block
/// entity keeps whatever else it holds.
pub fn write_colony(
    host: &mut dyn ServerHost,
    level: LevelRef,
    at: [i32; 3],
    colony: &Colony,
) -> Result<(), WriteRefusal> {
    let entity = host
        .block_entity(level, at)
        .ok_or(WriteRefusal::NoRecordHolder)?;
    let payload = Compound::from([(KEY_RECORD.into(), Tag::Compound(colony.to_compound()))]);
    entity
        .merge_data(&mut *host, payload)
        .map_err(|_| WriteRefusal::NoRecordHolder)?;
    Ok(())
}

/// The colony's summary on a click, built by the same `commands::reply` the
/// command answers with: the status line, then every line of `info`.
///
/// A right click has one channel for text — the action bar
/// (`PlayerRef::action_bar`) — and one line is all it shows, so the lines are
/// joined instead of sent one after another, where each would overwrite the
/// last.
pub fn summary(status: Text, colony: &Colony) -> Text {
    let fleet = Fleet::new(vec![colony.clone()]);
    let command = ColonyCommand::Info {
        colony: ColonyRef::Id(colony.id),
    };
    let mut line = status;
    for part in commands::reply::render(&fleet, Some(colony), &command) {
        line = line.append(Text::literal(SEPARATOR)).append(part);
    }
    line
}

/// Whether `invocation` is this tree's: the root literal, and nothing else.
pub fn is_colony(invocation: &CommandInvocation) -> bool {
    invocation
        .literals
        .first()
        .is_some_and(|root| root == ROOT)
}

/// The words `commands::parse` reads, rebuilt from the dispatcher's invocation:
/// the literals after the root, then every argument **in parse order**, which is
/// the order `CommandArgumentValue` is documented to arrive in
/// (`crates/horizon-mod-vocab/src/command/mod.rs:147-154`).
///
/// `BlockPos` becomes three words because that is what the tree's usage says
/// (`here [pos:block_pos]`), and `parse` already resolves `~` — the value it
/// receives is the resolved block, so the three words are exact.
pub fn words(invocation: &CommandInvocation) -> Vec<String> {
    let mut words: Vec<String> = invocation.literals.iter().skip(1).cloned().collect();
    for argument in &invocation.arguments {
        words.extend(value_words(&argument.value));
    }
    words
}

/// The actor a command source is, with Java's two facts: a player or not, and
/// permission level 4 (`IMCCommand.java:16`, `:87-114`).
pub fn actor(source: &CommandSource) -> Actor {
    if let CommandSource::Player { player, permission } = source {
        return match permission {
            PermissionLevel::Owners => Actor::operator(player.uuid()),
            _ => Actor::player(player.uuid()),
        };
    }
    if source.permission() == PermissionLevel::Owners {
        Actor::console()
    } else {
        Actor {
            player: None,
            op: false,
        }
    }
}

/// The words one argument is, or none when the kind names no word the tree
/// declares — `parse` then says which word is missing, which is better than a
/// word this module made up.
fn value_words(value: &CommandArgumentValue) -> Vec<String> {
    match value {
        CommandArgumentValue::String(word) => vec![word.clone()],
        CommandArgumentValue::Integer(number) => vec![number.to_string()],
        CommandArgumentValue::Boolean(flag) => vec![flag.to_string()],
        CommandArgumentValue::BlockPos(at) => {
            vec![at[0].to_string(), at[1].to_string(), at[2].to_string()]
        }
        CommandArgumentValue::Custom { value, .. } => vec![value.clone()],
        CommandArgumentValue::Player(player) => vec![uuid::to_text(player.uuid())],
        _ => Vec::new(),
    }
}

/// The record a block entity holds, when it holds one: a colony whose `id` is
/// `0` is a record that was deleted, and reads as no colony at all.
fn record_in(data: &Compound) -> Option<Colony> {
    let Some(Tag::Compound(inner)) = data.get(&KEY_RECORD.into()) else {
        return None;
    };
    let colony = Colony::from_compound(inner);
    (colony.id > 0).then_some(colony)
}

/// The record at `at`, when a block entity stands there and holds one.
fn record_at(host: &dyn Host, level: LevelRef, at: [i32; 3]) -> Option<Colony> {
    let entity = host.block_entity(level, at)?;
    let data = entity.data(host)?;
    record_in(&data)
}

/// The colony a block entity at `pos` leads to. Two shapes, one question: the
/// main building holds the record itself; a hut holds the link and the record is
/// one hop away.
fn colony_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<(Colony, [i32; 3])> {
    let entity = host.block_entity(level, pos)?;
    let data = entity.data(host)?;
    if let Some(colony) = record_in(&data) {
        return Some((colony, pos));
    }
    colony_of(host, level, &hut::state_of(&data))
}

/// **The stand-in for `getIColony(level, pos)`.** The port has no claim map, so
/// `/colony` reads the colony whose block entity the command source stands in,
/// or the one it stands on — a player's feet are inside a block, and the block
/// they stand on is the one under them. `/execute positioned <pos> run colony …`
/// names any other.
fn anchor_at(host: &dyn Host, level: LevelRef, at: [i32; 3]) -> Option<(Colony, [i32; 3])> {
    for pos in [at, [at[0], at[1] - 1, at[2]]] {
        if let Some(found) = colony_at(host, level, pos) {
            return Some(found);
        }
    }
    None
}

/// The level a command runs in: the acting player's, else the one the invocation
/// names. Both are answers the host gives; nothing here builds a `LevelRef`.
fn level_of(host: &dyn ServerHost, key: DimensionKey) -> Option<LevelRef> {
    let playing = host.players().acting_player();
    let reads: &dyn Host = host;
    if let Some(player) = playing {
        if let Some(level) = player.level(reads) {
            return Some(level);
        }
    }
    host.levels()
        .list_levels()
        .into_iter()
        .find(|level| level.dimension() == key)
}

/// The block a position is in, floored: `invocation.position` is the source's
/// exact position, and a negative `-0.5` must be block `-1`, not block `0`.
fn block_of(position: [f64; 3]) -> [i32; 3] {
    [
        position[0].floor() as i32,
        position[1].floor() as i32,
        position[2].floor() as i32,
    ]
}

/// The centre of a block, which is where a teleport lands.
fn centre(at: [i32; 3]) -> [f64; 3] {
    [
        f64::from(at[0]) + 0.5,
        f64::from(at[1]),
        f64::from(at[2]) + 0.5,
    ]
}

/// Whether the command writes the record: the three subcommands that change it
/// (`add`, `remove`, `rename`), and not the ones that only read it. `delete` is
/// [`Effect::Delete`], which is not a merge.
fn writes_the_record(words: &[&str]) -> bool {
    matches!(
        commands::parse(words),
        Ok(ColonyCommand::Add { .. }
            | ColonyCommand::Remove { .. }
            | ColonyCommand::Rename { .. })
    )
}

/// What the world said: the fleet the command runs on, and where its record is.
struct World {
    /// The colonies a command can reach. One, because the port has no index
    /// (module doc, *No index*).
    fleet: Fleet,
    /// The colony the world could name: the level it stands in, and the
    /// position of the block entity that holds its record.
    anchor: Option<(LevelRef, [i32; 3])>,
}

impl World {
    /// Reads the world: the level the command runs in, the colony its source
    /// stands on, and the one answer `@here` has — plus the one player name the
    /// words may carry.
    ///
    /// `commands::run` resolves `add`/`remove`'s `playername` through the fleet,
    /// as Java does through `GameProfileCache` (`CommandAddOfficer.java:41-51`),
    /// and the port has no cache: the host does. Only the name the command
    /// actually carries is looked up, so a command that names no player costs no
    /// host call, and a name nobody answers to stays missing —
    /// `Refusal::UnknownPlayer` is then the rule that fires, not a guess.
    fn read(host: &dyn ServerHost, invocation: &CommandInvocation) -> World {
        let Some(level) = level_of(host, invocation.dimension) else {
            return World {
                fleet: Fleet::new(Vec::new()),
                anchor: None,
            };
        };
        let at = block_of(invocation.position);
        let reads: &dyn Host = host;
        let Some((colony, main)) = anchor_at(reads, level, at) else {
            return World {
                fleet: Fleet::new(Vec::new()),
                anchor: None,
            };
        };
        let id = colony.id;
        let mut fleet = Fleet::new(vec![colony]).with_here(at, id);
        let words = words(invocation);
        let asked: Vec<&str> = words.iter().map(String::as_str).collect();
        if let Ok(ColonyCommand::Add { player, .. } | ColonyCommand::Remove { player, .. }) =
            commands::parse(&asked)
        {
            if let Some(found) = host.players().player_by_name(&player) {
                fleet = fleet.with_name(found.uuid(), &player);
            }
        }
        World {
            fleet,
            anchor: Some((level, main)),
        }
    }

    /// Writes the record back, and performs the two world actions a colony
    /// command asks for. Nothing is written when the world named no colony: the
    /// command then never reached the record (it was refused), and a failed
    /// answer is not a reason to touch the world.
    fn apply(
        &self,
        host: &mut dyn ServerHost,
        effect: &Effect,
        words: &[&str],
        source: &CommandSource,
    ) {
        let Some((level, main)) = self.anchor else {
            return;
        };
        let Some(colony) = self.fleet.iter().next() else {
            return;
        };
        match effect {
            Effect::None => {
                if writes_the_record(words) {
                    let _ = write_colony(&mut *host, level, main, colony);
                }
            }
            // `TeleportHelper.colonyTeleport` (`core/util/TeleportHelper.java:140-195`):
            // the entity keeps its motion, and the call is `Entity.teleport`, not
            // a move — a player is told by the game, not by this mod.
            Effect::Teleport { dimension, at, .. } => {
                let Some(player) = player_of(source) else {
                    return;
                };
                player
                    .entity()
                    .teleport_to_level(&mut *host, dimension, centre(*at), None);
            }
            // The record goes, and, when the player said so, the huts with it
            // (`CommandDeleteColony.java:172-175`). The block entity stays: a
            // merge cannot remove a key, so the record is written empty and
            // `record_in` reads an id of `0` as no colony.
            Effect::Delete { buildings, .. } => {
                if *buildings {
                    for building in &colony.buildings {
                        level.remove_block(&mut *host, building.at, false);
                    }
                }
                let empty = Colony::from_compound(&Compound::new());
                let _ = write_colony(&mut *host, level, main, &empty);
            }
        }
    }
}

/// The player a source is, when it is one: the console and a command block have
/// no entity to move.
fn player_of(source: &CommandSource) -> Option<mod_api::PlayerRef> {
    match source {
        CommandSource::Player { player, .. } => Some(*player),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{Building, Citizen};
    use crate::commands::reply;
    use crate::harness::FakeHost;
    use mod_api::CommandArgument;

    const OWNER: u128 = 7;
    const HUT: [i32; 3] = [4, 64, -2];
    const MAIN: [i32; 3] = [10, 64, 10];
    const ID: i32 = 7;

    /// `Bridgewater`, id 7, one citizen and one hut.
    fn colony() -> Colony {
        let mut colony = Colony::new(ID, "Bridgewater", "minecraft:overworld", MAIN, OWNER);
        colony.citizens.push(Citizen::new(1, "Ada"));
        colony.buildings.push(Building::new(hut::BLOCK_ID, HUT, 1));
        colony
    }

    /// A world holding the record at `MAIN` and the link at `HUT`.
    fn world() -> FakeHost {
        let mut host = FakeHost::new();
        host.level.put_block(MAIN, hut::BLOCK_ID, None);
        host.level.put_data(
            MAIN,
            Compound::from([(
                KEY_RECORD.into(),
                Tag::Compound(colony().to_compound()),
            )]),
        );
        host.level.put_block(HUT, hut::BLOCK_ID, None);
        host.level.put_data(HUT, hut::initial_data());
        let link = hut::with_colony(ID, MAIN);
        assert!(host.level.merge_at(HUT, link));
        host
    }

    fn text(answer: &Text) -> String {
        reply::plain(answer)
    }

    #[test]
    fn the_words_of_an_invocation_are_the_ones_the_parser_reads() {
        let info = CommandInvocation::new(
            vec![ROOT.to_owned(), "info".to_owned()],
            vec![CommandArgument::new(
                "colonyID".to_owned(),
                CommandArgumentValue::String("7".to_owned()),
            )],
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        );
        assert!(is_colony(&info));
        assert_eq!(words(&info), ["info", "7"]);

        // `here`: a block position is three words, and it is the one usage that
        // takes one.
        let here = CommandInvocation::new(
            vec![ROOT.to_owned(), "here".to_owned()],
            vec![CommandArgument::new(
                "pos".to_owned(),
                CommandArgumentValue::BlockPos([-3, 71, 1208]),
            )],
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        );
        assert_eq!(words(&here), ["here", "-3", "71", "1208"]);
        assert_eq!(
            commands::parse(&["here", "-3", "71", "1208"])
                .map(|command| command.subcommand().name()),
            Ok("here")
        );

        // `delete` keeps the order the dispatcher delivered: the colony, then the
        // two booleans (`CommandDeleteColony.java:44`, `:49`).
        let delete = CommandInvocation::new(
            vec![ROOT.to_owned(), "delete".to_owned()],
            vec![
                CommandArgument::new(
                    "colonyID".to_owned(),
                    CommandArgumentValue::Integer(7),
                ),
                CommandArgument::new(
                    "buildings".to_owned(),
                    CommandArgumentValue::Boolean(true),
                ),
                CommandArgument::new(
                    "confirm".to_owned(),
                    CommandArgumentValue::Boolean(false),
                ),
            ],
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        );
        assert_eq!(words(&delete), ["delete", "7", "true", "false"]);

        // The root alone is this tree's, and says so.
        let bare = CommandInvocation::new(
            vec![ROOT.to_owned()],
            Vec::new(),
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        );
        assert!(words(&bare).is_empty());
        assert!(is_colony(&bare));

        // Another root is not, and is left to whoever registered it.
        let foreign = CommandInvocation::new(
            vec!["othermod".to_owned(), "colony".to_owned()],
            Vec::new(),
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        );
        assert!(!is_colony(&foreign));
    }

    #[test]
    fn the_actor_is_the_two_facts_java_reads() {
        let owner = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(mod_api::WorldEpoch::new(1), 100),
            OWNER,
        );
        assert_eq!(
            actor(&CommandSource::Player {
                player: owner,
                permission: PermissionLevel::Owners
            }),
            Actor::operator(OWNER)
        );
        assert_eq!(
            actor(&CommandSource::Player {
                player: owner,
                permission: PermissionLevel::All
            }),
            Actor::player(OWNER)
        );
        assert_eq!(
            actor(&CommandSource::Console {
                permission: PermissionLevel::Owners
            }),
            Actor::console()
        );
        assert_eq!(
            actor(&CommandSource::Console {
                permission: PermissionLevel::All
            }),
            Actor {
                player: None,
                op: false
            },
            "the console at level 0 is nobody"
        );
    }

    #[test]
    fn the_next_citizen_id_is_the_highest_saved_one_plus_one() {
        let mut colony = colony();
        assert_eq!(next_citizen_id(&colony), 2, "Ada is 1");
        colony.citizens.clear();
        assert_eq!(next_citizen_id(&colony), 1, "an empty colony starts at 1");
        colony.citizens.push(Citizen::new(9, "Ada"));
        colony.citizens.push(Citizen::new(4, "Bram"));
        assert_eq!(next_citizen_id(&colony), 10, "the highest, not the last");
    }

    #[test]
    fn a_hut_link_reads_the_record_of_its_main_building() {
        let host = world();
        let level = host.level.level();
        let state = hut::state_of(host.level.data_at(HUT).unwrap());
        assert_eq!(state.colony, Some(ID));
        assert_eq!(state.main, Some(MAIN));

        let (read, main) = colony_of(&host, level, &state).expect("the link is followed");
        assert_eq!(main, MAIN);
        assert_eq!(read.id, ID);
        assert_eq!(read.name, "Bridgewater");
        assert_eq!(read.citizens.len(), 1);
        assert_eq!(read.center, MAIN);
    }

    #[test]
    fn a_hut_without_a_link_and_a_link_that_lies_read_as_no_colony() {
        let host = world();
        let level = host.level.level();

        // A hut placed before the link existed: five keys, no colony.
        let unlinked = hut::state_of(&hut::initial_data());
        assert_eq!(colony_of(&host, level, &unlinked), None);

        // The record's id is not the id the hut stores: the main building was
        // replaced, and the port does not follow a link it cannot trust.
        let mut lying = unlinked.clone();
        lying.colony = Some(ID + 1);
        lying.main = Some(MAIN);
        assert_eq!(colony_of(&host, level, &lying), None);

        // The link is there, the main building is not.
        let mut broken = lying;
        broken.colony = Some(ID);
        broken.main = Some([99, 99, 99]);
        assert_eq!(colony_of(&host, level, &broken), None);
    }

    #[test]
    fn writing_a_record_makes_it_readable_and_a_deleted_one_reads_as_none() {
        let mut host = world();
        let level = host.level.level();

        let mut colony = colony();
        colony.citizens.push(Citizen::new(2, "Bram"));
        write_colony(&mut host, level, MAIN, &colony).expect("the record holder stands");
        let read = record_at(&host, level, MAIN).expect("it reads back");
        assert_eq!(read, colony);

        // The block entity's other keys survive a merge of one.
        assert_eq!(host.level.data_at(MAIN).unwrap().len(), 1, "one key, replaced");

        // Deleting is writing an empty record: a merge cannot remove a key.
        let empty = Colony::from_compound(&Compound::new());
        write_colony(&mut host, level, MAIN, &empty).expect("written");
        assert_eq!(record_at(&host, level, MAIN), None, "id 0 is no colony");

        // And nothing stands here: the refusal is named, not a bare false.
        assert_eq!(
            write_colony(&mut host, level, [0, 0, 0], &colony),
            Err(WriteRefusal::NoRecordHolder)
        );
    }

    #[test]
    fn a_colony_command_answers_from_the_record_under_the_source() {
        let mut host = world();
        // The source stands in the main building's block. The console runs at
        // level 4, which is what a stock server gives `here`: its switch
        // (`canPlayerUseHomeTPCommand`) is off by default (`Config::default`).
        let invocation = CommandInvocation::new(
            vec![ROOT.to_owned(), "here".to_owned()],
            Vec::new(),
            DimensionKey::OVERWORLD,
            [
                f64::from(MAIN[0]) + 0.5,
                f64::from(MAIN[1]),
                f64::from(MAIN[2]) + 0.5,
            ],
        );
        let source = CommandSource::Console {
            permission: PermissionLevel::Owners,
        };
        let mut event = RunModCommandEvent {
            source: &source,
            command: &invocation,
            cancelled: false,
        };
        // `here` teleports, and the fake host has no teleport: the console has no
        // entity to move, so the answer is all this test reads.
        assert_eq!(run(&mut host, &mut event), Flow::Cancel);
        let told: Vec<String> = host
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                crate::harness::Call::Feedback(line) => Some(text(&line)),
                _ => None,
            })
            .collect();
        assert_eq!(told, ["Teleported to Bridgewater"]);
    }

    #[test]
    fn a_word_the_tree_does_not_know_is_refused_by_the_parser_and_nothing_is_written() {
        let mut host = world();
        let invocation = CommandInvocation::new(
            vec![ROOT.to_owned(), "destroy".to_owned()],
            Vec::new(),
            DimensionKey::OVERWORLD,
            [
                f64::from(MAIN[0]) + 0.5,
                f64::from(MAIN[1]),
                f64::from(MAIN[2]) + 0.5,
            ],
        );
        let source = CommandSource::Console {
            permission: PermissionLevel::Owners,
        };
        let mut event = RunModCommandEvent {
            source: &source,
            command: &invocation,
            cancelled: false,
        };
        assert_eq!(run(&mut host, &mut event), Flow::Cancel);
        let told: Vec<String> = host
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                crate::harness::Call::Feedback(line) => Some(text(&line)),
                _ => None,
            })
            .collect();
        assert_eq!(told[0], "Unknown colony subcommand: destroy");
        assert!(
            !host.tags().contains(&"block_entity_merge_data"),
            "a refused command writes nothing"
        );
    }

    #[test]
    fn the_summary_is_the_status_line_then_the_info_lines() {
        let status = Text::translate("block.minecolonies.blockhutbuilder.status", Vec::new());
        let shown = summary(status, &colony());
        assert_eq!(
            shown.children.iter().map(text).collect::<Vec<String>>(),
            [
                SEPARATOR.to_owned(),
                format!("ID: {ID} Name: Bridgewater"),
                SEPARATOR.to_owned(),
                format!("Mayor: {}", uuid::to_text(OWNER)),
                SEPARATOR.to_owned(),
                "Citizens: 1".to_owned(),
                SEPARATOR.to_owned(),
                format!("Coordinates: x={} y={} z={}", MAIN[0], MAIN[1], MAIN[2]),
                SEPARATOR.to_owned(),
                "Members:".to_owned(),
                SEPARATOR.to_owned(),
                format!("Owner: {}", uuid::to_text(OWNER)),
            ],
            "the same lines `colony info` answers with, joined on one action-bar line"
        );
    }
}
