use super::{Decimal, Integer, NumericError, NumericLimits};
use crate::syntax::{
    StringLimits, SyntaxError, SyntaxFault, Token, TokenKind, decode_identifier_budget,
    decode_name_budget, scan_token_budget,
};
#[path = "color_data.rs"]
mod data;
#[derive(Debug, Clone, Copy)]
pub struct ColorLimits {
    pub syntax: StringLimits,
    pub numeric: NumericLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemColor(&'static str);
impl SystemColor {
    pub fn name(self) -> &'static str {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Transparent,
    Invert,
    Srgb([u8; 3]),
    IntegerRgb([Integer; 3]),
    PercentageRgb([Decimal; 3]),
    System(SystemColor),
}
#[derive(Debug, PartialEq, Eq)]
pub enum ColorError {
    Invalid,
    Syntax(SyntaxError),
    Numeric(NumericError),
}

impl std::fmt::Display for ColorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid CSS2.2 color"),
            Self::Syntax(error) => error.fmt(f),
            Self::Numeric(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ColorError {}
fn syntax_error(error: SyntaxError) -> ColorError {
    if matches!(error.fault, SyntaxFault::Resource(_)) {
        ColorError::Syntax(error)
    } else {
        ColorError::Invalid
    }
}
fn numeric_error(error: NumericError) -> ColorError {
    if matches!(
        error,
        NumericError::InvalidNumber | NumericError::UnsupportedUnit
    ) {
        ColorError::Invalid
    } else {
        ColorError::Numeric(error)
    }
}
struct Parser<'a> {
    input: &'a str,
    offset: usize,
    work: usize,
    limits: ColorLimits,
}
impl<'a> Parser<'a> {
    fn next(&mut self) -> Result<Option<(usize, Token<'a>)>, ColorError> {
        while self.offset < self.input.len() {
            let start = self.offset;
            let token = scan_token_budget(&self.input[start..], &mut self.work)
                .map_err(|mut error| {
                    error.offset += start;
                    syntax_error(error)
                })?
                .expect("nonempty input");
            self.offset += token.raw.len();
            if token.invalid_escape
                || matches!(token.kind, TokenKind::BadUri | TokenKind::BadString)
            {
                return Err(ColorError::Invalid);
            }
            if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
                return Ok(Some((start, token)));
            }
        }
        Ok(None)
    }
    fn name(&mut self, raw: &str, start: usize, identifier: bool) -> Result<String, ColorError> {
        let decoded = if identifier {
            decode_identifier_budget(raw, self.limits.syntax.retained_bytes, &mut self.work)
        } else {
            decode_name_budget(
                raw,
                self.limits.syntax.retained_bytes,
                &mut self.work,
                false,
            )
        };
        decoded.map(|value| value.text).map_err(|mut error| {
            error.offset += start;
            syntax_error(error)
        })
    }
    fn comma(&mut self) -> Result<(), ColorError> {
        if self
            .next()?
            .is_some_and(|(_, token)| token.kind == TokenKind::Delim && token.raw == ",")
        {
            Ok(())
        } else {
            Err(ColorError::Invalid)
        }
    }
    fn rgb(&mut self) -> Result<Color, ColorError> {
        let a = self.next()?.ok_or(ColorError::Invalid)?.1;
        self.comma()?;
        let b = self.next()?.ok_or(ColorError::Invalid)?.1;
        self.comma()?;
        let c = self.next()?.ok_or(ColorError::Invalid)?.1;
        match self.next()? {
            Some((_, token)) if token.kind == TokenKind::CloseParen => {}
            None => {} // Q285: all operands exist; only the closing delimiter is missing.
            _ => return Err(ColorError::Invalid),
        }
        if [a, b, c]
            .iter()
            .all(|token| token.kind == TokenKind::Number)
        {
            let parse = |token: Token<'_>| {
                Integer::parse(token.raw, self.limits.numeric).map_err(numeric_error)
            };
            Ok(Color::IntegerRgb([parse(a)?, parse(b)?, parse(c)?]))
        } else if [a, b, c]
            .iter()
            .all(|token| token.kind == TokenKind::Percentage)
        {
            let parse = |token: Token<'_>| {
                Decimal::parse(&token.raw[..token.raw.len() - 1], self.limits.numeric)
                    .map_err(numeric_error)
            };
            Ok(Color::PercentageRgb([parse(a)?, parse(b)?, parse(c)?]))
        } else {
            Err(ColorError::Invalid)
        }
    }
}
/// Parse the CSS2.2 `<color>` domain. Inherit and property-specific transparent or
/// invert keywords belong to their property's grammar, not this primitive.
/// System colors stay symbolic; integer and percentage RGB components stay exact.
/// Syntax scans/decoding share one work budget; each of the three possible numeric
/// operands uses the separately supplied numeric complexity budget.
pub fn parse_color(input: &str, limits: ColorLimits) -> Result<Color, ColorError> {
    let mut work = limits.syntax.work.get();
    parse_color_budget(input, limits, &mut work)
}
pub(super) fn parse_color_budget(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Color, ColorError> {
    let mut parser = Parser {
        input,
        offset: 0,
        work: *work,
        limits,
    };
    let (start, token) = parser.next()?.ok_or(ColorError::Invalid)?;
    let color = match token.kind {
        TokenKind::Ident => {
            let name = parser.name(token.raw, start, true)?;
            if let Some((_, rgb)) = data::NAMED
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&name))
            {
                Color::Srgb(*rgb)
            } else if let Some(key) = data::SYSTEM
                .iter()
                .find(|key| key.eq_ignore_ascii_case(&name))
            {
                Color::System(SystemColor(key))
            } else {
                return Err(ColorError::Invalid);
            }
        }
        TokenKind::Hash => {
            let name = parser.name(&token.raw[1..], start + 1, false)?;
            if !matches!(name.len(), 3 | 6) || !name.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ColorError::Invalid);
            }
            let digit = |index: usize| {
                (name.as_bytes()[index] as char)
                    .to_digit(16)
                    .expect("checked hex") as u8
            };
            let rgb = if name.len() == 3 {
                [digit(0) * 17, digit(1) * 17, digit(2) * 17]
            } else {
                [
                    digit(0) * 16 + digit(1),
                    digit(2) * 16 + digit(3),
                    digit(4) * 16 + digit(5),
                ]
            };
            Color::Srgb(rgb)
        }
        TokenKind::Function => {
            let name = parser.name(&token.raw[..token.raw.len() - 1], start, true)?;
            if !name.eq_ignore_ascii_case("rgb") {
                return Err(ColorError::Invalid);
            }
            parser.rgb()?
        }
        _ => return Err(ColorError::Invalid),
    };
    if parser.next()?.is_some() {
        return Err(ColorError::Invalid);
    }
    *work = parser.work;
    Ok(color)
}
