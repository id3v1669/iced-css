use super::{
    Ledger, ScalarRecord, SourceRecord, admitted, build, context_error, cost, invariant,
    newline_error, operation, payload, selection_error,
};
use crate as iced_css;
use std::{error::Error, mem::size_of};
include!("../../tests/support/source_casing/adapter.rs");
fn leaf(id: usize, text: &str, op: &str, tag: Option<&str>) -> InputNode {
    InputNode::Payload {
        id,
        text: text.to_owned(),
        mode: "normal".to_owned(),
        suppressed: false,
        runs: None,
        operation: op.to_owned(),
        language: tag.map(str::to_owned),
    }
}
#[test]
fn raw_empty_language_is_malformed_not_public_unknown() {
    assert_eq!(
        super::language::classify(""),
        (
            SourceCasingLanguageStatus::Malformed,
            SourceCasingProfile::Default
        )
    );
}
#[test]
fn actual_components_and_drops_bound_every_phase() {
    let eight = || {
        (0..8)
            .map(|i| {
                leaf(
                    i,
                    "IΣ\u{307}",
                    if i < 4 { "lower" } else { "upper" },
                    Some(["en", "lt", "tr", "az"][i % 4]),
                )
            })
            .collect::<Vec<_>>()
    };
    let fixtures = vec![
        (vec![leaf(0, "", "none", None)], false, 0, 0),
        (vec![leaf(0, "IΣ", "none", Some("tr"))], false, 0, 0),
        (
            vec![leaf(0, "A", "none", None), leaf(1, "Σ", "lower", None)],
            false,
            1,
            1,
        ),
        (eight(), false, 1, 8),
        (
            vec![
                InputNode::Block {
                    id: 0,
                    suppressed: false,
                    children: eight(),
                },
                InputNode::Block {
                    id: 1,
                    suppressed: false,
                    children: vec![leaf(8, "é\r\n漢", "lower", Some("tr"))],
                },
            ],
            false,
            2,
            9,
        ),
        (
            (0..12).map(|i| InputNode::Empty { id: i }).collect(),
            false,
            0,
            0,
        ),
        (
            vec![leaf(0, &"IΣ\u{307}".repeat(200), "upper", Some("lt"))],
            false,
            1,
            1,
        ),
        (vec![leaf(0, "ignored", "lower", None)], true, 0, 0),
    ];
    for (nodes, hidden, active, lanes) in fixtures {
        if lanes == 8 {
            observe(0, &nodes);
        }
        let (construction, mut css) = actual_tree(&nodes);
        if hidden {
            css.push_str("#scope{display:none;}");
        }
        let h = host(construction, None, None);
        let ua = catalog();
        let g = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(h.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let whitespace = preprocess_source_whitespace(
            &SourceParticipants::new(&eligibility),
            whitespace_limits(),
        )
        .unwrap();
        let mut receipts = Vec::new();
        let mut lim = casing_limits();
        lim.retained_bytes = nz(usize::MAX);
        lim.work = nz(usize::MAX);
        let out = build(&whitespace, lim, &mut |r| receipts.push(r)).unwrap();
        assert_eq!(out.active_context_count(), active);
        assert_eq!(out.selection_pass_count(), lanes);
        let mut peak = 0;
        let mut previous_work = usize::MAX;
        let mut selected = None;
        let mut context_start = None;
        for receipt in &receipts {
            for index in [3, 6, 7, 8] {
                assert_eq!(receipt.components[index], 0);
            }
            let total: usize = receipt.components.iter().sum();
            assert_eq!(receipt.live, total);
            peak = peak.max(total);
            assert_eq!(receipt.peak, peak);
            assert_eq!(receipt.remaining_bytes, usize::MAX - total);
            assert!(receipt.remaining_work <= previous_work);
            previous_work = receipt.remaining_work;
            assert_eq!(admitted(0, total, total), Ok(total));
            if total > 0 {
                assert_eq!(admitted(0, total, total - 1), Err(SourceCasingError::Bytes));
            }
            if let Some((bytes, forwarded_work)) = receipt.forwarded {
                let component = match receipt.phase {
                    "view" => 0,
                    "normalized" => 5,
                    "contexts" => 9,
                    "selection" => 10,
                    _ => panic!("unexpected helper"),
                };
                let before = total - receipt.components[component];
                assert_eq!(bytes, (usize::MAX - before).min(isize::MAX as usize));
                if receipt.phase != "view" {
                    let end=whitespace.events().filter(|event|event.kind()!=SourceStructuralEventKind::Content && event.participant().outer_category()==crate::evaluation::source_participants::SourceOuterCategory::Block).map(|event|event.scalar_range().start).find(|&end|end>receipt.start).unwrap_or(whitespace.scalar_count());
                    let m = end - receipt.start;
                    let u: usize = (receipt.start..end)
                        .map(|i| whitespace.scalar(i).unwrap().normalized().len_utf8())
                        .sum();
                    let expected = match receipt.phase {
                        "normalized" => 2 * u,
                        "contexts" => 1 + 3 * m,
                        "selection" => 1 + m,
                        _ => unreachable!(),
                    };
                    assert_eq!(forwarded_work, expected);
                }
            }
            match receipt.phase {
                "view" => assert_eq!(receipt.forwarded.unwrap().1, usize::MAX - 1),
                "profiles" => {
                    assert!(receipt.components[0] > 0);
                    assert!(
                        receipt.components[1] >= out.source_count() * size_of::<SourceRecord>()
                    );
                }
                "view_drop" => assert_eq!(receipt.components[0], 0),
                "scalars" | "units" => assert_eq!(receipt.components[0], 0),
                "string" => {
                    assert!(context_start.is_none());
                    context_start = Some(receipt.start);
                    assert_eq!(receipt.components[0], 0);
                }
                "selection" => {
                    assert!(selected.is_none());
                    assert_eq!(context_start, Some(receipt.start));
                    selected = receipt.lane;
                }
                "selection_drop" => {
                    assert_eq!(selected, receipt.lane);
                    assert_eq!(receipt.components[10], 0);
                    selected = None;
                }
                "contexts_drop" => assert_eq!(receipt.components[9], 0),
                "normalized_drop" => assert_eq!(receipt.components[5], 0),
                "string_drop" => {
                    assert!(selected.is_none());
                    assert_eq!(&receipt.components[4..], &[0; 7]);
                    context_start = None;
                }
                "publish" => {
                    assert_eq!(receipt.live, out.retained_bytes());
                    assert_eq!(receipt.remaining_work, usize::MAX - out.work());
                }
                "normalized" | "contexts" => {}
                _ => panic!("unknown observed phase"),
            }
        }
        assert!(selected.is_none() && context_start.is_none());
        assert_eq!(peak, out.peak_retained_bytes());
        assert_eq!(
            out.retained_bytes(),
            out.sources.capacity() * size_of::<SourceRecord>()
                + out.scalars.capacity() * size_of::<ScalarRecord>()
        );
        assert_eq!(
            receipts.iter().filter(|r| r.phase == "selection").count(),
            lanes
        );
        assert_eq!(
            receipts.iter().filter(|r| r.phase == "string").count(),
            active
        );
        if hidden {
            assert_eq!(out.work(), 1);
            assert_eq!(out.retained_bytes(), 0);
            assert_eq!(receipts.len(), 1);
        } else {
            let mut tiny = casing_limits();
            tiny.retained_bytes = nz(1);
            assert!(evaluate_source_casing(&whitespace, tiny).is_err());
        }
        println!(
            "actual phases: active={active} lanes={lanes} retained={} peak={} work={} receipts={receipts:?}",
            out.retained_bytes(),
            out.peak_retained_bytes(),
            out.work()
        );
    }
}
#[test]
fn synthetic_admission_and_nested_error_boundaries() {
    assert_eq!(payload::<u8>(usize::MAX), Err(SourceCasingError::Bytes));
    assert_eq!(
        payload::<ScalarRecord>(usize::MAX),
        Err(SourceCasingError::Bytes)
    );
    assert_eq!(
        admitted(usize::MAX, 1, usize::MAX),
        Err(SourceCasingError::Bytes)
    );
    assert_eq!(
        admitted(isize::MAX as usize, 1, usize::MAX),
        Ok(isize::MAX as usize + 1)
    );
    assert_eq!(cost(usize::MAX, 2, 1), Err(SourceCasingError::Work));
    let mut ledger = Ledger {
        work: 1,
        allowance: usize::MAX,
        live: 0,
        peak: 0,
    };
    assert_eq!(ledger.forwarded().unwrap().get(), isize::MAX as usize);
    ledger.live = usize::MAX;
    assert_eq!(ledger.forwarded(), Err(SourceCasingError::Bytes));
    ledger.charge(1).unwrap();
    assert_eq!(ledger.charge(1), Err(SourceCasingError::Work));
    for e in [
        super::NewlineError::InputBytes,
        super::NewlineError::Bytes,
        super::NewlineError::Work,
        super::NewlineError::Allocation,
    ] {
        let wrapped = newline_error(7, e);
        assert_eq!(wrapped, SourceCasingError::Newlines { start: 7, error: e });
        assert_eq!(
            wrapped
                .source()
                .unwrap()
                .downcast_ref::<super::NewlineError>(),
            Some(&e)
        );
    }
    for e in [
        super::CasingContextError::Scalars,
        super::CasingContextError::Bytes,
        super::CasingContextError::Work,
        super::CasingContextError::Allocation,
    ] {
        let wrapped = context_error(7, e);
        assert_eq!(wrapped, SourceCasingError::Contexts { start: 7, error: e });
        assert_eq!(
            wrapped
                .source()
                .unwrap()
                .downcast_ref::<super::CasingContextError>(),
            Some(&e)
        );
    }
    for e in [
        super::CasingSelectionError::Scalars,
        super::CasingSelectionError::Bytes,
        super::CasingSelectionError::Work,
        super::CasingSelectionError::Allocation,
        super::CasingSelectionError::Ambiguous { index: 3 },
    ] {
        let wrapped = selection_error(
            7,
            SourceCasingOperation::Upper,
            SourceCasingProfile::Turkish,
            e,
        );
        assert_eq!(
            wrapped,
            SourceCasingError::Selection {
                start: 7,
                operation: SourceCasingOperation::Upper,
                profile: SourceCasingProfile::Turkish,
                error: e
            }
        );
        assert_eq!(
            wrapped
                .source()
                .unwrap()
                .downcast_ref::<super::CasingSelectionError>(),
            Some(&e)
        );
    }
    for e in [
        super::ConstructionSourceError::StateCount,
        super::ConstructionSourceError::EmptyDocumentLanguage,
        super::ConstructionSourceError::DuplicateCssId,
        super::ConstructionSourceError::Entries,
        super::ConstructionSourceError::Bytes,
        super::ConstructionSourceError::Work,
        super::ConstructionSourceError::Allocation,
    ] {
        assert_eq!(
            SourceCasingError::LanguageView(e)
                .source()
                .unwrap()
                .downcast_ref::<super::ConstructionSourceError>(),
            Some(&e)
        );
    }
    for reason in [
        super::SourceCasingInvariantReason::Source,
        super::SourceCasingInvariantReason::OriginalCr,
        super::SourceCasingInvariantReason::Alignment,
        super::SourceCasingInvariantReason::TargetCoverage,
    ] {
        assert_eq!(
            invariant(7, reason),
            SourceCasingError::Invariant { start: 7, reason }
        );
    }
    assert!(matches!(
        operation("not-a-keyword", 4),
        Err(SourceCasingError::Transform {
            source: 4,
            reason: super::SourceCasingTransformReason::InvalidKeyword
        })
    ));
}
