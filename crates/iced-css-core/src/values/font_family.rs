use super::owned::{Storage, resource};
use super::{
    ColorLimits, Keyword, PropertyError, TokenKind, decode_identifier_budget, significant,
};
use crate::syntax::{SyntaxFault, SyntaxResource, decode_string_budget};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontFamily {
    Quoted(String),
    Identifiers(Vec<String>),
    Generic(Keyword),
}
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<FontFamily>>, PropertyError> {
    let mut offset = 0;
    let mut families = Vec::new();
    let mut storage = Storage {
        used: 0,
        limit: limits.syntax.retained_bytes.get(),
    };
    if storage.limit > isize::MAX as usize {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    loop {
        let Some((start, first)) = significant(input, &mut offset, work)? else {
            return Ok(None);
        };
        if first.invalid_escape {
            return Ok(None);
        }
        storage.reserve(&mut families, start, work)?;
        let pending;
        let family = if first.kind == TokenKind::String {
            let decoded = match decode_string_budget(first.raw, storage.remaining(start)?, work) {
                Ok(value) => value,
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(None),
            };
            storage.used += decoded.text.capacity();
            pending = significant(input, &mut offset, work)?;
            FontFamily::Quoted(decoded.text)
        } else if first.kind == TokenKind::Ident {
            let mut names = Vec::new();
            let mut next = Some((start, first));
            while let Some((start, token)) =
                next.filter(|(_, token)| token.kind == TokenKind::Ident)
            {
                if token.invalid_escape {
                    return Ok(None);
                }
                storage.reserve(&mut names, start, work)?;
                let name =
                    match decode_identifier_budget(token.raw, storage.remaining(start)?, work) {
                        Ok(value) => value.text,
                        Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                            error.offset += start;
                            return Err(PropertyError::Syntax(error));
                        }
                        Err(_) => return Ok(None),
                    };
                storage.used += name.capacity();
                names.push(name);
                next = significant(input, &mut offset, work)?;
            }
            pending = next;
            if names.len() == 1 {
                if ["inherit", "default", "initial"]
                    .iter()
                    .any(|key| names[0].eq_ignore_ascii_case(key))
                {
                    return Ok(None);
                }
                if let Some(generic) = ["serif", "sans-serif", "cursive", "fantasy", "monospace"]
                    .iter()
                    .find(|key| names[0].eq_ignore_ascii_case(key))
                {
                    storage.used -= names.capacity() * size_of::<String>() + names[0].capacity();
                    FontFamily::Generic(Keyword(generic))
                } else {
                    FontFamily::Identifiers(names)
                }
            } else {
                FontFamily::Identifiers(names)
            }
        } else {
            return Ok(None);
        };
        families.push(family);
        match pending {
            None => return Ok(Some(families)),
            Some((_, token)) if token.kind == TokenKind::Delim && token.raw == "," => {}
            _ => return Ok(None),
        }
    }
}
