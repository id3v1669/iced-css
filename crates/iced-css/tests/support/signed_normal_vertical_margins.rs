use super::*;

fn snv_bfc_css(rules: &str) -> String {
    format!(
        "#o,#p,#b1,#f,#b2 {{display:block;margin:0;padding:0;clear:none;}} \
        #o {{width:100px;}} #p {{overflow:hidden;padding-top:1px;padding-bottom:1px;}} \
        #b1 {{height:2px;overflow:hidden;}} #f {{float:left;width:10px;height:2px;}} \
        #b2 {{height:1px;}} {rules}"
    )
}

#[test]
fn snv_new_bfc_checks_both_nonempty_published_intervals() {
    for (rules, b1, float, tail, height, extreme, extended) in [
        (
            "#b1 {margin-top:10px;margin-bottom:-7px;}",
            (10., 12.),
            (5., 7.),
            (5., 6.),
            7.,
            6.,
            true,
        ),
        (
            "#b1 {margin-top:10px;margin-bottom:-4px;}",
            (10., 12.),
            (8., 10.),
            (8., 9.),
            10.,
            9.,
            true,
        ),
        (
            "#b1 {margin-top:-1px;}",
            (-1., 1.),
            (1., 3.),
            (1., 2.),
            3.,
            2.,
            true,
        ),
        (
            "#b1 {margin-top:10px;margin-bottom:-1px;} #f {height:0px;}",
            (10., 12.),
            (11., 11.),
            (11., 12.),
            12.,
            12.,
            false,
        ),
        (
            "#b1 {margin-top:10px;margin-bottom:-1px;} #f {height:auto;}",
            (10., 12.),
            (11., 11.),
            (11., 12.),
            12.,
            12.,
            false,
        ),
        (
            "#b1 {margin-top:10px;margin-bottom:-1px;} #f {height:2px;margin-top:-2px;}",
            (10., 12.),
            (11., 11.),
            (11., 12.),
            12.,
            12.,
            false,
        ),
        (
            "#b1 {height:1px;margin-top:16777216px;margin-bottom:-4px;} #f {height:8px;}",
            (16777216., 16777216.),
            (16777212., 16777220.),
            (16777212., 16777213.),
            16777220.,
            16777213.,
            true,
        ),
    ] {
        for side in ["left", "right"] {
            let css = snv_bfc_css(&format!("{rules} #f {{float:{side};}}"));
            sibling_fixture(5, &css, |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
                let layout = build_captured_sibling_layout(
                    &facts,
                    ClearanceOuterContentGeometry {
                        outer: &projection.nodes()[0],
                        content_left: 0.,
                        content_width: 100.,
                        content_top: -1.,
                    },
                    layout_limits(),
                )
                .unwrap();
                let children = layout.children();
                assert_eq!((children[0].border().top, children[0].border().bottom), b1);
                let placed = layout.sequence().placed_float();
                assert_eq!((placed.top, placed.bottom), float);
                assert_eq!(
                    (children[2].border().top, children[2].border().bottom),
                    tail
                );
                assert_eq!(layout.parent().height().used_height, height);
                assert_eq!(layout.parent().height().content_bottom, height);
                let extent = layout.parent().height().auto_extent.unwrap();
                assert_eq!(
                    (extent.normal_margin_bottom, extent.float_extended),
                    (extreme, extended)
                );
            });
        }
    }
    let css = snv_bfc_css("#b1 {margin-top:10px;margin-bottom:-3px;}");
    sibling_fixture(5, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert!(matches!(
            build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: -1.,
                },
                layout_limits()
            ),
            Err(SiblingLayoutError::Core(
                ClearanceSequenceError::Placement {
                    source: 2,
                    error: ClearanceOwnerError::NewBfcFloatContact,
                }
            ))
        ));
    });
}

#[test]
fn snv_extrema_zero_floor_and_clearance_witnesses() {
    // Expected literals come from the separately reviewed integer-lattice model.
    for (n, rules, borders, height, bottom, extreme, extended) in [
        (
            5,
            "#b1 {margin-top:-4px;margin-bottom:-4px;} #b2 {margin-top:-4px;margin-bottom:-4px;} #f {height:0px;}",
            vec![(7., 8.), (4., 5.)],
            0.,
            11.,
            4.,
            false,
        ),
        (
            6,
            "#b2 {margin-top:-8px;} #b3 {margin-bottom:-8px;} #f {height:0px;}",
            vec![(11., 12.), (8., 9.), (12., 13.)],
            6.,
            17.,
            16.,
            false,
        ),
        (
            6,
            "#b2 {margin-top:-8px;} #b3 {margin-bottom:-8px;} #f {height:8px;}",
            vec![(11., 12.), (8., 9.), (12., 13.)],
            13.,
            24.,
            16.,
            true,
        ),
        (
            6,
            "#b3 {margin-top:-20px;}",
            vec![(11., 12.), (16., 17.), (0., 1.)],
            9.,
            20.,
            20.,
            false,
        ),
        (
            5,
            "#b2 {margin-top:-3px;margin-bottom:2px;height:2px;}",
            vec![(11., 12.), (13., 15.)],
            7.,
            18.,
            17.,
            true,
        ),
        (
            5,
            "#b2 {margin-top:-3px;clear:both;}",
            vec![(11., 12.), (18., 19.)],
            11.,
            22.,
            22.,
            false,
        ),
        (
            5,
            "#b1 {margin-bottom:-1px;} #b2 {clear:both;}",
            vec![(11., 12.), (14., 15.)],
            7.,
            18.,
            18.,
            false,
        ),
    ] {
        let css = format!("{} #b2,#b3 {{clear:none;}} {rules}", baseline_css());
        sibling_fixture(n, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let layout = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: 10.,
                },
                layout_limits(),
            )
            .unwrap();
            let normal: Vec<_> = layout
                .children()
                .iter()
                .filter(|child| child.kind() == PrincipalChildKind::Normal)
                .collect();
            assert_eq!(
                normal
                    .iter()
                    .map(|child| (child.border().top, child.border().bottom))
                    .collect::<Vec<_>>(),
                borders
            );
            let got = layout.parent().height();
            assert_eq!(got.used_height.to_bits(), (height as f32).to_bits());
            assert_eq!(got.content_bottom, bottom);
            let extent = got.auto_extent.unwrap();
            assert_eq!(
                (
                    extent.normal_margin_bottom,
                    extent.selected_bottom,
                    extent.float_extended
                ),
                (extreme, bottom, extended)
            );
            for child in normal {
                let row = facts.row(child.source_index()).unwrap();
                assert_eq!(child.margin_top(), row.margin[0]);
                assert_eq!(child.margin_bottom(), row.margin[2]);
            }
        });
    }
}

#[test]
fn snv_matrix_preserves_origins_geometry_containment_and_work() {
    for (n, plain_height, padded_height, core_work, facade_work) in [
        (5, 7., 17., 1373, 3026),
        (6, 12., 27., 1687, 3955),
        (7, 15., 35., 2002, 4884),
    ] {
        for side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                for padded in [false, true] {
                    for float_height in ["height:0px;", "height:auto;", ""] {
                        for explicit in [false, true] {
                            for overflow in ["hidden", "auto"] {
                                let expected = if explicit {
                                    40.
                                } else if padded {
                                    padded_height
                                } else {
                                    plain_height
                                };
                                let padding = if padded {
                                    "padding-top:2px;padding-bottom:3px;"
                                } else {
                                    ""
                                };
                                let parent_height = if explicit { "height:40px;" } else { "" };
                                let css = format!(
                                    "#o {{width:100px;direction:{direction};}} #p {{overflow:{overflow};padding-top:1px;padding-bottom:1px;{parent_height}}} \
                                     #b1 {{height:1px;margin-bottom:-1px;{padding}}} #f {{float:{side};width:10px;{float_height}}} \
                                     #b2 {{height:1px;margin-top:3px;margin-bottom:3px;{padding}}} \
                                     #b3 {{height:1px;margin-top:2px;margin-bottom:4px;{padding}}} \
                                     #b4 {{height:1px;margin-top:1px;margin-bottom:2px;{padding}}}"
                                );
                                sibling_fixture(n, &css, |projection, display| {
                                    let facts = evaluate_sibling_box_facts(
                                        projection,
                                        display,
                                        sibling_fact_limits(),
                                    )
                                    .unwrap();
                                    let outer = || ClearanceOuterContentGeometry {
                                        outer: &projection.nodes()[0],
                                        content_left: 0.,
                                        content_width: 100.,
                                        content_top: 10.,
                                    };
                                    let mut limits = layout_limits();
                                    limits.work = nz(facade_work);
                                    limits.core.work = nz(core_work);
                                    let layout =
                                        build_captured_sibling_layout(&facts, outer(), limits)
                                            .unwrap();
                                    assert_eq!(layout.parent().height().used_height, expected);
                                    assert_eq!(
                                        layout.parent().height().content_bottom,
                                        11. + expected
                                    );
                                    assert_eq!(
                                        layout.parent().height().auto_extent.is_none(),
                                        explicit
                                    );
                                    assert_eq!(layout.receipt().work, facade_work);
                                    assert_eq!(layout.receipt().core.work, core_work);
                                    let float = &layout.children()[1];
                                    assert_eq!(float.used_height().to_bits(), 0);
                                    assert_eq!(
                                        float.computed_height(),
                                        facts.row(3).unwrap().height
                                    );
                                    limits.work = nz(facade_work - 1);
                                    assert!(matches!(
                                        build_captured_sibling_layout(&facts, outer(), limits),
                                        Err(SiblingLayoutError::Work)
                                    ));
                                    limits.work = nz(facade_work);
                                    limits.core.work = nz(core_work - 1);
                                    assert!(matches!(
                                        build_captured_sibling_layout(&facts, outer(), limits),
                                        Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
                                    ));
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn snv_clearance_equality_and_containment_remain_typed() {
    for (rules, expected_source, equality) in [
        ("#b1 {margin-bottom:-1px;} #b2 {margin-top:2px;}", 4, true),
        (
            "#p {overflow:auto;} #b1 {margin-top:-4px;margin-bottom:-4px;} #b2 {margin-top:-4px;margin-bottom:-4px;clear:none;} #f {height:0px;}",
            2,
            false,
        ),
    ] {
        let css = format!("{} {rules}", baseline_css());
        sibling_fixture(5, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: 10.,
                },
                layout_limits(),
            )
            .unwrap_err();
            if equality {
                assert!(
                    matches!(error, SiblingLayoutError::Core(ClearanceSequenceError::Placement {source, error:ClearanceOwnerError::UnsupportedEquality}) if source == expected_source)
                );
            } else {
                assert!(
                    matches!(error, SiblingLayoutError::ParentAutoOverflow {source} if source == expected_source)
                );
            }
        });
    }
}
