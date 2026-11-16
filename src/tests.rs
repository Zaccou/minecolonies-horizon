//! Unit tests on the host target: no wasm, no game, no prepared resources.
//!
//! **Why there is no `dispatch_*` test here.** The documented harness is
//! `mod_api::testing::FakeHost` plus the generated `dispatch_<hook>` functions
//! (`docs/modding/src/concepts/testing.md:3-13`), and every host call this slice
//! makes on the way back is one the fake does **not** model:
//! `crates/horizon-mod-vocab/src/testing.rs:373-388` builds `FakeHost` with
//! `impl_host_areas!(FakeHost, except(block_entities, block_states, ...))`, so
//! `block_entity_data`, `block_entity_merge_data`, `level_spawn_mob` and the
//! whole `mobs` area answer with `unimplemented!("<method>")`
//! (`crates/horizon-mod-vocab/src/world/phase.rs:247-250`). A `dispatch_*` test
//! of the click or of `hire` would therefore panic inside the fake, not in the
//! port. Extending `FakeHost` is a change to `crates/`, which this slice is not
//! allowed to make; the report says so instead of hiding it.
//!
//! What is left is what the slice really decides, and all of it is pure: the
//! compound a hut starts with, the reads with their defaults and clamps, the
//! merge payload, the cell the citizen appears in and the one it walks to, the
//! command it answers, and the goal kinds it removes.

use super::*;
use mod_api::{
    CommandArgument, CommandArgumentValue, CommandInvocation, DimensionKey, MobGoalEntry, Tag,
};

fn initial() -> mod_api::Compound {
    hut::initial_data()
}

fn tag<'a>(data: &'a mod_api::Compound, key: &str) -> &'a Tag {
    data.get(&key.into())
        .unwrap_or_else(|| panic!("missing key {key}"))
}

#[test]
fn the_factory_writes_the_five_keys_a_hut_needs() {
    let data = initial();
    assert_eq!(data.len(), 5, "style, mirror, blueprint, level, hired");
    assert_eq!(
        tag(&data, hut::KEY_STYLE),
        &Tag::String(hut::STYLE_DEFAULT.into())
    );
    assert_eq!(
        tag(&data, hut::KEY_MIRROR),
        &Tag::String(hut::MIRROR_DEFAULT.into())
    );
    assert_eq!(
        tag(&data, hut::KEY_BLUEPRINT),
        &Tag::String(hut::BLUEPRINT_DEFAULT.into())
    );
    assert_eq!(tag(&data, hut::KEY_LEVEL), &Tag::Int(hut::LEVEL_MIN));
    assert_eq!(tag(&data, hut::KEY_HIRED), &Tag::Int(0));
}

#[test]
fn an_empty_compound_reads_as_a_level_one_hut_with_no_citizen() {
    let state = hut::state_of(&mod_api::Compound::new());
    assert_eq!(state.style, hut::STYLE_DEFAULT);
    assert_eq!(state.mirror, hut::MIRROR_DEFAULT);
    assert_eq!(state.blueprint, hut::BLUEPRINT_DEFAULT);
    assert_eq!(state.level, hut::LEVEL_MIN);
    assert_eq!(state.hired, 0);
}

#[test]
fn a_level_written_by_hand_is_clamped_and_a_wrong_type_falls_back() {
    // `/data merge block` can write anything; the port assumes 1..=5 everywhere
    // else, so the read is where the range is enforced.
    let level = |value: Tag| {
        let mut data = initial();
        data.insert(hut::KEY_LEVEL.into(), value);
        hut::state_of(&data).level
    };
    assert_eq!(level(Tag::Int(0)), hut::LEVEL_MIN);
    assert_eq!(level(Tag::Int(-7)), hut::LEVEL_MIN);
    assert_eq!(level(Tag::Int(3)), 3);
    assert_eq!(level(Tag::Int(9)), hut::LEVEL_MAX);
    assert_eq!(level(Tag::Byte(2)), 2);
    assert_eq!(
        level(Tag::String("3".into())),
        hut::LEVEL_MIN,
        "a string is not a level, and guessing one is not this port's job"
    );
    assert_eq!(hut::clamp_level(0), 1);
    assert_eq!(hut::clamp_level(5), 5);
    assert_eq!(hut::clamp_level(6), 5);
}

#[test]
fn a_hire_merges_one_key_and_leaves_the_others_alone() {
    // `merge_data` merges; the payload must not carry the whole state back.
    let payload = hut::with_hired(2);
    assert_eq!(payload.len(), 1);
    assert_eq!(tag(&payload, hut::KEY_HIRED), &Tag::Int(2));

    let mut data = initial();
    data.insert(hut::KEY_STYLE.into(), Tag::String("minecolonies:caledonia".into()));
    data.extend(payload);
    let state = hut::state_of(&data);
    assert_eq!(state.hired, 2);
    assert_eq!(state.style, "minecolonies:caledonia");
    assert_eq!(state.level, hut::LEVEL_MIN);
}

#[test]
fn a_hut_houses_one_citizen_per_level_and_refuses_the_next() {
    assert_eq!(hut::capacity(0), 1, "a level is clamped before it counts");
    assert_eq!(hut::capacity(3), 3);
    assert_eq!(hut::capacity(9), 5);
    let full = hut::State {
        level: 2,
        hired: 2,
        ..hut::state_of(&mod_api::Compound::new())
    };
    assert_eq!(hut::room(&full), 0);
    let room_for_one = hut::State { hired: 1, ..full };
    assert_eq!(hut::room(&room_for_one), 1);
}

#[test]
fn the_citizen_appears_next_to_the_hut_and_walks_four_blocks_out() {
    let hut = [10, 64, -3];
    // north is -Z, south +Z, east +X, west -X, and an unknown facing is north.
    assert_eq!(
        hut::spawn_at(hut, "north"),
        [10.5, 64.0, -3.5],
        "one block in front, centred in the cell"
    );
    assert_eq!(hut::spawn_at(hut, "south"), [10.5, 64.0, -1.5]);
    assert_eq!(hut::spawn_at(hut, "east"), [11.5, 64.0, -2.5]);
    assert_eq!(hut::spawn_at(hut, "west"), [9.5, 64.0, -2.5]);
    assert_eq!(hut::spawn_at(hut, "up"), hut::spawn_at(hut, "north"));
    assert_eq!(
        hut::walk_to(hut, "north"),
        [10.5, 64.0, -6.5],
        "four blocks out, so the hire is a walk and not a pop-in"
    );
    assert_eq!(hut::walk_to(hut, "east"), [14.5, 64.0, -2.5]);
}

#[test]
fn the_command_is_recognised_by_its_words_and_its_block_position() {
    let invocation = |literals: &[&str], value: Option<CommandArgumentValue>| {
        CommandInvocation::new(
            literals.iter().map(|word| (*word).to_owned()).collect(),
            value
                .map(|value| CommandArgument::new(hire::POS.to_owned(), value))
                .into_iter()
                .collect(),
            DimensionKey::OVERWORLD,
            [0.5, 64.0, 0.5],
        )
    };
    let hire = invocation(
        &["mcproto", "hire"],
        Some(CommandArgumentValue::BlockPos([12, 70, -4])),
    );
    assert!(hire::is_hire(&hire));
    assert_eq!(hire::hut_position(&hire), Some([12, 70, -4]));

    // Another subcommand of the same root is not this one.
    let other = invocation(&["mcproto", "fire"], Some(CommandArgumentValue::BlockPos([0, 64, 0])));
    assert!(!hire::is_hire(&other));
    // The root alone, and a word borrowed from another mod's command.
    assert!(!hire::is_hire(&invocation(&["mcproto"], None)));
    assert!(!hire::is_hire(&invocation(
        &["othermod", "hire"],
        Some(CommandArgumentValue::BlockPos([0, 64, 0]))
    )));
    // The right words with the wrong argument kind name no hut at all.
    let wrong_kind = invocation(
        &["mcproto", "hire"],
        Some(CommandArgumentValue::Vec3([1.0, 2.0, 3.0])),
    );
    assert!(hire::is_hire(&wrong_kind));
    assert_eq!(hire::hut_position(&wrong_kind), None);
    assert_eq!(hire::hut_position(&invocation(&["mcproto", "hire"], None)), None);
}

#[test]
fn every_goal_kind_is_removed_once_in_the_order_it_was_read() {
    let goal = |priority: i32, id: &'static str, running: bool| MobGoalEntry {
        priority,
        id,
        running,
    };
    let goals = [
        goal(0, "float", false),
        goal(1, "panic", false),
        goal(5, "stroll", true),
        goal(6, "look_at_player", false),
        goal(7, "stroll", false),
        goal(8, "float", false),
    ];
    assert_eq!(
        citizen::goal_ids(&goals),
        vec!["float", "panic", "stroll", "look_at_player"],
        "a self-styled mob keeps its own name, so the ids come from the read, never from a list here"
    );
    assert!(citizen::goal_ids(&[]).is_empty());
}

#[test]
fn the_container_declares_the_menu_it_names() {
    // The engine drops a container's menu when its slot count and the menu's
    // disagree (`crates/horizon-client/src/game/native_hooks/shared.rs:2832-2834`),
    // and `minecraft:generic_3x3` shows 9 slots.
    let container = hut::container();
    assert_eq!(container.slots, hut::SLOTS);
    assert_eq!(container.slots, 9, "minecraft:generic_3x3 shows nine slots");
    assert_eq!(container.menu.as_deref(), Some(hut::MENU_ID));
    assert!(
        container.inputs.is_empty() && container.outputs.is_empty(),
        "every face reaches every slot; nothing is automation-only"
    );
}
