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
            &layout::Limits::new(Size::ZERO, Size::new(100., 100.)),
        )
        .size()
        .width
}
#[iced_css::style(inline=".ok {width:37px}",policy=Compile)]
fn inactive_invalid() -> Element<'static, (), iced::Theme, ()> {
    #[cfg(any())]
    let _ = container(Space::new()).style_classes(missing_array);
    #[cfg_attr(all(), cfg(any()))]
    container(Space::new()).style_classes([missing_literal()]);
    #[cfg(any())]
    {
        let _ = container(Space::new()).style_classes(another_missing_array);
    }
    container(Space::new()).style_classes(["ok"])
}
fn tracked(calls: &Cell<usize>) -> iced::widget::Container<'static, (), iced::Theme, ()> {
    calls.set(calls.get() + 1);
    container(Space::new())
}
#[iced_css::style(inline=".ok {width:37px}",policy=Compile)]
fn guarded_effects(calls: &Cell<usize>) -> Element<'static, (), iced::Theme, ()> {
    #[cfg(any())]
    tracked(calls).style_classes(["ok"]);
    #[cfg_attr(all(), cfg(any()))]
    undefined_in_removed_branch().style_classes(["ok"]);
    tracked(calls).style_classes(["ok"])
}
#[test]
fn inactive_class_errors_do_not_disable_active_sibling_styling() {
    assert_eq!(width(inactive_invalid()), 37.);
}
#[test]
fn valid_guarded_method_statements_keep_their_compiler_conditions() {
    let calls = Cell::new(0);
    assert_eq!(width(guarded_effects(&calls)), 37.);
    assert_eq!(calls.get(), 1);
}
