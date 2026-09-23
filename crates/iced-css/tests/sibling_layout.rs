#[path = "support/clearance_fixture.rs"]
mod support;

use iced_css::evaluation::sibling_layout::{
    CapturedSiblingLayout, PrincipalChildKind, PrincipalEdges, SiblingLayoutError,
    SiblingLayoutLimits, build_captured_sibling_layout,
};
use iced_css::evaluation::{
    clearance_box_facts::{
        AbsoluteBoxFact, BoxFactOrigin, BoxFactTarget, BoxFactUnsupported, BoxLengthFact,
        ClearanceBoxFactsError,
    },
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ClearanceOuterContentGeometry,
        ParentBottomError, ParentClearanceError, ParentHeightError,
        build_captured_clearance_with_parent_bottom,
    },
    sibling_box_facts::SiblingBoxFactsError,
};
use iced_css_core::formatting::{
    bfc_height::{BfcHeightPlacement, BfcHeightStage},
    block::{
        BlockCoordinateStage, BlockHorizontalError, BlockHorizontalPlacement, BlockWidthError,
        BlockWidthResult,
    },
    block_bottom::BlockBottomPlacement,
    clearance::{ClearSide, ClearanceRequirement},
    clearance_owner::{
        ClearanceOwnerError, MarginGroupId, MarginMember, MarginSide, OwnerProfileReason,
    },
    clearance_sequence::{ClearanceSequenceError, ClearanceSequenceLimits},
    first_child_origin::{FirstChildOriginError, FirstChildOriginPlacement, OriginCoordinateStage},
    floats::PlacedFloat,
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

fn assert_oversized_float_case(rule: &str, left: f32, right: f32) {
    let css = format!("{} #f {{ width:140px; {rule} }}", baseline_css());
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
        let child = &layout.children()[1];
        assert_eq!(child.source_index(), 3);
        assert_eq!(child.containing_source_index(), 1);
        assert_eq!(child.kind(), PrincipalChildKind::Float);
        assert_eq!(child.normal_width(), None);
        assert_eq!(child.used_width().to_bits(), 140.0_f32.to_bits());
        assert_eq!(child.used_margin_left().to_bits(), 0.0_f32.to_bits());
        assert_eq!(child.used_margin_right().to_bits(), 0.0_f32.to_bits());
        assert_eq!(
            child.content(),
            PrincipalEdges {
                left,
                right,
                top: 16.0,
                bottom: 18.0
            }
        );
        assert_eq!(child.computed_width(), facts.row(3).unwrap().width);
        assert_eq!(
            child.computed_margin_left(),
            facts.row(3).unwrap().margin[3]
        );
        assert_eq!(
            child.computed_margin_right(),
            facts.row(3).unwrap().margin[1]
        );
        assert!(std::ptr::eq(
            layout.source(3).unwrap(),
            &projection.nodes()[3]
        ));
    });
}

#[test]
fn oversized_left_float_keeps_its_explicit_width() {
    assert_oversized_float_case("margin-left:0; margin-right:0;", 0.0, 140.0);
}

#[test]
fn oversized_right_float_with_auto_margins_keeps_its_explicit_width() {
    assert_oversized_float_case(
        "float:right; margin-left:auto; margin-right:auto;",
        -40.0,
        100.0,
    );
}

fn assert_float_auto_case(rule: &str, left: f32, right: f32) {
    let css = format!("{} #f {{ {rule} }}", baseline_css());
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
        let child = &layout.children()[1];
        assert_eq!(child.source_index(), 3);
        assert_eq!(child.containing_source_index(), 1);
        assert_eq!(child.kind(), PrincipalChildKind::Float);
        assert_eq!(child.normal_width(), None);
        assert_eq!(child.used_margin_left().to_bits(), 0.0f32.to_bits());
        assert_eq!(child.used_margin_right().to_bits(), 0.0f32.to_bits());
        assert_eq!(child.used_width(), 10.0);
        assert_eq!(
            child.content(),
            PrincipalEdges {
                left,
                right,
                top: 16.0,
                bottom: 18.0
            }
        );
        assert_eq!(
            child.computed_margin_left(),
            facts.row(3).unwrap().margin[3]
        );
        assert_eq!(
            child.computed_margin_right(),
            facts.row(3).unwrap().margin[1]
        );
        assert!(std::ptr::eq(
            layout.source(3).unwrap(),
            &projection.nodes()[3]
        ));
    });
}

#[test]
fn float_paired_auto_margins_use_zero_in_actual_css() {
    assert_float_auto_case("margin-left:auto; margin-right:auto;", 0.0, 10.0);
}

#[test]
fn right_float_single_auto_margin_stays_flush_right() {
    assert_float_auto_case(
        "float:right; margin-left:auto; margin-right:0;",
        90.0,
        100.0,
    );
}

fn assert_authored_margin_case(rule: &str, expected: BlockWidthResult, left: f32, right: f32) {
    let css = format!(
        "{} #o {{ direction:rtl; }} #p {{ direction:ltr; }} \
         #b3 {{ direction:rtl; {rule} }}",
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
        let child = &layout.children()[3];
        assert_eq!(child.source_index(), 5);
        assert_eq!(child.containing_source_index(), 1);
        let row = facts.row(5).unwrap();
        assert_eq!(child.computed_margin_left(), row.margin[3]);
        assert_eq!(child.computed_margin_right(), row.margin[1]);
        assert_eq!(child.normal_width(), Some(expected));
        assert_eq!(
            child.normal_width().unwrap().width.to_bits(),
            child.used_width().to_bits()
        );
        assert_eq!(
            child.content(),
            PrincipalEdges {
                left,
                right,
                top: 22.0,
                bottom: 23.0,
            }
        );
        assert!(std::ptr::eq(
            layout.source(5).unwrap(),
            &projection.nodes()[5]
        ));
    });
}

#[test]
fn authored_positive_horizontal_margins_reach_actual_css_geometry() {
    assert_authored_margin_case(
        "width:40px; margin-left:10px; margin-right:20px;",
        BlockWidthResult {
            width: 40.0,
            margin_left: 10.0,
            margin_right: 50.0,
            passes: 1,
        },
        10.0,
        50.0,
    );
}

#[test]
fn authored_negative_horizontal_margin_reaches_actual_css_geometry() {
    assert_authored_margin_case(
        "width:auto; margin-left:-10px; margin-right:20px;",
        BlockWidthResult {
            width: 90.0,
            margin_left: -10.0,
            margin_right: 20.0,
            passes: 1,
        },
        -10.0,
        80.0,
    );
}

#[test]
fn authored_paired_auto_horizontal_margins_reach_geometry() {
    assert_authored_margin_case(
        "width:40px; margin-left:auto; margin-right:auto;",
        BlockWidthResult {
            width: 40.0,
            margin_left: 30.0,
            margin_right: 30.0,
            passes: 1,
        },
        30.0,
        70.0,
    );
}

#[test]
fn authored_single_auto_horizontal_margin_reaches_geometry() {
    assert_authored_margin_case(
        "width:40px; margin-left:auto; margin-right:20px;",
        BlockWidthResult {
            width: 40.0,
            margin_left: 40.0,
            margin_right: 20.0,
            passes: 1,
        },
        40.0,
        80.0,
    );
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
            let row = facts.row(source).unwrap();
            assert_eq!(child.computed_margin_left(), row.margin[3]);
            assert_eq!(child.computed_margin_right(), row.margin[1]);
            if kind == PrincipalChildKind::Float {
                assert_eq!(child.normal_width(), None);
                for fact in [child.computed_margin_left(), child.computed_margin_right()] {
                    assert!(matches!(
                        fact,
                        BoxLengthFact::Absolute(value)
                            if value.pixels == 0.0
                                && !value.rounded_to_zero
                                && value.origin == BoxFactOrigin::Initial
                    ));
                }
            } else {
                assert!(child.normal_width().is_some());
                assert_eq!(
                    child.normal_width().unwrap().width.to_bits(),
                    child.used_width().to_bits()
                );
            }
        }
        assert_eq!(layout.sequence().tail()[1].source_index(), 5);
        assert_eq!(layout.receipt().nodes, 6);
        assert_eq!(
            layout.receipt().work,
            32 + 32 * 6 + 19 * (6 - 2) + 64 * (6 - 3)
        );
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

#[derive(Clone, Debug, PartialEq)]
struct MarginGroupObservation {
    id: MarginGroupId,
    members: Vec<MarginMember>,
    positive: f32,
    negative: f32,
}

fn observe_margin_group(
    group: &iced_css_core::formatting::clearance_owner::MarginGroup,
) -> MarginGroupObservation {
    MarginGroupObservation {
        id: group.id(),
        members: group.members().to_vec(),
        positive: group.positive(),
        negative: group.negative(),
    }
}

fn observed_group(
    id: MarginGroupId,
    members: Vec<MarginMember>,
    positive: f32,
) -> MarginGroupObservation {
    MarginGroupObservation {
        id,
        members,
        positive,
        negative: 0.0,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TailVerticalObservation {
    source: usize,
    top: f32,
    bottom: f32,
    hypothetical_top: f32,
    zero_clearance_top: Option<f32>,
    clearance: Option<f32>,
    barrier_source: Option<usize>,
    hypothetical_group: MarginGroupObservation,
    actual_groups: Vec<MarginGroupObservation>,
}

#[derive(Clone, Debug, PartialEq)]
struct PrincipalObservation {
    source: usize,
    containing: usize,
    kind: PrincipalChildKind,
    computed_width: BoxLengthFact,
    computed_left: BoxLengthFact,
    computed_right: BoxLengthFact,
    normal_width: Option<BlockWidthResult>,
    used_margin_left: f32,
    used_margin_right: f32,
    used_width: f32,
    used_height: f32,
    content: PrincipalEdges,
    padding: PrincipalEdges,
    border: PrincipalEdges,
}

#[derive(Clone, Debug, PartialEq)]
struct VerticalObservation {
    children: Vec<(usize, f32, f32)>,
    principals: Vec<PrincipalObservation>,
    before: (f32, f32),
    float: (f32, f32),
    tail: Vec<TailVerticalObservation>,
    parent_source: (usize, usize),
    parent_horizontal: BlockHorizontalPlacement,
    parent_origin: FirstChildOriginPlacement,
    parent_height: BfcHeightPlacement,
    parent_bottom: BlockBottomPlacement,
}

fn observe_layout(layout: &CapturedSiblingLayout<'_>) -> VerticalObservation {
    VerticalObservation {
        children: layout
            .children()
            .iter()
            .map(|child| {
                (
                    child.source_index(),
                    child.content().top,
                    child.content().bottom,
                )
            })
            .collect(),
        principals: layout
            .children()
            .iter()
            .map(|child| {
                let normal_width = child.normal_width();
                match child.kind() {
                    PrincipalChildKind::Normal => {
                        let width = normal_width.unwrap();
                        assert_eq!(
                            child.used_margin_left().to_bits(),
                            width.margin_left.to_bits()
                        );
                        assert_eq!(
                            child.used_margin_right().to_bits(),
                            width.margin_right.to_bits()
                        );
                        assert_eq!(child.used_width().to_bits(), width.width.to_bits());
                    }
                    PrincipalChildKind::Float => {
                        assert_eq!(normal_width, None);
                        assert_eq!(child.used_margin_left().to_bits(), 0.0f32.to_bits());
                        assert_eq!(child.used_margin_right().to_bits(), 0.0f32.to_bits());
                    }
                }
                PrincipalObservation {
                    source: child.source_index(),
                    containing: child.containing_source_index(),
                    kind: child.kind(),
                    computed_width: child.computed_width(),
                    computed_left: child.computed_margin_left(),
                    computed_right: child.computed_margin_right(),
                    normal_width,
                    used_margin_left: child.used_margin_left(),
                    used_margin_right: child.used_margin_right(),
                    used_width: child.used_width(),
                    used_height: child.used_height(),
                    content: child.content(),
                    padding: child.padding(),
                    border: child.border(),
                }
            })
            .collect(),
        before: (
            layout.sequence().before().top,
            layout.sequence().before().bottom,
        ),
        float: (
            layout.sequence().placed_float().top,
            layout.sequence().placed_float().bottom,
        ),
        tail: layout
            .sequence()
            .tail()
            .iter()
            .map(|record| TailVerticalObservation {
                source: record.source_index(),
                top: record.vertical().top,
                bottom: record.vertical().bottom,
                hypothetical_top: record.hypothetical_top(),
                zero_clearance_top: record.zero_clearance_top(),
                clearance: record.clearance(),
                barrier_source: record.barrier_source_index(),
                hypothetical_group: observe_margin_group(record.hypothetical_group()),
                actual_groups: record
                    .actual_groups()
                    .iter()
                    .map(observe_margin_group)
                    .collect(),
            })
            .collect(),
        parent_source: (
            layout.parent().source_index(),
            layout.parent().containing_source_index(),
        ),
        parent_horizontal: layout.parent().horizontal(),
        parent_origin: layout.parent().origin(),
        parent_height: layout.parent().height(),
        parent_bottom: layout.parent().bottom(),
    }
}

fn assert_same_vertical(actual: &VerticalObservation, expected: &VerticalObservation) {
    assert_eq!(actual.children, expected.children);
    assert_eq!(actual.before, expected.before);
    assert_eq!(actual.float, expected.float);
    assert_eq!(actual.tail, expected.tail);
    assert_eq!(actual.parent_source, expected.parent_source);
    assert_eq!(actual.parent_horizontal, expected.parent_horizontal);
    assert_eq!(actual.parent_origin, expected.parent_origin);
    assert_eq!(actual.parent_height, expected.parent_height);
    assert_eq!(actual.parent_bottom, expected.parent_bottom);
}

fn actual_horizontal_observation(
    source_count: usize,
    selector: &str,
    selected_source: usize,
    width_css: &str,
    left_margin: &str,
    right_margin: &str,
    parent_direction: &str,
) -> (
    BlockWidthResult,
    PrincipalEdges,
    BoxLengthFact,
    BoxLengthFact,
    VerticalObservation,
) {
    let opposite = if parent_direction == "ltr" {
        "rtl"
    } else {
        "ltr"
    };
    let b4 = if source_count == 7 {
        "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
    } else {
        ""
    };
    let css = format!(
        "{} {b4} #o {{ direction:{opposite}; }} #p {{ direction:{parent_direction}; }} \
         #f {{ width:140px; }} \
         {selector} {{ direction:{opposite}; width:{width_css}; \
         margin-left:{left_margin}; margin-right:{right_margin}; }}",
        baseline_css()
    );
    let mut observed = None;
    sibling_fixture(source_count, &css, |projection, display| {
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
        let float = &layout.children()[1];
        let initial_zero = BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 0.0,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Initial,
        });
        assert_eq!(float.computed_margin_left(), initial_zero);
        assert_eq!(float.computed_margin_right(), initial_zero);
        assert_eq!(float.normal_width(), None);
        let child = &layout.children()[selected_source - 2];
        assert_eq!(child.source_index(), selected_source);
        assert_eq!(child.containing_source_index(), 1);
        assert!(std::ptr::eq(
            layout.source(selected_source).unwrap(),
            &projection.nodes()[selected_source]
        ));
        let width = child.normal_width().unwrap();
        assert_eq!(width.width.to_bits(), child.used_width().to_bits());
        let computed_left = child.computed_margin_left();
        let computed_right = child.computed_margin_right();
        assert_eq!(computed_left, facts.row(selected_source).unwrap().margin[3]);
        assert_eq!(
            computed_right,
            facts.row(selected_source).unwrap().margin[1]
        );
        let vertical = observe_layout(&layout);
        observed = Some((
            width,
            child.content(),
            computed_left,
            computed_right,
            vertical,
        ));
    });
    observed.unwrap()
}

fn actual_margin_observation(
    source_count: usize,
    selector: &str,
    selected_source: usize,
    width_css: &str,
    left_margin: i32,
    right_margin: i32,
    parent_direction: &str,
) -> (
    BlockWidthResult,
    PrincipalEdges,
    BoxLengthFact,
    BoxLengthFact,
    VerticalObservation,
) {
    actual_horizontal_observation(
        source_count,
        selector,
        selected_source,
        width_css,
        &format!("{left_margin}px"),
        &format!("{right_margin}px"),
        parent_direction,
    )
}

#[derive(Clone, Debug, PartialEq)]
struct FloatObservation {
    computed_width: BoxLengthFact,
    computed_left: BoxLengthFact,
    computed_right: BoxLengthFact,
    used_left: f32,
    used_right: f32,
    normal_width: Option<BlockWidthResult>,
    used_width: f32,
    content: PrincipalEdges,
    placed: PlacedFloat,
    layout: VerticalObservation,
    original_parent: usize,
}

#[allow(clippy::too_many_arguments)]
fn actual_float_observation(
    source_count: usize,
    side: &str,
    width_css: &str,
    height_css: &str,
    left_margin: &str,
    right_margin: &str,
    parent_direction: &str,
    extra_css: &str,
    content_left: f32,
    content_width: f32,
) -> FloatObservation {
    let opposite = if parent_direction == "ltr" {
        "rtl"
    } else {
        "ltr"
    };
    let b4 = if source_count == 7 {
        "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
    } else {
        ""
    };
    let left_declaration = if left_margin.is_empty() {
        String::new()
    } else {
        format!("margin-left:{left_margin};")
    };
    let right_declaration = if right_margin.is_empty() {
        String::new()
    } else {
        format!("margin-right:{right_margin};")
    };
    let css = format!(
        "{} {b4} #o {{ direction:{opposite}; width:{content_width}px; }} \
         #p {{ direction:{parent_direction}; }} #f {{ direction:{opposite}; float:{side}; \
         width:{width_css}; height:{height_css}; {left_declaration} \
         {right_declaration} }} {extra_css}",
        baseline_css()
    );
    let mut observed = None;
    sibling_fixture(source_count, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            facts.row(0).unwrap().width,
            BoxLengthFact::Absolute(value)
                if value.pixels.to_bits() == content_width.to_bits()
                    && value.origin == BoxFactOrigin::Specified
        ));
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left,
                content_width,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        let receipt = layout.receipt();
        assert_eq!(layout.sources().len(), source_count);
        for (source, node) in projection.nodes().iter().enumerate() {
            assert!(std::ptr::eq(layout.source(source).unwrap(), node));
        }
        let child = &layout.children()[1];
        let row = facts.row(3).unwrap();
        let computed_left = child.computed_margin_left();
        let computed_right = child.computed_margin_right();
        assert_eq!(computed_left, row.margin[3]);
        assert_eq!(computed_right, row.margin[1]);
        assert_eq!(child.source_index(), 3);
        assert_eq!(child.containing_source_index(), 1);
        assert_eq!(child.kind(), PrincipalChildKind::Float);
        assert_eq!(child.normal_width(), None);
        for _ in 0..2 {
            assert_eq!(child.used_margin_left().to_bits(), 0.0f32.to_bits());
            assert_eq!(child.used_margin_right().to_bits(), 0.0f32.to_bits());
            assert_eq!(child.normal_width(), None);
        }
        let result = FloatObservation {
            computed_width: child.computed_width(),
            computed_left,
            computed_right,
            used_left: child.used_margin_left(),
            used_right: child.used_margin_right(),
            normal_width: child.normal_width(),
            used_width: child.used_width(),
            content: child.content(),
            placed: layout.sequence().placed_float(),
            layout: observe_layout(&layout),
            original_parent: projection.original_index(1).unwrap(),
        };
        assert_eq!(layout.receipt(), receipt);
        observed = Some(result);
    });
    observed.unwrap()
}

#[test]
fn float_auto_matrix_uses_zero_and_matches_absolute_zero_layouts() {
    for source_count in [5, 6, 7] {
        for side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                for width in [0, 10, 100] {
                    for height in [2, 40] {
                        for (left, right) in [("auto", "0"), ("0", "auto"), ("auto", "auto")] {
                            let auto = actual_float_observation(
                                source_count,
                                side,
                                &format!("{width}px"),
                                &format!("{height}px"),
                                left,
                                right,
                                direction,
                                "",
                                0.0,
                                100.0,
                            );
                            let zero = actual_float_observation(
                                source_count,
                                side,
                                &format!("{width}px"),
                                &format!("{height}px"),
                                "0",
                                "0",
                                direction,
                                "",
                                0.0,
                                100.0,
                            );
                            let expected_x = if side == "left" {
                                (0.0, width as f32)
                            } else {
                                (100.0 - width as f32, 100.0)
                            };
                            assert_eq!(
                                auto.content,
                                PrincipalEdges {
                                    left: expected_x.0,
                                    right: expected_x.1,
                                    top: 16.0,
                                    bottom: 16.0 + height as f32,
                                }
                            );
                            assert_eq!(auto.used_width, width as f32);
                            assert_eq!(auto.used_left.to_bits(), 0.0f32.to_bits());
                            assert_eq!(auto.used_right.to_bits(), 0.0f32.to_bits());
                            assert_eq!(auto.normal_width, None);
                            assert_eq!(auto.content, zero.content);
                            assert_eq!(auto.placed, zero.placed);
                            assert_same_layout_except_float_computed_margins(
                                &auto.layout,
                                &zero.layout,
                            );
                            assert_eq!(auto.original_parent, zero.original_parent);
                            for principal in &auto.layout.principals {
                                if principal.kind == PrincipalChildKind::Normal {
                                    assert_eq!(principal.normal_width.unwrap().width, 100.0);
                                    assert_eq!(
                                        (principal.content.left, principal.content.right),
                                        (0.0, 100.0)
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn assert_only_float_horizontal_changes(oversized: &FloatObservation, control: &FloatObservation) {
    assert_eq!(oversized.layout.children, control.layout.children);
    assert_eq!(oversized.layout.before, control.layout.before);
    assert_eq!(oversized.layout.float, control.layout.float);
    assert_eq!(oversized.layout.tail, control.layout.tail);
    assert_eq!(oversized.layout.parent_source, control.layout.parent_source);
    assert_eq!(
        oversized.layout.parent_horizontal,
        control.layout.parent_horizontal
    );
    assert_eq!(oversized.layout.parent_origin, control.layout.parent_origin);
    assert_eq!(oversized.layout.parent_height, control.layout.parent_height);
    assert_eq!(oversized.layout.parent_bottom, control.layout.parent_bottom);
    assert_eq!(
        oversized
            .layout
            .principals
            .iter()
            .filter(|principal| principal.source != 3)
            .collect::<Vec<_>>(),
        control
            .layout
            .principals
            .iter()
            .filter(|principal| principal.source != 3)
            .collect::<Vec<_>>()
    );

    let actual_float = &oversized.layout.principals[1];
    let control_float = &control.layout.principals[1];
    assert_eq!(actual_float.source, 3);
    assert_eq!(actual_float.containing, 1);
    assert_eq!(actual_float.kind, PrincipalChildKind::Float);
    assert_eq!(actual_float.normal_width, None);
    assert_eq!(actual_float.computed_left, control_float.computed_left);
    assert_eq!(actual_float.computed_right, control_float.computed_right);
    assert_eq!(actual_float.used_margin_left.to_bits(), 0.0f32.to_bits());
    assert_eq!(actual_float.used_margin_right.to_bits(), 0.0f32.to_bits());
    assert_eq!(actual_float.used_height, control_float.used_height);
    assert_eq!(actual_float.content, oversized.content);
    assert_eq!(actual_float.padding, oversized.content);
    assert_eq!(actual_float.border, oversized.content);
}

fn assert_same_layout_except_float_computed_margins(
    actual: &VerticalObservation,
    expected: &VerticalObservation,
) {
    assert_same_vertical(actual, expected);
    assert_eq!(actual.principals.len(), expected.principals.len());
    for (actual, expected) in actual.principals.iter().zip(&expected.principals) {
        assert_eq!(actual.source, expected.source);
        assert_eq!(actual.containing, expected.containing);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.computed_width, expected.computed_width);
        if actual.kind == PrincipalChildKind::Normal {
            assert_eq!(actual.computed_left, expected.computed_left);
            assert_eq!(actual.computed_right, expected.computed_right);
        }
        assert_eq!(actual.normal_width, expected.normal_width);
        assert_eq!(
            actual.used_margin_left.to_bits(),
            expected.used_margin_left.to_bits()
        );
        assert_eq!(
            actual.used_margin_right.to_bits(),
            expected.used_margin_right.to_bits()
        );
        assert_eq!(actual.used_width.to_bits(), expected.used_width.to_bits());
        assert_eq!(actual.used_height.to_bits(), expected.used_height.to_bits());
        assert_eq!(actual.content, expected.content);
        assert_eq!(actual.padding, expected.padding);
        assert_eq!(actual.border, expected.border);
    }
}

#[test]
fn oversized_float_matrix_preserves_full_non_float_layout_and_actual_width() {
    for source_count in [5, 6, 7] {
        for side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                for (left_margin, right_margin) in
                    [("0", "0"), ("auto", "0"), ("0", "auto"), ("auto", "auto")]
                {
                    for width in [101, 140] {
                        for height in [2, 40] {
                            let oversized = actual_float_observation(
                                source_count,
                                side,
                                &format!("{width}px"),
                                &format!("{height}px"),
                                left_margin,
                                right_margin,
                                direction,
                                "",
                                0.0,
                                100.0,
                            );
                            let control = actual_float_observation(
                                source_count,
                                side,
                                "10px",
                                &format!("{height}px"),
                                left_margin,
                                right_margin,
                                direction,
                                "",
                                0.0,
                                100.0,
                            );
                            let expected = if side == "left" {
                                (0.0, width as f32)
                            } else {
                                (100.0 - width as f32, 100.0)
                            };
                            assert_eq!(
                                oversized.content,
                                PrincipalEdges {
                                    left: expected.0,
                                    right: expected.1,
                                    top: 16.0,
                                    bottom: 16.0 + height as f32,
                                }
                            );
                            assert_eq!(oversized.placed.left, expected.0);
                            assert_eq!(oversized.placed.right, expected.1);
                            assert_eq!(oversized.used_width, width as f32);
                            assert_eq!(oversized.normal_width, None);
                            assert_eq!(oversized.used_left.to_bits(), 0.0f32.to_bits());
                            assert_eq!(oversized.used_right.to_bits(), 0.0f32.to_bits());
                            assert_eq!(oversized.original_parent, source_count - 2);
                            assert_eq!(
                                oversized.computed_width,
                                BoxLengthFact::Absolute(AbsoluteBoxFact {
                                    pixels: width as f32,
                                    rounded_to_zero: false,
                                    origin: BoxFactOrigin::Specified,
                                })
                            );
                            assert_only_float_horizontal_changes(&oversized, &control);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn float_auto_and_normal_auto_keep_their_distinct_width_categories() {
    for side in ["left", "right"] {
        let float = actual_float_observation(
            6, side, "10px", "2px", "auto", "auto", "ltr", "", 0.0, 100.0,
        );
        assert_eq!(
            (float.content.left, float.content.right),
            if side == "left" {
                (0.0, 10.0)
            } else {
                (90.0, 100.0)
            }
        );
    }

    for (source_count, selector, source, width, left, right, direction, expected) in [
        (
            6,
            "#b1",
            2,
            "40px",
            "auto",
            "auto",
            "ltr",
            (40.0, 30.0, 30.0, 1, 30.0, 70.0),
        ),
        (
            6,
            "#b2",
            4,
            "40px",
            "-10px",
            "auto",
            "ltr",
            (40.0, -10.0, 70.0, 1, -10.0, 30.0),
        ),
        (
            6,
            "#b3",
            5,
            "120px",
            "auto",
            "auto",
            "ltr",
            (120.0, 0.0, -20.0, 1, 0.0, 120.0),
        ),
        (
            6,
            "#b3",
            5,
            "120px",
            "auto",
            "auto",
            "rtl",
            (120.0, -20.0, 0.0, 1, -20.0, 100.0),
        ),
        (
            7,
            "#b4",
            6,
            "auto",
            "120px",
            "auto",
            "ltr",
            (0.0, 120.0, -20.0, 2, 120.0, 120.0),
        ),
        (
            7,
            "#b4",
            6,
            "auto",
            "120px",
            "auto",
            "rtl",
            (0.0, 100.0, 0.0, 2, 100.0, 100.0),
        ),
    ] {
        let (used, content, _, _, observation) = actual_horizontal_observation(
            source_count,
            selector,
            source,
            width,
            left,
            right,
            direction,
        );
        assert_eq!(
            used,
            BlockWidthResult {
                width: expected.0,
                margin_left: expected.1,
                margin_right: expected.2,
                passes: expected.3,
            }
        );
        assert_eq!((content.left, content.right), (expected.4, expected.5));
        let principal = observation
            .principals
            .iter()
            .find(|principal| principal.source == source)
            .unwrap();
        assert_eq!(principal.normal_width, Some(used));
        assert_eq!(
            principal.used_margin_left.to_bits(),
            used.margin_left.to_bits()
        );
        assert_eq!(
            principal.used_margin_right.to_bits(),
            used.margin_right.to_bits()
        );
    }
}

#[test]
fn float_computed_provenance_remains_distinct_from_used_zero() {
    let specified_auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Specified,
    };
    let absolute = |origin, rounded_to_zero| {
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 0.0,
            rounded_to_zero,
            origin,
        })
    };
    for (left, right, extra, inherited, expected_left, expected_right) in [
        ("auto", "auto", "", false, specified_auto, specified_auto),
        (
            "inherit",
            "inherit",
            "#p { margin-left:auto; margin-right:auto; }",
            true,
            specified_auto,
            specified_auto,
        ),
        (
            "",
            "",
            "",
            false,
            absolute(BoxFactOrigin::Initial, false),
            absolute(BoxFactOrigin::Initial, false),
        ),
        (
            "0",
            "0",
            "",
            false,
            absolute(BoxFactOrigin::Specified, false),
            absolute(BoxFactOrigin::Specified, false),
        ),
        (
            "0.0000000000000000000000000000000000000000000000000001px",
            "-0.0000000000000000000000000000000000000000000000000001px",
            "",
            false,
            absolute(BoxFactOrigin::Specified, true),
            absolute(BoxFactOrigin::Specified, true),
        ),
        (
            "0; margin-left:auto",
            "0",
            "",
            false,
            specified_auto,
            absolute(BoxFactOrigin::Specified, false),
        ),
        (
            "auto; margin-left:0",
            "auto",
            "",
            false,
            absolute(BoxFactOrigin::Specified, false),
            specified_auto,
        ),
        (
            "0",
            "0; margin-right:auto",
            "",
            false,
            absolute(BoxFactOrigin::Specified, false),
            specified_auto,
        ),
        (
            "auto",
            "auto; margin-right:0",
            "",
            false,
            specified_auto,
            absolute(BoxFactOrigin::Specified, false),
        ),
    ] {
        let observed = actual_float_observation(
            6, "left", "10px", "2px", left, right, "ltr", extra, 0.0, 100.0,
        );
        let expected_left = if inherited {
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Inherited {
                    original_parent: observed.original_parent,
                },
            }
        } else {
            expected_left
        };
        let expected_right = if inherited {
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Inherited {
                    original_parent: observed.original_parent,
                },
            }
        } else {
            expected_right
        };
        assert_eq!(observed.computed_left, expected_left);
        assert_eq!(observed.computed_right, expected_right);
        assert_eq!(observed.used_left.to_bits(), 0.0f32.to_bits());
        assert_eq!(observed.used_right.to_bits(), 0.0f32.to_bits());
        assert_eq!(observed.normal_width, None);
    }

    let inherited = actual_float_observation(
        6,
        "left",
        "140px",
        "2px",
        "inherit",
        "inherit",
        "ltr",
        "#p { margin-left:auto; margin-right:auto; }",
        0.0,
        100.0,
    );
    assert_eq!(inherited.original_parent, 4);
    let inherited_auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Inherited { original_parent: 4 },
    };
    assert_eq!(inherited.computed_left, inherited_auto);
    assert_eq!(inherited.computed_right, inherited_auto);
    assert_eq!(inherited.used_left.to_bits(), 0.0f32.to_bits());
    assert_eq!(inherited.used_right.to_bits(), 0.0f32.to_bits());
    assert_eq!(
        inherited.computed_width,
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 140.0,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Specified,
        })
    );
    assert_eq!(inherited.used_width, 140.0);
    assert_eq!(inherited.normal_width, None);
}

#[test]
fn float_auto_uses_published_staged_bands_and_overflow_placement() {
    const LEFT: f32 = 16_777_216.0;
    let auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Specified,
    };
    for source_count in [5, 6, 7] {
        for side in ["left", "right"] {
            for (left_margin, right_margin) in [("auto", "0"), ("0", "auto"), ("auto", "auto")] {
                let staged = actual_float_observation(
                    source_count,
                    side,
                    "1px",
                    "2px",
                    left_margin,
                    right_margin,
                    "ltr",
                    "",
                    LEFT,
                    3.0,
                );
                assert_eq!(staged.used_width, 1.0);
                assert_eq!(staged.used_left.to_bits(), 0.0f32.to_bits());
                assert_eq!(staged.used_right.to_bits(), 0.0f32.to_bits());
                assert_eq!(
                    (staged.content.left, staged.content.right),
                    if side == "left" {
                        (LEFT, LEFT)
                    } else {
                        (LEFT + 4.0, LEFT + 4.0)
                    }
                );
                assert_eq!(staged.layout.float, (16.0, 18.0));
                assert_eq!(
                    (
                        staged.layout.parent_horizontal.content.left,
                        staged.layout.parent_horizontal.content.right,
                        staged.layout.parent_horizontal.used.width,
                    ),
                    (LEFT, LEFT + 4.0, 3.0)
                );
                assert_eq!(staged.computed_left == auto, left_margin == "auto");
                assert_eq!(staged.computed_right == auto, right_margin == "auto");

                let wide_staged = actual_float_observation(
                    source_count,
                    side,
                    "5px",
                    "2px",
                    left_margin,
                    right_margin,
                    "ltr",
                    "",
                    LEFT,
                    3.0,
                );
                assert_eq!(wide_staged.used_width, 5.0);
                assert_eq!(
                    (wide_staged.content.left, wide_staged.content.right),
                    if side == "left" {
                        (LEFT, LEFT + 4.0)
                    } else {
                        (LEFT - 1.0, LEFT + 4.0)
                    }
                );

                let collapsed = actual_float_observation(
                    source_count,
                    side,
                    "0",
                    "2px",
                    left_margin,
                    right_margin,
                    "ltr",
                    "",
                    LEFT,
                    1.0,
                );
                assert_eq!(collapsed.used_width.to_bits(), 0.0f32.to_bits());
                assert_eq!(
                    (collapsed.content.left, collapsed.content.right),
                    (LEFT, LEFT)
                );
                assert_eq!(
                    (
                        collapsed.layout.parent_horizontal.content.left,
                        collapsed.layout.parent_horizontal.content.right,
                        collapsed.layout.parent_horizontal.used.width,
                    ),
                    (LEFT, LEFT, 1.0)
                );

                let b4 = if source_count == 7 {
                    "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
                } else {
                    ""
                };
                let css = format!(
                    "{} {b4} #o {{ direction:rtl; width:1px; }} #p {{ direction:ltr; }} \
                     #f {{ direction:rtl; float:{side}; width:1px; height:2px; \
                     margin-left:{left_margin}; margin-right:{right_margin}; }}",
                    baseline_css()
                );
                sibling_fixture(source_count, &css, |projection, display| {
                    let facts =
                        evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
                            .unwrap();
                    assert_eq!(
                        facts.row(3).unwrap().margin[3] == auto,
                        left_margin == "auto"
                    );
                    assert_eq!(
                        facts.row(3).unwrap().margin[1] == auto,
                        right_margin == "auto"
                    );
                    let layout = build_captured_sibling_layout(
                        &facts,
                        ClearanceOuterContentGeometry {
                            outer: &projection.nodes()[0],
                            content_left: LEFT,
                            content_width: 1.0,
                            content_top: 10.0,
                        },
                        layout_limits(),
                    )
                    .unwrap();
                    assert_eq!(
                        (
                            layout.children()[1].content().left,
                            layout.children()[1].content().right
                        ),
                        if side == "left" {
                            (LEFT, LEFT)
                        } else {
                            (LEFT - 1.0, LEFT)
                        }
                    );
                    assert_eq!(layout.children()[1].used_width(), 1.0);
                });

                for (width_css, width, expected) in [
                    (
                        "1px",
                        1.0,
                        if side == "left" {
                            (0.0, 1.0)
                        } else {
                            (-1.0, 0.0)
                        },
                    ),
                    (
                        "140px",
                        140.0,
                        if side == "left" {
                            (0.0, 140.0)
                        } else {
                            (-140.0, 0.0)
                        },
                    ),
                ] {
                    let zero_band = actual_float_observation(
                        source_count,
                        side,
                        width_css,
                        "2px",
                        left_margin,
                        right_margin,
                        "ltr",
                        "",
                        0.0,
                        0.0,
                    );
                    assert_eq!(zero_band.layout.parent_horizontal.content.left, 0.0);
                    assert_eq!(zero_band.layout.parent_horizontal.content.right, 0.0);
                    assert_eq!(zero_band.layout.parent_horizontal.used.width, 0.0);
                    assert_eq!(zero_band.used_width, width);
                    assert_eq!((zero_band.content.left, zero_band.content.right), expected);
                }
            }
        }
    }
}

#[test]
fn maximum_float_width_keeps_finite_cancellation_and_typed_publication_failures() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (origin, side, expected) in [
        (f32::MAX, "right", (0.0, f32::MAX)),
        (-f32::MAX, "left", (-f32::MAX, 0.0)),
    ] {
        let observed =
            actual_float_observation(6, side, MAX, "2px", "auto", "auto", "ltr", "", origin, 0.0);
        assert_eq!((observed.content.left, observed.content.right), expected);
        assert_eq!(observed.used_width, f32::MAX);
    }

    for (origin, side) in [(f32::MAX, "left"), (-f32::MAX, "right")] {
        let css = format!(
            "{} #o {{ width:0; direction:rtl; }} #p {{ direction:ltr; }} \
             #f {{ width:{MAX}; float:{side}; margin-left:auto; margin-right:auto; \
             direction:rtl; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                facts.row(0).unwrap().width,
                BoxLengthFact::Absolute(value) if value.pixels.to_bits() == 0.0f32.to_bits()
            ));
            assert!(matches!(
                facts.row(3).unwrap().width,
                BoxLengthFact::Absolute(value) if value.pixels.to_bits() == f32::MAX.to_bits()
            ));
            assert!(matches!(
                build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: origin,
                        content_width: 0.0,
                        content_top: 10.0,
                    },
                    layout_limits(),
                ),
                Err(SiblingLayoutError::Core(
                    ClearanceSequenceError::Placement {
                        source: 3,
                        error: ClearanceOwnerError::FloatPlacement(
                            iced_css_core::formatting::floats::FloatError::Unrepresentable
                        ),
                    }
                ))
            ));
        });
    }
}

#[test]
fn oversized_zero_height_keeps_equality_order_and_complete_clear_none_layout() {
    let failing_css = format!(
        "{} #f {{ width:140px; height:0; margin-left:auto; margin-right:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &failing_css, |projection, display| {
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
            Err(SiblingLayoutError::Core(
                ClearanceSequenceError::Placement {
                    source: 4,
                    error: ClearanceOwnerError::UnsupportedEquality,
                }
            ))
        ));
    });

    let tail_none = "#b2, #b3, #b4 { clear:none; }";
    let oversized = actual_float_observation(
        7, "left", "140px", "0", "auto", "auto", "ltr", tail_none, 0.0, 100.0,
    );
    let control = actual_float_observation(
        7, "left", "10px", "0", "auto", "auto", "ltr", tail_none, 0.0, 100.0,
    );
    assert_eq!(oversized.placed.top, oversized.placed.bottom);
    assert_eq!(oversized.layout.tail.len(), 3);
    assert!(
        oversized
            .layout
            .tail
            .iter()
            .all(|tail| tail.clearance.is_none()
                && tail.barrier_source.is_none()
                && tail.actual_groups.len() == 1
                && tail.actual_groups[0].id == MarginGroupId::ActualPair)
    );
    assert_only_float_horizontal_changes(&oversized, &control);
}

#[test]
fn zero_height_equality_precedes_later_principal_horizontal_overflow() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (clear, expected_equality) in [("both", true), ("none", false)] {
        let css = format!(
            "{} #o {{ width:0; }} #f {{ float:right; width:140px; height:0; }} \
             #b2 {{ clear:{clear}; width:{MAX}; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let result = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: f32::MAX,
                    content_width: 0.0,
                    content_top: 10.0,
                },
                layout_limits(),
            );
            if expected_equality {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::Core(
                        ClearanceSequenceError::Placement {
                            source: 4,
                            error: ClearanceOwnerError::UnsupportedEquality,
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::ChildHorizontal {
                        source: 4,
                        error: BlockHorizontalError::Unrepresentable {
                            stage: BlockCoordinateStage::ContentRight,
                        },
                    })
                ));
            }
        });
    }
}

#[test]
fn signed_horizontal_margin_matrix_uses_parent_direction_and_preserves_vertical_layout() {
    let cases = [
        ("auto", 10, 20, "ltr", 70.0, 10.0, 20.0, 10.0, 80.0, 1),
        ("auto", 10, 20, "rtl", 70.0, 10.0, 20.0, 10.0, 80.0, 1),
        ("40px", 10, 20, "ltr", 40.0, 10.0, 50.0, 10.0, 50.0, 1),
        ("40px", 10, 20, "rtl", 40.0, 40.0, 20.0, 40.0, 80.0, 1),
        ("auto", -10, 20, "ltr", 90.0, -10.0, 20.0, -10.0, 80.0, 1),
        ("40px", -10, 20, "ltr", 40.0, -10.0, 70.0, -10.0, 30.0, 1),
        ("40px", -10, 20, "rtl", 40.0, 40.0, 20.0, 40.0, 80.0, 1),
        ("auto", 80, 30, "ltr", 0.0, 80.0, 20.0, 80.0, 80.0, 2),
        ("auto", 80, 30, "rtl", 0.0, 70.0, 30.0, 70.0, 70.0, 2),
        (
            "140px", -10, 20, "ltr", 140.0, -10.0, -30.0, -10.0, 130.0, 1,
        ),
        ("140px", -10, 20, "rtl", 140.0, -60.0, 20.0, -60.0, 80.0, 1),
        ("0px", 10, 20, "rtl", 0.0, 80.0, 20.0, 80.0, 80.0, 1),
    ];
    for (source_count, selector, selected_source) in [(6, "#b1", 2), (6, "#b3", 5), (7, "#b4", 6)] {
        for (width_css, left, right, direction, width, used_left, used_right, x1, x2, passes) in
            cases
        {
            let (used, edges, computed_left, computed_right, vertical) = actual_margin_observation(
                source_count,
                selector,
                selected_source,
                width_css,
                left,
                right,
                direction,
            );
            assert_eq!(
                used,
                BlockWidthResult {
                    width,
                    margin_left: used_left,
                    margin_right: used_right,
                    passes,
                }
            );
            assert_eq!((edges.left, edges.right), (x1, x2));
            assert!(matches!(
                computed_left,
                BoxLengthFact::Absolute(value)
                    if value.pixels == left as f32
                        && !value.rounded_to_zero
                        && value.origin == BoxFactOrigin::Specified
            ));
            assert!(matches!(
                computed_right,
                BoxLengthFact::Absolute(value)
                    if value.pixels == right as f32
                        && !value.rounded_to_zero
                        && value.origin == BoxFactOrigin::Specified
            ));

            let (_, zero_edges, _, _, zero_vertical) = actual_margin_observation(
                source_count,
                selector,
                selected_source,
                width_css,
                0,
                0,
                direction,
            );
            assert_same_vertical(&vertical, &zero_vertical);
            assert_eq!(
                (edges.top, edges.bottom),
                (zero_edges.top, zero_edges.bottom)
            );
            assert_eq!(vertical.before, (11.0, 12.0));
            assert_eq!(vertical.float, (16.0, 18.0));
            let b2_pair = [
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
            ];
            assert_eq!(
                vertical.tail[0].hypothetical_group,
                observed_group(MarginGroupId::HypotheticalPair, b2_pair.to_vec(), 4.0)
            );
            assert_eq!(
                vertical.tail[0].actual_groups,
                vec![
                    observed_group(MarginGroupId::BeforeBarrier, vec![b2_pair[0]], 4.0),
                    observed_group(MarginGroupId::AfterBarrier, vec![b2_pair[1]], 3.0),
                ]
            );
            let b3_pair = [
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
            ];
            assert_eq!(
                vertical.tail[1].hypothetical_group,
                observed_group(MarginGroupId::HypotheticalPair, b3_pair.to_vec(), 3.0)
            );
            assert_eq!(
                vertical.tail[1].actual_groups,
                vec![observed_group(
                    MarginGroupId::ActualPair,
                    b3_pair.to_vec(),
                    3.0,
                )]
            );
            if source_count == 7 {
                let b4_pair = [
                    MarginMember {
                        source_index: 5,
                        side: MarginSide::Bottom,
                        value: 4.0,
                    },
                    MarginMember {
                        source_index: 6,
                        side: MarginSide::Top,
                        value: 1.0,
                    },
                ];
                assert_eq!(
                    vertical.tail[2].hypothetical_group,
                    observed_group(MarginGroupId::HypotheticalPair, b4_pair.to_vec(), 4.0)
                );
                assert_eq!(
                    vertical.tail[2].actual_groups,
                    vec![observed_group(
                        MarginGroupId::ActualPair,
                        b4_pair.to_vec(),
                        4.0,
                    )]
                );
            }
            let expected_y = match selected_source {
                2 => (11.0, 12.0),
                5 => (22.0, 23.0),
                6 => (27.0, 29.0),
                _ => unreachable!(),
            };
            assert_eq!((edges.top, edges.bottom), expected_y);
            let expected_parent = if source_count == 6 {
                (27.0, 16.0, 28.0)
            } else {
                (31.0, 20.0, 32.0)
            };
            assert_eq!(vertical.parent_height.content_bottom, expected_parent.0);
            assert_eq!(vertical.parent_height.used_height, expected_parent.1);
            assert_eq!(vertical.parent_bottom.padding_bottom, expected_parent.2);
        }
    }
}

#[test]
fn auto_horizontal_margins_follow_all_normal_width_branches() {
    let auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Specified,
    };
    let absolute = |pixels| {
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Specified,
        })
    };
    let cases = [
        (
            "40px", "auto", "auto", "ltr", 40.0, 30.0, 30.0, 1, 30.0, 70.0, auto, auto,
        ),
        (
            "40px", "auto", "auto", "rtl", 40.0, 30.0, 30.0, 1, 30.0, 70.0, auto, auto,
        ),
        (
            "40px",
            "auto",
            "20px",
            "ltr",
            40.0,
            40.0,
            20.0,
            1,
            40.0,
            80.0,
            auto,
            absolute(20.0),
        ),
        (
            "40px",
            "auto",
            "20px",
            "rtl",
            40.0,
            40.0,
            20.0,
            1,
            40.0,
            80.0,
            auto,
            absolute(20.0),
        ),
        (
            "40px",
            "10px",
            "auto",
            "ltr",
            40.0,
            10.0,
            50.0,
            1,
            10.0,
            50.0,
            absolute(10.0),
            auto,
        ),
        (
            "40px",
            "10px",
            "auto",
            "rtl",
            40.0,
            10.0,
            50.0,
            1,
            10.0,
            50.0,
            absolute(10.0),
            auto,
        ),
        (
            "40px",
            "auto",
            "-20px",
            "ltr",
            40.0,
            80.0,
            -20.0,
            1,
            80.0,
            120.0,
            auto,
            absolute(-20.0),
        ),
        (
            "40px",
            "auto",
            "-20px",
            "rtl",
            40.0,
            80.0,
            -20.0,
            1,
            80.0,
            120.0,
            auto,
            absolute(-20.0),
        ),
        (
            "120px", "auto", "auto", "ltr", 120.0, 0.0, -20.0, 1, 0.0, 120.0, auto, auto,
        ),
        (
            "120px", "auto", "auto", "rtl", 120.0, -20.0, 0.0, 1, -20.0, 100.0, auto, auto,
        ),
        (
            "90px",
            "auto",
            "20px",
            "ltr",
            90.0,
            0.0,
            10.0,
            1,
            0.0,
            90.0,
            auto,
            absolute(20.0),
        ),
        (
            "90px",
            "auto",
            "20px",
            "rtl",
            90.0,
            -10.0,
            20.0,
            1,
            -10.0,
            80.0,
            auto,
            absolute(20.0),
        ),
        (
            "auto",
            "auto",
            "20px",
            "ltr",
            80.0,
            0.0,
            20.0,
            1,
            0.0,
            80.0,
            auto,
            absolute(20.0),
        ),
        (
            "auto",
            "auto",
            "20px",
            "rtl",
            80.0,
            0.0,
            20.0,
            1,
            0.0,
            80.0,
            auto,
            absolute(20.0),
        ),
        (
            "auto",
            "-20px",
            "auto",
            "ltr",
            120.0,
            -20.0,
            0.0,
            1,
            -20.0,
            100.0,
            absolute(-20.0),
            auto,
        ),
        (
            "auto",
            "-20px",
            "auto",
            "rtl",
            120.0,
            -20.0,
            0.0,
            1,
            -20.0,
            100.0,
            absolute(-20.0),
            auto,
        ),
        (
            "auto",
            "auto",
            "120px",
            "ltr",
            0.0,
            0.0,
            100.0,
            2,
            0.0,
            0.0,
            auto,
            absolute(120.0),
        ),
        (
            "auto",
            "auto",
            "120px",
            "rtl",
            0.0,
            -20.0,
            120.0,
            2,
            -20.0,
            -20.0,
            auto,
            absolute(120.0),
        ),
        (
            "auto",
            "120px",
            "auto",
            "ltr",
            0.0,
            120.0,
            -20.0,
            2,
            120.0,
            120.0,
            absolute(120.0),
            auto,
        ),
        (
            "auto",
            "120px",
            "auto",
            "rtl",
            0.0,
            100.0,
            0.0,
            2,
            100.0,
            100.0,
            absolute(120.0),
            auto,
        ),
    ];

    for (source_count, selector, selected_source, top, bottom) in [
        (6, "#b1", 2, 11.0, 12.0),
        (6, "#b3", 5, 22.0, 23.0),
        (7, "#b4", 6, 27.0, 29.0),
    ] {
        for (
            width_css,
            left_css,
            right_css,
            direction,
            width,
            margin_left,
            margin_right,
            passes,
            content_left,
            content_right,
            computed_left,
            computed_right,
        ) in cases
        {
            let (used, edges, actual_left, actual_right, vertical) = actual_horizontal_observation(
                source_count,
                selector,
                selected_source,
                width_css,
                left_css,
                right_css,
                direction,
            );
            assert_eq!(
                used,
                BlockWidthResult {
                    width,
                    margin_left,
                    margin_right,
                    passes,
                }
            );
            assert_eq!(
                edges,
                PrincipalEdges {
                    left: content_left,
                    right: content_right,
                    top,
                    bottom,
                }
            );
            assert_eq!(actual_left, computed_left);
            assert_eq!(actual_right, computed_right);

            let (_, _, zero_left, zero_right, zero_vertical) = actual_horizontal_observation(
                source_count,
                selector,
                selected_source,
                width_css,
                "0px",
                "0px",
                direction,
            );
            assert_eq!(zero_left, absolute(0.0));
            assert_eq!(zero_right, absolute(0.0));
            assert_same_vertical(&vertical, &zero_vertical);
        }
    }
}

#[test]
fn vertical_observation_equality_detects_margin_group_member_changes() {
    let (_, _, _, _, baseline) = actual_margin_observation(6, "#b3", 5, "40px", 10, 20, "ltr");
    let mut changed = baseline.clone();
    let original = changed.tail[0].hypothetical_group.members[0].value;
    changed.tail[0].hypothetical_group.members[0].value = original + 1.0;
    assert_ne!(changed, baseline);
    changed.tail[0].hypothetical_group.members[0].value = original;
    assert_eq!(changed, baseline);
}

#[test]
fn fixed_width_preserves_full_vertical_sequence_and_parent_equation() {
    for (width_rule, expected_width, expected_margins, expected_edges) in [
        ("", 95.0, (0.0, 0.0), (2.0, 97.0)),
        ("width:40px;", 40.0, (55.0, 0.0), (57.0, 97.0)),
        (
            "width:40px; margin-left:10px; margin-right:20px;",
            40.0,
            (35.0, 20.0),
            (37.0, 77.0),
        ),
    ] {
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
            assert_eq!(
                (
                    layout.children()[3].content().left,
                    layout.children()[3].content().right,
                ),
                expected_edges
            );
            assert_eq!(layout.children()[3].content().top, 36.0);
            assert_eq!(layout.children()[3].content().bottom, 37.0);

            assert_eq!(
                layout.children()[3].normal_width(),
                Some(BlockWidthResult {
                    width: expected_width,
                    margin_left: expected_margins.0,
                    margin_right: expected_margins.1,
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
fn horizontal_margin_provenance_is_copied_from_actual_css() {
    for (rule, expected_pixels, expected_origin, rounded_to_zero) in [
        ("", 0.0, BoxFactOrigin::Initial, false),
        ("margin-left:0;", 0.0, BoxFactOrigin::Specified, false),
        (
            "margin-left:1px; margin-left:-10px;",
            -10.0,
            BoxFactOrigin::Specified,
            false,
        ),
        (
            "margin-left:-0.0000000000000000000000000000000000000000000000000001px;",
            0.0,
            BoxFactOrigin::Specified,
            true,
        ),
    ] {
        let css = format!("{} #b3 {{ {rule} }}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let expected = facts.row(5).unwrap().margin[3];
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
            let copied = layout.children()[3].computed_margin_left();
            drop(layout);
            assert_eq!(copied, expected);
            assert!(matches!(
                copied,
                BoxLengthFact::Absolute(value)
                    if value.pixels == expected_pixels
                        && value.origin == expected_origin
                        && value.rounded_to_zero == rounded_to_zero
            ));
        });
    }

    let inherited_css = format!(
        "{} #p {{ margin-left:-10px; margin-right:20px; }} \
         #b3 {{ margin-left:inherit; margin-right:inherit; }}",
        baseline_css()
    );
    sibling_fixture(6, &inherited_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let expected = (
            facts.row(5).unwrap().margin[3],
            facts.row(5).unwrap().margin[1],
        );
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
            layout.parent().horizontal().used,
            BlockWidthResult {
                width: 90.0,
                margin_left: -10.0,
                margin_right: 20.0,
                passes: 1,
            }
        );
        let b3 = &layout.children()[3];
        assert_eq!(
            b3.normal_width(),
            Some(BlockWidthResult {
                width: 80.0,
                margin_left: -10.0,
                margin_right: 20.0,
                passes: 1,
            })
        );
        assert_eq!((b3.content().left, b3.content().right), (-20.0, 60.0));
        let copied = (b3.computed_margin_left(), b3.computed_margin_right());
        drop(layout);
        assert_eq!(copied, expected);
        for fact in [copied.0, copied.1] {
            assert!(matches!(
                fact,
                BoxLengthFact::Absolute(value)
                    if !value.rounded_to_zero
                        && value.origin == BoxFactOrigin::Inherited { original_parent: 4 }
            ));
        }
    });
}

#[test]
fn auto_margin_provenance_survives_used_zero_and_inheritance() {
    for (direction, width_css, expected) in [
        (
            "ltr",
            "40px",
            BlockWidthResult {
                width: 40.0,
                margin_left: 30.0,
                margin_right: 30.0,
                passes: 1,
            },
        ),
        (
            "rtl",
            "40px",
            BlockWidthResult {
                width: 40.0,
                margin_left: 30.0,
                margin_right: 30.0,
                passes: 1,
            },
        ),
        (
            "ltr",
            "auto",
            BlockWidthResult {
                width: 100.0,
                margin_left: 0.0,
                margin_right: 0.0,
                passes: 1,
            },
        ),
        (
            "rtl",
            "auto",
            BlockWidthResult {
                width: 100.0,
                margin_left: 0.0,
                margin_right: 0.0,
                passes: 1,
            },
        ),
        (
            "ltr",
            "120px",
            BlockWidthResult {
                width: 120.0,
                margin_left: 0.0,
                margin_right: -20.0,
                passes: 1,
            },
        ),
        (
            "rtl",
            "120px",
            BlockWidthResult {
                width: 120.0,
                margin_left: -20.0,
                margin_right: 0.0,
                passes: 1,
            },
        ),
    ] {
        let css = format!(
            "{} #p {{ direction:{direction}; margin-left:auto; margin-right:auto; }} \
             #b3 {{ width:{width_css}; margin-left:inherit; margin-right:inherit; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let original_parent = projection.original_index(1).unwrap();
            assert_eq!(original_parent, 4);
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
            assert_eq!(b3.normal_width(), Some(expected));
            assert_eq!(
                b3.computed_margin_left(),
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Inherited { original_parent },
                }
            );
            assert_eq!(
                b3.computed_margin_right(),
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Inherited { original_parent },
                }
            );
        });
    }

    for (rule, expected_fact, expected_left) in [
        (
            "margin-left:10px; margin-left:auto;",
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified,
            },
            60.0,
        ),
        (
            "margin-left:auto; margin-left:10px;",
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 10.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Specified,
            }),
            10.0,
        ),
    ] {
        let css = format!("{} #b3 {{ width:40px; {rule} }}", baseline_css());
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
            let b3 = &layout.children()[3];
            assert_eq!(b3.computed_margin_left(), expected_fact);
            assert_eq!(b3.normal_width().unwrap().margin_left, expected_left);
        });
    }

    let controls = format!(
        "{} #p {{ margin-left:0; }} #b2 {{ margin-left:0; }} \
         #b3 {{ margin-left:inherit; \
         margin-right:-0.0000000000000000000000000000000000000000000000000001px; }}",
        baseline_css()
    );
    sibling_fixture(6, &controls, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let original_parent = projection.original_index(1).unwrap();
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
            layout.children()[0].computed_margin_left(),
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 0.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Initial,
            })
        ));
        assert!(matches!(
            layout.children()[2].computed_margin_left(),
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 0.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Specified,
            })
        ));
        assert_eq!(
            layout.children()[3].computed_margin_left(),
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 0.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Inherited { original_parent },
            })
        );
        assert!(matches!(
            layout.children()[3].computed_margin_right(),
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 0.0,
                rounded_to_zero: true,
                origin: BoxFactOrigin::Specified,
            })
        ));
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

    let css = format!(
        "{} #f {{ width:0; }} #p {{ direction:ltr; }} \
         #b3 {{ width:1px; margin-left:1px; margin-right:0; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = build_captured_sibling_layout(
            &facts,
            ClearanceOuterContentGeometry {
                outer: &projection.nodes()[0],
                content_left: 16_777_216.0,
                content_width: 100.0,
                content_top: 10.0,
            },
            layout_limits(),
        )
        .unwrap();
        let b3 = &layout.children()[3];
        assert_eq!(
            b3.normal_width(),
            Some(BlockWidthResult {
                width: 1.0,
                margin_left: 1.0,
                margin_right: 98.0,
                passes: 1,
            })
        );
        assert_eq!(
            (b3.content().left, b3.content().right),
            (16_777_216.0, 16_777_216.0)
        );
        assert_ne!(b3.content().right - b3.content().left, b3.used_width());
        let exact_combined = 16_777_218.0_f64;
        assert_ne!(f64::from(b3.content().right), exact_combined);
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
    let css = format!(
        "{} #f {{ width:140px; margin-left:auto; margin-right:auto; }} #b1 {{ margin-left:auto; }} \
         #b3 {{ margin-right:auto; overflow:hidden; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let work = 492;
        for short in [1, 491] {
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
        (
            5,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b1 { margin-left:auto; }",
            377,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            607,
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
            let children = source_count - 2;
            let normals = source_count - 3;
            assert_eq!(work, 32 + 32 * source_count + 19 * children + 64 * normals);
        });
    }
}

#[test]
fn fitting_and_oversized_widths_have_identical_resource_receipts() {
    for (source_count, facade_work, core_work) in [(5, 377, 213), (6, 492, 275), (7, 607, 338)] {
        let mut receipts = Vec::new();
        for width in [10, 140] {
            let b4 = if source_count == 7 {
                "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
            } else {
                ""
            };
            let css = format!(
                "{} {b4} #f {{ width:{width}px; margin-left:auto; margin-right:auto; }}",
                baseline_css()
            );
            sibling_fixture(source_count, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let mut limits = layout_limits();
                limits.work = nz(facade_work);
                limits.core.work = nz(core_work);
                let layout = build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.0,
                        content_width: 100.0,
                        content_top: 10.0,
                    },
                    limits,
                )
                .unwrap();
                receipts.push(layout.receipt());
            });
        }
        assert_eq!(receipts[0], receipts[1]);
        assert_eq!(receipts[0].work, facade_work);
        assert_eq!(receipts[0].core.work, core_work);
    }
}

#[test]
fn source_scratch_retained_and_core_budgets_keep_typed_boundaries() {
    use iced_css_core::formatting::{
        clearance_owner::ChildFacts, clearance_sequence::SequenceNormalPlacement,
    };
    use std::mem::size_of;

    for (source_count, suffix, facade_work, core_work, old_core_work) in [
        (
            5,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b1 { margin-left:auto; }",
            377,
            213,
            197,
        ),
        (
            6,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b3 { margin-left:auto; }",
            492,
            275,
            259,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            607,
            338,
            322,
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

            let mut limits = layout_limits();
            limits.sources = nz(source_count - 1);
            limits.work = nz(1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Sources)
            ));

            limits = layout_limits();
            limits.work = nz(facade_work - 1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Work)
            ));

            limits = layout_limits();
            let child_count = source_count - 2;
            let tail_count = source_count - 4;
            limits.scratch_bytes = nz(child_count * size_of::<ChildFacts<'_>>() - 1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::ScratchBytes)
            ));

            limits = layout_limits();
            limits.retained_bytes = nz(tail_count * size_of::<SequenceNormalPlacement>()
                + child_count
                    * size_of::<iced_css::evaluation::sibling_layout::PrincipalChildBox>()
                - 1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Bytes)
            ));

            limits = layout_limits();
            limits.core.work = nz(core_work - 1);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ));

            limits = layout_limits();
            limits.core.work = nz(old_core_work);
            assert!(matches!(
                build_captured_sibling_layout(&facts, outer(), limits),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ));

            limits = layout_limits();
            limits.work = nz(facade_work);
            limits.core.work = nz(core_work);
            let layout = build_captured_sibling_layout(&facts, outer(), limits).unwrap();
            assert_eq!(layout.receipt().nodes, source_count);
            assert_eq!(layout.receipt().work, facade_work);
            assert!(layout.receipt().scratch_bytes >= child_count * size_of::<ChildFacts<'_>>());
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
            assert_eq!(layout.receipt().core.nodes, source_count);
            assert_eq!(layout.receipt().core.tail, tail_count);
            assert_eq!(layout.receipt().core.work, core_work);
            assert!(layout.receipt().child_capacity >= child_count);
            assert!(layout.receipt().core.record_capacity >= tail_count);
            assert_eq!(
                layout.receipt().core.retained_bytes,
                layout.receipt().core.record_capacity * size_of::<SequenceNormalPlacement>()
            );
        });
    }
}

#[test]
fn exact_outer_pointer_binding_precedes_facade_prepayment() {
    let css = format!(
        "{} #f {{ width:140px; margin-left:auto; margin-right:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
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

        let mut limits = layout_limits();
        limits.sources = nz(5);
        limits.work = nz(1);
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: actual,
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                limits,
            ),
            Err(SiblingLayoutError::Sources)
        ));
    });
}

#[test]
fn auto_multi_faults_preserve_phase_dominance() {
    use iced_css_core::formatting::clearance_owner::ChildFacts;
    use std::mem::size_of;

    const MAX: &str = "340282346638528859811704183484516925440px";
    let scratch_css = format!("{} #f {{ width:140px; height:auto; }}", baseline_css());
    sibling_fixture(6, &scratch_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.0,
            content_width: 100.0,
            content_top: 10.0,
        };
        let mut scratch_short = layout_limits();
        scratch_short.scratch_bytes = nz(4 * size_of::<ChildFacts<'_>>() - 1);
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), scratch_short),
            Err(SiblingLayoutError::ScratchBytes)
        ));
        assert!(matches!(
            build_captured_sibling_layout(&facts, outer(), layout_limits()),
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "height",
                    reason: CapturedClearanceUnsupported::HeightKind,
                }
            ))
        ));
    });

    let conversion_css = format!(
        "{} #f {{ width:140px; margin-left:auto; margin-right:auto; }} \
         #p {{ margin-top:{MAX}; }} #b2 {{ margin-bottom:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &conversion_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let mut scratch_short = layout_limits();
        scratch_short.scratch_bytes = nz(4 * size_of::<ChildFacts<'_>>() - 1);
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: f32::MAX,
                },
                scratch_short,
            ),
            Err(SiblingLayoutError::ScratchBytes)
        ));
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: f32::MAX,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 4,
                    property: "margin-bottom",
                    reason: CapturedClearanceUnsupported::AutoMargin,
                }
            ))
        ));
    });

    let conversion_control_css = format!(
        "{} #f {{ width:140px; margin-left:auto; margin-right:auto; }} \
         #p {{ margin-top:{MAX}; }} #b1 {{ margin-left:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &conversion_control_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: f32::MAX,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::ParentOrigin(
                FirstChildOriginError::Unrepresentable {
                    stage: OriginCoordinateStage::BorderTop,
                }
            ))
        ));
    });

    let horizontal_css = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} \
         #p {{ width:{MAX}; margin-left:{MAX}; margin-right:auto; }} \
         #b1 {{ margin-left:auto; }} #b3 {{ padding-left:1px; }}",
        baseline_css()
    );
    sibling_fixture(6, &horizontal_css, |projection, display| {
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
            Err(SiblingLayoutError::ParentHorizontal(_))
        ));
    });

    let origin_css = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} \
         #p {{ margin-top:{MAX}; }} #b1 {{ margin-left:auto; }} \
         #b3 {{ padding-left:1px; }}",
        baseline_css()
    );
    sibling_fixture(6, &origin_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: f32::MAX,
                },
                layout_limits(),
            ),
            Err(SiblingLayoutError::ParentOrigin(_))
        ));
    });
}

#[test]
fn dynamic_child_conversion_precedes_float_width_extraction() {
    let invalid_later = format!(
        "{} #f {{ width:auto; }} #b3 {{ overflow:hidden; }}",
        baseline_css()
    );
    sibling_fixture(6, &invalid_later, |projection, display| {
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
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 5,
                    property: "overflow",
                    reason: CapturedClearanceUnsupported::ChildOverflow,
                }
            ))
        ));
    });

    let width_only = format!("{} #f {{ width:auto; }}", baseline_css());
    sibling_fixture(6, &width_only, |projection, display| {
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
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "width",
                    reason: CapturedClearanceUnsupported::FloatWidth,
                }
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
        let css = format!(
            "{} #f {{ margin-left:auto; margin-right:auto; }} {suffix}",
            baseline_css()
        );
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
            "#b1 { margin-top:auto; }",
            2,
            "margin-top",
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
        let css = format!(
            "{} #f {{ margin-left:auto; margin-right:auto; }} {suffix}",
            baseline_css()
        );
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
            "#f { width:101px; margin-left:1px; }",
            3,
            OwnerProfileReason::FloatMargin,
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
    ] {
        let css = format!(
            "{} #f {{ margin-left:auto; margin-right:auto; }} {suffix}",
            baseline_css()
        );
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
fn remaining_auto_margin_errors_keep_source_and_edge_order() {
    use CapturedClearanceUnsupported as U;

    for (selector, projected, margins, property) in [
        (
            "#b1",
            2,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            "margin-bottom",
        ),
        (
            "#b3",
            5,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            "margin-bottom",
        ),
        (
            "#b1",
            2,
            "margin-top:auto; margin-right:auto; margin-bottom:auto; margin-left:auto;",
            "margin-top",
        ),
        (
            "#b3",
            5,
            "margin-top:auto; margin-right:auto; margin-bottom:auto; margin-left:auto;",
            "margin-top",
        ),
        (
            "#f",
            3,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            "margin-bottom",
        ),
        (
            "#f",
            3,
            "margin-top:auto; margin-right:auto; margin-left:auto;",
            "margin-top",
        ),
    ] {
        let css = format!("{} {selector} {{ {margins} }}", baseline_css());
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
                    projected: actual_projected,
                    property: actual_property,
                    reason: U::AutoMargin,
                })) if actual_projected == projected && actual_property == property
            ));
        });
    }

    for suffix in [
        "#b1 { margin-bottom:auto; } #b3 { height:auto; }",
        "#b1 { margin-bottom:auto; } #b3 { padding-left:1px; }",
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
                Err(SiblingLayoutError::Captured(
                    CapturedClearanceError::Unsupported {
                        projected: 2,
                        property: "margin-bottom",
                        reason: U::AutoMargin,
                    }
                ))
            ));
        });
    }

    let same_child = format!(
        "{} #b1 {{ height:auto; margin-top:auto; margin-bottom:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &same_child, |projection, display| {
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
            Err(SiblingLayoutError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 2,
                    property: "height",
                    reason: U::HeightKind,
                }
            ))
        ));
    });

    for (rule, property, reason) in [
        (
            "height:2px; width:auto; margin-left:auto; margin-right:auto;",
            "width",
            U::FloatWidth,
        ),
        (
            "height:auto; width:auto; margin-left:auto; margin-right:auto;",
            "height",
            U::HeightKind,
        ),
    ] {
        let css = format!("{} #f {{ {rule} }}", baseline_css());
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
                    projected: 3,
                    property: actual_property,
                    reason: actual_reason,
                })) if actual_property == property && actual_reason == reason
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
        let css = format!(
            "{} #f {{ margin-left:auto; margin-right:auto; }} {suffix}",
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
            assert_eq!(layout.sequence().tail()[0].clearance(), clearance);
        });
    }

    let css = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} #b2 {{margin-top:6px;}}",
        baseline_css()
    );
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

    for (tail_rule, succeeds) in [("clear:both", false), ("clear:none", true)] {
        let css = format!(
            "{} #f {{ height:0; margin-left:auto; margin-right:auto; }} \
             #b2 {{ {tail_rule}; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let result = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.0,
                    content_width: 100.0,
                    content_top: 10.0,
                },
                layout_limits(),
            );
            if succeeds {
                let layout = result.unwrap();
                assert_eq!(layout.sequence().placed_float().bottom, 16.0);
                assert_eq!(layout.sequence().tail()[0].hypothetical_top(), 16.0);
            } else {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::Core(
                        ClearanceSequenceError::Placement {
                            source: 4,
                            error: ClearanceOwnerError::UnsupportedEquality,
                        }
                    ))
                ));
            }
        });
    }
}

#[test]
fn finite_child_endpoint_overflow_and_vertical_error_phase_remain_typed() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (selector, source) in [("#b1", 2), ("#b3", 5)] {
        for rule in [
            format!("width:auto; margin-left:-{MAX}; margin-right:-{MAX};"),
            format!("width:{MAX}; margin-left:{MAX}; margin-right:0;"),
        ] {
            let css = format!(
                "{} #f {{ width:0; }} {selector} {{ {rule} }}",
                baseline_css()
            );
            sibling_fixture(6, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let row = facts.row(source).unwrap();
                assert!(matches!(
                    row.margin[3],
                    BoxLengthFact::Absolute(value)
                        if value.pixels.abs() == f32::MAX && value.pixels.is_finite()
                ));
                if rule.starts_with("width:auto") {
                    assert!(matches!(
                        row.margin[1],
                        BoxLengthFact::Absolute(value) if value.pixels == -f32::MAX
                    ));
                } else {
                    assert!(matches!(
                        row.width,
                        BoxLengthFact::Absolute(value) if value.pixels == f32::MAX
                    ));
                }
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
                    Err(SiblingLayoutError::ChildHorizontal {
                        source: actual_source,
                        error: BlockHorizontalError::Width(BlockWidthError::Unrepresentable),
                    }) if actual_source == source
                ));
            });
        }

        let css = format!(
            "{} #f {{ width:0; }} {selector} {{ width:0; margin-left:{MAX}; margin-right:0; }}",
            baseline_css()
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                facts.row(source).unwrap().margin[3],
                BoxLengthFact::Absolute(value)
                    if value.pixels == f32::MAX && value.pixels.is_finite()
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
                Err(SiblingLayoutError::ChildHorizontal {
                    source: actual_source,
                    error: BlockHorizontalError::Unrepresentable {
                        stage: BlockCoordinateStage::ContentLeft,
                    },
                }) if actual_source == source
            ));
        });
    }

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
        "{} #f {{ width:0; height:0; margin-left:auto; margin-right:auto; }} \
         #b1 {{ width:auto; margin-left:-{MAX}; margin-right:-{MAX}; }} \
         #b2 {{ clear:both; }} #b3 {{ margin-left:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &competing_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            facts.row(2).unwrap().width,
            BoxLengthFact::Auto { .. }
        ));
        for fact in [
            facts.row(2).unwrap().margin[3],
            facts.row(2).unwrap().margin[1],
        ] {
            assert!(matches!(
                fact,
                BoxLengthFact::Absolute(value) if value.pixels == -f32::MAX
            ));
        }
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

    for (margin, expected_width, expected_left, expected_reason) in [
        (1.0, 99.0, 1.0, OwnerProfileReason::HorizontalMargin),
        (-1.0, 101.0, -1.0, OwnerProfileReason::NegativeMargin),
    ] {
        let signed_css = format!("{css} #b2 {{ margin-left:{margin}px; }}");
        sibling_fixture(5, &signed_css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert!(matches!(
                facts.row(4).unwrap().width,
                BoxLengthFact::Auto { .. }
            ));
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
                    width: expected_width,
                    margin_left: margin,
                    margin_right: 0.0,
                    passes: 1,
                })
            );
            assert_eq!(
                (
                    dynamic.children()[2].content().left,
                    dynamic.children()[2].content().right,
                ),
                (expected_left, 100.0)
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
            assert_eq!(legacy_projection.receipt().work, 36);
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
                    ParentClearanceError::Captured(CapturedClearanceError::Core(
                        ClearanceOwnerError::Unsupported { source: 4, reason }
                    ))
                ))) if reason == expected_reason
            ));
        });
    }
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
