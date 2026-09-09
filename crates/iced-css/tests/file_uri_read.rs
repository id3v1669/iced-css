#![cfg(unix)]
use iced_css::resources::{
    file_path::{FilePathError, FilePathLimits, unix_file_uri},
    file_uri_read::{FileUriReadError, FileUriReadLimits, FileUriSubmitError},
    local::ReadBodyLimits,
    worker::{FileReader, SubmitError},
};
use iced_css_core::resources::{
    body::{BodyError, BodyLimits},
    file_uri::FileUriError,
    uri::UriLimits,
};
use std::{future::Future, num::NonZeroUsize};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> FileUriReadLimits {
    FileUriReadLimits {
        uri: UriLimits {
            bytes: nz(4096),
            work: nz(100_000),
        },
        path: FilePathLimits {
            bytes: nz(4096),
            work: nz(100_000),
        },
        read: ReadBodyLimits {
            reads: nz(10),
            body: BodyLimits {
                retained_bytes: nz(4096),
                work: nz(100_000),
            },
        },
    }
}
fn execute<F: Future + Send + 'static>(future: F) -> F::Output {
    iced::futures::executor::block_on(future)
}
#[test]
fn background_file_uri_bytes_keep_the_base_for_escaped_relative_imports() {
    let root = std::env::temp_dir().join(format!("iced-css-file-uri-read-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let native = root.join("theme:%.css");
    std::fs::write(&native, b"@import 'child%3F.css'; button { margin: 4px }").unwrap();
    std::fs::write(root.join("child?.css"), b"button { color: red }").unwrap();
    let uri = unix_file_uri(&native, limits().path).unwrap();
    let reader = FileReader::new(nz(1));
    let work = reader.read_file_uri(uri.clone(), limits()).unwrap();
    assert_eq!(reader.in_flight(), 1);
    assert!(matches!(
        reader.read_file_uri(uri.clone(), limits()),
        Err(FileUriSubmitError::Worker(SubmitError::Limit))
    ));
    let asset = execute(work).unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(asset.acquisition_uri(), uri);
    assert_eq!(asset.native_path(), native);
    assert_eq!(
        asset.body().as_slice(),
        b"@import 'child%3F.css'; button { margin: 4px }"
    );
    let candidate = parse_asset(&asset);
    let imports = iced_css_core::stylesheet::requests::resolve_import_requests(
        candidate.sheet(),
        asset.acquisition_uri(),
        iced_css_core::stylesheet::requests::ImportRequestLimits {
            entries: nz(100_000),
            retained_bytes: nz(100_000),
            work: nz(100_000),
        },
    )
    .unwrap();
    assert_eq!(imports.len(), 1);
    let child = execute(
        reader
            .read_file_uri(imports[0].acquisition_uri().to_owned(), limits())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(child.body().as_slice(), b"button { color: red }");
    assert_eq!(child.native_path(), root.join("child?.css"));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn acquisition_errors_keep_syntax_mapping_and_read_failures_distinct() {
    let reader = FileReader::new(nz(1));
    assert!(matches!(
        execute(
            reader
                .read_file_uri("file:///a?x".into(), limits())
                .unwrap()
        ),
        Err(FileUriReadError::Uri(FileUriError::Query))
    ));
    assert!(matches!(
        execute(
            reader
                .read_file_uri("file://example.org/a".into(), limits())
                .unwrap()
        ),
        Err(FileUriReadError::Path(FilePathError::UnresolvedAuthority))
    ));
    assert!(matches!(
        execute(
            reader
                .read_file_uri("file:///a%00b".into(), limits())
                .unwrap()
        ),
        Err(FileUriReadError::Path(FilePathError::Nul))
    ));
    let absent = std::env::temp_dir()
        .join(format!("iced-css-absent-uri-{}", std::process::id()))
        .join("absent.css");
    let uri = unix_file_uri(&absent, limits().path).unwrap();
    assert!(matches!(
        execute(reader.read_file_uri(uri, limits()).unwrap()),
        Err(FileUriReadError::Read(_))
    ));
}
#[test]
fn owned_input_and_completed_body_limits_are_enforced() {
    let reader = FileReader::new(nz(1));
    let mut input = String::with_capacity(8192);
    input.push_str("file:///");
    assert!(matches!(
        reader.read_file_uri(input, limits()),
        Err(FileUriSubmitError::UriBytes)
    ));
    assert_eq!(reader.in_flight(), 0);
    let native =
        std::env::temp_dir().join(format!("iced-css-uri-body-bound-{}", std::process::id()));
    std::fs::write(&native, b"1234").unwrap();
    let uri = unix_file_uri(&native, limits().path).unwrap();
    let mut small = limits();
    small.read.body.retained_bytes = nz(3);
    assert!(matches!(
        execute(reader.read_file_uri(uri, small).unwrap()),
        Err(FileUriReadError::Read(
            iced_css::resources::local::ReadBodyError::Body(BodyError::AdvertisedLength)
        ))
    ));
    std::fs::remove_file(native).unwrap();
}

fn parse_asset(
    asset: &iced_css::resources::file_uri_read::FileUriAsset,
) -> iced_css_core::stylesheet::UnicodeStylesheetCandidate {
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
    let a = nz(100000);
    parse_separate_unicode_stylesheet(
        asset.body().as_slice(),
        EncodingMetadata {
            transport: None,
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
    .unwrap()
}
