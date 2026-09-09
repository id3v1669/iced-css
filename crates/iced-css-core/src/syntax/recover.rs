use super::token::scan_token_budget;
use super::{SyntaxError, SyntaxFault, SyntaxResource, TokenKind};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct RecoveryLimits {
    pub work: NonZeroUsize,
    pub nesting: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryMode {
    Declaration,
    AtRule,
    /// A selector owns everything through its following block. Unexpected
    /// semicolons/closing braces do not start a fresh selector before that block.
    QualifiedRule,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    Semicolon,
    RuleClose,
    AtRuleBlock,
    EndOfInput,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Recovered<'a> {
    pub block_start: Option<usize>,
    pub raw: &'a str,
    pub consumed: usize,
    pub boundary: Boundary,
    pub invalid_tokens: bool,
    pub recovery_only: bool,
    pub implicit_close: bool,
    pub unclosed_delimiters: usize,
}

fn resource(offset: usize, kind: SyntaxResource) -> SyntaxError {
    SyntaxError {
        offset,
        fault: SyntaxFault::Resource(kind),
    }
}
/// Find a declaration, at-rule or qualified-rule recovery boundary without
/// accepting its grammar. Declaration/at-rule modes leave a containing rule's
/// closing brace unconsumed; qualified mode consumes unexpected closing tokens
/// in the selector before its following block.
/// Missing delimiters are recorded at EOF; no source text is fabricated.
pub fn recover_prefix(
    input: &str,
    mode: RecoveryMode,
    limits: RecoveryLimits,
) -> Result<Recovered<'_>, SyntaxError> {
    recover_inner(input, mode, limits, limits.work.get()).map(|(result, _)| result)
}

pub(super) fn recover_prefix_budget<'a>(
    input: &'a str,
    mode: RecoveryMode,
    limits: RecoveryLimits,
    remaining: &mut usize,
) -> Result<Recovered<'a>, SyntaxError> {
    let (result, left) = recover_inner(input, mode, limits, *remaining)?;
    *remaining = left;
    Ok(result)
}

fn recover_inner(
    input: &str,
    mode: RecoveryMode,
    limits: RecoveryLimits,
    mut remaining: usize,
) -> Result<(Recovered<'_>, usize), SyntaxError> {
    let mut stack: Vec<TokenKind> = Vec::new();
    let capacity_limit = limits
        .nesting
        .get()
        .min(limits.retained_bytes.get().min(isize::MAX as usize) / size_of::<TokenKind>());
    let mut result = Recovered {
        block_start: None,
        raw: "",
        consumed: 0,
        boundary: Boundary::EndOfInput,
        invalid_tokens: false,
        recovery_only: false,
        implicit_close: false,
        unclosed_delimiters: 0,
    };
    let mut offset = 0;
    while offset < input.len() {
        let token = scan_token_budget(&input[offset..], &mut remaining)
            .map_err(|mut error| {
                error.offset += offset;
                error
            })?
            .expect("nonempty token input");
        result.invalid_tokens |=
            token.invalid_escape || matches!(token.kind, TokenKind::BadString | TokenKind::BadUri);
        result.implicit_close |= token.implicit_close;
        let next = offset + token.raw.len();
        if stack.is_empty() {
            if token.kind == TokenKind::CloseBrace && mode != RecoveryMode::QualifiedRule {
                result.boundary = Boundary::RuleClose;
                break;
            }
            if token.kind == TokenKind::Semicolon && mode != RecoveryMode::QualifiedRule {
                offset = next;
                result.boundary = Boundary::Semicolon;
                break;
            }
        }
        match token.kind {
            TokenKind::OpenBrace
            | TokenKind::OpenParen
            | TokenKind::OpenBracket
            | TokenKind::Function
            | TokenKind::BadUri => {
                if token.kind == TokenKind::OpenBrace {
                    if stack.is_empty() && result.block_start.is_none() {
                        result.block_start = Some(offset);
                    }
                    result.recovery_only = true;
                }
                if stack.len() >= limits.nesting.get() {
                    return Err(resource(offset, SyntaxResource::Depth));
                }
                if stack.len() == stack.capacity() {
                    let needed = stack.len() + 1;
                    if needed > capacity_limit {
                        return Err(resource(offset, SyntaxResource::Bytes));
                    }
                    let target = needed.max(stack.capacity().saturating_mul(2).min(capacity_limit));
                    remaining = remaining
                        .checked_sub(stack.len())
                        .ok_or_else(|| resource(offset, SyntaxResource::Work))?;
                    let mut candidate = Vec::new();
                    candidate
                        .try_reserve_exact(target)
                        .map_err(|_| resource(offset, SyntaxResource::Allocation))?;
                    if candidate.capacity() > capacity_limit {
                        return Err(resource(offset, SyntaxResource::Bytes));
                    }
                    candidate.extend_from_slice(&stack);
                    stack = candidate;
                }
                stack.push(match token.kind {
                    TokenKind::OpenBrace => TokenKind::CloseBrace,
                    TokenKind::OpenBracket => TokenKind::CloseBracket,
                    _ => TokenKind::CloseParen,
                });
            }
            TokenKind::CloseBrace | TokenKind::CloseParen | TokenKind::CloseBracket => {
                if stack.last() == Some(&token.kind) {
                    stack.pop();
                    if mode != RecoveryMode::Declaration
                        && stack.is_empty()
                        && token.kind == TokenKind::CloseBrace
                    {
                        offset = next;
                        result.boundary = Boundary::AtRuleBlock;
                        break;
                    }
                } else {
                    result.invalid_tokens = true;
                }
            }
            TokenKind::Semicolon | TokenKind::Cdo | TokenKind::Cdc => result.recovery_only = true,
            TokenKind::AtKeyword if mode == RecoveryMode::Declaration => {
                result.recovery_only = true
            }
            _ => {}
        }
        offset = next;
    }
    result.raw = &input[..offset];
    result.consumed = offset;
    result.unclosed_delimiters = stack.len();
    result.implicit_close |= !stack.is_empty();
    Ok((result, remaining))
}
