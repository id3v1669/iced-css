//! Same-side computed width inheritance and resolved numeric target comparison.
use super::{
    NonnegativeF32, NumericLimits,
    border_width::{BorderFontContext, BorderWidthError, compute_border_width},
    property::ParsedProperty,
    registry::PropertyId,
};
use crate::{
    cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners},
    formatting::table_borders::BorderStyle,
};

#[derive(Debug, Clone, Copy)]
pub struct ComputedBorderWidth {
    property: PropertyId,
    sample: NonnegativeF32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum BorderComputationError {
    UnsupportedProperty,
    InvalidStyle,
    PropertyMismatch,
    Defaulting(DefaultingError),
    Width(BorderWidthError),
}
impl std::fmt::Display for BorderComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed border width: {self:?}")
    }
}
impl std::error::Error for BorderComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Defaulting(error) => Some(error),
            Self::Width(error) => Some(error),
            _ => None,
        }
    }
}
impl ComputedBorderWidth {
    pub fn property(self) -> PropertyId {
        self.property
    }
    /// Conversion events belong to this computation, not inherited provenance.
    pub fn sample(self) -> NonnegativeF32 {
        self.sample
    }
    /// Only the numerical dependency-change test. Equal numbers do not establish
    /// equal typed target semantics, bindings, instances or effective precedence.
    pub fn resolved_target_changed(self, next: Self) -> Result<bool, BorderComputationError> {
        if self.property != next.property {
            return Err(BorderComputationError::PropertyMismatch);
        }
        Ok(self.sample.value != next.sample.value)
    }
}

/// Compute after style defaulting. The supplied parent is its same-side computed
/// number, never its source expression. Resolve parent presentation inheritance
/// and bind source instances in the effect owner before calling this boundary.
/// `None` means this element has no parent; a missing parent value is not a root.
pub fn compute_cascaded_border_width(
    winners: &PropertyWinners<'_>,
    property: PropertyId,
    style: &ParsedProperty,
    font: BorderFontContext,
    parent: Option<ComputedBorderWidth>,
    limits: NumericLimits,
) -> Result<ComputedBorderWidth, BorderComputationError> {
    let expected_style = match property.descriptor().name {
        "border-top-width" => "border-top-style",
        "border-right-width" => "border-right-style",
        "border-bottom-width" => "border-bottom-style",
        "border-left-width" => "border-left-style",
        _ => return Err(BorderComputationError::UnsupportedProperty),
    };
    if style.property.descriptor().name != expected_style {
        return Err(BorderComputationError::InvalidStyle);
    }
    let resolved_style = BorderStyle::from_resolved(&style.value)
        .map_err(|_| BorderComputationError::InvalidStyle)?;
    let suppressed = matches!(resolved_style, BorderStyle::None | BorderStyle::Hidden);
    let sample = match winners
        .defaulted(property, parent.is_some())
        .map_err(BorderComputationError::Defaulting)?
    {
        DefaultedValueRef::Compute(width) => {
            compute_border_width(width, &style.value, font, limits)
                .map_err(BorderComputationError::Width)?
        }
        DefaultedValueRef::ParentComputed => {
            let parent = parent.expect("defaulting checked parent presence");
            if parent.property != property {
                return Err(BorderComputationError::PropertyMismatch);
            }
            NonnegativeF32 {
                value: if suppressed { 0.0 } else { parent.sample.value },
                event: None,
            }
        }
        // The existing border keyword policy maps the initial medium to 3px.
        DefaultedValueRef::Initial(_) => NonnegativeF32 {
            value: if suppressed { 0.0 } else { 3.0 },
            event: None,
        },
    };
    Ok(ComputedBorderWidth { property, sample })
}
