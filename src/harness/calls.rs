//! What the port asked the host for, in the order it asked.

use mod_api::{Compound, Text};

/// One host call the port made, with the arguments a test checks it against.
///
/// Closed on purpose: it names the eighteen calls this slice makes, so a test can
/// assert **the order** of a whole step and not only its result.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// `block_key(id)`.
    BlockKey(String),
    /// `block_entity(level, pos)`: a handle was asked for.
    BlockEntity([i32; 3]),
    /// `block_entity.data`: the saved compound was read.
    Data([i32; 3]),
    /// `block_entity.merge_data`, with the payload the port wrote.
    Merge { at: [i32; 3], data: Compound },
    /// `level.block(pos)`.
    Block([i32; 3]),
    /// `level.block_property(pos, key)`.
    Property { at: [i32; 3], key: String },
    /// `players().acting_player()`.
    ActingPlayer,
    /// `player.level(host)`.
    PlayerLevel,
    /// `level.spawn_mob(host, kind, at, reason)`.
    Spawn { kind: String, at: [f64; 3] },
    /// `host.entity(id)`.
    Entity(u32),
    /// `host.mob(entity)`.
    Mob(u32),
    /// `mob.goals(host)`.
    Goals(u32),
    /// `mob.remove_goal(host, id)`.
    RemoveGoal(String),
    /// `mob.set_persistence_required(host)`.
    Persist(u32),
    /// `entity.set_custom_name(host, name)`; `None` clears it.
    Name(Option<Text>),
    /// `mob.move_to(host, at, speed)`.
    MoveTo { at: [f64; 3], speed: f64 },
    /// `player.action_bar(host, line)`.
    ActionBar(Text),
    /// `commands().feedback(line)`.
    Feedback(Text),
}

impl Call {
    /// The host method's name, for an order assertion that ignores arguments.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::BlockKey(_) => "block_key",
            Self::BlockEntity(_) => "block_entity",
            Self::Data(_) => "block_entity_data",
            Self::Merge { .. } => "block_entity_merge_data",
            Self::Block(_) => "level_block",
            Self::Property { .. } => "level_block_property",
            Self::ActingPlayer => "acting_player",
            Self::PlayerLevel => "player_level",
            Self::Spawn { .. } => "spawn_mob",
            Self::Entity(_) => "entity",
            Self::Mob(_) => "mob",
            Self::Goals(_) => "mob_goals",
            Self::RemoveGoal(_) => "remove_goal",
            Self::Persist(_) => "set_persistence_required",
            Self::Name(_) => "set_custom_name",
            Self::MoveTo { .. } => "move_to",
            Self::ActionBar(_) => "action_bar",
            Self::Feedback(_) => "feedback",
        }
    }
}
