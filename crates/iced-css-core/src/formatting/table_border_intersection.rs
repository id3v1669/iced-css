//! Clip resolved borders to admitted cell boundaries without changing winners.
use super::{table_borders::CollapsedBorderSegment, table_line_boundaries::TableLineBoundaries};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct TableIntersectionLimits {
    pub input_segments: NonZeroUsize,
    pub segments: NonZeroUsize,
    /// Actual retained output capacity; borrowed inputs are excluded.
    pub retained_bytes: NonZeroUsize,
    /// Admission, input validation, two-pointer steps, and output emissions.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableIntersectionError {
    Range,
    InputSegments,
    Segments,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for TableIntersectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table border intersection: {self:?}")
    }
}
impl std::error::Error for TableIntersectionError {}
#[derive(Debug)]
pub struct TableBorderIntersection<'boundary, 'grid> {
    boundaries: &'boundary TableLineBoundaries<'grid>,
    segments: Vec<CollapsedBorderSegment>,
}
impl<'boundary, 'grid> TableBorderIntersection<'boundary, 'grid> {
    pub fn boundaries(&self) -> &'boundary TableLineBoundaries<'grid> {
        self.boundaries
    }
    pub fn segments(&self) -> &[CollapsedBorderSegment] {
        &self.segments
    }
    pub fn retained_bytes(&self) -> usize {
        self.segments.capacity() * size_of::<CollapsedBorderSegment>()
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), TableIntersectionError> {
        self.0 = self.0.checked_sub(n).ok_or(TableIntersectionError::Work)?;
        Ok(())
    }
}
/// Preserve resolved source indices while removing spanning-cell interiors.
///
/// The caller must supply segments from the same physical axis and line as the
/// boundaries. Untagged splitter output cannot prove this provenance. Winner
/// indices continue to refer to the original contender slice, not this output.
/// Adjacent output segments are not merged; absent/suppressed values are retained.
///
/// The output keeps the boundary owner borrowed:
/// ```compile_fail,E0505
/// use iced_css_core::formatting::{table_line_boundaries::TableLineBoundaries, table_border_intersection::{intersect_table_borders,TableIntersectionLimits}};
/// fn replace(boundaries:TableLineBoundaries<'_>,limits:TableIntersectionLimits) {
///     let output=intersect_table_borders(&boundaries,&[],limits).unwrap();
///     drop(boundaries);
///     std::hint::black_box(output.segments());
/// }
/// ```
///
/// # Errors
/// Rejects nonpositive, overlapping, unsorted or out-of-extent input ranges and
/// exhausted count/work/storage/allocation bounds. Failure preserves both inputs.
pub fn intersect_table_borders<'boundary, 'grid>(
    boundaries: &'boundary TableLineBoundaries<'grid>,
    input: &[CollapsedBorderSegment],
    limits: TableIntersectionLimits,
) -> Result<TableBorderIntersection<'boundary, 'grid>, TableIntersectionError> {
    let mut work = Work(limits.work.get());
    work.charge(1)?;
    if input.len() > limits.input_segments.get() {
        return Err(TableIntersectionError::InputSegments);
    }
    work.charge(input.len())?;
    let mut previous_end = 0;
    for segment in input {
        if segment.start >= segment.end
            || segment.start < previous_end
            || segment.end > boundaries.extent()
        {
            return Err(TableIntersectionError::Range);
        }
        previous_end = segment.end;
    }
    let mut segments = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < input.len() && j < boundaries.intervals().len() {
        work.charge(1)?;
        let segment = input[i];
        let interval = boundaries.intervals()[j];
        let start = segment.start.max(interval.start);
        let end = segment.end.min(interval.end);
        if start < end {
            work.charge(1)?;
            let count = segments
                .len()
                .checked_add(1)
                .ok_or(TableIntersectionError::Segments)?;
            if count > limits.segments.get() {
                return Err(TableIntersectionError::Segments);
            }
            let bytes = |count: usize| {
                count
                    .checked_mul(size_of::<CollapsedBorderSegment>())
                    .filter(|n| *n <= limits.retained_bytes.get() && *n <= isize::MAX as usize)
                    .ok_or(TableIntersectionError::Bytes)
            };
            bytes(count)?;
            // Grow by doubling, within what the limits allow, so pushes do not each copy the
            // whole output.
            if segments.len() == segments.capacity() {
                let most = limits
                    .segments
                    .get()
                    .min(limits.retained_bytes.get() / size_of::<CollapsedBorderSegment>());
                let grow = segments.len().max(4).min(most - segments.len()).max(1);
                segments
                    .try_reserve_exact(grow)
                    .map_err(|_| TableIntersectionError::Allocation)?;
            }
            bytes(segments.capacity())?;
            segments.push(CollapsedBorderSegment {
                start,
                end,
                border: segment.border,
            });
        }
        if segment.end <= interval.end {
            i += 1;
        }
        if interval.end <= segment.end {
            j += 1;
        }
    }
    // What the doubling left over is not retained.
    segments.shrink_to_fit();
    Ok(TableBorderIntersection {
        boundaries,
        segments,
    })
}
