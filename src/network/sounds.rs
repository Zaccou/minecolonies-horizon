//! Four sounds: a jingle for the player, the stop that follows it, music at a
//! position, and a citizen's voice queued with its repetitions.
//!
//! Java reference: `core/network/messages/client/PlayAudioMessage.java` (131),
//! `StopMusicMessage.java` (46), `client/colony/PlayMusicAtPosMessage.java` (110),
//! `client/colony/PlaySoundForCitizenMessage.java` (196).
//!
//! **All four are client-bound and carry no colony.** Even the two that name a
//! dimension (`PlayMusicAtPosMessage.java:78`, `PlaySoundForCitizenMessage.java:156`)
//! do it so the client can drop a sound for a level it is not in (`:104`) — not to
//! reach a colony. [`Message::RULE`] is [`crate::network::Rule::None`] for all four.
//!
//! **The sound category stays an integer.** `SoundSource` is vanilla's
//! (`SoundSource.values()[buf.readVarInt()]`, `PlayAudioMessage.java:77`), with
//! eleven values; the port has no type for it and does not invent one (`common`'s
//! note on the two vanilla enums). What the port does note is that Java writes it as
//! a **varint** here and as a plain `int` in `PlaySoundForCitizenMessage.java:154` —
//! the same value, two widths.
//!
//! **`PlayAudioMessage.sendToAll` is not ported.** It loops the colony's players and
//! sends a `StopMusicMessage` first when asked (`:111-129`) — a colony call, and the
//! colony's player lists are the layer that owns them.

use super::common::{float, int, pos_of, position, text, text_of, Message, Refusal, KEY_DIMENSION};
use crate::base::Dimension;
use mod_api::Compound;

/// `PlayAudioMessage.java:27-130` — play one sound to the player, at their own ears
/// (`:99-101`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayAudio {
    /// The sound event's registry name (`:71`), `minecolonies:colony_founded`.
    pub sound_event: String,
    /// Java's `SoundSource` ordinal (`:70`, `:77`).
    pub category: i32,
}

impl Message for PlayAudio {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("category".into(), int(self.category)),
            ("soundEvent".into(), text(&self.sound_event)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(PlayAudio {
            sound_event: named(data, "soundEvent")?,
            category: int_data(data, "category")?,
        })
    }
}

/// `StopMusicMessage.java:13-45` — stop whatever is playing, music included
/// (`:42-43`).
///
/// No fields (`:20-30`): the message *is* the stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopMusic;

impl Message for StopMusic {
    fn to_compound(&self) -> Compound {
        Compound::new()
    }

    fn from_compound(_data: &Compound) -> Result<Self, Refusal> {
        Ok(StopMusic)
    }
}

/// `PlayMusicAtPosMessage.java:23-109` — a record at a position, in one dimension
/// only (`:104-107`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayMusicAtPos {
    pub sound_event: String,
    pub pos: [i32; 3],
    pub dimension: Dimension,
    /// `writeFloat` (`:79`).
    pub volume: f32,
    pub pitch: f32,
}

impl Message for PlayMusicAtPos {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("soundEvent".into(), text(&self.sound_event)),
            ("pos".into(), position(self.pos)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("volume".into(), float(self.volume)),
            ("pitch".into(), float(self.pitch)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(PlayMusicAtPos {
            sound_event: named(data, "soundEvent")?,
            pos: pos_of(data, "pos")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            volume: super::common::float_of(data, "volume")?,
            pitch: super::common::float_of(data, "pitch")?,
        })
    }
}

/// `PlaySoundForCitizenMessage.java:29-195` — a citizen's voice, queued with how long
/// it lasts and how many times it repeats (`:192`).
///
/// All nine fields travel (`:151-161`), including the entity id — the client looks the
/// citizen up by it (`:189`) — and the position, which is where the sound is heard
/// from, not where the citizen stands.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaySoundForCitizen {
    pub entity_id: i32,
    pub sound_event: String,
    /// Java's `SoundSource` ordinal (`:168`).
    pub sound_source: i32,
    pub pos: [i32; 3],
    pub dimension: Dimension,
    pub volume: f32,
    pub pitch: f32,
    /// The sound's length in ticks (`:69`), used to space the repetitions.
    pub length: i32,
    pub repetitions: i32,
}

impl Message for PlaySoundForCitizen {
    fn to_compound(&self) -> Compound {
        Compound::from([
            ("soundEvent".into(), text(&self.sound_event)),
            ("soundSource".into(), int(self.sound_source)),
            ("pos".into(), position(self.pos)),
            (KEY_DIMENSION.into(), text(self.dimension.as_str())),
            ("volume".into(), float(self.volume)),
            ("pitch".into(), float(self.pitch)),
            ("length".into(), int(self.length)),
            ("repetitions".into(), int(self.repetitions)),
            ("entityid".into(), int(self.entity_id)),
        ])
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        Ok(PlaySoundForCitizen {
            entity_id: int_data(data, "entityid")?,
            sound_event: named(data, "soundEvent")?,
            sound_source: int_data(data, "soundSource")?,
            pos: pos_of(data, "pos")?,
            dimension: super::common::dimension_of(data, KEY_DIMENSION)?,
            volume: super::common::float_of(data, "volume")?,
            pitch: super::common::float_of(data, "pitch")?,
            length: non_negative(data, "length")?,
            repetitions: non_negative(data, "repetitions")?,
        })
    }
}

fn int_data(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    super::common::int_of(data, key)
}

fn non_negative(data: &Compound, key: &'static str) -> Result<i32, Refusal> {
    super::common::int_in(data, key, 0, i32::MAX)
}

/// A registry name that must name something: an empty string is not a sound.
fn named(data: &Compound, key: &'static str) -> Result<String, Refusal> {
    let value = text_of(data, key)?;
    if value.is_empty() {
        return Err(Refusal::Name { key });
    }
    Ok(value)
}

/// The four messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("client/PlayAudioMessage.java", "PlayAudio"),
    ("client/StopMusicMessage.java", "StopMusic"),
    ("client/colony/PlayMusicAtPosMessage.java", "PlayMusicAtPos"),
    ("client/colony/PlaySoundForCitizenMessage.java", "PlaySoundForCitizen"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn a_jingle_round_trips_its_event_and_category() {
        let audio = PlayAudio { sound_event: "minecolonies:colony_founded".to_owned(), category: 6 };
        assert_eq!(round_trip(audio).category, 6);
    }

    #[test]
    fn the_stop_round_trips_to_nothing() {
        assert_eq!(round_trip(StopMusic).to_compound(), Compound::new());
    }

    #[test]
    fn music_at_a_position_round_trips_its_float_gain() {
        let music = PlayMusicAtPos {
            sound_event: "minecolonies:music.tavern".to_owned(),
            pos: [32, 64, -16],
            dimension: overworld(),
            volume: 0.35,
            pitch: 1.0,
        };
        let read = round_trip(music);
        assert_eq!((read.volume, read.pitch), (0.35, 1.0));
        assert_eq!(read.dimension.as_str(), "minecraft:overworld");
    }

    #[test]
    fn a_citizen_voice_round_trips_all_nine_fields() {
        let voice = PlaySoundForCitizen {
            entity_id: 4021,
            sound_event: "minecolonies:voice.male".to_owned(),
            sound_source: 6,
            pos: [1, 64, 2],
            dimension: overworld(),
            volume: 1.0,
            pitch: 0.9,
            length: 40,
            repetitions: 3,
        };
        let read = round_trip(voice);
        assert_eq!((read.entity_id, read.length, read.repetitions), (4021, 40, 3));
        assert_eq!(read.pitch, 0.9);
    }

    #[test]
    fn a_sound_nobody_can_resolve_is_refused() {
        let mut data = PlayAudio { sound_event: "minecolonies:voice".to_owned(), category: 0 }.to_compound();
        data.insert("soundEvent".into(), text(""));
        assert_eq!(PlayAudio::from_compound(&data), Err(Refusal::Name { key: "soundEvent" }));
        data.insert("soundEvent".into(), text("minecolonies:voice"));
        data.insert("category".into(), text("MASTER"));
        assert_eq!(PlayAudio::from_compound(&data), Err(Refusal::Kind { key: "category" }));
    }

    #[test]
    fn a_negative_length_or_repetition_is_refused() {
        let mut data = PlaySoundForCitizen {
            entity_id: 1,
            sound_event: "minecolonies:voice".to_owned(),
            sound_source: 0,
            pos: [0, 0, 0],
            dimension: overworld(),
            volume: 1.0,
            pitch: 1.0,
            length: 1,
            repetitions: 1,
        }
        .to_compound();
        data.insert("repetitions".into(), int(-2));
        assert_eq!(
            PlaySoundForCitizen::from_compound(&data),
            Err(Refusal::Number { key: "repetitions", min: 0, max: i64::from(i32::MAX) })
        );
        assert_eq!(CENSUS.len(), 4);
    }
}
