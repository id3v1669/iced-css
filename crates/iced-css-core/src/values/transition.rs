//! The transition properties (CSS Transitions 1, section 2), the animation properties
//! (CSS Animations 1, section 4), and the easing functions they take (CSS Easing
//! Functions 1): `linear`, the cubic Bézier keywords and function, and `steps()`.
use super::{
    ColorLimits, ParsedProperty, PropertyError, PropertyId, PropertyResult, SpecifiedValue, Token,
    TokenKind, significant,
};
use crate::{syntax::decode_identifier_budget, values::Decimal};

/// The most transitions one list may hold.
const LIST_LIMIT: usize = 256;

/// One entry of `transition-property`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionProperty {
    All,
    /// `none`: alone in its list.
    None,
    /// A property name, lowercase; an unknown one is kept and does nothing.
    Named(String),
}

/// A `<time>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeValue {
    pub value: Decimal,
    /// `ms` rather than `s`.
    pub milliseconds: bool,
}

/// Where `steps()` jumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

/// An `<easing-function>`; the keywords are kept as the functions they name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EasingValue {
    Linear,
    /// x1, y1, x2, y2; both x in 0 to 1.
    CubicBezier([Decimal; 4]),
    /// A positive count (more than one for `jump-none`).
    Steps(u32, StepPosition),
}

/// A transition or animation longhand's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionValue {
    Properties(Vec<TransitionProperty>),
    Times(Vec<TimeValue>),
    Easings(Vec<EasingValue>),
    /// `animation-name`: `None` is `none`.
    Names(Vec<Option<String>>),
    /// `animation-iteration-count`: `None` is `infinite`; never negative.
    Counts(Vec<Option<Decimal>>),
    /// `animation-direction`, `animation-fill-mode` or `animation-play-state`, lowercase.
    Keywords(Vec<&'static str>),
}

const DIRECTIONS: [&str; 4] = ["normal", "reverse", "alternate", "alternate-reverse"];
const FILL_MODES: [&str; 4] = ["none", "forwards", "backwards", "both"];
const PLAY_STATES: [&str; 2] = ["running", "paused"];

/// The keyword of `list` this token spells.
fn one_of(token: &Token<'_>, list: &[&'static str]) -> Option<&'static str> {
    (token.kind == TokenKind::Ident)
        .then(|| {
            list.iter()
                .find(|word| token.raw.eq_ignore_ascii_case(word))
        })
        .flatten()
        .copied()
}

/// An `animation-name` entry: `none`, a custom identifier, or a string.
fn animation_name(
    token: &Token<'_>,
    limits: ColorLimits,
    work: &mut usize,
) -> Option<Option<String>> {
    match token.kind {
        TokenKind::Ident if token.raw.eq_ignore_ascii_case("none") => Some(None),
        TokenKind::Ident => {
            let name = decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work)
                .ok()?
                .text;
            let reserved = ["initial", "inherit", "unset", "default"];
            (!reserved.iter().any(|word| name.eq_ignore_ascii_case(word))).then_some(Some(name))
        }
        TokenKind::String => {
            crate::syntax::decode_string_budget(token.raw, limits.syntax.retained_bytes, work)
                .ok()
                .map(|decoded| Some(decoded.text))
        }
        _ => None,
    }
}

/// An `animation-iteration-count` entry.
fn count(token: &Token<'_>, limits: ColorLimits) -> Option<Option<Decimal>> {
    if is_keyword(token, "infinite") {
        return Some(None);
    }
    (token.kind == TokenKind::Number)
        .then(|| decimal(token.raw, limits))
        .flatten()
        .filter(|number| !number.is_negative())
        .map(Some)
}

type Tokens<'a> = Vec<(usize, Token<'a>)>;

fn tokens<'a>(input: &'a str, work: &mut usize) -> Result<Option<Tokens<'a>>, PropertyError> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(token) = significant(input, &mut offset, work)? {
        if token.1.invalid_escape || found.len() > LIST_LIMIT * 8 {
            return Ok(None);
        }
        found.push(token);
    }
    Ok(Some(found))
}

/// Splits at top-level commas.
fn commas<'t, 'a>(list: &'t [(usize, Token<'a>)]) -> Vec<&'t [(usize, Token<'a>)]> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, (_, token)) in list.iter().enumerate() {
        match token.kind {
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Delim if token.raw == "," && depth == 0 => {
                parts.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&list[start..]);
    parts
}

fn is_keyword(token: &Token<'_>, keyword: &str) -> bool {
    token.kind == TokenKind::Ident && token.raw.eq_ignore_ascii_case(keyword)
}

fn decimal(text: &str, limits: ColorLimits) -> Option<Decimal> {
    Decimal::parse(text, limits.numeric).ok()
}

fn literal(text: &str, limits: ColorLimits) -> Decimal {
    decimal(text, limits).expect("a small literal")
}

/// One `<time>` token.
fn time(token: &Token<'_>, limits: ColorLimits) -> Option<TimeValue> {
    if token.kind != TokenKind::Dimension {
        return None;
    }
    let raw = token.raw;
    let lower = raw.to_ascii_lowercase();
    let (number, milliseconds) = if lower.ends_with("ms") {
        (&raw[..raw.len() - 2], true)
    } else if lower.ends_with('s') {
        (&raw[..raw.len() - 1], false)
    } else {
        return None;
    };
    // The number part alone: no other unit hides in it.
    if number.is_empty() || number.ends_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(TimeValue {
        value: decimal(number, limits)?,
        milliseconds,
    })
}

/// One easing function from the start of `list`, and how many tokens it took.
fn easing(list: &[(usize, Token<'_>)], limits: ColorLimits) -> Option<(EasingValue, usize)> {
    let (_, first) = list.first()?;
    let bezier = |numbers: [&str; 4]| {
        EasingValue::CubicBezier(numbers.map(|number| literal(number, limits)))
    };
    if first.kind == TokenKind::Ident {
        return Some((
            match first.raw.to_ascii_lowercase().as_str() {
                "linear" => EasingValue::Linear,
                "ease" => bezier(["0.25", "0.1", "0.25", "1"]),
                "ease-in" => bezier(["0.42", "0", "1", "1"]),
                "ease-out" => bezier(["0", "0", "0.58", "1"]),
                "ease-in-out" => bezier(["0.42", "0", "0.58", "1"]),
                "step-start" => EasingValue::Steps(1, StepPosition::JumpStart),
                "step-end" => EasingValue::Steps(1, StepPosition::JumpEnd),
                _ => return None,
            },
            1,
        ));
    }
    if first.kind != TokenKind::Function {
        return None;
    }
    let name = first.raw.to_ascii_lowercase();
    let close = list
        .iter()
        .position(|(_, token)| token.kind == TokenKind::CloseParen)?;
    let arguments: Vec<&[(usize, Token<'_>)]> = commas(&list[1..close]);
    let value = match name.as_str() {
        "cubic-bezier(" => {
            let numbers: Option<Vec<Decimal>> = arguments
                .iter()
                .map(|argument| match argument {
                    [(_, token)] if token.kind == TokenKind::Number => decimal(token.raw, limits),
                    _ => None,
                })
                .collect();
            let numbers: [Decimal; 4] = numbers?.try_into().ok()?;
            let unit = |number: &Decimal| {
                number
                    .scaled_finite_f32(1.0, limits.numeric)
                    .is_ok_and(|converted| (0.0..=1.0).contains(&converted.value))
            };
            if !unit(&numbers[0]) || !unit(&numbers[2]) {
                return None;
            }
            EasingValue::CubicBezier(numbers)
        }
        "steps(" => {
            let count = match arguments.first()? {
                [(_, token)] if token.kind == TokenKind::Number => {
                    token.raw.parse::<u32>().ok().filter(|count| *count > 0)?
                }
                _ => return None,
            };
            let position = match arguments.get(1) {
                None => StepPosition::JumpEnd,
                Some([(_, token)]) if token.kind == TokenKind::Ident => {
                    match token.raw.to_ascii_lowercase().as_str() {
                        "jump-start" | "start" => StepPosition::JumpStart,
                        "jump-end" | "end" => StepPosition::JumpEnd,
                        "jump-none" => StepPosition::JumpNone,
                        "jump-both" => StepPosition::JumpBoth,
                        _ => return None,
                    }
                }
                Some(_) => return None,
            };
            if arguments.len() > 2 || (position == StepPosition::JumpNone && count < 2) {
                return None;
            }
            EasingValue::Steps(count, position)
        }
        _ => return None,
    };
    Some((value, close + 1))
}

/// One `transition-property` entry.
fn property(
    token: &Token<'_>,
    limits: ColorLimits,
    work: &mut usize,
) -> Option<TransitionProperty> {
    if token.kind != TokenKind::Ident {
        return None;
    }
    let name = decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work)
        .ok()?
        .text
        .to_ascii_lowercase();
    Some(match name.as_str() {
        "all" => TransitionProperty::All,
        "none" => TransitionProperty::None,
        "initial" | "inherit" | "unset" | "default" => return None,
        _ => TransitionProperty::Named(name),
    })
}

/// A comma-separated list of one item each, `None` when any item is invalid.
fn list<T>(
    parts: &[&[(usize, Token<'_>)]],
    mut one: impl FnMut(&[(usize, Token<'_>)]) -> Option<T>,
) -> Option<Vec<T>> {
    if parts.len() > LIST_LIMIT {
        return None;
    }
    parts.iter().map(|part| one(part)).collect()
}

fn longhand(
    name: &str,
    list_tokens: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Option<TransitionValue> {
    let parts = commas(list_tokens);
    match name {
        "transition-property" => {
            let found = list(&parts, |part| match part {
                [(_, token)] => property(token, limits, work),
                _ => None,
            })?;
            // `none` stands alone.
            (found.len() == 1 || !found.contains(&TransitionProperty::None))
                .then_some(TransitionValue::Properties(found))
        }
        "transition-duration" | "transition-delay" | "animation-duration" | "animation-delay" => {
            list(&parts, |part| match part {
                [(_, token)] => time(token, limits)
                    .filter(|time| name.ends_with("delay") || !time.value.is_negative()),
                _ => None,
            })
            .map(TransitionValue::Times)
        }
        "animation-name" => list(&parts, |part| match part {
            [(_, token)] => animation_name(token, limits, work),
            _ => None,
        })
        .map(TransitionValue::Names),
        "animation-iteration-count" => list(&parts, |part| match part {
            [(_, token)] => count(token, limits),
            _ => None,
        })
        .map(TransitionValue::Counts),
        "animation-direction" | "animation-fill-mode" | "animation-play-state" => {
            let words: &[&'static str] = match name {
                "animation-direction" => &DIRECTIONS,
                "animation-fill-mode" => &FILL_MODES,
                _ => &PLAY_STATES,
            };
            list(&parts, |part| match part {
                [(_, token)] => one_of(token, words),
                _ => None,
            })
            .map(TransitionValue::Keywords)
        }
        "transition-timing-function" | "animation-timing-function" => {
            list(&parts, |part| match easing(part, limits) {
                Some((value, taken)) if taken == part.len() => Some(value),
                _ => None,
            })
            .map(TransitionValue::Easings)
        }
        _ => None,
    }
}

fn inherit(list: &[(usize, Token<'_>)]) -> bool {
    matches!(list, [(_, only)] if is_keyword(only, "inherit"))
}

/// A transition longhand, `None` when its value is invalid.
pub(super) fn parse(
    property_id: PropertyId,
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<SpecifiedValue>, PropertyError> {
    let Some(list) = tokens(input, work)? else {
        return Ok(None);
    };
    if inherit(&list) {
        return Ok(Some(SpecifiedValue::Inherit));
    }
    Ok(
        longhand(property_id.descriptor().name, &list, limits, work)
            .map(SpecifiedValue::Transition),
    )
}

/// The `transition` shorthand: per transition, a property, a duration, an easing and a
/// delay in any order, the first time the duration.
pub(super) fn shorthand(
    property_id: PropertyId,
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    let targets = property_id.longhands();
    let whole = |values: Vec<SpecifiedValue>| {
        PropertyResult::Parsed(ParsedProperty {
            property: property_id,
            value: SpecifiedValue::Expanded(
                targets
                    .iter()
                    .zip(values)
                    .map(|(longhand, value)| ParsedProperty {
                        property: *longhand,
                        value,
                        important,
                    })
                    .collect(),
            ),
            important,
        })
    };
    let Some(tokens) = tokens(input, work)? else {
        return Ok(PropertyResult::Invalid);
    };
    if inherit(&tokens) {
        return Ok(whole(vec![SpecifiedValue::Inherit; targets.len()]));
    }
    let parts = commas(&tokens);
    if parts.len() > LIST_LIMIT {
        return Ok(PropertyResult::Invalid);
    }
    let zero = TimeValue {
        value: literal("0", limits),
        milliseconds: false,
    };
    let (mut properties, mut durations, mut easings, mut delays) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for part in &parts {
        let (mut name, mut duration, mut ease, mut delay) = (None, None, None, None);
        let mut at = 0;
        if part.is_empty() {
            return Ok(PropertyResult::Invalid);
        }
        while at < part.len() {
            let token = part[at].1;
            if let Some(found) = time(&token, limits) {
                if duration.is_none() {
                    if found.value.is_negative() {
                        return Ok(PropertyResult::Invalid);
                    }
                    duration = Some(found);
                } else if delay.is_none() {
                    delay = Some(found);
                } else {
                    return Ok(PropertyResult::Invalid);
                }
                at += 1;
            } else if let Some((found, taken)) = easing(&part[at..], limits) {
                if ease.replace(found).is_some() {
                    return Ok(PropertyResult::Invalid);
                }
                at += taken;
            } else if let Some(found) = property(&token, limits, work) {
                if name.replace(found).is_some() {
                    return Ok(PropertyResult::Invalid);
                }
                at += 1;
            } else {
                return Ok(PropertyResult::Invalid);
            }
        }
        // `none` names no property only as the one transition.
        if name == Some(TransitionProperty::None) && parts.len() > 1 {
            return Ok(PropertyResult::Invalid);
        }
        properties.push(name.unwrap_or(TransitionProperty::All));
        durations.push(duration.unwrap_or_else(|| zero.clone()));
        easings.push(ease.unwrap_or_else(|| {
            EasingValue::CubicBezier(["0.25", "0.1", "0.25", "1"].map(|n| literal(n, limits)))
        }));
        delays.push(delay.unwrap_or_else(|| zero.clone()));
    }
    Ok(whole(vec![
        SpecifiedValue::Transition(TransitionValue::Properties(properties)),
        SpecifiedValue::Transition(TransitionValue::Times(durations)),
        SpecifiedValue::Transition(TransitionValue::Easings(easings)),
        SpecifiedValue::Transition(TransitionValue::Times(delays)),
    ]))
}

/// The `animation` shorthand: per animation, its duration and then its delay among the
/// times, an easing, an iteration count, a direction, a fill mode, a play state and a
/// name, in any order. A keyword another part can take goes to that part first.
pub(super) fn animation_shorthand(
    property_id: PropertyId,
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    let targets = property_id.longhands();
    let whole = |values: Vec<SpecifiedValue>| {
        PropertyResult::Parsed(ParsedProperty {
            property: property_id,
            value: SpecifiedValue::Expanded(
                targets
                    .iter()
                    .zip(values)
                    .map(|(longhand, value)| ParsedProperty {
                        property: *longhand,
                        value,
                        important,
                    })
                    .collect(),
            ),
            important,
        })
    };
    let Some(tokens) = tokens(input, work)? else {
        return Ok(PropertyResult::Invalid);
    };
    if inherit(&tokens) {
        return Ok(whole(vec![SpecifiedValue::Inherit; targets.len()]));
    }
    let parts = commas(&tokens);
    if parts.len() > LIST_LIMIT {
        return Ok(PropertyResult::Invalid);
    }
    let zero = TimeValue {
        value: literal("0", limits),
        milliseconds: false,
    };
    let mut lists: (
        Vec<Option<String>>,
        Vec<TimeValue>,
        Vec<EasingValue>,
        Vec<TimeValue>,
        Vec<Option<Decimal>>,
        [Vec<&'static str>; 3],
    ) = Default::default();
    for part in &parts {
        if part.is_empty() {
            return Ok(PropertyResult::Invalid);
        }
        let (mut name, mut duration, mut ease, mut delay, mut repeat) =
            (None, None, None, None, None);
        let mut words: [Option<&'static str>; 3] = [None; 3];
        let mut at = 0;
        while at < part.len() {
            let token = part[at].1;
            let mut taken = 1;
            let word = [&DIRECTIONS[..], &FILL_MODES[..], &PLAY_STATES[..]]
                .iter()
                .enumerate()
                .find_map(|(slot, list)| {
                    one_of(&token, list)
                        .filter(|_| words[slot].is_none())
                        .map(|word| (slot, word))
                });
            let invalid = if let Some(found) = time(&token, limits) {
                if duration.is_none() {
                    let negative = found.value.is_negative();
                    duration = Some(found);
                    negative
                } else {
                    delay.replace(found).is_some()
                }
            } else if let Some((found, used)) = easing(&part[at..], limits) {
                taken = used;
                ease.replace(found).is_some()
            } else if let Some(found) = count(&token, limits) {
                repeat.replace(found).is_some()
            } else if let Some((slot, word)) = word {
                words[slot] = Some(word);
                false
            } else if let Some(found) = animation_name(&token, limits, work) {
                name.replace(found).is_some()
            } else {
                true
            };
            if invalid {
                return Ok(PropertyResult::Invalid);
            }
            at += taken;
        }
        lists.0.push(name.flatten());
        lists.1.push(duration.unwrap_or_else(|| zero.clone()));
        lists.2.push(ease.unwrap_or_else(|| {
            EasingValue::CubicBezier(["0.25", "0.1", "0.25", "1"].map(|n| literal(n, limits)))
        }));
        lists.3.push(delay.unwrap_or_else(|| zero.clone()));
        lists
            .4
            .push(repeat.unwrap_or_else(|| Some(literal("1", limits))));
        for (slot, initial) in ["normal", "none", "running"].into_iter().enumerate() {
            lists.5[slot].push(words[slot].unwrap_or(initial));
        }
    }
    let [directions, fills, states] = lists.5;
    Ok(whole(vec![
        SpecifiedValue::Transition(TransitionValue::Names(lists.0)),
        SpecifiedValue::Transition(TransitionValue::Times(lists.1)),
        SpecifiedValue::Transition(TransitionValue::Easings(lists.2)),
        SpecifiedValue::Transition(TransitionValue::Times(lists.3)),
        SpecifiedValue::Transition(TransitionValue::Counts(lists.4)),
        SpecifiedValue::Transition(TransitionValue::Keywords(directions)),
        SpecifiedValue::Transition(TransitionValue::Keywords(fills)),
        SpecifiedValue::Transition(TransitionValue::Keywords(states)),
    ]))
}
