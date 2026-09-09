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
fn redirects() -> RedirectLimits {
    RedirectLimits {
        hops: nz(10),
        retained_bytes: nz(65536),
        work: nz(1_000_000),
    }
}
fn chain(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}/start", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 512];
            while !bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                let n = socket.read(&mut buffer).unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buffer[..n]);
                assert!(bytes.len() < 8192);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            socket.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (uri, worker)
}
fn redirect(status: u16, location: &str) -> String {
    format!(
        "HTTP/1.1 {status} Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}
fn ok() -> String {
    "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\na {}".into()
}
#[test]
fn recognized_get_redirects_keep_fragments_provenance_and_final_bases() {
    let statuses = [301, 302, 303, 307, 308, 300];
    let mut responses = Vec::new();
    for (i, status) in statuses.iter().enumerate() {
        responses.push(redirect(
            *status,
            &if i == 2 {
                "/nested/step3#".into()
            } else {
                format!("/nested/step{}", i + 1)
            },
        ));
    }
    responses[0] = responses[0].replacen("HTTP/1.1", "HTTP/1.0", 1);
    responses.push(ok());
    let (url, server) = chain(responses);
    let initial = format!("{url}#section");
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let work = transport
        .get_following_redirects(initial.clone(), limits(), redirects())
        .unwrap();
    fn send_static(_: &(impl Send + 'static)) {}
    send_static(&work);
    assert!(matches!(
        transport.get(url.clone(), limits()),
        Err(HttpError::Limit)
    ));
    let response = block_on(work).unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.version(), reqwest::Version::HTTP_11);
    assert_eq!(response.body().as_slice(), b"a {}");
    assert_eq!(response.initial_reference_uri(), Some(initial.as_str()));
    let base = url.strip_suffix("/start").unwrap();
    assert_eq!(response.effective_uri(), format!("{base}/nested/step6"));
    assert_eq!(
        response.final_reference_uri(),
        format!("{base}/nested/step6#")
    );
    assert_eq!(response.redirects().len(), 6);
    let mut previous = None;
    for (hop, status) in response.redirects().iter().zip(statuses) {
        assert_eq!(hop.status(), status);
        let timing = hop.timing();
        assert!(timing.request().monotonic() <= timing.response_headers().monotonic());
        if let Some(previous) = previous {
            assert!(previous <= timing.request().monotonic());
        }
        previous = Some(timing.response_headers().monotonic());
    }
    assert!(previous.unwrap() <= response.timing().request().monotonic());
    assert_eq!(response.redirects()[0].source_uri(), url);
    assert_eq!(response.redirects()[0].version(), reqwest::Version::HTTP_10);
    assert!(
        response.redirects()[1..]
            .iter()
            .all(|hop| hop.version() == reqwest::Version::HTTP_11)
    );
    assert_eq!(response.redirects()[0].location(), b"/nested/step1");
    assert_eq!(
        response.redirects()[0].target_reference_uri(),
        format!("{base}/nested/step1#section")
    );
    assert_eq!(transport.in_flight(), 0);
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 7);
    assert!(requests[0].starts_with("GET /start HTTP/1.1\r\n"));
    assert!(requests[6].starts_with("GET /nested/step6 HTTP/1.1\r\n"));
}
#[test]
fn direct_requests_and_nonredirect_statuses_do_not_follow_location() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let (url, server) = chain(vec![redirect(302, "/unused")]);
    let response = block_on(transport.get(url, limits()).unwrap()).unwrap();
    assert_eq!(response.status(), 302);
    assert!(response.redirects().is_empty());
    server.join().unwrap();
    for response in ["HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),"HTTP/1.1 201 Created\r\nLocation: file:///unused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()] {
        let (url,server)=chain(vec![response]);
        let result=block_on(transport.get_following_redirects(url,limits(),redirects()).unwrap()).unwrap();
        assert!(result.redirects().is_empty());server.join().unwrap();
    }
}
#[test]
fn loops_and_hop_exhaustion_stop_before_another_request() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    for (responses, bounds, want_loop) in [
        (vec![redirect(302, "")], redirects(), true),
        (
            vec![redirect(302, "/next"), redirect(302, "/start")],
            redirects(),
            true,
        ),
        (
            vec![redirect(302, "/next"), redirect(302, "/last")],
            RedirectLimits {
                hops: nz(1),
                ..redirects()
            },
            false,
        ),
    ] {
        let (url, server) = chain(responses);
        let error = block_on(
            transport
                .get_following_redirects(url, limits(), bounds)
                .unwrap(),
        )
        .unwrap_err();
        if want_loop {
            assert!(matches!(error, HttpError::Redirect(RedirectError::Loop)));
        } else {
            assert!(matches!(error, HttpError::Redirect(RedirectError::Hops)));
        }
        server.join().unwrap();
    }
}
#[test]
fn invalid_locations_and_non_http_targets_are_not_acquired() {
    use iced_css_core::resources::location::LocationError;
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let responses=[
        redirect(302,"file:///unused"),
        redirect(302,"http://user@127.0.0.1:1/a"),
        redirect(302,"/bad%"),
        "HTTP/1.1 302 Found\r\nLocation: /a\r\nLocation: /b\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    ];
    for (index, response) in responses.into_iter().enumerate() {
        let (url, server) = chain(vec![response]);
        let error = block_on(
            transport
                .get_following_redirects(url, limits(), redirects())
                .unwrap(),
        )
        .unwrap_err();
        match index {
            0 | 1 => assert!(matches!(error, HttpError::UnsupportedUri)),
            2 => assert!(matches!(
                error,
                HttpError::Redirect(RedirectError::Location(LocationError::Uri(
                    iced_css_core::resources::uri::UriError::Syntax
                )))
            )),
            3 => assert!(matches!(
                error,
                HttpError::Redirect(RedirectError::Location(LocationError::MultipleFields {
                    count: 2
                }))
            )),
            _ => unreachable!(),
        }
        server.join().unwrap();
    }
}

#[test]
fn history_limits_reject_before_unbounded_growth() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    assert!(matches!(
        transport.get_following_redirects(
            "http://127.0.0.1:1/".into(),
            limits(),
            RedirectLimits {
                retained_bytes: nz(1),
                ..redirects()
            }
        ),
        Err(HttpError::Redirect(RedirectError::Bytes))
    ));
    assert_eq!(transport.in_flight(), 0);
    let (url, server) = chain(vec![redirect(302, "/next")]);
    assert!(matches!(
        block_on(
            transport
                .get_following_redirects(
                    url,
                    limits(),
                    RedirectLimits {
                        work: nz(1),
                        ..redirects()
                    }
                )
                .unwrap()
        ),
        Err(HttpError::Redirect(RedirectError::Work))
    ));
    server.join().unwrap();
}
#[test]
fn cross_origin_gets_rebuild_host_without_cookies_or_validators() {
    let (target, target_server) = chain(vec![ok()]);
    let (source, source_server) = chain(vec![format!(
        "HTTP/1.1 303 See Other\r\nLocation: {target}\r\nSet-Cookie: secret=value\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )]);
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let response = block_on(
        transport
            .get_following_redirects(source, limits(), redirects())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response.effective_uri(), target);
    let requests = target_server.join().unwrap();
    let request = requests[0].to_ascii_lowercase();
    for field in [
        "\r\ncookie:",
        "\r\nauthorization:",
        "\r\nproxy-authorization:",
        "\r\nreferer:",
        "\r\nif-none-match:",
    ] {
        assert!(!request.contains(field));
    }
    let authority = target
        .strip_prefix("http://")
        .unwrap()
        .strip_suffix("/start")
        .unwrap();
    assert!(request.contains(&format!("\r\nhost: {authority}\r\n")));
    source_server.join().unwrap();
}

#[test]
fn retained_hop_storage_and_intermediate_bodies_have_separate_limits() {
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let (url, server) = chain(vec![redirect(302, "/next")]);
    let budget = nz(url.capacity()); // Initial reference fits; no hop metadata can fit.
    assert!(matches!(
        block_on(
            transport
                .get_following_redirects(
                    url,
                    limits(),
                    RedirectLimits {
                        retained_bytes: budget,
                        ..redirects()
                    }
                )
                .unwrap()
        ),
        Err(HttpError::Redirect(RedirectError::Bytes))
    ));
    server.join().unwrap();
    let (url, server) = chain(vec![
        "HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 999\r\nConnection: close\r\n\r\n"
            .into(),
    ]);
    assert!(matches!(
        block_on(
            transport
                .get_following_redirects(url, limits(), redirects())
                .unwrap()
        ),
        Err(HttpError::Read(
            iced_css::resources::local::ReadBodyError::Body(
                iced_css_core::resources::body::BodyError::AdvertisedLength
            )
        ))
    ));
    server.join().unwrap();
}

#[test]
fn cancelling_an_inflight_chain_prevents_the_next_hop() {
    use std::{io::ErrorKind, sync::mpsc, time::Instant};
    let next = TcpListener::bind("127.0.0.1:0").unwrap();
    next.set_nonblocking(true).unwrap();
    let target = format!("http://{}/next", next.local_addr().unwrap());
    let first = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("http://{}/start", first.local_addr().unwrap());
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = first.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 512];
        while !bytes.windows(4).any(|b| b == b"\r\n\r\n") {
            let n = socket.read(&mut buffer).unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&buffer[..n]);
            assert!(bytes.len() < 8192);
        }
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        socket.write_all(redirect(302, &target).as_bytes()).unwrap();
    });
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let work = transport
        .get_following_redirects(uri, limits(), redirects())
        .unwrap();
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(work);
    assert_eq!(transport.in_flight(), 1);
    release_tx.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while transport.in_flight() != 0 && Instant::now() < until {
        thread::yield_now();
    }
    assert_eq!(transport.in_flight(), 0);
    assert!(matches!(next.accept(),Err(error) if error.kind()==ErrorKind::WouldBlock));
    server.join().unwrap();
}
