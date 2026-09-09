use super::{
    ColorLimits, LengthTarget, PropertyError, SpecifiedValue, TokenKind, decode_identifier_budget,
    length_property, significant,
};
use crate::syntax::SyntaxFault;
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<[LengthTarget; 4]>, PropertyError> {
    let mut offset = 0;
    let Some((start, function)) = significant(input, &mut offset, work)? else {
        return Ok(None);
    };
    if function.kind != TokenKind::Function || function.invalid_escape {
        return Ok(None);
    }
    let name = match decode_identifier_budget(
        &function.raw[..function.raw.len() - 1],
        limits.syntax.retained_bytes,
        work,
    ) {
        Ok(value) => value.text,
        Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
            error.offset += start;
            return Err(PropertyError::Syntax(error));
        }
        Err(_) => return Ok(None),
    };
    if !name.eq_ignore_ascii_case("rect") {
        return Ok(None);
    }
    drop(name);
    let mut edges = std::array::from_fn(|_| LengthTarget::Auto);
    let mut pending = significant(input, &mut offset, work)?;
    let mut commas = None;
    for (index, edge) in edges.iter_mut().enumerate() {
        let Some(token) = pending else {
            return Ok(None);
        };
        let Some(SpecifiedValue::Length(value)) =
            length_property::clip_component(token, limits, work)?
        else {
            return Ok(None);
        };
        *edge = value;
        pending = significant(input, &mut offset, work)?;
        if index < 3 {
            let Some((_, token)) = pending else {
                return Ok(None);
            };
            let comma = token.kind == TokenKind::Delim && token.raw == ",";
            if commas.is_some_and(|expected| expected != comma) {
                return Ok(None);
            }
            commas = Some(comma);
            if comma {
                pending = significant(input, &mut offset, work)?;
            }
        }
    }
    match pending {
        None => Ok(Some(edges)), // selected EOF closure, after all four operands only
        Some((_, token)) if token.kind == TokenKind::CloseParen => {
            Ok(significant(input, &mut offset, work)?
                .is_none()
                .then_some(edges))
        }
        _ => Ok(None),
    }
}
