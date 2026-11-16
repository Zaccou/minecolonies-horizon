//! What each rule of the bus and the raid machine claims, asserted.
//!
//! Every test names the Java line it is about; where the rule is the port's own,
//! the test says so.

use std::cell::RefCell;
use std::rc::Rc;

use mod_api::{Compound, Tag};

use crate::colony::{Rank, WorkOrder, WorkType};

use super::*;

fn died(citizen: i32) -> ColonieEvent {
    ColonieEvent::CitizenDied {
        id: EventId::UNASSIGNED,
        colony: 4,
        citizen,
        damage: "minecraft:fall".to_owned(),
    }
}

fn created() -> ColonieEvent {
    ColonieEvent::ColonyCreated {
        id: EventId::UNASSIGNED,
        colony: 4,
    }
}

fn entering() -> ColonieEvent {
    ColonieEvent::PlayerEntering {
        id: EventId::UNASSIGNED,
        colony: 4,
        player: 0xabc,
        notices: Notices::new(),
    }
}

/// One event of a topic, for the tests that need a chain of them.
fn event_of(topic: Topic) -> ColonieEvent {
    let id = EventId::UNASSIGNED;
    match topic {
        Topic::ColonyCreated => ColonieEvent::ColonyCreated { id, colony: 4 },
        Topic::ColonyDeleted => ColonieEvent::ColonyDeleted { id, colony: 4 },
        Topic::ColonyFlagChanged => ColonieEvent::ColonyFlagChanged { id, colony: 4 },
        Topic::ColonyNameChanged => ColonieEvent::ColonyNameChanged { id, colony: 4 },
        Topic::ColonyTeamColorChanged => ColonieEvent::ColonyTeamColorChanged { id, colony: 4 },
        Topic::ColonyViewUpdated => ColonieEvent::ColonyViewUpdated { id, colony: 4 },
        Topic::BuildingAdded => ColonieEvent::BuildingAdded {
            id,
            colony: 4,
            building: [0, 0, 0],
        },
        Topic::BuildingRemoved => ColonieEvent::BuildingRemoved {
            id,
            colony: 4,
            building: [0, 0, 0],
        },
        Topic::CitizenAdded => ColonieEvent::CitizenAdded {
            id,
            colony: 4,
            citizen: 1,
            source: CitizenAddedSource::Hired,
        },
        other => panic!("no builder for {other:?}"),
    }
}

#[test]
fn the_package_holds_twenty_concrete_classes_with_twenty_distinct_names() {
    // 3 root + 7 colony + 3 buildings + 5 citizens + 2 permissions.
    assert_eq!(Topic::ALL.len(), 20);
    let mut names: Vec<&str> = Topic::ALL.iter().map(|t| t.java_class()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 20);
    for topic in Topic::ALL {
        assert!(
            topic.java_class().starts_with("com.minecolonies.api.eventbus.events"),
            "{}",
            topic.java_class()
        );
        assert_eq!(Topic::from_java_class(topic.java_class()), Some(*topic));
    }
    assert_eq!(Topic::from_java_class("com.minecolonies.api.Colony"), None);
}

#[test]
fn only_the_forge_event_lacks_the_mod_suffix_because_it_lacks_the_ancestor() {
    // `CustomRecipesReloadedEvent` extends Forge's `Event` (`:9`), not
    // `AbstractModEvent`, which is why it is the only one without `Mod`.
    assert_eq!(
        Topic::CustomRecipesReloaded.simple_name(),
        "CustomRecipesReloadedEvent"
    );
    assert_eq!(Topic::ColonyCreated.simple_name(), "ColonyCreatedModEvent");
    assert_eq!(Topic::CustomRecipesReloaded.family(), Family::ForgeEvent);
}

#[test]
fn the_families_count_what_java_actually_extends() {
    assert_eq!(Family::AbstractModEvent.topics().len(), 2);
    assert_eq!(Family::AbstractColonyModEvent.topics().len(), 11);
    assert_eq!(Family::AbstractBuildingModEvent.topics().len(), 3);
    assert_eq!(Family::AbstractCitizenModEvent.topics().len(), 3);
    assert_eq!(Family::ForgeEvent.topics().len(), 1);
    let total: usize = Family::ALL.iter().map(|f| f.topics().len()).sum();
    assert_eq!(total, 20);
}

#[test]
fn buried_removed_and_the_two_permission_events_are_colony_events_not_citizen_ones() {
    // Measured: `CitizenBuriedModEvent.java:12`, `CitizenRemovedModEvent.java:11`
    // and `PlayerEnteringModEvent.java:11` all extend `AbstractColonyModEvent`.
    for topic in [
        Topic::CitizenBuried,
        Topic::CitizenRemoved,
        Topic::PlayerEntering,
        Topic::PlayerLeaving,
    ] {
        assert_eq!(topic.family(), Family::AbstractColonyModEvent, "{topic:?}");
    }
    // So subscribing to the citizen class misses two citizen events.
    let citizen_family = Family::AbstractCitizenModEvent.topics();
    assert!(!citizen_family.contains(&Topic::CitizenBuried));
    assert!(!citizen_family.contains(&Topic::CitizenRemoved));
}

#[test]
fn the_permission_package_is_not_the_family_package() {
    assert_eq!(
        Topic::PlayerEntering.package(),
        "com.minecolonies.api.eventbus.events.colony.permissions"
    );
    assert_eq!(
        Topic::CitizenAdded.package(),
        "com.minecolonies.api.eventbus.events.colony.citizens"
    );
    assert_eq!(
        Topic::ColonyManagerLoaded.package(),
        "com.minecolonies.api.eventbus.events"
    );
}

#[test]
fn seventeen_topics_name_a_colony_and_three_do_not() {
    let scoped = Topic::ALL.iter().filter(|t| t.colony_scoped()).count();
    assert_eq!(scoped, 17);
    for topic in [
        Topic::ColonyManagerLoaded,
        Topic::ColonyManagerUnloaded,
        Topic::CustomRecipesReloaded,
    ] {
        assert!(!topic.colony_scoped());
    }
}

#[test]
fn one_event_rides_forges_bus_and_nineteen_ride_the_mod_one() {
    // `CustomRecipeManager.java:438` posts on `MinecraftForge.EVENT_BUS`; every
    // other post in the tree goes through `getEventBus()`.
    let forge = Topic::ALL.iter().filter(|t| t.bus() == Bus::Forge).count();
    assert_eq!(forge, 1);
    assert_eq!(Topic::CustomRecipesReloaded.bus(), Bus::Forge);
    assert_eq!(Topic::ColonyDeleted.bus(), Bus::Mod);
}

#[test]
fn an_event_is_born_without_an_id_and_the_bus_names_it_once() {
    // Java names it in the constructor (`AbstractModEvent.java:22`); the port's
    // counter is in the bus, so the id exists from the first post only.
    let mut event = created();
    assert!(event.id().is_unassigned());
    let bus = EventBus::new();
    let first = bus.post(&mut event).expect("no loop");
    assert_eq!(first.event.raw(), 1);
    assert_eq!(event.id(), first.event);
    // A second post is a second delivery of the *same* event, not a new id.
    let again = bus.post(&mut event).expect("no loop");
    assert_eq!(again.event, first.event);
    let other = bus.post(&mut created()).expect("no loop");
    assert_eq!(other.event.raw(), 2);
}

#[test]
fn the_event_type_of_a_payload_is_the_java_class_it_copies() {
    assert_eq!(died(1).topic(), Topic::CitizenDied);
    assert_eq!(created().topic(), Topic::ColonyCreated);
    assert_eq!(
        ColonieEvent::CustomRecipesReloaded {
            id: EventId::UNASSIGNED
        }
        .topic(),
        Topic::CustomRecipesReloaded
    );
}

#[test]
fn the_family_accessors_answer_what_the_abstract_classes_answer() {
    // `AbstractColonyModEvent.getColony()` (`:32`).
    assert_eq!(died(12).colony(), Some(4));
    assert_eq!(
        ColonieEvent::ColonyManagerLoaded {
            id: EventId::UNASSIGNED,
            manager: ManagerRef(3)
        }
        .colony(),
        None
    );
    // `AbstractCitizenModEvent.getCitizen().getId()` (`:36`).
    assert_eq!(died(12).citizen(), Some(12));
    // `AbstractBuildingModEvent.getBuilding()` (`:32`), as a position.
    let built = ColonieEvent::BuildingAdded {
        id: EventId::UNASSIGNED,
        colony: 4,
        building: [1, 2, 3],
    };
    assert_eq!(built.building(), Some([1, 2, 3]));
    assert_eq!(built.citizen(), None);
    // The three events that name a player, two of which return another type in
    // Java (`ColonyPlayer` vs `Player`).
    assert_eq!(entering().player(), Some(0xabc));
}

#[test]
fn a_citizen_added_event_keeps_java_s_five_sources_even_the_one_nothing_posts() {
    let names: Vec<&str> = CitizenAddedSource::ALL.iter().map(|s| s.java_name()).collect();
    assert_eq!(
        names,
        ["INITIAL", "BORN", "HIRED", "RESURRECTED", "COMMANDS"]
    );
    // `BORN` is declared (`:49`) and never posted in this tree: the birth path
    // fires the *description* event instead (`ReproductionManager.java:213`).
    assert!(CitizenAddedSource::ALL.contains(&CitizenAddedSource::Born));
}

#[test]
fn a_rank_change_carries_both_ranks_so_a_listener_can_tell_promotion_from_demotion() {
    let event = ColonieEvent::ColonyPlayerRankChanged {
        id: EventId::UNASSIGNED,
        colony: 4,
        player: 0x10,
        new_rank: Rank::Officer,
        old_rank: Rank::Friend,
    };
    match &event {
        ColonieEvent::ColonyPlayerRankChanged {
            new_rank, old_rank, ..
        } => {
            assert_eq!(*new_rank, Rank::Officer);
            assert_eq!(*old_rank, Rank::Friend);
            assert!(new_rank.is_manager());
        }
        _ => panic!("wrong variant"),
    }
    assert_eq!(event.player(), Some(0x10));
}

#[test]
fn a_construction_event_carries_the_order_that_finished_not_only_its_id() {
    // `getWorkOrder()` (`:33`): Java hands the completed `WorkOrderBuilding`.
    let order = WorkOrder::new(7, "minecolonies:builder", [1, 2, 3], WorkType::Upgrade, 3);
    let event = ColonieEvent::BuildingConstruction {
        id: EventId::UNASSIGNED,
        colony: 4,
        building: [1, 2, 3],
        work_order: order.clone(),
    };
    match &event {
        ColonieEvent::BuildingConstruction { work_order, .. } => {
            assert_eq!(work_order.work, WorkType::Upgrade);
            assert_eq!(work_order.target_level, 3);
        }
        _ => panic!("wrong variant"),
    }
    assert_eq!(event.building(), Some([1, 2, 3]));
}

#[test]
fn a_burial_event_owns_the_saved_nbt_so_the_caller_cannot_change_it_later() {
    // Java copies in (`:46`) and copies out (`:68`) because the tag is mutable;
    // in Rust the event takes it by value, which is the same guarantee.
    let mut saved = Compound::new();
    saved.insert("name".into(), Tag::String("Alice".into()));
    let event = ColonieEvent::CitizenBuried {
        id: EventId::UNASSIGNED,
        colony: 4,
        grave: [5, 6, 7],
        saved: saved.clone(),
        citizen_name: "Alice".to_owned(),
        citizen_job: Some("minecolonies:farmer".to_owned()),
        undertaker: 9,
        day: 42,
    };
    saved.insert("later".into(), Tag::Int(1));
    match &event {
        ColonieEvent::CitizenBuried {
            saved,
            citizen_job,
            day,
            ..
        } => {
            assert_eq!(saved.len(), 1);
            assert_eq!(*day, 42);
            assert_eq!(citizen_job.as_deref(), Some("minecolonies:farmer"));
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn a_removal_reason_is_one_of_minecraft_s_five() {
    let event = ColonieEvent::CitizenRemoved {
        id: EventId::UNASSIGNED,
        colony: 4,
        citizen: 12,
        reason: RemovalReason::Killed,
    };
    // `getCitizenId()` (`:44`) — the id, not the citizen data.
    assert_eq!(event.citizen(), Some(12));
    assert_ne!(RemovalReason::Killed, RemovalReason::Discarded);
}

#[test]
fn an_entering_event_hides_itself_from_spectators_unless_told_otherwise() {
    // `shouldShowNotification()` (`:53-56`), whole.
    let notices = Notices::new();
    assert!(notices.should_show(false));
    assert!(!notices.should_show(true));
    let mut lifted = Notices::new();
    lifted.allow_spectators();
    assert!(lifted.should_show(true));
    let mut off = Notices::new();
    off.disable();
    assert!(!off.should_show(false));
    assert!(!off.should_show(true));
    // The constructor's defaults, with their source.
    assert!(NOTICES_SHOW_DEFAULT);
    assert!(!NOTICES_SPECTATORS_DEFAULT);
    assert!(NOTICES_SHOW_DEFAULT_SOURCE.ends_with("PlayerEnteringModEvent.java:21"));
}

#[test]
fn an_event_reaches_each_subscriber_once_in_registration_order() {
    // `List.add` at the end (`:26`), iteration in that order (`:40`).
    let mut bus = EventBus::new();
    for label in ["colony-log", "achievements", "sound"] {
        bus.subscribe(Topic::CitizenDied, label, |_, _| Ok(()));
    }
    let mut event = died(12);
    let delivery = bus.post(&mut event).expect("no loop");
    assert_eq!(delivery.called.len(), 3);
    assert_eq!(
        bus.trace().labels(delivery.event),
        ["colony-log", "achievements", "sound"]
    );
    assert_eq!(bus.subscribers(Topic::CitizenDied).len(), 3);
}

#[test]
fn a_failing_subscriber_is_recorded_and_the_next_one_still_runs() {
    // `catch (Exception)` per handler, log, carry on (`:42-50`).
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut bus = EventBus::new();
    bus.subscribe(Topic::CitizenDied, "first", {
        let seen = Rc::clone(&seen);
        move |_, _| {
            seen.borrow_mut().push("first");
            Ok(())
        }
    });
    bus.subscribe(Topic::CitizenDied, "broken", |_, _| {
        Err(HandlerError("journeymap map missing"))
    });
    bus.subscribe(Topic::CitizenDied, "third", {
        let seen = Rc::clone(&seen);
        move |_, _| {
            seen.borrow_mut().push("third");
            Ok(())
        }
    });
    let mut event = died(12);
    let delivery = bus.post(&mut event).expect("no loop");
    assert_eq!(delivery.called.len(), 3);
    assert_eq!(delivery.failed.len(), 1);
    assert_eq!(*seen.borrow(), ["first", "third"]);
    // The trace keeps the error, which the delivery alone would lose.
    assert_eq!(
        bus.trace().failures(),
        [("broken", HandlerError("journeymap map missing"))]
    );
}

#[test]
fn the_same_handler_registered_twice_is_called_twice_because_java_appends() {
    // `subscribe` does not deduplicate (`:26`): the same handler registered
    // twice is two list entries and two calls.
    let mut bus = EventBus::new();
    let id = bus.subscribe(Topic::CitizenDied, "double", |_, _| Ok(()));
    let same = bus.subscribe(Topic::CitizenDied, "double", |_, _| Ok(()));
    assert_ne!(id, same);
    let mut event = died(12);
    let delivery = bus.post(&mut event).expect("no loop");
    assert_eq!(delivery.called, vec![id, same]);
}

#[test]
fn subscribing_to_an_abstract_class_is_accepted_and_never_fires() {
    // `DefaultEventBus.java:32` looks up `event.getClass()`: an abstract class
    // is nobody's class, so the subscription is dead weight.
    let mut bus = EventBus::new();
    let id = bus.subscribe_family(Family::AbstractCitizenModEvent, "wants-all-citizens", |_, _| {
        Ok(())
    });
    let mut event = died(12);
    let delivery = bus.post(&mut event).expect("no loop");
    assert!(delivery.called.is_empty());
    assert!(bus.trace().is_empty());
    // The event was posted all the same: `posted` is not `delivered`.
    assert_eq!(bus.trace().posted().len(), 1);
    assert_eq!(bus.inert(), vec![(id, Inert::Family(Family::AbstractCitizenModEvent), "wants-all-citizens")]);
    // What the subscriber probably wanted, and can name now.
    assert!(Family::AbstractCitizenModEvent
        .topics()
        .contains(&Topic::CitizenDied));
}

#[test]
fn subscribing_to_the_forge_event_on_the_mod_bus_never_fires_either() {
    // The class exists and compiles; no post ever reaches this bus
    // (`CustomRecipeManager.java:438`).
    let mut bus = EventBus::new();
    let id = bus.subscribe(Topic::CustomRecipesReloaded, "recipes", |_, _| Ok(()));
    let mut event = ColonieEvent::CustomRecipesReloaded {
        id: EventId::UNASSIGNED,
    };
    assert!(bus.post(&mut event).expect("no loop").called.is_empty());
    assert_eq!(
        bus.inert(),
        vec![(id, Inert::ForeignBus(Topic::CustomRecipesReloaded), "recipes")]
    );
}

#[test]
fn forge_s_bus_orders_by_priority_and_registration_order_inside_one_priority() {
    // `EventPriority.HIGHEST` first (`journeymap/v6/EventListener.java:61`), and
    // two handlers of the same priority keep their registration order.
    let mut bus = EventBus::new();
    bus.subscribe_by_priority(Topic::CustomRecipesReloaded, Priority::Low, "low", |_, _| Ok(()))
        .unwrap();
    bus.subscribe_by_priority(
        Topic::CustomRecipesReloaded,
        Priority::Highest,
        "highest",
        |_, _| Ok(()),
    )
    .unwrap();
    bus.subscribe_by_priority(Topic::CustomRecipesReloaded, Priority::Normal, "normal", |_, _| Ok(()))
        .unwrap();
    bus.subscribe_by_priority(Topic::CustomRecipesReloaded, Priority::Low, "low-again", |_, _| Ok(()))
        .unwrap();
    let labels: Vec<&str> = bus
        .subscribers(Topic::CustomRecipesReloaded)
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    assert_eq!(labels, ["highest", "normal", "low", "low-again"]);
}

#[test]
fn a_priority_on_the_mod_bus_is_refused_because_java_s_subscribe_has_no_such_parameter() {
    // `EventBus.subscribe(Class, EventHandler)` — two arguments (`:17`).
    let mut bus = EventBus::new();
    let refusal = bus
        .subscribe_by_priority(Topic::ColonyCreated, Priority::High, "x", |_, _| Ok(()))
        .unwrap_err();
    assert_eq!(
        refusal,
        Refusal::WrongBus {
            topic: Topic::ColonyCreated,
            bus: Bus::Mod
        }
    );
    assert_eq!(refusal.name(), "wrong_bus");
}

#[test]
fn an_event_with_no_subscriber_is_still_posted_and_delivered_to_nobody() {
    // `:33-35`: return silently, which the trace keeps as `posted` only.
    let bus = EventBus::new();
    let mut event = created();
    let delivery = bus.post(&mut event).expect("no loop");
    assert!(delivery.called.is_empty());
    assert!(delivery.failed.is_empty());
    assert_eq!(bus.trace().posted().len(), 1);
    assert_eq!(bus.trace().event(delivery.event), Some(&event));
}

#[test]
fn a_handler_that_posts_its_own_topic_is_refused_instead_of_recursing() {
    // Java re-enters `post` (`:40-44`) and recurses until `StackOverflowError`.
    let refusals = Rc::new(RefCell::new(Vec::new()));
    let mut bus = EventBus::new();
    bus.subscribe(Topic::ColonyDeleted, "gateway", {
        let refusals = Rc::clone(&refusals);
        move |_, ctx| {
            let mut nested = ColonieEvent::ColonyDeleted {
                id: EventId::UNASSIGNED,
                colony: 4,
            };
            match ctx.post(&mut nested) {
                Ok(_) => Ok(()),
                Err(refusal) => {
                    refusals.borrow_mut().push(refusal.name());
                    Err(HandlerError("nested post refused"))
                }
            }
        }
    });
    let mut event = ColonieEvent::ColonyDeleted {
        id: EventId::UNASSIGNED,
        colony: 4,
    };
    let delivery = bus.post(&mut event).expect("the outer post is fine");
    assert_eq!(delivery.called.len(), 1);
    assert_eq!(*refusals.borrow(), ["reentered"]);
    // One post reached the trace; the refused one did not.
    assert_eq!(bus.trace().posted().len(), 1);
}

#[test]
fn a_chain_of_distinct_topics_stops_at_the_port_s_depth_limit() {
    // Port-only (`DEPTH_LIMIT`): Java has no guard and would blow the stack on a
    // cycle across types.
    let topics = [
        Topic::ColonyCreated,
        Topic::ColonyDeleted,
        Topic::ColonyFlagChanged,
        Topic::ColonyNameChanged,
        Topic::ColonyTeamColorChanged,
        Topic::ColonyViewUpdated,
        Topic::BuildingAdded,
        Topic::BuildingRemoved,
        Topic::CitizenAdded,
    ];
    let refusals = Rc::new(RefCell::new(Vec::new()));
    let mut bus = EventBus::new();
    for (index, topic) in topics.iter().enumerate() {
        let next = topics.get(index + 1).copied();
        let refusals = Rc::clone(&refusals);
        bus.subscribe(*topic, "chain", move |_event, ctx| {
            let Some(next) = next else { return Ok(()) };
            let mut nested = event_of(next);
            match ctx.post(&mut nested) {
                Ok(_) => Ok(()),
                Err(refusal) => {
                    refusals.borrow_mut().push(refusal.name());
                    Ok(())
                }
            }
        });
    }
    let mut first = created();
    bus.post(&mut first).expect("the outermost post is fine");
    assert_eq!(*refusals.borrow(), ["too_deep"]);
    assert_eq!(DEPTH_LIMIT, 8);
    assert!(DEPTH_LIMIT_SOURCE.contains("port-only"));
}

#[test]
fn a_later_handler_sees_the_veto_an_earlier_handler_set_and_so_does_the_caller() {
    // `Colony.java:1641-1644`: the event is posted, then the caller reads
    // `shouldShowNotification()`. Order therefore decides the outcome.
    let seen_by_second = Rc::new(RefCell::new(None));
    let mut bus = EventBus::new();
    bus.subscribe(Topic::PlayerEntering, "chat-notice", |event, _| {
        if let Some(notices) = event.notices_mut() {
            notices.disable();
        }
        Ok(())
    });
    bus.subscribe(Topic::PlayerEntering, "audit", {
        let seen_by_second = Rc::clone(&seen_by_second);
        move |event, _| {
            *seen_by_second.borrow_mut() = event.notices().map(|notices| notices.show);
            Ok(())
        }
    });
    let mut event = entering();
    bus.post(&mut event).expect("no loop");
    assert_eq!(*seen_by_second.borrow(), Some(false));
    // The event was borrowed, not consumed: the caller still holds it.
    assert!(!event.notices().expect("a permission event").should_show(false));
}

#[test]
fn the_trace_keeps_the_event_as_it_was_posted_not_as_handlers_left_it() {
    // Otherwise a replay would start from the state a handler already changed.
    let mut bus = EventBus::new();
    bus.subscribe(Topic::PlayerEntering, "veto", |event, _| {
        if let Some(notices) = event.notices_mut() {
            notices.disable();
        }
        Ok(())
    });
    let mut event = entering();
    let delivery = bus.post(&mut event).expect("no loop");
    let posted = bus.trace().event(delivery.event).expect("posted").clone();
    assert!(posted.notices().expect("permission event").show);
    assert!(!event.notices().expect("permission event").show);
    assert_ne!(posted, event);
}

#[test]
fn a_trace_reads_back_who_heard_what_and_when() {
    let mut bus = EventBus::new();
    let death = bus.subscribe(Topic::CitizenDied, "graveyard", |_, _| Ok(()));
    let creation = bus.subscribe(Topic::ColonyCreated, "founding-notice", |_, _| Ok(()));
    let mut first = died(12);
    let first = bus.post(&mut first).expect("no loop");
    let mut second = created();
    let second = bus.post(&mut second).expect("no loop");
    let trace = bus.trace();
    assert_eq!(trace.len(), 2);
    assert_eq!(trace.sequence(first.event), vec![death]);
    assert_eq!(trace.sequence(second.event), vec![creation]);
    assert_eq!(trace.heard_by(death), vec![(first.event, Topic::CitizenDied)]);
    assert_eq!(trace.events_of(Topic::ColonyCreated).len(), 1);
    assert_eq!(trace.records()[0].seq, 0);
    assert_eq!(trace.records()[1].seq, 1);
}

#[test]
fn a_trace_replayed_into_a_bus_wired_the_same_way_gives_the_same_trace() {
    // This is the point of the trace: a run is provable, not described.
    let wire = |bus: &mut EventBus| {
        bus.subscribe(Topic::CitizenDied, "graveyard", |_, _| Ok(()));
        bus.subscribe(Topic::CitizenDied, "statistics", |event, _| {
            if event.citizen() == Some(0) {
                return Err(HandlerError("no citizen zero"));
            }
            Ok(())
        });
        bus.subscribe(Topic::ColonyCreated, "founding-notice", |_, _| Ok(()));
    };
    let mut original = EventBus::new();
    wire(&mut original);
    for mut event in [died(12), died(0), created()] {
        let _ = original.post(&mut event);
    }

    let mut replay = EventBus::new();
    wire(&mut replay);
    replay.replay(&original.trace()).expect("nothing loops");

    assert_eq!(replay.trace().posted(), original.trace().posted());
    assert_eq!(replay.trace().records(), original.trace().records());
    // And the ids are the ones the first run gave, not fresh ones.
    let ids: Vec<u64> = replay.trace().posted().iter().map(|e| e.id().raw()).collect();
    assert_eq!(ids, [1, 2, 3]);
}

#[test]
fn the_bus_names_the_label_of_a_subscription_it_holds() {
    let mut bus = EventBus::new();
    let id = bus.subscribe(Topic::CitizenDied, "graveyard", |_, _| Ok(()));
    assert_eq!(bus.label_of(id), Some("graveyard"));
    assert_eq!(bus.label_of(SubscriberId(99)), None);
}

#[test]
fn raid_ids_start_at_one_and_the_counter_never_enters_the_last_hundred() {
    // `EventManager.java:44` then `:85-95`.
    assert_eq!(FIRST_ID, 1);
    let mut life = Lifecycle::new();
    assert_eq!(life.take_id(), 1);
    assert_eq!(life.take_id(), 2);
    assert_eq!(take_from(1), (1, 2));
    // Three calls from the ceiling: the guard resets to 1 before incrementing.
    assert_eq!(take_from(i32::MAX - 99), (1, 2));
    assert_eq!(take_from(i32::MAX - 100), (i32::MAX - 100, i32::MAX - 99));
    assert_eq!(ID_CEILING_MARGIN, 100);
    assert!(ID_CEILING_MARGIN_SOURCE.ends_with("EventManager.java:87"));
}

#[test]
fn a_colony_event_without_an_id_is_refused_because_the_map_keys_on_it() {
    // `addEvent` warns and returns when `getID() == 0` (`:70-74`).
    let mut life = Lifecycle::new();
    let refusal = life.add(Raid::new(0, "minecolonies:pirate_raid")).unwrap_err();
    assert_eq!(refusal, LifecycleRefusal::NoId);
    assert_eq!(refusal.name(), "no_id");
    assert!(life.is_empty());
}

#[test]
fn adding_the_same_id_twice_overwrites_because_put_is_not_a_check() {
    // `events.put(colonyEvent.getID(), colonyEvent)` (`:75`).
    let mut life = Lifecycle::new();
    life.add(Raid::new(1, "minecolonies:barbarian")).unwrap();
    life.add(Raid::new(1, "minecolonies:pirate")).unwrap();
    assert_eq!(life.len(), 1);
    assert_eq!(life.ids(), vec![1]);
}

#[test]
fn a_starting_event_starts_on_every_tick_until_its_status_moves() {
    // `:198-201` changes nothing, so `onStart` runs again and again.
    let mut life = Lifecycle::new();
    let mut trace = Trace::default();
    let id = life.take_id();
    life.add(Raid::new(id, "minecolonies:pirate_raid")).unwrap();
    assert_eq!(life.tick(&mut trace), vec![RaidStep::Started { raid: 1 }]);
    assert_eq!(life.tick(&mut trace), vec![RaidStep::Started { raid: 1 }]);
    assert_eq!(life.len(), 1);
    life.set_status(1, Status::Preparing).unwrap();
    assert_eq!(life.tick(&mut trace), vec![RaidStep::Updated { raid: 1 }]);
    assert_eq!(trace.notes().len(), 3);
    assert_eq!(trace.notes()[0].name(), "onStart");
    assert_eq!(trace.notes()[2].name(), "onUpdate");
}

#[test]
fn a_done_event_finishes_once_and_leaves_the_colony() {
    // `:191-197`: `onFinish`, restore the backup, remove.
    let mut life = Lifecycle::new();
    let mut trace = Trace::default();
    life.add(Raid::new(3, "minecolonies:norsemen")).unwrap();
    life.set_status(3, Status::Done).unwrap();
    assert_eq!(life.tick(&mut trace), vec![RaidStep::Finished { raid: 3 }]);
    assert!(life.is_empty());
    assert!(life.tick(&mut trace).is_empty());
    assert_eq!(trace.notes()[0].name(), "onFinish");
}

#[test]
fn a_canceled_event_leaves_without_finishing_and_java_never_restores_its_ship() {
    // `:202-206` removes on `CANCELED` with no `onFinish`, so the backup the
    // `DONE` branch restores at `:194` stays unrestored.
    let mut life = Lifecycle::new();
    let mut trace = Trace::default();
    life.add(Raid::new(2, "minecolonies:egyptian")).unwrap();
    life.set_status(2, Status::Canceled).unwrap();
    assert_eq!(life.tick(&mut trace), vec![RaidStep::Dropped { raid: 2 }]);
    assert!(life.is_empty());
    assert_eq!(trace.notes()[0].name(), "removed");
}

#[test]
fn a_raid_is_active_in_two_statuses_only() {
    // `IColonyRaidEvent.isRaidActive()` (`:50-55`).
    let active: Vec<bool> = Status::ALL.iter().map(|s| s.is_active()).collect();
    assert_eq!(active, [false, true, true, false, false, false]);
    let mut life = Lifecycle::new();
    life.add(Raid::new(5, "minecolonies:amazon")).unwrap();
    assert!(!life.is_active(5));
    life.set_status(5, Status::Progressing).unwrap();
    assert!(life.is_active(5));
    assert!(!life.is_active(6));
    assert_eq!(
        life.set_status(6, Status::Done).unwrap_err(),
        LifecycleRefusal::Unknown(6)
    );
    assert_eq!(LifecycleRefusal::Unknown(6).name(), "unknown_event");
}

#[test]
fn the_machine_visits_its_events_in_id_order_and_the_trace_says_so() {
    // Port choice: Java walks a `HashMap` (`:54`), whose order is unspecified.
    let mut life = Lifecycle::new();
    let mut trace = Trace::default();
    life.add(Raid::new(9, "minecolonies:pirate")).unwrap();
    life.add(Raid::new(3, "minecolonies:barbarian")).unwrap();
    life.set_status(9, Status::Progressing).unwrap();
    life.set_status(3, Status::Progressing).unwrap();
    let steps = life.tick(&mut trace);
    assert_eq!(
        steps,
        vec![RaidStep::Updated { raid: 3 }, RaidStep::Updated { raid: 9 }]
    );
    assert_eq!(life.ids(), vec![3, 9]);
}
