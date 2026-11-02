//! NZS baseline-compatible RED witnesses: only accepted CTT public geometry APIs.
use super::*;

#[test]
fn nzs_red_g1_nonliteral_b1_preserves_separate_margins() {
    let view = zhn_layout(
        5,
        "#b1 { height:10px; max-height:0; margin-top:1px; margin-bottom:4px; } \
         #b2 { height:1px; margin-top:3px; margin-bottom:2px; clear:none; }",
    );
    for (name, expected) in [
        ("c0.border.top", 12.0_f32),
        ("c0.border.bottom", 12.),
        ("c0.used_height", 0.),
        ("float.top", 16.),
        ("float.bottom", 18.),
        ("c2.border.top", 16.),
        ("c2.border.bottom", 17.),
        ("p.used_height", 8.),
        ("p.content_bottom", 19.),
    ] {
        assert_eq!(
            zhn_pick(&view, name).to_bits(),
            expected.to_bits(),
            "{name}"
        );
    }
}

#[test]
fn nzs_red_g3_consecutive_nonliteral_tails_reset_outgoing_groups() {
    let view = zhn_layout(
        6,
        "#b1 { height:2px; } \
         #b2 { height:10px; max-height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
         #b3 { height:10px; max-height:0; margin-top:2px; margin-bottom:1px; clear:none; }",
    );
    for (name, expected) in [
        ("float.top", 17.0_f32),
        ("float.bottom", 19.),
        ("c2.border.top", 17.),
        ("c2.border.bottom", 17.),
        ("c3.border.top", 27.),
        ("c3.border.bottom", 27.),
        ("p.used_height", 17.),
        ("p.content_bottom", 28.),
    ] {
        assert_eq!(
            zhn_pick(&view, name).to_bits(),
            expected.to_bits(),
            "{name}"
        );
    }
}

const NZS_G3: &str = "#b1 { height:2px; } \
    #b2 { height:10px; max-height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
    #b3 { height:10px; max-height:0; margin-top:2px; margin-bottom:1px; clear:none; }";

fn nzs_numbers(nodes: usize, fragment: &str, expected: &[(&str, f32)]) {
    let view = zhn_layout(nodes, fragment);
    for &(name, value) in expected {
        assert_eq!(
            zhn_pick(&view, name).to_bits(),
            value.to_bits(),
            "{fragment}: {name}"
        );
    }
}

#[test]
fn nzs_g2_g4_g5_g6_g7_g8_g9_literal_geometry() {
    nzs_numbers(
        5,
        "#b1 { height:10px; max-height:0; margin-top:1px; margin-bottom:4px; } \
         #b2 { height:1px; margin-top:3px; margin-bottom:2px; clear:both; }",
        &[
            ("c0.border.top", 12.),
            ("float.top", 16.),
            ("float.bottom", 18.),
            ("c2.border.top", 18.),
            ("c2.border.bottom", 19.),
            ("tail0.clearance", -1.),
            ("tail0.cleared", 1.),
            ("p.used_height", 10.),
        ],
    );
    nzs_numbers(
        7,
        "#b1 { height:2px; } \
         #b2 { height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
         #b3 { height:10px; max-height:0; margin-top:2px; margin-bottom:1px; clear:none; } \
         #b4 { height:0; margin-top:5px; margin-bottom:7px; clear:none; }",
        &[
            ("c2.border.top", 17.),
            ("c3.border.top", 23.),
            ("c4.border.top", 28.),
            ("p.used_height", 24.),
            ("p.content_bottom", 35.),
        ],
    );
    nzs_numbers(
        6,
        "#b1 { height:2px; } \
         #b2 { height:10px; max-height:0; margin-top:3px; margin-bottom:10px; clear:both; } \
         #b3 { height:1px; margin-top:2px; margin-bottom:1px; clear:none; }",
        &[
            ("c2.border.top", 19.),
            ("tail0.clearance", -1.),
            ("tail0.cleared", 1.),
            ("c3.border.top", 29.),
            ("c3.border.bottom", 30.),
            ("p.used_height", 20.),
        ],
    );
    nzs_numbers(
        5,
        "#b1 { height:2px; } \
         #b2 { height:10px; max-height:0; margin-top:2px; margin-bottom:10px; clear:both; }",
        &[
            ("c2.border.top", 19.),
            ("tail0.clearance", 0.),
            ("tail0.cleared", 1.),
            ("p.used_height", 18.),
        ],
    );
    nzs_numbers(
        5,
        "#b1 { height:10px; max-height:0; margin-top:-2px; margin-bottom:4px; } \
         #b2 { height:1px; margin-top:-3px; margin-bottom:2px; clear:none; }",
        &[
            ("c0.border.top", 9.),
            ("float.top", 13.),
            ("float.bottom", 15.),
            ("c2.border.top", 10.),
            ("c2.border.bottom", 11.),
            ("p.used_height", 4.),
            ("p.extent.normal_margin_bottom", 13.),
            ("p.extent.selected_bottom", 15.),
            ("p.extent.float_extended", 1.),
        ],
    );
    nzs_numbers(
        5,
        "#b1 { height:2px; } \
         #b2 { height:10px; max-height:0; margin-top:5px; margin-bottom:0; clear:none; }",
        &[
            ("float.top", 17.),
            ("float.bottom", 19.),
            ("c2.border.top", 18.),
            ("c2.border.bottom", 18.),
            ("p.used_height", 8.),
        ],
    );
    nzs_numbers(
        6,
        "#b1 { height:10px; margin-bottom:2px; } #f { height:0; } \
         #b2 { height:10px; max-height:0; margin-top:5px; margin-bottom:7px; clear:none; } \
         #b3 { height:1px; margin-top:11px; margin-bottom:0; clear:none; }",
        &[
            ("c2.border.top", 26.),
            ("c3.border.top", 37.),
            ("c3.border.bottom", 38.),
            ("p.used_height", 27.),
        ],
    );
}

#[test]
fn nzs_group_membership_splits_before_after_and_terminal_runs() {
    let css = pct_css(6, NZS_G3);
    pct_with(6, &css, PCT_OUTER, layout_limits(), |layout| {
        let owner = layout.sequence();
        for (index, previous, current) in [(0, 2, 4), (1, 4, 5)] {
            let view = owner.tail_view(index).unwrap();
            let group = view.hypothetical_group();
            assert_eq!(group.id(), MarginGroupId::HypotheticalPair);
            let members: Vec<_> = group
                .members()
                .iter()
                .map(|m| (m.source_index, m.side))
                .collect();
            assert_eq!(
                members,
                [(previous, MarginSide::Bottom), (current, MarginSide::Top)]
            );
            let actual: Vec<_> = view.actual_groups().map(|g| g.id()).collect();
            assert_eq!(actual, [MarginGroupId::ActualPair]);
        }
        let terminal: Vec<_> = owner
            .terminal_run()
            .members()
            .iter()
            .map(|m| (m.source_index, m.side))
            .collect();
        assert_eq!(terminal, [(5, MarginSide::Bottom)]);
    })
    .unwrap();
    for top in [2, 3] {
        let css = pct_css(
            5,
            &format!(
                "#b1 {{ height:2px; }} #b2 {{ height:10px; max-height:0; \
             margin-top:{top}px; margin-bottom:10px; clear:both; }}"
            ),
        );
        pct_with(5, &css, PCT_OUTER, layout_limits(), |layout| {
            let owner = layout.sequence();
            let view = owner.tail_view(0).unwrap();
            assert_eq!(view.clearance(), Some(if top == 2 { 0.0 } else { -1.0 }));
            let groups: Vec<_> = view
                .actual_groups()
                .map(|g| {
                    (
                        g.id(),
                        g.members()
                            .iter()
                            .map(|m| (m.source_index, m.side))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            assert_eq!(
                groups,
                [
                    (MarginGroupId::BeforeBarrier, vec![(2, MarginSide::Bottom)]),
                    (MarginGroupId::AfterBarrier, vec![(4, MarginSide::Top)]),
                ]
            );
            assert_eq!(owner.terminal_run().members().len(), 1);
            assert_eq!(owner.terminal_run().members()[0].side, MarginSide::Bottom);
        })
        .unwrap();
    }

    let css = pct_css(
        7,
        "#b1 { height:2px; } \
         #b2 { height:0; margin-top:3px; margin-bottom:10px; clear:none; } \
         #b3 { height:10px; max-height:0; margin-top:2px; margin-bottom:1px; clear:none; } \
         #b4 { height:0; margin-top:5px; margin-bottom:7px; clear:none; }",
    );
    pct_with(7, &css, PCT_OUTER, layout_limits(), |layout| {
        let owner = layout.sequence();
        let incoming = owner.tail_view(1).unwrap().hypothetical_group();
        let members: Vec<_> = incoming
            .members()
            .iter()
            .map(|m| (m.source_index, m.side))
            .collect();
        assert_eq!(
            members,
            [
                (2, MarginSide::Bottom),
                (4, MarginSide::Top),
                (4, MarginSide::Bottom),
                (5, MarginSide::Top)
            ]
        );
        let after = owner.tail_view(2).unwrap().hypothetical_group();
        let members: Vec<_> = after
            .members()
            .iter()
            .map(|m| (m.source_index, m.side))
            .collect();
        assert_eq!(members, [(5, MarginSide::Bottom), (6, MarginSide::Top)]);
        let terminal: Vec<_> = owner
            .terminal_run()
            .members()
            .iter()
            .map(|m| (m.source_index, m.side))
            .collect();
        assert_eq!(
            terminal,
            [
                (5, MarginSide::Bottom),
                (6, MarginSide::Top),
                (6, MarginSide::Bottom)
            ]
        );
    })
    .unwrap();
}

#[test]
fn nzs_source_families_preserve_computed_facts_at_each_position() {
    let tiny = "0.0000000000000000000000000000000000000000000001px";
    let cases = [
        ("", "height:10px; max-height:0;".to_owned()),
        ("#p { height:20px; }", "height:0%;".to_owned()),
        ("#p { height:0; }", "height:50%;".to_owned()),
        ("", "height:50%;".to_owned()),
        ("", "height:0; min-height:0%;".to_owned()),
        ("", "height:auto; min-height:5%;".to_owned()),
        ("#p { height:0; }", "height:auto; min-height:5%;".to_owned()),
        ("", format!("height:{tiny};")),
        ("", format!("height:auto; min-height:{tiny};")),
    ];
    for (parent, declaration) in cases {
        for (selector, source) in [("b1", 2), ("b2", 4), ("b3", 5), ("b4", 6)] {
            let rule = format!("{parent} #{selector} {{ {declaration} }}");
            nzs_assert_zero_layout(7, &rule, source);
            // Keep the original height/minimum facts, not a relabeled literal zero.
            let css = pct_css(7, &rule);
            sibling_fixture(7, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let row = facts.row(source).unwrap();
                let saved = (row.height, row.min_height, row.max_height);
                let layout = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    0.,
                    100.,
                    10.,
                    layout_limits(),
                )
                .unwrap();
                assert_eq!(layout.children()[source - 2].computed_height(), saved.0);
                assert_eq!(layout.children()[source - 2].used_height().to_bits(), 0);
                let row_after = facts.row(source).unwrap();
                assert_eq!(
                    (row_after.height, row_after.min_height, row_after.max_height),
                    saved
                );
                if declaration.starts_with("height:50%;") || declaration.starts_with("height:0%;") {
                    assert!(matches!(saved.0, BoxLengthFact::Percentage(_)));
                }
                if declaration.contains("min-height:5%") || declaration.contains("min-height:0%") {
                    assert!(matches!(saved.1, LengthPercentageBoxFact::Percentage(_)));
                }
            });
        }
    }
}

#[test]
fn nzs_origins_and_positive_minimum_do_not_change_computed_categories() {
    for (parent, rule, origin) in [
        ("", "min-height:0%;", "initial"),
        ("", "height:auto; min-height:0%;", "specified"),
        (
            "#p { height:0; }",
            "height:inherit; min-height:0%;",
            "inherited",
        ),
        ("", "height:10px; max-height:0;", "specified"),
    ] {
        // Omit B3 height from the fixture itself: an absent declaration, not
        // authored `initial`, exercises the CSS2.2 initial-value path.
        let css = format!(
            "#o {{ width:100px; }} \
             #p {{ overflow:hidden; padding-top:1px; padding-bottom:1px; }} \
             #b1 {{ height:1px; margin-bottom:4px; }} \
             #f {{ float:left; width:10px; height:2px; }} \
             #b2 {{ height:1px; margin-top:3px; margin-bottom:3px; clear:both; }} \
             #b3 {{ margin-top:2px; margin-bottom:4px; clear:both; {rule} }} {parent}"
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let computed = facts.row(5).unwrap().height;
            let actual_origin = match computed {
                BoxLengthFact::Auto { origin } => origin,
                BoxLengthFact::Absolute(value) => value.origin,
                BoxLengthFact::Percentage(value) => value.origin,
            };
            let expected_origin = match origin {
                "initial" => BoxFactOrigin::Initial,
                "inherited" => BoxFactOrigin::Inherited {
                    original_parent: projection.original_index(1).unwrap(),
                },
                "specified" => BoxFactOrigin::Specified,
                _ => unreachable!("finite origin table"),
            };
            assert_eq!(actual_origin, expected_origin);
            let layout = default_outer_layout(
                &facts,
                &projection.nodes()[0],
                0.,
                100.,
                10.,
                layout_limits(),
            )
            .unwrap();
            assert_eq!(layout.children()[3].computed_height(), computed);
            assert_eq!(layout.children()[3].used_height(), 0.0);
        });
    }
    for height in ["auto", "0", "50%", "10px"] {
        let view = zhn_layout(
            5,
            &format!("#b1 {{ height:{height}; min-height:3px; max-height:0; }}"),
        );
        assert_eq!(zhn_pick(&view, "c0.used_height"), 3.0);
    }
    for parent in ["height:50%;", "min-height:20px;"] {
        let view = zhn_layout(5, &format!("#p {{ {parent} }} #b1 {{ height:50%; }}"));
        assert_eq!(zhn_pick(&view, "c0.used_height"), 0.0);
        assert_eq!(zhn_pick(&view, "c0.border.top"), 11.0);
        assert_eq!(zhn_pick(&view, "float.top"), 15.0);
    }
}

#[test]
fn nzs_all_facade_entries_keep_indefinite_child_basis_and_group_split() {
    use iced_css::evaluation::sibling_layout::{
        build_captured_sibling_layout_with_outer_height,
        build_captured_sibling_layout_with_viewport_height,
    };
    let css = pct_css(
        6,
        "#b1 { height:50%; } #b2 { clear:none; } #b3 { clear:none; }",
    );
    sibling_fixture(6, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let outer = || ClearanceOuterContentGeometry {
            outer: &projection.nodes()[0],
            content_left: 0.,
            content_width: 100.,
            content_top: 10.,
        };
        let first = build_captured_sibling_layout(&facts, outer(), layout_limits()).unwrap();
        let second = build_captured_sibling_layout_with_outer_height(
            &facts,
            outer(),
            Some(900.),
            layout_limits(),
        )
        .unwrap();
        let third = build_captured_sibling_layout_with_viewport_height(
            &facts,
            outer(),
            Some(900.),
            Some(700.),
            layout_limits(),
        )
        .unwrap();
        for layout in [&first, &second, &third] {
            assert_eq!(layout.children()[0].used_height(), 0.);
            assert_eq!(
                layout.children()[0].computed_height(),
                facts.row(2).unwrap().height
            );
            assert_eq!(layout.sequence().placed_float().top, 15.);
            assert_eq!(
                layout
                    .sequence()
                    .tail_view(0)
                    .unwrap()
                    .hypothetical_group()
                    .members()
                    .len(),
                2
            );
            assert_pct_view(
                &pct_view(layout),
                &pct_view(&first),
                &[],
                "NZS public entry parity",
            );
        }
    });
}

#[test]
fn nzs_clear_side_selection_preserves_negative_and_absent_clearance() {
    for side in ["left", "right"] {
        for clear in ["left", "right", "both", "none"] {
            let selected = clear == side || clear == "both";
            let fragment = format!(
                "#b1 {{ height:2px; }} #f {{ float:{side}; }} \
                #b2 {{ height:10px; max-height:0; margin-top:3px; margin-bottom:10px; clear:{clear}; }} \
                #b3 {{ height:1px; margin-top:2px; margin-bottom:1px; clear:none; }}"
            );
            nzs_numbers(
                6,
                &fragment,
                &[
                    ("float.left", if side == "left" { 0. } else { 90. }),
                    ("c2.border.top", if selected { 19. } else { 17. }),
                    ("c3.border.top", if selected { 29. } else { 27. }),
                    ("tail0.cleared", if selected { 1. } else { 0. }),
                    ("tail0.clearance", if selected { -1. } else { 0. }),
                    ("p.used_height", if selected { 20. } else { 18. }),
                ],
            );
        }
    }
}

#[test]
fn nzs_keeps_equality_contact_and_separation_under_signed_through() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { height:2px; } \
        #b2 { height:10px; max-height:0; margin-top:6px; clear:both; }"
        ),
        "Core(Placement { source: 4, error: UnsupportedEquality })"
    );
    assert_eq!(
        zhn_error(
            5,
            "#b1 { height:2px; } \
        #b2 { height:10px; max-height:0; margin-top:5px; margin-bottom:0; clear:none; overflow:hidden; }"
        ),
        "Core(Placement { source: 4, error: NewBfcFloatContact })"
    );
    let signed = zhn_layout(
        5,
        "#b1 { height:10px; max-height:0; margin-top:-1px; } #b2 { height:0; clear:none; }",
    );
    assert_eq!(zhn_pick(&signed, "c0.border.top"), 10.0);
    assert_eq!(zhn_pick(&signed, "float.top"), 14.0);
    assert_eq!(zhn_pick(&signed, "c2.border.top"), 14.0);
    assert_eq!(zhn_pick(&signed, "p.used_height"), 6.0);
    for height in ["0", "auto", "-0"] {
        let view = zhn_layout(6, &format!("{CTT_G1} #b2 {{ height:{height}; }}"));
        assert_eq!(zhn_pick(&view, "c3.border.top"), 23.);
    }
}

#[test]
fn nzs_new_admission_exposes_offsets_padding_later_child_and_width_in_order() {
    for (rule, expected) in [
        (
            format!(
                "#p {{ height:{PCT_F32_MAX_PX}; width:{PCT_F32_MAX_PX}; }} \
            #b1 {{ height:10px; max-height:0; position:relative; top:200%; padding-top:200%; }}"
            ),
            "Percentage { source: 2, property: \"top\", error: Unrepresentable }".to_owned(),
        ),
        (
            format!(
                "#p {{ width:{PCT_F32_MAX_PX}; }} \
            #b1 {{ height:10px; max-height:0; padding-top:200%; }} #b2 {{ overflow:scroll; }}"
            ),
            "Percentage { source: 2, property: \"padding-top\", error: Unrepresentable }"
                .to_owned(),
        ),
        (
            "#b1 { height:10px; max-height:0; } #f { width:auto; } #b2 { overflow:scroll; }"
                .to_owned(),
            pht_refusal(4, "overflow", "ChildOverflow"),
        ),
    ] {
        assert_eq!(zhn_error(5, &rule), expected, "{rule}");
    }
    assert_eq!(
        daz_held("#b1 { height:10px; max-height:0; } #f { width:auto; }"),
        pht_refusal(3, "width", "FloatWidth")
    );
    assert_eq!(
        daz_held("#b1 { height:10px; max-height:0; } #b2 { overflow:scroll; }"),
        pht_refusal(4, "overflow", "ChildOverflow")
    );
}

#[test]
fn nzs_facade_work_budget_one_short_and_exact_are_behavioral() {
    for (nodes, facade, core) in [(5, 3284, 2141), (6, 4231, 2791), (7, 5178, 3442)] {
        let css = pct_css(nodes, "#b1 { height:10px; max-height:0; }");
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
            assert_eq!(actual, expected, "NZS N{nodes}/{work}/{core_work}");
        }
    }
}

#[test]
fn nzs_float_width_after_later_nonliteral_zero_migrates() {
    assert_eq!(
        zhn_error(6, "#f { width:auto; } #b3 { height:auto; min-height:0%; }"),
        pht_refusal(3, "width", "FloatWidth")
    );
}

#[test]
fn nzs_decimal_cleared_reference_keeps_existing_publication_boundary() {
    for (bottom, height) in [
        ("0.999999940395355224609375", "0.0000000894069671630859375"),
        ("0.99999994", "0.00000008940697"),
    ] {
        let fragment = format!(
            "#b1 {{ height:1px; margin-top:0; margin-bottom:{bottom}px; }} \
             #f {{ height:{height}px; margin-top:0; margin-bottom:0; }} \
             #b2 {{ height:0; margin-top:1px; margin-bottom:1px; clear:both; }} \
             #b3 {{ height:10px; max-height:0; margin-top:0; margin-bottom:0; clear:none; }}"
        );
        let view = ctt_g8_decimal_layout(&fragment).unwrap();
        for (name, bits) in [
            ("float.top", 0xb380_0000_u32),
            ("float.bottom", 0x3300_0000),
            ("c2.border.top", 0x3300_0000),
            ("c3.border.top", 0x3300_0000),
            ("c3.border.bottom", 0x3300_0000),
            ("c3.used_height", 0x0000_0000),
            ("tail0.clearance", 0xbf7f_fffe),
            ("p.used_height", 0x4000_0000),
            ("p.content_bottom", 0x3300_0000),
        ] {
            assert_eq!(zhn_pick(&view, name).to_bits(), bits, "{name}/{fragment}");
        }
        let through = ctt_g8_decimal_layout(&format!("{fragment} #b3 {{ height:0; }}")).unwrap();
        assert_pct_view(
            &view,
            &through,
            &[],
            "NZS final separator leaves prior exact reference intact",
        );
        assert_eq!(
            format!(
                "{:?}",
                ctt_g8_decimal_layout(&format!("{fragment} #p {{ overflow:auto; }}")).unwrap_err()
            ),
            "ParentAutoOverflow { source: 4 }"
        );
    }
}

#[test]
fn nzs_height_resolution_errors_still_precede_admission() {
    let parent = format!("#p {{ height:{PCT_F32_MAX_PX}; }}");
    for (rule, property) in [
        ("height:200%; max-height:0;", "height"),
        ("height:0; min-height:200%; max-height:0;", "min-height"),
        ("height:0; min-height:0%; max-height:200%;", "max-height"),
    ] {
        let fragment = format!("{parent} #b1 {{ {rule} }}");
        assert_eq!(
            zhn_error(5, &fragment),
            format!("Percentage {{ source: 2, property: {property:?}, error: Unrepresentable }}")
        );
    }
    assert_eq!(
        zhn_error(
            5,
            &format!("{parent} #b1 {{ overflow:scroll; height:200%; max-height:0; }}")
        ),
        pht_refusal(2, "overflow", "ChildOverflow")
    );
    nzs_numbers(
        5,
        "#p { height:20px; } #b1 { height:50%; }",
        &[("c0.used_height", 10.)],
    );
}
