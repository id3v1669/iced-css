//! Bounded computed box facts from exact captured source and stylesheet owners.
//! This stage establishes neither used geometry nor a root formatting context.
use super::{
    clearance_profile::{ClearanceProjectionShape, ClearanceSlots, ProjectionView},
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
    formatting::{clearance::ClearSide, display::Position, table_borders::BorderStyle},
    selectors::PseudoElement,
    source::selector_tree::SelectorTreeLimits,
    values::{
        CompactDecimal, LengthTarget, NumericLimits, RangeEvent,
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
/// A computed percentage (PCT): the authored value exactly, as a bounded `Copy`
/// decimal, and its origin (CSS2.2 6.1.2: the computed value is the percentage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PercentageBoxFact {
    pub value: CompactDecimal,
    pub origin: BoxFactOrigin,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxLengthFact {
    Absolute(AbsoluteBoxFact),
    Auto {
        origin: BoxFactOrigin,
    },
    /// PCT: a computed percentage of the containing block's width; see the
    /// `ClearanceBoxFact` contract for the rows and longhands that may hold it.
    Percentage(PercentageBoxFact),
}
/// A computed `max-width` or `max-height` (MMX): an absolute length, or `none`; a
/// Sequence `max-width` may also be a percentage (PPM).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxMaxFact {
    Absolute(AbsoluteBoxFact),
    None {
        origin: BoxFactOrigin,
    },
    /// PPM: a computed `max-width` percentage of the containing block's width (CSS2.2
    /// 10.4); `max_height` never holds it.
    Percentage(PercentageBoxFact),
}
/// A computed `<length> | <percentage>` (PPM): the four paddings and `min-width`
/// (CSS2.2 8.4, 10.4). Only a Sequence row may hold a percentage; see the
/// `ClearanceBoxFact` contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentageBoxFact {
    Absolute(AbsoluteBoxFact),
    Percentage(PercentageBoxFact),
}
impl LengthPercentageBoxFact {
    /// The absolute fact, or `None` for a percentage.
    pub fn absolute(self) -> Option<AbsoluteBoxFact> {
        match self {
            Self::Absolute(value) => Some(value),
            Self::Percentage(_) => None,
        }
    }
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
/// One computed `border-*-width` side. `pixels` is finite and non-negative, and +0
/// when zero. `event` is the range event of the conversion that produced the value:
/// upper saturation to `f32::MAX`, or a positive length rounded to +0. An inherited
/// fact keeps the parent's pixels and event, as an inherited rounding flag does; this
/// deliberately differs from `ComputedBorderWidth::sample`, which drops inherited
/// events. This layer delivers no range diagnostic; a later delivery reports only
/// facts whose origin is `Specified`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderWidthFact {
    pub pixels: f32,
    pub event: Option<RangeEvent>,
    pub origin: BoxFactOrigin,
}
/// Computed facts only. Edge arrays use top, right, bottom, left order. An inherited
/// rounding flag or range event describes the original conversion; inheritance does not
/// emit a new warning.
///
/// Min/max facts (MMX) depend on the entry. Legacy construction refuses every
/// nondefault `min-*`/`max-*` value and never stores one: every Legacy row carries
/// `min_width`/`min_height` (+0, `Initial`) and `max_width`/`max_height`
/// `None { Initial }`, and that `origin` is not provenance. A Sequence row carries the
/// computed constraints with true provenance (CSS2.2 10.4, 10.7): an absolute
/// nonnegative length converted once (zero is +0), `none` for a max, or the parent
/// row's fact for `inherit`; a `min-width` or `max-width` percentage is kept exactly
/// (PPM, see below), while height-constraint percentages and `em`/`ex` stay refused.
///
/// Border facts depend on the entry. `evaluate_clearance_box_facts` (Legacy) admits
/// only `none`/`hidden` styles and never computes a width: every Legacy width fact is
/// (+0, no event, `Initial`), and that `origin` is not provenance, so Legacy
/// `border_widths()` is zero. `evaluate_sibling_box_facts` (Sequence) admits every
/// CSS2.2 style and computes each side (CSS2.2 8.5.1): +0 without conversion when the
/// side's own style is `none` or `hidden`; otherwise `thin`/`medium`/`thick` are
/// 1/3/5, an absolute length is the nearest f32 (ties to even, saturating at
/// `f32::MAX`), and `inherit` copies the parent's computed fact. Sequence width facts
/// carry true provenance.
///
/// `offset` (REL) holds the computed `top`, `right`, `bottom` and `left` of a Sequence
/// row whose computed `position` is `relative`: `auto`, or an absolute length
/// converted like a signed margin (zero is +0), or the parent row's offset for
/// `inherit`. It is `None` for every other row (CSS2.2 9.3.1: the offsets do not apply
/// to a static box; absolute and fixed boxes are refused later) and for every Legacy
/// row, whose offsets are never resolved.
///
/// Percentage facts (PCT, PPM) depend on the entry. A Legacy row never holds
/// `BoxLengthFact::Percentage`, `LengthPercentageBoxFact::Percentage` or
/// `BoxMaxFact::Percentage`: `evaluate_clearance_box_facts` refuses every percentage
/// with `PercentageLength`. A Sequence row may hold one only in `width`, `margin`,
/// `padding`, `min_width`, `max_width` and, on a relatively positioned row, `offset[1]`
/// (`right`) and `offset[3]` (`left`): the authored percentage kept exactly (CSS2.2
/// 10.2, 8.3, 8.4, 10.4, 9.3.2), or the parent row's fact for `inherit`. It is resolved
/// later against the containing block's width; percentages of every other longhand
/// stay refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceBoxFact {
    pub width: BoxLengthFact,
    pub height: BoxLengthFact,
    /// Computed `min-width`, `min-height`, `max-width` and `max-height`; see the type
    /// contract for Legacy and Sequence rows.
    pub min_width: LengthPercentageBoxFact,
    pub min_height: AbsoluteBoxFact,
    pub max_width: BoxMaxFact,
    pub max_height: BoxMaxFact,
    pub margin: [BoxLengthFact; 4],
    pub padding: [LengthPercentageBoxFact; 4],
    pub clear: KeywordBoxFact<ClearSide>,
    pub overflow: KeywordBoxFact<BoxOverflow>,
    pub border_style: [KeywordBoxFact<BorderStyle>; 4],
    /// Computed border widths; see the type contract for Legacy and Sequence rows.
    pub border_width: [BorderWidthFact; 4],
    pub before: PseudoAbsence,
    pub after: PseudoAbsence,
    /// Computed offsets of a relatively positioned Sequence row; see the type contract.
    pub offset: Option<[BoxLengthFact; 4]>,
}
impl ClearanceBoxFact {
    /// Computed border widths in pixels, top, right, bottom, left.
    pub fn border_widths(&self) -> [f32; 4] {
        self.border_width.map(|width| width.pixels)
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
    /// Legacy only: `evaluate_clearance_box_facts` refuses every nondefault
    /// `min-*`/`max-*`; the Sequence entry stores them.
    NondefaultMinMax,
    BorderStyle,
    GeneratedContent,
    /// REL: `inherit` on an offset of a relatively positioned row whose parent row is
    /// not relatively positioned. CSS2.2 9.3.2 defines a computed value there; this is
    /// a supported-subset refusal, because only relative rows resolve their offsets.
    InheritedOffset,
    /// PCT: an admitted percentage whose canonical coefficient exceeds `u64::MAX` or
    /// whose scale exceeds `u32::MAX` (the bounded `Copy` representation).
    PercentagePrecision,
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
    let mut prepared = prepare_box_facts(projection, display, limits, BoxFactMode::Legacy)?;
    let mut rows = match projection.shape() {
        ClearanceProjectionShape::RootRegion => ClearanceSlots::Four([resolve::initial_row(); 4]),
        ClearanceProjectionShape::NestedRegion => ClearanceSlots::Five([resolve::initial_row(); 5]),
    };
    evaluate_box_rows(
        projection,
        display,
        limits,
        &mut prepared,
        rows.as_mut_slice(),
    )?;
    Ok(ClearanceBoxFacts {
        projection,
        display,
        rows,
        receipt: prepared.receipt,
    })
}

/// Fact mode, bound once by each public entry and carried in `PreparedBoxFacts`, so
/// that the work charge, the numeric allowance and the resolved longhands agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoxFactMode {
    /// `evaluate_clearance_box_facts`: indices 0-19, `none`/`hidden` styles only,
    /// width facts never computed.
    Legacy,
    /// `evaluate_sibling_box_facts`: indices 0-23, every style, computed widths, and
    /// the offsets 25-28 of relatively positioned rows (REL).
    Sequence,
}

pub(super) struct PreparedBoxFacts {
    pub(super) ids: [PropertyId; 29],
    pub(super) receipt: ClearanceBoxFactsReceipt,
    pub(super) per_target: RuleBatchLimits,
    /// Private to this module: the Sequence entry can neither build nor change it.
    mode: BoxFactMode,
}

pub(super) fn initial_box_fact() -> ClearanceBoxFact {
    resolve::initial_row()
}

pub(super) fn prepare_box_facts<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    limits: ClearanceBoxFactsLimits,
    mode: BoxFactMode,
) -> Result<PreparedBoxFacts, ClearanceBoxFactsError> {
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
    for projected in 0..count {
        charge(&mut remaining, 1)?;
        let original = projection
            .original_index(projected)
            .ok_or(E::SourceJoin { projected })?;
        let joined = projection.projected_index(original) == Some(projected)
            && projection
                .original_node(projected)
                .zip(nodes.get(original))
                .is_some_and(|(a, b)| std::ptr::eq(a, b))
            && projection
                .token(projected)
                .zip(display.token(original))
                .is_some_and(|(a, b)| std::ptr::eq(a, b))
            && display.resolution(original).is_some();
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
    // Per node: one row, three target dispatches, one unit per resolved longhand (20
    // Legacy, 28 Sequence with the four REL offsets) and two pseudos; plus one. The
    // numeric allowance reserves one conversion per length-valued longhand: 14 Legacy,
    // 22 Sequence (four widths, four offsets). Both are charged on every path of the
    // mode, static rows included, without credit.
    let (per_node, numeric_per_node) = match mode {
        BoxFactMode::Legacy => (26, 14),
        BoxFactMode::Sequence => (34, 22),
    };
    charge(
        &mut remaining,
        count
            .checked_mul(per_node)
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
        count.checked_mul(numeric_per_node).ok_or(E::NumericWork)?,
        limits.numeric.work.get(),
        limits.numeric_work,
        E::NumericWork,
    )?;
    let receipt = ClearanceBoxFactsReceipt {
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
    // Match SourceDisplay's addressability cap: a larger caller allowance must
    // not reject a small admissible matcher stack.
    let mut per_target = limits.per_target;
    per_target.per_rule.matching.retained_bytes = per_target
        .per_rule
        .matching
        .retained_bytes
        .min(NonZeroUsize::new(isize::MAX as usize).expect("isize::MAX is positive"));
    Ok(PreparedBoxFacts {
        ids,
        receipt,
        per_target,
        mode,
    })
}

pub(super) fn evaluate_box_rows<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    limits: ClearanceBoxFactsLimits,
    prepared: &mut PreparedBoxFacts,
    rows: &mut [ClearanceBoxFact],
) -> Result<(), ClearanceBoxFactsError> {
    use ClearanceBoxFactsError as E;
    // The mode bound by `prepare_box_facts` selects both the charge and the loop.
    let mode = prepared.mode;
    let per_target = prepared.per_target;
    let ids = &prepared.ids;
    let receipt = &mut prepared.receipt;
    let snapshot = projection.snapshot();
    let nodes = snapshot.construction().nodes();
    let count = projection.nodes().len();
    if rows.len() != count {
        return Err(E::Nodes);
    }
    let bundle = display.bundle();
    let source = snapshot
        .selector_source(limits.selectors)
        .map_err(E::Source)?;
    for projected in 0..count {
        let original = projection
            .original_index(projected)
            .ok_or(E::SourceJoin { projected })?;
        let parent = nodes[original]
            .parent()
            .map(|index| {
                projection
                    .projected_index(index)
                    .filter(|&p| p < projected)
                    .map(|p| (index, rows[p]))
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
            ids,
            original,
            parent,
            limits.numeric,
            mode,
            &mut receipt.numeric_calls,
        )?;
        // REL (CSS2.2 9.3.2, 9.4.3): only a Sequence row whose computed `position` is
        // `relative` resolves its offsets, after its other longhands and before its
        // pseudos; the offsets do not apply to any other row (9.3.1). Only the Sequence
        // entry reads the position (MMX K-2), which `prepare_box_facts` proves present;
        // an absence is the existing join error.
        if mode == BoxFactMode::Sequence {
            let position = display
                .resolution(original)
                .ok_or(E::SourceJoin { projected })?
                .position;
            if position == Position::Relative {
                row.offset = Some(resolve::offsets(
                    &winners,
                    ids,
                    original,
                    parent,
                    limits.numeric,
                    &mut receipt.numeric_calls,
                )?);
            }
        }
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
            let absence = resolve::pseudo(&winners, ids[24], original, target)?;
            match target {
                BoxFactTarget::Before => row.before = absence,
                BoxFactTarget::After => row.after = absence,
                BoxFactTarget::Element => unreachable!(),
            }
        }
        rows[projected] = row;
    }
    Ok(())
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
