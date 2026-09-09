use iced_css_core::{
    diagnostics::{
        CauseKey, CauseLimits, CauseTracker, Diagnostic, DiagnosticQueue, Observation, Origin,
        QueueLimits,
    },
    identity::IdAllocator,
    values::{Decimal, NumericLimits, registry::PropertyId},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::{NonZeroU64, NonZeroUsize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    digits: NonZeroUsize,
    work: NonZeroUsize,
    reports: usize,
    report_bytes: usize,
    causes: usize,
    cause_bytes: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    Detect,
    ReevaluateSame,
    Resolve,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    events: Vec<Event>,
    coefficient: String,
    context_bits: u32,
    limits: Limits,
}
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "cause_lifecycle" {
        return Err("unknown diagnostic scenario".into());
    }
    let limits = NumericLimits {
        digits: input.limits.digits,
        work: input.limits.work,
    };
    let converted = Decimal::parse(&input.coefficient, limits)
        .map_err(|e| e.to_string())?
        .scaled_nonnegative_f32(f32::from_bits(input.context_bits), limits)
        .map_err(|e| e.to_string())?;
    let event = converted
        .event
        .ok_or("scenario requires an actual range event")?;
    let mut causes = CauseTracker::new(
        CauseLimits::new(input.limits.causes, input.limits.cause_bytes)
            .map_err(|e| e.to_string())?,
    );
    let mut queue = DiagnosticQueue::new(
        NonZeroU64::MIN,
        QueueLimits::new(input.limits.reports, input.limits.report_bytes)
            .map_err(|e| e.to_string())?,
    );
    let mut ids = IdAllocator::new(NonZeroU64::MIN);
    let property = PropertyId::lookup("font-size").ok_or("missing font-size registry entry")?;
    let key = CauseKey {
        instance: ids.allocate().map_err(|e| e.to_string())?,
        property,
        event,
        semantic: NonZeroU64::MIN,
    };
    for action in input.events {
        match action {
            Event::Resolve => {
                causes.resolve(key);
            }
            Event::Detect | Event::ReevaluateSame => {
                if causes.observe(key).map_err(|e| e.to_string())? == Observation::NewCause {
                    let report = Diagnostic::range(
                        converted,
                        Origin::Synthetic("controlled fixture".into()),
                        Some(key.instance),
                        property,
                    )
                    .ok_or("missing range event")?;
                    queue.admit(report);
                }
            }
        }
    }
    Ok(json!({"admitted_reports":queue.pending_count()}))
}
