//! Byte-prefix recognition only: CSS encoding selection and decoding remain the
//! intake owner's responsibility. Do not interpret these hints as decoded text.
use std::{num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Big,
    Little,
    Order2143,
    Order3412,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingHint {
    Utf8,
    Utf16(ByteOrder),
    Utf32(ByteOrder),
    Declared { default_order: Option<ByteOrder> },
}
#[derive(Debug, Clone, Copy)]
struct Unit {
    width: usize,
    ascii_index: usize,
}
struct Pattern {
    prefix: &'static [u8],
    unit: Option<Unit>,
    suffix: &'static [u8],
    hint: EncodingHint,
}
#[path = "encoding_table.rs"]
mod table;

#[derive(Debug)]
pub struct CharsetName<'a> {
    raw: &'a [u8],
    unit: Unit,
    pub span: Range<usize>,
}
impl CharsetName<'_> {
    /// Original XX bytes in table order. These are not certified ASCII, an IANA
    /// name, or an appropriate post-decoding @charset declaration.
    pub fn bytes(&self) -> impl ExactSizeIterator<Item = u8> + Clone + '_ {
        self.raw
            .chunks_exact(self.unit.width)
            .map(|unit| unit[self.unit.ascii_index])
    }
}
#[derive(Debug)]
pub struct PrefixMatch<'a> {
    pub hint: EncodingHint,
    pub name: Option<CharsetName<'a>>,
    pub consumed: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixWork;
impl std::fmt::Display for PrefixWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CSS encoding-prefix work allowance exhausted")
    }
}
impl std::error::Error for PrefixWork {}
fn charge(work: &mut usize) -> Result<(), PrefixWork> {
    *work = work.checked_sub(1).ok_or(PrefixWork)?;
    Ok(())
}
fn starts_with(input: &[u8], expected: &[u8], work: &mut usize) -> Result<bool, PrefixWork> {
    for (index, byte) in expected.iter().enumerate() {
        charge(work)?;
        if input.get(index) != Some(byte) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Match the 21 Unicode-family rows of the retained CSS2.2 section 4.4 table,
/// in source order with shortest matching repetition. No allocations or decoding.
/// EBCDIC/IBM1026/GSM and optional analogous patterns are outside this primitive.
/// This is the BOM/@charset table, not the separate transport-override table.
pub fn detect_unicode_prefix(
    input: &[u8],
    work: NonZeroUsize,
) -> Result<Option<PrefixMatch<'_>>, PrefixWork> {
    let mut work = work.get();
    for pattern in table::PATTERNS {
        if !starts_with(input, pattern.prefix, &mut work)? {
            continue;
        }
        let Some(unit) = pattern.unit else {
            return Ok(Some(PrefixMatch {
                hint: pattern.hint,
                name: None,
                consumed: pattern.prefix.len(),
            }));
        };
        let start = pattern.prefix.len();
        let mut cursor = start;
        loop {
            if starts_with(&input[cursor..], pattern.suffix, &mut work)? {
                return Ok(Some(PrefixMatch {
                    hint: pattern.hint,
                    name: Some(CharsetName {
                        raw: &input[start..cursor],
                        unit,
                        span: start..cursor,
                    }),
                    consumed: cursor + pattern.suffix.len(),
                }));
            }
            let Some(next) = input[cursor..].get(..unit.width) else {
                break;
            };
            let mut matches = true;
            for (index, byte) in next.iter().enumerate() {
                charge(&mut work)?;
                if index != unit.ascii_index && *byte != 0 {
                    matches = false;
                    break;
                }
            }
            if !matches {
                break;
            }
            cursor += unit.width;
        }
    }
    Ok(None)
}
