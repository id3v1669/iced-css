//! Captured native mouse event routing without eager callback evaluation.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{Element, Point, mouse, widget::MouseArea};
/// Capture one Content source role before native MouseArea construction.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::mouse_area(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn mouse_area<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<MouseArea<'a, Message, Theme, Renderer>>, CaptureError>
where
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.mouse_area(limits)?;
    Ok(Captured {
        payload: MouseArea::new(payload),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<MouseArea<'a, Message, Theme, Renderer>> {
    #[must_use]
    pub fn on_press(mut self, message: Message) -> Self {
        self.payload = self.payload.on_press(message);
        self
    }
    #[must_use]
    pub fn on_release(mut self, message: Message) -> Self {
        self.payload = self.payload.on_release(message);
        self
    }
    #[must_use]
    pub fn on_double_click(mut self, message: Message) -> Self {
        self.payload = self.payload.on_double_click(message);
        self
    }
    #[must_use]
    pub fn on_right_press(mut self, message: Message) -> Self {
        self.payload = self.payload.on_right_press(message);
        self
    }
    #[must_use]
    pub fn on_right_release(mut self, message: Message) -> Self {
        self.payload = self.payload.on_right_release(message);
        self
    }
    #[must_use]
    pub fn on_middle_press(mut self, message: Message) -> Self {
        self.payload = self.payload.on_middle_press(message);
        self
    }
    #[must_use]
    pub fn on_middle_release(mut self, message: Message) -> Self {
        self.payload = self.payload.on_middle_release(message);
        self
    }
    #[must_use]
    pub fn on_enter(mut self, message: Message) -> Self {
        self.payload = self.payload.on_enter(message);
        self
    }
    #[must_use]
    pub fn on_exit(mut self, message: Message) -> Self {
        self.payload = self.payload.on_exit(message);
        self
    }
    /// Store the movement callback; native events provide local coordinates.
    #[must_use]
    pub fn on_move(mut self, callback: impl Fn(Point) -> Message + 'a) -> Self {
        self.payload = self.payload.on_move(callback);
        self
    }
    /// Store the wheel callback without evaluating it during capture.
    #[must_use]
    pub fn on_scroll(mut self, callback: impl Fn(mouse::ScrollDelta) -> Message + 'a) -> Self {
        self.payload = self.payload.on_scroll(callback);
        self
    }
    #[must_use]
    pub fn interaction(mut self, interaction: mouse::Interaction) -> Self {
        self.payload = self.payload.interaction(interaction);
        self
    }
}
