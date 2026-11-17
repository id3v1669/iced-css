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
    /// Selectors Level 3 `~`: any earlier sibling.
    Sibling,
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
            // Selectors Level 3 substring matches: two delimiters with nothing between.
            TokenKind::Delim if matches!(look.token.raw, "^" | "$" | "*") => {
                let equals = self.take()?;
                if equals.space || equals.token.kind != TokenKind::Delim || equals.token.raw != "="
                {
                    return Err(SyntaxError {
                        offset: equals.start,
                        fault: SyntaxFault::InvalidSelector,
                    });
                }
                match look.token.raw {
                    "^" => AttributeOperator::Prefix,
                    "$" => AttributeOperator::Suffix,
                    _ => AttributeOperator::Substring,
                }
            }
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
    /// `an+b` up to the closing parenthesis, which it consumes (Selectors Level 3, 6.6.5.2).
    fn nth(&mut self) -> Result<(i32, i32), SyntaxError> {
        let invalid = |parser: &Self| parser.invalid();
        // The tokens' text, a space kept only where one was written.
        let mut text = String::new();
        loop {
            let look = self.take()?;
            match look.token.kind {
                TokenKind::CloseParen => break,
                TokenKind::Ident | TokenKind::Number | TokenKind::Dimension => {}
                TokenKind::Delim if matches!(look.token.raw, "+" | "-") => {}
                _ => {
                    return Err(SyntaxError {
                        offset: look.start,
                        fault: SyntaxFault::InvalidSelector,
                    });
                }
            }
            if look.token.raw.len() > 32 || text.len() > 64 {
                return Err(invalid(self));
            }
            self.work = self
                .work
                .checked_sub(look.token.raw.len())
                .ok_or_else(|| self.resource(SyntaxResource::Work))?;
            if look.space && !text.is_empty() {
                text.push(' ');
            }
            text.push_str(look.token.raw);
        }
        parse_nth(&text.to_ascii_lowercase()).ok_or_else(|| invalid(self))
    }
    /// `:not(`'s argument: one simple selector, as Selectors Level 3 has it, or a list of
    /// compound selectors, as Level 4 does.
    fn negation(&mut self) -> Result<SimpleSelector, SyntaxError> {
        match self.any_of(true, false)? {
            SimpleSelector::Matches { list, .. } if list.len() == 1 && list[0].len() == 1 => Ok(
                SimpleSelector::Not(Box::new(list.into_iter().flatten().next().expect("one"))),
            ),
            other => Ok(other),
        }
    }
    /// `:has(`'s relative selectors, comma-separated, through its closing parenthesis:
    /// each an optional `>`, `+` or `~` and a compound selector with no pseudo-element and
    /// no `:has()` of its own.
    fn relatives(&mut self) -> Result<SimpleSelector, SyntaxError> {
        let mut relatives = Vec::new();
        let mut highest = Specificity::ZERO;
        loop {
            let combinator = match self.peek()? {
                Some(look) if look.token.kind == TokenKind::Delim => match look.token.raw {
                    ">" => Some(Combinator::Child),
                    "+" => Some(Combinator::Adjacent),
                    "~" => Some(Combinator::Sibling),
                    _ => None,
                },
                _ => None,
            };
            if combinator.is_some() {
                self.take()?;
            }
            let mut target = None;
            let mut weight = Specificity::ZERO;
            let compound = self.compound(&mut target, &mut weight)?;
            if target.is_some()
                || compound
                    .tests
                    .iter()
                    .any(|test| matches!(test, SimpleSelector::Has { .. }))
            {
                return Err(self.invalid());
            }
            highest = highest.max(weight);
            self.push(
                &mut relatives,
                (combinator.unwrap_or(Combinator::Descendant), compound.tests),
            )?;
            let look = self.take()?;
            match look.token.kind {
                TokenKind::Delim if look.token.raw == "," => {}
                TokenKind::CloseParen => break,
                _ => return Err(self.invalid()),
            }
        }
        Ok(SimpleSelector::Has {
            relatives,
            specificity: highest,
        })
    }
    /// The compound selectors of `:is(`, `:where(` or `:not(`, comma-separated, through
    /// its closing parenthesis. None may hold a pseudo-element, and none a combinator.
    fn any_of(&mut self, negated: bool, weightless: bool) -> Result<SimpleSelector, SyntaxError> {
        let mut list = Vec::new();
        let mut highest = Specificity::ZERO;
        loop {
            let mut target = None;
            let mut weight = Specificity::ZERO;
            let compound = self.compound(&mut target, &mut weight)?;
            if target.is_some() {
                return Err(self.invalid());
            }
            // `:has()` is tested by the matcher, not inside another list.
            if compound
                .tests
                .iter()
                .any(|test| matches!(test, SimpleSelector::Has { .. }))
            {
                return Err(self.invalid());
            }
            highest = highest.max(weight);
            self.push(&mut list, compound.tests)?;
            let look = self.take()?;
            match look.token.kind {
                TokenKind::Delim if look.token.raw == "," => {}
                TokenKind::CloseParen => break,
                _ => return Err(self.invalid()),
            }
        }
        Ok(SimpleSelector::Matches {
            list,
            negated,
            specificity: if weightless {
                Specificity::ZERO
            } else {
                highest
            },
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
            let function = name.to_ascii_lowercase();
            self.release_name(name);
            // A functional pseudo-class has one colon.
            if double {
                return Err(self.invalid());
            }
            return Ok(Some(match function.as_str() {
                "lang" => {
                    let argument = self.identifier()?;
                    self.expect(TokenKind::CloseParen)?;
                    SimpleSelector::Lang(argument)
                }
                "not" => self.negation()?,
                "is" | "matches" => self.any_of(false, false)?,
                "has" => self.relatives()?,
                "where" => self.any_of(false, true)?,
                "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                    let (a, b) = self.nth()?;
                    SimpleSelector::Nth {
                        a,
                        b,
                        last: function.contains("last"),
                        of_type: function.ends_with("of-type"),
                    }
                }
                _ => return Err(self.invalid()),
            }));
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
            let nth = |last, of_type| {
                Some(SimpleSelector::Nth {
                    a: 0,
                    b: 1,
                    last,
                    of_type,
                })
            };
            match name.to_ascii_lowercase().as_str() {
                "root" => Some(SimpleSelector::Root),
                "checked" => Some(SimpleSelector::Checked),
                "disabled" => Some(SimpleSelector::Disabled),
                "enabled" => Some(SimpleSelector::Enabled),
                "empty" => Some(SimpleSelector::Empty),
                "last-child" => nth(true, false),
                "first-of-type" => nth(false, true),
                "last-of-type" => nth(true, true),
                "only-child" => Some(SimpleSelector::Only { of_type: false }),
                "only-of-type" => Some(SimpleSelector::Only { of_type: true }),
                _ => None,
            }
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
                    // A HASH name may start with a digit or "-digit"; an ID
                    // selector's must be an identifier (CSS 2.2 5.9).
                    Some(SimpleSelector::Id(self.name(
                        &look.token.raw[1..],
                        look.start + 1,
                        true,
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
        // A negation counts as its argument does (Selectors Level 3, 9), and `:is()` and
        // `:not()` of a list as their most specific argument (Level 4, 17).
        fn weight(test: &SimpleSelector) -> Specificity {
            match test {
                SimpleSelector::Not(inner) => weight(inner),
                SimpleSelector::Matches { specificity, .. }
                | SimpleSelector::Has { specificity, .. } => *specificity,
                SimpleSelector::Universal => Specificity::ZERO,
                SimpleSelector::Type(_) => Specificity::selector(0, 0, 1),
                SimpleSelector::Id(_) => Specificity::selector(1, 0, 0),
                _ => Specificity::selector(0, 1, 0),
            }
        }
        let delta = weight(&test);
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
            let combinator = if look.token.kind == TokenKind::Delim
                && matches!(look.token.raw, ">" | "+" | "~")
            {
                self.take()?;
                match look.token.raw {
                    ">" => Combinator::Child,
                    "+" => Combinator::Adjacent,
                    _ => Combinator::Sibling,
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
/// `an+b` in lower case with single spaces where any were written: `odd`, `even`, an
/// integer, or `[sign][digits]n` with an optional signed offset whose sign may stand apart.
fn parse_nth(text: &str) -> Option<(i32, i32)> {
    match text {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let integer = |digits: &str| -> Option<i32> {
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .then(|| digits.parse().ok())
            .flatten()
    };
    let signed = |text: &str| -> Option<i32> {
        match text.strip_prefix('-') {
            Some(digits) => integer(digits).map(|value| -value),
            None => integer(text.strip_prefix('+').unwrap_or(text)),
        }
    };
    let Some((step, offset)) = text.split_once('n') else {
        return signed(text).map(|b| (0, b));
    };
    let a = match step {
        "" | "+" => 1,
        "-" => -1,
        _ => signed(step)?,
    };
    // After `n`: nothing, or a sign and digits with spaces allowed around the sign only.
    let offset = offset.trim_start_matches(' ');
    if offset.is_empty() {
        return Some((a, 0));
    }
    let negative = match offset.as_bytes()[0] {
        b'+' => false,
        b'-' => true,
        _ => return None,
    };
    let digits = offset[1..].trim_start_matches(' ');
    let b = integer(digits)?;
    Some((a, if negative { -b } else { b }))
}
/// Parse a complete selector group: CSS 2.2 selectors and those of Selectors Level 3. Any invalid member rejects the group.
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
