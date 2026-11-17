use super::{
    DeclarationLocation, PageAssembly, ParsedPageRule, ParsedStyleRule, Records,
    RejectedDeclaration, RuleAssembly, RuleAssemblyError, RuleAssemblyLimits, RuleContext,
    parse_page_rule_prefix, parse_style_rule_prefix, shift,
};
use crate::syntax::{
    DeclarationItem, DeclarationLimits, Declarations, ImportError, ImportPrelude, MediaError,
    MediaListContext, RuleError, Statement, StatementContext, StatementKind, Statements,
    SyntaxError, SyntaxFault, decode_identifier_budget,
    import_placement::{ImportPlacement, PlacementDecision, StatementDisposition},
    media_matches_screen, parse_import_prelude,
};
use crate::values::property::{PropertyResult, parse_property};
use std::{num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct StylesheetLimits {
    pub rules: RuleAssemblyLimits,
    /// Includes top-level statements and direct statements inside media groups.
    pub statements: NonZeroUsize,
    /// Sheet entry/rejection/media-rule vector capacities; rule-owned storage is
    /// separately bounded by rules and repetition by statements.
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub struct ParsedStylesheet {
    pub entries: Vec<SheetEntry>,
    pub rejected: Vec<SheetRejection>,
    pub next_context: Option<RuleContext>,
}
impl ParsedStylesheet {
    /// Borrow applicable rules authored in this sheet in source order. Imported
    /// sheets are not expanded here; page context never participates in screen
    /// source-element styling. No allocation or matching is performed.
    pub fn local_screen_rules(&self) -> impl Iterator<Item = &ParsedStyleRule> {
        self.entries.iter().flat_map(|entry| {
            let rules: &[ParsedStyleRule] = match &entry.kind {
                SheetEntryKind::Style(rule) => std::slice::from_ref(rule),
                SheetEntryKind::Media(media) if media.applies_to_screen => &media.rules,
                _ => &[],
            };
            rules.iter()
        })
    }
}
#[derive(Debug)]
pub struct SheetEntry {
    pub span: Range<usize>,
    pub kind: SheetEntryKind,
}
#[derive(Debug)]
pub enum SheetEntryKind {
    Style(ParsedStyleRule),
    Page(ParsedPageRule),
    Import(ImportPrelude),
    Media(ParsedMediaRule),
    /// `@keyframes` (CSS Animations 1).
    Keyframes(ParsedKeyframes),
}
/// A `@keyframes` rule: its name and its keyframes in source order.
#[derive(Debug)]
pub struct ParsedKeyframes {
    pub name: String,
    pub frames: Vec<ParsedKeyframe>,
}
/// One keyframe rule: where it sits, from 0 to 1 (one or more places), and the
/// declarations it gives. `!important` ones and the animation and transition properties
/// were dropped, as section 3 says, except `animation-timing-function`.
#[derive(Debug)]
pub struct ParsedKeyframe {
    pub offsets: Vec<f32>,
    pub declarations: Vec<crate::values::property::ParsedProperty>,
}
#[derive(Debug)]
pub struct ParsedMediaRule {
    pub applies_to_screen: bool,
    pub rules: Vec<ParsedStyleRule>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetRejectionReason {
    InvalidRule,
    UnknownAtRule,
    /// Byte-intake owns charset detection; this decoded-text parser ignores it.
    Charset,
    InvalidImport,
    LateImport,
    InvalidMedia,
    NestedAtRule,
}
#[derive(Debug)]
pub struct SheetRejection {
    pub span: Range<usize>,
    pub reason: SheetRejectionReason,
}

/// Assemble one already-decoded sheet atomically, without fetching imports or
/// publishing styles. All spans and consumed offsets address this input. Cursors
/// are local to this sheet occurrence; an import owner must establish final
/// occurrence order before offering declarations to the cascade.
pub fn parse_stylesheet(
    input: &str,
    context: RuleContext,
    limits: StylesheetLimits,
) -> Result<ParsedStylesheet, RuleAssemblyError> {
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(RuleAssemblyError::Bytes);
    }
    let mut owner = Owner {
        layers: Vec::new(),
        limits,
        context: Some(context),
        records: Records {
            bytes: 0,
            limit: limits.record_bytes.get(),
            work: limits.work.get(),
        },
        remaining: limits.statements.get(),
        rejected: Vec::new(),
    };
    let mut entries = Vec::new();
    let mut placement = ImportPlacement::new(StatementContext::Stylesheet);
    for statement in Statements::new(input, StatementContext::Stylesheet, limits.rules.recovery) {
        owner.tick()?;
        let statement = statement.map_err(RuleAssemblyError::Syntax)?;
        let kind = match statement.kind {
            StatementKind::QualifiedRule => owner.style(&statement, 0)?.map(SheetEntryKind::Style),
            StatementKind::AtRule { keyword } => {
                let mut work = limits.rules.values.syntax.work.get();
                let name = match decode_identifier_budget(
                    &keyword[1..],
                    limits.rules.name_bytes,
                    &mut work,
                ) {
                    Ok(name) => name,
                    Err(error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                        return Err(RuleAssemblyError::Syntax(rebase_syntax(
                            error,
                            statement.span.start + 1,
                        )));
                    }
                    Err(_) => {
                        owner.reject(statement.span, SheetRejectionReason::UnknownAtRule)?;
                        continue;
                    }
                };
                if name.text.eq_ignore_ascii_case("import") {
                    owner
                        .import(&statement, &mut placement)?
                        .map(SheetEntryKind::Import)
                } else if name.text.eq_ignore_ascii_case("media") {
                    owner.media(&statement)?.map(SheetEntryKind::Media)
                } else if name.text.eq_ignore_ascii_case("supports") {
                    // CSS Conditional 3: a group of rules, as `@media` is, that applies
                    // when its condition holds.
                    owner.supports(&statement)?.map(SheetEntryKind::Media)
                } else if name.text.eq_ignore_ascii_case("page") {
                    owner.page(&statement)?.map(SheetEntryKind::Page)
                } else if name.text.eq_ignore_ascii_case("keyframes") {
                    owner.keyframes(&statement)?.map(SheetEntryKind::Keyframes)
                } else if name.text.eq_ignore_ascii_case("layer") {
                    owner.layer(&statement)?.map(SheetEntryKind::Media)
                } else {
                    owner.reject(
                        statement.span,
                        if name.text.eq_ignore_ascii_case("charset") {
                            SheetRejectionReason::Charset
                        } else {
                            SheetRejectionReason::UnknownAtRule
                        },
                    )?;
                    continue;
                }
            }
        };
        if let Some(kind) = kind {
            if !matches!(kind, SheetEntryKind::Import(_)) {
                placement.observe(StatementDisposition::Other);
            }
            owner.records.reserve(&mut entries)?;
            entries.push(SheetEntry {
                span: statement.span,
                kind,
            });
        }
    }
    Ok(ParsedStylesheet {
        entries,
        rejected: owner.rejected,
        next_context: owner.context,
    })
}
struct Owner {
    /// CSS Cascade 5: the layers named so far, in the order they were first named.
    layers: Vec<String>,
    limits: StylesheetLimits,
    context: Option<RuleContext>,
    records: Records,
    remaining: usize,
    rejected: Vec<SheetRejection>,
}
impl Owner {
    fn tick(&mut self) -> Result<(), RuleAssemblyError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(RuleAssemblyError::Entries)?;
        self.records.spend(1)
    }
    fn reject(
        &mut self,
        span: Range<usize>,
        reason: SheetRejectionReason,
    ) -> Result<(), RuleAssemblyError> {
        self.records.reserve(&mut self.rejected)?;
        self.rejected.push(SheetRejection { span, reason });
        Ok(())
    }
    fn style(
        &mut self,
        statement: &Statement<'_>,
        base: usize,
    ) -> Result<Option<ParsedStyleRule>, RuleAssemblyError> {
        let start = base + statement.span.start;
        let parsed = parse_style_rule_prefix(
            statement.raw,
            self.context.ok_or(RuleAssemblyError::Identity)?,
            self.limits.rules,
        );
        match parsed {
            Ok(RuleAssembly::Accepted(mut rule)) => {
                rebase_locations(&mut rule.locations, &mut rule.rejected, start);
                rule.consumed += start;
                self.context = rule.next_context;
                Ok(Some(rule))
            }
            Ok(RuleAssembly::Discarded { .. })
            | Err(RuleAssemblyError::Boundary(RuleError::Invalid)) => {
                self.reject(
                    shift(statement.span.clone(), base),
                    SheetRejectionReason::InvalidRule,
                )?;
                Ok(None)
            }
            Err(error) => Err(rebase_error(error, start)),
        }
    }
    fn page(
        &mut self,
        statement: &Statement<'_>,
    ) -> Result<Option<ParsedPageRule>, RuleAssemblyError> {
        let start = statement.span.start;
        match parse_page_rule_prefix(
            statement.raw,
            self.context.ok_or(RuleAssemblyError::Identity)?,
            self.limits.rules,
        )
        .map_err(|error| rebase_error(error, start))?
        {
            PageAssembly::Accepted(mut rule) => {
                rebase_locations(&mut rule.locations, &mut rule.rejected, start);
                rule.consumed += start;
                self.context = rule.next_context;
                Ok(Some(rule))
            }
            PageAssembly::Discarded { .. } => {
                self.reject(statement.span.clone(), SheetRejectionReason::InvalidRule)?;
                Ok(None)
            }
        }
    }
    fn import(
        &mut self,
        statement: &Statement<'_>,
        placement: &mut ImportPlacement,
    ) -> Result<Option<ImportPrelude>, RuleAssemblyError> {
        if statement.body.is_some() {
            self.reject(statement.span.clone(), SheetRejectionReason::InvalidImport)?;
            return Ok(None);
        }
        let mut import =
            match parse_import_prelude(statement.prelude, self.limits.rules.values.syntax) {
                Ok(import) => import,
                Err(error) => {
                    let syntax = match error {
                        ImportError::Syntax(error)
                        | ImportError::Media(MediaError::Syntax(error)) => Some(error),
                        _ => None,
                    };
                    if let Some(error) =
                        syntax.filter(|error| matches!(error.fault, SyntaxFault::Resource(_)))
                    {
                        return Err(RuleAssemblyError::Syntax(rebase_syntax(
                            error,
                            statement.prelude_span.start,
                        )));
                    }
                    self.reject(statement.span.clone(), SheetRejectionReason::InvalidImport)?;
                    return Ok(None);
                }
            };
        if placement.observe(StatementDisposition::Import) != PlacementDecision::Allowed {
            self.reject(statement.span.clone(), SheetRejectionReason::LateImport)?;
            return Ok(None);
        }
        import.location_span = shift(import.location_span, statement.prelude_span.start);
        Ok(Some(import))
    }
    fn media(
        &mut self,
        statement: &Statement<'_>,
    ) -> Result<Option<ParsedMediaRule>, RuleAssemblyError> {
        let Some(body) = statement.body else {
            self.reject(statement.span.clone(), SheetRejectionReason::InvalidMedia)?;
            return Ok(None);
        };
        let matches = match media_matches_screen(
            statement.prelude,
            MediaListContext::MediaRule,
            self.limits.rules.values.syntax,
        ) {
            Ok(matches) => matches,
            Err(MediaError::Syntax(error)) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                return Err(RuleAssemblyError::Syntax(rebase_syntax(
                    error,
                    statement.prelude_span.start,
                )));
            }
            Err(_) => {
                self.reject(statement.span.clone(), SheetRejectionReason::InvalidMedia)?;
                return Ok(None);
            }
        };
        self.group(statement, body, matches).map(Some)
    }
    fn supports(
        &mut self,
        statement: &Statement<'_>,
    ) -> Result<Option<ParsedMediaRule>, RuleAssemblyError> {
        let Some(body) = statement.body else {
            self.reject(statement.span.clone(), SheetRejectionReason::InvalidMedia)?;
            return Ok(None);
        };
        let mut work = self.limits.rules.values.syntax.work.get();
        let Some(matches) = supports_condition(statement.prelude, self.limits.rules, &mut work, 0)
        else {
            self.reject(statement.span.clone(), SheetRejectionReason::InvalidMedia)?;
            return Ok(None);
        };
        self.group(statement, body, matches).map(Some)
    }
    /// CSS Animations 1, section 3: a name, an identifier or a string, and keyframe rules
    /// whose selectors are `from`, `to` or percentages. An invalid keyframe rule is left
    /// out; an invalid name drops the whole rule.
    fn keyframes(
        &mut self,
        statement: &Statement<'_>,
    ) -> Result<Option<ParsedKeyframes>, RuleAssemblyError> {
        let limits = self.limits.rules;
        let reject = |owner: &mut Self| {
            owner.reject(statement.span.clone(), SheetRejectionReason::InvalidRule)
        };
        let (Some(body), Some(name)) = (statement.body, keyframes_name(statement.prelude, limits))
        else {
            reject(self)?;
            return Ok(None);
        };
        let mut frames = Vec::new();
        for child in Statements::new(body, StatementContext::Block, limits.recovery) {
            self.tick()?;
            let Ok(child) = child else {
                continue;
            };
            let (StatementKind::QualifiedRule, Some(block)) = (child.kind, child.body) else {
                continue;
            };
            let Some(offsets) = keyframe_offsets(child.prelude) else {
                continue;
            };
            let mut declarations = Vec::new();
            for item in Declarations::new(
                block,
                DeclarationLimits {
                    recovery: limits.recovery,
                    name_bytes: limits.name_bytes,
                },
            ) {
                self.records.spend(1)?;
                let Ok(DeclarationItem::Declaration(declaration)) = item else {
                    continue;
                };
                if let Ok(PropertyResult::Parsed(parsed)) =
                    parse_property(&declaration, limits.values)
                {
                    let name = parsed.property.descriptor().name;
                    // `animation-timing-function` stays: it eases from this keyframe on.
                    if !parsed.important
                        && (name == "animation-timing-function"
                            || !(name.starts_with("animation") || name.starts_with("transition")))
                    {
                        declarations.push(parsed);
                    }
                }
            }
            frames.push(ParsedKeyframe {
                offsets,
                declarations,
            });
        }
        Ok(Some(ParsedKeyframes { name, frames }))
    }
    /// The rank of layer `name`, naming it now if it is new.
    fn layer_rank(&mut self, name: &str) -> u32 {
        let position = match self.layers.iter().position(|known| known == name) {
            Some(position) => position,
            None => {
                self.layers.push(name.to_owned());
                self.layers.len() - 1
            }
        };
        u32::try_from(position).unwrap_or(u32::MAX)
    }
    /// CSS Cascade 5 `@layer`: a statement names layers in order; a block's rules are in
    /// its layer (a block without a name in a layer of its own). Layers are ranked as
    /// they are first named in this sheet.
    fn layer(
        &mut self,
        statement: &Statement<'_>,
    ) -> Result<Option<ParsedMediaRule>, RuleAssemblyError> {
        let names: Vec<String> = statement
            .prelude
            .split(',')
            .map(|name| name.trim().to_owned())
            .collect();
        // Each dotted part a custom identifier: not starting with a digit.
        let valid = |name: &String| {
            !name.is_empty()
                && name.split('.').all(|part| {
                    part.chars()
                        .next()
                        .is_some_and(|first| !first.is_ascii_digit())
                        && part
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                })
        };
        let Some(body) = statement.body else {
            if names.iter().all(valid) {
                for name in &names {
                    self.layer_rank(name);
                }
            } else {
                self.reject(statement.span.clone(), SheetRejectionReason::InvalidRule)?;
            }
            return Ok(None);
        };
        let name = match names.as_slice() {
            [only] if only.is_empty() => format!("\u{0}{}", self.layers.len()),
            [only] if valid(only) => only.clone(),
            _ => {
                self.reject(statement.span.clone(), SheetRejectionReason::InvalidRule)?;
                return Ok(None);
            }
        };
        let rank = self.layer_rank(&name);
        let mut group = self.group(statement, body, true)?;
        for rule in &mut group.rules {
            for selector in &mut rule.selectors {
                selector.specificity = selector.specificity.in_layer(rank);
            }
        }
        Ok(Some(group))
    }
    /// The style rules of a conditional group: parsed whether or not it applies.
    fn group(
        &mut self,
        statement: &Statement<'_>,
        body: &str,
        matches: bool,
    ) -> Result<ParsedMediaRule, RuleAssemblyError> {
        let base = statement.body_span.as_ref().expect("body span").start;
        let mut rules = Vec::new();
        for child in Statements::new(body, StatementContext::Block, self.limits.rules.recovery) {
            self.tick()?;
            let child =
                child.map_err(|error| RuleAssemblyError::Syntax(rebase_syntax(error, base)))?;
            if matches!(child.kind, StatementKind::AtRule { .. }) {
                self.reject(shift(child.span, base), SheetRejectionReason::NestedAtRule)?;
            } else if let Some(rule) = self.style(&child, base)? {
                self.records.reserve(&mut rules)?;
                rules.push(rule);
            }
        }
        Ok(ParsedMediaRule {
            applies_to_screen: matches,
            rules,
        })
    }
}

/// One operand of a `@supports` condition at the top level of its text.
enum SupportsPart<'a> {
    /// The text inside one pair of parentheses.
    Group(&'a str),
    /// A function: something this does not know, and so does not support.
    Unknown,
    Word(String),
}

/// Whether a `@supports` condition holds (CSS Conditional 3, section 6): a declaration in
/// parentheses is supported when this parser accepts it; `not`, `and` and `or` combine
/// them, without mixing `and` with `or` at one level. `None` is a malformed condition.
fn supports_condition(
    text: &str,
    limits: RuleAssemblyLimits,
    work: &mut usize,
    depth: usize,
) -> Option<bool> {
    use crate::syntax::{TokenKind, scan_token_budget};
    if depth > 32 {
        return None;
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let token = scan_token_budget(&text[offset..], work).ok()??;
        let start = offset;
        offset += token.raw.len();
        match token.kind {
            TokenKind::Space | TokenKind::Comment => {}
            TokenKind::Ident => parts.push(SupportsPart::Word(token.raw.to_ascii_lowercase())),
            TokenKind::OpenParen | TokenKind::Function => {
                // Through the matching closing parenthesis.
                let mut nesting = 1usize;
                let inner = offset;
                while nesting > 0 {
                    let token = scan_token_budget(text.get(offset..)?, work).ok()??;
                    offset += token.raw.len();
                    match token.kind {
                        TokenKind::OpenParen | TokenKind::Function => nesting += 1,
                        TokenKind::CloseParen => nesting -= 1,
                        _ => {}
                    }
                }
                parts.push(if text[start..].starts_with('(') {
                    SupportsPart::Group(&text[inner..offset - 1])
                } else {
                    SupportsPart::Unknown
                });
            }
            _ => return None,
        }
    }
    let operand = |part: &SupportsPart<'_>, work: &mut usize| match part {
        SupportsPart::Unknown => Some(false),
        SupportsPart::Word(_) => None,
        SupportsPart::Group(inner) => {
            let trimmed = inner.trim_start();
            // A nested condition starts with a parenthesis or `not`; anything else is a
            // declaration.
            if trimmed.starts_with('(') || trimmed.to_ascii_lowercase().starts_with("not ") {
                return supports_condition(inner, limits, work, depth + 1);
            }
            let declaration = Declarations::new(
                inner,
                DeclarationLimits {
                    recovery: limits.recovery,
                    name_bytes: limits.name_bytes,
                },
            )
            .next()?
            .ok()?;
            let DeclarationItem::Declaration(declaration) = declaration else {
                return None;
            };
            Some(matches!(
                parse_property(&declaration, limits.values),
                Ok(PropertyResult::Parsed(_))
            ))
        }
    };
    match parts.as_slice() {
        [SupportsPart::Word(not), part] if not == "not" => operand(part, work).map(|held| !held),
        [first, rest @ ..] if rest.len() % 2 == 0 => {
            let mut held = operand(first, work)?;
            let mut joiner: Option<&str> = None;
            for pair in rest.chunks(2) {
                let SupportsPart::Word(word) = &pair[0] else {
                    return None;
                };
                if !matches!(word.as_str(), "and" | "or") || joiner.is_some_and(|j| j != word) {
                    return None;
                }
                joiner = Some(word);
                let next = operand(&pair[1], work)?;
                held = if word == "and" {
                    held && next
                } else {
                    held || next
                };
            }
            Some(held)
        }
        _ => None,
    }
}
fn rebase_syntax(mut error: SyntaxError, base: usize) -> SyntaxError {
    error.offset += base;
    error
}
fn rebase_error(error: RuleAssemblyError, base: usize) -> RuleAssemblyError {
    match error {
        RuleAssemblyError::Syntax(error) => RuleAssemblyError::Syntax(rebase_syntax(error, base)),
        RuleAssemblyError::Boundary(RuleError::Syntax(error)) => {
            RuleAssemblyError::Boundary(RuleError::Syntax(rebase_syntax(error, base)))
        }
        RuleAssemblyError::Property { mut error, span } => {
            if let crate::values::property::PropertyError::Syntax(syntax) = &mut error {
                syntax.offset += base;
            }
            RuleAssemblyError::Property {
                error,
                span: shift(span, base),
            }
        }
        other => other,
    }
}
fn rebase_locations(
    locations: &mut [DeclarationLocation],
    rejected: &mut [RejectedDeclaration],
    base: usize,
) {
    for location in locations {
        location.declaration = shift(location.declaration.clone(), base);
        location.value = shift(location.value.clone(), base);
    }
    for rejection in rejected {
        rejection.span = shift(rejection.span.clone(), base);
    }
}

/// A `@keyframes` name: a custom identifier other than `none` and the CSS-wide keywords,
/// or a string.
fn keyframes_name(prelude: &str, limits: super::RuleAssemblyLimits) -> Option<String> {
    use crate::syntax::{TokenKind, decode_string_budget, scan_token_budget};
    let mut work = limits.values.syntax.work.get();
    let text = prelude.trim();
    let token = scan_token_budget(text, &mut work).ok()??;
    if token.raw.len() != text.len() || token.invalid_escape {
        return None;
    }
    match token.kind {
        TokenKind::Ident => {
            let name = decode_identifier_budget(token.raw, limits.name_bytes, &mut work)
                .ok()?
                .text;
            let reserved = ["none", "initial", "inherit", "unset", "default"];
            (!reserved.iter().any(|word| name.eq_ignore_ascii_case(word))).then_some(name)
        }
        TokenKind::String => decode_string_budget(token.raw, limits.name_bytes, &mut work)
            .ok()
            .map(|decoded| decoded.text),
        _ => None,
    }
}

/// A keyframe selector list: `from`, `to` and percentages from 0% to 100%.
fn keyframe_offsets(prelude: &str) -> Option<Vec<f32>> {
    prelude
        .split(',')
        .map(|part| {
            let part = part.trim();
            if part.eq_ignore_ascii_case("from") {
                Some(0.0)
            } else if part.eq_ignore_ascii_case("to") {
                Some(1.0)
            } else {
                let number: f32 = part.strip_suffix('%')?.parse().ok()?;
                (number.is_finite() && (0.0..=100.0).contains(&number)).then_some(number / 100.0)
            }
        })
        .collect()
}
