use super::{ColorLimits, PropertyError, TokenKind, significant};
use crate::syntax::{SyntaxError, SyntaxFault, SyntaxResource, decode_string_budget};
use std::num::NonZeroUsize;
fn resource(offset: usize, resource: SyntaxResource) -> PropertyError {
    PropertyError::Syntax(SyntaxError {
        offset,
        fault: SyntaxFault::Resource(resource),
    })
}
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<[String; 2]>>, PropertyError> {
    let mut offset = 0;
    let mut pairs: Vec<[String; 2]> = Vec::new();
    let mut strings = 0usize;
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    while let Some((start, first)) = significant(input, &mut offset, work)? {
        if first.kind != TokenKind::String || first.invalid_escape {
            return Ok(None);
        }
        let Some((second_start, second)) = significant(input, &mut offset, work)? else {
            return Ok(None);
        };
        if second.kind != TokenKind::String || second.invalid_escape {
            return Ok(None);
        }
        if pairs.len() == pairs.capacity() {
            let available = (bytes - strings) / size_of::<[String; 2]>();
            let needed = pairs
                .len()
                .checked_add(1)
                .filter(|n| *n <= available)
                .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
            let target = needed.max(pairs.capacity().saturating_mul(2).min(available));
            *work = work
                .checked_sub(pairs.len() * size_of::<[String; 2]>())
                .ok_or_else(|| resource(start, SyntaxResource::Work))?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| resource(start, SyntaxResource::Allocation))?;
            if next.capacity() > available {
                return Err(resource(start, SyntaxResource::Bytes));
            }
            next.append(&mut pairs);
            pairs = next;
        }
        let mut pair = [String::new(), String::new()];
        for ((start, token), value) in [(start, first), (second_start, second)]
            .into_iter()
            .zip(&mut pair)
        {
            let available = bytes
                .checked_sub(pairs.capacity() * size_of::<[String; 2]>() + strings)
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
            let decoded = match decode_string_budget(token.raw, available, work) {
                Ok(value) => value,
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(None),
            };
            strings += decoded.text.capacity();
            *value = decoded.text;
        }
        pairs.push(pair);
    }
    Ok((!pairs.is_empty()).then_some(pairs))
}
