use super::{
    ColorLimits, Keyword, LengthTarget, PropertyError, SpecifiedValue, length_property, significant,
};
fn axis(value: &SpecifiedValue, horizontal: bool) -> bool {
    match value {
        SpecifiedValue::Length(_) => true,
        SpecifiedValue::Keyword(Keyword(name)) => {
            *name == "center"
                || if horizontal {
                    matches!(*name, "left" | "right")
                } else {
                    matches!(*name, "top" | "bottom")
                }
        }
        _ => false,
    }
}
fn length(value: SpecifiedValue, limits: ColorLimits) -> Result<LengthTarget, PropertyError> {
    match value {
        SpecifiedValue::Length(value) => Ok(value),
        SpecifiedValue::Keyword(Keyword(name)) => {
            let percentage = match name {
                "left" | "top" => "0%",
                "center" => "50%",
                "right" | "bottom" => "100%",
                _ => unreachable!("validated position keyword"),
            };
            LengthTarget::parse(percentage, limits.numeric).map_err(PropertyError::Numeric)
        }
        _ => unreachable!("validated position component"),
    }
}
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    let mut offset = 0;
    let Some(first) = significant(input, &mut offset, work)? else {
        return Ok(None);
    };
    let second = significant(input, &mut offset, work)?;
    if significant(input, &mut offset, work)?.is_some() {
        return Ok(None);
    }
    let Some(mut first) = length_property::position_component(first, limits, work)? else {
        return Ok(None);
    };
    let mut second = if let Some(second) = second {
        let Some(value) = length_property::position_component(second, limits, work)? else {
            return Ok(None);
        };
        value
    } else {
        SpecifiedValue::Keyword(Keyword("center"))
    };
    if !(axis(&first, true) && axis(&second, false)) {
        if matches!(first, SpecifiedValue::Keyword(_))
            && matches!(second, SpecifiedValue::Keyword(_))
            && axis(&second, true)
            && axis(&first, false)
        {
            std::mem::swap(&mut first, &mut second);
        } else {
            return Ok(None);
        }
    }
    Ok(Some(SpecifiedValue::LengthPair(
        length(first, limits)?,
        length(second, limits)?,
    )))
}
