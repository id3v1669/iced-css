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

/// Validate a complete CSS2.2 media-list prelude and match the screen canvas.
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
    let mut offset = 0;
    let mut need_name = true;
    let mut seen_name = false;
    let mut matches = false;
    loop {
        let rebase = |mut error: SyntaxError| {
            error.offset += offset;
            MediaError::Syntax(error)
        };
        let Some(token) = scan_token_budget(&input[offset..], work).map_err(rebase)? else {
            if !seen_name && context == MediaListContext::Import {
                return Ok(true);
            }
            return if need_name {
                Err(MediaError::Invalid { offset })
            } else {
                Ok(matches)
            };
        };
        if matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            offset += token.raw.len();
            continue;
        }
        if need_name && token.kind == TokenKind::Ident && !token.invalid_escape {
            let name =
                decode_identifier_budget(token.raw, limits.retained_bytes, work).map_err(rebase)?;
            matches |=
                name.text.eq_ignore_ascii_case("screen") || name.text.eq_ignore_ascii_case("all");
            seen_name = true;
            need_name = false;
        } else if !need_name && token.kind == TokenKind::Delim && token.raw == "," {
            need_name = true;
        } else {
            return Err(MediaError::Invalid { offset });
        }
        offset += token.raw.len();
    }
}
