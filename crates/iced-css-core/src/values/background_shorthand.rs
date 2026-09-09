use super::{
    ColorLimits, ParsedProperty, PropertyError, PropertyId, PropertyResult, SpecifiedValue,
    TokenKind, decode_identifier_budget, inner, owned::resource, shorthand::components,
};
use crate::syntax::{SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;
const ROLES: [&str; 5] = [
    "background-color",
    "background-image",
    "background-repeat",
    "background-attachment",
    "background-position",
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
        PropertyId::lookup(name).expect("registered background role"),
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
pub(super) fn parse(
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ParsedProperty>>, PropertyError> {
    let Some(components) = components::<6>(input, work)? else {
        return Ok(None);
    };
    let mut assigned = [None; 5];
    let mut inherited = false;
    let mut index = 0;
    while index < components.count {
        let (start, kind, raw) = components.values[index].expect("populated component");
        if kind == TokenKind::Ident {
            match decode_identifier_budget(raw, limits.syntax.retained_bytes, work) {
                Ok(value) if value.text.eq_ignore_ascii_case("inherit") => {
                    if components.count != 1 {
                        return Ok(None);
                    }
                    inherited = true;
                    break;
                }
                Ok(_) => {}
                Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                    error.offset += start;
                    return Err(PropertyError::Syntax(error));
                }
                Err(_) => return Ok(None),
            }
        }
        let mut matched = false;
        for (role_index, role) in ROLES.iter().enumerate() {
            let mut candidate = raw;
            let mut count = 1;
            if role_index == 4 && index + 1 < components.count {
                let (next_start, _, next_raw) =
                    components.values[index + 1].expect("populated component");
                let pair = &input[start..next_start + next_raw.len()];
                if matches!(
                    leaf(role, start, pair, important, limits, work)?,
                    PropertyResult::Parsed(_)
                ) {
                    candidate = pair;
                    count = 2;
                }
            }
            if matches!(
                leaf(role, start, candidate, important, limits, work)?,
                PropertyResult::Parsed(_)
            ) {
                if assigned[role_index].is_some() {
                    return Ok(None);
                }
                assigned[role_index] = Some((start, candidate));
                index += count;
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(None);
        }
    }
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize || bytes / size_of::<ParsedProperty>() < ROLES.len() {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(ROLES.len())
        .map_err(|_| resource(0, SyntaxResource::Allocation))?;
    let mut available = bytes
        .checked_sub(values.capacity() * size_of::<ParsedProperty>())
        .ok_or_else(|| resource(0, SyntaxResource::Bytes))?;
    for (role, raw) in ROLES.into_iter().zip(assigned) {
        let property = PropertyId::lookup(role).expect("registered background role");
        let value = if inherited {
            SpecifiedValue::Inherit
        } else if let Some((start, raw)) = raw {
            let mut leaf_limits = limits;
            leaf_limits.syntax.retained_bytes = NonZeroUsize::new(available)
                .ok_or_else(|| resource(start, SyntaxResource::Bytes))?;
            let PropertyResult::Parsed(value) =
                leaf(role, start, raw, important, leaf_limits, work)?
            else {
                return Ok(None);
            };
            if let SpecifiedValue::Uri(uri) = &value.value {
                available -= uri.capacity();
            }
            value.value
        } else {
            SpecifiedValue::Initial
        };
        values.push(ParsedProperty {
            property,
            value,
            important,
        });
    }
    Ok(Some(values))
}
