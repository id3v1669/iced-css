//! PCT: percentages of the containing block's width in `width`, the four margins and
//! `left`/`right` on every Sequence row (CSS2.2 10.2, 8.3, 9.3.2, 9.4.3).
//!
//! The RED part of this module (the helpers, T1, T2 and the two guard rows) compiles
//! against the predecessor API at c64dfb5f: it reads only published geometry and
//! errors. The rows appended at FLIP also read the percentage facts. Every literal is
//! derived from `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with
//! padding-top and padding-bottom of 1) by the PCT plan's exact-rational model; each
//! admitted row also compares every other published number with a control and with its
//! absolute twin (the same CSS with each percentage replaced by the value it resolves
//! to), bit for bit.

use super::*;

/// The facade fixture: `baseline_css()`, the N7 leading rule, then the fragment.
fn pct_css(nodes: usize, fragment: &str) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} {fragment}", baseline_css())
}

fn pct_push(out: &mut Vec<(String, f32)>, name: &str, value: f32) {
    out.push((name.to_owned(), value));
}

fn pct_push_edges(out: &mut Vec<(String, f32)>, name: &str, edges: PrincipalEdges) {
    for (side, value) in [
        ("left", edges.left),
        ("right", edges.right),
        ("top", edges.top),
        ("bottom", edges.bottom),
    ] {
        pct_push(out, &format!("{name}.{side}"), value);
    }
}

/// Every published number of one layout by name (counts and flags as exact f32
/// integers). P: `p.*`; children `c0` (B1), `c1` (F), `c2` (B2), ... in source order;
/// `float` is `placed_float()`; `tailN.*` are the tail clearances; `*.offset.*` are the
/// used relative offsets. An absent `auto_extent` publishes `p.extent` 0 and its three
/// fields 0.
fn pct_view(layout: &CapturedSiblingLayout<'_>) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    let parent = layout.parent();
    let origin = parent.origin();
    let height = parent.height();
    let bottom = parent.bottom();
    let horizontal = parent.horizontal();
    let offset = parent.relative_offset();
    for (name, value) in [
        ("p.border_top", origin.border_top),
        ("p.content_top", origin.content_top),
        ("p.content_bottom", height.content_bottom),
        ("p.padding_bottom", bottom.padding_bottom),
        ("p.border_bottom", bottom.border_bottom),
        ("p.margin_bottom", bottom.margin_bottom),
        ("p.used_height", height.used_height),
        ("p.width", horizontal.used.width),
        ("p.margin_left", horizontal.used.margin_left),
        ("p.margin_right", horizontal.used.margin_right),
        ("p.passes", f32::from(horizontal.used.passes)),
        ("p.content_left", horizontal.content.left),
        ("p.content_right", horizontal.content.right),
        ("p.offset.left", offset.left),
        ("p.offset.right", offset.right),
        ("p.offset.top", offset.top),
        ("p.offset.bottom", offset.bottom),
    ] {
        pct_push(&mut out, name, value);
    }
    let (extent, normal, selected, extended) = match height.auto_extent {
        Some(extent) => (
            1.0,
            extent.normal_margin_bottom,
            extent.selected_bottom,
            f32::from(u8::from(extent.float_extended)),
        ),
        None => (0.0, 0.0, 0.0, 0.0),
    };
    pct_push(&mut out, "p.extent", extent);
    pct_push(&mut out, "p.extent.normal_margin_bottom", normal);
    pct_push(&mut out, "p.extent.selected_bottom", selected);
    pct_push(&mut out, "p.extent.float_extended", extended);
    for (index, child) in layout.children().iter().enumerate() {
        let name = format!("c{index}");
        pct_push(
            &mut out,
            &format!("{name}.source"),
            f32::from(u8::try_from(child.source_index()).unwrap()),
        );
        pct_push_edges(&mut out, &format!("{name}.border"), child.border());
        pct_push_edges(&mut out, &format!("{name}.padding"), child.padding());
        pct_push_edges(&mut out, &format!("{name}.content"), child.content());
        for (side, value) in [
            ("top", child.used_margin_top()),
            ("right", child.used_margin_right()),
            ("bottom", child.used_margin_bottom()),
            ("left", child.used_margin_left()),
        ] {
            pct_push(&mut out, &format!("{name}.margin.{side}"), value);
        }
        pct_push(&mut out, &format!("{name}.used_width"), child.used_width());
        pct_push(
            &mut out,
            &format!("{name}.used_height"),
            child.used_height(),
        );
        let passes = child.normal_width().map_or(0, |width| width.passes);
        pct_push(&mut out, &format!("{name}.passes"), f32::from(passes));
        let offset = child.relative_offset();
        for (side, value) in [
            ("left", offset.left),
            ("right", offset.right),
            ("top", offset.top),
            ("bottom", offset.bottom),
        ] {
            pct_push(&mut out, &format!("{name}.offset.{side}"), value);
        }
    }
    let placed = layout.sequence().placed_float();
    pct_push_edges(
        &mut out,
        "float",
        PrincipalEdges {
            left: placed.left,
            right: placed.right,
            top: placed.top,
            bottom: placed.bottom,
        },
    );
    for (index, tail) in layout.sequence().tail().iter().enumerate() {
        let (cleared, value) = tail.clearance().map_or((0.0, 0.0), |value| (1.0, value));
        pct_push(&mut out, &format!("tail{index}.cleared"), cleared);
        pct_push(&mut out, &format!("tail{index}.clearance"), value);
    }
    let receipt = layout.receipt();
    for (name, value) in [("work", receipt.work), ("core_work", receipt.core.work)] {
        pct_push(&mut out, name, f32::from(u16::try_from(value).unwrap()));
    }
    out
}

/// A producer error by name: `Unsupported` prints the property's CSS name.
fn pct_describe(error: &ClearanceBoxFactsError) -> String {
    match error {
        ClearanceBoxFactsError::Unsupported {
            original,
            property,
            target,
            reason,
        } => format!(
            "Unsupported {{ original: {original}, property: {}, target: {target:?}, \
             reason: {reason:?} }}",
            property.descriptor().name
        ),
        other => format!("{other:?}"),
    }
}

/// A Sequence producer error by name.
fn pct_sequence_describe(error: &SiblingBoxFactsError) -> String {
    match error {
        SiblingBoxFactsError::Evaluation(error) => {
            format!("Evaluation({})", pct_describe(error))
        }
        other => format!("{other:?}"),
    }
}

/// O's content geometry (left, width, top) of every row unless stated.
const PCT_OUTER: (f32, f32, f32) = (0.0, 100.0, 10.0);

/// Runs the facts (which must succeed; otherwise panics naming the producer error and
/// every source's original index) and the facade with O's content geometry `outer`,
/// then `observe`.
fn pct_with<T>(
    nodes: usize,
    css: &str,
    outer: (f32, f32, f32),
    limits: SiblingLayoutLimits,
    observe: impl FnOnce(&CapturedSiblingLayout<'_>) -> T,
) -> Result<T, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| {
                let originals: Vec<usize> = (0..nodes)
                    .map(|source| projection.original_index(source).unwrap())
                    .collect();
                panic!(
                    "{css}: producer error {} (originals {originals:?})",
                    pct_sequence_describe(&error)
                )
            });
        let (left, width, top) = outer;
        out = Some(
            default_outer_layout(&facts, &projection.nodes()[0], left, width, top, limits)
                .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// Success-asserting view with O's content geometry `outer`.
fn pct_layout_in(nodes: usize, css: &str, outer: (f32, f32, f32)) -> Vec<(String, f32)> {
    pct_with(nodes, css, outer, layout_limits(), pct_view)
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

fn pct_layout(nodes: usize, css: &str) -> Vec<(String, f32)> {
    pct_layout_in(nodes, css, PCT_OUTER)
}

/// One literal expectation.
fn pct_lit(name: &str, value: f32) -> (String, f32) {
    (name.to_owned(), value)
}

/// Child `index`'s border, padding and content edge `side` (children carry no
/// padding or border in these rows, so the three boxes share their edges).
fn pct_x(index: usize, side: &str, value: f32) -> Vec<(String, f32)> {
    ["border", "padding", "content"]
        .into_iter()
        .map(|kind| (format!("c{index}.{kind}.{side}"), value))
        .collect()
}

/// `view` has the `expected` literals and every other number of `control`, bit for
/// bit; the two views publish the same names.
fn assert_pct_view(
    view: &[(String, f32)],
    control: &[(String, f32)],
    expected: &[(String, f32)],
    context: &str,
) {
    let names = |v: &[(String, f32)]| v.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(view), names(control), "{context}: published names");
    let mut wrong = Vec::new();
    for (name, value) in expected {
        match view.iter().find(|(n, _)| n == name) {
            Some((_, actual)) if actual.to_bits() == value.to_bits() => {}
            Some((_, actual)) => wrong.push(format!("{name} {actual} (expected {value})")),
            None => wrong.push(format!("{name} not published")),
        }
    }
    for ((name, actual), (_, twin)) in view.iter().zip(control) {
        if !expected.iter().any(|(n, _)| n == name) && actual.to_bits() != twin.to_bits() {
            wrong.push(format!("{name} {actual} (control {twin})"));
        }
    }
    assert!(wrong.is_empty(), "{context}: {wrong:?}");
}

/// An admitted N5 row: `fragment` gives the `expected` literals and every other number
/// of the baseline (or `control`) layout, and every number of its absolute `twin`.
fn assert_pct_row(fragment: &str, control: &str, twin: &str, expected: &[(String, f32)]) {
    let css = pct_css(5, fragment);
    let view = pct_layout(5, &css);
    assert_pct_view(&view, &pct_layout(5, &pct_css(5, control)), expected, &css);
    assert_pct_view(&view, &pct_layout(5, &pct_css(5, twin)), &[], &css);
}

/// The producer's error and every source's original index; panics when the facts
/// succeed.
fn pct_fact_error(nodes: usize, css: &str) -> (SiblingBoxFactsError, Vec<usize>) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let originals = (0..nodes)
            .map(|source| projection.original_index(source).unwrap())
            .collect();
        match evaluate_sibling_box_facts(projection, display, sibling_fact_limits()) {
            Err(error) => out = Some((error, originals)),
            Ok(_) => panic!("{css}: expected a fact error, got facts Ok"),
        }
    });
    out.unwrap()
}

/// The Legacy producer's result for a five-source capture of `css` and every source's
/// original index in the Legacy projection.
fn pct_legacy_error(css: &str) -> (Option<ClearanceBoxFactsError>, Vec<usize>) {
    let mut out = None;
    sibling_fixture(5, css, |projection, display| {
        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let originals = (0..5)
            .map(|projected| legacy_projection.original_index(projected).unwrap())
            .collect();
        let error = evaluate_clearance_box_facts(&legacy_projection, display, fact_limits()).err();
        out = Some((error, originals));
    });
    out.unwrap()
}

/// The `Unsupported` refusal text of the element longhand `name` at `original`.
fn pct_refusal(original: usize, name: &str, reason: BoxFactUnsupported) -> String {
    format!(
        "Unsupported {{ original: {original}, property: {name}, target: Element, reason: \
         {reason:?} }}"
    )
}

/// T1 (RED; M1 witness): `width:50%` of P's used width 100 is 50: B1's used width 50,
/// margins (0, 50), `passes` 1, border [0, 50] x [11, 12]; every other number is the
/// baseline's and every number is the `width:50px` twin's. Predecessor at c64dfb5f:
/// the producer refuses B1's `width` with `Evaluation(Unsupported { original: 0,
/// property: width, target: Element, reason: PercentageLength })`.
#[test]
fn pct_red_t1_width_percentage_narrows_b1() {
    let mut expected = vec![
        pct_lit("c0.used_width", 50.0),
        pct_lit("c0.margin.right", 50.0),
        pct_lit("c0.passes", 1.0),
    ];
    expected.extend(pct_x(0, "right", 50.0));
    assert_pct_row("#b1 { width:50%; }", "", "#b1 { width:50px; }", &expected);
}

/// T2 (RED; M2 witness): P's `margin-left:10%` of O's content width 100 is 10: P width
/// 90, content [10, 100]; B1 and B2 [10, 100]; F placed [10, 20]; the vertical numbers
/// stay the baseline. Predecessor at c64dfb5f: `Evaluation(Unsupported { original: 3,
/// property: margin-left, target: Element, reason: PercentageLength })`.
#[test]
fn pct_red_t2_parent_margin_left_percentage_moves_p() {
    let mut expected = vec![
        pct_lit("p.margin_left", 10.0),
        pct_lit("p.width", 90.0),
        pct_lit("p.content_left", 10.0),
        pct_lit("c0.used_width", 90.0),
        pct_lit("c2.used_width", 90.0),
        pct_lit("float.left", 10.0),
        pct_lit("float.right", 20.0),
    ];
    for child in [0, 1, 2] {
        expected.extend(pct_x(child, "left", 10.0));
    }
    expected.extend(pct_x(1, "right", 20.0));
    assert_pct_row(
        "#p { margin-left:10%; }",
        "",
        "#p { margin-left:10px; }",
        &expected,
    );
}

/// FX2 (guard; M4, M5, M6 witness): every percentage outside `width`, the margins,
/// `right` and `left` and, after PPM and PHT, the paddings, `min-width`, `max-width`,
/// `height`, `min-height` and `max-height` keeps `PercentageLength` at its own index on
/// a Sequence row: `top` and `bottom`. Passes at RED and after PCT, PPM and PHT.
#[test]
fn pct_red_guard_fx2_other_percentages_stay_refused() {
    for (fragment, name) in [
        ("#b1 { position:relative; top:10%; }", "top"),
        ("#b1 { position:relative; bottom:10%; }", "bottom"),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_fact_error(5, &css);
        assert_eq!(
            pct_sequence_describe(&error),
            format!(
                "Evaluation({})",
                pct_refusal(originals[2], name, BoxFactUnsupported::PercentageLength)
            ),
            "{css}"
        );
    }
}

/// LG (Legacy guard, C-6; M3 witness): the Legacy entry refuses every percentage
/// unchanged, here B2's `margin-left` (original 2) and F's `width` (original 1), each
/// alone. Passes at RED and after PCT.
#[test]
fn pct_red_guard_lg_legacy_refuses_percentages() {
    for (fragment, projected, name) in [
        ("#b2 { margin-left:10%; }", 4, "margin-left"),
        ("#f { width:10%; }", 3, "width"),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_legacy_error(&css);
        let error = error.unwrap_or_else(|| panic!("{css}: Legacy facts Ok"));
        assert_eq!(
            pct_describe(&error),
            pct_refusal(
                originals[projected],
                name,
                BoxFactUnsupported::PercentageLength
            ),
            "{css}"
        );
        assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: Legacy originals");
    }
}

// FLIP rows: they read the percentage facts and the PCT error.

use iced_css_core::{
    formatting::bfc_height::BfcHeightError,
    values::{CompactDecimal, ConversionError, Decimal},
};

/// `f32::MAX` in CSS pixels (an exact decimal).
const PCT_F32_MAX_PX: &str = "340282346638528859811704183484516925440px";

/// The bounded decimal of `text` (an exact CSS decimal).
fn pct_value(text: &str) -> CompactDecimal {
    Decimal::parse(text, values().numeric)
        .unwrap()
        .compact()
        .unwrap()
}

/// A computed percentage fact.
fn pct_fact(text: &str, origin: BoxFactOrigin) -> BoxLengthFact {
    BoxLengthFact::Percentage(PercentageBoxFact {
        value: pct_value(text),
        origin,
    })
}

/// Child `index`'s vertical edges of all three boxes.
fn pct_y(index: usize, top: f32, bottom: f32) -> Vec<(String, f32)> {
    let mut out = pct_x(index, "top", top);
    out.extend(pct_x(index, "bottom", bottom));
    out
}

/// The Sequence fact rows, the fact receipt and every source's original index; the
/// producer must succeed.
fn pct_facts(
    nodes: usize,
    css: &str,
) -> (Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt, Vec<usize>) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| {
                panic!("{css}: producer error {}", pct_sequence_describe(&error))
            });
        let rows = (0..nodes)
            .map(|source| *facts.row(source).unwrap())
            .collect();
        let originals = (0..nodes)
            .map(|source| projection.original_index(source).unwrap())
            .collect();
        out = Some((rows, facts.receipt().evaluation, originals));
    });
    out.unwrap()
}

/// Layout-error-asserting helper with O's content geometry `outer` (the facts must
/// succeed): panics on a layout.
fn pct_error_in(css: &str, outer: (f32, f32, f32)) -> SiblingLayoutError {
    match pct_with(5, css, outer, layout_limits(), |_| ()) {
        Ok(()) => panic!("{css}: expected a layout error, got a layout"),
        Err(error) => error,
    }
}

/// The computed width, left and right margin facts of P and of each child.
fn pct_computed(css: &str) -> Vec<(BoxLengthFact, BoxLengthFact, BoxLengthFact)> {
    pct_with(5, css, PCT_OUTER, layout_limits(), |layout| {
        let parent = layout.parent();
        let mut out = vec![(
            parent.computed_width(),
            parent.computed_margin_top(),
            parent.computed_margin_bottom(),
        )];
        out.extend(layout.children().iter().map(|child| {
            (
                child.computed_width(),
                child.computed_margin_left(),
                child.computed_margin_right(),
            )
        }));
        out
    })
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// T1 and T6 (M21 witness): the computed accessors publish the percentage facts, not
/// the resolved values: B1's `computed_width()` is `Percentage(50, Specified)`, F's
/// `Percentage(30, Specified)`, B1's `computed_margin_right()` `Percentage(5,
/// Specified)` and P's `computed_margin_top()` `Percentage(2, Specified)`.
#[test]
fn pct_t1_computed_facts_keep_the_percentage() {
    let css = pct_css(
        5,
        "#b1 { width:50%; margin-right:5%; } #f { width:30%; } #p { margin-top:2%; }",
    );
    let computed = pct_computed(&css);
    let specified = BoxFactOrigin::Specified;
    assert_eq!(computed[0].1, pct_fact("2", specified), "{css}: P");
    assert_eq!(computed[1].0, pct_fact("50", specified), "{css}: B1 width");
    assert_eq!(computed[1].2, pct_fact("5", specified), "{css}: B1 margin");
    assert_eq!(computed[2].0, pct_fact("30", specified), "{css}: F");
}

/// T3: P's `width:50%` of O's 100 is 50: margin-right 50, content [0, 50]; B1 and B2
/// [0, 50]; F keeps [0, 10].
#[test]
fn pct_t3_parent_width_percentage() {
    let mut expected = vec![
        pct_lit("p.width", 50.0),
        pct_lit("p.margin_right", 50.0),
        pct_lit("p.content_right", 50.0),
        pct_lit("c0.used_width", 50.0),
        pct_lit("c2.used_width", 50.0),
    ];
    expected.extend(pct_x(0, "right", 50.0));
    expected.extend(pct_x(2, "right", 50.0));
    assert_pct_row("#p { width:50%; }", "", "#p { width:50px; }", &expected);
}

/// T3b (C-3; M10 witness): with O's content width 200, P's `width:50%` is 100 and its
/// margin-right 100; the layout equals the `width:100px` twin with the same O.
#[test]
fn pct_t3b_parent_basis_is_the_outer_width() {
    let outer = (0.0, 200.0, 10.0);
    let css = pct_css(5, "#p { width:50%; }");
    let view = pct_layout_in(5, &css, outer);
    let twin = pct_layout_in(5, &pct_css(5, "#p { width:100px; }"), outer);
    let expected = [
        pct_lit("p.width", 100.0),
        pct_lit("p.margin_right", 100.0),
        pct_lit("p.content_right", 100.0),
        pct_lit("c0.used_width", 100.0),
    ];
    assert_pct_view(&view, &twin, &expected, &css);
}

/// T4: B1 `width:50%` and `margin-left:25%` with `margin-right:auto`: (50, 25, 25),
/// border [25, 75].
#[test]
fn pct_t4_width_and_margin_percentages_with_an_auto_margin() {
    let mut expected = vec![
        pct_lit("c0.used_width", 50.0),
        pct_lit("c0.margin.left", 25.0),
        pct_lit("c0.margin.right", 25.0),
    ];
    expected.extend(pct_x(0, "left", 25.0));
    expected.extend(pct_x(0, "right", 75.0));
    assert_pct_row(
        "#b1 { width:50%; margin-left:25%; margin-right:auto; }",
        "",
        "#b1 { width:50px; margin-left:25px; margin-right:auto; }",
        &expected,
    );
}

/// T5 (M24 witness): a child's basis is P's used width 50, not O's 100: B1 25,
/// margins (0, 25), border [0, 25]; F's `width:20%` of 50 is its baseline 10.
#[test]
fn pct_t5_child_basis_is_the_parent_width() {
    let mut expected = vec![
        pct_lit("p.width", 50.0),
        pct_lit("p.margin_right", 50.0),
        pct_lit("p.content_right", 50.0),
        pct_lit("c0.used_width", 25.0),
        pct_lit("c0.margin.right", 25.0),
        pct_lit("c2.used_width", 50.0),
    ];
    expected.extend(pct_x(0, "right", 25.0));
    expected.extend(pct_x(2, "right", 50.0));
    assert_pct_row(
        "#p { width:50%; } #b1 { width:50%; } #f { width:20%; }",
        "",
        "#p { width:50px; } #b1 { width:25px; } #f { width:10px; }",
        &expected,
    );
}

/// T6: F's `width:30%` is 30: placed [0, 30] x [16, 18].
#[test]
fn pct_t6_float_width_percentage() {
    let mut expected = vec![pct_lit("c1.used_width", 30.0), pct_lit("float.right", 30.0)];
    expected.extend(pct_x(1, "right", 30.0));
    assert_pct_row("#f { width:30%; }", "", "#f { width:30px; }", &expected);
}

/// T7 (M14 witness): F's resolved width 30 is above `max-width` 20: used width 20.
#[test]
fn pct_t7_float_width_percentage_is_constrained() {
    let mut expected = vec![pct_lit("c1.used_width", 20.0), pct_lit("float.right", 20.0)];
    expected.extend(pct_x(1, "right", 20.0));
    assert_pct_row(
        "#f { width:30%; max-width:20px; }",
        "",
        "#f { width:20px; }",
        &expected,
    );
}

/// T8: F's `margin-left:5%` is 5: margin box [0, 15], border band [5, 15].
#[test]
fn pct_t8_float_margin_percentage() {
    let mut expected = vec![pct_lit("c1.margin.left", 5.0), pct_lit("float.right", 15.0)];
    expected.extend(pct_x(1, "left", 5.0));
    expected.extend(pct_x(1, "right", 15.0));
    assert_pct_row(
        "#f { margin-left:5%; }",
        "",
        "#f { margin-left:5px; }",
        &expected,
    );
}

/// T9 (M15 witness): a vertical margin percentage uses the width basis (CSS2.2 8.3):
/// B1's `margin-bottom:10%` is 10; F [22, 24]; B2's clearance max(24 - 25, 22 - 25) =
/// -1, border [24, 25]; P [10, 11, 28, 29, 29].
#[test]
fn pct_t9_vertical_margin_uses_the_width_basis() {
    let mut expected = vec![
        pct_lit("c0.margin.bottom", 10.0),
        pct_lit("float.top", 22.0),
        pct_lit("float.bottom", 24.0),
        pct_lit("tail0.clearance", -1.0),
        pct_lit("p.content_bottom", 28.0),
        pct_lit("p.padding_bottom", 29.0),
        pct_lit("p.border_bottom", 29.0),
        pct_lit("p.margin_bottom", 29.0),
        pct_lit("p.used_height", 17.0),
        pct_lit("p.extent.normal_margin_bottom", 28.0),
        pct_lit("p.extent.selected_bottom", 28.0),
    ];
    expected.extend(pct_y(1, 22.0, 24.0));
    expected.extend(pct_y(2, 24.0, 25.0));
    assert_pct_row(
        "#b1 { margin-bottom:10%; }",
        "",
        "#b1 { margin-bottom:10px; }",
        &expected,
    );
}

/// T10: under an `rtl` P, B1's `width:50%` leaves the free margin on the left: margins
/// (50, 0), border [50, 100].
#[test]
fn pct_t10_rtl_parent_moves_the_free_margin_left() {
    let mut expected = vec![
        pct_lit("c0.used_width", 50.0),
        pct_lit("c0.margin.left", 50.0),
    ];
    expected.extend(pct_x(0, "left", 50.0));
    assert_pct_row(
        "#p { direction:rtl; } #b1 { width:50%; }",
        "#p { direction:rtl; }",
        "#p { direction:rtl; } #b1 { width:50px; }",
        &expected,
    );
}

/// T11: B1's `left:10%` of P's 100 gives the offsets (10, -10, +0, +0); the normal-flow
/// numbers stay the baseline.
#[test]
fn pct_t11_left_percentage_offsets_b1() {
    let expected = [
        pct_lit("c0.offset.left", 10.0),
        pct_lit("c0.offset.right", -10.0),
    ];
    assert_pct_row(
        "#b1 { position:relative; left:10%; }",
        "",
        "#b1 { position:relative; left:10px; }",
        &expected,
    );
}

/// T11b (C-3; M13 witness): under a 50px P, B1's `left:10%` is 5, not 10: offsets
/// (5, -5, +0, +0).
#[test]
fn pct_t11b_child_offset_basis_is_the_parent_width() {
    let expected = [
        pct_lit("c0.offset.left", 5.0),
        pct_lit("c0.offset.right", -5.0),
    ];
    assert_pct_row(
        "#p { width:50px; } #b1 { position:relative; left:10%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #b1 { position:relative; left:5px; }",
        &expected,
    );
}

/// T12: a negative margin percentage is admitted: B1 (110, -10, 0), border [-10, 100]
/// (P `hidden`).
#[test]
fn pct_t12_negative_margin_percentage() {
    let mut expected = vec![
        pct_lit("c0.used_width", 110.0),
        pct_lit("c0.margin.left", -10.0),
    ];
    expected.extend(pct_x(0, "left", -10.0));
    assert_pct_row(
        "#b1 { margin-left:-10%; }",
        "",
        "#b1 { margin-left:-10px; }",
        &expected,
    );
}

/// T13 (C-2): the property grammar drops a negative `width` percentage, so B1's width
/// fact is the initial `Auto`; the producer's `InvalidValue` test is defensive.
#[test]
fn pct_t13_negative_width_percentage_is_dropped_by_the_parser() {
    let css = pct_css(5, "#b1 { width:-10%; }");
    let (rows, _, _) = pct_facts(5, &css);
    assert_eq!(
        rows[2].width,
        BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        },
        "{css}"
    );
}

/// T14 (M8, M9 witness): `margin-left:inherit` copies P's computed percentage with
/// origin `Inherited`, which B1 resolves against P's used width 90: margin-left 9,
/// border [19, 100].
#[test]
fn pct_t14_inherit_keeps_the_percentage() {
    let fragment = "#p { margin-left:10%; } #b1 { margin-left:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(rows[2].margin[3], pct_fact("10", inherited), "{css}");
    let mut expected = vec![
        pct_lit("p.margin_left", 10.0),
        pct_lit("p.width", 90.0),
        pct_lit("p.content_left", 10.0),
        pct_lit("c0.used_width", 81.0),
        pct_lit("c0.margin.left", 9.0),
        pct_lit("c2.used_width", 90.0),
        pct_lit("float.left", 10.0),
        pct_lit("float.right", 20.0),
    ];
    expected.extend(pct_x(0, "left", 19.0));
    expected.extend(pct_x(1, "left", 10.0));
    expected.extend(pct_x(1, "right", 20.0));
    expected.extend(pct_x(2, "left", 10.0));
    assert_pct_row(
        fragment,
        "",
        "#p { margin-left:10px; } #b1 { margin-left:9px; }",
        &expected,
    );
}

/// T15 (M12 witness): the largest stored coefficient is `u64::MAX`; its percentage of
/// 100 rounds once to 2^64, so B1's width reruns at its minimum: width 0, margins
/// (0, 100), `passes` 2. One more is `PercentagePrecision` at B1's `margin-right`.
#[test]
fn pct_t15_precision_boundary() {
    let mut expected = vec![
        pct_lit("c0.used_width", 0.0),
        pct_lit("c0.margin.right", 100.0),
        pct_lit("c0.passes", 2.0),
    ];
    expected.extend(pct_x(0, "right", 0.0));
    assert_pct_row(
        "#b1 { margin-right:18446744073709551615%; }",
        "",
        "#b1 { margin-right:18446744073709551616px; }",
        &expected,
    );
    let css = pct_css(5, "#b1 { margin-right:18446744073709551616%; }");
    let (error, originals) = pct_fact_error(5, &css);
    assert_eq!(
        pct_sequence_describe(&error),
        format!(
            "Evaluation({})",
            pct_refusal(
                originals[2],
                "margin-right",
                BoxFactUnsupported::PercentagePrecision
            )
        ),
        "{css}"
    );
}

/// T16 (M16 witness): under a P of width `f32::MAX`, B1's `margin-left:200%` exceeds
/// `f32::MAX`: `Percentage { source: 2, property: "margin-left", error: Unrepresentable
/// }`, and the error's source is the conversion error.
#[test]
fn pct_t16_unrepresentable_resolution_is_a_percentage_error() {
    let css = pct_css(
        5,
        &format!("#p {{ width:{PCT_F32_MAX_PX}; }} #b1 {{ margin-left:200%; }}"),
    );
    let error = pct_error_in(&css, PCT_OUTER);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Percentage {
                source: 2,
                property: "margin-left",
                error: ConversionError::Unrepresentable,
            }
        ),
        "{css}: {error:?}"
    );
    let source = std::error::Error::source(&error).unwrap();
    assert_eq!(
        source.downcast_ref::<ConversionError>(),
        Some(&ConversionError::Unrepresentable),
        "{css}"
    );
}

/// T17: P's `right:5%` of O's 100 gives P the offsets (-5, 5, +0, +0) (O ltr); the
/// normal-flow numbers stay the baseline.
#[test]
fn pct_t17_parent_right_percentage() {
    let expected = [
        pct_lit("p.offset.left", -5.0),
        pct_lit("p.offset.right", 5.0),
    ];
    assert_pct_row(
        "#p { position:relative; right:5%; }",
        "",
        "#p { position:relative; right:5px; }",
        &expected,
    );
}

/// The layout error of `css` (N5) with the facts' numeric digit limit `digits` and O's
/// content geometry `outer`; panics on a producer error or a layout.
fn pct_error_with_digits(css: &str, outer: (f32, f32, f32), digits: usize) -> String {
    let mut out = None;
    sibling_fixture(5, css, |projection, display| {
        let mut limits = sibling_fact_limits();
        limits.evaluation.numeric.digits = nz(digits);
        let facts =
            evaluate_sibling_box_facts(projection, display, limits).unwrap_or_else(|error| {
                panic!("{css}: producer error {}", pct_sequence_describe(&error))
            });
        let (left, width, top) = outer;
        let result = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            left,
            width,
            top,
            layout_limits(),
        );
        out = Some(result.err().map(|error| format!("{error:?}")));
    });
    out.unwrap()
        .unwrap_or_else(|| panic!("{css}: expected a layout error, got a layout"))
}

/// T18 (M17, M25 witness): an invalid O basis keeps the P solver's own error, equal to
/// the absolute twin's: a NaN width is `containing_width`, a NaN left `InvalidOrigin`.
/// The error is unconditional (plan 2, K-1): in a capture whose lengths are all zero,
/// with the facts' digit limit 2 below the four digits of P's `12.25%`, the primitive is
/// never called for an invalid basis, while a valid basis reaches it and fails with
/// `Numeric(DigitLimit)`.
#[test]
fn pct_t18_invalid_outer_basis_keeps_the_solver_error() {
    let css = "#p { overflow:hidden; width:12.25%; } #b1 { height:0px; } #f { float:left; \
               width:10%; } #b2 { height:0px; clear:both; }";
    for (outer, text) in [
        (
            (0.0, f32::NAN, 10.0),
            "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
        ),
        ((f32::NAN, 100.0, 10.0), "ParentHorizontal(InvalidOrigin)"),
        (
            PCT_OUTER,
            "Percentage { source: 1, property: \"width\", error: Numeric(DigitLimit) }",
        ),
    ] {
        assert_eq!(
            pct_error_with_digits(css, outer, 2),
            text,
            "{css}: digits 2"
        );
    }
    for (outer, text) in [
        (
            (0.0, f32::NAN, 10.0),
            "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
        ),
        ((f32::NAN, 100.0, 10.0), "ParentHorizontal(InvalidOrigin)"),
    ] {
        for fragment in ["#p { width:50%; }", "#p { width:50px; }"] {
            let css = pct_css(5, fragment);
            assert_eq!(format!("{:?}", pct_error_in(&css, outer)), text, "{css}");
        }
    }
}

/// C-5 (M7 witness): P's held failure keeps every earlier error first: with O's width
/// NaN, F's `FloatWidth` refusal (source 3) and B1's `ChildOverflow` (source 2) come
/// before P's `containing_width` error, which a later child percentage does not move.
#[test]
fn pct_c5_held_parent_error_keeps_the_order() {
    let outer = (0.0, f32::NAN, 10.0);
    for (fragment, text) in [
        (
            "#f { width:auto; } #p { width:50%; }",
            "Captured(Unsupported { projected: 3, property: \"width\", reason: FloatWidth })",
        ),
        (
            "#b1 { overflow:scroll; } #p { width:50%; }",
            "Captured(Unsupported { projected: 2, property: \"overflow\", reason: \
             ChildOverflow })",
        ),
        (
            "#p { width:50%; } #b1 { width:50%; }",
            "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
        ),
    ] {
        let css = pct_css(5, fragment);
        assert_eq!(format!("{:?}", pct_error_in(&css, outer)), text, "{css}");
    }
}

/// T19 (C-3; M20 witness): a child's admission tests precede its resolution: B1's
/// `overflow:scroll` is refused before its unrepresentable `margin-left:200%`.
#[test]
fn pct_t19_child_admission_precedes_resolution() {
    let css = pct_css(
        5,
        &format!("#p {{ width:{PCT_F32_MAX_PX}; }} #b1 {{ margin-left:200%; overflow:scroll; }}"),
    );
    assert_eq!(
        format!("{:?}", pct_error_in(&css, PCT_OUTER)),
        "Captured(Unsupported { projected: 2, property: \"overflow\", reason: ChildOverflow \
         })",
        "{css}"
    );
}

/// RW1: B1's `margin-left:0.007%` of 100 is one nearest-even rounding of 0.007
/// (`3be56042`); the width is the exact 100 minus that margin, rounded (`42c7fc6a`).
#[test]
fn pct_rw1_one_rounding_margin() {
    let mut expected = vec![
        pct_lit("c0.margin.left", f32::from_bits(0x3be56042)),
        pct_lit("c0.used_width", f32::from_bits(0x42c7fc6a)),
    ];
    expected.extend(pct_x(0, "left", f32::from_bits(0x3be56042)));
    assert_pct_row(
        "#b1 { margin-left:0.007%; }",
        "",
        "#b1 { margin-left:0.007px; }",
        &expected,
    );
}

/// RW2: under a 7px P, B1's `width:1.001%` is one rounding of 0.07007 (`3d8f80dc`);
/// margin-right `40ddc1fd`.
#[test]
fn pct_rw2_one_rounding_width() {
    let mut expected = vec![
        pct_lit("c0.used_width", f32::from_bits(0x3d8f80dc)),
        pct_lit("c0.margin.right", f32::from_bits(0x40ddc1fd)),
    ];
    expected.extend(pct_x(0, "right", f32::from_bits(0x3d8f80dc)));
    assert_pct_row(
        "#p { width:7px; } #b1 { width:1.001%; }",
        "#p { width:7px; }",
        "#p { width:7px; } #b1 { width:0.07007px; }",
        &expected,
    );
}

/// RW3 (C-3; M11 witness): B1's `margin-left:0.049%` of 100 is one rounding of 0.049
/// (`3d48b439`); every f32 pipeline (f32(p) * 100 / 100, f32(p / 100) * 100,
/// f32(f32(p) / 100) * 100) gives `3d48b43a`. The width is the exact 100 minus the
/// margin, rounded (`42c7e6e9`).
#[test]
fn pct_rw3_one_rounding_beats_every_f32_pipeline() {
    let exact = f32::from_bits(0x3d48b439);
    let p = 0.049_f32;
    for pipeline in [p * 100.0 / 100.0, 0.000_49_f32 * 100.0, (p / 100.0) * 100.0] {
        assert_eq!(pipeline.to_bits(), 0x3d48b43a);
    }
    let mut expected = vec![
        pct_lit("c0.margin.left", exact),
        pct_lit("c0.used_width", f32::from_bits(0x42c7e6e9)),
    ];
    expected.extend(pct_x(0, "left", exact));
    assert_pct_row(
        "#b1 { margin-left:0.049%; }",
        "",
        "#b1 { margin-left:0.049px; }",
        &expected,
    );
}

/// FX1: the facts keep `width:50%` and `margin-top:-2.5%` exactly, `Specified`, with no
/// counted conversion: `numeric_calls` equals the baseline's.
#[test]
fn pct_fx1_percentage_facts_are_exact_and_unconverted() {
    let css = pct_css(5, "#b1 { width:50%; margin-top:-2.5%; }");
    let (rows, receipt, _) = pct_facts(5, &css);
    let (_, baseline, _) = pct_facts(5, &pct_css(5, ""));
    let specified = BoxFactOrigin::Specified;
    assert_eq!(rows[2].width, pct_fact("50", specified), "{css}");
    assert_eq!(rows[2].margin[0], pct_fact("-2.5", specified), "{css}");
    assert!(pct_value("-2.5").is_negative() && !pct_value("-0").is_negative());
    assert_eq!(receipt.numeric_calls, baseline.numeric_calls, "{css}");
}

/// K2-F (MMX K-2, ML-10; M18 witness): P's content top 10 + 2^103 rounds to 2^103; its
/// Auto span is below `min-height` `f32::MAX`, so the explicit rerun's content bottom
/// 2^103 + `f32::MAX` = 2^128 - 2^103 is the midpoint of `f32::MAX` and 2^128 and rounds
/// past `f32::MAX`: `Height(Unrepresentable { stage: ContentBottom })` from
/// `constrain_bfc_height`. B2 does not clear, so no equality refusal comes first.
#[test]
fn pct_k2f_parent_min_height_rerun_overflows() {
    let css = pct_css(
        5,
        &format!(
            "#p {{ padding-top:10141204801825835211973625643008px; min-height:{PCT_F32_MAX_PX}; \
             }} #b2 {{ clear:none; }}"
        ),
    );
    let error = pct_error_in(&css, PCT_OUTER);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Height(BfcHeightError::Unrepresentable {
                stage: BfcHeightStage::ContentBottom,
            })
        ),
        "{css}: {error:?}"
    );
}

/// The work fixture: percentages on every source that resolve to the baseline values.
const PCT_WORK_FRAGMENT: &str = "#o { width:100%; margin-left:0%; } \
                                 #p { width:100%; margin-left:0%; margin-right:0%; \
                                 margin-top:0%; } \
                                 #f { width:10%; margin-left:0%; } \
                                 #b1, #b2, #b3, #b4 { margin-left:0%; margin-right:0%; }";

/// Facade and core work (M19 witness): exact and one short at N5, N6 and N7 with
/// percentages on every source; the facade charge is `461 + 32N + 449(N-2) +
/// 313(N-3)`; the layout equals the baseline's.
#[test]
fn pct_work_thresholds_with_percentages() {
    for (nodes, facade, core) in [(5, 2594, 1249), (6, 3388, 1501), (7, 4182, 1754)] {
        assert_eq!(
            facade,
            461 + 32 * nodes + 449 * (nodes - 2) + 313 * (nodes - 3)
        );
        let css = pct_css(nodes, PCT_WORK_FRAGMENT);
        assert_pct_view(
            &pct_layout(nodes, &css),
            &pct_layout(nodes, &pct_css(nodes, "")),
            &[],
            &css,
        );
        let limits = |work: usize, core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = pct_with(nodes, &css, PCT_OUTER, limits(facade, core), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert!(
            matches!(
                pct_with(nodes, &css, PCT_OUTER, limits(facade - 1, core), |_| ()),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                pct_with(nodes, &css, PCT_OUTER, limits(facade, core - 1), |_| ()),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}

/// D4 measurement: the new `Copy` types (x86_64; measured at FLIP).
#[test]
fn pct_representation_measurement() {
    let measured = [
        (
            "CompactDecimal",
            std::mem::size_of::<CompactDecimal>(),
            std::mem::align_of::<CompactDecimal>(),
        ),
        (
            "PercentageBoxFact",
            std::mem::size_of::<PercentageBoxFact>(),
            std::mem::align_of::<PercentageBoxFact>(),
        ),
        (
            "BoxLengthFact",
            std::mem::size_of::<BoxLengthFact>(),
            std::mem::align_of::<BoxLengthFact>(),
        ),
    ];
    for (name, size, align) in measured {
        println!("PCT_REPRESENTATION {name} {size} {align}");
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    assert_eq!(
        measured.map(|(_, size, align)| (size, align)),
        [(16, 8), (32, 8), (32, 8)]
    );
}

// PPM: the padding, `min-width` and `max-width` percentage rows reuse this module's
// private helpers as a child module (review C-3, ruling PM-3).
#[path = "padding_min_max_percentages.rs"]
mod padding_min_max_percentages;
