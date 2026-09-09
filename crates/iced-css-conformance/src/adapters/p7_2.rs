//! Bounded scenarios around the real private progress owner.
use iced_css::animation::{
    clock::AnimationTiming,
    engine::{ProgressEngine, ProgressLimits},
};
use iced_css_core::{
    animation::registry::AnimationSelection, identity::SourceRegistry, values::registry::PropertyId,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    time::{Duration, Instant},
};
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    RebuildWithoutRequest {
        rebuilds: u32,
        explicit_requests: u32,
    },
    CaptureAndRestart {
        outgoing_duration_ms: u32,
        accept_after_ms: u32,
        incoming_duration_ms: u32,
        incoming_delay_ms: u32,
        sample_after_ms: u32,
    },
}
pub fn run(input: &Value) -> crate::CaseResult {
    match serde_json::from_value(input.clone()).map_err(|e| e.to_string())? {
        Input::RebuildWithoutRequest {
            rebuilds,
            explicit_requests,
        } => rebuild(rebuilds, explicit_requests),
        Input::CaptureAndRestart {
            outgoing_duration_ms,
            accept_after_ms,
            incoming_duration_ms,
            incoming_delay_ms,
            sample_after_ms,
        } => capture(
            outgoing_duration_ms,
            accept_after_ms,
            incoming_duration_ms,
            incoming_delay_ms,
            sample_after_ms,
        ),
    }
}
fn rebuild(rebuilds: u32, explicit_requests: u32) -> crate::CaseResult {
    if rebuilds > 1000 || explicit_requests > 1 {
        return Err("P7.2 seed permits at most 1000 builds and one request".into());
    }
    let mut engine = engine();
    let source = engine.register_source().map_err(|e| e.to_string())?;
    let property = PropertyId::lookup("color").ok_or("missing color registry entry")?;
    let now = Instant::now();
    if explicit_requests == 1 {
        let selection = AnimationSelection::new(&[property], NonZeroUsize::new(100000).unwrap())
            .map_err(|e| e.to_string())?;
        engine
            .start_progress(
                &source,
                &selection,
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                now,
            )
            .map_err(|e| e.to_string())?;
    }
    let sample_time = now
        .checked_add(Duration::from_millis(250))
        .ok_or("fixture timestamp overflow")?;
    engine.advance(sample_time);
    for _ in 0..rebuilds {
        engine.accepted_build();
    }
    // This fixture admits at most one request and starts with no bindings. Its
    // final presence count is therefore not presented as a general restart count.
    let active = engine
        .sample(&source, property)
        .map_err(|e| e.to_string())?
        .is_some();
    Ok(json!({"new_animations":usize::from(active)}))
}

fn engine() -> ProgressEngine {
    ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(1).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(1).unwrap(),
            bindings: NonZeroUsize::new(1).unwrap(),
            tracks: NonZeroUsize::new(100).unwrap(),
            retained_bytes: NonZeroUsize::new(100000).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    )
}

fn capture(
    outgoing_duration_ms: u32,
    accept_after_ms: u32,
    incoming_duration_ms: u32,
    incoming_delay_ms: u32,
    sample_after_ms: u32,
) -> crate::CaseResult {
    if [
        outgoing_duration_ms,
        accept_after_ms,
        incoming_duration_ms,
        incoming_delay_ms,
        sample_after_ms,
    ]
    .iter()
    .any(|v| *v > 60000)
    {
        return Err("P7.2 capture seed permits at most 60000ms per field".into());
    }
    let duration = |ms| Duration::from_millis(u64::from(ms));
    let mut engine = engine();
    let source = engine.register_source().map_err(|e| e.to_string())?;
    let property = PropertyId::lookup("color").ok_or("missing color registry entry")?;
    let selection = AnimationSelection::new(&[property], NonZeroUsize::new(100000).unwrap())
        .map_err(|e| e.to_string())?;
    let now = Instant::now();
    engine
        .start_progress(
            &source,
            &selection,
            AnimationTiming::new(duration(outgoing_duration_ms), Duration::ZERO),
            now,
        )
        .map_err(|e| e.to_string())?;
    let acceptance = now
        .checked_add(duration(accept_after_ms))
        .ok_or("fixture acceptance overflow")?;
    let later = acceptance
        .checked_add(duration(sample_after_ms))
        .ok_or("fixture sample overflow")?;
    let mut prepared = engine
        .prepare_progress(
            &source,
            &selection,
            AnimationTiming::new(duration(incoming_duration_ms), duration(incoming_delay_ms)),
            acceptance,
        )
        .map_err(|e| e.to_string())?;
    prepared.advance_outgoing();
    let outgoing = prepared
        .sample_outgoing(&source, property)
        .map_err(|e| e.to_string())?;
    prepared.commit();
    let incoming_at_acceptance = engine
        .sample(&source, property)
        .map_err(|e| e.to_string())?;
    engine.advance(later);
    let incoming_at_sample = engine
        .sample(&source, property)
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"outgoing_progress":outgoing,"incoming_at_acceptance":incoming_at_acceptance,"incoming_at_sample":incoming_at_sample}),
    )
}
