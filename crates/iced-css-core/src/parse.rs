use cssparser::{Delimiter, Parser, ParserInput, Token};

use crate::resolve::{Length, MarginValue, Margins};

#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    Width(Length),
    Height(Length),
    MinWidth(Length),
    MaxWidth(Length),
    Margin(Margins),
    MarginTop(MarginValue),
    MarginRight(MarginValue),
    MarginBottom(MarginValue),
    MarginLeft(MarginValue),
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

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: u32,
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
        let class = parser.expect_ident_cloned().map_err(Error::from)?;
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
        let name = parser.expect_ident_cloned().map_err(Error::from)?;
        parser.expect_colon().map_err(Error::from)?;

        let declaration = parser
            .parse_until_after(Delimiter::Semicolon, |value| parse_value(&name, value))?;

        declarations.push(declaration);
    }

    Ok(declarations)
}

fn parse_value<'i>(name: &str, parser: &mut Parser<'i, '_>) -> Result<Property, Error<'i>> {
    let property = match name {
        "width" => Property::Width(parse_length(parser)?),
        "height" => Property::Height(parse_length(parser)?),
        "min-width" => Property::MinWidth(parse_length(parser)?),
        "max-width" => Property::MaxWidth(parse_length(parser)?),
        "margin" => Property::Margin(parse_margin_shorthand(parser)?),
        "margin-top" => Property::MarginTop(parse_margin_value(parser)?),
        "margin-right" => Property::MarginRight(parse_margin_value(parser)?),
        "margin-bottom" => Property::MarginBottom(parse_margin_value(parser)?),
        "margin-left" => Property::MarginLeft(parse_margin_value(parser)?),
        _ => {
            // Consume (and thereby syntax-check) the rest of the value.
            while !parser.is_exhausted() {
                let _ = parser.next()?;
            }
            Property::Unsupported(name.to_string())
        }
    };

    if !matches!(property, Property::Unsupported(_)) {
        parser.expect_exhausted().map_err(Error::from)?;
    }

    Ok(property)
}

fn parse_length<'i>(parser: &mut Parser<'i, '_>) -> Result<Length, Error<'i>> {
    match parser.next()?.clone() {
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Ok(Length::Px(value))
        }
        Token::Dimension { unit, .. } => {
            error(parser, format!("unsupported length unit: {unit}"))
        }
        Token::Percentage { unit_value, .. } => Ok(Length::Percent(unit_value)),
        Token::Number { value, .. } if value == 0.0 => Ok(Length::Px(0.0)),
        Token::Number { .. } => error(parser, "non-zero lengths require a unit"),
        Token::Ident(ident) if ident.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
        other => error(parser, format!("expected a length, got {other:?}")),
    }
}

fn parse_margin_value<'i>(parser: &mut Parser<'i, '_>) -> Result<MarginValue, Error<'i>> {
    match parse_length(parser)? {
        Length::Px(value) => Ok(MarginValue::Px(value)),
        Length::Auto => Ok(MarginValue::Auto),
        Length::Percent(_) => error(parser, "percentage margins are not supported yet"),
    }
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
        assert_eq!(margins[0], Margins { top: px(10.0), right: px(10.0), bottom: px(10.0), left: px(10.0) });
        assert_eq!(margins[1], Margins { top: px(10.0), right: px(20.0), bottom: px(10.0), left: px(20.0) });
        assert_eq!(margins[2], Margins { top: px(10.0), right: px(20.0), bottom: px(30.0), left: px(20.0) });
        assert_eq!(margins[3], Margins { top: px(10.0), right: px(20.0), bottom: px(30.0), left: px(40.0) });
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
    fn unterminated_block_is_an_error() {
        assert!(parse(".btn { width: 50px; height:").is_err());
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
    }
}
