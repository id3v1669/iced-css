//! Exact Unicode 17 raw bidi properties, without paragraph or line analysis.
//! Raw bracket mates are not normalized opening keys. Mirroring character data
//! neither transforms source text nor selects a font glyph.
mod tables;

/// Pinned Unicode data edition.
pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);

/// The 23 original Bidi_Class values; no directional resolution is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    /// AL: Arabic_Letter.
    Al,
    /// AN: Arabic_Number.
    An,
    /// B: Paragraph_Separator.
    B,
    /// BN: Boundary_Neutral.
    Bn,
    /// CS: Common_Separator.
    Cs,
    /// EN: European_Number.
    En,
    /// ES: European_Separator.
    Es,
    /// ET: European_Terminator.
    Et,
    /// FSI: First_Strong_Isolate.
    Fsi,
    /// L: Left_To_Right.
    L,
    /// LRE: Left_To_Right_Embedding.
    Lre,
    /// LRI: Left_To_Right_Isolate.
    Lri,
    /// LRO: Left_To_Right_Override.
    Lro,
    /// NSM: Nonspacing_Mark.
    Nsm,
    /// ON: Other_Neutral.
    On,
    /// PDF: Pop_Directional_Format.
    Pdf,
    /// PDI: Pop_Directional_Isolate.
    Pdi,
    /// R: Right_To_Left.
    R,
    /// RLE: Right_To_Left_Embedding.
    Rle,
    /// RLI: Right_To_Left_Isolate.
    Rli,
    /// RLO: Right_To_Left_Override.
    Rlo,
    /// S: Segment_Separator.
    S,
    /// WS: White_Space.
    Ws,
}

/// Raw Bidi_Paired_Bracket_Type; absence does not imply an identity pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiPairedBracketType {
    /// n: None.
    None,
    /// o: Open.
    Open,
    /// c: Close.
    Close,
}

/// Original Bidi_Class, including layered defaults for unassigned characters.
/// A table miss is Left_To_Right (L); explicit overrides are retained.
pub fn bidi_class(scalar: char) -> BidiClass {
    let end = tables::BIDI_CLASS.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::BIDI_CLASS[i].1)
        .map_or(BidiClass::L, |i| tables::BIDI_CLASS[i].2)
}

/// Raw reciprocal mate, or None for an absent Bidi_Paired_Bracket value.
/// U+2329 maps to U+232A, not the normalized opening representative U+3008.
pub fn bidi_paired_bracket(scalar: char) -> Option<char> {
    tables::BIDI_BRACKETS
        .binary_search_by_key(&scalar, |&(key, _, _)| key)
        .ok()
        .map(|i| tables::BIDI_BRACKETS[i].1)
}

/// Original bracket type, independently exposing the property's None default.
pub fn bidi_paired_bracket_type(scalar: char) -> BidiPairedBracketType {
    tables::BIDI_BRACKETS
        .binary_search_by_key(&scalar, |&(key, _, _)| key)
        .map_or(BidiPairedBracketType::None, |i| tables::BIDI_BRACKETS[i].2)
}

/// Original Bidi_Mirrored flag, including true characters with no mirroring mapping.
pub fn is_bidi_mirrored(scalar: char) -> bool {
    let end = tables::BIDI_MIRRORED.partition_point(|(start, _)| *start <= scalar);
    end.checked_sub(1)
        .is_some_and(|i| scalar <= tables::BIDI_MIRRORED[i].1)
}

/// Informative Bidi_Mirroring_Glyph character mapping, or None when absent.
/// This is neither an identity fallback nor actual mirrored font-glyph selection.
pub fn bidi_mirroring_glyph(scalar: char) -> Option<char> {
    tables::BIDI_MIRRORING_GLYPH
        .binary_search_by_key(&scalar, |&(key, _)| key)
        .ok()
        .map(|i| tables::BIDI_MIRRORING_GLYPH[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_tables_preserve_nondefault_members_and_gaps() {
        assert_eq!(DATA_VERSION, (17, 0, 0));
        assert_eq!(tables::BIDI_CLASS.len(), 763);
        assert_eq!(tables::BIDI_MIRRORED.len(), 114);
        for &(a, b, value) in tables::BIDI_CLASS {
            assert!(a <= b);
            assert_ne!(value, BidiClass::L);
            assert!((b as u32) < 0xD800 || (a as u32) > 0xDFFF);
            assert_eq!(bidi_class(a), value);
            assert_eq!(bidi_class(b), value);
        }
        for pair in tables::BIDI_CLASS.windows(2) {
            assert!(pair[0].1 < pair[1].0);
            assert!(pair[0].1 as u32 + 1 != pair[1].0 as u32 || pair[0].2 != pair[1].2);
            if let Some(gap) = char::from_u32(pair[0].1 as u32 + 1)
                && gap < pair[1].0
            {
                assert_eq!(bidi_class(gap), BidiClass::L);
            }
        }
        for &(a, b) in tables::BIDI_MIRRORED {
            assert!(a <= b);
            assert!(is_bidi_mirrored(a));
            assert!(is_bidi_mirrored(b));
        }
        for pair in tables::BIDI_MIRRORED.windows(2) {
            assert!(pair[0].1 as u32 + 1 < pair[1].0 as u32);
            if let Some(gap) = char::from_u32(pair[0].1 as u32 + 1) {
                assert!(!is_bidi_mirrored(gap));
            }
        }
        assert_eq!(
            tables::BIDI_MIRRORED
                .iter()
                .map(|&(a, b)| b as usize - a as usize + 1)
                .sum::<usize>(),
            554
        );
        for (value, expected) in [
            (BidiClass::Al, 1731),
            (BidiClass::An, 73),
            (BidiClass::B, 7),
            (BidiClass::Bn, 4016),
            (BidiClass::Cs, 15),
            (BidiClass::En, 178),
            (BidiClass::Es, 12),
            (BidiClass::Et, 92),
            (BidiClass::Fsi, 1),
            (BidiClass::Lre, 1),
            (BidiClass::Lri, 1),
            (BidiClass::Lro, 1),
            (BidiClass::Nsm, 2067),
            (BidiClass::On, 6854),
            (BidiClass::Pdf, 1),
            (BidiClass::Pdi, 1),
            (BidiClass::R, 3631),
            (BidiClass::Rle, 1),
            (BidiClass::Rli, 1),
            (BidiClass::Rlo, 1),
            (BidiClass::S, 3),
            (BidiClass::Ws, 17),
        ] {
            let count: usize = tables::BIDI_CLASS
                .iter()
                .filter(|r| r.2 == value)
                .map(|&(a, b, _)| b as usize - a as usize + 1)
                .sum();
            assert_eq!(count, expected, "{value:?}");
        }
    }

    #[test]
    fn exact_key_tables_preserve_pair_types_and_mirroring_independence() {
        assert_eq!(tables::BIDI_BRACKETS.len(), 128);
        assert_eq!(tables::BIDI_MIRRORING_GLYPH.len(), 428);
        for pair in tables::BIDI_BRACKETS.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        for pair in tables::BIDI_MIRRORING_GLYPH.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        for &(c, mate, kind) in tables::BIDI_BRACKETS {
            assert_ne!(kind, BidiPairedBracketType::None);
            assert_eq!(bidi_paired_bracket(c), Some(mate));
            assert_eq!(bidi_paired_bracket(mate), Some(c));
            assert_eq!(bidi_paired_bracket_type(c), kind);
            assert_ne!(bidi_paired_bracket_type(mate), kind);
            assert_eq!(bidi_class(c), BidiClass::On);
            assert!(is_bidi_mirrored(c));
            assert_eq!(bidi_mirroring_glyph(c), Some(mate));
        }
        assert_eq!(
            tables::BIDI_BRACKETS
                .iter()
                .filter(|r| r.2 == BidiPairedBracketType::Open)
                .count(),
            64
        );
        for &(c, mate) in tables::BIDI_MIRRORING_GLYPH {
            assert_eq!(bidi_mirroring_glyph(c), Some(mate));
            assert_eq!(bidi_mirroring_glyph(mate), Some(c));
            assert!(is_bidi_mirrored(c));
        }
    }
}
