#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::clearance_bridge::*;
use iced_css_core::formatting::{block::*, clearance_owner::*, floats::Band};
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
) -> Result<CapturedHorizontalClearance<'p>, HorizontalClearanceError> {
    build_captured_clearance_with_horizontal(
        facts,
        ClearanceOuterHorizontalGeometry {
            outer: &facts.projection().nodes()[0],
            content_left: left,
            content_width: width,
        },
        ClearanceParentVerticalOrigin {
            parent: &facts.projection().nodes()[1],
            content_top: 1.,
        },
    )
}
#[test]
fn actual_css_horizontal_and_vertical_geometry_share_full_graph() {
    for (suffix, float_edges) in [("", (40., 50.)), ("#f {float:right;}", (70., 80.))] {
        fixture(
            &format!(
                "{CSS} #p {{width:40px; padding-left:5px; padding-right:5px; margin-left:auto; margin-right:auto;}} {suffix}"
            ),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let result = build(&facts, 10., 100.).unwrap();
                assert_eq!(
                    result.horizontal().used,
                    BlockWidthResult {
                        width: 40.,
                        margin_left: 25.,
                        margin_right: 25.,
                        passes: 1
                    }
                );
                assert_eq!(
                    result.horizontal().content,
                    Band {
                        left: 40.,
                        right: 80.
                    }
                );
                let owner = result.owner();
                assert!(std::ptr::eq(owner.sources(), p.nodes()));
                assert_eq!(owner.sources().len(), 5);
                assert_eq!(owner.sources()[1].parent, Some(owner.sources()[0].instance));
                let f = owner.placed_float();
                assert_eq!((f.left, f.right), float_edges);
                assert_eq!((f.top, f.bottom), (6., 8.));
                assert_eq!(owner.hypothetical_top(), 6.);
                assert_eq!(owner.zero_clearance_top(), Some(9.));
                assert_eq!(owner.clearance(), Some(-1.));
                assert_eq!(owner.after().top, 8.);
                assert_eq!(owner.barrier_source_index(), Some(4));
                let mut copy = result.horizontal();
                copy.content.left = -100.;
                copy.used.width = 99.;
                assert_eq!(copy.used.width, 99.);
                assert_eq!(result.horizontal().content.left, 40.);
            },
        );
    }
}
#[test]
fn css_auto_signed_and_containing_source_direction_reach_width_solver() {
    for (suffix, width, margins, band, passes) in [
        (
            "#p {width:auto; margin-left:10px; margin-right:20px; padding-left:5px; padding-right:7px;}",
            58.,
            (10., 20.),
            (25., 83.),
            1,
        ),
        (
            "#o {direction:ltr;} #p {direction:rtl; width:40px; margin-left:10px; margin-right:20px;}",
            40.,
            (10., 50.),
            (20., 60.),
            1,
        ),
        (
            "#o {direction:rtl !important; direction:ltr;} #p {direction:ltr; width:40px; margin-left:10px; margin-right:20px;}",
            40.,
            (40., 20.),
            (50., 90.),
            1,
        ),
        (
            "#o {direction:inherit;} #p {direction:rtl; width:40px; margin-left:10px; margin-right:20px;}",
            40.,
            (10., 50.),
            (20., 60.),
            1,
        ),
        (
            "#p {width:40px; margin-left:-10px; margin-right:auto;}",
            40.,
            (-10., 70.),
            (0., 40.),
            1,
        ),
        (
            "#p {width:auto; padding-left:110px;} #f {width:0;}",
            0.,
            (0., -10.),
            (120., 120.),
            2,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            assert_eq!(p.original_index(0), Some(4));
            assert_eq!(p.original_index(1), Some(3));
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let result = build(&facts, 10., 100.).unwrap();
            let h = result.horizontal();
            assert_eq!(
                h.used,
                BlockWidthResult {
                    width,
                    margin_left: margins.0,
                    margin_right: margins.1,
                    passes
                },
                "{suffix}"
            );
            assert_eq!((h.content.left, h.content.right), band, "{suffix}");
        });
    }
}
#[test]
fn endpoint_collapse_and_expansion_keep_used_width_and_downstream_policy() {
    for (width, float_width, accepted, collapsed, right) in [
        (1, 1, false, true, 16777216.),
        (1, 0, true, true, 16777216.),
        (3, 4, true, false, 16777220.),
    ] {
        fixture(
            &format!("{CSS} #p {{width:{width}px;}} #f {{width:{float_width}px;}}"),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let result = build(&facts, 16777216., width as f32);
                if accepted {
                    let result = result.unwrap();
                    let h = result.horizontal();
                    assert_eq!(h.used.width, width as f32);
                    assert_eq!(
                        h.content,
                        Band {
                            left: 16777216.,
                            right
                        }
                    );
                    assert_eq!(h.collapsed_positive_width(), collapsed);
                    assert_eq!(result.owner().clearance(), Some(-1.));
                } else {
                    assert!(matches!(
                        result,
                        Err(HorizontalClearanceError::Captured(
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
fn source_storage_binding_and_preflight_precedence_are_strict() {
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        fixture(CSS, |other, other_display| {
            // Independently captured owners have equal logical IDs, distinct storage.
            assert_eq!(p.nodes()[0].instance, other.nodes()[0].instance);
            assert_eq!(p.nodes()[1].instance, other.nodes()[1].instance);
            for (parent, outer, expected) in [
                (
                    &other.nodes()[1],
                    &other.nodes()[0],
                    HorizontalClearanceError::Captured(CapturedClearanceError::GeometrySource),
                ),
                (
                    &p.nodes()[1],
                    &other.nodes()[0],
                    HorizontalClearanceError::OuterGeometrySource,
                ),
            ] {
                let result = build_captured_clearance_with_horizontal(
                    &facts,
                    ClearanceOuterHorizontalGeometry {
                        outer,
                        content_left: f32::NAN,
                        content_width: 100.,
                    },
                    ClearanceParentVerticalOrigin {
                        parent,
                        content_top: 1.,
                    },
                );
                assert_eq!(result.err(), Some(expected));
            }
            assert!(evaluate_clearance_box_facts(p, other_display, fact_limits()).is_err());
        });
        assert_eq!(
            build(&facts, 0., 100.).unwrap().owner().clearance(),
            Some(-1.)
        );
    });
    support::fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build(&facts, f32::NAN, 100.).err(),
            Some(HorizontalClearanceError::Captured(
                CapturedClearanceError::Shape
            ))
        );
    });
}
#[test]
fn shared_admission_precedes_coordinates_and_coordinates_precede_core_geometry() {
    fixture(&format!("{CSS} #p {{overflow:visible;}}"), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build(&facts, f32::NAN, 100.).err(),
            Some(HorizontalClearanceError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 1,
                    property: "overflow",
                    reason: CapturedClearanceUnsupported::ParentOverflow
                }
            ))
        );
    });
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let result = build_captured_clearance_with_horizontal(
            &facts,
            ClearanceOuterHorizontalGeometry {
                outer: &p.nodes()[0],
                content_left: f32::MAX,
                content_width: f32::MAX,
            },
            ClearanceParentVerticalOrigin {
                parent: &p.nodes()[1],
                content_top: f32::NAN,
            },
        );
        assert_eq!(
            result.err(),
            Some(HorizontalClearanceError::Horizontal(
                BlockHorizontalError::Unrepresentable {
                    stage: BlockCoordinateStage::ContentRight
                }
            ))
        );
        let result = build_captured_clearance_with_horizontal(
            &facts,
            ClearanceOuterHorizontalGeometry {
                outer: &p.nodes()[0],
                content_left: 0.,
                content_width: 100.,
            },
            ClearanceParentVerticalOrigin {
                parent: &p.nodes()[1],
                content_top: f32::NAN,
            },
        );
        assert_eq!(
            result.err(),
            Some(HorizontalClearanceError::Captured(
                CapturedClearanceError::Core(ClearanceOwnerError::InvalidNumber {
                    source: 1,
                    field: "content_top"
                })
            ))
        );
        for width in [-1., f32::NAN, f32::INFINITY] {
            assert_eq!(
                build(&facts, 0., width).err(),
                Some(HorizontalClearanceError::Horizontal(
                    BlockHorizontalError::Width(BlockWidthError::InvalidInput("containing_width"))
                ))
            );
        }
        let old = build_captured_clearance_owner(
            &facts,
            ClearanceContentGeometry {
                parent: &p.nodes()[1],
                content_top: 1.,
                containing: Band {
                    left: 0.,
                    right: 100.,
                },
            },
        )
        .unwrap();
        assert_eq!(old.clearance(), Some(-1.));
    });
}
#[test]
fn downstream_and_upstream_errors_retain_their_layer_and_cause() {
    use std::error::Error;
    for (suffix, expected) in [
        (
            "#b2 {margin-top:6px;}",
            ClearanceOwnerError::UnsupportedEquality,
        ),
        (
            "#p {width:auto; padding-left:110px;}",
            ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::FloatTooWide,
            },
        ),
        (
            "#f {width:101px;}",
            ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::FloatTooWide,
            },
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let before = *facts.row(3).unwrap();
            let error = build(&facts, 0., 100.).err().unwrap();
            assert_eq!(
                error,
                HorizontalClearanceError::Captured(CapturedClearanceError::Core(expected))
            );
            let captured = error
                .source()
                .unwrap()
                .downcast_ref::<CapturedClearanceError>()
                .unwrap();
            assert_eq!(
                captured
                    .source()
                    .unwrap()
                    .downcast_ref::<ClearanceOwnerError>(),
                Some(&expected)
            );
            assert_eq!(*facts.row(3).unwrap(), before);
        });
    }
    let error = HorizontalClearanceError::Horizontal(BlockHorizontalError::Width(
        BlockWidthError::Unrepresentable,
    ));
    assert!(error.source().unwrap().is::<BlockHorizontalError>());
    assert!(
        HorizontalClearanceError::OuterGeometrySource
            .source()
            .is_none()
    );
    fixture(&format!("{CSS} #o:before {{content:\"\";}}"), |p, d| {
        assert!(matches!(
            evaluate_clearance_box_facts(p, d, fact_limits()),
            Err(ClearanceBoxFactsError::Unsupported {
                original: 4,
                target: BoxFactTarget::Before,
                reason: BoxFactUnsupported::GeneratedContent,
                ..
            })
        ));
    });
}
#[test]
fn horizontal_entry_preserves_vertical_overflow_cause() {
    use std::error::Error;
    fixture(
        &format!("{CSS} #b1 {{height:300000000000000000000000000000000000000px;}}"),
        |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let error = build_captured_clearance_with_horizontal(
                &facts,
                ClearanceOuterHorizontalGeometry {
                    outer: &p.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                },
                ClearanceParentVerticalOrigin {
                    parent: &p.nodes()[1],
                    content_top: 3e38,
                },
            )
            .unwrap_err();
            let expected = ClearanceOwnerError::Unrepresentable {
                stage: "before_bottom",
            };
            assert_eq!(
                error,
                HorizontalClearanceError::Captured(CapturedClearanceError::Core(expected))
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
        },
    );
}
