use iced_css_core::{
    formatting::{
        block::AutoLength,
        clearance::ClearSide,
        clearance_owner::{
            ChildContext, ChildFacts, ClearanceOwnerError, ClearanceOwnerInput, MarginGroupId,
            MarginMember, MarginSide, OuterFacts, OwnerContent, OwnerEdges, OwnerGenerated,
            OwnerHeight, OwnerProfileReason, ParentBfc, ParentFacts,
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
    children[0].margin.bottom = 4.;
    children[1].display.float = Float::Left;
    children[1].display.flow = Flow::Floating;
    children[1].context = ChildContext::FloatBfc;
    children[1].height = OwnerHeight::Explicit(2.);
    children[2].margin.top = 3.;
    children[2].margin.bottom = 3.;
    children[2].clear = ClearSide::Both;
    if children.len() > 3 {
        children[3].margin.top = 2.;
        children[3].margin.bottom = 4.;
        children[3].clear = ClearSide::Both;
    }
    if children.len() > 4 {
        children[4].height = OwnerHeight::Explicit(2.);
        children[4].margin.top = 1.;
        children[4].margin.bottom = 2.;
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
                value: children[previous_child].margin.bottom,
            },
            MarginMember {
                source_index: source,
                side: MarginSide::Top,
                value: children[source - 2].margin.top,
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
        children[source - 2].margin.top = -1.0;
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: actual_source,
                    reason: OwnerProfileReason::NegativeMargin,
                }
            )) if actual_source == source
        ));

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
    for (value, reason) in [
        (AutoLength::Value(1.0), OwnerProfileReason::FloatMargin),
        (AutoLength::Value(-1.0), OwnerProfileReason::NegativeMargin),
    ] {
        let (outer, parent, mut children) = facts(&source_nodes);
        children[1].margin.left = value;
        assert!(matches!(
            build(&source_nodes, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: actual_reason,
                }
            )) if actual_reason == reason
        ));
    }
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
                0 => float.margin.top = bad,
                1 => float.margin.right = AutoLength::Value(bad),
                2 => float.margin.bottom = bad,
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

    for (value, reason) in [
        (-1.0, OwnerProfileReason::NegativeMargin),
        (1.0, OwnerProfileReason::FloatMargin),
    ] {
        for edge in 0..4 {
            let (outer, parent, mut children) = facts(&source_nodes);
            let float = &mut children[1];
            if edge != 3 {
                float.margin.left = AutoLength::Auto;
            } else {
                float.margin.right = AutoLength::Auto;
            }
            match edge {
                0 => float.margin.top = value,
                1 => float.margin.right = AutoLength::Value(value),
                2 => float.margin.bottom = value,
                _ => float.margin.left = AutoLength::Value(value),
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

    for (fault, reason) in [
        (0, OwnerProfileReason::ChildDecoration),
        (1, OwnerProfileReason::ClearOnBeforeOrFloat),
    ] {
        let (outer, parent, mut children) = facts(&source_nodes);
        children[1].margin.left = AutoLength::Auto;
        if fault == 0 {
            children[1].border.left = 1.0;
        } else {
            children[1].clear = ClearSide::Left;
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
            0 => float.height = OwnerHeight::Auto,
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
                children[source - 2].margin.top = -1.0;
            } else {
                children[source - 2].margin.bottom = -1.0;
            }
            assert!(matches!(
                build(&source_nodes, outer, parent, &children, limits(6)),
                Err(ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                    source: actual_source,
                    reason: OwnerProfileReason::NegativeMargin,
                })) if actual_source == source
            ));
        }
    }

    let sources = sources(6);
    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.top = -1.0;
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

    let (outer, parent, mut children) = facts(&sources);
    children[0].margin.left = AutoLength::Value(f32::NAN);
    children[0].clear = ClearSide::Left;
    assert!(matches!(
        build(&sources, outer, parent, &children, limits(6)),
        Err(ClearanceSequenceError::Profile(
            ClearanceOwnerError::InvalidNumber {
                source: 2,
                field: "margin",
            }
        ))
    ));

    for (value, reason) in [
        (1.0, OwnerProfileReason::FloatMargin),
        (-1.0, OwnerProfileReason::NegativeMargin),
    ] {
        let (outer, parent, mut children) = facts(&sources);
        children[1].margin.left = AutoLength::Value(value);
        assert!(matches!(
            build(&sources, outer, parent, &children, limits(6)),
            Err(ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                source: 3,
                reason: actual_reason,
            })) if actual_reason == reason
        ));
    }
}

#[test]
fn complete_profile_validation_precedes_bytes_and_numeric_publication() {
    let sources = sources(6);
    let (outer, mut parent, mut children) = facts(&sources);
    parent.content_top = f32::MAX;
    children[0].margin.top = f32::MAX;
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

    for (count, work) in [(5, 197), (6, 259), (7, 322)] {
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
        limit.work = NonZeroUsize::new(work).unwrap();
        let owner = build(&sources, outer, parent, &children, limit).unwrap();
        let child_count = count - 2;
        let tail_count = count - 4;
        assert_eq!(owner.receipt().nodes, count);
        assert_eq!(owner.receipt().tail, tail_count);
        assert_eq!(owner.receipt().work, work);
        assert_eq!(
            work,
            32 + 8 * count + count * (count - 1) / 2 + 16 * tail_count + 33 * child_count
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
    children[0].margin.top = f32::MAX;
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
    children[0].margin.bottom = f32::MAX;
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
    children[3].margin.top = 0.;
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
    short.work = NonZeroUsize::new(258).unwrap();
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
    children[3].margin.top = f32::NAN;
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
    children[3].padding.left = 1.0;
    let mut one_short = limits(6);
    one_short.work = NonZeroUsize::new(258).unwrap();
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
