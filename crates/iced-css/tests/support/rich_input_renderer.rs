//! Controlled span rectangles for input tests; no glyph or font qualification.
use iced_test::core::{
    self, Background, Color, Font, Pixels, Point, Rectangle, Size, Transformation, alignment, text,
};
#[derive(Default)]
pub struct FixedRenderer;
impl core::Renderer for FixedRenderer {
    fn start_layer(&mut self, _: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _: Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, _: core::renderer::Quad, _: impl Into<Background>) {}
    fn reset(&mut self, _: Rectangle) {}
    fn allocate_image(
        &mut self,
        handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        core::Renderer::allocate_image(&mut (), handle, callback);
    }
}
impl text::Renderer for FixedRenderer {
    type Font = Font;
    type Paragraph = FixedParagraph;
    type Editor = ();
    const ICON_FONT: Font = Font::DEFAULT;
    const CHECKMARK_ICON: char = '0';
    const ARROW_DOWN_ICON: char = '0';
    const SCROLL_UP_ICON: char = '0';
    const SCROLL_DOWN_ICON: char = '0';
    const SCROLL_LEFT_ICON: char = '0';
    const SCROLL_RIGHT_ICON: char = '0';
    const ICED_LOGO: char = '0';
    fn default_font(&self) -> Font {
        Font::DEFAULT
    }
    fn default_size(&self) -> Pixels {
        Pixels(16.0)
    }
    fn fill_paragraph(&mut self, _: &FixedParagraph, _: Point, _: Color, _: Rectangle) {}
    fn fill_editor(&mut self, _: &(), _: Point, _: Color, _: Rectangle) {}
    fn fill_text(&mut self, _: text::Text<String, Font>, _: Point, _: Color, _: Rectangle) {}
}
/// Each supplied span occupies a declared 20x20 fixture cell. Native Rich owns
/// hover/press state; this fixture supplies only deterministic input geometry.
#[derive(Default)]
pub struct FixedParagraph {
    bounds: Size,
    spans: Vec<Rectangle>,
}
impl text::Paragraph for FixedParagraph {
    type Font = Font;
    fn with_text(text: text::Text<&str, Font>) -> Self {
        Self {
            bounds: text.bounds,
            spans: Vec::new(),
        }
    }
    fn with_spans<Link>(text: text::Text<&[text::Span<'_, Link, Font>], Font>) -> Self {
        Self {
            bounds: text.bounds,
            spans: (0..text.content.len())
                .map(|i| Rectangle::new(Point::new(i as f32 * 20.0, 0.0), Size::new(20.0, 20.0)))
                .collect(),
        }
    }
    fn resize(&mut self, bounds: Size) {
        self.bounds = bounds;
    }
    fn compare(&self, _: text::Text<(), Font>) -> text::Difference {
        text::Difference::None
    }
    fn size(&self) -> Pixels {
        Pixels(16.0)
    }
    fn font(&self) -> Font {
        Font::DEFAULT
    }
    fn line_height(&self) -> text::LineHeight {
        text::LineHeight::default()
    }
    fn align_x(&self) -> text::Alignment {
        text::Alignment::Default
    }
    fn align_y(&self) -> alignment::Vertical {
        alignment::Vertical::Top
    }
    fn wrapping(&self) -> text::Wrapping {
        text::Wrapping::default()
    }
    fn shaping(&self) -> text::Shaping {
        text::Shaping::Advanced
    }
    fn bounds(&self) -> Size {
        self.bounds
    }
    fn min_bounds(&self) -> Size {
        Size::new(self.spans.len() as f32 * 20.0, 20.0)
    }
    fn hit_test(&self, _: Point) -> Option<text::Hit> {
        None
    }
    fn hit_span(&self, point: Point) -> Option<usize> {
        self.spans.iter().position(|span| span.contains(point))
    }
    fn span_bounds(&self, index: usize) -> Vec<Rectangle> {
        self.spans.get(index).copied().into_iter().collect()
    }
    fn grapheme_position(&self, _: usize, _: usize) -> Option<Point> {
        None
    }
}
