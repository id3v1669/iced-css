//! Computed values for the explicitly supported single-keyword properties.
use super::{
    property::{Keyword, ParsedProperty, SpecifiedValue, finite_keyword_allowed},
    registry::PropertyId,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputedKeyword {
    property: PropertyId,
    keyword: Keyword,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedKeywordError {
    UnsupportedProperty,
    Unresolved,
    InvalidKeyword,
}
impl std::fmt::Display for ComputedKeywordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed single-keyword value: {self:?}")
    }
}
impl std::error::Error for ComputedKeywordError {}
impl ComputedKeyword {
    pub fn property(self) -> PropertyId {
        self.property
    }
    pub fn keyword(self) -> Keyword {
        self.keyword
    }
}
impl TryFrom<&ParsedProperty> for ComputedKeyword {
    type Error = ComputedKeywordError;
    fn try_from(parsed: &ParsedProperty) -> Result<Self, Self::Error> {
        // Explicit approved subset: all compute as specified and combine discretely.
        // Do not infer eligibility or computed semantics from a Keyword variant.
        if !matches!(
            parsed.property.descriptor().name,
            "background-attachment"
                | "background-repeat"
                | "border-collapse"
                | "caption-side"
                | "clear"
                | "direction"
                | "empty-cells"
                | "font-variant"
                | "list-style-position"
                | "list-style-type"
                | "overflow"
                | "position"
                | "table-layout"
                | "text-transform"
                | "unicode-bidi"
                | "white-space"
        ) {
            return Err(ComputedKeywordError::UnsupportedProperty);
        }
        let SpecifiedValue::Keyword(keyword) = parsed.value else {
            return Err(ComputedKeywordError::Unresolved);
        };
        if !finite_keyword_allowed(parsed.property, keyword) {
            return Err(ComputedKeywordError::InvalidKeyword);
        }
        Ok(Self {
            property: parsed.property,
            keyword,
        })
    }
}
