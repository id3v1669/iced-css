//! Source exclusion and preorder traversal borrowing the exact computed owner.
use super::source_display::SourceDisplay;
use iced_css_core::{formatting::display::Display, identity::SourceRef};
use std::{mem::size_of, num::NonZeroUsize};

/// Exclusion by computed display:none only; eligibility does not certify a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    Eligible,
    DisplayNone,
    /// First directly suppressing ancestor from the root, in original source indices.
    Ancestor {
        index: usize,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EligibleSource {
    /// Original construction index, not an ordered ordinal or box identity.
    pub source: usize,
    /// Actual source parent's ordinal in this owner's ordered sequence.
    pub parent: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
pub struct SourceEligibilityLimits {
    pub nodes: NonZeroUsize,
    /// Admitted simultaneous vector payload capacity, including temporary links.
    pub retained_bytes: NonZeroUsize,
    /// Exactly 1+6*n+k structural entries; allocator internals are excluded.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEligibilityError {
    Nodes,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for SourceEligibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source eligibility: {self:?}")
    }
}
impl std::error::Error for SourceEligibilityError {}
#[derive(Clone, Copy, Default)]
struct Record {
    disposition: Option<Eligibility>,
    ordinal: Option<usize>,
}
#[derive(Clone, Copy, Default)]
struct Links {
    first_child: Option<usize>,
    next_sibling: Option<usize>,
}

/// Immutable source eligibility, preserving suppressed source facts and tokens.
/// This does not establish visibility, normal flow, formatting boxes or anonymous
/// text participation. The exact display owner and all its dependencies stay borrowed.
///
/// ```compile_fail,E0505
/// use iced_css::evaluation::{source_display::SourceDisplay, source_eligibility::*};
/// fn outlive(display: SourceDisplay<'_, '_, '_>, limits: SourceEligibilityLimits) {
///     let result = evaluate_source_eligibility(&display, limits).unwrap();
///     drop(display);
///     let _ = result.token(0);
/// }
/// ```
/// ```compile_fail,E0502
/// use iced_css::{host::source_publication::InitialSourceSnapshot,
///     evaluation::{display_rules::*, source_display::*, source_eligibility::*}};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog};
/// fn mutate(snapshot: &InitialSourceSnapshot, ua: &UaDisplayCatalog,
///     sheets: &mut AuthorSheets, bl: BundleLimits, dl: SourceDisplayLimits,
///     el: SourceEligibilityLimits) {
///     let bindings = [];
///     let bundle = DisplayRuleBundle::new(ua, &bindings, sheets, bl).unwrap();
///     let display = evaluate_source_display(snapshot, &bundle, dl).unwrap();
///     let result = evaluate_source_eligibility(&display, el).unwrap();
///     sheets.set_author_enabled(false).unwrap();
///     let _ = result.token(0);
/// }
/// ```
#[must_use]
pub struct SourceEligibility<'display, 'snapshot, 'bundle, 'rules> {
    display: &'display SourceDisplay<'snapshot, 'bundle, 'rules>,
    records: Vec<Record>,
    ordered: Vec<EligibleSource>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'display, 'snapshot, 'bundle, 'rules> SourceEligibility<'display, 'snapshot, 'bundle, 'rules> {
    pub fn display(&self) -> &'display SourceDisplay<'snapshot, 'bundle, 'rules> {
        self.display
    }
    /// Checked original source index; suppressed sources retain their disposition.
    pub fn disposition(&self, source: usize) -> Option<Eligibility> {
        self.records.get(source)?.disposition
    }
    /// Ordered ordinal for an original source index; None for excluded/invalid sources.
    pub fn ordinal(&self, source: usize) -> Option<usize> {
        self.records.get(source)?.ordinal
    }
    pub fn ordered(&self) -> &[EligibleSource] {
        &self.ordered
    }
    /// Checked ordered-sequence ordinal, not an original source index.
    pub fn entry(&self, ordinal: usize) -> Option<&EligibleSource> {
        self.ordered.get(ordinal)
    }
    /// Exact borrowed token at an original source index, including excluded sources.
    pub fn token(&self, source: usize) -> Option<&SourceRef> {
        self.display.token(source)
    }
    pub fn node_count(&self) -> usize {
        self.records.len()
    }
    pub fn eligible_count(&self) -> usize {
        self.ordered.len()
    }
    /// Actual retained Record and EligibleSource vector payload capacities.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Successful candidate's admitted simultaneous payload capacity, including links.
    /// Excludes borrowed owners and allocator internals. This does not bound the
    /// allocator's physical peak or transient over-acquisition before rejection.
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}

/// Classify all sources and emit eligible sources in original sibling preorder.
/// Traverses all n nodes with exactly n visits and n completions, including root,
/// even for an empty eligible sequence. Total structural work is 1+6*n+k.
///
/// # Errors
/// Rejects count, checked work/byte admission, or fallible allocation. Work is
/// admitted in two stages: 1+6*n before allocation, then +k after classification.
/// Actual capacities are checked after reservation; an allocator may transiently
/// acquire excess capacity before rejection. No partial result escapes and all
/// borrowed owners remain unchanged on every failure.
pub fn evaluate_source_eligibility<'display, 'snapshot, 'bundle, 'rules>(
    display: &'display SourceDisplay<'snapshot, 'bundle, 'rules>,
    limits: SourceEligibilityLimits,
) -> Result<SourceEligibility<'display, 'snapshot, 'bundle, 'rules>, SourceEligibilityError> {
    let nodes = display.snapshot().construction().nodes();
    let n = nodes.len();
    if n > limits.nodes.get() {
        return Err(SourceEligibilityError::Nodes);
    }
    let base = base_work(n, limits.work)?;
    let allowance = limits.retained_bytes;
    bytes(n, n, 0, allowance)?;
    let mut records = Vec::<Record>::new();
    records
        .try_reserve_exact(n)
        .map_err(|_| SourceEligibilityError::Allocation)?;
    bytes(records.capacity(), n, 0, allowance)?;
    let mut links = Vec::<Links>::new();
    links
        .try_reserve_exact(n)
        .map_err(|_| SourceEligibilityError::Allocation)?;
    bytes(records.capacity(), links.capacity(), 0, allowance)?;
    records.resize(n, Record::default());
    links.resize(n, Links::default());
    let mut k = 0usize;
    for index in (0..n).rev() {
        let parent = nodes[index].parent().map(|parent| {
            (
                parent,
                records[parent]
                    .disposition
                    .expect("private postorder parent is classified"),
            )
        });
        let disposition = match parent {
            Some((index, Eligibility::DisplayNone))
            | Some((_, Eligibility::Ancestor { index })) => Eligibility::Ancestor { index },
            _ if display
                .resolution(index)
                .expect("complete display has every source")
                .display
                == Display::None =>
            {
                Eligibility::DisplayNone
            }
            _ => Eligibility::Eligible,
        };
        records[index].disposition = Some(disposition);
        if disposition == Eligibility::Eligible {
            k = k.checked_add(1).ok_or(SourceEligibilityError::Nodes)?;
        }
    }
    let work = total_work(base, k, limits.work)?;
    for index in (0..n).rev() {
        if let Some(parent) = nodes[index].parent() {
            links[index].next_sibling = links[parent].first_child;
            links[parent].first_child = Some(index);
        }
    }
    bytes(records.capacity(), links.capacity(), k, allowance)?;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(k)
        .map_err(|_| SourceEligibilityError::Allocation)?;
    let peak_retained_bytes = bytes(
        records.capacity(),
        links.capacity(),
        ordered.capacity(),
        allowance,
    )?;
    let mut current = display.snapshot().construction().root();
    'visit: loop {
        if records[current].disposition == Some(Eligibility::Eligible) {
            let parent = nodes[current].parent().map(|parent| {
                records[parent]
                    .ordinal
                    .expect("eligible actual parent was emitted in preorder")
            });
            records[current].ordinal = Some(ordered.len());
            ordered.push(EligibleSource {
                source: current,
                parent,
            });
        }
        if let Some(child) = links[current].first_child {
            current = child;
            continue;
        }
        loop {
            // Complete this node once, after all its children; ascents do not revisit.
            if let Some(sibling) = links[current].next_sibling {
                current = sibling;
                continue 'visit;
            }
            if let Some(parent) = nodes[current].parent() {
                current = parent;
            } else {
                break 'visit;
            }
        }
    }
    drop(links);
    let retained_bytes = bytes(records.capacity(), 0, ordered.capacity(), allowance)?;
    Ok(SourceEligibility {
        display,
        records,
        ordered,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}
fn base_work(n: usize, allowance: NonZeroUsize) -> Result<usize, SourceEligibilityError> {
    let base = n
        .checked_mul(6)
        .and_then(|n| n.checked_add(1))
        .ok_or(SourceEligibilityError::Work)?;
    total_work(base, 0, allowance)
}
fn total_work(
    base: usize,
    k: usize,
    allowance: NonZeroUsize,
) -> Result<usize, SourceEligibilityError> {
    match base.checked_add(k) {
        Some(total) if total <= allowance.get() => Ok(total),
        _ => Err(SourceEligibilityError::Work),
    }
}
fn bytes(
    records: usize,
    links: usize,
    ordered: usize,
    allowance: NonZeroUsize,
) -> Result<usize, SourceEligibilityError> {
    let total = records
        .checked_mul(size_of::<Record>())
        .and_then(|r| {
            links
                .checked_mul(size_of::<Links>())
                .and_then(|l| r.checked_add(l))
        })
        .and_then(|rl| {
            ordered
                .checked_mul(size_of::<EligibleSource>())
                .and_then(|o| rl.checked_add(o))
        })
        .ok_or(SourceEligibilityError::Bytes)?;
    if total > allowance.get() || total > isize::MAX as usize {
        return Err(SourceEligibilityError::Bytes);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_work_rejects_multiplication_and_second_stage_overflow() {
        assert_eq!(
            base_work(usize::MAX, NonZeroUsize::MAX),
            Err(SourceEligibilityError::Work)
        );
        assert_eq!(
            base_work(usize::MAX / 6 + 1, NonZeroUsize::MAX),
            Err(SourceEligibilityError::Work)
        );
        assert_eq!(
            total_work(usize::MAX, 1, NonZeroUsize::MAX),
            Err(SourceEligibilityError::Work)
        );
        assert_eq!(total_work(31, 0, NonZeroUsize::new(31).unwrap()), Ok(31));
        assert_eq!(
            total_work(31, 5, NonZeroUsize::new(35).unwrap()),
            Err(SourceEligibilityError::Work)
        );
    }
    #[test]
    fn checked_payload_arithmetic_and_addressability() {
        for counts in [(usize::MAX, 0, 0), (0, usize::MAX, 0), (0, 0, usize::MAX)] {
            assert_eq!(
                bytes(counts.0, counts.1, counts.2, NonZeroUsize::MAX),
                Err(SourceEligibilityError::Bytes)
            );
        }
        let too_many = isize::MAX as usize / size_of::<Record>() + 1;
        assert_eq!(
            bytes(too_many, 0, 0, NonZeroUsize::MAX),
            Err(SourceEligibilityError::Bytes)
        );
        assert_eq!(bytes(0, 0, 0, NonZeroUsize::MAX), Ok(0));
        assert_eq!(
            bytes(1, 1, 1, NonZeroUsize::MAX),
            Ok(size_of::<Record>() + size_of::<Links>() + size_of::<EligibleSource>())
        );
    }
}
