//! Narrow Unicode property lookups; this is not a complete Unicode text service.
//! Source: Unicode 17.0.0 PropList.txt, Bidi_Control (four ranges, 12 scalars).
//! Retained source and SHA-256: docs/tmp/implementation-bidi-controls/source.json.

pub const UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);
/// Unicode Bidi_Control binary property. Keep these characters in source and
/// bidi processing even when a whitespace stage treats them as transparent.
pub fn is_bidi_control(scalar: char) -> bool {
    matches!(scalar, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
