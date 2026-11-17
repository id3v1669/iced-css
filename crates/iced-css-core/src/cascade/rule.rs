//! One pre-parsed, media-admitted rule applied atomically to a source/pseudo target.
use super::{
    Origin,
    sheets::{AuthorSheets, SheetError, SheetId},
    winner_table::{PropertyWinners, TableAdmission, TableError},
};
use crate::{
    selectors::{
        MatchError, MatchLimits, PseudoElement, Selector, SelectorSource, matching_specificity,
    },
    values::property::ParsedProperty,
};
use std::num::{NonZeroU64, NonZeroUsize};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleContext {
    pub first_order: u64,
    pub first_declaration: NonZeroU64,
}
impl RuleContext {
    pub(crate) fn offset(self, amount: usize) -> Option<Self> {
        let amount = u64::try_from(amount).ok()?;
        Some(Self {
            first_order: self.first_order.checked_add(amount)?,
            first_declaration: NonZeroU64::new(self.first_declaration.get().checked_add(amount)?)?,
        })
    }
}
#[derive(Debug)]
pub struct RuleDeclaration {
    pub parsed: ParsedProperty,
    pub order: u64,
    pub declaration: NonZeroU64,
}
#[derive(Debug, Clone, Copy)]
pub enum RuleOrigin {
    UserAgent,
    User,
    Author(SheetId),
}
#[derive(Debug, Clone, Copy)]
pub struct RuleOffer<'a> {
    pub selectors: &'a [Selector],
    pub declarations: &'a [RuleDeclaration],
    pub origin: RuleOrigin,
}
pub struct RuleTarget<'a, S: SelectorSource> {
    pub source: &'a S,
    pub node: S::Node,
    pub pseudo: Option<PseudoElement>,
}
#[derive(Debug, Clone, Copy)]
pub struct RuleCascadeLimits {
    pub matching: MatchLimits,
    pub declarations: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCascadeResult {
    Excluded,
    NotMatched,
    Applied {
        declarations: usize,
        slot_replacements: usize,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCascadeError {
    Membership(SheetError),
    Match(MatchError),
    Declarations,
    Table(TableError),
    Count,
    Identity,
}
impl std::fmt::Display for RuleCascadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule cascade: {self:?}")
    }
}
impl std::error::Error for RuleCascadeError {}

/// Imported author rules carry the registered root sheet identity. The caller
/// must already have applied all containing/import-path media conditions. This
/// does not parse CSS, assign orders or publish an entire stylesheet generation.
/// Fixed table copies clone references only; declaration count bounds the number
/// of fixed-registry-size structural offers. Any failure leaves `winners` intact.
pub fn cascade_rule<'a, S: SelectorSource>(
    winners: &mut PropertyWinners<'a>,
    rule: RuleOffer<'a>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleCascadeLimits,
) -> Result<RuleCascadeResult, RuleCascadeError> {
    cascade_rule_inner(winners, rule, target, sheets, limits, None)
}
fn cascade_rule_inner<'a, S: SelectorSource>(
    winners: &mut PropertyWinners<'a>,
    rule: RuleOffer<'a>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleCascadeLimits,
    position: Option<RuleContext>,
) -> Result<RuleCascadeResult, RuleCascadeError> {
    let origin = match rule.origin {
        RuleOrigin::UserAgent => Origin::UserAgent,
        RuleOrigin::User => Origin::User,
        RuleOrigin::Author(sheet) => {
            if !sheets
                .applies(sheet)
                .map_err(RuleCascadeError::Membership)?
            {
                return Ok(RuleCascadeResult::Excluded);
            }
            Origin::Author
        }
    };
    let Some(specificity) = matching_specificity(
        rule.selectors,
        target.source,
        target.node,
        target.pseudo,
        limits.matching,
    )
    .map_err(RuleCascadeError::Match)?
    else {
        return Ok(RuleCascadeResult::NotMatched);
    };
    if rule.declarations.len() > limits.declarations.get() {
        return Err(RuleCascadeError::Declarations);
    }
    let mut next = winners.clone();
    let mut slot_replacements = 0usize;
    for (ordinal, declaration) in rule.declarations.iter().enumerate() {
        let context = match position {
            Some(position) => position.offset(ordinal).ok_or(RuleCascadeError::Identity)?,
            None => RuleContext {
                first_order: declaration.order,
                first_declaration: declaration.declaration,
            },
        };
        let replaced = next
            .offer(
                &declaration.parsed,
                TableAdmission {
                    origin,
                    specificity,
                    order: context.first_order,
                    declaration: context.first_declaration,
                },
            )
            .map_err(RuleCascadeError::Table)?;
        slot_replacements = slot_replacements
            .checked_add(replaced)
            .ok_or(RuleCascadeError::Count)?;
    }
    *winners = next;
    Ok(RuleCascadeResult::Applied {
        declarations: rule.declarations.len(),
        slot_replacements,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct RuleBatchLimits {
    pub rules: NonZeroUsize,
    pub per_rule: RuleCascadeLimits,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RuleBatchResult {
    pub excluded: usize,
    pub not_matched: usize,
    pub applied: usize,
    pub declarations: usize,
    pub slot_replacements: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleBatchError {
    Rules,
    Rule(RuleCascadeError),
    Count,
}
impl std::fmt::Display for RuleBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule batch: {self:?}")
    }
}
impl std::error::Error for RuleBatchError {}

/// Apply a media-admitted sequence atomically to one source/pseudo target. Any
/// failure preserves the caller's table, including changes offered by earlier
/// rules in this sequence. Fixed table clones borrow the same parsed leaves.
///
/// The caller establishes import occurrence ordering and media admission. Rule
/// count bounds repeated per-rule work; evaluating the caller's iterator itself
/// is outside these limits. This is not host-wide multi-target publication.
pub fn cascade_rules<'a, S: SelectorSource>(
    winners: &mut PropertyWinners<'a>,
    rules: impl IntoIterator<Item = RuleOffer<'a>>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleBatchLimits,
) -> Result<RuleBatchResult, RuleBatchError> {
    cascade_batch_inner(
        winners,
        rules.into_iter().map(|rule| (rule, None)),
        target,
        sheets,
        limits,
    )
}
/// A shared parsed rule with independently assigned occurrence positions.
#[derive(Debug, Clone, Copy)]
pub struct PositionedRuleOffer<'a> {
    pub rule: RuleOffer<'a>,
    pub context: RuleContext,
}
/// Apply expanded occurrences without copying or mutating their shared parsed
/// leaves. Each declaration uses its accepted-declaration ordinal from context;
/// the caller must assign nonoverlapping occurrence ranges before matching.
pub fn cascade_positioned_rules<'a, S: SelectorSource>(
    winners: &mut PropertyWinners<'a>,
    rules: impl IntoIterator<Item = PositionedRuleOffer<'a>>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleBatchLimits,
) -> Result<RuleBatchResult, RuleBatchError> {
    cascade_batch_inner(
        winners,
        rules
            .into_iter()
            .map(|offer| (offer.rule, Some(offer.context))),
        target,
        sheets,
        limits,
    )
}
fn cascade_batch_inner<'a, S: SelectorSource>(
    winners: &mut PropertyWinners<'a>,
    rules: impl IntoIterator<Item = (RuleOffer<'a>, Option<RuleContext>)>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleBatchLimits,
) -> Result<RuleBatchResult, RuleBatchError> {
    let mut next = winners.clone();
    let mut result = RuleBatchResult::default();
    let mut remaining = limits.rules.get();
    for (rule, context) in rules {
        remaining = remaining.checked_sub(1).ok_or(RuleBatchError::Rules)?;
        match cascade_rule_inner(
            &mut next,
            rule,
            RuleTarget {
                source: target.source,
                node: target.node,
                pseudo: target.pseudo,
            },
            sheets,
            limits.per_rule,
            context,
        )
        .map_err(RuleBatchError::Rule)?
        {
            RuleCascadeResult::Excluded => add(&mut result.excluded, 1)?,
            RuleCascadeResult::NotMatched => add(&mut result.not_matched, 1)?,
            RuleCascadeResult::Applied {
                declarations,
                slot_replacements,
            } => {
                add(&mut result.applied, 1)?;
                add(&mut result.declarations, declarations)?;
                add(&mut result.slot_replacements, slot_replacements)?;
            }
        }
    }
    *winners = next;
    Ok(result)
}
fn add(value: &mut usize, amount: usize) -> Result<(), RuleBatchError> {
    *value = value.checked_add(amount).ok_or(RuleBatchError::Count)?;
    Ok(())
}

/// Call `visit` with every declaration of every rule that applies to `target`, with the
/// cascade position it has there: what [`cascade_positioned_rules`] offers to a winner
/// table, for a caller that cascades by something other than property identity (custom
/// properties, by name).
///
/// # Errors
/// As [`cascade_positioned_rules`]: membership, matching and count limits.
pub fn matched_declarations<'a, S: SelectorSource>(
    rules: impl IntoIterator<Item = PositionedRuleOffer<'a>>,
    target: RuleTarget<'_, S>,
    sheets: &AuthorSheets,
    limits: RuleBatchLimits,
    visit: &mut dyn FnMut(&'a ParsedProperty, super::CascadeKey),
) -> Result<(), RuleBatchError> {
    let mut remaining = limits.rules.get();
    for offer in rules {
        remaining = remaining.checked_sub(1).ok_or(RuleBatchError::Rules)?;
        let rule = offer.rule;
        let origin = match rule.origin {
            RuleOrigin::UserAgent => Origin::UserAgent,
            RuleOrigin::User => Origin::User,
            RuleOrigin::Author(sheet) => {
                if !sheets
                    .applies(sheet)
                    .map_err(|error| RuleBatchError::Rule(RuleCascadeError::Membership(error)))?
                {
                    continue;
                }
                Origin::Author
            }
        };
        let Some(specificity) = matching_specificity(
            rule.selectors,
            target.source,
            target.node,
            target.pseudo,
            limits.per_rule.matching,
        )
        .map_err(|error| RuleBatchError::Rule(RuleCascadeError::Match(error)))?
        else {
            continue;
        };
        for (ordinal, declaration) in rule.declarations.iter().enumerate() {
            let position = offer
                .context
                .offset(ordinal)
                .ok_or(RuleBatchError::Rule(RuleCascadeError::Identity))?;
            visit(
                &declaration.parsed,
                super::CascadeKey {
                    level: origin.level(declaration.parsed.important),
                    specificity: specificity.for_importance(declaration.parsed.important),
                    order: position.first_order,
                },
            );
        }
    }
    Ok(())
}
