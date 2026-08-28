#![allow(dead_code)]

#[allow(unused_imports)]
pub mod prelude {
    pub use super::{El, Msg, Sim, assert_equivalent, mount};
    pub use iced::widget::{button, container, text};
    pub use iced_css::StyleClasses;
}

#[derive(Debug, Clone)]
pub enum Msg {
    Css(iced_css::Event),
    Noop,
}

impl From<iced_css::Event> for Msg {
    fn from(event: iced_css::Event) -> Self {
        Msg::Css(event)
    }
}

pub type El = iced::Element<'static, Msg>;

pub struct Sim {
    _private: (),
}

pub fn mount(view: fn() -> El) -> Sim {
    let _ = view;
    unimplemented!("iced-css harness: mount not implemented yet")
}

pub fn assert_equivalent(actual: Sim, expected: Sim) {
    let _ = (actual, expected);
    unimplemented!("iced-css harness: assert_equivalent not implemented yet")
}

#[macro_export]
macro_rules! css_test {
    ($name:ident, $css:literal, $input:expr, $expected:expr $(,)?) => {
        #[test]
        fn $name() {
            #[iced_css::style(inline = $css, policy = Compile)]
            fn input() -> El {
                iced::Element::from($input)
            }
            fn expected() -> El {
                iced::Element::from($expected)
            }
            assert_equivalent(mount(input), mount(expected));
        }
    };
}

#[macro_export]
macro_rules! css_todo_test {
    ($name:ident, $why:literal) => {
        #[test]
        fn $name() {
            // TODO, figure out later
            panic!("TODO, figure out later: {}", $why);
        }
    };
}
