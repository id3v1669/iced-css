use iced_css_core::resources::{vary::*, vary_request::*};
use std::num::NonZeroUsize;
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn vary(input: &[u8]) -> Vary<'_> {
    parse_vary_fields(
        &[input],
        VaryLimits {
            fields: n(10),
            bytes: n(10000),
            members: n(20),
            record_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap()
}
fn limits() -> VaryRequestLimits {
    VaryRequestLimits {
        fields: n(20),
        bytes: n(10000),
        work: n(10000),
    }
}
fn field<'a>(name: &'a [u8], value: &'a [u8]) -> RequestField<'a> {
    RequestField { name, value }
}
#[test]
fn selected_request_lines_preserve_absence_values_and_per_name_order() {
    assert_eq!(
        compare_vary_exact(
            &vary(b"x"),
            &[field(b"x", b" \ta \t")],
            &[field(b"X", b"a")],
            limits()
        ),
        Ok(VaryComparison::ExactMatch)
    );
    assert_eq!(
        compare_vary_exact(
            &vary(b"x"),
            &[field(b"x", b" \t")],
            &[field(b"X", b"")],
            limits()
        ),
        Ok(VaryComparison::ExactMatch)
    );
    let selected = vary(b"Accept, X-Choice, ACCEPT");
    let stored = [
        field(b"Accept", b"text/css"),
        field(b"Ignored", b"old"),
        field(b"ACCEPT", b"text/plain"),
    ];
    let incoming = [
        field(b"IGNORED", b"new"),
        field(b"accept", b"text/css"),
        field(b"Accept", b"text/plain"),
    ];
    assert_eq!(
        compare_vary_exact(&selected, &stored, &incoming, limits()),
        Ok(VaryComparison::ExactMatch)
    );
    let reordered = [
        field(b"accept", b"text/plain"),
        field(b"Accept", b"text/css"),
    ];
    assert_eq!(
        compare_vary_exact(&selected, &stored, &reordered, limits()),
        Ok(VaryComparison::DifferentFieldLines)
    );
    let combined = [field(b"Accept", b"text/css, text/plain")];
    assert_eq!(
        compare_vary_exact(&selected, &stored, &combined, limits()),
        Ok(VaryComparison::DifferentFieldLines)
    );
    assert_eq!(
        compare_vary_exact(&vary(b"x"), &[], &[], limits()),
        Ok(VaryComparison::ExactMatch)
    );
    assert_eq!(
        compare_vary_exact(&vary(b"x"), &[], &[field(b"X", b"")], limits()),
        Ok(VaryComparison::DifferentFieldLines)
    );
    assert_eq!(
        compare_vary_exact(
            &vary(b"x"),
            &[field(b"x", b"A")],
            &[field(b"x", b"a")],
            limits()
        ),
        Ok(VaryComparison::DifferentFieldLines)
    );
    assert_eq!(
        compare_vary_exact(
            &vary(b"x"),
            &[field(b"x", b"a b")],
            &[field(b"x", b"a\tb")],
            limits()
        ),
        Ok(VaryComparison::DifferentFieldLines)
    );
    assert_eq!(
        compare_vary_exact(&vary(b"Accept,*"), &stored, &stored, limits()),
        Ok(VaryComparison::Wildcard)
    );
    assert_eq!(
        compare_vary_exact(&vary(b""), &stored, &combined, limits()),
        Ok(VaryComparison::ExactMatch)
    );
    let absent = parse_vary_fields(
        &[],
        VaryLimits {
            fields: n(1),
            bytes: n(1),
            members: n(1),
            record_bytes: n(1000),
            work: n(1000),
        },
    )
    .unwrap();
    assert_eq!(
        compare_vary_exact(&absent, &stored, &combined, limits()),
        Ok(VaryComparison::ExactMatch)
    );
}
#[test]
fn request_field_validation_and_caps_apply_even_without_selected_fields() {
    let selected = vary(b"*");
    for byte in 0u8..=255 {
        let value = [byte];
        let allowed = byte == b'\t' || (b' '..=b'~').contains(&byte) || byte >= 128;
        assert_eq!(
            compare_vary_exact(&selected, &[field(b"x", &value)], &[], limits()).is_ok(),
            allowed,
            "{byte}"
        );
    }
    assert!(matches!(
        compare_vary_exact(&selected, &[field(b"", b"")], &[], limits()),
        Err(VaryRequestError::InvalidName {
            side: RequestSide::Stored,
            index: 0
        })
    ));
    assert!(matches!(
        compare_vary_exact(&selected, &[], &[field(b"bad name", b"")], limits()),
        Err(VaryRequestError::InvalidName {
            side: RequestSide::Incoming,
            index: 0
        })
    ));
    assert!(matches!(
        compare_vary_exact(&vary(b""), &[], &[field(b"x", b"a\rb")], limits()),
        Err(VaryRequestError::InvalidValue {
            side: RequestSide::Incoming,
            index: 0
        })
    ));
    let stored = [field(b"x", b"abc")];
    assert_eq!(
        compare_vary_exact(
            &selected,
            &stored,
            &stored,
            VaryRequestLimits {
                fields: n(1),
                ..limits()
            }
        ),
        Err(VaryRequestError::Fields)
    );
    assert_eq!(
        compare_vary_exact(
            &selected,
            &stored,
            &stored,
            VaryRequestLimits {
                bytes: n(7),
                ..limits()
            }
        ),
        Err(VaryRequestError::Bytes)
    );
    assert_eq!(
        compare_vary_exact(
            &vary(b"x"),
            &stored,
            &stored,
            VaryRequestLimits {
                work: n(1),
                ..limits()
            }
        ),
        Err(VaryRequestError::Work)
    );
}

#[test]
fn selected_value_comparison_spends_work_after_input_admission() {
    let stored = [field(b"x", b"abc")];
    let cap = VaryRequestLimits {
        work: n(20),
        ..limits()
    };
    assert_eq!(
        compare_vary_exact(&vary(b"*"), &stored, &stored, cap),
        Ok(VaryComparison::Wildcard)
    );
    assert_eq!(
        compare_vary_exact(&vary(b"x"), &stored, &stored, cap),
        Err(VaryRequestError::Work)
    );
    assert_eq!(
        compare_vary_exact(&vary(b"x"), &stored, &stored, limits()),
        Ok(VaryComparison::ExactMatch)
    );
    for byte in 0u8..=255 {
        let name = [byte];
        let allowed = byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
        assert_eq!(
            compare_vary_exact(&vary(b""), &[field(&name, b"")], &[], limits()).is_ok(),
            allowed,
            "{byte}"
        );
    }
}
