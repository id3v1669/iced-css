//! Initial-only publication of captured metadata with exact live source bindings.
//! This is evaluator input, not a complete native/style/geometry generation.
use crate::{
    animation::engine::{EngineError, PreparedSourceBindings, ProgressEngine, ProgressLimits},
    capture::{
        Construction,
        source::{ConstructionSource, ConstructionSourceError, ConstructionState},
    },
};
use iced_css_core::{
    identity::{SourceChangeLimits, SourceRef, SourceRegistry},
    source::selector_tree::{DocumentLanguage, SelectorTreeLimits},
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};

#[derive(Debug, Clone, Copy)]
pub struct PublicationLimits {
    pub nodes: NonZeroUsize,
    /// Exact Construction + state Vec + language String + token Vec capacities.
    /// This is not a whole-host peak-memory bound.
    pub snapshot_bytes: NonZeroUsize,
    /// Facade entries: 3+n+P+B; see InitialSourceHost::prepare.
    pub work: NonZeroUsize,
    /// Temporary selector-view indexing; released before identity preparation.
    pub selectors: SelectorTreeLimits,
    /// Identity candidate live/removed/token vectors and their validation work.
    pub identities: SourceChangeLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationError {
    AlreadyPublished,
    Nodes,
    StateCount,
    EmptyLanguage,
    Bytes,
    Work,
    DuplicateKey { first: usize, second: usize },
    Source(ConstructionSourceError),
    Engine(EngineError),
}
impl std::fmt::Display for PublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "initial source publication: {self:?}")
    }
}
impl std::error::Error for PublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Engine(error) => Some(error),
            _ => None,
        }
    }
}

/// An immutable captured document and its exact construction-order live tokens.
/// Source descriptors retain their metadata; no formatting boxes/IDs are invented.
#[derive(Debug)]
pub struct InitialSourceSnapshot {
    construction: Construction,
    states: Vec<ConstructionState>,
    language: Option<String>,
    tokens: Vec<SourceRef>,
    retained_bytes: usize,
}
impl InitialSourceSnapshot {
    pub fn construction(&self) -> &Construction {
        &self.construction
    }
    /// Caller-supplied facts; selector-derived ancestor propagation is separate.
    pub fn states(&self) -> &[ConstructionState] {
        &self.states
    }
    /// None is explicit Unknown; Some is always nonempty Known.
    pub fn document_language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    pub fn tokens(&self) -> &[SourceRef] {
        &self.tokens
    }
    pub fn token(&self, index: usize) -> Option<&SourceRef> {
        self.tokens.get(index)
    }
    /// Exact snapshot payload capacity, excluding registry/engine and view scratch.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Build a new bounded view borrowing this exact snapshot. View allocation
    /// failure leaves publication unchanged; no self-referential view is retained.
    pub fn selector_source(
        &self,
        limits: SelectorTreeLimits,
    ) -> Result<ConstructionSource<'_>, ConstructionSourceError> {
        ConstructionSource::new(
            &self.construction,
            &self.states,
            language(self.language.as_deref()),
            limits,
        )
    }
}
fn language(value: Option<&str>) -> DocumentLanguage<'_> {
    match value {
        Some(value) => DocumentLanguage::Known(value),
        None => DocumentLanguage::Unknown,
    }
}

/// Fresh private source/engine scope permitting exactly one accepted snapshot.
/// Only shared engine access is exposed, so tokens cannot be removed behind it.
/// Whole-host drop retires the registry and snapshot together; token Vec drop
/// alone is not revocation. No replacement, native UI or animation-start API exists.
///
/// ```compile_fail,E0596
/// use iced_css::host::source_publication::InitialSourceHost;
/// fn mutate(host: &InitialSourceHost) { let _ = host.engine().register_source(); }
/// ```
/// ```compile_fail,E0505
/// use iced_css::host::source_publication::InitialSourceHost;
/// fn outlive(host: InitialSourceHost) {
///     let snapshot = host.snapshot().unwrap();
///     drop(host);
///     let _ = snapshot.tokens();
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css::host::source_publication::InitialSourceHost;
/// use iced_css_core::source::selector_tree::SelectorTreeLimits;
/// fn outlive(host: InitialSourceHost, limits: SelectorTreeLimits) {
///     let view = host.snapshot().unwrap().selector_source(limits).unwrap();
///     drop(host);
///     let _ = view.root();
/// }
/// ```
pub struct InitialSourceHost {
    snapshot: Option<InitialSourceSnapshot>,
    engine: ProgressEngine,
}
impl InitialSourceHost {
    /// Registry marker/Motion construction uses the existing infallible
    /// constructors; their allocations are outside publication payload accounts.
    pub fn new(source_limit: NonZeroU64, progress: ProgressLimits) -> Self {
        Self {
            snapshot: None,
            engine: ProgressEngine::new(SourceRegistry::new(source_limit), progress),
        }
    }
    pub fn snapshot(&self) -> Option<&InitialSourceSnapshot> {
        self.snapshot.as_ref()
    }
    pub fn engine(&self) -> &ProgressEngine {
        &self.engine
    }

    /// Validate and reserve one complete captured source snapshot without publishing.
    ///
    /// Owns the supplied Construction, state Vec and optional language String.
    /// Every state is explicit; None language is Unknown and Some(empty) is invalid.
    /// Private Construction guarantees complete built-in postorder evidence. Future
    /// custom/deferred producer variants must revisit this admission boundary.
    /// Duplicate explicit keys reject only within the same actual parent scope.
    /// No key/static-path inference, native callbacks or constructor replay occurs.
    ///
    /// Successful facade work is 3+n+P+B: initial admission, n outer node visits,
    /// P earlier-node visits for each keyed node (including disqualified parents),
    /// B=min(key byte lengths)+1 for each same-parent explicit-key comparison,
    /// then final capacity accounting and prepaid commit. Every charge precedes its
    /// operation. Selector/identity work retains independent named limits.
    ///
    /// Snapshot bytes include caller-owned spare capacities and the exact prepared
    /// token Vec capacity. Identity staging also bounds that token Vec plus its
    /// own live/removed vectors: these overlapping accounts are not a disjoint
    /// whole-host peak. Temporary selector scratch is dropped before identity staging.
    ///
    /// # Errors
    /// Rejects second publication, invalid context/metadata/keys and resource/ID
    /// exhaustion before commit. Failed/abandoned preparation leaves the host empty
    /// and fresh IDs unconsumed; incoming owned candidate storage is discarded.
    pub fn prepare(
        &mut self,
        construction: Construction,
        states: Vec<ConstructionState>,
        language_value: Option<String>,
        limits: PublicationLimits,
    ) -> Result<PreparedInitialSource<'_>, PublicationError> {
        if self.snapshot.is_some() {
            return Err(PublicationError::AlreadyPublished);
        }
        let mut work = Work(limits.work.get());
        work.charge(1)?;
        let n = construction.nodes().len();
        if n > limits.nodes.get() {
            return Err(PublicationError::Nodes);
        }
        if states.len() != n {
            return Err(PublicationError::StateCount);
        }
        if language_value.as_ref().is_some_and(String::is_empty) {
            return Err(PublicationError::EmptyLanguage);
        }
        let limit = limits.snapshot_bytes.get();
        let base = states
            .capacity()
            .checked_mul(size_of::<ConstructionState>())
            .and_then(|bytes| bytes.checked_add(construction.retained_bytes()))
            .and_then(|bytes| {
                bytes.checked_add(language_value.as_ref().map_or(0, String::capacity))
            })
            .ok_or(PublicationError::Bytes)?;
        if base > isize::MAX as usize || base > limit {
            return Err(PublicationError::Bytes);
        }
        for (second, node) in construction.nodes().iter().enumerate() {
            work.charge(1)?;
            if let Some(key) = node.source_key() {
                for (first, earlier) in construction.nodes()[..second].iter().enumerate() {
                    work.charge(1)?;
                    if earlier.parent() == node.parent()
                        && let Some(previous) = earlier.source_key()
                    {
                        work.charge(
                            key.len()
                                .min(previous.len())
                                .checked_add(1)
                                .ok_or(PublicationError::Work)?,
                        )?;
                        if key == previous {
                            return Err(PublicationError::DuplicateKey { first, second });
                        }
                    }
                }
            }
        }
        drop(
            ConstructionSource::new(
                &construction,
                &states,
                language(language_value.as_deref()),
                limits.selectors,
            )
            .map_err(PublicationError::Source)?,
        );
        // Prepay final token-capacity accounting and the no-failure commit boundary.
        work.charge(2)?;
        let bindings = self
            .engine
            .prepare_source_changes(&[], n, limits.identities)
            .map_err(PublicationError::Engine)?;
        let retained_bytes = base
            .checked_add(bindings.token_retained_bytes())
            .ok_or(PublicationError::Bytes)?;
        if retained_bytes > isize::MAX as usize || retained_bytes > limit {
            return Err(PublicationError::Bytes);
        }
        Ok(PreparedInitialSource {
            bindings,
            slot: &mut self.snapshot,
            construction,
            states,
            language: language_value,
            retained_bytes,
        })
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), PublicationError> {
        self.0 = self.0.checked_sub(amount).ok_or(PublicationError::Work)?;
        Ok(())
    }
}

/// An exclusive initial candidate; dropping it does not publish or consume IDs.
///
/// ```compile_fail,E0502
/// use iced_css::host::source_publication::{InitialSourceHost,PublicationLimits};
/// use iced_css::capture::{Construction,source::ConstructionState};
/// fn interleave(host: &mut InitialSourceHost, construction: Construction,
///     states: Vec<ConstructionState>, limits: PublicationLimits) {
///     let prepared = host.prepare(construction, states, None, limits).unwrap();
///     let _ = host.snapshot();
///     prepared.commit();
/// }
/// ```
#[must_use = "commit the initial source snapshot or abandon it unchanged"]
pub struct PreparedInitialSource<'host> {
    bindings: PreparedSourceBindings<'host>,
    slot: &'host mut Option<InitialSourceSnapshot>,
    construction: Construction,
    states: Vec<ConstructionState>,
    language: Option<String>,
    retained_bytes: usize,
}
impl<'host> PreparedInitialSource<'host> {
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Publish tokens and immutable metadata together. Fresh private ownership
    /// ensures no removed progress binding, so no cleanup callback is invoked.
    /// Everything is prepaid: only ownership/Option moves remain after commit.
    pub fn commit(self) -> &'host InitialSourceSnapshot {
        let tokens = self.bindings.commit();
        self.slot.insert(InitialSourceSnapshot {
            construction: self.construction,
            states: self.states,
            language: self.language,
            tokens,
            retained_bytes: self.retained_bytes,
        })
    }
}
