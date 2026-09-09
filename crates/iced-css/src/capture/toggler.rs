//! Native toggler configuration and authored source-label capture.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Length, Pixels,
    advanced::text,
    widget::{
        Toggler,
        toggler::{Catalog, Status, Style, StyleFn},
    },
};

pub fn toggler<'a, Message, Theme, Renderer>(
    checked: bool,
    limits: CaptureLimits,
) -> Result<Captured<Toggler<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    let construction = Construction::toggler(limits)?;
    Ok(Captured {
        payload: Toggler::new(checked),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<Toggler<'a, Message, Theme, Renderer>>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Convert once, then admit the source copy before changing the native label.
    /// Limits cover resulting evidence, excluding transient replacement overlap,
    /// native storage and user-provided conversion work.
    pub fn label(
        mut self,
        label: impl text::IntoFragment<'a>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let fragment = label.into_fragment();
        self.construction.set_control_label(&fragment, limits)?;
        self.payload = self.payload.label(fragment);
        Ok(self)
    }
    #[must_use]
    pub fn on_toggle(mut self, callback: impl Fn(bool) -> Message + 'a) -> Self {
        self.payload = self.payload.on_toggle(callback);
        self
    }
    #[must_use]
    pub fn on_toggle_maybe(mut self, callback: Option<impl Fn(bool) -> Message + 'a>) -> Self {
        self.payload = self.payload.on_toggle_maybe(callback);
        self
    }
    #[must_use]
    pub fn size(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.size(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn spacing(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.spacing(value);
        self
    }
    #[must_use]
    pub fn text_size(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.text_size(value);
        self
    }
    #[must_use]
    pub fn text_line_height(mut self, value: impl Into<text::LineHeight>) -> Self {
        self.payload = self.payload.text_line_height(value);
        self
    }
    #[must_use]
    pub fn text_alignment(mut self, value: impl Into<text::Alignment>) -> Self {
        self.payload = self.payload.text_alignment(value);
        self
    }
    #[must_use]
    pub fn text_shaping(mut self, value: text::Shaping) -> Self {
        self.payload = self.payload.text_shaping(value);
        self
    }
    #[must_use]
    pub fn text_wrapping(mut self, value: text::Wrapping) -> Self {
        self.payload = self.payload.text_wrapping(value);
        self
    }
    #[must_use]
    pub fn font(mut self, value: impl Into<Renderer::Font>) -> Self {
        self.payload = self.payload.font(value);
        self
    }
    /// Store the callback without invoking it during capture.
    #[must_use]
    pub fn style(mut self, value: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(value);
        self
    }
    /// Native class selection is independent of CSS source attributes.
    #[must_use]
    pub fn class(mut self, value: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(value);
        self
    }
}
