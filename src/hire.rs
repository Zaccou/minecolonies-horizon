//! `/mcproto hire <pos>`: the lever that needs no client.
//!
//! A hut's menu has no button the mod can declare (a `menu_button` has no
//! content declaration to trigger it, `TRANCHE.md` §3 M1), so the first citizen
//! is hired by a command. Everything the command does is a host method that
//! already exists: `block_key`, `block_entity` + `_data`, `LevelRef::spawn_mob`,
//! `MobRef::move_to`, `BlockEntityRef::merge_data`.
//!
//! **The count is the colony's when the hut knows its colony.** A hut's `hired`
//! key is a mirror written by this command; the citizens themselves are
//! `colony::Colony::citizens`, so a hire through a linked hut appends a
//! [`Citizen`] to the record and the room left is the colony's, not a second
//! counter that could drift. A hut placed before the link existed has no colony
//! to append to: it keeps the older path, counting in its own compound, because
//! **nothing in this slice links a hut** (no town hall block, no `/colony
//! create`) and refusing would leave such a hut with no way to hire at all. A
//! hut that *names* a colony whose record is unreachable is the broken case, and
//! it refuses: [`Refusal::NoColonyRecord`].

use crate::colony::Citizen;
use crate::{citizen, hut, wiring};
use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, Flow, RegisterCommandsEvent,
    RunModCommandEvent, ServerHost, SpawnReason, Text,
};

/// The command's root word.
pub const ROOT: &str = "mcproto";
/// The one usage this port adds.
pub const USAGE: &str = "mcproto hire <pos:block_pos>";
/// The argument that names the hut.
pub const POS: &str = "pos";
/// The block state property the citizen walks away along.
pub const FACING: &str = "facing";
pub const FACING_DEFAULT: &str = "north";

/// Why a hire did nothing. Each variant is a `lang` key, never an English
/// string in the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    NoPlayer,
    NotAHut,
    NoHutData,
    Full,
    NoRoomForCitizen,
    NotACitizen,
    WriteFailed,
    /// The hut names a colony whose record cannot be read: the main building is
    /// gone, or what stands there holds no colony. The link is broken, so the
    /// hire cannot know how many citizens the colony holds.
    NoColonyRecord,
}

impl Refusal {
    pub fn key(self) -> &'static str {
        match self {
            Self::NoPlayer => "mcproto.error.no_player",
            Self::NotAHut => "mcproto.error.not_a_hut",
            Self::NoHutData => "mcproto.error.no_hut_data",
            Self::Full => "mcproto.error.full",
            Self::NoRoomForCitizen => "mcproto.error.no_room",
            Self::NotACitizen => "mcproto.error.not_a_mob",
            Self::WriteFailed => "mcproto.error.write",
            Self::NoColonyRecord => "mcproto.error.no_colony_record",
        }
    }
}

/// `register_commands`: the server's tree is asked once when a world opens to
/// players, and again after a mod asks for a refresh
/// (`crates/horizon-mod-vocab/src/hooks/register_commands.rs:8-19`).
pub fn register_commands(_host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
    event.commands.extend([Command::new(USAGE)]);
}

/// `run_mod_command`: `Flow::Cancel` means this port answered the command.
pub fn run(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    if !is_hire(event.command) {
        return Flow::Continue;
    }
    let Some(pos) = hut_position(event.command) else {
        feedback(host, Text::translate("mcproto.error.usage", vec![]));
        return Flow::Cancel;
    };
    match hire(host, pos) {
        Ok(hired) => feedback(
            host,
            Text::translate(STATUS_KEY, vec![Text::literal(hired.to_string())]),
        ),
        Err(refusal) => feedback(host, Text::translate(refusal.key(), vec![])),
    }
    Flow::Cancel
}

/// Success line of a hire, from `assets/minecolonies/lang/`.
pub const STATUS_KEY: &str = "mcproto.hired";

/// Whether `invocation` is this port's `mcproto hire ...`.
pub fn is_hire(invocation: &CommandInvocation) -> bool {
    matches!(
        (invocation.literals.first(), invocation.literals.get(1)),
        (Some(root), Some(word)) if root == ROOT && word == "hire"
    )
}

/// The hut the command names, from its `block_pos` argument. `~` is already
/// resolved against the source by the dispatcher
/// (`crates/horizon-mod-vocab/src/command/mod.rs:88-92`), so this is the block
/// the player meant.
pub fn hut_position(invocation: &CommandInvocation) -> Option<[i32; 3]> {
    match invocation.argument(POS) {
        Some(CommandArgumentValue::BlockPos(pos)) => Some(*pos),
        _ => None,
    }
}

/// Spawn one citizen in front of the hut at `pos`, order it to walk, and record
/// it in the hut's compound and in its colony. The new count comes back.
///
/// The order matters: `LevelRef::spawn_mob` fires `entity_join_level` **inside** the
/// spawn (`crates/horizon-mod-vocab/src/host/levels.rs:807-816`, the `LevelsBackendMut`
/// method the handle calls), so by the time it returns the citizen's own goals are
/// already gone and `move_to` has a path nothing will overwrite on the next tick.
fn hire(host: &mut dyn ServerHost, pos: [i32; 3]) -> Result<i32, Refusal> {
    let player = host
        .players()
        .acting_player()
        .ok_or(Refusal::NoPlayer)?;
    let level = player.level(&*host).ok_or(Refusal::NotAHut)?;
    let hut_block = host.block_key(hut::BLOCK_ID).ok_or(Refusal::NotAHut)?;
    if level.block(&*host, pos) != Some(hut_block) {
        return Err(Refusal::NotAHut);
    }
    let entity = host.block_entity(level, pos).ok_or(Refusal::NoHutData)?;
    let data = entity.data(&*host).ok_or(Refusal::NoHutData)?;
    let state = hut::state_of(&data);
    // The colony, when the hut names one. Read **before** the spawn, so a room
    // that is already full is a refusal and not a citizen to take back.
    let colony = wiring::colony_of(&*host, level, &state);
    if state.colony.is_some() && colony.is_none() {
        return Err(Refusal::NoColonyRecord);
    }
    let housed = match &colony {
        Some((colony, _)) => colony.citizens.len() as i32,
        None => state.hired,
    };
    if hut::capacity(state.level) - housed <= 0 {
        return Err(Refusal::Full);
    }
    let facing = level
        .block_property(&*host, pos, FACING)
        .unwrap_or_else(|| FACING_DEFAULT.to_owned());
    let spawned = level
        .spawn_mob(
            &mut *host,
            citizen::KIND,
            hut::spawn_at(pos, &facing),
            SpawnReason::Command,
        )
        .ok_or(Refusal::NoRoomForCitizen)?;
    let mob = host.mob(spawned).ok_or(Refusal::NotACitizen)?;
    let _ = mob.move_to(&mut *host, hut::walk_to(pos, &facing), citizen::SPEED);
    let hired = match colony {
        // The colony's own list is the count: the citizen is appended, the
        // record is written back where it was read, and the hut's `hired` key
        // keeps mirroring it for a reader that only has the hut.
        Some((mut colony, main)) => {
            let id = wiring::next_citizen_id(&colony);
            // No personal name exists in the port: Java draws one from a `lang`
            // list (`CitizenNameGenerator`), and this record states the type.
            colony.citizens.push(Citizen::new(id, citizen::KIND));
            wiring::write_colony(&mut *host, level, main, &colony)
                .map_err(|_| Refusal::WriteFailed)?;
            colony.citizens.len() as i32
        }
        None => state.hired + 1,
    };
    entity
        .merge_data(&mut *host, hut::with_hired(hired))
        .map_err(|_| Refusal::WriteFailed)?;
    Ok(hired)
}

fn feedback(host: &mut dyn ServerHost, line: Text) {
    let _ = host.commands().feedback(line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{Citizen, Colony};
    use crate::harness::{Call, Scenario};
    use crate::wiring::KEY_RECORD;
    use mod_api::text::TextContent;
    use mod_api::{Compound, Tag};

    const OWNER: u128 = 7;
    const COLONY: i32 = 7;
    const HUT: [i32; 3] = [4, 64, -2];
    const MAIN: [i32; 3] = [10, 64, 10];

    /// A hut of level 1 facing north, linked to a colony of `citizens` that
    /// already lives at `MAIN`. The record is written where `wiring` reads it,
    /// which is what `/data merge block` would do by hand.
    fn linked(citizens: i32) -> Scenario {
        let mut world = Scenario::place(HUT, "north").with_player(OWNER);
        let mut colony = Colony::new(COLONY, "Bridgewater", "minecraft:overworld", MAIN, OWNER);
        for id in 1..=citizens {
            colony.citizens.push(Citizen::new(id, "minecolonies:citizen"));
        }
        world.host.level.put_block(MAIN, hut::BLOCK_ID, None);
        world.host.level.put_data(
            MAIN,
            Compound::from([(KEY_RECORD.into(), Tag::Compound(colony.to_compound()))]),
        );
        assert!(world
            .host
            .level
            .merge_at(HUT, hut::with_colony(COLONY, MAIN)));
        world
    }

    /// The colony as its record holds it now.
    fn record_of(world: &Scenario) -> Colony {
        let data = world.host.level.data_at(MAIN).expect("the record stands there");
        let Some(Tag::Compound(inner)) = data.get(&KEY_RECORD.into()) else {
            panic!("no record under {KEY_RECORD}");
        };
        Colony::from_compound(inner)
    }

    /// The key of the one line the command answered with.
    fn said(world: &Scenario) -> Option<String> {
        world.host.calls().into_iter().find_map(|call| match call {
            Call::Feedback(line) => match &line.content {
                TextContent::Translatable { key, .. } => Some(key.clone()),
                _ => None,
            },
            _ => None,
        })
    }

    #[test]
    fn a_hire_through_a_linked_hut_appends_the_citizen_to_the_colony() {
        let mut world = linked(0);
        assert_eq!(world.hire(), Flow::Cancel);
        assert_eq!(said(&world).as_deref(), Some(STATUS_KEY));

        let colony = record_of(&world);
        assert_eq!(colony.citizens.len(), 1, "the count is the colony's");
        assert_eq!(colony.citizens[0].id, 1, "the first free id");
        assert_eq!(colony.citizens[0].name, citizen::KIND);
        assert_eq!(world.state().hired, 1, "the hut mirrors the colony");

        let merges = world
            .host
            .tags()
            .into_iter()
            .filter(|tag| *tag == "block_entity_merge_data")
            .count();
        assert_eq!(merges, 2, "the record at the main building, then the hut");
    }

    #[test]
    fn a_full_colony_refuses_the_hire_before_spawning_anyone() {
        // A level-1 hut houses one, and the colony already holds it.
        let mut world = linked(1);
        assert_eq!(world.hire(), Flow::Cancel);
        assert_eq!(said(&world).as_deref(), Some(Refusal::Full.key()));
        assert!(
            !world.host.tags().contains(&"spawn_mob"),
            "a full hut spawns no citizen to take back"
        );
    }

    #[test]
    fn a_hut_that_names_a_colony_that_is_gone_refuses_the_hire() {
        // The link is there; nothing stands where the record should be.
        let mut world = Scenario::place(HUT, "north").with_player(OWNER);
        assert!(world
            .host
            .level
            .merge_at(HUT, hut::with_colony(COLONY, MAIN)));
        assert_eq!(world.hire(), Flow::Cancel);
        assert_eq!(
            said(&world).as_deref(),
            Some(Refusal::NoColonyRecord.key()),
            "a broken link is a named refusal, not a hut with no room"
        );
        assert!(!world.host.tags().contains(&"spawn_mob"));
    }
}
