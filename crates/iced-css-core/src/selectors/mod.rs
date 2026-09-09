use crate::source::Metadata;
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    Present,
    Equal,
    Includes,
    DashMatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    Universal,
    Type(String),
    Id(String),
    Class(String),
    Attribute {
        name: String,
        operator: AttributeOperator,
        value: String,
    },
    FirstChild,
    Link,
    Visited,
    Hover,
    Active,
    Focus,
    Lang(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    NotLink,
    Unvisited,
    Visited,
}
#[derive(Debug, Clone, Copy)]
pub struct ElementFacts<'a> {
    pub type_name: &'a str,
    pub metadata: &'a Metadata,
    pub has_parent: bool,
    pub has_previous_element: bool,
    pub language: Option<&'a str>,
    pub link: LinkState,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchLimit;
pub struct MatchBudget {
    remaining: usize,
}
impl MatchBudget {
    pub fn new(work: NonZeroUsize) -> Self {
        Self {
            remaining: work.get(),
        }
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
}

impl std::fmt::Display for MatchLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("selector comparison work budget exhausted")
    }
}
impl std::error::Error for MatchLimit {}
impl MatchBudget {
    fn charge(&mut self, amount: usize) -> Result<(), MatchLimit> {
        self.remaining = self.remaining.checked_sub(amount).ok_or(MatchLimit)?;
        Ok(())
    }
    fn strings(&mut self, strings: &[&str]) -> Result<(), MatchLimit> {
        let amount = strings
            .iter()
            .try_fold(0usize, |total, text| {
                total
                    .checked_add(text.len())
                    .and_then(|total| total.checked_add(1))
            })
            .ok_or(MatchLimit)?;
        self.charge(amount)
    }
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}
fn hyphen_match(actual: &str, expected: &str, ignore_ascii_case: bool) -> bool {
    let equal = |a: &str, b: &str| {
        if ignore_ascii_case {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    equal(actual, expected)
        || (actual
            .get(..expected.len())
            .is_some_and(|prefix| equal(prefix, expected))
            && actual.as_bytes().get(expected.len()) == Some(&b'-'))
}
/// Evaluate a single selector test against already-established source facts.
/// String comparison work is charged before comparison. Metadata storage/lookup
/// and ancestor propagation belong to the source snapshot's separate limits.
/// This function does not infer parents, links, language or native state.
pub fn matches_simple(
    selector: &SimpleSelector,
    element: ElementFacts<'_>,
    budget: &mut MatchBudget,
) -> Result<bool, MatchLimit> {
    budget.charge(1)?;
    Ok(match selector {
        SimpleSelector::Universal => true,
        SimpleSelector::Type(name) => {
            budget.strings(&[name, element.type_name])?;
            name == element.type_name
        }
        SimpleSelector::Id(id) => {
            budget.strings(&[id])?;
            match element.metadata.css_id() {
                Some(actual) => {
                    budget.strings(&[actual])?;
                    actual == id
                }
                None => false,
            }
        }
        SimpleSelector::Class(class) => {
            budget.strings(&[class])?;
            match element.metadata.attribute("class") {
                Some(actual) => {
                    budget.strings(&[actual])?;
                    !class.is_empty()
                        && !class.chars().any(whitespace)
                        && actual.split(whitespace).any(|word| word == class)
                }
                None => false,
            }
        }
        SimpleSelector::Attribute {
            name,
            operator,
            value,
        } => {
            budget.strings(&[name])?;
            match element.metadata.attribute(name) {
                None => false,
                Some(actual) => match operator {
                    AttributeOperator::Present => true,
                    _ => {
                        budget.strings(&[actual, value])?;
                        match operator {
                            AttributeOperator::Equal => actual == value,
                            AttributeOperator::Includes => {
                                !value.is_empty()
                                    && !value.chars().any(whitespace)
                                    && actual.split(whitespace).any(|word| word == value)
                            }
                            AttributeOperator::DashMatch => hyphen_match(actual, value, false),
                            AttributeOperator::Present => unreachable!("presence handled above"),
                        }
                    }
                },
            }
        }
        SimpleSelector::FirstChild => element.has_parent && !element.has_previous_element,
        SimpleSelector::Link => element.link == LinkState::Unvisited,
        SimpleSelector::Visited => element.link == LinkState::Visited,
        SimpleSelector::Hover => element.hovered,
        SimpleSelector::Active => element.active,
        SimpleSelector::Focus => element.focused,
        SimpleSelector::Lang(language) => {
            budget.strings(&[language])?;
            match element.language {
                Some(actual) => {
                    budget.strings(&[actual])?;
                    !language.is_empty() && hyphen_match(actual, language, true)
                }
                None => false,
            }
        }
    })
}
mod parse;
pub use parse::{
    Combinator, Compound, PseudoElement, Selector, SelectorLimits, parse_selector_list,
};
mod matching;
pub use matching::{
    MatchError, MatchLimits, SelectorSource, matches_selector, matching_specificity,
};
