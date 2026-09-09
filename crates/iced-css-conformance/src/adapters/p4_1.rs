use iced_css_core::resources::publication::{
    PublicationGate, PublicationLimits, PublicationStatus,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    num::{NonZeroU64, NonZeroUsize},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    active_revision: u64,
    requests: Vec<String>,
    completions: Vec<Completion>,
    request_limit: NonZeroU64,
    revision_limit: NonZeroU64,
    work: NonZeroUsize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    request: String,
    result: Outcome,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Success,
    Error,
}
/// Controlled generation/revision admission; no HTTP, IO, cache or host claims.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "latest_failure" {
        return Err("unknown publication scenario".into());
    }
    let prior = usize::try_from(input.active_revision)
        .map_err(|_| "initial revision exceeds work domain")?;
    let steps = prior
        .checked_add(input.requests.len())
        .and_then(|n| n.checked_add(input.completions.len()))
        .ok_or("work overflow")?;
    if steps > input.work.get() {
        return Err("publication adapter work exhausted".into());
    }
    let mut gate = PublicationGate::new(
        "controlled-asset",
        PublicationLimits {
            requests: input.request_limit,
            revisions: input.revision_limit,
        },
    );
    // Establish previous revisions through real successful admission, not injected state.
    for _ in 0..prior {
        let ticket = gate.request().map_err(|e| e.to_string())?;
        gate.finish(ticket, true).map_err(|e| e.to_string())?;
    }
    let mut tickets = BTreeMap::new();
    for name in input.requests {
        if tickets.contains_key(&name) {
            return Err("duplicate request label".into());
        }
        tickets.insert(name, gate.request().map_err(|e| e.to_string())?);
    }
    for completion in input.completions {
        let ticket = tickets
            .get(&completion.request)
            .ok_or("unknown completion request")?;
        gate.finish(*ticket, matches!(completion.result, Outcome::Success))
            .map_err(|e| e.to_string())?;
    }
    Ok(
        json!({"active_revision":gate.active_revision().map_or(0,|revision|revision.get()),"stale_status":gate.status()==PublicationStatus::Failed}),
    )
}
