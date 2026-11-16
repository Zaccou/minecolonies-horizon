//! NBT: reading a `.blueprint`'s document, and writing a structure's.
//!
//! **Why this file exists at all.** The engine has an NBT reader and writer
//! (`crates/horizon-core/src/nbt.rs`, `read_named` at `:743`, `write_named` at
//! `:1026`), but a mod cannot reach them: the API re-exports the *tree*
//! (`mod_api::{Compound, Tag}`, `crates/horizon-mod-vocab/src/exports/
//! horizon_world.rs:7-8`) and one error type
//! (`crates/horizon-mod-vocab/src/exports/horizon_core.rs:13`), not the byte
//! layer. A blueprint arrives as bytes — `Host::resource` hands over the file
//! (`crates/horizon-mod-vocab/src/host/resources.rs:20`) — so this module is the
//! byte layer, in the same shape as the engine's: same tag ids, same strings,
//! same limits. `MANQUANT.md` §1 asks for the re-export that would delete it.
//!
//! **The tree is the engine's, not a copy.** [`Tag`] and [`Compound`] are
//! `mod_api`'s: `Compound` is `BTreeMap<JavaString, Tag>` and a blueprint's
//! block entities are `Compound`s this reader produces and the converter hands
//! straight back to the game (`StructureTemplateBlock.nbt`,
//! `crates/horizon-mod-vocab/src/world/structure_template.rs:47-54`). Porting the
//! standalone crate's own tree would have meant translating every payload
//! twice for nothing.
//!
//! **Two spellings of one string.** Java writes NBT strings with
//! `DataOutput.writeUTF`: *modified* UTF-8, one to three bytes per UTF-16 code
//! unit, with U+0000 as `C0 80` and a supplementary character as its two
//! surrogates (six bytes). The engine decodes that at
//! `crates/horizon-core/src/nbt.rs:709-739` and encodes it at `:892-921`; this
//! file does the same and says so, because standard UTF-8 would round-trip
//! differently on those two inputs. One asymmetry is deliberate: the reader
//! hands back a Rust `String` (`JavaString` is not re-exported, so it cannot be
//! named here), which means an unpaired surrogate is replaced by U+FFFD. No
//! blueprint in the corpus has one — block ids, property names and values,
//! keys and paths are ASCII.
//!
//! **Format reference.** <https://minecraft.wiki/w/NBT_format>, tag ids as in
//! `net.minecraft.nbt.TagTypes`: 1 byte, 2 short, 3 int, 4 long, 5 float,
//! 6 double, 7 byte array, 8 string, 9 list, 10 compound, 11 int array,
//! 12 long array; 0 ends a compound and types nothing else.
//!
//! **Floats and doubles travel as their bits.** `Tag::Float(u32)` and
//! `Tag::Double(u64)` hold IEEE bit patterns (engine's choice, see
//! `crates/horizon-core/src/nbt.rs:39-40`), so this file never converts them:
//! a position written by Structurize comes back byte for byte. Arithmetic on
//! them is the caller's business ([`as_f64`]).

use mod_api::{Compound, Tag};

/// Deepest nesting this reader accepts. Same guard as the engine's
/// (`crates/horizon-core/src/nbt.rs:193`), and no real file comes near it: the
/// deepest path in the corpus is a block entity inside a list inside the root,
/// four levels.
pub const MAX_DEPTH: usize = 512;

/// A malformed document, with the byte offset it failed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NbtError {
    pub offset: usize,
    pub kind: NbtErrorKind,
}

/// What is wrong with a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NbtErrorKind {
    /// Ran off the end of the buffer.
    Truncated,
    /// A tag id outside `0..=12`.
    UnknownTag(u8),
    /// A list or array declared a negative length.
    NegativeLength(i32),
    /// The root tag is not a compound. `NbtIo.read` requires one.
    RootNotCompound(u8),
    /// A list header declared element type `TAG_End` with a non-zero length.
    ListOfEnd(i32),
    /// Nesting deeper than [`MAX_DEPTH`].
    TooDeep,
    /// A list whose elements are not all of one type (write side: NBT lists are
    /// homogeneous).
    HeterogeneousList,
    /// A string whose modified-UTF-8 form does not fit a `u16` byte length.
    BadStringLength(usize),
    /// [`Tag::End`] where a value is required.
    EndAsValue,
}

impl std::fmt::Display for NbtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NBT error at byte {}: ", self.offset)?;
        match &self.kind {
            NbtErrorKind::Truncated => write!(f, "unexpected end of data"),
            NbtErrorKind::UnknownTag(id) => write!(f, "unknown tag id {id}"),
            NbtErrorKind::NegativeLength(len) => write!(f, "negative list length {len}"),
            NbtErrorKind::RootNotCompound(id) => {
                write!(f, "root tag is id {id}, expected a compound (10)")
            }
            NbtErrorKind::ListOfEnd(len) => {
                write!(f, "a list of TAG_End claims {len} elements")
            }
            NbtErrorKind::TooDeep => write!(f, "nesting deeper than {MAX_DEPTH}"),
            NbtErrorKind::HeterogeneousList => {
                write!(f, "a list holds elements of more than one type")
            }
            NbtErrorKind::BadStringLength(len) => {
                write!(f, "a string needs {len} bytes, more than a u16 length allows")
            }
            NbtErrorKind::EndAsValue => write!(f, "TAG_End is not a value"),
        }
    }
}

impl std::error::Error for NbtError {}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Parse a complete document: one root compound and its name.
///
/// The name is `""` in every blueprint (Structurize never sets it,
/// `blueprints/v1/BlueprintUtil.java:626` writes whatever
/// `writeBlueprintToNBT` returns, whose root tag is `new CompoundTag()`).
pub fn parse(bytes: &[u8]) -> Result<(String, Compound), NbtError> {
    let mut reader = Reader { data: bytes, pos: 0 };
    let id = reader.u8()?;
    if id != 10 {
        return Err(reader.error(NbtErrorKind::RootNotCompound(id)));
    }
    let name = reader.string()?;
    let root = reader.compound_payload(1)?;
    Ok((name, root))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn error(&self, kind: NbtErrorKind) -> NbtError {
        NbtError {
            offset: self.pos,
            kind,
        }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], NbtError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| self.error(NbtErrorKind::Truncated))?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| self.error(NbtErrorKind::Truncated))?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, NbtError> {
        Ok(self.take(1)?[0])
    }

    fn i16_be(&mut self) -> Result<i16, NbtError> {
        let bytes = self.take(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn i32_be(&mut self) -> Result<i32, NbtError> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn i64_be(&mut self) -> Result<i64, NbtError> {
        let bytes = self.take(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// A length prefix. NBT lengths are signed ints; a negative one is a
    /// malformed file, not an empty collection.
    fn length(&mut self) -> Result<usize, NbtError> {
        let len = self.i32_be()?;
        if len < 0 {
            return Err(self.error(NbtErrorKind::NegativeLength(len)));
        }
        Ok(len as usize)
    }

    /// A string: a `u16` *byte* length, then modified UTF-8.
    fn string(&mut self) -> Result<String, NbtError> {
        let header = self.take(2)?;
        let len = u16::from_be_bytes([header[0], header[1]]) as usize;
        let bytes = self.take(len)?;
        // Modified UTF-8 first; a file that is plain UTF-8 (some editors write
        // one) still reads, just without the surrogate pairing.
        match modified_utf8(bytes) {
            Some(units) => Ok(String::from_utf16_lossy(&units)),
            None => Ok(String::from_utf8_lossy(bytes).into_owned()),
        }
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag, NbtError> {
        if depth > MAX_DEPTH {
            return Err(self.error(NbtErrorKind::TooDeep));
        }
        Ok(match id {
            0 => return Err(self.error(NbtErrorKind::EndAsValue)),
            1 => Tag::Byte(self.take(1)?[0] as i8),
            2 => Tag::Short(self.i16_be()?),
            3 => Tag::Int(self.i32_be()?),
            4 => Tag::Long(self.i64_be()?),
            // Bits, not numbers: see the module docs.
            5 => Tag::Float(u32::from_be_bytes({
                let b = self.take(4)?;
                [b[0], b[1], b[2], b[3]]
            })),
            6 => Tag::Double(u64::from_be_bytes({
                let b = self.take(8)?;
                [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]
            })),
            7 => {
                let len = self.length()?;
                Tag::ByteArray(self.take(len)?.to_vec())
            }
            8 => {
                let text = self.string()?;
                Tag::String(text.as_str().into())
            }
            9 => {
                let element = self.u8()?;
                let len = self.length()?;
                if element == 0 {
                    if len != 0 {
                        return Err(self.error(NbtErrorKind::ListOfEnd(len as i32)));
                    }
                    Tag::List(Vec::new())
                } else {
                    if element > 12 {
                        return Err(self.error(NbtErrorKind::UnknownTag(element)));
                    }
                    // Every element takes at least one byte: a length that
                    // cannot fit in what is left is a truncated file.
                    if len > self.data.len() - self.pos {
                        return Err(self.error(NbtErrorKind::Truncated));
                    }
                    let mut items = Vec::with_capacity(len.min(1 << 16));
                    for _ in 0..len {
                        items.push(self.payload(element, depth + 1)?);
                    }
                    Tag::List(items)
                }
            }
            10 => Tag::Compound(self.compound_payload(depth + 1)?),
            11 => {
                let len = self.length()?;
                if len > (self.data.len() - self.pos) / 4 {
                    return Err(self.error(NbtErrorKind::Truncated));
                }
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(self.i32_be()?);
                }
                Tag::IntArray(values)
            }
            12 => {
                let len = self.length()?;
                if len > (self.data.len() - self.pos) / 8 {
                    return Err(self.error(NbtErrorKind::Truncated));
                }
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(self.i64_be()?);
                }
                Tag::LongArray(values)
            }
            other => return Err(self.error(NbtErrorKind::UnknownTag(other))),
        })
    }

    fn compound_payload(&mut self, depth: usize) -> Result<Compound, NbtError> {
        if depth > MAX_DEPTH {
            return Err(self.error(NbtErrorKind::TooDeep));
        }
        let mut compound = Compound::new();
        loop {
            let id = self.u8()?;
            if id == 0 {
                return Ok(compound);
            }
            if id > 12 {
                return Err(self.error(NbtErrorKind::UnknownTag(id)));
            }
            let name = self.string()?;
            let tag = self.payload(id, depth)?;
            compound.insert(name.as_str().into(), tag);
        }
    }
}

/// `DataInput.readUTF`'s decoding: one to three bytes per UTF-16 code unit,
/// overlong forms accepted, `None` for anything else.
///
/// Same rule as `crates/horizon-core/src/nbt.rs:709-739`, and the reason this
/// is not `str::from_utf8`: Java writes U+0000 as `C0 80` and a supplementary
/// character as two three-byte surrogates, neither of which is valid UTF-8.
fn modified_utf8(bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.is_ascii() {
        return Some(bytes.iter().map(|&byte| u16::from(byte)).collect());
    }
    let mut units = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        let (unit, size) = match byte {
            0..=127 => (u16::from(byte), 1),
            192..=223 if bytes.get(at + 1).is_some_and(|next| next & 0xc0 == 0x80) => (
                (u16::from(byte & 0x1f) << 6) | u16::from(bytes[at + 1] & 0x3f),
                2,
            ),
            224..=239
                if bytes.get(at + 1).is_some_and(|next| next & 0xc0 == 0x80)
                    && bytes.get(at + 2).is_some_and(|next| next & 0xc0 == 0x80) =>
            {
                (
                    (u16::from(byte & 0x0f) << 12)
                        | (u16::from(bytes[at + 1] & 0x3f) << 6)
                        | u16::from(bytes[at + 2] & 0x3f),
                    3,
                )
            }
            _ => return None,
        };
        units.push(unit);
        at += size;
    }
    Some(units)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Serialise a compound as a complete document: `TAG_Compound`, its name, its
/// payload, then the terminating `TAG_End`.
///
/// The result is what `NbtIo.write` produces — one uncompressed document. A
/// `.nbt` *file* is that inside gzip; [`crate::blueprint::gzip::gzip`] is the
/// container, and [`crate::blueprint::structure_nbt::Conversion::to_gzipped_nbt`]
/// is the pair.
pub fn write(name: &str, root: &Compound) -> Result<Vec<u8>, NbtError> {
    let mut out = Vec::with_capacity(4096);
    out.push(10);
    let units: Vec<u16> = name.encode_utf16().collect();
    string_units(&mut out, &units)?;
    compound_payload(&mut out, root)?;
    Ok(out)
}

fn fail(offset: usize, kind: NbtErrorKind) -> NbtError {
    NbtError { offset, kind }
}

/// A length prefix, or the error that says the tree cannot be written.
fn count(out: &mut Vec<u8>, len: usize) -> Result<(), NbtError> {
    let signed = i32::try_from(len)
        .map_err(|_| fail(out.len(), NbtErrorKind::NegativeLength(i32::MAX)))?;
    out.extend_from_slice(&signed.to_be_bytes());
    Ok(())
}

/// One string, byte length first, in modified UTF-8.
fn string_units(out: &mut Vec<u8>, units: &[u16]) -> Result<(), NbtError> {
    let len: usize = units
        .iter()
        .map(|&unit| match unit {
            1..=127 => 1,
            0..=2047 => 2,
            _ => 3,
        })
        .sum();
    let Ok(len) = u16::try_from(len) else {
        return Err(fail(out.len(), NbtErrorKind::BadStringLength(len)));
    };
    out.extend_from_slice(&len.to_be_bytes());
    for &unit in units {
        match unit {
            1..=127 => out.push(unit as u8),
            0..=2047 => out.extend_from_slice(&[
                0xc0 | (unit >> 6) as u8,
                0x80 | (unit & 0x3f) as u8,
            ]),
            _ => out.extend_from_slice(&[
                0xe0 | (unit >> 12) as u8,
                0x80 | ((unit >> 6) & 0x3f) as u8,
                0x80 | (unit & 0x3f) as u8,
            ]),
        }
    }
    Ok(())
}

fn compound_payload(out: &mut Vec<u8>, compound: &Compound) -> Result<(), NbtError> {
    for (key, tag) in compound {
        if tag.id() == 0 {
            return Err(fail(out.len(), NbtErrorKind::EndAsValue));
        }
        out.push(tag.id());
        // `JavaString`'s units, without naming the type: the API re-exports
        // `Tag` and `Compound`, and its field is public (`horizon-core/src/nbt.rs:6`).
        string_units(out, &key.0)?;
        payload(out, tag)?;
    }
    out.push(0);
    Ok(())
}

fn payload(out: &mut Vec<u8>, tag: &Tag) -> Result<(), NbtError> {
    match tag {
        Tag::End => Err(fail(out.len(), NbtErrorKind::EndAsValue)),
        Tag::Byte(value) => {
            out.push(*value as u8);
            Ok(())
        }
        Tag::Short(value) => {
            out.extend_from_slice(&value.to_be_bytes());
            Ok(())
        }
        Tag::Int(value) => {
            out.extend_from_slice(&value.to_be_bytes());
            Ok(())
        }
        Tag::Long(value) => {
            out.extend_from_slice(&value.to_be_bytes());
            Ok(())
        }
        Tag::Float(bits) => {
            out.extend_from_slice(&bits.to_be_bytes());
            Ok(())
        }
        Tag::Double(bits) => {
            out.extend_from_slice(&bits.to_be_bytes());
            Ok(())
        }
        Tag::ByteArray(values) => {
            count(out, values.len())?;
            out.extend_from_slice(values);
            Ok(())
        }
        Tag::String(units) => string_units(out, &units.0),
        Tag::List(items) => {
            // NBT lists are homogeneous, and an empty list is element type 0 —
            // which is what Java's `new ListTag()` writes (engine's writer,
            // `crates/horizon-core/src/nbt.rs:977-989`, does the same).
            let element = match items.first() {
                None => 0,
                Some(first) => {
                    let id = first.id();
                    if items.iter().any(|item| item.id() != id) {
                        return Err(fail(out.len(), NbtErrorKind::HeterogeneousList));
                    }
                    id
                }
            };
            if element == 0 && !items.is_empty() {
                return Err(fail(out.len(), NbtErrorKind::EndAsValue));
            }
            out.push(element);
            count(out, items.len())?;
            for item in items {
                payload(out, item)?;
            }
            Ok(())
        }
        Tag::Compound(compound) => compound_payload(out, compound),
        Tag::IntArray(values) => {
            count(out, values.len())?;
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
            Ok(())
        }
        Tag::LongArray(values) => {
            count(out, values.len())?;
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Reading values out of a tree
// ---------------------------------------------------------------------------

/// A compound entry by name.
///
/// `Compound` keys are `JavaString`, which this crate cannot name; `.into()`
/// from `&str` is how every other module reads one too (`src/building/nbt.rs:44`).
pub fn tag<'a>(compound: &'a Compound, name: &str) -> Option<&'a Tag> {
    compound.get(&name.into())
}

/// Any integer width, as an `i64`. Minecraft's `getShort` accepts every numeric
/// tag, and a blueprint's block-entity coordinates really are written both ways
/// (`blueprints/v1/BlueprintUtil.java:106-108` writes shorts; the reader at
/// `:562` uses `getShort`, which takes ints too).
pub fn as_i64(tag: &Tag) -> Option<i64> {
    match tag {
        Tag::Byte(value) => Some(i64::from(*value)),
        Tag::Short(value) => Some(i64::from(*value)),
        Tag::Int(value) => Some(i64::from(*value)),
        Tag::Long(value) => Some(*value),
        _ => None,
    }
}

/// A floating-point value out of a tag, whatever width it was saved with.
pub fn as_f64(tag: &Tag) -> Option<f64> {
    match tag {
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        other => as_i64(other).map(|value| value as f64),
    }
}

/// A string tag's text. A non-string tag is `None`, not its debug form.
pub fn as_text(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(units) => Some(units.to_string_lossy()),
        _ => None,
    }
}

/// The name of a tag's type, for an error message.
pub fn type_name(tag: &Tag) -> &'static str {
    match tag {
        Tag::End => "end",
        Tag::Byte(_) => "byte",
        Tag::Short(_) => "short",
        Tag::Int(_) => "int",
        Tag::Long(_) => "long",
        Tag::Float(_) => "float",
        Tag::Double(_) => "double",
        Tag::ByteArray(_) => "byte array",
        Tag::String(_) => "string",
        Tag::List(_) => "list",
        Tag::Compound(_) => "compound",
        Tag::IntArray(_) => "int array",
        Tag::LongArray(_) => "long array",
    }
}

/// A compound's entries, minus the named ones, order otherwise kept.
///
/// A vanilla structure template carries a block entity's position in `pos`, not
/// inside its payload: measured on 1 837 real templates, none of the 13 269
/// block-entity payloads has `x`/`y`/`z`.
pub fn without(compound: &Compound, drop: &[&str]) -> Compound {
    let mut out = Compound::new();
    for (key, value) in compound {
        let name = key.to_string_lossy();
        if !drop.contains(&name.as_str()) {
            out.insert(name.as_str().into(), value.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-built bytes: a root compound with no name, one int `a = 7` and one
    /// string `b = "hi"`.
    fn tiny_document() -> Vec<u8> {
        let mut bytes = vec![10, 0, 0];
        bytes.extend_from_slice(&[3, 0, 1, b'a', 0, 0, 0, 7]);
        bytes.extend_from_slice(&[8, 0, 1, b'b', 0, 2, b'h', b'i']);
        bytes.push(0);
        bytes
    }

    #[test]
    fn a_tiny_document_parses_into_the_engines_own_tree() {
        let (name, root) = parse(&tiny_document()).expect("the document is well formed");
        assert_eq!(name, "");
        assert_eq!(tag(&root, "a"), Some(&Tag::Int(7)));
        assert_eq!(tag(&root, "b").and_then(as_text), Some("hi".to_string()));
    }

    #[test]
    fn a_root_that_is_not_a_compound_is_refused() {
        let error = parse(&[3, 0, 0, 0, 0, 0, 1]).expect_err("an int root is not a document");
        assert_eq!(error.kind, NbtErrorKind::RootNotCompound(3));
        assert!(parse(b"").is_err(), "an empty buffer has no root at all");
        assert!(parse(b"not nbt at all").is_err());
    }

    #[test]
    fn a_truncated_payload_says_where_it_stopped() {
        let mut bytes = tiny_document();
        bytes.truncate(bytes.len() - 3);
        let error = parse(&bytes).expect_err("the string payload is cut");
        assert_eq!(error.kind, NbtErrorKind::Truncated);
        assert!(error.offset > 0);
    }

    #[test]
    fn a_negative_length_is_a_malformed_file_not_an_empty_list() {
        // root compound, list tag `l`, element type int, length -1
        let bytes = vec![10, 0, 0, 9, 0, 1, b'l', 3, 0xff, 0xff, 0xff, 0xff, 0];
        let error = parse(&bytes).expect_err("a negative length cannot be a list");
        assert_eq!(error.kind, NbtErrorKind::NegativeLength(-1));
    }

    #[test]
    fn a_list_of_type_end_with_a_length_is_refused() {
        // root compound, list tag `l`, element type 0, length 3
        let bytes = vec![10, 0, 0, 9, 0, 1, b'l', 0, 0, 0, 0, 3, 0];
        let error = parse(&bytes).expect_err("a list of TAG_End cannot hold elements");
        assert_eq!(error.kind, NbtErrorKind::ListOfEnd(3));
    }

    /// Every tag type in one tree, including the two that travel as bits.
    fn every_tag() -> Compound {
        let mut properties = Compound::new();
        properties.insert("facing".into(), Tag::String("north".into()));
        Compound::from([
            ("byte".into(), Tag::Byte(-3)),
            ("short".into(), Tag::Short(-2)),
            ("int".into(), Tag::Int(i32::MIN)),
            ("long".into(), Tag::Long(i64::MAX)),
            ("float".into(), Tag::Float(0.25f32.to_bits())),
            ("double".into(), Tag::Double((-1.5f64).to_bits())),
            ("blob".into(), Tag::ByteArray(vec![0, 1, 2, 255])),
            ("text".into(), Tag::String("minecraft:oak_stairs".into())),
            ("ints".into(), Tag::IntArray(vec![-1, 0, 1])),
            ("longs".into(), Tag::LongArray(vec![i64::MIN, i64::MAX])),
            ("empty".into(), Tag::List(Vec::new())),
            (
                "list".into(),
                Tag::List(vec![Tag::Int(1), Tag::Int(2), Tag::Int(3)]),
            ),
            ("compound".into(), Tag::Compound(properties)),
        ])
    }

    #[test]
    fn every_tag_round_trips_and_the_writer_is_deterministic() {
        let root = every_tag();
        let bytes = write("", &root).expect("the tree is writable");
        let (name, read) = parse(&bytes).expect("what we write, we read");
        assert_eq!(name, "");
        assert_eq!(read, root);
        assert_eq!(bytes, write("", &root).expect("stable"), "byte-stable");
    }

    #[test]
    fn an_empty_list_is_written_with_element_type_end() {
        let mut root = Compound::new();
        root.insert("entities".into(), Tag::List(Vec::new()));
        let bytes = write("", &root).expect("an empty list is legal");
        assert_eq!(
            bytes,
            vec![
                10, 0, 0, // root compound, empty name
                9, 0, 8, b'e', b'n', b't', b'i', b't', b'i', b'e', b's', // the list
                0, // element type TAG_End
                0, 0, 0, 0, // length 0
                0, // end of the root
            ]
        );
    }

    #[test]
    fn a_mixed_list_is_refused_on_the_write_side() {
        let mut root = Compound::new();
        root.insert(
            "bad".into(),
            Tag::List(vec![Tag::Int(1), Tag::String("two".into())]),
        );
        let error = write("", &root).expect_err("NBT lists are homogeneous");
        assert_eq!(error.kind, NbtErrorKind::HeterogeneousList);
    }

    #[test]
    fn an_overlong_string_is_refused_rather_than_truncated() {
        let mut root = Compound::new();
        root.insert("name".into(), Tag::String("x".repeat(70_000).as_str().into()));
        let error = write("", &root).expect_err("a u16 length cannot hold 70 000 bytes");
        assert_eq!(error.kind, NbtErrorKind::BadStringLength(70_000));
    }

    #[test]
    fn a_nul_and_a_supplementary_character_survive_modified_utf8() {
        // The two inputs where modified UTF-8 and standard UTF-8 disagree: NUL
        // is `C0 80` there, and U+1F600 is two surrogate three-byte sequences.
        let text = "a\u{0}b\u{1F600}c";
        let mut root = Compound::new();
        root.insert("text".into(), Tag::String(text.into()));
        let bytes = write("", &root).expect("writable");
        assert!(
            bytes.windows(2).any(|pair| pair[0] == 0xc0 && pair[1] == 0x80),
            "NUL is written as an overlong C0 80"
        );
        let (_name, read) = parse(&bytes).expect("readable");
        assert_eq!(tag(&read, "text").and_then(as_text), Some(text.to_string()));
    }

    #[test]
    fn a_key_with_a_nul_also_round_trips() {
        let mut root = Compound::new();
        root.insert("a\u{0}b".into(), Tag::Int(1));
        let (_name, read) = parse(&write("", &root).expect("writable")).expect("readable");
        assert_eq!(tag(&read, "a\u{0}b"), Some(&Tag::Int(1)));
        assert_eq!(read.len(), 1);
    }

    #[test]
    fn nesting_past_the_limit_is_refused_instead_of_overflowing_the_stack() {
        let mut tree = Tag::Compound(Compound::new());
        for _ in 0..(MAX_DEPTH + 8) {
            let mut wrapper = Compound::new();
            wrapper.insert("in".into(), tree);
            tree = Tag::Compound(wrapper);
        }
        let mut root = Compound::new();
        root.insert("deep".into(), tree);
        let bytes = write("", &root).expect("the writer does not guard depth");
        let error = parse(&bytes).expect_err("the reader does");
        assert_eq!(error.kind, NbtErrorKind::TooDeep);
    }

    #[test]
    fn tag_end_is_not_a_value_on_the_write_side() {
        let mut root = Compound::new();
        root.insert("end".into(), Tag::End);
        assert_eq!(
            write("", &root).expect_err("TAG_End names no value").kind,
            NbtErrorKind::EndAsValue
        );
        // The same tag inside a list cannot be written either: a list typed by
        // its first element has no type to announce.
        let mut root = Compound::new();
        root.insert("list".into(), Tag::List(vec![Tag::End]));
        assert!(write("", &root).is_err());
    }

    #[test]
    fn an_unknown_tag_id_is_named() {
        // Root compound, then an entry that claims tag id 13.
        let bytes = vec![10, 0, 0, 13, 0, 1, b'a', 0];
        let error = parse(&bytes).expect_err("13 is not a tag id");
        assert_eq!(error.kind, NbtErrorKind::UnknownTag(13));
    }

    #[test]
    fn integers_read_at_any_width_and_floats_keep_their_bits() {
        assert_eq!(as_i64(&Tag::Byte(-3)), Some(-3));
        assert_eq!(as_i64(&Tag::Short(70)), Some(70));
        assert_eq!(as_i64(&Tag::Int(71)), Some(71));
        assert_eq!(as_i64(&Tag::Long(72)), Some(72));
        assert_eq!(as_i64(&Tag::String("no".into())), None);
        assert_eq!(
            as_f64(&Tag::Double((-1.5f64).to_bits())),
            Some(-1.5),
            "the bit pattern is read back as the number it holds"
        );
        assert_eq!(as_f64(&Tag::Float(0.5f32.to_bits())), Some(0.5));
        assert_eq!(as_f64(&Tag::Int(3)), Some(3.0));
        assert_eq!(as_f64(&Tag::ByteArray(vec![])), None);
    }

    #[test]
    fn without_drops_exactly_the_named_keys() {
        let source = Compound::from([
            ("id".into(), Tag::String("minecraft:chest".into())),
            ("x".into(), Tag::Short(1)),
            ("y".into(), Tag::Short(2)),
            ("z".into(), Tag::Short(3)),
            ("Items".into(), Tag::List(Vec::new())),
        ]);
        let kept = without(&source, &["x", "y", "z"]);
        assert_eq!(kept.len(), 2);
        assert_eq!(tag(&kept, "id").and_then(as_text).as_deref(), Some("minecraft:chest"));
        assert_eq!(tag(&kept, "x"), None);
        assert_eq!(without(&source, &[]).len(), 5);
    }
}
