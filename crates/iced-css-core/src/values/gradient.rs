//! `linear-gradient()` (CSS Images 3, section 3.1) as a `background-image`.
//!
//! Left out: color hints (a position alone between two stops), and the repeating and
//! radial gradients.
use super::{ColorLimits, LengthTarget, PropertyError, Token, TokenKind, significant};
use crate::values::{
    Decimal,
    color::{Color, ColorError, parse_color_budget},
};

/// The most color stops one gradient may hold.
const STOP_LIMIT: usize = 256;

/// An `<angle>` unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Deg,
    Rad,
    Grad,
    Turn,
}

/// Which way a linear gradient runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradientDirection {
    /// An angle, clockwise from up.
    Angle(Decimal, AngleUnit),
    /// `to` a side or corner: across, -1 left and 1 right, then down, -1 top and 1 bottom;
    /// 0 is neither.
    Toward(i8, i8),
}

/// One color stop; a position left out is placed between its neighbours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradientStop {
    /// `None` is `currentColor`.
    pub color: Option<Color>,
    pub position: Option<LengthTarget>,
}

/// A `linear-gradient()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearGradient {
    pub direction: GradientDirection,
    /// At least two.
    pub stops: Vec<GradientStop>,
}

/// The tokens inside the function, split at top-level commas.
fn arguments<'a>(
    input: &'a str,
    work: &mut usize,
) -> Result<Option<Vec<Vec<(usize, Token<'a>)>>>, PropertyError> {
    let mut offset = 0;
    let Some((_, first)) = significant(input, &mut offset, work)? else {
        return Ok(None);
    };
    if first.kind != TokenKind::Function || !first.raw.eq_ignore_ascii_case("linear-gradient(") {
        return Ok(None);
    }
    let mut parts = vec![Vec::new()];
    let mut depth = 0usize;
    let mut closed = false;
    while let Some((start, token)) = significant(input, &mut offset, work)? {
        if closed || token.invalid_escape || parts.len() > STOP_LIMIT {
            return Ok(None);
        }
        match token.kind {
            TokenKind::CloseParen if depth == 0 => {
                closed = true;
                continue;
            }
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth -= 1,
            TokenKind::Delim if token.raw == "," && depth == 0 => {
                parts.push(Vec::new());
                continue;
            }
            _ => {}
        }
        parts.last_mut().expect("one part").push((start, token));
    }
    Ok(closed.then_some(parts))
}

/// An `<angle>` token: a dimension in one of the four units, or a bare zero.
fn angle(token: &Token<'_>, limits: ColorLimits) -> Option<GradientDirection> {
    if token.kind == TokenKind::Number && token.raw.trim_start_matches(['+', '-']) == "0" {
        let zero = Decimal::parse("0", limits.numeric).ok()?;
        return Some(GradientDirection::Angle(zero, AngleUnit::Deg));
    }
    if token.kind != TokenKind::Dimension {
        return None;
    }
    let lower = token.raw.to_ascii_lowercase();
    let (unit, length) = [
        (AngleUnit::Grad, "grad"),
        (AngleUnit::Turn, "turn"),
        (AngleUnit::Deg, "deg"),
        (AngleUnit::Rad, "rad"),
    ]
    .into_iter()
    .find(|(_, suffix)| lower.ends_with(suffix))
    .map(|(unit, suffix)| (unit, suffix.len()))?;
    let number = &token.raw[..token.raw.len() - length];
    Some(GradientDirection::Angle(
        Decimal::parse(number, limits.numeric).ok()?,
        unit,
    ))
}

/// `to <side-or-corner>`.
fn toward(part: &[(usize, Token<'_>)]) -> Option<GradientDirection> {
    let [(_, to), rest @ ..] = part else {
        return None;
    };
    if to.kind != TokenKind::Ident || !to.raw.eq_ignore_ascii_case("to") || rest.is_empty() {
        return None;
    }
    let (mut across, mut down) = (None, None);
    for (_, token) in rest {
        if token.kind != TokenKind::Ident {
            return None;
        }
        match token.raw.to_ascii_lowercase().as_str() {
            "left" if across.is_none() => across = Some(-1),
            "right" if across.is_none() => across = Some(1),
            "top" if down.is_none() => down = Some(-1),
            "bottom" if down.is_none() => down = Some(1),
            _ => return None,
        }
    }
    (rest.len() <= 2).then(|| GradientDirection::Toward(across.unwrap_or(0), down.unwrap_or(0)))
}

/// One color stop: a color, then up to two positions (two make two stops).
fn stop(
    input: &str,
    part: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<GradientStop>>, PropertyError> {
    let Some(&(start, first)) = part.first() else {
        return Ok(None);
    };
    // The color: one token, or a function through its closing parenthesis.
    let mut taken = 1;
    if first.kind == TokenKind::Function {
        let mut depth = 1usize;
        while depth > 0 {
            let Some((_, token)) = part.get(taken) else {
                return Ok(None);
            };
            match token.kind {
                TokenKind::Function | TokenKind::OpenParen => depth += 1,
                TokenKind::CloseParen => depth -= 1,
                _ => {}
            }
            taken += 1;
        }
    }
    let last = part[taken - 1];
    let end = last.0 + last.1.raw.len();
    let color = match parse_color_budget(&input[start..end], limits, work) {
        Ok(color) => color,
        Err(ColorError::Invalid) => return Ok(None),
        Err(ColorError::Syntax(error)) => return Err(PropertyError::Syntax(error)),
        Err(ColorError::Numeric(error)) => return Err(PropertyError::Numeric(error)),
    };
    let color = (color != Color::CurrentColor).then_some(color);
    let mut positions = Vec::new();
    for &(at, token) in &part[taken..] {
        let Some(super::SpecifiedValue::Length(position)) =
            super::length_property::position_component((at, token), limits, work)?
        else {
            return Ok(None);
        };
        if matches!(position, LengthTarget::Auto) {
            return Ok(None);
        }
        positions.push(position);
    }
    if positions.len() > 2 {
        return Ok(None);
    }
    Ok(Some(if positions.is_empty() {
        vec![GradientStop {
            color,
            position: None,
        }]
    } else {
        positions
            .into_iter()
            .map(|position| GradientStop {
                color: color.clone(),
                position: Some(position),
            })
            .collect()
    }))
}

/// A whole `linear-gradient(...)`, `None` when invalid.
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<LinearGradient>, PropertyError> {
    let Some(parts) = arguments(input, work)? else {
        return Ok(None);
    };
    let mut parts = parts.into_iter().peekable();
    let direction = match parts.peek().map(Vec::as_slice) {
        Some([(_, only)]) if angle(only, limits).is_some() => {
            let direction = angle(only, limits);
            parts.next();
            direction
        }
        Some(part) if toward(part).is_some() => {
            let direction = toward(part);
            parts.next();
            direction
        }
        _ => None,
    }
    .unwrap_or(GradientDirection::Toward(0, 1));
    let mut stops = Vec::new();
    for part in parts {
        let Some(found) = stop(input, &part, limits, work)? else {
            return Ok(None);
        };
        stops.extend(found);
    }
    Ok((stops.len() >= 2 && stops.len() <= STOP_LIMIT)
        .then_some(LinearGradient { direction, stops }))
}
