use super::{DecodedString, MediaError, MediaListContext, StringLimits, SyntaxError, TokenKind};
use super::{
    decode_string_budget, decode_uri_budget, media::media_matches_screen_budget, scan_token_budget,
};
use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub struct ImportPrelude {
    /// Unresolved location, with repair offsets relative to location_span.start.
    pub location: DecodedString,
    pub location_span: Range<usize>,
    pub applies_to_screen: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    Invalid { offset: usize },
    Syntax(SyntaxError),
    Media(MediaError),
}
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { offset } => write!(f, "invalid CSS2.2 import prelude at {offset}"),
            Self::Syntax(error) => error.fmt(f),
            Self::Media(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ImportError {}

/// Parse the complete prelude after @import, excluding its terminating semicolon.
/// Ordering, statement recovery, base URI resolution and acquisition belong to
/// the stylesheet owner. No resource is acquired here, even for matching media.
pub fn parse_import_prelude(
    input: &str,
    limits: StringLimits,
) -> Result<ImportPrelude, ImportError> {
    let mut work = limits.work.get();
    let mut offset = 0;
    let location = loop {
        let token = scan_token_budget(&input[offset..], &mut work)
            .map_err(|mut error| {
                error.offset += offset;
                ImportError::Syntax(error)
            })?
            .ok_or(ImportError::Invalid { offset })?;
        if matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            offset += token.raw.len();
            continue;
        }
        if !matches!(token.kind, TokenKind::String | TokenKind::Uri) || token.invalid_escape {
            return Err(ImportError::Invalid { offset });
        }
        break token;
    };
    let end = offset + location.raw.len();
    // Evaluate media first so its temporary decoded identifiers are dropped before
    // the retained location is allocated. Both stages use one work allowance.
    let applies_to_screen =
        media_matches_screen_budget(&input[end..], MediaListContext::Import, limits, &mut work)
            .map_err(|error| {
                ImportError::Media(match error {
                    MediaError::Invalid { offset } => MediaError::Invalid {
                        offset: end + offset,
                    },
                    MediaError::Syntax(mut error) => {
                        error.offset += end;
                        MediaError::Syntax(error)
                    }
                })
            })?;
    let decoded = if location.kind == TokenKind::String {
        decode_string_budget(location.raw, limits.retained_bytes, &mut work)
    } else {
        decode_uri_budget(location.raw, limits.retained_bytes, &mut work)
    }
    .map_err(|mut error| {
        error.offset += offset;
        ImportError::Syntax(error)
    })?;
    Ok(ImportPrelude {
        location: decoded,
        location_span: offset..end,
        applies_to_screen,
    })
}
