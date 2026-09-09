//! Horizontal and vertical Rule capture share one canonical leaf source type.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Pixels,
    widget::rule::{Catalog, Rule, Style, StyleFn},
};
/// Capture a horizontal native rule after bounded source admission.
///
/// # Errors
/// Rejects source byte bounds/allocation before thickness conversion.
pub fn horizontal_rule<'a, Theme: Catalog>(
    thickness: impl Into<Pixels>,
    limits: CaptureLimits,
) -> Result<Captured<Rule<'a, Theme>>, CaptureError> {
    let construction = Construction::rule(limits)?;
    Ok(Captured {
        payload: iced::widget::rule::horizontal(thickness),
        construction,
    })
}
/// Capture a vertical native rule after bounded source admission.
///
/// # Errors
/// Rejects source byte bounds/allocation before thickness conversion.
pub fn vertical_rule<'a, Theme: Catalog>(
    thickness: impl Into<Pixels>,
    limits: CaptureLimits,
) -> Result<Captured<Rule<'a, Theme>>, CaptureError> {
    let construction = Construction::rule(limits)?;
    Ok(Captured {
        payload: iced::widget::rule::vertical(thickness),
        construction,
    })
}
impl<'a, Theme: Catalog> Captured<Rule<'a, Theme>> {
    /// Store the native drawing callback without evaluating it during capture.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    /// Set the native class without changing CSS class attributes.
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
