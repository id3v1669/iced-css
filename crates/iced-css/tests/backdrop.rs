use iced::{Color, window::Settings};
use iced_css::paint::backdrop::*;
use std::num::NonZeroU64;
#[test]
fn theme_and_explicit_selection_follow_the_opaque_canvas_contract() {
    let theme = Color::from_rgb(0.2, 0.4, 0.6);
    assert_eq!(select_backdrop(None, theme).unwrap().color(), theme);
    assert_eq!(
        select_backdrop(None, Color { a: 0.5, ..theme })
            .unwrap()
            .color(),
        Color::WHITE
    );
    let explicit = OpaqueBackdrop::new(0.9, 0.8, 0.7).unwrap();
    assert_eq!(
        select_backdrop(
            Some(explicit),
            Color {
                r: f32::NAN,
                ..theme
            }
        )
        .unwrap(),
        explicit
    );
    assert!(
        select_backdrop(
            None,
            Color {
                r: f32::NAN,
                ..theme
            }
        )
        .is_err()
    );
}
#[test]
fn invalid_explicit_channels_and_incompatible_window_settings_are_rejected() {
    for value in [-1.0, 1.1, f32::NAN, f32::INFINITY] {
        assert!(OpaqueBackdrop::new(value, 0.0, 0.0).is_err());
        assert!(OpaqueBackdrop::new(0.0, value, 0.0).is_err());
        assert!(OpaqueBackdrop::new(0.0, 0.0, value).is_err());
    }
    assert!(validate_window(&Settings::default()).is_ok());
    assert!(matches!(
        validate_window(&Settings {
            transparent: true,
            ..Settings::default()
        }),
        Err(BackdropError::Transparency)
    ));
    assert!(matches!(
        validate_window(&Settings {
            blur: true,
            ..Settings::default()
        }),
        Err(BackdropError::Blur)
    ));
}
#[test]
fn window_local_revision_changes_only_when_canonical_backdrop_changes() {
    let white = OpaqueBackdrop::new(1.0, 1.0, 1.0).unwrap();
    let black = OpaqueBackdrop::new(0.0, 0.0, 0.0).unwrap();
    let mut state = BackdropState::new(white, NonZeroU64::new(2).unwrap());
    assert_eq!(state.snapshot().revision().get(), 1);
    assert!(!state.replace(white).unwrap());
    assert!(state.replace(black).unwrap());
    let current = state.snapshot();
    assert_eq!(current.revision().get(), 2);
    assert_eq!(current.backdrop(), black);
    assert!(
        !state
            .replace(OpaqueBackdrop::new(-0.0, 0.0, -0.0).unwrap())
            .unwrap()
    );
    assert!(matches!(
        state.replace(white),
        Err(BackdropError::RevisionExhausted)
    ));
    assert_eq!(state.snapshot(), current);
}
