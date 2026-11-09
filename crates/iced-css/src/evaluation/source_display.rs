//! All-source display computation borrowing exact captured source and rule owners.
use super::display_rules::DisplayRuleBundle;
use crate::{
    capture::source::ConstructionSourceError, host::source_publication::InitialSourceSnapshot,
};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchError, RuleBatchLimits, RuleTarget, cascade_positioned_rules},
        winner_table::PropertyWinners,
    },
    formatting::display::DisplayResolution,
    identity::SourceRef,
    source::selector_tree::SelectorTreeLimits,
    values::{
        bidi::{ComputedBidiError, ComputedBidiStyle, compute_bidi_style},
        display::{ComputedDisplayError, compute_display_style},
        text_keywords::{ComputedTextKeywords, ComputedTextKeywordsError, compute_text_keywords},
    },
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct SourceDisplayLimits {
    pub nodes: NonZeroUsize,
    /// Actual output-vector capacity, separate from borrowed owners and scratch.
    pub retained_bytes: NonZeroUsize,
    /// Facade entries: admission + rule checks + slot initialization/display/text/bidi computation.
    /// Exactly 1+r+4*n for a successful evaluation.
    pub work: NonZeroUsize,
    /// Conservative n*r reservation, including excluded and unmatched rules.
    pub rule_visits: NonZeroUsize,
    /// Conservative n*r*m reservation using each rule's matcher allowance m.
    pub matching_work: NonZeroUsize,
    /// Conservative n*d declaration offers, with fixed registry-size offer work.
    pub declaration_offers: NonZeroUsize,
    /// Conservative n*a*s, using author rules a and controls' actual allowance s.
    pub membership_work: NonZeroUsize,
    /// One temporary captured selector view; never rebuilt per source target.
    pub selectors: SelectorTreeLimits,
    pub per_target: RuleBatchLimits,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SourceDisplayError {
    Nodes,
    Bytes,
    Allocation,
    Work,
    RuleVisits,
    MatchingWork,
    DeclarationOffers,
    MembershipWork,
    Rules,
    Declarations {
        rule: usize,
    },
    Source(ConstructionSourceError),
    Cascade {
        index: usize,
        error: RuleBatchError,
    },
    ComputedBidi {
        index: usize,
        error: ComputedBidiError,
    },
    ComputedText {
        index: usize,
        error: ComputedTextKeywordsError,
    },
    Computed {
        index: usize,
        error: ComputedDisplayError,
    },
}
impl std::fmt::Display for SourceDisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source display: {self:?}")
    }
}
impl std::error::Error for SourceDisplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Cascade { error, .. } => Some(error),
            Self::Computed { error, .. } => Some(error),
            Self::ComputedText { error, .. } => Some(error),
            Self::ComputedBidi { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceComputed {
    display: DisplayResolution,
    text: ComputedTextKeywords,
    bidi: ComputedBidiStyle,
}

/// Complete immutable display/position/float, text and bidi keywords in original source order.
/// This is source computation, not formatting suppression or native publication.
/// The exact snapshot and bundle remain borrowed, including all bundle owners.
///
/// ```compile_fail,E0505
/// use iced_css::{host::source_publication::InitialSourceHost,
///     evaluation::{display_rules::DisplayRuleBundle, source_display::*}};
/// fn outlive(host: InitialSourceHost, bundle: &DisplayRuleBundle<'_>, limits: SourceDisplayLimits) {
///     let result = evaluate_source_display(host.snapshot().unwrap(), bundle, limits).unwrap();
///     drop(host);
///     let _ = result.token(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css::{host::source_publication::InitialSourceSnapshot,
///     evaluation::{display_rules::DisplayRuleBundle, source_display::*}};
/// fn outlive(snapshot: &InitialSourceSnapshot, bundle: DisplayRuleBundle<'_>, limits: SourceDisplayLimits) {
///     let result = evaluate_source_display(snapshot, &bundle, limits).unwrap();
///     drop(bundle);
///     let _ = result.text_keywords(0);
/// }
/// ```
/// ```compile_fail,E0502
/// use iced_css::{host::source_publication::InitialSourceSnapshot,
///     evaluation::{display_rules::*, source_display::*}};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog};
/// fn mutate(snapshot: &InitialSourceSnapshot, ua: &UaDisplayCatalog,
///     sheets: &mut AuthorSheets, bundle_limits: BundleLimits, limits: SourceDisplayLimits) {
///     let bindings = [];
///     let bundle = DisplayRuleBundle::new(ua, &bindings, sheets, bundle_limits).unwrap();
///     let result = evaluate_source_display(snapshot, &bundle, limits).unwrap();
///     sheets.set_author_enabled(false).unwrap();
///     let _ = result.text_keywords(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css::{host::source_publication::InitialSourceHost,
///     evaluation::{display_rules::DisplayRuleBundle, source_display::*}};
/// fn outlive(host: InitialSourceHost, bundle: &DisplayRuleBundle<'_>, limits: SourceDisplayLimits) {
///     let result = evaluate_source_display(host.snapshot().unwrap(), bundle, limits).unwrap();
///     drop(host);
///     let _ = result.bidi_style(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css::{host::source_publication::InitialSourceSnapshot,
///     evaluation::{display_rules::DisplayRuleBundle, source_display::*}};
/// fn outlive(snapshot: &InitialSourceSnapshot, bundle: DisplayRuleBundle<'_>, limits: SourceDisplayLimits) {
///     let result = evaluate_source_display(snapshot, &bundle, limits).unwrap();
///     drop(bundle);
///     let _ = result.bidi_style(0);
/// }
/// ```
/// ```compile_fail,E0502
/// use iced_css::{host::source_publication::InitialSourceSnapshot,
///     evaluation::{display_rules::*, source_display::*}};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog};
/// fn mutate(snapshot: &InitialSourceSnapshot, ua: &UaDisplayCatalog,
///     sheets: &mut AuthorSheets, bundle_limits: BundleLimits, limits: SourceDisplayLimits) {
///     let bindings = [];
///     let bundle = DisplayRuleBundle::new(ua, &bindings, sheets, bundle_limits).unwrap();
///     let result = evaluate_source_display(snapshot, &bundle, limits).unwrap();
///     sheets.set_author_enabled(false).unwrap();
///     let _ = result.bidi_style(0);
/// }
/// ```
#[must_use]
pub struct SourceDisplay<'snapshot, 'bundle, 'rules> {
    snapshot: &'snapshot InitialSourceSnapshot,
    bundle: &'bundle DisplayRuleBundle<'rules>,
    resolutions: Vec<Option<SourceComputed>>,
    retained_bytes: usize,
    work: usize,
    rule_visits: usize,
    matching_work: usize,
    declaration_offers: usize,
    membership_work: usize,
}
impl<'snapshot, 'bundle, 'rules> SourceDisplay<'snapshot, 'bundle, 'rules> {
    pub fn snapshot(&self) -> &'snapshot InitialSourceSnapshot {
        self.snapshot
    }
    pub fn bundle(&self) -> &'bundle DisplayRuleBundle<'rules> {
        self.bundle
    }
    pub fn resolution(&self, index: usize) -> Option<&DisplayResolution> {
        self.resolutions
            .get(index)?
            .as_ref()
            .map(|value| &value.display)
    }
    /// Computed keywords for this original source, including suppressed descendants.
    pub fn text_keywords(&self, index: usize) -> Option<&ComputedTextKeywords> {
        self.resolutions
            .get(index)?
            .as_ref()
            .map(|value| &value.text)
    }
    /// Computed CSS bidi keywords at the original source slot, including suppressed sources.
    /// This does not identify a CSS paragraph or its containing block.
    pub fn bidi_style(&self, index: usize) -> Option<&ComputedBidiStyle> {
        self.resolutions
            .get(index)?
            .as_ref()
            .map(|value| &value.bidi)
    }
    /// Exact snapshot token at this original construction index; no token search.
    pub fn token(&self, index: usize) -> Option<&SourceRef> {
        self.snapshot.token(index)
    }
    pub fn node_count(&self) -> usize {
        self.resolutions.len()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn rule_visits(&self) -> usize {
        self.rule_visits
    }
    pub fn matching_work(&self) -> usize {
        self.matching_work
    }
    pub fn declaration_offers(&self) -> usize {
        self.declaration_offers
    }
    pub fn membership_work(&self) -> usize {
        self.membership_work
    }
}

/// Compute every source record, including descendants of display:none.
/// Parents are computed first by reversing Construction's private postorder.
/// Explicit inheritance therefore consumes the actual source parent's computed
/// values, with root transformation only for the single parentless source.
///
/// Aggregate work fields reserve conservative structural bounds, not measured
/// matching consumption or CPU instructions. Computation and winner copies each
/// retain their existing fixed property-registry bound. Captured selector indexing,
/// sequential matcher allocation/growth and borrowed owner memory are separate
/// accounts; retained_bytes is not a whole-process or exact scratch-peak bound.
///
/// # Errors
/// Rejects named resource admission before computation, or reports the original
/// index and cascade/computed error. No partially filled candidate escapes and
/// all borrowed owners remain unchanged. Allocation is fallible; a larger byte
/// allowance alone does not reject an otherwise admissible small candidate.
pub fn evaluate_source_display<'snapshot, 'bundle, 'rules>(
    snapshot: &'snapshot InitialSourceSnapshot,
    bundle: &'bundle DisplayRuleBundle<'rules>,
    limits: SourceDisplayLimits,
) -> Result<SourceDisplay<'snapshot, 'bundle, 'rules>, SourceDisplayError> {
    let mut remaining = limits.work.get();
    charge(&mut remaining, 1)?;
    let nodes = snapshot.construction().nodes();
    let n = nodes.len();
    if n > limits.nodes.get() {
        return Err(SourceDisplayError::Nodes);
    }
    let r = bundle.rule_count();
    if r > limits.per_target.rules.get() {
        return Err(SourceDisplayError::Rules);
    }
    for (rule, offer) in bundle.offers().iter().enumerate() {
        charge(&mut remaining, 1)?;
        if offer.rule.declarations.len() > limits.per_target.per_rule.declarations.get() {
            return Err(SourceDisplayError::Declarations { rule });
        }
    }
    let rule_visits = product(n, r, limits.rule_visits, SourceDisplayError::RuleVisits)?;
    let matching_work = product(
        rule_visits,
        limits.per_target.per_rule.matching.work.get(),
        limits.matching_work,
        SourceDisplayError::MatchingWork,
    )?;
    let declaration_offers = product(
        n,
        bundle.declaration_count(),
        limits.declaration_offers,
        SourceDisplayError::DeclarationOffers,
    )?;
    let author_visits = n
        .checked_mul(bundle.author_rule_count())
        .ok_or(SourceDisplayError::MembershipWork)?;
    let membership_work = product(
        author_visits,
        bundle.sheets().work_limit().get(),
        limits.membership_work,
        SourceDisplayError::MembershipWork,
    )?;
    charge(
        &mut remaining,
        n.checked_mul(4).ok_or(SourceDisplayError::Work)?,
    )?;
    let work = limits.work.get() - remaining;
    let (mut resolutions, retained_bytes) = output(n, limits.retained_bytes)?;
    let source = snapshot
        .selector_source(limits.selectors)
        .map_err(SourceDisplayError::Source)?;
    let mut per_target = limits.per_target;
    per_target.per_rule.matching.retained_bytes = per_target
        .per_rule
        .matching
        .retained_bytes
        .min(NonZeroUsize::new(isize::MAX as usize).expect("isize::MAX is positive"));
    for index in (0..n).rev() {
        let mut winners = PropertyWinners::new();
        cascade_positioned_rules(
            &mut winners,
            bundle.offers_for(&source, index),
            RuleTarget {
                source: &source,
                node: index,
                pseudo: None,
            },
            bundle.sheets(),
            per_target,
        )
        .map_err(|error| SourceDisplayError::Cascade { index, error })?;
        let parent = nodes[index].parent().map(|parent| {
            resolutions[parent]
                .as_ref()
                .expect("private postorder source parent is already computed")
        });
        let display = compute_display_style(&winners, parent.map(|value| &value.display))
            .map_err(|error| SourceDisplayError::Computed { index, error })?;
        let text = compute_text_keywords(&winners, parent.map(|value| &value.text))
            .map_err(|error| SourceDisplayError::ComputedText { index, error })?;
        let bidi = compute_bidi_style(&winners, parent.map(|value| &value.bidi))
            .map_err(|error| SourceDisplayError::ComputedBidi { index, error })?;
        resolutions[index] = Some(SourceComputed {
            display,
            text,
            bidi,
        });
    }
    Ok(SourceDisplay {
        snapshot,
        bundle,
        resolutions,
        retained_bytes,
        work,
        rule_visits,
        matching_work,
        declaration_offers,
        membership_work,
    })
}

fn charge(remaining: &mut usize, amount: usize) -> Result<(), SourceDisplayError> {
    *remaining = remaining
        .checked_sub(amount)
        .ok_or(SourceDisplayError::Work)?;
    Ok(())
}
fn product(
    left: usize,
    right: usize,
    allowance: NonZeroUsize,
    error: SourceDisplayError,
) -> Result<usize, SourceDisplayError> {
    match left.checked_mul(right) {
        Some(total) if total <= allowance.get() => Ok(total),
        _ => Err(error),
    }
}
fn output(
    count: usize,
    allowance: NonZeroUsize,
) -> Result<(Vec<Option<SourceComputed>>, usize), SourceDisplayError> {
    output_bytes(count, allowance)?;
    let mut resolutions = Vec::new();
    resolutions
        .try_reserve_exact(count)
        .map_err(|_| SourceDisplayError::Allocation)?;
    let bytes = output_bytes(resolutions.capacity(), allowance)?;
    resolutions.resize(count, None);
    Ok((resolutions, bytes))
}
fn output_bytes(capacity: usize, allowance: NonZeroUsize) -> Result<usize, SourceDisplayError> {
    let bytes = capacity
        .checked_mul(size_of::<Option<SourceComputed>>())
        .ok_or(SourceDisplayError::Bytes)?;
    if bytes > allowance.get() || bytes > isize::MAX as usize {
        return Err(SourceDisplayError::Bytes);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_account_uses_actual_capacity_and_checked_addressability() {
        let ample = NonZeroUsize::MAX;
        for count in [0, 1, 7] {
            let (values, bytes) = output(count, ample).unwrap();
            assert_eq!(values.len(), count);
            assert_eq!(
                bytes,
                values.capacity() * size_of::<Option<SourceComputed>>()
            );
            assert!(values.iter().all(Option::is_none));
        }
        assert_eq!(
            output_bytes(usize::MAX, ample),
            Err(SourceDisplayError::Bytes)
        );
        let too_many = (isize::MAX as usize / size_of::<Option<SourceComputed>>()) + 1;
        assert_eq!(
            output_bytes(too_many, ample),
            Err(SourceDisplayError::Bytes)
        );
    }
    #[test]
    fn synthetic_account_overflow_and_zero_have_named_results() {
        let ample = NonZeroUsize::MAX;
        assert_eq!(
            product(usize::MAX, 2, ample, SourceDisplayError::RuleVisits),
            Err(SourceDisplayError::RuleVisits)
        );
        assert_eq!(
            product(usize::MAX, 2, ample, SourceDisplayError::DeclarationOffers),
            Err(SourceDisplayError::DeclarationOffers)
        );
        assert_eq!(
            product(0, usize::MAX, ample, SourceDisplayError::MembershipWork),
            Ok(0)
        );
        let mut remaining = 1;
        assert_eq!(charge(&mut remaining, 2), Err(SourceDisplayError::Work));
        assert_eq!(remaining, 1);
    }
}
