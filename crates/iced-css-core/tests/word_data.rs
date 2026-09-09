use iced_css_core::text::word_data::{
    DATA_VERSION, WordBreak::*, is_extended_pictographic, word_break,
};
#[test]
fn all_nineteen_classes_have_literal_meanings() {
    assert_eq!(DATA_VERSION, (17, 0, 0));
    for (c, expected) in [
        ('\0', Other),
        ('"', DoubleQuote),
        ('\'', SingleQuote),
        ('א', HebrewLetter),
        ('\r', Cr),
        ('\n', Lf),
        ('\u{b}', Newline),
        ('\u{300}', Extend),
        ('\u{1f1e6}', RegionalIndicator),
        ('\u{ad}', Format),
        ('\u{3031}', Katakana),
        ('A', ALetter),
        (':', MidLetter),
        (',', MidNum),
        ('.', MidNumLet),
        ('0', Numeric),
        ('_', ExtendNumLet),
        ('\u{200d}', Zwj),
        (' ', WSegSpace),
    ] {
        assert_eq!(word_break(c), expected, "{c:?}");
    }
}
#[test]
fn pictographic_is_independent_including_all_six_letter_overlaps() {
    for c in [
        '\u{2139}',
        '\u{24c2}',
        '\u{1f170}',
        '\u{1f171}',
        '\u{1f17e}',
        '\u{1f17f}',
    ] {
        assert_eq!(word_break(c), ALetter);
        assert!(is_extended_pictographic(c));
        assert!(is_extended_pictographic(c));
        assert_eq!(word_break(c), ALetter);
    }
    for c in ['©', '😀', '\u{1fc00}', '\u{1fffd}'] {
        assert!(is_extended_pictographic(c));
        assert_eq!(word_break(c), Other);
    }
    for c in ['A', '0', '\u{1fbff}', '\u{1fffe}', '\u{10ffff}'] {
        assert!(!is_extended_pictographic(c), "{c:?}");
    }
}
#[test]
fn gaps_inclusive_endpoints_and_supplementary_ranges() {
    for c in ['\u{1f1e6}', '\u{1f1ff}'] {
        assert_eq!(word_break(c), RegionalIndicator);
    }
    for c in ['\u{1f1e5}', '\u{1f200}', '\u{10ffff}', '\u{378}'] {
        assert_eq!(word_break(c), Other, "{c:?}");
    }
    assert_eq!(word_break('\u{10400}'), ALetter);
    assert_eq!(word_break('\u{e0100}'), Extend);
    assert_eq!(word_break('\u{e01ef}'), Extend);
    assert_eq!(word_break('\u{e01f0}'), Other);
    assert!(!is_extended_pictographic('\0'));
    for c in 0x1fc00..=0x1fffd {
        assert!(is_extended_pictographic(char::from_u32(c).unwrap()));
    }
}
