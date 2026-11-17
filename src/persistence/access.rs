//! The permission block: who owns the colony, who is in it, and what each rank
//! may do — in Java's five keys, not in a port layout.
//!
//! Java reference: `core/colony/permissions/Permissions.java:560-622`
//! (`savePermissions`) and `:330-376` (`loadPermissions`), `:228-262`
//! (`upgradePermissions`), `:119` (`permissionsVersion`), and
//! `api/colony/permissions/Action.java:9-43` for the names written in `flags`.
//!
//! **Three keys, three questions.** `ranks` is the ladder itself, `owners` is who
//! holds which rank, and `permissionMap` is what a rank may do. The port's
//! ladder is a closed enum of five (`src/colony/permissions.rs:36-43`), so
//! `ranks` is written for a Java reader and read only to spot an id the port does
//! not know; `owners` is what it actually loads; `permissionMap` is compared
//! against [`Rank::mask`], never applied, because the port computes the mask from
//! the fall-through switch instead of trusting a file.
//!
//! **The version gate is Java's, and it changes what a `rank` field means.**
//! `permissionVersion` 3 introduced integer rank ids; below it the field holds an
//! `OldRank` **name** (`Permissions.java:398-407`). A save from before that would
//! otherwise read every member as `OWNER` (an absent int is 0, `Rank.java:36-40`),
//! which is the one mistake here that hands out a colony.
//!
//! **`upgradePermissions` is ported as a comparison, not as a write.** Java adds
//! bits to the freshly loaded rank so an old save grows the permissions a newer
//! preset expects (`:228-262`); the port adds the same bits before comparing, so
//! a three-year-old save is not reported as "different" for having the defaults
//! of its own version. What it cannot do is *apply* an edited table, because the
//! masks are compiled in — [`Repair::RanksDiffer`] is what says so.

use mod_api::{Compound, Tag};

use crate::colony::permissions::{uuid, Action, Members, Rank};

use super::format::{
    KEY_FLAGS, KEY_ID, KEY_IS_COLONY_MANAGER, KEY_IS_HOSTILE, KEY_IS_INITIAL, KEY_NAME,
    KEY_OWNER_ID, KEY_OWNERS, KEY_PERMISSION_MAP, KEY_PERMISSION_VERSION, KEY_RANK, KEY_RANKS,
};
use super::{nbt, Refusal, Repair};

/// `Permissions.permissionsVersion` (`core/colony/permissions/Permissions.java:119`).
/// This is the version whose preset the five masks in `src/colony/permissions.rs`
/// already are; anything above it is a format this port does not know.
pub const PERMISSIONS_VERSION: i32 = 5;

/// The version from which `rank` is an integer id instead of an `OldRank` name
/// (`Permissions.java:398`).
pub const RANK_ID_VERSION: i32 = 3;

/// Java's action names, in `Action.java`'s declaration order — which is bit
/// order, and is the order `savePermissions` writes them in
/// (`Permissions.java:596-602` iterates `Action.values()`).
///
/// The table exists because the port's [`Action`] is an enum whose variants are
/// Rust names (`AccessHuts`) and whose discriminants are the bits
/// (`src/colony/permissions.rs:150-180`); a save holds Java's names
/// (`ACCESS_HUTS`), and the two must not be guessed from each other.
pub const ACTION_NAMES: &[(&str, Action)] = &[
    ("ACCESS_HUTS", Action::AccessHuts),
    ("PLACE_HUTS", Action::PlaceHuts),
    ("BREAK_HUTS", Action::BreakHuts),
    ("EDIT_PERMISSIONS", Action::EditPermissions),
    ("MANAGE_HUTS", Action::ManageHuts),
    ("RECEIVE_MESSAGES", Action::ReceiveMessages),
    ("USE_SCAN_TOOL", Action::UseScanTool),
    ("PLACE_BLOCKS", Action::PlaceBlocks),
    ("BREAK_BLOCKS", Action::BreakBlocks),
    ("TOSS_ITEM", Action::TossItem),
    ("PICKUP_ITEM", Action::PickupItem),
    ("FILL_BUCKET", Action::FillBucket),
    ("OPEN_CONTAINER", Action::OpenContainer),
    ("RIGHTCLICK_BLOCK", Action::RightClickBlock),
    ("RIGHTCLICK_ENTITY", Action::RightClickEntity),
    ("THROW_POTION", Action::ThrowPotion),
    ("SHOOT_ARROW", Action::ShootArrow),
    ("ATTACK_CITIZEN", Action::AttackCitizen),
    ("ATTACK_ENTITY", Action::AttackEntity),
    ("TELEPORT_TO_COLONY", Action::TeleportToColony),
    ("EXPLODE", Action::Explode),
    ("RALLY_GUARDS", Action::RallyGuards),
    ("HURT_CITIZEN", Action::HurtCitizen),
    ("HURT_VISITOR", Action::HurtVisitor),
    ("MAP_BORDER", Action::MapBorder),
    ("MAP_DEATHS", Action::MapDeaths),
    ("ACCESS_TOGGLEABLES", Action::AccessToggleables),
];

/// The owner of a member list, if it still has one.
pub fn owner(members: &Members) -> Option<u128> {
    members
        .iter()
        .find(|(_, rank)| *rank == Rank::Owner)
        .map(|(player, _)| player)
}

/// Writes the four keys into the colony's own compound, as
/// `savePermissions(compound)` does (`Permissions.java:560`).
pub fn write(data: &mut Compound, members: &Members) {
    let mut ranks = Vec::with_capacity(Rank::ALL.len());
    let mut map = Vec::with_capacity(Rank::ALL.len());
    for rank in Rank::ALL {
        ranks.push(Compound::from([
            (KEY_ID.into(), Tag::Int(rank.id())),
            (KEY_NAME.into(), Tag::String(rank.name().into())),
            // `loadRanks` builds the five stock ranks with `isInitial = true`
            // (`Permissions.java:168`).
            (KEY_IS_INITIAL.into(), nbt::boolean_tag(true)),
            (KEY_IS_COLONY_MANAGER.into(), nbt::boolean_tag(rank.is_manager())),
            (KEY_IS_HOSTILE.into(), nbt::boolean_tag(rank.is_hostile())),
        ]));
        let flags: Vec<Tag> = ACTION_NAMES
            .iter()
            .filter(|(_, action)| rank.allows(*action))
            .map(|(name, _)| Tag::String((*name).into()))
            .collect();
        map.push(Compound::from([
            (KEY_RANK.into(), Tag::Int(rank.id())),
            (KEY_FLAGS.into(), Tag::List(flags)),
        ]));
    }

    let owners: Vec<Compound> = members
        .iter()
        .map(|(player, rank)| {
            Compound::from([
                (KEY_ID.into(), Tag::String(uuid::to_text(player).as_str().into())),
                (KEY_RANK.into(), Tag::Int(rank.id())),
            ])
        })
        .collect();

    data.insert(KEY_RANKS.into(), nbt::compound_list(ranks));
    data.insert(KEY_OWNERS.into(), nbt::compound_list(owners));
    data.insert(KEY_PERMISSION_MAP.into(), nbt::compound_list(map));
    data.insert(
        KEY_PERMISSION_VERSION.into(),
        Tag::Int(PERMISSIONS_VERSION),
    );
    if let Some(player) = owner(members) {
        data.insert(
            KEY_OWNER_ID.into(),
            Tag::String(uuid::to_text(player).as_str().into()),
        );
    }
}

/// Reads the member list back.
///
/// Every member with a rank the port knows becomes a [`Members`] entry; the
/// `permissionMap` is only read to compare, and the owner is restored from
/// `ownerid` when the list lost it, exactly as `restoreOwnerIfNull` does
/// (`Permissions.java:453-469`).
pub fn read(data: &Compound, repairs: &mut Vec<Repair>) -> Result<Members, Refusal> {
    let version = nbt::int(data, KEY_PERMISSION_VERSION).unwrap_or(0);
    if version > PERMISSIONS_VERSION {
        return Err(Refusal::UnknownPermissionsVersion {
            found: version,
            known: PERMISSIONS_VERSION,
        });
    }

    // The ladder: read to know which ranks this save's table holds, which is
    // what `ranks.get(...)` answers in `loadPermissions` (`Permissions.java:358`).
    // Without the key Java calls `loadRanks()` and gets the five (`:334-380`).
    let mut known: Vec<Rank> = nbt::compounds(data, KEY_RANKS)
        .into_iter()
        .filter_map(|record| Rank::from_id(nbt::int(record, KEY_ID).unwrap_or(0)))
        .collect();
    if known.is_empty() {
        known.extend(Rank::ALL.iter().copied());
    }

    let mut members = Members::new();
    for (index, record) in nbt::compounds(data, KEY_OWNERS).into_iter().enumerate() {
        let Some(text) = nbt::text(record, KEY_ID) else {
            return Err(Refusal::MemberWithoutId { index });
        };
        let Some(player) = uuid::parse(&text) else {
            return Err(Refusal::MemberWithoutId { index });
        };
        let rank = member_rank(record, version, &text)?;
        if !known.contains(&rank) {
            // `Permissions.java:410-417` only adds a player whose rank the table
            // resolved; the rest of the colony loads without him.
            repairs.push(Repair::MemberDropped { id: text });
            continue;
        }
        if members.contains(player) {
            repairs.push(Repair::MemberDuplicated { id: text });
        }
        // Java's `players` is a map keyed by the uuid, so the last record for one
        // player wins (`Permissions.java:412`); the port keeps that order.
        members.set(player, rank);
    }

    for record in nbt::compounds(data, KEY_PERMISSION_MAP) {
        let id = nbt::int(record, KEY_RANK).unwrap_or(0);
        let Some(rank) = Rank::from_id(id) else {
            continue;
        };
        let mut saved = 0u64;
        if let Some(Tag::List(items)) = record.get(&KEY_FLAGS.into()) {
            for item in items {
                let Tag::String(name) = item else {
                    continue;
                };
                let name = name.to_string_lossy();
                if let Some((_, action)) = ACTION_NAMES.iter().find(|(known, _)| *known == name) {
                    saved |= action.flag();
                }
                // An unknown name is ignored, as `Action.valueOf`'s
                // `IllegalArgumentException` is caught at `:368-375`.
            }
        }
        // An old preset grows its bits on load (`:228-262`), so the comparison
        // is against the upgraded table and not against the raw one.
        if (upgrade(version, rank) | saved) != rank.mask() {
            repairs.push(Repair::RanksDiffer { rank });
        }
    }

    if owner(&members).is_none() {
        if let Some(text) = nbt::text(data, KEY_OWNER_ID).and_then(|text| uuid::parse(&text)) {
            members.set(text, Rank::Owner);
            repairs.push(Repair::OwnerRestored);
        }
    }
    if owner(&members).is_none() {
        repairs.push(Repair::NoOwner);
    }
    Ok(members)
}

/// One member's rank, with the version gate of `Permissions.java:398-407`.
fn member_rank(record: &Compound, version: i32, text: &str) -> Result<Rank, Refusal> {
    let Some(tag) = record.get(&KEY_RANK.into()) else {
        // Java reads `getInt`, i.e. 0, which is the Owner rank (`Rank.java:36-40`):
        // a member handed the colony by a missing field is not a save to trust.
        return Err(Refusal::MemberWithoutRank {
            id: text.to_owned(),
        });
    };
    if version < RANK_ID_VERSION {
        if let Tag::String(name) = tag {
            let name = name.to_string_lossy();
            return Rank::ALL
                .iter()
                .find(|rank| rank.name().eq_ignore_ascii_case(&name))
                .copied()
                .ok_or(Refusal::MemberWithoutRank {
                    id: text.to_owned(),
                });
        }
    }
    match nbt::int_of(tag) {
        Some(id) => Rank::from_id(id).ok_or(Refusal::UnknownRank { found: id }),
        None => Err(Refusal::MemberWithoutRank {
            id: text.to_owned(),
        }),
    }
}

/// The bits `upgradePermissions` would have added to a rank of this save's
/// version (`Permissions.java:228-262`), so a comparison is not a false alarm.
pub fn upgrade(version: i32, rank: Rank) -> u64 {
    let mut bits = 0u64;
    if version < 4 {
        if rank.is_hostile() {
            bits |= Action::HurtCitizen.flag() | Action::HurtVisitor.flag();
        }
        if rank.is_manager() {
            bits |= Action::MapDeaths.flag();
        }
        bits |= Action::MapBorder.flag();
    }
    if version < 5 && !rank.is_hostile() {
        bits |= Action::AccessToggleables.flag();
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210;
    const OFFICER: u128 = 0x0000_0000_0000_0000_0000_0000_0000_0002;
    const FRIEND: u128 = 3;

    fn members() -> Members {
        let mut members = Members::with_owner(OWNER);
        members.set(OFFICER, Rank::Officer);
        members.set(FRIEND, Rank::Friend);
        members
    }

    fn written() -> Compound {
        let mut data = Compound::new();
        write(&mut data, &members());
        data
    }

    fn flags_of(data: &Compound, id: i32) -> Vec<String> {
        for record in nbt::compounds(data, KEY_PERMISSION_MAP) {
            if nbt::int(record, KEY_RANK) == Some(id) {
                if let Some(Tag::List(items)) = record.get(&KEY_FLAGS.into()) {
                    return items
                        .iter()
                        .filter_map(|item| match item {
                            Tag::String(name) => Some(name.to_string_lossy()),
                            _ => None,
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }

    #[test]
    fn the_action_table_is_javas_twenty_seven_in_bit_order() {
        assert_eq!(ACTION_NAMES.len(), 27, "Action.java:9-43, four gaps kept empty");
        let mut bits: Vec<u32> = ACTION_NAMES.iter().map(|(_, action)| action.bit()).collect();
        let ordered = bits.clone();
        bits.sort_unstable();
        bits.dedup();
        assert_eq!(bits.len(), 27, "two actions share a bit");
        assert_eq!(bits, ordered, "the table is in bit order");
        for (name, action) in ACTION_NAMES {
            assert_eq!(
                action.flag(),
                1u64 << action.bit(),
                "{name} does not match its own bit"
            );
            assert!(name.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
        }
    }

    #[test]
    fn the_five_ranks_and_their_members_round_trip() {
        let data = written();
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("the block reads");
        assert!(repairs.is_empty(), "a clean block needs no repair");
        assert_eq!(read, members());
        assert_eq!(read.rank_of(OWNER), Rank::Owner);
        assert_eq!(owner(&read), Some(OWNER));
    }

    #[test]
    fn the_ranks_written_are_the_five_and_say_what_java_says() {
        let data = written();
        let ranks = nbt::compounds(&data, KEY_RANKS);
        assert_eq!(ranks.len(), 5);
        assert_eq!(nbt::int(ranks[0], KEY_ID), Some(0));
        assert_eq!(nbt::text(ranks[0], KEY_NAME).as_deref(), Some("Owner"));
        assert_eq!(nbt::boolean(ranks[0], KEY_IS_INITIAL), Some(true));
        assert_eq!(nbt::boolean(ranks[0], KEY_IS_COLONY_MANAGER), Some(true));
        assert_eq!(nbt::boolean(ranks[4], KEY_IS_HOSTILE), Some(true));
        assert_eq!(
            nbt::int(&data, KEY_PERMISSION_VERSION),
            Some(PERMISSIONS_VERSION)
        );
        assert_eq!(
            nbt::text(&data, KEY_OWNER_ID).as_deref(),
            Some(uuid::to_text(OWNER).as_str())
        );
    }

    #[test]
    fn the_flags_written_are_each_ranks_own_mask() {
        let data = written();
        for rank in Rank::ALL {
            let saved = flags_of(&data, rank.id());
            let expected: Vec<String> = ACTION_NAMES
                .iter()
                .filter(|(_, action)| rank.allows(*action))
                .map(|(name, _)| (*name).to_owned())
                .collect();
            assert_eq!(saved, expected, "{} is not its own mask", rank.name());
        }
        assert!(flags_of(&data, Rank::Hostile.id()).contains(&"HURT_CITIZEN".to_owned()));
        assert!(!flags_of(&data, Rank::Hostile.id()).contains(&"PLACE_HUTS".to_owned()));
    }

    #[test]
    fn a_member_without_a_rank_is_refused_because_java_reads_owner() {
        let mut data = written();
        let Some(Tag::List(items)) = data.get_mut(&KEY_OWNERS.into()) else {
            panic!("owners is a list")
        };
        let officer = items
            .iter_mut()
            .find(|item| match item {
                Tag::Compound(record) => nbt::int(record, KEY_RANK) == Some(Rank::Officer.id()),
                _ => false,
            })
            .expect("the officer is in the list");
        let Tag::Compound(record) = officer else {
            panic!("a member is a compound")
        };
        record.remove(&KEY_RANK.into());
        let text = uuid::to_text(OFFICER);
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::MemberWithoutRank { id: text })
        );
    }

    #[test]
    fn a_member_without_a_usable_id_is_refused_with_its_index() {
        let mut data = written();
        let Some(Tag::List(items)) = data.get_mut(&KEY_OWNERS.into()) else {
            panic!("owners is a list")
        };
        let Tag::Compound(record) = &mut items[0] else {
            panic!("a member is a compound")
        };
        record.insert(KEY_ID.into(), Tag::String("nobody".into()));
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::MemberWithoutId { index: 0 })
        );
    }

    #[test]
    fn an_unknown_rank_id_is_refused_rather_than_demoted() {
        let mut data = written();
        let Some(Tag::List(items)) = data.get_mut(&KEY_OWNERS.into()) else {
            panic!("owners is a list")
        };
        let Tag::Compound(record) = &mut items[1] else {
            panic!("a member is a compound")
        };
        record.insert(KEY_RANK.into(), Tag::Int(9));
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::UnknownRank { found: 9 })
        );
    }

    #[test]
    fn a_future_permissions_version_is_refused() {
        let mut data = written();
        data.insert(KEY_PERMISSION_VERSION.into(), Tag::Int(6));
        assert_eq!(
            read(&data, &mut Vec::new()).err(),
            Some(Refusal::UnknownPermissionsVersion {
                found: 6,
                known: PERMISSIONS_VERSION
            })
        );
    }

    #[test]
    fn an_old_save_names_its_ranks_with_the_enum_name() {
        let text = uuid::to_text(OWNER);
        let data = Compound::from([(
            KEY_OWNERS.into(),
            nbt::compound_list(vec![Compound::from([
                (KEY_ID.into(), Tag::String(text.as_str().into())),
                (KEY_RANK.into(), Tag::String("OWNER".into())),
            ])]),
        )]);
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("a pre-v3 save reads");
        assert_eq!(read.rank_of(OWNER), Rank::Owner);
        assert!(
            repairs.is_empty(),
            "an absent permissionMap is the upgraded preset, not a difference"
        );
        // And the same field read as an int is 0 — the Owner rank Java would
        // have handed out by accident, which is why the gate exists.
        assert_eq!(Rank::from_id(0), Some(Rank::Owner));
    }

    #[test]
    fn a_permission_map_that_disagrees_with_the_table_is_reported() {
        let mut data = written();
        let Some(Tag::List(items)) = data.get_mut(&KEY_PERMISSION_MAP.into()) else {
            panic!("permissionMap is a list")
        };
        let Tag::Compound(owner) = &mut items[0] else {
            panic!("a map entry is a compound")
        };
        let Some(Tag::List(flags)) = owner.get_mut(&KEY_FLAGS.into()) else {
            panic!("flags is a list")
        };
        let before = flags.len();
        flags.retain(|item| match item {
            Tag::String(name) => name.to_string_lossy() != "PLACE_HUTS",
            _ => true,
        });
        assert_eq!(flags.len(), before - 1, "the fixture holds PLACE_HUTS");
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("reads");
        assert_eq!(repairs, vec![Repair::RanksDiffer { rank: Rank::Owner }]);
        assert_eq!(
            read.rank_of(OWNER),
            Rank::Owner,
            "the port's mask is not edited by a save"
        );
    }

    #[test]
    fn a_table_of_an_older_version_is_upgraded_before_it_is_compared() {
        // A v3 save: no ACCESS_TOGGLEABLES anywhere, which is exactly what
        // `upgradePermissions` adds at `:248-253`.
        let mut data = Compound::new();
        write(&mut data, &members());
        data.insert(KEY_PERMISSION_VERSION.into(), Tag::Int(3));
        let mut repairs = Vec::new();
        let _ = read(&data, &mut repairs).expect("a v3 save reads");
        assert!(
            repairs.is_empty(),
            "a v3 table read with the v3 upgrade is the stock v5 table: {:?}",
            repairs
        );
    }

    #[test]
    fn the_owner_is_restored_from_ownerid_when_the_list_lost_him() {
        let mut data = written();
        data.insert(KEY_OWNERS.into(), nbt::compound_list(Vec::new()));
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("reads");
        assert_eq!(read.rank_of(OWNER), Rank::Owner);
        assert_eq!(repairs, vec![Repair::OwnerRestored]);
    }

    #[test]
    fn a_colony_with_no_owner_left_says_so() {
        let mut data = written();
        data.insert(KEY_OWNERS.into(), nbt::compound_list(Vec::new()));
        data.insert(KEY_OWNER_ID.into(), Tag::String("".into()));
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("reads");
        assert!(read.iter().next().is_none());
        assert_eq!(repairs, vec![Repair::NoOwner], "ids nobody can parse restore nobody");
    }

    #[test]
    fn a_duplicate_member_keeps_the_last_rank_and_says_so() {
        let mut data = written();
        let text = uuid::to_text(FRIEND);
        let Some(Tag::List(items)) = data.get_mut(&KEY_OWNERS.into()) else {
            panic!("owners is a list")
        };
        items.push(Tag::Compound(Compound::from([
            (KEY_ID.into(), Tag::String(text.as_str().into())),
            (KEY_RANK.into(), Tag::Int(Rank::Officer.id())),
        ])));
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("reads");
        assert_eq!(read.rank_of(FRIEND), Rank::Officer, "the last record wins");
        assert_eq!(repairs, vec![Repair::MemberDuplicated { id: text }]);
    }

    #[test]
    fn a_member_list_of_another_kind_reads_as_no_member_at_all() {
        let data = Compound::from([(KEY_OWNERS.into(), Tag::Int(3))]);
        let mut repairs = Vec::new();
        let read = read(&data, &mut repairs).expect("reads");
        assert!(read.iter().next().is_none());
        assert_eq!(repairs, vec![Repair::NoOwner], "Java's getList answers empty");
    }

    #[test]
    fn the_rank_table_of_another_version_is_upgraded_by_the_two_clauses_java_has() {
        for rank in Rank::ALL {
            assert_eq!(upgrade(PERMISSIONS_VERSION, rank), 0, "the v5 table needs nothing");
            let old = upgrade(0, rank);
            assert_eq!(
                old & Action::MapBorder.flag(),
                Action::MapBorder.flag(),
                "every rank got the border before v4 (:245)"
            );
            if rank.is_hostile() {
                assert_ne!(old & Action::HurtCitizen.flag(), 0, ":236-237");
                assert_eq!(old & Action::AccessToggleables.flag(), 0, "hostile, :248-253");
            } else {
                assert_ne!(old & Action::AccessToggleables.flag(), 0, ":248-253");
            }
            if rank.is_manager() {
                assert_ne!(old & Action::MapDeaths.flag(), 0, ":240-243");
            }
        }
    }
}
