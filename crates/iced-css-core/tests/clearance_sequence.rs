use iced_css_core::{
    formatting::{
        block::AutoLength,
        clearance::ClearSide,
        clearance_owner::{
            BlockVertical, ChildContext, ChildFacts, ClearanceOwnerError, ClearanceOwnerInput,
            MarginGroupId, MarginMember, MarginSide, OuterFacts, OwnerContent, OwnerEdges,
            OwnerGenerated, OwnerHeight, OwnerProfileReason, ParentBfc, ParentFacts,
        },
        clearance_sequence::{
            ClearanceSequenceError, ClearanceSequenceInput, ClearanceSequenceLimits,
            SequenceNormalPlacement, build_clearance_sequence,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        floats::Band,
    },
    identity::IdAllocator,
    source::{Metadata, SourceNode},
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};

fn sources(count: usize) -> Vec<SourceNode> {
    let mut ids = IdAllocator::new(NonZeroU64::new(41).unwrap());
    let mut nodes: Vec<_> = (0..count)
        .map(|_| SourceNode {
            instance: ids.allocate().unwrap(),
            parent: None,
            children: vec![],
            metadata: Metadata::default(),
        })
        .collect();
    let parent_id = nodes[1].instance;
    nodes[0].children.push(parent_id);
    nodes[1].parent = Some(nodes[0].instance);
    nodes[1].children = nodes[2..].iter().map(|node| node.instance).collect();
    let parent = nodes[1].instance;
    for node in &mut nodes[2..] {
        node.parent = Some(parent);
    }
    nodes
}

fn edges() -> OwnerEdges {
    OwnerEdges {
        top: 0.,
        right: 0.,
        bottom: 0.,
        left: 0.,
    }
}

fn display() -> DisplayResolution {
    DisplayResolution {
        display: Display::Block,
        position: Position::Static,
        float: Float::None,
        flow: Flow::Normal,
    }
}

fn facts(sources: &[SourceNode]) -> (OuterFacts<'_>, ParentFacts<'_>, Vec<ChildFacts<'_>>) {
    let outer = OuterFacts {
        source: &sources[0],
        display: display(),
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
    };
    let parent = ParentFacts {
        source: &sources[1],
        display: display(),
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
        bfc: ParentBfc::NewWithEmptyPrefix,
        content_top: 11.,
        containing: Band {
            left: 0.,
            right: 100.,
        },
        border: OwnerEdges {
            top: 1.,
            bottom: 1.,
            ..edges()
        },
        padding: edges(),
    };
    let mut children: Vec<_> = sources[2..]
        .iter()
        .map(|source| ChildFacts {
            source,
            display: display(),
            context: ChildContext::OrdinaryNonBfc,
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            height: OwnerHeight::Explicit(1.),
            margin: edges().into(),
            border: edges(),
            padding: edges(),
            clear: ClearSide::None,
        })
        .collect();
    children[0].margin.bottom = AutoLength::Value(4.);
    children[1].display.float = Float::Left;
    children[1].display.flow = Flow::Floating;
    children[1].context = ChildContext::FloatBfc;
    children[1].height = OwnerHeight::Explicit(2.);
    children[2].margin.top = AutoLength::Value(3.);
    children[2].margin.bottom = AutoLength::Value(3.);
    children[2].clear = ClearSide::Both;
    if children.len() > 3 {
        children[3].margin.top = AutoLength::Value(2.);
        children[3].margin.bottom = AutoLength::Value(4.);
        children[3].clear = ClearSide::Both;
    }
    if children.len() > 4 {
        children[4].height = OwnerHeight::Explicit(2.);
        children[4].margin.top = AutoLength::Value(1.);
        children[4].margin.bottom = AutoLength::Value(2.);
    }
    (outer, parent, children)
}

fn limits(count: usize) -> ClearanceSequenceLimits {
    ClearanceSequenceLimits {
        sources: NonZeroUsize::new(count).unwrap(),
        work: NonZeroUsize::new(10_000).unwrap(),
        retained_bytes: NonZeroUsize::new(10_000).unwrap(),
    }
}

fn build<'s>(
    sources: &'s [SourceNode],
    outer: OuterFacts<'s>,
    parent: ParentFacts<'s>,
    children: &[ChildFacts<'s>],
    limits: ClearanceSequenceLimits,
) -> Result<
    iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'s>,
    ClearanceSequenceError,
> {
    build_clearance_sequence(
        sources,
        ClearanceSequenceInput {
            outer,
            parent,
            children,
            float_width: 10.,
        },
        limits,
    )
}

fn build_width<'s>(
    sources: &'s [SourceNode],
    outer: OuterFacts<'s>,
    parent: ParentFacts<'s>,
    children: &[ChildFacts<'s>],
    float_width: f32,
    limits: ClearanceSequenceLimits,
) -> Result<
    iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'s>,
    ClearanceSequenceError,
> {
    build_clearance_sequence(
        sources,
        ClearanceSequenceInput {
            outer,
            parent,
            children,
            float_width,
        },
        limits,
    )
}

#[test]
fn oversized_core_float_uses_the_existing_overflow_placement() {
    let sources = sources(6);
    let (outer, parent, children) = facts(&sources);
    let owner = build_clearance_sequence(
        &sources,
        ClearanceSequenceInput {
            outer,
            parent,
            children: &children,
            float_width: 140.0,
        },
        limits(6),
    )
    .unwrap();
    let placed = owner.placed_float();
    assert_eq!(
        (placed.left, placed.right, placed.top, placed.bottom),
        (0.0, 140.0, 16.0, 18.0)
    );
    assert_eq!(
        (
            owner.tail()[1].vertical().top,
            owner.tail()[1].vertical().bottom
        ),
        (22.0, 23.0)
    );
}

#[test]
fn oversized_core_matrix_changes_only_the_float_horizontal_placement() {
    for count in [5, 6, 7] {
        for side in [Float::Left, Float::Right] {
            for (left, right) in [
                (AutoLength::Value(0.0), AutoLength::Value(0.0)),
                (AutoLength::Auto, AutoLength::Value(0.0)),
                (AutoLength::Value(0.0), AutoLength::Auto),
                (AutoLength::Auto, AutoLength::Auto),
            ] {
                for width in [101.0, 140.0] {
                    for height in [2.0, 40.0] {
                        let sources = sources(count);
                        let (outer, parent, mut children) = facts(&sources);
                        children[1].display.float = side;
                        children[1].margin.left = left;
                        children[1].margin.right = right;
                        children[1].height = OwnerHeight::Explicit(height);
                        let oversized =
                            build_width(&sources, outer, parent, &children, width, limits(count))
                                .unwrap();
                        let control =
                            build_width(&sources, outer, parent, &children, 10.0, limits(count))
                                .unwrap();
                        let placed = oversized.placed_float();
                        assert_eq!(
                            (placed.left, placed.right, placed.top, placed.bottom),
                            if side == Float::Left {
                                (0.0, width, 16.0, 16.0 + height)
                            } else {
                                (100.0 - width, 100.0, 16.0, 16.0 + height)
                            }
                        );
                        assert_eq!(oversized.before(), control.before());
                        assert_eq!(oversized.receipt(), control.receipt());
                        assert_eq!(oversized.tail().len(), control.tail().len());
                        for (actual, expected) in oversized.tail().iter().zip(control.tail().iter())
                        {
                            assert_eq!(actual.source_index(), expected.source_index());
                            assert_eq!(actual.vertical(), expected.vertical());
                            assert_eq!(actual.hypothetical_top(), expected.hypothetical_top());
                            assert_eq!(actual.zero_clearance_top(), expected.zero_clearance_top());
                            assert_eq!(actual.clearance(), expected.clearance());
                            assert_eq!(
                                actual.barrier_source_index(),
                                expected.barrier_source_index()
                            );
                            assert_eq!(actual.hypothetical_group(), expected.hypothetical_group());
                            assert_eq!(actual.actual_groups(), expected.actual_groups());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn six_and_seven_sources_publish_each_real_tail_in_order() {
    for (count, expected) in [
        (6, &[(4, 18., 19.), (5, 22., 23.)][..]),
        (7, &[(4, 18., 19.), (5, 22., 23.), (6, 27., 29.)][..]),
    ] {
        let sources = sources(count);
        let owner = {
            let (outer, parent, children) = facts(&sources);
            build_clearance_sequence(
                &sources,
                ClearanceSequenceInput {
                    outer,
                    parent,
                    children: &children,
                    float_width: 10.,
                },
                limits(count),
            )
            .unwrap()
        };
        assert!(std::ptr::eq(owner.sources(), sources.as_slice()));
        assert_eq!((owner.before().top, owner.before().bottom), (11., 12.));
        assert_eq!(
            (owner.placed_float().top, owner.placed_float().bottom),
            (16., 18.)
        );
        assert_eq!(owner.tail().len(), expected.len());
        for (record, &(source, top, bottom)) in owner.tail().iter().zip(expected) {
            assert_eq!(record.source_index(), source);
            assert_eq!(
                (record.vertical().top, record.vertical().bottom),
                (top, bottom)
            );
            assert!(std::ptr::eq(
                owner.source(source).unwrap(),
                &sources[source]
            ));
        }
        assert_eq!(owner.tail().last().unwrap().source_index(), count - 1);
    }
}

#[test]
fn five_source_sequence_matches_the_legacy_nested_owner() {
    let sources = sources(5);
    let (outer, parent, children) = facts(&sources);
    let legacy = iced_css_core::formatting::clearance_owner::build_nested_clearance_owner(
        &sources,
        outer,
        ClearanceOwnerInput {
            parent,
            before: children[0],
            float: children[1],
            after: children[2],
            float_width: 10.,
        },
    )
    .unwrap();
    let dynamic = build(&sources, outer, parent, &children, limits(5)).unwrap();
    let after = &dynamic.tail()[0];
    assert_eq!(dynamic.before(), legacy.before());
    assert_eq!(dynamic.placed_float(), legacy.placed_float());
    assert_eq!(after.vertical(), legacy.after());
    assert_eq!(after.hypothetical_top(), legacy.hypothetical_top());
    assert_eq!(after.zero_clearance_top(), legacy.zero_clearance_top());
    assert_eq!(after.clearance(), legacy.clearance());
    assert_eq!(after.hypothetical_group(), legacy.hypothetical_group());
    assert_eq!(after.actual_groups(), legacy.actual_groups());
    assert_eq!(after.barrier_source_index(), legacy.barrier_source_index());
}

#[test]
fn dynamic_sequence_admits_signed_horizontal_margins_while_legacy_stays_strict() {
    for (child_index, source) in [(0, 2), (2, 4)] {
        for edge in ["left", "right"] {
            for (value, reason) in [
                (1.0, OwnerProfileReason::HorizontalMargin),
                (-1.0, OwnerProfileReason::NegativeMargin),
            ] {
                let sources = sources(5);
                let (outer, parent, mut children) = facts(&sources);
                if edge == "left" {
                    children[child_index].margin.left = AutoLength::Value(value);
                } else {
                    children[child_index].margin.right = AutoLength::Value(value);
                }
                assert!(build(&sources, outer, parent, &children, limits(5)).is_ok());
                assert_eq!(
                    iced_css_core::formatting::clearance_owner::build_nested_clearance_owner(
                        &sources,
                        outer,
                        ClearanceOwnerInput {
                            parent,
                            before: children[0],
                            float: children[1],
                            after: children[2],
                            float_width: 10.0,
                        },
                    )
                    .unwrap_err(),
                    ClearanceOwnerError::Unsupported { source, reason }
                );
            }
        }
    }

    let source_nodes = sources(5);
    let (outer, parent, mut children) = facts(&source_nodes);
    children[0].margin.left = AutoLength::Auto;
    assert!(build(&source_nodes, outer, parent, &children, limits(5)).is_ok());
    assert_eq!(
        iced_css_core::formatting::clearance_owner::build_nested_clearance_owner(
            &source_nodes,
            outer,
            ClearanceOwnerInput {
                parent,
                before: children[0],
                float: children[1],
                after: children[2],
                float_width: 10.0,
            },
        )
        .unwrap_err(),
        ClearanceOwnerError::Unsupported {
            source: 2,
            reason: OwnerProfileReason::AutoHorizontalMargin,
        }
    );

    let source_nodes = sources(5);
    let (outer, parent, mut children) = facts(&source_nodes);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    assert!(build(&source_nodes, outer, parent, &children, limits(5)).is_ok());
    assert_eq!(
        iced_css_core::formatting::clearance_owner::build_nested_clearance_owner(
            &source_nodes,
            outer,
            ClearanceOwnerInput {
                parent,
                before: children[0],
                float: children[1],
                after: children[2],
                float_width: 10.0,
            },
        )
        .unwrap_err(),
        ClearanceOwnerError::Unsupported {
            source: 3,
            reason: OwnerProfileReason::AutoHorizontalMargin,
        }
    );
}

#[test]
fn every_tail_record_owns_local_pair_membership_and_barrier_identity() {
    let sources = sources(7);
    let (outer, parent, children) = facts(&sources);
    let owner = build(&sources, outer, parent, &children, limits(7)).unwrap();
    for (index, record) in owner.tail().iter().enumerate() {
        let source = index + 4;
        let previous_source = [2, 4, 5][index];
        let previous_child = [0, 2, 3][index];
        let expected = [
            MarginMember {
                source_index: previous_source,
                side: MarginSide::Bottom,
                value: match children[previous_child].margin.bottom {
                    AutoLength::Auto => 0.0,
                    AutoLength::Value(value) => value,
                },
            },
            MarginMember {
                source_index: source,
                side: MarginSide::Top,
                value: match children[source - 2].margin.top {
                    AutoLength::Auto => 0.0,
                    AutoLength::Value(value) => value,
                },
            },
        ];
        assert_eq!(
            record.hypothetical_group().id(),
            MarginGroupId::HypotheticalPair
        );
        assert_eq!(record.hypothetical_group().members(), expected);
        if source == 4 {
            assert_eq!(record.barrier_source_index(), Some(4));
            assert_eq!(record.actual_groups().len(), 2);
            assert_eq!(record.actual_groups()[0].id(), MarginGroupId::BeforeBarrier);
            assert_eq!(record.actual_groups()[1].id(), MarginGroupId::AfterBarrier);
        } else {
            assert_eq!(record.barrier_source_index(), None);
            assert_eq!(record.actual_groups().len(), 1);
            assert_eq!(record.actual_groups()[0].id(), MarginGroupId::ActualPair);
        }
    }
}

#[test]
fn vertical_auto_core_margin_is_a_zero_group_member() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[2].margin.top = AutoLength::Auto;
    children[3].margin.bottom = AutoLength::Auto;
    let owner = build_clearance_sequence(
        &sources,
        ClearanceSequenceInput {
            outer,
            parent,
            children: &children,
            float_width: 10.0,
        },
        limits(6),
    )
    .unwrap();
    assert_eq!(owner.tail()[0].vertical().top, 18.0);
    assert_eq!(owner.tail()[0].vertical().bottom, 19.0);
    assert_eq!(owner.tail()[1].vertical().bottom, 23.0);
    assert_eq!(children[2].margin.top, AutoLength::Auto);
    assert_eq!(children[2].margin.used_top().to_bits(), 0.0_f32.to_bits());

    let first = &owner.tail()[0];
    assert_eq!(
        first.hypothetical_group().members(),
        [
            MarginMember {
                source_index: 2,
                side: MarginSide::Bottom,
                value: 4.0,
            },
            MarginMember {
                source_index: 4,
                side: MarginSide::Top,
                value: 0.0,
            },
        ]
    );
    assert_eq!(first.hypothetical_group().positive(), 4.0);
    assert_eq!(first.hypothetical_group().negative(), 0.0);
    assert_eq!(first.hypothetical_top(), 16.0);
    assert_eq!(first.zero_clearance_top(), Some(16.0));
    assert_eq!(first.clearance(), Some(2.0));
    assert_eq!(first.actual_groups()[0].positive(), 4.0);
    assert_eq!(first.actual_groups()[0].negative(), 0.0);
    assert_eq!(first.actual_groups()[1].positive(), 0.0);
    assert_eq!(first.actual_groups()[1].negative(), 0.0);
}

#[test]
fn signed_normal_horizontal_facts_preserve_vertical_sequence() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.left = AutoLength::Value(-10.0);
    children[0].margin.right = AutoLength::Value(20.0);
    children[3].margin.left = AutoLength::Value(80.0);
    children[3].margin.right = AutoLength::Value(30.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!((owner.before().top, owner.before().bottom), (11.0, 12.0));
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (16.0, 18.0)
    );
    assert_eq!(owner.tail()[1].source_index(), 5);
    assert_eq!(
        (
            owner.tail()[1].vertical().top,
            owner.tail()[1].vertical().bottom
        ),
        (22.0, 23.0)
    );
    assert_eq!(children[0].margin.left, AutoLength::Value(-10.0));
    assert_eq!(children[3].margin.right, AutoLength::Value(30.0));
}

#[test]
fn real_auto_horizontal_facts_preserve_vertical_sequence() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.left = AutoLength::Auto;
    children[0].margin.right = AutoLength::Auto;
    children[3].margin.left = AutoLength::Auto;
    children[3].margin.right = AutoLength::Value(120.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!((owner.before().top, owner.before().bottom), (11.0, 12.0));
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (16.0, 18.0)
    );
    assert_eq!(
        (
            owner.tail()[1].vertical().top,
            owner.tail()[1].vertical().bottom
        ),
        (22.0, 23.0)
    );
    assert_eq!(children[0].margin.left, AutoLength::Auto);
    assert_eq!(children[3].margin.right, AutoLength::Value(120.0));
}

#[test]
fn genuine_float_auto_facts_preserve_vertical_sequence() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    let placed = owner.placed_float();
    assert_eq!(
        (placed.left, placed.right, placed.top, placed.bottom),
        (0.0, 10.0, 16.0, 18.0)
    );
    assert_eq!(
        (
            owner.tail()[1].vertical().top,
            owner.tail()[1].vertical().bottom
        ),
        (22.0, 23.0)
    );
    assert_eq!(children[1].margin.left, AutoLength::Auto);
    assert_eq!(children[1].margin.right, AutoLength::Auto);
}

#[test]
fn strict_auto_margin_errors_preserve_numeric_precedence() {
    for source in [2, 5] {
        for (left, right) in [
            (AutoLength::Auto, AutoLength::Value(0.0)),
            (AutoLength::Value(0.0), AutoLength::Auto),
            (AutoLength::Auto, AutoLength::Auto),
            (AutoLength::Auto, AutoLength::Value(-1.0)),
            (AutoLength::Auto, AutoLength::Value(1.0)),
        ] {
            let source_nodes = sources(6);
            let (outer, parent, mut children) = facts(&source_nodes);
            children[source - 2].margin.left = left;
            children[source - 2].margin.right = right;
            assert!(build(&source_nodes, outer, parent, &children, limits(6)).is_ok());
        }

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let source_nodes = sources(6);
            let (outer, parent, mut children) = facts(&source_nodes);
            children[source - 2].margin.left = AutoLength::Auto;
            children[source - 2].margin.right = AutoLength::Value(bad);
            assert!(matches!(
                build(&source_nodes, outer, parent, &children, limits(6)),
                Err(ClearanceSequenceError::Profile(
                    ClearanceOwnerError::InvalidNumber {
                        source: actual_source,
                        field: "margin",
                    }
                )) if actual_source == source
            ));
        }

        let source_nodes = sources(6);
        let (outer, parent, mut children) = facts(&source_nodes);
        children[source - 2].margin.left = AutoLength::Auto;
        children[source - 2].margin.top = AutoLength::Value(-1.0);
        snv_assert_core_oracle(
            &build(&source_nodes, outer, parent, &children, limits(6)).unwrap(),
            if source == 2 { "F" } else { "A" },
        );

        let (outer, parent, mut children) = facts(&source_nodes);
        children[source - 2].margin.left = AutoLength::Auto;
        children[source - 2].border.left = 1.0;
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: actual_source,
                    reason: OwnerProfileReason::ChildDecoration,
                }
            )) if actual_source == source
        ));

        let (outer, parent, mut children) = facts(&source_nodes);
        children[source - 2].margin.left = AutoLength::Auto;
        children[source - 2].height = OwnerHeight::Explicit(0.0);
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: actual_source,
                    reason: OwnerProfileReason::NonpositiveBlockHeight,
                }
            )) if actual_source == source
        ));
    }

    let source_nodes = sources(6);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (outer, parent, mut children) = facts(&source_nodes);
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Value(bad);
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::InvalidNumber {
                    source: 3,
                    field: "margin",
                }
            ))
        ));
    }
    // A negative float margin is admitted while both outer extents stay nonnegative (SFM, C1):
    // margin box 0..9, content -1..9.
    let (outer, parent, mut children) = facts(&source_nodes);
    children[1].margin.left = AutoLength::Value(-1.0);
    assert_eq!(
        sfm_core_view(&build(&source_nodes, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 9.0, 16.0, 18.0],
            [-1.0, 9.0, 16.0, 18.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)]
        )
    );
    // A nonzero vertical float margin is admitted: margin box 16..19, content 17..19.
    let (outer, parent, mut children) = facts(&source_nodes);
    children[1].margin.top = AutoLength::Value(1.0);
    let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
    let placed = owner.placed_float();
    assert_eq!(
        (placed.left, placed.right, placed.top, placed.bottom),
        (0.0, 10.0, 16.0, 19.0)
    );
    assert_eq!(
        owner.float_content_vertical(),
        BlockVertical {
            top: 17.0,
            bottom: 19.0
        }
    );
    let (outer, parent, mut children) = facts(&source_nodes);
    children[1].margin.left = AutoLength::Value(1.0);
    let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
    assert_eq!(
        (owner.placed_float().left, owner.placed_float().right),
        (0.0, 11.0)
    );
    assert_eq!(
        owner.float_content(),
        Band {
            left: 1.0,
            right: 11.0
        }
    );
}

#[test]
fn float_auto_error_table_preserves_dynamic_profile_precedence() {
    let source_nodes = sources(6);
    for (left, right) in [
        (AutoLength::Auto, AutoLength::Value(0.0)),
        (AutoLength::Value(0.0), AutoLength::Auto),
        (AutoLength::Auto, AutoLength::Auto),
    ] {
        let (outer, parent, mut children) = facts(&source_nodes);
        children[1].margin.left = left;
        children[1].margin.right = right;
        assert!(build(&source_nodes, outer, parent, &children, limits(6)).is_ok());
    }

    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for edge in 0..4 {
            let (outer, parent, mut children) = facts(&source_nodes);
            let float = &mut children[1];
            if edge == 3 {
                float.margin.right = AutoLength::Auto;
            } else {
                float.margin.left = AutoLength::Auto;
            }
            match edge {
                0 => float.margin.top = AutoLength::Value(bad),
                1 => float.margin.right = AutoLength::Value(bad),
                2 => float.margin.bottom = AutoLength::Value(bad),
                _ => float.margin.left = AutoLength::Value(bad),
            }
            assert!(matches!(
                build(&source_nodes, outer, parent, &children, limits(6)),
                Err(ClearanceSequenceError::Profile(
                    ClearanceOwnerError::InvalidNumber {
                        source: 3,
                        field: "margin",
                    }
                ))
            ));
        }
    }

    // A negative value on any edge is admitted while both outer extents stay nonnegative
    // (SFM); a positive value takes an admitted-success branch below.
    for value in [-1.0, 1.0] {
        for edge in 0..4 {
            let (outer, parent, mut children) = facts(&source_nodes);
            let float = &mut children[1];
            if edge != 3 {
                float.margin.left = AutoLength::Auto;
            } else {
                float.margin.right = AutoLength::Auto;
            }
            match edge {
                0 => float.margin.top = AutoLength::Value(value),
                1 => float.margin.right = AutoLength::Value(value),
                2 => float.margin.bottom = AutoLength::Value(value),
                _ => float.margin.left = AutoLength::Value(value),
            }
            let result = build(&source_nodes, outer, parent, &children, limits(6));
            if value > 0.0 && (edge == 0 || edge == 2) {
                // A positive vertical float margin is admitted: margin box 16..19,
                // content 17..19 (top) or 16..18 (bottom).
                let owner = result.unwrap();
                let content = if edge == 0 {
                    (17.0, 19.0)
                } else {
                    (16.0, 18.0)
                };
                assert_eq!(
                    (owner.placed_float().top, owner.placed_float().bottom),
                    (16.0, 19.0)
                );
                let span = owner.float_content_vertical();
                assert_eq!((span.top, span.bottom), content);
                continue;
            }
            if value > 0.0 && (edge == 1 || edge == 3) {
                // A positive horizontal float margin is admitted; the other
                // horizontal edge is Auto (used zero).
                let owner = result.unwrap();
                let content = if edge == 3 { (1.0, 11.0) } else { (0.0, 10.0) };
                assert_eq!(
                    (owner.placed_float().left, owner.placed_float().right),
                    (0.0, 11.0)
                );
                assert_eq!(
                    (owner.float_content().left, owner.float_content().right),
                    content
                );
                continue;
            }
            // SFM: top -1 (C11), right -1 (C10), bottom -1 (C8a) or left -1 (C1), the other
            // horizontal edge Auto.
            let expected = [
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 10.0, 16.0, 17.0],
                    [0.0, 10.0, 15.0, 17.0],
                    &[(17.0, 18.0, Some(-2.0)), (21.0, 22.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 9.0, 16.0, 18.0],
                    [0.0, 10.0, 16.0, 18.0],
                    &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 10.0, 16.0, 17.0],
                    [0.0, 10.0, 16.0, 18.0],
                    &[(17.0, 18.0, Some(-2.0)), (21.0, 22.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 9.0, 16.0, 18.0],
                    [-1.0, 9.0, 16.0, 18.0],
                    &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
                ),
            ];
            assert_eq!(sfm_core_view(&result.unwrap()), expected[edge]);
        }
    }

    let (outer, parent, mut children) = facts(&source_nodes);
    children[1].margin.left = AutoLength::Auto;
    children[1].border.left = 1.0;
    assert!(matches!(
        build(&source_nodes, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::ChildDecoration,
            }
        ))
    ));
    // Leading clear on the float is admitted and inert: the owner equals the same
    // input with `clear: None`.
    let (outer, parent, mut control) = facts(&source_nodes);
    control[1].margin.left = AutoLength::Auto;
    let mut children = control.clone();
    children[1].clear = ClearSide::Left;
    let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
    let control = build(&source_nodes, outer, parent, &control, limits(6)).unwrap();
    assert_eq!(format!("{owner:?}"), format!("{control:?}"));

    for (fault, reason) in [
        (0, OwnerProfileReason::HeightKind),
        (1, OwnerProfileReason::ChildContext),
        (2, OwnerProfileReason::DisplayOrFlow),
        (3, OwnerProfileReason::Content),
    ] {
        let (outer, parent, mut children) = facts(&source_nodes);
        let float = &mut children[1];
        float.margin.left = AutoLength::Auto;
        float.margin.right = AutoLength::Value(1.0);
        match fault {
            0 => float.height = OwnerHeight::Percentage(0.0),
            1 => float.context = ChildContext::Unknown,
            2 => float.display.display = Display::Inline,
            _ => float.content = OwnerContent::TextOrRichText,
        }
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: actual,
                }
            )) if actual == reason
        ));
    }
}

#[test]
fn signed_normal_margin_validation_is_finite_ordered_and_source_typed() {
    for source in [2, 5] {
        for edge in ["left", "right"] {
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let sources = sources(6);
                let (outer, parent, mut children) = facts(&sources);
                let margin = &mut children[source - 2].margin;
                if edge == "left" {
                    margin.left = AutoLength::Value(bad);
                } else {
                    margin.right = AutoLength::Value(bad);
                }
                assert!(matches!(
                    build(&sources, outer, parent, &children, limits(6)),
                    Err(ClearanceSequenceError::Profile(
                        ClearanceOwnerError::InvalidNumber {
                            source: actual_source,
                            field: "margin",
                        }
                    )) if actual_source == source
                ));
            }
        }

        let source_nodes = sources(6);
        let (outer, parent, mut children) = facts(&source_nodes);
        children[source - 2].margin.left = AutoLength::Value(-f32::MAX);
        children[source - 2].margin.right = AutoLength::Value(f32::MAX);
        assert!(build(&source_nodes, outer, parent, &children, limits(6)).is_ok());

        for edge in ["top", "bottom"] {
            let source_nodes = sources(6);
            let (outer, parent, mut children) = facts(&source_nodes);
            if edge == "top" {
                children[source - 2].margin.top = AutoLength::Value(-1.0);
            } else {
                children[source - 2].margin.bottom = AutoLength::Value(-1.0);
            }
            let oracle = match (source, edge) {
                (2, "top") => "F",
                (2, "bottom") => "G",
                (5, "top") => "A",
                (5, "bottom") => "B",
                _ => unreachable!(),
            };
            snv_assert_core_oracle(
                &build(&source_nodes, outer, parent, &children, limits(6)).unwrap(),
                oracle,
            );
        }
    }

    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.top = AutoLength::Value(-1.0);
    children[0].margin.right = AutoLength::Value(f32::INFINITY);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 2,
                field: "margin",
            }
        ))
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[0].context = ChildContext::Unknown;
    children[0].margin.left = AutoLength::Value(f32::NAN);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 2,
                reason: OwnerProfileReason::ChildContext,
            }
        ))
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[0].height = OwnerHeight::Auto;
    children[0].margin.left = AutoLength::Value(f32::NAN);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 2,
                reason: OwnerProfileReason::HeightKind,
            }
        ))
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.left = AutoLength::Value(f32::NAN);
    children[0].padding.left = 1.0;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 2,
                field: "margin",
            }
        ))
    ));

    // LC-D-001 (discriminating, LC-P-010): B1 `clear` is no longer a fault, so it
    // masks nothing; a later source-5 fault is reported (34dbc294 reported source 2
    // ClearOnBeforeOrFloat).
    let (outer, parent, mut children) = facts(&sources);
    children[0].clear = ClearSide::Left;
    children[3].border.left = 1.0;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 5,
                reason: OwnerProfileReason::ChildDecoration,
            }
        ))
    ));
    // B1 `clear` alone is admitted: the owner equals the `clear: None` owner.
    let (outer, parent, control) = facts(&sources);
    let mut children = control.clone();
    children[0].clear = ClearSide::Left;
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    let control = build(&sources, outer, parent, &control, limits(6)).unwrap();
    assert_eq!(format!("{owner:?}"), format!("{control:?}"));

    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(1.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!(
        (owner.placed_float().left, owner.placed_float().right),
        (0.0, 11.0)
    );

    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(1.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (16.0, 19.0)
    );

    // A negative float margin is admitted while both outer extents stay nonnegative (SFM, C1).
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-1.0);
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 9.0, 16.0, 18.0],
            [-1.0, 9.0, 16.0, 18.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)]
        )
    );
}

#[test]
fn complete_profile_validation_precedes_bytes_and_numeric_publication() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.content_top = f32::MAX;
    children[0].margin.top = AutoLength::Value(f32::MAX);
    children[0].margin.left = AutoLength::Auto;
    children[3].height = OwnerHeight::Auto;
    let tiny_bytes = ClearanceSequenceLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits(6)
    };
    assert!(matches!(
        build(&sources, outer, parent, &children, tiny_bytes),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 5,
                reason: OwnerProfileReason::HeightKind,
            }
        ))
    ));
}

#[test]
fn float_width_domain_storage_and_placement_follow_the_documented_phase_order() {
    let source_nodes = sources(6);
    let (outer, parent, mut children) = facts(&source_nodes);
    children[3].margin.top = AutoLength::Value(-1.0);
    assert!(matches!(
        build_width(&source_nodes, outer, parent, &children, f32::NAN, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 3,
                field: "float_width"
            }
        ))
    ));
    snv_assert_core_oracle(
        &build_width(&source_nodes, outer, parent, &children, 10.0, limits(6)).unwrap(),
        "A",
    );
    children[3].margin.bottom = AutoLength::Value(f32::NAN);
    assert!(matches!(
        build_width(&source_nodes, outer, parent, &children, f32::NAN, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 5,
                field: "margin"
            }
        ))
    ));

    for width in [f32::NAN, -1.0] {
        let (outer, parent, children) = facts(&source_nodes);
        let short = ClearanceSequenceLimits {
            retained_bytes: NonZeroUsize::MIN,
            ..limits(6)
        };
        assert!(matches!(
            build_width(&source_nodes, outer, parent, &children, width, short),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::InvalidNumber {
                    source: 3,
                    field: "float_width",
                }
            ))
        ));
    }

    let (outer, parent, children) = facts(&source_nodes);
    let short = ClearanceSequenceLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits(6)
    };
    assert!(matches!(
        build_width(&source_nodes, outer, parent, &children, 140.0, short),
        Err(ClearanceSequenceError::Bytes)
    ));

    let sources = sources(5);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = Band {
        left: f32::MAX,
        right: f32::MAX,
    };
    children[1].height = OwnerHeight::Explicit(0.0);
    children[1].display.float = Float::Left;
    assert!(matches!(
        build_width(&sources, outer, parent, &children, f32::MAX, limits(5),),
        Err(ClearanceSequenceError::Placement {
            source: 3,
            error: ClearanceOwnerError::FloatPlacement(
                iced_css_core::formatting::floats::FloatError::Unrepresentable
            ),
        })
    ));
    children[1].display.float = Float::Right;
    assert!(matches!(
        build_width(&sources, outer, parent, &children, f32::MAX, limits(5),),
        Err(ClearanceSequenceError::Placement {
            source: 4,
            error: ClearanceOwnerError::UnsupportedEquality,
        })
    ));
}

#[test]
fn source_pointer_checks_reject_equal_ids_from_unrelated_storage() {
    let actual = sources(6);
    let other = sources(6);
    assert_eq!(actual[5].instance, other[5].instance);
    let (outer, parent, mut children) = facts(&actual);
    children[3].source = &other[5];
    assert!(matches!(
        build(&actual, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::SourcePointer { source: 5 }
        ))
    ));
}

#[test]
fn source_fact_work_and_requested_byte_boundaries_are_exact() {
    let sources6 = sources(6);
    let (outer, parent, children) = facts(&sources6);
    let mut limit = limits(6);
    limit.sources = NonZeroUsize::new(5).unwrap();
    assert!(matches!(
        build(&sources6, outer, parent, &children, limit),
        Err(ClearanceSequenceError::Sources)
    ));

    for (count, work, old_work) in [(5, 945, 419), (6, 1107, 501), (7, 1270, 584)] {
        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work - 1).unwrap();
        assert!(matches!(
            build(&sources, outer, parent, &children, limit),
            Err(ClearanceSequenceError::Work)
        ));
        limit.work = NonZeroUsize::new(old_work).unwrap();
        assert!(matches!(
            build(&sources, outer, parent, &children, limit),
            Err(ClearanceSequenceError::Work)
        ));
        limit.work = NonZeroUsize::new(work).unwrap();
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        let oversized = build_width(&sources, outer, parent, &children, 140.0, limit).unwrap();
        let child_count = count - 2;
        let tail_count = count - 4;
        assert_eq!(owner.receipt().nodes, count);
        assert_eq!(owner.receipt().tail, tail_count);
        assert_eq!(owner.receipt().work, work);
        assert_eq!(oversized.receipt(), owner.receipt());
        assert_eq!(
            work,
            560 + 8 * count
                + count * (count - 1) / 2
                + 24 * tail_count
                + 61 * child_count
                + 64 * (count - 3)
        );
        assert!(owner.receipt().record_capacity >= tail_count);
        assert_eq!(
            owner.receipt().retained_bytes,
            owner.receipt().record_capacity * size_of::<SequenceNormalPlacement>()
        );
    }

    let floor = 2 * size_of::<SequenceNormalPlacement>();
    let mut limit = limits(6);
    limit.retained_bytes = NonZeroUsize::new(floor - 1).unwrap();
    assert!(matches!(
        build(&sources6, outer, parent, &children, limit),
        Err(ClearanceSequenceError::Bytes)
    ));
    let mut wrong = children.clone();
    wrong.pop();
    assert!(matches!(
        build(&sources6, outer, parent, &wrong, limits(6)),
        Err(ClearanceSequenceError::Facts)
    ));
}

#[test]
fn placement_errors_identify_first_float_and_actual_tail_sources() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.content_top = f32::MAX;
    children[0].margin.top = AutoLength::Value(f32::MAX);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Placement {
            source: 2,
            error: ClearanceOwnerError::Unrepresentable {
                stage: "before_top"
            },
        })
    ));

    let (outer, mut parent, mut children) = facts(&sources);
    parent.content_top = f32::MAX;
    children[0].margin.bottom = AutoLength::Value(f32::MAX);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Placement {
            source: 3,
            error: ClearanceOwnerError::Unrepresentable { stage: "float_top" },
        })
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[2].clear = ClearSide::None;
    children[2].height = OwnerHeight::Explicit(f32::MAX / 2.);
    children[3].clear = ClearSide::Both;
    children[3].margin.top = AutoLength::Value(0.);
    children[3].height = OwnerHeight::Explicit(f32::MAX);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Placement { source: 5, .. })
    ));
}

#[test]
fn work_graph_and_profile_failures_follow_the_documented_order() {
    let mut malformed = sources(6);
    let unrelated = malformed[0].instance;
    malformed[5].children.push(unrelated);
    let (outer, parent, mut children) = facts(&malformed);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    let mut short = limits(6);
    short.work = NonZeroUsize::new(1106).unwrap();
    assert!(matches!(
        build(&malformed, outer, parent, &children, short),
        Err(ClearanceSequenceError::Work)
    ));
    assert!(matches!(
        build(&malformed, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::ChildCount { source: 5 }
        ))
    ));

    let sources = sources(6);
    let (mut outer, mut parent, mut children) = facts(&sources);
    outer.content = OwnerContent::TextOrRichText;
    parent.bfc = ParentBfc::OtherOrUnknown;
    children[0].height = OwnerHeight::Auto;
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 0,
                reason: OwnerProfileReason::Content,
            }
        ))
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[0].height = OwnerHeight::Auto;
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    children[3].margin.top = AutoLength::Value(f32::NAN);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 2,
                reason: OwnerProfileReason::HeightKind,
            }
        ))
    ));

    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.left = AutoLength::Auto;
    children[3].margin.right = AutoLength::Auto;
    children[3].border.left = 1.0;
    let mut one_short = limits(6);
    one_short.work = NonZeroUsize::new(1106).unwrap();
    assert!(matches!(
        build(&sources, outer, parent, &children, one_short),
        Err(ClearanceSequenceError::Work)
    ));
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 5,
                reason: OwnerProfileReason::ChildDecoration,
            }
        ))
    ));
}

#[test]
fn floating_zero_height_remains_admitted_and_equality_remains_unsupported() {
    let sources = sources(5);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    children[1].height = OwnerHeight::Explicit(0.);
    children[2].clear = ClearSide::None;
    assert!(build(&sources, outer, parent, &children, limits(5)).is_ok());

    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    children[1].height = OwnerHeight::Explicit(0.);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(5)),
        Err(ClearanceSequenceError::Placement {
            source: 4,
            error: ClearanceOwnerError::UnsupportedEquality,
        })
    ));
}

/// Independent integer vertical model for the core fixture: P content top 11,
/// explicit heights, nonnegative used margins, and strict clearance equality.
#[derive(Debug, PartialEq)]
struct ModelTail {
    source: usize,
    top: f32,
    bottom: f32,
    hypothetical_top: f32,
    zero_clearance_top: Option<f32>,
    clearance: Option<f32>,
    previous: (usize, f32),
    own_top: f32,
}

#[derive(Debug, PartialEq)]
struct ModelSequence {
    before: (f32, f32),
    float: (f32, f32),
    tail: Vec<ModelTail>,
}

fn model_sequence(
    used: &[(f32, f32)],
    heights: &[f32],
    clear_both: &[bool],
) -> Result<ModelSequence, usize> {
    let before_top = 11.0 + used[0].0;
    let before_bottom = before_top + heights[0];
    let float_top = before_bottom + used[0].1;
    let float_bottom = float_top + heights[1];
    let mut previous_bottom = before_bottom;
    let mut previous = (2, used[0].1);
    let mut tail = Vec::new();
    for index in 2..used.len() {
        let source = index + 2;
        let own_top = used[index].0;
        let hypothetical_top = previous_bottom + previous.1.max(own_top);
        let clear = clear_both[index];
        if clear && hypothetical_top == float_bottom {
            return Err(source);
        }
        let required = clear && hypothetical_top < float_bottom;
        let zero = required.then_some(previous_bottom + previous.1 + own_top);
        let top = if required {
            float_bottom
        } else {
            hypothetical_top
        };
        let bottom = top + heights[index];
        tail.push(ModelTail {
            source,
            top,
            bottom,
            hypothetical_top,
            zero_clearance_top: zero,
            clearance: zero.map(|zero| float_bottom - zero),
            previous,
            own_top,
        });
        previous_bottom = bottom;
        previous = (source, used[index].1);
    }
    Ok(ModelSequence {
        before: (before_top, before_bottom),
        float: (float_top, float_bottom),
        tail,
    })
}

/// Literal fixture used values, heights and clear sides, independent of getters.
fn fixture_profile(count: usize) -> (Vec<(f32, f32)>, Vec<f32>, Vec<bool>) {
    let used = [(0., 4.), (0., 0.), (3., 3.), (2., 4.), (1., 2.)];
    let heights = [1., 2., 1., 1., 2.];
    let clear = [false, false, true, true, false];
    (
        used[..count - 2].to_vec(),
        heights[..count - 2].to_vec(),
        clear[..count - 2].to_vec(),
    )
}

fn assert_owner_matches_model(
    owner: &iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'_>,
    model: &ModelSequence,
) {
    assert_eq!((owner.before().top, owner.before().bottom), model.before);
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        model.float
    );
    assert_eq!(owner.tail().len(), model.tail.len());
    for (record, expected) in owner.tail().iter().zip(&model.tail) {
        assert_eq!(record.source_index(), expected.source);
        assert_eq!(
            (record.vertical().top, record.vertical().bottom),
            (expected.top, expected.bottom)
        );
        assert_eq!(record.hypothetical_top(), expected.hypothetical_top);
        assert_eq!(record.zero_clearance_top(), expected.zero_clearance_top);
        assert_eq!(record.clearance(), expected.clearance);
        let members = [
            MarginMember {
                source_index: expected.previous.0,
                side: MarginSide::Bottom,
                value: expected.previous.1,
            },
            MarginMember {
                source_index: expected.source,
                side: MarginSide::Top,
                value: expected.own_top,
            },
        ];
        for (actual, literal) in record.hypothetical_group().members().iter().zip(&members) {
            assert_eq!(actual.value.to_bits(), literal.value.to_bits());
        }
        assert_eq!(
            record.hypothetical_group().id(),
            MarginGroupId::HypotheticalPair
        );
        assert_eq!(record.hypothetical_group().members(), members);
        let positive = expected.previous.1.max(expected.own_top);
        assert_eq!(record.hypothetical_group().positive(), positive);
        assert_eq!(record.hypothetical_group().negative(), 0.0);
        if expected.clearance.is_some() {
            assert_eq!(record.barrier_source_index(), Some(expected.source));
            let groups = record.actual_groups();
            assert_eq!(groups.len(), 2);
            assert_eq!(groups[0].id(), MarginGroupId::BeforeBarrier);
            assert_eq!(groups[0].members(), &members[..1]);
            assert_eq!(groups[0].positive(), expected.previous.1);
            assert_eq!(groups[1].id(), MarginGroupId::AfterBarrier);
            assert_eq!(groups[1].members(), &members[1..]);
            assert_eq!(groups[1].positive(), expected.own_top);
            assert!(groups.iter().all(|group| group.negative() == 0.0));
        } else {
            assert_eq!(record.barrier_source_index(), None);
            let groups = record.actual_groups();
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].id(), MarginGroupId::ActualPair);
            assert_eq!(groups[0].members(), members);
            assert_eq!(groups[0].positive(), positive);
            assert_eq!(groups[0].negative(), 0.0);
        }
    }
}

type OwnerObservation = (
    (f32, f32),
    iced_css_core::formatting::floats::PlacedFloat,
    iced_css_core::formatting::clearance_sequence::ClearanceSequenceReceipt,
    Vec<(
        usize,
        (f32, f32),
        f32,
        Option<f32>,
        Option<f32>,
        Option<usize>,
        iced_css_core::formatting::clearance_owner::MarginGroup,
        Vec<iced_css_core::formatting::clearance_owner::MarginGroup>,
    )>,
);

fn observe_owner(
    owner: &iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'_>,
) -> OwnerObservation {
    (
        (owner.before().top, owner.before().bottom),
        owner.placed_float(),
        owner.receipt(),
        owner
            .tail()
            .iter()
            .map(|record| {
                (
                    record.source_index(),
                    (record.vertical().top, record.vertical().bottom),
                    record.hypothetical_top(),
                    record.zero_clearance_top(),
                    record.clearance(),
                    record.barrier_source_index(),
                    *record.hypothetical_group(),
                    record.actual_groups().to_vec(),
                )
            })
            .collect(),
    )
}

#[test]
fn direct_typed_vertical_auto_matrix_matches_specified_zero_controls() {
    let zero = AutoLength::Value(0.0);
    for count in [5, 6, 7] {
        for target in 2..count {
            for side in [Float::Left, Float::Right] {
                let sources = sources(count);
                let (outer, parent, mut children) = facts(&sources);
                children[1].display.float = side;
                children[target - 2].margin.top = zero;
                children[target - 2].margin.bottom = zero;
                let control = build(&sources, outer, parent, &children, limits(count)).unwrap();
                let (mut used, heights, clear) = fixture_profile(count);
                used[target - 2] = (0.0, 0.0);
                let model = model_sequence(&used, &heights, &clear).unwrap();
                assert_owner_matches_model(&control, &model);
                let placed = control.placed_float();
                assert_eq!(
                    (placed.left, placed.right),
                    if side == Float::Left {
                        (0.0, 10.0)
                    } else {
                        (90.0, 100.0)
                    }
                );
                let control = observe_owner(&control);

                for (top, bottom) in [
                    (AutoLength::Auto, zero),
                    (zero, AutoLength::Auto),
                    (AutoLength::Auto, AutoLength::Auto),
                ] {
                    let (outer, parent, mut children) = facts(&sources);
                    children[1].display.float = side;
                    children[target - 2].margin.top = top;
                    children[target - 2].margin.bottom = bottom;
                    let owner = build(&sources, outer, parent, &children, limits(count)).unwrap();
                    assert_owner_matches_model(&owner, &model);
                    assert_eq!(observe_owner(&owner), control);
                    // The computed input category is retained unchanged.
                    assert_eq!(children[target - 2].margin.top, top);
                    assert_eq!(children[target - 2].margin.bottom, bottom);
                    assert_eq!(
                        children[target - 2].margin.used_top().to_bits(),
                        0.0f32.to_bits()
                    );
                    assert_eq!(
                        children[target - 2].margin.used_bottom().to_bits(),
                        0.0f32.to_bits()
                    );
                }
            }
        }
    }
}

#[test]
fn direct_seeds_cover_first_normal_tail_top_and_final_bottom_auto() {
    let sources6 = sources(6);
    // B1 bottom Auto with every other fixture margin unchanged.
    let (outer, parent, mut children) = facts(&sources6);
    children[0].margin.bottom = AutoLength::Auto;
    let owner = build(&sources6, outer, parent, &children, limits(6)).unwrap();
    assert_eq!((owner.before().top, owner.before().bottom), (11.0, 12.0));
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (12.0, 14.0)
    );
    assert_eq!(
        owner
            .tail()
            .iter()
            .map(|record| (record.vertical().top, record.vertical().bottom))
            .collect::<Vec<_>>(),
        [(15.0, 16.0), (19.0, 20.0)]
    );
    assert_eq!(owner.tail()[0].clearance(), None);
    assert_eq!(
        owner.tail()[0].hypothetical_group().members()[0],
        MarginMember {
            source_index: 2,
            side: MarginSide::Bottom,
            value: 0.0,
        }
    );

    // Final normal bottom Auto changes no placement; the retained final record
    // still ends at 23 and no later kernel consumes its used bottom.
    let (outer, parent, mut children) = facts(&sources6);
    children[3].margin.bottom = AutoLength::Auto;
    let owner = build(&sources6, outer, parent, &children, limits(6)).unwrap();
    let (mut used, heights, clear) = fixture_profile(6);
    used[3].1 = 0.0;
    assert_owner_matches_model(&owner, &model_sequence(&used, &heights, &clear).unwrap());
    assert_eq!(owner.tail()[1].vertical().bottom, 23.0);

    // Seven sources: the B4 top Auto member follows B3's actual used bottom 4.
    let sources7 = sources(7);
    let (outer, parent, mut children) = facts(&sources7);
    children[4].margin.top = AutoLength::Auto;
    let owner = build(&sources7, outer, parent, &children, limits(7)).unwrap();
    assert_eq!(owner.tail()[2].hypothetical_top(), 27.0);
    assert_eq!(
        owner.tail()[2].hypothetical_group().members(),
        [
            MarginMember {
                source_index: 5,
                side: MarginSide::Bottom,
                value: 4.0,
            },
            MarginMember {
                source_index: 6,
                side: MarginSide::Top,
                value: 0.0,
            },
        ]
    );
    assert_eq!(
        (
            owner.tail()[2].vertical().top,
            owner.tail()[2].vertical().bottom
        ),
        (27.0, 29.0)
    );
}

#[test]
fn all_vertical_auto_core_float_extension_and_equality() {
    for count in [5, 6, 7] {
        for side in [Float::Left, Float::Right] {
            for (float_height, clear_both) in
                [(40.0, false), (2.0, true), (40.0, true), (2.0, false)]
            {
                let sources = sources(count);
                let (outer, parent, mut children) = facts(&sources);
                children[1].display.float = side;
                children[1].height = OwnerHeight::Explicit(float_height);
                for (index, child) in children.iter_mut().enumerate() {
                    child.margin.top = AutoLength::Auto;
                    child.margin.bottom = AutoLength::Auto;
                    if index >= 2 {
                        child.clear = if clear_both {
                            ClearSide::Both
                        } else {
                            ClearSide::None
                        };
                    }
                }
                let owner = build(&sources, outer, parent, &children, limits(count)).unwrap();
                let (_, mut heights, _) = fixture_profile(count);
                heights[1] = float_height;
                let used = vec![(0.0, 0.0); count - 2];
                let clear: Vec<_> = (0..count - 2)
                    .map(|index| index >= 2 && clear_both)
                    .collect();
                let model = model_sequence(&used, &heights, &clear).unwrap();
                assert_owner_matches_model(&owner, &model);
                assert_eq!(model.before, (11.0, 12.0));
                assert_eq!(model.float, (12.0, 12.0 + float_height));
                if float_height == 40.0 && !clear_both {
                    assert_eq!(
                        model
                            .tail
                            .iter()
                            .map(|tail| (tail.top, tail.bottom))
                            .collect::<Vec<_>>(),
                        [(12.0, 13.0), (13.0, 14.0), (14.0, 16.0)][..count - 4]
                    );
                }
            }

            // F height 0 with clear both makes B2's hypothetical edge equal F bottom.
            let sources = sources(count);
            let (outer, parent, mut children) = facts(&sources);
            children[1].display.float = side;
            children[1].height = OwnerHeight::Explicit(0.0);
            for child in &mut children {
                child.margin.top = AutoLength::Auto;
                child.margin.bottom = AutoLength::Auto;
            }
            assert!(matches!(
                build(&sources, outer, parent, &children, limits(count)),
                Err(ClearanceSequenceError::Placement {
                    source: 4,
                    error: ClearanceOwnerError::UnsupportedEquality,
                })
            ));
        }
    }
}

#[test]
fn negative_zero_absolute_margins_keep_bits_and_are_not_relabelled() {
    let negative = iced_css_core::formatting::clearance_owner::ChildMargins {
        top: AutoLength::Value(-0.0),
        right: AutoLength::Value(-0.0),
        bottom: AutoLength::Value(-0.0),
        left: AutoLength::Value(-0.0),
    };
    assert_eq!(negative.used_top().to_bits(), (-0.0f32).to_bits());
    assert_eq!(negative.used_bottom().to_bits(), (-0.0f32).to_bits());
    assert_eq!(negative.top, AutoLength::Value(-0.0));
    assert!(
        matches!(negative.top, AutoLength::Value(value) if value.to_bits() == (-0.0f32).to_bits())
    );
    let auto = iced_css_core::formatting::clearance_owner::ChildMargins {
        top: AutoLength::Auto,
        bottom: AutoLength::Auto,
        ..negative
    };
    assert_eq!(auto.used_top().to_bits(), 0.0f32.to_bits());
    assert_eq!(auto.used_bottom().to_bits(), 0.0f32.to_bits());
    assert_eq!(auto.top, AutoLength::Auto);
    let converted: iced_css_core::formatting::clearance_owner::ChildMargins = OwnerEdges {
        top: -0.0,
        right: 1.0,
        bottom: 2.0,
        left: -3.0,
    }
    .into();
    assert!(
        matches!(converted.top, AutoLength::Value(value) if value.to_bits() == (-0.0f32).to_bits())
    );
    assert_eq!(converted.right, AutoLength::Value(1.0));
    assert_eq!(converted.bottom, AutoLength::Value(2.0));
    assert_eq!(converted.left, AutoLength::Value(-3.0));

    // Negative-zero absolute vertical margins are not negative and are admitted;
    // the member keeps the supplied bits while arithmetic publishes positive zero.
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.top = AutoLength::Value(-0.0);
    children[0].margin.bottom = AutoLength::Value(-0.0);
    children[2].margin.top = AutoLength::Value(-0.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!((owner.before().top, owner.before().bottom), (11.0, 12.0));
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (12.0, 14.0)
    );
    let members = owner.tail()[0].hypothetical_group().members();
    assert_eq!(members[0].value.to_bits(), (-0.0f32).to_bits());
    assert_eq!(members[1].value.to_bits(), (-0.0f32).to_bits());
    assert_eq!(
        owner.tail()[0].zero_clearance_top().unwrap().to_bits(),
        12.0f32.to_bits()
    );
    assert_eq!(owner.tail()[0].clearance(), Some(2.0));
    assert!(
        matches!(children[0].margin.bottom, AutoLength::Value(value) if value.to_bits() == (-0.0f32).to_bits())
    );
}

#[test]
fn float_used_horizontal_getters_keep_negative_zero_bits_and_resolve_auto() {
    use iced_css_core::formatting::clearance_owner::ChildMargins;
    let mut margins: ChildMargins = edges().into();
    margins.left = AutoLength::Value(-0.0);
    margins.right = AutoLength::Value(-0.0);
    assert_eq!(margins.float_used_left().to_bits(), 0x8000_0000);
    assert_eq!(margins.float_used_right().to_bits(), 0x8000_0000);
    margins.left = AutoLength::Auto;
    margins.right = AutoLength::Auto;
    assert_eq!(margins.float_used_left().to_bits(), 0x0000_0000);
    assert_eq!(margins.float_used_right().to_bits(), 0x0000_0000);
    margins.left = AutoLength::Value(5.0);
    margins.right = AutoLength::Value(7.0);
    assert_eq!(margins.float_used_left().to_bits(), 0x40a0_0000);
    assert_eq!(margins.float_used_right().to_bits(), 0x40e0_0000);
}

#[test]
fn vertical_auto_keeps_every_sequence_exclusion() {
    use OwnerProfileReason as R;
    let source_nodes = sources(6);
    let rejected = |edit: &dyn Fn(&mut Vec<ChildFacts<'_>>), expected_source, expected| {
        let (outer, parent, mut children) = facts(&source_nodes);
        for child in &mut children {
            child.margin.top = AutoLength::Auto;
        }
        edit(&mut children);
        let error = build(&source_nodes, outer, parent, &children, limits(6)).unwrap_err();
        assert!(
            matches!(
                error,
                ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported { source, reason })
                    if source == expected_source && reason == expected
            ),
            "{error:?}"
        );
    };
    // Signed normal bottoms beside all-top Auto retain their exact geometry.
    for (slot, oracle) in [(3, "H"), (0, "I")] {
        let (outer, parent, mut children) = facts(&source_nodes);
        for child in &mut children {
            child.margin.top = AutoLength::Auto;
        }
        children[slot].margin.bottom = AutoLength::Value(-1.0);
        snv_assert_core_oracle(
            &build(&source_nodes, outer, parent, &children, limits(6)).unwrap(),
            oracle,
        );
    }
    // A float margin of either sign on any edge is admitted beside the vertical Auto
    // margins while both outer extents stay nonnegative (SFM: C12t, C12r, C12b and C12l).
    for edge in 0..4 {
        for value in [-1.0, 1.0] {
            if value > 0.0 && (edge == 0 || edge == 2) {
                let (outer, parent, mut children) = facts(&source_nodes);
                for child in &mut children {
                    child.margin.top = AutoLength::Auto;
                }
                if edge == 0 {
                    children[1].margin.top = AutoLength::Value(value);
                } else {
                    children[1].margin.bottom = AutoLength::Value(value);
                }
                let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
                assert_eq!(
                    (owner.placed_float().top, owner.placed_float().bottom),
                    (16.0, 19.0)
                );
                let b2 = &owner.tail()[0];
                assert_eq!((b2.vertical().top, b2.clearance()), (19.0, Some(3.0)));
                continue;
            }
            if value > 0.0 && (edge == 1 || edge == 3) {
                let (outer, parent, mut children) = facts(&source_nodes);
                for child in &mut children {
                    child.margin.top = AutoLength::Auto;
                }
                if edge == 1 {
                    children[1].margin.right = AutoLength::Value(value);
                } else {
                    children[1].margin.left = AutoLength::Value(value);
                }
                let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
                let content = if edge == 3 { (1.0, 11.0) } else { (0.0, 10.0) };
                assert_eq!(
                    (owner.placed_float().left, owner.placed_float().right),
                    (0.0, 11.0)
                );
                assert_eq!(
                    (owner.float_content().left, owner.float_content().right),
                    content
                );
                continue;
            }
            let (outer, parent, mut children) = facts(&source_nodes);
            for child in &mut children {
                child.margin.top = AutoLength::Auto;
            }
            let value = AutoLength::Value(value);
            match edge {
                0 => children[1].margin.top = value,
                1 => children[1].margin.right = value,
                2 => children[1].margin.bottom = value,
                _ => children[1].margin.left = value,
            }
            let expected = [
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 10.0, 16.0, 17.0],
                    [0.0, 10.0, 15.0, 17.0],
                    &[(17.0, 18.0, Some(1.0)), (21.0, 22.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 9.0, 16.0, 18.0],
                    [0.0, 10.0, 16.0, 18.0],
                    &[(18.0, 19.0, Some(2.0)), (22.0, 23.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 10.0, 16.0, 17.0],
                    [0.0, 10.0, 16.0, 18.0],
                    &[(17.0, 18.0, Some(1.0)), (21.0, 22.0, None)],
                ),
                sfm_core_expected(
                    [11.0, 12.0],
                    [0.0, 9.0, 16.0, 18.0],
                    [-1.0, 9.0, 16.0, 18.0],
                    &[(18.0, 19.0, Some(2.0)), (22.0, 23.0, None)],
                ),
            ];
            let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
            assert_eq!(sfm_core_view(&owner), expected[edge]);
        }
    }
    rejected(&|c| c[3].height = OwnerHeight::Auto, 5, R::HeightKind);
    rejected(
        &|c| c[2].context = ChildContext::OtherNewBfc,
        4,
        R::ChildContext,
    );
    rejected(&|c| c[3].border.bottom = 1.0, 5, R::ChildDecoration);
    rejected(&|c| c[1].border.top = 1.0, 3, R::ChildDecoration);
    // Leading clear on B1 or the float is admitted beside every vertical Auto and is
    // inert: each owner equals the same all-tops-Auto input without `clear`.
    for (slot, side) in [(0, ClearSide::Left), (1, ClearSide::Both)] {
        let (outer, parent, mut control) = facts(&source_nodes);
        for child in &mut control {
            child.margin.top = AutoLength::Auto;
        }
        let mut children = control.clone();
        children[slot].clear = side;
        let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
        let control = build(&source_nodes, outer, parent, &control, limits(6)).unwrap();
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "slot {slot}");
    }
    rejected(
        &|c| c[2].height = OwnerHeight::Explicit(0.0),
        4,
        R::NonpositiveBlockHeight,
    );

    // Signed and Auto normal horizontal plus float horizontal Auto remain admitted
    // alongside every vertical Auto.
    let (outer, parent, mut children) = facts(&source_nodes);
    for child in &mut children {
        child.margin.top = AutoLength::Auto;
        child.margin.bottom = AutoLength::Auto;
    }
    children[0].margin.left = AutoLength::Value(-10.0);
    children[0].margin.right = AutoLength::Auto;
    children[1].margin.left = AutoLength::Auto;
    children[1].margin.right = AutoLength::Auto;
    children[3].margin.left = AutoLength::Value(80.0);
    children[3].margin.right = AutoLength::Value(-30.0);
    let owner = build(&source_nodes, outer, parent, &children, limits(6)).unwrap();
    let (_, heights, clear) = fixture_profile(6);
    assert_owner_matches_model(
        &owner,
        &model_sequence(&[(0.0, 0.0); 4], &heights, &clear).unwrap(),
    );
}

#[test]
fn vertical_auto_priority_rows_have_later_fault_controls() {
    let source_nodes = sources(6);
    let auto_profiles = |children: &mut Vec<ChildFacts<'_>>| {
        children[0].margin.bottom = AutoLength::Auto;
        children[1].margin.top = AutoLength::Auto;
        children[1].margin.bottom = AutoLength::Auto;
        children[2].margin.top = AutoLength::Auto;
        children[3].margin.bottom = AutoLength::Auto;
    };

    // Source/count preflight precedes every Auto-bearing profile.
    let (outer, parent, mut children) = facts(&source_nodes);
    auto_profiles(&mut children);
    let mut short = limits(6);
    short.sources = NonZeroUsize::new(5).unwrap();
    assert!(matches!(
        build(&source_nodes, outer, parent, &children, short),
        Err(ClearanceSequenceError::Sources)
    ));
    let mut missing = children.clone();
    missing.pop();
    assert!(matches!(
        build(&source_nodes, outer, parent, &missing, limits(6)),
        Err(ClearanceSequenceError::Facts)
    ));
    assert!(build(&source_nodes, outer, parent, &children, limits(6)).is_ok());

    // Both signed and nonnegative normals expose the later float-width domain.
    for negative in [true, false] {
        let (outer, parent, mut children) = facts(&source_nodes);
        children[2].margin.top = AutoLength::Auto;
        if negative {
            children[3].margin.bottom = AutoLength::Value(-1.0);
        }
        let error =
            build_width(&source_nodes, outer, parent, &children, f32::NAN, limits(6)).unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 3,
                field: "float_width",
            })
        ));
    }

    // A genuinely invalid later profile still precedes float-width validation.
    let (outer, parent, mut children) = facts(&source_nodes);
    children[3].margin.bottom = AutoLength::Value(f32::NAN);
    assert!(matches!(
        build_width(&source_nodes, outer, parent, &children, f32::NAN, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 5,
                field: "margin"
            }
        ))
    ));

    // Core top Auto plus bottom NaN: same-source InvalidNumber before sign/Auto
    // classification; the negative and valid controls classify normally.
    for source in [2, 3, 4, 5] {
        for (bottom, expected) in [
            (AutoLength::Value(f32::NAN), 0),
            (AutoLength::Value(f32::INFINITY), 0),
            (AutoLength::Value(-1.0), 1),
            (AutoLength::Value(0.0), 2),
        ] {
            let (outer, parent, mut children) = facts(&source_nodes);
            children[source - 2].margin.top = AutoLength::Auto;
            children[source - 2].margin.bottom = bottom;
            let result = build(&source_nodes, outer, parent, &children, limits(6));
            match expected {
                0 => assert!(matches!(
                    result,
                    Err(ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                        source: actual,
                        field: "margin",
                    })) if actual == source
                )),
                // SFM: F's top Auto beside its bottom -1 is admitted (C13).
                1 if source == 3 => {
                    assert_eq!(
                        sfm_core_view(&result.unwrap()),
                        sfm_core_expected(
                            [11.0, 12.0],
                            [0.0, 10.0, 16.0, 17.0],
                            [0.0, 10.0, 16.0, 18.0],
                            &[(17.0, 18.0, Some(-2.0)), (21.0, 22.0, None)]
                        )
                    )
                }
                1 => {
                    let oracle = match source {
                        2 => "G",
                        4 => "J",
                        5 => "D",
                        _ => unreachable!(),
                    };
                    snv_assert_core_oracle(&result.unwrap(), oracle);
                }
                _ => assert!(result.is_ok()),
            }
        }
    }

    // Valid Auto profiles, invalid width and insufficient retained bytes: the
    // float-width domain precedes Bytes; a valid width reaches Bytes.
    let tiny = ClearanceSequenceLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits(6)
    };
    for (width, invalid) in [
        (f32::NAN, true),
        (-1.0, true),
        (10.0, false),
        (140.0, false),
    ] {
        let (outer, parent, mut children) = facts(&source_nodes);
        auto_profiles(&mut children);
        let error = build_width(&source_nodes, outer, parent, &children, width, tiny).unwrap_err();
        if invalid {
            assert!(matches!(
                error,
                ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                    source: 3,
                    field: "float_width",
                })
            ));
        } else {
            assert!(matches!(error, ClearanceSequenceError::Bytes));
        }
    }

    // Valid Auto profiles and width 140 with insufficient bytes reject before any
    // kernel even when the first kernel would overflow; enough bytes reach it.
    for enough in [false, true] {
        let (outer, mut parent, mut children) = facts(&source_nodes);
        auto_profiles(&mut children);
        parent.content_top = f32::MAX;
        children[0].margin.top = AutoLength::Value(f32::MAX);
        let limit = if enough { limits(6) } else { tiny };
        let error = build_width(&source_nodes, outer, parent, &children, 140.0, limit).unwrap_err();
        if enough {
            assert!(matches!(
                error,
                ClearanceSequenceError::Placement {
                    source: 2,
                    error: ClearanceOwnerError::Unrepresentable {
                        stage: "before_top"
                    },
                }
            ));
        } else {
            assert!(matches!(error, ClearanceSequenceError::Bytes));
        }
    }

    // Finite float publication overflow at source 3 precedes the prospective
    // source 4 equality; the right-side control reaches that equality.
    for count in [5, 6, 7] {
        let sources = sources(count);
        for (side, float_failure) in [(Float::Left, true), (Float::Right, false)] {
            let (outer, mut parent, mut children) = facts(&sources);
            parent.containing = Band {
                left: f32::MAX,
                right: f32::MAX,
            };
            for child in &mut children {
                child.margin.top = AutoLength::Auto;
                child.margin.bottom = AutoLength::Auto;
            }
            children[1].display.float = side;
            children[1].height = OwnerHeight::Explicit(0.0);
            let error = build_width(&sources, outer, parent, &children, f32::MAX, limits(count))
                .unwrap_err();
            if float_failure {
                assert!(matches!(
                    error,
                    ClearanceSequenceError::Placement {
                        source: 3,
                        error: ClearanceOwnerError::FloatPlacement(
                            iced_css_core::formatting::floats::FloatError::Unrepresentable
                        ),
                    }
                ));
            } else {
                assert!(matches!(
                    error,
                    ClearanceSequenceError::Placement {
                        source: 4,
                        error: ClearanceOwnerError::UnsupportedEquality,
                    }
                ));
            }
        }
    }
}

#[test]
fn vertical_auto_work_and_storage_boundaries_are_exact() {
    for (count, work, old_work) in [(5, 945, 419), (6, 1107, 501), (7, 1270, 584)] {
        let all_auto = |children: &mut Vec<ChildFacts<'_>>| {
            for child in children.iter_mut() {
                child.margin.top = AutoLength::Auto;
                child.margin.bottom = AutoLength::Auto;
            }
            children[1].margin.left = AutoLength::Auto;
            children[1].margin.right = AutoLength::Auto;
        };

        // One-short and old bounds fail before the graph scan would report the
        // malformed child count; the exact bound reaches that scan.
        let mut malformed = sources(count);
        let unrelated = malformed[0].instance;
        malformed[count - 1].children.push(unrelated);
        let (outer, parent, mut children) = facts(&malformed);
        all_auto(&mut children);
        for short in [work - 1, old_work] {
            let mut limit = limits(count);
            limit.work = NonZeroUsize::new(short).unwrap();
            assert!(matches!(
                build(&malformed, outer, parent, &children, limit),
                Err(ClearanceSequenceError::Work)
            ));
        }
        let mut exact = limits(count);
        exact.work = NonZeroUsize::new(work).unwrap();
        assert!(matches!(
            build(&malformed, outer, parent, &children, exact),
            Err(ClearanceSequenceError::Profile(ClearanceOwnerError::ChildCount { source }))
                if source == count - 1
        ));

        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        all_auto(&mut children);
        let tail_count = count - 4;
        let floor = tail_count * size_of::<SequenceNormalPlacement>();
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work).unwrap();
        limit.retained_bytes = NonZeroUsize::new(floor - 1).unwrap();
        assert!(matches!(
            build_width(&sources, outer, parent, &children, 140.0, limit),
            Err(ClearanceSequenceError::Bytes)
        ));
        limit.retained_bytes = NonZeroUsize::new(floor).unwrap();
        let owner = build_width(&sources, outer, parent, &children, 140.0, limit).unwrap();
        assert_eq!(owner.receipt().nodes, count);
        assert_eq!(owner.receipt().tail, tail_count);
        assert_eq!(owner.receipt().work, work);
        assert_eq!(owner.receipt().record_capacity, tail_count);
        assert_eq!(owner.receipt().retained_bytes, floor);
        assert_eq!(
            (owner.placed_float().left, owner.placed_float().right),
            (0.0, 140.0)
        );
    }
}

#[test]
fn nonzero_float_horizontal_margins_place_the_margin_box() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(5.0);
    children[1].margin.right = AutoLength::Value(7.0);
    let owner = build_width(&sources, outer, parent, &children, 30.0, limits(6)).unwrap();
    let placed = owner.placed_float();
    assert_eq!(
        (placed.left, placed.right, placed.top, placed.bottom),
        (0.0, 42.0, 16.0, 18.0)
    );
    assert_eq!(
        owner.float_content(),
        Band {
            left: 5.0,
            right: 35.0
        }
    );
    assert_eq!(children[1].margin.float_used_left().to_bits(), 0x40a0_0000);
    assert_eq!(children[1].margin.float_used_right().to_bits(), 0x40e0_0000);
    let (outer, parent, control) = facts(&sources);
    let control = build_width(&sources, outer, parent, &control, 30.0, limits(6)).unwrap();
    assert_eq!(owner.before(), control.before());
    assert_eq!(owner.tail().len(), control.tail().len());
    for (actual, expected) in owner.tail().iter().zip(control.tail()) {
        assert_eq!(actual.vertical(), expected.vertical());
        assert_eq!(
            actual.hypothetical_top().to_bits(),
            expected.hypothetical_top().to_bits()
        );
        assert_eq!(actual.zero_clearance_top(), expected.zero_clearance_top());
        assert_eq!(actual.clearance(), expected.clearance());
        assert_eq!(actual.hypothetical_group(), expected.hypothetical_group());
        assert_eq!(actual.actual_groups(), expected.actual_groups());
    }
}

const F32_2P102: u32 = 0x7280_0000;
const F32_2P103: u32 = 0x7300_0000;
const F32_2P103_PLUS_2P80: u32 = 0x7300_0001;
const F32_MAX_BITS: u32 = 0x7f7f_ffff;
const F32_MAX_MINUS_ULP: u32 = 0x7f7f_fffe;

/// Direct-core first-float observation on the N6 fixture with a chosen containing
/// band, float side, content width and horizontal float margins.
fn nfh_first_float(
    containing: Band,
    side: Float,
    width: f32,
    left: AutoLength,
    right: AutoLength,
) -> Result<(iced_css_core::formatting::floats::PlacedFloat, Band), ClearanceSequenceError> {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = containing;
    children[1].display.float = side;
    children[1].margin.left = left;
    children[1].margin.right = right;
    build_width(&sources, outer, parent, &children, width, limits(6))
        .map(|owner| (owner.placed_float(), owner.float_content()))
}

fn bits(band: (f32, f32)) -> (u32, u32) {
    (band.0.to_bits(), band.1.to_bits())
}

fn placement_stage(
    result: Result<(iced_css_core::formatting::floats::PlacedFloat, Band), ClearanceSequenceError>,
) -> String {
    match result {
        Err(ClearanceSequenceError::Placement { source: 3, error }) => format!("{error:?}"),
        other => panic!("expected a source3 placement failure, got {other:?}"),
    }
}

#[test]
fn zero_margin_identity_path_keeps_negative_zero_band_bits() {
    for (left, right) in [
        (AutoLength::Value(0.0), AutoLength::Value(0.0)),
        (AutoLength::Value(-0.0), AutoLength::Value(-0.0)),
        (AutoLength::Auto, AutoLength::Auto),
        (AutoLength::Value(-0.0), AutoLength::Auto),
    ] {
        let (placed, content) = nfh_first_float(
            Band {
                left: -0.0,
                right: 100.0,
            },
            Float::Left,
            10.0,
            left,
            right,
        )
        .unwrap();
        assert_eq!(placed.left.to_bits(), 0x8000_0000);
        assert_eq!(content.left.to_bits(), 0x8000_0000);
        assert_eq!(content.right.to_bits(), placed.right.to_bits());

        let (placed, content) = nfh_first_float(
            Band {
                left: -100.0,
                right: -0.0,
            },
            Float::Right,
            10.0,
            left,
            right,
        )
        .unwrap();
        assert_eq!(placed.right.to_bits(), 0x8000_0000);
        assert_eq!(content.right.to_bits(), 0x8000_0000);
        assert_eq!(content.left.to_bits(), placed.left.to_bits());
    }
}

#[test]
fn nonzero_float_margin_stages_publish_or_fail_with_named_stages() {
    let v = |value: u32| AutoLength::Value(f32::from_bits(value));
    let band = |left: u32, right: u32| Band {
        left: f32::from_bits(left),
        right: f32::from_bits(right),
    };
    let zero_hundred = Band {
        left: 0.0,
        right: 100.0,
    };
    // float_outer_width: MAX + 2^103 rounds to infinity on both sides.
    for side in [Float::Left, Float::Right] {
        let result = nfh_first_float(zero_hundred, side, f32::MAX, v(F32_2P103), AutoLength::Auto);
        assert_eq!(
            placement_stage(result),
            "Unrepresentable { stage: \"float_outer_width\" }"
        );
    }
    // 2^102 is absorbed into MAX: left content 2^102..MAX; right content -MAX..100.
    let (placed, content) = nfh_first_float(
        zero_hundred,
        Float::Left,
        f32::MAX,
        v(F32_2P102),
        AutoLength::Auto,
    )
    .unwrap();
    assert_eq!(
        bits((placed.left, placed.right)),
        (0x0000_0000, F32_MAX_BITS)
    );
    assert_eq!(
        bits((content.left, content.right)),
        (F32_2P102, F32_MAX_BITS)
    );
    let (placed, content) = nfh_first_float(
        zero_hundred,
        Float::Right,
        f32::MAX,
        v(F32_2P102),
        AutoLength::Auto,
    )
    .unwrap();
    assert_eq!(
        bits((placed.left, placed.right)),
        (0xff7f_ffff, 0x42c8_0000)
    );
    assert_eq!(
        bits((content.left, content.right)),
        (0xff7f_ffff, 0x42c8_0000)
    );
    // Margin box: a left F on band MAX..MAX with outer 2^103 fails placement; the
    // right F succeeds with its own-side edge anchored.
    let max_band = band(F32_MAX_BITS, F32_MAX_BITS);
    let result = nfh_first_float(
        max_band,
        Float::Left,
        f32::from_bits(F32_2P103),
        AutoLength::Value(1.0),
        AutoLength::Auto,
    );
    assert_eq!(placement_stage(result), "FloatPlacement(Unrepresentable)");
    let (placed, content) = nfh_first_float(
        max_band,
        Float::Right,
        f32::from_bits(F32_2P103),
        AutoLength::Value(1.0),
        AutoLength::Auto,
    )
    .unwrap();
    assert_eq!(
        bits((placed.left, placed.right)),
        (F32_MAX_MINUS_ULP, F32_MAX_BITS)
    );
    assert_eq!(
        bits((content.left, content.right)),
        (F32_MAX_MINUS_ULP, F32_MAX_BITS)
    );
    // Far content edge: the margin box is finite but the far edge published from
    // the rounded near edge overflows.
    let result = nfh_first_float(
        band(F32_MAX_MINUS_ULP, F32_MAX_MINUS_ULP),
        Float::Left,
        f32::from_bits(F32_2P103),
        v(F32_2P103_PLUS_2P80),
        AutoLength::Auto,
    );
    assert_eq!(
        placement_stage(result),
        "Unrepresentable { stage: \"float_content_right\" }"
    );
    let negative = 0x8000_0000 | F32_MAX_MINUS_ULP;
    let result = nfh_first_float(
        band(negative, negative),
        Float::Right,
        f32::from_bits(F32_2P103),
        AutoLength::Auto,
        v(F32_2P103_PLUS_2P80),
    );
    assert_eq!(
        placement_stage(result),
        "Unrepresentable { stage: \"float_content_left\" }"
    );
}

fn profile<'s>(
    result: Result<
        iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'s>,
        ClearanceSequenceError,
    >,
) -> String {
    match result {
        Err(ClearanceSequenceError::Profile(error)) => format!("{error:?}"),
        other => panic!("expected a profile failure, got {other:?}"),
    }
}

#[test]
fn nonzero_float_margin_admission_keeps_fault_precedence() {
    let sources = sources(6);
    // A negative horizontal edge is admitted on either edge while the outer width stays
    // nonnegative (SFM: C1 and C10).
    for right in [false, true] {
        let (outer, parent, mut children) = facts(&sources);
        if right {
            children[1].margin.right = AutoLength::Value(-1.0);
        } else {
            children[1].margin.left = AutoLength::Value(-1.0);
        }
        let expected = if right {
            sfm_core_expected(
                [11.0, 12.0],
                [0.0, 9.0, 16.0, 18.0],
                [0.0, 10.0, 16.0, 18.0],
                &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            )
        } else {
            sfm_core_expected(
                [11.0, 12.0],
                [0.0, 9.0, 16.0, 18.0],
                [-1.0, 9.0, 16.0, 18.0],
                &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
            )
        };
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        assert_eq!(sfm_core_view(&owner), expected);
    }
    // A negative horizontal edge beside a nonzero vertical edge is admitted too (SFM, C14).
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-1.0);
    children[1].margin.top = AutoLength::Value(1.0);
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 9.0, 16.0, 19.0],
            [-1.0, 9.0, 17.0, 19.0],
            &[(19.0, 20.0, Some(0.0)), (23.0, 24.0, None)]
        )
    );
    // A nonzero vertical edge is admitted, alone or beside nonzero horizontal edges:
    // (left, top, bottom, right) -> margin box, content.
    for (left, top, bottom, right, margin_box, content) in [
        (
            0.0,
            1.0,
            0.0,
            0.0,
            (0.0, 10.0, 16.0, 19.0),
            (0.0, 10.0, 17.0, 19.0),
        ),
        (
            0.0,
            0.0,
            1.0,
            0.0,
            (0.0, 10.0, 16.0, 19.0),
            (0.0, 10.0, 16.0, 18.0),
        ),
        (
            5.0,
            1.0,
            0.0,
            7.0,
            (0.0, 22.0, 16.0, 19.0),
            (5.0, 15.0, 17.0, 19.0),
        ),
    ] {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.left = AutoLength::Value(left);
        children[1].margin.top = AutoLength::Value(top);
        children[1].margin.bottom = AutoLength::Value(bottom);
        children[1].margin.right = AutoLength::Value(right);
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        let placed = owner.placed_float();
        assert_eq!(
            (placed.left, placed.right, placed.top, placed.bottom),
            margin_box
        );
        let band = owner.float_content();
        let span = owner.float_content_vertical();
        assert_eq!((band.left, band.right, span.top, span.bottom), content);
    }
    // A nonfinite horizontal edge is InvalidNumber before any sign check.
    for right in [false, true] {
        let (outer, parent, mut children) = facts(&sources);
        if right {
            children[1].margin.right = AutoLength::Value(f32::NAN);
            children[1].margin.left = AutoLength::Value(-1.0);
        } else {
            children[1].margin.left = AutoLength::Value(f32::NAN);
            children[1].margin.right = AutoLength::Value(-1.0);
        }
        assert_eq!(
            profile(build(&sources, outer, parent, &children, limits(6))),
            "InvalidNumber { source: 3, field: \"margin\" }"
        );
    }
    // A later child's profile fault precedes the float-width domain check, which
    // precedes nothing numeric.
    for later_fault in [true, false] {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.left = AutoLength::Value(5.0);
        children[1].margin.right = AutoLength::Value(7.0);
        if later_fault {
            children[3].border.left = 1.0;
        }
        let result = build_width(&sources, outer, parent, &children, -1.0, limits(6));
        let expected = if later_fault {
            "Unsupported { source: 5, reason: ChildDecoration }"
        } else {
            "InvalidNumber { source: 3, field: \"float_width\" }"
        };
        assert_eq!(profile(result), expected);
    }
    // Valid nonzero margins whose kernel would fail: the byte floor fails first.
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(f32::from_bits(F32_2P103));
    let floor = 2 * size_of::<SequenceNormalPlacement>();
    let mut limit = limits(6);
    limit.retained_bytes = NonZeroUsize::new(floor - 1).unwrap();
    assert!(matches!(
        build_width(&sources, outer, parent, &children, f32::MAX, limit),
        Err(ClearanceSequenceError::Bytes)
    ));
    assert!(matches!(
        build_width(&sources, outer, parent, &children, f32::MAX, limits(6)),
        Err(ClearanceSequenceError::Placement {
            source: 3,
            error: ClearanceOwnerError::Unrepresentable {
                stage: "float_outer_width"
            },
        })
    ));
}

#[test]
fn float_outer_width_failure_precedes_later_unsupported_equality() {
    let sources = sources(5);
    let equality = |left: f32, width: f32| {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.left = AutoLength::Value(left);
        children[1].margin.right = AutoLength::Auto;
        children[1].height = OwnerHeight::Explicit(0.);
        build_width(&sources, outer, parent, &children, width, limits(5)).map(|_| ())
    };
    // Controls: zero and admitted nonzero horizontal margins both reach source4
    // UnsupportedEquality, so horizontal margins do not change the vertical result.
    for left in [0.0, 5.0] {
        assert!(matches!(
            equality(left, 10.0),
            Err(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ));
    }
    assert!(matches!(
        equality(f32::from_bits(F32_2P103), f32::MAX),
        Err(ClearanceSequenceError::Placement {
            source: 3,
            error: ClearanceOwnerError::Unrepresentable {
                stage: "float_outer_width"
            },
        })
    ));
}

#[test]
fn nonzero_float_margin_work_bounds_are_exact() {
    for (count, work, old_work) in [(5, 945, 419), (6, 1107, 501), (7, 1270, 584)] {
        let margins = |children: &mut Vec<ChildFacts<'_>>| {
            children[1].margin.left = AutoLength::Value(5.0);
            children[1].margin.right = AutoLength::Value(7.0);
        };
        // One-short and predecessor bounds fail before the graph scan would report
        // the malformed child count; the exact bound reaches that scan.
        let mut malformed = sources(count);
        let unrelated = malformed[0].instance;
        malformed[count - 1].children.push(unrelated);
        let (outer, parent, mut children) = facts(&malformed);
        margins(&mut children);
        for short in [work - 1, old_work] {
            let mut limit = limits(count);
            limit.work = NonZeroUsize::new(short).unwrap();
            assert!(matches!(
                build(&malformed, outer, parent, &children, limit),
                Err(ClearanceSequenceError::Work)
            ));
        }
        let mut exact = limits(count);
        exact.work = NonZeroUsize::new(work).unwrap();
        assert!(matches!(
            build(&malformed, outer, parent, &children, exact),
            Err(ClearanceSequenceError::Profile(ClearanceOwnerError::ChildCount { source }))
                if source == count - 1
        ));

        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        margins(&mut children);
        let tail_count = count - 4;
        let floor = tail_count * size_of::<SequenceNormalPlacement>();
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work).unwrap();
        limit.retained_bytes = NonZeroUsize::new(floor).unwrap();
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        assert_eq!(owner.receipt().work, work);
        assert_eq!(owner.receipt().record_capacity, tail_count);
        assert_eq!(owner.receipt().retained_bytes, floor);
        assert_eq!(
            (owner.placed_float().left, owner.placed_float().right),
            (0.0, 22.0)
        );
        assert_eq!(
            (owner.float_content().left, owner.float_content().right),
            (5.0, 15.0)
        );
    }
}

#[test]
fn nonzero_float_vertical_margins_place_the_margin_box() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(3.0);
    children[1].margin.bottom = AutoLength::Value(4.0);
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    let placed = owner.placed_float();
    assert_eq!(
        (placed.left, placed.right, placed.top, placed.bottom),
        (0.0, 10.0, 16.0, 25.0)
    );
    let tail = owner.tail();
    assert_eq!(
        (
            tail[0].vertical().top,
            tail[0].vertical().bottom,
            tail[0].clearance()
        ),
        (25.0, 26.0, Some(6.0))
    );
    assert_eq!(
        (
            tail[1].vertical().top,
            tail[1].vertical().bottom,
            tail[1].clearance()
        ),
        (29.0, 30.0, None)
    );
    // Margin-box-equivalent control: zero margins, height RN(3 + 2 + 4) = 9.
    let (outer, parent, mut equivalent) = facts(&sources);
    equivalent[1].height = OwnerHeight::Explicit(9.0);
    let equivalent = build(&sources, outer, parent, &equivalent, limits(6)).unwrap();
    assert_eq!(owner.before(), equivalent.before());
    assert_eq!(owner.placed_float(), equivalent.placed_float());
    assert_eq!(owner.float_content(), equivalent.float_content());
    assert_eq!(
        format!("{:?}", owner.tail()),
        format!("{:?}", equivalent.tail())
    );
    assert_eq!(owner.receipt(), equivalent.receipt());
    assert_eq!(
        owner.float_content_vertical(),
        BlockVertical {
            top: 19.0,
            bottom: 21.0
        }
    );
    assert_eq!(
        equivalent.float_content_vertical(),
        BlockVertical {
            top: 16.0,
            bottom: 25.0
        }
    );
}

/// One direct-core FVM input on the N6 fixture (P content top 11, B1 0/1/4).
#[derive(Clone, Copy)]
struct FvmCore {
    containing: Band,
    side: Float,
    width: f32,
    left: AutoLength,
    right: AutoLength,
    b1_bottom: f32,
    height: f32,
    top: AutoLength,
    bottom: AutoLength,
}

fn fvm_core() -> FvmCore {
    FvmCore {
        containing: Band {
            left: 0.0,
            right: 100.0,
        },
        side: Float::Left,
        width: 10.0,
        left: AutoLength::Value(0.0),
        right: AutoLength::Value(0.0),
        b1_bottom: 4.0,
        height: 2.0,
        top: AutoLength::Value(0.0),
        bottom: AutoLength::Value(0.0),
    }
}

fn fvm_first_float(
    case: FvmCore,
) -> Result<
    (
        iced_css_core::formatting::floats::PlacedFloat,
        BlockVertical,
    ),
    ClearanceSequenceError,
> {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = case.containing;
    children[0].margin.bottom = AutoLength::Value(case.b1_bottom);
    children[1].display.float = case.side;
    children[1].margin.left = case.left;
    children[1].margin.right = case.right;
    children[1].margin.top = case.top;
    children[1].margin.bottom = case.bottom;
    children[1].height = OwnerHeight::Explicit(case.height);
    build_width(&sources, outer, parent, &children, case.width, limits(6))
        .map(|owner| (owner.placed_float(), owner.float_content_vertical()))
}

/// The typed source-tagged placement failure, for `assert_eq!` (NFH-T2Q-004).
fn placement_failure<T: std::fmt::Debug>(
    result: Result<T, ClearanceSequenceError>,
) -> (usize, ClearanceOwnerError) {
    match result {
        Err(ClearanceSequenceError::Placement { source, error }) => (source, error),
        other => panic!("expected a placement failure, got {other:?}"),
    }
}

fn unrepresentable(stage: &'static str) -> ClearanceOwnerError {
    ClearanceOwnerError::Unrepresentable { stage }
}

#[test]
fn nonzero_float_vertical_margin_stages_fail_with_named_stages() {
    let v = |bits: u32| AutoLength::Value(f32::from_bits(bits));
    // float_outer_height: MAX + 2^103 rounds to infinity on both sides.
    for side in [Float::Left, Float::Right] {
        let case = FvmCore {
            side,
            height: f32::MAX,
            top: v(F32_2P103),
            ..fvm_core()
        };
        assert_eq!(
            placement_failure(fvm_first_float(case)),
            (3, unrepresentable("float_outer_height"))
        );
    }
    // Placement via height: float top MAX, margin-box bottom MAX + 2^103.
    for side in [Float::Left, Float::Right] {
        let case = FvmCore {
            side,
            b1_bottom: f32::MAX,
            height: 0.0,
            top: v(F32_2P103),
            ..fvm_core()
        };
        assert_eq!(
            placement_failure(fvm_first_float(case)),
            (
                3,
                ClearanceOwnerError::FloatPlacement(
                    iced_css_core::formatting::floats::FloatError::Unrepresentable
                )
            )
        );
    }
    // float_content_bottom: the margin box is finite (top MAX - ulp, bottom MAX), but
    // the content top rounds up to MAX and the bottom published from it overflows.
    for side in [Float::Left, Float::Right] {
        let case = FvmCore {
            side,
            b1_bottom: f32::from_bits(F32_MAX_MINUS_ULP),
            height: f32::from_bits(F32_2P103),
            top: v(F32_2P103_PLUS_2P80),
            ..fvm_core()
        };
        assert_eq!(
            placement_failure(fvm_first_float(case)),
            (3, unrepresentable("float_content_bottom"))
        );
    }
}

#[test]
fn horizontal_float_stages_precede_vertical_float_stages() {
    let v = |bits: u32| AutoLength::Value(f32::from_bits(bits));
    // Both outer extents overflow: the horizontal outer stage is reported.
    for side in [Float::Left, Float::Right] {
        let case = FvmCore {
            side,
            width: f32::MAX,
            left: v(F32_2P103),
            height: f32::MAX,
            top: v(F32_2P103),
            ..fvm_core()
        };
        assert_eq!(
            placement_failure(fvm_first_float(case)),
            (3, unrepresentable("float_outer_width"))
        );
    }
    // Both far content edges overflow: the horizontal far edge is reported.
    let case = FvmCore {
        containing: Band {
            left: f32::from_bits(F32_MAX_MINUS_ULP),
            right: f32::from_bits(F32_MAX_MINUS_ULP),
        },
        width: f32::from_bits(F32_2P103),
        left: v(F32_2P103_PLUS_2P80),
        b1_bottom: f32::from_bits(F32_MAX_MINUS_ULP),
        height: f32::from_bits(F32_2P103),
        top: v(F32_2P103_PLUS_2P80),
        ..fvm_core()
    };
    assert_eq!(
        placement_failure(fvm_first_float(case)),
        (3, unrepresentable("float_content_right"))
    );
    // Control: the vertical far edge alone fails float_content_bottom.
    let case = FvmCore {
        left: AutoLength::Value(0.0),
        width: 10.0,
        containing: fvm_core().containing,
        ..case
    };
    assert_eq!(
        placement_failure(fvm_first_float(case)),
        (3, unrepresentable("float_content_bottom"))
    );
}

#[test]
fn negative_zero_vertical_margins_and_height_take_the_identity_path() {
    let sources = sources(6);
    for side in [Float::Left, Float::Right] {
        let (outer, parent, mut children) = facts(&sources);
        children[1].display.float = side;
        children[1].margin.top = AutoLength::Value(-0.0);
        children[1].margin.bottom = AutoLength::Value(-0.0);
        children[1].height = OwnerHeight::Explicit(-0.0);
        // A zero-height float would put each clearing tail's hypothetical top at the
        // float bottom (UnsupportedEquality); as in the ruled zero-extent control
        // (FVM-P1-001), the tails do not clear.
        for child in &mut children[2..] {
            child.clear = ClearSide::None;
        }
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        let placed = owner.placed_float();
        let span = owner.float_content_vertical();
        assert_eq!(
            (span.top.to_bits(), span.bottom.to_bits()),
            (placed.top.to_bits(), placed.bottom.to_bits())
        );
        assert_eq!((placed.top, placed.bottom), (16.0, 16.0));
    }
}

#[test]
fn nonzero_float_vertical_margin_admission_keeps_fault_precedence() {
    let sources = sources(6);
    // A negative vertical edge beside a positive one is admitted (SFM, C9): margin box
    // 16..22, content 15..17, B2 required at 22 with clearance 3.
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(-1.0);
    children[1].margin.bottom = AutoLength::Value(5.0);
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 22.0],
            [0.0, 10.0, 15.0, 17.0],
            &[(22.0, 23.0, Some(3.0)), (26.0, 27.0, None)]
        )
    );
    // A nonfinite vertical edge is InvalidNumber before any sign check.
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(f32::NAN);
    children[1].margin.bottom = AutoLength::Value(-1.0);
    assert_eq!(
        profile(build(&sources, outer, parent, &children, limits(6))),
        "InvalidNumber { source: 3, field: \"margin\" }"
    );
    // A later child's profile fault precedes the float-width domain check.
    for later_fault in [true, false] {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.top = AutoLength::Value(3.0);
        children[1].margin.bottom = AutoLength::Value(4.0);
        if later_fault {
            children[3].border.left = 1.0;
        }
        let result = build_width(&sources, outer, parent, &children, -1.0, limits(6));
        let expected = if later_fault {
            "Unsupported { source: 5, reason: ChildDecoration }"
        } else {
            "InvalidNumber { source: 3, field: \"float_width\" }"
        };
        assert_eq!(profile(result), expected);
    }
    // Valid nonzero vertical margins whose kernel would fail: the byte floor first.
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(f32::from_bits(F32_2P103));
    children[1].height = OwnerHeight::Explicit(f32::MAX);
    let floor = 2 * size_of::<SequenceNormalPlacement>();
    let mut limit = limits(6);
    limit.retained_bytes = NonZeroUsize::new(floor - 1).unwrap();
    assert!(matches!(
        build(&sources, outer, parent, &children, limit),
        Err(ClearanceSequenceError::Bytes)
    ));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_outer_height"))
    );
}

#[test]
fn float_outer_height_failure_precedes_later_unsupported_equality() {
    let sources = sources(5);
    let equality = |top: f32, bottom: f32| {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.top = AutoLength::Value(top);
        children[1].margin.bottom = AutoLength::Value(bottom);
        children[1].height = OwnerHeight::Explicit(0.);
        build(&sources, outer, parent, &children, limits(5)).map(|_| ())
    };
    assert_eq!(
        placement_failure(equality(0.0, 0.0)),
        (4, ClearanceOwnerError::UnsupportedEquality)
    );
    assert_eq!(
        placement_failure(equality(f32::MAX, f32::from_bits(F32_2P103))),
        (3, unrepresentable("float_outer_height"))
    );
}

#[test]
fn nonzero_float_vertical_margin_work_bounds_are_exact() {
    for (count, work, old_work) in [(5, 945, 419), (6, 1107, 501), (7, 1270, 584)] {
        let margins = |children: &mut Vec<ChildFacts<'_>>| {
            children[1].margin.top = AutoLength::Value(3.0);
            children[1].margin.bottom = AutoLength::Value(4.0);
        };
        // One-short and predecessor bounds fail before the graph scan would report
        // the malformed child count; the exact bound reaches that scan.
        let mut malformed = sources(count);
        let unrelated = malformed[0].instance;
        malformed[count - 1].children.push(unrelated);
        let (outer, parent, mut children) = facts(&malformed);
        margins(&mut children);
        for short in [work - 1, old_work] {
            let mut limit = limits(count);
            limit.work = NonZeroUsize::new(short).unwrap();
            assert!(matches!(
                build(&malformed, outer, parent, &children, limit),
                Err(ClearanceSequenceError::Work)
            ));
        }
        let mut exact = limits(count);
        exact.work = NonZeroUsize::new(work).unwrap();
        assert!(matches!(
            build(&malformed, outer, parent, &children, exact),
            Err(ClearanceSequenceError::Profile(ClearanceOwnerError::ChildCount { source }))
                if source == count - 1
        ));

        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        margins(&mut children);
        let tail_count = count - 4;
        let floor = tail_count * size_of::<SequenceNormalPlacement>();
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work).unwrap();
        limit.retained_bytes = NonZeroUsize::new(floor).unwrap();
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        assert_eq!(owner.receipt().work, work);
        assert_eq!(owner.receipt().record_capacity, tail_count);
        assert_eq!(owner.receipt().retained_bytes, floor);
        assert_eq!(
            (owner.placed_float().top, owner.placed_float().bottom),
            (16.0, 25.0)
        );
        assert_eq!(
            owner.float_content_vertical(),
            BlockVertical {
                top: 19.0,
                bottom: 21.0
            }
        );
    }
}

fn assert_leading_clear_is_inert(slot: usize) {
    let sources = sources(6);
    let (outer, parent, control) = facts(&sources);
    let control = build(&sources, outer, parent, &control, limits(6)).unwrap();
    for side in [ClearSide::Left, ClearSide::Right, ClearSide::Both] {
        let (outer, parent, mut children) = facts(&sources);
        children[slot].clear = side;
        let owner = build(&sources, outer, parent, &children, limits(6))
            .unwrap_or_else(|error| panic!("slot {slot} clear {side:?}: {error:?}"));
        // Debug of the whole owner: sources, before, margin box, both content spans,
        // every tail record and the receipt; finite f32 Debug is bit-exact.
        assert_eq!(
            format!("{owner:?}"),
            format!("{control:?}"),
            "slot {slot} clear {side:?}"
        );
        let b2 = &owner.tail()[0];
        assert_eq!(
            (
                b2.vertical().top.to_bits(),
                b2.clearance().map(f32::to_bits)
            ),
            (18.0_f32.to_bits(), Some((-1.0_f32).to_bits())),
            "slot {slot} clear {side:?}"
        );
    }
}

#[test]
fn leading_clear_on_b1_is_admitted_with_no_effect() {
    assert_leading_clear_is_inert(0);
}

#[test]
fn leading_clear_on_float_is_admitted_with_no_effect() {
    assert_leading_clear_is_inert(1);
}

#[test]
fn float_clear_no_longer_masks_the_float_width_domain() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].clear = ClearSide::Both;
    let error = build_width(&sources, outer, parent, &children, f32::NAN, limits(6)).unwrap_err();
    assert!(
        matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 3,
                field: "float_width",
            })
        ),
        "{error:?}"
    );
}

/// The typed profile failure, for `assert_eq!` (NFH-T2Q-004, FVM-T2Q-001).
fn profile_error<T: std::fmt::Debug>(
    result: Result<T, ClearanceSequenceError>,
) -> ClearanceOwnerError {
    match result {
        Err(ClearanceSequenceError::Profile(error)) => error,
        other => panic!("expected a profile failure, got {other:?}"),
    }
}

#[test]
fn leading_clear_masks_no_sequence_profile_fault_bytes_or_kernel() {
    let sources = sources(6);
    // B1 `clear` with its own decoration: the decoration is the fault, not the clear.
    let (outer, parent, mut children) = facts(&sources);
    children[0].clear = ClearSide::Left;
    children[0].border.left = 1.0;
    assert_eq!(
        profile_error(build(&sources, outer, parent, &children, limits(6))),
        ClearanceOwnerError::Unsupported {
            source: 2,
            reason: OwnerProfileReason::ChildDecoration,
        }
    );
    // F `clear` with a negative bottom margin: the clear selects nothing and the margin is
    // admitted (SFM, C8): margin box 16..17, B2 required at 17 with clearance -2.
    let (outer, parent, mut children) = facts(&sources);
    children[1].clear = ClearSide::Both;
    children[1].margin.bottom = AutoLength::Value(-1.0);
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 17.0],
            [0.0, 10.0, 16.0, 18.0],
            &[(17.0, 18.0, Some(-2.0)), (21.0, 22.0, None)]
        )
    );
    // B1 and F `clear` with too few retained bytes: Bytes, before any kernel.
    let tiny_bytes = ClearanceSequenceLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits(6)
    };
    let (outer, parent, mut children) = facts(&sources);
    children[0].clear = ClearSide::Both;
    children[1].clear = ClearSide::Both;
    let result = build(&sources, outer, parent, &children, tiny_bytes);
    assert!(
        matches!(result, Err(ClearanceSequenceError::Bytes)),
        "{result:?}"
    );
    // The same with a float whose outer-height kernel would fail: still Bytes first,
    // and the kernel failure with sufficient bytes.
    children[1].margin.top = AutoLength::Value(f32::from_bits(F32_2P103));
    children[1].height = OwnerHeight::Explicit(f32::MAX);
    let result = build(&sources, outer, parent, &children, tiny_bytes);
    assert!(
        matches!(result, Err(ClearanceSequenceError::Bytes)),
        "{result:?}"
    );
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_outer_height"))
    );
}

#[test]
fn leading_clear_is_inert_at_five_and_seven_sources() {
    for count in [5, 7] {
        let sources = sources(count);
        let (outer, parent, control) = facts(&sources);
        let control = build(&sources, outer, parent, &control, limits(count)).unwrap();
        let (outer, parent, mut children) = facts(&sources);
        children[0].clear = ClearSide::Right;
        let owner = build(&sources, outer, parent, &children, limits(count))
            .unwrap_or_else(|error| panic!("N{count}: {error:?}"));
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "N{count}");
    }
}

// Q1: a Sequence B1 labelled `OtherNewBfc` admits boundary contact with F's margin
// box. Clearance equality remains independently unsupported.

/// B1's border bottom at exactly 2^24 (content top 11 plus this height), where a 1px bottom margin
/// rounds away.
const B1_HEIGHT_TO_2P24: f32 = 16_777_205.0;

#[test]
fn sequence_b1_other_new_bfc_strictly_above_the_float_matches_ordinary() {
    for count in [5, 6, 7] {
        let sources = sources(count);
        let (outer, parent, control) = facts(&sources);
        let control = build(&sources, outer, parent, &control, limits(count)).unwrap();
        let (outer, parent, mut children) = facts(&sources);
        children[0].context = ChildContext::OtherNewBfc;
        let owner = build(&sources, outer, parent, &children, limits(count))
            .unwrap_or_else(|error| panic!("N{count}: {error:?}"));
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "N{count}");
        assert!(owner.placed_float().top > owner.before().bottom, "N{count}");
    }
}

#[test]
fn sequence_b1_other_new_bfc_contact_is_admitted() {
    let sources = sources(6);
    let mut rows = 0;
    for (height, bottom) in [
        (1.0, AutoLength::Value(0.0)),
        (1.0, AutoLength::Auto),
        (1.0, AutoLength::Value(-0.0)),
        (B1_HEIGHT_TO_2P24, AutoLength::Value(1.0)),
    ] {
        let (outer, parent, mut children) = facts(&sources);
        children[0].height = OwnerHeight::Explicit(height);
        children[0].margin.bottom = bottom;
        let control = build(&sources, outer, parent, &children, limits(6))
            .unwrap_or_else(|error| panic!("{bottom:?}: {error:?}"));
        assert_eq!(
            control.placed_float().top.to_bits(),
            control.before().bottom.to_bits(),
            "{bottom:?}"
        );
        children[0].context = ChildContext::OtherNewBfc;
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "{bottom:?}");
        rows += 1;
    }
    assert_eq!(rows, 4);
}

#[test]
fn sequence_b1_contact_exposes_tail_equality() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.bottom = AutoLength::Value(0.0);
    children[2].margin.top = AutoLength::Value(2.0);
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (4, ClearanceOwnerError::UnsupportedEquality)
    );
    children[0].context = ChildContext::OtherNewBfc;
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (4, ClearanceOwnerError::UnsupportedEquality)
    );
}

// Leading overflow Task2: the context exception stays at B1, and the work bound is exact.

#[test]
fn sequence_other_new_bfc_stays_rejected_off_b1() {
    let sources = sources(6);
    let mut rows = 0;
    for (slot, source) in [(1, 3), (2, 4), (3, 5)] {
        let (outer, parent, mut children) = facts(&sources);
        children[slot].context = ChildContext::OtherNewBfc;
        assert_eq!(
            profile_error(build(&sources, outer, parent, &children, limits(6))),
            ClearanceOwnerError::Unsupported {
                source,
                reason: OwnerProfileReason::ChildContext,
            }
        );
        rows += 1;
    }
    assert_eq!(rows, 3);
}

#[test]
fn sequence_b1_other_new_bfc_work_bounds_are_exact() {
    for (count, work) in [(5, 945), (6, 1107), (7, 1270)] {
        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        children[0].context = ChildContext::OtherNewBfc;
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work - 1).unwrap();
        assert!(matches!(
            build(&sources, outer, parent, &children, limit),
            Err(ClearanceSequenceError::Work)
        ));
        limit.work = NonZeroUsize::new(work).unwrap();
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        assert_eq!(owner.receipt().work, work);
        assert_eq!(
            work,
            560 + 8 * count
                + count * (count - 1) / 2
                + 24 * (count - 4)
                + 61 * (count - 2)
                + 64 * (count - 3)
        );
    }
}

// Contact margin tests (LO-T2Q-001): a positive float top margin moves the float's content top
// below B1's border bottom; the contact check reads the placed margin box.

#[test]
fn sequence_b1_contact_reads_the_float_margin_box() {
    let mut rows = 0;
    for count in [5, 6, 7] {
        let sources = sources(count);
        for (height, bottom, margin_top, margin_box_top, content_top) in [
            (1.0, AutoLength::Value(0.0), 3.0, 12.0_f32, 15.0_f32),
            (1.0, AutoLength::Auto, 3.0, 12.0, 15.0),
            // B1's bottom at 2^24 absorbs its 1px margin; F's 4px top margin survives rounding.
            (
                B1_HEIGHT_TO_2P24,
                AutoLength::Value(1.0),
                4.0,
                16_777_216.0,
                16_777_220.0,
            ),
        ] {
            let (outer, parent, mut children) = facts(&sources);
            children[0].height = OwnerHeight::Explicit(height);
            children[0].margin.bottom = bottom;
            children[1].margin.top = AutoLength::Value(margin_top);
            let control = build(&sources, outer, parent, &children, limits(count))
                .unwrap_or_else(|error| panic!("N{count} {bottom:?}: {error:?}"));
            assert_eq!(
                [
                    control.before().bottom,
                    control.placed_float().top,
                    control.float_content_vertical().top,
                ]
                .map(f32::to_bits),
                [margin_box_top, margin_box_top, content_top].map(f32::to_bits),
                "N{count} {bottom:?}"
            );
            children[0].context = ChildContext::OtherNewBfc;
            let owner = build(&sources, outer, parent, &children, limits(count)).unwrap();
            assert_eq!(
                format!("{owner:?}"),
                format!("{control:?}"),
                "N{count} {bottom:?}"
            );
            rows += 1;
        }
    }
    assert_eq!(rows, 9);
}

#[test]
fn sequence_b1_other_new_bfc_above_a_float_top_margin_matches_ordinary() {
    for count in [5, 6, 7] {
        let sources = sources(count);
        let (outer, parent, mut control) = facts(&sources);
        control[0].margin.bottom = AutoLength::Value(1.0);
        control[1].margin.top = AutoLength::Value(3.0);
        let mut children = control.clone();
        children[0].context = ChildContext::OtherNewBfc;
        let control = build(&sources, outer, parent, &control, limits(count)).unwrap();
        assert_eq!(
            [
                control.before().bottom,
                control.placed_float().top,
                control.float_content_vertical().top,
            ]
            .map(f32::to_bits),
            [12.0_f32, 13.0, 16.0].map(f32::to_bits),
            "N{count}"
        );
        let owner = build(&sources, outer, parent, &children, limits(count))
            .unwrap_or_else(|error| panic!("N{count}: {error:?}"));
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "N{count}");
    }
}

/// LO-T2Q-002: a Sequence B1 labelled `FloatBfc`, reachable only through this API, stays a
/// `ChildContext` profile error. A signed B1 bottom margin is admitted, then
/// intersecting nonempty B1 border/F margin intervals reject at source-2 placement.
#[test]
fn sequence_b1_context_and_signed_overlap_keep_typed_errors() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].context = ChildContext::FloatBfc;
    assert_eq!(
        profile_error(build(&sources, outer, parent, &children, limits(6))),
        ClearanceOwnerError::Unsupported {
            source: 2,
            reason: OwnerProfileReason::ChildContext,
        }
    );
    let (outer, parent, mut children) = facts(&sources);
    children[0].context = ChildContext::OtherNewBfc;
    children[0].margin.bottom = AutoLength::Value(-1.0);
    children[1].margin.top = AutoLength::Value(3.0);
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Placement {
            source: 2,
            error: ClearanceOwnerError::NewBfcFloatContact
        })
    ));
}

/// CMT-D-004: with zero height and zero vertical margins, F's margin box is empty (12..12) and
/// touches B1's border bottom. Q1 admits the new-BFC B1 with the same layout as
/// the `OrdinaryNonBfc` control.
#[test]
fn sequence_b1_contact_with_an_empty_float_margin_box_is_admitted() {
    for count in [5, 6, 7] {
        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        children[0].margin.bottom = AutoLength::Value(0.0);
        children[1].height = OwnerHeight::Explicit(0.0);
        let control = build(&sources, outer, parent, &children, limits(count))
            .unwrap_or_else(|error| panic!("N{count}: {error:?}"));
        assert_eq!(
            [
                control.before().bottom,
                control.placed_float().top,
                control.placed_float().bottom,
                control.float_content_vertical().top,
            ]
            .map(f32::to_bits),
            [12.0_f32; 4].map(f32::to_bits),
            "N{count}"
        );
        children[0].context = ChildContext::OtherNewBfc;
        let owner = build(&sources, outer, parent, &children, limits(count)).unwrap();
        assert_eq!(format!("{owner:?}"), format!("{control:?}"), "N{count}");
    }
}

// Signed float margins (SFM plan): the core rows. F's own margins may be negative while both exact
// outer extents are nonnegative; otherwise the profile rejects F with `NegativeFloatOuterExtent`.
// Every expected value below is the ported model's (plan package, outputs/reds-model.json).

/// 2^112, the near-edge magnitude of the design's sealed scripts (SFM-D-004).
const SFM_2P112: u32 = 0x7780_0000;
/// 2^60, the SFM-D-001 witness magnitude.
const SFM_2P60: u32 = 0x5d80_0000;

/// The core-owned geometry an SFM row pins, as binary32 bits: B1's top and bottom, F's margin box
/// (left, right, top, bottom), F's content band and vertical span (left, right, top, bottom), and
/// every tail's top, bottom and clearance.
#[derive(Debug, PartialEq)]
struct SfmCoreView {
    before: [u32; 2],
    margin_box: [u32; 4],
    content: [u32; 4],
    tails: Vec<(u32, u32, Option<u32>)>,
}

fn sfm_core_view(
    owner: &iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'_>,
) -> SfmCoreView {
    let before = owner.before();
    let placed = owner.placed_float();
    let band = owner.float_content();
    let span = owner.float_content_vertical();
    SfmCoreView {
        before: [before.top, before.bottom].map(f32::to_bits),
        margin_box: [placed.left, placed.right, placed.top, placed.bottom].map(f32::to_bits),
        content: [band.left, band.right, span.top, span.bottom].map(f32::to_bits),
        tails: owner
            .tail()
            .iter()
            .map(|tail| {
                (
                    tail.vertical().top.to_bits(),
                    tail.vertical().bottom.to_bits(),
                    tail.clearance().map(f32::to_bits),
                )
            })
            .collect(),
    }
}

fn sfm_core_expected(
    before: [f32; 2],
    margin_box: [f32; 4],
    content: [f32; 4],
    tails: &[(f32, f32, Option<f32>)],
) -> SfmCoreView {
    SfmCoreView {
        before: before.map(f32::to_bits),
        margin_box: margin_box.map(f32::to_bits),
        content: content.map(f32::to_bits),
        tails: tails
            .iter()
            .map(|&(top, bottom, clearance)| {
                (top.to_bits(), bottom.to_bits(), clearance.map(f32::to_bits))
            })
            .collect(),
    }
}

/// The outer-extent rejection at F, typed.
fn assert_sfm_core_extent_rejected<T: std::fmt::Debug>(result: Result<T, ClearanceSequenceError>) {
    assert_eq!(
        profile_error(result),
        ClearanceOwnerError::Unsupported {
            source: 3,
            reason: OwnerProfileReason::NegativeFloatOuterExtent,
        }
    );
}

/// C2: a later child's decoration fault (source 5) precedes F's extent error.
#[test]
fn signed_float_margins_later_decoration_fault_comes_first() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-11.0);
    children[3].border.left = 1.0;
    assert_eq!(
        profile_error(build(&sources, outer, parent, &children, limits(6))),
        ClearanceOwnerError::Unsupported {
            source: 5,
            reason: OwnerProfileReason::ChildDecoration
        }
    );
}

/// C2b: the outer width -1 is rejected.
#[test]
fn signed_float_margins_negative_outer_width_is_typed() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-11.0);
    assert_sfm_core_extent_rejected(build(&sources, outer, parent, &children, limits(6)));
}

/// C3: the float-width domain precedes F's extent error.
#[test]
fn signed_float_margins_float_width_domain_comes_first() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-11.0);
    assert_eq!(
        profile_error(build_width(
            &sources,
            outer,
            parent,
            &children,
            -1.0,
            limits(6)
        )),
        ClearanceOwnerError::InvalidNumber {
            source: 3,
            field: "float_width"
        }
    );
}

/// C4: F's extent error precedes the retained-byte floor.
#[test]
fn signed_float_margins_outer_extent_precedes_the_byte_floor() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-11.0);
    let mut limit = limits(6);
    limit.retained_bytes = NonZeroUsize::new(2 * size_of::<SequenceNormalPlacement>() - 1).unwrap();
    assert_sfm_core_extent_rejected(build(&sources, outer, parent, &children, limit));
}

/// C4b: an admitted signed F reaches the retained-byte floor.
#[test]
fn signed_float_margins_admitted_signed_float_reaches_the_byte_floor() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-1.0);
    let mut limit = limits(6);
    limit.retained_bytes = NonZeroUsize::new(2 * size_of::<SequenceNormalPlacement>() - 1).unwrap();
    let result = build(&sources, outer, parent, &children, limit);
    assert!(
        matches!(result, Err(ClearanceSequenceError::Bytes)),
        "{result:?}"
    );
}

/// C5: the exact extent -1 + 2^25 - 2^25 = -1 is rejected; a left-to-right f32 sum would give 0.
#[test]
fn signed_float_margins_exact_outer_width_rejects_minus_one_beside_2p25() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-1.0);
    children[1].margin.right = AutoLength::Value(-33_554_432.0);
    assert_sfm_core_extent_rejected(build_width(
        &sources,
        outer,
        parent,
        &children,
        33_554_432.0,
        limits(6),
    ));
}

/// C5a: the exact extent 3 + 2^25 - 2^25 = 3 lays out: margin box 0..3.
#[test]
fn signed_float_margins_exact_outer_width_admits_three_beside_2p25() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(3.0);
    children[1].margin.right = AutoLength::Value(-33_554_432.0);
    assert_eq!(
        sfm_core_view(
            &build_width(&sources, outer, parent, &children, 33_554_432.0, limits(6)).unwrap()
        ),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 3.0, 16.0, 18.0],
            [3.0, 33_554_436.0, 16.0, 18.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)]
        )
    );
}

/// C5v: the exact vertical extent -1 is rejected.
#[test]
fn signed_float_margins_exact_outer_height_rejects_minus_one_beside_2p25() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].height = OwnerHeight::Explicit(33_554_432.0);
    children[1].margin.top = AutoLength::Value(-1.0);
    children[1].margin.bottom = AutoLength::Value(-33_554_432.0);
    assert_sfm_core_extent_rejected(build(&sources, outer, parent, &children, limits(6)));
}

/// C5c: the exact extent -2 MAX + 10 is rejected with the typed reason, never `Unrepresentable`.
#[test]
fn signed_float_margins_extreme_outer_width_is_typed_not_unrepresentable() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-f32::MAX);
    children[1].margin.right = AutoLength::Value(-f32::MAX);
    assert_sfm_core_extent_rejected(build(&sources, outer, parent, &children, limits(6)));
}

/// C6: the outer width is exactly 0.
#[test]
fn signed_float_margins_zero_outer_width_is_admitted() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-10.0);
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 0.0, 16.0, 18.0],
            [-10.0, 0.0, 16.0, 18.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)]
        )
    );
}

/// C6v: the outer height is exactly 0 (B2 `clear:none`).
#[test]
fn signed_float_margins_zero_outer_height_is_admitted() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.top = AutoLength::Value(-2.0);
    children[2].clear = ClearSide::None;
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 16.0],
            [0.0, 10.0, 14.0, 16.0],
            &[(16.0, 17.0, None), (20.0, 21.0, None)]
        )
    );
}

/// C7: the near sum -MAX - 2^112 of a left F leaves the finite range.
#[test]
fn signed_float_margins_near_left_edge_overflow_is_typed() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = Band {
        left: -f32::MAX,
        right: -f32::MAX,
    };
    children[1].margin.left = AutoLength::Value(-f32::from_bits(SFM_2P112));
    children[1].margin.right = AutoLength::Value(f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_content_left"))
    );
}

/// C7r: the near sum MAX + 2^112 of a right F leaves the finite range.
#[test]
fn signed_float_margins_near_right_edge_overflow_is_typed() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = Band {
        left: f32::MAX,
        right: f32::MAX,
    };
    children[1].display.float = Float::Right;
    children[1].margin.right = AutoLength::Value(-f32::from_bits(SFM_2P112));
    children[1].margin.left = AutoLength::Value(f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_content_right"))
    );
}

/// C7p: the near margin 2^112 beside the far margin -2^112 leaves the finite range.
#[test]
fn signed_float_margins_near_left_edge_overflow_beside_a_negative_right_is_typed() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = Band {
        left: f32::MAX,
        right: f32::MAX,
    };
    children[1].margin.left = AutoLength::Value(f32::from_bits(SFM_2P112));
    children[1].margin.right = AutoLength::Value(-f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build_width(
            &sources,
            outer,
            parent,
            &children,
            0.0,
            limits(6)
        )),
        (3, unrepresentable("float_content_left"))
    );
}

/// C7pr: a right F's near margin 2^112 beside the far margin -2^112 leaves the finite range.
#[test]
fn signed_float_margins_near_right_edge_overflow_beside_a_negative_left_is_typed() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.containing = Band {
        left: -f32::MAX,
        right: -f32::MAX,
    };
    children[1].display.float = Float::Right;
    children[1].margin.right = AutoLength::Value(f32::from_bits(SFM_2P112));
    children[1].margin.left = AutoLength::Value(-f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build_width(
            &sources,
            outer,
            parent,
            &children,
            0.0,
            limits(6)
        )),
        (3, unrepresentable("float_content_right"))
    );
}

/// C7t: the content top -MAX - 2^112 leaves the finite range.
#[test]
fn signed_float_margins_near_top_edge_overflow_above_is_typed() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.content_top = -f32::MAX;
    children[1].margin.top = AutoLength::Value(-f32::from_bits(SFM_2P112));
    children[1].margin.bottom = AutoLength::Value(f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_content_top"))
    );
}

/// C7tp: the content top MAX + 2^112 leaves the finite range.
#[test]
fn signed_float_margins_near_top_edge_overflow_below_is_typed() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.bottom = AutoLength::Value(f32::MAX);
    children[1].margin.top = AutoLength::Value(f32::from_bits(SFM_2P112));
    children[1].margin.bottom = AutoLength::Value(-f32::from_bits(SFM_2P112));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (3, unrepresentable("float_content_top"))
    );
}

/// C15 (SFM-D-001): -1 + 2^60 - 2^60 = -1 is rejected; an f64 sum would give 0.
#[test]
fn signed_float_margins_exact_outer_width_rejects_minus_one_beside_2p60() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.left = AutoLength::Value(-1.0);
    children[1].margin.right = AutoLength::Value(-f32::from_bits(SFM_2P60));
    assert_sfm_core_extent_rejected(build_width(
        &sources,
        outer,
        parent,
        &children,
        f32::from_bits(SFM_2P60),
        limits(6),
    ));
}

/// C15c (SFM-D-001): the control 0 + 2^60 - 2^60 = 0 lays out.
#[test]
fn signed_float_margins_exact_outer_width_admits_zero_beside_2p60() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].margin.right = AutoLength::Value(-f32::from_bits(SFM_2P60));
    assert_eq!(
        sfm_core_view(
            &build_width(
                &sources,
                outer,
                parent,
                &children,
                f32::from_bits(SFM_2P60),
                limits(6)
            )
            .unwrap()
        ),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 0.0, 16.0, 18.0],
            [0.0, f32::from_bits(SFM_2P60), 16.0, 18.0],
            &[(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)]
        )
    );
}

/// C16 (SFM-D-001): the vertical twin of C15 (B2 `clear:none`).
#[test]
fn signed_float_margins_exact_outer_height_rejects_minus_one_beside_2p60() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].height = OwnerHeight::Explicit(f32::from_bits(SFM_2P60));
    children[1].margin.top = AutoLength::Value(-1.0);
    children[1].margin.bottom = AutoLength::Value(-f32::from_bits(SFM_2P60));
    children[2].clear = ClearSide::None;
    assert_sfm_core_extent_rejected(build(&sources, outer, parent, &children, limits(6)));
}

/// C16c (SFM-D-001): the vertical control lays out.
#[test]
fn signed_float_margins_exact_outer_height_admits_zero_beside_2p60() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[1].height = OwnerHeight::Explicit(f32::from_bits(SFM_2P60));
    children[1].margin.bottom = AutoLength::Value(-f32::from_bits(SFM_2P60));
    children[2].clear = ClearSide::None;
    assert_eq!(
        sfm_core_view(&build(&sources, outer, parent, &children, limits(6)).unwrap()),
        sfm_core_expected(
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 16.0],
            [0.0, 10.0, 16.0, f32::from_bits(SFM_2P60)],
            &[(16.0, 17.0, None), (20.0, 21.0, None)]
        )
    );
}

// Append to crates/iced-css-core/tests/clearance_sequence.rs before production edits.
// These tests intentionally use only predecessor APIs, so their red is semantic.
#[test]
fn cp_b1_padding_moves_border_float_and_clearance() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].padding.top = 2.0;
    children[0].padding.bottom = 3.0;
    let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
    assert_eq!(
        owner.before(),
        BlockVertical {
            top: 11.0,
            bottom: 17.0
        }
    );
    assert_eq!(
        [owner.placed_float().top, owner.placed_float().bottom],
        [21.0, 23.0]
    );
    assert_eq!(
        owner.tail()[0].vertical(),
        BlockVertical {
            top: 23.0,
            bottom: 24.0
        }
    );
    assert_eq!(
        owner.tail()[1].vertical(),
        BlockVertical {
            top: 27.0,
            bottom: 28.0
        }
    );
}

#[test]
fn cp_float_padding_preserves_content_dimensions() {
    for right in [false, true] {
        let sources = sources(6);
        let (outer, parent, mut children) = facts(&sources);
        children[1].padding = OwnerEdges {
            top: 3.0,
            right: 2.0,
            bottom: 4.0,
            left: 1.0,
        };
        if right {
            children[1].display.float = Float::Right;
        }
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        let placed = owner.placed_float();
        assert_eq!(
            [placed.left, placed.right],
            if right { [87.0, 100.0] } else { [0.0, 13.0] }
        );
        assert_eq!([placed.top, placed.bottom], [16.0, 25.0]);
        assert_eq!(
            owner.float_content(),
            if right {
                Band {
                    left: 88.0,
                    right: 98.0,
                }
            } else {
                Band {
                    left: 1.0,
                    right: 11.0,
                }
            }
        );
        assert_eq!(
            owner.float_content_vertical(),
            BlockVertical {
                top: 19.0,
                bottom: 21.0
            }
        );
        assert_eq!(
            owner.tail()[0].vertical(),
            BlockVertical {
                top: 25.0,
                bottom: 26.0
            }
        );
        assert_eq!(
            owner.tail()[1].vertical(),
            BlockVertical {
                top: 29.0,
                bottom: 30.0
            }
        );
    }
}

#[test]
fn cp_q1_contact_and_zero_height_float_are_admitted() {
    for count in [5, 6, 7] {
        for height in [0.0, 2.0] {
            let sources = sources(count);
            let (outer, parent, mut children) = facts(&sources);
            children[0].context = ChildContext::OtherNewBfc;
            children[0].margin.bottom = AutoLength::Value(0.0);
            children[1].height = OwnerHeight::Explicit(height);
            let owner = build(&sources, outer, parent, &children, limits(count)).unwrap();
            assert_eq!(
                owner.placed_float().top.to_bits(),
                owner.before().bottom.to_bits()
            );
            assert_eq!(owner.placed_float().bottom, 12.0 + height);
        }
    }
}

#[test]
fn cp_q1_still_exposes_tail_clearance_equality() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].context = ChildContext::OtherNewBfc;
    children[0].margin.bottom = AutoLength::Value(0.0);
    children[2].margin.top = AutoLength::Value(2.0);
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (4, ClearanceOwnerError::UnsupportedEquality)
    );
}

#[test]
fn cp_padding_rescues_exact_negative_float_extent() {
    for horizontal in [true, false] {
        let sources = sources(6);
        let (outer, parent, mut children) = facts(&sources);
        if horizontal {
            children[1].margin.left = AutoLength::Value(-11.0);
            children[1].padding.left = 1.0;
        } else {
            children[1].margin.top = AutoLength::Value(-3.0);
            children[1].padding.top = 1.0;
            for child in &mut children[2..] {
                child.clear = ClearSide::None;
            }
        }
        let owner = build(&sources, outer, parent, &children, limits(6)).unwrap();
        let placed = owner.placed_float();
        if horizontal {
            assert_eq!(placed.left.to_bits(), placed.right.to_bits());
        } else {
            assert_eq!(placed.top.to_bits(), placed.bottom.to_bits());
        }
    }
}

#[test]
fn cp_later_profile_precedes_padding_arithmetic() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].padding.top = f32::MAX;
    children[0].height = OwnerHeight::Explicit(f32::MAX);
    children[3].border.left = 1.0;
    assert_eq!(
        profile_error(build(&sources, outer, parent, &children, limits(6))),
        ClearanceOwnerError::Unsupported {
            source: 5,
            reason: OwnerProfileReason::ChildDecoration
        }
    );
}

#[test]
fn cp_bytes_precede_padding_arithmetic() {
    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].padding.top = f32::MAX;
    children[0].height = OwnerHeight::Explicit(f32::MAX);
    let tiny = ClearanceSequenceLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits(6)
    };
    assert!(matches!(
        build(&sources, outer, parent, &children, tiny),
        Err(ClearanceSequenceError::Bytes)
    ));
    assert_eq!(
        placement_failure(build(&sources, outer, parent, &children, limits(6))),
        (2, unrepresentable("before_content_bottom"))
    );
}

// Append after Task 1 passes and is independently reviewed.
#[test]
fn cp_copied_content_and_border_spans_outlive_fact_scratch() {
    let sources = sources(6);
    let owner = {
        let (outer, parent, mut children) = facts(&sources);
        children[0].padding = OwnerEdges {
            top: 2.0,
            right: 0.0,
            bottom: 3.0,
            left: 0.0,
        };
        children[1].padding = OwnerEdges {
            top: 3.0,
            right: 2.0,
            bottom: 4.0,
            left: 1.0,
        };
        children[2].padding.top = 2.0;
        children[2].padding.bottom = 3.0;
        build(&sources, outer, parent, &children, limits(6)).unwrap()
    };
    assert_eq!(
        owner.before(),
        BlockVertical {
            top: 11.0,
            bottom: 17.0
        }
    );
    assert_eq!(
        owner.before_content(),
        BlockVertical {
            top: 13.0,
            bottom: 14.0
        }
    );
    assert_eq!(
        owner.float_border(),
        Band {
            left: 0.0,
            right: 13.0
        }
    );
    assert_eq!(
        owner.float_content(),
        Band {
            left: 1.0,
            right: 11.0
        }
    );
    assert_eq!(
        owner.float_border_vertical(),
        BlockVertical {
            top: 21.0,
            bottom: 30.0
        }
    );
    assert_eq!(
        owner.float_content_vertical(),
        BlockVertical {
            top: 24.0,
            bottom: 26.0
        }
    );
    assert_eq!(
        owner.tail()[0].vertical(),
        BlockVertical {
            top: 30.0,
            bottom: 36.0
        }
    );
    assert_eq!(
        owner.tail()[0].content_vertical(),
        BlockVertical {
            top: 32.0,
            bottom: 33.0
        }
    );
    assert_eq!(
        owner.tail()[1].vertical(),
        BlockVertical {
            top: 39.0,
            bottom: 40.0
        }
    );
    for (source, expected) in sources.iter().enumerate().take(6) {
        assert!(std::ptr::eq(owner.source(source).unwrap(), expected));
    }
}

#[test]
fn cp_padding_domain_checks_every_source_and_edge() {
    for count in [5, 6, 7] {
        let sources = sources(count);
        for source in 2..count {
            for edge in 0..4 {
                for bad in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    let (outer, parent, mut children) = facts(&sources);
                    let p = &mut children[source - 2].padding;
                    match edge {
                        0 => p.top = bad,
                        1 => p.right = bad,
                        2 => p.bottom = bad,
                        _ => p.left = bad,
                    }
                    assert_eq!(
                        profile_error(build(&sources, outer, parent, &children, limits(count))),
                        ClearanceOwnerError::InvalidNumber {
                            source,
                            field: "padding"
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn cp_exact_work_and_capacity_boundaries() {
    for (count, work) in [(5, 945), (6, 1107), (7, 1270)] {
        let sources = sources(count);
        let (outer, parent, mut children) = facts(&sources);
        children[0].padding.top = 2.0;
        let floor = (count - 4) * size_of::<SequenceNormalPlacement>();
        let measured = build(&sources, outer, parent, &children, limits(count))
            .unwrap()
            .receipt();
        let bytes = measured.retained_bytes;
        assert_eq!(
            bytes,
            measured.record_capacity * size_of::<SequenceNormalPlacement>()
        );
        assert!(bytes >= floor);
        let mut limit = limits(count);
        limit.work = NonZeroUsize::new(work - 1).unwrap();
        assert!(matches!(
            build(&sources, outer, parent, &children, limit),
            Err(ClearanceSequenceError::Work)
        ));
        limit.work = NonZeroUsize::new(work).unwrap();
        limit.retained_bytes = NonZeroUsize::new(floor - 1).unwrap();
        assert!(matches!(
            build(&sources, outer, parent, &children, limit),
            Err(ClearanceSequenceError::Bytes)
        ));
        limit.retained_bytes = NonZeroUsize::MAX;
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        let receipt = owner.receipt();
        assert_eq!(receipt.work, work);
        assert_eq!(
            receipt.retained_bytes,
            receipt.record_capacity * size_of::<SequenceNormalPlacement>()
        );
        assert!(receipt.record_capacity >= count - 4);
        limit.work = NonZeroUsize::new(work + 1).unwrap();
        limit.retained_bytes = NonZeroUsize::MAX;
        let above = build(&sources, outer, parent, &children, limit)
            .unwrap()
            .receipt();
        assert_eq!(above.work, work);
        assert_eq!(
            above.retained_bytes,
            above.record_capacity * size_of::<SequenceNormalPlacement>()
        );
        assert!(above.record_capacity >= count - 4);
    }
}

// Task 2 direct-core control: CSS parsing normalizes zero, so retain the input bits here.
#[test]
fn cp_negative_zero_padding_preserves_zero_axis_bits() {
    fn observed(
        owner: &iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'_>,
    ) -> Vec<u32> {
        let before = owner.before();
        let content = owner.before_content();
        let f = owner.placed_float();
        let fb = owner.float_border();
        let fv = owner.float_border_vertical();
        let fc = owner.float_content();
        let fcv = owner.float_content_vertical();
        let mut bits = [
            before.top,
            before.bottom,
            content.top,
            content.bottom,
            f.left,
            f.right,
            f.top,
            f.bottom,
            fb.left,
            fb.right,
            fv.top,
            fv.bottom,
            fc.left,
            fc.right,
            fcv.top,
            fcv.bottom,
        ]
        .map(f32::to_bits)
        .to_vec();
        for tail in owner.tail() {
            bits.extend(
                [
                    tail.vertical().top,
                    tail.vertical().bottom,
                    tail.content_vertical().top,
                    tail.content_vertical().bottom,
                    tail.hypothetical_top(),
                ]
                .map(f32::to_bits),
            );
            bits.push(u32::from(tail.clearance().is_some()));
            bits.push(tail.clearance().unwrap_or(0.0).to_bits());
        }
        bits
    }
    for count in 5..=7 {
        let sources = sources(count);
        for content_top in [11.0, 33_554_432.0] {
            let (outer, mut parent, mut children) = facts(&sources);
            parent.content_top = content_top;
            for child in &mut children[2..] {
                child.clear = ClearSide::None;
            }
            let baseline = build(&sources, outer, parent, &children, limits(count)).unwrap();
            if content_top == 33_554_432.0 {
                assert_eq!(
                    baseline.before().top.to_bits(),
                    baseline.before().bottom.to_bits()
                );
            }
            let expected = observed(&baseline);
            for source in 2..count {
                for edge in 0..4 {
                    let mut padded = children.clone();
                    let p = &mut padded[source - 2].padding;
                    let value = match edge {
                        0 => &mut p.top,
                        1 => &mut p.right,
                        2 => &mut p.bottom,
                        _ => &mut p.left,
                    };
                    *value = f32::from_bits(0x8000_0000);
                    assert_eq!(value.to_bits(), 0x8000_0000);
                    let owner = build(&sources, outer, parent, &padded, limits(count)).unwrap();
                    assert_eq!(
                        observed(&owner),
                        expected,
                        "N{count} source{source} edge{edge} top{content_top}"
                    );
                }
            }
        }
    }
}

// Append to crates/iced-css-core/tests/clearance_sequence.rs before production edits.
#[test]
fn cfh_core_auto_matches_positive_zero() {
    for count in [5, 6, 7] {
        for side in [Float::Left, Float::Right] {
            for padded in [false, true] {
                let nodes = sources(count);
                let (outer, parent, mut children) = facts(&nodes);
                children[1].display.float = side;
                for child in &mut children[2..] {
                    child.clear = ClearSide::None;
                }
                if padded {
                    children[1].padding.top = 2.0;
                    children[1].padding.bottom = 3.0;
                }
                children[1].height = OwnerHeight::Explicit(0.0);
                let expected = build(&nodes, outer, parent, &children, limits(count)).unwrap();
                children[1].height = OwnerHeight::Auto;
                let actual = build(&nodes, outer, parent, &children, limits(count)).unwrap();
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                let content = actual.float_content_vertical();
                assert_eq!(content.top.to_bits(), content.bottom.to_bits());
                if padded {
                    let border = actual.float_border_vertical();
                    assert_eq!(border.bottom - border.top, 5.0);
                }
            }
        }
    }
}

#[test]
fn cfh_core_profile_fences() {
    for count in [5, 6, 7] {
        let nodes = sources(count);
        for index in 0..count - 2 {
            if index == 1 {
                continue;
            }
            let (outer, parent, mut children) = facts(&nodes);
            children[index].height = OwnerHeight::Auto;
            assert!(
                matches!(build(&nodes, outer, parent, &children, limits(count)),
                Err(ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                    source, reason: OwnerProfileReason::HeightKind,
                })) if source == index + 2)
            );
        }
        let (outer, parent, mut children) = facts(&nodes);
        children[1].height = OwnerHeight::Percentage(0.0);
        assert!(matches!(
            build(&nodes, outer, parent, &children, limits(count)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: OwnerProfileReason::HeightKind,
                }
            ))
        ));
    }
    let nodes = sources(5);
    let (outer, parent, mut children) = facts(&nodes);
    children[1].height = OwnerHeight::Auto;
    let result = iced_css_core::formatting::clearance_owner::build_nested_clearance_owner(
        &nodes,
        outer,
        ClearanceOwnerInput {
            parent,
            before: children[0],
            float: children[1],
            after: children[2],
            float_width: 10.0,
        },
    );
    assert!(matches!(
        result,
        Err(ClearanceOwnerError::Unsupported {
            source: 3,
            reason: OwnerProfileReason::HeightKind,
        })
    ));
}

#[test]
fn cfh_core_exact_extent_and_later_profile_order() {
    for (top, bottom) in [(0.0, 0.0), (-3.0, 3.0), (f32::MAX, -f32::MAX), (-1.0, 0.0)] {
        let nodes = sources(6);
        let (outer, parent, mut children) = facts(&nodes);
        children[1].margin.top = AutoLength::Value(top);
        children[1].margin.bottom = AutoLength::Value(bottom);
        children[1].height = OwnerHeight::Explicit(0.0);
        let expected = build(&nodes, outer, parent, &children, limits(6));
        children[1].height = OwnerHeight::Auto;
        let actual = build(&nodes, outer, parent, &children, limits(6));
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }
    let nodes = sources(6);
    let (outer, parent, mut children) = facts(&nodes);
    children[1].height = OwnerHeight::Auto;
    children[1].margin.bottom = AutoLength::Value(-1.0);
    children[3].height = OwnerHeight::Auto;
    let mut short = limits(6);
    short.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        build_width(&nodes, outer, parent, &children, -1.0, short),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::Unsupported {
                source: 5,
                reason: OwnerProfileReason::HeightKind,
            }
        ))
    ));
}

#[test]
fn cfh_core_auto_work_thresholds() {
    for (count, work) in [(5, 945), (6, 1107), (7, 1270)] {
        let nodes = sources(count);
        let (outer, parent, mut children) = facts(&nodes);
        children[1].height = OwnerHeight::Auto;
        for child in &mut children[2..] {
            child.clear = ClearSide::None;
        }
        let mut bound = limits(count);
        bound.work = NonZeroUsize::new(work - 1).unwrap();
        assert!(matches!(
            build(&nodes, outer, parent, &children, bound),
            Err(ClearanceSequenceError::Work)
        ));
        bound.work = NonZeroUsize::new(work).unwrap();
        let owner = build(&nodes, outer, parent, &children, bound).unwrap();
        assert_eq!(owner.receipt().work, work);
    }
}

#[test]
fn cfh_core_mandatory_equality_is_preserved() {
    let nodes = sources(6);
    let (outer, parent, mut children) = facts(&nodes);
    for height in [OwnerHeight::Explicit(0.0), OwnerHeight::Auto] {
        children[1].height = height;
        assert!(matches!(
            build(&nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::UnsupportedEquality,
            })
        ));
    }
}

#[test]
fn cfh_core_validation_order() {
    for (height, width, margin, padding, field) in [
        (OwnerHeight::Auto, -1.0, -1.0, 0.0, "float_width"),
        (OwnerHeight::Auto, 10.0, f32::NAN, -1.0, "margin"),
        (
            OwnerHeight::Explicit(f32::NAN),
            10.0,
            f32::NAN,
            -1.0,
            "height",
        ),
        (OwnerHeight::Explicit(-1.0), 10.0, f32::NAN, -1.0, "height"),
    ] {
        let nodes = sources(6);
        let (outer, parent, mut children) = facts(&nodes);
        children[1].height = height;
        children[1].margin.bottom = AutoLength::Value(margin);
        children[1].padding.top = padding;
        assert!(
            matches!(build_width(&nodes, outer, parent, &children, width, limits(6)),
            Err(ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 3, field: actual,
            })) if actual == field)
        );
    }
}

#[test]
fn cfh_core_auto_content_and_shape_fences() {
    let nodes = sources(6);
    for generated in [OwnerGenerated::Present, OwnerGenerated::Unresolved] {
        let (outer, parent, mut children) = facts(&nodes);
        children[1].height = OwnerHeight::Auto;
        children[1].generated = generated;
        assert!(matches!(
            build(&nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: OwnerProfileReason::GeneratedContent,
                }
            ))
        ));
    }
    let mut nodes = sources(6);
    let extra = nodes[4].instance;
    nodes[3].children.push(extra);
    let (outer, parent, mut children) = facts(&nodes);
    children[1].height = OwnerHeight::Auto;
    assert!(matches!(
        build(&nodes, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::ChildCount { source: 3 }
        ))
    ));
}

fn snv_assert_core_oracle(
    owner: &iced_css_core::formatting::clearance_sequence::ClearanceSequenceOwner<'_>,
    oracle: &str,
) {
    let (b1, float, tails) = match oracle {
        "A" => (
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 18.0],
            [(18.0, 19.0, Some(-1.0)), (21.0, 22.0, None)],
        ),
        "B" => (
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 18.0],
            [(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
        ),
        "C" => (
            [11.0, 12.0],
            [0.0, 10.0, 12.0, 14.0],
            [(15.0, 16.0, None), (19.0, 20.0, None)],
        ),
        "D" => (
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 18.0],
            [(18.0, 19.0, Some(-1.0)), (22.0, 23.0, None)],
        ),
        "E" => (
            [10.0, 11.0],
            [0.0, 10.0, 11.0, 13.0],
            [(14.0, 15.0, None), (18.0, 19.0, None)],
        ),
        "F" => (
            [10.0, 11.0],
            [0.0, 10.0, 15.0, 17.0],
            [(17.0, 18.0, Some(-1.0)), (21.0, 22.0, None)],
        ),
        "G" => (
            [11.0, 12.0],
            [0.0, 10.0, 11.0, 13.0],
            [(14.0, 15.0, None), (18.0, 19.0, None)],
        ),
        "H" => (
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 18.0],
            [(18.0, 19.0, Some(2.0)), (22.0, 23.0, None)],
        ),
        "I" => (
            [11.0, 12.0],
            [0.0, 10.0, 11.0, 13.0],
            [(13.0, 14.0, Some(2.0)), (17.0, 18.0, None)],
        ),
        "J" => (
            [11.0, 12.0],
            [0.0, 10.0, 16.0, 18.0],
            [(18.0, 19.0, Some(2.0)), (20.0, 21.0, None)],
        ),
        _ => panic!("unknown independently reviewed signed-normal oracle {oracle}"),
    };
    assert_eq!(
        sfm_core_view(owner),
        sfm_core_expected(b1, float, float, &tails),
        "{oracle}"
    );
}
