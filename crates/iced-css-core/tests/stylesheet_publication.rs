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

use iced_css_core::{
    resources::publication::{PublicationError, PublicationLimits},
    stylesheet::publication::*,
};
fn owner() -> StylesheetPublication {
    StylesheetPublication::new(PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    })
}
fn candidate(css: &str) -> UnicodeStylesheetCandidate {
    parse_separate_unicode_stylesheet(
        css.as_bytes(),
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    .unwrap()
}
fn complete(
    owner: &mut StylesheetPublication,
    ticket: StylesheetTicket,
    css: &str,
) -> SheetCompletion<&'static str> {
    owner.finish(ticket, Ok(candidate(css))).unwrap()
}
#[test]
fn first_failure_omits_sheet_and_reload_failure_retains_owned_source() {
    let mut owner = owner();
    let first = owner.request().unwrap();
    assert!(matches!(
        owner
            .finish(first, Err::<UnicodeStylesheetCandidate, _>("read failed"))
            .unwrap(),
        SheetCompletion::Failed("read failed")
    ));
    assert!(owner.active().is_none());
    assert!(owner.active_revision().is_none());
    let first = owner.request().unwrap();
    assert!(matches!(
        complete(&mut owner, first, "a{color:red}"),
        SheetCompletion::Published(_)
    ));
    let revision = owner.active_revision();
    let failed = owner.request().unwrap();
    owner
        .finish(
            failed,
            Err::<UnicodeStylesheetCandidate, _>("decode failed"),
        )
        .unwrap();
    assert_eq!(owner.active().unwrap().source().text(), "a{color:red}");
    assert_eq!(owner.active_revision(), revision);
}
#[test]
fn recovered_candidate_publishes_valid_declarations_and_empty_success_replaces_it() {
    let mut owner = owner();
    let ticket = owner.request().unwrap();
    complete(&mut owner, ticket, "a{color:red;width:bogus;height:2px}");
    let sheet = owner.active().unwrap().sheet();
    let rule = sheet.local_screen_rules().next().unwrap();
    assert_eq!(rule.declarations.len(), 2);
    assert_eq!(rule.rejected.len(), 1);
    assert_eq!(rule.rejected[0].reason, RejectionReason::InvalidValue);
    let ticket = owner.request().unwrap();
    complete(&mut owner, ticket, "");
    assert!(owner.active().unwrap().sheet().entries.is_empty());
    assert_eq!(owner.active().unwrap().source().text(), "");
    assert_eq!(owner.active_revision().unwrap().get(), 2);
}
#[test]
fn stale_foreign_and_duplicate_results_cannot_replace_current_content() {
    let mut owner = owner();
    let a = owner.request().unwrap();
    let b = owner.request().unwrap();
    let b_again = b.clone();
    complete(&mut owner, b, "a{color:blue}");
    assert!(matches!(
        complete(&mut owner, a, "a{color:red}"),
        SheetCompletion::Ignored
    ));
    assert!(matches!(
        complete(&mut owner, b_again, "a{color:red}"),
        SheetCompletion::Ignored
    ));
    let mut other = StylesheetPublication::new(PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    });
    let foreign = other.request().unwrap();
    assert!(matches!(
        complete(&mut owner, foreign, "a{color:red}"),
        SheetCompletion::Ignored
    ));
    assert_eq!(owner.active().unwrap().source().text(), "a{color:blue}");
    let older = owner.request().unwrap();
    let newest = owner.request().unwrap();
    owner
        .finish(newest, Err::<UnicodeStylesheetCandidate, _>("failed"))
        .unwrap();
    assert!(matches!(
        complete(&mut owner, older, "a{color:green}"),
        SheetCompletion::Ignored
    ));
    assert_eq!(owner.active().unwrap().source().text(), "a{color:blue}");
}
#[test]
fn revision_exhaustion_retains_candidate_and_retired_owner_tickets_are_foreign() {
    let mut owner = StylesheetPublication::new(PublicationLimits {
        requests: NonZeroU64::new(3).unwrap(),
        revisions: NonZeroU64::MIN,
    });
    let first = owner.request().unwrap();
    complete(&mut owner, first, "a{color:red}");
    let next = owner.request().unwrap();
    assert!(matches!(
        owner.finish::<()>(next, Ok(candidate(""))),
        Err(PublicationError::Revisions)
    ));
    assert_eq!(owner.active().unwrap().source().text(), "a{color:red}");
    let old = owner.request().unwrap();
    assert!(matches!(owner.request(), Err(PublicationError::Requests)));
    drop(owner);
    let mut replacement = StylesheetPublication::new(PublicationLimits {
        requests: NonZeroU64::new(3).unwrap(),
        revisions: NonZeroU64::MIN,
    });
    replacement.request().unwrap();
    replacement.request().unwrap();
    replacement.request().unwrap();
    assert!(matches!(
        complete(&mut replacement, old, "a{color:red}"),
        SheetCompletion::Ignored
    ));
    assert!(replacement.active().is_none());
}

#[test]
fn owner_scoped_preflight_cannot_admit_foreign_or_completed_tickets() {
    use iced_css_core::stylesheet::publication::CandidatePublication;
    let bounds = iced_css_core::resources::publication::PublicationLimits {
        requests: NonZeroU64::new(3).unwrap(),
        revisions: NonZeroU64::new(3).unwrap(),
    };
    let mut owner = CandidatePublication::<u8>::new(bounds);
    let mut other = CandidatePublication::<u8>::new(bounds);
    let first = owner.request().unwrap();
    let foreign = other.request().unwrap();
    assert!(owner.is_current(&first));
    assert!(!owner.is_current(&foreign));
    assert!(!other.is_current(&first));
    owner.finish(first.clone(), Ok::<_, ()>(1)).unwrap();
    assert!(!owner.is_current(&first));
    let older = owner.request().unwrap();
    let newest = owner.request().unwrap();
    assert!(!owner.is_current(&older));
    assert!(owner.is_current(&newest));
    owner
        .finish(newest.clone(), Err::<u8, _>("failed"))
        .unwrap();
    assert!(!owner.is_current(&newest));
    assert!(!owner.is_current(&older));
    assert_eq!(owner.active(), Some(&1));
}
