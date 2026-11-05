//! Bounded dynamic identity and membership storage for a complete sibling capture.
//! This stage establishes source topology only; it does not establish CSS roles.

use super::{
    clearance_profile::ProjectionView, clearance_source_projection::ClearanceProjectionError,
};
use crate::{
    animation::engine::EngineError,
    capture::{ConstructionNode, ContentCapability, ManagedContentFamily},
    host::source_publication::{InitialSourceHost, InitialSourceSnapshot},
};
use iced_css_core::{
    identity::{InstanceId, SourceRef},
    source::{Metadata, SourceNode},
};
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

/// Resource limits for a dynamic sibling source projection.
#[derive(Debug, Clone, Copy)]
pub struct SiblingProjectionLimits {
    /// Maximum admitted source count.
    pub sources: NonZeroUsize,
    /// Conservative named operation allowance, rather than machine instructions.
    pub work: NonZeroUsize,
    /// Maximum retained payload capacity for nodes and child identity vectors.
    pub retained_bytes: NonZeroUsize,
}

/// Actual retained capacities and prepaid work for a sibling source projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingProjectionReceipt {
    /// Complete projected source count.
    pub nodes: usize,
    /// Structural child-row count after source 3. The single-float layout interprets
    /// these as its normal tail; the cluster layout classifies its own float/tail partition.
    pub tail: usize,
    /// Prepaid named operations.
    pub work: usize,
    /// Actual capacity of the owned node vector.
    pub node_capacity: usize,
    /// Combined actual capacity of the outer and parent child vectors.
    pub child_capacity: usize,
    /// Retained node and child payload capacity in bytes.
    pub retained_bytes: usize,
}

/// Failure to construct a complete dynamic sibling source projection.
#[derive(Debug)]
pub enum SiblingProjectionError {
    /// The captured source count is below five or exceeds the source allowance.
    Sources,
    /// Checked work arithmetic overflowed or the work allowance is insufficient.
    Work,
    /// Requested or actual retained capacity exceeds its byte allowance.
    Bytes,
    /// A node or child-vector reservation failed.
    Allocation(TryReserveError),
    /// Existing source publication, topology, content, or identity validation failed.
    Source(ClearanceProjectionError),
}

impl std::fmt::Display for SiblingProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sibling source projection: {self:?}")
    }
}

impl std::error::Error for SiblingProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Sources | Self::Work | Self::Bytes => None,
        }
    }
}

/// Immutable storage for a complete O/P/[B1,F,tail...] source capture.
///
/// Original observations retain the host lifetime. [`Self::nodes`] borrows this
/// owner and therefore cannot outlive or cross a move of the projection.
pub struct SiblingSourceProjection<'h> {
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    nodes: Vec<SourceNode>,
    receipt: SiblingProjectionReceipt,
}

impl std::fmt::Debug for SiblingSourceProjection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiblingSourceProjection")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}

impl<'h> SiblingSourceProjection<'h> {
    /// Exact initial source host retained by this projection.
    pub fn host(&self) -> &'h InitialSourceHost {
        <Self as ProjectionView>::host(self)
    }

    /// Exact published snapshot retained by this projection.
    pub fn snapshot(&self) -> &'h InitialSourceSnapshot {
        <Self as ProjectionView>::snapshot(self)
    }

    /// Geometry-facing topology with deliberately empty metadata.
    pub fn nodes(&self) -> &[SourceNode] {
        <Self as ProjectionView>::nodes(self)
    }

    /// Maps projected O/P/children order to original postorder.
    pub fn original_index(&self, projected: usize) -> Option<usize> {
        <Self as ProjectionView>::original_index(self, projected)
    }

    /// Maps original postorder to projected O/P/children order.
    pub fn projected_index(&self, original: usize) -> Option<usize> {
        <Self as ProjectionView>::projected_index(self, original)
    }

    /// Exact original captured node for a projected index.
    pub fn original_node(&self, projected: usize) -> Option<&'h ConstructionNode> {
        <Self as ProjectionView>::original_node(self, projected)
    }

    /// Exact original live source token for a projected index.
    pub fn token(&self, projected: usize) -> Option<&'h SourceRef> {
        <Self as ProjectionView>::token(self, projected)
    }

    /// Exact original selector metadata for a projected index.
    pub fn metadata(&self, projected: usize) -> Option<&'h Metadata> {
        <Self as ProjectionView>::metadata(self, projected)
    }

    /// Actual retained capacities and prepaid work.
    pub fn receipt(&self) -> SiblingProjectionReceipt {
        self.receipt
    }
}

impl<'h> ProjectionView<'h> for SiblingSourceProjection<'h> {
    fn host(&self) -> &'h InitialSourceHost {
        self.host
    }
    fn snapshot(&self) -> &'h InitialSourceSnapshot {
        self.snapshot
    }
    fn nodes(&self) -> &[SourceNode] {
        &self.nodes
    }
    fn original_index(&self, projected: usize) -> Option<usize> {
        original_index(self.nodes.len(), projected)
    }
    fn projected_index(&self, original: usize) -> Option<usize> {
        projected_index(self.nodes.len(), original)
    }
    fn original_node(&self, projected: usize) -> Option<&'h ConstructionNode> {
        self.snapshot
            .construction()
            .nodes()
            .get(original_index(self.nodes.len(), projected)?)
    }
    fn token(&self, projected: usize) -> Option<&'h SourceRef> {
        self.snapshot
            .token(original_index(self.nodes.len(), projected)?)
    }
}

fn original_index(count: usize, projected: usize) -> Option<usize> {
    if projected >= count {
        return None;
    }
    match projected {
        0 => count.checked_sub(1),
        1 => count.checked_sub(2),
        _ => projected.checked_sub(2),
    }
}

fn projected_index(count: usize, original: usize) -> Option<usize> {
    if original >= count {
        return None;
    }
    if original < count.checked_sub(2)? {
        original.checked_add(2)
    } else if original == count.checked_sub(2)? {
        Some(1)
    } else {
        Some(0)
    }
}

/// Builds a complete dynamic O/P/[B1,F,tail...] source projection.
///
/// Work is prepaid as `1 + 6N + N^2 + N(N-1)/2` before proportional scans or
/// allocation. The `N^2` identity allowance conservatively dominates the N
/// bounded registry lookups; it does not describe their binary-search cost.
///
/// # Errors
///
/// Returns [`SiblingProjectionError::Sources`] for an unsupported source count,
/// [`SiblingProjectionError::Work`] before proportional work when prepayment
/// fails, and preserves source/allocation causes for later validation failures.
pub fn prepare_sibling_source_projection<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    limits: SiblingProjectionLimits,
) -> Result<SiblingSourceProjection<'h>, SiblingProjectionError> {
    prepare_with_reserve(
        host,
        snapshot,
        limits,
        |nodes, requested| nodes.try_reserve_exact(requested),
        |children, requested| children.try_reserve_exact(requested),
    )
}

fn prepare_with_reserve<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    limits: SiblingProjectionLimits,
    mut reserve_nodes: impl FnMut(&mut Vec<SourceNode>, usize) -> Result<(), TryReserveError>,
    mut reserve_children: impl FnMut(&mut Vec<InstanceId>, usize) -> Result<(), TryReserveError>,
) -> Result<SiblingSourceProjection<'h>, SiblingProjectionError> {
    prepare_with_services(
        host,
        snapshot,
        limits,
        &mut reserve_nodes,
        &mut reserve_children,
        |original| host.engine().instance(&snapshot.tokens()[original]),
    )
}

fn prepare_with_services<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    limits: SiblingProjectionLimits,
    mut reserve_nodes: impl FnMut(&mut Vec<SourceNode>, usize) -> Result<(), TryReserveError>,
    mut reserve_children: impl FnMut(&mut Vec<InstanceId>, usize) -> Result<(), TryReserveError>,
    mut identity: impl FnMut(usize) -> Result<InstanceId, EngineError>,
) -> Result<SiblingSourceProjection<'h>, SiblingProjectionError> {
    use ClearanceProjectionError as C;
    use SiblingProjectionError as E;

    let remaining = limits.work.get().checked_sub(1).ok_or(E::Work)?;
    let published = host.snapshot().ok_or(E::Source(C::UnpublishedHost))?;
    if !std::ptr::eq(published, snapshot) {
        return Err(E::Source(C::SnapshotMismatch));
    }

    let source = snapshot.construction();
    let count = source.nodes().len();
    if count < 5 || count > limits.sources.get() {
        return Err(E::Sources);
    }
    if snapshot.tokens().len() != count {
        return Err(E::Source(C::TokenCount));
    }
    if host.engine().live_source_count() != count {
        return Err(E::Source(C::LiveCount));
    }
    if source.root() != count - 1 {
        return Err(E::Source(C::Root));
    }

    let work = projection_work(count).ok_or(E::Work)?;
    remaining.checked_sub(work - 1).ok_or(E::Work)?;

    let parent_original = count - 2;
    let outer_original = count - 1;
    for (original, node) in source.nodes().iter().enumerate() {
        let parent = if original < parent_original {
            Some(parent_original)
        } else if original == parent_original {
            Some(outer_original)
        } else {
            None
        };
        if node.parent() != parent {
            return Err(E::Source(C::Parent { original }));
        }
        let (role, position) = if original == outer_original {
            (None, None)
        } else {
            (
                Some("children"),
                Some(if original < parent_original {
                    original
                } else {
                    0
                }),
            )
        };
        if node.role() != role || node.role_position() != position {
            return Err(E::Source(C::Role { original }));
        }
    }
    for (original, node) in source.nodes().iter().enumerate() {
        if !matches!(
            node.content_capability(),
            ContentCapability::Managed(ManagedContentFamily::Row | ManagedContentFamily::Column)
        ) {
            return Err(E::Source(C::Content { original }));
        }
    }

    let requested_child_capacity = count.checked_sub(1).ok_or(E::Bytes)?;
    checked_retained_bytes(count, requested_child_capacity, limits.retained_bytes)?;

    let mut nodes = Vec::new();
    reserve_nodes(&mut nodes, count).map_err(E::Allocation)?;
    checked_retained_bytes(
        nodes.capacity(),
        requested_child_capacity,
        limits.retained_bytes,
    )?;

    for original in 0..count {
        let instance =
            identity(original).map_err(|error| E::Source(C::Identity { original, error }))?;
        nodes.push(SourceNode {
            instance,
            parent: None,
            children: Vec::new(),
            metadata: Metadata::default(),
        });
    }
    for first in 0..count {
        for second in first + 1..count {
            if nodes[first].instance == nodes[second].instance {
                return Err(E::Source(C::DuplicateInstance { first, second }));
            }
        }
    }

    nodes.rotate_right(2);
    nodes.swap(0, 1);

    reserve_children(&mut nodes[0].children, 1).map_err(E::Allocation)?;
    let remaining_parent_floor = count.checked_sub(2).ok_or(E::Bytes)?;
    checked_retained_bytes(
        nodes.capacity(),
        nodes[0]
            .children
            .capacity()
            .checked_add(remaining_parent_floor)
            .ok_or(E::Bytes)?,
        limits.retained_bytes,
    )?;

    reserve_children(&mut nodes[1].children, remaining_parent_floor).map_err(E::Allocation)?;
    let child_capacity = nodes[0]
        .children
        .capacity()
        .checked_add(nodes[1].children.capacity())
        .ok_or(E::Bytes)?;
    let retained_bytes =
        checked_retained_bytes(nodes.capacity(), child_capacity, limits.retained_bytes)?;

    let outer_instance = nodes[0].instance;
    let parent_instance = nodes[1].instance;
    nodes[0].children.push(parent_instance);
    nodes[1].parent = Some(outer_instance);
    for projected in 2..count {
        let child_instance = nodes[projected].instance;
        nodes[projected].parent = Some(parent_instance);
        nodes[1].children.push(child_instance);
    }

    let receipt = SiblingProjectionReceipt {
        nodes: count,
        tail: count - 4,
        work,
        node_capacity: nodes.capacity(),
        child_capacity,
        retained_bytes,
    };
    Ok(SiblingSourceProjection {
        host,
        snapshot,
        nodes,
        receipt,
    })
}

fn projection_work(count: usize) -> Option<usize> {
    let linear = count.checked_mul(6)?;
    let identity = count.checked_mul(count)?;
    let duplicate_pairs = count.checked_mul(count.checked_sub(1)?)?.checked_div(2)?;
    1usize
        .checked_add(linear)?
        .checked_add(identity)?
        .checked_add(duplicate_pairs)
}

fn checked_retained_bytes(
    node_capacity: usize,
    child_capacity: usize,
    limit: NonZeroUsize,
) -> Result<usize, SiblingProjectionError> {
    let bytes = node_capacity
        .checked_mul(size_of::<SourceNode>())
        .and_then(|value| {
            child_capacity
                .checked_mul(size_of::<InstanceId>())
                .and_then(|children| value.checked_add(children))
        })
        .filter(|bytes| *bytes <= isize::MAX as usize && *bytes <= limit.get())
        .ok_or(SiblingProjectionError::Bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::engine::ProgressLimits,
        capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
        host::source_publication::PublicationLimits,
    };
    use iced_css_core::{identity::SourceChangeLimits, source::selector_tree::SelectorTreeLimits};
    use std::num::NonZeroU64;

    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    fn host() -> InitialSourceHost {
        let limits = CaptureLimits {
            nodes: nz(100),
            depth: nz(100),
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        };
        type Element = CapturedElement<'static, (), iced::Theme, ()>;
        let leaf = || {
            adapters::row_from_vec(Vec::<Element>::new(), limits)
                .unwrap()
                .erase()
        };
        let parent = adapters::column_from_vec(vec![leaf(), leaf(), leaf()], limits)
            .unwrap()
            .erase();
        let outer = adapters::column_from_vec(vec![parent], limits).unwrap();
        let (_, construction) = outer.into_parts();
        let mut host = InitialSourceHost::new(
            NonZeroU64::new(100).unwrap(),
            ProgressLimits {
                addresses: NonZeroU64::new(100).unwrap(),
                bindings: nz(100),
                tracks: nz(100),
                retained_bytes: nz(1_000_000),
                work: nz(1_000_000),
            },
        );
        host.prepare(
            construction,
            vec![ConstructionState::default(); 5],
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

    fn limits() -> SiblingProjectionLimits {
        SiblingProjectionLimits {
            sources: nz(100),
            work: nz(66),
            retained_bytes: nz(1_000_000),
        }
    }

    #[test]
    fn duplicate_identity_seam_rejects_the_original_pair_without_repair() {
        let host = host();
        let snapshot = host.snapshot().unwrap();
        let ids = snapshot
            .tokens()
            .iter()
            .map(|token| host.engine().instance(token).unwrap())
            .collect::<Vec<_>>();

        let result = prepare_with_services(
            &host,
            snapshot,
            limits(),
            |nodes, requested| nodes.try_reserve_exact(requested),
            |children, requested| children.try_reserve_exact(requested),
            |original| Ok(if original == 1 { ids[0] } else { ids[original] }),
        );

        assert!(matches!(
            result,
            Err(SiblingProjectionError::Source(
                ClearanceProjectionError::DuplicateInstance {
                    first: 0,
                    second: 1,
                }
            )),
        ));
    }

    #[test]
    fn identity_failure_keeps_the_actual_engine_error_chain() {
        let primary = host();
        let other = host();
        let snapshot = primary.snapshot().unwrap();
        let error = prepare_with_services(
            &primary,
            snapshot,
            limits(),
            |nodes, requested| nodes.try_reserve_exact(requested),
            |children, requested| children.try_reserve_exact(requested),
            |original| {
                let token = if original == 0 {
                    other.snapshot().unwrap().token(0).unwrap()
                } else {
                    snapshot.token(original).unwrap()
                };
                primary.engine().instance(token)
            },
        )
        .unwrap_err();

        let source = std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<ClearanceProjectionError>()
            .unwrap();
        assert!(matches!(
            source,
            ClearanceProjectionError::Identity { original: 0, .. }
        ));
        assert!(
            std::error::Error::source(source)
                .unwrap()
                .downcast_ref::<EngineError>()
                .is_some()
        );
    }

    #[test]
    fn work_prepayment_precedes_identity_and_every_reserve() {
        let host = host();
        let snapshot = host.snapshot().unwrap();
        for work in [1, 65] {
            let mut limited = limits();
            limited.work = nz(work);
            let result = prepare_with_services(
                &host,
                snapshot,
                limited,
                |_, _| panic!("node reserve reached before work prepayment"),
                |_, _| panic!("child reserve reached before work prepayment"),
                |_| panic!("identity lookup reached before work prepayment"),
            );
            assert!(matches!(result, Err(SiblingProjectionError::Work)));
        }
    }

    #[test]
    fn genuine_node_and_child_reserve_errors_keep_their_sources() {
        let host = host();
        let snapshot = host.snapshot().unwrap();
        let node_error = prepare_with_reserve(
            &host,
            snapshot,
            limits(),
            |nodes, _| nodes.try_reserve_exact(usize::MAX),
            |children, requested| children.try_reserve_exact(requested),
        )
        .unwrap_err();
        assert!(
            std::error::Error::source(&node_error)
                .unwrap()
                .downcast_ref::<TryReserveError>()
                .is_some()
        );

        for fail_at in [1, 2] {
            let mut calls = 0;
            let error = prepare_with_reserve(
                &host,
                snapshot,
                limits(),
                |nodes, requested| nodes.try_reserve_exact(requested),
                |children, requested| {
                    calls += 1;
                    if calls == fail_at {
                        children.try_reserve_exact(usize::MAX)
                    } else {
                        children.try_reserve_exact(requested)
                    }
                },
            )
            .unwrap_err();
            assert_eq!(calls, fail_at);
            assert!(
                std::error::Error::source(&error)
                    .unwrap()
                    .downcast_ref::<TryReserveError>()
                    .is_some()
            );
        }
    }

    #[test]
    fn requested_and_actual_capacity_floors_stop_later_reserves() {
        let host = host();
        let snapshot = host.snapshot().unwrap();
        let floor = 5 * size_of::<SourceNode>() + 4 * size_of::<InstanceId>();

        let mut limited = limits();
        limited.retained_bytes = nz(floor - 1);
        let result = prepare_with_reserve(
            &host,
            snapshot,
            limited,
            |_, _| panic!("node reserve reached before requested floor check"),
            |_, _| panic!("child reserve reached before requested floor check"),
        );
        assert!(matches!(result, Err(SiblingProjectionError::Bytes)));

        limited.retained_bytes = nz(floor);
        let result = prepare_with_reserve(
            &host,
            snapshot,
            limited,
            |nodes, requested| nodes.try_reserve_exact(requested + 1),
            |_, _| panic!("child reserve reached after excessive node capacity"),
        );
        assert!(matches!(result, Err(SiblingProjectionError::Bytes)));

        let mut child_calls = 0;
        let result = prepare_with_reserve(
            &host,
            snapshot,
            limits(),
            |nodes, requested| nodes.try_reserve_exact(requested),
            |children, _| {
                child_calls += 1;
                assert_eq!(child_calls, 1, "parent reserve must not be attempted");
                children.try_reserve_exact(200_000)
            },
        );
        assert!(matches!(result, Err(SiblingProjectionError::Bytes)));
        assert_eq!(child_calls, 1);

        child_calls = 0;
        let result = prepare_with_reserve(
            &host,
            snapshot,
            limits(),
            |nodes, requested| nodes.try_reserve_exact(requested),
            |children, requested| {
                child_calls += 1;
                if child_calls == 2 {
                    children.try_reserve_exact(200_000)
                } else {
                    children.try_reserve_exact(requested)
                }
            },
        );
        assert!(matches!(result, Err(SiblingProjectionError::Bytes)));
        assert_eq!(child_calls, 2);
    }

    #[test]
    fn successful_spare_capacities_are_charged_from_the_same_projection() {
        let host = host();
        let projection = prepare_with_reserve(
            &host,
            host.snapshot().unwrap(),
            limits(),
            |nodes, requested| nodes.try_reserve_exact(requested + 1),
            |children, requested| children.try_reserve_exact(requested + 1),
        )
        .unwrap();

        let receipt = projection.receipt();
        assert_eq!(receipt.node_capacity, projection.nodes.capacity());
        assert_eq!(
            receipt.child_capacity,
            projection.nodes[0].children.capacity() + projection.nodes[1].children.capacity(),
        );
        assert_eq!(
            receipt.retained_bytes,
            receipt.node_capacity * size_of::<SourceNode>()
                + receipt.child_capacity * size_of::<InstanceId>(),
        );
        assert_eq!(projection.nodes[0].children.len(), 1);
        assert_eq!(projection.nodes[1].children.len(), 3);
    }
}
