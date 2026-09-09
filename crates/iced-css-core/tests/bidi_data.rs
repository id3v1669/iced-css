use iced_css_core::text::bidi_data::{
    BidiClass, BidiPairedBracketType, bidi_class, bidi_mirroring_glyph, bidi_paired_bracket,
    bidi_paired_bracket_type, is_bidi_mirrored,
};

#[test]
fn every_class_and_layered_defaults_preserve_raw_values() {
    for (scalar, expected) in [
        ('\u{0608}', BidiClass::Al),
        ('\u{0600}', BidiClass::An),
        ('\u{000A}', BidiClass::B),
        ('\u{0000}', BidiClass::Bn),
        ('\u{002C}', BidiClass::Cs),
        ('\u{0030}', BidiClass::En),
        ('\u{002B}', BidiClass::Es),
        ('\u{0023}', BidiClass::Et),
        ('\u{2068}', BidiClass::Fsi),
        ('\u{0041}', BidiClass::L),
        ('\u{202A}', BidiClass::Lre),
        ('\u{2066}', BidiClass::Lri),
        ('\u{202D}', BidiClass::Lro),
        ('\u{0300}', BidiClass::Nsm),
        ('\u{0021}', BidiClass::On),
        ('\u{202C}', BidiClass::Pdf),
        ('\u{2069}', BidiClass::Pdi),
        ('\u{0590}', BidiClass::R),
        ('\u{202B}', BidiClass::Rle),
        ('\u{2067}', BidiClass::Rli),
        ('\u{202E}', BidiClass::Rlo),
        ('\u{0009}', BidiClass::S),
        ('\u{000C}', BidiClass::Ws),
        ('\u{0600}', BidiClass::An),
        ('\u{0860}', BidiClass::Al),
        ('\u{20A0}', BidiClass::Et),
        ('\u{10800}', BidiClass::R),
        ('\u{10D00}', BidiClass::Al),
        ('\u{1EF00}', BidiClass::R),
        ('\u{D7FF}', BidiClass::L),
        ('\u{E000}', BidiClass::L),
        ('\u{10FFFF}', BidiClass::Bn),
    ] {
        assert_eq!(bidi_class(scalar), expected, "U+{:04X}", scalar as u32);
    }
}

#[test]
fn pairs_keep_raw_reciprocal_values_and_tick_exceptions() {
    for (opening, closing) in [
        ('(', ')'),
        ('\u{2329}', '\u{232A}'),
        ('\u{3008}', '\u{3009}'),
        ('\u{298D}', '\u{2990}'),
        ('\u{298F}', '\u{298E}'),
    ] {
        assert_eq!(bidi_paired_bracket(opening), Some(closing));
        assert_eq!(bidi_paired_bracket(closing), Some(opening));
        assert_eq!(
            bidi_paired_bracket_type(opening),
            BidiPairedBracketType::Open
        );
        assert_eq!(
            bidi_paired_bracket_type(closing),
            BidiPairedBracketType::Close
        );
        assert_eq!(bidi_mirroring_glyph(opening), Some(closing));
        assert_eq!(bidi_mirroring_glyph(closing), Some(opening));
        assert!(is_bidi_mirrored(opening));
        assert!(is_bidi_mirrored(closing));
    }
    for scalar in ['A', '\u{FD3E}', '\u{FD3F}', '\u{10FFFF}'] {
        assert_eq!(bidi_paired_bracket(scalar), None);
        assert_eq!(
            bidi_paired_bracket_type(scalar),
            BidiPairedBracketType::None
        );
        assert_eq!(bidi_mirroring_glyph(scalar), None);
        assert!(!is_bidi_mirrored(scalar));
    }
}

#[test]
fn mirrored_flag_is_independent_of_optional_mappings_and_brackets() {
    assert!(is_bidi_mirrored('\u{2140}'));
    assert_eq!(bidi_mirroring_glyph('\u{2140}'), None);
    assert_eq!(bidi_paired_bracket('\u{2140}'), None);
    assert_eq!(
        bidi_paired_bracket_type('\u{2140}'),
        BidiPairedBracketType::None
    );
    assert!(is_bidi_mirrored('<'));
    assert_eq!(bidi_mirroring_glyph('<'), Some('>'));
    assert_eq!(bidi_paired_bracket('<'), None);
    assert_eq!(bidi_paired_bracket_type('<'), BidiPairedBracketType::None);
}
