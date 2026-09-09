//! PickList capture keeps borrowed option data and native menu behavior intact.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Length, Padding, Pixels,
    advanced::text,
    widget::{
        PickList,
        overlay::menu,
        pick_list::{Catalog, Handle, Status, Style, StyleFn},
    },
};
use std::borrow::Borrow;

/// Captured native pick list, retaining its concrete option and selection storage.
pub type CapturedPickList<'a, T, L, V, Message, Theme = iced::Theme, Renderer = iced::Renderer> =
    Captured<PickList<'a, T, L, V, Message, Theme, Renderer>>;

/// Capture one source leaf without inspecting or stringifying option data.
pub fn pick_list<'a, T, L, V, Message, Theme, Renderer>(
    options: L,
    selected: Option<V>,
    on_select: impl Fn(T) -> Message + 'a,
    limits: CaptureLimits,
) -> Result<CapturedPickList<'a, T, L, V, Message, Theme, Renderer>, CaptureError>
where
    T: ToString + PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    let construction = Construction::pick_list(limits)?;
    Ok(Captured {
        payload: PickList::new(options, selected, on_select),
        construction,
    })
}

impl<'a, T, L, V, Message, Theme, Renderer>
    Captured<PickList<'a, T, L, V, Message, Theme, Renderer>>
where
    T: ToString + PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    #[must_use]
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.payload = self.payload.placeholder(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn menu_height(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.menu_height(value);
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.payload = self.payload.padding(value);
        self
    }
    #[must_use]
    pub fn text_size(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.text_size(value);
        self
    }
    #[must_use]
    pub fn text_line_height(mut self, value: impl Into<text::LineHeight>) -> Self {
        self.payload = self.payload.text_line_height(value);
        self
    }
    #[must_use]
    pub fn text_shaping(mut self, value: text::Shaping) -> Self {
        self.payload = self.payload.text_shaping(value);
        self
    }
    #[must_use]
    pub fn font(mut self, value: impl Into<Renderer::Font>) -> Self {
        self.payload = self.payload.font(value);
        self
    }
    #[must_use]
    pub fn handle(mut self, value: Handle<Renderer::Font>) -> Self {
        self.payload = self.payload.handle(value);
        self
    }
    #[must_use]
    pub fn on_open(mut self, value: Message) -> Self {
        self.payload = self.payload.on_open(value);
        self
    }
    #[must_use]
    pub fn on_close(mut self, value: Message) -> Self {
        self.payload = self.payload.on_close(value);
        self
    }
    #[must_use]
    pub fn style(mut self, value: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(value);
        self
    }
    #[must_use]
    pub fn menu_style(mut self, value: impl Fn(&Theme) -> menu::Style + 'a) -> Self
    where
        <Theme as menu::Catalog>::Class<'a>: From<menu::StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.menu_style(value);
        self
    }
    #[must_use]
    pub fn class(mut self, value: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.payload = self.payload.class(value);
        self
    }
    #[must_use]
    pub fn menu_class(mut self, value: impl Into<<Theme as menu::Catalog>::Class<'a>>) -> Self {
        self.payload = self.payload.menu_class(value);
        self
    }
}
