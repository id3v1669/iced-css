use super::{SyntaxError, SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericTokenKind {
    Number,
    Percentage,
    Dimension,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericToken {
    pub kind: NumericTokenKind,
    pub consumed: usize,
}

pub(super) struct Scan<'a> {
    pub(super) input: &'a str,
    pub(super) remaining: usize,
}
impl Scan<'_> {
    pub(super) fn at(&mut self, offset: usize) -> Result<Option<char>, SyntaxError> {
        self.remaining = self.remaining.checked_sub(1).ok_or(SyntaxError {
            offset,
            fault: SyntaxFault::Resource(SyntaxResource::Work),
        })?;
        Ok(self.input[offset..].chars().next())
    }
    fn digits(&mut self, mut offset: usize) -> Result<usize, SyntaxError> {
        while self.at(offset)?.is_some_and(|c| c.is_ascii_digit()) {
            offset += 1;
        }
        Ok(offset)
    }
    pub(super) fn escape(&mut self, start: usize) -> Result<Option<usize>, SyntaxError> {
        let mut end = start + 1;
        let Some(c) = self.at(end)? else {
            return Ok(None);
        };
        if matches!(c, '\r' | '\n' | '\u{c}') {
            return Ok(None);
        }
        if !c.is_ascii_hexdigit() {
            return Ok(Some(end + c.len_utf8()));
        }
        for _ in 0..6 {
            if !self.at(end)?.is_some_and(|c| c.is_ascii_hexdigit()) {
                break;
            }
            end += 1;
        }
        if let Some(c) = self
            .at(end)?
            .filter(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}'))
        {
            end += 1;
            if c == '\r' && self.at(end)? == Some('\n') {
                end += 1;
            }
        }
        Ok(Some(end))
    }
    pub(super) fn name_char(
        &mut self,
        offset: usize,
        first: bool,
    ) -> Result<Option<usize>, SyntaxError> {
        let Some(c) = self.at(offset)? else {
            return Ok(None);
        };
        if c == '\\' {
            return self.escape(offset);
        }
        // Q279 replaces literal NUL before lexical classification. Preserve its
        // original one-byte span; semantic decoding records the repair later.
        if c == '\0'
            || c >= '\u{80}'
            || c == '_'
            || c.is_ascii_alphabetic()
            || (!first && (c.is_ascii_digit() || c == '-'))
        {
            Ok(Some(offset + c.len_utf8()))
        } else {
            Ok(None)
        }
    }
    pub(super) fn ident(&mut self, start: usize) -> Result<Option<usize>, SyntaxError> {
        let mut end = start;
        if self.at(end)? == Some('-') {
            end += 1;
        }
        let Some(next) = self.name_char(end, true)? else {
            return Ok(None);
        };
        end = next;
        while let Some(next) = self.name_char(end, false)? {
            end = next;
        }
        Ok(Some(end))
    }
}
/// Recognize the chapter-4 NUMBER/PERCENTAGE/DIMENSION competition at the start
/// of decoded UTF-8 input. This does not validate a property's numeric grammar
/// or decode escaped dimension units. The original spelling remains untouched.
/// Each scalar inspection consumes one work unit, including repeated lookahead.
/// No allocation is performed and exhaustion never selects a shorter token.
pub fn scan_numeric_prefix(
    input: &str,
    work: NonZeroUsize,
) -> Result<Option<NumericToken>, SyntaxError> {
    let mut scan = Scan {
        input,
        remaining: work.get(),
    };
    scan_numeric(&mut scan)
}

pub(super) fn scan_numeric(scan: &mut Scan<'_>) -> Result<Option<NumericToken>, SyntaxError> {
    let mut start = 0;
    if matches!(scan.at(start)?, Some('+' | '-')) {
        start += 1;
    }
    let integral = scan.digits(start)?;
    let mut base = integral;
    if scan.at(integral)? == Some('.') {
        let fractional = scan.digits(integral + 1)?;
        if fractional > integral + 1 {
            base = fractional;
        }
    }
    if base == start {
        return Ok(None);
    }
    let mut number = base;
    if matches!(scan.at(base)?, Some('e' | 'E')) {
        let mut exponent = base + 1;
        if matches!(scan.at(exponent)?, Some('+' | '-')) {
            exponent += 1;
        }
        let end = scan.digits(exponent)?;
        if end > exponent {
            number = end;
        }
    }
    let mut result = NumericToken {
        kind: NumericTokenKind::Number,
        consumed: number,
    };
    // Both numeric alternatives matter: in 1e2px, the dimension grammar can
    // split at 1 or at 1e2. Tokens retain raw spans, not an arbitrary split.
    for prefix in [base, number] {
        if scan.at(prefix)? == Some('%') && prefix + 1 > result.consumed {
            result = NumericToken {
                kind: NumericTokenKind::Percentage,
                consumed: prefix + 1,
            };
        }
        if let Some(end) = scan.ident(prefix)?
            && end > result.consumed
        {
            result = NumericToken {
                kind: NumericTokenKind::Dimension,
                consumed: end,
            };
        }
        if base == number {
            break;
        }
    }
    Ok(Some(result))
}
