//! Pinned Unicode17 word-boundary properties, without boundary evaluation.
//! Word_Break and Extended_Pictographic are independent, overlapping properties.
//! This selects no CSS words, capitalize behavior, tailoring or source domains.
mod tables;

pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBreak {
    Other,
    DoubleQuote,
    SingleQuote,
    HebrewLetter,
    Cr,
    Lf,
    Newline,
    Extend,
    RegionalIndicator,
    Format,
    Katakana,
    ALetter,
    MidLetter,
    MidNum,
    MidNumLet,
    Numeric,
    ExtendNumLet,
    Zwj,
    WSegSpace,
}
/// Exact Word_Break assignment; unlisted scalars have Other.
/// AHLetter/MidNumLetQ are rule macros, not additional property values.
pub fn word_break(scalar: char) -> WordBreak {
    let end = tables::WORD_BREAK.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::WORD_BREAK[i].1)
        .map_or(WordBreak::Other, |i| tables::WORD_BREAK[i].2)
}
/// Exact independent EP membership, including its reserved/unassigned ranges.
/// This is not Emoji/Emoji_Presentation or a Word_Break=Other-only lookup.
pub fn is_extended_pictographic(scalar: char) -> bool {
    let end = tables::EXTENDED_PICTOGRAPHIC.partition_point(|(start, _)| *start <= scalar);
    end.checked_sub(1)
        .is_some_and(|i| scalar <= tables::EXTENDED_PICTOGRAPHIC[i].1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_tables_are_sorted_disjoint_and_merged() {
        assert_eq!(tables::WORD_BREAK.len(), 1100);
        assert_eq!(tables::EXTENDED_PICTOGRAPHIC.len(), 156);
        for &(a, b, value) in tables::WORD_BREAK {
            assert!(a <= b);
            assert_ne!(value, WordBreak::Other);
            assert_eq!(word_break(a), value);
            assert_eq!(word_break(b), value);
        }
        for pair in tables::WORD_BREAK.windows(2) {
            assert!(pair[0].1 < pair[1].0);
            assert!(pair[0].1 as u32 + 1 != pair[1].0 as u32 || pair[0].2 != pair[1].2);
        }
        for &(a, b) in tables::EXTENDED_PICTOGRAPHIC {
            assert!(a <= b);
            assert!(is_extended_pictographic(a));
            assert!(is_extended_pictographic(b));
        }
        for pair in tables::EXTENDED_PICTOGRAPHIC.windows(2) {
            assert!((pair[0].1 as u32) + 1 < (pair[1].0 as u32));
        }
        assert_eq!(
            tables::WORD_BREAK
                .iter()
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum::<usize>(),
            37958
        );
        assert_eq!(
            tables::EXTENDED_PICTOGRAPHIC
                .iter()
                .map(|(a, b)| *b as usize - *a as usize + 1)
                .sum::<usize>(),
            2848
        );
    }
}
