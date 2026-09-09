use iced_css_core::text::{line_break_aux_data::*, word_data::is_extended_pictographic};

#[test]
fn all_reachable_values_and_independent_original_properties() {
    assert_eq!(DATA_VERSION, (17, 0, 0));
    // Literal expectations from the independent DerivedEAW/UnicodeData preparation.
    for (c, width, category) in [
        ('\u{0}', EastAsianWidth::Neutral, GeneralCategory::Control),
        (
            '\u{20}',
            EastAsianWidth::Narrow,
            GeneralCategory::SpaceSeparator,
        ),
        (
            '\u{21}',
            EastAsianWidth::Narrow,
            GeneralCategory::OtherPunctuation,
        ),
        (
            '\u{24}',
            EastAsianWidth::Narrow,
            GeneralCategory::CurrencySymbol,
        ),
        (
            '\u{28}',
            EastAsianWidth::Narrow,
            GeneralCategory::OpenPunctuation,
        ),
        (
            '\u{29}',
            EastAsianWidth::Narrow,
            GeneralCategory::ClosePunctuation,
        ),
        (
            '\u{2b}',
            EastAsianWidth::Narrow,
            GeneralCategory::MathSymbol,
        ),
        (
            '\u{2d}',
            EastAsianWidth::Narrow,
            GeneralCategory::DashPunctuation,
        ),
        (
            '\u{30}',
            EastAsianWidth::Narrow,
            GeneralCategory::DecimalNumber,
        ),
        (
            '\u{41}',
            EastAsianWidth::Narrow,
            GeneralCategory::UppercaseLetter,
        ),
        (
            '\u{5e}',
            EastAsianWidth::Narrow,
            GeneralCategory::ModifierSymbol,
        ),
        (
            '\u{5f}',
            EastAsianWidth::Narrow,
            GeneralCategory::ConnectorPunctuation,
        ),
        (
            '\u{61}',
            EastAsianWidth::Narrow,
            GeneralCategory::LowercaseLetter,
        ),
        (
            '\u{a1}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::OtherPunctuation,
        ),
        (
            '\u{a6}',
            EastAsianWidth::Narrow,
            GeneralCategory::OtherSymbol,
        ),
        (
            '\u{aa}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{ab}',
            EastAsianWidth::Neutral,
            GeneralCategory::InitialPunctuation,
        ),
        ('\u{ad}', EastAsianWidth::Ambiguous, GeneralCategory::Format),
        (
            '\u{b2}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::OtherNumber,
        ),
        (
            '\u{bb}',
            EastAsianWidth::Neutral,
            GeneralCategory::FinalPunctuation,
        ),
        (
            '\u{1c5}',
            EastAsianWidth::Neutral,
            GeneralCategory::TitlecaseLetter,
        ),
        (
            '\u{2b0}',
            EastAsianWidth::Neutral,
            GeneralCategory::ModifierLetter,
        ),
        (
            '\u{300}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::NonspacingMark,
        ),
        (
            '\u{378}',
            EastAsianWidth::Neutral,
            GeneralCategory::Unassigned,
        ),
        (
            '\u{488}',
            EastAsianWidth::Neutral,
            GeneralCategory::EnclosingMark,
        ),
        (
            '\u{903}',
            EastAsianWidth::Neutral,
            GeneralCategory::SpacingMark,
        ),
        (
            '\u{1100}',
            EastAsianWidth::Wide,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{16ee}',
            EastAsianWidth::Neutral,
            GeneralCategory::LetterNumber,
        ),
        ('\u{200c}', EastAsianWidth::Neutral, GeneralCategory::Format),
        (
            '\u{2028}',
            EastAsianWidth::Neutral,
            GeneralCategory::LineSeparator,
        ),
        (
            '\u{2029}',
            EastAsianWidth::Neutral,
            GeneralCategory::ParagraphSeparator,
        ),
        (
            '\u{20a9}',
            EastAsianWidth::Halfwidth,
            GeneralCategory::CurrencySymbol,
        ),
        (
            '\u{3000}',
            EastAsianWidth::Fullwidth,
            GeneralCategory::SpaceSeparator,
        ),
        (
            '\u{3401}',
            EastAsianWidth::Wide,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{4e01}',
            EastAsianWidth::Wide,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{ac01}',
            EastAsianWidth::Wide,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{e000}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::PrivateUse,
        ),
        (
            '\u{e001}',
            EastAsianWidth::Ambiguous,
            GeneralCategory::PrivateUse,
        ),
        (
            '\u{ff01}',
            EastAsianWidth::Fullwidth,
            GeneralCategory::OtherPunctuation,
        ),
        (
            '\u{ff66}',
            EastAsianWidth::Halfwidth,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{20001}',
            EastAsianWidth::Wide,
            GeneralCategory::OtherLetter,
        ),
        (
            '\u{3fffd}',
            EastAsianWidth::Wide,
            GeneralCategory::Unassigned,
        ),
        (
            '\u{3fffe}',
            EastAsianWidth::Neutral,
            GeneralCategory::Unassigned,
        ),
        (
            '\u{10ffff}',
            EastAsianWidth::Neutral,
            GeneralCategory::Unassigned,
        ),
    ] {
        assert_eq!(east_asian_width(c), width, "{c:?}");
        assert_eq!(general_category(c), category, "{c:?}");
        assert_eq!(east_asian_width(c), width);
    }
}

#[test]
fn original_mark_and_lb10_a_properties_remain_distinct() {
    assert_eq!(general_category('A'), GeneralCategory::UppercaseLetter);
    assert_eq!(east_asian_width('A'), EastAsianWidth::Narrow);
    assert!(!is_extended_pictographic('A'));
    assert_eq!(general_category('\u{300}'), GeneralCategory::NonspacingMark);
    assert_eq!(east_asian_width('\u{300}'), EastAsianWidth::Ambiguous);
    assert_eq!(general_category('\u{903}'), GeneralCategory::SpacingMark);
    assert_eq!(general_category('«'), GeneralCategory::InitialPunctuation);
    assert_eq!(general_category('»'), GeneralCategory::FinalPunctuation);
    // Reserved pictographic input remains Unassigned; neither property aliases the other.
    assert!(is_extended_pictographic('\u{1fc00}'));
    assert_eq!(general_category('\u{1fc00}'), GeneralCategory::Unassigned);
    assert!(!is_extended_pictographic('\u{378}'));
    assert_eq!(general_category('\u{378}'), GeneralCategory::Unassigned);
}
#[test]
fn supplementary_defaults_and_range_interiors() {
    for c in ['\u{3401}', '\u{4e01}', '\u{20001}'] {
        assert_eq!(general_category(c), GeneralCategory::OtherLetter);
        assert_eq!(east_asian_width(c), EastAsianWidth::Wide);
    }
    assert_eq!(general_category('\u{ac01}'), GeneralCategory::OtherLetter);
    assert_eq!(general_category('\u{e001}'), GeneralCategory::PrivateUse);
    assert_eq!(general_category('\u{f0001}'), GeneralCategory::PrivateUse);
    assert_eq!(general_category('\u{3fffd}'), GeneralCategory::Unassigned);
    assert_eq!(east_asian_width('\u{3fffd}'), EastAsianWidth::Wide);
    assert_eq!(east_asian_width('\u{3fffe}'), EastAsianWidth::Neutral);
    assert_eq!(general_category('\u{10ffff}'), GeneralCategory::Unassigned);
    // Surrogate belongs to the source schema, but no char lookup can return it.
    assert_ne!(general_category('\u{e000}'), GeneralCategory::Surrogate);
}
