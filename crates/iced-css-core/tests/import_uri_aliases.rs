use iced_css_core::stylesheet::{RuleAssemblyError, uri_aliases::*};
use std::num::NonZeroUsize;
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> ImportUriAliasLimits {
    ImportUriAliasLimits {
        entries: n(16),
        bytes: n(10000),
        work: n(100000),
    }
}
#[test]
fn observed_alias_batches_are_idempotent_and_keep_exact_keys() {
    let mut aliases = ImportUriAliases::new(limits()).unwrap();
    aliases
        .bind(
            "https://example.test/final.css",
            &["https://example.test/a.css", "https://example.test/b.css"],
        )
        .unwrap();
    assert_eq!(aliases.len(), 3);
    let retained = aliases.retained_bytes();
    aliases
        .bind(
            "https://example.test/final.css",
            &["https://example.test/a.css", "https://example.test/a.css"],
        )
        .unwrap();
    assert_eq!(aliases.retained_bytes(), retained);
    assert_eq!(aliases.len(), 3);
    for key in [
        "https://example.test/final.css",
        "https://example.test/a.css",
        "https://example.test/b.css",
    ] {
        assert_eq!(
            aliases.lookup(key).unwrap(),
            Some("https://example.test/final.css")
        );
    }
    for key in [
        "https://example.test/unknown",
        "https://example.test/a.css?",
        "https://EXAMPLE.test/a.css",
    ] {
        assert_eq!(aliases.lookup(key).unwrap(), None);
    }
}
#[test]
fn late_conflict_and_invalid_uri_leave_no_partial_batch_or_capacity_change() {
    let mut aliases = ImportUriAliases::new(limits()).unwrap();
    aliases
        .bind(
            "https://example.test/final.css",
            &["https://example.test/a.css"],
        )
        .unwrap();
    let retained = aliases.retained_bytes();
    assert!(matches!(
        aliases.bind(
            "https://example.test/other.css",
            &["https://example.test/new.css", "https://example.test/a.css"]
        ),
        Err(ImportUriAliasError::Conflict)
    ));
    assert_eq!(
        aliases.lookup("https://example.test/new.css").unwrap(),
        None
    );
    assert_eq!(
        aliases.lookup("https://example.test/other.css").unwrap(),
        None
    );
    assert_eq!(aliases.retained_bytes(), retained);
    assert!(matches!(
        aliases.bind(
            "https://example.test/final.css",
            &["https://example.test/new.css", "relative.css"]
        ),
        Err(ImportUriAliasError::Uri(_))
    ));
    assert_eq!(
        aliases.lookup("https://example.test/new.css").unwrap(),
        None
    );
    assert_eq!(aliases.retained_bytes(), retained);
    assert!(matches!(
        aliases.bind("https://example.test/a.css", &[]),
        Err(ImportUriAliasError::Conflict)
    ));
    assert!(matches!(
        aliases.lookup("https://example.test/a.css#part"),
        Err(ImportUriAliasError::Fragment)
    ));
    aliases
        .bind(
            "https://example.test/final.css",
            &["https://example.test/new.css"],
        )
        .unwrap();
    assert_eq!(aliases.len(), 3);
}
#[test]
fn explicit_alias_budgets_reject_without_changing_previous_evidence() {
    assert!(matches!(
        ImportUriAliases::new(ImportUriAliasLimits {
            bytes: n(usize::MAX),
            ..limits()
        }),
        Err(ImportUriAliasError::Budget(RuleAssemblyError::Bytes))
    ));
    let mut aliases = ImportUriAliases::new(ImportUriAliasLimits {
        entries: n(2),
        ..limits()
    })
    .unwrap();
    aliases.bind("app:/final", &["app:/first"]).unwrap();
    let retained = aliases.retained_bytes();
    assert!(matches!(
        aliases.bind("app:/final", &["app:/new"]),
        Err(ImportUriAliasError::Budget(RuleAssemblyError::Entries))
    ));
    assert_eq!(aliases.lookup("app:/new").unwrap(), None);
    assert_eq!(aliases.retained_bytes(), retained);
    let mut aliases = ImportUriAliases::new(ImportUriAliasLimits {
        bytes: n(1),
        ..limits()
    })
    .unwrap();
    assert!(matches!(
        aliases.bind("app:/final", &[]),
        Err(ImportUriAliasError::Budget(RuleAssemblyError::Bytes))
            | Err(ImportUriAliasError::Uri(_))
    ));
    assert!(aliases.is_empty());
    assert_eq!(aliases.retained_bytes(), 0);
    let mut aliases = ImportUriAliases::new(ImportUriAliasLimits {
        work: n(1),
        ..limits()
    })
    .unwrap();
    assert!(aliases.bind("app:/final", &[]).is_err());
    assert!(aliases.lookup("app:/final").is_err());
    assert!(aliases.is_empty());
}

#[test]
fn late_work_and_storage_failures_preserve_existing_aliases() {
    use iced_css_core::resources::uri::UriError;
    let mut aliases = ImportUriAliases::new(ImportUriAliasLimits {
        work: n(10000),
        ..limits()
    })
    .unwrap();
    aliases.bind("app:/final", &["app:/first"]).unwrap();
    let retained = aliases.retained_bytes();
    let long = format!("app:/{}", "x".repeat(1000));
    assert!(matches!(
        aliases.bind("app:/final", &["app:/new", &long]),
        Err(ImportUriAliasError::Uri(UriError::Work))
    ));
    assert_eq!(aliases.retained_bytes(), retained);
    assert_eq!(aliases.lookup("app:/new").unwrap(), None);
    assert_eq!(aliases.lookup("app:/first").unwrap(), Some("app:/final"));
    let mut limited = ImportUriAliases::new(ImportUriAliasLimits {
        bytes: n(retained + 1),
        ..limits()
    })
    .unwrap();
    limited.bind("app:/final", &["app:/first"]).unwrap();
    let retained = limited.retained_bytes();
    assert!(matches!(
        limited.bind("app:/final", &["app:/new"]),
        Err(ImportUriAliasError::Budget(RuleAssemblyError::Bytes))
    ));
    assert_eq!(limited.retained_bytes(), retained);
    assert_eq!(limited.lookup("app:/new").unwrap(), None);
    assert_eq!(limited.lookup("app:/first").unwrap(), Some("app:/final"));
}
