use iced_css_core::resources::{connection::*, storage_fields::*};
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn parse_options(raw: &[u8]) -> ConnectionOptions<'_> {
    parse_connection_fields(
        &[raw],
        ConnectionLimits {
            fields: n(10),
            bytes: n(10000),
            members: n(20),
            record_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap()
}
fn limits() -> StorageFieldLimits {
    StorageFieldLimits {
        name_bytes: n(1000),
        work: n(10000),
    }
}
#[test]
fn generic_storage_roles_are_exact_and_preserve_raw_nomination_evidence() {
    let options = parse_options(b"x-hop, Cache-Control, *, Keep-Alive");
    let mut classify = StorageFieldClassifier::new(&options, limits());
    assert_eq!(
        classify.classify(b"CoNnEcTiOn"),
        Ok(StorageFieldRole::Connection)
    );
    for name in [
        b"Proxy-Connection".as_slice(),
        b"kEeP-aLiVe",
        b"te",
        b"TRANSFER-ENCODING",
        b"Upgrade",
    ] {
        assert_eq!(
            classify.classify(name),
            Ok(StorageFieldRole::KnownConnectionSpecific)
        );
    }
    for name in [
        b"Proxy-Authenticate".as_slice(),
        b"proxy-authentication-info",
        b"PROXY-AUTHORIZATION",
    ] {
        assert_eq!(classify.classify(name), Ok(StorageFieldRole::ProxySpecific));
    }
    for name in [b"X-HOP".as_slice(), b"cache-control", b"*"] {
        assert_eq!(
            classify.classify(name),
            Ok(StorageFieldRole::ConnectionNominated)
        );
    }
    for name in [
        b"Content-Type".as_slice(),
        b"ETag",
        b"unknown",
        b"Proxy-Unrecognized",
        b"TE-Suffix",
        b"X-Hop-Suffix",
    ] {
        assert_eq!(classify.classify(name), Ok(StorageFieldRole::Candidate));
    }
    assert!(options.nominates(b"cache-control", n(1000)).unwrap());
    let independent = parse_options(b"*");
    let mut classify = StorageFieldClassifier::new(&independent, limits());
    assert_eq!(
        classify.classify(b"Cache-Control"),
        Ok(StorageFieldRole::Candidate)
    );
}
#[test]
fn storage_role_limits_apply_across_calls_and_reject_invalid_names() {
    let options = parse_options(b"long-nominated-name");
    let mut classify = StorageFieldClassifier::new(&options, limits());
    for bad in [b"".as_slice(), b"bad name", b"x\r\ny", b"x\xff"] {
        assert_eq!(classify.classify(bad), Err(StorageFieldError::InvalidName));
    }
    let mut classify = StorageFieldClassifier::new(
        &options,
        StorageFieldLimits {
            name_bytes: n(1),
            ..limits()
        },
    );
    assert_eq!(classify.classify(b"xx"), Err(StorageFieldError::Bytes));
    let mut classify = StorageFieldClassifier::new(
        &options,
        StorageFieldLimits {
            work: n(1000),
            ..limits()
        },
    );
    let initial = classify.remaining_work();
    assert_eq!(
        classify.classify(b"long-nominated-name"),
        Ok(StorageFieldRole::ConnectionNominated)
    );
    assert!(classify.remaining_work() < initial);
    let mut successes = 1;
    loop {
        match classify.classify(b"long-nominated-name") {
            Ok(StorageFieldRole::ConnectionNominated) => {
                successes += 1;
                assert!(successes < 1000);
            }
            Err(StorageFieldError::Work) => break,
            other => panic!("{other:?}"),
        }
    }
    let mut classify = StorageFieldClassifier::new(
        &options,
        StorageFieldLimits {
            work: n(1),
            ..limits()
        },
    );
    assert_eq!(classify.classify(b"x"), Err(StorageFieldError::Work));
}

#[test]
fn nomination_lookup_uses_the_remaining_classifier_budget() {
    let absent = parse_options(b"");
    let mut probe = StorageFieldClassifier::new(&absent, limits());
    assert_eq!(probe.classify(b"x"), Ok(StorageFieldRole::Candidate));
    let fixed_cost = limits().work.get() - probe.remaining_work();
    let cap = StorageFieldLimits {
        work: n(fixed_cost),
        ..limits()
    };
    let mut exact = StorageFieldClassifier::new(&absent, cap);
    assert_eq!(exact.classify(b"x"), Ok(StorageFieldRole::Candidate));
    assert_eq!(exact.remaining_work(), 0);
    let nominated = parse_options(b"other");
    let mut insufficient = StorageFieldClassifier::new(&nominated, cap);
    assert_eq!(insufficient.classify(b"x"), Err(StorageFieldError::Work));
    assert_eq!(insufficient.remaining_work(), 0);
}

#[test]
fn update_actions_protect_only_actual_processing_dependencies() {
    let options = parse_options(b"x-hop");
    for stage in [
        StoredProcessing::CodedBytes,
        StoredProcessing::ContentDecoded,
        StoredProcessing::StylesheetPrepared,
    ] {
        let mut classify = StorageFieldClassifier::new(&options, limits());
        assert_eq!(
            classify.classify_update(b"cOnTeNt-LeNgTh", stage),
            Ok(HeaderUpdateAction::PreserveStored)
        );
        assert_eq!(
            classify.classify_update(b"Content-Encoding", stage),
            Ok(if stage == StoredProcessing::CodedBytes {
                HeaderUpdateAction::ReplaceFromReceived
            } else {
                HeaderUpdateAction::PreserveStored
            })
        );
        assert_eq!(
            classify.classify_update(b"Content-Type", stage),
            Ok(if stage == StoredProcessing::StylesheetPrepared {
                HeaderUpdateAction::PreserveStored
            } else {
                HeaderUpdateAction::ReplaceFromReceived
            })
        );
        for name in [
            b"Content-Language".as_slice(),
            b"Content-Location",
            b"ETag",
            b"Date",
            b"Cache-Control",
            b"Unrecognized",
        ] {
            assert_eq!(
                classify.classify_update(name, stage),
                Ok(HeaderUpdateAction::ReplaceFromReceived)
            );
        }
        assert_eq!(
            classify.classify_update(b"Connection", stage),
            Ok(HeaderUpdateAction::Exclude(StorageFieldRole::Connection))
        );
        assert_eq!(
            classify.classify_update(b"X-Hop", stage),
            Ok(HeaderUpdateAction::Exclude(
                StorageFieldRole::ConnectionNominated
            ))
        );
        assert_eq!(
            classify.classify_update(b"TE", stage),
            Ok(HeaderUpdateAction::Exclude(
                StorageFieldRole::KnownConnectionSpecific
            ))
        );
        assert_eq!(
            classify.classify_update(b"Proxy-Authorization", stage),
            Ok(HeaderUpdateAction::Exclude(StorageFieldRole::ProxySpecific))
        );
    }
}
#[test]
fn update_actions_do_not_reset_work_after_storage_classification() {
    let options = parse_options(b"");
    let mut probe = StorageFieldClassifier::new(&options, limits());
    assert_eq!(
        probe.classify(b"Content-Length"),
        Ok(StorageFieldRole::Candidate)
    );
    let cost = limits().work.get() - probe.remaining_work();
    let mut bounded = StorageFieldClassifier::new(
        &options,
        StorageFieldLimits {
            work: n(cost),
            ..limits()
        },
    );
    assert_eq!(
        bounded.classify_update(b"Content-Length", StoredProcessing::CodedBytes),
        Err(StorageFieldError::Work)
    );
}
