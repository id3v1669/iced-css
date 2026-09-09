//! Exact legacy-sRGB pair arithmetic and finite inverse conversion, before gamut mapping.
use super::combine::Combination;
use crate::values::{
    ConversionError, Decimal, DecimalArithmeticError, DecimalScalingError, FiniteF32, NumericError,
    NumericLimits,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedLegacyColor {
    rgb: [f32; 3],
    alpha: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorEndpointError {
    NonfiniteChannel,
    InvalidAlpha,
}
impl std::fmt::Display for ColorEndpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolved legacy color: {self:?}")
    }
}
impl std::error::Error for ColorEndpointError {}
impl ResolvedLegacyColor {
    /// Channels are gamma-encoded sRGB, not linear-light or premultiplied. They
    /// may be out of gamut. Alpha must already have resolved to its 0..1 domain.
    /// Property admission, system colors and invert remain separate concerns.
    pub fn new(rgb: [f32; 3], alpha: f32) -> Result<Self, ColorEndpointError> {
        if rgb.iter().any(|v| !v.is_finite()) {
            return Err(ColorEndpointError::NonfiniteChannel);
        }
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(ColorEndpointError::InvalidAlpha);
        }
        let zero = |v| if v == 0.0 { 0.0 } else { v };
        Ok(Self {
            rgb: rgb.map(zero),
            alpha: zero(alpha),
        })
    }
    pub fn rgb(self) -> [f32; 3] {
        self.rgb
    }
    pub fn alpha(self) -> f32 {
        self.alpha
    }
    fn premultiply(self, limits: NumericLimits) -> Result<[Decimal; 4], ColorCombinationError> {
        let mut values = std::array::from_fn(|_| Decimal::zero());
        for (out, channel) in values[..3].iter_mut().zip(self.rgb) {
            *out = Decimal::from_finite_f32(channel, limits)
                .map_err(ColorCombinationError::Scaling)?
                .checked_scale_f32(self.alpha, limits)
                .map_err(ColorCombinationError::Scaling)?;
        }
        values[3] =
            Decimal::from_finite_f32(self.alpha, limits).map_err(ColorCombinationError::Scaling)?;
        Ok(values)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCombinationError {
    NonfiniteProgress,
    Numeric(NumericError),
    Scaling(DecimalScalingError),
    Arithmetic(DecimalArithmeticError),
}
impl std::fmt::Display for ColorCombinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "legacy color combination: {self:?}")
    }
}
impl std::error::Error for ColorCombinationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(error) => Some(error),
            Self::Scaling(error) => Some(error),
            Self::Arithmetic(error) => Some(error),
            Self::NonfiniteProgress => None,
        }
    }
}
#[derive(Debug)]
pub struct CombinedLegacyColor([Decimal; 4]);
impl CombinedLegacyColor {
    /// Intermediate unpremultiplied R/G/B and alpha, with one rounding each.
    /// Exact zero alpha retains premultiplied channels; nonzero alpha is used for
    /// division even if it later rounds to zero. No gamut mapping or alpha clamp
    /// occurs. Exact inverse components outside finite-f32 range are rejected;
    /// this is not a final property/renderer conversion. Each of the four calls
    /// uses the supplied numeric bounds independently.
    pub fn to_finite_unpremultiplied(
        &self,
        limits: NumericLimits,
    ) -> Result<[FiniteF32; 4], ConversionError> {
        let alpha = &self.0[3];
        let mut values = [FiniteF32 {
            value: 0.0,
            rounded_to_zero: false,
        }; 4];
        for (out, channel) in values[..3].iter_mut().zip(&self.0[..3]) {
            *out = if alpha.is_zero() {
                channel.scaled_finite_f32(1.0, limits)?
            } else {
                channel.ratio_finite_f32(alpha, limits)?
            };
        }
        values[3] = alpha.scaled_finite_f32(1.0, limits)?;
        Ok(values)
    }

    /// Exact premultiplied R/G/B and alpha. No inverse conversion or final range
    /// enforcement has occurred. At exact zero alpha, the inverse rule retains
    /// these channels unchanged; otherwise it divides by the interpolated alpha.
    pub fn components(&self) -> &[Decimal; 4] {
        &self.0
    }
}
/// At most18 numeric calls, each with the supplied digit/work bound. Interpolation
/// may extrapolate; addition and accumulation sum premultiplied components. This
/// is not a final presentation color and must not be sent directly to a renderer.
pub fn combine_legacy_color(
    start: ResolvedLegacyColor,
    end: ResolvedLegacyColor,
    operation: Combination,
    limits: NumericLimits,
) -> Result<CombinedLegacyColor, ColorCombinationError> {
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(ColorCombinationError::NonfiniteProgress);
    }
    let start = start.premultiply(limits)?;
    let end = end.premultiply(limits)?;
    let mut output = std::array::from_fn(|_| Decimal::zero());
    for (i, value) in output.iter_mut().enumerate() {
        *value = match operation {
            Combination::Interpolate(p) => start[i]
                .checked_interpolate(&end[i], p, limits)
                .map_err(ColorCombinationError::Arithmetic)?,
            Combination::Add | Combination::Accumulate => start[i]
                .checked_add(&end[i], limits)
                .map_err(ColorCombinationError::Numeric)?,
        };
    }
    Ok(CombinedLegacyColor(output))
}
