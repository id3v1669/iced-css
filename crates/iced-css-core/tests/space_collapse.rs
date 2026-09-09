use iced_css_core::text::{line_edges::WhiteSpace, space_collapse::*};
use std::num::NonZeroUsize;
fn limits() -> SpaceCollapseLimits {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    SpaceCollapseLimits {
        items: nz(100),
        retained_bytes: nz(1000),
        work: nz(100),
    }
}
fn item(unit: SpaceUnit, white_space: WhiteSpace) -> SpaceItem {
    SpaceItem { unit, white_space }
}
fn scalar(c: char, mode: WhiteSpace) -> SpaceItem {
    item(SpaceUnit::Scalar(c), mode)
}
#[test]
fn tabs_and_spaces_collapse_across_all_compatible_modes() {
    for left in [WhiteSpace::Normal, WhiteSpace::Nowrap, WhiteSpace::PreLine] {
        for right in [WhiteSpace::Normal, WhiteSpace::Nowrap, WhiteSpace::PreLine] {
            let items = [
                scalar('\t', left),
                scalar(' ', right),
                scalar('\t', right),
                scalar('x', right),
                scalar(' ', left),
            ];
            assert_eq!(
                collapse_adjacent_spaces(&items, limits()).unwrap(),
                [
                    SpaceAction::TabToSpace,
                    SpaceAction::RemovedAdjacentSpace,
                    SpaceAction::RemovedAdjacentSpace,
                    SpaceAction::Unchanged,
                    SpaceAction::Unchanged
                ]
            );
        }
    }
}
#[test]
fn controls_and_previous_removals_do_not_break_adjacency_or_lose_source_entries() {
    let n = WhiteSpace::Normal;
    let items = [
        scalar(' ', n),
        scalar('\u{202b}', n),
        item(SpaceUnit::Removed, WhiteSpace::Pre),
        scalar('\t', n),
        scalar('\u{202c}', n),
    ];
    assert_eq!(
        collapse_adjacent_spaces(&items, limits()).unwrap(),
        [
            SpaceAction::Unchanged,
            SpaceAction::Unchanged,
            SpaceAction::Unchanged,
            SpaceAction::RemovedAdjacentSpace,
            SpaceAction::Unchanged
        ]
    );
}
#[test]
fn preserved_items_atomics_and_other_scalars_stop_collapse() {
    let n = WhiteSpace::Normal;
    for middle in [
        scalar(' ', WhiteSpace::Pre),
        scalar(' ', WhiteSpace::PreWrap),
        scalar('\t', WhiteSpace::PreWrap),
        scalar('\n', WhiteSpace::PreLine),
        scalar('\u{a0}', n),
        scalar('\u{200b}', n),
        item(SpaceUnit::Atomic, n),
    ] {
        assert_eq!(
            collapse_adjacent_spaces(&[scalar(' ', n), middle, scalar(' ', n)], limits()).unwrap(),
            [SpaceAction::Unchanged; 3]
        );
    }
    for mode in [WhiteSpace::Pre, WhiteSpace::PreWrap] {
        assert_eq!(
            collapse_adjacent_spaces(
                &[
                    scalar(' ', mode),
                    scalar(' ', mode),
                    scalar('\t', mode),
                    scalar('\n', mode)
                ],
                limits()
            )
            .unwrap(),
            [SpaceAction::Unchanged; 4]
        );
    }
}
#[test]
fn unapplied_earlier_stages_and_limits_fail_without_partial_output() {
    let n = WhiteSpace::Normal;
    for (c, mode) in [
        ('\r', n),
        ('\r', WhiteSpace::Pre),
        ('\n', n),
        ('\n', WhiteSpace::Nowrap),
    ] {
        assert!(matches!(
            collapse_adjacent_spaces(&[scalar('a', n), scalar(c, mode)], limits()),
            Err(SpaceCollapseError::StageOrder { item: 1 })
        ));
    }
    let items = [scalar(' ', n), scalar(' ', n)];
    let mut cap = limits();
    cap.items = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        collapse_adjacent_spaces(&items, cap),
        Err(SpaceCollapseError::Items)
    ));
    cap = limits();
    cap.work = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        collapse_adjacent_spaces(&items, cap),
        Err(SpaceCollapseError::Work)
    ));
    cap = limits();
    cap.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        collapse_adjacent_spaces(&items, cap),
        Err(SpaceCollapseError::Bytes)
    ));
    assert!(collapse_adjacent_spaces(&[], limits()).unwrap().is_empty());
}
