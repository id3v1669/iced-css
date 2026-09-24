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
            54 + 32 * 6 + 43 * (6 - 2) + 66 * (6 - 3)
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
        let work = 616;
        for short in [1, 615] {
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
            475,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            757,
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
            assert_eq!(work, 54 + 32 * source_count + 43 * children + 66 * normals);
        });
    }
}

#[test]
fn fitting_and_oversized_widths_have_identical_resource_receipts() {
    for (source_count, facade_work, core_work) in [(5, 475, 337), (6, 616, 419), (7, 757, 502)] {
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
            475,
            337,
            269,
        ),
        (
            6,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b3 { margin-left:auto; }",
            616,
            419,
            351,
        ),
        (
            7,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b4 { height:2px; margin-top:1px; margin-bottom:2px; margin-left:auto; clear:none; }",
            757,
            502,
            434,
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
            "#f { width:101px; margin-top:1px; }",
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
        [475, 616, 757][case.nodes - 5],
        "{id}"
    );
    assert_eq!(
        layout.receipt().core.work,
        [337, 419, 502][case.nodes - 5],
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
    for (source_count, work, selector) in [(5, 475, "#b2"), (6, 616, "#b3"), (7, 757, "#b4")] {
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
        (5, 475, 451, 337, 269),
        (6, 616, 588, 419, 351),
        (7, 757, 725, 502, 434),
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
            "#f { margin-top:auto; margin-bottom:-1px; }",
            profile(3, OwnerProfileReason::NegativeMargin),
        ),
        (
            "#f { margin-top:auto; margin-bottom:1px; }",
            profile(3, OwnerProfileReason::FloatMargin),
        ),
        (
            "#f { margin-top:1px; margin-bottom:auto; }",
            profile(3, OwnerProfileReason::FloatMargin),
        ),
        (
            "#f { margin-top:auto; margin-left:-1px; }",
            profile(3, OwnerProfileReason::NegativeMargin),
        ),
        (
            "#f { margin-bottom:auto; margin-right:1px; margin-top:2px; }",
            profile(3, OwnerProfileReason::FloatMargin),
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
            "#b1 { margin-top:auto; clear:left; }",
            profile(2, OwnerProfileReason::ClearOnBeforeOrFloat),
        ),
        (
            "#f { margin-bottom:auto; clear:both; }",
            profile(3, OwnerProfileReason::ClearOnBeforeOrFloat),
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
        (5, 475, 451, 337, 269),
        (6, 616, 588, 419, 351),
        (7, 757, 725, 502, 434),
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
