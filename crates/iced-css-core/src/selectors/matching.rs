use super::{Combinator, ElementFacts, MatchBudget, PseudoElement, Selector, matches_simple};
use crate::cascade::Specificity;
use std::num::NonZeroUsize;
/// A read-only view of validated source-element relationships, not native layout.
pub trait SelectorSource {
    type Node: Copy + Eq;
    fn facts(&self, node: Self::Node) -> Option<ElementFacts<'_>>;
    fn parent(&self, node: Self::Node) -> Option<Self::Node>;
    fn previous_element(&self, node: Self::Node) -> Option<Self::Node>;
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
    ancestors: bool,
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
                ancestors: false,
            },
        )?;
        while let Some(frame) = stack.pop() {
            self.charge(1)?;
            if frame.ancestors {
                if let Some(parent) = self.parent(frame.node)? {
                    self.push(
                        &mut stack,
                        Frame {
                            node: parent,
                            ..frame
                        },
                    )?;
                }
                self.push(
                    &mut stack,
                    Frame {
                        ancestors: false,
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
            let mut matched = true;
            for test in &selector.compounds[frame.index].tests {
                if !matches_simple(test, facts, &mut self.budget).map_err(|_| MatchError::Work)? {
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
            let next = match relation {
                Combinator::Child | Combinator::Descendant => parent,
                Combinator::Adjacent => previous,
            };
            if let Some(node) = next {
                self.push(
                    &mut stack,
                    Frame {
                        index: frame.index - 1,
                        node,
                        ancestors: relation == Combinator::Descendant,
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
/// preserve alternative ancestors; no recursive Rust call stack is used.
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
