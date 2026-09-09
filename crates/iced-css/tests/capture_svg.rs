#![cfg(feature = "svg")]
use iced_css::capture::{CaptureLimits, SvgStyleSelection, adapters};
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
fn svg_leaf_retains_resource_and_defaults() {
    let handle = iced::widget::svg::Handle::from_memory(&b"not decoded"[..]);
    let captured = adapters::svg::<iced::Theme>(handle.clone(), limits()).unwrap();
    let node = &captured.construction().nodes()[0];
    assert_eq!(node.canonical_type(), "svg");
    assert_eq!(node.parent(), None);
    assert_eq!(node.text(), None);
    let source = node.svg_source().unwrap();
    assert_eq!(source.handle(), &handle);
    assert_eq!(source.hints().width, iced::Length::Fill);
    assert_eq!(source.hints().style_selection, SvgStyleSelection::Default);
}
#[path = "support/borrowed_svg_theme.rs"]
mod borrowed_svg_theme;
#[path = "support/resource_renderer.rs"]
mod resource_renderer;
use borrowed_svg_theme::{AuthoredClass, DEFAULT_CALLS, ProbeTheme};
use iced::widget::svg::{Handle, Style};
use iced::{ContentFit, Length};
use iced_test::core::svg::Data;
use resource_renderer::{Draw, ResourceRenderer, observe};
use std::{borrow::Cow, cell::Cell, path::PathBuf};
#[test]
fn handles_keep_variants_and_shared_data() {
    let borrowed = Handle::from_memory(&b"raw svg bytes"[..]);
    let owned = Handle::from_memory(vec![4; 200000]);
    let path = Handle::from_path("no-file-read");
    for handle in [borrowed, owned, path] {
        let address = handle.data() as *const Data;
        let c = adapters::svg::<ProbeTheme>(handle.clone(), limits()).unwrap();
        let s = c.construction().nodes()[0].svg_source().unwrap();
        assert_eq!(s.handle(), &handle);
        assert_eq!(s.handle().data() as *const Data, address);
        assert!(c.construction().retained_bytes() < 200000);
    }
    assert!(matches!(
        Handle::from_memory(&b"x"[..]).data(),
        Data::Bytes(Cow::Borrowed(_))
    ));
    assert!(matches!(
        Handle::from_memory(vec![1]).data(),
        Data::Bytes(Cow::Owned(_))
    ));
    struct Path<'a>(&'a Cell<usize>);
    impl From<Path<'_>> for PathBuf {
        fn from(v: Path<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            PathBuf::from("absent")
        }
    }
    let calls = Cell::new(0);
    let c = adapters::svg_from_path::<ProbeTheme>(Path(&calls), limits()).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(c.construction().nodes()[0].svg_source().unwrap().handle().data(),Data::Path(p) if p==std::path::Path::new("absent"))
    );
}
#[test]
fn all_geometry_builders_match_native() {
    let h = Handle::from_memory(vec![1]);
    for which in 0..7 {
        let mut c = adapters::svg::<iced::Theme>(h.clone(), limits()).unwrap();
        let mut n = iced::widget::Svg::new(h.clone());
        macro_rules! both {
            ($method:ident,$value:expr) => {{
                c = c.$method($value);
                n = n.$method($value);
            }};
        }
        match which {
            0 => {}
            1 => both!(width, 71.0),
            2 => both!(height, 33.0),
            3 => both!(content_fit, ContentFit::Cover),
            4 => both!(rotation, 0.4),
            5 => both!(opacity, 0.3_f32),
            _ => {
                both!(width, 71.0);
                both!(height, 33.0);
                both!(content_fit, ContentFit::Fill);
                both!(rotation, 0.2);
                both!(opacity, 0.7_f32);
                both!(width, 61.0);
            }
        }
        let (a, b, r) = observe(
            c.erase().into_parts().0,
            &iced::Theme::Dark,
            ResourceRenderer::default(),
        );
        let (x, y, s) = observe(n.into(), &iced::Theme::Dark, ResourceRenderer::default());
        assert_eq!((a, b), (x, y));
        assert_eq!(r.commands, s.commands);
        assert!(!r.commands.is_empty());
    }
}
#[test]
fn borrowed_callbacks_and_classes_stay_native_and_convert_once() {
    let color = iced::Color::from_rgb(0.1, 0.2, 0.3);
    let calls = Cell::new(0);
    let conversions = Cell::new(0);
    DEFAULT_CALLS.with(|v| v.set(0));
    let c = adapters::svg::<ProbeTheme>(Handle::from_memory(vec![1]), limits()).unwrap();
    assert_eq!(DEFAULT_CALLS.with(Cell::get), 1);
    let c = c.style(|_, _| {
        calls.set(calls.get() + 1);
        Style { color: Some(color) }
    });
    assert_eq!(calls.get(), 0);
    assert_eq!(
        c.construction().nodes()[0]
            .svg_source()
            .unwrap()
            .hints()
            .style_selection,
        SvgStyleSelection::Style
    );
    let (_, _, r) = observe(
        c.erase().into_parts().0,
        &ProbeTheme,
        ResourceRenderer::default(),
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(&r.commands[0],Draw::Svg(s,_,_) if s.color==Some(color)));
    let c = adapters::svg::<ProbeTheme>(Handle::from_memory(vec![1]), limits())
        .unwrap()
        .style(|_, _| panic!("replaced callback"))
        .class(AuthoredClass {
            conversions: &conversions,
            color: &color,
            calls: &calls,
        });
    assert_eq!(conversions.get(), 1);
    assert_eq!(calls.get(), 1);
    assert_eq!(
        c.construction().nodes()[0]
            .svg_source()
            .unwrap()
            .hints()
            .style_selection,
        SvgStyleSelection::Class
    );
    let _ = observe(
        c.erase().into_parts().0,
        &ProbeTheme,
        ResourceRenderer::default(),
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(conversions.get(), 1);
}
#[test]
fn conversions_and_nonfinite_hints_are_not_normalized() {
    struct Input<'a>(&'a Cell<usize>);
    impl From<Input<'_>> for Handle {
        fn from(v: Input<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Handle::from_memory(vec![1])
        }
    }
    struct Value<'a>(&'a Cell<usize>, f32);
    impl From<Value<'_>> for Length {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Length::Fixed(v.1)
        }
    }
    impl From<Value<'_>> for f32 {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    impl From<Value<'_>> for iced::Rotation {
        fn from(v: Value<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1.into()
        }
    }
    let calls = Cell::new(0);
    let c = adapters::svg::<ProbeTheme>(Input(&calls), limits())
        .unwrap()
        .width(Value(&calls, 2.0))
        .height(Value(&calls, 3.0))
        .rotation(Value(&calls, 0.5))
        .opacity(Value(&calls, f32::NEG_INFINITY));
    assert_eq!(calls.get(), 5);
    let h = c.construction().nodes()[0].svg_source().unwrap().hints();
    assert_eq!(h.width, Length::Fixed(2.0));
    assert_eq!(h.height, Length::Fixed(3.0));
    assert_eq!(h.rotation, 0.5.into());
    assert_eq!(h.opacity, f32::NEG_INFINITY);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    let before = DEFAULT_CALLS.with(Cell::get);
    assert!(matches!(
        adapters::svg::<ProbeTheme>(Input(&calls), cap),
        Err(iced_css::capture::CaptureError::Bytes)
    ));
    assert_eq!(calls.get(), 6);
    assert_eq!(DEFAULT_CALLS.with(Cell::get), before);
}
#[cfg(feature = "image-without-codecs")]
#[test]
fn resource_accessors_do_not_cross_families() {
    let c = adapters::svg::<ProbeTheme>(Handle::from_memory(vec![1]), limits()).unwrap();
    assert!(c.construction().nodes()[0].image_source().is_none());
}
#[path = "support/resource_source.rs"]
mod resource_source;
#[test]
fn source_publication_and_ua_join_retains_resource_identity() {
    resource_source::exercise(
        adapters::svg::<iced::Theme>(Handle::from_memory(vec![1]), limits())
            .unwrap()
            .erase(),
        "svg",
    );
}
#[test]
fn all_default_hints_match_stock() {
    let c = adapters::svg::<ProbeTheme>(Handle::from_memory(vec![]), limits()).unwrap();
    let h = c.construction().nodes()[0].svg_source().unwrap().hints();
    assert_eq!((h.width, h.height), (Length::Fill, Length::Shrink));
    assert_eq!(h.content_fit, ContentFit::Contain);
    assert_eq!(h.rotation, iced::Rotation::default());
    assert_eq!(h.opacity, 1.0);
    assert_eq!(h.style_selection, SvgStyleSelection::Default);
}

#[test]
fn shared_handle_does_not_merge_source_identity() {
    let h = Handle::from_memory(vec![1]);
    resource_source::distinct_sources(vec![
        adapters::svg::<iced::Theme>(h.clone(), limits())
            .unwrap()
            .erase(),
        adapters::svg::<iced::Theme>(h, limits()).unwrap().erase(),
    ]);
}
