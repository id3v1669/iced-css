use super::{ColorLimits, PropertyError, TokenKind, decode_identifier_budget, significant};
use crate::syntax::SyntaxFault;

/// Specified CSS2.2 decorations; propagation and rendering are separate stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDecoration(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDecorationError {
    Unresolved,
}
impl std::fmt::Display for TextDecorationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text-decoration requires a resolved decoration set")
    }
}
impl std::error::Error for TextDecorationError {}
impl TryFrom<&super::ParsedProperty> for TextDecoration {
    type Error = TextDecorationError;
    fn try_from(value: &super::ParsedProperty) -> Result<Self, Self::Error> {
        if value.property.descriptor().name != "text-decoration" {
            return Err(TextDecorationError::Unresolved);
        }
        match value.value {
            super::SpecifiedValue::TextDecoration(decoration) => Ok(decoration),
            _ => Err(TextDecorationError::Unresolved),
        }
    }
}
impl TextDecoration {
    pub fn underline(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn overline(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn line_through(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn blink(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<TextDecoration>, PropertyError> {
    let mut offset = 0;
    let mut bits = 0;
    while let Some((start, token)) = significant(input, &mut offset, work)? {
        if token.kind != TokenKind::Ident || token.invalid_escape {
            return Ok(None);
        }
        let name = match decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work) {
            Ok(value) => value.text,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(None),
        };
        if name.eq_ignore_ascii_case("none") {
            return if bits == 0 && significant(input, &mut offset, work)?.is_none() {
                Ok(Some(TextDecoration(0)))
            } else {
                Ok(None)
            };
        }
        let bit = if name.eq_ignore_ascii_case("underline") {
            1
        } else if name.eq_ignore_ascii_case("overline") {
            2
        } else if name.eq_ignore_ascii_case("line-through") {
            4
        } else if name.eq_ignore_ascii_case("blink") {
            8
        } else {
            return Ok(None);
        };
        if bits & bit != 0 {
            return Ok(None);
        }
        bits |= bit;
    }
    Ok((bits != 0).then_some(TextDecoration(bits)))
}
