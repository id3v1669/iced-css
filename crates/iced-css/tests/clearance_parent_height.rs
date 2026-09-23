#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::clearance_bridge::*;
use iced_css_core::formatting::{
    bfc_height::*, block::*, clearance_owner::*, first_child_origin::*,
};
use support::*;
const CSS: &str = "#o, #p, #b1, #f, #b2 {display:block; margin:0; padding:0; border-style:none;} #p {overflow:hidden; padding-top:1px; padding-bottom:1px;} #b1 {height:1px; margin-bottom:4px;} #f {float:left; width:10px; height:2px;} #b2 {height:1px; margin-top:3px; clear:both;}";
fn fixture(
    css: &str,
    check: impl FnOnce(
        &ClearanceSourceProjection<'_>,
        &iced_css::evaluation::source_display::SourceDisplay<'_, '_, '_>,
    ),
) {
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
    let graph = graph(css, context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    check(&projection, &display);
}

fn outer<'p>(
    facts: &ClearanceBoxFacts<'p, '_, '_, '_, '_, '_>,
) -> ClearanceOuterContentGeometry<'p> {
    ClearanceOuterContentGeometry {
        outer: &facts.projection().nodes()[0],
        content_left: 0.,
        content_width: 100.,
        content_top: 10.,
    }
}
#[test]
fn auto_uses_real_normal_margin_and_all_float_sides_from_designated_top() {
    for (suffix, normal, float, edge, height, extended) in [
        ("", 19., 18., 19., 8., false),
        ("#b2 {margin-bottom:3px;}", 19., 18., 22., 11., false),
        (
            "#b2 {clear:none;} #f {height:20px;}",
            17.,
            36.,
            17.,
            25.,
            true,
        ),
        (
            "#b2 {clear:none; margin-bottom:30px;} #f {height:20px;}",
            17.,
            36.,
            47.,
            36.,
            false,
        ),
        (
            "#b2 {clear:left;} #f {float:right; height:20px;}",
            17.,
            36.,
            17.,
            25.,
            true,
        ),
        (
            "#b2 {clear:right;} #f {float:left; height:20px;}",
            17.,
            36.,
            17.,
            25.,
            true,
        ),
        ("#b1 {margin-top:2px;}", 21., 20., 21., 10., false),
        (
            "#p {padding-bottom:9px; margin-bottom:-20px;}",
            19.,
            18.,
            19.,
            8.,
            false,
        ),
        ("#p {margin-bottom:auto;}", 19., 18., 19., 8., false),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let got = build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap();
            assert_eq!(got.parent().parent_origin().content_top, 11.);
            assert_eq!(got.parent().owner().after().bottom, normal);
            assert_eq!(got.parent().owner().placed_float().bottom, float);
            assert_eq!(got.height().used_height, height);
            assert_eq!(got.height().content_bottom, 11. + height);
            assert_eq!(
                got.height().auto_extent,
                Some(AutoHeightExtent {
                    normal_margin_bottom: edge,
                    selected_bottom: 11. + height,
                    float_extended: extended
                })
            );
            assert_eq!(
                got.computed_height(),
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Initial
                }
            );
            assert!(std::ptr::eq(got.parent().owner().sources(), p.nodes()));
            for (index, node) in got.parent().owner().sources().iter().enumerate() {
                assert!(std::ptr::eq(node, &p.nodes()[index]));
            }
            assert_eq!(
                got.parent().owner().sources()[1].parent,
                Some(p.nodes()[0].instance)
            );
        });
    }
}
#[test]
fn explicit_and_inherited_height_preserve_actual_computed_provenance() {
    for (suffix, pixels, origin, rounded) in [
        ("#p {height:2px;}", 2., BoxFactOrigin::Specified, false),
        ("#p {height:0;}", 0., BoxFactOrigin::Specified, false),
        (
            "#o {height:2px;} #p {height:inherit;}",
            2.,
            BoxFactOrigin::Inherited { original_parent: 4 },
            false,
        ),
        (
            "#p {height:4px; height:2px;}",
            2.,
            BoxFactOrigin::Specified,
            false,
        ),
        (
            "#p {height:2px !important; height:4px;}",
            2.,
            BoxFactOrigin::Specified,
            false,
        ),
        (
            "#p {height:0.000000000000000000000000000000000000000000000000001px;}",
            0.,
            BoxFactOrigin::Specified,
            true,
        ),
        (
            "#o {height:0.000000000000000000000000000000000000000000000000001px;} #p {height:inherit;}",
            0.,
            BoxFactOrigin::Inherited { original_parent: 4 },
            true,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let got = build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap();
            assert_eq!(
                got.height(),
                BfcHeightPlacement {
                    used_height: pixels,
                    content_bottom: 11. + pixels,
                    auto_extent: None
                }
            );
            assert_eq!(
                got.computed_height(),
                BoxLengthFact::Absolute(AbsoluteBoxFact {
                    pixels,
                    origin,
                    rounded_to_zero: rounded
                })
            );
            assert_eq!(got.computed_height(), facts.row(1).unwrap().height);
            let mut copied = got.height();
            copied.used_height = 99.;
            assert_ne!(copied, got.height());
            let mut computed = got.computed_height();
            if let BoxLengthFact::Absolute(ref mut value) = computed {
                value.pixels = 99.;
            }
            assert_ne!(computed, got.computed_height());
        });
    }
    for (suffix, origin) in [
        ("#p {height:auto;}", BoxFactOrigin::Specified),
        (
            "#o {height:auto;} #p {height:inherit;}",
            BoxFactOrigin::Inherited { original_parent: 4 },
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let got = build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap();
            assert_eq!(got.computed_height(), BoxLengthFact::Auto { origin });
            assert_eq!(got.height().used_height, 8.);
        });
    }
}
#[test]
fn overflowing_unused_normal_extent_is_lazy_only_for_explicit_height() {
    use std::error::Error;
    const MAX: &str = "340282346638528859811704183484516925440px";
    for height in ["2px", "auto"] {
        fixture(
            &format!("{CSS} #b2 {{height:{MAX}; margin-bottom:{MAX};}} #p {{height:{height};}}"),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let old = build_captured_clearance_from_outer(&facts, outer(&facts)).unwrap();
                assert_eq!(old.owner().after().bottom, f32::MAX);
                let result = build_captured_clearance_with_parent_height(&facts, outer(&facts));
                if height == "2px" {
                    assert_eq!(
                        result.unwrap().height(),
                        BfcHeightPlacement {
                            used_height: 2.,
                            content_bottom: 13.,
                            auto_extent: None
                        }
                    );
                } else {
                    let error = result.unwrap_err();
                    let cause = BfcHeightError::Unrepresentable {
                        stage: BfcHeightStage::NormalMarginBottom,
                    };
                    assert_eq!(error, ParentHeightError::Height(cause));
                    assert_eq!(
                        error.source().unwrap().downcast_ref::<BfcHeightError>(),
                        Some(&cause)
                    );
                    assert!(cause.source().is_none());
                }
            },
        );
    }
}
#[test]
fn parent_errors_and_their_typed_causes_precede_height() {
    use std::error::Error;
    for (suffix, expected) in [
        (
            "#b2 {margin-top:6px;}",
            ParentClearanceError::Captured(CapturedClearanceError::Core(
                ClearanceOwnerError::UnsupportedEquality,
            )),
        ),
        (
            "#f {width:101px;}",
            ParentClearanceError::Captured(CapturedClearanceError::Core(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: OwnerProfileReason::FloatTooWide,
                },
            )),
        ),
        (
            "#b2 {position:relative;}",
            ParentClearanceError::Captured(CapturedClearanceError::Unsupported {
                projected: 4,
                property: "display/position/float",
                reason: CapturedClearanceUnsupported::DisplayOrFlow,
            }),
        ),
        (
            "#p {overflow:visible;}",
            ParentClearanceError::Captured(CapturedClearanceError::Unsupported {
                projected: 1,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ParentOverflow,
            }),
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let error =
                build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap_err();
            assert_eq!(error, ParentHeightError::Parent(expected));
            assert_eq!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<ParentClearanceError>(),
                Some(&expected)
            );
            assert_eq!(
                build_captured_clearance_from_outer(&facts, outer(&facts)).unwrap_err(),
                expected
            );
        });
    }
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        for (left, top, expected) in [
            (
                f32::MAX,
                f32::NAN,
                ParentClearanceError::Horizontal(BlockHorizontalError::Unrepresentable {
                    stage: BlockCoordinateStage::ContentRight,
                }),
            ),
            (
                0.,
                f32::NAN,
                ParentClearanceError::Origin(FirstChildOriginError::InvalidInput("containing_top")),
            ),
        ] {
            let mut geometry = outer(&facts);
            geometry.content_left = left;
            geometry.content_top = top;
            geometry.content_width = f32::MAX;
            assert_eq!(
                build_captured_clearance_with_parent_height(&facts, geometry).unwrap_err(),
                ParentHeightError::Parent(expected)
            );
        }
        fixture(CSS, |other, _| {
            let mut geometry = outer(&facts);
            geometry.outer = &other.nodes()[0];
            geometry.content_top = f32::NAN;
            assert_eq!(
                build_captured_clearance_with_parent_height(&facts, geometry).unwrap_err(),
                ParentHeightError::Parent(ParentClearanceError::OuterGeometrySource)
            );
        });
    });
    support::fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap_err(),
            ParentHeightError::Parent(ParentClearanceError::Captured(
                CapturedClearanceError::Shape
            ))
        );
    });
    fixture(&format!("{CSS} #b2 {{margin-bottom:-1px;}}"), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let prior = build_captured_clearance_from_outer(&facts, outer(&facts)).unwrap_err();
        assert!(matches!(
            prior,
            ParentClearanceError::Captured(CapturedClearanceError::Core(_))
        ));
        assert_eq!(
            build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap_err(),
            ParentHeightError::Parent(prior)
        );
    });
}
#[test]
fn generated_content_is_rejected_before_composition() {
    for selector in ["#o:before", "#p:after", "#b2:before"] {
        fixture(&format!(r#"{CSS} {selector} {{content:"";}}"#), |p, d| {
            assert!(matches!(
                evaluate_clearance_box_facts(p, d, fact_limits()),
                Err(ClearanceBoxFactsError::Unsupported {
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                })
            ));
        });
    }
}
