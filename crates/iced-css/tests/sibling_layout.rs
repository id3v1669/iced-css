#[path = "support/clearance_fixture.rs"]
mod support;

use iced_css::evaluation::sibling_layout::{
    PrincipalChildKind, PrincipalEdges, SiblingLayoutError, SiblingLayoutLimits,
    build_captured_sibling_layout,
};
use iced_css::evaluation::{
    clearance_box_facts::{BoxFactOrigin, BoxLengthFact},
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ClearanceOuterContentGeometry,
        build_captured_clearance_with_parent_bottom,
    },
};
use iced_css_core::formatting::{
    bfc_height::BfcHeightStage,
    clearance::{ClearSide, ClearanceRequirement},
    clearance_owner::{ClearanceOwnerError, OwnerProfileReason},
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
        assert_eq!(layout.receipt().work, 32 + 32 * 6);
    });
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
    let css = format!("{} #b3 {{ width:2px; }}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let work = 32 + 32 * 6;
        for short in [1, work - 1] {
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
                    property: "width",
                    reason: CapturedClearanceUnsupported::BlockWidth,
                }
            ))
        ));
    });
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
