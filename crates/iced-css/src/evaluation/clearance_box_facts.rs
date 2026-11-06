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
        CompactDecimal, LengthTarget, NumericLimits, RangeEvent, RelativeUnit,
        content_computation::{
            ContentComputationError, ContentSelection, ContentTarget, select_content,
        },
        font_size::{FontSizeContext, FontSizeError, FontSizeTable, compute_font_size},
        geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
        property::SpecifiedValue,
        registry::PropertyId,
    },
};
use std::num::NonZeroUsize;
mod resolve;
pub(super) use resolve::fallback_x_height;

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
    /// PCT: a computed percentage of the containing block's width (PHT: of its height,
    /// for `height`; PTB: for `top` and `bottom`); see the `ClearanceBoxFact` contract for
    /// the rows and longhands that may hold it.
    Percentage(PercentageBoxFact),
}
/// A computed `max-width` or `max-height` (MMX): an absolute length, or `none`; a
/// Sequence `max-width` (PPM) or `max-height` (PHT) may also be a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxMaxFact {
    Absolute(AbsoluteBoxFact),
    None {
        origin: BoxFactOrigin,
    },
    /// PPM: a computed `max-width` percentage of the containing block's width (CSS2.2
    /// 10.4); a Sequence `max_height` may hold one too (PHT, CSS2.2 10.7), a percentage
    /// of the containing block's height: the field gives its axis.
    Percentage(PercentageBoxFact),
}
/// A computed `<length> | <percentage>` (PPM): the four paddings and `min-width`
/// (CSS2.2 8.4, 10.4), and `min-height` (PHT, 10.7). Only a Sequence row may hold a
/// percentage; see the `ClearanceBoxFact` contract.
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
/// EML (CSS2.2 15.7, 6.1.2): one computed `font-size` in pixels, finite and non-negative
/// and +0 when zero: a keyword table entry, the parent's fact for inheritance, one
/// nearest/ties-even conversion of a length, `em`, `ex` (EXF, the user's U5=B: against the
/// parent's 0.5em x-height) or percentage, saturating at `f32::MAX`, or `larger` or
/// `smaller` through the relative mapping against the parent's
/// size (architecture 29), an interpolation of several f32 steps that is neither a table
/// entry nor one conversion and can saturate. `event` is the range event of the
/// computation (upper saturation, or a positive size rounded to +0); an inherited fact
/// keeps the parent's pixels and event, as an inherited border width does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSizeFact {
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
/// row's fact for `inherit`; a `min-width` or `max-width` percentage (PPM) and a
/// `min-height` or `max-height` percentage (PHT) are kept exactly (see below), and an
/// `em` length of a row with a computed `font_size` (EML), and (EXF, the user's U5=B) an
/// `ex` length against its 0.5em x-height, is converted once like an absolute one, while
/// `em` and `ex` on any other row stay refused.
///
/// Border facts depend on the entry. `evaluate_clearance_box_facts` (Legacy) admits
/// only `none`/`hidden` styles and never computes a width: every Legacy width fact is
/// (+0, no event, `Initial`), and that `origin` is not provenance, so Legacy
/// `border_widths()` is zero. `evaluate_sibling_box_facts` (Sequence) admits every
/// CSS2.2 style and computes each side (CSS2.2 8.5.1): +0 without conversion when the
/// side's own style is `none` or `hidden`; otherwise `thin`/`medium`/`thick` are
/// 1/3/5, an absolute length, or (EML) an `em` length against the row's computed
/// `font_size` or (EXF, the user's U5=B) an `ex` length against its 0.5em x-height, is
/// the nearest f32 (ties to even, saturating at `f32::MAX`), and
/// `inherit` copies the parent's computed fact. Sequence width facts carry true
/// provenance.
///
/// `offset` (REL) holds the computed `top`, `right`, `bottom` and `left` of a Sequence
/// row whose computed `position` is `relative`: `auto`, or an absolute length
/// converted like a signed margin (zero is +0), or a percentage (PCT: `right`/`left`;
/// PTB: `top`/`bottom`), or, for `inherit`, the parent row's computed offset: its
/// `offset` when the parent row is relatively positioned; otherwise (ISP; CSS2.2 6.2.1,
/// 6.1.2: a computed value exists even where the property does not apply) the parent's
/// own computed value: `auto` (initial or explicit), or its length or percentage computed
/// against the parent's own resolver (a percentage validated and kept; an `em` against the
/// parent's font size, an `ex` (EXF, the user's U5=B) against its 0.5em x-height), or,
/// when the parent's own side is `inherit`, O's computed offset (its fact when O is
/// relative, otherwise computed in the same way; O's `inherit` is the initial `auto`). It
/// is `None` for every other row (CSS2.2 9.3.1: the offsets do not apply to a static box;
/// absolute and fixed boxes are refused later) and for every Legacy row, whose offsets are
/// never resolved.
///
/// `font_size` (EML) is the computed `font-size` (CSS2.2 15.7) of every row from
/// `evaluate_sibling_box_facts_with_font_medium`, computed before the row's other
/// longhands, and `None` for every other row (`evaluate_sibling_box_facts`, Legacy),
/// whose `em` and `ex` lengths stay refused. An `em` in a length longhand of a row with a
/// computed `font_size` (a width, height, constraint, margin, padding, border width or
/// offset) is that size times the authored number, one exact product rounded once (CSS2.2
/// 6.1.2: `em` computes to an absolute length), so `inherit` copies the parent's absolute
/// fact. An `ex` (EXF, the user's U5=B) is the same product with the row's 0.5em x-height
/// (CSS2.2 4.3.2: no font metrics are read here), and `ex` in `font-size` uses the
/// parent's (the root's: `medium`'s).
///
/// Percentage facts (PCT, PPM, PHT, PTB) depend on the entry. A Legacy row never holds
/// `BoxLengthFact::Percentage`, `LengthPercentageBoxFact::Percentage` or
/// `BoxMaxFact::Percentage`: `evaluate_clearance_box_facts` refuses every percentage
/// with `PercentageLength`. A Sequence row may hold one only in `width`, `height`,
/// `margin`, `padding`, `min_width`, `min_height`, `max_width`, `max_height` and, on a
/// relatively positioned row, `offset`: the authored percentage kept exactly (CSS2.2
/// 10.2, 10.5, 8.3, 8.4, 10.4, 10.7, 9.3.2), or the parent row's fact for `inherit`. A
/// `height`, `min_height` or `max_height` percentage is resolved later against the
/// containing block's height when that height is specified explicitly; otherwise it
/// takes the CSS2.2 10.5/10.7 rule (`auto`, 0 or `none`). A child's `top` or `bottom`
/// percentage (PTB, PTA) resolves against P's used content height, explicit or Auto
/// (CSS2.2 9.3.2, 9.4.3: offsets apply after layout). P's `height`, `min-height`,
/// `max-height`, `top` and `bottom` percentages under an explicit O resolve against O's
/// used content height when the host supplies it
/// (`build_captured_sibling_layout_with_outer_height`, OCH) and are refused
/// (`OuterHeightBasis`) otherwise; P's `top` and `bottom` percentages under an `auto` O
/// resolve in the facade against O's used content height (OAH) or are refused
/// (`IndefiniteOffsetBasis`, when O's `min-height` or `max-height` is a percentage and
/// the host supplies no initial containing block height; VPH, the user's U2=A).
/// Every other one is resolved against the containing block's width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceBoxFact {
    pub width: BoxLengthFact,
    pub height: BoxLengthFact,
    /// Computed `min-width`, `min-height`, `max-width` and `max-height`; see the type
    /// contract for Legacy and Sequence rows.
    pub min_width: LengthPercentageBoxFact,
    pub min_height: LengthPercentageBoxFact,
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
    /// Computed `font-size` of a row of the font entry (EML); see the type contract.
    pub font_size: Option<FontSizeFact>,
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
    /// REL, ISP: `inherit` on an offset of a relatively positioned row whose parent row
    /// is not relatively positioned and whose computed offset on that side is outside the
    /// supported subset: `em` or `ex` without the font entry, an unrepresentable length, a
    /// percentage outside the bounded representation, or a defaulting or initial-recipe
    /// failure (reachable only through a property-table invariant), also through a static P
    /// from O. CSS2.2 6.2.1 defines the value; this is a supported-subset refusal.
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
    /// EML: the `font-size` computation of the element at `original` failed (font entry
    /// only).
    FontSize {
        original: usize,
        error: FontSizeError,
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
            Self::FontSize { error, .. } => Some(error),
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
    let mut prepared = prepare_box_facts(projection, display, limits, BoxFactMode::Legacy, None)?;
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
    /// Whole-tree layout places absolute boxes; the sibling facade refuses them unread.
    pub(super) absolute_offsets: bool,
    pub(super) ids: [PropertyId; 29],
    pub(super) receipt: ClearanceBoxFactsReceipt,
    pub(super) per_target: RuleBatchLimits,
    /// Private to this module: the Sequence entry can neither build nor change it.
    mode: BoxFactMode,
    /// EML: the font entry's keyword table and the `font-size` id, bound with the mode;
    /// `None` for the other entries.
    font: Option<FontContext>,
}

/// EML: the keyword table built from the font entry's `medium` (architecture 27) and the
/// `font-size` property, looked up outside the 29 captured longhands (whose indices 25-28
/// are the offsets).
#[derive(Debug, Clone, Copy)]
pub(super) struct FontContext {
    table: FontSizeTable,
    property: PropertyId,
}

pub(super) fn initial_box_fact() -> ClearanceBoxFact {
    resolve::initial_row()
}

pub(super) fn prepare_box_facts<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    limits: ClearanceBoxFactsLimits,
    mode: BoxFactMode,
    font: Option<FontSizeTable>,
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
    let font = match font {
        Some(table) => Some(FontContext {
            table,
            property: PropertyId::lookup("font-size")
                .ok_or(E::PropertyRegistry { name: "font-size" })?,
        }),
        None => None,
    };
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
    // mode, static rows included, without credit. EML: the font entry also resolves
    // `font-size` on every row (one unit) and reserves one conversion for it.
    let (per_node, numeric_per_node) = match (mode, font.is_some()) {
        (BoxFactMode::Legacy, _) => (26, 14),
        (BoxFactMode::Sequence, false) => (34, 22),
        (BoxFactMode::Sequence, true) => (35, 23),
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
        absolute_offsets: false,
        ids,
        receipt,
        per_target,
        mode,
        font,
    })
}

pub(super) trait BoxRowObserver<'r> {
    fn observe(
        &mut self,
        original: usize,
        winners: &PropertyWinners<'r>,
        row: &ClearanceBoxFact,
        ids: &[PropertyId; 29],
    );
}

struct NoBorderObserver;
impl<'r> BoxRowObserver<'r> for NoBorderObserver {
    fn observe(
        &mut self,
        _original: usize,
        _winners: &PropertyWinners<'r>,
        _row: &ClearanceBoxFact,
        _ids: &[PropertyId; 29],
    ) {
    }
}

pub(super) fn evaluate_box_rows<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    limits: ClearanceBoxFactsLimits,
    prepared: &mut PreparedBoxFacts,
    rows: &mut [ClearanceBoxFact],
) -> Result<(), ClearanceBoxFactsError> {
    evaluate_box_rows_observed(
        projection,
        display,
        limits,
        prepared,
        rows,
        &mut NoBorderObserver,
    )
}

pub(super) fn evaluate_box_rows_observed<'h, 'r>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, 'r>,
    limits: ClearanceBoxFactsLimits,
    prepared: &mut PreparedBoxFacts,
    rows: &mut [ClearanceBoxFact],
    observer: &mut impl BoxRowObserver<'r>,
) -> Result<(), ClearanceBoxFactsError> {
    use ClearanceBoxFactsError as E;
    // The mode bound by `prepare_box_facts` selects both the charge and the loop.
    let mode = prepared.mode;
    let absolute_offsets = prepared.absolute_offsets;
    let font = prepared.font;
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
    // ISP (CSS2.2 6.2.1): the offsets of an O or P that is not relatively positioned
    // (static, or absolute or fixed and refused later by the facade; projected 0 and 1, the
    // only rows with children in the sibling projection) for a relative child's `inherit`;
    // Sequence only, never published or converted here.
    let mut statics: [Option<resolve::StaticOffsets<'_>>; 2] = [None; 2];
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
        // EML (CSS2.2 15.7, 6.1.2): the font entry computes the row's `font-size` first;
        // its other longhands and its offsets resolve an `em` against it and (EXF, the
        // user's U5=B) an `ex` against its 0.5em x-height.
        let font_size = match font {
            Some(font) => Some(resolve::font_size(
                &winners,
                font,
                original,
                parent,
                limits.numeric,
                &mut receipt.numeric_calls,
            )?),
            None => None,
        };
        let resolver =
            resolve::Resolver::new(original, limits.numeric, font_size.map(|fact| fact.pixels));
        let mut row = resolve::element(
            &winners,
            ids,
            parent,
            resolver,
            mode,
            &mut receipt.numeric_calls,
        )?;
        row.font_size = font_size;
        // REL (CSS2.2 9.3.2, 9.4.3): only a Sequence row whose computed `position` is
        // `relative` resolves its offsets, after its other longhands and before its
        // pseudos; the offsets do not apply to any other row (9.3.1). An O or P that is
        // not relatively positioned keeps its specified offsets, unconverted, for a
        // relative child's `inherit` (ISP). Only the Sequence entry reads the position
        // (MMX K-2), which `prepare_box_facts` proves present; an absence is the existing
        // join error.
        if mode == BoxFactMode::Sequence {
            let position = display
                .resolution(original)
                .ok_or(E::SourceJoin { projected })?
                .position;
            let parent_static = parent.and_then(|(original_parent, _)| {
                statics
                    .into_iter()
                    .flatten()
                    .find(|row| row.original == original_parent)
            });
            if position == Position::Relative
                || (absolute_offsets && matches!(position, Position::Absolute | Position::Fixed))
            {
                row.offset = Some(resolve::offsets(
                    &winners,
                    ids,
                    parent,
                    parent_static,
                    resolver,
                    &mut receipt.numeric_calls,
                )?);
            } else if let Some(slot) = statics.get_mut(projected) {
                *slot = Some(resolve::static_offsets(
                    &winners,
                    ids,
                    parent,
                    parent_static,
                    resolver,
                ));
            }
        }
        // CF-34 captures graph-owned operands only. Reporting failure is retained
        // by the observer; all ordinary pseudo/remaining-row checks still run.
        observer.observe(original, &winners, &row, ids);
        // Whole-tree layout generates this content itself, so nothing is refused here.
        let pseudo_targets: &[(BoxFactTarget, PseudoElement)] = if absolute_offsets {
            &[]
        } else {
            &[
                (BoxFactTarget::Before, PseudoElement::Before),
                (BoxFactTarget::After, PseudoElement::After),
            ]
        };
        for &(target, pseudo) in pseudo_targets {
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
