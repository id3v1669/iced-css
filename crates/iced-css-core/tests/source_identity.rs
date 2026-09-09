use iced_css_core::identity::{IdAllocator, IdentityError, SourceRegistry};
use iced_css_core::source::{Metadata, SourceNode, validate_tree};
use std::num::NonZeroU64;
fn limit(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}
#[test]
fn checked_ids_exhaust_without_reuse() {
    let mut ids = IdAllocator::new(limit(2));
    assert_eq!(ids.allocate().unwrap().get(), 1);
    assert_eq!(ids.allocate().unwrap().get(), 2);
    assert_eq!(ids.allocate(), Err(IdentityError::Exhausted));
    assert_eq!(ids.allocate(), Err(IdentityError::Exhausted));
}
#[test]
fn foreign_and_removed_tokens_cannot_retarget_live_instances() {
    let mut a = SourceRegistry::new(limit(3));
    let mut b = SourceRegistry::new(limit(3));
    let old = a.insert().unwrap();
    let foreign = b.insert().unwrap();
    assert_eq!(a.validate(&foreign), Err(IdentityError::WrongHost));
    assert_eq!(a.remove(&foreign), Err(IdentityError::WrongHost));
    let old_id = a.validate(&old).unwrap();
    a.remove(&old).unwrap();
    let fresh = a.insert().unwrap();
    assert_eq!(a.validate(&old), Err(IdentityError::DeadInstance));
    assert_ne!(a.validate(&fresh).unwrap(), old_id);
}
#[test]
fn token_clone_and_host_move_preserve_scope_but_not_liveness() {
    let mut registry = SourceRegistry::new(limit(1));
    let token = registry.insert().unwrap();
    let copy = token.clone();
    let mut moved = registry;
    assert!(moved.validate(&copy).is_ok());
    moved.remove(&token).unwrap();
    assert_eq!(moved.validate(&copy), Err(IdentityError::DeadInstance));
    assert!(matches!(moved.insert(), Err(IdentityError::Exhausted)));
    let replacement = SourceRegistry::new(limit(1));
    assert_eq!(replacement.validate(&copy), Err(IdentityError::WrongHost));
}
#[test]
fn metadata_builders_share_slots_and_reject_invalid_classes_atomically() {
    let mut m = Metadata::default();
    m.set_classes(&["a", "b"]).unwrap();
    assert_eq!(m.attribute("class"), Some("a b"));
    m.set_attribute("class", "other\tlast");
    assert!(m.has_class("last"));
    assert!(!m.has_class("a"));
    assert!(m.set_classes(&["valid", "bad\nclass"]).is_err());
    assert_eq!(m.attribute("class"), Some("other\tlast"));
    assert!(m.set_classes(&[""]).is_err());
    m.set_classes(&["A\u{00a0}B"]).unwrap();
    assert!(m.has_class("A\u{00a0}B"));
    m.set_id("one");
    m.set_attribute("id", "two");
    assert_eq!(m.css_id(), Some("two"));
    m.set_id("");
    assert_eq!(m.css_id(), None);
    assert_eq!(m.attribute("id"), Some(""));
}
#[test]
fn explicit_empty_language_stops_inheritance_and_case_is_preserved() {
    let mut m = Metadata::default();
    assert_eq!(m.language(Some("en")), Some("en"));
    m.set_attribute("lang", "");
    assert_eq!(m.language(Some("en")), None);
    m.set_attribute("lang", "FR");
    assert_eq!(m.language(Some("en")), Some("FR"));
    m.set_attribute("CLASS", "a");
    assert!(!m.has_class("a"));
}
fn nodes() -> Vec<SourceNode> {
    let mut ids = IdAllocator::new(limit(2));
    let root = ids.allocate().unwrap();
    let child = ids.allocate().unwrap();
    vec![
        SourceNode {
            instance: root,
            parent: None,
            children: vec![child],
            metadata: Metadata::default(),
        },
        SourceNode {
            instance: child,
            parent: Some(root),
            children: vec![],
            metadata: Metadata::default(),
        },
    ]
}
#[test]
fn tree_validation_requires_reciprocal_single_parent_membership() {
    let tree = nodes();
    assert!(validate_tree(&tree).is_ok());
    let mut broken = tree.clone();
    broken[0].children.clear();
    assert!(validate_tree(&broken).is_err());
    let mut broken = tree.clone();
    broken[1].children.push(tree[0].instance);
    assert!(validate_tree(&broken).is_err());
    let mut broken = tree.clone();
    broken[0].children.push(tree[1].instance);
    assert!(validate_tree(&broken).is_err());
    let mut broken = tree.clone();
    broken[1].parent = None;
    assert!(validate_tree(&broken).is_err());
    assert!(validate_tree(&[]).is_err());
}
#[test]
fn duplicate_nonempty_css_ids_fail_without_mutating_candidate() {
    let mut tree = nodes();
    tree[0].metadata.set_id("same");
    tree[1].metadata.set_id("same");
    assert!(validate_tree(&tree).is_err());
    assert_eq!(tree[1].metadata.css_id(), Some("same"));
    tree[0].metadata.set_id("");
    tree[1].metadata.set_id("");
    assert!(validate_tree(&tree).is_ok());
}
