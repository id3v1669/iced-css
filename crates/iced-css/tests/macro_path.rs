mod common;
use common::prelude::*;

#[test]
fn path_stylesheet_is_read_from_the_manifest_directory() {
    #[iced_css::style("tests/fixtures/macro-path/sized.css", policy = Compile)]
    fn input() -> El {
        container(text("t")).style_classes(["s"]).into()
    }
    fn expected() -> El {
        container(text("t")).width(10.0).height(20.0).into()
    }

    assert_equivalent(&mut mount(input), &mut mount(expected));
}
