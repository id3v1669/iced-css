use iced_css::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
    evaluation::sibling_source_projection::{
        SiblingProjectionError, SiblingProjectionLimits, prepare_sibling_source_projection,
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    identity::SourceChangeLimits,
    source::{AttributeLimits, SourceNode, selector_tree::SelectorTreeLimits},
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(1_000_000),
        work: nz(1_000_000),
    }
}

fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: nz(100),
        retained_bytes: nz(10_000),
        work: nz(10_000),
    }
}

type Element = CapturedElement<'static, (), iced::Theme, ()>;

fn leaf(id: &'static str) -> Element {
    adapters::row_from_vec(Vec::<Element>::new(), capture_limits())
        .unwrap()
        .style_id(id, attrs())
        .unwrap()
        .erase()
}

fn captured_element(source_count: usize) -> Element {
    let names = ["b1", "f", "b2", "b3", "b4"];
    let children = names[..source_count - 2]
        .iter()
        .copied()
        .map(leaf)
        .collect();
    let parent = adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .style_id("p", attrs())
        .unwrap()
        .erase();
    adapters::column_from_vec(vec![parent], capture_limits())
        .unwrap()
        .style_id("o", attrs())
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
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        },
    )
}

fn publish(element: Element) -> InitialSourceHost {
    let (_, construction) = element.into_parts();
    let source_count = construction.nodes().len();
    let mut host = empty_host();
    host.prepare(
        construction,
        vec![ConstructionState::default(); source_count],
        None,
        PublicationLimits {
            nodes: nz(100),
            snapshot_bytes: nz(1_000_000),
            work: nz(1_000_000),
            selectors: SelectorTreeLimits {
                entries: nz(100),
                retained_bytes: nz(1_000_000),
                work: nz(1_000_000),
            },
            identities: SourceChangeLimits {
                entries: nz(100),
                retained_bytes: nz(1_000_000),
                work: nz(1_000_000),
            },
        },
    )
    .unwrap()
    .commit();
    host
}

fn captured(source_count: usize) -> InitialSourceHost {
    let element = captured_element(source_count);
    let host = publish(element);
    assert_eq!(
        host.snapshot().unwrap().construction().nodes().len(),
        source_count
    );
    host
}

fn limits() -> SiblingProjectionLimits {
    SiblingProjectionLimits {
        sources: nz(100),
        work: nz(100_000),
        retained_bytes: nz(1_000_000),
    }
}

#[test]
fn real_five_six_and_seven_source_captures_preserve_the_full_graph() {
    for source_count in [5, 6, 7] {
        let host = captured(source_count);
        let snapshot = host.snapshot().unwrap();
        let projection = prepare_sibling_source_projection(&host, snapshot, limits()).unwrap();

        assert!(std::ptr::eq(projection.host(), &host));
        assert!(std::ptr::eq(projection.snapshot(), snapshot));
        assert_eq!(projection.nodes().len(), source_count);
        for projected in 0..source_count {
            let original = match projected {
                0 => source_count - 1,
                1 => source_count - 2,
                _ => projected - 2,
            };
            assert_eq!(projection.original_index(projected), Some(original));
            assert_eq!(projection.projected_index(original), Some(projected));
            assert!(std::ptr::eq(
                projection.original_node(projected).unwrap(),
                &snapshot.construction().nodes()[original],
            ));
            assert!(std::ptr::eq(
                projection.token(projected).unwrap(),
                snapshot.token(original).unwrap(),
            ));
            assert!(std::ptr::eq(
                projection.metadata(projected).unwrap(),
                snapshot.construction().nodes()[original].metadata(),
            ));
            assert_eq!(
                projection.nodes()[projected].instance,
                host.engine()
                    .instance(snapshot.token(original).unwrap())
                    .unwrap(),
            );
        }

        let nodes = projection.nodes();
        assert_eq!(nodes[0].children, vec![nodes[1].instance]);
        assert_eq!(nodes[1].parent, Some(nodes[0].instance));
        assert_eq!(
            nodes[1].children,
            nodes[2..]
                .iter()
                .map(|node| node.instance)
                .collect::<Vec<_>>(),
        );
        for node in &nodes[2..] {
            assert_eq!(node.parent, Some(nodes[1].instance));
            assert!(node.children.is_empty());
        }

        let receipt = projection.receipt();
        assert_eq!(receipt.nodes, source_count);
        assert_eq!(receipt.tail, source_count - 4);
        assert_eq!(receipt.work, [66, 88, 113][source_count - 5]);
        assert!(receipt.node_capacity >= source_count);
        assert_eq!(
            receipt.child_capacity,
            nodes[0].children.capacity() + nodes[1].children.capacity(),
        );
        assert_eq!(
            receipt.retained_bytes,
            receipt.node_capacity * size_of::<SourceNode>()
                + receipt.child_capacity * size_of::<iced_css_core::identity::InstanceId>(),
        );
        for index in [source_count, usize::MAX] {
            assert_eq!(projection.original_index(index), None);
            assert_eq!(projection.projected_index(index), None);
            assert!(projection.original_node(index).is_none());
            assert!(projection.token(index).is_none());
            assert!(projection.metadata(index).is_none());
        }
    }
}

#[test]
fn source_and_work_limits_precede_proportional_validation() {
    let host = captured(6);
    let snapshot = host.snapshot().unwrap();

    let mut limited = limits();
    limited.sources = nz(5);
    limited.work = nz(1);
    assert!(matches!(
        prepare_sibling_source_projection(&host, snapshot, limited),
        Err(SiblingProjectionError::Sources),
    ));

    limited.sources = nz(6);
    assert!(matches!(
        prepare_sibling_source_projection(&host, snapshot, limited),
        Err(SiblingProjectionError::Work),
    ));
    limited.work = nz(87);
    assert!(matches!(
        prepare_sibling_source_projection(&host, snapshot, limited),
        Err(SiblingProjectionError::Work),
    ));
    limited.work = nz(88);
    assert_eq!(
        prepare_sibling_source_projection(&host, snapshot, limited)
            .unwrap()
            .receipt()
            .work,
        88,
    );
}

#[test]
fn requested_retained_floor_is_checked_before_allocation() {
    let host = captured(5);
    let count = 5;
    let floor = count * size_of::<SourceNode>()
        + (count - 1) * size_of::<iced_css_core::identity::InstanceId>();
    let mut limited = limits();
    limited.retained_bytes = nz(floor - 1);

    assert!(matches!(
        prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limited),
        Err(SiblingProjectionError::Bytes),
    ));
}

#[test]
fn publication_and_snapshot_errors_keep_the_legacy_source_cause() {
    use iced_css::evaluation::clearance_source_projection::ClearanceProjectionError;

    let first = captured(5);
    let second = captured(5);
    assert!(matches!(
        prepare_sibling_source_projection(&first, second.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Source(
            ClearanceProjectionError::SnapshotMismatch
        )),
    ));
    assert!(matches!(
        prepare_sibling_source_projection(&empty_host(), second.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Source(
            ClearanceProjectionError::UnpublishedHost
        )),
    ));
}

#[test]
fn malformed_real_capture_reports_parent_then_role_then_content() {
    use iced_css::evaluation::clearance_source_projection::ClearanceProjectionError;

    let parent = captured_element(5);
    let extra_sibling = adapters::column_from_vec(vec![parent, leaf("extra")], capture_limits())
        .unwrap()
        .erase();
    let host = publish(extra_sibling);
    assert!(matches!(
        prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Source(
            ClearanceProjectionError::Parent { original: 0 }
        )),
    ));

    let parent = adapters::column_from_vec(vec![leaf("a"), leaf("b"), leaf("c")], capture_limits())
        .unwrap()
        .erase();
    let wrong_role = adapters::container(parent, capture_limits())
        .unwrap()
        .erase();
    let host = publish(wrong_role);
    assert!(matches!(
        prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Source(
            ClearanceProjectionError::Role { original: 3 }
        )),
    ));

    let unsupported = adapters::stack_from_vec(Vec::<Element>::new(), capture_limits())
        .unwrap()
        .erase();
    let parent =
        adapters::column_from_vec(vec![leaf("a"), unsupported, leaf("c")], capture_limits())
            .unwrap()
            .erase();
    let outer = adapters::column_from_vec(vec![parent], capture_limits())
        .unwrap()
        .erase();
    let host = publish(outer);
    assert!(matches!(
        prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Source(
            ClearanceProjectionError::Content { original: 1 }
        )),
    ));
}

#[test]
fn fewer_than_five_sources_are_not_a_dynamic_sibling_projection() {
    let children = vec![leaf("a"), leaf("b"), leaf("c")];
    let root = adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .erase();
    let host = publish(root);

    assert!(matches!(
        prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limits()),
        Err(SiblingProjectionError::Sources),
    ));
}

#[test]
fn original_metadata_outlives_projection_storage() {
    let host = captured(7);
    let metadata = {
        let projection =
            prepare_sibling_source_projection(&host, host.snapshot().unwrap(), limits()).unwrap();
        let moved = projection;
        assert_eq!(moved.nodes().len(), 7);
        moved.metadata(6).unwrap()
    };

    assert_eq!(metadata.css_id(), Some("b4"));
}
