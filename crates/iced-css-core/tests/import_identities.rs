use iced_css_core::{
    stylesheet::{
        RuleContext,
        identities::{IdentityLimits, ImportIdentityRegistry},
    },
    syntax::encoding_selection::EncodingMetadata,
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> IdentityLimits {
    IdentityLimits {
        entries: n(100),
        bytes: n(100000),
        work: n(100000),
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 0,
        first_declaration: NonZeroU64::MIN,
    }
}
#[test]
fn identities_share_cycles_without_erasing_decode_context() {
    let mut ids = ImportIdentityRegistry::new(limits()).unwrap();
    let base = EncodingMetadata::default();
    let first = ids.identify("file:///a.css", base, context()).unwrap();
    assert_eq!(
        first,
        ids.identify("file:///a.css", base, context()).unwrap()
    );
    for metadata in [
        EncodingMetadata {
            referring: Some(b"UTF-8"),
            ..base
        },
        EncodingMetadata {
            referring: Some(b"UTF-16LE"),
            ..base
        },
        EncodingMetadata {
            transport: Some(b""),
            ..base
        },
        EncodingMetadata {
            linking: Some(b""),
            ..base
        },
    ] {
        let other = ids.identify("file:///a.css", metadata, context()).unwrap();
        assert_eq!(other.cycle, first.cycle);
        assert_ne!(other.source, first.source);
        assert_eq!(
            other,
            ids.identify("file:///a.css", metadata, context()).unwrap()
        );
    }
    let other = ids.identify("file:///b.css", base, context()).unwrap();
    assert_ne!(other.cycle, first.cycle);
    assert_ne!(other.source, first.source);
    let shifted = ids
        .identify(
            "file:///a.css",
            base,
            RuleContext {
                first_order: 1,
                ..context()
            },
        )
        .unwrap();
    assert_eq!(shifted.cycle, first.cycle);
    assert_ne!(shifted.source, first.source);
    // Exact keys: syntax equivalence is an explicit caller policy.
    assert_ne!(
        ids.identify("FILE:///a.css", base, context())
            .unwrap()
            .cycle,
        first.cycle
    );
}
#[test]
fn admission_owns_metadata_and_does_not_consume_identity_on_failure() {
    let mut ids = ImportIdentityRegistry::new(IdentityLimits {
        entries: n(2),
        ..limits()
    })
    .unwrap();
    let label = b"UTF-8".to_vec();
    let first = ids
        .identify(
            "asset:root",
            EncodingMetadata {
                referring: Some(&label),
                ..Default::default()
            },
            context(),
        )
        .unwrap();
    drop(label);
    for uri in ["relative", "asset:root#", "asset:é", "asset:%gg"] {
        assert!(
            ids.identify(uri, EncodingMetadata::default(), context())
                .is_err()
        );
    }
    assert_eq!(
        first,
        ids.identify(
            "asset:root",
            EncodingMetadata {
                referring: Some(b"UTF-8"),
                ..Default::default()
            },
            context()
        )
        .unwrap()
    );
    let second = ids
        .identify("asset:child", EncodingMetadata::default(), context())
        .unwrap();
    assert_eq!(second.source.get(), first.source.get() + 1);
    assert!(
        ids.identify("asset:third", EncodingMetadata::default(), context())
            .is_err()
    );
    assert_eq!(
        second,
        ids.identify("asset:child", EncodingMetadata::default(), context())
            .unwrap()
    );
}
#[test]
fn identity_storage_and_lookup_work_are_bounded() {
    assert!(
        ImportIdentityRegistry::new(IdentityLimits {
            bytes: n(usize::MAX),
            ..limits()
        })
        .is_err()
    );
    for limited in [
        IdentityLimits {
            bytes: n(1),
            ..limits()
        },
        IdentityLimits {
            work: n(1),
            ..limits()
        },
    ] {
        let mut ids = ImportIdentityRegistry::new(limited).unwrap();
        assert!(
            ids.identify("asset:root", EncodingMetadata::default(), context())
                .is_err()
        );
    }
    let mut ids = ImportIdentityRegistry::new(IdentityLimits {
        bytes: n(1024),
        ..limits()
    })
    .unwrap();
    let first = ids
        .identify("asset:root", EncodingMetadata::default(), context())
        .unwrap();
    let large = vec![b'x'; 1024];
    assert!(
        ids.identify(
            "asset:child",
            EncodingMetadata {
                transport: Some(&large),
                ..Default::default()
            },
            context()
        )
        .is_err()
    );
    assert_eq!(
        first,
        ids.identify("asset:root", EncodingMetadata::default(), context())
            .unwrap()
    );
    let second = ids
        .identify("asset:child", EncodingMetadata::default(), context())
        .unwrap();
    assert_eq!(second.source.get(), first.source.get() + 1);
}
