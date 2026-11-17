use crate::source::Metadata;
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    Present,
    Equal,
    Includes,
    DashMatch,
    /// Selectors Level 3: `^=`, the value begins with it.
    Prefix,
    /// Selectors Level 3: `$=`, the value ends with it.
    Suffix,
    /// Selectors Level 3: `*=`, the value contains it.
    Substring,
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
    /// Selectors Level 3 `:root`.
    Root,
    /// Selectors Level 3 `:empty`.
    Empty,
    /// Selectors Level 3 `:nth-child(an+b)` and its family: counted from the end for the
    /// `last` forms, among siblings of the same type for the `of-type` forms. `:last-child`,
    /// `:first-of-type` and `:last-of-type` are the `0n+1` cases.
    Nth {
        a: i32,
        b: i32,
        last: bool,
        of_type: bool,
    },
    /// Selectors Level 3 `:only-child` and `:only-of-type`.
    Only {
        of_type: bool,
    },
    /// Selectors Level 3 `:not()`, holding one simple selector that is not itself a
    /// negation or a pseudo-element.
    Not(Box<SimpleSelector>),
    /// Selectors Level 4 `:checked`, `:disabled` and `:enabled`, from the state a captured
    /// control records as attributes: `checked` and `disabled` present, and for `:enabled`
    /// a control (the internal marker) without `disabled`.
    Checked,
    Disabled,
    Enabled,
    /// Selectors Level 4 `:is()`, `:where()` and a `:not()` of more than one simple
    /// selector: whether the element matches any of the compound selectors (none, for
    /// `negated`). `specificity` is what it adds: its most specific argument's, or none for
    /// `:where()`.
    Matches {
        list: Vec<Vec<SimpleSelector>>,
        negated: bool,
        specificity: crate::cascade::Specificity,
    },
}
impl SimpleSelector {
    /// Whether testing it needs the element's place among its siblings.
    pub(crate) fn needs_position(&self) -> bool {
        match self {
            Self::Nth { .. } | Self::Only { .. } => true,
            Self::Not(inner) => inner.needs_position(),
            Self::Matches { list, .. } => list.iter().flatten().any(Self::needs_position),
            _ => false,
        }
    }
}
/// An element's place among its sibling elements, each counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingPosition {
    pub index: usize,
    pub from_end: usize,
    /// Among the siblings of its own type.
    pub type_index: usize,
    pub type_from_end: usize,
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
    /// No child elements and no text of its own.
    pub empty: bool,
    /// Filled in by the matcher when a structural pseudo-class asks for it; `None` matches
    /// none of them.
    pub position: Option<SiblingPosition>,
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
    // Reserve a full scan even for early hits. Unequal key lengths short-circuit;
    // equal lengths compare at most name_bytes bytes, plus a visit/length check.
    fn metadata_lookup(&mut self, entries: usize, name_bytes: usize) -> Result<(), MatchLimit> {
        let cost = name_bytes
            .checked_add(2)
            .and_then(|per_entry| entries.checked_mul(per_entry))
            .ok_or(MatchLimit)?;
        self.charge(cost)
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
/// String comparisons and full possible metadata name scans are charged before
/// lookup, even for early hits. Metadata construction/storage and ancestor
/// propagation belong to the source snapshot's separate limits.
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
            budget.metadata_lookup(element.metadata.attribute_count(), "id".len())?;
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
            budget.metadata_lookup(element.metadata.attribute_count(), "class".len())?;
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
            budget.metadata_lookup(element.metadata.attribute_count(), name.len())?;
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
                            // An empty value matches nothing (Selectors Level 3, 6.3.2).
                            AttributeOperator::Prefix => {
                                !value.is_empty() && actual.starts_with(value.as_str())
                            }
                            AttributeOperator::Suffix => {
                                !value.is_empty() && actual.ends_with(value.as_str())
                            }
                            AttributeOperator::Substring => {
                                !value.is_empty() && actual.contains(value.as_str())
                            }
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
        SimpleSelector::Root => !element.has_parent,
        SimpleSelector::Empty => element.empty,
        // Selectors Level 3 counts children of a parent: the root is no one's child.
        SimpleSelector::Nth {
            a,
            b,
            last,
            of_type,
        } => {
            element.has_parent
                && element.position.is_some_and(|position| {
                    let index = match (last, of_type) {
                        (false, false) => position.index,
                        (true, false) => position.from_end,
                        (false, true) => position.type_index,
                        (true, true) => position.type_from_end,
                    };
                    nth_matches(*a, *b, index)
                })
        }
        SimpleSelector::Only { of_type } => {
            element.has_parent
                && element.position.is_some_and(|position| {
                    if *of_type {
                        position.type_index == 1 && position.type_from_end == 1
                    } else {
                        position.index == 1 && position.from_end == 1
                    }
                })
        }
        SimpleSelector::Not(inner) => !matches_simple(inner, element, budget)?,
        SimpleSelector::Checked => element.metadata.attribute("checked").is_some(),
        SimpleSelector::Disabled => element.metadata.attribute("disabled").is_some(),
        SimpleSelector::Enabled => {
            element.metadata.attribute(CONTROL).is_some()
                && element.metadata.attribute("disabled").is_none()
        }
        SimpleSelector::Matches { list, negated, .. } => {
            let mut any = false;
            for compound in list {
                let mut all = true;
                for test in compound {
                    if !matches_simple(test, element, budget)? {
                        all = false;
                        break;
                    }
                }
                if all {
                    any = true;
                    break;
                }
            }
            any != *negated
        }
    })
}
/// The attribute a captured control that can be disabled carries, for `:enabled`. No
/// selector can name it: a selector's names hold no NUL.
pub const CONTROL: &str = "\u{0}control";
/// Whether `index` (from 1) is `a*n + b` for some `n >= 0`.
fn nth_matches(a: i32, b: i32, index: usize) -> bool {
    let Ok(index) = i64::try_from(index) else {
        return false;
    };
    let (a, offset) = (i64::from(a), index - i64::from(b));
    if a == 0 {
        offset == 0
    } else {
        offset % a == 0 && offset / a >= 0
    }
}
mod parse;
pub use parse::{
    Combinator, Compound, PseudoElement, Selector, SelectorLimits, parse_selector_list,
};
mod matching;
pub use matching::{
    MatchError, MatchLimits, SelectorSource, matches_selector, matching_specificity,
};

#[cfg(test)]
mod metadata_work_tests {
    use super::MatchBudget;
    use std::num::NonZeroUsize;

    #[test]
    fn lookup_arithmetic_overflow_preserves_remaining_work() {
        for (entries, name_bytes) in [(usize::MAX, 1), (1, usize::MAX)] {
            let mut budget = MatchBudget::new(NonZeroUsize::new(12).unwrap());
            assert!(budget.metadata_lookup(entries, name_bytes).is_err());
            assert_eq!(budget.remaining(), 12);
        }
    }

    #[test]
    fn lookup_reservation_has_exact_empty_and_exhaustion_boundaries() {
        let mut exact = MatchBudget::new(NonZeroUsize::new(12).unwrap());
        exact.metadata_lookup(0, 5).unwrap();
        assert_eq!(exact.remaining(), 12);
        exact.metadata_lookup(3, 2).unwrap();
        assert_eq!(exact.remaining(), 0);
        let mut short = MatchBudget::new(NonZeroUsize::new(11).unwrap());
        assert!(short.metadata_lookup(3, 2).is_err());
        assert_eq!(short.remaining(), 11);
    }
}
