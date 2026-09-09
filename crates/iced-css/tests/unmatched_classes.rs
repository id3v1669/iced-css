use iced::{
    Element, Size,
    advanced::{layout, widget::Tree},
    widget::{Space, container},
};
use std::cell::Cell;
fn width(mut e: Element<'_, (), iced::Theme, ()>) -> f32 {
    let mut tree = Tree::new(&e);
    e.as_widget_mut()
        .layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
        )
        .size()
        .width
}
#[iced_css::style(inline=".matched { width: 37px; } .unused { width: 90px; }",policy=Compile)]
fn absent(calls: &Cell<usize>) -> Element<'static, (), iced::Theme, ()> {
    let build = || {
        calls.set(calls.get() + 1);
        container(Space::new())
    };
    build().style_classes(["absent"])
}
#[iced_css::style(inline=".matched { width: 37px; } .unused { width: 90px; }",policy=Compile)]
fn empty() -> Element<'static, (), iced::Theme, ()> {
    container(Space::new()).style_classes([])
}
#[iced_css::style(inline=".matched { width: 37px; } .unused { width: 90px; }",policy=Compile)]
fn mixed() -> Element<'static, (), iced::Theme, ()> {
    container(Space::new()).style_classes(["absent", "matched"])
}
#[iced_css::style(inline=".matched { width: 37px; } .unused { width: 90px; }",policy=Compile)]
fn reordered() -> Element<'static, (), iced::Theme, ()> {
    container(Space::new()).style_classes(["matched", "absent", "matched"])
}
#[test]
fn unmatched_membership_is_valid_and_construction_runs_once() {
    let calls = Cell::new(0);
    assert_eq!(width(absent(&calls)), width(empty()));
    assert_eq!(calls.get(), 1);
}
#[test]
fn unmatched_members_do_not_change_rule_selection_or_membership_order() {
    assert_eq!(width(mixed()), 37.0);
    assert_eq!(width(reordered()), 37.0);
}
