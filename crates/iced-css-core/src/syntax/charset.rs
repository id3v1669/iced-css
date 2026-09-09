//! Registered IANA identity lookup, independent of supported decoder selection.
use super::encoding_transport::TransportEncoding;
use std::{cmp::Ordering, num::NonZeroUsize};

#[derive(Debug)]
pub struct Charset {
    mib: u16,
    name: &'static str,
    preferred: Option<&'static str>,
}
impl Charset {
    pub fn mib(&self) -> u16 {
        self.mib
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn preferred_mime_name(&self) -> Option<&'static str> {
        self.preferred
    }
    /// Classify the seven registered Unicode charset names referenced by the CSS2.2
    /// transport override rule. This does not assert availability of a decoder.
    pub fn unicode_transport_class(&self) -> Option<TransportEncoding> {
        Some(match self.mib {
            106 => TransportEncoding::Utf8,
            1013 => TransportEncoding::Utf16Be,
            1014 => TransportEncoding::Utf16Le,
            1015 => TransportEncoding::Utf16,
            1017 => TransportEncoding::Utf32,
            1018 => TransportEncoding::Utf32Be,
            1019 => TransportEncoding::Utf32Le,
            _ => return None,
        })
    }
}
#[path = "charset_data.rs"]
mod data;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharsetWork;
impl std::fmt::Display for CharsetWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("charset name comparison work allowance exhausted")
    }
}
impl std::error::Error for CharsetWork {}
fn charge(work: &mut usize) -> Result<(), CharsetWork> {
    *work = work.checked_sub(1).ok_or(CharsetWork)?;
    Ok(())
}
fn compare<I: ExactSizeIterator<Item = u8>>(
    label: I,
    registered: &str,
    work: &mut usize,
) -> Result<Ordering, CharsetWork> {
    let length = label.len();
    charge(work)?;
    for (actual, expected) in label.zip(registered.bytes()) {
        charge(work)?;
        let comparison = actual.to_ascii_lowercase().cmp(&expected);
        if comparison != Ordering::Equal {
            return Ok(comparison);
        }
    }
    Ok(length.cmp(&registered.len()))
}

/// Look up the retained IANA registry snapshot with ASCII case folding only.
/// No whitespace trimming, browser label remapping, allocation or byte decoding.
/// None means no label in this snapshot; Some identifies a registry entry and
/// must not be interpreted as product decoder support.
pub fn lookup_iana_charset(
    label: &[u8],
    work: NonZeroUsize,
) -> Result<Option<&'static Charset>, CharsetWork> {
    lookup_iana_charset_iter(label.iter().copied(), &mut work.get())
}

pub(crate) fn lookup_iana_charset_iter<I>(
    label: I,
    work: &mut usize,
) -> Result<Option<&'static Charset>, CharsetWork>
where
    I: ExactSizeIterator<Item = u8> + Clone,
{
    let mut low = 0;
    let mut high = data::LABELS.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let (registered, record) = data::LABELS[middle];
        match compare(label.clone(), registered, work)? {
            Ordering::Less => high = middle,
            Ordering::Greater => low = middle + 1,
            Ordering::Equal => return Ok(Some(&data::CHARSETS[record])),
        }
    }
    Ok(None)
}
