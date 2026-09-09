//! Native Container builder transport without changing captured source membership.
use super::{Captured, ContainerRecipe as Container};
use iced::{
    Length, Padding, Pixels, alignment,
    widget::container::{Catalog, Style, StyleFn},
};
impl<'a, Message, Theme, Renderer> Captured<Container<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Set the native widget ID; this does not edit the CSS source ID.
    #[must_use]
    pub fn id(mut self, value: impl Into<iced::advanced::widget::Id>) -> Self {
        self.payload = self.payload.id(value);
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.payload = self.payload.padding(value);
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
    pub fn center(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.center(value);
        self
    }
    #[must_use]
    pub fn center_x(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.center_x(value);
        self
    }
    #[must_use]
    pub fn center_y(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.center_y(value);
        self
    }
    #[must_use]
    pub fn align_left(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.align_left(value);
        self
    }
    #[must_use]
    pub fn align_right(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.align_right(value);
        self
    }
    #[must_use]
    pub fn align_top(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.align_top(value);
        self
    }
    #[must_use]
    pub fn align_bottom(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.align_bottom(value);
        self
    }
    #[must_use]
    pub fn max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.max_width(value);
        self
    }
    #[must_use]
    pub fn max_height(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.max_height(value);
        self
    }
    #[must_use]
    pub fn align_x(mut self, value: impl Into<alignment::Horizontal>) -> Self {
        self.payload = self.payload.align_x(value);
        self
    }
    #[must_use]
    pub fn align_y(mut self, value: impl Into<alignment::Vertical>) -> Self {
        self.payload = self.payload.align_y(value);
        self
    }
    #[must_use]
    pub fn clip(mut self, value: bool) -> Self {
        self.payload = self.payload.clip(value);
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
    /// Set the native theme class; this does not edit CSS class attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
