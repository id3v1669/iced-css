//! Bounded dynamic computed box facts for a complete sibling projection.
//! This stage establishes computed properties only, rather than used geometry.

use super::{
    clearance_box_facts::{
        BoxFactMode, ClearanceBoxFact, ClearanceBoxFactsError, ClearanceBoxFactsLimits,
        ClearanceBoxFactsReceipt, evaluate_box_rows, initial_box_fact, prepare_box_facts,
    },
    sibling_source_projection::SiblingSourceProjection,
    source_display::SourceDisplay,
};
use iced_css_core::values::NumericLimits;
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

/// Independent source-count, row-storage, and evaluation service limits.
#[derive(Debug, Clone, Copy)]
pub struct SiblingBoxFactsLimits {
    /// Maximum admitted projected source count.
    pub sources: NonZeroUsize,
    /// Maximum retained capacity of the computed row vector.
    pub retained_bytes: NonZeroUsize,
    /// Existing bounded selector, cascade, and numeric service limits.
    pub evaluation: ClearanceBoxFactsLimits,
}

/// Existing evaluation service receipts plus actual dynamic row capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingBoxFactsReceipt {
    /// Existing target, work, service, numeric, and owned-byte receipt.
    pub evaluation: ClearanceBoxFactsReceipt,
    /// Actual capacity of the private computed row vector.
    pub row_capacity: usize,
}

/// Failure to evaluate a complete dynamic sibling fact batch.
#[derive(Debug)]
pub enum SiblingBoxFactsError {
    /// The projection source count is below five or exceeds its independent limit.
    Sources,
    /// Requested or actual row capacity exceeds the retained-byte limit.
    Bytes,
    /// Dynamic row reservation failed.
    Allocation(TryReserveError),
    /// Existing source join, budget, cascade, defaulting, or property evaluation failed.
    Evaluation(ClearanceBoxFactsError),
}

impl std::fmt::Display for SiblingBoxFactsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sibling box facts: {self:?}")
    }
}

impl std::error::Error for SiblingBoxFactsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Evaluation(error) => Some(error),
            Self::Sources | Self::Bytes => None,
        }
    }
}

/// Immutable computed rows for every actual projected source.
#[must_use]
pub struct SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r> {
    projection: &'p SiblingSourceProjection<'h>,
    display: &'d SourceDisplay<'s, 'b, 'r>,
    rows: Vec<ClearanceBoxFact>,
    receipt: SiblingBoxFactsReceipt,
    /// PCT: the numeric limits the rows were evaluated with.
    numeric: NumericLimits,
}

impl<'p, 'h, 'd, 's, 'b, 'r> SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r> {
    /// Exact dynamic projection supplied to evaluation.
    pub fn projection(&self) -> &'p SiblingSourceProjection<'h> {
        self.projection
    }

    /// Exact display owner supplied to evaluation.
    pub fn display(&self) -> &'d SourceDisplay<'s, 'b, 'r> {
        self.display
    }

    /// Computed row for one projected source.
    pub fn row(&self, projected: usize) -> Option<&ClearanceBoxFact> {
        self.rows.get(projected)
    }

    /// Existing service receipts and actual dynamic row capacity.
    pub fn receipt(&self) -> SiblingBoxFactsReceipt {
        self.receipt
    }

    /// PCT: the numeric limits of this evaluation (`limits.evaluation.numeric`), with
    /// which a later stage resolves the rows' percentage facts.
    pub fn numeric_limits(&self) -> NumericLimits {
        self.numeric
    }
}

/// Evaluates all element, `::before`, and `::after` targets in a complete sibling capture.
///
/// This is the Sequence fact mode: every CSS2.2 border style is admitted and every row
/// carries computed `border-*-width` facts with true provenance (see `ClearanceBoxFact`).
/// Work is `35*N+2+r` for N sources and r rules, and the numeric allowance is `22*N`
/// conversions; the Legacy entry keeps `27*N+2+r` and `14*N`. A Sequence row whose
/// computed `position` is `relative` (O included) also resolves `top`, `right`, `bottom`
/// and `left`, after its other longhands and before its pseudos, and can fail there with
/// `Unsupported` (`PercentageLength`, `RelativeLength` or `InheritedOffset`),
/// `InvalidValue`, `Defaulting`, `InitialRecipe` or a `Length` conversion error.
/// Every Sequence row also carries its computed `min-width`, `min-height`, `max-width`
/// and `max-height` (absolute, or `none` for a maximum, or inherited) and can fail there
/// with `Unsupported` (`PercentageLength` or `RelativeLength`), `InvalidValue` or a
/// `Length` conversion error. A percentage `width`, margin, `right` or `left` (PCT) is
/// kept exactly as `BoxLengthFact::Percentage`, without a conversion, and can fail
/// with `InvalidValue` (a negative `width`) or `Unsupported` (`PercentagePrecision`);
/// every other percentage keeps `PercentageLength`.
///
/// Shared registry/rule/declaration preflight and aggregate service reservations finish
/// before row allocation. Actual row capacity is then charged before row initialization
/// and before selector or cascade scratch is constructed.
///
/// # Errors
///
/// Returns [`SiblingBoxFactsError::Sources`] before proportional work when the source
/// limit is insufficient, [`SiblingBoxFactsError::Bytes`] for requested or actual row
/// capacity, and preserves allocation and existing evaluation causes. Failure publishes
/// no partial batch.
pub fn evaluate_sibling_box_facts<'p, 'h, 'd, 's, 'b, 'r>(
    projection: &'p SiblingSourceProjection<'h>,
    display: &'d SourceDisplay<'s, 'b, 'r>,
    limits: SiblingBoxFactsLimits,
) -> Result<SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>, SiblingBoxFactsError> {
    evaluate_with_reserve(projection, display, limits, |rows, requested| {
        rows.try_reserve_exact(requested)
    })
}

fn evaluate_with_reserve<'p, 'h, 'd, 's, 'b, 'r>(
    projection: &'p SiblingSourceProjection<'h>,
    display: &'d SourceDisplay<'s, 'b, 'r>,
    limits: SiblingBoxFactsLimits,
    reserve: impl FnOnce(&mut Vec<ClearanceBoxFact>, usize) -> Result<(), TryReserveError>,
) -> Result<SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>, SiblingBoxFactsError> {
    use SiblingBoxFactsError as E;

    let count = projection.nodes().len();
    if count < 5 || count > limits.sources.get() {
        return Err(E::Sources);
    }

    let mut prepared = prepare_box_facts(
        projection,
        display,
        limits.evaluation,
        BoxFactMode::Sequence,
    )
    .map_err(E::Evaluation)?;
    let (mut rows, owned_heap_bytes) = allocate_rows(count, limits.retained_bytes, reserve)?;
    prepared.receipt.owned_heap_bytes = owned_heap_bytes;
    evaluate_box_rows(
        projection,
        display,
        limits.evaluation,
        &mut prepared,
        &mut rows,
    )
    .map_err(E::Evaluation)?;

    let receipt = SiblingBoxFactsReceipt {
        evaluation: prepared.receipt,
        row_capacity: rows.capacity(),
    };
    Ok(SiblingBoxFacts {
        projection,
        display,
        rows,
        receipt,
        numeric: limits.evaluation.numeric,
    })
}

fn allocate_rows(
    count: usize,
    retained_bytes: NonZeroUsize,
    reserve: impl FnOnce(&mut Vec<ClearanceBoxFact>, usize) -> Result<(), TryReserveError>,
) -> Result<(Vec<ClearanceBoxFact>, usize), SiblingBoxFactsError> {
    checked_row_bytes(count, retained_bytes)?;
    let mut rows = Vec::new();
    reserve(&mut rows, count).map_err(SiblingBoxFactsError::Allocation)?;
    let bytes = checked_row_bytes(rows.capacity(), retained_bytes)?;
    rows.resize(count, initial_box_fact());
    Ok((rows, bytes))
}

fn checked_row_bytes(
    capacity: usize,
    retained_bytes: NonZeroUsize,
) -> Result<usize, SiblingBoxFactsError> {
    capacity
        .checked_mul(size_of::<ClearanceBoxFact>())
        .filter(|&bytes| bytes <= isize::MAX as usize && bytes <= retained_bytes.get())
        .ok_or(SiblingBoxFactsError::Bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn requested_floor_precedes_reserve_and_actual_capacity_is_charged() {
        let floor = 6 * size_of::<ClearanceBoxFact>();
        let error = allocate_rows(6, nz(floor - 1), |_, _| {
            panic!("reserve reached before requested floor")
        })
        .unwrap_err();
        assert!(matches!(error, SiblingBoxFactsError::Bytes));

        let error = allocate_rows(6, nz(floor), |rows, requested| {
            rows.try_reserve_exact(requested + 1)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingBoxFactsError::Bytes));

        let (rows, bytes) = allocate_rows(6, nz(floor * 2), |rows, requested| {
            rows.try_reserve_exact(requested + 1)
        })
        .unwrap();
        assert!(rows.capacity() >= 7);
        assert_eq!(bytes, rows.capacity() * size_of::<ClearanceBoxFact>());
    }

    #[test]
    fn genuine_reserve_error_is_preserved_as_the_source() {
        let real = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
        let error = allocate_rows(6, NonZeroUsize::MAX, |rows, _| {
            rows.try_reserve_exact(usize::MAX)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingBoxFactsError::Allocation(_)));
        assert!(error.source().unwrap().is::<TryReserveError>());
        assert_eq!(error.source().unwrap().to_string(), real.to_string());
    }
}
