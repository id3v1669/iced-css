//! CSS22 12.4 counter traversal. Private placement shares bounded exact arithmetic;
//! the public interface belongs to formatting::counters.
use super::{Budget, Integer, Natural, NumericError, NumericLimits, counter_format};
use crate::values::{CounterError, CounterLimits, CounterStyle, property::CounterDirective};
use std::{borrow::Cow, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct CounterTraversalLimits {
    pub participants: NonZeroUsize,
    /// Maximum ancestor path length, including the root.
    pub depth: NonZeroUsize,
    pub scopes: NonZeroUsize,
    pub results: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    /// One cumulative work budget for the entire traversal and its formatting.
    pub numeric: NumericLimits,
}
#[derive(Debug, Clone, Copy)]
pub struct CounterRead<'a> {
    pub name: &'a str,
    /// None selects counter(); Some selects counters(), even for an empty separator.
    pub separator: Option<&'a str>,
    pub style: CounterStyle,
}
impl<'a> CounterRead<'a> {
    /// Borrow a decoded counter function without copying its name or separator.
    /// Other content items and keywords outside the selected CSS2 style set return None.
    pub fn from_content(item: &'a crate::values::property::ContentItem) -> Option<Self> {
        let crate::values::property::ContentItem::Counter {
            name,
            separator,
            style,
        } = item
        else {
            return None;
        };
        Some(Self {
            name,
            separator: separator.as_deref(),
            style: CounterStyle::parse(style.as_str())?,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CounterParticipant<'a> {
    pub parent: Option<usize>,
    /// False excludes this participant and its subtree. Hidden participants use true.
    pub participates: bool,
    pub resets: &'a [CounterDirective],
    pub increments: &'a [CounterDirective],
    pub reads: &'a [CounterRead<'a>],
}
#[derive(Debug, PartialEq, Eq)]
pub struct CounterText {
    pub participant: usize,
    pub read: usize,
    pub text: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterTraversalError {
    Order,
    Participants,
    Depth,
    Scopes,
    Results,
    Bytes,
    Numeric(NumericError),
    Formatting(CounterError),
}
impl From<NumericError> for CounterTraversalError {
    fn from(value: NumericError) -> Self {
        Self::Numeric(value)
    }
}
impl From<CounterError> for CounterTraversalError {
    fn from(value: CounterError) -> Self {
        Self::Formatting(value)
    }
}
impl std::fmt::Display for CounterTraversalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "counter traversal: {self:?}")
    }
}
impl std::error::Error for CounterTraversalError {}

struct Scope<'a> {
    name: &'a str,
    value: Cow<'a, Integer>,
    depth: usize,
}
impl Scope<'_> {
    fn owned_bytes(&self) -> usize {
        match &self.value {
            Cow::Borrowed(_) => 0,
            Cow::Owned(value) => value.magnitude.0.capacity() * size_of::<u32>(),
        }
    }
}
struct State<'a> {
    scopes: Vec<Scope<'a>>,
    bytes: usize,
    limits: CounterTraversalLimits,
    budget: Budget,
}
impl<'a> State<'a> {
    fn available(&self) -> usize {
        self.limits.retained_bytes.get() - self.bytes
    }
    fn lookup(&mut self, name: &str) -> Result<Option<usize>, CounterTraversalError> {
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            self.budget.spend(1)?;
            self.budget.spend(name.len().min(scope.name.len()))?;
            if scope.name == name {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
    fn push(&mut self, scope: Scope<'a>) -> Result<usize, CounterTraversalError> {
        if self.scopes.len() == self.limits.scopes.get() {
            return Err(CounterTraversalError::Scopes);
        }
        reserve(
            &mut self.scopes,
            &mut self.bytes,
            self.limits.retained_bytes.get(),
            &mut self.budget,
        )?;
        let index = self.scopes.len();
        self.scopes.push(scope);
        Ok(index)
    }
    fn ensure(&mut self, name: &'a str, depth: usize) -> Result<usize, CounterTraversalError> {
        if let Some(index) = self.lookup(name)? {
            return Ok(index);
        }
        self.push(Scope {
            name,
            depth,
            value: Cow::Owned(Integer {
                negative: false,
                magnitude: Natural(Vec::new()),
            }),
        })
    }
    fn reset(
        &mut self,
        directive: &'a CounterDirective,
        depth: usize,
    ) -> Result<(), CounterTraversalError> {
        self.budget.spend(1)?;
        directive
            .value
            .magnitude
            .check_digits(self.limits.numeric)?;
        if let Some(index) = self.lookup(&directive.name)?
            && self.scopes[index].depth == depth
        {
            self.bytes -= self.scopes[index].owned_bytes();
            self.scopes[index].value = Cow::Borrowed(&directive.value);
            return Ok(());
        }
        self.push(Scope {
            name: &directive.name,
            value: Cow::Borrowed(&directive.value),
            depth,
        })?;
        Ok(())
    }
    fn increment(
        &mut self,
        directive: &'a CounterDirective,
        depth: usize,
    ) -> Result<(), CounterTraversalError> {
        self.budget.spend(1)?;
        let index = self.ensure(&directive.name, depth)?;
        let old = &self.scopes[index];
        // This is the exact allocator request made by add_with_budget, not a
        // narrowed arithmetic value. Account the old result until addition succeeds.
        let capacity = old
            .value
            .magnitude
            .0
            .len()
            .max(directive.value.magnitude.0.len())
            .checked_add(1)
            .ok_or(CounterTraversalError::Bytes)?
            .min(self.limits.numeric.digits.get().div_ceil(9));
        if capacity
            .checked_mul(size_of::<u32>())
            .ok_or(CounterTraversalError::Bytes)?
            > self.available()
        {
            return Err(CounterTraversalError::Bytes);
        }
        let value =
            old.value
                .add_with_budget(&directive.value, self.limits.numeric, &mut self.budget)?;
        let bytes = value.magnitude.0.capacity() * size_of::<u32>();
        if bytes > self.available() {
            return Err(CounterTraversalError::Bytes);
        }
        self.bytes = self.bytes - old.owned_bytes() + bytes;
        self.scopes[index].value = Cow::Owned(value);
        Ok(())
    }
    fn text(
        &mut self,
        read: CounterRead<'a>,
        depth: usize,
    ) -> Result<String, CounterTraversalError> {
        let index = self.ensure(read.name, depth)?;
        let retained_bytes =
            NonZeroUsize::new(self.available()).ok_or(CounterTraversalError::Bytes)?;
        let limits = CounterLimits {
            numeric: self.limits.numeric,
            retained_bytes,
        };
        if let Some(separator) = read.separator {
            // Prepay all name comparisons; the iterator cannot hide unbounded work.
            for scope in &self.scopes {
                self.budget.spend(1)?;
                self.budget.spend(read.name.len().min(scope.name.len()))?;
            }
            Ok(counter_format::format_iter(
                self.scopes
                    .iter()
                    .filter(|scope| scope.name == read.name)
                    .map(|scope| scope.value.as_ref()),
                separator,
                read.style,
                limits,
                &mut self.budget,
            )?)
        } else {
            Ok(counter_format::format_iter(
                std::iter::once(self.scopes[index].value.as_ref()),
                "",
                read.style,
                limits,
                &mut self.budget,
            )?)
        }
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    bytes: &mut usize,
    limit: usize,
    budget: &mut Budget,
) -> Result<(), CounterTraversalError> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    let old = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(CounterTraversalError::Bytes)?;
    let available = (limit - (*bytes - old)) / size_of::<T>();
    let needed = values
        .len()
        .checked_add(1)
        .filter(|n| *n <= available)
        .ok_or(CounterTraversalError::Bytes)?;
    let target = needed.max(values.capacity().saturating_mul(2).min(available));
    budget.spend(
        values
            .len()
            .checked_mul(size_of::<T>())
            .ok_or(NumericError::WorkLimit)?,
    )?;
    let mut next = Vec::new();
    next.try_reserve_exact(target)
        .map_err(|_| NumericError::Allocation)?;
    if next.capacity() > available {
        return Err(CounterTraversalError::Bytes);
    }
    *bytes = *bytes - old + next.capacity() * size_of::<T>();
    next.append(values);
    *values = next;
    Ok(())
}

#[path = "generated_content.rs"]
pub(crate) mod generated;
struct Participant<'a> {
    parent: Option<usize>,
    participates: bool,
    resets: &'a [CounterDirective],
    increments: &'a [CounterDirective],
}
fn walk<'a, E: From<CounterTraversalError>>(
    participants: impl ExactSizeIterator<Item = Participant<'a>>,
    limits: CounterTraversalLimits,
    mut visit: impl FnMut(&mut State<'a>, usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    if participants.len() > limits.participants.get() {
        return Err(CounterTraversalError::Participants.into());
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(CounterTraversalError::Bytes.into());
    }
    let mut state = State {
        scopes: Vec::new(),
        bytes: 0,
        limits,
        budget: Budget::new(limits.numeric),
    };
    let mut path: Vec<(usize, bool)> = Vec::new();
    for (index, participant) in participants.enumerate() {
        state.budget.spend(1).map_err(CounterTraversalError::from)?;
        match participant.parent {
            None if index == 0 => {}
            None => return Err(CounterTraversalError::Order.into()),
            Some(parent) => loop {
                state.budget.spend(1).map_err(CounterTraversalError::from)?;
                match path.last() {
                    Some((node, _)) if *node == parent => break,
                    Some(_) => {
                        path.pop();
                    }
                    None => return Err(CounterTraversalError::Order.into()),
                }
            },
        }
        let depth = path.len();
        if depth >= limits.depth.get() {
            return Err(CounterTraversalError::Depth.into());
        }
        let active = participant.participates && path.last().is_none_or(|(_, active)| *active);
        reserve(
            &mut path,
            &mut state.bytes,
            limits.retained_bytes.get(),
            &mut state.budget,
        )?;
        path.push((index, active));
        while let Some(scope) = state.scopes.last() {
            state.budget.spend(1).map_err(CounterTraversalError::from)?;
            if scope.depth <= depth {
                break;
            }
            state.bytes -= scope.owned_bytes();
            state.scopes.pop();
        }
        if !active {
            continue;
        }
        for directive in participant.resets {
            state.reset(directive, depth)?;
        }
        for directive in participant.increments {
            state.increment(directive, depth)?;
        }
        visit(&mut state, index, depth)?;
    }
    Ok(())
}
/// Evaluate one rooted preorder tree. Empty input returns no text. Caller input,
/// names and reset values remain borrowed; only arithmetic and output text are owned.
/// Failure drops the complete candidate, with no host/input mutation or partial output.
pub fn evaluate_counters(
    participants: &[CounterParticipant<'_>],
    limits: CounterTraversalLimits,
) -> Result<Vec<CounterText>, CounterTraversalError> {
    let mut result = Vec::new();
    walk(
        participants.iter().map(|p| Participant {
            parent: p.parent,
            participates: p.participates,
            resets: p.resets,
            increments: p.increments,
        }),
        limits,
        |state, index, depth| -> Result<(), CounterTraversalError> {
            for (read_index, read) in participants[index].reads.iter().enumerate() {
                state.budget.spend(1)?;
                if result.len() == limits.results.get() {
                    return Err(CounterTraversalError::Results);
                }
                reserve(
                    &mut result,
                    &mut state.bytes,
                    limits.retained_bytes.get(),
                    &mut state.budget,
                )?;
                let text = state.text(*read, depth)?;
                state.bytes += text.capacity();
                result.push(CounterText {
                    participant: index,
                    read: read_index,
                    text,
                });
            }
            Ok(())
        },
    )?;
    Ok(result)
}
