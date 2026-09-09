use iced_css_core::text::line_boundaries::*;
use std::{mem::size_of, num::NonZeroUsize};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> LineBoundaryLimits {
    let n = nz(usize::MAX);
    LineBoundaryLimits {
        input_bytes: n,
        scalars: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn literal_ordered_breaks_and_empty_precedence() {
    use LineBreakKind::*;
    for (s, expected) in [
        ("", vec![]),
        ("a", vec![(1, Mandatory)]),
        ("a b", vec![(2, Optional), (3, Mandatory)]),
        ("a\r\nb", vec![(3, Mandatory), (4, Mandatory)]),
        ("\u{b}a", vec![(1, Mandatory), (2, Mandatory)]),
        ("\u{200b}  a", vec![(5, Optional), (6, Mandatory)]),
        ("a\u{200d}b", vec![(5, Mandatory)]),
        ("a\u{e31}b", vec![(5, Mandatory)]),
        (" .5", vec![(1, Optional), (3, Mandatory)]),
        ("$(.5)", vec![(5, Mandatory)]),
    ] {
        let r = evaluate_line_boundaries(s, limits()).unwrap();
        let actual: Vec<_> = r
            .opportunities()
            .iter()
            .map(|v| (v.offset(), v.kind()))
            .collect();
        assert_eq!(actual, expected, "{s:?}");
        assert_eq!(r.break_at(0), Some(Prohibited));
    }
}
#[test]
fn owner_utf8_queries_and_exact_admission() {
    use LineBreakKind::*;
    let source = String::from("é 😀");
    let r = evaluate_line_boundaries(&source, limits()).unwrap();
    assert_eq!(r.source().as_ptr(), source.as_ptr());
    assert_eq!(r.source(), source);
    assert_eq!(r.scalar_count(), 3);
    assert_eq!(r.work(), 13);
    for b in 0..=source.len() {
        let expected = if !source.is_char_boundary(b) {
            None
        } else {
            Some(match b {
                3 => Optional,
                7 => Mandatory,
                _ => Prohibited,
            })
        };
        assert_eq!(r.break_at(b), expected, "byte {b}");
    }
    assert_eq!(r.break_at(8), None);
    assert_eq!(r.break_at(usize::MAX), None);
    assert!(r.retained_bytes() >= 3 * size_of::<LineOpportunity>());
    assert!(r.peak_retained_bytes() > r.retained_bytes());
    let mut exact = limits();
    exact.input_bytes = nz(7);
    exact.scalars = nz(3);
    exact.work = nz(13);
    exact.retained_bytes = nz(r.peak_retained_bytes());
    assert!(evaluate_line_boundaries(&source, exact).is_ok());
    for (field, error) in [
        (0, LineBoundaryError::InputBytes),
        (1, LineBoundaryError::Scalars),
        (2, LineBoundaryError::Work),
        (3, LineBoundaryError::Bytes),
    ] {
        let mut l = exact;
        match field {
            0 => l.input_bytes = nz(6),
            1 => l.scalars = nz(2),
            2 => l.work = nz(12),
            _ => l.retained_bytes = nz(r.peak_retained_bytes() - 1),
        }
        assert_eq!(evaluate_line_boundaries(&source, l).unwrap_err(), error);
    }
    let tiny = nz(1);
    let r = evaluate_line_boundaries(
        "",
        LineBoundaryLimits {
            input_bytes: tiny,
            scalars: tiny,
            retained_bytes: tiny,
            work: tiny,
        },
    )
    .unwrap();
    assert_eq!(r.work(), 1);
    assert_eq!(r.retained_bytes(), 0);
    assert_eq!(r.peak_retained_bytes(), 0);
    assert_eq!(r.break_at(0), Some(Prohibited));
    let mut l = limits();
    l.work = nz(1);
    assert_eq!(
        evaluate_line_boundaries("😀", l).unwrap_err(),
        LineBoundaryError::Work
    );
    l.work = nz(5);
    assert!(evaluate_line_boundaries("😀", l).is_ok());
    l.scalars = nz(1);
    assert_eq!(
        evaluate_line_boundaries("ab", l).unwrap_err(),
        LineBoundaryError::Scalars
    );
    let e = LineBoundaryError::Work;
    assert!(e.to_string().contains("Work"));
    fn error_trait(_: &dyn std::error::Error) {}
    error_trait(&e);
}

#[test]
fn pinned_quote_numeric_brahmic_and_ignored_context_targets() {
    // Literal expectations independently prepared from original-rule regex oracle.
    for (source, expected) in [
        ("\u{201c}\u{20}\u{41}", vec![(5, LineBreakKind::Mandatory)]),
        ("\u{41}\u{201d}\u{20}", vec![(5, LineBreakKind::Mandatory)]),
        (
            "\u{3008}\u{201c}\u{3009}",
            vec![(9, LineBreakKind::Mandatory)],
        ),
        (
            "\u{24}\u{28}\u{2e}\u{31}",
            vec![(4, LineBreakKind::Mandatory)],
        ),
        (
            "\u{31}\u{2f}\u{2c}\u{29}\u{24}",
            vec![(5, LineBreakKind::Mandatory)],
        ),
        (
            "\u{25cc}\u{1b44}\u{1b05}",
            vec![(9, LineBreakKind::Mandatory)],
        ),
        (
            "\u{1b05}\u{1b05}\u{1bf2}",
            vec![(9, LineBreakKind::Mandatory)],
        ),
        ("\u{11003}\u{25cc}", vec![(7, LineBreakKind::Mandatory)]),
        (
            "\u{1f1e6}\u{1f1e6}\u{1f1e6}\u{1f1e6}\u{1f1e6}",
            vec![
                (8, LineBreakKind::Optional),
                (16, LineBreakKind::Optional),
                (20, LineBreakKind::Mandatory),
            ],
        ),
        (
            "\u{200b}\u{20}\u{41}",
            vec![(4, LineBreakKind::Optional), (5, LineBreakKind::Mandatory)],
        ),
        (
            "\u{d}\u{a}\u{41}",
            vec![(2, LineBreakKind::Mandatory), (3, LineBreakKind::Mandatory)],
        ),
        (
            "\u{20}\u{308}\u{41}",
            vec![(1, LineBreakKind::Optional), (4, LineBreakKind::Mandatory)],
        ),
    ] {
        let r = evaluate_line_boundaries(source, limits()).unwrap();
        let actual: Vec<_> = r
            .opportunities()
            .iter()
            .map(|v| (v.offset(), v.kind()))
            .collect();
        assert_eq!(actual, expected, "{source:?}");
    }
}
