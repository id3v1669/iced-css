//! Compute the three box-participation properties from actual cascade winners.
use crate::{
    cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners},
    formatting::display::{
        Display, DisplayError, DisplayResolution, Float, Position, resolve_display,
    },
    values::{property::SpecifiedValue, registry::PropertyId},
};
#[derive(Debug, PartialEq, Eq)]
pub enum ComputedDisplayError {
    Defaulting(DefaultingError),
    Value(DisplayError),
}
impl std::fmt::Display for ComputedDisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Defaulting(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ComputedDisplayError {}
fn selected<T: Copy>(
    name: &'static str,
    winners: &PropertyWinners<'_>,
    parent: Option<T>,
    initial: fn(&str) -> Result<T, DisplayError>,
    specified: fn(&SpecifiedValue) -> Result<T, DisplayError>,
) -> Result<T, ComputedDisplayError> {
    let property = PropertyId::lookup(name).expect("CSS22 box property is registered");
    match winners
        .defaulted(property, parent.is_some())
        .map_err(ComputedDisplayError::Defaulting)?
    {
        DefaultedValueRef::Compute(value) => specified(value).map_err(ComputedDisplayError::Value),
        DefaultedValueRef::Initial(property) => {
            initial(property.descriptor().initial).map_err(ComputedDisplayError::Value)
        }
        DefaultedValueRef::ParentComputed => {
            Ok(parent.expect("parent-computed default requires a parent"))
        }
    }
}
/// `parent` is the actual source parent's computed result. None denotes the
/// document root. Anonymous formatting boxes must not be supplied as parents.
/// No native hints, source values or parent values are mutated or cloned here.
pub fn compute_display_style(
    winners: &PropertyWinners<'_>,
    parent: Option<&DisplayResolution>,
) -> Result<DisplayResolution, ComputedDisplayError> {
    // CSS Flexbox 1 and CSS Grid 1: a flex or grid container is a block outside, or an
    // inline-block, with its children laid out as items (`flex` or `grid` says so).
    fn display_of(value: &SpecifiedValue) -> Result<Display, DisplayError> {
        match value {
            SpecifiedValue::Keyword(keyword) if matches!(keyword.as_str(), "flex" | "grid") => {
                Ok(Display::Block)
            }
            SpecifiedValue::Keyword(keyword)
                if matches!(keyword.as_str(), "inline-flex" | "inline-grid") =>
            {
                Ok(Display::InlineBlock)
            }
            other => Display::from_resolved(other),
        }
    }
    let display = selected(
        "display",
        winners,
        parent.map(|p| p.display),
        Display::from_keyword,
        display_of,
    )?;
    let display_property = PropertyId::lookup("display").expect("registered");
    let (flex, grid) = match winners
        .defaulted(display_property, parent.is_some())
        .map_err(ComputedDisplayError::Defaulting)?
    {
        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => (
            matches!(keyword.as_str(), "flex" | "inline-flex"),
            matches!(keyword.as_str(), "grid" | "inline-grid"),
        ),
        DefaultedValueRef::ParentComputed => {
            parent.map_or((false, false), |parent| (parent.flex, parent.grid))
        }
        _ => (false, false),
    };
    let position = selected(
        "position",
        winners,
        parent.map(|p| p.position),
        Position::from_keyword,
        Position::from_resolved,
    )?;
    let float = selected(
        "float",
        winners,
        parent.map(|p| p.float),
        Float::from_keyword,
        Float::from_resolved,
    )?;
    // 4: a flex or grid item is blockified, as the root is, and does not float.
    let item = parent.is_some_and(|parent| parent.flex || parent.grid);
    let float = if item { Float::None } else { float };
    let mut resolved = resolve_display(display, position, float, parent.is_none() || item);
    // An absolutely positioned or suppressed box is still a flex container inside.
    resolved.flex = flex && resolved.display != Display::None;
    resolved.grid = grid && resolved.display != Display::None;
    Ok(resolved)
}
