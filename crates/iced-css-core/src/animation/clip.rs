//! Whole-property clip combination with exact numeric rectangle components.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::values::computed_clip::ComputedClip;
#[derive(Debug, Clone, Copy)]
pub struct CombinedClip(Option<[Option<CombinedNumber>; 4]>);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipSample {
    pub value: ComputedClip,
    pub clamped: [Option<Clamp>; 4],
}
impl CombinedClip {
    /// Final signed coordinate bounds, after whole-property combination.
    pub fn resolve(self, minimum: f32, maximum: f32) -> Result<ClipSample, NumberError> {
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(NumberError::InvalidRange);
        }
        let mut sides = [None; 4];
        let mut clamped = [None; 4];
        let value = if let Some(components) = self.0 {
            for (i, component) in components.into_iter().enumerate() {
                if let Some(component) = component {
                    let sample = component.resolve(minimum, maximum)?;
                    sides[i] = Some(sample.value);
                    clamped[i] = sample.clamped;
                }
            }
            ComputedClip::rectangle(sides).expect("numeric resolution gives finite coordinates")
        } else {
            ComputedClip::Auto
        };
        Ok(ClipSample { value, clamped })
    }
}
/// A single side type mismatch switches the whole property to discrete rules.
/// Auto sides remain unresolved; border-box substitution belongs to used geometry.
pub fn combine_clip(
    start: ComputedClip,
    end: ComputedClip,
    operation: Combination,
) -> Result<CombinedClip, NumberError> {
    let selected = discrete_choice(operation).map_err(|_| NumberError::NonfiniteProgress)?;
    let (a, b, operation) = match (start.sides(), end.sides()) {
        (Some(a), Some(b)) if a.iter().zip(b).all(|(x, y)| x.is_some() == y.is_some()) => {
            (a, b, operation)
        }
        _ => {
            let value = match selected {
                Endpoint::Start => start,
                Endpoint::End => end,
            };
            let Some(sides) = value.sides() else {
                return Ok(CombinedClip(None));
            };
            // A selected rectangle still receives final numeric range checks.
            (sides, sides, Combination::Interpolate(0.0))
        }
    };
    let mut components = [None; 4];
    for (i, (a, b)) in a.into_iter().zip(b).enumerate() {
        if let (Some(a), Some(b)) = (a, b) {
            components[i] = Some(combine_number(a, b, operation)?);
        }
    }
    Ok(CombinedClip(Some(components)))
}
