use super::{
    ColorLimits, Keyword, PropertyError, TokenKind, decode_identifier_budget, significant,
};
use crate::syntax::{SyntaxError, SyntaxFault, SyntaxResource, decode_uri_budget};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub uris: Vec<String>,
    pub fallback: Keyword,
}
const KEYWORDS: &[&str] = &[
    "auto",
    "crosshair",
    "default",
    "pointer",
    "move",
    "e-resize",
    "ne-resize",
    "nw-resize",
    "n-resize",
    "se-resize",
    "sw-resize",
    "s-resize",
    "w-resize",
    "text",
    "wait",
    "help",
    "progress",
];
pub(super) fn valid_fallback(keyword: Keyword) -> bool {
    KEYWORDS.contains(&keyword.as_str())
}
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
) -> Result<Option<Cursor>, PropertyError> {
    let mut offset = 0;
    let mut uris: Vec<String> = Vec::new();
    let mut strings = 0;
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    loop {
        let Some((start, token)) = significant(input, &mut offset, work)? else {
            return Ok(None);
        };
        if token.invalid_escape {
            return Ok(None);
        }
        if token.kind == TokenKind::Ident {
            let available = bytes
                .checked_sub(uris.capacity() * size_of::<String>() + strings)
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
            let Some(keyword) = KEYWORDS
                .iter()
                .find(|keyword| keyword.eq_ignore_ascii_case(&name))
            else {
                return Ok(None);
            };
            if significant(input, &mut offset, work)?.is_some() {
                return Ok(None);
            }
            return Ok(Some(Cursor {
                uris,
                fallback: Keyword(keyword),
            }));
        }
        if token.kind != TokenKind::Uri {
            return Ok(None);
        }
        if uris.len() == uris.capacity() {
            let available = (bytes - strings) / size_of::<String>();
            let needed = uris
                .len()
                .checked_add(1)
                .filter(|n| *n <= available)
                .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
            let target = needed.max(uris.capacity().saturating_mul(2).min(available));
            *work = work
                .checked_sub(uris.len() * size_of::<String>())
                .ok_or_else(|| resource(start, SyntaxResource::Work))?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| resource(start, SyntaxResource::Allocation))?;
            if next.capacity() > available {
                return Err(resource(start, SyntaxResource::Bytes));
            }
            next.append(&mut uris);
            uris = next;
        }
        let available = bytes
            .checked_sub(uris.capacity() * size_of::<String>() + strings)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
        let decoded = match decode_uri_budget(token.raw, available, work) {
            Ok(value) => value,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(None),
        };
        strings += decoded.text.capacity();
        uris.push(decoded.text);
        let Some((_, separator)) = significant(input, &mut offset, work)? else {
            return Ok(None);
        };
        if separator.kind != TokenKind::Delim || separator.raw != "," {
            return Ok(None);
        }
    }
}
