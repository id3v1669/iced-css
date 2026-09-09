use super::{limits as transport_limits, nz, owned_server, settings};
use iced::futures::executor::block_on;
use iced_css::resources::{http::DirectHttpTransport, http_headers::*};
use iced_css_core::resources::{connection::*, header_update::*, storage_fields::*};
use reqwest::header::{HeaderMap, HeaderValue};

fn limits() -> HttpHeaderLimits {
    HttpHeaderLimits {
        fields: nz(32),
        input_bytes: nz(8192),
        record_bytes: nz(8192),
        work: nz(100000),
    }
}

#[test]
fn field_envelopes_preserve_per_name_order_without_copying_payloads() {
    let mut headers = HeaderMap::new();
    headers.append("x-repeat", HeaderValue::from_static(" \tfirst\t "));
    headers.append("x-unrelated", HeaderValue::from_static("other"));
    headers.append("x-repeat", HeaderValue::from_bytes(b"second\xff").unwrap());
    headers.append("x-empty", HeaderValue::from_static(" \t"));
    let block = HttpHeaderBlock::prepare(&headers, limits()).unwrap();
    assert_eq!(block.fields().len(), 4);
    let values: Vec<_> = block
        .fields()
        .iter()
        .copied()
        .filter(|field| field.name() == b"x-repeat")
        .collect();
    assert_eq!(values[0].raw_value(), b" \tfirst\t ");
    assert_eq!(values[0].value(), b"first");
    assert_eq!(values[0].value_span(), 2..7);
    assert_eq!(
        values[0].raw_value().as_ptr(),
        headers
            .get_all("x-repeat")
            .iter()
            .next()
            .unwrap()
            .as_bytes()
            .as_ptr()
    );
    assert_eq!(values[1].value(), b"second\xff");
    let empty = block
        .fields()
        .iter()
        .copied()
        .find(|field| field.name() == b"x-empty")
        .unwrap();
    assert_eq!(empty.value(), b"");
    assert!(block.retained_bytes() <= limits().record_bytes.get());
}

#[test]
fn header_adapter_limits_return_no_partial_block_or_input_mutation() {
    let mut headers = HeaderMap::new();
    headers.append("x", HeaderValue::from_static("one"));
    headers.append("x", HeaderValue::from_static("two"));
    let original = headers.clone();
    let mut limit = limits();
    limit.fields = nz(1);
    assert!(matches!(
        HttpHeaderBlock::prepare(&headers, limit),
        Err(HttpHeaderError::Fields)
    ));
    limit = limits();
    limit.input_bytes = nz(7);
    assert!(matches!(
        HttpHeaderBlock::prepare(&headers, limit),
        Err(HttpHeaderError::InputBytes)
    ));
    limit = limits();
    limit.record_bytes = nz(1);
    assert!(matches!(
        HttpHeaderBlock::prepare(&headers, limit),
        Err(HttpHeaderError::Records)
    ));
    limit = limits();
    limit.work = nz(1);
    assert!(matches!(
        HttpHeaderBlock::prepare(&headers, limit),
        Err(HttpHeaderError::Work)
    ));
    // Admit the input-count scan but exhaust the shared validation/move allowance.
    limit = limits();
    limit.work = nz(10);
    assert!(matches!(
        HttpHeaderBlock::prepare(&headers, limit),
        Err(HttpHeaderError::Work)
    ));
    assert_eq!(headers, original);
    let mut repeated = HeaderMap::new();
    let payload = HeaderValue::from_bytes(&[b'a'; 64]).unwrap();
    limit = limits();
    limit.work = nz(500);
    repeated.append("x", payload.clone());
    assert!(HttpHeaderBlock::prepare(&repeated, limit).is_ok());
    for _ in 1..20 {
        repeated.append("x", payload.clone());
    }
    assert!(matches!(
        HttpHeaderBlock::prepare(&repeated, limit),
        Err(HttpHeaderError::Work)
    ));
    limit = limits();
    limit.input_bytes = nz(8);
    assert_eq!(
        HttpHeaderBlock::prepare(&headers, limit)
            .unwrap()
            .fields()
            .len(),
        2
    );
    let empty = HeaderMap::new();
    let minimum = HttpHeaderLimits {
        fields: nz(1),
        input_bytes: nz(1),
        record_bytes: nz(1),
        work: nz(1),
    };
    let block = HttpHeaderBlock::prepare(&empty, minimum).unwrap();
    assert!(block.fields().is_empty());
    assert_eq!(block.retained_bytes(), 0);
}

#[test]
fn received_headers_feed_atomic_core_updates_without_filtering_at_intake() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let raw = b"HTTP/1.1 304 Not Modified\r\nX-Repeat: first\r\nX-Unrelated: other\r\nX-Repeat: second\r\nConnection: close, x-hop\r\nX-Hop: transient\r\nCache-Control: no-store\r\nContent-Length: 999\r\n\r\n";
    let (uri, peer) = owned_server(raw.to_vec());
    let response = block_on(transport.get(uri, transport_limits()).unwrap()).unwrap();
    peer.join().unwrap();
    let block = response.header_block(limits()).unwrap();
    assert!(block.fields().iter().any(|field| field.name() == b"x-hop"));
    assert!(
        block
            .fields()
            .iter()
            .any(|field| field.name() == b"cache-control" && field.value() == b"no-store")
    );
    drop(block);
    let (metadata, body) = response.into_parts();
    assert!(body.is_empty());
    let received = metadata.header_block(limits()).unwrap();
    let old = HeaderField::new(
        b"Content-Length",
        b"4",
        HeaderFieldLimits {
            bytes: nz(100),
            work: nz(1000),
        },
    )
    .unwrap();
    let options_limits = ConnectionLimits {
        fields: nz(10),
        bytes: nz(1000),
        members: nz(20),
        record_bytes: nz(4096),
        work: nz(10000),
    };
    let old_options = parse_connection_fields(&[], options_limits).unwrap();
    let connection: Vec<_> = received
        .fields()
        .iter()
        .copied()
        .filter(|f| f.name() == b"connection")
        .map(|f| f.value())
        .collect();
    let new_options = parse_connection_fields(&connection, options_limits).unwrap();
    let classify_limits = StorageFieldLimits {
        name_bytes: nz(100),
        work: nz(100000),
    };
    let mut old_classifier = StorageFieldClassifier::new(&old_options, classify_limits);
    let mut new_classifier = StorageFieldClassifier::new(&new_options, classify_limits);
    // Exercise metadata composition only. no-store above is not a storage grant.
    let updated = prepare_header_update(
        &[old],
        received.fields(),
        &mut old_classifier,
        &mut new_classifier,
        StoredProcessing::StylesheetPrepared,
        HeaderUpdateLimits {
            fields: nz(40),
            input_bytes: nz(8192),
            record_bytes: nz(8192),
            work: nz(100000),
        },
    )
    .unwrap();
    let values: Vec<_> = updated
        .fields()
        .iter()
        .map(|f| f.field())
        .filter(|f| f.name() == b"x-repeat")
        .map(|f| f.value())
        .collect();
    assert_eq!(values, [b"first".as_slice(), b"second"]);
    assert!(
        !updated
            .fields()
            .iter()
            .any(|f| [b"connection".as_slice(), b"x-hop"].contains(&f.field().name()))
    );
    assert_eq!(
        updated
            .fields()
            .iter()
            .find(|f| f.field().name().eq_ignore_ascii_case(b"content-length"))
            .unwrap()
            .field()
            .value(),
        b"4"
    );
}
