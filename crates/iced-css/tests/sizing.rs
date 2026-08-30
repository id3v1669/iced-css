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

css_test!(
    min_width_beats_max_width,
    ".s { min-width: 300px; max-width: 100px; height: 40px; }",
    container(text("t")).style_classes(["s"]),
    container(text("t")).width(300.0).height(40.0),
);

#[test]
fn max_width_under_live_resize() {
    #[iced_css::style(
        inline = ".outer { width: auto; } .inner { max-width: 400px; height: 40px; }",
        policy = Compile
    )]
    fn input() -> El {
        iced::Element::from(
            container(container(text("t")).style_classes(["inner"])).style_classes(["outer"]),
        )
    }
    fn inner_400() -> El {
        container(container(text("t")).width(400.0).height(40.0))
            .width(iced::Length::Fill)
            .into()
    }
    fn inner_300() -> El {
        container(container(text("t")).width(300.0).height(40.0))
            .width(iced::Length::Fill)
            .into()
    }

    let mut actual = mount_at(input, 500.0, 600.0);
    assert_equivalent(&mut actual, &mut mount_at(inner_400, 500.0, 600.0));

    resize(&mut actual, 300.0, 600.0);
    assert_equivalent(&mut actual, &mut mount_at(inner_300, 300.0, 600.0));
}

#[test]
fn min_width_under_live_resize() {
    #[iced_css::style(
        inline = ".outer { width: auto; } .inner { min-width: 400px; height: 40px; }",
        policy = Compile
    )]
    fn input() -> El {
        iced::Element::from(
            container(container(text("t")).style_classes(["inner"])).style_classes(["outer"]),
        )
    }
    fn inner_500() -> El {
        container(container(text("t")).width(500.0).height(40.0))
            .width(iced::Length::Fill)
            .into()
    }

    let mut actual = mount_at(input, 500.0, 600.0);
    assert_equivalent(&mut actual, &mut mount_at(inner_500, 500.0, 600.0));

    // overflow beyond the 300px outer: asserted directly (inexpressible as a
    // plain iced expected tree — iced clamps Fixed sizes to parent limits)
    resize(&mut actual, 300.0, 600.0);
    assert_path(&mut actual, &[
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 40.0 }),
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 40.0 }),
    ]);
}

#[test]
fn percent_under_live_resize() {
    #[iced_css::style(
        inline = ".outer { width: auto; } .inner { width: 50%; height: 40px; }",
        policy = Compile
    )]
    fn input() -> El {
        iced::Element::from(
            container(container(text("t")).style_classes(["inner"])).style_classes(["outer"]),
        )
    }
    fn inner_400() -> El {
        container(container(text("t")).width(400.0).height(40.0))
            .width(iced::Length::Fill)
            .into()
    }
    fn inner_200() -> El {
        container(container(text("t")).width(200.0).height(40.0))
            .width(iced::Length::Fill)
            .into()
    }

    let mut actual = mount_at(input, 800.0, 600.0);
    assert_equivalent(&mut actual, &mut mount_at(inner_400, 800.0, 600.0));

    resize(&mut actual, 400.0, 600.0);
    assert_equivalent(&mut actual, &mut mount_at(inner_200, 400.0, 600.0));
}

css_test!(
    max_width_fills_until_cap,
    ".s { max-width: 120px; height: 40px; } .w { width: 800px; }",
    container(container(text("t")).style_classes(["s"])).style_classes(["w"]),
    container(
        container(text("t"))
            .width(iced::Length::Fill)
            .max_width(120.0)
            .height(40.0)
    )
    .width(800.0),
);

css_test!(
    max_width_fill_in_narrow_parent,
    ".s { max-width: 120px; height: 40px; } .w { width: 100px; }",
    container(container(text("t")).style_classes(["s"])).style_classes(["w"]),
    container(
        container(text("t"))
            .width(iced::Length::Fill)
            .max_width(120.0)
            .height(40.0)
    )
    .width(100.0),
);

css_test!(
    min_width_fill_when_parent_larger,
    ".s { min-width: 200px; height: 40px; } .w { width: 800px; }",
    container(container(text("t")).style_classes(["s"])).style_classes(["w"]),
    container(container(text("t")).width(iced::Length::Fill).height(40.0)).width(800.0),
);

#[test]
fn min_width_overflows_narrow_parent() {
    #[iced_css::style(
        inline = ".s { min-width: 200px; height: 40px; } .w { width: 150px; }",
        policy = Compile
    )]
    fn input() -> El {
        iced::Element::from(
            container(container(text("t")).style_classes(["s"])).style_classes(["w"]),
        )
    }

    let mut actual = mount(input);
    assert_path(&mut actual, &[
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 150.0, height: 40.0 }),
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 40.0 }),
    ]);
}


css_test!(
    min_height_stretches_above_content,
    ".s { min-height: 80px; } .content { width: 50px; height: 20px; }",
    container(container(text("t")).style_classes(["content"])).style_classes(["s"]),
    container(container(text("t")).width(50.0).height(20.0))
        .width(iced::Length::Fill)
        .height(80.0),
);

css_test!(
    min_height_dormant_below_content,
    ".s { min-height: 80px; } .content { width: 50px; height: 120px; }",
    container(container(text("t")).style_classes(["content"])).style_classes(["s"]),
    container(container(text("t")).width(50.0).height(120.0)).width(iced::Length::Fill),
);

#[test]
fn max_height_caps_content_height() {
    #[iced_css::style(
        inline = ".s { width: 50px; max-height: 10px; } .content { width: 50px; height: 20px; }",
        policy = Compile
    )]
    fn input() -> El {
        iced::Element::from(
            container(container(text("t")).style_classes(["content"])).style_classes(["s"]),
        )
    }

    let mut actual = mount(input);
    assert_path(&mut actual, &[
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 10.0 }),
        ("container", iced::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 20.0 }),
    ]);
}

// % height against parent height
css_test!(
    height_percent_of_definite_parent,
    ".p { width: 50px; height: 200px; } .s { height: 25%; }",
    container(container(text("t")).style_classes(["s"])).style_classes(["p"]),
    container(container(text("t")).width(iced::Length::Fill).height(50.0))
        .width(50.0)
        .height(200.0),
);

// max height wins height
css_test!(
    max_height_clamps,
    ".s { width: 50px; height: 500px; max-height: 120px; }",
    container(text("t")).style_classes(["s"]),
    container(text("t")).width(50.0).height(500.0).max_height(120.0),
);

// min height wins height
css_test!(
    min_height_raises_explicit_height,
    ".s { width: 50px; height: 20px; min-height: 80px; }",
    container(text("t")).style_classes(["s"]),
    container(text("t")).width(50.0).height(80.0),
);

// min height wins max height
css_test!(
    min_height_beats_max_height,
    ".s { width: 50px; height: 20px; min-height: 80px; max-height: 30px; }",
    container(text("t")).style_classes(["s"]),
    container(text("t")).width(50.0).height(80.0),
);

