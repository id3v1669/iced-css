//! ISP (gap G10): a relatively positioned Sequence row whose `top`, `right`, `bottom` or
//! `left` is `inherit` and whose parent row is not relatively positioned takes the
//! parent's computed offset (CSS2.2 6.2.1: `inherit` takes the parent's computed value;
//! 6.1.2: a computed value exists even where the property does not apply; 9.3.2: the
//! computed `top` is `auto`, an absolute length, or a percentage as specified). A static O
//! or P keeps its specified offsets, unconverted; the inheriting child computes the side
//! against the parent's own resolver (an `em` against the parent's font size), one counted
//! conversion per inheriting length side. A side outside the supported subset (`ex`, `em`
//! without the font entry, an unrepresentable length, a percentage outside the bounded
//! representation, the same through a static P from O) keeps `InheritedOffset` at the
//! child.
//!
//! A child module of `outer_auto_heights` (ISP design section 6): it reuses
//! `assert_pct_row`, `pct_css`, `pct_lit`, `pct_facts`, `pct_fact`, `pct_fact_error`,
//! `pct_sequence_describe`, `ptb_offset`, `pht_error_in`, `pht_refusal`, `eml_row`,
//! `eml_error` and `PCT_OUTER` through `use super::*`. The RED part (`isp_refusal`,
//! `isp_y`, I1 and the guards IG1-IG6) compiles against the OAH commit da8dab1b: it
//! names no new item and reads only published geometry, facts and errors. Fixture:
//! `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with padding-top and
//! padding-bottom 1: content [11, 22], so P's used content height is 11 and O's 13), N5:
//! projected O 0, P 1, B1 2, F 3, B2 4, originals [4, 3, 0, 1, 2]. Each admitted row
//! compares the listed literals and every other published number with its control (the
//! same CSS without the inheriting declarations), and every number with its absolute twin
//! (the inherited value written out), bit for bit.

use super::*;

/// The producer error of the N5 `fragment` as text (`pct_sequence_describe`); the facts
/// must fail.
fn isp_refusal(fragment: &str) -> String {
    pct_sequence_describe(&pct_fact_error(5, &pct_css(5, fragment)).0)
}

/// The refusal of B1's (original 0) `top:inherit` (REL, ISP).
const ISP_INHERITED_TOP: &str = "Evaluation(Unsupported { original: 0, property: top, \
                                 target: Element, reason: InheritedOffset })";

/// The used `top` literal of the box `name` (`p`, `c0` B1, `c1` F, `c2` B2) and its
/// `bottom`, `-top` (9.4.3).
fn isp_y(name: &str, top: f32) -> [(String, f32); 2] {
    [
        pct_lit(&format!("{name}.offset.top"), top),
        pct_lit(&format!("{name}.offset.bottom"), -top),
    ]
}

/// I1 (RED; MI1, MI6, MI7 witness): B1's `top:inherit` under the static P's `top:3px`
/// takes P's computed 3px, so B1's offsets are (0, 0, 3, -3); every other number is the
/// control's. Predecessor (OAH commit da8dab1b): the producer refuses it,
/// `Evaluation(Unsupported { original: 0, property: top, target: Element, reason:
/// InheritedOffset })`.
#[test]
fn isp_red_i1_inherit_from_a_static_parent_copies_its_top() {
    assert_pct_row(
        "#p { top:3px; } #b1 { position:relative; top:inherit; }",
        "#p { top:3px; } #b1 { position:relative; }",
        "#p { top:3px; } #b1 { position:relative; top:3px; }",
        &isp_y("c0", 3.0),
    );
}

/// IG1 (RED guard): P's `top:2em` has no computed value here without the font entry, so
/// B1's `top:inherit` keeps `InheritedOffset`. Passes before and after ISP.
#[test]
fn isp_red_guard_ig1_an_em_parent_without_the_font_entry_stays_refused() {
    assert_eq!(
        isp_refusal("#p { top:2em; } #b1 { position:relative; top:inherit; }"),
        ISP_INHERITED_TOP
    );
}

/// IG2 (RED guard): P's `top:1ex` (`ex` stays refused, EM-4) keeps B1's `InheritedOffset`.
/// Passes before and after ISP.
#[test]
fn isp_red_guard_ig2_an_ex_parent_stays_refused() {
    assert_eq!(
        isp_refusal("#p { top:1ex; } #b1 { position:relative; top:inherit; }"),
        ISP_INHERITED_TOP
    );
}

/// IG3 (RED guard): P's unrepresentable `top` (the E14 conversion error, swallowed) keeps
/// B1's `InheritedOffset`. Passes before and after ISP.
#[test]
fn isp_red_guard_ig3_an_unrepresentable_parent_stays_refused() {
    assert_eq!(
        isp_refusal(
            "#p { top:400000000000000000000000000000000000000px; } \
             #b1 { position:relative; top:inherit; }"
        ),
        ISP_INHERITED_TOP
    );
}

/// IG4 (RED guard): through a static P from O, O's `top:1ex` keeps B1's
/// `InheritedOffset`. Passes before and after ISP.
#[test]
fn isp_red_guard_ig4_an_unavailable_grandparent_stays_refused() {
    assert_eq!(
        isp_refusal("#o { top:1ex; } #p { top:inherit; } #b1 { position:relative; top:inherit; }"),
        ISP_INHERITED_TOP
    );
}

/// IG5 (RED guard): static rows publish no offset and convert none, even with offsets
/// outside the supported subset: facts Ok, only B1 holds an offset row, and
/// `numeric_calls` is that of `#b1 { position:relative; }`. Passes before and after ISP.
#[test]
fn isp_red_guard_ig5_static_rows_convert_and_publish_nothing() {
    let (_, control, _) = pct_facts(5, &pct_css(5, "#b1 { position:relative; }"));
    for fragment in [
        "#p { top:3px; } #b1 { position:relative; }",
        "#o { top:1ex; } #p { top:2em; right:400000000000000000000000000000000000000px; \
         bottom:18446744073709551616%; left:inherit; } #b1 { position:relative; }",
    ] {
        let (rows, receipt, _) = pct_facts(5, &pct_css(5, fragment));
        let published: Vec<bool> = rows.iter().map(|row| row.offset.is_some()).collect();
        assert_eq!(published, [false, false, true, false, false], "{fragment}");
        assert_eq!(receipt.numeric_calls, control.numeric_calls, "{fragment}");
    }
}

/// IG6 (RED guard): P's `top:18446744073709551616%` (2^64, outside the bounded
/// representation, `PercentagePrecision` on a relative row) keeps B1's
/// `InheritedOffset`. Passes before and after ISP.
#[test]
fn isp_red_guard_ig6_a_parent_percentage_beyond_the_bounded_representation_stays_refused() {
    assert_eq!(
        isp_refusal("#p { top:18446744073709551616%; } #b1 { position:relative; top:inherit; }"),
        ISP_INHERITED_TOP
    );
}

// ISP FLIP rows: they need the static offset table, the lazy computation and the facts of
// the inherited sides.

/// The used `left` literal of the box `name` and its `right`, `-left` (ltr: `left` wins).
fn isp_x(name: &str, left: f32) -> [(String, f32); 2] {
    [
        pct_lit(&format!("{name}.offset.left"), left),
        pct_lit(&format!("{name}.offset.right"), -left),
    ]
}

/// The facts' `numeric_calls` of the N5 `fragment` (the producer must succeed).
fn isp_calls(fragment: &str) -> usize {
    pct_facts(5, &pct_css(5, fragment)).1.numeric_calls
}

/// I2 (MI8 witness): B1's inherited `top` fact is P's 3px with origin `Inherited` from P
/// (original 3), its other sides `Initial` `auto`; P publishes no offset. The conversion
/// of P's value is the one counted call of B1's side (the control's +1, equal to the
/// twin's), and the facts work is the control's (prepaid per node).
#[test]
fn isp_i2_the_inherited_fact_and_its_conversion() {
    let fragment = "#p { top:3px; } #b1 { position:relative; top:inherit; }";
    let control = "#p { top:3px; } #b1 { position:relative; }";
    let (rows, receipt, originals) = pct_facts(5, &pct_css(5, fragment));
    assert_eq!(originals, vec![4, 3, 0, 1, 2], "{fragment}");
    let auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Initial,
    };
    let top = BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels: 3.0,
        rounded_to_zero: false,
        origin: BoxFactOrigin::Inherited { original_parent: 3 },
    });
    assert_eq!(rows[2].offset, Some([top, auto, auto, auto]), "{fragment}");
    assert_eq!(rows[1].offset, None, "{fragment}");
    let (_, control_receipt, _) = pct_facts(5, &pct_css(5, control));
    assert_eq!(
        receipt.numeric_calls,
        control_receipt.numeric_calls + 1,
        "{fragment}"
    );
    assert_eq!(
        receipt.numeric_calls,
        isp_calls("#p { top:3px; } #b1 { position:relative; top:3px; }"),
        "{fragment}"
    );
    assert_eq!(receipt.work, control_receipt.work, "{fragment}");
}

/// I3: all four sides inherit P's `top:3px; right:4px; bottom:5px; left:6px`: `left` wins
/// over `right` (ltr) and `top` over `bottom` (9.4.3), so B1's offsets are (6, -6, 3, -3);
/// four counted conversions (the control's +4).
#[test]
fn isp_i3_all_four_sides() {
    let parent = "#p { top:3px; right:4px; bottom:5px; left:6px; }";
    let fragment = format!(
        "{parent} #b1 {{ position:relative; top:inherit; right:inherit; bottom:inherit; \
         left:inherit; }}"
    );
    let control = format!("{parent} #b1 {{ position:relative; }}");
    let mut expected = isp_y("c0", 3.0).to_vec();
    expected.extend(isp_x("c0", 6.0));
    assert_pct_row(
        &fragment,
        &control,
        &format!("{parent} #b1 {{ position:relative; top:3px; right:4px; bottom:5px; left:6px; }}"),
        &expected,
    );
    assert_eq!(isp_calls(&fragment), isp_calls(&control) + 4, "{fragment}");
}

/// I3b: B1 and B2 each inherit P's `top:3px`: (0, 0, 3, -3) each, one counted conversion
/// per inheriting side (the control's +2).
#[test]
fn isp_i3b_each_inheriting_child_converts_once() {
    let fragment = "#p { top:3px; } #b1, #b2 { position:relative; top:inherit; }";
    let control = "#p { top:3px; } #b1, #b2 { position:relative; }";
    let mut expected = isp_y("c0", 3.0).to_vec();
    expected.extend(isp_y("c2", 3.0));
    assert_pct_row(
        fragment,
        control,
        "#p { top:3px; } #b1, #b2 { position:relative; top:3px; }",
        &expected,
    );
    assert_eq!(isp_calls(fragment), isp_calls(control) + 2, "{fragment}");
}

/// I3c (MI9 witness): a negative static offset is kept signed: P's `top:-3px` gives B1
/// (0, 0, -3, 3).
#[test]
fn isp_i3c_a_negative_parent_offset() {
    assert_pct_row(
        "#p { top:-3px; } #b1 { position:relative; top:inherit; }",
        "#p { top:-3px; } #b1 { position:relative; }",
        "#p { top:-3px; } #b1 { position:relative; top:-3px; }",
        &isp_y("c0", -3.0),
    );
}

/// I4 (MI10 witness): P's `top:10%` is kept as the percentage (computed value, CSS2.2
/// 9.3.2) with origin `Inherited`, without a conversion; B1's containing block is P, so
/// it resolves against P's used content height 11 (PTA): f32(1.1), not 10% of O's 13.
#[test]
fn isp_i4_a_parent_percentage_is_kept() {
    let fragment = "#p { top:10%; } #b1 { position:relative; top:inherit; }";
    let control = "#p { top:10%; } #b1 { position:relative; }";
    let (offset, _) = ptb_offset(fragment, 2);
    let inherited = BoxFactOrigin::Inherited { original_parent: 3 };
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(pct_fact("10", inherited)),
        "{fragment}"
    );
    assert_pct_row(
        fragment,
        control,
        "#p { top:10%; } #b1 { position:relative; top:1.1px; }",
        &isp_y("c0", 1.1),
    );
    assert_eq!(isp_calls(fragment), isp_calls(control), "{fragment}");
}

/// I4b (review n-4): a horizontal percentage resolves against the child's containing
/// block too: with P's `width:50px`, P's `left:10%` inherited by B1 is 5 (10% of P's
/// content width 50), not 10 (O's 100).
#[test]
fn isp_i4b_a_horizontal_parent_percentage_resolves_against_the_parent_width() {
    assert_pct_row(
        "#p { width:50px; left:10%; } #b1 { position:relative; left:inherit; }",
        "#p { width:50px; left:10%; } #b1 { position:relative; }",
        "#p { width:50px; left:10%; } #b1 { position:relative; left:5px; }",
        &isp_x("c0", 5.0),
    );
}

/// I5 (MI4 witness): through a static P from a static O: P's `top:inherit` is O's computed
/// 3px, so B1's `top:inherit` is 3: (0, 0, 3, -3).
#[test]
fn isp_i5_through_a_static_p_from_a_static_o() {
    let chain = "#o { top:3px; } #p { top:inherit; }";
    assert_pct_row(
        &format!("{chain} #b1 {{ position:relative; top:inherit; }}"),
        &format!("{chain} #b1 {{ position:relative; }}"),
        &format!("{chain} #b1 {{ position:relative; top:3px; }}"),
        &isp_y("c0", 3.0),
    );
}

/// I5b (MI5 witness): through a static P from a relative O: B1's fact is O's resolved 3px,
/// with origin `Inherited` from P (original 3); the facade still refuses the relative O
/// (`DisplayOrFlow` at projected 0, E10).
#[test]
fn isp_i5b_through_a_static_p_from_a_relative_o() {
    let fragment = "#o { position:relative; top:3px; } #p { top:inherit; } \
                    #b1 { position:relative; top:inherit; }";
    let (offset, _) = ptb_offset(fragment, 2);
    let top = BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels: 3.0,
        rounded_to_zero: false,
        origin: BoxFactOrigin::Inherited { original_parent: 3 },
    });
    assert_eq!(offset.map(|offset| offset[0]), Some(top), "{fragment}");
    assert_eq!(
        pht_error_in(&pct_css(5, fragment), PCT_OUTER),
        pht_refusal(0, "display/position/float", "DisplayOrFlow")
    );
}

/// I5c (MI11 witness): O's `top:inherit` is the initial `auto` (the root, CSS2.2 6.2.1),
/// P's `top:inherit` is O's `auto`, and B1's `top:inherit` is `auto`, so B1's
/// `bottom:2px` applies: (0, 0, -2, 2).
#[test]
fn isp_i5c_the_root_inherit_is_auto() {
    let chain = "#o { top:inherit; } #p { top:inherit; }";
    assert_pct_row(
        &format!("{chain} #b1 {{ position:relative; top:inherit; bottom:2px; }}"),
        &format!("{chain} #b1 {{ position:relative; }}"),
        &format!("{chain} #b1 {{ position:relative; bottom:2px; }}"),
        &isp_y("c0", -2.0),
    );
}

/// I6 (MI2 witness): P's initial `top` is `auto`, so B1's `top:inherit` is `auto` with
/// origin `Inherited` from P and B1's `bottom:2px` applies: (0, 0, -2, 2). Copying a kept
/// fact converts nothing: `numeric_calls` equals the twin's.
#[test]
fn isp_i6_an_initial_parent_offset_is_auto() {
    let fragment = "#b1 { position:relative; top:inherit; bottom:2px; }";
    let twin = "#b1 { position:relative; bottom:2px; }";
    let (offset, _) = ptb_offset(fragment, 2);
    let inherited = BoxFactOrigin::Inherited { original_parent: 3 };
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(BoxLengthFact::Auto { origin: inherited }),
        "{fragment}"
    );
    assert_pct_row(
        fragment,
        "#b1 { position:relative; }",
        twin,
        &isp_y("c0", -2.0),
    );
    assert_eq!(isp_calls(fragment), isp_calls(twin), "{fragment}");
}

/// I6b (review m-2; MP1 witness): P's explicit `top:auto` is computed through the
/// specified value: B1's `top:inherit` is `auto` with origin `Inherited` from P, and
/// B1's `bottom:2px` applies: (0, 0, -2, 2).
#[test]
fn isp_i6b_an_explicit_parent_auto_is_auto() {
    let fragment = "#p { top:auto; } #b1 { position:relative; top:inherit; bottom:2px; }";
    let (offset, _) = ptb_offset(fragment, 2);
    let inherited = BoxFactOrigin::Inherited { original_parent: 3 };
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(BoxLengthFact::Auto { origin: inherited }),
        "{fragment}"
    );
    assert_pct_row(
        fragment,
        "#p { top:auto; } #b1 { position:relative; }",
        "#p { top:auto; } #b1 { position:relative; bottom:2px; }",
        &isp_y("c0", -2.0),
    );
}

/// I7: a relative P inherits the static O's `top:50%`, which resolves against O's used
/// content height 13 (OAH): P's offsets (0, 0, 6.5, -6.5).
#[test]
fn isp_i7_a_relative_p_inherits_a_static_o_percentage() {
    assert_pct_row(
        "#o { top:50%; } #p { position:relative; top:inherit; }",
        "#o { top:50%; } #p { position:relative; }",
        "#o { top:50%; } #p { position:relative; top:6.5px; }",
        &isp_y("p", 6.5),
    );
}

/// I7b: a relative P inherits the static O's `top:3px`: (0, 0, 3, -3), one counted
/// conversion (the control's +1).
#[test]
fn isp_i7b_a_relative_p_inherits_a_static_o_length() {
    let fragment = "#o { top:3px; } #p { position:relative; top:inherit; }";
    let control = "#o { top:3px; } #p { position:relative; }";
    assert_pct_row(
        fragment,
        control,
        "#o { top:3px; } #p { position:relative; top:3px; }",
        &isp_y("p", 3.0),
    );
    assert_eq!(isp_calls(fragment), isp_calls(control) + 1, "{fragment}");
}

/// I8 (EML; MI3 witness): with the font entry, P's `top:2em` computes against P's own
/// font size 10: B1's `top:inherit` is 20.
#[test]
fn isp_i8_an_em_parent_offset_against_the_parent_font_size() {
    eml_row(
        "#p { font-size:10px; top:2em; } #b1 { position:relative; top:inherit; }",
        "#b1 { position:relative; top:20px; }",
        &isp_y("c0", 20.0),
    );
}

/// I8b (EML): B1's own `font-size:20px` is not the basis (it would give 40): 20. The twin
/// stays `#b1 { position:relative; top:20px; }` through the old entry, which does not
/// read `font-size` (review n-7).
#[test]
fn isp_i8b_the_child_font_size_is_not_the_basis() {
    eml_row(
        "#p { font-size:10px; top:2em; } #b1 { font-size:20px; position:relative; \
         top:inherit; }",
        "#b1 { position:relative; top:20px; }",
        &isp_y("c0", 20.0),
    );
}

/// I8c (EML): `ex` stays refused with the font entry too (EM-4): B1's `InheritedOffset`.
#[test]
fn isp_i8c_an_ex_parent_stays_refused_with_the_font_entry() {
    assert_eq!(
        eml_error("#p { font-size:10px; top:1ex; } #b1 { position:relative; top:inherit; }"),
        ISP_INHERITED_TOP
    );
}

/// I8d (EML; review m-3, MP2 witness): through a static P from O, O's `top:2em` computes
/// against O's own font size 10, not P's 20 (CSS2.2 6.1.2: P's computed value is O's
/// absolute one): B1's `top:inherit` is 20 (P's size would give 40).
#[test]
fn isp_i8d_an_em_chain_from_o_uses_the_outer_font_size() {
    eml_row(
        "#o { font-size:10px; top:2em; } #p { font-size:20px; top:inherit; } \
         #b1 { position:relative; top:inherit; }",
        "#b1 { position:relative; top:20px; }",
        &isp_y("c0", 20.0),
    );
}

/// I9: a relative F inherits P's `left:4px` (ltr): F's offsets (4, -4, 0, 0).
#[test]
fn isp_i9_a_float_inherits_left() {
    assert_pct_row(
        "#p { left:4px; } #f { position:relative; left:inherit; }",
        "#p { left:4px; } #f { position:relative; }",
        "#p { left:4px; } #f { position:relative; left:4px; }",
        &isp_x("c1", 4.0),
    );
}

/// IE1 (error order): an absolutely positioned P's `top:3px` is inherited (P is not
/// relatively positioned), so the facts succeed and the facade refuses P
/// (`DisplayOrFlow` at projected 1). Predecessor: the I1 producer error.
#[test]
fn isp_ie1_an_absolute_parent_is_refused_by_the_facade() {
    assert_eq!(
        pht_error_in(
            &pct_css(
                5,
                "#p { position:absolute; top:3px; } #b1 { position:relative; top:inherit; }"
            ),
            PCT_OUTER
        ),
        pht_refusal(1, "display/position/float", "DisplayOrFlow")
    );
}

/// IE2 (error order): with B1's inheritance admitted, B2's later `height:1ex` is the
/// producer's first error (predecessor: `InheritedOffset` at B1, original 0).
#[test]
fn isp_ie2_a_later_row_error_now_surfaces() {
    assert_eq!(
        isp_refusal("#p { top:3px; } #b1 { position:relative; top:inherit; } #b2 { height:1ex; }"),
        "Evaluation(Unsupported { original: 2, property: height, target: Element, reason: \
         RelativeLength })"
    );
}
