//! The citizen views: what a client keeps about each citizen, and how one update
//! reaches it without losing the rest.
//!
//! Java reference: `core/colony/CitizenDataManager.java:22-39` (the update itself),
//! `core/colony/ColonyView.java:818-827` (where the answer is stored), `:899-902`
//! (a removal), `:95` (the map) and `core/colony/ColonyManager.java:752-761` (the
//! colony lookup that comes first).
//!
//! **A citizen view is updated, not replaced.** `createFromNetworkData` takes the
//! view the colony already holds and deserialises into it (`:25`), so a caller that
//! keeps the object — a window, a row, a renderer — sees the new numbers without
//! being told. That is the rule this file ports: the map is asked for the citizen
//! first, and the decode is handed **what the client already knew**. A field the
//! message does not mention keeps its old value, which is the whole point of the
//! reuse: `CitizenDataView.deserialize` reads forty fields and clears three of them.
//!
//! **The half of `CitizenDataManager` that is *not* here is the file half.** The
//! same class has `createFromNBT` (`:14-20`), which builds a citizen out of its
//! saved compound: it is already ported, in `crate::persistence::roster::read`
//! (the id, the `0` id no factory hands out, the needs), and this file does not
//! repeat it. The two halves also fail differently, and Java's difference is a
//! rule worth keeping visible:
//!
//! | Where | One bad citizen costs |
//! | --- | --- |
//! | the save (`CitizenManager.java:178-180`) | **the whole colony** — `Collectors.toMap` throws, out of `loadColony`'s catch |
//! | the network (`CitizenDataManager.java:31-36`) | **that citizen only** — the exception is logged and `null` goes back |
//!
//! A network message is a patch on a client's copy, so a refusal is a missing
//! citizen on one screen; a save is the colony's only copy, so a refusal is the
//! colony. The tests below pin the second line.
//!
//! **Three things this port names instead of doing silently.**
//!
//! 1. **A message for a colony the client does not hold is dropped**
//!    (`ColonyManager.java:755-758` returns without a word). Named here as
//!    [`Refusal::UnknownColony`]: a caller that reads the refusal learns it is out
//!    of step, which a silent `return` never says.
//! 2. **A citizen id of zero or less is refused.** Java's `getCitizen` routes a
//!    non-positive id to the *visitor* map (`ColonyView.java:633-643`) and then
//!    builds a `CitizenDataView` with it anyway, which the caller stores under that
//!    id (`:823`): a citizen of id 0 answers for nobody, and
//!    `crate::persistence` already refuses the same id on the file side
//!    (`Refusal::CitizenWithoutId`).
//! 3. **A failed decode leaves the map untouched.** Java reuses the object and
//!    writes into it *before* the read that throws, so the map keeps a half-written
//!    view of a citizen whose state "cannot be restored"
//!    (`CitizenDataManager.java:33-35`). This port's decode produces the new value
//!    and only then stores it, so a refusal is a citizen that did not move — the
//!    same message, one field worse than Java, rather than forty fields
//!    half-updated.
//!
//! **Pure data, no network.** There is no `FriendlyByteBuf` here: the decode is a
//! closure the caller supplies, and this module only owns the order — look the
//! colony up, look the citizen up, decode, *then* write. That order is what the
//! batch test earns: a refusal on the second of three citizens writes none of the
//! three, and does not even clear the old list.

use std::collections::BTreeMap;

/// Why an update did not reach the map, named.
///
/// `E` is the caller's own decode error — this module has no buffer and therefore
/// no opinion on what a bad field is called. Every variant is a different answer to
/// "what did the client keep": nothing at all (the first two) or what it had (the
/// third).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal<E> {
    /// No colony view for that id in that dimension, so there is nothing to patch
    /// (`ColonyManager.java:752-758`). Java returns without a word.
    UnknownColony,
    /// A citizen id of zero or less, which `CitizenManager` never hands out; Java
    /// would store a view of id 0 under that id (`ColonyView.java:633-643`, `:823`).
    NonPositiveId { id: i32 },
    /// The decode refused the data (`CitizenDataManager.java:31-36`): the view the
    /// client had is still the one it has.
    DecodeFailed { id: i32, cause: E },
}

impl<E> Refusal<E> {
    /// A stable name, namespaced like a registry entry: a log line has to be
    /// searchable and can never be mistaken for a player-facing sentence.
    pub fn name(&self) -> &'static str {
        match self {
            Self::UnknownColony => "views.refusal.unknown_colony",
            Self::NonPositiveId { .. } => "views.refusal.non_positive_id",
            Self::DecodeFailed { .. } => "views.refusal.decode_failed",
        }
    }

    /// The citizen the refusal is about, when it is about one.
    pub fn id(&self) -> Option<i32> {
        match self {
            Self::UnknownColony => None,
            Self::NonPositiveId { id } | Self::DecodeFailed { id, .. } => Some(*id),
        }
    }
}

impl<E: std::fmt::Debug> std::fmt::Display for Refusal<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::UnknownColony => Ok(()),
            Self::NonPositiveId { id } => write!(f, " (id {id})"),
            Self::DecodeFailed { id, cause } => write!(f, " (id {id}: {cause:?})"),
        }
    }
}

/// The citizens a client knows, Java's `ColonyView.citizens` (`:95`) as a value.
///
/// `T` is the citizen as this port holds it — `crate::colony::Citizen` fits, and a
/// test below uses it, so the registry is not generic for want of a type.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: one field, Java's own map
pub struct Views<T> {
    citizens: BTreeMap<i32, T>,
}

/// `Default` by hand, not by derive: the derive would demand `T: Default` for a
/// struct whose field is a `BTreeMap` that needs nothing of `T`.
impl<T> Default for Views<T> {
    fn default() -> Self {
        Views {
            citizens: BTreeMap::new(),
        }
    }
}

impl<T> Views<T> {
    pub fn new() -> Views<T> {
        Views::default()
    }

    pub fn len(&self) -> usize {
        self.citizens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.citizens.is_empty()
    }

    /// One citizen, as the client last saw them (`ColonyView.java:633-643`, the
    /// positive branch of `getCitizen`).
    pub fn get(&self, id: i32) -> Option<&T> {
        self.citizens.get(&id)
    }

    /// Every id the client holds, in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.citizens.keys().copied()
    }

    /// Every citizen, by id.
    pub fn values(&self) -> impl Iterator<Item = &T> + '_ {
        self.citizens.values()
    }

    /// `ColonyView.handleColonyViewRemoveCitizenMessage` (`:899-902`, the removal at
    /// `:901`): takes one citizen out and answers whether the client had them.
    pub fn remove(&mut self, id: i32) -> bool {
        self.citizens.remove(&id).is_some()
    }

    /// One citizen message, `CitizenDataManager.createFromNetworkData`
    /// (`:22-39`) through `ColonyView.handleColonyViewCitizensMessage`
    /// (`:818-827`).
    ///
    /// `colony_known` is the lookup `ColonyManager.handleColonyViewCitizensMessage`
    /// does first (`:752-757`): a message for a colony this client does not hold is
    /// dropped before anything is read.
    ///
    /// `decode` is handed the view already held for that id — `colonyView.getCitizen(id)`
    /// in Java (`:25`), the reused object — and produces the updated one. Answer
    /// `true` when an existing citizen was updated, `false` when this was the first
    /// message about them.
    ///
    /// **The write is last**: a refusal of any kind leaves the map exactly as it
    /// was, which is the one thing this port does differently from Java (module
    /// note, point 3).
    pub fn apply<E>(
        &mut self,
        colony_known: bool,
        id: i32,
        decode: impl FnOnce(Option<&T>) -> Result<T, E>,
    ) -> Result<bool, Refusal<E>> {
        if !colony_known {
            return Err(Refusal::UnknownColony);
        }
        if id <= 0 {
            return Err(Refusal::NonPositiveId { id });
        }
        let previous = self.citizens.get(&id);
        let view = decode(previous).map_err(|cause| Refusal::DecodeFailed { id, cause })?;
        Ok(self.citizens.insert(id, view).is_some())
    }

    /// A whole list in one message, `ColonyView.handleColonyViewVisitorMessage`
    /// (`:830-853`, and `:858-880` for animals, which is the same loop).
    ///
    /// `refresh` is Java's flag (`:834-838`): the list is replaced, so a citizen
    /// this message does not name is forgotten. Reuse still works under `refresh`
    /// in Java, because the cache it reuses from is a copy taken *before* the clear
    /// (`:832`) — here the decode reads the live map before anything is written, so
    /// it sees the same thing.
    ///
    /// **Atomic where Java is not.** Java's loop has no `try`, so a bad entry
    /// throws out of the handler with the map half-applied: the client keeps the
    /// entries before it and loses the ones after. This port decodes every entry
    /// first and writes only when all of them are good — a refusal writes nothing,
    /// not even the `clear`.
    ///
    /// Answers how many citizens the map holds from this message.
    pub fn apply_batch<E>(
        &mut self,
        colony_known: bool,
        refresh: bool,
        ids: &[i32],
        mut decode: impl FnMut(i32, Option<&T>) -> Result<T, E>,
    ) -> Result<usize, Refusal<E>> {
        if !colony_known {
            return Err(Refusal::UnknownColony);
        }
        if let Some(id) = ids.iter().copied().find(|id| *id <= 0) {
            return Err(Refusal::NonPositiveId { id });
        }

        // Phase 1: decode everything, write nothing. `self.citizens` is still the
        // cache Java snapshots at `:832`, so reuse and `refresh` both behave.
        let mut decoded = Vec::with_capacity(ids.len());
        for id in ids.iter().copied() {
            let view = decode(id, self.citizens.get(&id))
                .map_err(|cause| Refusal::DecodeFailed { id, cause })?;
            decoded.push((id, view));
        }

        // Phase 2: the write, and only now.
        if refresh {
            self.citizens.clear();
        }
        for (id, view) in decoded {
            self.citizens.insert(id, view);
        }
        Ok(self.citizens.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view with a field the update carries and a field it does not, which is how
    /// Java's reuse is visible.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Seen {
        name: String,
        status: i32,
    }

    /// The decode of one message: it reads the name and **keeps** the status the
    /// client already had, as `CitizenDataView.deserialize` keeps everything the
    /// buffer does not name.
    fn read_name(name: &'static str, previous: Option<&Seen>) -> Result<Seen, &'static str> {
        if name.is_empty() {
            return Err("empty name");
        }
        Ok(Seen {
            name: name.to_owned(),
            status: previous.map_or(0, |seen| seen.status),
        })
    }

    #[test]
    fn a_first_message_creates_the_citizen() {
        let mut views: Views<Seen> = Views::new();
        assert_eq!(views.apply(true, 4, |previous| read_name("Alfred", previous)), Ok(false));
        assert_eq!(views.len(), 1);
        assert_eq!(views.get(4).map(|seen| seen.name.as_str()), Some("Alfred"));
    }

    #[test]
    fn an_update_is_applied_to_the_citizen_the_client_already_had() {
        let mut views: Views<Seen> = Views::new();
        views
            .apply(true, 4, |_| Ok::<_, &'static str>(Seen { name: "Alfred".to_owned(), status: 3 }))
            .expect("created");
        assert_eq!(
            views.apply(true, 4, |previous| read_name("Alfred II", previous)),
            Ok(true),
            "`true`: the client knew that citizen, so this was an update"
        );
        assert_eq!(
            views.get(4),
            Some(&Seen { name: "Alfred II".to_owned(), status: 3 }),
            "the status was not in the message, so reuse kept it (`:25`)"
        );
    }

    #[test]
    fn a_message_for_a_colony_this_client_does_not_hold_writes_nothing() {
        let mut views: Views<Seen> = Views::new();
        let refused = views.apply(false, 4, |previous| read_name("Alfred", previous));
        assert_eq!(refused, Err(Refusal::UnknownColony));
        assert!(views.is_empty(), "`ColonyManager.java:755-758`: nothing was read at all");
        assert_eq!(Refusal::<&str>::UnknownColony.id(), None);
    }

    #[test]
    fn a_citizen_id_of_zero_or_less_is_refused_and_writes_nothing() {
        let mut views: Views<Seen> = Views::new();
        for id in [0, -1, -700] {
            assert_eq!(
                views.apply(true, id, |previous| read_name("Nobody", previous)),
                Err(Refusal::NonPositiveId { id })
            );
        }
        assert!(views.is_empty(), "a visitor id never lands in the citizen map");
        assert_eq!(Refusal::<&str>::NonPositiveId { id: 0 }.id(), Some(0));
    }

    #[test]
    fn a_view_that_fails_to_decode_leaves_the_old_one_in_place() {
        let mut views = Views::new();
        views
            .apply(true, 4, |_| read_name("Alfred", None))
            .expect("created");
        let refused = views.apply(true, 4, |previous| read_name("", previous));
        assert_eq!(refused, Err(Refusal::DecodeFailed { id: 4, cause: "empty name" }));
        assert_eq!(
            views.get(4).map(|seen| seen.name.as_str()),
            Some("Alfred"),
            "Java logs and answers null (`:33-35`); the citizen is one message behind, not blank"
        );
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn one_bad_citizen_costs_one_citizen_and_not_the_list() {
        let mut views: Views<Seen> = Views::new();
        views
            .apply(true, 1, |_| read_name("Alfred", None))
            .expect("first");
        views
            .apply(true, 2, |_| read_name("Beatrice", None))
            .expect("second");
        assert!(views.apply(true, 3, |_| read_name("", None)).is_err());
        assert_eq!(views.ids().collect::<Vec<i32>>(), vec![1, 2], "`:31-36` loses that citizen only");
    }

    #[test]
    fn a_refreshed_batch_forgets_the_citizens_it_does_not_name() {
        let mut views: Views<Seen> = Views::new();
        for (id, name) in [(1, "Alfred"), (2, "Beatrice")] {
            views
                .apply(true, id, |_| read_name(name, None))
                .expect("created");
        }
        let kept = views.apply_batch(true, true, &[1], |_id, previous| read_name("Alfred", previous));
        assert_eq!(kept, Ok(1));
        assert_eq!(views.ids().collect::<Vec<i32>>(), vec![1], "`:834-838` cleared before writing");
    }

    #[test]
    fn a_batch_without_refresh_keeps_the_citizens_it_did_not_name() {
        let mut views: Views<Seen> = Views::new();
        views
            .apply(true, 1, |_| read_name("Alfred", None))
            .expect("created");
        let held = views.apply_batch(true, false, &[2], |_id, previous| read_name("Beatrice", previous));
        assert_eq!(held, Ok(2));
        assert_eq!(views.ids().collect::<Vec<i32>>(), vec![1, 2]);
        assert_eq!(
            views.get(2),
            Some(&Seen { name: "Beatrice".to_owned(), status: 0 })
        );
    }

    #[test]
    fn a_batch_that_fails_writes_nothing_not_even_the_clear() {
        let mut views: Views<Seen> = Views::new();
        views
            .apply(true, 1, |_| Ok::<_, &'static str>(Seen { name: "Alfred".to_owned(), status: 5 }))
            .expect("created");
        let refused = views.apply_batch(true, true, &[2, 3, 4], |id, previous| {
            if id == 3 {
                return Err("unreadable");
            }
            read_name("Newcomer", previous)
        });
        assert_eq!(refused, Err(Refusal::DecodeFailed { id: 3, cause: "unreadable" }));
        assert_eq!(
            views.ids().collect::<Vec<i32>>(),
            vec![1],
            "the clear waits for the last decode: Java's loop would have thrown on 3 with 2 already in"
        );
        assert_eq!(views.get(1).map(|seen| seen.status), Some(5), "and citizen 1 is untouched");
    }

    #[test]
    fn a_batch_of_non_positive_ids_is_refused_before_it_reads_one() {
        let mut views: Views<Seen> = Views::new();
        let refused = views.apply_batch(true, false, &[1, 0, 2], |_id, previous| {
            read_name("Alfred", previous)
        });
        assert_eq!(refused, Err(Refusal::NonPositiveId { id: 0 }));
        assert!(views.is_empty());
    }

    #[test]
    fn a_removal_takes_one_citizen_and_says_whether_it_was_there() {
        let mut views: Views<Seen> = Views::new();
        views
            .apply(true, 9, |_| read_name("Carol", None))
            .expect("created");
        assert!(views.remove(9));
        assert!(!views.remove(9), "`ColonyView.java:901` answers nothing, this port says so");
        assert!(views.is_empty());
    }

    #[test]
    fn the_registry_holds_the_port_s_own_citizen_record() {
        use crate::colony::Citizen;

        let mut views: Views<Citizen> = Views::new();
        let added = views.apply(true, 12, |previous| {
            assert!(previous.is_none(), "the client knows nobody yet");
            Ok::<Citizen, &'static str>(Citizen::new(12, "Dora"))
        });
        assert_eq!(added, Ok(false));
        assert_eq!(views.get(12).map(|citizen| citizen.name.as_str()), Some("Dora"));
    }
}
