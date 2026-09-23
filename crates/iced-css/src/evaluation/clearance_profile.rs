//! Finite whole-document storage and reciprocal postorder mappings.
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
