//! Radio capture preserving native equality, factory and label-conversion order.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Length, Pixels,
    advanced::text,
    widget::{
        Radio,
        radio::{Catalog, Status, Style, StyleFn},
    },
};

struct CaptureLabel<'e, L> {
    value: L,
    limits: CaptureLimits,
    evidence: &'e mut Option<Result<Construction, CaptureError>>,
}
impl<L: Into<String>> From<CaptureLabel<'_, L>> for String {
    fn from(label: CaptureLabel<'_, L>) -> Self {
        let value = label.value.into();
        *label.evidence = Some(Construction::radio(&value, label.limits));
        value
    }
}

/// Let the native constructor compare selection and invoke the message factory
/// before converting the label. Evidence failure can therefore follow user side
/// effects; it does not promise to prevent native construction or allocations.
pub fn radio<'a, Message, Theme, Renderer, V>(
    label: impl Into<String>,
    value: V,
    selected: Option<V>,
    factory: impl FnOnce(V) -> Message,
    limits: CaptureLimits,
) -> Result<Captured<Radio<'a, Message, Theme, Renderer>>, CaptureError>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
    V: Copy + Eq,
{
    let mut evidence = None;
    let payload = Radio::new(
        CaptureLabel {
            value: label,
            limits,
            evidence: &mut evidence,
        },
        value,
        selected,
        factory,
    );
    let construction = evidence.ok_or(CaptureError::Role)??;
    Ok(Captured {
        payload,
        construction,
    })
}

impl<'a, Message, Theme, Renderer> Captured<Radio<'a, Message, Theme, Renderer>>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
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
    #[must_use]
    pub fn style(mut self, value: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(value);
        self
    }
    #[must_use]
    pub fn class(mut self, value: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(value);
        self
    }
}
