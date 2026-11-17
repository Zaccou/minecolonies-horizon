//! `capabilityproviders/`: three Forge capabilities, and the one Horizon primitive that
//! replaces them.
//!
//! Java reference: three files, 172 lines together —
//! `MinecoloniesChunkCapabilityProvider` (55), `MinecoloniesWorldCapabilityProvider`
//! (56), `MinecoloniesWorldColonyManagerCapabilityProvider` (61) — three rows in
//! [`super::table`]. Each is the same shape: a value, a `LazyOptional` around it, a
//! `serializeNBT`/`deserializeNBT` pair that delegates to the capability's own
//! `Storage`, and a `getCapability` that answers the value for one key and
//! `LazyOptional.empty()` for everything else.
//!
//! **A capability provider is a plug.** It does not decide anything: it is how Forge
//! lets a mod hang a value on a `LevelChunk` or a `Level` without owning the object.
//! Horizon's answer is `AttachmentType` (`entity/attachment_holder.rs:15-26`: a holder
//! is an entity, a player, a block entity, **a chunk** or **a level**), declared by the
//! mod and read through `Host::query`. So all three rows are the same finding — *the
//! storage exists, the announcement does not* — and the primitive that is missing is
//! not `AttachmentType`: it is the **`AttachCapabilitiesEvent` moment**, which has no
//! hook (`ls crates/horizon-mod-vocab/src/hooks/` finds no `attach_*`).
//!
//! **What that costs, exactly.** Java's three capabilities are filled by
//! `ChunkDataHelper` and `ColonyManager` **as the world loads** (`onWorldLoad`,
//! `ChunkEvent.Load`), and the provider is constructed by Forge before any of that.
//! A Horizon mod can declare an `AttachmentType` and write it from
//! `chunk_load`/`level_load`; what it cannot do is have the value *exist* on a chunk
//! nobody has loaded yet, nor be told "this level object was just built, hand it its
//! capability". [`Attached`] records the mapping so the gap is one table and not three
//! prose paragraphs.
//!
//! **One asymmetry between the three.** The chunk capability is keyed by
//! `CLOSE_COLONY_CAP` and holds what the client also needs (`crate::network::common::ChunkCap`);
//! the two level ones are server-only and one of them — the colony manager — writes
//! **differently on the overworld**: `new MinecoloniesWorldColonyManagerCapabilityProvider(overworld)`
//! (`EventHandler.java:257`), and the flag decides what `Storage.writeNBT` emits. A
//! port that ignored the flag would restore the nether's manager into the overworld's
//! save slot.

/// The three capability keys the providers answer, as Java registers them (`:245`,
/// `:256-257`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// `minecolonies:closecolony` on a `LevelChunk` (`EventHandler.java:245`).
    CloseColony,
    /// `minecolonies:chunkupdate` on a `Level` (`:256`).
    ChunkUpdate,
    /// `minecolonies:colonymanager` on a `Level` (`:257`).
    ColonyManager,
}

impl Capability {
    /// The three, in the order the handler attaches them.
    pub const ALL: &'static [Capability] = &[
        Capability::CloseColony,
        Capability::ChunkUpdate,
        Capability::ColonyManager,
    ];

    /// The id `addCapability` is called with.
    pub fn id(self) -> &'static str {
        match self {
            Capability::CloseColony => "minecolonies:closecolony",
            Capability::ChunkUpdate => "minecolonies:chunkupdate",
            Capability::ColonyManager => "minecolonies:colonymanager",
        }
    }

    /// The Java interface behind it, as the provider names it.
    pub fn interface(self) -> &'static str {
        match self {
            Capability::CloseColony => "IColonyTagCapability",
            Capability::ChunkUpdate => "IChunkmanagerCapability",
            Capability::ColonyManager => "IColonyManagerCapability",
        }
    }

    /// The static field the provider's `serializeNBT` passes to `Storage`
    /// (`CLOSE_COLONY_CAP`, `CHUNK_STORAGE_UPDATE_CAP`, `COLONY_MANAGER_CAP`).
    pub fn storage(self) -> &'static str {
        match self {
            Capability::CloseColony => "CLOSE_COLONY_CAP",
            Capability::ChunkUpdate => "CHUNK_STORAGE_UPDATE_CAP",
            Capability::ColonyManager => "COLONY_MANAGER_CAP",
        }
    }

    /// Which object the provider is attached to.
    pub fn holder(self) -> Holder {
        match self {
            Capability::CloseColony => Holder::Chunk,
            Capability::ChunkUpdate | Capability::ColonyManager => Holder::Level,
        }
    }

    /// Whether the provider's constructor takes the overworld flag
    /// (`MinecoloniesWorldColonyManagerCapabilityProvider.java:37`).
    pub fn per_dimension(self) -> bool {
        self == Capability::ColonyManager
    }

    /// The `AttachmentType` holder that replaces it
    /// (`entity/attachment_holder.rs:15-26`): a chunk and a level are both holders, so
    /// the storage side needs nothing new.
    pub fn attachment_holder(self) -> &'static str {
        match self.holder() {
            Holder::Chunk => "AttachmentHolder::Chunk",
            Holder::Level => "AttachmentHolder::Level",
        }
    }
}

/// What a capability hangs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Chunk,
    Level,
}

/// The three capabilities with the Horizon primitive that replaces each.
#[derive(Clone, Copy, Debug)]
pub struct Attached {
    pub capability: Capability,
    /// The `AttachmentType` holder.
    pub holder: &'static str,
    /// What the port can do today.
    pub port: &'static str,
    /// What it cannot, and why.
    pub gap: &'static str,
}

/// The three, as one table. The `gap` strings are the `MANQUANT.md` entries.
pub const ATTACHED: &[Attached] = &[
    Attached {
        capability: Capability::CloseColony,
        holder: "AttachmentHolder::Chunk",
        port: "declare an AttachmentType and write it from `chunk_load`; `ChunkCap` already crosses to the client",
        gap: "no `AttachCapabilitiesEvent`: the value cannot exist before the chunk is loaded, and nothing is told when the level builds one",
    },
    Attached {
        capability: Capability::ChunkUpdate,
        holder: "AttachmentHolder::Level",
        port: "declare an AttachmentType on the level and write it from `level_load`",
        gap: "no `AttachCapabilitiesEvent<Level>`: a dimension's update table cannot be handed out at level construction",
    },
    Attached {
        capability: Capability::ColonyManager,
        holder: "AttachmentHolder::Level",
        port: "declare an AttachmentType on the level, with the overworld flag as a separate key",
        gap: "the flag has no carrier: `AttachmentType` has no per-dimension variant, so the overworld's manager and another dimension's need two types or a field",
    },
];

/// `getCapability`: the value for its own key, and nothing for every other.
///
/// The one thing every provider does that a port must not lose: answering a wrong key
/// is not an error, it is an empty answer (`LazyOptional.empty()`), so two mods can
/// ask the same object for different capabilities and both be answered.
pub fn answers(capability: Capability, asked: &str) -> bool {
    capability.id() == asked
}

/// `deserializeNBT`: whether the tag is the one this provider owns, or `getCapability`
/// with a different key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    /// The stored value, replaced by the tag's.
    Restored,
    /// The tag is empty: the provider keeps its fresh value (`IColonyTagCapability.Impl`
    /// starts at `NO_COLONY_ID`).
    Fresh,
}

/// What `Storage.readNBT` answers for an empty compound.
///
/// Java's `readNBT` (`IColonyTagCapability.Storage`) leaves the value untouched on a
/// missing key, so a provider whose chunk was never claimed has an owner of 0 and an
/// empty claim list — [`Load::Fresh`], not a refusal.
pub fn load(has_values: bool) -> Load {
    if has_values {
        Load::Restored
    } else {
        Load::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_ids_are_the_ones_the_handler_attaches() {
        assert_eq!(
            Capability::ALL.iter().map(|cap| cap.id()).collect::<Vec<_>>(),
            vec![
                "minecolonies:closecolony",
                "minecolonies:chunkupdate",
                "minecolonies:colonymanager"
            ]
        );
    }

    #[test]
    fn one_provider_answers_one_key() {
        assert!(answers(Capability::CloseColony, "minecolonies:closecolony"));
        assert!(!answers(Capability::CloseColony, "minecolonies:chunkupdate"));
        assert!(!answers(Capability::ColonyManager, "forge:energy"));
    }

    #[test]
    fn the_chunk_capability_hangs_on_a_chunk_and_the_two_others_on_a_level() {
        assert_eq!(Capability::CloseColony.holder(), Holder::Chunk);
        assert_eq!(Capability::ChunkUpdate.holder(), Holder::Level);
        assert_eq!(Capability::ColonyManager.holder(), Holder::Level);
    }

    #[test]
    fn only_the_colony_manager_reads_the_dimension() {
        assert!(Capability::ColonyManager.per_dimension());
        assert!(!Capability::ChunkUpdate.per_dimension());
        assert!(!Capability::CloseColony.per_dimension());
    }

    #[test]
    fn every_capability_names_its_storage_field() {
        assert_eq!(Capability::CloseColony.storage(), "CLOSE_COLONY_CAP");
        assert_eq!(Capability::ChunkUpdate.storage(), "CHUNK_STORAGE_UPDATE_CAP");
        assert_eq!(Capability::ColonyManager.storage(), "COLONY_MANAGER_CAP");
    }

    #[test]
    fn the_attachment_table_has_one_row_per_capability() {
        assert_eq!(ATTACHED.len(), 3);
        for attached in ATTACHED {
            assert_eq!(attached.holder, attached.capability.attachment_holder());
            assert!(attached.gap.contains("AttachCapabilitiesEvent") || attached.gap.contains("flag"));
        }
    }

    #[test]
    fn an_empty_compound_leaves_the_fresh_value() {
        assert_eq!(load(false), Load::Fresh);
        assert_eq!(load(true), Load::Restored);
    }

    #[test]
    fn the_three_interface_names_are_distinct() {
        let mut names: Vec<&str> = Capability::ALL
            .iter()
            .map(|cap| cap.interface())
            .collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
