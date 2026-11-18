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

/// Whether a declaration's value holds a `var()` and so parses at computed-value time: a
/// `var(` function token, not that text in a string or comment.
pub fn has_reference(value: &str) -> bool {
    // Cheap first: most values have no `var(` text at all.
    if !value
        .as_bytes()
        .windows(4)
        .any(|window| window.eq_ignore_ascii_case(b"var("))
    {
        return false;
    }
    let mut work = value.len().saturating_mul(256).saturating_add(4096);
    let mut offset = 0;
    while offset < value.len() {
        let Ok(Some(token)) = scan_token_budget(&value[offset..], &mut work) else {
            // Past what it can read: as it reads to the parser, with `var(` in it.
            return true;
        };
        offset += token.raw.len();
        if token.kind == TokenKind::Function && token.raw.eq_ignore_ascii_case("var(") {
            return true;
        }
    }
    false
}

/// The custom property names a value's `var()` references name, fallbacks' included.
fn referenced<'a>(raw: &'a str, work: &mut usize) -> Vec<&'a str> {
    let mut names = Vec::new();
    let mut offset = 0;
    // After `var(`: waiting for its name, past white space.
    let mut expecting = false;
    let mut dash = None;
    while offset < raw.len() {
        let Some(Some(token)) = scan_token_budget(&raw[offset..], work).ok() else {
            break;
        };
        let at = offset;
        offset += token.raw.len();
        if token.kind == TokenKind::Function && token.raw.eq_ignore_ascii_case("var(") {
            expecting = true;
            dash = None;
            continue;
        }
        if !expecting || matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            continue;
        }
        // `--name`: a dash, then `-name` right after it.
        match dash {
            None if token.kind == TokenKind::Delim && token.raw == "-" => dash = Some(at),
            Some(from)
                if token.kind == TokenKind::Ident
                    && token.raw.starts_with('-')
                    && from + 1 == at =>
            {
                names.push(&raw[from..offset]);
                expecting = false;
                dash = None;
            }
            _ => {
                expecting = false;
                dash = None;
            }
        }
    }
    names
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
///
/// The CSS-wide keywords default it: `initial` is the guaranteed-invalid value, and
/// `inherit`, `unset` and `revert` (it inherits) take the parent's. Only a name in a
/// `var()` is a reference; the properties are resolved without recursion, however long a
/// chain of them is.
pub fn compute(inherited: &CustomValues, own: &[(&str, &str)], work: &mut usize) -> CustomValues {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Raw,
        Visiting,
        Done,
    }
    // Own declarations replace inherited values before anything is read: a reference to a
    // name declared here is to this element's value, never the inherited one.
    let mut values = inherited.clone();
    for (name, _) in own {
        values.remove(*name);
    }
    let mut state = vec![State::Raw; own.len()];
    // What each refers to among its own declarations, by place.
    let edges: Vec<Vec<usize>> = own
        .iter()
        .map(|(_, raw)| {
            referenced(raw, work)
                .into_iter()
                .filter_map(|name| own.iter().position(|(other, _)| *other == name))
                .collect()
        })
        .collect();
    // In a cycle of references (a strongly connected component of more than one, or one
    // that refers to itself): invalid whatever its fallbacks. Found before any is read,
    // so the order they are visited in does not matter.
    let cyclic = cycles(&edges);
    // The properties being resolved, outermost first, each with what it refers to and
    // how many of those it has gone through.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..own.len() {
        if state[start] != State::Raw {
            continue;
        }
        state[start] = State::Visiting;
        stack.push((start, 0));
        while let Some((at, next)) = stack.last_mut() {
            let at = *at;
            if let Some(&other) = edges[at].get(*next) {
                *next += 1;
                if state[other] == State::Raw {
                    state[other] = State::Visiting;
                    stack.push((other, 0));
                }
                continue;
            }
            // Everything it refers to is resolved: read it.
            let (name, raw) = own[at];
            let keyword = raw.trim();
            let computed = if keyword.eq_ignore_ascii_case("initial") {
                None
            } else if ["inherit", "unset", "revert", "revert-layer"]
                .iter()
                .any(|word| keyword.eq_ignore_ascii_case(word))
            {
                inherited.get(name).cloned().flatten()
            } else if cyclic[at] {
                None
            } else {
                let lookup = |name: &str| values.get(name).cloned().flatten();
                let mut out = String::new();
                substitute_at(raw, &lookup, work, 0, &mut out).map(|()| out)
            };
            values.insert(name.to_owned(), computed);
            state[at] = State::Done;
            stack.pop();
        }
    }
    values
}

/// Which of a graph's nodes lie on a cycle: Tarjan's strongly connected components,
/// without recursion.
fn cycles(edges: &[Vec<usize>]) -> Vec<bool> {
    let count = edges.len();
    let mut index = vec![usize::MAX; count];
    let mut low = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut cyclic = vec![false; count];
    let mut next_index = 0;
    for root in 0..count {
        if index[root] != usize::MAX {
            continue;
        }
        // (node, how many of its edges it has gone through)
        let mut calls: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next_index;
        low[root] = next_index;
        next_index += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (node, ref mut at)) = calls.last_mut() {
            if let Some(&other) = edges[node].get(*at) {
                *at += 1;
                if index[other] == usize::MAX {
                    index[other] = next_index;
                    low[other] = next_index;
                    next_index += 1;
                    stack.push(other);
                    on_stack[other] = true;
                    calls.push((other, 0));
                } else if on_stack[other] {
                    low[node] = low[node].min(index[other]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut members = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    members.push(member);
                    if member == node {
                        break;
                    }
                }
                let looped = members.len() > 1 || edges[node].contains(&node);
                for member in members {
                    cyclic[member] = looped;
                }
            }
        }
    }
    cyclic
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

    #[test]
    fn only_var_names_are_references_and_keywords_default() {
        let inherited = values(&[("--p", Some("5px"))]);
        let mut work = 10_000_000;
        // A bare `--name` is a value, not a reference: no cycle.
        let got = compute(&inherited, &[("--name", "--name")], &mut work);
        assert_eq!(got["--name"].as_deref(), Some("--name"));
        // `initial` is the guaranteed-invalid value; `inherit` and `unset` the parent's.
        let got = compute(
            &inherited,
            &[("--w", "initial"), ("--p", "inherit"), ("--q", " unset ")],
            &mut work,
        );
        assert_eq!(got["--w"], None);
        assert_eq!(got["--p"].as_deref(), Some("5px"));
        assert_eq!(got["--q"], None, "the parent has none");
        // A long chain resolves without recursion: each refers to the next.
        let names: Vec<String> = (0..3000).map(|at| format!("--c{at}")).collect();
        let raws: Vec<String> = (0..3000)
            .map(|at| {
                if at == 2999 {
                    "1px".to_owned()
                } else {
                    format!("var(--c{})", at + 1)
                }
            })
            .collect();
        let got = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let own: Vec<(&str, &str)> = names
                    .iter()
                    .zip(&raws)
                    .map(|(name, raw)| (name.as_str(), raw.as_str()))
                    .collect();
                let mut work = 100_000_000;
                compute(&CustomValues::new(), &own, &mut work)["--c0"].clone()
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(
            got.map(|text| text.replace("/**/", "")).as_deref(),
            Some("1px")
        );
    }

    #[test]
    fn overlapping_cycles_are_invalid_in_any_order() {
        let declarations = [
            ("--a", "var(--b, 1px) var(--c, 1px)"),
            ("--b", "var(--a, 2px)"),
            ("--c", "var(--b, 3px)"),
        ];
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let own: Vec<(&str, &str)> = order.iter().map(|at| declarations[*at]).collect();
            let mut work = 100_000;
            let got = compute(&CustomValues::new(), &own, &mut work);
            for name in ["--a", "--b", "--c"] {
                assert_eq!(got[name], None, "{name} in {order:?}");
            }
        }
    }

    #[test]
    fn var_text_in_strings_and_comments_is_no_reference() {
        assert!(!has_reference("\"var(--x)\""));
        assert!(!has_reference("red /* var( */"));
        assert!(has_reference("VAR(--x)"));
        assert!(has_reference("1px var(--x, \"a\")"));
    }
}
