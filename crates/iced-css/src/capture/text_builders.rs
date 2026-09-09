//! Native Text configuration transport, preserving authored source text.
use super::Captured;
use iced::{
    Color, Length, Pixels, alignment,
    widget::text::{Alignment, Catalog, LineHeight, Shaping, Style, StyleFn, Text, Wrapping},
};
impl<'a, Theme, Renderer> Captured<Text<'a, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    #[must_use]
    pub fn size(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.size(value);
        self
    }
    #[must_use]
    pub fn line_height(mut self, value: impl Into<LineHeight>) -> Self {
        self.payload = self.payload.line_height(value);
        self
    }
    #[must_use]
    pub fn font(mut self, value: impl Into<Renderer::Font>) -> Self {
        self.payload = self.payload.font(value);
        self
    }
    #[must_use]
    pub fn font_maybe(mut self, value: Option<impl Into<Renderer::Font>>) -> Self {
        self.payload = self.payload.font_maybe(value);
        self
    }
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
    pub fn align_x(mut self, value: impl Into<Alignment>) -> Self {
        self.payload = self.payload.align_x(value);
        self
    }
    #[must_use]
    pub fn align_y(mut self, value: impl Into<alignment::Vertical>) -> Self {
        self.payload = self.payload.align_y(value);
        self
    }
    #[must_use]
    pub fn shaping(mut self, value: Shaping) -> Self {
        self.payload = self.payload.shaping(value);
        self
    }
    #[must_use]
    pub fn wrapping(mut self, value: Wrapping) -> Self {
        self.payload = self.payload.wrapping(value);
        self
    }
    #[must_use]
    pub fn center(mut self) -> Self {
        self.payload = self.payload.center();
        self
    }
    /// Store the native callback without evaluating it during capture.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    #[must_use]
    pub fn color(mut self, color: impl Into<Color>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.color(color);
        self
    }
    #[must_use]
    pub fn color_maybe(mut self, color: Option<impl Into<Color>>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.color_maybe(color);
        self
    }
    /// Set the native theme class; this does not edit CSS class attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
