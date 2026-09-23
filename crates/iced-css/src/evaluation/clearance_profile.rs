//! Finite whole-document storage and reciprocal postorder mappings.
use crate::{
    capture::ConstructionNode,
    host::source_publication::{InitialSourceHost, InitialSourceSnapshot},
};
use iced_css_core::{
    identity::SourceRef,
    source::{Metadata, SourceNode},
};

/// Internal read-only projection contract shared by fixed and dynamic owners.
/// Original objects retain the host lifetime; projected nodes retain the owner borrow.
pub(super) trait ProjectionView<'h> {
    fn host(&self) -> &'h InitialSourceHost;
    fn snapshot(&self) -> &'h InitialSourceSnapshot;
    fn nodes(&self) -> &[SourceNode];
    fn original_index(&self, projected: usize) -> Option<usize>;
    fn projected_index(&self, original: usize) -> Option<usize>;
    fn original_node(&self, projected: usize) -> Option<&'h ConstructionNode>;
    fn token(&self, projected: usize) -> Option<&'h SourceRef>;

    fn metadata(&self, projected: usize) -> Option<&'h Metadata> {
        Some(self.original_node(projected)?.metadata())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceProjectionShape {
    RootRegion,
    NestedRegion,
}
impl ClearanceProjectionShape {
    pub(super) fn original(self) -> &'static [usize] {
        match self {
            Self::RootRegion => &[3, 0, 1, 2],
            Self::NestedRegion => &[4, 3, 0, 1, 2],
        }
    }
    pub(super) fn projected(self) -> &'static [usize] {
        match self {
            Self::RootRegion => &[1, 2, 3, 0],
            Self::NestedRegion => &[2, 3, 4, 1, 0],
        }
    }
    pub(super) fn parents(self) -> &'static [Option<usize>] {
        match self {
            Self::RootRegion => &[None, Some(0), Some(0), Some(0)],
            Self::NestedRegion => &[None, Some(0), Some(1), Some(1), Some(1)],
        }
    }
}
pub(super) enum ClearanceSlots<T> {
    Four([T; 4]),
    Five([T; 5]),
}
impl<T> ClearanceSlots<T> {
    pub(super) fn as_slice(&self) -> &[T] {
        match self {
            Self::Four(v) => v,
            Self::Five(v) => v,
        }
    }
    pub(super) fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Self::Four(v) => v,
            Self::Five(v) => v,
        }
    }
}
