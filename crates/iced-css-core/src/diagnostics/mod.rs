//! Diagnostic state is owned by one host; delivery and fatal Result paths are separate.
mod causes;
pub use causes::{CauseError, CauseKey, CauseLimits, CauseTracker, Observation};
mod queue;
pub use queue::{
    Admission, Delivery, Diagnostic, DiagnosticKind, DiagnosticQueue, OmissionReason, Omissions,
    Origin, Provenance, QueueConfigError, QueueLimits, Report, SourceSpan,
};
