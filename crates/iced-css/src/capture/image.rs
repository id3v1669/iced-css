//! Stock native image capture with one resource source leaf.
use super::{CaptureError, CaptureLimits, Captured, Construction};
/// Capture a resource handle without decoding it or determining intrinsic size.
///
/// # Errors
/// Rejects evidence byte bounds or a fallible evidence allocation failure.
pub fn image(
    handle: impl Into<iced::widget::image::Handle>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Image>, CaptureError> {
    let construction = Construction::image(handle.into(), limits)?;
    let handle = construction.nodes()[construction.root()]
        .image_source()
        .expect("resource leaf")
        .handle()
        .clone();
    Ok(Captured {
        payload: iced::widget::Image::new(handle),
        construction,
    })
}
impl Captured<iced::widget::Image> {
    #[must_use]
    pub fn width(mut self, value: impl Into<iced::Length>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().width = value;
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<iced::Length>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().height = value;
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn content_fit(mut self, value: iced::ContentFit) -> Self {
        self.construction.image_hints_mut().content_fit = value;
        self.payload = self.payload.content_fit(value);
        self
    }
    #[must_use]
    pub fn rotation(mut self, value: impl Into<iced::Rotation>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().rotation = value;
        self.payload = self.payload.rotation(value);
        self
    }
    #[must_use]
    pub fn opacity(mut self, value: impl Into<f32>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().opacity = value;
        self.payload = self.payload.opacity(value);
        self
    }
    #[must_use]
    pub fn expand(mut self, value: bool) -> Self {
        self.construction.image_hints_mut().expand = value;
        self.payload = self.payload.expand(value);
        self
    }
    #[must_use]
    pub fn filter_method(mut self, value: iced::widget::image::FilterMethod) -> Self {
        self.construction.image_hints_mut().filter_method = value;
        self.payload = self.payload.filter_method(value);
        self
    }
    #[must_use]
    pub fn scale(mut self, value: impl Into<f32>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().scale = value;
        self.payload = self.payload.scale(value);
        self
    }
    #[must_use]
    pub fn crop(mut self, value: iced::Rectangle<u32>) -> Self {
        self.construction.image_hints_mut().crop = Some(value);
        self.payload = self.payload.crop(value);
        self
    }
    #[must_use]
    pub fn border_radius(mut self, value: impl Into<iced::border::Radius>) -> Self {
        let value = value.into();
        self.construction.image_hints_mut().border_radius = value;
        self.payload = self.payload.border_radius(value);
        self
    }
}
