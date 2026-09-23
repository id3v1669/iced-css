//! Inner vertical layout from the actual private computed batch and full capture.
//! The explicit-band entry accepts P geometry; the horizontal entry derives its
//! band from actual P box facts and source-bound upstream O left/width. Both retain
//! explicit P top. The outer-only entry also derives P top from actual CSS.
//! Source binding proves identity, not upstream layout agreement.
//! The height entry also derives bounded P content height from those results.
//! These entries do not compute O placement, clipping or native geometry.
use super::{
    clearance_box_facts::{BoxLengthFact, BoxOverflow, ClearanceBoxFact, ClearanceBoxFacts},
    clearance_profile::ProjectionView,
    clearance_source_projection::ClearanceProjectionShape,
    source_display::SourceDisplay,
};
use iced_css_core::{
    formatting::{
        bfc_height::{BfcHeightError, BfcHeightPlacement},
        block::{
            AutoLength, BlockHorizontalError, BlockHorizontalPlacement, BlockWidthInput, Direction,
            place_block_horizontal,
        },
        clearance::{
            ClearSide, ClearanceFloatError, ClearancePresenceError, ClearanceRequirement,
            classify_clearance, lowest_clearance_float_bottom,
        },
        clearance_owner::{
            ChildContext, ChildFacts, ChildMargins, ClearanceOwner, ClearanceOwnerError,
            ClearanceOwnerInput, OuterFacts, OwnerContent, OwnerEdges, OwnerGenerated, OwnerHeight,
            ParentBfc, ParentFacts, build_nested_clearance_owner,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        first_child_origin::{
            FirstChildOriginError, FirstChildOriginInput, FirstChildOriginPlacement,
            place_first_child_origin,
        },
        floats::Band,
    },
    source::SourceNode,
    values::bidi::ComputedDirection,
};
/// Already established upstream CSS geometry for this exact inner source.
/// Its agreement with the computed box equation and surrounding placement is a
/// caller assertion; neither the band nor top is inferred from iced measurements.
pub struct ClearanceContentGeometry<'p> {
    pub parent: &'p SourceNode,
    pub content_top: f32,
    pub containing: Band,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedClearanceUnsupported {
    ParentOverflow,
    ChildOverflow,
    AutoMargin,
    FloatWidth,
    BlockWidth,
    HeightKind,
    DisplayOrFlow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedClearanceError {
    Shape,
    GeometrySource,
    Join {
        projected: usize,
    },
    Unsupported {
        projected: usize,
        property: &'static str,
        reason: CapturedClearanceUnsupported,
    },
    Core(ClearanceOwnerError),
}
impl std::fmt::Display for CapturedClearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured clearance: {self:?}")
    }
}
impl std::error::Error for CapturedClearanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            _ => None,
        }
    }
}
pub(super) fn unsupported(
    projected: usize,
    property: &'static str,
    reason: CapturedClearanceUnsupported,
) -> CapturedClearanceError {
    CapturedClearanceError::Unsupported {
        projected,
        property,
        reason,
    }
}
pub(super) fn edges([top, right, bottom, left]: [f32; 4]) -> OwnerEdges {
    OwnerEdges {
        top,
        right,
        bottom,
        left,
    }
}
/// Derive the finite nonroot region from real computed CSS facts without allocation.
/// The result borrows projection storage, independently of temporary fact/style
/// wrappers. The full five-node source graph remains intact.
/// # Errors
/// Rejects unsupported shapes, geometry-source mismatch, unsupported computed
/// categories or core profile/numeric failures; publishes no partial owner.
pub fn build_captured_clearance_owner<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    geometry: ClearanceContentGeometry<'p>,
) -> Result<ClearanceOwner<'p>, CapturedClearanceError> {
    preflight(facts, geometry.parent)?;
    admit_captured_region(facts)?.finish(geometry.content_top, geometry.containing)
}
fn preflight(
    facts: &ClearanceBoxFacts<'_, '_, '_, '_, '_, '_>,
    parent: &SourceNode,
) -> Result<(), CapturedClearanceError> {
    shape_preflight(facts)?;
    let nodes = facts.projection().nodes();
    if !same_source(parent, &nodes[1]) {
        return Err(CapturedClearanceError::GeometrySource);
    }
    Ok(())
}
fn shape_preflight(
    facts: &ClearanceBoxFacts<'_, '_, '_, '_, '_, '_>,
) -> Result<(), CapturedClearanceError> {
    let projection = facts.projection();
    if projection.shape() != ClearanceProjectionShape::NestedRegion || projection.nodes().len() != 5
    {
        return Err(CapturedClearanceError::Shape);
    }
    Ok(())
}
pub(super) fn same_source(supplied: &SourceNode, actual: &SourceNode) -> bool {
    std::ptr::eq(supplied, actual) && supplied.instance == actual.instance
}
/// Copied admitted style data and actual sources, with no provisional geometry.
struct AdmittedRegion<'p> {
    nodes: &'p [SourceNode],
    outer: OuterFacts<'p>,
    parent_display: DisplayResolution,
    parent_border: OwnerEdges,
    parent_padding: OwnerEdges,
    margin_top: BoxLengthFact,
    parent_clear: ClearSide,
    width: AutoLength,
    margin_left: AutoLength,
    margin_right: AutoLength,
    before: ChildFacts<'p>,
    float: ChildFacts<'p>,
    after: ChildFacts<'p>,
    float_width: f32,
}
impl<'p> AdmittedRegion<'p> {
    fn finish(
        self,
        content_top: f32,
        containing: Band,
    ) -> Result<ClearanceOwner<'p>, CapturedClearanceError> {
        build_nested_clearance_owner(
            self.nodes,
            self.outer,
            ClearanceOwnerInput {
                parent: ParentFacts {
                    source: &self.nodes[1],
                    display: self.parent_display,
                    content: OwnerContent::NoOwnContentNonReplaced,
                    generated: OwnerGenerated::None,
                    // P is nonroot, never HTML body. Hidden overflow establishes its
                    // BFC; the full graph proves an empty preceding float prefix.
                    bfc: ParentBfc::NewWithEmptyPrefix,
                    content_top,
                    containing,
                    border: self.parent_border,
                    padding: self.parent_padding,
                },
                before: self.before,
                float: self.float,
                after: self.after,
                float_width: self.float_width,
            },
        )
        .map_err(CapturedClearanceError::Core)
    }
}
pub(super) fn auto_length(value: BoxLengthFact) -> AutoLength {
    match value {
        BoxLengthFact::Auto { .. } => AutoLength::Auto,
        BoxLengthFact::Absolute(value) => AutoLength::Value(value.pixels),
    }
}

pub(super) fn joined_display<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    projected: usize,
) -> Result<DisplayResolution, CapturedClearanceError> {
    let error = CapturedClearanceError::Join { projected };
    let original = projection.original_index(projected).ok_or(error)?;
    if projection.projected_index(original) != Some(projected) {
        return Err(error);
    }
    let projected_token = projection.token(projected).ok_or(error)?;
    let display_token = display.token(original).ok_or(error)?;
    if !std::ptr::eq(projected_token, display_token) {
        return Err(error);
    }
    display.resolution(original).copied().ok_or(error)
}

pub(super) fn joined_direction<'h>(
    projection: &impl ProjectionView<'h>,
    display: &SourceDisplay<'_, '_, '_>,
    projected: usize,
) -> Result<Direction, CapturedClearanceError> {
    let error = CapturedClearanceError::Join { projected };
    let original = projection.original_index(projected).ok_or(error)?;
    if projection.projected_index(original) != Some(projected) {
        return Err(error);
    }
    let projected_token = projection.token(projected).ok_or(error)?;
    let display_token = display.token(original).ok_or(error)?;
    if !std::ptr::eq(projected_token, display_token) {
        return Err(error);
    }
    let bidi = display.bidi_style(original).ok_or(error)?;
    Ok(match bidi.direction() {
        ComputedDirection::LeftToRight => Direction::Ltr,
        ComputedDirection::RightToLeft => Direction::Rtl,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildMarginMode {
    AbsoluteOnly,
    SequenceHorizontal,
}

/// Convert captured computed child facts without replacing their margin categories.
/// `SequenceHorizontal` preserves horizontal `Auto`; sequence float admission gives
/// that computed category used value zero. Vertical `Auto` remains unsupported.
pub(super) fn child_facts_from_row<'p>(
    nodes: &'p [SourceNode],
    display: DisplayResolution,
    row: &ClearanceBoxFact,
    projected: usize,
    height: OwnerHeight,
    mode: ChildMarginMode,
) -> Result<ChildFacts<'p>, CapturedClearanceError> {
    use CapturedClearanceError as E;
    use CapturedClearanceUnsupported as U;
    let allow_horizontal_auto = mode == ChildMarginMode::SequenceHorizontal;
    let vertical = |edge: usize, property: &'static str| match row.margin[edge] {
        BoxLengthFact::Absolute(value) => Ok(value.pixels),
        BoxLengthFact::Auto { .. } => Err(unsupported(projected, property, U::AutoMargin)),
    };
    let horizontal = |edge: usize, property: &'static str| match row.margin[edge] {
        BoxLengthFact::Absolute(value) => Ok(AutoLength::Value(value.pixels)),
        BoxLengthFact::Auto { .. } if allow_horizontal_auto => Ok(AutoLength::Auto),
        BoxLengthFact::Auto { .. } => Err(unsupported(projected, property, U::AutoMargin)),
    };
    let top = vertical(0, "margin-top")?;
    let right = horizontal(1, "margin-right")?;
    let bottom = vertical(2, "margin-bottom")?;
    let left = horizontal(3, "margin-left")?;
    let margin = ChildMargins {
        top,
        right,
        bottom,
        left,
    };
    Ok(ChildFacts {
        source: nodes.get(projected).ok_or(E::Join { projected })?,
        display,
        context: if projected == 3 {
            ChildContext::FloatBfc
        } else {
            ChildContext::OrdinaryNonBfc
        },
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
        height,
        margin,
        border: edges(row.border_widths()),
        padding: edges(row.padding.map(|value| value.pixels)),
        clear: row.clear.value,
    })
}
fn admit_captured_region<'p>(
    facts: &ClearanceBoxFacts<'p, '_, '_, '_, '_, '_>,
) -> Result<AdmittedRegion<'p>, CapturedClearanceError> {
    use CapturedClearanceError as E;
    use CapturedClearanceUnsupported as U;
    let projection = facts.projection();
    let nodes = projection.nodes();
    let row = |projected| facts.row(projected).ok_or(E::Join { projected });
    let display = |projected| {
        let original = projection
            .original_index(projected)
            .ok_or(E::Join { projected })?;
        if projection.projected_index(original) != Some(projected) {
            return Err(E::Join { projected });
        }
        facts
            .display()
            .resolution(original)
            .copied()
            .ok_or(E::Join { projected })
    };
    for projected in 0..5 {
        row(projected)?;
        let d = display(projected)?;
        if d.display != Display::Block
            || d.position != Position::Static
            || if projected == 3 {
                d.flow != Flow::Floating || !matches!(d.float, Float::Left | Float::Right)
            } else {
                d.flow != Flow::Normal || d.float != Float::None
            }
        {
            return Err(unsupported(
                projected,
                "display/position/float",
                U::DisplayOrFlow,
            ));
        }
    }
    let p = row(1)?;
    if p.overflow.value != BoxOverflow::Hidden {
        return Err(unsupported(1, "overflow", U::ParentOverflow));
    }
    for projected in 2..5 {
        if row(projected)?.overflow.value != BoxOverflow::Visible {
            return Err(unsupported(projected, "overflow", U::ChildOverflow));
        }
    }
    for projected in [2, 4] {
        if !matches!(row(projected)?.width, BoxLengthFact::Auto { .. }) {
            return Err(unsupported(projected, "width", U::BlockWidth));
        }
    }
    let height = |projected| match row(projected)?.height {
        BoxLengthFact::Absolute(value) => Ok(OwnerHeight::Explicit(value.pixels)),
        BoxLengthFact::Auto { .. } => Err(unsupported(projected, "height", U::HeightKind)),
    };
    let heights = [height(2)?, height(3)?, height(4)?];
    let float_width = match row(3)?.width {
        BoxLengthFact::Absolute(value) => value.pixels,
        BoxLengthFact::Auto { .. } => return Err(unsupported(3, "width", U::FloatWidth)),
    };
    let child = |projected: usize| {
        let r = row(projected)?;
        child_facts_from_row(
            nodes,
            display(projected)?,
            r,
            projected,
            heights[projected - 2],
            ChildMarginMode::AbsoluteOnly,
        )
    };
    Ok(AdmittedRegion {
        nodes,
        outer: OuterFacts {
            source: &nodes[0],
            display: display(0)?,
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
        },
        parent_display: display(1)?,
        parent_border: edges(p.border_widths()),
        parent_padding: edges(p.padding.map(|v| v.pixels)),
        margin_top: p.margin[0],
        parent_clear: p.clear.value,
        width: auto_length(p.width),
        margin_left: auto_length(p.margin[3]),
        margin_right: auto_length(p.margin[1]),
        before: child(2)?,
        float: child(3)?,
        after: child(4)?,
        float_width,
    })
}

/// Upstream O content geometry for this exact projected source. Agreement with
/// surrounding CSS layout remains the caller's assertion; no O right is formed.
pub struct ClearanceOuterHorizontalGeometry<'p> {
    pub outer: &'p SourceNode,
    pub content_left: f32,
    pub content_width: f32,
}
/// Upstream P vertical origin, independently source-bound to projected P.
pub struct ClearanceParentVerticalOrigin<'p> {
    pub parent: &'p SourceNode,
    pub content_top: f32,
}
/// Retains projection sources only, not temporary CSS evaluation/input wrappers.
#[derive(Debug)]
pub struct CapturedHorizontalClearance<'p> {
    horizontal: BlockHorizontalPlacement,
    owner: ClearanceOwner<'p>,
}
impl<'p> CapturedHorizontalClearance<'p> {
    /// A copy of the used dimensions and separately published endpoint band.
    pub fn horizontal(&self) -> BlockHorizontalPlacement {
        self.horizontal
    }
    /// This reference borrows the combined result; sources retain their original
    /// projection lifetime through ClearanceOwner::sources().
    pub fn owner(&self) -> &ClearanceOwner<'p> {
        &self.owner
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalClearanceError {
    OuterGeometrySource,
    Captured(CapturedClearanceError),
    Horizontal(BlockHorizontalError),
}
impl std::fmt::Display for HorizontalClearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured horizontal clearance: {self:?}")
    }
}
impl std::error::Error for HorizontalClearanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OuterGeometrySource => None,
            Self::Captured(error) => Some(error),
            Self::Horizontal(error) => Some(error),
        }
    }
}
/// Derive P's horizontal content band from actual box facts and O's computed
/// direction, then feed that published band unchanged to the shared inner owner.
/// Endpoint rounding can collapse or expand the band relative to used width;
/// existing endpoint-based float admission remains authoritative here.
/// # Errors
/// Checks shape, P identity, O identity, shared profile, O direction join and
/// horizontal publication in that order, then core geometry/numeric admission.
/// Coordinate errors can therefore precede invalid P top or child numeric facts.
/// No partial result is published. Upstream O geometry and P top remain assertions.
pub fn build_captured_clearance_with_horizontal<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterHorizontalGeometry<'p>,
    parent: ClearanceParentVerticalOrigin<'p>,
) -> Result<CapturedHorizontalClearance<'p>, HorizontalClearanceError> {
    use HorizontalClearanceError as E;
    preflight(facts, parent.parent).map_err(E::Captured)?;
    let projection = facts.projection();
    if !same_source(outer.outer, &projection.nodes()[0]) {
        return Err(E::OuterGeometrySource);
    }
    let admitted = admit_captured_region(facts).map_err(E::Captured)?;
    let horizontal =
        place_admitted_horizontal(facts, &admitted, outer.content_left, outer.content_width)?;
    let owner = admitted
        .finish(parent.content_top, horizontal.content)
        .map_err(E::Captured)?;
    Ok(CapturedHorizontalClearance { horizontal, owner })
}

/// One direction join and horizontal solver dispatch shared by both derived entries.
fn place_admitted_horizontal(
    facts: &ClearanceBoxFacts<'_, '_, '_, '_, '_, '_>,
    admitted: &AdmittedRegion<'_>,
    content_left: f32,
    content_width: f32,
) -> Result<BlockHorizontalPlacement, HorizontalClearanceError> {
    use HorizontalClearanceError as E;
    let direction =
        joined_direction(facts.projection(), facts.display(), 0).map_err(E::Captured)?;
    place_block_horizontal(
        content_left,
        BlockWidthInput {
            containing_width: content_width,
            direction,
            width: admitted.width,
            margin_left: admitted.margin_left,
            margin_right: admitted.margin_right,
            border_left: admitted.parent_border.left,
            border_right: admitted.parent_border.right,
            padding_left: admitted.parent_padding.left,
            padding_right: admitted.parent_padding.right,
            // The private computed batch admits only default min/max constraints.
            min_width: 0.,
            max_width: None,
        },
    )
    .map_err(E::Horizontal)
}

/// Already established content geometry of the exact outer source O.
/// Agreement with upstream CSS layout remains the caller's assertion.
pub struct ClearanceOuterContentGeometry<'p> {
    pub outer: &'p SourceNode,
    pub content_left: f32,
    pub content_width: f32,
    pub content_top: f32,
}
/// Copied observations and inner owner borrowing only projection source storage.
#[derive(Debug)]
pub struct CapturedParentClearance<'p> {
    horizontal: BlockHorizontalPlacement,
    parent_origin: FirstChildOriginPlacement,
    computed_margin_top: BoxLengthFact,
    parent_clear: ClearSide,
    parent_clearance: ClearanceRequirement,
    owner: ClearanceOwner<'p>,
}
impl<'p> CapturedParentClearance<'p> {
    /// Used width and separately published content endpoints.
    pub fn horizontal(&self) -> BlockHorizontalPlacement {
        self.horizontal
    }
    /// Used top margin and separately published border/content tops.
    pub fn parent_origin(&self) -> FirstChildOriginPlacement {
        self.parent_origin
    }
    /// Actual computed P margin, including inheritance and rounding history.
    pub fn computed_margin_top(&self) -> BoxLengthFact {
        self.computed_margin_top
    }
    /// Actual computed P clear side.
    pub fn parent_clear(&self) -> ClearSide {
        self.parent_clear
    }
    /// Classified absence, distinct from numerical zero clearance.
    pub fn parent_clearance(&self) -> ClearanceRequirement {
        self.parent_clearance
    }
    /// Borrows this result; owner sources retain their upstream projection lifetime.
    pub fn owner(&self) -> &ClearanceOwner<'p> {
        &self.owner
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentClearanceError {
    OuterGeometrySource,
    Captured(CapturedClearanceError),
    Horizontal(BlockHorizontalError),
    Origin(FirstChildOriginError),
    ParentFloatSelection(ClearanceFloatError),
    ParentPresence(ClearancePresenceError),
    UnexpectedParentClearance,
}
impl std::fmt::Display for ParentClearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured parent clearance: {self:?}")
    }
}
impl std::error::Error for ParentClearanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Captured(e) => Some(e),
            Self::Horizontal(e) => Some(e),
            Self::Origin(e) => Some(e),
            Self::ParentFloatSelection(e) => Some(e),
            Self::ParentPresence(e) => Some(e),
            Self::OuterGeometrySource | Self::UnexpectedParentClearance => None,
        }
    }
}
/// Derive P's content origin and horizontal band from actual CSS and O geometry.
/// O is the actual root with only P; root margins do not collapse. P is a nonroot
/// hidden-overflow BFC. Its descendants cannot constrain its own clearance. This
/// establishes noncollapsing first-child placement without asserting a root BFC.
/// O content top already accounts for O's margins/padding; they are not added again.
/// P auto-height, bottom geometry and native layout remain outside this entry.
/// # Errors
/// Checks shape, O identity, shared admission, horizontal placement, top origin,
/// parent clearance absence and finally the inner owner, in that order. No partial
/// result is published. Temporary facts/style/geometry wrappers are not retained.
pub fn build_captured_clearance_from_outer<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
) -> Result<CapturedParentClearance<'p>, ParentClearanceError> {
    use ParentClearanceError as E;
    shape_preflight(facts).map_err(E::Captured)?;
    if !same_source(outer.outer, &facts.projection().nodes()[0]) {
        return Err(E::OuterGeometrySource);
    }
    let admitted = admit_captured_region(facts).map_err(E::Captured)?;
    let horizontal =
        place_admitted_horizontal(facts, &admitted, outer.content_left, outer.content_width)
            .map_err(|error| match error {
                HorizontalClearanceError::Captured(e) => E::Captured(e),
                HorizontalClearanceError::Horizontal(e) => E::Horizontal(e),
                HorizontalClearanceError::OuterGeometrySource => E::OuterGeometrySource,
            })?;
    let computed_margin_top = admitted.margin_top;
    let parent_clear = admitted.parent_clear;
    let parent_origin = place_first_child_origin(FirstChildOriginInput {
        containing_top: outer.content_top,
        margin_top: auto_length(computed_margin_top),
        border_top: admitted.parent_border.top,
        padding_top: admitted.parent_padding.top,
    })
    .map_err(E::Origin)?;
    // Full graph/content admission proves no earlier outer float. Original source
    // indices are postorder storage positions, never float-eligibility evidence.
    let selected = lowest_clearance_float_bottom(&[], parent_clear, std::num::NonZeroUsize::MIN)
        .map_err(E::ParentFloatSelection)?;
    let parent_clearance =
        classify_clearance(parent_origin.border_top, selected).map_err(E::ParentPresence)?;
    if parent_clearance != ClearanceRequirement::Absent {
        return Err(E::UnexpectedParentClearance);
    }
    let owner = admitted
        .finish(parent_origin.content_top, horizontal.content)
        .map_err(E::Captured)?;
    Ok(CapturedParentClearance {
        horizontal,
        parent_origin,
        computed_margin_top,
        parent_clear,
        parent_clearance,
        owner,
    })
}

/// Accepted parent layout plus copied content-height observations and provenance.
/// Only the projection source borrow is retained; style/fact wrappers may end.
#[derive(Debug)]
pub struct CapturedParentHeight<'p> {
    parent: CapturedParentClearance<'p>,
    height: BfcHeightPlacement,
    computed_height: BoxLengthFact,
}
impl<'p> CapturedParentHeight<'p> {
    /// Borrows this combined result; its owner's sources retain projection lifetime.
    pub fn parent(&self) -> &CapturedParentClearance<'p> {
        &self.parent
    }
    /// Used content height and independently published content-bottom observations.
    pub fn height(&self) -> BfcHeightPlacement {
        self.height
    }
    /// Actual computed P height, preserving origin and conversion history.
    pub fn computed_height(&self) -> BoxLengthFact {
        self.computed_height
    }
}
/// Existing parent errors precede any new content-height work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentHeightError {
    Parent(ParentClearanceError),
    FactInvariant {
        projected: usize,
        property: &'static str,
    },
    Height(BfcHeightError),
}
impl std::fmt::Display for ParentHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured parent height: {self:?}")
    }
}
impl std::error::Error for ParentHeightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parent(error) => Some(error),
            Self::Height(error) => Some(error),
            Self::FactInvariant { .. } => None,
        }
    }
}
/// Derive bounded P content height from real CSS and the accepted parent pipeline.
/// Runs that pipeline once, then uses its published top and child/float bottoms.
/// Explicit height does not evaluate descendant extents for this new stage.
/// P bottom decorations and own bottom margin are outside content height; general
/// tree height, O placement, clipping and native geometry remain outside scope.
/// # Errors
/// Existing parent validation/placement errors come first. FactInvariant denotes
/// an internal predecessor inconsistency, not a new authored-CSS category.
/// Height errors name invalid primitive inputs or finite publication limits.
/// No partial combined output is published on failure.
pub fn build_captured_clearance_with_parent_height<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
) -> Result<CapturedParentHeight<'p>, ParentHeightError> {
    use ParentHeightError as E;
    use iced_css_core::formatting::bfc_height::{BfcHeightInput, BfcHeightMode, solve_bfc_height};
    let parent = build_captured_clearance_from_outer(facts, outer).map_err(E::Parent)?;
    let computed_height = facts
        .row(1)
        .ok_or(E::FactInvariant {
            projected: 1,
            property: "height",
        })?
        .height;
    let mode = match computed_height {
        BoxLengthFact::Absolute(value) => BfcHeightMode::Explicit(value.pixels),
        BoxLengthFact::Auto { .. } => {
            let invariant = E::FactInvariant {
                projected: 4,
                property: "margin-bottom",
            };
            let BoxLengthFact::Absolute(margin) = facts.row(4).ok_or(invariant)?.margin[2] else {
                return Err(invariant);
            };
            BfcHeightMode::Auto {
                last_in_flow_border_bottom: parent.owner().after().bottom,
                last_in_flow_margin_bottom: margin.pixels,
                float_bottom: parent.owner().placed_float().bottom,
            }
        }
    };
    let height = solve_bfc_height(BfcHeightInput {
        content_top: parent.parent_origin().content_top,
        mode,
    })
    .map_err(E::Height)?;
    Ok(CapturedParentHeight {
        parent,
        height,
        computed_height,
    })
}

use iced_css_core::formatting::block_bottom::{BlockBottomError, BlockBottomPlacement};

/// Accepted parent height plus copied own bottom edges and computed provenance.
/// Only the projection source borrow is retained; temporary style/fact wrappers end.
#[derive(Debug)]
pub struct CapturedParentBottom<'p> {
    parent: CapturedParentHeight<'p>,
    bottom: BlockBottomPlacement,
    computed_margin_bottom: BoxLengthFact,
}
impl<'p> CapturedParentBottom<'p> {
    /// Borrow the preceding layout; its source getter retains projection lifetime.
    pub fn parent(&self) -> &CapturedParentHeight<'p> {
        &self.parent
    }
    /// Independently published bottom coordinates and used margin length/category.
    pub fn bottom(&self) -> BlockBottomPlacement {
        self.bottom
    }
    /// Actual computed margin, including origin and underflow history.
    pub fn computed_margin_bottom(&self) -> BoxLengthFact {
        self.computed_margin_bottom
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentBottomError {
    Parent(ParentHeightError),
    FactInvariant {
        projected: usize,
        property: &'static str,
    },
    Bottom(BlockBottomError),
}
impl std::fmt::Display for ParentBottomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured parent bottom: {self:?}")
    }
}
impl std::error::Error for ParentBottomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parent(e) => Some(e),
            Self::Bottom(e) => Some(e),
            Self::FactInvariant { .. } => None,
        }
    }
}
/// Derive P's own bottom edges from the once-delegated height pipeline and facts.
/// Uses its authoritative content endpoint, never top plus rounded used height.
/// Signed own margins may invert edge ordering. This does not establish sibling
/// flow/collapse, O height, native geometry or broader border-style admission.
/// # Errors
/// All predecessor errors come first. A missing private row is an internal fact
/// invariant, not authored CSS invalidity. Bottom publication errors return no
/// partial result. Earlier inputs remain reusable by the independent height API.
pub fn build_captured_clearance_with_parent_bottom<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &ClearanceBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
) -> Result<CapturedParentBottom<'p>, ParentBottomError> {
    use ParentBottomError as E;
    use iced_css_core::formatting::block_bottom::{BlockBottomInput, place_block_bottom};
    let parent = build_captured_clearance_with_parent_height(facts, outer).map_err(E::Parent)?;
    let row = facts.row(1).ok_or(E::FactInvariant {
        projected: 1,
        property: "margin-bottom",
    })?;
    let computed_margin_bottom = row.margin[2];
    let bottom = place_block_bottom(BlockBottomInput {
        content_bottom: parent.height().content_bottom,
        padding_bottom: row.padding[2].pixels,
        border_bottom: row.border_widths()[2],
        margin_bottom: auto_length(computed_margin_bottom),
    })
    .map_err(E::Bottom)?;
    Ok(CapturedParentBottom {
        parent,
        bottom,
        computed_margin_bottom,
    })
}
