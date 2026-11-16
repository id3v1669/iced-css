use super::{ConversionError, NonnegativeF32, RangeEvent};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSizeTable {
    entries: [f32; 7],
}
#[derive(Debug, PartialEq, Eq)]
pub enum FontSizeError {
    InvalidMedium,
    InvalidTable,
    InvalidParent,
    Arithmetic,
    UnresolvedValue,
    Conversion(ConversionError),
}
impl FontSizeTable {
    pub fn new(medium: f32) -> Result<Self, FontSizeError> {
        if !medium.is_finite() || medium <= 0.0 {
            return Err(FontSizeError::InvalidMedium);
        }
        let entries = [0.60f32, 0.75, 0.89, 1.00, 1.20, 1.50, 2.00].map(|factor| medium * factor);
        if entries
            .iter()
            .any(|entry| !entry.is_finite() || *entry <= 0.0)
            || entries.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(FontSizeError::InvalidTable);
        }
        Ok(Self { entries })
    }
    pub fn entries(self) -> [f32; 7] {
        self.entries
    }
    pub fn larger(self, parent: f32) -> Result<NonnegativeF32, FontSizeError> {
        self.relative(parent, true)
    }
    pub fn smaller(self, parent: f32) -> Result<NonnegativeF32, FontSizeError> {
        self.relative(parent, false)
    }
    fn relative(self, parent: f32, larger: bool) -> Result<NonnegativeF32, FontSizeError> {
        if !parent.is_finite() || parent < 0.0 {
            return Err(FontSizeError::InvalidParent);
        }
        if parent == 0.0 {
            return Ok(NonnegativeF32 {
                value: 0.0,
                event: None,
            });
        }
        let t = self.entries;
        if let Some(index) = t.iter().position(|entry| *entry == parent) {
            if larger && index < 6 {
                return Ok(NonnegativeF32 {
                    value: t[index + 1],
                    event: None,
                });
            }
            if !larger && index > 0 {
                return Ok(NonnegativeF32 {
                    value: t[index - 1],
                    event: None,
                });
            }
        }
        let boundary = if larger {
            if parent <= t[0] {
                Some((t[1], t[0]))
            } else if parent >= t[5] {
                Some((t[6], t[5]))
            } else {
                None
            }
        } else if parent <= t[1] {
            Some((t[0], t[1]))
        } else if parent >= t[6] {
            Some((t[5], t[6]))
        } else {
            None
        };
        let value = if let Some((numerator, denominator)) = boundary {
            let ratio = numerator / denominator;
            if !ratio.is_finite() || ratio <= 0.0 {
                return Err(FontSizeError::Arithmetic);
            }
            parent * ratio
        } else {
            let index = if larger {
                (0..5).find(|i| parent < t[i + 1])
            } else {
                (1..6).find(|i| parent < t[i + 1])
            }
            .ok_or(FontSizeError::Arithmetic)?;
            let fraction = (parent - t[index]) / (t[index + 1] - t[index]);
            if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
                return Err(FontSizeError::Arithmetic);
            }
            let (lower, upper) = if larger {
                (t[index + 1], t[index + 2])
            } else {
                (t[index - 1], t[index])
            };
            lower + fraction * (upper - lower)
        };
        if value.is_nan() || value < 0.0 {
            return Err(FontSizeError::Arithmetic);
        }
        if value.is_infinite() {
            return Ok(NonnegativeF32 {
                value: f32::MAX,
                event: Some(RangeEvent::UpperSaturation),
            });
        }
        Ok(NonnegativeF32 {
            value,
            event: (value == 0.0).then_some(RangeEvent::PositiveRoundedToZero),
        })
    }
}
impl std::fmt::Display for FontSizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMedium => f.write_str("font medium must be finite and positive"),
            Self::InvalidTable => {
                f.write_str("font keyword table must be positive, finite and strictly increasing")
            }
            Self::InvalidParent => f.write_str("parent font size must be finite and nonnegative"),
            Self::Arithmetic => {
                f.write_str("font keyword arithmetic produced an invalid intermediate")
            }
            Self::UnresolvedValue => f.write_str("font size requires a resolved font-size value"),
            Self::Conversion(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for FontSizeError {}

#[derive(Debug, Clone, Copy)]
pub struct FontSizeContext {
    pub table: FontSizeTable,
    pub parent_size: f32,
    pub parent_x_height: f32,
    /// What `rem` measures: the root element's computed font size, or the initial font
    /// size when this is the root's own (CSS Values 3, 5.1.1).
    pub root_size: f32,
}
pub fn compute_font_size(
    value: &super::property::SpecifiedValue,
    context: FontSizeContext,
    limits: super::NumericLimits,
) -> Result<NonnegativeF32, FontSizeError> {
    use super::{LengthTarget, RelativeUnit, property::SpecifiedValue};
    match value {
        SpecifiedValue::Keyword(keyword) => {
            if let Some(index) = [
                "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large",
            ]
            .iter()
            .position(|name| *name == keyword.as_str())
            {
                Ok(NonnegativeF32 {
                    value: context.table.entries[index],
                    event: None,
                })
            } else if keyword.as_str() == "larger" {
                context.table.larger(context.parent_size)
            } else if keyword.as_str() == "smaller" {
                context.table.smaller(context.parent_size)
            } else {
                Err(FontSizeError::UnresolvedValue)
            }
        }
        SpecifiedValue::Length(length) => match length {
            LengthTarget::Absolute(value) => value.to_nonnegative_f32(limits),
            LengthTarget::Relative(value, RelativeUnit::Em) => {
                value.scaled_nonnegative_f32(context.parent_size, limits)
            }
            LengthTarget::Relative(value, RelativeUnit::Ex) => {
                value.scaled_nonnegative_f32(context.parent_x_height, limits)
            }
            LengthTarget::Relative(value, RelativeUnit::Rem) => {
                value.scaled_nonnegative_f32(context.root_size, limits)
            }
            LengthTarget::Relative(value, RelativeUnit::Ch) => {
                value.scaled_nonnegative_f32(context.parent_size * 0.5, limits)
            }
            LengthTarget::Percentage(value) => {
                value.percentage_nonnegative_f32(context.parent_size, limits)
            }
            LengthTarget::Auto => return Err(FontSizeError::UnresolvedValue),
            // A percentage, `em` and `ch` are of the parent's size; never below zero.
            LengthTarget::Calc(sum) => {
                let size = sum.pixels()
                    + sum.percent() / 100.0 * context.parent_size
                    + sum.relative(RelativeUnit::Em) * context.parent_size
                    + sum.relative(RelativeUnit::Ex) * context.parent_x_height
                    + sum.relative(RelativeUnit::Rem) * context.root_size
                    + sum.relative(RelativeUnit::Ch) * context.parent_size * 0.5;
                if !size.is_finite() {
                    return Err(FontSizeError::UnresolvedValue);
                }
                Ok(NonnegativeF32 {
                    value: size.max(0.0),
                    event: None,
                })
            }
        }
        .map_err(FontSizeError::Conversion),
        _ => Err(FontSizeError::UnresolvedValue),
    }
}
