#[path = "decoration.rs"]
mod decoration;
#[path = "keyword_css3.rs"]
mod keyword_css3;
#[path = "keyword_data.rs"]
mod keyword_data;
/// The finite keyword grammars: CSS 2.2's, then those of later modules.
fn keyword_grammars() -> impl Iterator<Item = &'static (&'static str, &'static [&'static str])> {
    keyword_data::KEYWORDS.iter().chain(keyword_css3::KEYWORDS)
}
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
#[path = "gradient.rs"]
mod gradient;
#[path = "grid.rs"]
mod grid;
#[path = "list_shorthand.rs"]
mod list_shorthand;
#[path = "owned.rs"]
mod owned;
#[path = "shadow.rs"]
mod shadow;
#[path = "shorthand.rs"]
mod shorthand;
#[path = "transform.rs"]
mod transform;
#[path = "transition.rs"]
mod transition;
pub use content::ContentItem;
pub use cursor::Cursor;
pub use gradient::{AngleUnit, GradientDirection, GradientStop, LinearGradient};
pub use grid::{
    GRID_LIMIT, GridBreadth, GridLineValue, GridRepeat, GridTrackEntry, GridTrackSize, GridValue,
};
pub use shadow::ShadowValue;
pub use transform::TransformFunction;
pub use transition::{EasingValue, StepPosition, TimeValue, TransitionProperty, TransitionValue};

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
    keyword_grammars().any(|(name, values)| {
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
    /// `box-shadow`: its shadows, first on top; empty is `none`.
    Shadows(Vec<ShadowValue>),
    /// A grid property (CSS Grid Layout 1).
    Grid(GridValue),
    /// A `background-image` gradient (CSS Images 3).
    Gradient(LinearGradient),
    /// `transform`: its functions in order; empty is `none` (CSS Transforms 1).
    Transform(Vec<TransformFunction>),
    /// `aspect-ratio`: width to height, both positive (CSS Box Sizing 4).
    Ratio(super::Decimal, super::Decimal),
    /// CSS Cascade 4 `revert`: as if no author declaration gave the property.
    Revert,
    /// A transition property's list (CSS Transitions 1).
    Transition(TransitionValue),
    /// A custom property declaration (`--name: value`): its name, with its dashes, and
    /// its value's text. Filed under [`PropertyId::custom`].
    Custom {
        name: String,
        raw: String,
    },
    /// A value holding `var()`: its text, parsed only once an element's custom properties
    /// are substituted into it. `shorthand` is the shorthand it was written for, when
    /// this is one of that shorthand's longhands.
    Pending {
        raw: String,
        shorthand: Option<PropertyId>,
    },
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
        if token.kind == TokenKind::Function && super::calc::is_math_function(token.raw) {
            return calc_token(input, start, token, offset, work).map(Some);
        }
        if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            return Ok(Some((start, token)));
        }
    }
    Ok(None)
}
/// A whole `calc(...)` as one function token whose text runs through its closing
/// parenthesis (or to the end of the value when it has none): the component parsers take
/// one token for one length.
fn calc_token<'a>(
    input: &'a str,
    start: usize,
    mut token: Token<'a>,
    offset: &mut usize,
    work: &mut usize,
) -> Result<(usize, Token<'a>), PropertyError> {
    let mut depth = 1usize;
    while depth > 0 && *offset < input.len() {
        let at = *offset;
        let inner = scan_token_budget(&input[at..], work)
            .map_err(|mut error| {
                error.offset += at;
                PropertyError::Syntax(error)
            })?
            .expect("nonempty input");
        *offset += inner.raw.len();
        token.invalid_escape |= inner.invalid_escape;
        match inner.kind {
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth -= 1,
            _ => {}
        }
    }
    token.raw = &input[start..*offset];
    Ok((start, token))
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
/// The transition properties, whose grammars live in `transition.rs`.
fn transition_property(name: &str) -> bool {
    matches!(
        name,
        "transition-property"
            | "transition-duration"
            | "transition-timing-function"
            | "transition-delay"
            | "transition"
            | "animation-name"
            | "animation-duration"
            | "animation-timing-function"
            | "animation-delay"
            | "animation-iteration-count"
            | "animation-direction"
            | "animation-fill-mode"
            | "animation-play-state"
            | "animation"
    )
}
/// The grid properties, whose grammars live in `grid.rs`.
fn grid_property(name: &str) -> bool {
    matches!(
        name,
        "grid-template-columns"
            | "grid-template-rows"
            | "grid-template-areas"
            | "grid-auto-columns"
            | "grid-auto-rows"
            | "grid-auto-flow"
            | "grid-row-start"
            | "grid-row-end"
            | "grid-column-start"
            | "grid-column-end"
            | "grid-row"
            | "grid-column"
            | "grid-area"
            | "grid-template"
            | "grid"
    )
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
    let keywords = keyword_grammars()
        .find(|(property, _)| *property == name)
        .map(|(_, values)| *values)
        .or_else(|| border_styles(name));
    let length_grammar = length_property::grammar(name);
    let counter_property = matches!(name, "counter-reset" | "counter-increment");
    let image_property = matches!(name, "background-image" | "list-style-image");
    let integer_property = matches!(name, "z-index" | "orphans" | "widows" | "order");
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
        && name != "transform-origin"
        && name != "transform"
        && name != "aspect-ratio"
        && name != "quotes"
        && name != "text-decoration"
        && name != "border-radius"
        && name != "box-shadow"
        && !matches!(
            name,
            "opacity" | "flex-grow" | "flex-shrink" | "flex-flow" | "flex" | "gap"
        )
        && !grid_property(name)
        && name != "background-size"
        && !transition_property(name)
    {
        return Err(PropertyError::Unimplemented(property));
    }
    if recovery_only {
        return Ok(PropertyResult::Invalid);
    }
    if matches!(name, "flex-flow" | "flex" | "gap") {
        return flex_shorthand(property, input, important, limits, work);
    }
    if name == "aspect-ratio" {
        // CSS Box Sizing 4: `auto`, or a ratio (a number, or two with a `/` between);
        // `auto` beside a ratio is that ratio here, where nothing is replaced.
        let mut offset = 0;
        let mut parts = Vec::new();
        while let Some((_, token)) = significant(input, &mut offset, work)? {
            if parts.len() == 4 {
                return Ok(PropertyResult::Invalid);
            }
            parts.push(token);
        }
        let is_auto = |token: &Token<'_>| {
            token.kind == TokenKind::Ident && token.raw.eq_ignore_ascii_case("auto")
        };
        if let [only] = parts.as_slice()
            && only.kind == TokenKind::Ident
        {
            return Ok(PropertyResult::Parsed(ParsedProperty {
                property,
                value: if is_auto(only) {
                    SpecifiedValue::Keyword(Keyword("auto"))
                } else if only.raw.eq_ignore_ascii_case("inherit") {
                    SpecifiedValue::Inherit
                } else {
                    return Ok(PropertyResult::Invalid);
                },
                important,
            }));
        }
        let autos = parts.iter().filter(|token| is_auto(token)).count();
        if autos > 1 || (autos == 1 && !(is_auto(&parts[0]) || is_auto(&parts[parts.len() - 1]))) {
            return Ok(PropertyResult::Invalid);
        }
        let ratio: Vec<&Token<'_>> = parts.iter().filter(|token| !is_auto(token)).collect();
        let number = |token: &Token<'_>| {
            (token.kind == TokenKind::Number)
                .then(|| super::Decimal::parse(token.raw, limits.numeric).ok())
                .flatten()
                .filter(|number| !number.is_negative())
        };
        let one = super::Decimal::parse("1", limits.numeric).expect("one");
        let (Some(width), Some(height)) = (match ratio.as_slice() {
            [width] => (number(width), Some(one)),
            [width, slash, height] if slash.kind == TokenKind::Delim && slash.raw == "/" => {
                (number(width), number(height))
            }
            _ => (None, None),
        }) else {
            return Ok(PropertyResult::Invalid);
        };
        // A ratio with a zero in it is degenerate: it does nothing, as `auto`.
        return Ok(PropertyResult::Parsed(ParsedProperty {
            property,
            value: SpecifiedValue::Ratio(width, height),
            important,
        }));
    }
    if name == "transform" {
        let value = if input.trim().eq_ignore_ascii_case("inherit") {
            SpecifiedValue::Inherit
        } else {
            match transform::parse(input, limits, work)? {
                Some(functions) => SpecifiedValue::Transform(functions),
                None => return Ok(PropertyResult::Invalid),
            }
        };
        return Ok(PropertyResult::Parsed(ParsedProperty {
            property,
            value,
            important,
        }));
    }
    if name == "background-size" {
        // CSS Backgrounds 3: `cover`, `contain`, or one or two sizes, the second `auto`
        // when left out.
        let mut offset = 0;
        let mut parts = Vec::new();
        while let Some(token) = significant(input, &mut offset, work)? {
            if parts.len() == 2 {
                return Ok(PropertyResult::Invalid);
            }
            parts.push(token);
        }
        let keyword = |word: &str| {
            parts.len() == 1
                && parts[0].1.kind == TokenKind::Ident
                && parts[0].1.raw.eq_ignore_ascii_case(word)
        };
        let value = if keyword("inherit") {
            SpecifiedValue::Inherit
        } else if keyword("cover") {
            SpecifiedValue::Keyword(Keyword("cover"))
        } else if keyword("contain") {
            SpecifiedValue::Keyword(Keyword("contain"))
        } else {
            let mut sizes = Vec::new();
            for part in &parts {
                match length_property::size_component(*part, limits, work)? {
                    Some(SpecifiedValue::Length(size)) => sizes.push(size),
                    _ => return Ok(PropertyResult::Invalid),
                }
            }
            match sizes.as_slice() {
                [across] => SpecifiedValue::LengthPair(across.clone(), LengthTarget::Auto),
                [across, down] => SpecifiedValue::LengthPair(across.clone(), down.clone()),
                _ => return Ok(PropertyResult::Invalid),
            }
        };
        return Ok(PropertyResult::Parsed(ParsedProperty {
            property,
            value,
            important,
        }));
    }
    if transition_property(name) {
        if name == "transition" {
            return transition::shorthand(property, input, important, limits, work);
        }
        if name == "animation" {
            return transition::animation_shorthand(property, input, important, limits, work);
        }
        return Ok(match transition::parse(property, input, limits, work)? {
            Some(value) => PropertyResult::Parsed(ParsedProperty {
                property,
                value,
                important,
            }),
            None => PropertyResult::Invalid,
        });
    }
    if grid_property(name) {
        if property.descriptor().shorthand {
            return grid::shorthand(property, input, important, limits, work);
        }
        return Ok(match grid::parse(property, input, limits, work)? {
            Some(value) => PropertyResult::Parsed(ParsedProperty {
                property,
                value,
                important,
            }),
            None => PropertyResult::Invalid,
        });
    }
    if matches!(name, "opacity" | "flex-grow" | "flex-shrink") {
        // One number, or `inherit`. CSS Color 3: an opacity of any size (it is clipped when
        // computed). CSS Flexbox 1: a factor that is not negative.
        let mut offset = 0;
        let Some((_, token)) = significant(input, &mut offset, work)? else {
            return Ok(PropertyResult::Invalid);
        };
        if significant(input, &mut offset, work)?.is_some() {
            return Ok(PropertyResult::Invalid);
        }
        let value = match token.kind {
            TokenKind::Ident if token.raw.eq_ignore_ascii_case("inherit") => {
                SpecifiedValue::Inherit
            }
            TokenKind::Number => match super::Decimal::parse(token.raw, limits.numeric) {
                Ok(number) if name != "opacity" && number.is_negative() => {
                    return Ok(PropertyResult::Invalid);
                }
                Ok(number) => SpecifiedValue::Number(number),
                Err(NumericError::InvalidNumber | NumericError::UnsupportedUnit) => {
                    return Ok(PropertyResult::Invalid);
                }
                Err(error) => return Err(PropertyError::Numeric(error)),
            },
            _ => return Ok(PropertyResult::Invalid),
        };
        return Ok(PropertyResult::Parsed(ParsedProperty {
            property,
            value,
            important,
        }));
    }
    if name == "box-shadow" {
        if input.trim().eq_ignore_ascii_case("inherit") {
            return Ok(PropertyResult::Parsed(ParsedProperty {
                property,
                value: SpecifiedValue::Inherit,
                important,
            }));
        }
        return Ok(match shadow::parse(input, limits, work)? {
            Some(shadows) => PropertyResult::Parsed(ParsedProperty {
                property,
                value: SpecifiedValue::Shadows(shadows),
                important,
            }),
            None => PropertyResult::Invalid,
        });
    }
    if name == "border-radius" {
        // `inherit` alone inherits every corner.
        if input.trim().eq_ignore_ascii_case("inherit") {
            return Ok(PropertyResult::Parsed(ParsedProperty {
                property,
                value: SpecifiedValue::Expanded(
                    property
                        .longhands()
                        .iter()
                        .map(|longhand| ParsedProperty {
                            property: *longhand,
                            value: SpecifiedValue::Inherit,
                            important,
                        })
                        .collect(),
                ),
                important,
            }));
        }
        return Ok(
            match length_property::radius_shorthand(input, limits, work)? {
                Some(corners) => PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Expanded(
                        property
                            .longhands()
                            .iter()
                            .zip(corners)
                            .map(|(longhand, (across, down))| ParsedProperty {
                                property: *longhand,
                                value: SpecifiedValue::LengthPair(across, down),
                                important,
                            })
                            .collect(),
                    ),
                    important,
                }),
                None => PropertyResult::Invalid,
            },
        );
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
    } else if name == "background-position" || name == "transform-origin" {
        // CSS Transforms 1: a transform's origin is a position, as a background's is.
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
        // CSS Images 3: a gradient draws as a background image.
        if name == "background-image" && first.kind == TokenKind::Function {
            return Ok(match gradient::parse(input, limits, work)? {
                Some(gradient) => PropertyResult::Parsed(ParsedProperty {
                    property,
                    value: SpecifiedValue::Gradient(gradient),
                    important,
                }),
                None => PropertyResult::Invalid,
            });
        }
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
            if !matches!(name, "z-index" | "order") && (integer.is_negative() || integer.is_zero())
            {
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
    // CSS Custom Properties: any name of two dashes and more declares one. Its value is
    // kept as written; nothing in it is invalid until it is used.
    if declaration.name.text.len() > 2 && declaration.name.text.starts_with("--") {
        return Ok(if declaration.recovery_only {
            PropertyResult::Invalid
        } else {
            PropertyResult::Parsed(ParsedProperty {
                property: PropertyId::custom(),
                value: SpecifiedValue::Custom {
                    name: declaration.name.text.clone(),
                    raw: declaration.value.to_owned(),
                },
                important: declaration.important,
            })
        });
    }
    let Some(property) = PropertyId::lookup(&declaration.name.text) else {
        return Ok(PropertyResult::Unknown);
    };
    // A value with `var()` in it is valid at parse time, whatever else it holds: it is
    // parsed for each element once its custom properties are substituted.
    if super::custom::has_reference(declaration.value) && !declaration.recovery_only {
        let pending = |property: PropertyId, shorthand| ParsedProperty {
            property,
            value: SpecifiedValue::Pending {
                raw: declaration.value.to_owned(),
                shorthand,
            },
            important: declaration.important,
        };
        return Ok(PropertyResult::Parsed(if property.descriptor().shorthand {
            ParsedProperty {
                property,
                value: SpecifiedValue::Expanded(
                    property
                        .longhands()
                        .iter()
                        .map(|longhand| pending(*longhand, Some(property)))
                        .collect(),
                ),
                important: declaration.important,
            }
        } else {
            pending(property, None)
        }));
    }
    if !declaration.recovery_only
        && let Some(parsed) = wide_keyword(property, declaration.value, declaration.important)
    {
        return Ok(PropertyResult::Parsed(parsed));
    }
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

/// Parse a value for a known property from text that is not a declaration in a sheet: a
/// value with its `var()` references substituted. A value that still holds `var()` is
/// invalid here.
pub fn parse_value(
    property: PropertyId,
    value: &str,
    important: bool,
    limits: ColorLimits,
) -> Result<PropertyResult, PropertyError> {
    if super::custom::has_reference(value) || property == PropertyId::custom() {
        return Ok(PropertyResult::Invalid);
    }
    if let Some(parsed) = wide_keyword(property, value, important) {
        return Ok(PropertyResult::Parsed(parsed));
    }
    let mut work = limits.syntax.work.get();
    inner(property, value, important, false, limits, &mut work)
}

/// CSS Cascade 4: `initial`, the property's initial value, and `unset`, `inherit` for an
/// inherited property and `initial` otherwise, as a whole value; on a shorthand, each
/// longhand's. `inherit` stays with each grammar, as CSS 2.2 has it.
fn wide_keyword(property: PropertyId, value: &str, important: bool) -> Option<ParsedProperty> {
    let word = value.trim();
    let unset = word.eq_ignore_ascii_case("unset");
    let revert = word.eq_ignore_ascii_case("revert");
    if !unset && !revert && !word.eq_ignore_ascii_case("initial") {
        return None;
    }
    let leaf = |property: PropertyId| ParsedProperty {
        property,
        value: if revert {
            SpecifiedValue::Revert
        } else if unset && property.descriptor().inheritance == super::registry::Inheritance::Yes {
            SpecifiedValue::Inherit
        } else {
            SpecifiedValue::Initial
        },
        important,
    };
    Some(if property.descriptor().shorthand {
        ParsedProperty {
            property,
            value: SpecifiedValue::Expanded(
                property.longhands().iter().copied().map(leaf).collect(),
            ),
            important,
        }
    } else {
        leaf(property)
    })
}

/// The `flex-flow`, `flex` and `gap` shorthands (CSS Flexbox 1, CSS Box Alignment 3): each
/// part is parsed as its longhand, and a part left out is that longhand's initial value,
/// or what the shorthand gives it.
fn flex_shorthand(
    property: PropertyId,
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    let name = property.descriptor().name;
    let targets = property.longhands();
    let whole = |values: Vec<SpecifiedValue>| {
        PropertyResult::Parsed(ParsedProperty {
            property,
            value: SpecifiedValue::Expanded(
                targets
                    .iter()
                    .zip(values)
                    .map(|(longhand, value)| ParsedProperty {
                        property: *longhand,
                        value,
                        important,
                    })
                    .collect(),
            ),
            important,
        })
    };
    let mut tokens: Vec<&str> = Vec::new();
    let mut offset = 0;
    while let Some((_, token)) = significant(input, &mut offset, work)? {
        if token.invalid_escape || tokens.len() == 3 {
            return Ok(PropertyResult::Invalid);
        }
        tokens.push(token.raw);
    }
    let keyword = |text: &str| tokens.len() == 1 && tokens[0].eq_ignore_ascii_case(text);
    if keyword("inherit") {
        return Ok(whole(vec![SpecifiedValue::Inherit; targets.len()]));
    }
    // One part as one of the longhands, by its own grammar.
    let part = |longhand: PropertyId, text: &str, work: &mut usize| {
        Ok::<_, PropertyError>(
            match inner(longhand, text, important, false, limits, work)? {
                PropertyResult::Parsed(parsed) if parsed.value != SpecifiedValue::Inherit => {
                    Some(parsed.value)
                }
                _ => None,
            },
        )
    };
    let number = |text: &str| {
        super::Decimal::parse(text, limits.numeric)
            .map(SpecifiedValue::Number)
            .expect("a small literal")
    };
    match name {
        "flex-flow" => {
            let mut values = [None, None];
            for token in &tokens {
                let slot = if let Some(value) = part(targets[0], token, work)? {
                    (0, value)
                } else if let Some(value) = part(targets[1], token, work)? {
                    (1, value)
                } else {
                    return Ok(PropertyResult::Invalid);
                };
                if values[slot.0].replace(slot.1).is_some() {
                    return Ok(PropertyResult::Invalid);
                }
            }
            if tokens.is_empty() {
                return Ok(PropertyResult::Invalid);
            }
            Ok(whole(
                values
                    .into_iter()
                    .map(|value| value.unwrap_or(SpecifiedValue::Initial))
                    .collect(),
            ))
        }
        "gap" => {
            let (Some(first), true) = (tokens.first(), tokens.len() <= 2) else {
                return Ok(PropertyResult::Invalid);
            };
            let Some(row) = part(targets[0], first, work)? else {
                return Ok(PropertyResult::Invalid);
            };
            let column = match tokens.get(1) {
                Some(second) => part(targets[1], second, work)?,
                None => Some(row.clone()),
            };
            Ok(column.map_or(PropertyResult::Invalid, |column| whole(vec![row, column])))
        }
        _ => {
            // `flex`: `none` is rigid, `auto` flexes from its size; a number alone grows
            // from nothing.
            if keyword("none") {
                let basis = part(targets[2], "auto", work)?.expect("auto");
                return Ok(whole(vec![number("0"), number("0"), basis]));
            }
            if keyword("auto") {
                let basis = part(targets[2], "auto", work)?.expect("auto");
                return Ok(whole(vec![number("1"), number("1"), basis]));
            }
            let (mut grow, mut shrink, mut basis) = (None, None, None);
            for (at, token) in tokens.iter().enumerate() {
                let numeric = token
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-'));
                // A unitless zero after both factors is the basis.
                let both = grow.is_some() && shrink.is_some();
                if numeric && !both {
                    let Some(value) = part(targets[0], token, work)? else {
                        return Ok(PropertyResult::Invalid);
                    };
                    // The factors come together, the growth first.
                    if grow.is_none() {
                        grow = Some(value);
                    } else if basis.is_none() || at == 2 {
                        shrink = Some(value);
                    } else {
                        return Ok(PropertyResult::Invalid);
                    }
                } else if basis.is_none() {
                    match part(targets[2], token, work)? {
                        Some(value) => basis = Some(value),
                        None => return Ok(PropertyResult::Invalid),
                    }
                } else {
                    return Ok(PropertyResult::Invalid);
                }
            }
            if tokens.is_empty() {
                return Ok(PropertyResult::Invalid);
            }
            let zero = part(targets[2], "0", work)?.expect("zero");
            Ok(whole(vec![
                grow.unwrap_or_else(|| number("1")),
                shrink.unwrap_or_else(|| number("1")),
                basis.unwrap_or(zero),
            ]))
        }
    }
}
