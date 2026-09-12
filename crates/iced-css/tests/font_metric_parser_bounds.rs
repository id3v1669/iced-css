use iced_css::fonts::metrics::{FontMetricError, FontMetricLimits, read_sfnt_design_metrics};
use std::num::NonZeroUsize;

fn limits() -> FontMetricLimits {
    FontMetricLimits {
        bytes: NonZeroUsize::new(128).unwrap(),
        tables: NonZeroUsize::new(1).unwrap(),
        work: NonZeroUsize::new(6).unwrap(),
    }
}

fn collection(count: u32) -> Vec<u8> {
    let mut data = b"ttcf\0\x01\0\0".to_vec();
    data.extend(count.to_be_bytes());
    data
}

#[test]
fn oversized_collection_offset_count_returns_an_error_without_panicking() {
    // A complete TTC fixed header, followed by no directory offsets. Its
    // declared array cannot fit these bytes or the parser's u32 stream range.
    let data = [b't', b't', b'c', b'f', 0, 1, 0, 0, 255, 255, 255, 255];
    let limits = FontMetricLimits {
        bytes: NonZeroUsize::new(data.len()).unwrap(),
        tables: NonZeroUsize::new(1).unwrap(),
        work: NonZeroUsize::new(6).unwrap(),
    };
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits),
        Err(FontMetricError::Directory(
            ttf_parser::FaceParsingError::MalformedFont
        ))
    ));
}

#[test]
fn offset_array_is_bounded_before_face_selection() {
    // Around the parser's u32 endpoint ceiling, plus a small truncated array.
    for count in [1, 1_073_741_820, 1_073_741_821, 1_073_741_822, u32::MAX] {
        for index in [0, 1, u32::MAX] {
            assert!(matches!(
                read_sfnt_design_metrics(&collection(count), index, limits()),
                Err(FontMetricError::Directory(
                    ttf_parser::FaceParsingError::MalformedFont
                ))
            ));
        }
    }
    assert!(matches!(
        read_sfnt_design_metrics(&collection(0), 0, limits()),
        Err(FontMetricError::Directory(
            ttf_parser::FaceParsingError::FaceIndexOutOfBounds
        ))
    ));
}

#[test]
fn public_limits_still_precede_malformed_collection_parsing() {
    let data = collection(u32::MAX);
    let mut cap = limits();
    cap.bytes = NonZeroUsize::new(11).unwrap();
    cap.work = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, cap),
        Err(FontMetricError::Bytes)
    ));
    cap.bytes = NonZeroUsize::new(12).unwrap();
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, cap),
        Err(FontMetricError::Work)
    ));
}

#[test]
fn safe_directory_errors_keep_the_dependency_error_categories() {
    use ttf_parser::FaceParsingError::{FaceIndexOutOfBounds, MalformedFont, UnknownMagic};
    for (offset, body, index, expected) in [
        (0u32, &b""[..], 0, MalformedFont),
        (u32::MAX, &b""[..], 0, MalformedFont),
        (16, &b""[..], 0, UnknownMagic),
        (16, &b"bad!"[..], 0, UnknownMagic),
        (16, &b"ttcf"[..], 0, UnknownMagic),
        (16, &b"\0\x01\0\0"[..], 0, MalformedFont),
        (16, &b"OTTO\0\x01\0\0\0\0\0\0"[..], 0, MalformedFont),
        (16, &b"bad!"[..], 1, FaceIndexOutOfBounds),
    ] {
        let mut data = collection(1);
        data.extend(offset.to_be_bytes());
        data.extend(body);
        let Err(FontMetricError::Directory(actual)) =
            read_sfnt_design_metrics(&data, index, limits())
        else {
            panic!("expected directory error for offset={offset}, index={index}");
        };
        assert_eq!(actual, expected);
    }
}
