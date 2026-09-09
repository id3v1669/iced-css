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
