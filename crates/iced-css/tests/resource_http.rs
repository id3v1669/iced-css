use iced::futures::executor::block_on;
use iced_css::resources::{http::*, local::ReadBodyLimits};
use iced_css_core::resources::{body::BodyLimits, uri::UriLimits};
use std::{
    io::{Read, Write},
    net::TcpListener,
    num::NonZeroUsize,
    thread,
    time::Duration,
};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

#[test]
fn exchange_samples_exclude_actor_queue_and_survive_metadata_move() {
    use std::{sync::mpsc, time::Instant};
    let mut config = settings();
    config.operations = nz(2);
    let transport = block_on(DirectHttpTransport::new(config)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}/style.css", listener.local_addr().unwrap());
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let peer = thread::spawn(move || {
        let mut observations = Vec::new();
        for index in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            let received = Instant::now();
            if index == 0 {
                ready_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            let sending = Instant::now();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\na {}")
                .unwrap();
            observations.push((received, sending));
        }
        observations
    });
    let before = Instant::now();
    let first = transport.get(uri.clone(), limits()).unwrap();
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = transport.get(uri, limits()).unwrap();
    let release = Instant::now();
    release_tx.send(()).unwrap();
    let first = block_on(first).unwrap();
    let second = block_on(second).unwrap();
    let after = Instant::now();
    let observations = peer.join().unwrap();
    assert!(second.timing().request().monotonic() >= release);
    for (response, (received, sending)) in [first, second].into_iter().zip(observations) {
        let timing = response.timing();
        assert!(before <= timing.request().monotonic());
        assert!(timing.request().monotonic() <= received);
        assert!(sending <= timing.response_headers().monotonic());
        assert!(timing.response_headers().monotonic() <= after);
        let (metadata, body) = response.into_parts();
        assert_eq!(metadata.timing, timing);
        assert_eq!(metadata.timing.request().wall(), timing.request().wall());
        assert_eq!(body.as_slice(), b"a {}");
    }
}

#[test]
fn received_protocol_version_survives_status_and_metadata_split() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    for (version, expected) in [
        ("HTTP/1.0", reqwest::Version::HTTP_10),
        ("HTTP/1.1", reqwest::Version::HTTP_11),
    ] {
        for status in ["200 OK", "304 Not Modified"] {
            let raw =
                format!("{version} {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let (url, server) = owned_server(raw.into_bytes());
            let response = block_on(transport.get(url, limits()).unwrap()).unwrap();
            server.join().unwrap();
            assert_eq!(response.version(), expected);
            let (metadata, body) = response.into_parts();
            assert_eq!(metadata.version, expected);
            assert!(body.is_empty());
        }
    }
}
fn settings() -> HttpSettings {
    HttpSettings {
        operations: nz(1),
        timeout: Duration::from_secs(3),
        connect_timeout: Duration::from_secs(3),
        additional_roots: vec![],
        root_entries: nz(4),
        root_bytes: nz(16384),
    }
}
fn limits() -> HttpLimits {
    HttpLimits {
        uri: UriLimits {
            bytes: nz(4096),
            work: nz(100000),
        },
        header_fields: nz(32),
        header_bytes: nz(8192),
        header_work: nz(100000),
        read: ReadBodyLimits {
            body: BodyLimits {
                retained_bytes: nz(128),
                work: nz(10000),
            },
            reads: nz(20),
        },
    }
}
fn server(response: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    owned_server(response.to_vec())
}
fn owned_server(response: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
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
        assert!(request.starts_with(b"GET /style.css HTTP/1.1\r\n"));
        socket.write_all(&response).unwrap();
    });
    (format!("http://127.0.0.1:{port}/style.css"), handle)
}
#[test]
fn complete_exchange_preserves_metadata_bytes_and_unread_result_admission() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let(url,server)=server(b"HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=\"utf-8\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\na { }\n");
    let work = transport.get(format!("{url}#ignored"), limits()).unwrap();
    fn is_send(_: &impl Send) {}
    is_send(&work);
    assert!(matches!(
        transport.get(url.clone(), limits()),
        Err(HttpError::Limit)
    ));
    let response = block_on(work).unwrap();
    server.join().unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.effective_uri(), url);
    assert_eq!(
        response.headers()["content-type"],
        "text/css; charset=\"utf-8\""
    );
    assert_eq!(response.body().as_slice(), b"a { }\n");
    assert_eq!(transport.in_flight(), 0);
}
#[test]
fn redirects_and_not_modified_are_exchanges_without_implicit_following() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    for (raw,status) in [(b"HTTP/1.1 302 Found\r\nLocation: https://unrequested.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),302),(b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nContent-Length: 999\r\nConnection: close\r\n\r\n".as_slice(),304)] {
 let(url,server)=server(raw); let response=block_on(transport.get(url.clone(),limits()).unwrap()).unwrap();server.join().unwrap(); assert_eq!(response.status(),status); assert_eq!(response.effective_uri(),url); assert!(response.body().is_empty());
 }
}
#[test]
fn incomplete_and_oversized_bodies_never_return_successful_prefixes() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    for raw in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 129\r\nConnection: close\r\n\r\n".as_slice(),
    ] {
        let (url, server) = server(raw);
        assert!(matches!(
            block_on(transport.get(url, limits()).unwrap()),
            Err(HttpError::Read(_))
        ));
        server.join().unwrap();
        assert_eq!(transport.in_flight(), 0);
    }
}
#[test]
fn invalid_requests_and_limits_are_explicit_and_release_admission() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    for uri in [
        "http:g",
        "http://user:password@host/",
        "http://:80/",
        "http:///g",
        "file:///tmp/a",
        "http://host/with space",
    ] {
        assert!(transport.get(uri.into(), limits()).is_err());
        assert_eq!(transport.in_flight(), 0);
    }
    let (url, server) = server(
        b"HTTP/1.1 200 OK\r\nX-One: abcdef\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let mut bound = limits();
    bound.header_bytes = nz(1);
    assert!(matches!(
        block_on(transport.get(url, bound).unwrap()),
        Err(HttpError::Headers)
    ));
    server.join().unwrap();
    let mut bad = settings();
    bad.timeout = Duration::ZERO;
    assert!(matches!(
        block_on(DirectHttpTransport::new(bad)),
        Err(HttpError::Settings)
    ));
}

fn tls_server(response: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    use std::sync::Arc;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(
                include_bytes!("fixtures/http/leaf.der").to_vec(),
            )],
            rustls::pki_types::PrivatePkcs8KeyDer::from(
                include_bytes!("fixtures/http/leaf-key.der").to_vec(),
            )
            .into(),
        )
        .unwrap();
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut stream = rustls::StreamOwned::new(
            rustls::ServerConnection::new(Arc::new(config)).unwrap(),
            socket,
        );
        let mut request = Vec::new();
        let mut b = [0; 512];
        while !request.windows(4).any(|x| x == b"\r\n\r\n") {
            match stream.read(&mut b) {
                Ok(0) | Err(_) => return,
                Ok(n) => request.extend_from_slice(&b[..n]),
            }
            assert!(request.len() < 8192);
        }
        stream.write_all(response).unwrap();
        stream.flush().unwrap();
        stream.conn.send_close_notify();
        let _ = stream.flush();
    });
    (format!("https://localhost:{port}/style.css"), handle)
}
#[test]
fn explicit_trust_succeeds_and_untrusted_or_wrong_host_tls_is_rejected() {
    for (trust, wrong_host) in [(true, false), (false, false), (true, true)] {
        let mut config = settings();
        if trust {
            config
                .additional_roots
                .push(include_bytes!("fixtures/http/ca.der").to_vec());
        }
        let transport = block_on(DirectHttpTransport::new(config)).unwrap();
        let (url, server) =
            tls_server(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\na { }\n");
        let url = if wrong_host {
            url.replace("localhost", "127.0.0.1")
        } else {
            url
        };
        let result = block_on(transport.get(url, limits()).unwrap());
        server.join().unwrap();
        if trust && !wrong_host {
            assert_eq!(result.unwrap().body().as_slice(), b"a { }\n");
        } else {
            let error = result.unwrap_err();
            assert!(matches!(error, HttpError::Request(_)));
            let diagnostic = format!("{error:?}");
            assert!(!diagnostic.contains("/style.css"), "{diagnostic}");
            assert!(!error.to_string().contains("/style.css"), "{error}");
            assert!(
                diagnostic.contains(if wrong_host {
                    "NotValidForName"
                } else {
                    "UnknownIssuer"
                }),
                "{diagnostic}"
            );
        }
        assert_eq!(transport.in_flight(), 0);
    }
}
#[test]
fn timeout_bounds_the_whole_exchange_not_each_body_read() {
    let transport = block_on(DirectHttpTransport::new(HttpSettings {
        timeout: Duration::from_millis(600),
        ..settings()
    }))
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut b = [0; 512];
        while !request.windows(4).any(|x| x == b"\r\n\r\n") {
            let n = socket.read(&mut b).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&b[..n]);
        }
        let _ =
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n");
        // Every read finishes well within the timeout; the whole body does not.
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(200));
            if socket.write_all(b"a").is_err() {
                break;
            }
        }
    });
    let mut limits = limits();
    limits.read.reads = nz(100);
    let result = block_on(
        transport
            .get(format!("http://127.0.0.1:{port}/style.css"), limits)
            .unwrap(),
    );
    assert!(result.is_err(), "{result:?}");
    server.join().unwrap();
    assert_eq!(transport.in_flight(), 0);
}
#[test]
fn invalid_or_excessive_application_roots_fail_before_exchange() {
    let mut config = settings();
    config.additional_roots.push(vec![0; 4]);
    assert!(matches!(
        block_on(DirectHttpTransport::new(config)),
        Err(HttpError::Tls(_))
    ));
    let mut config = settings();
    config.root_bytes = nz(1);
    config.additional_roots.push(vec![0; 4]);
    assert!(matches!(
        block_on(DirectHttpTransport::new(config)),
        Err(HttpError::Roots)
    ));
}
#[test]
fn dropping_started_request_retains_capacity_until_io_finishes() {
    use std::{sync::mpsc, time::Instant};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (start_tx, start_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut b = [0; 1024];
        assert!(socket.read(&mut b).unwrap() > 0);
        start_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let _ =
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    });
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let url = format!("http://127.0.0.1:{port}/style.css");
    let work = transport.get(url.clone(), limits()).unwrap();
    start_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(work);
    assert_eq!(transport.in_flight(), 1);
    assert!(matches!(
        transport.get(url, limits()),
        Err(HttpError::Limit)
    ));
    release_tx.send(()).unwrap();
    server.join().unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    while transport.in_flight() != 0 && Instant::now() < end {
        thread::yield_now();
    }
    assert_eq!(transport.in_flight(), 0);
}

#[test]
fn content_coding_and_duplicate_header_values_are_preserved_for_host_validation() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let(url,server)=server(b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: 5\r\nConnection: close\r\n\r\nraw!!");
    let response = block_on(transport.get(url, limits()).unwrap()).unwrap();
    server.join().unwrap();
    assert_eq!(response.body().as_slice(), b"raw!!");
    assert_eq!(response.headers()["content-encoding"], "gzip");
    let fields: Vec<_> = response
        .headers()
        .get_all("content-type")
        .iter()
        .map(|v| v.as_bytes())
        .collect();
    use iced_css_core::resources::http_metadata::{
        ContentTypeError, ContentTypeLimits, parse_content_type_fields,
    };
    assert!(matches!(
        parse_content_type_fields(
            &fields,
            ContentTypeLimits {
                parameters: nz(10),
                retained_bytes: nz(100),
                work: nz(1000)
            }
        ),
        Err(ContentTypeError::MultipleFields { count: 2 })
    ));
}

#[test]
fn conditional_get_sends_only_the_explicit_validated_tag_and_exposes_304() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut b = [0; 512];
        while !request.windows(4).any(|x| x == b"\r\n\r\n") {
            let n = socket.read(&mut b).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&b[..n]);
            assert!(request.len() < 8192);
        }
        assert!(
            request
                .windows(b"if-none-match: W/\"a\\b\"\r\n".len())
                .any(|x| x == b"if-none-match: W/\"a\\b\"\r\n")
        );
        socket
            .write_all(
                b"HTTP/1.1 304 Not Modified\r\nETag: W/\"a\\b\"\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
    });
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let uri = format!("http://127.0.0.1:{port}/style.css");
    assert!(matches!(
        transport.get_if_none_match(uri.clone(), b"\"tag\"\r\nInjected: yes".to_vec(), limits()),
        Err(HttpError::EntityTag(_))
    ));
    assert_eq!(transport.in_flight(), 0);
    let response = block_on(
        transport
            .get_if_none_match(uri, b"W/\"a\\b\"".to_vec(), limits())
            .unwrap(),
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(response.status(), 304);
    assert_eq!(response.headers()["etag"], "W/\"a\\b\"");
    assert!(response.body().is_empty());
}

#[test]
fn actual_redirect_target_drives_css_charset_and_nested_import_base() {
    use iced_css_core::{
        resources::{
            http_metadata::{ContentTypeLimits, parse_content_type_fields},
            location::resolve_redirect_location,
        },
        selectors::SelectorLimits,
        stylesheet::{
            requests::{ImportRequestLimits, resolve_import_requests},
            *,
        },
        syntax::{
            RecoveryLimits, StringLimits,
            decode::DecodeLimits,
            encoding_selection::{EncodingMetadata, EncodingOrigin, SelectionLimits},
            unicode_intake::IntakeLimits,
        },
        values::{NumericLimits, color::ColorLimits},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        for target in [
            b"GET /start HTTP/1.1\r\n".as_slice(),
            b"GET /nested/main.css?theme HTTP/1.1\r\n".as_slice(),
        ] {
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
            assert!(request.starts_with(target));
            if target.starts_with(b"GET /start ") {
                socket.write_all(b"HTTP/1.1 303 See Other\r\nLocation: /nested/main.css?theme\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                let body = b"@import \"child.css\"; a{color:red}";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=\"csUTF8\"\r\nContent-Location: /wrong/base.css\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).unwrap();
                socket.write_all(body).unwrap();
            }
        }
    });
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let original = format!("http://127.0.0.1:{port}/start#part");
    let first = block_on(transport.get(original.clone(), limits()).unwrap()).unwrap();
    assert_eq!(first.status(), 303);
    let location = resolve_redirect_location(
        first.effective_uri(),
        &original,
        &[first.headers()["location"].as_bytes()],
        limits().uri,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        location.reference_uri(),
        format!("http://127.0.0.1:{port}/nested/main.css?theme#part")
    );
    let response = block_on(
        transport
            .get(location.into_reference_uri(), limits())
            .unwrap(),
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(response.status(), 200);
    let metadata = parse_content_type_fields(
        &[response.headers()["content-type"].as_bytes()],
        ContentTypeLimits {
            parameters: nz(10),
            retained_bytes: nz(1000),
            work: nz(10000),
        },
    )
    .unwrap()
    .unwrap();
    assert!(metadata.is_css());
    assert!(response.headers().get("content-encoding").is_none());
    let a = nz(100000);
    let candidate = parse_separate_unicode_stylesheet(
        response.body().as_slice(),
        EncodingMetadata {
            transport: metadata.charset(),
            ..Default::default()
        },
        RuleContext {
            first_order: 0,
            first_declaration: std::num::NonZeroU64::MIN,
        },
        StylesheetIntakeLimits {
            intake: IntakeLimits {
                selection: SelectionLimits {
                    registry_work: a,
                    prefix_work: a,
                },
                decode: DecodeLimits {
                    entries: a,
                    retained_bytes: a,
                    work: a,
                },
                charset_work: a,
            },
            sheet: StylesheetLimits {
                statements: a,
                record_bytes: a,
                work: a,
                rules: RuleAssemblyLimits {
                    recovery: RecoveryLimits {
                        work: a,
                        retained_bytes: a,
                        nesting: a,
                    },
                    selectors: SelectorLimits {
                        work: a,
                        retained_bytes: a,
                        entries: a,
                    },
                    values: ColorLimits {
                        syntax: StringLimits {
                            work: a,
                            retained_bytes: a,
                        },
                        numeric: NumericLimits { work: a, digits: a },
                    },
                    entries: a,
                    record_bytes: a,
                    work: a,
                    name_bytes: a,
                },
            },
        },
    )
    .unwrap();
    assert_eq!(candidate.source().origin(), EncodingOrigin::Transport);
    let imports = resolve_import_requests(
        candidate.sheet(),
        response.effective_uri(),
        ImportRequestLimits {
            entries: a,
            retained_bytes: a,
            work: a,
        },
    )
    .unwrap();
    assert_eq!(imports.len(), 1);
    assert_eq!(
        imports[0].acquisition_uri(),
        format!("http://127.0.0.1:{port}/nested/child.css")
    );
    let rule = candidate.sheet().local_screen_rules().next().unwrap();
    let span = candidate
        .original_span(rule.locations[0].value.clone())
        .unwrap();
    assert_eq!(
        &response.body().as_slice()[span.start.byte..span.end.byte],
        b"red"
    );
}

#[test]
fn owned_response_metadata_survives_content_decoding_before_charset_intake() {
    use iced_css::resources::content_decoding::*;
    use iced_css_core::{
        resources::{content_codings::*, http_metadata::*},
        syntax::{
            decode::DecodeLimits,
            encoding_selection::{EncodingMetadata, SelectionLimits},
            unicode_intake::{IntakeLimits, decode_separate_unicode},
        },
    };
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let (uri, server) = server(include_bytes!(
        "fixtures/content_decoding/utf16-response.http"
    ));
    let response = block_on(transport.get(uri.clone(), limits()).unwrap()).unwrap();
    server.join().unwrap();
    let (metadata, body) = response.into_parts();
    let coded_length = body.len();
    assert_eq!(metadata.status, 200);
    assert_eq!(metadata.effective_uri, uri);
    assert!(metadata.initial_reference.is_none());
    assert!(metadata.redirect_chain.is_empty());
    let encodings = [metadata.headers["content-encoding"].as_bytes()];
    let codings = parse_content_coding_fields(
        &encodings,
        ContentCodingLimits {
            fields: nz(10),
            bytes: nz(4096),
            members: nz(10),
            record_bytes: nz(4096),
            work: nz(100000),
        },
    )
    .unwrap();
    let decoded = decode_content_codings(
        body,
        &codings,
        ContentDecodeLimits {
            input_bytes: nz(4096),
            output: BodyLimits {
                retained_bytes: nz(4096),
                work: nz(100000),
            },
            calls: nz(100),
            stages: nz(10),
            members: nz(10),
        },
    )
    .unwrap();
    assert_ne!(coded_length, decoded.len());
    // Headers still describe the coded HTTP representation, not a synthetic one.
    assert_eq!(
        metadata.headers["content-length"]
            .to_str()
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        coded_length
    );
    let content_type = [metadata.headers["content-type"].as_bytes()];
    let content_type = parse_content_type_fields(
        &content_type,
        ContentTypeLimits {
            parameters: nz(10),
            retained_bytes: nz(4096),
            work: nz(100000),
        },
    )
    .unwrap()
    .unwrap();
    assert!(content_type.is_css());
    let work = nz(100000);
    let source = decode_separate_unicode(
        decoded.as_slice(),
        EncodingMetadata {
            transport: content_type.charset(),
            linking: Some(b"UTF-8"),
            referring: Some(b"UTF-8"),
        },
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
        },
    )
    .unwrap();
    assert_eq!(source.text(), "r{color:red}");
    assert_eq!(source.referring_label(), b"UTF-16LE");
}

#[path = "resource_http_stylesheet/mod.rs"]
mod stylesheet;

#[path = "resource_http_parts/headers.rs"]
mod header_adapter;
