//! Captured Tooltip source ownership is independent of native overlay visibility.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{
    Element, Pixels,
    widget::{
        container::{Catalog, Style, StyleFn},
        tooltip::{Position, Tooltip},
    },
};
use std::time::Duration;
/// Retain both fixed source roles before native construction and conversion.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{CapturedElement, adapters, CaptureLimits};
/// fn bypass<'a>(anchor: CapturedElement<'a, (), iced::Theme, ()>,
///     tip: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::tooltip(anchor, tip, iced::widget::tooltip::Position::Bottom, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversions.
pub fn tooltip<'a, Message, Theme, Renderer, A, B>(
    content: Captured<A>,
    tooltip: Captured<B>,
    position: Position,
    limits: CaptureLimits,
) -> Result<Captured<Tooltip<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
    A: Into<Element<'a, Message, Theme, Renderer>>,
    B: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload: content,
        construction,
    } = content;
    let Captured {
        payload: tooltip,
        construction: tooltip_construction,
    } = tooltip;
    let construction = construction.tooltip(tooltip_construction, limits)?;
    Ok(Captured {
        payload: Tooltip::new(content, tooltip, position),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Tooltip<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    #[must_use]
    pub fn gap(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.gap(value);
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.padding(value);
        self
    }
    #[must_use]
    pub fn delay(mut self, value: Duration) -> Self {
        self.payload = self.payload.delay(value);
        self
    }
    #[must_use]
    pub fn snap_within_viewport(mut self, value: bool) -> Self {
        self.payload = self.payload.snap_within_viewport(value);
        self
    }
    /// Store the overlay's container-style callback without evaluating it.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native class without changing CSS attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
