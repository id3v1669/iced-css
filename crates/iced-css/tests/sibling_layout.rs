#[path = "support/clearance_fixture.rs"]
mod support;

use iced_css::evaluation::sibling_layout::{
    CapturedSiblingLayout, PrincipalChildKind, PrincipalEdges, SiblingLayoutError,
    SiblingLayoutLimits, SiblingLayoutReceipt, build_captured_sibling_layout,
};
use iced_css::evaluation::{
    clearance_box_facts::{
        AbsoluteBoxFact, BoxFactOrigin, BoxFactTarget, BoxFactUnsupported, BoxLengthFact,
        ClearanceBoxFactsError, KeywordBoxFact,
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
    block_bottom::{BlockBottomError, BlockBottomPlacement, BlockBottomStage},
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

fn assert_vertical_auto_case(
    rule: &str,
    expected_bottom: f32,
    expected_last_used_bottom: f32,
    expected_parent: (f32, f32, f32),
    auto_source: usize,
) {
    let css = format!("{} {rule}", baseline_css());
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
        let children = layout.children();
        let float = &children[1];
        let last = children.last().unwrap();
        assert_eq!(last.content().bottom, expected_bottom);
        assert_eq!(float.used_margin_top().to_bits(), 0.0_f32.to_bits());
        assert_eq!(float.used_margin_bottom().to_bits(), 0.0_f32.to_bits());
        assert_eq!(last.used_margin_top(), 2.0);
        assert_eq!(
            last.used_margin_bottom().to_bits(),
            expected_last_used_bottom.to_bits()
        );
        assert_eq!(layout.parent().height().content_bottom, expected_parent.0);
        assert_eq!(layout.parent().height().used_height, expected_parent.1);
        assert_eq!(layout.parent().bottom().padding_bottom, expected_parent.2);
        if auto_source == 3 {
            assert!(matches!(float.margin_top(), BoxLengthFact::Auto { .. }));
            assert!(matches!(float.margin_bottom(), BoxLengthFact::Auto { .. }));
            assert_eq!(float.margin_top(), facts.row(3).unwrap().margin[0]);
            assert_eq!(float.margin_bottom(), facts.row(3).unwrap().margin[2]);
        } else {
            assert_eq!(auto_source, 5);
            assert!(matches!(last.margin_bottom(), BoxLengthFact::Auto { .. }));
            assert_eq!(last.margin_bottom(), facts.row(5).unwrap().margin[2]);
        }
    });
}

#[test]
fn vertical_auto_final_normal_reaches_parent_auto_height() {
    assert_vertical_auto_case(
        "#b3 { margin-bottom:auto; }",
        23.0,
        0.0,
        (23.0, 12.0, 24.0),
        5,
    );
}

#[test]
fn vertical_auto_float_preserves_placement() {
    assert_vertical_auto_case(
        "#f { margin-top:auto; margin-bottom:auto; }",
        23.0,
        4.0,
        (27.0, 16.0, 28.0),
        3,
    );
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
            80 + 32 * 6 + 74 * (6 - 2) + 66 * (6 - 3)
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
    computed_height: BoxLengthFact,
    margin_top: BoxLengthFact,
    margin_bottom: BoxLengthFact,
    used_margin_top_bits: u32,
    used_margin_bottom_bits: u32,
    clear: KeywordBoxFact<ClearSide>,
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
    parent_computed: [BoxLengthFact; 4],
    parent_clear: KeywordBoxFact<ClearSide>,
    parent_clearance: ClearanceRequirement,
    receipt: SiblingLayoutReceipt,
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
                    computed_height: child.computed_height(),
                    margin_top: child.margin_top(),
                    margin_bottom: child.margin_bottom(),
                    used_margin_top_bits: child.used_margin_top().to_bits(),
                    used_margin_bottom_bits: child.used_margin_bottom().to_bits(),
                    clear: child.clear(),
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
        parent_computed: [
            layout.parent().computed_width(),
            layout.parent().computed_height(),
            layout.parent().computed_margin_top(),
            layout.parent().computed_margin_bottom(),
        ],
        parent_clear: layout.parent().clear(),
        parent_clearance: layout.parent().clearance(),
        receipt: layout.receipt(),
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
    assert_eq!(actual.parent_computed, expected.parent_computed);
    assert_eq!(actual.parent_clear, expected.parent_clear);
    assert_eq!(actual.parent_clearance, expected.parent_clearance);
    assert_eq!(actual.receipt, expected.receipt);
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
    assert_eq!(actual_float.computed_height, control_float.computed_height);
    assert_eq!(actual_float.margin_top, control_float.margin_top);
    assert_eq!(actual_float.margin_bottom, control_float.margin_bottom);
    assert_eq!(
        actual_float.used_margin_top_bits,
        control_float.used_margin_top_bits
    );
    assert_eq!(
        actual_float.used_margin_bottom_bits,
        control_float.used_margin_bottom_bits
    );
    assert_eq!(actual_float.used_margin_top_bits, 0.0f32.to_bits());
    assert_eq!(actual_float.used_margin_bottom_bits, 0.0f32.to_bits());
    assert_eq!(actual_float.clear, control_float.clear);
    assert_eq!(
        oversized.layout.parent_computed,
        control.layout.parent_computed
    );
    assert_eq!(oversized.layout.parent_clear, control.layout.parent_clear);
    assert_eq!(
        oversized.layout.parent_clearance,
        control.layout.parent_clearance
    );
    assert_eq!(oversized.layout.receipt, control.layout.receipt);
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
        assert_eq!(actual.computed_height, expected.computed_height);
        assert_eq!(actual.margin_top, expected.margin_top);
        assert_eq!(actual.margin_bottom, expected.margin_bottom);
        assert_eq!(actual.used_margin_top_bits, expected.used_margin_top_bits);
        assert_eq!(
            actual.used_margin_bottom_bits,
            expected.used_margin_bottom_bits
        );
        assert_eq!(actual.clear, expected.clear);
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
        let work = 766;
        for short in [1, 765] {
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
            594,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            938,
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
            assert_eq!(work, 80 + 32 * source_count + 74 * children + 66 * normals);
        });
    }
}

#[test]
fn fitting_and_oversized_widths_have_identical_resource_receipts() {
    for (source_count, facade_work, core_work) in [(5, 594, 481), (6, 766, 563), (7, 938, 646)] {
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
            594,
            481,
            419,
        ),
        (
            6,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b3 { margin-left:auto; }",
            766,
            563,
            501,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            938,
            646,
            584,
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
            Err(SiblingLayoutError::ParentOrigin(_))
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

    // Leading clear on B1 or the float is admitted and inert: the layout equals the
    // same CSS without the `clear` declaration, and only the targeted fact differs.
    let base = "#f { margin-left:auto; margin-right:auto; }";
    for (clear_rule, source, value) in [
        ("#b1 { clear:left; }", 2, ClearSide::Left),
        ("#f { clear:both; }", 3, ClearSide::Both),
    ] {
        assert_leading_clear_case(
            6,
            base,
            &format!("{base} {clear_rule}"),
            &[(
                source,
                KeywordBoxFact {
                    value,
                    origin: SPECIFIED,
                },
            )],
        );
    }
}

#[test]
fn unsupported_categories_keep_typed_sources_while_overwide_normals_succeed() {
    use CapturedClearanceUnsupported as Unsupported;

    for (suffix, source, property, reason) in [
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

    // A nonzero vertical float margin on an oversized float is admitted: margin box
    // 0..101 x 16..19, content 0..101 x 17..19.
    let vertical_css = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} #f {{ width:101px; margin-top:1px; }}",
        baseline_css()
    );
    sibling_fixture(6, &vertical_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let placed = layout.sequence().placed_float();
        assert_eq!(
            (placed.left, placed.right, placed.top, placed.bottom),
            (0.0, 101.0, 16.0, 19.0)
        );
        let c = layout.children()[1].content();
        assert_eq!((c.left, c.right, c.top, c.bottom), (0.0, 101.0, 17.0, 19.0));
    });

    // A nonzero horizontal float margin is admitted; with Auto right (used zero)
    // it makes the float oversized through its margin alone.
    let margin_css = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} #f {{ width:101px; margin-left:1px; }}",
        baseline_css()
    );
    sibling_fixture(6, &margin_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let placed = layout.sequence().placed_float();
        assert_eq!((placed.left, placed.right), (0.0, 102.0));
        let band = layout.sequence().float_content();
        assert_eq!((band.left, band.right), (1.0, 102.0));
        let float = &layout.children()[1];
        assert_eq!((float.content().left, float.content().right), (1.0, 102.0));
        assert_eq!(float.used_margin_left().to_bits(), 1.0_f32.to_bits());
        assert_eq!(float.used_margin_right().to_bits(), 0.0_f32.to_bits());
    });

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
fn auto_vertical_margins_advance_to_later_profile_faults() {
    use CapturedClearanceUnsupported as U;

    for (selector, projected, margins, used_top, used_bottom) in [
        (
            "#b1",
            2,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            0.0,
            0.0,
        ),
        (
            "#b3",
            5,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            2.0,
            0.0,
        ),
        (
            "#b1",
            2,
            "margin-top:auto; margin-right:auto; margin-bottom:auto; margin-left:auto;",
            0.0,
            0.0,
        ),
        (
            "#b3",
            5,
            "margin-top:auto; margin-right:auto; margin-bottom:auto; margin-left:auto;",
            0.0,
            0.0,
        ),
        (
            "#f",
            3,
            "margin-right:auto; margin-bottom:auto; margin-left:auto;",
            0.0,
            0.0,
        ),
        (
            "#f",
            3,
            "margin-top:auto; margin-right:auto; margin-left:auto;",
            0.0,
            0.0,
        ),
    ] {
        let css = format!("{} {selector} {{ {margins} }}", baseline_css());
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
            let child = &layout.children()[projected - 2];
            assert_eq!(child.used_margin_top(), used_top);
            assert_eq!(child.used_margin_bottom(), used_bottom);
        });
    }

    for (suffix, captured) in [
        ("#b1 { margin-bottom:auto; } #b3 { height:auto; }", true),
        (
            "#b1 { margin-bottom:auto; } #b3 { padding-left:1px; }",
            false,
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
            if captured {
                assert!(matches!(
                    error,
                    SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                        projected: 5,
                        property: "height",
                        reason: U::HeightKind,
                    })
                ));
            } else {
                assert!(matches!(
                    error,
                    SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                        ClearanceOwnerError::Unsupported {
                            source: 5,
                            reason: OwnerProfileReason::ChildDecoration,
                        }
                    ))
                ));
            }
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

// ---------------------------------------------------------------------------
// Vertical Auto Task 2: complete observations over the planned 284-case inventory.
// ---------------------------------------------------------------------------

const TINY_PX: &str = "0.0000000000000000000000000000000000000000000000000001px";
const MAX_NUMBER: &str = "340282346638528859811704183484516925440";
const INVENTORY_CSS_FNV1A64: u64 = 0x0f59_2eef_51c0_efb7;
const INVENTORY_SEED_FNV1A64: u64 = 0x8702_70b4_3e59_570b;

#[derive(Clone, Copy, Debug, PartialEq)]
enum ProvenanceKind {
    Initial,
    InheritedAuto,
    InheritedZero,
    Rounded,
}

#[derive(Clone, Copy, Debug)]
struct ExtremeCase {
    content_left: f32,
    parent_width: f32,
    parent_width_css: &'static str,
    float_width: f32,
    float_width_css: &'static str,
    overflow: bool,
}

#[derive(Clone, Debug)]
struct InventoryCase {
    id: String,
    family: &'static str,
    nodes: usize,
    side: &'static str,
    parent_height: &'static str,
    auto: Vec<[bool; 2]>,
    used: Vec<[i64; 2]>,
    float_height: i64,
    clear: &'static str,
    origin: Option<(usize, ProvenanceKind)>,
    direction: &'static str,
    extreme: Option<ExtremeCase>,
    target: Option<usize>,
    css: String,
}

struct InventoryOptions {
    change: Option<(usize, [bool; 2])>,
    all_auto: bool,
    float_height: i64,
    clear: &'static str,
    origin: Option<(usize, ProvenanceKind)>,
    direction: &'static str,
    extreme: Option<ExtremeCase>,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            change: None,
            all_auto: false,
            float_height: 2,
            clear: "both",
            origin: None,
            direction: "ltr",
            extreme: None,
        }
    }
}

fn add_inventory_case(
    cases: &mut Vec<InventoryCase>,
    family: &'static str,
    nodes: usize,
    side: &'static str,
    parent_height: &'static str,
    options: InventoryOptions,
) {
    let base = [[0, 4], [0, 0], [3, 3], [2, 4], [1, 2]];
    let mut used = base[..nodes - 2].to_vec();
    let mut auto = vec![[false, false]; nodes - 2];
    if options.all_auto {
        used = vec![[0, 0]; nodes - 2];
        auto = vec![[true, true]; nodes - 2];
    }
    if let Some((source, pair)) = options.change {
        used[source - 2] = [0, 0];
        auto[source - 2] = pair;
    }
    let mut css = vec![
        "#o { width:100px; }".to_string(),
        format!(
            "#p {{ overflow:hidden; padding-top:1px; padding-bottom:1px; height:{parent_height}; direction:{}; }}",
            options.direction
        ),
    ];
    for source in 2..nodes {
        let selector = match source {
            2 => "#b1".to_string(),
            3 => "#f".to_string(),
            _ => format!("#b{}", source - 2),
        };
        let height = match source {
            3 => options.float_height,
            6 => 2,
            _ => 1,
        };
        let mut rules = vec![format!("height:{height}px")];
        if source == 3 {
            rules.push(format!("float:{side}"));
            rules.push("width:10px".to_string());
        } else if source >= 4 {
            rules.push(format!("clear:{}", options.clear));
        }
        for (edge, index) in [("top", 0), ("bottom", 1)] {
            let mut value = if auto[source - 2][index] {
                "auto".to_string()
            } else {
                format!("{}px", used[source - 2][index])
            };
            if let Some((origin_source, kind)) = options.origin
                && origin_source == source
            {
                match kind {
                    ProvenanceKind::Initial => continue,
                    ProvenanceKind::InheritedAuto | ProvenanceKind::InheritedZero => {
                        value = "inherit".to_string();
                    }
                    ProvenanceKind::Rounded => value = TINY_PX.to_string(),
                }
            }
            rules.push(format!("margin-{edge}:{value}"));
        }
        css.push(format!("{selector} {{{};}}", rules.join(";")));
    }
    if let Some((_, kind)) = options.origin
        && matches!(
            kind,
            ProvenanceKind::InheritedAuto | ProvenanceKind::InheritedZero
        )
    {
        let value = if kind == ProvenanceKind::InheritedAuto {
            "auto"
        } else {
            "0px"
        };
        css.push(format!(
            "#p {{ margin-top:{value}; margin-bottom:{value}; }}"
        ));
    }
    if family == "horizontal-contrast" {
        let opposite = if options.direction == "ltr" {
            "rtl"
        } else {
            "ltr"
        };
        css.push(format!("#o {{ direction:{opposite}; }}"));
        css.push("#f {width:140px;margin-left:auto;margin-right:auto;}".to_string());
        css.push("#b1 {width:40px;margin-left:auto;margin-right:auto;}".to_string());
        css.push("#b2 {width:40px;margin-left:-10px;margin-right:auto;}".to_string());
        if nodes > 5 {
            css.push("#b3 {width:120px;margin-left:auto;margin-right:auto;}".to_string());
        }
        if nodes > 6 {
            css.push("#b4 {width:auto;margin-left:120px;margin-right:auto;}".to_string());
        }
    }
    if let Some(extreme) = options.extreme {
        css.push(format!("#o {{width:{}px;}}", extreme.parent_width_css));
        css.push(format!("#p {{width:{}px;}}", extreme.parent_width_css));
        css.push(format!(
            "#f {{width:{}px;margin-left:auto;margin-right:auto;}}",
            extreme.float_width_css
        ));
    }
    cases.push(InventoryCase {
        id: format!("va-{:03}", cases.len()),
        family,
        nodes,
        side,
        parent_height,
        auto,
        used,
        float_height: options.float_height,
        clear: options.clear,
        origin: options.origin,
        direction: options.direction,
        extreme: options.extreme,
        target: options.change.map(|(source, _)| source),
        css: css.join("\n"),
    });
}

/// Regenerates the sealed plan inventory in its exact enumeration order.
fn inventory_cases() -> Vec<InventoryCase> {
    let mut cases = Vec::new();
    let pairs = [[false, false], [true, false], [false, true], [true, true]];
    for nodes in [5, 6, 7] {
        for target in 2..nodes {
            for side in ["left", "right"] {
                for parent_height in ["auto", "20px"] {
                    for pair in pairs {
                        add_inventory_case(
                            &mut cases,
                            "single-target",
                            nodes,
                            side,
                            parent_height,
                            InventoryOptions {
                                change: Some((target, pair)),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }
    for nodes in [5, 6, 7] {
        for side in ["left", "right"] {
            for parent_height in ["auto", "20px"] {
                for float_height in [2, 40] {
                    for clear in ["none", "both"] {
                        add_inventory_case(
                            &mut cases,
                            "all-auto",
                            nodes,
                            side,
                            parent_height,
                            InventoryOptions {
                                all_auto: true,
                                float_height,
                                clear,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }
    for nodes in [5, 6, 7] {
        for side in ["left", "right"] {
            for parent_height in ["auto", "20px"] {
                add_inventory_case(
                    &mut cases,
                    "equality",
                    nodes,
                    side,
                    parent_height,
                    InventoryOptions {
                        all_auto: true,
                        float_height: 0,
                        ..Default::default()
                    },
                );
            }
        }
    }
    for source in [2, 3, 6] {
        for kind in [
            ProvenanceKind::Initial,
            ProvenanceKind::InheritedAuto,
            ProvenanceKind::InheritedZero,
            ProvenanceKind::Rounded,
        ] {
            let pair = if kind == ProvenanceKind::InheritedAuto {
                [true, true]
            } else {
                [false, false]
            };
            add_inventory_case(
                &mut cases,
                "provenance",
                7,
                if source == 3 { "right" } else { "left" },
                "auto",
                InventoryOptions {
                    change: Some((source, pair)),
                    origin: Some((source, kind)),
                    ..Default::default()
                },
            );
        }
    }
    for nodes in [5, 6, 7] {
        for side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                add_inventory_case(
                    &mut cases,
                    "horizontal-contrast",
                    nodes,
                    side,
                    "auto",
                    InventoryOptions {
                        all_auto: true,
                        direction,
                        ..Default::default()
                    },
                );
            }
        }
    }
    for (parent_width, parent_width_css) in [(1.0, "1"), (3.0, "3")] {
        for side in ["left", "right"] {
            add_inventory_case(
                &mut cases,
                "extreme",
                6,
                side,
                "auto",
                InventoryOptions {
                    all_auto: true,
                    extreme: Some(ExtremeCase {
                        content_left: 16_777_216.0,
                        parent_width,
                        parent_width_css,
                        float_width: 140.0,
                        float_width_css: "140",
                        overflow: false,
                    }),
                    ..Default::default()
                },
            );
        }
    }
    for sign in [1.0f32, -1.0] {
        for side in ["left", "right"] {
            add_inventory_case(
                &mut cases,
                "extreme",
                6,
                side,
                "auto",
                InventoryOptions {
                    all_auto: true,
                    extreme: Some(ExtremeCase {
                        content_left: sign * f32::MAX,
                        parent_width: 0.0,
                        parent_width_css: "0",
                        float_width: f32::MAX,
                        float_width_css: MAX_NUMBER,
                        overflow: (sign > 0.0 && side == "left") || (sign < 0.0 && side == "right"),
                    }),
                    ..Default::default()
                },
            );
        }
    }
    cases
}

#[derive(Debug)]
struct InventoryTail {
    source: usize,
    hypothetical_top: i64,
    zero_clearance_top: Option<i64>,
    clearance: Option<i64>,
    top: i64,
    bottom: i64,
    previous: (usize, i64),
    own_top: i64,
}

#[derive(Debug)]
struct InventoryLayout {
    before: (i64, i64),
    float: (i64, i64),
    tail: Vec<InventoryTail>,
    content_bottom: i64,
    used_height: i64,
    padding_bottom: i64,
    extent: Option<(i64, i64, bool)>,
}

#[derive(Debug)]
enum InventorySeed {
    Layout(InventoryLayout),
    Equality(usize),
    FloatPlacement,
}

/// Independent integer model: every published stage is an exact f32 integer.
fn inventory_seed(case: &InventoryCase) -> InventorySeed {
    if case.extreme.is_some_and(|extreme| extreme.overflow) {
        return InventorySeed::FloatPlacement;
    }
    let used = &case.used;
    let clear = case.clear == "both";
    let before_top = 11 + used[0][0];
    let before_bottom = before_top + 1;
    let float_top = before_bottom + used[0][1];
    let float_bottom = float_top + case.float_height;
    let mut bottom = before_bottom;
    let mut previous = (2, used[0][1]);
    let mut tail = Vec::new();
    for source in 4..case.nodes {
        let own_top = used[source - 2][0];
        let hypothetical_top = bottom + previous.1.max(own_top);
        if clear && hypothetical_top == float_bottom {
            return InventorySeed::Equality(source);
        }
        let required = clear && hypothetical_top < float_bottom;
        let zero = required.then_some(bottom + previous.1 + own_top);
        let top = if required {
            float_bottom
        } else {
            hypothetical_top
        };
        let height = if source == 6 { 2 } else { 1 };
        bottom = top + height;
        tail.push(InventoryTail {
            source,
            hypothetical_top,
            zero_clearance_top: zero,
            clearance: zero.map(|zero| float_bottom - zero),
            top,
            bottom,
            previous,
            own_top,
        });
        previous = (source, used[source - 2][1]);
    }
    let normal_edge = bottom + previous.1;
    let selected = normal_edge.max(float_bottom);
    let explicit = case.parent_height == "20px";
    InventorySeed::Layout(InventoryLayout {
        before: (before_top, before_bottom),
        float: (float_top, float_bottom),
        tail,
        content_bottom: if explicit { 31 } else { selected },
        used_height: if explicit { 20 } else { selected - 11 },
        padding_bottom: if explicit { 32 } else { selected + 1 },
        extent: (!explicit).then_some((normal_edge, selected, float_bottom > normal_edge)),
    })
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn seed_line(case: &InventoryCase, seed: &InventorySeed) -> String {
    let optional = |value: Option<i64>| value.map_or("-".to_string(), |value| value.to_string());
    match seed {
        InventorySeed::Layout(layout) => {
            let tail = layout
                .tail
                .iter()
                .map(|tail| {
                    format!(
                        "{},{},{},{},{},{}",
                        tail.source,
                        tail.hypothetical_top,
                        optional(tail.zero_clearance_top),
                        optional(tail.clearance),
                        tail.top,
                        tail.bottom
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            let extent = layout
                .extent
                .map_or("-".to_string(), |(normal, selected, extended)| {
                    format!("{normal},{selected},{}", u8::from(extended))
                });
            format!(
                "{}|layout|{},{}|{},{}|{tail}|{},{},{}|{extent}",
                case.id,
                layout.before.0,
                layout.before.1,
                layout.float.0,
                layout.float.1,
                layout.content_bottom,
                layout.used_height,
                layout.padding_bottom
            )
        }
        InventorySeed::Equality(source) => format!("{}|UnsupportedEquality|{source}", case.id),
        InventorySeed::FloatPlacement => format!("{}|FloatPlacement/Unrepresentable|3", case.id),
    }
}

fn expected_child_margin(
    case: &InventoryCase,
    source: usize,
    edge: usize,
    original_parent: usize,
) -> BoxLengthFact {
    let absolute = |pixels: f32, rounded_to_zero, origin| {
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels,
            rounded_to_zero,
            origin,
        })
    };
    let inherited = BoxFactOrigin::Inherited { original_parent };
    if let Some((origin_source, kind)) = case.origin
        && origin_source == source
    {
        return match kind {
            ProvenanceKind::Initial => absolute(0.0, false, BoxFactOrigin::Initial),
            ProvenanceKind::InheritedAuto => BoxLengthFact::Auto { origin: inherited },
            ProvenanceKind::InheritedZero => absolute(0.0, false, inherited),
            ProvenanceKind::Rounded => absolute(0.0, true, BoxFactOrigin::Specified),
        };
    }
    if case.auto[source - 2][edge] {
        BoxLengthFact::Auto {
            origin: BoxFactOrigin::Specified,
        }
    } else {
        absolute(
            case.used[source - 2][edge] as f32,
            false,
            BoxFactOrigin::Specified,
        )
    }
}

/// Literal normal width equations for the horizontal contrast family.
fn contrast_width(source: usize, direction: &str) -> (BlockWidthResult, (f32, f32)) {
    let rtl = direction == "rtl";
    let result = |width, margin_left, margin_right, passes| BlockWidthResult {
        width,
        margin_left,
        margin_right,
        passes,
    };
    match source {
        2 => (result(40.0, 30.0, 30.0, 1), (30.0, 70.0)),
        4 => (result(40.0, -10.0, 70.0, 1), (-10.0, 30.0)),
        5 if rtl => (result(120.0, -20.0, 0.0, 1), (-20.0, 100.0)),
        5 => (result(120.0, 0.0, -20.0, 1), (0.0, 120.0)),
        6 if rtl => (result(0.0, 100.0, 0.0, 2), (100.0, 100.0)),
        6 => (result(0.0, 120.0, -20.0, 2), (120.0, 120.0)),
        _ => unreachable!(),
    }
}

fn assert_inventory_layout(
    case: &InventoryCase,
    expected: &InventoryLayout,
    layout: &CapturedSiblingLayout<'_>,
    original_parent: usize,
) {
    let id = &case.id;
    let f = |value: i64| value as f32;
    let auto_specified = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Specified,
    };
    let initial_zero = BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels: 0.0,
        rounded_to_zero: false,
        origin: BoxFactOrigin::Initial,
    });

    // Parent horizontal band, computed provenance and vertical results.
    let parent = layout.parent();
    let (band, parent_width) = match case.extreme {
        None => ((0.0, 100.0), 100.0),
        Some(extreme) if extreme.content_left == 16_777_216.0 => (
            if extreme.parent_width == 3.0 {
                (16_777_216.0, 16_777_220.0)
            } else {
                (16_777_216.0, 16_777_216.0)
            },
            extreme.parent_width,
        ),
        Some(extreme) => ((extreme.content_left, extreme.content_left), 0.0),
    };
    assert_eq!(parent.source_index(), 1, "{id}");
    assert_eq!(parent.containing_source_index(), 0, "{id}");
    assert_eq!(
        (parent.content().left, parent.content().right),
        band,
        "{id}"
    );
    assert_eq!(parent.horizontal().used.width, parent_width, "{id}");
    assert_eq!(parent.origin().border_top, 10.0, "{id}");
    assert_eq!(parent.origin().content_top, 11.0, "{id}");
    assert_eq!(
        parent.origin().used_margin_top.to_bits(),
        0.0f32.to_bits(),
        "{id}"
    );
    let parent_margin = match case.origin {
        Some((_, ProvenanceKind::InheritedAuto)) => auto_specified,
        Some((_, ProvenanceKind::InheritedZero)) => BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 0.0,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Specified,
        }),
        _ => initial_zero,
    };
    assert_eq!(parent.computed_margin_top(), parent_margin, "{id}");
    assert_eq!(parent.computed_margin_bottom(), parent_margin, "{id}");
    assert_eq!(
        parent.computed_height(),
        if case.parent_height == "auto" {
            auto_specified
        } else {
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 20.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Specified,
            })
        },
        "{id}"
    );
    assert_eq!(parent.clearance(), ClearanceRequirement::Absent, "{id}");
    assert_eq!(
        parent.height().content_bottom,
        f(expected.content_bottom),
        "{id}"
    );
    assert_eq!(parent.height().used_height, f(expected.used_height), "{id}");
    assert_eq!(
        parent.height().auto_extent,
        expected.extent.map(|(normal, selected, float_extended)| {
            iced_css_core::formatting::bfc_height::AutoHeightExtent {
                normal_margin_bottom: f(normal),
                selected_bottom: f(selected),
                float_extended,
            }
        }),
        "{id}"
    );
    assert_eq!(parent.content().top, 11.0, "{id}");
    assert_eq!(parent.content().bottom, f(expected.content_bottom), "{id}");
    let bottom = parent.bottom();
    assert_eq!(bottom.padding_bottom, f(expected.padding_bottom), "{id}");
    assert_eq!(bottom.border_bottom, f(expected.padding_bottom), "{id}");
    assert_eq!(bottom.margin_bottom, f(expected.padding_bottom), "{id}");
    assert_eq!(
        bottom.used_margin_bottom.to_bits(),
        0.0f32.to_bits(),
        "{id}"
    );

    // Every principal: source joins, geometry, widths, computed and used margins.
    let children = layout.children();
    assert_eq!(children.len(), case.nodes - 2, "{id}");
    for (index, child) in children.iter().enumerate() {
        let source = index + 2;
        assert_eq!(child.source_index(), source, "{id}");
        assert_eq!(child.containing_source_index(), 1, "{id}");
        assert!(std::ptr::eq(
            layout.source(source).unwrap(),
            &layout.sources()[source]
        ));
        let (top, bottom) = match source {
            2 => expected.before,
            3 => expected.float,
            _ => {
                let tail = &expected.tail[source - 4];
                (tail.top, tail.bottom)
            }
        };
        assert_eq!(
            (child.content().top, child.content().bottom),
            (f(top), f(bottom)),
            "{id} source {source}"
        );
        assert_eq!(child.padding(), child.content(), "{id}");
        assert_eq!(child.border(), child.content(), "{id}");
        let height = match source {
            3 => case.float_height,
            6 => 2,
            _ => 1,
        };
        assert_eq!(child.used_height(), f(height), "{id}");
        for (edge, computed, used) in [
            (0, child.margin_top(), child.used_margin_top()),
            (1, child.margin_bottom(), child.used_margin_bottom()),
        ] {
            let expected_fact = expected_child_margin(case, source, edge, original_parent);
            assert_eq!(computed, expected_fact, "{id} source {source} edge {edge}");
            if let BoxLengthFact::Absolute(value) = computed {
                assert_eq!(
                    value.pixels.to_bits(),
                    f(case.used[source - 2][edge]).to_bits(),
                    "{id}"
                );
            }
            assert_eq!(
                used.to_bits(),
                f(case.used[source - 2][edge]).to_bits(),
                "{id} source {source} edge {edge}"
            );
        }
        let expected_clear = if source >= 4 {
            KeywordBoxFact {
                value: if case.clear == "both" {
                    ClearSide::Both
                } else {
                    ClearSide::None
                },
                origin: BoxFactOrigin::Specified,
            }
        } else {
            KeywordBoxFact {
                value: ClearSide::None,
                origin: BoxFactOrigin::Initial,
            }
        };
        assert_eq!(child.clear(), expected_clear, "{id}");
        if source == 3 {
            assert_eq!(child.kind(), PrincipalChildKind::Float, "{id}");
            assert_eq!(child.normal_width(), None, "{id}");
            assert_eq!(child.used_margin_left().to_bits(), 0.0f32.to_bits());
            assert_eq!(child.used_margin_right().to_bits(), 0.0f32.to_bits());
            let (width, edges) = match (case.family, case.extreme) {
                ("horizontal-contrast", _) => (
                    140.0,
                    if case.side == "left" {
                        (0.0, 140.0)
                    } else {
                        (-40.0, 100.0)
                    },
                ),
                (_, Some(extreme)) if extreme.content_left == 16_777_216.0 => (
                    extreme.float_width,
                    match (case.side, extreme.parent_width == 3.0) {
                        ("left", _) => (16_777_216.0, 16_777_356.0),
                        (_, true) => (16_777_080.0, 16_777_220.0),
                        (_, false) => (16_777_076.0, 16_777_216.0),
                    },
                ),
                (_, Some(extreme)) => (
                    extreme.float_width,
                    if extreme.content_left > 0.0 {
                        (0.0, f32::MAX)
                    } else {
                        (-f32::MAX, 0.0)
                    },
                ),
                _ => (
                    10.0,
                    if case.side == "left" {
                        (0.0, 10.0)
                    } else {
                        (90.0, 100.0)
                    },
                ),
            };
            assert_eq!(child.used_width(), width, "{id}");
            assert_eq!((child.content().left, child.content().right), edges, "{id}");
        } else {
            assert_eq!(child.kind(), PrincipalChildKind::Normal, "{id}");
            let (width, edges) = if case.family == "horizontal-contrast" {
                contrast_width(source, case.direction)
            } else {
                (
                    BlockWidthResult {
                        width: parent_width,
                        margin_left: 0.0,
                        margin_right: 0.0,
                        passes: 1,
                    },
                    band,
                )
            };
            assert_eq!(child.normal_width(), Some(width), "{id} source {source}");
            assert_eq!(child.used_width().to_bits(), width.width.to_bits(), "{id}");
            assert_eq!((child.content().left, child.content().right), edges, "{id}");
        }
    }

    // The retained core: before, placed float, every tail record and group.
    let sequence = layout.sequence();
    assert_eq!(
        (sequence.before().top, sequence.before().bottom),
        (f(expected.before.0), f(expected.before.1)),
        "{id}"
    );
    assert_eq!(
        (sequence.placed_float().top, sequence.placed_float().bottom),
        (f(expected.float.0), f(expected.float.1)),
        "{id}"
    );
    assert_eq!(sequence.tail().len(), expected.tail.len(), "{id}");
    for (record, tail) in sequence.tail().iter().zip(&expected.tail) {
        assert_eq!(record.source_index(), tail.source, "{id}");
        assert_eq!(
            (record.vertical().top, record.vertical().bottom),
            (f(tail.top), f(tail.bottom)),
            "{id}"
        );
        assert_eq!(record.hypothetical_top(), f(tail.hypothetical_top), "{id}");
        assert_eq!(
            record.zero_clearance_top(),
            tail.zero_clearance_top.map(f),
            "{id}"
        );
        assert_eq!(record.clearance(), tail.clearance.map(f), "{id}");
        let members = vec![
            MarginMember {
                source_index: tail.previous.0,
                side: MarginSide::Bottom,
                value: f(tail.previous.1),
            },
            MarginMember {
                source_index: tail.source,
                side: MarginSide::Top,
                value: f(tail.own_top),
            },
        ];
        let positive = f(tail.previous.1.max(tail.own_top));
        assert_eq!(
            observe_margin_group(record.hypothetical_group()),
            observed_group(MarginGroupId::HypotheticalPair, members.clone(), positive),
            "{id}"
        );
        for (actual, literal) in record.hypothetical_group().members().iter().zip(&members) {
            assert_eq!(actual.value.to_bits(), literal.value.to_bits(), "{id}");
        }
        let actual_groups: Vec<_> = record
            .actual_groups()
            .iter()
            .map(observe_margin_group)
            .collect();
        if tail.clearance.is_some() {
            assert_eq!(record.barrier_source_index(), Some(tail.source), "{id}");
            assert_eq!(
                actual_groups,
                vec![
                    observed_group(
                        MarginGroupId::BeforeBarrier,
                        vec![members[0]],
                        f(tail.previous.1)
                    ),
                    observed_group(
                        MarginGroupId::AfterBarrier,
                        vec![members[1]],
                        f(tail.own_top)
                    ),
                ],
                "{id}"
            );
        } else {
            assert_eq!(record.barrier_source_index(), None, "{id}");
            assert_eq!(
                actual_groups,
                vec![observed_group(MarginGroupId::ActualPair, members, positive)],
                "{id}"
            );
        }
    }
    assert_eq!(layout.receipt().nodes, case.nodes, "{id}");
    assert_eq!(
        layout.receipt().work,
        [594, 766, 938][case.nodes - 5],
        "{id}"
    );
    assert_eq!(
        layout.receipt().core.work,
        [481, 563, 646][case.nodes - 5],
        "{id}"
    );
}

/// Replace only the intentionally changed computed facts after asserting both
/// sides, then require full equality of every other retained observation.
fn assert_same_except_changed_vertical_facts(
    id: &str,
    actual: &VerticalObservation,
    control: &VerticalObservation,
    target: usize,
    expected_changed: [Option<BoxLengthFact>; 2],
    parent_margin: Option<BoxLengthFact>,
) {
    let mut patched = actual.clone();
    let principal = &mut patched.principals[target - 2];
    let control_principal = &control.principals[target - 2];
    assert_eq!(principal.source, target, "{id}");
    let zero_specified = BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels: 0.0,
        rounded_to_zero: false,
        origin: BoxFactOrigin::Specified,
    });
    for (edge, expected) in expected_changed.into_iter().enumerate() {
        let (actual_fact, control_fact) = if edge == 0 {
            (&mut principal.margin_top, control_principal.margin_top)
        } else {
            (
                &mut principal.margin_bottom,
                control_principal.margin_bottom,
            )
        };
        assert_eq!(control_fact, zero_specified, "{id} control edge {edge}");
        if let Some(expected) = expected {
            assert_eq!(*actual_fact, expected, "{id} edge {edge}");
            assert_ne!(*actual_fact, control_fact, "{id} edge {edge}");
            *actual_fact = control_fact;
        }
    }
    assert_eq!(principal.used_margin_top_bits, 0.0f32.to_bits(), "{id}");
    assert_eq!(principal.used_margin_bottom_bits, 0.0f32.to_bits(), "{id}");
    if let Some(parent_margin) = parent_margin {
        assert_eq!(patched.parent_computed[2], parent_margin, "{id}");
        assert_eq!(patched.parent_computed[3], parent_margin, "{id}");
        patched.parent_computed[2] = control.parent_computed[2];
        patched.parent_computed[3] = control.parent_computed[3];
        patched.parent_origin.margin_resolution = control.parent_origin.margin_resolution;
        patched.parent_bottom.margin_resolution = control.parent_bottom.margin_resolution;
    }
    assert_eq!(&patched, control, "{id}");
}

#[test]
fn vertical_auto_inventory_cases_reach_complete_outputs() {
    use std::collections::HashMap;
    let cases = inventory_cases();
    assert_eq!(cases.len(), 284);
    let extreme_widths: Vec<_> = cases
        .iter()
        .filter_map(|case| case.extreme.map(|extreme| extreme.float_width.to_bits()))
        .collect();
    assert_eq!(
        extreme_widths,
        [
            140.0f32,
            140.0,
            140.0,
            140.0,
            f32::MAX,
            f32::MAX,
            f32::MAX,
            f32::MAX
        ]
        .map(f32::to_bits)
    );
    let mut families: HashMap<&str, usize> = HashMap::new();
    for case in &cases {
        *families.entry(case.family).or_default() += 1;
    }
    assert_eq!(
        families,
        HashMap::from([
            ("single-target", 192),
            ("all-auto", 48),
            ("equality", 12),
            ("provenance", 12),
            ("horizontal-contrast", 12),
            ("extreme", 8),
        ])
    );
    let css_bytes: Vec<u8> = cases
        .iter()
        .flat_map(|case| {
            let mut bytes = case.id.as_bytes().to_vec();
            bytes.push(0);
            bytes.extend_from_slice(case.css.as_bytes());
            bytes.push(0);
            bytes
        })
        .collect();
    assert_eq!(fnv1a64(&css_bytes), INVENTORY_CSS_FNV1A64);
    let seeds: Vec<_> = cases.iter().map(inventory_seed).collect();
    let lines = cases
        .iter()
        .zip(&seeds)
        .map(|(case, seed)| seed_line(case, seed))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(fnv1a64(lines.as_bytes()), INVENTORY_SEED_FNV1A64);

    let mut counts = [0usize; 3];
    let mut compared = 0usize;
    let mut controls: HashMap<(usize, usize, &str, &str), VerticalObservation> = HashMap::new();
    for (case, seed) in cases.iter().zip(&seeds) {
        let (content_left, content_width) = case.extreme.map_or((0.0, 100.0), |extreme| {
            (extreme.content_left, extreme.parent_width)
        });
        let mut observed = None;
        sibling_fixture(case.nodes, &case.css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let original_parent = projection.original_index(1).unwrap();
            assert_eq!(original_parent, case.nodes - 2);
            let result = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left,
                    content_width,
                    content_top: 10.0,
                },
                layout_limits(),
            );
            match seed {
                InventorySeed::Layout(expected) => {
                    let layout = result.unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
                    assert_inventory_layout(case, expected, &layout, original_parent);
                    for child in layout.children() {
                        let row = facts.row(child.source_index()).unwrap();
                        assert_eq!(child.margin_top(), row.margin[0], "{}", case.id);
                        assert_eq!(child.margin_bottom(), row.margin[2], "{}", case.id);
                        assert_eq!(child.computed_margin_left(), row.margin[3]);
                        assert_eq!(child.computed_margin_right(), row.margin[1]);
                        assert_eq!(child.computed_width(), row.width);
                        assert_eq!(child.computed_height(), row.height);
                        assert!(std::ptr::eq(
                            layout.source(child.source_index()).unwrap(),
                            &projection.nodes()[child.source_index()]
                        ));
                    }
                    counts[0] += 1;
                    observed = Some(observe_layout(&layout));
                }
                InventorySeed::Equality(source) => {
                    assert!(
                        matches!(
                            result,
                            Err(SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                                source: actual,
                                error: ClearanceOwnerError::UnsupportedEquality,
                            })) if actual == *source
                        ),
                        "{}: {result:?}",
                        case.id
                    );
                    counts[1] += 1;
                }
                InventorySeed::FloatPlacement => {
                    assert!(
                        matches!(
                            result,
                            Err(SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                                source: 3,
                                error: ClearanceOwnerError::FloatPlacement(
                                    iced_css_core::formatting::floats::FloatError::Unrepresentable
                                ),
                            }))
                        ),
                        "{}: {result:?}",
                        case.id
                    );
                    counts[2] += 1;
                }
            }
        });

        let Some(observed) = observed else { continue };
        let Some(target) = case.target else { continue };
        let key = (case.nodes, target, case.side, case.parent_height);
        let auto_specified = BoxLengthFact::Auto {
            origin: BoxFactOrigin::Specified,
        };
        match (case.family, case.origin) {
            ("single-target", _) if case.auto[target - 2] == [false, false] => {
                controls.insert(key, observed);
            }
            ("single-target", _) => {
                let control = &controls[&key];
                let changed = case.auto[target - 2].map(|auto| auto.then_some(auto_specified));
                assert_same_except_changed_vertical_facts(
                    &case.id, &observed, control, target, changed, None,
                );
                compared += 1;
            }
            ("provenance", Some((source, kind))) => {
                let control = &controls[&key];
                let original_parent = case.nodes - 2;
                let fact = expected_child_margin(case, source, 0, original_parent);
                let parent_margin = match kind {
                    ProvenanceKind::InheritedAuto => Some(auto_specified),
                    ProvenanceKind::InheritedZero => {
                        Some(BoxLengthFact::Absolute(AbsoluteBoxFact {
                            pixels: 0.0,
                            rounded_to_zero: false,
                            origin: BoxFactOrigin::Specified,
                        }))
                    }
                    _ => None,
                };
                assert_same_except_changed_vertical_facts(
                    &case.id,
                    &observed,
                    control,
                    target,
                    [Some(fact), Some(fact)],
                    parent_margin,
                );
                compared += 1;
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(counts, [270, 12, 2]);
    // 144 single-target Auto variants plus 12 provenance witnesses.
    assert_eq!(compared, 156);
    assert_eq!(controls.len(), 48);
}

fn default_outer_layout<'p>(
    facts: &iced_css::evaluation::sibling_box_facts::SiblingBoxFacts<'p, '_, '_, '_, '_, '_>,
    outer: &'p iced_css_core::source::SourceNode,
    content_left: f32,
    content_width: f32,
    content_top: f32,
    limits: SiblingLayoutLimits,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    build_captured_sibling_layout(
        facts,
        ClearanceOuterContentGeometry {
            outer,
            content_left,
            content_width,
            content_top,
        },
        limits,
    )
}

fn vertical_spans(layout: &CapturedSiblingLayout<'_>) -> Vec<(usize, f32, f32)> {
    layout
        .children()
        .iter()
        .map(|child| {
            (
                child.source_index(),
                child.content().top,
                child.content().bottom,
            )
        })
        .collect()
}

#[test]
fn explicit_vertical_auto_seeds_reach_real_outputs() {
    use iced_css_core::formatting::bfc_height::AutoHeightExtent;
    let extent = |normal, selected, float_extended| {
        Some(AutoHeightExtent {
            normal_margin_bottom: normal,
            selected_bottom: selected,
            float_extended,
        })
    };
    let all_auto = "#b1, #f, #b2, #b3 { margin-top:auto; margin-bottom:auto; }";
    for (rule, spans, parent, auto_extent) in [
        (
            String::new(),
            [(11.0, 12.0), (16.0, 18.0), (18.0, 19.0), (22.0, 23.0)],
            (27.0, 16.0, 28.0),
            extent(27.0, 27.0, false),
        ),
        (
            "#b3 { margin-bottom:auto; }".to_string(),
            [(11.0, 12.0), (16.0, 18.0), (18.0, 19.0), (22.0, 23.0)],
            (23.0, 12.0, 24.0),
            extent(23.0, 23.0, false),
        ),
        (
            "#b1 { margin-bottom:auto; }".to_string(),
            [(11.0, 12.0), (12.0, 14.0), (15.0, 16.0), (19.0, 20.0)],
            (24.0, 13.0, 25.0),
            extent(24.0, 24.0, false),
        ),
        (
            "#b2 { margin-top:auto; }".to_string(),
            [(11.0, 12.0), (16.0, 18.0), (18.0, 19.0), (22.0, 23.0)],
            (27.0, 16.0, 28.0),
            extent(27.0, 27.0, false),
        ),
        (
            format!("{all_auto} #f {{ height:40px; }} #b2, #b3 {{ clear:none; }}"),
            [(11.0, 12.0), (12.0, 52.0), (12.0, 13.0), (13.0, 14.0)],
            (52.0, 41.0, 53.0),
            extent(14.0, 52.0, true),
        ),
        (
            format!(
                "{all_auto} #f {{ height:40px; }} #b2, #b3 {{ clear:none; }} #p {{ height:20px; }}"
            ),
            [(11.0, 12.0), (12.0, 52.0), (12.0, 13.0), (13.0, 14.0)],
            (31.0, 20.0, 32.0),
            None,
        ),
    ] {
        let css = format!("{} {rule}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap();
            assert_eq!(
                vertical_spans(&layout),
                (2..6)
                    .zip(spans)
                    .map(|(source, (top, bottom))| (source, top, bottom))
                    .collect::<Vec<_>>(),
                "{rule}"
            );
            assert_eq!(layout.parent().height().content_bottom, parent.0, "{rule}");
            assert_eq!(layout.parent().height().used_height, parent.1, "{rule}");
            assert_eq!(layout.parent().bottom().padding_bottom, parent.2, "{rule}");
            assert_eq!(layout.parent().height().auto_extent, auto_extent, "{rule}");
        });
    }

    // B2 top Auto alone: exact hypothetical/zero-clearance/clearance values and the
    // same ordered barrier split with a zero AfterBarrier member.
    let css = format!("{} #b2 {{ margin-top:auto; }}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let observation = observe_layout(&layout);
        let b2 = &observation.tail[0];
        assert_eq!(b2.source, 4);
        assert_eq!(
            (b2.hypothetical_top, b2.zero_clearance_top, b2.clearance),
            (16.0, Some(16.0), Some(2.0))
        );
        assert_eq!((b2.top, b2.bottom), (18.0, 19.0));
        assert_eq!(b2.barrier_source, Some(4));
        let pair = [
            MarginMember {
                source_index: 2,
                side: MarginSide::Bottom,
                value: 4.0,
            },
            MarginMember {
                source_index: 4,
                side: MarginSide::Top,
                value: 0.0,
            },
        ];
        assert_eq!(
            b2.hypothetical_group,
            observed_group(MarginGroupId::HypotheticalPair, pair.to_vec(), 4.0)
        );
        assert_eq!(
            b2.actual_groups,
            vec![
                observed_group(MarginGroupId::BeforeBarrier, vec![pair[0]], 4.0),
                observed_group(MarginGroupId::AfterBarrier, vec![pair[1]], 0.0),
            ]
        );
        assert_eq!(
            layout.children()[2].margin_top(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified
            }
        );
        assert_eq!(
            layout.children()[2].used_margin_top().to_bits(),
            0.0f32.to_bits()
        );
        assert_eq!(layout.children()[2].used_margin_bottom(), 3.0);
    });

    // All children Auto with F height 0 and clear both is source 4 equality.
    let css = format!("{} {all_auto} #f {{ height:0; }}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits()
            ),
            Err(SiblingLayoutError::Core(
                ClearanceSequenceError::Placement {
                    source: 4,
                    error: ClearanceOwnerError::UnsupportedEquality,
                }
            ))
        ));
    });

    // Horizontal-contrast and extreme representatives with all vertical Auto.
    let css = format!(
        "{} {all_auto} #o {{ direction:rtl; }} #p {{ direction:ltr; }} \
         #f {{ float:right; width:140px; margin-left:auto; margin-right:auto; }} \
         #b1 {{ width:40px; margin-left:auto; margin-right:auto; }} \
         #b3 {{ width:120px; margin-left:auto; margin-right:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let children = layout.children();
        assert_eq!(
            (children[1].content().left, children[1].content().right),
            (-40.0, 100.0)
        );
        assert_eq!(children[1].normal_width(), None);
        assert_eq!(
            children[0].normal_width(),
            Some(BlockWidthResult {
                width: 40.0,
                margin_left: 30.0,
                margin_right: 30.0,
                passes: 1,
            })
        );
        assert_eq!(
            children[3].normal_width(),
            Some(BlockWidthResult {
                width: 120.0,
                margin_left: 0.0,
                margin_right: -20.0,
                passes: 1,
            })
        );
        assert_eq!(
            vertical_spans(&layout),
            [
                (2, 11.0, 12.0),
                (3, 12.0, 14.0),
                (4, 14.0, 15.0),
                (5, 15.0, 16.0)
            ]
        );
        for child in children {
            assert_eq!(child.used_margin_top().to_bits(), 0.0f32.to_bits());
            assert_eq!(child.used_margin_bottom().to_bits(), 0.0f32.to_bits());
        }
    });
    let css = format!(
        "{} {all_auto} #o {{ width:3px; }} #p {{ width:3px; }} \
         #f {{ width:140px; margin-left:auto; margin-right:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            16_777_216.0,
            3.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        assert_eq!(
            (
                layout.parent().content().left,
                layout.parent().content().right
            ),
            (16_777_216.0, 16_777_220.0)
        );
        assert_eq!(
            (
                layout.children()[1].content().left,
                layout.children()[1].content().right
            ),
            (16_777_216.0, 16_777_356.0)
        );
        assert_eq!(layout.children()[0].used_width(), 3.0);
        assert_eq!(
            vertical_spans(&layout),
            [
                (2, 11.0, 12.0),
                (3, 12.0, 14.0),
                (4, 14.0, 15.0),
                (5, 15.0, 16.0)
            ]
        );
    });
}

#[test]
fn vertical_auto_provenance_is_actual_css_and_used_values_are_copied() {
    let initial_css = "
        #o { width: 100px; }
        #p { overflow: hidden; padding-top: 1px; padding-bottom: 1px; }
        #b1 { height: 1px; margin-bottom: 4px; }
        #f { float: left; width: 10px; height: 2px; }
        #b2 { height: 1px; margin-top: 3px; margin-bottom: 3px; clear: both; }
        #b3 { height: 1px; clear: both; }
    ";
    assert!(!initial_css.contains("#b3 { height: 1px; margin"));
    let absolute = |pixels: f32, rounded_to_zero, origin| {
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels,
            rounded_to_zero,
            origin,
        })
    };
    let inherited = BoxFactOrigin::Inherited { original_parent: 4 };
    let specified_zero_css = format!(
        "{} #b3 {{ margin-top:0; margin-bottom:0; }}",
        baseline_css()
    );
    let cases = [
        (
            initial_css.to_string(),
            absolute(0.0, false, BoxFactOrigin::Initial),
            None,
        ),
        (
            format!(
                "{} #p {{ margin-top:auto; margin-bottom:auto; }} \
                 #b3 {{ margin-top:inherit; margin-bottom:inherit; }}",
                baseline_css()
            ),
            BoxLengthFact::Auto { origin: inherited },
            Some(BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified,
            }),
        ),
        (
            format!(
                "{} #p {{ margin-top:0; margin-bottom:0; }} \
                 #b3 {{ margin-top:inherit; margin-bottom:inherit; }}",
                baseline_css()
            ),
            absolute(0.0, false, inherited),
            Some(absolute(0.0, false, BoxFactOrigin::Specified)),
        ),
        (
            format!(
                "{} #b3 {{ margin-top:{TINY_PX}; margin-bottom:{TINY_PX}; }}",
                baseline_css()
            ),
            absolute(0.0, true, BoxFactOrigin::Specified),
            None,
        ),
        (
            format!(
                "{} #b3 {{ margin-top:-{TINY_PX}; margin-bottom:-{TINY_PX}; }}",
                baseline_css()
            ),
            absolute(0.0, true, BoxFactOrigin::Specified),
            None,
        ),
        (
            format!(
                "{} #b3 {{ margin-top:auto; margin-bottom:auto; }}",
                baseline_css()
            ),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified,
            },
            None,
        ),
    ];
    let mut control = None;
    sibling_fixture(6, &specified_zero_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        control = Some(observe_layout(&layout));
    });
    let control = control.unwrap();
    for (css, expected, parent_margin) in cases {
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert_eq!(projection.original_index(1), Some(4));
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap();
            let b3 = &layout.children()[3];
            assert_eq!(b3.source_index(), 5);
            let copied = (
                b3.margin_top(),
                b3.margin_bottom(),
                b3.used_margin_top(),
                b3.used_margin_bottom(),
                layout.children()[2].margin_top(),
                layout.children()[2].used_margin_top(),
            );
            assert_eq!(copied.0, facts.row(5).unwrap().margin[0]);
            assert_eq!(copied.1, facts.row(5).unwrap().margin[2]);
            let observation = observe_layout(&layout);
            drop(layout);
            // Copied observations remain valid after the layout owner is dropped.
            assert_eq!(copied.0, expected, "{css}");
            assert_eq!(copied.1, expected, "{css}");
            if let BoxLengthFact::Absolute(value) = copied.0 {
                assert_eq!(value.pixels.to_bits(), 0.0f32.to_bits(), "{css}");
            }
            assert_eq!(copied.2.to_bits(), 0.0f32.to_bits(), "{css}");
            assert_eq!(copied.3.to_bits(), 0.0f32.to_bits(), "{css}");
            // Nonzero normal vertical absolutes remain literal controls.
            assert_eq!(copied.4, absolute(3.0, false, BoxFactOrigin::Specified));
            assert_eq!(copied.5, 3.0);
            assert_same_except_changed_vertical_facts(
                &css,
                &observation,
                &control,
                5,
                [Some(expected), Some(expected)],
                parent_margin,
            );
        });
    }
}

#[test]
fn vertical_auto_facade_priority_rows_have_later_fault_controls() {
    use CapturedClearanceUnsupported as U;
    use iced_css_core::formatting::clearance_owner::ChildFacts;
    use std::mem::size_of;
    const MAX: &str = "340282346638528859811704183484516925440px";
    let vertical = "#b1 { margin-bottom:auto; } #f { margin-top:auto; margin-bottom:auto; } \
                    #b2 { margin-top:auto; } #b3 { margin-bottom:auto; }";
    let run_with =
        |with_auto: bool,
         source_count: usize,
         rule: &str,
         outer: (f32, f32, f32),
         limits: SiblingLayoutLimits,
         check: &dyn Fn(Result<CapturedSiblingLayout<'_>, SiblingLayoutError>)| {
            let b4 = if source_count == 7 {
                "#b4 { height:2px; margin-top:auto; margin-bottom:2px; clear:none; }"
            } else {
                ""
            };
            let vertical = if with_auto { vertical } else { "" };
            let css = format!("{} {b4} {vertical} {rule}", baseline_css());
            sibling_fixture(source_count, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                check(default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    outer.0,
                    outer.1,
                    outer.2,
                    limits,
                ));
            });
        };
    let run =
        |source_count: usize,
         rule: &str,
         outer: (f32, f32, f32),
         limits: SiblingLayoutLimits,
         check: &dyn Fn(Result<CapturedSiblingLayout<'_>, SiblingLayoutError>)| {
            run_with(true, source_count, rule, outer, limits, check)
        };
    let normal = (0.0, 100.0, 10.0);

    // Source preflight precedes every Auto-bearing profile.
    let mut limits = layout_limits();
    limits.sources = nz(5);
    run(6, "", normal, limits, &|result| {
        assert!(matches!(result, Err(SiblingLayoutError::Sources)))
    });
    run(6, "", normal, layout_limits(), &|result| {
        assert!(result.is_ok())
    });

    // One-short facade work beside a later child overflow: Work first; the exact
    // bound reaches ChildOverflow on the actual last source.
    for (source_count, work, selector) in [(5, 594, "#b2"), (6, 766, "#b3"), (7, 938, "#b4")] {
        for (bound, overflow) in [(work - 1, false), (work, true)] {
            let mut limits = layout_limits();
            limits.work = nz(bound);
            let rule = format!("{selector} {{ overflow:hidden; }}");
            run(source_count, &rule, normal, limits, &|result| {
                if overflow {
                    assert!(matches!(
                        result,
                        Err(SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                            projected,
                            property: "overflow",
                            reason: U::ChildOverflow,
                        })) if projected == source_count - 1
                    ));
                } else {
                    assert!(matches!(result, Err(SiblingLayoutError::Work)));
                }
            });
        }
    }

    // One-short scratch with supported Auto and a later invalid height.
    for enough in [false, true] {
        let mut limits = layout_limits();
        if !enough {
            limits.scratch_bytes = nz(4 * size_of::<ChildFacts<'_>>() - 1);
        }
        run(6, "#b3 { height:auto; }", normal, limits, &|result| {
            if enough {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::Captured(
                        CapturedClearanceError::Unsupported {
                            projected: 5,
                            property: "height",
                            reason: U::HeightKind,
                        }
                    ))
                ));
            } else {
                assert!(matches!(result, Err(SiblingLayoutError::ScratchBytes)));
            }
        });
    }

    // F vertical Auto, later source 5 height:auto and F width:auto: complete
    // conversion reports source 5 height; removing it reaches source 3 width.
    run(
        6,
        "#f { width:auto; } #b3 { height:auto; }",
        normal,
        layout_limits(),
        &|result| {
            assert!(matches!(
                result,
                Err(SiblingLayoutError::Captured(
                    CapturedClearanceError::Unsupported {
                        projected: 5,
                        property: "height",
                        reason: U::HeightKind,
                    }
                ))
            ));
        },
    );
    run(
        6,
        "#f { width:auto; }",
        normal,
        layout_limits(),
        &|result| {
            assert!(matches!(
                result,
                Err(SiblingLayoutError::Captured(
                    CapturedClearanceError::Unsupported {
                        projected: 3,
                        property: "width",
                        reason: U::FloatWidth,
                    }
                ))
            ));
        },
    );

    // F vertical Auto plus parent top MAX+MAX: ParentOrigin BorderTop, since Auto no
    // longer blocks conversion; without the MAX origin the same CSS succeeds.
    let parent_max = format!("#p {{ margin-top:{MAX}; }}");
    for with_auto in [true, false] {
        run_with(
            with_auto,
            6,
            &parent_max,
            (0.0, 100.0, f32::MAX),
            layout_limits(),
            &|result| {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::ParentOrigin(
                        FirstChildOriginError::Unrepresentable {
                            stage: OriginCoordinateStage::BorderTop,
                        }
                    ))
                ));
            },
        );
    }
    run(6, "", normal, layout_limits(), &|result| {
        let layout = result.unwrap();
        assert!(matches!(
            layout.children()[1].margin_top(),
            BoxLengthFact::Auto { .. }
        ));
        assert_eq!(
            layout.children()[1].used_margin_top().to_bits(),
            0.0f32.to_bits()
        );
    });

    // Insufficient facade/core retained bytes with Auto profiles and width 140,
    // then the paired sufficient control succeeds with the oversized float.
    for (facade_short, core_short) in [(true, false), (false, true), (false, false)] {
        let mut limits = layout_limits();
        let floor =
            2 * size_of::<iced_css_core::formatting::clearance_sequence::SequenceNormalPlacement>();
        if facade_short {
            limits.retained_bytes = nz(floor
                + 4 * size_of::<iced_css::evaluation::sibling_layout::PrincipalChildBox>()
                - 1);
        }
        if core_short {
            limits.core.retained_bytes = nz(floor - 1);
        }
        run(6, "#f { width:140px; }", normal, limits, &|result| {
            if facade_short {
                assert!(matches!(result, Err(SiblingLayoutError::Bytes)));
            } else if core_short {
                assert!(matches!(
                    result,
                    Err(SiblingLayoutError::Core(ClearanceSequenceError::Bytes))
                ));
            } else {
                let layout = result.unwrap();
                assert_eq!(
                    (
                        layout.children()[1].content().left,
                        layout.children()[1].content().right
                    ),
                    (0.0, 140.0)
                );
            }
        });
    }

    // Finite float publication overflow at source 3 precedes source 4 equality.
    let all_auto = "#b1, #f, #b2, #b3, #b4 { margin-top:auto; margin-bottom:auto; }";
    for (side, float_failure) in [("left", true), ("right", false)] {
        let rule =
            format!("{all_auto} #o {{ width:0; }} #f {{ float:{side}; width:{MAX}; height:0; }}");
        run(
            6,
            &rule,
            (f32::MAX, 0.0, 10.0),
            layout_limits(),
            &|result| {
                if float_failure {
                    assert!(matches!(
                        result,
                        Err(SiblingLayoutError::Core(
                            ClearanceSequenceError::Placement {
                                source: 3,
                                error: ClearanceOwnerError::FloatPlacement(
                                    iced_css_core::formatting::floats::FloatError::Unrepresentable
                                ),
                            }
                        ))
                    ));
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
            },
        );
    }

    // All-Auto F height 0 with a prospective normal horizontal overflow: equality
    // precedes principal assembly; the clear:none control reaches the overflow.
    for (clear, equality) in [("both", true), ("none", false)] {
        let rule = format!(
            "{all_auto} #o {{ width:0; }} #f {{ float:right; width:140px; height:0; }} \
             #b2 {{ clear:{clear}; width:{MAX}; }}"
        );
        run(
            6,
            &rule,
            (f32::MAX, 0.0, 10.0),
            layout_limits(),
            &|result| {
                if equality {
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
            },
        );
    }

    // Final normal bottom Auto plus P height:auto succeeds with used zero at every
    // N; the explicit-height control is independent of that margin.
    // With the shared vertical rules: N5 B2 14..15, N6 B3 18..19, N7 B4 19..21.
    for (source_count, selector, content_bottom) in
        [(5, "#b2", 15.0), (6, "#b3", 19.0), (7, "#b4", 21.0)]
    {
        for (parent_height, expected_bottom) in [("auto", content_bottom), ("20px", 31.0)] {
            let rule =
                format!("{selector} {{ margin-bottom:auto; }} #p {{ height:{parent_height}; }}");
            run(source_count, &rule, normal, layout_limits(), &|result| {
                let layout = result.unwrap();
                let last = layout.children().last().unwrap();
                assert_eq!(last.source_index(), source_count - 1);
                assert!(matches!(last.margin_bottom(), BoxLengthFact::Auto { .. }));
                assert_eq!(last.used_margin_bottom().to_bits(), 0.0f32.to_bits());
                assert_eq!(layout.parent().height().content_bottom, expected_bottom);
                assert_eq!(
                    layout.parent().height().auto_extent.is_some(),
                    parent_height == "auto"
                );
            });
        }
    }
}

#[test]
fn vertical_auto_facade_work_and_storage_boundaries_are_exact() {
    use iced_css::evaluation::sibling_layout::PrincipalChildBox;
    use iced_css_core::formatting::{
        clearance_owner::ChildFacts, clearance_sequence::SequenceNormalPlacement,
    };
    use std::mem::size_of;

    for (source_count, facade_work, old_facade, core_work, old_core) in [
        (5, 594, 475, 481, 419),
        (6, 766, 616, 563, 501),
        (7, 938, 757, 646, 584),
    ] {
        let b4 = if source_count == 7 {
            "#b4 { height:2px; margin-top:auto; margin-bottom:auto; clear:none; }"
        } else {
            ""
        };
        let css = format!(
            "{} {b4} #b1, #f, #b2, #b3 {{ margin-top:auto; margin-bottom:auto; }} \
             #f {{ width:140px; margin-left:auto; margin-right:auto; }}",
            baseline_css()
        );
        sibling_fixture(source_count, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let build = |limits| {
                default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
            };
            let child_count = source_count - 2;
            let tail_count = source_count - 4;
            let scratch_floor = child_count * size_of::<ChildFacts<'_>>();
            let core_floor = tail_count * size_of::<SequenceNormalPlacement>();
            let retained_floor = core_floor + child_count * size_of::<PrincipalChildBox>();

            for short in [facade_work - 1, old_facade] {
                let mut limits = layout_limits();
                limits.work = nz(short);
                limits.scratch_bytes = nz(1);
                assert!(matches!(build(limits), Err(SiblingLayoutError::Work)));
            }
            for short in [core_work - 1, old_core] {
                let mut limits = layout_limits();
                limits.core.work = nz(short);
                assert!(matches!(
                    build(limits),
                    Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
                ));
            }
            let mut limits = layout_limits();
            limits.scratch_bytes = nz(scratch_floor - 1);
            assert!(matches!(
                build(limits),
                Err(SiblingLayoutError::ScratchBytes)
            ));
            let mut limits = layout_limits();
            limits.retained_bytes = nz(retained_floor - 1);
            assert!(matches!(build(limits), Err(SiblingLayoutError::Bytes)));
            let mut limits = layout_limits();
            limits.core.retained_bytes = nz(core_floor - 1);
            assert!(matches!(
                build(limits),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Bytes))
            ));

            let mut exact = layout_limits();
            exact.work = nz(facade_work);
            exact.core.work = nz(core_work);
            exact.scratch_bytes = nz(scratch_floor);
            exact.retained_bytes = nz(retained_floor);
            exact.core.retained_bytes = nz(core_floor);
            let layout = build(exact).unwrap();
            let receipt = layout.receipt();
            assert_eq!(receipt.nodes, source_count);
            assert_eq!(receipt.work, facade_work);
            assert_eq!(receipt.scratch_bytes, scratch_floor);
            assert_eq!(receipt.child_capacity, child_count);
            assert_eq!(receipt.retained_bytes, retained_floor);
            assert_eq!(receipt.core.work, core_work);
            assert_eq!(receipt.core.tail, tail_count);
            assert_eq!(receipt.core.record_capacity, tail_count);
            assert_eq!(receipt.core.retained_bytes, core_floor);
            for child in layout.children() {
                assert_eq!(child.used_margin_top().to_bits(), 0.0f32.to_bits());
                assert_eq!(child.used_margin_bottom().to_bits(), 0.0f32.to_bits());
            }
            assert_eq!(
                (
                    layout.children()[1].content().left,
                    layout.children()[1].content().right
                ),
                (0.0, 140.0)
            );
        });
    }
}

#[test]
fn vertical_auto_keeps_facade_exclusions_and_horizontal_categories() {
    use CapturedClearanceUnsupported as U;
    let captured = |projected, property, reason| {
        SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
            projected,
            property,
            reason,
        })
    };
    let profile = |source, reason| {
        SiblingLayoutError::Core(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported { source, reason },
        ))
    };
    for (rule, expected) in [
        (
            "#b3 { margin-top:auto; margin-bottom:-1px; }",
            profile(5, OwnerProfileReason::NegativeMargin),
        ),
        (
            "#b1 { margin-top:-1px; margin-bottom:auto; }",
            profile(2, OwnerProfileReason::NegativeMargin),
        ),
        (
            "#f { margin-top:auto; margin-bottom:-3px; }",
            profile(3, OwnerProfileReason::NegativeFloatOuterExtent),
        ),
        (
            "#f { margin-top:auto; margin-left:-11px; }",
            profile(3, OwnerProfileReason::NegativeFloatOuterExtent),
        ),
        (
            "#b3 { margin-top:auto; height:auto; }",
            captured(5, "height", U::HeightKind),
        ),
        (
            "#b2 { margin-bottom:auto; overflow:hidden; }",
            captured(4, "overflow", U::ChildOverflow),
        ),
        (
            "#f { margin-top:auto; width:auto; }",
            captured(3, "width", U::FloatWidth),
        ),
        (
            "#b3 { margin-top:auto; padding-top:1px; }",
            profile(5, OwnerProfileReason::ChildDecoration),
        ),
        (
            "#f { margin-bottom:auto; padding-bottom:1px; }",
            profile(3, OwnerProfileReason::ChildDecoration),
        ),
        (
            "#b1 { margin-bottom:auto; height:0; }",
            profile(2, OwnerProfileReason::NonpositiveBlockHeight),
        ),
    ] {
        let css = format!("{} {rule}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_err();
            assert_eq!(format!("{error:?}"), format!("{expected:?}"), "{rule}");
        });
    }

    // Leading clear beside a vertical Auto is admitted and inert: the layout equals
    // the same CSS without the `clear` declaration.
    for (control_rule, rule, source, value) in [
        (
            "#b1 { margin-top:auto; }",
            "#b1 { margin-top:auto; clear:left; }",
            2,
            ClearSide::Left,
        ),
        (
            "#f { margin-bottom:auto; }",
            "#f { margin-bottom:auto; clear:both; }",
            3,
            ClearSide::Both,
        ),
    ] {
        assert_leading_clear_case(
            6,
            control_rule,
            rule,
            &[(
                source,
                KeywordBoxFact {
                    value,
                    origin: SPECIFIED,
                },
            )],
        );
    }

    // Nonzero vertical float margins beside a vertical Auto are admitted:
    // (rule, margin box, F content), each as (left, right, top, bottom).
    for (rule, margin_box, content) in [
        (
            "#f { margin-top:auto; margin-bottom:1px; }",
            (0.0, 10.0, 16.0, 19.0),
            (0.0, 10.0, 16.0, 18.0),
        ),
        (
            "#f { margin-top:1px; margin-bottom:auto; }",
            (0.0, 10.0, 16.0, 19.0),
            (0.0, 10.0, 17.0, 19.0),
        ),
        (
            "#f { margin-bottom:auto; margin-right:1px; margin-top:2px; }",
            (0.0, 11.0, 16.0, 20.0),
            (0.0, 10.0, 18.0, 20.0),
        ),
    ] {
        let css = format!("{} {rule}", baseline_css());
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_or_else(|e| panic!("{rule}: {e:?}"));
            let placed = layout.sequence().placed_float();
            assert_eq!(
                (placed.left, placed.right, placed.top, placed.bottom),
                margin_box,
                "{rule}"
            );
            let c = layout.children()[1].content();
            assert_eq!((c.left, c.right, c.top, c.bottom), content, "{rule}");
        });
    }

    // A nonzero horizontal float margin beside a vertical Auto is admitted.
    let css = format!(
        "{} #f {{ margin-bottom:auto; margin-right:1px; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let placed = layout.sequence().placed_float();
        assert_eq!((placed.left, placed.right), (0.0, 11.0));
        let float = &layout.children()[1];
        assert_eq!((float.content().left, float.content().right), (0.0, 10.0));
        assert_eq!(float.used_margin_right().to_bits(), 0x3f80_0000);
    });

    // Signed and Auto normal horizontal categories plus F horizontal Auto remain
    // admitted beside every vertical Auto; F never receives a normal solver.
    let css = format!(
        "{} #b1, #f, #b2, #b3 {{ margin-top:auto; margin-bottom:auto; }} \
         #b1 {{ margin-left:-10px; margin-right:auto; }} \
         #f {{ margin-left:auto; margin-right:auto; }} \
         #b3 {{ width:40px; margin-left:auto; margin-right:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let children = layout.children();
        assert_eq!(
            children[0].normal_width(),
            Some(BlockWidthResult {
                width: 110.0,
                margin_left: -10.0,
                margin_right: 0.0,
                passes: 1,
            })
        );
        assert_eq!(children[1].normal_width(), None);
        assert_eq!(
            (children[1].content().left, children[1].content().right),
            (0.0, 10.0)
        );
        assert_eq!(
            children[3].normal_width(),
            Some(BlockWidthResult {
                width: 40.0,
                margin_left: 30.0,
                margin_right: 30.0,
                passes: 1,
            })
        );
        assert_eq!(
            vertical_spans(&layout),
            [
                (2, 11.0, 12.0),
                (3, 12.0, 14.0),
                (4, 14.0, 15.0),
                (5, 15.0, 16.0)
            ]
        );
        assert_eq!(layout.parent().height().content_bottom, 16.0);
    });
}

fn zero_margin_control_view(layout: &CapturedSiblingLayout<'_>) -> Vec<String> {
    let sequence = layout.sequence();
    let placed = sequence.placed_float();
    let mut view = vec![
        format!("{:?}", sequence.before()),
        format!("{:?}", sequence.tail()),
        format!("{:?}", (placed.side, placed.top, placed.bottom)),
        format!("{:?}", layout.parent()),
    ];
    for child in layout.children() {
        if child.source_index() == 3 {
            let content = child.content();
            // Two tuples: std implements Debug for tuples of at most 12 elements.
            view.push(format!(
                "{:?}",
                (
                    child.kind(),
                    content.top,
                    content.bottom,
                    child.used_width(),
                    child.normal_width(),
                    child.used_height(),
                    child.used_margin_top(),
                ),
            ));
            view.push(format!(
                "{:?}",
                (
                    child.used_margin_bottom(),
                    child.computed_width(),
                    child.computed_height(),
                    child.margin_top(),
                    child.margin_bottom(),
                    child.clear(),
                ),
            ));
        } else {
            view.push(format!("{child:?}"));
        }
    }
    view
}

fn assert_nonzero_float_case(
    rule: &str,
    outer: (f32, f32),
    margin_box: (u32, u32),
    content: (u32, u32),
    used_margins: (u32, u32),
) {
    let control_css = format!(
        "{} {rule} #f {{ margin-left:0; margin-right:0; }}",
        baseline_css()
    );
    let mut control = None;
    sibling_fixture(6, &control_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            outer.0,
            outer.1,
            10.0,
            layout_limits(),
        )
        .unwrap();
        control = Some(zero_margin_control_view(&layout));
    });
    let css = format!("{} {rule}", baseline_css());
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            outer.0,
            outer.1,
            10.0,
            layout_limits(),
        )
        .unwrap();
        let placed = layout.sequence().placed_float();
        assert_eq!((placed.left.to_bits(), placed.right.to_bits()), margin_box);
        let band = layout.sequence().float_content();
        assert_eq!((band.left.to_bits(), band.right.to_bits()), content);
        let float = &layout.children()[1];
        assert_eq!(float.source_index(), 3);
        assert_eq!(
            (
                float.content().left.to_bits(),
                float.content().right.to_bits()
            ),
            content
        );
        assert_eq!(
            (
                float.used_margin_left().to_bits(),
                float.used_margin_right().to_bits()
            ),
            used_margins,
        );
        assert!(float.normal_width().is_none());
        assert_eq!(Some(zero_margin_control_view(&layout)), control);
    });
}

#[test]
fn nonzero_left_float_horizontal_margins_publish_the_content_band() {
    assert_nonzero_float_case(
        "#f { width:30px; margin-left:5px; margin-right:7px; }",
        (0.0, 100.0),
        (0.0_f32.to_bits(), 42.0_f32.to_bits()),
        (5.0_f32.to_bits(), 35.0_f32.to_bits()),
        (0x40a0_0000, 0x40e0_0000),
    );
}

#[test]
fn nonzero_right_float_horizontal_margins_publish_the_content_band() {
    assert_nonzero_float_case(
        "#f { float:right; width:30px; margin-left:5px; margin-right:7px; }",
        (0.0, 100.0),
        (58.0_f32.to_bits(), 100.0_f32.to_bits()),
        (63.0_f32.to_bits(), 93.0_f32.to_bits()),
        (0x40a0_0000, 0x40e0_0000),
    );
}

#[derive(Debug, Clone, Copy)]
enum NfhOrigin {
    Specified,
    Initial,
    Inherited,
}

/// Expected computed length fact: exact pixel bits, rounded-to-zero flag, origin.
type NfhFact = (u32, bool, NfhOrigin);

fn nfh_fact(fact: NfhFact, original_parent: usize) -> BoxLengthFact {
    BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels: f32::from_bits(fact.0),
        rounded_to_zero: fact.1,
        origin: match fact.2 {
            NfhOrigin::Specified => BoxFactOrigin::Specified,
            NfhOrigin::Initial => BoxFactOrigin::Initial,
            NfhOrigin::Inherited => BoxFactOrigin::Inherited { original_parent },
        },
    })
}

/// Evaluates the targeted input through real CSS and checks F's computed width and
/// horizontal margin facts (exact bits via Debug), then returns the layout result
/// as a Debug string for failures or None for successes.
fn nfh_targeted_facts(
    rule: &str,
    outer: (f32, f32),
    width_bits: u32,
    left: NfhFact,
    right: NfhFact,
) -> Option<String> {
    let css = format!("{} {rule}", baseline_css());
    let mut failure = None;
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let original_parent = projection.original_index(1).unwrap();
        let row = facts.row(3).unwrap();
        let width = nfh_fact((width_bits, false, NfhOrigin::Specified), original_parent);
        assert_eq!(format!("{:?}", row.width), format!("{width:?}"), "{rule}");
        let expected_left = nfh_fact(left, original_parent);
        let expected_right = nfh_fact(right, original_parent);
        assert_eq!(
            format!("{:?}", row.margin[3]),
            format!("{expected_left:?}"),
            "{rule}"
        );
        assert_eq!(
            format!("{:?}", row.margin[1]),
            format!("{expected_right:?}"),
            "{rule}"
        );
        match default_outer_layout(
            &facts,
            &projection.nodes()[0],
            outer.0,
            outer.1,
            10.0,
            layout_limits(),
        ) {
            Ok(layout) => {
                let float = &layout.children()[1];
                assert_eq!(float.used_width().to_bits(), width_bits, "{rule}");
                assert_eq!(
                    format!("{:?}", float.computed_margin_left()),
                    format!("{expected_left:?}")
                );
                assert_eq!(
                    format!("{:?}", float.computed_margin_right()),
                    format!("{expected_right:?}")
                );
            }
            Err(SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error,
            })) => failure = Some(format!("{error:?}")),
            Err(other) => panic!("{rule}: unexpected {other:?}"),
        }
    });
    failure
}

#[allow(clippy::too_many_arguments)]
fn assert_nfh_targeted_success(
    rule: &str,
    outer: (f32, f32),
    width_bits: u32,
    left: NfhFact,
    right: NfhFact,
    margin_box: (u32, u32),
    content: (u32, u32),
    used_margins: (u32, u32),
) {
    assert_eq!(
        nfh_targeted_facts(rule, outer, width_bits, left, right),
        None
    );
    assert_nonzero_float_case(rule, outer, margin_box, content, used_margins);
}

fn assert_nfh_targeted_failure(
    rule: &str,
    outer: (f32, f32),
    width_bits: u32,
    left: NfhFact,
    right: NfhFact,
    expected: &str,
) {
    assert_eq!(
        nfh_targeted_facts(rule, outer, width_bits, left, right).as_deref(),
        Some(expected)
    );
}

#[test]
fn nfh_targeted_band_collapsed_left() {
    assert_nfh_targeted_success(
        "#f { width:1px; margin-left:1px; margin-right:1px; }",
        (f32::from_bits(0x4b800000), f32::from_bits(0x00000000)),
        0x3f800000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x3f800000, false, NfhOrigin::Specified),
        (0x4b800000, 0x4b800002),
        (0x4b800000, 0x4b800000),
        (0x3f800000, 0x3f800000),
    );
}

#[test]
fn nfh_targeted_band_rounded_left() {
    assert_nfh_targeted_success(
        "#f { width:5px; margin-left:1px; margin-right:1px; }",
        (f32::from_bits(0x4b800000), f32::from_bits(0x40800000)),
        0x40a00000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x3f800000, false, NfhOrigin::Specified),
        (0x4b800000, 0x4b800004),
        (0x4b800000, 0x4b800002),
        (0x3f800000, 0x3f800000),
    );
}

#[test]
fn nfh_targeted_band_collapsed_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:1px; margin-left:1px; margin-right:1px; }",
        (f32::from_bits(0x4b800000), f32::from_bits(0x00000000)),
        0x3f800000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x3f800000, false, NfhOrigin::Specified),
        (0x4b7ffffd, 0x4b800000),
        (0x4b7ffffe, 0x4b7fffff),
        (0x3f800000, 0x3f800000),
    );
}

#[test]
fn nfh_targeted_band_rounded_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:5px; margin-left:1px; margin-right:1px; }",
        (f32::from_bits(0x4b800000), f32::from_bits(0x40800000)),
        0x40a00000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x3f800000, false, NfhOrigin::Specified),
        (0x4b7ffffd, 0x4b800002),
        (0x4b7fffff, 0x4b800002),
        (0x3f800000, 0x3f800000),
    );
}

#[test]
fn nfh_targeted_inherited_left() {
    assert_nfh_targeted_success(
        "#p { margin-left:5px; } #f { width:30px; margin-left:inherit; margin-right:7px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x40a00000, false, NfhOrigin::Inherited),
        (0x40e00000, false, NfhOrigin::Specified),
        (0x40a00000, 0x423c0000),
        (0x41200000, 0x42200000),
        (0x40a00000, 0x40e00000),
    );
}

#[test]
fn nfh_targeted_inherited_right() {
    assert_nfh_targeted_success(
        "#p { margin-right:5px; } #f { float:right; width:30px; margin-left:7px; margin-right:inherit; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x40e00000, false, NfhOrigin::Specified),
        (0x40a00000, false, NfhOrigin::Inherited),
        (0x42540000, 0x42be0000),
        (0x42700000, 0x42b40000),
        (0x40e00000, 0x40a00000),
    );
}

#[test]
fn nfh_targeted_rounded_to_zero_left() {
    assert_nfh_targeted_success(
        "#f { width:30px; margin-left:0.0000000000000000000000000000000000000000000000000001px; margin-right:7px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x00000000, true, NfhOrigin::Specified),
        (0x40e00000, false, NfhOrigin::Specified),
        (0x00000000, 0x42140000),
        (0x00000000, 0x41f00000),
        (0x00000000, 0x40e00000),
    );
}

#[test]
fn nfh_targeted_rounded_to_zero_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:30px; margin-left:5px; margin-right:0.0000000000000000000000000000000000000000000000000001px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x40a00000, false, NfhOrigin::Specified),
        (0x00000000, true, NfhOrigin::Specified),
        (0x42820000, 0x42c80000),
        (0x428c0000, 0x42c80000),
        (0x40a00000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_initial_left() {
    assert_nfh_targeted_success(
        "#f { width:30px; margin-right:7px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x00000000, false, NfhOrigin::Initial),
        (0x40e00000, false, NfhOrigin::Specified),
        (0x00000000, 0x42140000),
        (0x00000000, 0x41f00000),
        (0x00000000, 0x40e00000),
    );
}

#[test]
fn nfh_targeted_initial_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:30px; margin-left:5px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x40a00000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        (0x42820000, 0x42c80000),
        (0x428c0000, 0x42c80000),
        (0x40a00000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_outer_width_overflow_left() {
    assert_nfh_targeted_failure(
        "#f { width:340282346638528859811704183484516925440px; margin-left:10141204801825835211973625643008px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x7f7fffff,
        (0x73000000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        "Unrepresentable { stage: \"float_outer_width\" }",
    );
}

#[test]
fn nfh_targeted_outer_absorbs_2p102_left() {
    assert_nfh_targeted_success(
        "#f { width:340282346638528859811704183484516925440px; margin-left:5070602400912917605986812821504px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x7f7fffff,
        (0x72800000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        (0x00000000, 0x7f7fffff),
        (0x72800000, 0x7f7fffff),
        (0x72800000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_margin_box_overflow_left() {
    assert_nfh_targeted_failure(
        "#f { width:10141204801825835211973625643008px; margin-left:1px; }",
        (f32::from_bits(0x7f7fffff), f32::from_bits(0x00000000)),
        0x73000000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        "FloatPlacement(Unrepresentable)",
    );
}

#[test]
fn nfh_targeted_outer_width_overflow_right() {
    assert_nfh_targeted_failure(
        "#f { float:right; width:340282346638528859811704183484516925440px; margin-left:10141204801825835211973625643008px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x7f7fffff,
        (0x73000000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        "Unrepresentable { stage: \"float_outer_width\" }",
    );
}

#[test]
fn nfh_targeted_outer_absorbs_2p102_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:340282346638528859811704183484516925440px; margin-left:5070602400912917605986812821504px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x7f7fffff,
        (0x72800000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        (0xff7fffff, 0x42c80000),
        (0xff7fffff, 0x42c80000),
        (0x72800000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_margin_box_anchored_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:10141204801825835211973625643008px; margin-left:1px; }",
        (f32::from_bits(0x7f7fffff), f32::from_bits(0x00000000)),
        0x73000000,
        (0x3f800000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        (0x7f7ffffe, 0x7f7fffff),
        (0x7f7ffffe, 0x7f7fffff),
        (0x3f800000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_far_edge_overflow_left() {
    assert_nfh_targeted_failure(
        "#f { width:10141204801825835211973625643008px; margin-left:10141206010751654826602800349184px; }",
        (f32::from_bits(0x7f7ffffe), f32::from_bits(0x00000000)),
        0x73000000,
        (0x73000001, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        "Unrepresentable { stage: \"float_content_right\" }",
    );
}

#[test]
fn nfh_targeted_far_edge_overflow_right() {
    assert_nfh_targeted_failure(
        "#f { float:right; width:10141204801825835211973625643008px; margin-right:10141206010751654826602800349184px; }",
        (f32::from_bits(0xff7ffffe), f32::from_bits(0x00000000)),
        0x73000000,
        (0x00000000, false, NfhOrigin::Initial),
        (0x73000001, false, NfhOrigin::Specified),
        "Unrepresentable { stage: \"float_content_left\" }",
    );
}

#[test]
fn nfh_targeted_negative_zero_left() {
    assert_nfh_targeted_success(
        "#f { width:30px; margin-left:-0px; margin-right:-0px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x00000000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Specified),
        (0x00000000, 0x41f00000),
        (0x00000000, 0x41f00000),
        (0x00000000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_negative_zero_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:30px; margin-left:-0px; margin-right:-0px; }",
        (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        0x41f00000,
        (0x00000000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Specified),
        (0x428c0000, 0x42c80000),
        (0x428c0000, 0x42c80000),
        (0x00000000, 0x00000000),
    );
}

#[test]
fn nfh_targeted_non_containment_right() {
    assert_nfh_targeted_success(
        "#f { float:right; width:1px; margin-right:1.5px; }",
        (f32::from_bits(0x4b7ffffc), f32::from_bits(0x00000000)),
        0x3f800000,
        (0x00000000, false, NfhOrigin::Initial),
        (0x3fc00000, false, NfhOrigin::Specified),
        (0x4b7ffffa, 0x4b7ffffc),
        (0x4b7ffff9, 0x4b7ffffa),
        (0x00000000, 0x3fc00000),
    );
}

#[test]
fn nfh_targeted_non_containment_left() {
    assert_nfh_targeted_success(
        "#f { width:1px; margin-left:1.5px; }",
        (f32::from_bits(0x4b7ffffc), f32::from_bits(0x00000000)),
        0x3f800000,
        (0x3fc00000, false, NfhOrigin::Specified),
        (0x00000000, false, NfhOrigin::Initial),
        (0x4b7ffffc, 0x4b7ffffe),
        (0x4b7ffffe, 0x4b7fffff),
        (0x3fc00000, 0x00000000),
    );
}

#[test]
fn nonzero_float_margin_facade_work_and_storage_boundaries_are_exact() {
    use iced_css::evaluation::sibling_layout::PrincipalChildBox;
    use iced_css_core::formatting::{
        clearance_owner::ChildFacts, clearance_sequence::SequenceNormalPlacement,
    };
    use std::mem::size_of;

    for (source_count, facade_work, old_facade, core_work, old_core) in [
        (5, 594, 475, 481, 419),
        (6, 766, 616, 563, 501),
        (7, 938, 757, 646, 584),
    ] {
        let b4 = if source_count == 7 {
            "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
        } else {
            ""
        };
        let css = format!(
            "{} {b4} #f {{ margin-left:5px; margin-right:7px; }}",
            baseline_css()
        );
        sibling_fixture(source_count, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let build = |limits| {
                default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
            };
            let child_count = source_count - 2;
            let tail_count = source_count - 4;
            let scratch_floor = child_count * size_of::<ChildFacts<'_>>();
            let core_floor = tail_count * size_of::<SequenceNormalPlacement>();
            let retained_floor = core_floor + child_count * size_of::<PrincipalChildBox>();

            // One short and the predecessor bound fail before the scratch scan.
            for short in [facade_work - 1, old_facade] {
                let mut limits = layout_limits();
                limits.work = nz(short);
                limits.scratch_bytes = nz(1);
                assert!(matches!(build(limits), Err(SiblingLayoutError::Work)));
            }
            for short in [core_work - 1, old_core] {
                let mut limits = layout_limits();
                limits.core.work = nz(short);
                assert!(matches!(
                    build(limits),
                    Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
                ));
            }
            let mut exact = layout_limits();
            exact.work = nz(facade_work);
            exact.core.work = nz(core_work);
            exact.scratch_bytes = nz(scratch_floor);
            exact.retained_bytes = nz(retained_floor);
            exact.core.retained_bytes = nz(core_floor);
            let layout = build(exact).unwrap();
            let receipt = layout.receipt();
            assert_eq!(receipt.work, facade_work);
            assert_eq!(receipt.scratch_bytes, scratch_floor);
            assert_eq!(receipt.retained_bytes, retained_floor);
            assert_eq!(receipt.core.work, core_work);
            assert_eq!(receipt.core.retained_bytes, core_floor);
            let placed = layout.sequence().placed_float();
            assert_eq!((placed.left, placed.right), (0.0, 22.0));
            let float = &layout.children()[1];
            assert_eq!((float.content().left, float.content().right), (5.0, 15.0));
            assert_eq!(float.used_margin_left().to_bits(), 0x40a0_0000);
            assert_eq!(float.used_margin_right().to_bits(), 0x40e0_0000);
        });
    }
}

// Everything except F's content top/bottom, used height, used/computed vertical
// margins and computed height. The F tuple has 12 elements, the std Debug limit.
fn float_vertical_view(layout: &CapturedSiblingLayout<'_>) -> Vec<String> {
    let sequence = layout.sequence();
    let mut view = vec![
        format!("{:?}", sequence.before()),
        format!("{:?}", sequence.tail()),
        format!("{:?}", sequence.placed_float()),
        format!("{:?}", sequence.float_content()),
        format!("{:?}", layout.parent()),
        format!("{:?}", layout.receipt()),
    ];
    for child in layout.children() {
        if child.source_index() == 3 {
            let content = child.content();
            view.push(format!(
                "{:?}",
                (
                    child.kind(),
                    child.containing_source_index(),
                    content.left,
                    content.right,
                    child.used_width(),
                    child.normal_width(),
                    child.used_margin_left(),
                    child.used_margin_right(),
                    child.computed_width(),
                    child.computed_margin_left(),
                    child.computed_margin_right(),
                    child.clear(),
                )
            ));
        } else {
            view.push(format!("{child:?}"));
        }
    }
    view
}

fn assert_float_vertical_case(
    source_count: usize,
    rule: &str,
    equivalent_rule: &str,
    margin_box: (f32, f32, f32, f32),
    content: (f32, f32, f32, f32),
    used_vertical: (u32, u32),
    parent: (f32, f32, bool),
) {
    let bits =
        |v: (f32, f32, f32, f32)| (v.0.to_bits(), v.1.to_bits(), v.2.to_bits(), v.3.to_bits());
    let control_css = format!("{} {rule} {equivalent_rule}", baseline_css());
    let mut control = None;
    sibling_fixture(source_count, &control_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap();
        control = Some(float_vertical_view(&layout));
    });
    let css = format!("{} {rule}", baseline_css());
    sibling_fixture(source_count, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap_or_else(|e| panic!("{rule}: {e:?}"));
        let placed = layout.sequence().placed_float();
        assert_eq!(
            bits((placed.left, placed.right, placed.top, placed.bottom)),
            bits(margin_box),
            "{rule}"
        );
        let span = layout.sequence().float_content_vertical();
        assert_eq!(
            (span.top.to_bits(), span.bottom.to_bits()),
            (content.2.to_bits(), content.3.to_bits()),
            "{rule}"
        );
        let float = &layout.children()[1];
        assert_eq!(float.source_index(), 3, "{rule}");
        let c = float.content();
        assert_eq!(
            bits((c.left, c.right, c.top, c.bottom)),
            bits(content),
            "{rule}"
        );
        assert_eq!(
            (
                float.used_margin_top().to_bits(),
                float.used_margin_bottom().to_bits()
            ),
            used_vertical,
            "{rule}"
        );
        assert_eq!(float.used_height().to_bits(), 2.0_f32.to_bits(), "{rule}");
        let height = layout.parent().height();
        let extent = height.auto_extent.unwrap();
        assert_eq!(
            (
                height.content_bottom.to_bits(),
                height.used_height.to_bits(),
                extent.float_extended
            ),
            (parent.0.to_bits(), parent.1.to_bits(), parent.2),
            "{rule}"
        );
        assert_eq!(Some(float_vertical_view(&layout)), control, "{rule}");
    });
}

#[test]
fn nonzero_left_float_vertical_margins_publish_the_content_span() {
    assert_float_vertical_case(
        6,
        "#f { margin-top:3px; margin-bottom:4px; }",
        "#f { margin-top:0; margin-bottom:0; height:9px; }",
        (0.0, 10.0, 16.0, 25.0),
        (0.0, 10.0, 19.0, 21.0),
        (0x4040_0000, 0x4080_0000),
        (34.0, 23.0, false),
    );
}

#[test]
fn nonzero_right_float_vertical_margins_publish_the_content_span() {
    assert_float_vertical_case(
        6,
        "#f { float:right; margin-top:3px; margin-bottom:4px; }",
        "#f { margin-top:0; margin-bottom:0; height:9px; }",
        (90.0, 100.0, 16.0, 25.0),
        (90.0, 100.0, 19.0, 21.0),
        (0x4040_0000, 0x4080_0000),
        (34.0, 23.0, false),
    );
}

#[test]
fn left_float_bottom_margin_alone_extends_parent_auto_height() {
    assert_float_vertical_case(
        5,
        "#b2 { clear:none; } #f { margin-bottom:20px; }",
        "#f { margin-bottom:0; height:22px; }",
        (0.0, 10.0, 16.0, 38.0),
        (0.0, 10.0, 16.0, 18.0),
        (0x0000_0000, 0x41a0_0000),
        (38.0, 27.0, true),
    );
}

#[test]
fn right_float_bottom_margin_alone_extends_parent_auto_height() {
    assert_float_vertical_case(
        5,
        "#b2 { clear:none; } #f { float:right; margin-bottom:20px; }",
        "#f { margin-bottom:0; height:22px; }",
        (90.0, 100.0, 16.0, 38.0),
        (90.0, 100.0, 16.0, 18.0),
        (0x0000_0000, 0x41a0_0000),
        (38.0, 27.0, true),
    );
}

/// One targeted FVM success row: expected exact bits from the design model.
struct FvmSuccess {
    nodes: usize,
    rule: &'static str,
    control: &'static str,
    /// F computed height, margin-top and margin-bottom facts.
    facts: [NfhFact; 3],
    /// Margin box and F content as left, right, top, bottom.
    margin_box: [u32; 4],
    content: [u32; 4],
    used_vertical: [u32; 2],
    used_height: u32,
    /// P content bottom, used height and float extension.
    parent: (u32, u32, bool),
}

/// One targeted FVM failure row: typed source-tagged placement failure.
struct FvmFailure {
    nodes: usize,
    rule: &'static str,
    outer: (f32, f32),
    facts: [NfhFact; 3],
    source: usize,
    error: &'static str,
}

fn assert_fvm_facts(
    facts: &iced_css::evaluation::sibling_box_facts::SiblingBoxFacts<'_, '_, '_, '_, '_, '_>,
    original_parent: usize,
    rule: &str,
    expected: [NfhFact; 3],
) {
    let row = facts.row(3).unwrap();
    let [height, top, bottom] =
        expected.map(|fact| format!("{:?}", nfh_fact(fact, original_parent)));
    assert_eq!(format!("{:?}", row.height), height, "{rule}");
    assert_eq!(format!("{:?}", row.margin[0]), top, "{rule}");
    assert_eq!(format!("{:?}", row.margin[2]), bottom, "{rule}");
}

fn assert_fvm_targeted_success(case: FvmSuccess) {
    let rule = case.rule;
    let control_css = format!("{} {rule} {}", baseline_css(), case.control);
    let mut control = None;
    sibling_fixture(case.nodes, &control_css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap_or_else(|e| panic!("control {rule}: {e:?}"));
        control = Some(float_vertical_view(&layout));
    });
    let css = format!("{} {rule}", baseline_css());
    sibling_fixture(case.nodes, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let original_parent = projection.original_index(1).unwrap();
        assert_fvm_facts(&facts, original_parent, rule, case.facts);
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap_or_else(|e| panic!("{rule}: {e:?}"));
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            case.margin_box,
            "{rule}"
        );
        let float = &layout.children()[1];
        assert_eq!(float.source_index(), 3, "{rule}");
        let c = float.content();
        assert_eq!(
            [c.left, c.right, c.top, c.bottom].map(f32::to_bits),
            case.content,
            "{rule}"
        );
        let span = layout.sequence().float_content_vertical();
        assert_eq!(
            [span.top.to_bits(), span.bottom.to_bits()],
            [case.content[2], case.content[3]],
            "{rule}"
        );
        assert_eq!(
            [
                float.used_margin_top().to_bits(),
                float.used_margin_bottom().to_bits()
            ],
            case.used_vertical,
            "{rule}"
        );
        assert_eq!(float.used_height().to_bits(), case.used_height, "{rule}");
        let row = facts.row(3).unwrap();
        assert_eq!(float.computed_height(), row.height, "{rule}");
        assert_eq!(
            format!("{:?}", (float.margin_top(), float.margin_bottom())),
            format!("{:?}", (row.margin[0], row.margin[2])),
            "{rule}"
        );
        let height = layout.parent().height();
        let extent = height.auto_extent.unwrap();
        assert_eq!(
            (
                height.content_bottom.to_bits(),
                height.used_height.to_bits(),
                extent.float_extended
            ),
            case.parent,
            "{rule}"
        );
        assert_eq!(Some(float_vertical_view(&layout)), control, "{rule}");
    });
}

fn assert_fvm_targeted_failure(case: FvmFailure) {
    let rule = case.rule;
    let css = format!("{} {rule}", baseline_css());
    sibling_fixture(case.nodes, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let original_parent = projection.original_index(1).unwrap();
        assert_fvm_facts(&facts, original_parent, rule, case.facts);
        match default_outer_layout(
            &facts,
            &projection.nodes()[0],
            case.outer.0,
            case.outer.1,
            10.0,
            layout_limits(),
        ) {
            Err(SiblingLayoutError::Core(ClearanceSequenceError::Placement { source, error })) => {
                assert_eq!(
                    (source, format!("{error:?}")),
                    (case.source, case.error.to_string()),
                    "{rule}"
                );
            }
            other => panic!("{rule}: expected a placement failure, got {other:?}"),
        }
    });
}

// FVM targeted inputs, generated by targeted_gen.py from targeted-inputs.json
// (sha256 9b5e5e94ace5e0c9a89303d9005e98e9e219e2c23f912389876582df1a8a345b).

#[test]
fn fvm_targeted_00_clearance_boundary_b2mt10_mt1_mb2_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:2px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:5px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41a80000],
        content: [0x00000000, 0x41200000, 0x41880000, 0x41980000],
        used_vertical: [0x3f800000, 0x40000000],
        used_height: 0x40000000,
        parent: (0x41b80000, 0x41400000, false),
    });
}

#[test]
fn fvm_targeted_01_clearance_boundary_b2mt10_mt1_mb3_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:3px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Specified),
        ],
        source: 4,
        error: "UnsupportedEquality",
    });
}

#[test]
fn fvm_targeted_02_clearance_boundary_b2mt10_mt1_mb4_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:4px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:7px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41b80000],
        content: [0x00000000, 0x41200000, 0x41880000, 0x41980000],
        used_vertical: [0x3f800000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41c00000, 0x41500000, false),
    });
}

#[test]
fn fvm_targeted_03_clearance_boundary_b2mt10_mt1_mb2_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:2px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:5px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41a80000],
        content: [0x42b40000, 0x42c80000, 0x41880000, 0x41980000],
        used_vertical: [0x3f800000, 0x40000000],
        used_height: 0x40000000,
        parent: (0x41b80000, 0x41400000, false),
    });
}

#[test]
fn fvm_targeted_04_clearance_boundary_b2mt10_mt1_mb3_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:3px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Specified),
        ],
        source: 4,
        error: "UnsupportedEquality",
    });
}

#[test]
fn fvm_targeted_05_clearance_boundary_b2mt10_mt1_mb4_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:4px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:7px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41b80000],
        content: [0x42b40000, 0x42c80000, 0x41880000, 0x41980000],
        used_vertical: [0x3f800000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41c00000, 0x41500000, false),
    });
}

#[test]
fn fvm_targeted_06_zero_height_h0_mt1_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-bottom:0px;} #f{height:0px;margin-top:1px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:1px;}",
        facts: [
            (0x00000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41880000],
        content: [0x00000000, 0x41200000, 0x41880000, 0x41880000],
        used_vertical: [0x3f800000, 0x00000000],
        used_height: 0x00000000,
        parent: (0x41900000, 0x40e00000, false),
    });
}

#[test]
fn fvm_targeted_07_zero_height_h0_mt1_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{margin-bottom:0px;} #f{height:0px;margin-top:1px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:1px;}",
        facts: [
            (0x00000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41880000],
        content: [0x42b40000, 0x42c80000, 0x41880000, 0x41880000],
        used_vertical: [0x3f800000, 0x00000000],
        used_height: 0x00000000,
        parent: (0x41900000, 0x40e00000, false),
    });
}

#[test]
fn fvm_targeted_08_float_extension_mb20_clear_none_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{clear:none;margin-bottom:0px;} #f{margin-bottom:20px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:22px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x41a00000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x42180000],
        content: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x41a00000],
        used_height: 0x40000000,
        parent: (0x42180000, 0x41d80000, true),
    });
}

#[test]
fn fvm_targeted_09_float_extension_mb20_clear_none_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{clear:none;margin-bottom:0px;} #f{margin-bottom:20px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:22px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x41a00000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x42180000],
        content: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x41a00000],
        used_height: 0x40000000,
        parent: (0x42180000, 0x41d80000, true),
    });
}

#[test]
fn fvm_targeted_10_clear_mismatch_left_f_b2_clear_right_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{clear:right;margin-bottom:0px;} #f{margin-bottom:20px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:22px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x41a00000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x42180000],
        content: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x41a00000],
        used_height: 0x40000000,
        parent: (0x42180000, 0x41d80000, true),
    });
}

#[test]
fn fvm_targeted_11_clear_mismatch_right_f_b2_clear_left_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 5,
        rule: "#b2{clear:left;margin-bottom:0px;} #f{margin-bottom:20px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:22px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x41a00000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x42180000],
        content: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x41a00000],
        used_height: 0x40000000,
        parent: (0x42180000, 0x41d80000, true),
    });
}

#[test]
fn fvm_targeted_12_2p24_collapsed_1_1_1_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1px;height:1px;margin-bottom:1px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:3px;}",
        facts: [
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x4b800000, 0x4b800002],
        content: [0x00000000, 0x41200000, 0x4b800000, 0x4b800000],
        used_vertical: [0x3f800000, 0x3f800000],
        used_height: 0x3f800000,
        parent: (0x4b800006, 0x4b800000, false),
    });
}

#[test]
fn fvm_targeted_13_2p24_rounded_1_5_1_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1px;height:5px;margin-bottom:1px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:7px;}",
        facts: [
            (0x40a00000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x4b800000, 0x4b800004],
        content: [0x00000000, 0x41200000, 0x4b800000, 0x4b800002],
        used_vertical: [0x3f800000, 0x3f800000],
        used_height: 0x40a00000,
        parent: (0x4b800008, 0x4b800002, false),
    });
}

#[test]
fn fvm_targeted_14_2p24_non_containment_1_5_1_0_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1.5px;height:1px;margin-bottom:0px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:2.5px;}",
        facts: [
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3fc00000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x4b800000, 0x4b800001],
        content: [0x00000000, 0x41200000, 0x4b800001, 0x4b800002],
        used_vertical: [0x3fc00000, 0x00000000],
        used_height: 0x3f800000,
        parent: (0x4b800006, 0x4b800000, false),
    });
}

#[test]
fn fvm_targeted_15_2p24_collapsed_1_1_1_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1px;height:1px;margin-bottom:1px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:3px;}",
        facts: [
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x4b800000, 0x4b800002],
        content: [0x42b40000, 0x42c80000, 0x4b800000, 0x4b800000],
        used_vertical: [0x3f800000, 0x3f800000],
        used_height: 0x3f800000,
        parent: (0x4b800006, 0x4b800000, false),
    });
}

#[test]
fn fvm_targeted_16_2p24_rounded_1_5_1_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1px;height:5px;margin-bottom:1px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:7px;}",
        facts: [
            (0x40a00000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x4b800000, 0x4b800004],
        content: [0x42b40000, 0x42c80000, 0x4b800000, 0x4b800002],
        used_vertical: [0x3f800000, 0x3f800000],
        used_height: 0x40a00000,
        parent: (0x4b800008, 0x4b800002, false),
    });
}

#[test]
fn fvm_targeted_17_2p24_non_containment_1_5_1_0_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#b1{margin-bottom:16777204px;} #f{margin-top:1.5px;height:1px;margin-bottom:0px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:2.5px;}",
        facts: [
            (0x3f800000, false, NfhOrigin::Specified),
            (0x3fc00000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x4b800000, 0x4b800001],
        content: [0x42b40000, 0x42c80000, 0x4b800001, 0x4b800002],
        used_vertical: [0x3fc00000, 0x00000000],
        used_height: 0x3f800000,
        parent: (0x4b800006, 0x4b800000, false),
    });
}

#[test]
fn fvm_targeted_18_provenance_inherited_3_4_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#p{margin-top:3px;margin-bottom:4px;} #f{margin-top:inherit;margin-bottom:inherit;}",
        control: "#f{margin-top:0;margin-bottom:0;height:9px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Inherited),
            (0x40800000, false, NfhOrigin::Inherited),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41980000, 0x41e00000],
        content: [0x00000000, 0x41200000, 0x41b00000, 0x41c00000],
        used_vertical: [0x40400000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x42140000, 0x41b80000, false),
    });
}

#[test]
fn fvm_targeted_19_provenance_rounded_to_zero_top_4_bottom_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-top:0.0000000000000000000000000000000000000000000000000001px;margin-bottom:4px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:6px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, true, NfhOrigin::Specified),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41b00000],
        content: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41f80000, 0x41a00000, false),
    });
}

#[test]
fn fvm_targeted_20_provenance_initial_top_4_bottom_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-bottom:4px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:6px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41b00000],
        content: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41f80000, 0x41a00000, false),
    });
}

#[test]
fn fvm_targeted_21_provenance_inherited_3_4_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#p{margin-top:3px;margin-bottom:4px;} #f{margin-top:inherit;margin-bottom:inherit;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:9px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Inherited),
            (0x40800000, false, NfhOrigin::Inherited),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41980000, 0x41e00000],
        content: [0x42b40000, 0x42c80000, 0x41b00000, 0x41c00000],
        used_vertical: [0x40400000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x42140000, 0x41b80000, false),
    });
}

#[test]
fn fvm_targeted_22_provenance_rounded_to_zero_top_4_bottom_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-top:0.0000000000000000000000000000000000000000000000000001px;margin-bottom:4px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:6px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, true, NfhOrigin::Specified),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41b00000],
        content: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41f80000, 0x41a00000, false),
    });
}

#[test]
fn fvm_targeted_23_provenance_initial_top_4_bottom_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-bottom:4px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:6px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
            (0x40800000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41b00000],
        content: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x40800000],
        used_height: 0x40000000,
        parent: (0x41f80000, 0x41a00000, false),
    });
}

#[test]
fn fvm_targeted_24_extreme_outer_height_overflow_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_height\" }",
    });
}

#[test]
fn fvm_targeted_25_extreme_2_102_absorption_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{height:340282346638528859811704183484516925440px;margin-top:5070602400912917605986812821504px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x72800000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 5,
        error: "UnsupportedEquality",
    });
}

#[test]
fn fvm_targeted_26_extreme_margin_box_overflow_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282346638528859811704183484516925440px;} #f{height:0px;margin-top:10141204801825835211973625643008px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x00000000, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "FloatPlacement(Unrepresentable)",
    });
}

#[test]
fn fvm_targeted_27_extreme_far_edge_overflow_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282326356119256160033759537265639424px;} #f{height:10141204801825835211973625643008px;margin-top:10141206010751654826602800349184px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x73000000, false, NfhOrigin::Specified),
            (0x73000001, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_content_bottom\" }",
    });
}

#[test]
fn fvm_targeted_28_extreme_outer_height_overflow_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_height\" }",
    });
}

#[test]
fn fvm_targeted_29_extreme_2_102_absorption_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{height:340282346638528859811704183484516925440px;margin-top:5070602400912917605986812821504px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x72800000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 5,
        error: "UnsupportedEquality",
    });
}

#[test]
fn fvm_targeted_30_extreme_margin_box_overflow_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282346638528859811704183484516925440px;} #f{height:0px;margin-top:10141204801825835211973625643008px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x00000000, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "FloatPlacement(Unrepresentable)",
    });
}

#[test]
fn fvm_targeted_31_extreme_far_edge_overflow_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282326356119256160033759537265639424px;} #f{height:10141204801825835211973625643008px;margin-top:10141206010751654826602800349184px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x73000000, false, NfhOrigin::Specified),
            (0x73000001, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_content_bottom\" }",
    });
}

#[test]
fn fvm_targeted_32_negative_zero_0px_top_bottom_left() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-top:-0px;margin-bottom:-0px;}",
        control: "#f{margin-top:0;margin-bottom:0;height:2px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        content: [0x00000000, 0x41200000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x00000000],
        used_height: 0x40000000,
        parent: (0x41d80000, 0x41800000, false),
    });
}

#[test]
fn fvm_targeted_33_negative_zero_0px_top_bottom_right() {
    assert_fvm_targeted_success(FvmSuccess {
        nodes: 6,
        rule: "#f{margin-top:-0px;margin-bottom:-0px;} #f{float:right;}",
        control: "#f{margin-top:0;margin-bottom:0;height:2px;}",
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Specified),
        ],
        margin_box: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        content: [0x42b40000, 0x42c80000, 0x41800000, 0x41900000],
        used_vertical: [0x00000000, 0x00000000],
        used_height: 0x40000000,
        parent: (0x41d80000, 0x41800000, false),
    });
}

#[test]
fn fvm_targeted_34_precedence_both_outer_extents_overflow_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{width:340282346638528859811704183484516925440px;margin-left:10141204801825835211973625643008px;height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_width\" }",
    });
}

#[test]
fn fvm_targeted_35_precedence_both_outer_extents_overflow_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{width:340282346638528859811704183484516925440px;margin-left:10141204801825835211973625643008px;height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;} #f{float:right;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_width\" }",
    });
}

#[test]
fn fvm_targeted_36_precedence_both_far_edges_overflow_left() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282326356119256160033759537265639424px;} #f{width:10141204801825835211973625643008px;margin-left:10141206010751654826602800349184px;height:10141204801825835211973625643008px;margin-top:10141206010751654826602800349184px;}",
        outer: (f32::from_bits(0x7f7ffffe), f32::from_bits(0x00000000)),
        facts: [
            (0x73000000, false, NfhOrigin::Specified),
            (0x73000001, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_content_right\" }",
    });
}

#[test]
fn fvm_targeted_37_precedence_both_far_edges_overflow_right() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#b1{margin-bottom:340282326356119256160033759537265639424px;} #f{width:10141204801825835211973625643008px;margin-right:10141206010751654826602800349184px;height:10141204801825835211973625643008px;margin-top:10141206010751654826602800349184px;} #f{float:right;}",
        outer: (f32::from_bits(0xff7ffffe), f32::from_bits(0x00000000)),
        facts: [
            (0x73000000, false, NfhOrigin::Specified),
            (0x73000001, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_content_left\" }",
    });
}

#[test]
fn nonzero_float_vertical_margin_facade_work_and_storage_boundaries_are_exact() {
    use iced_css::evaluation::sibling_layout::PrincipalChildBox;
    use iced_css_core::formatting::{
        clearance_owner::ChildFacts, clearance_sequence::SequenceNormalPlacement,
    };
    use std::mem::size_of;

    for (source_count, facade_work, old_facade, core_work, old_core) in [
        (5, 594, 475, 481, 419),
        (6, 766, 616, 563, 501),
        (7, 938, 757, 646, 584),
    ] {
        let b4 = if source_count == 7 {
            "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }"
        } else {
            ""
        };
        let css = format!(
            "{} {b4} #f {{ margin-top:3px; margin-bottom:4px; }}",
            baseline_css()
        );
        sibling_fixture(source_count, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let build = |limits| {
                default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
            };
            let child_count = source_count - 2;
            let tail_count = source_count - 4;
            let scratch_floor = child_count * size_of::<ChildFacts<'_>>();
            let core_floor = tail_count * size_of::<SequenceNormalPlacement>();
            let retained_floor = core_floor + child_count * size_of::<PrincipalChildBox>();

            // One short and the predecessor bound fail before the scratch scan.
            for short in [facade_work - 1, old_facade] {
                let mut limits = layout_limits();
                limits.work = nz(short);
                limits.scratch_bytes = nz(1);
                assert!(matches!(build(limits), Err(SiblingLayoutError::Work)));
            }
            // Positive control: the exact bound passes work and reaches the scratch
            // floor (NFH-T2Q-008).
            let mut limits = layout_limits();
            limits.work = nz(facade_work);
            limits.scratch_bytes = nz(1);
            assert!(matches!(
                build(limits),
                Err(SiblingLayoutError::ScratchBytes)
            ));
            for short in [core_work - 1, old_core] {
                let mut limits = layout_limits();
                limits.core.work = nz(short);
                assert!(matches!(
                    build(limits),
                    Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
                ));
            }
            let mut exact = layout_limits();
            exact.work = nz(facade_work);
            exact.core.work = nz(core_work);
            exact.scratch_bytes = nz(scratch_floor);
            exact.retained_bytes = nz(retained_floor);
            exact.core.retained_bytes = nz(core_floor);
            let layout = build(exact).unwrap();
            let receipt = layout.receipt();
            assert_eq!(receipt.work, facade_work);
            assert_eq!(receipt.scratch_bytes, scratch_floor);
            assert_eq!(receipt.retained_bytes, retained_floor);
            assert_eq!(receipt.core.work, core_work);
            assert_eq!(receipt.core.retained_bytes, core_floor);
            let placed = layout.sequence().placed_float();
            assert_eq!((placed.top, placed.bottom), (16.0, 25.0));
            let float = &layout.children()[1];
            assert_eq!((float.content().top, float.content().bottom), (19.0, 21.0));
            assert_eq!(float.used_margin_top().to_bits(), 0x4040_0000);
            assert_eq!(float.used_margin_bottom().to_bits(), 0x4080_0000);
        });
    }
}

// Every observation except the children's computed `clear`, which is compared separately.
fn leading_clear_view(layout: &CapturedSiblingLayout<'_>) -> Vec<String> {
    let mut view = vec![
        format!("{:?}", layout.sequence()),
        format!("{:?}", layout.parent()),
        format!("{:?}", layout.receipt()),
    ];
    for child in layout.children() {
        view.push(format!(
            "{:?}",
            (
                child.source_index(),
                child.containing_source_index(),
                child.kind(),
                child.content(),
                child.padding(),
                child.border(),
                child.used_width(),
                child.normal_width(),
                child.used_margin_left(),
                child.used_margin_right(),
                child.used_height(),
                child.used_margin_top(),
            )
        ));
        view.push(format!(
            "{:?}",
            (
                child.used_margin_bottom(),
                child.computed_width(),
                child.computed_height(),
                child.computed_margin_left(),
                child.computed_margin_right(),
                child.margin_top(),
                child.margin_bottom(),
            )
        ));
    }
    view
}

type LeadingClears = Vec<(usize, KeywordBoxFact<ClearSide>)>;

fn leading_clear_layout(nodes: usize, css: &str) -> (Vec<String>, LeadingClears) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap_or_else(|error| panic!("{css}: {error:?}"));
        let clears = layout
            .children()
            .iter()
            .map(|child| (child.source_index(), child.clear()))
            .collect();
        out = Some((leading_clear_view(&layout), clears));
    });
    out.unwrap()
}

fn assert_leading_clear_case(
    nodes: usize,
    control_rule: &str,
    rule: &str,
    expected: &[(usize, KeywordBoxFact<ClearSide>)],
) {
    let (control_view, control_clears) =
        leading_clear_layout(nodes, &format!("{} {control_rule}", baseline_css()));
    let (view, clears) = leading_clear_layout(nodes, &format!("{} {rule}", baseline_css()));
    assert_eq!(view, control_view, "{rule}");
    for ((source, fact), (control_source, control_fact)) in clears.iter().zip(&control_clears) {
        assert_eq!(source, control_source, "{rule}");
        match expected.iter().find(|(s, _)| s == source) {
            Some((_, want)) => assert_eq!(fact, want, "{rule}: source {source}"),
            None => assert_eq!(fact, control_fact, "{rule}: source {source}"),
        }
    }
}

const SPECIFIED: BoxFactOrigin = BoxFactOrigin::Specified;

#[test]
fn css_b1_clear_left_matches_the_clear_none_layout() {
    let rule = "#b1 { clear:left; }";
    assert_leading_clear_case(
        6,
        "",
        rule,
        &[(
            2,
            KeywordBoxFact {
                value: ClearSide::Left,
                origin: SPECIFIED,
            },
        )],
    );
    // Literal red-model geometry, so the equivalence cannot pass on two equally wrong
    // layouts.
    sibling_fixture(
        6,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_or_else(|error| panic!("{rule}: {error:?}"));
            let b1 = layout.children()[0].content();
            assert_eq!(
                (b1.top.to_bits(), b1.bottom.to_bits()),
                (11.0_f32.to_bits(), 12.0_f32.to_bits()),
                "{rule}"
            );
            let placed = layout.sequence().placed_float();
            assert_eq!(
                [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
                [0.0_f32, 10.0, 16.0, 18.0].map(f32::to_bits),
                "{rule}"
            );
            let tail = layout.sequence().tail();
            assert_eq!(
                (
                    tail[0].vertical().top.to_bits(),
                    tail[0].vertical().bottom.to_bits(),
                    tail[0].clearance().map(f32::to_bits),
                    tail[0].zero_clearance_top().map(f32::to_bits)
                ),
                (
                    18.0_f32.to_bits(),
                    19.0_f32.to_bits(),
                    Some(0xbf80_0000),
                    Some(19.0_f32.to_bits())
                ),
                "{rule}"
            );
            assert_eq!(
                (
                    tail[1].vertical().top.to_bits(),
                    tail[1].vertical().bottom.to_bits(),
                    tail[1].clearance()
                ),
                (22.0_f32.to_bits(), 23.0_f32.to_bits(), None),
                "{rule}"
            );
            let height = layout.parent().height();
            assert_eq!(
                (
                    height.content_bottom.to_bits(),
                    height.used_height.to_bits(),
                    height.auto_extent.unwrap().float_extended
                ),
                (27.0_f32.to_bits(), 0x4180_0000, false),
                "{rule}"
            );
        },
    );
}

#[test]
fn css_right_float_clear_right_matches_the_clear_none_layout() {
    assert_leading_clear_case(
        6,
        "#f { float:right; }",
        "#f { float:right; clear:right; }",
        &[(
            3,
            KeywordBoxFact {
                value: ClearSide::Right,
                origin: SPECIFIED,
            },
        )],
    );
}

#[test]
fn css_inherited_leading_clear_matches_the_parent_clear_control() {
    let inherited = KeywordBoxFact {
        value: ClearSide::Right,
        origin: BoxFactOrigin::Inherited { original_parent: 4 },
    };
    assert_leading_clear_case(
        6,
        "#p { clear:right; }",
        "#p { clear:right; } #b1, #f { clear:inherit; }",
        &[(2, inherited), (3, inherited)],
    );
}

#[test]
fn css_b1_clear_no_longer_masks_a_later_negative_margin() {
    let rule = "#b1 { clear:left; } #b3 { margin-top:-1px; }";
    sibling_fixture(
        6,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_err();
            let expected = SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 5,
                    reason: OwnerProfileReason::NegativeMargin,
                },
            ));
            assert_eq!(format!("{error:?}"), format!("{expected:?}"), "{rule}");
        },
    );
}

// Leading clear Task2: the facade matrix, the stage rows and the receipts control.

/// The source-count rule, identical on both sides of every comparison: N5 matches the
/// numeric `base_css(5)` and N7 gives B4 its numeric fixture (LC-P-005).
fn leading_clear_source_rule(nodes: usize) -> &'static str {
    match nodes {
        5 => "#b2 { margin-bottom:0px; }",
        6 => "",
        7 => "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; }",
        _ => panic!("no leading-clear fixture for N{nodes}"),
    }
}

fn clear_keyword(keyword: &str) -> ClearSide {
    match keyword {
        "none" => ClearSide::None,
        "left" => ClearSide::Left,
        "right" => ClearSide::Right,
        "both" => ClearSide::Both,
        _ => panic!("unknown clear keyword {keyword}"),
    }
}

/// The 15 (B1, F) `clear` pairs other than (none, none).
fn leading_clear_pairs() -> Vec<(&'static str, &'static str)> {
    let keywords = ["none", "left", "right", "both"];
    let pairs: Vec<_> = keywords
        .iter()
        .flat_map(|b1| keywords.iter().map(move |f| (*b1, *f)))
        .filter(|pair| *pair != ("none", "none"))
        .collect();
    assert_eq!(pairs.len(), 15);
    pairs
}

/// Every pair against the same control; both declared members, `none` included, are
/// expected `Specified` facts (LC-P-005). Returns the number of comparisons.
fn assert_leading_clear_pairs(nodes: usize, control_rule: &str) -> usize {
    let mut comparisons = 0;
    for (b1, f) in leading_clear_pairs() {
        let rule = format!("{control_rule} #b1 {{ clear:{b1}; }} #f {{ clear:{f}; }}");
        let fact = |keyword| KeywordBoxFact {
            value: clear_keyword(keyword),
            origin: SPECIFIED,
        };
        assert_leading_clear_case(nodes, control_rule, &rule, &[(2, fact(b1)), (3, fact(f))]);
        comparisons += 1;
    }
    comparisons
}

/// N × float side × P height, 60 comparisons per N.
fn assert_leading_clear_matrix(nodes: usize) {
    let mut comparisons = 0;
    for side in ["left", "right"] {
        for height in ["", "#p { height:20px; }"] {
            let control = format!(
                "{} #f {{ float:{side}; }} {height}",
                leading_clear_source_rule(nodes)
            );
            comparisons += assert_leading_clear_pairs(nodes, &control);
        }
    }
    assert_eq!(comparisons, 60);
}

#[test]
fn leading_clear_matrix_n5_matches_the_clear_none_layouts() {
    assert_leading_clear_matrix(5);
}

#[test]
fn leading_clear_matrix_n6_matches_the_clear_none_layouts() {
    assert_leading_clear_matrix(6);
}

#[test]
fn leading_clear_matrix_n7_matches_the_clear_none_layouts() {
    assert_leading_clear_matrix(7);
}

#[test]
fn leading_clear_beside_a_nonclearing_b2_matches_the_clear_none_layouts() {
    let mut comparisons = 0;
    for side in ["left", "right"] {
        let control = format!("#b2 {{ clear:none; }} #f {{ float:{side}; }}");
        comparisons += assert_leading_clear_pairs(6, &control);
    }
    assert_eq!(comparisons, 30);
}

#[test]
fn leading_clear_keeps_fvm_targeted_01_unsupported_equality() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 5,
        rule: "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:3px;} #b1{clear:both;} #f{clear:left;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Specified),
        ],
        source: 4,
        error: "UnsupportedEquality",
    });
}

#[test]
fn leading_clear_keeps_fvm_targeted_34_float_outer_width_precedence() {
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: "#f{width:340282346638528859811704183484516925440px;margin-left:10141204801825835211973625643008px;height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;} #b1{clear:right;} #f{clear:both;}",
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_width\" }",
    });
}

#[test]
fn leading_clear_both_keeps_the_none_control_receipts() {
    for nodes in [5, 6, 7] {
        let receipts = |rule: &str| {
            let css = format!("{} {rule}", baseline_css());
            let mut out = None;
            sibling_fixture(nodes, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let layout = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    0.0,
                    100.0,
                    10.0,
                    layout_limits(),
                )
                .unwrap_or_else(|error| panic!("{rule}: {error:?}"));
                out = Some((
                    layout.receipt(),
                    format!("{:?}", layout.sequence().receipt()),
                ));
            });
            out.unwrap()
        };
        let control = leading_clear_source_rule(nodes);
        assert_eq!(
            receipts(&format!("{control} #b1, #f {{ clear:both; }}")),
            receipts(control),
            "N{nodes}"
        );
    }
}

// Float overflow hidden Task1: the visible-overflow equivalence reds.

type OverflowRows = Vec<ClearanceBoxFact>;

// The whole layout Debug and every fact row. No published structure carries overflow, so the
// layout Debug is compared whole (design, "Visible-overflow equivalence control").
fn float_overflow_layout(nodes: usize, css: &str) -> (String, OverflowRows) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let layout = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        )
        .unwrap_or_else(|error| panic!("{css}: {error:?}"));
        let rows = (0..nodes)
            .map(|source| *facts.row(source).unwrap())
            .collect();
        out = Some((format!("{layout:?}"), rows));
    });
    out.unwrap()
}

fn assert_float_overflow_case(
    nodes: usize,
    control_rule: &str,
    rule: &str,
    expected: KeywordBoxFact<BoxOverflow>,
) {
    let (control_view, control_rows) =
        float_overflow_layout(nodes, &format!("{} {control_rule}", baseline_css()));
    let (view, rows) = float_overflow_layout(nodes, &format!("{} {rule}", baseline_css()));
    assert_eq!(view, control_view, "{rule}");
    assert_eq!(rows.len(), control_rows.len(), "{rule}");
    for (source, (row, control)) in rows.iter().zip(&control_rows).enumerate() {
        if source == 3 {
            assert_eq!(row.overflow, expected, "{rule}");
            assert_eq!(
                ClearanceBoxFact {
                    overflow: control.overflow,
                    ..*row
                },
                *control,
                "{rule}"
            );
        } else {
            assert_eq!(row, control, "{rule}: source {source}");
        }
    }
}

const HIDDEN: KeywordBoxFact<BoxOverflow> = KeywordBoxFact {
    value: BoxOverflow::Hidden,
    origin: BoxFactOrigin::Specified,
};

fn float_overflow_error(nodes: usize, rule: &str) -> SiblingLayoutError {
    let mut out = None;
    sibling_fixture(
        nodes,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            out = Some(
                default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    0.0,
                    100.0,
                    10.0,
                    layout_limits(),
                )
                .unwrap_err(),
            );
        },
    );
    out.unwrap()
}

fn assert_captured(error: SiblingLayoutError, expected: CapturedClearanceError) {
    match error {
        SiblingLayoutError::Captured(actual) => assert_eq!(actual, expected),
        other => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn float_overflow_rule(nodes: usize, side: &str, value: &str) -> (String, String) {
    let control = format!(
        "{} #f {{ float:{side}; }}",
        leading_clear_source_rule(nodes)
    );
    let rule = format!("{control} #f {{ overflow:{value}; }}");
    (control, rule)
}

/// The layout of `rule` at N6, for the literal geometry checks.
fn float_overflow_geometry(rule: &str, check: impl FnOnce(&CapturedSiblingLayout<'_>)) {
    sibling_fixture(
        6,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_or_else(|error| panic!("{rule}: {error:?}"));
            check(&layout);
        },
    );
}

#[test]
fn css_float_overflow_hidden_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(6, "left", "hidden");
    assert_float_overflow_case(6, &control, &rule, HIDDEN);
    // Literal red-model geometry, so the equivalence cannot pass on two equally wrong layouts.
    float_overflow_geometry(&rule, |layout| {
        let b1 = layout.children()[0].content();
        assert_eq!(
            (b1.top.to_bits(), b1.bottom.to_bits()),
            (11.0_f32.to_bits(), 12.0_f32.to_bits()),
            "{rule}"
        );
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            [0.0_f32, 10.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
        let tail = layout.sequence().tail();
        assert_eq!(
            (
                tail[0].vertical().top.to_bits(),
                tail[0].vertical().bottom.to_bits(),
                tail[0].clearance().map(f32::to_bits),
                tail[0].zero_clearance_top().map(f32::to_bits)
            ),
            (
                18.0_f32.to_bits(),
                19.0_f32.to_bits(),
                Some(0xbf80_0000),
                Some(19.0_f32.to_bits())
            ),
            "{rule}"
        );
        assert_eq!(
            (
                tail[1].vertical().top.to_bits(),
                tail[1].vertical().bottom.to_bits(),
                tail[1].clearance()
            ),
            (22.0_f32.to_bits(), 23.0_f32.to_bits(), None),
            "{rule}"
        );
        let height = layout.parent().height();
        assert_eq!(
            (
                height.content_bottom.to_bits(),
                height.used_height.to_bits(),
                height.auto_extent.unwrap().float_extended
            ),
            (27.0_f32.to_bits(), 0x4180_0000, false),
            "{rule}"
        );
    });
}

#[test]
fn css_float_overflow_hidden_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(6, "right", "hidden");
    assert_float_overflow_case(6, &control, &rule, HIDDEN);
    float_overflow_geometry(&rule, |layout| {
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            [90.0_f32, 100.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

#[test]
fn css_float_overflow_hidden_n5_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(5, "left", "hidden");
    assert_float_overflow_case(5, &control, &rule, HIDDEN);
}

#[test]
fn css_float_overflow_hidden_n5_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(5, "right", "hidden");
    assert_float_overflow_case(5, &control, &rule, HIDDEN);
}

#[test]
fn css_float_overflow_hidden_n7_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(7, "left", "hidden");
    assert_float_overflow_case(7, &control, &rule, HIDDEN);
}

#[test]
fn css_float_overflow_hidden_n7_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(7, "right", "hidden");
    assert_float_overflow_case(7, &control, &rule, HIDDEN);
}

#[test]
fn css_float_overflow_inherit_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(6, "left", "inherit");
    let inherited = KeywordBoxFact {
        value: BoxOverflow::Hidden,
        origin: BoxFactOrigin::Inherited { original_parent: 4 },
    };
    assert_float_overflow_case(6, &control, &rule, inherited);
}

#[test]
fn float_overflow_hidden_no_longer_masks_its_height_kind() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:hidden; height:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "height",
            reason: CapturedClearanceUnsupported::HeightKind,
        },
    );
}

#[test]
fn float_overflow_hidden_no_longer_masks_a_later_child_overflow() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:hidden; } #b3 { overflow:hidden; }"),
        CapturedClearanceError::Unsupported {
            projected: 5,
            property: "overflow",
            reason: CapturedClearanceUnsupported::ChildOverflow,
        },
    );
}

#[test]
fn float_overflow_hidden_no_longer_masks_the_float_width_kind() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:hidden; width:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "width",
            reason: CapturedClearanceUnsupported::FloatWidth,
        },
    );
}

#[test]
fn float_overflow_hidden_no_longer_masks_a_later_negative_margin() {
    let error = float_overflow_error(6, "#f { overflow:hidden; } #b3 { margin-top:-1px; }");
    let expected = SiblingLayoutError::Core(ClearanceSequenceError::Profile(
        ClearanceOwnerError::Unsupported {
            source: 5,
            reason: OwnerProfileReason::NegativeMargin,
        },
    ));
    assert_eq!(format!("{error:?}"), format!("{expected:?}"));
}

// Float overflow hidden Task2: the matrix, the category rows, the failure seeds, the
// negatives, the order control and the work thresholds.

/// F's computed overflow fact for `rule` at `nodes`, so a seed row shows its declaration took
/// effect.
fn float_overflow_fact(nodes: usize, rule: &str) -> KeywordBoxFact<BoxOverflow> {
    let mut out = None;
    sibling_fixture(
        nodes,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            out = Some(facts.row(3).unwrap().overflow);
        },
    );
    out.unwrap()
}

/// Float side × P height × form against the visible control built from the source-count
/// rule: 8 comparisons per N. `inherit` takes P's `hidden`, whose original index is N − 2.
fn assert_float_overflow_matrix(nodes: usize) -> usize {
    let inherited = KeywordBoxFact {
        value: BoxOverflow::Hidden,
        origin: BoxFactOrigin::Inherited {
            original_parent: nodes - 2,
        },
    };
    let mut comparisons = 0;
    for side in ["left", "right"] {
        for height in ["", "#p { height:20px; }"] {
            let control = format!(
                "{} #f {{ float:{side}; }} {height}",
                leading_clear_source_rule(nodes)
            );
            for (value, expected) in [("hidden", HIDDEN), ("inherit", inherited)] {
                let rule = format!("{control} #f {{ overflow:{value}; }}");
                assert_float_overflow_case(nodes, &control, &rule, expected);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 8, "N{nodes}");
    comparisons
}

#[test]
fn float_overflow_matrix_matches_the_visible_layouts() {
    let comparisons: usize = [5, 6, 7]
        .into_iter()
        .map(assert_float_overflow_matrix)
        .sum();
    assert_eq!(comparisons, 24);
}

#[test]
fn float_overflow_hidden_category_rows_match_the_visible_layouts() {
    let mut comparisons = 0;
    for category in [
        "#f { margin:auto; }",
        "#f { margin-left:5px; margin-right:7px; }",
        "#f { margin-top:3px; margin-bottom:4px; }",
        "#f { width:140px; }",
        "#b1 { clear:left; } #f { clear:both; }",
        "#b2 { clear:none; }",
    ] {
        for side in ["left", "right"] {
            let control = format!("{category} #f {{ float:{side}; }}");
            let rule = format!("{control} #f {{ overflow:hidden; }}");
            assert_float_overflow_case(6, &control, &rule, HIDDEN);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 12);
}

const FOH_SEED_01: &str = "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:3px;} #f{overflow:hidden;}";
const FOH_SEED_34: &str = "#f{width:340282346638528859811704183484516925440px;margin-left:10141204801825835211973625643008px;height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;} #f{overflow:hidden;}";

#[test]
fn float_overflow_hidden_keeps_fvm_targeted_01_unsupported_equality() {
    assert_eq!(float_overflow_fact(5, FOH_SEED_01), HIDDEN);
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 5,
        rule: FOH_SEED_01,
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x40000000, false, NfhOrigin::Specified),
            (0x3f800000, false, NfhOrigin::Specified),
            (0x40400000, false, NfhOrigin::Specified),
        ],
        source: 4,
        error: "UnsupportedEquality",
    });
}

#[test]
fn float_overflow_hidden_keeps_fvm_targeted_34_float_outer_width_precedence() {
    assert_eq!(float_overflow_fact(6, FOH_SEED_34), HIDDEN);
    assert_fvm_targeted_failure(FvmFailure {
        nodes: 6,
        rule: FOH_SEED_34,
        outer: (f32::from_bits(0x00000000), f32::from_bits(0x42c80000)),
        facts: [
            (0x7f7fffff, false, NfhOrigin::Specified),
            (0x73000000, false, NfhOrigin::Specified),
            (0x00000000, false, NfhOrigin::Initial),
        ],
        source: 3,
        error: "Unrepresentable { stage: \"float_outer_width\" }",
    });
}

/// The `FloatPlacement` seed of
/// `maximum_float_width_keeps_finite_cancellation_and_typed_publication_failures` (left float,
/// `content_left = f32::MAX`, `content_width = 0`), hidden against visible.
#[test]
fn float_overflow_hidden_keeps_the_float_placement_failure() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let control = format!(
        "#o {{ width:0; direction:rtl; }} #p {{ direction:ltr; }} \
         #f {{ width:{MAX}; float:left; margin-left:auto; margin-right:auto; direction:rtl; }}"
    );
    let rule = format!("{control} #f {{ overflow:hidden; }}");
    let observe = |rule: &str| {
        let mut out = None;
        sibling_fixture(
            6,
            &format!("{} {rule}", baseline_css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let error = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    f32::MAX,
                    0.0,
                    10.0,
                    layout_limits(),
                )
                .unwrap_err();
                out = Some((facts.row(3).unwrap().overflow, format!("{error:?}")));
            },
        );
        out.unwrap()
    };
    let expected = SiblingLayoutError::Core(ClearanceSequenceError::Placement {
        source: 3,
        error: ClearanceOwnerError::FloatPlacement(
            iced_css_core::formatting::floats::FloatError::Unrepresentable,
        ),
    });
    let (control_fact, control_error) = observe(&control);
    let (fact, error) = observe(&rule);
    assert_eq!(control_fact.value, BoxOverflow::Visible);
    assert_eq!(fact, HIDDEN);
    assert_eq!(control_error, format!("{expected:?}"));
    assert_eq!(error, control_error);
}

fn assert_float_overflow_stays_child_overflow(value: &str) {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for side in ["left", "right"] {
            let (_, rule) = float_overflow_rule(nodes, side, value);
            assert_captured(
                float_overflow_error(nodes, &rule),
                CapturedClearanceError::Unsupported {
                    projected: 3,
                    property: "overflow",
                    reason: CapturedClearanceUnsupported::ChildOverflow,
                },
            );
            rows += 1;
        }
    }
    assert_eq!(rows, 6);
}

#[test]
fn css_float_overflow_scroll_stays_child_overflow() {
    assert_float_overflow_stays_child_overflow("scroll");
}

#[test]
fn css_float_overflow_scroll_before_height_kind() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:scroll; height:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "overflow",
            reason: CapturedClearanceUnsupported::ChildOverflow,
        },
    );
}

/// The default outer geometry with only the facade work limit changed.
fn float_overflow_build<'p>(
    facts: &iced_css::evaluation::sibling_box_facts::SiblingBoxFacts<'p, '_, '_, '_, '_, '_>,
    outer: &'p iced_css_core::source::SourceNode,
    work: usize,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    let mut limits = layout_limits();
    limits.work = nz(work);
    default_outer_layout(facts, outer, 0.0, 100.0, 10.0, limits)
}

#[test]
fn float_overflow_facade_work_thresholds_are_exact() {
    for (nodes, exact, lc, core) in [(5, 594, 481, 481), (6, 766, 622, 563), (7, 938, 763, 646)] {
        let css = format!(
            "{} {} #f {{ overflow:hidden; }}",
            baseline_css(),
            leading_clear_source_rule(nodes)
        );
        sibling_fixture(nodes, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert_eq!(facts.row(3).unwrap().overflow, HIDDEN, "N{nodes}");
            for short in [exact - 1, lc] {
                assert!(
                    matches!(
                        float_overflow_build(&facts, &projection.nodes()[0], short),
                        Err(SiblingLayoutError::Work)
                    ),
                    "N{nodes} work {short}"
                );
            }
            let layout = float_overflow_build(&facts, &projection.nodes()[0], exact).unwrap();
            assert_eq!(layout.receipt().work, exact, "N{nodes}");
            assert_eq!(exact, 80 + 32 * nodes + 74 * (nodes - 2) + 66 * (nodes - 3));
            assert_eq!(layout.receipt().core.work, core, "N{nodes}");
            assert_eq!(layout.sequence().receipt().work, core, "N{nodes}");
        });
    }
    // The work is prepaid before the profile scan reaches the later child's overflow.
    let css = format!(
        "{} #f {{ overflow:hidden; }} #b3 {{ overflow:hidden; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            float_overflow_build(&facts, &projection.nodes()[0], 765),
            Err(SiblingLayoutError::Work)
        ));
        assert_captured(
            float_overflow_build(&facts, &projection.nodes()[0], 766).unwrap_err(),
            CapturedClearanceError::Unsupported {
                projected: 5,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
    });
}

// Float overflow auto: F admits computed `auto` with the `visible` geometry (FOA plan).

const AUTO: KeywordBoxFact<BoxOverflow> = KeywordBoxFact {
    value: BoxOverflow::Auto,
    origin: BoxFactOrigin::Specified,
};

/// The literal N6 left layout of the red model (as FOH R1), so an equivalence cannot pass on two
/// equally wrong layouts.
fn assert_float_overflow_n6_left_geometry(rule: &str) {
    float_overflow_geometry(rule, |layout| {
        let b1 = layout.children()[0].content();
        assert_eq!(
            (b1.top.to_bits(), b1.bottom.to_bits()),
            (11.0_f32.to_bits(), 12.0_f32.to_bits()),
            "{rule}"
        );
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            [0.0_f32, 10.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
        let tail = layout.sequence().tail();
        assert_eq!(
            (
                tail[0].vertical().top.to_bits(),
                tail[0].vertical().bottom.to_bits(),
                tail[0].clearance().map(f32::to_bits),
                tail[0].zero_clearance_top().map(f32::to_bits)
            ),
            (
                18.0_f32.to_bits(),
                19.0_f32.to_bits(),
                Some(0xbf80_0000),
                Some(19.0_f32.to_bits())
            ),
            "{rule}"
        );
        assert_eq!(
            (
                tail[1].vertical().top.to_bits(),
                tail[1].vertical().bottom.to_bits(),
                tail[1].clearance()
            ),
            (22.0_f32.to_bits(), 23.0_f32.to_bits(), None),
            "{rule}"
        );
        let height = layout.parent().height();
        assert_eq!(
            (
                height.content_bottom.to_bits(),
                height.used_height.to_bits(),
                height.auto_extent.unwrap().float_extended
            ),
            (27.0_f32.to_bits(), 0x4180_0000, false),
            "{rule}"
        );
    });
}

#[test]
fn css_float_overflow_auto_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(6, "left", "auto");
    assert_float_overflow_case(6, &control, &rule, AUTO);
    assert_float_overflow_n6_left_geometry(&rule);
}

#[test]
fn css_float_overflow_auto_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(6, "right", "auto");
    assert_float_overflow_case(6, &control, &rule, AUTO);
    float_overflow_geometry(&rule, |layout| {
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            [90.0_f32, 100.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

#[test]
fn css_float_overflow_auto_n5_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(5, "left", "auto");
    assert_float_overflow_case(5, &control, &rule, AUTO);
}

#[test]
fn css_float_overflow_auto_n5_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(5, "right", "auto");
    assert_float_overflow_case(5, &control, &rule, AUTO);
}

#[test]
fn css_float_overflow_auto_n7_left_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(7, "left", "auto");
    assert_float_overflow_case(7, &control, &rule, AUTO);
}

#[test]
fn css_float_overflow_auto_n7_right_matches_the_visible_layout() {
    let (control, rule) = float_overflow_rule(7, "right", "auto");
    assert_float_overflow_case(7, &control, &rule, AUTO);
}

#[test]
fn float_overflow_auto_no_longer_masks_its_height_kind() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:auto; height:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "height",
            reason: CapturedClearanceUnsupported::HeightKind,
        },
    );
}

#[test]
fn float_overflow_auto_no_longer_masks_a_later_child_overflow() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:auto; } #b3 { overflow:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 5,
            property: "overflow",
            reason: CapturedClearanceUnsupported::ChildOverflow,
        },
    );
}

#[test]
fn float_overflow_auto_no_longer_masks_the_float_width_kind() {
    assert_captured(
        float_overflow_error(6, "#f { overflow:auto; width:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "width",
            reason: CapturedClearanceUnsupported::FloatWidth,
        },
    );
}

#[test]
fn float_overflow_auto_no_longer_masks_a_later_negative_margin() {
    let error = float_overflow_error(6, "#f { overflow:auto; } #b3 { margin-top:-1px; }");
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 5,
                    reason: OwnerProfileReason::NegativeMargin,
                },
            ))
        ),
        "{error:?}"
    );
}

#[test]
fn css_float_overflow_auto_overrides_an_earlier_hidden() {
    let (_, control) = float_overflow_rule(6, "left", "hidden");
    let rule = format!("{control} #f {{ overflow:auto; }}");
    assert_float_overflow_case(6, &control, &rule, AUTO);
}

/// Float side × P height against the visible and the hidden control built from the
/// source-count rule, plus the cascade override: 12 comparisons per N.
fn assert_float_overflow_auto_matrix(nodes: usize) -> usize {
    let mut comparisons = 0;
    for side in ["left", "right"] {
        for height in ["", "#p { height:20px; }"] {
            let control = format!(
                "{} #f {{ float:{side}; }} {height}",
                leading_clear_source_rule(nodes)
            );
            let hidden = format!("{control} #f {{ overflow:hidden; }}");
            let auto = format!("{control} #f {{ overflow:auto; }}");
            let overridden = format!("{hidden} #f {{ overflow:auto; }}");
            assert_float_overflow_case(nodes, &control, &auto, AUTO);
            assert_float_overflow_case(nodes, &hidden, &auto, AUTO);
            assert_float_overflow_case(nodes, &control, &overridden, AUTO);
            comparisons += 3;
        }
    }
    assert_eq!(comparisons, 12, "N{nodes}");
    comparisons
}

#[test]
fn float_overflow_auto_matrix_matches_the_visible_and_hidden_layouts() {
    let comparisons: usize = [5, 6, 7]
        .into_iter()
        .map(assert_float_overflow_auto_matrix)
        .sum();
    assert_eq!(comparisons, 36);
}

#[test]
fn float_overflow_auto_category_rows_match_the_visible_and_hidden_layouts() {
    let mut comparisons = 0;
    for category in [
        "#f { margin:auto; }",
        "#f { margin-left:5px; margin-right:7px; }",
        "#f { margin-top:3px; margin-bottom:4px; }",
        "#f { width:140px; }",
        "#b1 { clear:left; } #f { clear:both; }",
        "#b2 { clear:none; }",
    ] {
        for side in ["left", "right"] {
            let control = format!("{category} #f {{ float:{side}; }}");
            let hidden = format!("{control} #f {{ overflow:hidden; }}");
            let auto = format!("{control} #f {{ overflow:auto; }}");
            assert_float_overflow_case(6, &control, &auto, AUTO);
            assert_float_overflow_case(6, &hidden, &auto, AUTO);
            comparisons += 2;
        }
    }
    assert_eq!(comparisons, 24);
}

const FOA_SEED_01: &str =
    "#b2{margin-top:10px;margin-bottom:0px;} #f{margin-top:1px;margin-bottom:3px;}";
const FOA_SEED_34: &str = "#f{width:340282346638528859811704183484516925440px;margin-left:10141204801825835211973625643008px;height:340282346638528859811704183484516925440px;margin-top:10141204801825835211973625643008px;}";

const VISIBLE: KeywordBoxFact<BoxOverflow> = KeywordBoxFact {
    value: BoxOverflow::Visible,
    origin: BoxFactOrigin::Initial,
};

/// A targeted FVM rule with F's `overflow` fact and a typed placement failure (no Debug text).
fn assert_float_overflow_auto_seed(
    nodes: usize,
    rule: &str,
    facts_expected: [NfhFact; 3],
    overflow: KeywordBoxFact<BoxOverflow>,
    expected: (usize, ClearanceOwnerError),
) {
    let css = format!("{} {rule}", baseline_css());
    sibling_fixture(nodes, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let original_parent = projection.original_index(1).unwrap();
        assert_fvm_facts(&facts, original_parent, rule, facts_expected);
        assert_eq!(facts.row(3).unwrap().overflow, overflow, "{rule}");
        match default_outer_layout(
            &facts,
            &projection.nodes()[0],
            0.0,
            100.0,
            10.0,
            layout_limits(),
        ) {
            Err(SiblingLayoutError::Core(ClearanceSequenceError::Placement { source, error })) => {
                assert_eq!((source, error), expected, "{rule}");
            }
            other => panic!("{rule}: expected a placement failure, got {other:?}"),
        }
    });
}

#[test]
fn float_overflow_auto_keeps_fvm_targeted_01_unsupported_equality() {
    let rule = format!("{FOA_SEED_01} #f{{overflow:auto;}}");
    let mut rows = 0;
    for (css, overflow) in [(FOA_SEED_01, VISIBLE), (rule.as_str(), AUTO)] {
        assert_float_overflow_auto_seed(
            5,
            css,
            [
                (0x40000000, false, NfhOrigin::Specified),
                (0x3f800000, false, NfhOrigin::Specified),
                (0x40400000, false, NfhOrigin::Specified),
            ],
            overflow,
            (4, ClearanceOwnerError::UnsupportedEquality),
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn float_overflow_auto_keeps_fvm_targeted_34_float_outer_width_precedence() {
    let rule = format!("{FOA_SEED_34} #f{{overflow:auto;}}");
    let mut rows = 0;
    for (css, overflow) in [(FOA_SEED_34, VISIBLE), (rule.as_str(), AUTO)] {
        assert_float_overflow_auto_seed(
            6,
            css,
            [
                (0x7f7fffff, false, NfhOrigin::Specified),
                (0x73000000, false, NfhOrigin::Specified),
                (0x00000000, false, NfhOrigin::Initial),
            ],
            overflow,
            (
                3,
                ClearanceOwnerError::Unrepresentable {
                    stage: "float_outer_width",
                },
            ),
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

/// The `FloatPlacement` seed of
/// `maximum_float_width_keeps_finite_cancellation_and_typed_publication_failures` (left float,
/// `content_left = f32::MAX`, `content_width = 0`), auto against visible, kept typed.
#[test]
fn float_overflow_auto_keeps_the_float_placement_failure() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let control = format!(
        "#o {{ width:0; direction:rtl; }} #p {{ direction:ltr; }} \
         #f {{ width:{MAX}; float:left; margin-left:auto; margin-right:auto; direction:rtl; }}"
    );
    let rule = format!("{control} #f {{ overflow:auto; }}");
    let observe = |rule: &str| {
        let mut out = None;
        sibling_fixture(
            6,
            &format!("{} {rule}", baseline_css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let error = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    f32::MAX,
                    0.0,
                    10.0,
                    layout_limits(),
                )
                .unwrap_err();
                out = Some((facts.row(3).unwrap().overflow, error));
            },
        );
        out.unwrap()
    };
    let mut rows = 0;
    for (css, overflow) in [(control.as_str(), VISIBLE), (rule.as_str(), AUTO)] {
        let (fact, error) = observe(css);
        assert_eq!(fact, overflow, "{css}");
        assert!(
            matches!(
                error,
                SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                    source: 3,
                    error: ClearanceOwnerError::FloatPlacement(
                        iced_css_core::formatting::floats::FloatError::Unrepresentable
                    ),
                })
            ),
            "{css}: {error:?}"
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_non_float_auto_or_inherited_overflow_stays_child_overflow() {
    let mut rows = 0;
    for (rule, projected) in [
        ("#b3 { overflow:auto; }", 5),
        ("#b3 { overflow:inherit; }", 5),
    ] {
        assert_captured(
            float_overflow_error(6, rule),
            CapturedClearanceError::Unsupported {
                projected,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_float_overflow_auto_generated_content_stays_a_fact_failure() {
    let css = format!(
        "{} #f:before {{ content:'x'; }} #f {{ overflow:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Unsupported {
                    original: 1,
                    target: BoxFactTarget::Before,
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                }
            ))
        ));
    });
}

#[test]
fn float_overflow_auto_facade_work_thresholds_are_unchanged() {
    for (nodes, exact, core) in [(5, 594, 481), (6, 766, 563), (7, 938, 646)] {
        let css = format!(
            "{} {} #f {{ overflow:auto; }}",
            baseline_css(),
            leading_clear_source_rule(nodes)
        );
        sibling_fixture(nodes, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert_eq!(facts.row(3).unwrap().overflow, AUTO, "N{nodes}");
            assert!(
                matches!(
                    float_overflow_build(&facts, &projection.nodes()[0], exact - 1),
                    Err(SiblingLayoutError::Work)
                ),
                "N{nodes} work {}",
                exact - 1
            );
            let layout = float_overflow_build(&facts, &projection.nodes()[0], exact).unwrap();
            assert_eq!(layout.receipt().work, exact, "N{nodes}");
            assert_eq!(exact, 80 + 32 * nodes + 74 * (nodes - 2) + 66 * (nodes - 3));
            assert_eq!(layout.receipt().core.work, core, "N{nodes}");
            assert_eq!(layout.sequence().receipt().work, core, "N{nodes}");
        });
    }
    // The work is prepaid before the profile scan reaches the later child's overflow.
    let css = format!(
        "{} #f {{ overflow:auto; }} #b3 {{ overflow:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            float_overflow_build(&facts, &projection.nodes()[0], 765),
            Err(SiblingLayoutError::Work)
        ));
        assert_captured(
            float_overflow_build(&facts, &projection.nodes()[0], 766).unwrap_err(),
            CapturedClearanceError::Unsupported {
                projected: 5,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
    });
}

// Leading overflow: B1 admits computed `hidden`/`auto` strictly above F (LO plan).

/// B1's `inherit` takes P's computed `hidden`, whose original index is N - 2.
fn b1_inherited(nodes: usize) -> KeywordBoxFact<BoxOverflow> {
    KeywordBoxFact {
        value: BoxOverflow::Hidden,
        origin: BoxFactOrigin::Inherited {
            original_parent: nodes - 2,
        },
    }
}

/// `assert_float_overflow_case` with the carved-out source moved from F (3) to B1 (2): the whole
/// layout Debug and every other fact row must equal the control's.
fn assert_b1_overflow_case(
    nodes: usize,
    control_rule: &str,
    rule: &str,
    expected: KeywordBoxFact<BoxOverflow>,
) {
    let (control_view, control_rows) =
        float_overflow_layout(nodes, &format!("{} {control_rule}", baseline_css()));
    let (view, rows) = float_overflow_layout(nodes, &format!("{} {rule}", baseline_css()));
    assert_eq!(view, control_view, "{rule}");
    assert_eq!(rows.len(), control_rows.len(), "{rule}");
    for (source, (row, control)) in rows.iter().zip(&control_rows).enumerate() {
        if source == 2 {
            assert_eq!(row.overflow, expected, "{rule}");
            assert_eq!(
                ClearanceBoxFact {
                    overflow: control.overflow,
                    ..*row
                },
                *control,
                "{rule}"
            );
        } else {
            assert_eq!(row, control, "{rule}: source {source}");
        }
    }
}

fn b1_overflow_rule(nodes: usize, side: &str, value: &str) -> (String, String) {
    let control = format!(
        "{} #f {{ float:{side}; }}",
        leading_clear_source_rule(nodes)
    );
    let rule = format!("{control} #b1 {{ overflow:{value}; }}");
    (control, rule)
}

/// The typed contact error at B1 (source 2).
fn assert_b1_contact(error: SiblingLayoutError) {
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 2,
                error: ClearanceOwnerError::NewBfcFloatContact,
            })
        ),
        "{error:?}"
    );
}

/// The `visible` control of a contact rule lays out, with F's margin top equal to B1's border
/// bottom: the published pair the core compares.
fn assert_b1_contact_control(nodes: usize, control: &str) {
    sibling_fixture(
        nodes,
        &format!("{} {control}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_or_else(|error| panic!("{control}: {error:?}"));
            assert_eq!(
                layout.sequence().placed_float().top.to_bits(),
                layout.sequence().before().bottom.to_bits(),
                "{control}"
            );
        },
    );
}

#[test]
fn css_b1_overflow_hidden_left_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(6, "left", "hidden");
    assert_b1_overflow_case(6, &control, &rule, HIDDEN);
    assert_float_overflow_n6_left_geometry(&rule);
}

#[test]
fn css_b1_overflow_hidden_right_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(6, "right", "hidden");
    assert_b1_overflow_case(6, &control, &rule, HIDDEN);
    float_overflow_geometry(&rule, |layout| {
        let placed = layout.sequence().placed_float();
        assert_eq!(
            [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
            [90.0_f32, 100.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

#[test]
fn css_b1_overflow_hidden_n5_left_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(5, "left", "hidden");
    assert_b1_overflow_case(5, &control, &rule, HIDDEN);
}

#[test]
fn css_b1_overflow_hidden_n5_right_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(5, "right", "hidden");
    assert_b1_overflow_case(5, &control, &rule, HIDDEN);
}

#[test]
fn css_b1_overflow_hidden_n7_left_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(7, "left", "hidden");
    assert_b1_overflow_case(7, &control, &rule, HIDDEN);
}

#[test]
fn css_b1_overflow_hidden_n7_right_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(7, "right", "hidden");
    assert_b1_overflow_case(7, &control, &rule, HIDDEN);
}

#[test]
fn css_b1_overflow_auto_left_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(6, "left", "auto");
    assert_b1_overflow_case(6, &control, &rule, AUTO);
}

#[test]
fn css_b1_overflow_inherit_matches_the_visible_layout() {
    let (control, rule) = b1_overflow_rule(6, "left", "inherit");
    assert_b1_overflow_case(6, &control, &rule, b1_inherited(6));
}

#[test]
fn css_b1_overflow_hidden_edge_contact_is_typed() {
    let control = "#b1 { margin-bottom:0px; }";
    assert_b1_contact_control(6, control);
    float_overflow_geometry(control, |layout| {
        assert_eq!(
            layout.sequence().placed_float().top.to_bits(),
            12.0_f32.to_bits(),
            "{control}"
        );
    });
    assert_b1_contact(float_overflow_error(
        6,
        "#b1 { margin-bottom:0px; } #b1 { overflow:hidden; }",
    ));
}

#[test]
fn css_b1_overflow_hidden_negative_zero_bottom_margin_is_contact() {
    let control = "#b1 { margin-bottom:-0px; }";
    assert_b1_contact_control(6, control);
    assert_b1_contact(float_overflow_error(
        6,
        "#b1 { margin-bottom:-0px; } #b1 { overflow:hidden; }",
    ));
}

#[test]
fn css_b1_overflow_auto_bottom_auto_margin_is_contact() {
    let control = "#b1 { margin-bottom:auto; }";
    assert_b1_contact_control(6, control);
    assert_b1_contact(float_overflow_error(
        6,
        "#b1 { margin-bottom:auto; } #b1 { overflow:auto; }",
    ));
}

#[test]
fn css_b1_overflow_rounding_absorbed_margin_is_contact() {
    // B1 runs from content top 11 to exactly 2^24, and RN(2^24 + 1) = 2^24 (ties to even).
    let control = "#b1 { height:16777205px; margin-bottom:1px; }";
    assert_b1_contact_control(6, control);
    float_overflow_geometry(control, |layout| {
        assert_eq!(
            layout.sequence().before().bottom.to_bits(),
            16_777_216.0_f32.to_bits(),
            "{control}"
        );
    });
    assert_b1_contact(float_overflow_error(
        6,
        "#b1 { height:16777205px; margin-bottom:1px; } #b1 { overflow:hidden; }",
    ));
}

#[test]
fn css_b1_overflow_hidden_kept_margin_at_2p24_matches_the_visible_layout() {
    let control = "#b1 { height:16777205px; margin-bottom:4px; }";
    let rule = format!("{control} #b1 {{ overflow:hidden; }}");
    assert_b1_overflow_case(6, control, &rule, HIDDEN);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            layout.sequence().placed_float().top.to_bits(),
            16_777_220.0_f32.to_bits(),
            "{rule}"
        );
    });
}

#[test]
fn b1_overflow_no_longer_masks_its_height_kind() {
    assert_captured(
        float_overflow_error(6, "#b1 { overflow:hidden; height:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 2,
            property: "height",
            reason: CapturedClearanceUnsupported::HeightKind,
        },
    );
}

#[test]
fn b1_overflow_no_longer_masks_a_later_child_overflow() {
    assert_captured(
        float_overflow_error(6, "#b1 { overflow:hidden; } #b3 { overflow:hidden; }"),
        CapturedClearanceError::Unsupported {
            projected: 5,
            property: "overflow",
            reason: CapturedClearanceUnsupported::ChildOverflow,
        },
    );
}

#[test]
fn b1_overflow_no_longer_masks_a_negative_margin() {
    let error = float_overflow_error(6, "#b1 { overflow:hidden; margin-top:-1px; }");
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 2,
                    reason: OwnerProfileReason::NegativeMargin,
                },
            ))
        ),
        "{error:?}"
    );
}

#[test]
fn b1_overflow_contact_precedes_a_tail_equality() {
    let control = "#b1 { margin-bottom:0px; } #b2 { margin-top:2px; }";
    let error = float_overflow_error(6, control);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ),
        "{error:?}"
    );
    assert_b1_contact(float_overflow_error(
        6,
        &format!("{control} #b1 {{ overflow:hidden; }}"),
    ));
}

#[test]
fn b1_overflow_contact_precedes_child_horizontal() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let base = format!("#f {{ width:0; }} #b1 {{ width:{MAX}; }}");
    let observe = |rule: &str| {
        let mut out = None;
        sibling_fixture(
            6,
            &format!("{} {rule}", baseline_css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                out = Some(
                    default_outer_layout(
                        &facts,
                        &projection.nodes()[0],
                        f32::MAX,
                        100.0,
                        10.0,
                        layout_limits(),
                    )
                    .unwrap_err(),
                );
            },
        );
        out.unwrap()
    };
    let mut rows = 0;
    for rule in [base.clone(), format!("{base} #b1 {{ overflow:hidden; }}")] {
        let error = observe(&rule);
        assert!(
            matches!(
                error,
                SiblingLayoutError::ChildHorizontal {
                    source: 2,
                    error: BlockHorizontalError::Unrepresentable {
                        stage: BlockCoordinateStage::ContentRight,
                    },
                }
            ),
            "{rule}: {error:?}"
        );
        rows += 1;
    }
    assert_b1_contact(observe(&format!(
        "{base} #b1 {{ overflow:hidden; margin-bottom:0px; }}"
    )));
    rows += 1;
    assert_eq!(rows, 3);
}

// Leading overflow Task2: the matrices, the category rows, the failure seeds, the negatives and the
// work thresholds.

/// Float side x P height x F overflow x B1 form against the same CSS without the B1 declaration,
/// built from the source-count rule: 36 comparisons per N, all strictly separated (B1's bottom
/// margin is 4px).
fn assert_b1_overflow_matrix(nodes: usize) -> usize {
    let mut comparisons = 0;
    for side in ["left", "right"] {
        for height in ["", "#p { height:20px; }"] {
            for float in ["", "#f { overflow:hidden; }", "#f { overflow:auto; }"] {
                let control = format!(
                    "{} #f {{ float:{side}; }} {height} {float}",
                    leading_clear_source_rule(nodes)
                );
                for (value, expected) in [
                    ("hidden", HIDDEN),
                    ("auto", AUTO),
                    ("inherit", b1_inherited(nodes)),
                ] {
                    let rule = format!("{control} #b1 {{ overflow:{value}; }}");
                    assert_b1_overflow_case(nodes, &control, &rule, expected);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 36, "N{nodes}");
    comparisons
}

#[test]
fn b1_overflow_matrix_matches_the_visible_layouts() {
    let comparisons: usize = [5, 6, 7].into_iter().map(assert_b1_overflow_matrix).sum();
    assert_eq!(comparisons, 108);
}

/// Float side x used-zero bottom margin x B1 form: each `visible` control lays out with F's margin
/// top equal to B1's border bottom, and each B1 form gives the typed contact error: 12 rows per N.
fn assert_b1_contact_matrix(nodes: usize) -> usize {
    let mut rows = 0;
    for side in ["left", "right"] {
        for bottom in ["0px", "auto"] {
            let control = format!(
                "{} #f {{ float:{side}; }} #b1 {{ margin-bottom:{bottom}; }}",
                leading_clear_source_rule(nodes)
            );
            assert_b1_contact_control(nodes, &control);
            for value in ["hidden", "auto", "inherit"] {
                let rule = format!("{control} #b1 {{ overflow:{value}; }}");
                assert_b1_contact(float_overflow_error(nodes, &rule));
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 12, "N{nodes}");
    rows
}

#[test]
fn b1_overflow_contact_matrix_is_typed() {
    let rows: usize = [5, 6, 7].into_iter().map(assert_b1_contact_matrix).sum();
    assert_eq!(rows, 36);
}

#[test]
fn b1_overflow_hidden_category_rows_match_the_visible_layouts() {
    let mut comparisons = 0;
    for category in [
        "#f { margin:auto; }",
        "#f { margin-left:5px; margin-right:7px; }",
        "#f { margin-top:3px; margin-bottom:4px; }",
        "#f { width:140px; }",
        "#b1 { clear:left; } #f { clear:both; }",
        "#b2 { clear:none; }",
    ] {
        for side in ["left", "right"] {
            let control = format!("{category} #f {{ float:{side}; }}");
            let rule = format!("{control} #b1 {{ overflow:hidden; }}");
            assert_b1_overflow_case(6, &control, &rule, HIDDEN);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 12);
}

/// B1's computed overflow fact for `rule` at `nodes`, so a seed row shows its declaration took
/// effect.
fn b1_overflow_fact(nodes: usize, rule: &str) -> KeywordBoxFact<BoxOverflow> {
    let mut out = None;
    sibling_fixture(
        nodes,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            out = Some(facts.row(2).unwrap().overflow);
        },
    );
    out.unwrap()
}

#[test]
fn b1_overflow_keeps_fvm_targeted_01_unsupported_equality() {
    let rule = format!("{FOA_SEED_01} #b1{{overflow:hidden;}}");
    let mut rows = 0;
    for (css, b1) in [(FOA_SEED_01, VISIBLE), (rule.as_str(), HIDDEN)] {
        assert_eq!(b1_overflow_fact(5, css), b1, "{css}");
        assert_float_overflow_auto_seed(
            5,
            css,
            [
                (0x40000000, false, NfhOrigin::Specified),
                (0x3f800000, false, NfhOrigin::Specified),
                (0x40400000, false, NfhOrigin::Specified),
            ],
            VISIBLE,
            (4, ClearanceOwnerError::UnsupportedEquality),
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

/// The `FloatPlacement` seed of
/// `maximum_float_width_keeps_finite_cancellation_and_typed_publication_failures` (left float,
/// `content_left = f32::MAX`, `content_width = 0`), B1 `hidden` against `visible`: the first-float
/// kernel fails before the contact check.
#[test]
fn b1_overflow_keeps_the_float_placement_failure() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let control = format!(
        "#o {{ width:0; direction:rtl; }} #p {{ direction:ltr; }} \
         #f {{ width:{MAX}; float:left; margin-left:auto; margin-right:auto; direction:rtl; }}"
    );
    let rule = format!("{control} #b1 {{ overflow:hidden; }}");
    let mut rows = 0;
    for (css, b1) in [(control.as_str(), VISIBLE), (rule.as_str(), HIDDEN)] {
        let mut out = None;
        sibling_fixture(
            6,
            &format!("{} {css}", baseline_css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let error = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    f32::MAX,
                    0.0,
                    10.0,
                    layout_limits(),
                )
                .unwrap_err();
                out = Some((facts.row(2).unwrap().overflow, error));
            },
        );
        let (fact, error) = out.unwrap();
        assert_eq!(fact, b1, "{css}");
        assert!(
            matches!(
                error,
                SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                    source: 3,
                    error: ClearanceOwnerError::FloatPlacement(
                        iced_css_core::formatting::floats::FloatError::Unrepresentable
                    ),
                })
            ),
            "{css}: {error:?}"
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

/// FOA-T2Q-001 closed: `auto` or `hidden` on an interior tail (B2 at N6 and N7) and on the last
/// source (B2 at N5) stays `ChildOverflow`, and so does B1 `scroll`, before its height kind.
#[test]
fn b1_scroll_and_b2_overflow_stay_child_overflow() {
    let mut rows = 0;
    for (nodes, rule, projected) in [
        (6, "#b2 { overflow:auto; }", 4),
        (7, "#b2 { overflow:auto; }", 4),
        (5, "#b2 { overflow:hidden; }", 4),
        (6, "#b1 { overflow:scroll; }", 2),
        (6, "#b1 { overflow:scroll; height:auto; }", 2),
    ] {
        assert_captured(
            float_overflow_error(nodes, rule),
            CapturedClearanceError::Unsupported {
                projected,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
        rows += 1;
    }
    assert_eq!(rows, 5);
}

#[test]
fn css_b1_overflow_auto_generated_content_stays_a_fact_failure() {
    let css = format!(
        "{} #b1:before {{ content:'x'; }} #b1 {{ overflow:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Unsupported {
                    original: 0,
                    target: BoxFactTarget::Before,
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                }
            ))
        ));
    });
}

#[test]
fn b1_overflow_no_longer_masks_the_float_width_kind() {
    assert_captured(
        float_overflow_error(6, "#b1 { overflow:hidden; } #f { width:auto; }"),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "width",
            reason: CapturedClearanceUnsupported::FloatWidth,
        },
    );
}

#[test]
fn b1_overflow_facade_work_thresholds_are_exact() {
    for (nodes, exact, core) in [(5, 594, 481), (6, 766, 563), (7, 938, 646)] {
        let css = format!(
            "{} {} #b1 {{ overflow:hidden; }}",
            baseline_css(),
            leading_clear_source_rule(nodes)
        );
        sibling_fixture(nodes, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert_eq!(facts.row(2).unwrap().overflow, HIDDEN, "N{nodes}");
            assert!(
                matches!(
                    float_overflow_build(&facts, &projection.nodes()[0], exact - 1),
                    Err(SiblingLayoutError::Work)
                ),
                "N{nodes} work {}",
                exact - 1
            );
            let layout = float_overflow_build(&facts, &projection.nodes()[0], exact).unwrap();
            assert_eq!(layout.receipt().work, exact, "N{nodes}");
            assert_eq!(exact, 80 + 32 * nodes + 74 * (nodes - 2) + 66 * (nodes - 3));
            assert_eq!(layout.receipt().core.work, core, "N{nodes}");
            assert_eq!(layout.sequence().receipt().work, core, "N{nodes}");
        });
    }
    // The work is prepaid before the profile scan reaches the later child's overflow.
    let css = format!(
        "{} #b1 {{ overflow:hidden; }} #b3 {{ overflow:hidden; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            float_overflow_build(&facts, &projection.nodes()[0], 765),
            Err(SiblingLayoutError::Work)
        ));
        assert_captured(
            float_overflow_build(&facts, &projection.nodes()[0], 766).unwrap_err(),
            CapturedClearanceError::Unsupported {
                projected: 5,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
    });
}

// Contact margin tests (LO-T2Q-001): with a positive F top margin, F's content box starts below
// B1's border box while its margin box can still touch it. Only the margin-box comparison
// (CSS2.2 §9.5) rejects that contact.

/// The `visible` control of a margin-box contact: F's margin top equals B1's border bottom, and
/// F's content top lies strictly below it.
fn assert_b1_margin_contact_control(nodes: usize, control: &str) {
    assert_b1_contact_control(nodes, control);
    sibling_fixture(
        nodes,
        &format!("{} {control}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.0,
                100.0,
                10.0,
                layout_limits(),
            )
            .unwrap_or_else(|error| panic!("{control}: {error:?}"));
            assert!(
                layout.sequence().float_content_vertical().top > layout.sequence().before().bottom,
                "{control}"
            );
        },
    );
}

#[test]
fn css_b1_overflow_contact_reads_the_float_margin_box() {
    // F's margin box starts on B1's border bottom (12) and its content box 3px lower (15).
    let control = "#b1 { margin-bottom:0px; } #f { margin-top:3px; }";
    assert_b1_margin_contact_control(6, control);
    float_overflow_geometry(control, |layout| {
        let sequence = layout.sequence();
        let placed = sequence.placed_float();
        let span = sequence.float_content_vertical();
        assert_eq!(
            [
                sequence.before().bottom,
                placed.top,
                placed.bottom,
                span.top,
                span.bottom
            ]
            .map(f32::to_bits),
            [12.0_f32, 12.0, 17.0, 15.0, 17.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_b1_contact(float_overflow_error(
        6,
        &format!("{control} #b1 {{ overflow:hidden; }}"),
    ));
    // A 1px B1 bottom margin separates the boxes: admitted with the `visible` layout.
    let control = "#b1 { margin-bottom:1px; } #f { margin-top:3px; }";
    let rule = format!("{control} #b1 {{ overflow:hidden; }}");
    assert_b1_overflow_case(6, control, &rule, HIDDEN);
    float_overflow_geometry(&rule, |layout| {
        let sequence = layout.sequence();
        let placed = sequence.placed_float();
        let span = sequence.float_content_vertical();
        assert_eq!(
            [
                sequence.before().bottom,
                placed.top,
                placed.bottom,
                span.top,
                span.bottom
            ]
            .map(f32::to_bits),
            [12.0_f32, 13.0, 18.0, 16.0, 18.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// Float side x used-zero B1 bottom margin x B1 form, with F's top margin 3px: each `visible`
/// control is a margin-box contact, and each B1 form gives the typed contact error. 12 rows per N.
fn assert_b1_margin_contact_matrix(nodes: usize) -> usize {
    let mut rows = 0;
    for side in ["left", "right"] {
        for bottom in ["0px", "auto"] {
            let control = format!(
                "{} #f {{ float:{side}; margin-top:3px; }} #b1 {{ margin-bottom:{bottom}; }}",
                leading_clear_source_rule(nodes)
            );
            assert_b1_margin_contact_control(nodes, &control);
            for value in ["hidden", "auto", "inherit"] {
                let rule = format!("{control} #b1 {{ overflow:{value}; }}");
                assert_b1_contact(float_overflow_error(nodes, &rule));
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 12, "N{nodes}");
    rows
}

#[test]
fn b1_overflow_float_top_margin_contact_matrix_is_typed() {
    let rows: usize = [5, 6, 7]
        .into_iter()
        .map(assert_b1_margin_contact_matrix)
        .sum();
    assert_eq!(rows, 36);
}

/// Float side x B1 form, with B1's bottom margin 1px and F's top margin 3px: F's margin top lies
/// 1px below B1's border bottom, so every form matches the `visible` layout. 6 per N.
fn assert_b1_float_top_margin_strict_matrix(nodes: usize) -> usize {
    let mut comparisons = 0;
    for side in ["left", "right"] {
        let control = format!(
            "{} #f {{ float:{side}; margin-top:3px; }} #b1 {{ margin-bottom:1px; }}",
            leading_clear_source_rule(nodes)
        );
        for (value, expected) in [
            ("hidden", HIDDEN),
            ("auto", AUTO),
            ("inherit", b1_inherited(nodes)),
        ] {
            let rule = format!("{control} #b1 {{ overflow:{value}; }}");
            assert_b1_overflow_case(nodes, &control, &rule, expected);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 6, "N{nodes}");
    comparisons
}

#[test]
fn b1_overflow_float_top_margin_strict_matrix_matches_the_visible_layouts() {
    let comparisons: usize = [5, 6, 7]
        .into_iter()
        .map(assert_b1_float_top_margin_strict_matrix)
        .sum();
    assert_eq!(comparisons, 18);
}

/// B1's border box cannot cross F's margin top: the only way down, a negative B1 bottom margin,
/// is a profile error before placement.
#[test]
fn b1_overflow_negative_bottom_margin_under_a_float_top_margin_is_a_profile_error() {
    let error = float_overflow_error(
        6,
        "#b1 { margin-bottom:-1px; overflow:hidden; } #f { margin-top:3px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 2,
                    reason: OwnerProfileReason::NegativeMargin,
                }
            ))
        ),
        "{error:?}"
    );
}

// Parent overflow auto: P admits computed `auto` only when every child's border box and margin box
// lie within P's published content box (POA plan).

/// `inherit` on a child of P takes P's computed `auto`; P's original index is N - 2.
fn p_auto_inherited(nodes: usize) -> KeywordBoxFact<BoxOverflow> {
    KeywordBoxFact {
        value: BoxOverflow::Auto,
        origin: BoxFactOrigin::Inherited {
            original_parent: nodes - 2,
        },
    }
}

/// The whole layout Debug and every fact row of `rule` equal those of `control_rule`, except that
/// each source in `changed` carries `auto` where the control carries `hidden`, with the same origin.
fn assert_parent_overflow_case(
    nodes: usize,
    control_rule: &str,
    rule: &str,
    changed: &[usize],
) -> OverflowRows {
    let (control_view, control_rows) =
        float_overflow_layout(nodes, &format!("{} {control_rule}", baseline_css()));
    let (view, rows) = float_overflow_layout(nodes, &format!("{} {rule}", baseline_css()));
    assert_eq!(view, control_view, "{rule}");
    assert_eq!(rows.len(), control_rows.len(), "{rule}");
    for (source, (row, control)) in rows.iter().zip(&control_rows).enumerate() {
        if changed.contains(&source) {
            assert_eq!(
                control.overflow.value,
                BoxOverflow::Hidden,
                "{rule}: {source}"
            );
            assert_eq!(
                row.overflow,
                KeywordBoxFact {
                    value: BoxOverflow::Auto,
                    origin: control.overflow.origin,
                },
                "{rule}: source {source}"
            );
            assert_eq!(
                ClearanceBoxFact {
                    overflow: control.overflow,
                    ..*row
                },
                *control,
                "{rule}: source {source}"
            );
        } else {
            assert_eq!(row, control, "{rule}: source {source}");
        }
    }
    rows
}

fn parent_overflow_rule(nodes: usize, side: &str) -> (String, String) {
    let control = format!(
        "{} #f {{ float:{side}; }}",
        leading_clear_source_rule(nodes)
    );
    let rule = format!("{control} #p {{ overflow:auto; }}");
    (control, rule)
}

/// The containment error at `source`, typed.
fn assert_parent_auto_overflow(error: SiblingLayoutError, source: usize) {
    match error {
        SiblingLayoutError::ParentAutoOverflow { source: actual } => assert_eq!(actual, source),
        other => panic!("expected ParentAutoOverflow at {source}, got {other:?}"),
    }
}

/// The `hidden` control of a containment row lays out, and P `auto` gives the containment error.
fn assert_parent_auto_overflow_row(nodes: usize, control_rule: &str, rule: &str, source: usize) {
    float_overflow_layout(nodes, &format!("{} {control_rule}", baseline_css()));
    assert_parent_auto_overflow(float_overflow_error(nodes, rule), source);
}

#[test]
fn css_parent_overflow_auto_contained_n6_left_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(6, "left");
    let rows = assert_parent_overflow_case(6, &control, &rule, &[1]);
    assert_eq!(rows[1].overflow, AUTO, "{rule}");
    assert_float_overflow_n6_left_geometry(&rule);
}

#[test]
fn css_parent_overflow_auto_contained_n6_right_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(6, "right");
    assert_parent_overflow_case(6, &control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_contained_n5_left_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(5, "left");
    assert_parent_overflow_case(5, &control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_contained_n5_right_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(5, "right");
    assert_parent_overflow_case(5, &control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_contained_n7_left_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(7, "left");
    assert_parent_overflow_case(7, &control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_contained_n7_right_matches_the_hidden_layout() {
    let (control, rule) = parent_overflow_rule(7, "right");
    assert_parent_overflow_case(7, &control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_admits_an_inheriting_b1_strictly_above_the_float() {
    let control = "#b1 { overflow:inherit; }";
    let rule = format!("#p {{ overflow:auto; }} {control}");
    let rows = assert_parent_overflow_case(6, control, &rule, &[1, 2]);
    assert_eq!(rows[2].overflow, p_auto_inherited(6), "{rule}");
}

#[test]
fn css_parent_overflow_auto_admits_an_inheriting_float() {
    let control = "#f { overflow:inherit; }";
    let rule = format!("#p {{ overflow:auto; }} {control}");
    let rows = assert_parent_overflow_case(6, control, &rule, &[1, 3]);
    assert_eq!(rows[3].overflow, p_auto_inherited(6), "{rule}");
}

#[test]
fn css_parent_overflow_auto_exact_explicit_height_matches_the_hidden_layout() {
    // B3's margin bottom 27 equals P's content bottom 27.
    assert_parent_overflow_case(
        6,
        "#p { height:16px; }",
        "#p { overflow:auto; height:16px; }",
        &[1],
    );
}

#[test]
fn css_parent_overflow_auto_one_short_is_typed() {
    assert_parent_auto_overflow_row(
        6,
        "#p { height:15px; }",
        "#p { overflow:auto; height:15px; }",
        5,
    );
}

#[test]
fn css_parent_overflow_auto_margin_box_only_is_typed() {
    // B3's border box 22..23 lies within 11..24; its margin bottom 27 does not.
    assert_parent_auto_overflow_row(
        6,
        "#p { height:13px; }",
        "#p { overflow:auto; height:13px; }",
        5,
    );
}

#[test]
fn css_parent_overflow_auto_reports_the_first_failing_child() {
    // F's bottom 18 lies below 16; B2 and B3 also fail.
    assert_parent_auto_overflow_row(
        6,
        "#p { height:5px; }",
        "#p { overflow:auto; height:5px; }",
        3,
    );
}

#[test]
fn css_parent_overflow_auto_border_box_only_is_typed() {
    // B2's border left is -1; its margin edge is back at 0.
    assert_parent_auto_overflow_row(
        6,
        "#b2 { margin-left:-1px; }",
        "#p { overflow:auto; } #b2 { margin-left:-1px; }",
        4,
    );
}

#[test]
fn css_parent_overflow_auto_overconstrained_width_is_typed() {
    // B2's border right is 101, with a used right margin of -1.
    assert_parent_auto_overflow_row(
        6,
        "#b2 { width:101px; }",
        "#p { overflow:auto; } #b2 { width:101px; }",
        4,
    );
}

#[test]
fn css_parent_overflow_auto_float_margin_box_only_is_typed() {
    // F's content box is 0..10; its margin box ends at 105.
    assert_parent_auto_overflow_row(
        6,
        "#f { margin-right:95px; }",
        "#p { overflow:auto; } #f { margin-right:95px; }",
        3,
    );
}

#[test]
fn css_parent_overflow_auto_right_oversized_float_is_typed() {
    assert_parent_auto_overflow_row(
        6,
        "#f { float:right; width:101px; }",
        "#p { overflow:auto; } #f { float:right; width:101px; }",
        3,
    );
}

#[test]
fn css_parent_overflow_auto_left_oversized_float_is_typed() {
    assert_parent_auto_overflow_row(
        6,
        "#f { width:101px; }",
        "#p { overflow:auto; } #f { width:101px; }",
        3,
    );
}

#[test]
fn css_parent_overflow_auto_rounded_top_margin_edge_is_typed() {
    // B1's top is RN(16777225) = 16777224 (a tie, to even); its staged margin top is 10 < 11.
    assert_parent_auto_overflow_row(
        6,
        "#b1 { margin-top:16777214px; }",
        "#p { overflow:auto; } #b1 { margin-top:16777214px; }",
        2,
    );
}

#[test]
fn css_parent_overflow_auto_unrounded_top_margin_edge_matches_the_hidden_layout() {
    assert_parent_overflow_case(
        6,
        "#b1 { margin-top:16777215px; }",
        "#p { overflow:auto; } #b1 { margin-top:16777215px; }",
        &[1],
    );
}

#[test]
fn css_parent_overflow_auto_absorbed_margin_at_2p24_matches_the_hidden_layout() {
    // P's content bottom and B3's border bottom are 2^24; RN(2^24 + 1) = 2^24.
    let control = "#p { height:16777205px; } #b3 { height:16777194px; margin-bottom:1px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
}

#[test]
fn css_parent_overflow_auto_kept_margin_at_2p24_is_typed() {
    let control = "#p { height:16777205px; } #b3 { height:16777194px; margin-bottom:2px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_auto_overflow_row(6, control, &rule, 5);
}

#[test]
fn css_parent_overflow_auto_keeps_b1_contact_first() {
    let control = "#b1 { overflow:inherit; margin-bottom:0px; }";
    let mut rows = 0;
    for rule in [
        control.to_owned(),
        format!("#p {{ overflow:auto; }} {control}"),
    ] {
        let error = float_overflow_error(6, &rule);
        assert!(
            matches!(
                error,
                SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                    source: 2,
                    error: ClearanceOwnerError::NewBfcFloatContact,
                })
            ),
            "{rule}: {error:?}"
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_parent_overflow_auto_keeps_tail_equality_first() {
    let control = "#p { height:5px; } #b1 { margin-bottom:0px; } #b2 { margin-top:2px; }";
    let mut rows = 0;
    for rule in [
        control.to_owned(),
        format!("{control} #p {{ overflow:auto; }}"),
    ] {
        let error = float_overflow_error(6, &rule);
        assert!(
            matches!(
                error,
                SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                    source: 4,
                    error: ClearanceOwnerError::UnsupportedEquality,
                })
            ),
            "{rule}: {error:?}"
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_parent_overflow_auto_keeps_the_bottom_error_first() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let control = format!("#p {{ height:5px; padding-bottom:{MAX}; margin-bottom:{MAX}; }}");
    let mut rows = 0;
    for rule in [
        control.clone(),
        format!("{control} #p {{ overflow:auto; }}"),
    ] {
        let error = float_overflow_error(6, &rule);
        assert!(
            matches!(
                error,
                SiblingLayoutError::Bottom(BlockBottomError::Unrepresentable {
                    stage: BlockBottomStage::MarginBottom,
                })
            ),
            "{rule}: {error:?}"
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_parent_overflow_auto_inherited_from_the_outer_matches_the_hidden_layout() {
    let rows = assert_parent_overflow_case(
        6,
        "#o { overflow:hidden; } #p { overflow:inherit; }",
        "#o { overflow:auto; } #p { overflow:inherit; }",
        &[0, 1],
    );
    assert_eq!(rows[1].overflow.value, BoxOverflow::Auto);
    assert!(matches!(
        rows[1].overflow.origin,
        BoxFactOrigin::Inherited { .. }
    ));
}

/// POA-D-001: P's content box is 2^24..2^24 + 8. F's margin box 2^24..2^24 + 8 lies on P's edges,
/// but its content band is 2^24 - 1..2^24 + 4, because its near edge 2^24 + 5 rounds to 2^24 + 4
/// (a tie, to even). Only F's border box leaves P.
#[test]
fn css_parent_overflow_auto_float_border_box_only_is_typed() {
    let control = "#f { float:right; width:5px; margin-right:3px; }";
    let observe = |rule: &str| {
        let mut out = None;
        sibling_fixture(
            6,
            &format!("{} {rule}", baseline_css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                out = Some(
                    default_outer_layout(
                        &facts,
                        &projection.nodes()[0],
                        16_777_216.0,
                        8.0,
                        10.0,
                        layout_limits(),
                    )
                    .map(|layout| {
                        let placed = layout.sequence().placed_float();
                        let band = layout.children()[1].border();
                        [placed.left, placed.right, band.left, band.right].map(f32::to_bits)
                    }),
                );
            },
        );
        out.unwrap()
    };
    assert_eq!(
        observe(control).unwrap(),
        [16_777_216.0_f32, 16_777_224.0, 16_777_215.0, 16_777_220.0].map(f32::to_bits),
        "{control}"
    );
    assert_parent_auto_overflow(
        observe(&format!("{control} #p {{ overflow:auto; }}")).unwrap_err(),
        3,
    );
}

// Parent overflow auto Task2: the contained and uncontained matrices, the negatives and the work
// thresholds.

/// P's exact explicit height under the source-count rule: the last tail's margin bottom meets P's
/// content bottom (P's content top is 11).
fn parent_overflow_exact_height(nodes: usize) -> usize {
    match nodes {
        5 => 8,
        6 => 16,
        7 => 20,
        _ => panic!("no parent-overflow fixture for N{nodes}"),
    }
}

/// Side x P height (Auto, exact) x B1 (`visible`, `hidden`, `inherit`) x F (`visible`, `auto`,
/// `inherit`) against the same CSS with P `hidden`: 36 comparisons per N, all contained.
fn assert_parent_overflow_matrix(nodes: usize) -> usize {
    let exact = format!("#p {{ height:{}px; }}", parent_overflow_exact_height(nodes));
    let mut comparisons = 0;
    for side in ["left", "right"] {
        for height in ["", exact.as_str()] {
            for (b1, b1_inherits) in [
                ("", false),
                ("#b1 { overflow:hidden; }", false),
                ("#b1 { overflow:inherit; }", true),
            ] {
                for (float, float_inherits) in [
                    ("", false),
                    ("#f { overflow:auto; }", false),
                    ("#f { overflow:inherit; }", true),
                ] {
                    let control = format!(
                        "{} #f {{ float:{side}; }} {height} {b1} {float}",
                        leading_clear_source_rule(nodes)
                    );
                    let rule = format!("{control} #p {{ overflow:auto; }}");
                    let mut changed = vec![1];
                    if b1_inherits {
                        changed.push(2);
                    }
                    if float_inherits {
                        changed.push(3);
                    }
                    let rows = assert_parent_overflow_case(nodes, &control, &rule, &changed);
                    assert_eq!(rows[1].overflow, AUTO, "{rule}");
                    for &source in &changed[1..] {
                        assert_eq!(rows[source].overflow, p_auto_inherited(nodes), "{rule}");
                    }
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 36, "N{nodes}");
    comparisons
}

#[test]
fn parent_overflow_matrix_matches_the_hidden_layouts() {
    let comparisons: usize = [5, 6, 7]
        .into_iter()
        .map(assert_parent_overflow_matrix)
        .sum();
    assert_eq!(comparisons, 108);
}

/// Side x {P height one short, F width 101px, the last tail's left margin -1px}: 6 typed rows per
/// N, each after its `hidden` control lays out. The sources are N - 1, 3 and N - 1.
fn assert_parent_overflow_uncontained_matrix(nodes: usize) -> usize {
    let last = ["#b2", "#b3", "#b4"][nodes - 5];
    let edits = [
        (
            format!(
                "#p {{ height:{}px; }}",
                parent_overflow_exact_height(nodes) - 1
            ),
            nodes - 1,
        ),
        ("#f { width:101px; }".to_owned(), 3),
        (format!("{last} {{ margin-left:-1px; }}"), nodes - 1),
    ];
    let mut rows = 0;
    for side in ["left", "right"] {
        for (edit, source) in &edits {
            let control = format!(
                "{} #f {{ float:{side}; }} {edit}",
                leading_clear_source_rule(nodes)
            );
            let rule = format!("{control} #p {{ overflow:auto; }}");
            assert_parent_auto_overflow_row(nodes, &control, &rule, *source);
            rows += 1;
        }
    }
    assert_eq!(rows, 6, "N{nodes}");
    rows
}

#[test]
fn parent_overflow_uncontained_matrix_is_typed() {
    let rows: usize = [5, 6, 7]
        .into_iter()
        .map(assert_parent_overflow_uncontained_matrix)
        .sum();
    assert_eq!(rows, 18);
}

#[test]
fn parent_overflow_scroll_and_visible_stay_parent_overflow() {
    let mut rows = 0;
    for rule in [
        "#p { overflow:scroll; }",
        "#p { overflow:visible; }",
        "#p { overflow:scroll; } #b1 { overflow:inherit; }",
    ] {
        assert_captured(
            float_overflow_error(6, rule),
            CapturedClearanceError::Unsupported {
                projected: 1,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ParentOverflow,
            },
        );
        rows += 1;
    }
    assert_eq!(rows, 3);
}

#[test]
fn parent_overflow_auto_child_overflow_stays_typed() {
    let mut rows = 0;
    for (rule, projected) in [
        ("#p { overflow:auto; } #b1 { overflow:scroll; }", 2),
        ("#p { overflow:auto; } #b3 { overflow:inherit; }", 5),
    ] {
        assert_captured(
            float_overflow_error(6, rule),
            CapturedClearanceError::Unsupported {
                projected,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn css_parent_overflow_auto_generated_content_stays_a_fact_failure() {
    let css = format!(
        "{} #p:before {{ content:'x'; }} #p {{ overflow:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        assert!(matches!(
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()),
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Unsupported {
                    original: 4,
                    target: BoxFactTarget::Before,
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                }
            ))
        ));
    });
}

#[test]
fn parent_overflow_auto_facade_work_thresholds_are_exact() {
    for (nodes, exact, core) in [(5, 594, 481), (6, 766, 563), (7, 938, 646)] {
        let css = format!(
            "{} {} #p {{ overflow:auto; }}",
            baseline_css(),
            leading_clear_source_rule(nodes)
        );
        sibling_fixture(nodes, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            assert_eq!(facts.row(1).unwrap().overflow, AUTO, "N{nodes}");
            assert!(
                matches!(
                    float_overflow_build(&facts, &projection.nodes()[0], exact - 1),
                    Err(SiblingLayoutError::Work)
                ),
                "N{nodes} work {}",
                exact - 1
            );
            let layout = float_overflow_build(&facts, &projection.nodes()[0], exact).unwrap();
            assert_eq!(layout.receipt().work, exact, "N{nodes}");
            assert_eq!(exact, 80 + 32 * nodes + 74 * (nodes - 2) + 66 * (nodes - 3));
            assert_eq!(layout.receipt().core.work, core, "N{nodes}");
            assert_eq!(layout.sequence().receipt().work, core, "N{nodes}");
        });
    }
    // The work is prepaid before the profile scan reaches the later child's overflow.
    let css = format!(
        "{} #p {{ overflow:auto; }} #b3 {{ overflow:auto; }}",
        baseline_css()
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            float_overflow_build(&facts, &projection.nodes()[0], 765),
            Err(SiblingLayoutError::Work)
        ));
        assert_captured(
            float_overflow_build(&facts, &projection.nodes()[0], 766).unwrap_err(),
            CapturedClearanceError::Unsupported {
                projected: 5,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ChildOverflow,
            },
        );
    });
}

// Parent overflow auto Task2-C1 (POA-T2Q-R1): a normal child's horizontal margin edges, F's far
// vertical edge, P's horizontal padding and P `auto` inherited from O on an uncontained row.

/// B2's border left and right and its used left and right margins, as bits.
fn parent_overflow_b2_edges(layout: &CapturedSiblingLayout<'_>) -> [u32; 4] {
    let b2 = &layout.children()[2];
    [
        b2.border().left,
        b2.border().right,
        b2.used_margin_left(),
        b2.used_margin_right(),
    ]
    .map(f32::to_bits)
}

/// The layout of `rule` at N6 in R26's outer geometry: O's content box starts at 2^24 and is
/// `width` wide, so P's content box is 2^24..2^24 + `width`. B2's edges, or the build's error.
fn parent_overflow_2p24_b2_edges(width: f32, rule: &str) -> Result<[u32; 4], SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(
        6,
        &format!("{} {rule}", baseline_css()),
        |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            out = Some(
                default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    16_777_216.0,
                    width,
                    10.0,
                    layout_limits(),
                )
                .map(|layout| parent_overflow_b2_edges(&layout)),
            );
        },
    );
    out.unwrap()
}

#[test]
fn css_parent_overflow_auto_normal_margin_left_matches_the_hidden_layout() {
    // B2's border box 60..100 and its margin box 0..100 lie within P's content box 0..100.
    let control = "#b2 { margin-left:60px; }";
    let rule = format!("#p {{ overflow:auto; }} {control}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [60.0_f32, 100.0, 60.0, 0.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

#[test]
fn css_parent_overflow_auto_normal_margin_right_matches_the_hidden_layout() {
    // B2's border box 0..40 and its margin box 0..100 lie within P's content box 0..100.
    let control = "#b2 { margin-right:60px; }";
    let rule = format!("#p {{ overflow:auto; }} {control}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [0.0_f32, 40.0, 0.0, 60.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// P's content box is 2^24..2^24 + 8. B2's border left RN(2^24 + 1) = 2^24 and border right
/// RN(2^24 + 7) = 2^24 + 8 (ties, to even) lie on P's edges; its staged margin left 2^24 - 1 does
/// not. F is 5px wide, so its margin box 2^24..2^24 + 4 fits.
#[test]
fn css_parent_overflow_auto_rounded_left_margin_edge_is_typed() {
    let control = "#f { width:5px; } #b2 { margin-left:1px; }";
    assert_eq!(
        parent_overflow_2p24_b2_edges(8.0, control).unwrap(),
        [16_777_216.0_f32, 16_777_224.0, 1.0, 0.0].map(f32::to_bits),
        "{control}"
    );
    assert_parent_auto_overflow(
        parent_overflow_2p24_b2_edges(8.0, &format!("{control} #p {{ overflow:auto; }}"))
            .unwrap_err(),
        4,
    );
}

/// P's content box is 2^24..2^24 + 10. B2's border box 2^24..RN(2^24 + 7) = 2^24 + 8 lies within
/// it; its staged margin right RN(2^24 + 8 + 3) = 2^24 + 12 (a tie, to even) does not.
#[test]
fn css_parent_overflow_auto_rounded_right_margin_edge_is_typed() {
    let control = "#b2 { margin-right:3px; }";
    assert_eq!(
        parent_overflow_2p24_b2_edges(10.0, control).unwrap(),
        [16_777_216.0_f32, 16_777_224.0, 0.0, 3.0].map(f32::to_bits),
        "{control}"
    );
    assert_parent_auto_overflow(
        parent_overflow_2p24_b2_edges(10.0, &format!("{control} #p {{ overflow:auto; }}"))
            .unwrap_err(),
        4,
    );
}

/// POA-D-001's vertical twin: F's margin box 16..16777232 ends on P's content bottom 16777232,
/// but its content top RN(16 + 16777215) = 16777232 (a tie, to even) puts its content bottom at
/// 16777234. Only F's border box leaves P.
#[test]
fn css_parent_overflow_auto_float_content_bottom_is_typed() {
    let control = "#p { height:16777221px; } #f { margin-top:16777215px; }";
    float_overflow_geometry(control, |layout| {
        let placed = layout.sequence().placed_float();
        let band = layout.children()[1].border();
        assert_eq!(
            [
                placed.top,
                placed.bottom,
                band.top,
                band.bottom,
                layout.parent().content().bottom,
            ]
            .map(f32::to_bits),
            [
                16.0_f32,
                16_777_232.0,
                16_777_232.0,
                16_777_234.0,
                16_777_232.0
            ]
            .map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow(
        float_overflow_error(6, &format!("{control} #p {{ overflow:auto; }}")),
        3,
    );
}

#[test]
fn css_parent_overflow_auto_left_padding_is_typed() {
    // B2's border left 1 lies within P's padding box 0..100 but not within its content box 2..100.
    let control = "#p { padding-left:2px; } #b2 { margin-left:-1px; }";
    float_overflow_geometry(control, |layout| {
        let area = layout.parent().content();
        assert_eq!(
            [area.left, area.right].map(f32::to_bits),
            [2.0_f32, 100.0].map(f32::to_bits),
            "{control}"
        );
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [1.0_f32, 100.0, -1.0, 0.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow(
        float_overflow_error(6, &format!("{control} #p {{ overflow:auto; }}")),
        4,
    );
}

#[test]
fn css_parent_overflow_auto_right_padding_is_typed() {
    // B2's border right 98 lies within P's padding box 0..100 but not within its content box 0..97.
    let control = "#p { padding-right:3px; } #b2 { margin-right:-1px; }";
    float_overflow_geometry(control, |layout| {
        let area = layout.parent().content();
        assert_eq!(
            [area.left, area.right].map(f32::to_bits),
            [0.0_f32, 97.0].map(f32::to_bits),
            "{control}"
        );
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [0.0_f32, 98.0, 0.0, -1.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow(
        float_overflow_error(6, &format!("{control} #p {{ overflow:auto; }}")),
        4,
    );
}

#[test]
fn css_parent_overflow_auto_inherited_from_the_outer_one_short_is_typed() {
    // POA-T2Q-004: P's `auto` is inherited from O. As with `#p { overflow:auto; height:15px; }`,
    // B3's margin bottom 27 lies below P's content bottom 26.
    assert_parent_auto_overflow_row(
        6,
        "#o { overflow:hidden; } #p { overflow:inherit; height:15px; }",
        "#o { overflow:auto; } #p { overflow:inherit; height:15px; }",
        5,
    );
}

// Parent overflow auto Task2-C1r1 (POA-C1-R1): an inverted staged margin box, F's margin-box
// vertical edges and the staged rounding of a normal's left, right and top margin edges.

/// POA-C1R-001: P's content box is 2^24..2^24 (zero width). B2's border box 2^24..RN(2^24 + 1) =
/// 2^24 lies on it, but its used margin right is -1, so its staged margin box 2^24..2^24 - 1 is
/// inverted and its right edge lies left of P. Each edge is compared on its own.
#[test]
fn css_parent_overflow_auto_inverted_margin_box_is_typed() {
    let control =
        "#p { margin-left:16777216px; width:0px; } #f { width:0px; } #b2 { margin-right:-1px; }";
    float_overflow_geometry(control, |layout| {
        let area = layout.parent().content();
        assert_eq!(
            [area.left, area.right].map(f32::to_bits),
            [16_777_216.0_f32, 16_777_216.0].map(f32::to_bits),
            "{control}"
        );
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [16_777_216.0_f32, 16_777_216.0, 0.0, -1.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow(
        float_overflow_error(6, &format!("{control} #p {{ overflow:auto; }}")),
        4,
    );
}

/// POA-C1R-002: F's border box 16..18 lies within P's content box 11..27, but its margin box
/// ends at 38. Only F's margin box leaves P, and only vertically.
#[test]
fn css_parent_overflow_auto_float_margin_bottom_is_typed() {
    let control = "#p { height:16px; } #f { margin-bottom:20px; }";
    float_overflow_geometry(control, |layout| {
        let placed = layout.sequence().placed_float();
        let band = layout.children()[1].border();
        assert_eq!(
            [
                placed.top,
                placed.bottom,
                band.top,
                band.bottom,
                layout.parent().content().bottom
            ]
            .map(f32::to_bits),
            [16.0_f32, 38.0, 16.0, 18.0, 27.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow(
        float_overflow_error(6, &format!("{control} #p {{ overflow:auto; }}")),
        3,
    );
}

/// POA-C1R-003: P's content box is 2^24..2^24 + 8. B2's border box is 2^24..2^24 + 8 with a used
/// margin left of 0.5; its staged margin left RN(2^24 - 0.5) = 2^24 (a tie, to even) lies on P's
/// edge, so the layout is admitted, although the exact edge 2^24 - 0.5 lies outside.
#[test]
fn css_parent_overflow_auto_staged_left_margin_edge_matches_the_hidden_layout() {
    let control =
        "#p { margin-left:16777216px; width:8px; } #f { width:5px; } #b2 { margin-left:0.5px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [16_777_216.0_f32, 16_777_224.0, 0.5, 0.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// POA-C1R-003: B2's border box is 2^24..2^24 + 8 with a used margin right of 0.5; its staged
/// margin right RN(2^24 + 8.5) = 2^24 + 8 lies on P's edge, the exact edge 2^24 + 8.5 outside.
#[test]
fn css_parent_overflow_auto_staged_right_margin_edge_matches_the_hidden_layout() {
    let control =
        "#p { margin-left:16777216px; width:8px; } #f { width:5px; } #b2 { margin-right:0.5px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [16_777_216.0_f32, 16_777_224.0, 0.0, 0.5].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// POA-C1R-003: P's content top is RN(10 + 16777206 + 1) = 2^24. B1's top is RN(2^24 + 0.5) = 2^24
/// and its staged margin top RN(2^24 - 0.5) = 2^24 (ties, to even) lies on P's edge; the exact edge
/// 2^24 - 0.5 lies outside.
#[test]
fn css_parent_overflow_auto_staged_top_margin_edge_matches_the_hidden_layout() {
    let control = "#p { margin-top:16777206px; } #b1 { margin-top:0.5px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        let b1 = &layout.children()[0];
        assert_eq!(
            [
                layout.parent().content().top,
                b1.border().top,
                b1.used_margin_top(),
            ]
            .map(f32::to_bits),
            [16_777_216.0_f32, 16_777_216.0, 0.5].map(f32::to_bits),
            "{rule}"
        );
    });
}

// Parent overflow auto Task2-C1r2 (POA-C1-R2): used rather than computed horizontal margins, F's
// placed margin box, and F's top on P's content top.

/// POA-C1R1-001: B2 is over-constrained, so its used margin right is 100 - 90 = 10, not the
/// computed 20; its margin box 0..100 lies within P.
#[test]
fn css_parent_overflow_auto_used_margin_right_matches_the_hidden_layout() {
    let control = "#b2 { width:90px; margin-right:20px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [0.0_f32, 90.0, 0.0, 10.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// POA-C1R1-001, left twin: with P `rtl` the over-constrained B2's used margin left is 10, not the
/// computed 20; its margin box 0..100 lies within P.
#[test]
fn css_parent_overflow_auto_used_margin_left_matches_the_hidden_layout() {
    let control = "#p { direction:rtl; } #b2 { width:90px; margin-left:20px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            parent_overflow_b2_edges(layout),
            [10.0_f32, 100.0, 10.0, 0.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

/// POA-C1R1-002: F's margin box is its placed box 2^24..RN(2^24 + 1) = 2^24, and its content box
/// is RN(2^24 + 1)..2^24 = 2^24..2^24, both within P's content box 2^24..2^24 + 8. F's content
/// edge moved by its used margin left 1 would be 2^24 - 1, outside.
#[test]
fn css_parent_overflow_auto_placed_float_margin_box_matches_the_hidden_layout() {
    let control = "#p { margin-left:16777216px; width:8px; } #f { width:0px; margin-left:1px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        let placed = layout.sequence().placed_float();
        let f = &layout.children()[1];
        assert_eq!(
            [
                placed.left,
                placed.right,
                f.border().left,
                f.border().right,
                f.used_margin_left(),
            ]
            .map(f32::to_bits),
            [
                16_777_216.0_f32,
                16_777_216.0,
                16_777_216.0,
                16_777_216.0,
                1.0
            ]
            .map(f32::to_bits),
            "{rule}"
        );
    });
}

/// POA-C1R1-003 (model-evidenced only): P's content top is 2^24, B1's height is absorbed, and F's
/// placed top and content top both lie on P's content top.
#[test]
fn css_parent_overflow_auto_float_top_on_the_content_top_matches_the_hidden_layout() {
    let control = "#p { margin-top:16777206px; } #b1 { margin-bottom:0px; }";
    let rule = format!("{control} #p {{ overflow:auto; }}");
    assert_parent_overflow_case(6, control, &rule, &[1]);
    float_overflow_geometry(&rule, |layout| {
        assert_eq!(
            [
                layout.parent().content().top,
                layout.sequence().placed_float().top,
                layout.children()[1].border().top,
            ]
            .map(f32::to_bits),
            [16_777_216.0_f32, 16_777_216.0, 16_777_216.0].map(f32::to_bits),
            "{rule}"
        );
    });
}

// Parent overflow auto Task2-C1r3 (POA-C1-R3): a right float's margin-box left edge.

/// POA-C1R2-001: F floats right with a used margin left of 95. Its content box 90..100 lies within
/// P's content box 0..100, but its placed box starts at 100 - 10 - 95 = -5.
#[test]
fn css_parent_overflow_auto_right_float_margin_left_is_typed() {
    let control = "#f { float:right; margin-left:95px; }";
    float_overflow_geometry(control, |layout| {
        let placed = layout.sequence().placed_float();
        let f = &layout.children()[1];
        assert_eq!(
            [
                placed.left,
                placed.right,
                f.border().left,
                f.used_margin_left()
            ]
            .map(f32::to_bits),
            [-5.0_f32, 100.0, 90.0, 95.0].map(f32::to_bits),
            "{control}"
        );
    });
    assert_parent_auto_overflow_row(6, control, &format!("#p {{ overflow:auto; }} {control}"), 3);
}

// Signed float margins (SFM plan): F's own margins may be negative (CSS2.2 §8.3) while both exact
// outer extents are nonnegative; any other signed F margin is `NegativeFloatOuterExtent` at F.
// Every expected value below is the ported model's (plan package, outputs/reds-model.json).

const SFM_OUTER: (f32, f32, f32) = (0.0, 100.0, 10.0);
/// P's content box 2^24..2^24 + 8 (design, "Binary32 rounding and staging", item 6).
const SFM_OUTER_2P24: (f32, f32, f32) = (16_777_216.0, 8.0, 10.0);

/// The published geometry an SFM row pins, as binary32 bits: F's margin box, content box and used
/// margins (each left, right, top, bottom), B1's top and bottom, every tail's top, bottom and
/// clearance, and P's content box (left, right, top, bottom) and used height.
#[derive(Debug, PartialEq)]
struct SfmGeometry {
    float_margin_box: [u32; 4],
    float_content: [u32; 4],
    float_used_margins: [u32; 4],
    b1: [u32; 2],
    tails: Vec<(u32, u32, Option<u32>)>,
    parent: [u32; 5],
}

fn sfm_geometry(
    float_margin_box: [f32; 4],
    float_content: [f32; 4],
    float_used_margins: [f32; 4],
    b1: [f32; 2],
    tails: &[(f32, f32, Option<f32>)],
    parent: [f32; 5],
) -> SfmGeometry {
    SfmGeometry {
        float_margin_box: float_margin_box.map(f32::to_bits),
        float_content: float_content.map(f32::to_bits),
        float_used_margins: float_used_margins.map(f32::to_bits),
        b1: b1.map(f32::to_bits),
        tails: tails
            .iter()
            .map(|&(top, bottom, clearance)| {
                (top.to_bits(), bottom.to_bits(), clearance.map(f32::to_bits))
            })
            .collect(),
        parent: parent.map(f32::to_bits),
    }
}

fn sfm_view(layout: &CapturedSiblingLayout<'_>) -> SfmGeometry {
    let sequence = layout.sequence();
    let placed = sequence.placed_float();
    let float = &layout.children()[1];
    assert_eq!(float.source_index(), 3);
    let content = float.content();
    let before = sequence.before();
    let parent = layout.parent().content();
    let tails: Vec<_> = sequence
        .tail()
        .iter()
        .map(|tail| {
            (
                tail.vertical().top,
                tail.vertical().bottom,
                tail.clearance(),
            )
        })
        .collect();
    sfm_geometry(
        [placed.left, placed.right, placed.top, placed.bottom],
        [content.left, content.right, content.top, content.bottom],
        [
            float.used_margin_left(),
            float.used_margin_right(),
            float.used_margin_top(),
            float.used_margin_bottom(),
        ],
        [before.top, before.bottom],
        &tails,
        [
            parent.left,
            parent.right,
            parent.top,
            parent.bottom,
            layout.parent().height().used_height,
        ],
    )
}

/// Build the SFM row `rule` at `nodes` sources, after `baseline_css()` and the source-count rule,
/// with the outer content geometry (left, width, top).
fn sfm_build(
    nodes: usize,
    outer: (f32, f32, f32),
    rule: &str,
) -> Result<SfmGeometry, SiblingLayoutError> {
    let css = format!(
        "{} {} {rule}",
        baseline_css(),
        leading_clear_source_rule(nodes)
    );
    let mut out = None;
    sibling_fixture(nodes, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        out = Some(
            default_outer_layout(
                &facts,
                &projection.nodes()[0],
                outer.0,
                outer.1,
                outer.2,
                layout_limits(),
            )
            .map(|layout| sfm_view(&layout)),
        );
    });
    out.unwrap()
}

/// An SFM row that lays out.
fn sfm_layout(nodes: usize, outer: (f32, f32, f32), rule: &str) -> SfmGeometry {
    sfm_build(nodes, outer, rule).unwrap_or_else(|error| panic!("{rule}: {error:?}"))
}

/// An SFM row that fails.
fn sfm_error(nodes: usize, outer: (f32, f32, f32), rule: &str) -> SiblingLayoutError {
    match sfm_build(nodes, outer, rule) {
        Ok(view) => panic!("{rule}: expected an error, got {view:?}"),
        Err(error) => error,
    }
}

/// The outer-extent rejection at F, typed.
fn assert_sfm_extent_rejected(error: SiblingLayoutError) {
    match error {
        SiblingLayoutError::Core(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::NegativeFloatOuterExtent,
            },
        )) => {}
        other => panic!("expected NegativeFloatOuterExtent at 3, got {other:?}"),
    }
}

/// The `hidden` control of an SFM containment row lays out, and P `auto` gives the containment
/// error at F (source 3).
fn assert_sfm_auto_overflow(nodes: usize, outer: (f32, f32, f32), control: &str, rule: &str) {
    sfm_layout(nodes, outer, control);
    assert_parent_auto_overflow(sfm_error(nodes, outer, rule), 3);
}

/// R1: F's margin box stays 0..10 x 16..18; its content band moves to -3..7.
#[test]
fn css_signed_float_margins_left_shift_publishes_the_content_band() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-left:-3px; margin-right:3px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [-3.0, 7.0, 16.0, 18.0],
            [-3.0, 3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R2: a right F's margin box stays 90..100; its content band moves to 93..103.
#[test]
fn css_signed_float_margins_right_float_right_shift_publishes_the_content_band() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#f { float:right; margin-right:-3px; margin-left:3px; }"
        ),
        sfm_geometry(
            [90.0, 100.0, 16.0, 18.0],
            [93.0, 103.0, 16.0, 18.0],
            [3.0, -3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R3: F's margin box stays 16..18; its content span moves to 13..15.
#[test]
fn css_signed_float_margins_up_shift_publishes_the_content_span() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-top:-3px; margin-bottom:3px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 10.0, 13.0, 15.0],
            [0.0, 0.0, -3.0, 3.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R4: the bottom margin lifts F's margin bottom to 17, so B2 is required at 17 with clearance -2.
#[test]
fn css_signed_float_margins_negative_bottom_lifts_the_clearance_edge() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-bottom:-1px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 17.0],
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 0.0, 0.0, -1.0],
            [11.0, 12.0],
            &[(17.0, 18.0, Some(-2.0)), (21.0, 22.0, None)],
            [0.0, 100.0, 11.0, 26.0, 15.0]
        )
    );
}

/// R5: the margin box narrows to 0..9; the content band is -1..9.
#[test]
fn css_signed_float_margins_negative_left_narrows_the_margin_box() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-left:-1px; }"),
        sfm_geometry(
            [0.0, 9.0, 16.0, 18.0],
            [-1.0, 9.0, 16.0, 18.0],
            [-1.0, 0.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R6: a right F's margin box narrows to 91..100; the content band is 91..101.
#[test]
fn css_signed_float_margins_right_float_negative_right_narrows_the_margin_box() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { float:right; margin-right:-1px; }"),
        sfm_geometry(
            [91.0, 100.0, 16.0, 18.0],
            [91.0, 101.0, 16.0, 18.0],
            [0.0, -1.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R7: the outer width is exactly 0: margin box 0..0, content band -11..-1.
#[test]
fn css_signed_float_margins_zero_outer_width_is_admitted() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-left:-11px; margin-right:1px; }"),
        sfm_geometry(
            [0.0, 0.0, 16.0, 18.0],
            [-11.0, -1.0, 16.0, 18.0],
            [-11.0, 1.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R8: the outer height is exactly 0: margin box 16..16, content span 13..15 (B2 `clear:none`).
#[test]
fn css_signed_float_margins_zero_outer_height_is_admitted() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#b2 { clear:none; } #f { margin-top:-3px; margin-bottom:1px; }"
        ),
        sfm_geometry(
            [0.0, 10.0, 16.0, 16.0],
            [0.0, 10.0, 13.0, 15.0],
            [0.0, 0.0, -3.0, 1.0],
            [11.0, 12.0],
            &[(16.0, 17.0, None), (20.0, 21.0, None)],
            [0.0, 100.0, 11.0, 25.0, 14.0]
        )
    );
}

/// R8e: the zero-height margin bottom 16 equals B2's hypothetical top, so equality stays typed.
#[test]
fn css_signed_float_margins_zero_outer_height_on_the_tail_top_is_equality() {
    let error = sfm_error(6, SFM_OUTER, "#f { margin-top:-3px; margin-bottom:1px; }");
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality
            })
        ),
        "{error:?}"
    );
}

/// R9: the outer width -1 is rejected.
#[test]
fn css_signed_float_margins_negative_outer_width_is_typed() {
    assert_sfm_extent_rejected(sfm_error(6, SFM_OUTER, "#f { margin-left:-11px; }"));
}

/// R10: the outer height -1 (bottom margin) is rejected.
#[test]
fn css_signed_float_margins_negative_outer_height_by_the_bottom_margin_is_typed() {
    assert_sfm_extent_rejected(sfm_error(6, SFM_OUTER, "#f { margin-bottom:-3px; }"));
}

/// R10t: the outer height -1 (top margin) is rejected.
#[test]
fn css_signed_float_margins_negative_outer_height_by_the_top_margin_is_typed() {
    assert_sfm_extent_rejected(sfm_error(6, SFM_OUTER, "#f { margin-top:-3px; }"));
}

/// R11: a right F's outer width -1 is rejected.
#[test]
fn css_signed_float_margins_right_float_negative_outer_width_is_typed() {
    assert_sfm_extent_rejected(sfm_error(
        6,
        SFM_OUTER,
        "#f { float:right; margin-right:-11px; }",
    ));
}

/// R11b: the exact extent -2 MAX + 10 is rejected with the typed reason, never `Unrepresentable`.
#[test]
fn css_signed_float_margins_extreme_negative_outer_width_is_typed_not_unrepresentable() {
    assert_sfm_extent_rejected(sfm_error(
        6,
        SFM_OUTER,
        "#f { margin-left:-340282346638528859811704183484516925440px; margin-right:-340282346638528859811704183484516925440px; }",
    ));
}

/// R12: a later tail's `NegativeMargin` (source 5) precedes F's extent error.
#[test]
fn css_signed_float_margins_later_tail_negative_margin_comes_first() {
    let error = sfm_error(
        6,
        SFM_OUTER,
        "#f { margin-left:-11px; } #b3 { margin-top:-1px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 5,
                    reason: OwnerProfileReason::NegativeMargin
                }
            ))
        ),
        "{error:?}"
    );
}

/// R13: F's extent error precedes B1's contact.
#[test]
fn css_signed_float_margins_outer_extent_precedes_b1_contact() {
    assert_sfm_extent_rejected(sfm_error(
        6,
        SFM_OUTER,
        "#f { margin-left:-11px; } #b1 { overflow:hidden; margin-bottom:0px; }",
    ));
}

/// R16: the margin bottom 16 equals B2's hypothetical top: equality stays typed.
#[test]
fn css_signed_float_margins_bottom_edge_on_the_tail_top_is_equality() {
    let error = sfm_error(6, SFM_OUTER, "#f { margin-bottom:-2px; }");
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality
            })
        ),
        "{error:?}"
    );
}

/// R18: F's margin bottom 36 sets P's Auto height; its border box ends at 40.
#[test]
fn css_signed_float_margins_n5_negative_bottom_sets_the_parent_auto_height() {
    assert_eq!(
        sfm_layout(
            5,
            SFM_OUTER,
            "#b2 { clear:none; } #f { height:24px; margin-bottom:-4px; }"
        ),
        sfm_geometry(
            [0.0, 10.0, 16.0, 36.0],
            [0.0, 10.0, 16.0, 40.0],
            [0.0, 0.0, 0.0, -4.0],
            [11.0, 12.0],
            &[(16.0, 17.0, None)],
            [0.0, 100.0, 11.0, 36.0, 25.0]
        )
    );
}

/// R18a: under P `auto` F's border bottom 40 lies below P's content bottom 36.
#[test]
fn css_signed_float_margins_n5_parent_auto_rejects_the_border_box_below_p() {
    assert_sfm_auto_overflow(
        5,
        SFM_OUTER,
        "#b2 { clear:none; } #f { height:24px; margin-bottom:-4px; }",
        "#p { overflow:auto; } #b2 { clear:none; } #f { height:24px; margin-bottom:-4px; }",
    );
}

/// R19: F's border box 10..12 reaches into B1 (11..12), but its margin box 16..18 lies strictly
/// below.
#[test]
fn css_signed_float_margins_new_bfc_b1_is_judged_on_the_margin_box() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#b1 { overflow:hidden; } #f { margin-top:-6px; margin-bottom:6px; }"
        ),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 10.0, 10.0, 12.0],
            [0.0, 0.0, -6.0, 6.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R20: F's margin top 12 touches B1's border bottom 12: contact stays typed.
#[test]
fn css_signed_float_margins_new_bfc_b1_contact_with_a_signed_float_top_is_typed() {
    let error = sfm_error(
        6,
        SFM_OUTER,
        "#b1 { overflow:hidden; margin-bottom:0px; } #f { margin-top:-1px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 2,
                error: ClearanceOwnerError::NewBfcFloatContact
            })
        ),
        "{error:?}"
    );
}

/// R21: under P `auto` F's border left -1 lies outside P.
#[test]
fn css_signed_float_margins_parent_auto_rejects_a_border_box_left_of_p() {
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        "#f { margin-left:-1px; }",
        "#p { overflow:auto; } #f { margin-left:-1px; }",
    );
}

/// R22: under P `auto` a right F's border right 101 lies outside P.
#[test]
fn css_signed_float_margins_parent_auto_rejects_a_right_float_border_box_right_of_p() {
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        "#f { float:right; margin-right:-1px; }",
        "#p { overflow:auto; } #f { float:right; margin-right:-1px; }",
    );
}

/// R23: under P `auto` F's border top 10 lies above P's content top 11.
#[test]
fn css_signed_float_margins_parent_auto_rejects_a_border_box_above_p() {
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        "#f { margin-top:-6px; margin-bottom:6px; }",
        "#p { overflow:auto; } #f { margin-top:-6px; margin-bottom:6px; }",
    );
}

/// R23h: the `hidden` control of R23 lays out: border box 10..12, margin box 16..18.
#[test]
fn css_signed_float_margins_border_box_above_the_margin_box_lays_out() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-top:-6px; margin-bottom:6px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 10.0, 10.0, 12.0],
            [0.0, 0.0, -6.0, 6.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R24: under P `auto` the content band 3..13 and the margin box 0..10 both lie inside P.
#[test]
fn css_signed_float_margins_parent_auto_admits_a_contained_border_box() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#p { overflow:auto; } #f { margin-left:3px; margin-right:-3px; }"
        ),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [3.0, 13.0, 16.0, 18.0],
            [3.0, -3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R24h: the `hidden` twin of R24 has the same geometry.
#[test]
fn css_signed_float_margins_left_margin_beside_a_negative_right_keeps_the_content_inside() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER, "#f { margin-left:3px; margin-right:-3px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [3.0, 13.0, 16.0, 18.0],
            [3.0, -3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R26: under P `rtl` and `auto` F's border left -1 lies outside P.
#[test]
fn css_signed_float_margins_rtl_parent_auto_rejects_a_negative_left_margin() {
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        "#p { direction:rtl; } #f { margin-left:-1px; }",
        "#p { overflow:auto; direction:rtl; } #f { margin-left:-1px; }",
    );
}

/// R26h: P `rtl` places a left F by its side: margin box 0..9, content band -1..9.
#[test]
fn css_signed_float_margins_rtl_places_the_float_by_its_side() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#p { direction:rtl; } #f { margin-left:-1px; }"
        ),
        sfm_geometry(
            [0.0, 9.0, 16.0, 18.0],
            [-1.0, 9.0, 16.0, 18.0],
            [-1.0, 0.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R26r: under P `rtl` and `auto` a right F's border right 103 lies outside P.
#[test]
fn css_signed_float_margins_rtl_parent_auto_rejects_a_right_float_negative_right_margin() {
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        "#p { direction:rtl; } #f { float:right; margin-left:3px; margin-right:-3px; }",
        "#p { overflow:auto; direction:rtl; } #f { float:right; margin-left:3px; margin-right:-3px; }",
    );
}

/// R26rh: P `rtl` places a right F by its side: margin box 90..100, content band 93..103.
#[test]
fn css_signed_float_margins_rtl_right_float_signed_margins_lay_out() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#p { direction:rtl; } #f { float:right; margin-left:3px; margin-right:-3px; }"
        ),
        sfm_geometry(
            [90.0, 100.0, 16.0, 18.0],
            [93.0, 103.0, 16.0, 18.0],
            [3.0, -3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R27: the near sum -MAX - 2^112 of a left F leaves the finite range: `float_content_left`.
#[test]
fn css_signed_float_margins_near_left_edge_overflow_is_typed() {
    let error = sfm_error(
        6,
        (-f32::MAX, 100.0, 10.0),
        "#f { margin-left:-5192296858534827628530496329220096px; margin-right:5192296858534827628530496329220096px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "float_content_left"
                }
            })
        ),
        "{error:?}"
    );
}

/// R27r: the near sum MAX + 2^112 of a right F leaves the finite range: `float_content_right`.
#[test]
fn css_signed_float_margins_near_right_edge_overflow_is_typed() {
    let error = sfm_error(
        6,
        (f32::MAX, 100.0, 10.0),
        "#f { float:right; margin-right:-5192296858534827628530496329220096px; margin-left:5192296858534827628530496329220096px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "float_content_right"
                }
            })
        ),
        "{error:?}"
    );
}

/// R28: F's margin top MAX plus 2^112 leaves the finite range: `float_content_top`.
#[test]
fn css_signed_float_margins_near_top_edge_overflow_below_is_typed() {
    let error = sfm_error(
        6,
        SFM_OUTER,
        "#b1 { margin-bottom:340282346638528859811704183484516925440px; } #f { margin-top:5192296858534827628530496329220096px; margin-bottom:-5192296858534827628530496329220096px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "float_content_top"
                }
            })
        ),
        "{error:?}"
    );
}

/// R29: F's margin top -MAX minus 2^112 leaves the finite range: `float_content_top`.
#[test]
fn css_signed_float_margins_near_top_edge_overflow_above_is_typed() {
    let error = sfm_error(
        6,
        (0.0, 100.0, -f32::MAX),
        "#f { margin-top:-5192296858534827628530496329220096px; margin-bottom:5192296858534827628530496329220096px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "float_content_top"
                }
            })
        ),
        "{error:?}"
    );
}

/// R30: F's margin left MAX plus 2^112 beside margin right -2^112 leaves the finite range.
#[test]
fn css_signed_float_margins_near_left_edge_overflow_beside_a_negative_right_is_typed() {
    let error = sfm_error(
        6,
        (f32::MAX, 100.0, 10.0),
        "#f { width:0px; margin-left:5192296858534827628530496329220096px; margin-right:-5192296858534827628530496329220096px; }",
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 3,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "float_content_left"
                }
            })
        ),
        "{error:?}"
    );
}

/// R31: at N5 the left shift moves only the content band.
#[test]
fn css_signed_float_margins_n5_left_shift_publishes_the_content_band() {
    assert_eq!(
        sfm_layout(5, SFM_OUTER, "#f { margin-left:-3px; margin-right:3px; }"),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [-3.0, 7.0, 16.0, 18.0],
            [-3.0, 3.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0))],
            [0.0, 100.0, 11.0, 19.0, 8.0]
        )
    );
}

/// R32: at N7 a right F's up shift moves only the content span.
#[test]
fn css_signed_float_margins_n7_right_float_up_shift_publishes_the_content_span() {
    assert_eq!(
        sfm_layout(
            7,
            SFM_OUTER,
            "#f { float:right; margin-top:-3px; margin-bottom:3px; }"
        ),
        sfm_geometry(
            [90.0, 100.0, 16.0, 18.0],
            [90.0, 100.0, 13.0, 15.0],
            [0.0, 0.0, -3.0, 3.0],
            [11.0, 12.0],
            &[
                (18.0, 19.0, Some(-1.0)),
                (22.0, 23.0, None),
                (27.0, 29.0, None)
            ],
            [0.0, 100.0, 11.0, 31.0, 20.0]
        )
    );
}

/// R33: content right RN(2^24 + 9) = 2^24 + 8, a tie to even, lies on P's edge.
#[test]
fn css_signed_float_margins_parent_auto_rounds_the_right_edge_onto_p() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER_2P24,
            "#p { overflow:auto; } #f { float:right; width:5px; margin-right:-1px; }"
        ),
        sfm_geometry(
            [16_777_220.0, 16_777_224.0, 16.0, 18.0],
            [16_777_220.0, 16_777_224.0, 16.0, 18.0],
            [0.0, -1.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [16_777_216.0, 16_777_224.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R34: content right 2^24 + 10 lies outside P's content box 2^24..2^24 + 8.
#[test]
fn css_signed_float_margins_parent_auto_rejects_the_rounded_right_edge_outside_p() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER_2P24,
            "#f { float:right; width:5px; margin-right:-2px; }"
        ),
        sfm_geometry(
            [16_777_220.0, 16_777_224.0, 16.0, 18.0],
            [16_777_220.0, 16_777_226.0, 16.0, 18.0],
            [0.0, -2.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [16_777_216.0, 16_777_224.0, 11.0, 27.0, 16.0]
        )
    );
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER_2P24,
        "#f { float:right; width:5px; margin-right:-2px; }",
        "#p { overflow:auto; } #f { float:right; width:5px; margin-right:-2px; }",
    );
}

/// R35: content left RN(2^24 - 0.5) = 2^24, a tie to even, lies on P's edge.
#[test]
fn css_signed_float_margins_parent_auto_rounds_the_left_edge_onto_p() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER_2P24,
            "#p { overflow:auto; } #f { width:5px; margin-left:-0.5px; }"
        ),
        sfm_geometry(
            [16_777_216.0, 16_777_220.0, 16.0, 18.0],
            [16_777_216.0, 16_777_220.0, 16.0, 18.0],
            [-0.5, 0.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [16_777_216.0, 16_777_224.0, 11.0, 27.0, 16.0]
        )
    );
}

/// R36: content left 2^24 - 1 lies outside P's content box.
#[test]
fn css_signed_float_margins_parent_auto_rejects_the_left_edge_outside_p() {
    assert_eq!(
        sfm_layout(6, SFM_OUTER_2P24, "#f { width:5px; margin-left:-1px; }"),
        sfm_geometry(
            [16_777_216.0, 16_777_220.0, 16.0, 18.0],
            [16_777_215.0, 16_777_220.0, 16.0, 18.0],
            [-1.0, 0.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [16_777_216.0, 16_777_224.0, 11.0, 27.0, 16.0]
        )
    );
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER_2P24,
        "#f { width:5px; margin-left:-1px; }",
        "#p { overflow:auto; } #f { width:5px; margin-left:-1px; }",
    );
}

/// R37: content top RN(16 - 2^30) = -2^30 while the placed margin top 16 stays strictly below B1.
#[test]
fn css_signed_float_margins_new_bfc_contact_reads_the_placed_margin_top() {
    assert_eq!(
        sfm_layout(
            6,
            SFM_OUTER,
            "#b1 { overflow:hidden; } #f { height:1073741824px; margin-top:-1073741824px; margin-bottom:2px; }"
        ),
        sfm_geometry(
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 10.0, f32::from_bits(0xce80_0000), 0.0],
            [0.0, 0.0, f32::from_bits(0xce80_0000), 2.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
}

// Signed float margins Task2: the shift matrix and POA-C1R3-F1.

/// The four shift forms: the declarations, F's used margins (left, right, top, bottom) and the move
/// of F's content box (left and right, or top and bottom) relative to its margin box.
const SFM_SHIFTS: [(&str, &str, [f32; 4], [f32; 4]); 4] = [
    (
        "shift_left",
        "margin-left:-3px; margin-right:3px;",
        [-3.0, 3.0, 0.0, 0.0],
        [-3.0, -3.0, 0.0, 0.0],
    ),
    (
        "shift_right",
        "margin-right:-3px; margin-left:3px;",
        [3.0, -3.0, 0.0, 0.0],
        [3.0, 3.0, 0.0, 0.0],
    ),
    (
        "shift_up",
        "margin-top:-3px; margin-bottom:3px;",
        [0.0, 0.0, -3.0, 3.0],
        [0.0, 0.0, -3.0, -3.0],
    ),
    (
        "shift_down",
        "margin-bottom:-3px; margin-top:3px;",
        [0.0, 0.0, 3.0, -3.0],
        [0.0, 0.0, 3.0, 3.0],
    ),
];

/// Side x the four shift forms x P `hidden`/`auto` at `nodes`: 16 rows. Each shift keeps F's outer
/// extents, so under `hidden` the layout equals the zero-margin control except F's content box and
/// used margins; under `auto` it equals that layout, or gives `ParentAutoOverflow` at F exactly
/// when F's border box leaves P: a left F shifted left, a right F shifted right, or F shifted down
/// at N5.
fn assert_signed_float_margin_matrix(nodes: usize) -> usize {
    let mut rows = 0;
    for side in ["left", "right"] {
        let control = sfm_layout(nodes, SFM_OUTER, &format!("#f {{ float:{side}; }}"));
        for (form, margins, used, shift) in SFM_SHIFTS {
            let mut content = [0.0; 4];
            for (edge, value) in content.iter_mut().enumerate() {
                *value = f32::from_bits(control.float_content[edge]) + shift[edge];
            }
            let expected = SfmGeometry {
                float_content: content.map(f32::to_bits),
                float_used_margins: used.map(f32::to_bits),
                tails: control.tails.clone(),
                ..control
            };
            let hidden = format!("#f {{ float:{side}; {margins} }}");
            assert_eq!(
                sfm_layout(nodes, SFM_OUTER, &hidden),
                expected,
                "N{nodes} {hidden}"
            );
            let auto = format!("{hidden} #p {{ overflow:auto; }}");
            if (side, form) == ("left", "shift_left")
                || (side, form) == ("right", "shift_right")
                || (nodes, form) == (5, "shift_down")
            {
                assert_parent_auto_overflow(sfm_error(nodes, SFM_OUTER, &auto), 3);
            } else {
                assert_eq!(
                    sfm_layout(nodes, SFM_OUTER, &auto),
                    expected,
                    "N{nodes} {auto}"
                );
            }
            rows += 2;
        }
    }
    assert_eq!(rows, 16, "N{nodes}");
    rows
}

#[test]
fn signed_float_margin_matrix_matches_the_zero_margin_layouts() {
    let rows: usize = [5, 6, 7]
        .into_iter()
        .map(assert_signed_float_margin_matrix)
        .sum();
    assert_eq!(rows, 48);
}

/// POA-C1R3-F1 (R25): P `rtl` with F's margin right 95. F's border box 0..10 lies inside P, but
/// its margin box 0..105 does not: the `hidden` control lays out and P `auto` gives the containment
/// error.
#[test]
fn css_signed_float_margins_rtl_float_margin_right_is_typed() {
    let control = "#p { direction:rtl; } #f { margin-right:95px; }";
    assert_eq!(
        sfm_layout(6, SFM_OUTER, control),
        sfm_geometry(
            [0.0, 105.0, 16.0, 18.0],
            [0.0, 10.0, 16.0, 18.0],
            [0.0, 95.0, 0.0, 0.0],
            [11.0, 12.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            [0.0, 100.0, 11.0, 27.0, 16.0]
        )
    );
    assert_sfm_auto_overflow(
        6,
        SFM_OUTER,
        control,
        "#p { overflow:auto; direction:rtl; } #f { margin-right:95px; }",
    );
}
