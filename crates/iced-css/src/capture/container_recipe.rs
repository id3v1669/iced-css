//! An owned content boundary composed with iced's public Container helpers.
use iced::{
    Element, Length, Padding, Pixels, Size,
    advanced::{Renderer, widget::Id},
    alignment,
    widget::container::{Catalog, Style, StyleFn},
};

/// Retains the original content and native class without replaying construction.
/// The native Tree belongs directly to the content; the layout has one child.
/// Raw content mutation is an integration boundary, not permission to reuse stale
/// capture evidence after replacing authored content.
pub struct ContainerRecipe<'a, Message, Theme = iced::Theme, R = iced::Renderer>
where
    Theme: Catalog,
    R: Renderer,
{
    content: Element<'a, Message, Theme, R>,
    class: Theme::Class<'a>,
    id: Option<Id>,
    padding: Padding,
    size: Size<Length>,
    maximum: Size,
    horizontal: alignment::Horizontal,
    vertical: alignment::Vertical,
    clip: bool,
}
impl<'a, Message, Theme: Catalog, R: Renderer> ContainerRecipe<'a, Message, Theme, R> {
    pub(super) fn new(content: impl Into<Element<'a, Message, Theme, R>>) -> Self {
        let content = content.into();
        let hint = content.as_widget().size_hint();
        let size = Size::new(hint.width.fluid(), hint.height.fluid());
        let class = Theme::default();
        Self {
            content,
            class,
            id: None,
            padding: Padding::ZERO,
            size,
            maximum: Size::new(f32::INFINITY, f32::INFINITY),
            horizontal: alignment::Horizontal::Left,
            vertical: alignment::Vertical::Top,
            clip: false,
        }
    }
    /// Borrow content for native state preparation without constructing another Tree.
    pub fn native_content(&self) -> &Element<'a, Message, Theme, R> {
        &self.content
    }
    /// Borrow content for native lifecycle forwarding while retaining its owner.
    /// Replacing authored content requires fresh capture evidence.
    ///
    /// ```compile_fail,E0505
    /// use iced_css::capture::ContainerRecipe;
    /// use iced::{advanced::{Layout,widget::Tree},Rectangle,Vector};
    /// fn erase(mut recipe:ContainerRecipe<'_,(),iced::Theme,()>,tree:&mut Tree,layout:Layout<'_>) {
    ///     let overlay=recipe.native_content_mut().as_widget_mut().overlay(tree,layout,&(),&Rectangle::default(),Vector::ZERO);
    ///     let _:iced::Element<'_,(),iced::Theme,()>=recipe.into();
    ///     drop(overlay);
    /// }
    /// ```
    pub fn native_content_mut(&mut self) -> &mut Element<'a, Message, Theme, R> {
        &mut self.content
    }
    #[must_use]
    pub fn id(mut self, value: impl Into<Id>) -> Self {
        self.id = Some(value.into());
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.padding = value.into();
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.size.width = value.into();
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.size.height = value.into();
        self
    }
    #[must_use]
    pub fn max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.maximum.width = value.into().0;
        self
    }
    #[must_use]
    pub fn max_height(mut self, value: impl Into<Pixels>) -> Self {
        self.maximum.height = value.into().0;
        self
    }
    #[must_use]
    pub fn align_x(mut self, value: impl Into<alignment::Horizontal>) -> Self {
        self.horizontal = value.into();
        self
    }
    #[must_use]
    pub fn align_y(mut self, value: impl Into<alignment::Vertical>) -> Self {
        self.vertical = value.into();
        self
    }
    #[must_use]
    pub fn center(self, value: impl Into<Length>) -> Self {
        let value = value.into();
        self.center_x(value).center_y(value)
    }
    #[must_use]
    pub fn center_x(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Center)
    }
    #[must_use]
    pub fn center_y(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Center)
    }
    #[must_use]
    pub fn align_left(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Left)
    }
    #[must_use]
    pub fn align_right(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Right)
    }
    #[must_use]
    pub fn align_top(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Top)
    }
    #[must_use]
    pub fn align_bottom(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Bottom)
    }
    #[must_use]
    pub fn clip(mut self, value: bool) -> Self {
        self.clip = value;
        self
    }
    #[must_use]
    pub fn style(self, value: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class(Box::new(value) as StyleFn<'a, Theme>)
    }
    #[must_use]
    pub fn class(mut self, value: impl Into<Theme::Class<'a>>) -> Self {
        self.class = value.into();
        self
    }
}
mod widget;

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{Layout, Widget, layout, mouse, renderer, widget::Tree};
    use std::cell::RefCell;
    thread_local! { static TRACE: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }
    fn record(value: &'static str) {
        TRACE.with_borrow_mut(|v| v.push(value));
    }
    fn take() -> Vec<&'static str> {
        TRACE.with_borrow_mut(std::mem::take)
    }
    struct Theme;
    struct Class(Style);
    impl Catalog for Theme {
        type Class<'a> = Class;
        fn default<'a>() -> Self::Class<'a> {
            record("default");
            Class(Style::default())
        }
        fn style(&self, class: &Class) -> Style {
            record("style");
            class.0
        }
    }
    struct Content;
    impl Widget<(), Theme, ()> for Content {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fixed(7.0))
        }
        fn size_hint(&self) -> Size<Length> {
            record("hint");
            self.size()
        }
        fn layout(&mut self, _: &mut Tree, _: &(), limits: &layout::Limits) -> layout::Node {
            layout::Node::new(limits.resolve(Length::Shrink, Length::Shrink, Size::ZERO))
        }
        fn draw(
            &self,
            _: &Tree,
            _: &mut (),
            _: &Theme,
            _: &renderer::Style,
            _: Layout<'_>,
            _: mouse::Cursor,
            _: &iced::Rectangle,
        ) {
        }
    }
    struct Convert;
    impl<'a> From<Convert> for Element<'a, (), Theme, ()> {
        fn from(_: Convert) -> Self {
            record("into");
            Element::new(Content)
        }
    }
    #[test]
    fn construction_and_erasure_preserve_native_conversion_hint_and_default_phases() {
        let stock = iced::widget::Container::<(), Theme, ()>::new(Convert);
        let expected = take();
        assert_eq!(expected, ["into", "hint", "default"]);
        let recipe = ContainerRecipe::<(), Theme, ()>::new(Convert);
        assert_eq!(take(), expected);
        assert_eq!(recipe.size(), stock.size());
        let element: Element<'_, (), Theme, ()> = recipe.into();
        let _tree = Tree::new(&element);
        assert!(take().is_empty());
    }
    #[derive(Debug, PartialEq)]
    enum PaintEvent {
        Quad(renderer::Quad, iced::Background),
        Content(iced::Color, iced::Rectangle),
    }
    #[derive(Default)]
    struct Paint(Vec<PaintEvent>);
    impl Renderer for Paint {
        fn start_layer(&mut self, _: iced::Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _: iced::Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, q: renderer::Quad, b: impl Into<iced::Background>) {
            self.0.push(PaintEvent::Quad(q, b.into()));
        }
        fn reset(&mut self, _: iced::Rectangle) {
            self.0.clear();
        }
        fn allocate_image(
            &mut self,
            h: &iced_test::core::image::Handle,
            f: impl FnOnce(Result<iced_test::core::image::Allocation, iced_test::core::image::Error>)
            + Send
            + 'static,
        ) {
            Renderer::allocate_image(&mut (), h, f);
        }
    }
    struct PaintedContent;
    impl Widget<(), Theme, Paint> for PaintedContent {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Shrink, Length::Shrink)
        }
        fn layout(&mut self, _: &mut Tree, _: &Paint, limits: &layout::Limits) -> layout::Node {
            layout::Node::new(limits.resolve(Length::Shrink, Length::Shrink, Size::new(7., 5.)))
        }
        fn draw(
            &self,
            _: &Tree,
            r: &mut Paint,
            _: &Theme,
            s: &renderer::Style,
            _: Layout<'_>,
            _: mouse::Cursor,
            v: &iced::Rectangle,
        ) {
            r.0.push(PaintEvent::Content(s.text_color, *v));
        }
    }
    impl<'a> From<PaintedContent> for Element<'a, (), Theme, Paint> {
        fn from(value: PaintedContent) -> Self {
            Element::new(value)
        }
    }
    fn paint_frame(
        mut element: Element<'_, (), Theme, Paint>,
        viewport: iced::Rectangle,
    ) -> Vec<PaintEvent> {
        let mut tree = Tree::new(&element);
        let mut paint = Paint::default();
        let node = element.as_widget_mut().layout(
            &mut tree,
            &paint,
            &layout::Limits::new(Size::ZERO, Size::new(100., 100.)),
        );
        take();
        element.as_widget().draw(
            &tree,
            &mut paint,
            &Theme,
            &renderer::Style {
                text_color: iced::Color::BLACK,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        assert_eq!(take(), ["style"]);
        paint.0
    }
    #[test]
    fn native_paint_inheritance_clip_and_offscreen_style_phases_match() {
        for clip in [false, true] {
            for outside in [false, true] {
                let style = Style {
                    background: Some(iced::Color::WHITE.into()),
                    text_color: Some(iced::Color::from_rgb(1., 0., 0.)),
                    border: iced::Border {
                        width: 2.,
                        color: iced::Color::BLACK,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let stock = iced::widget::Container::<(), Theme, Paint>::new(PaintedContent)
                    .width(30)
                    .height(20)
                    .padding(3)
                    .clip(clip)
                    .class(Class(style));
                let recipe = ContainerRecipe::<(), Theme, Paint>::new(PaintedContent)
                    .width(30)
                    .height(20)
                    .padding(3)
                    .clip(clip)
                    .class(Class(style));
                let viewport = iced::Rectangle {
                    x: if outside { 200. } else { 5. },
                    y: 5.,
                    width: 50.,
                    height: 50.,
                };
                let native = paint_frame(stock.into(), viewport);
                let actual = paint_frame(recipe.into(), viewport);
                assert_eq!(actual, native);
                if outside {
                    assert!(actual.is_empty());
                } else {
                    assert_eq!(actual.len(), 2);
                    let expected = if clip {
                        iced::Rectangle {
                            x: 5.,
                            y: 5.,
                            width: 25.,
                            height: 15.,
                        }
                    } else {
                        viewport
                    };
                    assert_eq!(
                        actual[1],
                        PaintEvent::Content(iced::Color::from_rgb(1., 0., 0.), expected)
                    );
                }
            }
        }
    }
}
