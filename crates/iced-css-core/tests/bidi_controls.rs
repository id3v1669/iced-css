use iced_css_core::text::{
    line_edges::*,
    newlines::*,
    unicode::{UNICODE_VERSION, is_bidi_control},
};
use std::num::NonZeroUsize;
#[test]
fn unicode17_members_and_nearby_invisible_nonmembers_are_distinct() {
    assert_eq!(UNICODE_VERSION, (17, 0, 0));
    for c in [
        '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
        '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ] {
        assert!(is_bidi_control(c));
        assert_eq!(LineEdgeKind::from_scalar(c), LineEdgeKind::BidiFormatting);
    }
    for c in [
        'a',
        ' ',
        '\r',
        '\n',
        '\u{61b}',
        '\u{61d}',
        '\u{200b}',
        '\u{200c}',
        '\u{200d}',
        '\u{2010}',
        '\u{2029}',
        '\u{202f}',
        '\u{2060}',
        '\u{2065}',
        '\u{206a}',
        '\u{feff}',
        '\u{10ffff}',
    ] {
        assert!(!is_bidi_control(c));
        assert_eq!(LineEdgeKind::from_scalar(c), LineEdgeKind::Scalar(c));
    }
}
#[test]
fn actual_source_scalars_keep_original_mapping_through_edge_removal() {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    let source = "\u{2066} x \u{2069}";
    let normalized = normalize_default_newlines(
        source,
        NewlineLimits {
            input_bytes: nz(100),
            retained_bytes: nz(1000),
            work: nz(200),
        },
    )
    .unwrap();
    let items: Vec<_> = normalized
        .text()
        .chars()
        .map(|c| LineEdgeItem {
            kind: LineEdgeKind::from_scalar(c),
            white_space: WhiteSpace::Normal,
        })
        .collect();
    let result = trim_line_edges(
        &items,
        LineEdgeLimits {
            items: nz(100),
            retained_bytes: nz(1000),
            work: nz(300),
        },
    )
    .unwrap();
    assert_eq!(
        result,
        [
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedLeading,
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedTrailing,
            EdgeDisposition::Keep
        ]
    );
    let kept: Vec<_> = normalized
        .scalars()
        .iter()
        .zip(&result)
        .filter(|(_, d)| **d == EdgeDisposition::Keep)
        .map(|(span, _)| &source[span.source.clone()])
        .collect();
    assert_eq!(kept, ["\u{2066}", "x", "\u{2069}"]);
}
