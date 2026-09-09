use iced_css_core::selectors::{SelectorLimits, parse_selector_list};
use iced_css_core::syntax::{
    RecoveryLimits, StatementContext, StatementKind, Statements, StringLimits, SyntaxFault,
    decode_identifier,
    import_placement::{ImportPlacement, PlacementDecision, StatementDisposition},
    imports::{ImportEntry, ImportLimits, ImportTracker, ResourceId},
    parse_import_prelude,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    work: NonZeroUsize,
    retained_bytes: NonZeroUsize,
    entries: NonZeroUsize,
    nesting: NonZeroUsize,
    occurrences: NonZeroU64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    root: String,
    a: String,
    limits: Limits,
}

/// Controlled root-import bookkeeping, with one in-memory resolver entry a.css.
/// Style selector validity and unknown-rule recovery determine import placement;
/// style declarations and media/page rule evaluation remain outside this fixture.
/// The child payload is acquired as opaque text, not evaluated as a stylesheet.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|error| error.to_string())?;
    if input.scenario != "duplicate_import_occurrence" {
        return Err("unknown import-occurrence scenario".into());
    }
    let limits = StringLimits {
        work: input.limits.work,
        retained_bytes: input.limits.retained_bytes,
    };
    let recovery = RecoveryLimits {
        work: input.limits.work,
        retained_bytes: input.limits.retained_bytes,
        nesting: input.limits.nesting,
    };
    let mut tracker = ImportTracker::new(ImportLimits {
        work: input.limits.work,
        retained_bytes: input.limits.retained_bytes,
        entries: input.limits.entries,
        occurrences: input.limits.occurrences,
    });
    let root_id = ResourceId::new(NonZeroU64::new(1).expect("one is nonzero"));
    let a_id = ResourceId::new(NonZeroU64::new(2).expect("two is nonzero"));
    let ImportEntry::Visit(root) = tracker
        .enter(root_id, true)
        .map_err(|error| error.to_string())?
    else {
        return Err("fresh tracker did not admit the root".into());
    };
    tracker
        .mark_acquired(root_id)
        .map_err(|error| error.to_string())?;
    let mut acquired_a: Option<&str> = None;
    let mut occurrences = 0usize;
    let mut placement = ImportPlacement::new(StatementContext::Stylesheet);
    for statement in Statements::new(&input.root, StatementContext::Stylesheet, recovery) {
        let statement = statement.map_err(|error| error.to_string())?;
        let StatementKind::AtRule { keyword } = statement.kind else {
            let disposition = if statement.body.is_none() {
                StatementDisposition::Ignored
            } else {
                match parse_selector_list(
                    statement.prelude,
                    SelectorLimits {
                        work: input.limits.work,
                        retained_bytes: input.limits.retained_bytes,
                        entries: input.limits.entries,
                    },
                ) {
                    Ok(_) => StatementDisposition::Other,
                    Err(error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                        return Err(error.to_string());
                    }
                    Err(_) => StatementDisposition::Ignored,
                }
            };
            placement.observe(disposition);
            continue;
        };
        let name = decode_identifier(&keyword[1..], limits).map_err(|error| error.to_string())?;
        if name.text.eq_ignore_ascii_case("media") || name.text.eq_ignore_ascii_case("page") {
            return Err("media/page evaluation is outside the controlled root fixture".into());
        }
        if !name.text.eq_ignore_ascii_case("import") {
            placement.observe(StatementDisposition::Ignored);
            continue;
        }
        if statement.body.is_some() {
            return Err("controlled fixture requires block-free import syntax".into());
        }
        if placement.observe(StatementDisposition::Import) != PlacementDecision::Allowed {
            continue;
        }
        let import =
            parse_import_prelude(statement.prelude, limits).map_err(|error| error.to_string())?;
        if !import.applies_to_screen {
            continue;
        }
        // Resolve only after media admission. This fixture service declares exactly
        // one location; general URI normalization belongs to the product resolver.
        if import.location.text != "a.css" {
            return Err("location not supplied by the controlled fixture service".into());
        }
        let ImportEntry::Visit(visit) = tracker
            .enter(a_id, true)
            .map_err(|error| error.to_string())?
        else {
            return Err("noncyclic controlled child was not admitted".into());
        };
        if visit.needs_acquisition {
            if input.a.len() > input.limits.retained_bytes.get() {
                return Err("fixture child acquisition exceeds its byte allowance".into());
            }
            acquired_a = Some(&input.a);
            tracker
                .mark_acquired(a_id)
                .map_err(|error| error.to_string())?;
        } else if acquired_a.is_none() {
            return Err("tracker cache identity has no retained fixture payload".into());
        }
        occurrences = occurrences
            .checked_add(1)
            .ok_or("fixture occurrence count overflow")?;
        tracker
            .finish(visit.occurrence)
            .map_err(|error| error.to_string())?;
    }
    tracker
        .finish(root.occurrence)
        .map_err(|error| error.to_string())?;
    Ok(
        json!({"import_occurrences":occurrences, "unique_acquisition_identities":usize::from(acquired_a.is_some())}),
    )
}
