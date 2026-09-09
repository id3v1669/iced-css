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

/// Known span storage whose slice access invokes no application provider code.
/// This marker is sealed; custom AsRef providers need their own phase contract.
///
/// A valid native provider is not automatically an admitted capture provider:
/// ```compile_fail,E0277
/// use iced::widget::text::Span;
/// use iced_css::capture::{adapters,CaptureLimits};
/// struct Custom(Vec<Span<'static>>);
/// impl AsRef<[Span<'static>]> for Custom {
///     fn as_ref(&self)->&[Span<'static>] { &self.0 }
/// }
/// fn capture(limits:CaptureLimits) {
///     let _=adapters::rich_text::<(),(),iced::Theme,()>(Custom(Vec::new()),limits);
/// }
/// ```
///
/// Application code cannot opt an unknown provider into this built-in marker:
/// ```compile_fail,E0277
/// use iced::widget::text::Span;
/// use iced_css::capture::SpanStorage;
/// struct Custom(Vec<Span<'static>>);
/// impl AsRef<[Span<'static>]> for Custom {
///     fn as_ref(&self)->&[Span<'static>] { &self.0 }
/// }
/// impl SpanStorage<'static,(),iced::Font> for Custom {}
/// ```
pub trait SpanStorage<'a, Link, Font>:
    storage_sealed::Sealed + AsRef<[Span<'a, Link, Font>]>
{
}
mod storage_sealed {
    use super::Span;
    pub trait Sealed {}
    impl<Link, Font> Sealed for Vec<Span<'_, Link, Font>> {}
    impl<Link, Font, const N: usize> Sealed for [Span<'_, Link, Font>; N] {}
    impl<Link, Font> Sealed for &[Span<'_, Link, Font>] {}
    impl<Link, Font> Sealed for &Vec<Span<'_, Link, Font>> {}
}
impl<'a, Link, Font> SpanStorage<'a, Link, Font> for Vec<Span<'a, Link, Font>> {}
impl<'a, Link, Font, const N: usize> SpanStorage<'a, Link, Font> for [Span<'a, Link, Font>; N] {}
impl<'a, Link, Font> SpanStorage<'a, Link, Font> for &[Span<'a, Link, Font>] {}
impl<'a, Link, Font> SpanStorage<'a, Link, Font> for &Vec<Span<'a, Link, Font>> {}

/// Capture known native span storage without changing its ownership representation.
/// Unknown AsRef providers are intentionally not inspected at construction time.
pub fn rich_text<'a, Link, Message, Theme, R>(
    spans: impl SpanStorage<'a, Link, R::Font> + 'a,
    limits: CaptureLimits,
) -> Result<CapturedRichText<'a, Link, Message, Theme, R>, CaptureError>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
{
    let construction = Construction::rich_text(spans.as_ref(), limits)?;
    Ok(Captured {
        payload: Rich::with_spans(spans),
        construction,
    })
}

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
    rich_text(spans, limits)
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
    rich_text(spans, limits)
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

/// Preserve native Rich::from_iter's original IntoIterator/Vec collection path.
/// Native iterator execution and Vec allocation occur before evidence admission
/// and are outside CaptureLimits. Only subsequent source scans/copy/storage are
/// bounded here; an evidence error follows collection and skips Rich construction.
pub fn rich_text_from_iter<'a, Link, Message, Theme, R>(
    spans: impl IntoIterator<Item = Span<'a, Link, R::Font>>,
    limits: CaptureLimits,
) -> Result<CapturedRichText<'a, Link, Message, Theme, R>, CaptureError>
where
    Link: Clone + 'static,
    Theme: Catalog,
    R: Renderer,
    R::Font: 'a,
{
    rich_text_vec(spans.into_iter().collect::<Vec<_>>(), limits)
}
