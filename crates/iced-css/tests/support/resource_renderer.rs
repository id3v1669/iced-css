//! Recording fixture: declared measurements and recorded native draw commands only.
//! No resource loading, decoding or intrinsic-size qualification.
use iced_test::core::{self, Background, Rectangle, Size, Transformation};
use std::cell::Cell;
#[derive(Debug, Clone, PartialEq)]
pub enum Draw {
    Image(core::image::Image, Rectangle, Rectangle),
    Svg(core::svg::Svg, Rectangle, Rectangle),
}
pub struct ResourceRenderer {
    pub image_size: Option<Size<u32>>,
    pub svg_size: Size<u32>,
    pub image_measures: Cell<usize>,
    pub svg_measures: Cell<usize>,
    pub commands: Vec<Draw>,
}
impl Default for ResourceRenderer {
    fn default() -> Self {
        Self {
            image_size: Some(Size::new(40, 20)),
            svg_size: Size::new(40, 20),
            image_measures: Cell::new(0),
            svg_measures: Cell::new(0),
            commands: Vec::new(),
        }
    }
}
impl core::Renderer for ResourceRenderer {
    fn start_layer(&mut self, _: Rectangle) {
        panic!("unexpected resource fixture layer")
    }
    fn end_layer(&mut self) {
        panic!("unexpected resource fixture layer end")
    }
    fn start_transformation(&mut self, _: Transformation) {
        panic!("unexpected resource fixture transform")
    }
    fn end_transformation(&mut self) {
        panic!("unexpected resource fixture transform end")
    }
    fn fill_quad(&mut self, _: core::renderer::Quad, _: impl Into<Background>) {
        panic!("unexpected resource fixture quad")
    }
    fn reset(&mut self, _: Rectangle) {
        self.commands.clear();
        self.image_measures.set(0);
        self.svg_measures.set(0);
    }
    fn allocate_image(
        &mut self,
        _: &core::image::Handle,
        _: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        panic!("capture/layout/draw must not request fixture allocation")
    }
}
impl core::image::Renderer for ResourceRenderer {
    type Handle = core::image::Handle;
    fn load_image(&self, _: &Self::Handle) -> Result<core::image::Allocation, core::image::Error> {
        panic!("fixture has no decoder or loader")
    }
    fn measure_image(&self, _: &Self::Handle) -> Option<Size<u32>> {
        self.image_measures.set(self.image_measures.get() + 1);
        self.image_size
    }
    fn draw_image(&mut self, image: core::image::Image, bounds: Rectangle, clip: Rectangle) {
        self.commands.push(Draw::Image(image, bounds, clip));
    }
}
impl core::svg::Renderer for ResourceRenderer {
    fn measure_svg(&self, _: &core::svg::Handle) -> Size<u32> {
        self.svg_measures.set(self.svg_measures.get() + 1);
        self.svg_size
    }
    fn draw_svg(&mut self, svg: core::svg::Svg, bounds: Rectangle, clip: Rectangle) {
        self.commands.push(Draw::Svg(svg, bounds, clip));
    }
}
pub fn observe<Theme>(
    mut element: iced::Element<'_, (), Theme, ResourceRenderer>,
    theme: &Theme,
    mut renderer: ResourceRenderer,
) -> (Size<iced::Length>, Size, ResourceRenderer) {
    use iced::advanced::{Layout, layout, widget::Tree};
    let size = element.as_widget().size();
    let mut tree = Tree::new(&element);
    assert!(tree.children.is_empty());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 80.0)),
    );
    element.as_widget().draw(
        &tree,
        &mut renderer,
        theme,
        &core::renderer::Style {
            text_color: iced::Color::BLACK,
        },
        Layout::new(&node),
        iced::mouse::Cursor::Unavailable,
        &Rectangle::with_size(Size::new(100.0, 80.0)),
    );
    (size, node.size(), renderer)
}
