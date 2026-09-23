use iced_css::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
    evaluation::clearance_source_projection::{
        ClearanceProjectionError, ClearanceProjectionLimits, prepare_clearance_source_projection,
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    identity::{InstanceId, SourceChangeLimits},
    selectors::LinkState,
    source::{AttributeLimits, selector_tree::SelectorTreeLimits},
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(1000000),
        work: nz(1000000),
    }
}
fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: nz(100),
        retained_bytes: nz(10000),
        work: nz(10000),
    }
}
type Element = CapturedElement<'static, (), iced::Theme, ()>;
fn leaf() -> Element {
    adapters::row_from_vec(Vec::<Element>::new(), capture_limits())
        .unwrap()
        .erase()
}
fn tree() -> Element {
    let children = ["first", "second", "third"].map(|id| {
        leaf()
            .style_id(id, attrs())
            .unwrap()
            .style_classes(&["original"], attrs())
            .unwrap()
            .source_key(id.as_bytes(), capture_limits())
            .unwrap()
    });
    adapters::column_from_vec(Vec::from(children), capture_limits())
        .unwrap()
        .style_id("parent", attrs())
        .unwrap()
        .erase()
}
fn empty_host() -> InitialSourceHost {
    InitialSourceHost::new(
        NonZeroU64::new(100).unwrap(),
        ProgressLimits {
            addresses: NonZeroU64::new(100).unwrap(),
            bindings: nz(100),
            tracks: nz(100),
            retained_bytes: nz(1000000),
            work: nz(1000000),
        },
    )
}
fn publish(
    element: Element,
) -> (
    InitialSourceHost,
    iced::Element<'static, (), iced::Theme, ()>,
) {
    let (native, construction) = element.into_parts();
    let states = vec![
        ConstructionState {
            link: LinkState::NotLink,
            hovered: false,
            active: false,
            focused: false
        };
        construction.nodes().len()
    ];
    let mut host = empty_host();
    host.prepare(
        construction,
        states,
        None,
        PublicationLimits {
            nodes: nz(100),
            snapshot_bytes: nz(1000000),
            work: nz(1000000),
            selectors: SelectorTreeLimits {
                entries: nz(100),
                retained_bytes: nz(1000000),
                work: nz(1000000),
            },
            identities: SourceChangeLimits {
                entries: nz(100),
                retained_bytes: nz(1000000),
                work: nz(1000000),
            },
        },
    )
    .unwrap()
    .commit();
    (host, native)
}
fn limits() -> ClearanceProjectionLimits {
    ClearanceProjectionLimits {
        retained_bytes: nz(10000),
        work: nz(27),
    }
}
#[test]
fn actual_capture_preserves_identity_membership_and_original_metadata() {
    let (host, _native) = publish(tree());
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_clearance_source_projection(&host, snapshot, limits()).unwrap();
    assert!(std::ptr::eq(projection.host(), &host));
    assert!(std::ptr::eq(projection.snapshot(), snapshot));
    for (projected, original) in [3, 0, 1, 2].into_iter().enumerate() {
        assert_eq!(projection.original_index(projected), Some(original));
        assert_eq!(projection.projected_index(original), Some(projected));
        assert!(std::ptr::eq(
            projection.original_node(projected).unwrap(),
            &snapshot.construction().nodes()[original]
        ));
        assert!(std::ptr::eq(
            projection.token(projected).unwrap(),
            snapshot.token(original).unwrap()
        ));
        assert!(std::ptr::eq(
            projection.metadata(projected).unwrap(),
            snapshot.construction().nodes()[original].metadata()
        ));
        let node = &projection.nodes()[projected];
        assert_eq!(
            node.instance,
            host.engine()
                .instance(snapshot.token(original).unwrap())
                .unwrap()
        );
        assert_eq!(node.metadata.retained_bytes(), 0);
        assert_eq!(
            projection.metadata(projected).unwrap().css_id(),
            Some(["first", "second", "third", "parent"][original])
        );
        if projected > 0 {
            assert!(
                projection
                    .metadata(projected)
                    .unwrap()
                    .has_class("original")
            );
            assert_eq!(
                projection.original_node(projected).unwrap().source_key(),
                Some(["first", "second", "third"][original].as_bytes())
            );
            assert_eq!(node.parent, Some(projection.nodes()[0].instance));
            assert!(node.children.is_empty());
        }
    }
    assert_eq!(projection.nodes()[0].parent, None);
    assert_eq!(
        projection.nodes()[0].children,
        projection.nodes()[1..]
            .iter()
            .map(|n| n.instance)
            .collect::<Vec<_>>()
    );
    for index in [4, usize::MAX] {
        assert_eq!(projection.original_index(index), None);
        assert_eq!(projection.projected_index(index), None);
        assert!(projection.original_node(index).is_none());
        assert!(projection.token(index).is_none());
        assert!(projection.metadata(index).is_none());
    }
    let receipt = projection.receipt();
    assert_eq!(receipt.nodes, 4);
    assert_eq!(receipt.work, 27);
    assert_eq!(
        receipt.child_capacity,
        projection.nodes()[0].children.capacity()
    );
    assert_eq!(receipt.child_element_bytes, size_of::<InstanceId>());
    assert_eq!(
        receipt.retained_bytes,
        receipt.child_capacity * size_of::<InstanceId>()
    );
}
#[test]
fn exact_host_join_precedes_traversal() {
    let (a, _native_a) = publish(tree());
    let (b, _native_b) = publish(tree());
    assert_eq!(
        a.engine()
            .instance(a.snapshot().unwrap().token(0).unwrap())
            .unwrap(),
        b.engine()
            .instance(b.snapshot().unwrap().token(0).unwrap())
            .unwrap()
    );
    assert!(matches!(
        prepare_clearance_source_projection(&a, b.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::SnapshotMismatch)
    ));
    assert!(matches!(
        prepare_clearance_source_projection(&empty_host(), b.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::UnpublishedHost)
    ));
}
#[test]
fn budgets_are_exact_and_failed_preparation_does_not_change_host() {
    let (host, _native) = publish(tree());
    let snapshot = host.snapshot().unwrap();
    let mut small = limits();
    small.work = nz(26);
    assert!(matches!(
        prepare_clearance_source_projection(&host, snapshot, small),
        Err(ClearanceProjectionError::Work)
    ));
    small = limits();
    small.retained_bytes = nz(3 * size_of::<InstanceId>() - 1);
    assert!(matches!(
        prepare_clearance_source_projection(&host, snapshot, small),
        Err(ClearanceProjectionError::Bytes)
    ));
    assert!(std::ptr::eq(host.snapshot().unwrap(), snapshot));
    assert_eq!(
        prepare_clearance_source_projection(&host, snapshot, limits())
            .unwrap()
            .receipt()
            .work,
        27
    );
}
#[test]
fn original_metadata_can_outlive_moved_projection() {
    let (host, _native) = publish(tree());
    let metadata = {
        let projection =
            prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()).unwrap();
        let moved = projection;
        assert_eq!(moved.nodes().len(), 4);
        moved.metadata(1).unwrap()
    };
    assert_eq!(metadata.css_id(), Some("first"));
}
#[test]
fn unsupported_actual_capture_profiles_report_specific_errors() {
    let (host, _native) = publish(leaf());
    assert!(matches!(
        prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::NodeCount)
    ));
    let nested = adapters::column_from_vec(
        vec![
            adapters::row_from_vec(vec![leaf()], capture_limits())
                .unwrap()
                .erase(),
            leaf(),
        ],
        capture_limits(),
    )
    .unwrap()
    .erase();
    let (host, _native) = publish(nested);
    assert!(matches!(
        prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::Parent { original: 0 })
    ));
    let stack = adapters::stack_from_vec(vec![leaf(), leaf(), leaf()], capture_limits())
        .unwrap()
        .erase();
    let (host, _native) = publish(stack);
    assert!(matches!(
        prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::Content { original: 3 })
    ));
    let rich = adapters::rich_text::<(), (), iced::Theme, ()>(
        vec![iced::widget::text::Span::new("payload")],
        capture_limits(),
    )
    .unwrap()
    .erase();
    for child in [
        adapters::space(capture_limits()).unwrap().erase(),
        adapters::text("payload", capture_limits()).unwrap().erase(),
        rich,
    ] {
        let element = adapters::column_from_vec(vec![leaf(), child, leaf()], capture_limits())
            .unwrap()
            .erase();
        let (host, _native) = publish(element);
        assert!(matches!(
            prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()),
            Err(ClearanceProjectionError::Content { original: 1 })
        ));
        let mut short = limits();
        short.work = nz(26);
        assert!(matches!(
            prepare_clearance_source_projection(&host, host.snapshot().unwrap(), short),
            Err(ClearanceProjectionError::Work)
        ));
    }
    let container = adapters::container(
        adapters::row_from_vec(vec![leaf(), leaf()], capture_limits())
            .unwrap()
            .erase(),
        capture_limits(),
    )
    .unwrap()
    .erase();
    let (host, _native) = publish(container);
    assert!(matches!(
        prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(ClearanceProjectionError::Parent { original: 0 })
    ));
}

#[test]
fn row_parent_and_empty_column_children_are_admitted() {
    let empty_column = || {
        adapters::column_from_vec(Vec::<Element>::new(), capture_limits())
            .unwrap()
            .erase()
    };
    let captured = adapters::row_from_vec(
        vec![empty_column(), leaf(), empty_column()],
        capture_limits(),
    )
    .unwrap()
    .erase();
    let (host, _native) = publish(captured);
    let projection =
        prepare_clearance_source_projection(&host, host.snapshot().unwrap(), limits()).unwrap();
    assert_eq!(projection.nodes()[0].children.len(), 3);
    assert_eq!(projection.original_node(0).unwrap().canonical_type(), "row");
    assert_eq!(
        projection.original_node(1).unwrap().canonical_type(),
        "column"
    );
}
