//! Native slider capture; source identity is independent of input callbacks.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Length, Pixels,
    widget::{
        Slider, VerticalSlider,
        slider::{Catalog, Status, Style, StyleFn},
    },
};
use std::ops::RangeInclusive;

/// Preserve native range/value and store the borrowed change callback.
///
/// # Errors
/// Rejects source byte bounds or allocation failure before native construction.
pub fn slider<'a, T, Message, Theme>(
    range: RangeInclusive<T>,
    value: T,
    on_change: impl Fn(T) -> Message + 'a,
    limits: CaptureLimits,
) -> Result<Captured<Slider<'a, T, Message, Theme>>, CaptureError>
where
    T: Copy + From<u8> + PartialOrd,
    Message: Clone,
    Theme: Catalog,
{
    let construction = Construction::slider(limits)?;
    Ok(Captured {
        payload: Slider::new(range, value, on_change),
        construction,
    })
}
impl<'a, T, Message, Theme> Captured<Slider<'a, T, Message, Theme>>
where
    T: Copy + From<u8> + PartialOrd,
    Message: Clone,
    Theme: Catalog,
{
    #[must_use]
    pub fn default(mut self, value: impl Into<T>) -> Self {
        self.payload = self.payload.default(value);
        self
    }
    #[must_use]
    pub fn on_release(mut self, value: Message) -> Self {
        self.payload = self.payload.on_release(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn step(mut self, value: impl Into<T>) -> Self {
        self.payload = self.payload.step(value);
        self
    }
    #[must_use]
    pub fn shift_step(mut self, value: impl Into<T>) -> Self {
        self.payload = self.payload.shift_step(value);
        self
    }
    /// Store the native style callback without invoking it during capture.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native class independently of CSS attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}

/// Preserve native range/value and store the borrowed change callback.
///
/// # Errors
/// Rejects source byte bounds or allocation failure before native construction.
pub fn vertical_slider<'a, T, Message, Theme>(
    range: RangeInclusive<T>,
    value: T,
    on_change: impl Fn(T) -> Message + 'a,
    limits: CaptureLimits,
) -> Result<Captured<VerticalSlider<'a, T, Message, Theme>>, CaptureError>
where
    T: Copy + From<u8> + PartialOrd,
    Message: Clone,
    Theme: Catalog,
{
    let construction = Construction::vertical_slider(limits)?;
    Ok(Captured {
        payload: VerticalSlider::new(range, value, on_change),
        construction,
    })
}
impl<'a, T, Message, Theme> Captured<VerticalSlider<'a, T, Message, Theme>>
where
    T: Copy + From<u8> + PartialOrd,
    Message: Clone,
    Theme: Catalog,
{
    #[must_use]
    pub fn default(mut self, value: impl Into<T>) -> Self {
        self.payload = self.payload.default(value);
        self
    }
    #[must_use]
    pub fn on_release(mut self, value: Message) -> Self {
        self.payload = self.payload.on_release(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.height(value);
        self
    }
    /// Preserve the native exact-T step argument.
    ///
    /// ```compile_fail,E0308
    /// # use iced_css::capture::{adapters, CaptureLimits};
    /// struct Converted;
    /// impl From<Converted> for f32 { fn from(_: Converted)->f32 { 2.0 } }
    /// fn wrong(limits:CaptureLimits) {
    ///     let _ = adapters::vertical_slider::<f32,_,iced::Theme>(0.0..=10.0,1.0,|v|v,limits).unwrap().step(Converted);
    /// }
    /// ```
    #[must_use]
    pub fn step(mut self, value: T) -> Self {
        self.payload = self.payload.step(value);
        self
    }
    #[must_use]
    pub fn shift_step(mut self, value: impl Into<T>) -> Self {
        self.payload = self.payload.shift_step(value);
        self
    }
    /// Store the native style callback without invoking it during capture.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native class independently of CSS attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
