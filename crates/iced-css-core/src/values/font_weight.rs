//! CSS22 computed font weight and fixed-scale family weight selection.
use crate::values::property::SpecifiedValue;

/// One of the nine CSS2.2 weights. This is not a selected native face identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontWeight(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeightError;
impl std::fmt::Display for FontWeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("font weight requires a resolved CSS2.2 weight value")
    }
}
impl std::error::Error for FontWeightError {}
impl FontWeight {
    pub const NORMAL: Self = Self(4);
    pub const BOLD: Self = Self(7);
    pub fn new(number: u16) -> Result<Self, FontWeightError> {
        if !(100..=900).contains(&number) || !number.is_multiple_of(100) {
            return Err(FontWeightError);
        }
        Ok(Self((number / 100) as u8))
    }
    pub fn number(self) -> u16 {
        u16::from(self.0) * 100
    }
    fn bolder(self) -> Self {
        Self(match self.0 {
            1..=3 => 4,
            4 | 5 => 7,
            _ => 9,
        })
    }
    fn lighter(self) -> Self {
        Self(match self.0 {
            1..=5 => 1,
            6 | 7 => 4,
            _ => 7,
        })
    }
}
/// Compute after cascade/defaulting; the root caller supplies the initial400
/// parent basis. A missing native face must not rewrite this inherited value.
pub fn compute_font_weight(
    value: &SpecifiedValue,
    parent: FontWeight,
) -> Result<FontWeight, FontWeightError> {
    match value {
        SpecifiedValue::Keyword(keyword) => match keyword.as_str() {
            "normal" => Ok(FontWeight::NORMAL),
            "bold" => Ok(FontWeight::BOLD),
            "bolder" => Ok(parent.bolder()),
            "lighter" => Ok(parent.lighter()),
            _ => Err(FontWeightError),
        },
        SpecifiedValue::Integer(integer) if !integer.negative => {
            // Exact integers have normalized base-10^9 magnitudes. A valid weight
            // occupies one limb; inspect no input-sized string or digit sequence.
            let [number] = integer.magnitude.0.as_slice() else {
                return Err(FontWeightError);
            };
            let number = u16::try_from(*number).map_err(|_| FontWeightError)?;
            FontWeight::new(number)
        }
        _ => Err(FontWeightError),
    }
}
/// Available CSS-mapped weights for an already chosen family/style/variant.
/// Fixed nine-bit storage; face identity and same-weight tiebreaks are separate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AvailableFontWeights(u16);
impl AvailableFontWeights {
    pub fn insert(&mut self, weight: FontWeight) {
        self.0 |= 1 << (weight.0 - 1);
    }
    pub fn contains(self, weight: FontWeight) -> bool {
        self.0 & (1 << (weight.0 - 1)) != 0
    }
    fn first(self, order: impl Iterator<Item = u8>) -> Option<FontWeight> {
        order.map(FontWeight).find(|weight| self.contains(*weight))
    }
    /// Exact match first, then the CSS22 weight-specific search. Empty sets have
    /// no match; this does not select another family or synthesize a face.
    pub fn select(self, desired: FontWeight) -> Option<FontWeight> {
        if self.contains(desired) {
            return Some(desired);
        }
        let n = desired.0;
        match n {
            1..=3 => self.first((1..n).rev().chain(n + 1..=9)),
            6..=9 => self.first((n + 1..=9).chain((1..n).rev())),
            _ => self.first([if n == 4 { 5 } else { 4 }, 3, 2, 1, 6, 7, 8, 9].into_iter()),
        }
    }
}
