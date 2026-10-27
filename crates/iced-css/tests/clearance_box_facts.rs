#[path = "support/clearance_fixture.rs"]
mod support;
use support::*;
const CSS: &str = "#p, #b1, #f, #b2 { display:block; } #p { padding-top:1px; padding-bottom:1px; } #b1 { height:1px; margin-bottom:4px; } #f { float:left; width:10px; height:2px; } #b2 { height:1px; margin-top:3px; clear:both; }";
fn px(v: BoxLengthFact) -> f32 {
    match v {
        BoxLengthFact::Absolute(v) => v.pixels,
        _ => panic!("expected absolute"),
    }
}
#[test]
fn actual_css_capture_computes_box_facts() {
    fixture(CSS, |projection, display| {
        let batch = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
        assert!(std::ptr::eq(batch.projection(), projection));
        assert!(std::ptr::eq(batch.display(), display));
        assert_eq!(
            batch
                .row(0)
                .unwrap()
                .padding
                .map(|v| v.absolute().unwrap().pixels),
            [1., 0., 1., 0.]
        );
        assert_eq!(px(batch.row(1).unwrap().height), 1.);
        assert_eq!(px(batch.row(1).unwrap().margin[2]), 4.);
        assert_eq!(px(batch.row(2).unwrap().width), 10.);
        assert_eq!(px(batch.row(2).unwrap().height), 2.);
        assert_eq!(px(batch.row(3).unwrap().height), 1.);
        assert_eq!(px(batch.row(3).unwrap().margin[0]), 3.);
        assert_eq!(batch.row(3).unwrap().clear.value, ClearSide::Both);
        for i in 0..4 {
            let row = batch.row(i).unwrap();
            assert_eq!(row.overflow.value, BoxOverflow::Visible);
            assert!(
                row.border_style
                    .iter()
                    .all(|s| s.value == BorderStyle::None)
            );
            assert_eq!(row.border_widths(), [0.; 4]);
            assert_eq!(row.before.origin, BoxFactOrigin::Initial);
            assert_eq!(row.after.origin, BoxFactOrigin::Initial);
        }
        for i in [0, 1, 3] {
            assert!(matches!(
                batch.row(i).unwrap().width,
                BoxLengthFact::Auto { .. }
            ));
        }
        assert!(matches!(
            batch.row(0).unwrap().height,
            BoxLengthFact::Auto { .. }
        ));
        assert!(batch.row(4).is_none());
        assert_eq!(batch.receipt().numeric_calls, 8);
        assert_eq!(batch.receipt().owned_heap_bytes, 0);
    });
}
#[test]
fn actual_cascade_shorthands_important_and_inheritance() {
    for parent_margin in [7, 13] {
        fixture(
            &format!(
                "#p {{margin: {parent_margin}px; padding: 1px 2px 3px 4px; clear:right; overflow:scroll; border-style:hidden;}} row {{margin:99px!important;}} #b1 {{margin:2px!important; margin-top:inherit!important; padding:inherit; clear:inherit; overflow:inherit; border-style:inherit;}} #b1 {{margin-right:5px!important;}} #p {{width:inherit;}}"
            ),
            |p, d| {
                let b = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let r = b.row(1).unwrap();
                assert_eq!(r.margin.map(px), [parent_margin as f32, 5., 2., 2.]);
                assert_eq!(
                    r.padding.map(|v| v.absolute().unwrap().pixels),
                    [1., 2., 3., 4.]
                );
                assert_eq!(r.clear.value, ClearSide::Right);
                assert_eq!(r.overflow.value, BoxOverflow::Scroll);
                assert!(
                    r.border_style
                        .iter()
                        .all(|s| s.value == BorderStyle::Hidden)
                );
                let origin = BoxFactOrigin::Inherited { original_parent: 3 };
                assert_eq!(r.clear.origin, origin);
                assert_eq!(r.padding[0].absolute().unwrap().origin, origin);
                assert_eq!(r.border_style[0].origin, origin);
                assert_eq!(
                    b.row(0).unwrap().width,
                    BoxLengthFact::Auto {
                        origin: BoxFactOrigin::Initial
                    }
                );
            },
        );
    }
}
#[test]
fn absolute_units_signed_margins_auto_and_numeric_history() {
    fixture(
        "#p {width:1in; margin-top:0.0000000000000000000000000000000000000000000000000001px;} #b1 {width:inherit; margin-top:inherit; margin-left:-2px; height:auto; margin-right:auto; min-width:inherit; max-height:inherit;}",
        |p, d| {
            let b = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(px(b.row(0).unwrap().width), 96.);
            assert_eq!(px(b.row(1).unwrap().width), 96.);
            for i in [0, 1] {
                let BoxLengthFact::Absolute(v) = b.row(i).unwrap().margin[0] else {
                    panic!()
                };
                assert_eq!(v.pixels, 0.);
                assert!(v.rounded_to_zero);
            }
            assert_eq!(px(b.row(1).unwrap().margin[3]), -2.);
            assert!(matches!(
                b.row(1).unwrap().margin[1],
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Specified
                }
            ));
            assert_eq!(b.receipt().numeric_calls, 3);
        },
    );
}
fn unsupported(
    css: &str,
    original: usize,
    name: &str,
    target: BoxFactTarget,
    reason: BoxFactUnsupported,
) {
    fixture(css, |p, d| {
        let error = evaluate_clearance_box_facts(p, d, fact_limits())
            .err()
            .unwrap();
        assert_eq!(
            error,
            ClearanceBoxFactsError::Unsupported {
                original,
                property: iced_css_core::values::registry::PropertyId::lookup(name).unwrap(),
                target,
                reason
            }
        );
    });
}
#[test]
fn unsupported_values_retain_actual_source_property_and_target() {
    for (value, reason) in [
        ("0em", BoxFactUnsupported::RelativeLength),
        ("2ex", BoxFactUnsupported::RelativeLength),
        ("0%", BoxFactUnsupported::PercentageLength),
    ] {
        unsupported(
            &format!("#b2 {{height:{value};}}"),
            2,
            "height",
            BoxFactTarget::Element,
            reason,
        );
    }
    for (name, value) in [
        ("min-width", "1px"),
        ("min-height", "2px"),
        ("max-width", "0px"),
        ("max-height", "1px"),
    ] {
        unsupported(
            &format!("#f {{{name}:{value};}}"),
            1,
            name,
            BoxFactTarget::Element,
            BoxFactUnsupported::NondefaultMinMax,
        );
    }
    for width in ["0px", "1px"] {
        unsupported(
            &format!("#b1 {{border: {width} solid;}}"),
            0,
            "border-top-style",
            BoxFactTarget::Element,
            BoxFactUnsupported::BorderStyle,
        );
    }
}
#[test]
fn zero_border_proof_skips_relative_width_and_inherits_styles() {
    fixture(
        "#p {border-width:3em; border-style:hidden;} #b1 {border-style:inherit; border-width:5ex;} #f {border:none 2em;} #b2 {border-style:inherit;}",
        |p, d| {
            let b = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(b.receipt().numeric_calls, 0);
            assert_eq!(b.row(1).unwrap().border_style[0].value, BorderStyle::Hidden);
            assert_eq!(b.row(2).unwrap().border_style[0].value, BorderStyle::None);
            assert_eq!(b.row(3).unwrap().border_widths(), [0.; 4]);
        },
    );
}
#[test]
fn pseudo_absence_uses_real_targets_and_originating_element_inheritance() {
    fixture(
        "#p {content:'ignored';} #p:before {content:inherit;} #b1 {content:'also ignored';} #b1:before {content:inherit;} #b1:after {content:normal;} #f:before {content:none;} #absent:after {content:'unmatched';}",
        |p, d| {
            let b = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(
                b.row(0).unwrap().before.origin,
                BoxFactOrigin::Inherited { original_parent: 3 }
            );
            assert_eq!(
                b.row(1).unwrap().before.origin,
                BoxFactOrigin::Inherited { original_parent: 0 }
            );
            assert_eq!(b.row(1).unwrap().after.origin, BoxFactOrigin::Specified);
            assert_eq!(b.row(2).unwrap().before.origin, BoxFactOrigin::Specified);
            assert_eq!(b.row(3).unwrap().after.origin, BoxFactOrigin::Initial);
        },
    );
    for (name, target) in [
        ("before", BoxFactTarget::Before),
        ("after", BoxFactTarget::After),
    ] {
        for content in ["''", "'x'"] {
            unsupported(
                &format!("#b2:{name} {{display:none; content:{content};}}"),
                2,
                "content",
                target,
                BoxFactUnsupported::GeneratedContent,
            );
        }
    }
}
#[test]
fn reservations_have_exact_boundaries_and_overflow_errors() {
    fixture(CSS, |p, d| {
        let result = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let r = result.receipt();
        let bundle = d.bundle();
        assert_eq!(r.work, 110 + bundle.rule_count());
        assert_eq!(r.targets, 12);
        assert_eq!(r.rule_visits, 12 * bundle.rule_count());
        assert_eq!(r.matching_work, r.rule_visits * 1000);
        assert_eq!(r.declaration_offers, 12 * bundle.declaration_count());
        assert_eq!(r.membership_work, 12 * bundle.author_rule_count() * 100);
        assert_eq!(r.numeric_work, 56 * 100_000);
        let mut exact = fact_limits();
        exact.work = nz(r.work);
        exact.rule_visits = nz(r.rule_visits);
        exact.matching_work = nz(r.matching_work);
        exact.declaration_offers = nz(r.declaration_offers);
        exact.membership_work = nz(r.membership_work);
        exact.numeric_work = nz(r.numeric_work);
        assert_eq!(
            evaluate_clearance_box_facts(p, d, exact).unwrap().receipt(),
            r
        );
        for (field, error) in [
            (0, ClearanceBoxFactsError::Work),
            (1, ClearanceBoxFactsError::RuleVisits),
            (2, ClearanceBoxFactsError::MatchingWork),
            (3, ClearanceBoxFactsError::DeclarationOffers),
            (4, ClearanceBoxFactsError::MembershipWork),
            (5, ClearanceBoxFactsError::NumericWork),
        ] {
            let mut short = exact;
            let slot = match field {
                0 => &mut short.work,
                1 => &mut short.rule_visits,
                2 => &mut short.matching_work,
                3 => &mut short.declaration_offers,
                4 => &mut short.membership_work,
                _ => &mut short.numeric_work,
            };
            *slot = nz(slot.get() - 1);
            assert_eq!(evaluate_clearance_box_facts(p, d, short).err(), Some(error));
        }
        let mut overflow = exact;
        overflow.numeric.work = NonZeroUsize::MAX;
        assert_eq!(
            evaluate_clearance_box_facts(p, d, overflow).err(),
            Some(ClearanceBoxFactsError::NumericWork)
        );
        overflow = exact;
        overflow.per_target.per_rule.matching.work = NonZeroUsize::MAX;
        assert_eq!(
            evaluate_clearance_box_facts(p, d, overflow).err(),
            Some(ClearanceBoxFactsError::MatchingWork)
        );
        let mut short = exact;
        short.per_target.rules = nz(bundle.rule_count() - 1);
        assert_eq!(
            evaluate_clearance_box_facts(p, d, short).err(),
            Some(ClearanceBoxFactsError::Rules)
        );
        short = exact;
        short.per_target.per_rule.declarations = nz(1);
        let first = bundle
            .offers()
            .iter()
            .position(|o| o.rule.declarations.len() > 1)
            .unwrap();
        assert_eq!(
            evaluate_clearance_box_facts(p, d, short).err(),
            Some(ClearanceBoxFactsError::Declarations { rule: first })
        );
    });
}
#[test]
fn late_after_failure_keeps_both_predecessors_and_exact_snapshot_join() {
    fixture("#b2:after {content:'late';}", |p, d| {
        let tokens = [
            p.token(0).unwrap(),
            p.token(1).unwrap(),
            p.token(2).unwrap(),
            p.token(3).unwrap(),
        ];
        let display = d.resolution(2).unwrap();
        let receipt = p.receipt();
        assert!(matches!(
            evaluate_clearance_box_facts(p, d, fact_limits()),
            Err(ClearanceBoxFactsError::Unsupported {
                original: 2,
                target: BoxFactTarget::After,
                ..
            })
        ));
        assert_eq!(p.receipt(), receipt);
        assert!(std::ptr::eq(display, d.resolution(2).unwrap()));
        for (i, token) in tokens.into_iter().enumerate() {
            assert!(std::ptr::eq(token, p.token(i).unwrap()));
        }
        let other = captured();
        let other_p = prepare_clearance_source_projection(
            &other,
            other.snapshot().unwrap(),
            ClearanceProjectionLimits {
                work: nz(27),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        assert_eq!(
            evaluate_clearance_box_facts(&other_p, d, fact_limits()).err(),
            Some(ClearanceBoxFactsError::SnapshotMismatch)
        );
    });
}
#[test]
fn service_errors_preserve_typed_causes() {
    use std::error::Error;
    fixture("#b1 {height:1in;}", |p, d| {
        let mut short = fact_limits();
        short.numeric.work = nz(1);
        let error = evaluate_clearance_box_facts(p, d, short).err().unwrap();
        assert!(
            matches!(error,ClearanceBoxFactsError::Length{original:0,property,error:iced_css_core::values::geometry::LengthEvaluationError::Conversion(iced_css_core::values::ConversionError::Numeric(iced_css_core::values::NumericError::WorkLimit))} if property.descriptor().name=="height")
        );
        assert!(
            error
                .source()
                .unwrap()
                .is::<iced_css_core::values::geometry::LengthEvaluationError>()
        );
        short = fact_limits();
        short.per_target.per_rule.matching.work = nz(1);
        let error = evaluate_clearance_box_facts(p, d, short).err().unwrap();
        assert!(matches!(
            error,
            ClearanceBoxFactsError::Cascade {
                original: 3,
                target: BoxFactTarget::Element,
                error: iced_css_core::cascade::rule::RuleBatchError::Rule(
                    iced_css_core::cascade::rule::RuleCascadeError::Match(
                        iced_css_core::selectors::MatchError::Work
                    )
                )
            }
        ));
        assert!(
            error
                .source()
                .unwrap()
                .is::<iced_css_core::cascade::rule::RuleBatchError>()
        );
        short = fact_limits();
        short.selectors.work = nz(1);
        let error = evaluate_clearance_box_facts(p, d, short).err().unwrap();
        assert_eq!(
            error,
            ClearanceBoxFactsError::Source(
                iced_css::capture::source::ConstructionSourceError::Work
            )
        );
        assert!(
            error
                .source()
                .unwrap()
                .is::<iced_css::capture::source::ConstructionSourceError>()
        );
    });
}
#[test]
fn changed_bundle_requires_its_own_display_and_fact_batch() {
    fixture("#b1 {height:3px;}", |p, d| {
        let original = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let ua = catalog();
        let graph = graph("#b1 {height:9px;}", context(100, 100));
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let changed = evaluate_source_display(p.snapshot(), &bundle, display_limits()).unwrap();
        let revised = evaluate_clearance_box_facts(p, &changed, fact_limits()).unwrap();
        assert_eq!(px(original.row(1).unwrap().height), 3.);
        assert_eq!(px(revised.row(1).unwrap().height), 9.);
        assert!(std::ptr::eq(revised.display().bundle(), &bundle));
        assert!(!std::ptr::eq(original.display().bundle(), &bundle));
        let mut copy = *revised.row(1).unwrap();
        copy.height = BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        };
        assert!(matches!(copy.height, BoxLengthFact::Auto { .. }));
        assert_eq!(px(revised.row(1).unwrap().height), 9.);
    });
}
#[test]
fn work_charges_precede_each_input_inspection_and_service_scratch() {
    fixture(CSS, |p, d| {
        let mut short = fact_limits();
        short.per_target.per_rule.declarations = nz(1);
        let first = d
            .bundle()
            .offers()
            .iter()
            .position(|o| o.rule.declarations.len() > 1)
            .unwrap();
        short.work = nz(5 + first);
        assert_eq!(
            evaluate_clearance_box_facts(p, d, short).err(),
            Some(ClearanceBoxFactsError::Work)
        );
        short.work = nz(6 + first);
        assert_eq!(
            evaluate_clearance_box_facts(p, d, short).err(),
            Some(ClearanceBoxFactsError::Declarations { rule: first })
        );
        short = fact_limits();
        short.selectors.work = nz(1);
        short.work = nz(109 + d.bundle().rule_count());
        assert_eq!(
            evaluate_clearance_box_facts(p, d, short).err(),
            Some(ClearanceBoxFactsError::Work)
        );
        for amount in 1..=4 {
            short = fact_limits();
            short.work = nz(amount);
            assert_eq!(
                evaluate_clearance_box_facts(p, d, short).err(),
                Some(ClearanceBoxFactsError::Work)
            );
        }
    });
}
#[test]
fn nondefault_max_rejection_precedes_numeric_conversion() {
    for (property, value) in [
        ("max-width", "1in"),
        (
            "max-height",
            "9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999px",
        ),
    ] {
        fixture(&format!("#p {{{property}:{value};}}"), |p, d| {
            let mut limits = fact_limits();
            limits.numeric.work = nz(1);
            assert_eq!(
                evaluate_clearance_box_facts(p, d, limits).err(),
                Some(ClearanceBoxFactsError::Unsupported {
                    original: 3,
                    property: iced_css_core::values::registry::PropertyId::lookup(property)
                        .unwrap(),
                    target: BoxFactTarget::Element,
                    reason: BoxFactUnsupported::NondefaultMinMax
                })
            );
        });
    }
}
#[test]
fn larger_matcher_byte_allowance_preserves_admissible_css() {
    fixture(
        "#b1 {height:1px;} #b1:before {content:none;} #b2:after {content:normal;}",
        |p, d| {
            let ordinary = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let mut larger = fact_limits();
            larger.per_target.per_rule.matching.retained_bytes = NonZeroUsize::MAX;
            let large = evaluate_clearance_box_facts(p, d, larger).unwrap();
            for i in 0..4 {
                assert_eq!(ordinary.row(i), large.row(i));
            }
            assert_eq!(ordinary.receipt(), large.receipt());
        },
    );
}

#[test]
fn nested_facts_follow_actual_ancestors_and_charge_fifteen_targets() {
    let host = captured_shape(true);
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_clearance_source_projection(
        &host,
        snapshot,
        ClearanceProjectionLimits {
            work: nz(36),
            retained_bytes: bound(),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph(
        "#o {padding-top:7px;} #p {padding-top:inherit; height:3px;} #o > #p > #b2 {height:inherit;} #p > #b2 {margin-top:9px;} #p:before {content:inherit;}",
        context(100, 100),
    );
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    let facts = evaluate_clearance_box_facts(&projection, &display, fact_limits()).unwrap();
    assert_eq!(
        facts.row(1).unwrap().padding[0].absolute().unwrap().pixels,
        7.
    );
    assert_eq!(
        facts.row(1).unwrap().padding[0].absolute().unwrap().origin,
        BoxFactOrigin::Inherited { original_parent: 4 }
    );
    assert_eq!(
        facts.row(4).unwrap().height,
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 3.,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Inherited { original_parent: 3 }
        })
    );
    assert_eq!(px(facts.row(4).unwrap().margin[0]), 9.);
    assert_eq!(
        facts.row(1).unwrap().before.origin,
        BoxFactOrigin::Inherited { original_parent: 3 }
    );
    assert!(facts.row(5).is_none());
    let receipt = facts.receipt();
    assert_eq!(receipt.targets, 15);
    assert_eq!(receipt.work, 137 + bundle.rule_count());
    assert_eq!(receipt.numeric_work, 70 * fact_limits().numeric.work.get());
    let mut exact = fact_limits();
    exact.work = nz(receipt.work);
    exact.rule_visits = nz(receipt.rule_visits);
    exact.matching_work = nz(receipt.matching_work);
    exact.declaration_offers = nz(receipt.declaration_offers);
    exact.membership_work = nz(receipt.membership_work);
    exact.numeric_work = nz(receipt.numeric_work);
    assert_eq!(
        evaluate_clearance_box_facts(&projection, &display, exact)
            .unwrap()
            .receipt(),
        receipt
    );
    for (field, error) in [
        (0, ClearanceBoxFactsError::Work),
        (1, ClearanceBoxFactsError::RuleVisits),
        (2, ClearanceBoxFactsError::MatchingWork),
        (3, ClearanceBoxFactsError::DeclarationOffers),
        (4, ClearanceBoxFactsError::MembershipWork),
        (5, ClearanceBoxFactsError::NumericWork),
    ] {
        let mut short = exact;
        let slot = match field {
            0 => &mut short.work,
            1 => &mut short.rule_visits,
            2 => &mut short.matching_work,
            3 => &mut short.declaration_offers,
            4 => &mut short.membership_work,
            _ => &mut short.numeric_work,
        };
        *slot = nz(slot.get() - 1);
        // An independent service failure would occur if aggregate preflight did
        // not reject first; no selector/service scratch has been admitted yet.
        short.selectors.work = nz(1);
        assert_eq!(
            evaluate_clearance_box_facts(&projection, &display, short).err(),
            Some(error)
        );
    }
    let mut overflow = exact;
    overflow.numeric.work = NonZeroUsize::MAX;
    assert_eq!(
        evaluate_clearance_box_facts(&projection, &display, overflow).err(),
        Some(ClearanceBoxFactsError::NumericWork)
    );
    overflow = exact;
    overflow.per_target.per_rule.matching.work = NonZeroUsize::MAX;
    assert_eq!(
        evaluate_clearance_box_facts(&projection, &display, overflow).err(),
        Some(ClearanceBoxFactsError::MatchingWork)
    );
}
