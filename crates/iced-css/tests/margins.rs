#[macro_use]
mod common;
use common::prelude::*;

css_test!(
    margin_single_value,
    ".m { margin: 10px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b"))).padding(10.0),
);

css_test!(
    margin_two,
    ".m { margin: 10px 20px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b"))).padding([10.0, 20.0]),
);

css_test!(
    margin_three,
    ".m { margin: 10px 20px 30px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b"))).padding(
        iced::Padding { top: 10.0, right: 20.0, bottom: 30.0, left: 20.0 }
    ),
);

css_test!(
    margin_four,
    ".m { margin: 10px 20px 30px 40px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b"))).padding(
        iced::Padding { top: 10.0, right: 20.0, bottom: 30.0, left: 40.0 }
    ),
);

css_test!(
    margin_long,
    ".m { margin-top: 5px; margin-right: 15px; margin-bottom: 25px; margin-left: 35px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b"))).padding(
        iced::Padding { top: 5.0, right: 15.0, bottom: 25.0, left: 35.0 }
    ),
);

css_test!(
    margin_auto_centers,
    ".m { margin: 0 auto; width: 100px; }",
    button(text("b")).style_classes(["m"]),
    container(button(text("b")).width(100.0)).center_x(iced::Length::Fill),
);

css_test!(
    margin_between_siblings,
    ".m { margin: 10px; } .sized { width: 100px; height: 40px; }",
    iced::widget::row![
        button(text("a")).style_classes(["m"]),
        container(text("b")).style_classes(["sized"]),
    ],
    iced::widget::row![
        container(button(text("a"))).padding(10.0),
        container(text("b")).width(100.0).height(40.0),
    ],
);
