//! Complete admitted source boundaries and raw content, before CSS box generation.
use super::{
    source_eligibility::SourceEligibility,
    source_participants::{SourceParticipant, SourceParticipants},
};
use crate::capture::{ContentCapability, ManagedContentFamily};
use iced_css_core::formatting::display::{Display, Flow};
use std::{iter::FusedIterator, num::NonZeroUsize};

/// Bounds for the complete eligible domain and its one prepaid traversal.
/// This stage allocates no owned payload, so it has no byte/allocation limit.
#[derive(Debug, Clone, Copy)]
pub struct SourceStructuralEventLimits {
    pub sources: NonZeroUsize,
    pub events: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructuralDomainReason {
    Flow,
    Family,
    Display,
    Root,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructuralEventError {
    Sources,
    Events,
    Work,
    Domain {
        source: usize,
        reason: SourceStructuralDomainReason,
    },
}
impl std::fmt::Display for SourceStructuralEventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source structural events: {self:?}")
    }
}
impl std::error::Error for SourceStructuralEventError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructuralEventKind {
    Begin,
    Content,
    End,
}

/// One source occurrence, with no invented formatting identity. Content occurs
/// only for Text/RichText and retains raw/empty text and original rich runs.
/// The item may outlive the stream, but not its borrowed eligibility owner.
#[must_use]
pub struct SourceStructuralEvent<'e, 'd, 's, 'b, 'r> {
    kind: SourceStructuralEventKind,
    participant: SourceParticipant<'e, 'd, 's, 'b, 'r>,
}
impl<'e, 'd, 's, 'b, 'r> SourceStructuralEvent<'e, 'd, 's, 'b, 'r> {
    pub fn kind(&self) -> SourceStructuralEventKind {
        self.kind
    }
    /// References to content/token/style obtained through this entry borrow the
    /// event. Its eligibility() accessor retains the explicit owner lifetime.
    pub fn participant(&self) -> &SourceParticipant<'e, 'd, 's, 'b, 'r> {
        &self.participant
    }
}

/// One-shot source traversal with fixed cursor state and no owned collections.
/// Each next() emits at most one event; closing an ancestor is itself an End.
/// Prepaid work is unchanged by early stopping or repeated exhaustion. Storage
/// for items retained by the caller, and all borrowed predecessor payload, is
/// outside this stage's incremental account. No generated-content completeness,
/// whitespace collapse, anonymous wrapper or inline continuation is certified.
#[must_use]
pub struct SourceStructuralEvents<'e, 'd, 's, 'b, 'r> {
    participants: SourceParticipants<'e, 'd, 's, 'b, 'r>,
    source_count: usize,
    content_count: usize,
    event_count: usize,
    work: usize,
    next_ordinal: usize,
    open_top: Option<usize>,
    pending_content: bool,
    remaining_events: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceStructuralEvents<'e, 'd, 's, 'b, 'r> {
    pub fn eligibility(&self) -> &'e SourceEligibility<'d, 's, 'b, 'r> {
        self.participants.eligibility()
    }
    /// Initial eligible source total, not events remaining.
    pub fn source_count(&self) -> usize {
        self.source_count
    }
    pub fn content_count(&self) -> usize {
        self.content_count
    }
    pub fn event_count(&self) -> usize {
        self.event_count
    }
    /// Exact prepaid 1+3*k+t schedule, including emissions not yet requested.
    pub fn work(&self) -> usize {
        self.work
    }
}
impl<'e, 'd, 's, 'b, 'r> Iterator for SourceStructuralEvents<'e, 'd, 's, 'b, 'r> {
    type Item = SourceStructuralEvent<'e, 'd, 's, 'b, 'r>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_events == 0 {
            return None;
        }
        let (kind, ordinal) = if self.pending_content {
            self.pending_content = false;
            (
                SourceStructuralEventKind::Content,
                self.open_top.expect("pending content has an open source"),
            )
        } else if self
            .eligibility()
            .entry(self.next_ordinal)
            .is_some_and(|entry| entry.parent == self.open_top)
        {
            let ordinal = self.next_ordinal;
            self.next_ordinal += 1;
            self.open_top = Some(ordinal);
            (SourceStructuralEventKind::Begin, ordinal)
        } else {
            let ordinal = self
                .open_top
                .expect("remaining events close an open source");
            self.open_top = self
                .eligibility()
                .entry(ordinal)
                .expect("validated eligible ordinal")
                .parent;
            (SourceStructuralEventKind::End, ordinal)
        };
        let source = self
            .eligibility()
            .entry(ordinal)
            .expect("validated eligible ordinal")
            .source;
        let participant = self
            .participants
            .get(source)
            .expect("validated immutable original source");
        if kind == SourceStructuralEventKind::Begin {
            self.pending_content = matches!(
                participant.content(),
                ContentCapability::Text(_) | ContentCapability::RichText { .. }
            );
        }
        self.remaining_events -= 1;
        Some(SourceStructuralEvent { kind, participant })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_events, Some(self.remaining_events))
    }
}
impl ExactSizeIterator for SourceStructuralEvents<'_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining_events
    }
}
impl FusedIterator for SourceStructuralEvents<'_, '_, '_, '_, '_> {}

fn preflight_work(
    k: usize,
    limits: SourceStructuralEventLimits,
) -> Result<usize, SourceStructuralEventError> {
    if k > limits.sources.get() {
        return Err(SourceStructuralEventError::Sources);
    }
    let work = k.checked_add(1).ok_or(SourceStructuralEventError::Work)?;
    if work > limits.work.get() {
        return Err(SourceStructuralEventError::Work);
    }
    Ok(work)
}
fn schedule(
    k: usize,
    t: usize,
    preflight: usize,
    limits: SourceStructuralEventLimits,
) -> Result<(usize, usize), SourceStructuralEventError> {
    let events = k
        .checked_mul(2)
        .and_then(|n| n.checked_add(t))
        .ok_or(SourceStructuralEventError::Events)?;
    if events > limits.events.get() {
        return Err(SourceStructuralEventError::Events);
    }
    let work = preflight
        .checked_add(events)
        .ok_or(SourceStructuralEventError::Work)?;
    if work > limits.work.get() {
        return Err(SourceStructuralEventError::Work);
    }
    Ok((events, work))
}

/// Validate the entire eligible source domain before exposing any event.
/// Managed Container/Row/Column admit normal-flow Block/Inline; Text/RichText
/// admit normal-flow Inline. A surviving root must be a managed Block. All other
/// eligible sources reject the operation, never silently disappear. Suppressed
/// subtrees contribute no events. Only finite facts are read: no text/run scan.
///
/// # Errors
/// Sources precedes the prepaid 1+k Work check. The eligible preorder then checks
/// Flow, Family, Display, Root, returning the first failing original index.
/// Checked 2*k+t Events follows domain validation; the final Work check admits
/// exactly 1+3*k+t. Empty eligible scope succeeds with work1. No allocation occurs.
///
/// ```compile_fail,E0505
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants, source_structural_events::*};
/// fn borrowed(owner: SourceEligibility<'_, '_, '_, '_>, limits: SourceStructuralEventLimits) {
///     let mut events = source_structural_events(&SourceParticipants::new(&owner), limits).unwrap();
///     drop(owner);
///     let _ = events.next();
/// }
/// ```
/// ```
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants, source_structural_events::*};
/// fn wrapper_can_drop(owner: &SourceEligibility<'_, '_, '_, '_>, limits: SourceStructuralEventLimits) {
///     let participants = SourceParticipants::new(owner);
///     let mut events = source_structural_events(&participants, limits).unwrap();
///     drop(participants);
///     if let Some(item) = events.next() {
///         drop(events);
///         let _ = item.participant().token();
///     }
/// }
/// ```
pub fn source_structural_events<'e, 'd, 's, 'b, 'r>(
    participants: &SourceParticipants<'e, 'd, 's, 'b, 'r>,
    limits: SourceStructuralEventLimits,
) -> Result<SourceStructuralEvents<'e, 'd, 's, 'b, 'r>, SourceStructuralEventError> {
    let eligibility = participants.eligibility();
    let k = eligibility.eligible_count();
    let preflight = preflight_work(k, limits)?;
    let mut t = 0usize;
    for (ordinal, entry) in eligibility.ordered().iter().enumerate() {
        let item = participants
            .get(entry.source)
            .expect("immutable eligible source");
        let reject = |reason| SourceStructuralEventError::Domain {
            source: entry.source,
            reason,
        };
        if item.resolution().flow != Flow::Normal {
            return Err(reject(SourceStructuralDomainReason::Flow));
        }
        let content = match item.content() {
            ContentCapability::Text(_) | ContentCapability::RichText { .. } => true,
            ContentCapability::Managed(
                ManagedContentFamily::Container
                | ManagedContentFamily::Row
                | ManagedContentFamily::Column,
            ) => false,
            ContentCapability::Managed(_)
            | ContentCapability::CheckboxLabel(_)
            | ContentCapability::TogglerLabel(_)
            | ContentCapability::RadioLabel(_)
            | ContentCapability::RecipeRequired(_) => {
                return Err(reject(SourceStructuralDomainReason::Family));
            }
            #[cfg(feature = "image-without-codecs")]
            ContentCapability::Image(_) => {
                return Err(reject(SourceStructuralDomainReason::Family));
            }
            #[cfg(feature = "svg")]
            ContentCapability::Svg(_) => return Err(reject(SourceStructuralDomainReason::Family)),
        };
        let display = item.resolution().display;
        if if content {
            display != Display::Inline
        } else {
            !matches!(display, Display::Block | Display::Inline)
        } {
            return Err(reject(SourceStructuralDomainReason::Display));
        }
        if ordinal == 0 && (content || display != Display::Block) {
            return Err(reject(SourceStructuralDomainReason::Root));
        }
        if content {
            t = t.checked_add(1).ok_or(SourceStructuralEventError::Events)?;
        }
    }
    let (event_count, work) = schedule(k, t, preflight, limits)?;
    Ok(SourceStructuralEvents {
        participants: SourceParticipants::new(eligibility),
        source_count: k,
        content_count: t,
        event_count,
        work,
        next_ordinal: 0,
        open_top: None,
        pending_content: false,
        remaining_events: event_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits(sources: usize, events: usize, work: usize) -> SourceStructuralEventLimits {
        SourceStructuralEventLimits {
            sources: NonZeroUsize::new(sources).unwrap(),
            events: NonZeroUsize::new(events).unwrap(),
            work: NonZeroUsize::new(work).unwrap(),
        }
    }
    #[test]
    fn synthetic_arithmetic_overflow_and_precedence() {
        // Synthetic counts exercise arithmetic helpers, not constructible source owners.
        let max = usize::MAX;
        assert_eq!(
            preflight_work(max, limits(max - 1, max, max)),
            Err(SourceStructuralEventError::Sources)
        );
        assert_eq!(
            preflight_work(max, limits(max, max, max)),
            Err(SourceStructuralEventError::Work)
        );
        let k = max / 2 + 1;
        assert_eq!(
            schedule(k, 0, k + 1, limits(max, max, max)),
            Err(SourceStructuralEventError::Events)
        );
        let k = max / 3 + 1;
        assert_eq!(
            schedule(k, 0, k + 1, limits(max, max, max)),
            Err(SourceStructuralEventError::Work)
        );
        assert_eq!(
            schedule(2, 1, 3, limits(2, 4, 3)),
            Err(SourceStructuralEventError::Events)
        );
        assert_eq!(
            schedule(2, 1, 3, limits(2, 5, 7)),
            Err(SourceStructuralEventError::Work)
        );
        assert_eq!(schedule(2, 1, 3, limits(2, 5, 8)), Ok((5, 8)));
        assert_eq!(schedule(0, 0, 1, limits(max, max, max)), Ok((0, 1)));
    }
}
