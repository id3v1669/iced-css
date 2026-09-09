use super::owned::resource;
use super::{
    ColorLimits, ParsedProperty, PropertyError, PropertyId, PropertyResult, TokenKind,
    decode_identifier_budget, inner, significant,
};
use crate::syntax::{SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;
pub(super) fn targets(name: &str) -> Option<[&'static str; 4]> {
    Some(match name {
        "margin" => ["margin-top", "margin-right", "margin-bottom", "margin-left"],
        "padding" => [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ],
        "border-width" => [
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ],
        "border-style" => [
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        ],
        "border-color" => [
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ],
        _ => return None,
    })
}
pub(super) fn parse(
    input: &str,
    targets: [&'static str; 4],
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ParsedProperty>>, PropertyError> {
    let Some(Components {
        values: components,
        count,
    }) = components::<4>(input, work)?
    else {
        return Ok(None);
    };
    if count != 1 {
        for (start, kind, raw) in components.into_iter().flatten() {
            if kind != TokenKind::Ident {
                continue;
            }
            match decode_identifier_budget(raw, limits.syntax.retained_bytes, work) {
                Ok(value) if value.text.eq_ignore_ascii_case("inherit") => return Ok(None),
                Ok(_) => {}
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(None),
            }
        }
    }
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize || bytes / size_of::<ParsedProperty>() < 4 {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(4)
        .map_err(|_| resource(0, SyntaxResource::Allocation))?;
    let available = bytes
        .checked_sub(values.capacity() * size_of::<ParsedProperty>())
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| resource(0, SyntaxResource::Bytes))?;
    let mut leaf_limits = limits;
    leaf_limits.syntax.retained_bytes = available;
    let indices = match count {
        1 => [0, 0, 0, 0],
        2 => [0, 1, 0, 1],
        3 => [0, 1, 2, 1],
        4 => [0, 1, 2, 3],
        _ => unreachable!(),
    };
    for (target, index) in targets.into_iter().zip(indices) {
        let (start, _, raw) = components[index].expect("populated component");
        let result = inner(
            PropertyId::lookup(target).expect("registered longhand"),
            raw,
            important,
            false,
            leaf_limits,
            work,
        )
        .map_err(|mut error| {
            if let PropertyError::Syntax(error) = &mut error {
                error.offset += start;
            }
            error
        })?;
        let PropertyResult::Parsed(value) = result else {
            return Ok(None);
        };
        values.push(value);
    }
    Ok(Some(values))
}

pub(super) struct Components<'a, const N: usize> {
    pub values: [Option<(usize, TokenKind, &'a str)>; N],
    pub count: usize,
}
pub(super) fn components<'a, const N: usize>(
    input: &'a str,
    work: &mut usize,
) -> Result<Option<Components<'a, N>>, PropertyError> {
    let mut offset = 0;
    let mut components = [None; N];
    let mut count = 0;
    while let Some((start, token)) = significant(input, &mut offset, work)? {
        if count == N {
            return Ok(None);
        }
        let mut raw = token.raw;
        if token.kind == TokenKind::Function {
            let mut depth = 1usize;
            while depth != 0 {
                let Some((_, next)) = significant(input, &mut offset, work)? else {
                    break;
                };
                match next.kind {
                    TokenKind::Function | TokenKind::OpenParen => {
                        depth = depth
                            .checked_add(1)
                            .ok_or_else(|| resource(start, SyntaxResource::Depth))?;
                    }
                    TokenKind::CloseParen => depth -= 1,
                    _ => {}
                }
            }
            raw = &input[start..offset];
        }
        components[count] = Some((start, token.kind, raw));
        count += 1;
    }
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(Components {
        values: components,
        count,
    }))
}
