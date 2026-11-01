//! CTT facade RED: existing geometry APIs only; no prospective group-view names.
//! Child module of collapse_through_b1; root must compile/run the planned stages.
use super::*;

/// G1 translated by the fixture's P content top11: B2 border17, B3 border23.
/// Source-last raw bottom24 gives P height13. Collapsed endpoint23 would give12;
/// the earlier B2 raw margin bottom27 would instead give16.
/// Accepted CTB baseline is expected to refuse ordinary zero B2 at source4.
#[test]
fn ctt_red_g1_tail_chain_preserves_source_last_parent_height() {
    let view = zhn_layout(
        6,
        "#b1 { height:2px; } \
         #b2 { height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
         #b3 { height:0; margin-top:2px; margin-bottom:1px; clear:none; }",
    );
    for (name, expected) in [
        ("p.content_top", 11.0_f32),
        ("c0.border.top", 11.),
        ("c0.border.bottom", 13.),
        ("float.top", 17.),
        ("float.bottom", 19.),
        ("c2.border.top", 17.),
        ("c2.border.bottom", 17.),
        ("c3.border.top", 23.),
        ("c3.border.bottom", 23.),
        ("p.used_height", 13.),
        ("p.content_bottom", 24.),
        ("p.extent.normal_margin_bottom", 24.),
        ("p.extent.selected_bottom", 24.),
        ("p.extent.float_extended", 0.),
    ] {
        assert_eq!(
            zhn_pick(&view, name).to_bits(),
            expected.to_bits(),
            "{name}"
        );
    }
}

const CTT_G1: &str = "#b1 { height:2px; } \
     #b2 { height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
     #b3 { height:0; margin-top:2px; margin-bottom:1px; clear:none; }";

fn ctt_facade_numbers(view: &[(String, f32)], expected: &[(&str, f32)], context: &str) {
    for &(name, value) in expected {
        assert_eq!(
            zhn_pick(view, name).to_bits(),
            value.to_bits(),
            "{context}: {name}"
        );
    }
}

#[test]
fn ctt_parent_height_g3_g4_g7_selects_last_raw_bottom() {
    for (name, fragment, expected) in [
        (
            "G3",
            "#f { height:20px; } #b2 { clear:both; }",
            [11.0_f32, 13., 17., 37., 37., 44., 34., 45., 17.],
        ),
        (
            "G4",
            "#f { height:7px; } #b2 { margin-top:10px; margin-bottom:20px; clear:both; }",
            [11., 13., 17., 24., 24., 34., 24., 35., -3.],
        ),
        (
            "G7",
            "#b1 { height:0; margin-top:1px; }",
            [12., 12., 12., 14., 15., 21., 11., 22., 0.],
        ),
    ] {
        let view = zhn_layout(6, &format!("{CTT_G1} {fragment}"));
        let keys = [
            "c0.border.top",
            "c0.border.bottom",
            "float.top",
            "float.bottom",
            "c2.border.top",
            "c3.border.top",
            "p.used_height",
            "p.content_bottom",
            "tail0.clearance",
        ];
        for (key, value) in keys.into_iter().zip(expected) {
            assert_eq!(
                zhn_pick(&view, key).to_bits(),
                value.to_bits(),
                "{name}: {key}"
            );
        }
        ctt_facade_numbers(
            &view,
            &[
                ("c2.border.bottom", expected[4]),
                ("c3.border.bottom", expected[5]),
                ("p.extent.normal_margin_bottom", expected[7]),
                ("p.extent.selected_bottom", expected[7]),
                ("p.extent.float_extended", 0.),
                ("tail0.cleared", if name == "G7" { 0. } else { 1. }),
            ],
            name,
        );
    }
}

#[test]
fn ctt_literal_auto_and_negative_zero_tails_have_identical_geometry() {
    let explicit = zhn_layout(6, CTT_G1);
    for height in ["0", "auto", "-0"] {
        let fragment = format!("{CTT_G1} #b2 {{ height:{height}; }} #b3 {{ height:{height}; }}");
        let actual = zhn_layout(6, &fragment);
        // Includes all existing numeric observations, including unchanged work.
        assert_pct_view(&actual, &explicit, &[], &fragment);
        ctt_facade_numbers(
            &actual,
            &[("c2.used_height", 0.), ("c3.used_height", 0.)],
            height,
        );
    }
}

#[test]
fn ctt_nonliteral_zero_tails_are_separated_in_s3() {
    // NZS: retain computed provenance and require a separate outgoing margin group.
    for fragment in [
        "#b2 { height:50%; }",
        "#p { height:20px; } #b2 { height:0%; }",
        "#b2 { height:0; min-height:0%; }",
        "#b2 { height:auto; min-height:5%; }",
        "#b2 { height:10px; max-height:0; }",
        "#b2 { height:0.0000000000000000000000000000000000000000000001px; }",
        "#b2 { height:auto; min-height:0.0000000000000000000000000000000000000000000001px; }",
    ] {
        nzs_assert_zero_layout(5, fragment, 4);
    }
}

#[test]
fn ctt_separated_nonliteral_zeros_remain_admitted() {
    for separator in [
        "padding-top:1px;",
        "border-bottom-width:1px; border-bottom-style:solid;",
        "overflow:hidden;",
    ] {
        let actual = format!("#b2 {{ height:10px; max-height:0; {separator} }}");
        let twin = format!("#b2 {{ height:0; {separator} }}");
        assert_pct_view(&zhn_layout(5, &actual), &zhn_layout(5, &twin), &[], &actual);
    }
}

#[test]
fn ctt_signed_normal_scope_refuses_first_needed_through_tail() {
    let refusal = "Core(Profile(Unsupported { source: 4, reason: NonpositiveBlockHeight }))";
    for suffix in [
        "#b1 { margin-top:-1px; }",
        "#b1 { margin-bottom:-1px; }",
        "#b2 { margin-top:-1px; }",
        "#b2 { margin-bottom:-1px; }",
        "#b3 { margin-top:-1px; }",
        "#b3 { margin-bottom:-1px; }",
    ] {
        assert_eq!(
            zhn_error(6, &format!("{CTT_G1} {suffix}")),
            refusal,
            "{suffix}"
        );
    }
    assert_eq!(
        zhn_error(
            6,
            &format!("{CTT_G1} #b1 {{ height:0; }} #b3 {{ margin-top:-1px; }}")
        ),
        CTB_B1_REFUSAL,
    );
    // F's signed margin is excluded; its outer extent is still nonnegative.
    let view = zhn_layout(6, &format!("{CTT_G1} #f {{ margin-bottom:-1px; }}"));
    ctt_facade_numbers(
        &view,
        &[
            ("float.top", 17.),
            ("float.bottom", 18.),
            ("c1.border.bottom", 19.),
            ("c2.border.top", 17.),
            ("c3.border.top", 23.),
            ("p.used_height", 13.),
        ],
        "signed F excluded from normal scope",
    );
}

#[test]
fn ctt_separators_reset_the_run_but_keep_sibling_margin_collapse() {
    for (separator, border_bottom, content_top, content_bottom, next_top, parent_height) in [
        ("height:1px;", 24., 23., 24., 25., 15.),
        ("height:0; padding-top:1px;", 24., 24., 24., 25., 15.),
        (
            "height:0; border-bottom-width:1px; border-bottom-style:solid;",
            24.,
            23.,
            23.,
            25.,
            15.,
        ),
        ("height:0; overflow:hidden;", 23., 23., 23., 24., 14.),
        ("height:0; overflow:auto;", 23., 23., 23., 24., 14.),
    ] {
        let fragment = format!(
            "{CTT_G1} #b3 {{ {separator} }} \
             #b4 {{ height:1px; margin-top:0; margin-bottom:0; clear:none; }}"
        );
        let view = zhn_layout(7, &fragment);
        ctt_facade_numbers(
            &view,
            &[
                ("c3.border.top", 23.),
                ("c3.border.bottom", border_bottom),
                ("c3.content.top", content_top),
                ("c3.content.bottom", content_bottom),
                ("c4.border.top", next_top),
                ("c4.border.bottom", next_top + 1.),
                ("p.used_height", parent_height),
            ],
            separator,
        );
    }
    let ordinary = zhn_layout(
        7,
        &format!("{CTT_G1} #b4 {{ height:1px; margin-top:0; margin-bottom:0; clear:none; }}"),
    );
    ctt_facade_numbers(
        &ordinary,
        &[
            ("c4.border.top", 23.),
            ("c4.border.bottom", 24.),
            ("p.used_height", 13.),
        ],
        "a zero margin alone does not separate",
    );
}

#[test]
fn ctt_hidden_auto_and_explicit_height_keep_raw_margin_containment() {
    let hidden = zhn_layout(6, CTT_G1);
    ctt_facade_numbers(
        &hidden,
        &[
            ("p.content_bottom", 24.),
            ("c2.border.bottom", 17.),
            ("c2.margin.bottom", 10.),
        ],
        "hidden admits B2 raw bottom27 beyond parent24",
    );
    assert_eq!(
        zhn_error(6, &format!("{CTT_G1} #p {{ overflow:auto; }}")),
        "ParentAutoOverflow { source: 4 }",
    );
    assert_eq!(
        zhn_error(6, &format!("{CTT_G1} #p {{ overflow:auto; height:15px; }}")),
        "ParentAutoOverflow { source: 4 }",
    );
    let explicit = zhn_layout(6, &format!("{CTT_G1} #p {{ overflow:auto; height:16px; }}"));
    ctt_facade_numbers(
        &explicit,
        &[
            ("p.content_bottom", 27.),
            ("p.used_height", 16.),
            ("c3.border.top", 23.),
        ],
        "raw boundary contact at27 is contained",
    );
}

#[test]
fn ctt_relative_offset_changes_containment_without_changing_flow_height() {
    let fragment = format!("{CTT_G1} #b2 {{ position:relative; top:1px; }}");
    let hidden = zhn_layout(6, &fragment);
    ctt_facade_numbers(
        &hidden,
        &[
            ("c2.border.top", 17.),
            ("c2.offset.top", 1.),
            ("c3.border.top", 23.),
            ("p.used_height", 13.),
        ],
        "offset does not enter flow",
    );
    assert_eq!(
        zhn_error(
            6,
            &format!("{fragment} #p {{ overflow:auto; height:16px; }}")
        ),
        "ParentAutoOverflow { source: 4 }",
    );
    let contained = zhn_layout(
        6,
        &format!("{fragment} #p {{ overflow:auto; height:17px; }}"),
    );
    ctt_facade_numbers(
        &contained,
        &[("p.content_bottom", 28.), ("c2.offset.top", 1.)],
        "raw offset bottom28 is contained",
    );
}

#[test]
fn ctt_float_extension_uses_strict_maximum_after_last_raw_normal() {
    for (height, bottom, parent_height, selected, extended) in [
        (6, 23., 13., 24., 0.),
        (7, 24., 13., 24., 0.),
        (8, 25., 14., 25., 1.),
    ] {
        let fragment = format!("{CTT_G1} #f {{ height:{height}px; }}");
        let view = zhn_layout(6, &fragment);
        ctt_facade_numbers(
            &view,
            &[
                ("float.bottom", bottom),
                ("c3.border.top", 23.),
                ("p.extent.normal_margin_bottom", 24.),
                ("p.extent.selected_bottom", selected),
                ("p.extent.float_extended", extended),
                ("p.used_height", parent_height),
            ],
            &fragment,
        );
    }
}

#[test]
fn ctt_admission_exposes_existing_later_facade_errors_in_order() {
    let parent = format!("#p {{ width:{PCT_F32_MAX_PX}; }}");
    for (nodes, fragment, expected) in [
        (
            5,
            format!("{parent} #b2 {{ height:auto; padding-top:200%; }}"),
            "Percentage { source: 4, property: \"padding-top\", error: Unrepresentable }"
                .to_owned(),
        ),
        (
            5,
            "#b2 { height:auto; } #f { width:auto; }".to_owned(),
            pht_refusal(3, "width", "FloatWidth"),
        ),
        (
            6,
            "#b2 { height:auto; } #b3 { overflow:scroll; }".to_owned(),
            pht_refusal(5, "overflow", "ChildOverflow"),
        ),
        (
            6,
            "#b2 { height:10px; max-height:0; } #b3 { overflow:scroll; }".to_owned(),
            pht_refusal(5, "overflow", "ChildOverflow"),
        ),
    ] {
        assert_eq!(zhn_error(nodes, &fragment), expected, "{fragment}");
    }
    assert_eq!(
        daz_held("#b2 { height:auto; padding-top:1%; }"),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
    );
    assert_eq!(
        daz_held("#b2 { height:10px; max-height:0; padding-top:1%; }"),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
    );
}

#[test]
fn ctt_parent_height_constraints_preserve_the_new_tail_flow() {
    for (declaration, used, bottom) in [
        ("max-height:12px;", 12., 23.),
        ("min-height:15px;", 15., 26.),
        ("height:16px;", 16., 27.),
    ] {
        let fragment = format!("{CTT_G1} #p {{ {declaration} }}");
        let view = zhn_layout(6, &fragment);
        ctt_facade_numbers(
            &view,
            &[
                ("c2.border.top", 17.),
                ("c3.border.top", 23.),
                ("p.used_height", used),
                ("p.content_bottom", bottom),
            ],
            declaration,
        );
    }
}

// Each matching-side case requires clearance at B2; the opposite side and none
// retain the uncleared run. B3 remains through and does not clear in this table.
#[test]
fn ctt_through_b2_selects_only_matching_float_sides() {
    for (side, float_left) in [("left", 0.0_f32), ("right", 90.0_f32)] {
        for clear in ["left", "right", "both", "none"] {
            let matching = clear == side || clear == "both";
            let fragment =
                format!("{CTT_G1} #f {{ float:{side}; height:20px; }} #b2 {{ clear:{clear}; }}");
            let view = zhn_layout(6, &fragment);
            ctt_facade_numbers(
                &view,
                &[
                    ("float.left", float_left),
                    ("float.right", float_left + 10.),
                    ("float.top", 17.),
                    ("float.bottom", 37.),
                    ("c2.border.top", if matching { 37. } else { 17. }),
                    ("c2.border.bottom", if matching { 37. } else { 17. }),
                    ("c3.border.top", if matching { 44. } else { 23. }),
                    ("c3.border.bottom", if matching { 44. } else { 23. }),
                    ("tail0.cleared", if matching { 1. } else { 0. }),
                    ("tail0.clearance", if matching { 17. } else { 0. }),
                    ("tail1.cleared", 0.),
                    ("tail1.clearance", 0.),
                    (
                        "p.extent.normal_margin_bottom",
                        if matching { 45. } else { 24. },
                    ),
                    ("p.extent.selected_bottom", if matching { 45. } else { 37. }),
                    ("p.extent.float_extended", if matching { 0. } else { 1. }),
                    ("p.used_height", if matching { 34. } else { 26. }),
                    ("p.content_bottom", if matching { 45. } else { 37. }),
                ],
                &fragment,
            );
        }
    }
}

// This table moves the first possible clearance to B3 after a through B2.
// The old run's maximum10 remains in the zero-clearance baseline13+10+2=25.
#[test]
fn ctt_through_b3_selects_matching_sides_after_an_uncleared_tail() {
    for side in ["left", "right"] {
        for clear in ["left", "right", "both", "none"] {
            let matching = clear == side || clear == "both";
            let fragment =
                format!("{CTT_G1} #f {{ float:{side}; height:20px; }} #b3 {{ clear:{clear}; }}");
            let view = zhn_layout(6, &fragment);
            ctt_facade_numbers(
                &view,
                &[
                    ("c2.border.top", 17.),
                    ("c2.border.bottom", 17.),
                    ("tail0.cleared", 0.),
                    ("tail0.clearance", 0.),
                    ("c3.border.top", if matching { 37. } else { 23. }),
                    ("c3.border.bottom", if matching { 37. } else { 23. }),
                    ("tail1.cleared", if matching { 1. } else { 0. }),
                    ("tail1.clearance", if matching { 12. } else { 0. }),
                    (
                        "p.extent.normal_margin_bottom",
                        if matching { 38. } else { 24. },
                    ),
                    ("p.extent.selected_bottom", if matching { 38. } else { 37. }),
                    ("p.extent.float_extended", if matching { 0. } else { 1. }),
                    ("p.used_height", if matching { 27. } else { 26. }),
                    ("p.content_bottom", if matching { 38. } else { 37. }),
                ],
                &fragment,
            );
        }
    }
}

// Exercise the actual CSS producer and facade, with an explicit numeric budget.
// O top -3 plus P padding-top1 gives P content top -2. This is a signed host
// coordinate, not a negative normal margin and not the deferred signed-through scope.
fn ctt_g8_decimal_layout(fragment: &str) -> Result<Vec<(String, f32)>, SiblingLayoutError> {
    let css = pct_css(6, fragment);
    let mut result = None;
    sibling_fixture(6, &css, |projection, display| {
        let mut fact_limits = sibling_fact_limits();
        fact_limits.evaluation.numeric = iced_css_core::values::NumericLimits {
            digits: nz(256),
            work: nz(20_000_000),
        };
        // Sequence prepays 22 conversions per source, six sources here.
        fact_limits.evaluation.numeric_work = nz(2_640_000_000);
        let facts = evaluate_sibling_box_facts(projection, display, fact_limits)
            .unwrap_or_else(|error| panic!("G8 decimal facts: {error:?}"));
        assert_eq!(facts.numeric_limits().digits.get(), 256);
        assert_eq!(facts.numeric_limits().work.get(), 20_000_000);
        result = Some(
            default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.,
                100.,
                -3.,
                layout_limits(),
            )
            .map(|layout| pct_view(&layout)),
        );
    });
    result.expect("G8 fixture callback ran")
}

#[test]
fn ctt_g8_decimal_conversion_preserves_cleared_reference_publications() {
    let mut exact_view: Option<Vec<(String, f32)>> = None;
    for (bottom, height) in [
        ("0.999999940395355224609375", "0.0000000894069671630859375"),
        ("0.99999994", "0.00000008940697"),
    ] {
        let fragment = format!(
            "#b1 {{ height:1px; margin-top:0; margin-bottom:{bottom}px; }} \
             #f {{ height:{height}px; margin-top:0; margin-bottom:0; }} \
             #b2 {{ height:0; margin-top:1px; margin-bottom:1px; clear:both; }} \
             #b3 {{ height:0; margin-top:0; margin-bottom:0; clear:none; }}"
        );
        let view = ctt_g8_decimal_layout(&fragment)
            .unwrap_or_else(|error| panic!("G8 decimal layout: {error:?}"));
        for (name, bits) in [
            ("p.border_top", 0xc040_0000_u32),
            ("p.content_top", 0xc000_0000),
            ("c0.border.top", 0xc000_0000),
            ("c0.border.bottom", 0xbf80_0000),
            ("c0.margin.bottom", 0x3f7f_ffff),
            ("c1.used_height", 0x33c0_0000),
            ("float.top", 0xb380_0000),
            ("float.bottom", 0x3300_0000),
            ("c2.border.top", 0x3300_0000),
            ("c2.border.bottom", 0x3300_0000),
            ("c3.border.top", 0x3300_0000),
            ("c3.border.bottom", 0x3300_0000),
            ("tail0.cleared", 0x3f80_0000),
            ("tail0.clearance", 0xbf7f_fffe),
            ("tail1.cleared", 0x0000_0000),
            ("tail1.clearance", 0x0000_0000),
            ("p.extent.normal_margin_bottom", 0x3300_0000),
            ("p.extent.selected_bottom", 0x3300_0000),
            ("p.extent.float_extended", 0x0000_0000),
            // These publications intentionally differ: RN(2+2^-25)=2,
            // but unconstrained Auto preserves its selected endpoint2^-25.
            ("p.used_height", 0x4000_0000),
            ("p.content_bottom", 0x3300_0000),
        ] {
            assert_eq!(
                zhn_pick(&view, name).to_bits(),
                bits,
                "G8 {bottom}/{height}: {name}"
            );
        }
        if let Some(exact) = &exact_view {
            assert_pct_view(
                &view,
                exact,
                &[],
                "G8 shorter CSS decimals round to the same facts",
            );
        } else {
            exact_view = Some(view);
        }
        // F ends exactly at P's published bottom and passes containment. The
        // next source, B2, has raw bottom RN(2^-25+1)=1 and is first to fail.
        assert_eq!(
            format!(
                "{:?}",
                ctt_g8_decimal_layout(&format!("{fragment} #p {{ overflow:auto; }}"))
                    .expect_err("G8 Auto P must reject B2's raw margin box")
            ),
            "ParentAutoOverflow { source: 4 }",
        );
    }
}

#[path = "nonliteral_zero_separation.rs"]
mod nonliteral_zero_separation;
