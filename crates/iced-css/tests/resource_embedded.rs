use iced::futures::executor::block_on;
use iced_css::resources::embedded::*;
use iced_css_core::resources::{body::BodyLimits, uri::UriLimits};
use std::{
    io,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn uri() -> UriLimits {
    UriLimits {
        bytes: n(4096),
        work: n(100000),
    }
}
fn limits() -> EmbeddedLimits {
    EmbeddedLimits {
        uri: uri(),
        metadata_bytes: n(10000),
        body: BodyLimits {
            retained_bytes: n(100),
            work: n(10000),
        },
    }
}
struct Assets {
    calls: Arc<AtomicUsize>,
    fail: bool,
}
impl EmbeddedResolver for Assets {
    type Error = io::Error;
    fn resolve(&self, request: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
        assert_eq!(std::thread::current().name(), Some("iced-css-resource"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.namespace, "app");
        assert_eq!(request.base, "app:/themes/main.css");
        assert_eq!(request.reference_uri, "app:/images/icon#part");
        assert_eq!(request.acquisition_uri, "app:/images/icon");
        if self.fail {
            return Err(io::Error::other("controlled unavailable asset"));
        }
        Ok(EmbeddedSource {
            bytes: b"a{color:red}".to_vec(),
            version: b"opaque-v1".to_vec(),
            dependency_base: "app:/compiled/resources/main.css".into(),
        })
    }
}
#[test]
fn declared_namespace_hook_runs_in_background_and_returns_an_immutable_candidate() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reader = EmbeddedReader::new(
        "app".into(),
        Assets {
            calls: calls.clone(),
            fail: false,
        },
        n(1),
        uri(),
    )
    .unwrap();
    let work = reader
        .resolve(
            "app:/themes/main.css".into(),
            "../images/icon#part".into(),
            limits(),
        )
        .unwrap();
    fn send(_: &impl Send) {}
    send(&work);
    let asset = block_on(work).unwrap();
    assert_eq!(asset.body().as_slice(), b"a{color:red}");
    assert_eq!(asset.version(), b"opaque-v1");
    assert_eq!(asset.dependency_base(), "app:/compiled/resources/main.css");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(reader.in_flight(), 0);
}
#[test]
fn namespace_mismatch_never_calls_the_hook_and_callback_failure_is_explicit() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reader = EmbeddedReader::new(
        "app".into(),
        Assets {
            calls: calls.clone(),
            fail: true,
        },
        n(1),
        uri(),
    )
    .unwrap();
    assert!(matches!(
        block_on(
            reader
                .resolve("https://example.invalid/".into(), "x".into(), limits())
                .unwrap()
        ),
        Err(EmbeddedError::Namespace)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        block_on(
            reader
                .resolve(
                    "app:/themes/main.css".into(),
                    "../images/icon#part".into(),
                    limits()
                )
                .unwrap()
        ),
        Err(EmbeddedError::Resolver(_))
    ));
    assert_eq!(reader.in_flight(), 0);
    for namespace in ["http", "HTTPS", "file", "1app", "app:", ""] {
        assert!(
            EmbeddedReader::new(
                namespace.into(),
                Assets {
                    calls: calls.clone(),
                    fail: false
                },
                n(1),
                uri()
            )
            .is_err()
        );
    }
}
struct Oversized;
impl EmbeddedResolver for Oversized {
    type Error = io::Error;
    fn resolve(&self, _: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
        Ok(EmbeddedSource {
            bytes: vec![0; 101],
            version: vec![],
            dependency_base: "app:/a".into(),
        })
    }
}
#[test]
fn owned_input_and_callback_output_limits_fail_without_publishing_a_prefix() {
    let reader = EmbeddedReader::new("app".into(), Oversized, n(1), uri()).unwrap();
    assert!(matches!(
        block_on(
            reader
                .resolve("app:/a".into(), "b".into(), limits())
                .unwrap()
        ),
        Err(EmbeddedError::BodyBytes)
    ));
    let mut cap = limits();
    cap.metadata_bytes = n(1);
    assert!(matches!(
        reader.resolve("app:/a".into(), "b".into(), cap),
        Err(EmbeddedSubmitError::Metadata)
    ));
    assert_eq!(reader.in_flight(), 0);
}

struct Changing(AtomicUsize);
impl EmbeddedResolver for Changing {
    type Error = io::Error;
    fn resolve(&self, _: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
        if self.0.fetch_add(1, Ordering::SeqCst) > 0 {
            return Err(io::Error::other("replacement failed"));
        }
        Ok(EmbeddedSource {
            bytes: b"old".to_vec(),
            version: b"v1".to_vec(),
            dependency_base: "app:/a".into(),
        })
    }
}
#[test]
fn failed_explicit_embedded_notice_preserves_the_published_asset() {
    use iced_css_core::resources::{
        publication::{PublicationLimits, PublicationStatus},
        slot::{ResourceCompletion, RuntimeResourceSlot},
        triggers::{RequestError, RequestReason, ResourceKind, RuntimeMode},
    };
    use std::num::NonZeroU64;
    let reader =
        EmbeddedReader::new("app".into(), Changing(AtomicUsize::new(0)), n(1), uri()).unwrap();
    let mut slot = RuntimeResourceSlot::new(
        1,
        RuntimeMode::Manual,
        ResourceKind::Embedded,
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
    );
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    let asset = block_on(
        reader
            .resolve("app:/a".into(), "b".into(), limits())
            .unwrap(),
    );
    assert!(matches!(
        slot.complete(first, asset),
        ResourceCompletion::Published { .. }
    ));
    assert_eq!(
        slot.request(RequestReason::LocalChange),
        Err(RequestError::Unauthorized)
    );
    let next = slot.request(RequestReason::EmbeddedNotification).unwrap();
    let asset = block_on(
        reader
            .resolve("app:/a".into(), "b".into(), limits())
            .unwrap(),
    );
    assert!(matches!(
        slot.complete(next, asset),
        ResourceCompletion::Failed(EmbeddedError::Resolver(_))
    ));
    assert_eq!(slot.active().unwrap().1.body().as_slice(), b"old");
    assert_eq!(slot.active().unwrap().1.version(), b"v1");
    assert_eq!(slot.status(), PublicationStatus::Failed);
}

struct InvalidMetadata {
    base: &'static str,
    version_bytes: usize,
}
impl EmbeddedResolver for InvalidMetadata {
    type Error = io::Error;
    fn resolve(&self, _: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
        Ok(EmbeddedSource {
            bytes: b"valid bytes".to_vec(),
            version: vec![0; self.version_bytes],
            dependency_base: self.base.into(),
        })
    }
}
#[test]
fn returned_dependency_base_and_version_must_pass_admission() {
    for base in ["relative.css", "app:/a#fragment"] {
        let reader = EmbeddedReader::new(
            "app".into(),
            InvalidMetadata {
                base,
                version_bytes: 0,
            },
            n(1),
            uri(),
        )
        .unwrap();
        assert!(matches!(
            block_on(
                reader
                    .resolve("app:/a".into(), "b".into(), limits())
                    .unwrap()
            ),
            Err(EmbeddedError::DependencyBase)
        ));
        assert_eq!(reader.in_flight(), 0);
    }
    let reader = EmbeddedReader::new(
        "app".into(),
        InvalidMetadata {
            base: "https://example.invalid/nested.css",
            version_bytes: 10001,
        },
        n(1),
        uri(),
    )
    .unwrap();
    assert!(matches!(
        block_on(
            reader
                .resolve("app:/a".into(), "b".into(), limits())
                .unwrap()
        ),
        Err(EmbeddedError::Metadata)
    ));
    assert_eq!(reader.in_flight(), 0);
}
