use iced_css_core::text::line_break_data::{DATA_VERSION, LineBreak as L, line_break};
#[test]
fn all_reachable_raw_classes_preserve_source_meanings() {
    assert_eq!(DATA_VERSION, (17, 0, 0));
    for (c, expected) in [
        ('\u{a7}', L::Ai),
        ('\u{1b05}', L::Ak),
        ('\u{23}', L::Al),
        ('\u{11003}', L::Ap),
        ('\u{1b50}', L::As),
        ('\u{2014}', L::B2),
        ('\u{9}', L::Ba),
        ('\u{b4}', L::Bb),
        ('\u{b}', L::Bk),
        ('\u{fffc}', L::Cb),
        ('\u{3041}', L::Cj),
        ('\u{7d}', L::Cl),
        ('\u{0}', L::Cm),
        ('\u{29}', L::Cp),
        ('\u{d}', L::Cr),
        ('\u{261d}', L::Eb),
        ('\u{1f3fb}', L::Em),
        ('\u{21}', L::Ex),
        ('\u{a0}', L::Gl),
        ('\u{ac00}', L::H2),
        ('\u{ac01}', L::H3),
        ('\u{58a}', L::Hh),
        ('\u{5d0}', L::Hl),
        ('\u{2d}', L::Hy),
        ('\u{1b5c}', L::Id),
        ('\u{2024}', L::In),
        ('\u{2c}', L::Is),
        ('\u{1100}', L::Jl),
        ('\u{11a8}', L::Jt),
        ('\u{1160}', L::Jv),
        ('\u{a}', L::Lf),
        ('\u{85}', L::Nl),
        ('\u{17d6}', L::Ns),
        ('\u{30}', L::Nu),
        ('\u{28}', L::Op),
        ('\u{25}', L::Po),
        ('\u{24}', L::Pr),
        ('\u{22}', L::Qu),
        ('\u{1f1e6}', L::Ri),
        ('\u{e01}', L::Sa),
        ('\u{20}', L::Sp),
        ('\u{2f}', L::Sy),
        ('\u{1bf2}', L::Vf),
        ('\u{1b44}', L::Vi),
        ('\u{2060}', L::Wj),
        ('\u{378}', L::Xx),
        ('\u{200b}', L::Zw),
        ('\u{200d}', L::Zwj),
    ] {
        assert_eq!(line_break(c), expected, "{c:?}");
        assert_ne!(line_break(c), L::Sg);
    }
}
#[test]
fn all_hh_and_primary_derived_region_edges_are_literal() {
    for c in [
        '\u{58a}',
        '\u{5be}',
        '\u{1400}',
        '\u{2010}',
        '\u{2012}',
        '\u{2013}',
        '\u{2e17}',
        '\u{2e40}',
        '\u{2e5d}',
        '\u{10d6e}',
        '\u{10ead}',
    ] {
        assert_eq!(line_break(c), L::Hh);
    }
    for (c, expected) in [
        ('\u{0}', L::Cm),
        ('\u{a}', L::Lf),
        ('\u{d}', L::Cr),
        ('\u{20a0}', L::Pr),
        ('\u{20cf}', L::Pr),
        ('\u{20d0}', L::Cm),
        ('\u{33ff}', L::Id),
        ('\u{3400}', L::Id),
        ('\u{4dbf}', L::Id),
        ('\u{4dc0}', L::Al),
        ('\u{4e00}', L::Id),
        ('\u{9fff}', L::Id),
        ('\u{a000}', L::Id),
        ('\u{f900}', L::Id),
        ('\u{faff}', L::Id),
        ('\u{fb00}', L::Al),
        ('\u{1f7ff}', L::Id),
        ('\u{1f800}', L::Al),
        ('\u{1f8ff}', L::Xx),
        ('\u{1f900}', L::Al),
        ('\u{1faff}', L::Id),
        ('\u{1fb00}', L::Al),
        ('\u{1fc00}', L::Id),
        ('\u{1fffd}', L::Id),
        ('\u{1fffe}', L::Xx),
        ('\u{20000}', L::Id),
        ('\u{2fffd}', L::Id),
        ('\u{2fffe}', L::Xx),
        ('\u{30000}', L::Id),
        ('\u{3fffd}', L::Id),
        ('\u{3fffe}', L::Xx),
        ('\u{e000}', L::Xx),
        ('\u{f0000}', L::Xx),
        ('\u{10ffff}', L::Xx),
    ] {
        assert_eq!(line_break(c), expected, "{c:?}");
    }
}
#[test]
fn raw_values_are_not_lb1_or_lb10_resolution() {
    assert_eq!(line_break('\u{2757}'), L::Ai);
    assert_eq!(line_break('\u{e01}'), L::Sa);
    assert_eq!(line_break('\u{3041}'), L::Cj);
    assert_eq!(line_break('\u{378}'), L::Xx);
    assert_eq!(line_break('\u{300}'), L::Cm);
    assert_eq!(line_break('\u{200d}'), L::Zwj);
}
