//! `calc()` for lengths and percentages (CSS Values 3, section 8): sums and products
//! reduced at parse time to one coefficient per unit kind.
use super::CalcLength;
#[cfg(test)]
use super::RelativeUnit;
use crate::syntax::{TokenKind, scan_token_budget};

/// Parentheses and nested `calc()` deeper than this are refused.
const MAX_DEPTH: usize = 32;

/// A number, or a sum of lengths and a percentage: pixels, percent, then `em`, `ex`, `rem`
/// and `ch`.
#[derive(Clone, Copy)]
enum Term {
    Number(f64),
    Length([f64; 6]),
}

struct Parser<'a, 'w> {
    input: &'a str,
    offset: usize,
    work: &'w mut usize,
    /// The token `peek` scanned and nothing has taken, with whether a space came before.
    look: Option<(TokenKind, &'a str, bool)>,
}

impl<'a> Parser<'a, '_> {
    fn peek(&mut self) -> Option<(TokenKind, &'a str, bool)> {
        if self.look.is_none() {
            let mut space = false;
            while self.offset < self.input.len() {
                let token = scan_token_budget(&self.input[self.offset..], self.work).ok()??;
                self.offset += token.raw.len();
                if token.invalid_escape {
                    return None;
                }
                match token.kind {
                    TokenKind::Space => space = true,
                    TokenKind::Comment => {}
                    kind => {
                        self.look = Some((kind, token.raw, space));
                        break;
                    }
                }
            }
        }
        self.look
    }
    fn take(&mut self) -> Option<(TokenKind, &'a str, bool)> {
        let look = self.peek();
        self.look = None;
        look
    }
    /// Whether a space follows what was last taken (before the next token or the end).
    fn space_follows(&mut self) -> bool {
        self.peek().is_some_and(|(_, _, space)| space)
    }
    fn value(&mut self, depth: usize) -> Option<Term> {
        let (kind, raw, _) = self.take()?;
        match kind {
            TokenKind::Number => raw.parse().ok().map(Term::Number),
            TokenKind::Percentage => {
                let percent: f64 = raw[..raw.len() - 1].parse().ok()?;
                Some(Term::Length([0.0, percent, 0.0, 0.0, 0.0, 0.0]))
            }
            TokenKind::Dimension => {
                let split = raw
                    .bytes()
                    .position(|b| !(b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')))?;
                let number: f64 = raw[..split].parse().ok()?;
                let unit = raw[split..].to_ascii_lowercase();
                let mut terms = [0.0; 6];
                let (slot, scale) = match unit.as_str() {
                    "px" => (0, 1.0),
                    "in" => (0, 96.0),
                    "cm" => (0, 4800.0 / 127.0),
                    "mm" => (0, 480.0 / 127.0),
                    "pt" => (0, 4.0 / 3.0),
                    "pc" => (0, 16.0),
                    "em" => (2, 1.0),
                    "ex" => (3, 1.0),
                    "rem" => (4, 1.0),
                    "ch" => (5, 1.0),
                    _ => return None,
                };
                terms[slot] = number * scale;
                Some(Term::Length(terms))
            }
            TokenKind::OpenParen => self.group(depth),
            TokenKind::Function if raw.eq_ignore_ascii_case("calc(") => self.group(depth),
            _ => None,
        }
    }
    /// A sum up to its closing parenthesis, which it takes.
    fn group(&mut self, depth: usize) -> Option<Term> {
        if depth >= MAX_DEPTH {
            return None;
        }
        let sum = self.sum(depth + 1)?;
        matches!(self.take()?, (TokenKind::CloseParen, ..)).then_some(sum)
    }
    fn product(&mut self, depth: usize) -> Option<Term> {
        let mut left = self.value(depth)?;
        while let Some((TokenKind::Delim, operator @ ("*" | "/"), _)) = self.peek() {
            self.take();
            let right = self.value(depth)?;
            left = match (operator, left, right) {
                ("*", Term::Number(a), Term::Number(b)) => Term::Number(a * b),
                ("*", Term::Number(n), Term::Length(terms))
                | ("*", Term::Length(terms), Term::Number(n)) => {
                    Term::Length(terms.map(|term| term * n))
                }
                // Only a number divides, and not zero.
                ("/", _, Term::Number(n)) if n == 0.0 => return None,
                ("/", Term::Number(a), Term::Number(b)) => Term::Number(a / b),
                ("/", Term::Length(terms), Term::Number(n)) => {
                    Term::Length(terms.map(|term| term / n))
                }
                _ => return None,
            };
        }
        Some(left)
    }
    fn sum(&mut self, depth: usize) -> Option<Term> {
        let mut left = self.product(depth)?;
        while let Some((TokenKind::Delim, operator @ ("+" | "-"), before)) = self.peek() {
            self.take();
            // 8.1: white space on both sides of `+` and `-`.
            if !before || !self.space_follows() {
                return None;
            }
            let right = self.product(depth)?;
            let sign = if operator == "+" { 1.0 } else { -1.0 };
            left = match (left, right) {
                (Term::Number(a), Term::Number(b)) => Term::Number(a + sign * b),
                (Term::Length(mut a), Term::Length(b)) => {
                    for (term, other) in a.iter_mut().zip(b) {
                        *term += sign * other;
                    }
                    Term::Length(a)
                }
                // A number and a length do not add.
                _ => return None,
            };
        }
        Some(left)
    }
}

/// Parse a whole `calc(...)` function, from its name to its closing parenthesis. `None`
/// is invalid: bad syntax, a unit that is no length, mixed types, a plain number, or a
/// value that is not finite.
pub(super) fn parse_calc(raw: &str, work: &mut usize) -> Option<CalcLength> {
    let mut parser = Parser {
        input: raw,
        offset: 0,
        work,
        look: None,
    };
    let (kind, name, _) = parser.take()?;
    if kind != TokenKind::Function || !name.eq_ignore_ascii_case("calc(") {
        return None;
    }
    let Term::Length(terms) = parser.group(0)? else {
        return None;
    };
    if parser.take().is_some() {
        return None;
    }
    let terms = terms.map(|term| term as f32);
    terms
        .iter()
        .all(|term| term.is_finite())
        .then(|| CalcLength::new(terms[0], terms[1], [terms[2], terms[3], terms[4], terms[5]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELATIVE: [RelativeUnit; 4] = [
        RelativeUnit::Em,
        RelativeUnit::Ex,
        RelativeUnit::Rem,
        RelativeUnit::Ch,
    ];

    fn calc(input: &str) -> Option<(f32, f32, [f32; 4])> {
        let mut work = 100_000;
        parse_calc(input, &mut work).map(|value| {
            (
                value.pixels(),
                value.percent(),
                RELATIVE.map(|unit| value.relative(unit)),
            )
        })
    }

    #[test]
    fn sums_and_products_reduce_to_one_term_per_unit() {
        let none = [0.0; 4];
        assert_eq!(calc("calc(10px)"), Some((10.0, 0.0, none)));
        assert_eq!(calc("calc(100% - 20px)"), Some((-20.0, 100.0, none)));
        assert_eq!(
            calc("CALC( 50% + 2em - 1px )"),
            Some((-1.0, 50.0, [2.0, 0.0, 0.0, 0.0]))
        );
        assert_eq!(calc("calc(2 * 10px)"), Some((20.0, 0.0, none)));
        assert_eq!(calc("calc(10px * 2 / 4)"), Some((5.0, 0.0, none)));
        assert_eq!(calc("calc((100% - 10px) / 2)"), Some((-5.0, 50.0, none)));
        assert_eq!(calc("calc(1in + 1pc)"), Some((112.0, 0.0, none)));
        assert_eq!(
            calc("calc(1rem + 2ch + 3ex)"),
            Some((0.0, 0.0, [0.0, 3.0, 1.0, 2.0]))
        );
        assert_eq!(
            calc("calc(calc(1px + 2px) * (1 + 2))"),
            Some((9.0, 0.0, none))
        );
        assert_eq!(calc("calc(10px - -5px)"), Some((15.0, 0.0, none)));
        assert_eq!(
            calc("calc(/**/10px/**/ + /**/5px)"),
            Some((15.0, 0.0, none))
        );
    }

    #[test]
    fn invalid_expressions_are_refused() {
        for input in [
            "calc()",
            "calc(10)",
            "calc(1 + 2)",
            "calc(10px + 5)",
            "calc(10px+5px)",
            "calc(10px -5px)",
            "calc(10px - 5px",
            "calc(10px * 5px)",
            "calc(10 / 5px)",
            "calc(10px / 0)",
            "calc(10px 5px)",
            "calc(10px +)",
            "calc(10deg)",
            "calc(10px) 5px",
            "calx(10px)",
            "(10px)",
        ] {
            assert_eq!(calc(input), None, "{input}");
        }
        let deep = format!("calc({}1px{})", "(".repeat(40), ")".repeat(40));
        assert_eq!(calc(&deep), None);
        let mut work = 3;
        assert!(parse_calc("calc(1px + 2px + 3px)", &mut work).is_none());
    }
}
