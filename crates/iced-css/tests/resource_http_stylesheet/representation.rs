use super::*;

#[test]
fn one_http_representation_supports_distinct_css_contexts_without_reacquisition() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::stylesheet::RuleContext;
    use std::sync::Arc;
    let transport = block_on(DirectHttpTransport::new(settings())).unwrap();
    let reader = FileReader::new(nz(1));
    let context = RuleContext {
        first_order: 0,
        first_declaration: std::num::NonZeroU64::MIN,
    };
    let fetch = |bytes| {
        let (uri, server) = owned_server(bytes);
        let response = block_on(transport.get(uri, limits()).unwrap()).unwrap();
        server.join().unwrap();
        response
    };
    let media = iced_css_core::resources::http_admission::StylesheetMediaPolicy::RequireCss;
    let response =
        fetch(include_bytes!("../fixtures/content_decoding/utf16-response.http").to_vec());
    let representation = Arc::new(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, stylesheet_worker_limits())
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        representation.content_decoded_body().as_slice(),
        b"r\0{\0c\0o\0l\0o\0r\0:\0r\0e\0d\0}\0"
    );
    assert_eq!(
        representation.transport_charset(),
        Some(b"UTF-16LE".as_slice())
    );
    assert_eq!(
        representation.metadata().headers["content-encoding"],
        "GZIP"
    );
    let parse_limits = HttpStylesheetContextLimits {
        input_bytes: nz(4096),
        metadata_bytes: nz(1024),
        intake: stylesheet_worker_limits().intake,
    };
    let work = reader
        .parse_http_stylesheet_representation(
            representation.clone(),
            HttpStylesheetEncoding {
                linking: Some(b"UTF-8".to_vec()),
                referring: Some(b"UTF-8".to_vec()),
            },
            context,
            parse_limits,
        )
        .unwrap();
    assert!(matches!(
        reader.parse_http_stylesheet_representation(
            representation.clone(),
            Default::default(),
            context,
            parse_limits
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    drop(representation);
    let parsed = block_on(work).unwrap();
    assert_eq!(parsed.source().text(), "r{color:red}");
    assert_eq!(parsed.source().referring_label(), b"UTF-16LE");
    let body = b"r{color:red}/*\xc3\xa9*/";
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(body);
    let representation = Arc::new(
        block_on(
            reader
                .decode_http_stylesheet_response(fetch(response), media, stylesheet_worker_limits())
                .unwrap(),
        )
        .unwrap(),
    );
    assert!(representation.transport_charset().is_none());
    for (label, expected) in [
        (b"UTF-8".as_slice(), "r{color:red}/*é*/"),
        (b"UTF-16LE".as_slice(), "筲潣潬㩲敲絤⨯꧃⼪"),
    ] {
        let parsed = block_on(
            reader
                .parse_http_stylesheet_representation(
                    representation.clone(),
                    HttpStylesheetEncoding {
                        linking: None,
                        referring: Some(label.to_vec()),
                    },
                    context,
                    parse_limits,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(parsed.source().text(), expected);
    }
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_representation(
                    representation.clone(),
                    HttpStylesheetEncoding {
                        linking: None,
                        referring: Some(b"ISO-8859-1".to_vec())
                    },
                    context,
                    parse_limits
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::Intake(
            iced_css_core::stylesheet::StylesheetIntakeError::Intake(
                iced_css_core::syntax::unicode_intake::IntakeError::UnimplementedCharset { mib: 4 }
            )
        ))
    ));
    assert_eq!(representation.content_decoded_body().as_slice(), body);
    assert!(matches!(
        reader.parse_http_stylesheet_representation(
            representation.clone(),
            Default::default(),
            context,
            HttpStylesheetContextLimits {
                input_bytes: nz(1),
                ..parse_limits
            }
        ),
        Err(HttpStylesheetSubmitError::InputBytes)
    ));
    assert!(matches!(
        reader.parse_http_stylesheet_representation(
            representation.clone(),
            HttpStylesheetEncoding {
                linking: Some(b"UTF-8".to_vec()),
                referring: None
            },
            context,
            HttpStylesheetContextLimits {
                metadata_bytes: nz(1),
                ..parse_limits
            }
        ),
        Err(HttpStylesheetSubmitError::Metadata)
    ));
    let mut limited = parse_limits;
    limited.intake.intake.decode.work = nz(1);
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_representation(
                    representation.clone(),
                    Default::default(),
                    context,
                    limited
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::Intake(_))
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(transport.in_flight(), 0);
    let response =
        fetch(include_bytes!("../fixtures/content_decoding/utf16-response.http").to_vec());
    let mut limited = stylesheet_worker_limits();
    limited.transport_charset_bytes = nz(1);
    assert!(matches!(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, limited)
                .unwrap()
        ),
        Err(HttpStylesheetError::TransportCharsetBytes)
    ));
    let response =
        fetch(include_bytes!("../fixtures/content_decoding/truncated-response.http").to_vec());
    assert!(matches!(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, stylesheet_worker_limits())
                .unwrap()
        ),
        Err(HttpStylesheetError::Content(_))
    ));
    let response = fetch(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
    assert!(matches!(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, stylesheet_worker_limits())
                .unwrap()
        ),
        Err(HttpStylesheetError::Media(_))
    ));
    let response = fetch(b"HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=\"\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
    let empty_charset = Arc::new(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, stylesheet_worker_limits())
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(empty_charset.transport_charset(), Some(b"".as_slice()));
    assert!(matches!(
        block_on(
            reader
                .parse_http_stylesheet_representation(
                    empty_charset,
                    HttpStylesheetEncoding {
                        linking: Some(b"UTF-8".to_vec()),
                        referring: None
                    },
                    context,
                    parse_limits
                )
                .unwrap()
        ),
        Err(HttpStylesheetError::Intake(
            iced_css_core::stylesheet::StylesheetIntakeError::Intake(
                iced_css_core::syntax::unicode_intake::IntakeError::UnknownCharset
            )
        ))
    ));
    let response = fetch(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec());
    assert!(matches!(
        block_on(
            reader
                .decode_http_stylesheet_response(response, media, stylesheet_worker_limits())
                .unwrap()
        ),
        Err(HttpStylesheetError::Status { received: 304, .. })
    ));
}
