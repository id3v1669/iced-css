use super::*;

#[test]
fn http_stylesheet_worker_preserves_received_metadata_and_actual_transport_charset() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::stylesheet::RuleContext;
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let fetch = |bytes| {
        let (uri, server) = server(bytes);
        let response = block_on(transport.get(uri.clone(), limits()).unwrap()).unwrap();
        server.join().unwrap();
        (uri, response)
    };
    let (uri, response) = fetch(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    let coded_length = response.body().len();
    let reader = FileReader::new(nz(1));
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let work = reader
        .parse_http_stylesheet_response(
            response,
            HttpStylesheetEncoding {
                linking: Some(b"UTF-8".to_vec()),
                referring: Some(b"UTF-8".to_vec()),
            },
            context,
            stylesheet_worker_limits(),
        )
        .unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert_eq!(reader.in_flight(), 1);
    let (_, response) = fetch(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    assert!(matches!(
        reader.parse_http_stylesheet_response(
            response,
            Default::default(),
            context,
            stylesheet_worker_limits()
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    let parsed = block_on(work).unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(parsed.stylesheet().source().text(), "r{color:red}");
    assert_eq!(parsed.stylesheet().source().referring_label(), b"UTF-16LE");
    assert_eq!(parsed.stylesheet().sheet().local_screen_rules().count(), 1);
    assert_eq!(parsed.metadata().status, 200);
    assert_eq!(parsed.metadata().effective_uri, uri);
    assert_eq!(
        parsed.metadata().headers["content-length"]
            .to_str()
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        coded_length
    );
    assert_eq!(parsed.metadata().headers["content-encoding"], "GZIP");
    let (stylesheet, metadata) = parsed.into_parts();
    assert_eq!(stylesheet.source().text(), "r{color:red}");
    assert_eq!(metadata.effective_uri, uri);
    let (_, response) =
        fetch(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}");
    let parsed = block_on(
        reader
            .parse_http_stylesheet_response(
                response,
                Default::default(),
                context,
                stylesheet_worker_limits(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.stylesheet().source().referring_label(), b"UTF-8");
    assert!(parsed.metadata().headers.get("content-type").is_none());
    let (_, response) = fetch(b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Type: text/plain\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}");
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_response(
                    response,
                    Default::default(),
                    context,
                    stylesheet_worker_limits()
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::ContentType(_))
    ));
    let (_, response) = fetch(b"HTTP/1.1 200 OK\r\nContent-Encoding: unknown\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}");
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_response(
                    response,
                    Default::default(),
                    context,
                    stylesheet_worker_limits()
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::Content(_))
    ));
    let (_, response) = fetch(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    assert!(matches!(
        reader.parse_http_stylesheet_response(
            response,
            Default::default(),
            context,
            HttpStylesheetLimits {
                header_fields: nz(1),
                ..stylesheet_worker_limits()
            }
        ),
        Err(HttpStylesheetSubmitError::Headers)
    ));
    assert_eq!(reader.in_flight(), 0);
    let (_, response) = fetch(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    assert!(matches!(
        reader.parse_http_stylesheet_response(
            response,
            HttpStylesheetEncoding {
                linking: Some(b"UTF-8".to_vec()),
                referring: None
            },
            context,
            HttpStylesheetLimits {
                metadata_bytes: nz(1),
                ..stylesheet_worker_limits()
            }
        ),
        Err(HttpStylesheetSubmitError::Metadata)
    ));
    let (_, response) = fetch(include_bytes!(
        "../fixtures/content_decoding/utf16-response.http"
    ));
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_response(
                    response,
                    Default::default(),
                    context,
                    HttpStylesheetLimits {
                        header_reference_bytes: nz(1),
                        ..stylesheet_worker_limits()
                    }
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::HeaderReferences)
    ));
}

#[test]
fn http_stylesheet_worker_rejects_incomplete_content_and_enforces_each_intake_boundary() {
    use iced_css::resources::{
        content_decoding::ContentDecodeError, http_stylesheet::*, worker::FileReader,
    };
    use iced_css_core::{resources::content_codings::ContentCodingError, stylesheet::RuleContext};
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let fetch = |bytes| {
        let (uri, server) = server(bytes);
        let response = block_on(transport.get(uri, limits()).unwrap()).unwrap();
        server.join().unwrap();
        response
    };
    let reader = FileReader::new(nz(1));
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let parse = |bytes, limits| {
        block_on(
            reader
                .parse_http_stylesheet_response(fetch(bytes), Default::default(), context, limits)
                .unwrap(),
        )
    };
    let nested = include_bytes!("../fixtures/content_decoding/nested-response.http");
    let parsed = parse(nested, stylesheet_worker_limits()).unwrap();
    assert_eq!(parsed.stylesheet().source().text(), "r{color:red}");
    let fields: Vec<_> = parsed
        .metadata()
        .headers
        .get_all("content-encoding")
        .iter()
        .map(|v| v.as_bytes())
        .collect();
    assert_eq!(fields, [b"gzip".as_slice(), b"deflate".as_slice()]);
    let empty = parse(
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        stylesheet_worker_limits(),
    )
    .unwrap();
    assert_eq!(empty.stylesheet().source().text(), "");
    assert_eq!(empty.stylesheet().sheet().local_screen_rules().count(), 0);
    assert!(matches!(parse(b"HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=\"utf-8\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", stylesheet_worker_limits()), Err(HttpStylesheetError::ContentType(_))));
    assert!(matches!(
        parse(
            include_bytes!("../fixtures/content_decoding/truncated-response.http"),
            stylesheet_worker_limits()
        ),
        Err(HttpStylesheetError::Content(ContentDecodeError::Incomplete))
    ));
    let mut limited = stylesheet_worker_limits();
    limited.content.input_bytes = nz(1);
    assert!(matches!(
        reader.parse_http_stylesheet_response(fetch(nested), Default::default(), context, limited),
        Err(HttpStylesheetSubmitError::InputBytes)
    ));
    assert_eq!(reader.in_flight(), 0);
    let mut limited = stylesheet_worker_limits();
    limited.codings.fields = nz(1);
    assert!(matches!(
        parse(nested, limited),
        Err(HttpStylesheetError::Codings(ContentCodingError::Fields))
    ));
    let mut limited = stylesheet_worker_limits();
    limited.content.stages = nz(1);
    assert!(matches!(
        parse(nested, limited),
        Err(HttpStylesheetError::Content(ContentDecodeError::Stages))
    ));
    let mut limited = stylesheet_worker_limits();
    limited.content.output.retained_bytes = nz(1);
    assert!(matches!(
        parse(nested, limited),
        Err(HttpStylesheetError::Content(ContentDecodeError::Body(_)))
    ));
    let mut limited = stylesheet_worker_limits();
    limited.intake.intake.decode.work = nz(1);
    assert!(matches!(
        parse(nested, limited),
        Err(HttpStylesheetError::Intake(_))
    ));
    assert_eq!(reader.in_flight(), 0);
}
