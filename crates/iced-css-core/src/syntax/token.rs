use super::{NumericTokenKind, SyntaxError};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    AtKeyword,
    String,
    BadString,
    BadUri,
    Hash,
    Number,
    Percentage,
    Dimension,
    Uri,
    UnicodeRange,
    Cdo,
    Cdc,
    Colon,
    Semicolon,
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Space,
    Comment,
    Function,
    Includes,
    DashMatch,
    Delim,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub raw: &'a str,
    pub implicit_close: bool,
    pub invalid_escape: bool,
}

use super::numeric::{Scan, scan_numeric};
fn space(c: char) -> bool {
    matches!(c, ' ' | '\r' | '\n' | '\t' | '\u{c}')
}
impl Scan<'_> {
    fn whitespace(&mut self, mut offset: usize) -> Result<usize, SyntaxError> {
        while self.at(offset)?.is_some_and(space) {
            offset += 1;
        }
        Ok(offset)
    }
    fn string(&mut self, start: usize) -> Result<(usize, bool, bool), SyntaxError> {
        let quote = self.at(start)?.expect("string starts with a quote");
        let mut end = start + 1;
        loop {
            match self.at(end)? {
                None => return Ok((end, true, false)),
                Some(c) if c == quote => return Ok((end + 1, false, false)),
                Some('\r' | '\n' | '\u{c}') => return Ok((end, false, true)),
                Some('\\') => {
                    if let Some(next) = self.escape(end)? {
                        end = next;
                    } else {
                        end += 1;
                        if let Some(c) = self.at(end)? {
                            end += c.len_utf8();
                            if c == '\r' && self.at(end)? == Some('\n') {
                                end += 1;
                            }
                        }
                    }
                }
                Some(c) => end += c.len_utf8(),
            }
        }
    }
    fn escape_value(&mut self, start: usize, end: usize) -> Result<Option<char>, SyntaxError> {
        let Some(first) = self.at(start + 1)? else {
            return Ok(None);
        };
        if !first.is_ascii_hexdigit() {
            return Ok(Some(first));
        }
        let mut value = 0;
        for offset in (start + 1..end).take(6) {
            if offset >= end {
                break;
            }
            let Some(c) = self.at(offset)?.filter(char::is_ascii_hexdigit) else {
                break;
            };
            value = value * 16 + c.to_digit(16).expect("ASCII hex digit");
        }
        Ok(match value {
            0 => None,
            0x110000.. => Some('\u{fffd}'),
            _ => char::from_u32(value),
        })
    }
    fn is_url(&mut self, end: usize) -> Result<bool, SyntaxError> {
        let mut offset = 0;
        for expected in ['u', 'r', 'l'] {
            if offset >= end {
                return Ok(false);
            }
            let c = self.at(offset)?.expect("within identifier");
            let value = if c == '\\' {
                let next = self.escape(offset)?.expect("identifier escape");
                let value = self.escape_value(offset, next)?;
                offset = next;
                value
            } else {
                offset += c.len_utf8();
                Some(c)
            };
            if !value.is_some_and(|c| c.eq_ignore_ascii_case(&expected)) {
                return Ok(false);
            }
        }
        Ok(offset == end)
    }
    fn uri(&mut self, start: usize) -> Result<(TokenKind, usize, bool), SyntaxError> {
        let mut end = self.whitespace(start)?;
        if matches!(self.at(end)?, Some('\'' | '"')) {
            let (next, implicit, bad) = self.string(end)?;
            end = next;
            if bad {
                return Ok((TokenKind::BadUri, end, false));
            }
            if implicit {
                return Ok((TokenKind::Uri, end, true));
            }
        } else {
            loop {
                match self.at(end)? {
                    Some('\\') => match self.escape(end)? {
                        Some(next) => end = next,
                        None => break,
                    },
                    Some(c)
                        if c == '\0'
                            || c >= '\u{80}'
                            || matches!(c,'!'|'#'|'$'|'%'|'&'|'*'..='['|']'..='~') =>
                    {
                        end += c.len_utf8()
                    }
                    _ => break,
                }
            }
        }
        end = self.whitespace(end)?;
        Ok(match self.at(end)? {
            Some(')') => (TokenKind::Uri, end + 1, false),
            None => (TokenKind::Uri, end, true),
            _ => (TokenKind::BadUri, end, false),
        })
    }
    fn unicode_range(&mut self) -> Result<Option<usize>, SyntaxError> {
        if !matches!(self.at(0)?, Some('u' | 'U')) || self.at(1)? != Some('+') {
            return Ok(None);
        }
        let mut end = 2;
        while end < 8 && self.at(end)?.is_some_and(|c| c.is_ascii_hexdigit()) {
            end += 1;
        }
        let hex_end = end;
        while end < 8 && self.at(end)? == Some('?') {
            end += 1;
        }
        if end == 2 {
            return Ok(None);
        }
        if hex_end == end && self.at(end)? == Some('-') {
            let mut tail = end + 1;
            while tail < end + 7 && self.at(tail)?.is_some_and(|c| c.is_ascii_hexdigit()) {
                tail += 1;
            }
            if tail > end + 1 {
                end = tail;
            }
        }
        Ok(Some(end))
    }
    fn invalid_escapes(&mut self, end: usize) -> Result<bool, SyntaxError> {
        let mut offset = 0;
        while offset < end {
            let c = self.at(offset)?.expect("within token");
            if c == '\\'
                && let Some(next) = self.escape(offset)?
            {
                if self.escape_value(offset, next)?.is_none() {
                    return Ok(true);
                }
                offset = next;
                continue;
            }
            offset += c.len_utf8();
        }
        Ok(false)
    }
}
/// Scan one chapter-4 token with the selected Q279/Q280/Q285 reconciliations.
/// Input and output spans are decoded UTF-8, not transport bytes. Semantic value
/// decoding and declaration recovery remain separate. No allocation is performed.
/// `invalid_escape` preserves numeric zero/surrogate invalidity for the parser.
pub fn scan_token(input: &str, work: NonZeroUsize) -> Result<Option<Token<'_>>, SyntaxError> {
    let mut remaining = work.get();
    scan_token_budget(input, &mut remaining)
}

pub(crate) fn scan_token_budget<'a>(
    input: &'a str,
    remaining: &mut usize,
) -> Result<Option<Token<'a>>, SyntaxError> {
    let mut scan = Scan {
        input,
        remaining: *remaining,
    };
    let result = scan_one(&mut scan);
    *remaining = scan.remaining;
    result
}

fn scan_one<'a>(scan: &mut Scan<'a>) -> Result<Option<Token<'a>>, SyntaxError> {
    let input = scan.input;
    let Some(first) = scan.at(0)? else {
        return Ok(None);
    };
    let (mut kind, end, mut implicit_close) = if matches!(first, '\'' | '"') {
        let (end, implicit, bad) = scan.string(0)?;
        (
            if bad {
                TokenKind::BadString
            } else {
                TokenKind::String
            },
            end,
            implicit,
        )
    } else if input.starts_with("/*") {
        let mut end = 2;
        loop {
            match scan.at(end)? {
                None => break (TokenKind::Comment, end, true),
                Some('*') if scan.at(end + 1)? == Some('/') => {
                    break (TokenKind::Comment, end + 2, false);
                }
                Some(c) => end += c.len_utf8(),
            }
        }
    } else if space(first) {
        (TokenKind::Space, scan.whitespace(0)?, false)
    } else if let Some(value) = scan_numeric(scan)? {
        (
            match value.kind {
                NumericTokenKind::Number => TokenKind::Number,
                NumericTokenKind::Percentage => TokenKind::Percentage,
                NumericTokenKind::Dimension => TokenKind::Dimension,
            },
            value.consumed,
            false,
        )
    } else if let Some(end) = scan.unicode_range()? {
        (TokenKind::UnicodeRange, end, false)
    } else if let Some(end) = scan.ident(0)? {
        if scan.at(end)? == Some('(') {
            if scan.is_url(end)? {
                scan.uri(end + 1)?
            } else {
                (TokenKind::Function, end + 1, false)
            }
        } else {
            (TokenKind::Ident, end, false)
        }
    } else if first == '@' && scan.ident(1)?.is_some() {
        (
            TokenKind::AtKeyword,
            scan.ident(1)?.expect("checked identifier"),
            false,
        )
    } else if first == '#' && scan.name_char(1, false)?.is_some() {
        let mut end = 1;
        while let Some(next) = scan.name_char(end, false)? {
            end = next;
        }
        (TokenKind::Hash, end, false)
    } else {
        match first {
            '<' if input.starts_with("<!--") => (TokenKind::Cdo, 4, false),
            '-' if input.starts_with("-->") => (TokenKind::Cdc, 3, false),
            '~' if input.starts_with("~=") => (TokenKind::Includes, 2, false),
            '|' if input.starts_with("|=") => (TokenKind::DashMatch, 2, false),
            c => (
                match c {
                    ':' => TokenKind::Colon,
                    ';' => TokenKind::Semicolon,
                    '{' => TokenKind::OpenBrace,
                    '}' => TokenKind::CloseBrace,
                    '(' => TokenKind::OpenParen,
                    ')' => TokenKind::CloseParen,
                    '[' => TokenKind::OpenBracket,
                    ']' => TokenKind::CloseBracket,
                    _ => TokenKind::Delim,
                },
                c.len_utf8(),
                false,
            ),
        }
    };
    let invalid_escape = kind != TokenKind::Comment && scan.invalid_escapes(end)?;
    if kind == TokenKind::Uri && implicit_close && invalid_escape {
        kind = TokenKind::BadUri;
        implicit_close = false;
    }
    Ok(Some(Token {
        kind,
        raw: &input[..end],
        implicit_close,
        invalid_escape,
    }))
}

/// Borrowed token stream with a cumulative work allowance. Collection storage is
/// caller-owned. Errors use original input offsets and terminate the stream.
pub struct Tokens<'a> {
    input: &'a str,
    offset: usize,
    remaining: usize,
    done: bool,
}
impl<'a> Tokens<'a> {
    pub fn new(input: &'a str, work: NonZeroUsize) -> Self {
        Self {
            input,
            offset: 0,
            remaining: work.get(),
            done: false,
        }
    }
    pub fn consumed(&self) -> usize {
        self.offset
    }
}
impl<'a> Iterator for Tokens<'a> {
    type Item = Result<Token<'a>, SyntaxError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.offset == self.input.len() {
            self.done = true;
            return None;
        }
        match scan_token_budget(&self.input[self.offset..], &mut self.remaining) {
            Ok(Some(token)) => {
                self.offset += token.raw.len();
                Some(Ok(token))
            }
            Ok(None) => {
                self.done = true;
                None
            }
            Err(mut error) => {
                error.offset += self.offset;
                self.done = true;
                Some(Err(error))
            }
        }
    }
}
impl std::iter::FusedIterator for Tokens<'_> {}
