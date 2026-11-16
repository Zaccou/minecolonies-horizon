//! Who may do what in a colony, and the refusal that says why not.
//!
//! Java reference: `core/colony/permissions/Permissions.java` (1 091 lines) over
//! `api/colony/permissions` (`Action`, `Rank`, `OldRank`).
//!
//! **The shape is Java's.** A rank carries a `long` bitmask (`Rank.java:35`,
//! `:145-171`), and `loadRanks` builds the five masks with a **fall-through
//! switch**: `case OFFICER:` has no `break`, so it falls into `FRIEND:`, which
//! falls into `NEUTRAL:` (`Permissions.java:173-219`). `HOSTILE` breaks and
//! inherits nothing. [`Rank::mask`] follows that switch instead of copying its
//! result, so the table cannot drift without a test failing.
//!
//! `Action` numbers are Java's (`Action.java:9-43`, `0x1L << bit`, `:55-58`), so
//! a mask written by MineColonies keeps its meaning.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;

/// A rank (`api/colony/permissions/OldRank.java:6-13`). The order is the Java
/// ordinal order, which is the id saved in NBT (`Permissions.java:171`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Rank {
    Owner,
    Officer,
    Friend,
    Neutral,
    Hostile,
}

impl Rank {
    /// The five stock ranks, in ladder order. MineColonies lets a colony add
    /// ranks (`Permissions.java:1054-1076`); the port does not.
    pub const ALL: [Rank; 5] = [
        Rank::Owner,
        Rank::Officer,
        Rank::Friend,
        Rank::Neutral,
        Rank::Hostile,
    ];

    /// The id this rank is saved under (`Permissions.java:171`).
    pub fn id(self) -> i32 {
        self as i32
    }

    /// The rank an id names. An unknown id is `None`, not a silent `Neutral`.
    pub fn from_id(id: i32) -> Option<Rank> {
        Rank::ALL.into_iter().find(|rank| rank.id() == id)
    }

    /// The name Java writes for a rank: the enum name, first letter capitalised
    /// (`Permissions.java:169-170`). Data, not a screen string.
    pub fn name(self) -> &'static str {
        match self {
            Rank::Owner => "Owner",
            Rank::Officer => "Officer",
            Rank::Friend => "Friend",
            Rank::Neutral => "Neutral",
            Rank::Hostile => "Hostile",
        }
    }

    /// `Rank.isColonyManager`, set in the `case OFFICER:` block
    /// (`Permissions.java:191`), which `OWNER` reaches by falling through.
    pub fn is_manager(self) -> bool {
        matches!(self, Rank::Owner | Rank::Officer)
    }

    /// `Rank.isHostile` (`Permissions.java:214`, `Rank.java:28-30`): the rank
    /// the "free to interact" escape does not cover
    /// (`ColonyPermissionEventHandler.java:356`).
    pub fn is_hostile(self) -> bool {
        self == Rank::Hostile
    }

    /// The `long` saved as `permissionData`: the switch of
    /// `Permissions.java:173-219`, rank by rank, each adding its bits and
    /// falling through to the one below.
    pub fn mask(self) -> u64 {
        let own = own_bits(self);
        match self {
            Rank::Owner => own | Rank::Officer.mask(),
            Rank::Officer => own | Rank::Friend.mask(),
            Rank::Friend => own | Rank::Neutral.mask(),
            // `case HOSTILE:` and `case NEUTRAL:` both end in `break`.
            Rank::Neutral | Rank::Hostile => own,
        }
    }

    /// Whether this rank carries `action` in its default mask.
    pub fn allows(self, action: Action) -> bool {
        self.mask() & action.flag() != 0
    }
}

/// The bits a rank adds before the fall-through: `Permissions.java:175-215`.
fn own_bits(rank: Rank) -> u64 {
    use Action::*;
    let actions: &[Action] = match rank {
        Rank::Owner => &[EditPermissions, MapBorder, MapDeaths],
        Rank::Officer => &[
            PlaceHuts,
            BreakHuts,
            ManageHuts,
            ReceiveMessages,
            PlaceBlocks,
            BreakBlocks,
            FillBucket,
            OpenContainer,
            RallyGuards,
            MapBorder,
            MapDeaths,
        ],
        Rank::Friend => &[
            AccessHuts,
            UseScanTool,
            TossItem,
            PickupItem,
            RightClickBlock,
            RightClickEntity,
            ThrowPotion,
            ShootArrow,
            AttackCitizen,
            AttackEntity,
            TeleportToColony,
            AccessToggleables,
            MapBorder,
        ],
        Rank::Neutral => &[AccessToggleables, MapBorder],
        Rank::Hostile => &[HurtCitizen, HurtVisitor, MapBorder],
    };
    actions.iter().fold(0u64, |mask, action| mask | action.flag())
}

/// One thing a player can be allowed to do (`Action.java:6-43`).
///
/// Each variant's discriminant **is** its bit, so the bit is documented next to
/// the name and `as u32` reads it. Bits 1, 20, 23 and 24 are gaps Java keeps for
/// actions it removed (`:11`, `:20`, `:23-24`): reusing one would remap a saved
/// mask. Placing a hut, breaking one and recruiting are `PlaceHuts`,
/// `BreakHuts` and `ManageHuts` (`HireMercenaryMessage.java:29-32`: a hire is a
/// GUI button, and every GUI button is `MANAGE_HUTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Action {
    AccessHuts = 0,
    PlaceHuts = 2,
    BreakHuts = 3,
    EditPermissions = 4,
    ManageHuts = 5,
    ReceiveMessages = 6,
    UseScanTool = 7,
    PlaceBlocks = 8,
    BreakBlocks = 9,
    TossItem = 10,
    PickupItem = 11,
    FillBucket = 12,
    OpenContainer = 13,
    RightClickBlock = 14,
    RightClickEntity = 15,
    ThrowPotion = 16,
    ShootArrow = 17,
    AttackCitizen = 18,
    AttackEntity = 19,
    TeleportToColony = 21,
    /// No stock rank holds it (`Permissions.java:173-219`), as in Java: the bit
    /// exists and the default table leaves it off.
    Explode = 22,
    RallyGuards = 25,
    HurtCitizen = 26,
    HurtVisitor = 27,
    MapBorder = 28,
    MapDeaths = 29,
    AccessToggleables = 30,
}

impl Action {
    /// The bit this action occupies (`Action.java:9-43`).
    pub fn bit(self) -> u32 {
        self as u32
    }

    /// `0x1L << bit` (`Action.java:57`).
    pub fn flag(self) -> u64 {
        1u64 << self.bit()
    }
}

/// Why a player was refused: the rule that fired, never a bare `false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Refusal {
    /// The rank does not carry the bit. The rank is the **effective** one, so a
    /// stranger is refused as `Neutral` (`Permissions.java:789-792`).
    LacksAction { rank: Rank, action: Action },
    /// The target is the owner, and no one demotes or expels the owner
    /// (`PermissionsMessage.java:485`, `Permissions.java:876-885`).
    TargetIsOwner,
    /// Expelling a member who is not hostile needs a colony manager on top of
    /// the bit (`PermissionsMessage.java:562-565`). Unreachable with the five
    /// stock ranks — the only rank with `EditPermissions` is `Owner`, which is a
    /// manager — but a colony that adds a rank (`Permissions.java:1054-1076`)
    /// reaches it.
    NotManager { rank: Rank },
    /// The target is not in the colony. Java dereferences the lookup without
    /// checking (`PermissionsMessage.java:560-562`); the port names the case.
    NotAMember,
}

/// Whether expelling `target` needs a manager as well as the bit:
/// `!target.isHostile()` (`PermissionsMessage.java:562-565`).
pub fn expulsion_needs_manager(target: Rank) -> bool {
    !target.is_hostile()
}

/// The players of a colony and the rank each holds. A player is a `u128`, which
/// is what the API takes for a player id
/// (`crates/horizon-mod-vocab/src/host/players.rs:18`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Members {
    ranks: BTreeMap<u128, Rank>,
}

impl Members {
    /// An empty colony, which is not a valid one: [`Members::with_owner`] is
    /// what creation uses (`Permissions.java:506`).
    pub fn new() -> Members {
        Members {
            ranks: BTreeMap::new(),
        }
    }

    /// A colony whose founder is its owner.
    pub fn with_owner(owner: u128) -> Members {
        let mut members = Members::new();
        members.set(owner, Rank::Owner);
        members
    }

    /// The rank a player holds. Someone never added is `Neutral`
    /// (`Permissions.java:789-792`).
    pub fn rank_of(&self, player: u128) -> Rank {
        self.ranks.get(&player).copied().unwrap_or(Rank::Neutral)
    }

    /// Whether the colony has this player in its list at all.
    pub fn contains(&self, player: u128) -> bool {
        self.ranks.contains_key(&player)
    }

    /// Writes a rank. No guard: the guards live where Java puts them, in
    /// [`Members::may_invite`] and [`Members::remove`].
    pub fn set(&mut self, player: u128, rank: Rank) {
        self.ranks.insert(player, rank);
    }

    /// Takes a player out and returns the rank he had. The owner is refused:
    /// `removePlayer` skips it before removing anything
    /// (`Permissions.java:876-885`), which is why a colony is abandoned rather
    /// than left.
    pub fn remove(&mut self, player: u128) -> Result<Rank, Refusal> {
        let rank = self.rank_of(player);
        if !self.contains(player) {
            return Err(Refusal::NotAMember);
        }
        if rank == Rank::Owner {
            return Err(Refusal::TargetIsOwner);
        }
        self.ranks.remove(&player);
        Ok(rank)
    }

    /// Every member, in uuid order.
    pub fn iter(&self) -> impl Iterator<Item = (u128, Rank)> + '_ {
        self.ranks.iter().map(|(player, rank)| (*player, *rank))
    }

    /// **The resolution.** May `player` do `action` here? `Ok(())` is yes, and
    /// the error is the named refusal (`hasPermission(Rank, Action)`,
    /// `Permissions.java:639-650`).
    pub fn may(&self, player: u128, action: Action) -> Result<(), Refusal> {
        let rank = self.rank_of(player);
        if rank.allows(action) {
            Ok(())
        } else {
            Err(Refusal::LacksAction { rank, action })
        }
    }

    /// May `actor` give `target` a rank? The `EditPermissions` bit, and the
    /// owner rank is never handed out (`PermissionsMessage.java:485-488`).
    pub fn may_invite(&self, actor: u128, target: Rank) -> Result<(), Refusal> {
        self.may(actor, Action::EditPermissions)?;
        if target == Rank::Owner {
            return Err(Refusal::TargetIsOwner);
        }
        Ok(())
    }

    /// May `actor` take `target` out? The three clauses of
    /// `PermissionsMessage.RemovePlayer` (`:561-568`): a member may always
    /// leave, a hostile player goes on the bit alone, anyone else needs the bit
    /// **and** a manager — plus the owner is never removed
    /// (`Permissions.java:876-885`).
    pub fn may_expel(&self, actor: u128, target: u128) -> Result<(), Refusal> {
        if !self.contains(target) {
            return Err(Refusal::NotAMember);
        }
        let target_rank = self.rank_of(target);
        if target_rank == Rank::Owner {
            return Err(Refusal::TargetIsOwner);
        }
        if actor == target {
            return Ok(());
        }
        self.may(actor, Action::EditPermissions)?;
        if !expulsion_needs_manager(target_rank) {
            return Ok(());
        }
        let actor_rank = self.rank_of(actor);
        if actor_rank.is_manager() {
            Ok(())
        } else {
            Err(Refusal::NotManager { rank: actor_rank })
        }
    }

    /// The list as a compound: `{id, rank}` per member, the id written as Java
    /// writes a UUID, `UUID.toString()` (`Permissions.java:616`).
    pub fn to_compound(&self) -> Compound {
        let records = self
            .iter()
            .map(|(player, rank)| {
                // `Tag::String` holds a `JavaString` and only `From<&str>`
                // exists for it (`crates/horizon-core/src/nbt.rs:8-12`).
                let id = uuid::to_text(player);
                Compound::from([
                    (uuid::KEY_ID.into(), Tag::String(id.as_str().into())),
                    (uuid::KEY_RANK.into(), Tag::Int(rank.id())),
                ])
            })
            .collect();
        Compound::from([(uuid::KEY_MEMBERS.into(), nbt::compound_list(records))])
    }

    /// Reads a list. A record with no uuid, or with a rank id the port does not
    /// know, is skipped rather than guessed.
    pub fn from_compound(data: &Compound) -> Members {
        let mut members = Members::new();
        for record in nbt::compounds(data, uuid::KEY_MEMBERS) {
            let player = nbt::text(record, uuid::KEY_ID).and_then(|id| uuid::parse(&id));
            let rank = nbt::int(record, uuid::KEY_RANK).and_then(Rank::from_id);
            if let (Some(player), Some(rank)) = (player, rank) {
                members.set(player, rank);
            }
        }
        members
    }
}

/// A player id as MineColonies saves it: the string `UUID.toString()` gives,
/// read back with `UUID.fromString` (`Permissions.java:428`, `:616`).
pub mod uuid {
    pub const KEY_MEMBERS: &str = "members";
    pub const KEY_ID: &str = "id";
    pub const KEY_RANK: &str = "rank";

    /// `u128` to Java's textual UUID: 8-4-4-4-12 lowercase hex digits.
    pub fn to_text(id: u128) -> String {
        let hex = format!("{id:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }

    /// Java's textual UUID back to a `u128`. Dashes are allowed anywhere, any
    /// other character refuses the whole string: a half-understood id is not an
    /// id.
    pub fn parse(text: &str) -> Option<u128> {
        let mut digits = String::with_capacity(32);
        for character in text.chars() {
            if character == '-' {
                continue;
            }
            if !character.is_ascii_hexdigit() {
                return None;
            }
            digits.push(character);
        }
        if digits.len() != 32 {
            return None;
        }
        u128::from_str_radix(&digits, 16).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 1;
    const OFFICER: u128 = 2;
    const FRIEND: u128 = 3;
    const STRANGER: u128 = 4;
    const ENEMY: u128 = 5;

    fn lacks(rank: Rank, action: Action) -> Result<(), Refusal> {
        Err(Refusal::LacksAction { rank, action })
    }

    fn colony() -> Members {
        let mut members = Members::with_owner(OWNER);
        members.set(OFFICER, Rank::Officer);
        members.set(FRIEND, Rank::Friend);
        members.set(ENEMY, Rank::Hostile);
        members
    }

    #[test]
    fn the_ladder_is_cumulative_and_hostile_inherits_nothing() {
        assert_eq!(Rank::Owner.mask() & Rank::Officer.mask(), Rank::Officer.mask());
        assert_eq!(Rank::Officer.mask() & Rank::Friend.mask(), Rank::Friend.mask());
        assert_eq!(Rank::Friend.mask() & Rank::Neutral.mask(), Rank::Neutral.mask());
        // `MapBorder` figure dans la liste de CHAQUE rang
        // (`Permissions.java:175-215`), donc l'intersection d'Hostile et de Friend est
        // exactement ce bit : un hostile voit la carte, il n'hérite de rien d'autre.
        assert_eq!(
            Rank::Hostile.mask() & Rank::Friend.mask(),
            Action::MapBorder.flag()
        );
        assert_eq!(
            Rank::Hostile.mask(),
            Action::HurtCitizen.flag() | Action::HurtVisitor.flag() | Action::MapBorder.flag()
        );
        for rank in [Rank::Owner, Rank::Officer, Rank::Friend, Rank::Neutral] {
            assert_eq!(rank.mask() & Action::Explode.flag(), 0, "no rank explodes");
            assert_eq!(rank.mask() & Action::HurtCitizen.flag(), 0, "hurting is hostile's");
        }
    }

    #[test]
    fn a_stranger_is_neutral_and_an_unknown_rank_id_is_none() {
        let members = colony();
        assert_eq!(members.rank_of(STRANGER), Rank::Neutral);
        assert!(!members.contains(STRANGER));
        for rank in Rank::ALL {
            assert_eq!(Rank::from_id(rank.id()), Some(rank));
            assert!(!rank.name().is_empty());
        }
        assert_eq!(Rank::from_id(5), None);
        assert_eq!((Rank::Owner.id(), Rank::Hostile.id()), (0, 4));
    }

    #[test]
    fn an_officer_may_place_break_and_recruit_and_a_friend_may_not() {
        let members = colony();
        for action in [Action::PlaceHuts, Action::BreakHuts, Action::ManageHuts] {
            assert_eq!(members.may(OWNER, action), Ok(()), "owner, {action:?}");
            assert_eq!(members.may(OFFICER, action), Ok(()), "officer, {action:?}");
            assert_eq!(members.may(FRIEND, action), lacks(Rank::Friend, action));
        }
    }

    #[test]
    fn only_the_owner_may_edit_permissions_and_never_hands_out_ownership() {
        let members = colony();
        assert_eq!(members.may(OWNER, Action::EditPermissions), Ok(()));
        assert_eq!(members.may_invite(OWNER, Rank::Friend), Ok(()));
        assert_eq!(
            members.may_invite(OFFICER, Rank::Friend),
            lacks(Rank::Officer, Action::EditPermissions)
        );
        assert_eq!(
            members.may_invite(OWNER, Rank::Owner),
            Err(Refusal::TargetIsOwner)
        );
    }

    #[test]
    fn the_owner_is_never_expelled_nor_demoted_not_even_by_himself() {
        let mut members = colony();
        assert_eq!(members.may_expel(OWNER, OWNER), Err(Refusal::TargetIsOwner));
        assert_eq!(members.may_expel(OFFICER, OWNER), Err(Refusal::TargetIsOwner));
        assert_eq!(members.remove(OWNER), Err(Refusal::TargetIsOwner));
        assert!(members.contains(OWNER));
    }

    #[test]
    fn a_member_may_leave_but_expelling_someone_else_needs_the_edit_bit() {
        let mut members = colony();
        assert_eq!(members.may_expel(FRIEND, FRIEND), Ok(()));
        assert_eq!(members.may_expel(STRANGER, STRANGER), Err(Refusal::NotAMember));
        assert_eq!(members.may_expel(OWNER, FRIEND), Ok(()));
        assert_eq!(members.may_expel(OWNER, STRANGER), Err(Refusal::NotAMember));
        assert_eq!(
            members.may_expel(FRIEND, OFFICER),
            lacks(Rank::Friend, Action::EditPermissions)
        );
        assert_eq!(members.remove(FRIEND), Ok(Rank::Friend));
        assert!(!members.contains(FRIEND));
    }

    #[test]
    fn a_hostile_player_goes_without_the_manager_clause_and_a_friend_does_not() {
        assert!(expulsion_needs_manager(Rank::Friend));
        assert!(!expulsion_needs_manager(Rank::Hostile));
        let members = colony();
        assert_eq!(members.may_expel(OWNER, ENEMY), Ok(()));
    }

    #[test]
    fn a_uuid_survives_java_textual_form_and_a_half_id_does_not() {
        let id = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;
        let text = uuid::to_text(id);
        assert_eq!(text, "01234567-89ab-cdef-fedc-ba9876543210");
        assert_eq!(uuid::parse(&text), Some(id));
        assert_eq!(uuid::parse("0123"), None);
        assert_eq!(uuid::parse("zz234567-89ab-cdef-fedc-ba9876543210"), None);
    }

    #[test]
    fn the_member_list_round_trips_and_skips_what_it_cannot_read() {
        let members = colony();
        assert_eq!(Members::from_compound(&members.to_compound()), members);
        let id = uuid::to_text(OWNER);
        let broken = Compound::from([(
            uuid::KEY_MEMBERS.into(),
            nbt::compound_list(vec![
                Compound::from([(uuid::KEY_ID.into(), Tag::String("nope".into()))]),
                Compound::from([
                    (uuid::KEY_ID.into(), Tag::String(id.as_str().into())),
                    (uuid::KEY_RANK.into(), Tag::Int(99)),
                ]),
            ]),
        )]);
        assert_eq!(Members::from_compound(&broken), Members::new());
    }
}
