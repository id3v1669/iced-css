#![cfg(feature = "image-without-codecs")]
use iced_css::capture::{CaptureLimits, RgbaBufferStatus, adapters};
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn resource_leaf_preserves_declared_rgba() {
    let handle = iced::widget::image::Handle::from_rgba(2, 2, vec![0; 15]);
    let captured = adapters::image(handle.clone(), limits()).unwrap();
    let node = &captured.construction().nodes()[0];
    assert_eq!(node.canonical_type(), "image");
    assert_eq!(node.parent(), None);
    assert_eq!(node.text(), None);
    assert_eq!(node.text_runs(), None);
    let source = node.image_source().unwrap();
    assert_eq!(source.handle(), &handle);
    let rgba = source.rgba_declaration().unwrap();
    assert_eq!(
        (rgba.width(), rgba.height(), rgba.pixel_bytes()),
        (2, 2, 15)
    );
    assert_eq!(rgba.buffer_status(), RgbaBufferStatus::LengthMismatch);
    assert_eq!(source.hints().width, iced::Length::Shrink);
}
#[path = "support/resource_renderer.rs"]
mod resource_renderer;
use iced::widget::image::{FilterMethod, Handle};
use iced::{ContentFit, Length, Rectangle, Rotation};
use resource_renderer::{ResourceRenderer, observe};
use std::{cell::Cell, path::PathBuf};
#[test]
fn resource_variants_and_shared_backing_are_truthful() {
    let mut path = PathBuf::from("absent-image");
    path.reserve(4096);
    let capacity = path.capacity();
    let c = adapters::image(Handle::from_path(path), limits()).unwrap();
    assert!(
        c.construction().retained_bytes()
            >= capacity + std::mem::size_of::<iced_css::capture::ConstructionNode>()
    );
    assert!(
        c.construction().nodes()[0]
            .image_source()
            .unwrap()
            .rgba_declaration()
            .is_none()
    );
    let data = iced_test::core::Bytes::from(vec![3; 200000]);
    let pointer = data.as_ptr();
    let c = adapters::image(Handle::from_bytes(data), limits()).unwrap();
    let Handle::Bytes(_, b) = c.construction().nodes()[0].image_source().unwrap().handle() else {
        panic!()
    };
    assert_eq!(b.as_ptr(), pointer);
    assert_eq!(b.len(), 200000);
    assert!(c.construction().retained_bytes() < 200000);
    for (w, h, len, status) in [
        (0, 7, 0, RgbaBufferStatus::ExactLength),
        (0, 7, 1, RgbaBufferStatus::LengthMismatch),
        (2, 3, 24, RgbaBufferStatus::ExactLength),
        (
            u32::MAX,
            u32::MAX,
            0,
            RgbaBufferStatus::RequiredBytesOverflow,
        ),
    ] {
        let c = adapters::image(Handle::from_rgba(w, h, vec![0; len]), limits()).unwrap();
        assert_eq!(
            c.construction().nodes()[0]
                .image_source()
                .unwrap()
                .rgba_declaration()
                .unwrap()
                .buffer_status(),
            status
        );
    }
}
#[test]
fn every_builder_preserves_native_layout_and_draw() {
    let h = Handle::from_rgba(40, 20, vec![0; 3200]);
    for which in 0..12 {
        let mut captured = adapters::image(h.clone(), limits()).unwrap();
        let mut native = iced::widget::Image::new(h.clone());
        macro_rules! both {
            ($method:ident,$value:expr) => {{
                captured = captured.$method($value);
                native = native.$method($value);
            }};
        }
        match which {
            0 => {}
            1 => both!(width, 71.0),
            2 => both!(height, 31.0),
            3 => both!(expand, true),
            4 => both!(content_fit, ContentFit::Cover),
            5 => both!(filter_method, FilterMethod::Nearest),
            6 => both!(rotation, Rotation::Solid(0.4.into())),
            7 => both!(opacity, 0.3_f32),
            8 => both!(scale, 1.7_f32),
            9 => both!(
                crop,
                Rectangle {
                    x: 3,
                    y: 2,
                    width: 17,
                    height: 11
                }
            ),
            10 => both!(border_radius, 6.0),
            _ => {
                both!(width, 71.0);
                both!(height, 31.0);
                both!(expand, true);
                both!(content_fit, ContentFit::Fill);
                both!(rotation, 0.2);
                both!(opacity, 0.8_f32);
                both!(scale, 0.7_f32);
                both!(filter_method, FilterMethod::Nearest);
                both!(
                    crop,
                    Rectangle {
                        x: 1,
                        y: 2,
                        width: 18,
                        height: 12
                    }
                );
                both!(border_radius, 3.0);
                both!(width, 66.0);
            }
        }
        let (a, b, r) = observe(
            captured.erase().into_parts().0,
            &iced::Theme::Dark,
            ResourceRenderer::default(),
        );
        let (x, y, s) = observe(
            native.into(),
            &iced::Theme::Dark,
            ResourceRenderer::default(),
        );
        assert_eq!((a, b), (x, y), "builder {which}");
        assert_eq!(r.commands, s.commands, "builder {which}");
        assert!(!r.commands.is_empty());
    }
}
#[test]
fn conversions_run_once_and_hints_keep_authored_bits() {
    struct Input<'a>(&'a Cell<usize>);
    impl From<Input<'_>> for Handle {
        fn from(v: Input<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::from_bytes(vec![1])
        }
    }
    struct Value<'a>(&'a Cell<usize>, f32);
    impl From<Value<'_>> for Length {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(v.1)
        }
    }
    impl From<Value<'_>> for f32 {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    impl From<Value<'_>> for Rotation {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1.into()
        }
    }
    impl From<Value<'_>> for iced::border::Radius {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1.into()
        }
    }
    let calls = Cell::new(0);
    let c = adapters::image(Input(&calls), limits()).unwrap();
    let pointer = c.construction().nodes().as_ptr();
    let nan = f32::from_bits(0x7fc00031);
    let c = c
        .width(Value(&calls, 2.0))
        .height(Value(&calls, 3.0))
        .rotation(Value(&calls, 0.4))
        .opacity(Value(&calls, nan))
        .scale(Value(&calls, f32::INFINITY))
        .border_radius(Value(&calls, 5.0))
        .expand(true)
        .content_fit(ContentFit::None)
        .filter_method(FilterMethod::Nearest)
        .crop(Rectangle {
            x: 4,
            y: 5,
            width: 6,
            height: 7,
        });
    assert_eq!(calls.get(), 7);
    assert_eq!(pointer, c.construction().nodes().as_ptr());
    let h = c.construction().nodes()[0].image_source().unwrap().hints();
    assert_eq!(h.opacity.to_bits(), nan.to_bits());
    assert_eq!(h.scale, f32::INFINITY);
    assert_eq!(h.width, Length::Fixed(2.0));
    assert_eq!(h.height, Length::Fixed(3.0));
    assert!(h.expand);
    assert_eq!(h.content_fit, ContentFit::None);
    assert_eq!(h.filter_method, FilterMethod::Nearest);
    assert_eq!(h.crop.unwrap().x, 4);
    assert_eq!(h.border_radius, 5.0.into());
    assert_eq!(h.rotation, 0.4.into());
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::image(Input(&calls), cap),
        Err(iced_css::capture::CaptureError::Bytes)
    ));
    assert_eq!(calls.get(), 8);
}
#[cfg(unix)]
#[test]
fn path_preserves_non_utf8_without_io() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'x', 255]));
    let c = adapters::image(Handle::from_path(path.clone()), limits()).unwrap();
    let Handle::Path(_, retained) = c.construction().nodes()[0].image_source().unwrap().handle()
    else {
        panic!()
    };
    assert_eq!(retained, &path);
}
#[cfg(feature = "svg")]
#[test]
fn resource_accessors_do_not_cross_families() {
    let c = adapters::image(Handle::from_bytes(vec![1]), limits()).unwrap();
    assert!(c.construction().nodes()[0].svg_source().is_none());
}
#[path = "support/resource_source.rs"]
mod resource_source;
#[test]
fn source_publication_and_ua_join_retains_resource_identity() {
    resource_source::exercise(
        adapters::image(Handle::from_bytes(vec![1]), limits())
            .unwrap()
            .erase(),
        "image",
    );
}
#[test]
fn all_default_hints_match_stock_and_rgba_arithmetic_is_host_sized() {
    let c = adapters::image(Handle::from_rgba(0, 0, vec![]), limits()).unwrap();
    let h = c.construction().nodes()[0].image_source().unwrap().hints();
    assert_eq!((h.width, h.height), (Length::Shrink, Length::Shrink));
    assert!(!h.expand);
    assert_eq!(h.content_fit, ContentFit::default());
    assert_eq!(h.filter_method, FilterMethod::default());
    assert_eq!(h.rotation, Rotation::default());
    assert_eq!((h.opacity, h.scale), (1.0, 1.0));
    assert_eq!(h.crop, None);
    assert_eq!(h.border_radius, iced::border::Radius::default());
    for w in [0, 1, 2, 65535, 65536, u32::MAX] {
        for h in [0, 1, 2, 65535, 65536, u32::MAX] {
            for len in [0, 1, 4, 16] {
                let required = u128::from(w) * u128::from(h) * 4;
                let expected = if required > usize::MAX as u128 {
                    RgbaBufferStatus::RequiredBytesOverflow
                } else if required == len as u128 {
                    RgbaBufferStatus::ExactLength
                } else {
                    RgbaBufferStatus::LengthMismatch
                };
                let c = adapters::image(Handle::from_rgba(w, h, vec![0; len]), limits()).unwrap();
                assert_eq!(
                    c.construction().nodes()[0]
                        .image_source()
                        .unwrap()
                        .rgba_declaration()
                        .unwrap()
                        .buffer_status(),
                    expected,
                    "{w} {h} {len}"
                );
            }
        }
    }
}

#[test]
fn shared_handle_does_not_merge_source_identity() {
    let h = Handle::from_bytes(vec![1]);
    resource_source::distinct_sources(vec![
        adapters::image(h.clone(), limits()).unwrap().erase(),
        adapters::image(h, limits()).unwrap().erase(),
    ]);
}
