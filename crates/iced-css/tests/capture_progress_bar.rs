use iced::{
    Element, Length, Rectangle, Size,
    advanced::{Layout, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::Cell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[derive(Default)]
struct Quads(Vec<Rectangle>);
impl iced::advanced::Renderer for Quads {
    fn start_layer(&mut self, _: Rectangle) {
        panic!("quad-only probe received a layer")
    }
    fn end_layer(&mut self) {
        panic!("quad-only probe received a layer end")
    }
    fn start_transformation(&mut self, _: iced::Transformation) {
        panic!("quad-only probe received a transformation")
    }
    fn end_transformation(&mut self) {
        panic!("quad-only probe received a transformation end")
    }
    fn fill_quad(&mut self, q: iced::advanced::renderer::Quad, _: impl Into<iced::Background>) {
        self.0.push(q.bounds)
    }
    fn reset(&mut self, _: Rectangle) {
        self.0.clear()
    }
    fn allocate_image(
        &mut self,
        _: &iced_test::core::image::Handle,
        _: impl FnOnce(Result<iced_test::core::image::Allocation, iced_test::core::image::Error>)
        + Send
        + 'static,
    ) {
        panic!("quad-only probe received an image")
    }
}
fn draw(mut e: Element<'_, (), iced::Theme, Quads>) -> Vec<Rectangle> {
    let mut renderer = Quads::default();
    let mut tree = Tree::new(&e);
    assert!(tree.children.is_empty());
    let node = e.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    e.as_widget().draw(
        &tree,
        &mut renderer,
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style {
            text_color: iced::Color::BLACK,
        },
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::with_size(Size::new(100.0, 100.0)),
    );
    renderer.0
}
#[test]
fn emitted_geometry_matches_stock_for_both_orientations_and_clamped_values() {
    for vertical in [false, true] {
        for (range, value, active) in [
            (0.0..=100.0, -25.0, 0.0),
            (0.0..=100.0, 0.0, 0.0),
            (0.0..=100.0, 25.0, 20.0),
            (0.0..=100.0, 100.0, 80.0),
            (0.0..=100.0, 125.0, 80.0),
            (10.0..=10.0, 20.0, 0.0),
        ] {
            let p = adapters::progress_bar::<iced::Theme>(range.clone(), value, limits())
                .unwrap()
                .length(80.0)
                .girth(10.0);
            let native = iced::widget::ProgressBar::new(range, value)
                .length(80.0)
                .girth(10.0);
            let (p, native) = if vertical {
                (p.vertical(), native.vertical())
            } else {
                (p, native)
            };
            let actual = draw(p.erase().into_parts().0);
            assert_eq!(actual, draw(native.into()));
            assert_eq!(
                actual[0].size(),
                if vertical {
                    Size::new(10.0, 80.0)
                } else {
                    Size::new(80.0, 10.0)
                }
            );
            if active == 0.0 {
                assert_eq!(actual.len(), 1);
            } else {
                assert_eq!(actual.len(), 2);
                assert_eq!(
                    actual[1],
                    if vertical {
                        Rectangle {
                            x: 0.0,
                            y: 80.0 - active,
                            width: 10.0,
                            height: active,
                        }
                    } else {
                        Rectangle {
                            x: 0.0,
                            y: 0.0,
                            width: active,
                            height: 10.0,
                        }
                    }
                );
            }
        }
    }
}
#[test]
fn progress_leaf_limits_conversions_and_orientation_keep_source_evidence() {
    struct Dimension<'a>(&'a Cell<usize>, f32);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(v.1)
        }
    }
    let calls = Cell::new(0);
    let p = adapters::progress_bar::<iced::Theme>(0.0..=100.0, 50.0, limits()).unwrap();
    let ptr = p.construction().nodes().as_ptr();
    let p = p
        .length(Dimension(&calls, 80.0))
        .girth(Dimension(&calls, 10.0))
        .vertical();
    assert_eq!(calls.get(), 2);
    assert_eq!(p.construction().nodes().as_ptr(), ptr);
    let n = p.construction().nodes();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].canonical_type(), "progress_bar");
    assert_eq!(n[0].text(), None);
    assert_eq!(n[0].parent(), None);
    let _ = draw(p.erase().into_parts().0);
    assert_eq!(calls.get(), 2);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::progress_bar::<iced::Theme>(0.0..=100.0, 50.0, cap),
        Err(CaptureError::Bytes)
    ));
}
#[test]
fn borrowed_progress_styles_and_classes_run_during_drawing() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::progress_bar::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::progress_bar::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |theme: &iced::Theme| {
            calls.set(calls.get() + 1);
            iced::widget::progress_bar::primary(theme)
        };
        let p = adapters::progress_bar::<iced::Theme>(0.0..=100.0, 25.0, limits()).unwrap();
        let before = calls.get();
        let p = if use_class {
            p.class(Class(&converted, Box::new(callback)))
        } else {
            p.style(callback)
        };
        assert_eq!(calls.get(), before);
        assert_eq!(converted.get(), usize::from(use_class));
        let (native, _) = p.erase().into_parts();
        assert_eq!(calls.get(), before);
        let _ = draw(native);
        assert_eq!(calls.get(), before + 1);
    }
}
