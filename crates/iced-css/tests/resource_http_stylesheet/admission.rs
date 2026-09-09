use super::*;

#[test]
fn admitted_http_stylesheets_require_representation_and_selected_media_policy() {
    use iced_css::resources::{http_stylesheet::*, worker::FileReader};
    use iced_css_core::{
        resources::http_admission::{GetResponseKind, StylesheetMediaError, StylesheetMediaPolicy},
        stylesheet::RuleContext,
    };
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
    let admit = |bytes, policy| {
        block_on(
            reader
                .admit_http_stylesheet_response(
                    fetch(bytes),
                    Default::default(),
                    context,
                    policy,
                    stylesheet_worker_limits(),
                )
                .unwrap(),
        )
    };
    let empty = b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    assert!(
        admit(empty, StylesheetMediaPolicy::RequireCss)
            .unwrap()
            .stylesheet()
            .source()
            .text()
            .is_empty()
    );
    for (bytes, expected) in [
        (b"HTTP/1.1 204 No Content\r\nContent-Type: text/css\r\nConnection: close\r\n\r\n".as_slice(), GetResponseKind::NoContent),
        (b"HTTP/1.1 304 Not Modified\r\nContent-Type: text/css\r\nConnection: close\r\n\r\n".as_slice(), GetResponseKind::NotModified),
        (b"HTTP/1.1 206 Partial Content\r\nContent-Type: text/css\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".as_slice(), GetResponseKind::PartialContent),
        (b"HTTP/1.1 404 Not Found\r\nContent-Type: text/css\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".as_slice(), GetResponseKind::ClientError),
    ] {
        let expected_status: u16 = std::str::from_utf8(&bytes[9..12]).unwrap().parse().unwrap();
        assert!(matches!(admit(bytes, StylesheetMediaPolicy::RequireCss), Err(HttpStylesheetError::Status { received, kind }) if kind == expected && received == expected_status));
    }
    let absent = b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}";
    assert!(matches!(
        admit(absent, StylesheetMediaPolicy::RequireCss),
        Err(HttpStylesheetError::Media(StylesheetMediaError::Missing))
    ));
    let parsed = admit(absent, StylesheetMediaPolicy::AllowAbsent).unwrap();
    assert_eq!(parsed.stylesheet().source().text(), "r{color:red}");
    assert!(parsed.metadata().headers.get("content-type").is_none());
    for policy in [
        StylesheetMediaPolicy::RequireCss,
        StylesheetMediaPolicy::AllowAbsent,
    ] {
        assert!(matches!(admit(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad", policy), Err(HttpStylesheetError::Media(StylesheetMediaError::Other))));
        assert!(matches!(admit(b"HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", policy), Err(HttpStylesheetError::ContentType(_))));
    }
    for (bytes, expected) in [
        (b"HTTP/1.1 203 Non-Authoritative Information\r\nContent-Type: text/css\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}".as_slice(), 203),
        (b"HTTP/1.1 299 Unrecognized\r\nContent-Type: text/css\r\nContent-Length: 12\r\nConnection: close\r\n\r\nr{color:red}".as_slice(), 299),
    ] {
        let (sheet, metadata) = admit(bytes, StylesheetMediaPolicy::RequireCss).unwrap().into_parts();
        assert_eq!(metadata.status, expected);
        assert_eq!(sheet.source().text(), "r{color:red}");
    }
    let work = reader
        .admit_http_stylesheet_response(
            fetch(include_bytes!(
                "../fixtures/content_decoding/utf16-response.http"
            )),
            HttpStylesheetEncoding {
                linking: Some(b"UTF-8".to_vec()),
                referring: Some(b"UTF-8".to_vec()),
            },
            context,
            StylesheetMediaPolicy::RequireCss,
            stylesheet_worker_limits(),
        )
        .unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert!(matches!(
        reader.parse_http_stylesheet_response(
            fetch(empty),
            Default::default(),
            context,
            stylesheet_worker_limits()
        ),
        Err(HttpStylesheetSubmitError::Worker(_))
    ));
    let parsed = block_on(work).unwrap();
    assert_eq!(parsed.stylesheet().source().referring_label(), b"UTF-16LE");
    assert_eq!(parsed.metadata().headers["content-encoding"], "GZIP");
    assert_eq!(reader.in_flight(), 0);
}
