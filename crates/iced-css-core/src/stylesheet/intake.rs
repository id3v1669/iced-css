use super::{ParsedStylesheet, RuleAssemblyError, RuleContext, StylesheetLimits, parse_stylesheet};
use crate::syntax::{
    RuleError,
    decode::SourcePosition,
    encoding_selection::EncodingMetadata,
    unicode_intake::{IntakeError, IntakeLimits, UnicodeStylesheet, decode_separate_unicode},
};
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct StylesheetIntakeLimits {
    pub intake: IntakeLimits,
    pub sheet: StylesheetLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginalSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}
/// Owns diagnostic source storage and the typed sheet without self references.
/// This is a runtime candidate, not a source-free Compile artifact.
pub struct UnicodeStylesheetCandidate {
    source: UnicodeStylesheet,
    sheet: ParsedStylesheet,
}
impl UnicodeStylesheetCandidate {
    pub fn source(&self) -> &UnicodeStylesheet {
        &self.source
    }
    pub fn sheet(&self) -> &ParsedStylesheet {
        &self.sheet
    }
    /// Both ends must be decoded scalar boundaries (or EOF), in ascending order.
    pub fn original_span(&self, decoded: Range<usize>) -> Option<OriginalSpan> {
        original_span(&self.source, decoded)
    }
}
#[derive(Debug)]
pub enum StylesheetIntakeError {
    Intake(IntakeError),
    Assembly {
        error: RuleAssemblyError,
        /// Present for located errors whose endpoints are decoded boundaries.
        original: Option<OriginalSpan>,
    },
}
impl std::fmt::Display for StylesheetIntakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stylesheet intake: {self:?}")
    }
}
impl std::error::Error for StylesheetIntakeError {}

/// Decode one separate-file Unicode stylesheet using explicit metadata, then
/// assemble a complete typed candidate. No I/O, base-URI inference or publication
/// occurs. Registered unsupported charsets remain explicit intake errors.
pub fn parse_separate_unicode_stylesheet(
    input: &[u8],
    metadata: EncodingMetadata<'_>,
    context: RuleContext,
    limits: StylesheetIntakeLimits,
) -> Result<UnicodeStylesheetCandidate, StylesheetIntakeError> {
    let source = decode_separate_unicode(input, metadata, limits.intake)
        .map_err(StylesheetIntakeError::Intake)?;
    let sheet = parse_stylesheet(source.text(), context, limits.sheet).map_err(|error| {
        let decoded = match &error {
            RuleAssemblyError::Property { span, .. } => Some(span.clone()),
            RuleAssemblyError::Syntax(error)
            | RuleAssemblyError::Boundary(RuleError::Syntax(error)) => {
                Some(error.offset..error.offset)
            }
            _ => None,
        };
        let original = decoded.and_then(|range| original_span(&source, range));
        StylesheetIntakeError::Assembly { error, original }
    })?;
    Ok(UnicodeStylesheetCandidate { source, sheet })
}
fn original_span(source: &UnicodeStylesheet, decoded: Range<usize>) -> Option<OriginalSpan> {
    if decoded.start > decoded.end {
        return None;
    }
    Some(OriginalSpan {
        start: source.position(decoded.start)?,
        end: source.position(decoded.end)?,
    })
}
