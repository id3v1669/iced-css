#[path = "decoration.rs"]
mod decoration;
#[path = "keyword_data.rs"]
mod keyword_data;
#[path = "length_property.rs"]
mod length_property;
use super::{
    LengthTarget, NumericError,
    color::{Color, ColorError, ColorLimits},
    registry::PropertyId,
};
use crate::syntax::{
    Declaration, SyntaxError, SyntaxFault, Token, TokenKind, decode_identifier_budget,
    scan_token_budget,
};
pub use decoration::{TextDecoration, TextDecorationError};
#[path = "background_shorthand.rs"]
mod background_shorthand;
#[path = "border_shorthand.rs"]
mod border_shorthand;
#[path = "clip.rs"]
mod clip;
#[path = "content.rs"]
mod content;
#[path = "cursor.rs"]
mod cursor;
#[path = "font_family.rs"]
mod font_family;
#[path = "font_shorthand.rs"]
mod font_shorthand;
#[path = "list_shorthand.rs"]
mod list_shorthand;
#[path = "owned.rs"]
mod owned;
#[path = "shorthand.rs"]
mod shorthand;
pub use content::ContentItem;
pub use cursor::Cursor;

pub(super) fn cursor_keyword_allowed(keyword: Keyword) -> bool {
    cursor::valid_fallback(keyword)
}
pub use font_family::FontFamily;
#[path = "counters.rs"]
mod counters;
#[path = "position.rs"]
mod position;
#[path = "quotes.rs"]
mod quotes;
pub use counters::CounterDirective;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keyword(&'static str);
impl Keyword {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}
/// Materialize a static initial only when the finite property grammar admits it.
pub(super) fn initial_finite_keyword(property: PropertyId) -> Option<Keyword> {
    let keyword = Keyword(property.descriptor().initial);
    finite_keyword_allowed(property, keyword).then_some(keyword)
}

/// Validate a finite keyword against the retained, source-derived grammar.
pub(super) fn finite_keyword_allowed(property: PropertyId, keyword: Keyword) -> bool {
    keyword_data::KEYWORDS.iter().any(|(name, values)| {
        *name == property.descriptor().name && values.contains(&keyword.as_str())
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecifiedValue {
    Keyword(Keyword),
    Inherit,
    /// Internal request for an omitted shorthand member; not authored CSS syntax.
    Initial,
    /// Internal provider request produced only by a whole system-font shorthand.
    SystemFont(Keyword),
    /// Context-dependent initial recipe; never admitted as an authored keyword.
    InitialReference(super::initial::InitialReference),
    Color(Color),
    Length(LengthTarget),
    Number(super::Decimal),
    Integer(super::Integer),
    TextDecoration(TextDecoration),
    Uri(String),
    Quotes(Vec<[String; 2]>),
    Counters(Vec<CounterDirective>),
    ClipRect([LengthTarget; 4]),
    Cursor(Cursor),
    FontFamily(Vec<FontFamily>),
    Content(Vec<ContentItem>),
    Expanded(Vec<ParsedProperty>),
    LengthPair(LengthTarget, LengthTarget),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProperty {
    pub property: PropertyId,
    pub value: SpecifiedValue,
    pub important: bool,
}
// Keep the bounded four-edge clip value inline. Boxing this result would add an
// infallible allocation to every successful parse, outside explicit value budgets.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Eq)]
pub enum PropertyResult {
    Parsed(ParsedProperty),
    Unknown,
    Invalid,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PropertyError {
    Unimplemented(PropertyId),
    UnimplementedNumericSpelling(PropertyId),
    Syntax(SyntaxError),
    Numeric(NumericError),
}

impl std::fmt::Display for PropertyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unimplemented(property) => write!(
                f,
                "CSS2.2 grammar not implemented: {}",
                property.descriptor().name
            ),
            Self::UnimplementedNumericSpelling(property) => write!(
                f,
                "numeric spelling is not implemented pending specification evidence: {}",
                property.descriptor().name
            ),
            Self::Syntax(error) => error.fmt(f),
            Self::Numeric(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for PropertyError {}
fn significant<'a>(
    input: &'a str,
    offset: &mut usize,
    work: &mut usize,
) -> Result<Option<(usize, Token<'a>)>, PropertyError> {
    while *offset < input.len() {
        let start = *offset;
        let token = scan_token_budget(&input[start..], work)
            .map_err(|mut error| {
                error.offset += start;
                PropertyError::Syntax(error)
            })?
            .expect("nonempty input");
        *offset += token.raw.len();
        if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            return Ok(Some((start, token)));
        }
    }
    Ok(None)
}
fn numeric(
    value: Result<LengthTarget, NumericError>,
) -> Result<Option<LengthTarget>, PropertyError> {
    match value {
        Ok(value) => Ok(Some(value)),
        Err(NumericError::InvalidNumber | NumericError::UnsupportedUnit) => Ok(None),
        Err(error) => Err(PropertyError::Numeric(error)),
    }
}
fn border_styles(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            Some(&[
                "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge",
                "inset", "outset",
            ])
        }
        "outline-style" => Some(&[
            "none", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset",
        ]),
        _ => None,
    }
}
fn color_property(name: &str) -> bool {
    matches!(
        name,
        "color"
            | "background-color"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "outline-color"
    )
}
fn special_color(name: &str, keyword: &str) -> Option<Color> {
    if matches!(
        name,
        "background-color"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
    ) && keyword.eq_ignore_ascii_case("transparent")
    {
        Some(Color::Transparent)
    } else if name == "outline-color" && keyword.eq_ignore_ascii_case("invert") {
        Some(Color::Invert)
    } else {
        None
    }
}
pub(super) fn inner(
    property: PropertyId,
    input: &str,
    important: bool,
    recovery_only: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    let name = property.descriptor().name;
    let keywords = keyword_data::KEYWORDS
        .iter()
        .find(|(property, _)| *property == name)
        .map(|(_, values)| *values)
        .or_else(|| border_styles(name));
    let length_grammar = length_property::grammar(name);
    let counter_property = matches!(name, "counter-reset" | "counter-increment");
    let image_property = matches!(name, "background-image" | "list-style-image");
    let integer_property = matches!(name, "z-index" | "orphans" | "widows");
    if !color_property(name)
        && keywords.is_none()
        && length_grammar.is_none()
        && !integer_property
        && name != "font"
        && name != "font-weight"
        && name != "list-style"
        && name != "background"
        && border_shorthand::roles(name).is_none()
        && shorthand::targets(name).is_none()
        && !counter_property
        && !image_property
        && name != "content"
        && name != "font-family"
        && name != "cursor"
        && name != "clip"
        && name != "background-position"
        && name != "quotes"
        && name != "text-decoration"
    {
        return Err(PropertyError::Unimplemented(property));
    }
    if recovery_only {
        return Ok(PropertyResult::Invalid);
    }
    if name == "font" {
        return font_shorthand::parse(input, important, limits, work).map(|value| {
            value.map_or(PropertyResult::Invalid, |values| {
                PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Expanded(values),
                    important,
                })
            })
        });
    }
    if name == "list-style" {
        return list_shorthand::parse(input, important, limits, work).map(|value| {
            value.map_or(PropertyResult::Invalid, |values| {
                PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Expanded(values),
                    important,
                })
            })
        });
    }
    if name == "background" {
        return background_shorthand::parse(input, important, limits, work).map(|value| {
            value.map_or(PropertyResult::Invalid, |values| {
                PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Expanded(values),
                    important,
                })
            })
        });
    }
    if let Some(roles) = border_shorthand::roles(name) {
        return border_shorthand::parse(property, input, roles, important, limits, work).map(
            |value| {
                value.map_or(PropertyResult::Invalid, |values| {
                    PropertyResult::Parsed(ParsedProperty {
                        property,
                        value: SpecifiedValue::Expanded(values),
                        important,
                    })
                })
            },
        );
    }
    if let Some(targets) = shorthand::targets(name) {
        return shorthand::parse(input, targets, important, limits, work).map(|value| {
            value.map_or(PropertyResult::Invalid, |values| {
                PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Expanded(values),
                    important,
                })
            })
        });
    }
    let mut offset = 0;
    let Some((start, first)) = significant(input, &mut offset, work)? else {
        return Ok(PropertyResult::Invalid);
    };
    if first.invalid_escape {
        return Ok(PropertyResult::Invalid);
    }
    let identifier = if first.kind == TokenKind::Ident {
        match decode_identifier_budget(first.raw, limits.syntax.retained_bytes, work) {
            Ok(value) => Some(value.text),
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(PropertyResult::Invalid),
        }
    } else {
        None
    };
    let value = if identifier
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("inherit"))
    {
        if significant(input, &mut offset, work)?.is_none() {
            SpecifiedValue::Inherit
        } else if name == "font-family" {
            // Only a whole unquoted family name equal to inherit is reserved;
            // an identifier sequence such as "inherit Times" is a named family.
            drop(identifier);
            let Some(value) = font_family::parse(input, limits, work)? else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::FontFamily(value)
        } else {
            return Ok(PropertyResult::Invalid);
        }
    } else if name == "font-weight" {
        if significant(input, &mut offset, work)?.is_some() {
            return Ok(PropertyResult::Invalid);
        }
        if first.kind == TokenKind::Number {
            if !matches!(
                first.raw,
                "100" | "200" | "300" | "400" | "500" | "600" | "700" | "800" | "900"
            ) {
                // Only another spelling of one of the nine weights (+100, 0100,
                // 100.0) is the unresolved form. A number naming no weight at
                // all (150, 1000, 0, 1.5, -100) is invalid under every reading
                // and drops just this declaration.
                let names_weight = first.raw.parse::<f64>().map_or(true, |number| {
                    (1..=9).any(|step| number == f64::from(step * 100))
                });
                if !names_weight {
                    return Ok(PropertyResult::Invalid);
                }
                return Err(PropertyError::UnimplementedNumericSpelling(property));
            }
            SpecifiedValue::Integer(
                super::Integer::parse(first.raw, limits.numeric).map_err(PropertyError::Numeric)?,
            )
        } else {
            let Some(keyword) = identifier.as_deref().and_then(|value| {
                ["normal", "bold", "bolder", "lighter"]
                    .iter()
                    .find(|key| key.eq_ignore_ascii_case(value))
            }) else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::Keyword(Keyword(keyword))
        }
    } else if name == "content" {
        if let Some(keyword) = identifier.as_deref().and_then(|name| {
            ["normal", "none"]
                .iter()
                .find(|key| key.eq_ignore_ascii_case(name))
        }) {
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Keyword(Keyword(keyword))
        } else {
            drop(identifier);
            let Some(value) = content::parse(input, limits, work)? else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::Content(value)
        }
    } else if name == "font-family" {
        drop(identifier);
        let Some(value) = font_family::parse(input, limits, work)? else {
            return Ok(PropertyResult::Invalid);
        };
        SpecifiedValue::FontFamily(value)
    } else if name == "cursor" {
        drop(identifier);
        let Some(value) = cursor::parse(input, limits, work)? else {
            return Ok(PropertyResult::Invalid);
        };
        SpecifiedValue::Cursor(value)
    } else if name == "clip" {
        if identifier
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("auto"))
        {
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Keyword(Keyword("auto"))
        } else {
            drop(identifier);
            let Some(value) = clip::parse(input, limits, work)? else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::ClipRect(value)
        }
    } else if name == "background-position" {
        drop(identifier);
        let Some(value) = position::parse(input, limits, work)? else {
            return Ok(PropertyResult::Invalid);
        };
        value
    } else if counter_property {
        if identifier
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("none"))
        {
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Keyword(Keyword("none"))
        } else {
            drop(identifier);
            let Some(value) = counters::parse(input, name == "counter-increment", limits, work)?
            else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::Counters(value)
        }
    } else if name == "quotes" {
        if identifier
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("none"))
        {
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Keyword(Keyword("none"))
        } else {
            drop(identifier);
            let Some(value) = quotes::parse(input, limits, work)? else {
                return Ok(PropertyResult::Invalid);
            };
            SpecifiedValue::Quotes(value)
        }
    } else if image_property {
        if significant(input, &mut offset, work)?.is_some() {
            return Ok(PropertyResult::Invalid);
        }
        if identifier
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("none"))
        {
            SpecifiedValue::Keyword(Keyword("none"))
        } else if first.kind == TokenKind::Uri {
            match crate::syntax::decode_uri_budget(first.raw, limits.syntax.retained_bytes, work) {
                Ok(value) => SpecifiedValue::Uri(value.text),
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(PropertyResult::Invalid),
            }
        } else {
            return Ok(PropertyResult::Invalid);
        }
    } else if name == "text-decoration" {
        drop(identifier);
        let Some(value) = decoration::parse(input, limits, work)? else {
            return Ok(PropertyResult::Invalid);
        };
        SpecifiedValue::TextDecoration(value)
    } else if integer_property {
        if significant(input, &mut offset, work)?.is_some() {
            return Ok(PropertyResult::Invalid);
        }
        if name == "z-index"
            && identifier
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case("auto"))
        {
            SpecifiedValue::Keyword(Keyword("auto"))
        } else {
            if first.kind != TokenKind::Number {
                return Ok(PropertyResult::Invalid);
            }
            let integer = match super::Integer::parse(first.raw, limits.numeric) {
                Ok(value) => value,
                Err(NumericError::InvalidNumber) => return Ok(PropertyResult::Invalid),
                Err(error) => return Err(PropertyError::Numeric(error)),
            };
            if name != "z-index" && (integer.is_negative() || integer.is_zero()) {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Integer(integer)
        }
    } else if let Some(keywords) = keywords {
        if significant(input, &mut offset, work)?.is_some() {
            return Ok(PropertyResult::Invalid);
        }
        let Some(value) = identifier.as_deref().and_then(|value| {
            keywords
                .iter()
                .find(|keyword| keyword.eq_ignore_ascii_case(value))
        }) else {
            return Ok(PropertyResult::Invalid);
        };
        SpecifiedValue::Keyword(Keyword(value))
    } else if color_property(name) {
        if let Some(color) = identifier
            .as_deref()
            .and_then(|keyword| special_color(name, keyword))
        {
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(PropertyResult::Invalid);
            }
            SpecifiedValue::Color(color)
        } else {
            match super::color::parse_color_budget(input, limits, work) {
                Ok(value) => SpecifiedValue::Color(value),
                Err(ColorError::Invalid) => return Ok(PropertyResult::Invalid),
                Err(ColorError::Syntax(error)) => return Err(PropertyError::Syntax(error)),
                Err(ColorError::Numeric(error)) => return Err(PropertyError::Numeric(error)),
            }
        }
    } else {
        drop(identifier);
        let Some(value) = length_property::parse(
            input,
            length_grammar.expect("admitted length grammar"),
            limits,
            work,
        )?
        else {
            return Ok(PropertyResult::Invalid);
        };
        value
    };
    Ok(PropertyResult::Parsed(ParsedProperty {
        property,
        value,
        important,
    }))
}
/// Execute currently implemented property grammars. Known required properties
/// without an implementation are explicit errors, never silently ignored CSS.
/// The declaration syntax layer and this stage have separately supplied budgets.
pub fn parse_property(
    declaration: &Declaration<'_>,
    limits: ColorLimits,
) -> Result<PropertyResult, PropertyError> {
    let Some(property) = PropertyId::lookup(&declaration.name.text) else {
        return Ok(PropertyResult::Unknown);
    };
    let mut work = limits.syntax.work.get();
    inner(
        property,
        declaration.value,
        declaration.important,
        declaration.recovery_only,
        limits,
        &mut work,
    )
    .map_err(|mut error| {
        if let PropertyError::Syntax(error) = &mut error {
            error.offset += declaration.value_span.start;
        }
        error
    })
}
