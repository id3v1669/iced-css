use cssparser::{Delimiter, Parser, ParserInput, Token};

use crate::resolve::{Length, MarginValue, Margins, Paddings};

#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    Width(Length),
    Height(Length),
    MinWidth(Length),
    MaxWidth(Length),
    MinHeight(Length),
    MaxHeight(Length),
    Margin(Margins),
    MarginTop(MarginValue),
    MarginRight(MarginValue),
    MarginBottom(MarginValue),
    MarginLeft(MarginValue),
    Padding(Paddings),
    PaddingTop(f32),
    PaddingRight(f32),
    PaddingBottom(f32),
    PaddingLeft(f32),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub class: String,
    pub declarations: Vec<Property>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// A stylesheet syntax error, located as cssparser reports it.
///
/// The two location fields use different bases; `Display` prints both
/// 1-based.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    /// 0-based line: the first line of the input is 0.
    pub line: u32,
    /// 1-based column within the line, counted in UTF-16 code units
    /// (not bytes or chars).
    pub column: u32,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CSS parse error at line {}, column {}: {}",
            self.line + 1,
            self.column,
            self.message
        )
    }
}

impl std::error::Error for ParseError {}

type Error<'i> = cssparser::ParseError<'i, String>;

fn error<'i, T>(parser: &Parser<'i, '_>, message: impl Into<String>) -> Result<T, Error<'i>> {
    Err(parser.new_custom_error(message.into()))
}

pub fn parse(css: &str) -> Result<Stylesheet, ParseError> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    parse_rules(&mut parser).map_err(|e| {
        let location = e.location;
        ParseError {
            message: match e.kind {
                cssparser::ParseErrorKind::Custom(message) => message,
                cssparser::ParseErrorKind::Basic(basic) => format!("{basic:?}"),
            },
            line: location.line,
            column: location.column,
        }
    })
}

fn parse_rules<'i>(parser: &mut Parser<'i, '_>) -> Result<Stylesheet, Error<'i>> {
    let mut rules = Vec::new();

    loop {
        if parser.is_exhausted() {
            return Ok(Stylesheet { rules });
        }
        match parser.next()?.clone() {
            Token::Delim('.') => {}
            other => return error(parser, format!("unsupported selector start: {other:?}")),
        }
        // A class selector is the '.' immediately followed by its name: no
        // whitespace or comment in between.
        let class = match parser.next_including_whitespace_and_comments()?.clone() {
            Token::Ident(class) => class,
            other => return error(parser, format!("expected class name after '.': {other:?}")),
        };
        parser.expect_curly_bracket_block().map_err(Error::from)?;
        let declarations = parser.parse_nested_block(parse_declarations)?;

        rules.push(Rule {
            class: class.to_string(),
            declarations,
        });
    }
}

fn parse_declarations<'i>(parser: &mut Parser<'i, '_>) -> Result<Vec<Property>, Error<'i>> {
    let mut declarations = Vec::new();

    while !parser.is_exhausted() {
        // Empty are valid, just skip them.
        if parser.try_parse(|p| p.expect_semicolon()).is_ok() {
            continue;
        }
        // Names are case-insensitive.
        let name = parser
            .expect_ident_cloned()
            .map_err(Error::from)?
            .to_ascii_lowercase();
        parser.expect_colon().map_err(Error::from)?;

        let declaration =
            parser.parse_until_after(Delimiter::Semicolon, |value| parse_value(&name, value))?;

        declarations.push(declaration);
    }

    Ok(declarations)
}

fn parse_value<'i>(name: &str, parser: &mut Parser<'i, '_>) -> Result<Property, Error<'i>> {
    let property = match name {
        "width" => Property::Width(parse_length(parser)?),
        "height" => Property::Height(parse_length(parser)?),
        "min-width" => Property::MinWidth(parse_length(parser)?),
        "max-width" => Property::MaxWidth(parse_max_length(parser)?),
        "min-height" => Property::MinHeight(parse_length(parser)?),
        "max-height" => Property::MaxHeight(parse_max_length(parser)?),
        "margin" => Property::Margin(parse_margin_shorthand(parser)?),
        "margin-top" => Property::MarginTop(parse_margin_value(parser)?),
        "margin-right" => Property::MarginRight(parse_margin_value(parser)?),
        "margin-bottom" => Property::MarginBottom(parse_margin_value(parser)?),
        "margin-left" => Property::MarginLeft(parse_margin_value(parser)?),
        "padding" => Property::Padding(parse_padding_shorthand(parser)?),
        "padding-top" => Property::PaddingTop(parse_padding_value(parser)?),
        "padding-right" => Property::PaddingRight(parse_padding_value(parser)?),
        "padding-bottom" => Property::PaddingBottom(parse_padding_value(parser)?),
        "padding-left" => Property::PaddingLeft(parse_padding_value(parser)?),
        _ => {
            if parser.is_exhausted() {
                return error(parser, "empty value");
            }
            // Consume (and thereby syntax-check) the rest of the value.
            check_value_tokens(parser)?;
            Property::Unsupported(name.to_string())
        }
    };

    if !matches!(property, Property::Unsupported(_)) {
        parser.expect_exhausted().map_err(Error::from)?;
    }

    Ok(property)
}

/// Consumes the rest of a value, rejecting the tokens that are parse
/// errors in css-syntax: bad strings and URLs and unmatched closers,
/// including inside nested blocks.
fn check_value_tokens<'i>(parser: &mut Parser<'i, '_>) -> Result<(), Error<'i>> {
    while !parser.is_exhausted() {
        match parser.next()?.clone() {
            token @ (Token::BadString(_)
            | Token::BadUrl(_)
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket) => {
                return error(parser, format!("invalid token in value: {token:?}"));
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => parser.parse_nested_block(check_value_tokens)?,
            _ => {}
        }
    }
    Ok(())
}

fn parse_length<'i>(parser: &mut Parser<'i, '_>) -> Result<Length, Error<'i>> {
    match parse_signed_length(parser)? {
        Length::Px(v) if v < 0.0 => error(parser, "negative length"),
        Length::Percent(f) if f < 0.0 => error(parser, "negative percentage"),
        length => Ok(length),
    }
}

fn parse_signed_length<'i>(parser: &mut Parser<'i, '_>) -> Result<Length, Error<'i>> {
    match parser.next()?.clone() {
        // cssparser overflows out-of-range numbers to infinity.
        Token::Dimension { value, .. }
        | Token::Percentage {
            unit_value: value, ..
        } if !value.is_finite() => error(parser, "length out of range"),
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Ok(Length::Px(value))
        }
        Token::Dimension { unit, .. } => error(parser, format!("unsupported length unit: {unit}")),
        Token::Percentage { unit_value, .. } => Ok(Length::Percent(unit_value)),
        Token::Number { value: 0.0, .. } => Ok(Length::Px(0.0)),
        Token::Number { .. } => error(parser, "non-zero lengths require a unit"),
        Token::Ident(ident) if ident.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
        other => error(parser, format!("expected a length, got {other:?}")),
    }
}

fn parse_max_length<'i>(parser: &mut Parser<'i, '_>) -> Result<Length, Error<'i>> {
    if parser
        .try_parse(|p| p.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(Length::Auto);
    }
    match parse_length(parser)? {
        Length::Auto => error(parser, "max-width/max-height take `none`, not `auto`"),
        length => Ok(length),
    }
}

fn parse_margin_value<'i>(parser: &mut Parser<'i, '_>) -> Result<MarginValue, Error<'i>> {
    match parse_signed_length(parser)? {
        Length::Px(value) => Ok(MarginValue::Px(value)),
        Length::Auto => Ok(MarginValue::Auto),
        Length::Percent(_) => error(parser, "percentage margins are not supported yet"),
    }
}

fn parse_padding_value<'i>(parser: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    match parse_length(parser)? {
        Length::Px(value) => Ok(value),
        Length::Auto => error(parser, "padding does not accept `auto`"),
        Length::Percent(_) => error(parser, "percentage padding is not supported yet"),
    }
}

fn parse_padding_shorthand<'i>(parser: &mut Parser<'i, '_>) -> Result<Paddings, Error<'i>> {
    let mut values = Vec::new();
    while !parser.is_exhausted() {
        if values.len() == 4 {
            return error(parser, "padding shorthand takes at most 4 values");
        }
        values.push(parse_padding_value(parser)?);
    }

    Ok(match values[..] {
        [all] => Paddings {
            top: all,
            right: all,
            bottom: all,
            left: all,
        },
        [vertical, horizontal] => Paddings {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        },
        [top, horizontal, bottom] => Paddings {
            top,
            right: horizontal,
            bottom,
            left: horizontal,
        },
        [top, right, bottom, left] => Paddings {
            top,
            right,
            bottom,
            left,
        },
        [] => return error(parser, "padding needs at least 1 value"),
        _ => unreachable!(),
    })
}

fn parse_margin_shorthand<'i>(parser: &mut Parser<'i, '_>) -> Result<Margins, Error<'i>> {
    let mut values = Vec::new();
    while !parser.is_exhausted() {
        if values.len() == 4 {
            return error(parser, "margin max 4 values");
        }
        values.push(parse_margin_value(parser)?);
    }

    Ok(match values[..] {
        [all] => Margins {
            top: all,
            right: all,
            bottom: all,
            left: all,
        },
        [vertical, horizontal] => Margins {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        },
        [top, horizontal, bottom] => Margins {
            top,
            right: horizontal,
            bottom,
            left: horizontal,
        },
        [top, right, bottom, left] => Margins {
            top,
            right,
            bottom,
            left,
        },
        [] => return error(parser, "margin needs at least 1 value"),
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_class_rule() {
        let sheet = parse(".btn { width: 50px; height: 50px; }").unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].class, "btn");
        assert_eq!(
            sheet.rules[0].declarations,
            vec![
                Property::Width(Length::Px(50.0)),
                Property::Height(Length::Px(50.0)),
            ]
        );
    }

    #[test]
    fn last_declaration_without_semicolon() {
        let sheet = parse(".a { width: 10px }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::Width(Length::Px(10.0))]
        );
    }

    #[test]
    fn percent_and_auto() {
        let sheet = parse(".a { width: 50%; height: auto; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![
                Property::Width(Length::Percent(0.5)),
                Property::Height(Length::Auto),
            ]
        );
    }

    #[test]
    fn unitless_zero_is_allowed() {
        let sheet = parse(".a { width: 0; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::Width(Length::Px(0.0))]
        );
    }

    #[test]
    fn unitless_nonzero_is_an_error() {
        assert!(parse(".a { width: 100; }").is_err());
    }

    #[test]
    fn margin_shorthands() {
        let sheet = parse(
            ".a1 { margin: 10px; } .a2 { margin: 10px 20px; } \
             .a3 { margin: 10px 20px 30px; } .a4 { margin: 10px 20px 30px 40px; }",
        )
        .unwrap();
        let margins: Vec<Margins> = sheet
            .rules
            .iter()
            .map(|r| match r.declarations[0] {
                Property::Margin(m) => m,
                _ => panic!("expected margin"),
            })
            .collect();

        let px = MarginValue::Px;
        assert_eq!(
            margins[0],
            Margins {
                top: px(10.0),
                right: px(10.0),
                bottom: px(10.0),
                left: px(10.0)
            }
        );
        assert_eq!(
            margins[1],
            Margins {
                top: px(10.0),
                right: px(20.0),
                bottom: px(10.0),
                left: px(20.0)
            }
        );
        assert_eq!(
            margins[2],
            Margins {
                top: px(10.0),
                right: px(20.0),
                bottom: px(30.0),
                left: px(20.0)
            }
        );
        assert_eq!(
            margins[3],
            Margins {
                top: px(10.0),
                right: px(20.0),
                bottom: px(30.0),
                left: px(40.0)
            }
        );
    }

    #[test]
    fn min_max_height() {
        let sheet = parse(".a { min-height: 10px; max-height: 20px; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![
                Property::MinHeight(Length::Px(10.0)),
                Property::MaxHeight(Length::Px(20.0)),
            ]
        );
    }

    #[test]
    fn padding_shorthands() {
        let sheet =
            parse(".a2 { padding: 10px 20px; } .a4 { padding: 10px 20px 30px 40px; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::Padding(Paddings {
                top: 10.0,
                right: 20.0,
                bottom: 10.0,
                left: 20.0
            })]
        );
        assert_eq!(
            sheet.rules[1].declarations,
            vec![Property::Padding(Paddings {
                top: 10.0,
                right: 20.0,
                bottom: 30.0,
                left: 40.0
            })]
        );
    }

    #[test]
    fn padding_longhand() {
        let sheet = parse(".a { padding-left: 15px; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::PaddingLeft(15.0)]
        );
    }

    #[test]
    fn padding_auto_is_an_error() {
        assert!(parse(".a { padding: auto; }").is_err());
    }

    #[test]
    fn padding_five_values_is_an_error() {
        assert!(parse(".a { padding: 1px 2px 3px 4px 5px; }").is_err());
    }

    #[test]
    fn negative_lengths_are_invalid_except_margins() {
        assert!(parse(".a { padding: -5px; }").is_err());
        assert!(parse(".a { width: -5px; }").is_err());
        assert!(parse(".a { min-height: -5%; }").is_err());
        // negative margins are valid
        assert_eq!(
            parse(".a { margin-left: -5px; }").unwrap().rules[0].declarations,
            vec![Property::MarginLeft(MarginValue::Px(-5.0))]
        );
    }

    #[test]
    fn unsupported_valid_css_errors_for_now() {
        // valid CSS that is not implemented yet: pinned as errors so the
        // behavior change is visible when they land (see unimplemented.rs)
        assert!(parse(".a { margin: 10%; }").is_err());
        assert!(parse(".a { padding: 10%; }").is_err());
        assert!(parse(".a { width: 10px !important; }").is_err());
    }

    #[test]
    fn margin_five_values_is_an_error() {
        assert!(parse(".m { margin: 1px 2px 3px 4px 5px; }").is_err());
    }

    #[test]
    fn margin_zero_auto() {
        let sheet = parse(".m { margin: 0 auto; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::Margin(Margins {
                top: MarginValue::Px(0.0),
                right: MarginValue::Auto,
                bottom: MarginValue::Px(0.0),
                left: MarginValue::Auto,
            })]
        );
    }

    #[test]
    fn margin_unitless_nonzero_is_an_error() {
        assert!(parse(".m { margin: 10 auto; }").is_err());
    }

    #[test]
    fn unsupported_property_is_carried() {
        let sheet = parse(".a { background-color: #ffffff; width: 10px; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![
                Property::Unsupported("background-color".to_string()),
                Property::Width(Length::Px(10.0)),
            ]
        );
    }

    #[test]
    fn malformed_unsupported_value_is_an_error() {
        for css in [
            ".a { color: red) ; width: 10px; }",
            ".a { color: ] ; }",
            ".a { color: ; }",
            ".a { color: }",
            ".a { color: rgb(1, 2]) ; }",
            ".a { content: \"abc\n\"; }",
            ".a { background: url(a b); }",
        ] {
            assert!(parse(css).is_err(), "{css}");
        }
        assert_eq!(
            parse(".a { color: rgb(1, 2, 3) [x] {y}; }").unwrap().rules[0].declarations,
            vec![Property::Unsupported("color".to_string())]
        );
    }

    #[test]
    fn class_name_must_follow_the_dot() {
        assert!(parse(". btn { width: 10px; }").is_err());
        assert!(parse("./* x */btn { width: 10px; }").is_err());
        assert!(parse(".\nbtn { width: 10px; }").is_err());
        assert_eq!(
            parse(".btn { width: 10px; }").unwrap().rules[0].class,
            "btn"
        );
    }

    #[test]
    fn out_of_range_length_is_an_error() {
        for css in [
            ".a { width: 1e39px; }",
            ".a { width: 1e41%; }",
            ".a { margin: -1e39px; }",
            ".a { padding: 1e39px; }",
        ] {
            let err = parse(css).unwrap_err();
            assert!(
                err.message.contains("out of range"),
                "{css}: {}",
                err.message
            );
        }
        assert!(parse(".a { width: 3e38px; }").is_ok());
    }

    #[test]
    fn empty_value_is_an_error() {
        assert!(parse(".btn { width: 50px; height:").is_err());
        assert!(parse(".btn { width: 50px; height: }").is_err());
    }

    #[test]
    fn unclosed_block_at_eof_is_valid() {
        // css-syntax: an unclosed block is implicitly closed at end of input
        let sheet = parse(".a { width: 10px;").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![Property::Width(Length::Px(10.0))]
        );
    }

    #[test]
    fn empty_declarations_are_skipped() {
        for css in [".a { width: 10px;; }", ".a { ; width: 10px; }", ".a { ;; }"] {
            let sheet = parse(css).unwrap();
            assert_eq!(sheet.rules.len(), 1, "{css}");
        }
        assert_eq!(
            parse(".a { width: 10px;; }").unwrap().rules[0].declarations,
            vec![Property::Width(Length::Px(10.0))]
        );
    }

    #[test]
    fn property_names_are_case_insensitive() {
        let sheet = parse(".a { WIDTH: 10px; Max-Width: none; }").unwrap();
        assert_eq!(
            sheet.rules[0].declarations,
            vec![
                Property::Width(Length::Px(10.0)),
                Property::MaxWidth(Length::Auto)
            ]
        );
    }

    #[test]
    fn max_none_is_no_constraint_and_auto_is_invalid() {
        assert_eq!(
            parse(".a { max-height: none; }").unwrap().rules[0].declarations,
            vec![Property::MaxHeight(Length::Auto)]
        );
        assert!(parse(".a { max-width: auto; }").is_err());
    }

    #[test]
    fn unsupported_selector_is_an_error() {
        assert!(parse("button { width: 10px; }").is_err());
        assert!(parse(".a, .b { width: 10px; }").is_err());
    }

    #[test]
    fn comments_and_empty_sheets() {
        assert_eq!(parse("").unwrap().rules.len(), 0);
        assert_eq!(parse("/* nothing */").unwrap().rules.len(), 0);
        let sheet = parse(".a /* c */ { /* c */ width: 10px; /* c */ }").unwrap();
        assert_eq!(sheet.rules.len(), 1);
    }

    #[test]
    fn error_reports_location() {
        let err = parse(".a { width: banana; }").unwrap_err();
        assert!(err.message.contains("banana"), "{}", err.message);
        assert_eq!(err.line, 0);
        // column is 1-based, in UTF-16 code units: the error is reported
        // just past "banana", at 1-based column 19 of line 0 ...
        assert_eq!(err.column, 19);
        // ... and a non-BMP character before it counts as two units, so a
        // five-char comment holding one adds 6 columns, not 5.
        let err = parse(".a { /*\u{1F600}*/width: banana; }").unwrap_err();
        assert_eq!((err.line, err.column), (0, 25));
        assert!(err.to_string().contains("line 1, column 25"), "{err}");
    }
}
