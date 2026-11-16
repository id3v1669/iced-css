//! `transform` (CSS Transforms 1, section 7), its translations and scalings: the 2D
//! functions iced can draw.
//!
//! Left out: rotations, skews, `matrix()` and the 3D functions.
use super::{
    ColorLimits, LengthTarget, PropertyError, SpecifiedValue, Token, TokenKind, length_property,
    significant,
};
use crate::values::Decimal;

/// The most functions one `transform` may hold.
const FUNCTION_LIMIT: usize = 64;

/// One transform function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformFunction {
    /// Across, then down; a percentage is of the border box.
    Translate(LengthTarget, LengthTarget),
    /// Across, then down.
    Scale(Decimal, Decimal),
}

/// The arguments of the function at `at`, split at commas, and the index after it.
fn arguments<'t, 'a>(
    list: &'t [(usize, Token<'a>)],
    at: usize,
) -> Option<(Vec<&'t [(usize, Token<'a>)]>, usize)> {
    let mut parts = Vec::new();
    let mut start = at + 1;
    for (index, (_, token)) in list.iter().enumerate().skip(at + 1) {
        match token.kind {
            TokenKind::CloseParen => {
                parts.push(&list[start..index]);
                return Some((parts, index + 1));
            }
            TokenKind::Delim if token.raw == "," => {
                parts.push(&list[start..index]);
                start = index + 1;
            }
            TokenKind::Function | TokenKind::OpenParen => return None,
            _ => {}
        }
    }
    None
}

/// `none` (an empty list) or a list of functions; `None` is invalid.
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<TransformFunction>>, PropertyError> {
    let mut list = Vec::new();
    let mut offset = 0;
    while let Some(token) = significant(input, &mut offset, work)? {
        if token.1.invalid_escape || list.len() > FUNCTION_LIMIT * 8 {
            return Ok(None);
        }
        list.push(token);
    }
    if let [(_, only)] = list.as_slice()
        && only.kind == TokenKind::Ident
        && only.raw.eq_ignore_ascii_case("none")
    {
        return Ok(Some(Vec::new()));
    }
    let zero = || LengthTarget::parse("0", limits.numeric).expect("zero");
    let one = || Decimal::parse("1", limits.numeric).expect("one");
    let mut functions = Vec::new();
    let mut at = 0;
    while at < list.len() {
        let token = list[at].1;
        // A `calc()` is one token, but no transform function.
        if token.kind != TokenKind::Function
            || token.raw.len() < 5
            || functions.len() == FUNCTION_LIMIT
        {
            return Ok(None);
        }
        let Some((parts, next)) = arguments(&list, at) else {
            return Ok(None);
        };
        let mut lengths = Vec::new();
        let mut numbers = Vec::new();
        for part in &parts {
            let [(start, token)] = part else {
                return Ok(None);
            };
            if token.kind == TokenKind::Number {
                if let Ok(number) = Decimal::parse(token.raw, limits.numeric) {
                    numbers.push(number);
                }
            }
            if let Some(SpecifiedValue::Length(length)) =
                length_property::position_component((*start, *token), limits, work)?
            {
                lengths.push(length);
            }
        }
        let count = parts.len();
        let name = token.raw[..token.raw.len() - 1].to_ascii_lowercase();
        let function = match (name.as_str(), count) {
            ("translate", 1 | 2) if lengths.len() == count => TransformFunction::Translate(
                lengths[0].clone(),
                lengths.get(1).cloned().unwrap_or_else(zero),
            ),
            ("translatex", 1) if lengths.len() == 1 => {
                TransformFunction::Translate(lengths[0].clone(), zero())
            }
            ("translatey", 1) if lengths.len() == 1 => {
                TransformFunction::Translate(zero(), lengths[0].clone())
            }
            ("scale", 1 | 2) if numbers.len() == count => TransformFunction::Scale(
                numbers[0].clone(),
                numbers
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| numbers[0].clone()),
            ),
            ("scalex", 1) if numbers.len() == 1 => {
                TransformFunction::Scale(numbers[0].clone(), one())
            }
            ("scaley", 1) if numbers.len() == 1 => {
                TransformFunction::Scale(one(), numbers[0].clone())
            }
            _ => return Ok(None),
        };
        // A position keyword is no length here.
        if let TransformFunction::Translate(across, down) = &function
            && [across, down]
                .iter()
                .any(|length| matches!(length, LengthTarget::Auto))
        {
            return Ok(None);
        }
        functions.push(function);
        at = next;
    }
    Ok((!functions.is_empty()).then_some(functions))
}
