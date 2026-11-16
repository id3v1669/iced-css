use super::{
    ColorLimits, Keyword, LengthTarget, NumericError, PropertyError, SpecifiedValue, Token,
    TokenKind, decode_identifier_budget, numeric, significant,
};
use crate::{
    syntax::{SyntaxError, SyntaxFault, SyntaxResource},
    values::Decimal,
};
#[derive(Clone, Copy)]
pub(super) struct Grammar {
    signed: bool,
    percentage: bool,
    number: bool,
    pair: bool,
    keywords: &'static [&'static str],
}
pub(super) fn grammar(name: &str) -> Option<Grammar> {
    let mut value = Grammar {
        signed: false,
        percentage: true,
        number: false,
        pair: false,
        keywords: &[],
    };
    match name {
        "width" | "height" => value.keywords = &["auto"],
        "min-width" | "min-height" => {}
        "max-width" | "max-height" => value.keywords = &["none"],
        "top" | "right" | "bottom" | "left" | "margin-top" | "margin-right" | "margin-bottom"
        | "margin-left" => {
            value.signed = true;
            value.keywords = &["auto"];
        }
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {}
        "text-indent" => value.signed = true,
        "letter-spacing" | "word-spacing" => {
            value.signed = true;
            value.percentage = false;
            value.keywords = &["normal"];
        }
        "border-top-width"
        | "border-right-width"
        | "border-bottom-width"
        | "border-left-width"
        | "outline-width" => {
            value.percentage = false;
            value.keywords = &["thin", "medium", "thick"];
        }
        "font-size" => {
            value.keywords = &[
                "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "larger",
                "smaller",
            ]
        }
        "vertical-align" => {
            value.signed = true;
            value.keywords = &[
                "baseline",
                "sub",
                "super",
                "top",
                "text-top",
                "middle",
                "bottom",
                "text-bottom",
            ];
        }
        "line-height" => {
            value.number = true;
            value.keywords = &["normal"];
        }
        "border-spacing" => {
            value.percentage = false;
            value.pair = true;
        }
        // CSS Backgrounds 3: a corner's horizontal radius, then its vertical one.
        "border-top-left-radius"
        | "border-top-right-radius"
        | "border-bottom-right-radius"
        | "border-bottom-left-radius" => value.pair = true,
        _ => return None,
    }
    Some(value)
}
fn component(
    (start, token): (usize, Token<'_>),
    grammar: Grammar,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    if token.invalid_escape {
        return Ok(None);
    }
    if token.kind == TokenKind::Ident {
        let name = match decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work) {
            Ok(value) => value.text,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(None),
        };
        return Ok(grammar
            .keywords
            .iter()
            .find(|keyword| keyword.eq_ignore_ascii_case(&name))
            .map(|keyword| {
                if *keyword == "auto" {
                    SpecifiedValue::Length(LengthTarget::Auto)
                } else {
                    SpecifiedValue::Keyword(Keyword(keyword))
                }
            }));
    }
    if token.kind == TokenKind::Number && grammar.number {
        return match Decimal::parse(token.raw, limits.numeric) {
            Ok(value) if grammar.signed || !value.is_negative() => {
                Ok(Some(SpecifiedValue::Number(value)))
            }
            Ok(_) | Err(NumericError::InvalidNumber | NumericError::UnsupportedUnit) => Ok(None),
            Err(error) => Err(PropertyError::Numeric(error)),
        };
    }
    let value = match token.kind {
        // A whole `calc(...)`, as `significant` groups it.
        TokenKind::Function => crate::values::calc::parse_calc(token.raw, work)
            .filter(|sum| grammar.percentage || sum.percent() == 0.0)
            .map(LengthTarget::Calc),
        TokenKind::Percentage if !grammar.percentage => return Ok(None),
        TokenKind::Number | TokenKind::Percentage => {
            numeric(LengthTarget::parse(token.raw, limits.numeric))?
        }
        TokenKind::Dimension => {
            let mut split = 0;
            while token
                .raw
                .as_bytes()
                .get(split)
                .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.'))
            {
                *work = work
                    .checked_sub(1)
                    .ok_or(PropertyError::Syntax(SyntaxError {
                        offset: start + split,
                        fault: SyntaxFault::Resource(SyntaxResource::Work),
                    }))?;
                split += 1;
            }
            let unit = match decode_identifier_budget(
                &token.raw[split..],
                limits.syntax.retained_bytes,
                work,
            ) {
                Ok(value) => value.text,
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start + split;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(None),
            };
            numeric(LengthTarget::from_dimension(
                &token.raw[..split],
                &unit,
                limits.numeric,
            ))?
        }
        _ => None,
    };
    Ok(value
        .filter(|value| grammar.signed || !value.is_negative())
        .map(SpecifiedValue::Length))
}
pub(super) fn parse(
    input: &str,
    grammar: Grammar,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    let mut offset = 0;
    let Some(first) = significant(input, &mut offset, work)? else {
        return Ok(None);
    };
    let second = significant(input, &mut offset, work)?;
    if (!grammar.pair && second.is_some()) || significant(input, &mut offset, work)?.is_some() {
        return Ok(None);
    }
    let Some(value) = component(first, grammar, limits, work)? else {
        return Ok(None);
    };
    if !grammar.pair {
        return Ok(Some(value));
    }
    let Some(SpecifiedValue::Length(other)) =
        component(second.unwrap_or(first), grammar, limits, work)?
    else {
        return Ok(None);
    };
    let SpecifiedValue::Length(value) = value else {
        return Ok(None);
    };
    Ok(Some(SpecifiedValue::LengthPair(value, other)))
}

pub(super) fn position_component(
    token: (usize, Token<'_>),
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    component(
        token,
        Grammar {
            signed: true,
            percentage: true,
            number: false,
            pair: false,
            keywords: &["left", "center", "right", "top", "bottom"],
        },
        limits,
        work,
    )
}

pub(super) fn clip_component(
    token: (usize, Token<'_>),
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    component(
        token,
        Grammar {
            signed: true,
            percentage: false,
            number: false,
            pair: false,
            keywords: &["auto"],
        },
        limits,
        work,
    )
}

/// The `border-radius` shorthand (CSS Backgrounds 3, 5.5): one to four horizontal radii,
/// then after a slash one to four vertical ones (the horizontal ones again when there is
/// no slash), each list filled out as the four-side shorthands are. The corners come top
/// left, top right, bottom right, bottom left. `None` is invalid.
pub(super) fn radius_shorthand(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<[(LengthTarget, LengthTarget); 4]>, PropertyError> {
    let grammar = Grammar {
        signed: false,
        percentage: true,
        number: false,
        pair: false,
        keywords: &[],
    };
    let mut lists: [Vec<LengthTarget>; 2] = [Vec::new(), Vec::new()];
    let mut list = 0;
    let mut offset = 0;
    while let Some(token) = significant(input, &mut offset, work)? {
        if token.1.kind == TokenKind::Delim && token.1.raw == "/" {
            if list == 1 || lists[0].is_empty() {
                return Ok(None);
            }
            list = 1;
            continue;
        }
        let Some(SpecifiedValue::Length(length)) = component(token, grammar, limits, work)? else {
            return Ok(None);
        };
        if lists[list].len() == 4 {
            return Ok(None);
        }
        lists[list].push(length);
    }
    if lists[0].is_empty() || (list == 1 && lists[1].is_empty()) {
        return Ok(None);
    }
    // One value is every corner; two are the two diagonals; three leave the last corner
    // to follow the second.
    let four = |values: &[LengthTarget]| -> [LengthTarget; 4] {
        let pick = |at: usize| values[at].clone();
        match values.len() {
            1 => [pick(0), pick(0), pick(0), pick(0)],
            2 => [pick(0), pick(1), pick(0), pick(1)],
            3 => [pick(0), pick(1), pick(2), pick(1)],
            _ => [pick(0), pick(1), pick(2), pick(3)],
        }
    };
    let across = four(&lists[0]);
    let down = if list == 1 {
        four(&lists[1])
    } else {
        across.clone()
    };
    let mut corners = across.into_iter().zip(down);
    Ok(Some(std::array::from_fn(|_| {
        corners.next().expect("four corners")
    })))
}
