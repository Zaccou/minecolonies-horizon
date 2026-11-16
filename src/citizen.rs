//! The citizen: a `minecolonies:citizen` entity is a `minecraft:wandering_trader`
//! wearing this mod's type, and the moment it joins a level it stops being one.
//!
//! Removing the goals is the whole trick: `NoAI` would freeze it and the
//! `aiStep` pause skips its navigation too, so the only switch left is
//! `removeGoal`, one kind at a time
//! (`crates/horizon-mod-vocab/src/host/mobs.rs:278-287`, `:402-412`). The list
//! is read from the mob instead of written here, so a template whose goals
//! change does not break the port.

use mod_api::{EntityJoinLevelEvent, Flow, MobGoalEntry, ServerHost, Text};

/// The citizen's entity type, declared by `data/minecolonies/horizon/entity_type/citizen.json`.
pub const KIND: &str = "minecolonies:citizen";
/// `PathNavigation` speed modifier for the one walk a hire orders: the
/// wandering trader's own movement speed is 0.25 blocks/tick, and 0.6 reads as
/// an ordinary villager's walk, not a sprint.
pub const SPEED: f64 = 0.6;
/// The name a citizen wears, from `assets/minecolonies/lang/`.
pub const NAME_KEY: &str = "entity.minecolonies.citizen";

/// `EntityJoinLevelEvent`: this fires for **every** entity that joins a level
/// (items, orbs, mobs, boats, projectiles, frames...), so the kind is checked
/// before anything else is asked of the host
/// (`crates/horizon-mod-vocab/src/hooks/entity_join_level.rs:27-30`).
pub fn join(host: &mut dyn ServerHost, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
    if event.kind != KIND {
        return Flow::Continue;
    }
    let Some(entity) = host.entity(event.entity) else {
        return Flow::Continue;
    };
    let Some(mob) = host.mob(entity) else {
        return Flow::Continue;
    };
    if let Some(goals) = mob.goals(&*host) {
        for id in goal_ids(&goals) {
            mob.remove_goal(&mut *host, id);
        }
    }
    mob.set_persistence_required(&mut *host);
    let _ = entity.set_custom_name(&mut *host, Some(Text::translate(NAME_KEY, Vec::new())));
    Flow::Continue
}

/// The goal kinds to remove, each once, in the order the selector lists them.
///
/// Vanilla registers the same goal kind at two priorities for some species, and
/// `remove_goal` is `removeAllGoals(<kind>)`: asking twice is wasted work on a
/// mob that has just entered the level.
pub fn goal_ids(goals: &[MobGoalEntry]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for goal in goals {
        if !out.contains(&goal.id) {
            out.push(goal.id);
        }
    }
    out
}
