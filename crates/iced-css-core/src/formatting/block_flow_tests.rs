use super::*;

fn limits() -> FlowLimits {
    FlowLimits {
        boxes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(16).unwrap(),
    }
}
fn viewport() -> FlowViewport {
    FlowViewport {
        left: 0.0,
        top: 0.0,
        width: 100.0,
        height: None,
    }
}
fn tree(mut boxes: Vec<FlowBox>, links: &[(usize, usize)]) -> Vec<FlowGeometry> {
    for &(parent, child) in links {
        boxes[parent].children.push(child);
    }
    layout_block_flow(&boxes, viewport(), limits()).unwrap()
}
fn h(height: f32) -> FlowBox {
    FlowBox {
        height: Dim::Px(height),
        ..FlowBox::default()
    }
}
fn margins(top: f32, bottom: f32, value: FlowBox) -> FlowBox {
    FlowBox {
        margin: [Dim::Px(top), Dim::Px(0.0), Dim::Px(bottom), Dim::Px(0.0)],
        ..value
    }
}
fn float(side: FloatSide, width: f32, height: f32) -> FlowBox {
    FlowBox {
        float: Some(side),
        width: Dim::Px(width),
        height: Dim::Px(height),
        ..FlowBox::default()
    }
}
fn vertical(g: &FlowGeometry) -> (f32, f32) {
    (g.border.top, g.border.bottom)
}

#[test]
fn sibling_margins_collapse_to_the_larger() {
    let g = tree(
        vec![
            FlowBox::default(),
            margins(0.0, 10.0, h(5.0)),
            margins(20.0, 0.0, h(5.0)),
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!(vertical(&g[1]), (0.0, 5.0));
    assert_eq!(vertical(&g[2]), (25.0, 30.0));
    assert_eq!(g[0].content.bottom, 30.0);
}

#[test]
fn signed_margins_sum_extrema() {
    let g = tree(
        vec![
            FlowBox::default(),
            margins(0.0, -5.0, h(10.0)),
            margins(8.0, 0.0, h(1.0)),
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!(g[2].border.top, 13.0);
}

#[test]
fn parent_and_first_child_top_margins_collapse() {
    // Root contains: spacer, P(margin 5) > C(margin 20). The block moves by 20, once.
    let g = tree(
        vec![
            FlowBox::default(),
            h(10.0),
            margins(5.0, 0.0, FlowBox::default()),
            margins(20.0, 0.0, h(7.0)),
        ],
        &[(0, 1), (0, 2), (2, 3)],
    );
    assert_eq!(g[2].border.top, 30.0);
    assert_eq!(g[3].border.top, 30.0);
    assert_eq!(g[2].border.bottom, 37.0);
}

#[test]
fn top_padding_separates_parent_and_child_margins() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                padding: [Dim::Px(2.0), Dim::Px(0.0), Dim::Px(0.0), Dim::Px(0.0)],
                ..margins(5.0, 0.0, FlowBox::default())
            },
            margins(20.0, 0.0, h(7.0)),
        ],
        &[(0, 1), (1, 2)],
    );
    assert_eq!(g[1].border.top, 5.0);
    assert_eq!(g[2].border.top, 27.0);
    assert_eq!(g[1].content.bottom, 34.0);
}

#[test]
fn last_child_bottom_margin_escapes_an_auto_parent_only() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            margins(0.0, 9.0, h(3.0)),
            h(1.0),
        ],
        &[(0, 1), (1, 2), (0, 3)],
    );
    assert_eq!(g[1].border.bottom, 3.0);
    assert_eq!(g[3].border.top, 12.0);
    // An explicit parent height keeps the child's margin inside and adds its own.
    let g = tree(
        vec![
            FlowBox::default(),
            margins(0.0, 2.0, h(4.0)),
            margins(0.0, 9.0, h(3.0)),
            h(1.0),
        ],
        &[(0, 1), (1, 2), (0, 3)],
    );
    assert_eq!(g[1].border.bottom, 4.0);
    assert_eq!(g[3].border.top, 6.0);
}

#[test]
fn empty_box_collapses_through_all_margins() {
    let g = tree(
        vec![
            FlowBox::default(),
            margins(0.0, 3.0, h(10.0)),
            margins(6.0, 9.0, FlowBox::default()),
            margins(4.0, 0.0, h(1.0)),
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert!(g[2].collapsed_through);
    assert_eq!(g[2].border.top, 16.0);
    assert_eq!(g[3].border.top, 19.0);
}

#[test]
fn bfc_root_contains_margins_and_floats() {
    let mut p = FlowBox {
        establishes_bfc: true,
        ..FlowBox::default()
    };
    p.margin[0] = Dim::Px(4.0);
    let g = tree(
        vec![
            FlowBox::default(),
            h(10.0),
            p,
            margins(6.0, 5.0, h(1.0)),
            float(FloatSide::Left, 10.0, 40.0),
        ],
        &[(0, 1), (0, 2), (2, 3), (2, 4)],
    );
    // The BFC's child margin does not collapse with it: 14 + 6.
    assert_eq!(g[2].border.top, 14.0);
    assert_eq!(g[3].border.top, 20.0);
    // Float: from the flow position 21+5=26, ends66 (10.6.7).
    assert_eq!(vertical(&g[4]), (26.0, 66.0));
    assert_eq!(g[2].border.bottom, 66.0);
}

#[test]
fn nested_float_escapes_to_the_enclosing_bfc_and_narrows_siblings_only_for_new_bfc() {
    // P > F; sibling S normal overlaps F, N is a new BFC and avoids it.
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            float(FloatSide::Left, 40.0, 30.0),
            h(5.0),
            FlowBox {
                establishes_bfc: true,
                height: Dim::Px(5.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (1, 2), (0, 3), (0, 4)],
    );
    assert_eq!(g[1].border.bottom, 0.0);
    assert_eq!((g[3].border.left, g[3].border.right), (0.0, 100.0));
    assert_eq!((g[4].border.left, g[4].border.right), (40.0, 100.0));
    assert_eq!(vertical(&g[4]), (5.0, 10.0));
}

#[test]
fn fixed_new_bfc_moves_below_a_float_that_leaves_no_room() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 40.0, 30.0),
            FlowBox {
                establishes_bfc: true,
                width: Dim::Px(70.0),
                height: Dim::Px(5.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!(g[2].border.top, 30.0);
    assert_eq!(g[2].border.left, 0.0);
}

#[test]
fn nested_clear_clears_the_outer_bfc_floats() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Right, 10.0, 25.0),
            FlowBox::default(),
            FlowBox {
                clear: ClearSide::Right,
                ..margins(3.0, 0.0, h(2.0))
            },
        ],
        &[(0, 1), (0, 2), (2, 3)],
    );
    assert_eq!(g[3].border.top, 25.0);
    assert_eq!(g[3].clearance, Some(22.0));
    // The open parent settled above the clearance, without the child's margin.
    assert_eq!(g[2].border.top, 0.0);
    assert_eq!(g[2].border.bottom, 27.0);
}

#[test]
fn clear_on_the_opposite_side_and_equal_position_adds_nothing() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 10.0, 25.0),
            FlowBox {
                clear: ClearSide::Right,
                ..h(2.0)
            },
            FlowBox {
                clear: ClearSide::Left,
                ..margins(23.0, 0.0, h(2.0))
            },
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert_eq!(g[2].clearance, None);
    assert_eq!(g[3].clearance, None);
    assert_eq!(g[3].border.top, 25.0);
}

#[test]
fn auto_width_float_shrinks_to_contents_and_percentages_resolve() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                float: Some(FloatSide::Right),
                ..FlowBox::default()
            },
            FlowBox {
                content: Some(FlowContent {
                    height: 12.0,
                    preferred_width: 30.0,
                    preferred_min_width: 10.0,
                }),
                ..FlowBox::default()
            },
            FlowBox {
                width: Dim::Percent(50.0),
                height: Dim::Px(1.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (1, 2), (0, 3)],
    );
    assert_eq!((g[1].border.left, g[1].border.right), (70.0, 100.0));
    assert_eq!(vertical(&g[2]), (0.0, 12.0));
    assert_eq!(g[3].border.right, 50.0);
}

#[test]
fn negative_margin_float_occupies_no_extent_and_keeps_its_border_box() {
    let mut f = float(FloatSide::Left, 40.0, 10.0);
    f.margin[0] = Dim::Px(-30.0);
    f.margin[1] = Dim::Px(-60.0);
    let g = tree(
        vec![
            FlowBox::default(),
            h(50.0),
            f,
            float(FloatSide::Left, 90.0, 5.0),
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert_eq!(vertical(&g[2]), (20.0, 30.0));
    assert_eq!((g[3].border.left, g[3].border.top), (0.0, 50.0));
}

#[test]
fn deep_nesting_auto_heights_and_centering() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                width: Dim::Px(60.0),
                margin: [Dim::Px(0.0), Dim::Auto, Dim::Px(0.0), Dim::Auto],
                border: [1.0, 1.0, 1.0, 1.0],
                ..FlowBox::default()
            },
            FlowBox::default(),
            h(8.0),
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    assert_eq!(g[1].border.left, 19.0);
    assert_eq!(g[1].border.right, 81.0);
    assert_eq!(g[3].border.left, 20.0);
    assert_eq!(g[3].border.right, 80.0);
    assert_eq!(g[1].border.bottom, 10.0);
}

#[test]
fn malformed_trees_and_limits_are_rejected() {
    let two = vec![
        FlowBox {
            children: vec![1, 1],
            ..FlowBox::default()
        },
        FlowBox::default(),
    ];
    assert!(matches!(
        layout_block_flow(&two, viewport(), limits()),
        Err(FlowError::Tree { node: 0 })
    ));
    let orphan = vec![FlowBox::default(), FlowBox::default()];
    assert!(matches!(
        layout_block_flow(&orphan, viewport(), limits()),
        Err(FlowError::Tree { node: 1 })
    ));
    let chain = vec![
        FlowBox {
            children: vec![1],
            ..FlowBox::default()
        },
        FlowBox::default(),
    ];
    let shallow = FlowLimits {
        depth: NonZeroUsize::MIN,
        ..limits()
    };
    assert_eq!(
        layout_block_flow(&chain, viewport(), shallow),
        Err(FlowError::Depth)
    );
    let bad = vec![h(f32::NAN)];
    assert_eq!(
        layout_block_flow(&bad, viewport(), limits()),
        Err(FlowError::Nonfinite { node: 0 })
    );
}

#[test]
fn float_never_rises_above_an_earlier_block_outer_top() {
    let g = tree(
        vec![
            FlowBox::default(),
            h(10.0),
            margins(0.0, -60.0, h(100.0)),
            float(FloatSide::Left, 40.0, 5.0),
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert_eq!(g[2].border.top, 10.0);
    assert_eq!(g[3].border.top, 50.0);
    let g = tree(
        vec![
            FlowBox::default(),
            h(10.0),
            margins(0.0, -105.0, h(100.0)),
            float(FloatSide::Left, 40.0, 5.0),
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert_eq!(g[3].border.top, 10.0);
}

#[test]
fn negative_width_margin_box_takes_no_horizontal_room() {
    let mut first = float(FloatSide::Left, 40.0, 10.0);
    first.margin[1] = Dim::Px(-60.0);
    let g = tree(
        vec![
            FlowBox::default(),
            first,
            float(FloatSide::Left, 100.0, 5.0),
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!((g[2].border.left, g[2].border.top), (0.0, 0.0));
}

#[test]
fn new_bfc_avoids_a_float_that_starts_below_its_top() {
    let mut block = FlowBox {
        establishes_bfc: true,
        width: Dim::Px(50.0),
        height: Dim::Px(50.0),
        ..FlowBox::default()
    };
    block.margin[0] = Dim::Px(-30.0);
    let g = tree(
        vec![
            FlowBox::default(),
            h(30.0),
            float(FloatSide::Left, 60.0, 10.0),
            block,
        ],
        &[(0, 1), (0, 2), (0, 3)],
    );
    assert_eq!(g[2].border.top, 30.0);
    assert_eq!(g[3].border.top, 40.0);
}

#[test]
fn shrink_to_fit_preferred_width_sums_a_row_of_floats() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                float: Some(FloatSide::Left),
                ..FlowBox::default()
            },
            float(FloatSide::Left, 30.0, 1.0),
            float(FloatSide::Left, 20.0, 1.0),
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[1].border.right, 50.0);
    assert_eq!(g[3].border.left, 30.0);
}

#[test]
fn float_in_an_open_parent_waits_for_the_parent_top() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            float(FloatSide::Left, 40.0, 20.0),
            margins(50.0, 0.0, h(5.0)),
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[1].border.top, 50.0);
    assert_eq!(g[2].border.top, 50.0);
}

#[test]
fn empty_child_of_an_open_parent_takes_the_parent_top() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            margins(0.0, 30.0, FlowBox::default()),
            h(5.0),
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[1].border.top, 30.0);
    assert_eq!(g[2].border.top, 30.0);
    assert_eq!(g[2].border.bottom, 30.0);
    assert_eq!(g[3].border.top, 30.0);
}

#[test]
fn max_height_does_not_stop_bottom_margin_collapse_but_caps_the_box() {
    let mut p = FlowBox {
        max_height: Dim::Px(100.0),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            p.clone(),
            margins(0.0, 30.0, h(10.0)),
            margins(5.0, 0.0, h(1.0)),
        ],
        &[(0, 1), (1, 2), (0, 3)],
    );
    assert_eq!(g[1].border.bottom, 10.0);
    assert_eq!(g[3].border.top, 40.0);
    p.max_height = Dim::Px(4.0);
    let g = tree(vec![FlowBox::default(), p, h(10.0)], &[(0, 1), (1, 2)]);
    assert_eq!(g[1].border.bottom, 4.0);
}

#[test]
fn auto_height_never_goes_negative() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            h(10.0),
            margins(-50.0, 0.0, h(5.0)),
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[3].border.top, -40.0);
    assert_eq!(g[1].border.bottom, 0.0);
}

#[test]
fn empty_box_with_clearance_keeps_its_bottom_margin_from_its_parent() {
    let parent = margins(0.0, 5.0, FlowBox::default());
    let empty = FlowBox {
        clear: ClearSide::Both,
        ..margins(0.0, 10.0, FlowBox::default())
    };
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 40.0, 50.0),
            parent,
            empty,
            h(1.0),
        ],
        &[(0, 1), (0, 2), (2, 3), (0, 4)],
    );
    assert_eq!(g[3].clearance, Some(50.0));
    // E's margin stays inside P (10.6.3), and only P's own margin follows it.
    assert_eq!(g[2].border.bottom, 60.0);
    assert_eq!(g[4].border.top, 65.0);
}

#[test]
fn negative_margin_float_row_keeps_preferred_at_least_minimum() {
    let mut narrow = float(FloatSide::Left, 10.0, 1.0);
    narrow.margin[3] = Dim::Px(-40.0);
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                float: Some(FloatSide::Left),
                ..FlowBox::default()
            },
            float(FloatSide::Left, 50.0, 1.0),
            narrow,
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[1].border.right, 50.0);
}

#[test]
fn new_bfc_narrowing_respects_min_width_and_ignores_over_constrained_margin() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 60.0, 30.0),
            FlowBox {
                establishes_bfc: true,
                min_width: Dim::Px(50.0),
                height: Dim::Px(5.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!(g[2].border.top, 30.0);
    assert_eq!(g[2].border.left, 0.0);
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 40.0, 30.0),
            FlowBox {
                establishes_bfc: true,
                width: Dim::Px(50.0),
                height: Dim::Px(5.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (0, 2)],
    );
    assert_eq!((g[2].border.left, g[2].border.top), (40.0, 0.0));
}

#[test]
fn preferred_width_honours_a_child_max_width() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                float: Some(FloatSide::Left),
                ..FlowBox::default()
            },
            FlowBox {
                max_width: Dim::Px(20.0),
                content: Some(FlowContent {
                    height: 1.0,
                    preferred_width: 90.0,
                    preferred_min_width: 5.0,
                }),
                ..FlowBox::default()
            },
        ],
        &[(0, 1), (1, 2)],
    );
    assert_eq!(g[1].border.right, 20.0);
}

#[test]
fn clear_sees_a_float_deferred_inside_the_same_open_parent() {
    let clearing = FlowBox {
        clear: ClearSide::Left,
        ..h(10.0)
    };
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            float(FloatSide::Left, 50.0, 100.0),
            clearing,
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[3].border.top, 100.0);
    assert_eq!(g[1].border.bottom, 110.0);
}

#[test]
fn clearfix_child_contains_the_float_and_clear_ignores_stale_bottoms() {
    let clearing = FlowBox {
        clear: ClearSide::Both,
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 10.0, 50.0),
            FlowBox::default(),
            float(FloatSide::Left, 10.0, 100.0),
            clearing,
        ],
        &[(0, 1), (0, 2), (2, 3), (2, 4)],
    );
    assert_eq!(g[4].border.top, 100.0);
    assert_eq!(g[2].border.bottom, 100.0);
}

#[test]
fn clear_on_an_uncle_sees_a_float_from_a_closed_empty_parent() {
    let clearing = FlowBox {
        clear: ClearSide::Left,
        ..h(1.0)
    };
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox::default(),
            FlowBox::default(),
            float(FloatSide::Left, 10.0, 30.0),
            clearing,
        ],
        &[(0, 1), (1, 2), (2, 3), (1, 4)],
    );
    assert_eq!(g[4].border.top, 30.0);
}

#[test]
fn leaf_height_follows_the_used_width_it_is_measured_at() {
    let mut boxes = vec![
        FlowBox::default(),
        FlowBox {
            width: Dim::Percent(50.0),
            ..FlowBox::default()
        },
        FlowBox {
            float: Some(FloatSide::Left),
            width: Dim::Px(25.0),
            ..FlowBox::default()
        },
    ];
    boxes[0].children = vec![1, 2];
    let mut seen = Vec::new();
    let g = layout_block_flow_measured(&boxes, viewport(), limits(), &mut |node, width| {
        seen.push((node, width));
        (node != 0).then_some(1000.0 / width)
    })
    .unwrap();
    assert_eq!(seen, vec![(1, 50.0), (2, 25.0)]);
    assert_eq!(g[1].border.bottom, 20.0);
    assert_eq!(g[2].border.bottom, 60.0);
}

fn text_leaf(min: f32) -> FlowBox {
    FlowBox {
        content: Some(FlowContent {
            height: 20.0,
            preferred_width: 200.0,
            preferred_min_width: min,
        }),
        ..FlowBox::default()
    }
}

#[test]
fn text_beside_a_float_narrows_and_is_remeasured_at_the_band() {
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 30.0, 25.0),
        text_leaf(10.0),
    ];
    boxes[0].children = vec![1, 2];
    let g = layout_block_flow_measured(&boxes, viewport(), limits(), &mut |node, width| {
        (node == 2).then_some(1400.0 / width)
    })
    .unwrap();
    // The box stays full width (9.5); only its content moves.
    assert_eq!((g[2].border.left, g[2].border.right), (0.0, 100.0));
    assert_eq!(
        g[2].leaf_band,
        Some(FlowRect {
            left: 30.0,
            right: 100.0,
            top: 0.0,
            bottom: 0.0
        })
    );
    assert_eq!(g[2].border.bottom, 20.0);
}

#[test]
fn text_with_no_room_beside_floats_starts_below_them() {
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 95.0, 25.0),
        text_leaf(10.0),
    ];
    boxes[0].children = vec![1, 2];
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    let band = g[2].leaf_band.unwrap();
    assert_eq!((band.left, band.top), (0.0, 25.0));
    // The gap above the first line counts toward the box.
    assert_eq!(g[2].border.bottom, 45.0);
}

#[test]
fn text_clear_of_floats_is_not_displaced() {
    let mut boxes = vec![
        FlowBox::default(),
        h(30.0),
        float(FloatSide::Right, 50.0, 5.0),
        text_leaf(10.0),
    ];
    boxes[0].children = vec![1, 2, 3];
    boxes[3].margin[0] = Dim::Px(10.0);
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    assert_eq!(g[3].leaf_band, None);
}

#[test]
fn text_avoids_a_float_deferred_in_its_own_open_parent() {
    let mut boxes = vec![
        FlowBox::default(),
        FlowBox::default(),
        float(FloatSide::Left, 30.0, 25.0),
        text_leaf(10.0),
    ];
    boxes[0].children = vec![1];
    boxes[1].children = vec![2, 3];
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    assert_eq!(g[3].leaf_band.unwrap().left, 30.0);
}

#[test]
fn wrapping_text_stays_clear_of_a_float_its_later_lines_reach() {
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 60.0, 20.0),
        float(FloatSide::Right, 60.0, 40.0),
        text_leaf(10.0),
    ];
    boxes[0].children = vec![1, 2, 3];
    // The right float lands at 20. Text is 15 tall at full width, 30 at 40 wide.
    let g = layout_block_flow_measured(&boxes, viewport(), limits(), &mut |node, width| {
        (node == 3).then_some(if width > 50.0 { 15.0 } else { 30.0 })
    })
    .unwrap();
    assert_eq!(g[2].border.top, 20.0);
    let band = g[3].leaf_band.unwrap();
    // At 0: 15 tall meets the left float, leaving 40 wide, which is 30 tall and reaches
    // the right float, leaving no room. At 20, past the left float, 40 wide fits.
    assert_eq!((band.left, band.right, band.top), (0.0, 40.0, 20.0));
    assert_eq!(g[3].border.bottom, 50.0);
}

#[test]
fn blank_text_with_clearance_counts_as_empty_for_the_margin_rule() {
    let clearing = FlowBox {
        clear: ClearSide::Both,
        ..margins(0.0, 20.0, FlowBox::default())
    };
    let mut boxes = vec![
        FlowBox::default(),
        FlowBox::default(),
        float(FloatSide::Left, 10.0, 30.0),
        clearing,
    ];
    boxes[0].children = vec![1];
    boxes[1].children = vec![2, 3];
    let g = layout_block_flow_measured(&boxes, viewport(), limits(), &mut |node, _| {
        (node == 3).then_some(0.0)
    })
    .unwrap();
    assert_eq!(g[1].border.bottom, 50.0);
}

fn cell(width: Dim, height: f32) -> FlowBox {
    FlowBox {
        table: Some(TableRole::Cell),
        establishes_bfc: true,
        width,
        height: Dim::Px(height),
        ..FlowBox::default()
    }
}
fn table_of(spacing: [f32; 2], width: Dim) -> FlowBox {
    FlowBox {
        table: Some(TableRole::Table { spacing }),
        establishes_bfc: true,
        width,
        ..FlowBox::default()
    }
}
fn row() -> FlowBox {
    FlowBox {
        table: Some(TableRole::Row),
        ..FlowBox::default()
    }
}

#[test]
fn table_columns_rows_and_spacing() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([2.0, 3.0], Dim::Auto),
            row(),
            cell(Dim::Px(10.0), 5.0),
            cell(Dim::Px(20.0), 8.0),
            row(),
            cell(Dim::Px(15.0), 4.0),
            cell(Dim::Px(5.0), 4.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4), (1, 5), (5, 6), (5, 7)],
    );
    // Columns 15 and 20, with three 2px gaps: an auto table shrinks to 41.
    assert_eq!((g[1].border.left, g[1].border.right), (0.0, 41.0));
    assert_eq!((g[3].border.left, g[3].border.right), (2.0, 17.0));
    assert_eq!((g[4].border.left, g[4].border.right), (19.0, 39.0));
    // Row heights 8 and 4, with three 3px gaps; cells fill their row.
    assert_eq!((g[3].border.top, g[3].border.bottom), (3.0, 11.0));
    assert_eq!(g[3].padding.bottom, 11.0);
    assert_eq!(g[3].content.bottom, 8.0);
    assert_eq!((g[6].border.top, g[6].border.bottom), (14.0, 18.0));
    assert_eq!(g[1].border.bottom, 21.0);
    assert_eq!((g[5].border.top, g[5].border.bottom), (14.0, 18.0));
}

#[test]
fn a_wide_table_shares_extra_room_and_a_narrow_one_shrinks_to_minimums() {
    let wide = table_of([0.0, 0.0], Dim::Px(90.0));
    let g = tree(
        vec![
            FlowBox::default(),
            wide,
            row(),
            cell(Dim::Px(10.0), 1.0),
            cell(Dim::Px(20.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    assert_eq!((g[3].border.right, g[4].border.right), (30.0, 90.0));
    let leaf = |pref: f32, min: f32| FlowBox {
        table: Some(TableRole::Cell),
        establishes_bfc: true,
        content: Some(FlowContent {
            height: 1.0,
            preferred_width: pref,
            preferred_min_width: min,
        }),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            row(),
            leaf(300.0, 20.0),
            leaf(100.0, 40.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    // 100 available, between 60 and 400: each moves 40/340 from its minimum.
    let first = g[3].border.right;
    assert!((first - (20.0 + 280.0 * 40.0 / 340.0)).abs() < 0.01);
    assert!((g[4].border.right - 100.0).abs() < 0.01);
}

#[test]
fn table_groups_order_header_first_and_footer_last_with_captions_above() {
    let group = |role| FlowBox {
        table: Some(role),
        ..FlowBox::default()
    };
    let caption = FlowBox {
        table: Some(TableRole::Caption),
        establishes_bfc: true,
        height: Dim::Px(7.0),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Px(50.0)),
            group(TableRole::FooterGroup),
            row(),
            cell(Dim::Auto, 1.0),
            group(TableRole::BodyGroup),
            row(),
            cell(Dim::Auto, 2.0),
            group(TableRole::HeaderGroup),
            row(),
            cell(Dim::Auto, 4.0),
            caption,
        ],
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (1, 5),
            (5, 6),
            (6, 7),
            (1, 8),
            (8, 9),
            (9, 10),
            (1, 11),
        ],
    );
    assert_eq!(g[11].border.top, 0.0);
    assert_eq!(g[11].border.right, 50.0);
    assert_eq!(g[10].border.top, 7.0);
    assert_eq!(g[7].border.top, 11.0);
    assert_eq!(g[4].border.top, 13.0);
    assert_eq!((g[8].border.top, g[8].border.bottom), (7.0, 11.0));
    assert_eq!((g[2].border.top, g[2].border.bottom), (13.0, 14.0));
    assert_eq!(g[1].border.bottom, 14.0);
}

#[test]
fn rtl_tables_run_columns_from_the_right_and_float_shrink_measures_them() {
    let mut rtl = table_of([0.0, 0.0], Dim::Px(50.0));
    rtl.direction = Direction::Rtl;
    let g = tree(
        vec![
            FlowBox::default(),
            rtl,
            row(),
            cell(Dim::Px(10.0), 1.0),
            cell(Dim::Px(10.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    assert_eq!(g[3].border.right, 50.0);
    assert_eq!(g[4].border.right, 25.0);
    let floating = FlowBox {
        float: Some(FloatSide::Left),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            floating,
            table_of([1.0, 0.0], Dim::Auto),
            row(),
            cell(Dim::Px(10.0), 1.0),
            cell(Dim::Px(20.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (3, 5)],
    );
    assert_eq!(g[1].border.right, 33.0);
}

fn inline_block(width: f32, height: f32) -> FlowBox {
    FlowBox {
        establishes_bfc: true,
        width: Dim::Px(width),
        height: Dim::Px(height),
        ..FlowBox::default()
    }
}
fn row_of(align: RowAlign) -> FlowBox {
    FlowBox {
        inline_row: Some(align),
        ..FlowBox::default()
    }
}

#[test]
fn inline_blocks_fill_lines_wrap_and_align_bottoms() {
    let g = tree(
        vec![
            FlowBox::default(),
            h(5.0),
            row_of(RowAlign::Left),
            inline_block(40.0, 10.0),
            inline_block(40.0, 20.0),
            inline_block(40.0, 7.0),
            h(1.0),
        ],
        &[(0, 1), (0, 2), (2, 3), (2, 4), (2, 5), (0, 6)],
    );
    assert_eq!((g[3].border.left, g[3].border.top), (0.0, 15.0));
    assert_eq!((g[4].border.left, g[4].border.top), (40.0, 5.0));
    assert_eq!((g[5].border.left, g[5].border.top), (0.0, 25.0));
    assert_eq!((g[2].border.top, g[2].border.bottom), (5.0, 32.0));
    assert_eq!(g[6].border.top, 32.0);
}

#[test]
fn inline_block_lines_center_right_and_keep_margins_and_shrink() {
    let mut first = inline_block(20.0, 10.0);
    first.margin[3] = Dim::Px(5.0);
    first.margin[1] = Dim::Px(5.0);
    first.margin[0] = Dim::Px(2.0);
    let g = tree(
        vec![FlowBox::default(), row_of(RowAlign::Center), first],
        &[(0, 1), (1, 2)],
    );
    assert_eq!((g[2].border.left, g[2].border.top), (40.0, 2.0));
    assert_eq!(g[1].border.bottom, 12.0);
    let shrink = FlowBox {
        establishes_bfc: true,
        content: Some(FlowContent {
            height: 3.0,
            preferred_width: 30.0,
            preferred_min_width: 10.0,
        }),
        ..FlowBox::default()
    };
    let g = tree(
        vec![FlowBox::default(), row_of(RowAlign::Right), shrink],
        &[(0, 1), (1, 2)],
    );
    assert_eq!((g[2].border.left, g[2].border.right), (70.0, 100.0));
    assert_eq!(g[2].border.bottom, 3.0);
    // An overlong box keeps its own line, at the start.
    let g = tree(
        vec![
            FlowBox::default(),
            row_of(RowAlign::Left),
            inline_block(500.0, 1.0),
            inline_block(5.0, 1.0),
        ],
        &[(0, 1), (1, 2), (1, 3)],
    );
    assert_eq!(g[2].border.left, 0.0);
    assert_eq!((g[3].border.left, g[3].border.top), (0.0, 1.0));
}

#[test]
fn a_table_is_never_narrower_than_its_columns_and_auto_margins_center_it() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Px(10.0)),
            row(),
            cell(Dim::Px(80.0), 1.0),
            cell(Dim::Px(80.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    assert_eq!(g[1].border.right, 160.0);
    let mut centered = table_of([0.0, 0.0], Dim::Auto);
    centered.margin[1] = Dim::Auto;
    centered.margin[3] = Dim::Auto;
    let g = tree(
        vec![
            FlowBox::default(),
            centered,
            row(),
            cell(Dim::Px(40.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    assert_eq!((g[1].border.left, g[1].border.right), (30.0, 70.0));
    // The root takes the same path, and shrinks.
    let mut root = table_of([0.0, 0.0], Dim::Auto);
    root.children = vec![1];
    let mut row_box = row();
    row_box.children = vec![2];
    let g = layout_block_flow(
        &[root, row_box, cell(Dim::Px(50.0), 1.0)],
        viewport(),
        limits(),
    )
    .unwrap();
    assert_eq!(g[0].border.right, 50.0);
}

#[test]
fn table_and_cell_heights_are_minimums_and_cells_align_content() {
    let tall = |align| FlowBox {
        cell_align: align,
        children: vec![],
        ..cell(Dim::Px(10.0), 10.0)
    };
    let mut boxes = vec![
        FlowBox::default(),
        table_of([0.0, 0.0], Dim::Auto),
        row(),
        tall(RowAlign::Left),
        tall(RowAlign::Center),
        tall(RowAlign::Right),
        cell(Dim::Px(10.0), 40.0),
        h(10.0),
        h(10.0),
    ];
    boxes[0].children = vec![1];
    boxes[1].children = vec![2];
    boxes[2].children = vec![3, 4, 5, 6];
    boxes[4].children = vec![7];
    boxes[5].children = vec![8];
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    assert_eq!(g[3].content.top, 0.0);
    assert_eq!((g[4].content.top, g[4].border.bottom), (15.0, 40.0));
    assert_eq!(g[7].border.top, 15.0);
    assert_eq!(g[5].content.top, 30.0);
    assert_eq!(g[8].border.top, 30.0);
    // Content taller than a specified height wins.
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            row(),
            FlowBox {
                children: vec![],
                ..cell(Dim::Px(10.0), 5.0)
            },
            h(30.0),
            h(1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 5)],
    );
    assert_eq!(g[3].border.bottom, 30.0);
    assert_eq!(g[5].border.top, 30.0);
}

#[test]
fn a_caption_can_sit_below_the_grid() {
    let caption = FlowBox {
        table: Some(TableRole::Caption),
        establishes_bfc: true,
        caption_below: true,
        height: Dim::Px(7.0),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Px(50.0)),
            caption,
            row(),
            cell(Dim::Auto, 3.0),
        ],
        &[(0, 1), (1, 2), (1, 3), (3, 4)],
    );
    assert_eq!(g[4].border.top, 0.0);
    assert_eq!(g[2].border.top, 3.0);
    assert_eq!(g[1].border.bottom, 10.0);
}

#[test]
fn a_cell_with_a_specified_height_still_aligns_its_content() {
    let mut boxes = vec![
        FlowBox::default(),
        table_of([0.0, 0.0], Dim::Auto),
        row(),
        FlowBox {
            cell_align: RowAlign::Center,
            padding: [Dim::Px(2.0), Dim::Px(0.0), Dim::Px(4.0), Dim::Px(0.0)],
            ..cell(Dim::Px(10.0), 100.0)
        },
        h(10.0),
    ];
    boxes[0].children = vec![1];
    boxes[1].children = vec![2];
    boxes[2].children = vec![3];
    boxes[3].children = vec![4];
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    // A 106 tall box, 100 of content room, 10 of content: 45 down from the content top.
    assert_eq!(g[3].border.bottom, 106.0);
    assert_eq!(g[4].border.top, 47.0);
    assert_eq!(g[3].content.bottom, 57.0);
}

#[test]
fn rows_avoid_floats_measure_as_their_sum_and_run_from_the_right_in_rtl() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 30.0, 20.0),
            row_of(RowAlign::Left),
            inline_block(30.0, 5.0),
            inline_block(30.0, 5.0),
            inline_block(30.0, 5.0),
        ],
        &[(0, 1), (0, 2), (2, 3), (2, 4), (2, 5)],
    );
    assert_eq!((g[3].border.left, g[3].border.top), (30.0, 0.0));
    assert_eq!(g[4].border.left, 60.0);
    // The third doesn't fit beside the float on the next line either, at 5..10 or 10..15:
    // it wraps, still clear of the float.
    assert_eq!((g[5].border.left, g[5].border.top), (30.0, 5.0));
    // A box wider than the room beside the float moves below it.
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 80.0, 20.0),
            row_of(RowAlign::Left),
            inline_block(30.0, 5.0),
        ],
        &[(0, 1), (0, 2), (2, 3)],
    );
    assert_eq!((g[3].border.left, g[3].border.top), (0.0, 20.0));
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                float: Some(FloatSide::Left),
                ..FlowBox::default()
            },
            row_of(RowAlign::Left),
            inline_block(50.0, 1.0),
            inline_block(30.0, 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    assert_eq!(g[1].border.right, 80.0);
    assert_eq!((g[4].border.left, g[4].border.top), (50.0, 0.0));
    let mut rtl = row_of(RowAlign::Right);
    rtl.direction = Direction::Rtl;
    let mut parent = FlowBox {
        direction: Direction::Rtl,
        ..FlowBox::default()
    };
    parent.children = vec![1];
    rtl.children = vec![2, 3];
    let g = layout_block_flow(
        &[
            parent,
            rtl,
            inline_block(10.0, 1.0),
            inline_block(20.0, 1.0),
        ],
        viewport(),
        limits(),
    )
    .unwrap();
    assert_eq!(g[2].border.right, 100.0);
    assert_eq!(g[3].border.right, 90.0);
    // An overlong centered box starts at the line's start.
    let g = tree(
        vec![
            FlowBox::default(),
            row_of(RowAlign::Center),
            inline_block(500.0, 1.0),
        ],
        &[(0, 1), (1, 2)],
    );
    assert_eq!(g[2].border.left, 0.0);
}

#[test]
fn a_floating_table_keeps_its_minimum_width() {
    let mut floating = table_of([0.0, 0.0], Dim::Px(10.0));
    floating.float = Some(FloatSide::Left);
    let g = tree(
        vec![
            FlowBox::default(),
            floating,
            row(),
            cell(Dim::Px(80.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    assert_eq!(g[1].border.right, 80.0);
}

fn spanned(cols: usize, rows: usize, width: Dim, height: f32) -> FlowBox {
    FlowBox {
        col_span: cols,
        row_span: rows,
        ..cell(width, height)
    }
}

#[test]
fn colspan_takes_columns_and_shares_its_shortfall() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([2.0, 0.0], Dim::Auto),
            row(),
            spanned(2, 1, Dim::Px(62.0), 1.0),
            cell(Dim::Px(5.0), 1.0),
            row(),
            cell(Dim::Px(10.0), 1.0),
            cell(Dim::Px(20.0), 1.0),
            cell(Dim::Px(5.0), 1.0),
        ],
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (2, 4),
            (1, 5),
            (5, 6),
            (5, 7),
            (5, 8),
        ],
    );
    // 62 less the inner 2px gap is 60; the columns give 30, so each gets 15 more.
    assert_eq!((g[6].border.left, g[6].border.right), (2.0, 27.0));
    assert_eq!((g[7].border.left, g[7].border.right), (29.0, 64.0));
    assert_eq!((g[3].border.left, g[3].border.right), (2.0, 64.0));
    assert_eq!((g[4].border.left, g[8].border.left), (66.0, 66.0));
    assert_eq!(g[1].border.right, 73.0);
}

#[test]
fn rowspan_holds_its_slots_and_extra_height_goes_to_its_last_row() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 1.0], Dim::Auto),
            row(),
            spanned(1, 2, Dim::Px(10.0), 30.0),
            cell(Dim::Px(20.0), 5.0),
            row(),
            cell(Dim::Px(20.0), 6.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4), (1, 5), (5, 6)],
    );
    // The second row's only cell moves past the slot the rowspan holds.
    assert_eq!((g[6].border.left, g[6].border.right), (10.0, 30.0));
    // 30 needs 5 + 1 + 24: the last row grows from 6 to 24.
    assert_eq!((g[3].border.top, g[3].border.bottom), (1.0, 31.0));
    assert_eq!((g[6].border.top, g[6].border.bottom), (7.0, 31.0));
    assert_eq!(g[1].border.bottom, 32.0);
}

#[test]
fn a_rowspan_past_the_last_row_stops_there() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            row(),
            spanned(1, 9, Dim::Px(10.0), 4.0),
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    assert_eq!(g[3].border.bottom, 4.0);
    assert_eq!(g[1].border.bottom, 4.0);
}

#[test]
fn text_widens_past_a_shorter_float_when_it_can_split() {
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 40.0, 20.0),
        text_leaf(5.0),
    ];
    boxes[0].children = vec![1, 2];
    let mut asked = Vec::new();
    let g = layout_block_flow_split(
        &boxes,
        viewport(),
        limits(),
        &mut |node, width| (node == 2).then_some(if width < 100.0 { 60.0 } else { 40.0 }),
        &mut |node, narrow, until, wide| {
            asked.push((node, narrow, until, wide));
            Some(50.0)
        },
    )
    .unwrap();
    assert_eq!(asked, vec![(2, 60.0, 20.0, 100.0)]);
    assert_eq!(g[2].border.bottom, 50.0);
    assert_eq!(g[2].leaf_band.unwrap().left, 40.0);
    let after = g[2].leaf_after.unwrap();
    assert_eq!((after.left, after.right, after.top), (0.0, 100.0, 20.0));
    // A refused split stays narrow, and text that fits beside the float never asks.
    let g = layout_block_flow_split(
        &boxes,
        viewport(),
        limits(),
        &mut |node, _| (node == 2).then_some(60.0),
        &mut |_, _, _, _| None,
    )
    .unwrap();
    assert_eq!(g[2].leaf_after, None);
    assert_eq!(g[2].border.bottom, 60.0);
    let mut asked = 0;
    let g = layout_block_flow_split(
        &boxes,
        viewport(),
        limits(),
        &mut |node, _| (node == 2).then_some(10.0),
        &mut |_, _, _, _| {
            asked += 1;
            Some(1.0)
        },
    )
    .unwrap();
    assert_eq!(asked, 0);
    assert_eq!(g[2].border.bottom, 10.0);
}

#[test]
fn a_rowspan_stops_at_its_row_group() {
    let group = |role| FlowBox {
        table: Some(role),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            group(TableRole::HeaderGroup),
            row(),
            spanned(1, 2, Dim::Px(10.0), 1.0),
            group(TableRole::BodyGroup),
            row(),
            cell(Dim::Px(10.0), 1.0),
            cell(Dim::Px(10.0), 1.0),
        ],
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (1, 5),
            (5, 6),
            (6, 7),
            (6, 8),
        ],
    );
    // The body row's cells start in the first column, with no phantom third one.
    assert_eq!(g[7].border.left, 0.0);
    assert_eq!(g[8].border.left, 10.0);
    assert_eq!(g[1].border.right, 20.0);
}

#[test]
fn absurd_spans_are_clamped() {
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            row(),
            spanned(usize::MAX, usize::MAX, Dim::Px(10.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    assert_eq!(g[1].border.bottom, 1.0);
}

#[test]
fn text_widens_past_stacked_floats_at_the_first_edge_with_more_room() {
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 80.0, 10.0),
        float(FloatSide::Left, 80.0, 10.0),
        text_leaf(5.0),
    ];
    boxes[0].children = vec![1, 2, 3];
    let mut asked = Vec::new();
    let g = layout_block_flow_split(
        &boxes,
        viewport(),
        limits(),
        &mut |node, _| (node == 3).then_some(60.0),
        &mut |_, narrow, until, wide| {
            asked.push((narrow, until, wide));
            Some(40.0)
        },
    )
    .unwrap();
    // The second float stacks below the first; only past both is there more room.
    assert_eq!(asked, vec![(20.0, 20.0, 100.0)]);
    assert_eq!(g[3].leaf_after.unwrap().top, 20.0);
}

#[test]
fn negative_spacing_overlaps_cells_without_touching_captions_or_going_negative() {
    let caption = FlowBox {
        table: Some(TableRole::Caption),
        establishes_bfc: true,
        height: Dim::Px(7.0),
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([-2.0, -1.0], Dim::Auto),
            caption,
            row(),
            cell(Dim::Px(10.0), 5.0),
            cell(Dim::Px(10.0), 5.0),
            row(),
            cell(Dim::Px(10.0), 5.0),
        ],
        &[(0, 1), (1, 2), (1, 3), (3, 4), (3, 5), (1, 6), (6, 7)],
    );
    assert_eq!((g[1].border.left, g[1].border.right), (0.0, 18.0));
    assert_eq!((g[4].border.left, g[5].border.left), (0.0, 8.0));
    // The grid starts right under the caption, flush with the table.
    assert_eq!(g[4].border.top, 7.0);
    assert_eq!(g[7].border.top, 11.0);
    assert_eq!(g[1].border.bottom, 16.0);
    // Empty cells don't make a negative width, floating or not.
    let empty = FlowBox {
        table: Some(TableRole::Cell),
        establishes_bfc: true,
        ..FlowBox::default()
    };
    let mut floating = table_of([-2.0, -2.0], Dim::Auto);
    floating.float = Some(FloatSide::Left);
    let g = tree(
        vec![FlowBox::default(), floating, row(), empty.clone(), empty],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    assert_eq!(g[1].border.right, 0.0);
}

#[test]
fn collapsed_overlap_skips_empty_tracks() {
    let empty = FlowBox {
        table: Some(TableRole::Cell),
        establishes_bfc: true,
        ..FlowBox::default()
    };
    // An empty first column and first row: nothing overlaps past the table's edge.
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([-10.0, -10.0], Dim::Auto),
            row(),
            empty.clone(),
            empty.clone(),
            row(),
            empty,
            cell(Dim::Px(30.0), 25.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4), (1, 5), (5, 6), (5, 7)],
    );
    assert_eq!(g[7].border.left, 0.0);
    assert_eq!(g[7].border.top, 0.0);
    assert_eq!(g[1].border.bottom, 25.0);
    assert_eq!(g[1].border.right, 30.0);
}

#[test]
fn mixed_lengths_resolve_their_percent_against_the_basis() {
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                // 10 pixels and a quarter of the width; a height against an indefinite
                // one is auto, as a percentage's is.
                width: Dim::Mix(10.0, 25.0),
                height: Dim::Mix(5.0, 50.0),
                margin: [
                    Dim::Px(0.0),
                    Dim::Px(0.0),
                    Dim::Px(0.0),
                    Dim::Mix(2.0, 10.0),
                ],
                content: Some(FlowContent {
                    height: 7.0,
                    preferred_width: 1.0,
                    preferred_min_width: 1.0,
                }),
                ..FlowBox::default()
            },
        ],
        &[(0, 1)],
    );
    let width = viewport().width;
    assert_eq!(g[1].border.left, 2.0 + width * 0.1);
    assert_eq!(g[1].border.right - g[1].border.left, 10.0 + width * 0.25);
    assert_eq!(vertical(&g[1]), (0.0, 7.0));
}

#[test]
fn mixed_heights_take_a_share_of_a_definite_height_and_must_be_finite() {
    let g = tree(
        vec![
            FlowBox {
                height: Dim::Px(400.0),
                ..FlowBox::default()
            },
            FlowBox {
                height: Dim::Mix(5.0, 50.0),
                ..FlowBox::default()
            },
        ],
        &[(0, 1)],
    );
    assert_eq!(vertical(&g[1]), (0.0, 205.0));
    for bad in [Dim::Mix(f32::NAN, 0.0), Dim::Mix(0.0, f32::INFINITY)] {
        let boxes = vec![FlowBox {
            width: bad,
            ..FlowBox::default()
        }];
        assert_eq!(
            layout_block_flow(&boxes, viewport(), limits()),
            Err(FlowError::Nonfinite { node: 0 })
        );
    }
}
