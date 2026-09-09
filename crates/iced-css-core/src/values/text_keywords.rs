//! Compute inherited white-space and text-transform keywords from cascade winners.
use super::{
    keyword::{ComputedKeyword, ComputedKeywordError},
    property::{ParsedProperty, SpecifiedValue, initial_finite_keyword},
    registry::PropertyId,
};
use crate::cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputedTextKeywords {
    white_space: ComputedKeyword,
    text_transform: ComputedKeyword,
}
impl ComputedTextKeywords {
    pub fn white_space(self) -> ComputedKeyword {
        self.white_space
    }
    pub fn text_transform(self) -> ComputedKeyword {
        self.text_transform
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ComputedTextKeywordsError {
    Defaulting(DefaultingError),
    InvalidInitial(PropertyId),
    Value {
        property: PropertyId,
        error: ComputedKeywordError,
    },
}
impl std::fmt::Display for ComputedTextKeywordsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed text keywords: {self:?}")
    }
}
impl std::error::Error for ComputedTextKeywordsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Defaulting(error) => Some(error),
            Self::Value { error, .. } => Some(error),
            Self::InvalidInitial(_) => None,
        }
    }
}
fn selected(
    name: &'static str,
    winners: &PropertyWinners<'_>,
    parent: Option<ComputedKeyword>,
) -> Result<ComputedKeyword, ComputedTextKeywordsError> {
    let property = PropertyId::lookup(name).expect("CSS22 text property is registered");
    let keyword = match winners
        .defaulted(property, parent.is_some())
        .map_err(ComputedTextKeywordsError::Defaulting)?
    {
        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => *keyword,
        DefaultedValueRef::Compute(_) => {
            return Err(ComputedTextKeywordsError::Value {
                property,
                error: ComputedKeywordError::Unresolved,
            });
        }
        DefaultedValueRef::Initial(property) => initial_finite_keyword(property)
            .ok_or(ComputedTextKeywordsError::InvalidInitial(property))?,
        DefaultedValueRef::ParentComputed => {
            return Ok(parent.expect("parent-computed default requires a parent"));
        }
    };
    ComputedKeyword::try_from(&ParsedProperty {
        property,
        value: SpecifiedValue::Keyword(keyword),
        important: false,
    })
    .map_err(|error| ComputedTextKeywordsError::Value { property, error })
}
/// `parent` is the actual source parent's completed pair. None denotes the root,
/// never an unavailable parent result or an anonymous formatting box.
/// This computes keywords only; source text and rich-text runs are unchanged.
///
/// # Errors
/// Preserves defaulting errors and rejects invalid finite initial recipes or
/// winning values that cannot compute to the selected property's keyword.
pub fn compute_text_keywords(
    winners: &PropertyWinners<'_>,
    parent: Option<&ComputedTextKeywords>,
) -> Result<ComputedTextKeywords, ComputedTextKeywordsError> {
    Ok(ComputedTextKeywords {
        white_space: selected("white-space", winners, parent.map(|p| p.white_space))?,
        text_transform: selected("text-transform", winners, parent.map(|p| p.text_transform))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_initial_recipes_require_the_finite_property_grammar() {
        for (name, expected) in [("white-space", "normal"), ("text-transform", "none")] {
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
