use iced_css::capture::{CaptureLimits, CapturedElement, adapters};
use iced_test::{
    core::{Point, Size, clipboard, mouse},
    runtime::{UserInterface, user_interface::Cache},
};
use std::{cell::Cell, num::NonZeroUsize};

fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    }
}

fn click<Message>(element: iced::Element<'_, Message, iced::Theme, ()>) -> Vec<Message> {
    let mut renderer = ();
    let mut ui = UserInterface::build(
        element,
        Size::new(100.0, 100.0),
        Cache::default(),
        &mut renderer,
    );
    let mut messages = Vec::new();
    let _ = ui.update(
        &iced_test::simulator::click().collect::<Vec<_>>(),
        mouse::Cursor::Available(Point::new(1.0, 1.0)),
        &mut renderer,
        &mut clipboard::Null,
        &mut messages,
    );
    messages
}

#[test]
fn map_and_explain_preserve_evidence_and_only_dispatch_callbacks_on_click() {
    let presses = Cell::new(0);
    let mappings = Cell::new(0);
    let borrowed = String::from("label");
    let child = adapters::text::<iced::Theme, ()>(&borrowed, limits()).unwrap();
    let button = adapters::button(child, limits())
        .unwrap()
        .on_press_with(|| {
            presses.set(presses.get() + 1);
            7u8
        });
    let evidence_address = button.construction().nodes().as_ptr();
    let button: CapturedElement<'_, u8, iced::Theme, ()> = button.erase();
    let mapped = button
        .map(|value| {
            mappings.set(mappings.get() + 1);
            u32::from(value) + borrowed.len() as u32
        })
        .explain(iced::Color::BLACK);
    assert_eq!(mapped.construction().nodes().as_ptr(), evidence_address);
    assert_eq!((presses.get(), mappings.get()), (0, 0));
    let (native, evidence) = mapped.into_parts();
    assert_eq!(evidence.nodes().len(), 2);
    assert_eq!(evidence.nodes()[1].canonical_type(), "button");
    assert_eq!(evidence.nodes()[0].parent(), Some(1));
    assert_eq!(evidence.nodes()[0].role(), Some("content"));
    assert_eq!(click(native), [12]);
    assert_eq!((presses.get(), mappings.get()), (1, 1));
}

#[test]
fn direct_and_optional_press_builders_keep_native_enable_disable_behavior() {
    for message in [None, Some(3u8)] {
        let child = adapters::text::<iced::Theme, ()>("label", limits()).unwrap();
        let button = adapters::button(child, limits())
            .unwrap()
            .on_press(8)
            .on_press_maybe(message);
        let button: CapturedElement<'_, u8, iced::Theme, ()> = button.erase();
        assert_eq!(
            click(button.into_parts().0),
            message.into_iter().collect::<Vec<_>>()
        );
    }
}
