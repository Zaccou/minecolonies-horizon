//! The founding itself: every check in Java's order, then every write in Java's
//! order.
//!
//! Java reference: `core/network/messages/server/CreateColonyMessage.java:104-201`
//! (what a town hall click runs), `core/colony/ColonyManager.java:91-120`
//! (`createColony`), `core/colony/ColonyList.java:43-71` (the id).
//!
//! | # | Check | `fichier:ligne` |
//! | --- | --- | --- |
//! | 1 | the cell holds the town hall's block entity | `CreateColonyMessage.java:118-126` |
//! | 2 | at least `minDistanceFromWorldSpawn` from the spawn | `:148-155` |
//! | 3 | at most `maxDistanceFromWorldSpawn` from it | `:156-163` |
//! | 4 | no colony nearer than `max(minColonyDistance, initialColonySize) << 4` | `ColonyManager.java:311-320` |
//! | 5 | the ground the colony would claim belongs to nobody | `:321-323`, `ChunkDataHelper.java:174-207` |
//! | 6 | the player owns no colony yet | `CreateColonyMessage.java:171-173` |
//!
//! | # | Write | `fichier:ligne` |
//! | --- | --- | --- |
//! | 1 | the id is handed out | `ColonyList.java:43-61` |
//! | 2 | the name is set | `ColonyManager.java:104` |
//! | 3 | the player becomes owner | `:105` |
//! | 4 | the ground is claimed | `:118` |
//! | 5 | the town hall becomes the colony's building | `CreateColonyMessage.java:176` |
//!
//! The pre-check screen orders the same rules differently — it asks the owner
//! question first and never reaches the distances for a player who already has a
//! colony (`GetColonyInfoMessage.java:80-124`). The server's order is kept here,
//! because it is the one that decides.
//!
//! **Why the writes are a list and not side effects.** [`found`] takes the colonies
//! as `&dyn Colonies` — it cannot register anything — and returns the writes as
//! [`Write`] values in order. A refusal therefore produces nothing at all: "no
//! write before a refusal" is the type, not a discipline.

use crate::building::{Building, Kind};
use crate::colony::{self, Colony};

use super::colonies::Colonies;
use super::rules::Rules;
use super::terrain::Ground;
use super::Refusal;

/// The level of the town hall the moment the colony exists: **0**, because the
/// hut is placed and not built (`AbstractSchematicProvider.java:64`). The level 1 a
/// new colony has is the *colony's* level, which is the town hall's building level
/// once the builder has finished — two different numbers.
pub const TOWN_HALL_LEVEL: i32 = 0;

/// The player founding the colony: who it belongs to, and what the default name is
/// made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Founder<'a> {
    /// The engine's player id, the `u128` a colony's member list keys on.
    pub player: u128,
    /// The name the game shows, which the default colony name is built from
    /// (`WindowTownHallColonyManage.java:54`; the string is `"%s's Colony"`,
    /// `manual_en_us.json:547`).
    pub name: &'a str,
}

impl<'a> Founder<'a> {
    pub fn new(player: u128, name: &'a str) -> Founder<'a> {
        Founder { player, name }
    }
}

/// Everything [`found`] is told: who is founding, where, and the two traits it may
/// ask. A hook fills those from the level; a test fills them with a list of blocked
/// cells and a `Vec` of colonies.
pub struct Founding<'a> {
    pub founder: Founder<'a>,
    /// The town hall's block position, which is also the colony's centre
    /// (`Colony.java:921`).
    pub at: [i32; 3],
    /// `dimensionId.location().toString()` (`Colony.java:917`).
    pub dimension: &'a str,
    /// The level's shared spawn: what the spawn distance is measured from
    /// (`CreateColonyMessage.java:147`, `world.getSharedSpawnPos()`).
    pub spawn: [i32; 3],
    pub ground: &'a dyn Ground,
    /// **Read-only**, so a founding cannot write into it.
    pub colonies: &'a dyn Colonies,
    /// The four distances: [`Rules::DEFAULT`] unless the caller changed them.
    pub rules: Rules,
}

impl<'a> Founding<'a> {
    pub fn new(
        founder: Founder<'a>,
        at: [i32; 3],
        dimension: &'a str,
        spawn: [i32; 3],
        ground: &'a dyn Ground,
        colonies: &'a dyn Colonies,
    ) -> Founding<'a> {
        Founding {
            founder,
            at,
            dimension,
            spawn,
            ground,
            colonies,
            rules: Rules::DEFAULT,
        }
    }

    /// The same founding under other distances: a server's configuration, or a test
    /// that wants a rule to bite.
    pub fn with_rules(mut self, rules: Rules) -> Founding<'a> {
        self.rules = rules;
        self
    }
}

/// One write a founding makes, in the order it makes it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub enum Write {
    /// `cap.createColony(w, pos)` hands out the id
    /// (`ColonyManager.java:101`, `ColonyList.java:43-61`).
    Registered { id: i32 },
    /// `colony.setName(colonyName)` (`ColonyManager.java:104`).
    Named { name: String },
    /// `colony.getPermissions().setOwner(player)` (`:105`, `Permissions.java:506`).
    OwnerSet { player: u128 },
    /// `ChunkDataHelper.claimColonyChunks(world, true, id, center)`
    /// (`ColonyManager.java:118`), which claims `chunks` chunks of radius around
    /// the centre (`ChunkDataHelper.java:135-139`).
    GroundClaimed { id: i32, chunks: i32 },
    /// `getServerBuildingManager().addNewBuilding(tileEntity, world)`
    /// (`CreateColonyMessage.java:176`): the town hall becomes the colony's first
    /// building (`RegisteredStructureManager.java:563-620`).
    TownHallAdded { at: [i32; 3] },
}

impl Write {
    /// A stable name for logs and tests, namespaced like the refusals.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Registered { .. } => "founding.write.registered",
            Self::Named { .. } => "founding.write.named",
            Self::OwnerSet { .. } => "founding.write.owner_set",
            Self::GroundClaimed { .. } => "founding.write.ground_claimed",
            Self::TownHallAdded { .. } => "founding.write.town_hall_added",
        }
    }
}

/// A colony that exists, and the ordered list of writes that made it.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Founded {
    /// As `colony::Colony` records it: level 1, the founder as owner, the town hall
    /// in its building list.
    pub colony: Colony,
    /// As `building::Building` holds it: kind `TownHall`, level 0, not built.
    pub town_hall: Building,
    /// Every write, in the order it has to be applied.
    pub writes: Vec<Write>,
}

impl Founded {
    /// The writes, in order.
    pub fn journal(&self) -> &[Write] {
        &self.writes
    }
}

/// Founds a colony, or says why not.
///
/// Every check runs before the first write, in the order of the table at the top of
/// this file. On `Ok`, [`Founded::journal`] is the whole plan: nothing has been
/// registered yet, and the caller inserts [`Founded::colony`] into its store.
pub fn found(founding: &Founding<'_>) -> Result<Founded, Refusal> {
    let at = founding.at;
    let rules = founding.rules;

    // 1. The town hall is where the click says it is: its block entity is the one
    //    `CreateColonyMessage.java:118-126` insists on, and anything else in that
    //    cell stops the founding before a distance is measured.
    if !founding.ground.holds_town_hall(at) {
        return Err(Refusal::NoTownHall { at });
    }

    // 2 and 3. The world spawn, in two dimensions only (`CreateColonyMessage.java:147-163`,
    //    `getDistanceSquared2D`).
    let from_spawn = Rules::distance_2d(at, founding.spawn);
    if from_spawn < i64::from(rules.min_from_spawn) {
        return Err(Refusal::TooCloseToSpawn {
            distance: from_spawn,
            allowed: i64::from(rules.min_from_spawn),
        });
    }
    if from_spawn > i64::from(rules.max_from_spawn) {
        return Err(Refusal::TooFarFromSpawn {
            distance: from_spawn,
            allowed: i64::from(rules.max_from_spawn),
        });
    }

    // 4. No colony nearer than the range (`ColonyManager.isFarEnoughFromColonies`,
    //    `:311-320`), in three dimensions and within this dimension only
    //    (`:493-503`).
    let range = rules.block_range();
    if let Some(closest) = founding.colonies.closest(founding.dimension, at) {
        let distance = Rules::distance(at, closest.center);
        if distance < range {
            return Err(Refusal::TooCloseToColony {
                id: closest.id,
                distance,
                required: range,
            });
        }
    }

    // 5. The ground the new colony would claim belongs to nobody (`:321-323` →
    //    `ChunkDataHelper.canClaimChunksInRange`, `:174-207`). The probe walks the
    //    box the colony would own, x then z ascending, so the refusal names the same
    //    chunk on every run.
    let radius = rules.claim_radius_chunks();
    let middle = Rules::chunk(at);
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let chunk = [middle[0] + dx, middle[1] + dz];
            if let Some(id) = founding.colonies.owns_chunk(founding.dimension, chunk, radius) {
                return Err(Refusal::GroundClaimed { chunk, id });
            }
        }
    }

    // 6. One colony per player, in any dimension (`CreateColonyMessage.java:171-173`).
    if let Some(owned) = founding.colonies.owned_by(founding.founder.player) {
        return Err(Refusal::AlreadyOwns { id: owned.id });
    }

    // Everything passed. The writes start here, in `ColonyManager.createColony`'s
    // order (`:101` to `:118`), then the town hall (`CreateColonyMessage.java:176`).
    let id = founding.colonies.next_id();
    let name = default_name(&founding.founder, founding.colonies);
    let writes = vec![
        Write::Registered { id },
        Write::Named { name: name.clone() },
        Write::OwnerSet {
            player: founding.founder.player,
        },
        Write::GroundClaimed { id, chunks: radius },
        Write::TownHallAdded { at },
    ];

    let mut colony = Colony::new(id, &name, founding.dimension, at, founding.founder.player);
    // A hut placed is a hut at level 0, not built
    // (`AbstractSchematicProvider.java:64`). The colony record keeps Java's own
    // `type` — a registry path — rather than the `Kind` the `building` module keys
    // on.
    colony
        .buildings
        .push(colony::Building::new(&Kind::TownHall.path(), at, TOWN_HALL_LEVEL));

    Ok(Founded {
        colony,
        town_hall: Building::new(Kind::TownHall, at),
        writes,
    })
}

/// The name a colony gets when the player types none: `"%s's Colony"` with the
/// player's name (`manual_en_us.json:547`, filled at
/// `WindowTownHallColonyManage.java:54`).
///
/// **The counter is a rule of this port.** Java stores whatever was typed and two
/// colonies may share a name; here a name is a player's only handle on a colony, so
/// the second `Simon's Colony` becomes `Simon's Colony 2`. If a thousand of them are
/// taken the colony's id is used instead, which is unique by construction
/// (`ColonyList.java:63-71`).
pub fn default_name(founder: &Founder<'_>, colonies: &dyn Colonies) -> String {
    let base = format!("{}'s Colony", founder.name);
    if !colonies.contains_name(&base) {
        return base;
    }
    for counter in 2..1_000 {
        let candidate = format!("{base} {counter}");
        if !colonies.contains_name(&candidate) {
            return candidate;
        }
    }
    format!("{base} {}", colonies.next_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Stage;
    use crate::colony::{Action, Rank, WorkType};

    use super::super::colonies::Registry;
    use super::super::terrain::Air;

    const PLAYER: u128 = 7;
    const OTHER: u128 = 9;
    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";
    /// The town hall: chunk [6, 6], about 141 blocks from the spawn at the origin.
    const AT: [i32; 3] = [100, 70, 100];
    const SPAWN: [i32; 3] = [0, 70, 0];
    /// Open sky, and no town hall: what a click on anything but a hut sees.
    const AIR: Air = Air;

    /// The world a founding runs in: the town hall's block entity where the click
    /// says, and every other cell empty.
    struct Hall;

    impl Ground for Hall {
        fn is_free(&self, _at: [i32; 3]) -> bool {
            true
        }

        fn holds_town_hall(&self, _at: [i32; 3]) -> bool {
            true
        }
    }

    /// As a `const`, so a `Founding` built anywhere can borrow it.
    const WITH_HALL: Hall = Hall;

    fn founding<'a>(ground: &'a dyn Ground, colonies: &'a dyn Colonies) -> Founding<'a> {
        Founding::new(Founder::new(PLAYER, "Simon"), AT, OVERWORLD, SPAWN, ground, colonies)
    }

    /// The same founding, with the level's spawn somewhere else.
    fn founding_at_spawn<'a>(spawn: [i32; 3], colonies: &'a Registry) -> Founding<'a> {
        Founding::new(Founder::new(PLAYER, "Simon"), AT, OVERWORLD, spawn, &WITH_HALL, colonies)
    }

    /// A registry holding one colony of another player.
    fn neighbour_at(place: [i32; 3], owner: u128, dimension: &str) -> Registry {
        let mut registry = Registry::new();
        let id = registry.next_id();
        registry.insert(Colony::new(id, "Neighbour", dimension, place, owner));
        registry
    }

    #[test]
    fn a_colony_founded_on_free_ground_has_its_founder_as_owner_and_its_town_hall() {
        let colonies = Registry::new();
        let founded = found(&founding(&WITH_HALL, &colonies)).expect("free ground, no neighbour");

        assert_eq!(founded.colony.id, 1);
        assert_eq!(founded.colony.name, "Simon's Colony");
        assert_eq!(founded.colony.level(), 1, "a new colony is a level one colony");
        assert_eq!(founded.colony.center, AT);
        assert_eq!(founded.colony.dimension, OVERWORLD);
        assert_eq!(founded.colony.members.rank_of(PLAYER), Rank::Owner);
        assert_eq!(founded.colony.may(PLAYER, Action::PlaceHuts), Ok(()));

        // The town hall is the colony's one building, and it is not built yet.
        assert_eq!(founded.colony.buildings.len(), 1);
        let recorded = &founded.colony.buildings[0];
        assert_eq!(recorded.kind, "minecolonies:townhall");
        assert_eq!((recorded.at, recorded.level), (AT, TOWN_HALL_LEVEL));
        assert_eq!(founded.town_hall.kind(), Kind::TownHall);
        assert_eq!(founded.town_hall.at(), AT);
        assert_eq!(founded.town_hall.stage(), Stage::Unbuilt);
    }

    #[test]
    fn the_journal_lists_every_write_in_the_order_java_makes_them() {
        let colonies = Registry::new();
        let founded = found(&founding(&WITH_HALL, &colonies)).expect("founded");
        let expected = vec![
            Write::Registered { id: 1 },
            Write::Named { name: "Simon's Colony".to_owned() },
            Write::OwnerSet { player: PLAYER },
            Write::GroundClaimed { id: 1, chunks: 4 },
            Write::TownHallAdded { at: AT },
        ];
        assert_eq!(founded.journal(), expected.as_slice());
        for write in founded.journal() {
            assert!(write.name().starts_with("founding.write."));
        }
    }

    #[test]
    fn the_work_order_counter_opens_on_the_town_hall() {
        let colonies = Registry::new();
        let mut founded = found(&founding(&WITH_HALL, &colonies)).expect("founded");
        assert_eq!(founded.colony.orders.len(), 0, "founding orders nothing");
        assert_eq!(
            founded.colony.place_order(&Kind::TownHall.path(), AT, WorkType::Build, 1),
            1,
            "the first order of a new colony is number one"
        );
    }

    #[test]
    fn a_click_that_did_not_land_on_a_town_hall_refuses_the_founding_and_writes_nothing() {
        let colonies = Registry::new();
        assert_eq!(
            found(&founding(&AIR, &colonies)),
            Err(Refusal::NoTownHall { at: AT }),
            "open air holds no colony block entity"
        );
        assert!(colonies.is_empty(), "a refusal registers nothing");
    }

    #[test]
    fn a_place_outside_the_spawn_distances_is_refused() {
        let colonies = Registry::new();
        let too_close =
            founding_at_spawn([AT[0] + 100, AT[1], AT[2]], &colonies).with_rules(Rules {
                min_from_spawn: 500,
                ..Rules::DEFAULT
            });
        assert_eq!(
            found(&too_close),
            Err(Refusal::TooCloseToSpawn {
                distance: 100,
                allowed: 500
            })
        );

        let too_far =
            founding_at_spawn([AT[0] + 5_000, AT[1], AT[2]], &colonies).with_rules(Rules {
                max_from_spawn: 1_000,
                ..Rules::DEFAULT
            });
        assert_eq!(
            found(&too_far),
            Err(Refusal::TooFarFromSpawn {
                distance: 5_000,
                allowed: 1_000
            })
        );
        assert!(colonies.is_empty());
    }

    #[test]
    fn a_colony_founded_next_to_another_one_is_refused() {
        let colonies = neighbour_at([AT[0] + 100, AT[1], AT[2]], OTHER, OVERWORLD);
        assert_eq!(
            found(&founding(&WITH_HALL, &colonies)),
            Err(Refusal::TooCloseToColony { id: 1, distance: 100, required: 128 }),
            "8 chunks of 16 blocks, and 100 is nearer than that"
        );
        assert_eq!(colonies.len(), 1, "the neighbour is still the only colony");
    }

    #[test]
    fn ground_another_colony_claims_is_refused() {
        // Far enough in blocks once the two settings are lowered, but its claim
        // reaches this chunk: 70 blocks apart, a range of 64 and a claim of 4 chunks.
        let colonies = neighbour_at([AT[0] + 70, AT[1], AT[2]], OTHER, OVERWORLD);
        let close = founding(&WITH_HALL, &colonies).with_rules(Rules {
            min_colony_distance: 1,
            ..Rules::DEFAULT
        });
        assert_eq!(
            found(&close),
            Err(Refusal::GroundClaimed { chunk: [6, 2], id: 1 }),
            "the first claimed chunk of the box, x then z ascending"
        );
    }

    #[test]
    fn a_player_who_already_owns_a_colony_anywhere_cannot_found_a_second_one() {
        let colonies = neighbour_at([AT[0] + 5_000, AT[1], AT[2]], PLAYER, NETHER);
        assert_eq!(
            found(&founding(&WITH_HALL, &colonies)),
            Err(Refusal::AlreadyOwns { id: 1 }),
            "another dimension does not make a second colony allowed"
        );
        assert_eq!(colonies.len(), 1);
    }

    #[test]
    fn the_default_name_takes_a_counter_when_the_first_is_taken() {
        let colonies = Registry::new();
        assert_eq!(
            default_name(&Founder::new(PLAYER, "Simon"), &colonies),
            "Simon's Colony",
            "the format of manual_en_us.json:547"
        );

        let mut colonies = colonies;
        let id = colonies.next_id();
        colonies.insert(Colony::new(
            id,
            "Simon's Colony",
            OVERWORLD,
            [AT[0] + 5_000, AT[1], AT[2]],
            OTHER,
        ));
        let founded = found(&founding(&WITH_HALL, &colonies)).expect("far from the other one");
        assert_eq!(founded.colony.name, "Simon's Colony 2");
        assert_eq!(
            founded.writes[1],
            Write::Named { name: "Simon's Colony 2".to_owned() },
            "the journal carries the name that was chosen"
        );
    }
}
