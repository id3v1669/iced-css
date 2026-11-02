//! S4: finite signed through participants and the explicit all-normal raw-edge rule.
//! Integer geometry below is hand-derived at fixture P content top11.
use super::*;

const S4_LEADING: &str = "#b1 { height:0; margin-top:-2px; margin-bottom:4px; } \
    #b2 { height:1px; margin-top:-3px; margin-bottom:2px; clear:none; }";
const S4_UPWARD: &str = "#b1 { height:2px; margin-top:0; margin-bottom:4px; } \
    #b2 { height:0; margin-top:3px; margin-bottom:-10px; clear:none; } \
    #b3 { height:1px; margin-top:2px; margin-bottom:1px; clear:none; }";
const S4_BARRIERS: &str = "#b1 { height:2px; margin-top:0; margin-bottom:4px; } \
    #b2 { height:0; margin-top:3px; margin-bottom:-10px; clear:both; } \
    #b3 { height:0; margin-top:2px; margin-bottom:0; clear:both; }";

fn s4_numbers(nodes: usize, css: &str, expected: &[(&str, f32)]) {
    let view = zhn_layout(nodes, css);
    for &(name, value) in expected {
        assert_eq!(
            zhn_pick(&view, name).to_bits(),
            value.to_bits(),
            "{css}: {name}"
        );
    }
}

#[test]
fn s4_signed_through_b1_uses_hypothetical_float_top_and_raw_height() {
    // clear:none is essential: matching clearance would move B2 to13.
    s4_numbers(
        5,
        S4_LEADING,
        &[
            ("c0.border.top", 9.0),
            ("c0.border.bottom", 9.0),
            ("float.top", 11.0),
            ("float.bottom", 13.0),
            ("c2.border.top", 12.0),
            ("c2.border.bottom", 13.0),
            ("tail0.cleared", 0.0),
            ("p.used_height", 4.0),
            ("p.content_bottom", 15.0),
            ("p.extent.normal_margin_bottom", 15.0),
            ("p.extent.float_extended", 0.0),
        ],
    );
}

#[test]
fn s4_later_upward_border_does_not_move_prior_boxes_or_use_group_height() {
    // Raw normal edges are B1(11,17), B2(14,7), B3(5,9).
    // Extrema [5,17] give height12, rebased to content bottom23.
    s4_numbers(
        6,
        S4_UPWARD,
        &[
            ("c0.border.bottom", 13.0),
            ("float.top", 17.0),
            ("float.bottom", 19.0),
            ("c2.border.top", 17.0),
            ("c2.border.bottom", 17.0),
            ("c3.border.top", 7.0),
            ("c3.border.bottom", 8.0),
            ("p.used_height", 12.0),
            ("p.content_bottom", 23.0),
            ("p.extent.normal_margin_bottom", 17.0),
            ("p.extent.selected_bottom", 23.0),
            ("p.extent.float_extended", 0.0),
        ],
    );
}

#[test]
fn s4_two_required_barriers_keep_negative_then_positive_clearance() {
    s4_numbers(
        6,
        S4_BARRIERS,
        &[
            ("float.top", 17.0),
            ("float.bottom", 19.0),
            ("c2.border.top", 19.0),
            ("c3.border.top", 19.0),
            ("tail0.cleared", 1.0),
            ("tail0.clearance", -1.0),
            ("tail1.cleared", 1.0),
            ("tail1.clearance", 8.0),
            ("p.used_height", 8.0),
            ("p.content_bottom", 19.0),
        ],
    );
    let css = pct_css(6, S4_BARRIERS);
    pct_with(6, &css, PCT_OUTER, layout_limits(), |layout| {
        for index in [0, 1] {
            let view = layout.sequence().tail_view(index).unwrap();
            let groups: Vec<_> = view.actual_groups().map(|group| group.id()).collect();
            assert_eq!(
                groups,
                [MarginGroupId::BeforeBarrier, MarginGroupId::AfterBarrier]
            );
        }
    })
    .unwrap();
}

#[test]
fn s4_all_negative_terminal_run_floors_span_then_extends_strictly() {
    let negative = "#b1 { height:0; margin-top:-4px; margin-bottom:-2px; } \
        #b2 { height:0; margin-top:-3px; margin-bottom:-5px; clear:none; }";
    for (float_height, height, extended) in [(0, 0.0_f32, 0.0_f32), (2, 2.0, 1.0)] {
        s4_numbers(
            5,
            &format!("{negative} #f {{ height:{float_height}px; }}"),
            &[
                ("c0.border.top", 7.0),
                ("c2.border.top", 7.0),
                ("float.top", 11.0),
                ("p.used_height", height),
                ("p.extent.normal_margin_bottom", 5.0),
                ("p.extent.float_extended", extended),
            ],
        );
    }
    for (float_height, height, extended) in [(4, 4.0_f32, 0.0_f32), (5, 5.0, 1.0)] {
        s4_numbers(
            5,
            &format!("{S4_LEADING} #f {{ height:{float_height}px; }}"),
            &[
                ("p.used_height", height),
                ("p.extent.float_extended", extended),
            ],
        );
    }
}

#[test]
fn s4_parent_rebase_limits_and_relative_offsets_keep_layout_observations() {
    for (suffix, height) in [
        ("#p { min-height:6px; }", 6.0_f32),
        ("#p { max-height:3px; }", 3.0),
        ("#p { height:10px; }", 10.0),
    ] {
        s4_numbers(
            5,
            &format!("{S4_LEADING} {suffix}"),
            &[
                ("c0.border.top", 9.0),
                ("c2.border.top", 12.0),
                ("p.used_height", height),
            ],
        );
    }
    s4_numbers(
        5,
        &format!("{S4_LEADING} #b2 {{ position:relative; top:50%; }}"),
        &[
            ("c2.border.top", 12.0),
            ("c2.offset.top", 2.0),
            ("p.used_height", 4.0),
        ],
    );
    let css = pct_css(6, S4_UPWARD);
    pct_with(6, &css, (0.0, 100.0, 100.0), layout_limits(), |layout| {
        assert_eq!(layout.parent().origin().content_top, 101.0);
        assert_eq!(layout.parent().height().used_height, 12.0);
        assert_eq!(layout.parent().height().content_bottom, 113.0);
        assert_eq!(layout.children()[3].border().top, 97.0);
    })
    .unwrap();
}

#[test]
fn s4_auto_containment_keeps_raw_boxes_and_source_order() {
    assert_eq!(
        zhn_error(5, &format!("{S4_LEADING} #p {{ overflow:auto; }}")),
        "ParentAutoOverflow { source: 2 }"
    );
    assert_eq!(
        zhn_error(6, &format!("{S4_UPWARD} #p {{ overflow:auto; }}")),
        "ParentAutoOverflow { source: 4 }"
    );
    assert_eq!(
        zhn_error(6, &format!("{S4_BARRIERS} #p {{ overflow:auto; }}")),
        "ParentAutoOverflow { source: 4 }"
    );
}

#[test]
fn s4_clear_side_selection_and_retained_equality_are_explicit() {
    assert_pct_view(
        &zhn_layout(5, &format!("{S4_LEADING} #b2 {{ clear:right; }}")),
        &zhn_layout(5, S4_LEADING),
        &[],
        "opposite side",
    );
    for clear in ["left", "both"] {
        s4_numbers(
            5,
            &format!("{S4_LEADING} #b2 {{ clear:{clear}; }}"),
            &[
                ("c2.border.top", 13.0),
                ("tail0.cleared", 1.0),
                ("tail0.clearance", 3.0),
                ("p.used_height", 5.0),
            ],
        );
    }
    assert_eq!(
        zhn_error(5, "#b1 { height:0; margin-top:-1px; margin-bottom:auto; }"),
        "Core(Placement { source: 4, error: UnsupportedEquality })"
    );
}

#[test]
fn s4_finite_admission_preserves_later_capture_error_precedence() {
    let leading = "#b1 { height:0; margin-top:-1px; }";
    assert_eq!(
        zhn_error(5, &format!("{leading} #f {{ width:auto; }}")),
        pht_refusal(3, "width", "FloatWidth")
    );
    assert_eq!(
        zhn_error(
            5,
            &format!("{leading} #f {{ width:auto; }} #b2 {{ overflow:scroll; }}")
        ),
        pht_refusal(4, "overflow", "ChildOverflow")
    );
    assert_eq!(
        daz_held(leading),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))"
    );
}

#[test]
fn s4_zero_categories_share_geometry_without_separating_nonliteral_zero() {
    let expected = zhn_layout(5, S4_LEADING);
    for height in ["0", "-0", "auto"] {
        assert_pct_view(
            &zhn_layout(5, &format!("{S4_LEADING} #b1 {{ height:{height}; }}")),
            &expected,
            &[],
            height,
        );
    }
    s4_numbers(
        5,
        "#b1 { height:10px; max-height:0; margin-top:-1px; } #b2 { height:0; clear:none; }",
        &[
            ("c0.border.top", 10.0),
            ("float.top", 14.0),
            ("c2.border.top", 14.0),
            ("p.used_height", 6.0),
        ],
    );
}

#[test]
fn s4_signed_through_exact_work_limits_are_independent() {
    for (nodes, suffix, facade, core) in [
        (5, S4_LEADING, 3284, 2141),
        (6, S4_BARRIERS, 4231, 2791),
        (
            7,
            "#b1 { height:0; margin-top:-1px; } #b4 { height:1px; clear:none; }",
            5178,
            3442,
        ),
    ] {
        let css = pct_css(nodes, suffix);
        for (work, core_work, expected) in [
            (facade - 1, core, Err("Work".to_owned())),
            (facade, core - 1, Err("Core(Work)".to_owned())),
            (facade, core, Ok((facade, core))),
        ] {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            let actual = pct_with(nodes, &css, PCT_OUTER, limits, |layout| {
                (layout.receipt().work, layout.sequence().receipt().work)
            })
            .map_err(|error| format!("{error:?}"));
            assert_eq!(actual, expected, "N{nodes}");
        }
    }
}
