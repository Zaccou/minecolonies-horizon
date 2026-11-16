//! The click on a hut: what it tells the player, and where the menu comes from.
//!
//! **The menu is not opened here, and that is deliberate.** The hut's template
//! is `minecraft:crafting_table`, whose class is in the closed list
//! `BlockBehaviour.useWithoutItem` opens a menu for
//! (`crates/horizon-world/src/level/interact.rs:441-466`); the level event it
//! emits reaches `open_mod_menu`
//! (`crates/horizon-client/src/game.rs:3643`, `:3954`), which sizes the block
//! entity's slots and sets the container
//! (`crates/horizon-client/src/game/native_hooks/shared.rs:2916-2930`). Every
//! click that answered `Success` from this hook would skip that path and open
//! nothing: a cancelled `right_click_block` ends the click instead of the
//! block's use (`crates/horizon-mod-vocab/src/hooks/right_click_block.rs:5-20`).
//!
//! What this hook adds is the read side: the click says what the hut holds, so a
//! player who placed one can see the compound is there without a command, and
//! the port proves on the server that `block_entity` + `_data` answer.
//!
//! **A hut that belongs to a colony says what the colony is.** The click reads
//! the hut's link, follows it to the main building, and appends the lines
//! `/colony info` answers with (`crate::commands::reply`) to the status line —
//! one action-bar message, because that is the one text channel a right click
//! has (`PlayerRef::action_bar`). A hut with no link shows the status line alone,
//! with no extra host call.

use crate::{hut, wiring};
use mod_api::{Flow, HostSide, RightClickBlockEvent, Text};

/// The line a click on a hut shows, from `assets/minecolonies/lang/`.
pub const STATUS_KEY: &str = "block.minecolonies.blockhutbuilder.status";

/// `right_click_block`: fires on the clicking client and on the server
/// (`crates/horizon-mod-vocab/src/hooks/right_click_block.rs:7-8`). The read and
/// the message are server-side; the client half answers `Continue` so its
/// prediction stays free to send the click on.
pub fn right_click_block(host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
    let HostSide::Server(host) = host else {
        return Flow::Continue;
    };
    // A held stack means the item gets the click first, as in vanilla; only an
    // empty hand is this port's to answer.
    if event.click.stack.is_some() {
        return Flow::Continue;
    }
    let Some(player) = host.players().acting_player() else {
        return Flow::Continue;
    };
    let Some(level) = player.level(&*host) else {
        return Flow::Continue;
    };
    let Some(hut_block) = host.block_key(hut::BLOCK_ID) else {
        return Flow::Continue;
    };
    if level.block(&*host, event.click.pos) != Some(hut_block) {
        return Flow::Continue;
    }
    let Some(entity) = host.block_entity(level, event.click.pos) else {
        return Flow::Continue;
    };
    let data = entity.data(&*host).unwrap_or_default();
    let state = hut::state_of(&data);
    let status = Text::translate(
        STATUS_KEY,
        vec![
            Text::literal(state.level.to_string()),
            Text::literal(format!("{}/{}", state.hired, hut::capacity(state.level))),
        ],
    );
    // The colony's summary replaces no line: it is appended to the status the
    // hut already answered, so a broken link still reads as a hut with a level.
    let line = match wiring::colony_of(&*host, level, &state) {
        Some((colony, _)) => wiring::summary(status, &colony),
        None => status,
    };
    let _ = player.action_bar(&mut *host, line);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Colony;
    use crate::commands::reply;
    use crate::harness::{Call, Scenario};
    use crate::wiring::KEY_RECORD;
    use mod_api::text::TextContent;
    use mod_api::{Compound, Tag};

    const OWNER: u128 = 7;
    const COLONY: i32 = 7;
    const HUT: [i32; 3] = [4, 64, -2];
    const MAIN: [i32; 3] = [10, 64, 10];

    #[test]
    fn a_linked_hut_shows_the_colony_summary_after_the_status_line() {
        let mut world = Scenario::place(HUT, "north").with_player(OWNER);
        let colony = Colony::new(COLONY, "Bridgewater", "minecraft:overworld", MAIN, OWNER);
        world.host.level.put_block(MAIN, hut::BLOCK_ID, None);
        world.host.level.put_data(
            MAIN,
            Compound::from([(KEY_RECORD.into(), Tag::Compound(colony.to_compound()))]),
        );
        assert!(world
            .host
            .level
            .merge_at(HUT, hut::with_colony(COLONY, MAIN)));

        assert_eq!(world.click(), Flow::Continue, "the engine's menu still opens");
        let Some(Call::ActionBar(shown)) = world.host.calls().pop() else {
            panic!("the click said nothing");
        };
        let TextContent::Translatable { key, arguments, .. } = &shown.content else {
            panic!("the status line is not a translation: {shown:?}");
        };
        assert_eq!(key.as_str(), STATUS_KEY, "the status line stays the root");
        assert_eq!(arguments.len(), 2, "level, then hired/capacity");

        let joined: String = shown.children.iter().map(reply::plain).collect();
        assert!(
            joined.contains("ID: 7 Name: Bridgewater"),
            "the colony's own header: {joined}"
        );
        assert!(joined.contains("Citizens: 0"), "{joined}");
        assert!(
            joined.contains(&format!(
                "Coordinates: x={} y={} z={}",
                MAIN[0], MAIN[1], MAIN[2]
            )),
            "{joined}"
        );
    }

    #[test]
    fn a_hut_that_names_a_colony_that_is_gone_still_answers_its_status() {
        let mut world = Scenario::place(HUT, "north").with_player(OWNER);
        assert!(world
            .host
            .level
            .merge_at(HUT, hut::with_colony(COLONY, MAIN)));

        assert_eq!(world.click(), Flow::Continue);
        let Some(Call::ActionBar(shown)) = world.host.calls().pop() else {
            panic!("the click said nothing");
        };
        assert!(
            shown.children.is_empty(),
            "a link that leads nowhere adds no summary"
        );
    }
}
