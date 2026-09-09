use super::{
    Boundary, DecodedIdentifier, RecoveryLimits, RecoveryMode, SyntaxError, SyntaxFault, Token,
    TokenKind,
};
use super::{
    recover::recover_prefix_budget, strings::decode_identifier_budget, token::scan_token_budget,
};
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct DeclarationLimits {
    pub recovery: RecoveryLimits,
    pub name_bytes: NonZeroUsize,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Declaration<'a> {
    pub name: DecodedIdentifier,
    pub value: &'a str,
    pub important: bool,
    pub span: Range<usize>,
    pub value_span: Range<usize>,
    pub implicit_close: bool,
    pub recovery_only: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum DeclarationItem<'a> {
    Declaration(Declaration<'a>),
    Malformed { raw: &'a str, span: Range<usize> },
    AtRule { raw: &'a str, span: Range<usize> },
}
pub struct Declarations<'a> {
    input: &'a str,
    limits: DeclarationLimits,
    offset: usize,
    remaining: usize,
    done: bool,
}
impl<'a> Declarations<'a> {
    pub fn new(input: &'a str, limits: DeclarationLimits) -> Self {
        Self {
            input,
            limits,
            offset: 0,
            remaining: limits.recovery.work.get(),
            done: false,
        }
    }
    pub fn remainder(&self) -> &'a str {
        &self.input[self.offset..]
    }
}

impl<'a> Declarations<'a> {
    fn token(&mut self, offset: usize, end: usize) -> Result<Option<Token<'a>>, SyntaxError> {
        scan_token_budget(&self.input[offset..end], &mut self.remaining).map_err(|mut error| {
            error.offset += offset;
            error
        })
    }
    fn significant(
        &mut self,
        cursor: &mut usize,
        end: usize,
    ) -> Result<Option<(usize, Token<'a>)>, SyntaxError> {
        while *cursor < end {
            let start = *cursor;
            let token = self.token(start, end)?.expect("nonempty input");
            *cursor += token.raw.len();
            if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
                return Ok(Some((start, token)));
            }
        }
        Ok(None)
    }
    fn advance(&mut self) -> Result<Option<DeclarationItem<'a>>, SyntaxError> {
        loop {
            if self.offset == self.input.len() {
                self.done = true;
                return Ok(None);
            }
            let token = self
                .token(self.offset, self.input.len())?
                .expect("nonempty input");
            if matches!(
                token.kind,
                TokenKind::Space | TokenKind::Comment | TokenKind::Semicolon
            ) {
                self.offset += token.raw.len();
                continue;
            }
            if token.kind == TokenKind::CloseBrace {
                self.done = true;
                return Ok(None);
            }
            break;
        }
        let start = self.offset;
        let first = self
            .token(start, self.input.len())?
            .expect("nonempty input");
        let mode = if first.kind == TokenKind::AtKeyword {
            RecoveryMode::AtRule
        } else {
            RecoveryMode::Declaration
        };
        let recovered = recover_prefix_budget(
            &self.input[start..],
            mode,
            self.limits.recovery,
            &mut self.remaining,
        )
        .map_err(|mut error| {
            error.offset += start;
            error
        })?;
        self.offset = start + recovered.consumed;
        let span = start..self.offset;
        let malformed = || DeclarationItem::Malformed {
            raw: recovered.raw,
            span: span.clone(),
        };
        if mode == RecoveryMode::AtRule {
            return Ok(Some(DeclarationItem::AtRule {
                raw: recovered.raw,
                span,
            }));
        }
        if first.kind != TokenKind::Ident || recovered.invalid_tokens {
            return Ok(Some(malformed()));
        }
        let end = self.offset - usize::from(recovered.boundary == Boundary::Semicolon);
        let mut cursor = start + first.raw.len();
        if !self
            .significant(&mut cursor, end)?
            .is_some_and(|(_, token)| token.kind == TokenKind::Colon)
        {
            return Ok(Some(malformed()));
        }
        let Some((value_start, initial)) = self.significant(&mut cursor, end)? else {
            return Ok(Some(malformed()));
        };
        let mut previous = None;
        let mut last = (value_start, initial);
        // End of the preceding significant token lets priority stripping also
        // omit whitespace/comments before '!'. Interior source remains intact.
        let mut before_previous_end = value_start;
        let mut before_last_end = value_start;
        while let Some(current) = self.significant(&mut cursor, end)? {
            before_previous_end = before_last_end;
            before_last_end = last.0 + last.1.raw.len();
            previous = Some(last);
            last = current;
        }
        let mut important = false;
        let mut value_end = last.0 + last.1.raw.len();
        if recovered.unclosed_delimiters == 0
            && last.1.kind == TokenKind::Ident
            && previous.is_some_and(|(_, token)| token.kind == TokenKind::Delim && token.raw == "!")
        {
            let decoded =
                decode_identifier_budget(last.1.raw, self.limits.name_bytes, &mut self.remaining)
                    .map_err(|mut error| {
                    error.offset += last.0;
                    error
                })?;
            if decoded.text.eq_ignore_ascii_case("important") {
                important = true;
                value_end = before_previous_end;
            }
        }
        if value_end <= value_start {
            return Ok(Some(malformed()));
        }
        let name = match decode_identifier_budget(
            first.raw,
            self.limits.name_bytes,
            &mut self.remaining,
        ) {
            Ok(name) => name,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(error);
            }
            Err(_) => return Ok(Some(malformed())),
        };
        Ok(Some(DeclarationItem::Declaration(Declaration {
            name,
            value: &self.input[value_start..value_end],
            important,
            span,
            value_span: value_start..value_end,
            implicit_close: recovered.implicit_close,
            recovery_only: recovered.recovery_only,
        })))
    }
}
impl<'a> Iterator for Declarations<'a> {
    type Item = Result<DeclarationItem<'a>, SyntaxError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.advance() {
            Ok(Some(item)) => Some(Ok(item)),
            Ok(None) => None,
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}
impl std::iter::FusedIterator for Declarations<'_> {}
