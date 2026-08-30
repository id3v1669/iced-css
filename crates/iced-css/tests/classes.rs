//! Classes handling: matching by class, multiple classes, cascade order.
//! (Unknown-class rejection is unit-tested in iced-css-macro.)

#[macro_use]
mod common;
use common::prelude::*;

css_test!(
    class_single,
    ".single { width: 10px; height: 20px; }",
    container(text("t")).style_classes(["single"]),
    container(text("t")).width(10.0).height(20.0),
);

// later wins
css_test!(
    class_cascade_sheet_order,
    ".wide { width: 10px; } .base { width: 20px; height: 30px; }",
    container(text("t")).style_classes(["wide", "base"]),
    container(text("t")).width(20.0).height(30.0),
);

css_test!(
    class_multiple_merge,
    ".w { width: 10px; } .h { height: 20px; }",
    container(text("t")).style_classes(["w", "h"]),
    container(text("t")).width(10.0).height(20.0),
);

// classless is not an error
css_test!(
    no_classes_untouched,
    ".single { width: 10px; height: 20px; }",
    iced::widget::column![
        container(text("styled")).style_classes(["single"]),
        container(text("plain")),
    ],
    iced::widget::column![
        container(text("styled")).width(10.0).height(20.0),
        container(text("plain")),
    ],
);

// multi-resolve
css_test!(
    classes_per_widget,
    ".single { width: 10px; height: 20px; } .w { width: 30px; } .h { height: 40px; }",
    iced::widget::column![
        container(text("a")).style_classes(["single"]),
        container(text("b")).style_classes(["w", "h"]),
    ],
    iced::widget::column![
        container(text("a")).width(10.0).height(20.0),
        container(text("b")).width(30.0).height(40.0),
    ],
);
