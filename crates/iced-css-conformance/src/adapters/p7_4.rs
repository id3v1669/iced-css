use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> RuleAssemblyLimits {
    RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        },
        selectors: SelectorLimits {
            work: n(),
            retained_bytes: n(),
            entries: n(),
        },
        values: ColorLimits {
            syntax: StringLimits {
                work: n(),
                retained_bytes: n(),
            },
            numeric: NumericLimits {
                work: n(),
                digits: n(),
            },
        },
        entries: n(),
        record_bytes: n(),
        work: n(),
        name_bytes: n(),
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 10,
        first_declaration: NonZeroU64::new(20).unwrap(),
    }
}

fn sheet_limits() -> StylesheetLimits {
    StylesheetLimits {
        rules: limits(),
        statements: n(),
        record_bytes: n(),
        work: n(),
    }
}

use iced_css_core::syntax::{
    decode::DecodeLimits,
    encoding_selection::{EncodingMetadata, SelectionLimits},
    unicode_intake::IntakeLimits,
};
fn intake_limits() -> StylesheetIntakeLimits {
    StylesheetIntakeLimits {
        intake: IntakeLimits {
            selection: SelectionLimits {
                registry_work: n(),
                prefix_work: n(),
            },
            decode: DecodeLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
            charset_work: n(),
        },
        sheet: sheet_limits(),
    }
}

use iced_css::host::stylesheet::RuntimeStylesheet;
use iced_css_core::resources::{
    publication::PublicationLimits,
    triggers::{RequestError, RequestReason, ResourceKind, RuntimeMode, RuntimeResourceGate},
};
use serde::Deserialize;
use serde_json::{Value, json};
mod graph;
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    RootImportReload {
        root_before: String,
        import_before: String,
        root_after: String,
        import_after: Option<String>,
    },
    RemoteRefreshTriggers {
        asset_state: AssetState,
        events: Vec<Event>,
    },
    StylesheetRequests {
        initial: Option<String>,
        request_count: usize,
        completions: Vec<Completion>,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AssetState {
    AlreadyLoaded,
    NeverRequested,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    Expiry,
    Eviction,
    LocalWatch,
    ViewRebuild,
    ExplicitMessage,
    FirstAcquisition,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    request: usize,
    css: Option<String>,
}
fn publication_limits() -> PublicationLimits {
    PublicationLimits {
        requests: NonZeroU64::new(128).unwrap(),
        revisions: NonZeroU64::new(128).unwrap(),
    }
}
fn parse(css: &str) -> Result<UnicodeStylesheetCandidate, String> {
    parse_separate_unicode_stylesheet(
        css.as_bytes(),
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    .map_err(|e| e.to_string())
}
/// Controlled request admission and parsed-resource/graph publication. No actual
/// network transport, watcher, automatic acquisition or native UI claim.
pub fn run(value: &Value) -> crate::CaseResult {
    match serde_json::from_value::<Input>(value.clone()).map_err(|e| e.to_string())? {
        Input::RootImportReload {
            root_before,
            import_before,
            root_after,
            import_after,
        } => graph::run(
            &root_before,
            &import_before,
            &root_after,
            import_after.as_deref(),
        ),
        Input::RemoteRefreshTriggers {
            asset_state,
            events,
        } => {
            if events.len() > 64 {
                return Err("fixture exceeds 64 events".into());
            }
            let mut gate = RuntimeResourceGate::new(
                "fixture-web-resource",
                RuntimeMode::Auto,
                ResourceKind::Web,
                publication_limits(),
            );
            if matches!(asset_state, AssetState::AlreadyLoaded) {
                let ticket = gate
                    .request(RequestReason::FirstAcquisition)
                    .map_err(|e| e.to_string())?;
                gate.finish(ticket, true).map_err(|e| e.to_string())?;
            }
            let mut count = 0;
            for event in events {
                let reason = match event {
                    Event::Expiry => RequestReason::CacheExpiry,
                    Event::Eviction => RequestReason::CacheEviction,
                    Event::LocalWatch => RequestReason::LocalChange,
                    Event::ViewRebuild => RequestReason::Reevaluation,
                    Event::ExplicitMessage => RequestReason::ExplicitReload,
                    Event::FirstAcquisition => RequestReason::FirstAcquisition,
                };
                match gate.request(reason) {
                    Ok(ticket) => {
                        count += 1;
                        gate.finish(ticket, true).map_err(|e| e.to_string())?;
                    }
                    Err(RequestError::Unauthorized) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            Ok(json!({"http_refresh_requests":count}))
        }
        Input::StylesheetRequests {
            initial,
            request_count,
            completions,
        } => {
            if request_count > 64 || completions.len() > 64 {
                return Err("fixture exceeds 64 requests/completions".into());
            }
            let mut bytes = initial.as_ref().map_or(0, String::len);
            for completion in &completions {
                if completion.request >= request_count {
                    return Err("completion names an absent request".into());
                }
                bytes = bytes
                    .checked_add(completion.css.as_ref().map_or(0, String::len))
                    .ok_or("fixture byte overflow")?;
            }
            if bytes > 4096 {
                return Err("fixture exceeds 4096 CSS bytes".into());
            }
            let mut owner = RuntimeStylesheet::new(publication_limits(), n());
            if let Some(initial) = initial {
                let candidate = parse(&initial)?;
                let ticket = owner.request().map_err(|e| e.to_string())?;
                owner
                    .finish::<String>(ticket, Ok(candidate))
                    .map_err(|e| e.to_string())?;
            }
            let mut requests = Vec::new();
            for _ in 0..request_count {
                requests.push(owner.request().map_err(|e| e.to_string())?);
            }
            for completion in completions {
                let candidate = match completion.css {
                    Some(css) => parse(&css),
                    None => Err("controlled acquisition failure".into()),
                };
                owner
                    .finish(requests[completion.request].clone(), candidate)
                    .map_err(|e| e.to_string())?;
            }
            let active = owner.active();
            let declarations = active.map_or(0, |c| {
                c.sheet()
                    .local_screen_rules()
                    .map(|r| r.declarations.len())
                    .sum::<usize>()
            });
            let rejected = active.map_or(0, |c| {
                c.sheet()
                    .local_screen_rules()
                    .map(|r| r.rejected.len())
                    .sum::<usize>()
            });
            Ok(
                json!({"active_css":active.map(|c|c.source().text()),"active_revision":owner.active_revision().map_or(0,|v|v.get()),"declarations":declarations,"recovered_declarations":rejected}),
            )
        }
    }
}
