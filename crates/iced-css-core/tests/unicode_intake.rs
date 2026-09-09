use iced_css_core::syntax::{
    decode::{DecodeError, DecodeLimits, SourcePosition},
    encoding_selection::{EncodingMetadata, SelectionLimits},
    unicode_intake::*,
};
use std::num::NonZeroUsize;
fn limits() -> IntakeLimits {
    let work = NonZeroUsize::new(100000).unwrap();
    IntakeLimits {
        selection: SelectionLimits {
            registry_work: work,
            prefix_work: work,
        },
        decode: DecodeLimits {
            entries: work,
            retained_bytes: work,
            work,
        },
        charset_work: work,
    }
}
fn metadata(label: &[u8]) -> EncodingMetadata<'_> {
    EncodingMetadata {
        transport: Some(label),
        ..Default::default()
    }
}
#[test]
fn default_and_bom_intake_rebase_original_positions_and_repairs() {
    let source =
        decode_separate_unicode(b"\xef\xbb\xbf.a\0{}", EncodingMetadata::default(), limits())
            .unwrap();
    assert_eq!(source.text(), ".a�{}");
    assert_eq!(source.signature_bytes(), 3);
    assert_eq!(
        source.position(2),
        Some(SourcePosition {
            byte: 5,
            line: 1,
            column: 3
        })
    );
    assert_eq!(source.nul_repairs().collect::<Vec<_>>(), vec![5..6]);
    let source = decode_separate_unicode(b"a{}", EncodingMetadata::default(), limits()).unwrap();
    assert_eq!(source.text(), "a{}");
    assert_eq!(source.signature_bytes(), 0);
}
#[test]
fn generic_unicode_schemes_use_verified_big_endian_defaults() {
    for (label, bytes) in [
        (b"UTF-16".as_slice(), vec![0, 65]),
        (b"UTF-32", vec![0, 0, 0, 65]),
    ] {
        assert_eq!(
            decode_separate_unicode(&bytes, metadata(label), limits())
                .unwrap()
                .text(),
            "A"
        );
    }
    let bytes: Vec<u8> = "@charset \"UTF-16\";a{}"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        decode_separate_unicode(&bytes, EncodingMetadata::default(), limits())
            .unwrap()
            .text(),
        "@charset \"UTF-16\";a{}"
    );
}
#[test]
fn transport_override_recovers_explicit_endian_and_rebases_decode_failure() {
    let bytes = b"\xff\xfe\x41\0";
    let source = decode_separate_unicode(bytes, metadata(b"UTF-32BE"), limits()).unwrap();
    assert_eq!(source.text(), "A");
    assert!(source.transport_endian_error());
    assert_eq!(source.position(1).unwrap().byte, 4);
    assert!(matches!(
        decode_separate_unicode(b"\xef\xbb\xbf\xff", metadata(b"UTF-16"), limits()),
        Err(IntakeError::Decode(DecodeError::Utf8 {
            valid_up_to: 3,
            ..
        }))
    ));
}
#[test]
fn as_specified_must_decode_to_an_appropriate_initial_declaration() {
    for input in [
        b"@charset \"UTF-8\";a{}".as_slice(),
        b"\xef\xbb\xbf@charset \"csUTF8\";a{}",
    ] {
        assert!(decode_separate_unicode(input, EncodingMetadata::default(), limits()).is_ok());
    }
    assert!(matches!(
        decode_separate_unicode(
            b"@charset \"UTF-16\";",
            EncodingMetadata::default(),
            limits()
        ),
        Err(IntakeError::InappropriateCharset)
    ));
    // Explicit-endian labels do not make their initial FEFF content disappear.
    let bytes: Vec<u8> = "\u{feff}@charset \"UTF-16BE\";"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    assert!(matches!(
        decode_separate_unicode(&bytes, EncodingMetadata::default(), limits()),
        Err(IntakeError::InappropriateCharset)
    ));
    // Transport wins; the declaration is then ordinary ignored charset syntax.
    assert_eq!(
        decode_separate_unicode(b"@charset \"UTF-16\";", metadata(b"UTF-8"), limits())
            .unwrap()
            .text(),
        "@charset \"UTF-16\";"
    );
}
#[test]
fn unknown_known_unimplemented_and_unusual_orders_remain_distinct() {
    assert!(matches!(
        decode_separate_unicode(b"a{}", metadata(b"unknown"), limits()),
        Err(IntakeError::UnknownCharset)
    ));
    assert!(matches!(
        decode_separate_unicode(b"a{}", metadata(b"latin1"), limits()),
        Err(IntakeError::UnimplementedCharset { mib: 4 })
    ));
    assert!(matches!(
        decode_separate_unicode(b"\0\0\xff\xfe", EncodingMetadata::default(), limits()),
        Err(IntakeError::UnsupportedOrder)
    ));
    assert!(matches!(
        decode_separate_unicode(
            b"@charset \"UTF-8\n\";",
            EncodingMetadata::default(),
            limits()
        ),
        Err(IntakeError::UnknownCharset)
    ));
}
#[test]
fn explicit_body_feff_is_preserved_when_selected_without_transport_override() {
    // Linking selection is reached only without a detector match. Here transport
    // selects a UTF-16BE stream whose FEFF follows its first content scalar.
    let source = decode_separate_unicode(b"\0A\xfe\xff", metadata(b"UTF-16BE"), limits()).unwrap();
    assert_eq!(source.text(), "A\u{feff}");
    assert!(!source.transport_endian_error());
    let mut bounded = limits();
    bounded.charset_work = NonZeroUsize::MIN;
    assert!(matches!(
        decode_separate_unicode(b"a{}", metadata(b"UTF-8"), bounded),
        Err(IntakeError::CharsetWork)
    ));
    bounded = limits();
    bounded.decode.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        decode_separate_unicode(b"a{}", EncodingMetadata::default(), bounded),
        Err(IntakeError::Decode(DecodeError::Bytes))
    ));
}

#[test]
fn declared_unicode_scheme_matrix_and_decoded_rule_pipeline() {
    use iced_css_core::{
        selectors::{SelectorLimits, parse_selector_list},
        syntax::{
            DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StatementContext,
            StatementKind, Statements, StringLimits,
        },
        values::{
            NumericLimits,
            color::ColorLimits,
            property::{PropertyResult, parse_property},
        },
    };
    let cases = [
        ("UTF-8", 1, false),
        ("csUTF8", 1, false),
        ("UTF-16", 2, false),
        ("UTF-16", 2, true),
        ("UTF-16BE", 2, false),
        ("UTF-16LE", 2, true),
        ("UTF-32", 4, false),
        ("UTF-32", 4, true),
        ("UTF-32BE", 4, false),
        ("UTF-32LE", 4, true),
    ];
    for (label, width, little) in cases {
        for signature in [false, true] {
            let css = format!("@charset \"{label}\";.a{{width:f(a;b);color:green}}");
            let mut input = match (width, little) {
                (1, _) => css.as_bytes().to_vec(),
                (2, false) => css.encode_utf16().flat_map(u16::to_be_bytes).collect(),
                (2, true) => css.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                (4, false) => css
                    .chars()
                    .flat_map(|c| u32::from(c).to_be_bytes())
                    .collect(),
                (4, true) => css
                    .chars()
                    .flat_map(|c| u32::from(c).to_le_bytes())
                    .collect(),
                _ => unreachable!(),
            };
            if signature {
                let prefix: &[u8] = match (width, little) {
                    (1, _) => b"\xef\xbb\xbf",
                    (2, false) => b"\xfe\xff",
                    (2, true) => b"\xff\xfe",
                    (4, false) => b"\0\0\xfe\xff",
                    (4, true) => b"\xff\xfe\0\0",
                    _ => unreachable!(),
                };
                input.splice(..0, prefix.iter().copied());
            }
            let decoded = decode_separate_unicode(&input, EncodingMetadata::default(), limits());
            if signature && (label.ends_with("BE") || label.ends_with("LE")) {
                assert!(
                    matches!(decoded, Err(IntakeError::InappropriateCharset)),
                    "{label}"
                );
                continue;
            }
            let decoded = decoded.unwrap();
            assert_eq!(decoded.text(), css);
            let work = limits().charset_work;
            let recovery = RecoveryLimits {
                work,
                nesting: work,
                retained_bytes: work,
            };
            let mut statements =
                Statements::new(decoded.text(), StatementContext::Stylesheet, recovery);
            assert!(matches!(
                statements.next().unwrap().unwrap().kind,
                StatementKind::AtRule { .. }
            ));
            let rule = statements.next().unwrap().unwrap();
            assert_eq!(rule.kind, StatementKind::QualifiedRule);
            assert_eq!(
                parse_selector_list(
                    rule.prelude,
                    SelectorLimits {
                        work,
                        retained_bytes: work,
                        entries: work
                    }
                )
                .unwrap()
                .len(),
                1
            );
            let mut accepted = Vec::new();
            let mut discarded = Vec::new();
            for declaration in Declarations::new(
                rule.body.unwrap(),
                DeclarationLimits {
                    recovery,
                    name_bytes: work,
                },
            ) {
                let DeclarationItem::Declaration(declaration) = declaration.unwrap() else {
                    panic!()
                };
                match parse_property(
                    &declaration,
                    ColorLimits {
                        syntax: StringLimits {
                            retained_bytes: work,
                            work,
                        },
                        numeric: NumericLimits { digits: work, work },
                    },
                )
                .unwrap()
                {
                    PropertyResult::Parsed(value) => {
                        accepted.push(value.property.descriptor().name)
                    }
                    PropertyResult::Invalid => discarded.push(declaration.name.text),
                    PropertyResult::Unknown => panic!(),
                }
            }
            assert_eq!(accepted, ["color"]);
            assert_eq!(discarded, ["width"]);
            assert!(statements.next().is_none());
        }
    }
}

#[test]
fn selected_charset_is_retained_separately_from_stream_byte_order() {
    let generic =
        decode_separate_unicode(&[0xff, 0xfe, 65, 0], metadata(b"UTF-16"), limits()).unwrap();
    assert_eq!(generic.text(), "A");
    assert_eq!(generic.referring_label(), b"UTF-16");
    let child = decode_separate_unicode(
        &[0, 66],
        EncodingMetadata {
            referring: Some(generic.referring_label()),
            ..Default::default()
        },
        limits(),
    )
    .unwrap();
    assert_eq!(child.text(), "B");
    assert_eq!(child.referring_label(), b"UTF-16");
    let explicit = decode_separate_unicode(&[65, 0], metadata(b"UTF-16LE"), limits()).unwrap();
    assert_eq!(explicit.referring_label(), b"UTF-16LE");
    let child = decode_separate_unicode(
        &[66, 0],
        EncodingMetadata {
            referring: Some(explicit.referring_label()),
            ..Default::default()
        },
        limits(),
    )
    .unwrap();
    assert_eq!(child.text(), "B");
    let override_child = decode_separate_unicode(
        b"C",
        EncodingMetadata {
            transport: Some(b"UTF-8"),
            referring: Some(explicit.referring_label()),
            ..Default::default()
        },
        limits(),
    )
    .unwrap();
    assert_eq!(override_child.text(), "C");
    assert_eq!(override_child.referring_label(), b"UTF-8");
}
#[test]
fn detection_and_transport_override_retain_resulting_charset_family() {
    for (bytes, label) in [
        (vec![0xff, 0xfe, 65, 0], b"UTF-16".as_slice()),
        (vec![0xff, 0xfe, 0, 0, 65, 0, 0, 0], b"UTF-32".as_slice()),
        (vec![0xef, 0xbb, 0xbf, 65], b"UTF-8".as_slice()),
    ] {
        let source = decode_separate_unicode(&bytes, metadata(b"UTF-8"), limits()).unwrap();
        assert_eq!(source.referring_label(), label);
        assert_eq!(source.text(), "A");
        let detected =
            decode_separate_unicode(&bytes, EncodingMetadata::default(), limits()).unwrap();
        assert_eq!(detected.referring_label(), label);
    }
    let bytes: Vec<u8> = "@charset \"UTF-16\";a{}"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let declared = decode_separate_unicode(&bytes, EncodingMetadata::default(), limits()).unwrap();
    assert_eq!(declared.referring_label(), b"UTF-16");
    let mut tiny = limits();
    tiny.charset_work = NonZeroUsize::MIN;
    assert_eq!(
        decode_separate_unicode(b"a{}", EncodingMetadata::default(), tiny)
            .unwrap()
            .referring_label(),
        b"UTF-8"
    );
}

#[test]
fn canonical_referring_labels_cover_all_supported_unicode_charsets() {
    for (label, canonical, bytes) in [
        (b"csUTF8".as_slice(), b"UTF-8".as_slice(), b"A".as_slice()),
        (b"utf-16", b"UTF-16", &[0, 65]),
        (b"utf-16be", b"UTF-16BE", &[0, 65]),
        (b"utf-16le", b"UTF-16LE", &[65, 0]),
        (b"utf-32", b"UTF-32", &[0, 0, 0, 65]),
        (b"utf-32be", b"UTF-32BE", &[0, 0, 0, 65]),
        (b"utf-32le", b"UTF-32LE", &[65, 0, 0, 0]),
    ] {
        let parent = decode_separate_unicode(bytes, metadata(label), limits()).unwrap();
        assert_eq!(parent.referring_label(), canonical);
        let child = decode_separate_unicode(
            bytes,
            EncodingMetadata {
                referring: Some(parent.referring_label()),
                ..Default::default()
            },
            limits(),
        )
        .unwrap();
        assert_eq!(child.text(), "A");
        assert_eq!(child.referring_label(), canonical);
    }
}
