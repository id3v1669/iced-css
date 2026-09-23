#[path = "support/clearance_fixture.rs"]
mod support;

use iced_css::evaluation::sibling_layout::{
    PrincipalChildKind, PrincipalEdges, SiblingLayoutError, SiblingLayoutLimits,
    build_captured_sibling_layout,
};
use iced_css::evaluation::{
    clearance_box_facts::{
        BoxFactOrigin, BoxFactTarget, BoxFactUnsupported, BoxLengthFact, ClearanceBoxFactsError,
    },
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ClearanceOuterContentGeometry,
        ParentBottomError, ParentClearanceError, ParentHeightError,
        build_captured_clearance_with_parent_bottom,
    },
    sibling_box_facts::SiblingBoxFactsError,
};
use iced_css_core::formatting::{
    bfc_height::BfcHeightStage,
    block::{BlockCoordinateStage, BlockHorizontalError, BlockWidthResult},
    clearance::{ClearSide, ClearanceRequirement},
    clearance_owner::{
        ClearanceOwnerError, MarginGroupId, MarginMember, MarginSide, OwnerProfileReason,
    },
    clearance_sequence::{ClearanceSequenceError, ClearanceSequenceLimits},
};
use support::*;

fn layout_limits() -> SiblingLayoutLimits {
    SiblingLayoutLimits {
        sources: bound(),
        work: bound(),
        retained_bytes: nz(20_000_000),
        scratch_bytes: nz(20_000_000),
        core: ClearanceSequenceLimits {
            sources: bound(),
            work: bound(),
            retained_bytes: nz(20_000_000),
        },
    }
}

fn sibling_fact_limits() -> SiblingBoxFactsLimits {
    SiblingBoxFactsLimits {
        sources: bound(),
        retained_bytes: nz(20_000_000),
        evaluation: fact_limits(),
    }
}

fn baseline_css() -> &'static str {
    "
        #o { width: 100px; }
        #p { overflow: hidden; padding-top: 1px; padding-bottom: 1px; }
        #b1 { height: 1px; margin-bottom: 4px; }
        #f { float: left; width: 10px; height: 2px; }
        #b2 { height: 1px; margin-top: 3px; margin-bottom: 3px; clear: both; }
        #b3 { height: 1px; margin-top: 2px; margin-bottom: 4px; clear: both; }
    "
}

#[test]
fn six_source_actual_css_builds_source_linked_principal_geometry() {
    sibling_fixture(6, baseline_css(), |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            iced_css::evaluation::clearance_bridge::ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();

        assert_eq!(layout.sources().len(), 6);
        for index in 0..6 {
            assert!(std::ptr::eq(
                layout.source(index).unwrap(),
                &projection.nodes()[index]
            ));
        }
        assert!(layout.source(6).is_none());
        assert_eq!(projection.original_index(0), Some(5));
        assert_eq!(projection.original_index(1), Some(4));
        for source in 2..6 {
            let original = projection.original_index(source).unwrap();
            assert_eq!(original, source - 2);
            assert_eq!(projection.original_node(source).unwrap().parent(), Some(4));
            assert_eq!(
                projection.nodes()[source].parent,
                Some(projection.nodes()[1].instance)
            );
        }
        assert_eq!(projection.original_node(1).unwrap().parent(), Some(5));
        assert_eq!(
            projection.nodes()[1].parent,
            Some(projection.nodes()[0].instance)
        );

        let parent = layout.parent();
        assert_eq!(parent.source_index(), 1);
        assert_eq!(parent.containing_source_index(), 0);
        assert_eq!(
            parent.content(),
            PrincipalEdges {
                left: 0.0,
                right: 100.0,
                top: 11.0,
                bottom: 27.0,
            }
        );
        assert_eq!(parent.height().used_height, 16.0);
        assert_eq!(parent.bottom().padding_bottom, 28.0);
        assert_eq!(parent.bottom().border_bottom, 28.0);
        assert_eq!(parent.bottom().margin_bottom, 28.0);

        let children = layout.children();
        assert_eq!(children.len(), 4);
        let expected = [
            (
                2,
                PrincipalChildKind::Normal,
                PrincipalEdges {
                    left: 0.0,
                    right: 100.0,
                    top: 11.0,
                    bottom: 12.0,
                },
                100.0,
                1.0,
            ),
            (
                3,
                PrincipalChildKind::Float,
                PrincipalEdges {
                    left: 0.0,
                    right: 10.0,
                    top: 16.0,
                    bottom: 18.0,
                },
                10.0,
                2.0,
            ),
            (
                4,
                PrincipalChildKind::Normal,
                PrincipalEdges {
                    left: 0.0,
                    right: 100.0,
                    top: 18.0,
                    bottom: 19.0,
                },
                100.0,
                1.0,
            ),
            (
                5,
                PrincipalChildKind::Normal,
                PrincipalEdges {
                    left: 0.0,
                    right: 100.0,
                    top: 22.0,
                    bottom: 23.0,
                },
                100.0,
                1.0,
            ),
        ];
        for (child, (source, kind, edges, width, height)) in children.iter().zip(expected) {
            assert_eq!(child.source_index(), source);
            assert_eq!(child.containing_source_index(), 1);
            assert_eq!(child.kind(), kind);
            assert_eq!(child.content(), edges);
            assert_eq!(child.padding(), edges);
            assert_eq!(child.border(), edges);
            assert_eq!(child.used_width(), width);
            assert_eq!(child.used_height(), height);
        }
        assert_eq!(layout.sequence().tail()[1].source_index(), 5);
        assert_eq!(layout.receipt().nodes, 6);
        assert_eq!(layout.receipt().work, 32 + 32 * 6 + (6 - 2));
    });
}

#[test]
fn actual_rtl_fixed_width_reaches_principal_geometry() {
    let css = format!(
        "{} #o {{ direction:ltr; }} #p {{ direction:rtl; }} \
         #b3 {{ direction:ltr; width:40px; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        let b3 = &layout.children()[3];
        assert_eq!(b3.source_index(), 5);
        assert_eq!(b3.containing_source_index(), 1);
        assert_eq!(b3.used_width(), 40.0);
        assert_eq!(
            b3.normal_width(),
            Some(BlockWidthResult {
                width: 40.0,
                margin_left: 60.0,
                margin_right: 0.0,
                passes: 1,
            })
        );
        assert_eq!(
            b3.normal_width().unwrap().width.to_bits(),
            b3.used_width().to_bits()
        );
        assert!(std::ptr::eq(
            layout.source(b3.source_index()).unwrap(),
            &projection.nodes()[5]
        ));
        assert_eq!(
            b3.content(),
            PrincipalEdges {
                left: 60.0,
                right: 100.0,
                top: 22.0,
                bottom: 23.0,
            }
        );
        assert_eq!(layout.children()[1].normal_width(), None);
    });
}

#[test]
fn actual_width_matrix_uses_parent_direction_and_every_normal_source() {
    let cases = [
        ("ltr", "rtl", "", 100.0, 0.0, 100.0, 0.0, 0.0),
        ("ltr", "rtl", "width:0;", 0.0, 0.0, 0.0, 0.0, 100.0),
        ("ltr", "rtl", "width:40px;", 40.0, 0.0, 40.0, 0.0, 60.0),
        ("ltr", "rtl", "width:100px;", 100.0, 0.0, 100.0, 0.0, 0.0),
        ("ltr", "rtl", "width:140px;", 140.0, 0.0, 140.0, 0.0, -40.0),
        ("rtl", "ltr", "", 100.0, 0.0, 100.0, 0.0, 0.0),
        ("rtl", "ltr", "width:0;", 0.0, 100.0, 100.0, 100.0, 0.0),
        ("rtl", "ltr", "width:40px;", 40.0, 60.0, 100.0, 60.0, 0.0),
        ("rtl", "ltr", "width:100px;", 100.0, 0.0, 100.0, 0.0, 0.0),
        (
            "rtl",
            "ltr",
            "width:140px;",
            140.0,
            -40.0,
            100.0,
            -40.0,
            0.0,
        ),
    ];
    for (source_count, selector, selected_source) in [(6, "#b1", 2), (6, "#b3", 5), (7, "#b4", 6)] {
        for (
            parent_direction,
            conflicting_direction,
            width_rule,
            width,
            left,
            right,
            margin_left,
            margin_right,
        ) in cases
        {
            let b4 = if source_count == 7 {
                "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
            } else {
                ""
            };
            let css = format!(
                "{} {b4} #o {{ direction:{conflicting_direction}; }} \
                 #p {{ direction:{parent_direction}; }} \
                 #b1, #f, #b2, #b3, #b4 {{ direction:{conflicting_direction}; }} \
                 {selector} {{ {width_rule} }}",
                baseline_css()
            );
            sibling_fixture(source_count, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let layout = build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.0,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                    layout_limits(),
                )
                .unwrap();

                for child in layout.children() {
                    assert_eq!(child.containing_source_index(), 1);
                    assert!(std::ptr::eq(
                        layout.source(child.source_index()).unwrap(),
                        &projection.nodes()[child.source_index()]
                    ));
                    if child.kind() == PrincipalChildKind::Float {
                        assert_eq!(child.source_index(), 3);
                        assert_eq!(child.normal_width(), None);
                    } else {
                        assert!(child.normal_width().is_some());
                    }
                }

                let child = &layout.children()[selected_source - 2];
                assert_eq!(child.source_index(), selected_source);
                assert_eq!(child.used_width(), width);
                assert_eq!(child.content().left, left);
                assert_eq!(child.content().right, right);
                assert_eq!(
                    child.normal_width(),
                    Some(BlockWidthResult {
                        width,
                        margin_left,
                        margin_right,
                        passes: 1,
                    })
                );
            });
        }
    }
}

#[test]
fn fixed_width_preserves_full_vertical_sequence_and_parent_equation() {
    for (width_rule, expected_left) in [("", 2.0), ("width:40px;", 57.0)] {
        let css = format!(
            "{} #p {{ padding-left:2px; padding-right:3px; direction:rtl; }} \
             #f {{ height:20px; }} #b2 {{ clear:none; }} #b3 {{ {width_rule} }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();

            assert_eq!(layout.parent().horizontal().used.width, 95.0);
            assert_eq!(layout.parent().content().left, 2.0);
            assert_eq!(layout.parent().content().right, 97.0);
            assert_eq!(layout.children()[0].content().top, 11.0);
            assert_eq!(layout.children()[0].content().bottom, 12.0);
            assert_eq!(layout.children()[1].content().left, 2.0);
            assert_eq!(layout.children()[1].content().right, 12.0);
            assert_eq!(layout.children()[1].content().top, 16.0);
            assert_eq!(layout.children()[1].content().bottom, 36.0);
            assert_eq!(
                layout
                    .children()
                    .iter()
                    .filter(|child| child.kind() == PrincipalChildKind::Float)
                    .count(),
                1
            );
            assert_eq!(layout.children()[2].content().top, 16.0);
            assert_eq!(layout.children()[2].content().bottom, 17.0);
            assert_eq!(layout.children()[3].content().left, expected_left);
            assert_eq!(layout.children()[3].content().right, 97.0);
            assert_eq!(layout.children()[3].content().top, 36.0);
            assert_eq!(layout.children()[3].content().bottom, 37.0);

            let width = if width_rule.is_empty() { 95.0 } else { 40.0 };
            let margin_left = if width_rule.is_empty() { 0.0 } else { 55.0 };
            assert_eq!(
                layout.children()[3].normal_width(),
                Some(BlockWidthResult {
                    width,
                    margin_left,
                    margin_right: 0.0,
                    passes: 1,
                })
            );

            let b2 = &layout.sequence().tail()[0];
            assert_eq!(b2.source_index(), 4);
            assert_eq!(b2.vertical().top, 16.0);
            assert_eq!(b2.vertical().bottom, 17.0);
            assert_eq!(b2.hypothetical_top(), 16.0);
            assert_eq!(b2.zero_clearance_top(), None);
            assert_eq!(b2.clearance(), None);
            assert_eq!(b2.barrier_source_index(), None);
            assert_eq!(
                b2.hypothetical_group().id(),
                MarginGroupId::HypotheticalPair
            );
            assert_eq!(
                b2.hypothetical_group().members(),
                [
                    MarginMember {
                        source_index: 2,
                        side: MarginSide::Bottom,
                        value: 4.0,
                    },
                    MarginMember {
                        source_index: 4,
                        side: MarginSide::Top,
                        value: 3.0,
                    },
                ]
            );
            assert_eq!(b2.actual_groups().len(), 1);
            assert_eq!(b2.actual_groups()[0].id(), MarginGroupId::ActualPair);
            assert_eq!(
                b2.actual_groups()[0].members(),
                b2.hypothetical_group().members()
            );

            let b3 = &layout.sequence().tail()[1];
            assert_eq!(b3.source_index(), 5);
            assert_eq!(b3.vertical().top, 36.0);
            assert_eq!(b3.vertical().bottom, 37.0);
            assert_eq!(b3.hypothetical_top(), 20.0);
            assert_eq!(b3.zero_clearance_top(), Some(22.0));
            assert_eq!(b3.clearance(), Some(14.0));
            assert_eq!(b3.barrier_source_index(), Some(5));
            assert_eq!(
                b3.hypothetical_group().id(),
                MarginGroupId::HypotheticalPair
            );
            assert_eq!(
                b3.hypothetical_group().members(),
                [
                    MarginMember {
                        source_index: 4,
                        side: MarginSide::Bottom,
                        value: 3.0,
                    },
                    MarginMember {
                        source_index: 5,
                        side: MarginSide::Top,
                        value: 2.0,
                    },
                ]
            );
            assert_eq!(b3.actual_groups().len(), 2);
            assert_eq!(b3.actual_groups()[0].id(), MarginGroupId::BeforeBarrier);
            assert_eq!(
                b3.actual_groups()[0].members(),
                [MarginMember {
                    source_index: 4,
                    side: MarginSide::Bottom,
                    value: 3.0,
                }]
            );
            assert_eq!(b3.actual_groups()[1].id(), MarginGroupId::AfterBarrier);
            assert_eq!(
                b3.actual_groups()[1].members(),
                [MarginMember {
                    source_index: 5,
                    side: MarginSide::Top,
                    value: 2.0,
                }]
            );
            assert_eq!(layout.children()[2].clear().value, ClearSide::None);
            assert_eq!(layout.children()[3].clear().value, ClearSide::Both);
            assert_eq!(layout.parent().height().used_height, 30.0);
            assert_eq!(layout.parent().height().content_bottom, 41.0);
            assert_eq!(layout.parent().bottom().padding_bottom, 42.0);
            assert_eq!(layout.parent().bottom().border_bottom, 42.0);
            assert_eq!(layout.parent().bottom().margin_bottom, 42.0);
        });
    }
}

#[test]
fn seven_sources_use_the_actual_last_tail_for_auto_height() {
    let css = format!(
        "{} #b4 {{ height: 2px; margin-top: 1px; margin-bottom: 2px; clear: none; }}",
        baseline_css()
    );
    sibling_fixture(7, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();

        assert_eq!(layout.children().len(), 5);
        let b4 = &layout.children()[4];
        assert_eq!(b4.source_index(), 6);
        assert_eq!(
            b4.content(),
            PrincipalEdges {
                left: 0.0,
                right: 100.0,
                top: 27.0,
                bottom: 29.0
            }
        );
        assert_eq!(layout.sequence().tail().len(), 3);
        assert_eq!(layout.sequence().tail()[2].source_index(), 6);
        assert_eq!(layout.parent().height().used_height, 20.0);
        assert_eq!(layout.parent().height().content_bottom, 31.0);
        assert_eq!(layout.parent().bottom().border_bottom, 32.0);
        assert_eq!(layout.parent().content().bottom, 31.0);
        assert!(std::ptr::eq(
            layout.source(6).unwrap(),
            &projection.nodes()[6]
        ));
    });
}

#[test]
fn tall_float_extends_auto_height_while_normals_keep_the_full_parent_span() {
    let css = format!(
        "{} #f {{ height: 20px; }} #b2 {{ clear: none; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            layout.children()[1].content(),
            PrincipalEdges {
                left: 0.0,
                right: 10.0,
                top: 16.0,
                bottom: 36.0
            }
        );
        assert_eq!(
            layout.children()[2].content(),
            PrincipalEdges {
                left: 0.0,
                right: 100.0,
                top: 16.0,
                bottom: 17.0
            }
        );
        assert_eq!(
            layout.children()[3].content(),
            PrincipalEdges {
                left: 0.0,
                right: 100.0,
                top: 36.0,
                bottom: 37.0
            }
        );
        assert_eq!(layout.sequence().tail()[1].hypothetical_top(), 20.0);
        assert_eq!(layout.sequence().tail()[1].zero_clearance_top(), Some(22.0));
        assert_eq!(layout.sequence().tail()[1].clearance(), Some(14.0));
        assert_eq!(layout.parent().height().content_bottom, 41.0);
        assert_eq!(layout.parent().height().used_height, 30.0);
    });
}

#[test]
fn actual_parent_padding_direction_and_independent_used_width_feed_children() {
    for (float_rule, float_edges) in [("float:left", (2.0, 12.0)), ("float:right", (87.0, 97.0))] {
        let css = format!(
            "{} #p {{ padding-left:2px; padding-right:3px; direction:rtl; }} #f {{{float_rule};}}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();
            assert_eq!(layout.parent().horizontal().used.width, 95.0);
            assert_eq!(layout.parent().content().left, 2.0);
            assert_eq!(layout.parent().content().right, 97.0);
            for child in layout
                .children()
                .iter()
                .filter(|child| child.kind() == PrincipalChildKind::Normal)
            {
                assert_eq!(child.content().left, 2.0);
                assert_eq!(child.content().right, 97.0);
                assert_eq!(child.used_width(), 95.0);
            }
            assert_eq!(layout.children()[1].content().left, float_edges.0);
            assert_eq!(layout.children()[1].content().right, float_edges.1);
        });
    }

    let css = format!("{} #f {{ width:0; }}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 1.0e20,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            layout.parent().content().left,
            layout.parent().content().right
        );
        assert_eq!(
            layout.children()[0].content().left,
            layout.children()[0].content().right
        );
        assert_eq!(layout.children()[0].used_width(), 100.0);
        assert_eq!(layout.children()[1].used_width(), 0.0);
    });
}

#[test]
fn width_defaulting_inheritance_cascade_and_rounding_reach_layout_records() {
    sibling_fixture(6, baseline_css(), |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            layout.children()[0].computed_width(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Initial,
            }
        );
        assert_eq!(
            layout.children()[0].normal_width(),
            Some(BlockWidthResult {
                width: 100.0,
                margin_left: 0.0,
                margin_right: 0.0,
                passes: 1,
            })
        );
    });

    let zero_css = format!("{} #b1 {{ width:0; }}", baseline_css());
    sibling_fixture(6, &zero_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert!(matches!(
            layout.children()[0].computed_width(),
            BoxLengthFact::Absolute(value)
                if value.pixels == 0.0
                    && !value.rounded_to_zero
                    && value.origin == BoxFactOrigin::Specified
        ));
        assert_eq!(layout.children()[0].normal_width().unwrap().width, 0.0);
    });

    let inherited_css = format!(
        "{} #p {{ width:40px; }} #b3 {{ width:inherit; }}",
        baseline_css()
    );
    sibling_fixture(6, &inherited_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(projection.original_index(1), Some(4));
        assert!(matches!(
            layout.children()[3].computed_width(),
            BoxLengthFact::Absolute(value)
                if value.pixels == 40.0
                    && !value.rounded_to_zero
                    && value.origin == BoxFactOrigin::Inherited { original_parent: 4 }
        ));
        assert_eq!(
            layout.children()[3].normal_width(),
            Some(BlockWidthResult {
                width: 40.0,
                margin_left: 0.0,
                margin_right: 0.0,
                passes: 1,
            })
        );
    });

    let cascade_css = format!(
        "{} #b3 {{ width:140px; }} #b3 {{ width:40px; }}",
        baseline_css()
    );
    sibling_fixture(6, &cascade_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert!(matches!(
            layout.children()[3].computed_width(),
            BoxLengthFact::Absolute(value)
                if value.pixels == 40.0 && value.origin == BoxFactOrigin::Specified
        ));
        assert_eq!(layout.children()[3].used_width(), 40.0);
    });

    let tiny_css = format!(
        "{} #b3 {{ width:0.0000000000000000000000000000000000000000000000000001px; }}",
        baseline_css()
    );
    sibling_fixture(6, &tiny_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let BoxLengthFact::Absolute(tiny) = facts.row(5).unwrap().width else {
            panic!()
        };
        assert_eq!(tiny.pixels, 0.0);
        assert!(tiny.rounded_to_zero);
        assert_eq!(tiny.origin, BoxFactOrigin::Specified);
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            layout.children()[3].computed_width(),
            facts.row(5).unwrap().width
        );
        assert_eq!(layout.children()[3].normal_width().unwrap().width, 0.0);
    });
}

#[test]
fn used_width_remains_independent_when_published_endpoints_collapse() {
    for (rule, expected_width) in [("", 100.0), ("#b3 { width:40px; }", 40.0)] {
        let css = format!("{} #f {{ width:0; }} {rule}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 1.0e20,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();
            let b3 = &layout.children()[3];
            assert_eq!(b3.used_width(), expected_width);
            assert_eq!(b3.normal_width().unwrap().width, expected_width);
            assert_eq!(b3.content().left, b3.content().right);
            assert_eq!(b3.content().right - b3.content().left, 0.0);
        });
    }
}

#[test]
fn explicit_parent_height_is_lazy_but_all_children_are_still_laid_out() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (height, bottom) in [("2px", 13.0), ("0", 11.0)] {
        let css = format!(
            "{} #p {{ height:{height}; }} #b3 {{ height:{MAX}; margin-bottom:{MAX}; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();
            assert_eq!(layout.children().len(), 4);
            assert_eq!(layout.children()[3].content().bottom, f32::MAX);
            assert_eq!(layout.parent().height().content_bottom, bottom);
            assert_eq!(layout.parent().height().auto_extent, None);
        });
    }

    let auto_css = format!(
        "{} #p {{ height:auto; }} #b3 {{ height:{MAX}; margin-bottom:{MAX}; }}",
        baseline_css()
    );
    sibling_fixture(6, &auto_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::Height(
                iced_css_core::formatting::bfc_height::BfcHeightError::Unrepresentable {
                    stage: BfcHeightStage::NormalMarginBottom,
                }
            ))
        ));
    });
}

#[test]
fn facade_work_is_prepaid_before_late_actual_css_profile_extraction() {
    let css = format!("{} #b3 {{ overflow:hidden; }}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let work = 228;
        for short in [1, 227] {
            let mut limits = layout_limits();
            limits.work = nz(short);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Work)
            ));
        }
        let mut exact = layout_limits();
        exact.work = nz(work);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), exact),
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 5,
                    property: "overflow",
                    reason: CapturedClearanceUnsupported::ChildOverflow,
                }
            ))
        ));
    });
}

#[test]
fn facade_work_thresholds_are_exact_for_five_and_seven_sources() {
    for (source_count, suffix, work) in [
        (5, "", 195),
        (
            7,
            "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }",
            261,
        ),
    ] {
        let css = format!("{} {suffix}", baseline_css());
        sibling_fixture(source_count, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let outer = || ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            };
            let mut one_short = layout_limits();
            one_short.work = nz(work - 1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), one_short),
                Err(SiblingLayoutError::Work)
            ));
            let mut exact = layout_limits();
            exact.work = nz(work);
            let layout = build_captured_sibling_layout(&facts, outer(), exact).unwrap();
            assert_eq!(layout.receipt().nodes, source_count);
            assert_eq!(layout.receipt().work, work);
        });
    }
}

#[test]
fn source_scratch_retained_and_core_budgets_keep_typed_boundaries() {
    use iced_css_core::formatting::{
        clearance_owner::ChildFacts, clearance_sequence::SequenceNormalPlacement,
    };
    use std::mem::size_of;

    sibling_fixture(6, baseline_css(), |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };

        let mut limits = layout_limits();
        limits.sources = nz(5);
        limits.work = nz(1);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), limits),
            Err(SiblingLayoutError::Sources)
        ));

        limits = layout_limits();
        limits.scratch_bytes = nz(4 * size_of::<ChildFacts<'_>>() - 1);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), limits),
            Err(SiblingLayoutError::ScratchBytes)
        ));

        limits = layout_limits();
        limits.retained_bytes = nz(2 * size_of::<SequenceNormalPlacement>()
            + 4 * size_of::<iced_css::evaluation::sibling_layout::PrincipalChildBox>()
            - 1);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), limits),
            Err(SiblingLayoutError::Bytes)
        ));

        limits = layout_limits();
        limits.core.work = nz(126);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), limits),
            Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
        ));

        let layout = build_captured_sibling_layout(&facts, outer(), layout_limits()).unwrap();
        assert!(
            layout.receipt().scratch_bytes >= layout.children().len() * size_of::<ChildFacts<'_>>()
        );
        assert_eq!(
            layout.receipt().scratch_bytes % size_of::<ChildFacts<'_>>(),
            0
        );
        assert_eq!(
            layout.receipt().retained_bytes,
            layout.receipt().core.retained_bytes
                + layout.receipt().child_capacity
                    * size_of::<iced_css::evaluation::sibling_layout::PrincipalChildBox>()
        );
        assert_eq!(layout.receipt().core.nodes, 6);
        assert_eq!(layout.receipt().core.tail, 2);
        assert_eq!(layout.receipt().core.work, 127);
        assert_eq!(
            layout.receipt().core.retained_bytes,
            layout.receipt().core.record_capacity * size_of::<SequenceNormalPlacement>()
        );
    });
}

#[test]
fn exact_outer_pointer_binding_precedes_facade_prepayment() {
    sibling_fixture(6, baseline_css(), |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let actual = &projection.nodes()[0];
        let foreign = iced_css_core::source::SourceNode {
            instance: actual.instance,
            parent: actual.parent,
            children: actual.children.clone(),
            metadata: Default::default(),
        };
        let mut limits = layout_limits();
        limits.work = nz(1);
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &foreign,
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                limits,
            ),
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::GeometrySource
            ))
        ));
    });
}

#[test]
fn parent_and_child_profile_errors_preserve_causes_and_source_indices() {
    for (suffix, expected_source, expected_reason) in [
        (
            "#b1 { clear:left; }",
            2,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ),
        (
            "#f { clear:both; }",
            3,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ),
        (
            "#b3 { margin-top:-1px; }",
            5,
            OwnerProfileReason::NegativeMargin,
        ),
        (
            "#b3 { padding-left:1px; }",
            5,
            OwnerProfileReason::ChildDecoration,
        ),
    ] {
        let css = format!("{} {suffix}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                    ClearanceOwnerError::Unsupported { source, reason }
                )) if source == expected_source && reason == expected_reason
            ));
        });
    }
}

#[test]
fn unsupported_categories_keep_typed_sources_while_overwide_normals_succeed() {
    use CapturedClearanceUnsupported as Unsupported;

    for (suffix, source, property, reason) in [
        (
            "#b1 { margin-left:auto; }",
            2,
            "margin-left",
            Unsupported::AutoMargin,
        ),
        ("#b3 { height:auto; }", 5, "height", Unsupported::HeightKind),
        (
            "#b3 { overflow:hidden; }",
            5,
            "overflow",
            Unsupported::ChildOverflow,
        ),
        ("#f { width:auto; }", 3, "width", Unsupported::FloatWidth),
    ] {
        let css = format!("{} {suffix}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.0,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                    layout_limits(),
                ),
                Err(SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                    projected,
                    property: actual_property,
                    reason: actual_reason,
                })) if projected == source && actual_property == property && actual_reason == reason
            ));
        });
    }

    for (suffix, source, reason) in [
        (
            "#b1 { margin-left:1px; }",
            2,
            OwnerProfileReason::HorizontalMargin,
        ),
        (
            "#b3 { padding-left:1px; }",
            5,
            OwnerProfileReason::ChildDecoration,
        ),
        (
            "#b1 { height:0; }",
            2,
            OwnerProfileReason::NonpositiveBlockHeight,
        ),
        ("#f { width:101px; }", 3, OwnerProfileReason::FloatTooWide),
    ] {
        let css = format!("{} {suffix}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.0,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                    layout_limits(),
                ),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                    ClearanceOwnerError::Unsupported {
                        source: actual_source,
                        reason: actual_reason,
                    }
                ))) if actual_source == source && actual_reason == reason
            ));
        });
    }

    let overwide_css = format!("{} #b3 {{ width:140px; }}", baseline_css());
    sibling_fixture(6, &overwide_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(layout.children()[3].used_width(), 140.0);
        assert_eq!(layout.children()[3].content().right, 140.0);
        assert_eq!(
            layout.children()[3].normal_width().unwrap().margin_right,
            -40.0
        );
    });

    let generated_css = format!("{} #b3:after {{ content:'late'; }}", baseline_css());
    sibling_fixture(6, &generated_css, |projection, display| {
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Unsupported {
                    original: 3,
                    target: BoxFactTarget::After,
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                }
            ))
        ));
    });
}

#[test]
fn clear_side_selection_and_equality_remain_core_owned() {
    for (suffix, clearance) in [
        ("#f {float:right;} #b2 {clear:left;}", None),
        ("#b2 {clear:none;}", None),
        ("#f {float:right;} #b2 {clear:right;}", Some(-1.0)),
        ("#b2 {clear:left;}", Some(-1.0)),
    ] {
        let css = format!("{} {suffix}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();
            assert_eq!(layout.sequence().tail()[0].clearance(), clearance);
        });
    }

    let css = format!("{} #b2 {{margin-top:6px;}}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let error = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ));
    });
}

#[test]
fn finite_child_endpoint_overflow_and_vertical_error_phase_remain_typed() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (selector, source) in [("#b1", 2), ("#b3", 5)] {
        let css = format!(
            "{} #f {{ width:0; }} {selector} {{ width:{MAX}; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                facts.row(source).unwrap().width,
                BoxLengthFact::Absolute(value) if value.pixels == f32::MAX
            ));

            let finite = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            )
            .unwrap();
            assert_eq!(finite.sequence().tail().len(), 2);
            assert_eq!(finite.children()[source - 2].content().right, f32::MAX);

            assert!(matches!(
                build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: f32::MAX,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                    layout_limits(),
                ),
                Err(SiblingLayoutError::ChildHorizontal {
                    source: actual_source,
                    error: BlockHorizontalError::Unrepresentable {
                        stage: BlockCoordinateStage::ContentRight,
                    },
                }) if actual_source == source
            ));
        });
    }

    let competing_css = format!(
        "{} #f {{ width:0; height:0; }} #b1 {{ width:{MAX}; }} #b2 {{ clear:both; }}",
        baseline_css()
    );
    sibling_fixture(6, &competing_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            facts.row(2).unwrap().width,
            BoxLengthFact::Absolute(value) if value.pixels == f32::MAX
        ));
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: f32::MAX,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::Core(
                ClearanceSequenceError::Placement {
                    source: 4,
                    error: ClearanceOwnerError::UnsupportedEquality,
                }
            ))
        ));
    });
}

#[test]
fn computed_provenance_is_copied_and_dynamic_five_matches_legacy_geometry() {
    let css = "
        #o { width:100px; }
        #p { overflow:hidden; padding-top:1px; padding-bottom:1px; }
        #b1 { height:1px; margin-bottom:4px; }
        #f { float:left; width:10px; height:2px; }
        #b2 { height:1px; margin-top:3px; clear:both; }
    ";
    sibling_fixture(5, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let layout = build_captured_sibling_layout(&facts, outer(), layout_limits()).unwrap();

        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let legacy_facts =
            evaluate_clearance_box_facts(&legacy_projection, display, fact_limits()).unwrap();
        let legacy = build_captured_clearance_with_parent_bottom(
            &legacy_facts,
            ClearanceOuterContentGeometry {
                outer: &legacy_projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
        )
        .unwrap();

        assert_eq!(
            layout.parent().horizontal(),
            legacy.parent().parent().horizontal()
        );
        assert_eq!(
            layout.parent().origin(),
            legacy.parent().parent().parent_origin()
        );
        assert_eq!(layout.parent().height(), legacy.parent().height());
        assert_eq!(layout.parent().bottom(), legacy.bottom());
        assert_eq!(
            layout.children()[0].content().top,
            legacy.parent().parent().owner().before().top
        );
        assert_eq!(
            layout.children()[1].content().bottom,
            legacy.parent().parent().owner().placed_float().bottom
        );
        assert_eq!(
            layout.children()[2].content().bottom,
            legacy.parent().parent().owner().after().bottom
        );
        assert_eq!(layout.children()[2].clear().value, ClearSide::Both);
        assert_eq!(
            layout.children()[2].clear().origin,
            BoxFactOrigin::Specified
        );
        assert_eq!(layout.children()[0].clear().origin, BoxFactOrigin::Initial);
        assert_eq!(layout.parent().clearance(), ClearanceRequirement::Absent);
        assert!(matches!(
            layout.parent().computed_height(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Initial
            }
        ));
        assert_eq!(legacy_projection.receipt().work, 36);
    });

    let fixed_css = format!("{css} #b2 {{ width:40px; }}");
    sibling_fixture(5, &fixed_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let dynamic = build_captured_sibling_layout(&facts, outer(), layout_limits()).unwrap();
        assert_eq!(
            dynamic.children()[2].normal_width(),
            Some(BlockWidthResult {
                width: 40.0,
                margin_left: 0.0,
                margin_right: 60.0,
                passes: 1,
            })
        );

        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let legacy_facts =
            evaluate_clearance_box_facts(&legacy_projection, display, fact_limits()).unwrap();
        assert!(matches!(
            build_captured_clearance_with_parent_bottom(
                &legacy_facts,
                ClearanceOuterContentGeometry {
                    outer: &legacy_projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
            ),
            Err(ParentBottomError::Parent(ParentHeightError::Parent(
                ParentClearanceError::Captured(CapturedClearanceError::Unsupported {
                    projected: 4,
                    property: "width",
                    reason: CapturedClearanceUnsupported::BlockWidth,
                })
            )))
        ));
    });
}

#[test]
fn actual_defaulting_inheritance_and_cascade_provenance_reach_layout_records() {
    let inherited_css = format!(
        "{} #p {{ height:2px; margin-top:5px; margin-bottom:4px; }} \
         #b1 {{ margin-top:0px; }} \
         #b3 {{ height:3px; height:inherit !important; margin-top:inherit; margin-bottom:inherit; }}",
        baseline_css()
    );
    sibling_fixture(6, &inherited_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        let b1 = &layout.children()[0];
        let b3 = &layout.children()[3];
        assert!(matches!(
            b1.margin_top(),
            BoxLengthFact::Absolute(value)
                if value.pixels == 0.0 && value.origin == BoxFactOrigin::Specified
        ));
        for fact in [b3.computed_height(), b3.margin_top(), b3.margin_bottom()] {
            assert!(matches!(
                fact,
                BoxLengthFact::Absolute(value)
                    if value.origin == BoxFactOrigin::Inherited { original_parent: 4 }
            ));
        }
        assert_eq!(b3.used_height(), 2.0);
        assert!(matches!(
            layout.parent().computed_height(),
            BoxLengthFact::Absolute(value)
                if value.pixels == 2.0 && value.origin == BoxFactOrigin::Specified
        ));
    });

    let explicit_auto_css = format!(
        "{} #p {{ height:auto; margin-top:auto; margin-bottom:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &explicit_auto_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            layout.parent().computed_height(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified
            }
        );
        assert_eq!(
            layout.parent().computed_margin_top(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified
            }
        );
        assert_eq!(
            layout.parent().computed_margin_bottom(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified
            }
        );
    });

    let tiny_css = format!(
        "{} #b3 {{ height:0.0000000000000000000000000000000000000000000000000001px; }}",
        baseline_css()
    );
    sibling_fixture(6, &tiny_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let BoxLengthFact::Absolute(tiny) = facts.row(5).unwrap().height else {
            panic!()
        };
        assert_eq!(tiny.pixels, 0.0);
        assert!(tiny.rounded_to_zero);
        assert_eq!(tiny.origin, BoxFactOrigin::Specified);
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 5,
                    reason: OwnerProfileReason::NonpositiveBlockHeight,
                }
            )))
        ));
    });
}
