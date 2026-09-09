use iced_css::resources::{local::*, worker::*};
use iced_css_core::resources::{body::*, publication::*, slot::*, triggers::*};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> BackgroundReadLimits {
    BackgroundReadLimits {
        path_bytes: n(4096),
        read: ReadBodyLimits {
            reads: n(100),
            body: BodyLimits {
                retained_bytes: n(100),
                work: n(1000),
            },
        },
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "iced-css-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn background_file_results_work_without_a_tokio_executor_and_bound_unread_completions() {
    let temp = Temp::new();
    let path = temp.0.join("input.css");
    std::fs::write(&path, b"a{color:red}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let first = reader.read_file(path.clone(), limits()).unwrap();
    fn send<T: Send>(_: &T) {}
    send(&first);
    assert_eq!(reader.in_flight(), 1);
    assert!(matches!(
        reader.read_file(path.clone(), limits()),
        Err(SubmitError::Limit)
    ));
    let body = iced::futures::executor::block_on(first).unwrap();
    assert_eq!(body.as_slice(), b"a{color:red}");
    assert_eq!(reader.in_flight(), 0);
    assert!(iced::futures::executor::block_on(reader.read_file(path, limits()).unwrap()).is_ok());
}
#[test]
fn actual_failed_file_replacement_preserves_the_published_body() {
    let temp = Temp::new();
    let path = temp.0.join("input.css");
    std::fs::write(&path, b"old").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut slot = RuntimeResourceSlot::new(
        1,
        RuntimeMode::Manual,
        ResourceKind::LocalFile,
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
    );
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    let result =
        iced::futures::executor::block_on(reader.read_file(path.clone(), limits()).unwrap());
    assert!(matches!(
        slot.complete(first, result),
        ResourceCompletion::Published { .. }
    ));
    std::fs::write(&path, [b'x'; 101]).unwrap();
    let next = slot.request(RequestReason::ExplicitReload).unwrap();
    let result = iced::futures::executor::block_on(reader.read_file(path, limits()).unwrap());
    assert!(matches!(
        slot.complete(next, result),
        ResourceCompletion::Failed(BackgroundReadError::Read(ReadBodyError::Body(
            BodyError::AdvertisedLength
        )))
    ));
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    assert_eq!(slot.status(), PublicationStatus::Failed);
}
#[test]
fn oversized_owned_paths_are_rejected_without_consuming_capacity() {
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut bounded = limits();
    bounded.path_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        reader.read_file(PathBuf::from("long-path.css"), bounded),
        Err(SubmitError::PathBytes)
    ));
    assert_eq!(reader.in_flight(), 0);
}
