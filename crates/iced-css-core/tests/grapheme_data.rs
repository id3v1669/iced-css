use iced_css_core::text::{
    grapheme_data::{
        DATA_VERSION, GraphemeBreak as G, IndicConjunctBreak as I, grapheme_break,
        indic_conjunct_break,
    },
    word_data::is_extended_pictographic,
};
#[test]
fn all_gcb_and_incb_values_have_literal_meanings() {
    assert_eq!(DATA_VERSION, (17, 0, 0));
    for (c, v) in [
        ('a', G::Other),
        ('\u{600}', G::Prepend),
        ('\r', G::Cr),
        ('\n', G::Lf),
        ('\0', G::Control),
        ('\u{300}', G::Extend),
        ('\u{1f1e6}', G::RegionalIndicator),
        ('\u{903}', G::SpacingMark),
        ('\u{1100}', G::L),
        ('\u{1160}', G::V),
        ('\u{11a8}', G::T),
        ('\u{ac00}', G::Lv),
        ('\u{ac01}', G::Lvt),
        ('\u{200d}', G::Zwj),
    ] {
        assert_eq!(grapheme_break(c), v, "{c:?}");
    }
    for (c, v) in [
        ('a', I::None),
        ('\u{915}', I::Consonant),
        ('\u{94d}', I::Linker),
        ('\u{300}', I::Extend),
    ] {
        assert_eq!(indic_conjunct_break(c), v, "{c:?}");
    }
}
#[test]
fn independent_overlap_and_existing_ep_are_not_equivalences() {
    for (c, g, i) in [
        ('\u{200c}', G::Extend, I::None),
        ('\u{200d}', G::Zwj, I::Extend),
        ('\u{94d}', G::Extend, I::Linker),
        ('\u{915}', G::Other, I::Consonant),
        ('\u{300}', G::Extend, I::Extend),
    ] {
        assert_eq!(grapheme_break(c), g);
        assert_eq!(indic_conjunct_break(c), i);
        assert_eq!(indic_conjunct_break(c), i);
        assert_eq!(grapheme_break(c), g);
    }
    for c in ['😀', '\u{1fc00}', '\u{1fffd}'] {
        assert!(is_extended_pictographic(c));
        assert_eq!(grapheme_break(c), G::Other);
        assert_eq!(indic_conjunct_break(c), I::None);
    }
    assert!(!is_extended_pictographic('a'));
    assert_eq!(grapheme_break('a'), G::Other);
    assert_eq!(indic_conjunct_break('a'), I::None);
}
#[test]
fn inclusive_ranges_holes_and_non_hangul_conjoining_values() {
    for c in ['\u{16d63}', '\u{16d67}', '\u{16d6a}'] {
        assert_eq!(grapheme_break(c), G::V);
    }
    for c in ['\u{16d62}', '\u{16d64}', '\u{16d6b}'] {
        assert_eq!(grapheme_break(c), G::Other);
    }
    for c in ['\u{e0100}', '\u{e01ef}'] {
        assert_eq!(grapheme_break(c), G::Extend);
        assert_eq!(indic_conjunct_break(c), I::Extend);
    }
    assert_eq!(grapheme_break('\u{e01f0}'), G::Control);
    assert_eq!(indic_conjunct_break('\u{e01f0}'), I::None);
    for c in ['\u{378}', '\u{10ffff}'] {
        assert_eq!(grapheme_break(c), G::Other);
        assert_eq!(indic_conjunct_break(c), I::None);
    }
    for c in ['\u{1f1e6}', '\u{1f1ff}'] {
        assert_eq!(grapheme_break(c), G::RegionalIndicator);
    }
    assert_eq!(grapheme_break('\u{1f200}'), G::Other);
}
