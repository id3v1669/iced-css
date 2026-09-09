//! Materialized explicit casing mappings with original-scalar byte provenance.
//! Byte origins are not caret affinity or CSS word, locale, or ordering policy.
use super::{
    casing_data::CaseMapping, casing_selection::CasingSelection, newlines::NormalizedText,
};
use std::{mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct CasingOutputLimits {
    pub scalars: NonZeroUsize,
    /// Maximum initialized UTF-8 output length, independently of capacity.
    pub output_bytes: NonZeroUsize,
    /// Admitted successful String + range-vector payload capacities, excluding
    /// borrowed/static owners, allocator overhead and rejected physical peaks.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation 1+2*n+2*u (input/output scalar counts).
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingOutputError {
    Scalars,
    OutputBytes,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for CasingOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "casing output materialization: {self:?}")
    }
}
impl std::error::Error for CasingOutputError {}
/// Immutable output retains the exact selection and its complete owner chain.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_output::*, casing_selection::*};
/// fn cannot_drop(selection:CasingSelection<'_, '_, '_>, limits:CasingOutputLimits) {
///     let result=materialize_case_mappings(&selection,limits).unwrap();
///     drop(selection);
///     let _=result.output();
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_output::*, casing_selection::*, casing_contexts::*};
/// fn cannot_drop(contexts:CasingContexts<'_, '_>, sl:CasingSelectionLimits, ol:CasingOutputLimits) {
///     let selection=select_case_mappings(&contexts,CasingOperation::Lower,None,sl).unwrap();
///     let result=materialize_case_mappings(&selection,ol).unwrap();
///     drop(contexts);
///     let _=result.output();
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_output::*, casing_selection::*, casing_contexts::*, newlines::*};
/// fn cannot_drop(text:NormalizedText<'_>, cl:CasingContextLimits, sl:CasingSelectionLimits, ol:CasingOutputLimits) {
///     let contexts=evaluate_casing_contexts(&text,cl).unwrap();
///     let selection=select_case_mappings(&contexts,CasingOperation::Lower,None,sl).unwrap();
///     let result=materialize_case_mappings(&selection,ol).unwrap();
///     drop(text);
///     let _=result.output();
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_output::*, casing_selection::*, casing_contexts::*, newlines::*};
/// fn cannot_drop(source:String, nl:NewlineLimits, cl:CasingContextLimits, sl:CasingSelectionLimits, ol:CasingOutputLimits) {
///     let text=normalize_default_newlines(&source,nl).unwrap();
///     let contexts=evaluate_casing_contexts(&text,cl).unwrap();
///     let selection=select_case_mappings(&contexts,CasingOperation::Lower,None,sl).unwrap();
///     let result=materialize_case_mappings(&selection,ol).unwrap();
///     drop(source);
///     let _=result.output();
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct CasingOutput<'selection, 'contexts, 'text, 'source> {
    selection: &'selection CasingSelection<'contexts, 'text, 'source>,
    output: String,
    ranges: Vec<Range<usize>>,
    output_scalars: usize,
    retained_bytes: usize,
    work: usize,
}
impl<'selection, 'contexts, 'text, 'source> CasingOutput<'selection, 'contexts, 'text, 'source> {
    pub fn selection(&self) -> &'selection CasingSelection<'contexts, 'text, 'source> {
        self.selection
    }
    pub fn text(&self) -> &'text NormalizedText<'source> {
        self.selection.text()
    }
    pub fn output(&self) -> &str {
        &self.output
    }
    pub fn scalar_count(&self) -> usize {
        self.ranges.len()
    }
    pub fn output_scalar_count(&self) -> usize {
        self.output_scalars
    }
    pub fn range(&self, index: usize) -> Option<&Range<usize>> {
        self.ranges.get(index)
    }
    /// Owning original scalar for an output byte, including interior UTF-8 bytes.
    /// End/beyond-end returns None; empty contributions never own a byte.
    /// This is not an insertion-position or caret-affinity query.
    pub fn source_scalar_at_output_byte(&self, offset: usize) -> Option<usize> {
        byte_origin(&self.ranges, self.output.len(), offset)
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
fn byte_origin(ranges: &[Range<usize>], length: usize, offset: usize) -> Option<usize> {
    if offset >= length {
        return None;
    }
    let index = ranges.partition_point(|r| r.end <= offset);
    ranges.get(index).map(|_| index)
}
fn base_work(count: usize) -> Result<usize, CasingOutputError> {
    count
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or(CasingOutputError::Work)
}
fn reserve_work(remaining: &mut usize, amount: usize) -> Result<(), CasingOutputError> {
    *remaining = remaining
        .checked_sub(amount)
        .ok_or(CasingOutputError::Work)?;
    Ok(())
}
fn add_output(
    scalars: usize,
    bytes: usize,
    scalar: char,
    allowance: usize,
) -> Result<(usize, usize), CasingOutputError> {
    let scalars = scalars.checked_add(1).ok_or(CasingOutputError::Work)?;
    let bytes = bytes
        .checked_add(scalar.len_utf8())
        .filter(|&n| n <= allowance)
        .ok_or(CasingOutputError::OutputBytes)?;
    Ok((scalars, bytes))
}
fn payload_bytes(
    string_capacity: usize,
    range_capacity: usize,
    allowance: usize,
) -> Result<usize, CasingOutputError> {
    let map = range_capacity
        .checked_mul(size_of::<Range<usize>>())
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or(CasingOutputError::Bytes)?;
    if string_capacity > isize::MAX as usize {
        return Err(CasingOutputError::Bytes);
    }
    string_capacity
        .checked_add(map)
        .filter(|&n| n <= allowance)
        .ok_or(CasingOutputError::Bytes)
}
/// Emit the exact immutable selections, preserving one output-byte range per
/// original normalized scalar, including empty deletions and full expansions.
/// The caller supplies the complete selection domain. This does not infer CSS
/// scope, locale, words, stage ordering, or caret/selection affinity.
///
/// # Errors
/// Returns Scalars for input count, Work for prepaid base/output-scalar exhaustion,
/// OutputBytes for UTF-8 length excess/overflow, Bytes for requested/actual payload
/// or addressability excess, and Allocation for failed fallible reservation.
/// Failure leaves all borrowed owners unchanged and publishes no partial output.
pub fn materialize_case_mappings<'selection, 'contexts, 'text, 'source>(
    selection: &'selection CasingSelection<'contexts, 'text, 'source>,
    limits: CasingOutputLimits,
) -> Result<CasingOutput<'selection, 'contexts, 'text, 'source>, CasingOutputError> {
    let count = selection.scalar_count();
    if count > limits.scalars.get() {
        return Err(CasingOutputError::Scalars);
    }
    let mut remaining = limits.work.get();
    reserve_work(&mut remaining, base_work(count)?)?;
    let (mut output_scalars, mut output_bytes) = (0, 0);
    for (index, scalar) in selection.text().text().chars().enumerate() {
        let mapping = selection
            .mapping(index)
            .expect("complete selection covers every original scalar");
        let identity = [scalar];
        let mapped = match mapping {
            CaseMapping::Identity => &identity[..],
            CaseMapping::Replacement(chars) => chars,
        };
        for &c in mapped {
            // Pay for both this length inspection and the later emission first.
            reserve_work(&mut remaining, 2)?;
            (output_scalars, output_bytes) =
                add_output(output_scalars, output_bytes, c, limits.output_bytes.get())?;
        }
    }
    let work = limits.work.get() - remaining;
    payload_bytes(output_bytes, count, limits.retained_bytes.get())?;
    let mut output = String::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|_| CasingOutputError::Allocation)?;
    payload_bytes(output.capacity(), count, limits.retained_bytes.get())?;
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(count)
        .map_err(|_| CasingOutputError::Allocation)?;
    let retained_bytes = payload_bytes(
        output.capacity(),
        ranges.capacity(),
        limits.retained_bytes.get(),
    )?;
    for (index, scalar) in selection.text().text().chars().enumerate() {
        let start = output.len();
        match selection
            .mapping(index)
            .expect("complete selection covers every original scalar")
        {
            CaseMapping::Identity => output.push(scalar),
            CaseMapping::Replacement(chars) => {
                for &c in chars {
                    output.push(c);
                }
            }
        }
        ranges.push(start..output.len());
    }
    debug_assert_eq!(output.len(), output_bytes);
    debug_assert_eq!(ranges.len(), count);
    Ok(CasingOutput {
        selection,
        output,
        ranges,
        output_scalars,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_impossible_intervals_use_production_byte_lookup() {
        // Leading/repeated empty contributions are synthetic, not claims about Unicode mappings.
        for count in 0..=6u32 {
            for mut code in 0..4usize.pow(count) {
                let (mut ranges, mut owners, mut end) = (Vec::new(), Vec::new(), 0);
                for index in 0..count as usize {
                    let length = code % 4;
                    code /= 4;
                    ranges.push(end..end + length);
                    end += length;
                    owners.extend(std::iter::repeat_n(index, length));
                }
                for offset in 0..=end {
                    assert_eq!(
                        byte_origin(&ranges, end, offset),
                        owners.get(offset).copied()
                    );
                }
                assert_eq!(byte_origin(&ranges, end, usize::MAX), None);
            }
        }
    }
    #[test]
    fn checked_resource_helpers_include_separate_addressability_and_actual_capacity() {
        use super::super::{casing_contexts::*, casing_selection::*, newlines::*};
        assert_eq!(base_work(0), Ok(1));
        assert_eq!(base_work(usize::MAX), Err(CasingOutputError::Work));
        assert_eq!(base_work(usize::MAX / 2), Ok(usize::MAX));
        let mut remaining = 1;
        assert_eq!(
            reserve_work(&mut remaining, 2),
            Err(CasingOutputError::Work)
        );
        assert_eq!(remaining, 1);
        assert_eq!(
            add_output(usize::MAX, 0, 'a', usize::MAX),
            Err(CasingOutputError::Work)
        );
        assert_eq!(
            add_output(0, usize::MAX, 'a', usize::MAX),
            Err(CasingOutputError::OutputBytes)
        );
        assert_eq!(
            add_output(0, 0, '漢', 2),
            Err(CasingOutputError::OutputBytes)
        );
        assert_eq!(
            payload_bytes(isize::MAX as usize + 1, 0, usize::MAX),
            Err(CasingOutputError::Bytes)
        );
        assert_eq!(
            payload_bytes(0, usize::MAX, usize::MAX),
            Err(CasingOutputError::Bytes)
        );
        let max_ranges = isize::MAX as usize / size_of::<Range<usize>>();
        assert_eq!(
            payload_bytes(0, max_ranges + 1, usize::MAX),
            Err(CasingOutputError::Bytes)
        );
        // Combined independent allocations may exceed isize::MAX, without either doing so.
        let sum = isize::MAX as usize + size_of::<Range<usize>>();
        assert_eq!(payload_bytes(isize::MAX as usize, 1, usize::MAX), Ok(sum));
        assert_eq!(
            payload_bytes(1, 1, size_of::<Range<usize>>()),
            Err(CasingOutputError::Bytes)
        );
        let n = NonZeroUsize::new(1000).unwrap();
        let text = normalize_default_newlines(
            "ß漢",
            NewlineLimits {
                input_bytes: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let contexts = evaluate_casing_contexts(
            &text,
            CasingContextLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let selection = select_case_mappings(
            &contexts,
            CasingOperation::Upper,
            None,
            CasingSelectionLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let got = materialize_case_mappings(
            &selection,
            CasingOutputLimits {
                scalars: n,
                output_bytes: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        assert_eq!(
            got.retained_bytes(),
            got.output.capacity() + got.ranges.capacity() * size_of::<Range<usize>>()
        );
        assert!(got.output.capacity() >= got.output.len());
        assert!(got.ranges.capacity() >= got.ranges.len());
    }
}
