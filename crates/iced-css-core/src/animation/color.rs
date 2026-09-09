//! Property-owned exact legacy color endpoints in units of 1/25,500 per channel.
use super::combine::{Combination, Endpoint, discrete_choice};
use super::legacy_color::ResolvedLegacyColor;
use crate::values::{
    ConversionError, Decimal, DecimalArithmeticError, DecimalScalingError, FiniteF32, NumericError,
    NumericLimits,
    color::{Color, SystemColor},
    computed_color::{ComputedColor, supported},
    registry::PropertyId,
};

#[derive(Debug)]
enum Value {
    Numeric {
        numerators: [Decimal; 3],
        alpha: f32,
    },
    Invert,
}

#[derive(Debug)]
enum Pair<'a> {
    Numeric([Decimal; 4]),
    Discrete(&'a ColorAnimationEndpoint),
}
#[derive(Debug)]
pub struct CombinedColor<'a> {
    property: PropertyId,
    value: Pair<'a>,
}
#[derive(Debug, Clone, Copy)]
pub enum ColorPairComponent<'a> {
    /// Premultiplied R/G/B numerators in units of1/25,500; alpha is unscaled.
    Numeric(&'a [Decimal; 4]),
    Discrete(&'a ColorAnimationEndpoint),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorFiniteValue {
    Numeric([FiniteF32; 4]),
    Invert,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorFiniteSample {
    pub property: PropertyId,
    pub value: ColorFiniteValue,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPairError {
    PropertyMismatch,
    NonfiniteProgress,
    Numeric(NumericError),
    Scaling(DecimalScalingError),
    Arithmetic(DecimalArithmeticError),
    Conversion(ConversionError),
}
impl std::fmt::Display for ColorPairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "property color combination: {self:?}")
    }
}
impl std::error::Error for ColorPairError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(error) => Some(error),
            Self::Scaling(error) => Some(error),
            Self::Arithmetic(error) => Some(error),
            Self::Conversion(error) => Some(error),
            _ => None,
        }
    }
}
impl CombinedColor<'_> {
    pub fn property(&self) -> PropertyId {
        self.property
    }
    pub fn component(&self) -> ColorPairComponent<'_> {
        match &self.value {
            Pair::Numeric(value) => ColorPairComponent::Numeric(value),
            Pair::Discrete(value) => ColorPairComponent::Discrete(value),
        }
    }
    /// Optional finite inverse conversion, before final alpha/gamut ranges. This
    /// is not a renderer color. Out-of-f32-range exact results are errors here;
    /// the gamut owner must map before conversion where required. At most five
    /// numeric calls use the supplied bounds independently. Invert does no work.
    pub fn to_finite_unpremultiplied(
        &self,
        limits: NumericLimits,
    ) -> Result<ColorFiniteSample, ColorPairError> {
        let (numerators, divisor, alpha): (&[Decimal], Decimal, FiniteF32) = match &self.value {
            Pair::Numeric(values) => {
                let divisor = if values[3].is_zero() {
                    Decimal::from_finite_f32(25_500.0, limits)
                } else {
                    values[3].checked_scale_f32(25_500.0, limits)
                }
                .map_err(ColorPairError::Scaling)?;
                (
                    &values[..3],
                    divisor,
                    values[3]
                        .scaled_finite_f32(1.0, limits)
                        .map_err(ColorPairError::Conversion)?,
                )
            }
            Pair::Discrete(endpoint) => match endpoint.component() {
                ColorEndpointComponent::Invert => {
                    return Ok(ColorFiniteSample {
                        property: self.property,
                        value: ColorFiniteValue::Invert,
                    });
                }
                ColorEndpointComponent::Numeric { numerators, alpha } => (
                    numerators,
                    Decimal::from_finite_f32(25_500.0, limits).map_err(ColorPairError::Scaling)?,
                    FiniteF32 {
                        value: alpha,
                        rounded_to_zero: false,
                    },
                ),
            },
        };
        let mut result = [FiniteF32 {
            value: 0.0,
            rounded_to_zero: false,
        }; 4];
        for (out, numerator) in result[..3].iter_mut().zip(numerators) {
            *out = numerator
                .ratio_finite_f32(&divisor, limits)
                .map_err(ColorPairError::Conversion)?;
        }
        result[3] = alpha;
        Ok(ColorFiniteSample {
            property: self.property,
            value: ColorFiniteValue::Numeric(result),
        })
    }
}
fn premultiply(
    numerators: &[Decimal; 3],
    alpha: f32,
    limits: NumericLimits,
) -> Result<[Decimal; 4], ColorPairError> {
    let mut result = std::array::from_fn(|_| Decimal::zero());
    for (out, numerator) in result[..3].iter_mut().zip(numerators) {
        *out = numerator
            .checked_scale_f32(alpha, limits)
            .map_err(ColorPairError::Scaling)?;
    }
    result[3] = Decimal::from_finite_f32(alpha, limits).map_err(ColorPairError::Scaling)?;
    Ok(result)
}
/// Same-property pair semantics. Numeric pairs require at most12 independently
/// bounded calls. Invert compatibility is classified before numeric copying;
/// discrete selection borrows the whole original endpoint, including its alpha.
pub fn combine_color<'a>(
    start: &'a ColorAnimationEndpoint,
    end: &'a ColorAnimationEndpoint,
    operation: Combination,
    limits: NumericLimits,
) -> Result<CombinedColor<'a>, ColorPairError> {
    if start.property != end.property {
        return Err(ColorPairError::PropertyMismatch);
    }
    let selected = discrete_choice(operation).map_err(|_| ColorPairError::NonfiniteProgress)?;
    let value = match (start.component(), end.component()) {
        (
            ColorEndpointComponent::Numeric {
                numerators: a,
                alpha: aa,
            },
            ColorEndpointComponent::Numeric {
                numerators: b,
                alpha: ba,
            },
        ) => {
            let a = premultiply(a, aa, limits)?;
            let b = premultiply(b, ba, limits)?;
            let mut values = std::array::from_fn(|_| Decimal::zero());
            for (i, value) in values.iter_mut().enumerate() {
                *value = match operation {
                    Combination::Interpolate(p) => a[i]
                        .checked_interpolate(&b[i], p, limits)
                        .map_err(ColorPairError::Arithmetic)?,
                    Combination::Add | Combination::Accumulate => {
                        a[i].checked_add(&b[i], limits)
                            .map_err(ColorPairError::Numeric)?
                    }
                };
            }
            Pair::Numeric(values)
        }
        _ => Pair::Discrete(match selected {
            Endpoint::Start => start,
            Endpoint::End => end,
        }),
    };
    Ok(CombinedColor {
        property: start.property,
        value,
    })
}
#[derive(Debug)]
pub struct ColorAnimationEndpoint {
    property: PropertyId,
    value: Value,
}
#[derive(Debug, Clone, Copy)]
pub enum ColorEndpointComponent<'a> {
    /// Divide channel numerators by25,500 only at the appropriate final boundary.
    Numeric {
        numerators: &'a [Decimal; 3],
        alpha: f32,
    },
    Invert,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorEndpointAdmissionError {
    UnsupportedProperty,
    UnresolvedSystem(SystemColor),
    Numeric(NumericError),
    Scaling(DecimalScalingError),
}
impl std::fmt::Display for ColorEndpointAdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "color endpoint admission: {self:?}")
    }
}
impl std::error::Error for ColorEndpointAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(error) => Some(error),
            Self::Scaling(error) => Some(error),
            _ => None,
        }
    }
}
fn from_f32(
    value: f32,
    scale: f32,
    limits: NumericLimits,
) -> Result<Decimal, ColorEndpointAdmissionError> {
    Decimal::from_finite_f32(value, limits)
        .map_err(ColorEndpointAdmissionError::Scaling)?
        .checked_scale_f32(scale, limits)
        .map_err(ColorEndpointAdmissionError::Scaling)
}
impl ColorAnimationEndpoint {
    pub fn property(&self) -> PropertyId {
        self.property
    }
    pub fn component(&self) -> ColorEndpointComponent<'_> {
        match &self.value {
            Value::Numeric { numerators, alpha } => ColorEndpointComponent::Numeric {
                numerators,
                alpha: *alpha,
            },
            Value::Invert => ColorEndpointComponent::Invert,
        }
    }
    /// At most six bounded numeric calls, each with the supplied limits. System
    /// colors require the provider owner to resolve them explicitly; no palette is
    /// invented here. Exact authored RGB channels are never clipped or converted
    /// individually to f32. Input storage remains unchanged on any failure.
    pub fn from_computed(
        color: ComputedColor<'_>,
        limits: NumericLimits,
    ) -> Result<Self, ColorEndpointAdmissionError> {
        let mut numerators = std::array::from_fn(|_| Decimal::zero());
        let alpha = match color.value() {
            Color::Invert => {
                return Ok(Self {
                    property: color.property(),
                    value: Value::Invert,
                });
            }
            Color::System(key) => return Err(ColorEndpointAdmissionError::UnresolvedSystem(*key)),
            Color::Transparent => 0.0,
            Color::Srgb(rgb) => {
                for (out, v) in numerators.iter_mut().zip(rgb) {
                    *out = from_f32(f32::from(*v), 100.0, limits)?;
                }
                1.0
            }
            Color::IntegerRgb(rgb) => {
                for (out, v) in numerators.iter_mut().zip(rgb) {
                    *out = Decimal::from_integer(v, limits)
                        .map_err(ColorEndpointAdmissionError::Numeric)?
                        .checked_scale_f32(100.0, limits)
                        .map_err(ColorEndpointAdmissionError::Scaling)?;
                }
                1.0
            }
            Color::PercentageRgb(rgb) => {
                for (out, v) in numerators.iter_mut().zip(rgb) {
                    *out = v
                        .checked_scale_f32(255.0, limits)
                        .map_err(ColorEndpointAdmissionError::Scaling)?;
                }
                1.0
            }
        };
        Ok(Self {
            property: color.property(),
            value: Value::Numeric { numerators, alpha },
        })
    }
    /// Admit a resolved palette or captured presentation value. The caller owns
    /// source/provider identity and revisions; this validates only property scope
    /// and performs exact channel scaling in at most six bounded numeric calls.
    pub fn from_resolved(
        property: PropertyId,
        color: ResolvedLegacyColor,
        limits: NumericLimits,
    ) -> Result<Self, ColorEndpointAdmissionError> {
        if !supported(property) {
            return Err(ColorEndpointAdmissionError::UnsupportedProperty);
        }
        let mut numerators = std::array::from_fn(|_| Decimal::zero());
        for (out, v) in numerators.iter_mut().zip(color.rgb()) {
            *out = from_f32(v, 25_500.0, limits)?;
        }
        Ok(Self {
            property,
            value: Value::Numeric {
                numerators,
                alpha: color.alpha(),
            },
        })
    }
}
