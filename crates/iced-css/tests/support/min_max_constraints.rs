//! MMX: absolute `min-width`, `max-width`, `min-height` and `max-height` on every
//! Sequence row (CSS2.2 10.4 and 10.7).
//!
//! The RED part of this module (the helpers, W1 and W2) compiles against the
//! predecessor API at a75c2066: it reads only published geometry. The rows appended at
//! FLIP also read the constraint facts. Every literal is derived from `baseline_css()`
//! (O content left 0, width 100, top 10; P `hidden` with padding-top and padding-bottom
//! of 1) by the MMX plan's exact-rational model; comparing every other published number
//! with a control is additional, never the only oracle.

use super::*;

/// The facade fixture: `baseline_css()`, the N7 leading rule, then the fragment.
fn mmx_css(nodes: usize, fragment: &str) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} {fragment}", baseline_css())
}

fn mmx_push(out: &mut Vec<(String, f32)>, name: &str, value: f32) {
    out.push((name.to_owned(), value));
}

fn mmx_push_edges(out: &mut Vec<(String, f32)>, name: &str, edges: PrincipalEdges) {
    for (side, value) in [
        ("left", edges.left),
        ("right", edges.right),
        ("top", edges.top),
        ("bottom", edges.bottom),
    ] {
        mmx_push(out, &format!("{name}.{side}"), value);
    }
}

/// Every published number of one layout by name (counts and flags as exact f32
/// integers). P: `p.*`; children `c0` (B1), `c1` (F), `c2` (B2), ... in source order;
/// `float` is `placed_float()`; `tailN.*` are the tail clearances. An absent
/// `auto_extent` publishes `p.extent` 0 and its three fields 0.
fn mmx_view(layout: &CapturedSiblingLayout<'_>) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    let parent = layout.parent();
    let origin = parent.origin();
    let height = parent.height();
    let bottom = parent.bottom();
    let horizontal = parent.horizontal();
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
    ] {
        mmx_push(&mut out, name, value);
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
    mmx_push(&mut out, "p.extent", extent);
    mmx_push(&mut out, "p.extent.normal_margin_bottom", normal);
    mmx_push(&mut out, "p.extent.selected_bottom", selected);
    mmx_push(&mut out, "p.extent.float_extended", extended);
    for (index, child) in layout.children().iter().enumerate() {
        let name = format!("c{index}");
        mmx_push(
            &mut out,
            &format!("{name}.source"),
            f32::from(u8::try_from(child.source_index()).unwrap()),
        );
        mmx_push_edges(&mut out, &format!("{name}.border"), child.border());
        mmx_push_edges(&mut out, &format!("{name}.padding"), child.padding());
        mmx_push_edges(&mut out, &format!("{name}.content"), child.content());
        for (side, value) in [
            ("top", child.used_margin_top()),
            ("right", child.used_margin_right()),
            ("bottom", child.used_margin_bottom()),
            ("left", child.used_margin_left()),
        ] {
            mmx_push(&mut out, &format!("{name}.margin.{side}"), value);
        }
        mmx_push(&mut out, &format!("{name}.used_width"), child.used_width());
        mmx_push(
            &mut out,
            &format!("{name}.used_height"),
            child.used_height(),
        );
        let passes = child.normal_width().map_or(0, |width| width.passes);
        mmx_push(&mut out, &format!("{name}.passes"), f32::from(passes));
    }
    let placed = layout.sequence().placed_float();
    mmx_push_edges(
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
        mmx_push(&mut out, &format!("tail{index}.cleared"), cleared);
        mmx_push(&mut out, &format!("tail{index}.clearance"), value);
    }
    let receipt = layout.receipt();
    for (name, value) in [("work", receipt.work), ("core_work", receipt.core.work)] {
        mmx_push(&mut out, name, f32::from(u16::try_from(value).unwrap()));
    }
    out
}

/// A producer error by name: `Unsupported` prints the property's CSS name.
fn mmx_describe(error: &SiblingBoxFactsError) -> String {
    match error {
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Unsupported {
            original,
            property,
            target,
            reason,
        }) => format!(
            "Evaluation(Unsupported {{ original: {original}, property: {}, target: {target:?}, \
             reason: {reason:?} }})",
            property.descriptor().name
        ),
        other => format!("{other:?}"),
    }
}

/// Runs the facts (which must succeed; otherwise panics naming the producer error and
/// every source's original index) and the facade, then `observe`.
fn mmx_with<T>(
    nodes: usize,
    css: &str,
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
                    mmx_describe(&error)
                )
            });
        out = Some(
            default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
                .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// Success-asserting view: panics with the typed error otherwise.
fn mmx_layout(nodes: usize, css: &str) -> Vec<(String, f32)> {
    mmx_with(nodes, css, layout_limits(), mmx_view)
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// One literal expectation.
fn mmx_lit(name: &str, value: f32) -> (String, f32) {
    (name.to_owned(), value)
}

/// Child `index`'s border, padding and content edge `side` (children carry no
/// padding or border in these rows, so the three boxes share their edges).
fn mmx_x(index: usize, side: &str, value: f32) -> Vec<(String, f32)> {
    ["border", "padding", "content"]
        .into_iter()
        .map(|kind| (format!("c{index}.{kind}.{side}"), value))
        .collect()
}

/// Child `index`'s vertical edges of all three boxes.
fn mmx_y(index: usize, top: f32, bottom: f32) -> Vec<(String, f32)> {
    let mut out = mmx_x(index, "top", top);
    out.extend(mmx_x(index, "bottom", bottom));
    out
}

/// P's content bottom and the edges below it (padding-bottom 1, no border or margin),
/// and its used height from content top 11.
fn mmx_p_bottom(content_bottom: f32) -> Vec<(String, f32)> {
    vec![
        mmx_lit("p.content_bottom", content_bottom),
        mmx_lit("p.padding_bottom", content_bottom + 1.0),
        mmx_lit("p.border_bottom", content_bottom + 1.0),
        mmx_lit("p.margin_bottom", content_bottom + 1.0),
        mmx_lit("p.used_height", content_bottom - 11.0),
    ]
}

/// P placed by the explicit route (a computed height, or the 10.7 rerun): no extent.
fn mmx_p_explicit(content_bottom: f32) -> Vec<(String, f32)> {
    let mut out = mmx_p_bottom(content_bottom);
    for name in [
        "p.extent",
        "p.extent.normal_margin_bottom",
        "p.extent.selected_bottom",
        "p.extent.float_extended",
    ] {
        out.push(mmx_lit(name, 0.0));
    }
    out
}

/// `view` has the `expected` literals and every other number of `control`, bit for
/// bit; the two views publish the same names.
fn assert_mmx_view(
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

/// The N5 baseline literals (design section 6): P [10, 11, 22, 23, 23], width 100;
/// B1 border [0, 100] x [11, 12]; F [0, 10] x [16, 18]; B2 border [0, 100] x [18, 19]
/// with clearance -1.
fn assert_mmx_baseline(view: &[(String, f32)], context: &str) {
    let mut expected = vec![
        mmx_lit("p.border_top", 10.0),
        mmx_lit("p.content_top", 11.0),
        mmx_lit("p.width", 100.0),
        mmx_lit("p.passes", 1.0),
        mmx_lit("p.content_left", 0.0),
        mmx_lit("p.content_right", 100.0),
        mmx_lit("p.extent", 1.0),
        mmx_lit("p.extent.normal_margin_bottom", 22.0),
        mmx_lit("p.extent.selected_bottom", 22.0),
        mmx_lit("p.extent.float_extended", 0.0),
        mmx_lit("float.left", 0.0),
        mmx_lit("float.right", 10.0),
        mmx_lit("float.top", 16.0),
        mmx_lit("float.bottom", 18.0),
        mmx_lit("tail0.cleared", 1.0),
        mmx_lit("tail0.clearance", -1.0),
        mmx_lit("c0.used_width", 100.0),
        mmx_lit("c0.passes", 1.0),
        mmx_lit("c1.used_width", 10.0),
        mmx_lit("c1.passes", 0.0),
        mmx_lit("c2.used_width", 100.0),
    ];
    expected.extend(mmx_p_bottom(22.0));
    expected.extend(mmx_y(0, 11.0, 12.0));
    expected.extend(mmx_x(0, "right", 100.0));
    expected.extend(mmx_y(1, 16.0, 18.0));
    expected.extend(mmx_x(1, "right", 10.0));
    expected.extend(mmx_y(2, 18.0, 19.0));
    expected.extend(mmx_x(2, "right", 100.0));
    assert_mmx_view(view, view, &expected, context);
}

/// W1 (RED; M2, M8 witness): `max-width` narrows B1 by the 10.4 max rerun: used width
/// 60, margins (0, 40), `passes` 2; every vertical number stays the baseline.
/// Predecessor at a75c2066: the producer refuses B1's `max-width` with
/// `Evaluation(Unsupported { property: max-width, target: Element, reason:
/// NondefaultMinMax })`.
#[test]
fn mmx_red_w1_max_width_narrows_b1() {
    let css = mmx_css(5, "#b1 { max-width:60px; }");
    let view = mmx_layout(5, &css);
    let control = mmx_layout(5, &mmx_css(5, ""));
    assert_mmx_baseline(&control, "N5 control");
    let mut expected = vec![
        mmx_lit("c0.used_width", 60.0),
        mmx_lit("c0.margin.right", 40.0),
        mmx_lit("c0.passes", 2.0),
    ];
    expected.extend(mmx_x(0, "right", 60.0));
    assert_mmx_view(&view, &control, &expected, &css);
}

/// W2 (RED; M18, M22 witness): P's Auto span 11 is below `min-height` 30, so the
/// explicit route reruns: P [10, 11, 41, 42, 42], used height 30, no extent; the
/// children keep the baseline. Predecessor at a75c2066: the producer refuses P's
/// `min-height` (after one conversion) with `Evaluation(Unsupported { property:
/// min-height, target: Element, reason: NondefaultMinMax })`.
#[test]
fn mmx_red_w2_min_height_reruns_the_parent() {
    let css = mmx_css(5, "#p { min-height:30px; }");
    let view = mmx_layout(5, &css);
    let control = mmx_layout(5, &mmx_css(5, ""));
    assert_mmx_view(&view, &control, &mmx_p_explicit(41.0), &css);
}

// FLIP rows: they read the constraint facts.

/// Child selectors by projected source, starting at source 2.
const MMX_CHILDREN: [&str; 5] = ["b1", "f", "b2", "b3", "b4"];

/// An admitted row: `fragment` gives the `expected` literals and every other number of
/// `control`'s layout.
fn assert_mmx_row(nodes: usize, fragment: &str, control: &str, expected: &[(String, f32)]) {
    let css = mmx_css(nodes, fragment);
    let view = mmx_layout(nodes, &css);
    assert_mmx_view(
        &view,
        &mmx_layout(nodes, &mmx_css(nodes, control)),
        expected,
        &css,
    );
}

/// Layout-error-asserting helper (the facts must succeed): panics on a layout.
fn mmx_error(nodes: usize, css: &str) -> SiblingLayoutError {
    match mmx_with(nodes, css, layout_limits(), |_| ()) {
        Ok(()) => panic!("{css}: expected a layout error, got a layout"),
        Err(error) => error,
    }
}

fn assert_mmx_overflow(fragment: &str, source: usize) {
    let css = mmx_css(5, fragment);
    let error = mmx_error(5, &css);
    assert!(
        matches!(error, SiblingLayoutError::ParentAutoOverflow { source: s } if s == source),
        "{css}: {error:?}"
    );
}

/// The Sequence fact rows, the fact receipt and every source's original index; the
/// producer must succeed.
fn mmx_facts(
    nodes: usize,
    css: &str,
) -> (Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt, Vec<usize>) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| panic!("{css}: producer error {}", mmx_describe(&error)));
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

/// The producer's error and every source's original index; panics when the facts
/// succeed.
fn mmx_fact_error(nodes: usize, css: &str) -> (SiblingBoxFactsError, Vec<usize>) {
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

/// `error` is the producer's `Unsupported` refusal of the element longhand `name` at
/// `original`, with `reason`.
fn assert_mmx_refusal(
    error: &SiblingBoxFactsError,
    expected: (usize, &str, BoxFactUnsupported),
    context: &str,
) {
    let (original, name, reason) = expected;
    assert_eq!(
        mmx_describe(error),
        format!(
            "Evaluation(Unsupported {{ original: {original}, property: {name}, target: \
             Element, reason: {reason:?} }})"
        ),
        "{context}"
    );
}

fn mmx_abs(pixels: f32, origin: BoxFactOrigin) -> AbsoluteBoxFact {
    AbsoluteBoxFact {
        pixels,
        rounded_to_zero: false,
        origin,
    }
}

/// The initial constraint facts: min +0 and max `none`, both `Initial`.
const MMX_MIN_INITIAL: AbsoluteBoxFact = AbsoluteBoxFact {
    pixels: 0.0,
    rounded_to_zero: false,
    origin: BoxFactOrigin::Initial,
};
const MMX_MAX_INITIAL: BoxMaxFact = BoxMaxFact::None {
    origin: BoxFactOrigin::Initial,
};

/// The four constraint facts of `row`, with the pixel bits of the two mins.
fn mmx_constraints(
    row: &ClearanceBoxFact,
) -> (
    AbsoluteBoxFact,
    AbsoluteBoxFact,
    BoxMaxFact,
    BoxMaxFact,
    [u32; 2],
) {
    (
        row.min_width.absolute().unwrap(),
        row.min_height.absolute().unwrap(),
        row.max_width,
        row.max_height,
        [
            row.min_width.absolute().unwrap().pixels.to_bits(),
            row.min_height.absolute().unwrap().pixels.to_bits(),
        ],
    )
}

/// W3 (M1, M9 witness): `min-width` widens B1 past P's content box; the ltr
/// overconstraint gives margin-right -20 (P `hidden` admits it).
#[test]
fn mmx_w3_min_width_overconstrains_b1() {
    let mut expected = vec![
        mmx_lit("c0.used_width", 120.0),
        mmx_lit("c0.margin.right", -20.0),
        mmx_lit("c0.passes", 2.0),
    ];
    expected.extend(mmx_x(0, "right", 120.0));
    assert_mmx_row(5, "#b1 { min-width:120px; }", "", &expected);
}

/// W4: width 80 is above `max-width` 60 (rerun 60), which is below `min-width` 70
/// (rerun 70): width 70, margin-right 30, `passes` 3 (control: width 80).
#[test]
fn mmx_w4_max_then_min_width() {
    let mut expected = vec![
        mmx_lit("c0.used_width", 70.0),
        mmx_lit("c0.margin.right", 30.0),
        mmx_lit("c0.passes", 3.0),
    ];
    expected.extend(mmx_x(0, "right", 70.0));
    assert_mmx_row(
        5,
        "#b1 { width:80px; max-width:60px; min-width:70px; }",
        "#b1 { width:80px; }",
        &expected,
    );
}

/// W5: under an `rtl` P the overconstrained free margin is the left one: margins
/// (40, 0), border [40, 100].
#[test]
fn mmx_w5_rtl_parent_moves_the_free_margin_left() {
    let mut expected = vec![
        mmx_lit("c0.used_width", 60.0),
        mmx_lit("c0.margin.left", 40.0),
        mmx_lit("c0.passes", 2.0),
    ];
    expected.extend(mmx_x(0, "left", 40.0));
    assert_mmx_row(
        5,
        "#p { direction:rtl; } #b1 { max-width:60px; }",
        "#p { direction:rtl; }",
        &expected,
    );
}

/// W6 (M7 witness): P's `max-width` 50 narrows P (margin-right 50, `passes` 2) and
/// every normal child's containing block; F keeps [0, 10].
#[test]
fn mmx_w6_parent_max_width_narrows_every_normal() {
    assert_mmx_row(5, "#p { max-width:50px; }", "", &mmx_w6_expected());
}

fn mmx_w6_expected() -> Vec<(String, f32)> {
    let mut expected = vec![
        mmx_lit("p.width", 50.0),
        mmx_lit("p.margin_right", 50.0),
        mmx_lit("p.passes", 2.0),
        mmx_lit("p.content_right", 50.0),
        mmx_lit("c0.used_width", 50.0),
        mmx_lit("c2.used_width", 50.0),
    ];
    expected.extend(mmx_x(0, "right", 50.0));
    expected.extend(mmx_x(2, "right", 50.0));
    expected
}

/// W7: auto horizontal margins center a box narrowed by `max-width`: margins 20/20,
/// border [20, 80] (control: the same auto margins without `max-width`).
#[test]
fn mmx_w7_auto_margins_center_a_max_width_box() {
    let mut expected = vec![
        mmx_lit("c0.used_width", 60.0),
        mmx_lit("c0.margin.left", 20.0),
        mmx_lit("c0.margin.right", 20.0),
        mmx_lit("c0.passes", 2.0),
    ];
    expected.extend(mmx_x(0, "left", 20.0));
    expected.extend(mmx_x(0, "right", 80.0));
    assert_mmx_row(
        5,
        "#b1 { margin-left:auto; margin-right:auto; max-width:60px; }",
        "#b1 { margin-left:auto; margin-right:auto; }",
        &expected,
    );
}

/// The computed width and height facts each child publishes.
fn mmx_computed(nodes: usize, css: &str) -> Vec<(BoxLengthFact, BoxLengthFact)> {
    mmx_with(nodes, css, layout_limits(), |layout| {
        layout
            .children()
            .iter()
            .map(|child| (child.computed_width(), child.computed_height()))
            .collect()
    })
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// F1 (M10 witness): F's used width is its computed width after the max rerun: 6,
/// placed [0, 6] x [16, 18]; `computed_width()` stays Absolute 10.
#[test]
fn mmx_f1_float_max_width() {
    let fragment = "#f { max-width:6px; }";
    let mut expected = vec![mmx_lit("c1.used_width", 6.0), mmx_lit("float.right", 6.0)];
    expected.extend(mmx_x(1, "right", 6.0));
    assert_mmx_row(5, fragment, "", &expected);
    let css = mmx_css(5, fragment);
    let BoxLengthFact::Absolute(width) = mmx_computed(5, &css)[1].0 else {
        panic!("{css}: F computed width");
    };
    assert_eq!(width.pixels, 10.0, "{css}");
}

/// F2 (M11 witness): 10 > 6 gives 6, then 6 < 12 gives 12: `min-width` wins.
#[test]
fn mmx_f2_float_min_width_wins() {
    let mut expected = vec![mmx_lit("c1.used_width", 12.0), mmx_lit("float.right", 12.0)];
    expected.extend(mmx_x(1, "right", 12.0));
    assert_mmx_row(5, "#f { min-width:12px; max-width:6px; }", "", &expected);
}

/// The H1 literals: B1's used height 3, border [11, 14]; F [0, 10] x [18, 20]; B2
/// border [20, 21] (clearance -1); P's Auto content bottom 24.
fn mmx_h1_expected() -> Vec<(String, f32)> {
    let mut expected = vec![
        mmx_lit("c0.used_height", 3.0),
        mmx_lit("float.top", 18.0),
        mmx_lit("float.bottom", 20.0),
        mmx_lit("p.extent.normal_margin_bottom", 24.0),
        mmx_lit("p.extent.selected_bottom", 24.0),
    ];
    expected.extend(mmx_y(0, 11.0, 14.0));
    expected.extend(mmx_y(1, 18.0, 20.0));
    expected.extend(mmx_y(2, 20.0, 21.0));
    expected.extend(mmx_p_bottom(24.0));
    expected
}

/// H1 (M12 witness): B1's explicit height 1 below `min-height` 3 becomes 3, which
/// moves F to [16 + 2, 20] and B2 to [20, 21] (clearance -1); P [10, 11, 24, 25, 25].
#[test]
fn mmx_h1_b1_min_height_moves_followers() {
    assert_mmx_row(5, "#b1 { min-height:3px; }", "", &mmx_h1_expected());
}

/// H2: `max-height` 0 makes B2's used height +0, which the core refuses.
#[test]
fn mmx_h2_max_height_zero_is_nonpositive() {
    let css = mmx_css(5, "#b2 { max-height:0px; }");
    let error = mmx_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 4,
                    reason: OwnerProfileReason::NonpositiveBlockHeight,
                }
            ))
        ),
        "{css}: {error:?}"
    );
}

/// The B2 geometry of H3, H5 and H9: used height 2, border [18, 20], margin box
/// [15, 23]; P's Auto content bottom 23.
fn mmx_b2_two_expected() -> Vec<(String, f32)> {
    let mut expected = vec![
        mmx_lit("c2.used_height", 2.0),
        mmx_lit("p.extent.normal_margin_bottom", 23.0),
        mmx_lit("p.extent.selected_bottom", 23.0),
    ];
    expected.extend(mmx_y(2, 18.0, 20.0));
    expected.extend(mmx_p_bottom(23.0));
    expected
}

/// The view of `fragment` equals the view of the explicit twin `#b2 { height:2px; }`
/// and has the literals of `mmx_b2_two_expected()`; B2's computed height is `computed`.
fn assert_mmx_b2_two(fragment: &str, computed: BoxLengthFact) {
    assert_mmx_row(5, fragment, "", &mmx_b2_two_expected());
    assert_mmx_row(5, fragment, "#b2 { height:2px; }", &[]);
    let css = mmx_css(5, fragment);
    assert_eq!(mmx_computed(5, &css)[2].1, computed, "{css}");
}

/// H3 (M13 witness, Q-3): an Auto normal with a positive `min-height` is admitted with
/// that used height (10.6.3 tentative +0, then 10.7); `computed_height()` stays Auto.
#[test]
fn mmx_h3_auto_normal_with_min_height_is_admitted() {
    assert_mmx_b2_two(
        "#b2 { height:auto; min-height:2px; }",
        BoxLengthFact::Auto {
            origin: BoxFactOrigin::Specified,
        },
    );
}

/// H4 (M14 witness): an Auto normal whose `min-height` is zero stays `HeightKind`.
#[test]
fn mmx_h4_auto_normal_without_min_height_stays_height_kind() {
    let css = mmx_css(5, "#b2 { height:auto; max-height:5px; }");
    let error = mmx_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                projected: 4,
                property: "height",
                reason: CapturedClearanceUnsupported::HeightKind,
            })
        ),
        "{css}: {error:?}"
    );
}

/// H5: a zero computed height raised by `min-height` 2 has the H3 geometry;
/// `computed_height()` stays Absolute 0.
#[test]
fn mmx_h5_zero_height_normal_with_min_height() {
    assert_mmx_b2_two(
        "#b2 { height:0px; min-height:2px; }",
        BoxLengthFact::Absolute(mmx_abs(0.0, BoxFactOrigin::Specified)),
    );
}

/// H6 (M15 witness): an Auto F with `min-height` 5 is placed [0, 10] x [16, 21];
/// B2's clearance is 2 (21 - 19 against -3), border [21, 22]; P bottom 25.
#[test]
fn mmx_h6_auto_float_min_height() {
    let mut expected = vec![
        mmx_lit("c1.used_height", 5.0),
        mmx_lit("float.bottom", 21.0),
        mmx_lit("tail0.clearance", 2.0),
        mmx_lit("p.extent.normal_margin_bottom", 25.0),
        mmx_lit("p.extent.selected_bottom", 25.0),
    ];
    expected.extend(mmx_x(1, "bottom", 21.0));
    expected.extend(mmx_y(2, 21.0, 22.0));
    expected.extend(mmx_p_bottom(25.0));
    assert_mmx_row(5, "#f { height:auto; min-height:5px; }", "", &expected);
}

/// H7 (Q-6): an Auto F whose constrained height is +0 keeps the Auto category; its
/// view equals the CFH control `#f { height:auto; }`. B2's margin-top 5 keeps its
/// hypothetical border top (17) past F's bottom 16 in both: with the baseline 3 it
/// would equal 16, which the core refuses (`UnsupportedEquality` at source 4).
#[test]
fn mmx_h7_auto_float_zero_constraint_keeps_auto() {
    assert_mmx_row(
        5,
        "#f { height:auto; max-height:1px; } #b2 { margin-top:5px; }",
        "#f { height:auto; } #b2 { margin-top:5px; }",
        &[mmx_lit("c1.used_height", 0.0)],
    );
    let css = mmx_css(5, "#f { height:auto; max-height:1px; }");
    let error = mmx_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ),
        "{css}: {error:?}"
    );
}

/// The H8 literals: F's used height 1, placed [0, 10] x [16, 17]; B2's clearance -2,
/// border [17, 18]; P's Auto content bottom 21.
fn mmx_h8_expected() -> Vec<(String, f32)> {
    let mut expected = vec![
        mmx_lit("c1.used_height", 1.0),
        mmx_lit("float.bottom", 17.0),
        mmx_lit("tail0.clearance", -2.0),
        mmx_lit("p.extent.normal_margin_bottom", 21.0),
        mmx_lit("p.extent.selected_bottom", 21.0),
    ];
    expected.extend(mmx_x(1, "bottom", 17.0));
    expected.extend(mmx_y(2, 17.0, 18.0));
    expected.extend(mmx_p_bottom(21.0));
    expected
}

/// H8: F's explicit height 2 above `max-height` 1 becomes 1: F [16, 17]; B2's
/// clearance -2 (17 - 19 against -3), border [17, 18]; P [10, 11, 21, 22, 22].
#[test]
fn mmx_h8_float_max_height() {
    assert_mmx_row(5, "#f { max-height:1px; }", "", &mmx_h8_expected());
}

/// H9 (M11 witness): 4 > 1 gives 1, then 1 < 2 gives 2: the H3 geometry.
#[test]
fn mmx_h9_normal_max_then_min_height() {
    assert_mmx_b2_two(
        "#b2 { height:4px; max-height:1px; min-height:2px; }",
        BoxLengthFact::Absolute(mmx_abs(4.0, BoxFactOrigin::Specified)),
    );
}

/// P2 (M17 witness): P's Auto span 11 is above `max-height` 5: rerun, P [10, 11, 16,
/// 17, 17]; the children keep the baseline (P `hidden`).
#[test]
fn mmx_p2_parent_auto_max_height() {
    assert_mmx_row(5, "#p { max-height:5px; }", "", &mmx_p_explicit(16.0));
}

/// P3 (M25 witness): under P `auto` the constrained content box [11, 16] holds B1
/// (border [11, 12], margin box [11, 16]) but not F (border bottom 18).
#[test]
fn mmx_p3_parent_auto_overflow_after_max_height() {
    assert_mmx_overflow("#p { overflow:auto; max-height:5px; }", 3);
}

/// P4 (M16 witness): explicit height 20 above `max-height` 15 gives 15, below
/// `min-height` 18 gives 18: P [10, 11, 29, 30, 30] (control: height 20).
#[test]
fn mmx_p4_parent_explicit_max_then_min_height() {
    assert_mmx_row(
        5,
        "#p { height:20px; max-height:15px; min-height:18px; }",
        "#p { height:20px; }",
        &mmx_p_bottom(29.0),
    );
}

/// P5 (M21 witness): limits equal to the span 11 apply no rerun: the tentative Auto
/// placement is kept bit for bit (extent present).
#[test]
fn mmx_p5_parent_limits_equal_to_span_keep_the_tentative() {
    let css = mmx_css(5, "#p { min-height:11px; max-height:11px; }");
    let view = mmx_layout(5, &css);
    assert_mmx_baseline(&view, &css);
    assert_mmx_row(5, "#p { min-height:11px; max-height:11px; }", "", &[]);
}

/// P6 (M18 witness): signed route: the extrema (11, 16) and F's bottom 18 give the
/// tentative bottom 18 (span 7 < 30), then the rerun: P [10, 11, 41, 42, 42].
#[test]
fn mmx_p6_signed_route_min_height() {
    let control = "#b2 { margin-bottom:-3px; }";
    let control_view = mmx_layout(5, &mmx_css(5, control));
    let mut tentative = mmx_p_bottom(18.0);
    tentative.extend([
        mmx_lit("p.extent", 1.0),
        mmx_lit("p.extent.normal_margin_bottom", 16.0),
        mmx_lit("p.extent.selected_bottom", 18.0),
        mmx_lit("p.extent.float_extended", 1.0),
    ]);
    assert_mmx_view(&control_view, &control_view, &tentative, control);
    assert_mmx_row(
        5,
        "#b2 { margin-bottom:-3px; } #p { min-height:30px; }",
        control,
        &mmx_p_explicit(41.0),
    );
}

/// P7: signed route: the tentative span 7 is above `max-height` 4: P [10, 11, 15, 16,
/// 16].
#[test]
fn mmx_p7_signed_route_max_height() {
    assert_mmx_row(
        5,
        "#b2 { margin-bottom:-3px; } #p { max-height:4px; }",
        "#b2 { margin-bottom:-3px; }",
        &mmx_p_explicit(15.0),
    );
}

/// C1: under P `auto`, `min-width` 120 puts B1's border right at 120 > 100.
#[test]
fn mmx_c1_min_width_overflows_an_auto_parent() {
    assert_mmx_overflow("#p { overflow:auto; } #b1 { min-width:120px; }", 2);
}

/// R-1 (fence): a B1 narrowed by `max-width` keeps its full-width margin box, so a
/// relative `left` shift overflows P `auto` (margin box right 60 + 40 + 5 = 105).
#[test]
fn mmx_r1_shifted_narrowed_b1_overflows() {
    assert_mmx_overflow(
        "#p { overflow:auto; } #b1 { position:relative; left:5px; max-width:60px; }",
        2,
    );
}

/// FA1 (M5 witness): B1's specified `min-width` 3 and `max-height: none` carry
/// `Specified`; its other two constraints stay initial; the layout is the baseline.
#[test]
fn mmx_fa1_constraint_facts_and_provenance() {
    let fragment = "#b1 { min-width:3px; max-height:none; }";
    let css = mmx_css(5, fragment);
    let (rows, _, _) = mmx_facts(5, &css);
    assert_eq!(
        mmx_constraints(&rows[2]),
        (
            mmx_abs(3.0, BoxFactOrigin::Specified),
            MMX_MIN_INITIAL,
            MMX_MAX_INITIAL,
            BoxMaxFact::None {
                origin: BoxFactOrigin::Specified,
            },
            [0x40400000, 0x00000000],
        ),
        "{css}"
    );
    assert_mmx_row(5, fragment, "", &[]);
}

/// FA2 (M6 witness): `max-width: inherit` copies P's computed 50 with origin
/// `Inherited`; the layout is W6's.
#[test]
fn mmx_fa2_inherit_copies_the_parent_constraint() {
    let fragment = "#p { max-width:50px; } #b1 { max-width:inherit; }";
    let css = mmx_css(5, fragment);
    let (rows, _, originals) = mmx_facts(5, &css);
    assert_eq!(
        rows[2].max_width,
        BoxMaxFact::Absolute(mmx_abs(
            50.0,
            BoxFactOrigin::Inherited {
                original_parent: originals[1],
            }
        )),
        "{css}"
    );
    assert_mmx_row(5, fragment, "", &mmx_w6_expected());
}

/// FA3: `em` and `ex` keep their refusals on the Sequence rows (PPM and PHT admit
/// constraint percentages).
#[test]
fn mmx_fa3_percentage_and_relative_constraints_are_refused() {
    for (fragment, name, reason) in [
        (
            "#b1 { max-height:1ex; }",
            "max-height",
            BoxFactUnsupported::RelativeLength,
        ),
        (
            "#b1 { min-height:1em; }",
            "min-height",
            BoxFactUnsupported::RelativeLength,
        ),
    ] {
        let css = mmx_css(5, fragment);
        let (error, originals) = mmx_fact_error(5, &css);
        assert_mmx_refusal(&error, (originals[2], name, reason), &css);
    }
}

/// FA4: an unrepresentable `max-height` gives the `margin-top` twin's conversion
/// error.
#[test]
fn mmx_fa4_unrepresentable_max_is_the_margin_error() {
    let value = "400000000000000000000000000000000000000px";
    let css = mmx_css(5, &format!("#b1 {{ max-height:{value}; }}"));
    let twin = mmx_css(5, &format!("#b1 {{ margin-top:{value}; }}"));
    let (error, originals) = mmx_fact_error(5, &css);
    let (twin_error, _) = mmx_fact_error(5, &twin);
    let (
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length {
            original,
            property,
            error: length_error,
        }),
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length {
            original: twin_original,
            property: twin_property,
            error: twin_length_error,
        }),
    ) = (&error, &twin_error)
    else {
        panic!("{css}: {error:?} against {twin_error:?}");
    };
    assert_eq!(*original, originals[2], "{css}");
    assert_eq!(twin_original, original, "{css}");
    assert_eq!(property.descriptor().name, "max-height", "{css}");
    assert_eq!(twin_property.descriptor().name, "margin-top", "{twin}");
    assert_eq!(length_error, twin_length_error, "{css}");
}

/// FA5: O's constraints are captured and unused.
#[test]
fn mmx_fa5_outer_constraints_are_captured_and_unused() {
    let fragment = "#o { min-width:500px; max-height:1px; }";
    let css = mmx_css(5, fragment);
    let (rows, _, _) = mmx_facts(5, &css);
    assert_eq!(
        (rows[0].min_width.absolute().unwrap(), rows[0].max_height),
        (
            mmx_abs(500.0, BoxFactOrigin::Specified),
            BoxMaxFact::Absolute(mmx_abs(1.0, BoxFactOrigin::Specified)),
        ),
        "{css}"
    );
    assert_mmx_row(5, fragment, "", &[]);
}

/// FA6: an absolute `max-width` is one counted conversion; `none` (the same rule
/// count) converts nothing; the work and the allowance are unchanged.
#[test]
fn mmx_fa6_absolute_max_is_one_conversion() {
    let css = mmx_css(5, "#b1 { max-width:60px; }");
    let (_, receipt, _) = mmx_facts(5, &css);
    let (_, none, _) = mmx_facts(5, &mmx_css(5, "#b1 { max-width:none; }"));
    let (_, baseline, _) = mmx_facts(5, &mmx_css(5, ""));
    assert_eq!(none.numeric_calls, baseline.numeric_calls, "{css}: none");
    assert_eq!(
        receipt.numeric_calls,
        none.numeric_calls + 1,
        "{css}: numeric calls"
    );
    assert_eq!(receipt.work, none.work, "{css}: work");
    assert_eq!(
        receipt.numeric_work, baseline.numeric_work,
        "{css}: allowance"
    );
}

/// The Legacy facts of a five-source capture of `css`.
fn mmx_legacy_rows(css: &str) -> Vec<ClearanceBoxFact> {
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
        let legacy = evaluate_clearance_box_facts(&legacy_projection, display, fact_limits())
            .unwrap_or_else(|error| panic!("{css}: Legacy facts error {error:?}"));
        out = Some(
            (0..5)
                .map(|projected| *legacy.row(projected).unwrap())
                .collect(),
        );
    });
    out.unwrap()
}

/// Legacy guard (Q-9, C-3; M26 witness): the Legacy `inherit` of a constraint stays a
/// no-op, and a specified `0px`/`none` is not stored: every Legacy row keeps the
/// initial constraints.
#[test]
fn mmx_legacy_inherit_keeps_initial_constraints() {
    for fragment in [
        "#b1 { min-width:inherit; max-height:inherit; }",
        "#b1 { min-height:0px; max-width:none; } #p { min-width:inherit; }",
    ] {
        let css = mmx_css(5, fragment);
        for (projected, row) in mmx_legacy_rows(&css).iter().enumerate() {
            assert_eq!(
                mmx_constraints(row),
                (
                    MMX_MIN_INITIAL,
                    MMX_MIN_INITIAL,
                    MMX_MAX_INITIAL,
                    MMX_MAX_INITIAL,
                    [0, 0],
                ),
                "{css}: Legacy row {projected}"
            );
        }
    }
}

/// Sequence twin of the Legacy guard (Q-9): the same `inherit` copies P's
/// initial constraints with origin `Inherited`.
#[test]
fn mmx_sequence_inherit_copies_parent_constraints() {
    let css = mmx_css(5, "#b1 { min-width:inherit; max-height:inherit; }");
    let (rows, _, originals) = mmx_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(
        mmx_constraints(&rows[2]),
        (
            mmx_abs(0.0, inherited),
            MMX_MIN_INITIAL,
            MMX_MAX_INITIAL,
            BoxMaxFact::None { origin: inherited },
            [0, 0],
        ),
        "{css}"
    );
}

/// MX-A (M7, M8, M9, M27 witness): N5, N6 and N7, every source 1..N-1, each of
/// `min-width:120px` and `max-width:60px` alone (30 rows). P's constraint changes P's
/// width and every normal child's containing width; a normal child's changes only its
/// own width (margin-right 100 - width, `passes` 2); F's follows the float rule.
#[test]
fn mmx_mx_a_width_matrix() {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for index in 0..nodes - 1 {
            for (declaration, width) in [("min-width:120px", 120.0), ("max-width:60px", 60.0)] {
                let mut expected = Vec::new();
                let selector = if index == 0 {
                    expected.extend([
                        mmx_lit("p.width", width),
                        mmx_lit("p.margin_right", 100.0 - width),
                        mmx_lit("p.passes", 2.0),
                        mmx_lit("p.content_right", width),
                    ]);
                    for child in (0..nodes - 2).filter(|&child| child != 1) {
                        expected.push(mmx_lit(&format!("c{child}.used_width"), width));
                        expected.extend(mmx_x(child, "right", width));
                    }
                    "p"
                } else if index == 2 {
                    // F: width 10 is below `max-width` 60 (no rerun), or widened to 120.
                    let used = if declaration.starts_with("min") {
                        width.max(10.0)
                    } else {
                        width.min(10.0)
                    };
                    expected.push(mmx_lit("c1.used_width", used));
                    expected.push(mmx_lit("float.right", used));
                    expected.extend(mmx_x(1, "right", used));
                    "f"
                } else {
                    let child = index - 1;
                    expected.extend([
                        mmx_lit(&format!("c{child}.used_width"), width),
                        mmx_lit(&format!("c{child}.margin.right"), 100.0 - width),
                        mmx_lit(&format!("c{child}.passes"), 2.0),
                    ]);
                    expected.extend(mmx_x(child, "right", width));
                    MMX_CHILDREN[child]
                };
                assert_mmx_row(
                    nodes,
                    &format!("#{selector} {{ {declaration}; }}"),
                    "",
                    &expected,
                );
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 30);
}

/// MX-B: N5, P and each child with a `min-height` above and a `max-height` below its
/// baseline height (P Auto 11, B1 1, F 2, B2 1), by `constrain` and the clearance
/// model (B2's clearance is max(F bottom - 19, -3) in the baseline frame).
#[test]
fn mmx_mx_b_height_matrix() {
    let b1_half = {
        let mut expected = vec![
            mmx_lit("c0.used_height", 0.5),
            mmx_lit("float.top", 15.5),
            mmx_lit("float.bottom", 17.5),
            mmx_lit("p.extent.normal_margin_bottom", 21.5),
            mmx_lit("p.extent.selected_bottom", 21.5),
        ];
        expected.extend(mmx_y(0, 11.0, 11.5));
        expected.extend(mmx_y(1, 15.5, 17.5));
        expected.extend(mmx_y(2, 17.5, 18.5));
        expected.extend(mmx_p_bottom(21.5));
        expected
    };
    let f_four = {
        let mut expected = vec![
            mmx_lit("c1.used_height", 4.0),
            mmx_lit("float.bottom", 20.0),
            mmx_lit("tail0.clearance", 1.0),
            mmx_lit("p.extent.normal_margin_bottom", 24.0),
            mmx_lit("p.extent.selected_bottom", 24.0),
        ];
        expected.extend(mmx_x(1, "bottom", 20.0));
        expected.extend(mmx_y(2, 20.0, 21.0));
        expected.extend(mmx_p_bottom(24.0));
        expected
    };
    let b2_half = {
        let mut expected = vec![
            mmx_lit("c2.used_height", 0.5),
            mmx_lit("p.extent.normal_margin_bottom", 21.5),
            mmx_lit("p.extent.selected_bottom", 21.5),
        ];
        expected.extend(mmx_y(2, 18.0, 18.5));
        expected.extend(mmx_p_bottom(21.5));
        expected
    };
    for (fragment, expected) in [
        ("#p { min-height:20px; }", mmx_p_explicit(31.0)),
        ("#p { max-height:6px; }", mmx_p_explicit(17.0)),
        ("#b1 { min-height:3px; }", mmx_h1_expected()),
        ("#b1 { max-height:0.5px; }", b1_half),
        ("#f { min-height:4px; }", f_four),
        ("#f { max-height:1px; }", mmx_h8_expected()),
        ("#b2 { min-height:2px; }", mmx_b2_two_expected()),
        ("#b2 { max-height:0.5px; }", b2_half),
    ] {
        assert_mmx_row(5, fragment, "", &expected);
    }
}

/// The work fixture: nondefault absolute constraints on every source; no rerun.
const MMX_WORK_FRAGMENT: &str = "#o { min-width:1px; max-width:500px; min-height:1px; \
                                 max-height:500px; } \
                                 #p { min-width:1px; max-width:500px; min-height:1px; \
                                 max-height:500px; } \
                                 #b1, #f, #b2, #b3, #b4 { min-width:1px; max-width:500px; \
                                 min-height:1px; max-height:50px; }";

/// Facade and core work (M24 witness): exact and one short at N5, N6 and N7 with every
/// constraint nondefault; the facade charge is `471 + 32N + 526(N-2) + 313(N-3)`.
#[test]
fn mmx_work_thresholds_with_constraints() {
    for (nodes, facade, core) in [(5, 2835, 1249), (6, 3706, 1501), (7, 4577, 1754)] {
        assert_eq!(
            facade,
            471 + 32 * nodes + 526 * (nodes - 2) + 313 * (nodes - 3)
        );
        let css = mmx_css(nodes, MMX_WORK_FRAGMENT);
        assert_mmx_view(
            &mmx_layout(nodes, &css),
            &mmx_layout(nodes, &mmx_css(nodes, "")),
            &[],
            &css,
        );
        let limits = |work: usize, core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = mmx_with(nodes, &css, limits(facade, core), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert!(
            matches!(
                mmx_with(nodes, &css, limits(facade - 1, core), |_| ()),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                mmx_with(nodes, &css, limits(facade, core - 1), |_| ()),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}
