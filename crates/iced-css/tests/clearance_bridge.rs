#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::clearance_bridge::*;
use iced_css_core::formatting::{clearance_owner::*, floats::Band};
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
#[test]
fn actual_captured_css_produces_negative_clearance_with_complete_membership() {
    fixture(CSS, |projection, display| {
        let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
        let owner = build_captured_clearance_owner(
            &facts,
            ClearanceContentGeometry {
                parent: &projection.nodes()[1],
                content_top: 1.,
                containing: Band {
                    left: 0.,
                    right: 100.,
                },
            },
        )
        .unwrap();
        assert!(std::ptr::eq(owner.sources(), projection.nodes()));
        assert_eq!(owner.sources().len(), 5);
        assert_eq!(owner.sources()[1].parent, Some(owner.sources()[0].instance));
        assert_eq!(
            owner.before(),
            BlockVertical {
                top: 1.,
                bottom: 2.
            }
        );
        let f = owner.placed_float();
        assert_eq!((f.left, f.right, f.top, f.bottom), (0., 10., 6., 8.));
        assert_eq!(owner.hypothetical_top(), 6.);
        assert_eq!(owner.zero_clearance_top(), Some(9.));
        assert_eq!(
            owner.after(),
            BlockVertical {
                top: 8.,
                bottom: 9.
            }
        );
        assert_eq!(owner.clearance(), Some(-1.));
        assert_eq!(owner.barrier_source_index(), Some(4));
        assert_eq!(
            owner.hypothetical_group().members(),
            &[
                MarginMember {
                    source_index: 2,
                    side: MarginSide::Bottom,
                    value: 4.
                },
                MarginMember {
                    source_index: 4,
                    side: MarginSide::Top,
                    value: 3.
                }
            ]
        );
        assert_eq!(owner.actual_groups()[0].id(), MarginGroupId::BeforeBarrier);
        assert_eq!(owner.actual_groups()[1].id(), MarginGroupId::AfterBarrier);
    });
}

fn geometry<'p>(projection: &'p ClearanceSourceProjection<'_>) -> ClearanceContentGeometry<'p> {
    ClearanceContentGeometry {
        parent: &projection.nodes()[1],
        content_top: 1.,
        containing: Band {
            left: 0.,
            right: 100.,
        },
    }
}
#[test]
fn actual_css_variations_preserve_signed_clearance_and_float_placement() {
    for (suffix, float_edges, h, b, c, top) in [
        (
            "#f {height:5px;}",
            (0., 10., 6., 11.),
            6.,
            Some(9.),
            Some(2.),
            11.,
        ),
        (
            "#f {height:3px;}",
            (0., 10., 6., 9.),
            6.,
            Some(9.),
            Some(0.),
            9.,
        ),
        ("#b2 {clear:right;}", (0., 10., 6., 8.), 6., None, None, 6.),
        (
            "#b2 {margin-top:8px;}",
            (0., 10., 6., 8.),
            10.,
            None,
            None,
            10.,
        ),
        (
            "#f {float:right;}",
            (90., 100., 6., 8.),
            6.,
            Some(9.),
            Some(-1.),
            8.,
        ),
        (
            "#f {width:0;}",
            (0., 0., 6., 8.),
            6.,
            Some(9.),
            Some(-1.),
            8.,
        ),
        (
            "#f {height:0;} #b2 {clear:right;}",
            (0., 10., 6., 6.),
            6.,
            None,
            None,
            6.,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            let owner = build_captured_clearance_owner(&facts, geometry(projection)).unwrap();
            let f = owner.placed_float();
            assert_eq!((f.left, f.right, f.top, f.bottom), float_edges, "{suffix}");
            assert_eq!(owner.hypothetical_top(), h);
            assert_eq!(owner.zero_clearance_top(), b);
            assert_eq!(owner.clearance(), c);
            assert_eq!(owner.after().top, top);
            assert_eq!(owner.barrier_source_index(), c.map(|_| 4));
            if c.is_none() {
                assert_eq!(owner.actual_groups().len(), 1);
                assert_eq!(owner.actual_groups()[0].id(), MarginGroupId::ActualPair);
            }
        });
    }
    for suffix in ["#b2 {margin-top:6px;}", "#f {height:0;}"] {
        fixture(&format!("{CSS} {suffix}"), |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Core(ClearanceOwnerError::UnsupportedEquality)
            );
        });
    }
}
#[test]
fn computed_categories_and_core_profile_failures_keep_projected_indices() {
    use CapturedClearanceError::{Core, Unsupported};
    use CapturedClearanceUnsupported as U;
    for (suffix, projected, property, reason) in [
        ("#p {overflow:visible;}", 1, "overflow", U::ParentOverflow),
        ("#p {overflow:auto;}", 1, "overflow", U::ParentOverflow),
        ("#p {overflow:scroll;}", 1, "overflow", U::ParentOverflow),
        ("#b1 {overflow:hidden;}", 2, "overflow", U::ChildOverflow),
        ("#b2 {overflow:hidden;}", 4, "overflow", U::ChildOverflow),
        ("#f {overflow:scroll;}", 3, "overflow", U::ChildOverflow),
        (
            "#b1 {position:relative;}",
            2,
            "display/position/float",
            U::DisplayOrFlow,
        ),
        (
            "#o {display:none;}",
            0,
            "display/position/float",
            U::DisplayOrFlow,
        ),
        ("#b2 {width:1px;}", 4, "width", U::BlockWidth),
        ("#b1 {height:auto;}", 2, "height", U::HeightKind),
        ("#f {width:auto;}", 3, "width", U::FloatWidth),
        ("#b2 {margin-top:auto;}", 4, "margin-top", U::AutoMargin),
        ("#b1 {margin-left:auto;}", 2, "margin-left", U::AutoMargin),
        ("#f {margin-right:auto;}", 3, "margin-right", U::AutoMargin),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(p)).unwrap_err(),
                Unsupported {
                    projected,
                    property,
                    reason
                },
                "{suffix}"
            );
        });
    }
    for (suffix, source, reason) in [
        ("#f {width:101px;}", 3, OwnerProfileReason::FloatTooWide),
        ("#f {margin-left:5px;}", 3, OwnerProfileReason::FloatMargin),
        ("#f {margin-right:7px;}", 3, OwnerProfileReason::FloatMargin),
        ("#f {margin-top:3px;}", 3, OwnerProfileReason::FloatMargin),
        (
            "#f {margin-bottom:4px;}",
            3,
            OwnerProfileReason::FloatMargin,
        ),
        (
            "#b2 {margin-top:-1px;}",
            4,
            OwnerProfileReason::NegativeMargin,
        ),
        (
            "#b1 {margin-left:1px;}",
            2,
            OwnerProfileReason::HorizontalMargin,
        ),
        (
            "#b2 {padding-top:1px;}",
            4,
            OwnerProfileReason::ChildDecoration,
        ),
        (
            "#b1 {height:0;}",
            2,
            OwnerProfileReason::NonpositiveBlockHeight,
        ),
        (
            "#b2 {height:0;}",
            4,
            OwnerProfileReason::NonpositiveBlockHeight,
        ),
        (
            "#p {padding-top:0;}",
            1,
            OwnerProfileReason::ParentIsolation,
        ),
        (
            "#p {padding-bottom:0;}",
            1,
            OwnerProfileReason::ParentIsolation,
        ),
        (
            "#b1 {clear:left;}",
            2,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ),
        (
            "#f {clear:both;}",
            3,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(p)).unwrap_err(),
                Core(ClearanceOwnerError::Unsupported { source, reason }),
                "{suffix}"
            );
        });
    }
}

#[test]
fn remaining_auto_margin_errors_keep_source_and_edge_order() {
    use CapturedClearanceUnsupported as U;

    for (suffix, projected, property) in [
        (
            "#b1 { width:auto; margin-right:auto; margin-bottom:auto; margin-left:auto; }",
            2,
            "margin-right",
        ),
        ("#b1 { width:auto; margin-left:auto; }", 2, "margin-left"),
        (
            "#b1, #b2 { width:auto; } #f { height:2px; margin-right:auto; margin-bottom:auto; }",
            3,
            "margin-right",
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected,
                    property,
                    reason: U::AutoMargin,
                }
            );
        });
    }

    fixture(
        &format!("{CSS} #b1 {{ width:auto; margin-right:auto; }} #b2 {{ height:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected: 4,
                    property: "height",
                    reason: U::HeightKind,
                }
            );
        },
    );

    fixture(
        &format!("{CSS} #f {{ width:auto; height:2px; margin-right:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "width",
                    reason: U::FloatWidth,
                }
            );
        },
    );
}

#[test]
fn legacy_capture_keeps_auto_margin_and_width_extraction_precedence() {
    fixture(
        &format!("{CSS} #f {{ width:101px; margin-top:auto; margin-left:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "margin-top",
                    reason: CapturedClearanceUnsupported::AutoMargin,
                }
            );
        },
    );

    fixture(
        &format!("{CSS} #f {{ width:101px; margin-left:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "margin-left",
                    reason: CapturedClearanceUnsupported::AutoMargin,
                }
            );
        },
    );

    fixture(
        &format!("{CSS} #f {{ width:101px; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Core(ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: OwnerProfileReason::FloatTooWide,
                })
            );
        },
    );

    fixture(
        &format!("{CSS} #f {{ width:auto; margin-left:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "width",
                    reason: CapturedClearanceUnsupported::FloatWidth,
                }
            );
        },
    );
}
#[test]
fn actual_generated_content_fails_before_bridge_and_geometry_is_pointer_bound() {
    for (selector, original, target) in [
        ("#o:before", 4, BoxFactTarget::Before),
        ("#o:after", 4, BoxFactTarget::After),
        ("#p:before", 3, BoxFactTarget::Before),
        ("#p:after", 3, BoxFactTarget::After),
    ] {
        fixture(&format!("{CSS} {selector} {{content:\"\";}}"), |p, d| {
            let error = evaluate_clearance_box_facts(p, d, fact_limits())
                .err()
                .unwrap();
            assert!(
                matches!(error,ClearanceBoxFactsError::Unsupported {original:o,target:t,reason:BoxFactUnsupported::GeneratedContent,..} if o==original && t==target)
            );
        });
    }
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let foreign = iced_css_core::source::SourceNode {
            instance: p.nodes()[1].instance,
            parent: p.nodes()[1].parent,
            children: vec![],
            metadata: Default::default(),
        };
        assert_eq!(
            build_captured_clearance_owner(
                &facts,
                ClearanceContentGeometry {
                    parent: &foreign,
                    ..geometry(p)
                }
            )
            .unwrap_err(),
            CapturedClearanceError::GeometrySource
        );
        let mut copied = *facts.row(3).unwrap();
        copied.height = BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        };
        assert!(matches!(copied.height, BoxLengthFact::Auto { .. }));
        assert_eq!(
            build_captured_clearance_owner(&facts, geometry(p))
                .unwrap()
                .clearance(),
            Some(-1.)
        );
    });
    support::fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build_captured_clearance_owner(&facts, geometry(p)).unwrap_err(),
            CapturedClearanceError::Shape
        );
    });
}
#[test]
fn actual_cascade_inheritance_and_f32_stage_overflow_reach_the_solver() {
    fixture(
        &format!(
            "{CSS} #o {{height:5px;}} #p {{height:inherit;}} #f {{height:inherit !important;}} #p > #f {{height:2px;}}"
        ),
        |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(p))
                    .unwrap()
                    .clearance(),
                Some(2.)
            );
        },
    );
    fixture(
        &format!("{CSS} #b1 {{height:300000000000000000000000000000000000000px;}}"),
        |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let g = ClearanceContentGeometry {
                content_top: 3e38,
                ..geometry(p)
            };
            assert_eq!(
                build_captured_clearance_owner(&facts, g).unwrap_err(),
                CapturedClearanceError::Core(ClearanceOwnerError::Unrepresentable {
                    stage: "before_bottom"
                })
            );
        },
    );
    fixture(
        &format!("{CSS} #p {{height:16777217px;}} #b1 {{height:inherit;}} #b2 {{clear:none;}}"),
        |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let owner = build_captured_clearance_owner(&facts, geometry(p)).unwrap();
            assert_eq!(owner.before().bottom, 16777216.);
        },
    );
}

#[test]
fn tiny_computed_heights_and_upstream_unsupported_values_keep_their_layer() {
    for id in ["b1", "b2"] {
        fixture(
            &format!(
                "{CSS} #{id} {{height:0.0000000000000000000000000000000000000000000000000001px;}}"
            ),
            |p, d| {
                let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
                let source = if id == "b1" { 2 } else { 4 };
                let BoxLengthFact::Absolute(value) = facts.row(source).unwrap().height else {
                    panic!()
                };
                assert!(value.rounded_to_zero);
                assert_eq!(value.pixels, 0.);
                assert_eq!(
                    build_captured_clearance_owner(&facts, geometry(p)).unwrap_err(),
                    CapturedClearanceError::Core(ClearanceOwnerError::Unsupported {
                        source,
                        reason: OwnerProfileReason::NonpositiveBlockHeight
                    })
                );
            },
        );
    }
    for (suffix, original, property, reason) in [
        (
            "#b2 {height:10%;}",
            2,
            "height",
            BoxFactUnsupported::PercentageLength,
        ),
        (
            "#f {width:2em;}",
            1,
            "width",
            BoxFactUnsupported::RelativeLength,
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            assert!(
                matches!(evaluate_clearance_box_facts(p,d,fact_limits()),Err(ClearanceBoxFactsError::Unsupported {original:o,property:p,target:BoxFactTarget::Element,reason:r}) if o==original && p.descriptor().name==property && r==reason)
            );
        });
    }
    fixture(
        &format!(
            "{CSS} #o {{overflow:scroll; width:99px; height:10px;}} #p {{width:70px; height:20px; margin:7px;}}"
        ),
        |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(p))
                    .unwrap()
                    .clearance(),
                Some(-1.)
            );
        },
    );
}

#[test]
fn captured_legacy_vertical_auto_keeps_property_auto_margin_and_phase_order() {
    use CapturedClearanceUnsupported as U;
    let auto_margin = |projected, property| CapturedClearanceError::Unsupported {
        projected,
        property,
        reason: U::AutoMargin,
    };

    // Every Legacy role keeps the captured property-specific rejection for top
    // and bottom Auto; the sequence-level admission does not change this entry.
    for (selector, projected) in [("#b1", 2), ("#f", 3), ("#b2", 4)] {
        for (edge, property) in [("top", "margin-top"), ("bottom", "margin-bottom")] {
            fixture(
                &format!("{CSS} {selector} {{ margin-{edge}:auto; }}"),
                |projection, display| {
                    let facts =
                        evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
                    assert!(matches!(
                        facts.row(projected).unwrap().margin[if edge == "top" { 0 } else { 2 }],
                        BoxLengthFact::Auto {
                            origin: BoxFactOrigin::Specified
                        }
                    ));
                    assert_eq!(
                        build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                        auto_margin(projected, property)
                    );
                },
            );
        }
    }

    // Edge order stays top/right/bottom/left and the first projected source wins.
    for (suffix, projected, property) in [
        (
            "#b1 { margin-top:auto; margin-left:auto; }",
            2,
            "margin-top",
        ),
        (
            "#b1 { margin-right:auto; margin-bottom:auto; }",
            2,
            "margin-right",
        ),
        (
            "#b1 { margin-bottom:auto; margin-left:auto; }",
            2,
            "margin-bottom",
        ),
        (
            "#b1 { margin-bottom:auto; } #f { margin-top:auto; }",
            2,
            "margin-bottom",
        ),
        (
            "#f { margin-bottom:auto; } #b2 { margin-top:auto; }",
            3,
            "margin-bottom",
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                auto_margin(projected, property)
            );
        });
    }

    // Competing faults: height and float-width extraction precede child margin
    // conversion; the paired controls without them reach the vertical AutoMargin.
    for (suffix, expected) in [
        (
            "#b1 { margin-top:auto; } #b2 { height:auto; }",
            CapturedClearanceError::Unsupported {
                projected: 4,
                property: "height",
                reason: U::HeightKind,
            },
        ),
        ("#b1 { margin-top:auto; }", auto_margin(2, "margin-top")),
        (
            "#f { width:auto; margin-bottom:auto; }",
            CapturedClearanceError::Unsupported {
                projected: 3,
                property: "width",
                reason: U::FloatWidth,
            },
        ),
        (
            "#f { margin-bottom:auto; }",
            auto_margin(3, "margin-bottom"),
        ),
        (
            "#b2 { margin-bottom:auto; overflow:hidden; }",
            CapturedClearanceError::Unsupported {
                projected: 4,
                property: "overflow",
                reason: U::ChildOverflow,
            },
        ),
        (
            "#b2 { margin-bottom:auto; }",
            auto_margin(4, "margin-bottom"),
        ),
        (
            "#f { width:101px; margin-top:auto; }",
            auto_margin(3, "margin-top"),
        ),
        (
            "#f { width:101px; margin-top:0; }",
            CapturedClearanceError::Core(ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::FloatTooWide,
            }),
        ),
        (
            "#b2 { margin-top:0; margin-bottom:-1px; }",
            CapturedClearanceError::Core(ClearanceOwnerError::Unsupported {
                source: 4,
                reason: OwnerProfileReason::NegativeMargin,
            }),
        ),
        (
            "#b2 { margin-top:auto; margin-bottom:-1px; }",
            auto_margin(4, "margin-top"),
        ),
    ] {
        fixture(&format!("{CSS} {suffix}"), |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_owner(&facts, geometry(projection)).unwrap_err(),
                expected
            );
        });
    }

    // The parent-height Legacy entry also rejects at captured conversion before
    // any computed-margin height invariant.
    fixture(
        &format!("{CSS} #b2 {{ margin-bottom:auto; }}"),
        |projection, display| {
            let facts = evaluate_clearance_box_facts(projection, display, fact_limits()).unwrap();
            assert_eq!(
                build_captured_clearance_with_parent_height(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.0,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                )
                .unwrap_err(),
                ParentHeightError::Parent(ParentClearanceError::Captured(auto_margin(
                    4,
                    "margin-bottom"
                )))
            );
        },
    );
}
