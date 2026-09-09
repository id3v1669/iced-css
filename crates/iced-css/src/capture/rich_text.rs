//! Typed span storage capture; arbitrary AsRef providers require a separate contract.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Color, Length, Pixels,
    advanced::text::Renderer,
    alignment,
    widget::text::{Alignment, Catalog, LineHeight, Rich, Span, Style, StyleFn, Wrapping},
};
pub type CapturedRichText<'a, Link, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<Rich<'a, Link, Message, Theme, R>>;

pub fn rich_text_new<'a, Link, Message, Theme, R>(
    limits: CaptureLimits,
) -> Result<CapturedRichText<'a, Link, Message, Theme, R>, CaptureError>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
{
    let construction = Construction::rich_text::<Link, R::Font>(&[], limits)?;
    Ok(Captured {
        payload: Rich::new(),
        construction,
    })
}
/// Capture known Vec span data without cloning links or invoking an AsRef provider.
pub fn rich_text_vec<'a, Link, Message, Theme, R>(
    spans: Vec<Span<'a, Link, R::Font>>,
    limits: CaptureLimits,
) -> Result<CapturedRichText<'a, Link, Message, Theme, R>, CaptureError>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
{
    let construction = Construction::rich_text(&spans, limits)?;
    Ok(Captured {
        payload: Rich::with_spans(spans),
        construction,
    })
}
/// Retain the borrowed span slice in native Rich while independently owning text.
pub fn rich_text_slice<'a, Link, Message, Theme, R>(
    spans: &'a [Span<'a, Link, R::Font>],
    limits: CaptureLimits,
) -> Result<CapturedRichText<'a, Link, Message, Theme, R>, CaptureError>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
{
    let construction = Construction::rich_text(spans, limits)?;
    Ok(Captured {
        payload: Rich::with_spans(spans),
        construction,
    })
}
impl<'a, Link, Message, Theme, R> Captured<Rich<'a, Link, Message, Theme, R>>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
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
    pub fn font(mut self, value: impl Into<R::Font>) -> Self {
        self.payload = self.payload.font(value);
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
    pub fn center(mut self) -> Self {
        self.payload = self.payload.center();
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
    pub fn wrapping(mut self, value: Wrapping) -> Self {
        self.payload = self.payload.wrapping(value);
        self
    }
    #[must_use]
    pub fn on_link_click(mut self, value: impl Fn(Link) -> Message + 'a) -> Self {
        self.payload = self.payload.on_link_click(value);
        self
    }
    #[must_use]
    pub fn style(mut self, value: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(value);
        self
    }
    #[must_use]
    pub fn color(mut self, value: impl Into<Color>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.color(value);
        self
    }
    #[must_use]
    pub fn color_maybe(mut self, value: Option<impl Into<Color>>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.color_maybe(value);
        self
    }
    #[must_use]
    pub fn class(mut self, value: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(value);
        self
    }
}
