//! A citizen's chat options: the seven kinds an interaction can be, which class
//! each one is rebuilt as, and what a bad `handlertype` costs.
//!
//! Java reference: `core/colony/interactionhandling/registry/InteractionResponseHandlerManager.java`
//! (49 lines), `api/colony/interactionhandling/ModInteractionResponseHandlers.java`
//! (the seven ids), `apiimp/initializer/ModInteractionsInitializer.java:19-56` (the
//! table, kind by kind) and `api/colony/interactionhandling/AbstractInteractionResponseHandler.java:118`
//! (what each one writes back).
//!
//! **The saved value is a bare path, and the reader puts the namespace on.**
//! `AbstractInteractionResponseHandler.serializeNBT` writes `getType()`, and every
//! `getType()` returns `ModInteractionResponseHandlers.X.getPath()` — `"pos"`,
//! not `"minecolonies:pos"` (`PosBasedInteraction.java:135-138`). `:25-27` then
//! builds `new ResourceLocation(Constants.MOD_ID, value)`, so the namespace is
//! **the reader's**, not the writer's. A value that already carries one
//! (`minecolonies:pos`) is a path containing a colon, which the constructor
//! refuses: the same throw as an empty value, and nothing between `:26` and
//! `CitizenData.java:1496` catches it *there* — the caller's `try` does, and
//! costs one quest interaction.
//!
//! **Two things the initialiser does that the class names do not suggest.**
//!
//! 1. **`simplenotification` is registered with `StandardInteraction`**
//!    (`ModInteractionsInitializer.java:26-30`), not with
//!    `SimpleNotificationInteraction`. So an interaction saved as
//!    `simplenotification` comes back as a `StandardInteraction`, whose
//!    `getType()` (`StandardInteraction.java:79-82`) writes `standard` — the
//!    value moves on the first save after the load. `SimpleNotificationInteraction`
//!    is produced by no entry at all.
//! 2. **The "unknown type" warning is unreachable for an unknown type.**
//!    `:28` does `getValue(handlerType).getProducer().apply(citizen)`: when the
//!    registry holds no entry, `getValue` answers `null` and `.getProducer()`
//!    throws a `NullPointerException` **before** the `if (handler != null)` at
//!    `:29`, so the `else` at `:45-48` can only be reached by a producer that
//!    answered `null`. The port separates the two: [`KindRefusal::Unknown`] is the
//!    registry miss, [`Refusal::NoProducer`] is the `null` producer.
//!
//! **The caller dereferences the `null`.** `CitizenData.java:1498-1502` casts
//! `createFrom`'s answer to `ServerCitizenInteraction` and calls `handler.getId()`
//! on it straight away: Java's `@Nullable` return is used as if it were not, and
//! the resulting `NullPointerException` is what the `catch` at `:1504-1507` logs
//! as "Did the quest vanish?". A port that returned an `Option` would have no
//! such sentence to explain; this one returns a [`Refusal`].

use mod_api::{Compound, Tag};

/// The key the kind is saved under (`NbtTagConstants.java:668`).
pub const KEY_HANDLER_TYPE: &str = "handlertype";

/// The key one chat option sits under inside the citizen's list
/// (`NbtTagConstants.java:497`).
pub const KEY_CHAT_OPTION: &str = "chatoption";

/// The key the list itself sits under (`NbtTagConstants.java:498`).
pub const KEY_CHAT_OPTIONS: &str = "chatoptions";

/// The namespace the reader puts on a bare path (`Constants.java:13`, used at
/// `InteractionResponseHandlerManager.java:26`).
pub const NAMESPACE: &str = "minecolonies";

/// The kind a citizen interaction is, with the path it is saved under
/// (`ModInteractionResponseHandlers.java:16-22`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: the registry's seven entries, and nothing else
pub enum Kind {
    Standard,
    SimpleNotification,
    Pos,
    Request,
    Recruitment,
    Quest,
    QuestAction,
}

impl Kind {
    /// Every kind, in the order the initialiser registers them
    /// (`ModInteractionsInitializer.java:21-55`).
    pub const ALL: &'static [Kind] = &[
        Kind::Standard,
        Kind::SimpleNotification,
        Kind::Pos,
        Kind::Request,
        Kind::Recruitment,
        Kind::Quest,
        Kind::QuestAction,
    ];

    /// The bare path (`ModInteractionResponseHandlers.java:16-22`).
    pub fn path(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::SimpleNotification => "simplenotification",
            Self::Pos => "pos",
            Self::Request => "request",
            Self::Recruitment => "recruitment",
            Self::Quest => "quest",
            Self::QuestAction => "questaction",
        }
    }

    /// The kind a bare path names, or `None`.
    pub fn from_path(path: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|kind| kind.path() == path)
    }

    /// The class the registry entry builds (`ModInteractionsInitializer.java:21-55`),
    /// as its Java name — this port has no interaction objects, so the class is
    /// data, and it is the only place the `simplenotification` quirk is visible.
    pub fn producer(self) -> &'static str {
        match self {
            Self::Standard | Self::SimpleNotification => "StandardInteraction",
            Self::Pos => "PosBasedInteraction",
            Self::Request => "RequestBasedInteraction",
            Self::Recruitment => "RecruitmentInteraction",
            Self::Quest => "QuestDialogueInteraction",
            Self::QuestAction => "QuestDeliveryInteraction",
        }
    }

    /// What the class `producer` writes back into `handlertype`
    /// (`AbstractInteractionResponseHandler.java:118` with each `getType()`).
    ///
    /// Equal to [`Self::path`] for six of the seven;`SimpleNotification` answers
    /// [`Kind::Standard`], because the entry builds a `StandardInteraction`.
    pub fn saved_as(self) -> Kind {
        match self.producer() {
            "StandardInteraction" => Kind::Standard,
            _ => self,
        }
    }
}

/// Why a saved kind did not resolve, named.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set for a name
pub enum KindRefusal {
    /// The key is present and empty: `new ResourceLocation("minecolonies", "")`
    /// throws at `:26-27`.
    Empty,
    /// The value carries characters a path may not hold — a colon, which is what
    /// a namespaced value does. The constructor throws; Java does not catch it
    /// here.
    BadPath { value: String },
    /// No registry entry for that kind (`:28`): Java dereferences the `null`
    /// registry answer and throws an NPE, which `CitizenData.java:1504` catches
    /// and logs as "Did the quest vanish?".
    Unknown { value: String },
}

impl KindRefusal {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "interaction.refusal.empty",
            Self::BadPath { .. } => "interaction.refusal.bad_path",
            Self::Unknown { .. } => "interaction.refusal.unknown",
        }
    }
}

/// Why no interaction was built, named.
///
/// `E` is the caller's own restore error: the handler's fields are its class's
/// business (`RequestBasedInteraction` alone reads a dozen keys).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: Java's three failures, with the one NPE it hides
pub enum Refusal<E> {
    /// The kind did not resolve.
    Kind(KindRefusal),
    /// The entry exists and its producer answered `null` (`:45-48`).
    NoProducer { kind: Kind },
    /// `deserializeNBT` threw (`:31-43`): "its state cannot be restored", and
    /// the answer is `null`, not a half-restored handler.
    StateLost { kind: Kind, cause: E },
}

impl<E> Refusal<E> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Kind(inner) => inner.name(),
            Self::NoProducer { .. } => "interaction.refusal.no_producer",
            Self::StateLost { .. } => "interaction.refusal.state_lost",
        }
    }

    /// The kind, when the value resolved to one.
    pub fn kind(&self) -> Option<Kind> {
        match self {
            Self::Kind(_) => None,
            Self::NoProducer { kind } | Self::StateLost { kind, .. } => Some(*kind),
        }
    }
}

/// The kind a saved compound names (`:25-27`): **an absent key is
/// [`Kind::Standard`]**, and a present one is a bare path the reader namespaces.
pub fn kind_of(compound: &Compound) -> Result<Kind, KindRefusal> {
    let value = match compound.get(&KEY_HANDLER_TYPE.into()) {
        None => return Ok(Kind::Standard),
        // A non-string reads as "" through `getString`, and "" throws.
        Some(Tag::String(value)) => value.to_string_lossy(),
        Some(_) => return Err(KindRefusal::Empty),
    };
    kind_named(&value)
}

/// `new ResourceLocation(NAMESPACE, value)` + the registry lookup (`:26-28`),
/// without the registry.
///
/// The path charset is vanilla's (`[a-z0-9_.-/]`); what this port checks is the
/// two things Java's own writers never produce and a hand-edited save does: an
/// empty value, and a colon inside it.
pub fn kind_named(value: &str) -> Result<Kind, KindRefusal> {
    if value.is_empty() {
        return Err(KindRefusal::Empty);
    }
    if value.contains(':') || value.contains('/') {
        return Err(KindRefusal::BadPath {
            value: value.to_owned(),
        });
    }
    Kind::from_path(value).ok_or_else(|| KindRefusal::Unknown {
        value: value.to_owned(),
    })
}

/// `createFrom` (`:24-49`), whole, with the handler's own decoding handed in.
///
/// The order is the value: **resolve the kind, build the handler, then restore
/// it**. A value that does not resolve builds nothing, and a producer that
/// answers `null` restores nothing.
pub fn load<H, E>(
    compound: &Compound,
    produce: impl FnOnce(Kind) -> Option<H>,
    restore: impl FnOnce(&mut H, &Compound) -> Result<(), E>,
) -> Result<H, Refusal<E>> {
    let kind = kind_of(compound).map_err(Refusal::Kind)?;
    let mut built = produce(kind).ok_or(Refusal::NoProducer { kind })?;
    match restore(&mut built, compound) {
        Ok(()) => Ok(built),
        Err(cause) => Err(Refusal::StateLost { kind, cause }),
    }
}

/// One chat option of a citizen's list (`CitizenData.java:1491-1509`), as the
/// caller reads it: the compound under [`KEY_CHAT_OPTION`] and the id the
/// handler carries (`:1502`).
///
/// **One refused option costs that option, not the citizen** (`:1496-1508`).
/// Java catches every `Exception` per element — which is also how the NPE of an
/// unknown kind survives as a log line — and keeps reading the rest.
pub fn load_options<H, E>(
    citizen: &Compound,
    produce: impl Fn(Kind) -> Option<H>,
    mut restore: impl FnMut(&mut H, &Compound) -> Result<(), E>,
) -> (Vec<H>, Vec<Refusal<E>>) {
    let mut built = Vec::new();
    let mut refused = Vec::new();
    for tag in chat_options(citizen) {
        match load(tag, &produce, &mut restore) {
            Ok(handler) => built.push(handler),
            Err(refusal) => refused.push(refusal),
        }
    }
    (built, refused)
}

/// The compounds of the `chatoptions` list, each already unwrapped from its
/// `chatoption` key (`:1493-1501`), in order. A missing list is empty.
pub fn chat_options(citizen: &Compound) -> Vec<&Compound> {
    let Some(Tag::List(entries)) = citizen.get(&KEY_CHAT_OPTIONS.into()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::Compound(entry) => entry.get(&KEY_CHAT_OPTION.into()),
            _ => None,
        })
        .filter_map(|inner| match inner {
            Tag::Compound(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(value: &str) -> Compound {
        Compound::from([(KEY_HANDLER_TYPE.into(), Tag::String(value.into()))])
    }

    #[test]
    fn an_absent_type_is_the_standard_interaction() {
        // `:25-27`: the fallback is `ModInteractionResponseHandlers.STANDARD`,
        // not a refusal.
        assert_eq!(kind_of(&Compound::new()), Ok(Kind::Standard));
        assert_eq!(kind_of(&saved("standard")), Ok(Kind::Standard));
    }

    #[test]
    fn the_seven_kinds_and_the_class_that_rebuilds_each_one() {
        assert_eq!(Kind::ALL.len(), 7);
        // The initialiser's table, entry by entry.
        assert_eq!(Kind::Standard.producer(), "StandardInteraction");
        assert_eq!(Kind::Pos.producer(), "PosBasedInteraction");
        assert_eq!(Kind::Request.producer(), "RequestBasedInteraction");
        assert_eq!(Kind::Recruitment.producer(), "RecruitmentInteraction");
        assert_eq!(Kind::Quest.producer(), "QuestDialogueInteraction");
        assert_eq!(Kind::QuestAction.producer(), "QuestDeliveryInteraction");

        // The quirk: `simplenotification` is registered with `StandardInteraction`
        // (`ModInteractionsInitializer.java:26-30`), so it comes back as one and
        // is re-saved as `standard`.
        assert_eq!(Kind::SimpleNotification.producer(), "StandardInteraction");
        assert_eq!(Kind::SimpleNotification.saved_as(), Kind::Standard);
        for kind in [Kind::Standard, Kind::Pos, Kind::Request, Kind::Recruitment, Kind::Quest, Kind::QuestAction] {
            assert_eq!(kind.saved_as(), kind, "{kind:?} is saved as itself");
        }

        // And every path resolves back to its kind.
        for kind in Kind::ALL {
            assert_eq!(Kind::from_path(kind.path()), Some(*kind));
        }
    }

    #[test]
    fn a_namespaced_or_empty_value_is_the_throw_java_leaves_uncaught() {
        // `AbstractInteractionResponseHandler.java:118` writes a bare path, so a
        // namespace in the value can only come from a hand-edited save — and the
        // constructor refuses a colon.
        assert_eq!(
            kind_of(&saved("minecolonies:pos")),
            Err(KindRefusal::BadPath {
                value: "minecolonies:pos".to_owned()
            })
        );
        assert_eq!(
            kind_of(&saved("pos/x")),
            Err(KindRefusal::BadPath {
                value: "pos/x".to_owned()
            })
        );
        // A non-string reads as "" and throws at `:26-27`.
        assert_eq!(
            kind_of(&Compound::from([(KEY_HANDLER_TYPE.into(), Tag::Int(1))])),
            Err(KindRefusal::Empty)
        );
        assert_eq!(kind_of(&saved("")), Err(KindRefusal::Empty));
    }

    #[test]
    fn an_unknown_kind_and_a_null_producer_are_two_different_refusals() {
        // The registry miss: Java dereferences the null handler registry entry
        // at `:28` and never reaches the "unknown type" warn at `:45-48`.
        assert_eq!(
            kind_of(&saved("nosuchinteraction")),
            Err(KindRefusal::Unknown {
                value: "nosuchinteraction".to_owned()
            })
        );
        // The producer that answered null: this *is* the branch `:45-48` logs.
        assert_eq!(
            load::<i32, ()>(&saved("quest"), |_| None, |_, _| Ok(())),
            Err(Refusal::NoProducer { kind: Kind::Quest })
        );
        assert_eq!(
            Refusal::<()>::NoProducer { kind: Kind::Quest }.kind(),
            Some(Kind::Quest)
        );
        assert_eq!(KindRefusal::Empty.name(), "interaction.refusal.empty");
    }

    #[test]
    fn an_interaction_is_built_before_it_is_restored_and_neither_before_the_name_resolves() {
        let mut produced = 0;
        let mut restored = 0;

        let refused = load::<i32, ()>(
            &saved("nosuchinteraction"),
            |_| {
                produced += 1;
                Some(1)
            },
            |_, _| {
                restored += 1;
                Ok(())
            },
        );
        assert!(matches!(refused, Err(Refusal::Kind(KindRefusal::Unknown { .. }))));
        assert_eq!((produced, restored), (0, 0));

        let lost = load(
            &saved("recruitment"),
            |kind| {
                produced += 1;
                assert_eq!(kind, Kind::Recruitment);
                Some(String::from("handler"))
            },
            |handler: &mut String, compound: &Compound| {
                restored += 1;
                assert_eq!(compound, &saved("recruitment"));
                handler.push_str("-restored");
                Ok::<(), &str>(())
            },
        );
        assert_eq!(lost, Ok(String::from("handler-restored")));
        assert_eq!((produced, restored), (1, 1));

        let broken = load(
            &saved("pos"),
            |_| Some(0u8),
            |_, _| Err("field missing"),
        );
        assert_eq!(
            broken,
            Err(Refusal::StateLost {
                kind: Kind::Pos,
                cause: "field missing"
            })
        );
    }

    #[test]
    fn one_refused_chat_option_costs_that_option_and_the_rest_are_read() {
        let citizen = Compound::from([(
            KEY_CHAT_OPTIONS.into(),
            Tag::List(vec![
                Tag::Compound(Compound::from([(
                    KEY_CHAT_OPTION.into(),
                    Tag::Compound(saved("pos")),
                )])),
                // An unknown kind in the middle: skipped, not fatal.
                Tag::Compound(Compound::from([(
                    KEY_CHAT_OPTION.into(),
                    Tag::Compound(saved("nosuchinteraction")),
                )])),
                // A list element with no `chatoption` key: skipped silently.
                Tag::Compound(Compound::new()),
                Tag::Compound(Compound::from([(
                    KEY_CHAT_OPTION.into(),
                    Tag::Compound(saved("quest")),
                )])),
            ]),
        )]);

        let (built, refused) = load_options(&citizen, |kind| Some(kind.path().to_owned()), |_, _| {
            Ok::<(), ()>(())
        });
        assert_eq!(built, vec!["pos".to_owned(), "quest".to_owned()]);
        assert_eq!(
            refused,
            vec![Refusal::Kind(KindRefusal::Unknown {
                value: "nosuchinteraction".to_owned()
            })]
        );
        // A citizen with no list has no option, and that is not an error.
        assert!(chat_options(&Compound::new()).is_empty());
    }
}
