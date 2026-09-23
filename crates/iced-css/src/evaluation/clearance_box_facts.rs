//! Bounded computed box facts from exact captured source and stylesheet owners.
//! This stage establishes neither used geometry nor a root formatting context.
use super::{
    clearance_profile::{ClearanceProjectionShape, ClearanceSlots},
    clearance_source_projection::ClearanceSourceProjection,
    source_display::SourceDisplay,
};
use crate::capture::source::ConstructionSourceError;
use iced_css_core::{
    cascade::{
        DefaultedValueRef, DefaultingError,
        rule::{RuleBatchError, RuleBatchLimits, RuleTarget, cascade_positioned_rules},
        winner_table::PropertyWinners,
    },
    formatting::{clearance::ClearSide, table_borders::BorderStyle},
    selectors::PseudoElement,
    source::selector_tree::SelectorTreeLimits,
    values::{
        LengthTarget, NumericLimits,
        content_computation::{
            ContentComputationError, ContentSelection, ContentTarget, select_content,
        },
        geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
        property::SpecifiedValue,
        registry::PropertyId,
    },
};
use std::num::NonZeroUsize;
mod resolve;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFactOrigin {
    Specified,
    Initial,
    /// Actual inheritance origin: source parent for elements, originating element for pseudos.
    Inherited {
        original_parent: usize,
    },
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsoluteBoxFact {
    pub pixels: f32,
    pub rounded_to_zero: bool,
    pub origin: BoxFactOrigin,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxLengthFact {
    Absolute(AbsoluteBoxFact),
    Auto { origin: BoxFactOrigin },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxOverflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordBoxFact<T> {
    pub value: T,
    pub origin: BoxFactOrigin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PseudoAbsence {
    pub origin: BoxFactOrigin,
}
/// Computed facts only. Construction guarantees min-width/min-height zero,
/// max-width/max-height none, and zero border widths from None/Hidden styles.
/// Edge arrays use top, right, bottom, left order. An inherited rounding flag
/// describes the original conversion; inheritance does not emit a new warning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceBoxFact {
    pub width: BoxLengthFact,
    pub height: BoxLengthFact,
    pub margin: [BoxLengthFact; 4],
    pub padding: [AbsoluteBoxFact; 4],
    pub clear: KeywordBoxFact<ClearSide>,
    pub overflow: KeywordBoxFact<BoxOverflow>,
    pub border_style: [KeywordBoxFact<BorderStyle>; 4],
    pub before: PseudoAbsence,
    pub after: PseudoAbsence,
}
impl ClearanceBoxFact {
    pub fn border_widths(&self) -> [f32; 4] {
        [0.; 4]
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ClearanceBoxFactsLimits {
    pub work: NonZeroUsize,
    pub rule_visits: NonZeroUsize,
    pub matching_work: NonZeroUsize,
    pub declaration_offers: NonZeroUsize,
    pub membership_work: NonZeroUsize,
    pub numeric_work: NonZeroUsize,
    pub selectors: SelectorTreeLimits,
    pub per_target: RuleBatchLimits,
    pub numeric: NumericLimits,
}
/// Conservative prepaid service allowances, not measured CPU work or scratch peak.
/// Only numeric_calls counts actual conversions. Temporary services may allocate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceBoxFactsReceipt {
    pub targets: usize,
    pub work: usize,
    pub rule_visits: usize,
    pub matching_work: usize,
    pub declaration_offers: usize,
    pub membership_work: usize,
    pub numeric_work: usize,
    pub numeric_calls: usize,
    pub owned_heap_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFactTarget {
    Element,
    Before,
    After,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFactUnsupported {
    RelativeLength,
    PercentageLength,
    AutoForProperty,
    NondefaultMinMax,
    BorderStyle,
    GeneratedContent,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ClearanceBoxFactsError {
    Work,
    SnapshotMismatch,
    Nodes,
    SourceJoin {
        projected: usize,
    },
    Rules,
    Declarations {
        rule: usize,
    },
    RuleVisits,
    MatchingWork,
    DeclarationOffers,
    MembershipWork,
    NumericWork,
    PropertyRegistry {
        name: &'static str,
    },
    Source(ConstructionSourceError),
    Cascade {
        original: usize,
        target: BoxFactTarget,
        error: RuleBatchError,
    },
    Defaulting {
        original: usize,
        property: PropertyId,
        error: DefaultingError,
    },
    InitialRecipe {
        property: PropertyId,
    },
    InvalidValue {
        original: usize,
        property: PropertyId,
    },
    Unsupported {
        original: usize,
        property: PropertyId,
        target: BoxFactTarget,
        reason: BoxFactUnsupported,
    },
    Length {
        original: usize,
        property: PropertyId,
        error: LengthEvaluationError,
    },
    Content {
        original: usize,
        target: BoxFactTarget,
        error: ContentComputationError,
    },
}
impl std::fmt::Display for ClearanceBoxFactsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance box facts: {self:?}")
    }
}
impl std::error::Error for ClearanceBoxFactsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(e) => Some(e),
            Self::Cascade { error, .. } => Some(error),
            Self::Defaulting { error, .. } => Some(error),
            Self::Length { error, .. } => Some(error),
            Self::Content { error, .. } => Some(error),
            _ => None,
        }
    }
}
#[must_use]
pub struct ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r> {
    projection: &'p ClearanceSourceProjection<'h>,
    display: &'d SourceDisplay<'s, 'b, 'r>,
    rows: ClearanceSlots<ClearanceBoxFact>,
    receipt: ClearanceBoxFactsReceipt,
}
impl<'p, 'h, 'd, 's, 'b, 'r> ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r> {
    pub fn projection(&self) -> &'p ClearanceSourceProjection<'h> {
        self.projection
    }
    pub fn display(&self) -> &'d SourceDisplay<'s, 'b, 'r> {
        self.display
    }
    pub fn row(&self, projected: usize) -> Option<&ClearanceBoxFact> {
        self.rows.as_slice().get(projected)
    }
    pub fn receipt(&self) -> ClearanceBoxFactsReceipt {
        self.receipt
    }
}
/// Recascade 12/15 actual element/pseudo targets into four/five immutable rows.
/// Fixed registry preparation is included in the initial registry-sized work unit.
/// Each source join and rule-shape visit is charged before inspection; the remaining
/// 26*N+1 facade units and aggregate reservations precede selector/service scratch.
/// # Errors
/// Returns named admission errors or source/property/target-specific service errors.
/// Failure publishes no partial batch and leaves both predecessors unchanged.
pub fn evaluate_clearance_box_facts<'p, 'h, 'd, 's, 'b, 'r>(
    projection: &'p ClearanceSourceProjection<'h>,
    display: &'d SourceDisplay<'s, 'b, 'r>,
    limits: ClearanceBoxFactsLimits,
) -> Result<ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>, ClearanceBoxFactsError> {
    use ClearanceBoxFactsError as E;
    let mut remaining = limits.work.get();
    charge(&mut remaining, 1)?;
    if !std::ptr::eq(projection.snapshot(), display.snapshot()) {
        return Err(E::SnapshotMismatch);
    }
    let snapshot = projection.snapshot();
    let nodes = snapshot.construction().nodes();
    let count = projection.nodes().len();
    if nodes.len() != count || display.node_count() != count || snapshot.tokens().len() != count {
        return Err(E::Nodes);
    }
    let mut originals = [0; 5];
    for (projected, original) in originals[..count].iter_mut().enumerate() {
        charge(&mut remaining, 1)?;
        *original = projection
            .original_index(projected)
            .ok_or(E::SourceJoin { projected })?;
        let joined = projection.projected_index(*original) == Some(projected)
            && projection
                .original_node(projected)
                .zip(nodes.get(*original))
                .is_some_and(|(a, b)| std::ptr::eq(a, b))
            && projection
                .token(projected)
                .zip(display.token(*original))
                .is_some_and(|(a, b)| std::ptr::eq(a, b))
            && display.resolution(*original).is_some();
        if !joined {
            return Err(E::SourceJoin { projected });
        }
    }
    let ids = resolve::property_ids()?;
    let bundle = display.bundle();
    let r = bundle.rule_count();
    if r > limits.per_target.rules.get() {
        return Err(E::Rules);
    }
    for (rule, offer) in bundle.offers().iter().enumerate() {
        charge(&mut remaining, 1)?;
        if offer.rule.declarations.len() > limits.per_target.per_rule.declarations.get() {
            return Err(E::Declarations { rule });
        }
    }
    charge(
        &mut remaining,
        count
            .checked_mul(26)
            .and_then(|n| n.checked_add(1))
            .ok_or(E::Work)?,
    )?;
    let targets = count.checked_mul(3).ok_or(E::Work)?;
    let rule_visits = product(targets, r, limits.rule_visits, E::RuleVisits)?;
    let matching_work = product(
        rule_visits,
        limits.per_target.per_rule.matching.work.get(),
        limits.matching_work,
        E::MatchingWork,
    )?;
    let declaration_offers = product(
        targets,
        bundle.declaration_count(),
        limits.declaration_offers,
        E::DeclarationOffers,
    )?;
    let authors = targets
        .checked_mul(bundle.author_rule_count())
        .ok_or(E::MembershipWork)?;
    let membership_work = product(
        authors,
        bundle.sheets().work_limit().get(),
        limits.membership_work,
        E::MembershipWork,
    )?;
    let numeric_work = product(
        count.checked_mul(14).ok_or(E::NumericWork)?,
        limits.numeric.work.get(),
        limits.numeric_work,
        E::NumericWork,
    )?;
    let mut receipt = ClearanceBoxFactsReceipt {
        targets,
        work: limits.work.get() - remaining,
        rule_visits,
        matching_work,
        declaration_offers,
        membership_work,
        numeric_work,
        numeric_calls: 0,
        owned_heap_bytes: 0,
    };
    let source = snapshot
        .selector_source(limits.selectors)
        .map_err(E::Source)?;
    // Match SourceDisplay's addressability cap: a larger caller allowance must
    // not reject a small admissible matcher stack.
    let mut per_target = limits.per_target;
    per_target.per_rule.matching.retained_bytes = per_target
        .per_rule
        .matching
        .retained_bytes
        .min(NonZeroUsize::new(isize::MAX as usize).expect("isize::MAX is positive"));
    let mut rows = match projection.shape() {
        ClearanceProjectionShape::RootRegion => ClearanceSlots::Four([resolve::initial_row(); 4]),
        ClearanceProjectionShape::NestedRegion => ClearanceSlots::Five([resolve::initial_row(); 5]),
    };
    for (projected, original) in originals[..count].iter().copied().enumerate() {
        let parent = nodes[original]
            .parent()
            .map(|index| {
                projection
                    .projected_index(index)
                    .filter(|&p| p < projected)
                    .map(|p| (index, rows.as_slice()[p]))
                    .ok_or(E::SourceJoin { projected })
            })
            .transpose()?;
        let mut winners = PropertyWinners::new();
        cascade_positioned_rules(
            &mut winners,
            bundle.offers().iter().copied(),
            RuleTarget {
                source: &source,
                node: original,
                pseudo: None,
            },
            bundle.sheets(),
            per_target,
        )
        .map_err(|error| E::Cascade {
            original,
            target: BoxFactTarget::Element,
            error,
        })?;
        let mut row = resolve::element(
            &winners,
            &ids,
            original,
            parent,
            limits.numeric,
            &mut receipt.numeric_calls,
        )?;
        for (target, pseudo) in [
            (BoxFactTarget::Before, PseudoElement::Before),
            (BoxFactTarget::After, PseudoElement::After),
        ] {
            let mut winners = PropertyWinners::new();
            cascade_positioned_rules(
                &mut winners,
                bundle.offers().iter().copied(),
                RuleTarget {
                    source: &source,
                    node: original,
                    pseudo: Some(pseudo),
                },
                bundle.sheets(),
                per_target,
            )
            .map_err(|error| E::Cascade {
                original,
                target,
                error,
            })?;
            let absence = resolve::pseudo(&winners, ids[20], original, target)?;
            match target {
                BoxFactTarget::Before => row.before = absence,
                BoxFactTarget::After => row.after = absence,
                BoxFactTarget::Element => unreachable!(),
            }
        }
        rows.as_mut_slice()[projected] = row;
    }
    Ok(ClearanceBoxFacts {
        projection,
        display,
        rows,
        receipt,
    })
}
fn charge(remaining: &mut usize, amount: usize) -> Result<(), ClearanceBoxFactsError> {
    *remaining = remaining
        .checked_sub(amount)
        .ok_or(ClearanceBoxFactsError::Work)?;
    Ok(())
}
fn product(
    left: usize,
    right: usize,
    allowance: NonZeroUsize,
    error: ClearanceBoxFactsError,
) -> Result<usize, ClearanceBoxFactsError> {
    match left.checked_mul(right) {
        Some(v) if v <= allowance.get() => Ok(v),
        _ => Err(error),
    }
}
