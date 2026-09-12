use super::{Receipt, ScalarRecord, SourceRecord, UnitRecord, admitted, build, payload};
use crate as iced_css;
use std::{error::Error, mem::size_of};
include!("../../tests/support/source_capitalize/adapter.rs");
fn leaf(id: usize, text: &str, op: &str, tag: Option<&str>) -> InputNode {
    InputNode::Payload {
        id,
        text: text.into(),
        mode: "normal".into(),
        suppressed: false,
        runs: None,
        operation: op.into(),
        language: tag.map(str::to_owned),
    }
}
fn actual(nodes: &[InputNode], check: impl FnOnce(&SourceWhitespace<'_, '_, '_, '_, '_>)) {
    let (construction, css) = actual_tree(nodes);
    let h = host(construction, None, None);
    let ua = catalog();
    let g = graph(&css, ua.next_context());
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(h.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let whitespace =
        preprocess_source_whitespace(&SourceParticipants::new(&eligibility), whitespace_limits())
            .unwrap();
    check(&whitespace);
}
#[test]
fn same_invocation_boundary_peak_drops_and_q0_capacity() {
    let families = [
        vec![
            leaf(0, "i", "capitalize", Some("lt")),
            leaf(1, "\u{307}", "capitalize", Some("lt")),
        ],
        vec![
            leaf(0, "q", "none", None),
            leaf(1, "uestions", "capitalize", None),
        ],
        vec![leaf(0, "a b c d", "capitalize", None)],
        vec![leaf(0, "", "capitalize", Some("tr"))],
        vec![leaf(0, "a\t b\n", "capitalize", None)],
        vec![
            InputNode::Empty { id: 80 },
            InputNode::Block {
                id: 30,
                suppressed: false,
                children: (0..8)
                    .map(|i| {
                        leaf(
                            i,
                            "IΣ ",
                            if i < 4 { "lower" } else { "upper" },
                            Some(["en", "lt", "tr", "az"][i % 4]),
                        )
                    })
                    .chain([leaf(9, "i\u{307}", "capitalize", Some("lt"))])
                    .collect(),
            },
            InputNode::Block {
                id: 31,
                suppressed: false,
                children: vec![leaf(10, "x y", "capitalize", None)],
            },
        ],
    ];
    // Exercise every copied observation/helper path on a real nested owner.
    observe(0, &families[5]);
    for (case, nodes) in families.iter().enumerate() {
        actual(nodes, |whitespace| {
            let mut lim = casing_limits();
            lim.retained_bytes = nz(usize::MAX);
            lim.work = nz(usize::MAX);
            let mut receipts: Vec<Receipt> = Vec::new();
            let out = build(whitespace, lim, &mut |r| receipts.push(r)).unwrap();
            let b = out.sources.capacity() * size_of::<SourceRecord>()
                + out.scalars.capacity() * size_of::<ScalarRecord>()
                + out.units.capacity() * size_of::<UnitRecord>();
            assert_eq!(out.retained_bytes(), b);
            assert!(out.units.capacity() >= out.capitalize_scalar_count());
            let mut peak = 0;
            let mut selection = None;
            let mut previous = None;
            for receipt in &receipts {
                let total: usize = receipt.components.iter().sum();
                peak = peak.max(total);
                assert_eq!(receipt.live, total);
                assert_eq!(receipt.peak, peak);
                assert_eq!(receipt.remaining_bytes, usize::MAX - total);
                assert_eq!(admitted(0, total, total), Ok(total));
                if total > 0 {
                    assert_eq!(admitted(0, total, total - 1), Err(SourceCasingError::Bytes));
                }
                if let Some((bytes, work)) = receipt.forwarded {
                    let allocated = match receipt.phase {
                        "view" => receipt.components[0],
                        "normalized" => receipt.components[5],
                        "word_peak" => receipt.components[6] + receipt.components[7],
                        "graphemes" => receipt.components[8],
                        "contexts" => receipt.components[9],
                        "selection" => receipt.components[10],
                        _ => panic!("helper phase"),
                    };
                    assert_eq!(
                        bytes,
                        (usize::MAX - (total - allocated)).min(isize::MAX as usize)
                    );
                    assert!(work > 0);
                }
                match receipt.phase {
                    "word_peak" => {
                        assert!(receipt.components[6] > 0);
                        assert!(receipt.components[7] > 0);
                        assert_eq!(&receipt.components[8..], &[0; 3]);
                    }
                    "word_retained" => {
                        assert_eq!(previous, Some("word_peak"));
                        assert_eq!(receipt.components[7], 0);
                    }
                    "graphemes" | "boundary_join" => {
                        assert!(receipt.components[6] > 0 && receipt.components[8] > 0);
                        assert_eq!(receipt.components[7], 0);
                    }
                    "graphemes_drop" => assert_eq!(receipt.components[8], 0),
                    "words_drop" => assert_eq!(&receipt.components[6..9], &[0; 3]),
                    "contexts" => assert_eq!(&receipt.components[6..9], &[0; 3]),
                    "selection" => {
                        assert!(selection.is_none());
                        selection = receipt.lane;
                    }
                    "selection_drop" => {
                        assert_eq!(selection, receipt.lane);
                        selection = None;
                        assert_eq!(receipt.components[10], 0);
                    }
                    "contexts_drop" => assert_eq!(receipt.components[9], 0),
                    "normalized_drop" => assert_eq!(receipt.components[5], 0),
                    "string_drop" => assert_eq!(&receipt.components[4..], &[0; 7]),
                    _ => {}
                }
                previous = Some(receipt.phase);
            }
            assert!(selection.is_none());
            assert_eq!(peak, out.peak_retained_bytes());
            if case == 0 {
                assert_eq!(out.work(), 566);
                assert_eq!(out.units.len(), 1);
                assert_eq!(out.capitalize_scalar_count(), 2);
                let scalars = receipts.iter().find(|r| r.phase == "scalars").unwrap();
                let units = receipts.iter().find(|r| r.phase == "units").unwrap();
                let before_j: usize = scalars.components.iter().sum();
                let requested_j = payload::<UnitRecord>(out.capitalize_scalar_count()).unwrap();
                assert_eq!(
                    admitted(before_j, requested_j, before_j + requested_j),
                    Ok(before_j + requested_j)
                );
                assert_eq!(
                    admitted(before_j, requested_j, before_j + requested_j - 1),
                    Err(SourceCasingError::Bytes)
                );
                assert!(units.components[3] >= requested_j);
            }
            if case == 1 {
                assert_eq!(out.work(), 184);
                assert_eq!(out.units.len(), 0);
                assert!(out.units.capacity() >= 8);
                assert_eq!(out.active_context_count(), 1);
                assert_eq!(out.capitalize_context_count(), 1);
                assert!(
                    !receipts
                        .iter()
                        .any(|r| matches!(r.phase, "contexts" | "selection"))
                );
            }
            if case == 2 {
                assert_eq!(out.units.len(), out.capitalize_scalar_count());
            }
            if case == 3 {
                assert_eq!(out.units.capacity(), 0);
                assert_eq!(out.capitalize_context_count(), 0);
            }
            println!("capitalize resource case={case} receipts={receipts:?}");
        });
    }
}
#[test]
fn literal_work_stops_before_next_stage() {
    let nodes = [
        leaf(0, "i", "capitalize", Some("lt")),
        leaf(1, "\u{307}", "capitalize", Some("lt")),
    ];
    actual(&nodes, |whitespace| {
        for (budget, last) in [
            (513, "scalars"),
            (514, "units"),
            (543, "normalized"),
            (548, "word_retained"),
            (552, "graphemes"),
            (559, "words_drop"),
            (565, "selection"),
        ] {
            let mut lim = casing_limits();
            lim.work = nz(budget);
            let mut receipts = Vec::new();
            assert!(matches!(
                build(whitespace, lim, &mut |r| receipts.push(r)),
                Err(SourceCasingError::Work)
            ));
            assert_eq!(receipts.last().unwrap().phase, last);
        }
        let mut lim = casing_limits();
        lim.work = nz(566);
        let mut receipts = Vec::new();
        let out = build(whitespace, lim, &mut |r| receipts.push(r)).unwrap();
        assert_eq!(out.work(), 566);
        for (phase, used) in [
            ("scalars", 513),
            ("units", 514),
            ("string", 527),
            ("word_peak", 544),
            ("graphemes", 549),
            ("boundary_join", 553),
            ("contexts", 560),
            ("selection", 564),
            ("publish", 566),
        ] {
            let receipt = receipts.iter().find(|r| r.phase == phase).unwrap();
            assert_eq!(566 - receipt.remaining_work, used);
        }
    });
}
#[test]
fn synthetic_boundary_errors_and_invalid_offsets_preserve_index_and_source() {
    use super::{
        GraphemeBoundaryError as G, SourceCasingInvariantReason as I, WordBoundaryError as W,
    };
    for (targets, before, after) in [(1, 0, 0), (0, 0, 1), (0, 2, 1)] {
        assert_eq!(
            super::zero_lane_coverage(7, targets, before, after),
            Err(SourceCasingError::Invariant {
                start: 7,
                reason: I::TargetCoverage
            })
        );
    }
    assert_eq!(super::zero_lane_coverage(7, 0, 2, 2), Ok(()));
    for e in [W::InputBytes, W::Scalars, W::Work, W::Bytes, W::Allocation] {
        let wrapped = super::capitalize::word_error(7, e);
        assert_eq!(
            wrapped,
            SourceCasingError::WordBoundaries { start: 7, error: e }
        );
        assert_eq!(wrapped.source().unwrap().downcast_ref::<W>(), Some(&e));
    }
    for e in [G::InputBytes, G::Scalars, G::Work, G::Bytes, G::Allocation] {
        let wrapped = super::capitalize::grapheme_error(7, e);
        assert_eq!(
            wrapped,
            SourceCasingError::GraphemeBoundaries { start: 7, error: e }
        );
        assert_eq!(wrapped.source().unwrap().downcast_ref::<G>(), Some(&e));
    }
    assert_eq!(
        payload::<UnitRecord>(usize::MAX),
        Err(SourceCasingError::Bytes)
    );
    let ledger = super::Ledger {
        work: 1,
        allowance: 1,
        live: 1,
        peak: 1,
    };
    assert_eq!(ledger.forwarded(), Err(SourceCasingError::Bytes));
    actual(&[leaf(0, "éa", "capitalize", None)], |whitespace| {
        let mut short = evaluate_source_casing(whitespace, casing_limits()).unwrap();
        short.units.clear();
        assert_eq!(
            super::capitalize::join(&mut short, 0..2, "a", &[0, 1], &[0, 1]),
            Err(SourceCasingError::Invariant {
                start: 0,
                reason: I::Boundaries
            })
        );
        for offsets in [
            &[][..],
            &[0, 1, 3],
            &[0, 0, 3],
            &[0, 2, 2, 3],
            &[0, 2],
            &[1, 3],
        ] {
            let mut out = evaluate_source_casing(whitespace, casing_limits()).unwrap();
            out.units.clear();
            let error = SourceCasingError::Invariant {
                start: 0,
                reason: I::Boundaries,
            };
            assert_eq!(
                super::capitalize::join(&mut out, 0..2, "éa", offsets, &[0, 2, 3]),
                Err(error)
            );
            out.units.clear();
            assert_eq!(
                super::capitalize::join(&mut out, 0..2, "éa", &[0, 2, 3], offsets),
                Err(error)
            );
        }
    });
}
