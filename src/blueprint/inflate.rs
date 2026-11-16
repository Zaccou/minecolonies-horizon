//! Raw DEFLATE (RFC 1951): the decompressor a `.blueprint` needs.
//!
//! **Written by hand because the package cannot add a dependency.** `Cargo.toml`
//! carries one dependency, `horizon-mod-api`, on purpose (see its comment: a wasm
//! mod that used a registry crate could not be built from a pinned lock file),
//! and this module lives in the crate, not in the API. So no `flate2`, no
//! `miniz_oxide`: the canonical-Huffman decoder below is the same shape as
//! zlib's `puff.c` — per-length symbol counts, decode bit by bit.
//!
//! **What calls it.** [`crate::blueprint::gzip::gunzip`], which strips the RFC
//! 1952 wrapper. Every one of the 9 515 blueprints in the pack is one gzip
//! member: Structurize writes them with `NbtIo.writeCompressed`
//! (`blueprints/v1/BlueprintUtil.java:626`) and reads them with
//! `NbtIo.readCompressed` (`storage/StructurePacks.java:428`), and Java's
//! `GZIPOutputStream` always speaks deflate.
//!
//! **Not a compressor.** [`crate::blueprint::gzip::gzip`] writes *stored*
//! blocks (BTYPE 00), which is legal deflate and needs no match finder; this
//! decoder reads all three block types, because a real file uses the two
//! Huffman ones.

/// What can be wrong with a DEFLATE stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InflateError {
    /// The stream ended in the middle of a block or a symbol.
    Truncated,
    /// A block declared type 3, which RFC 1951 §3.2.3 reserves.
    InvalidBlockType,
    /// A stored block's `NLEN` is not the one's complement of its `LEN`
    /// (RFC 1951 §3.2.4).
    BadStoredLength,
    /// A Huffman table is over-subscribed: more codes than leaves.
    OverSubscribed,
    /// A code length, or the symbol it names, is out of range.
    InvalidCode(u32),
    /// A back-reference points before the start of the output.
    InvalidDistance,
    /// A code-length repeat ran past the end of the alphabet it builds.
    RepeatOverflow,
}

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InflateError::Truncated => write!(f, "deflate stream truncated"),
            InflateError::InvalidBlockType => write!(f, "deflate block type 3 is reserved"),
            InflateError::BadStoredLength => {
                write!(f, "stored block length and its complement disagree")
            }
            InflateError::OverSubscribed => write!(f, "deflate Huffman table over-subscribed"),
            InflateError::InvalidCode(code) => write!(f, "deflate code {code} out of range"),
            InflateError::InvalidDistance => {
                write!(f, "deflate back-reference before the output start")
            }
            InflateError::RepeatOverflow => write!(f, "deflate code-length repeat overflow"),
        }
    }
}

impl std::error::Error for InflateError {}

// RFC 1951 §3.2.5, tables 1 and 2.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// The order the code-length code lengths are stored in (RFC 1951 §3.2.7).
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// LSB-first bit reader, as DEFLATE requires.
struct BitReader<'a> {
    data: &'a [u8],
    /// Next byte to load into the bit buffer.
    pos: usize,
    buf: u64,
    /// Valid bits held in `buf`'s low end.
    nbits: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            buf: 0,
            nbits: 0,
        }
    }

    #[inline]
    fn bits(&mut self, count: u32) -> Result<u32, InflateError> {
        debug_assert!(count <= 32);
        while self.nbits < count {
            let byte = *self.data.get(self.pos).ok_or(InflateError::Truncated)?;
            self.pos += 1;
            self.buf |= (byte as u64) << self.nbits;
            self.nbits += 8;
        }
        let value = if count == 0 {
            0
        } else {
            (self.buf & ((1u64 << count) - 1)) as u32
        };
        self.buf >>= count;
        self.nbits -= count;
        Ok(value)
    }

    /// Drop the bits up to the next byte boundary, giving whole bytes back so
    /// `pos` stays authoritative for a stored block's payload.
    fn align_to_byte(&mut self) {
        let drop = self.nbits % 8;
        self.buf >>= drop;
        self.nbits -= drop;
        let whole = (self.nbits / 8) as usize;
        self.pos -= whole;
        self.buf = 0;
        self.nbits = 0;
    }
}

/// A canonical Huffman decoding table.
#[derive(Debug)]
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn build(lengths: &[u8]) -> Result<Huffman, InflateError> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len as usize > 15 {
                return Err(InflateError::InvalidCode(len as u32));
            }
            counts[len as usize] += 1;
        }

        // At most 2^len codes may have a length of at most len.
        let mut left: i32 = 1;
        for &count in counts.iter().skip(1) {
            left <<= 1;
            left -= count as i32;
            if left < 0 {
                return Err(InflateError::OverSubscribed);
            }
        }
        // `left > 0` is an incomplete code: legal in DEFLATE only for the
        // degenerate single-symbol case, so we build it and let a decode fail
        // if it is ever used.

        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &len) in lengths.iter().enumerate() {
            if len != 0 {
                symbols[offsets[len as usize] as usize] = symbol as u16;
                offsets[len as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut BitReader<'_>) -> Result<u16, InflateError> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..=15 {
            code |= bits.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                let at = index + (code - first);
                return self
                    .symbols
                    .get(at as usize)
                    .copied()
                    .ok_or(InflateError::InvalidCode(at as u32));
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(InflateError::InvalidCode(0))
    }
}

/// Decompress a raw DEFLATE stream — no gzip, no zlib wrapper.
///
/// `size_hint` pre-allocates the output; pass the gzip `ISIZE` when the
/// container said it (a hint only: it wraps at 4 GiB and a file may lie).
pub fn inflate(data: &[u8], size_hint: Option<usize>) -> Result<Vec<u8>, InflateError> {
    let mut bits = BitReader::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(size_hint.unwrap_or(1 << 16).min(1 << 30));

    loop {
        let last = bits.bits(1)? == 1;
        let block_type = bits.bits(2)?;
        match block_type {
            0 => stored_block(&mut bits, &mut out)?,
            1 => {
                let (literals, distances) = fixed_tables()?;
                compressed_block(&mut bits, &mut out, &literals, &distances)?;
            }
            2 => {
                let (literals, distances) = dynamic_tables(&mut bits)?;
                compressed_block(&mut bits, &mut out, &literals, &distances)?;
            }
            _ => return Err(InflateError::InvalidBlockType),
        }
        if last {
            return Ok(out);
        }
    }
}

fn stored_block(bits: &mut BitReader<'_>, out: &mut Vec<u8>) -> Result<(), InflateError> {
    bits.align_to_byte();
    let pos = bits.pos;
    let header = bits
        .data
        .get(pos..pos + 4)
        .ok_or(InflateError::Truncated)?;
    let len = u16::from_le_bytes([header[0], header[1]]) as usize;
    let nlen = u16::from_le_bytes([header[2], header[3]]);
    if nlen != !(len as u16) {
        return Err(InflateError::BadStoredLength);
    }
    let start = pos + 4;
    let payload = bits
        .data
        .get(start..start + len)
        .ok_or(InflateError::Truncated)?;
    out.extend_from_slice(payload);
    bits.pos = start + len;
    Ok(())
}

/// RFC 1951 §3.2.6: the fixed literal/length and distance tables.
fn fixed_tables() -> Result<(Huffman, Huffman), InflateError> {
    let mut literal_lengths = [0u8; 288];
    for (symbol, slot) in literal_lengths.iter_mut().enumerate() {
        *slot = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let distance_lengths = [5u8; 30];
    Ok((
        Huffman::build(&literal_lengths)?,
        Huffman::build(&distance_lengths)?,
    ))
}

/// RFC 1951 §3.2.7: the code lengths a block declares for itself.
fn dynamic_tables(bits: &mut BitReader<'_>) -> Result<(Huffman, Huffman), InflateError> {
    let hlit = bits.bits(5)? as usize + 257;
    let hdist = bits.bits(5)? as usize + 1;
    let hclen = bits.bits(4)? as usize + 4;

    let mut code_lengths = [0u8; 19];
    for &slot in CODE_LENGTH_ORDER.iter().take(hclen) {
        code_lengths[slot] = bits.bits(3)? as u8;
    }
    let code_length_code = Huffman::build(&code_lengths)?;

    let total = hlit + hdist;
    let mut lengths: Vec<u8> = Vec::with_capacity(total);
    while lengths.len() < total {
        match code_length_code.decode(bits)? {
            symbol @ 0..=15 => lengths.push(symbol as u8),
            16 => {
                let previous = *lengths.last().ok_or(InflateError::RepeatOverflow)?;
                let repeat = 3 + bits.bits(2)? as usize;
                if lengths.len() + repeat > total {
                    return Err(InflateError::RepeatOverflow);
                }
                lengths.extend(std::iter::repeat_n(previous, repeat));
            }
            17 => {
                let repeat = 3 + bits.bits(3)? as usize;
                if lengths.len() + repeat > total {
                    return Err(InflateError::RepeatOverflow);
                }
                lengths.extend(std::iter::repeat_n(0u8, repeat));
            }
            18 => {
                let repeat = 11 + bits.bits(7)? as usize;
                if lengths.len() + repeat > total {
                    return Err(InflateError::RepeatOverflow);
                }
                lengths.extend(std::iter::repeat_n(0u8, repeat));
            }
            other => return Err(InflateError::InvalidCode(other as u32)),
        }
    }

    let literals = Huffman::build(&lengths[..hlit])?;
    let distances = Huffman::build(&lengths[hlit..])?;
    Ok((literals, distances))
}

fn compressed_block(
    bits: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    literals: &Huffman,
    distances: &Huffman,
) -> Result<(), InflateError> {
    loop {
        let symbol = literals.decode(bits)? as usize;
        if symbol < 256 {
            out.push(symbol as u8);
        } else if symbol == 256 {
            return Ok(());
        } else {
            let slot = symbol - 257;
            if slot >= LENGTH_BASE.len() {
                return Err(InflateError::InvalidCode(symbol as u32));
            }
            let length = LENGTH_BASE[slot] as usize + bits.bits(LENGTH_EXTRA[slot] as u32)? as usize;
            let distance_symbol = distances.decode(bits)? as usize;
            if distance_symbol >= DIST_BASE.len() {
                return Err(InflateError::InvalidCode(distance_symbol as u32));
            }
            let distance = DIST_BASE[distance_symbol] as usize
                + bits.bits(DIST_EXTRA[distance_symbol] as u32)? as usize;
            if distance > out.len() {
                return Err(InflateError::InvalidDistance);
            }
            // Byte by byte on purpose: an overlapping copy is part of DEFLATE.
            let start = out.len() - distance;
            for offset in 0..length {
                let byte = out[start + offset];
                out.push(byte);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_stored_stream_decompresses_to_nothing() {
        // BFINAL=1, BTYPE=00, LEN=0, NLEN=0xffff.
        let data = [0x01, 0x00, 0x00, 0xff, 0xff];
        assert_eq!(inflate(&data, None).expect("valid"), Vec::<u8>::new());
    }

    #[test]
    fn a_stored_block_gives_its_payload_back() {
        // BFINAL=1 BTYPE=00, LEN=3, NLEN=!3, then "abc" — checked against zlib.
        let data = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(inflate(&data, None).expect("valid"), b"abc".to_vec());
    }

    #[test]
    fn two_stored_blocks_in_one_stream_are_both_read() {
        let data = [
            0x00, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c', // BFINAL=0
            0x01, 0x02, 0x00, 0xfd, 0xff, b'd', b'e', // BFINAL=1
        ];
        assert_eq!(inflate(&data, None).expect("valid"), b"abcde".to_vec());
    }

    #[test]
    fn a_stored_length_that_disagrees_with_its_complement_is_refused() {
        let data = [0x01, 0x03, 0x00, 0x01, 0x00, b'a', b'b', b'c'];
        assert_eq!(
            inflate(&data, None).expect_err("NLEN is wrong"),
            InflateError::BadStoredLength
        );
    }

    #[test]
    fn truncation_is_an_error_not_a_panic() {
        assert_eq!(
            inflate(&[0x01, 0x03, 0x00, 0xfc, 0xff, b'a'], None).expect_err("cut"),
            InflateError::Truncated
        );
        assert_eq!(
            inflate(&[], None).expect_err("no block header at all"),
            InflateError::Truncated
        );
    }

    #[test]
    fn block_type_three_is_refused() {
        // BFINAL=1, BTYPE=11.
        assert_eq!(
            inflate(&[0x07], None).expect_err("reserved"),
            InflateError::InvalidBlockType
        );
    }

    #[test]
    fn the_fixed_tables_decode_a_literal_run() {
        // "abc" and end-of-block with the fixed Huffman codes; the five bytes
        // were produced and checked against zlib's inflate, not by hand.
        let data = [0x4b, 0x4c, 0x4a, 0x06, 0x00];
        assert_eq!(inflate(&data, None).expect("valid"), b"abc".to_vec());
        // The same with an empty run: final, fixed, end-of-block, padding.
        assert_eq!(inflate(&[0x03, 0x00], None).expect("valid"), Vec::<u8>::new());
    }

    #[test]
    fn an_oversubscribed_table_is_refused() {
        // Four symbols of length 1 cannot fit a binary code.
        assert_eq!(
            Huffman::build(&[1, 1, 1, 1]).expect_err("over-subscribed"),
            InflateError::OverSubscribed
        );
        assert_eq!(
            Huffman::build(&[16, 0]).expect_err("longer than 15 bits"),
            InflateError::InvalidCode(16)
        );
    }

    #[test]
    fn a_back_reference_can_copy_a_byte_it_just_wrote() {
        // Fixed block holding 'a' and then a length-3 / distance-1 copy: this is
        // the overlapping case RFC 1951 §3.2.2 allows, and `compressed_block`
        // has to reproduce it byte by byte. The bytes come from a bit writer
        // checked against zlib, not by hand.
        let data = [0x4b, 0x04, 0x02, 0x00];
        assert_eq!(
            inflate(&data, Some(4)).expect("valid"),
            b"aaaa".to_vec(),
            "one literal then a copy of itself"
        );
    }
}
