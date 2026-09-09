#![cfg(unix)]
use iced_css::resources::{
    file_path::{FilePathError, FilePathLimits, unix_path},
    local::ReadBodyLimits,
    worker::{BackgroundReadLimits, FileReader},
};
use iced_css_core::resources::{body::BodyLimits, file_uri::FileUri, uri::UriLimits};
use std::{
    ffi::OsString,
    num::NonZeroUsize,
    os::unix::ffi::{OsStrExt, OsStringExt},
};
fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn uri_limits() -> UriLimits {
    UriLimits {
        bytes: nz(4096),
        work: nz(100_000),
    }
}
fn path_limits() -> FilePathLimits {
    FilePathLimits {
        bytes: nz(4096),
        work: nz(100_000),
    }
}

#[test]
fn unix_mapping_preserves_bytes_and_does_not_infer_windows_drives() {
    for (uri, expected) in [
        ("file:///tmp/a:", &b"/tmp/a:"[..]),
        ("file:///tmp/a%7C", &b"/tmp/a|"[..]),
        (
            "file://LOCALHOST/Tmp/~user/%FF%80%25%252F",
            &b"/Tmp/~user/\xff\x80%%2F"[..],
        ),
        ("file:/a//b/", &b"/a//b/"[..]),
        ("file:/%2e%2E/a", &b"/../a"[..]),
        ("file:/%E2%82%AC", &b"/\xe2\x82\xac"[..]),
        ("file:/", &b"/"[..]),
    ] {
        let result = unix_path(FileUri::parse(uri, uri_limits()).unwrap(), path_limits()).unwrap();
        assert_eq!(result.as_os_str().as_bytes(), expected, "{uri}");
    }
}

#[test]
fn invalid_native_segments_and_unresolved_hosts_are_explicit_errors() {
    for (uri, expected) in [
        ("file:///tmp/a%00b", FilePathError::Nul),
        ("file:///tmp/a%2Fb", FilePathError::EncodedSeparator),
        ("file:///tmp/a%2fb", FilePathError::EncodedSeparator),
        (
            "file://example.org/tmp/a",
            FilePathError::UnresolvedAuthority,
        ),
        ("file://[::1]/tmp/a", FilePathError::UnresolvedAuthority),
    ] {
        assert_eq!(
            unix_path(FileUri::parse(uri, uri_limits()).unwrap(), path_limits()),
            Err(expected)
        );
    }
}

#[test]
fn decoded_capacity_and_preprocessing_work_are_bounded() {
    let uri = FileUri::parse("file:/%FF%80", uri_limits()).unwrap();
    let mut limits = path_limits();
    limits.bytes = nz(2);
    assert_eq!(unix_path(uri, limits), Err(FilePathError::Bytes));
    limits.bytes = nz(3);
    let path = unix_path(uri, limits).unwrap();
    assert_eq!(path.as_os_str().as_bytes(), b"/\xff\x80");
    assert!(path.capacity() <= 3);
    limits = path_limits();
    limits.work = nz(1);
    assert_eq!(unix_path(uri, limits), Err(FilePathError::Work));
    limits = path_limits();
    limits.bytes = nz(usize::MAX);
    assert_eq!(unix_path(uri, limits), Err(FilePathError::Bytes));
}

#[test]
fn real_byte_named_files_are_read_through_the_existing_background_owner() {
    let root = std::env::temp_dir().join(format!("iced-css-native-name-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let reader = FileReader::new(nz(1));
    for (name, encoded) in [(&b"a:"[..], "a:"), (&b"\xff%?"[..], "%FF%25%3F")] {
        let native = root.join(OsString::from_vec(name.to_vec()));
        std::fs::write(&native, b"button { margin: 4px }").unwrap();
        let uri = format!("file://{}/{encoded}", root.to_str().unwrap());
        let path = unix_path(FileUri::parse(&uri, uri_limits()).unwrap(), path_limits()).unwrap();
        assert_eq!(path.as_os_str(), native.as_os_str());
        let work = reader
            .read_file(
                path,
                BackgroundReadLimits {
                    path_bytes: nz(4096),
                    read: ReadBodyLimits {
                        reads: nz(10),
                        body: BodyLimits {
                            retained_bytes: nz(4096),
                            work: nz(100_000),
                        },
                    },
                },
            )
            .unwrap();
        let body = iced::futures::executor::block_on(work).unwrap();
        assert_eq!(body.as_slice(), b"button { margin: 4px }");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_paths_produce_explicit_escaped_stylesheet_bases() {
    use iced_css::resources::file_path::unix_file_uri;
    use std::path::Path;
    for (native, expected) in [
        (&b"/"[..], "file:///"),
        (&b"/Tmp/a:~%?#\\"[..], "file:///Tmp/a%3A~%25%3F%23%5C"),
        (&b"/\xff\x80\xe2\x82\xac"[..], "file:///%FF%80%E2%82%AC"),
        (&b"/a//b/"[..], "file:///a//b/"),
    ] {
        let path = Path::new(std::ffi::OsStr::from_bytes(native));
        assert_eq!(unix_file_uri(path, path_limits()).unwrap(), expected);
    }
}

#[test]
fn every_unix_filename_octet_survives_uri_round_trip() {
    use iced_css::resources::file_path::unix_file_uri;
    let mut bytes = b"/".to_vec();
    bytes.extend((1..=255u8).filter(|&b| b != b'/'));
    let path = std::path::PathBuf::from(OsString::from_vec(bytes.clone()));
    let encoded = unix_file_uri(&path, path_limits()).unwrap();
    assert_eq!(
        encoded,
        include_str!("fixtures/file-path/all-octets-uri.txt").trim_end()
    );
    let uri = FileUri::parse(&encoded, uri_limits()).unwrap();
    let decoded = unix_path(uri, path_limits()).unwrap();
    assert_eq!(decoded.as_os_str().as_bytes(), bytes);
}

#[test]
fn native_base_encoding_rejects_ambiguous_roots_and_enforces_bounds() {
    use iced_css::resources::file_path::unix_file_uri;
    use std::path::Path;
    for raw in ["", "a/b", "C:/a"] {
        assert_eq!(
            unix_file_uri(Path::new(raw), path_limits()),
            Err(FilePathError::RelativePath)
        );
    }
    assert_eq!(
        unix_file_uri(Path::new("//server/a"), path_limits()),
        Err(FilePathError::DoubleRoot)
    );
    assert_eq!(
        unix_file_uri(Path::new("/a\0b"), path_limits()),
        Err(FilePathError::Nul)
    );
    let mut limits = path_limits();
    limits.bytes = nz(7);
    assert_eq!(
        unix_file_uri(Path::new("/"), limits),
        Err(FilePathError::Bytes)
    );
    limits.bytes = nz(8);
    assert_eq!(unix_file_uri(Path::new("/"), limits).unwrap(), "file:///");
    limits = path_limits();
    limits.work = nz(1);
    assert_eq!(
        unix_file_uri(Path::new("/"), limits),
        Err(FilePathError::Work)
    );
}
