//! Captured native scrolling, preserving constructor and conditional builder behavior.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{
    Element, Length, Pixels,
    widget::scrollable::{
        Anchor, Catalog, Direction, Scrollable, Status, Style, StyleFn, Viewport,
    },
};
/// Capture the sole source child before stock vertical Scrollable construction.
///
/// # Errors
/// Rejects aggregate source evidence bounds or allocation failure.
pub fn scrollable<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Scrollable<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    scrollable_with_direction(child, Direction::default(), limits)
}
/// Keep direction and child conversion inside the native constructor, in its order.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::scrollable(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native conversions.
pub fn scrollable_with_direction<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    direction: impl Into<Direction>,
    limits: CaptureLimits,
) -> Result<Captured<Scrollable<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.scrollable(limits)?;
    Ok(Captured {
        payload: Scrollable::with_direction(payload, direction),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Scrollable<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    #[must_use]
    pub fn direction(mut self, value: impl Into<Direction>) -> Self {
        self.payload = self.payload.direction(value);
        self
    }
    #[must_use]
    pub fn id(mut self, value: impl Into<iced::advanced::widget::Id>) -> Self {
        self.payload = self.payload.id(value);
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
    pub fn anchor_x(mut self, value: Anchor) -> Self {
        self.payload = self.payload.anchor_x(value);
        self
    }
    #[must_use]
    pub fn anchor_y(mut self, value: Anchor) -> Self {
        self.payload = self.payload.anchor_y(value);
        self
    }
    /// Stock Both-axis spacing deliberately omits the Into conversion.
    #[must_use]
    pub fn spacing(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.spacing(value);
        self
    }
    #[must_use]
    pub fn auto_scroll(mut self, value: bool) -> Self {
        self.payload = self.payload.auto_scroll(value);
        self
    }
    #[must_use]
    pub fn horizontal(mut self) -> Self {
        self.payload = self.payload.horizontal();
        self
    }
    #[must_use]
    pub fn anchor_top(mut self) -> Self {
        self.payload = self.payload.anchor_top();
        self
    }
    #[must_use]
    pub fn anchor_bottom(mut self) -> Self {
        self.payload = self.payload.anchor_bottom();
        self
    }
    #[must_use]
    pub fn anchor_left(mut self) -> Self {
        self.payload = self.payload.anchor_left();
        self
    }
    #[must_use]
    pub fn anchor_right(mut self) -> Self {
        self.payload = self.payload.anchor_right();
        self
    }
    /// Store the callback until a native scrolling event produces a viewport.
    #[must_use]
    pub fn on_scroll(mut self, callback: impl Fn(Viewport) -> Message + 'a) -> Self {
        self.payload = self.payload.on_scroll(callback);
        self
    }
    /// Store the native style callback without inspecting its result.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native theme class without changing CSS attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
