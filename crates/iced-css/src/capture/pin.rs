//! Pin has a distinct source node despite forwarding its child's native state.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{Element, Length, Pixels, Point, widget::Pin};
/// Capture the source child independently of native Pin state forwarding.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::pin(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn pin<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Pin<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.pin(limits)?;
    Ok(Captured {
        payload: Pin::new(payload),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Pin<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
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
    pub fn position(mut self, value: impl Into<Point>) -> Self {
        self.payload = self.payload.position(value);
        self
    }
    #[must_use]
    pub fn x(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.x(value);
        self
    }
    #[must_use]
    pub fn y(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.y(value);
        self
    }
}
