//! The kinds of building MineColonies registers, and the name each is keyed by.
//!
//! Java reference: `api/colony/buildings/ModBuildings.java`, which declares
//! **55** `_ID` constants, and the initialiser that registers them,
//! `apiimp/initializer/ModBuildingsInitializer.java`, which calls
//! `createBuildingEntry()` **55** times.
//!
//! **54 are registered**, not 55: the `largeQuarry` entry sits inside a block
//! comment (`ModBuildingsInitializer.java:643-648`), so its constant names a
//! building no data pack can produce and `ModBuildings.largeQuarry` is commented
//! out too (`ModBuildings.java:116`). The count this port keeps is **54**, and
//! `Kind::ALL` is that list — a test holds the number.
//!
//! The registry path is `minecolonies:<name>`, which is what Java stores under
//! the building's `type` key (`api/util/constant/NbtTagConstants.java:140`) and
//! what `BuildingDataManager.createFrom` looks the entry up by
//! (`core/colony/buildings/registry/BuildingDataManager.java:38-44`).
//!
//! **Why a macro.** The name is the only thing tying a variant to Java's path,
//! and `Kind::ALL`, `name()` and `from_name()` must agree. One table expanded
//! three ways cannot disagree with itself; three hand-written tables would.

use std::fmt;

/// The namespace every building kind is registered under: MineColonies' own
/// mod id, not this package's (`Kind` is a *content* name, `hut::BLOCK_ID` is
/// the same).
pub const NAMESPACE: &str = "minecolonies";

macro_rules! kinds {
    ($($variant:ident => $name:literal,)*) => {
        /// One building entry of the `minecolonies:buildings` registry.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        // exhaustive: the registry is a closed list in Java, and a new kind
        // has to be added to the table below
        pub enum Kind {
            $($variant,)*
        }

        impl Kind {
            /// Every registered kind, alphabetically by registry path — the
            /// order `ALL` is compared in, so a caller may rely on it.
            pub const ALL: &'static [Kind] = &[$(Kind::$variant,)*];

            /// The registry path **without** its namespace, which is what
            /// `ModBuildings.*_ID` holds.
            pub fn name(self) -> &'static str {
                match self {
                    $(Kind::$variant => $name,)*
                }
            }

            /// The kind a bare name names, or `None`. Case matters: the
            /// engine's charset is lower case only (`horizon-core/src/identifier.rs:65-76`).
            pub fn from_name(name: &str) -> Option<Kind> {
                match name {
                    $($name => Some(Kind::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

kinds! {
    Alchemist => "alchemist",
    Archery => "archery",
    Baker => "baker",
    Barracks => "barracks",
    BarracksTower => "barrackstower",
    Beekeeper => "beekeeper",
    Blacksmith => "blacksmith",
    Builder => "builder",
    ChickenHerder => "chickenherder",
    CombatAcademy => "combatacademy",
    Composter => "composter",
    ConcreteMixer => "concretemixer",
    Cook => "cook",
    Cowboy => "cowboy",
    Crusher => "crusher",
    Deliveryman => "deliveryman",
    Dyer => "dyer",
    Enchanter => "enchanter",
    Farmer => "farmer",
    Fisherman => "fisherman",
    Fletcher => "fletcher",
    Florist => "florist",
    GateHouse => "gatehouse",
    Glassblower => "glassblower",
    Graveyard => "graveyard",
    GuardTower => "guardtower",
    Hospital => "hospital",
    Kitchen => "kitchen",
    Library => "library",
    Lumberjack => "lumberjack",
    Mechanic => "mechanic",
    MediumQuarry => "mediumquarry",
    Miner => "miner",
    MysticalSite => "mysticalsite",
    NetherWorker => "netherworker",
    Plantation => "plantation",
    Postbox => "postbox",
    RabbitHutch => "rabbithutch",
    Residence => "residence",
    Sawmill => "sawmill",
    School => "school",
    Shepherd => "shepherd",
    Sifter => "sifter",
    SimpleQuarry => "simplequarry",
    Smeltery => "smeltery",
    Stable => "stable",
    Stash => "stash",
    StoneMason => "stonemason",
    StoneSmeltery => "stonesmeltery",
    SwineHerder => "swineherder",
    Tavern => "tavern",
    TownHall => "townhall",
    University => "university",
    Warehouse => "warehouse",
}

impl Kind {
    /// The full registry path, `minecolonies:builder`: what the `type` key of a
    /// saved building holds and what a blueprint is named after
    /// (`api/blocks/AbstractBlockHut.java:344-347`).
    pub fn path(self) -> String {
        format!("{NAMESPACE}:{}", self.name())
    }

    /// The kind a registry path names. A bare name is accepted, because a
    /// hand-written `/data merge` may drop the namespace and guessing
    /// *minecolonies* is the only reading that can work here: no other mod's
    /// building is loadable by this one.
    pub fn from_path(text: &str) -> Option<Kind> {
        match text.split_once(':') {
            None => Kind::from_name(text),
            Some((namespace, path)) if namespace == NAMESPACE => Kind::from_name(path),
            Some(_) => None,
        }
    }

    /// Whether this is the one building a colony is founded with
    /// (`api/colony/buildings/ModBuildings.java:63`), and the only kind a colony
    /// may hold once (`core/blocks/huts/BlockHutTownHall.java:219-234`).
    pub fn is_town_hall(self) -> bool {
        self == Kind::TownHall
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_the_fifty_four_kinds_java_registers() {
        assert_eq!(
            Kind::ALL.len(),
            54,
            "55 `_ID` constants, one entry commented out (ModBuildingsInitializer.java:643-648)"
        );
        let mut names: Vec<&str> = Kind::ALL.iter().map(|kind| kind.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 54, "two kinds share a name, so a save cannot tell them apart");
    }

    #[test]
    fn every_kind_names_itself_and_comes_back_from_its_path() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_name(kind.name()), Some(*kind));
            assert_eq!(Kind::from_path(&kind.path()), Some(*kind), "{kind}");
            assert_eq!(kind.path(), format!("minecolonies:{}", kind.name()));
            assert!(
                !kind.name().contains(':') && !kind.name().is_empty(),
                "the path, not the name, carries the namespace"
            );
        }
        // A bare name resolves; another mod's namespace does not.
        assert_eq!(Kind::from_path("builder"), Some(Kind::Builder));
        assert_eq!(Kind::from_path("minecraft:builder"), None);
        assert_eq!(Kind::from_path("minecolonies:castle"), None);
        assert_eq!(Kind::from_path(""), None);
        assert_eq!(Kind::from_name("Builder"), None, "the charset is lower case only");
    }

    #[test]
    fn the_town_hall_is_the_only_town_hall() {
        let town_halls: Vec<Kind> = Kind::ALL
            .iter()
            .copied()
            .filter(|kind| kind.is_town_hall())
            .collect();
        assert_eq!(town_halls, vec![Kind::TownHall]);
        // The three the brief names by hand, so a rename cannot pass unnoticed.
        for (name, kind) in [
            ("builder", Kind::Builder),
            ("farmer", Kind::Farmer),
            ("townhall", Kind::TownHall),
        ] {
            assert_eq!(Kind::from_name(name), Some(kind));
        }
    }
}
