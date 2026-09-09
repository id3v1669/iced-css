use super::{
    EventRecord, Ledger, LfConversionError, LfConversionLimits, NewlineError, Phase,
    PreservedSpaceError, PreservedSpaceLimits, Receipt, ScalarRecord, SpaceCollapseError,
    SpaceCollapseLimits, SpaceItem, SpaceUnit, admitted, amount, build, collapse_adjacent_spaces,
    collapse_error, convert_line_feeds, forwarded, lf_error, newline_error, payload,
    preserved_error, preserved_space_sequences,
};
use crate as iced_css;
use std::{error::Error, mem::size_of};
include!("../../tests/support/source_whitespace/adapter.rs");
fn leaf(id: usize, text: String) -> InputNode {
    InputNode::Payload {
        id,
        text,
        mode: "normal".into(),
        suppressed: false,
        runs: None,
    }
}
fn phase_payload(r: &Receipt) -> usize {
    let c = r.components;
    c.event_capacity * size_of::<EventRecord>()
        + c.scalar_capacity * size_of::<ScalarRecord>()
        + c.item_capacity * size_of::<SpaceItem>()
        + c.preserved_capacity * size_of::<PreservedSpace>()
        + c.action_capacity * size_of::<SpaceAction>()
        + c.normalized_bytes
        + if r.phase == Phase::LfPeak {
            c.lf_peak_bytes
        } else {
            c.lf_retained_bytes
        }
}
#[test]
fn same_invocation_components_reconstruct_every_peak_and_drop_phase() {
    let fixtures = vec![
        (vec![leaf(0, "".into())], false),
        (vec![leaf(0, "\r\n".into())], false),
        (vec![leaf(0, "🙂".into())], false),
        (
            (0..12)
                .flat_map(|i| [InputNode::Empty { id: i }, leaf(i, "".into())])
                .collect(),
            false,
        ),
        (vec![leaf(0, " 漢\r\n漢 \t\n ".repeat(100))], false),
        ((0..20).map(|i| leaf(i, " \n漢 ".into())).collect(), false),
        (
            (0..8)
                .map(|i| InputNode::Block {
                    id: i,
                    suppressed: false,
                    children: vec![leaf(i, "a \t b".into())],
                })
                .collect(),
            false,
        ),
        (
            vec![InputNode::Payload {
                id: 0,
                text: "A\r\nB".into(),
                mode: "pre-wrap".into(),
                suppressed: false,
                runs: Some(vec![0..0, 0..2, 2..4]),
            }],
            false,
        ),
        (vec![leaf(0, "ignored".into())], true),
    ];
    for (case, (nodes, suppressed)) in fixtures.into_iter().enumerate() {
        let (construction, mut css) = actual_tree(&nodes);
        if suppressed {
            css.push_str("#scope {display:none;}");
        }
        let h = host(construction, None, None);
        let ua = catalog();
        let g = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
        let b = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let d = evaluate_source_display(h.snapshot().unwrap(), &b, limits()).unwrap();
        let e = evaluate_source_eligibility(&d, eligibility_limits()).unwrap();
        let p = SourceParticipants::new(&e);
        let mut receipts = Vec::new();
        let mut selected_limits = whitespace_limits();
        if case == 2 {
            selected_limits.retained_bytes = NonZeroUsize::MAX;
        }
        let out = build(&p, selected_limits, &mut |r| receipts.push(r)).unwrap();
        let mut peak = 0;
        let mut pending_payload = false;
        let mut pending_actions = false;
        let mut seen_sources = Vec::new();
        let mut seen_segments = Vec::new();
        for receipt in &receipts {
            let live = phase_payload(receipt);
            peak = peak.max(live);
            assert_eq!(receipt.reported_live, live);
            assert_eq!(receipt.reported_peak, peak);
            // Same production admission helper, exact measured components from this
            // invocation; this is not a second allocator invocation at old capacity.
            assert_eq!(admitted(&[live], live), Ok(live));
            if live > 0 {
                assert_eq!(
                    admitted(&[live], live - 1),
                    Err(SourceWhitespaceError::Bytes)
                );
            }
            if let Some(bytes) = receipt.allowances.forwarded_bytes {
                assert!(bytes <= isize::MAX as usize);
                assert!(
                    bytes <= receipt.allowances.true_bytes
                        || bytes == 1 && receipt.allowances.true_bytes == 0
                );
            }
            if let Some(work) = receipt.allowances.forwarded_work {
                assert_eq!(work, receipt.allowances.required_work.max(1));
                assert!(receipt.allowances.required_work <= receipt.allowances.true_work);
            }
            match receipt.phase {
                Phase::Normalized => {
                    assert!(!pending_payload);
                    pending_payload = true;
                    seen_sources.push(receipt.allowances.source.unwrap());
                }
                Phase::PayloadDropped => {
                    assert!(pending_payload);
                    pending_payload = false;
                    assert_eq!(receipt.components.normalized_bytes, 0);
                    assert_eq!(receipt.components.lf_retained_bytes, 0);
                    assert_eq!(receipt.components.preserved_capacity, 0);
                }
                Phase::Collapse => {
                    assert!(!pending_payload && !pending_actions);
                    pending_actions = true;
                    seen_segments.push(receipt.allowances.segment.unwrap());
                }
                Phase::ActionsDropped => {
                    assert!(pending_actions);
                    pending_actions = false;
                    assert_eq!(receipt.components.action_capacity, 0);
                }
                Phase::ItemsDropped => {
                    assert!(!pending_actions);
                    assert_eq!(receipt.components.item_capacity, 0);
                }
                _ => {}
            }
            println!("resource-case={case} {receipt:?}");
        }
        assert!(!pending_payload && !pending_actions);
        assert_eq!(seen_sources.len(), out.content_count());
        if case == 2 {
            assert!(
                receipts
                    .iter()
                    .filter_map(|r| r.allowances.forwarded_bytes)
                    .all(|v| v == isize::MAX as usize)
            );
        }
        assert_eq!(
            seen_segments.iter().map(|(a, b)| b - a).sum::<usize>(),
            out.scalar_count()
        );
        assert_eq!(
            out.retained_bytes(),
            out.events.capacity() * size_of::<EventRecord>()
                + out.scalars.capacity() * size_of::<ScalarRecord>()
        );
        assert_eq!(out.peak_retained_bytes(), peak);
        assert_eq!(receipts.last().unwrap().phase, Phase::Published);
        assert_eq!(
            out.work(),
            2 + out.source_count()
                + 4 * out.event_count()
                + out.input_scalar_count()
                + 2 * out.input_bytes()
                + out.content_count()
                + 12 * out.scalar_count()
        );
        assert!(out.scalars.capacity() >= out.input_scalar_count());
        if case == 1 {
            assert_eq!((out.input_scalar_count(), out.scalar_count()), (2, 1));
        }
        if !suppressed && out.content_count() > 0 {
            for phase in [
                Phase::Normalized,
                Phase::LfPeak,
                Phase::LfScratchDropped,
                Phase::Preserved,
                Phase::PayloadDropped,
            ] {
                assert!(receipts.iter().any(|r| r.phase == phase));
            }
        }
        if suppressed {
            assert_eq!(
                (out.work(), out.retained_bytes(), out.peak_retained_bytes()),
                (2, 0, 0)
            );
        }
    }
    observe(99, &[leaf(0, " ".into())]);
    let _ = wrapped("resource-helper");
    let _ = named("resource-named");
    let _ = text();
    assert_eq!(row(vec![]).nodes().len(), 1);
}
#[test]
fn actual_empty_helpers_and_synthetic_zero_headroom_addressability_use_production_admission() {
    use iced_css_core::text::newlines::*;
    let zero = forwarded(0, 0).unwrap();
    assert_eq!(zero.get(), 1);
    assert_eq!(forwarded(0, 1), Err(SourceWhitespaceError::Bytes));
    let mut l = Ledger::new(whitespace_limits());
    l.work_limit = 0;
    assert_eq!(l.debit(0).unwrap().get(), 1);
    assert_eq!(l.work, 0);
    assert_eq!(l.debit(1), Err(SourceWhitespaceError::Work));
    let text = normalize_default_newlines(
        "",
        NewlineLimits {
            input_bytes: zero,
            retained_bytes: zero,
            work: zero,
        },
    )
    .unwrap();
    assert_eq!(admitted(&[text.retained_bytes()], 0), Ok(0));
    let lf = convert_line_feeds(
        &text,
        WhiteSpace::Normal,
        LfConversionLimits {
            scalars: zero,
            retained_bytes: zero,
            work: zero,
        },
    )
    .unwrap();
    assert_eq!(
        admitted(&[lf.retained_bytes(), lf.peak_retained_bytes()], 0),
        Ok(0)
    );
    assert_eq!(lf.work(), 1);
    let preserved = preserved_space_sequences(
        &text,
        WhiteSpace::Normal,
        PreservedSpaceLimits {
            scalars: zero,
            retained_bytes: zero,
            work: zero,
        },
    )
    .unwrap();
    assert_eq!(
        admitted(
            &[payload::<PreservedSpace>(preserved.capacity()).unwrap()],
            0
        ),
        Ok(0)
    );
    let actions = collapse_adjacent_spaces(
        &[],
        SpaceCollapseLimits {
            items: zero,
            retained_bytes: zero,
            work: zero,
        },
    )
    .unwrap();
    assert_eq!(
        admitted(&[payload::<SpaceAction>(actions.capacity()).unwrap()], 0),
        Ok(0)
    );
    assert_eq!(forwarded(usize::MAX, 1).unwrap().get(), isize::MAX as usize);
    assert_eq!(
        forwarded(usize::MAX, isize::MAX as usize + 1),
        Err(SourceWhitespaceError::Bytes)
    );
    assert_eq!(
        admitted(&[isize::MAX as usize, 1], usize::MAX),
        Ok(isize::MAX as usize + 1)
    );
    assert_eq!(
        admitted(&[usize::MAX, 1], usize::MAX),
        Err(SourceWhitespaceError::Bytes)
    );
    assert_eq!(
        payload::<ScalarRecord>(usize::MAX),
        Err(SourceWhitespaceError::Bytes)
    );
    assert_eq!(amount(usize::MAX, 12, 1), Err(SourceWhitespaceError::Work));
}
#[test]
fn synthetic_nested_errors_use_exact_context_translators_without_forged_owners() {
    use iced_css_core::text::lf_whitespace::LfWhitespaceError;
    for error in [
        NewlineError::InputBytes,
        NewlineError::Bytes,
        NewlineError::Work,
        NewlineError::Allocation,
    ] {
        assert_eq!(
            newline_error(7, error),
            SourceWhitespaceError::Newlines { source: 7, error }
        );
    }
    for error in [
        LfConversionError::Scalars,
        LfConversionError::Bytes,
        LfConversionError::Work,
        LfConversionError::Allocation,
        LfConversionError::Step1(LfWhitespaceError::Allocation),
    ] {
        assert_eq!(
            lf_error(7, error),
            SourceWhitespaceError::Lf { source: 7, error }
        );
    }
    for step in [
        LfWhitespaceError::Scalars,
        LfWhitespaceError::Bytes,
        LfWhitespaceError::Work,
        LfWhitespaceError::Allocation,
    ] {
        let nested = LfConversionError::Step1(step);
        let outer = lf_error(7, nested);
        assert_eq!(
            outer,
            SourceWhitespaceError::Lf {
                source: 7,
                error: nested
            }
        );
        assert_eq!(outer.source().unwrap().to_string(), nested.to_string());
        assert_eq!(
            outer.source().unwrap().source().unwrap().to_string(),
            step.to_string()
        );
    }
    for error in [
        PreservedSpaceError::Scalars,
        PreservedSpaceError::Bytes,
        PreservedSpaceError::Work,
        PreservedSpaceError::Allocation,
    ] {
        assert_eq!(
            preserved_error(7, error),
            SourceWhitespaceError::Preserved { source: 7, error }
        );
    }
    for error in [
        SpaceCollapseError::Items,
        SpaceCollapseError::Bytes,
        SpaceCollapseError::Work,
        SpaceCollapseError::Allocation,
        SpaceCollapseError::StageOrder { item: 2 },
    ] {
        assert_eq!(
            collapse_error(9, error),
            SourceWhitespaceError::Collapse { start: 9, error }
        );
    }
    let malformed = [SpaceItem {
        unit: SpaceUnit::Scalar('\r'),
        white_space: WhiteSpace::Normal,
    }];
    let error = collapse_adjacent_spaces(
        &malformed,
        SpaceCollapseLimits {
            items: bound(),
            retained_bytes: bound(),
            work: bound(),
        },
    )
    .unwrap_err();
    assert!(matches!(
        collapse_error(9, error),
        SourceWhitespaceError::Collapse {
            start: 9,
            error: SpaceCollapseError::StageOrder { item: 0 }
        }
    ));
}
