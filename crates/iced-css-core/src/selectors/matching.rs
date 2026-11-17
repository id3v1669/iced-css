use super::{
    Combinator, ElementFacts, MatchBudget, PseudoElement, Selector, SiblingPosition,
    SimpleSelector, matches_simple,
};
use crate::cascade::Specificity;
use std::num::NonZeroUsize;
/// A read-only view of validated source-element relationships, not native layout.
/// The relationships must form a tree: an element and its previous element share a parent.
pub trait SelectorSource {
    type Node: Copy + Eq;
    fn facts(&self, node: Self::Node) -> Option<ElementFacts<'_>>;
    fn parent(&self, node: Self::Node) -> Option<Self::Node>;
    fn previous_element(&self, node: Self::Node) -> Option<Self::Node>;
    /// The sibling element after it: what `previous_element` of that one returns it for.
    fn next_element(&self, node: Self::Node) -> Option<Self::Node>;
    /// Its last child element, from which `previous_element` walks the others. A source
    /// that does not know its children has none, and `:has()` matches nothing in it.
    fn last_child(&self, _node: Self::Node) -> Option<Self::Node> {
        None
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MatchLimits {
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchError {
    Work,
    Bytes,
    Allocation,
    InvalidSource,
    InvalidSelector,
}

impl std::fmt::Display for MatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Work => "selector matching work exhausted",
            Self::Bytes => "selector matching stack byte limit",
            Self::Allocation => "selector matching allocation failed",
            Self::InvalidSource => "selector source node missing",
            Self::InvalidSelector => "invalid selector structure",
        })
    }
}
impl std::error::Error for MatchError {}
#[derive(Clone, Copy)]
struct Frame<N> {
    index: usize,
    node: N,
    walk: Walk,
}
/// Whether a frame tries its node, or first passes the search on to the next candidate a
/// descendant or general sibling combinator allows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    Here,
    Ancestors,
    Siblings,
}
struct Engine<'a, S: SelectorSource> {
    source: &'a S,
    budget: MatchBudget,
    bytes: usize,
}
impl<S: SelectorSource> Engine<'_, S> {
    fn charge(&mut self, amount: usize) -> Result<(), MatchError> {
        self.budget.charge(amount).map_err(|_| MatchError::Work)
    }
    fn parent(&mut self, node: S::Node) -> Result<Option<S::Node>, MatchError> {
        self.charge(1)?;
        Ok(self.source.parent(node))
    }
    fn previous(&mut self, node: S::Node) -> Result<Option<S::Node>, MatchError> {
        self.charge(1)?;
        Ok(self.source.previous_element(node))
    }
    /// Counts the sibling elements on both sides, all of them and those of `type_name`.
    /// One step is charged per sibling.
    fn position(&mut self, node: S::Node, type_name: &str) -> Result<SiblingPosition, MatchError> {
        let mut position = SiblingPosition {
            index: 1,
            from_end: 1,
            type_index: 1,
            type_from_end: 1,
        };
        for forward in [false, true] {
            let mut cursor = node;
            loop {
                self.charge(1)?;
                let next = if forward {
                    self.source.next_element(cursor)
                } else {
                    self.source.previous_element(cursor)
                };
                let Some(sibling) = next else {
                    break;
                };
                let same = self
                    .source
                    .facts(sibling)
                    .ok_or(MatchError::InvalidSource)?
                    .type_name
                    == type_name;
                let (all, typed) = if forward {
                    (&mut position.from_end, &mut position.type_from_end)
                } else {
                    (&mut position.index, &mut position.type_index)
                };
                *all += 1;
                *typed += usize::from(same);
                cursor = sibling;
            }
        }
        Ok(position)
    }
    fn push(
        &mut self,
        stack: &mut Vec<Frame<S::Node>>,
        frame: Frame<S::Node>,
    ) -> Result<(), MatchError> {
        if stack.len() == stack.capacity() {
            let max = self.bytes / size_of::<Frame<S::Node>>();
            let needed = stack
                .len()
                .checked_add(1)
                .filter(|needed| *needed <= max)
                .ok_or(MatchError::Bytes)?;
            let target = needed.max(stack.capacity().saturating_mul(2).min(max));
            self.charge(stack.len())?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| MatchError::Allocation)?;
            if next.capacity() > max {
                return Err(MatchError::Bytes);
            }
            next.extend_from_slice(stack);
            *stack = next;
        }
        stack.push(frame);
        Ok(())
    }
    /// Whether one compound selector (with no `:has()` in it) holds for `node`.
    fn compound_holds(
        &mut self,
        node: S::Node,
        tests: &[SimpleSelector],
    ) -> Result<bool, MatchError> {
        let parent = self.parent(node)?;
        let previous = self.previous(node)?;
        self.charge(1)?;
        let mut facts = self.source.facts(node).ok_or(MatchError::InvalidSource)?;
        facts.has_parent = parent.is_some();
        facts.has_previous_element = previous.is_some();
        if tests.iter().any(SimpleSelector::needs_position) {
            facts.position = Some(self.position(node, facts.type_name)?);
        }
        for test in tests {
            if !matches_simple(test, facts, &mut self.budget).map_err(|_| MatchError::Work)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Selectors Level 4 `:has()`: whether some element in the relation each relative
    /// selector names to `node` matches it: a descendant, a child, the next sibling or a
    /// later one.
    fn has(
        &mut self,
        node: S::Node,
        relatives: &[(Combinator, Vec<SimpleSelector>)],
    ) -> Result<bool, MatchError> {
        for (combinator, tests) in relatives {
            let mut candidates: Vec<S::Node> = Vec::new();
            match combinator {
                Combinator::Descendant | Combinator::Child => {
                    let mut child = self.source.last_child(node);
                    while let Some(at) = child {
                        self.charge(1)?;
                        candidates.push(at);
                        child = self.source.previous_element(at);
                    }
                }
                Combinator::Adjacent | Combinator::Sibling => {
                    let mut sibling = self.source.next_element(node);
                    while let Some(at) = sibling {
                        self.charge(1)?;
                        candidates.push(at);
                        if *combinator == Combinator::Adjacent {
                            break;
                        }
                        sibling = self.source.next_element(at);
                    }
                }
            }
            while let Some(candidate) = candidates.pop() {
                self.charge(1)?;
                if self.compound_holds(candidate, tests)? {
                    return Ok(true);
                }
                // A descendant may be deeper: its own children follow.
                if *combinator == Combinator::Descendant {
                    let mut child = self.source.last_child(candidate);
                    while let Some(at) = child {
                        self.charge(1)?;
                        candidates.push(at);
                        child = self.source.previous_element(at);
                    }
                }
            }
        }
        Ok(false)
    }
    fn matches(
        &mut self,
        selector: &Selector,
        subject: S::Node,
        target: Option<PseudoElement>,
    ) -> Result<bool, MatchError> {
        self.charge(selector.compounds.len())?;
        if selector.compounds.is_empty()
            || selector.combinators.len() != selector.compounds.len() - 1
            || selector
                .compounds
                .iter()
                .any(|compound| compound.tests.is_empty())
        {
            return Err(MatchError::InvalidSelector);
        }
        if target != selector.pseudo {
            return Ok(false);
        }
        let mut stack = Vec::new();
        self.push(
            &mut stack,
            Frame {
                index: selector.compounds.len() - 1,
                node: subject,
                walk: Walk::Here,
            },
        )?;
        // The lowest compound whose ancestor walk has begun. Reaching a compound passes every
        // one after it, so each descendant compound from here on has had its walk. A walk tries
        // every ancestor of where it starts, and a later one could only start further up, at
        // one of those: it has nothing new to try, so matching is linear in the depth.
        let mut walked = selector.compounds.len();
        while let Some(frame) = stack.pop() {
            self.charge(1)?;
            if frame.walk != Walk::Here {
                let further = if frame.walk == Walk::Ancestors {
                    self.parent(frame.node)?
                } else {
                    self.previous(frame.node)?
                };
                if let Some(node) = further {
                    self.push(&mut stack, Frame { node, ..frame })?;
                }
                self.push(
                    &mut stack,
                    Frame {
                        walk: Walk::Here,
                        ..frame
                    },
                )?;
                continue;
            }
            let parent = self.parent(frame.node)?;
            let previous = self.previous(frame.node)?;
            self.charge(1)?;
            let mut facts = self
                .source
                .facts(frame.node)
                .ok_or(MatchError::InvalidSource)?;
            facts.has_parent = parent.is_some();
            facts.has_previous_element = previous.is_some();
            if selector.compounds[frame.index]
                .tests
                .iter()
                .any(|test| test.needs_position())
            {
                facts.position = Some(self.position(frame.node, facts.type_name)?);
            }
            let mut matched = true;
            for test in &selector.compounds[frame.index].tests {
                let holds = if let SimpleSelector::Has { relatives, .. } = test {
                    self.has(frame.node, relatives)?
                } else {
                    matches_simple(test, facts, &mut self.budget).map_err(|_| MatchError::Work)?
                };
                if !holds {
                    matched = false;
                    break;
                }
            }
            if !matched {
                continue;
            }
            if frame.index == 0 {
                return Ok(true);
            }
            let relation = selector.combinators[frame.index - 1];
            if relation == Combinator::Descendant {
                if frame.index - 1 >= walked {
                    continue;
                }
                walked = frame.index - 1;
            }
            let next = match relation {
                Combinator::Child | Combinator::Descendant => parent,
                Combinator::Adjacent | Combinator::Sibling => previous,
            };
            if let Some(node) = next {
                self.push(
                    &mut stack,
                    Frame {
                        index: frame.index - 1,
                        node,
                        walk: match relation {
                            Combinator::Descendant => Walk::Ancestors,
                            Combinator::Sibling => Walk::Siblings,
                            Combinator::Child | Combinator::Adjacent => Walk::Here,
                        },
                    },
                )?;
            }
        }
        Ok(false)
    }
}
fn engine<S: SelectorSource>(source: &S, limits: MatchLimits) -> Result<Engine<'_, S>, MatchError> {
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(MatchError::Bytes);
    }
    Ok(Engine {
        source,
        budget: MatchBudget::new(limits.work),
        bytes: limits.retained_bytes.get(),
    })
}
/// Match a parsed selector from its subject backwards. Descendant continuations
/// preserve alternative ancestors, each compound walking them once; no recursive
/// Rust call stack is used.
/// Limits cover traversal/comparison work and retained backtracking frames, not
/// the caller-owned AST/source snapshot or work internal to a source view method.
pub fn matches_selector<S: SelectorSource>(
    selector: &Selector,
    source: &S,
    subject: S::Node,
    target: Option<PseudoElement>,
    limits: MatchLimits,
) -> Result<bool, MatchError> {
    engine(source, limits)?.matches(selector, subject, target)
}
/// Return the largest specificity among matching group members, sharing one work
/// allowance across the group. Unmatched members contribute no specificity.
pub fn matching_specificity<S: SelectorSource>(
    selectors: &[Selector],
    source: &S,
    subject: S::Node,
    target: Option<PseudoElement>,
    limits: MatchLimits,
) -> Result<Option<Specificity>, MatchError> {
    if selectors.is_empty() {
        return Err(MatchError::InvalidSelector);
    }
    let mut engine = engine(source, limits)?;
    let mut winner = None;
    for selector in selectors {
        engine.charge(1)?;
        if engine.matches(selector, subject, target)? {
            winner = Some(winner.map_or(selector.specificity, |current: Specificity| {
                current.max(selector.specificity)
            }));
        }
    }
    Ok(winner)
}
