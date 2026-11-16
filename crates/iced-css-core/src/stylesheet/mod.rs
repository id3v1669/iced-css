//! Typed decoded stylesheet assembly. Import acquisition/expansion and publication
//! are separate; this module does not replace the legacy macro parser by itself.
use crate::{
    cascade::rule::{RuleDeclaration, RuleOffer, RuleOrigin},
    selectors::{Selector, SelectorLimits, parse_selector_list},
    syntax::{
        DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, RuleError, SyntaxError,
        SyntaxFault, qualified_rule_prefix,
    },
    values::{
        color::ColorLimits,
        property::{PropertyError, PropertyResult, parse_property},
    },
};
use std::{num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct RuleAssemblyLimits {
    pub recovery: RecoveryLimits,
    pub selectors: SelectorLimits,
    pub values: ColorLimits,
    pub entries: NonZeroUsize,
    /// Vector records and rejection names; nested selector/property storage is
    /// separately bounded by their parser limits. This is not total process memory.
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    pub name_bytes: NonZeroUsize,
}
pub use crate::cascade::rule::RuleContext;
#[derive(Debug)]
pub struct DeclarationLocation {
    pub declaration: Range<usize>,
    pub value: Range<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    UnknownProperty,
    InvalidValue,
    InapplicableProperty,
    MalformedDeclaration,
    AtRule,
}
#[derive(Debug)]
pub struct RejectedDeclaration {
    pub span: Range<usize>,
    pub name: Option<String>,
    pub reason: RejectionReason,
}
#[derive(Debug)]
pub struct ParsedStyleRule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<RuleDeclaration>,
    pub locations: Vec<DeclarationLocation>,
    pub rejected: Vec<RejectedDeclaration>,
    pub consumed: usize,
    pub implicit_close: bool,
    pub next_context: Option<RuleContext>,
}
impl ParsedStyleRule {
    /// The caller admits media/import membership before offering this rule.
    pub fn offer(&self, origin: RuleOrigin) -> RuleOffer<'_> {
        RuleOffer {
            selectors: &self.selectors,
            declarations: &self.declarations,
            origin,
        }
    }
}
#[derive(Debug)]
pub enum RuleAssembly {
    Accepted(ParsedStyleRule),
    Discarded {
        consumed: usize,
        diagnostic: SyntaxError,
    },
}
#[derive(Debug)]
pub enum RuleAssemblyError {
    Boundary(RuleError),
    Syntax(SyntaxError),
    Property {
        error: PropertyError,
        span: Range<usize>,
    },
    Entries,
    Bytes,
    Work,
    Allocation,
    Identity,
}
impl std::fmt::Display for RuleAssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "style-rule assembly: {self:?}")
    }
}
impl std::error::Error for RuleAssemblyError {}

/// Assemble one rule prefix with original byte coordinates. Invalid selector
/// lists discard the entire rule; property/resource implementation errors never
/// become recoverable invalid declarations. Caller-owned cursors are not mutated.
pub fn parse_style_rule_prefix(
    input: &str,
    context: RuleContext,
    limits: RuleAssemblyLimits,
) -> Result<RuleAssembly, RuleAssemblyError> {
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(RuleAssemblyError::Bytes);
    }
    let rule =
        qualified_rule_prefix(input, limits.recovery).map_err(RuleAssemblyError::Boundary)?;
    let selectors = match parse_selector_list(rule.prelude, limits.selectors) {
        Ok(selectors) => selectors,
        Err(mut error) => {
            error.offset += rule.prelude_span.start;
            if matches!(error.fault, SyntaxFault::Resource(_)) {
                return Err(RuleAssemblyError::Syntax(error));
            }
            return Ok(RuleAssembly::Discarded {
                consumed: rule.consumed,
                diagnostic: error,
            });
        }
    };
    let parsed = parse_declarations(
        rule.declarations,
        rule.declaration_span.start,
        context,
        limits,
        false,
    )?;
    Ok(RuleAssembly::Accepted(ParsedStyleRule {
        selectors,
        declarations: parsed.declarations,
        locations: parsed.locations,
        rejected: parsed.rejected,
        consumed: rule.consumed,
        implicit_close: rule.implicit_close,
        next_context: parsed.next_context,
    }))
}
struct ParsedDeclarations {
    declarations: Vec<RuleDeclaration>,
    locations: Vec<DeclarationLocation>,
    rejected: Vec<RejectedDeclaration>,
    next_context: Option<RuleContext>,
}
fn parse_declarations(
    input: &str,
    base: usize,
    context: RuleContext,
    limits: RuleAssemblyLimits,
    page: bool,
) -> Result<ParsedDeclarations, RuleAssemblyError> {
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(RuleAssemblyError::Bytes);
    }
    let mut declarations = Vec::new();
    let mut locations = Vec::new();
    let mut rejected = Vec::new();
    let mut records = Records {
        bytes: 0,
        limit: limits.record_bytes.get(),
        work: limits.work.get(),
    };
    let mut count = 0usize;
    for item in Declarations::new(
        input,
        DeclarationLimits {
            recovery: limits.recovery,
            name_bytes: limits.name_bytes,
        },
    ) {
        if count == limits.entries.get() {
            return Err(RuleAssemblyError::Entries);
        }
        records.spend(1)?;
        let ordinal = count;
        count += 1;
        let item = item.map_err(|mut error| {
            error.offset += base;
            RuleAssemblyError::Syntax(error)
        })?;
        let rejection = match item {
            DeclarationItem::Declaration(declaration) => {
                let span = shift(declaration.span.clone(), base);
                let value = shift(declaration.value_span.clone(), base);
                match parse_context_property(&declaration, limits.values, page).map_err(
                    |mut error| {
                        if let PropertyError::Syntax(syntax) = &mut error {
                            syntax.offset += base;
                        }
                        RuleAssemblyError::Property {
                            error,
                            span: value.clone(),
                        }
                    },
                )? {
                    ContextProperty::Parsed(parsed) => {
                        let position =
                            context.offset(ordinal).ok_or(RuleAssemblyError::Identity)?;
                        records.reserve(&mut declarations)?;
                        records.reserve(&mut locations)?;
                        declarations.push(RuleDeclaration {
                            parsed,
                            order: position.first_order,
                            declaration: position.first_declaration,
                        });
                        locations.push(DeclarationLocation {
                            declaration: span,
                            value,
                        });
                        continue;
                    }
                    ContextProperty::Inapplicable => RejectedDeclaration {
                        span,
                        name: Some(declaration.name.text),
                        reason: RejectionReason::InapplicableProperty,
                    },
                    ContextProperty::Unknown => RejectedDeclaration {
                        span,
                        name: Some(declaration.name.text),
                        reason: RejectionReason::UnknownProperty,
                    },
                    ContextProperty::Invalid => RejectedDeclaration {
                        span,
                        name: Some(declaration.name.text),
                        reason: RejectionReason::InvalidValue,
                    },
                }
            }
            DeclarationItem::Malformed { span, .. } => RejectedDeclaration {
                span: shift(span, base),
                name: None,
                reason: RejectionReason::MalformedDeclaration,
            },
            DeclarationItem::AtRule { span, .. } => RejectedDeclaration {
                span: shift(span, base),
                name: None,
                reason: RejectionReason::AtRule,
            },
        };
        let name_bytes = rejection.name.as_ref().map_or(0, String::capacity);
        records.bytes = records
            .bytes
            .checked_add(name_bytes)
            .filter(|n| *n <= records.limit)
            .ok_or(RuleAssemblyError::Bytes)?;
        records.reserve(&mut rejected)?;
        rejected.push(rejection);
    }
    Ok(ParsedDeclarations {
        declarations,
        locations,
        rejected,
        next_context: context.offset(count),
    })
}
fn shift(span: Range<usize>, base: usize) -> Range<usize> {
    base + span.start..base + span.end
}
struct Records {
    bytes: usize,
    limit: usize,
    work: usize,
}
impl Records {
    fn retain(&mut self, amount: usize) -> Result<(), RuleAssemblyError> {
        self.bytes = self
            .bytes
            .checked_add(amount)
            .filter(|bytes| *bytes <= self.limit)
            .ok_or(RuleAssemblyError::Bytes)?;
        Ok(())
    }
    fn spend(&mut self, amount: usize) -> Result<(), RuleAssemblyError> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or(RuleAssemblyError::Work)?;
        Ok(())
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>) -> Result<(), RuleAssemblyError> {
        self.reserve_with_capacity_limit(values, self.limit)
    }
    fn reserve_with_capacity_limit<T>(
        &mut self,
        values: &mut Vec<T>,
        capacity_bytes: usize,
    ) -> Result<(), RuleAssemblyError> {
        let old = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(RuleAssemblyError::Bytes)?;
        if old > capacity_bytes {
            return Err(RuleAssemblyError::Bytes);
        }
        if values.len() < values.capacity() {
            return Ok(());
        }
        let available = ((self.limit - (self.bytes - old)) / size_of::<T>())
            .min(capacity_bytes / size_of::<T>());
        let needed = values
            .len()
            .checked_add(1)
            .filter(|n| *n <= available)
            .ok_or(RuleAssemblyError::Bytes)?;
        let target = needed.max(values.capacity().saturating_mul(2).min(available));
        self.spend(
            values
                .len()
                .checked_mul(size_of::<T>())
                .ok_or(RuleAssemblyError::Work)?,
        )?;
        let mut next = Vec::new();
        next.try_reserve_exact(target)
            .map_err(|_| RuleAssemblyError::Allocation)?;
        if next.capacity() > available {
            return Err(RuleAssemblyError::Bytes);
        }
        self.bytes = self.bytes - old + next.capacity() * size_of::<T>();
        next.append(values);
        *values = next;
        Ok(())
    }
}

mod page;
pub use page::{PageAssembly, PageSelector, ParsedPageRule, parse_page_rule_prefix};

// A separate context result keeps inapplicability distinct from invalid CSS.
#[allow(clippy::large_enum_variant)]
enum ContextProperty {
    Parsed(crate::values::property::ParsedProperty),
    Unknown,
    Invalid,
    Inapplicable,
}
fn parse_context_property(
    declaration: &crate::syntax::Declaration<'_>,
    limits: ColorLimits,
    page: bool,
) -> Result<ContextProperty, PropertyError> {
    use crate::values::{LengthTarget, property::SpecifiedValue, registry::PropertyId};
    if page {
        let Some(property) = PropertyId::lookup(&declaration.name.text) else {
            return Ok(ContextProperty::Unknown);
        };
        if !matches!(
            property.descriptor().name,
            "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left"
        ) {
            return Ok(ContextProperty::Inapplicable);
        }
    }
    Ok(match parse_property(declaration, limits)? {
        PropertyResult::Parsed(parsed) => {
            let relative = |value: &SpecifiedValue| {
                matches!(value, SpecifiedValue::Length(LengthTarget::Relative(..)))
            };
            if page
                && (relative(&parsed.value)
                    || matches!(&parsed.value, SpecifiedValue::Expanded(parts) if parts.iter().any(|part| relative(&part.value))))
            {
                ContextProperty::Invalid
            } else {
                ContextProperty::Parsed(parsed)
            }
        }
        PropertyResult::Unknown => ContextProperty::Unknown,
        PropertyResult::Invalid => ContextProperty::Invalid,
    })
}

mod sheet;
pub use sheet::{
    ParsedKeyframe, ParsedKeyframes, ParsedMediaRule, ParsedStylesheet, SheetEntry, SheetEntryKind,
    SheetRejection, SheetRejectionReason, StylesheetLimits, parse_stylesheet,
};

mod intake;
pub use intake::{
    OriginalSpan, StylesheetIntakeError, StylesheetIntakeLimits, UnicodeStylesheetCandidate,
    parse_separate_unicode_stylesheet,
};

pub mod imports;

pub mod requests;

pub mod located_graph;
pub mod owned_graph;
pub mod publication;

pub mod collector;

pub mod identities;

pub mod uri_aliases;
