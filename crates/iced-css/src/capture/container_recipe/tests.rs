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
#[allow(clippy::large_enum_variant)]
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
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<u32>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(17u32)
    }
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

#[test]
fn presentation_changes_and_restoration_preserve_native_class_and_state() {
    use crate::capture::ContainerPresentation;
    let base = Style {
        background: Some(iced::Color::BLACK.into()),
        text_color: Some(iced::Color::WHITE),
        border: iced::Border {
            width: 2.,
            color: iced::Color::BLACK,
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: iced::Color::BLACK,
            offset: iced::Vector::new(1., 2.),
            blur_radius: 3.,
        },
        snap: false,
    };
    let cases = [
        (ContainerPresentation::default(), base),
        (
            ContainerPresentation {
                background: Some(None),
                ..Default::default()
            },
            Style {
                background: None,
                ..base
            },
        ),
        (
            ContainerPresentation {
                background: Some(Some(iced::Color::WHITE.into())),
                ..Default::default()
            },
            Style {
                background: Some(iced::Color::WHITE.into()),
                ..base
            },
        ),
        (
            ContainerPresentation {
                text_color: Some(None),
                ..Default::default()
            },
            Style {
                text_color: None,
                ..base
            },
        ),
        (
            ContainerPresentation {
                text_color: Some(Some(iced::Color::BLACK)),
                ..Default::default()
            },
            Style {
                text_color: Some(iced::Color::BLACK),
                ..base
            },
        ),
        (
            ContainerPresentation {
                border: Some(iced::Border::default()),
                ..Default::default()
            },
            Style {
                border: iced::Border::default(),
                ..base
            },
        ),
        (
            ContainerPresentation {
                background: Some(None),
                text_color: Some(None),
                border: Some(iced::Border::default()),
            },
            Style {
                background: None,
                text_color: None,
                border: iced::Border::default(),
                ..base
            },
        ),
        (ContainerPresentation::default(), base),
    ];
    let mut recipe = ContainerRecipe::<(), Theme, Paint>::new(PaintedContent)
        .width(30)
        .height(20)
        .class(Class(base));
    assert_eq!(take(), ["default"]);
    let mut tree = Tree::new(&recipe as &dyn Widget<(), Theme, Paint>);
    let state_address = std::ptr::from_ref(tree.state.downcast_ref::<u32>());
    let mut paint = Paint::default();
    let viewport = iced::Rectangle::with_size(Size::new(100., 100.));
    let node = recipe.layout(
        &mut tree,
        &paint,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    for (presentation, expected) in cases {
        recipe.set_presentation(presentation);
        assert_eq!(
            std::ptr::from_ref(tree.state.downcast_ref::<u32>()),
            state_address
        );
        assert_eq!(*tree.state.downcast_ref::<u32>(), 17);
        assert!(take().is_empty());
        paint.0.clear();
        recipe.draw(
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
        let stock = iced::widget::Container::<(), Theme, Paint>::new(PaintedContent)
            .width(30)
            .height(20)
            .class(Class(expected));
        assert_eq!(take(), ["default"]);
        assert_eq!(paint.0, paint_frame(stock.into(), viewport));
    }
}
