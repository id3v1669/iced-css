use super::{
    ColorLimits, ParsedProperty, PropertyError, PropertyId, PropertyResult, SpecifiedValue,
    TokenKind, decode_identifier_budget, inner, owned::resource, shorthand::components,
};
use crate::syntax::{SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;
pub(super) fn roles(name: &str) -> Option<[&'static str; 3]> {
    Some(match name {
        "border" | "border-top" => ["border-top-width", "border-top-style", "border-top-color"],
        "border-right" => [
            "border-right-width",
            "border-right-style",
            "border-right-color",
        ],
        "border-bottom" => [
            "border-bottom-width",
            "border-bottom-style",
            "border-bottom-color",
        ],
        "border-left" => [
            "border-left-width",
            "border-left-style",
            "border-left-color",
        ],
        "outline" => ["outline-width", "outline-style", "outline-color"],
        _ => return None,
    })
}
pub(super) fn parse(
    property: PropertyId,
    input: &str,
    roles: [&'static str; 3],
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ParsedProperty>>, PropertyError> {
    let Some(components) = components::<4>(input, work)? else {
        return Ok(None);
    };
    if components.count > 3 {
        return Ok(None);
    }
    let mut inherited = false;
    let mut assigned = [None; 3];
    for (start, kind, raw) in components.values.into_iter().flatten() {
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
        for (index, role) in roles.iter().enumerate() {
            let result = inner(
                PropertyId::lookup(role).expect("registered role"),
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
            })?;
            if matches!(result, PropertyResult::Parsed(_)) {
                if assigned[index].is_some() {
                    return Ok(None);
                }
                assigned[index] = Some((start, raw));
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(None);
        }
    }
    let targets = property.longhands();
    let bytes = limits.syntax.retained_bytes.get();
    if bytes > isize::MAX as usize || bytes / size_of::<ParsedProperty>() < targets.len() {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(targets.len())
        .map_err(|_| resource(0, SyntaxResource::Allocation))?;
    let available = bytes
        .checked_sub(values.capacity() * size_of::<ParsedProperty>())
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| resource(0, SyntaxResource::Bytes))?;
    let mut leaf_limits = limits;
    leaf_limits.syntax.retained_bytes = available;
    for &target in targets {
        let name = target.descriptor().name;
        let index = if name.ends_with("-width") {
            0
        } else if name.ends_with("-style") {
            1
        } else if name.ends_with("-color") {
            2
        } else {
            unreachable!("registered border/outline leaf")
        };
        let value = if inherited {
            SpecifiedValue::Inherit
        } else if let Some((start, raw)) = assigned[index] {
            let result =
                inner(target, raw, important, false, leaf_limits, work).map_err(|mut error| {
                    if let PropertyError::Syntax(error) = &mut error {
                        error.offset += start;
                    }
                    error
                })?;
            let PropertyResult::Parsed(value) = result else {
                return Ok(None);
            };
            value.value
        } else {
            SpecifiedValue::Initial
        };
        values.push(ParsedProperty {
            property: target,
            value,
            important,
        });
    }
    Ok(Some(values))
}
