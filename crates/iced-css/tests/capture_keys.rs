use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, Construction, adapters};
use iced_css_core::source::{
    AttributeLimits,
    sibling_matches::{MatchLimits, SiblingKey, match_siblings},
};
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: n(100),
        depth: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    }
}
fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    }
}
fn text() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text("text", limits()).unwrap().erase()
}
#[test]
fn explicit_keys_are_owned_and_independent_of_style_metadata() {
    let mut bytes = vec![0, 255, 1];
    let captured = text()
        .source_key(&bytes, limits())
        .unwrap()
        .style_id("id", attrs())
        .unwrap()
        .style_classes(&["class"], attrs())
        .unwrap();
    bytes[0] = 9;
    let node = &captured.construction().nodes()[0];
    assert_eq!(node.source_key(), Some(&[0, 255, 1][..]));
    assert_eq!(node.metadata().css_id(), Some("id"));
    assert!(node.metadata().has_class("class"));
    assert_eq!(text().construction().nodes()[0].source_key(), None);
    assert_eq!(
        text()
            .source_key(&[], limits())
            .unwrap()
            .construction()
            .nodes()[0]
            .source_key(),
        Some(&[][..])
    );
}
#[test]
fn wrapping_erasure_and_message_mapping_preserve_the_child_key() {
    let child = text().source_key(b"child", limits()).unwrap().map(|_| 7u8);
    let parent = adapters::container(child, limits())
        .unwrap()
        .erase::<u8, iced::Theme, ()>();
    assert_eq!(
        parent.construction().nodes()[0].source_key(),
        Some(&b"child"[..])
    );
    assert_eq!(
        parent.construction().nodes()[parent.construction().root()].source_key(),
        None
    );
    let parent = parent.source_key(b"parent", limits()).unwrap();
    assert_eq!(
        parent.construction().nodes()[0].source_key(),
        Some(&b"child"[..])
    );
    assert_eq!(
        parent.construction().nodes()[parent.construction().root()].source_key(),
        Some(&b"parent"[..])
    );
}
fn row(keys: &[&[u8]]) -> Construction {
    let children: Vec<_> = keys
        .iter()
        .map(|key| text().source_key(key, limits()).unwrap())
        .collect();
    adapters::row_from_vec(children, limits())
        .unwrap()
        .into_parts()
        .1
}
#[test]
fn captured_keys_drive_exact_sibling_matching_across_reordered_views() {
    let before = row(&[b"a", b"b"]);
    let after = row(&[b"b", b"c", b"a"]);
    let keys = |construction: &Construction| {
        construction
            .nodes()
            .iter()
            .filter(|node| node.parent().is_some())
            .map(|node| node.source_key().unwrap().to_vec())
            .collect::<Vec<_>>()
    };
    let old = keys(&before);
    let new = keys(&after);
    let old_keys: Vec<_> = old.iter().map(|key| SiblingKey::Explicit(key)).collect();
    let new_keys: Vec<_> = new.iter().map(|key| SiblingKey::Explicit(key)).collect();
    let matched = match_siblings(
        &old_keys,
        &new_keys,
        MatchLimits {
            entries: n(100),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    assert_eq!(matched.previous_indices(), &[Some(1), None, Some(0)]);
}
#[test]
fn replacement_releases_obsolete_key_capacity_and_parent_charges_child_keys() {
    let base = text().construction().retained_bytes();
    let large = text().source_key(&[1; 4096], limits()).unwrap();
    assert!(large.construction().retained_bytes() >= base + 4096);
    let replaced = large.source_key(&[], limits()).unwrap();
    assert_eq!(replaced.construction().retained_bytes(), base);
    let child = text().source_key(&[1; 4096], limits()).unwrap();
    let mut small = limits();
    small.retained_bytes = n(4096);
    assert!(matches!(
        adapters::container(child, small),
        Err(CaptureError::Bytes)
    ));
}
#[test]
fn key_replacement_enforces_each_construction_budget() {
    let mut bytes = limits();
    bytes.retained_bytes = n(1);
    assert!(matches!(
        text().source_key(b"a", bytes),
        Err(CaptureError::Bytes)
    ));
    let mut work = limits();
    work.work = n(1);
    assert!(matches!(
        text().source_key(b"a", work),
        Err(CaptureError::Work)
    ));
    let mut nodes = limits();
    nodes.nodes = n(1);
    assert!(matches!(
        adapters::container(text(), limits())
            .unwrap()
            .source_key(b"a", nodes),
        Err(CaptureError::Nodes)
    ));
    let mut depth = limits();
    depth.depth = n(1);
    assert!(matches!(
        adapters::container(text(), limits())
            .unwrap()
            .source_key(b"a", depth),
        Err(CaptureError::Depth)
    ));
}
