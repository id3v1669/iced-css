use super::*;
use crate::{
    formatting::{
        clearance::ClearSide,
        clearance_owner::{
            ChildContext, ChildFacts, OuterFacts, OwnerContent, OwnerEdges, OwnerGenerated,
            OwnerHeight, ParentBfc, ParentFacts,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        floats::Band,
    },
    identity::IdAllocator,
    source::{Metadata, SourceNode},
};
use std::{
    cell::RefCell,
    collections::TryReserveError,
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};
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

fn allocation_error() -> TryReserveError {
    Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err()
}

fn reserve<T>(
    vector: &mut Vec<T>,
    requested: usize,
    index: usize,
    trace: &RefCell<Vec<usize>>,
    failure: Option<usize>,
    short: Option<usize>,
    extra: Option<usize>,
) -> Result<(), TryReserveError> {
    trace.borrow_mut().push(index);
    if failure == Some(index) {
        return Err(allocation_error());
    }
    if short == Some(index) {
        return Ok(());
    }
    vector.try_reserve_exact(requested + if extra == Some(index) { 37 } else { 0 })
}

#[test]
fn g9a_budget_preflight_and_reservation_failure_atomic() {
    let nodes = sources(2, 1);
    let (outer, parent, children) = facts(&nodes, 2);
    let widths = [40.0, 30.0];
    let input = || ClearanceFloatClusterInput {
        outer,
        parent,
        children: &children,
        float_widths: &widths,
    };
    let trace = RefCell::new(Vec::new());
    for failed in 0..4 {
        trace.borrow_mut().clear();
        let result = build_with_reserves(
            &nodes,
            input(),
            limits(),
            |v, n| reserve(v, n, 0, &trace, Some(failed), None, None),
            |v, n| reserve(v, n, 1, &trace, Some(failed), None, None),
            |v, n| reserve(v, n, 2, &trace, Some(failed), None, None),
            |v, n| reserve(v, n, 3, &trace, Some(failed), None, None),
        );
        let error = result.expect_err("reserve failure cannot publish an owner");
        assert!(matches!(error, ClearanceFloatClusterError::Allocation(_)));
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(*trace.borrow(), (0..=failed).collect::<Vec<_>>());
        let rebuilt = build_clearance_float_cluster(&nodes, input(), limits()).unwrap();
        assert_eq!(rebuilt.floats()[0].placed_float().bottom, 30.0);
        assert_eq!(rebuilt.floats()[1].placed_float().left, 70.0);
    }
    for short in 0..4 {
        trace.borrow_mut().clear();
        let result = build_with_reserves(
            &nodes,
            input(),
            limits(),
            |v, n| reserve(v, n, 0, &trace, None, Some(short), None),
            |v, n| reserve(v, n, 1, &trace, None, Some(short), None),
            |v, n| reserve(v, n, 2, &trace, None, Some(short), None),
            |v, n| reserve(v, n, 3, &trace, None, Some(short), None),
        );
        assert!(match result.err().unwrap() {
            ClearanceFloatClusterError::Bytes => short < 3,
            ClearanceFloatClusterError::ScratchBytes => short == 3,
            _ => false,
        });
        assert_eq!(*trace.borrow(), (0..=short).collect::<Vec<_>>());
    }
    // Measure actual baseline capacities; do not assume try_reserve_exact returns
    // precisely the requested capacity on every allocator.
    let capacities = RefCell::new(Vec::new());
    let control = build_with_reserves(
        &nodes,
        input(),
        limits(),
        |v, n| {
            v.try_reserve_exact(n)?;
            capacities.borrow_mut().push(v.capacity());
            Ok(())
        },
        |v, n| {
            v.try_reserve_exact(n)?;
            capacities.borrow_mut().push(v.capacity());
            Ok(())
        },
        |v, n| {
            v.try_reserve_exact(n)?;
            capacities.borrow_mut().push(v.capacity());
            Ok(())
        },
        |v, n| {
            v.try_reserve_exact(n)?;
            capacities.borrow_mut().push(v.capacity());
            Ok(())
        },
    )
    .unwrap();
    let caps = capacities.borrow().clone();
    let retained = caps[0] * size_of::<FloatClusterFloatPlacement>()
        + caps[1] * size_of::<SequenceNormalPlacement>()
        + caps[2] * size_of::<MarginMember>();
    let scratch = caps[3] * size_of::<PlacedFloat>();
    assert_eq!(control.receipt().retained_bytes, retained);
    assert_eq!(control.receipt().scratch_bytes, scratch);
    for extra in 0..4 {
        trace.borrow_mut().clear();
        let mut bounded = limits();
        bounded.retained_bytes = nz(retained);
        bounded.scratch_bytes = nz(scratch);
        let extra_seen = RefCell::new(None);
        macro_rules! callback {
            ($index:expr) => {
                |v: &mut Vec<_>, n| {
                    trace.borrow_mut().push($index);
                    v.try_reserve_exact(if extra == $index {
                        caps[$index] + 37
                    } else {
                        n
                    })?;
                    if extra == $index {
                        *extra_seen.borrow_mut() = Some(v.capacity());
                    }
                    Ok(())
                }
            };
        }
        let result = build_with_reserves(
            &nodes,
            input(),
            bounded,
            callback!(0),
            callback!(1),
            callback!(2),
            callback!(3),
        );
        assert!(
            extra_seen
                .borrow()
                .is_some_and(|actual| actual > caps[extra])
        );
        assert!(match result.err().unwrap() {
            ClearanceFloatClusterError::Bytes => extra < 3,
            ClearanceFloatClusterError::ScratchBytes => extra == 3,
            _ => false,
        });
        assert_eq!(*trace.borrow(), (0..=extra).collect::<Vec<_>>());
    }
    for which in 0..5 {
        trace.borrow_mut().clear();
        let mut bounded = limits();
        match which {
            0 => bounded.sources = nz(1),
            1 => bounded.floats = nz(1),
            2 => bounded.work = nz(1),
            3 => bounded.retained_bytes = nz(1),
            _ => bounded.scratch_bytes = nz(1),
        }
        let result = build_with_reserves(
            &nodes,
            input(),
            bounded,
            |v, n| reserve(v, n, 0, &trace, None, None, None),
            |v, n| reserve(v, n, 1, &trace, None, None, None),
            |v, n| reserve(v, n, 2, &trace, None, None, None),
            |v, n| reserve(v, n, 3, &trace, None, None, None),
        );
        assert!(result.is_err());
        assert!(trace.borrow().is_empty());
    }
    let rebuilt = build_clearance_float_cluster(&nodes, input(), limits()).unwrap();
    assert_eq!(rebuilt.floats().len(), 2);
    assert_eq!(rebuilt.tail().len(), 1);
}
