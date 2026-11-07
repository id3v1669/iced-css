use super::{AttributeOperator, SimpleSelector};
use crate::{
    cascade::Specificity,
    syntax::{
        SyntaxError, SyntaxFault, SyntaxResource, Token, TokenKind, decode_name_budget,
        decode_string_budget, scan_token_budget,
    },
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    Adjacent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
    FirstLine,
    FirstLetter,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compound {
    pub tests: Vec<SimpleSelector>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<Combinator>,
    pub pseudo: Option<PseudoElement>,
    pub specificity: Specificity,
}
#[derive(Debug, Clone, Copy)]
pub struct SelectorLimits {
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub entries: NonZeroUsize,
}

#[derive(Clone, Copy)]
struct Look<'a> {
    start: usize,
    token: Token<'a>,
    space: bool,
}
struct Parser<'a> {
    input: &'a str,
    offset: usize,
    look: Option<Look<'a>>,
    work: usize,
    bytes: usize,
    used: usize,
    entries: usize,
}
impl<'a> Parser<'a> {
    fn invalid(&self) -> SyntaxError {
        SyntaxError {
            offset: self.look.map_or(self.offset, |look| look.start),
            fault: SyntaxFault::InvalidSelector,
        }
    }
    fn resource(&self, kind: SyntaxResource) -> SyntaxError {
        SyntaxError {
            offset: self.look.map_or(self.offset, |look| look.start),
            fault: SyntaxFault::Resource(kind),
        }
    }
    fn peek(&mut self) -> Result<Option<Look<'a>>, SyntaxError> {
        if self.look.is_some() {
            return Ok(self.look);
        }
        let mut space = false;
        while self.offset < self.input.len() {
            let start = self.offset;
            let token = scan_token_budget(&self.input[start..], &mut self.work)
                .map_err(|mut error| {
                    error.offset += start;
                    error
                })?
                .expect("nonempty input");
            self.offset += token.raw.len();
            if token.invalid_escape
                || matches!(token.kind, TokenKind::BadString | TokenKind::BadUri)
            {
                return Err(SyntaxError {
                    offset: start,
                    fault: SyntaxFault::InvalidSelector,
                });
            }
            if token.kind == TokenKind::Space {
                space = true;
                continue;
            }
            if token.kind == TokenKind::Comment {
                continue;
            }
            self.look = Some(Look {
                start,
                token,
                space,
            });
            break;
        }
        Ok(self.look)
    }
    fn take(&mut self) -> Result<Look<'a>, SyntaxError> {
        let value = self.peek()?.ok_or_else(|| self.invalid())?;
        self.look = None;
        Ok(value)
    }
    fn expect(&mut self, kind: TokenKind) -> Result<(), SyntaxError> {
        let value = self.take()?;
        if value.token.kind == kind {
            Ok(())
        } else {
            Err(SyntaxError {
                offset: value.start,
                fault: SyntaxFault::InvalidSelector,
            })
        }
    }
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), SyntaxError> {
        self.entries = self
            .entries
            .checked_sub(1)
            .ok_or_else(|| self.resource(SyntaxResource::Entries))?;
        if values.len() == values.capacity() {
            let size = size_of::<T>();
            let available = (self.bytes - self.used) / size;
            let old = values.capacity();
            let needed = old
                .checked_add(1)
                .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
            let max = old
                .checked_add(available)
                .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
            if needed > max {
                return Err(self.resource(SyntaxResource::Bytes));
            }
            let target = needed.max(old.saturating_mul(2).min(max));
            self.work = self
                .work
                .checked_sub(values.len())
                .ok_or_else(|| self.resource(SyntaxResource::Work))?;
            values
                .try_reserve_exact(target - values.len())
                .map_err(|_| self.resource(SyntaxResource::Allocation))?;
            let added = (values.capacity() - old)
                .checked_mul(size)
                .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
            self.used = self
                .used
                .checked_add(added)
                .filter(|used| *used <= self.bytes)
                .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
        }
        values.push(value);
        Ok(())
    }
    fn name(
        &mut self,
        raw: &str,
        start: usize,
        identifier: bool,
        string: bool,
    ) -> Result<String, SyntaxError> {
        let available = NonZeroUsize::new(self.bytes - self.used)
            .ok_or_else(|| self.resource(SyntaxResource::Bytes))?;
        let text = if string {
            decode_string_budget(raw, available, &mut self.work).map(|value| value.text)
        } else {
            decode_name_budget(raw, available, &mut self.work, identifier).map(|value| value.text)
        }
        .map_err(|mut error| {
            error.offset += start;
            if !matches!(error.fault, SyntaxFault::Resource(_)) {
                error.fault = SyntaxFault::InvalidSelector;
            }
            error
        })?;
        self.used += text.capacity();
        Ok(text)
    }
    fn release_name(&mut self, name: String) {
        self.used -= name.capacity();
    }
    fn identifier(&mut self) -> Result<String, SyntaxError> {
        let look = self.take()?;
        if look.token.kind != TokenKind::Ident {
            return Err(SyntaxError {
                offset: look.start,
                fault: SyntaxFault::InvalidSelector,
            });
        }
        self.name(look.token.raw, look.start, true, false)
    }
    fn attribute(&mut self) -> Result<SimpleSelector, SyntaxError> {
        let name = self.identifier()?;
        let look = self.take()?;
        if look.token.kind == TokenKind::CloseBracket {
            return Ok(SimpleSelector::Attribute {
                name,
                operator: AttributeOperator::Present,
                value: String::new(),
            });
        }
        let operator = match look.token.kind {
            TokenKind::Includes => AttributeOperator::Includes,
            TokenKind::DashMatch => AttributeOperator::DashMatch,
            TokenKind::Delim if look.token.raw == "=" => AttributeOperator::Equal,
            _ => {
                return Err(SyntaxError {
                    offset: look.start,
                    fault: SyntaxFault::InvalidSelector,
                });
            }
        };
        let look = self.take()?;
        let value = match look.token.kind {
            TokenKind::Ident => self.name(look.token.raw, look.start, true, false)?,
            TokenKind::String => self.name(look.token.raw, look.start, false, true)?,
            _ => {
                return Err(SyntaxError {
                    offset: look.start,
                    fault: SyntaxFault::InvalidSelector,
                });
            }
        };
        self.expect(TokenKind::CloseBracket)?;
        Ok(SimpleSelector::Attribute {
            name,
            operator,
            value,
        })
    }
    fn pseudo(
        &mut self,
        target: &mut Option<PseudoElement>,
    ) -> Result<Option<SimpleSelector>, SyntaxError> {
        let mut look = self.take()?;
        if look.space {
            return Err(SyntaxError {
                offset: look.start,
                fault: SyntaxFault::InvalidSelector,
            });
        }
        // Selectors Level 3 writes the CSS 2.2 pseudo-elements with two colons; only those
        // four are admitted that way. A comment between the colons is no separator, as in
        // CSS Syntax.
        let double = look.token.kind == TokenKind::Colon;
        if double {
            look = self.take()?;
            if look.space || look.token.kind != TokenKind::Ident {
                return Err(SyntaxError {
                    offset: look.start,
                    fault: SyntaxFault::InvalidSelector,
                });
            }
        }
        if look.token.kind == TokenKind::Function {
            let name = self.name(
                &look.token.raw[..look.token.raw.len() - 1],
                look.start,
                true,
                false,
            )?;
            let lang = name.eq_ignore_ascii_case("lang");
            self.release_name(name);
            if !lang {
                return Err(self.invalid());
            }
            let argument = self.identifier()?;
            self.expect(TokenKind::CloseParen)?;
            return Ok(Some(SimpleSelector::Lang(argument)));
        }
        if look.token.kind != TokenKind::Ident {
            return Err(self.invalid());
        }
        let name = self.name(look.token.raw, look.start, true, false)?;
        let pseudo = if double {
            None
        } else if name.eq_ignore_ascii_case("first-child") {
            Some(SimpleSelector::FirstChild)
        } else if name.eq_ignore_ascii_case("link") {
            Some(SimpleSelector::Link)
        } else if name.eq_ignore_ascii_case("visited") {
            Some(SimpleSelector::Visited)
        } else if name.eq_ignore_ascii_case("hover") {
            Some(SimpleSelector::Hover)
        } else if name.eq_ignore_ascii_case("active") {
            Some(SimpleSelector::Active)
        } else if name.eq_ignore_ascii_case("focus") {
            Some(SimpleSelector::Focus)
        } else {
            None
        };
        if pseudo.is_none() {
            *target = if name.eq_ignore_ascii_case("before") {
                Some(PseudoElement::Before)
            } else if name.eq_ignore_ascii_case("after") {
                Some(PseudoElement::After)
            } else if name.eq_ignore_ascii_case("first-line") {
                Some(PseudoElement::FirstLine)
            } else if name.eq_ignore_ascii_case("first-letter") {
                Some(PseudoElement::FirstLetter)
            } else {
                self.release_name(name);
                return Err(self.invalid());
            };
        }
        self.release_name(name);
        Ok(pseudo)
    }
    fn compound(
        &mut self,
        target: &mut Option<PseudoElement>,
        specificity: &mut Specificity,
    ) -> Result<Compound, SyntaxError> {
        let mut tests = Vec::new();
        let first = self.peek()?.ok_or_else(|| self.invalid())?;
        if first.token.kind == TokenKind::Ident {
            let name = self.identifier()?;
            self.add_test(&mut tests, SimpleSelector::Type(name), specificity)?;
        } else if first.token.kind == TokenKind::Delim && first.token.raw == "*" {
            self.take()?;
            self.add_test(&mut tests, SimpleSelector::Universal, specificity)?;
        }
        let mut started = !tests.is_empty();
        while let Some(look) = self.peek()? {
            if started && look.space {
                break;
            }
            let test = match look.token.kind {
                TokenKind::Hash => {
                    self.take()?;
                    Some(SimpleSelector::Id(self.name(
                        &look.token.raw[1..],
                        look.start + 1,
                        false,
                        false,
                    )?))
                }
                TokenKind::Delim if look.token.raw == "." => {
                    self.take()?;
                    if self.peek()?.is_some_and(|look| look.space) {
                        return Err(self.invalid());
                    }
                    Some(SimpleSelector::Class(self.identifier()?))
                }
                TokenKind::OpenBracket => {
                    self.take()?;
                    Some(self.attribute()?)
                }
                TokenKind::Colon => {
                    self.take()?;
                    self.pseudo(target)?
                }
                _ => break,
            };
            started = true;
            if let Some(test) = test {
                self.add_test(&mut tests, test, specificity)?;
            }
            if target.is_some() {
                break;
            }
        }
        if !started {
            return Err(self.invalid());
        }
        if tests.is_empty() {
            self.add_test(&mut tests, SimpleSelector::Universal, specificity)?;
        }
        Ok(Compound { tests })
    }
    fn add_test(
        &mut self,
        tests: &mut Vec<SimpleSelector>,
        test: SimpleSelector,
        specificity: &mut Specificity,
    ) -> Result<(), SyntaxError> {
        let delta = match test {
            SimpleSelector::Universal => Specificity::ZERO,
            SimpleSelector::Type(_) => Specificity::selector(0, 0, 1),
            SimpleSelector::Id(_) => Specificity::selector(1, 0, 0),
            _ => Specificity::selector(0, 1, 0),
        };
        *specificity = specificity
            .checked_selector_sum(delta)
            .map_err(|_| self.resource(SyntaxResource::Entries))?;
        self.push(tests, test)
    }
    fn selector(&mut self) -> Result<Selector, SyntaxError> {
        let mut selector = Selector {
            compounds: Vec::new(),
            combinators: Vec::new(),
            pseudo: None,
            specificity: Specificity::ZERO,
        };
        loop {
            let compound = self.compound(&mut selector.pseudo, &mut selector.specificity)?;
            self.push(&mut selector.compounds, compound)?;
            let Some(look) = self.peek()? else {
                break;
            };
            if look.token.kind == TokenKind::Delim && look.token.raw == "," {
                break;
            }
            if selector.pseudo.is_some() {
                return Err(self.invalid());
            }
            let combinator =
                if look.token.kind == TokenKind::Delim && matches!(look.token.raw, ">" | "+") {
                    self.take()?;
                    if look.token.raw == ">" {
                        Combinator::Child
                    } else {
                        Combinator::Adjacent
                    }
                } else if look.space {
                    Combinator::Descendant
                } else {
                    return Err(self.invalid());
                };
            self.push(&mut selector.combinators, combinator)?;
        }
        if selector.pseudo.is_some() {
            selector.specificity = selector
                .specificity
                .checked_selector_sum(Specificity::selector(0, 0, 1))
                .map_err(|_| self.resource(SyntaxResource::Entries))?;
        }
        Ok(selector)
    }
}
/// Parse a complete CSS2.2 selector group. Any invalid member rejects the group.
/// Work, retained semantic-string/AST capacities and AST entry count are bounded.
/// Fixed state, temporary decoder repair buffers and allocator overhead are not
/// retained AST storage. No partial list is returned after an error.
pub fn parse_selector_list(
    input: &str,
    limits: SelectorLimits,
) -> Result<Vec<Selector>, SyntaxError> {
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(SyntaxError {
            offset: 0,
            fault: SyntaxFault::Resource(SyntaxResource::Bytes),
        });
    }
    let mut parser = Parser {
        input,
        offset: 0,
        look: None,
        work: limits.work.get(),
        bytes: limits.retained_bytes.get(),
        used: 0,
        entries: limits.entries.get(),
    };
    let mut selectors = Vec::new();
    loop {
        let selector = parser.selector()?;
        parser.push(&mut selectors, selector)?;
        let Some(look) = parser.peek()? else {
            break;
        };
        if look.token.kind != TokenKind::Delim || look.token.raw != "," {
            return Err(parser.invalid());
        }
        parser.take()?;
    }
    Ok(selectors)
}
