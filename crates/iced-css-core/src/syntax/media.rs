use super::{StringLimits, SyntaxError, TokenKind, decode_identifier_budget, scan_token_budget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaListContext {
    /// CSS2.2 allows an omitted import media list, equivalent to all.
    Import,
    /// An @media rule requires at least one identifier.
    MediaRule,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MediaError {
    Invalid { offset: usize },
    Syntax(SyntaxError),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { offset } => write!(f, "invalid CSS2.2 media list at {offset}"),
            Self::Syntax(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for MediaError {}

/// What media queries are answered against (Media Queries Level 3): the size of the area
/// the document is shown in, in CSS pixels, and whether the user prefers a dark scheme.
/// Without a viewport no size feature matches.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MediaEnvironment {
    pub viewport: Option<(f32, f32)>,
    pub dark: Option<bool>,
}
thread_local! {
    static ENVIRONMENT: std::cell::Cell<MediaEnvironment> =
        const { std::cell::Cell::new(MediaEnvironment { viewport: None, dark: None }) };
}
/// Run `parse` with `environment` answering the media queries of every sheet it parses on
/// this thread, `@media` and `@import` alike. A sheet parsed outside any such call sees
/// no viewport. The parsers take their limits by value everywhere; the environment is
/// scoped this way instead of being one more field in each of them.
pub fn with_media_environment<T>(environment: MediaEnvironment, parse: impl FnOnce() -> T) -> T {
    struct Restore(MediaEnvironment);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENVIRONMENT.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ENVIRONMENT.with(|cell| cell.replace(environment)));
    parse()
}

/// Validate a complete media-list prelude and match the screen canvas: CSS2.2 media types,
/// and Media Queries Level 3 queries over the environment of [`with_media_environment`].
/// Consumes the entire list before accepting a match. Unknown identifiers do not
/// match; a malformed member invalidates the list under the selected Q267 rule.
/// Retained bytes bound one decoded identifier and its repair records at a time.
pub fn media_matches_screen(
    input: &str,
    context: MediaListContext,
    limits: StringLimits,
) -> Result<bool, MediaError> {
    let mut work = limits.work.get();
    media_matches_screen_budget(input, context, limits, &mut work)
}

pub(crate) fn media_matches_screen_budget(
    input: &str,
    context: MediaListContext,
    limits: StringLimits,
    work: &mut usize,
) -> Result<bool, MediaError> {
    let environment = ENVIRONMENT.with(std::cell::Cell::get);
    // The significant tokens, each with where it starts.
    let mut tokens: Vec<(usize, TokenKind, &str)> = Vec::new();
    let mut offset = 0;
    while let Some(token) = scan_token_budget(&input[offset..], work).map_err(|mut error| {
        error.offset += offset;
        MediaError::Syntax(error)
    })? {
        if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            if token.invalid_escape {
                return Err(MediaError::Invalid { offset });
            }
            tokens.push((offset, token.kind, token.raw));
        }
        offset += token.raw.len();
    }
    if tokens.is_empty() {
        return if context == MediaListContext::Import {
            Ok(true)
        } else {
            Err(MediaError::Invalid { offset })
        };
    }
    let comma = |token: &(usize, TokenKind, &str)| token.1 == TokenKind::Delim && token.2 == ",";
    let mut matches = false;
    let mut at = 0;
    loop {
        // A member starts with a media type or a parenthesized feature: anything else
        // makes the whole list invalid, as CSS2.2 has it (Q267).
        let Some(first) = tokens.get(at) else {
            return Err(MediaError::Invalid { offset });
        };
        if !matches!(first.1, TokenKind::Ident | TokenKind::OpenParen) {
            return Err(MediaError::Invalid { offset: first.0 });
        }
        let end = tokens[at..]
            .iter()
            .position(comma)
            .map_or(tokens.len(), |found| at + found);
        // A query malformed past its start matches nothing (Media Queries 3, section 3.1).
        matches |= query(&tokens[at..end], environment, limits, work)
            .map_err(|mut error| {
                error.offset += first.0;
                MediaError::Syntax(error)
            })?
            .unwrap_or(false);
        if end == tokens.len() {
            return Ok(matches);
        }
        at = end + 1;
    }
}

/// One media query. `None` is malformed: it matches nothing.
fn query(
    tokens: &[(usize, TokenKind, &str)],
    environment: MediaEnvironment,
    limits: StringLimits,
    work: &mut usize,
) -> Result<Option<bool>, SyntaxError> {
    let mut name = |raw: &str| {
        decode_identifier_budget(raw, limits.retained_bytes, work)
            .map(|name| name.text.to_ascii_lowercase())
    };
    let mut at = 0;
    let mut negated = false;
    let mut result = true;
    if tokens[0].1 == TokenKind::Ident {
        let mut word = name(tokens[0].2)?;
        at = 1;
        // `not` and `only` come before a media type.
        if matches!(word.as_str(), "not" | "only")
            && tokens
                .get(1)
                .is_some_and(|token| token.1 == TokenKind::Ident)
        {
            negated = word == "not";
            word = name(tokens[1].2)?;
            at = 2;
        }
        result = matches!(word.as_str(), "screen" | "all");
        if at == tokens.len() {
            return Ok(Some(result != negated));
        }
        if tokens[at].1 != TokenKind::Ident || name(tokens[at].2)? != "and" {
            return Ok(None);
        }
        at += 1;
    }
    loop {
        // `(` feature [`:` value] `)`
        let Some(close) = tokens[at..]
            .iter()
            .position(|token| token.1 == TokenKind::CloseParen)
            .map(|found| at + found)
        else {
            return Ok(None);
        };
        let inside = &tokens[at..close];
        if inside.len() < 2
            || inside[0].1 != TokenKind::OpenParen
            || inside[1].1 != TokenKind::Ident
        {
            return Ok(None);
        }
        let feature_name = name(inside[1].2)?;
        let value = match &inside[2..] {
            [] => None,
            [colon, value] if colon.1 == TokenKind::Colon => Some(*value),
            _ => return Ok(None),
        };
        match feature(&feature_name, value, environment) {
            Some(matched) => result &= matched,
            None => return Ok(None),
        }
        at = close + 1;
        if at == tokens.len() {
            return Ok(Some(result != negated));
        }
        if tokens[at].1 != TokenKind::Ident || name(tokens[at].2)? != "and" {
            return Ok(None);
        }
        at += 1;
        if at == tokens.len() {
            return Ok(None);
        }
    }
}

/// A length in CSS pixels: `em` and `rem` are of the initial font size (Media Queries 3,
/// section 6). A unitless zero is a length.
fn length(token: (usize, TokenKind, &str)) -> Option<f32> {
    let raw = token.2;
    match token.1 {
        TokenKind::Number => raw.parse::<f32>().ok().filter(|value| *value == 0.0),
        TokenKind::Dimension => {
            let split = raw
                .bytes()
                .position(|b| !(b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')))?;
            let number: f32 = raw[..split].parse().ok()?;
            let scale = match raw[split..].to_ascii_lowercase().as_str() {
                "px" => 1.0,
                "em" | "rem" => 16.0,
                "in" => 96.0,
                "cm" => 4800.0 / 127.0,
                "mm" => 480.0 / 127.0,
                "pt" => 4.0 / 3.0,
                "pc" => 16.0,
                _ => return None,
            };
            Some(number * scale)
        }
        _ => None,
    }
}

/// Whether a feature holds. `None` is a feature or value this does not know: the query
/// matches nothing.
fn feature(
    name: &str,
    value: Option<(usize, TokenKind, &str)>,
    environment: MediaEnvironment,
) -> Option<bool> {
    let (range, base) = match name.strip_prefix("min-") {
        Some(base) => (std::cmp::Ordering::Greater, base),
        None => match name.strip_prefix("max-") {
            Some(base) => (std::cmp::Ordering::Less, base),
            None => (std::cmp::Ordering::Equal, name),
        },
    };
    match base {
        "width" | "height" => {
            let size = environment
                .viewport
                .map(|(width, height)| if base == "width" { width } else { height });
            let Some(value) = value else {
                // In a boolean context: any size but none. `min-` and `max-` need a value.
                return (range == std::cmp::Ordering::Equal)
                    .then(|| size.is_some_and(|size| size != 0.0));
            };
            let wanted = length(value).filter(|wanted| *wanted >= 0.0)?;
            Some(size.is_some_and(|size| match range {
                std::cmp::Ordering::Greater => size >= wanted,
                std::cmp::Ordering::Less => size <= wanted,
                std::cmp::Ordering::Equal => size == wanted,
            }))
        }
        "orientation" if range == std::cmp::Ordering::Equal => {
            let portrait = environment.viewport.map(|(width, height)| height >= width);
            match value {
                None => Some(portrait.is_some()),
                Some(value) if value.1 == TokenKind::Ident => {
                    match value.2.to_ascii_lowercase().as_str() {
                        "portrait" => Some(portrait == Some(true)),
                        "landscape" => Some(portrait == Some(false)),
                        _ => None,
                    }
                }
                Some(_) => None,
            }
        }
        // Media Queries 5: light unless the environment says dark.
        "prefers-color-scheme" if range == std::cmp::Ordering::Equal => {
            let dark = environment.dark == Some(true);
            match value {
                None => Some(true),
                Some(value) if value.1 == TokenKind::Ident => {
                    match value.2.to_ascii_lowercase().as_str() {
                        "dark" => Some(dark),
                        "light" => Some(!dark),
                        _ => None,
                    }
                }
                Some(_) => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn matches(input: &str, environment: MediaEnvironment) -> Result<bool, MediaError> {
        let limits = StringLimits {
            work: NonZeroUsize::new(100_000).unwrap(),
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
        };
        with_media_environment(environment, || {
            media_matches_screen(input, MediaListContext::MediaRule, limits)
        })
    }
    fn at(width: f32, height: f32) -> MediaEnvironment {
        MediaEnvironment {
            viewport: Some((width, height)),
            dark: None,
        }
    }

    #[test]
    fn size_features_compare_with_the_viewport() {
        let wide = at(1000.0, 600.0);
        for (query, expected) in [
            ("screen", true),
            ("print", false),
            ("(min-width: 800px)", true),
            ("(min-width: 1000px)", true),
            ("(min-width: 1001px)", false),
            ("(max-width: 999px)", false),
            (
                "screen and (min-width: 800px) and (max-width: 1200px)",
                true,
            ),
            (
                "screen and (min-width: 800px) and (max-height: 500px)",
                false,
            ),
            ("(width: 1000px)", true),
            ("(height: 600px) and (orientation: landscape)", true),
            ("(orientation: portrait)", false),
            ("(min-width: 50em)", true),
            ("(min-width: 70em)", false),
            ("(max-width: 0)", false),
            ("(width)", true),
            ("ALL AND (MIN-WIDTH: 10PX)", true),
            ("only screen and (min-width: 10px)", true),
            ("not screen and (min-width: 10px)", false),
            ("not screen and (min-width: 2000px)", true),
            ("not print", true),
            ("print, (min-width: 10px)", true),
            ("print, (min-width: 2000px)", false),
        ] {
            assert_eq!(matches(query, wide), Ok(expected), "{query}");
        }
        // Without a viewport no size feature matches.
        let unknown = MediaEnvironment::default();
        assert_eq!(matches("(min-width: 1px)", unknown), Ok(false));
        assert_eq!(matches("(max-width: 9999px)", unknown), Ok(false));
        assert_eq!(matches("screen", unknown), Ok(true));
        assert_eq!(
            matches("(orientation: portrait)", at(300.0, 500.0)),
            Ok(true)
        );
    }

    #[test]
    fn color_scheme_is_light_unless_told_dark() {
        let dark = MediaEnvironment {
            viewport: None,
            dark: Some(true),
        };
        assert_eq!(matches("(prefers-color-scheme: dark)", dark), Ok(true));
        assert_eq!(matches("(prefers-color-scheme: light)", dark), Ok(false));
        let unknown = MediaEnvironment::default();
        assert_eq!(matches("(prefers-color-scheme: dark)", unknown), Ok(false));
        assert_eq!(matches("(prefers-color-scheme: light)", unknown), Ok(true));
    }

    #[test]
    fn a_malformed_query_matches_nothing_and_a_bad_start_spoils_the_list() {
        let wide = at(1000.0, 600.0);
        for query in [
            "(min-width)",
            "(min-width: 10)",
            "(min-width: -1px)",
            "(min-width: 10px",
            "(color-gamut: p3)",
            "(orientation: sideways)",
            "screen and",
            "screen (min-width: 10px)",
            "screen and min-width",
            "(min-width: 10px) or (max-width: 5px)",
            "(min-width: 10px 20px)",
        ] {
            assert_eq!(matches(query, wide), Ok(false), "{query}");
        }
        // The other members of the list still count.
        assert_eq!(matches("(nope), screen", wide), Ok(true));
        for query in ["", "3d", "screen,", ", screen", "screen, , print"] {
            assert!(
                matches!(matches(query, wide), Err(MediaError::Invalid { .. })),
                "{query}"
            );
        }
        // The environment is put back when the call ends.
        assert_eq!(
            matches("(min-width: 1px)", MediaEnvironment::default()),
            Ok(false)
        );
    }
}
