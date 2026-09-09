//! Compute CSS direction and unicode-bidi without assembling bidi paragraphs.
use super::{
    keyword::{ComputedKeyword, ComputedKeywordError},
    property::{ParsedProperty, SpecifiedValue, initial_finite_keyword},
    registry::PropertyId,
};
use crate::cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedDirection {
    LeftToRight,
    RightToLeft,
}
impl ComputedDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeftToRight => "ltr",
            Self::RightToLeft => "rtl",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedUnicodeBidi {
    Normal,
    Embed,
    BidiOverride,
}
impl ComputedUnicodeBidi {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Embed => "embed",
            Self::BidiOverride => "bidi-override",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputedBidiStyle {
    direction: ComputedDirection,
    unicode_bidi: ComputedUnicodeBidi,
}
impl ComputedBidiStyle {
    pub fn direction(self) -> ComputedDirection {
        self.direction
    }
    pub fn unicode_bidi(self) -> ComputedUnicodeBidi {
        self.unicode_bidi
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ComputedBidiError {
    Defaulting(DefaultingError),
    InvalidInitial(PropertyId),
    Value {
        property: PropertyId,
        error: ComputedKeywordError,
    },
}
impl std::fmt::Display for ComputedBidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed bidi style: {self:?}")
    }
}
impl std::error::Error for ComputedBidiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Defaulting(e) => Some(e),
            Self::Value { error, .. } => Some(error),
            Self::InvalidInitial(_) => None,
        }
    }
}
fn selected(
    property: PropertyId,
    winners: &PropertyWinners<'_>,
    has_parent: bool,
) -> Result<Option<ComputedKeyword>, ComputedBidiError> {
    let keyword = match winners
        .defaulted(property, has_parent)
        .map_err(ComputedBidiError::Defaulting)?
    {
        DefaultedValueRef::ParentComputed => return Ok(None),
        DefaultedValueRef::Initial(property) => {
            initial_finite_keyword(property).ok_or(ComputedBidiError::InvalidInitial(property))?
        }
        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => *keyword,
        DefaultedValueRef::Compute(_) => {
            return Err(ComputedBidiError::Value {
                property,
                error: ComputedKeywordError::Unresolved,
            });
        }
    };
    ComputedKeyword::try_from(&ParsedProperty {
        property,
        value: SpecifiedValue::Keyword(keyword),
        important: false,
    })
    .map(Some)
    .map_err(|error| ComputedBidiError::Value { property, error })
}
/// `parent` is the actual source parent's computed pair; None denotes the root.
/// This computes CSS keywords only, not a containing-block base, paragraph or UBA result.
///
/// # Errors
/// Preserves defaulting errors, rejects invalid finite initial recipes and winning
/// values which cannot compute to the fixed property's admitted keyword.
pub fn compute_bidi_style(
    winners: &PropertyWinners<'_>,
    parent: Option<&ComputedBidiStyle>,
) -> Result<ComputedBidiStyle, ComputedBidiError> {
    let property = PropertyId::lookup("direction").expect("CSS direction is registered");
    let direction = match selected(property, winners, parent.is_some())? {
        None => {
            parent
                .expect("parent-computed requires actual parent")
                .direction
        }
        Some(keyword) => match keyword.keyword().as_str() {
            "ltr" => ComputedDirection::LeftToRight,
            "rtl" => ComputedDirection::RightToLeft,
            _ => {
                return Err(ComputedBidiError::Value {
                    property,
                    error: ComputedKeywordError::InvalidKeyword,
                });
            }
        },
    };
    let property = PropertyId::lookup("unicode-bidi").expect("CSS unicode-bidi is registered");
    let unicode_bidi = match selected(property, winners, parent.is_some())? {
        None => {
            parent
                .expect("parent-computed requires actual parent")
                .unicode_bidi
        }
        Some(keyword) => match keyword.keyword().as_str() {
            "normal" => ComputedUnicodeBidi::Normal,
            "embed" => ComputedUnicodeBidi::Embed,
            "bidi-override" => ComputedUnicodeBidi::BidiOverride,
            _ => {
                return Err(ComputedBidiError::Value {
                    property,
                    error: ComputedKeywordError::InvalidKeyword,
                });
            }
        },
    };
    Ok(ComputedBidiStyle {
        direction,
        unicode_bidi,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_initial_recipes_require_the_finite_property_grammar() {
        for (name, expected) in [("direction", "ltr"), ("unicode-bidi", "normal")] {
            assert_eq!(
                initial_finite_keyword(PropertyId::lookup(name).unwrap())
                    .unwrap()
                    .as_str(),
                expected
            );
        }
        assert!(initial_finite_keyword(PropertyId::lookup("width").unwrap()).is_none());
    }
}
