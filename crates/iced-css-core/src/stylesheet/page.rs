use super::{
    DeclarationLocation, RejectedDeclaration, RuleAssemblyError, RuleAssemblyLimits, RuleContext,
    RuleDeclaration, parse_declarations,
};
use crate::syntax::{
    RuleError, StatementContext, StatementKind, Statements, SyntaxError, TokenKind,
    decode_identifier_budget, scan_token_budget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSelector {
    All,
    First,
    Left,
    Right,
}
/// Page context is deliberately separate from source-element selector matching.
#[derive(Debug)]
pub struct ParsedPageRule {
    pub selector: PageSelector,
    pub declarations: Vec<RuleDeclaration>,
    pub locations: Vec<DeclarationLocation>,
    pub rejected: Vec<RejectedDeclaration>,
    pub consumed: usize,
    pub implicit_close: bool,
    pub next_context: Option<RuleContext>,
}
#[derive(Debug)]
pub enum PageAssembly {
    Accepted(ParsedPageRule),
    Discarded { consumed: usize, offset: usize },
}
/// Parse one page-rule prefix; does not render pages or offer page declarations
/// to the source-element cascade. Locations refer to the original input bytes.
pub fn parse_page_rule_prefix(
    input: &str,
    context: RuleContext,
    limits: RuleAssemblyLimits,
) -> Result<PageAssembly, RuleAssemblyError> {
    let statement = Statements::new(input, StatementContext::Block, limits.recovery)
        .next()
        .ok_or(RuleAssemblyError::Boundary(RuleError::Invalid))?
        .map_err(RuleAssemblyError::Syntax)?;
    let StatementKind::AtRule { keyword } = statement.kind else {
        return Err(RuleAssemblyError::Boundary(RuleError::Invalid));
    };
    let mut work = limits.values.syntax.work.get();
    let name = decode_identifier_budget(
        &keyword[1..],
        limits.values.syntax.retained_bytes,
        &mut work,
    )
    .map_err(|mut error| {
        error.offset += statement.span.start + 1;
        RuleAssemblyError::Syntax(error)
    })?;
    if !name.text.eq_ignore_ascii_case("page") {
        return Err(RuleAssemblyError::Boundary(RuleError::Invalid));
    }
    let consumed = statement.span.end;
    let discard = |offset| Ok(PageAssembly::Discarded { consumed, offset });
    let Some(body) = statement.body else {
        return discard(statement.prelude_span.end);
    };
    let selector = selector(statement.prelude, limits, &mut work).map_err(|mut error| {
        error.offset += statement.prelude_span.start;
        RuleAssemblyError::Syntax(error)
    })?;
    let selector = match selector {
        Ok(value) => value,
        Err(offset) => return discard(statement.prelude_span.start + offset),
    };
    let parsed = parse_declarations(
        body,
        statement.body_span.expect("body span").start,
        context,
        limits,
        true,
    )?;
    Ok(PageAssembly::Accepted(ParsedPageRule {
        selector,
        declarations: parsed.declarations,
        locations: parsed.locations,
        rejected: parsed.rejected,
        consumed,
        implicit_close: statement.implicit_close,
        next_context: parsed.next_context,
    }))
}
fn selector(
    input: &str,
    limits: RuleAssemblyLimits,
    work: &mut usize,
) -> Result<Result<PageSelector, usize>, SyntaxError> {
    let mut offset = 0;
    let mut state = 0;
    let mut selected = PageSelector::All;
    loop {
        let rebase = |mut error: SyntaxError| {
            error.offset += offset;
            error
        };
        let Some(token) = scan_token_budget(&input[offset..], work).map_err(rebase)? else {
            return Ok(if state == 1 {
                Err(offset)
            } else {
                Ok(selected)
            });
        };
        if token.kind == TokenKind::Comment || (token.kind == TokenKind::Space && state != 1) {
            offset += token.raw.len();
            continue;
        }
        if state == 0 && token.kind == TokenKind::Colon {
            state = 1;
        } else if state == 1 && token.kind == TokenKind::Ident && !token.invalid_escape {
            let name =
                decode_identifier_budget(token.raw, limits.values.syntax.retained_bytes, work)
                    .map_err(rebase)?;
            selected = if name.text.eq_ignore_ascii_case("first") {
                PageSelector::First
            } else if name.text.eq_ignore_ascii_case("left") {
                PageSelector::Left
            } else if name.text.eq_ignore_ascii_case("right") {
                PageSelector::Right
            } else {
                return Ok(Err(offset));
            };
            state = 2;
        } else {
            return Ok(Err(offset));
        }
        offset += token.raw.len();
    }
}
