use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct StringLimits {
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closure {
    Explicit,
    EndOfInput,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    LiteralNul,
    OutOfRangeEscape,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repair {
    pub start: usize,
    pub end: usize,
    pub kind: RepairKind,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DecodedString {
    pub text: String,
    pub consumed: usize,
    pub closure: Closure,
    pub repairs: Vec<Repair>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxResource {
    Entries,
    Depth,
    Bytes,
    Work,
    Allocation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxFault {
    InvalidSelector,
    InvalidUri,
    InvalidIdentifier,
    ExpectedQuote,
    UnexpectedNewline,
    InvalidNumericEscape(u32),
    Resource(SyntaxResource),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxError {
    pub offset: usize,
    pub fault: SyntaxFault,
}
impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "string decoding {:?} at byte {}",
            self.fault, self.offset
        )
    }
}
impl std::error::Error for SyntaxError {}

struct Decoder<'a> {
    input: &'a str,
    offset: usize,
    work: usize,
    bytes: usize,
    text: String,
    repairs: Vec<Repair>,
}
impl Decoder<'_> {
    fn fault(&self, fault: SyntaxFault) -> SyntaxError {
        SyntaxError {
            offset: self.offset,
            fault,
        }
    }
    fn resource(&self, resource: SyntaxResource) -> SyntaxError {
        self.fault(SyntaxFault::Resource(resource))
    }
    fn charge(&mut self, amount: usize) -> Result<(), SyntaxError> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or_else(|| self.resource(SyntaxResource::Work))?;
        Ok(())
    }
    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }
    fn take(&mut self) -> Result<Option<char>, SyntaxError> {
        let value = self.peek();
        if let Some(c) = value {
            self.charge(c.len_utf8())?;
            self.offset += c.len_utf8();
        }
        Ok(value)
    }
    fn push(&mut self, c: char) -> Result<(), SyntaxError> {
        let available = self
            .bytes
            .checked_sub(self.repairs.capacity() * size_of::<Repair>())
            .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
        let needed = self
            .text
            .len()
            .checked_add(c.len_utf8())
            .filter(|n| *n <= available)
            .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
        if needed > self.text.capacity() {
            let target = needed.max(self.text.capacity().saturating_mul(2).min(available));
            self.charge(self.text.len())?;
            let mut next = String::new();
            next.try_reserve_exact(target)
                .map_err(|_| self.resource(SyntaxResource::Allocation))?;
            if next.capacity() > available {
                return Err(self.resource(SyntaxResource::Bytes));
            }
            next.push_str(&self.text);
            self.text = next;
        }
        self.text.push(c);
        Ok(())
    }
    fn repair(&mut self, repair: Repair) -> Result<(), SyntaxError> {
        let available = (self.bytes - self.text.capacity()) / size_of::<Repair>();
        let needed = self
            .repairs
            .len()
            .checked_add(1)
            .filter(|n| *n <= available)
            .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
        if needed > self.repairs.capacity() {
            let target = needed.max(self.repairs.capacity().saturating_mul(2).min(available));
            self.charge(self.repairs.len() * size_of::<Repair>())?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| self.resource(SyntaxResource::Allocation))?;
            if next.capacity() > available {
                return Err(self.resource(SyntaxResource::Bytes));
            }
            next.extend_from_slice(&self.repairs);
            self.repairs = next;
        }
        self.repairs.push(repair);
        Ok(())
    }
    fn literal(&mut self, c: char, start: usize) -> Result<(), SyntaxError> {
        if c == '\0' {
            self.push('\u{fffd}')?;
            self.repair(Repair {
                start,
                end: start + 1,
                kind: RepairKind::LiteralNul,
            })
        } else {
            self.push(c)
        }
    }
    fn continuation(&mut self, c: char) -> Result<(), SyntaxError> {
        if c == '\r' && self.peek() == Some('\n') {
            self.take()?;
        }
        Ok(())
    }
    fn escape(&mut self, start: usize) -> Result<(), SyntaxError> {
        let escaped_start = self.offset;
        let Some(c) = self.take()? else {
            return self.push('\\');
        };
        if matches!(c, '\r' | '\n' | '\u{c}') {
            return self.continuation(c);
        }
        if !c.is_ascii_hexdigit() {
            return self.literal(c, escaped_start);
        }
        let mut value = c.to_digit(16).expect("ASCII hex digit");
        for _ in 1..6 {
            let Some(next) = self.peek().filter(char::is_ascii_hexdigit) else {
                break;
            };
            self.take()?;
            value = value * 16 + next.to_digit(16).expect("ASCII hex digit");
        }
        let end = self.offset;
        if let Some(ws) = self
            .peek()
            .filter(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}'))
        {
            self.take()?;
            self.continuation(ws)?;
        }
        match value {
            0 | 0xd800..=0xdfff => Err(SyntaxError {
                offset: start,
                fault: SyntaxFault::InvalidNumericEscape(value),
            }),
            0x110000.. => {
                self.push('\u{fffd}')?;
                self.repair(Repair {
                    start,
                    end,
                    kind: RepairKind::OutOfRangeEscape,
                })
            }
            _ => self.push(char::from_u32(value).expect("validated scalar")),
        }
    }
}
/// Decode a quoted CSS string from already decoded UTF-8 input.
///
/// Offsets refer to this input, not transport bytes. Retained bytes charge actual
/// output capacities; work charges consumed bytes and copies during growth.
/// Temporary growth allocations and fixed decoder state are outside that budget.
/// No partial value is returned on error.
pub fn decode_string(input: &str, limits: StringLimits) -> Result<DecodedString, SyntaxError> {
    let mut work = limits.work.get();
    decode_string_budget(input, limits.retained_bytes, &mut work)
}
pub(crate) fn decode_string_budget(
    input: &str,
    retained_bytes: NonZeroUsize,
    work: &mut usize,
) -> Result<DecodedString, SyntaxError> {
    let mut decoder = Decoder {
        input,
        offset: 0,
        work: *work,
        bytes: retained_bytes.get(),
        text: String::new(),
        repairs: Vec::new(),
    };
    if decoder.bytes > isize::MAX as usize {
        return Err(decoder.resource(SyntaxResource::Bytes));
    }
    let quote = match decoder.peek() {
        Some(c @ ('\'' | '"')) => c,
        _ => return Err(decoder.fault(SyntaxFault::ExpectedQuote)),
    };
    decoder.take()?;
    let closure = loop {
        let start = decoder.offset;
        match decoder.take()? {
            None => break Closure::EndOfInput,
            Some(c) if c == quote => break Closure::Explicit,
            Some('\r' | '\n' | '\u{c}') => {
                return Err(SyntaxError {
                    offset: start,
                    fault: SyntaxFault::UnexpectedNewline,
                });
            }
            Some('\\') => decoder.escape(start)?,
            Some(c) => decoder.literal(c, start)?,
        }
    };
    *work = decoder.work;
    Ok(DecodedString {
        text: decoder.text,
        consumed: decoder.offset,
        closure,
        repairs: decoder.repairs,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedIdentifier {
    pub text: String,
    pub repairs: Vec<Repair>,
}

/// Decode one complete CSS2.2 identifier. Escaped values are not retokenized;
/// an escaped digit may therefore begin the decoded value. Case is preserved.
pub fn decode_identifier(
    input: &str,
    limits: StringLimits,
) -> Result<DecodedIdentifier, SyntaxError> {
    let mut work = limits.work.get();
    decode_identifier_budget(input, limits.retained_bytes, &mut work)
}

pub(crate) fn decode_identifier_budget(
    input: &str,
    retained_bytes: NonZeroUsize,
    work: &mut usize,
) -> Result<DecodedIdentifier, SyntaxError> {
    decode_name_budget(input, retained_bytes, work, true)
}

pub(crate) fn decode_name_budget(
    input: &str,
    retained_bytes: NonZeroUsize,
    work: &mut usize,
    identifier: bool,
) -> Result<DecodedIdentifier, SyntaxError> {
    let mut decoder = Decoder {
        input,
        offset: 0,
        work: *work,
        bytes: retained_bytes.get(),
        text: String::new(),
        repairs: Vec::new(),
    };
    if decoder.bytes > isize::MAX as usize {
        return Err(decoder.resource(SyntaxResource::Bytes));
    }
    let mut scan = super::numeric::Scan {
        input,
        remaining: decoder.work,
    };
    let end = if identifier {
        scan.ident(0)?
    } else {
        let mut end = 0;
        while let Some(next) = scan.name_char(end, false)? {
            end = next;
        }
        (end > 0).then_some(end)
    };
    decoder.work = scan.remaining;
    if end != Some(input.len()) {
        return Err(SyntaxError {
            offset: end.unwrap_or(0),
            fault: SyntaxFault::InvalidIdentifier,
        });
    }
    while decoder.offset < input.len() {
        let start = decoder.offset;
        let c = decoder.take()?.expect("within validated identifier");
        if c == '\\' {
            decoder.escape(start)?;
        } else {
            decoder.literal(c, start)?;
        }
    }
    *work = decoder.work;
    Ok(DecodedIdentifier {
        text: decoder.text,
        repairs: decoder.repairs,
    })
}

/// Decoded URI token, before base-URI resolution or resource loading.
pub fn decode_uri(input: &str, limits: StringLimits) -> Result<DecodedString, SyntaxError> {
    let mut work = limits.work.get();
    decode_uri_budget(input, limits.retained_bytes, &mut work)
}

pub(crate) fn decode_uri_budget(
    input: &str,
    retained_bytes: NonZeroUsize,
    work: &mut usize,
) -> Result<DecodedString, SyntaxError> {
    let invalid = || SyntaxError {
        offset: 0,
        fault: SyntaxFault::InvalidUri,
    };
    let token = super::token::scan_token_budget(input, work)?.ok_or_else(invalid)?;
    if token.kind != super::TokenKind::Uri || token.raw.len() != input.len() || token.invalid_escape
    {
        return Err(invalid());
    }
    let mut decoder = Decoder {
        input,
        offset: 0,
        work: *work,
        bytes: retained_bytes.get(),
        text: String::new(),
        repairs: Vec::new(),
    };
    if decoder.bytes > isize::MAX as usize {
        return Err(decoder.resource(SyntaxResource::Bytes));
    }
    // A validated URI token's opening parenthesis follows its url identifier.
    while decoder.take()? != Some('(') {}
    while decoder.peek().is_some_and(uri_space) {
        decoder.take()?;
    }
    if let Some(quote @ ('\'' | '"')) = decoder.peek() {
        decoder.take()?;
        loop {
            let start = decoder.offset;
            match decoder.take()? {
                None => break,
                Some(c) if c == quote => break,
                Some('\\') => decoder.escape(start)?,
                Some(c) => decoder.literal(c, start)?,
            }
        }
    } else {
        while decoder.peek().is_some_and(|c| c != ')' && !uri_space(c)) {
            let start = decoder.offset;
            match decoder.take()?.expect("validated URI payload") {
                '\\' => decoder.escape(start)?,
                c => decoder.literal(c, start)?,
            }
        }
    }
    // The scanner already verified the remaining whitespace and closing delimiter.
    while decoder.take()?.is_some() {}
    *work = decoder.work;
    Ok(DecodedString {
        text: decoder.text,
        repairs: decoder.repairs,
        consumed: decoder.offset,
        closure: if token.implicit_close {
            Closure::EndOfInput
        } else {
            Closure::Explicit
        },
    })
}
fn uri_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}
