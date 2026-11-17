//! `box-shadow` (CSS Backgrounds and Borders 3, section 7.1).
use super::{
    ColorLimits, LengthTarget, PropertyError, SpecifiedValue, TokenKind, length_property,
    significant,
};
use crate::{
    syntax::{decode_identifier_budget, scan_token_budget},
    values::color::{Color, ColorError, parse_color_budget},
};

/// One shadow of a `box-shadow` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowValue {
    pub inset: bool,
    /// To the right, then down.
    pub offset: [LengthTarget; 2],
    pub blur: LengthTarget,
    pub spread: LengthTarget,
    /// `None` is the element's `color`.
    pub color: Option<Color>,
}

/// Where each top-level comma of `input` is.
pub(super) fn commas(input: &str, work: &mut usize) -> Result<Vec<usize>, PropertyError> {
    let mut found = Vec::new();
    let mut depth = 0usize;
    let mut offset = 0;
    while offset < input.len() {
        let token = scan_token_budget(&input[offset..], work)
            .map_err(|mut error| {
                error.offset += offset;
                PropertyError::Syntax(error)
            })?
            .expect("nonempty input");
        match token.kind {
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Delim if token.raw == "," && depth == 0 => found.push(offset),
            _ => {}
        }
        offset += token.raw.len();
    }
    Ok(found)
}

/// One shadow: `inset`, two to four lengths and a color, the lengths together and the
/// other two before or after them. `None` is invalid.
fn shadow(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<ShadowValue>, PropertyError> {
    let mut inset = false;
    let mut lengths: Vec<LengthTarget> = Vec::new();
    let mut lengths_done = false;
    let mut color = None;
    let mut offset = 0;
    while let Some((start, token)) = significant(input, &mut offset, work)? {
        if let Some(SpecifiedValue::Length(length)) =
            length_property::shadow_length((start, token), limits, work)?
        {
            if lengths_done || lengths.len() == 4 {
                return Ok(None);
            }
            lengths.push(length);
            continue;
        }
        // Anything else ends the run of lengths.
        lengths_done = !lengths.is_empty();
        if token.kind == TokenKind::Ident
            && decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work)
                .is_ok_and(|name| name.text.eq_ignore_ascii_case("inset"))
        {
            if inset {
                return Ok(None);
            }
            inset = true;
            continue;
        }
        if color.is_some() {
            return Ok(None);
        }
        // A color: one token, or a function through its closing parenthesis.
        if token.kind == TokenKind::Function {
            let mut depth = 1usize;
            while depth > 0 {
                let Some((_, inner)) = significant(input, &mut offset, work)? else {
                    break;
                };
                match inner.kind {
                    TokenKind::Function | TokenKind::OpenParen => depth += 1,
                    TokenKind::CloseParen => depth -= 1,
                    _ => {}
                }
            }
        }
        match parse_color_budget(&input[start..offset], limits, work) {
            Ok(parsed) => color = Some(parsed),
            Err(ColorError::Invalid) => return Ok(None),
            Err(ColorError::Syntax(error)) => return Err(PropertyError::Syntax(error)),
            Err(ColorError::Numeric(error)) => return Err(PropertyError::Numeric(error)),
        }
    }
    if lengths.len() < 2 || lengths.get(2).is_some_and(LengthTarget::is_negative) {
        return Ok(None);
    }
    let zero = || LengthTarget::parse("0", limits.numeric).expect("zero");
    let mut lengths = lengths.into_iter();
    let mut next = || lengths.next().unwrap_or_else(zero);
    Ok(Some(ShadowValue {
        inset,
        offset: [next(), next()],
        blur: next(),
        spread: next(),
        color: color.filter(|color| *color != Color::CurrentColor),
    }))
}

/// `none`, or a comma-separated list of shadows, first on top. `None` is invalid.
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ShadowValue>>, PropertyError> {
    if input.trim().eq_ignore_ascii_case("none") {
        return Ok(Some(Vec::new()));
    }
    let mut shadows = Vec::new();
    let mut from = 0;
    let mut ends = commas(input, work)?;
    ends.push(input.len());
    for end in ends {
        let Some(value) = shadow(&input[from..end], limits, work)? else {
            return Ok(None);
        };
        shadows.push(value);
        from = end + 1;
    }
    Ok(Some(shadows))
}
