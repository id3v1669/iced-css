#[path = "support/clearance_fixture.rs"]
mod support;
use std::{error::Error, mem::size_of};
use support::*;

fn limits() -> SiblingBoxFactsLimits {
    SiblingBoxFactsLimits {
        sources: bound(),
        retained_bytes: nz(20_000_000),
        evaluation: fact_limits(),
    }
}

fn px(value: BoxLengthFact) -> f32 {
    let BoxLengthFact::Absolute(value) = value else {
        panic!("expected absolute length")
    };
    value.pixels
}

#[test]
fn real_six_and_seven_source_css_produces_every_dynamic_row_and_target() {
    let css = "
        #o { width: 100px; }
        #p { height: 7px; margin-top: 2px; overflow: hidden; }
        #b1 { height: 1px; margin-bottom: 4px; }
        #f { width: 10px; height: 2px; }
        #b2 { height: 1px; margin-top: 3px; clear: both; }
        #b3 { height: inherit; margin-right: auto; }
        #b3:before { content: inherit; }
        #b3:after { content: none; }
        #b4 { height: 2px; margin-top: inherit; overflow: inherit; }
    ";
    for source_count in [6, 7] {
        sibling_fixture(source_count, css, |projection, display| {
            let facts = evaluate_sibling_box_facts(projection, display, limits()).unwrap();
            assert!(std::ptr::eq(facts.projection(), projection));
            assert!(std::ptr::eq(facts.display(), display));
            assert_eq!(facts.receipt().evaluation.targets, 3 * source_count);
            assert_eq!(
                facts.receipt().evaluation.numeric_work,
                14 * source_count * 100_000
            );
            assert_eq!(
                facts.receipt().evaluation.numeric_calls,
                if source_count == 6 { 9 } else { 10 }
            );
            assert_eq!(
                facts.receipt().evaluation.owned_heap_bytes,
                facts.receipt().row_capacity * size_of::<ClearanceBoxFact>()
            );
            assert!(facts.receipt().row_capacity >= source_count);
            assert!(facts.row(source_count).is_none());

            for projected in 0..source_count {
                let original = projection.original_index(projected).unwrap();
                assert!(std::ptr::eq(
                    projection.token(projected).unwrap(),
                    display.token(original).unwrap(),
                ));
                assert!(display.resolution(original).is_some());
                assert!(facts.row(projected).is_some());
            }

            let b3 = facts.row(5).unwrap();
            assert_eq!(px(b3.height), 7.0);
            assert_eq!(
                b3.height,
                BoxLengthFact::Absolute(AbsoluteBoxFact {
                    pixels: 7.0,
                    rounded_to_zero: false,
                    origin: BoxFactOrigin::Inherited {
                        original_parent: source_count - 2,
                    },
                })
            );
            assert_eq!(
                b3.margin[1],
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Specified,
                }
            );
            assert_eq!(
                b3.before.origin,
                BoxFactOrigin::Inherited { original_parent: 3 }
            );
            assert_eq!(b3.after.origin, BoxFactOrigin::Specified);
            assert_eq!(b3.clear.origin, BoxFactOrigin::Initial);

            if source_count == 7 {
                let b4 = facts.row(6).unwrap();
                assert_eq!(px(b4.height), 2.0);
                assert_eq!(px(b4.margin[0]), 2.0);
                assert_eq!(
                    b4.margin[0],
                    BoxLengthFact::Absolute(AbsoluteBoxFact {
                        pixels: 2.0,
                        rounded_to_zero: false,
                        origin: BoxFactOrigin::Inherited { original_parent: 5 },
                    })
                );
                assert_eq!(b4.overflow.value, BoxOverflow::Hidden);
                assert_eq!(b4.before.origin, BoxFactOrigin::Initial);
                assert_eq!(b4.after.origin, BoxFactOrigin::Initial);
            }
        });
    }
}

#[test]
fn dynamic_five_source_rows_match_the_legacy_entry_and_keep_legacy_receipts() {
    let css = "
        #o { width: 100px; }
        #p { height: 7px; margin: 2px; overflow: hidden; }
        #b1 { height: inherit; padding-top: 1px; }
        #f { width: 10px; height: 2px; }
        #b2 { height: 1px; margin-top: 3px; clear: both; }
        #b2:before { content: none; }
    ";
    sibling_fixture(5, css, |projection, display| {
        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let legacy =
            evaluate_clearance_box_facts(&legacy_projection, display, fact_limits()).unwrap();
        let dynamic = evaluate_sibling_box_facts(projection, display, limits()).unwrap();

        for projected in 0..5 {
            assert_eq!(dynamic.row(projected), legacy.row(projected));
        }
        assert!(dynamic.row(5).is_none());
        assert!(legacy.row(5).is_none());

        let mut expected_evaluation = legacy.receipt();
        assert_eq!(
            expected_evaluation.work,
            137 + display.bundle().rule_count()
        );
        assert_eq!(expected_evaluation.targets, 15);
        assert_eq!(expected_evaluation.owned_heap_bytes, 0);
        expected_evaluation.owned_heap_bytes = dynamic.receipt().evaluation.owned_heap_bytes;
        assert_eq!(dynamic.receipt().evaluation, expected_evaluation);
    });
}

#[test]
fn exact_dynamic_receipts_and_each_aggregate_limit_are_enforced() {
    sibling_fixture(7, "#b4 { height: 2px; }", |projection, display| {
        let result = evaluate_sibling_box_facts(projection, display, limits()).unwrap();
        let receipt = result.receipt();
        let evaluation = receipt.evaluation;
        assert_eq!(evaluation.work, 27 * 7 + 2 + display.bundle().rule_count());
        assert_eq!(evaluation.targets, 21);
        assert_eq!(evaluation.rule_visits, 21 * display.bundle().rule_count());
        assert_eq!(
            evaluation.declaration_offers,
            21 * display.bundle().declaration_count()
        );
        assert_eq!(
            evaluation.membership_work,
            21 * display.bundle().author_rule_count() * 100
        );

        let mut exact = limits();
        exact.evaluation.work = nz(evaluation.work);
        exact.evaluation.rule_visits = nz(evaluation.rule_visits);
        exact.evaluation.matching_work = nz(evaluation.matching_work);
        exact.evaluation.declaration_offers = nz(evaluation.declaration_offers);
        exact.evaluation.membership_work = nz(evaluation.membership_work);
        exact.evaluation.numeric_work = nz(evaluation.numeric_work);
        let exact_receipt = evaluate_sibling_box_facts(projection, display, exact)
            .unwrap()
            .receipt();
        let mut expected_services = receipt.evaluation;
        let mut actual_services = exact_receipt.evaluation;
        expected_services.owned_heap_bytes = 0;
        actual_services.owned_heap_bytes = 0;
        assert_eq!(actual_services, expected_services);

        for (field, expected) in [
            (0, ClearanceBoxFactsError::Work),
            (1, ClearanceBoxFactsError::RuleVisits),
            (2, ClearanceBoxFactsError::MatchingWork),
            (3, ClearanceBoxFactsError::DeclarationOffers),
            (4, ClearanceBoxFactsError::MembershipWork),
            (5, ClearanceBoxFactsError::NumericWork),
        ] {
            let mut short = exact;
            let slot = match field {
                0 => &mut short.evaluation.work,
                1 => &mut short.evaluation.rule_visits,
                2 => &mut short.evaluation.matching_work,
                3 => &mut short.evaluation.declaration_offers,
                4 => &mut short.evaluation.membership_work,
                _ => &mut short.evaluation.numeric_work,
            };
            *slot = nz(slot.get() - 1);
            let error = evaluate_sibling_box_facts(projection, display, short)
                .err()
                .unwrap();
            assert!(matches!(
                error,
                SiblingBoxFactsError::Evaluation(actual) if actual == expected
            ));
        }
    });
}

#[test]
fn sources_and_requested_row_floor_fail_before_selector_scratch() {
    sibling_fixture(6, "#b3 { height: 1px; }", |projection, display| {
        let mut limited = limits();
        limited.sources = nz(5);
        limited.evaluation.selectors.work = nz(1);
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, limited),
            Err(SiblingBoxFactsError::Sources)
        ));

        limited = limits();
        limited.retained_bytes = nz(6 * size_of::<ClearanceBoxFact>() - 1);
        limited.evaluation.selectors.work = nz(1);
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, limited),
            Err(SiblingBoxFactsError::Bytes)
        ));
    });
}

#[test]
fn evaluation_prepayment_finishes_before_row_reserve_and_selector_scratch() {
    sibling_fixture(6, "#b3 { height: 1px; }", |projection, display| {
        let mut limited = limits();
        limited.retained_bytes = nz(1);
        limited.evaluation.selectors.work = nz(1);
        limited.evaluation.work = nz(1);
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, limited),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Work
            ))
        ));

        limited = limits();
        limited.retained_bytes = nz(1);
        limited.evaluation.selectors.work = nz(1);
        limited.evaluation.numeric_work = nz(1);
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, limited),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::NumericWork
            ))
        ));
    });
}

#[test]
fn actual_later_source_property_and_target_errors_publish_no_batch() {
    for (css, original, target, reason) in [
        (
            "#b3 { height: 1%; }",
            3,
            BoxFactTarget::Element,
            BoxFactUnsupported::PercentageLength,
        ),
        (
            "#b4:after { content: 'late'; }",
            4,
            BoxFactTarget::After,
            BoxFactUnsupported::GeneratedContent,
        ),
    ] {
        let source_count = if original == 3 { 6 } else { 7 };
        sibling_fixture(source_count, css, |projection, display| {
            let projection_receipt = projection.receipt();
            let token = projection.token(source_count - 1).unwrap();
            let error = evaluate_sibling_box_facts(projection, display, limits())
                .err()
                .unwrap();
            assert!(matches!(
                error,
                SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Unsupported {
                    original: actual,
                    target: actual_target,
                    reason: actual_reason,
                    ..
                }) if actual == original && actual_target == target && actual_reason == reason
            ));
            assert_eq!(projection.receipt(), projection_receipt);
            assert!(std::ptr::eq(
                token,
                projection.token(source_count - 1).unwrap()
            ));
        });
    }
}

#[test]
fn snapshot_and_service_causes_remain_typed() {
    let first = captured_siblings(6);
    let second = captured_siblings(6);
    let projection = prepare_sibling_source_projection(
        &first,
        first.snapshot().unwrap(),
        SiblingProjectionLimits {
            sources: bound(),
            work: bound(),
            retained_bytes: bound(),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph("#b3 { height: 1px; }", context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display =
        evaluate_source_display(second.snapshot().unwrap(), &bundle, display_limits()).unwrap();
    assert!(matches!(
        evaluate_sibling_box_facts(&projection, &display, limits()),
        Err(SiblingBoxFactsError::Evaluation(
            ClearanceBoxFactsError::SnapshotMismatch
        ))
    ));

    sibling_fixture(6, "#b3 { height: 1in; }", |projection, display| {
        let mut short = limits();
        short.evaluation.numeric.work = nz(1);
        let error = evaluate_sibling_box_facts(projection, display, short)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length { original: 3, .. })
        ));
        assert!(error.source().unwrap().is::<ClearanceBoxFactsError>());
    });
}

#[test]
fn per_target_and_selector_limits_retain_existing_typed_errors() {
    sibling_fixture(
        6,
        "#b3 { height: 1px; width: 2px; }",
        |projection, display| {
            let mut short = limits();
            short.evaluation.per_target.rules = nz(display.bundle().rule_count() - 1);
            assert!(matches!(
                evaluate_sibling_box_facts(projection, display, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Rules
                ))
            ));

            short = limits();
            short.evaluation.per_target.per_rule.declarations = nz(1);
            assert!(matches!(
                evaluate_sibling_box_facts(projection, display, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Declarations { .. }
                ))
            ));

            short = limits();
            short.evaluation.per_target.per_rule.matching.work = nz(1);
            assert!(matches!(
                evaluate_sibling_box_facts(projection, display, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Cascade { .. }
                ))
            ));

            short = limits();
            short.evaluation.selectors.work = nz(1);
            assert!(matches!(
                evaluate_sibling_box_facts(projection, display, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Source(_)
                ))
            ));
        },
    );
}
