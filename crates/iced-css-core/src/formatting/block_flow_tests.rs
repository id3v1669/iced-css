use super::*;

fn limits() -> FlowLimits {
    FlowLimits {
        boxes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(16).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
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
fn floats_ending_above_the_highest_float_top_still_bound_later_ones() {
    let g = tree(
        vec![
            FlowBox::default(),
            float(FloatSide::Left, 50.0, 10.0),
            float(FloatSide::Left, 60.0, 10.0),
            float(FloatSide::Left, 100.0, 0.0),
            float(FloatSide::Left, 30.0, 5.0),
            float(FloatSide::Right, 30.0, 5.0),
            float(FloatSide::Left, 50.0, 5.0),
        ],
        &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6)],
    );
    assert_eq!((g[1].border.left, vertical(&g[1])), (0.0, (0.0, 10.0)));
    assert_eq!((g[2].border.left, vertical(&g[2])), (0.0, (10.0, 20.0)));
    // Zero high below the second, it ends where rule 6 now starts every later float.
    assert_eq!((g[3].border.left, vertical(&g[3])), (0.0, (20.0, 20.0)));
    assert_eq!((g[4].border.left, vertical(&g[4])), (0.0, (20.0, 25.0)));
    assert_eq!((g[5].border.left, vertical(&g[5])), (70.0, (20.0, 25.0)));
    assert_eq!((g[6].border.left, vertical(&g[6])), (0.0, (25.0, 30.0)));
}

#[test]
fn float_after_collapsed_through_box_sits_at_its_top_border_edge() {
    // The user's U1=A, as the clearance owners rule: not below the larger bottom margin.
    let g = tree(
        vec![
            FlowBox::default(),
            margins(6.0, 9.0, FlowBox::default()),
            float(FloatSide::Left, 20.0, 5.0),
            float(FloatSide::Left, 20.0, 5.0),
            h(1.0),
            margins(0.0, 4.0, FlowBox::default()),
            float(FloatSide::Right, 20.0, 5.0),
        ],
        &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6)],
    );
    assert!(g[1].collapsed_through);
    assert_eq!(g[1].border.top, 6.0);
    assert_eq!(vertical(&g[2]), (6.0, 11.0));
    assert_eq!(vertical(&g[3]), (6.0, 11.0));
    assert_eq!(g[4].border.top, 9.0);
    assert!(g[5].collapsed_through);
    assert_eq!(g[5].border.top, 10.0);
    assert_eq!(vertical(&g[6]), (10.0, 15.0));
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
    let detached_cycle = vec![
        FlowBox::default(),
        FlowBox {
            children: vec![2],
            ..FlowBox::default()
        },
        FlowBox {
            children: vec![1],
            ..FlowBox::default()
        },
    ];
    assert!(matches!(
        layout_block_flow(&detached_cycle, viewport(), limits()),
        Err(FlowError::Tree { node: 1 })
    ));
    let self_loop = vec![
        FlowBox::default(),
        FlowBox {
            children: vec![1],
            ..FlowBox::default()
        },
    ];
    assert!(matches!(
        layout_block_flow(&self_loop, viewport(), limits()),
        Err(FlowError::Tree { .. })
    ));
    assert_eq!(
        layout_block_flow(&[], viewport(), limits()),
        Err(FlowError::Boxes)
    );
    let one = FlowLimits {
        boxes: NonZeroUsize::MIN,
        ..limits()
    };
    let pair = vec![
        FlowBox {
            children: vec![1],
            ..FlowBox::default()
        },
        FlowBox::default(),
    ];
    assert_eq!(
        layout_block_flow(&pair, viewport(), one),
        Err(FlowError::Boxes)
    );
    assert!(layout_block_flow(&[FlowBox::default()], viewport(), one).is_ok());
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
fn shrink_to_fit_preferred_width_adds_content_beside_a_float() {
    let text = || FlowBox {
        content: Some(FlowContent {
            height: 10.0,
            preferred_width: 40.0,
            preferred_min_width: 40.0,
        }),
        ..FlowBox::default()
    };
    let shrink = || FlowBox {
        float: Some(FloatSide::Left),
        ..FlowBox::default()
    };
    let links = [(0, 1), (1, 2), (1, 3)];
    let wide = float(FloatSide::Left, 50.0, 20.0);
    // Text sits beside the float, so the longest line holds both.
    let g = tree(
        vec![FlowBox::default(), shrink(), wide.clone(), text()],
        &links,
    );
    assert_eq!(g[1].border.right, 90.0);
    assert_eq!(g[3].border.top, 0.0);
    // So does a box that establishes a formatting context.
    let block = FlowBox {
        establishes_bfc: true,
        width: Dim::Px(40.0),
        ..h(10.0)
    };
    let g = tree(
        vec![FlowBox::default(), shrink(), wide.clone(), block],
        &links,
    );
    assert_eq!(g[1].border.right, 90.0);
    assert_eq!((g[3].border.top, g[3].border.left), (0.0, 50.0));
    // A plain block lies under the float, and cleared text starts below it.
    let plain = FlowBox {
        width: Dim::Px(40.0),
        ..h(10.0)
    };
    let g = tree(
        vec![FlowBox::default(), shrink(), wide.clone(), plain],
        &links,
    );
    assert_eq!(g[1].border.right, 50.0);
    let cleared = FlowBox {
        clear: ClearSide::Left,
        ..text()
    };
    let g = tree(vec![FlowBox::default(), shrink(), wide, cleared], &links);
    assert_eq!(g[1].border.right, 50.0);
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
fn empty_box_with_clearance_collapses_its_own_margins_into_the_next_run() {
    for (top, bottom, next) in [(10.0, 30.0, 70.0), (30.0, 10.0, 50.0), (0.0, 30.0, 80.0)] {
        let clearing = FlowBox {
            clear: ClearSide::Both,
            ..margins(top, bottom, FlowBox::default())
        };
        let g = tree(
            vec![
                FlowBox::default(),
                float(FloatSide::Left, 40.0, 50.0),
                clearing,
                h(5.0),
            ],
            &[(0, 1), (0, 2), (0, 3)],
        );
        assert_eq!(g[2].border.top, 50.0);
        assert_eq!(g[3].border.top, next);
    }
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
fn a_cell_width_below_its_content_minimum_does_not_narrow_the_column() {
    let narrow = FlowBox {
        content: Some(FlowContent {
            height: 1.0,
            preferred_width: 300.0,
            preferred_min_width: 60.0,
        }),
        ..cell(Dim::Px(10.0), 1.0)
    };
    let g = tree(
        vec![
            FlowBox::default(),
            table_of([0.0, 0.0], Dim::Auto),
            row(),
            narrow,
            cell(Dim::Px(20.0), 1.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (2, 4)],
    );
    // The first column is its content's 60 minimum, not the 10 the cell asks for.
    assert_eq!((g[3].border.left, g[3].border.right), (0.0, 60.0));
    assert_eq!((g[4].border.left, g[4].border.right), (60.0, 80.0));
    assert_eq!(g[1].border.right, 80.0);
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
fn inline_blocks_that_may_not_wrap_stay_on_one_line() {
    let strip = FlowBox {
        row_no_wrap: true,
        ..row_of(RowAlign::Left)
    };
    let g = tree(
        vec![
            FlowBox::default(),
            strip,
            inline_block(40.0, 10.0),
            inline_block(40.0, 20.0),
            inline_block(40.0, 7.0),
            h(1.0),
        ],
        &[(0, 1), (1, 2), (1, 3), (1, 4), (0, 5)],
    );
    // 120 of boxes in the same width the wrapping row above breaks in.
    assert_eq!(g[2].border.left, 0.0);
    assert_eq!(g[3].border.left, 40.0);
    assert_eq!((g[4].border.left, g[4].border.top), (80.0, 13.0));
    assert_eq!((g[1].border.top, g[1].border.bottom), (0.0, 20.0));
    assert_eq!(g[5].border.top, 20.0);
}

#[test]
fn inline_blocks_share_the_line_baseline() {
    let text = |height: f32, width: f32| FlowBox {
        establishes_bfc: true,
        line_baseline: true,
        content: Some(FlowContent {
            height,
            preferred_width: width,
            preferred_min_width: width,
        }),
        ..FlowBox::default()
    };
    // Its text 30 tall with its baseline at 24, and 10 of padding below.
    let mut boxed = inline_block(40.0, 30.0);
    boxed.height = Dim::Auto;
    boxed.line_baseline = true;
    boxed.padding[2] = Dim::Px(10.0);
    let inner = FlowBox {
        content: Some(FlowContent {
            height: 30.0,
            preferred_width: 40.0,
            preferred_min_width: 40.0,
        }),
        ..FlowBox::default()
    };
    // One that clips: its bottom margin edge for its baseline.
    let mut clipped = inline_block(10.0, 10.0);
    clipped.line_baseline = true;
    clipped.clips = true;
    // One aligned to the line's bottom.
    let bottom = inline_block(10.0, 50.0);
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        text(20.0, 30.0),
        boxed,
        inner,
        clipped,
        bottom,
    ];
    for (parent, child) in [(0, 1), (1, 2), (1, 3), (3, 4), (1, 5), (1, 6)] {
        boxes[parent].children.push(child);
    }
    let baselines = [(2, 15.0), (4, 24.0)];
    let g = layout_block_flow_baselines(
        &boxes,
        viewport(),
        limits(),
        &mut |_, _| None,
        &mut |_, _, _, _| None,
        &mut |node, _, _| {
            baselines
                .iter()
                .find(|entry| entry.0 == node)
                .map(|entry| entry.1)
        },
    )
    .unwrap();
    // 24 above the baseline at most, 16 below: the text at 9, the box at 0, the clipped
    // one at 14; the bottom one fills the 50 the line takes.
    assert_eq!(g[2].border.top, 9.0);
    assert_eq!(g[3].border.top, 0.0);
    assert_eq!(g[5].border.top, 14.0);
    assert_eq!((g[6].border.top, g[1].border.bottom), (0.0, 50.0));
    // One high on its baseline, one low: the line takes both reaches, 28 above and 28 below.
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        text(30.0, 10.0),
        text(30.0, 10.0),
    ];
    for (parent, child) in [(0, 1), (1, 2), (1, 3)] {
        boxes[parent].children.push(child);
    }
    let reach = [(2, 2.0), (3, 28.0)];
    let g = layout_block_flow_baselines(
        &boxes,
        viewport(),
        limits(),
        &mut |_, _| None,
        &mut |_, _, _, _| None,
        &mut |node, _, _| {
            reach
                .iter()
                .find(|entry| entry.0 == node)
                .map(|entry| entry.1)
        },
    )
    .unwrap();
    assert_eq!(
        (g[2].border.top, g[3].border.top, g[1].border.bottom),
        (26.0, 0.0, 56.0)
    );
    // A box out of flow last in an inline-block gives no line: the one before it does.
    let mut holder = inline_block(40.0, 40.0);
    holder.line_baseline = true;
    let mut floated = text(10.0, 10.0);
    floated.out_of_flow = true;
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        holder,
        text(10.0, 10.0),
        floated,
        text(20.0, 10.0),
    ];
    for (parent, child) in [(0, 1), (1, 2), (2, 3), (2, 4), (1, 5)] {
        boxes[parent].children.push(child);
    }
    let marks = [(3, 8.0), (4, 9.0), (5, 18.0)];
    let g = layout_block_flow_baselines(
        &boxes,
        viewport(),
        limits(),
        &mut |_, _| None,
        &mut |_, _, _, _| None,
        &mut |node, _, _| {
            marks
                .iter()
                .find(|entry| entry.0 == node)
                .map(|entry| entry.1)
        },
    )
    .unwrap();
    // Its baseline at 8: 10 above the text's 18.
    assert_eq!((g[2].border.top, g[5].border.top), (10.0, 0.0));
    let run = |boxes: &mut Vec<FlowBox>, links: &[(usize, usize)], marks: &[(usize, f32)]| {
        for &(parent, child) in links {
            boxes[parent].children.push(child);
        }
        layout_block_flow_baselines(
            boxes,
            viewport(),
            limits(),
            &mut |_, _| None,
            &mut |_, _, _, _| None,
            &mut |node, _, _| {
                marks
                    .iter()
                    .find(|entry| entry.0 == node)
                    .map(|entry| entry.1)
            },
        )
        .unwrap()
    };
    // A line its baseline makes taller than any box in it keeps clear of a float that
    // starts below the tallest: here one at 35, wider than the one above it.
    let mut boxes = vec![
        FlowBox::default(),
        float(FloatSide::Left, 60.0, 35.0),
        float(FloatSide::Left, 80.0, 35.0),
        row_of(RowAlign::Left),
        text(30.0, 10.0),
        text(30.0, 10.0),
    ];
    let g = run(
        &mut boxes,
        &[(0, 1), (0, 2), (0, 3), (3, 4), (3, 5)],
        &[(4, 2.0), (5, 28.0)],
    );
    assert_eq!(g[2].border.top, 35.0);
    assert_eq!(g[4].border.left, 80.0);
    // An inline-block holding a row of its own: that row's last line's baseline, even when
    // its last box there clips.
    let mut holder = FlowBox {
        establishes_bfc: true,
        line_baseline: true,
        width: Dim::Px(40.0),
        ..FlowBox::default()
    };
    holder.padding[0] = Dim::Px(5.0);
    let mut shut = inline_block(10.0, 30.0);
    shut.line_baseline = true;
    shut.clips = true;
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        holder,
        row_of(RowAlign::Left),
        text(10.0, 10.0),
        shut,
        text(20.0, 10.0),
    ];
    let g = run(
        &mut boxes,
        &[(0, 1), (1, 2), (2, 3), (3, 4), (3, 5), (1, 6)],
        &[(4, 8.0), (6, 18.0)],
    );
    // Inside: the clipped box's 30 on the baseline, the text 22 down; the holder's baseline
    // 5 + 30 = 35 below its top, the outer text's 18: the holder at 0, the text at 17.
    assert_eq!((g[4].border.top, g[5].border.top), (27.0, 5.0));
    assert_eq!((g[2].border.top, g[6].border.top), (0.0, 17.0));
    // An inline-table: its first row's baseline, not its last.
    let mut table = table_of([0.0, 0.0], Dim::Px(30.0));
    table.line_baseline = true;
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        table,
        row(),
        cell(Dim::Auto, 20.0),
        text(10.0, 10.0),
        row(),
        cell(Dim::Auto, 20.0),
        text(10.0, 10.0),
        text(20.0, 10.0),
    ];
    let g = run(
        &mut boxes,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (2, 6),
            (6, 7),
            (7, 8),
            (1, 9),
        ],
        &[(5, 8.0), (8, 8.0), (9, 18.0)],
    );
    // The first row's 8: the table 10 down, the text at the top.
    assert_eq!((g[2].border.top, g[9].border.top), (10.0, 0.0));
    // A hidden first row gives none: the next one does.
    boxes.iter_mut().for_each(|value| value.children.clear());
    boxes[3].hidden_row = true;
    let g = run(
        &mut boxes,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (2, 6),
            (6, 7),
            (7, 8),
            (1, 9),
        ],
        &[(5, 2.0), (8, 8.0), (9, 18.0)],
    );
    assert_eq!(g[9].border.top, 0.0);
    // A footer group first in the source still lays out last: the body's row is first.
    let mut table = table_of([0.0, 0.0], Dim::Px(30.0));
    table.line_baseline = true;
    let group = |role: TableRole| FlowBox {
        table: Some(role),
        ..FlowBox::default()
    };
    let mut boxes = vec![
        FlowBox::default(),
        row_of(RowAlign::Left),
        table,
        group(TableRole::FooterGroup),
        row(),
        cell(Dim::Auto, 20.0),
        text(10.0, 10.0),
        group(TableRole::BodyGroup),
        row(),
        cell(Dim::Auto, 20.0),
        text(10.0, 10.0),
        text(20.0, 10.0),
    ];
    let g = run(
        &mut boxes,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (2, 7),
            (7, 8),
            (8, 9),
            (9, 10),
            (1, 11),
        ],
        &[(6, 2.0), (10, 8.0), (11, 18.0)],
    );
    // The body row on top, its 8 on the text's 18: the table 10 down.
    assert_eq!((g[2].border.top, g[11].border.top), (10.0, 0.0));
    assert_eq!(
        g[2].border.top + (g[8].border.top - g[2].border.top) + 8.0,
        18.0
    );
    // Without baselines told, bottoms stand in, as before.
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    assert_eq!(g[2].border.bottom, g[11].border.bottom);
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
    // 17.4: the caption lies outside the table's box, and what follows clears it.
    assert_eq!(g[1].border.bottom, 3.0);
    assert_eq!(g[2].border.bottom, 10.0);
}

#[test]
fn captions_lie_outside_the_tables_box_with_their_margins() {
    let caption = |below| FlowBox {
        table: Some(TableRole::Caption),
        establishes_bfc: true,
        caption_below: below,
        height: Dim::Px(7.0),
        margin: [Dim::Px(1.0), Dim::Px(2.0), Dim::Px(8.0), Dim::Px(3.0)],
        ..FlowBox::default()
    };
    let g = tree(
        vec![
            FlowBox::default(),
            FlowBox {
                border: [1.0; 4],
                padding: [Dim::Px(4.0); 4],
                height: Dim::Px(20.0),
                ..table_of([0.0, 0.0], Dim::Px(50.0))
            },
            caption(false),
            row(),
            cell(Dim::Auto, 3.0),
            caption(true),
            h(1.0),
        ],
        &[(0, 1), (1, 2), (1, 3), (3, 4), (1, 5), (0, 6)],
    );
    // Above: as wide as the table's border box less its own margins, from the table's
    // border edge, with its bottom margin between it and the table's border.
    assert_eq!((g[2].border.left, g[2].border.right), (3.0, 58.0));
    assert_eq!((g[2].border.top, g[2].border.bottom), (1.0, 8.0));
    assert_eq!(g[2].margin, [1.0, 2.0, 8.0, 3.0]);
    // The table's border, padding and height hold the grid alone.
    assert_eq!((g[1].border.top, g[1].border.bottom), (16.0, 46.0));
    assert_eq!((g[1].content.top, g[1].content.bottom), (21.0, 41.0));
    assert_eq!(g[4].border.top, 21.0);
    // Below: under the table's border, and the next block under its bottom margin.
    assert_eq!((g[5].border.top, g[5].border.bottom), (47.0, 54.0));
    assert_eq!(g[6].border.top, 62.0);
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
fn spans_stop_adding_columns_past_the_grids_limit() {
    // Twelve cells of the widest span: ten fill the limit, the rest take one slot each.
    let mut boxes = vec![FlowBox::default(), table_of([0.0, 0.0], Dim::Auto), row()];
    boxes[0].children.push(1);
    boxes[1].children.push(2);
    for index in 3..15 {
        boxes.push(spanned(usize::MAX, 1, Dim::Auto, 1.0));
        boxes[2].children.push(index);
    }
    let (placed, columns) = table_grid(&boxes, 1, &[2]);
    assert_eq!(columns, MAX_COLUMNS + 2);
    assert_eq!(placed[9].col_span, MAX_SPAN);
    assert_eq!((placed[10].column, placed[10].col_span), (MAX_COLUMNS, 1));
    assert_eq!(
        (placed[11].column, placed[11].col_span),
        (MAX_COLUMNS + 1, 1)
    );
    // A span that starts under the limit ends at it.
    boxes[3].col_span = 1;
    let (placed, columns) = table_grid(&boxes, 1, &[2]);
    assert_eq!((placed[10].column, placed[10].col_span), (9001, 999));
    assert_eq!(columns, MAX_COLUMNS + 1);
    layout_block_flow(&boxes, viewport(), limits()).unwrap();
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

/// Each level holds a float, a second float below it and a box that starts a formatting
/// context around the next level: that box fits beside the first float, turns out to
/// reach the second, and is laid out again below, so every level doubles the work.
fn doubling_levels(levels: usize) -> Vec<FlowBox> {
    let mut boxes = vec![FlowBox::default()];
    let mut parent = 0;
    for _ in 0..levels {
        let first = boxes.len();
        boxes.push(float(FloatSide::Left, 1.0, 10.0));
        boxes.push(FlowBox {
            clear: ClearSide::Left,
            ..float(FloatSide::Left, 2.0, 10.0)
        });
        boxes.push(FlowBox {
            establishes_bfc: true,
            ..FlowBox::default()
        });
        boxes[parent].children.extend(first..first + 3);
        parent = first + 2;
    }
    boxes
}

#[test]
fn nested_retries_beside_floats_stop_at_the_work_limit() {
    let wide = FlowLimits {
        depth: NonZeroUsize::new(64).unwrap(),
        ..limits()
    };
    // 2^30 layouts of the innermost level without the limit.
    assert_eq!(
        layout_block_flow(&doubling_levels(30), viewport(), wide),
        Err(FlowError::Work)
    );
    // Three levels are 3 * (1 + 2 + 4) boxes laid out: well inside it.
    let g = layout_block_flow(&doubling_levels(3), viewport(), wide).unwrap();
    assert_eq!((g[3].border.top, g[3].border.left), (10.0, 2.0));
    let exact = FlowLimits {
        work: NonZeroUsize::new(21).unwrap(),
        ..wide
    };
    assert!(layout_block_flow(&doubling_levels(3), viewport(), exact).is_ok());
    let short = FlowLimits {
        work: NonZeroUsize::new(20).unwrap(),
        ..wide
    };
    assert_eq!(
        layout_block_flow(&doubling_levels(3), viewport(), short),
        Err(FlowError::Work)
    );
}

#[test]
fn only_the_first_header_and_footer_groups_move() {
    let group = |role| FlowBox {
        table: Some(role),
        ..FlowBox::default()
    };
    let mut boxes = vec![FlowBox::default(), table_of([0.0, 0.0], Dim::Auto)];
    let mut rows = Vec::new();
    for role in [
        TableRole::FooterGroup,
        TableRole::HeaderGroup,
        TableRole::BodyGroup,
        TableRole::HeaderGroup,
        TableRole::FooterGroup,
    ] {
        let at = boxes.len();
        boxes.push(group(role));
        boxes.push(row());
        boxes.push(spanned(1, 10, Dim::Auto, 1.0));
        boxes[1].children.push(at);
        boxes[at].children.push(at + 1);
        boxes[at + 1].children.push(at + 2);
        rows.push(at + 1);
    }
    boxes[0].children.push(1);
    // The first header leads and the first footer ends; the later ones are row groups
    // in source order (17.2).
    let (parts, _) = table_parts(&boxes, 1);
    assert_eq!(parts, vec![rows[1], rows[2], rows[3], rows[4], rows[0]]);
    // Each still ends its own rowspans.
    let (placed, _) = table_grid(&boxes, 1, &parts);
    assert!(placed.iter().all(|item| item.row_span == 1));
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    let tops: Vec<f32> = rows.iter().map(|row| g[*row].border.top).collect();
    assert_eq!(tops, vec![4.0, 0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn a_shrink_to_fit_box_makes_room_for_its_rows_indent() {
    let indented = FlowBox {
        first_indent: 20.0,
        ..row_of(RowAlign::Left)
    };
    let shrink = FlowBox {
        float: Some(FloatSide::Left),
        ..FlowBox::default()
    };
    let mut boxes = vec![
        FlowBox::default(),
        shrink,
        indented,
        inline_block(30.0, 10.0),
        inline_block(30.0, 10.0),
    ];
    let links = [(0, 1), (1, 2), (2, 3), (2, 4)];
    for &(parent, child) in &links {
        boxes[parent].children.push(child);
    }
    // The first line holds the indent and the first box at the narrowest.
    assert_eq!(preferred_content_widths(&boxes, 2), (50.0, 80.0));
    let g = layout_block_flow(&boxes, viewport(), limits()).unwrap();
    assert_eq!(g[1].border.right, 80.0);
    assert_eq!((g[3].border.left, g[4].border.left), (20.0, 50.0));
    assert_eq!(g[4].border.top, 0.0);
    // A negative indent sizes as without it.
    boxes[2].first_indent = -20.0;
    assert_eq!(preferred_content_widths(&boxes, 2), (30.0, 60.0));
}

#[test]
fn clear_past_a_float_in_an_open_parent_keeps_the_parent_with_its_float() {
    let clearing = FlowBox {
        clear: ClearSide::Left,
        ..margins(150.0, 0.0, h(10.0))
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
    // Moving the parent down with the margin would move the float past the box's
    // hypothetical top, so clearance (of zero) stops the margin joining the parent's.
    assert_eq!(vertical(&g[2]), (0.0, 100.0));
    assert_eq!(g[3].clearance, Some(0.0));
    assert_eq!(vertical(&g[3]), (150.0, 160.0));
    assert_eq!(vertical(&g[1]), (0.0, 160.0));
}

#[test]
fn an_empty_sibling_after_a_cleared_empty_box_keeps_its_margin_in_the_parent() {
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
            FlowBox::default(),
            h(1.0),
        ],
        &[(0, 1), (0, 2), (2, 3), (2, 4), (0, 5)],
    );
    assert!(g[4].collapsed_through);
    // As with the cleared box last: its margin stays inside the parent.
    assert_eq!(g[2].border.bottom, 60.0);
    assert_eq!(g[5].border.top, 65.0);
}

#[test]
fn geometry_that_overflows_f32_is_an_error() {
    let thick = FlowBox {
        border: [f32::MAX, 0.0, f32::MAX, 0.0],
        ..FlowBox::default()
    };
    let mut boxes = vec![FlowBox::default(), thick];
    boxes[0].children.push(1);
    assert!(matches!(
        layout_block_flow(&boxes, viewport(), limits()),
        Err(FlowError::Nonfinite { .. })
    ));
    // A finite percentage of a finite basis can overflow too.
    let boxes = [FlowBox {
        height: Dim::Percent(1e37),
        ..FlowBox::default()
    }];
    let tall = FlowViewport {
        height: Some(800.0),
        ..viewport()
    };
    assert!(matches!(
        layout_block_flow(&boxes, tall, limits()),
        Err(FlowError::Nonfinite { .. })
    ));
}

#[test]
fn border_box_sizes_lose_their_own_padding_and_border() {
    let viewport = FlowViewport {
        left: 0.0,
        top: 0.0,
        width: 400.0,
        height: Some(300.0),
    };
    let child = FlowBox {
        border_box: true,
        width: Dim::Px(100.0),
        height: Dim::Px(60.0),
        min_width: Dim::Px(0.0),
        padding: [Dim::Px(5.0), Dim::Px(10.0), Dim::Px(5.0), Dim::Px(10.0)],
        border: [1.0, 2.0, 1.0, 2.0],
        ..FlowBox::default()
    };
    let root = FlowBox {
        children: vec![1],
        ..FlowBox::default()
    };
    let border = |child: FlowBox| {
        let geometry = layout_block_flow(&[root.clone(), child], viewport, limits()).unwrap();
        let area = geometry[1].border;
        (area.right - area.left, area.bottom - area.top)
    };
    // The border box is the size given; the content is what is left.
    assert_eq!(border(child.clone()), (100.0, 60.0));
    assert_eq!(
        border(FlowBox {
            border_box: false,
            ..child.clone()
        }),
        (124.0, 72.0)
    );
    // A percentage is of the containing block, still less the frame.
    assert_eq!(
        border(FlowBox {
            width: Dim::Percent(50.0),
            ..child.clone()
        })
        .0,
        200.0
    );
    // A percentage padding comes out of a width too.
    let padded = FlowBox {
        padding: [
            Dim::Px(0.0),
            Dim::Percent(10.0),
            Dim::Px(0.0),
            Dim::Percent(10.0),
        ],
        ..child.clone()
    };
    assert_eq!(border(padded).0, 100.0);
    // Smaller than its own frame: no content, and the frame stays.
    assert_eq!(
        border(FlowBox {
            width: Dim::Px(10.0),
            height: Dim::Px(4.0),
            ..child.clone()
        }),
        (24.0, 12.0)
    );
    // Minimum and maximum are border-box sizes as well.
    assert_eq!(
        border(FlowBox {
            width: Dim::Px(300.0),
            max_width: Dim::Px(150.0),
            ..child.clone()
        })
        .0,
        150.0
    );
    assert_eq!(
        border(FlowBox {
            width: Dim::Px(50.0),
            min_width: Dim::Px(80.0),
            ..child.clone()
        })
        .0,
        80.0
    );
    // Auto is untouched, and the pass borrows when nothing asks for it.
    assert_eq!(
        border(FlowBox {
            width: Dim::Auto,
            ..child
        })
        .0,
        400.0
    );
    assert!(matches!(
        content_boxes(&[root]),
        std::borrow::Cow::Borrowed(_)
    ));
}

// CSS Flexible Box Layout Level 1.
mod flex {
    use super::*;

    fn container(flex: FlexContainer, children: usize) -> FlowBox {
        FlowBox {
            children: (1..=children).collect(),
            establishes_bfc: true,
            flex: Some(flex),
            ..FlowBox::default()
        }
    }
    fn item(width: f32, height: f32) -> FlowBox {
        FlowBox {
            width: Dim::Px(width),
            height: Dim::Px(height),
            ..FlowBox::default()
        }
    }
    fn flexing(base: FlowBox, grow: f32, shrink: f32, basis: Dim) -> FlowBox {
        FlowBox {
            flex_item: FlexItem {
                grow,
                shrink,
                basis,
                ..FlexItem::default()
            },
            ..base
        }
    }
    /// Border boxes as (left, top, width, height), of a 300 by 200 viewport's layout.
    fn laid(boxes: &[FlowBox]) -> Vec<(f32, f32, f32, f32)> {
        let viewport = FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 300.0,
            height: Some(200.0),
        };
        layout_block_flow(boxes, viewport, limits())
            .unwrap()
            .iter()
            .map(|geometry| {
                let area = geometry.border;
                (
                    area.left,
                    area.top,
                    area.right - area.left,
                    area.bottom - area.top,
                )
            })
            .collect()
    }
    fn row() -> FlexContainer {
        FlexContainer::default()
    }

    #[test]
    fn a_row_places_items_side_by_side_and_is_as_tall_as_the_tallest() {
        let got = laid(&[container(row(), 2), item(50.0, 20.0), item(80.0, 40.0)]);
        assert_eq!(got[1], (0.0, 0.0, 50.0, 20.0));
        assert_eq!(got[2], (50.0, 0.0, 80.0, 40.0));
        assert_eq!(got[0], (0.0, 0.0, 300.0, 40.0));
    }

    #[test]
    fn an_auto_height_stretches_to_the_line_and_other_alignments_place_within_it() {
        let auto = FlowBox {
            height: Dim::Auto,
            ..item(50.0, 0.0)
        };
        let got = laid(&[container(row(), 2), auto.clone(), item(80.0, 40.0)]);
        assert_eq!(got[1], (0.0, 0.0, 50.0, 40.0));
        for (align, top) in [
            (FlexAlign::Start, 0.0),
            (FlexAlign::Center, 10.0),
            (FlexAlign::End, 20.0),
        ] {
            let aligned = FlexContainer {
                align_items: align,
                ..row()
            };
            let got = laid(&[container(aligned, 2), item(50.0, 20.0), item(80.0, 40.0)]);
            assert_eq!(got[1].1, top, "{align:?}");
            // An auto height is its content's when it is not stretched: none here.
            let got = laid(&[container(aligned, 2), auto.clone(), item(80.0, 40.0)]);
            assert_eq!(got[1].3, 0.0, "{align:?}");
        }
        // `align-self` overrides the container for one item.
        let own = FlowBox {
            flex_item: FlexItem {
                align: Some(FlexAlign::End),
                ..FlexItem::default()
            },
            ..item(50.0, 20.0)
        };
        let got = laid(&[container(row(), 2), own, item(80.0, 40.0)]);
        assert_eq!(got[1].1, 20.0);
        // A container with a height: its one line is that tall.
        let tall = FlowBox {
            height: Dim::Px(100.0),
            ..container(row(), 1)
        };
        let got = laid(&[tall, auto]);
        assert_eq!(got[1].3, 100.0);
    }

    #[test]
    fn free_space_grows_items_by_their_factors() {
        let grow = |factor: f32| flexing(item(50.0, 10.0), factor, 1.0, Dim::Auto);
        let got = laid(&[container(row(), 2), grow(1.0), item(100.0, 10.0)]);
        assert_eq!(got[1].2, 200.0);
        assert_eq!(got[2].0, 200.0);
        // 200 free, shared one to two on top of each one's 50.
        let got = laid(&[container(row(), 2), grow(1.0), grow(2.0)]);
        assert!((got[1].2 - (50.0 + 200.0 / 3.0)).abs() < 0.01, "{got:?}");
        assert!((got[2].2 - (50.0 + 400.0 / 3.0)).abs() < 0.01, "{got:?}");
        // `flex-basis` replaces the width as the size to grow from; 0 shares everything.
        let zero = |factor: f32| flexing(item(50.0, 10.0), factor, 1.0, Dim::Px(0.0));
        let got = laid(&[container(row(), 2), zero(1.0), zero(3.0)]);
        assert_eq!((got[1].2, got[2].2), (75.0, 225.0));
        // A maximum stops one, and the rest take what it leaves.
        let capped = FlowBox {
            max_width: Dim::Px(80.0),
            ..grow(1.0)
        };
        let got = laid(&[container(row(), 2), capped, grow(1.0)]);
        assert_eq!((got[1].2, got[2].2), (80.0, 220.0));
        // Factors under one take only that share.
        let got = laid(&[container(row(), 1), grow(0.5)]);
        assert_eq!(got[1].2, 175.0);
    }

    #[test]
    fn overflow_shrinks_items_in_proportion_to_their_size_and_factor() {
        let got = laid(&[container(row(), 2), item(200.0, 10.0), item(200.0, 10.0)]);
        assert_eq!((got[1].2, got[2].2), (150.0, 150.0));
        // 300 over 300: 100 too many, taken one to three by size.
        let got = laid(&[container(row(), 2), item(100.0, 10.0), item(300.0, 10.0)]);
        assert_eq!((got[1].2, got[2].2), (75.0, 225.0));
        // `flex-shrink: 0` keeps its size, and the other gives it all.
        let rigid = flexing(item(200.0, 10.0), 0.0, 0.0, Dim::Auto);
        let got = laid(&[container(row(), 2), rigid, item(200.0, 10.0)]);
        assert_eq!((got[1].2, got[2].2), (200.0, 100.0));
        // A minimum stops one.
        let floored = FlowBox {
            min_width: Dim::Px(180.0),
            ..item(200.0, 10.0)
        };
        let got = laid(&[container(row(), 2), floored, item(200.0, 10.0)]);
        assert_eq!((got[1].2, got[2].2), (180.0, 120.0));
    }

    #[test]
    fn wrapping_breaks_lines_and_stacks_them() {
        let wrap = FlexContainer {
            wrap: FlexWrap::Wrap,
            ..row()
        };
        let boxes = [
            container(wrap, 3),
            item(120.0, 20.0),
            item(120.0, 30.0),
            item(120.0, 10.0),
        ];
        let got = laid(&boxes);
        assert_eq!(got[1], (0.0, 0.0, 120.0, 20.0));
        assert_eq!(got[2], (120.0, 0.0, 120.0, 30.0));
        assert_eq!(got[3], (0.0, 30.0, 120.0, 10.0));
        assert_eq!(got[0].3, 40.0);
        // Gaps: between items, and between lines.
        let gapped = FlexContainer {
            gap: [5.0, 70.0],
            ..wrap
        };
        let got = laid(&[
            container(gapped, 3),
            boxes[1].clone(),
            boxes[2].clone(),
            boxes[3].clone(),
        ]);
        // 120 + 70 + 120 is over 300: one item a line.
        assert_eq!((got[2].0, got[2].1), (0.0, 25.0));
        assert_eq!(got[3].1, 60.0);
        assert_eq!(got[0].3, 70.0);
        // `wrap-reverse` stacks from the far edge.
        let reversed = FlexContainer {
            wrap: FlexWrap::WrapReverse,
            ..row()
        };
        let got = laid(&[
            container(reversed, 3),
            boxes[1].clone(),
            boxes[2].clone(),
            boxes[3].clone(),
        ]);
        assert_eq!(got[3].1, 0.0);
        assert_eq!(got[1].1, 10.0 + 30.0 - 20.0);
    }

    #[test]
    fn justify_content_shares_free_space_along_the_line() {
        let offsets = |justify: FlexSpace| {
            let flex = FlexContainer { justify, ..row() };
            let got = laid(&[container(flex, 2), item(50.0, 10.0), item(50.0, 10.0)]);
            (got[1].0, got[2].0)
        };
        assert_eq!(offsets(FlexSpace::Start), (0.0, 50.0));
        assert_eq!(offsets(FlexSpace::End), (200.0, 250.0));
        assert_eq!(offsets(FlexSpace::Center), (100.0, 150.0));
        assert_eq!(offsets(FlexSpace::SpaceBetween), (0.0, 250.0));
        assert_eq!(offsets(FlexSpace::SpaceAround), (50.0, 200.0));
        let (first, second) = offsets(FlexSpace::SpaceEvenly);
        assert!((first - 200.0 / 3.0).abs() < 0.01 && (second - (400.0 / 3.0 + 50.0)).abs() < 0.01);
        // An auto margin takes the free space first.
        let pushed = FlowBox {
            margin: [Dim::Px(0.0), Dim::Px(0.0), Dim::Px(0.0), Dim::Auto],
            ..item(50.0, 10.0)
        };
        let got = laid(&[container(row(), 2), item(50.0, 10.0), pushed]);
        assert_eq!(got[2].0, 250.0);
    }

    #[test]
    fn order_and_reverse_change_where_items_go() {
        let ordered = FlowBox {
            flex_item: FlexItem {
                order: -1,
                ..FlexItem::default()
            },
            ..item(80.0, 10.0)
        };
        let got = laid(&[container(row(), 2), item(50.0, 10.0), ordered]);
        assert_eq!((got[2].0, got[1].0), (0.0, 80.0));
        let reverse = FlexContainer {
            reverse: true,
            ..row()
        };
        let got = laid(&[container(reverse, 2), item(50.0, 10.0), item(80.0, 10.0)]);
        assert_eq!((got[1].0, got[2].0), (250.0, 170.0));
        // A right-to-left row starts from the right, and its reverse from the left.
        let rtl = |flex: FlexContainer| FlowBox {
            direction: Direction::Rtl,
            ..container(flex, 2)
        };
        let got = laid(&[rtl(row()), item(50.0, 10.0), item(80.0, 10.0)]);
        assert_eq!((got[1].0, got[2].0), (250.0, 170.0));
        let got = laid(&[rtl(reverse), item(50.0, 10.0), item(80.0, 10.0)]);
        assert_eq!((got[1].0, got[2].0), (0.0, 50.0));
    }

    #[test]
    fn a_column_stacks_items_and_stretches_them_across() {
        let column = FlexContainer {
            column: true,
            ..row()
        };
        let wide = FlowBox {
            width: Dim::Auto,
            ..item(0.0, 20.0)
        };
        let got = laid(&[container(column, 2), wide.clone(), item(80.0, 40.0)]);
        assert_eq!(got[1], (0.0, 0.0, 300.0, 20.0));
        assert_eq!(got[2], (0.0, 20.0, 80.0, 40.0));
        assert_eq!(got[0].3, 60.0);
        // Not stretched, an auto width is its content's: none here.
        let centered = FlexContainer {
            align_items: FlexAlign::Center,
            ..column
        };
        let got = laid(&[container(centered, 2), wide.clone(), item(80.0, 40.0)]);
        assert_eq!((got[1].2, got[2].0), (0.0, 110.0));
        // With a height, free space grows and overflow shrinks along the column.
        let tall = |flex: FlexContainer, children: usize| FlowBox {
            height: Dim::Px(100.0),
            ..container(flex, children)
        };
        let growing = flexing(item(50.0, 20.0), 1.0, 1.0, Dim::Auto);
        let got = laid(&[tall(column, 2), growing, item(50.0, 30.0)]);
        assert_eq!((got[1].3, got[2].1), (70.0, 70.0));
        let got = laid(&[tall(column, 2), item(50.0, 80.0), item(50.0, 80.0)]);
        assert_eq!((got[1].3, got[2].1, got[2].3), (50.0, 50.0, 50.0));
        // `justify-content` and gaps run down; `column-reverse` starts at the bottom.
        let spaced = FlexContainer {
            justify: FlexSpace::SpaceBetween,
            ..column
        };
        let got = laid(&[tall(spaced, 2), item(50.0, 20.0), item(50.0, 30.0)]);
        assert_eq!((got[1].1, got[2].1), (0.0, 70.0));
        let upward = FlexContainer {
            reverse: true,
            gap: [10.0, 0.0],
            ..column
        };
        let got = laid(&[tall(upward, 2), item(50.0, 20.0), item(50.0, 30.0)]);
        assert_eq!((got[1].1, got[2].1), (80.0, 40.0));
    }

    #[test]
    fn margins_padding_and_borders_take_their_room() {
        let framed = FlowBox {
            margin: [Dim::Px(4.0), Dim::Px(6.0), Dim::Px(8.0), Dim::Px(10.0)],
            padding: [Dim::Px(1.0); 4],
            border: [2.0; 4],
            ..item(50.0, 20.0)
        };
        let got = laid(&[container(row(), 2), framed, item(30.0, 10.0)]);
        // Border box: 50 + 2 + 4 wide, 20 + 2 + 4 tall, after its left and top margins.
        assert_eq!(got[1], (10.0, 4.0, 56.0, 26.0));
        assert_eq!(got[2].0, 10.0 + 56.0 + 6.0);
        assert_eq!(got[0].3, 4.0 + 26.0 + 8.0);
        // The container's own padding and border move its content box.
        let padded = FlowBox {
            padding: [Dim::Px(5.0); 4],
            border: [1.0; 4],
            ..container(row(), 1)
        };
        let got = laid(&[padded, item(30.0, 10.0)]);
        assert_eq!(got[1], (6.0, 6.0, 30.0, 10.0));
        assert_eq!(got[0], (0.0, 0.0, 300.0, 22.0));
    }

    #[test]
    fn nested_content_flows_inside_an_item_at_its_flexed_width() {
        // A growing item holds a block child, which fills the item's used width.
        let boxes = [
            container(row(), 2),
            FlowBox {
                children: vec![3],
                ..flexing(FlowBox::default(), 1.0, 1.0, Dim::Px(0.0))
            },
            item(100.0, 10.0),
            FlowBox {
                height: Dim::Px(30.0),
                ..FlowBox::default()
            },
        ];
        let got = laid(&boxes);
        assert_eq!(got[1], (0.0, 0.0, 200.0, 30.0));
        assert_eq!(got[3], (0.0, 0.0, 200.0, 30.0));
        assert_eq!(got[2].0, 200.0);
        // An absolutely positioned child takes no part: its static position is the corner.
        let boxes = [
            container(row(), 2),
            FlowBox {
                out_of_flow: true,
                ..FlowBox::default()
            },
            item(100.0, 10.0),
        ];
        let got = laid(&boxes);
        assert_eq!(got[2].0, 0.0);
        assert_eq!((got[1].0, got[1].1), (0.0, 0.0));
    }
}

mod grid {
    use super::*;

    fn px(value: f32) -> TrackSize {
        TrackSize::breadth(Breadth::Fixed(Dim::Px(value)))
    }
    fn fr(value: f32) -> TrackSize {
        TrackSize::breadth(Breadth::Flex(value))
    }
    fn list(sizes: &[TrackSize]) -> TrackList {
        TrackList {
            sizes: sizes.to_vec(),
            names: vec![Vec::new(); sizes.len() + 1],
            repeat: None,
            subgrid: false,
        }
    }
    fn container(grid: GridContainer, children: usize) -> FlowBox {
        FlowBox {
            children: (1..=children).collect(),
            establishes_bfc: true,
            grid: Some(Box::new(grid)),
            ..FlowBox::default()
        }
    }
    fn item(height: f32) -> FlowBox {
        FlowBox {
            height: Dim::Px(height),
            ..FlowBox::default()
        }
    }
    fn at(base: FlowBox, row: [i32; 2], column: [i32; 2]) -> FlowBox {
        let line = |number: i32| GridLine {
            number,
            ..GridLine::default()
        };
        FlowBox {
            grid_item: Some(Box::new(GridItem {
                row: [line(row[0]), line(row[1])],
                column: [line(column[0]), line(column[1])],
                justify: None,
            })),
            ..base
        }
    }
    /// Border boxes as (left, top, width, height), of a 300 by 200 viewport's layout.
    fn laid(boxes: &[FlowBox]) -> Vec<(f32, f32, f32, f32)> {
        let viewport = FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 300.0,
            height: Some(200.0),
        };
        layout_block_flow(boxes, viewport, limits())
            .unwrap()
            .iter()
            .map(|geometry| {
                let area = geometry.border;
                (
                    area.left,
                    area.top,
                    area.right - area.left,
                    area.bottom - area.top,
                )
            })
            .collect()
    }

    #[test]
    fn nested_subgrids_lay_out_in_linear_work() {
        // One grid, then nineteen column subgrids each the only item of the one above,
        // and a leaf.
        let depth = 20;
        let mut boxes: Vec<FlowBox> = (0..depth)
            .map(|at| FlowBox {
                children: vec![at + 1],
                establishes_bfc: true,
                grid: Some(Box::new(GridContainer {
                    columns: TrackList {
                        subgrid: at > 0,
                        ..TrackList::default()
                    },
                    ..GridContainer::default()
                })),
                ..FlowBox::default()
            })
            .collect();
        boxes.push(item(10.0));
        let viewport = FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 300.0,
            height: Some(200.0),
        };
        let bounded = FlowLimits {
            boxes: NonZeroUsize::new(100).unwrap(),
            depth: NonZeroUsize::new(64).unwrap(),
            work: NonZeroUsize::new(1_000_000).unwrap(),
        };
        let geometry = layout_block_flow(&boxes, viewport, bounded).unwrap();
        let leaf = geometry[depth].border;
        assert_eq!(leaf.bottom - leaf.top, 10.0);
        assert_eq!(leaf.right - leaf.left, 300.0);
    }

    #[test]
    fn nested_row_subgrids_lay_out_in_linear_work() {
        let depth = 20;
        let mut boxes: Vec<FlowBox> = (0..depth)
            .map(|at| FlowBox {
                children: vec![at + 1],
                establishes_bfc: true,
                grid: Some(Box::new(GridContainer {
                    rows: TrackList {
                        subgrid: at > 0,
                        ..TrackList::default()
                    },
                    ..GridContainer::default()
                })),
                ..FlowBox::default()
            })
            .collect();
        boxes.push(item(10.0));
        let viewport = FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 300.0,
            height: Some(200.0),
        };
        let bounded = FlowLimits {
            boxes: NonZeroUsize::new(100).unwrap(),
            depth: NonZeroUsize::new(64).unwrap(),
            work: NonZeroUsize::new(1_000_000).unwrap(),
        };
        let geometry = layout_block_flow(&boxes, viewport, bounded).unwrap();
        let leaf = geometry[depth].border;
        assert_eq!(leaf.bottom - leaf.top, 10.0);
    }

    #[test]
    fn nested_grids_lay_out_in_linear_work() {
        // Twenty grids, each the only item of the one above, and a leaf: each lays its item
        // out and stretches it, laying it out again. That again is the same as before.
        let depth = 20;
        let mut boxes: Vec<FlowBox> = (0..depth)
            .map(|at| FlowBox {
                children: vec![at + 1],
                establishes_bfc: true,
                grid: Some(Box::new(GridContainer::default())),
                ..FlowBox::default()
            })
            .collect();
        boxes.push(item(10.0));
        let viewport = FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 300.0,
            height: Some(200.0),
        };
        let bounded = FlowLimits {
            boxes: NonZeroUsize::new(100).unwrap(),
            depth: NonZeroUsize::new(64).unwrap(),
            work: NonZeroUsize::new(1_000_000).unwrap(),
        };
        let geometry = layout_block_flow(&boxes, viewport, bounded).unwrap();
        let leaf = geometry[depth].border;
        assert_eq!(leaf.bottom - leaf.top, 10.0);
    }

    #[test]
    fn fixed_and_flexible_columns_share_the_width_and_items_flow_into_rows() {
        let grid = GridContainer {
            columns: list(&[px(100.0), fr(1.0), fr(3.0)]),
            ..GridContainer::default()
        };
        let got = laid(&[
            container(grid, 4),
            item(10.0),
            item(20.0),
            item(30.0),
            item(5.0),
        ]);
        assert_eq!(got[1], (0.0, 0.0, 100.0, 10.0));
        assert_eq!((got[2].0, got[2].2), (100.0, 50.0));
        assert_eq!((got[3].0, got[3].2), (150.0, 150.0));
        // The fourth wraps to a second row, below the tallest of the first.
        assert_eq!((got[4].0, got[4].1), (0.0, 30.0));
        assert_eq!(got[0].3, 35.0);
    }

    #[test]
    fn gaps_go_between_tracks_and_auto_items_stretch_across_their_row() {
        let grid = GridContainer {
            columns: list(&[fr(1.0), fr(1.0)]),
            gap: [5.0, 10.0],
            ..GridContainer::default()
        };
        let auto = FlowBox::default;
        let got = laid(&[container(grid, 3), item(40.0), auto(), auto()]);
        assert_eq!(got[1], (0.0, 0.0, 145.0, 40.0));
        assert_eq!(got[2], (155.0, 0.0, 145.0, 40.0));
        assert_eq!((got[3].0, got[3].1, got[3].3), (0.0, 45.0, 0.0));
    }

    #[test]
    fn line_numbers_place_items_and_auto_items_fill_around_them() {
        let grid = GridContainer {
            columns: list(&[px(50.0), px(50.0), px(50.0)]),
            rows: list(&[px(20.0), px(20.0)]),
            ..GridContainer::default()
        };
        let auto = FlowBox::default;
        // The first spans the last two columns of the second row; -1 is the last line.
        let got = laid(&[
            container(grid, 3),
            at(auto(), [2, 3], [2, -1]),
            auto(),
            auto(),
        ]);
        assert_eq!(got[1], (50.0, 20.0, 100.0, 20.0));
        assert_eq!((got[2].0, got[2].1), (0.0, 0.0));
        assert_eq!((got[3].0, got[3].1), (50.0, 0.0));
    }

    #[test]
    fn a_column_flow_fills_columns_first_and_adds_implicit_ones() {
        let grid = GridContainer {
            rows: list(&[px(20.0), px(20.0)]),
            auto_columns: vec![px(70.0)],
            column_flow: true,
            ..GridContainer::default()
        };
        let auto = FlowBox::default;
        let got = laid(&[container(grid, 3), auto(), auto(), auto()]);
        assert_eq!((got[1].0, got[1].1), (0.0, 0.0));
        assert_eq!((got[2].0, got[2].1), (0.0, 20.0));
        assert_eq!((got[3].0, got[3].1, got[3].2), (70.0, 0.0, 70.0));
    }

    #[test]
    fn auto_fill_repeats_as_many_tracks_as_fit() {
        let grid = GridContainer {
            columns: TrackList {
                sizes: Vec::new(),
                names: vec![Vec::new()],
                subgrid: false,
                repeat: Some(AutoRepeat {
                    at: 0,
                    fit: false,
                    sizes: vec![px(90.0)],
                    names: vec![Vec::new(); 2],
                    after: Vec::new(),
                }),
            },
            gap: [0.0, 10.0],
            ..GridContainer::default()
        };
        let auto = FlowBox::default;
        // 300 holds three of 90 with two gaps of 10; the fourth item wraps.
        let got = laid(&[container(grid, 4), auto(), auto(), auto(), auto()]);
        assert_eq!(got[3].0, 200.0);
        assert_eq!((got[4].0, got[4].2), (0.0, 90.0));
    }

    #[test]
    fn named_areas_place_items_by_name() {
        let areas = grid_areas(&[
            vec![Some("head".into()), Some("head".into())],
            vec![Some("side".into()), Some("main".into())],
        ]);
        let grid = GridContainer {
            columns: list(&[px(100.0), fr(1.0)]),
            areas,
            ..GridContainer::default()
        };
        let named = |name: &str| FlowBox {
            height: Dim::Px(10.0),
            grid_item: Some(Box::new(GridItem {
                row: [
                    GridLine {
                        name: Some(name.into()),
                        ..GridLine::default()
                    },
                    GridLine {
                        name: Some(name.into()),
                        ..GridLine::default()
                    },
                ],
                column: [
                    GridLine {
                        name: Some(name.into()),
                        ..GridLine::default()
                    },
                    GridLine {
                        name: Some(name.into()),
                        ..GridLine::default()
                    },
                ],
                justify: None,
            })),
            ..FlowBox::default()
        };
        let got = laid(&[
            container(grid, 3),
            named("main"),
            named("side"),
            named("head"),
        ]);
        assert_eq!(got[3], (0.0, 0.0, 300.0, 10.0));
        assert_eq!(got[2], (0.0, 10.0, 100.0, 10.0));
        assert_eq!(got[1], (100.0, 10.0, 200.0, 10.0));
    }

    #[test]
    fn auto_columns_size_to_their_content_and_alignment_places_items() {
        let grid = GridContainer {
            columns: list(&[TrackSize::AUTO, px(100.0)]),
            rows: list(&[px(50.0)]),
            justify_content: FlexSpace::Start,
            justify_items: FlexAlign::Center,
            align_items: FlexAlign::End,
            ..GridContainer::default()
        };
        let sized = |width: f32| FlowBox {
            width: Dim::Px(width),
            height: Dim::Px(10.0),
            ..FlowBox::default()
        };
        let got = laid(&[container(grid, 2), sized(30.0), sized(40.0)]);
        assert_eq!(got[1], (0.0, 40.0, 30.0, 10.0));
        assert_eq!(got[2], (60.0, 40.0, 40.0, 10.0));
    }

    #[test]
    fn a_flexible_column_is_never_narrower_than_its_content() {
        let grid = GridContainer {
            columns: list(&[fr(1.0), fr(1.0)]),
            ..GridContainer::default()
        };
        let wide = FlowBox {
            width: Dim::Px(250.0),
            height: Dim::Px(10.0),
            ..FlowBox::default()
        };
        // Many auto items flow into as many rows as they need.
        let mut boxes = vec![container(grid, 40), wide];
        boxes.extend((0..39).map(|_| item(1.0)));
        let got = laid(&boxes);
        assert_eq!(got[2].0, 250.0);
        assert_eq!(got[2].2, 50.0);
        assert_eq!(got[40].1, 10.0 + 18.0);
    }

    #[test]
    fn a_shrinking_grid_measures_its_tracks() {
        let grid = GridContainer {
            columns: list(&[px(40.0), px(60.0)]),
            gap: [0.0, 5.0],
            ..GridContainer::default()
        };
        let root = FlowBox {
            children: vec![1],
            ..FlowBox::default()
        };
        let mut shrunk = container(grid, 0);
        shrunk.children = vec![2];
        shrunk.float = Some(FloatSide::Left);
        let got = laid(&[root, shrunk, item(10.0)]);
        assert_eq!(got[1].2, 105.0);
    }
}
