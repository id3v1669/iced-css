use iced_css::resources::content_decoding::*;
use iced_css_core::resources::{body::*, content_codings::*};
use std::num::NonZeroUsize;
const RED: &[u8] = b"r{color:red}";
const GZIP: &[u8] = include_bytes!("fixtures/content_decoding/red.gz");
const ZLIB: &[u8] = include_bytes!("fixtures/content_decoding/red.zlib");
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> ContentDecodeLimits {
    ContentDecodeLimits {
        input_bytes: n(100000),
        output: BodyLimits {
            retained_bytes: n(100000),
            work: n(1000000),
        },
        calls: n(1000),
        stages: n(10),
        members: n(10),
    }
}
fn body(bytes: &[u8]) -> CompletedBody {
    let mut b = BodyCollector::new(None, limits().output).unwrap();
    b.append(bytes).unwrap();
    b.finish(BodyEnd::Complete).unwrap()
}
fn decode(
    bytes: &[u8],
    coding: &[u8],
    limit: ContentDecodeLimits,
) -> Result<CompletedBody, ContentDecodeError> {
    let fields = if coding.is_empty() {
        Vec::new()
    } else {
        vec![coding]
    };
    let list = parse_content_coding_fields(
        &fields,
        ContentCodingLimits {
            fields: n(10),
            bytes: n(1000),
            members: n(10),
            record_bytes: n(1000),
            work: n(10000),
        },
    )
    .unwrap();
    decode_content_codings(body(bytes), &list, limit)
}
#[test]
fn independent_fixtures_decode_in_reverse_coding_order() {
    for (bytes, coding, expected) in [
        (RED, b"".as_slice(), RED),
        (GZIP, b"GZip", RED),
        (GZIP, b"x-gzip", RED),
        (ZLIB, b"deflate", RED),
        (
            include_bytes!("fixtures/content_decoding/nested.zlib").as_slice(),
            b"gzip, deflate",
            RED,
        ),
        (
            include_bytes!("fixtures/content_decoding/multi.gz"),
            b"gzip",
            b"r{color:red}a{margin:2px}",
        ),
        (
            include_bytes!("fixtures/content_decoding/empty.gz"),
            b"gzip",
            b"",
        ),
        (
            include_bytes!("fixtures/content_decoding/empty.zlib"),
            b"deflate",
            b"",
        ),
    ] {
        assert_eq!(
            decode(bytes, coding, limits()).unwrap().as_slice(),
            expected
        );
    }
}
#[test]
fn incomplete_corrupt_and_extra_bytes_never_publish_a_prefix() {
    for (bytes, coding) in [(GZIP, b"gzip".as_slice()), (ZLIB, b"deflate")] {
        for end in 0..bytes.len() {
            assert!(
                decode(&bytes[..end], coding, limits()).is_err(),
                "prefix {end}"
            );
        }
        let mut corrupt = bytes.to_vec();
        let end = corrupt.len() - 1;
        corrupt[end] ^= 1;
        assert!(decode(&corrupt, coding, limits()).is_err());
        let mut extra = bytes.to_vec();
        extra.push(b'x');
        assert!(decode(&extra, coding, limits()).is_err());
    }
    let mut crc = GZIP.to_vec();
    let crc_at = crc.len() - 8;
    crc[crc_at] ^= 1;
    assert!(decode(&crc, b"gzip", limits()).is_err());
    assert!(
        decode(
            include_bytes!("fixtures/content_decoding/red.raw"),
            b"deflate",
            limits()
        )
        .is_err()
    );
    for unsupported in [b"br".as_slice(), b"identity", b"compress"] {
        assert!(matches!(
            decode(RED, unsupported, limits()),
            Err(ContentDecodeError::Unsupported { .. })
        ));
    }
}
#[test]
fn decoder_stage_member_input_output_and_call_limits_are_enforced() {
    assert!(matches!(
        decode(
            GZIP,
            b"gzip",
            ContentDecodeLimits {
                input_bytes: n(1),
                ..limits()
            }
        ),
        Err(ContentDecodeError::InputBytes)
    ));
    assert!(matches!(
        decode(
            GZIP,
            b"gzip",
            ContentDecodeLimits {
                output: BodyLimits {
                    retained_bytes: n(1),
                    ..limits().output
                },
                ..limits()
            }
        ),
        Err(ContentDecodeError::Body(BodyError::Bytes))
    ));
    assert!(matches!(
        decode(
            include_bytes!("fixtures/content_decoding/large.gz"),
            b"gzip",
            ContentDecodeLimits {
                calls: n(1),
                ..limits()
            }
        ),
        Err(ContentDecodeError::Calls)
    ));
    assert!(matches!(
        decode(
            include_bytes!("fixtures/content_decoding/multi.gz"),
            b"gzip",
            ContentDecodeLimits {
                members: n(1),
                ..limits()
            }
        ),
        Err(ContentDecodeError::Members)
    ));
    assert!(matches!(
        decode(
            include_bytes!("fixtures/content_decoding/nested.zlib"),
            b"gzip,deflate",
            ContentDecodeLimits {
                stages: n(1),
                ..limits()
            }
        ),
        Err(ContentDecodeError::Stages)
    ));
    assert!(
        decode(
            RED,
            b"",
            ContentDecodeLimits {
                output: BodyLimits {
                    retained_bytes: n(1),
                    ..limits().output
                },
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn optional_gzip_headers_and_dictionary_rejection_use_the_same_bounds() {
    let header = include_bytes!("fixtures/content_decoding/header-crc.gz");
    assert_eq!(decode(header, b"gzip", limits()).unwrap().as_slice(), RED);
    let mut corrupt = header.to_vec();
    corrupt[10] ^= 1;
    assert!(decode(&corrupt, b"gzip", limits()).is_err());
    let mut reserved = GZIP.to_vec();
    reserved[3] |= 0x20;
    assert!(decode(&reserved, b"gzip", limits()).is_err());
    let long = include_bytes!("fixtures/content_decoding/long-name.gz");
    assert_eq!(decode(long, b"gzip", limits()).unwrap().as_slice(), RED);
    assert!(matches!(
        decode(
            long,
            b"gzip",
            ContentDecodeLimits {
                calls: n(1),
                ..limits()
            }
        ),
        Err(ContentDecodeError::Calls)
    ));
    assert!(matches!(
        decode(
            include_bytes!("fixtures/content_decoding/dictionary.zlib"),
            b"deflate",
            limits()
        ),
        Err(ContentDecodeError::Decode(_))
    ));
    assert!(matches!(
        decode(
            GZIP,
            b"gzip",
            ContentDecodeLimits {
                output: BodyLimits {
                    work: n(1),
                    ..limits().output
                },
                ..limits()
            }
        ),
        Err(ContentDecodeError::Body(BodyError::Work))
    ));
}
