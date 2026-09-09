//! Stock native svg capture with one resource source leaf.
use super::{CaptureError, CaptureLimits, Captured, Construction};
/// Capture a resource handle without decoding it or determining intrinsic size.
///
/// # Errors
/// Rejects evidence byte bounds or a fallible evidence allocation failure.
pub fn svg<'a, Theme: iced::widget::svg::Catalog>(
    handle: impl Into<iced::widget::svg::Handle>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Svg<'a, Theme>>, CaptureError> {
    let construction = Construction::svg(handle.into(), limits)?;
    let handle = construction.nodes()[construction.root()]
        .svg_source()
        .expect("resource leaf")
        .handle()
        .clone();
    Ok(Captured {
        payload: iced::widget::Svg::new(handle),
        construction,
    })
}
/// Capture the stock SVG path constructor without reading the path.
///
/// # Errors
/// Rejects evidence byte bounds or a fallible evidence allocation failure.
pub fn svg_from_path<'a, Theme: iced::widget::svg::Catalog>(
    path: impl Into<std::path::PathBuf>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Svg<'a, Theme>>, CaptureError> {
    svg(iced::widget::svg::Handle::from_path(path), limits)
}
impl<'a, Theme: iced::widget::svg::Catalog> Captured<iced::widget::Svg<'a, Theme>> {
    #[must_use]
    pub fn width(mut self, value: impl Into<iced::Length>) -> Self {
        let value = value.into();
        self.construction.svg_hints_mut().width = value;
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<iced::Length>) -> Self {
        let value = value.into();
        self.construction.svg_hints_mut().height = value;
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn content_fit(mut self, value: iced::ContentFit) -> Self {
        self.construction.svg_hints_mut().content_fit = value;
        self.payload = self.payload.content_fit(value);
        self
    }
    #[must_use]
    pub fn rotation(mut self, value: impl Into<iced::Rotation>) -> Self {
        let value = value.into();
        self.construction.svg_hints_mut().rotation = value;
        self.payload = self.payload.rotation(value);
        self
    }
    #[must_use]
    pub fn opacity(mut self, value: impl Into<f32>) -> Self {
        let value = value.into();
        self.construction.svg_hints_mut().opacity = value;
        self.payload = self.payload.opacity(value);
        self
    }
    /// Store the callback only in the native widget without evaluating it.
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, iced::widget::svg::Status) -> iced::widget::svg::Style + 'a,
    ) -> Self
    where
        Theme::Class<'a>: From<iced::widget::svg::StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self.construction.svg_hints_mut().style_selection = super::SvgStyleSelection::Style;
        self
    }
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self.construction.svg_hints_mut().style_selection = super::SvgStyleSelection::Class;
        self
    }
}
