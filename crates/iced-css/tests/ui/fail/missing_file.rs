use iced_css::StyleClasses;
use iced::widget::{container, text};

#[derive(Debug, Clone)]
enum Msg {}

#[iced_css::style("tests/ui/css/missing.css", policy = Compile)]
fn view() -> iced::Element<'static, Msg> {
    container(text("hello world")).style_classes(["btn"])
}

fn main() {
    let _ = view;
}
