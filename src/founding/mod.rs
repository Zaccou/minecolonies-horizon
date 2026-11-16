//! Founding a colony: from the town hall block to a colony that exists.
//!
//! Java reference: `core/network/messages/server/CreateColonyMessage.java:104-201`
//! (the method a town hall click runs), `core/colony/ColonyManager.java:91-120`
//! (`createColony`), `core/colony/ColonyList.java:43-71` (the id) and
//! `core/util/ChunkDataHelper.java:135-207` (the ground a colony claims).
//!
//! | File | Question |
//! | --- | --- |
//! | `rules` | how far from what: the four settings, and the maths they are read in |
//! | `terrain` | what the ground answers, and which cell of a box is in the way |
//! | `colonies` | the colonies that already exist, asked four questions |
//! | `found` | the checks in Java's order, then the writes in Java's order |
//! | `placement` | a hut after the founding: is its plot free, and does it stand in the colony |
//!
//! **The order is the deliverable.** [`found`] runs every check before it makes
//! one write, and takes the store of colonies as `&dyn Colonies` — read-only — so
//! the only thing it *can* produce is the ordered journal the caller applies
//! ([`Founded::journal`]). A refusal costs nothing by construction, not by care.
//!
//! **Pure data, pure rules**, like `colony` and `inventory`: no host call, no
//! clock, no thread, no file. The world is asked two questions, through [`Ground`]
//! and [`Colonies`], which is what lets a founding be a few structs in a test.
//!
//! **Not ported, and why**: the settlement covenant (`WindowTownHallColonyManage.java:55`,
//! `manual_en_us.json:2707-2712`) is a confirmation screen, not a rule; the
//! world-type gate (`EventHandler.java:711-715`) compares a *level type* and is on
//! by default (`ServerConfiguration.java:128`); the reach check
//! (`CreateColonyMessage.java:110`) is a network guard; the structure pack and
//! blueprint path (`ColonyManager.java:102`) are a pack's data; the package
//! manager (`:107-108`) has nobody to subscribe in a port with no view packets.
//!
//! **The one rule that is the port's own** is [`Refusal::PlotBlocked`], in
//! `placement`, because Java hands a site's clearing to the builder's job.

use crate::building;

mod colonies;
mod found;
mod placement;
mod rules;
mod terrain;

pub use colonies::{Colonies, ColonyAt, Registry};
pub use found::{default_name, found, Founded, Founder, Founding, Write, TOWN_HALL_LEVEL};
pub use placement::{hut_of, plan, standing_of, Placed};
pub use rules::{Rules, BLOCKS_PER_CHUNK};
pub use terrain::{first_obstacle, Air, Ground};

/// Why a founding, or the placement of a hut after it, was refused — named.
///
/// Every variant is a rule that exists in Java, with the place it is written,
/// except [`Self::PlotBlocked`], which says so. A variant is **not** a `lang` key:
/// those live in `assets/minecolonies/lang/`, which this module does not own.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// The town hall's cell does not hold its colony block entity, so the click did
    /// not land on a hut (`CreateColonyMessage.java:118-126`,
    /// `WARNING_TOWN_HALL_NO_TILE_ENTITY`). Java checks this before every distance
    /// rule, and it is the only "is the hut really here" question a founding asks:
    /// the block's own cell is *not* air at this point, it is the town hall.
    NoTownHall { at: [i32; 3] },
    /// Nearer the world spawn than the server allows
    /// (`CreateColonyMessage.java:148-155`).
    TooCloseToSpawn { distance: i64, allowed: i64 },
    /// Further from it than it allows (`:156-163`).
    TooFarFromSpawn { distance: i64, allowed: i64 },
    /// Another colony is nearer than the configured range
    /// (`ColonyManager.java:311-320`, `MESSAGE_COLONY_CREATE_DENIED_TOO_CLOSE`).
    TooCloseToColony {
        id: i32,
        distance: i64,
        /// `max(minColonyDistance, initialColonySize) << 4` (`:313`).
        required: i64,
    },
    /// A chunk the new colony would claim already belongs to another one
    /// (`ChunkDataHelper.java:200-203`, reached from `ColonyManager.java:321-323`).
    GroundClaimed { chunk: [i32; 2], id: i32 },
    /// The player already owns a colony, in any dimension
    /// (`CreateColonyMessage.java:171-173`, searched by `ColonyManager.java:547-564`).
    AlreadyOwns { id: i32 },
    /// The hut does not stand in a colony (`EventHandler.java:722-742`:
    /// `MESSAGE_WARNING_TOWN_HALL_NOT_PRESENT`, or `..._TOO_FAR_AWAY`).
    OutsideColony { at: [i32; 3] },
    /// A building already holds that position, so `addNewBuilding` would keep the
    /// old one (`RegisteredStructureManager.java:566`).
    PositionTaken { at: [i32; 3] },
    /// A block stands inside the box the building's schematic covers.
    ///
    /// **A rule of this port.** Java does not pre-check it: the builder's job
    /// clears the site while it builds, and the port has no builder yet.
    PlotBlocked { at: [i32; 3] },
    /// The colony's own placement rules refused it
    /// (`building::Standing::check_placement`; Java `AbstractBlockHut.java:305-338`,
    /// `BlockHutTownHall.java:219-234`).
    Building(building::Refusal),
    /// A saved building names a kind the registry table cannot read. Java logs
    /// "unable to create AbstractBuilding" (`RegisteredStructureManager.java:606-609`).
    UnknownKind { path: String },
}

impl Refusal {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoTownHall { .. } => "founding.refusal.no_town_hall",
            Self::TooCloseToSpawn { .. } => "founding.refusal.too_close_to_spawn",
            Self::TooFarFromSpawn { .. } => "founding.refusal.too_far_from_spawn",
            Self::TooCloseToColony { .. } => "founding.refusal.too_close_to_colony",
            Self::GroundClaimed { .. } => "founding.refusal.ground_claimed",
            Self::AlreadyOwns { .. } => "founding.refusal.already_owns",
            Self::OutsideColony { .. } => "founding.refusal.outside_colony",
            Self::PositionTaken { .. } => "founding.refusal.position_taken",
            Self::PlotBlocked { .. } => "founding.refusal.plot_blocked",
            Self::Building(_) => "founding.refusal.building",
            Self::UnknownKind { .. } => "founding.refusal.unknown_kind",
        }
    }
}

impl std::fmt::Display for Refusal {
    /// The name, and for [`Self::Building`] the rule that said no: two different
    /// building rules share that variant, and a line that hid which one would be
    /// unreadable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Building(inner) => write!(f, "{}: {inner}", self.name()),
            _ => f.write_str(self.name()),
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_its_own_name() {
        let at = [1, 2, 3];
        let all = [
            Refusal::NoTownHall { at },
            Refusal::TooCloseToSpawn { distance: 4, allowed: 500 },
            Refusal::TooFarFromSpawn { distance: 9_000, allowed: 30_000 },
            Refusal::TooCloseToColony { id: 2, distance: 100, required: 128 },
            Refusal::GroundClaimed { chunk: [6, 6], id: 2 },
            Refusal::AlreadyOwns { id: 2 },
            Refusal::OutsideColony { at },
            Refusal::PositionTaken { at },
            Refusal::PlotBlocked { at },
            Refusal::Building(building::Refusal::TownHallMissing),
            Refusal::UnknownKind { path: "minecolonies:castle".to_owned() },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("founding.refusal."));
            assert!(
                refusal.to_string().starts_with(refusal.name()),
                "Display starts with the name, so a log line can be searched by it"
            );
            for other in &all[index + 1..] {
                assert_ne!(
                    refusal.name(),
                    other.name(),
                    "two refusals share a name, so a log line cannot tell them apart"
                );
            }
        }
        assert_eq!(
            Refusal::Building(building::Refusal::TownHallMissing).to_string(),
            "founding.refusal.building: building.refusal.town_hall_missing",
            "the building rule that said no is kept in the line"
        );
    }
}
