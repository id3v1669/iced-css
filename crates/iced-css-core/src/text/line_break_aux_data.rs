//! Exact original Unicode17 East_Asian_Width and General_Category properties.
//! These lookups do not apply LB1/LB9/LB10, compute breaks, or select CSS tailoring.
//! Extended_Pictographic remains available from the same-pinned word_data service.
mod tables;

pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);
/// Unicode property values, not measured glyph widths or terminal column counts.
/// Neutral (N) and Narrow (Na) are distinct. UAX14's F/W/H set excludes Ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EastAsianWidth {
    Ambiguous,
    Fullwidth,
    Halfwidth,
    Neutral,
    Narrow,
    Wide,
}
/// The 30 leaf source-schema values; aggregate aliases are not assignments.
/// Surrogate (Cs) is never returned for a Rust char. Unassigned includes valid
/// noncharacter scalars; PrivateUse is a separate category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralCategory {
    UppercaseLetter,
    LowercaseLetter,
    TitlecaseLetter,
    ModifierLetter,
    OtherLetter,
    NonspacingMark,
    SpacingMark,
    EnclosingMark,
    DecimalNumber,
    LetterNumber,
    OtherNumber,
    ConnectorPunctuation,
    DashPunctuation,
    OpenPunctuation,
    ClosePunctuation,
    InitialPunctuation,
    FinalPunctuation,
    OtherPunctuation,
    MathSymbol,
    CurrencySymbol,
    ModifierSymbol,
    OtherSymbol,
    SpaceSeparator,
    LineSeparator,
    ParagraphSeparator,
    Control,
    Format,
    Surrogate,
    PrivateUse,
    Unassigned,
}
/// Exact original EAW, preserving explicit wide unassigned areas; default Neutral.
/// A later LB10 effective substitution must not overwrite these original facts.
pub fn east_asian_width(scalar: char) -> EastAsianWidth {
    let end = tables::EAST_ASIAN_WIDTH.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::EAST_ASIAN_WIDTH[i].1)
        .map_or(EastAsianWidth::Neutral, |i| tables::EAST_ASIAN_WIDTH[i].2)
}
/// Exact original leaf GC, including expanded UnicodeData range interiors.
/// Missing table entries are Unassigned; this is not an assigned-character filter.
pub fn general_category(scalar: char) -> GeneralCategory {
    let end = tables::GENERAL_CATEGORY.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::GENERAL_CATEGORY[i].1)
        .map_or(GeneralCategory::Unassigned, |i| {
            tables::GENERAL_CATEGORY[i].2
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_tables_are_sorted_disjoint_merged_and_nondefault() {
        assert_eq!(tables::EAST_ASIAN_WIDTH.len(), 318);
        assert_eq!(tables::GENERAL_CATEGORY.len(), 3408);
        for &(a, b, v) in tables::EAST_ASIAN_WIDTH {
            assert!(a <= b);
            assert_ne!(v, EastAsianWidth::Neutral);
            assert_eq!(east_asian_width(a), v);
            assert_eq!(east_asian_width(b), v);
        }
        for p in tables::EAST_ASIAN_WIDTH.windows(2) {
            assert!(p[0].1 < p[1].0);
            assert!(p[0].1 as u32 + 1 != p[1].0 as u32 || p[0].2 != p[1].2);
        }
        for &(a, b, v) in tables::GENERAL_CATEGORY {
            assert!(a <= b);
            assert_ne!(v, GeneralCategory::Unassigned);
            assert_ne!(v, GeneralCategory::Surrogate);
            assert_eq!(general_category(a), v);
            assert_eq!(general_category(b), v);
        }
        for p in tables::GENERAL_CATEGORY.windows(2) {
            assert!(p[0].1 < p[1].0);
            assert!(p[0].1 as u32 + 1 != p[1].0 as u32 || p[0].2 != p[1].2);
        }
        assert_eq!(
            tables::EAST_ASIAN_WIDTH
                .iter()
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum::<usize>(),
            321849
        );
        assert_eq!(
            tables::GENERAL_CATEGORY
                .iter()
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum::<usize>(),
            297334
        );
    }
    #[test]
    fn pinned_per_class_member_counts() {
        for (value, expected) in [
            (EastAsianWidth::Ambiguous, 138739),
            (EastAsianWidth::Fullwidth, 104),
            (EastAsianWidth::Halfwidth, 123),
            (EastAsianWidth::Narrow, 111),
            (EastAsianWidth::Wide, 182772),
        ] {
            let count: usize = tables::EAST_ASIAN_WIDTH
                .iter()
                .filter(|(_, _, v)| *v == value)
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum();
            assert_eq!(count, expected, "{value:?}");
        }
        for (value, expected) in [
            (GeneralCategory::Control, 65),
            (GeneralCategory::Format, 170),
            (GeneralCategory::PrivateUse, 137468),
            (GeneralCategory::LowercaseLetter, 2283),
            (GeneralCategory::ModifierLetter, 410),
            (GeneralCategory::OtherLetter, 141062),
            (GeneralCategory::TitlecaseLetter, 31),
            (GeneralCategory::UppercaseLetter, 1886),
            (GeneralCategory::SpacingMark, 471),
            (GeneralCategory::EnclosingMark, 13),
            (GeneralCategory::NonspacingMark, 2059),
            (GeneralCategory::DecimalNumber, 770),
            (GeneralCategory::LetterNumber, 239),
            (GeneralCategory::OtherNumber, 915),
            (GeneralCategory::ConnectorPunctuation, 10),
            (GeneralCategory::DashPunctuation, 27),
            (GeneralCategory::ClosePunctuation, 77),
            (GeneralCategory::FinalPunctuation, 10),
            (GeneralCategory::InitialPunctuation, 12),
            (GeneralCategory::OtherPunctuation, 641),
            (GeneralCategory::OpenPunctuation, 79),
            (GeneralCategory::CurrencySymbol, 64),
            (GeneralCategory::ModifierSymbol, 125),
            (GeneralCategory::MathSymbol, 960),
            (GeneralCategory::OtherSymbol, 7468),
            (GeneralCategory::LineSeparator, 1),
            (GeneralCategory::ParagraphSeparator, 1),
            (GeneralCategory::SpaceSeparator, 17),
        ] {
            let count: usize = tables::GENERAL_CATEGORY
                .iter()
                .filter(|(_, _, v)| *v == value)
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum();
            assert_eq!(count, expected, "{value:?}");
        }
    }
}
