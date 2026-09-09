//! Native progress geometry capture with one leaf source node.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Length,
    widget::progress_bar::{Catalog, ProgressBar, Style, StyleFn},
};
use std::ops::RangeInclusive;
/// Preserve the supplied native range/value after bounded source admission.
///
/// # Errors
/// Rejects source byte bounds or allocation failure.
pub fn progress_bar<'a, Theme: Catalog>(
    range: RangeInclusive<f32>,
    value: f32,
    limits: CaptureLimits,
) -> Result<Captured<ProgressBar<'a, Theme>>, CaptureError> {
    let construction = Construction::progress_bar(limits)?;
    Ok(Captured {
        payload: ProgressBar::new(range, value),
        construction,
    })
}
impl<'a, Theme: Catalog> Captured<ProgressBar<'a, Theme>> {
    #[must_use]
    pub fn length(mut self, length: impl Into<Length>) -> Self {
        self.payload = self.payload.length(length);
        self
    }
    #[must_use]
    pub fn girth(mut self, girth: impl Into<Length>) -> Self {
        self.payload = self.payload.girth(girth);
        self
    }
    #[must_use]
    pub fn vertical(mut self) -> Self {
        self.payload = self.payload.vertical();
        self
    }
    /// Store the native drawing callback without evaluating it during capture.
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
