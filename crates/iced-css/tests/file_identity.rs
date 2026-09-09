#![cfg(unix)]
use iced_css::resources::{
    file_identity::{FileIdentityPool, IdentityReadError},
    local::ReadBodyLimits,
};
use iced_css_core::resources::body::BodyLimits;
use std::{num::NonZeroUsize, path::PathBuf};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> ReadBodyLimits {
    ReadBodyLimits {
        reads: nz(10),
        body: BodyLimits {
            retained_bytes: nz(1024),
            work: nz(10000),
        },
    }
}
struct Directory(PathBuf);
impl Directory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("iced-css-identity-{name}-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn links_share_identity_but_equal_bytes_in_distinct_files_do_not() {
    let root = Directory::new("links");
    let path = root.0.join("a");
    std::fs::write(&path, b"original").unwrap();
    std::fs::hard_link(&path, root.0.join("hard")).unwrap();
    std::os::unix::fs::symlink("a", root.0.join("soft")).unwrap();
    std::fs::write(root.0.join("different"), b"original").unwrap();
    let pool = FileIdentityPool::new(nz(4));
    let a = pool.open(&path).unwrap().read(limits()).unwrap();
    let hard = pool
        .open(&root.0.join("hard"))
        .unwrap()
        .read(limits())
        .unwrap();
    let soft = pool
        .open(&root.0.join("soft"))
        .unwrap()
        .read(limits())
        .unwrap();
    let different = pool
        .open(&root.0.join("different"))
        .unwrap()
        .read(limits())
        .unwrap();
    assert_eq!(a.identity(), hard.identity());
    assert_eq!(a.identity(), soft.identity());
    assert_ne!(a.identity(), different.identity());
    assert_eq!(a.body().as_slice(), b"original");
    assert_eq!(pool.retained_handles(), 4);
}
#[test]
fn read_uses_the_opened_object_after_last_link_removal_and_path_replacement() {
    let root = Directory::new("replacement");
    let path = root.0.join("theme.css");
    std::fs::write(&path, b"old").unwrap();
    let pool = FileIdentityPool::new(nz(3));
    let before = pool.open(&path).unwrap();
    let alias = pool.open(&path).unwrap().read(limits()).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"new").unwrap();
    let after = pool.open(&path).unwrap().read(limits()).unwrap();
    let old = before.read(limits()).unwrap();
    assert_eq!(old.body().as_slice(), b"old");
    assert_eq!(after.body().as_slice(), b"new");
    assert_eq!(old.identity(), alias.identity());
    assert_ne!(old.identity(), after.identity());
    let (body, identity) = old.into_parts();
    drop(body);
    assert_eq!(pool.retained_handles(), 3);
    drop(identity);
    assert_eq!(pool.retained_handles(), 2);
}
#[test]
fn shared_handle_budget_outlives_reads_and_failures_release_admission() {
    let root = Directory::new("budget");
    let path = root.0.join("a");
    std::fs::write(&path, b"four").unwrap();
    let pool = FileIdentityPool::new(nz(1));
    let clone = pool.clone();
    assert!(matches!(
        pool.open(&root.0.join("missing")),
        Err(IdentityReadError::Io(_))
    ));
    assert_eq!(pool.retained_handles(), 0);
    assert!(matches!(
        pool.open(&root.0),
        Err(IdentityReadError::NotRegular)
    ));
    assert_eq!(pool.retained_handles(), 0);
    let opened = pool.open(&path).unwrap();
    assert!(matches!(clone.open(&path), Err(IdentityReadError::Handles)));
    let body = opened.read(limits()).unwrap();
    assert!(matches!(clone.open(&path), Err(IdentityReadError::Handles)));
    drop(body);
    assert_eq!(clone.retained_handles(), 0);
    let mut small = limits();
    small.body.retained_bytes = nz(1);
    assert!(matches!(
        clone.open(&path).unwrap().read(small),
        Err(IdentityReadError::Read(_))
    ));
    assert_eq!(pool.retained_handles(), 0);
}
#[test]
fn in_place_edits_preserve_identity_without_claiming_a_content_revision() {
    let root = Directory::new("edit");
    let path = root.0.join("a");
    std::fs::write(&path, b"one").unwrap();
    let pool = FileIdentityPool::new(nz(2));
    let old = pool.open(&path).unwrap().read(limits()).unwrap();
    std::fs::write(&path, b"two").unwrap();
    let new = pool.open(&path).unwrap().read(limits()).unwrap();
    assert_eq!(old.identity(), new.identity());
    assert_eq!(old.body().as_slice(), b"one");
    assert_eq!(new.body().as_slice(), b"two");
}

#[test]
fn background_uri_acquisition_preserves_request_and_descriptor_admission() {
    use iced_css::resources::{
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::{FileUriReadError, FileUriReadLimits},
        worker::FileReader,
    };
    use iced_css_core::resources::uri::UriLimits;
    let root = Directory::new("uri");
    let path = root.0.join("theme:%.css");
    std::fs::write(&path, b"button { color: red }").unwrap();
    let limits = FileUriReadLimits {
        uri: UriLimits {
            bytes: nz(4096),
            work: nz(100000),
        },
        path: FilePathLimits {
            bytes: nz(4096),
            work: nz(100000),
        },
        read: limits(),
    };
    let uri = unix_file_uri(&path, limits.path).unwrap();
    let reader = FileReader::new(nz(1));
    let pool = FileIdentityPool::new(nz(1));
    let work = reader
        .read_identified_file_uri(uri.clone(), limits, pool.clone())
        .unwrap();
    fn execute<F: std::future::Future + Send + 'static>(f: F) -> F::Output {
        iced::futures::executor::block_on(f)
    }
    let result = execute(work).unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(result.asset().acquisition_uri(), uri);
    assert_eq!(result.asset().native_path(), path);
    assert_eq!(result.asset().body().as_slice(), b"button { color: red }");
    assert_eq!(pool.retained_handles(), 1);
    assert!(matches!(
        execute(
            reader
                .read_identified_file_uri(uri.clone(), limits, pool.clone())
                .unwrap()
        ),
        Err(FileUriReadError::Identity(IdentityReadError::Handles))
    ));
    let (asset, identity) = result.into_parts();
    drop(asset);
    assert_eq!(pool.retained_handles(), 1);
    drop(identity);
    assert_eq!(pool.retained_handles(), 0);
    let next = execute(
        reader
            .read_identified_file_uri(uri, limits, pool.clone())
            .unwrap(),
    )
    .unwrap();
    let other_pool = FileIdentityPool::new(nz(1));
    let direct = other_pool.open(&path).unwrap().read(limits.read).unwrap();
    assert_eq!(next.identity(), direct.identity());
    drop(next);
    assert_eq!(pool.retained_handles(), 0);
}

#[test]
fn symlink_retarget_and_atomic_rename_do_not_relabel_an_opened_body() {
    let root = Directory::new("retarget");
    let a = root.0.join("a");
    let b = root.0.join("b");
    let link = root.0.join("link");
    std::fs::write(&a, b"aaa").unwrap();
    std::fs::write(&b, b"bbb").unwrap();
    std::os::unix::fs::symlink("a", &link).unwrap();
    let pool = FileIdentityPool::new(nz(4));
    let opened = pool.open(&link).unwrap();
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("b", &link).unwrap();
    let retargeted = pool.open(&link).unwrap().read(limits()).unwrap();
    std::fs::rename(&b, &a).unwrap();
    let replacement = pool.open(&a).unwrap().read(limits()).unwrap();
    let original = opened.read(limits()).unwrap();
    assert_eq!(original.body().as_slice(), b"aaa");
    assert_eq!(retargeted.body().as_slice(), b"bbb");
    assert_eq!(replacement.body().as_slice(), b"bbb");
    assert_ne!(original.identity(), retargeted.identity());
    assert_eq!(replacement.identity(), retargeted.identity());
    assert!(matches!(pool.open(&link), Err(IdentityReadError::Io(_))));
    assert_eq!(pool.retained_handles(), 3);
}
