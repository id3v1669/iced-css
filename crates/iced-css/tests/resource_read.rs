use iced_css::resources::local::*;
use iced_css_core::resources::body::{BodyError, BodyLimits};
use std::{
    io::{self, Cursor, Read},
    num::NonZeroUsize,
};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> ReadBodyLimits {
    ReadBodyLimits {
        body: BodyLimits {
            retained_bytes: n(100),
            work: n(1000),
        },
        reads: n(100),
    }
}
#[test]
fn reader_preserves_bytes_and_requires_a_complete_end_after_the_expected_length() {
    let bytes = b"\xff\xfea\0";
    assert_eq!(
        read_body_blocking(&mut Cursor::new(bytes), Some(4), limits())
            .unwrap()
            .as_slice(),
        bytes
    );
    assert!(matches!(
        read_body_blocking(&mut Cursor::new(bytes), Some(5), limits()),
        Err(ReadBodyError::Body(BodyError::Incomplete))
    ));
    let mut bounded = limits();
    bounded.reads = NonZeroUsize::MIN;
    assert!(matches!(
        read_body_blocking(&mut Cursor::new(bytes), Some(4), bounded),
        Err(ReadBodyError::Reads)
    ));
    bounded = limits();
    bounded.body.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        read_body_blocking(&mut Cursor::new(bytes), None, bounded),
        Err(ReadBodyError::Body(BodyError::Bytes))
    ));
}
struct InterruptThenRead {
    attempts: usize,
    data: Cursor<&'static [u8]>,
}
impl Read for InterruptThenRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.attempts += 1;
        if self.attempts == 1 {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            self.data.read(buf)
        }
    }
}
struct BadCount;
impl Read for BadCount {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(buf.len() + 1)
    }
}
struct PartialError(bool);
impl Read for PartialError {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            Err(io::ErrorKind::UnexpectedEof.into())
        } else {
            self.0 = true;
            buf[0] = b'a';
            Ok(1)
        }
    }
}
#[test]
fn interrupted_reads_are_bounded_and_other_failures_cannot_publish_prefixes() {
    let mut reader = InterruptThenRead {
        attempts: 0,
        data: Cursor::new(b"abc"),
    };
    assert_eq!(
        read_body_blocking(&mut reader, None, limits())
            .unwrap()
            .as_slice(),
        b"abc"
    );
    assert_eq!(reader.attempts, 3);
    let mut reader = InterruptThenRead {
        attempts: 0,
        data: Cursor::new(b"abc"),
    };
    let mut bounded = limits();
    bounded.reads = NonZeroUsize::MIN;
    assert!(matches!(
        read_body_blocking(&mut reader, None, bounded),
        Err(ReadBodyError::Reads)
    ));
    assert!(matches!(
        read_body_blocking(&mut BadCount, None, limits()),
        Err(ReadBodyError::InvalidReadCount)
    ));
    assert!(matches!(
        read_body_blocking(&mut PartialError(false), None, limits()),
        Err(ReadBodyError::Io(_))
    ));
}
#[test]
fn explicit_local_file_acquisition_preserves_original_encoding_bytes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "iced-css-body-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let cleanup = Remove(path.clone());
    let file = path.join("input.css");
    let bytes = b"\xef\xbb\xbfa{color:red}";
    std::fs::write(&file, bytes).unwrap();
    let body = read_file_blocking(&file, limits()).unwrap();
    assert_eq!(body.as_slice(), bytes);
    use iced_css_core::{
        selectors::SelectorLimits,
        stylesheet::*,
        syntax::{
            RecoveryLimits, StringLimits,
            decode::DecodeLimits,
            encoding_selection::{EncodingMetadata, SelectionLimits},
            unicode_intake::IntakeLimits,
        },
        values::{NumericLimits, color::ColorLimits},
    };
    let allowance = n(100000);
    let candidate = parse_separate_unicode_stylesheet(
        body.as_slice(),
        EncodingMetadata::default(),
        RuleContext {
            first_order: 0,
            first_declaration: std::num::NonZeroU64::MIN,
        },
        StylesheetIntakeLimits {
            intake: IntakeLimits {
                selection: SelectionLimits {
                    registry_work: allowance,
                    prefix_work: allowance,
                },
                decode: DecodeLimits {
                    entries: allowance,
                    retained_bytes: allowance,
                    work: allowance,
                },
                charset_work: allowance,
            },
            sheet: StylesheetLimits {
                statements: allowance,
                record_bytes: allowance,
                work: allowance,
                rules: RuleAssemblyLimits {
                    recovery: RecoveryLimits {
                        work: allowance,
                        retained_bytes: allowance,
                        nesting: allowance,
                    },
                    selectors: SelectorLimits {
                        work: allowance,
                        retained_bytes: allowance,
                        entries: allowance,
                    },
                    values: ColorLimits {
                        syntax: StringLimits {
                            work: allowance,
                            retained_bytes: allowance,
                        },
                        numeric: NumericLimits {
                            work: allowance,
                            digits: allowance,
                        },
                    },
                    entries: allowance,
                    record_bytes: allowance,
                    work: allowance,
                    name_bytes: allowance,
                },
            },
        },
    )
    .unwrap();
    let rule = candidate.sheet().local_screen_rules().next().unwrap();
    assert_eq!(
        rule.declarations[0].parsed.property.descriptor().name,
        "color"
    );
    let original = candidate
        .original_span(rule.locations[0].value.clone())
        .unwrap();
    assert_eq!(&bytes[original.start.byte..original.end.byte], b"red");
    let mut bounded = limits();
    bounded.body.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        read_file_blocking(&file, bounded),
        Err(ReadBodyError::Body(BodyError::AdvertisedLength))
    ));
    assert!(matches!(
        read_file_blocking(&path.join("missing.css"), limits()),
        Err(ReadBodyError::Io(_))
    ));
    drop(cleanup);
}
