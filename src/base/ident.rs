//! The three interfaces every module ends up needing, cut to what the port uses.
//!
//! Java exposes far more than this — `IColonyRelated` (26 lines) and `ILocation`
//! (37) are small, but `ICitizen`, `IBuilding` and `IColony` are not, and none of
//! the three carries anything a wasm mod can call. What survives the cut is the
//! part that is *data*: a name for a dimension, a position in it, the id of the
//! colony a thing belongs to, and the id by which a thing is known.
//!
//! | Java | Here | Why the shape changed |
//! | --- | --- | --- |
//! | `ILocation` (`api/colony/requestsystem/location/ILocation.java:11-37`) | [`Positioned`] | `ResourceKey<Level>` becomes the dimension's name |
//! | `IColonyRelated` (`api/colony/IColonyRelated.java:6-26`) | [`InColony`] | `IColony` becomes its `i32` id, and null becomes `None` |
//! | `ICitizen.getId()` (`api/colony/ICitizen.java:15`) | [`Identified`] | unchanged |
//!
//! **Reachability is one comparison, and no distance.** Java's `ILocation`
//! answers `isReachableFromLocation` by comparing the dimension alone
//! (`core/colony/requestsystem/locations/StaticLocation.java:66-70`): two huts
//! two thousand blocks apart in the same dimension are "reachable", because the
//! deliveryman may still walk there. [`is_reachable`] keeps exactly that rule —
//! a threshold here would be a rule Java does not have.
//!
//! **The entity variant is the exception, and it maps to `Option`.** A location
//! that tracks an entity is unreachable once that entity is gone
//! (`EntityLocation.java:113-118`), which in Rust is a `None` at the caller, not
//! a second boolean here.

/// A dimension's registry name: `"minecraft:overworld"`, `"minecraft:the_nether"`.
///
/// Java compares `ResourceKey<Level>` by identity (`StaticLocation.java:86`);
/// the port compares the full name, which is what `colony::Colony::dimension`
/// already stores (`src/colony/colonie.rs:160`, `Colony.java:917`) and what the
/// wire carries. Two spellings of one dimension (`minecraft:overworld` and
/// `overworld`) are therefore **not** equal here: nothing in the port ever
/// produces the short one, and a silent normalisation would hide a real
/// mismatch between a save and a command argument.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Dimension(String);

impl Dimension {
    /// Names a dimension. No prefixing, no case folding — see the type's docs.
    pub fn new(name: &str) -> Dimension {
        Dimension(name.to_owned())
    }

    /// The name, as it appears in a save and on the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Dimension {
    fn from(name: &str) -> Dimension {
        Dimension::new(name)
    }
}

impl From<String> for Dimension {
    fn from(name: String) -> Dimension {
        Dimension(name)
    }
}

/// Anything that can say where it is: Java's `ILocation`
/// (`api/colony/requestsystem/location/ILocation.java:20`, `:28`).
///
/// Two methods, because those are the two an `ILocation` has to answer; the third
/// (`isReachableFromLocation`, `:36`) is [`is_reachable`], free, since it reads
/// nothing private.
pub trait Positioned {
    /// The dimension's registry name.
    fn dimension(&self) -> &str;

    /// The block position in that dimension.
    fn position(&self) -> [i32; 3];
}

/// A fixed place in a dimension: Java's `StaticLocation`
/// (`core/colony/requestsystem/locations/StaticLocation.java:20-34`).
///
/// This is what a request's destination is, and what a hut's position becomes
/// when it travels through the request system. Java calls it *static* as opposed
/// to `EntityLocation`, which tracks a moving entity — a distinction the port
/// keeps in the type system instead: an entity's position is read when needed,
/// never stored.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Site {
    pub dimension: Dimension,
    pub at: [i32; 3],
}

impl Site {
    pub fn new(dimension: impl Into<Dimension>, at: [i32; 3]) -> Site {
        Site {
            dimension: dimension.into(),
            at,
        }
    }

    /// The same place, moved — a hut is not moved, but a request re-targeted to
    /// another slot of the same hut is.
    pub fn with_position(&self, at: [i32; 3]) -> Site {
        Site {
            dimension: self.dimension.clone(),
            at,
        }
    }
}

impl Positioned for Site {
    fn dimension(&self) -> &str {
        self.dimension.as_str()
    }

    fn position(&self) -> [i32; 3] {
        self.at
    }
}

/// May a thing at `from` reach a thing at `to`? Java's answer, whole:
/// **same dimension** (`StaticLocation.java:66-70`), whatever the distance.
///
/// Two `?Sized` parameters so a `&dyn Positioned` is accepted as readily as a
/// `&Site`: the hooks that will call this hold trait objects, not concrete sites.
pub fn is_reachable<F, T>(from: &F, to: &T) -> bool
where
    F: Positioned + ?Sized,
    T: Positioned + ?Sized,
{
    from.dimension() == to.dimension()
}

/// Something that belongs to a colony: Java's `IColonyRelated`
/// (`api/colony/IColonyRelated.java:6-26`).
///
/// **The colony is an id, and it can be absent.** Java's `getColony()` returns a
/// live `IColony` that is `null` until `setColony` is called — a field, not a
/// promise — and `setColony(null)` is ignored rather than clearing it
/// (`core/entity/mobs/EntityMercenary.java:389-396`). The port says `None` for
/// "not assigned yet" and offers no detach, because Java has none.
pub trait InColony {
    /// The colony this belongs to, or `None` before it is assigned.
    fn colony(&self) -> Option<i32>;

    /// Assigns it. Java's `setColony` also calls `registerWithColony`
    /// (`EntityMercenary.java:394`), which the port cannot: see `base`'s docs.
    fn attach_to_colony(&mut self, colony: i32);
}

/// Something with a stable id inside a colony: Java's `ICitizen.getId()`
/// (`api/colony/ICitizen.java:15`).
///
/// **A building is not `Identified`, and that is Java's own answer**: a building
/// has no id, its position is its identity — the work manager keys on it
/// (`WorkManager.java:450`, already noted in `colony::Building`) and a claim
/// points at it. A type that wanted an id would have to invent one the save does
/// not carry.
pub trait Identified {
    fn id(&self) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the raider that is the only `IColonyRelated` in Java
    /// (`core/entity/mobs/EntityMercenary.java:72`).
    struct Raider {
        colony: Option<i32>,
        id: i32,
    }

    impl InColony for Raider {
        fn colony(&self) -> Option<i32> {
            self.colony
        }

        fn attach_to_colony(&mut self, colony: i32) {
            self.colony = Some(colony);
        }
    }

    impl Identified for Raider {
        fn id(&self) -> i32 {
            self.id
        }
    }

    #[test]
    fn a_site_remembers_its_dimension_and_its_block() {
        let site = Site::new("minecraft:overworld", [10, 64, -3]);
        assert_eq!(site.dimension(), "minecraft:overworld");
        assert_eq!(site.position(), [10, 64, -3]);
        assert_eq!(site, Site::new(Dimension::new("minecraft:overworld"), [10, 64, -3]));
    }

    #[test]
    fn reachable_is_the_same_dimension_and_nothing_else() {
        // Java: `location.getDimension() == getDimension()`
        // (`StaticLocation.java:66-70`). Two huts far apart are reachable.
        let hut = Site::new("minecraft:overworld", [0, 70, 0]);
        let far_hut = Site::new("minecraft:overworld", [2_000, 4, -9_999]);
        assert!(is_reachable(&hut, &far_hut));
    }

    #[test]
    fn the_nether_is_not_reachable_from_the_overworld_even_at_the_same_block() {
        let overworld = Site::new("minecraft:overworld", [0, 70, 0]);
        let nether = Site::new("minecraft:the_nether", [0, 70, 0]);
        assert!(!is_reachable(&overworld, &nether));
        assert!(!is_reachable(&nether, &overworld));
    }

    #[test]
    fn a_dimension_name_is_compared_whole_because_nothing_in_the_port_shortens_it() {
        assert_eq!(
            Dimension::new("minecraft:overworld"),
            Dimension::from("minecraft:overworld")
        );
        assert_ne!(Dimension::new("overworld"), Dimension::new("minecraft:overworld"));
        assert_eq!(Dimension::new("minecraft:overworld").as_str(), "minecraft:overworld");
    }

    #[test]
    fn a_raider_has_no_colony_until_it_is_assigned_one() {
        let mut raider = Raider {
            colony: None,
            id: 7,
        };
        assert_eq!(raider.colony(), None);
        raider.attach_to_colony(12);
        assert_eq!(raider.colony(), Some(12));
        assert_eq!(raider.id(), 7);
    }

    #[test]
    fn a_site_accepts_a_trait_object_so_a_hook_can_pass_what_it_has() {
        let site = Site::new("minecraft:overworld", [1, 2, 3]);
        let positional: &dyn Positioned = &site;
        assert!(is_reachable(positional, &site));
        assert_eq!(positional.position(), [1, 2, 3]);
    }
}
