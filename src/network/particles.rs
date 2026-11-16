//! The eight particle effects: a block cracking, a ring, compost, an item, a shower,
//! a sleeping `Z`, a stream between two points, a vanilla burst.
//!
//! Java reference: `core/network/messages/client/` — `BlockParticleEffectMessage.java`
//! (86), `CircleParticleEffectMessage.java` (118), `CompostParticleMessage.java`
//! (115), `ItemParticleEffectMessage.java` (152),
//! `LocalizedParticleEffectMessage.java` (109), `SleepingParticleMessage.java` (73),
//! `StreamParticleEffectMessage.java` (161), `VanillaParticleMessage.java` (109).
//!
//! **Every one of the eight is client-bound and carries nothing else**: no colony, no
//! permission, no range check. They are the one family where the port has nothing to
//! refuse but a wrong NBT kind — which is the point of keeping them separate: a
//! reader of `particles.rs` sees at a glance that a particle message cannot be a
//! security problem.
//!
//! **Two values are not what they look like.**
//!
//! - A particle type travels as its **registry name** (`buf.writeResourceLocation`,
//!   `CircleParticleEffectMessage.java:86`) — `minecraft:heart`, and for
//!   `SleepingParticleMessage` a type this mod registers
//!   (`ModParticleTypesInitializer.SLEEPINGPARTICLE_TYPE`, `:64`). A name is what a
//!   registry resolves; the port keeps the string and refuses an empty one.
//! - A block travels as its **state id** (`Block.getId(block)`,
//!   `BlockParticleEffectMessage.java:62`) — the same number `DirectPlace` carries,
//!   and for the same reason (`MANQUANT.md` §3).
//!
//! **`BlockParticleEffectMessage.BREAK_BLOCK` is `-1`** (`:22`): the `side` field is
//! either a `Direction` ordinal or that sentinel, and the two arms do different
//! things (`:76-83`). The port keeps the number and exports the sentinel, because a
//! type that turned `-1` into an `Option` would make `Some(0)` and `Some(-1)` both
//! mean "down", and `-1` means "break, all faces".

use super::common::{
    double, int, pos_of, position, stack, stack_of, text, text_of, ItemStack,
    Message, Refusal,
};
use mod_api::Compound;

/// `BlockParticleEffectMessage.java:22` — "this is a break, not a side".
pub const BREAK_BLOCK: i32 = -1;

/// `BlockParticleEffectMessage.java:20-85` — crack a block on one face, or break it
/// outright.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockParticleEffect {
    pub pos: [i32; 3],
    /// `Block.getId(state)` (`:62`) — the block state's registry id (`MANQUANT.md`
    /// §3).
    pub block: i32,
    /// A `Direction` ordinal, or [`BREAK_BLOCK`] (`:22`, `:76-83`).
    pub side: i32,
}

impl BlockParticleEffect {
    /// Whether this message breaks the block rather than cracking a face.
    pub fn breaks(&self) -> bool {
        self.side == BREAK_BLOCK
    }
}

impl Message for BlockParticleEffect {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("pos".into(), position(self.pos)),
            ("block".into(), int(self.block)),
            ("side".into(), int(self.side)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(BlockParticleEffect {
            pos: pos_of(data, "pos")?,
            block: int_data(data, "block")?,
            side: int_data(data, "side")?,
        })
    }
}

/// `CircleParticleEffectMessage.java:20-117` — a ring of particles around a point,
/// one stage at a time (`:101-102`).
#[derive(Clone, Debug, PartialEq)]
pub struct CircleParticleEffect {
    /// The particle's registry name (`:86`).
    pub kind: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    /// Which of the ring's eight steps (`:42`, `:101`).
    pub stage: i32,
}

impl Message for CircleParticleEffect {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("type".into(), text(&self.kind)),
            ("posX".into(), double(self.pos_x)),
            ("posY".into(), double(self.pos_y)),
            ("posZ".into(), double(self.pos_z)),
            ("stage".into(), int(self.stage)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(CircleParticleEffect {
            kind: non_empty(data, "type")?,
            pos_x: super::common::double_of(data, "posX")?,
            pos_y: super::common::double_of(data, "posY")?,
            pos_z: super::common::double_of(data, "posZ")?,
            stage: int_data(data, "stage")?,
        })
    }
}

/// `CompostParticleMessage.java:20-114` — the barrel's happy villagers (`:86-93`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompostParticle {
    pub pos: [i32; 3],
}

impl Message for CompostParticle {
    fn to_compound(&self) -> Compound {
        Compound::from([("pos".into(), position(self.pos))])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(CompostParticle { pos: pos_of(data, "pos")? })
    }
}

/// `ItemParticleEffectMessage.java:22-151` — the crumbs of a stack being eaten, at
/// the eater's eyes (`:127-148`).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemParticleEffect {
    pub stack: ItemStack,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub rotation_pitch: f64,
    pub rotation_yaw: f64,
    pub eye_height: f64,
}

impl Message for ItemParticleEffect {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("stack".into(), stack(&self.stack)),
            ("posX".into(), double(self.pos_x)),
            ("posY".into(), double(self.pos_y)),
            ("posZ".into(), double(self.pos_z)),
            ("rotationPitch".into(), double(self.rotation_pitch)),
            ("rotationYaw".into(), double(self.rotation_yaw)),
            ("eyeHeight".into(), double(self.eye_height)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(ItemParticleEffect {
            stack: stack_of(data, "stack")?,
            pos_x: super::common::double_of(data, "posX")?,
            pos_y: super::common::double_of(data, "posY")?,
            pos_z: super::common::double_of(data, "posZ")?,
            rotation_pitch: super::common::double_of(data, "rotationPitch")?,
            rotation_yaw: super::common::double_of(data, "rotationYaw")?,
            eye_height: super::common::double_of(data, "eyeHeight")?,
        })
    }
}

/// `LocalizedParticleEffectMessage.java:22-108` — the same crumbs, at a block's
/// centre (`:59-61`).
#[derive(Clone, Debug, PartialEq)]
pub struct LocalizedParticleEffect {
    pub stack: ItemStack,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
}

impl Message for LocalizedParticleEffect {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("stack".into(), stack(&self.stack)),
            ("posX".into(), double(self.pos_x)),
            ("posY".into(), double(self.pos_y)),
            ("posZ".into(), double(self.pos_z)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(LocalizedParticleEffect {
            stack: stack_of(data, "stack")?,
            pos_x: super::common::double_of(data, "posX")?,
            pos_y: super::common::double_of(data, "posY")?,
            pos_z: super::common::double_of(data, "posZ")?,
        })
    }
}

/// `SleepingParticleMessage.java:16-72` — the `Z` over a sleeping citizen's head
/// (`:64-70`).
#[derive(Clone, Debug, PartialEq)]
pub struct SleepingParticle {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Message for SleepingParticle {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("x".into(), double(self.x)),
            ("y".into(), double(self.y)),
            ("z".into(), double(self.z)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(SleepingParticle {
            x: super::common::double_of(data, "x")?,
            y: super::common::double_of(data, "y")?,
            z: super::common::double_of(data, "z")?,
        })
    }
}

/// `StreamParticleEffectMessage.java:20-160` — particles along the line between two
/// points, the last step arcing (`:134-158`).
#[derive(Clone, Debug, PartialEq)]
pub struct StreamParticleEffect {
    pub kind: String,
    pub start_x: f64,
    pub start_y: f64,
    pub start_z: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub end_z: f64,
    pub stage: i32,
    pub max_stage: i32,
}

impl Message for StreamParticleEffect {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("type".into(), text(&self.kind)),
            ("sPosX".into(), double(self.start_x)),
            ("sPosY".into(), double(self.start_y)),
            ("sPosZ".into(), double(self.start_z)),
            ("ePosX".into(), double(self.end_x)),
            ("ePosY".into(), double(self.end_y)),
            ("ePosZ".into(), double(self.end_z)),
            ("stage".into(), int(self.stage)),
            ("maxStage".into(), int(self.max_stage)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let max_stage = int_data(data, "maxStage")?;
        if max_stage < 1 {
            // `:136-138` divides by `maxStage`: a zero would be a division by zero on
            // every client that receives it.
            return Err(Refusal::Number { key: "maxStage", min: 1, max: i64::from(i32::MAX) });
        }
        Ok(StreamParticleEffect {
            kind: non_empty(data, "type")?,
            start_x: super::common::double_of(data, "sPosX")?,
            start_y: super::common::double_of(data, "sPosY")?,
            start_z: super::common::double_of(data, "sPosZ")?,
            end_x: super::common::double_of(data, "ePosX")?,
            end_y: super::common::double_of(data, "ePosY")?,
            end_z: super::common::double_of(data, "ePosZ")?,
            stage: int_data(data, "stage")?,
            max_stage,
        })
    }
}

/// `VanillaParticleMessage.java:24-108` — five of a vanilla particle over a citizen's
/// head (`:91-107`).
#[derive(Clone, Debug, PartialEq)]
pub struct VanillaParticle {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// The particle's registry name (`:63`).
    pub kind: String,
}

impl Message for VanillaParticle {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("x".into(), double(self.x)),
            ("y".into(), double(self.y)),
            ("z".into(), double(self.z)),
            ("type".into(), text(&self.kind)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(VanillaParticle {
            x: super::common::double_of(data, "x")?,
            y: super::common::double_of(data, "y")?,
            z: super::common::double_of(data, "z")?,
            kind: non_empty(data, "type")?,
        })
    }
}

fn int_data(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    super::common::int_of(data, key)
}

fn non_empty(data: &Compound, key: &'static str) -> Result<String, Refusal> {
    let value = text_of(data, key)?;
    if value.is_empty() {
        return Err(Refusal::Name { key });
    }
    Ok(value)
}

/// The eight messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("client/BlockParticleEffectMessage.java", "BlockParticleEffect"),
    ("client/CircleParticleEffectMessage.java", "CircleParticleEffect"),
    ("client/CompostParticleMessage.java", "CompostParticle"),
    ("client/ItemParticleEffectMessage.java", "ItemParticleEffect"),
    ("client/LocalizedParticleEffectMessage.java", "LocalizedParticleEffect"),
    ("client/SleepingParticleMessage.java", "SleepingParticle"),
    ("client/StreamParticleEffectMessage.java", "StreamParticleEffect"),
    ("client/VanillaParticleMessage.java", "VanillaParticle"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    #[test]
    fn a_cracked_block_round_trips_and_knows_a_break_from_a_face() {
        let cracked = BlockParticleEffect { pos: [1, 2, 3], block: 99, side: 2 };
        assert!(!round_trip(cracked.clone()).breaks());
        let broken = BlockParticleEffect { side: BREAK_BLOCK, ..cracked };
        assert!(round_trip(broken).breaks());
    }

    #[test]
    fn a_circle_round_trips_its_four_coordinates_and_stage() {
        let circle = CircleParticleEffect {
            kind: "minecraft:heart".to_owned(),
            pos_x: 1.5,
            pos_y: 64.0,
            pos_z: -3.25,
            stage: 5,
        };
        let read = round_trip(circle);
        assert_eq!((read.stage, read.pos_z), (5, -3.25));
    }

    #[test]
    fn the_two_crumb_messages_round_trip_their_stack() {
        let eaten = ItemParticleEffect {
            stack: ItemStack::new("minecraft:bread", 1).expect("a valid stack"),
            pos_x: 0.0,
            pos_y: 64.62,
            pos_z: 0.0,
            rotation_pitch: 12.5,
            rotation_yaw: -90.0,
            eye_height: 1.62,
        };
        let read = round_trip(eaten);
        assert_eq!(read.stack.item.as_str(), "minecraft:bread");
        assert_eq!(read.eye_height, 1.62);
        let localized = LocalizedParticleEffect {
            stack: ItemStack::new("minecraft:wheat", 2).expect("a valid stack"),
            pos_x: 4.5,
            pos_y: 65.5,
            pos_z: 4.5,
        };
        assert_eq!(round_trip(localized).pos_y, 65.5);
    }

    #[test]
    fn a_stream_refuses_the_zero_it_would_divide_by() {
        let stream = StreamParticleEffect {
            kind: "minecraft:end_rod".to_owned(),
            start_x: 0.0,
            start_y: 64.0,
            start_z: 0.0,
            end_x: 5.0,
            end_y: 64.0,
            end_z: 5.0,
            stage: 1,
            max_stage: 10,
        };
        assert_eq!(round_trip(stream.clone()).max_stage, 10);
        let mut data = stream.to_compound();
        data.insert("maxStage".into(), int(0));
        assert_eq!(
            StreamParticleEffect::from_compound(&data),
            Err(Refusal::Number { key: "maxStage", min: 1, max: i64::from(i32::MAX) })
        );
    }

    #[test]
    fn the_three_single_particle_messages_round_trip() {
        assert_eq!(round_trip(CompostParticle { pos: [-8, 63, 12] }).pos, [-8, 63, 12]);
        assert_eq!(
            round_trip(SleepingParticle { x: 1.0, y: 64.0, z: 2.0 }).y,
            64.0
        );
        assert_eq!(
            round_trip(VanillaParticle { x: 0.0, y: 64.0, z: 0.0, kind: "minecraft:cloud".to_owned() })
                .kind,
            "minecraft:cloud"
        );
    }

    #[test]
    fn an_empty_particle_name_is_refused() {
        let mut data = VanillaParticle { x: 0.0, y: 0.0, z: 0.0, kind: "minecraft:cloud".to_owned() }
            .to_compound();
        data.insert("type".into(), text(""));
        assert_eq!(VanillaParticle::from_compound(&data), Err(Refusal::Name { key: "type" }));
        data.insert("type".into(), int(4));
        assert_eq!(VanillaParticle::from_compound(&data), Err(Refusal::Kind { key: "type" }));
    }

    #[test]
    fn a_float_and_a_bool_field_are_not_accepted_where_a_position_is_expected() {
        // The three codecs this family does not use, pinned so a future edit cannot
        // quietly widen a reader: a `Float` is not a `Double`, and a `Byte` is a bool.
        let data = Compound::from([
            ("f".into(), super::super::common::float(0.25)),
            ("b".into(), super::super::common::boolean(true)),
        ]);
        assert_eq!(super::super::common::double_of(&data, "f"), Err(Refusal::Kind { key: "f" }));
        assert_eq!(super::super::common::bool_of(&data, "b"), Ok(true));
        assert_eq!(CENSUS.len(), 8);
    }
}
