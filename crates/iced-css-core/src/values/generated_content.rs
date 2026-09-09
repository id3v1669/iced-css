//! Logical expansion shares the counter owner, without replaying its directives.
use super::{
    CounterDirective, CounterTraversalError, CounterTraversalLimits, Participant, State, reserve,
    walk,
};
use crate::{
    formatting::quotes::{QuoteDepth, QuoteError, QuotePairs},
    values::content_computation::{ComputedContent, ComputedContentItem},
};
use std::{borrow::Cow, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct GeneratedLimits {
    /// Results bounds emitted contributions; bytes/work cover traversal, scopes
    /// and owned output. Borrowed computed-content revisions are caller-owned.
    pub counters: CounterTraversalLimits,
    pub quote_depth: NonZeroUsize,
}
#[derive(Debug, Clone, Copy)]
pub struct GeneratedParticipant<'a> {
    pub parent: Option<usize>,
    pub participates: bool,
    pub content: &'a ComputedContent<'a>,
    pub resets: &'a [CounterDirective],
    pub increments: &'a [CounterDirective],
    pub quotes: QuotePairs<'a>,
}
#[derive(Debug)]
pub enum FragmentContent<'a> {
    Text(Cow<'a, str>),
    Image(&'a str),
}
#[derive(Debug)]
pub struct GeneratedFragment<'a> {
    /// Local logical participant index, not a live host/source capability.
    pub participant: usize,
    pub item: usize,
    pub content: FragmentContent<'a>,
}
#[derive(Debug)]
pub struct GeneratedExpansion<'a> {
    pub fragments: Vec<GeneratedFragment<'a>>,
    pub quote_depth: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedError {
    Counter(CounterTraversalError),
    Quote(QuoteError),
}
impl From<CounterTraversalError> for GeneratedError {
    fn from(value: CounterTraversalError) -> Self {
        Self::Counter(value)
    }
}
impl From<QuoteError> for GeneratedError {
    fn from(value: QuoteError) -> Self {
        Self::Quote(value)
    }
}
impl std::fmt::Display for GeneratedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "generated content: {self:?}")
    }
}
impl std::error::Error for GeneratedError {}

/// Expand a complete logical participant tree. The formatting owner supplies
/// source/pseudo associations and computed revisions. No URI is fetched and no
/// selector child, shaped glyph, marker box or native widget is created here.
pub fn expand_generated<'a>(
    participants: &[GeneratedParticipant<'a>],
    limits: GeneratedLimits,
) -> Result<GeneratedExpansion<'a>, GeneratedError> {
    let mut fragments = Vec::new();
    let mut quotes = QuoteDepth::new(limits.quote_depth);
    walk(
        participants.iter().map(|p| Participant {
            parent: p.parent,
            participates: p.participates && !matches!(p.content, ComputedContent::Suppressed),
            resets: p.resets,
            increments: p.increments,
        }),
        limits.counters,
        |state, index, depth| -> Result<(), GeneratedError> {
            let participant = &participants[index];
            let content: &'a ComputedContent<'a> = participant.content;
            let ComputedContent::Items(items) = content else {
                return Ok(());
            };
            for (item_index, item) in items.iter().enumerate() {
                state.budget.spend(1).map_err(CounterTraversalError::from)?;
                let content = match item {
                    ComputedContentItem::Text(text) => FragmentContent::Text(Cow::Borrowed(text)),
                    ComputedContentItem::Uri(uri) => FragmentContent::Image(uri),
                    ComputedContentItem::Counter(read) => {
                        FragmentContent::Text(Cow::Owned(state.text(*read, depth)?))
                    }
                    ComputedContentItem::Quote(operation) => {
                        let Some(mark) = quotes.apply(*operation, participant.quotes)? else {
                            continue;
                        };
                        FragmentContent::Text(Cow::Borrowed(mark))
                    }
                };
                emit(
                    state,
                    &mut fragments,
                    GeneratedFragment {
                        participant: index,
                        item: item_index,
                        content,
                    },
                )?;
            }
            Ok(())
        },
    )?;
    Ok(GeneratedExpansion {
        fragments,
        quote_depth: quotes.depth(),
    })
}
fn emit<'a>(
    state: &mut State<'_>,
    fragments: &mut Vec<GeneratedFragment<'a>>,
    fragment: GeneratedFragment<'a>,
) -> Result<(), CounterTraversalError> {
    if fragments.len() == state.limits.results.get() {
        return Err(CounterTraversalError::Results);
    }
    let owned = match &fragment.content {
        FragmentContent::Text(Cow::Owned(text)) => text.capacity(),
        _ => 0,
    };
    if owned > state.available() {
        return Err(CounterTraversalError::Bytes);
    }
    state.bytes += owned;
    reserve(
        fragments,
        &mut state.bytes,
        state.limits.retained_bytes.get(),
        &mut state.budget,
    )?;
    fragments.push(fragment);
    Ok(())
}
