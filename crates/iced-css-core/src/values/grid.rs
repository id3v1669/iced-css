//! The grid properties (CSS Grid Layout 1, section 7 and 8): track lists, named areas,
//! the auto-placement flow and line placements, and their shorthands.
//!
//! `subgrid` takes `[ ]` line names after it, not `repeat()` of them. Left out: `masonry`, and `[ ]` line names on the `grid-template` areas form
//! after the last row's size are read as the next row's.
use super::{
    ColorLimits, LengthTarget, ParsedProperty, PropertyError, PropertyId, PropertyResult,
    SpecifiedValue, Token, TokenKind, length_property, significant,
};
use crate::{
    syntax::{decode_identifier_budget, decode_string_budget},
    values::{Decimal, Integer},
};

/// The most tracks one repetition, or one axis, may hold, and the largest line number or
/// span; larger values are clamped to it (CSS Grid 1, 5.3).
pub const GRID_LIMIT: u32 = 1000;

/// One side of a track's sizing function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridBreadth {
    /// A length or percentage, never negative.
    Length(LengthTarget),
    /// `<flex>`: a number of `fr`, never negative.
    Flex(Decimal),
    Auto,
    MinContent,
    MaxContent,
}

/// `<track-size>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridTrackSize {
    Breadth(GridBreadth),
    /// `minmax(min, max)`; the minimum is never `<flex>`.
    MinMax(GridBreadth, GridBreadth),
    /// `fit-content(<length-percentage>)`.
    FitContent(LengthTarget),
}

/// How many times a `repeat()` repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridRepeat {
    /// At least one, at most [`GRID_LIMIT`].
    Count(u32),
    AutoFill,
    AutoFit,
}

/// One part of a track list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridTrackEntry {
    /// `[name ...]`: names for the line at this point.
    Names(Vec<String>),
    Size(GridTrackSize),
    /// `repeat()`: its entries are names and sizes, never another repetition.
    Repeat(GridRepeat, Vec<GridTrackEntry>),
}

/// One line placement (`grid-row-start` and the like). Auto is all empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GridLineValue {
    pub span: bool,
    /// Never zero; zero is none given. A span's is positive.
    pub number: i32,
    pub name: Option<String>,
}

/// A grid property's specified value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridValue {
    /// `none`, for `grid-template-*`.
    None,
    /// A track list, or for `grid-auto-*` a list of sizes only.
    Tracks(Vec<GridTrackEntry>),
    /// CSS Grid 2 `subgrid`, for `grid-template-*`: the parent grid's tracks, with the
    /// names given for its lines from the first.
    Subgrid(Vec<Vec<String>>),
    /// `grid-template-areas`: per row, per column, the cell's area name; `None` is a `.`.
    /// The rows are as long as each other, and each name fills a rectangle.
    Areas(Vec<Vec<Option<String>>>),
    /// `grid-auto-flow`.
    Flow { column: bool, dense: bool },
    /// A line placement.
    Line(GridLineValue),
}

type Tokens<'a> = Vec<(usize, Token<'a>)>;

fn tokens<'a>(input: &'a str, work: &mut usize) -> Result<Option<Tokens<'a>>, PropertyError> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(token) = significant(input, &mut offset, work)? {
        if token.1.invalid_escape {
            return Ok(None);
        }
        // A runaway list is no valid value.
        if found.len() > GRID_LIMIT as usize * 4 {
            return Ok(None);
        }
        found.push(token);
    }
    Ok(Some(found))
}

fn ident(token: &Token<'_>, limits: ColorLimits, work: &mut usize) -> Option<String> {
    (token.kind == TokenKind::Ident)
        .then(|| decode_identifier_budget(token.raw, limits.syntax.retained_bytes, work).ok())
        .flatten()
        .map(|name| name.text)
}

fn is_keyword(token: &Token<'_>, keyword: &str) -> bool {
    token.kind == TokenKind::Ident && token.raw.eq_ignore_ascii_case(keyword)
}

/// A `<custom-ident>` that may name a line or area: not a CSS-wide keyword nor one the
/// grammar reserves.
fn custom_ident(token: &Token<'_>, limits: ColorLimits, work: &mut usize) -> Option<String> {
    let name = ident(token, limits, work)?;
    let reserved = ["span", "auto", "inherit", "initial", "unset", "default"];
    (!reserved
        .iter()
        .any(|keyword| name.eq_ignore_ascii_case(keyword)))
    .then_some(name)
}

/// `<track-breadth>` (or `<inflexible-breadth>` when `flex` is false).
fn breadth(
    token: (usize, Token<'_>),
    flex: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<GridBreadth>, PropertyError> {
    let raw = token.1.raw;
    if token.1.kind == TokenKind::Ident {
        return Ok(match raw.to_ascii_lowercase().as_str() {
            "auto" => Some(GridBreadth::Auto),
            "min-content" => Some(GridBreadth::MinContent),
            "max-content" => Some(GridBreadth::MaxContent),
            _ => None,
        });
    }
    if token.1.kind == TokenKind::Dimension
        && raw.len() > 2
        && raw[raw.len() - 2..].eq_ignore_ascii_case("fr")
    {
        if !flex {
            return Ok(None);
        }
        return Ok(
            match Decimal::parse(&raw[..raw.len() - 2], limits.numeric) {
                Ok(number) if !number.is_negative() => Some(GridBreadth::Flex(number)),
                _ => None,
            },
        );
    }
    Ok(length(token, limits, work)?.map(GridBreadth::Length))
}

/// A `<length-percentage>` that is not negative.
fn length(
    token: (usize, Token<'_>),
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<LengthTarget>, PropertyError> {
    Ok(match length_property::track_length(token, limits, work)? {
        Some(SpecifiedValue::Length(target)) => Some(target),
        _ => None,
    })
}

/// The tokens of a function's arguments, from just after `at` (the function token) to its
/// closing parenthesis, split at top-level commas; and the index after that parenthesis.
fn arguments<'t, 'a>(
    list: &'t [(usize, Token<'a>)],
    at: usize,
) -> Option<(Vec<&'t [(usize, Token<'a>)]>, usize)> {
    let mut depth = 0usize;
    let mut parts = Vec::new();
    let mut start = at + 1;
    for (index, (_, token)) in list.iter().enumerate().skip(at + 1) {
        match token.kind {
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen if depth == 0 => {
                parts.push(&list[start..index]);
                return Some((parts, index + 1));
            }
            TokenKind::CloseParen => depth -= 1,
            TokenKind::Delim if token.raw == "," && depth == 0 => {
                parts.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    None
}

/// One `<track-size>` starting at `at`, and the index after it.
fn track_size(
    list: &[(usize, Token<'_>)],
    at: usize,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<(GridTrackSize, usize)>, PropertyError> {
    let token = list[at];
    if token.1.kind == TokenKind::Function {
        let name = token.1.raw.to_ascii_lowercase();
        // A `calc()` is one whole token.
        if name.starts_with("calc(") {
            return Ok(breadth(token, true, limits, work)?
                .map(|value| (GridTrackSize::Breadth(value), at + 1)));
        }
        let Some((parts, next)) = arguments(list, at) else {
            return Ok(None);
        };
        return Ok(match (name.as_str(), parts.as_slice()) {
            ("minmax(", [[min], [max]]) => {
                match (
                    breadth(*min, false, limits, work)?,
                    breadth(*max, true, limits, work)?,
                ) {
                    (Some(min), Some(max)) => Some((GridTrackSize::MinMax(min, max), next)),
                    _ => None,
                }
            }
            ("fit-content(", [[limit]]) => {
                length(*limit, limits, work)?.map(|limit| (GridTrackSize::FitContent(limit), next))
            }
            _ => None,
        });
    }
    Ok(breadth(token, true, limits, work)?.map(|value| (GridTrackSize::Breadth(value), at + 1)))
}

/// `[name ...]` starting at `at`, and the index after it.
fn line_names(
    list: &[(usize, Token<'_>)],
    at: usize,
    limits: ColorLimits,
    work: &mut usize,
) -> Option<(Vec<String>, usize)> {
    let mut names = Vec::new();
    for (index, (_, token)) in list.iter().enumerate().skip(at + 1) {
        if token.kind == TokenKind::CloseBracket {
            return Some((names, index + 1));
        }
        names.push(custom_ident(token, limits, work)?);
    }
    None
}

/// Whether a size is a `<fixed-size>`: one side, at least, a length, and no `<flex>` or
/// content size alone.
fn fixed(size: &GridTrackSize) -> bool {
    let length = |breadth: &GridBreadth| matches!(breadth, GridBreadth::Length(_));
    match size {
        GridTrackSize::Breadth(breadth) => length(breadth),
        GridTrackSize::MinMax(min, max) => {
            length(min) || (length(max) && !matches!(min, GridBreadth::Flex(_)))
        }
        GridTrackSize::FitContent(_) => false,
    }
}

/// A list of names and sizes, with `repeat()` allowed when `repeats`.
fn entries(
    list: &[(usize, Token<'_>)],
    repeats: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<GridTrackEntry>>, PropertyError> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < list.len() {
        let token = list[at].1;
        if token.kind == TokenKind::OpenBracket {
            let Some((names, next)) = line_names(list, at, limits, work) else {
                return Ok(None);
            };
            // Two name lists in a row make one line's.
            if let Some(GridTrackEntry::Names(previous)) = found.last_mut() {
                previous.extend(names);
            } else {
                found.push(GridTrackEntry::Names(names));
            }
            at = next;
            continue;
        }
        if repeats && token.kind == TokenKind::Function && token.raw.eq_ignore_ascii_case("repeat(")
        {
            let Some((parts, next)) = arguments(list, at) else {
                return Ok(None);
            };
            let [[count], inner] = parts.as_slice() else {
                return Ok(None);
            };
            let count = if is_keyword(&count.1, "auto-fill") {
                GridRepeat::AutoFill
            } else if is_keyword(&count.1, "auto-fit") {
                GridRepeat::AutoFit
            } else if count.1.kind == TokenKind::Number {
                match Integer::parse(count.1.raw, limits.numeric) {
                    Ok(number) if !number.is_negative() && !number.is_zero() => {
                        GridRepeat::Count(number.saturating_i32().unsigned_abs().min(GRID_LIMIT))
                    }
                    _ => return Ok(None),
                }
            } else {
                return Ok(None);
            };
            let Some(inner) = entries(inner, false, limits, work)? else {
                return Ok(None);
            };
            let sizes: Vec<&GridTrackSize> = inner
                .iter()
                .filter_map(|entry| match entry {
                    GridTrackEntry::Size(size) => Some(size),
                    _ => None,
                })
                .collect();
            if sizes.is_empty()
                || (matches!(count, GridRepeat::AutoFill | GridRepeat::AutoFit)
                    && !sizes.iter().all(|size| fixed(size)))
            {
                return Ok(None);
            }
            found.push(GridTrackEntry::Repeat(count, inner));
            at = next;
            continue;
        }
        let Some((size, next)) = track_size(list, at, limits, work)? else {
            return Ok(None);
        };
        found.push(GridTrackEntry::Size(size));
        at = next;
    }
    Ok(Some(found))
}

/// `<track-list> | <auto-track-list>`: at least one size; at most one automatic
/// repetition, and then every other size fixed.
fn track_list(
    list: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<GridTrackEntry>>, PropertyError> {
    let Some(found) = entries(list, true, limits, work)? else {
        return Ok(None);
    };
    let automatic = found
        .iter()
        .filter(|entry| {
            matches!(
                entry,
                GridTrackEntry::Repeat(GridRepeat::AutoFill | GridRepeat::AutoFit, _)
            )
        })
        .count();
    let has_size = found
        .iter()
        .any(|entry| !matches!(entry, GridTrackEntry::Names(_)));
    let others_fixed = found.iter().all(|entry| match entry {
        GridTrackEntry::Size(size) => fixed(size),
        GridTrackEntry::Repeat(GridRepeat::Count(_), inner) => {
            inner.iter().all(|entry| match entry {
                GridTrackEntry::Size(size) => fixed(size),
                _ => true,
            })
        }
        _ => true,
    });
    Ok((has_size && (automatic == 0 || (automatic == 1 && others_fixed))).then_some(found))
}

/// The cells of one `grid-template-areas` string, or `None` when it holds anything but
/// names, dots and white space.
fn area_row(text: &str) -> Option<Vec<Option<String>>> {
    let mut cells = Vec::new();
    let mut chars = text.chars().peekable();
    let name_char = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '.' {
            while chars.peek() == Some(&'.') {
                chars.next();
            }
            cells.push(None);
        } else if name_char(c) {
            let mut name = String::new();
            while let Some(&c) = chars.peek().filter(|c| name_char(**c)) {
                name.push(c);
                chars.next();
            }
            cells.push(Some(name));
        } else {
            return None;
        }
    }
    Some(cells)
}

/// Whether every named area of `rows` is one filled rectangle, and the rows are as long
/// as each other.
fn rectangular(rows: &[Vec<Option<String>>]) -> bool {
    let Some(width) = rows.first().map(Vec::len) else {
        return false;
    };
    if width == 0 || rows.iter().any(|row| row.len() != width) {
        return false;
    }
    let mut seen: Vec<&str> = Vec::new();
    for (top, row) in rows.iter().enumerate() {
        for (left, cell) in row.iter().enumerate() {
            let Some(name) = cell.as_deref() else {
                continue;
            };
            if seen.contains(&name) {
                continue;
            }
            seen.push(name);
            let right = left
                + row[left..]
                    .iter()
                    .take_while(|c| c.as_deref() == Some(name))
                    .count();
            let bottom = top
                + rows[top..]
                    .iter()
                    .take_while(|row| row[left].as_deref() == Some(name))
                    .count();
            // Inside the rectangle every cell is the name, and outside none is.
            for (y, row) in rows.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    let inside = (top..bottom).contains(&y) && (left..right).contains(&x);
                    if inside != (cell.as_deref() == Some(name)) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn string(token: &Token<'_>, limits: ColorLimits, work: &mut usize) -> Option<String> {
    (token.kind == TokenKind::String)
        .then(|| decode_string_budget(token.raw, limits.syntax.retained_bytes, work).ok())
        .flatten()
        .map(|decoded| decoded.text)
}

/// A line placement: `auto`, `<custom-ident>`, `<integer> && <custom-ident>?`, or
/// `span && [ <integer> || <custom-ident> ]`.
fn line(
    list: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<GridLineValue>, PropertyError> {
    if let [(_, only)] = list {
        if is_keyword(only, "auto") {
            return Ok(Some(GridLineValue::default()));
        }
    }
    if list.is_empty() || list.len() > 3 {
        return Ok(None);
    }
    let mut value = GridLineValue::default();
    for (_, token) in list {
        if is_keyword(token, "span") {
            if value.span {
                return Ok(None);
            }
            value.span = true;
        } else if token.kind == TokenKind::Number {
            let Ok(number) = Integer::parse(token.raw, limits.numeric) else {
                return Ok(None);
            };
            if value.number != 0 || number.is_zero() {
                return Ok(None);
            }
            let limit = GRID_LIMIT as i32;
            value.number = number.saturating_i32().clamp(-limit, limit);
        } else if let Some(name) = custom_ident(token, limits, work) {
            if value.name.replace(name).is_some() {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }
    }
    // `span` sits at either end, and needs a number or a name; a span is positive.
    let span_inside = value.span && list.len() == 3 && is_keyword(&list[1].1, "span");
    if span_inside
        || (value.span && (value.number < 0 || (value.number == 0 && value.name.is_none())))
        || (!value.span && value.number == 0 && value.name.is_none())
    {
        return Ok(None);
    }
    Ok(Some(value))
}

fn flow(list: &[(usize, Token<'_>)]) -> Option<GridValue> {
    let (mut axis, mut dense) = (None, false);
    if list.is_empty() || list.len() > 2 {
        return None;
    }
    for (_, token) in list {
        if is_keyword(token, "dense") && !dense {
            dense = true;
        } else if is_keyword(token, "row") && axis.is_none() {
            axis = Some(false);
        } else if is_keyword(token, "column") && axis.is_none() {
            axis = Some(true);
        } else {
            return None;
        }
    }
    Some(GridValue::Flow {
        column: axis.unwrap_or(false),
        dense,
    })
}

/// One grid longhand's value from its tokens; `None` is invalid.
fn longhand(
    name: &str,
    list: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<GridValue>, PropertyError> {
    let none = matches!(list, [(_, only)] if is_keyword(only, "none"));
    Ok(match name {
        "grid-template-columns" | "grid-template-rows" if none => Some(GridValue::None),
        "grid-template-columns" | "grid-template-rows"
            if list
                .first()
                .is_some_and(|(_, first)| is_keyword(first, "subgrid")) =>
        {
            // Only `[ ]` name groups may follow.
            let Some(found) = entries(&list[1..], false, limits, work)? else {
                return Ok(None);
            };
            let mut names = Vec::new();
            for entry in found {
                match entry {
                    GridTrackEntry::Names(group) => names.push(group),
                    _ => return Ok(None),
                }
            }
            Some(GridValue::Subgrid(names))
        }
        "grid-template-columns" | "grid-template-rows" => {
            track_list(list, limits, work)?.map(GridValue::Tracks)
        }
        "grid-auto-columns" | "grid-auto-rows" => {
            let mut sizes = Vec::new();
            let mut at = 0;
            while at < list.len() {
                let Some((size, next)) = track_size(list, at, limits, work)? else {
                    return Ok(None);
                };
                sizes.push(GridTrackEntry::Size(size));
                at = next;
            }
            (!sizes.is_empty()).then_some(GridValue::Tracks(sizes))
        }
        "grid-template-areas" if none => Some(GridValue::None),
        "grid-template-areas" => {
            let mut rows = Vec::new();
            for (_, token) in list {
                let Some(row) = string(token, limits, work).and_then(|text| area_row(&text)) else {
                    return Ok(None);
                };
                rows.push(row);
            }
            rectangular(&rows).then_some(GridValue::Areas(rows))
        }
        "grid-auto-flow" => flow(list),
        "grid-row-start" | "grid-row-end" | "grid-column-start" | "grid-column-end" => {
            line(list, limits, work)?.map(GridValue::Line)
        }
        _ => None,
    })
}

fn inherit(list: &[(usize, Token<'_>)]) -> bool {
    matches!(list, [(_, only)] if is_keyword(only, "inherit"))
}

/// A grid longhand, `None` when its value is invalid.
pub(super) fn parse(
    property: PropertyId,
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
    Ok(longhand(property.descriptor().name, &list, limits, work)?.map(SpecifiedValue::Grid))
}

/// Splits `list` at its top-level `/` delimiters.
fn slashes<'t, 'a>(list: &'t [(usize, Token<'a>)]) -> Vec<&'t [(usize, Token<'a>)]> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, (_, token)) in list.iter().enumerate() {
        match token.kind {
            TokenKind::Function | TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            TokenKind::Delim if token.raw == "/" && depth == 0 => {
                parts.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&list[start..]);
    parts
}

/// The `grid-template` areas form: `[ <line-names>? <string> <track-size>? <line-names>? ]+
/// [ / <explicit-track-list> ]?`, as rows, columns and areas.
fn areas_form(
    parts: &[&[(usize, Token<'_>)]],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<[GridValue; 3]>, PropertyError> {
    let (rows_part, columns) = match parts {
        [rows] => (*rows, None),
        [rows, columns] => (*rows, Some(*columns)),
        _ => return Ok(None),
    };
    let mut rows: Vec<GridTrackEntry> = Vec::new();
    let mut areas = Vec::new();
    let mut at = 0;
    let push_names = |rows: &mut Vec<GridTrackEntry>, names: Vec<String>| {
        if let Some(GridTrackEntry::Names(previous)) = rows.last_mut() {
            previous.extend(names);
        } else {
            rows.push(GridTrackEntry::Names(names));
        }
    };
    while at < rows_part.len() {
        if rows_part[at].1.kind == TokenKind::OpenBracket {
            let Some((names, next)) = line_names(rows_part, at, limits, work) else {
                return Ok(None);
            };
            push_names(&mut rows, names);
            at = next;
        }
        let Some(text) = rows_part
            .get(at)
            .and_then(|(_, token)| string(token, limits, work))
        else {
            return Ok(None);
        };
        let Some(row) = area_row(&text) else {
            return Ok(None);
        };
        areas.push(row);
        at += 1;
        let size = match rows_part.get(at) {
            Some((_, token))
                if !matches!(token.kind, TokenKind::String | TokenKind::OpenBracket) =>
            {
                let Some((size, next)) = track_size(rows_part, at, limits, work)? else {
                    return Ok(None);
                };
                at = next;
                size
            }
            _ => GridTrackSize::Breadth(GridBreadth::Auto),
        };
        rows.push(GridTrackEntry::Size(size));
        if rows_part
            .get(at)
            .is_some_and(|(_, token)| token.kind == TokenKind::OpenBracket)
        {
            let Some((names, next)) = line_names(rows_part, at, limits, work) else {
                return Ok(None);
            };
            push_names(&mut rows, names);
            at = next;
        }
    }
    if !rectangular(&areas) {
        return Ok(None);
    }
    let columns = match columns {
        None => GridValue::None,
        Some(list) => {
            let Some(found) = track_list(list, limits, work)? else {
                return Ok(None);
            };
            // An explicit track list repeats nothing automatically.
            if found
                .iter()
                .any(|entry| matches!(entry, GridTrackEntry::Repeat(..)))
            {
                return Ok(None);
            }
            GridValue::Tracks(found)
        }
    };
    Ok(Some([
        GridValue::Tracks(rows),
        columns,
        GridValue::Areas(areas),
    ]))
}

/// `grid-template`: rows, columns, areas.
fn template(
    list: &[(usize, Token<'_>)],
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<[GridValue; 3]>, PropertyError> {
    if matches!(list, [(_, only)] if is_keyword(only, "none")) {
        return Ok(Some([GridValue::None, GridValue::None, GridValue::None]));
    }
    let parts = slashes(list);
    if list
        .iter()
        .any(|(_, token)| token.kind == TokenKind::String)
    {
        return areas_form(&parts, limits, work);
    }
    let [rows, columns] = parts.as_slice() else {
        return Ok(None);
    };
    let (Some(rows), Some(columns)) = (
        longhand("grid-template-rows", rows, limits, work)?,
        longhand("grid-template-columns", columns, limits, work)?,
    ) else {
        return Ok(None);
    };
    Ok(Some([rows, columns, GridValue::None]))
}

/// The `grid-row`, `grid-column`, `grid-area`, `grid-template` and `grid` shorthands.
pub(super) fn shorthand(
    property: PropertyId,
    input: &str,
    important: bool,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<PropertyResult, PropertyError> {
    let targets = property.longhands();
    let whole = |values: Vec<SpecifiedValue>| {
        PropertyResult::Parsed(ParsedProperty {
            property,
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
    let Some(list) = tokens(input, work)? else {
        return Ok(PropertyResult::Invalid);
    };
    if inherit(&list) {
        return Ok(whole(vec![SpecifiedValue::Inherit; targets.len()]));
    }
    let grid =
        |values: Vec<GridValue>| whole(values.into_iter().map(SpecifiedValue::Grid).collect());
    let invalid = Ok(PropertyResult::Invalid);
    match property.descriptor().name {
        "grid-row" | "grid-column" | "grid-area" => {
            let parts = slashes(&list);
            if parts.len() > targets.len() {
                return invalid;
            }
            let mut lines = Vec::new();
            for part in &parts {
                let Some(value) = line(part, limits, work)? else {
                    return invalid;
                };
                lines.push(value);
            }
            // A part left out copies a lone name from its partner, or is `auto`.
            let partner = |line: &GridLineValue| {
                if !line.span && line.number == 0 && line.name.is_some() {
                    line.clone()
                } else {
                    GridLineValue::default()
                }
            };
            // `grid-area` is row-start, column-start, row-end, column-end: the column
            // start copies the row start, and each end its own start. `grid-row` and
            // `grid-column` are start then end.
            while lines.len() < targets.len() {
                let at = lines.len();
                let from = usize::from(at == 3);
                lines.push(partner(&lines[from]));
            }
            Ok(grid(lines.into_iter().map(GridValue::Line).collect()))
        }
        "grid-template" => Ok(match template(&list, limits, work)? {
            Some(values) => grid(values.into()),
            None => PropertyResult::Invalid,
        }),
        _ => {
            // `grid`: a template, or one axis's tracks and the other's auto-flow.
            let initial_sizes = || {
                GridValue::Tracks(vec![GridTrackEntry::Size(GridTrackSize::Breadth(
                    GridBreadth::Auto,
                ))])
            };
            if let Some([rows, columns, areas]) = template(&list, limits, work)? {
                return Ok(grid(vec![
                    rows,
                    columns,
                    areas,
                    initial_sizes(),
                    initial_sizes(),
                    GridValue::Flow {
                        column: false,
                        dense: false,
                    },
                ]));
            }
            let parts = slashes(&list);
            let [first, second] = parts.as_slice() else {
                return invalid;
            };
            // `auto-flow` and `dense` lead the side that flows.
            let flowing = |part: &[(usize, Token<'_>)]| {
                let mut at = 0;
                let (mut auto_flow, mut dense) = (false, false);
                while let Some((_, token)) = part.get(at) {
                    if is_keyword(token, "auto-flow") && !auto_flow {
                        auto_flow = true;
                    } else if is_keyword(token, "dense") && !dense {
                        dense = true;
                    } else {
                        break;
                    }
                    at += 1;
                }
                auto_flow.then_some((dense, at))
            };
            let auto_sizes = |part: &[(usize, Token<'_>)], work: &mut usize| {
                if part.is_empty() {
                    Ok(Some(initial_sizes()))
                } else {
                    longhand("grid-auto-rows", part, limits, work)
                }
            };
            if let Some((dense, at)) = flowing(first) {
                let (Some(auto_rows), Some(columns)) = (
                    auto_sizes(&first[at..], work)?,
                    longhand("grid-template-columns", second, limits, work)?,
                ) else {
                    return invalid;
                };
                Ok(grid(vec![
                    GridValue::None,
                    columns,
                    GridValue::None,
                    auto_rows,
                    initial_sizes(),
                    GridValue::Flow {
                        column: false,
                        dense,
                    },
                ]))
            } else if let Some((dense, at)) = flowing(second) {
                let (Some(rows), Some(auto_columns)) = (
                    longhand("grid-template-rows", first, limits, work)?,
                    auto_sizes(&second[at..], work)?,
                ) else {
                    return invalid;
                };
                Ok(grid(vec![
                    rows,
                    GridValue::None,
                    GridValue::None,
                    initial_sizes(),
                    auto_columns,
                    GridValue::Flow {
                        column: true,
                        dense,
                    },
                ]))
            } else {
                invalid
            }
        }
    }
}
