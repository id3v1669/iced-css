//! Theme-changing native transport with independently owned source evidence.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{Background, Color, Element, widget::Themer};
/// Capture an inner-theme child; native erasure may use a distinct outer theme.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::themer(Some(iced::Theme::Dark), child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn themer<'a, Message, Theme, Renderer, W>(
    theme: Option<Theme>,
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Themer<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.themer(limits)?;
    Ok(Captured {
        payload: Themer::new(theme, payload),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Themer<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    /// Preserve the native function-pointer contract without eager evaluation.
    ///
    /// ```compile_fail,E0308
    /// # use iced_css::capture::Captured;
    /// fn capture(parent: Captured<iced::widget::Themer<'_, (), iced::Theme, ()>>, color: iced::Color) {
    ///     let _ = parent.text_color(|_| color);
    /// }
    /// ```
    #[must_use]
    pub fn text_color(mut self, callback: fn(&Theme) -> Color) -> Self {
        self.payload = self.payload.text_color(callback);
        self
    }
    /// Store the native background function pointer for drawing.
    #[must_use]
    pub fn background(mut self, callback: fn(&Theme) -> Background) -> Self {
        self.payload = self.payload.background(callback);
        self
    }
}
