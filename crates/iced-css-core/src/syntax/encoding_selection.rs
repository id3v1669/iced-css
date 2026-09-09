//! Separate-file precedence only. Selected labels still require decoder support
//! and as-specified declarations still require post-decoding validation.
use super::{
    charset::lookup_iana_charset,
    encoding::{PrefixMatch, detect_unicode_prefix},
    encoding_transport::{TransportOverride, transport_bom_override},
};
use std::num::NonZeroUsize;

/// Already extracted protocol/link/source metadata, without invented defaults or
/// label trimming. None means absent; an empty or unknown present label remains
/// present. Embedded stylesheet encoding is governed by its containing document.
#[derive(Debug, Clone, Copy, Default)]
pub struct EncodingMetadata<'a> {
    pub transport: Option<&'a [u8]>,
    pub linking: Option<&'a [u8]>,
    pub referring: Option<&'a [u8]>,
}
#[derive(Debug, Clone, Copy)]
pub struct SelectionLimits {
    pub registry_work: NonZeroUsize,
    pub prefix_work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingOrigin {
    Transport,
    Detection,
    Linking,
    Referring,
    Default,
}
#[derive(Debug)]
pub enum EncodingChoice<'a> {
    Label(&'a [u8]),
    TransportBom(TransportOverride),
    Detected(PrefixMatch<'a>),
    DefaultUtf8,
}
#[derive(Debug)]
pub struct EncodingSelection<'a> {
    pub origin: EncodingOrigin,
    pub choice: EncodingChoice<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    RegistryWork,
    PrefixWork,
}
impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS encoding selection: {self:?}")
    }
}
impl std::error::Error for SelectionError {}

/// CSS2.2 section 4.4's five priorities, using the Unicode-family detection rows
/// supported by `detect_unicode_prefix`. No decoding, allocation or acceptance
/// claim. Lower stages cannot replace a selected unknown label. The independent
/// registry/prefix allowances are spent only if that stage is reached; transport
/// BOM comparisons have the fixed bound documented by `transport_bom_override`.
pub fn select_separate_encoding<'a>(
    input: &'a [u8],
    metadata: EncodingMetadata<'a>,
    limits: SelectionLimits,
) -> Result<EncodingSelection<'a>, SelectionError> {
    if let Some(label) = metadata.transport {
        let class = lookup_iana_charset(label, limits.registry_work)
            .map_err(|_| SelectionError::RegistryWork)?
            .and_then(|charset| charset.unicode_transport_class());
        let choice = match transport_bom_override(class, input) {
            Some(overridden) => EncodingChoice::TransportBom(overridden),
            None => EncodingChoice::Label(label),
        };
        return Ok(EncodingSelection {
            origin: EncodingOrigin::Transport,
            choice,
        });
    }
    if let Some(prefix) =
        detect_unicode_prefix(input, limits.prefix_work).map_err(|_| SelectionError::PrefixWork)?
    {
        return Ok(EncodingSelection {
            origin: EncodingOrigin::Detection,
            choice: EncodingChoice::Detected(prefix),
        });
    }
    for (origin, label) in [
        (EncodingOrigin::Linking, metadata.linking),
        (EncodingOrigin::Referring, metadata.referring),
    ] {
        if let Some(label) = label {
            return Ok(EncodingSelection {
                origin,
                choice: EncodingChoice::Label(label),
            });
        }
    }
    Ok(EncodingSelection {
        origin: EncodingOrigin::Default,
        choice: EncodingChoice::DefaultUtf8,
    })
}
