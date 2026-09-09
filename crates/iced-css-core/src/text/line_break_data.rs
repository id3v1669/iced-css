//! Pinned Unicode17 raw Line_Break values, not break opportunities or resolution.
//! LB1, LB9 and LB10 belong to a separate ordered engine. This selects no CSS
//! wrapping, language tailoring, fitting lines or source-domain behavior.
mod tables;
pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);
/// All49 source-schema values. Sg denotes surrogates and cannot be returned for char.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBreak {
    Ai,
    Ak,
    Al,
    Ap,
    As,
    B2,
    Ba,
    Bb,
    Bk,
    Cb,
    Cj,
    Cl,
    Cm,
    Cp,
    Cr,
    Eb,
    Em,
    Ex,
    Gl,
    H2,
    H3,
    Hh,
    Hl,
    Hy,
    Id,
    In,
    Is,
    Jl,
    Jt,
    Jv,
    Lf,
    Nl,
    Ns,
    Nu,
    Op,
    Po,
    Pr,
    Qu,
    Ri,
    Sa,
    Sg,
    Sp,
    Sy,
    Vf,
    Vi,
    Wj,
    Xx,
    Zw,
    Zwj,
}
/// Exact raw scalar assignment. Unlisted scalars have Xx; no class is resolved
/// according to language, general category, neighboring input or normalization.
pub fn line_break(scalar: char) -> LineBreak {
    let end = tables::LINE_BREAK.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&i| scalar <= tables::LINE_BREAK[i].1)
        .map_or(LineBreak::Xx, |i| tables::LINE_BREAK[i].2)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_shape_endpoints_and_members_are_exact() {
        assert_eq!(tables::LINE_BREAK.len(), 2916);
        assert_eq!(
            tables::LINE_BREAK
                .iter()
                .map(|(a, b, _)| *b as usize - *a as usize + 1)
                .sum::<usize>(),
            217414
        );
        for &(a, b, v) in tables::LINE_BREAK {
            assert!(a <= b);
            assert_ne!(v, LineBreak::Xx);
            assert_ne!(v, LineBreak::Sg);
            assert_eq!(line_break(a), v);
            assert_eq!(line_break(b), v);
        }
        for p in tables::LINE_BREAK.windows(2) {
            assert!(p[0].1 < p[1].0);
            assert!((p[0].1 as u32) + 1 != p[1].0 as u32 || p[0].2 != p[1].2);
            if (p[0].1 as u32) + 1 < (p[1].0 as u32)
                && let Some(c) = char::from_u32(p[0].1 as u32 + 1)
            {
                assert_eq!(line_break(c), LineBreak::Xx);
            }
        }
    }
}
