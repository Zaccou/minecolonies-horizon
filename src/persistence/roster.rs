//! The citizen roster: who lives in the colony, and what Java saves beside them.
//!
//! Java reference: `core/colony/managers/CitizenManager.java:173-205` (the list)
//! and `core/colony/CitizenData.java:1316-1380` / `:1425-1500` (one record).
//!
//! **A citizen record is one `CitizenData.serializeNBT`, and the colony only
//! reads part of it.** The full record carries thirty keys — a job, a family, an
//! inventory, seven chat options, a skill handler, a happiness handler. What this
//! port models is the identity (`id`, `name`) and the three needs
//! [`Needs`] already holds in the same units: `saturation`, `asleep`, `justAte`.
//! Writing the rest would be inventing data; `MANQUANT.md` says what each unread
//! key would cost.
//!
//! **One default is a trap and is kept on purpose.** `CitizenData.java:1460`
//! reads saturation with `getDouble`, so a record without the key is a **starving**
//! citizen, not a fresh one (`:526` creates them at the maximum). The port takes
//! Java's 0.0 and reports it, because filling the colony back to full would heal
//! a save Java would have starved.
//!
//! **Two counters are not saved.** `until_saturation_check` and
//! `until_heal_check` are this port's tick bookkeeping (`src/citizens/needs.rs:190-215`);
//! Java has no equivalent field, so a record read back starts them at their
//! defaults. [`Needs::health`] is not saved either: Java keeps a citizen's health
//! on the entity's attribute, never in the colony compound.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use crate::citizens::{Needs, MAX_SATURATION};
use crate::colony::{Citizen, Colony};

use super::format::{
    KEY_ASLEEP, KEY_CITIZEN_MANAGER, KEY_CITIZENS, KEY_ID, KEY_JUST_ATE, KEY_NAME, KEY_SATURATION,
};
use super::{nbt, Refusal, Repair};

/// One record, written: the identity Java keys by `id`, and the three needs.
fn record(citizen: &Citizen, needs: &Needs) -> Compound {
    Compound::from([
        (KEY_ID.into(), Tag::Int(citizen.id)),
        (KEY_NAME.into(), Tag::String(citizen.name.as_str().into())),
        (KEY_SATURATION.into(), nbt::double(needs.saturation)),
        (KEY_ASLEEP.into(), nbt::boolean_tag(needs.asleep)),
        (KEY_JUST_ATE.into(), nbt::boolean_tag(needs.just_ate)),
    ])
}

/// The `citizenManager` compound: `{citizens: [...]}`.
///
/// **Refused when the two lists disagree**, in either direction: Java keeps one
/// `CitizenData` per citizen and the needs live inside it, so a colony passed
/// here with a citizen whose needs are missing (or needs for a citizen who is not
/// in the colony) is a caller bug this format refuses to paper over with a
/// default.
pub fn write(colony: &Colony, needs: &BTreeMap<i32, Needs>) -> Result<Compound, Refusal> {
    for citizen in &colony.citizens {
        if !needs.contains_key(&citizen.id) {
            return Err(Refusal::RosterDiverges { id: citizen.id });
        }
    }
    for id in needs.keys() {
        if !colony.citizens.iter().any(|citizen| citizen.id == *id) {
            return Err(Refusal::RosterDiverges { id: *id });
        }
    }
    let records: Vec<Compound> = colony
        .citizens
        .iter()
        .map(|citizen| {
            let held: &Needs = &needs[&citizen.id];
            record(citizen, held)
        })
        .collect();
    Ok(Compound::from([(
        KEY_CITIZEN_MANAGER.into(),
        Tag::Compound(Compound::from([(
            KEY_CITIZENS.into(),
            nbt::compound_list(records),
        )])),
    )]))
}

/// The roster, read back as the colony's citizen list plus one [`Needs`] each.
///
/// Java's three answers are kept: a record that is not a compound is skipped
/// (`CitizenManager.java:178` streams compounds only), a missing name is the
/// empty name, a missing saturation is 0.0.
pub fn read(
    data: &Compound,
    repairs: &mut Vec<Repair>,
) -> Result<(Vec<Citizen>, BTreeMap<i32, Needs>), Refusal> {
    let Some(Tag::Compound(manager)) = data.get(&KEY_CITIZEN_MANAGER.into()) else {
        return Err(Refusal::MissingCitizens);
    };
    let Some(Tag::List(records)) = manager.get(&KEY_CITIZENS.into()) else {
        return Err(Refusal::MissingCitizens);
    };

    let mut citizens = Vec::with_capacity(records.len());
    let mut needs: BTreeMap<i32, Needs> = BTreeMap::new();
    for (index, item) in records.iter().enumerate() {
        let Tag::Compound(record) = item else {
            repairs.push(Repair::CitizenDropped { index });
            continue;
        };
        let Some(id) = nbt::int(record, KEY_ID).filter(|id| *id > 0) else {
            // `CitizenDataManager.java:16` reads 0 for an absent id, and `CitizenManager`
            // only ever hands out ids from 1: a citizen of id 0 is hired by
            // nothing and answers for no one.
            return Err(Refusal::CitizenWithoutId { index });
        };
        if needs.contains_key(&id) {
            // `CitizenManager.java:178-180` collects into a map, and
            // `Collectors.toMap` throws on a duplicate key — out of
            // `loadColony`'s `catch` (:740-744), so Java drops the whole colony.
            return Err(Refusal::DuplicateCitizen { id });
        }
        let name = nbt::text(record, KEY_NAME).unwrap_or_default();
        if name.is_empty() {
            repairs.push(Repair::CitizenUnnamed { id });
        }
        let saturation = saturation(record, id, repairs);
        let mut held = Needs::new();
        held.saturation = saturation;
        held.asleep = nbt::boolean(record, KEY_ASLEEP).unwrap_or(false);
        held.just_ate = nbt::boolean(record, KEY_JUST_ATE).unwrap_or(false);
        citizens.push(Citizen::new(id, &name));
        needs.insert(id, held);
    }
    Ok((citizens, needs))
}

/// `saturation`, with Java's 0.0 when the key is absent and this port's range
/// when it is not (`src/citizens/needs.rs:32`: `0..=60`). A save cannot hand the
/// AI a value every other rule would then have to guard.
fn saturation(record: &Compound, id: i32, repairs: &mut Vec<Repair>) -> f64 {
    let Some(tag) = record.get(&KEY_SATURATION.into()) else {
        repairs.push(Repair::SaturationMissing { id });
        return 0.0;
    };
    let Some(raw) = nbt::double_of(tag) else {
        repairs.push(Repair::SaturationMissing { id });
        return 0.0;
    };
    let held = if raw.is_finite() {
        raw.clamp(0.0, MAX_SATURATION)
    } else {
        0.0
    };
    if held != raw {
        repairs.push(Repair::SaturationOutOfRange { id });
    }
    held
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 1;

    fn colony() -> Colony {
        let mut colony = Colony::new(7, "Bridgewater", "minecraft:overworld", [0, 70, 0], OWNER);
        for (id, name) in [(1, "Ada"), (2, "Bram"), (3, "Cleo"), (4, "Dirk"), (5, "Els")] {
            colony.citizens.push(Citizen::new(id, name));
        }
        colony
    }

    fn needs() -> BTreeMap<i32, Needs> {
        let mut needs = BTreeMap::new();
        for id in 1..=5 {
            let mut held = Needs::new();
            held.saturation = 60.0 - f64::from(id);
            held.asleep = id == 3;
            held.just_ate = id == 2;
            needs.insert(id, held);
        }
        needs
    }

    /// The citizen list of `citizenManager.citizens`, for a test that edits one
    /// record the way a hand-written `/data merge` would.
    fn records(data: &mut Compound) -> &mut Vec<Tag> {
        let Some(Tag::Compound(manager)) = data.get_mut(&KEY_CITIZEN_MANAGER.into()) else {
            panic!("citizenManager is a compound")
        };
        let Some(Tag::List(items)) = manager.get_mut(&KEY_CITIZENS.into()) else {
            panic!("citizens is a list")
        };
        items
    }

    #[test]
    fn five_citizens_round_trip_with_their_needs() {
        let colony = colony();
        let data = write(&colony, &needs()).expect("the roster writes");
        let mut repairs = Vec::new();
        let (citizens, read) = read(&data, &mut repairs).expect("the roster reads");
        assert!(repairs.is_empty(), "a clean roster needs no repair");
        assert_eq!(citizens, colony.citizens);
        assert_eq!(read, needs());
        assert!(read[&3].asleep, "the third citizen is in bed");
        assert!(read[&2].just_ate);
        assert!(!read[&1].asleep);
    }

    #[test]
    fn the_record_is_java_shape_key_for_key() {
        let data = write(&colony(), &needs()).expect("writes");
        let mut repairs = Vec::new();
        let (_, read) = read(&data, &mut repairs).expect("reads");
        assert_eq!(read.len(), 5);

        let mut edited = data.clone();
        let items = records(&mut edited);
        assert_eq!(items.len(), 5);
        let Tag::Compound(first) = &items[0] else {
            panic!("a record is a compound")
        };
        assert_eq!(nbt::int(first, KEY_ID), Some(1));
        assert_eq!(nbt::text(first, KEY_NAME).as_deref(), Some("Ada"));
        assert_eq!(
            first.get(&KEY_SATURATION.into()).and_then(nbt::double_of),
            Some(59.0)
        );
        assert_eq!(nbt::boolean(first, KEY_ASLEEP), Some(false));
        assert_eq!(nbt::boolean(first, KEY_JUST_ATE), Some(false));
    }

    #[test]
    fn a_citizen_without_an_id_is_refused_with_its_index() {
        let mut data = write(&colony(), &needs()).expect("writes");
        let Tag::Compound(second) = &mut records(&mut data)[1] else {
            panic!("a record is a compound")
        };
        second.remove(&KEY_ID.into());
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::CitizenWithoutId { index: 1 })
        );
    }

    #[test]
    fn two_citizens_with_one_id_are_refused_because_java_drops_the_colony() {
        let mut data = write(&colony(), &needs()).expect("writes");
        let copy = records(&mut data)[0].clone();
        records(&mut data).push(copy);
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::DuplicateCitizen { id: 1 })
        );
    }

    #[test]
    fn an_unnamed_citizen_keeps_its_id_and_is_reported() {
        let mut data = write(&colony(), &needs()).expect("writes");
        let Tag::Compound(first) = &mut records(&mut data)[0] else {
            panic!("a record is a compound")
        };
        first.remove(&KEY_NAME.into());
        let mut repairs = Vec::new();
        let (citizens, _) = read(&data, &mut repairs).expect("an unnamed citizen is a citizen");
        assert_eq!(citizens[0].name, "");
        assert_eq!(repairs, vec![Repair::CitizenUnnamed { id: 1 }]);
    }

    #[test]
    fn a_record_without_saturation_starves_like_java_and_says_so() {
        let mut data = write(&colony(), &needs()).expect("writes");
        let Tag::Compound(first) = &mut records(&mut data)[0] else {
            panic!("a record is a compound")
        };
        first.remove(&KEY_SATURATION.into());
        let mut repairs = Vec::new();
        let (_, read) = read(&data, &mut repairs).expect("reads");
        assert_eq!(
            read[&1].saturation, 0.0,
            "CitizenData.java:1460 getDouble — absent is 0.0, not the maximum"
        );
        assert_eq!(repairs, vec![Repair::SaturationMissing { id: 1 }]);
    }

    #[test]
    fn a_saturation_out_of_range_is_clamped_and_named() {
        let mut data = write(&colony(), &needs()).expect("writes");
        let Tag::Compound(first) = &mut records(&mut data)[0] else {
            panic!("a record is a compound")
        };
        first.insert(KEY_SATURATION.into(), nbt::double(900.0));
        let mut repairs = Vec::new();
        let (_, read) = read(&data, &mut repairs).expect("reads");
        assert_eq!(read[&1].saturation, MAX_SATURATION);
        assert_eq!(repairs, vec![Repair::SaturationOutOfRange { id: 1 }]);
    }

    #[test]
    fn asleep_and_just_ate_are_false_when_the_keys_are_gone() {
        let mut data = write(&colony(), &needs()).expect("writes");
        for item in records(&mut data).iter_mut() {
            let Tag::Compound(record) = item else {
                panic!("a record is a compound")
            };
            record.remove(&KEY_ASLEEP.into());
            record.remove(&KEY_JUST_ATE.into());
        }
        let (_, read) = read(&data, &mut Vec::new()).expect("reads");
        assert!(read.values().all(|needs| !needs.asleep && !needs.just_ate));
    }

    #[test]
    fn a_roster_that_is_not_a_list_of_records_is_refused() {
        let data = Compound::from([(
            KEY_CITIZEN_MANAGER.into(),
            Tag::Compound(Compound::from([(KEY_CITIZENS.into(), Tag::Int(3))])),
        )]);
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::MissingCitizens)
        );
        assert_eq!(
            read(&Compound::new(), &mut Vec::new()).err(),
            Some(Refusal::MissingCitizens)
        );
    }

    #[test]
    fn a_record_that_is_not_a_compound_is_skipped_and_counted() {
        let data = Compound::from([(
            KEY_CITIZEN_MANAGER.into(),
            Tag::Compound(Compound::from([(
                KEY_CITIZENS.into(),
                Tag::List(vec![Tag::Int(4)]),
            )])),
        )]);
        let mut repairs = Vec::new();
        let (citizens, needs) = read(&data, &mut repairs).expect("an empty roster");
        assert!(citizens.is_empty() && needs.is_empty());
        assert_eq!(repairs, vec![Repair::CitizenDropped { index: 0 }]);
    }

    #[test]
    fn needs_for_an_absent_citizen_and_a_citizen_without_needs_are_both_refused() {
        let colony = colony();
        let mut short = needs();
        short.remove(&4);
        assert_eq!(
            write(&colony, &short).err(),
            Some(Refusal::RosterDiverges { id: 4 })
        );
        let mut extra = needs();
        extra.insert(9, Needs::new());
        assert_eq!(
            write(&colony, &extra).err(),
            Some(Refusal::RosterDiverges { id: 9 })
        );
    }
}
