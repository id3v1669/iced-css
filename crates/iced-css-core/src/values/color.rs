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
    /// CSS Color 3: red, green, blue and an opacity below 1, each in 0 to 255. Written as
    /// `rgba()`, `hsla()`, `#rgba` or `#rrggbbaa`; an `hsl()` color is its bytes with 255.
    SrgbA([u8; 4]),
    /// CSS Color 3 `currentColor`: the element's `color`. Computation resolves it.
    CurrentColor,
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
            Self::Invalid => f.write_str("invalid color"),
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
    /// The arguments of `rgb()`, `rgba()`, `hsl()` or `hsla()`: three, or four with an
    /// opacity, separated by commas, up to the closing parenthesis.
    fn arguments(&mut self) -> Result<([Token<'a>; 3], Option<Token<'a>>), ColorError> {
        let a = self.next()?.ok_or(ColorError::Invalid)?.1;
        self.comma()?;
        let b = self.next()?.ok_or(ColorError::Invalid)?.1;
        self.comma()?;
        let c = self.next()?.ok_or(ColorError::Invalid)?.1;
        let mut alpha = None;
        loop {
            match self.next()? {
                Some((_, token)) if token.kind == TokenKind::CloseParen => break,
                // Q285: all operands exist; only the closing delimiter is missing.
                None => break,
                Some((_, token))
                    if alpha.is_none() && token.kind == TokenKind::Delim && token.raw == "," =>
                {
                    alpha = Some(self.next()?.ok_or(ColorError::Invalid)?.1);
                }
                _ => return Err(ColorError::Invalid),
            }
        }
        Ok(([a, b, c], alpha))
    }
    fn decimal(&self, raw: &str) -> Result<f32, ColorError> {
        Decimal::parse(raw, self.limits.numeric)
            .map_err(numeric_error)?
            .scaled_finite_f32(1.0, self.limits.numeric)
            .map(|converted| converted.value)
            .map_err(|_| ColorError::Invalid)
    }
    /// An opacity as a byte: a number in 0 to 1 or a percentage, clipped to that range.
    fn alpha(&self, token: Option<Token<'a>>) -> Result<u8, ColorError> {
        let Some(token) = token else {
            return Ok(u8::MAX);
        };
        let value = match token.kind {
            TokenKind::Number => self.decimal(token.raw)?,
            TokenKind::Percentage => self.decimal(&token.raw[..token.raw.len() - 1])? / 100.0,
            _ => return Err(ColorError::Invalid),
        };
        Ok((value.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
    fn rgb(&mut self) -> Result<Color, ColorError> {
        let ([a, b, c], alpha) = self.arguments()?;
        let alpha = self.alpha(alpha)?;
        if [a, b, c]
            .iter()
            .all(|token| token.kind == TokenKind::Number)
        {
            let parse = |token: Token<'_>| {
                Integer::parse(token.raw, self.limits.numeric).map_err(numeric_error)
            };
            let channels = [parse(a)?, parse(b)?, parse(c)?];
            Ok(if alpha == u8::MAX {
                Color::IntegerRgb(channels)
            } else {
                let [r, g, b] = channels.map(|channel| channel.clamped_byte());
                Color::SrgbA([r, g, b, alpha])
            })
        } else if [a, b, c]
            .iter()
            .all(|token| token.kind == TokenKind::Percentage)
        {
            let parse = |token: Token<'_>| {
                Decimal::parse(&token.raw[..token.raw.len() - 1], self.limits.numeric)
                    .map_err(numeric_error)
            };
            let channels = [parse(a)?, parse(b)?, parse(c)?];
            if alpha == u8::MAX {
                return Ok(Color::PercentageRgb(channels));
            }
            let mut bytes = [0, 0, 0, alpha];
            for (byte, channel) in bytes.iter_mut().zip(&channels) {
                // 4.3.6: out of range clips, including values too long to convert.
                let unit = match channel.percentage_finite_f32(1.0, self.limits.numeric) {
                    Ok(converted) => converted.value.clamp(0.0, 1.0),
                    Err(_) if channel.is_negative() => 0.0,
                    Err(_) => 1.0,
                };
                *byte = (unit * 255.0).round() as u8;
            }
            Ok(Color::SrgbA(bytes))
        } else {
            Err(ColorError::Invalid)
        }
    }
    /// CSS Color 3, 4.2.4: a hue in degrees, wrapping, with saturation and lightness as
    /// percentages clipped to 0 to 100.
    fn hsl(&mut self) -> Result<Color, ColorError> {
        let ([h, s, l], alpha) = self.arguments()?;
        let alpha = self.alpha(alpha)?;
        if h.kind != TokenKind::Number
            || s.kind != TokenKind::Percentage
            || l.kind != TokenKind::Percentage
        {
            return Err(ColorError::Invalid);
        }
        let percent = |parser: &Self, token: Token<'_>| {
            parser
                .decimal(&token.raw[..token.raw.len() - 1])
                .map(|value| (value / 100.0).clamp(0.0, 1.0))
        };
        let hue = self.decimal(h.raw)?.rem_euclid(360.0) / 60.0;
        let (saturation, lightness) = (percent(self, s)?, percent(self, l)?);
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
        let (r, g, b) = match hue as u8 {
            0 => (chroma, second, 0.0),
            1 => (second, chroma, 0.0),
            2 => (0.0, chroma, second),
            3 => (0.0, second, chroma),
            4 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        let lift = lightness - chroma / 2.0;
        let byte = |channel: f32| ((channel + lift).clamp(0.0, 1.0) * 255.0).round() as u8;
        Ok(if alpha == u8::MAX {
            Color::Srgb([byte(r), byte(g), byte(b)])
        } else {
            Color::SrgbA([byte(r), byte(g), byte(b), alpha])
        })
    }
}
/// Parse a `<color>`: the CSS2.2 domain, and from CSS Color 3 `rgba()`, `hsl()`, `hsla()`,
/// hexadecimal colors with an opacity, `transparent` and `currentColor`. Inherit and the
/// outline's invert keyword belong to their property's grammar, not this primitive.
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
    // Charge the scan even when the colour fails: shorthands retry other roles.
    let result = parse_color_tokens(&mut parser);
    *work = parser.work;
    result
}
fn parse_color_tokens(parser: &mut Parser<'_>) -> Result<Color, ColorError> {
    let (start, token) = parser.next()?.ok_or(ColorError::Invalid)?;
    let color = match token.kind {
        TokenKind::Ident => {
            let name = parser.name(token.raw, start, true)?;
            if name.eq_ignore_ascii_case("transparent") {
                Color::Transparent
            } else if name.eq_ignore_ascii_case("currentcolor") {
                Color::CurrentColor
            } else if let Some((_, rgb)) = data::NAMED
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
            if !matches!(name.len(), 3 | 4 | 6 | 8) || !name.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(ColorError::Invalid);
            }
            let digit = |index: usize| {
                (name.as_bytes()[index] as char)
                    .to_digit(16)
                    .expect("checked hex") as u8
            };
            // One digit a channel, doubled, or two; a fourth channel is the opacity.
            let short = name.len() < 6;
            let channel = |at: usize| {
                if short {
                    digit(at) * 17
                } else {
                    digit(at * 2) * 16 + digit(at * 2 + 1)
                }
            };
            let rgb = [channel(0), channel(1), channel(2)];
            match name.len() {
                4 | 8 if channel(3) < u8::MAX => Color::SrgbA([rgb[0], rgb[1], rgb[2], channel(3)]),
                _ => Color::Srgb(rgb),
            }
        }
        TokenKind::Function => {
            let name = parser.name(&token.raw[..token.raw.len() - 1], start, true)?;
            // CSS Color 4 makes each pair one function; the opacity is optional in both.
            match name.to_ascii_lowercase().as_str() {
                "rgb" | "rgba" => parser.rgb()?,
                "hsl" | "hsla" => parser.hsl()?,
                _ => return Err(ColorError::Invalid),
            }
        }
        _ => return Err(ColorError::Invalid),
    };
    if parser.next()?.is_some() {
        return Err(ColorError::Invalid);
    }
    Ok(color)
}
#[cfg(test)]
mod budget_tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn limits() -> ColorLimits {
        let n = |n| NonZeroUsize::new(n).unwrap();
        ColorLimits {
            syntax: StringLimits {
                work: n(100_000),
                retained_bytes: n(4096),
            },
            numeric: NumericLimits {
                digits: n(256),
                work: n(100_000),
            },
        }
    }

    #[test]
    fn failed_colours_charge_the_shared_work_budget() {
        for input in ["none", "#12", "hsl(1, 2, 3%)", "rgb(1, 2, x)", "red blue"] {
            let mut work = 100_000;
            assert_eq!(
                parse_color_budget(input, limits(), &mut work),
                Err(ColorError::Invalid),
                "{input}"
            );
            assert!(work < 100_000, "{input} scanned for free");
        }
        let mut work = 100_000;
        parse_color_budget("red", limits(), &mut work).unwrap();
        assert!(work < 100_000);
    }
}

#[cfg(test)]
mod color3_tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn parse(input: &str) -> Result<Color, ColorError> {
        let n = |n| NonZeroUsize::new(n).unwrap();
        parse_color(
            input,
            ColorLimits {
                syntax: StringLimits {
                    work: n(100_000),
                    retained_bytes: n(4096),
                },
                numeric: NumericLimits {
                    digits: n(256),
                    work: n(100_000),
                },
            },
        )
    }

    #[test]
    fn opacity_comes_as_a_fourth_argument_or_hex_digits() {
        assert_eq!(
            parse("rgba(255, 0, 0, 0.5)"),
            Ok(Color::SrgbA([255, 0, 0, 128]))
        );
        assert_eq!(
            parse("rgb(255, 0, 0, 50%)"),
            Ok(Color::SrgbA([255, 0, 0, 128]))
        );
        assert_eq!(
            parse("RGBA(100%, 0%, 50%, 0)"),
            Ok(Color::SrgbA([255, 0, 128, 0]))
        );
        // Out of range clips: the channels and the opacity.
        assert_eq!(
            parse("rgba(300, -5, 0, -1)"),
            Ok(Color::SrgbA([255, 0, 0, 0]))
        );
        assert_eq!(parse("#f008"), Ok(Color::SrgbA([255, 0, 0, 136])));
        assert_eq!(parse("#ff000080"), Ok(Color::SrgbA([255, 0, 0, 128])));
        // Opaque keeps the CSS 2.2 values exactly.
        assert_eq!(parse("#f00f"), Ok(Color::Srgb([255, 0, 0])));
        assert_eq!(parse("#ff0000ff"), Ok(Color::Srgb([255, 0, 0])));
        assert!(matches!(
            parse("rgba(1, 2, 3, 1)"),
            Ok(Color::IntegerRgb(_))
        ));
        assert!(matches!(parse("rgba(1, 2, 3)"), Ok(Color::IntegerRgb(_))));
        assert!(matches!(
            parse("rgb(1%, 2%, 3%, 2)"),
            Ok(Color::PercentageRgb(_))
        ));
        for bad in [
            "rgba(1, 2, 3, 0.5, 1)",
            "rgba(1, 2, 3,)",
            "rgba(1, 2%, 3, 0.5)",
            "rgba(1, 2, 3, x)",
            "rgba(1 2 3)",
            "#12345",
            "#1234567",
            "#ggg8",
        ] {
            assert_eq!(parse(bad), Err(ColorError::Invalid), "{bad}");
        }
    }

    #[test]
    fn hsl_converts_to_srgb_bytes() {
        for (input, rgb) in [
            ("hsl(0, 100%, 50%)", [255, 0, 0]),
            ("hsl(120, 100%, 50%)", [0, 255, 0]),
            ("hsl(240, 100%, 50%)", [0, 0, 255]),
            ("hsl(60, 100%, 50%)", [255, 255, 0]),
            ("hsl(180, 100%, 25%)", [0, 128, 128]),
            ("hsl(0, 0%, 50%)", [128, 128, 128]),
            ("hsl(0, 100%, 100%)", [255, 255, 255]),
            ("hsl(300, 50%, 0%)", [0, 0, 0]),
            // The hue wraps, and the percentages clip.
            ("hsl(480, 100%, 50%)", [0, 255, 0]),
            ("hsl(-120, 100%, 50%)", [0, 0, 255]),
            ("hsl(0, 200%, 50%)", [255, 0, 0]),
            ("HSLA(0, 100%, 50%, 1)", [255, 0, 0]),
        ] {
            assert_eq!(parse(input), Ok(Color::Srgb(rgb)), "{input}");
        }
        assert_eq!(
            parse("hsla(120, 100%, 50%, 0.25)"),
            Ok(Color::SrgbA([0, 255, 0, 64]))
        );
        for bad in [
            "hsl(0, 100, 50%)",
            "hsl(0%, 100%, 50%)",
            "hsl(0, 100%)",
            "hsl(red, 1%, 1%)",
        ] {
            assert_eq!(parse(bad), Err(ColorError::Invalid), "{bad}");
        }
    }

    #[test]
    fn transparent_and_current_color_are_colors() {
        assert_eq!(parse("transparent"), Ok(Color::Transparent));
        assert_eq!(parse("TRANSPARENT"), Ok(Color::Transparent));
        assert_eq!(parse("currentColor"), Ok(Color::CurrentColor));
        assert_eq!(parse("currentcolor"), Ok(Color::CurrentColor));
    }
}
