use iced_css_core::syntax::{
    encoding::{ByteOrder, EncodingHint},
    encoding_selection::*,
};
use std::num::NonZeroUsize;
fn limits() -> SelectionLimits {
    SelectionLimits {
        registry_work: NonZeroUsize::new(10000).unwrap(),
        prefix_work: NonZeroUsize::new(10000).unwrap(),
    }
}
#[test]
fn separate_file_priority_preserves_actual_metadata_presence() {
    let mut metadata = EncodingMetadata {
        transport: Some(b"ISO-8859-1"),
        linking: Some(b"UTF-16"),
        referring: Some(b"UTF-32"),
    };
    let input = b"@charset \"UTF-8\";";
    let selected = select_separate_encoding(input, metadata, limits()).unwrap();
    assert_eq!(selected.origin, EncodingOrigin::Transport);
    assert!(matches!(
        selected.choice,
        EncodingChoice::Label(b"ISO-8859-1")
    ));
    metadata.transport = None;
    let selected = select_separate_encoding(input, metadata, limits()).unwrap();
    assert_eq!(selected.origin, EncodingOrigin::Detection);
    assert!(matches!(selected.choice, EncodingChoice::Detected(_)));
    let selected = select_separate_encoding(b"a{}", metadata, limits()).unwrap();
    assert_eq!(selected.origin, EncodingOrigin::Linking);
    assert!(matches!(selected.choice, EncodingChoice::Label(b"UTF-16")));
    metadata.linking = None;
    let selected = select_separate_encoding(b"a{}", metadata, limits()).unwrap();
    assert_eq!(selected.origin, EncodingOrigin::Referring);
    assert!(matches!(selected.choice, EncodingChoice::Label(b"UTF-32")));
    metadata.referring = None;
    let selected = select_separate_encoding(b"a{}", metadata, limits()).unwrap();
    assert_eq!(selected.origin, EncodingOrigin::Default);
    assert!(matches!(selected.choice, EncodingChoice::DefaultUtf8));
}
#[test]
fn unknown_and_empty_selected_labels_do_not_fall_through() {
    for label in [b"".as_slice(), b"not-a-registered-charset", b"utf8"] {
        for origin in [
            EncodingOrigin::Transport,
            EncodingOrigin::Linking,
            EncodingOrigin::Referring,
        ] {
            let mut metadata = EncodingMetadata::default();
            match origin {
                EncodingOrigin::Transport => metadata.transport = Some(label),
                EncodingOrigin::Linking => metadata.linking = Some(label),
                EncodingOrigin::Referring => metadata.referring = Some(label),
                _ => unreachable!(),
            }
            let selected = select_separate_encoding(b"a{}", metadata, limits()).unwrap();
            assert_eq!(selected.origin, origin);
            assert!(matches!(selected.choice, EncodingChoice::Label(value) if value == label));
        }
    }
}
#[test]
fn only_the_transport_unicode_class_invokes_its_bom_table() {
    for label in [
        b"csUTF8".as_slice(),
        b"uTf-16",
        b"UTF-32",
        b"UTF-16BE",
        b"UTF-32LE",
    ] {
        let metadata = EncodingMetadata {
            transport: Some(label),
            ..Default::default()
        };
        let selected = select_separate_encoding(b"\xfe\xff\0\0", metadata, limits()).unwrap();
        assert_eq!(selected.origin, EncodingOrigin::Transport);
        let EncodingChoice::TransportBom(overridden) = selected.choice else {
            panic!()
        };
        assert_eq!(overridden.encoding, EncodingHint::Utf16(ByteOrder::Big));
        assert_eq!(overridden.bom_bytes, 2);
        assert_eq!(
            overridden.explicit_endian_error,
            matches!(label, b"UTF-16BE" | b"UTF-32LE")
        );
    }
    for label in [b"latin1".as_slice(), b"UTF-7", b"unknown", b""] {
        let metadata = EncodingMetadata {
            transport: Some(label),
            ..Default::default()
        };
        let selected = select_separate_encoding(b"\xef\xbb\xbf", metadata, limits()).unwrap();
        assert!(matches!(selected.choice, EncodingChoice::Label(value) if value == label));
    }
}
#[test]
fn declaration_detection_retains_the_selected_name_and_order_without_accepting_it() {
    let mut input = vec![0xff, 0xfe];
    input.extend(
        "@charset \"UTF-16\";a{}"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let selected = select_separate_encoding(&input, EncodingMetadata::default(), limits()).unwrap();
    let EncodingChoice::Detected(prefix) = selected.choice else {
        panic!()
    };
    assert_eq!(
        prefix.hint,
        EncodingHint::Declared {
            default_order: Some(ByteOrder::Little)
        }
    );
    assert_eq!(prefix.name.unwrap().bytes().collect::<Vec<_>>(), b"UTF-16");
    let selected = select_separate_encoding(
        b"@charset \"unknown\";",
        EncodingMetadata::default(),
        limits(),
    )
    .unwrap();
    assert!(matches!(selected.choice, EncodingChoice::Detected(_)));
}
#[test]
fn work_errors_are_distinct_and_unreached_stages_spend_nothing() {
    let mut bounded = limits();
    bounded.registry_work = NonZeroUsize::MIN;
    assert!(matches!(
        select_separate_encoding(
            b"a{}",
            EncodingMetadata {
                transport: Some(b"UTF-8"),
                ..Default::default()
            },
            bounded
        ),
        Err(SelectionError::RegistryWork)
    ));
    assert!(select_separate_encoding(b"a{}", EncodingMetadata::default(), bounded).is_ok());
    bounded = limits();
    bounded.prefix_work = NonZeroUsize::MIN;
    assert!(matches!(
        select_separate_encoding(b"a{}", EncodingMetadata::default(), bounded),
        Err(SelectionError::PrefixWork)
    ));
    assert!(
        select_separate_encoding(
            b"a{}",
            EncodingMetadata {
                transport: Some(b"UTF-8"),
                ..Default::default()
            },
            bounded
        )
        .is_ok()
    );
}
