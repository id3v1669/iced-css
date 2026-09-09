use super::{Boundary, RecoveryLimits, RecoveryMode, SyntaxError, TokenKind};
use super::{recover::recover_prefix_budget, scan_token_budget};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementContext {
    Stylesheet,
    /// A previously delimited body slice, excluding its enclosing braces.
    /// Unlike stylesheet mode, CDO/CDC are not skipped between statements.
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind<'a> {
    AtRule { keyword: &'a str },
    QualifiedRule,
}

/// Borrowed syntax boundaries, not an accepted CSS rule. The owner must validate
/// the keyword/prelude, rule placement and body grammar before applying content.
#[derive(Debug, PartialEq, Eq)]
pub struct Statement<'a> {
    pub kind: StatementKind<'a>,
    pub raw: &'a str,
    pub span: Range<usize>,
    pub prelude: &'a str,
    pub prelude_span: Range<usize>,
    pub body: Option<&'a str>,
    pub body_span: Option<Range<usize>>,
    pub implicit_close: bool,
    /// Includes body token faults; these do not necessarily invalidate the whole
    /// statement (for example, one malformed declaration in an otherwise valid rule).
    pub invalid_tokens: bool,
}

/// Iterates one syntax level with a cumulative recovery work allowance. Nested
/// bodies are borrowed, not recursively parsed. Collecting results and processing
/// nested bodies require the owner's separate aggregate storage/work accounting.
pub struct Statements<'a> {
    input: &'a str,
    context: StatementContext,
    limits: RecoveryLimits,
    offset: usize,
    work: usize,
    done: bool,
}

impl<'a> Statements<'a> {
    pub fn new(input: &'a str, context: StatementContext, limits: RecoveryLimits) -> Self {
        Self {
            input,
            context,
            limits,
            offset: 0,
            work: limits.work.get(),
            done: false,
        }
    }
    pub fn remainder(&self) -> &'a str {
        &self.input[self.offset..]
    }
    fn advance(&mut self) -> Result<Option<Statement<'a>>, SyntaxError> {
        let first = loop {
            let token = scan_token_budget(&self.input[self.offset..], &mut self.work).map_err(
                |mut error| {
                    error.offset += self.offset;
                    error
                },
            )?;
            let Some(token) = token else { return Ok(None) };
            if matches!(token.kind, TokenKind::Space | TokenKind::Comment)
                || (self.context == StatementContext::Stylesheet
                    && matches!(token.kind, TokenKind::Cdo | TokenKind::Cdc))
            {
                self.offset += token.raw.len();
                continue;
            }
            if self.context == StatementContext::Block && token.kind == TokenKind::CloseBrace {
                return Ok(None);
            }
            break token;
        };
        let start = self.offset;
        let (kind, mode, prelude_start) = if first.kind == TokenKind::AtKeyword {
            (
                StatementKind::AtRule { keyword: first.raw },
                RecoveryMode::AtRule,
                start + first.raw.len(),
            )
        } else {
            (
                StatementKind::QualifiedRule,
                RecoveryMode::QualifiedRule,
                start,
            )
        };
        let recovered =
            recover_prefix_budget(&self.input[start..], mode, self.limits, &mut self.work)
                .map_err(|mut error| {
                    error.offset += start;
                    error
                })?;
        let end = start + recovered.consumed;
        let terminal = usize::from(matches!(
            recovered.boundary,
            Boundary::Semicolon | Boundary::AtRuleBlock
        ));
        let (prelude_end, body_span) = match recovered.block_start {
            Some(block) => (start + block, Some(start + block + 1..end - terminal)),
            None => (end - terminal, None),
        };
        self.offset = end;
        Ok(Some(Statement {
            kind,
            raw: &self.input[start..end],
            span: start..end,
            prelude: &self.input[prelude_start..prelude_end],
            prelude_span: prelude_start..prelude_end,
            body: body_span.as_ref().map(|span| &self.input[span.clone()]),
            body_span,
            implicit_close: recovered.implicit_close,
            invalid_tokens: recovered.invalid_tokens,
        }))
    }
}

impl<'a> Iterator for Statements<'a> {
    type Item = Result<Statement<'a>, SyntaxError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.advance() {
            Ok(Some(value)) => Some(Ok(value)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}
impl std::iter::FusedIterator for Statements<'_> {}
