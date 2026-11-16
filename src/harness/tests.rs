//! The harness on the port's own code: the hut, the command, and the fake's rules.

use mod_api::prelude::{BlockEntitiesBackendMut, LevelsBackendMut, ServicesServerHost};
use mod_api::text::TextContent;
use mod_api::{BlockEntityError, Compound, DimensionKey, Flow, LevelRef, SpawnReason, Tag, Text,
    WorldEpoch};

use super::*;
use crate::{citizen, hire, hut, menu};

/// Where the hut stands, in every test.
const HUT: [i32; 3] = [4, 64, -2];

/// The key and the arguments of a line the port showed, as plain text.
fn line(text: &Text) -> (&str, Vec<String>) {
    let TextContent::Translatable { key, arguments, .. } = &text.content else {
        panic!("not a translation: {text:?}");
    };
    let plain = arguments
        .iter()
        .map(|argument| match &argument.content {
            TextContent::Literal(value) => value.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    (key.as_str(), plain)
}

#[test]
fn the_placed_hut_writes_its_compound_and_the_click_reads_it_back() {
    let mut world = Scenario::place(HUT, "east").with_player(7);

    // The Part's answer is what the block entity now holds: five keys written by the
    // port (`hut::initial_data`), not values the reader's defaults would give.
    let data = world.data();
    assert_eq!(data.len(), 5, "style, mirror, blueprint, level, hired");
    assert_eq!(
        data.get(&hut::KEY_STYLE.into()),
        Some(&Tag::String(hut::STYLE_DEFAULT.into()))
    );
    assert_eq!(
        data.get(&hut::KEY_LEVEL.into()),
        Some(&Tag::Int(hut::LEVEL_MIN))
    );

    // A level written by hand, as `/data merge block` does, so the click has
    // something to read that the defaults would not give.
    assert!(world
        .host
        .level
        .merge_at(HUT, Compound::from([(hut::KEY_LEVEL.into(), Tag::Int(3))])));

    assert_eq!(world.click(), Flow::Continue);
    assert_eq!(
        world.host.tags(),
        [
            "acting_player", "player_level", "block_key", "level_block", "block_entity",
            "block_entity_data", "action_bar",
        ],
        "the click reads the block, then the compound, then shows the line"
    );

    let shown = match world.host.calls().pop() {
        Some(Call::ActionBar(text)) => text,
        other => panic!("the click said nothing: {other:?}"),
    };
    assert_eq!(
        line(&shown),
        (menu::STATUS_KEY, vec!["3".to_owned(), "0/3".to_owned()]),
        "level, then hired/capacity"
    );

    // The dimension is the fake level's, and the host answers it.
    assert_eq!(world.host.level.dimension(), "minecraft:overworld");
    assert_eq!(world.host.levels().list_levels(), vec![world.host.level.level()]);
}

#[test]
fn the_hire_spawns_a_citizen_walks_it_and_merges_the_count_back() {
    let mut world = Scenario::place(HUT, "north").with_player(7);

    // The command tree first: the usage is the one the port declares.
    let commands = world.register();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].usage, hire::USAGE);

    assert_eq!(world.hire(), Flow::Cancel, "the port answered the command");
    assert_eq!(
        world.host.tags(),
        [
            "acting_player", "player_level", "block_key", "level_block", "block_entity",
            "block_entity_data", "level_block_property", "spawn_mob",
            // `spawn_mob` fires `entity_join_level` inside the spawn
            // (`crates/horizon-mod-vocab/src/host/levels.rs:793-816`): the citizen is
            // disarmed and named before the hire orders its walk.
            "entity", "mob", "mob_goals", "remove_goal", "remove_goal", "remove_goal",
            "remove_goal", "set_persistence_required", "set_custom_name",
            "mob", "move_to", "block_entity_merge_data", "feedback",
        ]
    );

    let (mut spawned, mut walked, mut merged) = (None, None, None);
    for call in world.host.calls() {
        match call {
            Call::Spawn { kind, at } => spawned = Some((kind, at)),
            Call::MoveTo { at, speed } => walked = Some((at, speed)),
            Call::Merge { data, .. } => merged = Some(data),
            _ => {}
        }
    }

    let (kind, at) = spawned.expect("a mob was spawned");
    assert_eq!(kind, citizen::KIND);
    assert_eq!(at, hut::spawn_at(HUT, "north"), "one block in front, centred");
    assert_eq!(
        walked,
        Some((hut::walk_to(HUT, "north"), citizen::SPEED)),
        "four blocks out, at the citizen's own speed"
    );

    // The merge payload carries one key, so the rest of the compound survives — and
    // the count in the level is the one the hire worked out.
    let merged = merged.expect("the hire wrote the count back");
    assert_eq!(merged.len(), 1, "merged, not replaced");
    assert_eq!(merged.get(&hut::KEY_HIRED.into()), Some(&Tag::Int(1)));
    let state = world.state();
    assert_eq!(state.hired, 1);
    assert_eq!(state.style, hut::STYLE_DEFAULT, "the other keys are untouched");

    // The citizen itself: no goal left, persistent, and named.
    let mob = world.host.level.mob(1).expect("the citizen is in the level");
    assert!(mob.goals.is_empty(), "every goal kind was removed");
    assert!(mob.persistent);
    assert_eq!(
        mob.name.as_ref().map(line).map(|(key, _)| key.to_owned()),
        Some(citizen::NAME_KEY.to_owned())
    );

    // And the player was told how many the hut holds.
    let told = world
        .host
        .calls()
        .into_iter()
        .find_map(|call| match call {
            Call::Feedback(text) => Some(text),
            _ => None,
        })
        .expect("the command answered");
    assert_eq!(line(&told), (hire::STATUS_KEY, vec!["1".to_owned()]));
}

#[test]
fn a_handle_this_world_never_minted_is_stale_and_a_spawn_elsewhere_is_refused() {
    let mut world = Scenario::place(HUT, "north");
    // Another open of the same level: a different handle, so a different world.
    let elsewhere = LevelRef::with_generation(WorldEpoch::new(EPOCH), DimensionKey::OVERWORLD, 1);
    let handle = mod_api::__private::game::block_entity_ref(elsewhere, HUT, 0);

    assert_eq!(
        world.host.block_entity_merge_data(handle, hut::with_hired(1)),
        Err(BlockEntityError::Stale),
        "a merge never lands in another world's block entity"
    );
    assert!(world.host.level.data_at(HUT).is_some(), "the hut is still there");
    assert_eq!(world.state().hired, 0);

    assert_eq!(
        world
            .host
            .level_spawn_mob(elsewhere, citizen::KIND, [0.0, 64.0, 0.0], SpawnReason::Command),
        None
    );
    assert!(
        world.host.level.mob(1).is_none(),
        "nothing spawned in a level this host does not have"
    );
}
