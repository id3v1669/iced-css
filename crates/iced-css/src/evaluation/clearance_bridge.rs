//! Inner vertical layout from the actual private computed batch and full capture.
//! The caller supplies an already established CSS content rectangle for P. Source
//! binding verifies its identity, not agreement with CSS box equations or outer
//! placement. This does not solve O/P dimensions, clipping or native geometry.
use super::{
    clearance_box_facts::{BoxLengthFact, BoxOverflow, ClearanceBoxFacts},
    clearance_source_projection::ClearanceProjectionShape,
};
use iced_css_core::{
    formatting::{
        clearance_owner::{
            ChildContext, ChildFacts, ClearanceOwner, ClearanceOwnerError, ClearanceOwnerInput,
            OuterFacts, OwnerContent, OwnerEdges, OwnerGenerated, OwnerHeight, ParentBfc,
            ParentFacts, build_nested_clearance_owner,
        },
        display::{Display, Float, Flow, Position},
        floats::Band,
    },
    source::SourceNode,
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
    use CapturedClearanceError as E;
    use CapturedClearanceUnsupported as U;
    let projection = facts.projection();
    let nodes = projection.nodes();
    if projection.shape() != ClearanceProjectionShape::NestedRegion || nodes.len() != 5 {
        return Err(E::Shape);
    }
    if !std::ptr::eq(geometry.parent, &nodes[1]) || geometry.parent.instance != nodes[1].instance {
        return Err(E::GeometrySource);
    }
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
    build_nested_clearance_owner(
        nodes,
        OuterFacts {
            source: &nodes[0],
            display: display(0)?,
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
        },
        ClearanceOwnerInput {
            parent: ParentFacts {
                source: &nodes[1],
                display: display(1)?,
                content: OwnerContent::NoOwnContentNonReplaced,
                generated: OwnerGenerated::None,
                // P is nonroot column/row, never HTML body. Hidden overflow establishes
                // its BFC; the complete graph proves an empty preceding float prefix.
                bfc: ParentBfc::NewWithEmptyPrefix,
                content_top: geometry.content_top,
                containing: geometry.containing,
                border: edges(p.border_widths()),
                padding: edges(p.padding.map(|v| v.pixels)),
            },
            before: child(2)?,
            float: child(3)?,
            after: child(4)?,
            float_width,
        },
    )
    .map_err(E::Core)
}
