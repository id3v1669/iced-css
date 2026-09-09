use iced_css_core::{
    animation::{combine::Combination, image::*},
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn parsed(input: &str) -> ParsedProperty {
    let n = |n| NonZeroUsize::new(n).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}

use iced_css_core::{
    resources::uri::{UriError, UriLimits},
    values::image::*,
};
fn limits() -> UriLimits {
    UriLimits {
        bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn parsed_images_resolve_before_whole_endpoint_selection() {
    for name in ["background-image", "list-style-image"] {
        let p = parsed(&format!("{name}:url(../img.png#part)"));
        let a = compute_image(&p, "https://example.test/a/theme.css", limits()).unwrap();
        let b = compute_image(&p, "https://example.test/b/c/theme.css", limits()).unwrap();
        let none = compute_image(&parsed(&format!("{name}:none")), "unused", limits()).unwrap();
        assert_eq!(
            a.component(),
            ImageComponent::Uri("https://example.test/img.png#part")
        );
        assert_eq!(
            b.component(),
            ImageComponent::Uri("https://example.test/b/img.png#part")
        );
        assert_eq!(none.component(), ImageComponent::None);
        assert_eq!(a.property().descriptor().name, name);
        for (start, end) in [(&a, &b), (&a, &none), (&none, &b), (&none, &none)] {
            for p in [
                -f32::MAX,
                0.0,
                f32::from_bits(0.5_f32.to_bits() - 1),
                0.5,
                1.0,
                f32::MAX,
            ] {
                assert!(std::ptr::eq(
                    combine_image(start, end, Combination::Interpolate(p)).unwrap(),
                    if p < 0.5 { start } else { end }
                ));
            }
            for op in [Combination::Add, Combination::Accumulate] {
                assert!(std::ptr::eq(combine_image(start, end, op).unwrap(), start));
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_image(start, end, Combination::Interpolate(p)).is_err());
            }
        }
    }
}
#[test]
fn property_identity_and_unresolved_values_are_checked() {
    let a = compute_image(&parsed("background-image:none"), "unused", limits()).unwrap();
    let b = compute_image(&parsed("list-style-image:none"), "unused", limits()).unwrap();
    for op in [
        Combination::Add,
        Combination::Accumulate,
        Combination::Interpolate(0.5),
    ] {
        assert_eq!(
            combine_image(&a, &b, op).unwrap_err(),
            ImageAnimationError::PropertyMismatch
        );
    }
    for css in ["background-image:inherit", "color:red", "width:2px"] {
        assert!(compute_image(&parsed(css), "https://example.test/", limits()).is_err());
    }
    let mut forged = parsed("background-image:none");
    forged.value = parsed("overflow:visible").value;
    assert!(compute_image(&forged, "unused", limits()).is_err());
}
#[test]
fn uri_errors_and_work_bounds_are_explicit() {
    let p = parsed("background-image:url(image.png)");
    assert_eq!(
        compute_image(&p, "relative/base", limits()).unwrap_err(),
        ImageComputationError::Uri(UriError::RelativeBase)
    );
    let mut tight = limits();
    tight.work = NonZeroUsize::MIN;
    assert_eq!(
        compute_image(&p, "https://example.test/", tight).unwrap_err(),
        ImageComputationError::Uri(UriError::Work)
    );
    tight = limits();
    tight.bytes = NonZeroUsize::MIN;
    assert_eq!(
        compute_image(&p, "https://example.test/", tight).unwrap_err(),
        ImageComputationError::Uri(UriError::Bytes)
    );
    let p = parsed("background-image:url(café.png)");
    assert_eq!(
        compute_image(&p, "https://example.test/", limits()).unwrap_err(),
        ImageComputationError::Uri(UriError::NonAscii)
    );
    let none = parsed("background-image:none");
    assert!(compute_image(&none, "unused", tight).is_ok());
}
