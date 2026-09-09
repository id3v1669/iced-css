use iced::advanced::{text::IntoFragment, widget::Tree};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{borrow::Cow, cell::Cell, num::NonZeroUsize};

fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    }
}

#[test]
fn borrowed_text_and_nested_containers_preserve_owned_source_evidence() {
    let value = String::from("borrowed λ");
    let text = adapters::text::<iced::Theme, ()>(&value, limits()).unwrap();
    let child: CapturedElement<'_, (), iced::Theme, ()> = text.erase();
    let parent = adapters::container(child, limits()).unwrap();
    let root = adapters::container(parent, limits()).unwrap();
    let (native, evidence) = root.into_parts();
    let element: iced::Element<'_, (), iced::Theme, ()> = native.into();
    let tree = Tree::new(&element);
    // Stock Container forwards the child's native state; source records must
    // preserve both containers even though they add no native Tree nodes.
    let plain: iced::Element<'_, (), iced::Theme, ()> = iced::widget::text("plain").into();
    assert_eq!(tree.tag, Tree::new(&plain).tag);
    assert!(tree.children.is_empty());
    drop(element);
    drop(value);
    assert_eq!(evidence.root(), 2);
    assert_eq!(evidence.depth(), 3);
    assert_eq!(evidence.nodes().len(), 3);
    assert_eq!(evidence.nodes()[0].text(), Some("borrowed λ"));
    assert_eq!(evidence.nodes()[0].parent(), Some(1));
    assert_eq!(evidence.nodes()[1].parent(), Some(2));
    assert_eq!(evidence.nodes()[2].parent(), None);
    assert_eq!(evidence.nodes()[0].canonical_type(), "text");
    assert_eq!(evidence.nodes()[1].canonical_type(), "container");
    assert_eq!(evidence.nodes()[0].role(), Some("content"));
    assert_eq!(evidence.nodes()[2].role(), None);
}

struct Counted<'a>(&'a Cell<usize>);
impl<'a> IntoFragment<'a> for Counted<'a> {
    fn into_fragment(self) -> Cow<'a, str> {
        self.0.set(self.0.get() + 1);
        Cow::Owned("owned text".into())
    }
}

#[test]
fn fragment_conversion_executes_once_and_erasure_preserves_evidence() {
    let calls = Cell::new(0);
    let text = adapters::text::<iced::Theme, ()>(Counted(&calls), limits()).unwrap();
    assert_eq!(calls.get(), 1);
    let text: CapturedElement<'_, (), iced::Theme, ()> = text.erase();
    let (_, evidence) = text.into_parts();
    assert_eq!(calls.get(), 1);
    assert_eq!(evidence.nodes()[0].text(), Some("owned text"));
    assert_eq!(evidence.depth(), 1);
    assert_eq!(evidence.root(), 0);
}

#[test]
fn evidence_limits_reject_before_a_parent_can_be_returned() {
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        adapters::text::<iced::Theme, ()>("x", small),
        Err(CaptureError::Bytes)
    ));
    small = limits();
    small.work = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        adapters::text::<iced::Theme, ()>("abc", small),
        Err(CaptureError::Work)
    ));
    for expected in [
        CaptureError::Nodes,
        CaptureError::Depth,
        CaptureError::Bytes,
        CaptureError::Work,
    ] {
        let child: CapturedElement<'_, (), iced::Theme, ()> =
            adapters::text("child", limits()).unwrap().erase();
        let mut small = limits();
        match expected {
            CaptureError::Nodes => small.nodes = NonZeroUsize::new(1).unwrap(),
            CaptureError::Depth => small.depth = NonZeroUsize::new(1).unwrap(),
            CaptureError::Bytes => small.retained_bytes = NonZeroUsize::new(1).unwrap(),
            CaptureError::Work => small.work = NonZeroUsize::new(1).unwrap(),
            _ => unreachable!(),
        }
        assert!(matches!(adapters::container(child, small), Err(actual) if actual == expected));
    }
}
