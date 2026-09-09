//! Shared HTTP field lexical grammar, distinct from CSS syntax.
use std::{borrow::Cow, ops::Range};
#[derive(Debug)]
pub(super) enum FieldError {
    Invalid { offset: usize },
    Bytes,
    Work,
    Allocation,
}
pub(super) struct Reader<'a> {
    pub(super) input: &'a [u8],
    pub(super) offset: usize,
    pub(super) work: usize,
}
pub(super) struct Value<'a> {
    payload: &'a [u8],
    pub(super) span: Range<usize>,
    escapes: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn peek(&self) -> Option<u8> {
        self.input.get(self.offset).copied()
    }
    pub(super) fn invalid(&self) -> FieldError {
        FieldError::Invalid {
            offset: self.offset,
        }
    }
    pub(super) fn spend(&mut self, count: usize) -> Result<(), FieldError> {
        self.work = self.work.checked_sub(count).ok_or(FieldError::Work)?;
        Ok(())
    }
    pub(super) fn take(&mut self) -> Result<Option<u8>, FieldError> {
        let Some(byte) = self.peek() else {
            return Ok(None);
        };
        self.spend(1)?;
        self.offset += 1;
        Ok(Some(byte))
    }
    pub(super) fn expect(&mut self, byte: u8) -> Result<(), FieldError> {
        if self.peek() != Some(byte) {
            return Err(self.invalid());
        }
        self.take()?;
        Ok(())
    }
    pub(super) fn ows(&mut self) -> Result<(), FieldError> {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.take()?;
        }
        Ok(())
    }
    pub(super) fn token(&mut self) -> Result<Range<usize>, FieldError> {
        let start = self.offset;
        while self.peek().is_some_and(tchar) {
            self.take()?;
        }
        if start == self.offset {
            return Err(self.invalid());
        }
        Ok(start..self.offset)
    }
    pub(super) fn value(&mut self) -> Result<Value<'a>, FieldError> {
        if self.peek() != Some(b'"') {
            let span = self.token()?;
            return Ok(Value {
                payload: &self.input[span.clone()],
                span,
                escapes: 0,
            });
        }
        let start = self.offset;
        self.take()?;
        let payload_start = self.offset;
        let mut escapes = 0;
        loop {
            let before = self.offset;
            match self.take()? {
                Some(b'"') => {
                    return Ok(Value {
                        payload: &self.input[payload_start..before],
                        span: start..self.offset,
                        escapes,
                    });
                }
                Some(b'\\') => {
                    let Some(escaped) = self.take()? else {
                        return Err(self.invalid());
                    };
                    if !matches!(escaped,b'\t'|b' '..=b'~'|0x80..=0xff) {
                        return Err(FieldError::Invalid {
                            offset: self.offset - 1,
                        });
                    }
                    escapes += 1;
                }
                Some(b'\t' | b' ' | 0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff) => {}
                Some(_) => return Err(FieldError::Invalid { offset: before }),
                None => return Err(self.invalid()),
            }
        }
    }
}
impl<'a> Value<'a> {
    pub(super) fn decode(
        &self,
        reader: &mut Reader<'_>,
        maximum: usize,
    ) -> Result<Cow<'a, [u8]>, FieldError> {
        if self.escapes == 0 {
            return Ok(Cow::Borrowed(self.payload));
        }
        let length = self.payload.len() - self.escapes;
        if length > maximum {
            return Err(FieldError::Bytes);
        }
        reader.spend(
            self.payload
                .len()
                .checked_add(length)
                .ok_or(FieldError::Work)?,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| FieldError::Allocation)?;
        if bytes.capacity() > maximum {
            return Err(FieldError::Bytes);
        }
        let mut offset = 0;
        while offset < self.payload.len() {
            if self.payload[offset] == b'\\' {
                offset += 1;
            }
            bytes.push(self.payload[offset]);
            offset += 1;
        }
        Ok(Cow::Owned(bytes))
    }
}
pub(super) fn tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

/// Octets admitted in a separated field-line payload, before outer OWS exclusion.
pub(super) fn field_octet(byte: u8) -> bool {
    matches!(byte, b'\t' | b' '..=b'~' | 128..=255)
}
