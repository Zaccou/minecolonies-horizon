//! The nine data stores of the request system: what they are keyed by, what a
//! read tolerates, and the two places the id is written.
//!
//! Java reference: `core/colony/requestsystem/data/StandardDataStoreManager.java`
//! (173 lines) — the container, not the six stores it holds
//! (`api/colony/requestsystem/data/`). Each store is itself a keyed map behind an
//! id (`IDataStore.java:9-20`), and this file owns the map of maps: **a token, a
//! store, and the four operations Java offers.**
//!
//! **`get` creates, and that is the whole lazy rule.** `get(id, factory)` answers
//! the store filed under `id`, and when there is none it builds one, **sets its
//! id to the key** and files it (`:48-54`). So a store's own id and the key it
//! sits under are the same value *on the creation path only*, which is exactly
//! why `StandardRequestManager.registerDataStore` can read the id back out of the
//! store it just asked for (`:145-149`). A store read from a save is the one case
//! where the two can differ, and the port reports it ([`Mend::InnerIdMismatch`]);
//! Java never compares them, so it cannot see the difference.
//!
//! **A save holds the token twice.** Each entry is `{Token, Value}` (`:101-108`)
//! and the `Value` is the store's own serialization, which writes its `Token`
//! again (`StandardRequestIdentitiesDataStore.java:101`). [`Book::write`] writes
//! both from the one field, so the two cannot drift in a file this port produces.
//!
//! **A read is tolerant, entry by entry.** Java wraps each entry in a
//! `try`/`catch` and logs what it could not take (`:122-131`): one broken store
//! costs that store, not the book. The port keeps the shape and adds the reason
//! ([`Skip`]), because "one entry was dropped" is not something a player can act
//! on but a log line that names it is.
//!
//! **Two keys Java's own order does not give.** The map is a `HashMap`
//! (`:30`, `:117`), so a save's entry order and the order `write` produces are
//! unspecified in Java. Both are token order here — the same divergence
//! `manager/mod.rs` states for the colonies.
//!
//! **What is not ported: the network half.** `serialize(controller, buffer)`
//! (`:137-145`) writes a count and then the pairs; `deserialize` reads them back
//! with the two `deserialize` calls *inside one `try`* (`:152-162`). When the
//! second of a pair throws, the first has already consumed bytes and the loop
//! goes on reading a stream that is now out of step — Java answers a book of
//! garbage rather than an error. There is nothing to port: this crate has no
//! packet buffer, and the stores' contents cross the wire as opaque bytes
//! (`src/network/building_modules.rs:592`). `MANQUANT.md` §3 says what a reader
//! would need to refuse instead.
//!
//! **One limit of the port's model, said here rather than hidden.** Java's token
//! is a `UUID` made of two `i64`s (`api/colony/requestsystem/token/AbstractTokenFactory.java:26-27`).
//! This port keys everything by the `i32` the rest of `crate::requests` uses
//! (`src/requests/request.rs:9`), and writes the pair as `{Id_MSB: 0, Id_LSB: n}`.
//! A token a real Java server wrote — any UUID whose top half is not zero — is
//! therefore **not expressible** here, and is skipped by name rather than
//! truncated into a different token.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use super::nbt;

/// A store's token, as this port keys it. See the module note: Java's is a UUID.
pub type Token = i32;

/// `NbtTagConstants.TAG_LIST` (`api/util/constant/NbtTagConstants.java:314`).
pub const TAG_LIST: &str = "List";
/// `NbtTagConstants.TAG_TOKEN` (`:236`).
pub const TAG_TOKEN: &str = "Token";
/// `NbtTagConstants.TAG_VALUE` (`:330`).
pub const TAG_VALUE: &str = "Value";
/// `AbstractTokenFactory.NBT_MSB` (`:26`).
pub const NBT_MSB: &str = "Id_MSB";
/// `AbstractTokenFactory.NBT_LSB` (`:27`).
pub const NBT_LSB: &str = "Id_LSB";

/// One store: its id, and everything it wants saved.
///
/// The body is opaque on purpose. Java's six stores each have their own factory
/// (`StandardRequestIdentitiesDataStore.java:88-137` and five siblings) and this
/// port models none of them: what a store *holds* belongs to whoever holds it,
/// and the two things the container needs are its token and its compound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    id: Token,
    body: Compound,
}

impl Store {
    /// The id Java reads with `getId()` (`:111` of the interface).
    pub fn id(&self) -> Token {
        self.id
    }

    /// The store's own fields, without its `Token`.
    pub fn body(&self) -> &Compound {
        &self.body
    }

    /// The store's own fields, to change them.
    pub fn body_mut(&mut self) -> &mut Compound {
        &mut self.body
    }
}

/// A store the read could not take, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skip {
    pub index: usize,
    pub reason: SkipReason,
}

/// Why one entry of the `List` was dropped (`:122-131`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// The entry is not a compound, so `getCompound(TAG_VALUE)` has nothing to
    /// read.
    NotACompound,
    /// No `Value`, or a `Value` that is not a compound: Java's
    /// `controller.deserialize` throws on the empty compound and the `catch`
    /// drops the entry.
    NoValue,
    /// A token this port cannot express: a UUID whose top half is not zero, or a
    /// bottom half outside `i32`. Java keeps it; the port cannot, and says so
    /// instead of truncating it into another token.
    NotExpressible { msb: i64, lsb: i64 },
}

impl SkipReason {
    pub fn name(self) -> &'static str {
        match self {
            Self::NotACompound => "datastores.skip.not_a_compound",
            Self::NoValue => "datastores.skip.no_value",
            Self::NotExpressible { .. } => "datastores.skip.not_expressible",
        }
    }
}

/// A repair: the entry was kept, with something Java would have kept differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two Java defaults and the one inconsistency the port can see
pub enum Mend {
    /// No `Token` on the entry, or a `Token` that is not a compound: Java's
    /// `getCompound` answers an empty one and `getLong` answers `0`, so the store
    /// files itself under the zero token (`:124`).
    DefaultToken { index: usize },
    /// Two entries under one token: Java's `storeMap.put` keeps the last
    /// (`:126`), which the port does too, and says so.
    Duplicate { token: Token },
    /// The entry's key and the store's own `Token` disagree. See the module note:
    /// lookup uses the key, `getId()` answers the inner one, and Java never
    /// compares them.
    InnerIdMismatch { key: Token, inner: Token },
}

impl Mend {
    pub fn name(self) -> &'static str {
        match self {
            Self::DefaultToken { .. } => "datastores.mend.default_token",
            Self::Duplicate { .. } => "datastores.mend.duplicate_token",
            Self::InnerIdMismatch { .. } => "datastores.mend.inner_token",
        }
    }
}

/// What a read had to report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    /// Entries the book does not hold, each with the reason it is not there.
    pub skipped: Vec<Skip>,
    /// Entries the book holds, each with what was odd about it.
    pub mends: Vec<Mend>,
}

/// The stores a request system runs on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Book {
    stores: BTreeMap<Token, Store>,
}

impl Book {
    pub fn new() -> Book {
        Book { stores: BTreeMap::new() }
    }

    pub fn len(&self) -> usize {
        self.stores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stores.is_empty()
    }

    pub fn get(&self, token: Token) -> Option<&Store> {
        self.stores.get(&token)
    }

    /// The stores, in token order — Java's `HashMap` order is unspecified
    /// (`:30`).
    pub fn iter(&self) -> impl Iterator<Item = &Store> {
        self.stores.values()
    }

    /// `get(id, factory)` (`:40-57`): the store filed under `token`, created with
    /// that id when there is none.
    ///
    /// The creation is three writes in Java — build, `setId(id)`, `put` — and
    /// they happen in that order (`:50-53`), so a factory that throws leaves the
    /// book untouched. There is no factory here and nothing can throw, which is
    /// why the signature has no `Result`: the port creates an empty body, and
    /// filling it is the caller's business.
    pub fn get_or_create(&mut self, token: Token) -> &mut Store {
        self.stores
            .entry(token)
            .or_insert_with(|| Store { id: token, body: Compound::new() })
    }

    /// `remove` (`:60-63`): silent in Java even when nothing was there; `true`
    /// here for the caller that wants to know.
    pub fn remove(&mut self, token: Token) -> bool {
        self.stores.remove(&token).is_some()
    }

    /// `removeAll` (`:66-69`).
    pub fn clear(&mut self) {
        self.stores.clear();
    }

    /// `serialize` (`:96-111`): one `List` of `{Token, Value}`.
    ///
    /// The `Value` repeats the token, because Java's `Value` is the store's own
    /// serialization and that serialization starts with its `Token`
    /// (`StandardRequestIdentitiesDataStore.java:101`).
    pub fn write(&self) -> Compound {
        let entries: Vec<Compound> = self
            .stores
            .values()
            .map(|store| {
                let mut value = Compound::from([(TAG_TOKEN.into(), token_tag(store.id))]);
                for (key, tag) in &store.body {
                    value.insert(key.clone(), tag.clone());
                }
                Compound::from([
                    (TAG_TOKEN.into(), token_tag(store.id)),
                    (TAG_VALUE.into(), Tag::Compound(value)),
                ])
            })
            .collect();
        Compound::from([(TAG_LIST.into(), nbt::compound_list(entries))])
    }

    /// `deserialize` (`:113-135`): every entry it can take, and a report on the
    /// ones it cannot.
    pub fn read(compound: &Compound) -> (Book, Loaded) {
        let mut book = Book::new();
        let mut loaded = Loaded::default();
        let entries = nbt::list(compound, TAG_LIST).map(Vec::as_slice).unwrap_or_default();
        for (index, item) in entries.iter().enumerate() {
            let Tag::Compound(entry) = item else {
                loaded.skipped.push(Skip { index, reason: SkipReason::NotACompound });
                continue;
            };
            let Some(value) = nbt::compound(entry, TAG_VALUE) else {
                loaded.skipped.push(Skip { index, reason: SkipReason::NoValue });
                continue;
            };
            let token = match entry.get(&TAG_TOKEN.into()) {
                Some(tag) => match token_of(tag) {
                    Ok(token) => token,
                    Err((msb, lsb)) => {
                        loaded
                            .skipped
                            .push(Skip { index, reason: SkipReason::NotExpressible { msb, lsb } });
                        continue;
                    }
                },
                None => {
                    loaded.mends.push(Mend::DefaultToken { index });
                    0
                }
            };
            if let Some(inner) = value.get(&TAG_TOKEN.into()).and_then(|tag| token_of(tag).ok()) {
                if inner != token {
                    loaded.mends.push(Mend::InnerIdMismatch { key: token, inner });
                }
            }
            let mut body = value.clone();
            body.remove(&TAG_TOKEN.into());
            if book.stores.insert(token, Store { id: token, body }).is_some() {
                loaded.mends.push(Mend::Duplicate { token });
            }
        }
        (book, loaded)
    }
}

/// The token a tag holds, in Java's two halves (`AbstractTokenFactory.java:44-73`),
/// or the pair this port cannot express. Shared with `recipes`, which saves the
/// same token shape.
pub(super) fn token_of(tag: &Tag) -> Result<Token, (i64, i64)> {
    // `getCompound` answers an empty compound for a key that is absent or holds
    // something else, and `getLong` then answers 0 (`:66-67`).
    let empty = Compound::new();
    let inner = match tag {
        Tag::Compound(inner) => inner,
        _ => &empty,
    };
    let msb = nbt::long(inner, NBT_MSB).unwrap_or(0);
    let lsb = nbt::long(inner, NBT_LSB).unwrap_or(0);
    if msb != 0 {
        return Err((msb, lsb));
    }
    match Token::try_from(lsb) {
        Ok(token) => Ok(token),
        Err(_) => Err((msb, lsb)),
    }
}

/// A token written Java's way: two longs, the top one zero.
pub(super) fn token_tag(token: Token) -> Tag {
    Tag::compound([
        (NBT_MSB, Tag::Long(0)),
        (NBT_LSB, Tag::Long(i64::from(token))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(tokens: &[Token]) -> Book {
        let mut book = Book::new();
        for token in tokens {
            book.get_or_create(*token).body_mut().insert("n".into(), Tag::Int(*token));
        }
        book
    }

    #[test]
    fn get_creates_once_and_sets_the_id_to_the_key() {
        let mut book = Book::new();
        assert_eq!(book.len(), 0);
        assert_eq!(book.get_or_create(7).id(), 7, ":52 setId(id) before the put");
        assert_eq!(book.len(), 1);
        book.get_or_create(7).body_mut().insert("kept".into(), Tag::Int(1));
        assert_eq!(
            book.get_or_create(7).body().get(&"kept".into()),
            Some(&Tag::Int(1)),
            ":56 returns the stored instance, not a fresh one"
        );
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn remove_and_clear_answer_what_java_does_silently() {
        let mut book = book(&[1, 2]);
        assert!(book.remove(1));
        assert!(!book.remove(1), ":62 removes whatever is there, nothing otherwise");
        assert_eq!(book.len(), 1);
        book.clear();
        assert!(book.is_empty(), ":68");
    }

    #[test]
    fn a_save_writes_the_token_twice_and_reads_it_back() {
        let book = book(&[4, 9]);
        let saved = book.write();
        let entries = nbt::compounds(&saved, TAG_LIST);
        assert_eq!(entries.len(), 2);
        // Java's own shape: the key, and the same token inside the value.
        assert_eq!(
            entries[0].get(&TAG_TOKEN.into()),
            Some(&token_tag(4)),
            ":104 the entry's key"
        );
        assert_eq!(
            nbt::compound(entries[0], TAG_VALUE)
                .expect("a value")
                .get(&TAG_TOKEN.into()),
            Some(&token_tag(4)),
            "StandardRequestIdentitiesDataStore.java:101"
        );
        let (read, loaded) = Book::read(&saved);
        assert_eq!(read, book, "a round trip keeps every store and body");
        assert_eq!(loaded, Loaded::default());
    }

    #[test]
    fn one_broken_entry_costs_that_entry_and_not_the_book() {
        let mut entry = Compound::from([
            (TAG_TOKEN.into(), token_tag(1)),
            (TAG_VALUE.into(), Tag::Compound(Compound::from([(TAG_TOKEN.into(), token_tag(1))]))),
        ]);
        let good = entry.clone();
        entry.insert(TAG_VALUE.into(), Tag::Int(3));
        let saved = Compound::from([(
            TAG_LIST.into(),
            Tag::List(vec![Tag::Compound(entry), Tag::Int(0), Tag::Compound(good)]),
        )]);
        let (read, loaded) = Book::read(&saved);
        assert_eq!(read.len(), 1, ":128-131 catches per entry");
        assert_eq!(
            loaded.skipped,
            vec![
                Skip { index: 0, reason: SkipReason::NoValue },
                Skip { index: 1, reason: SkipReason::NotACompound },
            ]
        );
    }

    #[test]
    fn a_token_this_port_cannot_express_is_skipped_not_truncated() {
        let token = Tag::compound([(NBT_MSB, Tag::Long(0x1234)), (NBT_LSB, Tag::Long(2))]);
        let entry = Compound::from([
            (TAG_TOKEN.into(), token),
            (TAG_VALUE.into(), Tag::Compound(Compound::new())),
        ]);
        let saved = Compound::from([(TAG_LIST.into(), Tag::List(vec![Tag::Compound(entry)]))]);
        let (read, loaded) = Book::read(&saved);
        assert!(read.is_empty());
        assert_eq!(
            loaded.skipped,
            vec![Skip {
                index: 0,
                reason: SkipReason::NotExpressible { msb: 0x1234, lsb: 2 }
            }]
        );
    }

    #[test]
    fn two_entries_under_one_token_keep_the_last_as_java_does() {
        let first = Compound::from([
            (TAG_TOKEN.into(), token_tag(5)),
            (TAG_VALUE.into(), Tag::Compound(Compound::from([("first".into(), Tag::Int(1))]))),
        ]);
        let second = Compound::from([
            (TAG_TOKEN.into(), token_tag(5)),
            (TAG_VALUE.into(), Tag::Compound(Compound::from([("second".into(), Tag::Int(2))]))),
        ]);
        let saved = Compound::from([(
            TAG_LIST.into(),
            Tag::List(vec![Tag::Compound(first), Tag::Compound(second)]),
        )]);
        let (read, loaded) = Book::read(&saved);
        assert_eq!(read.len(), 1);
        assert_eq!(
            read.get(5).expect("kept").body().get(&"second".into()),
            Some(&Tag::Int(2)),
            ":126 put overwrites"
        );
        assert_eq!(loaded.mends, vec![Mend::Duplicate { token: 5 }]);
    }

    #[test]
    fn a_missing_token_files_the_store_under_the_zero_token_as_java_does() {
        let entry = Compound::from([(TAG_VALUE.into(), Tag::Compound(Compound::new()))]);
        let saved = Compound::from([(TAG_LIST.into(), Tag::List(vec![Tag::Compound(entry)]))]);
        let (read, loaded) = Book::read(&saved);
        assert_eq!(read.get(0).map(Store::id), Some(0), ":124 an empty compound reads as 0");
        assert_eq!(loaded.mends, vec![Mend::DefaultToken { index: 0 }]);
        assert!(loaded.skipped.is_empty());
    }

    #[test]
    fn a_store_whose_key_and_inner_token_disagree_is_reported_and_looked_up_by_its_key() {
        let entry = Compound::from([
            (TAG_TOKEN.into(), token_tag(3)),
            (
                TAG_VALUE.into(),
                Tag::Compound(Compound::from([(TAG_TOKEN.into(), token_tag(8))])),
            ),
        ]);
        let saved = Compound::from([(TAG_LIST.into(), Tag::List(vec![Tag::Compound(entry)]))]);
        let (read, loaded) = Book::read(&saved);
        assert_eq!(read.len(), 1);
        assert_eq!(read.get(3).map(Store::id), Some(3), "lookup uses the key");
        assert_eq!(
            loaded.mends,
            vec![Mend::InnerIdMismatch { key: 3, inner: 8 }]
        );
    }

    #[test]
    fn every_skip_and_mend_has_its_own_name() {
        let names = [
            SkipReason::NotACompound.name(),
            SkipReason::NoValue.name(),
            SkipReason::NotExpressible { msb: 1, lsb: 2 }.name(),
            Mend::DefaultToken { index: 0 }.name(),
            Mend::Duplicate { token: 0 }.name(),
            Mend::InnerIdMismatch { key: 1, inner: 2 }.name(),
        ];
        for (index, name) in names.iter().enumerate() {
            for other in &names[index + 1..] {
                assert_ne!(name, other, "two names collide, so a log line lies");
            }
            assert!(name.starts_with("datastores."), "{name} is not in this namespace");
        }
    }
}
