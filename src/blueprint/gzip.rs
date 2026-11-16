//! The gzip container (RFC 1952) around a blueprint's NBT document.
//!
//! **This is the container Java writes.** Structurize serialises a blueprint
//! with `NbtIo.writeCompressed` (`blueprints/v1/BlueprintUtil.java:626`) and
//! reads one back with `NbtIo.readCompressed`
//! (`storage/StructurePacks.java:428`, `commands/UpdateSchematicsCommand.java:109`),
//! which in vanilla is `GZIPOutputStream`/`GZIPInputStream`: a gzip member, not
//! zlib and not raw deflate. Measured on the pack: all 9 515 files begin
//! `1f 8b 08`.
//!
//! **Reading** is what the mod needs — a blueprint is a resource, delivered as
//! its file's bytes (`crates/horizon-mod-vocab/src/host/resources.rs:20`).
//! **Writing** exists for one reason: a converted structure can be handed on in
//! the exact shape a real `.nbt` file has, which is what
//! [`crate::blueprint::structure_nbt::Conversion::to_gzipped_nbt`] returns and
//! what its tests feed back into the reader. The writer uses *stored* deflate
//! blocks (RFC 1951 §3.2.4), so the output is not smaller than the input; it is
//! a valid member, and byte-stable.
//!
//! Nothing here touches a path: the bytes come in and go out of the caller's
//! hands, and the point is to produce a document, not a file.

use crate::blueprint::inflate::{inflate, InflateError};

/// What can be wrong with a gzip member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GzipError {
    /// Not `1f 8b`, or shorter than a header.
    BadMagic,
    /// Compression method other than 8 (deflate).
    BadMethod(u8),
    /// A header field or the trailer is cut off.
    Truncated,
    /// The trailer's CRC32 does not match the payload.
    ChecksumMismatch { expected: u32, actual: u32 },
    /// The trailer's `ISIZE` does not match the payload length.
    SizeMismatch { expected: u32, actual: u32 },
    /// The deflate payload itself is broken.
    Deflate(InflateError),
}

impl From<InflateError> for GzipError {
    fn from(error: InflateError) -> Self {
        GzipError::Deflate(error)
    }
}

impl std::fmt::Display for GzipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GzipError::BadMagic => write!(f, "not a gzip stream (bad magic)"),
            GzipError::BadMethod(method) => {
                write!(f, "gzip compression method {method} is not deflate")
            }
            GzipError::Truncated => write!(f, "gzip stream truncated"),
            GzipError::ChecksumMismatch { expected, actual } => write!(
                f,
                "gzip CRC32 mismatch: trailer says {expected:#010x}, payload hashes to {actual:#010x}"
            ),
            GzipError::SizeMismatch { expected, actual } => write!(
                f,
                "gzip ISIZE mismatch: trailer says {expected} bytes, deflate produced {actual}"
            ),
            GzipError::Deflate(error) => write!(f, "deflate error: {error}"),
        }
    }
}

impl std::error::Error for GzipError {}

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut value = i as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[i] = value;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = make_crc_table();

/// Standard CRC-32, the one gzip's trailer carries.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc = CRC_TABLE[((crc ^ byte as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

/// `true` when `bytes` starts with the gzip magic **and** the deflate method.
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[2] == 0x08
}

/// Wrap `payload` in one gzip member, with stored deflate blocks.
///
/// Header fields are fixed (mtime 0, XFL 0, OS 0) so the same input always
/// produces the same bytes — the byte-stability test pins that, and it is what
/// makes a converted structure comparable between two runs.
pub fn gzip(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + payload.len() / 65_535 * 5 + 32);
    out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x00]);

    if payload.is_empty() {
        // One final stored block with LEN 0: BFINAL=1, BTYPE=00, then zeros.
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    } else {
        for (index, chunk) in payload.chunks(65_535).enumerate() {
            let last = (index + 1) * 65_535 >= payload.len();
            // A stored block starts on a byte boundary, and we always are on
            // one: BFINAL + BTYPE 00 in one byte, then LEN and its complement,
            // both little-endian.
            out.push(if last { 0x01 } else { 0x00 });
            let len = chunk.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(chunk);
        }
    }

    // Trailer: CRC32 and ISIZE, both little-endian (RFC 1952 §2.3.1).
    out.extend_from_slice(&crc32(payload).to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out
}

/// Decompress one gzip member, checking both trailer fields.
///
/// One member is all a blueprint is. Trailing bytes after the member are
/// ignored, as Java's `GZIPInputStream` ignores them once the first member ends.
pub fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, GzipError> {
    if bytes.len() < 18 {
        return Err(GzipError::Truncated);
    }
    if bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(GzipError::BadMagic);
    }
    let method = bytes[2];
    if method != 8 {
        return Err(GzipError::BadMethod(method));
    }
    let flags = bytes[3];
    // magic(2) + method(1) + flags(1) + mtime(4) + xfl(1) + os(1)
    let mut pos = 10usize;

    if flags & 0x04 != 0 {
        // FEXTRA: a little-endian length, then that many bytes.
        let extra_len = read_u16(bytes, pos)? as usize;
        pos += 2;
        pos = pos
            .checked_add(extra_len)
            .filter(|end| *end <= bytes.len())
            .ok_or(GzipError::Truncated)?;
    }
    if flags & 0x08 != 0 {
        // FNAME: NUL-terminated.
        pos = skip_cstring(bytes, pos)?;
    }
    if flags & 0x10 != 0 {
        // FCOMMENT: NUL-terminated.
        pos = skip_cstring(bytes, pos)?;
    }
    if flags & 0x02 != 0 {
        // FHCRC: the low 16 bits of the CRC32 of the header so far.
        let stored = read_u16(bytes, pos)?;
        let computed = (crc32(&bytes[..pos]) & 0xffff) as u16;
        if stored != computed {
            return Err(GzipError::ChecksumMismatch {
                expected: u32::from(stored),
                actual: u32::from(computed),
            });
        }
        pos += 2;
    }

    let payload = &bytes[pos..];
    if payload.len() < 8 {
        return Err(GzipError::Truncated);
    }
    let body_len = payload.len() - 8;
    let trailer = &payload[body_len..];
    let expected_crc = u32::from_le_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    let expected_size = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);

    // ISIZE is a hint (it wraps at 4 GiB); it saves re-growing the buffer.
    let out = inflate(&payload[..body_len], Some(expected_size as usize))?;

    let actual_crc = crc32(&out);
    if actual_crc != expected_crc {
        return Err(GzipError::ChecksumMismatch {
            expected: expected_crc,
            actual: actual_crc,
        });
    }
    let actual_size = (out.len() & 0xffff_ffff) as u32;
    if actual_size != expected_size {
        return Err(GzipError::SizeMismatch {
            expected: expected_size,
            actual: actual_size,
        });
    }
    Ok(out)
}

fn read_u16(bytes: &[u8], pos: usize) -> Result<u16, GzipError> {
    let pair = bytes.get(pos..pos + 2).ok_or(GzipError::Truncated)?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn skip_cstring(bytes: &[u8], mut pos: usize) -> Result<usize, GzipError> {
    while *bytes.get(pos).ok_or(GzipError::Truncated)? != 0 {
        pos += 1;
    }
    Ok(pos + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_its_known_vector() {
        // CRC-32("123456789") = 0xcbf43926, the check value every CRC table
        // lists for this polynomial.
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_stream_that_is_not_gzip_is_named_as_such() {
        assert_eq!(
            gunzip(b"plain nbt bytes here").expect_err("not gzip"),
            GzipError::BadMagic
        );
        assert_eq!(gunzip(b"\x1f\x8b").expect_err("too short"), GzipError::Truncated);
        assert!(!is_gzip(b"NBT"));
        assert!(is_gzip(&[0x1f, 0x8b, 0x08]));
        // zlib and raw deflate are not gzip: the third byte is the method.
        assert!(!is_gzip(&[0x1f, 0x8b, 0x00]));
        assert_eq!(
            gunzip(&[0x1f, 0x8b, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
                .expect_err("method 7 is not deflate"),
            GzipError::BadMethod(7)
        );
    }

    #[test]
    fn what_the_writer_produces_the_reader_accepts() {
        for payload in [
            Vec::new(),
            b"hello".to_vec(),
            vec![0x5au8; 65_535],
            vec![0x01u8; 65_536],
            (0..200_000u32).map(|i| (i % 251) as u8).collect(),
        ] {
            let compressed = gzip(&payload);
            assert!(
                is_gzip(&compressed),
                "the writer must produce a gzip member ({} bytes)",
                payload.len()
            );
            assert_eq!(
                gunzip(&compressed).expect("our own member reads back"),
                payload,
                "round trip of {} bytes",
                payload.len()
            );
            assert_eq!(
                compressed,
                gzip(&payload),
                "the writer must be deterministic"
            );
        }
    }

    #[test]
    fn the_header_flags_a_java_writer_may_set_are_skipped() {
        // Same member as gzip("abc"), but with FEXTRA and FNAME in the header.
        let payload = b"abc";
        let mut member = vec![0x1f, 0x8b, 0x08, 0x04 | 0x08, 0, 0, 0, 0, 0x00, 0x00];
        member.extend_from_slice(&[0x04, 0x00]); // FEXTRA length 4
        member.extend_from_slice(b"XYZW"); // …its four bytes
        member.extend_from_slice(b"abc.blueprint\0"); // FNAME
        member.extend_from_slice(&[0x01, 0x03, 0x00, 0xfc, 0xff]); // stored block
        member.extend_from_slice(payload);
        member.extend_from_slice(&crc32(payload).to_le_bytes());
        member.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        assert_eq!(gunzip(&member).expect("flags are skipped"), payload.to_vec());
    }

    #[test]
    fn a_corrupt_payload_is_caught_by_the_crc() {
        let mut member = gzip(b"hello, blueprint");
        let last = member.len();
        member[last - 6] ^= 0xff; // one byte of the CRC32
        assert!(matches!(
            gunzip(&member).expect_err("the CRC must disagree"),
            GzipError::ChecksumMismatch { .. }
        ));
    }

    #[test]
    fn a_wrong_size_in_the_trailer_is_named() {
        let mut member = gzip(b"hello");
        let last = member.len();
        // ISIZE is the last four bytes, little-endian; 5 -> 6.
        member[last - 4] = 6;
        assert!(matches!(
            gunzip(&member).expect_err("the size must disagree"),
            GzipError::SizeMismatch {
                expected: 6,
                actual: 5
            }
        ));
    }

    #[test]
    fn a_cut_member_says_it_is_truncated() {
        let member = gzip(b"a payload long enough to cut");
        for cut in [0, 4, 10, member.len() - 9] {
            let error = gunzip(&member[..cut]).expect_err("cut members do not read");
            assert!(
                matches!(&error, GzipError::Truncated | GzipError::Deflate(_)),
                "cut at {cut} gave {error:?}"
            );
        }
    }

    #[test]
    fn the_writer_never_shrinks_but_always_wraps() {
        // A stored-block member is bigger than its payload and starts with the
        // ten header bytes; that is the price of not writing a real compressor.
        let payload = vec![0u8; 1000];
        let member = gzip(&payload);
        assert_eq!(
            &member[..10],
            &[0x1f, 0x8b, 0x08, 0, 0, 0, 0, 0, 0x00, 0x00][..]
        );
        assert_eq!(member.len(), 10 + 5 + payload.len() + 8);
    }
}
