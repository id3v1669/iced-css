//! Native Float capture; CSS float formatting is a separate subsystem.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{
    Element, Rectangle, Vector,
    widget::float::{Catalog, Float, Style, StyleFn},
};
/// Preserve a distinct source node despite native state forwarding.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::float(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn float<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Float<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: Catalog,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.float(limits)?;
    Ok(Captured {
        payload: Float::new(payload),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Float<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
{
    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.payload = self.payload.scale(scale);
        self
    }
    /// Store the borrowed native translation callback without querying geometry.
    #[must_use]
    pub fn translate(mut self, translate: impl Fn(Rectangle, Rectangle) -> Vector + 'a) -> Self {
        self.payload = self.payload.translate(translate);
        self
    }
    /// Store the native base/overlay style callback without evaluating it.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native class without modifying CSS attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
