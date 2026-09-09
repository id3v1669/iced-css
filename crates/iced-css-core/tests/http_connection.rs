use iced_css_core::resources::connection::*;
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> ConnectionLimits {
    ConnectionLimits {
        fields: n(10),
        bytes: n(10000),
        members: n(20),
        record_bytes: n(10000),
        work: n(10000),
    }
}
#[test]
fn connection_options_keep_provenance_and_star_is_only_a_literal_name() {
    let options = parse_connection_fields(
        &[b" , Keep-Alive,,\t".as_slice(), b"X-Hop, keep-alive, *"],
        limits(),
    )
    .unwrap();
    assert!(options.is_present());
    assert_eq!(options.options().len(), 4);
    assert_eq!(options.options()[0].name(), b"Keep-Alive");
    assert_eq!(options.options()[0].field(), 0);
    assert_eq!(options.options()[0].span(), 3..13);
    assert_eq!(options.options()[1].field(), 1);
    assert!(options.nominates(b"KEEP-ALIVE", n(1000)).unwrap());
    assert!(options.nominates(b"x-hop", n(1000)).unwrap());
    assert!(options.nominates(b"*", n(1000)).unwrap());
    assert!(!options.nominates(b"unlisted", n(1000)).unwrap());
    assert!(!parse_connection_fields(&[], limits()).unwrap().is_present());
    let empty = parse_connection_fields(&[b", ,\t"], limits()).unwrap();
    assert!(empty.is_present() && empty.options().is_empty());
    let forbidden_sender_option = parse_connection_fields(&[b"Cache-Control"], limits()).unwrap();
    assert!(
        forbidden_sender_option
            .nominates(b"cache-control", n(1000))
            .unwrap()
    );
}
#[test]
fn connection_invalid_tail_and_every_limit_remain_explicit_errors() {
    for invalid in [
        b"keep-alive, @bad".as_slice(),
        b"keep alive",
        b"\"close\"",
        b"close;param",
        b"x\r\ny",
        b"x\xff",
    ] {
        assert!(
            matches!(
                parse_connection_fields(&[invalid], limits()),
                Err(ConnectionError::Invalid { .. })
            ),
            "{invalid:?}"
        );
    }
    assert!(matches!(
        parse_connection_fields(
            &[b"x", b"y"],
            ConnectionLimits {
                fields: n(1),
                ..limits()
            }
        ),
        Err(ConnectionError::Fields)
    ));
    assert!(matches!(
        parse_connection_fields(
            &[b"xy"],
            ConnectionLimits {
                bytes: n(1),
                ..limits()
            }
        ),
        Err(ConnectionError::Bytes)
    ));
    assert!(matches!(
        parse_connection_fields(
            &[b"x,y"],
            ConnectionLimits {
                members: n(1),
                ..limits()
            }
        ),
        Err(ConnectionError::Members)
    ));
    assert!(matches!(
        parse_connection_fields(
            &[b"x"],
            ConnectionLimits {
                record_bytes: n(1),
                ..limits()
            }
        ),
        Err(ConnectionError::Records)
    ));
    assert!(matches!(
        parse_connection_fields(
            &[b",,,,,"],
            ConnectionLimits {
                work: n(1),
                ..limits()
            }
        ),
        Err(ConnectionError::Work)
    ));
    let options = parse_connection_fields(&[b"long-name"], limits()).unwrap();
    assert_eq!(
        options.nominates(b"long-name", n(1)),
        Err(ConnectionError::Work)
    );
}
