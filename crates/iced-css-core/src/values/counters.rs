use super::{
    ColorLimits, NumericError, PropertyError, TokenKind, decode_identifier_budget, significant,
};
use crate::{
    syntax::{SyntaxError, SyntaxFault, SyntaxResource},
    values::Integer,
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterDirective {
    pub name: String,
    pub value: Integer,
}
fn resource(offset: usize, resource: SyntaxResource) -> PropertyError {
    PropertyError::Syntax(SyntaxError {
        offset,
        fault: SyntaxFault::Resource(resource),
    })
}
pub(super) fn parse(
    input: &str,
    increment: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<CounterDirective>>, PropertyError> {
    let mut offset = 0;
    let mut entries: Vec<CounterDirective> = Vec::new();
    let mut strings = 0;
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut pending = significant(input, &mut offset, work)?;
    while let Some((start, token)) = pending {
        if token.kind != TokenKind::Ident || token.invalid_escape {
            return Ok(None);
        }
        if entries.len() == entries.capacity() {
            let available = (bytes - strings) / size_of::<CounterDirective>();
            let needed = entries
                .len()
                .checked_add(1)
                .filter(|n| *n <= available)
                .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
            let target = needed.max(entries.capacity().saturating_mul(2).min(available));
            *work = work
                .checked_sub(entries.len() * size_of::<CounterDirective>())
                .ok_or_else(|| resource(start, SyntaxResource::Work))?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| resource(start, SyntaxResource::Allocation))?;
            if next.capacity() > available {
                return Err(resource(start, SyntaxResource::Bytes));
            }
            next.append(&mut entries);
            entries = next;
        }
        let available = bytes
            .checked_sub(entries.capacity() * size_of::<CounterDirective>() + strings)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
        let name = match decode_identifier_budget(token.raw, available, work) {
            Ok(value) => value.text,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(None),
        };
        if ["none", "inherit", "initial"]
            .iter()
            .any(|reserved| name.eq_ignore_ascii_case(reserved))
        {
            return Ok(None);
        }
        pending = significant(input, &mut offset, work)?;
        let raw = if let Some((_, token)) =
            pending.filter(|(_, token)| token.kind == TokenKind::Number)
        {
            pending = significant(input, &mut offset, work)?;
            token.raw
        } else if increment {
            "1"
        } else {
            "0"
        };
        let value = match Integer::parse(raw, limits.numeric) {
            Ok(value) => value,
            Err(NumericError::InvalidNumber) => return Ok(None),
            Err(error) => return Err(PropertyError::Numeric(error)),
        };
        strings += name.capacity();
        entries.push(CounterDirective { name, value });
    }
    Ok((!entries.is_empty()).then_some(entries))
}
