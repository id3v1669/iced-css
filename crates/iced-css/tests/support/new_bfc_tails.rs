//! TNB: computed `overflow:hidden`/`auto` on Sequence tails (new-BFC tails).
//!
//! Every literal interval below is derived by hand in the TNB test notes from the
//! cited CSS; the twin comparisons are additional, never the only oracle.

use super::*;
use iced_css_core::formatting::bfc_height::{SignedNormalMarginEdge, SignedNormalMarginError};

/// (nodes, facade exact work, core exact work) from the TNB counted action table.
const TNB_WORK: [(usize, usize, usize); 3] = [(5, 1294, 975), (6, 1726, 1167), (7, 2158, 1360)];

/// Copied observations of one successful layout.
#[derive(Debug)]
struct TnbView {
    debug: String,
    float: (f32, f32),
    borders: Vec<(f32, f32)>,
    contents: Vec<(f32, f32)>,
    work: usize,
    core_work: usize,
}

/// Fact rows of one capture plus its layout view or typed error.
struct TnbRun {
    rows: Vec<ClearanceBoxFact>,
    result: Result<TnbView, SiblingLayoutError>,
}

fn tnb_limits(work: usize, core_work: usize) -> SiblingLayoutLimits {
    let mut limits = layout_limits();
    limits.work = nz(work);
    limits.core.work = nz(core_work);
    limits
}

fn tnb_run_with(
    nodes: usize,
    css: &str,
    content_left: f32,
    content_top: f32,
    limits: SiblingLayoutLimits,
) -> TnbRun {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let rows = (0..nodes)
            .map(|source| *facts.row(source).unwrap())
            .collect();
        let result = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            content_left,
            100.0,
            content_top,
            limits,
        )
        .map(|layout| {
            let placed = layout.sequence().placed_float();
            TnbView {
                debug: format!("{layout:?}"),
                float: (placed.top, placed.bottom),
                borders: layout
                    .children()
                    .iter()
                    .map(|child| (child.border().top, child.border().bottom))
                    .collect(),
                contents: layout
                    .children()
                    .iter()
                    .map(|child| (child.content().top, child.content().bottom))
                    .collect(),
                work: layout.receipt().work,
                core_work: layout.receipt().core.work,
            }
        });
        out = Some(TnbRun { rows, result });
    });
    out.unwrap()
}

fn tnb_run(nodes: usize, css: &str) -> TnbRun {
    tnb_run_with(nodes, css, 0.0, 10.0, layout_limits())
}

/// The success-asserting helper (plan rule S1): panics with the typed error otherwise.
fn tnb_layout(nodes: usize, css: &str) -> TnbView {
    match tnb_run(nodes, css).result {
        Ok(view) => view,
        Err(error) => panic!("{css}: expected a layout, got {error:?}"),
    }
}

fn tnb_error(nodes: usize, css: &str) -> SiblingLayoutError {
    match tnb_run(nodes, css).result {
        Ok(view) => panic!("{css}: expected an error, got {view:?}"),
        Err(error) => error,
    }
}

fn assert_contact(error: &SiblingLayoutError, source: usize, context: &str) {
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: actual,
                error: ClearanceOwnerError::NewBfcFloatContact,
            }) if *actual == source
        ),
        "{context}: expected NewBfcFloatContact at source {source}, got {error:?}"
    );
}

fn inherited(value: BoxOverflow, nodes: usize) -> KeywordBoxFact<BoxOverflow> {
    KeywordBoxFact {
        value,
        origin: BoxFactOrigin::Inherited {
            original_parent: nodes - 2,
        },
    }
}

/// The new-BFC run equals its `visible` twin except for the overflow fact at `source`.
fn assert_fact_twins(
    run: &TnbRun,
    twin: &TnbRun,
    source: usize,
    expected: KeywordBoxFact<BoxOverflow>,
) {
    assert_eq!(run.rows.len(), twin.rows.len());
    for (index, (row, control)) in run.rows.iter().zip(&twin.rows).enumerate() {
        if index == source {
            assert_eq!(row.overflow, expected, "source {source}");
            assert_eq!(control.overflow, VISIBLE, "source {source}");
            assert_eq!(
                ClearanceBoxFact {
                    overflow: control.overflow,
                    ..*row
                },
                *control,
                "source {source}"
            );
        } else {
            assert_eq!(row, control, "source {index}");
        }
    }
}

// RED (plan Task 1, step 1.2).

/// L1: a tail whose `clear` selects F lies at F's margin bottom (contact) and is admitted.
#[test]
fn tnb_cleared_tail_is_admitted() {
    let css = format!(
        "{} #f {{ height:10px; }} #b2 {{ clear:none; }} #b3 {{ overflow:hidden; clear:both; }}",
        baseline_css()
    );
    let view = tnb_layout(6, &css);
    assert_eq!(view.float, (16.0, 26.0));
    assert_eq!(view.borders[3], (26.0, 27.0));
}

/// L7: an uncleared new-BFC tail that strictly intersects F is rejected at its own source.
#[test]
fn tnb_intersecting_tail_reports_its_source() {
    let css = format!(
        "{} #f {{ height:10px; }} #b2 {{ clear:none; }} #b3 {{ overflow:hidden; clear:none; }}",
        baseline_css()
    );
    assert_contact(&tnb_error(6, &css), 5, &css);
}

// IR-2 / DR-8: clear selection rows L1-L8 (N6; F margin box [16,26]; B2 [16,17]).

#[test]
fn tnb_clear_selection_rows() {
    let mut rows = 0;
    for (fragment, b3, admitted) in [
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:both; }",
            (26.0, 27.0),
            true,
        ),
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:left; }",
            (26.0, 27.0),
            true,
        ),
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:right; }",
            (20.0, 21.0),
            false,
        ),
        (
            "#f { height:10px; float:right; } #b2 { clear:none; } #b3 { overflow:hidden; clear:right; }",
            (26.0, 27.0),
            true,
        ),
        (
            "#f { height:10px; float:right; } #b2 { clear:none; } #b3 { overflow:hidden; clear:left; }",
            (20.0, 21.0),
            false,
        ),
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:right; margin-top:10px; }",
            (27.0, 28.0),
            true,
        ),
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:none; }",
            (20.0, 21.0),
            false,
        ),
        (
            "#f { height:10px; } #b2 { clear:none; } #b3 { overflow:hidden; clear:none; margin-top:10px; }",
            (27.0, 28.0),
            true,
        ),
    ] {
        let css = format!("{} {fragment}", baseline_css());
        let twin_css = css.replace("overflow:hidden; ", "");
        let twin = tnb_run(6, &twin_css);
        let twin_view = twin
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("{twin_css}: {error:?}"));
        assert_eq!(twin_view.float, (16.0, 26.0), "{fragment}");
        assert_eq!(twin_view.borders[2], (16.0, 17.0), "{fragment}");
        assert_eq!(twin_view.borders[3], b3, "{fragment}");
        let run = tnb_run(6, &css);
        assert_fact_twins(&run, &twin, 5, HIDDEN);
        match (&run.result, admitted) {
            (Ok(view), true) => {
                assert_eq!(view.debug, twin_view.debug, "{fragment}");
                assert_eq!(view.borders[3], b3, "{fragment}");
            }
            (Err(error), false) => assert_contact(error, 5, fragment),
            (other, _) => panic!(
                "{fragment}: unexpected {:?}",
                other.as_ref().map(|view| &view.borders)
            ),
        }
        rows += 1;
    }
    assert_eq!(rows, 8);
}

// IR-3 / DR-2: error-order pins.

#[test]
fn tnb_error_order_profile_rows() {
    use CapturedClearanceUnsupported as U;
    // E1: the same tail's height kind follows its admitted overflow.
    assert_captured(
        tnb_error(
            6,
            &format!("{} #b3 {{ overflow:hidden; height:auto; }}", baseline_css()),
        ),
        CapturedClearanceError::Unsupported {
            projected: 5,
            property: "height",
            reason: U::HeightKind,
        },
    );
    // E3: a later child's overflow and an N7 tail without a height become reachable.
    assert_captured(
        tnb_error(
            6,
            &format!(
                "{} #b2 {{ overflow:hidden; }} #b3 {{ overflow:scroll; }}",
                baseline_css()
            ),
        ),
        CapturedClearanceError::Unsupported {
            projected: 5,
            property: "overflow",
            reason: U::ChildOverflow,
        },
    );
    assert_captured(
        tnb_error(7, &format!("{} #b2 {{ overflow:auto; }}", baseline_css())),
        CapturedClearanceError::Unsupported {
            projected: 6,
            property: "height",
            reason: U::HeightKind,
        },
    );
    // E4: F width extraction follows the child loop (plan pin P2).
    assert_captured(
        tnb_error(
            6,
            &format!(
                "{} #f {{ width:auto; }} #b3 {{ overflow:hidden; }}",
                baseline_css()
            ),
        ),
        CapturedClearanceError::Unsupported {
            projected: 3,
            property: "width",
            reason: U::FloatWidth,
        },
    );
    // C1 (facade): core profile continuation after the tail's context.
    let error = tnb_error(
        6,
        &format!("{} #b3 {{ overflow:hidden; height:0; }}", baseline_css()),
    );
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 5,
                    reason: OwnerProfileReason::NonpositiveBlockHeight,
                }
            ))
        ),
        "{error:?}"
    );
}

#[test]
fn tnb_error_order_parent_stages_keep_their_errors() {
    // E5: the parent horizontal and origin witnesses (tests 3721-3765) with a new-BFC B3.
    let max = format!("{MAX_NUMBER}px");
    let horizontal = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} \
         #p {{ width:{max}; margin-left:{max}; margin-right:auto; }} \
         #b1 {{ margin-left:auto; }} #b3 {{ margin-top:-1px; overflow:hidden; }}",
        baseline_css()
    );
    let error = tnb_error(6, &horizontal);
    assert!(
        matches!(error, SiblingLayoutError::ParentHorizontal(_)),
        "{error:?}"
    );
    let twin = tnb_error(6, &horizontal.replace(" overflow:hidden;", ""));
    assert_eq!(format!("{error:?}"), format!("{twin:?}"));

    let origin = format!(
        "{} #f {{ margin-left:auto; margin-right:auto; }} \
         #p {{ margin-top:{max}; }} #b1 {{ margin-left:auto; }} \
         #b3 {{ margin-top:-1px; overflow:hidden; }}",
        baseline_css()
    );
    let run = tnb_run_with(6, &origin, 0.0, f32::MAX, layout_limits());
    let error = run.result.unwrap_err();
    assert!(
        matches!(error, SiblingLayoutError::ParentOrigin(_)),
        "{error:?}"
    );
    let twin = tnb_run_with(
        6,
        &origin.replace(" overflow:hidden;", ""),
        0.0,
        f32::MAX,
        layout_limits(),
    )
    .result
    .unwrap_err();
    assert_eq!(format!("{error:?}"), format!("{twin:?}"));
}

#[test]
fn tnb_error_order_placement_rows() {
    // C2: the earlier tail's guard precedes a later tail's kernel equality.
    let css = format!(
        "{} #f {{ height:10px; }} #b2 {{ overflow:hidden; clear:none; }} \
         #b3 {{ clear:both; margin-top:9px; }}",
        baseline_css()
    );
    assert_contact(&tnb_error(6, &css), 4, &css);
    let twin = tnb_error(6, &css.replace("overflow:hidden; ", ""));
    assert!(
        matches!(
            twin,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 5,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ),
        "{twin:?}"
    );
    // C3: the B1 guard precedes every tail guard.
    let css = format!(
        "{} #f {{ height:10px; }} #b1 {{ overflow:hidden; margin-bottom:-1px; }} \
         #b2 {{ overflow:hidden; clear:none; }}",
        baseline_css()
    );
    assert_contact(&tnb_error(6, &css), 2, &css);
}

#[test]
fn tnb_error_order_post_core_rows() {
    // E8 ChildHorizontal: the witness at tests 4331-4360 with a new-BFC B3.
    let max = format!("{MAX_NUMBER}px");
    let css = format!(
        "{} #f {{ width:0; }} #b3 {{ width:0; margin-left:{max}; margin-right:0; overflow:hidden; }}",
        baseline_css()
    );
    let error = tnb_run_with(6, &css, f32::MAX, 10.0, layout_limits())
        .result
        .unwrap_err();
    assert!(
        matches!(
            error,
            SiblingLayoutError::ChildHorizontal {
                source: 5,
                error: BlockHorizontalError::Unrepresentable {
                    stage: BlockCoordinateStage::ContentLeft,
                },
            }
        ),
        "{error:?}"
    );
    // E8 containment: P content box [11,16] excludes F's margin box [16,18].
    let css = format!(
        "{} #p {{ overflow:auto; height:5px; }} #b3 {{ overflow:hidden; }}",
        baseline_css()
    );
    let error = tnb_error(6, &css);
    assert!(
        matches!(error, SiblingLayoutError::ParentAutoOverflow { source: 3 }),
        "{error:?}"
    );
    // E8 signed height: the SNV API witness with a new-BFC B2 whose interval is empty.
    let max = MAX_NUMBER;
    let css = format!(
        "#o,#p,#b1,#f,#b2,#b3 {{display:block;margin:0;padding:0;clear:none;}} \
         #o {{width:100px;}} #p {{overflow:hidden;padding-top:1px;padding-bottom:1px;}} \
         #b1 {{height:1px;}} #f {{float:left;width:10px;height:0px;}} \
         #b2 {{height:1px;margin-top:-{max}px;margin-bottom:-{max}px;overflow:hidden;}} \
         #b3 {{height:1px;margin-top:{max}px;}}"
    );
    let error = tnb_run_with(6, &css, 0.0, -1.0, layout_limits())
        .result
        .unwrap_err();
    assert!(
        matches!(
            error,
            SiblingLayoutError::NormalMargin {
                source: 4,
                error: SignedNormalMarginError::Unrepresentable {
                    edge: SignedNormalMarginEdge::Bottom
                }
            }
        ),
        "{error:?}"
    );
}

// IR-4: twin matrix (plan "Twin matrix construction").

#[derive(Clone, Copy, Debug)]
enum TnbKind {
    Hidden,
    Auto,
    Inherit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TnbRelation {
    Contact,
    Below,
    Intersect,
    NonSelecting,
}

#[derive(Clone, Copy, Debug)]
struct TnbTwin {
    nodes: usize,
    side: &'static str,
    direction: &'static str,
    kind: TnbKind,
    relation: TnbRelation,
    explicit_parent: bool,
    parent_overflow: &'static str,
}

/// Returns `(new_bfc, visible)`; they differ only in the target tail's overflow.
fn tnb_twin_css(twin: TnbTwin) -> (String, String) {
    let height = if twin.explicit_parent {
        " height:40px;"
    } else {
        ""
    };
    let mut css = format!(
        "#o {{ width:100px; }} \
         #p {{ overflow:{}; padding-top:1px; padding-bottom:1px; direction:{};{height} }} \
         #b1 {{ height:1px; margin-bottom:4px; }} \
         #f {{ float:{}; width:10px; height:10px; }}",
        twin.parent_overflow, twin.direction, twin.side
    );
    let target = twin.nodes - 3;
    for tail in 2..target {
        css.push_str(&format!(
            " #b{tail} {{ height:1px; margin-top:3px; margin-bottom:3px; clear:none; }}"
        ));
    }
    let opposite = if twin.side == "left" { "right" } else { "left" };
    let relation = match twin.relation {
        TnbRelation::Contact => "clear:both; margin-top:3px;".to_owned(),
        TnbRelation::Below => "clear:none; margin-top:20px;".to_owned(),
        TnbRelation::Intersect => "clear:none; margin-top:3px;".to_owned(),
        TnbRelation::NonSelecting => format!("clear:{opposite}; margin-top:3px;"),
    };
    let overflow = match twin.kind {
        TnbKind::Hidden => "overflow:hidden;",
        TnbKind::Auto => "overflow:auto;",
        TnbKind::Inherit => "overflow:inherit;",
    };
    let visible = format!("{css} #b{target} {{ height:1px; margin-bottom:3px; {relation} }}");
    let new_bfc =
        format!("{css} #b{target} {{ height:1px; margin-bottom:3px; {relation} {overflow} }}");
    (new_bfc, visible)
}

fn tnb_twin_target(nodes: usize, relation: TnbRelation) -> (f32, f32) {
    match (relation, nodes) {
        (TnbRelation::Contact, _) => (26.0, 27.0),
        (TnbRelation::Below, 5) => (32.0, 33.0),
        (TnbRelation::Below, 6) => (37.0, 38.0),
        (TnbRelation::Below, _) => (41.0, 42.0),
        (_, 5) => (16.0, 17.0),
        (_, 6) => (20.0, 21.0),
        (_, _) => (24.0, 25.0),
    }
}

fn assert_tnb_twin(twin: TnbTwin) {
    let (new_bfc, visible) = tnb_twin_css(twin);
    let target = twin.nodes - 1;
    let interval = tnb_twin_target(twin.nodes, twin.relation);
    let control = tnb_run(twin.nodes, &visible);
    let control_view = control
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("{twin:?} visible: {error:?}"));
    assert_eq!(control_view.float, (16.0, 26.0), "{twin:?}");
    assert_eq!(control_view.borders[twin.nodes - 3], interval, "{twin:?}");
    let run = tnb_run(twin.nodes, &new_bfc);
    let parent = if twin.parent_overflow == "auto" {
        BoxOverflow::Auto
    } else {
        BoxOverflow::Hidden
    };
    let expected = match twin.kind {
        TnbKind::Hidden => HIDDEN,
        TnbKind::Auto => AUTO,
        TnbKind::Inherit => inherited(parent, twin.nodes),
    };
    assert_fact_twins(&run, &control, target, expected);
    match twin.relation {
        TnbRelation::Contact | TnbRelation::Below => {
            let view = run
                .result
                .as_ref()
                .unwrap_or_else(|error| panic!("{twin:?}: {error:?}"));
            assert_eq!(view.debug, control_view.debug, "{twin:?}");
            assert_eq!(view.float, (16.0, 26.0), "{twin:?}");
            assert_eq!(view.borders[twin.nodes - 3], interval, "{twin:?}");
            assert_eq!(
                (view.work, view.core_work),
                (control_view.work, control_view.core_work)
            );
        }
        TnbRelation::Intersect | TnbRelation::NonSelecting => match &run.result {
            Err(error) => assert_contact(error, target, &format!("{twin:?}")),
            Ok(view) => panic!("{twin:?}: admitted {:?}", view.borders),
        },
    }
}

#[test]
fn tnb_twin_matrix_partitions_on_the_strict_nonempty_intersection() {
    let mut rows = 0;
    // Block A: 3 N x 2 sides x 2 directions x 3 overflow sources x 4 relations.
    for nodes in [5, 6, 7] {
        for side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                for kind in [TnbKind::Hidden, TnbKind::Auto, TnbKind::Inherit] {
                    for relation in [
                        TnbRelation::Contact,
                        TnbRelation::Below,
                        TnbRelation::Intersect,
                        TnbRelation::NonSelecting,
                    ] {
                        assert_tnb_twin(TnbTwin {
                            nodes,
                            side,
                            direction,
                            kind,
                            relation,
                            explicit_parent: false,
                            parent_overflow: "hidden",
                        });
                        rows += 1;
                    }
                }
            }
        }
    }
    // Block B: P height x P overflow x 3 N x 2 sides x {contact, intersect}.
    for explicit_parent in [false, true] {
        for parent_overflow in ["hidden", "auto"] {
            for nodes in [5, 6, 7] {
                for side in ["left", "right"] {
                    for relation in [TnbRelation::Contact, TnbRelation::Intersect] {
                        assert_tnb_twin(TnbTwin {
                            nodes,
                            side,
                            direction: "ltr",
                            kind: TnbKind::Hidden,
                            relation,
                            explicit_parent,
                            parent_overflow,
                        });
                        rows += 1;
                    }
                }
            }
        }
    }
    assert_eq!(rows, 192);
}

// Inherited provenance and P `auto` containment (plan pins P9 and P14).

#[test]
fn css_tail_auto_or_inherited_overflow_is_admitted() {
    for (rule, expected) in [
        ("#b3 { overflow:auto; }", AUTO),
        (
            "#b3 { overflow:inherit; }",
            inherited(BoxOverflow::Hidden, 6),
        ),
        (
            "#p { overflow:auto; } #b3 { overflow:inherit; }",
            inherited(BoxOverflow::Auto, 6),
        ),
    ] {
        let css = format!("{} {rule}", baseline_css());
        let run = tnb_run(6, &css);
        assert_eq!(run.rows[5].overflow, expected, "{rule}");
        let view = run
            .result
            .unwrap_or_else(|error| panic!("{rule}: {error:?}"));
        assert_eq!(view.float, (16.0, 18.0), "{rule}");
        assert_eq!(view.borders[3], (22.0, 23.0), "{rule}");
    }
}

#[test]
fn tnb_parent_auto_contains_new_bfc_tails() {
    for (nodes, extra, last) in [
        (5, leading_clear_source_rule(5), (18.0, 19.0)),
        (6, leading_clear_source_rule(6), (22.0, 23.0)),
        (7, leading_clear_source_rule(7), (27.0, 29.0)),
    ] {
        let css = format!(
            "{} {extra} #p {{ overflow:auto; }} #b2, #b3, #b4 {{ overflow:auto; }}",
            baseline_css()
        );
        let view = tnb_layout(nodes, &css);
        assert_eq!(view.float, (16.0, 18.0), "N{nodes}");
        assert_eq!(view.borders[nodes - 3], last, "N{nodes}");
    }
}

// Work and storage: exact and one-short thresholds, with and without new-BFC tails (M15).

#[test]
fn tnb_exact_and_one_short_work_at_every_node_count() {
    for (nodes, facade, core) in TNB_WORK {
        assert_eq!(
            facade,
            208 + 32 * nodes + 126 * (nodes - 2) + 274 * (nodes - 3)
        );
        assert_eq!(
            core,
            560 + 8 * nodes
                + nodes * (nodes - 1) / 2
                + 54 * (nodes - 4)
                + 61 * (nodes - 2)
                + 64 * (nodes - 3)
        );
        for tails in ["", "#b2, #b3, #b4 { overflow:hidden; }"] {
            let css = format!(
                "{} {} {tails}",
                baseline_css(),
                leading_clear_source_rule(nodes)
            );
            let exact = tnb_run_with(nodes, &css, 0.0, 10.0, tnb_limits(facade, core));
            let view = exact
                .result
                .unwrap_or_else(|error| panic!("N{nodes} {tails}: {error:?}"));
            assert_eq!(
                (view.work, view.core_work),
                (facade, core),
                "N{nodes} {tails}"
            );
            let facade_short =
                tnb_run_with(nodes, &css, 0.0, 10.0, tnb_limits(facade - 1, core)).result;
            assert!(
                matches!(facade_short, Err(SiblingLayoutError::Work)),
                "N{nodes} {tails}"
            );
            let core_short =
                tnb_run_with(nodes, &css, 0.0, 10.0, tnb_limits(facade, core - 1)).result;
            assert!(
                matches!(
                    core_short,
                    Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
                ),
                "N{nodes} {tails}"
            );
        }
    }
}

// Admitted outcomes of the migrated pins (plan "Pin migration map", rule S1).

#[test]
fn tnb_pins_admit_tail_overflow_with_literal_intervals() {
    let mut rows = 0;
    for (pin, nodes, rule, index, interval) in [
        (
            "P1",
            6,
            "#f { width:140px; margin-left:auto; margin-right:auto; } #b1 { margin-left:auto; } \
             #b3 { margin-right:auto; overflow:hidden; }",
            3,
            (22.0, 23.0),
        ),
        (
            "P3",
            6,
            "#f { margin-left:auto; margin-right:auto; } #b3 { overflow:hidden; }",
            3,
            (22.0, 23.0),
        ),
        (
            "P5",
            6,
            "#b2 { margin-bottom:auto; overflow:hidden; }",
            2,
            (18.0, 19.0),
        ),
        (
            "P6",
            6,
            "#f { overflow:hidden; } #b3 { overflow:hidden; }",
            3,
            (22.0, 23.0),
        ),
        (
            "P8",
            6,
            "#f { overflow:auto; } #b3 { overflow:auto; }",
            3,
            (22.0, 23.0),
        ),
        (
            "P11",
            6,
            "#b1 { overflow:hidden; } #b3 { overflow:hidden; }",
            3,
            (22.0, 23.0),
        ),
        ("P12 N5", 5, "#b2 { overflow:hidden; }", 2, (18.0, 19.0)),
        ("P12 N6", 6, "#b2 { overflow:auto; }", 2, (18.0, 19.0)),
        (
            "P12 N7",
            7,
            "#b4 { height:2px; margin-top:1px; margin-bottom:2px; clear:none; } #b2 { overflow:auto; }",
            2,
            (18.0, 19.0),
        ),
    ] {
        let css = format!("{} {rule}", baseline_css());
        let view = tnb_layout(nodes, &css);
        assert_eq!(view.float, (16.0, 18.0), "{pin}");
        assert_eq!(view.borders[index], interval, "{pin}");
        if pin == "P12 N7" {
            assert_eq!(view.borders[4], (27.0, 29.0), "{pin}");
        }
        rows += 1;
    }
    assert_eq!(rows, 9);
    // P12: the bare N7 baseline has no B4 height, so the admitted tail exposes HeightKind.
    assert_captured(
        tnb_error(7, &format!("{} #b2 {{ overflow:auto; }}", baseline_css())),
        CapturedClearanceError::Unsupported {
            projected: 6,
            property: "height",
            reason: CapturedClearanceUnsupported::HeightKind,
        },
    );
}

/// P4: the vertical-Auto priority fixture (tests 6299-6375) with a new-BFC last tail is
/// admitted at the TNB exact work; F is [12,14] because B1's bottom margin is Auto.
#[test]
fn tnb_pin_p4_vertical_auto_last_tail_is_admitted_at_exact_work() {
    let vertical = "#b1 { margin-bottom:auto; } #f { margin-top:auto; margin-bottom:auto; } \
                    #b2 { margin-top:auto; } #b3 { margin-bottom:auto; }";
    for ((nodes, facade, core), (selector, last)) in TNB_WORK.into_iter().zip([
        ("#b2", (14.0, 15.0)),
        ("#b3", (18.0, 19.0)),
        ("#b4", (19.0, 21.0)),
    ]) {
        let b4 = if nodes == 7 {
            "#b4 { height:2px; margin-top:auto; margin-bottom:2px; clear:none; }"
        } else {
            ""
        };
        let css = format!(
            "{} {b4} {vertical} {selector} {{ overflow:hidden; }}",
            baseline_css()
        );
        let run = tnb_run_with(nodes, &css, 0.0, 10.0, tnb_limits(facade, core));
        let view = run
            .result
            .unwrap_or_else(|error| panic!("N{nodes}: {error:?}"));
        assert_eq!(view.float, (12.0, 14.0), "N{nodes}");
        assert_eq!(view.borders[nodes - 3], last, "N{nodes}");
        assert_eq!(view.work, facade, "N{nodes}");
        let short = tnb_run_with(nodes, &css, 0.0, 10.0, tnb_limits(facade - 1, core)).result;
        assert!(matches!(short, Err(SiblingLayoutError::Work)), "N{nodes}");
    }
}

// Mutation witnesses (plan "Mutation table").

/// M6: a middle tail (B3 of N7) is guarded, not only the last tail.
#[test]
fn tnb_m6_middle_tail_is_guarded() {
    let css = format!(
        "{} #f {{ height:10px; }} #b2, #b3 {{ clear:none; }} #b3 {{ margin-top:3px; overflow:hidden; }} \
         #b4 {{ height:1px; margin-top:3px; margin-bottom:3px; clear:none; }}",
        baseline_css()
    );
    assert_contact(&tnb_error(7, &css), 5, &css);
    let twin = tnb_layout(7, &css.replace(" overflow:hidden;", ""));
    assert_eq!(twin.float, (16.0, 26.0));
    assert_eq!(twin.borders[2], (16.0, 17.0));
    assert_eq!(twin.borders[3], (20.0, 21.0));
    assert_eq!(twin.borders[4], (25.0, 26.0));
}

/// M9: the guard reads the tail's border box, not its content box.
#[test]
fn tnb_m9_guard_uses_the_tail_border_box() {
    let css = format!(
        "{} #f {{ height:10px; }} #b2 {{ clear:none; }} \
         #b3 {{ overflow:hidden; clear:none; padding-top:10px; }}",
        baseline_css()
    );
    assert_contact(&tnb_error(6, &css), 5, &css);
    let twin = tnb_layout(6, &css.replace("overflow:hidden; ", ""));
    assert_eq!(twin.float, (16.0, 26.0));
    assert_eq!(twin.borders[3], (20.0, 31.0));
    assert_eq!(twin.contents[3], (30.0, 31.0));
}

/// M11: the guard reads F's margin box, not its border box.
#[test]
fn tnb_m11_guard_uses_the_float_margin_box() {
    let css = format!(
        "{} #f {{ height:10px; margin-bottom:-5px; }} #b2 {{ clear:none; }} \
         #b3 {{ overflow:hidden; clear:none; margin-top:5px; }}",
        baseline_css()
    );
    let view = tnb_layout(6, &css);
    assert_eq!(view.float, (16.0, 21.0));
    assert_eq!(view.borders[1], (16.0, 26.0));
    assert_eq!(view.borders[3], (22.0, 23.0));
    let twin = tnb_layout(6, &css.replace("overflow:hidden; ", ""));
    assert_eq!(view.debug, twin.debug);
}

/// M13 (PR-5): a rounded-empty tail strictly straddled by F's margin box is admitted.
/// Near 2^24 the f32 spacing is 2, so 16777216 + 1 rounds back to 16777216.
#[test]
fn tnb_m13_rounded_empty_tail_inside_the_float_is_admitted() {
    let css = "#o { width:100px; } \
               #p { overflow:hidden; padding-top:1px; padding-bottom:1px; } \
               #b1 { height:1px; margin-top:16777200px; } \
               #f { float:left; width:10px; height:8px; } \
               #b2 { height:1px; margin-top:4px; clear:none; overflow:hidden; }";
    let view = tnb_layout(5, css);
    assert_eq!(view.borders[0], (16777211.0, 16777212.0));
    assert_eq!(view.float, (16777212.0, 16777220.0));
    assert_eq!(view.borders[2], (16777216.0, 16777216.0));
    let twin = tnb_layout(
        5,
        &css.replace("clear:none; overflow:hidden;", "clear:none;"),
    );
    assert_eq!(view.debug, twin.debug);
}

/// M14: an empty F margin box inside an uncleared tail's span is admitted.
#[test]
fn tnb_m14_empty_float_inside_a_tail_is_admitted() {
    let css = format!(
        "{} #f {{ height:0; }} #b2 {{ overflow:hidden; clear:none; margin-top:-1px; height:2px; }}",
        baseline_css()
    );
    let view = tnb_layout(6, &css);
    assert_eq!(view.float, (16.0, 16.0));
    assert_eq!(view.borders[2], (15.0, 17.0));
    assert_eq!(view.borders[3], (20.0, 21.0));
}

/// M16: the guard reads the tail's own published interval, never its predecessor's.
#[test]
fn tnb_m16_guard_reads_the_tail_own_interval() {
    let css = format!(
        "{} #f {{ height:10px; }} #b2 {{ clear:none; }} \
         #b3 {{ overflow:hidden; clear:none; margin-top:10px; }}",
        baseline_css()
    );
    let view = tnb_layout(6, &css);
    assert_eq!(view.float, (16.0, 26.0));
    assert_eq!(view.borders[2], (16.0, 17.0));
    assert_eq!(view.borders[3], (27.0, 28.0));
}
