use iced_css_core::{
    formatting::{
        block::AutoLength,
        clearance::ClearSide,
        clearance_float_cluster::{
            ClearanceFloatClusterError, ClearanceFloatClusterInput, ClearanceFloatClusterLimits,
            ClearanceFloatClusterOwner, build_clearance_float_cluster,
        },
        clearance_owner::{
            ChildContext, ChildFacts, ClearanceOwnerError, OuterFacts, OwnerContent, OwnerEdges,
            OwnerGenerated, OwnerHeight, ParentBfc, ParentFacts,
        },
        clearance_sequence::{
            ClearanceSequenceInput, ClearanceSequenceLimits, build_clearance_sequence,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        floats::Band,
    },
    identity::IdAllocator,
    source::{Metadata, SourceNode},
};
use std::num::{NonZeroU64, NonZeroUsize};

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn sources(k: usize, tail: usize) -> Vec<SourceNode> {
    let mut ids = IdAllocator::new(NonZeroU64::new(3409).unwrap());
    let mut nodes: Vec<_> = (0..k + tail + 3)
        .map(|_| SourceNode {
            instance: ids.allocate().unwrap(),
            parent: None,
            children: vec![],
            metadata: Metadata::default(),
        })
        .collect();
    nodes[0].children = vec![nodes[1].instance];
    nodes[1].parent = Some(nodes[0].instance);
    nodes[1].children = nodes[2..].iter().map(|n| n.instance).collect();
    let parent = nodes[1].instance;
    for node in &mut nodes[2..] {
        node.parent = Some(parent);
    }
    nodes
}
fn edges() -> OwnerEdges {
    OwnerEdges {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
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
fn facts(nodes: &[SourceNode], k: usize) -> (OuterFacts<'_>, ParentFacts<'_>, Vec<ChildFacts<'_>>) {
    let outer = OuterFacts {
        source: &nodes[0],
        display: display(),
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
    };
    let parent = ParentFacts {
        source: &nodes[1],
        display: display(),
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
        bfc: ParentBfc::NewWithEmptyPrefix,
        content_top: 0.0,
        containing: Band {
            left: 0.0,
            right: 100.0,
        },
        border: edges(),
        padding: edges(),
    };
    let mut children: Vec<_> = nodes[2..]
        .iter()
        .map(|source| ChildFacts {
            source,
            display: display(),
            context: ChildContext::OrdinaryNonBfc,
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            height: OwnerHeight::Explicit(1.0),
            margin: edges().into(),
            border: edges(),
            padding: edges(),
            clear: ClearSide::None,
        })
        .collect();
    children[0].height = OwnerHeight::Explicit(10.0);
    for (i, child) in children[1..=k].iter_mut().enumerate() {
        child.display.float = if i == 1 { Float::Right } else { Float::Left };
        child.display.flow = Flow::Floating;
        child.context = ChildContext::FloatBfc;
        child.height = OwnerHeight::Explicit([20.0, 10.0, 5.0][i % 3]);
    }
    (outer, parent, children)
}
fn limits() -> ClearanceFloatClusterLimits {
    ClearanceFloatClusterLimits {
        sources: nz(100),
        floats: nz(16),
        work: nz(10_000_000),
        retained_bytes: nz(1_000_000),
        scratch_bytes: nz(1_000_000),
    }
}
fn build<'s>(
    nodes: &'s [SourceNode],
    outer: OuterFacts<'s>,
    parent: ParentFacts<'s>,
    children: &[ChildFacts<'s>],
    widths: &[f32],
) -> Result<ClearanceFloatClusterOwner<'s>, ClearanceFloatClusterError> {
    build_clearance_float_cluster(
        nodes,
        ClearanceFloatClusterInput {
            outer,
            parent,
            children,
            float_widths: widths,
        },
        limits(),
    )
}
fn float_edges(owner: &ClearanceFloatClusterOwner<'_>) -> Vec<[f32; 4]> {
    owner
        .floats()
        .iter()
        .map(|f| {
            let p = f.placed_float();
            [p.left, p.right, p.top, p.bottom]
        })
        .collect()
}

#[test]
fn g9a_cluster_three_floats_shared_band() {
    let nodes = sources(3, 1);
    let (outer, parent, children) = facts(&nodes, 3);
    let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0, 40.0]).unwrap();
    // Independent exclusion-band oracle: initial residual width30 cannot fit F3.
    assert_eq!(
        float_edges(&owner),
        vec![
            [0.0, 40.0, 10.0, 30.0],
            [70.0, 100.0, 10.0, 20.0],
            [40.0, 80.0, 20.0, 25.0]
        ]
    );
    assert_eq!(
        owner
            .floats()
            .iter()
            .map(|f| f.source_index())
            .collect::<Vec<_>>(),
        vec![3, 4, 5]
    );
    assert_eq!(owner.floats()[2].bands_visited(), 2);
    for (index, node) in nodes.iter().enumerate() {
        assert!(std::ptr::eq(owner.source(index).unwrap(), node));
    }
    let mut mirrored = children.clone();
    for child in &mut mirrored[1..=3] {
        child.display.float = if child.display.float == Float::Left {
            Float::Right
        } else {
            Float::Left
        };
    }
    let mirror = build(&nodes, outer, parent, &mirrored, &[40.0, 30.0, 40.0]).unwrap();
    assert_eq!(
        float_edges(&mirror),
        vec![
            [60.0, 100.0, 10.0, 30.0],
            [0.0, 30.0, 10.0, 20.0],
            [20.0, 60.0, 20.0, 25.0]
        ]
    );
}

#[test]
fn g9a_clear_uses_all_matching_floats() {
    let nodes = sources(2, 1);
    for (side, top) in [
        (ClearSide::Left, 30.0),
        (ClearSide::Right, 20.0),
        (ClearSide::Both, 30.0),
        (ClearSide::None, 10.0),
    ] {
        let (outer, parent, mut children) = facts(&nodes, 2);
        children[3].clear = side;
        let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0]).unwrap();
        assert_eq!(owner.tail()[0].vertical().top, top);
    }
    let (outer, parent, mut children) = facts(&nodes, 2);
    children[3].clear = ClearSide::Left;
    children[3].margin.top = AutoLength::Value(20.0);
    assert!(matches!(
        build(&nodes, outer, parent, &children, &[40.0, 30.0]),
        Err(ClearanceFloatClusterError::Placement {
            source: 5,
            error: ClearanceOwnerError::UnsupportedEquality
        })
    ));
}

#[test]
fn g9a_float_clear_respects_prior_sides() {
    let nodes = sources(2, 1);
    for (side, top) in [
        (ClearSide::Left, 30.0),
        (ClearSide::Both, 30.0),
        (ClearSide::Right, 10.0),
        (ClearSide::None, 10.0),
    ] {
        let (outer, parent, mut children) = facts(&nodes, 2);
        children[2].clear = side;
        let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0]).unwrap();
        assert_eq!(owner.floats()[1].placed_float().top, top);
    }
    // Zero-height preceding float meets the requested top exactly: float clear allows equality.
    let (outer, parent, mut children) = facts(&nodes, 2);
    children[1].height = OwnerHeight::Explicit(0.0);
    children[2].clear = ClearSide::Left;
    let owner = build(&nodes, outer, parent, &children, &[0.0, 30.0]).unwrap();
    assert_eq!(owner.floats()[1].placed_float().top, 10.0);
}

#[test]
fn g9a_oversized_float_descends_without_scaling() {
    let nodes = sources(2, 1);
    let (outer, parent, children) = facts(&nodes, 2);
    let owner = build(&nodes, outer, parent, &children, &[40.0, 140.0]).unwrap();
    assert_eq!(float_edges(&owner)[1], [-40.0, 100.0, 30.0, 40.0]);
    assert_eq!(
        owner.floats()[1].content().right - owner.floats()[1].content().left,
        140.0
    );
}

#[test]
fn g9a_signed_through_cluster_membership() {
    let nodes = sources(2, 2);
    let (outer, parent, mut children) = facts(&nodes, 2);
    children[0].height = OwnerHeight::Explicit(0.0);
    children[0].margin.top = AutoLength::Value(10.0);
    children[0].margin.bottom = AutoLength::Value(-4.0);
    children[3].height = OwnerHeight::Explicit(0.0);
    children[3].margin.top = AutoLength::Value(3.0);
    children[3].margin.bottom = AutoLength::Value(-7.0);
    children[4].margin.top = AutoLength::Value(2.0);
    let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0]).unwrap();
    assert_eq!(owner.tail()[0].hypothetical_top(), 6.0);
    assert_eq!(owner.tail()[1].hypothetical_top(), 3.0);
    let members: Vec<_> = owner
        .tail_views()
        .nth(1)
        .unwrap()
        .hypothetical_group()
        .members()
        .iter()
        .map(|m| m.source_index)
        .collect();
    assert_eq!(members, vec![2, 2, 5, 5, 6]);
}

#[test]
fn g9a_cleared_through_two_scalar_reference() {
    let nodes = sources(2, 2);
    let (outer, parent, mut children) = facts(&nodes, 2);
    children[0].height = OwnerHeight::Explicit(0.0);
    children[0].margin.top = AutoLength::Value(10.0);
    children[0].margin.bottom = AutoLength::Value(-4.0);
    children[3].height = OwnerHeight::Explicit(0.0);
    children[3].margin.top = AutoLength::Value(3.0);
    children[3].margin.bottom = AutoLength::Value(-7.0);
    children[3].clear = ClearSide::Both;
    children[4].margin.top = AutoLength::Value(2.0);
    let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0]).unwrap();
    assert_eq!(owner.tail()[0].vertical().top, 30.0);
    // Actual cleared reference30 minus own top3 plus collapsed(3,-7,2)=-4.
    assert_eq!(owner.tail()[1].vertical().top, 23.0);
}

#[test]
fn g9a_new_bfc_overlap_uses_all_float_ranges() {
    let nodes = sources(2, 1);
    let (outer, parent, mut children) = facts(&nodes, 2);
    children[2].height = OwnerHeight::Explicit(1.0);
    children[3].margin.top = AutoLength::Value(2.0);
    children[3].context = ChildContext::OtherNewBfc;
    assert!(matches!(
        build(&nodes, outer, parent, &children, &[40.0, 30.0]),
        Err(ClearanceFloatClusterError::Placement {
            source: 5,
            error: ClearanceOwnerError::NewBfcFloatContact
        })
    ));
    children[3].clear = ClearSide::Both;
    let owner = build(&nodes, outer, parent, &children, &[40.0, 30.0]).unwrap();
    assert_eq!(owner.tail()[0].vertical().top, 30.0);
}

#[test]
fn g9a_one_float_legacy_bit_parity() {
    let nodes = sources(1, 2);
    for zero in [0.0_f32, -0.0_f32] {
        for decorated in [false, true] {
            let (outer, parent, mut children) = facts(&nodes, 1);
            children[1].margin.top = AutoLength::Value(zero);
            children[1].margin.left = AutoLength::Value(-2.0);
            children[1].margin.right = AutoLength::Value(3.0);
            if decorated {
                children[1].padding.left = 2.0;
                children[1].border.top = 1.0;
            }
            children[2].clear = ClearSide::Both;
            let owner = build(&nodes, outer, parent, &children, &[40.0]).unwrap();
            let legacy = build_clearance_sequence(
                &nodes,
                ClearanceSequenceInput {
                    outer,
                    parent,
                    children: &children,
                    float_width: 40.0,
                },
                ClearanceSequenceLimits {
                    sources: nz(100),
                    work: nz(10_000_000),
                    retained_bytes: nz(1_000_000),
                },
            )
            .unwrap();
            let f = &owner.floats()[0];
            let a = f.placed_float();
            let b = legacy.placed_float();
            assert_eq!(
                [a.left, a.right, a.top, a.bottom].map(f32::to_bits),
                [b.left, b.right, b.top, b.bottom].map(f32::to_bits)
            );
            assert_eq!(f.border(), legacy.float_border());
            assert_eq!(f.border_vertical(), legacy.float_border_vertical());
            assert_eq!(f.content(), legacy.float_content());
            assert_eq!(f.content_vertical(), legacy.float_content_vertical());
            assert_eq!(f.padding(), legacy.float_padding());
            assert_eq!(f.padding_vertical(), legacy.float_padding_vertical());
            assert_eq!(owner.before(), legacy.before());
            assert_eq!(owner.before_content(), legacy.before_content());
            assert_eq!(owner.before_padding(), legacy.before_padding());
            for (a, b) in owner.tail_views().zip(legacy.tail_views()) {
                assert_eq!(a.vertical(), b.vertical());
                assert_eq!(a.content_vertical(), b.content_vertical());
                assert_eq!(a.padding_vertical(), b.padding_vertical());
                assert_eq!(
                    a.hypothetical_top().to_bits(),
                    b.hypothetical_top().to_bits()
                );
                assert_eq!(
                    a.zero_clearance_top().map(f32::to_bits),
                    b.zero_clearance_top().map(f32::to_bits)
                );
                assert_eq!(
                    a.clearance().map(f32::to_bits),
                    b.clearance().map(f32::to_bits)
                );
                assert_eq!(a.hypothetical_group(), b.hypothetical_group());
                assert_eq!(
                    a.actual_groups().collect::<Vec<_>>(),
                    b.actual_groups().collect::<Vec<_>>()
                );
            }
            assert_eq!(owner.terminal_margin_run(), legacy.terminal_run());
        }
    }
}
