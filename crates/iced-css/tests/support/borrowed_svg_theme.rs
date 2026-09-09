//! Borrowed SVG class/style fixture.
use iced::widget::svg::{Catalog, Status, Style, StyleFn};
use std::cell::Cell;
pub struct ProbeTheme;
// Deliberately neither Clone nor Debug.
pub enum ProbeClass<'a> {
    Default,
    Callback(StyleFn<'a, ProbeTheme>),
    Borrowed {
        color: &'a iced::Color,
        calls: &'a Cell<usize>,
    },
}
thread_local! {pub static DEFAULT_CALLS:Cell<usize>=const{Cell::new(0)};}
impl Catalog for ProbeTheme {
    type Class<'a> = ProbeClass<'a>;
    fn default<'a>() -> Self::Class<'a> {
        DEFAULT_CALLS.with(|v| v.set(v.get() + 1));
        ProbeClass::Default
    }
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        match class {
            ProbeClass::Default => Style::default(),
            ProbeClass::Callback(f) => f(self, status),
            ProbeClass::Borrowed { color, calls } => {
                calls.set(calls.get() + 1);
                Style {
                    color: Some(**color),
                }
            }
        }
    }
}
impl<'a> From<StyleFn<'a, ProbeTheme>> for ProbeClass<'a> {
    fn from(value: StyleFn<'a, ProbeTheme>) -> Self {
        Self::Callback(value)
    }
}
pub struct AuthoredClass<'a> {
    pub conversions: &'a Cell<usize>,
    pub color: &'a iced::Color,
    pub calls: &'a Cell<usize>,
}
impl<'a> From<AuthoredClass<'a>> for ProbeClass<'a> {
    fn from(value: AuthoredClass<'a>) -> Self {
        value.conversions.set(value.conversions.get() + 1);
        Self::Borrowed {
            color: value.color,
            calls: value.calls,
        }
    }
}
