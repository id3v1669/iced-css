use iced_css_core::source::{AttributeError, AttributeLimits, Metadata};
use std::num::NonZeroUsize;
fn limits() -> AttributeLimits {
    AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    }
}

#[test]
fn bounded_writes_share_id_class_slots_and_replace_in_sequence() {
    let mut metadata = Metadata::default();
    assert!(metadata.try_set_attribute("id", "first", limits()).unwrap());
    assert!(!metadata.try_set_attribute("id", "first", limits()).unwrap());
    assert!(metadata.try_set_attribute("id", "", limits()).unwrap());
    assert_eq!(metadata.css_id(), None);
    assert_eq!(metadata.attribute("id"), Some(""));
    metadata
        .try_set_classes(&["one", "two", "one"], limits())
        .unwrap();
    assert_eq!(metadata.attribute("class"), Some("one two one"));
    assert!(metadata.has_class("two"));
    metadata
        .try_set_attribute("class", "raw\tvalue", limits())
        .unwrap();
    assert!(metadata.has_class("raw"));
    assert!(!metadata.has_class("one"));
    metadata.try_set_classes(&[], limits()).unwrap();
    assert_eq!(metadata.attribute("class"), Some(""));
    metadata
        .try_set_attribute("ID", "different", limits())
        .unwrap();
    assert_eq!(metadata.css_id(), None);
    assert_eq!(metadata.attribute("ID"), Some("different"));
}

#[test]
fn rejected_writes_preserve_existing_values_and_capacity() {
    let mut metadata = Metadata::default();
    metadata.try_set_attribute("id", "kept", limits()).unwrap();
    let pointer = metadata.attribute("id").unwrap().as_ptr();
    let bytes = metadata.retained_bytes();
    for error in [
        AttributeError::Entries,
        AttributeError::Bytes,
        AttributeError::Work,
    ] {
        let mut small = limits();
        match error {
            AttributeError::Entries => small.entries = NonZeroUsize::new(1).unwrap(),
            AttributeError::Bytes => small.retained_bytes = NonZeroUsize::new(1).unwrap(),
            AttributeError::Work => small.work = NonZeroUsize::new(1).unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            metadata.try_set_attribute("class", "new", small),
            Err(error)
        );
        assert_eq!(metadata.css_id(), Some("kept"));
        assert_eq!(metadata.attribute("class"), None);
        assert_eq!(metadata.attribute("id").unwrap().as_ptr(), pointer);
        assert_eq!(metadata.retained_bytes(), bytes);
    }
    for token in ["", "has space", "tab\t", "lf\n", "cr\r", "ff\u{c}"] {
        assert_eq!(
            metadata.try_set_classes(&["valid", token], limits()),
            Err(AttributeError::InvalidClassToken)
        );
        assert_eq!(metadata.attribute("class"), None);
    }
    metadata
        .try_set_classes(&["non\u{a0}breaking"], limits())
        .unwrap();
    assert!(metadata.has_class("non\u{a0}breaking"));
}

#[test]
fn replacement_keeps_unrelated_owned_strings_and_works_at_entry_limit() {
    let mut metadata = Metadata::default();
    metadata.set_attribute("kept", "allocation");
    metadata.set_attribute("replace", "old");
    let pointer = metadata.attribute("kept").unwrap().as_ptr();
    let mut exact = limits();
    exact.entries = NonZeroUsize::new(2).unwrap();
    metadata
        .try_set_attribute("replace", "longer replacement", exact)
        .unwrap();
    assert_eq!(metadata.attribute("kept").unwrap().as_ptr(), pointer);
    assert_eq!(metadata.attribute("replace"), Some("longer replacement"));
}
