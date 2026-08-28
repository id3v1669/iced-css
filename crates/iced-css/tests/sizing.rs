#[macro_use]
mod common;
use common::prelude::*;

css_test!(
    width_px,
    ".s { width: 10px; height: 20px; }",
    container(text("t")).style_classes(["s"]),
    container(text("t")).width(10.0).height(20.0),
);

css_test!(
    width_auto_fills_parent,
    ".auto { width: auto; height: 40px; } .sized { width: 100px; height: 40px; }",
    container(container(text("in")).style_classes(["sized"])).style_classes(["auto"]),
    container(container(text("in")).width(100.0).height(40.0))
        .width(iced::Length::Fill)
        .height(40.0),
);

// width: x% unimplemented.rs::percent_of_dynamic_parent
// min-width unimplemented.rs::min_width_constraint_widget
// min-width vs max-width interaction unimplemented.rs::min_width_constraint_widget

