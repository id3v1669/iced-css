#[macro_use]
mod common;
use common::prelude::*;

css_test!(
    padding_single_value,
    ".p { padding: 10px; }",
    button(text("b")).style_classes(["p"]),
    button(text("b")).padding(10.0).width(iced::Length::Fill),
);

css_test!(
    padding_two,
    // 10px vertical, 20px horizontal
    ".p { padding: 10px 20px; }",
    button(text("b")).style_classes(["p"]),
    button(text("b")).padding([10.0, 20.0]).width(iced::Length::Fill),
);

css_test!(
    padding_three,
    // top 10, horizontal 20, bottom 30
    ".p { padding: 10px 20px 30px; }",
    button(text("b")).style_classes(["p"]),
    button(text("b"))
        .padding(iced::Padding { top: 10.0, right: 20.0, bottom: 30.0, left: 20.0 })
        .width(iced::Length::Fill),
);

css_test!(
    padding_four,
    ".p { padding: 10px 20px 30px 40px; }",
    button(text("b")).style_classes(["p"]),
    button(text("b"))
        .padding(iced::Padding { top: 10.0, right: 20.0, bottom: 30.0, left: 40.0 })
        .width(iced::Length::Fill),
);

css_test!(
    padding_long,
    ".p { padding-top: 5px; padding-right: 15px; padding-bottom: 25px; padding-left: 35px; }",
    button(text("b")).style_classes(["p"]),
    button(text("b"))
        .padding(iced::Padding { top: 5.0, right: 15.0, bottom: 25.0, left: 35.0 })
        .width(iced::Length::Fill),
);

// width 100 with padding 10 is +20px box
css_test!(
    padding_with_width_is_content_box,
    ".p { width: 100px; height: 40px; padding: 10px; }",
    container(text("t")).style_classes(["p"]),
    container(text("t")).padding(10.0).width(120.0).height(60.0),
);

css_test!(
    padding_and_margin,
    ".p { padding: 10px; margin: 20px; }",
    button(text("b")).style_classes(["p"]),
    container(button(text("b")).padding(10.0).width(iced::Length::Fill)).padding(20.0),
);

#[test]
fn padding_is_in_the_hit_area_margin_is_not() {
    #[iced_css::style(inline = ".p { padding: 20px; width: 100px; }", policy = Compile)]
    fn padded() -> El {
        iced::Element::from(button(text("b")).on_press(Msg::Pressed).style_classes(["p"]))
    }
    #[iced_css::style(inline = ".m { margin: 20px; width: 100px; }", policy = Compile)]
    fn margined() -> El {
        iced::Element::from(button(text("b")).on_press(Msg::Pressed).style_classes(["m"]))
    }

    assert_eq!(click_at(&mut mount(padded), 5.0, 5.0), vec![Msg::Pressed]);
    assert_eq!(click_at(&mut mount(margined), 5.0, 5.0), vec![]);
}
