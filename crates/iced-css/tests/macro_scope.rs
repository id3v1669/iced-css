use iced::{
    Element, Size,
    advanced::{layout, widget::Tree},
    widget::{Space, container},
};
use std::cell::Cell;
fn width(mut element: Element<'_, (), iced::Theme, ()>) -> f32 {
    let mut tree = Tree::new(&element);
    element
        .as_widget_mut()
        .layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
        )
        .size()
        .width
}
struct Marker;
impl Marker {
    const fn style_classes(self, _: [&str; 1]) -> usize {
        1
    }
}
#[iced_css::style(inline=".outer { width: 37px; }",policy=Compile)]
fn ordinary_nested(
    calls: &Cell<usize>,
    _signature: [(); Marker.style_classes(["signature-only"])],
) -> Element<'static, (), iced::Theme, ()> {
    fn helper(calls: &Cell<usize>) -> usize {
        calls.set(calls.get() + 1);
        Marker.style_classes(["nested-only"])
    }
    assert_eq!(helper(calls), 1);
    container(Space::new()).style_classes(["outer"])
}
#[test]
fn outer_annotation_leaves_nested_items_and_signature_expressions_ordinary() {
    let calls = Cell::new(0);
    assert_eq!(width(ordinary_nested(&calls, [()])), 37.0);
    assert_eq!(calls.get(), 1);
}
#[iced_css::style(inline=".outer { width: 37px; }",policy=Compile)]
fn independently_annotated() -> Element<'static, (), iced::Theme, ()> {
    #[iced_css::style(inline=".inner { width: 11px; }",policy=Compile)]
    fn inner() -> Element<'static, (), iced::Theme, ()> {
        container(Space::new()).style_classes(["inner"])
    }
    inner()
}
#[test]
fn nested_annotation_uses_its_own_stylesheet() {
    assert_eq!(width(independently_annotated()), 11.0);
}
#[iced_css::style(inline=".outer { width: 37px; }",policy=Compile)]
fn closure_view() -> Element<'static, (), iced::Theme, ()> {
    let build = || container(Space::new()).style_classes(["outer"]);
    build()
}
#[iced_css::style(inline=".outer { width: 37px; }",policy=Compile)]
async fn async_view() -> Element<'static, (), iced::Theme, ()> {
    async { container(Space::new()).style_classes(["outer"]) }.await
}
#[test]
fn closures_and_async_expression_bodies_keep_outer_scope() {
    assert_eq!(width(closure_view()), 37.0);
    let mut future = std::pin::pin!(async_view());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(element) = std::future::Future::poll(future.as_mut(), &mut context)
    else {
        panic!("ready fixture")
    };
    assert_eq!(width(element), 37.0);
}
#[iced_css::style(inline=".outer { width: 37px; }",policy=Compile)]
fn temporary_receiver(name: &str) -> f32 {
    // The text borrows a temporary that lives to the end of this statement.
    width(container(iced::widget::text(name.to_uppercase().as_str())).style_classes(["outer"]))
}
#[test]
fn receiver_temporaries_live_to_the_end_of_the_statement() {
    assert_eq!(temporary_receiver("a"), 37.0);
}
