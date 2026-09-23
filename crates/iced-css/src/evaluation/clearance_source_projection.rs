//! Bounded identity/membership storage for an entire four-source captured document.
//! No CSS box, generated-content, BFC or geometry facts are established here.
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

#[derive(Debug, Clone, Copy)]
pub struct ClearanceProjectionLimits {
    /// New owned heap payload capacity; excludes inline and predecessor storage.
    pub retained_bytes: NonZeroUsize,
    /// Named facade operations, not machine comparisons. Success consumes 27.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceProjectionReceipt {
    pub nodes: usize,
    pub work: usize,
    pub child_capacity: usize,
    pub child_element_bytes: usize,
    pub retained_bytes: usize,
}
#[derive(Debug)]
pub enum ClearanceProjectionError {
    Work,
    UnpublishedHost,
    SnapshotMismatch,
    NodeCount,
    TokenCount,
    LiveCount,
    Root,
    Parent { original: usize },
    Role { original: usize },
    Content { original: usize },
    Identity { original: usize, error: EngineError },
    DuplicateInstance { first: usize, second: usize },
    Bytes,
    Allocation(TryReserveError),
}
impl std::fmt::Display for ClearanceProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance source projection: {self:?}")
    }
}
impl std::error::Error for ClearanceProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity { error, .. } => Some(error),
            Self::Allocation(error) => Some(error),
            _ => None,
        }
    }
}
const ORIGINAL: [usize; 4] = [3, 0, 1, 2];
const PROJECTED: [usize; 4] = [1, 2, 3, 0];
/// Immutable storage retaining the exact initial host and snapshot provenance.
/// Original observations borrow the host and may outlive this projection; `nodes`
/// borrows this owner's inline storage and cannot outlive or cross its move.
/// The initial-only host's shared engine API prevents token revocation. Revisit
/// this admission/lifetime proof if mutable publication or engine access is added.
pub struct ClearanceSourceProjection<'h> {
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    nodes: [SourceNode; 4],
    receipt: ClearanceProjectionReceipt,
}
impl std::fmt::Debug for ClearanceSourceProjection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClearanceSourceProjection")
            .field("receipt", &self.receipt)
            .field("original_indices", &ORIGINAL)
            .finish_non_exhaustive()
    }
}
impl<'h> ClearanceSourceProjection<'h> {
    pub fn host(&self) -> &'h InitialSourceHost {
        self.host
    }
    pub fn snapshot(&self) -> &'h InitialSourceSnapshot {
        self.snapshot
    }
    /// Geometry-facing identity/membership only: all metadata here is deliberately
    /// empty. Do not run selectors/cascade against these as document metadata.
    /// Use `metadata` or `original_node` for the exact borrowed original objects.
    pub fn nodes(&self) -> &[SourceNode; 4] {
        &self.nodes
    }
    pub fn original_index(&self, projected: usize) -> Option<usize> {
        ORIGINAL.get(projected).copied()
    }
    pub fn projected_index(&self, original: usize) -> Option<usize> {
        PROJECTED.get(original).copied()
    }
    pub fn original_node(&self, projected: usize) -> Option<&'h ConstructionNode> {
        self.snapshot
            .construction()
            .nodes()
            .get(self.original_index(projected)?)
    }
    pub fn token(&self, projected: usize) -> Option<&'h SourceRef> {
        self.snapshot.token(self.original_index(projected)?)
    }
    pub fn metadata(&self, projected: usize) -> Option<&'h Metadata> {
        Some(self.original_node(projected)?.metadata())
    }
    pub fn receipt(&self) -> ClearanceProjectionReceipt {
        self.receipt
    }
}
/// Prepare an entire Row/Column root with exactly three empty Row/Column children.
/// Source positions do not imply CSS B1/float/B2 roles. No identities are minted.
///
/// One preflight unit precedes publication/pointer/count/root checks; 26 prepaid
/// units cover 4 membership, 4 content, 4 identity, 6 duplicate, 4 construction,
/// 3 child writes and 1 publication/accounting operations. Each identity lookup
/// is bounded by the checked four live sources. Only the parent's child vector
/// allocates; its actual capacity is charged, excluding borrowed source storage.
///
/// # Errors
/// Rejects publication mismatch, unsupported whole-document profile, invalid
/// identity or resource exhaustion without mutating the host or publishing output.
pub fn prepare_clearance_source_projection<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    limits: ClearanceProjectionLimits,
) -> Result<ClearanceSourceProjection<'h>, ClearanceProjectionError> {
    prepare_with_reserve(host, snapshot, limits, |children| {
        children.try_reserve_exact(3)
    })
}
// Private, synchronous reserve seam: production always performs precisely one
// try_reserve_exact(3). Tests can preserve real reserve errors or spare capacity.
fn prepare_with_reserve<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    limits: ClearanceProjectionLimits,
    reserve: impl FnOnce(&mut Vec<InstanceId>) -> Result<(), TryReserveError>,
) -> Result<ClearanceSourceProjection<'h>, ClearanceProjectionError> {
    use ClearanceProjectionError as E;
    let remaining = limits.work.get().checked_sub(1).ok_or(E::Work)?;
    let published = host.snapshot().ok_or(E::UnpublishedHost)?;
    if !std::ptr::eq(published, snapshot) {
        return Err(E::SnapshotMismatch);
    }
    let source = snapshot.construction();
    if source.nodes().len() != 4 {
        return Err(E::NodeCount);
    }
    if snapshot.tokens().len() != 4 {
        return Err(E::TokenCount);
    }
    if host.engine().live_source_count() != 4 {
        return Err(E::LiveCount);
    }
    if source.root() != 3 {
        return Err(E::Root);
    }
    remaining.checked_sub(26).ok_or(E::Work)?;
    for (original, node) in source.nodes().iter().enumerate() {
        let parent = if original == 3 { None } else { Some(3) };
        if node.parent() != parent {
            return Err(E::Parent { original });
        }
        let (role, position) = if original == 3 {
            (None, None)
        } else {
            (Some("children"), Some(original))
        };
        if node.role() != role || node.role_position() != position {
            return Err(E::Role { original });
        }
    }
    for (original, node) in source.nodes().iter().enumerate() {
        if !matches!(
            node.content_capability(),
            ContentCapability::Managed(ManagedContentFamily::Row | ManagedContentFamily::Column)
        ) {
            return Err(E::Content { original });
        }
    }
    let identity = |original| {
        host.engine()
            .instance(&snapshot.tokens()[original])
            .map_err(|error| E::Identity { original, error })
    };
    let ids = [identity(0)?, identity(1)?, identity(2)?, identity(3)?];
    for first in 0..4 {
        for second in first + 1..4 {
            if ids[first] == ids[second] {
                return Err(E::DuplicateInstance { first, second });
            }
        }
    }
    checked_bytes(3, limits.retained_bytes)?;
    let mut children = Vec::new();
    reserve(&mut children).map_err(E::Allocation)?;
    let retained_bytes = checked_bytes(children.capacity(), limits.retained_bytes)?;
    let receipt = ClearanceProjectionReceipt {
        nodes: 4,
        work: 27,
        child_capacity: children.capacity(),
        child_element_bytes: size_of::<InstanceId>(),
        retained_bytes,
    };
    children.extend_from_slice(&ids[..3]);
    let leaf = |instance| SourceNode {
        instance,
        parent: Some(ids[3]),
        children: Vec::new(),
        metadata: Metadata::default(),
    };
    let nodes = [
        SourceNode {
            instance: ids[3],
            parent: None,
            children,
            metadata: Metadata::default(),
        },
        leaf(ids[0]),
        leaf(ids[1]),
        leaf(ids[2]),
    ];
    Ok(ClearanceSourceProjection {
        host,
        snapshot,
        nodes,
        receipt,
    })
}
fn checked_bytes(capacity: usize, limit: NonZeroUsize) -> Result<usize, ClearanceProjectionError> {
    capacity
        .checked_mul(size_of::<InstanceId>())
        .filter(|bytes| *bytes <= isize::MAX as usize && *bytes <= limit.get())
        .ok_or(ClearanceProjectionError::Bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::engine::ProgressLimits,
        capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
        host::source_publication::PublicationLimits,
    };
    use iced_css_core::{
        identity::SourceChangeLimits, selectors::LinkState,
        source::selector_tree::SelectorTreeLimits,
    };
    use std::{cell::Cell, error::Error, num::NonZeroU64};
    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }
    fn host() -> InitialSourceHost {
        let n = nz(100000);
        let c = CaptureLimits {
            nodes: n,
            depth: n,
            retained_bytes: n,
            work: n,
        };
        type Element = CapturedElement<'static, (), iced::Theme, ()>;
        let leaf = || {
            adapters::row_from_vec(Vec::<Element>::new(), c)
                .unwrap()
                .erase()
        };
        let captured = adapters::column_from_vec(vec![leaf(), leaf(), leaf()], c).unwrap();
        let (_native, construction) = captured.into_parts();
        let mut host = InitialSourceHost::new(
            NonZeroU64::new(100).unwrap(),
            ProgressLimits {
                addresses: NonZeroU64::new(100).unwrap(),
                bindings: n,
                tracks: n,
                retained_bytes: n,
                work: n,
            },
        );
        host.prepare(
            construction,
            vec![
                ConstructionState {
                    link: LinkState::NotLink,
                    hovered: false,
                    active: false,
                    focused: false
                };
                4
            ],
            None,
            PublicationLimits {
                nodes: n,
                snapshot_bytes: n,
                work: n,
                selectors: SelectorTreeLimits {
                    entries: n,
                    retained_bytes: n,
                    work: n,
                },
                identities: SourceChangeLimits {
                    entries: n,
                    retained_bytes: n,
                    work: n,
                },
            },
        )
        .unwrap()
        .commit();
        host
    }
    fn limits() -> ClearanceProjectionLimits {
        ClearanceProjectionLimits {
            work: nz(27),
            retained_bytes: nz(10000),
        }
    }
    #[test]
    fn allocation_seam_preserves_real_reserve_error_and_source() {
        let host = host();
        let raw = Vec::<InstanceId>::new()
            .try_reserve_exact(usize::MAX)
            .unwrap_err();
        let expected = raw.to_string();
        let error = prepare_with_reserve(&host, host.snapshot().unwrap(), limits(), |_| Err(raw))
            .unwrap_err();
        assert!(matches!(error, ClearanceProjectionError::Allocation(_)));
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<TryReserveError>()
                .unwrap()
                .to_string(),
            expected
        );
    }
    #[test]
    fn requested_floor_precedes_reserve_and_actual_capacity_is_checked() {
        let host = host();
        let snapshot = host.snapshot().unwrap();
        let called = Cell::new(false);
        let mut small = limits();
        small.retained_bytes = nz(3 * size_of::<InstanceId>() - 1);
        assert!(matches!(
            prepare_with_reserve(&host, snapshot, small, |v| {
                called.set(true);
                v.try_reserve_exact(3)
            }),
            Err(ClearanceProjectionError::Bytes)
        ));
        assert!(!called.get());
        small.retained_bytes = nz(3 * size_of::<InstanceId>());
        let capacity = Cell::new(0);
        assert!(matches!(
            prepare_with_reserve(&host, snapshot, small, |v| {
                v.try_reserve_exact(4)?;
                capacity.set(v.capacity());
                Ok(())
            }),
            Err(ClearanceProjectionError::Bytes)
        ));
        assert!(capacity.get() >= 4);
        let result = prepare_with_reserve(&host, snapshot, limits(), |v| {
            v.try_reserve_exact(4)?;
            capacity.set(v.capacity());
            Ok(())
        })
        .unwrap();
        assert_eq!(result.receipt().child_capacity, capacity.get());
        assert_eq!(
            result.receipt().retained_bytes,
            capacity.get() * size_of::<InstanceId>()
        );
        assert_eq!(result.nodes()[0].children.len(), 3);
        assert_eq!(result.nodes()[0].children.capacity(), capacity.get());
    }
    #[test]
    fn prepayment_precedes_allocation_and_identity_error_retains_source() {
        let host = host();
        let mut short = limits();
        short.work = nz(26);
        assert!(matches!(
            prepare_with_reserve(&host, host.snapshot().unwrap(), short, |_| panic!(
                "reserve reached before prepayment"
            )),
            Err(ClearanceProjectionError::Work)
        ));
        let other = self::host();
        let engine_error = host
            .engine()
            .instance(other.snapshot().unwrap().token(0).unwrap())
            .unwrap_err();
        let error = ClearanceProjectionError::Identity {
            original: 0,
            error: engine_error,
        };
        assert_eq!(
            error.source().unwrap().downcast_ref::<EngineError>(),
            Some(&engine_error)
        );
    }
}
