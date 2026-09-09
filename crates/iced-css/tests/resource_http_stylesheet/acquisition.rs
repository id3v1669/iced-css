use super::*;

fn http_stylesheet_read_limits() -> iced_css::resources::http_stylesheet::HttpStylesheetReadLimits {
    iced_css::resources::http_stylesheet::HttpStylesheetReadLimits {
        http: limits(),
        redirects: RedirectLimits {
            hops: nz(4),
            retained_bytes: nz(8192),
            work: nz(100000),
        },
        stylesheet: stylesheet_worker_limits(),
        media: iced_css_core::resources::http_admission::StylesheetMediaPolicy::RequireCss,
    }
}
#[test]
fn http_stylesheet_acquisition_uses_one_worker_and_preserves_redirect_provenance() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::stylesheet::RuleContext;
    use std::sync::Arc;
    let transport = Arc::new(block_on(DirectHttpTransport::new(settings())).unwrap());
    let reader = FileReader::new(nz(1));
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let (destination, final_server) = server(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    let head = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: {destination}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let (initial, first_server) = owned_server(head.into_bytes());
    let reference = format!("{initial}#fragment");
    let source = || HttpStylesheetSource {
        uri: reference.clone(),
        encoding: HttpStylesheetEncoding {
            linking: Some(b"UTF-8".to_vec()),
            referring: Some(b"UTF-8".to_vec()),
        },
    };
    let work = reader
        .read_http_stylesheet(
            transport.clone(),
            source(),
            context,
            http_stylesheet_read_limits(),
        )
        .unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert_eq!(reader.in_flight(), 1);
    assert!(matches!(
        reader.read_http_stylesheet(
            transport.clone(),
            source(),
            context,
            http_stylesheet_read_limits()
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    let parsed = block_on(work).unwrap();
    first_server.join().unwrap();
    final_server.join().unwrap();
    assert_eq!(parsed.stylesheet().source().text(), "r{color:red}");
    assert_eq!(parsed.stylesheet().source().referring_label(), b"UTF-16LE");
    assert_eq!(parsed.metadata().effective_uri, destination);
    assert_eq!(
        parsed.metadata().initial_reference.as_deref(),
        Some(reference.as_str())
    );
    assert_eq!(parsed.metadata().redirect_chain.len(), 1);
    assert_eq!(parsed.metadata().redirect_chain[0].source_uri(), initial);
    assert_eq!(
        parsed.metadata().redirect_chain[0].target_reference_uri(),
        format!("{destination}#fragment")
    );
    assert_eq!(parsed.metadata().headers["content-encoding"], "GZIP");
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(transport.in_flight(), 0);

    let mut limited = http_stylesheet_read_limits();
    limited.http.uri.bytes = nz(1);
    assert!(matches!(
        reader.read_http_stylesheet(transport.clone(), source(), context, limited),
        Err(HttpStylesheetSubmitError::UriBytes)
    ));
    let mut limited = http_stylesheet_read_limits();
    limited.stylesheet.metadata_bytes = nz(1);
    assert!(matches!(
        reader.read_http_stylesheet(transport.clone(), source(), context, limited),
        Err(HttpStylesheetSubmitError::Metadata)
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(transport.in_flight(), 0);
    // Keep an exchange future unread so transport admission must fail before a
    // second request can reach this server, even though the resource slot is free.
    let (uri, server) = server(b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let held = transport.get(uri.clone(), limits()).unwrap();
    server.join().unwrap();
    let work = reader
        .read_http_stylesheet(
            transport.clone(),
            HttpStylesheetSource {
                uri,
                encoding: Default::default(),
            },
            context,
            http_stylesheet_read_limits(),
        )
        .unwrap();
    assert!(matches!(
        block_on(work),
        Err(HttpStylesheetReadError::Transport(HttpError::Limit))
    ));
    assert_eq!(reader.in_flight(), 0);
    block_on(held).unwrap();
    assert_eq!(transport.in_flight(), 0);
}
#[test]
fn http_stylesheet_acquisition_reports_each_failure_without_retry() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::{resources::http_admission::GetResponseKind, stylesheet::RuleContext};
    use std::sync::Arc;
    let transport = Arc::new(block_on(DirectHttpTransport::new(settings())).unwrap());
    let reader = FileReader::new(nz(1));
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let acquire = |bytes, limits| {
        let (uri, server) = server(bytes);
        let result = block_on(
            reader
                .read_http_stylesheet(
                    transport.clone(),
                    HttpStylesheetSource {
                        uri,
                        encoding: Default::default(),
                    },
                    context,
                    limits,
                )
                .unwrap(),
        );
        server.join().unwrap();
        result
    };
    assert!(matches!(
        acquire(
            b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n",
            http_stylesheet_read_limits()
        ),
        Err(HttpStylesheetReadError::Stylesheet(
            HttpStylesheetError::Status {
                received: 304,
                kind: GetResponseKind::NotModified
            }
        ))
    ));
    assert!(matches!(
        acquire(
            include_bytes!("../fixtures/content_decoding/truncated-response.http"),
            http_stylesheet_read_limits()
        ),
        Err(HttpStylesheetReadError::Stylesheet(
            HttpStylesheetError::Content(_)
        ))
    ));
    let mut limited = http_stylesheet_read_limits();
    limited.stylesheet.header_fields = nz(1);
    assert!(matches!(
        acquire(
            include_bytes!("../fixtures/content_decoding/utf16-response.http"),
            limited
        ),
        Err(HttpStylesheetReadError::Input(
            HttpStylesheetSubmitError::Headers
        ))
    ));
    let mut limited = http_stylesheet_read_limits();
    limited.stylesheet.content.input_bytes = nz(1);
    assert!(matches!(
        acquire(
            include_bytes!("../fixtures/content_decoding/utf16-response.http"),
            limited
        ),
        Err(HttpStylesheetReadError::Input(
            HttpStylesheetSubmitError::InputBytes
        ))
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(transport.in_flight(), 0);
}
#[test]
fn dropping_http_stylesheet_work_keeps_its_worker_slot_until_exchange_finishes() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::stylesheet::RuleContext;
    use std::sync::{Arc, mpsc};
    let transport = Arc::new(block_on(DirectHttpTransport::new(settings())).unwrap());
    let reader = FileReader::new(nz(1));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!(
        "http://127.0.0.1:{}/style.css",
        listener.local_addr().unwrap().port()
    );
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut b = [0; 512];
        while !request.windows(4).any(|x| x == b"\r\n\r\n") {
            let n = socket.read(&mut b).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&b[..n]);
            assert!(request.len() < 8192);
        }
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let source = || HttpStylesheetSource {
        uri: uri.clone(),
        encoding: Default::default(),
    };
    let work = reader
        .read_http_stylesheet(
            transport.clone(),
            source(),
            context,
            http_stylesheet_read_limits(),
        )
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(work);
    assert_eq!(reader.in_flight(), 1);
    assert_eq!(transport.in_flight(), 1);
    assert!(matches!(
        reader.read_http_stylesheet(
            transport.clone(),
            source(),
            context,
            http_stylesheet_read_limits()
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    release_tx.send(()).unwrap();
    server.join().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while reader.in_flight() != 0 {
        assert!(std::time::Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(transport.in_flight(), 0);
}

#[test]
fn acquired_http_representation_can_feed_contexts_after_its_single_read_job() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::stylesheet::RuleContext;
    use std::sync::Arc;
    let transport = Arc::new(block_on(DirectHttpTransport::new(settings())).unwrap());
    let reader = FileReader::new(nz(1));
    let (uri, server) = server(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    let reference = format!("{uri}#part");
    let work = reader
        .read_http_stylesheet_representation(
            transport.clone(),
            reference.clone(),
            http_stylesheet_read_limits(),
        )
        .unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert!(matches!(
        reader.read_http_stylesheet_representation(
            transport.clone(),
            reference.clone(),
            http_stylesheet_read_limits()
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    let representation = Arc::new(block_on(work).unwrap());
    server.join().unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(
        representation.metadata().initial_reference.as_deref(),
        Some(reference.as_str())
    );
    assert_eq!(representation.metadata().effective_uri, uri);
    assert_eq!(
        representation.transport_charset(),
        Some(b"UTF-16LE".as_slice())
    );
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    for label in [b"UTF-8".as_slice(), b"UTF-16BE".as_slice()] {
        let parsed = block_on(
            reader
                .parse_http_stylesheet_representation(
                    representation.clone(),
                    HttpStylesheetEncoding {
                        linking: None,
                        referring: Some(label.to_vec()),
                    },
                    context,
                    HttpStylesheetContextLimits {
                        input_bytes: nz(4096),
                        metadata_bytes: nz(1024),
                        intake: stylesheet_worker_limits().intake,
                    },
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(parsed.source().text(), "r{color:red}");
        assert_eq!(parsed.source().referring_label(), b"UTF-16LE");
    }
    let mut limited = http_stylesheet_read_limits();
    limited.http.uri.bytes = nz(1);
    assert!(matches!(
        reader.read_http_stylesheet_representation(transport.clone(), reference, limited),
        Err(HttpStylesheetSubmitError::UriBytes)
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(transport.in_flight(), 0);
}
