use iced::{
    Color, Rectangle, Size,
    advanced::{
        Renderer,
        renderer::{Headless, Quad},
    },
};
use iced_css::paint::layers::{ClipLayerError, ClipScope};
use iced_css_core::formatting::clip::ClipRegion;
fn rect(edges: [f32; 4]) -> ClipRegion {
    ClipRegion::from_edges(edges).unwrap()
}
fn fill(renderer: &mut iced::Renderer, bounds: Rectangle, color: Color) {
    renderer.fill_quad(
        Quad {
            bounds,
            snap: false,
            ..Quad::default()
        },
        color,
    );
}
#[test]
fn native_clip_scopes_preserve_nested_bounds_and_ordered_layer_pixels() {
    for scale in [1.0, 1.5, 2.0] {
        let mut renderer = iced::futures::executor::block_on(iced::Renderer::new(
            iced::Font::DEFAULT,
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .expect("required tiny-skia test backend");
        assert_eq!(Headless::name(&renderer), "tiny-skia");
        let viewport = Size::new(24.0 / scale, 24.0 / scale);
        let scope = ClipScope::for_viewport(viewport).unwrap();
        scope
            .with_clip(&mut renderer, ClipRegion::Unbounded, |renderer, _| {
                fill(
                    renderer,
                    Rectangle::with_size(viewport),
                    Color::from_rgb(0.0, 0.0, 1.0),
                );
            })
            .unwrap();
        let result = scope.with_clip(
            &mut renderer,
            rect([2.0, 2.0, 8.0, 8.0]),
            |renderer, parent| {
                parent
                    .with_clip(
                        renderer,
                        rect([-f32::MAX, -f32::MAX, f32::MAX, f32::MAX]),
                        |renderer, _| {
                            fill(
                                renderer,
                                Rectangle::with_size(viewport),
                                Color::from_rgb(1.0, 0.0, 0.0),
                            );
                            7usize
                        },
                    )
                    .unwrap()
            },
        );
        assert_eq!(result, Some(7));
        // A later managed operation gets its own layer, both outside and
        // overlapping the earlier parent/child clip.
        scope
            .with_clip(&mut renderer, ClipRegion::Unbounded, |renderer, _| {
                fill(
                    renderer,
                    Rectangle::new(iced::Point::ORIGIN, Size::new(4.0, 4.0)),
                    Color::from_rgb(0.0, 1.0, 0.0),
                );
            })
            .unwrap();
        let pixels = renderer.screenshot(Size::new(24, 24), scale, Color::BLACK);
        assert_eq!(pixels.len(), 24 * 24 * 4);
        for y in 0..24 {
            for x in 0..24 {
                let fx = (x as f32 + 0.5) / scale;
                let fy = (y as f32 + 0.5) / scale;
                let expected = if fx < 4.0 && fy < 4.0 {
                    [0, 255, 0, 255]
                } else if (2.0..8.0).contains(&fx) && (2.0..8.0).contains(&fy) {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                assert_eq!(
                    &pixels[(y * 24 + x) * 4..(y * 24 + x + 1) * 4],
                    expected,
                    "scale {scale} pixel {x},{y}"
                );
            }
        }
    }
}
#[test]
fn empty_clips_skip_callbacks_and_unbounded_children_keep_the_viewport() {
    let mut renderer = ();
    let scope = ClipScope::for_viewport(Size::new(10.0, 20.0)).unwrap();
    assert_eq!(
        scope.with_clip(&mut renderer, ClipRegion::Empty, |_, _| panic!(
            "empty region drew"
        )),
        None::<()>
    );
    assert_eq!(
        scope.with_clip(&mut renderer, rect([10.0, 0.0, 20.0, 20.0]), |_, _| panic!(
            "outside region drew"
        )),
        None::<()>
    );
    let edges = scope
        .with_clip(&mut renderer, ClipRegion::Unbounded, |_, child| {
            child.effective_clip().edges()
        })
        .unwrap();
    assert_eq!(edges, Some([0.0, 0.0, 10.0, 20.0]));
    let zero = ClipScope::for_viewport(Size::ZERO).unwrap();
    assert_eq!(
        zero.with_clip(&mut renderer, ClipRegion::Unbounded, |_, _| panic!(
            "zero viewport drew"
        )),
        None::<()>
    );
}
#[test]
fn invalid_viewports_cannot_reach_native_layers() {
    for size in [
        Size::new(-1.0, 10.0),
        Size::new(10.0, f32::NAN),
        Size::new(f32::INFINITY, 10.0),
    ] {
        assert!(matches!(
            ClipScope::for_viewport(size),
            Err(ClipLayerError::Viewport)
        ));
    }
}
