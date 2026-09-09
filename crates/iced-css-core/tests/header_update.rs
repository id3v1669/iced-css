use iced_css_core::resources::{connection::*, header_update::*, storage_fields::*};
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn field<'a>(name: &'a [u8], value: &'a [u8]) -> HeaderField<'a> {
    HeaderField::new(
        name,
        value,
        HeaderFieldLimits {
            bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap()
}
fn options(raw: &[u8]) -> ConnectionOptions<'_> {
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
fn classifier<'a, 'b>(options: &'a ConnectionOptions<'b>) -> StorageFieldClassifier<'a, 'b> {
    StorageFieldClassifier::new(
        options,
        StorageFieldLimits {
            name_bytes: n(1000),
            work: n(10000),
        },
    )
}
fn limits() -> HeaderUpdateLimits {
    HeaderUpdateLimits {
        fields: n(100),
        input_bytes: n(10000),
        record_bytes: n(10000),
        work: n(10000),
    }
}
#[test]
fn header_fields_keep_raw_spans_but_exclude_outer_ows_from_value_views() {
    let parsed = field(b"X-Example", b" \tvalue \t");
    assert_eq!(parsed.name(), b"X-Example");
    assert_eq!(parsed.raw_value(), b" \tvalue \t");
    assert_eq!(parsed.value(), b"value");
    assert_eq!(parsed.value_span(), 2..7);
    assert_eq!(field(b"X", b" \t").value(), b"");
    for byte in 0u8..=255 {
        let raw = [byte];
        let allowed = byte == b'\t' || (b' '..=b'~').contains(&byte) || byte >= 128;
        assert_eq!(
            HeaderField::new(
                b"X",
                &raw,
                HeaderFieldLimits {
                    bytes: n(1000),
                    work: n(1000)
                }
            )
            .is_ok(),
            allowed,
            "{byte}"
        );
    }
    assert!(matches!(
        HeaderField::new(
            b"bad name",
            b"x",
            HeaderFieldLimits {
                bytes: n(1000),
                work: n(1000)
            }
        ),
        Err(HeaderFieldError::InvalidName { offset: 3 })
    ));
    assert!(matches!(
        HeaderField::new(
            b"X",
            b"a\rb",
            HeaderFieldLimits {
                bytes: n(1000),
                work: n(1000)
            }
        ),
        Err(HeaderFieldError::InvalidValue { offset: 1 })
    ));
    assert!(matches!(
        HeaderField::new(
            b"X",
            b"a",
            HeaderFieldLimits {
                bytes: n(1),
                work: n(1000)
            }
        ),
        Err(HeaderFieldError::Bytes)
    ));
    assert!(matches!(
        HeaderField::new(
            b"X",
            b"a",
            HeaderFieldLimits {
                bytes: n(1000),
                work: n(1)
            }
        ),
        Err(HeaderFieldError::Work)
    ));
}
#[test]
fn prepared_update_preserves_received_repetitions_and_processing_metadata() {
    let stored = [
        field(b"X-Keep", b" old "),
        field(b"X-Repeat", b"old-a"),
        field(b"x-repeat", b"old-b"),
        field(b"Content-Length", b"17"),
        field(b"Content-Encoding", b"gzip"),
        field(b"Content-Type", b"text/css; charset=UTF-16LE"),
        field(b"Content-Language", b"en"),
        field(b"X-Old-Hop", b"gone"),
        field(b"Connection", b"x-old-hop"),
        field(b"Proxy-Authenticate", b"gone"),
    ];
    let received = [
        field(b"x-repeat", b" new-a "),
        field(b"X-Repeat", b"new-b"),
        field(b"Content-Length", b"999"),
        field(b"Content-Encoding", b"br"),
        field(b"Content-Type", b"text/plain"),
        field(b"content-language", b"fr"),
        field(b"X-New", b"added"),
        field(b"X-New-Hop", b"gone"),
        field(b"Connection", b"x-new-hop"),
        field(b"Transfer-Encoding", b"chunked"),
    ];
    let old_options = options(b"x-old-hop");
    let new_options = options(b"x-new-hop");
    let plan = prepare_header_update(
        &stored,
        &received,
        &mut classifier(&old_options),
        &mut classifier(&new_options),
        StoredProcessing::StylesheetPrepared,
        limits(),
    )
    .unwrap();
    let actual: Vec<_> = plan
        .fields()
        .iter()
        .map(|entry| {
            (
                entry.origin(),
                std::str::from_utf8(entry.field().name()).unwrap(),
                std::str::from_utf8(entry.field().value()).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (HeaderOrigin::Stored(0), "X-Keep", "old"),
            (HeaderOrigin::Stored(3), "Content-Length", "17"),
            (HeaderOrigin::Stored(4), "Content-Encoding", "gzip"),
            (
                HeaderOrigin::Stored(5),
                "Content-Type",
                "text/css; charset=UTF-16LE"
            ),
            (HeaderOrigin::Received(0), "x-repeat", "new-a"),
            (HeaderOrigin::Received(1), "X-Repeat", "new-b"),
            (HeaderOrigin::Received(5), "content-language", "fr"),
            (HeaderOrigin::Received(6), "X-New", "added")
        ]
    );
    assert!(plan.retained_bytes() <= limits().record_bytes.get());
    assert_eq!(stored[1].value(), b"old-a");
    assert_eq!(received[0].raw_value(), b" new-a ");
}
#[test]
fn preparation_handles_each_processing_stage_and_never_fabricates_protected_values() {
    let options = options(b"");
    let stored = [
        field(b"Content-Length", b"17"),
        field(b"Content-Encoding", b"gzip"),
        field(b"Content-Type", b"text/css"),
    ];
    let received = [
        field(b"Content-Length", b"999"),
        field(b"Content-Encoding", b"br"),
        field(b"Content-Type", b"text/plain"),
    ];
    for (stage, encoding, media) in [
        (
            StoredProcessing::CodedBytes,
            b"br".as_slice(),
            b"text/plain".as_slice(),
        ),
        (StoredProcessing::ContentDecoded, b"gzip", b"text/plain"),
        (StoredProcessing::StylesheetPrepared, b"gzip", b"text/css"),
    ] {
        let plan = prepare_header_update(
            &stored,
            &received,
            &mut classifier(&options),
            &mut classifier(&options),
            stage,
            limits(),
        )
        .unwrap();
        for (name, value) in [
            (b"Content-Length".as_slice(), b"17".as_slice()),
            (b"Content-Encoding", encoding),
            (b"Content-Type", media),
        ] {
            assert_eq!(
                plan.fields()
                    .iter()
                    .find(|entry| entry.field().name() == name)
                    .unwrap()
                    .field()
                    .value(),
                value
            );
        }
    }
    let absent = prepare_header_update(
        &[],
        &received,
        &mut classifier(&options),
        &mut classifier(&options),
        StoredProcessing::StylesheetPrepared,
        limits(),
    )
    .unwrap();
    assert!(absent.fields().is_empty());
}
#[test]
fn preparation_limits_and_classifier_errors_leave_inputs_unchanged() {
    let options = options(b"");
    let stored = [field(b"X", b"old")];
    let received = [field(b"X", b"new")];
    for (cap, expected) in [
        (
            HeaderUpdateLimits {
                fields: n(1),
                ..limits()
            },
            HeaderUpdateError::Fields,
        ),
        (
            HeaderUpdateLimits {
                input_bytes: n(1),
                ..limits()
            },
            HeaderUpdateError::InputBytes,
        ),
        (
            HeaderUpdateLimits {
                record_bytes: n(1),
                ..limits()
            },
            HeaderUpdateError::Records,
        ),
        (
            HeaderUpdateLimits {
                work: n(1),
                ..limits()
            },
            HeaderUpdateError::Work,
        ),
    ] {
        let result = prepare_header_update(
            &stored,
            &received,
            &mut classifier(&options),
            &mut classifier(&options),
            StoredProcessing::CodedBytes,
            cap,
        );
        assert_eq!(result.unwrap_err(), expected);
    }
    let mut exhausted = StorageFieldClassifier::new(
        &options,
        StorageFieldLimits {
            name_bytes: n(1000),
            work: n(1),
        },
    );
    assert_eq!(
        prepare_header_update(
            &stored,
            &received,
            &mut classifier(&options),
            &mut exhausted,
            StoredProcessing::CodedBytes,
            limits()
        )
        .unwrap_err(),
        HeaderUpdateError::Classification {
            origin: HeaderOrigin::Received(0),
            error: StorageFieldError::Work
        }
    );
    assert_eq!(stored[0].value(), b"old");
    assert_eq!(received[0].value(), b"new");
}

#[test]
fn late_preparation_budget_failure_exposes_no_partial_list() {
    let options = options(b"");
    let stored = [field(b"X", b"old-x"), field(b"Y", b"old-y")];
    let received = [field(b"X", b"new-x"), field(b"Y", b"new-y")];
    let mut previous_remaining = None;
    let mut succeeded = false;
    for work in 1..=4096 {
        let mut old_classifier = classifier(&options);
        let mut new_classifier = classifier(&options);
        let result = prepare_header_update(
            &stored,
            &received,
            &mut old_classifier,
            &mut new_classifier,
            StoredProcessing::CodedBytes,
            HeaderUpdateLimits {
                work: n(work),
                ..limits()
            },
        );
        match result {
            Err(HeaderUpdateError::Work) => {
                previous_remaining = Some(new_classifier.remaining_work())
            }
            Ok(plan) => {
                // The preceding failure had already classified all incoming fields.
                assert_eq!(previous_remaining, Some(new_classifier.remaining_work()));
                assert_eq!(
                    plan.fields()
                        .iter()
                        .map(|entry| entry.field().value())
                        .collect::<Vec<_>>(),
                    vec![b"new-x".as_slice(), b"new-y"]
                );
                succeeded = true;
                break;
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(stored[0].value(), b"old-x");
        assert_eq!(stored[1].value(), b"old-y");
    }
    assert!(succeeded);
    let empty = prepare_header_update(
        &[],
        &[],
        &mut classifier(&options),
        &mut classifier(&options),
        StoredProcessing::CodedBytes,
        HeaderUpdateLimits {
            fields: n(1),
            input_bytes: n(1),
            record_bytes: n(1),
            work: n(1),
        },
    )
    .unwrap();
    assert!(empty.fields().is_empty());
    assert_eq!(empty.retained_bytes(), 0);
}

#[test]
fn owned_update_survives_source_drop_and_preserves_raw_bytes_and_spans() {
    let owned = {
        let name = b"X-Owned".to_vec();
        let mut value = Vec::with_capacity(2048);
        value.extend_from_slice(b" \towned \t");
        let received = [field(&name, &value)];
        let options = options(b"");
        let prepared = prepare_header_update(
            &[],
            &received,
            &mut classifier(&options),
            &mut classifier(&options),
            StoredProcessing::CodedBytes,
            limits(),
        )
        .unwrap();
        prepared
            .try_to_owned(HeaderOwnershipLimits {
                entries: n(1),
                retained_bytes: n(1000),
                work: n(1000),
            })
            .unwrap()
    };
    fn send_sync<T: Send + Sync>(_: &T) {}
    send_sync(&owned);
    assert_eq!(owned.fields().len(), 1);
    let entry = &owned.fields()[0];
    assert_eq!(entry.origin(), HeaderOrigin::Received(0));
    assert_eq!(entry.field().name(), b"X-Owned");
    assert_eq!(entry.field().raw_value(), b" \towned \t");
    assert_eq!(entry.field().value(), b"owned");
    assert_eq!(entry.field().value_span(), 2..7);
    assert!(owned.retained_bytes() <= 1000);
}
#[test]
fn header_ownership_caps_fail_without_consuming_prepared_headers() {
    let options = options(b"");
    let stored = [field(b"A", b"one"), field(b"B", b"\xff")];
    let prepared = prepare_header_update(
        &stored,
        &[],
        &mut classifier(&options),
        &mut classifier(&options),
        StoredProcessing::CodedBytes,
        limits(),
    )
    .unwrap();
    let cap = HeaderOwnershipLimits {
        entries: n(10),
        retained_bytes: n(1000),
        work: n(1000),
    };
    assert_eq!(
        prepared
            .try_to_owned(HeaderOwnershipLimits {
                entries: n(1),
                ..cap
            })
            .unwrap_err(),
        HeaderOwnershipError::Entries
    );
    assert_eq!(
        prepared
            .try_to_owned(HeaderOwnershipLimits {
                retained_bytes: n(1),
                ..cap
            })
            .unwrap_err(),
        HeaderOwnershipError::Bytes
    );
    assert_eq!(
        prepared
            .try_to_owned(HeaderOwnershipLimits { work: n(1), ..cap })
            .unwrap_err(),
        HeaderOwnershipError::Work
    );
    let owned = prepared.try_to_owned(cap).unwrap();
    assert_eq!(owned.fields()[1].field().value(), b"\xff");
    assert_eq!(prepared.fields()[0].field().value(), b"one");
    let empty = prepare_header_update(
        &[],
        &[],
        &mut classifier(&options),
        &mut classifier(&options),
        StoredProcessing::CodedBytes,
        limits(),
    )
    .unwrap()
    .try_to_owned(HeaderOwnershipLimits {
        entries: n(1),
        retained_bytes: n(1),
        work: n(1),
    })
    .unwrap();
    assert!(empty.fields().is_empty());
    assert_eq!(empty.retained_bytes(), 0);
}
