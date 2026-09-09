use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::RefCell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
thread_local! {static TRACE:RefCell<Vec<&'static str>>=const {RefCell::new(Vec::new())};}
struct Theme;
impl iced::widget::text::Catalog for Theme {
    type Class<'a> = ();
    fn default<'a>() -> Self::Class<'a> {
        TRACE.with(|v| v.borrow_mut().push("text"));
    }
    fn style(&self, _: &Self::Class<'_>) -> iced::widget::text::Style {
        Default::default()
    }
}
impl iced::widget::container::Catalog for Theme {
    type Class<'a> = ();
    fn default<'a>() -> Self::Class<'a> {
        TRACE.with(|v| v.borrow_mut().push("container"));
    }
    fn style(&self, _: &Self::Class<'_>) -> iced::widget::container::Style {
        Default::default()
    }
}
fn take_trace() -> Vec<&'static str> {
    TRACE.with(|v| std::mem::take(&mut *v.borrow_mut()))
}
#[test]
fn native_string_conversion_stays_at_parent_construction() {
    let value = String::from("borrowed");
    let child = adapters::str_child(value.as_str(), limits()).unwrap();
    assert!(take_trace().is_empty());
    assert_eq!(child.construction().nodes().len(), 1);
    assert_eq!(child.construction().nodes()[0].canonical_type(), "text");
    assert_eq!(child.construction().nodes()[0].text(), Some("borrowed"));
    let captured = adapters::container::<(), Theme, (), _>(child, limits()).unwrap();
    let trace = take_trace();
    assert_eq!(trace, ["text", "container"]);
    let _: iced::widget::Container<'_, (), Theme, ()> =
        iced::widget::Container::new(value.as_str());
    assert_eq!(trace, take_trace());
    assert_eq!(captured.construction().nodes()[0].parent(), Some(1));
    let child = adapters::str_child(value.as_str(), limits()).unwrap();
    let mut cap = limits();
    cap.nodes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::container::<(), Theme, (), _>(child, cap),
        Err(CaptureError::Nodes)
    ));
    assert!(take_trace().is_empty());
    let _ = adapters::str_child(value.as_str(), limits())
        .unwrap()
        .erase::<(), Theme, ()>();
    assert_eq!(take_trace(), ["text"]);
}
#[test]
fn source_text_is_owned_and_admission_observes_byte_and_work_bounds() {
    let value = String::from("source");
    let (_, source) = adapters::str_child(value.as_str(), limits())
        .unwrap()
        .into_parts();
    drop(value);
    assert_eq!(source.nodes()[0].text(), Some("source"));
    for bytes in [false, true] {
        let mut cap = limits();
        if bytes {
            cap.retained_bytes = NonZeroUsize::MIN;
        } else {
            cap.work = NonZeroUsize::MIN;
        }
        assert!(
            matches!(adapters::str_child("source",cap),Err(e) if e==if bytes{CaptureError::Bytes}else{CaptureError::Work})
        );
    }
}
