use super::{BorderCandidate, CollapsedBorder, resolve_indexed};
use std::{mem::size_of, num::NonZeroUsize};

/// One contender on a single grid line, between two logical track boundaries.
/// All edges supplied to a split operation must belong to the same grid line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderEdge {
    start: u32,
    end: u32,
    candidate: BorderCandidate,
}
impl BorderEdge {
    pub fn new(
        start: u32,
        end: u32,
        candidate: BorderCandidate,
    ) -> Result<Self, BorderSegmentError> {
        if start >= end {
            return Err(BorderSegmentError::Range);
        }
        Ok(Self {
            start,
            end,
            candidate,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BorderSegmentLimits {
    pub edges: NonZeroUsize,
    pub segments: NonZeroUsize,
    /// Combined endpoint/output vector capacities, including temporary storage.
    /// The borrowed input slice and fixed stack metadata are separate.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderSegmentError {
    Range,
    Edges,
    Segments,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for BorderSegmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "collapsed border segmentation: {self:?}")
    }
}
impl std::error::Error for BorderSegmentError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapsedBorderSegment {
    pub start: u32,
    pub end: u32,
    /// Winner index refers to the original BorderEdge slice, not a sorted copy.
    pub border: CollapsedBorder,
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), BorderSegmentError> {
        self.0 = self.0.checked_sub(amount).ok_or(BorderSegmentError::Work)?;
        Ok(())
    }
}
fn reserve<T>(count: usize, available: &mut usize) -> Result<Vec<T>, BorderSegmentError> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(BorderSegmentError::Bytes)?;
    if bytes > *available || bytes > isize::MAX as usize {
        return Err(BorderSegmentError::Bytes);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| BorderSegmentError::Allocation)?;
    let retained = result
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(BorderSegmentError::Bytes)?;
    *available = available
        .checked_sub(retained)
        .ok_or(BorderSegmentError::Bytes)?;
    Ok(result)
}
/// Split at every contender boundary and resolve each segment atomically.
/// Endpoints are logical indices; this performs no physical length arithmetic.
///
/// # Errors
/// Rejects explicit limits, checked arithmetic overflow and allocation failures.
/// Sorting and segment scans are bounded by the supplied work allowance.
pub fn split_collapsed_borders(
    edges: &[BorderEdge],
    limits: BorderSegmentLimits,
) -> Result<Vec<CollapsedBorderSegment>, BorderSegmentError> {
    if edges.len() > limits.edges.get() {
        return Err(BorderSegmentError::Edges);
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(BorderSegmentError::Bytes);
    }
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let mut work = Work(limits.work.get());
    let count = edges
        .len()
        .checked_mul(2)
        .ok_or(BorderSegmentError::Bytes)?;
    work.charge(count)?;
    let mut available = limits.retained_bytes.get();
    let mut endpoints = reserve(count, &mut available)?;
    for edge in edges {
        endpoints.push(edge.start);
        endpoints.push(edge.end);
    }
    // Budgeted insertion sort: one charged step includes comparison and possible
    // fixed-size swap. No uninterruptible input-sized library sort is hidden here.
    for i in 1..endpoints.len() {
        let mut at = i;
        while at > 0 {
            work.charge(1)?;
            if endpoints[at - 1] <= endpoints[at] {
                break;
            }
            endpoints.swap(at - 1, at);
            at -= 1;
        }
    }
    work.charge(endpoints.len())?;
    endpoints.dedup();
    let count = endpoints.len() - 1; // Nonempty input contributes two distinct ends.
    if count > limits.segments.get() {
        return Err(BorderSegmentError::Segments);
    }
    // Prepay output traversal and all candidate coverage/priority checks.
    let scan = edges
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(count))
        .ok_or(BorderSegmentError::Work)?;
    work.charge(scan)?;
    let mut segments = reserve(count, &mut available)?;
    for ends in endpoints.windows(2) {
        let (start, end) = (ends[0], ends[1]);
        let candidates = edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.start <= start && edge.end >= end)
            .map(|(index, edge)| (index, &edge.candidate));
        segments.push(CollapsedBorderSegment {
            start,
            end,
            border: resolve_indexed(candidates),
        });
    }
    Ok(segments)
}
