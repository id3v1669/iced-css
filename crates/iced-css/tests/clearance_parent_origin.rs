#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::clearance_bridge::*;
use iced_css_core::formatting::{block::*, clearance_owner::*, floats::Band};
use iced_css_core::formatting::{clearance::ClearanceRequirement, first_child_origin::*};
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
fn build<'p>(
    facts: &ClearanceBoxFacts<'p, '_, '_, '_, '_, '_>,
    left: f32,
    width: f32,
    top: f32,
) -> Result<CapturedParentClearance<'p>, ParentClearanceError> {
    build_captured_clearance_from_outer(
        facts,
        ClearanceOuterContentGeometry {
            outer: &facts.projection().nodes()[0],
            content_left: left,
            content_width: width,
            content_top: top,
        },
    )
}
#[test]
fn actual_margin_categories_and_provenance_translate_inner_clearance() {
    for (suffix, used, resolution, border, content, after) in [
        ("", 0., TopMarginResolution::Absolute, 10., 11., 18.),
        (
            "#o {margin-top:auto;} #p {margin-top:inherit;}",
            0.,
            TopMarginResolution::AutoZero,
            10.,
            11.,
            18.,
        ),
        (
            "#p {margin-top:auto;}",
            0.,
            TopMarginResolution::AutoZero,
            10.,
            11.,
            18.,
        ),
        (
            "#p {margin-top:4px;} * {margin-top:8px;} #b1, #f {margin-top:0;} #b2 {margin-top:3px;}",
            4.,
            TopMarginResolution::Absolute,
            14.,
            15.,
            22.,
        ),
        (
            "#p {margin-top:-4px;}",
            -4.,
            TopMarginResolution::Absolute,
            6.,
            7.,
            14.,
        ),
        (
            "#o {margin-top:4px;} #p {margin-top:inherit;}",
            4.,
            TopMarginResolution::Absolute,
            14.,
            15.,
            22.,
        ),
        (
            "#o {margin-top:40px; padding-top:20px;} #p {margin-top:4px !important; margin-top:8px;}",
            4.,
            TopMarginResolution::Absolute,
            14.,
            15.,
            22.,
        ),
        (
            "#p {padding-top:2px; height:40px; min-height:0; margin-bottom:-4px;}",
            0.,
            TopMarginResolution::Absolute,
            10.,
            12.,
            19.,
        ),
        (
            "#p {height:auto; margin-bottom:auto;}",
            0.,
            TopMarginResolution::Absolute,
            10.,
            11.,
            18.,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let result = build(&facts, 0., 100., 10.).unwrap();
            assert_eq!(
                result.parent_origin(),
                FirstChildOriginPlacement {
                    used_margin_top: used,
                    margin_resolution: resolution,
                    border_top: border,
                    content_top: content
                }
            );
            assert_eq!(
                result.computed_margin_top(),
                facts.row(1).unwrap().margin[0]
            );
            let expected_origin = if suffix.contains("inherit") {
                BoxFactOrigin::Inherited { original_parent: 4 }
            } else {
                BoxFactOrigin::Specified
            };
            let expected_fact = if resolution == TopMarginResolution::AutoZero {
                BoxLengthFact::Auto {
                    origin: expected_origin,
                }
            } else {
                BoxLengthFact::Absolute(AbsoluteBoxFact {
                    pixels: used,
                    rounded_to_zero: false,
                    origin: expected_origin,
                })
            };
            assert_eq!(result.computed_margin_top(), expected_fact);
            assert_eq!(result.owner().after().top, after);
            assert_eq!(result.owner().hypothetical_top(), content + 5.);
            assert_eq!(result.owner().zero_clearance_top(), Some(content + 8.));
            assert_eq!(result.owner().clearance(), Some(-1.));
            assert_eq!(result.owner().barrier_source_index(), Some(4));
            assert!(std::ptr::eq(result.owner().sources(), p.nodes()));
            assert_eq!(
                result.owner().sources()[1].parent,
                Some(p.nodes()[0].instance)
            );
            assert_eq!(
                result.horizontal().content,
                Band {
                    left: 0.,
                    right: 100.
                }
            );
        });
    }
}
#[test]
fn all_parent_clear_sides_exclude_postorder_descendant_float() {
    for (value, side) in [
        ("none", ClearSide::None),
        ("left", ClearSide::Left),
        ("right", ClearSide::Right),
        ("both", ClearSide::Both),
        ("inherit", ClearSide::Right),
    ] {
        fixture(
            &format!("{CSS} #o {{clear:right;}} #p {{clear:{value};}}"),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let result = build(&facts, 0., 100., 10.).unwrap();
                assert_eq!(p.original_index(3), Some(1));
                assert_eq!(p.original_index(1), Some(3));
                assert_eq!(result.parent_clear(), side);
                assert_eq!(result.parent_clearance(), ClearanceRequirement::Absent);
                assert_eq!(result.parent_origin().border_top, 10.);
                assert_eq!(result.owner().after().top, 18.);
            },
        );
    }
}
#[test]
fn rounded_zero_computed_history_survives_used_margin_resolution() {
    for sign in ["", "-"] {
        fixture(
            &format!(
                "{CSS} #p {{margin-top:{sign}0.000000000000000000000000000000000000000000000000001px;}}"
            ),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let result = build(&facts, 0., 100., 10.).unwrap();
                assert!(matches!(
                    result.computed_margin_top(),
                    BoxLengthFact::Absolute(AbsoluteBoxFact {
                        rounded_to_zero: true,
                        ..
                    })
                ));
                assert_eq!(
                    result.computed_margin_top(),
                    facts.row(1).unwrap().margin[0]
                );
                assert_eq!(result.parent_origin().used_margin_top.to_bits(), 0);
                assert_eq!(
                    result.parent_origin().margin_resolution,
                    TopMarginResolution::Absolute
                );
            },
        );
    }
}
#[test]
fn errors_keep_source_binding_admission_coordinate_and_core_order() {
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        fixture(CSS, |other, _| {
            assert_eq!(other.nodes()[0].instance, p.nodes()[0].instance);
            assert_eq!(
                build_captured_clearance_from_outer(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &other.nodes()[0],
                        content_left: f32::NAN,
                        content_width: 100.,
                        content_top: f32::NAN
                    }
                )
                .err(),
                Some(ParentClearanceError::OuterGeometrySource)
            );
        });
        assert_eq!(
            build(&facts, f32::MAX, f32::MAX, f32::NAN).err(),
            Some(ParentClearanceError::Horizontal(
                BlockHorizontalError::Unrepresentable {
                    stage: BlockCoordinateStage::ContentRight
                }
            ))
        );
        assert_eq!(
            build(&facts, 0., 100., f32::NAN).err(),
            Some(ParentClearanceError::Origin(
                FirstChildOriginError::InvalidInput("containing_top")
            ))
        );
    });
    support::fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build_captured_clearance_from_outer(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &p.nodes()[1],
                    content_left: f32::NAN,
                    content_width: 100.,
                    content_top: f32::NAN,
                }
            )
            .err(),
            Some(ParentClearanceError::Captured(
                CapturedClearanceError::Shape
            ))
        );
    });
    fixture(&format!("{CSS} #p {{overflow:visible;}}"), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build(&facts, f32::NAN, 100., f32::NAN).err(),
            Some(ParentClearanceError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 1,
                    property: "overflow",
                    reason: CapturedClearanceUnsupported::ParentOverflow
                }
            ))
        );
    });
    fixture(&format!("{CSS} #b1 {{margin-top:-1px;}}"), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build(&facts, 0., 100., f32::NAN).err(),
            Some(ParentClearanceError::Origin(
                FirstChildOriginError::InvalidInput("containing_top")
            ))
        );
        assert!(matches!(
            build(&facts, 0., 100., 10.),
            Err(ParentClearanceError::Captured(
                CapturedClearanceError::Core(_)
            ))
        ));
    });
}
#[test]
fn downstream_band_equality_and_typed_causes_remain_authoritative() {
    use std::error::Error;
    for (suffix, expected) in [
        (
            "#b2 {margin-top:6px;}",
            ClearanceOwnerError::UnsupportedEquality,
        ),
        (
            "#f {width:101px;}",
            ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::FloatTooWide,
            },
        ),
        (
            "#p {padding-top:0;}",
            ClearanceOwnerError::Unsupported {
                source: 1,
                reason: OwnerProfileReason::ParentIsolation,
            },
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let error = build(&facts, 0., 100., 10.).unwrap_err();
            assert_eq!(
                error,
                ParentClearanceError::Captured(CapturedClearanceError::Core(expected))
            );
            assert_eq!(
                error
                    .source()
                    .unwrap()
                    .source()
                    .unwrap()
                    .downcast_ref::<ClearanceOwnerError>(),
                Some(&expected)
            );
        });
    }
    for (fw, accepted) in [(0, true), (1, false)] {
        fixture(
            &format!("{CSS} #p {{width:1px;}} #f {{width:{fw}px;}}"),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let result = build(&facts, 16777216., 1., 10.);
                if accepted {
                    assert!(result.unwrap().horizontal().collapsed_positive_width());
                } else {
                    assert!(matches!(
                        result,
                        Err(ParentClearanceError::Captured(
                            CapturedClearanceError::Core(ClearanceOwnerError::Unsupported {
                                source: 3,
                                reason: OwnerProfileReason::FloatTooWide
                            })
                        ))
                    ));
                }
            },
        );
    }
}
#[test]
fn generated_content_still_fails_in_actual_batch() {
    for selector in ["#o:before", "#p:before", "#p:after"] {
        fixture(&format!("{CSS} {selector} {{content:\"\";}}"), |p, d| {
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
#[test]
fn defaulted_margin_retains_initial_provenance() {
    fixture(&CSS.replace("margin:0;", ""), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let result = build(&facts, 0., 100., 10.).unwrap();
        assert_eq!(
            result.computed_margin_top(),
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 0.,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Initial
            })
        );
        assert_eq!(result.parent_origin().content_top, 11.);
    });
}
