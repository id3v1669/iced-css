use super::{Boundary, RecoveryLimits, RecoveryMode, SyntaxError, TokenKind};
use super::{recover::recover_prefix_budget, token::scan_token_budget};
use std::ops::Range;
#[derive(Debug, PartialEq, Eq)]
pub struct QualifiedRule<'a> {
    pub prelude: &'a str,
    pub declarations: &'a str,
    pub prelude_span: Range<usize>,
    pub declaration_span: Range<usize>,
    pub consumed: usize,
    pub implicit_close: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RuleError {
    Invalid,
    Syntax(SyntaxError),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid => f.write_str("expected a qualified rule block"),
            Self::Syntax(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RuleError {}
/// Split one qualified rule into borrowed prelude and declaration-list input.
/// This finds syntax boundaries; it does not validate the selector grammar.
pub fn qualified_rule_prefix(
    input: &str,
    limits: RecoveryLimits,
) -> Result<QualifiedRule<'_>, RuleError> {
    let mut remaining = limits.work.get();
    let mut start = 0;
    loop {
        let Some(token) =
            scan_token_budget(&input[start..], &mut remaining).map_err(|mut error| {
                error.offset += start;
                RuleError::Syntax(error)
            })?
        else {
            return Err(RuleError::Invalid);
        };
        if matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            start += token.raw.len();
            continue;
        }
        if matches!(token.kind, TokenKind::AtKeyword | TokenKind::OpenBrace) {
            return Err(RuleError::Invalid);
        }
        break;
    }
    let recovered = recover_prefix_budget(
        &input[start..],
        RecoveryMode::QualifiedRule,
        limits,
        &mut remaining,
    )
    .map_err(|mut error| {
        error.offset += start;
        RuleError::Syntax(error)
    })?;
    let block = start + recovered.block_start.ok_or(RuleError::Invalid)?;
    if !matches!(
        recovered.boundary,
        Boundary::AtRuleBlock | Boundary::EndOfInput
    ) {
        return Err(RuleError::Invalid);
    }
    let mut cursor = start;
    let mut prelude_end = start;
    while cursor < block {
        let token = scan_token_budget(&input[cursor..block], &mut remaining)
            .map_err(|mut error| {
                error.offset += cursor;
                RuleError::Syntax(error)
            })?
            .expect("nonempty prelude");
        cursor += token.raw.len();
        if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            prelude_end = cursor;
        }
    }
    if prelude_end == start {
        return Err(RuleError::Invalid);
    }
    let consumed = start + recovered.consumed;
    let body_start = block + 1;
    let body_end = consumed - usize::from(recovered.boundary == Boundary::AtRuleBlock);
    Ok(QualifiedRule {
        prelude: &input[start..prelude_end],
        declarations: &input[body_start..body_end],
        prelude_span: start..prelude_end,
        declaration_span: body_start..body_end,
        consumed,
        implicit_close: recovered.implicit_close,
    })
}
