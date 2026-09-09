use super::{
    ColorLimits, Keyword, ParsedProperty, PropertyError, PropertyId, PropertyResult,
    SpecifiedValue, TokenKind, decode_identifier_budget, inner, owned::resource, significant,
};
use crate::syntax::{SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;
const ROLES: [&str; 6] = [
    "font-style",
    "font-variant",
    "font-weight",
    "font-size",
    "line-height",
    "font-family",
];
fn leaf(
    name: &str,
    start: usize,
    raw: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    inner(
        PropertyId::lookup(name).expect("registered font role"),
        raw,
        important,
        false,
        limits,
        work,
    )
    .map_err(|mut error| {
        if let PropertyError::Syntax(error) = &mut error {
            error.offset += start;
        }
        error
    })
}
#[derive(Clone, Copy)]
enum Whole {
    Inherit,
    System(Keyword),
}
pub(super) fn parse(
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ParsedProperty>>, PropertyError> {
    let mut offset = 0;
    let Some(first) = significant(input, &mut offset, work)? else {
        return Ok(None);
    };
    let mut whole = None;
    if first.1.kind == TokenKind::Ident {
        let name = match decode_identifier_budget(first.1.raw, limits.syntax.retained_bytes, work) {
            Ok(value) => value.text,
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += first.0;
                return Err(PropertyError::Syntax(error));
            }
            Err(_) => return Ok(None),
        };
        if name.eq_ignore_ascii_case("inherit") {
            whole = Some(Whole::Inherit);
        } else if let Some(keyword) = [
            "caption",
            "icon",
            "menu",
            "message-box",
            "small-caption",
            "status-bar",
        ]
        .iter()
        .find(|key| key.eq_ignore_ascii_case(&name))
        {
            whole = Some(Whole::System(Keyword(keyword)));
        }
        if whole.is_some() && significant(input, &mut offset, work)?.is_some() {
            return Ok(None);
        }
    }
    let mut assigned = [None; 6];
    if whole.is_none() {
        let mut pending = Some(first);
        let mut normals = 0;
        let mut normal_raw = None;
        let mut prefix_count = 0;
        loop {
            let Some((start, token)) = pending else {
                return Ok(None);
            };
            if token.invalid_escape {
                return Ok(None);
            }
            let mut normal = false;
            if token.kind == TokenKind::Ident {
                match decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work) {
                    Ok(value) if value.text.eq_ignore_ascii_case("inherit") => return Ok(None),
                    Ok(value) => normal = value.text.eq_ignore_ascii_case("normal"),
                    Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                        error.offset += start;
                        return Err(PropertyError::Syntax(error));
                    }
                    Err(_) => return Ok(None),
                }
            }
            if !normal
                && matches!(
                    leaf("font-size", start, token.raw, important, limits, work)?,
                    PropertyResult::Parsed(_)
                )
            {
                assigned[3] = Some((start, token.raw));
                break;
            }
            if prefix_count == 3 {
                return Ok(None);
            }
            prefix_count += 1;
            if normal {
                normals += 1;
                normal_raw = Some((start, token.raw));
            } else {
                let mut matched = false;
                for (index, role) in ROLES[..3].iter().enumerate() {
                    if matches!(
                        leaf(role, start, token.raw, important, limits, work)?,
                        PropertyResult::Parsed(_)
                    ) {
                        if assigned[index].is_some() {
                            return Ok(None);
                        }
                        assigned[index] = Some((start, token.raw));
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return Ok(None);
                }
            }
            pending = significant(input, &mut offset, work)?;
        }
        if normals != 0 {
            if normals > assigned[..3].iter().filter(|role| role.is_none()).count() {
                return Ok(None);
            }
            for role in &mut assigned[..3] {
                if role.is_none() {
                    *role = normal_raw;
                }
            }
        }
        let Some(mut next) = significant(input, &mut offset, work)? else {
            return Ok(None);
        };
        if next.1.kind == TokenKind::Delim && next.1.raw == "/" {
            let Some(line) = significant(input, &mut offset, work)? else {
                return Ok(None);
            };
            let PropertyResult::Parsed(value) =
                leaf("line-height", line.0, line.1.raw, important, limits, work)?
            else {
                return Ok(None);
            };
            if value.value == SpecifiedValue::Inherit {
                return Ok(None);
            }
            assigned[4] = Some((line.0, line.1.raw));
            let Some(family) = significant(input, &mut offset, work)? else {
                return Ok(None);
            };
            next = family;
        }
        assigned[5] = Some((next.0, &input[next.0..]));
    }
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize || bytes / size_of::<ParsedProperty>() < ROLES.len() {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(ROLES.len())
        .map_err(|_| resource(0, SyntaxResource::Allocation))?;
    let available = bytes
        .checked_sub(values.capacity() * size_of::<ParsedProperty>())
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| resource(0, SyntaxResource::Bytes))?;
    let mut leaf_limits = limits;
    leaf_limits.syntax.retained_bytes = available;
    // The only syntax-owned dynamic leaf (family) is emitted last. Exact numeric
    // storage belongs to the separately bounded numeric operands.
    for (role, raw) in ROLES.into_iter().zip(assigned) {
        let property = PropertyId::lookup(role).expect("registered font role");
        let value = match whole {
            Some(Whole::Inherit) => SpecifiedValue::Inherit,
            Some(Whole::System(keyword)) => SpecifiedValue::SystemFont(keyword),
            None => {
                if let Some((start, raw)) = raw {
                    let PropertyResult::Parsed(value) =
                        leaf(role, start, raw, important, leaf_limits, work)?
                    else {
                        return Ok(None);
                    };
                    if value.value == SpecifiedValue::Inherit {
                        return Ok(None);
                    }
                    value.value
                } else {
                    SpecifiedValue::Initial
                }
            }
        };
        values.push(ParsedProperty {
            property,
            value,
            important,
        });
    }
    Ok(Some(values))
}
