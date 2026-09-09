//! Fresh GET representation and explicit stylesheet media admission; no cache reuse.
use super::http_metadata::HttpContentType;

/// RFC9110 success codes are understood here; other 2xx use its §15 x00 fallback.
/// This classification does not implement extension-specific response semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetResponseKind {
    Complete,
    Transformed,
    UnrecognizedSuccess,
    Created,
    Accepted,
    NoContent,
    ResetContent,
    PartialContent,
    NotModified,
    Informational,
    Redirection,
    ClientError,
    ServerError,
    Invalid,
}
impl GetResponseKind {
    /// Eligibility of content, not MIME, framing, decoding or cache admission.
    pub fn has_complete_representation(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Transformed | Self::UnrecognizedSuccess
        )
    }
}
/// Only for responses to GET, never HEAD, CONNECT or a state-changing method.
/// Bodyless/partial/validation responses cannot become an empty fresh stylesheet.
pub fn classify_get_response(status: u16) -> GetResponseKind {
    use GetResponseKind::*;
    match status {
        100..=199 => Informational,
        200 => Complete,
        201 => Created,
        202 => Accepted,
        203 => Transformed,
        204 => NoContent,
        205 => ResetContent,
        206 => PartialContent,
        207..=299 => UnrecognizedSuccess,
        304 => NotModified,
        300..=399 => Redirection,
        400..=499 => ClientError,
        500..=599 => ServerError,
        _ => Invalid,
    }
}
/// Project policy for absent HTTP Content-Type. Neither option sniffs content,
/// synthesizes a charset or accepts an explicitly different media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylesheetMediaPolicy {
    RequireCss,
    AllowAbsent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylesheetMediaError {
    Missing,
    Other,
}
impl std::fmt::Display for StylesheetMediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP stylesheet media admission: {self:?}")
    }
}
impl std::error::Error for StylesheetMediaError {}
/// Malformed/ambiguous metadata must first fail Content-Type parsing.
pub fn admit_stylesheet_media(
    content_type: Option<&HttpContentType<'_>>,
    policy: StylesheetMediaPolicy,
) -> Result<(), StylesheetMediaError> {
    match content_type {
        Some(value) if value.is_css() => Ok(()),
        Some(_) => Err(StylesheetMediaError::Other),
        None if policy == StylesheetMediaPolicy::AllowAbsent => Ok(()),
        None => Err(StylesheetMediaError::Missing),
    }
}
