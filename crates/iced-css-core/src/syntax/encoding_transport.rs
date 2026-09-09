use super::encoding::{ByteOrder, EncodingHint};

/// Classification of an actually supplied, resolved transport charset label.
/// Other means a known encoding outside these seven forms, not an unknown label. This
/// type does not resolve aliases or manufacture a charset for absent metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportEncoding {
    Utf8,
    Utf16,
    Utf32,
    Utf16Be,
    Utf16Le,
    Utf32Be,
    Utf32Le,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportOverride {
    pub encoding: EncodingHint,
    pub bom_bytes: usize,
    pub explicit_endian_error: bool,
}

/// Apply only the five-row transport override table in CSS2.2 section 4.4. The
/// result reports the required explicit-endian error; diagnostic publication and
/// actual decoding belong to intake. None leaves the transport decision unchanged
/// (or leaves lower-precedence detection to the owner when metadata was absent).
/// At most five constant-size prefix comparisons; no allocations or input scan.
pub fn transport_bom_override(
    transport: Option<TransportEncoding>,
    input: &[u8],
) -> Option<TransportOverride> {
    let explicit_endian_error = match transport? {
        TransportEncoding::Utf8 | TransportEncoding::Utf16 | TransportEncoding::Utf32 => false,
        TransportEncoding::Utf16Be
        | TransportEncoding::Utf16Le
        | TransportEncoding::Utf32Be
        | TransportEncoding::Utf32Le => true,
        TransportEncoding::Other => return None,
    };
    const TABLE: &[(&[u8], EncodingHint)] = &[
        (b"\0\0\xfe\xff", EncodingHint::Utf32(ByteOrder::Big)),
        (b"\xff\xfe\0\0", EncodingHint::Utf32(ByteOrder::Little)),
        (b"\xfe\xff", EncodingHint::Utf16(ByteOrder::Big)),
        (b"\xff\xfe", EncodingHint::Utf16(ByteOrder::Little)),
        (b"\xef\xbb\xbf", EncodingHint::Utf8),
    ];
    TABLE.iter().find_map(|(bom, encoding)| {
        input.starts_with(bom).then_some(TransportOverride {
            encoding: *encoding,
            bom_bytes: bom.len(),
            explicit_endian_error,
        })
    })
}
