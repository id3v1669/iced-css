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
    unicode_intake::{IntakeError, IntakeLimits},
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
#[test]
fn unicode_signatures_and_scalar_widths_map_typed_values_to_original_bytes() {
    let text = "/*😀*/\r\n@media screen{a{color:red}}";
    for width in [1, 2, 4] {
        let bytes = match width {
            1 => [b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat(),
            2 => [
                b"\xff\xfe".as_slice(),
                text.encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
                    .as_slice(),
            ]
            .concat(),
            _ => [
                b"\xff\xfe\0\0".as_slice(),
                text.chars()
                    .flat_map(|c| (c as u32).to_le_bytes())
                    .collect::<Vec<_>>()
                    .as_slice(),
            ]
            .concat(),
        };
        let candidate = parse_separate_unicode_stylesheet(
            &bytes,
            EncodingMetadata::default(),
            context(),
            intake_limits(),
        )
        .unwrap();
        let rule = candidate.sheet().local_screen_rules().next().unwrap();
        let original = candidate
            .original_span(rule.locations[0].value.clone())
            .unwrap();
        let prefix = &text[..text.find("red").unwrap()];
        let expected = match width {
            1 => 3 + prefix.len(),
            2 => 2 + prefix.encode_utf16().count() * 2,
            _ => 4 + prefix.chars().count() * 4,
        };
        assert_eq!(original.start.byte, expected);
        assert_eq!(original.end.byte, expected + 3 * width);
        assert_eq!(original.start.line, 2);
        let invalid_range = std::ops::Range { start: 2, end: 1 };
        assert!(candidate.original_span(invalid_range).is_none());
        assert!(candidate.original_span(3..4).is_none()); // inside UTF-8 emoji in decoded text
    }
}
#[test]
fn fatal_property_errors_keep_their_original_byte_range_after_source_is_dropped() {
    let text = "/*😀*/a{font-weight:+100}";
    let bytes: Vec<u8> = b"\xff\xfe"
        .iter()
        .copied()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let Err(StylesheetIntakeError::Assembly {
        error: RuleAssemblyError::Property { .. },
        original: Some(span),
    }) = parse_separate_unicode_stylesheet(
        &bytes,
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    else {
        panic!()
    };
    assert_eq!(
        span.start.byte,
        2 + text[..text.find('+').unwrap()].encode_utf16().count() * 2
    );
    assert_eq!(span.end.byte - span.start.byte, 8);
}
#[test]
fn charset_selection_and_unimplemented_encodings_are_not_reclassified_by_the_parser() {
    let candidate = parse_separate_unicode_stylesheet(
        b"@charset \"UTF-16\";a{color:red}",
        EncodingMetadata {
            transport: Some(b"UTF-8"),
            ..Default::default()
        },
        context(),
        intake_limits(),
    )
    .unwrap();
    assert_eq!(candidate.sheet().local_screen_rules().count(), 1);
    assert!(matches!(
        parse_separate_unicode_stylesheet(
            b"a{}",
            EncodingMetadata {
                transport: Some(b"latin1"),
                ..Default::default()
            },
            context(),
            intake_limits()
        ),
        Err(StylesheetIntakeError::Intake(
            IntakeError::UnimplementedCharset { mib: 4 }
        ))
    ));
    assert!(matches!(
        parse_separate_unicode_stylesheet(
            b"\xff",
            EncodingMetadata::default(),
            context(),
            intake_limits()
        ),
        Err(StylesheetIntakeError::Intake(IntakeError::Decode(_)))
    ));
}

#[test]
fn resolved_imports_keep_referring_encoding_after_parent_is_dropped() {
    use iced_css_core::stylesheet::requests::*;
    let css = "@import '../child.css#part'; @import 'ignored' print; @import '../child.css#other';";
    let bytes: Vec<u8> = css.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let parent = parse_separate_unicode_stylesheet(
        &bytes,
        EncodingMetadata {
            transport: Some(b"UTF-16LE"),
            ..Default::default()
        },
        context(),
        intake_limits(),
    )
    .unwrap();
    let requests = resolve_unicode_import_requests(
        &parent,
        "https://host/css/root.css",
        ImportRequestLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    drop(parent);
    std::thread::spawn(move || {
        assert_eq!(requests.requests().len(), 2);
        assert_eq!(requests.requests()[0].entry, 0);
        assert_eq!(requests.requests()[1].entry, 2);
        assert_eq!(
            requests.requests()[0].reference_uri(),
            "https://host/child.css#part"
        );
        assert_eq!(
            requests.requests()[0].acquisition_uri(),
            requests.requests()[1].acquisition_uri()
        );
        let child = "a{color:red}";
        let bytes: Vec<u8> = child.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let parsed = parse_separate_unicode_stylesheet(
            &bytes,
            requests.encoding_metadata(None, None),
            context(),
            intake_limits(),
        )
        .unwrap();
        assert_eq!(parsed.source().text(), child);
        assert_eq!(parsed.sheet().local_screen_rules().count(), 1);
        for metadata in [
            requests.encoding_metadata(Some(b"UTF-8"), None),
            requests.encoding_metadata(None, Some(b"UTF-8")),
        ] {
            let parsed = parse_separate_unicode_stylesheet(
                child.as_bytes(),
                metadata,
                context(),
                intake_limits(),
            )
            .unwrap();
            assert_eq!(parsed.source().referring_label(), b"UTF-8");
        }
        let parsed = parse_separate_unicode_stylesheet(
            b"\xef\xbb\xbfa{color:red}",
            requests.encoding_metadata(None, None),
            context(),
            intake_limits(),
        )
        .unwrap();
        assert_eq!(parsed.source().referring_label(), b"UTF-8");
    })
    .join()
    .unwrap();
}

#[test]
fn unicode_import_resolution_preserves_budget_and_located_uri_errors() {
    use iced_css_core::stylesheet::requests::*;
    let parent = parse_separate_unicode_stylesheet(
        b"@import 'a'; @import 'b';",
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    .unwrap();
    let limits = ImportRequestLimits {
        entries: NonZeroUsize::MIN,
        retained_bytes: n(),
        work: n(),
    };
    assert!(matches!(
        resolve_unicode_import_requests(&parent, "https://host/root.css", limits),
        Err(ImportRequestError::Budget(RuleAssemblyError::Entries))
    ));
    let limits = ImportRequestLimits {
        entries: n(),
        ..limits
    };
    assert!(matches!(
        resolve_unicode_import_requests(&parent, "relative", limits),
        Err(ImportRequestError::Uri { entry: 0, .. })
    ));
}
