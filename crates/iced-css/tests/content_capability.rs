use iced_css::capture::{
    CaptureLimits, ContentCapability, ControlContentFamily, ReplacementCapability, adapters,
};
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100_000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn text_rich_ranges_and_control_labels_borrow_original_storage() {
    let text = adapters::text::<iced::Theme, ()>("", limits()).unwrap();
    assert!(matches!(
        text.construction().nodes()[0].content_capability(),
        ContentCapability::Text("")
    ));
    let spans: Vec<iced::widget::text::Span<'_, (), iced::Font>> = vec![
        iced::widget::text::Span::new("é"),
        iced::widget::text::Span::new(""),
        iced::widget::text::Span::new("中\r\n"),
    ];
    let rich = adapters::rich_text_slice::<_, (), iced::Theme, ()>(&spans, limits()).unwrap();
    let node = &rich.construction().nodes()[0];
    match node.content_capability() {
        ContentCapability::RichText { text, runs } => {
            assert_eq!(text, "é中\r\n");
            assert_eq!(runs, [0..2, 2..2, 2..7]);
            assert!(std::ptr::eq(text, node.text().unwrap()));
            assert!(std::ptr::eq(runs, node.text_runs().unwrap()));
        }
        _ => panic!("rich source must retain typed runs"),
    }
    assert_eq!(rich.construction().nodes().len(), 1);
    let absent = adapters::checkbox::<(), iced::Theme, ()>(false, limits()).unwrap();
    assert!(matches!(
        absent.construction().nodes()[0].content_capability(),
        ContentCapability::CheckboxLabel(None)
    ));
    let empty = absent.label("", limits()).unwrap();
    assert!(matches!(
        empty.construction().nodes()[0].content_capability(),
        ContentCapability::CheckboxLabel(Some(""))
    ));
    let absent = adapters::toggler::<(), iced::Theme, ()>(false, limits()).unwrap();
    assert!(matches!(
        absent.construction().nodes()[0].content_capability(),
        ContentCapability::TogglerLabel(None)
    ));
    let empty = absent.label("", limits()).unwrap();
    assert!(matches!(
        empty.construction().nodes()[0].content_capability(),
        ContentCapability::TogglerLabel(Some(""))
    ));
    let radio = adapters::radio::<(), iced::Theme, (), _>("", 1u8, None, |_| (), limits()).unwrap();
    let content = radio.construction().nodes()[0].content_capability();
    assert!(matches!(content, ContentCapability::RadioLabel("")));
    assert_eq!(
        content.replacement(),
        ReplacementCapability::RecipeRequired(ControlContentFamily::Radio)
    );
}

#[cfg(feature = "image-without-codecs")]
#[test]
fn image_capability_borrows_typed_resource_even_for_invalid_rgba() {
    // Approved capture API; actual typed record qualification is prerequisite.
    let handle = iced::widget::image::Handle::from_rgba(2, 3, vec![0u8; 1]);
    let captured = adapters::image(handle, limits()).unwrap();
    let node = &captured.construction().nodes()[0];
    match node.content_capability() {
        ContentCapability::Image(source) => {
            assert!(std::ptr::eq(source, node.image_source().unwrap()));
            assert!(std::ptr::eq(
                source.handle(),
                node.image_source().unwrap().handle()
            ));
            assert!(source.rgba_declaration().is_some());
        }
        _ => panic!("typed image capability required"),
    }
    assert_eq!(
        node.content_capability().replacement(),
        ReplacementCapability::Replaced
    );
}
#[cfg(feature = "svg")]
#[test]
fn svg_capability_borrows_typed_resource() {
    let handle = iced::widget::svg::Handle::from_memory(b"<svg/>".to_vec());
    let captured = adapters::svg::<iced::Theme>(handle, limits()).unwrap();
    let node = &captured.construction().nodes()[0];
    match node.content_capability() {
        ContentCapability::Svg(source) => assert!(std::ptr::eq(source, node.svg_source().unwrap())),
        _ => panic!("typed svg capability required"),
    }
}
