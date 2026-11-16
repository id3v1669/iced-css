//! Custom properties and `var()` (CSS Custom Properties for Cascading Variables Level 1).
//!
//! A custom property's value is its tokens, kept as text. `var()` is replaced at
//! computed-value time, per element, before the property that holds it is parsed.
use crate::syntax::{TokenKind, scan_token_budget};
use std::collections::BTreeMap;

/// An element's computed custom properties by name (with its two dashes). `None` is the
/// guaranteed-invalid value: a reference to it takes its fallback, or fails.
pub type CustomValues = BTreeMap<String, Option<String>>;

/// The longest text a substitution may produce: longer is invalid at computed-value time.
pub const MAX_SUBSTITUTED_BYTES: usize = 64 * 1024;
/// `var()` in a fallback in a fallback, this deep at most.
const MAX_DEPTH: usize = 32;

/// Whether a declaration's value holds a `var()` and so parses at computed-value time.
pub fn has_reference(value: &str) -> bool {
    value
        .as_bytes()
        .windows(4)
        .any(|window| window.eq_ignore_ascii_case(b"var("))
}

/// One `var(` and everything through its closing parenthesis.
struct Reference<'a> {
    name: &'a str,
    /// The text after the first comma, when there is one (it may be empty).
    fallback: Option<&'a str>,
    /// Where it ends in the scanned text.
    end: usize,
}

/// Reads a reference whose `var(` token ends at `start`. `None` is invalid syntax.
fn reference<'a>(text: &'a str, start: usize, work: &mut usize) -> Option<Reference<'a>> {
    let mut offset = start;
    let mut name = None;
    let mut fallback_from = None;
    let mut depth = 1usize;
    while offset < text.len() {
        let token = scan_token_budget(&text[offset..], work).ok()??;
        let at = offset;
        offset += token.raw.len();
        if matches!(token.kind, TokenKind::BadString | TokenKind::BadUri) {
            return None;
        }
        match token.kind {
            TokenKind::Function | TokenKind::OpenParen | TokenKind::OpenBracket => depth += 1,
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBracket | TokenKind::CloseBrace => depth = depth.checked_sub(1)?,
            TokenKind::CloseParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(Reference {
                        name: name?,
                        fallback: fallback_from.map(|from| &text[from..at]),
                        end: offset,
                    });
                }
            }
            _ => {}
        }
        if fallback_from.is_some() {
            continue;
        }
        // Before the comma: white space, then one custom property name, then white space.
        match token.kind {
            TokenKind::Space | TokenKind::Comment => {}
            // The CSS 2.2 scanner reads `--name` as a dash, then `-name` right after it.
            TokenKind::Delim if name.is_none() && token.raw == "-" => {
                let rest = scan_token_budget(&text[offset..], work).ok()??;
                if rest.kind != TokenKind::Ident || !rest.raw.starts_with('-') {
                    return None;
                }
                offset += rest.raw.len();
                name = Some(&text[at..offset]);
            }
            TokenKind::Delim if token.raw == "," && name.is_some() && depth == 1 => {
                fallback_from = Some(offset);
            }
            _ => return None,
        }
    }
    None
}

fn substitute_at(
    text: &str,
    values: &dyn Fn(&str) -> Option<String>,
    work: &mut usize,
    depth: usize,
    out: &mut String,
) -> Option<()> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut offset = 0;
    while offset < text.len() {
        let token = scan_token_budget(&text[offset..], work).ok()??;
        offset += token.raw.len();
        if token.kind != TokenKind::Function || !token.raw.eq_ignore_ascii_case("var(") {
            out.push_str(token.raw);
        } else {
            let found = reference(text, offset, work)?;
            offset = found.end;
            // Substitution is of tokens: a comment on each side keeps the inserted text
            // from joining what stands beside it into another token.
            out.push_str("/**/");
            match values(found.name) {
                Some(value) => out.push_str(&value),
                None => substitute_at(found.fallback?, values, work, depth + 1, out)?,
            }
            out.push_str("/**/");
        }
        if out.len() > MAX_SUBSTITUTED_BYTES {
            return None;
        }
    }
    Some(())
}

/// `text` with every `var()` replaced by its property's value from `values`, or by its
/// fallback when the property has none. `None` is invalid at computed-value time: a
/// reference without a value or fallback, bad `var()` syntax, or a result too long.
pub fn substitute(
    text: &str,
    values: &dyn Fn(&str) -> Option<String>,
    work: &mut usize,
) -> Option<String> {
    let mut out = String::new();
    substitute_at(text, values, work, 0, &mut out)?;
    Some(out)
}

/// An element's computed custom properties: what it inherits, under what its own
/// declarations give, with each `var()` in them replaced. A property that refers to
/// itself, directly or through others, is invalid, and so is each one in the cycle.
pub fn compute(inherited: &CustomValues, own: &[(&str, &str)], work: &mut usize) -> CustomValues {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Raw,
        Visiting,
        Done,
    }
    struct Pass<'a, 'w> {
        own: &'a [(&'a str, &'a str)],
        state: Vec<State>,
        /// The properties being resolved, outermost first.
        stack: Vec<usize>,
        /// In a cycle of references: invalid whatever its fallbacks.
        cyclic: Vec<bool>,
        values: CustomValues,
        work: &'w mut usize,
    }
    impl Pass<'_, '_> {
        fn resolve(&mut self, at: usize) {
            self.state[at] = State::Visiting;
            self.stack.push(at);
            let (name, raw) = self.own[at];
            let mut out = String::new();
            let computed = {
                // Resolve what this value refers to first, then read it.
                let mut offset = 0;
                let mut names = Vec::new();
                let mut dash = None;
                while offset < raw.len() {
                    let Some(Some(token)) = scan_token_budget(&raw[offset..], self.work).ok()
                    else {
                        break;
                    };
                    let at = offset;
                    offset += token.raw.len();
                    // `--name`: a dash, then `-name` right after it.
                    if token.kind == TokenKind::Ident && token.raw.starts_with('-') {
                        if let Some(from) = dash.filter(|from| from + 1 == at) {
                            names.push(&raw[from..offset]);
                        }
                    }
                    dash = (token.kind == TokenKind::Delim && token.raw == "-").then_some(at);
                }
                for referred in names {
                    match self.own.iter().position(|(other, _)| *other == referred) {
                        // Everything from that property down to this one is in the cycle.
                        Some(other) if self.state[other] == State::Visiting => {
                            let from = self.stack.iter().position(|on| *on == other);
                            for member in &self.stack[from.unwrap_or(0)..] {
                                self.cyclic[*member] = true;
                            }
                        }
                        Some(other) if self.state[other] == State::Raw => self.resolve(other),
                        _ => {}
                    }
                }
                let values = &self.values;
                let lookup = |name: &str| values.get(name).cloned().flatten();
                (!self.cyclic[at])
                    .then(|| substitute_at(raw, &lookup, self.work, 0, &mut out))
                    .flatten()
                    .map(|()| out)
            };
            self.values.insert(name.to_owned(), computed);
            self.stack.pop();
            self.state[at] = State::Done;
        }
    }
    // Own declarations replace inherited values before anything is read: a reference to a
    // name declared here is to this element's value, never the inherited one.
    let mut values = inherited.clone();
    for (name, _) in own {
        values.remove(*name);
    }
    let mut pass = Pass {
        own,
        state: vec![State::Raw; own.len()],
        stack: Vec::new(),
        cyclic: vec![false; own.len()],
        values,
        work,
    };
    for at in 0..own.len() {
        if pass.state[at] == State::Raw {
            pass.resolve(at);
        }
    }
    pass.values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, Option<&str>)]) -> CustomValues {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.map(str::to_owned)))
            .collect()
    }
    fn replaced(text: &str, known: &CustomValues) -> Option<String> {
        let mut work = 100_000;
        substitute(text, &|name| known.get(name).cloned().flatten(), &mut work)
            .map(|text| text.replace("/**/", ""))
    }

    #[test]
    fn references_take_the_value_or_the_fallback() {
        let known = values(&[
            ("--a", Some("10px")),
            ("--bad", None),
            ("--empty", Some("")),
        ]);
        assert_eq!(replaced("var(--a)", &known).as_deref(), Some("10px"));
        assert_eq!(
            replaced("1px var( --a ) solid", &known).as_deref(),
            Some("1px 10px solid")
        );
        assert_eq!(replaced("VAR(--a, 5px)", &known).as_deref(), Some("10px"));
        assert_eq!(replaced("var(--b, 5px)", &known).as_deref(), Some(" 5px"));
        assert_eq!(
            replaced("var(--b, var(--a))", &known).as_deref(),
            Some(" 10px")
        );
        assert_eq!(replaced("var(--b, a, b)", &known).as_deref(), Some(" a, b"));
        assert_eq!(replaced("var(--b,)", &known).as_deref(), Some(""));
        assert_eq!(replaced("var(--empty)", &known).as_deref(), Some(""));
        assert_eq!(
            replaced("calc(var(--a) * 2)", &known).as_deref(),
            Some("calc(10px * 2)")
        );
        // Names are case-sensitive, and text inside a string is not a reference.
        assert_eq!(replaced("var(--A, x)", &known).as_deref(), Some(" x"));
        assert_eq!(
            replaced("\"var(--a)\"", &known).as_deref(),
            Some("\"var(--a)\"")
        );
        // No value and no fallback, or an invalid value: invalid.
        for bad in [
            "var(--b)",
            "var(--bad)",
            "var()",
            "var(a)",
            "var(--a",
            "var(--a b)",
            "var(,x)",
        ] {
            assert_eq!(replaced(bad, &known), None, "{bad}");
        }
        // The inserted text does not join its neighbours into one token.
        let mut work = 100_000;
        let raw = substitute("var(--a)px", &|_| Some("1".to_owned()), &mut work).unwrap();
        assert_eq!(raw, "/**/1/**/px");
    }

    #[test]
    fn computed_values_inherit_override_and_reject_cycles() {
        let inherited = values(&[("--a", Some("1px")), ("--b", Some("2px"))]);
        let mut work = 100_000;
        let clean = |map: CustomValues| -> Vec<(String, Option<String>)> {
            map.into_iter()
                .map(|(name, value)| (name, value.map(|text| text.replace("/**/", ""))))
                .collect()
        };
        let got = clean(compute(
            &inherited,
            &[
                ("--b", "var(--a) var(--c)"),
                ("--c", "3px"),
                ("--d", "var(--nope, 4px)"),
            ],
            &mut work,
        ));
        assert_eq!(
            got,
            clean(values(&[
                ("--a", Some("1px")),
                ("--b", Some("1px 3px")),
                ("--c", Some("3px")),
                ("--d", Some(" 4px")),
            ]))
        );
        // A property that reaches itself is invalid, with everything in the cycle; one
        // that only refers to the cycle is invalid too, and takes a fallback if it has one.
        let got = compute(
            &inherited,
            &[
                ("--a", "var(--a)"),
                ("--x", "var(--y)"),
                ("--y", "var(--x)"),
                ("--z", "var(--x, 9px)"),
            ],
            &mut work,
        );
        assert_eq!(got["--a"], None);
        assert_eq!(got["--x"], None);
        assert_eq!(got["--y"], None);
        assert_eq!(
            got["--z"].as_deref().map(|t| t.replace("/**/", "")),
            Some(" 9px".to_owned())
        );
        assert_eq!(got["--b"].as_deref(), Some("2px"));
    }
}
