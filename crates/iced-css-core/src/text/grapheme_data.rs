//! Independent Unicode17 grapheme and Indic conjunct property assignments.
//! This evaluates no boundaries, CSS first character, shaping or caret policy.
//! Extended_Pictographic remains available from the same-pinned word_data service.
mod tables;
pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeBreak {
    Other,
    Prepend,
    Cr,
    Lf,
    Control,
    Extend,
    RegionalIndicator,
    SpacingMark,
    L,
    V,
    T,
    Lv,
    Lvt,
    Zwj,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicConjunctBreak {
    None,
    Consonant,
    Linker,
    Extend,
}
/// Exact GCB assignment; unlisted scalars have Other. GCB Extend is not InCB Extend.
pub fn grapheme_break(scalar: char) -> GraphemeBreak {
    let end = tables::GRAPHEME_BREAK.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::GRAPHEME_BREAK[i].1)
        .map_or(GraphemeBreak::Other, |i| tables::GRAPHEME_BREAK[i].2)
}
/// Exact InCB assignment; None is a property value, not a missing query result.
pub fn indic_conjunct_break(scalar: char) -> IndicConjunctBreak {
    let end = tables::INDIC_CONJUNCT_BREAK.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::INDIC_CONJUNCT_BREAK[i].1)
        .map_or(IndicConjunctBreak::None, |i| {
            tables::INDIC_CONJUNCT_BREAK[i].2
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_tables_preserve_shape_endpoints_and_all_class_counts() {
        assert_eq!(tables::GRAPHEME_BREAK.len(), 1386);
        assert_eq!(tables::INDIC_CONJUNCT_BREAK.len(), 473);
        for &(a, b, v) in tables::GRAPHEME_BREAK {
            assert!(a <= b);
            assert_ne!(v, GraphemeBreak::Other);
            assert_eq!(grapheme_break(a), v);
            assert_eq!(grapheme_break(b), v);
        }
        for p in tables::GRAPHEME_BREAK.windows(2) {
            assert!(p[0].1 < p[1].0);
            assert!((p[0].1 as u32) + 1 != p[1].0 as u32 || p[0].2 != p[1].2);
        }
        for &(a, b, v) in tables::INDIC_CONJUNCT_BREAK {
            assert!(a <= b);
            assert_ne!(v, IndicConjunctBreak::None);
            assert_eq!(indic_conjunct_break(a), v);
            assert_eq!(indic_conjunct_break(b), v);
        }
        for p in tables::INDIC_CONJUNCT_BREAK.windows(2) {
            assert!(p[0].1 < p[1].0);
            assert!((p[0].1 as u32) + 1 != p[1].0 as u32 || p[0].2 != p[1].2);
        }
        use GraphemeBreak as G;
        for (value, ranges, members) in [
            (G::Cr, 1, 1),
            (G::Lf, 1, 1),
            (G::Control, 19, 3893),
            (G::Extend, 384, 2237),
            (G::L, 2, 125),
            (G::V, 4, 100),
            (G::T, 2, 137),
            (G::Lv, 399, 399),
            (G::Lvt, 399, 10773),
            (G::Prepend, 15, 27),
            (G::RegionalIndicator, 1, 26),
            (G::SpacingMark, 158, 381),
            (G::Zwj, 1, 1),
        ] {
            let matched: Vec<_> = tables::GRAPHEME_BREAK
                .iter()
                .filter(|(_, _, v)| *v == value)
                .collect();
            assert_eq!(matched.len(), ranges);
            assert_eq!(
                matched
                    .iter()
                    .map(|(a, b, _)| *b as usize - *a as usize + 1)
                    .sum::<usize>(),
                members
            );
        }
        use IndicConjunctBreak as I;
        for (value, ranges, members) in [
            (I::Consonant, 76, 911),
            (I::Linker, 20, 20),
            (I::Extend, 377, 2217),
        ] {
            let matched: Vec<_> = tables::INDIC_CONJUNCT_BREAK
                .iter()
                .filter(|(_, _, v)| *v == value)
                .collect();
            assert_eq!(matched.len(), ranges);
            assert_eq!(
                matched
                    .iter()
                    .map(|(a, b, _)| *b as usize - *a as usize + 1)
                    .sum::<usize>(),
                members
            );
        }
    }
}
