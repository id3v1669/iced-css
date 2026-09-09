//! Native Button configuration transport; press builders live in adapters.
use super::Captured;
use iced::{
    Length, Padding,
    widget::button::{Button, Catalog, Status, Style, StyleFn},
};
impl<'a, Message, Theme, Renderer> Captured<Button<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.payload = self.payload.padding(value);
        self
    }
    #[must_use]
    pub fn clip(mut self, value: bool) -> Self {
        self.payload = self.payload.clip(value);
        self
    }
    /// Store the native status-aware callback without evaluating it during capture.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native theme class; this does not edit CSS class attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
