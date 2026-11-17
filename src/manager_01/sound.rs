//! The sound queue: who speaks, how long a voice waits, and how many speak at once.
//!
//! Java reference: `api/sounds/SoundManager.java:13-127`, and its only caller,
//! `core/network/messages/client/colony/PlaySoundForCitizenMessage.java:185-195`.
//!
//! **The key is the speaker, not the listener.** `addToQueue` is called with
//! `entity.getUUID()` of an `AbstractCivilianEntity` (`:189-192`), so the map is
//! *one queue per citizen*: a citizen that says three lines does not overlap
//! itself, and two citizens talk independently. The listener is always
//! `Minecraft.getInstance().player` (`SoundManager.java:51`), because the whole
//! class is client-side (`ColonyManager.java:624-629`, `onClientTick`).
//!
//! **One sound per queue per tick**, and the head has to reach its time-out
//! first: a sound plays, then `length` more ticks pass before the same queue is
//! asked again (`:57-67`) — so two plays are `length + 1` ticks apart, not
//! `length`. `length` is the sound's own duration, and the extra tick is Java's
//! (it sets the time-out *and* counts it down in the same call), kept here so a
//! client and this port do not drift.
//!
//! **`MAX_CONCURRENT_SOUNDS` is five and the loop yields four.** The counter
//! starts at 1 (`:41`) and breaks once it reaches the constant (`:69-72`) after
//! being incremented once per queue visited (`:68`) — so four queues are advanced
//! per tick. [`QUEUES_PER_TICK`] is that number, [`MAX_CONCURRENT_SOUNDS`] is the
//! constant, and the test below pins both. Raising the counter to 0 would be a
//! one-character fix; it is *not* made here, because a busy colony would then
//! speak half a voice more often than a Java server does.
//!
//! **Two things Java keeps that this port keeps**: a queue that has played its
//! last repetition stays in the map (the emptiness test is on the *map*,
//! `:30-33`, never on the individual deques), and a queue of zero sounds costs
//! nothing in the per-tick budget (`:44-47` continues before `:68` increments).
//! A client that queued a hundred chat lines therefore keeps a hundred empty
//! entries for the rest of the session; the map is cleared only when the level
//! goes away (`:35-39`).
//!
//! **The one thing this port changes is the order.** Java walks a `HashMap`
//! (`:23`), so which four of five queues are advanced is unspecified and can
//! change between runs. Here the map is a `BTreeMap` keyed by the speaker, so a
//! test can say *which* four — and a citizen whose queue is never reached is
//! always the same one. This is the same choice `crate::manager::claims` made for
//! a chunk's owner, and for the same reason.
//!
//! **Pure data, no host call.** [`SoundQueue::tick`] answers the sounds that
//! started, and the caller plays them. On a client that is one call per line of
//! [`Playback`] — `play_local_sound(&SoundPlay::at(event, source, pos), 0)` with
//! `volume` and `pitch` applied (`crates/horizon-mod-vocab/src/client/types.rs:556-590`,
//! `host/effects.rs:38`) — which is Java's `level.playSound(player, pos, event,
//! source, volume, pitch)` one for one. `MANQUANT.md` §1 says why the call is
//! named here and not made here.

use std::collections::{BTreeMap, VecDeque};

/// How many voices Java's counter allows, as the constant spells it
/// (`SoundManager.java:18`).
pub const MAX_CONCURRENT_SOUNDS: usize = 5;

/// How many voices a tick *actually* advances: `MAX_CONCURRENT_SOUNDS - 1`
/// (`SoundManager.java:41` starts the count at 1, `:68-72` breaks at the
/// constant, so four queues are visited).
pub const QUEUES_PER_TICK: usize = MAX_CONCURRENT_SOUNDS - 1;

/// The id of the entity that speaks: Java's `UUID` argument to `addToQueue`
/// (`SoundManager.java:87-99`), always a citizen's (`PlaySoundForCitizenMessage.java:189`).
pub type Speaker = u128;

/// One queued sound, Java's `TimedSound` (`SoundManager.java:104-126`).
///
/// The six fields Java takes in the constructor (`:116-125`) are immutable; the
/// two it mutates are `repetitions` and `timeout` (`:113-114`), and only
/// [`SoundQueue`] touches them.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: 1:1 with `TimedSound`, whose field list is closed
pub struct TimedSound {
    /// The sound event's registry name (`:106`).
    pub sound_event: String,
    /// Vanilla's `SoundSource` ordinal, kept an integer exactly as
    /// `crate::network::sounds::PlaySoundForCitizen` keeps it (`:107`, written
    /// `:168` of the message).
    pub source: i32,
    /// How long the sound lasts, in ticks: the gap it asks for (`:108`).
    pub length: i32,
    /// Where it is heard from, which is the citizen's position at send time, not
    /// the listener's (`:109`).
    pub pos: [i32; 3],
    pub volume: f32,
    pub pitch: f32,
    /// How many plays are left *after* the one in progress: Java's
    /// `repetitions` (`:113`), so `0` means "once, then take it out".
    pub repetitions: u32,
    /// Ticks the head must wait before it plays; Java starts it at 0 so a fresh
    /// sound plays on the next tick (`:114`).
    timeout: i32,
}

impl TimedSound {
    /// A sound ready to play on the next tick; the argument order is Java's
    /// (`SoundManager.java:87-95`, `:116`).
    ///
    /// `repetitions` is a `u32` where Java has an `int`: a negative count would
    /// play once and then be dropped (`:58-62`), which is what `0` already does,
    /// and a negative count cannot reach here — `PlaySoundForCitizenMessage`
    /// refuses one when it reads the packet
    /// (`crate::network::sounds::PlaySoundForCitizen::from_compound`).
    pub fn new(
        sound_event: impl Into<String>,
        source: i32,
        repetitions: u32,
        length: i32,
        pos: [i32; 3],
        volume: f32,
        pitch: f32,
    ) -> TimedSound {
        TimedSound {
            sound_event: sound_event.into(),
            source,
            length,
            pos,
            volume,
            pitch,
            repetitions,
            timeout: 0,
        }
    }

    /// Ticks until this sound is allowed to play: 0 or less means "now"
    /// (`SoundManager.java:49`).
    pub fn timeout(&self) -> i32 {
        self.timeout
    }
}

/// One sound that started this tick: what the caller has to play.
///
/// Java plays inline (`SoundManager.java:51-56`) and hands the same six values
/// to `playSound`; here they come back as a value, because a module of this crate
/// makes no host call.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the six arguments of `Level.playSound`, nothing else
pub struct Playback {
    /// The entity whose voice this is (`:48`, the map's key).
    pub speaker: Speaker,
    pub sound_event: String,
    /// Vanilla's `SoundSource` ordinal (`:54`).
    pub source: i32,
    /// Where it plays (`:52`); Java passes the citizen's position, not the
    /// player's, so a voice is heard where the citizen stands.
    pub pos: [i32; 3],
    pub volume: f32,
    pub pitch: f32,
}

/// The queue of queues: Java's static `soundQueue` (`SoundManager.java:23`) as a
/// value the client's session owns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundQueue {
    /// One deque per speaker. `BTreeMap`, not `HashMap`: see the module note.
    speakers: BTreeMap<Speaker, VecDeque<TimedSound>>,
}

impl SoundQueue {
    pub fn new() -> SoundQueue {
        SoundQueue::default()
    }

    /// How many speakers hold a queue — including speakers whose queue is empty
    /// but not yet forgotten (`SoundManager.java:30-33`).
    pub fn len(&self) -> usize {
        self.speakers.len()
    }

    /// True when no speaker holds a queue, which is the only condition that makes
    /// [`SoundQueue::tick`] return at once (`SoundManager.java:30-33`).
    pub fn is_empty(&self) -> bool {
        self.speakers.is_empty()
    }

    /// How many sounds are waiting for `speaker`, the head included.
    pub fn pending(&self, speaker: Speaker) -> usize {
        self.speakers.get(&speaker).map_or(0, VecDeque::len)
    }

    /// The queued sounds of one speaker, head first.
    pub fn sounds(&self, speaker: Speaker) -> impl Iterator<Item = &TimedSound> + '_ {
        self.speakers
            .get(&speaker)
            .into_iter()
            .flat_map(|queue| queue.iter())
    }

    /// Every speaker that holds a queue, in uuid order.
    pub fn speakers(&self) -> impl Iterator<Item = Speaker> + '_ {
        self.speakers.keys().copied()
    }

    /// Adds one sound at the **back** of that speaker's queue, creating the queue
    /// if the speaker has none (`SoundManager.java:97-98`).
    pub fn add(&mut self, speaker: Speaker, sound: TimedSound) {
        self.speakers
            .entry(speaker)
            .or_default()
            .push_back(sound);
    }

    /// Forgets every queue: what Java does when the client has no level
    /// (`SoundManager.java:35-39`).
    pub fn clear(&mut self) {
        self.speakers.clear();
    }

    /// One tick, Java's `tick` (`SoundManager.java:28-74`).
    ///
    /// `level_present` is Java's `Minecraft.getInstance().level != null` test
    /// (`:35`): without a level the whole queue is dropped, and nothing plays.
    /// Returns the sounds that started, in the order they were started.
    ///
    /// The three rules the loop keeps, in Java's order:
    ///
    /// 1. a queue with nothing in it is skipped **and costs no budget**
    ///    (`:44-47` runs before `:68`);
    /// 2. a head whose [`TimedSound::timeout`] is 0 or less plays and is re-armed
    ///    at `length` (`:49-57`); a head that still waits loses one tick (`:66`);
    /// 3. when the play was the last repetition, the head is dropped (`:58-62`).
    pub fn tick(&mut self, level_present: bool) -> Vec<Playback> {
        if self.speakers.is_empty() {
            return Vec::new();
        }
        if !level_present {
            self.speakers.clear();
            return Vec::new();
        }

        let mut started = Vec::new();
        let mut visited = 0usize;
        for (speaker, queue) in self.speakers.iter_mut() {
            let Some(head) = queue.front_mut() else {
                // `:44-47`: an empty deque is skipped, and `:68` is not reached.
                continue;
            };
            let mut finished = false;
            if head.timeout <= 0 {
                started.push(Playback {
                    speaker: *speaker,
                    sound_event: head.sound_event.clone(),
                    source: head.source,
                    pos: head.pos,
                    volume: head.volume,
                    pitch: head.pitch,
                });
                head.timeout = head.length;
                // `:58-62`: `repetitions--`, and the sound leaves the queue once
                // the count is below zero. `saturating_sub` is the same rule on a
                // count that cannot go negative: zero means this play was the last.
                finished = head.repetitions == 0;
                head.repetitions = head.repetitions.saturating_sub(1);
            } else {
                head.timeout -= 1;
            }
            if finished {
                queue.pop_front();
            }
            visited += 1;
            if visited == QUEUES_PER_TICK {
                break;
            }
        }
        started
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sound(repetitions: u32, length: i32) -> TimedSound {
        TimedSound::new("minecolonies:voice.male", 6, repetitions, length, [10, 64, -3], 1.0, 0.9)
    }

    /// The speakers a tick started, for a test that only cares about who spoke.
    fn spoken(started: &[Playback]) -> Vec<Speaker> {
        started.iter().map(|play| play.speaker).collect()
    }

    #[test]
    fn a_fresh_sound_plays_on_the_next_tick_and_leaves_its_queue() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(0, 40));
        assert_eq!(queue.pending(1), 1, "queued, not played");
        let started = queue.tick(true);
        assert_eq!(spoken(&started), vec![1]);
        assert_eq!(queue.pending(1), 0, "no repetition asked, the queue is done");
    }

    #[test]
    fn repetitions_are_the_number_of_plays_after_the_first() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(2, 0));
        let mut plays = 0;
        for _ in 0..100 {
            plays += queue.tick(true).len();
        }
        assert_eq!(plays, 3, "repetitions = 2 means three plays, as `:58-62` counts");
        assert!(queue.pending(1) == 0);
    }

    #[test]
    fn two_plays_of_one_voice_are_length_plus_one_ticks_apart() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(2, 5));
        let mut at = Vec::new();
        for tick in 0..40 {
            if !queue.tick(true).is_empty() {
                at.push(tick);
            }
        }
        // The play re-arms the time-out at `length` and the same call counts one
        // tick down, so five ticks of waiting plus the tick the time-out is read
        // on: `:57` then `:66` then `:49`.
        assert_eq!(at, vec![0, 6, 12]);
    }

    #[test]
    fn a_negative_length_plays_every_tick() {
        // Nothing refuses a negative `length` in `SoundManager` (`:108`); the port
        // keeps Java's behaviour and lets `crate::network::sounds` refuse the
        // packet that would carry one.
        let mut queue = SoundQueue::new();
        queue.add(1, sound(3, -1));
        for tick in 0..4 {
            assert_eq!(queue.tick(true).len(), 1, "tick {tick}");
        }
        assert_eq!(
            queue.pending(1),
            0,
            "four plays on four ticks: the first and its three repetitions, no waiting in between"
        );
    }

    #[test]
    fn four_voices_advance_per_tick_and_the_constant_says_five() {
        assert_eq!(MAX_CONCURRENT_SOUNDS, 5);
        assert_eq!(QUEUES_PER_TICK, 4);
        let mut queue = SoundQueue::new();
        for speaker in 1..=6 {
            queue.add(speaker, sound(10, 0));
        }
        assert_eq!(spoken(&queue.tick(true)), vec![1, 2, 3, 4], "the fifth is never reached");
        let untouched = queue.sounds(5).next().expect("still queued");
        assert_eq!(
            (untouched.repetitions, untouched.timeout),
            (10, 0),
            "and it is not even ticked down: `:66` is never reached either"
        );
    }

    #[test]
    fn a_voice_that_never_ends_starves_the_ones_after_it() {
        // The consequence of four-per-tick with a deterministic order: six voices
        // all asking for a lot of repetitions keep the last two silent for good.
        let mut queue = SoundQueue::new();
        for speaker in 1..=6 {
            queue.add(speaker, sound(u32::MAX, 0));
        }
        let mut heard = Vec::new();
        for _ in 0..20 {
            heard.extend(spoken(&queue.tick(true)));
        }
        assert!(heard.iter().all(|speaker| *speaker <= 4), "5 and 6 never speak: {heard:?}");
    }

    #[test]
    fn a_queue_with_nothing_in_it_costs_no_budget() {
        let mut queue = SoundQueue::new();
        // Six speakers, each with exactly one play, so the first four queues empty
        // themselves on the first tick and stay in the map (`:30-33`).
        for speaker in 1..=6 {
            queue.add(speaker, sound(0, 0));
        }
        assert_eq!(spoken(&queue.tick(true)), vec![1, 2, 3, 4]);
        assert_eq!(queue.len(), 6, "the four empty queues are still there");
        assert_eq!(
            spoken(&queue.tick(true)),
            vec![5, 6],
            "the empty ones are skipped before `:68` increments the counter"
        );
    }

    #[test]
    fn a_finished_queue_stays_in_the_map_until_the_level_goes_away() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(0, 0));
        queue.tick(true);
        assert_eq!(queue.pending(1), 0);
        assert_eq!(queue.len(), 1, "`:30-33` tests the map, never the deque");
        assert!(!queue.is_empty());
        assert!(queue.tick(true).is_empty(), "and an empty map of sounds plays nothing");
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn no_level_drops_every_queue_and_plays_nothing() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(5, 0));
        queue.add(2, sound(5, 0));
        assert!(queue.tick(false).is_empty());
        assert_eq!(queue.len(), 0, "`:35-39` clears the map, it does not just skip a tick");
    }

    #[test]
    fn two_voices_do_not_share_a_queue() {
        let mut queue = SoundQueue::new();
        queue.add(7, sound(0, 0));
        queue.add(9, sound(0, 0));
        let started = queue.tick(true);
        assert_eq!(spoken(&started), vec![7, 9], "one voice each, both on the same tick");
        assert_eq!(started[1].pos, [10, 64, -3]);
        assert_eq!(queue.pending(7), 0);
        assert_eq!(queue.pending(9), 0);
    }

    #[test]
    fn a_second_sound_waits_for_the_first_to_finish() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(0, 0));
        queue.add(1, sound(0, 0));
        assert_eq!(queue.pending(1), 2);
        let first = queue.tick(true);
        assert_eq!(first.len(), 1, "one sound per queue per tick (`:48` peeks, never drains)");
        let second = queue.tick(true);
        assert_eq!(spoken(&second), vec![1]);
        assert_eq!(queue.pending(1), 0);
    }

    #[test]
    fn a_playback_carries_the_six_values_java_hands_to_play_sound() {
        let mut queue = SoundQueue::new();
        queue.add(
            42,
            TimedSound::new("minecolonies:voice.female", 7, 0, 33, [-1, 70, 8], 0.5, 1.25),
        );
        let started = queue.tick(true);
        assert_eq!(
            started,
            vec![Playback {
                speaker: 42,
                sound_event: "minecolonies:voice.female".to_owned(),
                source: 7,
                pos: [-1, 70, 8],
                volume: 0.5,
                pitch: 1.25,
            }]
        );
    }

    #[test]
    fn a_played_sound_waits_out_its_length_before_it_is_asked_again() {
        let mut queue = SoundQueue::new();
        queue.add(1, sound(2, 2));
        queue.tick(true);
        let head = queue.sounds(1).next().expect("two repetitions left, still queued");
        assert_eq!(head.timeout, 2, "re-armed at `length` (`:57`)");
        assert_eq!(head.repetitions, 1, "one repetition spent (`:58`)");
        assert!(queue.tick(true).is_empty(), "two ticks of wait: `:66` counts the first");
        assert_eq!(queue.pending(1), 1, "nothing was dropped while it waited");
    }
}
