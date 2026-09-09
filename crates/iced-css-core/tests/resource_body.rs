use iced_css_core::resources::body::*;
use std::num::NonZeroUsize;
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> BodyLimits {
    BodyLimits {
        retained_bytes: n(100),
        work: n(1000),
    }
}
#[test]
fn exact_bytes_survive_arbitrary_chunks_without_character_decoding() {
    let bytes = b"\xff\xfea\0\0\0";
    for split in 0..=bytes.len() {
        let mut body = BodyCollector::new(Some(bytes.len() as u64), limits()).unwrap();
        body.append(&bytes[..split]).unwrap();
        body.append(&bytes[split..]).unwrap();
        let complete = body.finish(BodyEnd::Complete).unwrap();
        assert_eq!(complete.as_slice(), bytes);
        assert_eq!(complete.len(), bytes.len());
        assert!(!complete.is_empty());
        assert!(complete.retained_bytes() <= 100);
    }
    let empty = BodyCollector::new(Some(0), limits())
        .unwrap()
        .finish(BodyEnd::Complete)
        .unwrap();
    assert!(empty.is_empty());
}
#[test]
fn overrun_truncation_and_transport_failure_never_produce_completed_bodies() {
    let mut body = BodyCollector::new(Some(3), limits()).unwrap();
    body.append(b"ab").unwrap();
    assert_eq!(
        body.finish(BodyEnd::Complete).unwrap_err(),
        BodyError::Incomplete
    );
    let mut body = BodyCollector::new(Some(1), limits()).unwrap();
    assert_eq!(body.append(b"ab"), Err(BodyError::Overrun));
    assert_eq!(body.append(b"a"), Err(BodyError::Overrun));
    assert_eq!(
        body.finish(BodyEnd::Complete).unwrap_err(),
        BodyError::Overrun
    );
    let mut body = BodyCollector::new(None, limits()).unwrap();
    body.append(b"valid prefix").unwrap();
    assert_eq!(
        body.finish(BodyEnd::Failed).unwrap_err(),
        BodyError::Transport
    );
}
#[test]
fn advertised_and_actual_size_and_work_limits_are_checked_before_acceptance() {
    assert!(matches!(
        BodyCollector::new(Some(101), limits()),
        Err(BodyError::AdvertisedLength)
    ));
    let mut bounded = limits();
    bounded.retained_bytes = n(2);
    let mut body = BodyCollector::new(None, bounded).unwrap();
    assert_eq!(body.append(b"abc"), Err(BodyError::Bytes));
    assert_eq!(
        body.finish(BodyEnd::Complete).unwrap_err(),
        BodyError::Bytes
    );
    bounded = limits();
    bounded.work = n(3);
    let mut body = BodyCollector::new(None, bounded).unwrap();
    body.append(b"a").unwrap();
    assert!(body.finish(BodyEnd::Complete).is_ok());
    let mut body = BodyCollector::new(None, bounded).unwrap();
    for _ in 0..3 {
        body.append(b"").unwrap();
    }
    assert_eq!(body.append(b""), Err(BodyError::Work));
    assert_eq!(body.finish(BodyEnd::Complete).unwrap_err(), BodyError::Work);
}
