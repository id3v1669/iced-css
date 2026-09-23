//! Inner vertical layout from the actual private computed batch and full capture.
//! The explicit-band entry accepts P geometry; the horizontal entry derives its
//! band from actual P box facts and source-bound upstream O left/width. Both retain
//! explicit P top. Source binding proves identity, not upstream layout agreement.
//! Neither computes outer placement, parent auto-height, clipping or native geometry.
use super::{
    clearance_box_facts::{BoxLengthFact, BoxOverflow, ClearanceBoxFacts},
    clearance_source_projection::ClearanceProjectionShape,
};
use iced_css_core::{
    formatting::{
        block::{
            AutoLength, BlockHorizontalError, BlockHorizontalPlacement, BlockWidthInput, Direction,
            place_block_horizontal,
        },
        clearance_owner::{
            ChildContext, ChildFacts, ClearanceOwner, ClearanceOwnerError, ClearanceOwnerInput,
            OuterFacts, OwnerContent, OwnerEdges, OwnerGenerated, OwnerHeight, ParentBfc,
            ParentFacts, build_nested_clearance_owner,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
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
fn unsupported(
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
fn edges([top, right, bottom, left]: [f32; 4]) -> OwnerEdges {
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
    let projection = facts.projection();
    let nodes = projection.nodes();
    if projection.shape() != ClearanceProjectionShape::NestedRegion || nodes.len() != 5 {
        return Err(CapturedClearanceError::Shape);
    }
    if !same_source(parent, &nodes[1]) {
        return Err(CapturedClearanceError::GeometrySource);
    }
    Ok(())
}
fn same_source(supplied: &SourceNode, actual: &SourceNode) -> bool {
    std::ptr::eq(supplied, actual) && supplied.instance == actual.instance
}
/// Copied admitted style data and actual sources, with no provisional geometry.
struct AdmittedRegion<'p> {
    nodes: &'p [SourceNode],
    outer: OuterFacts<'p>,
    parent_display: DisplayResolution,
    parent_border: OwnerEdges,
    parent_padding: OwnerEdges,
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
fn auto_length(value: BoxLengthFact) -> AutoLength {
    match value {
        BoxLengthFact::Auto { .. } => AutoLength::Auto,
        BoxLengthFact::Absolute(value) => AutoLength::Value(value.pixels),
    }
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
        let mut margin = [0.; 4];
        for (edge, property) in ["margin-top", "margin-right", "margin-bottom", "margin-left"]
            .into_iter()
            .enumerate()
        {
            margin[edge] = match r.margin[edge] {
                BoxLengthFact::Absolute(value) => value.pixels,
                BoxLengthFact::Auto { .. } => {
                    return Err(unsupported(projected, property, U::AutoMargin));
                }
            };
        }
        Ok(ChildFacts {
            source: &nodes[projected],
            display: display(projected)?,
            context: if projected == 3 {
                ChildContext::FloatBfc
            } else {
                ChildContext::OrdinaryNonBfc
            },
            // Projection admits only Row/Column own-content capabilities; the
            // private batch has resolved both pseudos absent on all five sources.
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            height: heights[projected - 2],
            margin: edges(margin),
            border: edges(r.border_widths()),
            padding: edges(r.padding.map(|v| v.pixels)),
            clear: r.clear.value,
        })
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
    let original = projection
        .original_index(0)
        .ok_or(E::Captured(CapturedClearanceError::Join { projected: 0 }))?;
    let bidi = facts
        .display()
        .bidi_style(original)
        .ok_or(E::Captured(CapturedClearanceError::Join { projected: 0 }))?;
    let direction = match bidi.direction() {
        ComputedDirection::LeftToRight => Direction::Ltr,
        ComputedDirection::RightToLeft => Direction::Rtl,
    };
    let horizontal = place_block_horizontal(
        outer.content_left,
        BlockWidthInput {
            containing_width: outer.content_width,
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
    .map_err(E::Horizontal)?;
    let owner = admitted
        .finish(parent.content_top, horizontal.content)
        .map_err(E::Captured)?;
    Ok(CapturedHorizontalClearance { horizontal, owner })
}
