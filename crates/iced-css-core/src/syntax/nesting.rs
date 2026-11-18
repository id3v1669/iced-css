//! CSS Nesting 1, by rewriting: a style rule's nested rules become rules of their own after
//! it, their selectors joined to its own, and a conditional group nested in a style rule
//! goes around a copy of it. A sheet with nothing nested comes back as it was.
//!
//! Joining is textual: `&` stands for each parent selector in turn, and a nested selector
//! without one is the parent's descendant (or `>`, `+` or `~` sibling, where it begins
//! with that combinator). A nested selector list makes one rule per pair, so a rule
//! weighs as its own pair does, not as `:is()` of the parents would.
use super::{TokenKind, scan_token_budget};
use std::borrow::Cow;

/// The most rules one flattening may write: a guard against lists multiplying.
const RULE_LIMIT: usize = 100_000;
/// The deepest nesting it follows; deeper, the sheet is left as written.
const DEPTH_LIMIT: usize = 64;
/// The most it may write: so many times the sheet, and never past this many bytes. A
/// selector list multiplied past it leaves the sheet as written.
const GROWTH: usize = 16;
const OUTPUT_LIMIT: usize = 4_000_000;

/// One piece of a block: a run of text with no block in it, or a prelude and its block.
enum Item<'a> {
    Text(&'a str),
    Block { prelude: &'a str, body: &'a str },
}

/// The items of `text`, split at its top-level blocks; `None` where it cannot be read
/// (an unclosed block is left as it is).
fn items(text: &str) -> Option<Vec<Item<'_>>> {
    // The scanner charges per step it takes; this only guards against a runaway.
    let mut work = text.len().saturating_mul(256).saturating_add(4096);
    let mut found = Vec::new();
    let mut offset = 0;
    let mut start = 0;
    let mut depth = 0usize;
    let mut open = 0;
    while offset < text.len() {
        let token = scan_token_budget(&text[offset..], &mut work).ok()??;
        let at = offset;
        offset += token.raw.len();
        match token.kind {
            TokenKind::OpenBrace => {
                if depth == 0 {
                    open = at;
                }
                depth += 1;
            }
            TokenKind::CloseBrace if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    // What came before the prelude, up to the last `;` or block, is text.
                    let before = &text[start..open];
                    let cut = before.rfind([';', '}']).map_or(0, |at| at + 1);
                    if cut > 0 {
                        found.push(Item::Text(&before[..cut]));
                    }
                    found.push(Item::Block {
                        prelude: &before[cut..],
                        body: &text[open + 1..at],
                    });
                    start = offset;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    if start < text.len() {
        found.push(Item::Text(&text[start..]));
    }
    Some(found)
}

/// The tokens of a selector, as (start, end, kind, raw); `None` where it cannot be read.
fn tokens(text: &str) -> Option<Vec<(usize, TokenKind, &str)>> {
    let mut work = text.len().saturating_mul(256).saturating_add(4096);
    let mut found = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let token = scan_token_budget(&text[offset..], &mut work).ok()??;
        found.push((offset, token.kind, token.raw));
        offset += token.raw.len();
    }
    Some(found)
}

/// Splits a selector list at its top-level commas: not those in brackets, strings or
/// comments.
fn selectors(prelude: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, kind, raw) in tokens(prelude).unwrap_or_default() {
        match kind {
            TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::Function => depth += 1,
            TokenKind::CloseParen | TokenKind::CloseBracket => {
                depth = depth.saturating_sub(1);
            }
            TokenKind::Delim if depth == 0 && raw == "," => {
                parts.push(prelude[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(prelude[start..].trim().to_owned());
    parts.retain(|part| !part.is_empty());
    parts
}

/// A nested selector joined to its parent: each nesting selector `&` (a delimiter token,
/// not one in a string or comment) stands for the parent.
fn join(parent: &str, nested: &str) -> String {
    let found = tokens(nested).unwrap_or_default();
    if !found
        .iter()
        .any(|(_, kind, raw)| *kind == TokenKind::Delim && *raw == "&")
    {
        return format!("{parent} {nested}");
    }
    let mut out = String::with_capacity(nested.len() + parent.len());
    for (_, kind, raw) in found {
        if kind == TokenKind::Delim && raw == "&" {
            out.push_str(parent);
        } else {
            out.push_str(raw);
        }
    }
    out
}

/// Whether the prelude opens a conditional group, whose block holds rules.
fn is_group(prelude: &str) -> bool {
    let name = prelude.trim_start().to_ascii_lowercase();
    name.starts_with("@media") || name.starts_with("@supports")
}

struct Flattener {
    out: String,
    rules: usize,
    nested: bool,
    /// How deep it is, and the most it may write.
    depth: usize,
    limit: usize,
}

impl Flattener {
    /// A list of rules, at the top level or in a group, with `parents` the selectors of
    /// the style rule they are nested in (none at the top level).
    fn rules(&mut self, text: &str, parents: &[String]) -> Option<()> {
        self.depth += 1;
        let done = self.rules_in(text, parents);
        self.depth -= 1;
        done
    }

    fn rules_in(&mut self, text: &str, parents: &[String]) -> Option<()> {
        if self.depth > DEPTH_LIMIT || self.out.len() > self.limit {
            return None;
        }
        for item in items(text)? {
            match item {
                Item::Text(text) => self.out.push_str(text),
                Item::Block { prelude, body } if prelude.trim_start().starts_with('@') => {
                    if is_group(prelude) {
                        self.out.push_str(prelude);
                        self.out.push_str("{\n");
                        // A group's own declarations, nested in a style rule, are that rule's.
                        if parents.is_empty() {
                            self.rules(body, parents)?;
                        } else {
                            self.style(&parents.join(", "), body, parents)?;
                        }
                        self.out.push('}');
                    } else {
                        self.out.push_str(prelude);
                        self.out.push('{');
                        self.out.push_str(body);
                        self.out.push('}');
                    }
                }
                Item::Block { prelude, body } => {
                    let own = selectors(prelude);
                    let joined: Vec<String> = if parents.is_empty() {
                        own
                    } else {
                        self.nested = true;
                        // Lists multiply: stop before what they make outgrows the limit.
                        let size: usize = parents
                            .iter()
                            .map(String::len)
                            .sum::<usize>()
                            .saturating_mul(own.len())
                            .saturating_add(
                                own.iter()
                                    .map(String::len)
                                    .sum::<usize>()
                                    .saturating_mul(parents.len()),
                            );
                        if self.out.len().saturating_add(size) > self.limit {
                            return None;
                        }
                        parents
                            .iter()
                            .flat_map(|parent| own.iter().map(move |nested| join(parent, nested)))
                            .collect()
                    };
                    self.style(&joined.join(", "), body, &joined)?;
                }
            }
        }
        Some(())
    }

    /// A style rule: its own declarations under `selector`, and its nested rules where
    /// they are written. Declarations after a nested rule stay after it, as a rule of their
    /// own under `selector` (CSS Nesting 1, nested declarations rules).
    fn style(&mut self, selector: &str, body: &str, selectors: &[String]) -> Option<()> {
        self.rules += 1;
        if self.rules > RULE_LIMIT || self.out.len() > self.limit {
            return None;
        }
        let parts = items(body)?;
        let mut first = true;
        let mut run = String::new();
        let mut parts = parts.into_iter().peekable();
        while let Some(part) = parts.next() {
            match part {
                Item::Text(text) => {
                    run.push_str(text);
                    if !matches!(parts.peek(), Some(Item::Text(_))) {
                        self.declarations(selector, &std::mem::take(&mut run), &mut first);
                    }
                }
                Item::Block { prelude, body } => {
                    // The rule's own declarations come first even when it has none.
                    self.declarations(selector, "", &mut first);
                    self.nested = true;
                    let block = format!("{prelude}{{{body}}}");
                    self.rules(&block, selectors)?;
                }
            }
        }
        self.declarations(selector, "", &mut first);
        Some(())
    }

    /// Writes a run of declarations under `selector`: the rule's first always, later
    /// ones where they hold something.
    fn declarations(&mut self, selector: &str, run: &str, first: &mut bool) {
        if !*first && run.trim().is_empty() {
            return;
        }
        *first = false;
        self.out.push_str(selector);
        self.out.push('{');
        self.out.push_str(run);
        self.out.push_str("}\n");
    }
}

/// `css` with its nested rules written out as rules of their own (CSS Nesting 1).
pub fn flatten_nesting(css: &str) -> Cow<'_, str> {
    if !css.contains('{') {
        return Cow::Borrowed(css);
    }
    let mut flattener = Flattener {
        out: String::with_capacity(css.len()),
        rules: 0,
        nested: false,
        depth: 0,
        limit: css
            .len()
            .saturating_mul(GROWTH)
            .clamp(1 << 16, OUTPUT_LIMIT),
    };
    match flattener.rules(css, &[]) {
        Some(()) if flattener.nested => Cow::Owned(flattener.out),
        _ => Cow::Borrowed(css),
    }
}

#[cfg(test)]
mod tests {
    use super::flatten_nesting;

    #[test]
    fn declarations_after_a_nested_rule_stay_after_it() {
        assert_eq!(
            flat(".a { & { color: red } color: blue; }"),
            ".a{} .a{ color: red } .a{ color: blue; }"
        );
    }

    #[test]
    fn only_nesting_selector_tokens_stand_for_the_parent() {
        assert_eq!(
            flat(".a { [title=\"&\"] { color: red } }"),
            ".a{} .a [title=\"&\"]{ color: red }"
        );
        // A comma in a string does not split the list.
        assert_eq!(
            flat(".a { [title=\"x,y\"] & { color: red } }"),
            ".a{} [title=\"x,y\"] .a{ color: red }"
        );
    }

    fn flat(css: &str) -> String {
        flatten_nesting(css)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn nested_rules_join_their_parent_selectors() {
        assert_eq!(
            flat(".a { color: red; .b { width: 1px } & > .c { height: 2px } }"),
            ".a{ color: red;} .a .b{ width: 1px } .a > .c{ height: 2px }"
        );
        assert_eq!(
            flat(".a, .b { &:hover, .c & { color: red } }"),
            ".a, .b{} .a:hover, .c .a, .b:hover, .c .b{ color: red }"
        );
        // Deeper, and a combinator to begin with.
        assert_eq!(
            flat("#x { .y { > .z { color: red } } }"),
            "#x{} #x .y{} #x .y > .z{ color: red }"
        );
    }

    #[test]
    fn a_nested_group_goes_around_its_parent() {
        assert_eq!(
            flat(".a { color: red; @media (max-width: 10px) { color: blue; .b { width: 1px } } }"),
            ".a{ color: red;} @media (max-width: 10px) { .a{ color: blue;} .a .b{ width: 1px } }"
        );
    }

    #[test]
    fn a_sheet_with_nothing_nested_is_returned_as_it_is() {
        for css in [
            ".a { color: red } @media screen { .b { width: 1px } }",
            "a { content: \"{ .b { } }\" }",
            "/* .a { .b { } } */ .c { width: 1px }",
            ".a { width: 1px",
        ] {
            assert!(
                matches!(flatten_nesting(css), std::borrow::Cow::Borrowed(_)),
                "{css}"
            );
        }
        // Too deep, or multiplying past its limit: left as written.
        let deep = format!(
            "{}{}",
            ".a { @media screen { ".repeat(3000),
            "} }".repeat(3000)
        );
        assert!(matches!(
            flatten_nesting(&deep),
            std::borrow::Cow::Borrowed(_)
        ));
        let list = ".a,.b,.c,.d,.e,.f,.g,.h,.i,.j";
        let many = format!(
            "{list} {{ {} }}",
            format!("& {list} {{ ").repeat(8) + &"}".repeat(8)
        );
        assert!(matches!(
            flatten_nesting(&many),
            std::borrow::Cow::Borrowed(_)
        ));
        // Text inside strings and comments is not read as rules.
        assert_eq!(
            flat(".a { content: \"}\"; .b { x: 1 } }"),
            ".a{ content: \"}\";} .a .b{ x: 1 }"
        );
    }
}
