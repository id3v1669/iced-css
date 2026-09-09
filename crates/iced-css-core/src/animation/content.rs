//! Whole computed-content selection before generated-content traversal.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::values::content_computation::ComputedContent;

/// Select retained computed content without re-reading attributes, resolving URLs,
/// reading counters, advancing quotes or allocating. The caller owns endpoint
/// computation and revision refresh. Counter names/separators/styles are discrete
/// arguments; the numeric state eventually read by a counter is not an endpoint.
///
/// The returned value borrows the endpoint and cannot outlive it:
/// ```compile_fail
/// use iced_css_core::{animation::{content::combine_content, combine::Combination},
///     values::content_computation::{ComputedContent, ComputedContentItem}};
/// let selected;
/// {
///     let start = ComputedContent::Items(vec![ComputedContentItem::Text("example")]);
///     let end = ComputedContent::Suppressed;
///     selected = combine_content(&start, &end, Combination::Add).unwrap();
/// }
/// println!("{:?}", selected);
/// ```
pub fn combine_content<'a, 'source>(
    start: &'a ComputedContent<'source>,
    end: &'a ComputedContent<'source>,
    operation: Combination,
) -> Result<&'a ComputedContent<'source>, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
